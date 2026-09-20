use super::tests::{adapter, comparator, delta, entry, full, identity, key, reduce, reduce_error};
use super::*;
use crate::drive::competition::adapters::fixture::{MemoryRawBoardJournalV1, ScriptedAdapterV1};
use crate::drive::competition::adapters::{
    AdapterFailureClassV1, AdapterFailureV1, CompetitionIdentityV1,
};
use crate::drive::competition::schema::{DirectorHealthStateV1, RetryPolicyV1};

#[test]
fn retry_delay_and_never_retry_have_distinct_durable_dispositions() {
    let timeout = AdapterFailureV1 {
        class: AdapterFailureClassV1::Transport,
        retry: RetryPolicyV1::AfterMs(50),
        detail_sha256: crate::knowledge::cut::sha256_hex(b"timeout"),
        provenance_sha256: crate::knowledge::cut::sha256_hex(b"fixture"),
    };
    let mut adapter = adapter(vec![
        Ok(full("initial", 1, vec![entry("leader", "10")])),
        Err(timeout),
    ]);
    let mut journal = MemoryRawBoardJournalV1::default();
    let mut reducer = BoardReducerV1::default();
    reducer
        .engage("campaign", &mut adapter, &mut journal, 0)
        .unwrap()
        .unwrap();
    let retry = reducer
        .engage("campaign", &mut adapter, &mut journal, 100)
        .unwrap()
        .unwrap_err();
    assert_eq!(retry.health, DirectorHealthStateV1::Retrying);
    assert_eq!(retry.next.next_attempt_at_ms, 150);
    let terminal = reducer
        .engage("campaign", &mut adapter, &mut journal, 200)
        .unwrap()
        .unwrap_err();
    assert_eq!(terminal.health, DirectorHealthStateV1::NeedsAttention);
    assert_eq!(terminal.next.action, "remediate_board_adapter");
    assert_eq!(terminal.next.next_attempt_at_ms, u64::MAX);
    assert_eq!(terminal.canonical.unwrap().board_epoch, 1);
}

#[test]
fn gap_latch_holds_late_delta_and_gap_until_authoritative_full() {
    let adapter = adapter(Vec::new());
    let mut journal = MemoryRawBoardJournalV1::default();
    let mut reducer = BoardReducerV1::default();
    reduce(
        &mut reducer,
        &adapter,
        &mut journal,
        full("seq-one", 1, vec![entry("leader", "10")]),
    );
    let gap = delta("seq-three", 3, entry("challenger", "30"));
    assert_eq!(
        reduce(&mut reducer, &adapter, &mut journal, gap.clone()).effect,
        BoardReduceEffectV1::GapDetected
    );
    let late = reduce(
        &mut reducer,
        &adapter,
        &mut journal,
        delta("seq-two", 2, entry("middle", "20")),
    );
    assert_eq!(late.effect, BoardReduceEffectV1::GapDetected);
    let board = late.canonical.unwrap();
    assert_eq!(board.frontier.global_target.unwrap().score.as_str(), "10");
    assert!(matches!(
        board.freshness,
        BoardFreshnessV1::Unconfirmed { .. }
    ));
    assert_eq!(
        reduce(&mut reducer, &adapter, &mut journal, gap).effect,
        BoardReduceEffectV1::GapDetected
    );
    let refreshed = reduce(
        &mut reducer,
        &adapter,
        &mut journal,
        full("seq-three-full", 3, vec![entry("challenger", "30")]),
    );
    let board = refreshed.canonical.unwrap();
    assert!(matches!(board.freshness, BoardFreshnessV1::Fresh));
    assert_eq!(board.frontier.global_target.unwrap().score.as_str(), "30");
}

