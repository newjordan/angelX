use super::rewards::{BindDispositionV1, RewardLedgerV1};
use super::rewards_tests::{context, official};
use super::schema::ComparatorKindV1;

#[test]
fn corrections_preserve_original_reward_identity() {
    let original = context(ComparatorKindV1::HigherIsBetter);
    let mut ledger = RewardLedgerV1::default();
    let (_, first) = ledger
        .bind_official(
            &original,
            official(&original, "identity-v1", 1, "12", "10", None),
        )
        .unwrap();
    for mutation in 0..8 {
        let mut switched = original.clone();
        match mutation {
            0 => switched.episode_id = "other-episode".into(),
            1 => switched.candidate_id = "other-candidate".into(),
            2 => switched.submission_id = "other-submission".into(),
            3 => switched.comparable_base_id = "other-base".into(),
            4 => switched.competition.profile_id = "other-profile".into(),
            5 => switched.competition.hardware_id = "other-hardware".into(),
            6 => switched.objective.version = "objective-v2".into(),
            _ => {
                switched.profile.policy_sha256 = crate::knowledge::cut::sha256_hex(b"other-policy")
            }
        }
        let correction = official(
            &switched,
            &format!("identity-v2-{mutation}"),
            2,
            "8",
            "10",
            Some(first.binding_id.clone()),
        );
        let mut attempt = ledger.clone();
        assert!(attempt.bind_official(&switched, correction).is_err());
        assert_eq!(attempt.bindings(), ledger.bindings());
    }
}

#[test]
fn official_receipt_deduplicates_across_result_ids() {
    let context = context(ComparatorKindV1::HigherIsBetter);
    let first_result = official(&context, "receipt-id-one", 1, "12", "10", None);
    let mut same_receipt = first_result.clone();
    same_receipt.result_id = "receipt-id-two".into();
    let mut ledger = RewardLedgerV1::default();
    let (_, first) = ledger.bind_official(&context, first_result).unwrap();
    let (disposition, duplicate) = ledger.bind_official(&context, same_receipt).unwrap();
    assert_eq!(disposition, BindDispositionV1::Duplicate);
    assert_eq!(duplicate, first);
    assert_eq!(ledger.bindings().len(), 1);

    let mut conflict = official(&context, "receipt-id-three", 1, "13", "10", None);
    conflict.provenance = first.official_provenance.clone();
    assert!(ledger.bind_official(&context, conflict).is_err());
    assert_eq!(ledger.bindings().len(), 1);
}

#[test]
fn reward_ledger_deserialize_requires_canonical_replay() {
    let context = context(ComparatorKindV1::HigherIsBetter);
    let mut ledger = RewardLedgerV1::default();
    let (_, first) = ledger
        .bind_official(
            &context,
            official(&context, "serde-v1", 1, "12", "10", None),
        )
        .unwrap();
    ledger
        .bind_official(
            &context,
            official(&context, "serde-v2", 2, "8", "10", Some(first.binding_id)),
        )
        .unwrap();
    let raw = serde_json::to_vec(&ledger).unwrap();
    let restored: RewardLedgerV1 = serde_json::from_slice(&raw).unwrap();
    restored.validate().unwrap();

    let mut tampered: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    tampered["bindings"][1]["primary_reward"] = 1.into();
    let tampered: RewardLedgerV1 = serde_json::from_value(tampered).unwrap();
    assert!(tampered.validate().is_err());

    let mut reordered: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    reordered["bindings"].as_array_mut().unwrap().reverse();
    let reordered: RewardLedgerV1 = serde_json::from_value(reordered).unwrap();
    assert!(reordered.validate().is_err());
}

#[test]
fn exact_decimal_handles_max_width_cross_sign_and_high_scale() {
    let cases = [
        ("max-width", "9".repeat(128), "0".into(), "9".repeat(128)),
        ("cross-sign", "1.25".into(), "-2.75".into(), "4".into()),
        (
            "high-scale",
            "1".into(),
            format!("0.{}1", "0".repeat(125)),
            format!("0.{}", "9".repeat(126)),
        ),
    ];
    for (id, candidate, base, expected) in cases {
        let context = context(ComparatorKindV1::HigherIsBetter);
        let binding = RewardLedgerV1::default()
            .bind_official(&context, official(&context, id, 1, &candidate, &base, None))
            .unwrap()
            .1;
        assert_eq!(binding.oriented_delta.as_str(), expected);
    }
}
