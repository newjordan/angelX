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

/// The headless task's system prompt: its personality type (the full Driver,
/// or the compact core with `ANGEL_TASK_COMPACT_PROMPT`), the resolved pace
/// (`⠍⠁` / `⠍⠃`) and, with `ANGEL_TASK_CODING_DISCIPLINE`, the repair
/// discipline (`⠽⠑`). The workspace map follows in a message of its own.
pub(crate) fn task_system_prompt(workspace: &Path, specialists: &[String]) -> String {
    let pace = resolved_task_pace().map(|pace| match pace {
        TaskPace::Rapid => super::book::m_method::RAPID,
        TaskPace::Deep => super::book::m_method::DEEP,
    });
    super::book::y_types::entry(
        workspace,
        &super::delegate_roles(specialists),
        task_compact_prompt_enabled(),
        pace,
        super::env_flag("ANGEL_TASK_CODING_DISCIPLINE", false),
    )
}

/// The pace headless startup resolved, if this process is a task.
pub(crate) fn resolved_task_pace() -> Option<TaskPace> {
    std::env::var("ANGEL_TASK_PACE_RESOLVED")
        .ok()
        .as_deref()
        .and_then(parse_task_pace)
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
        // Free preturn map (repo_recon + symbol/lockfile packet) so paid hops
        // are not spent on first-pass reading (expensive reading fix).
        ("ANGEL_TASK_RECON", "repo"),
        // The coding repair discipline (`⠽⠑`) on the headless entry warpath.
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
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_TASK_PACE_RESOLVED", pace.pace.as_str()) };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_TASK_PACE_SOURCE", &pace.source) };
    maybe_apply_task_treebeard_lane();
    pace
}

/// A workspace of at most this many files is small enough that the map shows
/// the whole of it.
const SMALL_WORKSPACE_FILES: usize = 40;

/// Route a headless task in a small workspace onto the light start: no recon
/// packet, no tool bubble, and only the tools that read, write and run. The
/// map already names every file, so a search tool has nothing left to find.
/// Size is the signal because it is known before the first request and task
/// text is not: across the 136-task board, task length had a 0.01 correlation
/// with how many calls a task took. `read_file` returns plain text here: the
/// files are short enough to edit by their own words, and the line anchors
/// cost about 6% of a task's input. The same route turns on the measured
/// speed stack: the up-front read as context, a test run after a write-only
/// response, the stop at a full green run, the brevity block ahead of the
/// map, collapsed copies of a locked loop and one Cargo home per process
/// (DeepSeek Flash, 2026-11-11: 136/136 on three seeds; held-out tests 113 of
/// 133; Go 36/36). Must run before [`apply_task_runtime_defaults`]; a value
/// already in the environment wins, so `0` turns any of them off.
pub(crate) fn apply_small_workspace_route(workspace: &Path) {
    if !workspace_is_small(workspace) {
        return;
    }
    for (key, value) in [
        ("ANGEL_TASK_RECON", "off"),
        ("ANGEL_TOOL_BUBBLE", "0"),
        ("ANGEL_HASHLINE_ANCHORS", "0"),
        (SMALL_WORKSPACE_PREREAD_ENV, "1"),
        (super::registry::SMALL_WORKSPACE_ROUTE_ENV, "1"),
        (PREREAD_AS_CONTEXT_ENV, "1"),
        (PREREAD_SKIP_MANIFESTS_ENV, "1"),
        (TEST_AFTER_WRITE_ENV, "1"),
        (END_AT_GREEN_ENV, "1"),
        ("ANGEL_TASK_STABLE_FIRST", "1"),
        (super::book::l_loops::LOOP_COLLAPSE_ENV, "1"),
        (crate::agent::tools::build::CARGO_HOME_REUSE_ENV, "1"),
    ] {
        if std::env::var_os(key).is_none() {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var(key, value) };
        }
    }
}

/// On in a small workspace's headless task ([`apply_small_workspace_route`]);
/// `0` turns it off. After a response that only writes files, the
/// harness adds one `run_tests` call to that response, so the result of the
/// write and the result of the tests reach the model together. Measured on
/// small workspaces, where the model's next response is otherwise always the
/// test run (about 1.9 s and one full request on DeepSeek Flash, 2026-11-10).
pub(crate) const TEST_AFTER_WRITE_ENV: &str = "ANGEL_TASK_TEST_AFTER_WRITE";

