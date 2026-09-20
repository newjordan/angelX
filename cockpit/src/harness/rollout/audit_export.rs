use super::recorder::{action_digest, rebuild_manifest, request_digest};
use super::schema::*;
use super::store::RolloutStore;
use crate::club::{ChatMsg, ChatRole, ToolCall};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

pub(crate) const COMPAT_EXPORT_SCHEMA: &str = "angel-trajectory/v2";
pub(crate) const CORPUS_EXPORT_SCHEMA: &str = "angel-harness-rollout-export-corpus/v1";
const LOSS_MASK_SCHEMA: &str = "angel-assistant-action-loss-mask/v1";

#[derive(Clone, Debug)]
pub(crate) struct AuditedRollout {
    pub(crate) manifest: HarnessRolloutV1,
    pub(crate) event_count: usize,
}

/// Deterministically load and verify one sealed rollout. This performs no
/// recovery, model call, tool call, Git operation, or mutation.
#[allow(dead_code)]
pub(crate) fn audit_workspace_rollout(
    workspace: &Path,
    rollout_id: &str,
) -> Result<AuditedRollout, String> {
    let store = RolloutStore::for_workspace(workspace);
    let project = RolloutStore::project_identity(workspace);
    audit_store_rollout(&store, rollout_id, &project.repo_key)
}

/// Export a bounded, body-free receipt for any sealed rollout, including one
/// that is intentionally ineligible for training because reward is owned by an
/// external evaluator. The audit validates the journal and every referenced
/// capture blob before projecting metadata; it never relaxes trajectory export
/// eligibility.
pub(crate) fn audit_workspace_rollout_receipt(
    workspace: &Path,
    rollout_id: &str,
) -> Result<Value, String> {
    let store = RolloutStore::for_workspace(workspace);
    let project = RolloutStore::project_identity(workspace);
    audit_store_rollout_receipt(&store, rollout_id, &project.repo_key)
}

/// Delegate journals must remain in the parent's configured store after the
/// disposable worktree is removed. Legacy per-worktree identities remain
/// useful captures, but cannot be linked by this same-store contract.
pub(crate) fn audit_delegate_rollout_receipt(
    parent: &Path,
    child: &Path,
    disposable_root: &Path,
    rollout_id: &str,
) -> Result<Value, String> {
    let parent_store = RolloutStore::for_workspace(parent);
    let child_store = RolloutStore::for_workspace(child);
    let root = child_store.canonical_root()?;
    if root != parent_store.canonical_root()?
        || root.starts_with(disposable_root.canonicalize().map_err(|e| e.to_string())?)
    {
        return Err("delegate capture is not durable in the parent rollout store".into());
    }
    let parent_project = RolloutStore::project_identity(parent);
    let child_project = RolloutStore::project_identity(child);
    if parent_project.repo_key != child_project.repo_key {
        return Err("delegate capture has an independent repository identity".into());
    }
    let receipt = audit_store_rollout_receipt(&child_store, rollout_id, &child_project.repo_key)?;
    let manifest = child_store.load_manifest(rollout_id, &child_project.repo_key)?;
    audit_delegate_task_subject(&manifest)?;
    Ok(receipt)
}

fn audit_delegate_task_subject(manifest: &HarnessRolloutV1) -> Result<(), String> {
    let binding = manifest
        .task_binding
        .as_ref()
        .ok_or("delegate has no task binding")?;
    let first = manifest
        .attempts
        .first()
        .ok_or("delegate has no captured request")?;
    if !first.request.messages.iter().any(|message| {
        message.role == RecordedRole::User && message.content.sha256 == binding.prompt_sha256
    }) {
        return Err("delegate task binding differs from its actual first request".into());
    }
    Ok(())
}

/// Explicit compatibility export. Ordinary trajectory-v2 logging never calls
/// this path, so capture/export cannot alter its bytes or write cadence.
#[allow(dead_code)]
pub(crate) fn export_workspace_rollout_v2(
    workspace: &Path,
    rollout_id: &str,
) -> Result<Value, String> {
    let store = RolloutStore::for_workspace(workspace);
    let project = RolloutStore::project_identity(workspace);
    export_store_rollout_v2(&store, rollout_id, &project.repo_key)
}

