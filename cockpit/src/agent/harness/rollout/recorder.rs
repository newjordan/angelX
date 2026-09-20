use super::schema::*;
use super::store::*;
use crate::agent::club::{ChatMsg, ChatRole, ClubReply, Media, RouteIdentity, ToolDef};
use std::collections::HashSet;
use std::path::Path;

const CUT_EVIDENCE_SCHEMA: &str = "angel-cut-evidence/v1";

#[derive(Clone, Debug, serde::Serialize)]
struct CutEvidenceObservation {
    ordinal: u32,
    command_sha256: String,
    directory_sha256: String,
    source_sha256: String,
    exit: i64,
    passed: bool,
    duration_ms: u64,
    diagnostic_sha256: Option<String>,
}

#[derive(serde::Serialize)]
struct CutEvidenceManifest<'a> {
    schema: &'static str,
    observation_count: usize,
    observations: &'a [CutEvidenceObservation],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AttemptToken {
    step_index: u32,
    attempt_index: u16,
}

pub(crate) struct RolloutRecorder {
    state: RecorderState,
}

enum RecorderState {
    Off,
    Active(Box<ActiveRecorder>),
}

struct ActiveRecorder {
    store: RolloutStore,
    _lease: RolloutLease,
    cursor: JournalCursor,
    manifest: HarnessRolloutV1,
    next_step: u32,
    next_attempt: u16,
    signals: EligibilitySignals,
    cut_evidence: Vec<CutEvidenceObservation>,
    coding_eval_evidence: Option<CodingEvalEvidence>,
    finished: bool,
}

impl RolloutRecorder {
    pub(crate) fn from_env<F>(
        workspace: &Path,
        requested_route: F,
        task_binding: Option<&TaskRolloutBindingV1>,
    ) -> Result<Self, String>
    where
        F: FnOnce() -> RouteIdentity,
    {
        let mode = CaptureMode::from_env();
        if mode == CaptureMode::Off {
            return Ok(Self::off());
        }
        let store = RolloutStore::for_workspace(workspace);
        let project = RolloutStore::project_identity(workspace);
        Self::start(
            store,
            mode,
            project,
            requested_route().into(),
            task_binding.cloned(),
        )
    }

    pub(crate) fn off() -> Self {
        Self {
            state: RecorderState::Off,
        }
    }

    fn start(
        store: RolloutStore,
        mode: CaptureMode,
        project: ProjectIdentity,
        requested_route: RecordedRoute,
        task_binding: Option<TaskRolloutBindingV1>,
    ) -> Result<Self, String> {
        if mode == CaptureMode::Off {
            return Ok(Self::off());
        }
        if let Some(binding) = task_binding.as_ref() {
            binding.validate()?;
        }
        let rollout_id = store.new_rollout_id();
        let lease = store.acquire_lease(&rollout_id)?;
        let started_ms = now_ms();
        let capture = CaptureDescriptor::new(mode);
        let mut cursor = JournalCursor::default();
        store.append_event(
            &rollout_id,
            &mut cursor,
            JournalEventKind::RolloutStarted {
                project: project.clone(),
                task_binding: task_binding.clone(),
                capture: capture.clone(),
                requested_route: requested_route.clone(),
                started_ms,
            },
        )?;
        let manifest = HarnessRolloutV1 {
            schema: ROLLOUT_SCHEMA.to_string(),
            rollout_id,
            project,
            task_binding,
            capture,
            started_ms,
            sealed_ms: None,
            status: RolloutStatus::Capturing,
            requested_route,
            attempts: Vec::new(),
            auxiliary_coverage: None,
            termination: None,
            reward: None,
            compatibility: None,
            eligibility: TrainingEligibility::Pending,
            journal_head_sha256: cursor.head_sha256.clone(),
            manifest_sha256: None,
        };
        Ok(Self {
            state: RecorderState::Active(Box::new(ActiveRecorder {
                store,
                _lease: lease,
                cursor,
                manifest,
                next_step: 0,
                next_attempt: 0,
                signals: EligibilitySignals {
                    semantic_bodies_missing: mode != CaptureMode::Local,
                    tool_pairing_valid: true,
                    ..EligibilitySignals::default()
                },
                cut_evidence: Vec::new(),
                coding_eval_evidence: None,
                finished: false,
            })),
        })
    }