/// See [`TEST_AFTER_WRITE_ENV`]. A response that also runs something is left
/// alone: the model has already chosen its own check.
pub(crate) fn append_test_after_write(calls: &mut Vec<crate::agent::club::ToolCall>, hop: usize) {
    if !super::env_flag(TEST_AFTER_WRITE_ENV, false) {
        return;
    }
    let is_write = |name: &str| {
        matches!(
            name,
            "write_file" | "str_replace" | "multi_edit" | "apply_patch"
        )
    };
    if calls.iter().any(|call| is_write(&call.name))
        && calls
            .iter()
            .all(|call| is_write(&call.name) || call.name == "read_file")
    {
        calls.push(crate::agent::club::ToolCall {
            id: format!("call_autotest_{hop}"),
            name: "run_tests".to_string(),
            args: serde_json::json!({}),
        });
    }
}

/// On in a small workspace's headless task, and only with
/// [`TEST_AFTER_WRITE_ENV`]; `0` turns it off. When the
/// harness-added test run passes in full, the turn stops there with the test
/// summary as its answer instead of asking the model for a closing message
/// (about 2.5 s and 240 output tokens of a 7 s easy task on DeepSeek Flash,
/// 2026-11-10). The stop goes through the same checks as a model's stop.
pub(crate) const END_AT_GREEN_ENV: &str = "ANGEL_TASK_END_AT_GREEN";

pub(crate) fn end_at_green() -> bool {
    super::env_flag(END_AT_GREEN_ENV, false) && super::env_flag(TEST_AFTER_WRITE_ENV, false)
}

/// The summary line of a test result in which at least one test passed and
/// none failed, was skipped or was ignored: `tests: 7 passed, 0 failed, 0 …`.
pub(crate) fn full_green_line(result: &str) -> Option<String> {
    let line = result.lines().next()?;
    let summary = line.split(" — ").next()?.trim();
    let counts = summary.strip_prefix("tests: ")?;
    let mut parts = counts.split(", ");
    let passed = parts.next()?.strip_suffix(" passed")?.parse::<u64>().ok()?;
    let rest: Vec<&str> = parts.collect();
    (passed > 0 && rest.len() == 2 && rest.iter().all(|part| part.starts_with("0 ")))
        .then(|| summary.to_string())
}

/// The answer of a turn that stopped at green: a completion notice written by
/// the harness, since no model message closes the turn. It says so, names the
/// files changed and carries the test summary (`tests: 7 passed, 0 failed, …`).
pub(crate) fn completion_notice(
    summary: &str,
    edited: &std::collections::BTreeSet<String>,
) -> String {
    const SHOWN: usize = 8;
    let mut notice = String::from("Task complete.");
    if !edited.is_empty() {
        let shown: Vec<&str> = edited.iter().take(SHOWN).map(String::as_str).collect();
        notice.push_str("\nChanged: ");
        notice.push_str(&shown.join(", "));
        if edited.len() > SHOWN {
            notice.push_str(&format!(" and {} more", edited.len() - SHOWN));
        }
    }
    let counts = summary.strip_prefix("tests: ").unwrap_or(summary);
    notice.push_str(&format!("\nTests: {counts} (run after the last edit)."));
    notice.push_str("\nReported by angelX; the model wrote no closing message.");
    notice
}

/// Set by [`apply_small_workspace_route`]; `0` leaves the reading to the model.
pub(crate) const SMALL_WORKSPACE_PREREAD_ENV: &str = "ANGEL_TASK_PREREAD";

/// A small workspace is read up front only when all of its files, as
/// `read_file` returns them, fit in this many bytes.
const SMALL_WORKSPACE_PREREAD_BYTES: usize = 32 * 1024;