/// Discover, audit, and deterministically export every eligible project-local
/// rollout. This is an offline read only: it does not write an inbox, start a
/// trainer, contact a service, or alter the rollout store.
pub(crate) fn export_workspace_rollout_corpus_v2(workspace: &Path) -> Result<Value, String> {
    let store = RolloutStore::for_workspace(workspace);
    let project = RolloutStore::project_identity(workspace);
    export_store_rollout_corpus_v2(&store, &project)
}

pub(super) fn audit_store_rollout(
    store: &RolloutStore,
    rollout_id: &str,
    expected_repo_key: &str,
) -> Result<AuditedRollout, String> {
    audit_store_rollout_at_depth(store, rollout_id, expected_repo_key, 0)
}

fn audit_store_rollout_at_depth(
    store: &RolloutStore,
    rollout_id: &str,
    expected_repo_key: &str,
    depth: usize,
) -> Result<AuditedRollout, String> {
    if depth > 4 {
        return Err("delegate audit nesting bound exceeded".into());
    }
    let manifest = store.load_manifest(rollout_id, expected_repo_key)?;
    if manifest.status == RolloutStatus::Capturing
        || manifest.termination.is_none()
        || manifest.sealed_ms.is_none()
    {
        return Err("rollout manifest is not terminal and sealed".to_string());
    }
    let events = store.audit_journal(rollout_id)?;
    let rebuilt = rebuild_manifest(rollout_id, expected_repo_key, &events)?;
    if rebuilt.project != manifest.project
        || rebuilt.task_binding != manifest.task_binding
        || rebuilt.capture != manifest.capture
        || rebuilt.started_ms != manifest.started_ms
        || rebuilt.status != manifest.status
        || rebuilt.requested_route != manifest.requested_route
        || rebuilt.attempts != manifest.attempts
        || rebuilt.termination != manifest.termination
        || rebuilt.reward != manifest.reward
        || rebuilt.compatibility != manifest.compatibility
        || rebuilt.auxiliary_coverage != manifest.auxiliary_coverage
        || rebuilt.eligibility != manifest.eligibility
    {
        return Err("rollout manifest does not match its journal".to_string());
    }
    if let Some(binding) = manifest.task_binding.as_ref() {
        binding.validate()?;
    }
    if let Some(coverage) = manifest.auxiliary_coverage.as_ref() {
        coverage.validate()?;
        for link in &coverage.linked_operations {
            if link.artifact.parent_workspace_sha256 != manifest.project.canonical_root_sha256 {
                return Err("linked delegate belongs to another parent repository".into());
            }
            let child = audit_store_rollout_receipt_at_depth(
                store,
                &link.artifact.child_rollout_id,
                expected_repo_key,
                depth + 1,
            )?;
            audit_delegate_task_subject(
                &store.load_manifest(&link.artifact.child_rollout_id, expected_repo_key)?,
            )?;
            if child != link.artifact.child_audit {
                return Err("linked delegate receipt differs from its audited journal".into());
            }
        }
    }
    audit_capture_descriptor(&manifest.capture)?;
    if let Some(receipt) = manifest.reward.as_ref() {
        receipt.validate()?;
        if receipt.evidence_storage == RewardEvidenceStorage::RolloutBlob {
            let evidence = store.read_blob_by_digest(&receipt.evaluator_evidence_sha256)?;
            let audited_reward = match receipt.owner {
                RewardOwner::Cut => audit_cut_evidence(&evidence)?,
                RewardOwner::CodingEval => {
                    let evidence: CodingEvalEvidence = serde_json::from_slice(&evidence)
                        .map_err(|error| format!("decode task verifier evidence: {error}"))?;
                    evidence.validate(
                        manifest
                            .task_binding
                            .as_ref()
                            .ok_or("CodingEval reward lacks task binding")?,
                    )?;
                    1.0_f32
                }
                _ => return Err("unsupported blob reward owner".into()),
            };
            if audited_reward.to_bits() != receipt.value.to_bits() {
                return Err("reward does not match its evidence manifest".to_string());
            }
        }
    }
    for attempt in &manifest.attempts {
        audit_request(store, manifest.capture.mode, &attempt.request)?;
        if let Some(response) = attempt.response.as_ref() {
            audit_response(store, manifest.capture.mode, response)?;
        }
    }
    audit_termination(&manifest)?;
    if manifest.eligibility == TrainingEligibility::Eligible && manifest.compatibility.is_none() {
        return Err("eligible rollout lacks compatibility metadata".to_string());
    }
    if let Some(compatibility) = manifest.compatibility.as_ref()
        && (manifest.capture.mode != CaptureMode::Local
            || compatibility.schema != "angel-rollout-compatibility/v1"
            || compatibility.club_label.len() > 256)
    {
        return Err("unknown rollout compatibility schema".to_string());
    }
    if manifest.status == RolloutStatus::Finalized {
        let secret_detected = manifest.attempts.iter().any(attempt_contains_secret);
        let signals = EligibilitySignals {
            secret_detected,
            capture_failed: false,
            reward_owner_conflict: events.iter().any(|event| {
                matches!(
                    &event.kind,
                    super::store::JournalEventKind::RewardAttachmentRejected { .. }
                )
            }),
            semantic_bodies_missing: manifest.capture.mode != CaptureMode::Local,
            tool_pairing_valid: manifest
                .attempts
                .iter()
                .all(|attempt| attempt.request.tool_pairing_valid),
        };
        let expected = TrainingEligibility::decide(
            manifest
                .termination
                .as_ref()
                .expect("terminal manifest checked above"),
            manifest.reward.as_ref(),
            signals,
        );
        if expected != manifest.eligibility {
            return Err("rollout eligibility does not match its evidence".to_string());
        }
    }
    Ok(AuditedRollout {
        manifest,
        event_count: events.len(),
    })
}