    /// Observe actual harness dispatch truth, never model claims. Later mutations
    /// and non-passing verifier attempts invalidate a previous green candidate.
    pub(crate) fn observe_task_verifier(
        &mut self,
        call: &crate::agent::club::ToolCall,
        outcome: crate::agent::harness::ToolOutcome,
        result: &str,
        mutation: bool,
    ) {
        use crate::agent::harness::{ExecutionOutcome, VerificationOutcome};
        let RecorderState::Active(active) = &mut self.state else {
            return;
        };
        if active.finished || active.manifest.capture.mode != CaptureMode::Local {
            return;
        }
        let Some(binding) = active.manifest.task_binding.as_ref() else {
            return;
        };
        if mutation || outcome.verification != VerificationOutcome::NotApplicable {
            active.coding_eval_evidence = None;
        }
        if matches!(call.name.as_str(), "run_tests" | "cargo")
            && outcome.execution == ExecutionOutcome::Succeeded
            && outcome.verification == VerificationOutcome::Passed
        {
            active.coding_eval_evidence = Some(CodingEvalEvidence {
                schema: "angel-coding-eval-evidence/v1".into(),
                task_binding_sha256: binding.binding_sha256.clone(),
                tool: call.name.clone(),
                call_id_sha256: crate::knowledge::cut::sha256_hex(call.id.as_bytes()),
                arguments_sha256: crate::knowledge::cut::sha256_hex(
                    call.args.to_string().as_bytes(),
                ),
                receipt_sha256: crate::knowledge::cut::sha256_hex(result.as_bytes()),
                execution: outcome.execution.as_str().into(),
                verification: outcome.verification.as_str().into(),
            });
        }
    }

    pub(crate) fn attach_task_reward(&mut self) -> Result<(), String> {
        let RecorderState::Active(active) = &mut self.state else {
            return Ok(());
        };
        let Some(evidence) = active.coding_eval_evidence.as_ref() else {
            return Ok(());
        };
        let binding = active
            .manifest
            .task_binding
            .as_ref()
            .ok_or("task reward lacks binding")?;
        evidence.validate(binding)?;
        let body = serde_json::to_vec(evidence).map_err(|error| error.to_string())?;
        let digest = active.store.put_blob(&body)?;
        let mut receipt = RewardReceipt::new(RewardOwner::CodingEval, 1.0, digest)?;
        receipt.evidence_storage = RewardEvidenceStorage::RolloutBlob;
        self.attach_reward(receipt)
    }

    pub(crate) fn reward_binding(&self) -> Option<RewardReceipt> {
        match &self.state {
            RecorderState::Off => None,
            RecorderState::Active(active) => active.manifest.reward.clone(),
        }
    }

