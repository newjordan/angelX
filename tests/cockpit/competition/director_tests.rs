use super::adapters::{AdapterFailureClassV1, AdapterFailureV1};
use super::board::{
    BoardFetchFailureV1, BoardFreshnessV1, BoardReduceEffectV1, BoardReduceOutcomeV1,
    RawObservationReceiptV1, RawObservationStatusV1,
};
use super::candidate::{originate_candidate, port_replay_candidate};
use super::candidate_store::CandidateEligibilityStatusV1;
use super::candidate_tests::{artifact, board, digest, evidence};
use super::director::CompetitionDirectorStateV1;
use super::director_services::{DirectorServiceKindV1, DirectorTransitionV1};
use super::schema::{DirectorHealthStateV1, RetryPolicyV1, ScheduledActionV1};

pub(super) fn outcome(
    board: super::board::CanonicalBoardV1,
    effect: BoardReduceEffectV1,
) -> BoardReduceOutcomeV1 {
    BoardReduceOutcomeV1 {
        raw_receipt: RawObservationReceiptV1 {
            observation_id: board.latest_observation_id.clone(),
            raw_sha256: board.latest_provenance.raw_sha256.clone(),
            journal_sequence: board.observation_revision,
            status: RawObservationStatusV1::Persisted,
        },
        canonical: Some(board),
        effect,
        refresh: None,
    }
}

fn install_first(director: &mut CompetitionDirectorStateV1) -> DirectorTransitionV1 {
    director
        .apply_board_outcome(
            &outcome(
                board(1, "100", "source-a", "60"),
                BoardReduceEffectV1::Initialized,
            ),
            1,
        )
        .unwrap()
}

/// FG-BOARD-001 engagement_requests_refresh_before_first_scheduler_quantum.
#[test]
fn engagement_requests_refresh_before_first_scheduler_quantum() {
    let mut director = CompetitionDirectorStateV1::engage("campaign", 0).unwrap();
    assert_eq!(director.health.state, DirectorHealthStateV1::Retrying);
    assert_eq!(director.health.next.action, "full_board_refresh");
    assert_eq!(director.health.next.next_attempt_at_ms, 0);
    assert!(matches!(
        director.services[0].kind,
        DirectorServiceKindV1::RefreshBoard { full: true }
    ));
    assert!(
        director
            .services
            .iter()
            .any(|intent| matches!(intent.kind, DirectorServiceKindV1::ContinueContext))
    );
    let transition = install_first(&mut director);
    assert_eq!(transition.health.state, DirectorHealthStateV1::Fresh);
    assert!(
        transition
            .scheduled
            .iter()
            .any(|intent| matches!(intent.kind, DirectorServiceKindV1::StepSubmissions))
    );
    assert!(
        transition
            .scheduled
            .iter()
            .any(|intent| matches!(intent.kind, DirectorServiceKindV1::AdvanceEpisodes))
    );
}

/// FG-BOARD-002 engagement_timeout_uses_durable_stale_snapshot_and_schedules_retry.
#[test]
fn engagement_timeout_uses_durable_stale_snapshot_and_schedules_retry() {
    let mut director = CompetitionDirectorStateV1::engage("campaign", 0).unwrap();
    install_first(&mut director);
    let prior_revision = director.revision;
    let prior = director.board.clone().unwrap();
    let mut stale = prior.clone();
    stale.freshness = BoardFreshnessV1::Stale {
        since_ms: 10,
        reason: "adapter Transport".into(),
    };
    let failure = BoardFetchFailureV1 {
        canonical: Some(stale),
        failure: AdapterFailureV1 {
            class: AdapterFailureClassV1::Transport,
            retry: RetryPolicyV1::AfterMs(50),
            detail_sha256: digest("timeout"),
            provenance_sha256: digest("adapter"),
        },
        health: DirectorHealthStateV1::Retrying,
        next: ScheduledActionV1 {
            action: "full_board_refresh".into(),
            next_attempt_at_ms: 60,
        },
    };
    let transition = director.apply_board_failure(&failure, 10).unwrap();
    let retained = director.board.as_ref().unwrap();
    assert_eq!(
        (retained.board_epoch, &retained.decision_sha256),
        (prior.board_epoch, &prior.decision_sha256)
    );
    assert!(matches!(retained.freshness, BoardFreshnessV1::Stale { .. }));
    assert_eq!(transition.health.last_good_revision, Some(prior_revision));
    assert_eq!(transition.health.next.next_attempt_at_ms, 60);
    assert!(
        transition
            .scheduled
            .iter()
            .any(|intent| matches!(intent.kind, DirectorServiceKindV1::ContinueContext))
    );
}

