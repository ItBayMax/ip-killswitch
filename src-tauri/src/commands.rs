use anyhow::Result;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_notification::NotificationExt;
use tracing::{info, warn};

use crate::config::{self, AppConfig, KillMode, Provider};
use crate::detector::{self, DetectionReport};
use crate::processes::{self, DiscoveredProcess, KillOutcome};
use crate::scheduler::SchedulerState;
use crate::state::{AppEvent, AppState};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ManualOptions {
    pub providers: Option<Vec<Provider>>,
    pub allowed_ips: Option<Vec<String>>,
}

#[tauri::command]
pub fn get_config(state: tauri::State<'_, AppState>) -> AppConfig {
    state.config.lock().clone()
}

#[tauri::command]
pub async fn save_config(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    cfg: AppConfig,
) -> Result<(), String> {
    let dir = state.app_dir.clone();
    config::save(&dir, &cfg).map_err(|e| e.to_string())?;
    *state.config.lock() = cfg.clone();
    let _ = state.events.send(AppEvent::ConfigUpdated);

    // Apply side-effects from updated config.
    if let Ok(currently_enabled) = app.autolaunch().is_enabled() {
        if cfg.autostart && !currently_enabled {
            let _ = app.autolaunch().enable();
        } else if !cfg.autostart && currently_enabled {
            let _ = app.autolaunch().disable();
        }
    }
    crate::scheduler::restart(app.clone(), state.inner().clone());
    Ok(())
}