    /// Observe the already-redacted Cut machine result at the post-write seam.
    /// Only typed facts and digests survive; command, directory, diagnostics,
    /// operator text, and provider text are never copied into the evidence body.
    pub(crate) fn observe_cut_machine(&mut self, machine: &serde_json::Value) {
        let RecorderState::Active(active) = &mut self.state else {
            return;
        };
        if active.finished || active.manifest.capture.mode != CaptureMode::Local {
            return;
        }
        let Some(object) = machine.as_object() else {
            return;
        };
        let Some(exit) = object.get("exit").and_then(serde_json::Value::as_i64) else {
            return;
        };
        if object
            .get("timed_out")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true)
        {
            return;
        }
        let Some(command) = object.get("cmd").and_then(serde_json::Value::as_str) else {
            return;
        };
        let Some(directory) = object.get("dir").and_then(serde_json::Value::as_str) else {
            return;
        };
        let source = object
            .get("source")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown");
        let observation = CutEvidenceObservation {
            ordinal: active.cut_evidence.len().min(u32::MAX as usize) as u32,
            command_sha256: crate::knowledge::cut::sha256_hex(command.as_bytes()),
            directory_sha256: crate::knowledge::cut::sha256_hex(directory.as_bytes()),
            source_sha256: crate::knowledge::cut::sha256_hex(source.as_bytes()),
            exit,
            passed: exit == 0,
            duration_ms: object
                .get("dur_ms")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            diagnostic_sha256: object
                .get("err")
                .and_then(serde_json::Value::as_str)
                .map(|error| crate::knowledge::cut::sha256_hex(error.as_bytes())),
        };
        active.cut_evidence.push(observation);
    }

    /// Bind the final Cut reward to one immutable evidence manifest. Successive
    /// post-write observations are inputs to this one receipt, never competing
    /// reward owners.
    pub(crate) fn attach_cut_reward(
        &mut self,
        reward: Option<f32>,
        cut_owns_label: bool,
    ) -> Result<(), String> {
        let Some(value) = reward.filter(|_| cut_owns_label) else {
            return Ok(());
        };
        let receipt = {
            let RecorderState::Active(active) = &mut self.state else {
                return Ok(());
            };
            if active.manifest.capture.mode != CaptureMode::Local {
                return Ok(());
            }
            if active.cut_evidence.is_empty() {
                return Err("Cut reward has no independent machine evidence".to_string());
            }
            let evidence = CutEvidenceManifest {
                schema: CUT_EVIDENCE_SCHEMA,
                observation_count: active.cut_evidence.len(),
                observations: &active.cut_evidence,
            };
            let body = serde_json::to_vec(&evidence)
                .map_err(|error| format!("encode Cut reward evidence: {error}"))?;
            let digest = match active.store.put_blob(&body) {
                Ok(digest) => digest,
                Err(error) => {
                    active.signals.capture_failed = true;
                    return Err(error);
                }
            };
            RewardReceipt::new(RewardOwner::Cut, value, digest)?
        };
        self.attach_reward(receipt)
    }

    /// Attach one independently owned reward before sealing. The same receipt
    /// is idempotent; a different owner, contract, value, or evidence digest
    /// fails closed and makes the rollout ineligible.
    pub(crate) fn attach_reward(&mut self, receipt: RewardReceipt) -> Result<(), String> {
        receipt.validate()?;
        let RecorderState::Active(active) = &mut self.state else {
            return Ok(());
        };
        if active.finished {
            return Err("cannot attach reward to a sealed rollout".to_string());
        }
        if active.manifest.reward.as_ref() == Some(&receipt) {
            return Ok(());
        }
        if active.manifest.reward.is_some() {
            if !active.signals.reward_owner_conflict {
                if let Err(error) = active.store.append_event(
                    &active.manifest.rollout_id,
                    &mut active.cursor,
                    JournalEventKind::RewardAttachmentRejected { attempted: receipt },
                ) {
                    active.signals.capture_failed = true;
                    active.signals.reward_owner_conflict = true;
                    return Err(error);
                }
                active.manifest.journal_head_sha256 = active.cursor.head_sha256.clone();
            }
            active.signals.reward_owner_conflict = true;
            return Err("rollout already has a different reward owner or receipt".to_string());
        }
        if let Err(error) = active.store.append_event(
            &active.manifest.rollout_id,
            &mut active.cursor,
            JournalEventKind::RewardAttached {
                receipt: receipt.clone(),
            },
        ) {
            active.signals.capture_failed = true;
            return Err(error);
        }
        active.manifest.reward = Some(receipt);
        active.manifest.journal_head_sha256 = active.cursor.head_sha256.clone();
        Ok(())
    }

    pub(crate) fn attach_compatibility_snapshot<F>(&mut self, snapshot: F) -> Result<(), String>
    where
        F: FnOnce() -> (String, serde_json::Value, serde_json::Value),
    {
        let RecorderState::Active(active) = &mut self.state else {
            return Ok(());
        };
        if active.finished {
            return Err("cannot attach compatibility data to a sealed rollout".to_string());
        }
        // The compatibility payload contains semantic metadata derived from
        // turn history. Shadow mode is digest-only, so do not even evaluate
        // the lazy snapshot closure outside Local capture.
        if active.manifest.capture.mode != CaptureMode::Local {
            return Ok(());
        }
        if active.manifest.compatibility.is_some() {
            return Ok(());
        }
        let (club_label, root_trajectory, harness_treatment) = snapshot();
        if club_label.len() > 256 {
            return Err("rollout club label exceeds 256 bytes".to_string());
        }
        let compatibility = RolloutCompatibilityV1 {
            schema: "angel-rollout-compatibility/v1".to_string(),
            club_label,
            root_trajectory,
            harness_treatment,
        };
        if let Err(error) = active.store.append_event(
            &active.manifest.rollout_id,
            &mut active.cursor,
            JournalEventKind::CompatibilityCaptured {
                compatibility: compatibility.clone(),
            },
        ) {
            active.signals.capture_failed = true;
            return Err(error);
        }
        active.manifest.compatibility = Some(compatibility);
        active.manifest.journal_head_sha256 = active.cursor.head_sha256.clone();
        Ok(())
    }

    pub(crate) fn begin_policy_attempt<F>(
        &mut self,
        history: &[ChatMsg],
        definitions: &[ToolDef],
        requested_route: F,
    ) -> Result<Option<AttemptToken>, String>
    where
        F: FnOnce() -> RouteIdentity,
    {
        let RecorderState::Active(active) = &mut self.state else {
            return Ok(None);
        };
        if active.finished {
            return Err("rollout is already sealed".to_string());
        }
        if active.signals.capture_failed {
            // Once a write fails, never append a second request onto a journal
            // whose previous event may be torn. The provider turn continues;
            // finalization will seal the valid prefix as incomplete.
            return Ok(None);
        }
        let request = semantic_request(
            &active.store,
            active.manifest.capture.mode,
            history,
            definitions,
            &mut active.signals,
        )?;
        active.signals.tool_pairing_valid &= request.tool_pairing_valid;
        let token = AttemptToken {
            step_index: active.next_step,
            attempt_index: active.next_attempt,
        };
        let attempt = PolicyAttemptV1 {
            step_index: token.step_index,
            attempt_index: token.attempt_index,
            request,
            response: None,
            requested_route: requested_route().into(),
            resolved_route: None,
            outcome: PolicyStepOutcome::Pending,
        };
        if let Err(error) = active.store.append_event(
            &active.manifest.rollout_id,
            &mut active.cursor,
            JournalEventKind::PolicyRequestCaptured {
                attempt: attempt.clone(),
            },
        ) {
            active.signals.capture_failed = true;
            return Err(error);
        }
        active.manifest.journal_head_sha256 = active.cursor.head_sha256.clone();
        active.manifest.attempts.push(attempt);
        Ok(Some(token))
    }

    pub(crate) fn complete_policy_attempt<F>(
        &mut self,
        token: Option<AttemptToken>,
        reply: &ClubReply,
        resolved_route: F,
    ) -> Result<(), String>
    where
        F: FnOnce() -> RouteIdentity,
    {
        let Some(token) = token else {
            return Ok(());
        };
        let RecorderState::Active(active) = &mut self.state else {
            return Ok(());
        };
        let response = semantic_response(
            &active.store,
            active.manifest.capture.mode,
            reply,
            &mut active.signals,
        )?;
        let outcome = match reply {
            ClubReply::Text(_) => PolicyStepOutcome::TextAction,
            ClubReply::Calls(_) => PolicyStepOutcome::ToolCallsAction,
        };
        let route: RecordedRoute = resolved_route().into();
        let pending_matches = active.manifest.attempts.last().is_some_and(|attempt| {
            attempt.step_index == token.step_index
                && attempt.attempt_index == token.attempt_index
                && attempt.outcome == PolicyStepOutcome::Pending
        });
        if !pending_matches {
            return Err("rollout response does not match the pending attempt".to_string());
        }
        if let Err(error) = active.store.append_event(
            &active.manifest.rollout_id,
            &mut active.cursor,
            JournalEventKind::PolicyResponseCaptured {
                step_index: token.step_index,
                attempt_index: token.attempt_index,
                response: response.clone(),
                resolved_route: route.clone(),
                outcome: outcome.clone(),
            },
        ) {
            active.signals.capture_failed = true;
            return Err(error);
        }
        let attempt = active
            .manifest
            .attempts
            .last_mut()
            .expect("pending rollout attempt was validated before append");
        attempt.response = Some(response);
        attempt.resolved_route = Some(route);
        attempt.outcome = outcome;
        active.manifest.journal_head_sha256 = active.cursor.head_sha256.clone();
        active.next_step = active.next_step.saturating_add(1);
        active.next_attempt = 0;
        Ok(())
    }

    pub(crate) fn fail_policy_attempt(
        &mut self,
        token: Option<AttemptToken>,
        error: &str,
        emitted_answer: bool,
        retryable: bool,
    ) -> Result<(), String> {
        let Some(token) = token else {
            return Ok(());
        };
        let RecorderState::Active(active) = &mut self.state else {
            return Ok(());
        };
        let failure = InfrastructureFailure {
            class: if emitted_answer {
                InfrastructureClass::ProviderPartialOutput
            } else {
                InfrastructureClass::ProviderUnavailable
            },
            retryable,
            // Error bodies can contain provider details. Persist a digest only.
            detail_sha256: crate::knowledge::cut::sha256_hex(error.as_bytes()),
        };
        let outcome = if emitted_answer {
            PolicyStepOutcome::ProviderFailedAfterPartial { failure }
        } else {
            PolicyStepOutcome::ProviderFailedNoAction { failure }
        };
        let pending_matches = active.manifest.attempts.last().is_some_and(|attempt| {
            attempt.step_index == token.step_index
                && attempt.attempt_index == token.attempt_index
                && attempt.outcome == PolicyStepOutcome::Pending
        });
        if !pending_matches {
            return Err("rollout failure does not match the pending attempt".to_string());
        }
        if let Err(error) = active.store.append_event(
            &active.manifest.rollout_id,
            &mut active.cursor,
            JournalEventKind::PolicyAttemptFailed {
                step_index: token.step_index,
                attempt_index: token.attempt_index,
                outcome: outcome.clone(),
            },
        ) {
            active.signals.capture_failed = true;
            return Err(error);
        }
        let attempt = active
            .manifest
            .attempts
            .last_mut()
            .expect("pending rollout attempt was validated before append");
        attempt.outcome = outcome;
        active.manifest.journal_head_sha256 = active.cursor.head_sha256.clone();
        active.next_attempt = active.next_attempt.saturating_add(1);
        Ok(())
    }

    pub(crate) fn record_auxiliary_coverage(
        &mut self,
        coverage: super::super::auxiliary::AuxiliaryCoverage,
    ) -> Result<(), String> {
        coverage.validate()?;
        let RecorderState::Active(active) = &mut self.state else {
            return Ok(());
        };
        if active.finished || active.manifest.auxiliary_coverage.is_some() {
            return Err("duplicate or late native auxiliary coverage".into());
        }
        if let Err(error) = active.store.append_event(
            &active.manifest.rollout_id,
            &mut active.cursor,
            JournalEventKind::AuxiliaryCoverageCaptured {
                coverage: coverage.clone(),
            },
        ) {
            active.signals.capture_failed = true;
            return Err(error);
        }
        active.manifest.auxiliary_coverage = Some(coverage);
        active.manifest.journal_head_sha256 = active.cursor.head_sha256.clone();
        Ok(())
    }

    pub(crate) fn finish_turn(
        &mut self,
        stop_reason: &str,
        interrupted: bool,
        deadline_reached: bool,
        max_hops_reached: bool,
        final_answer: Option<&str>,
    ) -> Result<(), String> {
        let RecorderState::Active(active) = &mut self.state else {
            return Ok(());
        };
        if active.finished {
            return Ok(());
        }
        if active
            .manifest
            .attempts
            .last()
            .is_some_and(|attempt| attempt.outcome == PolicyStepOutcome::Pending)
        {
            active.signals.capture_failed = true;
        }
        if active.signals.capture_failed {
            // A failed append can leave an unterminated fragment. Re-anchor the
            // cursor to the audited complete prefix before adding the terminal
            // exclusion event; recovery never guesses past corrupt evidence.
            let events = active.store.audit_journal(&active.manifest.rollout_id)?;
            active
                .store
                .discard_torn_tail(&active.manifest.rollout_id)?;
            active.cursor.next_seq = events.len() as u64;
            active.cursor.head_sha256 = events
                .last()
                .map(|event| event.event_sha256.clone())
                .unwrap_or_default();
            active.manifest = rebuild_manifest(
                &active.manifest.rollout_id,
                &active.manifest.project.repo_key,
                &events,
            )?;
        }
        let termination = Termination::from_turn(
            stop_reason,
            interrupted,
            deadline_reached,
            max_hops_reached,
            final_answer,
        );
        let eligibility = TrainingEligibility::decide(
            &termination,
            active.manifest.reward.as_ref(),
            active.signals,
        );
        let status = if active.signals.capture_failed {
            RolloutStatus::Incomplete
        } else {
            RolloutStatus::Finalized
        };
        if let Err(error) = active.store.append_event(
            &active.manifest.rollout_id,
            &mut active.cursor,
            JournalEventKind::TurnStopped {
                termination: termination.clone(),
                eligibility: eligibility.clone(),
                status,
            },
        ) {
            active.signals.capture_failed = true;
            active.manifest.status = RolloutStatus::Incomplete;
            active.manifest.eligibility = TrainingEligibility::Excluded {
                reason: ExclusionReason::CaptureFailure,
                detail_sha256: Some(crate::knowledge::cut::sha256_hex(error.as_bytes())),
            };
            active.manifest.termination = Some(termination);
            active.manifest.sealed_ms = Some(now_ms());
            active.manifest.journal_head_sha256 = active.cursor.head_sha256.clone();
            active.store.save_manifest(&mut active.manifest)?;
            active.finished = true;
            return Err(error);
        }
        active.manifest.status = status;
        active.manifest.termination = Some(termination);
        active.manifest.eligibility = eligibility;
        active.manifest.sealed_ms = Some(now_ms());
        active.manifest.journal_head_sha256 = active.cursor.head_sha256.clone();
        active.store.save_manifest(&mut active.manifest)?;
        active.finished = true;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn start_for_test(
        store: RolloutStore,
        mode: CaptureMode,
        project: ProjectIdentity,
    ) -> Result<Self, String> {
        Self::start_for_test_with_binding(store, mode, project, None)
    }

    #[cfg(test)]
    pub(crate) fn start_for_test_with_binding(
        store: RolloutStore,
        mode: CaptureMode,
        project: ProjectIdentity,
        task_binding: Option<TaskRolloutBindingV1>,
    ) -> Result<Self, String> {
        Self::start(
            store,
            mode,
            project,
            RecordedRoute {
                driver: "scripted".to_string(),
                model_revision: Some("fixture-v1".to_string()),
                reasoning_effort: None,
            },
            task_binding,
        )
    }

    pub(crate) fn rollout_id(&self) -> Option<&str> {
        match &self.state {
            RecorderState::Off => None,
            RecorderState::Active(active) => Some(&active.manifest.rollout_id),
        }
    }
}