/// Read a small workspace before the first request. The map names every file
/// and the model's first move is to read them, which costs a whole model round
/// trip (1.2 s of a 9.4 s easy task on DeepSeek Flash, 2026-11-09). The reads
/// go through the real `read_file` tool and enter the history as the calls the
/// model would have made, so the model sees nothing it could not have fetched
/// and no new prompt text. Licence and lock files are skipped, as are files
/// that are not text. A workspace that does not fit is left alone: half a
/// workspace would look like all of it.
pub(crate) fn preread_small_workspace(
    registry: &super::ToolRegistry,
    workspace: &Path,
    history: &mut Vec<crate::agent::club::ChatMsg>,
) {
    use crate::agent::club::{ChatMsg, ToolCall};
    if !super::env_flag(SMALL_WORKSPACE_PREREAD_ENV, false) {
        return;
    }
    let root = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    let mut calls = Vec::new();
    let mut results = Vec::new();
    let mut total = 0usize;
    for path in collect_workspace_rel_paths(&root, SMALL_WORKSPACE_FILES + 1) {
        if !preread_candidate(&root, &path) {
            continue;
        }
        let args = serde_json::json!({ "path": path });
        let Ok(mut text) = registry.dispatch("read_file", &args) else {
            continue;
        };
        if super::env_flag(PREREAD_AS_CONTEXT_ENV, false) {
            text = read_remaining_pages(registry, &path, text);
        }
        total += text.len();
        if total > SMALL_WORKSPACE_PREREAD_BYTES {
            return;
        }
        let id = format!("call_preread_{}", calls.len());
        calls.push(ToolCall {
            id: id.clone(),
            name: "read_file".to_string(),
            args,
        });
        results.push(ChatMsg::tool(id, text));
    }
    if calls.is_empty() {
        return;
    }
    if super::env_flag(PREREAD_AS_CONTEXT_ENV, false) {
        // The same `read_file` text, as data in the task's context: no
        // assistant turn the model did not write.
        let mut text = String::new();
        for (call, result) in calls.iter().zip(&results) {
            let path = call.args["path"].as_str().unwrap_or_default();
            text.push_str(&format!(
                "`{path}`\n```\n{}\n```\n\n",
                result.content.trim_end()
            ));
        }
        history.push(ChatMsg::user(text.trim_end().to_string()));
        return;
    }
    history.push(ChatMsg::assistant_calls_with_reasoning(calls, None));
    history.extend(results);
}

/// A file read up front arrives whole: `read_file` serves 200 lines a page, and
/// a longer file left at its first page costs the model a round trip to fetch
/// the rest (6 to 9 of 136 first calls on DeepSeek Flash, 2026-11-11, about
/// 1.5 s each). Follows the page's own `next offset` note, at most 20 pages.
fn read_remaining_pages(registry: &super::ToolRegistry, path: &str, first: String) -> String {
    let mut whole = String::new();
    let mut page = first;
    for _ in 0..20 {
        let Some((body, next)) = split_next_offset(&page) else {
            whole.push_str(&page);
            return whole;
        };
        whole.push_str(body);
        let args = serde_json::json!({ "path": path, "offset": next, "limit": 400 });
        match registry.dispatch("read_file", &args) {
            Ok(text) => page = text,
            Err(_) => return whole,
        }
    }
    whole.push_str(&page);
    whole
}

/// A page's text without its trailing continuation note, and the offset the
/// note names: `…[more content; next offset N]` (with its stamp line after it)
/// or `[truncated: … next offset N]`.
fn split_next_offset(page: &str) -> Option<(&str, u64)> {
    let at = page
        .rfind("[more content; next offset ")
        .map(|i| (i, "[more content; next offset "))
        .or_else(|| page.rfind("[truncated: ").map(|i| (i, "[truncated: ")))?;
    let (start, _) = at;
    let tail = &page[start..];
    let number = tail.split("next offset ").nth(1)?;
    let next: u64 = number
        .trim_start()
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()?;
    let body = page[..start].trim_end_matches('…');
    Some((body, next))
}

/// On in a small workspace's headless task; `0` turns it off. The up-front
/// read reaches the model as data in the
/// task's context (each file's path and its `read_file` text) instead of an
/// assistant turn of tool calls the model never wrote. Measured 2026-11-11 on
/// the ten hardest DeepSeek Flash tasks: missed runs had reasoning text on 6%
/// of their turns, solved runs on 61-81%; with the read sent as a silent
/// assistant turn, 13 of 59 runs missed, without it 1 of 102.
pub(crate) const PREREAD_AS_CONTEXT_ENV: &str = "ANGEL_TASK_PREREAD_AS_CONTEXT";

