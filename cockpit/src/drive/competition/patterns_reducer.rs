use super::patterns::{
    FIELD_PATTERN_SCHEMA_V1, FieldBucketKeyV1, FieldPatternBucketV1, PatternAggregateEntryV1,
    PatternAggregateV1, PatternEvidenceClassV1, PatternEvidenceV1, PatternKeyV1, PatternOutcomeV1,
};
use super::rewards::OfficialRewardBindingV1;
use super::schema_validation::{validate_id, validate_sha256};
use std::collections::{BTreeMap, BTreeSet};

impl PatternKeyV1 {
    pub(crate) fn from_binding(
        binding: &OfficialRewardBindingV1,
        bottleneck_class: String,
        mechanism: String,
    ) -> Result<Self, String> {
        let key = Self {
            field_id: binding.competition.field_id.clone(),
            benchmark_id: binding.competition.benchmark_id.clone(),
            profile_id: binding.competition.profile_id.clone(),
            hardware_id: binding.competition.hardware_id.clone(),
            objective_id: binding.objective.objective_id.clone(),
            comparator_version: binding.objective.version.clone(),
            bottleneck_class,
            mechanism,
        };
        key.validate()?;
        Ok(key)
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        for (value, label) in [
            (&self.field_id, "invalid pattern field"),
            (&self.benchmark_id, "invalid pattern benchmark"),
            (&self.profile_id, "invalid pattern profile"),
            (&self.hardware_id, "invalid pattern hardware"),
            (&self.objective_id, "invalid pattern objective"),
            (&self.comparator_version, "invalid comparator version"),
            (&self.bottleneck_class, "invalid bottleneck class"),
            (&self.mechanism, "invalid mechanism"),
        ] {
            validate_id(value, label).map_err(str::to_string)?;
        }
        Ok(())
    }

    pub(super) fn bucket(&self) -> FieldBucketKeyV1 {
        FieldBucketKeyV1 {
            field_id: self.field_id.clone(),
            benchmark_id: self.benchmark_id.clone(),
        }
    }
}

fn apply_evidence(
    aggregate: &mut PatternAggregateV1,
    evidence: &PatternEvidenceV1,
) -> Result<(), String> {
    let increment = |value: &mut u64| -> Result<(), String> {
        *value = value
            .checked_add(1)
            .ok_or_else(|| "pattern count exhausted".to_string())?;
        Ok(())
    };
    match evidence.class {
        PatternEvidenceClassV1::ConfirmedOfficial { .. } => {
            increment(&mut aggregate.confirmed_official)?
        }
        PatternEvidenceClassV1::ProvisionalTransfer { .. } => {
            increment(&mut aggregate.provisional_transfer)?
        }
        PatternEvidenceClassV1::OperationalOutcome => increment(&mut aggregate.operational)?,
    }
    match evidence.outcome {
        PatternOutcomeV1::Improvement => increment(&mut aggregate.improvements)?,
        PatternOutcomeV1::Tie => increment(&mut aggregate.ties)?,
        PatternOutcomeV1::Regression => increment(&mut aggregate.regressions)?,
        PatternOutcomeV1::Failure => increment(&mut aggregate.failures)?,
        PatternOutcomeV1::Timeout => increment(&mut aggregate.timeouts)?,
    }
    aggregate.uncertainty_millis = aggregate
        .uncertainty_millis
        .checked_add(evidence.uncertainty_millis.into())
        .ok_or_else(|| "pattern uncertainty exhausted".to_string())?;
    aggregate.cost_microunits = aggregate
        .cost_microunits
        .checked_add(evidence.cost_microunits)
        .ok_or_else(|| "pattern cost exhausted".to_string())?;
    Ok(())
}

pub(super) fn rebuild_fields(
    evidence: &[PatternEvidenceV1],
) -> Result<Vec<FieldPatternBucketV1>, String> {
    let active = canonical_active_indices(evidence)?;
    let mut fields = Vec::new();
    for (index, entry) in evidence.iter().enumerate() {
        if active.contains(&index) {
            apply_to_fields(&mut fields, entry)?;
        }
    }
    Ok(fields)
}

