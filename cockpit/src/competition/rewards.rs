use super::profile::DeepCutProfileIdentityV1;
use super::rewards_decimal::subtract_score;
use super::rewards_validation::{
    same_official_event, same_reward_identity, validate_binding, validate_context, validate_ledger,
    validate_result,
};
use super::schema::{
    AdapterIdentityV1, ComparatorKindV1, CompetitionKeyV1, ObjectiveComparatorV1, ScoreV1,
};
use super::schema_validation::{validate_id, validate_sha256};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

pub(crate) const OFFICIAL_REWARD_SCHEMA_V1: &str = "angel.deep-cut-official-reward/v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RewardContextV1 {
    pub(crate) episode_id: String,
    pub(crate) candidate_id: String,
    pub(crate) submission_id: String,
    pub(crate) comparable_base_id: String,
    pub(crate) competition: CompetitionKeyV1,
    pub(crate) objective: ObjectiveComparatorV1,
    pub(crate) profile: DeepCutProfileIdentityV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OfficialResultSourceV1 {
    OfficialSubmissionResult,
    ModelSelfReport,
    ControlGate,
    UnrelatedRankMovement,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OfficialProvenanceV1 {
    pub(crate) source: OfficialResultSourceV1,
    pub(crate) adapter: AdapterIdentityV1,
    pub(crate) receipt_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OfficialResultV1 {
    pub(crate) result_id: String,
    pub(crate) result_revision: u64,
    pub(crate) episode_id: String,
    pub(crate) candidate_id: String,
    pub(crate) submission_id: String,
    pub(crate) comparable_base_id: String,
    pub(crate) competition: CompetitionKeyV1,
    pub(crate) objective: ObjectiveComparatorV1,
    pub(crate) profile: DeepCutProfileIdentityV1,
    pub(crate) candidate_score: ScoreV1,
    pub(crate) base_score: ScoreV1,
    pub(crate) provenance: OfficialProvenanceV1,
    pub(crate) corrects_binding_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OrientedComparisonV1 {
    Improvement,
    Tie,
    Regression,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OfficialRewardBindingV1 {
    pub(crate) schema: String,
    pub(crate) binding_id: String,
    pub(crate) episode_id: String,
    pub(crate) candidate_id: String,
    pub(crate) submission_id: String,
    pub(crate) official_result_id: String,
    pub(crate) official_result_revision: u64,
    pub(crate) competition: CompetitionKeyV1,
    pub(crate) objective: ObjectiveComparatorV1,
    pub(crate) profile: DeepCutProfileIdentityV1,
    pub(crate) comparable_base_id: String,
    pub(crate) candidate_score: ScoreV1,
    pub(crate) base_score: ScoreV1,
    pub(crate) oriented_comparison: OrientedComparisonV1,
    pub(crate) oriented_delta: ScoreV1,
    pub(crate) primary_reward: i8,
    pub(crate) official_provenance: OfficialProvenanceV1,
    pub(crate) corrects_binding_id: Option<String>,
}

impl OfficialRewardBindingV1 {
    pub(crate) fn validate(&self) -> Result<(), String> {
        validate_binding(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProvisionalShapingV1 {
    pub(crate) shaping_id: String,
    pub(crate) episode_id: String,
    pub(crate) value_millis: i64,
    pub(crate) provenance_sha256: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BindDispositionV1 {
    Appended,
    Duplicate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RewardSelectionV1<'a> {
    Official(&'a OfficialRewardBindingV1),
    Provisional(&'a ProvisionalShapingV1),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RewardLedgerV1 {
    shaping: Vec<ProvisionalShapingV1>,
    bindings: Vec<OfficialRewardBindingV1>,
}

impl RewardLedgerV1 {
    pub(crate) fn record_shaping(&mut self, shaping: ProvisionalShapingV1) -> Result<bool, String> {
        validate_id(&shaping.shaping_id, "invalid shaping id").map_err(str::to_string)?;
        validate_id(&shaping.episode_id, "invalid episode id").map_err(str::to_string)?;
        validate_sha256(&shaping.provenance_sha256).map_err(str::to_string)?;
        if let Some(existing) = self
            .shaping
            .iter()
            .find(|entry| entry.shaping_id == shaping.shaping_id)
        {
            return if existing == &shaping {
                Ok(false)
            } else {
                Err("conflicting shaping identity".into())
            };
        }
        self.shaping.push(shaping);
        Ok(true)
    }

    pub(crate) fn bind_official(
        &mut self,
        context: &RewardContextV1,
        result: OfficialResultV1,
    ) -> Result<(BindDispositionV1, OfficialRewardBindingV1), String> {
        validate_context(context)?;
        validate_result(context, &result)?;
        let binding = make_binding(result)?;
        if let Some(existing) = self
            .bindings
            .iter()
            .find(|entry| entry.official_result_id == binding.official_result_id)
        {
            return if existing == &binding {
                Ok((BindDispositionV1::Duplicate, existing.clone()))
            } else {
                Err("conflicting official result identity".into())
            };
        }
        if let Some(existing) = self
            .bindings
            .iter()
            .find(|entry| entry.official_provenance == binding.official_provenance)
        {
            return if same_official_event(existing, &binding) {
                Ok((BindDispositionV1::Duplicate, existing.clone()))
            } else {
                Err("conflicting official result receipt".into())
            };
        }
        match self.canonical_binding(&binding.episode_id) {
            None if binding.corrects_binding_id.is_some() => {
                return Err("first official result cannot be a correction".into());
            }
            Some(current)
                if binding.corrects_binding_id.as_deref() != Some(&current.binding_id)
                    || binding.official_result_revision <= current.official_result_revision
                    || !same_reward_identity(current, &binding) =>
            {
                return Err("correction must advance the canonical binding".into());
            }
            _ => {}
        }
        self.bindings.push(binding.clone());
        Ok((BindDispositionV1::Appended, binding))
    }

    pub(crate) fn canonical_binding(&self, episode_id: &str) -> Option<&OfficialRewardBindingV1> {
        self.bindings
            .iter()
            .rev()
            .find(|entry| entry.episode_id == episode_id)
    }

    pub(crate) fn shaping(&self) -> &[ProvisionalShapingV1] {
        &self.shaping
    }

    pub(crate) fn bindings(&self) -> &[OfficialRewardBindingV1] {
        &self.bindings
    }

    pub(crate) fn effective_reward(&self, episode_id: &str) -> Option<RewardSelectionV1<'_>> {
        self.canonical_binding(episode_id)
            .map(RewardSelectionV1::Official)
            .or_else(|| {
                self.shaping
                    .iter()
                    .rev()
                    .find(|entry| entry.episode_id == episode_id)
                    .map(RewardSelectionV1::Provisional)
            })
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        validate_ledger(&self.shaping, &self.bindings)
    }
}

pub(super) fn make_binding(result: OfficialResultV1) -> Result<OfficialRewardBindingV1, String> {
    let (left, right) = match result.objective.kind {
        ComparatorKindV1::HigherIsBetter => (&result.candidate_score, &result.base_score),
        ComparatorKindV1::LowerIsBetter => (&result.base_score, &result.candidate_score),
        ComparatorKindV1::AdapterDefined { .. } => {
            return Err("adapter-defined reward comparison requires its adapter".into());
        }
    };
    let ordering = result
        .objective
        .compare(&result.candidate_score, &result.base_score)
        .map_err(|_| "unsupported objective comparison".to_string())?;
    let oriented_comparison = match ordering {
        Ordering::Greater => OrientedComparisonV1::Improvement,
        Ordering::Equal => OrientedComparisonV1::Tie,
        Ordering::Less => OrientedComparisonV1::Regression,
    };
    let oriented_delta = subtract_score(left, right)?;
    let mut binding = OfficialRewardBindingV1 {
        schema: OFFICIAL_REWARD_SCHEMA_V1.into(),
        binding_id: String::new(),
        episode_id: result.episode_id,
        candidate_id: result.candidate_id,
        submission_id: result.submission_id,
        official_result_id: result.result_id,
        official_result_revision: result.result_revision,
        competition: result.competition,
        objective: result.objective,
        profile: result.profile,
        comparable_base_id: result.comparable_base_id,
        candidate_score: result.candidate_score,
        base_score: result.base_score,
        oriented_comparison,
        oriented_delta,
        primary_reward: match ordering {
            Ordering::Greater => 1,
            Ordering::Equal => 0,
            Ordering::Less => -1,
        },
        official_provenance: result.provenance,
        corrects_binding_id: result.corrects_binding_id,
    };
    binding.binding_id = binding_sha(&binding)?;
    Ok(binding)
}

pub(super) fn binding_sha(binding: &OfficialRewardBindingV1) -> Result<String, String> {
    let mut value = binding.clone();
    value.binding_id.clear();
    serde_json::to_vec(&value)
        .map(|body| crate::cut::sha256_hex(&body))
        .map_err(|error| format!("encode official reward binding: {error}"))
}
