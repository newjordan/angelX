//! Sandboxed process execution with timeouts and process-group kill.

use super::*;
use crate::agent::sandbox::process_owner::OwnedCommandExt;

mod activity;
mod delegate_activity;
mod handoff;
mod release;
pub(crate) use activity::{
    ChildSnapshot, link_child_owner, owned_child_active, owned_child_setting_up,
    owned_child_snapshot,
};
pub(crate) use delegate_activity::{
    DelegateSnapshot, observe_delegate_turn, owned_delegate_snapshot,
};
pub(crate) use handoff::{Handoff, tool_handoff_after, with_handoff};
pub(crate) use release::{
    Release, clear_release, release_pending, request_release, take_watchdog_release,
};

thread_local! {
    static TOOL_IDLE_ESCALATED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

thread_local! {
    static SANDBOX_RECEIPT: std::cell::RefCell<Option<serde_json::Value>> = const { std::cell::RefCell::new(None) };
}

thread_local! {
    static CALL_BUDGET: std::cell::Cell<Option<Duration>> = const { std::cell::Cell::new(None) };
}

pub(crate) fn with_call_budget<T>(budget: Option<Duration>, f: impl FnOnce() -> T) -> T {
    struct Restore(Option<Duration>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CALL_BUDGET.with(|cell| cell.set(self.0));
        }
    }
    let previous = call_budget();
    let effective = match (budget, previous) {
        (Some(budget), Some(outer)) => Some(budget.min(outer)),
        (budget, outer) => budget.or(outer),
    };
    let _restore = Restore(CALL_BUDGET.with(|cell| cell.replace(effective)));
    f()
}

pub(crate) fn call_budget() -> Option<Duration> {
    CALL_BUDGET.with(|cell| cell.get())
}

fn within_call_budget(bound: Option<Duration>) -> Option<Duration> {
    match (bound, call_budget()) {
        (Some(bound), Some(budget)) => Some(bound.min(budget)),
        (bound, None) => bound,
        (None, budget) => budget,
    }
}

pub(crate) fn set_sandbox_receipt(receipt: Option<serde_json::Value>) {
    SANDBOX_RECEIPT.with(|cell| *cell.borrow_mut() = receipt);
}

pub(crate) fn sandbox_receipt() -> Option<serde_json::Value> {
    SANDBOX_RECEIPT.with(|cell| cell.borrow().clone())
}

pub(crate) fn take_tool_idle_escalation() -> bool {
    TOOL_IDLE_ESCALATED.with(|flag| flag.replace(false))
}

