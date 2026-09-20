use super::schema::*;

fn key() -> CompetitionKeyV1 {
    CompetitionKeyV1 {
        platform_id: "fixture".into(),
        competition_id: "contest".into(),
        field_id: "kernels".into(),
        benchmark_id: "gemm".into(),
        profile_id: "b200".into(),
        hardware_id: "b200-sxm".into(),
    }
}

#[test]
fn shared_identity_and_digest_validation_fails_closed() {
    assert_eq!(key().validate(), Ok(()));
    let mut blank = key();
    blank.hardware_id.clear();
    assert!(blank.validate().is_err());

    let explicit = ObjectiveComparatorV1 {
        objective_id: "latency".into(),
        version: "v1".into(),
        kind: ComparatorKindV1::AdapterDefined {
            contract_id: "official".into(),
            version_sha256: "A".repeat(64),
        },
    };
    assert!(explicit.validate().is_err());
}

#[test]
fn action_intent_requires_canonical_identity() {
    let intent = ActionIntentV1 {
        action_key: "a".repeat(64),
        campaign_id: "campaign".into(),
        competition: key(),
        kind: ActionKindV1::StartEpisode,
        subject_id: "episode".into(),
        payload_sha256: "b".repeat(64),
        intent_version: "v1".into(),
    };
    assert_eq!(intent.validate(), Ok(()));
    let mut malformed = intent;
    malformed.payload_sha256 = "short".into();
    assert!(malformed.validate().is_err());
}

#[test]
fn health_requires_a_real_scheduled_action() {
    let health = DirectorHealthV1 {
        schema: COMPETITION_SCHEMA_V1.into(),
        state: DirectorHealthStateV1::NeedsAttention,
        last_good_revision: Some(1),
        reason: Some("unsupported board capability".into()),
        next: ScheduledActionV1 {
            action: " ".into(),
            next_attempt_at_ms: 10,
        },
        updated_at_ms: 9,
    };
    assert!(health.validate().is_err());
}
