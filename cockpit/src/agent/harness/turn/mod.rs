//! Single-agent turn execution: `run_turn` and its anti-spin / nudge machinery.
pub(crate) mod background;

mod classify;
mod competition;
mod governors;
mod nudges;
mod reasoning;
pub(crate) mod research;
mod verify;

use super::*;

pub(crate) use classify::*;
pub(crate) use competition::*;
pub(crate) use governors::*;
pub(crate) use nudges::*;
pub(crate) use reasoning::*;
pub(crate) use verify::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct UnproductivePolicy {
    escalate: usize,
    stop: usize,
}

fn configured_unproductive_policy(metered_sota: bool, competition: bool) -> UnproductivePolicy {
    let metered_interactive = metered_sota && !competition;
    let task_active = std::env::var("ANGEL_TASK_ACTIVE").is_ok_and(|value| value == "1");
    let escalate = env_usize(
        "ANGEL_UNPRODUCTIVE_STREAK_ESCALATE",
        if metered_interactive || task_active {
            8
        } else {
            0
        },
    );
    if competition {
        return UnproductivePolicy { escalate, stop: 0 };
    }

    let default_stop = if task_active { 16 } else { 0 };
    let stop = env_usize("ANGEL_UNPRODUCTIVE_STREAK_STOP", default_stop);

    UnproductivePolicy { escalate, stop }
}

