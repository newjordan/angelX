//! Single-agent turn execution: `run_turn`. Everything the turn tells the
//! model goes through the book of behaviors (`harness/book/`) as a braille
//! warpath; this file gathers the facts and dispatches the work.
pub(crate) mod background;

mod classify;
mod competition;
mod dispatch;
mod reasoning;
pub(crate) mod research;
mod verify;

use super::*;

pub(crate) use classify::*;
pub(crate) use competition::*;
pub(crate) use dispatch::*;
pub(crate) use reasoning::*;
pub(crate) use verify::*;

/// The unproductive streak never ends a turn: it gives the model a notice.
/// Returns the hop count at which the notice fires (0 = never).
/// Unproductive hops at which the redirect is given (`⠇⠛⠉`): the 0.1.6 task
/// value of `ANGEL_UNPRODUCTIVE_STREAK_STOP`, whose stop is gone.
pub(crate) const UNPRODUCTIVE_REDIRECT_HOPS: usize = 16;

fn configured_unproductive_escalate(metered_sota: bool, competition: bool) -> usize {
    let _ = (metered_sota, competition);
    env_usize("ANGEL_UNPRODUCTIVE_STREAK_ESCALATE", 8)
}

/// Build the one compact telemetry snapshot written at each turn exit. A
/// constructor macro keeps the field mapping centralized without turning an
/// expanding metrics record into a long positional function interface.
macro_rules! turn_counters {
    (
        $markup_replies:expr_2021,
        $spin:expr_2021,
        $err_streak:expr_2021,
        $duplicate_inspection_results:expr_2021,
        $duplicate_inspection_bytes_saved:expr_2021,
        $actions:expr_2021 $(,)?
    ) => {{
        let actions: ActionCapsuleMetrics = $actions;
        crate::knowledge::experience::TurnCounters {
            markup_replies: $markup_replies,
            spin: $spin,
            err_streak: $err_streak,
            duplicate_inspection_results: $duplicate_inspection_results,
            duplicate_inspection_bytes_saved: $duplicate_inspection_bytes_saved,
            aged_inspection_results: 0,
            aged_inspection_bytes_saved: 0,
            eager_offload_results: 0,
            eager_offload_bytes_saved: 0,
            post_edit_diagnostic_attempts: 0,
            post_edit_diagnostic_findings: 0,
            post_edit_diagnostic_failures: 0,
            post_edit_diagnostic_paths_skipped: 0,
            post_edit_diagnostic_output_bytes: 0,
            post_edit_diagnostic_elapsed_ms: 0,
            tool_argument_shrinks: 0,
            tool_argument_strings_shrunk: 0,
            tool_argument_bytes_saved: 0,
            repeated_inspections: 0,
            code_mode_calls: 0,
            code_mode_nested_calls: 0,
            code_mode_nested_output_bytes: 0,
            code_mode_policy_rejections: 0,
            code_mode_recipe_calls: 0,
            task_recon_context_bytes: 0,
            code_mode_schema_tokens: 0,
            request_overflows: 0,
            cache_control_requests: 0,
            cache_read_input_tokens: 0,
            cache_write_input_tokens: 0,
            cache_read_accounting_responses: 0,
            cache_write_accounting_responses: 0,
            unverified_completion_claims: 0,
            discovered_tool_schema_failures: 0,
            skill_hints: 0,
            provider_truncation_retries: 0,
            provider_truncation_episodes: 0,
            provider_truncation_retained_partials: 0,
            provider_truncation_recoveries: 0,
            provider_truncation_failures: 0,
            action_operations: actions.operations,
            action_previews: actions.previews,
            action_denied: actions.denied,
            action_preflight_us: actions.preflight_us,
            action_approval_wait_ms: actions.approval_wait_ms,
            tool_errors_by_class: crate::knowledge::experience::ToolErrorClasses::default(),
            action_exec_ms: actions.action_exec_ms,
        }
    }};
}

pub fn run_turn(
    club: &dyn Club,
    registry: &ToolRegistry,
    history: &mut Vec<ChatMsg>,
    cancel: &AtomicBool,
    max_hops: Option<usize>,
    events: &mpsc::Sender<TurnEvent>,
) -> Result<String, String> {
    run_turn_observed(club, registry, history, cancel, max_hops, events)
        .map(|outcome| outcome.answer)
        .map_err(|failure| failure.message)
}

/// Machine-readable reason a turn stopped. The ordinary [`run_turn`] API keeps
/// returning only answer/error strings; headless evaluators use this metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TurnStopReason {
    Answer,
    Interrupt,
    IdleTimeout,
    Deadline,
    MaxHops,
    ExecutionBlocked,
    CaptureFailure,
    CheckpointFailure,
    ProviderError,
    NeedsPro,
}

impl TurnStopReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Answer => "answer",
            Self::Interrupt => "interrupt",
            Self::IdleTimeout => "idle_timeout",
            Self::Deadline => "deadline",
            Self::MaxHops => "max_hops",
            Self::ExecutionBlocked => "execution_blocked",
            Self::CaptureFailure => "capture_failure",
            Self::CheckpointFailure => "checkpoint_failure",
            Self::ProviderError => "provider_error",
            Self::NeedsPro => "needs_pro",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TurnOutcome {
    pub(crate) reward_binding: Option<super::rollout::RewardReceipt>,
    pub(crate) answer: String,
    pub(crate) stop_notice: Option<String>,
    pub(crate) stop_reason: TurnStopReason,
    pub(crate) hops: usize,
    pub(crate) interrupted: bool,
    pub(crate) deadline_reached: bool,
    pub(crate) max_hops_reached: bool,
    pub(crate) acceptance: Option<TaskAcceptanceTelemetry>,
    pub(crate) rollout_id: Option<String>,
    pub(crate) timing: Option<TaskTimingTelemetry>,
    /// One entry per dispatched tool call (hop, tool, exec, err, class, verify,
    /// ms, bytes) — the same ledger the trajectory record carries.
    pub(crate) tools: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct TaskAcceptanceTelemetry {
    pub(crate) schema: &'static str,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(crate) command_sha256: String,
    pub(crate) armed: bool,
    pub(crate) baseline_result: &'static str,
    pub(crate) baseline_passed: bool,
    pub(crate) baseline_ms: u128,
    pub(crate) post_checks: u64,
    pub(crate) post_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_post_result: Option<&'static str>,
    pub(crate) terminal_passed: bool,
    pub(crate) completion_source: &'static str,
    /// Present only when a media/rendered requirement is in force. Independent
    /// of `terminal_passed` (unit/build/accept_cmd evidence). Model prose and
    /// arbitrary repo JSON cannot promote this field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) rendered_output: Option<RenderedOutputAcceptance>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct RenderedOutputAcceptance {
    pub(crate) schema: &'static str,
    pub(crate) state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) limitation: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) checked_revision_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) current_revision_sha256: Option<String>,
}

impl TaskAcceptanceTelemetry {
    pub(crate) fn task_accepted(&self) -> bool {
        match &self.rendered_output {
            Some(rendered) => self.terminal_passed && rendered.state == "accepted",
            None => self.terminal_passed,
        }
    }
}

impl RenderedOutputAcceptance {
    pub(crate) const SCHEMA: &'static str = "angel-rendered-acceptance/v1";
    pub(crate) const EXTERNAL_LIMITATION: &'static str = "trusted external verification cannot distinguish rendered-output acceptance from unit/build success";

    pub(crate) fn unverified_from_external_only() -> Self {
        Self {
            schema: Self::SCHEMA,
            state: "unverified",
            limitation: Some(Self::EXTERNAL_LIMITATION),
            checked_revision_sha256: None,
            current_revision_sha256: None,
        }
    }

    pub(crate) fn bind_revision(
        mut self,
        checked: Option<String>,
        current: Option<String>,
    ) -> Self {
        if let (Some(checked_sha), Some(current_sha)) = (checked.as_deref(), current.as_deref())
            && checked_sha != current_sha
        {
            self.state = "stale";
        }
        self.checked_revision_sha256 = checked;
        self.current_revision_sha256 = current;
        self
    }
}

fn rendered_requirement_active() -> bool {
    env_flag("ANGEL_TASK_RENDERED_REQUIREMENT", false)
}

/// Wall-time split for one turn: how long the harness waited on the model vs
/// executed tools, with the remainder attributed to harness overhead
/// (compaction, recon, verification, waits). Rides the headless task receipt
/// so speed work can measure the split per run instead of guessing. Provider
/// retry backoff sleeps count as model wait; `model_retry_ms`/`model_retries`
/// split that retry slice out of `model_ms`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct TaskTimingTelemetry {
    pub(crate) schema: &'static str,
    pub(crate) model_ms: u128,
    pub(crate) model_calls: u64,
    pub(crate) model_retry_ms: u128,
    pub(crate) model_retries: u64,
    pub(crate) tool_ms: u128,
    pub(crate) tool_calls: u64,
    pub(crate) tool_errors: u64,
    pub(crate) tool_max_ms: u128,
    pub(crate) tool_max_name: Option<String>,
    pub(crate) background: background::BackgroundTelemetry,
    pub(crate) other_ms: u128,
    pub(crate) wall_ms: u128,
    pub(crate) envelope_wall_ms: Option<u128>,
    pub(crate) startup_shutdown_ms: Option<u128>,
    pub(crate) startup_ms: u128,
    pub(crate) shutdown_ms: u128,
    pub(crate) startup: serde_json::Value,
    pub(crate) tool_overhead_ms: u128,
    pub(crate) serial_overhead_ms: u128,
    pub(crate) residual_ms: u128,
    pub(crate) overlap_ms: u128,
    pub(crate) spans: serde_json::Value,
    /// Every model and provider call sample so far: the receipt's one part
    /// that grows with the turn. Shared, so the per-hop snapshots the ledger
    /// and the retained copy hold do not each deep-copy it.
    pub(crate) calls: std::sync::Arc<serde_json::Value>,
}

/// Running per-turn totals behind [`TaskTimingTelemetry`]. Batch wall time is
/// accumulated whole (parallel/segmented members are timed as one span so
/// concurrent calls never double-count); only serially dispatched calls are
/// timed individually for the longest-call figures.
#[derive(Default)]
pub(crate) struct TaskTimingAccumulator {
    background: background::Meter,
    model_ms: u128,
    model_calls: u64,
    model_retry_ms: u128,
    model_retries: u64,
    tool_ms: u128,
    tool_calls: u64,
    tool_errors: u64,
    tool_max_ms: u128,
    tool_max_name: Option<String>,
    tool_member_ms: u128,
    model_samples: Vec<serde_json::Value>,
    call_first_delta: Option<u128>,
    call_first_answer: Option<u128>,
    call_last_delta: Option<u128>,
    call_max_idle_ms: u128,
    call_start_ms: u128,
    retry_pending: bool,
    first_visible: Option<u128>,
    first_answer: Option<u128>,
    first_action: Option<u128>,
    last_tool_end: Option<u128>,
}

impl TaskTimingAccumulator {
    fn finish_with_history(&self, elapsed: u128, history: &[ChatMsg]) -> TaskTimingTelemetry {
        background::observe(history);
        self.finish(elapsed)
    }

    fn note_model_span(&mut self, start: u128, end: u128) {
        let id = self.model_samples.len();
        let retry_of = self.retry_pending.then(|| id.checked_sub(1)).flatten();
        self.model_samples.push(serde_json::json!({
            "id":id,"start":start,"end":end,"ms":end.saturating_sub(start),
            "retry_of":retry_of,
            "first_delta_ms":self.call_first_delta.map(|t| t.saturating_sub(start)),
            "first_visible_ms":self.call_first_delta.map(|t| t.saturating_sub(start)),
            "ttft_ms":self.call_first_delta.map(|t| t.saturating_sub(start)),
            "first_answer_token_ms":self.call_first_answer.map(|t| t.saturating_sub(start)),
            "stream_ms":self.call_first_delta.map(|t| end.saturating_sub(t)),
            "longest_silence_ms":self.call_max_idle_ms.max(end.saturating_sub(self.call_last_delta.unwrap_or(start))),
            "idle_max_ms":self.call_max_idle_ms.max(end.saturating_sub(self.call_last_delta.unwrap_or(start)))
        }));
        self.retry_pending = false;
    }

    // Turn spans use turn-relative milliseconds; model-call samples subtract
    // that call's start. Missing answer deltas stay null, including tool-only calls.
    fn note_answer(&mut self, elapsed: u128) {
        self.first_answer.get_or_insert(elapsed);
        self.call_first_answer.get_or_insert(elapsed);
        self.note_visible(elapsed);
    }

    fn note_visible(&mut self, elapsed: u128) {
        self.first_visible.get_or_insert(elapsed);
        self.call_first_delta.get_or_insert(elapsed);
        self.call_max_idle_ms = self
            .call_max_idle_ms
            .max(elapsed.saturating_sub(self.call_last_delta.unwrap_or(self.call_start_ms)));
        self.call_last_delta = Some(elapsed);
    }

    /// One completed await on the model (request send → final token). In-hop
    /// provider retries each count: every attempt is real model wait time.
    pub(crate) fn note_model_wait(&mut self, waited: Duration) {
        self.model_ms = self.model_ms.saturating_add(waited.as_millis());
        self.model_calls = self.model_calls.saturating_add(1);
    }

    /// One provider-retry backoff sleep between attempts. The wait belongs to
    /// the model (the turn is idle waiting on the provider to become
    /// retryable), so it lands in `model_ms` — with the retry slice and count
    /// split out so receipts can show waits caused by retries.
    pub(crate) fn note_model_retry_wait(&mut self, waited: Duration) {
        self.model_ms = self.model_ms.saturating_add(waited.as_millis());
        self.model_retry_ms = self.model_retry_ms.saturating_add(waited.as_millis());
        self.model_retries = self.model_retries.saturating_add(1);
        self.retry_pending = true;
    }

    /// One dispatched tool batch's wall time.
    pub(crate) fn note_tool_batch(&mut self, waited: Duration) {
        self.tool_ms = self.tool_ms.saturating_add(waited.as_millis());
    }

    /// One serially dispatched tool call's wall time: keeps the longest single
    /// call, with its name, alongside the batch totals.
    pub(crate) fn note_tool_call(&mut self, name: &str, waited: Duration) {
        let ms = waited.as_millis();
        if ms > self.tool_max_ms {
            self.tool_max_ms = ms;
            self.tool_max_name = Some(name.to_string());
        }
    }

    /// One tool result folded back into history: counts executed calls
    /// (denied/suppressed batches never started) and dispatch-level errors.
    pub(crate) fn note_tool_result(&mut self, executed: bool, errored: bool) {
        if executed {
            self.tool_calls = self.tool_calls.saturating_add(1);
        }
        if errored {
            self.tool_errors = self.tool_errors.saturating_add(1);
        }
    }

    /// Final receipt block. `other_ms` derives by saturating subtraction from
    /// the turn's own elapsed figure, so it can never underflow.
    pub(crate) fn finish(&self, turn_elapsed_ms: u128) -> TaskTimingTelemetry {
        let mut result = TaskTimingTelemetry {
            schema: "angel-task-timing/v2",
            background: self.background.snapshot(),
            model_ms: self.model_ms,
            model_calls: self.model_calls,
            model_retry_ms: self.model_retry_ms,
            model_retries: self.model_retries,
            tool_ms: self.tool_ms,
            tool_calls: self.tool_calls,
            tool_errors: self.tool_errors,
            tool_max_ms: self.tool_max_ms,
            tool_max_name: self.tool_max_name.clone(),
            wall_ms: turn_elapsed_ms,
            envelope_wall_ms: None,
            startup_shutdown_ms: None,
            startup_ms: 0,
            shutdown_ms: 0,
            startup: serde_json::json!({}),
            tool_overhead_ms: self.tool_ms.saturating_sub(self.tool_member_ms),
            serial_overhead_ms: 0,
            residual_ms: turn_elapsed_ms
                .saturating_sub(self.model_ms)
                .saturating_sub(self.tool_ms),
            overlap_ms: self
                .model_ms
                .saturating_add(self.tool_ms)
                .saturating_sub(turn_elapsed_ms),
            spans: serde_json::json!({"turn_start":0,
                "first_model_request":self.model_samples.first().map(|v| &v["start"]),
                "first_visible_output":self.first_visible,
                "first_visible_ms":self.first_visible,
                "ttft_ms":self.first_visible,
                "first_answer_token_ms":self.first_answer,
                "longest_silence_ms":self.call_max_idle_ms,
                "first_action":self.first_action,
                "last_tool_end":self.last_tool_end,"turn_end":turn_elapsed_ms}),
            calls: std::sync::Arc::new(serde_json::Value::Object(serde_json::Map::from_iter([
                (
                    "model_calls".to_string(),
                    serde_json::Value::Array(self.model_samples.clone()),
                ),
                (
                    "retry_backoff_ms".to_string(),
                    serde_json::json!(self.model_retry_ms),
                ),
                (
                    "provider_calls".to_string(),
                    serde_json::Value::Array(
                        crate::agent::harness::trajectory::provider_call_samples(),
                    ),
                ),
            ]))),
            other_ms: turn_elapsed_ms
                .saturating_sub(self.model_ms)
                .saturating_sub(self.tool_ms),
        };
        result.refresh_task_lifecycle();
        crate::agent::harness::trajectory::retain_task_timing(&result);
        result
    }
}

impl TaskTimingTelemetry {
    pub(crate) fn refresh_task_lifecycle(&mut self) {
        if let Some((wall, startup, shutdown, phases)) =
            crate::agent::harness::trajectory::task_lifecycle_snapshot()
        {
            self.wall_ms = wall;
            self.envelope_wall_ms = Some(wall);
            self.startup_ms = startup;
            self.shutdown_ms = shutdown;
            self.startup_shutdown_ms = Some(startup + shutdown);
            self.startup = phases;
            let attributed = startup + shutdown + self.model_ms + self.tool_ms;
            self.other_ms = wall.saturating_sub(attributed);
            self.residual_ms = self.other_ms;
            self.overlap_ms = attributed.saturating_sub(wall);
        }
    }
}

/// Wall attribution for concurrent tools, preserving raw durations separately.
/// Cumulative integer apportionment avoids per-member rounding drift.
fn tool_wall_shares(durations: &[u128], batch_ms: u128) -> Vec<u128> {
    let sum: u128 = durations.iter().sum();
    if sum <= batch_ms {
        return durations.to_vec();
    }
    let mut cumulative = 0;
    let mut previous = 0;
    durations
        .iter()
        .map(|ms| {
            cumulative += ms;
            let boundary = cumulative * batch_ms / sum;
            let share = boundary - previous;
            previous = boundary;
            share
        })
        .collect()
}

impl TurnOutcome {
    fn answer(answer: String, hops: usize) -> Self {
        Self {
            answer,
            stop_notice: None,
            stop_reason: TurnStopReason::Answer,
            hops,
            interrupted: false,
            deadline_reached: false,
            max_hops_reached: false,
            acceptance: None,
            rollout_id: None,
            tools: Vec::new(),
            timing: None,
            reward_binding: None,
        }
    }

    fn with_stop_notice(mut self, notice: String) -> Self {
        self.stop_notice = Some(notice);
        self
    }

    fn stopped(answer: String, stop_reason: TurnStopReason, hops: usize) -> Self {
        Self {
            answer,
            stop_notice: None,
            stop_reason,
            hops,
            interrupted: true,
            deadline_reached: stop_reason == TurnStopReason::Deadline,
            max_hops_reached: stop_reason == TurnStopReason::MaxHops,
            acceptance: None,
            rollout_id: None,
            tools: Vec::new(),
            timing: None,
            reward_binding: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TurnFailure {
    pub(crate) message: String,
    pub(crate) stop_reason: TurnStopReason,
    pub(crate) hops: usize,
    pub(crate) interrupted: bool,
    pub(crate) deadline_reached: bool,
    pub(crate) max_hops_reached: bool,
    pub(crate) acceptance: Option<Box<TaskAcceptanceTelemetry>>,
    pub(crate) rollout_id: Option<String>,
}

#[allow(clippy::too_many_arguments)]
fn task_acceptance_snapshot(
    command: Option<&str>,
    armed: bool,
    baseline_result: Option<&'static str>,
    baseline_ms: u128,
    post_checks: u64,
    post_ms: u128,
    last_post_result: Option<&'static str>,
    completion_source: &'static str,
    checked_revision_sha256: Option<String>,
    current_revision_sha256: impl FnOnce() -> Option<String>,
) -> Option<TaskAcceptanceTelemetry> {
    let rendered_output = rendered_requirement_active().then(|| {
        RenderedOutputAcceptance::unverified_from_external_only()
            .bind_revision(checked_revision_sha256, current_revision_sha256())
    });
    if command.is_none() && rendered_output.is_none() {
        return None;
    }
    Some(TaskAcceptanceTelemetry {
        schema: "angel-task-acceptance/v1",
        command_sha256: command
            .map(|command| crate::knowledge::cut::sha256_hex(command.as_bytes()))
            .unwrap_or_default(),
        armed,
        baseline_result: baseline_result.unwrap_or("not_run"),
        baseline_passed: baseline_result == Some("passed"),
        baseline_ms,
        post_checks,
        post_ms,
        last_post_result,
        terminal_passed: last_post_result == Some("passed"),
        completion_source,
        rendered_output,
    })
}

/// [`run_turn`] with truthful stop metadata for non-interactive evaluators.
pub(crate) fn run_turn_observed(
    club: &dyn Club,
    registry: &ToolRegistry,
    history: &mut Vec<ChatMsg>,
    cancel: &AtomicBool,
    max_hops: Option<usize>,
    events: &mpsc::Sender<TurnEvent>,
) -> Result<TurnOutcome, TurnFailure> {
    run_turn_steered_observed(
        club, registry, history, cancel, max_hops, events, None, None, None,
    )
}

/// A native delegate owns its task identity; it must not inherit a parent
/// task's environment binding or discard the child's terminal rollout ID.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_delegate_turn_observed(
    club: &dyn Club,
    registry: &ToolRegistry,
    history: &mut Vec<ChatMsg>,
    cancel: &AtomicBool,
    max_hops: Option<usize>,
    events: &mpsc::Sender<TurnEvent>,
    task_binding: &TaskRolloutBindingV1,
    checkpoint: &dyn Fn(&[ChatMsg]) -> Result<(), String>,
) -> Result<TurnOutcome, TurnFailure> {
    run_turn_steered_observed(
        club,
        registry,
        history,
        cancel,
        max_hops,
        events,
        None,
        Some(TaskCaptureContext {
            binding: task_binding,
            requested_driver: None,
        }),
        Some(checkpoint),
    )
}

#[derive(Clone, Copy)]
struct TaskCaptureContext<'a> {
    binding: &'a TaskRolloutBindingV1,
    requested_driver: Option<&'a str>,
}

/// Headless tasks have no App::advance consumer for proc_run completions: a
/// finished job reaches the model as a `⠏` warpath at the hop boundary, its
/// receipt in the ledger. Returns whether any failed or ended without a known
/// exit (`⠏⠉`); a clean exit never buys another model hop.
fn deliver_job_completions(
    registry: &ToolRegistry,
    history: &mut Vec<ChatMsg>,
    events: &mpsc::Sender<TurnEvent>,
) -> bool {
    let finished = crate::agent::tools::proc::take_completions(
        &registry.workspace_boundary().canonical_root,
        8,
    );
    if finished.is_empty() {
        return false;
    }
    for job in &finished {
        let _ = events.send(TurnEvent::Notice(job.task_message()));
    }
    history.push(ChatMsg::harness(format!(
        "{}\n{}",
        book::warpath(
            registry.current_workspace(),
            &book::p_processes::completions(&finished),
        ),
        book::p_processes::receipt_lines(&finished),
    )));
    book::p_processes::any_failed(&finished)
}

/// Headless task turn with an immutable task-to-rollout identity binding.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_task_turn_observed(
    club: &dyn Club,
    registry: &ToolRegistry,
    history: &mut Vec<ChatMsg>,
    cancel: &AtomicBool,
    max_hops: Option<usize>,
    events: &mpsc::Sender<TurnEvent>,
    task_binding: &TaskRolloutBindingV1,
    requested_driver: Option<&str>,
) -> Result<TurnOutcome, TurnFailure> {
    // The headless task contract delegates scoring to its external evaluator.
    // In-task Cut checks remain evidence, but cannot claim this rollout's label.
    let _external_reward_owner = EvalLabelScope::new();
    run_turn_steered_observed(
        club,
        registry,
        history,
        cancel,
        max_hops,
        events,
        None,
        Some(TaskCaptureContext {
            binding: task_binding,
            requested_driver,
        }),
        None,
    )
}

