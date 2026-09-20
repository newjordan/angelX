//! Headless task CLI parsing and the stable JSON result envelope.

use super::{
    TaskAcceptanceTelemetry, TaskTimingTelemetry, TurnFailure, TurnOutcome, TurnStopReason,
};
use crate::agent::club::{OutputBudgetPolicy, OutputBudgetSource, RouteMetadata};
use crate::agent::tools::work_landing::{self, Mode};
use serde::Serialize;
use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct TaskCliArgs {
    pub(crate) json: bool,
    pub(crate) workspace: Option<PathBuf>,
    /// Named sandbox posture for the run (`sealed`). `None` keeps the
    /// ordinary permissive/mandatory ladder assembled ad hoc by each tool.
    pub(crate) sandbox_profile: Option<String>,
    pub(crate) task_id: Option<String>,
    pub(crate) run_id: Option<String>,
    pub(crate) driver: Option<String>,
    pub(crate) reasoning_effort: Option<String>,
    pub(crate) task_pace: Option<String>,
    pub(crate) max_hops: Option<usize>,
    pub(crate) deadline_secs: Option<usize>,
    pub(crate) tool_profile: Option<String>,
    pub(crate) rollout_capture: Option<String>,
    pub(crate) require_rollout: bool,
    pub(crate) require_rendered_output: bool,
    pub(crate) prompt: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TaskCliError {
    pub(crate) message: String,
    pub(crate) parsed: Box<TaskCliArgs>,
}

impl TaskCliError {
    fn missing(message: &str, parsed: TaskCliArgs) -> Self {
        Self {
            message: message.to_string(),
            parsed: Box::new(parsed),
        }
    }

    fn invalid(message: impl Into<String>, parsed: TaskCliArgs) -> Self {
        Self {
            message: message.into(),
            parsed: Box::new(parsed),
        }
    }
}

impl fmt::Display for TaskCliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// Named sandbox postures a headless task may request. `sealed` is the S02
/// host read/network isolation profile: deny-by-default reads outside the
/// resolved allow-list, netless, mandatory — never widened, not even by YOLO.
pub(crate) fn valid_sandbox_profile(value: &str) -> bool {
    matches!(value, "sealed")
}

/// Parse the arguments following either `--task` or `--task-json`.
///
/// `--task-json --task ...` and `--task --task-json ...` are both accepted so
/// evaluators can treat JSON as a modifier without changing ordinary task mode.
pub(crate) fn parse_task_args(
    first: &str,
    args: impl IntoIterator<Item = String>,
) -> Result<TaskCliArgs, TaskCliError> {
    let mut out = TaskCliArgs {
        json: first == "--task-json",
        ..TaskCliArgs::default()
    };
    let mut args = args.into_iter();
    let mut positional_only = false;
    let mut seen = HashSet::new();
    while let Some(arg) = args.next() {
        if positional_only {
            if out.prompt.is_some() {
                return Err(TaskCliError::invalid(
                    "task prompt must be one quoted argument",
                    out,
                ));
            }
            out.prompt = Some(arg);
            positional_only = false;
            continue;
        }
        match arg.as_str() {
            "--task" if first == "--task-json" => {}
            "--task-json" => out.json = true,
            "--sandbox-profile" => {
                if !seen.insert("--sandbox-profile") {
                    return Err(TaskCliError::invalid(
                        "--sandbox-profile may be supplied only once",
                        out,
                    ));
                }
                let Some(value) = args.next() else {
                    return Err(TaskCliError::missing(
                        "--sandbox-profile requires a value",
                        out,
                    ));
                };
                if !valid_sandbox_profile(&value) {
                    return Err(TaskCliError::invalid(
                        format!("unknown sandbox profile {value:?} (supported: sealed)"),
                        out,
                    ));
                }
                out.sandbox_profile = Some(value);
            }
            "--" => positional_only = true,
            "--workspace" => {
                if !seen.insert("--workspace") {
                    return Err(TaskCliError::invalid(
                        "--workspace may be supplied only once",
                        out,
                    ));
                }
                let Some(value) = args.next() else {
                    return Err(TaskCliError::missing(
                        "--workspace requires a directory",
                        out,
                    ));
                };
                out.workspace = Some(PathBuf::from(value));
            }
            "--task-id" => {
                if !seen.insert("--task-id") {
                    return Err(TaskCliError::invalid(
                        "--task-id may be supplied only once",
                        out,
                    ));
                }
                let Some(value) = args.next() else {
                    return Err(TaskCliError::missing("--task-id requires a value", out));
                };
                if !valid_runner_identity(&value) {
                    return Err(TaskCliError::invalid(
                        "--task-id must be 1-256 portable identity characters",
                        out,
                    ));
                }
                out.task_id = Some(value);
            }
            "--run-id" => {
                if !seen.insert("--run-id") {
                    return Err(TaskCliError::invalid(
                        "--run-id may be supplied only once",
                        out,
                    ));
                }
                let Some(value) = args.next() else {
                    return Err(TaskCliError::missing("--run-id requires a value", out));
                };
                if !valid_runner_identity(&value) {
                    return Err(TaskCliError::invalid(
                        "--run-id must be 1-256 portable identity characters",
                        out,
                    ));
                }
                out.run_id = Some(value);
            }
            "--driver" => {
                if !seen.insert("--driver") {
                    return Err(TaskCliError::invalid(
                        "--driver may be supplied only once",
                        out,
                    ));
                }
                let Some(value) = args.next() else {
                    return Err(TaskCliError::missing("--driver requires a route name", out));
                };
                if !valid_runner_identity(&value) {
                    return Err(TaskCliError::invalid(
                        "--driver must be 1-256 portable identity characters",
                        out,
                    ));
                }
                out.driver = Some(value);
            }
            "--reasoning-effort" => {
                if !seen.insert("--reasoning-effort") {
                    return Err(TaskCliError::invalid(
                        "--reasoning-effort may be supplied only once",
                        out,
                    ));
                }
                let Some(value) = args.next() else {
                    return Err(TaskCliError::missing(
                        "--reasoning-effort requires a provider-supported level",
                        out,
                    ));
                };
                let value = value.trim().to_ascii_lowercase();
                if value.is_empty()
                    || value.len() > 32
                    || !value
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
                {
                    return Err(TaskCliError::invalid(
                        "--reasoning-effort must be a short alphanumeric level",
                        out,
                    ));
                }
                out.reasoning_effort = Some(value);
            }
            "--task-pace" => {
                if !seen.insert("--task-pace") {
                    return Err(TaskCliError::invalid(
                        "--task-pace may be supplied only once",
                        out,
                    ));
                }
                let Some(value) = args.next() else {
                    return Err(TaskCliError::missing(
                        "--task-pace requires auto, rapid, or deep",
                        out,
                    ));
                };
                let value = value.trim().to_ascii_lowercase();
                if !matches!(value.as_str(), "auto" | "rapid" | "deep") {
                    return Err(TaskCliError::invalid(
                        "--task-pace requires auto, rapid, or deep",
                        out,
                    ));
                }
                out.task_pace = Some(value);
            }
            "--max-hops" => {
                if !seen.insert("--max-hops") {
                    return Err(TaskCliError::invalid(
                        "--max-hops may be supplied only once",
                        out,
                    ));
                }
                let Some(value) = args.next() else {
                    return Err(TaskCliError::missing(
                        "--max-hops requires a non-negative integer",
                        out,
                    ));
                };
                let Ok(value) = value.parse::<usize>() else {
                    return Err(TaskCliError::invalid(
                        "--max-hops requires a non-negative integer",
                        out,
                    ));
                };
                out.max_hops = Some(value);
            }
            "--deadline-secs" => {
                if !seen.insert("--deadline-secs") {
                    return Err(TaskCliError::invalid(
                        "--deadline-secs may be supplied only once",
                        out,
                    ));
                }
                let Some(value) = args.next() else {
                    return Err(TaskCliError::missing(
                        "--deadline-secs requires a non-negative integer",
                        out,
                    ));
                };
                let Ok(value) = value.parse::<usize>() else {
                    return Err(TaskCliError::invalid(
                        "--deadline-secs requires a non-negative integer",
                        out,
                    ));
                };
                out.deadline_secs = Some(value);
            }
            "--tool-profile" => {
                if !seen.insert("--tool-profile") {
                    return Err(TaskCliError::invalid(
                        "--tool-profile may be supplied only once",
                        out,
                    ));
                }
                let Some(value) = args.next() else {
                    return Err(TaskCliError::missing(
                        "--tool-profile requires auto, essential, lean, or full",
                        out,
                    ));
                };
                let value = value.trim().to_ascii_lowercase();
                if !matches!(value.as_str(), "auto" | "essential" | "lean" | "full") {
                    return Err(TaskCliError::invalid(
                        "--tool-profile requires auto, essential, lean, or full",
                        out,
                    ));
                }
                out.tool_profile = Some(value);
            }
            "--rollout" => {
                if !seen.insert("--rollout") {
                    return Err(TaskCliError::invalid(
                        "--rollout may be supplied only once",
                        out,
                    ));
                }
                let Some(value) = args.next() else {
                    return Err(TaskCliError::missing(
                        "--rollout requires off, shadow, or local",
                        out,
                    ));
                };
                let value = value.trim().to_ascii_lowercase();
                if !matches!(value.as_str(), "off" | "shadow" | "local") {
                    return Err(TaskCliError::invalid(
                        "--rollout requires off, shadow, or local",
                        out,
                    ));
                }
                out.rollout_capture = Some(value);
            }
            "--require-rollout" => {
                if !seen.insert("--require-rollout") {
                    return Err(TaskCliError::invalid(
                        "--require-rollout may be supplied only once",
                        out,
                    ));
                }
                out.require_rollout = true;
            }
            "--require-rendered-output" => {
                if !seen.insert("--require-rendered-output") {
                    return Err(TaskCliError::invalid(
                        "--require-rendered-output may be supplied only once",
                        out,
                    ));
                }
                out.require_rendered_output = true;
            }
            "-" => {}
            _ if arg.starts_with('-') && out.prompt.is_none() => {
                return Err(TaskCliError::invalid(
                    format!("unknown task option: {arg}"),
                    out,
                ));
            }
            _ if out.prompt.is_none() => out.prompt = Some(arg),
            _ => {
                return Err(TaskCliError::invalid(
                    "task prompt must be one quoted argument",
                    out,
                ));
            }
        }
    }
    if positional_only {
        return Err(TaskCliError::missing(
            "-- must be followed by one quoted task prompt",
            out,
        ));
    }
    if out.require_rollout && !matches!(out.rollout_capture.as_deref(), Some("shadow" | "local")) {
        return Err(TaskCliError::invalid(
            "--require-rollout requires --rollout shadow or --rollout local",
            out,
        ));
    }
    Ok(out)
}

fn valid_runner_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/')
        })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TaskStartupStopReason {
    InvalidArguments,
    EmptyPrompt,
    WorkspaceError,
    DriverUnavailable,
    NoRoute,
}

