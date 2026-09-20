use super::deep_cut_policy::{
    DeepCutDirectionV1, DirectionDecisionBasisV1, DirectionEvidenceUseKindV1,
    DirectionEvidenceUseV1, allocate_direction,
};
use super::patterns::{FieldPatternMemoryV1, PatternOutcomeV1};
use super::patterns_adversarial_tests::corrected_memory;
use super::patterns_tests::key;

#[test]
fn dc_pat_005_direction_allocation_never_mixes_unmarked_transfer() {
    let source = key();
    let mut transfer_target = key();
    transfer_target.hardware_id = "transfer-target".into();
    let mut local_target = key();
    local_target.mechanism = "local-alternative".into();
    let mut memory = FieldPatternMemoryV1::default();
    memory
        .record_transfer(
            "transfer-evidence".into(),
            "episode-transfer".into(),
            source,
            transfer_target.clone(),
            PatternOutcomeV1::Improvement,
            100,
            1,
        )
        .unwrap();
    memory
        .record_operational(
            "local-evidence".into(),
            "episode-local".into(),
            local_target.clone(),
            PatternOutcomeV1::Timeout,
            100,
            1,
        )
        .unwrap();

    let silent_transfer = DeepCutDirectionV1 {
        direction_id: "silent-transfer".into(),
        target_key: transfer_target.clone(),
        evidence: vec![DirectionEvidenceUseV1 {
            evidence_id: "transfer-evidence".into(),
            kind: DirectionEvidenceUseKindV1::TargetEvidence,
        }],
    };
    let local_alternative = DeepCutDirectionV1 {
        direction_id: "local-alternative".into(),
        target_key: local_target,
        evidence: vec![DirectionEvidenceUseV1 {
            evidence_id: "local-evidence".into(),
            kind: DirectionEvidenceUseKindV1::TargetEvidence,
        }],
    };
    let decision = allocate_direction(
        &memory,
        &[silent_transfer.clone(), local_alternative],
        "fresh-direction",
        90,
    )
    .unwrap();
    assert_eq!(decision.selected_direction_id, "local-alternative");
    assert_eq!(decision.basis, DirectionDecisionBasisV1::TargetEvidence);
    assert_eq!(decision.next.action, "explore-direction:local-alternative");

    let fallback = allocate_direction(&memory, &[silent_transfer], "fresh-direction", 91).unwrap();
    assert_eq!(fallback.selected_direction_id, "fresh-direction");
    assert_eq!(fallback.basis, DirectionDecisionBasisV1::UntriedFallback);
    assert_eq!(fallback.next.next_attempt_at_ms, 91);

    let marked = DeepCutDirectionV1 {
        direction_id: "marked-transfer".into(),
        target_key: transfer_target,
        evidence: vec![DirectionEvidenceUseV1 {
            evidence_id: "transfer-evidence".into(),
            kind: DirectionEvidenceUseKindV1::MarkedProvisionalTransfer,
        }],
    };
    let explicit = allocate_direction(&memory, &[marked], "fresh-direction", 92).unwrap();
    assert_eq!(explicit.selected_direction_id, "marked-transfer");
    assert_eq!(
        explicit.basis,
        DirectionDecisionBasisV1::ExplicitProvisionalTransfer
    );

    let mut tampered = serde_json::to_value(&memory).unwrap();
    tampered["fields"][0]["patterns"][0]["aggregate"]["improvements"] = 99.into();
    let tampered = serde_json::from_value(tampered).unwrap();
    assert!(allocate_direction(&tampered, &[], "fresh-direction", 93).is_err());
}

#[test]
fn allocator_after_correction_uses_only_canonical_active_evidence() {
    let (memory, target_key) = corrected_memory();
    let old_id = memory.evidence_records()[0].evidence_id.clone();
    let active_id = memory.evidence_records()[1].evidence_id.clone();
    assert!(memory.canonical_active_evidence(&old_id).unwrap().is_none());
    assert!(
        memory
            .canonical_active_evidence(&active_id)
            .unwrap()
            .is_some()
    );

    let usage = |evidence_id| DirectionEvidenceUseV1 {
        evidence_id,
        kind: DirectionEvidenceUseKindV1::TargetEvidence,
    };
    let superseded = DeepCutDirectionV1 {
        direction_id: "superseded-direction".into(),
        target_key: target_key.clone(),
        evidence: vec![usage(old_id.clone())],
    };
    let mixed = DeepCutDirectionV1 {
        direction_id: "mixed-direction".into(),
        target_key: target_key.clone(),
        evidence: vec![usage(old_id), usage(active_id.clone())],
    };
    let canonical = DeepCutDirectionV1 {
        direction_id: "canonical-direction".into(),
        target_key,
        evidence: vec![usage(active_id)],
    };
    let decision = allocate_direction(
        &memory,
        &[superseded.clone(), mixed.clone(), canonical],
        "fresh-direction",
        100,
    )
    .unwrap();
    assert_eq!(decision.selected_direction_id, "canonical-direction");
    assert_eq!(decision.basis, DirectionDecisionBasisV1::TargetEvidence);

    let fallback =
        allocate_direction(&memory, &[superseded, mixed], "fresh-direction", 101).unwrap();
    assert_eq!(fallback.selected_direction_id, "fresh-direction");
    assert_eq!(fallback.basis, DirectionDecisionBasisV1::UntriedFallback);
}