#[test]
fn board_failure_rejects_any_nonfreshness_last_good_mutation_atomically() {
    let mut director = CompetitionDirectorStateV1::engage("campaign", 0).unwrap();
    install_first(&mut director);
    let before = director.clone();
    let mut stale = director.board.clone().unwrap();
    stale.freshness = BoardFreshnessV1::Stale {
        since_ms: 10,
        reason: "adapter transport".into(),
    };
    let template = BoardFetchFailureV1 {
        canonical: Some(stale.clone()),
        failure: AdapterFailureV1 {
            class: AdapterFailureClassV1::Transport,
            retry: RetryPolicyV1::AfterMs(50),
            detail_sha256: digest("timeout-tamper"),
            provenance_sha256: digest("adapter-tamper"),
        },
        health: DirectorHealthStateV1::Retrying,
        next: ScheduledActionV1 {
            action: "full_board_refresh".into(),
            next_attempt_at_ms: 60,
        },
    };
    let mut variants = Vec::new();
    let mut entries = stale.clone();
    entries.entries[0].participant_id = "different-participant".into();
    variants.push(entries);
    let mut revision = stale.clone();
    revision.observation_revision += 1;
    variants.push(revision);
    let mut observation = stale.clone();
    observation.latest_observation_id = "different-observation".into();
    variants.push(observation);
    let mut provenance = stale.clone();
    provenance.latest_provenance.raw_sha256 = digest("different-raw");
    variants.push(provenance);
    let mut sequence = stale.clone();
    sequence.last_sequence = Some(99);
    variants.push(sequence);
    let mut predecessor = stale;
    predecessor.predecessor_epoch = Some(99);
    variants.push(predecessor);
    for canonical in variants {
        let mut failure = template.clone();
        failure.canonical = Some(canonical);
        assert!(director.apply_board_failure(&failure, 10).is_err());
        assert_eq!(director, before);
    }
}

#[test]
fn sequence_gap_keeps_last_good_unconfirmed_and_schedules_full_refresh() {
    let mut director = CompetitionDirectorStateV1::engage("campaign", 0).unwrap();
    install_first(&mut director);
    let mut unconfirmed = director.board.clone().unwrap();
    unconfirmed.observation_revision += 1;
    unconfirmed.latest_observation_id = "observation-gap".into();
    unconfirmed.freshness = BoardFreshnessV1::Unconfirmed {
        reason: "board sequence gap requires full refresh".into(),
        refresh_due_ms: 75,
    };
    let mut gap = outcome(unconfirmed, BoardReduceEffectV1::GapDetected);
    gap.refresh = Some(ScheduledActionV1 {
        action: "full_board_refresh".into(),
        next_attempt_at_ms: 75,
    });
    let transition = director.apply_board_outcome(&gap, 20).unwrap();
    assert_eq!(transition.health.state, DirectorHealthStateV1::Retrying);
    assert_eq!(transition.health.next.next_attempt_at_ms, 75);
    assert!(matches!(
        director.board.as_ref().unwrap().freshness,
        BoardFreshnessV1::Unconfirmed { .. }
    ));
    assert_eq!(director.candidates.as_ref().unwrap().current_board_epoch, 1);
    assert_eq!(
        director
            .board
            .as_ref()
            .unwrap()
            .frontier
            .source_accessible_base
            .as_ref()
            .unwrap()
            .selected_from_observation_id,
        "observation-1"
    );
}

#[test]
fn movement_atomically_stales_ancestor_until_replay_descendant_and_restarts() {
    let mut director = CompetitionDirectorStateV1::engage("campaign", 0).unwrap();
    install_first(&mut director);
    let first = director.board.clone().unwrap();
    let candidate = originate_candidate(
        &director.candidates.as_ref().unwrap().catalog,
        "candidate-parent",
        "hypothesis",
        "episode-parent",
        &first,
        artifact("parent"),
        evidence("parent"),
    )
    .unwrap();
    director
        .candidates
        .as_mut()
        .unwrap()
        .insert(candidate)
        .unwrap();
    let moved = board(2, "110", "source-b", "70");
    let transition = director
        .apply_board_outcome(
            &outcome(moved.clone(), BoardReduceEffectV1::DecisionAdvanced),
            20,
        )
        .unwrap();
    let candidates = director.candidates.as_mut().unwrap();
    assert_eq!(
        candidates.eligibility["candidate-parent"].status,
        CandidateEligibilityStatusV1::StaleBoard
    );
    assert!(transition.scheduled.iter().any(|intent| matches!(
        intent.kind,
        DirectorServiceKindV1::ReplayCandidates {
            from_epoch: 1,
            to_epoch: 2
        }
    )));
    let child = port_replay_candidate(
        &candidates.catalog,
        "candidate-child",
        "candidate-parent",
        "episode-child",
        &moved,
        artifact("child"),
        evidence("child"),
    )
    .unwrap();
    candidates.insert(child).unwrap();
    assert!(candidates.require_eligible("candidate-parent").is_err());
    assert!(candidates.require_eligible("candidate-child").is_ok());

    let encoded = serde_json::to_vec(&director).unwrap();
    let restored: CompetitionDirectorStateV1 = serde_json::from_slice(&encoded).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored, director);
}

#[test]
fn rejected_transition_is_atomic_and_health_has_no_global_terminal_variant() {
    let mut director = CompetitionDirectorStateV1::engage("campaign", 0).unwrap();
    install_first(&mut director);
    let before = director.clone();
    let conflicting = board(1, "999", "source-z", "90");
    assert!(
        director
            .apply_board_outcome(
                &outcome(conflicting, BoardReduceEffectV1::DecisionAdvanced),
                30,
            )
            .is_err()
    );
    assert_eq!(director, before);
    let json = serde_json::to_string(&director).unwrap();
    for forbidden in ["blocked", "stopped", "frozen"] {
        assert!(!json.contains(forbidden));
    }
}