/// Work cadence for a task or competition turn. Pace is deliberately separate
/// from competition mode: a long-horizon competition still needs its tools and
/// telemetry, but must not inherit a rapid-fire submit loop merely because the
/// standing goal mentions a leaderboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TaskPace {
    Rapid,
    Deep,
}

impl TaskPace {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Rapid => "rapid",
            Self::Deep => "deep",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TaskPaceResolution {
    pub(crate) requested: String,
    pub(crate) pace: TaskPace,
    pub(crate) source: String,
}

fn parse_task_pace(value: &str) -> Option<TaskPace> {
    match value.trim().to_ascii_lowercase().as_str() {
        "rapid" => Some(TaskPace::Rapid),
        "deep" => Some(TaskPace::Deep),
        _ => None,
    }
}

fn prompt_requests_deep_pace(prompt: &str) -> bool {
    [
        "slow burn",
        "slow-burn",
        "long thought",
        "long-chain",
        "long chain",
        "deep solve",
        "deep research",
        "long horizon",
        "long-horizon",
        "major solve",
        "overnight",
        "do not submit",
        "no submission",
        "without submitting",
    ]
    .into_iter()
    .any(|needle| super::ascii_contains_ignore_case(prompt, needle))
}

fn prompt_requests_rapid_pace(prompt: &str) -> bool {
    [
        "rapid fire",
        "rapid-fire",
        "quick pass",
        "quick solve",
        "smoke test",
        "fire now",
        "submit now",
        "fire one off",
    ]
    .into_iter()
    .any(|needle| super::ascii_contains_ignore_case(prompt, needle))
}

/// Resolve pace from explicit operator configuration or task intent.
/// Execution budgets do not select a slower cadence.
pub(crate) fn resolve_task_pace(prompt: &str) -> TaskPaceResolution {
    let requested = std::env::var("ANGEL_TASK_PACE")
        .unwrap_or_else(|_| "auto".to_string())
        .trim()
        .to_ascii_lowercase();
    if let Some(pace) = parse_task_pace(&requested) {
        return TaskPaceResolution {
            requested,
            pace,
            source: "explicit".to_string(),
        };
    }
    if prompt_requests_deep_pace(prompt) {
        return TaskPaceResolution {
            requested,
            pace: TaskPace::Deep,
            source: "prompt".to_string(),
        };
    }
    if prompt_requests_rapid_pace(prompt) {
        return TaskPaceResolution {
            requested,
            pace: TaskPace::Rapid,
            source: "prompt".to_string(),
        };
    }
    // Operator law 2026-09-11: an unbounded or long budget is NOT a request for
    // slow-burn cadence. With hop/deadline caps off by default (L00/L01) every
    // competition loop was inferred DEEP and told "submit only when the operator
    // explicitly asks" — the loop ran for hours with measured candidates and
    // zero submissions. Depth is opted into by words or ANGEL_TASK_PACE=deep only.
    TaskPaceResolution {
        requested,
        pace: TaskPace::Rapid,
        source: "default".to_string(),
    }
}

/// Pace visible to the live turn loop. Headless startup stamps a resolved value
/// so receipt generation and turn policy cannot independently classify the same
/// task; interactive turns fall back to the latest operator message.
pub(crate) fn configured_task_pace(history: &[super::ChatMsg]) -> TaskPace {
    if let Ok(value) = std::env::var("ANGEL_TASK_PACE_RESOLVED")
        && let Some(pace) = parse_task_pace(&value)
    {
        return pace;
    }
    let prompt = history
        .iter()
        .rev()
        .find(|message| message.role == super::ChatRole::User)
        .map(|message| message.content.as_ref())
        .unwrap_or_default();
    resolve_task_pace(prompt).pace
}

impl TaskStartupStopReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::InvalidArguments => "invalid_arguments",
            Self::EmptyPrompt => "empty_prompt",
            Self::WorkspaceError => "workspace_error",
            Self::DriverUnavailable => "driver_unavailable",
            Self::NoRoute => "no_route",
        }
    }
}

