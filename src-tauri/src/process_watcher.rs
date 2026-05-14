//! Event-driven process launch interception (Windows-only for now).
//!
//! Subscribes to `Win32_ProcessStartTrace` via WMI so we react to a new
//! process within ~50-500ms of its creation, rather than waiting for the
//! next scheduler tick. Used in conjunction with the cached IP verdict
//! (see `verdict.rs`): when a matched target spawns AND the cache says
//! "IP currently mismatched", we kill the new PID immediately.
//!
//! Subscription to `Win32_ProcessStartTrace` requires admin elevation —
//! without it WMI returns access-denied. We handle this gracefully by
//! logging a warning and leaving the watcher dormant; the rest of the app
//! continues to work and `intercept_on_launch` simply becomes a no-op.
//!
//! ## Threading model
//!
//! COM (and thus `WMIConnection`) has *per-thread* state and the connection
//! handle is `!Send` — we can't move it across an `await` inside the tokio
//! runtime. The pragmatic Rust pattern is to give COM its own dedicated OS
//! thread and use the synchronous WMI iterator on it. That's what `spawn()`
//! sets up here.
//!
//! ## On non-Windows platforms
//!
//! `spawn()` is a no-op stub. Equivalent event sources exist (kqueue
//! NOTE_FORK on macOS, fanotify/audit/proc-events on Linux) but are out
//! of scope for this commit.

use tauri::AppHandle;

use crate::state::AppState;

#[cfg(windows)]
pub fn spawn(app: AppHandle, state: AppState) {
    std::thread::Builder::new()
        .name("ip-killswitch-process-watcher".into())
        .spawn(move || windows_impl::run_blocking(app, state))
        .expect("failed to spawn process-watcher thread");
}

#[cfg(not(windows))]
pub fn spawn(_app: AppHandle, _state: AppState) {
    // Other platforms have their own kernel-level process-create event
    // mechanisms (kqueue/fanotify/audit), but those aren't implemented yet.
    // Falling through means `intercept_on_launch` is silently ignored on
    // these platforms — the scheduler-poll + kill loop still works.
    tracing::info!("process_watcher: skipped on non-Windows platform");
}

#[cfg(windows)]
mod windows_impl {
    use std::collections::HashMap;

    use serde::Deserialize;
    use tauri::{AppHandle, Emitter};
    use tracing::{info, warn};
    use wmi::{COMLibrary, FilterValue, WMIConnection};

    use crate::processes;
    use crate::state::AppState;
    use crate::verdict;

    /// Shape we deserialize each WMI event into. Field names map to the
    /// `Win32_ProcessStartTrace` class properties.
    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    struct ProcessStart {
        process_name: String,
        #[serde(rename = "ProcessID")]
        process_id: u32,
    }

    pub fn run_blocking(app: AppHandle, state: AppState) {
        // WMI needs COM initialized on the calling thread. The wmi crate's
        // COMLibrary takes care of that with per-thread state.
        let com = match COMLibrary::new() {
            Ok(c) => c,
            Err(e) => {
                warn!("process_watcher: COMLibrary init failed: {e}");
                return;
            }
        };
        let conn = match WMIConnection::new(com) {
            Ok(c) => c,
            Err(e) => {
                warn!("process_watcher: WMIConnection failed: {e}");
                return;
            }
        };

        let filters = HashMap::from([(
            "TargetInstance".to_string(),
            FilterValue::is_a::<ProcessStart>().expect("filter typed correctly"),
        )]);
        let iter = match conn.filtered_notification::<ProcessStart>(
            &filters,
            Some(std::time::Duration::from_secs(1)),
        ) {
            Ok(i) => i,
            Err(e) => {
                warn!(
                    "process_watcher: failed to subscribe to Win32_ProcessStartTrace \
                     ({e}). intercept_on_launch will be inactive — typical cause is \
                     running without admin rights."
                );
                return;
            }
        };
        info!("process_watcher: subscribed to Win32_ProcessStartTrace");

        for event in iter {
            match event {
                Ok(ev) => handle(&app, &state, ev),
                Err(e) => warn!("process_watcher: iterator error: {e}"),
            }
        }
        info!("process_watcher: iterator exhausted");
    }

    /// React to a single process-start event. No async, no shared locks
    /// held across long operations — everything is taken, used, dropped.
    fn handle(app: &AppHandle, state: &AppState, ev: ProcessStart) {
        let (targets, ttl) = {
            let cfg = state.config.lock();
            let targets: Vec<_> = cfg
                .processes
                .iter()
                .filter(|t| t.enabled && t.intercept_on_launch)
                .cloned()
                .collect();
            let ttl = verdict::ttl_for_schedule(&cfg.schedule);
            (targets, ttl)
        };
        if targets.is_empty() {
            return;
        }

        // Best-effort exe path. Fails silently for system / cross-user
        // processes when we're not elevated.
        let exe = current_exe_path(ev.process_id);

        let Some(target) = targets
            .iter()
            .find(|t| processes::matches(t, &ev.process_name, exe.as_deref()))
        else {
            return;
        };

        // Verdict gate. If the cache is fresh AND says "mismatch", kill.
        // Stale / missing cache → fail-open with a log line; the next
        // scheduler tick will handle it (consistent with current
        // architecture).
        let verdict = state.verdict.current_fresh(ttl);
        match verdict {
            Some(v) if !v.matched && !v.allowed_ips.is_empty() => {
                info!(
                    pid = ev.process_id,
                    name = %ev.process_name,
                    target = %target.label,
                    "intercept: killing newly-started process (IP verdict mismatch)"
                );
                let outcomes = processes::kill(&[ev.process_id]);
                let _ = app.emit("ipkillswitch://intercepted", &outcomes);
            }
            Some(_) => {
                // Either matched, or no allow-list configured — let through.
            }
            None => {
                warn!(
                    pid = ev.process_id,
                    name = %ev.process_name,
                    target = %target.label,
                    "intercept candidate but verdict stale/missing — fail-open, \
                     next scheduler tick will catch up"
                );
            }
        }
    }

    /// Best-effort exe path for a PID, used for path-substring matching.
    /// Returns None if the OpenProcess call is denied (typical for system
    /// processes when we're not elevated).
    fn current_exe_path(pid: u32) -> Option<String> {
        use windows_sys::Win32::Foundation::{CloseHandle, MAX_PATH};
        use windows_sys::Win32::System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
        };

        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return None;
            }
            let mut buf = vec![0u16; MAX_PATH as usize];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
            let _ = CloseHandle(handle);
            if ok == 0 {
                return None;
            }
            Some(String::from_utf16_lossy(&buf[..len as usize]))
        }
    }
}