/// On in a small workspace's headless task; `0` turns it off. The up-front
/// read leaves out package manifests and
/// tool configuration (`package.json`, `Cargo.toml`, `babel.config.js`, …);
/// the map still names them. Measured 2026-11-11: with nothing read up front,
/// DeepSeek Flash opened `package.json` on 0 of 26 tasks, `babel.config.js` on
/// 0 of 26 and `Cargo.toml` on 0 of 4, while every one of them was read up
/// front and sent uncached in the first request. Build scripts that name the
/// targets (`CMakeLists.txt`, 9 of 10 opened) are still read.
pub(crate) const PREREAD_SKIP_MANIFESTS_ENV: &str = "ANGEL_TASK_PREREAD_SKIP_MANIFESTS";

/// Off unless set to `1`. The up-front read leaves out test files (a name
/// with `test` or `spec` in it, or a file under `test/` or `tests/`); the map
/// still names them and the harness test run still reports on them.
pub(crate) const PREREAD_SKIP_TESTS_ENV: &str = "ANGEL_TASK_PREREAD_SKIP_TESTS";

fn is_manifest_or_tool_config(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let stem = lower.split('.').next().unwrap_or(&lower);
    matches!(
        lower.as_str(),
        "package.json"
            | "cargo.toml"
            | "pyproject.toml"
            | "setup.cfg"
            | "go.mod"
            | "go.sum"
            | "tsconfig.json"
            | "jsconfig.json"
            | "deno.json"
            | "composer.json"
            | "gemfile"
    ) || matches!(
        stem,
        "babel" | "jest" | "vite" | "vitest" | "webpack" | "rollup" | "eslint" | "prettier"
    ) && (lower.contains(".config.") || lower.ends_with("rc.json") || lower.ends_with("rc.js"))
}

fn is_test_file(rel: &str) -> bool {
    let lower = rel.to_ascii_lowercase();
    let mut dirs: Vec<&str> = lower.split('/').collect();
    let name = dirs.pop().unwrap_or_default();
    dirs.iter()
        .any(|dir| matches!(*dir, "test" | "tests" | "spec" | "__tests__"))
        || name
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|word| matches!(word, "test" | "tests" | "spec" | "specs"))
}

/// A file worth reading up front: text that `read_file` returns whole, and
/// not a licence or a lock file.
fn preread_candidate(root: &Path, rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    let upper = name.to_ascii_uppercase();
    if ["LICENSE", "LICENCE", "COPYING", "NOTICE"]
        .iter()
        .any(|prefix| upper.starts_with(prefix))
        || upper.ends_with(".LOCK")
        || upper.ends_with("-LOCK.JSON")
        || upper.ends_with("-LOCK.YAML")
    {
        return false;
    }
    if super::env_flag(PREREAD_SKIP_MANIFESTS_ENV, false) && is_manifest_or_tool_config(name) {
        return false;
    }
    if super::env_flag(PREREAD_SKIP_TESTS_ENV, false) && is_test_file(rel) {
        return false;
    }
    // A file larger than the whole budget would arrive cut short.
    std::fs::read(root.join(rel)).is_ok_and(|bytes| {
        bytes.len() <= SMALL_WORKSPACE_PREREAD_BYTES && std::str::from_utf8(&bytes).is_ok()
    })
}

fn workspace_is_small(workspace: &Path) -> bool {
    let root = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    collect_workspace_rel_paths(&root, SMALL_WORKSPACE_FILES + 1).len() <= SMALL_WORKSPACE_FILES
}

/// Names the private cache root a headless task installed in place of the
/// shared `~/.cache`. [`crate::agent::sandbox::SandboxPolicy::permissive`]
/// reads it to stop granting the shared cache.
pub(crate) const TASK_CACHE_HOME_ENV: &str = "ANGEL_TASK_CACHE_HOME";

