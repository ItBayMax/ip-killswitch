use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::broadcast;

use crate::config::AppConfig;
use crate::verdict::VerdictCache;

#[derive(Clone)]
pub struct AppState {
    pub app_dir: PathBuf,
    pub log_dir: PathBuf,
    pub config: Arc<Mutex<AppConfig>>,
    /// Latest IP verdict — the single source of truth for "is the egress IP
    /// currently in the allow-list?" Writers: the detection pipeline. Readers:
    /// UI, tray, and (in upcoming commits) the process-watcher and firewall
    /// modules. See `verdict.rs` for TTL / subscribe semantics.
    pub verdict: VerdictCache,
    pub events: broadcast::Sender<AppEvent>,
    pub scheduler_handle: Arc<Mutex<Option<tauri::async_runtime::JoinHandle<()>>>>,
    /// Runtime-only pause toggle. When true, the scheduler stays stopped even
    /// if the persisted `schedule` is non-disabled. Resets to false on restart.
    pub scheduler_paused: Arc<AtomicBool>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppEvent {
    DetectionStarted,
    DetectionFinished { matched: bool },
    Mismatch { detected: Vec<String>, allowed: Vec<String> },
    KillRequested { pids: Vec<u32> },
    Killed { count: usize },
    ConfigUpdated,
    LogLine { line: String },
}

impl AppState {
    pub fn new(app_dir: PathBuf, log_dir: PathBuf, cfg: AppConfig) -> Self {
        let (tx, _rx) = broadcast::channel(256);
        Self {
            app_dir,
            log_dir,
            config: Arc::new(Mutex::new(cfg)),
            verdict: VerdictCache::new(),
            events: tx,
            scheduler_handle: Arc::new(Mutex::new(None)),
            scheduler_paused: Arc::new(AtomicBool::new(false)),
        }
    }
}
