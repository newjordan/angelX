use super::profile::DeepCutProfileV1;
use super::rewards::{
    BindDispositionV1, OfficialProvenanceV1, OfficialResultSourceV1, OfficialResultV1,
    OrientedComparisonV1, ProvisionalShapingV1, RewardContextV1, RewardLedgerV1, RewardSelectionV1,
};
use super::schema::{
    AdapterCapabilityV1, AdapterIdentityV1, ComparatorKindV1, CompetitionKeyV1,
    ObjectiveComparatorV1, ScoreV1,
};
use std::collections::BTreeSet;

pub(super) fn context(kind: ComparatorKindV1) -> RewardContextV1 {
    RewardContextV1 {
        episode_id: "episode-rwd".into(),
        candidate_id: "candidate-rwd".into(),
        submission_id: "submission-rwd".into(),
        comparable_base_id: "base-rwd".into(),
        competition: CompetitionKeyV1 {
            platform_id: "fixture".into(),
            competition_id: "competition".into(),
            field_id: "field".into(),
            benchmark_id: "benchmark".into(),
            profile_id: "profile".into(),
            hardware_id: "hardware".into(),
        },
        objective: ObjectiveComparatorV1 {
            objective_id: "latency".into(),
            version: "objective-v1".into(),
            kind,
        },
        profile: DeepCutProfileV1::embedded().identity().unwrap(),
    }
}

pub(super) fn official(
    context: &RewardContextV1,
    id: &str,
    revision: u64,
    candidate: &str,
    base: &str,
    corrects_binding_id: Option<String>,
) -> OfficialResultV1 {
    OfficialResultV1 {
        result_id: id.into(),
        result_revision: revision,
        episode_id: context.episode_id.clone(),
        candidate_id: context.candidate_id.clone(),
        submission_id: context.submission_id.clone(),
        comparable_base_id: context.comparable_base_id.clone(),
        competition: context.competition.clone(),
        objective: context.objective.clone(),
        profile: context.profile.clone(),
        candidate_score: ScoreV1::new(candidate).unwrap(),
        base_score: ScoreV1::new(base).unwrap(),
        provenance: OfficialProvenanceV1 {
            source: OfficialResultSourceV1::OfficialSubmissionResult,
            adapter: AdapterIdentityV1 {
                adapter_id: "fixture-adapter".into(),
                adapter_version: "v1".into(),
                runtime_sha256: crate::cut::sha256_hex(b"fixture-adapter"),
                capabilities: BTreeSet::from([AdapterCapabilityV1::Results]),
            },
            receipt_sha256: crate::cut::sha256_hex(format!("receipt:{id}").as_bytes()),
        },
        corrects_binding_id,
    }
}

#[test]
fn dc_rwd_001_higher_lower_tie_regression_golden_matrix() {
    let cases = [
        (
            ComparatorKindV1::HigherIsBetter,
            "12.5",
            "10.25",
            OrientedComparisonV1::Improvement,
            "2.25",
            1,
        ),
        (
            ComparatorKindV1::LowerIsBetter,
            "8",
            "10",
            OrientedComparisonV1::Improvement,
            "2",
            1,
        ),
        (
            ComparatorKindV1::HigherIsBetter,
            "-1.5",
            "-1.5",
            OrientedComparisonV1::Tie,
            "0",
            0,
        ),
        (
            ComparatorKindV1::HigherIsBetter,
            "8",
            "10",
            OrientedComparisonV1::Regression,
            "-2",
            -1,
        ),
        (
            ComparatorKindV1::LowerIsBetter,
            "12",
            "10",
            OrientedComparisonV1::Regression,
            "-2",
            -1,
        ),
    ];
    for (index, (kind, candidate, base, comparison, delta, reward)) in cases.into_iter().enumerate()
    {
        let context = context(kind);
        let result = official(
            &context,
            &format!("result-{index}"),
            1,
            candidate,
            base,
            None,
        );
        let (_, binding) = RewardLedgerV1::default()
            .bind_official(&context, result)
            .unwrap();
        assert_eq!(binding.oriented_comparison, comparison);
        assert_eq!(binding.oriented_delta.as_str(), delta);
        assert_eq!(binding.primary_reward, reward);
    }
}

