use super::patterns::{
    FIELD_PATTERN_SCHEMA_V1, FieldBucketKeyV1, FieldPatternBucketV1, PatternAggregateEntryV1,
    PatternAggregateV1, PatternEvidenceClassV1, PatternEvidenceV1, PatternKeyV1, PatternOutcomeV1,
};
use super::rewards::OfficialRewardBindingV1;
use super::schema_validation::{validate_id, validate_sha256};
use crate::agent::harness::live_policy;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// The Bend leg that folds active evidence into pattern aggregates.
const PATTERN_REDUCER_LEG: &str = "patterns-reducer";

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
    reduce_fields(&active, evidence)
}

/// The `apply_evidence`/`apply_to_fields` pass kept intact as the parity oracle
/// for the leg: test builds only.
#[cfg(test)]
fn reduce_evidence(
    active: &BTreeSet<usize>,
    evidence: &[PatternEvidenceV1],
) -> Result<Vec<FieldPatternBucketV1>, String> {
    let mut fields = Vec::new();
    for (index, entry) in evidence.iter().enumerate() {
        if active.contains(&index) {
            apply_to_fields(&mut fields, entry)?;
        }
    }
    Ok(fields)
}

/// One rebuild through the `patterns-reducer` leg. Only the counter arithmetic
/// is decided by the leg: the bucket and pattern order, the active set and every
/// id come from Rust, and an out-of-contract answer keeps the shipped decision.
fn reduce_fields(
    active: &BTreeSet<usize>,
    evidence: &[PatternEvidenceV1],
) -> Result<Vec<FieldPatternBucketV1>, String> {
    let plan = FieldPlan::new(active, evidence);
    let (reduced, _source) = live_policy::decide_shipped(
        PATTERN_REDUCER_LEG,
        &plan.input(),
        |reduced: &LegReduction| plan.check(reduced),
    );
    plan.finish(reduced)
}

/// `reduce_fields` for a caller that already holds a programme, so a test can
/// pin one generation and read which leg answered.
#[cfg(test)]
pub(super) fn reduce_fields_with(
    program: Option<angel_live::Program>,
    evidence: &[PatternEvidenceV1],
) -> Result<(Vec<FieldPatternBucketV1>, live_policy::LegSource), String> {
    let active = canonical_active_indices(evidence)?;
    let plan = FieldPlan::new(&active, evidence);
    let (reduced, source) = live_policy::decide_with(
        program,
        PATTERN_REDUCER_LEG,
        &plan.input(),
        |reduced: &LegReduction| plan.check(reduced),
        || plan.native(&active, evidence),
    );
    Ok((plan.finish(reduced)?, source))
}

/// `reduce_fields` through the shipped route with its source, so a test can see
/// that no live generation still leaves a real decision.
#[cfg(test)]
pub(super) fn reduce_fields_shipped(
    evidence: &[PatternEvidenceV1],
) -> Result<(Vec<FieldPatternBucketV1>, live_policy::LegSource), String> {
    let active = canonical_active_indices(evidence)?;
    let plan = FieldPlan::new(&active, evidence);
    let (reduced, source) = live_policy::decide_shipped(
        PATTERN_REDUCER_LEG,
        &plan.input(),
        |reduced: &LegReduction| plan.check(reduced),
    );
    Ok((plan.finish(reduced)?, source))
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

/// One u64 counter as the leg sees it: two u32 halves, plus the refusal flag
/// the leg raises instead of wrapping a `checked_add` past `u64::MAX`.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct LegCount {
    hi: u32,
    lo: u32,
    over: bool,
}

impl LegCount {
    fn from_u64(value: u64) -> Self {
        Self {
            hi: (value >> 32) as u32,
            lo: value as u32,
            over: false,
        }
    }

    fn to_u64(self) -> u64 {
        (u64::from(self.hi) << 32) | u64::from(self.lo)
    }
}

/// `PatternAggregateV1` in the halves that cross the module boundary exactly.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct LegAggregate {
    confirmed_official: LegCount,
    provisional_transfer: LegCount,
    operational: LegCount,
    improvements: LegCount,
    ties: LegCount,
    regressions: LegCount,
    failures: LegCount,
    timeouts: LegCount,
    uncertainty_millis: LegCount,
    cost_microunits: LegCount,
}

