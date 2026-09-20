use super::dossier::DossierV1;
use super::leases::{HEARTBEAT_CADENCE_MS, LeasePhaseV1, WorkLeaseKeyV1, WorkerLeaseV1};
use super::schema::{
    COMPETITION_SCHEMA_V1, DirectorHealthStateV1, DirectorHealthV1, ScheduledActionV1,
};
use super::schema_validation::{validate_id, validate_sha256};
use super::store::CompetitionStore;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub(crate) const SEMANTIC_LOOP_SCHEMA_V1: &str = "angel.competition-semantic-loop/v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LeaseRecoveryStepV1 {
    Healthy(LeasePhaseV1),
    Nudged,
    Inspected,
    Replaced(WorkerLeaseV1),
}

pub(crate) fn next_heartbeat_at_ms(last_at_ms: u64) -> Result<u64, String> {
    last_at_ms
        .checked_add(HEARTBEAT_CADENCE_MS)
        .ok_or_else(|| "heartbeat schedule exhausted".into())
}

pub(crate) fn recover_hung_worker(
    store: &CompetitionStore,
    key: &WorkLeaseKeyV1,
    generation: u64,
    now_ms: u64,
    replacement_worker: &str,
    replacement_deadline_at_ms: u64,
) -> Result<LeaseRecoveryStepV1, String> {
    store.update_leases(|leases| {
        let phase = leases.poll(key, generation, now_ms)?;
        match phase {
            LeasePhaseV1::Suspect => {
                leases.nudge(key, generation)?;
                Ok(LeaseRecoveryStepV1::Nudged)
            }
            LeasePhaseV1::Nudged => {
                leases.inspect(key, generation)?;
                Ok(LeaseRecoveryStepV1::Inspected)
            }
            LeasePhaseV1::Inspected => leases
                .replace(
                    key,
                    generation,
                    replacement_worker.into(),
                    now_ms,
                    replacement_deadline_at_ms,
                )
                .map(LeaseRecoveryStepV1::Replaced),
            other => Ok(LeaseRecoveryStepV1::Healthy(other)),
        }
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SemanticObservationV1 {
    pub(crate) tool_calls: BTreeSet<String>,
    pub(crate) hypotheses: BTreeSet<String>,
    pub(crate) read_targets: BTreeSet<String>,
    pub(crate) patch_mechanisms: BTreeSet<String>,
    pub(crate) benchmark_setups: BTreeSet<String>,
    pub(crate) payload_sha256: BTreeSet<String>,
    pub(crate) novel_evidence_sha256: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArchivedDirectionV1 {
    pub(crate) direction_id: String,
    pub(crate) fingerprint_sha256: String,
    pub(crate) stale_iterations: u8,
    pub(crate) archived_at_dossier_revision: u64,
    pub(crate) negative_evidence: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SemanticLoopStateV1 {
    pub(crate) schema: String,
    pub(crate) direction_id: String,
    pub(crate) last_fingerprint_sha256: Option<String>,
    pub(crate) known_evidence_sha256: BTreeSet<String>,
    pub(crate) stale_iterations: u8,
    pub(crate) pivot_count: u64,
    pub(crate) archived: Vec<ArchivedDirectionV1>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SemanticDirectiveV1 {
    Continue(DirectorHealthV1),
    Pivot(DirectorHealthV1),
    FreshTurn(DirectorHealthV1),
}

impl SemanticLoopStateV1 {
    pub(crate) fn new(direction_id: String) -> Result<Self, String> {
        validate_id(&direction_id, "invalid direction id").map_err(str::to_string)?;
        Ok(Self {
            schema: SEMANTIC_LOOP_SCHEMA_V1.into(),
            direction_id,
            last_fingerprint_sha256: None,
            known_evidence_sha256: BTreeSet::new(),
            stale_iterations: 0,
            pivot_count: 0,
            archived: Vec::new(),
        })
    }
}

pub(crate) fn record_semantic_iteration(
    state: &mut SemanticLoopStateV1,
    dossier: &mut DossierV1,
    observation: &SemanticObservationV1,
    now_ms: u64,
    alternative_direction: &str,
    next_turn_id: &str,
    journal_head_sha256: &str,
) -> Result<SemanticDirectiveV1, String> {
    validate_state(state)?;
    validate_id(alternative_direction, "invalid alternative direction").map_err(str::to_string)?;
    validate_id(next_turn_id, "invalid turn id").map_err(str::to_string)?;
    validate_sha256(journal_head_sha256).map_err(str::to_string)?;
    let fingerprint = fingerprint(observation)?;
    let novel = observation
        .novel_evidence_sha256
        .iter()
        .any(|digest| !state.known_evidence_sha256.contains(digest));
    let progress = novel || state.last_fingerprint_sha256.as_ref() != Some(&fingerprint);
    let mut next_state = state.clone();
    let mut next_dossier = dossier.clone();
    next_state
        .known_evidence_sha256
        .extend(observation.novel_evidence_sha256.iter().cloned());
    next_state.last_fingerprint_sha256 = Some(fingerprint.clone());
    let directive = if progress {
        next_state.stale_iterations = 0;
        SemanticDirectiveV1::Continue(health(
            DirectorHealthStateV1::Fresh,
            None,
            dossier.dossier_revision,
            "continue_direction",
            now_ms,
        )?)
    } else {
        next_state.stale_iterations = next_state
            .stale_iterations
            .checked_add(1)
            .ok_or_else(|| "semantic staleness counter exhausted".to_string())?;
        stale_directive(
            &mut next_state,
            &mut next_dossier,
            &fingerprint,
            now_ms,
            alternative_direction,
            next_turn_id,
            journal_head_sha256,
        )?
    };
    *state = next_state;
    *dossier = next_dossier;
    Ok(directive)
}

fn stale_directive(
    state: &mut SemanticLoopStateV1,
    dossier: &mut DossierV1,
    fingerprint: &str,
    now_ms: u64,
    alternative: &str,
    next_turn_id: &str,
    journal_head: &str,
) -> Result<SemanticDirectiveV1, String> {
    if state.stale_iterations == 4 {
        if state.direction_id == alternative {
            return Err("archived semantic direction requires a different alternative".into());
        }
        state.pivot_count = checked_pivot(state.pivot_count)?;
        dossier.rollover_fresh_turn(
            next_turn_id.into(),
            journal_head.into(),
            "semantic loop archived after four stale iterations".into(),
        )?;
        state.archived.push(ArchivedDirectionV1 {
            direction_id: state.direction_id.clone(),
            fingerprint_sha256: fingerprint.into(),
            stale_iterations: 4,
            archived_at_dossier_revision: dossier.dossier_revision,
            negative_evidence: "four equivalent semantic iterations".into(),
        });
        state.direction_id = alternative.into();
        state.stale_iterations = 0;
        state.last_fingerprint_sha256 = None;
        let scheduled = format!("start_direction:{alternative}");
        return Ok(SemanticDirectiveV1::FreshTurn(health(
            DirectorHealthStateV1::NeedsAttention,
            Some("semantic direction archived; fresh continuation required"),
            dossier.dossier_revision,
            &scheduled,
            now_ms,
        )?));
    }
    if state.stale_iterations == 2 {
        state.pivot_count = checked_pivot(state.pivot_count)?;
        return Ok(SemanticDirectiveV1::Pivot(health(
            DirectorHealthStateV1::Degraded,
            Some("two stale semantic iterations require structural pivot"),
            dossier.dossier_revision,
            "pivot_structure",
            now_ms,
        )?));
    }
    Ok(SemanticDirectiveV1::Continue(health(
        DirectorHealthStateV1::Retrying,
        Some("semantic progress is stale; inspect next iteration"),
        dossier.dossier_revision,
        "inspect_semantic_progress",
        now_ms,
    )?))
}

fn fingerprint(observation: &SemanticObservationV1) -> Result<String, String> {
    for value in observation
        .tool_calls
        .iter()
        .chain(&observation.hypotheses)
        .chain(&observation.read_targets)
        .chain(&observation.patch_mechanisms)
        .chain(&observation.benchmark_setups)
    {
        validate_id(value, "invalid semantic fingerprint value").map_err(str::to_string)?;
    }
    for digest in observation
        .payload_sha256
        .iter()
        .chain(&observation.novel_evidence_sha256)
    {
        validate_sha256(digest).map_err(str::to_string)?;
    }
    serde_json::to_vec(observation)
        .map(|body| crate::cut::sha256_hex(&body))
        .map_err(|error| format!("encode semantic fingerprint: {error}"))
}

fn health(
    state: DirectorHealthStateV1,
    reason: Option<&str>,
    revision: u64,
    action: &str,
    now_ms: u64,
) -> Result<DirectorHealthV1, String> {
    let health = DirectorHealthV1 {
        schema: COMPETITION_SCHEMA_V1.into(),
        state,
        last_good_revision: (revision > 0).then_some(revision),
        reason: reason.map(str::to_string),
        next: ScheduledActionV1 {
            action: action.into(),
            next_attempt_at_ms: now_ms,
        },
        updated_at_ms: now_ms,
    };
    health.validate().map_err(str::to_string)?;
    Ok(health)
}

fn validate_state(state: &SemanticLoopStateV1) -> Result<(), String> {
    if state.schema != SEMANTIC_LOOP_SCHEMA_V1 || state.stale_iterations > 3 {
        return Err("invalid semantic loop state".into());
    }
    validate_id(&state.direction_id, "invalid direction id").map_err(str::to_string)?;
    if state
        .last_fingerprint_sha256
        .as_deref()
        .is_some_and(|digest| validate_sha256(digest).is_err())
        || state
            .known_evidence_sha256
            .iter()
            .any(|digest| validate_sha256(digest).is_err())
    {
        return Err("invalid semantic loop digest".into());
    }
    Ok(())
}

fn checked_pivot(value: u64) -> Result<u64, String> {
    value
        .checked_add(1)
        .ok_or_else(|| "semantic pivot counter exhausted".into())
}
