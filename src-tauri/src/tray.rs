use std::sync::Mutex as StdMutex;

use anyhow::Result;
use once_cell::sync::OnceCell;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconEvent},
    AppHandle, Emitter, Manager,
};
use tracing::warn;

use crate::scheduler::SchedulerState;
use crate::state::AppState;

const ICON_IDLE: &[u8] = include_bytes!("../icons/tray-idle.png");
const ICON_OK: &[u8] = include_bytes!("../icons/tray-ok.png");
const ICON_WARN: &[u8] = include_bytes!("../icons/tray-warn.png");

/// `pause_resume` menu item handle, used to flip the label between
/// "暂停定时检测" and "恢复定时检测".
static PAUSE_RESUME_ITEM: OnceCell<StdMutex<MenuItem<tauri::Wry>>> = OnceCell::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayVisual {
    Idle,
    Ok,
    Warn,
}

pub fn install(app: &AppHandle) -> Result<()> {
    let handle = app.clone();
    let show = MenuItem::with_id(&handle, "show", "显示主窗口", true, None::<&str>)?;
    let detect = MenuItem::with_id(&handle, "detect", "立即检测", true, None::<&str>)?;
    let pause_resume =
        MenuItem::with_id(&handle, "pause_resume", "暂停定时检测", true, None::<&str>)?;
    let separator = MenuItem::with_id(&handle, "sep", "──────────", false, None::<&str>)?;
    let quit = MenuItem::with_id(&handle, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(
        &handle,
        &[&show, &detect, &pause_resume, &separator, &quit],
    )?;

    // Remember the pause/resume item so we can mutate its label later.
    let _ = PAUSE_RESUME_ITEM.set(StdMutex::new(pause_resume));

    if let Some(tray) = app.tray_by_id("main-tray") {
        tray.set_menu(Some(menu))?;
        tray.on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "detect" => {
                let state = app.state::<AppState>().inner().clone();
                let app2 = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) =
                        crate::commands::run_detection_internal(app2, state, None, None).await
                    {
                        warn!("tray detect failed: {e}");
                    }
                });
            }
            "pause_resume" => {
                let state = app.state::<AppState>().inner().clone();
                match crate::scheduler::current_state(&state) {
                    SchedulerState::Running => crate::scheduler::pause(&state),
                    SchedulerState::Paused => {
                        crate::scheduler::resume(app.clone(), state.clone())
                    }
                    SchedulerState::Disabled => {
                        // No schedule configured — nothing to pause/resume.
                    }
                }
                refresh_state(app, &state);
                let _ = app.emit("ipkillswitch://scheduler-changed", ());
            }
            "quit" => {
                let _ = app.emit("ipkillswitch://request-exit", ());
            }
            _ => {}
        });
        tray.on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    }
    Ok(())
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

/// Recompute tray visual + menu + tooltip from the current state. Cheap; safe
/// to call after every config change, detection result, or scheduler toggle.
pub fn refresh_state(app: &AppHandle, state: &AppState) {
    let sched_state = crate::scheduler::current_state(state);
    let report = state.last_report.lock().clone();

    let visual = match sched_state {
        SchedulerState::Disabled | SchedulerState::Paused => TrayVisual::Idle,
        SchedulerState::Running => match report.as_ref() {
            Some(r) if !r.allowed_ips.is_empty() => {
                if r.matched {
                    TrayVisual::Ok
                } else {
                    TrayVisual::Warn
                }
            }
            _ => TrayVisual::Idle,
        },
    };

    if let Some(tray) = app.tray_by_id("main-tray") {
        let bytes: &[u8] = match visual {
            TrayVisual::Idle => ICON_IDLE,
            TrayVisual::Ok => ICON_OK,
            TrayVisual::Warn => ICON_WARN,
        };
        match Image::from_bytes(bytes) {
            Ok(img) => {
                if let Err(e) = tray.set_icon(Some(img)) {
                    warn!("tray set_icon failed: {e}");
                }
            }
            Err(e) => warn!("decoding tray icon failed: {e}"),
        }
        let tooltip = tooltip_for(sched_state, report.as_ref());
        let _ = tray.set_tooltip(Some(tooltip.as_str()));
    }

    // Toggle menu item label.
    if let Some(item) = PAUSE_RESUME_ITEM.get() {
        if let Ok(mi) = item.lock() {
            let label = match sched_state {
                SchedulerState::Running => "暂停定时检测",
                SchedulerState::Paused => "恢复定时检测",
                SchedulerState::Disabled => "定时检测未启用",
            };
            let enabled = !matches!(sched_state, SchedulerState::Disabled);
            let _ = mi.set_text(label);
            let _ = mi.set_enabled(enabled);
        }
    }
}

fn tooltip_for(
    state: SchedulerState,
    report: Option<&crate::detector::DetectionReport>,
) -> String {
    let head = match state {
        SchedulerState::Disabled => "IP Killswitch · 未启用定时",
        SchedulerState::Paused => "IP Killswitch · 已暂停",
        SchedulerState::Running => "IP Killswitch · 运行中",
    };
    let tail = match report {
        None => "尚无检测记录".to_string(),
        Some(r) if r.allowed_ips.is_empty() => format!(
            "最近检测：{} (未配置目标IP)",
            r.detected_ips.first().cloned().unwrap_or_else(|| "—".into())
        ),
        Some(r) if r.matched => format!("匹配：{}", r.matched_ip.clone().unwrap_or_default()),
        Some(r) => format!(
            "不匹配：检测到 {}",
            if r.detected_ips.is_empty() {
                "—".to_string()
            } else {
                r.detected_ips.join(", ")
            }
        ),
    };
    format!("{head}\n{tail}")
}
