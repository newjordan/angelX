//! Agent graphs — graph engineering as a first-class cockpit primitive.
//!
//! A graph declares **nodes** (specialized agents: persona + tool grant + club),
//! **edges** (`depends_on` routing, fan-out/fan-in for free, and a `gate` that
//! can reject and loop back), **role pools** (bounded logical task leases), and
//! **shared state** (typed upstream result receipts flowing into downstream
//! prompts). Where `spawn` is ad-hoc parallel seats and the
//! swarm is one fixed MoA pipeline, a graph is a *declared org chart*: the
//! researcher/writer/reviewer split with a retry edge, or a council fan-in —
//! authored as TOML, executed with real parallelism, watched live.
//!
//! Execution is ready-set dispatch: a node launches the moment every dependency
//! is done, bounded by its optional role pool, `ANGEL_GRAPH_MAX_SEATS` locally,
//! and the process-wide spawn admission control globally. Each node is a full
//! `run_turn` seat with a Grant-scoped registry (the spawn shape — no MCP/LSP,
//! microsecond startup). No tokio: named threads + mpsc, the house concurrency
//! model.
//!
//! Specs live in bundled `cockpit/graphs/` and user `~/.angel0/graphs` (user
//! wins on name collision). Every gate speaks: parse failures surface in
//! `/graph list`, capacity/deadline/club failures name their cause.
//! Finished runs also emit a native, digest-bound multi-agent episode receipt.
//! Gate outcomes remain control-flow evidence only: this module never promotes
//! a model verdict or substring predicate into training reward.

use super::*;
use crate::agent::club::{OutputBudgetPolicy, OutputBudgetSource, RouteIdentity, RouteMetadata};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::sync::Mutex;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

const DEP_CONTEXT_CAP: usize = 12_000;
const SNAPSHOT_ANSWER_CAP: usize = 1_200;
const EVENT_CAP: usize = 64;
const GRAPH_FAILURE_REASON_CAP: usize = 1_200;
const GRAPH_TRACE_STOP_REASON_CAP: usize = 320;
const GRAPH_EVENT_TEXT_CAP: usize = 640;
const GRAPH_DEADLINE_DEFAULT_SECS: usize = 0;
const GRAPH_MAX_SEATS_DEFAULT: usize = 3;
/// Grace for in-flight seats to land after a failure/cancel before the run
/// stops waiting on them (they wind down at their next hop boundary).
const WIND_DOWN_GRACE: Duration = Duration::from_secs(10);

const GRAPH_EPISODE_SCHEMA: &str = "angel-agent-graph-episode/v1";
const GRAPH_REWARD_BINDING_SCHEMA: &str = "angel-agent-graph-reward-binding/v1";
/// Serialized reward payloads are bounded so an external scorer cannot smuggle
/// bulk into the audited receipts (a reward is a number plus small evidence).
const MAX_GRAPH_REWARD_BYTES: usize = 16 * 1024;
const MAX_GRAPH_REWARD_SOURCE_BYTES: usize = 256;
const MAX_GRAPH_REWARD_CONTRACT_BYTES: usize = 128;
const GRAPH_TRACE_SCHEMA: &str = "angel-agent-graph-trace/v1";
const GRAPH_GATE_CONTROL_CONTRACT: &str = "angel-agent-graph-gate-control/v1";
static GRAPH_EPISODE_SEQ: AtomicU64 = AtomicU64::new(1);

fn is_zero_usize(value: &usize) -> bool {
    *value == 0
}

/// Backoff between a graph seat's provider retries, sliced so a cancel lands
/// within one slice instead of after the whole wait. Same exponential ladder as
/// before (`backoff × 2^attempt`, saturating); only cancellability changed.
fn wait_graph_backoff(cancel: &AtomicBool, backoff_ms: usize, attempt: u32) {
    let total = Duration::from_millis(backoff_ms.saturating_mul(1usize << attempt.min(8)) as u64);
    let slice = Duration::from_millis(50);
    let mut waited = Duration::ZERO;
    while waited < total && !cancel.load(Ordering::Relaxed) {
        std::thread::sleep(slice.min(total - waited));
        waited += slice;
    }
}

// ---------------------------------------------------------------------------
// RL/eval episode receipt
// ---------------------------------------------------------------------------

/// One exact text identity in a graph trace. Receipts are digest-only by
/// default: semantic bodies and previews belong in a separately authorized,
/// audited capture pipeline rather than this orchestration ledger.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GraphTextArtifactV1 {
    pub schema: String,
    pub sha256: String,
    pub utf8_bytes: u64,
    pub chars: u64,
    pub preview: Option<String>,
    pub preview_truncated: bool,
    pub secret_rejected: bool,
}