/// Resolve the per-turn hop guard from `ANGEL_MAX_HOPS`.
///
/// The harness uses `Option<usize>` for the guard, where `None` is unbounded. The
/// public environment contract uses `0` for that same meaning, so normalize it
/// here instead of accidentally passing `Some(0)` to the turn loop (which would
/// stop before the first provider call). Headless and interactive turns are
/// unbounded by default; only a positive operator override enables this cap.
pub(crate) fn configured_max_hops() -> Option<usize> {
    // Launch config in release: the worker asks once per turn. Tests keep the
    // live read so EnvGuard hop caps stay visible.
    #[cfg(not(test))]
    {
        static HOPS: std::sync::OnceLock<Option<usize>> = std::sync::OnceLock::new();
        *HOPS.get_or_init(|| parse_task_max_hops(std::env::var("ANGEL_MAX_HOPS").ok().as_deref()))
    }
    #[cfg(test)]
    parse_task_max_hops(std::env::var("ANGEL_MAX_HOPS").ok().as_deref())
}

/// Install the production headless coding policy without changing interactive
/// TUI turns or overriding an evaluator/operator ablation. These are the same
/// bounded values used by the pinned Prime harness: enough reconnaissance to
/// locate the edit, then an action-or-explain boundary; once a mutation exists,
/// reserve the final six hops for verification or a diagnostic-backed fix.
/// Mutation thrash / peripheral fan-out defaults come from the Roll 07
/// retained-failure analysis (identical multi_edit replay and docs/testdata
/// fan-out without core library hits).
///
/// Production headless also defaults **Treebeard auto** (RLM/HiQ strategy root
/// when hop budget ≥ floor) and **relentless execution** so stalled pure-text
/// hops re-enter tool use. Classic ReAct ablation still wins with
/// `ANGEL_TASK_TREEBEARD=0` (or explicit `ANGEL_LANE=…`).
pub(crate) fn apply_task_runtime_defaults(prompt: &str) -> TaskPaceResolution {
    for (key, value) in [
        // Run limits are off unless the operator supplies a positive override.
        ("ANGEL_MAX_HOPS", "0"),
        ("ANGEL_TURN_DEADLINE_SECS", "0"),
        // Keep advisory settings; legacy stop knobs remain off.
        ("ANGEL_MUTATION_THRASH_NUDGE", "3"),
        ("ANGEL_MUTATION_THRASH_STOP", "0"),
        // Four peripheral-only mutations without a core hit → layer nudge.
        ("ANGEL_PERIPHERAL_MUTATION_NUDGE", "4"),
        // Green verification does not truncate remaining requested work.
        ("ANGEL_POST_GREEN_TOOL_BATCHES", "0"),
        ("ANGEL_NO_EDIT_ANSWER_GUARD", "0"),
        // Free preturn map (repo_recon + symbol/lockfile packet) so paid hops
        // are not spent on first-pass reading (expensive reading fix).
        ("ANGEL_TASK_RECON", "repo"),
        // Compact repair discipline block in the headless system prompt.
        ("ANGEL_TASK_CODING_DISCIPLINE", "1"),
        // Always inject workspace root + shallow file inventory so paid hops
        // are not spent on `find /` / `$HOME` thrash under YOLO.
        ("ANGEL_TASK_WORKSPACE_MAP", "1"),
        // Stalled pure-text hops → force tool-backed progress (same latch as
        // /relentless); explicit 0 still disables.
        ("ANGEL_RELENTLESS_EXECUTION", "1"),
        // RLM/HiQ by default for professional hop budgets; set
        // ANGEL_TASK_TREEBEARD=0 for classic ReAct ablation control.
        ("ANGEL_TASK_TREEBEARD", "auto"),
        // Mark the process as headless `--task` so tools can apply task-only
        // orientation without changing interactive TUI behavior.
        ("ANGEL_TASK_ACTIVE", "1"),
        // Resolved exactly once below. Typed CLI and operator environment
        // values retain precedence over this auto default.
        ("ANGEL_TASK_PACE", "auto"),
    ] {
        if std::env::var_os(key).is_none() {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var(key, value) };
        }
    }
    let pace = resolve_task_pace(prompt);
    // A headless task already has a wall-clock ceiling. Its verifier (a
    // sandboxed build plus a benchmark) is one tool call that may legitimately
    // run for most of that ceiling, and the interactive 120 s default killed
    // every real Yukon verifier the fleet ran on 2026-09-05 (matrices ~3-4 min,
    // Gemma build 284 s + benchmark 585 s) — twice each, until the worker
    // stopped with a blocker. Derive both tool ceilings from the deadline when
    // the operator has not pinned them.
    let deadline_secs = super::configured_turn_deadline_secs();
    if deadline_secs > 0 {
        for key in ["ANGEL_TOOL_TIMEOUT", "ANGEL_TOOL_HARD_TIMEOUT"] {
            if std::env::var_os(key).is_none() {
                // TODO: Audit that the environment access only happens in single-threaded code.
                unsafe { std::env::set_var(key, deadline_secs.to_string()) };
            }
        }
    }
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_TASK_PACE_RESOLVED", pace.pace.as_str()) };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_TASK_PACE_SOURCE", &pace.source) };
    // Final-mile hints remain bounded-run defaults. First-write pressure is
    // deliberately not inferred: live Yukon evidence audits legitimately need
    // long read-only stretches, so only an explicit ANGEL_FIRST_WRITE_CALLS
    // operator setting may arm that policy.
    let unbounded = configured_max_hops().is_none();
    let (final_mile, answer_window) = match pace.pace {
        TaskPace::Rapid if !unbounded => ("6", "2"),
        _ => ("4", "1"),
    };
    for (key, value) in [
        ("ANGEL_FINAL_MILE_HOPS", final_mile),
        ("ANGEL_FINAL_MILE_ANSWER_HOPS", answer_window),
    ] {
        if std::env::var_os(key).is_none() {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var(key, value) };
        }
    }
    maybe_apply_task_treebeard_lane();
    pace
}