fn semantic_request(
    store: &RolloutStore,
    mode: CaptureMode,
    history: &[ChatMsg],
    definitions: &[ToolDef],
    signals: &mut EligibilitySignals,
) -> Result<PolicyRequestRef, String> {
    let pairing_valid = tool_pairing_valid(history);
    let mut messages = Vec::with_capacity(history.len());
    for message in history {
        let role = match message.role {
            ChatRole::System => RecordedRole::System,
            ChatRole::User => RecordedRole::User,
            ChatRole::Harness => RecordedRole::Harness,
            ChatRole::Assistant => RecordedRole::Assistant,
            ChatRole::Tool => RecordedRole::Tool,
        };
        let content = capture_blob(
            store,
            mode,
            message.content.as_bytes(),
            "text/plain; charset=utf-8",
            true,
            signals,
        )?;
        let attachments = message
            .attachments
            .iter()
            .map(media_ref)
            .collect::<Vec<_>>();
        let tool_calls = message
            .tool_calls
            .iter()
            .map(|call| recorded_tool_call(store, mode, call, signals))
            .collect::<Result<Vec<_>, _>>()?;
        messages.push(RecordedMessage {
            role,
            content,
            attachments,
            tool_calls,
            tool_call_id: message.tool_call_id.clone(),
            recovery_context: message.recovery_context.clone(),
        });
    }
    #[derive(serde::Serialize)]
    struct ToolSchema<'a> {
        name: &'a str,
        description: &'a str,
        params: &'a serde_json::Value,
    }
    let schemas = definitions
        .iter()
        .map(|definition| ToolSchema {
            name: &definition.name,
            description: &definition.description,
            params: &definition.params,
        })
        .collect::<Vec<_>>();
    let schema_bytes =
        serde_json::to_vec(&schemas).map_err(|error| format!("encode tool schemas: {error}"))?;
    let tool_schema_set = capture_blob(
        store,
        mode,
        &schema_bytes,
        "application/vnd.angel.tool-schema-set+json",
        true,
        signals,
    )?;
    let semantic_sha256 = request_digest(&messages, &tool_schema_set)?;
    Ok(PolicyRequestRef {
        messages,
        tool_schema_set,
        semantic_sha256,
        tool_pairing_valid: pairing_valid,
    })
}