/// [`run_turn`] with a mid-run steer queue: user notes typed while the turn is
/// in flight are drained at each hop boundary and injected as framed user
/// messages, so the model sees the guidance in its next request without the
/// turn being interrupted (see [`crate::agent::steer`]). `None` — sub-agents, bounded
/// delegate tasks, `--task` mode — behaves exactly as before.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub fn run_turn_steered(
    club: &dyn Club,
    registry: &ToolRegistry,
    history: &mut Vec<ChatMsg>,
    cancel: &AtomicBool,
    max_hops: Option<usize>,
    events: &mpsc::Sender<TurnEvent>,
    steers: Option<&crate::agent::steer::SteerQueue>,
) -> Result<String, String> {
    run_turn_steered_observed(
        club, registry, history, cancel, max_hops, events, steers, None, None,
    )
    .map(|outcome| outcome.answer)
    .map_err(|failure| failure.message)
}

/// Interactive turn variant that durably records the exact model request and
/// model-emitted tool intent immediately before each external dispatch. A
/// failed checkpoint stops before the provider or any tool in that batch can
/// execute.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub(crate) fn run_turn_steered_checkpointed(
    club: &dyn Club,
    registry: &ToolRegistry,
    history: &mut Vec<ChatMsg>,
    cancel: &AtomicBool,
    max_hops: Option<usize>,
    events: &mpsc::Sender<TurnEvent>,
    steers: Option<&crate::agent::steer::SteerQueue>,
    checkpoint: &dyn Fn(&[ChatMsg]) -> Result<(), String>,
) -> Result<String, String> {
    run_turn_steered_observed(
        club,
        registry,
        history,
        cancel,
        max_hops,
        events,
        steers,
        None,
        Some(checkpoint),
    )
    .map(|outcome| outcome.answer)
    .map_err(|failure| failure.message)
}

/// Checkpointed interactive turn with the machine-readable stop reason intact.
/// Long-running controllers use this to distinguish a normal answer from a
/// policy boundary such as a deadline, hop ceiling, or pro-tier request.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_turn_steered_checkpointed_observed(
    club: &dyn Club,
    registry: &ToolRegistry,
    history: &mut Vec<ChatMsg>,
    cancel: &AtomicBool,
    max_hops: Option<usize>,
    events: &mpsc::Sender<TurnEvent>,
    steers: Option<&crate::agent::steer::SteerQueue>,
    checkpoint: &dyn Fn(&[ChatMsg]) -> Result<(), String>,
) -> Result<TurnOutcome, TurnFailure> {
    run_turn_steered_observed(
        club,
        registry,
        history,
        cancel,
        max_hops,
        events,
        steers,
        None,
        Some(checkpoint),
    )
}

/// One user turn, with the model self-report escalation ladder around it.
///
/// Unarmed (`ANGEL_NEEDS_PRO` unset, or armed with no usable seat) this is a
/// single [`NeedsProTier::Off`] run and nothing about the turn changes. Armed
/// with a resolvable seat, the fast attempt carries the escalation contract; if
/// the model leads its reply with the marker that attempt is discarded — no
/// answer surfaced, nothing about it left in history — and the same turn is
/// re-run once on the named seat. Exactly one escalation per user turn: the
/// second run is served at [`NeedsProTier::Pro`], where the marker is stripped
/// rather than obeyed, so the ladder cannot cycle.
type HistoryCheckpoint<'a> = &'a dyn Fn(&[ChatMsg]) -> Result<(), String>;

#[allow(clippy::too_many_arguments)]
fn run_turn_steered_observed(
    club: &dyn Club,
    registry: &ToolRegistry,
    history: &mut Vec<ChatMsg>,
    cancel: &AtomicBool,
    max_hops: Option<usize>,
    events: &mpsc::Sender<TurnEvent>,
    steers: Option<&crate::agent::steer::SteerQueue>,
    task_capture: Option<TaskCaptureContext<'_>>,
    history_checkpoint: Option<HistoryCheckpoint<'_>>,
) -> Result<TurnOutcome, TurnFailure> {
    let _phase = crate::agent::turn::phase::Scope::enter();
    crate::agent::turn::phase::mark("harness_start");
    let effective_cancel = AtomicBool::new(cancel.load(Ordering::Acquire));
    // Keep nested turn/process observations attached to the operator's turn.
    let _owner_link = super::exec::link_child_owner(&effective_cancel, cancel);
    let done = AtomicBool::new(false);
    let timed_out = AtomicBool::new(false);
    let idle = crate::agent::turn::configured_turn_idle_timeout_secs().map(Duration::from_secs);
    let owner = &effective_cancel as *const _ as usize;
    let (relay_tx, relay_rx) = mpsc::channel();
    std::thread::scope(|scope| {
        let (effective_cancel, done, timed_out) = (&effective_cancel, &done, &timed_out);
        let watcher = scope.spawn(move || {
            let mut last_progress = Instant::now();
            let mut tool_active = false;
            while !done.load(Ordering::Acquire) {
                match relay_rx.recv_timeout(Duration::from_millis(20)) {
                    Ok(event) => {
                        match &event {
                            TurnEvent::ToolCall { .. } => tool_active = true,
                            TurnEvent::ToolResult { .. } => tool_active = false,
                            _ => {}
                        }
                        last_progress = Instant::now();
                        let _ = events.send(event);
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                // Defer the idle clock during bounded helper setup, separately
                // from child CPU/output progress. This does not credit the ledger.
                if super::exec::owned_child_active(owner)
                    || super::exec::owned_child_setting_up(owner)
                {
                    last_progress = Instant::now();
                }
                if !effective_cancel.load(Ordering::Acquire)
                    && idle.is_some_and(|limit| {
                        // Let a due tool-idle escalation settle its owned child
                        // and return a recoverable failure before turn cancellation.
                        let grace = if tool_active
                            && super::exec::tool_idle_timeout()
                                .is_some_and(|tool_limit| tool_limit <= limit)
                        {
                            Duration::from_secs(3)
                        } else {
                            Duration::ZERO
                        };
                        last_progress.elapsed() >= limit.saturating_add(grace)
                    })
                {
                    timed_out.store(true, Ordering::Release);
                    effective_cancel.store(true, Ordering::Release);
                }
                if cancel.load(Ordering::Acquire) {
                    effective_cancel.store(true, Ordering::Release);
                }
                if effective_cancel.load(Ordering::Acquire) {
                    crate::agent::tools::proc::stop_owned_for(
                        owner,
                        if timed_out.load(Ordering::Acquire) {
                            "tool_idle"
                        } else {
                            "cancelled"
                        },
                    );
                }
            }
            for event in relay_rx.try_iter() {
                let _ = events.send(event);
            }
        });
        struct Finish<'a>(&'a AtomicBool, usize);
        impl Drop for Finish<'_> {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
                if std::thread::panicking() {
                    crate::agent::tools::proc::stop_owned(self.1);
                }
            }
        }
        let finish = Finish(done, owner);
        let mut result = run_turn_steered_observed_inner(
            club,
            registry,
            history,
            effective_cancel,
            max_hops,
            &relay_tx,
            steers,
            task_capture,
            history_checkpoint,
        );
        let reason = if timed_out.load(Ordering::Acquire) {
            Some("tool_idle")
        } else if cancel.load(Ordering::Acquire) {
            Some("cancelled")
        } else if matches!(&result, Ok(outcome) if outcome.stop_reason == TurnStopReason::Deadline)
            || matches!(&result, Err(failure) if failure.stop_reason == TurnStopReason::Deadline)
        {
            Some("deadline")
        } else if matches!(&result, Ok(outcome) if outcome.stop_reason == TurnStopReason::Answer) {
            None
        } else {
            Some("owner_reap")
        };
        if let Some(reason) = reason {
            crate::agent::tools::proc::stop_owned_for(owner, reason);
        }
        drop(finish);
        drop(relay_tx);
        let _ = watcher.join();
        let kills = crate::agent::tools::proc::take_owned_kills(owner);
        crate::agent::harness::trajectory::note_proc_kills(&kills);
        // A cancelled provider wait can return an error rather than an outcome.
        // The process kill remains the authoritative cancellation receipt.
        if kills.iter().any(|(_, kill)| kill.reason == "cancelled") {
            match &mut result {
                Ok(outcome) => {
                    outcome.stop_reason = TurnStopReason::Interrupt;
                    outcome.interrupted = true;
                }
                Err(failure) => {
                    failure.stop_reason = TurnStopReason::Interrupt;
                    failure.interrupted = true;
                }
            }
        }
        if let Ok(outcome) = &mut result {
            outcome.tools = crate::agent::harness::trajectory::tool_ledger_snapshot();
        }
        if timed_out.load(Ordering::Acquire) {
            return match result {
                Ok(mut outcome) => {
                    outcome.stop_reason = TurnStopReason::IdleTimeout;
                    outcome.answer = format!(
                        "turn idle timeout ({}s without progress)",
                        idle.unwrap().as_secs()
                    );
                    Ok(outcome)
                }
                Err(mut failure) => {
                    failure.stop_reason = TurnStopReason::IdleTimeout;
                    failure.message = format!(
                        "turn idle timeout ({}s without progress): {}",
                        idle.unwrap().as_secs(),
                        failure.message
                    );
                    Err(failure)
                }
            };
        }
        result
    })
}

#[allow(clippy::too_many_arguments)]
fn run_turn_steered_observed_inner(
    club: &dyn Club,
    registry: &ToolRegistry,
    history: &mut Vec<ChatMsg>,
    cancel: &AtomicBool,
    max_hops: Option<usize>,
    events: &mpsc::Sender<TurnEvent>,
    steers: Option<&crate::agent::steer::SteerQueue>,
    task_capture: Option<TaskCaptureContext<'_>>,
    history_checkpoint: Option<HistoryCheckpoint<'_>>,
) -> Result<TurnOutcome, TurnFailure> {
    // One cumulative allowance spans the whole root turn, including a
    // needs-pro rerun. Seat threads install this same handle before recursively
    // entering `run_turn`, so nested fan-out cannot mint a fresh budget.
    let _descendant_budget = DescendantBudgetScope::enter_root();
    crate::agent::turn::phase::mark("seat_resolution");
    let Some(pro) = resolve_seat(club, registry, history, events) else {
        return run_turn_tiered(
            club,
            registry,
            history,
            cancel,
            max_hops,
            events,
            steers,
            NeedsProTier::Off,
            task_capture,
            history_checkpoint,
        );
    };
    let fast = run_turn_tiered(
        club,
        registry,
        history,
        cancel,
        max_hops,
        events,
        steers,
        NeedsProTier::Fast,
        task_capture,
        history_checkpoint,
    );
    let Ok(outcome) = &fast else { return fast };
    if outcome.stop_reason != TurnStopReason::NeedsPro {
        return fast;
    }
    let reason = outcome.answer.trim();
    let reason = if reason.is_empty() {
        "no reason given"
    } else {
        reason
    };
    let _ = events.send(TurnEvent::Notice(format!(
        "needs-pro: escalating to {} — {reason}",
        pro.label()
    )));
    run_turn_tiered(
        pro.as_ref(),
        registry,
        history,
        cancel,
        max_hops,
        events,
        steers,
        NeedsProTier::Pro,
        task_capture,
        history_checkpoint,
    )
}

fn publish_slot_telemetry(
    events: &mpsc::Sender<TurnEvent>,
    watcher: &SubmissionWatcher,
    competition: bool,
    published: &mut Option<SubmissionSlotTelemetry>,
) {
    let next = watcher.telemetry(competition);
    if published.as_ref() == Some(&next) {
        return;
    }
    let _ = events.send(TurnEvent::SubmissionSlot(next.clone()));
    *published = Some(next);
}

