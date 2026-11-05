use super::patterns::{
    FieldPatternMemoryV1, PatternEvidenceClassV1, PatternKeyV1, PatternOutcomeV1,
};
use super::schema::ScheduledActionV1;
use super::schema_validation::validate_id;
use crate::agent::harness::live_policy::{self, LegSource};
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

// The `deep-cut` leg decides which direction has the best score and which id
// wins a tie. Rust still rejects every malformed input before the leg runs; the
// allocator the leg replaced survives in the test module as its parity oracle.

/// One evidence use, reduced to the numbers the leg decides from. Text stays in
/// Rust: ids, class tags and the lexicographic rank arrive precomputed.
#[derive(Debug, Serialize)]
struct UseFeatureV1 {
    active: bool,
    matches_target: bool,
    provisional: bool,
    kind: &'static str,
    outcome: &'static str,
    cost_hi: u32,
    cost_lo: u32,
}

#[derive(Debug, Serialize)]
struct DirectionFeatureV1 {
    id: String,
    rank: u32,
    uses: Vec<UseFeatureV1>,
}

#[derive(Debug, Serialize)]
struct DirectionFeaturesV1 {
    fallback_id: String,
    directions: Vec<DirectionFeatureV1>,
}

/// The leg's plain-JSON answer: which direction, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DirectionLegOutV1 {
    pub(crate) selected_direction_id: String,
    pub(crate) basis: DirectionDecisionBasisV1,
}

/// A prepared leg call: the input and the contract check that rejects an
/// unoffered direction.
#[derive(Debug)]
pub(crate) struct DeepCutCall {
    pub(crate) input: serde_json::Value,
    features: DirectionFeaturesV1,
}

impl DeepCutCall {
    /// An answer is in contract when it names an offered direction or the
    /// configured fallback. Ranking itself stays the leg's business.
    pub(crate) fn check(&self, answer: &DirectionLegOutV1) -> Result<(), String> {
        if answer.selected_direction_id == self.features.fallback_id {
            return Ok(());
        }
        if self
            .features
            .directions
            .iter()
            .any(|direction| direction.id == answer.selected_direction_id)
        {
            return Ok(());
        }
        Err(format!(
            "deep-cut leg selected an unoffered direction: {}",
            answer.selected_direction_id
        ))
    }
}

fn outcome_tag(outcome: PatternOutcomeV1) -> &'static str {
    match outcome {
        PatternOutcomeV1::Improvement => "improvement",
        PatternOutcomeV1::Tie => "tie",
        PatternOutcomeV1::Regression => "regression",
        PatternOutcomeV1::Failure => "failure",
        PatternOutcomeV1::Timeout => "timeout",
    }
}

fn use_feature(
    memory: &FieldPatternMemoryV1,
    direction: &DeepCutDirectionV1,
    usage: &DirectionEvidenceUseV1,
) -> Result<UseFeatureV1, String> {
    let kind = match usage.kind {
        DirectionEvidenceUseKindV1::TargetEvidence => "target_evidence",
        DirectionEvidenceUseKindV1::MarkedProvisionalTransfer => "marked_provisional_transfer",
    };
    let Some(evidence) = memory.canonical_active_evidence(&usage.evidence_id)? else {
        // The native allocator drops a direction whose evidence is not active,
        // so these fields are never read; they only keep the shape total.
        return Ok(UseFeatureV1 {
            active: false,
            matches_target: false,
            provisional: false,
            kind,
            outcome: "tie",
            cost_hi: 0,
            cost_lo: 0,
        });
    };
    let penalty = evidence.cost_microunits / 1_000_000;
    Ok(UseFeatureV1 {
        active: true,
        matches_target: evidence.target_key == direction.target_key,
        provisional: matches!(
            evidence.class,
            PatternEvidenceClassV1::ProvisionalTransfer { .. }
        ),
        kind,
        outcome: outcome_tag(evidence.outcome),
        cost_hi: (penalty >> 32) as u32,
        cost_lo: penalty as u32,
    })
}

/// Build the leg's input, or fail with the native allocator's own message.
pub(crate) fn prepare(
    memory: &FieldPatternMemoryV1,
    directions: &[DeepCutDirectionV1],
    fallback_direction_id: &str,
) -> Result<DeepCutCall, String> {
    memory.validate()?;
    validate_id(fallback_direction_id, "invalid fallback direction").map_err(str::to_string)?;
    let mut seen = BTreeSet::new();
    let mut rows = Vec::with_capacity(directions.len());
    for direction in directions {
        validate_id(&direction.direction_id, "invalid direction id").map_err(str::to_string)?;
        direction.target_key.validate()?;
        if !seen.insert(direction.direction_id.clone()) {
            return Err("duplicate deep-cut direction id".into());
        }
        let mut uses = Vec::with_capacity(direction.evidence.len());
        let mut ids = BTreeSet::new();
        for usage in &direction.evidence {
            validate_id(&usage.evidence_id, "invalid direction evidence id")
                .map_err(str::to_string)?;
            if !ids.insert(usage.evidence_id.clone()) {
                return Err("duplicate evidence use in deep-cut direction".into());
            }
            uses.push(use_feature(memory, direction, usage)?);
        }
        rows.push((direction.direction_id.clone(), uses));
    }
    // The native leg breaks a score tie towards the smaller direction id, so the
    // leg receives that byte order as a rank instead of comparing strings.
    let mut ordered: Vec<String> = rows.iter().map(|(id, _)| id.clone()).collect();
    ordered.sort_unstable();
    let rank = |id: &str| {
        ordered
            .iter()
            .position(|candidate| candidate == id)
            .unwrap_or(0) as u32
    };
    let features = DirectionFeaturesV1 {
        fallback_id: fallback_direction_id.to_string(),
        directions: rows
            .into_iter()
            .map(|(id, uses)| {
                let position = rank(&id);
                DirectionFeatureV1 {
                    id,
                    rank: position,
                    uses,
                }
            })
            .collect(),
    };
    Ok(DeepCutCall {
        input: serde_json::to_value(&features).map_err(|error| error.to_string())?,
        features,
    })
}

pub(crate) fn decided(answer: DirectionLegOutV1, now_ms: u64) -> DirectionDecisionV1 {
    DirectionDecisionV1 {
        next: ScheduledActionV1 {
            action: format!("explore-direction:{}", answer.selected_direction_id),
            next_attempt_at_ms: now_ms,
        },
        selected_direction_id: answer.selected_direction_id,
        basis: answer.basis,
    }
}

/// The shipped route: the newest published `deep-cut` generation decides, and an
/// out-of-contract generation falls back to the generation compiled in.
pub(crate) fn allocate_direction_via_leg(
    memory: &FieldPatternMemoryV1,
    directions: &[DeepCutDirectionV1],
    fallback_direction_id: &str,
    now_ms: u64,
) -> Result<(DirectionDecisionV1, LegSource), String> {
    let call = prepare(memory, directions, fallback_direction_id)?;
    let (answer, source) =
        live_policy::decide_shipped("deep-cut", &call.input, |answer: &DirectionLegOutV1| {
            call.check(answer)
        });
    Ok((decided(answer, now_ms), source))
}