/// Optional headless Treebeard (RLM/HiQ) activation when `ANGEL_LANE` is unset.
///
/// `ANGEL_TASK_TREEBEARD`:
/// - unset — leave the global Treebeard default alone
/// - `0` / `off` / `false` — force `ANGEL_LANE=default` (ReAct ablation)
/// - `1` / `on` / `always` — force `ANGEL_LANE=treebeard` for this `--task`
/// - `auto` — treebeard when hop budget is unbounded or
///   `>= ANGEL_TASK_TREEBEARD_HOPS` (default **32**)
///
/// Explicit `ANGEL_LANE` always wins. Interactive TUI never calls this.
pub(crate) fn maybe_apply_task_treebeard_lane() {
    if std::env::var_os("ANGEL_LANE").is_some() {
        return;
    }
    let mode = std::env::var("ANGEL_TASK_TREEBEARD")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if mode.is_empty() {
        return;
    }
    if matches!(mode.as_str(), "0" | "off" | "false" | "no") {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_LANE", "default") };
        return;
    }
    let enable = match mode.as_str() {
        "1" | "on" | "true" | "yes" | "always" | "treebeard" | "rlm" | "hiq" => true,
        "auto" => {
            let floor = std::env::var("ANGEL_TASK_TREEBEARD_HOPS")
                .ok()
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(32);
            match configured_max_hops() {
                None => true, // unbounded = long
                Some(h) => h >= floor,
            }
        }
        _ => false,
    };
    if enable {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_LANE", "treebeard") };
    }
}

fn parse_task_max_hops(raw: Option<&str>) -> Option<usize> {
    match raw.and_then(|value| value.trim().parse::<usize>().ok()) {
        Some(0) => None,
        Some(hops) => Some(hops),
        None => None,
    }
}

/// Stopped task-mode outcomes are machine failures by default even though the
/// truthful JSON/stdout note is still emitted. Explicitly opt into the legacy
/// zero-exit behavior only for a caller that grades the envelope itself.
pub(crate) fn task_strict_exit() -> bool {
    parse_task_strict_exit(std::env::var("ANGEL_TASK_STRICT_EXIT").ok().as_deref())
}

fn parse_task_strict_exit(raw: Option<&str>) -> bool {
    !matches!(
        raw.unwrap_or("1").trim().to_ascii_lowercase().as_str(),
        "0" | "false" | "off" | "no" | "disable" | "disabled"
    )
}

// ---------------------------------------------------------------------------
// Warm start — the workspace context blocks headless `--task` used to skip.
// ---------------------------------------------------------------------------

/// The workspace warm-start for headless `--task`, mirroring the blocks the
/// interactive bootstrap injects (`bootstrap::build_system_prompt`) minus the
/// ones that only make sense with a human at the keyboard and the full TUI
/// toolbelt.
///
/// Why it exists: `--task` is the seat EVERY autonomous coding run goes through
/// (the Conductor's code rung included) and it opened **cold** — no verified
/// rituals, no standing traps, no "last session here". A capable model dropped
/// into a large repo with no map re-reads its way into the churn guard instead
/// of writing code. Measured 2026-07-11: two headless runs on real backlog items
/// burned 588s and 676s and died in the re-read guard with zero writes.
///
/// What rides along, and why it earns its tokens:
///   * the **work context** — but only once a human has CONFIRMED it. The mode it
///     carries (internal-dev velocity vs public-facing-care) governs secret
///     hygiene for an agent that writes and commits unattended, and it is ~3
///     lines. The *unconfirmed* branch of that block is deliberately dropped: it
///     instructs the agent to call `work_landing` — a tool task mode does not
///     register — and to confirm with a user who is not there. In an unattended
///     seat that is pure churn, which is the exact failure being fixed.
///   * the **repo dossier** — verified rituals, standing traps, and where the
///     last session left off, confidence-gated by `ANGEL_DOSSIER_MIN_BELIEF`
///     (default 0.70) so a low-belief guess is *withheld*, never asserted.
///
/// What deliberately does NOT ride along: the self-model
/// (`self_model::self_context`). A read-only `self_map` is available on demand
/// only with an explicit ANGEL_SELF_SRC pin. The context block is by far the
/// largest of the blocks, and it maps the
/// *cockpit's own* source — irrelevant to the arbitrary repo a headless task
/// usually runs in, and self-modification priming is the last thing an
/// unattended seat needs.
///
/// Both halves render to an empty string when there is nothing confident to say,
/// so a workspace angel has never seen pays exactly zero tokens.
pub(crate) fn task_system_contract(workspace: &Path) -> String {
    let mut block = task_pace_contract_block();
    block.push_str(&task_coding_discipline_block());
    // The workspace map is harness-derived (an absolute root and file names,
    // no repository text), so it stays in the System contract: inside the
    // JSON workspace-context carrier Flash read the escaped map next to the
    // `workspace` field and prefixed the directory name to its paths (two
    // read_file errors per js-duration-parser run on the 2026-09-07 merge);
    // in the System prompt it navigated cleanly (arena r4, 2026-08-10 note).
    block.push_str(&task_workspace_map_block(workspace));
    block
}

/// Repository-derived context is data; callers must use a non-System carrier.
pub(crate) fn task_warm_start(workspace: &Path) -> String {
    let mut block = String::new();
    block.push_str(&task_work_context_block(workspace));
    let dossier = task_dossier_block(workspace);
    if !dossier.is_empty() {
        block.push_str(&crate::knowledge::evidence::fence(
            "dossier",
            &format!(
                "repo={} age=unknown verification=belief-gated",
                crate::platform::workspace_store::repo_identity(workspace).key
            ),
            &dossier,
        ));
    }
    // Caddy M06b: cost-aware recipes/hazards card right after the dossier
    // block — only entries relevant to this workspace's verifier family.
    let card = crate::knowledge::caddy::render_card_for_task(
        workspace,
        crate::knowledge::caddy::card_cap(),
    );
    if !crate::agent::backplane::active() && !card.is_empty() {
        block.push_str(&crate::knowledge::evidence::fence(
            "caddy",
            &format!(
                "repo={} age=unknown verification=store-claimed",
                crate::platform::workspace_store::repo_identity(workspace).key
            ),
            &card,
        ));
    }
    block.push_str(&task_continual_harness_block(workspace));
    block
}