/// Give a headless task its own empty `XDG_CACHE_HOME`.
///
/// The shared `~/.cache` is a sandbox writable root, and every sandboxed launch
/// alias-scans its writable roots. Once a tool cache there hard-links files
/// to paths outside the grants (`~/.cache/mise` → `~/.local/share/mise/installs`,
/// 8,190 aliases on 2026-10-28), each launch takes the Bubblewrap alias path:
/// 0.93 s per process against 0.01 s for an empty root, paid by every shell
/// call, test run and post-write check. A private cache keeps that protection
/// and drops the cost. An operator `XDG_CACHE_HOME` that names some other
/// directory is kept; `ANGEL_TASK_PRIVATE_CACHE=0` keeps the shared cache.
pub(crate) fn install_private_task_cache() -> Option<PathBuf> {
    let dir = install_private_task_cache_in(&std::env::temp_dir())?;
    remove_at_exit(dir.clone());
    Some(dir)
}

pub(crate) fn install_private_task_cache_in(base: &Path) -> Option<PathBuf> {
    let off = std::env::var("ANGEL_TASK_PRIVATE_CACHE").is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "off" | "false" | "no"
        )
    });
    // A nested task inherits its parent's private cache; it never owns it.
    if off || std::env::var_os(TASK_CACHE_HOME_ENV).is_some_and(|value| !value.is_empty()) {
        return None;
    }
    let shared = PathBuf::from(std::env::var_os("HOME")?).join(".cache");
    if let Some(current) = std::env::var_os("XDG_CACHE_HOME").filter(|value| !value.is_empty()) {
        let current = PathBuf::from(current);
        let same = current == shared
            || matches!(
                (current.canonicalize(), shared.canonicalize()),
                (Ok(a), Ok(b)) if a == b
            );
        if !same {
            return None;
        }
    }
    let dir = base.join(format!("angel-task-cache-{}", std::process::id()));
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .ok()?;
    }
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe {
        std::env::set_var("XDG_CACHE_HOME", &dir);
        std::env::set_var(TASK_CACHE_HOME_ENV, &dir);
    }
    Some(dir)
}

/// Remove the private task cache when the process exits normally
/// (`std::process::exit` and a return from `main` both run `atexit`).
fn remove_at_exit(dir: PathBuf) {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    extern "C" fn remove() {
        if let Some(dir) = DIR.get() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
    if DIR.set(dir).is_ok() {
        unsafe { libc::atexit(remove) };
    }
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
    let mut block = String::new();
    // The workspace map is harness-derived (an absolute root and file names,
    // no repository text), so it stays in the System contract: inside the
    // JSON workspace-context carrier Flash read the escaped map next to the
    // `workspace` field and prefixed the directory name to its paths (two
    // read_file errors per js-duration-parser run on the 2026-09-07 merge);
    // in the System prompt it navigated cleanly (arena r4, 2026-08-10 note).
    block.push_str(&task_workspace_map_block(workspace));
    block
}

/// Headless tasks use the compact core (`⠽⠉` / `⠽⠙`) and the skill referral
/// unless explicitly disabled. Keep the bootstrap and standalone execution
/// prepend on the same flag parser.
pub(crate) fn task_compact_prompt_enabled() -> bool {
    super::env_flag("ANGEL_TASK_COMPACT_PROMPT", true)
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
    // `⠍⠓` frames the map: its labels and its note (stay inside the root) are
    // the ledger pages; the root and the inventory are the data.
    let mut body = format!(
        "\n{}\n`{}`\n",
        super::book::m_method::MAP_NOTE.cells(),
        root.display()
    );
    for path in &entries {
        body.push_str("- `");
        body.push_str(path);
        body.push_str("`\n");
    }
    if entries.len() >= max_entries {
        body.push_str("…\n");
    }
    if body.len() > max_bytes {
        // Keep the header + root line even if the list is long.
        let mut end = max_bytes;
        while end > 0 && !body.is_char_boundary(end) {
            end -= 1;
        }
        body.truncate(end);
        body.push_str("\n…\n");
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
                    OutputBudgetSource::ModelCard => "model-card",
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
    // A node-failure label names the node by id; an id that says "cancelled"
    // is still a node failure.
    if reason == "cancelled" || (!reason.starts_with("node_failed") && reason.contains("cancelled"))
    {
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
