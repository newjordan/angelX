//! Headless task CLI parsing and the stable JSON result envelope.

use super::{
    TaskAcceptanceTelemetry, TaskTimingTelemetry, TurnFailure, TurnOutcome, TurnStopReason,
};
use crate::club::{OutputBudgetPolicy, OutputBudgetSource, RouteMetadata};
use crate::tools::work_landing::{self, Mode};
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
        block.push_str(&crate::evidence::fence(
            "dossier",
            &format!(
                "repo={} age=unknown verification=belief-gated",
                crate::workspace_store::repo_identity(workspace).key
            ),
            &dossier,
        ));
    }
    // Caddy M06b: cost-aware recipes/hazards card right after the dossier
    // block — only entries relevant to this workspace's verifier family.
    let card = crate::caddy::render_card_for_task(workspace, crate::caddy::card_cap());
    if !crate::backplane::active() && !card.is_empty() {
        block.push_str(&crate::evidence::fence(
            "caddy",
            &format!(
                "repo={} age=unknown verification=store-claimed",
                crate::workspace_store::repo_identity(workspace).key
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
    crate::continual_harness::context_block(workspace)
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
    if crate::backplane::active() {
        return String::new();
    }
    if !super::env_flag("ANGEL_DOSSIER_TASK", true) {
        return String::new();
    }
    crate::dossier::context_block(workspace)
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

pub(crate) type TaskTokenUsage = crate::club::AccountingReport;

pub(crate) fn task_usage_delta(
    before: crate::club::AccountingView,
    after: crate::club::AccountingView,
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
        let cockpit_source_sha256 = crate::harness::run_identity::source_sha256();
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
        let config_sha256 = crate::cut::sha256_hex(
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
    pub(crate) authority_profile: crate::authority_profile::AuthorityProfile,
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
    pub(crate) memory_health: crate::caddy::StoreHealthSummary,
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
    pub(crate) memory_health: crate::caddy::StoreHealthSummary,
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
            crate::caddy::write_back_from_history(&ctx.workspace, history);
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
        let graph_episodes = crate::harness::graph_turn_episodes();
        let judged = ran_graph
            .then(crate::harness::judged_graph_snapshot)
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
                memory_health: crate::caddy::StoreHealthSummary::default(),
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
        ctx.memory_health = crate::caddy::memory_health(&ctx.workspace);
        let memory_health = std::mem::take(&mut ctx.memory_health);
        let runtime_error = error
            .as_deref()
            .and_then(crate::tools::runtime_missing::RuntimeMissing::decode)
            .or_else(|| crate::tools::runtime_missing::unresolved_verifier(&ctx.tools));
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
            authority_profile: crate::authority_profile::active(true),
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
            sandbox_profile: if crate::sandbox::sealed::identity().is_some() {
                "sealed"
            } else if crate::yolo::enabled() {
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
mod tests {
    use crate::harness::RenderedOutputAcceptance;
    #[test]
    fn task_mode_parses_sealed_profile_and_rejects_unknown_or_duplicate() {
        let args = parse_task_args(
            "--task-json",
            ["--sandbox-profile", "sealed", "fixture"]
                .into_iter()
                .map(str::to_string),
        )
        .unwrap();
        assert_eq!(args.sandbox_profile.as_deref(), Some("sealed"));
        for flags in [
            vec!["--sandbox-profile"],
            vec!["--sandbox-profile", "unknown", "fixture"],
            vec![
                "--sandbox-profile",
                "sealed",
                "--sandbox-profile",
                "sealed",
                "fixture",
            ],
        ] {
            assert!(parse_task_args("--task-json", flags.into_iter().map(str::to_string)).is_err());
        }
    }

    use super::*;
    use crate::tools::work_landing::{Visibility, WorkContext};

    #[test]
    fn authority_profile_task_envelope_reports_full_and_guarded_startup() {
        let _lock = crate::tests::env_lock();
        let _smart = crate::tests::TestEnvGuard::set("ANGEL_YOLO_SMART", "0");
        for (flag, expected) in [("1", "full"), ("0", "guarded")] {
            let _full = crate::tests::TestEnvGuard::set("ANGEL_YOLO", flag);
            let value = serde_json::to_value(TaskJsonEnvelope::from_startup_failure(
                &TaskCliArgs::default(),
                PathBuf::from("/fixture"),
                0,
                TaskStartupStopReason::InvalidArguments,
                "fixture".into(),
            ))
            .unwrap();
            assert_eq!(value["authority_profile"]["profile"], expected);
            assert_eq!(
                value["authority_profile"]["text"],
                crate::authority_profile::active(true).text
            );
        }
    }

    #[test]
    fn task_usage_serializes_partial_zero_and_cache_only_without_fabrication() {
        let cell = crate::club::AccountingCell::default();
        let before = cell.view();
        cell.record(Some(crate::club::UsageObservation {
            raw: [Some(0), None, None, Some(0), None],
            ..Default::default()
        }));
        let usage = task_usage_delta(before, cell.view()).unwrap();
        let value = serde_json::to_value(usage).unwrap();
        assert_eq!(value["input"], 0);
        assert_eq!(value["cache_read"], 0);
        assert!(
            value["output"].is_null()
                && value["reasoning"].is_null()
                && value["uncached_input"].is_null()
        );
        assert_eq!(value["core_complete"], false);
        let before = cell.view();
        cell.record(Some(crate::club::UsageObservation {
            raw: [None, None, None, Some(80), None],
            ..Default::default()
        }));
        let usage = task_usage_delta(before, cell.view()).unwrap();
        assert_eq!(
            (usage.input, usage.output, usage.cache_read),
            (None, None, Some(80))
        );
    }

    // -----------------------------------------------------------------------
    // Warm start.
    // -----------------------------------------------------------------------

    /// A dossier artifact shaped exactly like the compiler's output: one confident
    /// ritual, one below-threshold ritual, one confident trap, and a thread.
    fn dossier_artifact() -> serde_json::Value {
        serde_json::json!({
            "v": 1,
            "repo": { "key": "k", "root": "/r", "slug": "u/p" },
            "generatedAt": "2026-07-11T00:00:00.000Z",
            "facts": [
                { "kind": "ritual", "class": "test", "text": "cargo test --quiet",
                  "meanDurMs": 42_000, "belief": 0.88,
                  "evidence": "9 runs, 9 pass, 4 session(s), last 2026-07-10" },
                { "kind": "ritual", "class": "build", "text": "cargo build --release",
                  "meanDurMs": 90_000, "belief": 0.55,
                  "evidence": "3 runs, 2 pass, 2 session(s), last 2026-07-02" },
                { "kind": "trap", "text": "npm test", "belief": 0.79,
                  "evidence": "3 runs, 0 pass (3 fail), 3 session(s), last 2026-07-05" },
            ],
            "thread": { "ts": 1_783_300_000u64, "stop": "answer", "driver": "gemma", "ok": true },
        })
    }

    /// Clear every knob the warm start reads, so a test never inherits the
    /// developer's real environment (or a previous test's leftovers).
    fn clear_warm_start_env() {
        for key in [
            "ANGEL_DOSSIER",
            "ANGEL_DOSSIER_TASK",
            "ANGEL_DOSSIER_DIR",
            "ANGEL_DOSSIER_MIN_BELIEF",
            "ANGEL_DOSSIER_MAX_BYTES",
            "ANGEL_WORK_LANDING",
            "ANGEL_WORK_CONTEXT_DIR",
            "ANGEL_TASK_WORKSPACE_MAP",
            "ANGEL_TASK_CODING_DISCIPLINE",
            "ANGEL_TASK_PACE",
            "ANGEL_TASK_PACE_RESOLVED",
            "ANGEL_TASK_PACE_SOURCE",
            "ANGEL_CONTINUAL_HARNESS",
            "ANGEL_CONTINUAL_HARNESS_TASK",
        ] {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::remove_var(key) };
        }
    }

    #[test]
    fn task_workspace_map_lists_fixture_files_when_enabled() {
        let _guard = crate::tests::env_lock();
        let (workspace, _d, _w) = warm_start_fixture("map");
        std::fs::create_dir_all(workspace.join("src")).unwrap();
        std::fs::write(workspace.join("src/parse-duration.mjs"), "export\n").unwrap();
        std::fs::write(workspace.join("test.mjs"), "test\n").unwrap();
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_TASK_WORKSPACE_MAP", "1") };

        let block = task_system_contract(&workspace);
        assert!(block.contains("## Task workspace map"), "{block}");
        assert!(block.contains("Coding root (absolute):"), "{block}");
        assert!(block.contains("src/parse-duration.mjs"), "{block}");
        assert!(block.contains("test.mjs"), "{block}");
        assert!(block.contains("Stay inside this directory"), "{block}");
        // The repository-derived carrier no longer duplicates the map.
        assert!(!task_warm_start(&workspace).contains("Task workspace map"));

        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_TASK_WORKSPACE_MAP", "0") };
        assert!(!task_system_contract(&workspace).contains("Task workspace map"));
        clear_warm_start_env();
    }

    /// A private workspace plus empty dossier / work-context dirs, wired into the
    /// env. Nothing is seeded — the tests opt into what they want to exist.
    fn warm_start_fixture(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
        clear_warm_start_env();
        let base =
            std::env::temp_dir().join(format!("angel-task-warm-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let dossier_dir = base.join("dossier");
        let work_dir = base.join("work-context");
        let workspace = base.join("repo");
        for dir in [&dossier_dir, &work_dir, &workspace] {
            std::fs::create_dir_all(dir).unwrap();
        }
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_DOSSIER_DIR", &dossier_dir) };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_WORK_CONTEXT_DIR", &work_dir) };
        (workspace, dossier_dir, work_dir)
    }

    fn seed_dossier(dir: &Path, workspace: &Path) {
        // Dossiers are repository-scoped, including when a fixture directory
        // lives beneath a checkout rather than in a Git-free system temp dir.
        let key = crate::workspace_store::repo_identity(workspace).key;
        std::fs::write(
            dir.join(format!("{key}.json")),
            serde_json::to_string(&dossier_artifact()).unwrap(),
        )
        .unwrap();
    }

    fn seed_work_context(dir: &Path, workspace: &Path, confirmed: bool) {
        let ctx = WorkContext {
            folder: workspace.display().to_string(),
            repo: Some("newjordan/angel0".to_string()),
            visibility: Visibility::Private,
            mode: Mode::InternalDev,
            confirmed,
            updated_at: 1_783_300_000,
        };
        crate::tools::work_landing::save_context_in(dir, workspace, &ctx).unwrap();
    }

    /// The headless seat gets the dossier the TUI gets — confident facts asserted,
    /// below-threshold facts withheld (counted, never stated).
    #[test]
    fn task_warm_start_injects_the_confidence_gated_dossier() {
        let _guard = crate::tests::env_lock();
        let _legacy = crate::tests::TestEnvGuard::set("ANGEL_BACKPLANE", "0");
        let (workspace, dossier_dir, _work_dir) = warm_start_fixture("dossier");
        seed_dossier(&dossier_dir, &workspace);

        let block = task_warm_start(&workspace);
        assert!(
            block.contains(crate::dossier::DOSSIER_BLOCK_HEADER),
            "warm start must carry the dossier block: {block}"
        );
        // S05: memory recall now crosses the evidence fence; the dossier's own
        // sentinel is inside it, so the block ends with the fence close.
        assert!(
            block
                .trim_end()
                .ends_with(crate::evidence::EVIDENCE_FENCE_SENTINEL)
        );
        // The confident ritual and trap are asserted...
        assert!(block.contains("test: `cargo test --quiet`"), "{block}");
        assert!(block.contains("trap: `npm test` fails here"), "{block}");
        assert!(block.contains("last session here"), "{block}");
        // ...and the 0.55-belief build ritual is withheld, not asserted.
        assert!(!block.contains("cargo build --release"), "{block}");
        assert!(
            block.contains("1 fact(s) below 0.70 belief withheld"),
            "{block}"
        );

        clear_warm_start_env();
    }

    /// The confidence gate is the whole safety story: raise it and even the 0.88
    /// ritual is withheld rather than stated.
    #[test]
    fn task_warm_start_honors_the_belief_threshold() {
        let _guard = crate::tests::env_lock();
        let _legacy = crate::tests::TestEnvGuard::set("ANGEL_BACKPLANE", "0");
        let (workspace, dossier_dir, _work_dir) = warm_start_fixture("belief");
        seed_dossier(&dossier_dir, &workspace);
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_DOSSIER_MIN_BELIEF", "0.95") };

        let block = task_warm_start(&workspace);
        assert!(!block.contains("cargo test --quiet"), "{block}");
        assert!(!block.contains("npm test"), "{block}");
        assert!(
            block.contains("3 fact(s) below 0.95 belief withheld"),
            "{block}"
        );

        clear_warm_start_env();
    }

    /// Two kill switches, both honored: the task-mode-only one and the family-wide
    /// one. And a workspace with no artifact is a clean no-op — zero tokens.
    #[test]
    fn task_warm_start_is_suppressible_and_a_no_op_when_empty() {
        let _guard = crate::tests::env_lock();
        let _legacy = crate::tests::TestEnvGuard::set("ANGEL_BACKPLANE", "0");
        let (workspace, dossier_dir, _work_dir) = warm_start_fixture("suppress");

        // Nothing seeded → nothing said.
        assert_eq!(task_warm_start(&workspace), "");

        seed_dossier(&dossier_dir, &workspace);
        assert!(!task_warm_start(&workspace).is_empty());

        // The headless-only knob.
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_DOSSIER_TASK", "0") };
        assert_eq!(task_warm_start(&workspace), "");
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_DOSSIER_TASK") };

        // The family-wide knob still governs both seats.
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_DOSSIER", "0") };
        assert_eq!(task_warm_start(&workspace), "");

        clear_warm_start_env();
    }

    /// The work context rides along only once a human has CONFIRMED it. The
    /// unconfirmed branch would tell an unattended agent to call `work_landing` (a
    /// tool task mode does not register) and to confirm with a user who is not
    /// there — so it must never reach a headless prompt.
    #[test]
    fn task_warm_start_carries_only_a_confirmed_work_context() {
        let _guard = crate::tests::env_lock();
        let (workspace, _dossier_dir, work_dir) = warm_start_fixture("work");

        seed_work_context(&work_dir, &workspace, false);
        let block = task_warm_start(&workspace);
        assert_eq!(
            block, "",
            "unconfirmed context must not prompt a headless run"
        );

        seed_work_context(&work_dir, &workspace, true);
        let block = task_warm_start(&workspace);
        assert!(block.contains("# Active work context"), "{block}");
        assert!(block.contains("mode=internal-dev"), "{block}");
        assert!(block.contains("Optimize for velocity"), "{block}");
        assert!(!block.contains("work_landing"), "{block}");

        // The existing kill switch covers it; no second knob.
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_WORK_LANDING", "0") };
        assert_eq!(task_warm_start(&workspace), "");

        clear_warm_start_env();
    }

    #[test]
    fn task_flags_accept_json_as_entrypoint_or_modifier() {
        let direct = parse_task_args(
            "--task-json",
            [
                "--task",
                "--workspace",
                "/tmp/work",
                "--task-id",
                "case-7",
                "--run-id",
                "run-9",
                "--driver",
                "longcat",
                "--reasoning-effort",
                "high",
                "--task-pace",
                "deep",
                "--max-hops",
                "48",
                "--deadline-secs",
                "900",
                "--tool-profile",
                "essential",
                "--rollout",
                "local",
                "--require-rollout",
                "--require-rendered-output",
                "fix it",
            ]
            .into_iter()
            .map(str::to_string),
        )
        .unwrap();
        assert!(direct.json);
        assert_eq!(direct.workspace, Some(PathBuf::from("/tmp/work")));
        assert_eq!(direct.task_id.as_deref(), Some("case-7"));
        assert_eq!(direct.run_id.as_deref(), Some("run-9"));
        assert_eq!(direct.driver.as_deref(), Some("longcat"));
        assert_eq!(direct.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(direct.task_pace.as_deref(), Some("deep"));
        assert_eq!(direct.max_hops, Some(48));
        assert_eq!(direct.deadline_secs, Some(900));
        assert_eq!(direct.tool_profile.as_deref(), Some("essential"));
        assert_eq!(direct.rollout_capture.as_deref(), Some("local"));
        assert!(direct.require_rollout);
        assert!(direct.require_rendered_output);
        assert_eq!(direct.prompt.as_deref(), Some("fix it"));

        let modifier = parse_task_args(
            "--task",
            ["--task-json", "--workspace", "/tmp/work", "fix it"]
                .into_iter()
                .map(str::to_string),
        )
        .unwrap();
        assert!(modifier.json);

        let ordinary = parse_task_args(
            "--task",
            ["--workspace", "/tmp/work", "fix it"]
                .into_iter()
                .map(str::to_string),
        )
        .unwrap();
        assert!(!ordinary.json);

        let flag_prompt = parse_task_args(
            "--task",
            ["--", "--literal-prompt"].into_iter().map(str::to_string),
        )
        .unwrap();
        assert_eq!(flag_prompt.prompt.as_deref(), Some("--literal-prompt"));
    }

    #[test]
    fn task_flags_reject_ambiguous_or_invalid_runner_controls() {
        let unknown = parse_task_args(
            "--task-json",
            ["--max-turns", "4", "fix it"]
                .into_iter()
                .map(str::to_string),
        )
        .unwrap_err();
        assert!(unknown.to_string().contains("unknown task option"));
        assert!(unknown.parsed.json);

        let in_turn_verifier = parse_task_args(
            "--task-json",
            ["--accept-cmd", "cargo test", "fix it"]
                .into_iter()
                .map(str::to_string),
        )
        .unwrap_err();
        assert!(
            in_turn_verifier
                .to_string()
                .contains("unknown task option: --accept-cmd")
        );

        let invalid_hops = parse_task_args(
            "--task",
            ["--max-hops", "many", "fix it"]
                .into_iter()
                .map(str::to_string),
        )
        .unwrap_err();
        assert!(invalid_hops.to_string().contains("non-negative integer"));

        let invalid_capture = parse_task_args(
            "--task",
            ["--rollout", "remote", "fix it"]
                .into_iter()
                .map(str::to_string),
        )
        .unwrap_err();
        assert!(
            invalid_capture
                .to_string()
                .contains("off, shadow, or local")
        );

        let invalid_pace = parse_task_args(
            "--task",
            ["--task-pace", "medium", "fix it"]
                .into_iter()
                .map(str::to_string),
        )
        .unwrap_err();
        assert!(invalid_pace.to_string().contains("auto, rapid, or deep"));

        let extra_prompt =
            parse_task_args("--task", ["fix", "it"].into_iter().map(str::to_string)).unwrap_err();
        assert!(extra_prompt.to_string().contains("one quoted argument"));

        let missing_capture = parse_task_args(
            "--task",
            ["--require-rollout", "fix it"]
                .into_iter()
                .map(str::to_string),
        )
        .unwrap_err();
        assert!(
            missing_capture
                .to_string()
                .contains("requires --rollout shadow")
        );

        let unsafe_effort = parse_task_args(
            "--task",
            ["--reasoning-effort", "high;rm", "fix it"]
                .into_iter()
                .map(str::to_string),
        )
        .unwrap_err();
        assert!(unsafe_effort.to_string().contains("short alphanumeric"));

        let duplicate_limit = parse_task_args(
            "--task-json",
            ["--max-hops", "8", "--max-hops", "16", "fix it"]
                .into_iter()
                .map(str::to_string),
        )
        .unwrap_err();
        assert!(duplicate_limit.to_string().contains("only once"));

        let unsafe_identity = parse_task_args(
            "--task-json",
            ["--run-id", "seed 1\nsecret", "fix it"]
                .into_iter()
                .map(str::to_string),
        )
        .unwrap_err();
        assert!(
            unsafe_identity
                .to_string()
                .contains("portable identity characters")
        );
    }

    #[test]
    fn task_pace_can_be_designated_or_inferred_without_submission_pressure() {
        let _guard = crate::tests::env_lock();
        let _requested = crate::tests::TestEnvGuard::unset("ANGEL_TASK_PACE");
        let _resolved = crate::tests::TestEnvGuard::unset("ANGEL_TASK_PACE_RESOLVED");

        let deep_prompt =
            resolve_task_pace("Treat this as a slow-burn major solve; do not submit yet.");
        assert_eq!(deep_prompt.pace, TaskPace::Deep);
        assert_eq!(deep_prompt.source, "prompt");

        let rapid_prompt = resolve_task_pace("Run a rapid-fire quick pass.");
        assert_eq!(rapid_prompt.pace, TaskPace::Rapid);
        assert_eq!(rapid_prompt.source, "prompt");

        // A long or unbounded budget is not a deep-pace request (operator law 2026-09-11).
        for (hops, seconds) in [("64", "3600"), ("0", "0")] {
            let _hops = crate::tests::TestEnvGuard::set("ANGEL_MAX_HOPS", hops);
            let _seconds = crate::tests::TestEnvGuard::set("ANGEL_TURN_DEADLINE_SECS", seconds);
            let pace = resolve_task_pace("Optimize this implementation.");
            assert_eq!(pace.pace, TaskPace::Rapid);
            assert_eq!(pace.source, "default");
        }

        let ordinary = resolve_task_pace("Fix this implementation.");
        assert_eq!(ordinary.pace, TaskPace::Rapid);
        assert_eq!(ordinary.source, "default");

        let _explicit = crate::tests::TestEnvGuard::set("ANGEL_TASK_PACE", "rapid");
        let explicit = resolve_task_pace("This is a slow burn.");
        assert_eq!(explicit.pace, TaskPace::Rapid);
        assert_eq!(explicit.source, "explicit");
    }

    #[test]
    fn task_hop_guard_is_only_enabled_by_an_explicit_positive_value() {
        assert_eq!(parse_task_max_hops(None), None);
        assert_eq!(parse_task_max_hops(Some("0")), None);
        assert_eq!(parse_task_max_hops(Some(" 0 ")), None);
        assert_eq!(parse_task_max_hops(Some("not-a-number")), None);
        assert_eq!(parse_task_max_hops(Some("17")), Some(17));
    }

    #[test]
    fn yolo_preserves_task_hops_and_bounded_coding_defaults() {
        let _guard = crate::tests::env_lock();
        const KEYS: [&str; 19] = [
            "ANGEL_YOLO",
            "ANGEL_MAX_HOPS",
            "ANGEL_TURN_DEADLINE_SECS",
            "ANGEL_TOOL_TIMEOUT",
            "ANGEL_TOOL_HARD_TIMEOUT",
            "ANGEL_FIRST_WRITE_CALLS",
            "ANGEL_FIRST_WRITE_REJECTIONS",
            "ANGEL_FINAL_MILE_HOPS",
            "ANGEL_FINAL_MILE_ANSWER_HOPS",
            "ANGEL_MUTATION_THRASH_NUDGE",
            "ANGEL_MUTATION_THRASH_STOP",
            "ANGEL_TASK_RECON",
            "ANGEL_TASK_CODING_DISCIPLINE",
            "ANGEL_RELENTLESS_EXECUTION",
            "ANGEL_TASK_TREEBEARD",
            "ANGEL_TASK_PACE",
            "ANGEL_TASK_PACE_RESOLVED",
            "ANGEL_TASK_PACE_SOURCE",
            "ANGEL_LANE",
        ];
        struct Restore([(&'static str, Option<std::ffi::OsString>); 19]);
        impl Drop for Restore {
            fn drop(&mut self) {
                for (key, value) in &self.0 {
                    match value {
                        // TODO: Audit that the environment access only happens in single-threaded code.
                        Some(value) => unsafe { std::env::set_var(key, value) },
                        // TODO: Audit that the environment access only happens in single-threaded code.
                        None => unsafe { std::env::remove_var(key) },
                    }
                }
            }
        }
        let _restore = Restore(KEYS.map(|key| (key, std::env::var_os(key))));
        for key in KEYS {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::remove_var(key) };
        }
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_YOLO", "1") };

        assert_eq!(configured_max_hops(), None);
        apply_task_runtime_defaults("");
        assert_eq!(configured_max_hops(), None);
        assert_eq!(std::env::var("ANGEL_TURN_DEADLINE_SECS").unwrap(), "0");
        assert!(std::env::var_os("ANGEL_FIRST_WRITE_CALLS").is_none());
        assert!(std::env::var_os("ANGEL_FIRST_WRITE_REJECTIONS").is_none());
        assert_eq!(std::env::var("ANGEL_FINAL_MILE_HOPS").unwrap(), "4");
        assert_eq!(std::env::var("ANGEL_FINAL_MILE_ANSWER_HOPS").unwrap(), "1");
        assert_eq!(std::env::var("ANGEL_MUTATION_THRASH_NUDGE").unwrap(), "3");
        assert_eq!(std::env::var("ANGEL_MUTATION_THRASH_STOP").unwrap(), "0");
        assert_eq!(std::env::var("ANGEL_TASK_RECON").unwrap(), "repo");
        assert_eq!(std::env::var("ANGEL_TASK_CODING_DISCIPLINE").unwrap(), "1");
        assert_eq!(std::env::var("ANGEL_RELENTLESS_EXECUTION").unwrap(), "1");
        assert_eq!(std::env::var("ANGEL_TASK_TREEBEARD").unwrap(), "auto");
        // The bounded 64-hop headless default still selects the long-horizon lane.
        assert_eq!(std::env::var("ANGEL_LANE").unwrap(), "treebeard");

        // The explicit individual off control still wins under YOLO.
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_MAX_HOPS", "0") };
        assert_eq!(configured_max_hops(), None);
    }

    #[test]
    fn task_treebeard_auto_enables_for_long_headless_when_lane_unset() {
        let _guard = crate::tests::env_lock();
        struct Restore(Vec<(&'static str, Option<std::ffi::OsString>)>);
        impl Drop for Restore {
            fn drop(&mut self) {
                for (key, value) in self.0.drain(..) {
                    match value {
                        // TODO: Audit that the environment access only happens in single-threaded code.
                        Some(value) => unsafe { std::env::set_var(key, value) },
                        // TODO: Audit that the environment access only happens in single-threaded code.
                        None => unsafe { std::env::remove_var(key) },
                    }
                }
            }
        }
        let keys = [
            "ANGEL_LANE",
            "ANGEL_TASK_TREEBEARD",
            "ANGEL_TASK_TREEBEARD_HOPS",
            "ANGEL_MAX_HOPS",
            "ANGEL_YOLO",
        ];
        let _restore = Restore(
            keys.into_iter()
                .map(|key| (key, std::env::var_os(key)))
                .collect(),
        );
        for key in keys {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::remove_var(key) };
        }
        // Explicit off: no treebeard (ReAct ablation control).
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_TASK_TREEBEARD", "0") };
        maybe_apply_task_treebeard_lane();
        assert_eq!(std::env::var("ANGEL_LANE").unwrap(), "default");

        // auto + unbounded default → treebeard.
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_LANE") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_TASK_TREEBEARD", "auto") };
        maybe_apply_task_treebeard_lane();
        assert_eq!(std::env::var("ANGEL_LANE").unwrap(), "treebeard");

        // Explicit lane wins.
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_LANE", "default") };
        maybe_apply_task_treebeard_lane();
        assert_eq!(std::env::var("ANGEL_LANE").unwrap(), "default");

        // Short hop budget under auto floor leaves the global Treebeard default
        // implicit rather than writing an environment override.
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_LANE") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_MAX_HOPS", "8") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_TASK_TREEBEARD", "auto") };
        maybe_apply_task_treebeard_lane();
        assert!(std::env::var_os("ANGEL_LANE").is_none());
        assert!(crate::harness::is_treebeard());

        // always forces regardless of hops.
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_TASK_TREEBEARD", "1") };
        maybe_apply_task_treebeard_lane();
        assert_eq!(std::env::var("ANGEL_LANE").unwrap(), "treebeard");
    }

    #[test]
    fn task_runtime_policy_defaults_are_headless_and_preserve_explicit_overrides() {
        let _guard = crate::tests::env_lock();
        const KEYS: [&str; 22] = [
            "ANGEL_MAX_HOPS",
            "ANGEL_TURN_DEADLINE_SECS",
            "ANGEL_TOOL_TIMEOUT",
            "ANGEL_TOOL_HARD_TIMEOUT",
            "ANGEL_TOOL_IDLE_FLOOR_SECS",
            "ANGEL_FIRST_WRITE_CALLS",
            "ANGEL_FIRST_WRITE_REJECTIONS",
            "ANGEL_FINAL_MILE_HOPS",
            "ANGEL_FINAL_MILE_ANSWER_HOPS",
            "ANGEL_MUTATION_THRASH_NUDGE",
            "ANGEL_MUTATION_THRASH_STOP",
            "ANGEL_PERIPHERAL_MUTATION_NUDGE",
            "ANGEL_POST_GREEN_TOOL_BATCHES",
            "ANGEL_NO_EDIT_ANSWER_GUARD",
            "ANGEL_TASK_RECON",
            "ANGEL_TASK_CODING_DISCIPLINE",
            "ANGEL_RELENTLESS_EXECUTION",
            "ANGEL_TASK_TREEBEARD",
            "ANGEL_TASK_PACE",
            "ANGEL_TASK_PACE_RESOLVED",
            "ANGEL_TASK_PACE_SOURCE",
            "ANGEL_LANE",
        ];
        struct Restore(Vec<(&'static str, Option<std::ffi::OsString>)>);
        impl Drop for Restore {
            fn drop(&mut self) {
                for (key, value) in self.0.drain(..) {
                    match value {
                        // TODO: Audit that the environment access only happens in single-threaded code.
                        Some(value) => unsafe { std::env::set_var(key, value) },
                        // TODO: Audit that the environment access only happens in single-threaded code.
                        None => unsafe { std::env::remove_var(key) },
                    }
                }
            }
        }
        let _restore = Restore(
            KEYS.into_iter()
                .map(|key| (key, std::env::var_os(key)))
                .collect(),
        );
        for key in KEYS {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::remove_var(key) };
        }

        apply_task_runtime_defaults("");
        assert_eq!(std::env::var("ANGEL_MAX_HOPS").unwrap(), "0");
        assert_eq!(std::env::var("ANGEL_TURN_DEADLINE_SECS").unwrap(), "0");
        // Tool ceilings follow the turn deadline: a benchmark-length verifier
        // must not die to the interactive 120 s default.
        assert!(std::env::var_os("ANGEL_TOOL_TIMEOUT").is_none());
        assert!(std::env::var_os("ANGEL_TOOL_HARD_TIMEOUT").is_none());
        assert!(std::env::var_os("ANGEL_TOOL_IDLE_FLOOR_SECS").is_none());
        assert!(std::env::var_os("ANGEL_FIRST_WRITE_CALLS").is_none());
        assert!(std::env::var_os("ANGEL_FIRST_WRITE_REJECTIONS").is_none());
        assert_eq!(std::env::var("ANGEL_FINAL_MILE_HOPS").unwrap(), "4");
        assert_eq!(std::env::var("ANGEL_FINAL_MILE_ANSWER_HOPS").unwrap(), "1");
        assert_eq!(std::env::var("ANGEL_MUTATION_THRASH_NUDGE").unwrap(), "3");
        assert_eq!(std::env::var("ANGEL_MUTATION_THRASH_STOP").unwrap(), "0");
        assert_eq!(
            std::env::var("ANGEL_PERIPHERAL_MUTATION_NUDGE").unwrap(),
            "4"
        );
        assert_eq!(std::env::var("ANGEL_POST_GREEN_TOOL_BATCHES").unwrap(), "0");
        assert_eq!(std::env::var("ANGEL_NO_EDIT_ANSWER_GUARD").unwrap(), "0");
        assert_eq!(std::env::var("ANGEL_TASK_RECON").unwrap(), "repo");
        assert_eq!(std::env::var("ANGEL_TASK_CODING_DISCIPLINE").unwrap(), "1");
        assert_eq!(std::env::var("ANGEL_RELENTLESS_EXECUTION").unwrap(), "1");
        assert_eq!(std::env::var("ANGEL_TASK_TREEBEARD").unwrap(), "auto");
        // The bounded long-horizon headless default → Treebeard lane.
        assert_eq!(std::env::var("ANGEL_LANE").unwrap(), "treebeard");

        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_FIRST_WRITE_CALLS", "9") };
        apply_task_runtime_defaults("");
        assert_eq!(std::env::var("ANGEL_FIRST_WRITE_CALLS").unwrap(), "9");

        // An explicit operator ceiling and a longer deadline are both honoured.
        for key in KEYS {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::remove_var(key) };
        }
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_TURN_DEADLINE_SECS", "3600") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_TOOL_TIMEOUT", "300") };
        apply_task_runtime_defaults("");
        assert_eq!(std::env::var("ANGEL_TOOL_TIMEOUT").unwrap(), "300");
        assert_eq!(std::env::var("ANGEL_TOOL_HARD_TIMEOUT").unwrap(), "3600");
        assert!(std::env::var_os("ANGEL_TOOL_IDLE_FLOOR_SECS").is_none());
        // An explicit operator floor remains opt-in and is preserved.
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_TOOL_IDLE_FLOOR_SECS", "45") };
        apply_task_runtime_defaults("");
        assert_eq!(std::env::var("ANGEL_TOOL_IDLE_FLOOR_SECS").unwrap(), "45");
    }

    #[test]
    fn coding_discipline_block_carries_action_ladder() {
        let _guard = crate::tests::env_lock();
        let prior = std::env::var_os("ANGEL_TASK_CODING_DISCIPLINE");
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_TASK_CODING_DISCIPLINE", "1") };
        let block = task_coding_discipline_block();
        assert!(block.contains("Action ladder"));
        assert!(block.contains("Map"));
        assert!(block.contains("Verify"));
        assert!(block.contains("implementing library"));
        assert!(block.contains("Treebeard"));
        assert!(block.contains("Batch independent"));
        assert!(block.contains("parallel"));
        assert!(block.contains("earliest actual prerequisite"));
        assert!(block.contains("usable input"));
        assert!(block.contains("zero performance"));
        assert!(block.contains("user-visible scope"));
        assert!(block.contains("never omit or auto-remove"));
        for word in ["budget", "deadline", "bounded horizon"] {
            assert!(!block.to_lowercase().contains(word));
            assert!(!task_pace_contract_block().to_lowercase().contains(word));
        }
        match prior {
            // TODO: Audit that the environment access only happens in single-threaded code.
            Some(v) => unsafe { std::env::set_var("ANGEL_TASK_CODING_DISCIPLINE", v) },
            // TODO: Audit that the environment access only happens in single-threaded code.
            None => unsafe { std::env::remove_var("ANGEL_TASK_CODING_DISCIPLINE") },
        }
    }

    #[test]
    fn task_stops_are_nonzero_by_default_with_explicit_legacy_override() {
        assert!(parse_task_strict_exit(None));
        assert!(parse_task_strict_exit(Some("unexpected")));
        assert!(!parse_task_strict_exit(Some("0")));
        assert!(!parse_task_strict_exit(Some("off")));
        assert!(!parse_task_strict_exit(Some("FALSE")));
    }

    #[test]
    fn task_parse_error_preserves_json_mode_and_prior_metadata() {
        let failure = parse_task_args(
            "--task",
            [
                "--task-json",
                "--workspace",
                "/tmp/work",
                "--task-id",
                "case-7",
                "--run-id",
            ]
            .into_iter()
            .map(str::to_string),
        )
        .unwrap_err();

        assert_eq!(failure.message, "--run-id requires a value");
        assert!(failure.parsed.json);
        assert_eq!(failure.parsed.workspace, Some(PathBuf::from("/tmp/work")));
        assert_eq!(failure.parsed.task_id.as_deref(), Some("case-7"));
    }

    /// Operator-ordered F01 contract: successful envelopes carry observational overruns.
    #[test]
    fn formation_budget_envelope_reports_over_allocation_without_stopping_answer() {
        let _lock = crate::tests::env_lock();
        let budget = super::super::formation_budget::Budget::new(Some(10), None);
        let _scope = super::super::formation_budget::enter(Some(budget.clone()));
        budget.reserve("coordinator", 20, 20).unwrap().settle(Some(
            crate::club::UsageObservation {
                raw: [Some(20), Some(20), Some(0), Some(0), Some(0)],
                contract: crate::club::UsageContract {
                    cache: crate::club::CacheConvention::Included,
                    reasoning: crate::club::ReasoningConvention::Included,
                },
                ..Default::default()
            },
        ));
        let envelope = TaskJsonEnvelope::new(
            TaskJsonContext {
                task_id: None,
                run_id: None,
                workspace: PathBuf::from("/w"),
                club: None,
                model: None,
                reasoning_effort: None,
                output_budget: None,
                elapsed_ms: 1,
                tools: Vec::new(),
                timing: None,
                usage: None,
                runtime: None,
                session_id: None,
                artifacts: Vec::new(),
                memory_health: crate::caddy::StoreHealthSummary::default(),
            },
            "completed",
            "answer",
            Some("complete answer".into()),
            None,
            1,
            false,
            false,
            false,
            None,
            None,
        );
        let value = serde_json::to_value(envelope).unwrap();
        assert_eq!(value["answer"], "complete answer");
        assert_eq!(value["stop_reason"], "answer");
        assert_eq!(value["over_allocation"], true);
        assert_eq!(value["over_allocation_tokens"], 30);
        assert_eq!(value["budget_exhausted"], true);
        assert_eq!(value["reservation_denied"], true);
        assert_eq!(value["formation_budget"]["remaining"], -30);
    }

    #[test]
    fn task_json_startup_failure_is_a_v1_error_envelope() {
        let args = TaskCliArgs {
            json: true,
            workspace: Some(PathBuf::from("/tmp/work")),
            task_id: Some("case-7".to_string()),
            run_id: Some("run-9".to_string()),
            prompt: None,
            ..TaskCliArgs::default()
        };
        let value = serde_json::to_value(TaskJsonEnvelope::from_startup_failure(
            &args,
            PathBuf::from("/tmp/work"),
            12,
            TaskStartupStopReason::InvalidArguments,
            "--workspace requires a directory".to_string(),
        ))
        .unwrap();

        assert_eq!(value["version"], 1);
        assert_eq!(value["kind"], "angel.task_result");
        assert_eq!(value["status"], "error");
        assert_eq!(value["stop_reason"], "invalid_arguments");
        assert_eq!(value["error"], "--workspace requires a directory");
        assert_eq!(value["workspace"], "/tmp/work");
        assert_eq!(value["task_id"], "case-7");
        assert_eq!(value["run_id"], "run-9");
        assert_eq!(value["hops"], 0);
        assert_eq!(value["interrupted"], false);
        assert_eq!(value["deadline_reached"], false);
        assert_eq!(value["max_hops_reached"], false);
        assert!(value.get("answer").is_none());
        assert!(value.get("club").is_none());
        assert!(value.get("model").is_none());
        assert!(value.get("output_budget").is_none());
    }

    #[test]
    fn task_output_budget_preserves_runtime_policy_and_provenance() {
        let explicit = RouteMetadata {
            output_budget: OutputBudgetPolicy::Explicit {
                tokens: 512,
                source: OutputBudgetSource::PerClubEnv,
            },
            output_budget_provenance: Some("ANGEL_LONGCAT_MAX_TOKENS".to_string()),
            ..RouteMetadata::default()
        };
        let value = serde_json::to_value(TaskOutputBudget::from_route_metadata(&explicit)).unwrap();
        assert_eq!(value["policy"], "explicit");
        assert_eq!(value["tokens"], 512);
        assert_eq!(value["source"], "per-club-env");
        assert_eq!(value["provenance"], "ANGEL_LONGCAT_MAX_TOKENS");

        let native = serde_json::to_value(TaskOutputBudget::from_route_metadata(
            &RouteMetadata::default(),
        ))
        .unwrap();
        assert_eq!(native["policy"], "provider-native");
        assert!(native.get("tokens").is_none());

        let managed = RouteMetadata {
            output_budget: OutputBudgetPolicy::EndpointManaged,
            ..RouteMetadata::default()
        };
        let managed =
            serde_json::to_value(TaskOutputBudget::from_route_metadata(&managed)).unwrap();
        assert_eq!(managed["policy"], "endpoint-managed");
        assert_eq!(managed["source"], "provider-plan");
    }

    #[test]
    fn task_json_serializes_truthful_stop_metadata_and_omits_unknowns() {
        let envelope = TaskJsonEnvelope::from_outcome(
            TaskJsonContext {
                task_id: Some("case-7".to_string()),
                run_id: None,
                workspace: PathBuf::from("/tmp/work"),
                club: Some("practice".to_string()),
                model: None,
                reasoning_effort: Some("high".to_string()),
                output_budget: Some(TaskOutputBudget {
                    policy: "explicit",
                    tokens: Some(4096),
                    source: Some("per-club-env"),
                    provenance: Some("ANGEL_PRACTICE_MAX_TOKENS".to_string()),
                }),
                elapsed_ms: 42,
                tools: Vec::new(),
                timing: None,
                usage: None,
                runtime: Some(TaskRuntimeConfig::new(
                    "b".repeat(64),
                    Some("practice".to_string()),
                    Some("high".to_string()),
                    TaskPaceResolution {
                        requested: "rapid".to_string(),
                        pace: TaskPace::Rapid,
                        source: "explicit".to_string(),
                    },
                    Some(64),
                    900,
                    "essential".to_string(),
                    "local".to_string(),
                    true,
                    false,
                )),
                session_id: None,
                artifacts: Vec::new(),
                memory_health: crate::caddy::StoreHealthSummary::default(),
            },
            TurnOutcome {
                stop_notice: None,
                reward_binding: Some(
                    super::super::rollout::RewardReceipt::new(
                        super::super::rollout::RewardOwner::CodingEval,
                        1.0,
                        "c".repeat(64),
                    )
                    .unwrap(),
                ),
                answer: "done".to_string(),
                stop_reason: TurnStopReason::Deadline,
                hops: 3,
                interrupted: true,
                deadline_reached: true,
                max_hops_reached: false,
                acceptance: Some(TaskAcceptanceTelemetry {
                    schema: "angel-task-acceptance/v1",
                    command_sha256: "a".repeat(64),
                    armed: true,
                    baseline_result: "failed",
                    baseline_passed: false,
                    baseline_ms: 9,
                    post_checks: 2,
                    post_ms: 17,
                    last_post_result: Some("passed"),
                    terminal_passed: true,
                    completion_source: "accept_cmd",
                    rendered_output: None,
                }),
                rollout_id: Some("rollout-fixture".to_string()),
                tools: Vec::new(),
                timing: None,
            },
            &[],
        );
        let value = serde_json::to_value(envelope).unwrap();
        assert_eq!(value["version"], 1);
        assert_eq!(value["kind"], "angel.task_result");
        assert_eq!(value["status"], "stopped");
        assert_eq!(value["stop_reason"], "deadline");
        assert_eq!(value["hops"], 3);
        assert_eq!(value["deadline_reached"], true);
        assert_eq!(value["acceptance"]["baseline_ms"], 9);
        assert_eq!(value["acceptance"]["post_checks"], 2);
        assert_eq!(value["acceptance"]["post_ms"], 17);
        assert_eq!(value["acceptance"]["schema"], "angel-task-acceptance/v1");
        assert_eq!(value["acceptance"]["baseline_passed"], false);
        assert_eq!(value["acceptance"]["armed"], true);
        assert_eq!(value["acceptance"]["baseline_result"], "failed");
        assert_eq!(value["acceptance"]["last_post_result"], "passed");
        assert_eq!(value["acceptance"]["terminal_passed"], true);
        assert_eq!(value["acceptance"]["completion_source"], "accept_cmd");
        assert_eq!(value["accepted"], true);
        assert_eq!(value["reward_binding"]["owner"], "coding_eval");
        assert_eq!(
            value["reward_binding"]["evaluator_evidence_sha256"],
            "c".repeat(64)
        );
        assert_eq!(value["rollout_id"], "rollout-fixture");
        assert_eq!(value["reasoning_effort"], "high");
        assert_eq!(value["runtime"]["schema"], "angel-task-runtime/v1");
        assert_eq!(value["runtime"]["max_hops"], 64);
        assert_eq!(value["runtime"]["deadline_secs"], 900);
        assert_eq!(value["runtime"]["verification_policy"], "external-only");
        assert_eq!(
            value["runtime"]["config_sha256"].as_str().unwrap().len(),
            64
        );
        assert_eq!(value["output_budget"]["policy"], "explicit");
        assert_eq!(value["output_budget"]["tokens"], 4096);
        assert_eq!(value["output_budget"]["source"], "per-club-env");
        assert_eq!(
            value["output_budget"]["provenance"],
            "ANGEL_PRACTICE_MAX_TOKENS"
        );
        assert!(value.get("run_id").is_none());
        assert!(value.get("model").is_none());
        assert!(value.get("usage").is_none());
        assert!(value.get("timing").is_none());
        assert!(value.get("session_id").is_none());
        assert!(value.get("artifacts").is_none());

        let failure = TaskJsonEnvelope::from_failure(
            TaskJsonContext {
                task_id: None,
                run_id: None,
                workspace: PathBuf::from("/tmp/work"),
                club: Some("practice".to_string()),
                model: None,
                reasoning_effort: None,
                output_budget: None,
                elapsed_ms: 1,
                tools: Vec::new(),
                timing: None,
                usage: None,
                runtime: None,
                session_id: None,
                artifacts: Vec::new(),
                memory_health: crate::caddy::StoreHealthSummary::default(),
            },
            TurnFailure {
                message: "guard reached".to_string(),
                stop_reason: TurnStopReason::MaxHops,
                hops: 4,
                interrupted: true,
                deadline_reached: false,
                max_hops_reached: true,
                acceptance: None,
                rollout_id: None,
            },
        );
        let value = serde_json::to_value(failure).unwrap();
        assert_eq!(value["status"], "stopped");
        assert_eq!(value["stop_reason"], "max_hops");
        assert_eq!(value["max_hops_reached"], true);
        assert!(value.get("answer").is_none());
        assert_eq!(value["error"], "guard reached");
    }

    fn passing_unit_build(rendered: Option<RenderedOutputAcceptance>) -> TaskAcceptanceTelemetry {
        TaskAcceptanceTelemetry {
            schema: "angel-task-acceptance/v1",
            command_sha256: "b".repeat(64),
            armed: true,
            baseline_result: "passed",
            baseline_passed: true,
            baseline_ms: 11,
            post_checks: 1,
            post_ms: 17,
            last_post_result: Some("passed"),
            terminal_passed: true,
            completion_source: "accept_cmd",
            rendered_output: rendered,
        }
    }

    fn serialize_acceptance(acceptance: TaskAcceptanceTelemetry) -> serde_json::Value {
        serde_json::to_value(TaskJsonEnvelope::from_outcome(
            TaskJsonContext {
                task_id: Some("t-render".to_string()),
                run_id: None,
                workspace: PathBuf::from("/tmp/work"),
                club: Some("practice".to_string()),
                model: None,
                reasoning_effort: None,
                output_budget: None,
                elapsed_ms: 12,
                tools: Vec::new(),
                timing: None,
                usage: None,
                runtime: None,
                session_id: None,
                artifacts: Vec::new(),
                memory_health: crate::caddy::StoreHealthSummary::default(),
            },
            TurnOutcome {
                stop_notice: None,
                reward_binding: None,
                answer: "ok".to_string(),
                stop_reason: TurnStopReason::Answer,
                hops: 3,
                interrupted: false,
                deadline_reached: false,
                max_hops_reached: false,
                acceptance: Some(acceptance),
                rollout_id: None,
                tools: Vec::new(),
                timing: None,
            },
            &[],
        ))
        .unwrap()
    }
    #[test]
    fn task_json_keeps_unit_build_distinct_from_rendered_output_acceptance() {
        let no_media = serialize_acceptance(passing_unit_build(None));
        assert!(no_media["acceptance"].get("rendered_output").is_none());
        assert_eq!(no_media["accepted"], true);

        let value = serialize_acceptance(passing_unit_build(Some(
            RenderedOutputAcceptance::unverified_from_external_only(),
        )));
        assert_eq!(value["acceptance"]["terminal_passed"], true);
        assert_eq!(
            value["acceptance"]["rendered_output"]["state"],
            "unverified"
        );
        assert_eq!(
            value["acceptance"]["rendered_output"]["limitation"],
            RenderedOutputAcceptance::EXTERNAL_LIMITATION
        );
        assert_eq!(value["accepted"], false);
    }

    #[test]
    fn public_task_json_preserves_unverified_rendered_without_accept_cmd() {
        use crate::club::Club;
        use crate::harness::{
            ChatMsg, TaskRolloutBindingV1, ToolRegistry, TurnStopReason, run_task_turn_observed,
        };
        use std::sync::atomic::AtomicBool;
        use std::sync::mpsc;

        struct FakePractice;
        impl Club for FakePractice {
            fn label(&self) -> &str {
                "practice"
            }
            fn respond(&self, _prompt: &str) -> Result<String, String> {
                Ok("done".into())
            }
        }

        let _lock = crate::tests::env_lock();
        let _accept = crate::tests::TestEnvGuard::unset("ANGEL_TASK_ACCEPT_CMD");
        let _recon = crate::tests::TestEnvGuard::set("ANGEL_TASK_RECON", "off");
        let _discipline = crate::tests::TestEnvGuard::set("ANGEL_TASK_CODING_DISCIPLINE", "0");
        let _map = crate::tests::TestEnvGuard::set("ANGEL_TASK_WORKSPACE_MAP", "0");
        let _verify = crate::tests::TestEnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
        let _skill = crate::tests::TestEnvGuard::set("ANGEL_SKILL_HINT", "0");
        let _advisor = crate::tests::TestEnvGuard::set("ANGEL_ADVISOR", "0");
        let _req = crate::tests::TestEnvGuard::set("ANGEL_TASK_RENDERED_REQUIREMENT", "1");

        let root =
            std::env::temp_dir().join(format!("angel-public-rendered-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let registry = ToolRegistry::with_team(root.clone(), Vec::new());
        let mut history = vec![ChatMsg::user("Say done when finished.")];
        let binding = TaskRolloutBindingV1::new(
            Some("public-rendered".into()),
            Some("run-1".into()),
            crate::cut::sha256_hex(b"Say done when finished."),
            crate::cut::sha256_hex(b"runtime"),
            "fixture".into(),
            crate::cut::sha256_hex(b"source"),
        );
        let (tx, _rx) = mpsc::channel();
        let outcome = run_task_turn_observed(
            &FakePractice,
            &registry,
            &mut history,
            &AtomicBool::new(false),
            Some(2),
            &tx,
            &binding,
            Some("practice"),
        )
        .expect("public task completes");
        assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
        assert!(outcome.acceptance.is_some());
        let envelope = TaskJsonEnvelope::from_outcome(
            TaskJsonContext {
                task_id: Some("public-rendered".into()),
                run_id: Some("run-1".into()),
                workspace: root.clone(),
                club: Some("practice".into()),
                model: None,
                reasoning_effort: None,
                output_budget: None,
                elapsed_ms: 1,
                tools: Vec::new(),
                timing: None,
                usage: None,
                runtime: None,
                session_id: None,
                artifacts: Vec::new(),
                memory_health: crate::caddy::StoreHealthSummary::default(),
            },
            outcome,
            &history,
        );
        let value = serde_json::to_value(&envelope).unwrap();
        assert_eq!(value["status"], "completed");
        assert!(value.get("acceptance").is_some());
        assert_eq!(
            value["acceptance"]["rendered_output"]["state"],
            "unverified"
        );
        assert_eq!(
            value["acceptance"]["rendered_output"]["limitation"],
            RenderedOutputAcceptance::EXTERNAL_LIMITATION
        );
        assert_ne!(value["acceptance"]["rendered_output"]["state"], "accepted");
        assert_eq!(value["accepted"], false);
        let _ = std::fs::remove_dir_all(&root);

        drop(_req);
        let _off = crate::tests::TestEnvGuard::unset("ANGEL_TASK_RENDERED_REQUIREMENT");
        let root_off =
            std::env::temp_dir().join(format!("angel-public-rendered-off-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root_off);
        std::fs::create_dir_all(&root_off).unwrap();
        let registry = ToolRegistry::with_team(root_off.clone(), Vec::new());
        let mut history = vec![ChatMsg::user("Say done when finished.")];
        let outcome = run_task_turn_observed(
            &FakePractice,
            &registry,
            &mut history,
            &AtomicBool::new(false),
            Some(2),
            &tx,
            &binding,
            Some("practice"),
        )
        .expect("ordinary public task completes");
        assert!(outcome.acceptance.is_none());
        let envelope = TaskJsonEnvelope::from_outcome(
            TaskJsonContext {
                task_id: Some("public-rendered-off".into()),
                run_id: Some("run-1".into()),
                workspace: root_off.clone(),
                club: Some("practice".into()),
                model: None,
                reasoning_effort: None,
                output_budget: None,
                elapsed_ms: 1,
                tools: Vec::new(),
                timing: None,
                usage: None,
                runtime: None,
                session_id: None,
                artifacts: Vec::new(),
                memory_health: crate::caddy::StoreHealthSummary::default(),
            },
            outcome,
            &history,
        );
        let value = serde_json::to_value(&envelope).unwrap();
        assert_eq!(value["status"], "completed");
        assert!(value.get("acceptance").is_none());
        assert!(value.get("accepted").is_none());
        let _ = std::fs::remove_dir_all(&root_off);
    }

    #[test]
    fn lifecycle_task_envelope_kill_reasons_preserve_recorded_answer() {
        let _lock = crate::tests::env_lock();
        for (stop, expected) in [
            ("interrupt", "cancelled"),
            ("deadline", "deadline"),
            ("idle_timeout", "tool_idle"),
            ("answer", "answer"),
        ] {
            let mut tool = serde_json::json!({"tool":"proc_run", "proc_id":17});
            crate::sandbox::process_owner::KillReceipt::new(Some(15), expected, "turn_owner")
                .apply(&mut tool);
            let ctx = TaskJsonContext {
                task_id: None,
                run_id: None,
                workspace: PathBuf::from("/w"),
                club: None,
                model: None,
                reasoning_effort: None,
                output_budget: None,
                elapsed_ms: 1,
                tools: vec![tool],
                timing: None,
                usage: None,
                runtime: None,
                session_id: None,
                artifacts: Vec::new(),
                memory_health: crate::caddy::StoreHealthSummary::default(),
            };
            let envelope = TaskJsonEnvelope::new(
                ctx,
                if stop == "answer" {
                    "completed"
                } else {
                    "stopped"
                },
                stop,
                None,
                None,
                1,
                stop == "interrupt",
                stop == "deadline",
                false,
                None,
                None,
            );
            let value = serde_json::to_value(envelope).unwrap();
            assert_eq!(value["stop_reason"], expected);
            assert_eq!(value["tools"][0]["status"], "killed");
            assert_eq!(value["tools"][0]["kill"]["signal"], 15);
            assert_eq!(value["tools"][0]["kill"]["owner"], "turn_owner");
        }
    }

    #[test]
    fn task_json_carries_the_tool_ledger_only_when_calls_ran() {
        let mut value = serde_json::to_value(TaskJsonEnvelope {
            stop_notice: None,
            authority_profile: crate::authority_profile::active(true),
            sandbox_profile: "ordinary",
            formation_budget: None,
            budget_exhausted: false,
            reservation_denied: false,
            over_allocation: false,
            over_allocation_tokens: None,
            reward_binding: None,
            identity: None,
            escalations: Vec::new(),
            hop_stream_cuts: Vec::new(),
            last_credited_progress: None,
            version: 1,
            kind: "task",
            task_id: None,
            run_id: None,
            workspace: "/w".to_string(),
            status: "completed",
            stop_reason: "answer",
            answer: Some("ok".to_string()),
            error: None,
            hops: 2,
            interrupted: false,
            deadline_reached: false,
            max_hops_reached: false,
            club: Some("c".to_string()),
            model: None,
            reasoning_effort: None,
            output_budget: None,
            elapsed_ms: 5,
            tools: Vec::new(),
            tools_output: Default::default(),
            store_rotations: Vec::new(),
            timing: None,
            acceptance: None,
            accepted: None,
            rollout_id: None,
            rollout_capture_errors: Vec::new(),
            runtime: None,
            usage: None,
            session_id: None,
            artifacts: Vec::new(),
            memory_health: crate::caddy::StoreHealthSummary::default(),
            graph_episode_id: None,
            graph_episodes: None,
            graph_episode_list: Vec::new(),
        })
        .unwrap();
        assert!(
            value.get("tools").is_none(),
            "empty ledger is omitted: {value}"
        );
        value["tools"] = serde_json::json!([{"hop": 1, "tool": "read_file", "exec": "ok", "err": false, "bytes": 3}]);
        let text = value.to_string();
        assert!(text.contains("\"tool\":\"read_file\""), "{text}");
    }

    #[test]
    fn task_timing_startup_failure_has_empty_request_samples() {
        let _guard = crate::tests::env_lock();
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                super::super::trajectory::clear_task_lifecycle();
            }
        }
        let _reset = Reset;
        super::super::trajectory::begin_task_lifecycle(std::time::Instant::now());
        let envelope = TaskJsonEnvelope::from_startup_failure(
            &TaskCliArgs::default(),
            PathBuf::from("."),
            0,
            TaskStartupStopReason::EmptyPrompt,
            "empty prompt".into(),
        );
        let timing = envelope.timing.unwrap();
        assert_eq!(timing.model_calls, 0);
        assert_eq!(timing.calls["model_calls"], serde_json::json!([]));
        assert_eq!(timing.wall_ms, timing.startup_ms);
    }

    #[test]
    fn task_json_serializes_timing_block_keys() {
        let envelope = TaskJsonEnvelope::from_outcome(
            TaskJsonContext {
                task_id: None,
                run_id: None,
                workspace: PathBuf::from("/tmp/work"),
                club: Some("practice".to_string()),
                model: None,
                reasoning_effort: None,
                output_budget: None,
                elapsed_ms: 1_000,
                tools: Vec::new(),
                timing: None,
                usage: None,
                runtime: None,
                session_id: None,
                artifacts: Vec::new(),
                memory_health: crate::caddy::StoreHealthSummary::default(),
            },
            TurnOutcome {
                stop_notice: None,
                reward_binding: None,
                answer: "done".to_string(),
                stop_reason: TurnStopReason::Answer,
                hops: 2,
                interrupted: false,
                deadline_reached: false,
                max_hops_reached: false,
                acceptance: None,
                rollout_id: None,
                tools: Vec::new(),
                timing: Some(TaskTimingTelemetry {
                    schema: "angel-task-timing/v2",
                    background: Default::default(),
                    model_ms: 600,
                    model_calls: 2,
                    model_retry_ms: 250,
                    model_retries: 1,
                    tool_ms: 300,
                    tool_calls: 3,
                    tool_errors: 1,
                    tool_max_ms: 250,
                    tool_max_name: Some("shell".to_string()),
                    other_ms: 100,
                    wall_ms: 1000,
                    envelope_wall_ms: None,
                    startup_shutdown_ms: None,
                    startup_ms: 0,
                    shutdown_ms: 0,
                    startup: serde_json::json!({}),
                    tool_overhead_ms: 0,
                    serial_overhead_ms: 0,
                    residual_ms: 100,
                    overlap_ms: 0,
                    spans: serde_json::json!({"turn_start":0,"turn_end":1000}),
                    calls: serde_json::json!({"model_calls":[]}),
                }),
            },
            &[],
        );
        let value = serde_json::to_value(envelope).unwrap();
        assert_eq!(value["timing"]["schema"], "angel-task-timing/v2");
        assert!(value["timing"]["background"].is_object());
        assert!(value["tools_output"].is_object());
        assert!(value["store_rotations"].is_array());
        assert_eq!(value["timing"]["model_ms"], 600);
        assert_eq!(value["timing"]["model_calls"], 2);
        assert_eq!(value["timing"]["model_retry_ms"], 250);
        assert_eq!(value["timing"]["model_retries"], 1);
        assert_eq!(value["timing"]["tool_ms"], 300);
        assert_eq!(value["timing"]["tool_calls"], 3);
        assert_eq!(value["timing"]["tool_errors"], 1);
        assert_eq!(value["timing"]["tool_max_ms"], 250);
        assert_eq!(value["timing"]["tool_max_name"], "shell");
        assert_eq!(value["timing"]["other_ms"], 100);
    }
    #[test]
    fn runtime_missing_task_envelope_is_structured_and_recovery_clears_it() {
        let _env = crate::tests::env_lock();
        let error = crate::tools::runtime_missing::RuntimeMissing::new("cargo", "PATH").encode();
        let mut tools = vec![serde_json::json!({
            "tool": "run_tests", "error": format!("tool error: {error}"),
        })];
        for recovered in [false, true] {
            if recovered {
                tools.push(serde_json::json!({"tool": "run_tests", "verify": "passed"}));
            }
            let envelope = TaskJsonEnvelope::new(
                TaskJsonContext {
                    task_id: None,
                    run_id: None,
                    workspace: PathBuf::from("/nonexistent"),
                    club: None,
                    model: None,
                    reasoning_effort: None,
                    output_budget: None,
                    elapsed_ms: 0,
                    tools: tools.clone(),
                    timing: None,
                    usage: None,
                    runtime: None,
                    session_id: None,
                    artifacts: Vec::new(),
                    memory_health: crate::caddy::StoreHealthSummary::default(),
                },
                "completed",
                "answer",
                Some("done".into()),
                None,
                2,
                false,
                false,
                false,
                None,
                None,
            );
            assert_eq!(envelope.stop_reason, "answer");
            if recovered {
                assert!(envelope.error.is_none());
                assert_eq!(envelope.status, "completed");
            } else {
                let error = envelope.error.unwrap();
                assert_eq!(error["kind"], "runtime_missing");
                assert_eq!(error["runtime"], "cargo");
                assert!(error["hint"].as_str().unwrap().contains("ANGEL_CARGO_BIN"));
                assert_eq!(envelope.status, "error");
            }
        }
    }
}