#[allow(clippy::too_many_arguments)]
fn run_turn_tiered(
    club: &dyn Club,
    registry: &ToolRegistry,
    history: &mut Vec<ChatMsg>,
    cancel: &AtomicBool,
    max_hops: Option<usize>,
    events: &mpsc::Sender<TurnEvent>,
    steers: Option<&crate::agent::steer::SteerQueue>,
    tier: NeedsProTier,
    task_capture: Option<TaskCaptureContext<'_>>,
    history_checkpoint: Option<HistoryCheckpoint<'_>>,
) -> Result<TurnOutcome, TurnFailure> {
    // YOLO bypasses approvals, confinement, and effect restrictions. It never
    // changes the caller's behavioral completion/runaway policy: an operator
    // asking for unrestricted machine access has not asked the model to ignore
    // hop, progress, verification, or deadline contracts.
    let _live_model = super::run_identity::LiveModelScope::enter(None);
    let _live_turn =
        super::run_identity::LiveTurnScope::enter(super::run_identity::live_turn().or_else(|| {
            Some(format!(
                "turn:{}:{:?}",
                registry.session_id,
                std::time::SystemTime::now()
            ))
        }));
    crate::agent::turn::phase::mark("workspace_identity");
    let workspace_state_started = Instant::now();
    let _workspace_start =
        crate::agent::harness::trajectory::WorkspaceStartScope::enter(registry.current_workspace());
    if let Some(notice) = _workspace_start.partial_notice() {
        let _ = events.send(TurnEvent::Notice(notice.to_string()));
    }
    super::trajectory::note_task_startup_phase(
        "workspace_state_ms",
        workspace_state_started.elapsed().as_millis(),
    );
    crate::agent::turn::phase::mark("formation_budget");
    let _formation_budget_scope =
        super::formation_budget::start_turn().map_err(|message| TurnFailure {
            message,
            stop_reason: TurnStopReason::ProviderError,
            hops: 0,
            interrupted: false,
            deadline_reached: false,
            max_hops_reached: false,
            acceptance: None,
            rollout_id: None,
        })?;
    if let Some(budget) = super::formation_budget::current()
        && !club.supports_formation_budget()
    {
        budget.note_untracked_provider();
    }
    crate::knowledge::experience::note_turn_workspace(registry.current_workspace());
    let rollout_required = env_flag("ANGEL_HARNESS_ROLLOUT_REQUIRED", false);
    let mut rollout_capture_error: Option<String> = None;
    let auxiliary_scope = registry.auxiliary.enter();
    let mut consumed_recovery_context = std::collections::HashSet::new();
    if matches!(tier, NeedsProTier::Pro) {
        registry.auxiliary.utility_entered("prior_policy_turn");
    }
    crate::agent::turn::phase::mark("rollout_start");
    let mut rollout_recorder = match RolloutRecorder::from_env(
        registry.current_workspace(),
        || {
            let mut route = club.route_identity();
            // The task envelope binds the caller's driver selector, which may
            // differ from the club's display label (e.g. glm vs glm-5.3-flash).
            if let Some(driver) = task_capture.and_then(|capture| capture.requested_driver) {
                route.driver = driver.to_string();
            }
            route
        },
        task_capture.map(|capture| capture.binding),
    ) {
        Ok(recorder) => recorder,
        Err(error) => {
            if rollout_required {
                return Err(TurnFailure {
                    message: format!("required rollout capture could not start: {error}"),
                    stop_reason: TurnStopReason::CaptureFailure,
                    hops: 0,
                    interrupted: false,
                    deadline_reached: false,
                    max_hops_reached: false,
                    acceptance: None,
                    rollout_id: None,
                });
            }
            let _ = events.send(TurnEvent::RolloutCaptureError(format!(
                "rollout capture unavailable; turn continues without it: {error}"
            )));
            RolloutRecorder::off()
        }
    };
    if rollout_required && rollout_recorder.rollout_id().is_none() {
        return Err(TurnFailure {
            message: "required rollout capture is off; select shadow or local capture".to_string(),
            stop_reason: TurnStopReason::CaptureFailure,
            hops: 0,
            interrupted: false,
            deadline_reached: false,
            max_hops_reached: false,
            acceptance: None,
            rollout_id: None,
        });
    }
    // New turn → clear the per-turn approval cache (approve-all decisions from a
    // previous turn don't carry over). Inert unless an approval gate fires.
    crate::agent::approval::reset_turn();
    // Tier contract for the self-report ladder. Installed into the leading
    // system run (never the tail) as a fixed string, so it stays on the cached
    // prefix; `Off` — the default — touches nothing.
    match tier {
        NeedsProTier::Off => {}
        NeedsProTier::Fast => install_contract(history, FAST_TIER_CONTRACT),
        NeedsProTier::Pro => install_contract(history, PRO_TIER_CONTRACT),
    }
    // Size the advertised tool schemas to the in-hand model's window. A small
    // local model (e.g. an 8K llama.cpp) can't afford every full schema + the
    // system prompt — the request overflows on schemas alone, before any
    // conversation, and compaction can't help (it only trims history). On a tight
    // window we advertise just the essential read/edit/shell/search/memory loop;
    // the rest stay dispatchable by name. Probes the window once (cached).
    crate::agent::turn::phase::mark("model_metadata");
    let ctx_window = club.metadata().map(|m| m.context_window).filter(|&c| c > 0);
    // Schema lean is additional: local unbounded interactive keeps the full
    // advertised set. Metered roots and bounded / competition / comp-mode use
    // the compact core, with a sticky hidden-tool bubble chosen once here.
    crate::agent::turn::phase::mark("context_assembly");
    let competition_trigger = competition_mode_trigger(history);
    let competition = competition_trigger.is_some();
    let metered_sota = crate::agent::club::is_sota_label(club.label());
    let streak_escalate = configured_unproductive_escalate(metered_sota, competition);
    let research_turn = !competition && research::selected(history);
    let research_origin = research_turn.then(|| research::origin(history)).flatten();
    if research_turn {
        let contract = [book::s_sources::contract(research_origin.as_deref())];
        history.push(ChatMsg::harness(book::warpath(
            registry.current_workspace(),
            &contract,
        )));
    }
    let turn_budget = configured_turn_deadline_secs_for(competition);
    let task_pace = configured_task_pace(history);
    // A byte-exact provider prefix cache renders the tool schemas ahead of the
    // whole conversation, so a tool set that changes at a new user turn
    // re-reads everything uncached (DeepSeek: every /loop iteration and every
    // follow-up, measured 2026-09-29). On such a seat a turn keeps the
    // activations earlier turns made, and the bubble is seeded once per
    // session: later turns add a tool only when the model searches for one.
    // DeepSeek's own harness keeps its catalogue fixed for the cache.
    let prefix_cached = club.prompt_cache_capable();
    if !prefix_cached {
        registry.reset_tool_activations();
    }
    let bubble = if metered_sota && !(prefix_cached && registry.has_tool_activations()) {
        let task = history
            .iter()
            .rev()
            .find(|message| message.role == ChatRole::User)
            .map(|message| message.content.as_ref())
            .unwrap_or_default();
        registry.seed_tool_bubble(task)
    } else {
        ToolBubbleDecision {
            source: ToolBubbleSource::Disabled,
            tools: Vec::new(),
        }
    };
    if env_flag("ANGEL_DEBUG_TOOL_BUBBLE", false) {
        let _ = events.send(TurnEvent::Notice(format!(
            "tool bubble: {} [{}]",
            bubble.source.as_str(),
            bubble.tools.join(", ")
        )));
    }
    let mut defs =
        registry.defs_for_driver_turn(ctx_window, max_hops.is_some(), competition, metered_sota);
    if research_turn {
        research::ensure_tools(&mut defs, registry);
    }
    research::describe_surface(&mut defs, research_origin.as_deref());
    let leading_system_count = history
        .iter()
        .take_while(|message| message.role == ChatRole::System)
        .count();
    let system_prompt_tokens = estimate_tokens(&history[..leading_system_count]);
    let project_doc_bytes = history
        .iter()
        .take_while(|message| crate::app::bootstrap::is_pinned_preamble(message))
        .find_map(crate::app::bootstrap::project_doc_bytes)
        .unwrap_or(0);
    let tool_schema_count = defs.len();
    let tool_schema_tokens = estimate_tool_tokens(&defs);
    let code_mode_schema_tokens = defs
        .iter()
        .find(|definition| definition.name == "code_mode")
        .map(|definition| estimate_tool_tokens(std::slice::from_ref(definition)))
        .unwrap_or(0);
    let tool_schema_peak_count = std::cell::Cell::new(tool_schema_count);
    let tool_schema_peak_tokens = std::cell::Cell::new(tool_schema_tokens);
    let tool_schema_token_requests = std::cell::Cell::new(0usize);
    let discovered_tool_schema_failures = std::cell::Cell::new(0usize);
    let skill_hints = registry
        .gauge
        .pending_skill_hints
        .swap(0, Ordering::Relaxed);
    // Whether the language server is wired this session (ANGEL_LSP) — gates the
    // post-edit self-correct below. Computed once: a name lookup, no LSP spawn.
    let lsp_postcheck =
        env_flag("ANGEL_POST_EDIT_DIAGNOSTICS", true) && registry.has_tool("lsp_diagnostics");
    let mut hop: usize = 0;
    let mut last_answer_route = club.route_identity();
    let mut book_continuity = book::continuity::Continuity::new(history);
    let task = history
        .iter()
        .rev()
        .find(|message| message.role == ChatRole::User)
        .map(|message| message.content.as_ref())
        .unwrap_or_default();
    let (research_task, research_verify) = registry.rl().research_objective(task);
    let mut live_research = crate::drive::rl_ctl::research_live::LiveResearch::new(
        registry.current_workspace(),
        &research_task,
        research_verify.as_deref(),
        &registry.session_id,
        events.clone(),
    );
    let task_active = std::env::var("ANGEL_TASK_ACTIVE").is_ok_and(|value| value == "1");
    // The book's detectors for this turn (`harness/book/`).
    let mut loops = book::l_loops::Loops::from_env();
    // Tool-call scavenging (opt-in): recover a call the model stranded in its
    // answer text with an empty structured `tool_calls` array.
    let toolcall_scavenge = env_flag("ANGEL_TOOLCALL_SCAVENGE", false);
    // History hygiene: opt-in hard cap on conversation length so an unbounded
    // multi-day loop can't grow the request body without bound. 0 = unbounded.
    let history_cap = env_usize("ANGEL_HISTORY_MAX_MSGS", 0);
    // Rolling tool-aging and deduplication cadence under cache-stable mode.
    // Flushes held inspection rewrites periodically so long turns never balloon
    // into 100k+ token request bodies.
    // A provider that publishes its own harness's compaction policy (DeepSeek)
    // keeps history append-only between compactions: its harness prunes tool
    // results only once compaction qualifies, and a held rewrite re-reads the
    // whole prefix uncached (39,530 and 44,011 tokens in a live DeepSeek loop,
    // 2026-09-29). An operator's cadence still wins.
    let rolling_rewrite_hops = if std::env::var_os("ANGEL_ROLLING_REWRITE_HOPS").is_none()
        && club.provider_compaction_budget().is_some()
    {
        0
    } else {
        env_usize("ANGEL_ROLLING_REWRITE_HOPS", 12)
    };
    // Auto-compaction: target budget above which the oldest turns are summarized
    // (not just evicted). 0 means "use the built-in 333k policy" unless
    // ANGEL_NO_AUTOCOMPACT disables it. Protect a token-sized recent tail by
    // default: message counts are a poor cost proxy for tool-heavy coding turns.
    // The token target is capped to half the active budget so small-window models
    // always leave a useful compactable region. Setting it to 0 restores the
    // legacy message-count policy.
    let context_budget = env_usize("ANGEL_CONTEXT_BUDGET_TOKENS", 0);
    let compact_keep = env_usize("ANGEL_COMPACT_KEEP_RECENT", 12).max(2);
    let compact_keep_token_target = env_usize("ANGEL_COMPACT_KEEP_RECENT_TOKENS", 20_000);
    // Resolve the *effective* budget once per turn: an explicit env value wins,
    // otherwise budget against the in-hand club's real context window (probed
    // from the backend, cached). This is what makes compaction fire at the
    // model's true limit instead of a guess. Probe happens here, once.
    let mut effective_budget = compaction_budget(club, context_budget);
    let mut compact_keep_tokens = if compact_keep_token_target == 0 {
        0
    } else {
        compact_keep_token_target.min(effective_budget / 2)
    };
    // Diagnostic (opt-in via ANGEL_DEBUG_CTX=1): surface what the in-hand club's
    // context window was detected as, and the resulting compaction budget, so the
    // truthful-budget wiring is verifiable live in one message.
    if std::env::var("ANGEL_DEBUG_CTX")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        let win = club
            .metadata()
            .map(|m| m.context_window)
            .filter(|&c| c > 0)
            .map(|c| c.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let _ = events.send(TurnEvent::Notice(format!(
            "ctx: club={} window={win} budget={effective_budget}",
            club.label()
        )));
    }
    // Auto-recall: on the first turn against a live palace, prime the context with
    // the most relevant long-term notes for this project — so a resumed (or new)
    // session starts already aware of past decisions instead of waiting for the
    // model to think to call `recall`. Once per run (`ANGEL_AUTO_RECALL=0` off).
    let cache_stable = cache_stable_mode(club);
    crate::agent::turn::phase::mark("knowledge_broker");
    refresh_knowledge_broker_with_prefix(registry, history, effective_budget, &defs, cache_stable);
    crate::agent::turn::phase::mark("auto_recall");
    maybe_auto_recall(registry, history, effective_budget, &defs, events);
    // Lifecycle hooks (PreToolUse/PostToolUse), loaded once from config. Empty
    // unless ~/.angelX/hooks.json exists, so default dispatch is unchanged.
    crate::agent::turn::phase::mark("hooks_load");
    let hooks = Hooks::load();
    // This reads a small local mode once, never calls a model, and only becomes
    // active for the interactive root registry.
    let action_capsule_mode = mode_for(registry);
    // Wall-clock turn budget: a safety net for long *unattended* runs (deli/swarm
    // research, automated drivers) so a runaway multi-hop turn can't grind for
    // hours unnoticed. 0 = off for ordinary chat; competition turns default to a
    // hard wall so agents cannot "think" for a day without submitting.
    // Checked at hop boundaries and mirrored into the cancellation token for
    // each blocking provider/tool operation. Cooperative implementations such
    // as streaming HTTP and the process-owning shell therefore stop at the
    // same wall instead of overrunning it until their independent timeout.
    let turn_start = std::time::Instant::now();
    // Carry the already configured turn wall into nested graph/provider calls.
    let _request_wall = (turn_budget > 0).then(|| {
        super::formation_budget::RequestWallScope::enter(
            turn_start + std::time::Duration::from_secs(turn_budget as u64),
        )
    });
    // Wall-time split for the headless task receipt: model waits vs tool
    // execution vs harness overhead, accumulated across the whole hop loop and
    // finalized once at the turn's exit seam. Scoped to this tier's own turn
    // (`turn_start`), so a needs-pro rerun's receipt covers the pro hop loop.
    let mut timing = TaskTimingAccumulator::default();
    let _background_scope = timing.background.enter(turn_start);
    let turn_deadline = (turn_budget > 0)
        .then(|| turn_start.checked_add(Duration::from_secs(turn_budget as u64)))
        .flatten();
    crate::agent::harness::trajectory::reset_first_action();
    crate::agent::turn::phase::mark("trajectory_reset");
    crate::agent::harness::trajectory::reset_turn_ledger(club);
    crate::agent::harness::trajectory::set_research_turn(research_turn);
    crate::agent::harness::trajectory::note_timing_origin(turn_start);
    crate::agent::harness::trajectory::note_turn_session(&registry.session_id);
    if let Some(trigger) = competition_trigger {
        // Once per competition turn: engage the loop at the resolved pace
        // (`⠅⠁…` rapid, `⠅⠃…` deep). Competition awareness and rapid
        // submission cadence are separate contracts; deep work must never
        // inherit the latter merely because the standing goal mentions a
        // leaderboard.
        let engage = book::warpath(
            registry.current_workspace(),
            &book::k_competition::engage(task_pace),
        );
        let already = history
            .iter()
            .any(|m| m.role == ChatRole::Harness && m.content.as_ref() == engage);
        if !already {
            history.push(ChatMsg::harness(engage));
            let policy = match task_pace {
                TaskPace::Rapid => {
                    "mutate + local preflight → explicit submission contract → improve next candidate"
                }
                TaskPace::Deep => {
                    "research + targeted experiments → evidence-ready candidate; submit when the local gate passes"
                }
            };
            let _ = events.send(TurnEvent::Notice(format!(
                "competition {} pace armed (trigger: {trigger}) — {policy}",
                task_pace.as_str()
            )));
        }
    }
    let mut slot_watcher = SubmissionWatcher::new();
    let mut watch_source: Option<ConfiguredWatchSource> = None;
    match crate::agent::harness::comp_packages::active_package().configured_watch() {
        Ok(Some((id, source))) => {
            slot_watcher.adopt(&id);
            watch_source = Some(source);
        }
        Ok(None) => {}
        Err(error) => {
            let _ = events.send(TurnEvent::Notice(error));
        }
    }
    let mut published_slot_telemetry = None;
    publish_slot_telemetry(
        events,
        &slot_watcher,
        competition,
        &mut published_slot_telemetry,
    );
    let mut preflight_seen_this_turn = false;
    let time_to_first_mutation_ms = std::cell::Cell::new(None::<u64>);
    let time_to_green_ms = std::cell::Cell::new(None::<u64>);
    // Hops where every call errored at dispatch (`⠭⠁` on the third).
    let mut errors = book::x_execution::ErrorStreak::default();
    let preturn_code_mode = registry.take_preturn_code_mode_metrics();
    let code_mode_calls = std::cell::Cell::new(preturn_code_mode.calls);
    let code_mode_recipe_calls = std::cell::Cell::new(preturn_code_mode.recipe_calls);
    let code_mode_nested_calls = std::cell::Cell::new(preturn_code_mode.nested_calls);
    let code_mode_nested_output_bytes = std::cell::Cell::new(preturn_code_mode.nested_output_bytes);
    let code_mode_policy_rejections = std::cell::Cell::new(preturn_code_mode.policy_rejections);
    let task_recon_context_bytes = preturn_code_mode.task_recon_context_bytes;
    let mut duplicate_inspection_results = 0usize;
    let mut duplicate_inspection_bytes_saved = 0u64;
    let aged_inspection_results = std::cell::Cell::new(0usize);
    let aged_inspection_bytes_saved = std::cell::Cell::new(0u64);
    // Cache-stable mode (default-on, see `cache_stable_mode`): tool-result aging and
    // tool-argument shrinking both rewrite history in place, which invalidates a
    // byte-exact provider prefix cache from that message on. Under the mode both
    // passes are held back so this turn's prefix only ever grows, and they run
    // only at a compaction boundary instead.
    // Hops whose deferred rewrites are still held; any boundary that actually
    // rewrote history resets it, so the count never claims flushed hops.
    let deferred_rewrite_hops = std::cell::Cell::new(0usize);
    let eager_offload_results = std::cell::Cell::new(0usize);
    let eager_offload_bytes_saved = std::cell::Cell::new(0u64);
    let post_edit_diagnostics = PostEditDiagnosticCounters {
        enabled: lsp_postcheck,
        ..PostEditDiagnosticCounters::default()
    };
    let tool_argument_shrinks = std::cell::Cell::new(0usize);
    let tool_argument_strings_shrunk = std::cell::Cell::new(0usize);
    let tool_argument_bytes_saved = std::cell::Cell::new(0u64);
    let adaptive_reasoning = env_flag("ANGEL_ADAPTIVE_REASONING", false);
    // An explicit operator policy may demote locked GLM seats after a reasoning
    // budget. By default sustained thought keeps the selected effort intact.
    let glm_reasoning_burn = std::cell::Cell::new(0u64);
    let glm_burn_limit = super::context::env_usize("ANGEL_GLM_THINKING_BURN", 0) as u64;
    // True when the previous hop ended in a plain answer (no tool calls): a
    // submit resets the runaway-thinking burn counter.
    let glm_last_hop_answered = std::cell::Cell::new(false);
    let mut green_verify_achieved = false;
    // The hop advisories of 0.1.6 (`⠼`, and `⡅` in a competition): each is a
    // route with its own turn, given under the condition 0.1.6 gave it.
    let mut green_verify_nudge_emitted = false;
    let post_edit_review = book::d3456_advisories::post_edit_review_enabled();
    let mut post_edit_logic_reviewed = false;
    let mut weak_verification_pending = false;
    let mut weak_verification_told = false;
    let mut mutation_thrash = book::d3456_advisories::MutationThrash::default();
    let mut peripheral_fanout = book::d3456_advisories::PeripheralFanout::default();
    let first_write_limit = book::d3456_advisories::first_write_limit();
    let no_edit_guard = book::d3456_advisories::no_edit_guard_armed();
    let mut prewrite_calls = 0usize;
    let mut first_write_attempted = false;
    let mut first_write_nudge_emitted = false;
    // A bounded turn that has edited keeps a reserve of hops for its final
    // mile; a task turn defaults it, as the 0.1.6 task defaults did.
    let final_mile_hops = if research_turn {
        0
    } else {
        env_usize(
            "ANGEL_FINAL_MILE_HOPS",
            if task_active {
                book::d3456_advisories::final_mile_reserve(task_pace, max_hops.is_some())
            } else {
                0
            },
        )
    };
    let mut final_mile_active = false;
    // The model's last passing test run and the workspace it passed on. At its
    // first stop the run is repeated on the unchanged code: one pass can be
    // luck. A failed re-run is `⠧⠑`.
    let mut last_green_run: Option<book::v_verification::GreenRun> = None;
    let confirm_green_runs = book::v_verification::confirm_green_extra_runs(competition);
    let confirm_green_on_chance =
        book::v_verification::confirm_green_by_chance(task_active, competition);
    // Files the model edited with file tools this turn, for the chance check.
    let mut edited_paths: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    // The model's last red test run and the workspace it failed on. A task-mode
    // answer on that same code carries `⠧⠁` to the stop checkpoint.
    // Task mode only: the run ends on its answer and nobody is there to say
    // "keep going"; interactive answers go to a person; competition never
    // re-opens answers. polyglot-v1: gpt-6-luna answered "tests still fail" on
    // five tasks with 74-94% of its 600 s left, and solved all five when re-run
    // at higher effort. Grok 4.7 un-skipped `grep.spec.js` in 5 of 6 js-grep
    // runs; the runs that passed restored it on their own.
    let track_task_facts = task_active && !competition;
    let mut last_red_run: Option<book::v_verification::RedRun> = None;
    let mut red_streak = book::v_verification::RedStreak::default();
    let task_wall_secs = if turn_budget > 0 {
        turn_budget
    } else {
        env_usize("ANGEL_TASK_WALL_SECS", 0)
    };
    // Test files that came with the task are its contract. A task-mode answer
    // with one of them changed carries `⠧⠛`; files already changed before
    // this turn are not the model's doing.
    let tests_changed_at_start = if track_task_facts {
        book::v_verification::changed_test_files(registry.current_workspace()).unwrap_or_default()
    } else {
        Vec::new()
    };
    // A task that declares its editable surface (a Yukon benchmark.json, or
    // ANGEL_TASK_EDITABLE_PATHS_JSON) gets `⠧⠓` on edits outside it: those
    // changes are not part of what is evaluated.
    let edit_scope = book::v_verification::task_edit_scope(registry.current_workspace());
    let mut edit_scope_noted: std::collections::HashSet<String> = std::collections::HashSet::new();
    // Paths that look like tests created/rewritten this turn — green checks that
    // only name these basenames leave the edits `⠧⠋`.
    let mut self_authored_test_basenames: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    let self_authored_verify_guard = env_flag("ANGEL_SELF_AUTHORED_VERIFY_GUARD", false);
    let mut verification_needed = false;
    // Where the model last ran a test of any kind (`⠧⠋` when it moved since).
    let mut untested = book::v_verification::Untested::default();
    // Timed cues (`⠺`): hygiene at the turn's first tool result, then cues
    // as the model's own actions make them relevant (`⠺`).
    let mut cues = book::w_workflow::Cues::default();
    let mut attempted_opaque_generation = 0;
    let unverified_completion_claims = std::cell::Cell::new(0usize);
    // The once-per-turn stop checkpoint (`⠟`).
    let mut checkpoint = book::q_stop::Checkpoint::default();
    // Loop evidence raised this turn (`⠇…`, `⠭⠁`), carried to the checkpoint.
    let mut loop_facts: Vec<book::Raise> = Vec::new();
    // Competition slots already announced to the model (`⠅⠉`).
    let mut watcher_announced: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut mutation_seen = false;
    let mut hooks_serial_notice_sent = false;
    // Tool names are not proof of workspace state: shell, MCP, code mode, or a
    // nested integration can edit without presenting as a direct write call.
    // Compare the real Git-backed state at finalization and after verifiers so
    // an opaque edit still reads as `⠥` at the stop checkpoint.
    let fingerprint_started = Instant::now();
    let initial_workspace_fingerprint = workspace_fingerprint(registry.current_workspace());
    super::trajectory::note_task_startup_phase(
        "workspace_fingerprint_ms",
        fingerprint_started.elapsed().as_millis(),
    );
    let mut verification_attempt_workspace_fingerprint = None;
    // A successful direct prose-only edit still changes the Git fingerprint,
    // but has no meaningful runtime verifier. Remember the exact post-edit
    // state so the opaque-write fallback does not undo the classifier.
    let mut prose_only_workspace_fingerprint = None;
    // A provider-side 413/context-length rejection is deterministic for the
    // current payload. Rebuild once after forced compaction instead of spending
    // the generic retry budget on the same bytes. The cap is per turn and small
    // by design: a broken summarizer or giant protected prompt cannot loop.
    let max_overflow_recoveries = env_usize("ANGEL_CONTEXT_OVERFLOW_RECOVERIES", 1).min(2);
    let mut overflow_recoveries = 0usize;
    let mut infeasible_context_notice_sent = false;
    let session_recovery_budget = max_session_recoveries();
    let mut session_recoveries = 0usize;
    let request_overflows = std::cell::Cell::new(0usize);
    // W5: dispatch-level tool errors split into four bounded classes so the
    // ledger can prove whether the schema class died. Cell so the shared
    // `write_exp` closure and the dispatch loop can both see it.
    let tool_errors_by_class =
        std::cell::Cell::new(crate::knowledge::experience::ToolErrorClasses::default());
    // A7: provider retry knobs are process-lifetime for a turn — hoist out of
    // the per-hop loop so multi-hop turns do not re-parse env every hop. The
    // budget is `None` when the knob is unset: the L01 unattended policy rides
    // out a recoverable outage without an implicit count, while an explicit
    // integer (including `0`) keeps the bounded policy.
    let provider_retries = provider_retry_budget();
    let provider_retry_backoff_ms = env_usize("ANGEL_PROVIDER_RETRY_BACKOFF_MS", 500);
    let evaluate_max_hops_workspace = env_flag("ANGEL_EVALUATE_MAX_HOPS_WORKSPACE", false);
    // Replies that printed raw tool markup instead of calling a tool.
    let mut markup_replies = 0usize;
    let mut action_capsule_metrics = ActionCapsuleMetrics::default();
    // Experience ledger: record this turn's config + outcome at whichever exit
    // it takes. Snapshot the driver's cumulative token counters up front so the
    // per-turn spend is a delta, and resolve the driver `path` tag once. The
    // write is best-effort and test-silent (`crate::knowledge::experience`), so it can
    // never fail a turn. `write_exp` is called at every exit below with the stop
    // reason and the anti-spin counters as they stand.
    crate::agent::turn::phase::mark("experience_path");
    let exp_path = crate::knowledge::experience::driver_path();
    let tok_before = club
        .token_usage()
        .map(|u| (u.total_input, u.total_output))
        .unwrap_or((0, 0));
    // Per-turn metered input is telemetry, never a termination budget. Count
    // every actual provider attempt (including retries) and prefer provider
    // counters when available; the already-computed local request size is the
    // conservative fallback for usage-blind backends.
    let mut metered_input_accounted = 0u64;
    let truncation_before = club.truncation_usage();
    let cache_before = club.cache_usage();
    let effort_gate_before = club.effort_gate_usage();
    // Per-hop cache ledger (perf-cache-ledger): measurement-only. Each provider
    // call below snapshots the club's cumulative cache/token counters so its
    // own hop gets an attributable hit rate, and `hop_breakers` collects the
    // known prefix breakers that ran since the prior request (defs delta,
    // aging/dedup receipts, compaction splice, steer injection…) so a hit-rate
    // drop can name its cause. Requests are never mutated on this path.
    let hop_cache_ledger = crate::agent::turn::cache_hop_ledger_enabled();
    let mut hop_breakers: Vec<&'static str> = Vec::new();
    // Reuse last hop's schema set while tool_search activations are idle —
    // the common multi-hop path. Activations bump a generation on change.
    let mut prev_activation_gen = registry.tool_activation_generation();
    let mut prev_defs_fingerprint = crate::agent::turn::defs_fingerprint(&defs);
    let mut cached_tool_schema_tokens = estimate_tool_tokens(&defs);
    let mut hist_tokens = HistoryTokenRoll::default();
    hist_tokens.recompute(history);
    let write_exp = |stop: &str,
                     ok: bool,
                     hop: usize,
                     mut counters: crate::knowledge::experience::TurnCounters| {
        let resolved_route = if ok {
            club.resolved_route_identity()
        } else {
            club.route_identity()
        };
        let (a_in, a_out) = club
            .token_usage()
            .map(|u| (u.total_input, u.total_output))
            .unwrap_or(tok_before);
        // Distinguish "usage-known zero" (the club reports usage accounting
        // but this turn moved no counters) from "no data" (a usage-blind
        // backend, which must not silently read as a free turn). See
        // docs/telemetry/token-efficiency.md P1.
        let tokens = if club.token_usage().is_some() {
            vec![(
                club.label().to_string(),
                a_in.saturating_sub(tok_before.0),
                a_out.saturating_sub(tok_before.1),
            )]
        } else {
            Vec::new()
        };
        let truncation_after = club.truncation_usage();
        let cache_after = club.cache_usage();
        // Gates must speak: if any club on this route withheld a requested
        // reasoning effort or learned a backend rejection during the turn,
        // voice the reason once instead of letting the strip pass silently.
        let effort_gate_after = club.effort_gate_usage();
        let gate_moved = effort_gate_after.withheld > effort_gate_before.withheld
            || effort_gate_after.rejections > effort_gate_before.rejections;
        if gate_moved {
            let reason = effort_gate_after
                .last
                .clone()
                .unwrap_or_else(|| "reasoning effort withheld by a capability gate".into());
            let _ = events.send(TurnEvent::Notice(format!("effort gate: {reason}")));
        }
        counters.provider_truncation_retries = truncation_after
            .retries
            .saturating_sub(truncation_before.retries);
        counters.provider_truncation_episodes = truncation_after
            .episodes
            .saturating_sub(truncation_before.episodes);
        counters.provider_truncation_retained_partials = truncation_after
            .retained_partials
            .saturating_sub(truncation_before.retained_partials);
        counters.provider_truncation_recoveries = truncation_after
            .recoveries
            .saturating_sub(truncation_before.recoveries);
        counters.provider_truncation_failures = truncation_after
            .failures
            .saturating_sub(truncation_before.failures);
        counters.skill_hints = skill_hints;
        counters.code_mode_calls = code_mode_calls.get();
        counters.code_mode_nested_calls = code_mode_nested_calls.get();
        counters.code_mode_nested_output_bytes = code_mode_nested_output_bytes.get();
        counters.code_mode_policy_rejections = code_mode_policy_rejections.get();
        counters.code_mode_recipe_calls = code_mode_recipe_calls.get();
        counters.task_recon_context_bytes = task_recon_context_bytes;
        counters.code_mode_schema_tokens = code_mode_schema_tokens;
        counters.request_overflows = request_overflows.get();
        counters.cache_control_requests = cache_after
            .control_requests
            .saturating_sub(cache_before.control_requests);
        counters.cache_read_input_tokens = cache_after
            .read_input_tokens
            .saturating_sub(cache_before.read_input_tokens);
        counters.cache_write_input_tokens = cache_after
            .write_input_tokens
            .saturating_sub(cache_before.write_input_tokens);
        counters.cache_read_accounting_responses = cache_after
            .read_accounting_responses
            .saturating_sub(cache_before.read_accounting_responses);
        counters.cache_write_accounting_responses = cache_after
            .write_accounting_responses
            .saturating_sub(cache_before.write_accounting_responses);
        counters.unverified_completion_claims = unverified_completion_claims.get();
        counters.discovered_tool_schema_failures = discovered_tool_schema_failures.get();
        counters.tool_argument_shrinks = tool_argument_shrinks.get();
        counters.tool_argument_strings_shrunk = tool_argument_strings_shrunk.get();
        counters.tool_argument_bytes_saved = tool_argument_bytes_saved.get();
        counters.aged_inspection_results = aged_inspection_results.get();
        counters.aged_inspection_bytes_saved = aged_inspection_bytes_saved.get();
        counters.eager_offload_results = eager_offload_results.get();
        counters.eager_offload_bytes_saved = eager_offload_bytes_saved.get();
        counters.post_edit_diagnostic_attempts = post_edit_diagnostics.attempts.get();
        counters.post_edit_diagnostic_findings = post_edit_diagnostics.findings.get();
        counters.post_edit_diagnostic_failures = post_edit_diagnostics.failures.get();
        counters.post_edit_diagnostic_paths_skipped = post_edit_diagnostics.paths_skipped.get();
        counters.post_edit_diagnostic_output_bytes = post_edit_diagnostics.output_bytes.get();
        counters.post_edit_diagnostic_elapsed_ms = post_edit_diagnostics.elapsed_ms.get();
        counters.tool_errors_by_class = tool_errors_by_class.get();
        crate::agent::turn::phase::mark("experience_record");
        crate::knowledge::experience::record_turn(
            &crate::knowledge::experience::TurnExperience {
                path: &exp_path,
                driver: &resolved_route.driver,
                model: resolved_route.model.as_deref(),
                reasoning_effort: resolved_route.reasoning_effort.as_deref(),
                ok,
                stop,
                latency_ms: turn_start.elapsed().as_millis(),
                hops: hop,
                time_to_first_mutation_ms: time_to_first_mutation_ms.get(),
                time_to_green_ms: time_to_green_ms.get(),
                system_prompt_tokens,
                project_doc_bytes,
                tool_schema_count,
                tool_schema_tokens,
                tool_schema_peak_count: tool_schema_peak_count.get(),
                tool_schema_peak_tokens: tool_schema_peak_tokens.get(),
                tool_schema_token_requests: tool_schema_token_requests.get(),
                counters,
                tokens,
                failovers: crate::knowledge::experience::drain_failovers(),
            },
            registry.current_workspace(),
        );
    };
    // The Cut's machine verdicts for this turn (docs/plans/the-cut.md, T2),
    // folded at the post-write seam below and read at every exit as this
    // rollout's REWARD — the label `~/.angelX/trajectories` has never carried, and
    // without which the forge can only imitate its teacher
    // (`crate::knowledge::cut::turn_reward` documents the semantics). A turn that wrote no
    // source earns no verdict and stays unlabeled, exactly as before.
    let mut verdicts = crate::knowledge::cut::TurnVerdicts::default();
    let mut post_write_verification = crate::knowledge::cut::PostWriteVerification::default();
    // Optional deterministic completion gate for headless action harnesses.
    // It is armed only when the command is red before the first model call;
    // otherwise an already-green fixture could short-circuit without work.
    let mut accept_baseline_ms = 0;
    let mut accept_post_checks = 0;
    let mut accept_post_ms = 0;
    let mut accept_baseline_result = None;
    let mut accept_last_post_result = None;
    let mut accept_last_checked_workspace = None;
    let mut accept_last_summary = None;
    let task_accept_requested = std::env::var("ANGEL_TASK_ACCEPT_CMD")
        .ok()
        .filter(|command| {
            // The root task owns this hook. Delegates have their own verifier
            // contracts and must not run the parent's hook in their worktree.
            !registry.external_evaluator_only
                && crate::agent::harness::lane::subcall_depth() == 0
                && !command.trim().is_empty()
        });
    let task_accept_cmd = task_accept_requested.clone().and_then(|command| {
        let workspace_before_baseline = workspace_evidence_sha256(registry.current_workspace());
        let baseline =
            book::v_verification::run_task_accept(&command, registry.current_workspace());
        accept_baseline_ms = baseline.elapsed_ms;
        accept_baseline_result = Some(baseline.result_class);
        let workspace_after_baseline = workspace_evidence_sha256(registry.current_workspace());
        accept_last_checked_workspace = match (workspace_before_baseline, workspace_after_baseline)
        {
            (Some(before), Some(after)) if before == after => Some(after),
            _ => None,
        };
        accept_last_summary = Some(baseline.summary.clone());
        if baseline.passed {
            let _ = events.send(TurnEvent::Notice(
                "task acceptance command was already green; deterministic auto-completion disabled"
                    .to_string(),
            ));
            None
        } else {
            Some(command)
        }
    });
    macro_rules! record_rollout_error {
        ($stage:expr_2021, $error:expr_2021) => {{
            let detail = format!("{}: {}", $stage, $error);
            if rollout_capture_error.is_none() {
                rollout_capture_error = Some(detail.clone());
            }
            let _ = events.send(TurnEvent::RolloutCaptureError(if rollout_required {
                format!("required rollout capture degraded: {detail}")
            } else {
                format!("rollout capture degraded; turn outcome is unchanged: {detail}")
            }));
        }};
    }
    macro_rules! prepare_rollout_seal {
        () => {{
            // Turn completion is append-only too: continuation requests can
            // reuse this prefix. Actual savings are collected at compaction.
            // The held count covers hops still unflushed; a boundary receipt is
            // reported even when the boundary itself already flushed everything.
            let boundary_savings = aged_inspection_results.get() > 0
                || tool_argument_shrinks.get() > 0
                || duplicate_inspection_results > 0;
            if cache_stable && (deferred_rewrite_hops.get() > 0 || boundary_savings) {
                let retained_bytes: usize = history.iter().map(|m| m.content.len()).sum();
                let note = format!(
                    "cache-stable: held rewrites for the last {} hop(s); prefix retained {} content bytes; at boundaries aged {} result(s) (−{} bytes), shrank {} tool-call argument(s) (−{} bytes), and deduped {} duplicate result(s) (−{} bytes)",
                    deferred_rewrite_hops.get(), retained_bytes,
                    aged_inspection_results.get(), aged_inspection_bytes_saved.get(),
                    tool_argument_shrinks.get(), tool_argument_bytes_saved.get(),
                    duplicate_inspection_results, duplicate_inspection_bytes_saved,
                );
                let _ = events.send(TurnEvent::Notice(note));
            }
            if eval_owns_label()
                && (task_accept_requested.is_none() || accept_last_post_result == Some("passed"))
                && let Err(error) = rollout_recorder.attach_task_reward()
            {
                record_rollout_error!("task reward evidence", error);
            }
            if let Err(error) = rollout_recorder.attach_cut_reward(
                verdicts.reward(),
                !eval_owns_label(),
            ) {
                record_rollout_error!("reward evidence", error);
            }
            if let Err(error) = rollout_recorder.attach_compatibility_snapshot(|| {
                (
                    club.label().to_string(),
                    root_trajectory_json(history),
                    harness_treatment_json(),
                )
            }) {
                record_rollout_error!("compatibility snapshot", error);
            }
        }};
    }
    macro_rules! observed_outcome {
        ($outcome:expr_2021, $source:expr_2021) => {{
            let mut outcome = $outcome;
            // Most answers close scaffolds in the Text arm, where failures can
            // still be repaired. Deterministic early-completion paths must not
            // bypass that final obligation either.
            if outcome.stop_reason == TurnStopReason::Answer {
                let notes = with_turn_deadline_cancel(cancel, turn_deadline, |verify_cancel| {
                    finish_post_write_verification(
                        registry,
                        club,
                        outcome.hops,
                        &mut post_write_verification,
                        &mut verdicts,
                        &mut rollout_recorder,
                        verify_cancel,
                    )
                });
                for note in notes {
                    let _ = events.send(TurnEvent::Notice(note.clone()));
                    outcome.answer.push_str(&format!("\n\n{note}"));
                }
            }
            if let Some(note) = verdicts.pending_note() {
                outcome.answer.push_str(&format!("\n\n{note}"));
            }
            crate::agent::harness::trajectory::note_task_answer();
            outcome.acceptance = task_acceptance_snapshot(
                task_accept_requested.as_deref(),
                task_accept_cmd.is_some(),
                accept_baseline_result,
                accept_baseline_ms,
                accept_post_checks,
                accept_post_ms,
                accept_last_post_result,
                $source,
                accept_last_checked_workspace.clone(),
                || {
                    crate::agent::harness::workspace_state::workspace_evidence_sha256(
                        registry.current_workspace(),
                    )
                },
            );
            prepare_rollout_seal!();
            if let Err(error) =
                rollout_recorder.record_auxiliary_coverage(auxiliary_scope.snapshot())
            {
                record_rollout_error!("auxiliary coverage", error);
            }
            if let Err(error) = rollout_recorder.finish_turn(
                outcome.stop_reason.as_str(),
                outcome.interrupted,
                outcome.deadline_reached,
                outcome.max_hops_reached,
                Some(outcome.answer.as_str()),
            ) {
                record_rollout_error!("terminal seal", error);
            }
            outcome.reward_binding = rollout_recorder.reward_binding();
            outcome.rollout_id = rollout_recorder.rollout_id().map(str::to_string);
            outcome.timing =
                Some(timing.finish_with_history(turn_start.elapsed().as_millis(), history));
            outcome.tools = crate::agent::harness::trajectory::tool_ledger_snapshot();
            if rollout_required {
                if let Some(error) = rollout_capture_error.take() {
                    return Err(TurnFailure {
                        message: format!("required rollout capture failed: {error}"),
                        stop_reason: TurnStopReason::CaptureFailure,
                        hops: outcome.hops,
                        interrupted: false,
                        deadline_reached: false,
                        max_hops_reached: false,
                        acceptance: outcome.acceptance.map(Box::new),
                        rollout_id: outcome.rollout_id,
                    });
                }
            }
            return Ok(outcome);
        }};
    }
    macro_rules! observed_failure {
        ($failure:expr_2021, $source:expr_2021) => {{
            let mut failure = $failure;
            if let Some(note) = verdicts.pending_note() {
                failure.message.push_str(&format!("\n\n{note}"));
            }
            failure.acceptance = task_acceptance_snapshot(
                task_accept_requested.as_deref(),
                task_accept_cmd.is_some(),
                accept_baseline_result,
                accept_baseline_ms,
                accept_post_checks,
                accept_post_ms,
                accept_last_post_result,
                $source,
                accept_last_checked_workspace.clone(),
                || {
                    crate::agent::harness::workspace_state::workspace_evidence_sha256(
                        registry.current_workspace(),
                    )
                },
            )
            .map(Box::new);
            prepare_rollout_seal!();
            if let Err(error) =
                rollout_recorder.record_auxiliary_coverage(auxiliary_scope.snapshot())
            {
                record_rollout_error!("auxiliary coverage", error);
            }
            if let Err(error) = rollout_recorder.finish_turn(
                failure.stop_reason.as_str(),
                failure.interrupted,
                failure.deadline_reached,
                failure.max_hops_reached,
                Some(failure.message.as_str()),
            ) {
                record_rollout_error!("terminal seal", error);
            }
            failure.rollout_id = rollout_recorder.rollout_id().map(str::to_string);
            if rollout_required {
                if let Some(error) = rollout_capture_error.take() {
                    failure.message = format!("required rollout capture failed: {error}");
                    failure.stop_reason = TurnStopReason::CaptureFailure;
                    failure.interrupted = false;
                    failure.deadline_reached = false;
                    failure.max_hops_reached = false;
                }
            }
            return Err(failure);
        }};
    }
    // The unproductive streak never ends a turn. At 0.1.6 it spoke to the model
    // at eight unproductive hops and again, as a redirect, at sixteen, before
    // it stopped the turn; the stop is gone and the two stamps of `⠇⠛` remain,
    // each its own turn under the warning sign with the count and the last
    // verifier beside it. A stretch that goes on earns the notice, then the
    // redirect, again.
    let unproductive_redirect = if task_active && !competition {
        UNPRODUCTIVE_REDIRECT_HOPS
    } else {
        0
    };
    macro_rules! handle_unproductive_streak {
        () => {{
            let (notice, redirect) = crate::agent::harness::trajectory::unproductive_escalation(
                hop,
                streak_escalate,
                unproductive_redirect,
            );
            // The redirect is the later word: when both fall on one hop the
            // model is given it, not the notice it already met.
            if let Some(turn) = redirect.clone().or_else(|| notice.clone()) {
                history.push(ChatMsg::harness(format!(
                    "{}{turn}",
                    book::l_loops::WARNING
                )));
                let _ = events.send(TurnEvent::Notice(turn));
            }
        }};
    }
    'turn: loop {
        // Soft interrupt takes priority: bail at the hop boundary with the
        // conversation preserved. Never a cap — the user is in control.
        if cancel.load(Ordering::Relaxed) {
            let note = format!(
                "⛔ interrupted after {hop} hop(s) — conversation kept; \
                 send another message to steer or resume."
            );
            // Deliberately UNLABELED (`None`), and the only exit that is. Every
            // other stop is a function of the policy's own behavior — it
            // answered, it spun, it thrashed, it ran out of hops — so the state
            // it left the build in is its own. This one is a human hitting stop
            // at a moment of their choosing, which can land one hop before the
            // fix. Scoring that 0.0 would punish the model for a repair it was
            // never allowed to attempt.
            crate::agent::harness::trajectory::note_task_timing(
                &timing.finish_with_history(turn_start.elapsed().as_millis(), history),
            );
            crate::agent::harness::trajectory::note_stop_reason(TurnStopReason::Interrupt.as_str());
            log_trajectory(club, history, &note, hop, true, None);
            write_exp(
                "interrupt",
                false,
                hop,
                turn_counters!(
                    markup_replies,
                    loops.spin(),
                    errors.streak(),
                    duplicate_inspection_results,
                    duplicate_inspection_bytes_saved,
                    action_capsule_metrics,
                ),
            );
            observed_outcome!(
                TurnOutcome::stopped(note, TurnStopReason::Interrupt, hop),
                "interrupt"
            );
        }
        handle_unproductive_streak!();
        if turn_expired(turn_start.elapsed().as_secs(), turn_budget) {
            let deadline_source = if std::env::var_os("ANGEL_TURN_DEADLINE_SECS").is_some() {
                "ANGEL_TURN_DEADLINE_SECS"
            } else {
                "ANGEL_COMPETITION_TURN_DEADLINE_SECS"
            };
            let note = format!(
                "⏱ turn hit the {turn_budget}s wall-clock budget (operator cap {deadline_source}={turn_budget}) \
                 after {hop} hop(s) — stopping; conversation kept. Send another message to \
                 continue."
            );
            crate::agent::harness::trajectory::note_task_timing(
                &timing.finish_with_history(turn_start.elapsed().as_millis(), history),
            );
            crate::agent::harness::trajectory::note_stop_reason(TurnStopReason::Deadline.as_str());
            log_trajectory(club, history, &note, hop, true, verdicts.reward());
            write_exp(
                "deadline",
                false,
                hop,
                turn_counters!(
                    markup_replies,
                    loops.spin(),
                    errors.streak(),
                    duplicate_inspection_results,
                    duplicate_inspection_bytes_saved,
                    action_capsule_metrics,
                ),
            );
            observed_outcome!(
                TurnOutcome::stopped(
                    if research_turn {
                        research::draft(history).unwrap_or_else(|| note.clone())
                    } else {
                        note.clone()
                    },
                    TurnStopReason::Deadline,
                    hop
                )
                .with_stop_notice(note),
                "deadline"
            );
        }
        if let Some(max) = max_hops
            && hop >= max
        {
            let message = format!(
                "tool loop hit the {max}-hop runaway guard without answering \
                     (remove the positive ANGEL_MAX_HOPS override or set ANGEL_MAX_HOPS=0 for unbounded)"
            );
            // Preserve the incomplete rollout for audit/training parity
            // with every other guarded stop. The stop note is the record's
            // answer field only; history remains untouched and all prior
            // assistant tool calls already have their tool results.
            crate::agent::harness::trajectory::note_task_timing(
                &timing.finish_with_history(turn_start.elapsed().as_millis(), history),
            );
            crate::agent::harness::trajectory::note_stop_reason(TurnStopReason::MaxHops.as_str());
            log_trajectory(club, history, &message, hop, true, verdicts.reward());
            write_exp(
                "max_hops",
                false,
                hop,
                turn_counters!(
                    markup_replies,
                    loops.spin(),
                    errors.streak(),
                    duplicate_inspection_results,
                    duplicate_inspection_bytes_saved,
                    action_capsule_metrics,
                ),
            );
            // Benchmark/evaluator callers must be able to score the real
            // workspace even when the policy exhausts its horizon. This
            // opt-in preserves a truthful stopped/max_hops envelope while
            // avoiding an infrastructure-shaped process failure that drops
            // the cohort row. Ordinary task and interactive callers retain
            // the historical structured error by default.
            if research_turn {
                // The answer's reader cannot read the ledger: `⠟⠛⠃`'s page.
                let answer =
                    research::draft(history).unwrap_or_else(|| book::q_stop::NO_DRAFT.to_string());
                observed_outcome!(
                    TurnOutcome::stopped(answer, TurnStopReason::MaxHops, hop)
                        .with_stop_notice(message),
                    "max_hops"
                );
            }
            if evaluate_max_hops_workspace {
                observed_outcome!(
                    TurnOutcome::stopped(message, TurnStopReason::MaxHops, hop),
                    "max_hops"
                );
            }
            observed_failure!(
                TurnFailure {
                    message,
                    stop_reason: TurnStopReason::MaxHops,
                    hops: hop,
                    interrupted: true,
                    deadline_reached: false,
                    max_hops_reached: true,
                    acceptance: None,
                    rollout_id: None,
                },
                "max_hops"
            );
        }
        hop += 1;
        crate::agent::turn::phase::mark("hop_start");
        let _phase_hop = crate::agent::turn::phase::Hop;
        let _progress_hop = crate::agent::harness::trajectory::begin_progress_hop();
        if hop == 1 {
            // The turn boundary itself rewrites the request tail: new user
            // input. Aging remains deferred until compaction even across turns.
            hop_breakers.push("turn boundary");
        }
        // Mid-run steering: the user typed while this turn was in flight —
        // deliver the note(s) at this hop boundary so the model sees the
        // guidance in the very next request without the turn being
        // interrupted. Always positioned after the previous hop's tool
        // results (never between a tool-call batch and its results, which
        // would break pairing).
        crate::agent::turn::phase::mark("steer_drain");
        if let Some(q) = steers {
            let queued = q.drain();
            if !queued.is_empty() {
                history.push(crate::agent::steer::context_message());
                hop_breakers.push("steer injection");
            }
            for msg in queued {
                let _ = events.send(TurnEvent::Notice(format!(
                    "steer delivered → {}",
                    crate::agent::steer::snippet(&msg.content)
                )));
                history.push(msg);
            }
        }
        // `tool_search` can surface hidden schemas during the previous hop.
        // Rebuild only when activations moved so those tools become real
        // structured provider definitions on the next hop; stable hops reuse
        // the prior set (byte-identical wire JSON + zero rebuild/hash/token
        // estimate cost).
        crate::agent::turn::phase::mark("hop_context");
        let activation_gen = registry.tool_activation_generation();
        if activation_gen != prev_activation_gen {
            defs = registry.defs_for_driver_turn(
                ctx_window,
                max_hops.is_some(),
                competition,
                metered_sota,
            );
            if research_turn {
                research::ensure_tools(&mut defs, registry);
            }
            research::describe_surface(&mut defs, research_origin.as_deref());
            prev_activation_gen = activation_gen;
            let next_fp = crate::agent::turn::defs_fingerprint(&defs);
            if prev_defs_fingerprint != next_fp {
                hop_breakers.push("defs delta");
            }
            prev_defs_fingerprint = next_fp;
            cached_tool_schema_tokens = estimate_tool_tokens(&defs);
        }
        discovered_tool_schema_failures.set(
            discovered_tool_schema_failures
                .get()
                .saturating_add(registry.activated_schema_failures(&defs)),
        );
        tool_schema_peak_count.set(tool_schema_peak_count.get().max(defs.len()));
        tool_schema_peak_tokens.set(tool_schema_peak_tokens.get().max(cached_tool_schema_tokens));
        // Trim the oldest complete turns if the conversation has outgrown the
        // hygiene cap, then summarize older turns if it's over the token budget,
        // before we build the next request body.
        let argument_shrink = if cache_stable {
            ToolArgumentShrink::default()
        } else {
            maybe_shrink_tool_arguments(history)
        };
        tool_argument_shrinks.set(
            tool_argument_shrinks
                .get()
                .saturating_add(argument_shrink.calls),
        );
        tool_argument_strings_shrunk.set(
            tool_argument_strings_shrunk
                .get()
                .saturating_add(argument_shrink.strings),
        );
        tool_argument_bytes_saved.set(
            tool_argument_bytes_saved
                .get()
                .saturating_add(argument_shrink.bytes_saved),
        );
        if argument_shrink.calls > 0 {
            hop_breakers.push("tool-arg shrink");
        }
        // Hold inspection rewrites until compaction or the configured rolling
        // boundary. Setting the cadence to zero retains strict append-only
        // prefixes for cache/cost comparison.
        let dedup = if cache_stable {
            InspectionDedup::default()
        } else {
            maybe_dedupe_inspection_results(history)
        };
        duplicate_inspection_results = duplicate_inspection_results.saturating_add(dedup.results);
        duplicate_inspection_bytes_saved =
            duplicate_inspection_bytes_saved.saturating_add(dedup.bytes_saved);
        if dedup.results > 0 {
            hop_breakers.push("dedup rewrite");
        }
        let aging = if cache_stable {
            deferred_rewrite_hops.set(deferred_rewrite_hops.get().saturating_add(1));
            InspectionAging::default()
        } else {
            maybe_age_tool_results(history)
        };
        aged_inspection_results.set(aged_inspection_results.get().saturating_add(aging.results));
        aged_inspection_bytes_saved.set(
            aged_inspection_bytes_saved
                .get()
                .saturating_add(aging.bytes_saved),
        );
        if aging.results > 0 {
            hop_breakers.push("result aging");
        }
        book_continuity.observe(history);
        let history_len_before_prune = history.len();
        prune_history(history, history_cap);
        if history.len() < history_len_before_prune {
            hop_breakers.push("history prune");
        }
        // Compaction is layered: land a finished background pass first (zero
        // dead air — the summarizer ran while the previous hops flowed), run
        // the synchronous compactor only as emergency overflow (no early pass
        // in flight, or usage blew past the +10% hard ceiling while one ran),
        // then arm the next early pass at ~80% of budget. Only the unbounded
        // driver owns the background pass — bounded delegate seats share this
        // registry and keep the pure sync path.
        let bg_owner = max_hops.is_none();
        if bg_owner {
            // A landed splice replaces a history prefix with its summary note,
            // so the message count moving is a faithful landed-splice signal.
            let history_len_before_splice = history.len();
            try_splice_bg_compact(registry, history, events);
            if history.len() != history_len_before_splice {
                hop_breakers.push("compaction splice");
            }
        }
        // History may have been rewritten above — recompute the rolling token
        // estimate once for the rest of the hop (budget, gauge, fit).
        let history_rewritten = argument_shrink.calls > 0
            || dedup.results > 0
            || aging.results > 0
            || history.len() < history_len_before_prune
            || hop_breakers
                .iter()
                .any(|b| matches!(*b, "compaction splice" | "history prune"));
        let hist_tok = if history_rewritten {
            hist_tokens.recompute(history)
        } else {
            hist_tokens.observe(history)
        };
        // The default local fallback is cheap enough to run as soon as the
        // budget is crossed. Only the explicit synchronous-model mode keeps
        // the old +10% grace period to avoid racing two model summaries.
        let bg_defer = crate::agent::compaction::sync_compaction_uses_model()
            && bg_owner
            && bg_compact_inflight(registry)
            && hist_tok + cached_tool_schema_tokens
                <= effective_budget.saturating_add(effective_budget / 10);
        if cache_stable
            && !bg_defer
            && effective_budget > 0
            && hist_tok + cached_tool_schema_tokens > effective_budget
        {
            // Age full tool bodies at the compact boundary *before* the
            // summarizer parks/replaces the window; otherwise compaction
            // stores emptied stubs and aging has nothing left to measure.
            let aged = age_tool_results_at_boundary(history);
            aged_inspection_results.set(aged_inspection_results.get().saturating_add(aged.results));
            aged_inspection_bytes_saved.set(
                aged_inspection_bytes_saved
                    .get()
                    .saturating_add(aged.bytes_saved),
            );
            if aged.results > 0 || aged.excerpts_dropped > 0 {
                hop_breakers.push("tool-aging boundary");
                hist_tokens.recompute(history);
                // This boundary rewrote held bytes, so nothing is held now.
                deferred_rewrite_hops.set(0);
            }
        }
        if !bg_defer
            && maybe_compact_for_turn(
                club,
                history,
                effective_budget,
                compact_keep,
                compact_keep_tokens,
                &defs,
                registry,
                events,
            )
        {
            hop_breakers.push("sync compaction");
            hist_tokens.recompute(history);
        }
        let rolling_flush = cache_stable
            && rolling_rewrite_hops > 0
            && deferred_rewrite_hops.get() >= rolling_rewrite_hops.max(4);
        // A rolling flush pays the whole held prefix again as a cache miss on
        // the next request. On a byte-exact provider prefix cache (the rust30
        // wire cohort: three tasks re-billed 4,096–12,288 tokens exactly at
        // hops 12 and 24) that penalty is paid in tokens *and* first-token
        // latency, for savings that only matter when the request body is
        // actually large. Gate the cadence flush on a body-size floor: short
        // turns keep their cache-warm prefix and never pay the rewrite. The
        // floor defaults to a sixth of the live compaction budget so a
        // small-window seat still flushes before its body balloons, and an
        // operator keeps the old behavior with
        // `ANGEL_ROLLING_REWRITE_MIN_TOKENS=0`; compaction/prune breakers above
        // still flush immediately.
        let rolling_rewrite_min_tokens = env_usize("ANGEL_ROLLING_REWRITE_MIN_TOKENS", 24_000);
        let rolling_rewrite_min_tokens = if rolling_rewrite_min_tokens > 0 {
            rolling_rewrite_min_tokens.min(effective_budget / 6)
        } else {
            0
        };
        let rolling_flush =
            rolling_flush && hist_tok + cached_tool_schema_tokens >= rolling_rewrite_min_tokens;
        if cache_stable
            && (rolling_flush
                || hop_breakers.iter().any(|b| {
                    matches!(
                        *b,
                        "compaction splice" | "sync compaction" | "history prune"
                    )
                }))
        {
            let aged = age_tool_results_at_boundary(history);
            let shrunk = maybe_shrink_tool_arguments(history);
            let deduped = maybe_dedupe_inspection_results(history);
            aged_inspection_results.set(aged_inspection_results.get().saturating_add(aged.results));
            aged_inspection_bytes_saved.set(
                aged_inspection_bytes_saved
                    .get()
                    .saturating_add(aged.bytes_saved),
            );
            tool_argument_shrinks.set(tool_argument_shrinks.get().saturating_add(shrunk.calls));
            tool_argument_bytes_saved.set(
                tool_argument_bytes_saved
                    .get()
                    .saturating_add(shrunk.bytes_saved),
            );
            duplicate_inspection_results =
                duplicate_inspection_results.saturating_add(deduped.results);
            duplicate_inspection_bytes_saved =
                duplicate_inspection_bytes_saved.saturating_add(deduped.bytes_saved);
            hist_tokens.recompute(history);
            // Splice/sync-compaction/prune/rolling-flush rewrote the prefix: the held hops
            // have been flushed and their passes just ran here.
            deferred_rewrite_hops.set(0);
            if rolling_flush {
                hop_breakers.push("rolling boundary flush");
            }
        }
        // A protected recent tail can itself exceed a small model's window:
        // compaction intentionally refuses to summarize those fresh messages.
        // Fit only their oldest tool payloads as a last local safeguard, with an
        // explicit rerun marker when tool trimming can actually reach the
        // target. An infeasible protected/schema floor preserves evidence;
        // provider hard limits still govern admission. This never adds a
        // tool/model hop or changes call pairing.
        // Root (C03g review): the emergency context-fit trim stays on under
        // cache-stable too — it only fires when the request is still over budget
        // after the compact boundary, and an overflowed request costs more than
        // one rare prefix-cache miss. Bulk reduction under the default is the
        // boundary (age_tool_results_at_boundary + compaction) above.
        let context_fit = fit_tool_results_to_budget(history, effective_budget, &defs);
        if context_fit > 0 {
            hop_breakers.push("context-fit trim");
            hist_tokens.recompute(history);
            let _ = events.send(TurnEvent::Notice(format!(
                "trimmed {context_fit} recent tool result(s) to fit the active context window"
            )));
        } else if !infeasible_context_notice_sent
            && effective_budget > 0
            && hist_tokens.observe(history) + cached_tool_schema_tokens > effective_budget
        {
            infeasible_context_notice_sent = true;
            let _ = events.send(TurnEvent::Notice(format!(
                "context target cannot be met by trimming tool results (target {effective_budget}); protected instructions, tool schemas and fresh evidence retained; provider limits still apply"
            )));
        }
        if bg_owner {
            maybe_start_bg_compact(
                history,
                effective_budget,
                compact_keep,
                compact_keep_tokens,
                &defs,
                registry,
                events,
            );
        }
        if task_capture.is_some() {
            deliver_job_completions(registry, history, events);
        }
        // Publish the live token gauge for the `get_context_remaining` tool —
        // count the tool schemas too, so "remaining" reflects the whole request.
        let hist_tok = hist_tokens.observe(history);
        registry
            .gauge
            .used_tokens
            .store(hist_tok + cached_tool_schema_tokens, Ordering::Relaxed);
        registry
            .gauge
            .budget_tokens
            .store(effective_budget, Ordering::Relaxed);
        // Submission watcher: one independent probe per hop, and only once a
        // real slot is in play. Ordinary coding hops do not open a status
        // source or scan for terminal rows.
        let watch_notify = if slot_watcher.slot_id().is_some() {
            if watch_source.is_none() {
                watch_source = open_configured_status_source(registry.current_workspace());
            }
            slot_watcher.poll(watch_source.as_mut())
        } else {
            None
        };
        if let Some(error) = watch_source
            .as_mut()
            .and_then(StatusSource::take_error_notice)
        {
            let _ = events.send(TurnEvent::Notice(error));
        }
        publish_slot_telemetry(
            events,
            &slot_watcher,
            competition,
            &mut published_slot_telemetry,
        );
        if let Some(notify) = watch_notify
            && watcher_announced.insert(notify.id.clone())
        {
            let _ = events.send(TurnEvent::Notice(notify.injection_text()));
            history.push(ChatMsg::harness(book::k_competition::watcher_turn(
                registry.current_workspace(),
                &notify,
            )));
        }
        // Stream the reply: forward each text delta to the UI as it arrives, and
        // let the club check `cancel` between chunks for a prompt mid-reply stop.
        //
        // Provider CLIs and gateways can occasionally fail before sending an
        // answer (empty stdout, transient transport reset, auth refresh race).
        // That used to surface as a dead-looking TUI turn and skipped the
        // experience ledger because `?` returned before the outcome path. Retry
        // while no answer text/tool call was emitted. Private reasoning alone is
        // not an answer and has performed no action, so it is safe to discard and
        // retry. A known incomplete HTTP stream may also replay: no tools
        // were dispatched, and SuppressPartial retracts its speculative text.
        let mut provider_attempt = 0usize;
        // provider_retries / provider_retry_backoff_ms captured once per turn (A7).
        let mut empty_reply_noted = false;
        let output_cap_notes = std::cell::Cell::new(0usize);
        let notes_workspace = registry.current_workspace().to_path_buf();
        // A reply that failed before it could act: `⠭⠉` output cap, `⠭⠙` all
        // reasoning, `⠭⠑` empty. A reply cut off at a fixed cap comes back
        // identical on a plain re-send; the route makes the retry different.
        let mut note_failed_reply =
            |error: &str, reasoning_only: bool, history: &mut Vec<ChatMsg>| {
                let Some(route) = book::x_execution::failed_reply(error, reasoning_only) else {
                    return;
                };
                if route == book::x_execution::EMPTY {
                    if empty_reply_noted {
                        return;
                    }
                    empty_reply_noted = true;
                } else if output_cap_notes.get() < book::x_execution::OUTPUT_CAP_LIMIT {
                    output_cap_notes.set(output_cap_notes.get() + 1);
                } else {
                    return;
                }
                let mut note = book::warpath(
                    &notes_workspace,
                    &[book::Raise::new(route, error.to_string())],
                );
                // The provider's own words for the cut-off ride beside the
                // stamp, as they did at 0.1.6.
                if route != book::x_execution::EMPTY {
                    note.push_str(&format!("\n({error})"));
                }
                history.push(ChatMsg::harness(note));
            };
        crate::agent::turn::phase::mark("context_assembled");
        let reply = loop {
            // A retry backoff can cross the existing turn deadline. Settle at
            // the owned boundary before opening another provider request.
            if cancel.load(Ordering::Acquire)
                || turn_expired(turn_start.elapsed().as_secs(), turn_budget)
            {
                continue 'turn;
            }
            // Preflight every actual provider attempt, including in-hop
            // retries. Provider counters are preferred after a call (and can
            // expose internal fan-out); the local request estimate is the
            // conservative fallback when a backend reports no usage.
            // Add reactive evidence and any real legend handoff before fitting
            // and checkpointing the exact provider request. Quiet turns only
            // update local metadata; they gain no message or prompt tokens.
            let before_advice = history.len();
            let handoff = book_continuity.prepare(history, &club.resolved_route_identity());
            live_research.poll(registry.current_workspace(), history, handoff);
            if let Some(notice) = registry
                .rl()
                .take_research_notice(registry.current_workspace())
            {
                history.push(ChatMsg::harness(notice));
            }
            if history.len() != before_advice {
                fit_tool_results_to_budget(history, effective_budget, &defs);
                hist_tokens.recompute(history);
            }
            let hist_tok = hist_tokens.observe(history);
            crate::agent::turn::phase::mark("history_checkpoint");
            if let Some(checkpoint) = history_checkpoint
                && let Err(error) = checkpoint(history)
            {
                let message = format!(
                    "provider request not started: could not durably checkpoint input: {error}"
                );
                let _ = events.send(TurnEvent::Notice(message.clone()));
                observed_failure!(
                    TurnFailure {
                        message,
                        stop_reason: TurnStopReason::CheckpointFailure,
                        hops: hop,
                        interrupted: false,
                        deadline_reached: false,
                        max_hops_reached: false,
                        acceptance: None,
                        rollout_id: None,
                    },
                    "checkpoint_failure"
                );
            }
            tool_schema_token_requests.set(
                tool_schema_token_requests
                    .get()
                    .saturating_add(cached_tool_schema_tokens),
            );
            let mut emitted_answer = false;
            let mut emitted_reasoning = false;
            // Friction = a hop whose every tool call errored, or an anti-spin
            // fingerprint that actually repeated (`spin >= 2`). `spin == 1` is
            // merely the first read-only batch of the turn — every normal
            // read-then-edit task has one — and treating it as friction pinned
            // `high` on every hop after the first (arena, 2026-09-05).
            let has_friction = errors.streak() > 0 || loops.spin() >= 2;
            // Explicit opt-in only. The former locked-GLM auto-arm flipped the
            // rung between hops (low → high), and z.ai keys its prefix cache on
            // the effort value: every flip re-prefilled the whole prompt
            // (measured: cached 0 after each flip, 3712/3771 when the rung
            // held). One rung per turn — the club's idle rung (Flash `auto`,
            // glm-5.3 `low`) or the operator pin — is both cheaper and cache-
            // stable. Reasoning-budget demotion below is also operator opt-in.
            let adaptive_armed = adaptive_reasoning;
            // `hop` is 1-based here (bumped at the top of the iteration); the
            // ladder's "hop 0 = intake at full effort" contract is 0-based.
            let mut effective_effort = if adaptive_armed {
                resolve_adaptive_reasoning_effort(club, hop.saturating_sub(1), has_friction)
            } else {
                None
            };
            // When explicitly configured, reasoning-budget demotion outranks
            // the adaptive baseline. No implicit budget lowers chosen effort.
            if let Some(demotion) = glm_thinking_burn_demotion(
                club,
                glm_reasoning_burn.get(),
                glm_last_hop_answered.get(),
                glm_burn_limit,
            ) {
                effective_effort = Some(demotion);
            }
            // This is the harness-owned policy-call boundary. Capture after all
            // history/schema transforms and immediately before the provider call.
            // The recorder has no API for private reasoning or provider headers.
            crate::agent::turn::phase::mark("policy_identity");
            let rollout_attempt =
                match rollout_recorder.begin_policy_attempt(history, &defs, || {
                    let mut ident = club.route_identity();
                    if let Some(effort) = effective_effort.as_ref() {
                        ident.reasoning_effort = Some(effort.clone());
                    }
                    ident
                }) {
                    Ok(attempt) => attempt,
                    Err(error) => {
                        record_rollout_error!("policy request", error);
                        None
                    }
                };
            // Hop-ledger snapshot: taken per attempt so a retried request's
            // sample covers only the attempt that actually answered.
            let hop_cache_before = hop_cache_ledger.then(|| {
                (
                    club.cache_usage(),
                    club.token_usage().map(|u| u.total_input).unwrap_or(0),
                )
            });
            let estimated_request_input =
                u64::try_from(hist_tok.saturating_add(cached_tool_schema_tokens))
                    .unwrap_or(u64::MAX);
            let metered_input_before = metered_sota.then(|| {
                club.token_usage()
                    .map(|usage| usage.total_input)
                    .unwrap_or(0)
            });
            auxiliary_scope.observe_recovery_context(history, &mut consumed_recovery_context);
            crate::agent::turn::phase::mark("verifier_preflight");
            super::run_identity::prepare_verifier(
                registry.current_workspace(),
                registry.external_evaluator_only,
                task_accept_cmd.as_deref(),
            );
            super::run_identity::prepare_turn(
                max_hops,
                effective_budget,
                compact_keep,
                compact_keep_tokens,
            );
            crate::agent::harness::trajectory::begin_model_request();
            let model_start_ms = turn_start.elapsed().as_millis();
            timing.call_start_ms = model_start_ms;
            timing.call_first_delta = None;
            timing.call_first_answer = None;
            timing.call_last_delta = None;
            timing.call_max_idle_ms = 0;
            let mut provider_not_started = false;
            // A connected seat inside this call (a mixture stage) reads this
            // workspace's ledger.
            let _ledger = book::connect::enter(registry.current_workspace());
            let result = with_turn_deadline_cancel(cancel, turn_deadline, |effective_cancel| {
                crate::agent::turn::phase::mark("bind_run_identity");
                club.bind_run_identity(effective_effort.as_deref())?;
                // Check after checkpointing/preflight and identity binding too:
                // those can consume the remaining wall before transport starts.
                if effective_cancel.load(Ordering::Acquire)
                    || turn_deadline.is_some_and(|deadline| {
                        let remaining = deadline.saturating_duration_since(Instant::now());
                        remaining.is_zero()
                    })
                {
                    provider_not_started = true;
                    return Err("provider request not started: turn deadline reached".into());
                }
                crate::agent::club::set_pending_responses_replay(None);
                crate::agent::turn::phase::mark("request_sent");
                match effective_effort.as_deref() {
                    Some(effort) => club.chat_streaming_with_effort(
                        history,
                        &defs,
                        Some(effort),
                        effective_cancel,
                        &mut |delta| match delta {
                            crate::agent::club::StreamDelta::Content(t) => {
                                if !t.is_empty() {
                                    timing.note_answer(turn_start.elapsed().as_millis());
                                }
                                emitted_answer = true;
                                let _ = events.send(TurnEvent::Token(t.to_string()));
                            }
                            crate::agent::club::StreamDelta::Reasoning(r) => {
                                if !r.is_empty() {
                                    timing.note_visible(turn_start.elapsed().as_millis());
                                }
                                emitted_reasoning = true;
                                let _ = events.send(TurnEvent::Reasoning(r.to_string()));
                            }
                            crate::agent::club::StreamDelta::Heartbeat => {
                                let _ = events.send(TurnEvent::Heartbeat);
                            }
                        },
                    ),
                    None => club.chat_streaming(history, &defs, effective_cancel, &mut |delta| {
                        match delta {
                            crate::agent::club::StreamDelta::Content(t) => {
                                if !t.is_empty() {
                                    timing.note_answer(turn_start.elapsed().as_millis());
                                }
                                emitted_answer = true;
                                let _ = events.send(TurnEvent::Token(t.to_string()));
                            }
                            crate::agent::club::StreamDelta::Reasoning(r) => {
                                if !r.is_empty() {
                                    timing.note_visible(turn_start.elapsed().as_millis());
                                }
                                emitted_reasoning = true;
                                let _ = events.send(TurnEvent::Reasoning(r.to_string()));
                            }
                            crate::agent::club::StreamDelta::Heartbeat => {
                                let _ = events.send(TurnEvent::Heartbeat);
                            }
                        }
                    }),
                }
            });
            // Every provider attempt (first try and in-hop retries alike) is
            // real time spent waiting on the model.
            crate::agent::harness::trajectory::end_model_request();
            crate::agent::turn::phase::mark("stream_done");
            let model_end_ms = turn_start.elapsed().as_millis();
            timing.note_model_wait(Duration::from_millis(
                (model_end_ms - model_start_ms) as u64,
            ));
            timing.note_model_span(model_start_ms, model_end_ms);
            crate::agent::harness::trajectory::note_task_timing(
                &timing.finish_with_history(turn_start.elapsed().as_millis(), history),
            );
            if provider_not_started {
                let _ = rollout_recorder.fail_policy_attempt(
                    rollout_attempt,
                    "provider request not started: turn deadline reached",
                    false,
                    false,
                );
                continue 'turn;
            }
            if let Some(input_before) = metered_input_before {
                let provider_reported = club
                    .token_usage()
                    .map(|usage| usage.total_input.saturating_sub(input_before))
                    .unwrap_or(0);
                let previous = metered_input_accounted;
                metered_input_accounted = metered_input_accounted
                    .saturating_add(provider_reported.max(estimated_request_input));
                if let Some(input_tokens) =
                    crossed_sota_input_milestone(previous, metered_input_accounted)
                {
                    let _ = events.send(TurnEvent::SpendMilestone { input_tokens });
                }
            }
            // Providers outside the HTTP adapter must obey the same answer
            // contract. Cancellation is handled at the owned turn boundary.
            let result = result.and_then(|reply| match reply {
                crate::agent::club::ClubReply::Text(ref text)
                    if text.trim().is_empty() && !cancel.load(Ordering::Relaxed) =>
                {
                    if emitted_answer {
                        let _ = events.send(TurnEvent::SuppressPartial);
                        emitted_answer = false;
                    }
                    Err("club returned an empty reply (no text and no tool calls)".to_string())
                }
                _ => Ok(reply),
            });
            match result {
                Ok(reply) => {
                    let resolved = club.resolved_route_identity();
                    super::run_identity::select_answer_route(&resolved);
                    if resolved.driver != last_answer_route.driver
                        || resolved.model != last_answer_route.model
                    {
                        crate::agent::harness::trajectory::note_route_switch(
                            hop,
                            &last_answer_route,
                            &resolved,
                        );
                        last_answer_route = resolved;
                    }
                    glm_last_hop_answered
                        .set(matches!(reply, crate::agent::club::ClubReply::Text(_)));
                    // Track reasoning for the optional operator budget.
                    // ANGEL_GLM_THINKING_BURN defaults to 0 (disabled).
                    if let Some(burn) = club
                        .token_usage()
                        .map(|u| u.last_reasoning)
                        .filter(|burn| *burn > 0)
                    {
                        glm_reasoning_burn.set(glm_reasoning_burn.get().saturating_add(burn));
                    }
                    if let Err(error) =
                        rollout_recorder.complete_policy_attempt(rollout_attempt, &reply, || {
                            let mut ident = club.resolved_route_identity();
                            if let Some(effort) = effective_effort.as_ref() {
                                ident.reasoning_effort = Some(effort.clone());
                            }
                            ident
                        })
                    {
                        record_rollout_error!("policy response", error);
                    }
                    // Fold this hop's cache economics into the per-route
                    // ledger. `reported` is a read-accounting delta, so a
                    // cache-blind backend records totals without faking a 0%
                    // rate; a genuine hit-rate drop names its breakers.
                    if let Some((cache_snap, input_snap)) = hop_cache_before {
                        let cache_now = club.cache_usage();
                        let input_now = club
                            .token_usage()
                            .map(|u| u.total_input)
                            .unwrap_or(input_snap);
                        let sample = crate::agent::turn::HopCacheSample {
                            hop,
                            input: input_now.saturating_sub(input_snap),
                            cache_read: cache_now
                                .read_input_tokens
                                .saturating_sub(cache_snap.read_input_tokens),
                            cache_write: cache_now
                                .write_input_tokens
                                .saturating_sub(cache_snap.write_input_tokens),
                            reported: cache_now.read_accounting_responses
                                > cache_snap.read_accounting_responses,
                            breakers: std::mem::take(&mut hop_breakers),
                        };
                        if let Some(notice) =
                            crate::agent::turn::cache_ledger_fold(club.label(), sample)
                        {
                            let _ = events.send(TurnEvent::Notice(notice));
                        }
                    }
                    if turn_expired(turn_start.elapsed().as_secs(), turn_budget) {
                        // A completed research reply is evidence we already have.
                        // Retain it before settling at the deadline boundary; do
                        // not execute late tool calls or manufacture an answer
                        // from an incomplete stream or private reasoning.
                        if research_turn
                            && let crate::agent::club::ClubReply::Text(answer) = &reply
                            && !answer.trim().is_empty()
                            && !crate::agent::club::contains_raw_tool_markup(answer)
                        {
                            history.push(ChatMsg::assistant(research::finalize(answer)));
                        }
                        if emitted_answer || emitted_reasoning {
                            let _ = events.send(TurnEvent::SuppressPartial);
                        }
                        continue 'turn;
                    }
                    break reply;
                }
                Err(err) => {
                    // A cancelled operation belongs to the turn's stop boundary,
                    // even if the provider reports its partial stream as an error.
                    // Do not classify that failed attempt as retryable in the rollout.
                    let attempt_cancelled = || {
                        cancel.load(Ordering::Acquire)
                            || turn_expired(turn_start.elapsed().as_secs(), turn_budget)
                    };
                    // Every no-terminal-event stream death is retryable, not only
                    // the ones the transport happened to label
                    // `INCOMPLETE_STREAM_ERR`: a stall observed mid-answer used to
                    // be fatal on its first occurrence while the identical fault
                    // behind keep-alives recovered, purely by error string.
                    let incomplete_stream = is_recoverable_stream_error(&err);
                    // Streamed private reasoning and no answer text: a cut-off
                    // here means the reply was spent thinking.
                    let reasoning_only_reply = emitted_reasoning && !emitted_answer;
                    if incomplete_stream && !attempt_cancelled() {
                        trajectory::note_stream_cut(hop);
                    }
                    // One disposition drives the rollout ledger, the retry gate,
                    // and the terminal message, so the receipt can never claim a
                    // retryability the loop did not honor. A permanent failure
                    // outranks the recoverable spelling even when both match.
                    let retry_allowed = !attempt_cancelled()
                        && retryable_provider_failure(&err, emitted_answer, incomplete_stream);
                    if let Err(capture_error) = rollout_recorder.fail_policy_attempt(
                        rollout_attempt,
                        &err,
                        emitted_answer,
                        retry_allowed,
                    ) {
                        record_rollout_error!("provider failure", capture_error);
                    }
                    if attempt_cancelled() {
                        if emitted_answer || emitted_reasoning {
                            let _ = events.send(TurnEvent::SuppressPartial);
                        }
                        continue 'turn;
                    }
                    if !emitted_answer
                        && !incomplete_stream
                        && !cancel.load(Ordering::Relaxed)
                        && is_context_overflow_error(&err)
                    {
                        request_overflows.set(request_overflows.get().saturating_add(1));
                    }
                    if !emitted_answer
                        && !incomplete_stream
                        && !cancel.load(Ordering::Relaxed)
                        && is_context_overflow_error(&err)
                        && overflow_recoveries < max_overflow_recoveries
                        && effective_budget > 0
                    {
                        let before = context_tokens(history, &defs);
                        let recovery_budget =
                            effective_budget.min(before.saturating_mul(3) / 4).max(256);
                        let compacted = maybe_compact_for_turn(
                            club,
                            history,
                            recovery_budget,
                            compact_keep,
                            compact_keep_token_target.min(recovery_budget / 2),
                            &defs,
                            registry,
                            events,
                        );
                        let fitted = fit_tool_results_to_budget(history, recovery_budget, &defs);
                        let after = context_tokens(history, &defs);
                        if (compacted || fitted > 0) && after < before {
                            // Provider truth outranks our stale/unknown metadata
                            // for the rest of this turn. Keep the recovered
                            // ceiling so later hops compact before repeating the
                            // same oversized request.
                            effective_budget = effective_budget.min(recovery_budget);
                            compact_keep_tokens = if compact_keep_token_target == 0 {
                                0
                            } else {
                                compact_keep_token_target.min(effective_budget / 2)
                            };
                            // The rebuilt retry sends a rewritten history: a
                            // known prefix breaker for its own hop sample.
                            hop_breakers.push("overflow compaction");
                            overflow_recoveries += 1;
                            if emitted_reasoning {
                                let _ = events.send(TurnEvent::SuppressPartial);
                            }
                            let _ = events.send(TurnEvent::Notice(format!(
                                "provider rejected an oversized context; compacted and rebuilt the request ({before} → {after} estimated tokens, active budget {effective_budget}, recovery {overflow_recoveries}/{max_overflow_recoveries})"
                            )));
                            continue;
                        }
                    }
                    let mut stop_after_session_recovery = false;
                    if !emitted_answer
                        && !incomplete_stream
                        && !cancel.load(Ordering::Relaxed)
                        && session_recoveries < session_recovery_budget
                        && !is_permanent_provider_error(&err)
                        && let Some((fault, note)) = recover_session(
                            club,
                            registry,
                            history,
                            &err,
                            hop,
                            effective_budget,
                            compact_keep,
                            compact_keep_tokens,
                            &defs,
                            events,
                        )
                    {
                        session_recoveries += 1;
                        hop_breakers.push("teacher-watch");
                        if emitted_reasoning {
                            let _ = events.send(TurnEvent::SuppressPartial);
                        }
                        history.push(ChatMsg::harness(book::warpath(
                            registry.current_workspace(),
                            &[book::Raise::new(book::x_execution::SESSION, note.clone())],
                        )));
                        let _ = events.send(TurnEvent::Notice(note.clone()));
                        if fault.retries_same_club()
                            && retry_budget_allows(provider_retries, provider_attempt)
                        {
                            provider_attempt = provider_attempt.saturating_add(1);
                            note_failed_reply(&err, reasoning_only_reply, history);
                            continue;
                        }
                        // Recovery notes are harness diagnostics, never an
                        // answer. Seal the same failure ledger/envelope as an
                        // exhausted cloud request, including local-seat deaths.
                        stop_after_session_recovery = true;
                    }
                    // Once every output-cap note is spent, a re-send is the same
                    // request against the same cap and fails the same way (GLM
                    // polyglot-v1 py-two-bucket: about three minutes of reasoning
                    // per cut-off, until the wall ran out).
                    let output_cap_exhausted = book::x_execution::is_output_cap_truncation(&err)
                        && output_cap_notes.get() >= book::x_execution::OUTPUT_CAP_LIMIT;
                    if !cancel.load(Ordering::Relaxed)
                        && !stop_after_session_recovery
                        && retry_allowed
                        && !output_cap_exhausted
                        && retry_budget_allows(provider_retries, provider_attempt)
                    {
                        provider_attempt = provider_attempt.saturating_add(1);
                        if emitted_reasoning || emitted_answer {
                            // Retries replace an incomplete reasoning trace; do
                            // not leave the failed attempt looking live in the UI.
                            let _ = events.send(TurnEvent::SuppressPartial);
                        }
                        // An empty 200 can be deterministic: a greedy local
                        // backend fed identical bytes fails identically. One
                        // transient re-prompt changes the token stream so the
                        // retry is not a pure replay.
                        note_failed_reply(&err, reasoning_only_reply, history);
                        let failure_stage = if incomplete_stream {
                            "provider stream incomplete"
                        } else {
                            "provider call failed before output"
                        };
                        let retry_plan = match provider_retries {
                            Some(limit) => format!("retrying {provider_attempt}/{limit}"),
                            None => format!(
                                "retrying attempt {provider_attempt} \
                                 (ANGEL_PROVIDER_RETRIES unset: unbounded)"
                            ),
                        };
                        let _ = events.send(TurnEvent::Notice(format!(
                            "{failure_stage}; {retry_plan}: {err}"
                        )));
                        // Give a struggling backend a beat before the retry,
                        // sliced so a user cancel still lands promptly.
                        let backoff_started = Instant::now();
                        let backoff = Duration::from_millis(
                            provider_retry_backoff_ms
                                .saturating_mul(provider_attempt)
                                .min(2_000) as u64,
                        );
                        let slice = Duration::from_millis(50);
                        let mut waited = Duration::ZERO;
                        while waited < backoff && !cancel.load(Ordering::Relaxed) {
                            std::thread::sleep(slice.min(backoff - waited));
                            waited += slice;
                        }
                        // The backoff sleep is model wait time too: the turn
                        // is idle waiting on the provider between attempts, so
                        // it lands in `model_ms` (and leaves `other_ms`)
                        // rather than masquerading as harness overhead. The
                        // retry slice and count split out for the receipt.
                        timing.note_model_retry_wait(backoff_started.elapsed());
                        if cancel.load(Ordering::Relaxed) {
                            // Re-enter the owned hop boundary instead of
                            // issuing one last provider call after Esc landed
                            // during backoff. Some simple/custom clubs use the
                            // default streaming implementation and do not
                            // observe `cancel` themselves.
                            continue 'turn;
                        }
                        continue;
                    }
                    crate::agent::harness::trajectory::note_escalation(hop, "provider_unavailable");
                    crate::agent::harness::trajectory::note_stop_reason(
                        TurnStopReason::ProviderError.as_str(),
                    );
                    // Seal the progress ledger even when no model answer exists.
                    // Failed transport never earns a verifier reward or answer.
                    log_trajectory(club, history, "", hop, false, None);
                    eprintln!(
                        "[turn] {} provider call failed on hop {hop} after {} retry attempt(s): {err}",
                        club.label(),
                        provider_attempt
                    );
                    write_exp(
                        "provider_error",
                        false,
                        hop,
                        turn_counters!(
                            markup_replies,
                            loops.spin(),
                            errors.streak(),
                            duplicate_inspection_results,
                            duplicate_inspection_bytes_saved,
                            action_capsule_metrics,
                        ),
                    );
                    // Name the real reason this hop stopped retrying: the caller
                    // reads this line as the recovery receipt, so a retry count
                    // that never ran must not be blamed for a permanent error or
                    // for text that had already been streamed.
                    let retry_disposition = if stop_after_session_recovery {
                        "not retried: the long-session recovery latch owned this fault".to_string()
                    } else if is_permanent_provider_error(&err) {
                        "not retried: permanent provider error (authentication, configuration, \
                         or exhausted quota)"
                            .to_string()
                    } else if !retry_allowed {
                        "not retried: visible text was already streamed for this hop".to_string()
                    } else if output_cap_exhausted {
                        format!(
                            "not retried: {} replies in a row were cut off at \
                             the output cap after being told so; a re-send would repeat it",
                            book::x_execution::OUTPUT_CAP_LIMIT
                        )
                    } else {
                        match provider_retries {
                            Some(limit) => format!(
                                "provider retry limit ANGEL_PROVIDER_RETRIES={limit} exhausted"
                            ),
                            None => "provider retries are unbounded when ANGEL_PROVIDER_RETRIES \
                                     is unset"
                                .to_string(),
                        }
                    };
                    observed_failure!(
                        TurnFailure {
                            message: format!("{err}; {retry_disposition}"),
                            stop_reason: TurnStopReason::ProviderError,
                            hops: hop,
                            interrupted: false,
                            deadline_reached: false,
                            max_hops_reached: false,
                            acceptance: None,
                            rollout_id: None,
                        },
                        "provider_error"
                    );
                }
            }
        };
        let _reply_model =
            super::run_identity::LiveModelScope::enter(club.resolved_route_identity().model);
        // Scavenge (opt-in): a reasoning model can finish a hop with an empty
        // `tool_calls` array while the call itself sits in the answer text. Only
        // the accumulated assistant text is available here — private reasoning
        // is streamed to the UI, never retained — so that is what is scanned.
        let reply = match reply {
            crate::agent::club::ClubReply::Text(answer)
                if toolcall_scavenge && !defs.is_empty() =>
            {
                let recovered = crate::agent::club::scavenge_stranded_tool_calls(&answer, &defs);
                if recovered.is_empty() {
                    crate::agent::club::ClubReply::Text(answer)
                } else {
                    let names = recovered
                        .iter()
                        .map(|call| call.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    // The text was streamed as if it were the answer; retract it
                    // before the recovered calls dispatch.
                    let _ = events.send(TurnEvent::SuppressPartial);
                    let _ = events.send(TurnEvent::Notice(format!(
                        "scavenge: recovered {names} call{} from prose",
                        if recovered.len() == 1 { "" } else { "s" }
                    )));
                    crate::agent::club::ClubReply::Calls(recovered)
                }
            }
            reply => reply,
        };
        match reply {
            crate::agent::club::ClubReply::Text(answer) => {
                // The answer's own reasoning, for a route that replays it
                // (DeepSeek's thinking mode wants every reasoned turn back).
                let answer_reasoning = crate::agent::club::take_pending_tool_reasoning();
                // Self-report escalation: the marker is a control token, not an
                // answer, so it is read before any answer policy sees the text.
                // Inert at `Off`, which is every unarmed turn.
                let answer = match read_self_report(&answer, tier) {
                    SelfReport::None => answer,
                    SelfReport::Escalate(reason) => {
                        // The marker streamed as if it were the answer: retract
                        // it, and leave history untouched so the escalation seat
                        // is handed exactly the request this seat declined —
                        // nothing about the aborted attempt goes upstream.
                        let _ = events.send(TurnEvent::SuppressPartial);
                        write_exp(
                            "needs_pro",
                            false,
                            hop,
                            turn_counters!(
                                markup_replies,
                                loops.spin(),
                                errors.streak(),
                                duplicate_inspection_results,
                                duplicate_inspection_bytes_saved,
                                action_capsule_metrics,
                            ),
                        );
                        observed_outcome!(
                            TurnOutcome::stopped(
                                reason.unwrap_or_default(),
                                TurnStopReason::NeedsPro,
                                hop
                            ),
                            "needs_pro"
                        );
                    }
                    SelfReport::Ignored(rest) => {
                        let _ = events.send(TurnEvent::SuppressPartial);
                        let _ = events.send(TurnEvent::Notice(format!(
                            "needs-pro: already on {}, marker ignored",
                            club.label()
                        )));
                        rest
                    }
                };
                // Stop checkpoint (`book/q_stop.rs`). Every measurement the harness
                // owns still runs here — post-write verification, the pinned
                // acceptance command, the confirming re-run, background-job state —
                // but none of them can deny the answer. Their findings are stop
                // facts; the first answer that carries any is shown them once as a
                // warpath ending in `⠟⠁`, and every later answer stands.
                let mut stop_facts: Vec<book::Raise> = Vec::new();
                let mut add_fact = |raise: book::Raise| {
                    if !stop_facts.iter().any(|seen| seen.route == raise.route) {
                        stop_facts.push(raise);
                    }
                };
                let final_check_notes =
                    with_turn_deadline_cancel(cancel, turn_deadline, |verify_cancel| {
                        finish_post_write_verification(
                            registry,
                            club,
                            hop,
                            &mut post_write_verification,
                            &mut verdicts,
                            &mut rollout_recorder,
                            verify_cancel,
                        )
                    });
                if !final_check_notes.is_empty() {
                    add_fact(book::Raise::new(
                        book::v_verification::POST_WRITE,
                        final_check_notes.join("\n\n"),
                    ));
                }
                // A long provider call can race a background build's exit:
                // deliver the completions, and a failed one is a fact.
                if task_capture.is_some() && deliver_job_completions(registry, history, events) {
                    add_fact(book::Raise::new(book::p_processes::FAILED, None));
                }
                // A headless answer shuts down its process registry; jobs the
                // task still owns stop with it. Say so; never wait on them.
                let proc_owner = cancel as *const AtomicBool as usize;
                let proc_workspace = &registry.workspace_boundary().canonical_root;
                if task_capture.is_some()
                    && crate::agent::tools::proc::owned_work_pending(proc_owner, proc_workspace)
                {
                    add_fact(book::Raise::new(book::p_processes::LIVE, None));
                }
                // Raw tool markup in a text answer: nothing in it ran.
                if !defs.is_empty() && crate::agent::club::contains_raw_tool_markup(&answer) {
                    markup_replies += 1;
                    add_fact(book::Raise::new(book::x_execution::MARKUP, None));
                }
                let current_workspace_fingerprint =
                    workspace_fingerprint(registry.current_workspace());
                let current_acceptance_workspace = task_accept_cmd
                    .as_ref()
                    .and_then(|_| workspace_evidence_sha256(registry.current_workspace()));
                let workspace_changed_after_verification =
                    match (initial_workspace_fingerprint, current_workspace_fingerprint) {
                        (Some(initial), Some(current)) => {
                            current != initial
                                && verification_attempt_workspace_fingerprint != Some(current)
                                && prose_only_workspace_fingerprint != Some(current)
                        }
                        _ => false,
                    };
                // The evaluator-pinned acceptance command is measured, never a
                // veto: red is `⠧⠃`, flaky `⠧⠑`, green is recorded.
                if let Some(command) = task_accept_cmd.as_deref() {
                    let last_result = accept_last_post_result.or(accept_baseline_result);
                    let reuse_unchanged_red = current_acceptance_workspace.is_some()
                        && current_acceptance_workspace == accept_last_checked_workspace
                        && last_result.is_some_and(|result| result != "passed")
                        && accept_last_summary.is_some();
                    let (passed, fact) = if reuse_unchanged_red {
                        let route = if last_result == Some("flaky") {
                            book::v_verification::FLAKY
                        } else {
                            book::v_verification::ACCEPTANCE
                        };
                        let summary = accept_last_summary
                            .clone()
                            .unwrap_or_else(|| "task acceptance failed".into());
                        (false, Some(book::Raise::new(route, summary)))
                    } else {
                        let proof = book::v_verification::run_task_accept(
                            command,
                            registry.current_workspace(),
                        );
                        accept_post_checks = accept_post_checks.saturating_add(1);
                        accept_post_ms = accept_post_ms.saturating_add(proof.elapsed_ms);
                        accept_last_post_result = Some(proof.result_class);
                        accept_last_summary = Some(proof.summary.clone());
                        let workspace_after_gate =
                            workspace_evidence_sha256(registry.current_workspace());
                        accept_last_checked_workspace =
                            match (current_acceptance_workspace.clone(), workspace_after_gate) {
                                (Some(before), Some(after)) if before == after => Some(after),
                                _ => None,
                            };
                        (proof.passed, proof.fact())
                    };
                    if passed {
                        crate::agent::harness::trajectory::note_verified(
                            turn_start.elapsed().as_millis() as u64,
                        );
                        verification_needed = false;
                        verification_attempt_workspace_fingerprint = current_workspace_fingerprint;
                        untested.note_attempt(current_workspace_fingerprint);
                        green_verify_achieved = true;
                        let _ = events.send(TurnEvent::Notice(format!(
                            "task acceptance passed at finalization: {}",
                            accept_last_summary.as_deref().unwrap_or("")
                        )));
                    } else if let Some(fact) = fact {
                        let _ = events.send(TurnEvent::Notice(format!(
                            "task acceptance remains red at answer: {}",
                            accept_last_summary.as_deref().unwrap_or("")
                        )));
                        add_fact(fact);
                    }
                }
                // One pass can be luck: repeat the model's last green run on the
                // unchanged code before its first stop. Once the checkpoint is
                // spent the answer stands, so the re-run would buy nothing.
                let confirm_runs = if confirm_green_runs > 0 {
                    confirm_green_runs
                } else if confirm_green_on_chance
                    && last_green_run.is_some()
                    && book::v_verification::edits_depend_on_chance(
                        registry.current_workspace(),
                        &edited_paths,
                    )
                {
                    book::v_verification::CHANCE_CONFIRM_GREEN_RUNS
                } else {
                    0
                };
                if confirm_runs > 0
                    && !checkpoint.spent()
                    && last_green_run.as_ref().is_some_and(|green| {
                        green.workspace.is_some()
                            && green.workspace == current_workspace_fingerprint
                    })
                {
                    let green = last_green_run.take().expect("checked above");
                    let label = book::v_verification::run_label(&green.call);
                    // A substitute runner has not run yet: it owes one more run.
                    let runs = confirm_runs + usize::from(green.substitute);
                    match book::v_verification::confirm_green_run(
                        registry,
                        &green.call,
                        runs,
                        cancel,
                    ) {
                        Ok(runs) => {
                            let _ = events.send(TurnEvent::Notice(format!(
                                "confirmed green: re-ran {label} {runs} more time(s) on the final code"
                            )));
                        }
                        Err((run, output)) => {
                            crate::agent::harness::trajectory::note_escalation(
                                hop,
                                "confirm_green_flaky",
                            );
                            add_fact(book::Raise::new(
                                book::v_verification::FLAKY,
                                format!(
                                    "Re-run {run} of {runs} of {label} failed:\n{}",
                                    book::v_verification::tail_chars(
                                        &output,
                                        book::v_verification::RUN_TAIL_CHARS
                                    )
                                ),
                            ));
                        }
                    }
                }
                // A test file that came with the task was changed: a pass that
                // leans on that change says nothing about the code.
                if track_task_facts
                    && task_accept_cmd.is_none()
                    && let Some(fact) = book::v_verification::tests_edited_fact(
                        registry.current_workspace(),
                        &tests_changed_at_start,
                    )
                {
                    add_fact(fact);
                }
                // The model's own last test run on this exact code was red.
                if track_task_facts
                    && task_accept_cmd.is_none()
                    && let Some(red) = last_red_run.as_ref().filter(|red| {
                        red.workspace.is_some() && red.workspace == current_workspace_fingerprint
                    })
                {
                    add_fact(red.fact());
                }
                let verification_outstanding = verification_needed
                    || workspace_changed_after_verification
                    || registry.mutation_targets.opaque_generation() > attempted_opaque_generation;
                if track_task_facts
                    && let Some(fact) = untested.fact(
                        initial_workspace_fingerprint,
                        current_workspace_fingerprint,
                        prose_only_workspace_fingerprint,
                    )
                {
                    add_fact(fact);
                }
                // An answer that claims progress with no edit (armed like 0.1.6:
                // an operator opt-in, with a first-write limit).
                let workspace_changed =
                    match (initial_workspace_fingerprint, current_workspace_fingerprint) {
                        (Some(initial), Some(current)) => current != initial,
                        _ => false,
                    };
                if no_edit_guard && !mutation_seen && !workspace_changed && !green_verify_achieved {
                    add_fact(book::Raise::new(book::d3456_advisories::NO_EDIT, None));
                }
                for raise in &loop_facts {
                    add_fact(book::Raise::new(raise.route, None));
                }
                // Research turns end on their own answer, unchecked.
                let left = (!research_turn)
                    .then(|| {
                        book::q_stop::task_budget_left(
                            hop,
                            max_hops,
                            turn_start.elapsed().as_secs(),
                            task_wall_secs,
                        )
                    })
                    .flatten();
                if let Some((cells, shown)) =
                    checkpoint.engage(registry.current_workspace(), &stop_facts, left.is_some())
                {
                    crate::agent::harness::trajectory::note_hop_stamps(
                        &shown.iter().map(|raise| raise.route).collect::<Vec<_>>(),
                    );
                    crate::agent::harness::trajectory::note_escalation(hop, "stop_checkpoint");
                    // The streamed answer is not final yet: retract it from the
                    // live pane, keep it in the model tail as the claim the
                    // checkpoint is about.
                    let _ = events.send(TurnEvent::SuppressPartial);
                    history.push(ChatMsg::assistant_with_reasoning(answer, answer_reasoning));
                    history.push(ChatMsg::harness(book::q_stop::stop_turn(
                        &cells,
                        &shown,
                        left.as_deref(),
                    )));
                    let _ = events.send(TurnEvent::Notice(format!(
                        "stop checkpoint {cells} · {}",
                        book::names(&shown)
                    )));
                    continue;
                }
                if !stop_facts.is_empty() {
                    let _ = events.send(TurnEvent::Notice(format!(
                        "answer stands · {}",
                        book::names(&stop_facts)
                    )));
                }
                // The ledger still records an unverified claim shipping.
                if verification_outstanding {
                    unverified_completion_claims
                        .set(unverified_completion_claims.get().saturating_add(1));
                }
                // Optional final-answer advisor (ANGEL_ADVISOR=1|final|hops).
                // SwarmClub may already have annotated; skip if so.
                let answer = apply_final_advisor(club, registry, history, answer);
                let answer = if research_turn {
                    let rendered = research::finalize(&answer);
                    if rendered != answer {
                        let _ = events.send(TurnEvent::SuppressPartial);
                    }
                    rendered
                } else {
                    answer
                };
                // Angel's Share: barrel the final-hop context + answer when
                // this club is an armed teacher (default-off, non-blocking —
                // see barrel.rs). Called before the answer is pushed so the
                // captured context never duplicates it.
                crate::agent::harness::trajectory::note_research_answer(&answer);
                crate::agent::harness::trajectory::note_task_answer();
                crate::knowledge::barrel::capture_answer(
                    club,
                    registry.current_workspace(),
                    history,
                    &answer,
                );
                history.push(ChatMsg::assistant_with_reasoning(
                    answer.clone(),
                    answer_reasoning,
                ));
                crate::agent::harness::trajectory::note_task_timing(
                    &timing.finish_with_history(turn_start.elapsed().as_millis(), history),
                );
                if research_turn && turn_expired(turn_start.elapsed().as_secs(), turn_budget) {
                    crate::agent::harness::trajectory::note_stop_reason(
                        TurnStopReason::Deadline.as_str(),
                    );
                    log_trajectory(club, history, &answer, hop, true, verdicts.reward());
                    observed_outcome!(
                        TurnOutcome::stopped(answer, TurnStopReason::Deadline, hop)
                            .with_stop_notice(format!("Turn deadline reached after {hop} hop(s).")),
                        "deadline"
                    );
                }
                crate::agent::harness::trajectory::note_stop_reason(
                    TurnStopReason::Answer.as_str(),
                );
                log_trajectory(club, history, &answer, hop, false, verdicts.reward());
                file_report(
                    club,
                    registry.current_workspace(),
                    history,
                    &answer,
                    &registry.store,
                    &registry.session_id,
                );
                write_exp(
                    "answer",
                    true,
                    hop,
                    turn_counters!(
                        markup_replies,
                        loops.spin(),
                        errors.streak(),
                        duplicate_inspection_results,
                        duplicate_inspection_bytes_saved,
                        action_capsule_metrics,
                    ),
                );
                observed_outcome!(TurnOutcome::answer(answer, hop), "answer");
            }
            crate::agent::club::ClubReply::Calls(mut calls) => {
                // Routes this hop raises; they ride the tail of its last tool
                // result as one warpath, never a message of their own.
                let mut raised: Vec<book::Raise> = Vec::new();
                cues.observe_calls(calls.iter().map(|call| call.name.as_str()));
                if let Some(origin) = research_origin.as_deref() {
                    for call in &mut calls {
                        if call.name == "web_search"
                            && let Some(args) = call.args.as_object_mut()
                        {
                            args.insert("_research_origin".into(), Value::String(origin.into()));
                        }
                    }
                }
                // Provider-private reasoning is a one-reply handoff. Consume it
                // before call-ID normalization and attach it only to this exact
                // live assistant message; serde deliberately skips it.
                let private_reasoning = crate::agent::club::take_pending_tool_reasoning();
                let tool_content = crate::agent::club::take_pending_tool_content();
                let responses_replay = crate::agent::club::take_pending_responses_replay();
                let repaired_call_ids = normalize_tool_call_ids(&mut calls, history, hop);
                if repaired_call_ids > 0 {
                    let _ = events.send(TurnEvent::Notice(format!(
                        "tool-call identity repair: normalized {repaired_call_ids}/{} blank, duplicate, or reused provider ID(s) on hop {hop}",
                        calls.len()
                    )));
                }
                let hop_path_active = slot_watcher.hop_path_active(competition);
                let (
                    mutation_this_hop,
                    outcome_this_hop,
                    wait_or_progress_this_hop,
                    burns_budget_this_hop,
                ) = hop_budget_flags_for_loop(
                    hop_budget_classify_applied(competition, hop_path_active),
                    &calls,
                );
                let mut cadence_verdict = None;
                if hop_path_active {
                    let inflight_kind = classify_inflight_hop(&calls);
                    if calls.iter().any(is_local_preflight_call) {
                        preflight_seen_this_turn = true;
                    }
                    let just_notified = slot_watcher.take_just_notified();
                    let inflight_verdict =
                        evaluate_inflight_hop(inflight_kind, slot_watcher.phase(), just_notified);
                    cadence_verdict = Some(inflight_verdict);
                    publish_slot_telemetry(
                        events,
                        &slot_watcher,
                        competition,
                        &mut published_slot_telemetry,
                    );
                    if inflight_verdict.is_fail() {
                        let phase = slot_watcher.phase();
                        let next = next_required_action(phase, false);
                        let _ = events.send(TurnEvent::Notice(format!(
                            "submission cadence failure ({inflight_verdict:?}); slot={phase:?}; next required action: {next:?}"
                        )));
                    } else if just_notified {
                        let next = next_required_action(slot_watcher.phase(), false);
                        let _ = events.send(TurnEvent::Notice(format!(
                            "watcher terminal consumed; next required action: {next:?}"
                        )));
                    }
                    if calls
                        .iter()
                        .any(|c| !runner_escalation_allowed(preflight_seen_this_turn, c))
                    {
                        let _ = events.send(TurnEvent::Notice(
                            "runner waste: local preflight required before runner dispatch".into(),
                        ));
                    }
                }
                let _ = cadence_verdict;
                if calls.iter().any(|c| c.id.starts_with("prose_")) {
                    let _ = events.send(TurnEvent::SuppressPartial);
                }
                // Loop detectors see the batch as issued (`book/l_loops.rs`). A
                // pure competition board wait/poll does not count: its outcome
                // can change under the same call.
                let loop_batch = loops.before_dispatch(
                    &calls,
                    book::l_loops::anti_spin_counts_batch(
                        mutation_this_hop,
                        outcome_this_hop,
                        wait_or_progress_this_hop,
                        burns_budget_this_hop,
                    ),
                    &mut raised,
                );
                raised.extend(mutation_thrash.observe(&calls));
                // Measure actual file state for potentially mutating batches;
                // successful opaque shell diagnostics alone are not progress.
                let progress_bytes_before =
                    trajectory::mutation_byte_snapshot(registry.current_workspace(), &calls);
                let progress_workspace_before = calls
                    .iter()
                    .any(|call| {
                        is_mutation_call(call) || matches!(call.name.as_str(), "shell" | "proc_run")
                    })
                    .then(|| workspace_fingerprint(registry.current_workspace()))
                    .flatten();
                let acceptance_workspace_before = task_accept_cmd
                    .as_ref()
                    .and_then(|_| workspace_evidence_sha256(registry.current_workspace()));
                // Action capsules inspect only already-parsed arguments. The
                // preview never calls a model, reads a file, shells out, or
                // enters model history. In approve mode one decision covers
                // the whole model-emitted batch, avoiding modal-per-file drag.
                let action_batch = if action_capsule_mode.active() {
                    let capsule_started = Instant::now();
                    let batch = ActionBatch::from_calls(
                        &calls,
                        action_capsule_mode,
                        registry.current_workspace(),
                    );
                    if let Some(batch) = &batch {
                        action_capsule_metrics
                            .note_preflight(capsule_started.elapsed(), batch.count());
                    }
                    batch
                } else {
                    // Do not even walk the batch on the default path. The only
                    // remaining work is the existing footprint scheduler below.
                    None
                };
                let deny_actions = if let Some(batch) = &action_batch {
                    let _ = events.send(TurnEvent::Notice(batch.preview_notice()));
                    if action_capsule_mode.needs_approval() {
                        let wait_started = Instant::now();
                        let decision = crate::agent::approval::ask(
                            crate::agent::approval::ApprovalScope::ActionBatch(
                                batch.approval_key().to_string(),
                            ),
                            &batch.approval_prompt(),
                        );
                        // UI wait is intentionally not mixed into local
                        // preflight overhead: the ledger can distinguish a
                        // cheap preview from an operator deliberation.
                        action_capsule_metrics.note_approval_wait(wait_started.elapsed());
                        let denied = matches!(decision, crate::agent::approval::Decision::Deny);
                        if denied {
                            action_capsule_metrics.note_denied(batch.count());
                            let _ = events.send(TurnEvent::Notice(format!(
                                "action capsule denied · {} scoped operation(s) were not dispatched",
                                batch.count()
                            )));
                        }
                        denied
                    } else {
                        false
                    }
                } else {
                    false
                };
                // Parallel tool calls: when the model batches tools whose
                // filesystem footprints don't conflict, run them concurrently — the
                // Codex (parallel.rs read/write gate) + Hermes (path-overlap) win.
                // Read-only batches parallelize as before; additionally, writes to
                // *disjoint* paths run together (a write never races a read that
                // could observe it). Effectful/conflicting calls form strict serial
                // barriers; safe consecutive runs on either side still parallelize.
                // Results are pushed to `history` in call order regardless, so each
                // tool_call_id pairs correctly.
                // An approval modal must never race another prompt; observe
                // mode retains the existing disjoint-write parallelism.
                // The parallel/segmented dispatchers have no view of the storm
                // map, so a batch carrying a suppressed duplicate runs serially.
                // Hook commands are arbitrary side effects (locks, ledgers,
                // approvals, external counters) that the filesystem footprint
                // planner cannot see. A configured hook therefore makes the
                // whole emitted batch a serial barrier.
                crate::agent::turn::phase::mark("tool_checkpoint");
                if let Some(checkpoint) = history_checkpoint {
                    let mut durable_prefix = history.clone();
                    durable_prefix.push(ChatMsg::assistant_calls_with_reasoning(
                        calls.clone(),
                        private_reasoning.clone(),
                    ));
                    if let Err(error) = checkpoint(&durable_prefix) {
                        let message = format!(
                            "tool batch not started: could not durably checkpoint intent: {error}"
                        );
                        let _ = events.send(TurnEvent::Notice(message.clone()));
                        write_exp(
                            "checkpoint_failure",
                            false,
                            hop,
                            turn_counters!(
                                markup_replies,
                                loops.spin(),
                                errors.streak(),
                                duplicate_inspection_results,
                                duplicate_inspection_bytes_saved,
                                action_capsule_metrics,
                            ),
                        );
                        observed_failure!(
                            TurnFailure {
                                message,
                                stop_reason: TurnStopReason::CheckpointFailure,
                                hops: hop,
                                interrupted: false,
                                deadline_reached: false,
                                max_hops_reached: false,
                                acceptance: None,
                                rollout_id: None,
                            },
                            "checkpoint_failure"
                        );
                    }
                }
                // Gates must speak: a configured lifecycle hook disables every
                // parallel/segmented dispatch path below. That is a deliberate
                // ordering guarantee, but left silent it reads as "the harness
                // got slow" (2026-08-15: a *-matcher PostToolUse hook serialized
                // every session for hours before anyone found it).
                if !hooks.is_empty() && calls.len() > 1 && !hooks_serial_notice_sent {
                    hooks_serial_notice_sent = true;
                    let _ = events.send(TurnEvent::Notice(
                        "lifecycle hooks configured (hooks.json) — tool batches run serial while hooks are active"
                            .to_string(),
                    ));
                }
                let parallel = hooks.is_empty()
                    && parallel_allowed(action_capsule_mode, registry.workspace_boundary(), &calls);
                let hooks = &hooks; // a Copy reference the move-closures can share
                let action_batch_ref = action_batch.as_ref();
                let sandbox_receipts = std::sync::Mutex::new(vec![None; calls.len()]);
                let mut dispatch_one = |index: usize, call: &ToolCall, gate: &AtomicBool| {
                    super::exec::set_sandbox_receipt(None);
                    let _ = events.send(TurnEvent::ToolCall {
                        id: ToolEventId(call.id.clone()),
                        name: call.name.clone(),
                        args_summary: summarize_args(&call.args),
                    });
                    crate::agent::tools::graph::emit_requested(
                        events,
                        &ToolEventId(call.id.clone()),
                        &call.name,
                        &call.args,
                    );
                    let preview = action_batch_ref.and_then(|batch| batch.contains(index));
                    let mut dispatch_elapsed = None;
                    let denied_this = deny_actions && preview.is_some();
                    let result = if denied_this {
                        format!(
                            "action capsule denied — {} not executed",
                            preview.expect("checked is_some").tool
                        )
                    } else {
                        let outer_id = ToolEventId(call.id.clone());
                        let call_started = Instant::now();
                        super::exec::take_tool_idle_escalation();
                        let mut result = dispatch_with_hooks_events_cancel(
                            registry,
                            hooks,
                            &call.name,
                            &call.args,
                            Some((&outer_id, events)),
                            Some(gate),
                        );
                        if super::exec::take_tool_idle_escalation() {
                            trajectory::note_escalation(hop, "tool_idle");
                            let _ = events.send(TurnEvent::Notice(
                                "tool_idle: silent tool failed; child tree termination requested; turn continues"
                                    .into(),
                            ));
                            result = format!(
                                "tool error: tool_idle: silence limit reached; child tree termination requested\n{result}\n{}",
                                super::book::p_processes::TOOL_IDLE.cells()
                            );
                        }
                        let elapsed = call_started.elapsed();
                        timing.note_tool_call(&call.name, elapsed);
                        dispatch_elapsed = Some(elapsed);
                        result
                    };
                    // Every dispatched call has timing, including headless and
                    // YOLO turns. Action previews only control the UI receipt.
                    if let Some((preview, elapsed)) = preview.zip(dispatch_elapsed) {
                        let _ = events.send(TurnEvent::Notice(
                            preview.receipt(&result, elapsed.as_millis()),
                        ));
                    }
                    let outcome = registry.executed_outcome(call, &result, denied_this);
                    crate::agent::tools::graph::emit_returned(
                        events,
                        &ToolEventId(call.id.clone()),
                        &call.name,
                        &result,
                        outcome,
                    );
                    let _ = events.send(TurnEvent::ToolResult {
                        id: ToolEventId(call.id.clone()),
                        name: call.name.clone(),
                        // Conversation projects the bounded capsule reason; trace
                        // keeps the dispatch diagnostic available in full.
                        summary: if result.starts_with("tool error:") {
                            result.clone()
                        } else {
                            summarize_result(&result)
                        },
                        outcome,
                    });
                    if outcome.execution == ExecutionOutcome::Succeeded
                        && call.name == "present"
                        && let Some((kind, label, url)) =
                            crate::ui::media::presentation_from_result(&result)
                    {
                        let _ = events.send(TurnEvent::Media { kind, label, url });
                    }
                    sandbox_receipts.lock().unwrap()[index] = super::exec::sandbox_receipt();
                    (result, dispatch_elapsed, outcome)
                };
                let segments =
                    if !hooks.is_empty() || parallel || action_capsule_mode.needs_approval() {
                        Vec::new()
                    } else {
                        batch_segments_in(registry.workspace_boundary(), &calls)
                    };
                let segmented = segments.iter().any(|segment| segment.len() > 1);
                crate::agent::turn::phase::mark("tool_dispatch");
                let tool_wait_started = Instant::now();
                let results: Vec<(String, Option<Duration>, ToolOutcome)> =
                    with_turn_deadline_cancel(cancel, turn_deadline, |dispatch_cancel| {
                        if parallel {
                            dispatch_parallel_segment(
                                registry,
                                hooks,
                                &calls,
                                0,
                                events,
                                dispatch_cancel,
                                action_batch_ref,
                                &sandbox_receipts,
                            )
                        } else if segmented {
                            let mut results = Vec::with_capacity(calls.len());
                            for segment in segments {
                                if segment.len() > 1 {
                                    results.extend(dispatch_parallel_segment(
                                        registry,
                                        hooks,
                                        &calls[segment.clone()],
                                        segment.start,
                                        events,
                                        dispatch_cancel,
                                        action_batch_ref,
                                        &sandbox_receipts,
                                    ));
                                } else {
                                    let index = segment.start;
                                    results.push(dispatch_one(
                                        index,
                                        &calls[index],
                                        dispatch_cancel,
                                    ));
                                }
                            }
                            results
                        } else {
                            calls
                                .iter()
                                .enumerate()
                                .map(|(index, call)| dispatch_one(index, call, dispatch_cancel))
                                .collect()
                        }
                    });
                // Batch wall time (serial, segmented, or parallel) so
                // concurrent members never double-count into `tool_ms`.
                let dispatch_wall = tool_wait_started.elapsed();
                let tool_shares = tool_wall_shares(
                    &results
                        .iter()
                        .map(|(_, elapsed, _)| elapsed.map_or(0, |d| d.as_millis()))
                        .collect::<Vec<_>>(),
                    dispatch_wall.as_millis(),
                );
                timing.last_tool_end = Some(turn_start.elapsed().as_millis());
                let cycle_observation = loops.cycle_observation(&loop_batch, &calls, &results);
                loops.observe_outcome(&loop_batch, &calls, &results);
                // Track the consecutive-error streak before the results are capped
                // into history: a hop where every call errored at dispatch. A red
                // test run is a verdict, not a dispatch error.
                let all_errored = !results.is_empty()
                    && results
                        .iter()
                        .zip(calls.iter())
                        .all(|((result, _, _), call)| is_dispatch_failure(call, result));
                let error_streak = errors.observe(all_errored);
                // Classify every dispatch-level failure into the bounded
                // schema/exec/timeout/other buckets for the experience ledger.
                for (result, _, outcome) in results.iter() {
                    timing.note_tool_result(
                        !matches!(
                            outcome.execution,
                            ExecutionOutcome::NotStarted | ExecutionOutcome::Denied
                        ),
                        is_error_result(result),
                    );
                    if is_error_result(result) {
                        let mut classes = tool_errors_by_class.get();
                        classes.record(crate::knowledge::experience::classify_tool_error(result));
                        tool_errors_by_class.set(classes);
                    }
                }
                // Ledger snapshot after this hop's results are counted, so a
                // record written at any later exit seam carries this hop.
                crate::agent::harness::trajectory::note_task_timing(
                    &timing.finish_with_history(turn_start.elapsed().as_millis(), history),
                );
                // Calls have now been fully dispatched, so move them into
                // history instead of cloning their potentially huge JSON args
                // before dispatch. Results still follow immediately in the
                // original order, preserving the provider tool-call protocol.
                let call_history_index = history.len();
                let mut assistant =
                    ChatMsg::assistant_calls_full(calls, private_reasoning, tool_content);
                assistant.responses_replay = responses_replay;
                history.push(assistant);
                let mut successful_mutation_this_hop = false;
                let mut cycle_state_changed_this_hop = false;
                let mut execution_blocked = None;
                post_write_verification.begin_batch();
                for (call_index, (result, dispatch_elapsed, tool_outcome)) in
                    results.into_iter().enumerate()
                {
                    if action_batch_ref.is_some_and(|batch| batch.contains(call_index).is_some())
                        && let Some(elapsed) = dispatch_elapsed
                    {
                        action_capsule_metrics.note_execution(elapsed);
                    }
                    // Post-write self-correct: after a successful file edit, surface
                    // what the machine already knows about it — the language server's
                    // errors for that file, and whether the project still builds —
                    // inline, so the model fixes it this turn instead of moving on.
                    // The same seam stamps the outcome onto The Cut's manifest.
                    let (call_id, result) = {
                        let call = &history[call_history_index].tool_calls[call_index];
                        let mut observers = PostWriteObservers {
                            diagnostic_counters: &post_edit_diagnostics,
                            verdicts: &mut verdicts,
                            rollout_recorder: &mut rollout_recorder,
                            verification: &mut post_write_verification,
                        };
                        let result = if tool_outcome.execution == ExecutionOutcome::Succeeded {
                            let (result, post_write) =
                                with_turn_deadline_cancel(cancel, turn_deadline, |verify_cancel| {
                                    post_write_verdict(
                                        registry,
                                        club,
                                        call,
                                        hop,
                                        result,
                                        &mut observers,
                                        verify_cancel,
                                    )
                                });
                            raised.extend(post_write);
                            result
                        } else {
                            result
                        };
                        (call.id.clone(), result)
                    };
                    super::exec::set_sandbox_receipt(
                        sandbox_receipts.lock().unwrap()[call_index].take(),
                    );
                    let call = &history[call_history_index].tool_calls[call_index];
                    if execution_blocked.is_none() {
                        execution_blocked = execution_blocker(&call.name, &result);
                    }
                    observe_tool_result_for_watch(
                        &mut slot_watcher,
                        &call.name,
                        &result,
                        competition,
                    );
                    publish_slot_telemetry(
                        events,
                        &slot_watcher,
                        competition,
                        &mut published_slot_telemetry,
                    );
                    if call.name == "code_mode" {
                        code_mode_calls.set(code_mode_calls.get().saturating_add(1));
                        if code_mode_receipt_is_recipe(&result, "repo_recon") {
                            code_mode_recipe_calls
                                .set(code_mode_recipe_calls.get().saturating_add(1));
                        }
                        if let Some((nested_calls, nested_bytes)) =
                            code_mode_receipt_metrics(&result)
                        {
                            code_mode_nested_calls
                                .set(code_mode_nested_calls.get().saturating_add(nested_calls));
                            code_mode_nested_output_bytes.set(
                                code_mode_nested_output_bytes
                                    .get()
                                    .saturating_add(nested_bytes),
                            );
                        }
                        if is_code_mode_policy_rejection(&result) {
                            code_mode_policy_rejections
                                .set(code_mode_policy_rejections.get().saturating_add(1));
                        }
                    }
                    let started = !matches!(
                        tool_outcome.execution,
                        ExecutionOutcome::NotStarted | ExecutionOutcome::Denied
                    );
                    let succeeded = tool_outcome.execution == ExecutionOutcome::Succeeded;
                    let mut call_fingerprint = CallFingerprint::new(registry.current_workspace());
                    if eval_owns_label() {
                        rollout_recorder.observe_task_verifier(
                            call,
                            tool_outcome,
                            &result,
                            started
                                && (is_mutation_call(call)
                                    || is_dependency_mutation_call(call)
                                    || matches!(
                                        call.name.as_str(),
                                        "shell" | "proc_run" | "spawn" | "delegate"
                                    )),
                        );
                    }

                    {
                        // Trajectory ledger entry for this call. Verifier verdicts
                        // are re-derived only for verifier calls (a cheap name
                        // match gates it), so non-verifier hops pay nothing.
                        let errored = is_error_result(&result);
                        let verify = (started
                            && tool_outcome.verification != VerificationOutcome::NotApplicable)
                            .then_some(tool_outcome.verification)
                            .map(|v| match v {
                                VerificationOutcome::Passed => "passed",
                                VerificationOutcome::Failed => "failed",
                                VerificationOutcome::Inconclusive => "inconclusive",
                                VerificationOutcome::NotApplicable => "n/a",
                            });
                        if verify == Some("passed") {
                            crate::agent::harness::trajectory::note_verified(
                                turn_start.elapsed().as_millis() as u64,
                            );
                        }
                        let class = errored.then(|| {
                            crate::knowledge::experience::classify_tool_error(&result).label()
                        });
                        crate::agent::harness::trajectory::note_tool_outcome(
                            hop,
                            &call.name,
                            &call.args,
                            &result,
                            match tool_outcome.execution {
                                ExecutionOutcome::Succeeded => "ok",
                                ExecutionOutcome::NotStarted => "not_started",
                                ExecutionOutcome::Failed => "failed",
                                ExecutionOutcome::Denied => "denied",
                                ExecutionOutcome::Cancelled => "cancelled",
                                ExecutionOutcome::Panicked => "panicked",
                            },
                            errored,
                            class,
                            verify,
                            dispatch_elapsed.map(|d| d.as_millis()),
                            result.len(),
                        );
                        crate::agent::harness::trajectory::note_last_tool_attribution(
                            tool_shares[call_index],
                        );
                        if let Some(entry) = registry.routed_execution(call, &result) {
                            crate::agent::harness::trajectory::note_tool_routing(&entry.receipt);
                        }
                    }
                    if started && is_mutation_call(call) {
                        timing
                            .first_action
                            .get_or_insert(turn_start.elapsed().as_millis());
                        for path in crate::knowledge::cut::mutation_targets(&call.name, &call.args)
                        {
                            if looks_like_test_source_path(&path)
                                && let Some(base) =
                                    Path::new(&path).file_name().and_then(|n| n.to_str())
                            {
                                self_authored_test_basenames.insert(base.to_string());
                            }
                        }
                    }
                    if succeeded && is_mutation_call(call) {
                        // Meta notes / living-handoff are bookkeeping: they do not
                        // arm mutation_seen or clear first-write (competition agents
                        // were "progressing" by rewriting LIVING_HANDOFF only).
                        if is_product_mutation_call(call) {
                            successful_mutation_this_hop = true;
                            cycle_state_changed_this_hop = true;
                            mutation_seen = true;
                            raised.extend(peripheral_fanout.observe(call));
                            if time_to_first_mutation_ms.get().is_none() {
                                time_to_first_mutation_ms
                                    .set(Some(turn_start.elapsed().as_millis() as u64));
                            }
                            crate::agent::harness::trajectory::note_first_action(
                                hop,
                                turn_start.elapsed().as_millis() as u64,
                            );
                            if mutation_requires_verification(call) {
                                verification_needed = true;
                                untested.note_edit();
                                cues.note_edit();
                                prose_only_workspace_fingerprint = None;
                            } else {
                                prose_only_workspace_fingerprint =
                                    workspace_fingerprint(registry.current_workspace());
                            }
                        }
                    } else if succeeded && is_dependency_mutation_call(call) {
                        // go get / npm install / cargo add change the tree even
                        // when no file tool was used; first-write + no-edit guards
                        // treat this as real progress.
                        mutation_seen = true;
                        cycle_state_changed_this_hop = true;
                        first_write_attempted = true;
                        untested.note_edit();
                        cues.note_edit();
                        crate::agent::harness::trajectory::note_first_action(
                            hop,
                            turn_start.elapsed().as_millis() as u64,
                        );
                        if time_to_first_mutation_ms.get().is_none() {
                            time_to_first_mutation_ms
                                .set(Some(turn_start.elapsed().as_millis() as u64));
                        }
                    }
                    // A real verifier attempt, including a red test or missing
                    // local dependency, is enough to let the model disclose the
                    // blocker. Record where the attempt occurred so a later
                    // opaque mutation makes it stale. Outcome quality is tracked
                    // separately; this fingerprint never claims the tree is green.
                    // Self-authored-only green checks do not clear the gate —
                    // they are recorded for a post-hop nudge instead.
                    if registry.mutation_targets.is_opaque() {
                        green_verify_achieved = false;
                    }
                    if started
                        && let Some(outcome) = registry
                            .routed_execution(call, &result)
                            .map(|entry| entry.outcome.verification)
                            .or_else(|| verification_outcome(call, &result))
                    {
                        // A real negative verifier invalidates the completion claim,
                        // even if an earlier compile passed on these bytes. Keep
                        // repair calls available; failure is not new green evidence.
                        if outcome == VerificationOutcome::Failed {
                            green_verify_achieved = false;
                            // A red on the code un-stales the advisory for the
                            // next green.
                            green_verify_nudge_emitted = false;
                        }
                        if let Some(raise) = red_streak.observe(outcome) {
                            raised.push(raise);
                        }
                        // A shell test that exits zero is not typed green, but
                        // it is what the model saw: the finish cue's fact.
                        if outcome == VerificationOutcome::Passed
                            || (call.name == "shell"
                                && outcome == VerificationOutcome::Inconclusive)
                        {
                            let command = call
                                .args
                                .get("command")
                                .and_then(|command| command.as_str())
                                .unwrap_or(call.name.as_str());
                            cues.observe_clean_run(command);
                        }
                        // Remember a red verdict and the code it ran on. A later
                        // test run that did not come back red replaces it, so
                        // `⠧⠁` only ever carries the model's latest run. A
                        // shell run that exits 0 is Inconclusive, not Passed;
                        // keeping the red record through it would have denied
                        // seven GLM answers on polyglot-v1 whose last run was
                        // green. A call that is not a test run (NotApplicable)
                        // and a runner that never started (exit 127, a timeout)
                        // say nothing about the code.
                        if track_task_facts {
                            match outcome {
                                VerificationOutcome::Passed | VerificationOutcome::Inconclusive => {
                                    last_red_run = None;
                                }
                                VerificationOutcome::Failed
                                    if is_red_verifier_run(call, &result)
                                        || !is_error_result(&result) =>
                                {
                                    last_red_run = Some(book::v_verification::RedRun {
                                        workspace: call_fingerprint.get(),
                                        label: book::v_verification::run_label(call),
                                        tail: book::v_verification::tail_chars(
                                            &result,
                                            book::v_verification::RUN_TAIL_CHARS,
                                        ),
                                    });
                                }
                                _ => {}
                            }
                        }
                        let weak_self_authored = self_authored_verify_guard
                            && outcome == VerificationOutcome::Passed
                            && verification_targets_self_authored(
                                call,
                                &self_authored_test_basenames,
                            );
                        if weak_self_authored {
                            // Keep verification_needed: the edits stay `⠧⠋`.
                            weak_verification_pending = true;
                        } else if verification_attempt_releases_gate(call, outcome)
                            && (outcome == VerificationOutcome::Failed
                                || !verification_result_has_known_gap(&result))
                        {
                            // A real attempt releases the gate unless the receipt
                            // names a known gap (a selected target that skipped a
                            // changed file). Unknown coverage — no git, an earlier
                            // opaque shell — is not a gap; `attempted_opaque_generation`
                            // below is what re-arms the gate on a *later* opaque edit.
                            verification_needed = false;
                            attempted_opaque_generation =
                                registry.mutation_targets.opaque_generation();
                            verification_attempt_workspace_fingerprint = call_fingerprint.get();
                            // Only a full-strength verifier counts as a completion
                            // green: a filtered slice going green is progress,
                            // not proof.
                            if is_completion_green(outcome, mutation_seen)
                                && verification_is_completion_sufficient(call)
                            {
                                green_verify_achieved = true;
                            }
                        } else {
                            // Keep verification_needed armed. Raw shell
                            // success is supplemental evidence until the
                            // execution boundary can attest a pinned argv.
                        }
                    }
                    // Opt-in: a green that only ran tests this turn wrote is not
                    // a test of the code, so it leaves the edits `⠧⠋`.
                    let self_authored_only = self_authored_verify_guard
                        && verification_outcome(call, &result) == Some(VerificationOutcome::Passed)
                        && verification_targets_self_authored(call, &self_authored_test_basenames);
                    if started
                        && book::v_verification::is_test_attempt(
                            registry,
                            call,
                            &result,
                            self_authored_only,
                        )
                    {
                        untested.note_attempt(call_fingerprint.get());
                    }
                    // `tests | tail; ls` or `tests || fallback` runs the tests
                    // too, but angelX cannot read a verdict from it. Once it
                    // exits 0 the earlier red run is no longer the model's
                    // latest word on the code (GLM py-book-store, polyglot-v1).
                    if track_task_facts && succeeded && shell_runs_tests_anywhere(call) {
                        last_red_run = None;
                    }
                    if succeeded
                        && (confirm_green_runs > 0 || confirm_green_on_chance)
                        && verification_outcome(call, &result) != Some(VerificationOutcome::Failed)
                    {
                        if is_verification_call(call)
                            || is_progress_verifier_call(&call.name, &call.args)
                        {
                            last_green_run = Some(book::v_verification::GreenRun {
                                call: call.clone(),
                                workspace: call_fingerprint.get(),
                                substitute: false,
                            });
                        } else if book::v_verification::test_run_behind_fallback(call)
                            && registry.has_tool("run_tests")
                        {
                            // `./test || ctest` can read green while the test
                            // failed; confirm with angelX's own runner instead.
                            last_green_run = Some(book::v_verification::GreenRun {
                                call: ToolCall {
                                    id: "confirm_green".into(),
                                    name: "run_tests".into(),
                                    args: serde_json::json!({}),
                                },
                                workspace: call_fingerprint.get(),
                                substitute: true,
                            });
                        }
                    }
                    // Universal ceiling: cap every result before it enters the
                    // conversation, so an uncapped tool (git_diff, find_files,
                    // delegate/integrate, MCP/peer…) can't dump unbounded text
                    // into the context window. The live UI event above already
                    // showed the full (summarized) result; only the model's copy
                    // is capped. Treebeard/HiQ then eagerly parks large eligible
                    // inspection bulk under a handle so root history stays LID.
                    let routing = registry
                        .routed_execution(call, &result)
                        .map(|entry| entry.receipt);
                    if tool_outcome.verification != VerificationOutcome::NotApplicable {
                        live_research.observe(
                            call,
                            tool_outcome,
                            &result,
                            &history[call_history_index].content,
                            &club.resolved_route_identity(),
                        );
                    }
                    let produced_bytes = result.len() as u64;
                    if succeeded && is_mutation_call(call) && confirm_green_on_chance {
                        crate::knowledge::cut::for_each_mutation_target_path(
                            &call.name,
                            &call.args,
                            |path| {
                                edited_paths.insert(path.to_owned());
                                false
                            },
                        );
                    }
                    if succeeded
                        && is_mutation_call(call)
                        && let Some(raise) = book::v_verification::out_of_scope_fact(
                            edit_scope.as_deref(),
                            registry.current_workspace(),
                            call,
                            &mut edit_scope_noted,
                        )
                    {
                        raised.push(raise);
                    }
                    let capped = cap_tool_output_owned(result, ctx_window);
                    let identity = inspection_identity_for_offload(call, &capped);
                    let off = eager_offload_tool_result(&call.name, capped, identity.as_deref());
                    if off.offloaded {
                        eager_offload_results.set(eager_offload_results.get().saturating_add(1));
                        eager_offload_bytes_saved.set(
                            eager_offload_bytes_saved
                                .get()
                                .saturating_add(off.bytes_saved),
                        );
                    }
                    background::produced(&call_id, produced_bytes, off.content.len() as u64);
                    history.push(
                        ChatMsg::tool(call_id, off.content)
                            .with_tool_receipt(call, tool_outcome)
                            .with_routing_receipt(routing)
                            .with_verified_workspace(
                                registry.current_workspace(),
                                history[call_history_index].tool_calls.len() == 1,
                            ),
                    );
                }
                // The hop's stamps ride the tail of its last tool result.
                let last_tool_index = history.len() - 1;
                timing.note_tool_batch(tool_wait_started.elapsed());
                timing.tool_member_ms += tool_shares.iter().sum::<u128>();
                debug_assert_eq!(
                    timing.tool_ms,
                    timing.tool_member_ms
                        + timing
                            .finish(turn_start.elapsed().as_millis())
                            .tool_overhead_ms
                );
                let progress_mutated = progress_bytes_before
                    != trajectory::mutation_byte_snapshot(
                        registry.current_workspace(),
                        &history[call_history_index].tool_calls,
                    )
                    || progress_workspace_before.is_some_and(|before| {
                        workspace_fingerprint(registry.current_workspace())
                            .is_some_and(|after| before != after)
                    });
                crate::agent::harness::trajectory::note_progress_hop(progress_mutated);
                live_research.flush();
                loops.after_hop(
                    &loop_batch,
                    cycle_observation,
                    progress_mutated,
                    cycle_state_changed_this_hop,
                    &mut raised,
                );
                handle_unproductive_streak!();
                // Stop before another paid model hop. Finish pairing the entire
                // dispatched batch first; unrelated successful calls cannot
                // erase a failed execution prerequisite.
                if let Some(note) = execution_blocked {
                    crate::agent::harness::trajectory::note_task_timing(
                        &timing.finish_with_history(turn_start.elapsed().as_millis(), history),
                    );
                    crate::agent::harness::trajectory::note_stop_reason(
                        TurnStopReason::ExecutionBlocked.as_str(),
                    );
                    log_trajectory(club, history, &note, hop, true, verdicts.reward());
                    write_exp(
                        "execution_blocked",
                        false,
                        hop,
                        turn_counters!(
                            markup_replies,
                            loops.spin(),
                            errors.streak(),
                            duplicate_inspection_results,
                            duplicate_inspection_bytes_saved,
                            action_capsule_metrics,
                        ),
                    );
                    observed_failure!(
                        TurnFailure {
                            message: note,
                            stop_reason: TurnStopReason::ExecutionBlocked,
                            hops: hop,
                            interrupted: false,
                            deadline_reached: false,
                            max_hops_reached: false,
                            acceptance: None,
                            rollout_id: None,
                        },
                        "execution_blocked"
                    );
                }
                if let Some(notify) = slot_watcher.pending_notify()
                    && watcher_announced.insert(notify.id.clone())
                {
                    let _ = events.send(TurnEvent::Notice(notify.injection_text()));
                    history.push(ChatMsg::harness(book::k_competition::watcher_turn(
                        registry.current_workspace(),
                        &notify,
                    )));
                }
                // Close the hop's manifest rows. Writes the postcheck above never
                // claimed (a `code_mode` script's inner edits, which dispatch
                // through the same registry but aren't tool calls of this loop)
                // land here, unstamped — a row is never held across a hop.
                crate::knowledge::cut::sweep();
                let acceptance_workspace_after = task_accept_cmd
                    .as_ref()
                    .and_then(|_| workspace_evidence_sha256(registry.current_workspace()));
                let verify_after_hop = task_accept_cmd.is_some()
                    && match (
                        acceptance_workspace_before,
                        acceptance_workspace_after.clone(),
                    ) {
                        (Some(before), Some(after)) => before != after,
                        _ => successful_mutation_this_hop,
                    };
                // The hop advisories (`⠼`, `⡅`), in the order 0.1.6 gave them.
                // A green that only ran the model's own tests is weak, once.
                if weak_verification_pending && !weak_verification_told {
                    weak_verification_told = true;
                    raised.push(book::Raise::new(book::d3456_advisories::WEAK, None));
                }
                weak_verification_pending = false;
                // A full verifier passed on changed code: use it as evidence and
                // finish what remains. In a competition, bank the candidate.
                if green_verify_achieved && !green_verify_nudge_emitted {
                    green_verify_nudge_emitted = true;
                    raised.push(book::Raise::new(
                        if competition {
                            book::k_competition::WINNER_BANK
                        } else {
                            book::d3456_advisories::GREEN
                        },
                        None,
                    ));
                }
                // The armed inspection budget is spent and nothing was edited.
                if first_write_limit > 0 && !first_write_attempted {
                    if successful_mutation_this_hop {
                        first_write_attempted = true;
                    } else {
                        let burned = history[call_history_index]
                            .tool_calls
                            .iter()
                            .filter(|call| is_free_form_recon(call))
                            .count();
                        prewrite_calls = prewrite_calls.saturating_add(burned);
                        if burned > 0
                            && prewrite_calls >= first_write_limit
                            && !first_write_nudge_emitted
                        {
                            // Once per turn: repeating the directive before and
                            // after every later call crowds out the evidence.
                            first_write_nudge_emitted = true;
                            raised.push(book::Raise::new(
                                if competition && task_pace == TaskPace::Rapid {
                                    book::k_competition::FIRST_WRITE_RAPID
                                } else {
                                    book::d3456_advisories::FIRST_WRITE
                                },
                                format!(
                                    "{prewrite_calls} inspection call(s) before the first edit"
                                ),
                            ));
                        }
                    }
                }
                // A mutation after the green stales it: the verifier proved a
                // tree that no longer exists.
                if green_verify_achieved && successful_mutation_this_hop {
                    if competition {
                        raised.push(book::Raise::new(
                            book::k_competition::CANDIDATE_CHANGED,
                            None,
                        ));
                    }
                    green_verify_achieved = false;
                    green_verify_nudge_emitted = false;
                }
                // The first successful edit of the turn: review it before
                // testing, once.
                if post_edit_review && successful_mutation_this_hop && !post_edit_logic_reviewed {
                    post_edit_logic_reviewed = true;
                    raised.push(book::Raise::new(book::d3456_advisories::POST_EDIT, None));
                }
                // The final mile. Inspection alone while a verifier is owed hears
                // the last sentence again, as its own page; the calls still run.
                if book::d3456_advisories::rejects_inspection(
                    final_mile_active,
                    verification_needed,
                    &history[call_history_index].tool_calls,
                ) {
                    raised.push(book::Raise::page(
                        book::d3456_advisories::FINAL_MILE,
                        book::d3456_advisories::FINAL_MILE_AGAIN,
                        None,
                    ));
                }
                if book::d3456_advisories::should_activate_final_mile(
                    max_hops,
                    hop,
                    final_mile_hops,
                    mutation_seen,
                    final_mile_active,
                ) {
                    final_mile_active = true;
                    let remaining = max_hops.unwrap_or(hop).saturating_sub(hop);
                    raised.push(book::Raise::new(
                        book::d3456_advisories::FINAL_MILE,
                        format!("{remaining} bounded hop(s) remaining"),
                    ));
                }
                if verify_after_hop && let Some(command) = task_accept_cmd.as_deref() {
                    let proof = book::v_verification::run_task_accept(
                        command,
                        registry.current_workspace(),
                    );
                    accept_post_checks += 1;
                    accept_post_ms += proof.elapsed_ms;
                    accept_last_post_result = Some(proof.result_class);
                    accept_last_summary = Some(proof.summary.clone());
                    let workspace_after_gate =
                        workspace_evidence_sha256(registry.current_workspace());
                    accept_last_checked_workspace =
                        match (acceptance_workspace_after.clone(), workspace_after_gate) {
                            (Some(before), Some(after)) if before == after => Some(after),
                            _ => None,
                        };
                    if proof.passed {
                        crate::agent::harness::trajectory::note_verified(
                            turn_start.elapsed().as_millis() as u64,
                        );
                        time_to_green_ms.set(Some(turn_start.elapsed().as_millis() as u64));
                        // The answer's reader cannot read the ledger:
                        // `⠟⠛⠁`'s page, the hop count in place.
                        let mut answer = book::q_stop::ACCEPTED.replace("{hop}", &hop.to_string());
                        let notes =
                            with_turn_deadline_cancel(cancel, turn_deadline, |verify_cancel| {
                                finish_post_write_verification(
                                    registry,
                                    club,
                                    hop,
                                    &mut post_write_verification,
                                    &mut verdicts,
                                    &mut rollout_recorder,
                                    verify_cancel,
                                )
                            });
                        for note in notes {
                            answer.push_str(&format!("\n\n{note}"));
                        }
                        let _ = events.send(TurnEvent::Notice(proof.summary));
                        history.push(ChatMsg::assistant(answer.clone()));
                        crate::agent::harness::trajectory::note_task_timing(
                            &timing.finish_with_history(turn_start.elapsed().as_millis(), history),
                        );
                        crate::agent::harness::trajectory::note_stop_reason(
                            TurnStopReason::Answer.as_str(),
                        );
                        log_trajectory(club, history, &answer, hop, false, verdicts.reward());
                        file_report(
                            club,
                            registry.current_workspace(),
                            history,
                            &answer,
                            &registry.store,
                            &registry.session_id,
                        );
                        write_exp(
                            "accept_cmd",
                            true,
                            hop,
                            turn_counters!(
                                markup_replies,
                                loops.spin(),
                                errors.streak(),
                                duplicate_inspection_results,
                                duplicate_inspection_bytes_saved,
                                action_capsule_metrics,
                            ),
                        );
                        observed_outcome!(TurnOutcome::answer(answer, hop), "accept_cmd");
                    } else if let Some(fact) = proof.fact() {
                        // The model just saw its own run go green; the pinned
                        // acceptance says the green does not hold (`⠧⠑`), or is
                        // red (`⠧⠃`).
                        let _ = events.send(TurnEvent::Notice(proof.summary.clone()));
                        if fact.route == book::v_verification::FLAKY {
                            raised.push(fact);
                        }
                    }
                }
                raised.extend(error_streak);
                raised.extend(cues.take());
                loops.latch(&mut raised, progress_mutated);
                for raise in &raised {
                    if raise.route.primary == book::l_loops::CELL
                        || raise.route == book::x_execution::ERRORS
                    {
                        loop_facts.push(raise.clone());
                    }
                }
                // The hop's routes: an advisory (`book::VOICED`) and a loop route
                // (`⠇`) each ride a turn of their own; the rest ride the tail of
                // the last tool result as one warpath. Inside the result a model
                // reads a cue as more output: replayed at DeepSeek loop points, no
                // cue there broke a loop (0/26, stamp through full English), while
                // `⛔⠇⠁` as its own turn broke 11/26 for 7 tokens. The advice of
                // 0.1.6 was always its own turn, and it said what to do.
                if !raised.is_empty() && history[last_tool_index].role == ChatRole::Tool {
                    let (looped, rest): (Vec<_>, Vec<_>) = raised
                        .iter()
                        .cloned()
                        .partition(|raise| raise.route.primary == book::l_loops::CELL);
                    let (voiced, riding): (Vec<_>, Vec<_>) = rest
                        .into_iter()
                        .partition(|raise| book::is_voiced(raise.route));
                    let mut shown = String::new();
                    if !riding.is_empty() {
                        let cells = book::warpath(registry.current_workspace(), &riding);
                        let message = &mut history[last_tool_index];
                        message.content = format!("{}\n{cells}", message.content).into();
                        shown.push_str(&cells);
                    }
                    // A warning leads with the sign; guidance is bare cells.
                    let (warnings, guidance): (Vec<_>, Vec<_>) = voiced
                        .into_iter()
                        .partition(|raise| book::is_warning(raise.route));
                    for (group, warning) in [(&guidance, false), (&warnings, true)] {
                        if !group.is_empty() {
                            let turn =
                                book::advice_turn(registry.current_workspace(), group, warning);
                            history.push(ChatMsg::harness(turn.clone()));
                            shown.push_str(turn.lines().next().unwrap_or_default());
                        }
                    }
                    if !looped.is_empty() {
                        let cells = book::warpath(registry.current_workspace(), &looped);
                        let turn = format!("{}{}", book::l_loops::WARNING, loops.turn_cells(cells));
                        history.push(ChatMsg::harness(turn.clone()));
                        shown.push_str(&turn);
                    }
                    for raise in &raised {
                        crate::agent::harness::trajectory::note_escalation(
                            hop,
                            &raise.route.name(),
                        );
                        if let Some(kind) = book::legacy_kind(raise.route) {
                            crate::agent::harness::trajectory::note_escalation(hop, kind);
                        }
                    }
                    let _ = events.send(TurnEvent::Notice(format!(
                        "warpath {shown} · {}",
                        book::names(&raised)
                    )));
                }
                crate::agent::harness::trajectory::note_hop_stamps(
                    &raised.iter().map(|raise| raise.route).collect::<Vec<_>>(),
                );
            }
        }
    }
}