pub(super) fn audit_store_rollout_receipt(
    store: &RolloutStore,
    rollout_id: &str,
    expected_repo_key: &str,
) -> Result<Value, String> {
    audit_store_rollout_receipt_at_depth(store, rollout_id, expected_repo_key, 0)
}

fn audit_store_rollout_receipt_at_depth(
    store: &RolloutStore,
    rollout_id: &str,
    expected_repo_key: &str,
    depth: usize,
) -> Result<Value, String> {
    let audit = audit_store_rollout_at_depth(store, rollout_id, expected_repo_key, depth)?;
    let manifest = &audit.manifest;
    let mut receipt = json!({
        "schema": "angel-harness-rollout-audit/v1",
        "rollout_id": manifest.rollout_id,
        "project": {
            "schema": "angel-project-digest/v1",
            "workspace_key_sha256": crate::cut::sha256_hex(manifest.project.workspace_key.as_bytes()),
            "repo_key_sha256": crate::cut::sha256_hex(manifest.project.repo_key.as_bytes()),
            "canonical_root_sha256": manifest.project.canonical_root_sha256,
        },
        "task_binding": manifest.task_binding,
        "capture": manifest.capture,
        "status": manifest.status,
        "eligibility": manifest.eligibility,
        "requested_route": manifest.requested_route,
        "resolved_actions": resolved_actions(manifest),
        "termination": manifest.termination,
        "reward": manifest.reward,
        "attempt_count": manifest.attempts.len(),
        "event_count": audit.event_count,
        "started_ms": manifest.started_ms,
        "sealed_ms": manifest.sealed_ms,
        "journal_head_sha256": manifest.journal_head_sha256,
        "manifest_sha256": manifest.manifest_sha256,
        "compatibility_present": manifest.compatibility.is_some(),
    });
    if let Some(coverage) = manifest.auxiliary_coverage.as_ref() {
        coverage.validate()?;
        receipt
            .as_object_mut()
            .expect("audit receipt object")
            .insert(
                "auxiliary_coverage".into(),
                serde_json::to_value(coverage).expect("auxiliary coverage serializes"),
            );
    }
    let receipt_sha256 = crate::cut::sha256_hex(
        &serde_json::to_vec(&receipt).expect("rollout audit receipt must serialize"),
    );
    receipt
        .as_object_mut()
        .expect("rollout audit receipt must be an object")
        .insert("receipt_sha256".to_string(), Value::String(receipt_sha256));
    Ok(receipt)
}

