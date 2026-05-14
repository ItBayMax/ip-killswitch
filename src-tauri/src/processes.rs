use serde::{Deserialize, Serialize};
use sysinfo::{ProcessRefreshKind, RefreshKind, System};

use crate::config::ProcessTarget;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredProcess {
    pub pid: u32,
    pub name: String,
    pub exe: Option<String>,
    pub matched_target_id: String,
    pub matched_target_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillOutcome {
    pub pid: u32,
    pub name: String,
    pub killed: bool,
    pub error: Option<String>,
}

fn matches(target: &ProcessTarget, proc_name: &str, exe: Option<&str>) -> bool {
    let needle = target.name.trim();
    if needle.is_empty() {
        return false;
    }
    let cmp = |a: &str, b: &str| {
        if cfg!(windows) {
            a.eq_ignore_ascii_case(b)
        } else {
            a == b
        }
    };
    if cmp(proc_name, needle) {
        return true;
    }
    if let Some(e) = exe {
        // Match against the file name part of the exe path.
        let base = std::path::Path::new(e)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if !base.is_empty() && cmp(&base, needle) {
            return true;
        }
        if cmp(e, needle) {
            return true;
        }
    }
    // Allow substring match for friendlier UX when names differ slightly.
    let n = needle.to_ascii_lowercase();
    proc_name.to_ascii_lowercase().contains(&n)
}

pub fn discover(targets: &[ProcessTarget]) -> Vec<DiscoveredProcess> {
    let enabled: Vec<&ProcessTarget> = targets.iter().filter(|t| t.enabled).collect();
    if enabled.is_empty() {
        return Vec::new();
    }
    let mut sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let mut out: Vec<DiscoveredProcess> = Vec::new();
    for (pid, proc) in sys.processes() {
        let name = proc.name().to_string_lossy().to_string();
        let exe = proc.exe().map(|p| p.to_string_lossy().to_string());
        for t in &enabled {
            if matches(t, &name, exe.as_deref()) {
                out.push(DiscoveredProcess {
                    pid: pid.as_u32(),
                    name: name.clone(),
                    exe: exe.clone(),
                    matched_target_id: t.id.clone(),
                    matched_target_label: t.label.clone(),
                });
                break;
            }
        }
    }
    out
}

pub fn kill(pids: &[u32]) -> Vec<KillOutcome> {
    let mut sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let mut out = Vec::with_capacity(pids.len());
    for &pid in pids {
        let spid = sysinfo::Pid::from_u32(pid);
        match sys.process(spid) {
            Some(proc) => {
                let name = proc.name().to_string_lossy().to_string();
                let killed = proc.kill();
                out.push(KillOutcome {
                    pid,
                    name,
                    killed,
                    error: if killed { None } else { Some("kill() returned false".into()) },
                });
            }
            None => out.push(KillOutcome {
                pid,
                name: String::new(),
                killed: false,
                error: Some("process not found".into()),
            }),
        }
    }
    out
}

pub fn kill_matching(targets: &[ProcessTarget]) -> Vec<KillOutcome> {
    let found = discover(targets);
    let pids: Vec<u32> = found.iter().map(|p| p.pid).collect();
    kill(&pids)
}
