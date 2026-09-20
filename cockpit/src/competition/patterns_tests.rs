use super::patterns::{
    FieldPatternMemoryV1, PatternEvidenceClassV1, PatternKeyV1, PatternOutcomeV1,
};
use super::rewards::{OfficialRewardBindingV1, RewardContextV1, RewardLedgerV1};
use super::rewards_tests::{context, official};
use super::schema::ComparatorKindV1;

pub(super) fn key() -> PatternKeyV1 {
    PatternKeyV1 {
        field_id: "field".into(),
        benchmark_id: "benchmark".into(),
        profile_id: "profile".into(),
        hardware_id: "hardware".into(),
        objective_id: "latency".into(),
        comparator_version: "objective-v1".into(),
        bottleneck_class: "memory-bandwidth".into(),
        mechanism: "vectorized-load".into(),
    }
}

pub(super) fn binding(
    context: &RewardContextV1,
    id: &str,
    candidate: &str,
    base: &str,
) -> OfficialRewardBindingV1 {
    RewardLedgerV1::default()
        .bind_official(context, official(context, id, 1, candidate, base, None))
        .unwrap()
        .1
}

fn operational(
    memory: &mut FieldPatternMemoryV1,
    id: &str,
    key: PatternKeyV1,
    outcome: PatternOutcomeV1,
    cost: u64,
) {
    memory
        .record_operational(id.into(), "episode-pattern".into(), key, outcome, 100, cost)
        .unwrap();
}

#[test]
fn dc_pat_001_exact_field_profile_hardware_objective_partition() {
    let mut keys = vec![key()];
    let mut changed = key();
    changed.field_id = "other-field".into();
    keys.push(changed);
    let mut changed = key();
    changed.profile_id = "other-profile".into();
    keys.push(changed);
    let mut changed = key();
    changed.hardware_id = "other-hardware".into();
    keys.push(changed);
    let mut changed = key();
    changed.objective_id = "throughput".into();
    keys.push(changed);

    let mut memory = FieldPatternMemoryV1::default();
    for (index, key) in keys.iter().cloned().enumerate() {
        operational(
            &mut memory,
            &format!("partition-{index}"),
            key,
            PatternOutcomeV1::Failure,
            1,
        );
    }
    for key in keys {
        let aggregate = memory.aggregate(&key).unwrap();
        assert_eq!((aggregate.operational, aggregate.failures), (1, 1));
    }
    assert_eq!(memory.field_bucket_count(), 2);
    assert_eq!(memory.evidence_records().len(), 5);
}

#[test]
fn dc_pat_002_cross_key_transfer_remains_provisional() {
    let source = key();
    let mut target = key();
    target.hardware_id = "target-hardware".into();
    let mut memory = FieldPatternMemoryV1::default();
    memory
        .record_transfer(
            "transfer-one".into(),
            "episode-transfer".into(),
            source,
            target.clone(),
            PatternOutcomeV1::Improvement,
            250,
            50,
        )
        .unwrap();
    let aggregate = memory.aggregate(&target).unwrap();
    assert_eq!(
        (aggregate.provisional_transfer, aggregate.confirmed_official),
        (1, 0)
    );
    assert!(matches!(
        memory
            .canonical_active_evidence("transfer-one")
            .unwrap()
            .unwrap()
            .class,
        PatternEvidenceClassV1::ProvisionalTransfer { .. }
    ));
    let restored: FieldPatternMemoryV1 =
        serde_json::from_slice(&serde_json::to_vec(&memory).unwrap()).unwrap();
    assert_eq!(restored, memory);
}

#[test]
fn dc_pat_003_target_result_promotes_only_target_key_evidence() {
    let source = key();
    let mut target = key();
    target.hardware_id = "target-hardware".into();
    let mut memory = FieldPatternMemoryV1::default();
    memory
        .record_transfer(
            "transfer-promote".into(),
            "episode-transfer".into(),
            source,
            target.clone(),
            PatternOutcomeV1::Improvement,
            100,
            5,
        )
        .unwrap();

    let source_context = context(ComparatorKindV1::HigherIsBetter);
    let source_binding = binding(&source_context, "source-result", "12", "10");
    assert!(
        memory
            .record_official(
                &source_binding,
                target.bottleneck_class.clone(),
                target.mechanism.clone(),
                Some("transfer-promote".into()),
                0,
                7,
            )
            .is_err()
    );

    let mut target_context = source_context;
    target_context.competition.hardware_id = target.hardware_id.clone();
    let target_binding = binding(&target_context, "target-result", "12", "10");
    memory
        .record_official(
            &target_binding,
            target.bottleneck_class.clone(),
            target.mechanism.clone(),
            Some("transfer-promote".into()),
            0,
            7,
        )
        .unwrap();
    let aggregate = memory.aggregate(&target).unwrap();
    assert_eq!(
        (aggregate.provisional_transfer, aggregate.confirmed_official),
        (1, 1)
    );
    assert_eq!(memory.evidence_records().len(), 2);
    assert_eq!(
        memory.evidence_records()[1].promotes_evidence_id.as_deref(),
        Some("transfer-promote")
    );
}

#[test]
fn dc_pat_004_failure_regression_timeout_and_cost_remain_represented() {
    let key = key();
    let mut memory = FieldPatternMemoryV1::default();
    operational(
        &mut memory,
        "failure",
        key.clone(),
        PatternOutcomeV1::Failure,
        7,
    );
    operational(
        &mut memory,
        "timeout",
        key.clone(),
        PatternOutcomeV1::Timeout,
        11,
    );
    let context = context(ComparatorKindV1::HigherIsBetter);
    let binding = binding(&context, "regression-result", "8", "10");
    memory
        .record_official(
            &binding,
            key.bottleneck_class.clone(),
            key.mechanism.clone(),
            None,
            300,
            13,
        )
        .unwrap();
    let aggregate = memory.aggregate(&key).unwrap();
    assert_eq!(
        (
            aggregate.failures,
            aggregate.regressions,
            aggregate.timeouts,
            aggregate.cost_microunits,
            aggregate.uncertainty_millis,
        ),
        (1, 1, 1, 31, 500)
    );
    assert_eq!(memory.evidence_records().len(), 3);
}