pub(crate) fn tool_idle_timeout() -> Option<Duration> {
    match std::env::var("ANGEL_TOOL_IDLE_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
    {
        Some(seconds) if seconds > 0 => Some(Duration::from_secs(seconds)),
        _ => None,
    }
}

fn signal_name(signal: i32) -> String {
    match signal {
        libc::SIGTERM => "SIGTERM".into(),
        libc::SIGKILL => "SIGKILL".into(),
        libc::SIGSEGV => "SIGSEGV".into(),
        libc::SIGINT => "SIGINT".into(),
        libc::SIGPIPE => "SIGPIPE".into(),
        _ => format!("signal-{signal}"),
    }
}

pub(crate) struct ExecObservation {
    pub(crate) output: String,
    pub(crate) exit: Option<i32>,
    pub(crate) timed_out: bool,
    pub(crate) cancelled: bool,
    pub(crate) dur_ms: u128,
    pub(crate) kill: Option<crate::agent::sandbox::process_owner::KillReceipt>,
    pub(crate) handed_off: Option<u64>,
}

pub(crate) fn run_sandboxed_observed(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    policy: &SandboxPolicy,
) -> Result<ExecObservation, String> {
    run_sandboxed_observed_cancellable(program, args, cwd, policy, None)
}

pub(crate) fn run_sandboxed_observed_cancellable(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    policy: &SandboxPolicy,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<ExecObservation, String> {
    run_sandboxed_observed_cancellable_with_progress(program, args, cwd, policy, cancel, None)
}

pub(crate) fn run_sandboxed_observed_cancellable_with_progress(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    policy: &SandboxPolicy,
    cancel: Option<&std::sync::atomic::AtomicBool>,
    progress: Option<Arc<ToolOutputProgress>>,
) -> Result<ExecObservation, String> {
    run_observed(program, args, cwd, policy, cancel, progress, OutputBound::Excerpt)
}

pub(crate) fn run_shell_tool_observed(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    policy: &SandboxPolicy,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<ExecObservation, String> {
    run_observed(program, args, cwd, policy, cancel, None, OutputBound::ToolCeiling)
}

#[derive(Clone, Copy)]
enum OutputBound {
    Excerpt,
    ToolCeiling,
}

const EXCERPT_BYTES: usize = 4000;

fn run_observed(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    policy: &SandboxPolicy,
    cancel: Option<&std::sync::atomic::AtomicBool>,
    progress: Option<Arc<ToolOutputProgress>>,
    bound: OutputBound,
) -> Result<ExecObservation, String> {
    set_sandbox_receipt(None);
    let started = Instant::now();
    let mut cmd = sandbox::command(program, args.iter().copied(), policy)?;
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    if let Some(path) = sandbox_command_path() {
        cmd.env("PATH", path);
    }
    strip_secret_env(&mut cmd);
    let status_channel = sandbox::status::attach(&mut cmd)
        .map_err(|error| format!("sandbox status channel: {error}"))?;
    let capture = handoff::armed(handoff::take_pending(), || {
        output_timed_extensible_cancellable_with_progress(cmd, tool_timeout(), cancel, progress)
    })?;
    let receipt = status_channel.receive();
    set_sandbox_receipt(receipt);
    if let Some(handed) = capture.handed_off {
        return Ok(ExecObservation {
            output: handed.receipt(),
            exit: None,
            timed_out: false,
            cancelled: false,
            dur_ms: started.elapsed().as_millis(),
            kill: None,
            handed_off: Some(handed.id),
        });
    }
    let out = capture.output;
    let timed_out = capture.timed_out;
    let cancelled = capture.cancelled;
    let exit = out.status.code();
    use std::os::unix::process::ExitStatusExt;
    let signal = out.status.signal();

    let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr);
    let cleaned_stderr = sandbox::strip_launcher_stderr(&stderr);
    if !cleaned_stderr.trim().is_empty() {
        combined.push_str("\n[stderr] ");
        combined.push_str(cleaned_stderr.trim());
    }
    #[cfg(target_os = "linux")]
    if !out.status.success() && stderr.contains("bwrap:") {
        let cause = sandbox::compatibility::detect()
            .unwrap_or(sandbox::compatibility::Cause::NamespaceDenied);
        combined.push_str(&format!("\n[doctor hint: {}]", cause.remedy()));
    }
    if let Some(signal) = signal {
        combined.push_str(&format!(
            "\n[{} reason=signal:{} signal={signal}]",
            super::book::d467_receipts::WORKER_KILLED.cells(),
            signal_name(signal)
        ));
    }
    if capture.tool_idle {
        combined.push_str("\n[escalation: tool_idle; silent tool child tree killed]\n");
        combined.push_str(&super::book::p_processes::TOOL_IDLE.cells());
    }
    if capture.grandchild_holds_stdout {
        combined.push_str(&format!(
            "\n[{}]",
            super::book::d467_receipts::STDOUT_HELD.cells()
        ));
    }
    if capture.grandchild_holds_stderr {
        combined.push_str(&format!(
            "\n[{}]",
            super::book::d467_receipts::STDERR_HELD.cells()
        ));
    }
    if timed_out && !capture.tool_idle {
        let elapsed_secs = started.elapsed().as_secs();
        let budget_secs = tool_timeout().map(|d| d.as_secs()).unwrap_or(elapsed_secs);
        let note_secs = if budget_secs > 0 && elapsed_secs.saturating_add(1) < budget_secs {
            elapsed_secs.max(1)
        } else if budget_secs > 0 {
            budget_secs
        } else {
            elapsed_secs.max(1)
        };
        combined.push_str(&timeout_note(note_secs, capture.timeout_diag.as_ref()));
    }
    if capture.deadline_extended {
        combined.push_str(
            "\n[⏱ deadline extended in flight — ANGEL_TOOL_TIMEOUT was raised while the job ran]",
        );
    }
    if cancelled {
        combined.push_str("\n[cancelled — process group killed]");
    }
    combined = match bound {
        OutputBound::Excerpt => cap_text_owned(combined, EXCERPT_BYTES, 0),
        OutputBound::ToolCeiling => cap_tool_output_owned(combined, None),
    };
    Ok(ExecObservation {
        output: combined.trim().to_string(),
        exit,
        timed_out,
        cancelled,
        dur_ms: started.elapsed().as_millis(),
        kill: (signal.is_some() || cancelled || timed_out).then(|| {
            crate::agent::sandbox::process_owner::KillReceipt::new(
                signal,
                if cancelled {
                    "cancelled"
                } else if capture.tool_idle {
                    "tool_idle"
                } else if timed_out {
                    "deadline"
                } else {
                    "signal_death"
                },
                if cancelled || timed_out {
                    "harness"
                } else {
                    "unknown_external"
                },
            )
        }),
        handed_off: None,
    })
}

pub(crate) fn run_sandboxed(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    policy: &SandboxPolicy,
) -> Result<String, String> {
    run_sandboxed_observed(program, args, cwd, policy).map(|o| o.output)
}

pub(crate) fn strip_secret_env(cmd: &mut Command) {
    if !crate::platform::yolo::enabled() && env_flag("ANGEL_TOOL_STRIP_SECRETS", true) {
        for (name, _) in std::env::vars_os() {
            if crate::knowledge::experience::is_secret_name(&name.to_string_lossy()) {
                cmd.env_remove(name);
            }
        }
    }
}

pub(crate) fn sandboxed_workspace_sh(
    script: &str,
    cwd: &Path,
    workspace: &Path,
) -> Result<Command, String> {
    let mut policy = SandboxPolicy::permissive();
    policy
        .writable_roots
        .extend(SandboxPolicy::git_worktree_roots(workspace));
    policy.writable_roots.push(workspace.to_path_buf());
    let mut cmd = sandbox::command("sh", ["-c", script], &policy)?;
    cmd.current_dir(cwd);
    if let Some(path) = sandbox_command_path() {
        cmd.env("PATH", path);
    }
    strip_secret_env(&mut cmd);
    Ok(cmd)
}

pub(crate) fn sandbox_command_path() -> Option<OsString> {
    let current = std::env::var_os("PATH")?;
    let path = prefer_user_bins(std::env::var_os("HOME").map(PathBuf::from), current);
    match crate::platform::workspace_lang::runtime_shims_dir() {
        Some(shims) => {
            let mut paths: Vec<PathBuf> = std::env::split_paths(&path).collect();
            if !paths.iter().any(|p| p == &shims) {
                paths.push(shims);
            }
            Some(std::env::join_paths(paths).unwrap_or(path))
        }
        None => Some(path),
    }
}

pub(crate) fn prefer_user_bins(home: Option<PathBuf>, current: OsString) -> OsString {
    let Some(home) = home else {
        return current;
    };
    let preferred = [home.join(".local/bin"), home.join("bin")];
    let installs = home.join(".local/share/mise/installs");
    let mut paths: Vec<PathBuf> = std::env::split_paths(&current)
        .filter(|p| p.starts_with(&installs) && p.is_dir())
        .collect();
    for p in preferred.iter().filter(|p| p.is_dir()) {
        if !paths.contains(p) {
            paths.push(p.clone());
        }
    }
    for p in std::env::split_paths(&current) {
        if !paths.iter().any(|existing| existing == &p) {
            paths.push(p);
        }
    }
    std::env::join_paths(paths).unwrap_or(current)
}

pub(crate) fn truncate_to_char_boundary(s: &mut String, max_bytes: usize) {
    let mut end = max_bytes.min(s.len());
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s.truncate(end);
}

pub(crate) fn tool_timeout() -> Option<Duration> {
    if crate::platform::yolo::enabled() {
        return call_budget();
    }
    within_call_budget(
        match std::env::var("ANGEL_TOOL_TIMEOUT")
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
        {
            Some(0) | None => None,
            Some(n) => Some(Duration::from_secs(n)),
        },
    )
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct ProcessActivity {
    parent: u32,
    state: char,
    cpu: u64,
    started: u64,
}

#[cfg(target_os = "linux")]
fn proc_activity(pid: u32) -> Option<ProcessActivity> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rest = stat.rsplit_once(')')?.1;
    let mut fields = rest.split_whitespace();
    let state = fields.next()?.chars().next()?;
    let parent = fields.next()?.parse().ok()?;
    let utime = fields.nth(9)?.parse::<u64>().ok()?;
    let stime = fields.next()?.parse::<u64>().ok()?;
    let started = fields.nth(6)?.parse().ok()?;
    Some(ProcessActivity {
        parent,
        state,
        cpu: utime.saturating_add(stime),
        started,
    })
}

#[cfg(target_os = "linux")]
fn owned_process_activity(leader: u32) -> Vec<(u32, ProcessActivity)> {
    let Some(root) = proc_activity(leader) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut children: std::collections::HashMap<u32, Vec<_>> = Default::default();
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        if pid != leader
            && let Some(row) = proc_activity(pid)
        {
            children.entry(row.parent).or_default().push((pid, row));
        }
    }
    if proc_activity(leader).is_none_or(|now| now.started != root.started) {
        return Vec::new();
    }
    let mut pending = vec![(leader, root)];
    let mut owned = Vec::new();
    while let Some((pid, row)) = pending.pop() {
        if let Some(descendants) = children.remove(&pid) {
            pending.extend(
                descendants
                    .into_iter()
                    .filter(|(_, child)| child.started >= row.started),
            );
        }
        owned.push((pid, row));
    }
    owned
}

#[cfg(target_os = "macos")]
fn proc_identity(pid: u32) -> Option<ProcessActivity> {
    use std::mem::MaybeUninit;

    let pid_arg = i32::try_from(pid).ok()?;
    let mut bsd = MaybeUninit::<libc::proc_bsdinfo>::uninit();
    let bsd_size = std::mem::size_of::<libc::proc_bsdinfo>() as i32;
    let written = unsafe {
        libc::proc_pidinfo(
            pid_arg,
            libc::PROC_PIDTBSDINFO,
            0,
            bsd.as_mut_ptr().cast::<libc::c_void>(),
            bsd_size,
        )
    };
    if written != bsd_size {
        return None;
    }
    let bsd = unsafe { bsd.assume_init() };
    if bsd.pbi_pid != pid || bsd.pbi_start_tvusec >= 1_000_000 {
        return None;
    }
    let started = bsd
        .pbi_start_tvsec
        .checked_mul(1_000_000)?
        .checked_add(bsd.pbi_start_tvusec)?;
    let state = match bsd.pbi_status {
        libc::SSTOP => 'T',
        libc::SZOMB => 'Z',
        _ => 'S',
    };
    Some(ProcessActivity {
        parent: bsd.pbi_ppid,
        state,
        cpu: 0,
        started,
    })
}

#[cfg(target_os = "macos")]
fn proc_activity(pid: u32) -> Option<ProcessActivity> {
    use std::mem::MaybeUninit;

    let mut activity = proc_identity(pid)?;
    let pid_arg = i32::try_from(pid).ok()?;
    let mut task = MaybeUninit::<libc::proc_taskinfo>::uninit();
    let task_size = std::mem::size_of::<libc::proc_taskinfo>() as i32;
    let written = unsafe {
        libc::proc_pidinfo(
            pid_arg,
            libc::PROC_PIDTASKINFO,
            0,
            task.as_mut_ptr().cast::<libc::c_void>(),
            task_size,
        )
    };
    if written != task_size {
        return None;
    }
    let task = unsafe { task.assume_init() };
    let current = proc_identity(pid)?;
    if current.started != activity.started || current.parent != activity.parent {
        return None;
    }
    if activity.state != 'T' && activity.state != 'Z' && task.pti_numrunning > 0 {
        activity.state = 'R';
    }
    activity.cpu = task.pti_total_user.saturating_add(task.pti_total_system);
    Some(activity)
}

#[cfg(target_os = "macos")]
fn owned_process_activity(leader: u32) -> Vec<(u32, ProcessActivity)> {
    use std::collections::HashMap;

    let Some(root) = proc_identity(leader) else {
        return Vec::new();
    };

    const PROC_ALL_PIDS: u32 = 1;
    let mut pids = vec![0i32; 1024];
    let count = loop {
        let Some(buffer_bytes) = pids
            .len()
            .checked_mul(std::mem::size_of::<i32>())
            .and_then(|bytes| i32::try_from(bytes).ok())
        else {
            return Vec::new();
        };
        let copied = unsafe {
            libc::proc_listpids(
                PROC_ALL_PIDS,
                0,
                pids.as_mut_ptr().cast::<libc::c_void>(),
                buffer_bytes,
            )
        };
        if copied <= 0 {
            return Vec::new();
        }
        if copied < buffer_bytes {
            break (copied as usize) / std::mem::size_of::<i32>();
        }
        let Some(next_len) = pids.len().checked_mul(2) else {
            return Vec::new();
        };
        pids.resize(next_len, 0);
    };

    let mut children: HashMap<u32, Vec<(u32, ProcessActivity)>> = HashMap::new();
    for raw_pid in pids.into_iter().take(count) {
        let Ok(pid) = u32::try_from(raw_pid) else {
            continue;
        };
        if pid != 0
            && let Some(row) = proc_identity(pid)
        {
            children.entry(row.parent).or_default().push((pid, row));
        }
    }

    if proc_identity(leader).is_none_or(|now| now.started != root.started) {
        return Vec::new();
    }
    let mut pending = vec![(leader, root)];
    let mut owned = Vec::new();
    while let Some((pid, row)) = pending.pop() {
        if let Some(descendants) = children.remove(&pid) {
            pending.extend(
                descendants
                    .into_iter()
                    .filter(|(_, child)| child.started >= row.started),
            );
        }
        owned.push((pid, row));
    }

    owned
        .into_iter()
        .filter_map(|(pid, row)| {
            proc_activity(pid)
                .filter(|current| current.started == row.started)
                .map(|current| (pid, current))
        })
        .collect()
}

fn sandbox_helper_process(pid: u32) -> bool {
    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default();
    if comm.trim() == "angel-sandbox" {
        return true;
    }
    comm.trim() == "angel"
        && std::fs::read(format!("/proc/{pid}/cmdline")).is_ok_and(|cmdline| {
            cmdline
                .split(|byte| *byte == 0)
                .any(|arg| arg == b"--sandbox-exec")
        })
}

fn sandbox_helper_still_setting_up(pid: u32) -> bool {
    if !sandbox_helper_process(pid) {
        return false;
    }
    let children =
        std::fs::read_to_string(format!("/proc/{pid}/task/{pid}/children")).unwrap_or_default();
    !children
        .split_whitespace()
        .filter_map(|pid| pid.parse::<u32>().ok())
        .any(|child| !sandbox_helper_process(child))
}

pub(crate) fn kill_process_group(pid: u32) {
    unsafe {
        libc::killpg(pid as libc::pid_t, libc::SIGKILL);
    }
}

fn extension_decision(
    old: Option<Duration>,
    live: Option<Option<Duration>>,
) -> Option<Option<Duration>> {
    match live {
        Some(Some(live_budget)) => match old {
            Some(budget) if live_budget > budget => Some(Some(live_budget)),
            _ => None,
        },
        Some(None) => Some(None),
        None => None,
    }
}

pub(crate) fn tool_kill_grace_ms() -> u64 {
    std::env::var("ANGEL_TOOL_KILL_GRACE_MS")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0)
}