impl GraphTextArtifactV1 {
    fn capture(text: &str) -> Self {
        let chars = text.chars().count();
        let secret_rejected = contains_likely_graph_secret(text);
        Self {
            schema: "angel-agent-graph-text/v1".to_string(),
            sha256: crate::knowledge::cut::sha256_hex(text.as_bytes()),
            utf8_bytes: text.len() as u64,
            chars: chars as u64,
            preview: None,
            preview_truncated: !text.is_empty(),
            secret_rejected,
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self.schema != "angel-agent-graph-text/v1" || !is_sha256(&self.sha256) {
            return Err("invalid graph trace text identity".to_string());
        }
        if self.preview.is_some() {
            return Err("graph trace text receipts must remain digest-only".to_string());
        }
        if self.preview_truncated != (self.utf8_bytes > 0) {
            return Err("graph trace text omission marker is inconsistent".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GraphRouteConfigV1 {
    pub schema: String,
    pub config_sha256: String,
    pub route: RouteIdentity,
    pub output_budget_policy: String,
    pub max_output_tokens: Option<u32>,
    pub output_budget_source: Option<String>,
    pub output_budget_provenance_sha256: Option<String>,
    pub sampling_policy: String,
}

impl GraphRouteConfigV1 {
    fn new(route: RouteIdentity, metadata: &RouteMetadata) -> Self {
        let (output_budget_policy, max_output_tokens, output_budget_source) =
            match metadata.output_budget {
                OutputBudgetPolicy::ProviderNative => ("provider_native".to_string(), None, None),
                OutputBudgetPolicy::Explicit { tokens, source } => (
                    "explicit".to_string(),
                    Some(tokens),
                    Some(
                        match source {
                            OutputBudgetSource::PerClubEnv => "per_club_env",
                            OutputBudgetSource::GlobalEnv => "global_env",
                        }
                        .to_string(),
                    ),
                ),
                OutputBudgetPolicy::EndpointManaged => ("endpoint_managed".to_string(), None, None),
            };
        let mut config = Self {
            schema: "angel-agent-graph-route-config/v1".to_string(),
            config_sha256: String::new(),
            route,
            output_budget_policy,
            max_output_tokens,
            output_budget_source,
            output_budget_provenance_sha256: metadata
                .output_budget_provenance
                .as_deref()
                .map(|value| crate::knowledge::cut::sha256_hex(value.as_bytes())),
            sampling_policy: "provider_or_route_default".to_string(),
        };
        config.config_sha256 = config
            .canonical_sha256()
            .expect("graph route config JSON is serializable");
        config
    }

    fn canonical_sha256(&self) -> Result<String, String> {
        let mut canonical = self.clone();
        canonical.config_sha256.clear();
        serde_json::to_vec(&canonical)
            .map(|body| crate::knowledge::cut::sha256_hex(&body))
            .map_err(|error| format!("encode graph route config: {error}"))
    }

    fn validate(&self) -> Result<(), String> {
        let budget_shape = match self.output_budget_policy.as_str() {
            "provider_native" | "endpoint_managed" => {
                self.max_output_tokens.is_none() && self.output_budget_source.is_none()
            }
            "explicit" => {
                self.max_output_tokens.is_some_and(|tokens| tokens > 0)
                    && matches!(
                        self.output_budget_source.as_deref(),
                        Some("per_club_env" | "global_env")
                    )
            }
            _ => false,
        };
        if self.schema != "angel-agent-graph-route-config/v1"
            || self.route.driver.trim().is_empty()
            || !budget_shape
            || self.sampling_policy != "provider_or_route_default"
            || !is_sha256(&self.config_sha256)
            || self
                .output_budget_provenance_sha256
                .as_deref()
                .is_some_and(|digest| !is_sha256(digest))
            || self.config_sha256 != self.canonical_sha256()?
        {
            return Err("invalid graph route config".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GraphTraceTerminationKind {
    Answer,
    Interrupt,
    Deadline,
    MaxHops,
    PolicyGuard,
    BudgetExhausted,
    ProviderFailure,
    CaptureFailure,
    SpawnFailure,
    Abandoned,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GraphTraceTerminationV1 {
    pub kind: GraphTraceTerminationKind,
    pub stop_reason: String,
    pub interrupted: bool,
    pub deadline_reached: bool,
    pub max_hops_reached: bool,
    pub infrastructure_failure: bool,
    pub detail_sha256: Option<String>,
}

impl GraphTraceTerminationV1 {
    fn answer() -> Self {
        Self {
            kind: GraphTraceTerminationKind::Answer,
            stop_reason: "answer".to_string(),
            interrupted: false,
            deadline_reached: false,
            max_hops_reached: false,
            infrastructure_failure: false,
            detail_sha256: None,
        }
    }

    fn failure(kind: GraphTraceTerminationKind, reason: &str) -> Self {
        Self {
            kind,
            // Keep the causal digest over the complete provider/harness error,
            // but never turn the durable receipt into an unbounded error-body
            // side channel.
            stop_reason: head_chars(reason, GRAPH_TRACE_STOP_REASON_CAP),
            interrupted: matches!(
                kind,
                GraphTraceTerminationKind::Interrupt | GraphTraceTerminationKind::Abandoned
            ),
            deadline_reached: matches!(kind, GraphTraceTerminationKind::Deadline),
            max_hops_reached: matches!(kind, GraphTraceTerminationKind::MaxHops),
            infrastructure_failure: matches!(
                kind,
                GraphTraceTerminationKind::ProviderFailure
                    | GraphTraceTerminationKind::CaptureFailure
                    | GraphTraceTerminationKind::SpawnFailure
                    | GraphTraceTerminationKind::Abandoned
            ),
            detail_sha256: Some(crate::knowledge::cut::sha256_hex(reason.as_bytes())),
        }
    }

    fn from_turn(
        stop_reason: TurnStopReason,
        interrupted: bool,
        deadline_reached: bool,
        max_hops_reached: bool,
        detail: Option<&str>,
    ) -> Self {
        let kind = match stop_reason {
            TurnStopReason::Answer => GraphTraceTerminationKind::Answer,
            TurnStopReason::Interrupt => GraphTraceTerminationKind::Interrupt,
            TurnStopReason::Deadline | TurnStopReason::IdleTimeout => {
                GraphTraceTerminationKind::Deadline
            }
            TurnStopReason::MaxHops => GraphTraceTerminationKind::MaxHops,
            TurnStopReason::ProviderError => GraphTraceTerminationKind::ProviderFailure,
            TurnStopReason::CaptureFailure | TurnStopReason::CheckpointFailure => {
                GraphTraceTerminationKind::CaptureFailure
            }
            TurnStopReason::DeferredStop
            | TurnStopReason::Spin
            | TurnStopReason::ErrorStop
            | TurnStopReason::ExecutionBlocked
            | TurnStopReason::NeedsPro
            | TurnStopReason::EscalatedUnproductive
            | TurnStopReason::AcceptanceStop => GraphTraceTerminationKind::PolicyGuard,
        };
        Self {
            kind,
            stop_reason: stop_reason.as_str().to_string(),
            interrupted,
            deadline_reached,
            max_hops_reached,
            infrastructure_failure: matches!(
                kind,
                GraphTraceTerminationKind::ProviderFailure
                    | GraphTraceTerminationKind::CaptureFailure
            ),
            detail_sha256: detail.map(|value| crate::knowledge::cut::sha256_hex(value.as_bytes())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GraphControlCreditV1 {
    pub contract: String,
    pub evaluator_role: String,
    pub evaluator_trace_id: String,
    pub evaluator_output_sha256: String,
    pub credit_distance: u32,
    pub outcome: String,
}

/// Deliberately not a reward. Graph gates are model/self-reported verdicts or
/// deterministic routing predicates, never external evaluator truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GraphTraceEligibilityV1 {
    pub training_eligible: bool,
    pub reasons: Vec<String>,
}

/// UTC Unix milliseconds paired with elapsed monotonic nanoseconds from episode start.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GraphTimestamp {
    pub utc_ms: u64,
    pub monotonic_ns: u128,
}

impl GraphTimestamp {
    fn now(origin: Instant) -> Self {
        Self {
            utc_ms: unix_ms(),
            monotonic_ns: origin.elapsed().as_nanos(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GraphTraceV1 {
    pub schema: String,
    pub trace_id: String,
    pub role: String,
    pub persona: Option<String>,
    pub trainable: bool,
    pub requested_route: RouteIdentity,
    pub resolved_route: RouteIdentity,
    pub requested_route_config: GraphRouteConfigV1,
    pub resolved_route_config: GraphRouteConfigV1,
    pub grant: String,
    pub attempt_index: u32,
    pub parent_trace_ids: Vec<String>,
    pub retry_of_trace_id: Option<String>,
    pub feedback_sha256: Option<String>,
    pub system: GraphTextArtifactV1,
    pub prompt: GraphTextArtifactV1,
    pub output: Option<GraphTextArtifactV1>,
    pub termination: GraphTraceTerminationV1,
    pub hops: usize,
    pub elapsed_ms: u128,
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub stall_retries: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall_remaining_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<GraphTimestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<GraphTimestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs_ready_at: Option<GraphTimestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancel_requested_at: Option<GraphTimestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopped_at: Option<GraphTimestamp>,
    pub rollout_id: Option<String>,
    pub control_credit: Vec<GraphControlCreditV1>,
    /// Always null in this receipt schema. An external scorer binds a separate
    /// reward receipt to `episode_id`, `trace_id`, and their content digests.
    pub reward: Option<serde_json::Value>,
    pub reward_source: Option<String>,
    pub eligibility: GraphTraceEligibilityV1,
}

impl GraphTraceV1 {
    fn decide_eligibility(&self) -> GraphTraceEligibilityV1 {
        let mut reasons = Vec::new();
        if !self.trainable {
            reasons.push("frozen_role".to_string());
        }
        if self.termination.kind != GraphTraceTerminationKind::Answer {
            reasons.push("non_answer_termination".to_string());
        }
        reasons.push("external_reward_missing".to_string());
        reasons.push("semantic_bodies_not_captured".to_string());
        let artifacts = [Some(&self.system), Some(&self.prompt), self.output.as_ref()];
        if artifacts
            .iter()
            .flatten()
            .any(|artifact| artifact.secret_rejected)
        {
            reasons.push("secret_detected".to_string());
        }
        if self.output.is_none() {
            reasons.push("missing_output".to_string());
        }
        reasons.sort();
        reasons.dedup();
        GraphTraceEligibilityV1 {
            training_eligible: false,
            reasons,
        }
    }
}

/// An external scorer's audited reward binding for one trace of a sealed
/// episode. The episode receipt itself stays reward-free forever; this receipt
/// is the only path that may attach training reward, and it is digest-tight:
/// the episode's sealed receipt SHA and the trace's output SHA are recorded
/// and re-verified before anything is written.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GraphRewardBindingV1 {
    pub schema: String,
    pub episode_id: String,
    pub episode_receipt_sha256: String,
    pub trace_id: String,
    pub trace_output_sha256: String,
    pub reward: serde_json::Value,
    pub reward_source: String,
    pub reward_contract: String,
    pub bound_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GraphEpisodeTerminationV1 {
    pub kind: GraphEpisodeTerminationKind,
    pub phase: GraphRunPhase,
    pub detail_sha256: Option<String>,
    pub final_output_sha256: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GraphEpisodeTerminationKind {
    Completed,
    OperatorCancelled,
    Deadline,
    Failed,
    Incomplete,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GraphRuntimeConfigV1 {
    pub schema: String,
    pub config_sha256: String,
    pub runner_version: String,
    pub cockpit_source_sha256: String,
    pub graph_deadline_secs: u64,
    pub graph_max_seats: usize,
    pub spawn_inflight_limit: usize,
    /// Zero is the portable spelling for explicitly unbounded.
    pub node_max_hops: usize,
    pub node_turn_deadline_secs: usize,
    pub competition_turn_deadline_secs: usize,
    /// Total dependency-result body characters admitted to one node prompt.
    pub dependency_context_chars: usize,
    pub wind_down_grace_secs: u64,
    pub provider_retries: usize,
    pub provider_retry_backoff_ms: usize,
    pub rollout_capture: String,
    pub rollout_required: bool,
    pub unrestricted: bool,
    pub harness_treatment_sha256: String,
    pub reward_policy: String,
}

impl GraphRuntimeConfigV1 {
    #[allow(clippy::too_many_arguments)]
    fn new(
        graph_deadline_secs: u64,
        graph_max_seats: usize,
        spawn_inflight_limit: usize,
        node_max_hops: usize,
        dependency_context_chars: usize,
    ) -> Result<Self, String> {
        let runner_version = env!("CARGO_PKG_VERSION").to_string();
        let cockpit_source_sha256 =
            crate::agent::harness::run_identity::source_sha256().to_string();
        let node_max_hops = if node_max_hops == usize::MAX {
            0
        } else {
            node_max_hops
        };
        let rollout_capture = match std::env::var("ANGEL_HARNESS_ROLLOUTS")
            .ok()
            .map(|value| value.trim().to_ascii_lowercase())
            .as_deref()
        {
            Some("local") => "local",
            Some("shadow") => "shadow",
            _ => "off",
        }
        .to_string();
        let harness_treatment_sha256 = crate::knowledge::cut::sha256_hex(
            &serde_json::to_vec(&harness_treatment_json())
                .map_err(|error| format!("encode graph harness treatment: {error}"))?,
        );
        let mut config = Self {
            schema: "angel-agent-graph-runtime/v1".to_string(),
            config_sha256: String::new(),
            runner_version,
            cockpit_source_sha256,
            graph_deadline_secs,
            graph_max_seats,
            spawn_inflight_limit,
            node_max_hops,
            node_turn_deadline_secs: configured_turn_deadline_secs(),
            competition_turn_deadline_secs: env_usize("ANGEL_COMPETITION_TURN_DEADLINE_SECS", 0),
            dependency_context_chars,
            wind_down_grace_secs: WIND_DOWN_GRACE.as_secs(),
            // `usize::MAX` is this receipt's spelling for the unbounded L01
            // budget (env unset); an explicit count, `0` included, is recorded
            // verbatim.
            provider_retries: provider_retry_budget().unwrap_or(usize::MAX),
            provider_retry_backoff_ms: env_usize("ANGEL_PROVIDER_RETRY_BACKOFF_MS", 500),
            rollout_capture,
            rollout_required: env_flag("ANGEL_HARNESS_ROLLOUT_REQUIRED", false),
            unrestricted: crate::platform::yolo::enabled(),
            harness_treatment_sha256,
            reward_policy: "external_only".to_string(),
        };
        config.config_sha256 = config.canonical_sha256()?;
        config.validate()?;
        Ok(config)
    }

    fn canonical_sha256(&self) -> Result<String, String> {
        let mut canonical = self.clone();
        canonical.config_sha256.clear();
        serde_json::to_vec(&canonical)
            .map(|body| crate::knowledge::cut::sha256_hex(&body))
            .map_err(|error| format!("encode graph runtime config: {error}"))
    }

    fn validate(&self) -> Result<(), String> {
        if self.schema != "angel-agent-graph-runtime/v1"
            || self.runner_version.is_empty()
            || (self.cockpit_source_sha256 != "unbound" && !is_sha256(&self.cockpit_source_sha256))
            || !is_sha256(&self.harness_treatment_sha256)
            || !is_sha256(&self.config_sha256)
            || self.reward_policy != "external_only"
            || self.graph_max_seats == 0
            || self.spawn_inflight_limit < self.graph_max_seats
            || self.dependency_context_chars < 1_000
        {
            return Err("invalid graph runtime config".to_string());
        }
        if self.config_sha256 != self.canonical_sha256()? {
            return Err("graph runtime config digest mismatch".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GraphEpisodeV1 {
    pub schema: String,
    pub episode_id: String,
    pub graph_name: String,
    pub graph_spec_sha256: String,
    pub task_sha256: String,
    pub workspace_key_sha256: String,
    pub workspace_state_sha256: Option<String>,
    pub workspace_state_source: String,
    pub runner_version: String,
    pub cockpit_source_sha256: String,
    pub runtime: GraphRuntimeConfigV1,
    pub started_ms: u64,
    pub sealed_ms: u64,
    pub traces: Vec<GraphTraceV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_allocation: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub incomplete_by_budget: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incomplete_reason: Option<String>,
    #[serde(default)]
    pub fanin: GraphFaninTruth,
    pub termination: GraphEpisodeTerminationV1,
    pub eligible_trace_count: usize,
    pub receipt_sha256: String,
}

impl GraphEpisodeV1 {
    fn seal(mut self) -> Result<Self, String> {
        self.eligible_trace_count = self
            .traces
            .iter()
            .filter(|trace| trace.eligibility.training_eligible)
            .count();
        self.receipt_sha256 = self.canonical_sha256()?;
        self.validate()?;
        Ok(self)
    }

    fn canonical_sha256(&self) -> Result<String, String> {
        let mut canonical = self.clone();
        canonical.receipt_sha256.clear();
        serde_json::to_vec(&canonical)
            .map(|body| crate::knowledge::cut::sha256_hex(&body))
            .map_err(|error| format!("encode graph episode receipt: {error}"))
    }

    fn validate(&self) -> Result<(), String> {
        if self.schema != GRAPH_EPISODE_SCHEMA
            || !is_sha256(&self.episode_id)
            || !is_sha256(&self.graph_spec_sha256)
            || !is_sha256(&self.task_sha256)
            || !is_sha256(&self.workspace_key_sha256)
            || self
                .workspace_state_sha256
                .as_deref()
                .is_some_and(|digest| !is_sha256(digest))
            || !is_sha256(&self.receipt_sha256)
        {
            return Err("invalid graph episode identity".to_string());
        }
        let workspace_state_matches = matches!(
            (
                self.workspace_state_source.as_str(),
                self.workspace_state_sha256.is_some()
            ),
            ("git-workspace-evidence/v1", true) | ("unavailable", false)
        );
        if !workspace_state_matches {
            return Err("graph episode workspace state provenance is inconsistent".to_string());
        }
        self.runtime.validate()?;
        if self.runner_version != self.runtime.runner_version
            || self.cockpit_source_sha256 != self.runtime.cockpit_source_sha256
        {
            return Err("graph episode provenance does not match runtime config".to_string());
        }
        if self.sealed_ms < self.started_ms || self.receipt_sha256 != self.canonical_sha256()? {
            return Err("graph episode receipt digest mismatch".to_string());
        }
        let termination_matches = matches!(
            (self.termination.kind, self.termination.phase),
            (GraphEpisodeTerminationKind::Completed, GraphRunPhase::Done)
                | (
                    GraphEpisodeTerminationKind::OperatorCancelled,
                    GraphRunPhase::Cancelled
                )
                | (
                    GraphEpisodeTerminationKind::Deadline
                        | GraphEpisodeTerminationKind::Failed
                        | GraphEpisodeTerminationKind::Incomplete,
                    GraphRunPhase::Failed
                )
        );
        if !termination_matches {
            return Err("graph episode termination kind/phase mismatch".to_string());
        }
        let trace_ids: HashSet<&str> = self
            .traces
            .iter()
            .map(|trace| trace.trace_id.as_str())
            .collect();
        if trace_ids.len() != self.traces.len() {
            return Err("graph episode contains duplicate trace ids".to_string());
        }
        let mut next_attempt: BTreeMap<&str, u32> = BTreeMap::new();
        let mut prior_trace_ids = HashSet::new();
        for trace in &self.traces {
            if trace.schema != GRAPH_TRACE_SCHEMA || !is_sha256(&trace.trace_id) {
                return Err("invalid graph trace identity".to_string());
            }
            trace.requested_route_config.validate()?;
            trace.resolved_route_config.validate()?;
            if trace.requested_route_config.route != trace.requested_route
                || trace.resolved_route_config.route != trace.resolved_route
            {
                return Err("graph trace route config does not match route identity".to_string());
            }
            if trace
                .feedback_sha256
                .as_deref()
                .is_some_and(|digest| !is_sha256(digest))
                || trace
                    .termination
                    .detail_sha256
                    .as_deref()
                    .is_some_and(|digest| !is_sha256(digest))
            {
                return Err("graph trace contains an invalid evidence digest".to_string());
            }
            if trace.trace_id != graph_trace_id(&self.episode_id, &trace.role, trace.attempt_index)
            {
                return Err(
                    "graph trace id does not bind its episode, role, and attempt".to_string(),
                );
            }
            let expected_attempt = next_attempt.entry(trace.role.as_str()).or_insert(0);
            if trace.attempt_index != *expected_attempt {
                return Err("graph trace attempt order is not contiguous".to_string());
            }
            *expected_attempt = expected_attempt.saturating_add(1);
            trace.system.validate()?;
            trace.prompt.validate()?;
            if let Some(output) = trace.output.as_ref() {
                output.validate()?;
            }
            if trace.eligibility != trace.decide_eligibility() {
                return Err(format!(
                    "graph trace '{}' eligibility does not match its evidence",
                    trace.trace_id
                ));
            }
            for parent in &trace.parent_trace_ids {
                if !prior_trace_ids.contains(parent.as_str()) {
                    return Err("graph trace parent did not complete before the child".to_string());
                }
            }
            if let Some(retry_of) = trace.retry_of_trace_id.as_deref()
                && !prior_trace_ids.contains(retry_of)
            {
                return Err("graph trace retry parent did not complete first".to_string());
            }
            if trace.reward.is_some() || trace.reward_source.is_some() {
                return Err("graph episode receipts cannot own reward".to_string());
            }
            for signal in &trace.control_credit {
                if signal.contract != GRAPH_GATE_CONTROL_CONTRACT
                    || !matches!(signal.outcome.as_str(), "accepted" | "rejected")
                    || signal.credit_distance == 0
                    || !is_sha256(&signal.evaluator_trace_id)
                    || !is_sha256(&signal.evaluator_output_sha256)
                    || !trace_ids.contains(signal.evaluator_trace_id.as_str())
                {
                    return Err("invalid graph gate control signal".to_string());
                }
                let evaluator = self
                    .traces
                    .iter()
                    .find(|candidate| candidate.trace_id == signal.evaluator_trace_id)
                    .expect("trace id membership checked above");
                if evaluator.role != signal.evaluator_role
                    || evaluator
                        .output
                        .as_ref()
                        .map(|output| output.sha256.as_str())
                        != Some(signal.evaluator_output_sha256.as_str())
                {
                    return Err(
                        "graph gate control signal does not match evaluator evidence".to_string(),
                    );
                }
            }
            prior_trace_ids.insert(trace.trace_id.as_str());
        }
        let eligible = self
            .traces
            .iter()
            .filter(|trace| trace.eligibility.training_eligible)
            .count();
        if eligible != self.eligible_trace_count {
            return Err("graph episode eligible trace count mismatch".to_string());
        }
        if self
            .termination
            .detail_sha256
            .as_deref()
            .is_some_and(|digest| !is_sha256(digest))
            || self
                .termination
                .final_output_sha256
                .as_deref()
                .is_some_and(|digest| !is_sha256(digest))
        {
            return Err("graph episode contains an invalid termination digest".to_string());
        }
        Ok(())
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

// ---------------------------------------------------------------------------
// Spec
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GraphSpec {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Whole-run deadline override; else `ANGEL_GRAPH_DEADLINE_SECS`.
    #[serde(default)]
    pub deadline_secs: Option<u64>,
    /// Optional shared allocation for this graph, inherited by every node/retry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_budget: Option<u64>,
    #[serde(rename = "node")]
    pub nodes: Vec<GraphNodeSpec>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GraphNodeSpec {
    pub id: String,
    /// Prompt template. `{task}` = the run task; `{output:<dep>}` inserts that
    /// dependency's typed result envelope plus its bounded body excerpt.
    /// Unreferenced dependency receipts are appended as an "Upstream results"
    /// section automatically.
    pub prompt: String,
    /// Persona from the spawn catalog (`cockpit/personas/` + `~/.angel0/personas`).
    #[serde(default)]
    pub persona: Option<String>,
    /// Club spec: `self`/`auto` (default, the in-hand driver), `smart` (the
    /// designated escalation seat), or an explicit club label.
    #[serde(default)]
    pub club: Option<String>,
    /// Tool grant: none (default) | read_only | code | research.
    #[serde(default)]
    pub tools: Option<String>,
    /// Per-node reasoning-effort request (e.g. "medium"). Applied to the seat's
    /// club at run start; a backend that rejects it says so in the run events.
    #[serde(default)]
    pub effort: Option<String>,
    /// Role policy for downstream RL corpus builders. Frozen is the safe
    /// default; `trainable = true` only expresses intent and never makes a
    /// trace eligible without a separately bound external reward receipt.
    #[serde(default)]
    pub trainable: bool,
    /// Optional reusable role pool. Nodes in one pool must declare the same
    /// persona/club/tool/effort/trainable identity; their prompts and graph
    /// dependencies may differ. Dispatches borrow short-lived logical leases
    /// from the pool rather than creating an unbounded wave of identical roles.
    #[serde(default)]
    pub pool: Option<String>,
    /// Concurrent leases allowed for this pool. A named pool defaults to one;
    /// any explicit declarations for the same pool must agree.
    #[serde(default)]
    pub pool_max_inflight: Option<usize>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Conditional loop-back edge: reject the run state and retry an ancestor.
    #[serde(default)]
    pub gate: Option<GraphGateSpec>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GraphGateSpec {
    /// The node must end with `VERDICT: PASS` or `VERDICT: FAIL — <reason>`
    /// (its system prompt demands exactly that shape).
    #[serde(default)]
    pub verdict: bool,
    /// Deterministic accept: this substring must appear in the node output.
    #[serde(default)]
    pub expect: Option<String>,
    /// On FAIL, reset this ancestor node (and the path back down to the gate)
    /// and re-run with the gate's feedback attached.
    #[serde(default)]
    pub retry: Option<String>,
    /// Attribute this gate's control-flow outcome to an ancestor trace. This
    /// is lineage metadata, not reward; an external evaluator remains the only
    /// authority that may bind training reward.
    #[serde(default)]
    pub credit_to: Option<String>,
    #[serde(default = "default_gate_retries")]
    pub max_retries: usize,
}

fn default_gate_retries() -> usize {
    1
}

/// Graph nodes default to NO tools (a bare reasoning seat); spawn's parse
/// defaults to read_only, which offers tool defs and arms the harness's
/// announce-only heuristics — that behavior stays opt-in via `tools = "…"`.
fn node_grant(node: &GraphNodeSpec) -> Result<Grant, String> {
    match node.tools.as_deref() {
        None => Ok(Grant::None),
        some => Grant::parse(some),
    }
}

pub(crate) fn graph_has_workspace_writes(spec: &GraphSpec) -> Result<bool, String> {
    let mut writable = false;
    for node in &spec.nodes {
        writable |= node_grant(node)? == Grant::Code;
    }
    Ok(writable)
}

const GRAPH_POOL_NAME_CAP: usize = 64;
const GRAPH_POOL_MAX_INFLIGHT: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
struct GraphPoolIdentity {
    persona: String,
    club: String,
    grant: String,
    effort: String,
    trainable: bool,
}

#[derive(Clone, Debug)]
struct GraphPoolDraft {
    first_node: String,
    identity: GraphPoolIdentity,
    explicit_limit: Option<usize>,
}

fn normalized_pool_identity(node: &GraphNodeSpec) -> Result<GraphPoolIdentity, String> {
    let persona = node
        .persona
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let raw_club = node
        .club
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let club = match raw_club.as_str() {
        "" | "self" | "auto" => "self".to_string(),
        "smart" | "sota" => "smart".to_string(),
        _ => raw_club,
    };
    let effort = node
        .effort
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    Ok(GraphPoolIdentity {
        persona,
        club,
        grant: node_grant(node)?.label().to_string(),
        effort,
        trainable: node.trainable,
    })
}

/// Resolve the graph's reusable role pools into deterministic capacity
/// policies. Validation calls this before execution; runtime calls it again so
/// the scheduler never derives policy from unchecked fields.
fn graph_pool_policies(spec: &GraphSpec) -> Result<BTreeMap<String, usize>, String> {
    let mut drafts: BTreeMap<String, GraphPoolDraft> = BTreeMap::new();
    for node in &spec.nodes {
        let Some(raw_pool) = node.pool.as_deref() else {
            if node.pool_max_inflight.is_some() {
                return Err(format!(
                    "graph '{}': node '{}' sets pool_max_inflight without a pool",
                    spec.name, node.id
                ));
            }
            continue;
        };
        let pool = raw_pool.trim();
        if pool.is_empty() {
            return Err(format!(
                "graph '{}': node '{}' has an empty pool name",
                spec.name, node.id
            ));
        }
        if pool.len() > GRAPH_POOL_NAME_CAP
            || !pool
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        {
            return Err(format!(
                "graph '{}': node '{}' pool '{}' must be 1..={} ASCII letters, digits, '_', '-', or '.'",
                spec.name, node.id, pool, GRAPH_POOL_NAME_CAP
            ));
        }
        if let Some(limit) = node.pool_max_inflight
            && !(1..=GRAPH_POOL_MAX_INFLIGHT).contains(&limit)
        {
            return Err(format!(
                "graph '{}': node '{}' pool '{}' max_inflight {} must be 1..={}",
                spec.name, node.id, pool, limit, GRAPH_POOL_MAX_INFLIGHT
            ));
        }
        let identity = normalized_pool_identity(node)
            .map_err(|error| format!("graph '{}': node '{}': {error}", spec.name, node.id))?;
        match drafts.get_mut(pool) {
            Some(draft) => {
                if draft.identity != identity {
                    return Err(format!(
                        "graph '{}': pool '{}' role identity differs between nodes '{}' and '{}' (persona, club, tools, effort, and trainable must match)",
                        spec.name, pool, draft.first_node, node.id
                    ));
                }
                if let (Some(previous), Some(limit)) =
                    (draft.explicit_limit, node.pool_max_inflight)
                {
                    if previous != limit {
                        return Err(format!(
                            "graph '{}': pool '{}' has conflicting max_inflight values {} and {}",
                            spec.name, pool, previous, limit
                        ));
                    }
                } else if draft.explicit_limit.is_none() {
                    draft.explicit_limit = node.pool_max_inflight;
                }
            }
            None => {
                drafts.insert(
                    pool.to_string(),
                    GraphPoolDraft {
                        first_node: node.id.clone(),
                        identity,
                        explicit_limit: node.pool_max_inflight,
                    },
                );
            }
        }
    }
    Ok(drafts
        .into_iter()
        .map(|(name, draft)| (name, draft.explicit_limit.unwrap_or(1)))
        .collect())
}

fn node_pool_name(node: &GraphNodeSpec) -> Option<&str> {
    node.pool
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
}

/// Structural validation. Every rejection names the offending node and rule so
/// a broken spec teaches its own fix.
pub(crate) fn validate_graph_spec(spec: &GraphSpec) -> Result<(), String> {
    if spec.name.trim().is_empty() {
        return Err("graph spec has no name".to_string());
    }
    if spec.nodes.is_empty() {
        return Err(format!(
            "graph '{}' declares no [[node]] entries",
            spec.name
        ));
    }
    let mut index: HashMap<&str, usize> = HashMap::new();
    for (i, node) in spec.nodes.iter().enumerate() {
        if node.id.trim().is_empty() {
            return Err(format!(
                "graph '{}': node {} has an empty id",
                spec.name,
                i + 1
            ));
        }
        if index.insert(node.id.as_str(), i).is_some() {
            return Err(format!(
                "graph '{}': duplicate node id '{}'",
                spec.name, node.id
            ));
        }
    }
    let mut code_nodes = Vec::new();
    for (node_idx, node) in spec.nodes.iter().enumerate() {
        for dep in &node.depends_on {
            if dep == &node.id {
                return Err(format!(
                    "graph '{}': node '{}' depends on itself",
                    spec.name, node.id
                ));
            }
            if !index.contains_key(dep.as_str()) {
                return Err(format!(
                    "graph '{}': node '{}' depends on unknown node '{dep}'",
                    spec.name, node.id
                ));
            }
        }
        let grant = node_grant(node)
            .map_err(|e| format!("graph '{}': node '{}': {e}", spec.name, node.id))?;
        if grant == Grant::Code {
            code_nodes.push(node_idx);
        }
        // {output:X} placeholders may only reference declared dependencies —
        // anything else would read state across a non-edge.
        let mut rest = node.prompt.as_str();
        while let Some(pos) = rest.find("{output:") {
            let tail = &rest[pos + "{output:".len()..];
            let Some(end) = tail.find('}') else {
                return Err(format!(
                    "graph '{}': node '{}': unterminated {{output:…}} placeholder",
                    spec.name, node.id
                ));
            };
            let referenced = &tail[..end];
            if !node.depends_on.iter().any(|d| d == referenced) {
                return Err(format!(
                    "graph '{}': node '{}' references {{output:{referenced}}} but does not depend_on '{referenced}'",
                    spec.name, node.id
                ));
            }
            rest = &tail[end..];
        }
        if let Some(gate) = &node.gate {
            if !gate.verdict && gate.expect.is_none() {
                return Err(format!(
                    "graph '{}': node '{}' gate needs verdict=true or expect=\"…\"",
                    spec.name, node.id
                ));
            }
            if gate.max_retries > 10 {
                return Err(format!(
                    "graph '{}': node '{}' gate max_retries {} exceeds the sanity cap (10)",
                    spec.name, node.id, gate.max_retries
                ));
            }
            for target in gate.retry.iter().chain(gate.credit_to.iter()) {
                if !index.contains_key(target.as_str()) {
                    return Err(format!(
                        "graph '{}': node '{}' gate targets unknown node '{target}'",
                        spec.name, node.id,
                    ));
                }
                let gate_idx = index[node.id.as_str()];
                if !ancestors_of(spec, &index, gate_idx).contains(&index[target.as_str()]) {
                    return Err(format!(
                        "graph '{}': node '{}' gate target '{target}' must be an ancestor of the gate",
                        spec.name, node.id
                    ));
                }
            }
        }
    }
    let _ = graph_pool_policies(spec)?;
    // Kahn cycle check.
    let mut indegree: Vec<usize> = spec
        .nodes
        .iter()
        .map(|node| node.depends_on.len())
        .collect();
    let mut queue: Vec<usize> = indegree
        .iter()
        .enumerate()
        .filter(|(_, d)| **d == 0)
        .map(|(i, _)| i)
        .collect();
    let mut seen = 0usize;
    while let Some(done) = queue.pop() {
        seen += 1;
        let done_id = spec.nodes[done].id.as_str();
        for (i, node) in spec.nodes.iter().enumerate() {
            if node.depends_on.iter().any(|d| d == done_id) {
                indegree[i] -= 1;
                if indegree[i] == 0 {
                    queue.push(i);
                }
            }
        }
    }
    if seen != spec.nodes.len() {
        let stuck: Vec<&str> = indegree
            .iter()
            .enumerate()
            .filter(|(_, d)| **d > 0)
            .map(|(i, _)| spec.nodes[i].id.as_str())
            .collect();
        return Err(format!(
            "graph '{}': dependency cycle through {}",
            spec.name,
            stuck.join(", ")
        ));
    }
    // A retry invalidates the target and all descendants, but generation tags
    // can discard only stale outputs—not workspace writes already made by an
    // in-flight Code node. Every affected writer must therefore be ordered
    // with the gate: before it (the write landed before the verdict) or after
    // it (the writer cannot start until the gate passes). An unordered sibling
    // writer could mutate after the retry and then be rerun, leaking stale
    // side effects into the shared workspace.
    for (gate_idx, gate_node) in spec.nodes.iter().enumerate() {
        let Some(retry_target) = gate_node
            .gate
            .as_ref()
            .and_then(|gate| gate.retry.as_deref())
        else {
            continue;
        };
        let target_idx = index[retry_target];
        let affected = descendants_including(spec, target_idx);
        let gate_ancestors = ancestors_of(spec, &index, gate_idx);
        for &code_idx in &code_nodes {
            if !affected.contains(&code_idx) || code_idx == gate_idx {
                continue;
            }
            let code_ancestors = ancestors_of(spec, &index, code_idx);
            if !gate_ancestors.contains(&code_idx) && !code_ancestors.contains(&gate_idx) {
                return Err(format!(
                    "graph '{}': retry gate '{}' and affected code node '{}' are unordered; add a transitive depends_on edge so the writer lands before the gate or starts only after it passes",
                    spec.name, gate_node.id, spec.nodes[code_idx].id
                ));
            }
        }
    }
    // Code-capable nodes share this graph's one workspace. Declaration order,
    // a common ancestor/descendant, pool capacity, and current seat limits do
    // not establish a durable happens-before edge: only dependency
    // reachability does.
    for (position, &left) in code_nodes.iter().enumerate() {
        let left_ancestors = ancestors_of(spec, &index, left);
        for &right in &code_nodes[position + 1..] {
            if !left_ancestors.contains(&right)
                && !ancestors_of(spec, &index, right).contains(&left)
            {
                return Err(format!(
                    "graph '{}': code nodes '{}' and '{}' are unordered but share one workspace; serialize them via depends_on (directly or transitively), or use a worktree-isolated delegate",
                    spec.name, spec.nodes[left].id, spec.nodes[right].id
                ));
            }
        }
    }
    Ok(())
}

fn ancestors_of(spec: &GraphSpec, index: &HashMap<&str, usize>, node: usize) -> HashSet<usize> {
    let mut out = HashSet::new();
    let mut stack: Vec<usize> = spec.nodes[node]
        .depends_on
        .iter()
        .map(|d| index[d.as_str()])
        .collect();
    while let Some(i) = stack.pop() {
        if out.insert(i) {
            stack.extend(spec.nodes[i].depends_on.iter().map(|d| index[d.as_str()]));
        }
    }
    out
}

fn descendants_including(spec: &GraphSpec, node: usize) -> HashSet<usize> {
    let mut out = HashSet::new();
    let mut stack = vec![node];
    while let Some(parent) = stack.pop() {
        if !out.insert(parent) {
            continue;
        }
        let parent_id = &spec.nodes[parent].id;
        stack.extend(
            spec.nodes
                .iter()
                .enumerate()
                .filter(|(_, candidate)| candidate.depends_on.contains(parent_id))
                .map(|(index, _)| index),
        );
    }
    out
}

// ---------------------------------------------------------------------------
// Catalog
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub(crate) struct LoadedGraph {
    pub name: String,
    pub source: PathBuf,
    pub spec: Option<GraphSpec>,
    /// Parse/validation failure — shown by `/graph list`, never swallowed.
    pub error: Option<String>,
}

fn graphs_bundled_dir() -> PathBuf {
    std::env::var_os("ANGEL_BUNDLED_GRAPHS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::platform::runtime_paths::cockpit_dir().join("graphs"))
}

fn graphs_user_dir() -> PathBuf {
    std::env::var_os("ANGEL_GRAPHS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".angel0/graphs")
        })
}

/// Bundled then user `*.toml`, user winning on graph-name collision.
pub(crate) fn load_graphs() -> Vec<LoadedGraph> {
    let mut by_name: Vec<LoadedGraph> = Vec::new();
    for dir in [graphs_bundled_dir(), graphs_user_dir()] {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("toml"))
            .collect();
        files.sort();
        for path in files {
            let loaded = load_graph_file(&path);
            if let Some(existing) = by_name.iter_mut().find(|g| g.name == loaded.name) {
                *existing = loaded; // later dir (user) wins
            } else {
                by_name.push(loaded);
            }
        }
    }
    by_name
}

fn load_graph_file(path: &Path) -> LoadedGraph {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("graph")
        .to_string();
    let body = match std::fs::read_to_string(path) {
        Ok(body) => body,
        Err(e) => {
            return LoadedGraph {
                name: stem,
                source: path.to_path_buf(),
                spec: None,
                error: Some(format!("unreadable: {e}")),
            };
        }
    };
    match toml::from_str::<GraphSpec>(&body) {
        Ok(spec) => match validate_graph_spec(&spec) {
            Ok(()) => LoadedGraph {
                name: spec.name.clone(),
                source: path.to_path_buf(),
                spec: Some(spec),
                error: None,
            },
            Err(e) => LoadedGraph {
                name: spec.name.clone(),
                source: path.to_path_buf(),
                spec: None,
                error: Some(e),
            },
        },
        Err(e) => LoadedGraph {
            name: stem,
            source: path.to_path_buf(),
            spec: None,
            error: Some(format!("TOML parse: {e}")),
        },
    }
}

// ---------------------------------------------------------------------------
// Live telemetry (lock-and-swap; workers publish, the UI tick cheap-clones)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GraphNodePhase {
    Pending,
    Running,
    Done,
    Failed,
    Skipped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GraphRunPhase {
    Running,
    Done,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, serde::Serialize)]
pub(crate) struct GraphNodeSnap {
    pub id: String,
    pub deps: Vec<String>,
    pub persona: String,
    pub club: String,
    pub grant: String,
    pub pool: Option<String>,
    pub pool_limit: Option<usize>,
    /// Current lease while running, or the last lease after landing.
    pub lease_id: Option<u64>,
    pub phase: GraphNodePhase,
    pub elapsed_ms: u128,
    pub retries: usize,
    pub gate: Option<String>,
    pub output_chars: usize,
}

/// Shared fan-in truth for the product trace and the G01 cohort reducer.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, Deserialize)]
pub(crate) struct GraphFaninTruth {
    pub finished_with_result: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub(crate) struct GraphSnapshot {
    /// Join the view to its exact durable episode and originating workspace.
    pub episode_id: String,
    pub workspace_key_sha256: String,
    pub graph: String,
    pub task: String,
    pub phase: GraphRunPhase,
    pub nodes: Vec<GraphNodeSnap>,
    pub events: Vec<String>,
    pub final_answer: Option<String>,
    pub error: Option<String>,
    pub elapsed_ms: u128,
    pub fanin: GraphFaninTruth,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incomplete_reason: Option<String>,
}

static CURRENT_GRAPH: Mutex<Option<GraphSnapshot>> = Mutex::new(None);
static GRAPH_TURN_SNAPS: Mutex<Vec<GraphSnapshot>> = Mutex::new(Vec::new());

/// One graph invocation in the current task turn.
#[derive(Clone, Debug, serde::Serialize)]
pub(crate) struct GraphTurnEpisode {
    pub episode_index: usize,
    pub episode_id: String,
    pub phase: GraphRunPhase,
    pub nodes_done: usize,
    pub nodes_total: usize,
    pub fanin: GraphFaninTruth,
}

pub(crate) fn clear_graph_turn_episodes() {
    if let Ok(mut log) = GRAPH_TURN_SNAPS.lock() {
        log.clear();
    }
}

fn graph_turn_episode_from_snap(index: usize, snapshot: &GraphSnapshot) -> GraphTurnEpisode {
    GraphTurnEpisode {
        episode_index: index,
        episode_id: snapshot.episode_id.clone(),
        phase: snapshot.phase,
        nodes_done: snapshot
            .nodes
            .iter()
            .filter(|n| n.phase == GraphNodePhase::Done)
            .count(),
        nodes_total: snapshot.nodes.len(),
        fanin: snapshot.fanin.clone(),
    }
}

pub(crate) fn graph_turn_episodes() -> Vec<GraphTurnEpisode> {
    GRAPH_TURN_SNAPS
        .lock()
        .ok()
        .map(|log| {
            log.iter()
                .enumerate()
                .map(|(i, snap)| graph_turn_episode_from_snap(i, snap))
                .collect()
        })
        .unwrap_or_default()
}

fn record_terminal_graph_episode(snapshot: &GraphSnapshot) {
    if snapshot.phase == GraphRunPhase::Running {
        return;
    }
    if let Ok(mut log) = GRAPH_TURN_SNAPS.lock() {
        if log
            .last()
            .is_some_and(|prev| prev.episode_id == snapshot.episode_id)
        {
            let idx = log.len() - 1;
            log[idx] = snapshot.clone();
            return;
        }
        log.push(snapshot.clone());
    }
}

fn publish_graph_snapshot(snapshot: GraphSnapshot) {
    record_terminal_graph_episode(&snapshot);
    if let Ok(mut g) = CURRENT_GRAPH.lock() {
        *g = Some(snapshot);
    }
}

/// The latest (possibly finished) run, for the Round Table stage and `/graph
/// status`. Sticky across completion so the operator can read the outcome.
pub(crate) fn current_graph_snapshot() -> Option<GraphSnapshot> {
    CURRENT_GRAPH.lock().ok().and_then(|g| g.clone())
}

/// Last episode that reached `Done`, else the last recorded episode.
pub(crate) fn judged_graph_snapshot() -> Option<GraphSnapshot> {
    let log = GRAPH_TURN_SNAPS.lock().ok()?.clone();
    log.iter()
        .rev()
        .find(|snap| snap.phase == GraphRunPhase::Done)
        .cloned()
        .or_else(|| log.last().cloned())
        .or_else(current_graph_snapshot)
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

pub(crate) struct AgentGraphEngine {
    workspace: PathBuf,
    self_club: Option<Arc<dyn Club>>,
    clubs: HashMap<String, Arc<dyn Club>>,
    personas: Vec<Skill>,
    persist: bool,
    cargo: PinnedCargo,
}

#[derive(Debug)]
pub(crate) struct GraphRunOutcome {
    pub answer: String,
    pub snapshot: GraphSnapshot,
    pub episode: GraphEpisodeV1,
    pub episode_persistence: GraphEpisodePersistence,
    pub descendant_budget: DescendantBudgetReceipt,
}

#[derive(Debug)]
pub(crate) struct GraphRunFailure {
    pub reason: String,
    pub snapshot: GraphSnapshot,
    pub episode: GraphEpisodeV1,
    pub episode_persistence: GraphEpisodePersistence,
    pub descendant_budget: DescendantBudgetReceipt,
}

#[derive(Debug)]
pub(crate) enum GraphRunError {
    /// Validation, resolution, or admission failed before an episode existed.
    Preflight(String),
    /// Execution reached the episode seal. The typed receipt is retained even
    /// when execution or its durable publication failed.
    Run(Box<GraphRunFailure>),
}

impl GraphRunError {
    fn message(&self) -> &str {
        match self {
            Self::Preflight(message) => message,
            Self::Run(failure) => &failure.reason,
        }
    }

    #[cfg(test)]
    fn contains(&self, pattern: &str) -> bool {
        self.message().contains(pattern)
    }

    #[cfg(test)]
    fn run_failure(&self) -> Option<&GraphRunFailure> {
        match self {
            Self::Run(failure) => Some(failure),
            Self::Preflight(_) => None,
        }
    }
}

impl std::fmt::Display for GraphRunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message())
    }
}

impl std::error::Error for GraphRunError {}

impl From<String> for GraphRunError {
    fn from(message: String) -> Self {
        Self::Preflight(message)
    }
}

impl From<&str> for GraphRunError {
    fn from(message: &str) -> Self {
        Self::Preflight(message.to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GraphEpisodePersistence {
    Disabled,
    Persisted,
    Failed(String),
}

impl GraphEpisodePersistence {
    fn label(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Persisted => "persisted",
            Self::Failed(_) => "failed",
        }
    }

    fn failure_detail(&self) -> Option<&str> {
        match self {
            Self::Failed(error) => Some(error),
            Self::Disabled | Self::Persisted => None,
        }
    }
}

fn graph_receipt_header(
    spec: &GraphSpec,
    snapshot: &GraphSnapshot,
    episode: &GraphEpisodeV1,
    episode_persistence: &GraphEpisodePersistence,
    descendant_budget: DescendantBudgetReceipt,
    elapsed: Duration,
) -> String {
    let done = snapshot
        .nodes
        .iter()
        .filter(|node| node.phase == GraphNodePhase::Done)
        .count();
    format!(
        "[graph {} phase={:?} nodes={done}/{} elapsed={}s episode={} receipt={} {}]",
        spec.name,
        snapshot.phase,
        snapshot.nodes.len(),
        elapsed.as_secs(),
        episode.episode_id,
        episode_persistence.label(),
        descendant_budget.status_fields(),
    )
}

fn render_graph_run_failure(
    spec: &GraphSpec,
    failure: &GraphRunFailure,
    elapsed: Duration,
) -> String {
    let header = graph_receipt_header(
        spec,
        &failure.snapshot,
        &failure.episode,
        &failure.episode_persistence,
        failure.descendant_budget,
        elapsed,
    );
    let mut detail = head_chars(&failure.reason, GRAPH_FAILURE_REASON_CAP);
    if let Some(persistence_error) = failure.episode_persistence.failure_detail() {
        detail.push_str("; episode persistence failed: ");
        detail.push_str(&head_chars(persistence_error, GRAPH_TRACE_STOP_REASON_CAP));
    }
    format!("{header}\n{detail}")
}

struct NodeDone {
    started_at: Option<GraphTimestamp>,
    finished_at: GraphTimestamp,
    idx: usize,
    elapsed_ms: u128,
    result: Result<String, String>,
    termination: GraphTraceTerminationV1,
    hops: usize,
    resolved_route: RouteIdentity,
    resolved_route_config: GraphRouteConfigV1,
    rollout_id: Option<String>,
    retry_count: usize,
    stall_retries: usize,
}

struct NodeAttemptContext {
    wall_remaining_ms: Option<u64>,
    inputs_ready_at: GraphTimestamp,
    timing: Arc<std::sync::Mutex<Option<GraphTimestamp>>>,
    causal_generation: u64,
    attempt_index: u32,
    trace_id: String,
    parent_trace_ids: Vec<String>,
    retry_of_trace_id: Option<String>,
    feedback_sha256: Option<String>,
    requested_route: RouteIdentity,
    requested_route_config: GraphRouteConfigV1,
    system: GraphTextArtifactV1,
    prompt: GraphTextArtifactV1,
}

impl AgentGraphEngine {
    pub(crate) fn new(
        workspace: PathBuf,
        self_club: Option<Arc<dyn Club>>,
        roster: Vec<Arc<dyn Club>>,
    ) -> Self {
        let cargo = PinnedCargo::capture(&workspace);
        Self::new_with_cargo(workspace, self_club, roster, cargo)
    }

    pub(crate) fn new_with_cargo(
        workspace: PathBuf,
        self_club: Option<Arc<dyn Club>>,
        roster: Vec<Arc<dyn Club>>,
        cargo: PinnedCargo,
    ) -> Self {
        let clubs = roster
            .into_iter()
            .map(|c| (c.label().to_lowercase(), c))
            .collect();
        Self {
            workspace,
            self_club,
            clubs,
            personas: load_personas(),
            persist: true,
            cargo,
        }
    }

    #[cfg(test)]
    fn without_persistence(mut self) -> Self {
        self.persist = false;
        self
    }

    /// `self`/`auto`/empty = the in-hand driver (quality floor), `smart` = the
    /// designated escalation seat, else an explicit club label. Paid SOTA seats
    /// honor the same withholding rules as `spawn` — and say so.
    fn resolve_club(&self, spec: Option<&str>) -> Result<Arc<dyn Club>, String> {
        let mut want = spec.unwrap_or("").trim().to_lowercase();
        if want == "smart" || want == "sota" {
            let seat = crate::agent::club::smart_seat();
            let seat = if self.clubs.contains_key(&seat.club.to_lowercase()) {
                seat
            } else {
                crate::agent::club::smart_seat_alt()
            };
            want = seat.club.to_lowercase();
            if let Some(club) = self.clubs.get(&want) {
                let (club, _) =
                    crate::agent::club::scoped_reasoning_effort(Arc::clone(club), &seat.effort);
                return Ok(club);
            }
            return Err(format!("smart seat '{want}' is not in the roster"));
        }
        if want.is_empty() || want == "self" || want == "auto" {
            if let Some(club) = &self.self_club {
                return Ok(Arc::clone(club));
            }
            if let Some(club) = self.clubs.values().find(|c| c.is_available()) {
                return Ok(Arc::clone(club));
            }
            return Err("no in-hand club and no reachable roster club".to_string());
        }
        match self.clubs.get(&want) {
            Some(club)
                if crate::agent::tools::consult::is_optional_local_label(club.label())
                    && !club.is_available() =>
            {
                if let Some(self_club) = &self.self_club {
                    return Ok(Arc::clone(self_club));
                }
                if let Some(live) = self.clubs.values().find(|c| c.is_available()) {
                    return Ok(Arc::clone(live));
                }
                Err(format!(
                    "club '{want}' is not reachable and no live local/self seat is available"
                ))
            }
            Some(club) => Ok(Arc::clone(club)),
            None if crate::agent::club::is_sota_label(&want)
                && (crate::agent::tools::solo::solo_mode_active()
                    || !crate::agent::harness::env_flag("ANGEL_ALLOW_SOTA_DELEGATE", true)) =>
            {
                Err(format!(
                    "club '{want}' is a paid SOTA seat withheld from graph nodes \
                     (solo mode or ANGEL_ALLOW_SOTA_DELEGATE=0)"
                ))
            }
            None => {
                let mut names: Vec<&String> = self.clubs.keys().collect();
                names.sort();
                Err(format!(
                    "unknown club '{want}'; have: self, auto, smart, {}",
                    names
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            }
        }
    }

    fn persona_body(&self, name: Option<&str>) -> Result<(String, String), String> {
        let Some(want) = name.map(str::trim).filter(|s| !s.is_empty()) else {
            return Ok((String::new(), String::new()));
        };
        if plain_persona_alias(want) {
            return Ok((String::new(), String::new()));
        }
        self.personas
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(want))
            .map(|p| (p.name.clone(), p.body.clone()))
            .ok_or_else(|| {
                let names: Vec<&str> = self.personas.iter().map(|p| p.name.as_str()).collect();
                format!(
                    "unknown persona '{want}'. Available: {}. Use an exact installed name, or omit `persona` for a plain node",
                    if names.is_empty() {
                        "(none installed)".to_string()
                    } else {
                        names.join(", ")
                    }
                )
            })
    }

    /// `cancel` is the caller's authority (operator `/graph stop`, or the turn
    /// flag when invoked as a tool); it is polled between landings. Seats share
    /// an internal flag the run also raises on failure/deadline so cut seats
    /// wind down at their next hop boundary.
    pub(crate) fn run(
        &self,
        spec: &GraphSpec,
        task: &str,
        cancel: &AtomicBool,
    ) -> Result<GraphRunOutcome, GraphRunError> {
        // Tool dispatch normally inherits the root turn's allowance. Direct
        // engine callers (including tests) still receive one root scope, while
        // graph worker threads install this exact handle below.
        let _budget_scope = DescendantBudgetScope::enter_root();
        let descendant_budget = current_descendant_budget()?;
        let _formation_scope = super::formation_budget::start_turn()?;
        let token_allocation = match (super::formation_budget::current(), spec.token_budget) {
            (Some(budget), _) => Some(budget),
            (None, Some(tokens)) => Some(super::formation_budget::Budget::new(Some(tokens), None)),
            (None, None) => Some(super::formation_budget::Budget::new(None, None)),
        };
        let _graph_allocation_scope = super::formation_budget::enter(token_allocation.clone());
        if let Some(budget) = &token_allocation {
            budget.set_unstarted(spec.nodes.len() as u64);
        }
        validate_graph_spec(spec)?;
        let _signal_guard = crate::agent::sandbox::process_owner::defer_exit_for_graph();
        let pool_limits = graph_pool_policies(spec)?;
        let started = Instant::now();
        let started_ms = unix_ms();
        let episode_start = GraphTimestamp::now(started);
        let mut cancel_requested_at = None;
        let graph_spec_sha256 = crate::knowledge::cut::sha256_hex(
            &serde_json::to_vec(spec)
                .map_err(|error| format!("encode graph spec receipt: {error}"))?,
        );
        let task_sha256 = crate::knowledge::cut::sha256_hex(task.as_bytes());
        let workspace_key_sha256 = crate::knowledge::cut::sha256_hex(
            crate::platform::workspace_store::workspace_key(&self.workspace).as_bytes(),
        );
        let n = spec.nodes.len();
        let index: HashMap<&str, usize> = spec
            .nodes
            .iter()
            .enumerate()
            .map(|(i, node)| (node.id.as_str(), i))
            .collect();

        // Resolve every club/persona/grant before any seat starts (fail fast,
        // and the failure names its node).
        let mut seats: Vec<(Arc<dyn Club>, String, Grant)> = Vec::with_capacity(n);
        let mut effort_notes: Vec<String> = Vec::new();
        for node in &spec.nodes {
            let mut club = self
                .resolve_club(node.club.as_deref())
                .map_err(|e| format!("node '{}': {e}", node.id))?;
            let (_, persona_body) = self
                .persona_body(node.persona.as_deref())
                .map_err(|e| format!("node '{}': {e}", node.id))?;
            let grant = node_grant(node).map_err(|e| format!("node '{}': {e}", node.id))?;
            let effort = node.effort.clone().or_else(|| club.reasoning_effort());
            if let Some(effort) = effort.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                // Pin an immutable view of the shared club. Every seat carries
                // its own effort on the provider call, so concurrent preflight
                // cannot overwrite another node's request.
                let (scoped, applied) = crate::agent::club::scoped_reasoning_effort(club, effort);
                club = scoped;
                match applied {
                    Some(applied) => effort_notes.push(format!(
                        "{} · effort {applied} pinned on {}",
                        node.id,
                        club.label()
                    )),
                    None => effort_notes.push(format!(
                        "{} · effort '{effort}' not accepted by {} (model-native route)",
                        node.id,
                        club.label()
                    )),
                }
            }
            seats.push((club, persona_body, grant));
        }
        // One scoped registry per distinct grant; node seats never re-enter
        // spawn/graph (no nested fan-out from inside a graph).
        let mut registries: Vec<(Grant, Arc<ToolRegistry>)> = Vec::new();
        for (_, _, grant) in &seats {
            if !registries.iter().any(|(g, _)| g == grant) {
                registries.push((
                    *grant,
                    Arc::new(grant.registry(&self.workspace, None, &self.cargo)),
                ));
            }
        }
        let registry_for = |grant: Grant| -> Arc<ToolRegistry> {
            registries
                .iter()
                .find(|(g, _)| *g == grant)
                .map(|(_, r)| Arc::clone(r))
                .expect("registry prebuilt for every grant")
        };

        let deadline = Duration::from_secs(
            spec.deadline_secs
                .unwrap_or(
                    env_usize("ANGEL_GRAPH_DEADLINE_SECS", GRAPH_DEADLINE_DEFAULT_SECS) as u64,
                ),
        );
        let graph_deadline = (!deadline.is_zero()).then(|| started + deadline);
        let inherited_deadline = super::formation_budget::request_wall_remaining()
            .map(|remaining| Instant::now() + remaining);
        let request_deadline = match (graph_deadline, inherited_deadline) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let max_seats = env_usize("ANGEL_GRAPH_MAX_SEATS", GRAPH_MAX_SEATS_DEFAULT).max(1);
        let inflight_limit = env_usize("ANGEL_SPAWN_INFLIGHT_MAX", 64).max(max_seats);
        let node_max_hops = default_max_hops();
        let dependency_context_chars = env_usize("ANGEL_GRAPH_DEP_CAP", DEP_CONTEXT_CAP).max(1_000);
        let runtime = GraphRuntimeConfigV1::new(
            deadline.as_secs(),
            max_seats,
            inflight_limit,
            node_max_hops,
            dependency_context_chars,
        )?;
        let episode_nonce = format!(
            "{started_ms}:{}:{}:{graph_spec_sha256}:{task_sha256}:{}",
            std::process::id(),
            GRAPH_EPISODE_SEQ.fetch_add(1, Ordering::Relaxed),
            runtime.config_sha256,
        );
        let episode_id = crate::knowledge::cut::sha256_hex(episode_nonce.as_bytes());

        // Admit the complete declared graph before the first node launches.
        // Nodes cancelled or skipped after this point remain charged logical
        // calls; live process capacity is enforced independently at dispatch.
        let initial_budget_receipt =
            reserve_descendant_calls(&descendant_budget, n, "agent graph initial wave")?;

        let mut phase = vec![GraphNodePhase::Pending; n];
        let mut elapsed_ms = vec![0u128; n];
        let mut retries = vec![0usize; n];
        let mut gate_note: Vec<Option<String>> = vec![None; n];
        let mut outputs: HashMap<String, String> = HashMap::new();
        let mut feedback: HashMap<String, String> = HashMap::new();
        let mut attempts_started = vec![0u32; n];
        let mut causal_generation = vec![0u64; n];
        let mut last_trace_index: Vec<Option<usize>> = vec![None; n];
        let mut accepted_trace_index: Vec<Option<usize>> = vec![None; n];
        let mut inflight: Vec<Option<NodeAttemptContext>> = (0..n).map(|_| None).collect();
        let mut pool_running: BTreeMap<String, usize> =
            pool_limits.keys().cloned().map(|name| (name, 0)).collect();
        let mut pool_lease_seq: BTreeMap<String, u64> =
            pool_limits.keys().cloned().map(|name| (name, 0)).collect();
        let mut lease_ids: Vec<Option<u64>> = vec![None; n];
        let mut traces: Vec<GraphTraceV1> = Vec::new();
        let mut events: Vec<String> = effort_notes
            .into_iter()
            .map(|note| format!("  0.0s · {}", head_chars(&note, GRAPH_EVENT_TEXT_CAP)))
            .collect();
        events.push(format!(
            "  0.0s · descendant budget admitted initial graph · {}",
            initial_budget_receipt.fields()
        ));
        let mut winding: Option<String> = None;
        let mut cancelled = false;
        let mut graph_deadline_reached = false;
        let mut wind_started: Option<Instant> = None;
        let mut running = 0usize;
        let seat_cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel::<NodeDone>();

        let push_event = |events: &mut Vec<String>, text: String| {
            if events.len() >= EVENT_CAP {
                events.remove(0);
            }
            events.push(format!(
                "{:>5.1}s · {}",
                started.elapsed().as_secs_f32(),
                head_chars(&text, GRAPH_EVENT_TEXT_CAP),
            ));
        };
        let snapshot_episode_id = episode_id.clone();
        let snapshot_workspace_key = workspace_key_sha256.clone();
        let snapshot = |phase: &[GraphNodePhase],
                        elapsed_ms: &[u128],
                        retries: &[usize],
                        gate_note: &[Option<String>],
                        lease_ids: &[Option<u64>],
                        outputs: &HashMap<String, String>,
                        events: &[String],
                        run_phase: GraphRunPhase,
                        final_answer: Option<&str>,
                        error: Option<&str>|
         -> GraphSnapshot {
            GraphSnapshot {
                episode_id: snapshot_episode_id.clone(),
                workspace_key_sha256: snapshot_workspace_key.clone(),
                graph: spec.name.clone(),
                task: task.to_string(),
                phase: run_phase,
                nodes: spec
                    .nodes
                    .iter()
                    .enumerate()
                    .map(|(i, node)| GraphNodeSnap {
                        id: node.id.clone(),
                        deps: node.depends_on.clone(),
                        persona: node.persona.clone().unwrap_or_default(),
                        club: seats[i].0.label().to_string(),
                        grant: seats[i].2.label().to_string(),
                        pool: node_pool_name(node).map(str::to_string),
                        pool_limit: node_pool_name(node)
                            .and_then(|pool| pool_limits.get(pool).copied()),
                        lease_id: lease_ids[i],
                        phase: phase[i],
                        elapsed_ms: elapsed_ms[i],
                        retries: retries[i],
                        gate: gate_note[i].clone(),
                        output_chars: outputs.get(&node.id).map_or(0, |o| o.chars().count()),
                    })
                    .collect(),
                events: events.to_vec(),
                final_answer: final_answer.map(|a| head_chars(a, SNAPSHOT_ANSWER_CAP)),
                error: error.map(|reason| head_chars(reason, GRAPH_FAILURE_REASON_CAP)),
                elapsed_ms: started.elapsed().as_millis(),
                fanin: graph_fanin_truth(spec, phase, outputs),
                incomplete_reason: error.filter(|_| run_phase != GraphRunPhase::Done).map(
                    |reason| {
                        graph_incomplete_reason_label(
                            reason,
                            matches!(run_phase, GraphRunPhase::Cancelled),
                        )
                    },
                ),
            }
        };
        publish_graph_snapshot(snapshot(
            &phase,
            &elapsed_ms,
            &retries,
            &gate_note,
            &lease_ids,
            &outputs,
            &events,
            GraphRunPhase::Running,
            None,
            None,
        ));

        loop {
            if winding.is_none()
                && (cancel.load(Ordering::Relaxed)
                    || crate::agent::sandbox::process_owner::exit_requested())
            {
                cancel_requested_at = Some(GraphTimestamp::now(started));
                seat_cancel.store(true, Ordering::Relaxed);
                winding = Some("cancelled by operator".to_string());
                cancelled = true;
                wind_started = Some(Instant::now());
            }
            if winding.is_none() && request_deadline.is_some_and(|end| Instant::now() >= end) {
                seat_cancel.store(true, Ordering::Relaxed);
                winding = Some(format!(
                    "deadline {}s reached; operator cap graph.deadline_secs / ANGEL_GRAPH_DEADLINE_SECS={}",
                    deadline.as_secs(),
                    deadline.as_secs()
                ));
                graph_deadline_reached = true;
                wind_started = Some(Instant::now());
            }
            if winding.is_none() {
                // Dispatch every ready node up to the seat cap.
                while running < max_seats {
                    let ready = spec.nodes.iter().enumerate().position(|(i, node)| {
                        phase[i] == GraphNodePhase::Pending
                            && node
                                .depends_on
                                .iter()
                                .all(|d| phase[index[d.as_str()]] == GraphNodePhase::Done)
                            && node_pool_name(node).is_none_or(|pool| {
                                pool_running.get(pool).copied().unwrap_or_default()
                                    < pool_limits.get(pool).copied().unwrap_or(1)
                            })
                    });
                    let Some(idx) = ready else { break };
                    match reserve_spawn_seats(1, inflight_limit) {
                        Ok(mut permits) => {
                            let permit = permits.pop().expect("one permit requested");
                            phase[idx] = GraphNodePhase::Running;
                            let node = &spec.nodes[idx];
                            let pool_lease = node_pool_name(node).map(|pool| {
                                let active = pool_running
                                    .get_mut(pool)
                                    .expect("validated pool has running state");
                                *active = active.saturating_add(1);
                                let sequence = pool_lease_seq
                                    .get_mut(pool)
                                    .expect("validated pool has lease sequence");
                                *sequence = sequence.saturating_add(1);
                                lease_ids[idx] = Some(*sequence);
                                (
                                    pool.to_string(),
                                    pool_limits.get(pool).copied().unwrap_or(1),
                                    *sequence,
                                )
                            });
                            let system =
                                node_system_prompt(&spec.name, node, &seats[idx].1, seats[idx].2);
                            let user = render_node_prompt(
                                node,
                                task,
                                &outputs,
                                &feedback,
                                dependency_context_chars,
                            );
                            let club = Arc::clone(&seats[idx].0);
                            let requested_route = club.route_identity();
                            let requested_route_config = GraphRouteConfigV1::new(
                                requested_route.clone(),
                                &club.route_metadata(),
                            );
                            let attempt_index = attempts_started[idx];
                            attempts_started[idx] = attempts_started[idx].saturating_add(1);
                            let trace_id = graph_trace_id(&episode_id, &node.id, attempt_index);
                            let parent_trace_ids = node
                                .depends_on
                                .iter()
                                .filter_map(|dep| accepted_trace_index[index[dep.as_str()]])
                                .map(|trace_idx| traces[trace_idx].trace_id.clone())
                                .collect();
                            let retry_of_trace_id = last_trace_index[idx]
                                .map(|trace_idx| traces[trace_idx].trace_id.clone());
                            let feedback_sha256 = feedback
                                .get(&node.id)
                                .map(|body| crate::knowledge::cut::sha256_hex(body.as_bytes()));
                            let inputs_ready_at = node
                                .depends_on
                                .iter()
                                .filter_map(|dep| accepted_trace_index[index[dep.as_str()]])
                                .filter_map(|i| traces[i].finished_at)
                                .max_by_key(|t| t.monotonic_ns)
                                .unwrap_or(episode_start);
                            let timing = Arc::new(std::sync::Mutex::new(None));
                            let thread_timing = Arc::clone(&timing);
                            let wall_remaining_ms = request_deadline.map(|end| {
                                end.saturating_duration_since(Instant::now())
                                    .as_millis()
                                    .min(u64::MAX as u128) as u64
                            });
                            inflight[idx] = Some(NodeAttemptContext {
                                wall_remaining_ms,
                                inputs_ready_at,
                                timing,
                                causal_generation: causal_generation[idx],
                                attempt_index,
                                trace_id,
                                parent_trace_ids,
                                retry_of_trace_id,
                                feedback_sha256,
                                requested_route: requested_route.clone(),
                                requested_route_config: requested_route_config.clone(),
                                system: GraphTextArtifactV1::capture(&system),
                                prompt: GraphTextArtifactV1::capture(&user),
                            });
                            let registry = registry_for(seats[idx].2);
                            let thread_cancel = Arc::clone(&seat_cancel);
                            let seat_tx = tx.clone();
                            let grant = seats[idx].2;
                            let thread_budget = descendant_budget.clone();
                            let thread_allocation = token_allocation.clone();
                            let budget_role = node.id.clone();
                            let spawn_result = std::thread::Builder::new()
                                .name(format!("graph-{}", node.id))
                                .spawn(move || {
                                    let _permit = permit;
                                    let _budget = DescendantBudgetScope::inherit(thread_budget);
                                    let _allocation =
                                        super::formation_budget::enter(thread_allocation);
                                    let _role = super::formation_budget::enter_role(&budget_role);
                                    let _wall = request_deadline
                                        .map(super::formation_budget::RequestWallScope::enter);
                                    let _depth = SubcallDepthGuard::enter();
                                    let seat_started = Instant::now();
                                    let started_at = GraphTimestamp::now(started);
                                    *thread_timing.lock().unwrap_or_else(|e| e.into_inner()) =
                                        Some(started_at);
                                    let mut history = vec![
                                        ChatMsg::system(system.as_str()),
                                        ChatMsg::user(user.as_str()),
                                    ];
                                    if let Some(budget) = super::formation_budget::current()
                                        && !club.supports_formation_budget()
                                    {
                                        budget.note_untracked_provider();
                                    }
                                    // Toolless nodes are deli-shaped workers: one direct
                                    // chat, no harness tool-loop — its coding guards
                                    // (no-edit denial, verification nudges) would tax a
                                    // reasoning-only seat with re-hops it can't satisfy.
                                    let stall_window = crate::agent::club::identity_stream_stall_secs();
                                    let provider_retries = provider_retry_budget();
                                    let backoff_ms = env_usize("ANGEL_PROVIDER_RETRY_BACKOFF_MS", 500);
                                    let mut node_retries = 0usize;
                                    let mut stall_retries = 0usize;
                                    let (answer, termination, hops, rollout_id) =
                                        if grant == Grant::None {
                                            let last;
                                            let mut attempt = 0u32;
                                            loop {
                                                match club.chat_streaming(
                                                    &history,
                                                    &[],
                                                    &thread_cancel,
                                                    &mut |_| {},
                                                ) {
                                                    Ok(crate::agent::club::ClubReply::Text(text))
                                                        if !text.trim().is_empty() =>
                                                    {
                                                        last = Some((
                                                            Ok(text),
                                                            GraphTraceTerminationV1::answer(),
                                                            1,
                                                            None,
                                                        ));
                                                        break;
                                                    }
                                                    Ok(crate::agent::club::ClubReply::Text(_)) => {
                                                        let error = "empty reply".to_string();
                                                        last = Some((
                                                            Err(error.clone()),
                                                            GraphTraceTerminationV1::failure(
                                                                GraphTraceTerminationKind::PolicyGuard,
                                                                &error,
                                                            ),
                                                            1,
                                                            None,
                                                        ));
                                                        break;
                                                    }
                                                    Ok(crate::agent::club::ClubReply::Calls(_)) => {
                                                        let error = "tool call from a toolless node".to_string();
                                                        last = Some((
                                                            Err(error.clone()),
                                                            GraphTraceTerminationV1::failure(
                                                                GraphTraceTerminationKind::PolicyGuard,
                                                                &error,
                                                            ),
                                                            1,
                                                            None,
                                                        ));
                                                        break;
                                                    }
                                                    Err(error)
                                                        if is_recoverable_stream_error(&error)
                                                            && !is_permanent_provider_error(&error)
                                                            && retry_budget_allows(
                                                                provider_retries,
                                                                attempt as usize,
                                                            )
                                                            && !thread_cancel.load(Ordering::Relaxed) =>
                                                    {
                                                        eprintln!(
                                                            "[graph] node {budget_role} stream stalled after {stall_window} s (attempt {attempt}) → retry"
                                                        );
                                                        wait_graph_backoff(
                                                            &thread_cancel,
                                                            backoff_ms,
                                                            attempt,
                                                        );
                                                        attempt = attempt.saturating_add(1);
                                                        node_retries = node_retries.saturating_add(1);
                                                        stall_retries = stall_retries.saturating_add(1);
                                                        continue;
                                                    }
                                                    Err(error) => {
                                                        last = Some((
                                                            Err(error.clone()),
                                                            GraphTraceTerminationV1::failure(
                                                                GraphTraceTerminationKind::ProviderFailure,
                                                                &error,
                                                            ),
                                                            1,
                                                            None,
                                                        ));
                                                        break;
                                                    }
                                                }
                                            }
                                            last.expect("toolless graph seat produces a landing")
                                        } else {
                                            let (evt_tx, _) = mpsc::channel::<TurnEvent>();
                                            // A tool-bearing node is a full turn: its
                                            // own retry budget owns recovery,
                                            // including replaying a cut hop with the
                                            // committed history and tool results
                                            // intact. Retrying here as well would
                                            // multiply the operator's explicit
                                            // allowance and re-run committed work, so
                                            // the graph lands the turn's verdict.
                                            match run_turn_observed(
                                                &*club,
                                                &registry,
                                                &mut history,
                                                &thread_cancel,
                                                Some(node_max_hops),
                                                &evt_tx,
                                            ) {
                                                Ok(outcome) => {
                                                    let termination =
                                                        GraphTraceTerminationV1::from_turn(
                                                            outcome.stop_reason,
                                                            outcome.interrupted,
                                                            outcome.deadline_reached,
                                                            outcome.max_hops_reached,
                                                            None,
                                                        );
                                                    (
                                                        Ok(outcome.answer),
                                                        termination,
                                                        outcome.hops,
                                                        outcome.rollout_id,
                                                    )
                                                }
                                                Err(failure) => {
                                                    let termination =
                                                        GraphTraceTerminationV1::from_turn(
                                                            failure.stop_reason,
                                                            failure.interrupted,
                                                            failure.deadline_reached,
                                                            failure.max_hops_reached,
                                                            Some(&failure.message),
                                                        );
                                                    (
                                                        Err(failure.message),
                                                        termination,
                                                        failure.hops,
                                                        failure.rollout_id,
                                                    )
                                                }
                                            }
                                        };

                                      let resolved_route = club.resolved_route_identity();
                                    let resolved_route_config = GraphRouteConfigV1::new(
                                        resolved_route.clone(),
                                        &club.route_metadata(),
                                    );
                                    let _ = seat_tx.send(NodeDone {
                                        started_at: Some(started_at),
                                        finished_at: GraphTimestamp::now(started),
                                        idx,
                                        elapsed_ms: seat_started.elapsed().as_millis(),
                                        result: answer,
                                        termination,
                                        hops,
                                        resolved_route,
                                        resolved_route_config,
                                        rollout_id,
                                        retry_count: node_retries,
                                        stall_retries,
                                    });
                                });
                            if let Err(e) = spawn_result {
                                let error = format!("seat thread failed to spawn: {e}");
                                let _ = tx.send(NodeDone {
                                    started_at: None,
                                    finished_at: GraphTimestamp::now(started),
                                    idx,
                                    elapsed_ms: 0,
                                    result: Err(error.clone()),
                                    termination: GraphTraceTerminationV1::failure(
                                        GraphTraceTerminationKind::SpawnFailure,
                                        &error,
                                    ),
                                    hops: 0,
                                    resolved_route: requested_route,
                                    resolved_route_config: requested_route_config,
                                    rollout_id: None,
                                    retry_count: 0,
                                    stall_retries: 0,
                                });
                            }
                            running += 1;
                            let dispatch = pool_lease.map_or_else(
                                || format!("▶ {} ({})", node.id, seats[idx].0.label()),
                                |(pool, limit, lease)| {
                                    format!(
                                        "▶ {} ← pool:{} lease#{} cap{} ({})",
                                        node.id,
                                        pool,
                                        lease,
                                        limit,
                                        seats[idx].0.label()
                                    )
                                },
                            );
                            push_event(&mut events, dispatch);
                            let live: Vec<String> = spec
                                .nodes
                                .iter()
                                .enumerate()
                                .filter(|(i, _)| phase[*i] == GraphNodePhase::Running)
                                .map(|(_, node)| node.id.clone())
                                .collect();
                            crate::ui::viz::agentviz::stage(format!("graph {}", spec.name), live);
                        }
                        Err(e) => {
                            if running == 0 {
                                winding = Some(format!("spawn capacity: {e}"));
                                seat_cancel.store(true, Ordering::Relaxed);
                                wind_started = Some(Instant::now());
                            }
                            break; // wait for landings to free seats
                        }
                    }
                }
            }
            if running == 0 {
                break; // all landed: run complete, failed, or nothing dispatchable
            }
            // An operator cancellation must observe the workers actually landing
            // before sealing stop timestamps. A wind-down grace expiry proves
            // abandonment, not stopped work; failure/deadline behavior remains
            // separate from this explicit cancellation path.
            if let Some(ws) = wind_started
                && !cancelled
                && ws.elapsed() > WIND_DOWN_GRACE
            {
                for (i, p) in phase.iter_mut().enumerate() {
                    if *p == GraphNodePhase::Running {
                        *p = GraphNodePhase::Failed;
                        gate_note[i] = Some("abandoned in wind-down".to_string());
                    }
                }
                break;
            }
            let done = match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(done) => done,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            running = running.saturating_sub(1);
            let idx = done.idx;
            if let Some(pool) = node_pool_name(&spec.nodes[idx]) {
                let active = pool_running
                    .get_mut(pool)
                    .expect("validated pool has running state");
                *active = active.saturating_sub(1);
            }
            elapsed_ms[idx] = done.elapsed_ms;
            retries[idx] = retries[idx].saturating_add(done.retry_count);
            let node = &spec.nodes[idx];
            let context = inflight[idx]
                .take()
                .expect("every landing belongs to one dispatched graph attempt");
            let landing_generation = context.causal_generation;
            let stale_landing = landing_generation != causal_generation[idx];
            let output = done
                .result
                .as_ref()
                .ok()
                .map(|text| GraphTextArtifactV1::capture(text));
            // A provider often reports cooperative graph wind-down as a
            // generic transport error. Attribute only failed landings to the
            // graph authority that actually stopped them; a seat that managed
            // to answer before landing retains its truthful Answer receipt.
            let termination = if done.termination.kind == GraphTraceTerminationKind::BudgetExhausted
            {
                done.termination
            } else if done.result.is_err() && cancelled {
                GraphTraceTerminationV1::failure(
                    GraphTraceTerminationKind::Interrupt,
                    winding.as_deref().unwrap_or("cancelled by operator"),
                )
            } else if done.result.is_err() && graph_deadline_reached {
                GraphTraceTerminationV1::failure(
                    GraphTraceTerminationKind::Deadline,
                    winding.as_deref().unwrap_or("graph deadline reached"),
                )
            } else if done.result.is_err() && winding.is_some() {
                GraphTraceTerminationV1::failure(
                    GraphTraceTerminationKind::Abandoned,
                    winding
                        .as_deref()
                        .unwrap_or("graph wound down after another node failed"),
                )
            } else {
                done.termination
            };
            let trace_idx = traces.len();
            traces.push(GraphTraceV1 {
                wall_remaining_ms: context.wall_remaining_ms,
                schema: GRAPH_TRACE_SCHEMA.to_string(),
                trace_id: context.trace_id,
                role: node.id.clone(),
                persona: node.persona.clone(),
                trainable: node.trainable,
                requested_route: context.requested_route,
                resolved_route: done.resolved_route,
                requested_route_config: context.requested_route_config,
                resolved_route_config: done.resolved_route_config,
                grant: seats[idx].2.label().to_string(),
                attempt_index: context.attempt_index,
                parent_trace_ids: context.parent_trace_ids,
                retry_of_trace_id: context.retry_of_trace_id,
                feedback_sha256: context.feedback_sha256,
                system: context.system,
                prompt: context.prompt,
                output,
                termination,
                hops: done.hops,
                elapsed_ms: done.elapsed_ms,
                stall_retries: done.stall_retries,
                started_at: done.started_at,
                finished_at: Some(done.finished_at),
                inputs_ready_at: Some(context.inputs_ready_at),
                duration_ms: done
                    .started_at
                    .map(|s| (done.finished_at.monotonic_ns - s.monotonic_ns) as f64 / 1_000_000.0),
                cancel_requested_at: cancel_requested_at
                    .filter(|t| t.monotonic_ns <= done.finished_at.monotonic_ns),
                stopped_at: cancel_requested_at
                    .filter(|t| t.monotonic_ns <= done.finished_at.monotonic_ns)
                    .map(|_| done.finished_at),
                rollout_id: done.rollout_id,
                control_credit: Vec::new(),
                reward: None,
                reward_source: None,
                eligibility: GraphTraceEligibilityV1 {
                    training_eligible: false,
                    reasons: vec!["pending_episode_seal".to_string()],
                },
            });
            last_trace_index[idx] = Some(trace_idx);
            if stale_landing {
                phase[idx] = GraphNodePhase::Pending;
                elapsed_ms[idx] = 0;
                lease_ids[idx] = None;
                gate_note[idx] = None;
                push_event(
                    &mut events,
                    format!(
                        "↻ {} discarded stale generation {} (current {})",
                        node.id, landing_generation, causal_generation[idx]
                    ),
                );
            } else {
                match done.result {
                    Ok(text) => {
                        let gate_verdict = node
                            .gate
                            .as_ref()
                            .map(|gate| evaluate_gate(gate, &text))
                            .unwrap_or(Ok(()));
                        match gate_verdict {
                            Ok(()) => {
                                phase[idx] = GraphNodePhase::Done;
                                accepted_trace_index[idx] = Some(trace_idx);
                                if node.gate.is_some() {
                                    gate_note[idx] = Some("PASS".to_string());
                                    attach_gate_control_credit(
                                        spec,
                                        &index,
                                        &mut traces,
                                        &last_trace_index,
                                        idx,
                                        true,
                                    );
                                }
                                push_event(
                                    &mut events,
                                    format!("✓ {} ({} chars)", node.id, text.chars().count()),
                                );
                                outputs.insert(node.id.clone(), text);
                            }
                            Err(reason) => {
                                let gate = node.gate.as_ref().expect("gate verdict implies gate");
                                attach_gate_control_credit(
                                    spec,
                                    &index,
                                    &mut traces,
                                    &last_trace_index,
                                    idx,
                                    false,
                                );
                                let can_retry = gate.retry.is_some()
                                    && retries[idx] < gate.max_retries
                                    && winding.is_none();
                                if can_retry {
                                    let target = gate.retry.clone().expect("checked above");
                                    let target_idx = index[target.as_str()];
                                    // Every descendant consumes the retry target's
                                    // causal generation, including sibling branches
                                    // outside the target→gate path. Completed work is
                                    // reset immediately; an in-flight old generation
                                    // remains Running only until its landing can be
                                    // recorded and discarded above.
                                    let affected = descendants_including(spec, target_idx);
                                    // Initial admission already charged each declared
                                    // node once. A retry needs one additional logical
                                    // call only for affected nodes whose previous
                                    // generation actually started. Reserve the whole
                                    // retry wave before invalidating any output.
                                    let retry_calls = affected
                                        .iter()
                                        .filter(|&&i| attempts_started[i] > 0)
                                        .count();
                                    match reserve_descendant_calls(
                                        &descendant_budget,
                                        retry_calls,
                                        &format!("agent graph retry {} -> {}", node.id, target),
                                    ) {
                                        Ok(retry_receipt) => {
                                            retries[idx] += 1;
                                            gate_note[idx] = Some(format!(
                                                "FAIL → retry {} ({}/{})",
                                                target, retries[idx], gate.max_retries
                                            ));
                                            push_event(
                                                &mut events,
                                                format!(
                                                    "✗ gate {} · {} → retry {} ({}/{}); {}",
                                                    node.id,
                                                    reason,
                                                    target,
                                                    retries[idx],
                                                    gate.max_retries,
                                                    retry_receipt.fields(),
                                                ),
                                            );
                                            feedback.insert(target.clone(), text);
                                            for i in affected {
                                                causal_generation[i] =
                                                    causal_generation[i].saturating_add(1);
                                                if i == idx || phase[i] != GraphNodePhase::Running {
                                                    phase[i] = GraphNodePhase::Pending;
                                                    lease_ids[i] = None;
                                                    elapsed_ms[i] = 0;
                                                }
                                                outputs.remove(&spec.nodes[i].id);
                                                accepted_trace_index[i] = None;
                                                if i != idx {
                                                    gate_note[i] = None;
                                                }
                                                if i != target_idx {
                                                    feedback.remove(&spec.nodes[i].id);
                                                }
                                            }
                                        }
                                        Err(budget_error) => {
                                            phase[idx] = GraphNodePhase::Failed;
                                            gate_note[idx] = Some(format!(
                                                "FAIL · retry budget denied · {reason}"
                                            ));
                                            winding = Some(format!(
                                                "gate '{}' retry denied: {budget_error}",
                                                node.id
                                            ));
                                            seat_cancel.store(true, Ordering::Relaxed);
                                            wind_started = Some(Instant::now());
                                            push_event(
                                                &mut events,
                                                format!(
                                                    "✗ gate {} · retry denied before dispatch · {}",
                                                    node.id, budget_error
                                                ),
                                            );
                                        }
                                    }
                                } else {
                                    phase[idx] = GraphNodePhase::Failed;
                                    gate_note[idx] = Some(format!("FAIL · {reason}"));
                                    if winding.is_none() {
                                        winding = Some(format!(
                                            "gate '{}' rejected after {} retr{}: {reason}",
                                            node.id,
                                            retries[idx],
                                            if retries[idx] == 1 { "y" } else { "ies" }
                                        ));
                                        seat_cancel.store(true, Ordering::Relaxed);
                                        wind_started = Some(Instant::now());
                                    }
                                    push_event(
                                        &mut events,
                                        format!("✗ gate {} · {}", node.id, reason),
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        phase[idx] = GraphNodePhase::Failed;
                        push_event(&mut events, format!("✗ {} · {e}", node.id));
                        if winding.is_none() {
                            winding = Some(format!("node '{}' failed: {e}", node.id));
                            seat_cancel.store(true, Ordering::Relaxed);
                            wind_started = Some(Instant::now());
                        }
                    }
                }
            }
            if winding.is_some() {
                for p in phase.iter_mut() {
                    if *p == GraphNodePhase::Pending {
                        *p = GraphNodePhase::Skipped;
                    }
                }
            }
            publish_graph_snapshot(snapshot(
                &phase,
                &elapsed_ms,
                &retries,
                &gate_note,
                &lease_ids,
                &outputs,
                &events,
                GraphRunPhase::Running,
                None,
                winding.as_deref(),
            ));
        }
        for (idx, context) in inflight.iter_mut().enumerate() {
            let Some(context) = context.take() else {
                continue;
            };
            let reason = winding
                .as_deref()
                .unwrap_or("graph result channel closed before the seat landed");
            let trace_idx = traces.len();
            let termination = if cancelled {
                GraphTraceTerminationV1::failure(GraphTraceTerminationKind::Interrupt, reason)
            } else if graph_deadline_reached {
                GraphTraceTerminationV1::failure(GraphTraceTerminationKind::Deadline, reason)
            } else {
                GraphTraceTerminationV1::failure(GraphTraceTerminationKind::Abandoned, reason)
            };
            traces.push(GraphTraceV1 {
                wall_remaining_ms: context.wall_remaining_ms,
                schema: GRAPH_TRACE_SCHEMA.to_string(),
                trace_id: context.trace_id,
                role: spec.nodes[idx].id.clone(),
                persona: spec.nodes[idx].persona.clone(),
                trainable: spec.nodes[idx].trainable,
                resolved_route: context.requested_route.clone(),
                requested_route: context.requested_route,
                resolved_route_config: context.requested_route_config.clone(),
                requested_route_config: context.requested_route_config,
                grant: seats[idx].2.label().to_string(),
                attempt_index: context.attempt_index,
                parent_trace_ids: context.parent_trace_ids,
                retry_of_trace_id: context.retry_of_trace_id,
                feedback_sha256: context.feedback_sha256,
                system: context.system,
                prompt: context.prompt,
                output: None,
                termination,
                hops: 0,
                elapsed_ms: started.elapsed().as_millis(),
                stall_retries: 0,
                started_at: *context.timing.lock().unwrap_or_else(|e| e.into_inner()),
                finished_at: None,
                inputs_ready_at: Some(context.inputs_ready_at),
                duration_ms: None,
                cancel_requested_at,
                // A detached seat is not evidence of a stopped worker.
                stopped_at: None,
                rollout_id: None,
                control_credit: Vec::new(),
                reward: None,
                reward_source: None,
                eligibility: GraphTraceEligibilityV1 {
                    training_eligible: false,
                    reasons: vec!["pending_episode_seal".to_string()],
                },
            });
            last_trace_index[idx] = Some(trace_idx);
            if phase[idx] == GraphNodePhase::Running {
                phase[idx] = GraphNodePhase::Failed;
            }
            if winding.is_none() {
                winding = Some(reason.to_string());
            }
        }
        crate::ui::viz::agentviz::clear();

        if winding.is_none() {
            let fanin = graph_fanin_truth(spec, &phase, &outputs);
            if !fanin.finished_with_result {
                winding = Some(classify_graph_incomplete(
                    spec, &phase, &outputs, cancelled, None,
                ));
            }
        }

        let run_phase = match (&winding, cancelled) {
            (Some(_), true) => GraphRunPhase::Cancelled,
            (Some(_), false) => GraphRunPhase::Failed,
            (None, _) => GraphRunPhase::Done,
        };
        let answer = if run_phase == GraphRunPhase::Done {
            sink_answer(spec, &outputs)
        } else {
            String::new()
        };
        for trace in &mut traces {
            trace.eligibility = trace.decide_eligibility();
        }
        let workspace_state_sha256 = workspace_evidence_sha256(&self.workspace);
        let workspace_state_source = if workspace_state_sha256.is_some() {
            "git-workspace-evidence/v1"
        } else {
            "unavailable"
        }
        .to_string();
        let episode = GraphEpisodeV1 {
            schema: GRAPH_EPISODE_SCHEMA.to_string(),
            episode_id,
            graph_name: spec.name.clone(),
            graph_spec_sha256,
            task_sha256,
            workspace_key_sha256,
            workspace_state_sha256,
            workspace_state_source,
            runner_version: runtime.runner_version.clone(),
            cockpit_source_sha256: runtime.cockpit_source_sha256.clone(),
            runtime,
            started_ms,
            sealed_ms: unix_ms().max(started_ms),
            incomplete_by_budget: if traces
                .iter()
                .any(|trace| trace.termination.kind == GraphTraceTerminationKind::BudgetExhausted)
            {
                spec.nodes
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| phase[*i] != GraphNodePhase::Done)
                    .map(|(_, node)| node.id.clone())
                    .collect()
            } else {
                Vec::new()
            },
            incomplete_reason: winding
                .as_ref()
                .map(|reason| graph_incomplete_reason_label(reason, cancelled)),
            fanin: graph_fanin_truth(spec, &phase, &outputs),
            token_allocation: token_allocation.as_ref().map(|budget| budget.snapshot()),
            traces,
            termination: GraphEpisodeTerminationV1 {
                kind: match run_phase {
                    GraphRunPhase::Done => GraphEpisodeTerminationKind::Completed,
                    GraphRunPhase::Cancelled => GraphEpisodeTerminationKind::OperatorCancelled,
                    GraphRunPhase::Failed if graph_deadline_reached => {
                        GraphEpisodeTerminationKind::Deadline
                    }
                    GraphRunPhase::Failed | GraphRunPhase::Running
                        if winding.as_deref().is_some_and(|r| {
                            r.contains("planner produced no")
                                || r.contains("no schedulable")
                                || r.contains("terminal fan-in")
                        }) =>
                    {
                        GraphEpisodeTerminationKind::Incomplete
                    }
                    GraphRunPhase::Failed | GraphRunPhase::Running => {
                        GraphEpisodeTerminationKind::Failed
                    }
                },
                phase: run_phase,
                detail_sha256: winding
                    .as_deref()
                    .map(|reason| crate::knowledge::cut::sha256_hex(reason.as_bytes())),
                final_output_sha256: (!answer.is_empty())
                    .then(|| crate::knowledge::cut::sha256_hex(answer.as_bytes())),
            },
            eligible_trace_count: 0,
            receipt_sha256: String::new(),
        }
        .seal()?;
        let episode_persistence = if self.persist {
            match persist_episode_receipt(&self.workspace, &episode) {
                Ok(()) => {
                    push_event(
                        &mut events,
                        format!("episode {} receipt persisted", &episode.episode_id[..12]),
                    );
                    GraphEpisodePersistence::Persisted
                }
                Err(error) => {
                    push_event(
                        &mut events,
                        format!("episode receipt persistence failed: {error}"),
                    );
                    GraphEpisodePersistence::Failed(error)
                }
            }
        } else {
            GraphEpisodePersistence::Disabled
        };
        let final_snapshot = snapshot(
            &phase,
            &elapsed_ms,
            &retries,
            &gate_note,
            &lease_ids,
            &outputs,
            &events,
            run_phase,
            (!answer.is_empty()).then_some(answer.as_str()),
            winding.as_deref(),
        );
        publish_graph_snapshot(final_snapshot.clone());
        let budget_status = descendant_budget_status(&descendant_budget)?;
        match winding {
            Some(reason) => Err(GraphRunError::Run(Box::new(GraphRunFailure {
                reason: format!("graph '{}' did not complete: {reason}", spec.name),
                snapshot: final_snapshot,
                episode,
                episode_persistence,
                descendant_budget: budget_status,
            }))),
            None => Ok(GraphRunOutcome {
                answer,
                snapshot: final_snapshot,
                episode,
                episode_persistence,
                descendant_budget: budget_status,
            }),
        }
    }
}

/// Sinks (nodes nothing depends on) are the graph's answer.
fn sink_answer(spec: &GraphSpec, outputs: &HashMap<String, String>) -> String {
    let sinks: Vec<&GraphNodeSpec> = spec
        .nodes
        .iter()
        .filter(|node| {
            !spec
                .nodes
                .iter()
                .any(|other| other.depends_on.iter().any(|d| d == &node.id))
        })
        .collect();
    match sinks.as_slice() {
        [only] => outputs.get(&only.id).cloned().unwrap_or_default(),
        many => many
            .iter()
            .filter_map(|node| {
                outputs
                    .get(&node.id)
                    .map(|out| format!("## {}\n{}", node.id, out))
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
    }
}

fn graph_sinks(spec: &GraphSpec) -> Vec<&GraphNodeSpec> {
    spec.nodes
        .iter()
        .filter(|node| {
            !spec
                .nodes
                .iter()
                .any(|other| other.depends_on.iter().any(|d| d == &node.id))
        })
        .collect()
}

fn graph_fanin_truth(
    spec: &GraphSpec,
    phase: &[GraphNodePhase],
    outputs: &HashMap<String, String>,
) -> GraphFaninTruth {
    let sinks = graph_sinks(spec);
    let mut result = String::new();
    let mut finished = !sinks.is_empty();
    for sink in &sinks {
        let idx = spec
            .nodes
            .iter()
            .position(|n| n.id == sink.id)
            .expect("sink is a declared node");
        let body = outputs.get(&sink.id).map(String::as_str).unwrap_or("");
        if phase[idx] != GraphNodePhase::Done || body.trim().is_empty() {
            finished = false;
        } else {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(body);
        }
    }
    GraphFaninTruth {
        finished_with_result: finished,
        result: finished.then_some(result),
        reason: (!finished).then(|| classify_graph_incomplete(spec, phase, outputs, false, None)),
    }
}

fn graph_incomplete_reason_label(reason: &str, cancelled: bool) -> String {
    if cancelled || reason.contains("cancelled") {
        return "cancelled".to_string();
    }
    // A recoverable stream death spells itself two ways now (a bare
    // `stream stalled` when nothing was produced, the incomplete-stream class
    // once partial output exists); both are the same provider stall in a
    // receipt.
    if is_recoverable_stream_error(reason) {
        if let Some(node) = reason
            .split("node '")
            .nth(1)
            .and_then(|s| s.split('\'').next())
        {
            return format!("provider_stall:{node}");
        }
        return "provider_stall".to_string();
    }
    if let Some(rest) = reason.strip_prefix("node '")
        && let Some(node) = rest.split('\'').next()
    {
        return format!("node_failed:{node}");
    }
    if reason.contains("no schedulable") {
        return "no_schedulable_nodes".to_string();
    }
    if reason.contains("planner") || reason.contains("no plan") {
        return "planner_no_plan".to_string();
    }
    reason.to_string()
}

fn classify_graph_incomplete(
    spec: &GraphSpec,
    phase: &[GraphNodePhase],
    _outputs: &HashMap<String, String>,
    cancelled: bool,
    winding: Option<&str>,
) -> String {
    if cancelled {
        return "cancelled by operator".to_string();
    }
    if let Some(reason) = winding {
        return reason.to_string();
    }
    let started = phase
        .iter()
        .filter(|p| !matches!(p, GraphNodePhase::Pending | GraphNodePhase::Skipped))
        .count();
    if started == 0 {
        return "no schedulable nodes".to_string();
    }
    let planner = spec.nodes.iter().position(|n| n.id == "planner");
    if planner.is_some_and(|i| phase[i] == GraphNodePhase::Done) && started == 1 {
        return "planner produced no schedulable plan".to_string();
    }
    "terminal fan-in did not finish with a result".to_string()
}

fn node_system_prompt(
    graph_name: &str,
    node: &GraphNodeSpec,
    persona_body: &str,
    grant: Grant,
) -> String {
    let mut s = format!(
        "You are node '{}' of the '{}' agent graph — specialized agents wired as a \
         graph; edges route work between nodes and shared state flows along them. \
         Work only your node's brief: upstream results arrive in your prompt, and \
         your output becomes upstream context for the nodes that depend on you. \
         Return your best complete result as plain text.",
        node.id, graph_name
    );
    if !persona_body.trim().is_empty() {
        s.push_str("\n\nYour persona — inhabit it fully:\n");
        s.push_str(persona_body.trim());
    }
    if node.gate.as_ref().is_some_and(|gate| gate.verdict) {
        s.push_str(
            "\n\nYou are a gate node: end your reply with exactly one final line — \
             `VERDICT: PASS` or `VERDICT: FAIL — <specific reasons>`.",
        );
    }
    if grant == Grant::None {
        s.push_str("\n\nYou have no tools this run: answer from reasoning alone.");
    } else {
        // A tool-bearing node gets the shared hop-efficiency advisory. The grant
        // decides which tools exist, so the text stays tool-agnostic.
        s.push_str("\n\n");
        s.push_str(super::TOOL_BATCHING_HINT);
    }
    s
}

fn render_node_prompt(
    node: &GraphNodeSpec,
    task: &str,
    outputs: &HashMap<String, String>,
    feedback: &HashMap<String, String>,
    dependency_context_chars: usize,
) -> String {
    // The cap is a TOTAL upstream body budget for this node, not a multiplier
    // applied independently to every dependency. Receipt metadata remains
    // visible for every edge even when a large fan-in receives no body excerpt.
    let dep_count = node.depends_on.len();
    let base_share = dependency_context_chars.checked_div(dep_count).unwrap_or(0);
    let extra_shares = dependency_context_chars.checked_rem(dep_count).unwrap_or(0);
    let mut receipts: BTreeMap<String, String> = BTreeMap::new();
    let mut appended: Vec<&String> = Vec::new();
    for (position, dep) in node.depends_on.iter().enumerate() {
        let placeholder = format!("{{output:{dep}}}");
        let occurrences = node.prompt.matches(&placeholder).count();
        let dep_share = base_share + usize::from(position < extra_shares);
        // Repeated authored references receive the same envelope while their
        // repeated body excerpts still stay inside this dependency's share.
        let body_cap = dep_share.checked_div(occurrences.max(1)).unwrap_or(0);
        receipts.insert(
            dep.clone(),
            graph_result_envelope(dep, outputs.get(dep).map(String::as_str), body_cap),
        );
        if occurrences == 0 {
            appended.push(dep);
        }
    }
    // Expand only placeholders present in the authored template. Task text or
    // an upstream model result may contain placeholder-shaped bytes, but those
    // are evidence and cannot trigger a second substitution pass.
    let mut prompt = render_graph_template(&node.prompt, task, &receipts);
    if !appended.is_empty() {
        prompt.push_str("\n\n## Upstream results\n");
        for dep in appended {
            let receipt = receipts
                .get(dep)
                .expect("every declared dependency has a result envelope");
            prompt.push_str(&format!("### {dep}\n{receipt}\n"));
        }
    }
    if let Some(fb) = feedback.get(&node.id) {
        prompt.push_str(
            "\n\n## Reviewer feedback on your previous attempt (it was rejected — address every point)\n",
        );
        // Reviewer feedback is a separate loop-control input with the same
        // explicit cap; it does not multiply with dependency count.
        prompt.push_str(&head_chars(fb, dependency_context_chars));
    }
    prompt
}

fn render_graph_template(
    template: &str,
    task: &str,
    receipts: &BTreeMap<String, String>,
) -> String {
    let mut rendered = String::with_capacity(template.len().saturating_add(task.len()));
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        rendered.push_str(&rest[..open]);
        rest = &rest[open..];
        if let Some(tail) = rest.strip_prefix("{task}") {
            rendered.push_str(task);
            rest = tail;
            continue;
        }
        if let Some(tail) = rest.strip_prefix("{output:")
            && let Some(close) = tail.find('}')
        {
            let dep = &tail[..close];
            if let Some(receipt) = receipts.get(dep) {
                rendered.push_str(receipt);
                rest = &tail[close + 1..];
                continue;
            }
        }
        // Unknown braces are ordinary authored text. `{` is one ASCII byte,
        // so advancing by one preserves UTF-8 boundaries.
        rendered.push('{');
        rest = &rest[1..];
    }
    rendered.push_str(rest);
    rendered
}

fn graph_result_envelope(source: &str, output: Option<&str>, body_cap: usize) -> String {
    let (status, body) = match output {
        Some(body) => ("done", body),
        None => ("missing", ""),
    };
    let chars = body.chars().count();
    let excerpt_chars = chars.min(body_cap);
    let omitted_chars = chars.saturating_sub(excerpt_chars);
    let source_json = serde_json::to_string(source).expect("graph node id serializes as JSON");
    let mut receipt = format!(
        "[angel-agent-graph-result/v1 source={source_json} status={status} chars={chars} sha256={} excerpt_chars={excerpt_chars} omitted_chars={omitted_chars}]",
        crate::knowledge::cut::sha256_hex(body.as_bytes())
    );
    if excerpt_chars > 0 {
        receipt.push('\n');
        receipt.extend(body.chars().take(excerpt_chars));
    }
    if omitted_chars > 0 {
        receipt.push_str("\n…[result body truncated]");
    }
    receipt
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ParsedGraphGateVerdict<'a> {
    Pass,
    Fail { reason: &'a str },
}

fn parse_graph_gate_verdict(value: &str) -> Result<ParsedGraphGateVerdict<'_>, String> {
    let value = value.trim();
    let token_end = value.find(char::is_whitespace).unwrap_or(value.len());
    let (token, trailing) = value.split_at(token_end);
    let trailing = trailing.trim();

    if token.eq_ignore_ascii_case("PASS") {
        if trailing.is_empty() {
            return Ok(ParsedGraphGateVerdict::Pass);
        }
        return Err("malformed gate verdict: PASS must be the complete value".to_string());
    }
    if token.eq_ignore_ascii_case("FAIL") {
        return Ok(ParsedGraphGateVerdict::Fail { reason: trailing });
    }

    let shown = if token.is_empty() { "<empty>" } else { token };
    Err(format!(
        "malformed gate verdict token '{shown}'; expected PASS or FAIL"
    ))
}

fn evaluate_gate(gate: &GraphGateSpec, output: &str) -> Result<(), String> {
    if gate.verdict {
        for line in output.lines().rev() {
            let line = line.trim();
            let Some(prefix) = line.get(.."VERDICT:".len()) else {
                continue;
            };
            if !prefix.eq_ignore_ascii_case("VERDICT:") {
                continue;
            }
            return match parse_graph_gate_verdict(&line["VERDICT:".len()..])? {
                ParsedGraphGateVerdict::Pass => Ok(()),
                ParsedGraphGateVerdict::Fail { reason } => {
                    if reason.is_empty() {
                        Err("verdict FAIL".to_string())
                    } else {
                        Err(format!("verdict FAIL ({reason})"))
                    }
                }
            };
        }
        return Err("no `VERDICT: PASS|FAIL` line in gate output".to_string());
    }
    if let Some(expect) = &gate.expect {
        if output.contains(expect.as_str()) {
            return Ok(());
        }
        return Err(format!("expected substring '{expect}' missing"));
    }
    Ok(())
}

fn head_chars(s: &str, cap: usize) -> String {
    if s.chars().count() <= cap {
        return s.to_string();
    }
    const SUFFIX: &str = "\n…[truncated]";
    let suffix_chars = SUFFIX.chars().count();
    if cap <= suffix_chars {
        return SUFFIX.chars().take(cap).collect();
    }
    let head: String = s.chars().take(cap - suffix_chars).collect();
    format!("{head}{SUFFIX}")
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn graph_trace_id(episode_id: &str, role: &str, attempt_index: u32) -> String {
    crate::knowledge::cut::sha256_hex(format!("{episode_id}:{role}:{attempt_index}").as_bytes())
}

fn graph_dependency_distance(
    spec: &GraphSpec,
    index: &HashMap<&str, usize>,
    ancestor_idx: usize,
    descendant_idx: usize,
) -> u32 {
    let mut frontier = vec![(ancestor_idx, 0u32)];
    let mut seen = HashSet::new();
    while let Some((current, distance)) = frontier.pop() {
        if current == descendant_idx {
            return distance;
        }
        if !seen.insert(current) {
            continue;
        }
        let current_id = spec.nodes[current].id.as_str();
        for node in &spec.nodes {
            if node.depends_on.iter().any(|dep| dep == current_id) {
                frontier.push((index[node.id.as_str()], distance.saturating_add(1)));
            }
        }
    }
    0
}

fn attach_gate_control_credit(
    spec: &GraphSpec,
    index: &HashMap<&str, usize>,
    traces: &mut [GraphTraceV1],
    last_trace_index: &[Option<usize>],
    evaluator_idx: usize,
    accepted: bool,
) {
    let Some(gate) = spec.nodes[evaluator_idx].gate.as_ref() else {
        return;
    };
    let Some(target) = gate.credit_to.as_ref().or(gate.retry.as_ref()) else {
        return;
    };
    let target_idx = index[target.as_str()];
    let (Some(target_trace_idx), Some(evaluator_trace_idx)) = (
        last_trace_index[target_idx],
        last_trace_index[evaluator_idx],
    ) else {
        return;
    };
    let evaluator = &traces[evaluator_trace_idx];
    let Some(evaluator_output_sha256) = evaluator
        .output
        .as_ref()
        .map(|output| output.sha256.clone())
    else {
        return;
    };
    let signal = GraphControlCreditV1 {
        contract: GRAPH_GATE_CONTROL_CONTRACT.to_string(),
        evaluator_role: evaluator.role.clone(),
        evaluator_trace_id: evaluator.trace_id.clone(),
        evaluator_output_sha256,
        credit_distance: graph_dependency_distance(spec, index, target_idx, evaluator_idx),
        outcome: if accepted { "accepted" } else { "rejected" }.to_string(),
    };
    traces[target_trace_idx].control_credit.push(signal);
}

/// Keep this deliberately small and conservative. A hit suppresses even the
/// preview; exact bytes remain represented only by their digest.
fn contains_likely_graph_secret(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if ["sk-", "ghp_", "github_pat_", "xoxb-", "xoxp-"]
        .iter()
        .any(|prefix| lower.contains(prefix))
    {
        return true;
    }
    let tokens = text.split_whitespace().collect::<Vec<_>>();
    for (position, token) in tokens.iter().enumerate() {
        let lower_token = token.to_ascii_lowercase();
        if let Some(scheme) = lower_token.find("://") {
            let rest = &lower_token[scheme + 3..];
            if rest.find('@').is_some_and(|at| !rest[..at].contains('/')) {
                return true;
            }
        }
        if let Some(separator) = token.find(['=', ':']) {
            let name = token[..separator].to_ascii_lowercase();
            let value = &token[separator + 1..];
            if !value.is_empty()
                && ["key", "token", "secret", "auth", "password", "passwd"]
                    .iter()
                    .any(|marker| name.contains(marker))
            {
                return true;
            }
        }
        let marker = matches!(
            lower_token.trim_matches(|character: char| character == ',' || character == '"'),
            "authorization"
                | "authorization:"
                | "bearer"
                | "--token"
                | "--api-key"
                | "password"
                | "passwd"
        );
        if marker && tokens.get(position + 1).is_some() {
            return true;
        }
    }
    false
}

/// Resolve + prepare the per-workspace episode store (owner-only dirs).
fn episode_store_dirs(workspace: &Path) -> Result<(PathBuf, PathBuf), String> {
    let dir = std::env::var_os("ANGEL_AGENT_GRAPH_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::platform::workspace_store::angel_subdir("agent-graphs"))
        .join(crate::platform::workspace_store::workspace_key(workspace));
    let episode_dir = dir.join("episodes");
    ensure_private_graph_directory(&dir)?;
    ensure_private_graph_directory(&episode_dir)?;
    Ok((dir, episode_dir))
}

fn persist_episode_receipt(workspace: &Path, episode: &GraphEpisodeV1) -> Result<(), String> {
    episode.validate()?;
    let (_dir, episode_dir) = episode_store_dirs(workspace)?;
    let episode_path = episode_dir.join(format!("{}.json", episode.episode_id));
    let body = crate::platform::secrets::to_redacted_vec_pretty(episode)
        .map_err(|error| format!("encode episode receipt: {error}"))?;
    if episode_path.exists() {
        return verify_existing_episode_receipt(&episode_path, &body);
    }
    let episode_tmp = episode_dir.join(format!(".{}.json.tmp", episode.episode_id));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .private_create_mode()
        .open(&episode_tmp)
        .map_err(|error| {
            format!(
                "create episode receipt temp '{}': {error}",
                episode_tmp.display()
            )
        })?;
    if let Err(error) = file.write_all(&body) {
        drop(file);
        let _ = std::fs::remove_file(&episode_tmp);
        return Err(format!(
            "write episode receipt temp '{}': {error}",
            episode_tmp.display()
        ));
    }
    if let Err(error) = file.sync_all() {
        drop(file);
        let _ = std::fs::remove_file(&episode_tmp);
        return Err(format!(
            "sync episode receipt temp '{}': {error}",
            episode_tmp.display()
        ));
    }
    drop(file);
    match std::fs::hard_link(&episode_tmp, &episode_path) {
        Ok(()) => {
            std::fs::remove_file(&episode_tmp).map_err(|error| {
                format!(
                    "remove published episode temp '{}': {error}",
                    episode_tmp.display()
                )
            })?;
            let directory = std::fs::File::open(&episode_dir).map_err(|error| {
                format!(
                    "open episode receipt directory '{}' for sync: {error}",
                    episode_dir.display()
                )
            })?;
            directory.sync_all().map_err(|error| {
                format!(
                    "sync episode receipt directory '{}': {error}",
                    episode_dir.display()
                )
            })?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = std::fs::remove_file(&episode_tmp);
            verify_existing_episode_receipt(&episode_path, &body)?;
        }
        Err(error) => {
            let _ = std::fs::remove_file(&episode_tmp);
            return Err(format!(
                "publish episode receipt '{}': {error}",
                episode_path.display()
            ));
        }
    }
    Ok(())
}

trait PrivateCreateMode {
    fn private_create_mode(&mut self) -> &mut Self;
}

impl PrivateCreateMode for std::fs::OpenOptions {
    fn private_create_mode(&mut self) -> &mut Self {
        #[cfg(unix)]
        self.mode(0o600);
        self
    }
}

/// Bind an external scorer's reward to one trace of a sealed episode receipt.
///
/// Loads and re-audits the persisted episode (`validate()` re-checks the sealed
/// digest), finds the trace and its captured output, then writes an owner-only
/// reward-binding receipt that records the episode receipt SHA and the trace
/// output SHA it was bound against. Idempotent: re-binding the identical
/// payload returns the existing receipt; a conflicting payload is refused.
///
/// This is the ONLY authority that may attach training reward to a graph
/// trace — the episode receipts themselves remain reward-free by construction.
pub(crate) fn bind_episode_reward(
    workspace: &Path,
    episode_id: &str,
    trace_id: &str,
    reward: serde_json::Value,
    reward_source: &str,
    reward_contract: &str,
) -> Result<GraphRewardBindingV1, String> {
    if reward.is_null() {
        return Err("a reward binding must carry a non-null reward".to_string());
    }
    let reward_bytes =
        serde_json::to_vec(&reward).map_err(|error| format!("encode reward payload: {error}"))?;
    if reward_bytes.len() > MAX_GRAPH_REWARD_BYTES {
        return Err(format!(
            "reward payload is {} bytes; maximum is {MAX_GRAPH_REWARD_BYTES}",
            reward_bytes.len()
        ));
    }
    if reward_source.trim().is_empty() || reward_source.len() > MAX_GRAPH_REWARD_SOURCE_BYTES {
        return Err(format!(
            "reward_source must be 1-{MAX_GRAPH_REWARD_SOURCE_BYTES} bytes"
        ));
    }
    if reward_contract.trim().is_empty() || reward_contract.len() > MAX_GRAPH_REWARD_CONTRACT_BYTES
    {
        return Err(format!(
            "reward_contract must be 1-{MAX_GRAPH_REWARD_CONTRACT_BYTES} bytes"
        ));
    }
    if !is_sha256(episode_id) || !is_sha256(trace_id) {
        return Err("episode_id and trace_id must be sha256 digests".to_string());
    }

    let (_dir, episode_dir) = episode_store_dirs(workspace)?;
    let episode_path = episode_dir.join(format!("{episode_id}.json"));
    let raw = std::fs::read_to_string(&episode_path)
        .map_err(|error| format!("read episode receipt '{}': {error}", episode_path.display()))?;
    let episode: GraphEpisodeV1 = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "decode episode receipt '{}': {error}",
            episode_path.display()
        )
    })?;
    episode.validate()?;
    let trace = episode
        .traces
        .iter()
        .find(|trace| trace.trace_id == trace_id)
        .ok_or_else(|| "trace id is not part of this episode".to_string())?;
    let output = trace
        .output
        .as_ref()
        .ok_or_else(|| "trace has no captured output to bind reward to".to_string())?;

    let binding = GraphRewardBindingV1 {
        schema: GRAPH_REWARD_BINDING_SCHEMA.to_string(),
        episode_id: episode.episode_id.clone(),
        episode_receipt_sha256: episode.receipt_sha256.clone(),
        trace_id: trace.trace_id.clone(),
        trace_output_sha256: output.sha256.clone(),
        reward,
        reward_source: reward_source.trim().to_string(),
        reward_contract: reward_contract.trim().to_string(),
        bound_at_ms: now_ms(),
    };
    let body =
        serde_json::to_vec(&binding).map_err(|error| format!("encode reward binding: {error}"))?;

    let binding_path = episode_dir.join(format!("{episode_id}.{trace_id}.reward.json"));
    if binding_path.exists() {
        // Idempotency is time-stable: the same binding payload (digests,
        // reward, source, contract) re-binds to the existing receipt; a
        // different payload is a conflict and is refused, never overwritten.
        let existing_raw = std::fs::read_to_string(&binding_path)
            .map_err(|error| format!("read existing binding: {error}"))?;
        let existing: GraphRewardBindingV1 = serde_json::from_str(&existing_raw)
            .map_err(|error| format!("decode existing binding: {error}"))?;
        let same = existing.schema == binding.schema
            && existing.episode_id == binding.episode_id
            && existing.episode_receipt_sha256 == binding.episode_receipt_sha256
            && existing.trace_id == binding.trace_id
            && existing.trace_output_sha256 == binding.trace_output_sha256
            && existing.reward == binding.reward
            && existing.reward_source == binding.reward_source
            && existing.reward_contract == binding.reward_contract;
        return if same {
            Ok(existing)
        } else {
            Err("episode trace already bound a different reward; refusing overwrite".to_string())
        };
    }
    let tmp = episode_dir.join(format!(".{episode_id}.{trace_id}.reward.json.tmp"));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .private_create_mode()
        .open(&tmp)
        .map_err(|error| format!("create reward binding temp '{}': {error}", tmp.display()))?;
    if let Err(error) = file.write_all(&body) {
        drop(file);
        let _ = std::fs::remove_file(&tmp);
        return Err(format!(
            "write reward binding temp '{}': {error}",
            tmp.display()
        ));
    }
    if let Err(error) = file.sync_all() {
        drop(file);
        let _ = std::fs::remove_file(&tmp);
        return Err(format!(
            "sync reward binding temp '{}': {error}",
            tmp.display()
        ));
    }
    drop(file);
    std::fs::hard_link(&tmp, &binding_path).map_err(|error| {
        format!(
            "publish reward binding '{}': {error}",
            binding_path.display()
        )
    })?;
    std::fs::remove_file(&tmp)
        .map_err(|error| format!("remove reward binding temp '{}': {error}", tmp.display()))?;
    Ok(binding)
}

fn ensure_private_graph_directory(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|error| {
        format!(
            "create episode receipt directory '{}': {error}",
            path.display()
        )
    })?;
    #[cfg(unix)]
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).map_err(|error| {
        format!(
            "set private episode receipt directory permissions '{}': {error}",
            path.display()
        )
    })?;
    Ok(())
}

fn set_private_graph_file_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(|error| {
        format!(
            "set private episode receipt permissions '{}': {error}",
            path.display()
        )
    })?;
    Ok(())
}

fn verify_existing_episode_receipt(path: &Path, expected: &[u8]) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        format!(
            "inspect existing episode receipt '{}': {error}",
            path.display()
        )
    })?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(format!(
            "existing episode receipt '{}' is not a regular file",
            path.display()
        ));
    }
    set_private_graph_file_permissions(path)?;
    let existing = std::fs::read(path).map_err(|error| {
        format!(
            "read existing episode receipt '{}': {error}",
            path.display()
        )
    })?;
    if existing == expected {
        return Ok(());
    }
    Err(format!(
        "episode receipt id collision at '{}': existing bytes differ; refusing overwrite",
        path.display()
    ))
}

// ---------------------------------------------------------------------------
// The agent-callable tool
// ---------------------------------------------------------------------------

/// `agent_graph` — the in-hand driver routes a task through a declared graph
/// mid-turn. Node seats never see this tool (Grant registries exclude it), so
/// graphs cannot recurse.
pub(crate) struct AgentGraphTool {
    workspace: PathBuf,
    self_club: Option<Arc<dyn Club>>,
    roster: Vec<Arc<dyn Club>>,
    /// Names cached at registry build for the tool description; `call` reloads
    /// specs fresh from disk.
    catalog_line: String,
    cargo: PinnedCargo,
    mutation_targets: Arc<crate::agent::tools::build::MutationTargets>,
}

impl AgentGraphTool {
    pub(crate) fn with_mutation_targets(
        mut self,
        targets: Arc<crate::agent::tools::build::MutationTargets>,
    ) -> Self {
        self.mutation_targets = targets;
        self
    }

    #[cfg(test)]
    pub(crate) fn new(
        workspace: PathBuf,
        self_club: Option<Arc<dyn Club>>,
        roster: Vec<Arc<dyn Club>>,
    ) -> Self {
        let cargo = PinnedCargo::capture(&workspace);
        Self::new_with_cargo(workspace, self_club, roster, cargo)
    }

    pub(crate) fn new_with_cargo(
        workspace: PathBuf,
        self_club: Option<Arc<dyn Club>>,
        roster: Vec<Arc<dyn Club>>,
        cargo: PinnedCargo,
    ) -> Self {
        let graphs = load_graphs();
        let mut names: Vec<String> = graphs
            .iter()
            .filter(|g| g.spec.is_some())
            .map(|g| {
                let desc = g
                    .spec
                    .as_ref()
                    .map(|s| s.description.as_str())
                    .unwrap_or("");
                if desc.is_empty() {
                    g.name.clone()
                } else {
                    format!("{} ({})", g.name, head_chars(desc, 90))
                }
            })
            .collect();
        if names.is_empty() {
            names.push("(none installed)".to_string());
        }
        Self {
            workspace,
            self_club,
            roster,
            catalog_line: names.join("; "),
            cargo,
            mutation_targets: Arc::new(crate::agent::tools::build::MutationTargets::default()),
        }
    }
}

impl Tool for AgentGraphTool {
    fn name(&self) -> &str {
        "agent_graph"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: "agent_graph".to_string(),
            description: format!(
                "Run a declared agent graph: specialized nodes (persona + tool grant) wired \
                 by depends_on edges, with parallel fan-out/fan-in and reviewer gates that \
                 can reject and loop back. Use for work that genuinely splits into \
                 specialties with handoffs; for ad-hoc parallel seats use `spawn`, and for \
                 most tasks just do the work yourself. Installed graphs: {}.",
                self.catalog_line
            ),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "graph": { "type": "string", "description": "graph name from the installed catalog" },
                    "task": { "type": "string", "description": "the task the graph works" }
                },
                "required": ["graph", "task"]
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        self.call_with_cancel(args, None)
    }

    fn call_with_cancel(
        &self,
        args: &Value,
        cancel: Option<&AtomicBool>,
    ) -> Result<String, String> {
        let _budget_scope = DescendantBudgetScope::enter_root();
        // Validate the configured root handle before catalog/execution work;
        // `engine.run` inherits this same scope.
        let _ = current_descendant_budget()?;
        let graph = args
            .get("graph")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or("agent_graph needs graph: the graph name")?;
        let task = args
            .get("task")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or("agent_graph needs task: the task to run")?;
        let catalog = load_graphs();
        let spec = find_graph(&catalog, graph)?;
        // Inspect the already loaded spec, not a second catalog read. Only
        // code grants can mutate this same workspace; reviewer/research nodes
        // keep their enforced ceiling and do not invalidate source inventory.
        if graph_has_workspace_writes(spec)? {
            self.mutation_targets.mark_opaque();
        }

        let engine = AgentGraphEngine::new_with_cargo(
            self.workspace.clone(),
            self.self_club.clone(),
            self.roster.clone(),
            self.cargo.clone(),
        );
        let started = Instant::now();
        let never = AtomicBool::new(false);
        let outcome = match engine.run(spec, task, cancel.unwrap_or(&never)) {
            Ok(outcome) => outcome,
            Err(GraphRunError::Preflight(message)) => return Err(message),
            Err(GraphRunError::Run(failure)) => {
                return Err(render_graph_run_failure(spec, &failure, started.elapsed()));
            }
        };
        let header = graph_receipt_header(
            spec,
            &outcome.snapshot,
            &outcome.episode,
            &outcome.episode_persistence,
            outcome.descendant_budget,
            started.elapsed(),
        );
        let min = subcall_offload_min_bytes();
        if let Some(receipt) = maybe_offload_root_body(
            &outcome.answer,
            HandleKind::Subcall,
            "agent_graph",
            &format!("agent_graph|{}", spec.name),
            min,
        ) {
            Ok(format!("{header}\n{receipt}"))
        } else {
            Ok(format!("{header}\n\n{}", outcome.answer))
        }
    }
}

/// Find a runnable spec by name; a miss speaks the whole catalog, including
/// broken files and why they are broken.
pub(crate) fn find_graph<'a>(
    catalog: &'a [LoadedGraph],
    name: &str,
) -> Result<&'a GraphSpec, String> {
    if let Some(found) = catalog.iter().find(|g| g.name.eq_ignore_ascii_case(name)) {
        return match (&found.spec, &found.error) {
            (Some(spec), _) => Ok(spec),
            (None, Some(error)) => Err(format!(
                "graph '{}' is installed but broken ({}): {error}",
                found.name,
                found.source.display()
            )),
            (None, None) => Err(format!("graph '{}' has no spec", found.name)),
        };
    }
    let mut lines: Vec<String> = Vec::new();
    for g in catalog {
        match &g.error {
            None => lines.push(g.name.clone()),
            Some(error) => lines.push(format!("{} (BROKEN: {error})", g.name)),
        }
    }
    Err(format!(
        "unknown graph '{name}'. Installed: {}",
        if lines.is_empty() {
            "(none — add TOML specs to ~/.angel0/graphs)".to_string()
        } else {
            lines.join(", ")
        }
    ))
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/agent_graph__tests.rs"]
mod tests;
