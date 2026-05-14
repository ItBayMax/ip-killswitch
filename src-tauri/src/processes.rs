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

/// Kill a set of PIDs and return per-PID outcomes with a real error reason
/// when the syscall fails. On Windows, this uses `OpenProcess(PROCESS_TERMINATE)`
/// + `TerminateProcess` directly so we can surface `ACCESS_DENIED` distinctly
/// from "process gone" or other failures. On Unix, falls back to libc's
/// `kill(2)` with SIGKILL.
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
