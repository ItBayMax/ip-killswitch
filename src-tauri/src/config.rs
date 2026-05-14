use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const CONFIG_FILE: &str = "config.json";

/// Strategy used to consider IP detection a success across multiple providers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DetectStrategy {
    /// At least one provider returns an IP (default).
    Any,
    /// Every configured provider must return an IP and they must all agree.
    All,
}

impl Default for DetectStrategy {
    fn default() -> Self {
        Self::Any
    }
}

/// Action taken when egress IP does not match the configured allow-list.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KillMode {
    /// Show a confirmation dialog before killing.
    Confirm,
    /// Kill immediately without asking.
    Auto,
    /// Do nothing automatically; the user must manually trigger a kill from the UI.
    Manual,
}

impl Default for KillMode {
    fn default() -> Self {
        Self::Confirm
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default = "Provider::default_enabled")]
    pub enabled: bool,
    /// Optional explicit regex to extract the IP from the body. Falls back to
    /// the built-in IPv4/IPv6 extractor when empty.
    #[serde(default)]
    pub extract_regex: Option<String>,
}

impl Provider {
    fn default_enabled() -> bool {
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessTarget {
    pub id: String,
    /// Friendly label shown in UI.
    pub label: String,
    /// Matched against the process name (e.g. "chrome.exe" or "node").
    /// Comparison is case-insensitive on Windows, case-sensitive elsewhere.
    pub name: String,
    #[serde(default = "ProcessTarget::default_enabled")]
    pub enabled: bool,
}

impl ProcessTarget {
    fn default_enabled() -> bool {
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Schedule {
    Disabled,
    Interval { seconds: u64 },
    Cron { expr: String },
}

impl Default for Schedule {
    fn default() -> Self {
        Self::Disabled
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub providers: Vec<Provider>,
    #[serde(default)]
    pub allowed_ips: Vec<String>,
    #[serde(default)]
    pub processes: Vec<ProcessTarget>,
    #[serde(default)]
    pub strategy: DetectStrategy,
    #[serde(default)]
    pub kill_mode: KillMode,
    #[serde(default = "AppConfig::default_retry")]
    pub retry: u32,
    #[serde(default = "AppConfig::default_timeout_ms")]
    pub request_timeout_ms: u64,
    #[serde(default)]
    pub schedule: Schedule,
    #[serde(default = "AppConfig::default_autostart")]
    pub autostart: bool,
    #[serde(default = "AppConfig::default_minimize_to_tray")]
    pub minimize_to_tray: bool,
    #[serde(default = "AppConfig::default_close_to_tray")]
    pub close_to_tray: bool,
    #[serde(default = "AppConfig::default_confirm_exit")]
    pub confirm_exit: bool,
    #[serde(default = "AppConfig::default_log_level")]
    pub log_level: String,
}

impl AppConfig {
    fn default_retry() -> u32 {
        3
    }
    fn default_timeout_ms() -> u64 {
        8_000
    }
    fn default_autostart() -> bool {
        false
    }
    fn default_minimize_to_tray() -> bool {
        true
    }
    fn default_close_to_tray() -> bool {
        true
    }
    fn default_confirm_exit() -> bool {
        true
    }
    fn default_log_level() -> String {
        "info".into()
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            providers: vec![
                Provider {
                    id: uuid_v4(),
                    name: "ipify".into(),
                    url: "https://api.ipify.org".into(),
                    enabled: true,
                    extract_regex: None,
                },
                Provider {
                    id: uuid_v4(),
                    name: "icanhazip".into(),
                    url: "https://ipv4.icanhazip.com".into(),
                    enabled: true,
                    extract_regex: None,
                },
                Provider {
                    id: uuid_v4(),
                    name: "ifconfig.me".into(),
                    url: "https://ifconfig.me/ip".into(),
                    enabled: true,
                    extract_regex: None,
                },
            ],
            allowed_ips: vec![],
            processes: vec![],
            strategy: DetectStrategy::Any,
            kill_mode: KillMode::Confirm,
            retry: Self::default_retry(),
            request_timeout_ms: Self::default_timeout_ms(),
            schedule: Schedule::Disabled,
            autostart: false,
            minimize_to_tray: true,
            close_to_tray: true,
            confirm_exit: true,
            log_level: "info".into(),
        }
    }
}

fn uuid_v4() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn config_path(app_dir: &PathBuf) -> PathBuf {
    app_dir.join(CONFIG_FILE)
}

pub fn load(app_dir: &PathBuf) -> Result<AppConfig> {
    std::fs::create_dir_all(app_dir).with_context(|| format!("creating {}", app_dir.display()))?;
    let path = config_path(app_dir);
    if !path.exists() {
        let cfg = AppConfig::default();
        save(app_dir, &cfg)?;
        return Ok(cfg);
    }
    let bytes = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let cfg: AppConfig = serde_json::from_slice(&bytes)
        .with_context(|| format!("parsing config at {}", path.display()))?;
    Ok(cfg)
}

pub fn save(app_dir: &PathBuf, cfg: &AppConfig) -> Result<()> {
    std::fs::create_dir_all(app_dir).with_context(|| format!("creating {}", app_dir.display()))?;
    let path = config_path(app_dir);
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(cfg).context("serializing config")?;
    std::fs::write(&tmp, &bytes).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, &path).with_context(|| format!("renaming into {}", path.display()))?;
    Ok(())
}