#[test]
fn malformed_source_claim_and_capability_conflict_remain_raw_evidence() {
    let adapter = adapter(Vec::new());
    let mut bad = entry("bad-source", "99");
    bad.source = Some(SourceAccessClaimV1 {
        source_id: "source".into(),
        commit_oid: "not-a-digest".into(),
        tree_oid: crate::knowledge::cut::sha256_hex(b"tree"),
        workspace_sha256: crate::knowledge::cut::sha256_hex(b"workspace"),
        access_proof_sha256: crate::knowledge::cut::sha256_hex(b"proof"),
    });
    let mut journal = MemoryRawBoardJournalV1::default();
    let mut reducer = BoardReducerV1::default();
    let error = reduce_error(
        &mut reducer,
        &adapter,
        &mut journal,
        full("bad-source", 1, vec![bad]),
    );
    assert!(error.is_err());
    assert_eq!(journal.observations().len(), 1);
    assert!(reducer.current().is_none());

    let mut no_board = identity();
    no_board.capabilities.clear();
    let mut observation = full("no-capability", 1, vec![entry("leader", "10")]);
    observation.provenance.adapter = no_board.clone();
    let adapter = ScriptedAdapterV1::new(
        no_board,
        CompetitionIdentityV1 {
            competition: key(),
            comparator: comparator(),
        },
        Vec::new(),
    );
    let mut journal = MemoryRawBoardJournalV1::default();
    assert!(
        reduce_error(
            &mut BoardReducerV1::default(),
            &adapter,
            &mut journal,
            observation
        )
        .is_err()
    );
    assert_eq!(journal.observations().len(), 1);
}

#[test]
fn durable_raw_replay_reconstructs_epoch_revision_and_idempotency() {
    let adapter = adapter(Vec::new());
    let mut journal = MemoryRawBoardJournalV1::default();
    let mut original = BoardReducerV1::default();
    let observations = [
        full("one", 1, vec![entry("leader", "10")]),
        full("two", 2, vec![entry("leader", "10")]),
        delta("three", 3, entry("challenger", "20")),
    ];
    for observation in observations.iter().cloned() {
        reduce(&mut original, &adapter, &mut journal, observation);
    }
    let mut restored = BoardReducerV1::default();
    restored
        .restore_from_persisted("campaign", comparator(), &adapter, journal.observations())
        .unwrap();
    assert_eq!(restored.current(), original.current());
    let duplicate = reduce(
        &mut restored,
        &adapter,
        &mut journal,
        observations[2].clone(),
    );
    assert_eq!(duplicate.effect, BoardReduceEffectV1::DuplicateObservation);
    let board = duplicate.canonical.unwrap();
    assert_eq!((board.board_epoch, board.observation_revision), (2, 3));
}

#[test]
fn failed_restore_preserves_last_good_and_idempotency_state() {
    let adapter = adapter(Vec::new());
    let mut journal = MemoryRawBoardJournalV1::default();
    let mut reducer = BoardReducerV1::default();
    let seed = full("seed", 5, vec![entry("leader", "10")]);
    reduce(&mut reducer, &adapter, &mut journal, seed.clone());
    let before = reducer.current.clone();
    let applied_before = reducer.applied_observations.clone();
    let pending_before = reducer.pending_gap_observations.clone();
    let due_before = reducer.gap_refresh_due_ms;
    let valid = full("replay-valid", 1, vec![entry("other", "20")]);
    let mut malformed = full("replay-malformed", 2, vec![entry("bad", "30")]);
    malformed.provenance.raw_sha256 = "not-a-digest".into();
    assert!(
        reducer
            .restore_from_persisted("campaign", comparator(), &adapter, &[valid, malformed])
            .is_err()
    );
    assert_eq!(reducer.current, before);
    assert_eq!(reducer.applied_observations, applied_before);
    assert_eq!(reducer.pending_gap_observations, pending_before);
    assert_eq!(reducer.gap_refresh_due_ms, due_before);
    let duplicate = reduce(&mut reducer, &adapter, &mut journal, seed);
    assert_eq!(duplicate.effect, BoardReduceEffectV1::DuplicateObservation);
    let board = duplicate.canonical.unwrap();
    assert_eq!((board.board_epoch, board.observation_revision), (1, 1));
}