fn semantic_response(
    store: &RolloutStore,
    mode: CaptureMode,
    reply: &ClubReply,
    signals: &mut EligibilitySignals,
) -> Result<PolicyResponseRef, String> {
    let action = match reply {
        ClubReply::Text(text) => RecordedAction::Text {
            content: capture_blob(
                store,
                mode,
                text.as_bytes(),
                "text/plain; charset=utf-8",
                true,
                signals,
            )?,
        },
        ClubReply::Calls(calls) => RecordedAction::ToolCalls {
            calls: calls
                .iter()
                .map(|call| recorded_tool_call(store, mode, call, signals))
                .collect::<Result<Vec<_>, _>>()?,
        },
    };
    let semantic_sha256 = action_digest(&action)?;
    Ok(PolicyResponseRef {
        action,
        semantic_sha256,
    })
}

fn recorded_tool_call(
    store: &RolloutStore,
    mode: CaptureMode,
    call: &crate::agent::club::ToolCall,
    signals: &mut EligibilitySignals,
) -> Result<RecordedToolCall, String> {
    let arguments = serde_json::to_vec(&call.args)
        .map_err(|error| format!("encode tool arguments: {error}"))?;
    Ok(RecordedToolCall {
        id: call.id.clone(),
        name: call.name.clone(),
        arguments: capture_blob(store, mode, &arguments, "application/json", true, signals)?,
    })
}

