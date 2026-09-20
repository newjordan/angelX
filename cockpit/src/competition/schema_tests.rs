use super::schema::*;
use std::cmp::Ordering;

fn comparator(kind: ComparatorKindV1) -> ObjectiveComparatorV1 {
    ObjectiveComparatorV1 {
        objective_id: "latency".into(),
        version: "v1".into(),
        kind,
    }
}

#[test]
fn canonical_scores_compare_without_binary_float_persistence() {
    let low = ScoreV1::new("0.09").unwrap();
    let high = ScoreV1::new("10.01").unwrap();
    assert_eq!(
        comparator(ComparatorKindV1::HigherIsBetter).compare(&high, &low),
        Ok(Ordering::Greater)
    );
    assert_eq!(
        comparator(ComparatorKindV1::LowerIsBetter).compare(&low, &high),
        Ok(Ordering::Greater)
    );
    assert!(ScoreV1::new("1.0").is_err());
    assert!(ScoreV1::new("NaN").is_err());
    assert!(serde_json::from_str::<ScoreV1>("\"01\"").is_err());
    assert_eq!(high.as_str(), "10.01");
}

#[test]
fn adapter_defined_comparison_cannot_be_guessed_locally() {
    let explicit = comparator(ComparatorKindV1::AdapterDefined {
        contract_id: "official".into(),
        version_sha256: "a".repeat(64),
    });
    assert_eq!(
        explicit.compare(&ScoreV1::new("1").unwrap(), &ScoreV1::new("2").unwrap()),
        Err(CompareError::AdapterRequired)
    );
}

#[test]
fn director_health_has_only_fail_forward_states_and_a_next_action() {
    let health = DirectorHealthV1 {
        schema: COMPETITION_SCHEMA_V1.into(),
        state: DirectorHealthStateV1::Retrying,
        last_good_revision: Some(7),
        reason: Some("board transport".into()),
        next: ScheduledActionV1 {
            action: "refresh_board".into(),
            next_attempt_at_ms: 42,
        },
        updated_at_ms: 40,
    };
    let encoded = serde_json::to_string(&health).unwrap();
    assert_eq!(health.validate(), Ok(()));
    assert!(encoded.contains("retrying"));
    assert!(!encoded.contains("blocked"));
    assert!(!encoded.contains("stopped"));
    assert!(!encoded.contains("frozen"));
}

#[test]
fn action_transitions_require_reconciliation_or_explicit_retry() {
    assert!(ActionPhaseV1::Planned.allows(ActionPhaseV1::Started, false));
    assert!(ActionPhaseV1::Started.allows(ActionPhaseV1::Ambiguous, false));
    assert!(ActionPhaseV1::Ambiguous.allows(ActionPhaseV1::Completed, false));
    assert!(!ActionPhaseV1::Ambiguous.allows(ActionPhaseV1::Started, true));
    assert!(!ActionPhaseV1::Failed.allows(ActionPhaseV1::Planned, false));
    assert!(ActionPhaseV1::Failed.allows(ActionPhaseV1::Planned, true));
    assert!(!ActionPhaseV1::Completed.allows(ActionPhaseV1::Started, true));
}
