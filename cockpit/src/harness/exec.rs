//! Sandboxed process execution with timeouts and process-group kill.

use super::*;
use crate::sandbox::process_owner::OwnedCommandExt;

mod activity;
mod delegate_activity;
pub(crate) use activity::{
    ChildSnapshot, link_child_owner, owned_child_active, owned_child_setting_up,
    owned_child_snapshot,
};
pub(crate) use delegate_activity::{
    DelegateSnapshot, observe_delegate_turn, owned_delegate_snapshot,
};

thread_local! {
    // Trusted execution metadata, never inferred from child-controlled output.
    static TOOL_IDLE_ESCALATED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

thread_local! {
    static SANDBOX_RECEIPT: std::cell::RefCell<Option<serde_json::Value>> = const { std::cell::RefCell::new(None) };
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

// ---------------------------------------------------------------------------
// Utility tools (safe, no exec) — exercise the loop.
// ---------------------------------------------------------------------------

// `present`, `reverse`, and `word_count` live in `crate::tools::utilities`
// (imported at the top of this module).

/// What one sandboxed execution observed beyond its combined output: the raw
/// exit status (`None` when killed by a signal), whether the deadline killed
/// it, and how long it ran. The experience ledger records these; the plain
/// [`run_sandboxed`] wrapper discards them.
pub(crate) struct ExecObservation {
    pub(crate) output: String,
    pub(crate) exit: Option<i32>,
    pub(crate) timed_out: bool,
    pub(crate) cancelled: bool,
    pub(crate) dur_ms: u128,
    pub(crate) kill: Option<crate::sandbox::process_owner::KillReceipt>,
}

/// Run `program args…` (optionally in `cwd`) through the single-threaded
/// sandbox helper, returning the combined output plus what the execution
/// observed (exit status, timeout, duration).
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
    set_sandbox_receipt(None);
    let started = Instant::now();
    let mut cmd = sandbox::command(program, args.iter().copied(), policy)?;
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    if let Some(path) = sandbox_command_path() {
        cmd.env("PATH", path);
    }
    // The Angel process needs its provider credentials; autonomous commands
    // never do. Removal happens at the final process boundary so every shell
    // descendant inherits the credential-free environment while the
    // already-constructed club keeps its in-memory client configuration.
    // On by default since 2026-09-07 (audit S04): a tool child that prints
    // its environment must not put a live key into the trajectory log.
    // `ANGEL_TOOL_STRIP_SECRETS=0` restores pass-through; YOLO bypasses.
    if !crate::yolo::enabled() && env_flag("ANGEL_TOOL_STRIP_SECRETS", true) {
        for (name, _) in std::env::vars_os() {
            if crate::experience::is_secret_name(&name.to_string_lossy()) {
                cmd.env_remove(name);
            }
        }
    }
    let status_channel = sandbox::status::attach(&mut cmd)
        .map_err(|error| format!("sandbox status channel: {error}"))?;
    let capture =
        output_timed_extensible_cancellable_with_progress(cmd, tool_timeout(), cancel, progress)?;
    let receipt = status_channel.receive();
    set_sandbox_receipt(receipt);
    let out = capture.output;
    let timed_out = capture.timed_out;
    let cancelled = capture.cancelled;
    let exit = out.status.code();
    use std::os::unix::process::ExitStatusExt;
    let signal = out.status.signal();

    let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !stderr.trim().is_empty() {
        combined.push_str("\n[stderr] ");
        combined.push_str(stderr.trim());
    }
    #[cfg(target_os = "linux")]
    if !out.status.success() && stderr.contains("bwrap:") {
        // A nested Bubblewrap invocation can fail even when our outer probe
        // succeeds. This is guidance only, never helper-failure attribution.
        let cause = sandbox::compatibility::detect()
            .unwrap_or(sandbox::compatibility::Cause::NamespaceDenied);
        combined.push_str(&format!("\n[doctor hint: {}]", cause.remedy()));
    }
    if let Some(signal) = signal {
        combined.push_str(&format!(
            "\n[worker killed by {}; reason=signal:{}; signal {signal}; verification inconclusive]",
            signal_name(signal),
            signal_name(signal)
        ));
    }
    if capture.tool_idle {
        combined.push_str("\n[escalation: tool_idle; silent tool child tree killed; retry with explicit input or use proc_run]");
    }
    if capture.grandchild_holds_stdout {
        combined.push_str("\n[grandchild_holds_stdout: drain deadline reached; inherited output pipe closed; capture incomplete]");
    }
    if capture.grandchild_holds_stderr {
        combined.push_str("\n[grandchild_holds_stderr: drain deadline reached; inherited error pipe closed; capture incomplete]");
    }
    if timed_out && !capture.tool_idle {
        // Idle-floor kills happen well before ANGEL_TOOL_TIMEOUT; report the
        // wall time that actually fired, not the unused 120s budget.
        let elapsed_secs = started.elapsed().as_secs();
        let budget_secs = tool_timeout().map(|d| d.as_secs()).unwrap_or(elapsed_secs);
        let note_secs = if budget_secs > 0 && elapsed_secs.saturating_add(1) < budget_secs {
            elapsed_secs.max(1)
        } else if budget_secs > 0 {
            budget_secs
        } else {
            elapsed_secs.max(1)
        };
        if capture.killed_by_idle_floor {
            combined.push_str(&idle_floor_note(note_secs, capture.timeout_diag.as_ref()));
        } else {
            combined.push_str(&timeout_note(note_secs, capture.timeout_diag.as_ref()));
        }
    }
    if capture.deadline_extended {
        combined.push_str(
            "\n[⏱ deadline extended in flight — ANGEL_TOOL_TIMEOUT was raised while the job ran]",
        );
    }
    if cancelled {
        combined.push_str("\n[cancelled — process group killed]");
    }
    // The low-level reader already keeps both the beginning and end of each
    // stream. Preserve that property at the smaller model-facing boundary too:
    // compiler/test diagnostics and exit summaries are commonly at the end,
    // and the old first-4K slice silently discarded stderr after noisy stdout.
    const MAX: usize = 4000;
    combined = cap_text_owned(combined, MAX, 0);
    Ok(ExecObservation {
        output: combined.trim().to_string(),
        exit,
        timed_out,
        cancelled,
        dur_ms: started.elapsed().as_millis(),
        kill: (signal.is_some() || cancelled || timed_out).then(|| {
            crate::sandbox::process_owner::KillReceipt::new(
                signal,
                if cancelled {
                    "cancelled"
                } else if capture.tool_idle || capture.killed_by_idle_floor {
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
    })
}

/// [`run_sandboxed_observed`] with the observation dropped — the plain
/// interface for callers that don't record experience (currently exercised by
/// the harness tests; kept for future tools that have no ledger duty).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn run_sandboxed(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    policy: &SandboxPolicy,
) -> Result<String, String> {
    run_sandboxed_observed(program, args, cwd, policy).map(|o| o.output)
}

pub(crate) fn sandbox_command_path() -> Option<OsString> {
    let current = std::env::var_os("PATH")?;
    let path = prefer_user_bins(std::env::var_os("HOME").map(PathBuf::from), current);
    // Runtime shims (`python` → python3 on a python3-only host) go LAST so a
    // real program on PATH always wins; see workspace_lang::runtime_shims_dir.
    match crate::workspace_lang::runtime_shims_dir() {
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
    // Omarchy's ~/.local/bin entries can be auto-install wrappers (mise use -g).
    // Keep already-resolved mise binaries ahead of those wrappers: running an
    // installed tool must not try to mutate configuration inside the sandbox.
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

/// External-command wall ceiling: `ANGEL_TOOL_TIMEOUT` seconds, an operator
/// cap only — unset or `0` = no limit. There is no default ceiling: a tool
/// that is still producing output or burning CPU is doing the work the
/// operator asked for (CUDA builds, benchmark runs, remote validation), and
/// the old 120 s default killed every real one of those the fleet ran
/// (2026-09-05 Yukon verifiers; 2026-09-10 the qwen38 loop had to hide its
/// 104 s builds and 95 s scored runs behind keepalive ticks). A hung tool is
/// the idle floor's job, not a deadline's.
pub(crate) fn tool_timeout() -> Option<Duration> {
    if crate::yolo::enabled() {
        return None;
    }
    match std::env::var("ANGEL_TOOL_TIMEOUT")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
    {
        Some(0) | None => None,
        Some(n) => Some(Duration::from_secs(n)),
    }
}

/// Wall-clock ceiling while a process group is *busy* (runnable or burning
/// CPU): `ANGEL_TOOL_HARD_TIMEOUT`, operator cap only. Idle groups never
/// consult this. Unset or `0` = unlimited while busy (a busy process is work).
pub(crate) fn tool_hard_timeout() -> Option<Duration> {
    match std::env::var("ANGEL_TOOL_HARD_TIMEOUT")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
    {
        Some(0) | None => None,
        Some(n) => Some(Duration::from_secs(n)),
    }
}

/// Optional idle hang floor. Unset or `0` leaves silent work alone: process
/// state cannot distinguish a hung command from a legitimate GPU/remote wait.
/// Operators may opt into the heuristic with `ANGEL_TOOL_IDLE_FLOOR_SECS`.
pub(crate) fn tool_idle_floor() -> Option<Duration> {
    match std::env::var("ANGEL_TOOL_IDLE_FLOOR_SECS")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
    {
        Some(n) if n > 0 => Some(Duration::from_secs(n)),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
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

/// Observe this tool's descendants across setsid/process-group boundaries.
/// The sandbox subreaper retains orphaned payloads under this same root.
/// This is a liveness snapshot, never authority to signal a process.
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
    // A reaped/reused root cannot lend its identity to another worker.
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

#[cfg(target_os = "linux")]
fn process_group_is_runnable(leader: u32) -> bool {
    let supervised = sandbox_helper_process(leader);
    owned_process_activity(leader)
        .iter()
        .any(|(pid, row)| !(supervised && *pid == leader) && matches!(row.state, 'R' | 'D'))
}

#[cfg(target_os = "macos")]
fn process_group_is_runnable(leader: u32) -> bool {
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
        for &pid in &pids[..count] {
            if pid > 0 {
                if let Some(state) = proc_stat_state(pid as u32) {
                    if state == 'R' {
                        return true;
                    }
                }
            }
        }
    }
    if let Some(state) = proc_stat_state(leader) {
        if state == 'R' {
            return true;
        }
    }
    false
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn process_group_is_runnable(_leader: u32) -> bool {
    false
}

/// Which idle-sampling deadline a kill just crossed. The kill note names a
/// different knob per reason, so the receipt points at the control the
/// operator actually wants to raise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IdleKillReason {
    /// Silent, non-busy group past `ANGEL_TOOL_IDLE_FLOOR_SECS`.
    IdleFloor,
    /// Busy group past `ANGEL_TOOL_HARD_TIMEOUT`.
    HardTimeout,
}

/// Helper setup is distinct from payload progress. Allow at most 60 seconds
/// for the helper to apply confinement and exec before starting idle sampling.
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
    // The lifecycle supervisor keeps the helper name after its child execs.
    // Its name alone must not exempt a sleeping payload for another minute.
    let children =
        std::fs::read_to_string(format!("/proc/{pid}/task/{pid}/children")).unwrap_or_default();
    !children
        .split_whitespace()
        .filter_map(|pid| pid.parse::<u32>().ok())
        .any(|child| !sandbox_helper_process(child))
}

fn idle_kill_reason(
    pid: u32,
    wall_started: Instant,
    output_epoch: Instant,
    last_output_ms: &AtomicU64,
) -> Option<IdleKillReason> {
    let floor = tool_idle_floor()?;
    // Quiet compiles still show R (runnable) or D (uninterruptible I/O). Those
    // follow ANGEL_TOOL_TIMEOUT / ANGEL_TOOL_HARD_TIMEOUT, not this floor.
    // Isolation in the group scan keeps the cargo-test / cockpit thread from
    // looking like the child.
    // Helper setup (Landlock / bwrap / exec) is silent and can sit in R or S
    // for seconds under load. Do not charge that time to the payload idle floor
    // or hard timeout — otherwise `echo warmup` dies before it runs.
    if sandbox_helper_still_setting_up(pid) {
        // Slow Landlock/bwrap under load is not a silent payload, but a wedged
        // helper must not own the turn forever.
        if output_epoch.elapsed() < Duration::from_secs(60) {
            return None;
        }
    }
    if process_group_is_runnable(pid) {
        return match tool_hard_timeout() {
            Some(hard) if wall_started.elapsed() >= hard => Some(IdleKillReason::HardTimeout),
            _ => None,
        };
    }
    let stamp = last_output_ms.load(Ordering::Acquire);
    let silent = if stamp == u64::MAX {
        wall_started.elapsed()
    } else {
        let silent_ms = (output_epoch.elapsed().as_millis() as u64).saturating_sub(stamp);
        Duration::from_millis(silent_ms)
    };
    (silent >= floor).then_some(IdleKillReason::IdleFloor)
}

/// Next time an idle-floor sample is due. `None` means idle kill is off.
fn idle_check_at(from: Instant) -> Option<Instant> {
    tool_idle_floor().map(|floor| from + floor)
}

/// Best-effort `SIGKILL` to the whole process group led by `pid` (the child is
/// the group leader, so `pgid == pid`). Reaps the child *and any grandchildren it
/// spawned* — e.g. a dev server (`vite`/`npm run dev`) started in the foreground,
/// which inherits our stdout/stderr pipes and would otherwise hold them open
/// forever. `killpg` is the reliable syscall (the `kill` binary's negative-pid
/// syntax is ambiguous across implementations).
pub(crate) fn kill_process_group(pid: u32) {
    // SAFETY: a plain signal syscall from the parent; no shared state.
    unsafe {
        libc::killpg(pid as libc::pid_t, libc::SIGKILL);
    }
}

/// The kill-decision re-read as a pure function: `old` is the budget that
/// just expired, `live` the freshly-read policy. `None` keeps the historical
/// kill; `Some(new)` adopts the new budget (`Some(None)` = unlimited). A
/// strictly-larger live budget is required — unchanged or shorter policies
/// never defer, and an already-unlimited run has no decision to make.
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

/// Optional SIGTERM-first grace before the timeout `SIGKILL`:
/// `ANGEL_TOOL_KILL_GRACE_MS` milliseconds (default 0 = immediate `SIGKILL`,
/// the historical behavior). Lets a well-behaved tool flush and clean up on
/// the way down; no timeout default changes.
pub(crate) fn tool_kill_grace_ms() -> u64 {
    std::env::var("ANGEL_TOOL_KILL_GRACE_MS")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0)
}

/// What the parent could truthfully observe at the instant a timeout kill
/// fired: the child's scheduler state, how many group members were still
/// alive under it, and how long its pipes had been silent. Captured *before*
/// any signal is sent, so the numbers describe the hang, not the kill.
#[derive(Debug, Clone)]
pub(crate) struct TimeoutDiagnostics {
    /// `/proc/<pid>/stat` state (R/S/D/Z/T…); `None` = already gone.
    pub(crate) child_state: Option<char>,
    /// Live processes still in the child's process group, besides the leader.
    pub(crate) live_descendants: usize,
    /// Seconds since the last stdout/stderr chunk; `None` = never wrote a byte.
    pub(crate) last_output_age_secs: Option<u64>,
    /// The SIGTERM grace that was applied (0 = none, straight to SIGKILL).
    pub(crate) grace_ms: u64,
    /// Whether the child exited on SIGTERM inside the grace window.
    pub(crate) exited_in_grace: bool,
}

impl TimeoutDiagnostics {
    /// One evidence clause for the timeout note, e.g.
    /// `no output for 118s, child state S, 2 live descendants`.
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

/// Operator-facing timeout receipt. The historical prefix and the
/// `ANGEL_TOOL_TIMEOUT` pointer are kept byte-for-byte; when kill-time
/// diagnostics exist they ride in the middle so a hang reads as evidence
/// ("what was it doing?") instead of a bare elapsed number.
pub(crate) fn timeout_note(timeout_secs: u64, diag: Option<&TimeoutDiagnostics>) -> String {
    match diag {
        Some(diag) => format!(
            "\n[timed out after {timeout_secs}s — {}; process killed; raise/disable via ANGEL_TOOL_TIMEOUT]",
            diag.summary()
        ),
        None => format!(
            "\n[timed out after {timeout_secs}s — process killed; raise/disable via ANGEL_TOOL_TIMEOUT]"
        ),
    }
}

/// Idle-floor kill receipt: the same evidence format as [`timeout_note`], but
/// the pointer names `ANGEL_TOOL_IDLE_FLOOR_SECS` — the group was reaped for
/// sleeping silently past the floor, not for outliving `ANGEL_TOOL_TIMEOUT` —
/// and names the legitimate silent waits the floor can mistake for a hang,
/// with `proc_run` as the escape hatch for a deliberate long background wait.
fn idle_floor_note(floor_secs: u64, diag: Option<&TimeoutDiagnostics>) -> String {
    const IDLE_SUFFIX: &str = "; process killed; silent sleeping wait reaped by the idle floor; \
         if this was a legitimate wait (remote validation, polling, downloads) \
         raise ANGEL_TOOL_IDLE_FLOOR_SECS or use proc_run";
    match diag {
        Some(diag) => format!(
            "\n[timed out after {floor_secs}s — {}{IDLE_SUFFIX}]",
            diag.summary()
        ),
        None => format!("\n[timed out after {floor_secs}s{IDLE_SUFFIX}]"),
    }
}

/// Single-character scheduler state from `/proc/<pid>/stat`, read at kill
/// time. `comm` may itself contain spaces or `)`, so the fixed fields resume
/// after the LAST `)`.
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

/// Live owned descendants, including payloads in their own sessions on Linux.
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

/// The timeout kill sequence: capture diagnostics while the hang is still
/// live, optionally give the group a SIGTERM grace window
/// (`ANGEL_TOOL_KILL_GRACE_MS`, default 0 = none), then `SIGKILL` the group
/// and wait for the reap. Returns the reaped status (`None` = the waiter
/// never delivered one, the caller's existing error path) plus what was seen.
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
    // SIGTERM is always first, including an explicitly zero-length grace.
    unsafe {
        libc::killpg(pid as libc::pid_t, libc::SIGTERM);
    }
    if diag.grace_ms > 0
        && let Ok(status) = rx_status.recv_timeout(Duration::from_millis(diag.grace_ms))
    {
        diag.exited_in_grace = true;
        reaped = Some(status);
    }
    // Always SIGKILL the group afterwards: it reaps stragglers even when the
    // leader honored SIGTERM, and is the exact historical behavior at grace 0.
    kill_process_group(pid);
    if reaped.is_none() {
        reaped = rx_status.recv_timeout(Duration::from_secs(5)).ok();
    }
    (reaped, diag)
}

/// Run `cmd` to completion, draining stdout/stderr in threads (so a full pipe
/// can't deadlock) and killing it if `timeout` elapses. Returns the captured
/// output plus whether it was killed for timing out.
///
/// The child runs in its **own process group**, and on exit *or* timeout we kill
/// the whole group — not just the direct child. This is the fix for a real
/// 3-hour hang: a command that spawned a foreground daemon (a dev server) which
/// inherited the pipes left `read_to_end` blocked at EOF forever, wedging the
/// agent loop. Killing the group closes those inherited pipes so the reader
/// joins are bounded; a properly-detached daemon (`setsid` → its own group, with
/// redirected output) survives this and never held our pipes. The join is also
/// hard-bounded by a short grace period as a last-resort backstop.
pub(crate) fn output_timed(
    cmd: Command,
    timeout: Option<Duration>,
) -> Result<(std::process::Output, bool), String> {
    let captured = output_timed_captured(cmd, timeout)?;
    Ok((captured.output, captured.timed_out))
}

/// Bounded process output plus the information needed to prove whether either
/// stream was truncated. Evaluators use this richer form so a head/tail capture
/// can never be mistaken for a complete verifier transcript.
#[derive(Debug)]
pub(crate) struct TimedCapture {
    pub output: std::process::Output,
    pub timed_out: bool,
    pub cancelled: bool,
    pub stdout_total_bytes: u64,
    pub stderr_total_bytes: u64,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    /// Kill-time forensics, present exactly when `timed_out` — what the child
    /// was doing (state, descendants, output silence) at the moment the
    /// deadline killed it.
    pub timeout_diag: Option<TimeoutDiagnostics>,
    /// True when the kill was the idle floor reaping a silent, non-busy group
    /// (rather than `ANGEL_TOOL_TIMEOUT` / `ANGEL_TOOL_HARD_TIMEOUT`) — the
    /// receipt then names `ANGEL_TOOL_IDLE_FLOOR_SECS` instead.
    pub killed_by_idle_floor: bool,
    pub tool_idle: bool,
    pub grandchild_holds_stdout: bool,
    pub grandchild_holds_stderr: bool,
    /// True when the kill decision re-read `tool_timeout()` and found a
    /// strictly-later (or unlimited) live policy — the operator extended a
    /// safe long job mid-flight instead of letting the original deadline
    /// kill it.
    pub deadline_extended: bool,
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

/// Run a short internal control-plane probe with a deadline that remains fixed
/// even when YOLO removes operator-workload timeouts. This is intentionally not
/// exposed to ordinary shell/build work: it is for health and repair probes
/// whose subprocesses must never own an agent turn indefinitely.
pub(crate) fn output_timed_fixed_captured(
    cmd: Command,
    timeout: Duration,
) -> Result<TimedCapture, String> {
    output_timed_inner(cmd, Some(timeout), None, None, false, true)
}

/// The operator-extensible variant: only callers whose timeout IS the live
/// `tool_timeout()` policy use this, so the mid-run `ANGEL_TOOL_TIMEOUT` raise
/// defers the kill. Fixed verifier/recon deadlines stay fixed.
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
    use std::io::Read;
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    use std::sync::mpsc::channel;
    // Some internal verifiers and recon tools pass their own fixed deadline
    // rather than `tool_timeout()`. Apply the unrestricted override here at
    // the shared process boundary so none of those call sites can accidentally
    // retain a command-killing timer while YOLO is live.
    let timeout = if crate::yolo::enabled() && !fixed_timeout {
        None
    } else {
        timeout
    };
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // New process group with the child as leader, so `kill -<pid>` reaches the
    // whole subtree (the child must be the group leader for that to work).
    cmd.process_group(0);
    // A deleted working directory makes every spawn fail with a bare ENOENT
    // that reads like a broken shell. Name the real problem instead.
    if let Some(dir) = cmd.get_current_dir()
        && !dir.exists()
    {
        return Err(format!(
            "working directory {} no longer exists (removed under this session?) — \
                 cd to a live directory and retry",
            dir.display()
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
                std::thread::sleep(Duration::from_millis(50));
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
    // Readers deliver over channels so the collect below can be *time-bounded*
    // even in the pathological case where something still holds a pipe open.
    let (tx_out, rx_out) = channel();
    let (tx_err, rx_err) = channel();
    // Capped readers: keep the first OUTPUT_KEEP_BYTES, then drain-and-discard
    // so the pipe never backs up (a blocked pipe would wedge the child) while a
    // runaway printer can no longer buffer hundreds of MB that the tool-result
    // cap throws away anyway.
    const OUTPUT_KEEP_BYTES: usize = 1 << 20;
    const OUTPUT_HEAD_BYTES: usize = 64 * 1024;
    const OUTPUT_TAIL_BYTES: usize = OUTPUT_KEEP_BYTES - OUTPUT_HEAD_BYTES;
    fn read_capped(
        mut r: impl Read + std::os::fd::AsRawFd,
        cap: usize,
        stream: ProcessStream,
        progress: Option<Arc<ToolOutputProgress>>,
        output_epoch: Instant,
        last_output_ms: Arc<AtomicU64>,
        stop: Arc<AtomicBool>,
    ) -> (Vec<u8>, u64) {
        use std::collections::VecDeque;

        // Keep both diagnostics at the beginning and terminal summaries at the
        // end. Cargo/libtest often writes the only useful pass/fail line after
        // megabytes of compiler chatter, so a first-N capture turns a bounded
        // runner into a misleading verifier.
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
            // A detached descendant can escape the owned process group while
            // retaining this pipe. Polling makes the read interruptible by the
            // parent's bounded drain deadline instead of leaking one blocked
            // reader thread for the descendant's lifetime.
            let mut descriptor = libc::pollfd {
                fd: r.as_raw_fd(),
                events: libc::POLLIN | libc::POLLHUP | libc::POLLERR,
                revents: 0,
            };
            // SAFETY: descriptor points to one live pollfd for this thread's
            // owned pipe; poll does not retain the pointer after returning.
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
                    // Stamp "ms since epoch" on every chunk (both streams share
                    // the stamp) so a timeout kill can truthfully report how
                    // long the child had been silent.
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
    // Kill-time forensics: the readers stamp this on every chunk, so a
    // timeout can report last-output age instead of a bare elapsed number.
    // `u64::MAX` = "never wrote a byte".
    let output_epoch = Instant::now();
    let last_output_ms = Arc::new(AtomicU64::new(u64::MAX));
    let stop_readers = Arc::new(AtomicBool::new(false));
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
        ));
    });
    // Exit detection: a blocking `wait` on a helper thread signalling a channel
    // gives exact exit wake-up (`recv_timeout` doubles as the deadline) — the
    // old 50ms try_wait poll added up to 50ms of pure latency to every shell,
    // cargo, and hook invocation. The waiter owns the child; on timeout the
    // group kill reaches the leader, so the waiter unblocks right after.
    let (tx_status, rx_status) = channel();
    std::thread::spawn(move || {
        let _ = tx_status.send(child.wait());
    });
    let mut timed_out = false;
    let mut cancelled = false;
    let mut killed_by_idle_floor = false;
    let mut timeout_diag = None;
    let mut deadline_extended = false;
    // Mutable budget: at the kill decision the live `tool_timeout()` is
    // re-read, and a strictly-larger budget (or `0` = unlimited) defers the
    // kill — the operator can extend a safe long job mid-flight by raising
    // `ANGEL_TOOL_TIMEOUT` instead of losing it to the historical hard kill.
    let mut deadline = timeout;
    let spawn_started = Instant::now();
    let mut wall_started = None;
    let mut next_idle_check = None;
    let wait_status =
        |rx: &std::sync::mpsc::Receiver<std::io::Result<std::process::ExitStatus>>,
         cap: Duration|
         -> Option<std::io::Result<std::process::ExitStatus>> { rx.recv_timeout(cap).ok() };
    let idle_due = |next: &mut Option<Instant>, wall: Instant| -> Option<IdleKillReason> {
        let at = (*next)?;
        if Instant::now() < at {
            return None;
        }
        match idle_kill_reason(pid, wall, output_epoch, &last_output_ms) {
            Some(reason) => Some(reason),
            None => {
                *next = idle_check_at(Instant::now());
                None
            }
        }
    };
    let mut tool_idle = false;
    let idle_limit = tool_idle_timeout();
    let mut wait_started = Instant::now();
    let status = loop {
        // Observe an already-exited child before any kill decision.
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
        // This exemption expires even if the helper never reaches exec. Once
        // the payload is observed, a later command name cannot renew setup grace.
        let setting_up = wall_started.is_none()
            && spawn_started.elapsed() < Duration::from_secs(60)
            && sandbox_helper_still_setting_up(pid);
        if wall_started.is_none() && !setting_up {
            let now = Instant::now();
            wall_started = Some(now);
            next_idle_check = idle_check_at(now);
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
        let activity_limit = match (idle_limit, tool_idle_floor()) {
            (Some(idle), Some(floor)) => Some(idle.min(floor)),
            (idle, floor) => idle.or(floor),
        };
        let active =
            activity_age.is_some_and(|age| activity_limit.is_some_and(|limit| age < limit));
        tool_idle = idle_limit.is_some_and(|limit| silent >= limit) && !active && !setting_up;
        let idle_reason = wall_started
            .and_then(|wall| idle_due(&mut next_idle_check, wall))
            .filter(|reason| *reason != IdleKillReason::IdleFloor || !active);
        let mut deadline_due = deadline.is_some_and(|budget| wait_started.elapsed() >= budget);
        if deadline_due
            && let Some(adopted) = extension_decision(deadline, extensible.then(tool_timeout))
        {
            deadline = adopted;
            deadline_extended = true;
            wait_started = Instant::now();
            deadline_due = false;
        }
        if tool_idle || idle_reason.is_some() || deadline_due {
            // A tool-idle escalation is recoverable by the model; it never
            // cancels the enclosing run. Preserve trusted metadata for the turn.
            if tool_idle || idle_reason == Some(IdleKillReason::IdleFloor) {
                TOOL_IDLE_ESCALATED.with(|flag| flag.set(true));
            }
            let (reaped, diag) =
                kill_group_on_timeout(pid, &rx_status, output_epoch, &last_output_ms, supervised);
            timed_out = true;
            killed_by_idle_floor = idle_reason == Some(IdleKillReason::IdleFloor);
            timeout_diag = Some(diag);
            break reaped
                .ok_or_else(|| "child did not reap after group kill".to_string())?
                .map_err(|e| format!("wait failed: {e}"))?;
        }
        if let Some(status) = wait_status(&rx_status, Duration::from_millis(20)) {
            break status.map_err(|e| format!("wait failed: {e}"))?;
        }
    };
    // The direct child has exited — but a grandchild that inherited the pipes can
    // still hold them open. In the ordinary profile, reap the group so those
    // pipes close and the readers hit EOF. YOLO deliberately leaves descendants
    // alone: `command &` must not be silently killed by harness cleanup.
    if !crate::yolo::enabled() {
        kill_process_group(pid);
    }
    // Bounded collect: use one shared deadline for both readers. If an
    // out-of-group descendant retained either pipe, ask the poll-based reader
    // to settle with the bytes already captured and give that handoff one
    // final short convergence window.
    let drain_deadline = Instant::now() + Duration::from_secs(2);
    let mut stdout_capture = None;
    let mut stderr_capture = None;
    while Instant::now() < drain_deadline && (stdout_capture.is_none() || stderr_capture.is_none())
    {
        if stdout_capture.is_none() {
            stdout_capture = rx_out.try_recv().ok();
        }
        if stderr_capture.is_none() {
            stderr_capture = rx_err.try_recv().ok();
        }
        if stdout_capture.is_none() || stderr_capture.is_none() {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    let grandchild_holds_stdout = stdout_capture.is_none();
    let grandchild_holds_stderr = stderr_capture.is_none();
    stop_readers.store(true, Ordering::Release);
    let stop_deadline = Instant::now() + Duration::from_millis(250);
    while Instant::now() < stop_deadline && (stdout_capture.is_none() || stderr_capture.is_none()) {
        if stdout_capture.is_none() {
            stdout_capture = rx_out.try_recv().ok();
        }
        if stderr_capture.is_none() {
            stderr_capture = rx_err.try_recv().ok();
        }
        if stdout_capture.is_none() || stderr_capture.is_none() {
            std::thread::sleep(Duration::from_millis(10));
        }
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
        killed_by_idle_floor,
        tool_idle,
        grandchild_holds_stdout,
        grandchild_holds_stderr,
        deadline_extended,
    })
}

#[cfg(test)]
#[path = "../../../tests/cockpit/harness/exec__timeout_diag_tests.rs"]
mod timeout_diag_tests;