#[derive(Debug, Clone)]
pub(crate) struct TimeoutDiagnostics {
    pub(crate) child_state: Option<char>,
    pub(crate) live_descendants: usize,
    pub(crate) last_output_age_secs: Option<u64>,
    pub(crate) grace_ms: u64,
    pub(crate) exited_in_grace: bool,
}

impl TimeoutDiagnostics {
    pub(crate) fn summary(&self) -> String {
        let age = match self.last_output_age_secs {
            Some(secs) => format!("no output for {secs}s"),
            None => "no output ever".to_string(),
        };
        let state = match self.child_state {
            Some(state) => format!("child state {state}"),
            None => "child already gone".to_string(),
        };
        let mut text = format!(
            "{age}, {state}, {} live descendant{}",
            self.live_descendants,
            if self.live_descendants == 1 { "" } else { "s" }
        );
        if self.grace_ms > 0 {
            if self.exited_in_grace {
                text.push_str(&format!(
                    ", exited on SIGTERM within {}ms grace",
                    self.grace_ms
                ));
            } else {
                text.push_str(&format!(", SIGTERM grace {}ms ignored", self.grace_ms));
            }
        }
        text
    }
}

pub(crate) fn timeout_note(timeout_secs: u64, diag: Option<&TimeoutDiagnostics>) -> String {
    let page = crate::agent::harness::book::d46_recovery::TIMEOUT_KNOB.cells();
    match diag {
        Some(diag) => format!(
            "\n[timed out after {timeout_secs}s — {}; process killed]\n{page}",
            diag.summary()
        ),
        None => format!("\n[timed out after {timeout_secs}s — process killed]\n{page}"),
    }
}

