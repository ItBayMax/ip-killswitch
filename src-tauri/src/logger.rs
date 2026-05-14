use std::path::{Path, PathBuf};

use anyhow::Result;
use tracing_appender::{non_blocking::WorkerGuard, rolling};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[allow(dead_code)]
pub struct LogHandle {
    pub dir: PathBuf,
    _guard: WorkerGuard,
}

pub fn init(log_dir: &Path, level: &str) -> Result<LogHandle> {
    std::fs::create_dir_all(log_dir)?;
    let file_appender = rolling::daily(log_dir, "ip-killswitch.log");
    let (nb, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let file_layer = fmt::layer()
        .with_writer(nb)
        .with_ansi(false)
        .with_target(false);
    let stdout_layer = fmt::layer().with_target(false);

    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(file_layer)
        .with(stdout_layer)
        .try_init();

    Ok(LogHandle {
        dir: log_dir.to_path_buf(),
        _guard: guard,
    })
}

pub fn read_recent(dir: &Path, max_bytes: usize) -> String {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    // tracing-appender daily files are named <prefix>.<date>
    let candidate = dir.join(format!("ip-killswitch.log.{today}"));
    let path = if candidate.exists() {
        candidate
    } else {
        // pick the newest log file in the directory
        let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
        if let Ok(read) = std::fs::read_dir(dir) {
            for entry in read.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        if let Ok(mtime) = meta.modified() {
                            if entry
                                .file_name()
                                .to_string_lossy()
                                .starts_with("ip-killswitch.log")
                                && newest.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true)
                            {
                                newest = Some((mtime, entry.path()));
                            }
                        }
                    }
                }
            }
        }
        match newest {
            Some((_, p)) => p,
            None => return String::new(),
        }
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return String::new();
    };
    if bytes.len() <= max_bytes {
        return String::from_utf8_lossy(&bytes).to_string();
    }
    let start = bytes.len() - max_bytes;
    String::from_utf8_lossy(&bytes[start..]).to_string()
}
