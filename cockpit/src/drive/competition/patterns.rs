use super::patterns_reducer::{canonical_active_indices, rebuild_fields, validate_evidence};
use super::rewards::{OfficialRewardBindingV1, OrientedComparisonV1};
use super::schema_validation::validate_id;
use serde::{Deserialize, Serialize};

pub(crate) const FIELD_PATTERN_SCHEMA_V1: &str = "angel.deep-cut-field-pattern/v1";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PatternKeyV1 {
    pub(crate) field_id: String,
    pub(crate) benchmark_id: String,
    pub(crate) profile_id: String,
    pub(crate) hardware_id: String,
    pub(crate) objective_id: String,
    pub(crate) comparator_version: String,
    pub(crate) bottleneck_class: String,
    pub(crate) mechanism: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FieldBucketKeyV1 {
    pub(crate) field_id: String,
    pub(crate) benchmark_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PatternOutcomeV1 {
    Improvement,
    Tie,
    Regression,
    Failure,
    Timeout,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum PatternEvidenceClassV1 {
    ConfirmedOfficial {
        reward_binding_id: String,
        corrects_reward_binding_id: Option<String>,
    },
    ProvisionalTransfer {
        source_key: PatternKeyV1,
    },
    OperationalOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PatternEvidenceV1 {
    pub(crate) schema: String,
    pub(crate) evidence_id: String,
    pub(crate) episode_id: String,
    pub(crate) target_key: PatternKeyV1,
    pub(crate) class: PatternEvidenceClassV1,
    pub(crate) outcome: PatternOutcomeV1,
    pub(crate) uncertainty_millis: u16,
    pub(crate) cost_microunits: u64,
    pub(crate) promotes_evidence_id: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PatternAggregateV1 {
    pub(crate) confirmed_official: u64,
    pub(crate) provisional_transfer: u64,
    pub(crate) operational: u64,
    pub(crate) improvements: u64,
    pub(crate) ties: u64,
    pub(crate) regressions: u64,
    pub(crate) failures: u64,
    pub(crate) timeouts: u64,
    pub(crate) uncertainty_millis: u64,
    pub(crate) cost_microunits: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PatternAggregateEntryV1 {
    pub(crate) key: PatternKeyV1,
    pub(crate) aggregate: PatternAggregateV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FieldPatternBucketV1 {
    pub(crate) key: FieldBucketKeyV1,
    pub(crate) patterns: Vec<PatternAggregateEntryV1>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FieldPatternMemoryV1 {
    evidence: Vec<PatternEvidenceV1>,
    fields: Vec<FieldPatternBucketV1>,
}

impl FieldPatternMemoryV1 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_transfer(
        &mut self,
        evidence_id: String,
        episode_id: String,
        source_key: PatternKeyV1,
        target_key: PatternKeyV1,
        outcome: PatternOutcomeV1,
        uncertainty_millis: u16,
        cost_microunits: u64,
    ) -> Result<bool, String> {
        if source_key == target_key {
            return Err("transfer evidence must cross an exact pattern key".into());
        }
        self.append(PatternEvidenceV1 {
            schema: FIELD_PATTERN_SCHEMA_V1.into(),
            evidence_id,
            episode_id,
            target_key,
            class: PatternEvidenceClassV1::ProvisionalTransfer { source_key },
            outcome,
            uncertainty_millis,
            cost_microunits,
            promotes_evidence_id: None,
        })
    }

    pub(crate) fn record_operational(
        &mut self,
        evidence_id: String,
        episode_id: String,
        target_key: PatternKeyV1,
        outcome: PatternOutcomeV1,
        uncertainty_millis: u16,
        cost_microunits: u64,
    ) -> Result<bool, String> {
        if !matches!(
            outcome,
            PatternOutcomeV1::Failure | PatternOutcomeV1::Timeout
        ) {
            return Err("non-official operational evidence must be failure or timeout".into());
        }
        self.append(PatternEvidenceV1 {
            schema: FIELD_PATTERN_SCHEMA_V1.into(),
            evidence_id,
            episode_id,
            target_key,
            class: PatternEvidenceClassV1::OperationalOutcome,
            outcome,
            uncertainty_millis,
            cost_microunits,
            promotes_evidence_id: None,
        })
    }

    pub(crate) fn record_official(
        &mut self,
        binding: &OfficialRewardBindingV1,
        bottleneck_class: String,
        mechanism: String,
        promotes_evidence_id: Option<String>,
        uncertainty_millis: u16,
        cost_microunits: u64,
    ) -> Result<bool, String> {
        binding.validate()?;
        let target_key = PatternKeyV1::from_binding(binding, bottleneck_class, mechanism)?;
        if let Some(id) = &promotes_evidence_id {
            validate_id(id, "invalid promoted evidence id").map_err(str::to_string)?;
            let prior = self
                .evidence
                .iter()
                .find(|entry| &entry.evidence_id == id)
                .ok_or_else(|| "promoted transfer evidence is absent".to_string())?;
            if prior.target_key != target_key
                || !matches!(
                    prior.class,
                    PatternEvidenceClassV1::ProvisionalTransfer { .. }
                )
            {
                return Err("only target-key official evidence promotes a transfer".into());
            }
        }
        let outcome = match binding.oriented_comparison {
            OrientedComparisonV1::Improvement => PatternOutcomeV1::Improvement,
            OrientedComparisonV1::Tie => PatternOutcomeV1::Tie,
            OrientedComparisonV1::Regression => PatternOutcomeV1::Regression,
        };
        self.append(PatternEvidenceV1 {
            schema: FIELD_PATTERN_SCHEMA_V1.into(),
            evidence_id: crate::knowledge::cut::sha256_hex(
                serde_json::to_vec(&(&binding.binding_id, &target_key, &promotes_evidence_id))
                    .map_err(|error| format!("encode pattern evidence identity: {error}"))?
                    .as_slice(),
            ),
            episode_id: binding.episode_id.clone(),
            target_key,
            class: PatternEvidenceClassV1::ConfirmedOfficial {
                reward_binding_id: binding.binding_id.clone(),
                corrects_reward_binding_id: binding.corrects_binding_id.clone(),
            },
            outcome,
            uncertainty_millis,
            cost_microunits,
            promotes_evidence_id,
        })
    }

    pub(crate) fn aggregate(&self, key: &PatternKeyV1) -> Option<&PatternAggregateV1> {
        self.fields
            .iter()
            .find(|field| field.key == key.bucket())?
            .patterns
            .iter()
            .find(|entry| entry.key == *key)
            .map(|entry| &entry.aggregate)
    }

    pub(crate) fn canonical_active_evidence(
        &self,
        id: &str,
    ) -> Result<Option<&PatternEvidenceV1>, String> {
        let active = canonical_active_indices(&self.evidence)?;
        Ok(self
            .evidence
            .iter()
            .enumerate()
            .find(|(index, entry)| active.contains(index) && entry.evidence_id == id)
            .map(|(_, entry)| entry))
    }

    pub(crate) fn evidence_records(&self) -> &[PatternEvidenceV1] {
        &self.evidence
    }

    pub(crate) fn field_bucket_count(&self) -> usize {
        self.fields.len()
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if rebuild_fields(&self.evidence)? != self.fields {
            return Err("pattern memory replay mismatch".into());
        }
        Ok(())
    }

    fn append(&mut self, evidence: PatternEvidenceV1) -> Result<bool, String> {
        self.validate()?;
        validate_evidence(&evidence)?;
        if let Some(existing) = self
            .evidence
            .iter()
            .find(|entry| entry.evidence_id == evidence.evidence_id)
        {
            return if existing == &evidence {
                Ok(false)
            } else {
                Err("conflicting pattern evidence identity".into())
            };
        }
        let mut next_evidence = self.evidence.clone();
        next_evidence.push(evidence);
        let next_fields = rebuild_fields(&next_evidence)?;
        self.evidence = next_evidence;
        self.fields = next_fields;
        Ok(true)
    }
}
