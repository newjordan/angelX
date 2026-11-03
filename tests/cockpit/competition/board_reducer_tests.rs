use super::*;
use crate::drive::competition::adapters::fixture::{
    FixtureCallV1, MemoryRawBoardJournalV1, ScriptedAdapterV1,
};
use crate::drive::competition::adapters::{AdapterFailureV1, CompetitionIdentityV1};
use crate::drive::competition::schema::{
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
        runtime_sha256: crate::knowledge::cut::sha256_hex(b"fixture-runtime"),
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
            raw_sha256: crate::knowledge::cut::sha256_hex(id.as_bytes()),
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
            version_sha256: crate::knowledge::cut::sha256_hex(b"missing"),
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

/// Every feature combination the disposition reads, including sequences above
/// a U32 and the saturated end of the cursor.
fn decision_table() -> Vec<ObservationDecisionInputV1> {
    let kinds = [BoardObservationKindV1::Full, BoardObservationKindV1::Delta];
    let priors = [
        None,
        Some(0),
        Some(1),
        Some(41),
        Some(u64::from(u32::MAX)),
        Some(u64::from(u32::MAX) + 1),
        Some(u64::MAX),
    ];
    let incomings = [
        None,
        Some(0),
        Some(1),
        Some(41),
        Some(42),
        Some(u64::from(u32::MAX)),
        Some(u64::from(u32::MAX) + 1),
        Some(u64::MAX),
    ];
    let mut rows = Vec::new();
    for kind in kinds {
        for prior in priors {
            for incoming in incomings {
                for refresh_pending in [false, true] {
                    for has_board in [false, true] {
                        for duplicate in [false, true] {
                            for identity_conflict in [false, true] {
                                rows.push(ObservationDecisionInputV1::new(
                                    identity_conflict,
                                    duplicate,
                                    kind,
                                    prior,
                                    incoming,
                                    refresh_pending,
                                    has_board,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    rows
}

/// The shipped leg and the native rule must agree on every disposition, and the
/// leg must be the one that answered.
#[test]
fn live_board_reducer_leg_matches_every_native_disposition() {
    use crate::agent::harness::live_policy::{LegSource, decide_shipped};
    let rows = decision_table();
    assert!(rows.len() >= 30, "the parity table needs real coverage");
    let mut checked = 0;
    for row in &rows {
        let features = serde_json::to_value(row).unwrap();
        let (verdict, source) =
            decide_shipped("board-reducer", &features, |wire: &ObservationVerdictV1| {
                row.check(wire.decision)
            });
        assert_eq!(
            verdict.decision,
            row.native(),
            "{row:?} left the native disposition"
        );
        assert!(
            matches!(source, LegSource::Shipped { .. }),
            "{row:?} kept the {} leg: {source:?}",
            source.as_str()
        );
        checked += 1;
    }
    assert!(checked >= 30, "only {checked} inputs were compared");
}

/// A throwaway policy module, so a deliberately out-of-contract leg can be
/// pinned without touching the shipped one.
fn temporary_board_leg(id: &str, render: &str) -> angel_live::Program {
    let root = std::env::temp_dir().join(format!("angel-board-leg-{}-{id}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let module = root.join(id);
    std::fs::create_dir_all(&module).unwrap();
    std::fs::write(
        module.join("module.json"),
        format!(
            r#"{{"api_version":1,"id":"{id}","kind":"policy","entry":"index.js","probe":{{}}}}"#
        ),
    )
    .unwrap();
    std::fs::write(module.join("index.js"), render).unwrap();
    let mut registry =
        angel_live::Registry::validated(root, crate::agent::hotload::evaluate, |_, _| Ok(()));
    registry.refresh(true);
    registry
        .snapshot()
        .policies
        .get(id)
        .cloned()
        .unwrap_or_else(|| panic!("fixture leg {id} did not publish"))
}

/// A verdict outside the contract is rejected and the native rule answers, so a
/// bad hot edit never moves the canonical board.
#[test]
fn an_out_of_contract_disposition_falls_back_to_the_native_rule() {
    use crate::agent::harness::live_policy::{LegSource, decide_with};
    let leg = temporary_board_leg(
        "board-reducer-gap",
        "module.exports={apiVersion:1,render:()=>({decision:'gap'})}",
    );
    // A full snapshot rebuilds the board, so `gap` is not an admissible verdict.
    let row = ObservationDecisionInputV1::new(
        false,
        false,
        BoardObservationKindV1::Full,
        None,
        Some(9),
        false,
        true,
    );
    let features = serde_json::to_value(row).unwrap();
    let (verdict, source) = decide_with(
        Some(leg),
        "board-reducer-gap",
        &features,
        |wire: &ObservationVerdictV1| row.check(wire.decision),
        || ObservationVerdictV1 {
            decision: row.native(),
        },
    );
    assert_eq!(verdict.decision, ObservationDecisionV1::Replace);
    assert!(
        matches!(source, LegSource::LiveFallback { .. }),
        "the out-of-contract leg answered: {}",
        source.as_str()
    );
}

#[test]
fn board_reducer_leg_decides_in_microseconds() {
    use crate::agent::harness::live_policy::decide_shipped;
    let row = ObservationDecisionInputV1::new(
        false,
        false,
        BoardObservationKindV1::Delta,
        Some(41),
        Some(42),
        false,
        true,
    );
    let features = serde_json::to_value(row).unwrap();
    let mut samples: Vec<std::time::Duration> = Vec::with_capacity(200);
    for _ in 0..200 {
        let started = std::time::Instant::now();
        let (verdict, source) =
            decide_shipped("board-reducer", &features, |wire: &ObservationVerdictV1| {
                row.check(wire.decision)
            });
        samples.push(started.elapsed());
        assert_eq!(source.as_str(), "shipped");
        assert_eq!(verdict.decision, ObservationDecisionV1::Replace);
    }
    samples.sort();
    eprintln!(
        "board-reducer leg: {:.1} µs median, {:.1} µs p95 over 200 decisions",
        samples[100].as_secs_f64() * 1e6,
        samples[190].as_secs_f64() * 1e6
    );
}

// --- the parity oracle ------------------------------------------------------
//
// The native disposition rule, moved here out of production: the shipped
// `board-reducer` leg decides every disposition now, and this copy exists so
// the parity table can prove the two still agree.

impl ObservationDecisionInputV1 {
    fn sequence(hi: u32, lo: u32) -> u64 {
        (u64::from(hi) << 32) | u64::from(lo)
    }

    /// The native rule, in the order the reducer applies it: duplicate, then
    /// stale ordering, then a sequence gap, then replacement.
    fn native(&self) -> ObservationDecisionV1 {
        if self.identity_conflict {
            return ObservationDecisionV1::Conflict;
        }
        if self.duplicate {
            return ObservationDecisionV1::Duplicate;
        }
        let ordered = (self.has_prior && self.has_incoming).then(|| {
            (
                Self::sequence(self.prior_hi, self.prior_lo),
                Self::sequence(self.incoming_hi, self.incoming_lo),
            )
        });
        if let Some((prior, incoming)) = ordered {
            let reordered = if self.full_snapshot {
                incoming < prior
            } else {
                incoming <= prior
            };
            if reordered {
                return ObservationDecisionV1::Stale;
            }
        }
        let delta = !self.full_snapshot;
        let gapped = delta
            && (self.refresh_pending
                || ordered.is_some_and(|(prior, incoming)| incoming > prior.saturating_add(1))
                || !self.has_board);
        if gapped {
            return ObservationDecisionV1::Gap;
        }
        ObservationDecisionV1::Replace
    }
}