#[test]
fn dc_rwd_002_delayed_official_result_overrides_only_shaping() {
    let context = context(ComparatorKindV1::HigherIsBetter);
    let mut ledger = RewardLedgerV1::default();
    ledger
        .record_shaping(ProvisionalShapingV1 {
            shaping_id: "local-measurement".into(),
            episode_id: context.episode_id.clone(),
            value_millis: 900,
            provenance_sha256: crate::cut::sha256_hex(b"local"),
        })
        .unwrap();
    assert!(matches!(
        ledger.effective_reward(&context.episode_id),
        Some(RewardSelectionV1::Provisional(_))
    ));
    ledger
        .bind_official(
            &context,
            official(&context, "official-delayed", 1, "8", "10", None),
        )
        .unwrap();
    assert!(matches!(
        ledger.effective_reward(&context.episode_id),
        Some(RewardSelectionV1::Official(binding)) if binding.primary_reward == -1
    ));
    assert_eq!(ledger.shaping().len(), 1);
}

#[test]
fn dc_rwd_003_correction_chain_is_append_only_and_exactly_once() {
    let context = context(ComparatorKindV1::HigherIsBetter);
    let mut ledger = RewardLedgerV1::default();
    let (_, first) = ledger
        .bind_official(
            &context,
            official(&context, "result-v1", 1, "12", "10", None),
        )
        .unwrap();
    let (_, correction) = ledger
        .bind_official(
            &context,
            official(
                &context,
                "result-v2",
                2,
                "8",
                "10",
                Some(first.binding_id.clone()),
            ),
        )
        .unwrap();
    assert_eq!(ledger.bindings().len(), 2);
    assert_eq!(ledger.bindings()[0], first);
    assert_eq!(correction.primary_reward, -1);
    assert_eq!(
        ledger.canonical_binding(&context.episode_id),
        Some(&correction)
    );
    let stale_correction = official(&context, "result-v3", 3, "11", "10", Some(first.binding_id));
    assert!(ledger.bind_official(&context, stale_correction).is_err());
    assert_eq!(ledger.bindings().len(), 2);
}

#[test]
fn dc_rwd_004_rejects_model_gate_self_report_and_unrelated_rank_credit() {
    let context = context(ComparatorKindV1::HigherIsBetter);
    let mut ledger = RewardLedgerV1::default();
    for source in [
        OfficialResultSourceV1::ModelSelfReport,
        OfficialResultSourceV1::ControlGate,
        OfficialResultSourceV1::UnrelatedRankMovement,
    ] {
        let mut result = official(&context, "not-official", 1, "12", "10", None);
        result.provenance.source = source;
        assert!(ledger.bind_official(&context, result).is_err());
    }
    assert!(ledger.bindings().is_empty());
}

#[test]
fn dc_rwd_005_rejects_incompatible_profile_hardware_objective() {
    let context = context(ComparatorKindV1::HigherIsBetter);
    for mutation in 0..3 {
        let mut result = official(&context, "incompatible", 1, "12", "10", None);
        match mutation {
            0 => result.competition.profile_id = "other-profile".into(),
            1 => result.competition.hardware_id = "other-hardware".into(),
            _ => result.objective.version = "objective-v2".into(),
        }
        assert!(
            RewardLedgerV1::default()
                .bind_official(&context, result)
                .is_err()
        );
    }
}

#[test]
fn dc_rwd_006_duplicate_official_event_is_idempotent() {
    let context = context(ComparatorKindV1::HigherIsBetter);
    let result = official(&context, "same-result", 1, "12", "10", None);
    let mut ledger = RewardLedgerV1::default();
    let (_, first) = ledger.bind_official(&context, result.clone()).unwrap();
    let (disposition, duplicate) = ledger.bind_official(&context, result).unwrap();
    assert_eq!(disposition, BindDispositionV1::Duplicate);
    assert_eq!(duplicate, first);
    assert_eq!(ledger.bindings().len(), 1);
    let conflict = official(&context, "same-result", 1, "13", "10", None);
    assert!(ledger.bind_official(&context, conflict).is_err());
    assert_eq!(ledger.bindings().len(), 1);
}
