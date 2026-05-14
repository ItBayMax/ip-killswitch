//! Outbound network blocking for matched processes via `netsh advfirewall`.
//!
//! When the IP verdict transitions to "mismatched" (or starts that way),
//! we add Windows Firewall rules that deny outbound traffic for each
//! matched process's executable. This complements the `kill` path:
//! `kill` removes the running process; the firewall rule prevents a
//! freshly-spawned instance of the same exe from reaching the network
//! during the window where IP is still unsafe.
//!
//! When the verdict transitions back to "matched", we remove the rules
//! so the targeted apps can resume networking normally.
//!
//! ## Rule ownership
//!
//! Every rule we create is named with a fixed prefix (`ip-killswitch:`) so
//! we can identify our own rules at runtime — and so users can spot them
//! in `wf.msc` if they want to audit. We never touch rules without this
//! prefix, even if their target matches.
//!
//! ## Scopes
//!
//! - **Currently-running scope** (`firewall_block` on a `ProcessTarget`):
//!   when verdict is mismatched, enumerate processes matching that target
//!   right now, take their `exe` paths, add a rule per path.
//! - **Historical scope** (`AppConfig.firewall_block_include_historical_paths`):
//!   in addition to currently-running matches, also block exe paths that
//!   matched in any *prior* detection cycle this session. Catches the
//!   "user starts the app right after we just unblocked everything"
//!   pattern.
//!
//! Both scopes operate on the same rule set; "historical" just widens
//! the input list.
//!
//! ## Non-Windows platforms
//!
//! Right now this module is a no-op on non-Windows. macOS would use
//! `pfctl`; Linux would use `iptables` / `nftables` rules. Add per-OS
//! impl when needed; the public API on `FirewallManager` is intentionally
//! platform-agnostic so callers don't need cfg-gates.

use std::collections::HashSet;
use std::sync::Arc;

use parking_lot::Mutex;
use tracing::{debug, info, warn};

const RULE_PREFIX: &str = "ip-killswitch:block:";

/// Process-level firewall manager.
///
/// Holds two pieces of state:
/// - `active_rules`: rule names we've added and not yet removed.
/// - `known_paths`: every exe path we've ever seen match a target this
///   session, used for the historical-scope option.
#[derive(Clone, Default)]
pub struct FirewallManager {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    active_rules: HashSet<String>,
    known_paths: HashSet<String>,
}

impl FirewallManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an exe path as "we've seen this match a target before".
    /// Used by the historical-scope branch of `apply_block_set`.
    pub fn remember_path(&self, exe: String) {
        self.inner.lock().known_paths.insert(exe);
    }

    pub fn known_paths(&self) -> Vec<String> {
        self.inner.lock().known_paths.iter().cloned().collect()
    }

    /// Idempotently bring the active rule set to exactly `paths`. Adds
    /// rules for new paths, removes rules for paths no longer in the set.
    /// Pass an empty set to remove everything (e.g. when the verdict
    /// transitions back to matched, or on app shutdown).
    pub fn apply_block_set(&self, paths: &HashSet<String>) {
        let (to_add, to_remove) = {
            let inner = self.inner.lock();
            let want_names: HashSet<String> =
                paths.iter().map(|p| rule_name_for(p)).collect();
            let to_add: Vec<(String, String)> = paths
                .iter()
                .filter(|p| !inner.active_rules.contains(&rule_name_for(p)))
                .map(|p| (rule_name_for(p), p.clone()))
                .collect();
            let to_remove: Vec<String> = inner
                .active_rules
                .iter()
                .filter(|n| !want_names.contains(*n))
                .cloned()
                .collect();
            (to_add, to_remove)
        };

        for (name, path) in to_add {
            match add_rule(&name, &path) {
                Ok(()) => {
                    info!(rule = %name, path = %path, "firewall: added block rule");
                    self.inner.lock().active_rules.insert(name);
                }
                Err(e) => {
                    warn!(rule = %name, path = %path, "firewall: add failed: {e}");
                }
            }
        }
        for name in to_remove {
            match remove_rule(&name) {
                Ok(()) => {
                    debug!(rule = %name, "firewall: removed block rule");
                    self.inner.lock().active_rules.remove(&name);
                }
                Err(e) => {
                    warn!(rule = %name, "firewall: remove failed: {e}");
                }
            }
        }
    }

    /// Best-effort: remove every rule we've added. Call on app shutdown
    /// so we don't leave the user with networking blocked after a crash.
    pub fn cleanup(&self) {
        let names: Vec<String> = self.inner.lock().active_rules.drain().collect();
        for name in names {
            let _ = remove_rule(&name);
        }
        info!("firewall: cleanup done");
    }
}

/// Derive a rule name from an exe path. Includes the basename (so it's
/// recognizable in `wf.msc`) plus a hash suffix (so two exes with the
/// same basename in different dirs don't collide).
fn rule_name_for(exe_path: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let basename = std::path::Path::new(exe_path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".into());
    let mut hasher = DefaultHasher::new();
    exe_path.hash(&mut hasher);
    format!("{RULE_PREFIX}{basename}:{:016x}", hasher.finish())
}

#[cfg(windows)]
fn add_rule(name: &str, exe_path: &str) -> Result<(), String> {
    use std::process::Command;
    let output = Command::new("netsh")
        .args([
            "advfirewall",
            "firewall",
            "add",
            "rule",
            &format!("name={name}"),
            "dir=out",
            &format!("program={exe_path}"),
            "action=block",
            "enable=yes",
            "profile=any",
        ])
        .output()
        .map_err(|e| format!("netsh spawn: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "netsh exit {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn remove_rule(name: &str) -> Result<(), String> {
    use std::process::Command;
    let output = Command::new("netsh")
        .args([
            "advfirewall",
            "firewall",
            "delete",
            "rule",
            &format!("name={name}"),
        ])
        .output()
        .map_err(|e| format!("netsh spawn: {e}"))?;
    if !output.status.success() {
        // Already deleted? netsh returns non-zero. Not fatal.
        return Err(format!(
            "netsh exit {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
fn add_rule(_name: &str, _exe_path: &str) -> Result<(), String> {
    Err("firewall: only implemented on Windows".into())
}

#[cfg(not(windows))]
fn remove_rule(_name: &str) -> Result<(), String> {
    Err("firewall: only implemented on Windows".into())
}