/// Completed action routes are observed separately from the requested route.
/// Failed/partial attempts and delegated work do not become model contributors
/// here. Missing identities remain counted as unresolved, including old stores.
fn resolved_actions(manifest: &HarnessRolloutV1) -> Value {
    let mut completed = 0u64;
    let mut unresolved = 0u64;
    let mut routes = BTreeMap::<(String, String, Option<String>), u64>::new();
    for attempt in &manifest.attempts {
        if !attempt.outcome.completed_action() {
            continue;
        }
        completed += 1;
        let identity = attempt.resolved_route.as_ref().and_then(|route| {
            let model = route.model_revision.as_ref()?;
            if route.driver.trim().is_empty() || model.trim().is_empty() {
                return None;
            }
            Some((
                route.driver.clone(),
                model.clone(),
                route.reasoning_effort.clone(),
            ))
        });
        if let Some(identity) = identity {
            *routes.entry(identity).or_default() += 1;
        } else {
            unresolved += 1;
        }
    }
    let routes: Vec<Value> = routes
        .into_iter()
        .map(|((driver, model, effort), count)| {
            json!({
                "route": {"driver": driver, "model_revision": model, "reasoning_effort": effort},
                "completed_actions": count,
            })
        })
        .collect();
    json!({
        "schema": "angel-resolved-actions/v1",
        "completed_actions": completed,
        "unresolved_actions": unresolved,
        "routes": routes,
    })
}

pub(super) fn export_store_rollout_v2(
    store: &RolloutStore,
    rollout_id: &str,
    expected_repo_key: &str,
) -> Result<Value, String> {
    let audit = audit_store_rollout(store, rollout_id, expected_repo_key)?;
    export_audited_rollout_v2(store, &audit)
}

pub(super) fn export_store_rollout_corpus_v2(
    store: &RolloutStore,
    project: &ProjectIdentity,
) -> Result<Value, String> {
    let rollout_ids = store.discover_rollout_ids()?;
    let mut records = Vec::new();
    let mut audited = 0u64;
    let mut rejected = 0u64;
    let mut by_schema = BTreeMap::<String, u64>::new();
    let mut by_status = BTreeMap::<String, u64>::new();
    let mut by_reward_owner = BTreeMap::<String, u64>::new();
    let mut by_exclusion_reason = BTreeMap::<String, u64>::new();
    let mut by_route = BTreeMap::<String, u64>::new();
    let mut by_model_revision = BTreeMap::<String, u64>::new();
    let mut rejected_by_reason = BTreeMap::<String, u64>::new();
    let mut not_exportable_by_reason = BTreeMap::<String, u64>::new();

    for rollout_id in &rollout_ids {
        let audit = match audit_store_rollout(store, rollout_id, &project.repo_key) {
            Ok(audit) => audit,
            Err(error) => {
                rejected = rejected.saturating_add(1);
                increment(&mut rejected_by_reason, audit_rejection_class(&error));
                continue;
            }
        };
        audited = audited.saturating_add(1);
        increment(&mut by_schema, audit.manifest.schema.clone());
        increment(
            &mut by_status,
            enum_key(&audit.manifest.status, "unknown_status"),
        );
        increment(
            &mut by_route,
            nonempty_key(&audit.manifest.requested_route.driver),
        );
        increment(
            &mut by_model_revision,
            audit
                .manifest
                .requested_route
                .model_revision
                .as_deref()
                .map(nonempty_key)
                .unwrap_or_else(|| "unknown".to_string()),
        );
        match audit.manifest.reward.as_ref() {
            Some(reward) => increment(
                &mut by_reward_owner,
                enum_key(&reward.owner, "unknown_reward_owner"),
            ),
            None => increment(&mut by_reward_owner, "none".to_string()),
        }
        match &audit.manifest.eligibility {
            TrainingEligibility::Excluded { reason, .. } => increment(
                &mut by_exclusion_reason,
                enum_key(reason, "unknown_exclusion"),
            ),
            TrainingEligibility::Eligible => {
                increment(&mut by_exclusion_reason, "eligible".to_string())
            }
            TrainingEligibility::Pending => {
                increment(&mut by_exclusion_reason, "pending".to_string())
            }
        }
        if audit.manifest.capture.mode == CaptureMode::Local
            && audit.manifest.status == RolloutStatus::Finalized
            && audit.manifest.eligibility == TrainingEligibility::Eligible
        {
            match export_audited_rollout_v2(store, &audit) {
                Ok(record) => records.push(record),
                Err(error) => {
                    increment(&mut not_exportable_by_reason, audit_rejection_class(&error))
                }
            }
        }
    }

    Ok(json!({
        "schema": CORPUS_EXPORT_SCHEMA,
        "project": project,
        "audit": {
            "scanned": rollout_ids.len(),
            "audited": audited,
            "rejected": rejected,
            "exported": records.len(),
            "by_schema": by_schema,
            "by_status": by_status,
            "by_reward_owner": by_reward_owner,
            "by_exclusion_reason": by_exclusion_reason,
            "by_route": by_route,
            "by_model_revision": by_model_revision,
            "rejected_by_reason": rejected_by_reason,
            "not_exportable_by_reason": not_exportable_by_reason,
        },
        "records": records,
    }))
}