fn capture_blob(
    store: &RolloutStore,
    mode: CaptureMode,
    bytes: &[u8],
    media_type: &str,
    detect_secrets: bool,
    signals: &mut EligibilitySignals,
) -> Result<BlobRef, String> {
    let secret = detect_secrets
        && std::str::from_utf8(bytes)
            .ok()
            .is_some_and(contains_likely_secret);
    let sha256 = crate::knowledge::cut::sha256_hex(bytes);
    if secret {
        signals.secret_detected = true;
        return Ok(BlobRef {
            sha256,
            bytes: bytes.len() as u64,
            media_type: media_type.to_string(),
            storage: BlobStorage::Absent,
            sensitivity: Sensitivity::SecretRejected,
        });
    }
    let storage = if mode.stores_bodies() {
        if let Err(error) = store.put_blob(bytes) {
            signals.capture_failed = true;
            return Err(error);
        }
        BlobStorage::Local
    } else {
        BlobStorage::Absent
    };
    Ok(BlobRef {
        sha256,
        bytes: bytes.len() as u64,
        media_type: media_type.to_string(),
        storage,
        sensitivity: Sensitivity::Project,
    })
}

fn media_ref(media: &Media) -> MediaRef {
    match media {
        Media::Image { mime, b64 } => MediaRef {
            media_type: mime.clone(),
            encoded_bytes: b64.len() as u64,
            sha256: crate::knowledge::cut::sha256_hex(b64.as_bytes()),
            body_stored: false,
        },
        Media::Audio { format, b64 } => MediaRef {
            media_type: format!("audio/{format}"),
            encoded_bytes: b64.len() as u64,
            sha256: crate::knowledge::cut::sha256_hex(b64.as_bytes()),
            body_stored: false,
        },
    }
}

fn tool_pairing_valid(history: &[ChatMsg]) -> bool {
    let mut declared = HashSet::new();
    let mut pending = HashSet::new();
    for message in history {
        for call in message.tool_calls.iter() {
            if call.id.is_empty()
                || !declared.insert(call.id.as_str())
                || !pending.insert(call.id.as_str())
            {
                return false;
            }
        }
        if message.role == ChatRole::Tool {
            let Some(id) = message.tool_call_id.as_deref() else {
                return false;
            };
            if !pending.remove(id) {
                return false;
            }
        } else if message.tool_call_id.is_some() {
            return false;
        }
    }
    pending.is_empty()
}

pub(super) fn request_digest(
    messages: &[RecordedMessage],
    tool_schema_set: &BlobRef,
) -> Result<String, String> {
    let semantic = serde_json::json!({
        "messages": messages.iter().map(message_digest_value).collect::<Vec<_>>(),
        "tool_schema_sha256": tool_schema_set.sha256,
    });
    serde_json::to_vec(&semantic)
        .map(|bytes| crate::knowledge::cut::sha256_hex(&bytes))
        .map_err(|error| format!("encode semantic request digest: {error}"))
}

