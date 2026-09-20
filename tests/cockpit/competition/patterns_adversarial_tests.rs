use super::patterns::{FieldPatternMemoryV1, PatternKeyV1, PatternOutcomeV1};
use super::patterns_tests::key;
use super::rewards::RewardLedgerV1;
use super::rewards_tests::{context, official};
use super::schema::ComparatorKindV1;

pub(super) fn corrected_memory() -> (FieldPatternMemoryV1, PatternKeyV1) {
    let context = context(ComparatorKindV1::HigherIsBetter);
    let mut rewards = RewardLedgerV1::default();
    let (_, first) = rewards
        .bind_official(
            &context,
            official(&context, "pattern-v1", 1, "12", "10", None),
        )
        .unwrap();
    let (_, correction) = rewards
        .bind_official(
            &context,
            official(
                &context,
                "pattern-v2",
                2,
                "8",
                "10",
                Some(first.binding_id.clone()),
            ),
        )
        .unwrap();
    let key = key();
    let mut memory = FieldPatternMemoryV1::default();
    for (binding, uncertainty, cost) in [(&first, 100, 5), (&correction, 200, 7)] {
        memory
            .record_official(
                binding,
                key.bottleneck_class.clone(),
                key.mechanism.clone(),
                None,
                uncertainty,
                cost,
            )
            .unwrap();
    }
    (memory, key)
}

#[test]
fn corrected_pattern_influence_selects_only_canonical_binding() {
    let (memory, key) = corrected_memory();
    let aggregate = memory.aggregate(&key).unwrap();
    assert_eq!(
        (
            aggregate.confirmed_official,
            aggregate.improvements,
            aggregate.regressions,
            aggregate.cost_microunits,
            aggregate.uncertainty_millis,
        ),
        (1, 0, 1, 7, 200)
    );
    assert_eq!(memory.evidence_records().len(), 2);
    memory.validate().unwrap();
}

#[test]
fn pattern_replay_and_overflow_are_failure_atomic() {
    let (memory, _) = corrected_memory();
    let raw = serde_json::to_vec(&memory).unwrap();
    let mut tampered: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    tampered["fields"][0]["patterns"][0]["aggregate"]["regressions"] = 9.into();
    let tampered: FieldPatternMemoryV1 = serde_json::from_value(tampered).unwrap();
    assert!(tampered.validate().is_err());
    let mut reordered: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    reordered["evidence"].as_array_mut().unwrap().reverse();
    let reordered: FieldPatternMemoryV1 = serde_json::from_value(reordered).unwrap();
    assert!(reordered.validate().is_err());

    let mut overflow = FieldPatternMemoryV1::default();
    overflow
        .record_operational(
            "max-cost".into(),
            "episode-pattern".into(),
            key(),
            PatternOutcomeV1::Failure,
            100,
            u64::MAX,
        )
        .unwrap();
    let before = overflow.clone();
    assert!(
        overflow
            .record_operational(
                "overflow-cost".into(),
                "episode-pattern".into(),
                key(),
                PatternOutcomeV1::Timeout,
                1,
                1,
            )
            .is_err()
    );
    assert_eq!(overflow, before);
}

#[test]
fn promotion_pointer_must_resolve_to_matching_provisional_transfer() {
    let source = key();
    let mut target = key();
    target.hardware_id = "target-hardware".into();
    let mut wrong_target = target.clone();
    wrong_target.hardware_id = "wrong-hardware".into();
    let mut memory = FieldPatternMemoryV1::default();
    for (id, target_key) in [
        ("matching-transfer", target.clone()),
        ("wrong-transfer", wrong_target),
    ] {
        memory
            .record_transfer(
                id.into(),
                "episode-transfer".into(),
                source.clone(),
                target_key,
                PatternOutcomeV1::Improvement,
                10,
                1,
            )
            .unwrap();
    }
    let mut target_context = context(ComparatorKindV1::HigherIsBetter);
    target_context.competition.hardware_id = target.hardware_id.clone();
    let binding = RewardLedgerV1::default()
        .bind_official(
            &target_context,
            official(&target_context, "promotion-result", 1, "12", "10", None),
        )
        .unwrap()
        .1;
    memory
        .record_official(
            &binding,
            target.bottleneck_class.clone(),
            target.mechanism.clone(),
            Some("matching-transfer".into()),
            10,
            1,
        )
        .unwrap();

    let mut tampered = serde_json::to_value(&memory).unwrap();
    tampered["evidence"][2]["promotes_evidence_id"] = "wrong-transfer".into();
    let tampered: FieldPatternMemoryV1 = serde_json::from_value(tampered).unwrap();
    assert!(tampered.validate().is_err());
}
