use super::patterns::{
    FieldPatternMemoryV1, PatternEvidenceClassV1, PatternKeyV1, PatternOutcomeV1,
};
use super::schema::ScheduledActionV1;
use super::schema_validation::validate_id;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DirectionEvidenceUseKindV1 {
    TargetEvidence,
    MarkedProvisionalTransfer,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DirectionEvidenceUseV1 {
    pub(crate) evidence_id: String,
    pub(crate) kind: DirectionEvidenceUseKindV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeepCutDirectionV1 {
    pub(crate) direction_id: String,
    pub(crate) target_key: PatternKeyV1,
    pub(crate) evidence: Vec<DirectionEvidenceUseV1>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DirectionDecisionBasisV1 {
    TargetEvidence,
    ExplicitProvisionalTransfer,
    UntriedFallback,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DirectionDecisionV1 {
    pub(crate) selected_direction_id: String,
    pub(crate) basis: DirectionDecisionBasisV1,
    pub(crate) next: ScheduledActionV1,
}

pub(crate) fn allocate_direction(
    memory: &FieldPatternMemoryV1,
    directions: &[DeepCutDirectionV1],
    fallback_direction_id: &str,
    now_ms: u64,
) -> Result<DirectionDecisionV1, String> {
    memory.validate()?;
    validate_id(fallback_direction_id, "invalid fallback direction").map_err(str::to_string)?;
    let mut seen = BTreeSet::new();
    let mut eligible = Vec::new();
    for direction in directions {
        validate_id(&direction.direction_id, "invalid direction id").map_err(str::to_string)?;
        direction.target_key.validate()?;
        if !seen.insert(direction.direction_id.clone()) {
            return Err("duplicate deep-cut direction id".into());
        }
        if let Some((score, basis)) = score_direction(memory, direction)? {
            eligible.push((score, direction.direction_id.as_str(), basis));
        }
    }
    eligible.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(right.1)));
    let (selected, basis) = eligible
        .first()
        .map(|(_, id, basis)| ((*id).to_string(), *basis))
        .unwrap_or_else(|| {
            (
                fallback_direction_id.to_string(),
                DirectionDecisionBasisV1::UntriedFallback,
            )
        });
    Ok(DirectionDecisionV1 {
        next: ScheduledActionV1 {
            action: format!("explore-direction:{selected}"),
            next_attempt_at_ms: now_ms,
        },
        selected_direction_id: selected,
        basis,
    })
}

fn score_direction(
    memory: &FieldPatternMemoryV1,
    direction: &DeepCutDirectionV1,
) -> Result<Option<(i64, DirectionDecisionBasisV1)>, String> {
    let mut ids = BTreeSet::new();
    let mut score = 0i64;
    let mut saw_target = false;
    let mut saw_transfer = false;
    for usage in &direction.evidence {
        validate_id(&usage.evidence_id, "invalid direction evidence id").map_err(str::to_string)?;
        if !ids.insert(&usage.evidence_id) {
            return Err("duplicate evidence use in deep-cut direction".into());
        }
        let Some(evidence) = memory.canonical_active_evidence(&usage.evidence_id)? else {
            return Ok(None);
        };
        if evidence.target_key != direction.target_key {
            return Ok(None);
        }
        let provisional = matches!(
            evidence.class,
            PatternEvidenceClassV1::ProvisionalTransfer { .. }
        );
        match (usage.kind, provisional) {
            (DirectionEvidenceUseKindV1::TargetEvidence, true)
            | (DirectionEvidenceUseKindV1::MarkedProvisionalTransfer, false) => return Ok(None),
            (DirectionEvidenceUseKindV1::TargetEvidence, false) => saw_target = true,
            (DirectionEvidenceUseKindV1::MarkedProvisionalTransfer, true) => saw_transfer = true,
        }
        let weight = match evidence.outcome {
            PatternOutcomeV1::Improvement => 4,
            PatternOutcomeV1::Tie => 1,
            PatternOutcomeV1::Regression => -4,
            PatternOutcomeV1::Failure => -5,
            PatternOutcomeV1::Timeout => -3,
        };
        score = score.saturating_add(weight);
        let cost_penalty = i64::try_from(evidence.cost_microunits / 1_000_000).unwrap_or(i64::MAX);
        score = score.saturating_sub(cost_penalty);
    }
    let basis = match (saw_target, saw_transfer, direction.evidence.is_empty()) {
        (true, _, _) => DirectionDecisionBasisV1::TargetEvidence,
        (false, true, _) => DirectionDecisionBasisV1::ExplicitProvisionalTransfer,
        (_, _, true) => DirectionDecisionBasisV1::UntriedFallback,
        _ => return Ok(None),
    };
    Ok(Some((score, basis)))
}