pub(crate) fn configured_turn_deadline_secs() -> usize {
    env_usize("ANGEL_TURN_DEADLINE_SECS", 0)
}

/// Wall-clock budget for one turn. Explicit `ANGEL_TURN_DEADLINE_SECS` always
/// wins. Competition turns do **not** invent a default kill clock — overnight
/// handoff/podrace loops must not die at an arbitrary 20-minute wall. Operators
/// who want a wall set `ANGEL_TURN_DEADLINE_SECS` or
/// `ANGEL_COMPETITION_TURN_DEADLINE_SECS` explicitly.
pub(crate) fn configured_turn_deadline_secs_for(competition: bool) -> usize {
    if std::env::var_os("ANGEL_TURN_DEADLINE_SECS").is_some() {
        return env_usize("ANGEL_TURN_DEADLINE_SECS", 0);
    }
    if competition && std::env::var_os("ANGEL_COMPETITION_TURN_DEADLINE_SECS").is_some() {
        return env_usize("ANGEL_COMPETITION_TURN_DEADLINE_SECS", 0);
    }
    0
}

/// Last operator/user task text for advisor prompts.
fn last_user_task(history: &[ChatMsg]) -> String {
    history
        .iter()
        .rev()
        .find(|m| m.role == ChatRole::User)
        .map(|m| m.content.to_string())
        .unwrap_or_default()
}

