use super::*;
use crate::competition::adapters::fixture::{
    FixtureCallV1, MemoryRawBoardJournalV1, ScriptedAdapterV1,
};
use crate::competition::adapters::{AdapterFailureV1, CompetitionIdentityV1};
use crate::competition::schema::{
    AdapterCapabilityV1, AdapterIdentityV1, ComparatorKindV1, CompetitionKeyV1, ScoreV1,
};

pub(super) fn key() -> CompetitionKeyV1 {
    CompetitionKeyV1 {
        platform_id: "fixture".into(),
        competition_id: "crown".into(),
        field_id: "kernels".into(),
        benchmark_id: "gemm".into(),
        profile_id: "official".into(),
        hardware_id: "target".into(),
    }
}

pub(super) fn comparator() -> ObjectiveComparatorV1 {
    ObjectiveComparatorV1 {
        objective_id: "throughput".into(),
        version: "1".into(),
        kind: ComparatorKindV1::HigherIsBetter,
    }
}

pub(super) fn identity() -> AdapterIdentityV1 {
    AdapterIdentityV1 {
        adapter_id: "scripted".into(),
        adapter_version: "1".into(),
        runtime_sha256: crate::cut::sha256_hex(b"fixture-runtime"),
        capabilities: [AdapterCapabilityV1::Board].into_iter().collect(),
    }
}

pub(super) fn entry(id: &str, score: &str) -> BoardEntryV1 {
    BoardEntryV1 {
        entry_id: id.into(),
        participant_id: id.into(),
        submission_id: Some(format!("submission-{id}")),
        rank: None,
        score: ScoreV1::new(score).unwrap(),
        personal: false,
        source: None,
    }
}

pub(super) fn observation(
    id: &str,
    kind: BoardObservationKindV1,
    sequence: u64,
    changes: Vec<BoardChangeV1>,
) -> BoardObservationV1 {
    BoardObservationV1 {
        schema: BOARD_OBSERVATION_SCHEMA_V1.into(),
        observation_id: id.into(),
        competition: key(),
        kind,
        cursor: ObservationCursorV1 {
            opaque: Some(format!("cursor-{sequence}")),
            sequence: Some(sequence),
        },
        changes,
        provenance: ObservationProvenanceV1 {
            adapter: identity(),
            source: ObservationSourceV1::Fixture,
            observed_at_ms: sequence * 10,
            platform_event_at_ms: Some(sequence * 10 - 1),
            raw_sha256: crate::cut::sha256_hex(id.as_bytes()),
        },
    }
}

pub(super) fn full(id: &str, sequence: u64, entries: Vec<BoardEntryV1>) -> BoardObservationV1 {
    observation(
        id,
        BoardObservationKindV1::Full,
        sequence,
        entries
            .into_iter()
            .map(|entry| BoardChangeV1::Upsert {
                entry: Box::new(entry),
            })
            .collect(),
    )
}

pub(super) fn delta(id: &str, sequence: u64, entry: BoardEntryV1) -> BoardObservationV1 {
    observation(
        id,
        BoardObservationKindV1::Delta,
        sequence,
        vec![BoardChangeV1::Upsert {
            entry: Box::new(entry),
        }],
    )
}

pub(super) fn adapter(
    board: Vec<Result<BoardObservationV1, AdapterFailureV1>>,
) -> ScriptedAdapterV1 {
    ScriptedAdapterV1::new(
        identity(),
        CompetitionIdentityV1 {
            competition: key(),
            comparator: comparator(),
        },
        board,
    )
}

pub(super) fn reduce(
    reducer: &mut BoardReducerV1,
    adapter: &ScriptedAdapterV1,
    journal: &mut MemoryRawBoardJournalV1,
    observation: BoardObservationV1,
) -> BoardReduceOutcomeV1 {
    reducer
        .observe("campaign", comparator(), adapter, journal, observation)
        .unwrap()
}

pub(super) fn reduce_error(
    reducer: &mut BoardReducerV1,
    adapter: &ScriptedAdapterV1,
    journal: &mut MemoryRawBoardJournalV1,
    observation: BoardObservationV1,
) -> Result<BoardReduceOutcomeV1, BoardReduceErrorV1> {
    reducer.observe("campaign", comparator(), adapter, journal, observation)
}

#[test]
fn engagement_requests_refresh_before_first_scheduler_quantum() {
    let mut adapter = adapter(vec![Ok(full("initial", 1, vec![entry("leader", "10")]))]);
    let mut journal = MemoryRawBoardJournalV1::default();
    let outcome = BoardReducerV1::default()
        .engage("campaign", &mut adapter, &mut journal, 0)
        .unwrap()
        .unwrap();
    assert_eq!(outcome.effect, BoardReduceEffectV1::Initialized);
    assert_eq!(journal.observations().len(), 1);
    assert_eq!(
        adapter.calls()[..2],
        [FixtureCallV1::Identify, FixtureCallV1::FetchBoard]
    );
}

#[test]
fn duplicate_observation_refreshes_without_epoch_advance() {
    let adapter = adapter(Vec::new());
    let mut journal = MemoryRawBoardJournalV1::default();
    let mut reducer = BoardReducerV1::default();
    let entries = vec![entry("leader", "10"), entry("other", "9")];
    reduce(
        &mut reducer,
        &adapter,
        &mut journal,
        full("first", 1, entries.clone()),
    );
    let refresh = full("refresh", 2, entries);
    let refreshed = reduce(&mut reducer, &adapter, &mut journal, refresh.clone());
    let board = refreshed.canonical.as_ref().unwrap();
    assert_eq!((board.board_epoch, board.observation_revision), (1, 2));
    let duplicate = reduce(&mut reducer, &adapter, &mut journal, refresh);
    assert_eq!(duplicate.effect, BoardReduceEffectV1::DuplicateObservation);
    assert_eq!(
        duplicate.raw_receipt.status,
        RawObservationStatusV1::AlreadyPresent
    );
}

#[test]
fn reordered_delta_cannot_roll_back_canonical_state() {
    let adapter = adapter(Vec::new());
    let mut journal = MemoryRawBoardJournalV1::default();
    let mut reducer = BoardReducerV1::default();
    reduce(
        &mut reducer,
        &adapter,
        &mut journal,
        full("seq-five", 5, vec![entry("leader", "10")]),
    );
    let stale = reduce(
        &mut reducer,
        &adapter,
        &mut journal,
        delta("seq-four", 4, entry("stale", "99")),
    );
    assert_eq!(stale.effect, BoardReduceEffectV1::ReorderedIgnored);
    assert_eq!(
        stale
            .canonical
            .unwrap()
            .frontier
            .global_target
            .unwrap()
            .score
            .as_str(),
        "10"
    );
}

#[test]
fn raw_observation_is_persisted_before_comparator_failure() {
    let adapter = adapter(Vec::new());
    let mut journal = MemoryRawBoardJournalV1::default();
    let custom = ObjectiveComparatorV1 {
        objective_id: "custom".into(),
        version: "1".into(),
        kind: ComparatorKindV1::AdapterDefined {
            contract_id: "missing".into(),
            version_sha256: crate::cut::sha256_hex(b"missing"),
        },
    };
    let error = BoardReducerV1::default()
        .observe(
            "campaign",
            custom,
            &adapter,
            &mut journal,
            full("raw-first", 1, vec![entry("one", "1"), entry("two", "2")]),
        )
        .unwrap_err();
    assert!(matches!(error, BoardReduceErrorV1::Adapter(_)));
    assert_eq!(journal.observations().len(), 1);
}
