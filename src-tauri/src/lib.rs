mod commands;
mod config;
mod detector;
mod logger;
mod processes;
mod scheduler;
mod state;
mod tray;

use std::path::PathBuf;

use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tracing::{info, warn};

use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            crate::tray::show_main_window(&app);
        }))
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ));

    builder = builder
        .setup(|app| {
            let handle = app.handle().clone();
            let app_dir: PathBuf = app
                .path()
                .app_config_dir()
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            let log_dir: PathBuf = app
                .path()
                .app_log_dir()
                .unwrap_or_else(|_| app_dir.join("logs"));

            // Initialise file logging before anything else.
            let cfg = config::load(&app_dir).unwrap_or_default();
            let _log_handle = match logger::init(&log_dir, &cfg.log_level) {
                Ok(h) => Some(h),
                Err(e) => {
                    eprintln!("failed to init logger: {e}");
                    None
                }
            };
            info!(?app_dir, ?log_dir, "starting ip-killswitch");
            let state = AppState::new(app_dir.clone(), log_dir.clone(), cfg.clone());
            app.manage(state.clone());

            crate::tray::install(&handle).unwrap_or_else(|e| warn!("tray install failed: {e}"));

            // Force the main window's icon early so Windows picks it up before
            // the taskbar button is registered. Without this, on a fresh launch
            // (vs. show-from-tray) the taskbar shows a generic blank-document
            // icon — Tauri's default-icon plumbing races WebView2 attach.
            const WINDOW_ICON_BYTES: &[u8] = include_bytes!("../icons/icon.png");
            if let Some(win) = handle.get_webview_window("main") {
                match tauri::image::Image::from_bytes(WINDOW_ICON_BYTES) {
                    Ok(img) => {
                        if let Err(e) = win.set_icon(img) {
                            warn!("set_icon failed: {e}");
                        }
                    }
                    Err(e) => warn!("decode window icon failed: {e}"),
                }
            }

            // Apply autostart preference at boot.
            if cfg.autostart {
                let _ = handle.autolaunch().enable();
            }

            // Kick off the scheduler if configured.
            crate::scheduler::restart(handle.clone(), state.clone());

            // Hidden start when launched with --minimized (used by autolaunch).
            let argv: Vec<String> = std::env::args().collect();
            let start_hidden = argv.iter().any(|a| a == "--minimized");
            if start_hidden {
                if let Some(win) = handle.get_webview_window("main") {
                    let _ = win.hide();
                }
            }

            Ok(())
        })
        .on_window_event(on_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::detect_now,
            commands::list_target_processes,
            commands::kill_processes,
            commands::last_report,
            commands::read_logs,
            commands::open_log_dir,
            commands::autostart_status,
            commands::set_autostart,
            commands::quit_app,
            commands::show_main_window,
            commands::restart_scheduler,
            commands::scheduler_status,
            commands::pause_scheduler,
            commands::resume_scheduler,
        ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// Required for the autostart-plugin extension trait.
use tauri_plugin_autostart::ManagerExt;

fn on_window_event(window: &tauri::Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        let label = window.label().to_string();
        if label != "main" {
            return;
        }
        let app = window.app_handle();
        let state = app.state::<AppState>();
        let cfg = state.config.lock().clone();
        if cfg.close_to_tray {
            api.prevent_close();
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.hide();
            }
        } else if cfg.confirm_exit {
            // Defer to the frontend confirmation modal.
            api.prevent_close();
            let _ = app.emit("ipkillswitch://request-exit", ());
        }
    }
}

// `Emitter` needs to be in scope to call `app.emit(..)`.
use tauri::Emitter;