pub(super) fn canonical_active_indices(
    evidence: &[PatternEvidenceV1],
) -> Result<BTreeSet<usize>, String> {
    let mut evidence_ids = BTreeMap::<String, usize>::new();
    let mut binding_ids = BTreeSet::new();
    let mut canonical = BTreeMap::<(String, PatternKeyV1), (String, usize)>::new();
    for (index, entry) in evidence.iter().enumerate() {
        validate_evidence(entry)?;
        if evidence_ids.contains_key(&entry.evidence_id) {
            return Err("duplicate pattern evidence identity".into());
        }
        if let Some(promotes) = &entry.promotes_evidence_id {
            let promoted = evidence_ids
                .get(promotes)
                .map(|prior| &evidence[*prior])
                .ok_or_else(|| {
                    "promoted transfer evidence is absent or out of order".to_string()
                })?;
            if promoted.target_key != entry.target_key
                || !matches!(
                    promoted.class,
                    PatternEvidenceClassV1::ProvisionalTransfer { .. }
                )
            {
                return Err("promotion must resolve to target-key provisional transfer".into());
            }
        }
        if let PatternEvidenceClassV1::ConfirmedOfficial {
            reward_binding_id,
            corrects_reward_binding_id,
        } = &entry.class
        {
            if !binding_ids.insert(reward_binding_id) {
                return Err("reward binding influences pattern memory more than once".into());
            }
            let key = (entry.episode_id.clone(), entry.target_key.clone());
            match (canonical.get(&key), corrects_reward_binding_id) {
                (None, None) => {}
                (Some((current, _)), Some(corrects)) if current == corrects => {}
                _ => return Err("pattern reward correction is missing or out of order".into()),
            }
            canonical.insert(key, (reward_binding_id.clone(), index));
        }
        evidence_ids.insert(entry.evidence_id.clone(), index);
    }
    let mut active = evidence
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            !matches!(
                entry.class,
                PatternEvidenceClassV1::ConfirmedOfficial { .. }
            )
        })
        .map(|(index, _)| index)
        .collect::<BTreeSet<_>>();
    active.extend(canonical.values().map(|(_, index)| *index));
    Ok(active)
}

fn apply_to_fields(
    fields: &mut Vec<FieldPatternBucketV1>,
    evidence: &PatternEvidenceV1,
) -> Result<(), String> {
    let bucket_key = evidence.target_key.bucket();
    let field_index = match fields.binary_search_by(|field| field.key.cmp(&bucket_key)) {
        Ok(index) => index,
        Err(index) => {
            fields.insert(
                index,
                FieldPatternBucketV1 {
                    key: bucket_key,
                    patterns: Vec::new(),
                },
            );
            index
        }
    };
    let patterns = &mut fields[field_index].patterns;
    let pattern_index = match patterns.binary_search_by(|item| item.key.cmp(&evidence.target_key)) {
        Ok(index) => index,
        Err(index) => {
            patterns.insert(
                index,
                PatternAggregateEntryV1 {
                    key: evidence.target_key.clone(),
                    aggregate: PatternAggregateV1::default(),
                },
            );
            index
        }
    };
    apply_evidence(&mut patterns[pattern_index].aggregate, evidence)
}

pub(super) fn validate_evidence(evidence: &PatternEvidenceV1) -> Result<(), String> {
    if evidence.schema != FIELD_PATTERN_SCHEMA_V1 || evidence.uncertainty_millis > 1_000 {
        return Err("invalid pattern evidence".into());
    }
    validate_id(&evidence.evidence_id, "invalid pattern evidence id").map_err(str::to_string)?;
    validate_id(&evidence.episode_id, "invalid episode id").map_err(str::to_string)?;
    evidence.target_key.validate()?;
    if let PatternEvidenceClassV1::ConfirmedOfficial {
        reward_binding_id,
        corrects_reward_binding_id,
    } = &evidence.class
    {
        validate_sha256(reward_binding_id).map_err(str::to_string)?;
        if let Some(id) = corrects_reward_binding_id {
            validate_sha256(id).map_err(str::to_string)?;
        }
    }
    if let PatternEvidenceClassV1::ProvisionalTransfer { source_key } = &evidence.class {
        source_key.validate()?;
        if source_key == &evidence.target_key {
            return Err("transfer evidence must cross an exact pattern key".into());
        }
    }
    if let Some(id) = &evidence.promotes_evidence_id {
        validate_id(id, "invalid promoted evidence id").map_err(str::to_string)?;
        if !matches!(
            evidence.class,
            PatternEvidenceClassV1::ConfirmedOfficial { .. }
        ) {
            return Err("only official evidence may record a promotion".into());
        }
    }
    Ok(())
}