#[cfg(target_os = "linux")]
fn proc_stat_state(pid: u32) -> Option<char> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    stat.rsplit_once(')')?
        .1
        .split_whitespace()
        .next()?
        .chars()
        .next()
}

#[cfg(target_os = "macos")]
fn proc_stat_state(pid: u32) -> Option<char> {
    use std::mem::MaybeUninit;
    let mut bsdinfo = MaybeUninit::<libc::proc_bsdinfo>::uninit();
    let bsdsize = std::mem::size_of::<libc::proc_bsdinfo>() as i32;
    let ret = unsafe {
        libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDTBSDINFO,
            0,
            bsdinfo.as_mut_ptr() as *mut libc::c_void,
            bsdsize,
        )
    };
    if ret < bsdsize {
        return None;
    }
    let bsdinfo = unsafe { bsdinfo.assume_init() };
    if bsdinfo.pbi_status == 3 {
        return Some('T');
    }
    if bsdinfo.pbi_status == 4 {
        return Some('Z');
    }

    let mut taskinfo = MaybeUninit::<libc::proc_taskinfo>::uninit();
    let tasksize = std::mem::size_of::<libc::proc_taskinfo>() as i32;
    let ret2 = unsafe {
        libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDTASKINFO,
            0,
            taskinfo.as_mut_ptr() as *mut libc::c_void,
            tasksize,
        )
    };
    if ret2 >= tasksize {
        let taskinfo = unsafe { taskinfo.assume_init() };
        if taskinfo.pti_numrunning > 0 {
            Some('R')
        } else {
            Some('S')
        }
    } else {
        Some('S')
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn proc_stat_state(_pid: u32) -> Option<char> {
    None
}

#[cfg(target_os = "linux")]
fn live_group_descendants(leader: u32) -> usize {
    owned_process_activity(leader)
        .iter()
        .filter(|(pid, row)| *pid != leader && !matches!(row.state, 'Z' | 'X'))
        .count()
}

#[cfg(target_os = "macos")]
fn live_group_descendants(leader: u32) -> usize {
    const PROC_PGRP_ONLY: u32 = 2;
    let mut pids = [0i32; 1024];
    let buf_size = (pids.len() * std::mem::size_of::<i32>()) as i32;
    let bytes = unsafe {
        libc::proc_listpids(
            PROC_PGRP_ONLY,
            leader,
            pids.as_mut_ptr() as *mut libc::c_void,
            buf_size,
        )
    };
    if bytes > 0 {
        let count = (bytes as usize) / std::mem::size_of::<i32>();
        pids[..count]
            .iter()
            .filter(|&&pid| pid > 0 && pid as u32 != leader)
            .count()
    } else {
        0
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn live_group_descendants(_leader: u32) -> usize {
    0
}

fn kill_group_on_timeout(
    pid: u32,
    rx_status: &std::sync::mpsc::Receiver<std::io::Result<std::process::ExitStatus>>,
    output_epoch: Instant,
    last_output_ms: &AtomicU64,
    supervised: bool,
) -> (
    Option<std::io::Result<std::process::ExitStatus>>,
    TimeoutDiagnostics,
) {
    let stamp = last_output_ms.load(Ordering::Acquire);
    let mut diag = TimeoutDiagnostics {
        child_state: proc_stat_state(pid),
        live_descendants: live_group_descendants(pid),
        last_output_age_secs: (stamp != u64::MAX)
            .then(|| (output_epoch.elapsed().as_millis() as u64).saturating_sub(stamp) / 1000),
        grace_ms: tool_kill_grace_ms().max(if supervised { 500 } else { 0 }),
        exited_in_grace: false,
    };
    let mut reaped = None;
    unsafe {
        libc::killpg(pid as libc::pid_t, libc::SIGTERM);
    }
    if diag.grace_ms > 0
        && let Ok(status) = rx_status.recv_timeout(Duration::from_millis(diag.grace_ms))
    {
        diag.exited_in_grace = true;
        reaped = Some(status);
    }
    kill_process_group(pid);
    if reaped.is_none() {
        reaped = rx_status.recv_timeout(Duration::from_secs(5)).ok();
    }
    (reaped, diag)
}

pub(crate) fn output_timed(
    cmd: Command,
    timeout: Option<Duration>,
) -> Result<(std::process::Output, bool), String> {
    let captured = output_timed_captured(cmd, timeout)?;
    Ok((captured.output, captured.timed_out))
}

#[derive(Debug)]
pub(crate) struct TimedCapture {
    pub output: std::process::Output,
    pub timed_out: bool,
    pub cancelled: bool,
    pub stdout_total_bytes: u64,
    pub stderr_total_bytes: u64,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub timeout_diag: Option<TimeoutDiagnostics>,
    pub tool_idle: bool,
    pub grandchild_holds_stdout: bool,
    pub grandchild_holds_stderr: bool,
    pub deadline_extended: bool,
    pub handed_off: Option<handoff::HandedOff>,
}

impl TimedCapture {
    pub(crate) fn output_complete(&self) -> bool {
        !self.stdout_truncated
            && !self.stderr_truncated
            && !self.grandchild_holds_stdout
            && !self.grandchild_holds_stderr
    }
}

pub(crate) fn output_timed_captured(
    cmd: Command,
    timeout: Option<Duration>,
) -> Result<TimedCapture, String> {
    output_timed_captured_cancellable(cmd, timeout, None)
}

pub(crate) fn output_timed_captured_cancellable(
    cmd: Command,
    timeout: Option<Duration>,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<TimedCapture, String> {
    output_timed_captured_cancellable_with_progress(cmd, timeout, cancel, None)
}

pub(crate) fn output_timed_captured_cancellable_with_progress(
    cmd: Command,
    timeout: Option<Duration>,
    cancel: Option<&std::sync::atomic::AtomicBool>,
    progress: Option<Arc<ToolOutputProgress>>,
) -> Result<TimedCapture, String> {
    output_timed_inner(cmd, timeout, cancel, progress, false, false)
}

pub(crate) fn output_timed_fixed_captured(
    cmd: Command,
    timeout: Duration,
) -> Result<TimedCapture, String> {
    output_timed_inner(cmd, Some(timeout), None, None, false, true)
}

pub(crate) fn output_timed_fixed_captured_cancellable(
    cmd: Command,
    timeout: Duration,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<TimedCapture, String> {
    output_timed_inner(cmd, Some(timeout), Some(cancel), None, false, true)
}

pub(crate) fn output_timed_extensible_cancellable_with_progress(
    cmd: Command,
    timeout: Option<Duration>,
    cancel: Option<&std::sync::atomic::AtomicBool>,
    progress: Option<Arc<ToolOutputProgress>>,
) -> Result<TimedCapture, String> {
    output_timed_inner(cmd, timeout, cancel, progress, true, false)
}

fn output_timed_inner(
    mut cmd: Command,
    timeout: Option<Duration>,
    cancel: Option<&std::sync::atomic::AtomicBool>,
    progress: Option<Arc<ToolOutputProgress>>,
    extensible: bool,
    fixed_timeout: bool,
) -> Result<TimedCapture, String> {
    let fixed_started = fixed_timeout.then(Instant::now);
    let mut handoff_spec = handoff::take_armed();
    use std::io::Read;
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    use std::sync::mpsc::channel;
    let timeout = if crate::platform::yolo::enabled() && !fixed_timeout {
        call_budget()
    } else {
        timeout
    };
    let fixed_remaining = || {
        fixed_started
            .zip(timeout)
            .map(|(started, budget)| budget.saturating_sub(started.elapsed()))
    };
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.process_group(0);
    if let Some(dir) = cmd.get_current_dir()
        && !dir.exists()
    {
        return Err(format!(
            "working directory {} no longer exists (removed under this session?)\n{}",
            dir.display(),
            crate::agent::harness::book::d46_recovery::LIVE_DIRECTORY.cells()
        ));
    }
    if cancel.is_some_and(|cancel| cancel.load(std::sync::atomic::Ordering::Acquire)) {
        return Err("execution cancelled before spawn".to_string());
    }
    let is_transient = |e: &std::io::Error| -> bool {
        if e.raw_os_error() == Some(26) {
            return true;
        }
        #[cfg(target_os = "linux")]
        if e.kind() == std::io::ErrorKind::ExecutableFileBusy {
            return true;
        }
        if e.kind() == std::io::ErrorKind::NotFound {
            return true;
        }
        false
    };
    let supervised = cmd
        .get_args()
        .next()
        .is_some_and(|arg| arg == "--sandbox-exec");
    if fixed_remaining().is_some_and(|remaining| remaining.is_zero()) {
        return Err("spawn failed: fixed execution deadline expired".into());
    }
    let mut child = match cmd.spawn_owned() {
        Ok(child) => child,
        Err(e) if is_transient(&e) => {
            let deadline = Instant::now() + Duration::from_millis(5000);
            let mut last_err = e;
            loop {
                if cancel.is_some_and(|c| c.load(std::sync::atomic::Ordering::Acquire)) {
                    return Err("execution cancelled before spawn".to_string());
                }
                if Instant::now() >= deadline {
                    return Err(format!("spawn failed: {last_err}"));
                }
                let pause = fixed_remaining().map_or(Duration::from_millis(50), |remaining| {
                    remaining.min(Duration::from_millis(50))
                });
                if pause.is_zero() {
                    return Err(format!("spawn failed: {last_err}"));
                }
                std::thread::sleep(pause);
                if fixed_remaining().is_some_and(|remaining| remaining.is_zero()) {
                    return Err(format!("spawn failed: {last_err}"));
                }
                match cmd.spawn_owned() {
                    Ok(child) => break child,
                    Err(err) if is_transient(&err) => {
                        last_err = err;
                    }
                    Err(err) => return Err(format!("spawn failed: {err}")),
                }
            }
        }
        Err(e) => return Err(format!("spawn failed: {e}")),
    };
    let pid = child.id();
    let mut activity = activity::ChildActivity::new(pid, cancel);
    let so = child.stdout.take().unwrap();
    let se = child.stderr.take().unwrap();
    let (tx_out, rx_out) = channel();
    let (tx_err, rx_err) = channel();
    const OUTPUT_KEEP_BYTES: usize = 1 << 20;
    const OUTPUT_HEAD_BYTES: usize = 64 * 1024;
    const OUTPUT_TAIL_BYTES: usize = OUTPUT_KEEP_BYTES - OUTPUT_HEAD_BYTES;
    #[allow(clippy::too_many_arguments)]
    fn read_capped(
        mut r: impl Read + std::os::fd::AsRawFd,
        cap: usize,
        stream: ProcessStream,
        progress: Option<Arc<ToolOutputProgress>>,
        output_epoch: Instant,
        last_output_ms: Arc<AtomicU64>,
        stop: Arc<AtomicBool>,
        tap: Option<Arc<handoff::Tap>>,
    ) -> (Vec<u8>, u64) {
        use std::collections::VecDeque;

        let head_cap = OUTPUT_HEAD_BYTES.min(cap);
        let tail_cap = cap.saturating_sub(head_cap).min(OUTPUT_TAIL_BYTES);
        let mut head = Vec::with_capacity(head_cap);
        let mut tail = VecDeque::with_capacity(tail_cap);
        let mut total = 0usize;
        let mut chunk = [0u8; 8192];
        loop {
            if stop.load(Ordering::Acquire) {
                break;
            }
            let mut descriptor = libc::pollfd {
                fd: r.as_raw_fd(),
                events: libc::POLLIN | libc::POLLHUP | libc::POLLERR,
                revents: 0,
            };
            let ready = unsafe { libc::poll(&mut descriptor, 1, 50) };
            if ready == 0 {
                continue;
            }
            if ready < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                break;
            }
            match r.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if let Some(progress) = progress.as_ref() {
                        progress(stream, &chunk[..n]);
                    }
                    if let Some(tap) = tap.as_ref() {
                        tap.feed(&chunk[..n]);
                    }
                    last_output_ms
                        .store(output_epoch.elapsed().as_millis() as u64, Ordering::Release);
                    total = total.saturating_add(n);
                    let mut rest = &chunk[..n];
                    if head.len() < head_cap {
                        let take = rest.len().min(head_cap - head.len());
                        head.extend_from_slice(&rest[..take]);
                        rest = &rest[take..];
                    }
                    if tail_cap > 0 && !rest.is_empty() {
                        tail.extend(rest.iter().copied());
                        let overflow = tail.len().saturating_sub(tail_cap);
                        if overflow > 0 {
                            tail.drain(..overflow);
                        }
                    }
                }
            }
        }
        if total <= cap {
            head.extend(tail);
            return (head, total as u64);
        }
        let omitted = total.saturating_sub(head.len()).saturating_sub(tail.len());
        head.extend_from_slice(format!("\n…[{omitted} output bytes omitted]…\n").as_bytes());
        head.extend(tail);
        (head, total as u64)
    }
    let output_epoch = Instant::now();
    let last_output_ms = Arc::new(AtomicU64::new(u64::MAX));
    let stop_readers = Arc::new(AtomicBool::new(false));
    let tap = handoff_spec
        .as_ref()
        .map(|_| Arc::new(handoff::Tap::default()));
    let stdout_tap = tap.clone();
    let stderr_tap = tap.clone();
    let stdout_progress = progress.clone();
    let stdout_stamp = Arc::clone(&last_output_ms);
    let stdout_stop = Arc::clone(&stop_readers);
    std::thread::spawn(move || {
        let _ = tx_out.send(read_capped(
            so,
            OUTPUT_KEEP_BYTES,
            ProcessStream::Stdout,
            stdout_progress,
            output_epoch,
            stdout_stamp,
            stdout_stop,
            stdout_tap,
        ));
    });
    let stderr_stamp = Arc::clone(&last_output_ms);
    let stderr_stop = Arc::clone(&stop_readers);
    std::thread::spawn(move || {
        let _ = tx_err.send(read_capped(
            se,
            OUTPUT_KEEP_BYTES,
            ProcessStream::Stderr,
            progress,
            output_epoch,
            stderr_stamp,
            stderr_stop,
            stderr_tap,
        ));
    });
    let (tx_status, mut rx_status) = channel();
    std::thread::spawn(move || {
        let _ = tx_status.send(child.wait());
    });
    let mut timed_out = false;
    let mut cancelled = false;
    let mut timeout_diag = None;
    let mut deadline_extended = false;
    let mut deadline = timeout;
    let spawn_started = Instant::now();
    let mut wall_started = None;
    let wait_status =
        |rx: &std::sync::mpsc::Receiver<std::io::Result<std::process::ExitStatus>>,
         cap: Duration|
         -> Option<std::io::Result<std::process::ExitStatus>> { rx.recv_timeout(cap).ok() };
    let mut tool_idle = false;
    let idle_limit = tool_idle_timeout();
    let mut wait_started = fixed_started.unwrap_or_else(Instant::now);
    let status = loop {
        match rx_status.try_recv() {
            Ok(status) => break status.map_err(|e| format!("wait failed: {e}"))?,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                return Err("wait thread died".into());
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
        if cancel.is_some_and(|c| c.load(Ordering::Acquire)) {
            unsafe {
                libc::killpg(pid as libc::pid_t, libc::SIGTERM);
            }
            let reaped = wait_status(&rx_status, Duration::from_millis(500));
            kill_process_group(pid);
            cancelled = true;
            break reaped
                .or_else(|| wait_status(&rx_status, Duration::from_secs(5)))
                .ok_or_else(|| "child did not reap after cancellation".to_string())?
                .map_err(|e| format!("wait failed: {e}"))?;
        }
        let setting_up = wall_started.is_none()
            && spawn_started.elapsed() < Duration::from_secs(60)
            && sandbox_helper_still_setting_up(pid);
        if wall_started.is_none() && !setting_up {
            wall_started = Some(Instant::now());
        }
        let stamp = last_output_ms.load(Ordering::Acquire);
        let silent = if stamp == u64::MAX {
            wall_started.map_or(Duration::ZERO, |wall| wall.elapsed())
        } else {
            output_epoch
                .elapsed()
                .saturating_sub(Duration::from_millis(stamp))
        };
        let activity_age = activity
            .observe((stamp != u64::MAX).then(|| output_epoch + Duration::from_millis(stamp)));
        let active = activity_age.is_some_and(|age| idle_limit.is_some_and(|limit| age < limit));
        tool_idle = idle_limit.is_some_and(|limit| silent >= limit) && !active && !setting_up;
        let mut deadline_due = deadline.is_some_and(|budget| wait_started.elapsed() >= budget);
        if deadline_due
            && let Some(adopted) = extension_decision(deadline, extensible.then(tool_timeout))
        {
            deadline = adopted;
            deadline_extended = true;
            wait_started = Instant::now();
            deadline_due = false;
        }
        let release =
            cancel.and_then(|c| release::take_release(c as *const _ as usize, spawn_started));
        let force_handoff = release == Some(release::Release::HandOff)
            && handoff_spec.is_some()
            && tap.is_some()
            && wall_started.is_some();
        let mut watchdog_stop = release.is_some() && !force_handoff;
        if let (Some(spec), Some(tap), Some(wall)) =
            (handoff_spec.as_ref(), tap.as_ref(), wall_started)
            && (wall.elapsed() >= spec.after || force_handoff)
        {
            let owner = cancel.map_or(0, |c| c as *const _ as usize);
            let stamp = last_output_ms.load(Ordering::Acquire);
            let silent = (stamp != u64::MAX).then(|| {
                output_epoch
                    .elapsed()
                    .saturating_sub(Duration::from_millis(stamp))
            });
            let mut status_back = None;
            let filed = tap.hand_off(|recent| {
                crate::agent::tools::proc::hand_off(
                    &spec.workspace,
                    &spec.command,
                    pid,
                    rx_status,
                    owner,
                    recent,
                )
                .map(|job| ((job.id, job.log), job.sink))
                .map_err(|(error, rx)| {
                    status_back = Some(rx);
                    error
                })
            });
            match filed {
                Ok(((id, _log), recent)) => {
                    if force_handoff {
                        release::note_released(release::Release::HandOff);
                    }
                    let handed = handoff::HandedOff::new(
                        id,
                        pid,
                        &spec.command,
                        wall.elapsed(),
                        silent,
                        &recent,
                    );
                    return Ok(TimedCapture {
                        output: std::process::Output {
                            status: std::os::unix::process::ExitStatusExt::from_raw(0),
                            stdout: Vec::new(),
                            stderr: Vec::new(),
                        },
                        timed_out: false,
                        cancelled: false,
                        stdout_total_bytes: 0,
                        stderr_total_bytes: 0,
                        stdout_truncated: false,
                        stderr_truncated: false,
                        timeout_diag: None,
                        tool_idle: false,
                        grandchild_holds_stdout: false,
                        grandchild_holds_stderr: false,
                        deadline_extended,
                        handed_off: Some(handed),
                    });
                }
                Err(_) => {
                    rx_status = status_back.expect("a failed hand-off returns the exit channel");
                    handoff_spec = None;
                    watchdog_stop |= force_handoff;
                }
            }
        }
        if tool_idle || deadline_due || watchdog_stop {
            if watchdog_stop {
                release::note_released(release::Release::Stop);
            } else if tool_idle {
                TOOL_IDLE_ESCALATED.with(|flag| flag.set(true));
            }
            let (reaped, diag) =
                kill_group_on_timeout(pid, &rx_status, output_epoch, &last_output_ms, supervised);
            timed_out = true;
            timeout_diag = Some(diag);
            break reaped
                .ok_or_else(|| "child did not reap after group kill".to_string())?
                .map_err(|e| format!("wait failed: {e}"))?;
        }
        let wait = fixed_remaining().map_or(Duration::from_millis(20), |remaining| {
            remaining.min(Duration::from_millis(20))
        });
        if let Some(status) = wait_status(&rx_status, wait) {
            break status.map_err(|e| format!("wait failed: {e}"))?;
        }
    };
    if !crate::platform::yolo::enabled() {
        kill_process_group(pid);
    }
    let drain_deadline = Instant::now() + Duration::from_secs(2);
    let mut stdout_capture = rx_out
        .recv_timeout(drain_deadline.saturating_duration_since(Instant::now()))
        .ok();
    let mut stderr_capture = rx_err
        .recv_timeout(drain_deadline.saturating_duration_since(Instant::now()))
        .ok();
    let grandchild_holds_stdout = stdout_capture.is_none();
    let grandchild_holds_stderr = stderr_capture.is_none();
    stop_readers.store(true, Ordering::Release);
    let stop_deadline = Instant::now() + Duration::from_millis(250);
    if stdout_capture.is_none() {
        stdout_capture = rx_out
            .recv_timeout(stop_deadline.saturating_duration_since(Instant::now()))
            .ok();
    }
    if stderr_capture.is_none() {
        stderr_capture = rx_err
            .recv_timeout(stop_deadline.saturating_duration_since(Instant::now()))
            .ok();
    }
    let (stdout, stdout_total_bytes) = stdout_capture.unwrap_or_default();
    let (stderr, stderr_total_bytes) = stderr_capture.unwrap_or_default();
    Ok(TimedCapture {
        stdout_truncated: stdout_total_bytes > OUTPUT_KEEP_BYTES as u64,
        stderr_truncated: stderr_total_bytes > OUTPUT_KEEP_BYTES as u64,
        stdout_total_bytes,
        stderr_total_bytes,
        output: std::process::Output {
            status,
            stdout,
            stderr,
        },
        timed_out,
        cancelled,
        timeout_diag,
        tool_idle,
        grandchild_holds_stdout,
        grandchild_holds_stderr,
        deadline_extended,
        handed_off: None,
    })
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/exec__timeout_diag_tests.rs"]
mod timeout_diag_tests;

#[cfg(all(test, target_os = "linux"))]
#[path = "../../../../tests/cockpit/harness/exec__spawn_budget_tests.rs"]
mod spawn_budget_tests;