fn task_pace_contract_block() -> String {
    match std::env::var("ANGEL_TASK_PACE_RESOLVED")
        .ok()
        .as_deref()
        .and_then(parse_task_pace)
    {
        Some(TaskPace::Deep) => "\n## Task pace: deep\n\
This is a slow-burn solve. Build and test an evidence chain before converging. Hop count alone is \
never an instruction to submit — but a candidate that passes the local gate is submitted (receipt/ID) \
and then improved. Submit immediately after the required protected gates pass, record the platform ID, \
and follow official acceptance and promotion while deeper experiments continue independently. \
A speculative larger gain never delays a validated win; only an already validated larger candidate \
ready for the same immediate upload replaces it. Prepare attribution and submission notes during validation. \
Continue through implementation and verification, but do not trade away necessary reasoning for artificial cadence.\n"
            .to_string(),
        Some(TaskPace::Rapid) => "\n## Task pace: rapid\n\
This is a rapid-fire solve. Map the smallest relevant surface, make an evidence-backed change, \
run the narrow verifier, and finish without broad reconnaissance. In a competition loop the loop \
itself is the submission contract: after the required protected gates pass, immediately submit the current \
best, record the platform ID, and follow official acceptance and promotion while improving the next. \
Prepare attribution and notes during validation. Never delay a validated win for a speculative larger gain; \
only an already validated larger candidate ready for the same immediate upload replaces it. \
Never wait for a further go-ahead within the authorized submission scope.\n"
            .to_string(),
        None => String::new(),
    }
}

/// Free preturn workspace orientation: absolute root + shallow file inventory.
/// Measured 2026-08-10: without this, Flash under YOLO spent 14/16 hops on
/// `find /` and `$HOME` scans before touching the fixture (js-duration-parser /
/// js-retry-plan max_hops fails). Off with `ANGEL_TASK_WORKSPACE_MAP=0`.
fn task_workspace_map_block(workspace: &Path) -> String {
    if !super::env_flag("ANGEL_TASK_WORKSPACE_MAP", false) {
        return String::new();
    }
    let root = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    let max_entries = super::env_usize("ANGEL_TASK_WORKSPACE_MAP_MAX", 80).clamp(8, 400);
    let max_bytes =
        super::env_usize("ANGEL_TASK_WORKSPACE_MAP_BYTES", 6 * 1024).clamp(512, 32 * 1024);
    let entries = collect_workspace_rel_paths(&root, max_entries);
    let mut body = String::from("\n## Task workspace map\n");
    body.push_str(&format!(
        "Coding root (absolute): `{}`\n\
         Stay inside this directory for reads, edits, and tests unless the task \
         explicitly needs an external resource. Prefer `read_file`/`grep`/`ls` here \
         over `find /` or scans of `$HOME`.\n",
        root.display()
    ));
    if entries.is_empty() {
        body.push_str("Inventory: (empty workspace)\n");
    } else {
        body.push_str("Inventory (relative paths):\n");
        for path in &entries {
            body.push_str("- `");
            body.push_str(path);
            body.push_str("`\n");
        }
        if entries.len() >= max_entries {
            body.push_str("… inventory truncated; use tools for the rest.\n");
        }
    }
    if body.len() > max_bytes {
        // Keep the header + root line even if the list is long.
        let mut end = max_bytes;
        while end > 0 && !body.is_char_boundary(end) {
            end -= 1;
        }
        body.truncate(end);
        body.push_str("\n… truncated.\n");
    }
    body
}

fn collect_workspace_rel_paths(root: &Path, max_entries: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(mut entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        // Stable order so the prompt prefix stays cache-friendly across runs.
        let mut batch: Vec<_> = entries.by_ref().flatten().collect();
        batch.sort_by_key(|e| e.file_name());
        for entry in batch {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.')
                || name == "node_modules"
                || name == "target"
                || name == "dist"
                || name == "__pycache__"
                || name == "opencode.json"
            {
                continue;
            }
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            let rel = path
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| path.display().to_string());
            if ft.is_dir() {
                stack.push(path);
            } else if ft.is_file() {
                out.push(rel);
                if out.len() >= max_entries {
                    out.sort();
                    return out;
                }
            }
        }
    }
    out.sort();
    out
}

/// Continual-harness warm-start for headless `--task` (same renderer as the
/// interactive system prompt). Suppress with `ANGEL_CONTINUAL_HARNESS=0` or
/// `ANGEL_CONTINUAL_HARNESS_TASK=0` for hermetic benchmarks.
fn task_continual_harness_block(workspace: &Path) -> String {
    if !super::env_flag("ANGEL_CONTINUAL_HARNESS_TASK", true) {
        return String::new();
    }
    crate::drive::continual_harness::context_block(workspace)
}

