use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};

use crate::config::ProcessTarget;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredProcess {
    pub pid: u32,
    pub name: String,
    pub exe: Option<String>,
    pub matched_target_id: String,
    pub matched_target_label: String,
    /// True when this row was found via the child-process walk (not by direct
    /// name match). The UI can use this to render a subtler badge.
    #[serde(default)]
    pub via_children: bool,
    /// True when this row was matched only because the keyword appeared in
    /// the full exe path (target has `match_path: true`). UI renders a
    /// "路径" badge so users can sanity-check for false positives.
    #[serde(default)]
    pub via_path: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillOutcome {
    pub pid: u32,
    pub name: String,
    pub killed: bool,
    pub error: Option<String>,
}

/// Public yes/no wrapper around `match_kind` for callers (the process
/// watcher) that don't need to distinguish Direct from Path matches.
pub fn matches(target: &ProcessTarget, proc_name: &str, exe: Option<&str>) -> bool {
    match_kind(target, proc_name, exe).is_some()
}

/// Reason a process matched a target. Useful to surface in the UI so the
/// user can audit weaker matches (path-substring is more easily over-broad
/// than name matching).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatchKind {
    /// Matched on process name / basename / name-substring (steps 1-3).
    Direct,
    /// Matched only because the keyword appears in the full exe path
    /// (step 4, requires `match_path: true` on the target).
    Path,
}

/// Decide whether the target matches the process and, if so, how.
/// `case_insensitive` and `match_path` on the target control which steps
/// fire. Returns `None` for no match.
fn match_kind(target: &ProcessTarget, proc_name: &str, exe: Option<&str>) -> Option<MatchKind> {
    let needle = target.name.trim();
    if needle.is_empty() {
        return None;
    }
    let ci = target.case_insensitive;
    let norm = |s: &str| if ci { s.to_lowercase() } else { s.to_string() };
    let needle_n = norm(needle);
    let name_n = norm(proc_name);

    // 1. Whole-string equality on the process name.
    if name_n == needle_n {
        return Some(MatchKind::Direct);
    }

    // 2. Whole-string equality on the exe's file-name component.
    if let Some(e) = exe {
        if let Some(base) = std::path::Path::new(e).file_name() {
            let base_s = base.to_string_lossy();
            if norm(&base_s) == needle_n {
                return Some(MatchKind::Direct);
            }
        }
    }

    // 3. Substring of needle in process name.
    if name_n.contains(&needle_n) {
        return Some(MatchKind::Direct);
    }

    // 4. Substring of needle in full exe path — opt-in via `match_path`.
    //    No-op when the path is unreadable (None), which is what happens
    //    for system / other-user processes when the app isn't elevated.
    if target.match_path {
        if let Some(e) = exe {
            if norm(e).contains(&needle_n) {
                return Some(MatchKind::Path);
            }
        }
    }

    None
}