fn export_audited_rollout_v2(
    store: &RolloutStore,
    audit: &AuditedRollout,
) -> Result<Value, String> {
    let manifest = &audit.manifest;
    if manifest.capture.mode != CaptureMode::Local {
        return Err("trajectory-v2 export requires local semantic bodies".to_string());
    }
    if manifest.status != RolloutStatus::Finalized
        || manifest.eligibility != TrainingEligibility::Eligible
    {
        return Err("trajectory-v2 export requires a finalized eligible rollout".to_string());
    }
    let termination = manifest
        .termination
        .as_ref()
        .ok_or_else(|| "rollout has no termination".to_string())?;
    if termination.kind != TerminationKind::Answer {
        return Err("trajectory-v2 export currently supports final-answer rollouts".to_string());
    }
    let final_attempt = manifest
        .attempts
        .iter()
        .rev()
        .find(|attempt| attempt.outcome.completed_action())
        .ok_or_else(|| "rollout has no completed policy action".to_string())?;
    let response = final_attempt
        .response
        .as_ref()
        .ok_or_else(|| "final policy action has no response".to_string())?;
    let RecordedAction::Text { content } = &response.action else {
        return Err("final answer rollout ends with tool calls".to_string());
    };
    let answer = read_utf8_blob(store, content)?;
    if termination.final_answer_sha256.as_deref()
        != Some(crate::cut::sha256_hex(answer.as_bytes()).as_str())
    {
        return Err("final answer digest does not match captured response".to_string());
    }
    let mut chat = final_attempt
        .request
        .messages
        .iter()
        .map(|message| recorded_message_to_chat(store, message))
        .collect::<Result<Vec<_>, _>>()?;
    chat.push(ChatMsg::assistant(answer.clone()));
    let messages = trajectory_messages(&chat);
    let loss_targets = chat
        .iter()
        .enumerate()
        .map(|(message_index, message)| {
            json!({
                "message_index": message_index,
                "target": message.role == ChatRole::Assistant,
                "source": if message.role == ChatRole::Assistant {
                    "assistant_policy_action"
                } else {
                    "masked_context"
                },
            })
        })
        .collect::<Vec<_>>();
    let compatibility = manifest
        .compatibility
        .as_ref()
        .ok_or_else(|| "eligible rollout lacks compatibility metadata".to_string())?;
    let reward = manifest
        .reward
        .as_ref()
        .ok_or_else(|| "eligible rollout lacks reward receipt".to_string())?;
    let manifest_sha256 = manifest
        .manifest_sha256
        .as_ref()
        .ok_or_else(|| "rollout manifest is unsealed".to_string())?;
    let policy_steps = manifest
        .attempts
        .iter()
        .map(|attempt| {
            json!({
                "step_index": attempt.step_index,
                "attempt_index": attempt.attempt_index,
                "request_semantic_sha256": attempt.request.semantic_sha256,
                "response_semantic_sha256": attempt
                    .response
                    .as_ref()
                    .map(|response| response.semantic_sha256.clone()),
                "tool_pairing_valid": attempt.request.tool_pairing_valid,
                "outcome": attempt.outcome,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "schema": COMPAT_EXPORT_SCHEMA,
        "ts_ms": manifest.started_ms,
        "club": compatibility.club_label,
        "hops": manifest
            .attempts
            .iter()
            .filter(|attempt| attempt.outcome.completed_action())
            .count(),
        "interrupted": termination.interrupted,
        "answer": answer,
        "messages": messages,
        "reward": reward.value,
        "evaluator_evidence_manifest_sha256": reward.evaluator_evidence_sha256,
        "reward_owner": reward.owner,
        "reward_contract": reward.contract,
        "root_trajectory": compatibility.root_trajectory,
        "harness_treatment": compatibility.harness_treatment,
        "repo": {
            "key": manifest.project.repo_key,
            "workspace_key": manifest.project.workspace_key,
            "canonical_root_sha256": manifest.project.canonical_root_sha256,
        },
        "source_rollout_id": manifest.rollout_id,
        "source_manifest_sha256": manifest_sha256,
        "source_rollout": {
            "schema": ROLLOUT_SCHEMA,
            "journal_head_sha256": manifest.journal_head_sha256,
            "capture_mode": manifest.capture.mode,
            "status": manifest.status,
            "eligibility": manifest.eligibility,
            "event_count": audit.event_count,
        },
        "structured_tool_pairing": {
            "preserved": true,
            "tool_call_ids": true,
            "tool_result_ids": true,
        },
        "loss_mask": {
            "schema": LOSS_MASK_SCHEMA,
            "policy": "assistant_actions_only",
            "messages": loss_targets,
        },
        "policy_steps": policy_steps,
    }))
}

fn increment(counts: &mut BTreeMap<String, u64>, key: String) {
    let count = counts.entry(key).or_default();
    *count = count.saturating_add(1);
}

fn enum_key<T: serde::Serialize>(value: &T, fallback: &str) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

fn nonempty_key(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        "unknown".to_string()
    } else {
        value.to_string()
    }
}