/// Compact repository-repair discipline for headless coding. Kept short; the
/// harness also enforces thrash/self-authored-test/peripheral-fanout guards.
/// Off by default so interactive/hermetic warm-start tests stay empty; headless
/// task mode turns it on via [`apply_task_runtime_defaults`].
fn task_coding_discipline_block() -> String {
    if !super::env_flag("ANGEL_TASK_CODING_DISCIPLINE", false) {
        return String::new();
    }
    "\n## Coding repair discipline\n\
     Action ladder (stay on it):\n\
     1. **Map** — use the workspace map + preturn recon; open implementing files with \
`read_file`/`grep` *inside the coding root*. Do not `find /`, `find $HOME`, or inventory unrelated repos.\n\
     2. **Edit** — smallest source change in src/pkg/lib/core/machinery (not docs/testdata copies).\n\
     3. **Verify** — run a *pre-existing* project test/check that matches the bug; read its diagnostics.\n\
     4. **Finish** — after green, complete any remaining requested work; after red, fix from diagnostics.\n\
     Speed:\n\
     - Batch independent reads/searches in parallel in a single tool hop (do not serialize one file at a time).\n\
     - Prefer `code_mode` for multi-step local inspect/transform when it saves hops.\n\
     - Skip re-reading files you just opened; act or name the blocker.\n\
     Rules:\n\
     - Prefer the implementing library over hand-editing every docs/testdata/example copy.\n\
     - Unique `old` context: when a short snippet appears twice, include the enclosing function/block.\n\
     - Do not invent new tests as completion proof; use existing suite entry points.\n\
     - Multi-surface bugs (code + config/markdown/model): update every surface the behavior needs.\n\
     - Dependency bugs: bump go.mod/package.json/Cargo.toml instead of vendoring a fork.\n\
     - After a green project verifier, continue until every requested deliverable is complete.\n\
     - Headless task exit stops remaining `proc_run` jobs, even after a successful answer. \
Finish required builds/checks before answering; starting a background job is not verification. \
Save a checkpoint before ending the task.\n\
     - Read captured background output through `proc_status`; its retained host log may be \
outside the workspace and unavailable to `read_file`.\n\
     - If all remaining work depends on a running job, use `proc_wait` for a cancellable \
wait of at most 30 seconds that returns early on exit; avoid shell sleep loops.\n\
     - Never claim fixed without a workspace mutation (or a concrete blocker with residual risk).\n\
     - Keep root strategy short; park bulk under handles/`code_mode` when the lane is Treebeard.\n\
     - One failed identical patch → change approach (more context, different path, or measure first).\n\
     - Treat tool results as evidence: keep the earliest actual prerequisite failure; confirm \
usable input before dependent measurements; stay inside allowed scratch; discover optional \
dependencies and authorized reference paths from real tool errors and permissions. Do not score \
a failed or denied input as zero performance. Explicit read_only/write_paths restrictions stay \
authoritative — ask for a user-visible scope change if needed, never omit or auto-remove them.\n"
        .to_string()
}

/// The dossier warm-start block for headless task mode: the *same* renderer the
/// TUI uses (`dossier::context_block` — one renderer, one confidence gate, one
/// size cap), suppressible on its own with `ANGEL_DOSSIER_TASK=0` for operators
/// who want a hermetic headless prompt (benchmarks pinning an exact preamble)
/// without killing the block for interactive sessions too. `ANGEL_DOSSIER=0`
/// still kills both.
fn task_dossier_block(workspace: &Path) -> String {
    if crate::agent::backplane::active() {
        return String::new();
    }
    if !super::env_flag("ANGEL_DOSSIER_TASK", true) {
        return String::new();
    }
    crate::knowledge::dossier::context_block(workspace)
}

/// The work-context block for headless task mode — **confirmed contexts only**
/// (see [`task_warm_start`]). Reuses `work_landing::render_block`; honors the
/// existing `ANGEL_WORK_LANDING` kill switch, so no new knob.
fn task_work_context_block(workspace: &Path) -> String {
    if !super::env_flag("ANGEL_WORK_LANDING", true) {
        return String::new();
    }
    match work_landing::load_context(workspace) {
        Some(ctx) if ctx.confirmed && ctx.mode != Mode::Unset => {
            work_landing::render_block(Some(&ctx))
        }
        _ => String::new(),
    }
}

pub(crate) type TaskTokenUsage = crate::agent::club::AccountingReport;

pub(crate) fn task_usage_delta(
    before: crate::agent::club::AccountingView,
    after: crate::agent::club::AccountingView,
) -> Option<TaskTokenUsage> {
    let report = after.delta(&before);
    (report.attempts > 0 || report.untracked_sources).then_some(report)
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct TaskOutputBudget {
    pub(crate) policy: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) source: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) provenance: Option<String>,
}

impl TaskOutputBudget {
    pub(crate) fn from_route_metadata(metadata: &RouteMetadata) -> Self {
        match metadata.output_budget {
            OutputBudgetPolicy::ProviderNative => Self {
                policy: "provider-native",
                tokens: None,
                source: None,
                provenance: None,
            },
            OutputBudgetPolicy::Explicit { tokens, source } => Self {
                policy: "explicit",
                tokens: Some(tokens),
                source: Some(match source {
                    OutputBudgetSource::PerClubEnv => "per-club-env",
                    OutputBudgetSource::GlobalEnv => "global-env",
                }),
                provenance: metadata.output_budget_provenance.clone(),
            },
            OutputBudgetPolicy::EndpointManaged => Self {
                policy: "endpoint-managed",
                tokens: None,
                source: Some("provider-plan"),
                provenance: metadata.output_budget_provenance.clone(),
            },
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct TaskRuntimeConfig {
    pub(crate) schema: &'static str,
    pub(crate) config_sha256: String,
    pub(crate) runner_version: &'static str,
    pub(crate) cockpit_source_sha256: &'static str,
    pub(crate) prompt_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) requested_driver: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) requested_reasoning_effort: Option<String>,
    pub(crate) requested_task_pace: String,
    pub(crate) task_pace: String,
    pub(crate) task_pace_source: String,
    /// Zero is the public spelling for explicitly unbounded.
    pub(crate) max_hops: usize,
    /// Zero is the public spelling for explicitly unbounded.
    pub(crate) deadline_secs: usize,
    pub(crate) tool_profile: String,
    pub(crate) rollout_capture: String,
    pub(crate) rollout_required: bool,
    pub(crate) unrestricted: bool,
    pub(crate) verification_policy: &'static str,
}

impl TaskRuntimeConfig {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        prompt_sha256: String,
        requested_driver: Option<String>,
        requested_reasoning_effort: Option<String>,
        task_pace: TaskPaceResolution,
        max_hops: Option<usize>,
        deadline_secs: usize,
        tool_profile: String,
        rollout_capture: String,
        rollout_required: bool,
        unrestricted: bool,
    ) -> Self {
        let runner_version = env!("CARGO_PKG_VERSION");
        let cockpit_source_sha256 = crate::agent::harness::run_identity::source_sha256();
        let max_hops = max_hops.unwrap_or(0);
        let canonical = serde_json::json!({
            "schema": "angel-task-runtime/v1",
            "runner_version": runner_version,
            "cockpit_source_sha256": cockpit_source_sha256,
            "prompt_sha256": prompt_sha256.clone(),
            "requested_driver": requested_driver.clone(),
            "requested_reasoning_effort": requested_reasoning_effort.clone(),
            "requested_task_pace": task_pace.requested.clone(),
            "task_pace": task_pace.pace.as_str(),
            "task_pace_source": task_pace.source.clone(),
            "max_hops": max_hops,
            "deadline_secs": deadline_secs,
            "tool_profile": tool_profile.clone(),
            "rollout_capture": rollout_capture.clone(),
            "rollout_required": rollout_required,
            "unrestricted": unrestricted,
            "verification_policy": "external-only",
        });
        let config_sha256 = crate::knowledge::cut::sha256_hex(
            &serde_json::to_vec(&canonical).expect("task runtime config must serialize"),
        );
        Self {
            schema: "angel-task-runtime/v1",
            config_sha256,
            runner_version,
            cockpit_source_sha256,
            prompt_sha256,
            requested_driver,
            requested_reasoning_effort,
            requested_task_pace: task_pace.requested,
            task_pace: task_pace.pace.as_str().to_string(),
            task_pace_source: task_pace.source,
            max_hops,
            deadline_secs,
            tool_profile,
            rollout_capture,
            rollout_required,
            unrestricted,
            verification_policy: "external-only",
        }
    }

