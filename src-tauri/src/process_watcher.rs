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
    use serde::Deserialize;
    use tauri::{AppHandle, Emitter};
    use tauri_plugin_notification::NotificationExt;
    use tracing::{debug, info, warn};
    use wmi::{COMLibrary, WMIConnection};

    use crate::processes;
    use crate::state::AppState;
    use crate::verdict;

    /// The `Win32_ProcessStartTrace` event class.
    ///
    /// IMPORTANT: the `#[serde(rename = "Win32_ProcessStartTrace")]` is what
    /// makes wmi's `notification::<T>()` produce the correct WQL query
    /// `SELECT * FROM Win32_ProcessStartTrace`. Without it the wmi crate
    /// uses the Rust struct name as the class name, which silently produces
    /// a query against a non-existent class and never fires events. (That
    /// was the bug in the previous version of this file.)
    #[derive(Deserialize, Debug)]
    #[serde(rename = "Win32_ProcessStartTrace")]
    #[serde(rename_all = "PascalCase")]
    #[allow(non_camel_case_types)]
    struct Win32_ProcessStartTrace {
        process_name: String,
        #[serde(rename = "ProcessID")]
        process_id: u32,
    }

    pub fn run_blocking(app: AppHandle, state: AppState) {
        info!("process-watcher: thread starting");

        // COM init for this thread (per-thread state).
        let com = match COMLibrary::new() {
            Ok(c) => c,
            Err(e) => {
                warn!("process-watcher: COMLibrary::new failed: {e}");
                return;
            }
        };
        info!("process-watcher: COM initialized");

        let conn = match WMIConnection::new(com) {
            Ok(c) => c,
            Err(e) => {
                warn!("process-watcher: WMIConnection::new failed: {e}");
                return;
            }
        };
        info!("process-watcher: WMI connection opened on root\\cimv2");

        // `notification::<T>()` builds `SELECT * FROM <T>` — correct for
        // an extrinsic event class like `Win32_ProcessStartTrace`. Don't
        // use `filtered_notification` here: that wraps the query inside
        // `__InstanceCreationEvent`, which is the wrong shape for ETW-
        // backed event classes.
        let iter = match conn.notification::<Win32_ProcessStartTrace>() {
            Ok(it) => {
                info!(
                    "process-watcher: subscribed to Win32_ProcessStartTrace \
                     — launch interception is active"
                );
                it
            }
            Err(e) => {
                warn!(
                    "process-watcher: subscribe to Win32_ProcessStartTrace failed: {e}. \
                     intercept_on_launch will be a no-op. Most common cause: not \
                     running as administrator (the kernel ETW class requires it). \
                     Re-launch the app via the 'Restart as admin' banner."
                );
                return;
            }
        };

        for ev_result in iter {
            match ev_result {
                Ok(ev) => {
                    debug!(
                        pid = ev.process_id,
                        name = %ev.process_name,
                        "process-watcher: process-start event"
                    );
                    handle(&app, &state, ev);
                }
                Err(e) => warn!("process-watcher: iterator error: {e}"),
            }
        }
        info!("process-watcher: iterator closed");
    }

    /// React to a single process-start event. No async, no shared locks
    /// held across long operations — everything is taken, used, dropped.
    fn handle(app: &AppHandle, state: &AppState, ev: Win32_ProcessStartTrace) {
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

        info!(
            pid = ev.process_id,
            name = %ev.process_name,
            exe = %exe.as_deref().unwrap_or("(unknown)"),
            target = %target.label,
            "process-watcher: candidate matched target rule"
        );

        // Verdict gate. If the cache is fresh AND says "mismatch", kill.
        // Stale / missing cache → fail-open with a log line; the next
        // scheduler tick will handle it.
        let verdict = state.verdict.current_fresh(ttl);
        match verdict {
            Some(v) if !v.matched && !v.allowed_ips.is_empty() => {
                info!(
                    pid = ev.process_id,
                    name = %ev.process_name,
                    detected = ?v.detected_ips,
                    allowed = ?v.allowed_ips,
                    "process-watcher: killing newly-started process — IP mismatch"
                );
                let outcomes = processes::kill(&[ev.process_id]);
                let killed_count = outcomes.iter().filter(|o| o.killed).count();
                let failed_reasons: Vec<String> = outcomes
                    .iter()
                    .filter(|o| !o.killed)
                    .filter_map(|o| o.error.clone())
                    .collect();

                // Right-bottom system toast so the user actually sees what
                // happened. Without this the action is invisible (the
                // intercepted app just "doesn't open").
                let title = if killed_count > 0 {
                    "已拦截进程启动"
                } else {
                    "拦截进程失败"
                };
                let body = if killed_count > 0 {
                    format!(
                        "{} (PID {}) 被结束 · 出口 IP 与允许列表不匹配",
                        ev.process_name, ev.process_id
                    )
                } else {
                    format!(
                        "{} (PID {}) 命中拦截规则但 kill 失败：{}",
                        ev.process_name,
                        ev.process_id,
                        failed_reasons.join("; ")
                    )
                };
                let _ = app.notification().builder().title(title).body(&body).show();
                let _ = app.emit("ipkillswitch://intercepted", &outcomes);

                info!(
                    pid = ev.process_id,
                    killed = killed_count,
                    "process-watcher: kill outcome"
                );
            }
            Some(_) => {
                // Either matched, or no allow-list configured — let through.
                debug!(
                    pid = ev.process_id,
                    "process-watcher: verdict OK, letting process run"
                );
            }
            None => {
                warn!(
                    pid = ev.process_id,
                    name = %ev.process_name,
                    target = %target.label,
                    "process-watcher: intercept candidate but verdict stale/missing — \
                     fail-open. Next scheduler tick will catch up."
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