/// Final-answer advisor gate for ordinary (and swarm-pre-annotated) turns.
fn apply_final_advisor(
    club: &dyn Club,
    registry: &ToolRegistry,
    history: &[ChatMsg],
    answer: String,
) -> String {
    if !crate::agent::advisor::final_enabled()
        || answer.trim().is_empty()
        || crate::agent::advisor::already_annotated(&answer)
    {
        return answer;
    }
    let task = last_user_task(history);
    registry.auxiliary.utility_entered("advisor");
    // The reviewer is connected: its brief is `⠌⠊`, read through the ledger.
    match crate::agent::advisor::review(club, registry.current_workspace(), &task, &answer) {
        Ok(reply) => {
            match crate::agent::advisor::annotate(&crate::agent::advisor::parse_verdict(&reply)) {
                Some(a) => format!("{answer}{a}"),
                None => answer,
            }
        }
        Err(_) => answer,
    }
}

struct PostWriteObservers<'a> {
    diagnostic_counters: &'a PostEditDiagnosticCounters,
    verdicts: &'a mut crate::knowledge::cut::TurnVerdicts,
    rollout_recorder: &'a mut RolloutRecorder,
    verification: &'a mut crate::knowledge::cut::PostWriteVerification,
}

/// The post-write seam: everything the machine can say about a mutation the
/// instant it lands, folded back into the tool result the model is about to read
/// — and stamped onto The Cut's manifest (docs/plans/the-cut.md, T2).
///
/// Two checks, cheapest first:
/// 1. the language server's errors for the edited file ([`maybe_lsp_postcheck`],
///    unchanged — gated on `ANGEL_LSP`);
/// 2. the project's *verify* command — `cargo check`, `node --check`, `tsc
///    --noEmit`, never a test run — run under a hard timeout
///    ([`crate::knowledge::cut::PostWriteVerification`]). On by default; `ANGEL_CUT_VERIFY=0` opts out.
///
/// Only a *failure* reaches the model — its data on the result, `⠧⠉` on the
/// warpath: a passing verify would spend context to say nothing. The verdict is recorded either way, so the manifest
/// carries a machine label on every write — the label density the whole plan
/// turns on — while the turn only pays tokens when angel actually broke the
/// build. This is also where the manifest row gets the facts only the loop knows
/// (the hop, the resolved model).
///
/// The same verdict is folded into `verdicts`, which is what the turn's
/// trajectory row is finally *rewarded* on ([`crate::knowledge::cut::TurnVerdicts`]). One
/// verify, three consumers: the model reads it, the manifest records it, and the
/// forge trains on it.
fn post_write_verdict(
    registry: &ToolRegistry,
    club: &dyn Club,
    call: &ToolCall,
    hop: usize,
    result: String,
    observers: &mut PostWriteObservers<'_>,
    cancel: &AtomicBool,
) -> (String, Option<book::Raise>) {
    if registry.external_evaluator_only {
        return (result, None);
    }
    // A failed or denied call authored nothing — there is nothing to check and
    // nothing to record.
    if is_error_result(&result) || result.starts_with("action capsule denied") {
        return (result, None);
    }
    let targets = crate::knowledge::cut::mutation_targets(&call.name, &call.args);
    if targets.is_empty() {
        return (result, None); // not a mutation tool
    }
    let receipt_len = result.len();
    let result = maybe_lsp_postcheck(
        registry,
        observers.diagnostic_counters.enabled,
        &targets,
        result,
        observers.diagnostic_counters,
    );
    let mut notes = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for target in targets {
        if !seen.insert(target.clone()) {
            continue;
        }
        let receipt = observers.verification.check(
            registry.current_workspace(),
            &target,
            cancel,
            observers.verdicts,
        );
        let machine_json = receipt.as_ref().map(|receipt| receipt.to_json());
        crate::knowledge::cut::settle(
            &call.name,
            &[target],
            hop,
            &club.resolved_route_identity(),
            machine_json.as_ref(),
        );
        if let Some(receipt) = receipt {
            if !receipt.shared
                && let Some(evidence) = machine_json.as_ref()
            {
                observers.rollout_recorder.observe_cut_machine(evidence);
            }
            if let Some(note) = receipt.machine.inline_note()
                && !notes.contains(&note)
            {
                notes.push(note);
            }
        }
    }
    // The machine's own words ride the result as data; the route is `⠧⠉`.
    let lsp = result[receipt_len..].trim().to_string();
    let mut data = Vec::new();
    if !lsp.is_empty() {
        data.push(lsp);
    }
    data.extend(notes.iter().cloned());
    if data.is_empty() {
        return (result, None);
    }
    let result = if notes.is_empty() {
        result
    } else {
        format!("{result}\n\n{}", notes.join("\n\n"))
    };
    let raise = book::Raise::new(book::v_verification::POST_WRITE, data.join("\n\n"));
    (result, Some(raise))
}