fn audit_rejection_class(error: &str) -> String {
    let lower = error.to_ascii_lowercase();
    if lower.contains("project identity")
        || lower.contains("repo_key")
        || lower.contains("identity")
    {
        "foreign_project".to_string()
    } else if lower.contains("schema") {
        "unknown_schema".to_string()
    } else if lower.contains("unsealed")
        || lower.contains("no termination")
        || lower.contains("not terminal")
        || lower.contains("manifest.json")
    {
        "unsealed_or_missing".to_string()
    } else {
        "corrupt_or_invalid".to_string()
    }
}

fn audit_capture_descriptor(capture: &CaptureDescriptor) -> Result<(), String> {
    if capture.semantic_bodies != capture.mode.stores_bodies()
        || capture.media_bodies
        || capture.private_reasoning_captured
        || capture.provider_headers_captured
    {
        return Err("rollout capture descriptor violates recorder policy".to_string());
    }
    Ok(())
}

fn audit_request(
    store: &RolloutStore,
    mode: CaptureMode,
    request: &PolicyRequestRef,
) -> Result<(), String> {
    audit_blob(store, mode, &request.tool_schema_set)?;
    for message in &request.messages {
        audit_blob(store, mode, &message.content)?;
        for media in &message.attachments {
            validate_sha256(&media.sha256)?;
            if media.body_stored {
                return Err("rollout media claims an unsupported stored body".to_string());
            }
        }
        for call in &message.tool_calls {
            audit_blob(store, mode, &call.arguments)?;
        }
    }
    if request.semantic_sha256 != request_digest(&request.messages, &request.tool_schema_set)? {
        return Err("policy request semantic digest mismatch".to_string());
    }
    if request.tool_pairing_valid != recorded_pairing_valid(&request.messages) {
        return Err("policy request pairing classification mismatch".to_string());
    }
    Ok(())
}