    pub(crate) fn rollout_binding(
        &self,
        task_id: Option<String>,
        run_id: Option<String>,
    ) -> super::TaskRolloutBindingV1 {
        super::TaskRolloutBindingV1::new(
            task_id,
            run_id,
            self.prompt_sha256.clone(),
            self.config_sha256.clone(),
            self.runner_version.to_string(),
            self.cockpit_source_sha256.to_string(),
        )
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct TaskJsonEnvelope {
    pub(crate) authority_profile: crate::platform::authority_profile::AuthorityProfile,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) formation_budget: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) budget_exhausted: bool,
    /// Compatibility field: spend exceeded allocation; no reservation was denied.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) reservation_denied: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) over_allocation: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) over_allocation_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reward_binding: Option<super::rollout::RewardReceipt>,
    pub(crate) identity: Option<super::run_identity::RunIdentity>,
    pub(crate) escalations: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) hop_stream_cuts: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_credited_progress: Option<serde_json::Value>,
    /// Sandbox ceiling in force for tool subprocesses: "sealed" | "yolo" | "ordinary" (S02).
    pub(crate) sandbox_profile: &'static str,
    pub(crate) version: u8,
    pub(crate) kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) run_id: Option<String>,
    pub(crate) workspace: String,
    pub(crate) status: &'static str,
    pub(crate) stop_reason: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) answer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stop_notice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<serde_json::Value>,
    pub(crate) hops: usize,
    pub(crate) interrupted: bool,
    pub(crate) deadline_reached: bool,
    pub(crate) max_hops_reached: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) club: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reasoning_effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) output_budget: Option<TaskOutputBudget>,
    pub(crate) elapsed_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) timing: Option<TaskTimingTelemetry>,
    /// Per-call tool ledger of the turn (hop, tool, exec, err, class, verify,
    /// ms, bytes); omitted when no tool was dispatched.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) tools: Vec<serde_json::Value>,
    /// M05: caddy memory-health summary (counts only) for this workspace's
    /// recipe/hazard stores.
    pub(crate) tools_output: super::turn::background::ToolsOutput,
    pub(crate) store_rotations: Vec<serde_json::Value>,
    pub(crate) memory_health: crate::knowledge::caddy::StoreHealthSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) acceptance: Option<TaskAcceptanceTelemetry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) accepted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) rollout_id: Option<String>,
    /// Capture failures do not replace the answer or stop an optional-capture run.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) rollout_capture_errors: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) runtime: Option<TaskRuntimeConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) usage: Option<TaskTokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) session_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) artifacts: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) graph_episode_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) graph_episodes: Option<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) graph_episode_list: Vec<super::GraphTurnEpisode>,
}

pub(crate) struct TaskJsonContext {
    pub(crate) task_id: Option<String>,
    pub(crate) run_id: Option<String>,
    pub(crate) workspace: PathBuf,
    pub(crate) club: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) reasoning_effort: Option<String>,
    pub(crate) output_budget: Option<TaskOutputBudget>,
    pub(crate) elapsed_ms: u128,
    pub(crate) timing: Option<TaskTimingTelemetry>,
    pub(crate) tools: Vec<serde_json::Value>,
    pub(crate) usage: Option<TaskTokenUsage>,
    pub(crate) runtime: Option<TaskRuntimeConfig>,
    pub(crate) session_id: Option<String>,
    pub(crate) artifacts: Vec<String>,
    /// M05: caddy memory-health summary (counts only), set by the task
    /// runner before the envelope is assembled.
    pub(crate) memory_health: crate::knowledge::caddy::StoreHealthSummary,
}

fn graph_incomplete_stop_reason(reason: &str) -> &'static str {
    if reason == "cancelled" || reason.contains("cancelled") {
        "cancelled"
    } else if reason.starts_with("planner") || reason.contains("planner_no_plan") {
        "planner_no_plan"
    } else if reason.contains("no_schedulable") {
        "no_schedulable_nodes"
    } else if reason.starts_with("node_failed") {
        "node_failed"
    } else if reason.starts_with("provider_stall") {
        "provider_stall"
    } else {
        "incomplete"
    }
}

impl TaskJsonEnvelope {
    pub(crate) fn from_outcome(
        mut ctx: TaskJsonContext,
        outcome: TurnOutcome,
        history: &[super::ChatMsg],
    ) -> Self {
        // The turn's own measurement wins; a caller-supplied context timing
        // (none today) would only cover work outside the observed turn.
        ctx.timing = outcome.timing.or(ctx.timing);
        ctx.tools = outcome.tools.clone();
        // Caddy v0 write-back: fold this task's tool receipts into the
        // per-repo recipe/hazard store. The envelope has no metadata map, so
        // the counts surface on stderr instead of as new top-level keys.
        let (caddy_recipes, caddy_hazards) =
            crate::knowledge::caddy::write_back_from_history(&ctx.workspace, history);
        if caddy_recipes + caddy_hazards > 0 {
            eprintln!("[angel --task] caddy: +{caddy_recipes} recipes +{caddy_hazards} hazards");
        }
        let reward_binding = outcome.reward_binding;
        let ran_graph = outcome.tools.iter().any(|row| {
            row.get("tool")
                .or_else(|| row.get("name"))
                .and_then(|v| v.as_str())
                == Some("agent_graph")
        });
        let graph_episodes = crate::agent::harness::graph_turn_episodes();
        let judged = ran_graph
            .then(crate::agent::harness::judged_graph_snapshot)
            .flatten();
        let graph_incomplete = judged.as_ref().and_then(|snap| {
            if snap.fanin.finished_with_result {
                None
            } else {
                Some(
                    snap.incomplete_reason
                        .clone()
                        .or(snap.fanin.reason.clone())
                        .unwrap_or_else(|| "incomplete".to_string()),
                )
            }
        });
        let mut envelope = Self::new(
            ctx,
            if graph_incomplete.is_some() {
                "incomplete"
            } else if outcome.stop_reason == TurnStopReason::Answer {
                "completed"
            } else {
                "stopped"
            },
            graph_incomplete
                .as_deref()
                .map(graph_incomplete_stop_reason)
                .unwrap_or_else(|| outcome.stop_reason.as_str()),
            Some(outcome.answer),
            None,
            outcome.hops,
            outcome.interrupted,
            outcome.deadline_reached,
            outcome.max_hops_reached,
            outcome.acceptance,
            outcome.rollout_id,
        );
        envelope.stop_notice = outcome.stop_notice;
        envelope.reward_binding = reward_binding;
        envelope.graph_episode_id = judged.as_ref().map(|s| s.episode_id.clone());
        envelope.graph_episodes = (!graph_episodes.is_empty()).then_some(graph_episodes.len());
        envelope.graph_episode_list = graph_episodes;
        envelope
    }