fn message_digest_value(message: &RecordedMessage) -> serde_json::Value {
    serde_json::json!({
        "role": message.role,
        "content_sha256": message.content.sha256,
        "attachments": message.attachments.iter().map(|media| {
            serde_json::json!({
                "media_type": media.media_type,
                "encoded_bytes": media.encoded_bytes,
                "sha256": media.sha256,
            })
        }).collect::<Vec<_>>(),
        "tool_calls": message.tool_calls.iter().map(|call| {
            serde_json::json!({
                "id": call.id,
                "name": call.name,
                "arguments_sha256": call.arguments.sha256,
            })
        }).collect::<Vec<_>>(),
        "tool_call_id": message.tool_call_id,
    })
}

pub(super) fn action_digest(action: &RecordedAction) -> Result<String, String> {
    let semantic = match action {
        RecordedAction::Text { content } => serde_json::json!({
            "kind": "text",
            "content_sha256": content.sha256,
        }),
        RecordedAction::ToolCalls { calls } => serde_json::json!({
            "kind": "tool_calls",
            "calls": calls.iter().map(|call| {
                serde_json::json!({
                    "id": call.id,
                    "name": call.name,
                    "arguments_sha256": call.arguments.sha256,
                })
            }).collect::<Vec<_>>(),
        }),
    };
    serde_json::to_vec(&semantic)
        .map(|bytes| crate::knowledge::cut::sha256_hex(&bytes))
        .map_err(|error| format!("encode semantic action digest: {error}"))
}

fn contains_likely_secret(text: &str) -> bool {
    // Rollouts certify exact policy bytes. Reject sensitive bodies instead of
    // rewriting them after their digest/task binding has been established.
    // Decode JSON too: quoted or escaped credentials must not evade detection.
    if crate::platform::secrets::contains_secret(text) {
        return true;
    }
    contains_transport_secret(text)
}

fn contains_transport_secret(text: &str) -> bool {
    // Inspect decoded values independently. A whitespace scan over serialized
    // JSON can join a description tail to the next field name (for example
    // `capability keywords","type":"string"`) and invent a credential.
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        return transport_value_contains_secret(&value);
    }
    let tokens = text.split_whitespace().collect::<Vec<_>>();
    for (index, token) in tokens.iter().enumerate() {
        let lower_token = token.to_ascii_lowercase();
        if let Some(scheme) = lower_token.find("://") {
            let rest = &lower_token[scheme + 3..];
            if rest.find('@').is_some_and(|at| !rest[..at].contains('/')) {
                return true;
            }
        }
        let exact_marker = matches!(
            lower_token.trim_matches(|character: char| character == ',' || character == '"'),
            "authorization"
                | "authorization:"
                | "bearer"
                | "--token"
                | "--api-key"
                | "password"
                | "passwd"
        );
        if exact_marker && tokens.get(index + 1).is_some() {
            return true;
        }
    }
    false
}

fn transport_value_contains_secret(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(text) => contains_transport_secret(text),
        serde_json::Value::Array(values) => values.iter().any(transport_value_contains_secret),
        serde_json::Value::Object(values) => values.values().any(transport_value_contains_secret),
        _ => false,
    }
}

/// Audit and seal one process-lost rollout without replaying model or tool
/// effects. The operator-facing audit command is a later plan phase; keeping
/// this entry point dormant does not make it part of the ordinary turn path.
#[allow(dead_code)]
pub(crate) fn recover_rollout(
    store: &RolloutStore,
    rollout_id: &str,
    expected_repo_key: &str,
) -> Result<HarnessRolloutV1, String> {
    let _lease = store.acquire_lease(rollout_id)?;
    if store.run_dir(rollout_id)?.join("manifest.json").exists() {
        return store.load_manifest(rollout_id, expected_repo_key);
    }
    let events = store.audit_journal(rollout_id)?;
    store.discard_torn_tail(rollout_id)?;
    let mut manifest = rebuild_manifest(rollout_id, expected_repo_key, &events)?;
    let mut cursor = JournalCursor {
        next_seq: events.len() as u64,
        head_sha256: events
            .last()
            .map(|event| event.event_sha256.clone())
            .unwrap_or_default(),
    };
    if manifest.termination.is_none() {
        let termination = Termination::process_lost();
        let eligibility = TrainingEligibility::Excluded {
            reason: ExclusionReason::InfrastructureFailure,
            detail_sha256: termination
                .infrastructure
                .as_ref()
                .map(|failure| failure.detail_sha256.clone()),
        };
        store.append_event(
            rollout_id,
            &mut cursor,
            JournalEventKind::TurnStopped {
                termination: termination.clone(),
                eligibility: eligibility.clone(),
                status: RolloutStatus::Incomplete,
            },
        )?;
        manifest.termination = Some(termination);
        manifest.eligibility = eligibility;
        manifest.status = RolloutStatus::Incomplete;
        manifest.sealed_ms = Some(now_ms());
    }
    manifest.journal_head_sha256 = cursor.head_sha256;
    store.save_manifest(&mut manifest)?;
    Ok(manifest)
}

