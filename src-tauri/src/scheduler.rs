use std::str::FromStr;
use std::sync::atomic::Ordering;
use std::time::Duration;

use chrono::Utc;
use cron::Schedule as CronSchedule;
use tauri::{async_runtime, AppHandle};
use tokio::time::{interval, sleep, MissedTickBehavior};
use tracing::{info, warn};

use crate::config::Schedule;
use crate::state::AppState;

/// Possible runtime states of the scheduler, surfaced to the UI and the tray.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SchedulerState {
    /// No schedule configured.
    Disabled,
    /// Schedule configured but user has paused it for this session.
    Paused,
    /// Schedule configured and the timer task is alive.
    Running,
}

pub fn current_state(state: &AppState) -> SchedulerState {
    let kind_disabled = matches!(state.config.lock().schedule, Schedule::Disabled);
    if kind_disabled {
        return SchedulerState::Disabled;
    }
    if state.scheduler_paused.load(Ordering::SeqCst) {
        return SchedulerState::Paused;
    }
    if state.scheduler_handle.lock().is_some() {
        SchedulerState::Running
    } else {
        SchedulerState::Paused
    }
}

/// Stop any running scheduler task without touching the pause flag.
pub fn stop(state: &AppState) {
    if let Some(h) = state.scheduler_handle.lock().take() {
        h.abort();
        info!("scheduler stopped");
    }
}

/// User-initiated pause: stop the running task and remember the pause until
/// `resume` (or app restart) flips it back.
pub fn pause(state: &AppState) {
    state.scheduler_paused.store(true, Ordering::SeqCst);
    stop(state);
}

/// User-initiated resume: clear the pause flag and rebuild the task from the
/// persisted schedule, if any.
pub fn resume(app: AppHandle, state: AppState) {
    state.scheduler_paused.store(false, Ordering::SeqCst);
    restart(app, state);
}

/// Replace the current schedule task with one derived from the persisted
/// config. Safe to call repeatedly. If the user has paused the scheduler
/// (`scheduler_paused == true`), this still stops the task and leaves it
/// stopped. Whoever calls `restart` is responsible for updating tray UI
/// afterwards via `crate::tray::refresh_state`.
pub fn restart(app: AppHandle, state: AppState) {
    stop(&state);
    if state.scheduler_paused.load(Ordering::SeqCst) {
        info!("scheduler paused — leaving timer stopped");
        crate::tray::refresh_state(&app, &state);
        return;
    }
    let schedule = state.config.lock().schedule.clone();
    match schedule {
        Schedule::Disabled => {
            info!("scheduler disabled");
        }
        Schedule::Interval { seconds } => {
            let secs = seconds.max(30);
            let app2 = app.clone();
            let state2 = state.clone();
            // `async_runtime::spawn` uses Tauri's globally-registered runtime
            // and therefore works from any thread — including the synchronous
            // command worker that handles pause/resume invocations and the
            // tray menu event callback. Using bare `tokio::spawn` here would
            // panic when called outside an async context.
            let h = async_runtime::spawn(async move {
                let mut t = interval(Duration::from_secs(secs));
                t.set_missed_tick_behavior(MissedTickBehavior::Delay);
                // skip the immediate first tick
                t.tick().await;
                loop {
                    t.tick().await;
                    run_one(&app2, &state2).await;
                }
            });
            *state.scheduler_handle.lock() = Some(h);
            info!("scheduler started: every {secs}s");
        }
        Schedule::Cron { expr } => {
            let app2 = app.clone();
            let state2 = state.clone();
            let expr_for_task = expr.clone();
            let h = async_runtime::spawn(async move {
                loop {
                    let parsed = CronSchedule::from_str(expr_for_task.as_str());
                    let sched = match parsed {
                        Ok(s) => s,
                        Err(e) => {
                            warn!("invalid cron expression {expr_for_task:?}: {e}");
                            sleep(Duration::from_secs(60)).await;
                            continue;
                        }
                    };
                    let now = Utc::now();
                    let Some(next) = sched.after(&now).next() else {
                        warn!("cron expression has no future occurrences: {expr_for_task:?}");
                        sleep(Duration::from_secs(60)).await;
                        continue;
                    };
                    let wait_ms = (next - now).num_milliseconds().max(1_000);
                    sleep(Duration::from_millis(wait_ms as u64)).await;
                    run_one(&app2, &state2).await;
                }
            });
            *state.scheduler_handle.lock() = Some(h);
            info!("scheduler started with cron: {expr}");
        }
    }
    crate::tray::refresh_state(&app, &state);
}

async fn run_one(app: &AppHandle, state: &AppState) {
    info!("scheduled detection tick");
    let result = crate::commands::run_detection_internal(app.clone(), state.clone(), None, None).await;
    if let Err(e) = result {
        warn!("scheduled detection failed: {e}");
    }
}
