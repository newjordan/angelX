use super::*;
use crate::drive::competition::board::{
    BOARD_OBSERVATION_SCHEMA_V1, BoardObservationKindV1, ObservationProvenanceV1,
    ObservationSourceV1,
};
use crate::drive::competition::schema::{AdapterCapabilityV1, ComparatorKindV1};

fn key() -> CompetitionKeyV1 {
    CompetitionKeyV1 {
        platform_id: "fixture".into(),
        competition_id: "adapter-contract".into(),
        field_id: "field".into(),
        benchmark_id: "bench".into(),
        profile_id: "profile".into(),
        hardware_id: "hardware".into(),
    }
}

fn identity() -> AdapterIdentityV1 {
    AdapterIdentityV1 {
        adapter_id: "scripted".into(),
        adapter_version: "1".into(),
        runtime_sha256: crate::knowledge::cut::sha256_hex(b"fixture-runtime"),
        capabilities: [
            AdapterCapabilityV1::Board,
            AdapterCapabilityV1::PersonalSubmissions,
            AdapterCapabilityV1::SourceAccess,
        ]
        .into_iter()
        .collect(),
    }
}

fn comparator(kind: ComparatorKindV1) -> ObjectiveComparatorV1 {
    ObjectiveComparatorV1 {
        objective_id: "score".into(),
        version: "1".into(),
        kind,
    }
}

fn board(id: &str) -> BoardObservationV1 {
    BoardObservationV1 {
        schema: BOARD_OBSERVATION_SCHEMA_V1.into(),
        observation_id: id.into(),
        competition: key(),
        kind: BoardObservationKindV1::Full,
        cursor: ObservationCursorV1::default(),
        changes: Vec::new(),
        provenance: ObservationProvenanceV1 {
            adapter: identity(),
            source: ObservationSourceV1::Fixture,
            observed_at_ms: 1,
            platform_event_at_ms: None,
            raw_sha256: crate::knowledge::cut::sha256_hex(id.as_bytes()),
        },
    }
}

fn scripted(observation: BoardObservationV1) -> ScriptedAdapterV1 {
    ScriptedAdapterV1::new(
        identity(),
        CompetitionIdentityV1 {
            competition: key(),
            comparator: comparator(ComparatorKindV1::HigherIsBetter),
        },
        vec![Ok(observation)],
    )
}

#[test]
fn shared_fixture_contract_matrix() {
    let personal = PersonalSubmissionObservationV1 {
        competition: key(),
        cursor: ObservationCursorV1::default(),
        entries: Vec::new(),
        observed_at_ms: 1,
        raw_sha256: crate::knowledge::cut::sha256_hex(b"personal"),
    };
    let source = SourceAccessObservationV1 {
        competition: key(),
        cursor: ObservationCursorV1::default(),
        claims: Vec::new(),
        observed_at_ms: 1,
        raw_sha256: crate::knowledge::cut::sha256_hex(b"source"),
    };
    let mut adapter = scripted(board("board"))
        .with_personal(vec![Ok(personal)])
        .with_source(vec![Ok(source)]);
    assert_eq!(adapter.identify_competition().unwrap().competition, key());
    assert_eq!(adapter.fetch_board(None).unwrap().observation_id, "board");
    assert!(adapter.fetch_personal_submissions(None).is_ok());
    assert!(adapter.observe_source_access(None).is_ok());
    let one = ScoreV1::new("1").unwrap();
    let two = ScoreV1::new("2").unwrap();
    assert_eq!(
        adapter
            .compare_scores(&comparator(ComparatorKindV1::HigherIsBetter), &two, &one)
            .unwrap(),
        Ordering::Greater
    );
    assert_eq!(
        adapter
            .compare_scores(&comparator(ComparatorKindV1::LowerIsBetter), &one, &two)
            .unwrap(),
        Ordering::Greater
    );
}

#[test]
fn shadow_adapter_is_read_only_and_matches_canonical_fixture_digest() {
    let expected = board("shadow");
    let digest = expected.provenance.raw_sha256.clone();
    let mut adapter = scripted(expected).shadow();
    let actual = adapter.fetch_board(None).unwrap().provenance.raw_sha256;
    assert!(adapter.shadow);
    assert_eq!(actual, digest);
    assert_eq!(adapter.calls(), vec![FixtureCallV1::FetchBoard]);
}