    pub(crate) fn from_failure(ctx: TaskJsonContext, failure: TurnFailure) -> Self {
        let status = if failure.stop_reason == TurnStopReason::MaxHops {
            "stopped"
        } else {
            "error"
        };
        Self::new(
            ctx,
            status,
            failure.stop_reason.as_str(),
            None,
            Some(failure.message),
            failure.hops,
            failure.interrupted,
            failure.deadline_reached,
            failure.max_hops_reached,
            failure.acceptance.map(|acceptance| *acceptance),
            failure.rollout_id,
        )
    }

    pub(crate) fn from_startup_failure(
        args: &TaskCliArgs,
        workspace: PathBuf,
        elapsed_ms: u128,
        reason: TaskStartupStopReason,
        error: String,
    ) -> Self {
        Self::new(
            TaskJsonContext {
                task_id: args.task_id.clone(),
                run_id: args.run_id.clone(),
                workspace,
                club: None,
                model: None,
                reasoning_effort: None,
                output_budget: None,
                elapsed_ms,
                tools: Vec::new(),
                timing: None,
                usage: None,
                runtime: None,
                session_id: None,
                artifacts: Vec::new(),
                memory_health: crate::knowledge::caddy::StoreHealthSummary::default(),
            },
            "error",
            reason.as_str(),
            None,
            Some(error),
            0,
            false,
            false,
            false,
            None,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        mut ctx: TaskJsonContext,
        status: &'static str,
        stop_reason: &'static str,
        answer: Option<String>,
        error: Option<String>,
        hops: usize,
        interrupted: bool,
        deadline_reached: bool,
        max_hops_reached: bool,
        acceptance: Option<TaskAcceptanceTelemetry>,
        rollout_id: Option<String>,
    ) -> Self {
        if ctx.timing.is_none() && super::trajectory::task_lifecycle_snapshot().is_some() {
            ctx.timing = Some(super::turn::TaskTimingAccumulator::default().finish(0));
        }
        if let Some(timing) = ctx.timing.as_mut() {
            timing.envelope_wall_ms = Some(ctx.elapsed_ms);
            timing.startup_shutdown_ms = ctx.elapsed_ms.checked_sub(timing.wall_ms);
            timing.refresh_task_lifecycle();
            ctx.elapsed_ms = timing.envelope_wall_ms.unwrap_or(ctx.elapsed_ms);
        }
        // Preserve existing stop vocabulary for unaffected tasks. Process kills
        // use their shared receipt reason, including cleanup after an answer.
        let stop_reason = if ctx.tools.iter().any(|tool| tool["status"] == "killed") {
            match stop_reason {
                "interrupt" => "cancelled",
                "idle_timeout" => "tool_idle",
                other => other,
            }
        } else {
            stop_reason
        };
        if let Some(route) = super::trajectory::last_route_switch() {
            ctx.club = Some(route.driver);
            ctx.model = route.model;
            ctx.reasoning_effort = route.reasoning_effort;
            ctx.output_budget = None;
        }
        let accepted = acceptance
            .as_ref()
            .map(TaskAcceptanceTelemetry::task_accepted);
        ctx.memory_health = crate::knowledge::caddy::memory_health(&ctx.workspace);
        let memory_health = std::mem::take(&mut ctx.memory_health);
        let runtime_error = error
            .as_deref()
            .and_then(crate::agent::tools::runtime_missing::RuntimeMissing::decode)
            .or_else(|| crate::agent::tools::runtime_missing::unresolved_verifier(&ctx.tools));
        let status = if runtime_error.is_some() {
            "error"
        } else {
            status
        };
        let formation_budget = super::formation_budget::snapshot();
        let budget_exhausted = formation_budget
            .as_ref()
            .is_some_and(|budget| budget["budget_exhausted"] == true);
        Self {
            authority_profile: crate::platform::authority_profile::active(true),
            budget_exhausted,
            reservation_denied: budget_exhausted,
            over_allocation: formation_budget
                .as_ref()
                .is_some_and(|budget| budget["over_allocation"] == true),
            over_allocation_tokens: formation_budget
                .as_ref()
                .and_then(|budget| budget["over_allocation_tokens"].as_u64()),
            formation_budget,
            reward_binding: None,
            version: 1,
            kind: "angel.task_result",
            identity: super::trajectory::turn_identity(),
            hop_stream_cuts: super::trajectory::progress_ledger_snapshot()["hop_stream_cuts"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
            last_credited_progress: super::trajectory::progress_ledger_snapshot()
                .get("last_credited_progress")
                .filter(|value| !value.is_null())
                .cloned(),
            escalations: super::trajectory::progress_ledger_snapshot()["escalations"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
            sandbox_profile: if crate::agent::sandbox::sealed::identity().is_some() {
                "sealed"
            } else if crate::platform::yolo::enabled() {
                "yolo"
            } else {
                "ordinary"
            },
            task_id: ctx.task_id,
            run_id: ctx.run_id,
            workspace: ctx.workspace.to_string_lossy().into_owned(),
            status,
            stop_reason,
            answer,
            stop_notice: None,
            error: runtime_error
                .map(|error| serde_json::to_value(error).expect("runtime diagnostic"))
                .or_else(|| {
                    error.map(|message| {
                        if stop_reason == "no_route" {
                            serde_json::json!({"kind": "no_route", "message": message})
                        } else {
                            serde_json::Value::String(message)
                        }
                    })
                }),
            hops,
            interrupted,
            deadline_reached,
            max_hops_reached,
            club: ctx.club,
            model: ctx.model,
            reasoning_effort: ctx.reasoning_effort,
            output_budget: ctx.output_budget,
            elapsed_ms: ctx.elapsed_ms,
            tools: ctx.tools,
            memory_health,
            tools_output: super::trajectory::tools_output_snapshot(),
            store_rotations: super::trajectory::store_rotations_snapshot(),
            timing: ctx.timing,
            acceptance,
            accepted,
            rollout_id,
            rollout_capture_errors: Vec::new(),
            runtime: ctx.runtime,
            usage: ctx.usage,
            session_id: ctx.session_id,
            artifacts: ctx.artifacts,
            graph_episode_id: None,
            graph_episodes: None,
            graph_episode_list: Vec::new(),
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/task_mode__tests.rs"]
mod tests;