impl LegAggregate {
    fn from_native(aggregate: &PatternAggregateV1) -> Self {
        Self {
            confirmed_official: LegCount::from_u64(aggregate.confirmed_official),
            provisional_transfer: LegCount::from_u64(aggregate.provisional_transfer),
            operational: LegCount::from_u64(aggregate.operational),
            improvements: LegCount::from_u64(aggregate.improvements),
            ties: LegCount::from_u64(aggregate.ties),
            regressions: LegCount::from_u64(aggregate.regressions),
            failures: LegCount::from_u64(aggregate.failures),
            timeouts: LegCount::from_u64(aggregate.timeouts),
            uncertainty_millis: LegCount::from_u64(aggregate.uncertainty_millis),
            cost_microunits: LegCount::from_u64(aggregate.cost_microunits),
        }
    }

    fn to_native(&self) -> PatternAggregateV1 {
        PatternAggregateV1 {
            confirmed_official: self.confirmed_official.to_u64(),
            provisional_transfer: self.provisional_transfer.to_u64(),
            operational: self.operational.to_u64(),
            improvements: self.improvements.to_u64(),
            ties: self.ties.to_u64(),
            regressions: self.regressions.to_u64(),
            failures: self.failures.to_u64(),
            timeouts: self.timeouts.to_u64(),
            uncertainty_millis: self.uncertainty_millis.to_u64(),
            cost_microunits: self.cost_microunits.to_u64(),
        }
    }

