use crate::club::RouteIdentity;
use serde::{Deserialize, Serialize};

pub(crate) const ROLLOUT_SCHEMA: &str = "angel-harness-rollout/v1";
pub(crate) const JOURNAL_SCHEMA: &str = "angel-harness-rollout-journal/v1";
pub(crate) const TASK_ROLLOUT_BINDING_SCHEMA: &str = "angel-task-rollout-binding/v1";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CaptureMode {
    #[default]
    Off,
    Shadow,
    Local,
}

impl CaptureMode {
    pub(crate) fn from_setting(value: Option<&str>) -> Self {
        match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
            Some("shadow") => Self::Shadow,
            Some("local") => Self::Local,
            _ => Self::Off,
        }
    }

    pub(crate) fn from_env() -> Self {
        Self::from_setting(std::env::var("ANGEL_HARNESS_ROLLOUTS").ok().as_deref())
    }

    pub(crate) fn stores_bodies(self) -> bool {
        self == Self::Local
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProjectIdentity {
    pub(crate) workspace_key: String,
    pub(crate) repo_key: String,
    pub(crate) canonical_root_sha256: String,
}

/// Body-free identity joining one headless task invocation to its rollout.
///
/// The operator prompt is never copied here. Its digest, the canonical runtime
/// configuration digest, and the caller's task/run identities are sealed into
/// the first journal event before any provider request is made.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TaskRolloutBindingV1 {
    pub(crate) schema: String,
    pub(crate) task_id: Option<String>,
    pub(crate) run_id: Option<String>,
    pub(crate) prompt_sha256: String,
    pub(crate) runtime_config_sha256: String,
    pub(crate) runner_version: String,
    pub(crate) cockpit_source_sha256: String,
    pub(crate) binding_sha256: String,
}