fn audit_response(
    store: &RolloutStore,
    mode: CaptureMode,
    response: &PolicyResponseRef,
) -> Result<(), String> {
    match &response.action {
        RecordedAction::Text { content } => audit_blob(store, mode, content)?,
        RecordedAction::ToolCalls { calls } => {
            for call in calls {
                audit_blob(store, mode, &call.arguments)?;
            }
        }
    }
    if response.semantic_sha256 != action_digest(&response.action)? {
        return Err("policy response semantic digest mismatch".to_string());
    }
    Ok(())
}

fn audit_blob(store: &RolloutStore, mode: CaptureMode, reference: &BlobRef) -> Result<(), String> {
    validate_sha256(&reference.sha256)?;
    let expected_storage = match reference.sensitivity {
        Sensitivity::SecretRejected => BlobStorage::Absent,
        Sensitivity::Project if mode == CaptureMode::Local => BlobStorage::Local,
        Sensitivity::Project => BlobStorage::Absent,
    };
    if reference.storage != expected_storage {
        return Err("rollout blob storage does not match capture policy".to_string());
    }
    if reference.storage == BlobStorage::Local {
        store.read_blob(reference)?;
    }
    Ok(())
}

fn audit_termination(manifest: &HarnessRolloutV1) -> Result<(), String> {
    let Some(termination) = manifest.termination.as_ref() else {
        return Err("sealed rollout has no termination".to_string());
    };
    if termination.kind != TerminationKind::Answer {
        return Ok(());
    }
    let answer_digest = termination
        .final_answer_sha256
        .as_deref()
        .ok_or_else(|| "answer termination has no final answer digest".to_string())?;
    validate_sha256(answer_digest)?;
    let final_response = manifest
        .attempts
        .iter()
        .rev()
        .find(|attempt| attempt.outcome.completed_action())
        .and_then(|attempt| attempt.response.as_ref())
        .ok_or_else(|| "answer termination has no completed policy response".to_string())?;
    let RecordedAction::Text { content } = &final_response.action else {
        return Err("answer termination ends with tool calls".to_string());
    };
    if content.sha256 != answer_digest {
        return Err("answer termination digest does not match final policy response".to_string());
    }
    Ok(())
}

fn validate_sha256(value: &str) -> Result<(), String> {
    let valid = value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    valid
        .then_some(())
        .ok_or_else(|| "invalid rollout SHA-256 digest".to_string())
}