/// Snapshot every running process and return the ones matching `targets`
/// (plus, for targets with `match_children == true`, their descendants).
pub fn discover(targets: &[ProcessTarget]) -> Vec<DiscoveredProcess> {
    let enabled: Vec<&ProcessTarget> = targets.iter().filter(|t| t.enabled).collect();
    if enabled.is_empty() {
        return Vec::new();
    }

    let mut sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    // First pass: direct matches by name / basename / substring.
    // We record the matched target ID per PID so the child walk can later
    // attribute descendants to the correct target.
    let mut direct: Vec<(Pid, DiscoveredProcess, &ProcessTarget)> = Vec::new();
    let mut matched_pids: HashSet<Pid> = HashSet::new();
    for (pid, proc) in sys.processes() {
        let name = proc.name().to_string_lossy().to_string();
        let exe = proc.exe().map(|p| p.to_string_lossy().to_string());
        for t in &enabled {
            if let Some(kind) = match_kind(t, &name, exe.as_deref()) {
                direct.push((
                    *pid,
                    DiscoveredProcess {
                        pid: pid.as_u32(),
                        name: name.clone(),
                        exe: exe.clone(),
                        matched_target_id: t.id.clone(),
                        matched_target_label: t.label.clone(),
                        via_children: false,
                        via_path: kind == MatchKind::Path,
                    },
                    t,
                ));
                matched_pids.insert(*pid);
                break;
            }
        }
    }

    // Second pass: for each direct match whose target has `match_children`,
    // BFS down the process tree adding descendants to the result set.
    let want_children = enabled.iter().any(|t| t.match_children);
    let mut child_rows: Vec<DiscoveredProcess> = Vec::new();
    if want_children {
        // Build parent -> [child PID, ...] map once.
        let mut children_of: HashMap<Pid, Vec<Pid>> = HashMap::new();
        for (pid, proc) in sys.processes() {
            if let Some(parent) = proc.parent() {
                children_of.entry(parent).or_default().push(*pid);
            }
        }

        // Seed BFS from every direct match whose target opted into child walking.
        let mut queue: Vec<(Pid, &ProcessTarget)> = direct
            .iter()
            .filter(|(_, _, t)| t.match_children)
            .map(|(pid, _, t)| (*pid, *t))
            .collect();

        while let Some((pid, target)) = queue.pop() {
            let Some(children) = children_of.get(&pid) else {
                continue;
            };
            for &child_pid in children {
                if !matched_pids.insert(child_pid) {
                    continue;
                }
                let Some(child) = sys.process(child_pid) else {
                    continue;
                };
                let cname = child.name().to_string_lossy().to_string();
                let cexe = child.exe().map(|p| p.to_string_lossy().to_string());
                child_rows.push(DiscoveredProcess {
                    pid: child_pid.as_u32(),
                    name: cname,
                    exe: cexe,
                    matched_target_id: target.id.clone(),
                    matched_target_label: target.label.clone(),
                    via_children: true,
                    via_path: false,
                });
                queue.push((child_pid, target));
            }
        }
    }

    let mut out: Vec<DiscoveredProcess> = direct.into_iter().map(|(_, dp, _)| dp).collect();
    out.extend(child_rows);
    out
}

/// Kill a set of PIDs and return per-PID outcomes with a real error reason
/// when the syscall fails. On Windows, this uses `OpenProcess(PROCESS_TERMINATE)`
/// + `TerminateProcess` directly so we can surface `ACCESS_DENIED` distinctly
/// from "process gone" or other failures. On Unix, falls back to sysinfo's
/// `Process::kill` (which calls `kill(2)` with SIGKILL).
pub fn kill(pids: &[u32]) -> Vec<KillOutcome> {
    let mut sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let mut out = Vec::with_capacity(pids.len());
    for &pid in pids {
        let spid = sysinfo::Pid::from_u32(pid);
        let name = sys
            .process(spid)
            .map(|p| p.name().to_string_lossy().to_string())
            .unwrap_or_default();
        if sys.process(spid).is_none() {
            out.push(KillOutcome {
                pid,
                name,
                killed: false,
                error: Some("process not found (already exited?)".into()),
            });
            continue;
        }
        match kill_one(pid) {
            Ok(()) => out.push(KillOutcome {
                pid,
                name,
                killed: true,
                error: None,
            }),
            Err(reason) => out.push(KillOutcome {
                pid,
                name,
                killed: false,
                error: Some(reason),
            }),
        }
    }
    out
}

#[cfg(windows)]
fn kill_one(pid: u32) -> Result<(), String> {
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, GetLastError};
    use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if handle.is_null() {
            let code = GetLastError();
            return Err(if code == ERROR_ACCESS_DENIED {
                "Access denied — try running as administrator".into()
            } else {
                format!("OpenProcess failed (Win32 error {code})")
            });
        }
        let ok = TerminateProcess(handle, 1);
        let err_code = if ok == 0 { GetLastError() } else { 0 };
        let _ = CloseHandle(handle);
        if ok == 0 {
            return Err(if err_code == ERROR_ACCESS_DENIED {
                "Access denied — try running as administrator".into()
            } else {
                format!("TerminateProcess failed (Win32 error {err_code})")
            });
        }
        Ok(())
    }
}

#[cfg(not(windows))]
fn kill_one(pid: u32) -> Result<(), String> {
    // SIGKILL via libc::kill — same semantics as sysinfo's Process::kill but
    // we get the errno on failure.
    let ret = unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
    if ret == 0 {
        Ok(())
    } else {
        let errno = std::io::Error::last_os_error();
        Err(format!("kill(SIGKILL) failed: {errno}"))
    }
}

pub fn kill_matching(targets: &[ProcessTarget]) -> Vec<KillOutcome> {
    let found = discover(targets);
    let pids: Vec<u32> = found.iter().map(|p| p.pid).collect();
    kill(&pids)
}