pub(super) fn rebuild_manifest(
    rollout_id: &str,
    expected_repo_key: &str,
    events: &[JournalEvent],
) -> Result<HarnessRolloutV1, String> {
    let first = events
        .first()
        .ok_or_else(|| "journal has no start event".to_string())?;
    let JournalEventKind::RolloutStarted {
        project,
        task_binding,
        capture,
        requested_route,
        started_ms,
    } = &first.kind
    else {
        return Err("journal does not begin with rollout_started".to_string());
    };
    if project.repo_key != expected_repo_key {
        return Err("journal project identity mismatch".to_string());
    }
    let mut manifest = HarnessRolloutV1 {
        schema: ROLLOUT_SCHEMA.to_string(),
        rollout_id: rollout_id.to_string(),
        project: project.clone(),
        task_binding: task_binding.clone(),
        capture: capture.clone(),
        started_ms: *started_ms,
        sealed_ms: None,
        status: RolloutStatus::Capturing,
        requested_route: requested_route.clone(),
        attempts: Vec::new(),
        auxiliary_coverage: None,
        termination: None,
        reward: None,
        compatibility: None,
        eligibility: TrainingEligibility::Pending,
        journal_head_sha256: String::new(),
        manifest_sha256: None,
    };
    let mut reward_conflict_seen = false;
    for event in events.iter().skip(1) {
        if manifest.termination.is_some() {
            return Err("journal contains events after its terminal event".to_string());
        }
        if manifest.auxiliary_coverage.is_some()
            && !matches!(&event.kind, JournalEventKind::TurnStopped { .. })
        {
            return Err("journal contains work after native auxiliary coverage seal".into());
        }
        match &event.kind {
            JournalEventKind::RolloutStarted { .. } => {
                return Err("journal contains a duplicate start event".to_string());
            }
            JournalEventKind::PolicyRequestCaptured { attempt } => {
                manifest.attempts.push(attempt.clone());
                validate_attempt_order(&manifest.attempts)?;
            }
            JournalEventKind::PolicyResponseCaptured {
                step_index,
                attempt_index,
                response,
                resolved_route,
                outcome,
            } => {
                let attempt =
                    matching_pending_attempt(&mut manifest.attempts, *step_index, *attempt_index)?;
                attempt.response = Some(response.clone());
                attempt.resolved_route = Some(resolved_route.clone());
                attempt.outcome = outcome.clone();
            }
            JournalEventKind::PolicyAttemptFailed {
                step_index,
                attempt_index,
                outcome,
            } => {
                matching_pending_attempt(&mut manifest.attempts, *step_index, *attempt_index)?
                    .outcome = outcome.clone();
            }
            JournalEventKind::RewardAttached { receipt } => {
                receipt.validate()?;
                if manifest.reward.replace(receipt.clone()).is_some() {
                    return Err("journal contains multiple reward receipts".to_string());
                }
            }
            JournalEventKind::RewardAttachmentRejected { attempted } => {
                attempted.validate()?;
                let Some(owner) = manifest.reward.as_ref() else {
                    return Err(
                        "journal rejects a reward before establishing its owner".to_string()
                    );
                };
                if owner == attempted || std::mem::replace(&mut reward_conflict_seen, true) {
                    return Err("journal contains an invalid reward conflict".to_string());
                }
            }
            JournalEventKind::CompatibilityCaptured { compatibility } => {
                if compatibility.schema != "angel-rollout-compatibility/v1"
                    || manifest
                        .compatibility
                        .replace(compatibility.clone())
                        .is_some()
                {
                    return Err("journal contains invalid compatibility metadata".to_string());
                }
            }
            JournalEventKind::AuxiliaryCoverageCaptured { coverage } => {
                coverage.validate()?;
                if manifest
                    .auxiliary_coverage
                    .replace(coverage.clone())
                    .is_some()
                {
                    return Err("journal contains duplicate native auxiliary coverage".into());
                }
            }
            JournalEventKind::TurnStopped {
                termination,
                eligibility,
                status,
            } => {
                if manifest.termination.is_some() {
                    return Err("journal contains multiple terminal events".to_string());
                }
                manifest.termination = Some(termination.clone());
                manifest.eligibility = eligibility.clone();
                manifest.status = *status;
                manifest.sealed_ms = Some(now_ms());
            }
        }
    }
    validate_attempt_order(&manifest.attempts)?;
    Ok(manifest)
}

fn matching_pending_attempt(
    attempts: &mut [PolicyAttemptV1],
    step_index: u32,
    attempt_index: u16,
) -> Result<&mut PolicyAttemptV1, String> {
    attempts
        .last_mut()
        .filter(|attempt| {
            attempt.step_index == step_index
                && attempt.attempt_index == attempt_index
                && attempt.outcome == PolicyStepOutcome::Pending
        })
        .ok_or_else(|| "journal response does not match pending attempt".to_string())
}

#[cfg(test)]
pub(crate) fn request_for_test(
    store: &RolloutStore,
    mode: CaptureMode,
    history: &[ChatMsg],
    definitions: &[ToolDef],
) -> Result<(PolicyRequestRef, EligibilitySignals), String> {
    let mut signals = EligibilitySignals {
        tool_pairing_valid: true,
        ..EligibilitySignals::default()
    };
    let request = semantic_request(store, mode, history, definitions, &mut signals)?;
    Ok((request, signals))
}