#[tauri::command]
pub async fn detect_now(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    options: Option<ManualOptions>,
) -> Result<DetectionReport, String> {
    run_detection_internal(
        app,
        state.inner().clone(),
        options.as_ref().and_then(|o| o.providers.clone()),
        options.as_ref().and_then(|o| o.allowed_ips.clone()),
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_target_processes(state: tauri::State<'_, AppState>) -> Vec<DiscoveredProcess> {
    let targets = state.config.lock().processes.clone();
    processes::discover(&targets)
}

#[tauri::command]
pub fn kill_processes(
    state: tauri::State<'_, AppState>,
    pids: Option<Vec<u32>>,
) -> Vec<KillOutcome> {
    let result = match pids {
        Some(pids) if !pids.is_empty() => processes::kill(&pids),
        _ => {
            let targets = state.config.lock().processes.clone();
            processes::kill_matching(&targets)
        }
    };
    let _ = state.events.send(AppEvent::Killed { count: result.iter().filter(|o| o.killed).count() });
    info!(?result, "kill_processes done");
    result
}

#[tauri::command]
pub fn last_report(state: tauri::State<'_, AppState>) -> Option<DetectionReport> {
    state.last_report.lock().clone()
}

#[tauri::command]
pub fn read_logs(state: tauri::State<'_, AppState>, max_kb: Option<usize>) -> String {
    let cap = max_kb.unwrap_or(256).clamp(8, 4096) * 1024;
    crate::logger::read_recent(&state.log_dir, cap)
}

#[tauri::command]
pub fn open_log_dir(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let dir = state.log_dir.clone();
    // Cross-platform open
    let res = open_path(&dir);
    if let Err(e) = res {
        warn!("open_log_dir failed: {e}");
        return Err(e.to_string());
    }
    let _ = app; // currently unused — kept for future plugin-opener migration
    Ok(())
}

fn open_path(p: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer").arg(p).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(p).spawn()?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(p).spawn()?;
    }
    Ok(())
}

#[tauri::command]
pub fn autostart_status(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    if enabled {
        app.autolaunch().enable().map_err(|e| e.to_string())
    } else {
        app.autolaunch().disable().map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn is_elevated() -> bool {
    crate::admin::is_elevated()
}

/// Spawn an elevated copy of this binary. Returns `true` if the user accepted
/// the UAC prompt and the new instance is launching, `false` if they declined.
/// On Unix this currently returns an error string — the UI shouldn't offer
/// the button there.
#[tauri::command]
pub async fn relaunch_as_admin() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(crate::admin::relaunch_as_admin)
        .await
        .map_err(|e| format!("join error: {e}"))?
}

#[tauri::command]
pub fn show_main_window(app: AppHandle) {
    crate::tray::show_main_window(&app);
}

#[tauri::command]
pub async fn restart_scheduler(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    crate::scheduler::restart(app, state.inner().clone());
    Ok(())
}

#[tauri::command]
pub fn scheduler_status(state: tauri::State<'_, AppState>) -> SchedulerState {
    crate::scheduler::current_state(state.inner())
}

// pause/resume are `async` on purpose: that puts the handler inside Tauri's
// async runtime, guaranteeing a live tokio context for any `spawn` invoked
// transitively (notably `scheduler::restart` for `resume`). A sync command
// would run on a worker thread that has no tokio context and would panic
// the moment `tokio::spawn` / `tauri::async_runtime::spawn` is touched.
#[tauri::command]
pub async fn pause_scheduler(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<SchedulerState, String> {
    crate::scheduler::pause(state.inner());
    crate::tray::refresh_state(&app, state.inner());
    let _ = app.emit("ipkillswitch://scheduler-changed", ());
    Ok(crate::scheduler::current_state(state.inner()))
}

#[tauri::command]
pub async fn resume_scheduler(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<SchedulerState, String> {
    crate::scheduler::resume(app.clone(), state.inner().clone());
    // `restart` (invoked by resume) already calls refresh_state.
    let _ = app.emit("ipkillswitch://scheduler-changed", ());
    Ok(crate::scheduler::current_state(state.inner()))
}

/// Heart of the detection flow: run probes, persist the report, emit events
/// and react to a mismatch by notifying / killing processes.
pub async fn run_detection_internal(
    app: AppHandle,
    state: AppState,
    override_providers: Option<Vec<Provider>>,
    override_allowed: Option<Vec<String>>,
) -> Result<DetectionReport> {
    let _ = state.events.send(AppEvent::DetectionStarted);
    let _ = app.emit("ipkillswitch://detection-started", ());

    let cfg_snapshot = state.config.lock().clone();
    let report =
        detector::run_detection(&cfg_snapshot, override_providers.clone(), override_allowed.clone())
            .await;
    *state.last_report.lock() = Some(report.clone());

    info!(
        matched = report.matched,
        detected = report.detected_ips.len(),
        "detection finished"
    );
    let _ = state
        .events
        .send(AppEvent::DetectionFinished { matched: report.matched });
    let _ = app.emit("ipkillswitch://detection-finished", &report);

    // Refresh tray colour now that we have a fresh report.
    crate::tray::refresh_state(&app, &state);

    // Only auto-react when no manual overrides were used and an allow-list is
    // configured. A manual one-shot from the UI should not auto-kill.
    let manual = override_providers.is_some() || override_allowed.is_some();
    if !manual && !report.allowed_ips.is_empty() && !report.matched {
        handle_mismatch(&app, &state, &report).await;
    }

    Ok(report)
}

async fn handle_mismatch(app: &AppHandle, state: &AppState, report: &DetectionReport) {
    let cfg = state.config.lock().clone();
    let _ = state.events.send(AppEvent::Mismatch {
        detected: report.detected_ips.clone(),
        allowed: report.allowed_ips.clone(),
    });
    let _ = app.emit("ipkillswitch://mismatch", &report);

    // System notification
    let title = "出口IP不匹配";
    let body = format!(
        "检测到 {}，期望 {}",
        if report.detected_ips.is_empty() {
            "未知".to_string()
        } else {
            report.detected_ips.join(", ")
        },
        report.allowed_ips.join(", ")
    );
    let _ = app
        .notification()
        .builder()
        .title(title)
        .body(&body)
        .show();

    match cfg.kill_mode {
        KillMode::Auto => {
            let killed = processes::kill_matching(&cfg.processes);
            let n = killed.iter().filter(|o| o.killed).count();
            let _ = state.events.send(AppEvent::Killed { count: n });
            let _ = app.emit("ipkillswitch://killed", &killed);
            let _ = app
                .notification()
                .builder()
                .title("已自动结束目标进程")
                .body(format!("共 {} 个进程被结束", n))
                .show();
        }
        KillMode::Confirm => {
            // Surface the prompt to the UI; frontend renders a modal dialog.
            let _ = app.emit("ipkillswitch://prompt-kill", &report);
        }
        KillMode::Manual => {
            // Notify only.
        }
    }
}