fn finish_post_write_verification(
    registry: &ToolRegistry,
    club: &dyn Club,
    hop: usize,
    verification: &mut crate::knowledge::cut::PostWriteVerification,
    verdicts: &mut crate::knowledge::cut::TurnVerdicts,
    rollout_recorder: &mut RolloutRecorder,
    cancel: &AtomicBool,
) -> Vec<String> {
    if registry.external_evaluator_only {
        return Vec::new();
    }
    verification
        .finish(registry.current_workspace(), cancel, verdicts)
        .into_iter()
        .filter_map(|receipt| {
            let evidence = receipt.to_json();
            rollout_recorder.observe_cut_machine(&evidence);
            crate::knowledge::cut::record_final_verification(
                registry.current_workspace(),
                hop,
                &club.resolved_route_identity(),
                &receipt,
            );
            receipt.machine.inline_note().or_else(|| {
                (!receipt.machine.passed()).then(|| {
                    format!(
                        "[final post-write verification unverified: {}]",
                        evidence.get("skipped").unwrap_or(&serde_json::Value::Null)
                    )
                })
            })
        })
        .collect()
}

#[derive(Default)]
pub(crate) struct PostEditDiagnosticCounters {
    pub(crate) enabled: bool,
    pub(crate) attempts: std::cell::Cell<usize>,
    pub(crate) findings: std::cell::Cell<usize>,
    pub(crate) failures: std::cell::Cell<usize>,
    pub(crate) paths_skipped: std::cell::Cell<usize>,
    pub(crate) output_bytes: std::cell::Cell<u64>,
    pub(crate) elapsed_ms: std::cell::Cell<u64>,
}