fn recorded_pairing_valid(messages: &[RecordedMessage]) -> bool {
    let mut declared = HashSet::new();
    let mut pending = HashSet::new();
    for message in messages {
        for call in &message.tool_calls {
            if call.id.is_empty()
                || !declared.insert(call.id.as_str())
                || !pending.insert(call.id.as_str())
            {
                return false;
            }
        }
        if message.role == RecordedRole::Tool {
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

fn attempt_contains_secret(attempt: &PolicyAttemptV1) -> bool {
    attempt.request.tool_schema_set.sensitivity == Sensitivity::SecretRejected
        || attempt.request.messages.iter().any(|message| {
            message.content.sensitivity == Sensitivity::SecretRejected
                || message
                    .tool_calls
                    .iter()
                    .any(|call| call.arguments.sensitivity == Sensitivity::SecretRejected)
        })
        || attempt
            .response
            .as_ref()
            .is_some_and(response_contains_secret)
}

fn response_contains_secret(response: &PolicyResponseRef) -> bool {
    match &response.action {
        RecordedAction::Text { content } => content.sensitivity == Sensitivity::SecretRejected,
        RecordedAction::ToolCalls { calls } => calls
            .iter()
            .any(|call| call.arguments.sensitivity == Sensitivity::SecretRejected),
    }
}

fn recorded_message_to_chat(
    store: &RolloutStore,
    message: &RecordedMessage,
) -> Result<ChatMsg, String> {
    if !message.attachments.is_empty() {
        return Err("trajectory-v2 export does not reconstruct media bodies".to_string());
    }
    let content = read_utf8_blob(store, &message.content)?;
    let calls = message
        .tool_calls
        .iter()
        .map(|call| {
            let body = store.read_blob(&call.arguments)?;
            let args = serde_json::from_slice(&body)
                .map_err(|error| format!("decode captured tool arguments: {error}"))?;
            Ok(ToolCall {
                id: call.id.clone(),
                name: call.name.clone(),
                args,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(ChatMsg {
        role: match message.role {
            RecordedRole::System => ChatRole::System,
            RecordedRole::User => ChatRole::User,
            RecordedRole::Harness => ChatRole::Harness,
            RecordedRole::Assistant => ChatRole::Assistant,
            RecordedRole::Tool => ChatRole::Tool,
        },
        content: content.into(),
        attachments: Vec::new().into(),
        tool_calls: calls.into(),
        tool_call_id: message.tool_call_id.clone(),
        // Private provider reasoning is intentionally absent from durable
        // rollout artifacts and cannot be reconstructed on export.
        private_reasoning: None,
        tool_receipt: None,
        recovery_context: message.recovery_context.clone(),
    })
}

fn read_utf8_blob(store: &RolloutStore, reference: &BlobRef) -> Result<String, String> {
    String::from_utf8(store.read_blob(reference)?)
        .map_err(|_| "captured rollout text is not UTF-8".to_string())
}

fn trajectory_messages(history: &[ChatMsg]) -> Vec<Value> {
    crate::club::messages_to_json(history, true)
        .into_iter()
        .zip(history)
        .map(|(mut wire, message)| {
            let origin = match message.role {
                ChatRole::User => Some("operator"),
                ChatRole::Harness => Some("harness"),
                _ => None,
            };
            if let (Some(origin), Some(object)) = (origin, wire.as_object_mut()) {
                object.insert("origin".to_string(), Value::String(origin.to_string()));
            }
            wire
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SafeCutEvidence {
    schema: String,
    observation_count: usize,
    observations: Vec<SafeCutObservation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SafeCutObservation {
    ordinal: u32,
    command_sha256: String,
    directory_sha256: String,
    source_sha256: String,
    exit: i64,
    passed: bool,
    duration_ms: u64,
    diagnostic_sha256: Option<String>,
}

fn audit_cut_evidence(bytes: &[u8]) -> Result<f32, String> {
    let evidence: SafeCutEvidence = serde_json::from_slice(bytes)
        .map_err(|error| format!("decode Cut evidence manifest: {error}"))?;
    if evidence.schema != "angel-cut-evidence/v1"
        || evidence.observation_count != evidence.observations.len()
        || evidence.observations.is_empty()
    {
        return Err("invalid Cut evidence manifest".to_string());
    }
    for (index, observation) in evidence.observations.iter().enumerate() {
        if observation.ordinal != index as u32 || observation.passed != (observation.exit == 0) {
            return Err("invalid Cut evidence observation order/outcome".to_string());
        }
        validate_sha256(&observation.command_sha256)?;
        validate_sha256(&observation.directory_sha256)?;
        validate_sha256(&observation.source_sha256)?;
        if let Some(digest) = observation.diagnostic_sha256.as_deref() {
            validate_sha256(digest)?;
        }
        let _ = observation.duration_ms;
    }
    let left_red = evidence.observations.iter().any(|observation| {
        evidence
            .observations
            .iter()
            .rev()
            .find(|candidate| {
                candidate.command_sha256 == observation.command_sha256
                    && candidate.directory_sha256 == observation.directory_sha256
            })
            .is_some_and(|last| !last.passed)
    });
    if left_red {
        return Ok(0.0);
    }
    let passes = evidence
        .observations
        .iter()
        .filter(|observation| observation.passed)
        .count();
    Ok(passes as f32 / evidence.observations.len() as f32)
}
