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
        // The `worker-health` leg chooses the step from the polled phase; the
        // lease mutation stays here so a bad edit cannot corrupt a lease.
        let (step, _source) = crate::agent::harness::live_policy::decide_shipped(
            "worker-health",
            &serde_json::json!({"kind": "recovery", "phase": phase_code(&phase)}),
            |step: &LiveRecoveryStep| match step.state {
                0 if step.phase == phase_code(&phase) => Ok(()),
                1..=3 => Ok(()),
                _ => Err(format!(
                    "out of contract recovery step {} for phase {}",
                    step.state,
                    phase_code(&phase)
                )),
            },
        );
        match step.state {
            1 => {
                leases.nudge(key, generation)?;
                Ok(LeaseRecoveryStepV1::Nudged)
            }
            2 => {
                leases.inspect(key, generation)?;
                Ok(LeaseRecoveryStepV1::Inspected)
            }
            3 => leases
                .replace(
                    key,
                    generation,
                    replacement_worker.into(),
                    now_ms,
                    replacement_deadline_at_ms,
                )
                .map(LeaseRecoveryStepV1::Replaced),
            _ => Ok(LeaseRecoveryStepV1::Healthy(phase)),
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
        let (step, _source) = crate::agent::harness::live_policy::decide_shipped(
            "worker-health",
            &serde_json::json!({"kind": "semantic", "progress": true, "stale_next": u64::from(state.stale_iterations)}),
            |step: &LiveSemanticStep| match (step.state, step.stale_next) {
                (0, 0) => Ok(()),
                _ => Err(format!("out of contract fresh semantic step {step:?}")),
            },
        );
        let _ = step;
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
        let (step, _source) = crate::agent::harness::live_policy::decide_shipped(
            "worker-health",
            &serde_json::json!({"kind": "semantic", "progress": false, "stale_next": u64::from(next_state.stale_iterations)}),
            |step: &LiveSemanticStep| match (step.state, step.stale_next) {
                (1, 2) | (3, 0) => Ok(()),
                (2, n) if n == u64::from(next_state.stale_iterations) => Ok(()),
                _ => Err(format!("out of contract stale semantic step {step:?}")),
            },
        );
        stale_directive(
            &mut next_state,
            &mut next_dossier,
            &fingerprint,
            now_ms,
            alternative_direction,
            next_turn_id,
            journal_head_sha256,
            step.state,
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
    step_state: u64,
) -> Result<SemanticDirectiveV1, String> {
    if step_state == 3 {
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
    if step_state == 1 {
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

pub(crate) fn phase_code(phase: &LeasePhaseV1) -> u64 {
    match phase {
        LeasePhaseV1::Granted => 0,
        LeasePhaseV1::Active => 1,
        LeasePhaseV1::Suspect => 2,
        LeasePhaseV1::Nudged => 3,
        LeasePhaseV1::Inspected => 4,
        LeasePhaseV1::Fenced => 5,
        LeasePhaseV1::Released => 6,
    }
}

/// The leg's recovery answer: state 0 keeps the healthy phase (in `phase`),
/// 1 nudges, 2 inspects, 3 replaces.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LiveRecoveryStep {
    pub(crate) state: u64,
    pub(crate) phase: u64,
}

/// The leg's semantic answer: state 0 continues fresh, 1 pivots, 2 keeps
/// retrying, 3 archives into a fresh turn; `stale_next` restates the
/// counter the shipped leg holds after the step.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LiveSemanticStep {
    pub(crate) state: u64,
    pub(crate) stale_next: u64,
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
        .map(|body| crate::knowledge::cut::sha256_hex(&body))
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