fn diagnostic_error_lines(diagnostics: &str) -> Vec<&str> {
    diagnostics
        .lines()
        .filter(|line| line.trim_start().starts_with("error "))
        .collect()
}

/// Exact UTF-8-safe prefix cap for diagnostic feedback. Unlike the universal
/// tool cap this must be a hard upper bound because it is injected *in addition*
/// to the edit receipt and can cover several files in one patch.
fn cap_post_edit_diagnostics(text: String, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text;
    }
    const MARKER: &str = "\n…[post-edit diagnostics truncated]";
    let keep = max_bytes.saturating_sub(MARKER.len());
    let mut end = keep.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    let marker = if MARKER.len() <= max_bytes {
        MARKER
    } else {
        ""
    };
    format!("{}{marker}", &text[..end])
}

/// After a successful mutation, append the language server's *errors* for the
/// changed files to the same tool result so the model can repair them on its
/// next inference. It reuses `lsp_diagnostics`, but hot-path calls never spawn a
/// cold server and have a short per-file deadline. LSP unavailability is counted
/// and remains non-destructive: the successful mutation receipt stays exact.
pub(crate) fn maybe_lsp_postcheck(
    registry: &ToolRegistry,
    enabled: bool,
    targets: &[String],
    result: String,
    counters: &PostEditDiagnosticCounters,
) -> String {
    if !enabled
        || is_error_result(&result)
        || result.starts_with("action capsule denied")
        || targets.is_empty()
    {
        return result;
    }

    let max_paths = env_usize("ANGEL_POST_EDIT_DIAGNOSTIC_MAX_PATHS", 4).min(8);
    let timeout_ms = env_usize("ANGEL_POST_EDIT_DIAGNOSTIC_TIMEOUT_MS", 1_500).clamp(50, 5_000);
    let max_bytes =
        env_usize("ANGEL_POST_EDIT_DIAGNOSTIC_MAX_BYTES", 8 * 1024).clamp(512, 32 * 1024);
    let mut paths: Vec<&str> = targets.iter().map(String::as_str).collect();
    paths.sort_unstable();
    paths.dedup();
    if paths.len() > max_paths {
        counters.paths_skipped.set(
            counters
                .paths_skipped
                .get()
                .saturating_add(paths.len() - max_paths),
        );
        paths.truncate(max_paths);
    }

    let mut blocks = Vec::new();
    for path in paths {
        counters
            .attempts
            .set(counters.attempts.get().saturating_add(1));
        let started = Instant::now();
        let diagnostic = registry.dispatch(
            "lsp_diagnostics",
            &serde_json::json!({
                "path": path,
                "_warm_only": true,
                "_deadline_ms": timeout_ms,
            }),
        );
        counters.elapsed_ms.set(
            counters
                .elapsed_ms
                .get()
                .saturating_add(started.elapsed().as_millis() as u64),
        );
        match diagnostic {
            Ok(diagnostics) => {
                let errors = diagnostic_error_lines(&diagnostics);
                if errors.is_empty() {
                    if diagnostics.contains("no diagnostics within") {
                        counters
                            .failures
                            .set(counters.failures.get().saturating_add(1));
                    }
                    continue;
                }
                counters
                    .findings
                    .set(counters.findings.get().saturating_add(errors.len()));
                blocks.push(format!(
                    "{path}: {} error(s)\n{}",
                    errors.len(),
                    errors.join("\n")
                ));
            }
            Err(_) => counters
                .failures
                .set(counters.failures.get().saturating_add(1)),
        }
    }
    if blocks.is_empty() {
        return result;
    }
    let diagnostic = cap_post_edit_diagnostics(blocks.join("\n\n"), max_bytes);
    counters.output_bytes.set(
        counters
            .output_bytes
            .get()
            .saturating_add(diagnostic.len() as u64),
    );
    format!("{result}\n\n{diagnostic}")
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/turn__mod__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/turn__mod__mutation_tests.rs"]
mod mutation_tests;