/// Build the one compact telemetry snapshot written at each turn exit. A
/// constructor macro keeps the field mapping centralized without turning an
/// expanding metrics record into a long positional function interface.
macro_rules! turn_counters {
    (
        $deferred_nudges:expr_2021,
        $spin:expr_2021,
        $err_streak:expr_2021,
        $churn:expr_2021,
        $first_write_rejections:expr_2021,
        $duplicate_inspection_results:expr_2021,
        $duplicate_inspection_bytes_saved:expr_2021,
        $verification_denials:expr_2021,
        $actions:expr_2021 $(,)?
    ) => {{
        let actions: ActionCapsuleMetrics = $actions;
        crate::knowledge::experience::TurnCounters {
            deferred_nudges: $deferred_nudges,
            spin: $spin,
            err_streak: $err_streak,
            churn: $churn,
            first_write_rejections: $first_write_rejections,
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
            redundant_verifier_skips: 0,
            discovered_tool_schema_failures: 0,
            verification_denials: $verification_denials,
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
    DeferredStop,
    Spin,
    ErrorStop,
    ExecutionBlocked,
    CaptureFailure,
    CheckpointFailure,
    ProviderError,
    NeedsPro,
    AcceptanceStop,
    EscalatedUnproductive,
}

impl TurnStopReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Answer => "answer",
            Self::Interrupt => "interrupt",
            Self::IdleTimeout => "idle_timeout",
            Self::Deadline => "deadline",
            Self::MaxHops => "max_hops",
            Self::DeferredStop => "deferred_stop",
            Self::Spin => "spin",
            Self::ErrorStop => "error_stop",
            Self::ExecutionBlocked => "execution_blocked",
            Self::CaptureFailure => "capture_failure",
            Self::CheckpointFailure => "checkpoint_failure",
            Self::ProviderError => "provider_error",
            Self::NeedsPro => "needs_pro",
            Self::AcceptanceStop => "acceptance_stop",
            Self::EscalatedUnproductive => "escalated_unproductive",
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
    pub(crate) calls: serde_json::Value,
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
            calls: serde_json::json!({"model_calls":self.model_samples,
                "retry_backoff_ms":self.model_retry_ms,
                "provider_calls":crate::agent::harness::trajectory::provider_call_samples()}),
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

/// Headless tasks have no App::advance consumer for proc_run completions.
/// Deliver only bounded, workspace-bound outcome notices, never log contents
/// or a fabricated verification result. Interactive turns retain their UI path.
fn inject_task_proc_completions(
    registry: &ToolRegistry,
    history: &mut Vec<ChatMsg>,
    events: &mpsc::Sender<TurnEvent>,
) -> usize {
    let notices = crate::agent::tools::proc::take_completions(
        &registry.workspace_boundary().canonical_root,
        8,
    );
    let count = notices.len();
    for completion in notices {
        let message = completion.task_message();
        history.push(ChatMsg::harness(message.clone()));
        let _ = events.send(TurnEvent::Notice(message));
    }
    count
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
    let unproductive_policy = configured_unproductive_policy(metered_sota, competition);
    let streak_escalate = unproductive_policy.escalate;
    let research_turn = !competition && research::selected(history);
    let research_origin = research_turn.then(|| research::origin(history)).flatten();
    if research_turn {
        history.push(ChatMsg::harness(research::preamble(
            research_origin.as_deref(),
        )));
    }
    let mut research_compose_sent = false;
    let turn_budget = configured_turn_deadline_secs_for(competition);
    let task_pace = configured_task_pace(history);
    registry.reset_tool_activations();
    let bubble = if metered_sota {
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
    // Anti-spin guardrail: if the model emits the *same* tool-call batch over and
    // over with no new outcome, it's stuck — not reasoning. Nudge once, then stop.
    // Distinct from `max_hops` (which counts every hop, productive ones included);
    // `ANGEL_SPIN_LIMIT=0` disables it entirely for pure unbounded loops.
    let task_active = std::env::var("ANGEL_TASK_ACTIVE").is_ok_and(|value| value == "1");
    let spin_stop = std::env::var("ANGEL_SPIN_LIMIT")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .unwrap_or(if task_active { 4 } else { 0 });
    let spin_nudge = (spin_stop / 2).max(2);
    // Perturbation injection: at the nudge point, jolt the model out of the loop
    // with a concrete reframe (opposite hypothesis / cross-domain analogy) rather
    // than a generic "change approach". On by default; `ANGEL_SPIN_PERTURB=0`
    // restores the plain nudge.
    let spin_perturb = env_flag("ANGEL_SPIN_PERTURB", true);
    let mut last_sig: Option<u64> = None;
    let mut spin = 0usize;
    // Gemini CLI-style bounded k-cycle detection, adapted to use angelX's
    // canonical call identity plus actual result/outcome evidence. Successful
    // workspace mutations clear the window. This catches alternating read/tool
    // loops that identical-batch anti-spin cannot see without penalizing a
    // changing poll result or productive edit sequence.
    let tool_cycle_max_period = env_usize("ANGEL_TOOL_CYCLE_MAX_PERIOD", 5).min(8);
    let tool_cycle_repeats = env_usize("ANGEL_TOOL_CYCLE_REPEATS", 5).min(10);
    let mut tool_cycle = (spin_stop > 0
        && std::env::var_os("ANGEL_TOOL_CYCLE_REPEATS").is_some()
        && tool_cycle_max_period >= 2
        && tool_cycle_repeats >= 2)
        .then(|| ToolBatchCycle::new(tool_cycle_max_period, tool_cycle_repeats));
    // Duplicate-call storm guard (opt-in). Reasoning models re-issue a
    // byte-identical call for hop after hop; anti-spin only ends the turn once
    // the whole batch repeats, so a storm interleaved with other work burns the
    // horizon undetected. Off unless armed; `ANGEL_TOOLCALL_STORM_WINDOW=0` also
    // disarms it.
    let mut toolcall_storm = env_flag("ANGEL_TOOLCALL_STORM", false)
        .then(|| env_usize("ANGEL_TOOLCALL_STORM_WINDOW", 6))
        .filter(|window| *window > 0)
        .map(ToolCallStorm::new);
    // The watcher supports waiting without requiring repeated status calls.
    // Suppression is operator opt-in: the model may still need to monitor a
    // real experiment, and a heuristic must not silently block that choice.
    let poll_guard_enabled = env_flag("ANGEL_POLL_GUARD", false);
    let poll_only_limit = env_usize("ANGEL_POLL_ONLY_LIMIT", 1).min(8);
    let passive_sleep_max_secs = env_usize("ANGEL_PASSIVE_SLEEP_MAX_SECS", 2) as u64;
    let mut passive_poll_guard = PassivePollGuard::default();
    let poll_repeat_limit = env_usize("ANGEL_POLL_REPEAT_LIMIT", 8).min(32);
    let mut repeated_poll_guard = RepeatedPollGuard::new(poll_repeat_limit);
    let mut passive_poll_nudge_sent = false;
    // One operator storm notice per tool name per turn; the model still gets
    // a per-call not-started result.
    let mut last_storm_notice: Option<String> = None;
    // Tool-call scavenging (opt-in): recover a call the model stranded in its
    // answer text with an empty structured `tool_calls` array.
    let toolcall_scavenge = env_flag("ANGEL_TOOLCALL_SCAVENGE", false);
    // History hygiene: opt-in hard cap on conversation length so an unbounded
    // multi-day loop can't grow the request body without bound. 0 = unbounded.
    let history_cap = env_usize("ANGEL_HISTORY_MAX_MSGS", 0);
    // Rolling tool-aging and deduplication cadence under cache-stable mode.
    // Flushes held inspection rewrites periodically so long turns never balloon
    // into 100k+ token request bodies.
    let rolling_rewrite_hops = env_usize("ANGEL_ROLLING_REWRITE_HOPS", 12);
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
        // Once per competition turn: stamp the resolved challenge pace into
        // model history. Competition awareness and rapid submission cadence are
        // separate contracts; deep work must never inherit the latter merely
        // because the standing goal mentions a leaderboard.
        let (posture, world_card) = competition_posture(task_pace);
        let pace_marker = format!(
            "COMPETITION CHALLENGE PACE — {}",
            task_pace.as_str().to_ascii_uppercase()
        );
        let already = history
            .iter()
            .any(|m| m.role == ChatRole::Harness && m.content.contains(&pace_marker));
        if !already {
            history.push(ChatMsg::harness(posture.to_string()));
            // Standing board digest so cold seats don't burn the inspection
            // budget re-deriving tip/slot/score every hop.
            let has_card = history.iter().any(|m| {
                m.role == ChatRole::Harness
                    && m.content.contains("COMPETITION WORLD CARD")
                    && m.content.contains(&task_pace.as_str().to_ascii_uppercase())
            });
            if !has_card {
                history.push(ChatMsg::harness(world_card.to_string()));
            }
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
    // Consecutive-error circuit breaker: a hop where *every* tool call errored is
    // a failed hop; several in a row is thrashing, not progress. Nudge at half,
    // stop at the limit. `ANGEL_ERROR_LIMIT=0` disables. Orthogonal to anti-spin
    // (identical-batch) — this catches *changing-but-failing* calls.
    let error_stop = env_usize("ANGEL_ERROR_LIMIT", if task_active { 6 } else { 0 });
    let error_nudge = (error_stop / 2).max(2);
    let mut err_streak = 0usize;
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
    // First-write pressure is operator opt-in. Real evidence and proof audits
    // can require long read-only stretches; inferred limits coerced premature
    // edits without guaranteeing progress.
    let first_write_limit = configured_first_write_limit();
    let first_write_rejection_limit = configured_first_write_rejection_limit(competition);
    let mut prewrite_calls = 0usize;
    let mut first_write_attempted = false;
    let mut first_write_nudge_emitted = false;
    let mut first_write_rejections = 0usize;
    // After a real green verifier, force the model to answer rather than thrash
    // until max hops (Roll 09 OpenCC: gold patches with protocol_completed=0).
    // Two grace batches: a real task typically owes a commit and an artifact
    // step after its verifier goes green.
    let post_green_tool_budget = env_usize("ANGEL_POST_GREEN_TOOL_BATCHES", 0);
    let mut green_verify_achieved = false;
    // The model's last passing test run and the workspace it passed on. Before
    // "done" is accepted, the run is repeated on the unchanged code: one pass
    // can be luck (see `confirm_green_run`).
    let mut last_green_run: Option<GreenRun> = None;
    let confirm_green_runs = confirm_green_extra_runs(competition);
    let mut confirm_green_rejections = 0usize;
    let mut green_verify_nudge_emitted = false;
    let mut post_green_tool_batches = 0usize;
    let mut consecutive_verification_failures = 0usize;
    let mut verification_recovery_emitted = false;
    // Coding tasks that answer "done" without mutating: default disabled (no synthetic completion denial).
    let no_edit_answer_guard = env_flag("ANGEL_NO_EDIT_ANSWER_GUARD", false);
    // Paths that look like tests created/rewritten this turn — green checks that
    // only name these basenames do not clear verify-before-done.
    let mut self_authored_test_basenames: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    let self_authored_verify_guard = env_flag("ANGEL_SELF_AUTHORED_VERIFY_GUARD", false);
    let mut self_authored_verify_nudge_emitted = false;
    // Completion verification: in competition mode, never deny completion or block answers.
    let verify_before_done = if competition {
        false
    } else {
        env_flag("ANGEL_VERIFY_BEFORE_DONE", false)
    };
    let mut verification_needed = false;
    let mut attempted_opaque_generation = 0;
    let post_edit_logic_review = env_flag("ANGEL_POST_EDIT_LOGIC_REVIEW", true);
    let mut post_edit_logic_reviewed = false;
    let final_verification_max_nudges = env_usize("ANGEL_VERIFY_NUDGES", 2).min(4);
    let mut final_verification_nudges = 0usize;
    // Set once the operator answers the first verification-gate modal with
    // "approve": the gate stands down for the rest of this turn.
    let mut verification_gate_released = false;
    let unverified_completion_claims = std::cell::Cell::new(0usize);
    // Re-running an identical verifier against identical Git-backed workspace
    // bytes cannot produce new repository evidence. Reuse a prior conclusive
    // pass/fail result so the policy spends its remaining horizon fixing code or
    // finishing instead of repeatedly testing an unchanged tree. Non-Git
    // workspaces and inconclusive/denied attempts fail open and execute normally.
    let reuse_verifier_results = env_flag("ANGEL_REUSE_VERIFIER_RESULTS", true);
    // Reuse a successful exact verifier invocation on unchanged workspace bytes.
    // A different tool or argument set is a distinct obligation and must execute;
    // a compile success cannot stand in for an unexecuted behavioral test.
    let single_green_verifier = env_flag("ANGEL_SINGLE_GREEN_VERIFIER", true);
    let mut verifier_results: HashMap<(String, String), VerificationOutcome> = HashMap::new();
    // Attempt identity is not a green receipt: remember red/inconclusive attempts
    // too, so post-green exemptions cannot become an unchanged-verifier retry lane.
    let mut attempted_verifier_invocations = std::collections::HashSet::new();
    let mut sufficient_green_verifiers: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    let redundant_verifier_skips = std::cell::Cell::new(0usize);
    // Evaluator-only last-mile reserve. Once a real mutation exists and the
    // bounded horizon is close, force unresolved work toward verification or a
    // concrete follow-up edit instead of allowing the remaining calls to drain
    // into broad inspection. Zero preserves the exact historical off-control.
    let final_mile_hops = if research_turn {
        0
    } else {
        env_usize("ANGEL_FINAL_MILE_HOPS", 0)
    };
    let final_mile_answer_hops = env_usize("ANGEL_FINAL_MILE_ANSWER_HOPS", 0).min(final_mile_hops);
    let mut mutation_seen = false;
    let mut final_mile_active = false;
    let mut final_mile_answer_notice_sent = false;
    let mut hooks_serial_notice_sent = false;
    // Tool names are not proof of workspace state: shell, MCP, code mode, or a
    // nested integration can edit without presenting as a direct write call.
    // Compare the real Git-backed state at finalization and after verifiers so
    // an opaque edit cannot bypass verify-before-done.
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
    let deferred_action_stop = env_usize("ANGEL_DEFERRED_ACTION_LIMIT", 0);
    let mut deferred_action_nudges = 0usize;
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
        counters.redundant_verifier_skips = redundant_verifier_skips.get();
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
    let task_accept_rejection_limit = env_usize("ANGEL_TASK_ACCEPT_REJECTIONS", 2).clamp(1, 4);
    let mut task_accept_rejections = 0usize;
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
        let baseline = run_task_accept(&command, registry.current_workspace());
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
    let mut unproductive_redirections = 0usize;
    let mut spin_redirections = 0usize;
    let mut error_redirections = 0usize;
    macro_rules! handle_unproductive_streak {
        () => {{
            let streak_stop = unproductive_policy.stop;
            let (notice, diagnosis) = crate::agent::harness::trajectory::unproductive_escalation(
                hop,
                streak_escalate,
                streak_stop,
            );
            if let Some(note) = notice {
                history.push(ChatMsg::harness(note.clone()));
                let _ = events.send(TurnEvent::Notice(note));
            }
            if let Some(note) = diagnosis {
                if unproductive_redirections < 2 {
                    unproductive_redirections += 1;
                    let redirect = format!(
                        "[harness-telemetry] MANDATORY PROGRESS REDIRECTION: {note}. \
                         You must stop inspecting and stop running unchanged commands. You MUST edit \
                         the target source code using `write_file` or `str_replace` before executing \
                         any more tools. State your concrete fix and modify the file now."
                    );
                    history.push(ChatMsg::harness(redirect.clone()));
                    let _ = events.send(TurnEvent::Notice(redirect));
                    crate::agent::harness::trajectory::clear_unproductive_streak();
                } else {
                    let note =
                        format!("{note}; operator cap ANGEL_UNPRODUCTIVE_STREAK_STOP={streak_stop}");
                    crate::agent::harness::trajectory::note_timing(
                        &timing.finish_with_history(turn_start.elapsed().as_millis(), history),
                    );
                    crate::agent::harness::trajectory::note_stop_reason("escalated_unproductive");
                    log_trajectory(club, history, &note, hop, true, verdicts.reward());
                    write_exp(
                        "escalated_unproductive",
                        false,
                        hop,
                        turn_counters!(
                            deferred_action_nudges,
                            spin,
                            err_streak,
                            0,
                            first_write_rejections,
                            duplicate_inspection_results,
                            duplicate_inspection_bytes_saved,
                            final_verification_nudges,
                            action_capsule_metrics,
                        ),
                    );
                    observed_outcome!(
                        TurnOutcome::stopped(note, TurnStopReason::EscalatedUnproductive, hop),
                        "escalated_unproductive"
                    );
                }
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
            crate::agent::harness::trajectory::note_timing(
                &timing.finish_with_history(turn_start.elapsed().as_millis(), history),
            );
            crate::agent::harness::trajectory::note_stop_reason(TurnStopReason::Interrupt.as_str());
            log_trajectory(club, history, &note, hop, true, None);
            write_exp(
                "interrupt",
                false,
                hop,
                turn_counters!(
                    deferred_action_nudges,
                    spin,
                    err_streak,
                    0,
                    first_write_rejections,
                    duplicate_inspection_results,
                    duplicate_inspection_bytes_saved,
                    final_verification_nudges,
                    action_capsule_metrics,
                ),
            );
            observed_outcome!(
                TurnOutcome::stopped(note, TurnStopReason::Interrupt, hop),
                "interrupt"
            );
        }
        handle_unproductive_streak!();
        // Reserve the final tenth of the declared wall for composition. Start
        // considering it with two reservations left; never borrow past the wall.
        let compose_reservation = Duration::from_secs(turn_budget as u64) / 10;
        let remaining_deadline =
            turn_deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));
        let compose_fits = remaining_deadline
            .is_none_or(|remaining| !remaining.is_zero() && remaining >= compose_reservation);
        let research_compose = research_turn
            && !research_compose_sent
            && (remaining_deadline.is_some_and(|remaining| remaining <= compose_reservation * 2)
                || max_hops.is_some_and(|max| hop >= max.saturating_sub(1))
                || research::repeated_search(history));
        if turn_expired(turn_start.elapsed().as_secs(), turn_budget)
            || ((research_compose || research_compose_sent) && !compose_fits)
        {
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
            crate::agent::harness::trajectory::note_timing(
                &timing.finish_with_history(turn_start.elapsed().as_millis(), history),
            );
            crate::agent::harness::trajectory::note_stop_reason(TurnStopReason::Deadline.as_str());
            log_trajectory(club, history, &note, hop, true, verdicts.reward());
            write_exp(
                "deadline",
                false,
                hop,
                turn_counters!(
                    deferred_action_nudges,
                    spin,
                    err_streak,
                    0,
                    first_write_rejections,
                    duplicate_inspection_results,
                    duplicate_inspection_bytes_saved,
                    final_verification_nudges,
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
            crate::agent::harness::trajectory::note_timing(
                &timing.finish_with_history(turn_start.elapsed().as_millis(), history),
            );
            crate::agent::harness::trajectory::note_stop_reason(TurnStopReason::MaxHops.as_str());
            log_trajectory(club, history, &message, hop, true, verdicts.reward());
            write_exp(
                "max_hops",
                false,
                hop,
                turn_counters!(
                    deferred_action_nudges,
                    spin,
                    err_streak,
                    0,
                    first_write_rejections,
                    duplicate_inspection_results,
                    duplicate_inspection_bytes_saved,
                    final_verification_nudges,
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
                let answer = research::draft(history).unwrap_or_else(|| {
                    "Evidence is missing; no research draft was produced before the hop cap.".into()
                });
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
        let final_mile_answer_only = should_force_final_mile_answer(
            max_hops,
            hop,
            final_mile_answer_hops,
            final_mile_active,
        );
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
        if final_mile_answer_only && !final_mile_answer_notice_sent {
            final_mile_answer_notice_sent = true;
            crate::agent::harness::trajectory::note_escalation(hop, "final_mile_answer_advisory");
            history.push(ChatMsg::harness(FINAL_MILE_ANSWER_NUDGE.to_string()));
            let _ = events.send(TurnEvent::Notice(
                "final-mile answer window active; schemas retained, tool calls disabled".into(),
            ));
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
        // Compaction may have consumed the prior directive. Restore it only at
        // the tail so the transport still forbids calls in this window.
        if final_mile_answer_only && !crate::agent::club::final_response_requested(history) {
            history.push(ChatMsg::harness(FINAL_MILE_ANSWER_NUDGE));
        }
        if research_compose {
            research_compose_sent = true;
            history.push(ChatMsg::harness(research::COMPOSE));
            crate::agent::harness::trajectory::note_escalation(hop, "research_compose");
            let _ = events.send(TurnEvent::Notice(
                "Research compose step: answer from evidence or decline citing nothing".into(),
            ));
        }
        if task_capture.is_some() {
            inject_task_proc_completions(registry, history, events);
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
        if let Some(notify) = watch_notify {
            let text = notify.injection_text();
            history.push(ChatMsg::harness(text.clone()));
            let _ = events.send(TurnEvent::Notice(text));
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
        let mut empty_reply_nudged = false;
        let mut output_cap_nudges = 0usize;
        let mut nudge_empty_reply = |error: &str, history: &mut Vec<ChatMsg>| {
            // A reply cut off at a fixed output cap comes back identical on a
            // plain re-send; tell the model so the retry is a different request.
            if is_output_cap_truncation(error) && output_cap_nudges < OUTPUT_CAP_NUDGE_LIMIT {
                output_cap_nudges += 1;
                history.push(ChatMsg::harness(format!("{OUTPUT_CAP_NUDGE}\n({error})")));
                return;
            }
            if is_empty_reply_error(error) && !empty_reply_nudged {
                empty_reply_nudged = true;
                history.push(ChatMsg::harness(format!(
                    "{TELEMETRY_MARK}Your previous reply arrived empty — no text \
                     and no tool calls were received. Respond now with either \
                     structured tool calls or answer text."
                )));
            }
        };
        crate::agent::turn::phase::mark("context_assembled");
        let reply = loop {
            // A retry backoff can cross the existing turn deadline. Settle at
            // the owned boundary before opening another provider request.
            if cancel.load(Ordering::Acquire)
                || turn_expired(turn_start.elapsed().as_secs(), turn_budget)
                || (research_compose
                    && turn_deadline.is_some_and(|deadline| {
                        deadline.saturating_duration_since(Instant::now()) < compose_reservation
                    }))
            {
                continue 'turn;
            }
            // Preflight every actual provider attempt, including in-hop
            // retries. Provider counters are preferred after a call (and can
            // expose internal fan-out); the local request estimate is the
            // conservative fallback when a backend reports no usage.
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
            let has_friction = err_streak > 0 || spin >= 2;
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
            let result = with_turn_deadline_cancel(cancel, turn_deadline, |effective_cancel| {
                crate::agent::turn::phase::mark("bind_run_identity");
                club.bind_run_identity(effective_effort.as_deref())?;
                // Check after checkpointing/preflight and identity binding too:
                // those can consume the remaining wall before transport starts.
                if effective_cancel.load(Ordering::Acquire)
                    || turn_deadline.is_some_and(|deadline| {
                        let remaining = deadline.saturating_duration_since(Instant::now());
                        remaining.is_zero() || (research_compose && remaining < compose_reservation)
                    })
                {
                    provider_not_started = true;
                    return Err("provider request not started: turn deadline reached".into());
                }
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
            crate::agent::harness::trajectory::note_timing(
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
                        history.push(ChatMsg::harness(format!("{TELEMETRY_MARK}{note}")));
                        let _ = events.send(TurnEvent::Notice(note.clone()));
                        if fault.retries_same_club()
                            && retry_budget_allows(provider_retries, provider_attempt)
                        {
                            provider_attempt = provider_attempt.saturating_add(1);
                            nudge_empty_reply(&err, history);
                            continue;
                        }
                        // Recovery notes are harness diagnostics, never an
                        // answer. Seal the same failure ledger/envelope as an
                        // exhausted cloud request, including local-seat deaths.
                        stop_after_session_recovery = true;
                    }
                    if !cancel.load(Ordering::Relaxed)
                        && !stop_after_session_recovery
                        && retry_allowed
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
                        nudge_empty_reply(&err, history);
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
                            deferred_action_nudges,
                            spin,
                            err_streak,
                            0,
                            first_write_rejections,
                            duplicate_inspection_results,
                            duplicate_inspection_bytes_saved,
                            final_verification_nudges,
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
            crate::agent::club::ClubReply::Calls(calls) if final_mile_answer_only => {
                // A provider can still violate an empty tool schema by
                // returning remembered/native calls. Never execute those in
                // the response-only window: close truthfully over the
                // workspace and receipts already produced.
                let _ = crate::agent::club::take_pending_tool_reasoning();
                let _ = crate::agent::club::take_pending_tool_content();
                let names = calls
                    .iter()
                    .map(|call| call.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = events.send(TurnEvent::SuppressPartial);
                let _ = events.send(TurnEvent::Notice(format!(
                    "final-mile answer window withheld {} unavailable tool call(s)",
                    calls.len()
                )));
                crate::agent::club::ClubReply::Text(format!(
                    "Final response window closed without a model-authored answer: the model \
                     attempted unavailable tool calls ({names}) after tool schemas were \
                     withdrawn. None executed; the workspace and prior verification receipts \
                     are preserved."
                ))
            }
            crate::agent::club::ClubReply::Text(answer)
                if final_mile_answer_only
                    && crate::agent::club::contains_raw_tool_markup(&answer) =>
            {
                // Raw markup is not an executable fallback when schemas are
                // absent. Replace it with an honest boundary result instead
                // of publishing apparent tool output that never happened.
                let _ = events.send(TurnEvent::SuppressPartial);
                let _ = events.send(TurnEvent::Notice(
                    "final-mile answer window withheld raw tool markup".into(),
                ));
                crate::agent::club::ClubReply::Text(
                    "Final response window closed without a model-authored answer: the model \
                     printed tool markup after tool schemas were withdrawn. Nothing in that \
                     markup executed; the workspace and prior verification receipts are \
                     preserved."
                        .into(),
                )
            }
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
                                deferred_action_nudges,
                                spin,
                                err_streak,
                                0,
                                first_write_rejections,
                                duplicate_inspection_results,
                                duplicate_inspection_bytes_saved,
                                final_verification_nudges,
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
                    let _ = events.send(TurnEvent::SuppressPartial);
                    history.push(ChatMsg::harness(final_check_notes.join("\n\n")));
                    // Existing hop/deadline/cancellation bounds still govern
                    // this recovery. A withheld answer is not completed work.
                    continue;
                }
                // A long provider call can race a background build's exit.
                // Let the task see the newly finished job before accepting an
                // answer based on its obsolete "running" snapshot. Existing
                // hop/deadline bounds and explicit escalation retain priority.
                if task_capture.is_some()
                    && !final_mile_answer_only
                    && inject_task_proc_completions(registry, history, events) > 0
                {
                    let _ = events.send(TurnEvent::SuppressPartial);
                    history.push(ChatMsg::harness(
                        "Background work finished while your answer was being generated. \
                         Inspect its proc_status outcome and incorporate it before finishing."
                            .to_string(),
                    ));
                    continue;
                }
                // A headless answer shuts down its process registry. Real
                // proof runs repeatedly answered while their last build was
                // still running, losing that work despite completion notices.
                // Give only this task's jobs a bounded, cancellable idle window
                // outside the provider/tool call. Interactive proc_status stays
                // nonblocking; the existing hop/deadline/final-mile policy wins.
                let proc_owner = cancel as *const AtomicBool as usize;
                let proc_workspace = &registry.workspace_boundary().canonical_root;
                if task_capture.is_some()
                    && crate::agent::tools::proc::owned_work_pending(proc_owner, proc_workspace)
                {
                    // The bounded answer window cannot inspect or finish this
                    // work. Preserve the model's handoff, but never label a
                    // task that is about to stop its live jobs as completed.
                    if final_mile_answer_only {
                        let reason =
                            if turn_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                                TurnStopReason::Deadline
                            } else {
                                TurnStopReason::MaxHops
                            };
                        let source = reason.as_str();
                        history.push(ChatMsg::assistant(answer.clone()));
                        crate::agent::harness::trajectory::note_stop_reason(source);
                        log_trajectory(club, history, &answer, hop, true, verdicts.reward());
                        observed_outcome!(
                            TurnOutcome::stopped(answer, reason, hop).with_stop_notice(
                                "Task stopped with owned background work unfinished; task exit stops remaining jobs."
                                    .to_string(),
                            ),
                            source
                        );
                    }
                    let _ = events.send(TurnEvent::SuppressPartial);
                    let _ = events.send(TurnEvent::Notice(
                        "Headless answer deferred: owned background work is still running; waiting up to 30s for its outcome.".to_string(),
                    ));
                    let idle_until = Instant::now() + Duration::from_secs(30);
                    while !cancel.load(Ordering::Acquire)
                        && Instant::now() < idle_until
                        && turn_deadline.is_none_or(|deadline| Instant::now() < deadline)
                        && crate::agent::tools::proc::owned_work_pending(proc_owner, proc_workspace)
                    {
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    inject_task_proc_completions(registry, history, events);
                    history.push(ChatMsg::harness(
                        "Your final answer was deferred because this task still owned running background work. \
                         Inspect proc_status and finish from its actual outcome. If a job is no longer needed, \
                         explicitly stop it with proc_stop before answering. Do not report an in-flight build \
                         as complete; task exit stops remaining jobs."
                            .to_string(),
                    ));
                    continue;
                }
                // Raw tool markup surviving into a text answer means the model
                // "called" a tool in prose that never ran — whatever the text
                // claims about results is invented. Refuse it like an
                // announce-only false start and force a real call.
                let raw_markup =
                    !defs.is_empty() && crate::agent::club::contains_raw_tool_markup(&answer);
                if raw_markup {
                    deferred_action_nudges += 1;
                    // Raw tool markup is unsafe to preserve: it looks executable
                    // while no tool actually ran. A plain progress sentence is
                    // different — the operator already saw it stream, and
                    // erasing it leaves a baffling blank answer until the next
                    // tool event. Keep legitimate status prose visible while we
                    // still nudge the model to continue with a real tool call.
                    if raw_markup {
                        let _ = events.send(TurnEvent::SuppressPartial);
                    }
                    history.push(ChatMsg::assistant(answer.clone()));
                    if deferred_action_stop > 0 && deferred_action_nudges >= deferred_action_stop {
                        let note = format!(
                            "⚠ stopped after {deferred_action_nudges} assistant false-starts: \
                             the model kept announcing work or printing raw tool markup \
                             without issuing tool calls. Conversation kept — send another \
                             message to retry or steer it."
                        );
                        crate::agent::harness::trajectory::note_timing(
                            &timing.finish_with_history(turn_start.elapsed().as_millis(), history),
                        );
                        crate::agent::harness::trajectory::note_stop_reason(
                            TurnStopReason::DeferredStop.as_str(),
                        );
                        log_trajectory(club, history, &note, hop, true, verdicts.reward());
                        write_exp(
                            "deferred_stop",
                            false,
                            hop,
                            turn_counters!(
                                deferred_action_nudges,
                                spin,
                                err_streak,
                                0,
                                first_write_rejections,
                                duplicate_inspection_results,
                                duplicate_inspection_bytes_saved,
                                final_verification_nudges,
                                action_capsule_metrics,
                            ),
                        );
                        observed_outcome!(
                            TurnOutcome::stopped(note, TurnStopReason::DeferredStop, hop),
                            "deferred_stop"
                        );
                    }
                    let _ = events.send(TurnEvent::Notice(format!(
                        "{} ({deferred_action_nudges}/{deferred_action_stop})",
                        if raw_markup {
                            "assistant printed raw tool markup; forcing a real tool call"
                        } else {
                            "assistant announced work without tools; forcing tool use"
                        }
                    )));
                    let correction = if raw_markup {
                        "Your previous message printed raw tool markup as plain text — no \
                         tool was executed, and any results it described were invented. \
                         Re-issue the action through the structured tool-call interface now \
                         (no tool markup in chat text), then answer from the real output."
                    } else {
                        "Your previous message only announced work; it did not do the work. \
                         Continue this same turn now by issuing structured tool calls only. \
                         Use shell/read/search tools to inspect the workspace, use delegate if \
                         council input was requested, then answer from evidence. Do not write \
                         another status sentence before the tool calls."
                    };
                    // The relentless directive is a standing steer (also re-injected
                    // per submit) — leave it unmarked so it can inform a summary. The
                    // correction is transient false-start commentary: mark it as
                    // telemetry so it never becomes a durable "the agent keeps failing"
                    // note. Split into two messages so only the correction is dropped
                    // by `render_transcript`; both stay visible to the model in the tail.
                    history.push(ChatMsg::harness(RELENTLESS_EXECUTION_DIRECTIVE.to_string()));
                    history.push(ChatMsg::harness(format!("{TELEMETRY_MARK}{correction}")));
                    continue;
                }
                let current_workspace_fingerprint =
                    workspace_fingerprint(registry.current_workspace());
                let current_acceptance_workspace = task_accept_cmd
                    .as_ref()
                    .and_then(|_| workspace_evidence_sha256(registry.current_workspace()));
                let workspace_changed =
                    match (initial_workspace_fingerprint, current_workspace_fingerprint) {
                        (Some(initial), Some(current)) => current != initial,
                        _ => false,
                    };
                let workspace_changed_after_verification =
                    match (initial_workspace_fingerprint, current_workspace_fingerprint) {
                        (Some(initial), Some(current)) => {
                            current != initial
                                && verification_attempt_workspace_fingerprint != Some(current)
                                && prose_only_workspace_fingerprint != Some(current)
                        }
                        _ => false,
                    };
                // An evaluator-pinned task acceptance command is a hard
                // completion contract. Passing it can terminate immediately at
                // the post-mutation seam below; a red result must likewise be
                // authoritative here. Previously the ordinary verification
                // nudge budget could expire and publish the model's "done"
                // answer even while this deterministic predicate remained red.
                if let Some(command) = task_accept_cmd.as_deref() {
                    let last_result = accept_last_post_result.or(accept_baseline_result);
                    let reuse_unchanged_red = current_acceptance_workspace.is_some()
                        && current_acceptance_workspace == accept_last_checked_workspace
                        && last_result.is_some_and(|result| result != "passed")
                        && accept_last_summary.is_some();
                    let (passed, summary) = if reuse_unchanged_red {
                        (
                            false,
                            format!(
                                "unchanged red task acceptance replay (process skipped): {}",
                                accept_last_summary
                                    .as_deref()
                                    .unwrap_or("task acceptance failed")
                            ),
                        )
                    } else {
                        let proof = run_task_accept(command, registry.current_workspace());
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
                        let summary = if proof.result_class == "flaky" {
                            format!(
                                "{}\nFailing run output:\n{}",
                                proof.summary, proof.output_tail
                            )
                        } else {
                            proof.summary
                        };
                        (proof.passed, summary)
                    };
                    if passed {
                        crate::agent::harness::trajectory::note_verified(
                            turn_start.elapsed().as_millis() as u64,
                        );
                        verification_needed = false;
                        verification_attempt_workspace_fingerprint = current_workspace_fingerprint;
                        green_verify_achieved = true;
                        let _ = events.send(TurnEvent::Notice(format!(
                            "task acceptance passed at finalization: {summary}"
                        )));
                    } else {
                        task_accept_rejections = task_accept_rejections.saturating_add(1);
                        let _ = events.send(TurnEvent::SuppressPartial);
                        history.push(ChatMsg::assistant(answer));
                        history.push(ChatMsg::harness(format!(
                            "{TASK_ACCEPT_RED_NUDGE}\nLatest receipt: {summary}"
                        )));
                        let _ = events.send(TurnEvent::Notice(format!(
                            "task acceptance remains red; denying completion ({task_accept_rejections}/{task_accept_rejection_limit}): {summary}"
                        )));
                        if task_accept_rejections >= task_accept_rejection_limit {
                            let note = format!(
                                "⚠ stopped after {task_accept_rejections} completion claim(s) while the operator-pinned task acceptance remained red. Last receipt: {summary}"
                            );
                            crate::agent::harness::trajectory::note_timing(
                                &timing
                                    .finish_with_history(turn_start.elapsed().as_millis(), history),
                            );
                            crate::agent::harness::trajectory::note_stop_reason(
                                TurnStopReason::AcceptanceStop.as_str(),
                            );
                            log_trajectory(club, history, &note, hop, true, verdicts.reward());
                            write_exp(
                                "acceptance_stop",
                                false,
                                hop,
                                turn_counters!(
                                    deferred_action_nudges,
                                    spin,
                                    err_streak,
                                    0,
                                    first_write_rejections,
                                    duplicate_inspection_results,
                                    duplicate_inspection_bytes_saved,
                                    final_verification_nudges,
                                    action_capsule_metrics,
                                ),
                            );
                            observed_outcome!(
                                TurnOutcome::stopped(note, TurnStopReason::AcceptanceStop, hop),
                                "acceptance_stop"
                            );
                        }
                        continue;
                    }
                }
                if confirm_green_runs > 0
                    && confirm_green_rejections < CONFIRM_GREEN_REJECTION_LIMIT
                    && last_green_run.as_ref().is_some_and(|green| {
                        green.workspace.is_some()
                            && green.workspace == current_workspace_fingerprint
                    })
                {
                    let green = last_green_run.take().expect("checked above");
                    let green_call = green.call;
                    let label = green_run_label(&green_call);
                    // A substitute runner has not run yet: it owes one more run.
                    let runs = confirm_green_runs + usize::from(green.substitute);
                    match confirm_green_run(registry, &green_call, runs, cancel) {
                        Ok(runs) => {
                            let _ = events.send(TurnEvent::Notice(format!(
                                "confirmed green: re-ran {label} {runs} more time(s) on the final code"
                            )));
                        }
                        Err((run, output)) => {
                            confirm_green_rejections += 1;
                            crate::agent::harness::trajectory::note_escalation(
                                hop,
                                "confirm_green_flaky",
                            );
                            let _ = events.send(TurnEvent::SuppressPartial);
                            history.push(ChatMsg::assistant(answer));
                            history.push(ChatMsg::harness(format!(
                                "{CONFIRM_GREEN_NUDGE}\nRe-run {run} of {runs} of {label} \
                                 failed:\n{}",
                                tail_chars(&output, TASK_ACCEPT_TAIL_CHARS)
                            )));
                            let _ = events.send(TurnEvent::Notice(format!(
                                "passing tests did not hold: re-run {run} of {label} failed on the \
                                 same code; denying completion ({confirm_green_rejections}/\
                                 {CONFIRM_GREEN_REJECTION_LIMIT})"
                            )));
                            continue;
                        }
                    }
                }
                let verification_outstanding = verification_needed
                    || workspace_changed_after_verification
                    || registry.mutation_targets.opaque_generation() > attempted_opaque_generation;
                if !verification_gate_released
                    && should_nudge_final_verification(
                        verify_before_done,
                        verification_outstanding,
                        final_verification_nudges,
                        final_verification_max_nudges,
                    )
                {
                    // First denial of the turn is the one the operator can still
                    // act on, so it prompts rather than murmurs. Approving
                    // releases the gate for the rest of the turn — re-asking on
                    // every hop would just be the same nag with a modal.
                    if final_verification_nudges == 0
                        && operator_releases_unverified_completion(registry.current_workspace())
                    {
                        verification_gate_released = true;
                        let _ = events.send(TurnEvent::Notice(
                            "verification gate released by operator; accepting the completion \
                             unverified"
                                .to_string(),
                        ));
                    } else {
                        final_verification_nudges += 1;
                        // The provider already streamed this answer. Retract it from
                        // the live pane, retain it in the model tail as the claim it
                        // must now substantiate, and add one transient policy nudge.
                        let _ = events.send(TurnEvent::SuppressPartial);
                        history.push(ChatMsg::assistant(answer));
                        crate::agent::harness::trajectory::note_escalation(
                            hop,
                            "final_verify_advisory",
                        );
                        history.push(ChatMsg::harness(FINAL_VERIFY_NUDGE.to_string()));
                        let _ = events.send(TurnEvent::Notice(
                            format!(
                                "edited workspace is not yet verified; denying unsupported completion ({final_verification_nudges}/{final_verification_max_nudges})"
                            ),
                        ));
                        continue;
                    }
                }
                // Optional soft advisory note for no-edit answers, never deny completion.
                if no_edit_answer_guard
                    && first_write_limit > 0
                    && !mutation_seen
                    && !workspace_changed
                    && !green_verify_achieved
                {
                    crate::agent::harness::trajectory::note_escalation(hop, "no_edit_advisory");
                    history.push(ChatMsg::harness(NO_EDIT_ANSWER_NUDGE.to_string()));
                }
                // An operator release is still an unverified claim shipping —
                // the ledger records it as one, exactly like exhausting the
                // nudge budget would.
                if verification_gate_released
                    || accepted_unverified_completion(
                        verify_before_done,
                        verification_outstanding,
                        final_verification_nudges,
                        final_verification_max_nudges,
                    )
                {
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
                history.push(ChatMsg::assistant(answer.clone()));
                crate::agent::harness::trajectory::note_timing(
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
                        deferred_action_nudges,
                        spin,
                        err_streak,
                        0,
                        first_write_rejections,
                        duplicate_inspection_results,
                        duplicate_inspection_bytes_saved,
                        final_verification_nudges,
                        action_capsule_metrics,
                    ),
                );
                observed_outcome!(TurnOutcome::answer(answer, hop), "answer");
            }
            crate::agent::club::ClubReply::Calls(mut calls) => {
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
                    hop_budget_classify_applied(competition, first_write_limit, hop_path_active),
                    &calls,
                );
                let first_write_guard_suppressing = first_write_limit > 0
                    && first_write_rejection_limit > 0
                    && !first_write_attempted
                    && prewrite_calls >= first_write_limit
                    && !mutation_this_hop
                    && !outcome_this_hop
                    && burns_budget_this_hop;
                if first_write_guard_suppressing
                    && first_write_rejections >= first_write_rejection_limit
                {
                    let _ = events.send(TurnEvent::SuppressPartial);
                    let note = format!(
                        "⚠ stopped after {first_write_rejections} post-budget inspection \
                         batch(es) were denied without candidate progress. \
                         Conversation kept — the autonomous loop can resume from this evidence."
                    );
                    crate::agent::harness::trajectory::note_timing(
                        &timing.finish_with_history(turn_start.elapsed().as_millis(), history),
                    );
                    crate::agent::harness::trajectory::note_stop_reason(
                        TurnStopReason::Spin.as_str(),
                    );
                    log_trajectory(club, history, &note, hop, true, verdicts.reward());
                    write_exp(
                        "first_write_stop",
                        false,
                        hop,
                        turn_counters!(
                            deferred_action_nudges,
                            spin,
                            err_streak,
                            0,
                            first_write_rejections,
                            duplicate_inspection_results,
                            duplicate_inspection_bytes_saved,
                            final_verification_nudges,
                            action_capsule_metrics,
                        ),
                    );
                    observed_outcome!(
                        TurnOutcome::stopped(note, TurnStopReason::Spin, hop),
                        "first_write_stop"
                    );
                }
                if first_write_guard_suppressing {
                    first_write_rejections = first_write_rejections.saturating_add(1);
                    let _ = events.send(TurnEvent::Notice(format!(
                        "first-write guard: blocked post-budget inspection batch ({first_write_rejections}/{first_write_rejection_limit})"
                    )));
                }
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
                let poll_batch_progress = poll_batch_advances_work(&calls);
                if poll_batch_progress {
                    passive_poll_nudge_sent = false;
                }
                let poll_guard_suppressing = poll_guard_enabled
                    && passive_poll_guard.should_suppress(
                        &calls,
                        true,
                        cadence_verdict,
                        poll_only_limit,
                        passive_sleep_max_secs,
                    );
                let passive_denied_streak = passive_poll_guard.denied_streak();
                let entire_batch_passive = passive_poll_only_batch(&calls, passive_sleep_max_secs);
                let repeated_poll_fingerprint =
                    (poll_repeat_limit > 0 && entire_batch_passive && !poll_batch_progress)
                        .then(|| anti_spin_batch_fingerprint(&calls));
                if poll_guard_suppressing {
                    // Full doctrine once per stall episode; each denied call
                    // still gets its short per-call receipt. Re-teaching the
                    // whole policy every suppressed hop is pure history bloat.
                    if !passive_poll_nudge_sent {
                        crate::agent::harness::trajectory::note_escalation(
                            hop,
                            "passive_poll_advisory",
                        );
                        history.push(ChatMsg::harness(passive_poll_nudge(task_pace).to_string()));
                        passive_poll_nudge_sent = true;
                    }
                    let _ = events.send(TurnEvent::Notice(
                        "passive wait blocked: status/sleep calls were not started; advance the candidate before checking again"
                            .into(),
                    ));
                }
                // Other execution remains uninterrupted: only passive
                // status/sleep members are suppressed; productive siblings in
                // a mixed batch still dispatch.
                // Post-green grace window: after a completion-grade green, the
                // model gets a bounded number of tool batches for protocol
                // steps (commit, artifact dump — the things a real task owes
                // after its verifier passes); exceeding the budget forces an
                // answer so protocol completion is not lost to max_hops
                // (Roll 09 OpenCC). Redundant re-verification inside the
                // window is separately skipped by the sufficient-green
                // interception, so the grace cannot become a thrash lane.
                let distinct_verification_batch = green_verify_achieved
                    && !calls.is_empty()
                    && (reuse_verifier_results || single_green_verifier)
                    && workspace_evidence_sha256(registry.current_workspace()).is_some_and(
                        |state| {
                            let mut batch_keys = std::collections::HashSet::new();
                            calls.iter().all(|call| {
                                if call.name == "shell" || verification_identity(call).is_none() {
                                    return false;
                                }
                                let key = (
                                    state.clone(),
                                    serde_json::json!([
                                        call.name,
                                        call.args,
                                        registry.mutation_targets.snapshot(),
                                        registry.mutation_targets.opaque_generation()
                                    ])
                                    .to_string(),
                                );
                                !attempted_verifier_invocations.contains(&key)
                                    && batch_keys.insert(key)
                            })
                        },
                    );
                // A green compile cannot discharge a different verification
                // obligation. Only wholly distinct verifier batches get this
                // exception; the finite horizon and final-answer reserve still apply.
                if green_verify_achieved && !distinct_verification_batch {
                    post_green_tool_batches = post_green_tool_batches.saturating_add(1);
                    if post_green_tool_budget > 0
                        && post_green_tool_batches > post_green_tool_budget
                    {
                        let mut answer = format!(
                            "A workspace verifier already passed green. Operator cap ANGEL_POST_GREEN_TOOL_BATCHES={post_green_tool_budget} reached."
                        );
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
                        history.push(ChatMsg::assistant(answer.clone()));
                        crate::agent::harness::trajectory::note_timing(
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
                            "post_green_answer",
                            true,
                            hop,
                            turn_counters!(
                                deferred_action_nudges,
                                spin,
                                err_streak,
                                0,
                                first_write_rejections,
                                duplicate_inspection_results,
                                duplicate_inspection_bytes_saved,
                                final_verification_nudges,
                                action_capsule_metrics,
                            ),
                        );
                        observed_outcome!(TurnOutcome::answer(answer, hop), "post_green_answer");
                    }
                    // Within the grace budget: dispatch normally so the task's
                    // protocol steps (commit, dump) actually run.
                    let note = if post_green_tool_budget == 0 {
                        format!("post-green work continues: batch {post_green_tool_batches}")
                    } else {
                        format!(
                            "post-green grace batch {post_green_tool_batches}/{post_green_tool_budget}; ANGEL_POST_GREEN_TOOL_BATCHES={post_green_tool_budget}"
                        )
                    };
                    let _ = events.send(TurnEvent::Notice(note));
                }
                // Soft advisory notice if final mile or first-write budget is reached,
                // but NEVER drop or reject the agent's tool calls.
                if final_mile_rejects_inspection(final_mile_active, verification_needed, &calls) {
                    crate::agent::harness::trajectory::note_escalation(hop, "final_mile_advisory");
                    history.push(ChatMsg::harness(FINAL_MILE_NUDGE.to_string()));
                }
                if calls.iter().any(|c| c.id.starts_with("prose_")) {
                    let _ = events.send(TurnEvent::SuppressPartial);
                }
                // Anti-spin: fingerprint this batch; bail if it keeps repeating.
                // Pure competition board wait/poll does not advance the counter
                // (legal under first-write; outcome can change under the same call).
                // Poll-guard denials count only when the WHOLE batch was passive
                // (a denied sleep beside a real edit is progress, not spin), and
                // all such batches share one sentinel identity: the deny→retry
                // treadmill varies its polls (sleep 30 → sleep 60 → status …),
                // and per-batch hashing let that variation reset the counter
                // forever — burning a full provider round trip per denial.
                let passive_treadmill = poll_guard_suppressing && entire_batch_passive;
                let count_spin = passive_treadmill
                    || anti_spin_counts_batch(
                        mutation_this_hop,
                        outcome_this_hop,
                        wait_or_progress_this_hop,
                        burns_budget_this_hop,
                    );
                let spin_fp = count_spin.then(|| {
                    if passive_treadmill {
                        PASSIVE_TREADMILL_SPIN_FINGERPRINT
                    } else {
                        anti_spin_batch_fingerprint(&calls)
                    }
                });
                if let Some(sig) = spin_fp {
                    if last_sig == Some(sig) {
                        spin += 1;
                    } else {
                        last_sig = Some(sig);
                        spin = 1;
                    }
                }
                {
                    if spin_stop > 0 && count_spin && spin >= spin_stop {
                        if spin_redirections < 2 {
                            spin_redirections += 1;
                            spin = 0;
                            last_sig = None;
                            let redirect = "[harness-telemetry] MANDATORY REDIRECTION: You have repeated the same tool call multiple times without making progress. You are caught in a deterministic loop. Break this loop immediately: you MUST NOT repeat this call or run another inspection. Step back and use `write_file` to rewrite the implementing file cleanly from first principles, or use `str_replace` to apply a completely different fix. State your new hypothesis and edit the code now.";
                            history.push(ChatMsg::harness(redirect.to_string()));
                            let _ = events.send(TurnEvent::Notice(redirect.to_string()));
                        } else {
                            crate::agent::harness::trajectory::note_escalation(hop, "spin_stop");
                            let note = if spin_fp == Some(PASSIVE_TREADMILL_SPIN_FINGERPRINT) {
                                format!(
                                    "⚠ stopped after {spin} consecutive passive wait batches were \
                                     denied with no candidate progress (deny→retry treadmill). \
                                     Operator cap ANGEL_SPIN_LIMIT={spin_stop}; conversation kept."
                                )
                            } else {
                                format!(
                                    "⚠ stopped after the same tool call repeated {spin}× with no new \
                                     outcome; operator cap ANGEL_SPIN_LIMIT={spin_stop}. Conversation kept."
                                )
                            };
                            crate::agent::harness::trajectory::note_timing(
                                &timing
                                    .finish_with_history(turn_start.elapsed().as_millis(), history),
                            );
                            crate::agent::harness::trajectory::note_stop_reason(
                                TurnStopReason::Spin.as_str(),
                            );
                            log_trajectory(club, history, &note, hop, true, verdicts.reward());
                            write_exp(
                                "spin",
                                false,
                                hop,
                                turn_counters!(
                                    deferred_action_nudges,
                                    spin,
                                    err_streak,
                                    0,
                                    first_write_rejections,
                                    duplicate_inspection_results,
                                    duplicate_inspection_bytes_saved,
                                    final_verification_nudges,
                                    action_capsule_metrics,
                                ),
                            );
                            observed_outcome!(
                                TurnOutcome::stopped(note, TurnStopReason::Spin, hop),
                                "spin"
                            );
                        }
                    }
                }
                // Storm guard: count this batch against the sliding window before
                // anything dispatches, so a suppressed duplicate never runs.
                let storm_counts = toolcall_storm.as_mut().map(|storm| storm.observe(&calls));
                let storm_suppressing = storm_counts.as_ref().is_some_and(|counts| {
                    counts
                        .iter()
                        .any(|count| *count >= TOOLCALL_STORM_THRESHOLD)
                });
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
                let action_batch = if action_capsule_mode.active()
                    && !(poll_guard_suppressing && entire_batch_passive)
                {
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
                                deferred_action_nudges,
                                spin,
                                err_streak,
                                0,
                                first_write_rejections,
                                duplicate_inspection_results,
                                duplicate_inspection_bytes_saved,
                                final_verification_nudges,
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
                    && !storm_suppressing
                    && !poll_guard_suppressing
                    && !first_write_guard_suppressing
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
                    let preview = action_batch_ref.and_then(|batch| batch.contains(index));
                    let mut dispatch_elapsed = None;
                    let denied_this = deny_actions && preview.is_some();
                    let poll_denied_this = poll_guard_suppressing
                        && (is_passive_status_call(call)
                            || shell_passive_sleep_secs(call)
                                .is_some_and(|secs| secs > passive_sleep_max_secs));
                    let first_write_denied_this =
                        first_write_guard_suppressing && burns_first_write_budget(call);
                    let storm_repeat = storm_counts
                        .as_ref()
                        .and_then(|counts| counts.get(index).copied())
                        .filter(|count| *count >= TOOLCALL_STORM_THRESHOLD);
                    let verifier_state = (!denied_this
                        && !poll_denied_this
                        && !first_write_denied_this
                        && storm_repeat.is_none()
                        && (reuse_verifier_results || single_green_verifier)
                        && is_verification_call(call))
                    .then(|| workspace_evidence_sha256(registry.current_workspace()))
                    .flatten();
                    let verifier_identity = verifier_state
                        .as_ref()
                        .and_then(|_| verification_identity(call))
                        // Keep full arguments and tool identity in the cache boundary.
                        // Semantic aliases are not proof of identical dispatch/configuration.
                        .map(|_| {
                            serde_json::json!([
                                call.name,
                                call.args,
                                registry.mutation_targets.snapshot(),
                                registry.mutation_targets.opaque_generation()
                            ])
                            .to_string()
                        });
                    let verifier_key = (reuse_verifier_results && call.name != "shell")
                        .then(|| Some((verifier_state.clone()?, verifier_identity.clone()?)))
                        .flatten();
                    let reused = verifier_key
                        .as_ref()
                        .and_then(|key| verifier_results.get(key).copied());
                    let sufficient_green = (single_green_verifier
                        && reused.is_none()
                        && verification_is_completion_sufficient(call))
                    .then(|| {
                        let state = verifier_state.as_ref()?;
                        let identity = verifier_identity.as_ref()?;
                        sufficient_green_verifiers
                            .contains(&(state.clone(), identity.clone()))
                            .then_some(())
                    })
                    .flatten();
                    let result = if first_write_denied_this {
                        FIRST_WRITE_REJECT_RESULT.to_string()
                    } else if poll_denied_this {
                        // The streak makes the treadmill visible to the model:
                        // each retry sees the count climbing toward the stop.
                        format!(
                            "{PASSIVE_POLL_RESULT} [passive-wait denial \
                             ×{passive_denied_streak} this turn without candidate progress]"
                        )
                    } else if denied_this {
                        format!(
                            "action capsule denied — {} not executed",
                            preview.expect("checked is_some").tool
                        )
                    } else if let Some(count) = storm_repeat {
                        // One operator line per tool name per turn. Window
                        // counts flicker (x3/x4) as hops age out; restacking
                        // that as scrollback is just a repeat notification.
                        if last_storm_notice.as_deref() != Some(call.name.as_str()) {
                            crate::agent::harness::trajectory::note_escalation(
                                hop,
                                "duplicate_storm_advisory",
                            );
                            let _ = events.send(TurnEvent::Notice(format!(
                                "storm: suppressed duplicate {} call (x{count})",
                                call.name
                            )));
                            last_storm_notice = Some(call.name.clone());
                        }
                        duplicate_storm_result(call, count)
                    } else if let Some(outcome) = reused {
                        redundant_verifier_skips
                            .set(redundant_verifier_skips.get().saturating_add(1));
                        cached_verification_result(call, outcome)
                    } else if sufficient_green.is_some() {
                        redundant_verifier_skips
                            .set(redundant_verifier_skips.get().saturating_add(1));
                        sufficient_verification_result(call)
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
                                "tool error: tool_idle: silence limit reached; child tree termination requested; retry with explicit input or use proc_run\n{result}"
                            );
                        }
                        let elapsed = call_started.elapsed();
                        timing.note_tool_call(&call.name, elapsed);
                        dispatch_elapsed = Some(elapsed);
                        result
                    };
                    let not_started = first_write_denied_this
                        || poll_denied_this
                        || storm_repeat.is_some()
                        || reused.is_some()
                        || sufficient_green.is_some();
                    if !not_started
                        && !denied_this
                        && call.name != "shell"
                        && let (Some(state), Some(identity)) = (&verifier_state, &verifier_identity)
                    {
                        attempted_verifier_invocations.insert((state.clone(), identity.clone()));
                    }
                    if !first_write_denied_this
                        && !poll_denied_this
                        && reused.is_none()
                        && sufficient_green.is_none()
                        && storm_repeat.is_none()
                        && let Some(outcome) = registry
                            .routed_execution(call, &result)
                            .map(|entry| entry.outcome.verification)
                            .or_else(|| verification_outcome(call, &result))
                    {
                        if let Some(key) = verifier_key
                            && outcome != VerificationOutcome::Inconclusive
                            && verification_result_covers_changes(&result)
                            && !registry.mutation_targets.is_opaque()
                        {
                            verifier_results.insert(key, outcome);
                        }
                        if outcome == VerificationOutcome::Passed
                            && !registry.mutation_targets.is_opaque()
                            && verification_result_covers_changes(&result)
                            && verification_is_completion_sufficient(call)
                            && let (Some(state), Some(identity)) =
                                (verifier_state, verifier_identity)
                        {
                            sufficient_green_verifiers.insert((state, identity));
                        }
                    }
                    // Every dispatched call has timing, including headless and
                    // YOLO turns. Action previews only control the UI receipt.
                    if let Some((preview, elapsed)) = preview.zip(dispatch_elapsed) {
                        let _ = events.send(TurnEvent::Notice(
                            preview.receipt(&result, elapsed.as_millis()),
                        ));
                    }
                    let outcome = if not_started {
                        ToolOutcome::not_started()
                    } else {
                        registry.executed_outcome(call, &result, denied_this)
                    };
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
                let segments = if !hooks.is_empty()
                    || parallel
                    || storm_suppressing
                    || poll_guard_suppressing
                    || first_write_guard_suppressing
                    || action_capsule_mode.needs_approval()
                {
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
                let cycle_observation = tool_cycle.as_ref().map(|_| {
                    tool_batch_cycle_observation_from_calls_fp(
                        spin_fp.unwrap_or_else(|| anti_spin_batch_fingerprint(&calls)),
                        &results,
                    )
                });
                // Track the consecutive-error streak before the results are capped
                // into history: a hop where every call errored at dispatch.
                let all_errored = !results.is_empty()
                    && results.iter().all(|(result, _, _)| is_error_result(result));
                err_streak = if all_errored { err_streak + 1 } else { 0 };
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
                crate::agent::harness::trajectory::note_timing(
                    &timing.finish_with_history(turn_start.elapsed().as_millis(), history),
                );
                // Calls have now been fully dispatched, so move them into
                // history instead of cloning their potentially huge JSON args
                // before dispatch. Results still follow immediately in the
                // original order, preserving the provider tool-call protocol.
                let call_history_index = history.len();
                history.push(ChatMsg::assistant_calls_full(
                    calls,
                    private_reasoning,
                    tool_content,
                ));
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
                            })
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
                        if is_first_write_progress_call(call) {
                            successful_mutation_this_hop = true;
                            cycle_state_changed_this_hop = true;
                            mutation_seen = true;
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
                            consecutive_verification_failures =
                                consecutive_verification_failures.saturating_add(1);
                            if green_verify_achieved {
                                green_verify_achieved = false;
                                green_verify_nudge_emitted = false;
                                post_green_tool_batches = 0;
                                let _ = events.send(TurnEvent::Notice(
                                    "post-green guard disarmed: an executed verifier failed".into(),
                                ));
                            }
                        } else if outcome == VerificationOutcome::Passed {
                            consecutive_verification_failures = 0;
                            verification_recovery_emitted = false;
                        }
                        let weak_self_authored = self_authored_verify_guard
                            && outcome == VerificationOutcome::Passed
                            && verification_targets_self_authored(
                                call,
                                &self_authored_test_basenames,
                            );
                        if weak_self_authored {
                            if !self_authored_verify_nudge_emitted {
                                self_authored_verify_nudge_emitted = true;
                                // Nudge is deferred until after tool results are
                                // paired so the model sees the green receipt first.
                            }
                            // Keep verification_needed; do not release opaque-write tracking.
                        } else if verification_attempt_releases_gate(call, outcome)
                            && (outcome == VerificationOutcome::Failed
                                || !verification_result_has_known_gap(&result))
                        {
                            // A real attempt releases the gate unless the receipt
                            // names a known gap (a selected target that skipped a
                            // changed file). Unknown coverage — no git, an earlier
                            // opaque shell — is not a gap; `attempted_opaque_generation`
                            // below is what re-arms the gate on a *later* opaque edit.
                            // Result caching and the post-green guard stay strict
                            // (`verification_result_covers_changes`) further down.
                            verification_needed = false;
                            attempted_opaque_generation =
                                registry.mutation_targets.opaque_generation();
                            verification_attempt_workspace_fingerprint =
                                workspace_fingerprint(registry.current_workspace());
                            // Only a full-strength verifier arms the post-green
                            // guard: a filtered slice going green is progress,
                            // not proof, and must not cut off the gates and
                            // protocol steps a task still owes.
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
                    if succeeded
                        && confirm_green_runs > 0
                        && verification_outcome(call, &result) != Some(VerificationOutcome::Failed)
                    {
                        let workspace = workspace_fingerprint(registry.current_workspace());
                        if is_verification_call(call)
                            || is_progress_verifier_call(&call.name, &call.args)
                        {
                            last_green_run = Some(GreenRun {
                                call: call.clone(),
                                workspace,
                                substitute: false,
                            });
                        } else if test_run_behind_fallback(call) && registry.has_tool("run_tests") {
                            // `./test || ctest` can read green while the test
                            // failed; confirm with angelX's own runner instead.
                            last_green_run = Some(GreenRun {
                                call: ToolCall {
                                    id: "confirm_green".into(),
                                    name: "run_tests".into(),
                                    args: serde_json::json!({}),
                                },
                                workspace,
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
                    let produced_bytes = result.len() as u64;
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
                if repeated_poll_guard.observe(repeated_poll_fingerprint, progress_mutated) {
                    let note = format!(
                        "⚠ stopped repeated passive polling: the same status/log batch ran \
                         {poll_repeat_limit} times within 32 polling batches without intervening \
                         work. Changing timestamps are not candidate progress. Conversation kept; \
                         inspect the background job or use its completion notification before \
                         resuming. ANGEL_POLL_REPEAT_LIMIT={poll_repeat_limit} (0 disables)."
                    );
                    crate::agent::harness::trajectory::note_escalation(hop, "repeated_poll_stop");
                    crate::agent::harness::trajectory::note_stop_reason(
                        TurnStopReason::Spin.as_str(),
                    );
                    log_trajectory(club, history, &note, hop, true, verdicts.reward());
                    write_exp(
                        "spin",
                        false,
                        hop,
                        turn_counters!(
                            deferred_action_nudges,
                            poll_repeat_limit,
                            err_streak,
                            0,
                            first_write_rejections,
                            duplicate_inspection_results,
                            duplicate_inspection_bytes_saved,
                            final_verification_nudges,
                            action_capsule_metrics,
                        ),
                    );
                    observed_outcome!(
                        TurnOutcome::stopped(note, TurnStopReason::Spin, hop),
                        "repeated_poll_stop"
                    );
                }
                handle_unproductive_streak!();
                // Stop before another paid model hop. Finish pairing the entire
                // dispatched batch first; unrelated successful calls cannot
                // erase a failed execution prerequisite.
                if let Some(note) = execution_blocked {
                    crate::agent::harness::trajectory::note_timing(
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
                            deferred_action_nudges,
                            spin,
                            err_streak,
                            0,
                            first_write_rejections,
                            duplicate_inspection_results,
                            duplicate_inspection_bytes_saved,
                            final_verification_nudges,
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
                if let Some(detector) = tool_cycle.as_mut() {
                    if cycle_state_changed_this_hop || !count_spin {
                        detector.clear();
                    } else if let Some(period) =
                        cycle_observation.and_then(|observation| detector.observe(observation))
                    {
                        crate::agent::harness::trajectory::note_escalation(hop, "spin_cycle");
                        let repeated_calls = period.saturating_mul(tool_cycle_repeats);
                        let note = format!(
                            "⚠ stopped after detecting a {period}-batch tool cycle repeated \
                             {tool_cycle_repeats}× ({repeated_calls} paired batches) with \
                             unchanged outcomes; operator cap ANGEL_SPIN_LIMIT={spin_stop}, ANGEL_TOOL_CYCLE_REPEATS={tool_cycle_repeats}. Conversation kept — break the cycle with a \
                             different hypothesis/tool or report the blocker."
                        );
                        crate::agent::harness::trajectory::note_timing(
                            &timing.finish_with_history(turn_start.elapsed().as_millis(), history),
                        );
                        crate::agent::harness::trajectory::note_stop_reason(
                            TurnStopReason::Spin.as_str(),
                        );
                        log_trajectory(club, history, &note, hop, true, verdicts.reward());
                        write_exp(
                            "spin_cycle",
                            false,
                            hop,
                            turn_counters!(
                                deferred_action_nudges,
                                spin,
                                err_streak,
                                0,
                                first_write_rejections,
                                duplicate_inspection_results,
                                duplicate_inspection_bytes_saved,
                                final_verification_nudges,
                                action_capsule_metrics,
                            ),
                        );
                        observed_outcome!(
                            TurnOutcome::stopped(note, TurnStopReason::Spin, hop),
                            "spin_cycle"
                        );
                    }
                }
                // Deferred quality guards after tool pairing is intact.
                if self_authored_verify_nudge_emitted
                    && !history.iter().any(|m| {
                        m.role == ChatRole::Harness && m.content.contains("WEAK VERIFICATION")
                    })
                {
                    crate::agent::harness::trajectory::note_escalation(
                        hop,
                        "self_authored_verify_advisory",
                    );
                    history.push(ChatMsg::harness(SELF_AUTHORED_VERIFY_NUDGE.to_string()));
                    let _ = events.send(TurnEvent::Notice(
                        "self-authored verifier guard: green check only covers agent-written tests"
                            .into(),
                    ));
                }
                if consecutive_verification_failures >= 3 && !verification_recovery_emitted {
                    verification_recovery_emitted = true;
                    crate::agent::harness::trajectory::note_escalation(
                        hop,
                        "verification_recovery",
                    );
                    history.push(ChatMsg::harness(
                        "[harness-telemetry] VERIFICATION RECOVERY: 3 consecutive verification failures detected. Pause speculative edits and inspect the first failing diagnostic. If errors span multiple functions, types, or borrow lifetimes, stop micro-patching with str_replace and use write_file to rewrite the module cleanly. Do not re-run tests without changing code. Report infrastructure failures honestly; never discard unrelated changes or assume a clean baseline exists."
                            .to_string(),
                    ));
                    let _ = events.send(TurnEvent::Notice(
                        "verification recovery: inspect repeated failures and preserve existing work"
                            .into(),
                    ));
                }
                if green_verify_achieved && !green_verify_nudge_emitted {
                    green_verify_nudge_emitted = true;
                    crate::agent::harness::trajectory::note_escalation(
                        hop,
                        "green_verify_advisory",
                    );
                    let nudge = if competition {
                        COMPETITION_WINNER_BANK_NUDGE
                    } else {
                        GREEN_VERIFY_DONE_NUDGE
                    };
                    history.push(ChatMsg::harness(nudge.to_string()));
                    let notice_text = if competition {
                        "competition candidate verified; preserve it and follow the authorized submission plan"
                    } else {
                        "green verifier achieved; continue any remaining requested work"
                    };
                    let _ = events.send(TurnEvent::Notice(notice_text.into()));
                }
                if let Some(notify) = slot_watcher.pending_notify() {
                    let text = notify.injection_text();
                    if !history.iter().any(|m| {
                        m.role == ChatRole::Harness
                            && m.content.contains(&notify.id)
                            && m.content.contains(WATCHER_NOTIFY_MARK)
                    }) {
                        history.push(ChatMsg::harness(text.clone()));
                        let _ = events.send(TurnEvent::Notice(text));
                    }
                }
                // Mutation loop allowed without artificial thrash kill switches.
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
                if first_write_limit > 0 && !first_write_attempted {
                    // Intent is not progress: a denied, failed, cancelled, or
                    // panicked mutation must leave later recon behind the gate.
                    if successful_mutation_this_hop {
                        first_write_attempted = true;
                    } else {
                        // Only free-form recon burns the pre-edit budget.
                        // Hilbert/popcorn status·submissions·score and living
                        // handoff / board-tip reads are wait/progress, not thrash.
                        let burned = history[call_history_index]
                            .tool_calls
                            .iter()
                            .filter(|c| burns_first_write_budget(c))
                            .count();
                        prewrite_calls = prewrite_calls.saturating_add(burned);
                        if burned > 0
                            && prewrite_calls >= first_write_limit
                            && !first_write_nudge_emitted
                        {
                            // One advisory per turn. Repeating the same directive
                            // before and after every later call crowds out the
                            // concrete tool evidence needed to recover.
                            crate::agent::harness::trajectory::note_escalation(
                                hop,
                                "first_write_advisory",
                            );
                            first_write_nudge_emitted = true;
                            history.push(ChatMsg::harness(
                                first_write_nudge(competition, task_pace).to_string(),
                            ));
                            let _ = events.send(TurnEvent::Notice(format!(
                                "first-write guard: {prewrite_calls} inspection call(s); mutation or board wait/poll required next"
                            )));
                        }
                    }
                }
                // A mutation after the green stales it: the verifier proved a
                // tree that no longer exists. Disarm the post-green guard so
                // the model can re-verify and finish, instead of being forced
                // to answer for a workspace its green never described (run11
                // lost its district wiring to the sticky version of this).
                if green_verify_achieved && successful_mutation_this_hop {
                    if competition {
                        history.push(ChatMsg::harness(
                            "[harness-telemetry] VERIFIED CANDIDATE CHANGED: The workspace changed after a passing check. Preserve the prior candidate if available and verify the new bytes before claiming success. Follow the operator-authorized submission plan; a local pass alone does not prove a competitive win."
                                .to_string(),
                        ));
                    }
                    green_verify_achieved = false;
                    green_verify_nudge_emitted = false;
                    post_green_tool_batches = 0;
                    let _ = events.send(TurnEvent::Notice(
                        "post-green guard disarmed: the workspace changed after the green".into(),
                    ));
                }
                if post_edit_logic_review
                    && successful_mutation_this_hop
                    && !post_edit_logic_reviewed
                {
                    post_edit_logic_reviewed = true;
                    crate::agent::harness::trajectory::note_escalation(
                        hop,
                        "post_edit_logic_advisory",
                    );
                    history.push(ChatMsg::harness(POST_EDIT_LOGIC_NUDGE.to_string()));
                }
                if should_activate_final_mile(
                    max_hops,
                    hop,
                    final_mile_hops,
                    mutation_seen,
                    final_mile_active,
                ) {
                    final_mile_active = true;
                    crate::agent::harness::trajectory::note_escalation(hop, "final_mile_advisory");
                    history.push(ChatMsg::harness(FINAL_MILE_NUDGE.to_string()));
                    let remaining = max_hops.unwrap_or(hop).saturating_sub(hop);
                    let _ = events.send(TurnEvent::Notice(format!(
                        "final-mile reserve active with {remaining} bounded hop(s) remaining"
                    )));
                }
                if verify_after_hop && let Some(command) = task_accept_cmd.as_deref() {
                    let proof = run_task_accept(command, registry.current_workspace());
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
                        let mut answer = format!(
                            "Acceptance gate passed after {hop} tool hop(s); the verified workspace is ready for inspection."
                        );
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
                        crate::agent::harness::trajectory::note_timing(
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
                                deferred_action_nudges,
                                spin,
                                err_streak,
                                0,
                                first_write_rejections,
                                duplicate_inspection_results,
                                duplicate_inspection_bytes_saved,
                                final_verification_nudges,
                                action_capsule_metrics,
                            ),
                        );
                        observed_outcome!(TurnOutcome::answer(answer, hop), "accept_cmd");
                    } else if proof.result_class == "flaky" {
                        // The model just saw its own run go green; say at once that
                        // the green does not hold, with the failing run's output.
                        let _ = events.send(TurnEvent::Notice(proof.summary.clone()));
                        history.push(ChatMsg::harness(format!(
                            "{}\nFailing run output:\n{}",
                            proof.summary, proof.output_tail
                        )));
                    }
                }
                // One redirect before the hard stop, in case it can self-correct.
                // Perturbation (default) actively reframes; the plain nudge just
                // asks for a different approach.
                if spin > 0 && spin.is_multiple_of(spin_nudge) {
                    crate::agent::harness::trajectory::note_escalation(hop, "spin_advisory");
                    history.push(ChatMsg::harness(spin_redirect(spin_perturb)));
                }
                // Consecutive-error breaker: nudge once at half, hard-stop at the
                // limit — the model is failing every call, not converging.
                if all_errored {
                    if err_streak > 0 && err_streak.is_multiple_of(error_nudge) {
                        crate::agent::harness::trajectory::note_escalation(hop, "error_advisory");
                        history.push(ChatMsg::harness(ERROR_NUDGE.to_string()));
                    }
                    if error_stop > 0 && err_streak >= error_stop {
                        if error_redirections < 2 {
                            error_redirections += 1;
                            err_streak = 0;
                            let redirect = format!(
                                "[harness-telemetry] ERROR CASCADE REDIRECTION: Every tool call in the last {error_stop} hops failed. \
                                 Stop repeating failing commands. Read the compiler diagnostics above and rewrite the file cleanly \
                                 using `write_file` instead of accumulating micro-patches."
                            );
                            history.push(ChatMsg::harness(redirect.clone()));
                            let _ = events.send(TurnEvent::Notice(redirect));
                        } else {
                            crate::agent::harness::trajectory::note_escalation(hop, "error_stop");
                            let note = format!(
                                "⚠ stopped after {err_streak} hops where every tool call errored — \
                                 operator cap ANGEL_ERROR_LIMIT={error_stop}. Conversation kept; \
                                 read the error messages and fix the precondition (path/state/args) \
                                 or change approach."
                            );
                            crate::agent::harness::trajectory::note_timing(
                                &timing
                                    .finish_with_history(turn_start.elapsed().as_millis(), history),
                            );
                            crate::agent::harness::trajectory::note_stop_reason(
                                TurnStopReason::ErrorStop.as_str(),
                            );
                            log_trajectory(club, history, &note, hop, true, verdicts.reward());
                            write_exp(
                                "error_stop",
                                false,
                                hop,
                                turn_counters!(
                                    deferred_action_nudges,
                                    spin,
                                    err_streak,
                                    0,
                                    first_write_rejections,
                                    duplicate_inspection_results,
                                    duplicate_inspection_bytes_saved,
                                    final_verification_nudges,
                                    action_capsule_metrics,
                                ),
                            );
                            observed_outcome!(
                                TurnOutcome::stopped(note, TurnStopReason::ErrorStop, hop),
                                "error_stop"
                            );
                        }
                    }
                }
                // No-progress nudge (hint only): re-reading known files without
                // No synthetic churn/reread stops.
                // Mid-turn hop advisor (ANGEL_ADVISOR=hops): sparse NOTE/BLOCK
                // after tool batches so the next model call sees course-correction.
                if crate::agent::advisor::hops_enabled()
                    && let Some(note) = run_hop_advisor(club, registry, history)
                {
                    history.push(ChatMsg::harness(note.clone()));
                    let _ = events.send(TurnEvent::Notice(note));
                }
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

pub(crate) fn configured_first_write_limit() -> usize {
    env_usize("ANGEL_FIRST_WRITE_CALLS", 0)
}

/// Number of post-budget inspection batches that may be denied before a stuck
/// turn is stopped. Ordinary interactive coding keeps the historical soft-only
/// posture; an explicitly armed competition gets a bounded circuit breaker.
/// Headless task defaults set the same value explicitly, and `0` is exact off.
pub(crate) fn configured_first_write_rejection_limit(_competition: bool) -> usize {
    if std::env::var_os("ANGEL_FIRST_WRITE_REJECTIONS").is_some() {
        return env_usize("ANGEL_FIRST_WRITE_REJECTIONS", 0);
    }
    0
}

#[derive(Debug)]
pub(crate) struct TaskAcceptResult {
    pub(crate) passed: bool,
    pub(crate) result_class: &'static str,
    pub(crate) summary: String,
    pub(crate) elapsed_ms: u128,
    /// The end of a failing run's output; empty when it passed.
    pub(crate) output_tail: String,
}

/// Run the task acceptance command until it has passed
/// `ANGEL_TASK_ACCEPT_REPEATS` times in a row (default 3) or failed once. One
/// green run is not proof: a solution that depends on randomness, timing or
/// state shared between tests can pass by luck. On polyglot-v1
/// cpp-robot-name, a `reset()` that released old names passed about one run
/// in four, and the single acceptance run happened to be one of them. Repeats
/// stop once the proof has taken `ANGEL_TASK_ACCEPT_REPEAT_SECS` (default 60)
/// in total, so a slow suite is not run three times. A failure after an earlier
/// pass comes back as `flaky`, with the failing output.
pub(crate) fn run_task_accept(command: &str, workspace: &Path) -> TaskAcceptResult {
    let repeats = env_usize("ANGEL_TASK_ACCEPT_REPEATS", 3).clamp(1, 10);
    let repeat_budget = Duration::from_secs(env_usize("ANGEL_TASK_ACCEPT_REPEAT_SECS", 60) as u64);
    let started = Instant::now();
    let mut result = run_task_accept_once(command, workspace);
    let mut runs = 1;
    while result.passed && runs < repeats && started.elapsed() < repeat_budget {
        let next = run_task_accept_once(command, workspace);
        runs += 1;
        if !next.passed {
            return TaskAcceptResult {
                passed: false,
                result_class: "flaky",
                summary: format!(
                    "task acceptance is nondeterministic: it passed {} run(s), then failed on run \
                     {runs} of {repeats} ({}). The solution passes by luck; something depends on \
                     randomness, timing or state shared between tests. Make it pass every run.",
                    runs - 1,
                    next.summary
                ),
                elapsed_ms: started.elapsed().as_millis(),
                output_tail: next.output_tail,
            };
        }
        result = next;
    }
    if result.passed && runs > 1 {
        result.summary = format!("{} ({runs} consecutive runs)", result.summary);
    }
    result.elapsed_ms = started.elapsed().as_millis();
    result
}

fn run_task_accept_once(command: &str, workspace: &Path) -> TaskAcceptResult {
    let started = Instant::now();
    let timeout =
        Duration::from_secs(env_usize("ANGEL_TASK_ACCEPT_TIMEOUT_SECS", 120).clamp(5, 600) as u64);
    let Ok((output, timed_out)) =
        crate::agent::harness::exec::sandboxed_workspace_sh(command, workspace, workspace)
            .and_then(|process| output_timed(process, Some(timeout)))
    else {
        return TaskAcceptResult {
            passed: false,
            result_class: "spawn_error",
            summary: "task acceptance command could not start".to_string(),
            elapsed_ms: started.elapsed().as_millis(),
            output_tail: String::new(),
        };
    };
    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    combined.push('\n');
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    let result = parse_test_result(&combined);
    let libtest = combined.contains("test result:");
    let passed = output.status.success()
        && !timed_out
        && (!libtest || (result.passed > 0 && result.failed == 0));
    let status = if timed_out {
        "timed out".to_string()
    } else {
        output
            .status
            .code()
            .map(|code| format!("exit {code}"))
            .unwrap_or_else(|| "terminated by signal".to_string())
    };
    TaskAcceptResult {
        passed,
        result_class: if timed_out {
            "timeout"
        } else if passed {
            "passed"
        } else {
            "failed"
        },
        summary: if libtest {
            format!(
                "task acceptance {status}: {} passed / {} failed",
                result.passed, result.failed
            )
        } else {
            format!("task acceptance {status}")
        },
        elapsed_ms: started.elapsed().as_millis(),
        output_tail: if passed {
            String::new()
        } else {
            tail_chars(&combined, TASK_ACCEPT_TAIL_CHARS)
        },
    }
}

const TASK_ACCEPT_TAIL_CHARS: usize = 1_500;

/// The model's last passing test run, kept for the completion check.
pub(crate) struct GreenRun {
    pub(crate) call: ToolCall,
    /// The workspace it passed on; the check runs only on that same code.
    pub(crate) workspace: Option<u64>,
    /// `run_tests` standing in for a shell run angelX will not repeat as-is.
    pub(crate) substitute: bool,
}

/// A shell test run angelX will not re-run as-is: an `a || b` fallback can turn
/// a failing test green (`./test || ctest` passes when CTest has nothing
/// registered). Recognised when the command without its fallbacks is a test run.
pub(crate) fn test_run_behind_fallback(call: &ToolCall) -> bool {
    if call.name != "shell" {
        return false;
    }
    let command = crate::agent::tools::shell::shell_command_arg(&call.args).unwrap_or("");
    let Some((primary, _)) = command.split_once("||") else {
        return false;
    };
    let primary: String = primary
        .chars()
        .filter(|ch| !matches!(ch, '(' | ')' | '{' | '}'))
        .collect();
    let args = serde_json::json!({"command": primary.trim()});
    let probe = ToolCall {
        id: String::new(),
        name: "shell".into(),
        args: args.clone(),
    };
    is_verification_call(&probe) || is_progress_verifier_call("shell", &args)
}

/// Output-cap notes per hop before retries fall back to plain re-sends.
const OUTPUT_CAP_NUDGE_LIMIT: usize = 3;

pub(crate) const OUTPUT_CAP_NUDGE: &str = "Your last reply was cut off at the output token limit \
before it finished, so nothing in it ran. Usually one tool call carried too much text. Split the \
work: write a large file in parts (create it with the first part, then add the rest with further \
edits), keep each tool call well under the limit, and do not restate large content.";

/// A provider error meaning the reply hit its output-token cap: re-sending the
/// same request hits the same cap (polyglot-v1 rust-decimal, DeepSeek: 25
/// identical 8192-token cut-offs until the task wall).
pub(crate) fn is_output_cap_truncation(error: &str) -> bool {
    error.starts_with("response incomplete:")
        || error == crate::agent::club::TRUNCATED_OUTPUT_ERR
        || error.contains("finish_reason=length")
}

/// Completions denied for a green that did not hold before one is accepted.
const CONFIRM_GREEN_REJECTION_LIMIT: usize = 2;

pub(crate) const CONFIRM_GREEN_NUDGE: &str = "Your last passing test run did not hold: angelX re-ran it on \
the same code and it failed. The solution passes by luck; something depends on randomness, \
timing, iteration order or state shared between tests or runs. Find that and fix it so the \
tests pass every run. Re-running until green is not a fix.";

/// Extra runs of the model's last passing test before "done" is accepted:
/// `ANGEL_CONFIRM_GREEN_RUNS` (0-5), off unless set. It repeats only what the
/// model already chose to run, as an in-turn check when the evaluator's own
/// acceptance command is withheld. Opt-in on the evidence: a two-seed polyglot-v1
/// A/B on DeepSeek V4.1 Flash (272 tasks per arm) solved 267 with it at 2 against
/// 269 without, for 21% more agent time; 255 greens re-run, one flaky pass
/// caught (cpp-robot-name).
pub(crate) fn confirm_green_extra_runs(_competition: bool) -> usize {
    std::env::var("ANGEL_CONFIRM_GREEN_RUNS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .map_or(0, |runs| runs.min(5))
}

/// Re-run the model's last passing test call up to `extra` more times on the
/// unchanged workspace, judged exactly as the turn judges any tool result.
/// `Ok(runs)` when every run passed; `Err((run, output))` at the first that did
/// not. Runs stop once they have taken `ANGEL_CONFIRM_GREEN_SECS` (default 60),
/// so a slow suite is not repeated at length. polyglot-v1 cpp-robot-name
/// passed about one run in four; a single green run was accepted and the
/// grader's run failed.
pub(crate) fn confirm_green_run(
    registry: &ToolRegistry,
    call: &ToolCall,
    extra: usize,
    cancel: &AtomicBool,
) -> Result<usize, (usize, String)> {
    let budget = Duration::from_secs(env_usize("ANGEL_CONFIRM_GREEN_SECS", 60) as u64);
    let started = Instant::now();
    let mut runs = 0;
    while runs < extra && started.elapsed() < budget && !cancel.load(Ordering::Relaxed) {
        runs += 1;
        let output = match registry.dispatch_with_cancel(&call.name, &call.args, Some(cancel)) {
            Ok(output) => output,
            Err(error) => format!("tool error: {error}"),
        };
        let executed =
            turn_event_outcome(call, &output, false).execution == ExecutionOutcome::Succeeded;
        if !executed || verification_outcome(call, &output) == Some(VerificationOutcome::Failed) {
            return Err((runs, output));
        }
    }
    Ok(runs)
}

fn green_run_label(call: &ToolCall) -> String {
    let detail = match call.name.as_str() {
        "shell" => crate::agent::tools::shell::shell_command_arg(&call.args)
            .unwrap_or("")
            .to_string(),
        "cargo" => call
            .args
            .get("args")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    };
    if detail.is_empty() {
        format!("`{}`", call.name)
    } else {
        format!(
            "`{}: {}`",
            call.name,
            detail.chars().take(120).collect::<String>()
        )
    }
}

fn tail_chars(text: &str, limit: usize) -> String {
    let text = text.trim_end();
    let skip = text.chars().count().saturating_sub(limit);
    text.chars().skip(skip).collect()
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
/// Last operator/user task text for advisor prompts.
fn last_user_task(history: &[ChatMsg]) -> String {
    history
        .iter()
        .rev()
        .find(|m| m.role == ChatRole::User)
        .map(|m| m.content.to_string())
        .unwrap_or_default()
}

/// Compact summary of the most recent tool hop (assistant calls + tool results).
fn last_hop_summary(history: &[ChatMsg]) -> String {
    // Walk backward to the last assistant_calls, then collect following tools.
    let mut start = None;
    for (i, m) in history.iter().enumerate().rev() {
        if m.role == ChatRole::Assistant && m.content.contains("tool_call") {
            start = Some(i);
            break;
        }
        // Also match structured tool-call assistant rows (empty content, has tool_calls).
        if m.role == ChatRole::Assistant && !m.tool_calls.is_empty() {
            start = Some(i);
            break;
        }
    }
    let Some(start) = start else {
        return "(no tool hop)".into();
    };
    let mut lines = Vec::new();
    for m in &history[start..] {
        match m.role {
            ChatRole::Assistant => {
                if !m.tool_calls.is_empty() {
                    for c in m.tool_calls.iter() {
                        lines.push(format!("call {}({})", c.name, truncate_args(&c.args, 120)));
                    }
                } else if !m.content.is_empty() {
                    lines.push(format!("assistant: {}", bound_line(&m.content, 160)));
                }
            }
            ChatRole::Tool => {
                let head = m.content.lines().next().unwrap_or("").trim();
                let status = if is_error_result(&m.content) {
                    "ERR"
                } else {
                    "ok"
                };
                lines.push(format!("result[{status}]: {}", bound_line(head, 200)));
            }
            _ => {}
        }
        if lines.len() >= 24 {
            break;
        }
    }
    if lines.is_empty() {
        "(empty hop)".into()
    } else {
        lines.join("\n")
    }
}

fn bound_line(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        t.to_string()
    } else {
        let mut o: String = t.chars().take(max.saturating_sub(1)).collect();
        o.push('…');
        o
    }
}

fn truncate_args(args: &serde_json::Value, max: usize) -> String {
    let s = args.to_string();
    bound_line(&s, max)
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
    let prompt = format!(
        "{}\n\n{}",
        crate::agent::advisor::ADVISOR_SYS,
        crate::agent::advisor::review_prompt(&task, &answer)
    );
    registry.auxiliary.utility_entered("advisor");
    match club.respond(&prompt) {
        Ok(reply) => {
            match crate::agent::advisor::annotate(&crate::agent::advisor::parse_verdict(&reply)) {
                Some(a) => format!("{answer}{a}"),
                None => answer,
            }
        }
        Err(_) => answer,
    }
}

/// Mid-turn hop advisor; returns a harness note or None on CLEAR/failure.
fn run_hop_advisor(
    club: &dyn Club,
    registry: &ToolRegistry,
    history: &[ChatMsg],
) -> Option<String> {
    let task = last_user_task(history);
    let summary = crate::agent::advisor::bound_hop_summary(&last_hop_summary(history), 4000);
    let prompt = format!(
        "{}\n\n{}",
        crate::agent::advisor::ADVISOR_HOP_SYS,
        crate::agent::advisor::hop_review_prompt(&task, &summary)
    );
    registry.auxiliary.utility_entered("advisor");
    let reply = club.respond(&prompt).ok()?;
    crate::agent::advisor::annotate_hop(&crate::agent::advisor::parse_verdict(&reply))
}

///
/// Only a *failure* is spoken back into the turn: a passing verify would spend
/// context to say nothing. The verdict is recorded either way, so the manifest
/// carries a machine label on every write — the label density the whole plan
/// turns on — while the turn only pays tokens when angel actually broke the
/// build. This is also where the manifest row gets the facts only the loop knows
/// (the hop, the resolved model).
///
/// The same verdict is folded into `verdicts`, which is what the turn's
/// trajectory row is finally *rewarded* on ([`crate::knowledge::cut::TurnVerdicts`]). One
/// verify, three consumers: the model reads it, the manifest records it, and the
/// forge trains on it.
struct PostWriteObservers<'a> {
    diagnostic_counters: &'a PostEditDiagnosticCounters,
    verdicts: &'a mut crate::knowledge::cut::TurnVerdicts,
    rollout_recorder: &'a mut RolloutRecorder,
    verification: &'a mut crate::knowledge::cut::PostWriteVerification,
}

fn post_write_verdict(
    registry: &ToolRegistry,
    club: &dyn Club,
    call: &ToolCall,
    hop: usize,
    result: String,
    observers: &mut PostWriteObservers<'_>,
    cancel: &AtomicBool,
) -> String {
    if registry.external_evaluator_only {
        return result;
    }
    // A failed or denied call authored nothing — there is nothing to check and
    // nothing to record.
    if is_error_result(&result) || result.starts_with("action capsule denied") {
        return result;
    }
    let targets = crate::knowledge::cut::mutation_targets(&call.name, &call.args);
    if targets.is_empty() {
        return result; // not a mutation tool
    }
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
    if notes.is_empty() {
        result
    } else {
        format!("{result}\n\n{}", notes.join("\n\n"))
    }
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
    let diagnostic = cap_post_edit_diagnostics(
        format!(
            "[post-edit LSP — fix these before continuing]\n{}",
            blocks.join("\n\n")
        ),
        max_bytes,
    );
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