    fn counts(&self) -> [LegCount; 10] {
        [
            self.confirmed_official,
            self.provisional_transfer,
            self.operational,
            self.improvements,
            self.ties,
            self.regressions,
            self.failures,
            self.timeouts,
            self.uncertainty_millis,
            self.cost_microunits,
        ]
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct LegPattern {
    aggregate: LegAggregate,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct LegBucket {
    patterns: Vec<LegPattern>,
}

/// The leg's whole answer. `error` is the native channel: a live answer never
/// carries one, because `check` refuses an overflowing or misshapen one first.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct LegReduction {
    buckets: Vec<LegBucket>,
    #[serde(default)]
    error: Option<String>,
}

/// One rebuild's ordered shape, in the order `apply_to_fields`' binary search
/// builds it, with every active piece of evidence already in its slot. Rust
/// keeps the keys and the order; the leg decides only what each row counts.
struct FieldPlan {
    buckets: Vec<FieldBucketKeyV1>,
    patterns: Vec<Vec<PatternKeyV1>>,
    evidence: Vec<Vec<Vec<serde_json::Value>>>,
}

impl FieldPlan {
    fn new(active: &BTreeSet<usize>, evidence: &[PatternEvidenceV1]) -> Self {
        let mut layout =
            BTreeMap::<FieldBucketKeyV1, BTreeMap<PatternKeyV1, Vec<serde_json::Value>>>::new();
        for index in active {
            let entry = &evidence[*index];
            layout
                .entry(entry.target_key.bucket())
                .or_default()
                .entry(entry.target_key.clone())
                .or_default()
                .push(evidence_row(entry));
        }
        let buckets = layout.keys().cloned().collect();
        let patterns = layout
            .values()
            .map(|bucket| bucket.keys().cloned().collect())
            .collect();
        let evidence = layout
            .into_values()
            .map(|bucket| bucket.into_values().collect())
            .collect();
        Self {
            buckets,
            patterns,
            evidence,
        }
    }

    fn input(&self) -> serde_json::Value {
        let buckets: Vec<_> = self
            .evidence
            .iter()
            .map(|bucket| {
                let patterns: Vec<_> = bucket
                    .iter()
                    .map(|rows| serde_json::json!({"evidence": rows}))
                    .collect();
                serde_json::json!({"patterns": patterns})
            })
            .collect();
        serde_json::json!({"buckets": buckets})
    }

    /// The native reduction, encoded for the same shape a live answer has. The
    /// parity oracle: test builds only.
    #[cfg(test)]
    fn native(&self, active: &BTreeSet<usize>, evidence: &[PatternEvidenceV1]) -> LegReduction {
        match reduce_evidence(active, evidence) {
            Ok(fields) => LegReduction {
                buckets: self.encode(&fields),
                error: None,
            },
            Err(error) => LegReduction {
                buckets: Vec::new(),
                error: Some(error),
            },
        }
    }

    #[cfg(test)]
    fn encode(&self, fields: &[FieldPatternBucketV1]) -> Vec<LegBucket> {
        fields
            .iter()
            .map(|field| LegBucket {
                patterns: field
                    .patterns
                    .iter()
                    .map(|entry| LegPattern {
                        aggregate: LegAggregate::from_native(&entry.aggregate),
                    })
                    .collect(),
            })
            .collect()
    }

    /// The contract the leg has to keep: the plan's exact shape, no refused
    /// counter, and one membership and one outcome per row.
    fn check(&self, reduced: &LegReduction) -> Result<(), String> {
        if reduced.error.is_some() {
            return Err("live pattern reducer reported a refused reduction".into());
        }
        if reduced.buckets.len() != self.evidence.len() {
            return Err(format!(
                "live pattern reducer returned {} buckets for {}",
                reduced.buckets.len(),
                self.evidence.len()
            ));
        }
        for (bucket, expected) in reduced.buckets.iter().zip(&self.evidence) {
            if bucket.patterns.len() != expected.len() {
                return Err(format!(
                    "live pattern reducer returned {} patterns for {}",
                    bucket.patterns.len(),
                    expected.len()
                ));
            }
            for (pattern, rows) in bucket.patterns.iter().zip(expected) {
                let aggregate = &pattern.aggregate;
                // An exhausted counter is the reducer's own answer; `finish`
                // turns it into the error, and its other fields mean nothing.
                if aggregate.counts().iter().any(|count| count.over) {
                    continue;
                }
                let membership = sum(&[
                    aggregate.confirmed_official,
                    aggregate.provisional_transfer,
                    aggregate.operational,
                ])?;
                let outcomes = sum(&[
                    aggregate.improvements,
                    aggregate.ties,
                    aggregate.regressions,
                    aggregate.failures,
                    aggregate.timeouts,
                ])?;
                let rows = rows.len() as u64;
                if membership != rows || outcomes != rows {
                    return Err(format!(
                        "live pattern reducer counted {membership} memberships and {outcomes} outcomes for {rows} rows"
                    ));
                }
            }
        }
        Ok(())
    }

    /// Rebuild the native field list from the plan's keys and the counters.
    fn finish(self, reduced: LegReduction) -> Result<Vec<FieldPatternBucketV1>, String> {
        if let Some(error) = reduced.error {
            return Err(error);
        }
        // The rebuild is all-or-nothing, as the checked arithmetic was. When
        // several counters run out at once the count message is reported.
        let exhausted = reduced
            .buckets
            .iter()
            .flat_map(|bucket| &bucket.patterns)
            .map(|pattern| pattern.aggregate.counts())
            .fold([false; 3], |seen, counts| {
                [
                    seen[0] || counts[..8].iter().any(|count| count.over),
                    seen[1] || counts[8].over,
                    seen[2] || counts[9].over,
                ]
            });
        if exhausted[0] {
            return Err("pattern count exhausted".into());
        } else if exhausted[1] {
            return Err("pattern uncertainty exhausted".into());
        } else if exhausted[2] {
            return Err("pattern cost exhausted".into());
        }
        Ok(self
            .buckets
            .into_iter()
            .zip(self.patterns)
            .zip(reduced.buckets)
            .map(|((key, patterns), bucket)| FieldPatternBucketV1 {
                key,
                patterns: patterns
                    .into_iter()
                    .zip(bucket.patterns)
                    .map(|(key, entry)| PatternAggregateEntryV1 {
                        key,
                        aggregate: entry.aggregate.to_native(),
                    })
                    .collect(),
            })
            .collect())
    }
}

fn sum(counts: &[LegCount]) -> Result<u64, String> {
    counts.iter().try_fold(0u64, |total, count| {
        total
            .checked_add(count.to_u64())
            .ok_or_else(|| "live pattern reducer counted past u64".to_string())
    })
}

/// One active row, in the numbers the leg folds. Ids never cross.
fn evidence_row(evidence: &PatternEvidenceV1) -> serde_json::Value {
    serde_json::json!({
        "kind": class_code(&evidence.class),
        "outcome": outcome_code(evidence.outcome),
        "uncertainty": evidence.uncertainty_millis,
        "cost_hi": (evidence.cost_microunits >> 32) as u32,
        "cost_lo": evidence.cost_microunits as u32,
    })
}

/// Membership codes shared with `patterns-reducer.bend`.
fn class_code(class: &PatternEvidenceClassV1) -> u8 {
    match class {
        PatternEvidenceClassV1::ConfirmedOfficial { .. } => 0,
        PatternEvidenceClassV1::ProvisionalTransfer { .. } => 1,
        PatternEvidenceClassV1::OperationalOutcome => 2,
    }
}

/// Outcome codes shared with `patterns-reducer.bend`.
fn outcome_code(outcome: PatternOutcomeV1) -> u8 {
    match outcome {
        PatternOutcomeV1::Improvement => 0,
        PatternOutcomeV1::Tie => 1,
        PatternOutcomeV1::Regression => 2,
        PatternOutcomeV1::Failure => 3,
        PatternOutcomeV1::Timeout => 4,
    }
}