impl TaskRolloutBindingV1 {
    pub(crate) fn new(
        task_id: Option<String>,
        run_id: Option<String>,
        prompt_sha256: String,
        runtime_config_sha256: String,
        runner_version: String,
        cockpit_source_sha256: String,
    ) -> Self {
        let mut binding = Self {
            schema: TASK_ROLLOUT_BINDING_SCHEMA.to_string(),
            task_id,
            run_id,
            prompt_sha256,
            runtime_config_sha256,
            runner_version,
            cockpit_source_sha256,
            binding_sha256: String::new(),
        };
        binding.binding_sha256 = binding.canonical_sha256();
        binding
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.schema != TASK_ROLLOUT_BINDING_SCHEMA {
            return Err("unknown task rollout binding schema".to_string());
        }
        for (label, value) in [("task_id", &self.task_id), ("run_id", &self.run_id)] {
            if let Some(value) = value {
                let portable = !value.is_empty()
                    && value.len() <= 256
                    && value.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric()
                            || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/')
                    });
                if !portable {
                    return Err(format!("task rollout {label} is not a portable identity"));
                }
            }
        }
        if !is_sha256(&self.prompt_sha256)
            || !is_sha256(&self.runtime_config_sha256)
            || !is_sha256(&self.binding_sha256)
        {
            return Err("task rollout binding contains an invalid digest".to_string());
        }
        if self.runner_version.is_empty() || self.runner_version.len() > 64 {
            return Err("task rollout binding has an invalid runner version".to_string());
        }
        if self.cockpit_source_sha256 != "unbound" && !is_sha256(&self.cockpit_source_sha256) {
            return Err("task rollout binding has an invalid cockpit source digest".to_string());
        }
        if self.binding_sha256 != self.canonical_sha256() {
            return Err("task rollout binding digest mismatch".to_string());
        }
        Ok(())
    }

    fn canonical_sha256(&self) -> String {
        let canonical = serde_json::json!({
            "schema": self.schema,
            "task_id": self.task_id,
            "run_id": self.run_id,
            "prompt_sha256": self.prompt_sha256,
            "runtime_config_sha256": self.runtime_config_sha256,
            "runner_version": self.runner_version,
            "cockpit_source_sha256": self.cockpit_source_sha256,
        });
        crate::cut::sha256_hex(
            &serde_json::to_vec(&canonical).expect("task rollout binding must serialize"),
        )
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CaptureDescriptor {
    pub(crate) mode: CaptureMode,
    pub(crate) semantic_bodies: bool,
    pub(crate) media_bodies: bool,
    pub(crate) private_reasoning_captured: bool,
    pub(crate) provider_headers_captured: bool,
    pub(crate) recorder_revision: String,
}

impl CaptureDescriptor {
    pub(crate) fn new(mode: CaptureMode) -> Self {
        Self {
            mode,
            semantic_bodies: mode.stores_bodies(),
            media_bodies: false,
            // These sources are deliberately not accepted by the recorder API.
            private_reasoning_captured: false,
            provider_headers_captured: false,
            recorder_revision: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecordedRoute {
    pub(crate) driver: String,
    pub(crate) model_revision: Option<String>,
    pub(crate) reasoning_effort: Option<String>,
}

impl From<RouteIdentity> for RecordedRoute {
    fn from(route: RouteIdentity) -> Self {
        Self {
            driver: route.driver,
            model_revision: route.model,
            reasoning_effort: route.reasoning_effort,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlobStorage {
    Absent,
    Local,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Sensitivity {
    Project,
    SecretRejected,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlobRef {
    pub(crate) sha256: String,
    pub(crate) bytes: u64,
    pub(crate) media_type: String,
    pub(crate) storage: BlobStorage,
    pub(crate) sensitivity: Sensitivity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RecordedRole {
    System,
    User,
    Harness,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecordedToolCall {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) arguments: BlobRef,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MediaRef {
    pub(crate) media_type: String,
    pub(crate) encoded_bytes: u64,
    pub(crate) sha256: String,
    pub(crate) body_stored: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecordedMessage {
    pub(crate) role: RecordedRole,
    pub(crate) content: BlobRef,
    pub(crate) attachments: Vec<MediaRef>,
    pub(crate) tool_calls: Vec<RecordedToolCall>,
    pub(crate) tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) recovery_context: Vec<crate::club::RecoveryContextRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PolicyRequestRef {
    pub(crate) messages: Vec<RecordedMessage>,
    pub(crate) tool_schema_set: BlobRef,
    pub(crate) semantic_sha256: String,
    pub(crate) tool_pairing_valid: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum RecordedAction {
    Text { content: BlobRef },
    ToolCalls { calls: Vec<RecordedToolCall> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PolicyResponseRef {
    pub(crate) action: RecordedAction,
    pub(crate) semantic_sha256: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InfrastructureClass {
    ProviderUnavailable,
    ProviderPartialOutput,
    HarnessCrash,
    Store,
    ProcessLost,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InfrastructureFailure {
    pub(crate) class: InfrastructureClass,
    pub(crate) retryable: bool,
    pub(crate) detail_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum PolicyStepOutcome {
    Pending,
    TextAction,
    ToolCallsAction,
    ProviderFailedNoAction { failure: InfrastructureFailure },
    ProviderFailedAfterPartial { failure: InfrastructureFailure },
}

impl PolicyStepOutcome {
    pub(crate) fn completed_action(&self) -> bool {
        matches!(self, Self::TextAction | Self::ToolCallsAction)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PolicyAttemptV1 {
    pub(crate) step_index: u32,
    pub(crate) attempt_index: u16,
    pub(crate) request: PolicyRequestRef,
    pub(crate) response: Option<PolicyResponseRef>,
    pub(crate) requested_route: RecordedRoute,
    pub(crate) resolved_route: Option<RecordedRoute>,
    pub(crate) outcome: PolicyStepOutcome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TerminationKind {
    Answer,
    OperatorInterrupt,
    Deadline,
    MaxHops,
    PolicyGuard,
    ProviderFailure,
    ProcessLost,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Termination {
    pub(crate) kind: TerminationKind,
    pub(crate) stop_reason: String,
    pub(crate) interrupted: bool,
    pub(crate) deadline_reached: bool,
    pub(crate) max_hops_reached: bool,
    pub(crate) infrastructure: Option<InfrastructureFailure>,
    pub(crate) final_answer_sha256: Option<String>,
}

impl Termination {
    pub(crate) fn from_turn(
        stop_reason: &str,
        interrupted: bool,
        deadline_reached: bool,
        max_hops_reached: bool,
        final_answer: Option<&str>,
    ) -> Self {
        let kind = match stop_reason {
            "answer" => TerminationKind::Answer,
            "interrupt" => TerminationKind::OperatorInterrupt,
            "deadline" => TerminationKind::Deadline,
            "max_hops" => TerminationKind::MaxHops,
            "provider_error" => TerminationKind::ProviderFailure,
            _ => TerminationKind::PolicyGuard,
        };
        let infrastructure =
            (kind == TerminationKind::ProviderFailure).then(|| InfrastructureFailure {
                class: InfrastructureClass::ProviderUnavailable,
                retryable: false,
                detail_sha256: crate::cut::sha256_hex(stop_reason.as_bytes()),
            });
        Self {
            kind,
            stop_reason: stop_reason.to_string(),
            interrupted,
            deadline_reached,
            max_hops_reached,
            infrastructure,
            final_answer_sha256: final_answer
                .map(|answer| crate::cut::sha256_hex(answer.as_bytes())),
        }
    }

    #[allow(dead_code)] // used by the dormant explicit recovery entry point
    pub(crate) fn process_lost() -> Self {
        Self {
            kind: TerminationKind::ProcessLost,
            stop_reason: "process_lost".to_string(),
            interrupted: true,
            deadline_reached: false,
            max_hops_reached: false,
            infrastructure: Some(InfrastructureFailure {
                class: InfrastructureClass::ProcessLost,
                retryable: false,
                detail_sha256: crate::cut::sha256_hex(b"process_lost"),
            }),
            final_answer_sha256: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RewardOwner {
    Cut,
    CodingEval,
    SwarmVerifier,
    CampaignEvaluator,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RewardContract {
    CutTurnVerdictV1,
    CodingEvalV1,
    SwarmVerifierV1,
    CampaignEvaluatorV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RewardEvidenceStorage {
    RolloutBlob,
    External,
}

/// Body-free receipt projected only at the harness tool-execution boundary.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CodingEvalEvidence {
    pub(crate) schema: String,
    pub(crate) task_binding_sha256: String,
    pub(crate) tool: String,
    pub(crate) call_id_sha256: String,
    pub(crate) arguments_sha256: String,
    pub(crate) receipt_sha256: String,
    pub(crate) execution: String,
    pub(crate) verification: String,
}

impl CodingEvalEvidence {
    pub(crate) fn validate(&self, binding: &TaskRolloutBindingV1) -> Result<(), String> {
        binding.validate()?;
        if self.schema != "angel-coding-eval-evidence/v1"
            || self.task_binding_sha256 != binding.binding_sha256
            || !matches!(self.tool.as_str(), "run_tests" | "cargo")
            || self.execution != "succeeded"
            || self.verification != "passed"
            || !is_sha256(&self.call_id_sha256)
            || !is_sha256(&self.arguments_sha256)
            || !is_sha256(&self.receipt_sha256)
        {
            return Err("invalid task verifier reward evidence".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RewardReceipt {
    pub(crate) owner: RewardOwner,
    pub(crate) contract: RewardContract,
    pub(crate) evidence_storage: RewardEvidenceStorage,
    pub(crate) value: f32,
    pub(crate) evaluator_evidence_sha256: String,
}

impl RewardReceipt {
    pub(crate) fn new(
        owner: RewardOwner,
        value: f32,
        evaluator_evidence_sha256: String,
    ) -> Result<Self, String> {
        let contract = match owner {
            RewardOwner::Cut => RewardContract::CutTurnVerdictV1,
            RewardOwner::CodingEval => RewardContract::CodingEvalV1,
            RewardOwner::SwarmVerifier => RewardContract::SwarmVerifierV1,
            RewardOwner::CampaignEvaluator => RewardContract::CampaignEvaluatorV1,
        };
        let evidence_storage = if owner == RewardOwner::Cut {
            RewardEvidenceStorage::RolloutBlob
        } else {
            RewardEvidenceStorage::External
        };
        let receipt = Self {
            owner,
            contract,
            evidence_storage,
            value,
            evaluator_evidence_sha256,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        let contract_matches = matches!(
            (self.owner, self.contract),
            (RewardOwner::Cut, RewardContract::CutTurnVerdictV1)
                | (RewardOwner::CodingEval, RewardContract::CodingEvalV1)
                | (RewardOwner::SwarmVerifier, RewardContract::SwarmVerifierV1)
                | (
                    RewardOwner::CampaignEvaluator,
                    RewardContract::CampaignEvaluatorV1
                )
        );
        let digest_valid = self.evaluator_evidence_sha256.len() == 64
            && self
                .evaluator_evidence_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit());
        let storage_matches = match self.owner {
            RewardOwner::Cut => self.evidence_storage == RewardEvidenceStorage::RolloutBlob,
            RewardOwner::CodingEval => true,
            RewardOwner::SwarmVerifier | RewardOwner::CampaignEvaluator => {
                self.evidence_storage == RewardEvidenceStorage::External
            }
        };
        if !self.value.is_finite() || !digest_valid || !contract_matches || !storage_matches {
            return Err("invalid rollout reward receipt".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RolloutCompatibilityV1 {
    pub(crate) schema: String,
    pub(crate) club_label: String,
    pub(crate) root_trajectory: serde_json::Value,
    pub(crate) harness_treatment: serde_json::Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExclusionReason {
    MissingReward,
    OperatorInterrupt,
    InfrastructureFailure,
    CaptureFailure,
    InvalidVerifierReceipt,
    RewardOwnerConflict,
    MissingSemanticBodies,
    InvalidToolPairing,
    SecretDetected,
    CorruptJournal,
    SchemaMismatch,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum TrainingEligibility {
    Pending,
    Eligible,
    Excluded {
        reason: ExclusionReason,
        detail_sha256: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct EligibilitySignals {
    pub(crate) secret_detected: bool,
    pub(crate) capture_failed: bool,
    pub(crate) reward_owner_conflict: bool,
    pub(crate) semantic_bodies_missing: bool,
    pub(crate) tool_pairing_valid: bool,
}

impl TrainingEligibility {
    pub(crate) fn decide(
        termination: &Termination,
        reward: Option<&RewardReceipt>,
        signals: EligibilitySignals,
    ) -> Self {
        let excluded = |reason, detail: Option<String>| Self::Excluded {
            reason,
            detail_sha256: detail,
        };
        if signals.capture_failed {
            return excluded(ExclusionReason::CaptureFailure, None);
        }
        if signals.secret_detected {
            return excluded(ExclusionReason::SecretDetected, None);
        }
        if !signals.tool_pairing_valid {
            return excluded(ExclusionReason::InvalidToolPairing, None);
        }
        if termination.kind == TerminationKind::OperatorInterrupt {
            return excluded(ExclusionReason::OperatorInterrupt, None);
        }
        if termination.infrastructure.is_some() {
            return excluded(
                ExclusionReason::InfrastructureFailure,
                termination
                    .infrastructure
                    .as_ref()
                    .map(|failure| failure.detail_sha256.clone()),
            );
        }
        if signals.reward_owner_conflict {
            return excluded(ExclusionReason::RewardOwnerConflict, None);
        }
        let Some(reward) = reward else {
            return excluded(ExclusionReason::MissingReward, None);
        };
        if signals.semantic_bodies_missing {
            return excluded(ExclusionReason::MissingSemanticBodies, None);
        }
        if reward.validate().is_err() {
            return excluded(ExclusionReason::InvalidVerifierReceipt, None);
        }
        Self::Eligible
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RolloutStatus {
    Capturing,
    Finalized,
    Incomplete,
    Corrupt,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HarnessRolloutV1 {
    pub(crate) schema: String,
    pub(crate) rollout_id: String,
    pub(crate) project: ProjectIdentity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) task_binding: Option<TaskRolloutBindingV1>,
    pub(crate) capture: CaptureDescriptor,
    pub(crate) started_ms: u64,
    pub(crate) sealed_ms: Option<u64>,
    pub(crate) status: RolloutStatus,
    pub(crate) requested_route: RecordedRoute,
    pub(crate) attempts: Vec<PolicyAttemptV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) auxiliary_coverage: Option<super::super::auxiliary::AuxiliaryCoverage>,
    pub(crate) termination: Option<Termination>,
    pub(crate) reward: Option<RewardReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) compatibility: Option<RolloutCompatibilityV1>,
    pub(crate) eligibility: TrainingEligibility,
    pub(crate) journal_head_sha256: String,
    pub(crate) manifest_sha256: Option<String>,
}

pub(crate) fn validate_attempt_order(attempts: &[PolicyAttemptV1]) -> Result<(), String> {
    for (index, attempt) in attempts.iter().enumerate() {
        if index == 0 {
            if attempt.step_index != 0 || attempt.attempt_index != 0 {
                return Err("rollout attempts must begin at step 0 attempt 0".to_string());
            }
        } else {
            let previous = &attempts[index - 1];
            let retry = attempt.step_index == previous.step_index
                && attempt.attempt_index == previous.attempt_index.saturating_add(1)
                && match &previous.outcome {
                    PolicyStepOutcome::ProviderFailedNoAction { .. } => true,
                    // Explicit transport truncation can retract speculative
                    // prose and retry without committing an action. Ordinary
                    // non-retryable partial failures must still terminate.
                    PolicyStepOutcome::ProviderFailedAfterPartial { failure } => failure.retryable,
                    _ => false,
                };
            let next = attempt.step_index == previous.step_index.saturating_add(1)
                && attempt.attempt_index == 0
                && previous.outcome.completed_action();
            if !retry && !next {
                return Err(format!(
                    "non-contiguous rollout order at step {} attempt {}",
                    attempt.step_index, attempt.attempt_index
                ));
            }
        }
        let response_matches = match &attempt.outcome {
            PolicyStepOutcome::TextAction | PolicyStepOutcome::ToolCallsAction => {
                attempt.response.is_some()
            }
            PolicyStepOutcome::Pending
            | PolicyStepOutcome::ProviderFailedNoAction { .. }
            | PolicyStepOutcome::ProviderFailedAfterPartial { .. } => attempt.response.is_none(),
        };
        if !response_matches {
            return Err("rollout response/outcome mismatch".to_string());
        }
    }
    Ok(())
}
