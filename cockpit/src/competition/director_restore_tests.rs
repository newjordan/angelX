use super::adapters::{AdapterFailureClassV1, AdapterFailureV1};
use super::board::{BoardFetchFailureV1, BoardFreshnessV1};
use super::candidate::originate_candidate;
use super::candidate_tests::{artifact, board, digest, evidence};
use super::director::CompetitionDirectorStateV1;
use super::director_results_tests::{acknowledge, ready_director};
use super::director_services::DirectorWorkerRefV1;
use super::frontier::GlobalTargetV1;
use super::patterns::{PatternKeyV1, PatternOutcomeV1};
use super::rewards::{ProvisionalShapingV1, RewardLedgerV1};
use super::schema::{DirectorHealthStateV1, LaneIdV1, RetryPolicyV1, ScheduledActionV1};
use super::submission::SubmissionSpoolV1;
use super::submission_results::OfficialResultV1 as SubmissionOfficialResultV1;

fn installed() -> CompetitionDirectorStateV1 {
    let mut director = CompetitionDirectorStateV1::engage("campaign", 0).unwrap();
    director
        .apply_board_outcome(
            &super::director_tests::outcome(
                board(1, "100", "source-a", "60"),
                super::board::BoardReduceEffectV1::Initialized,
            ),
            1,
        )
        .unwrap();
    director
}

fn make_stale(director: &CompetitionDirectorStateV1, due: u64) -> BoardFetchFailureV1 {
    let mut stale = director.board.clone().unwrap();
    stale.freshness = BoardFreshnessV1::Stale {
        since_ms: 10,
        reason: "adapter transport".into(),
    };
    BoardFetchFailureV1 {
        canonical: Some(stale),
        failure: AdapterFailureV1 {
            class: AdapterFailureClassV1::Transport,
            retry: RetryPolicyV1::AfterMs(due),
            detail_sha256: digest("failure-detail"),
            provenance_sha256: digest("failure-provenance"),
        },
        health: DirectorHealthStateV1::Retrying,
        next: ScheduledActionV1 {
            action: "full_board_refresh".into(),
            next_attempt_at_ms: due,
        },
    }
}

fn pattern_key(mechanism: &str) -> PatternKeyV1 {
    PatternKeyV1 {
        field_id: "kernels".into(),
        benchmark_id: "gemm".into(),
        profile_id: "official".into(),
        hardware_id: "gpu".into(),
        objective_id: "throughput".into(),
        comparator_version: "1".into(),
        bottleneck_class: "memory".into(),
        mechanism: mechanism.into(),
    }
}
#[test]
fn canonical_board_restore_rejects_resealed_semantic_tampering() {
    let state = installed();

    let mut duplicate = state.clone();
    let entry = duplicate.board.as_ref().unwrap().entries[0].clone();
    duplicate.board.as_mut().unwrap().entries.push(entry);
    assert!(duplicate.validate().is_err());

    let mut ancestry = state.clone();
    ancestry.board.as_mut().unwrap().predecessor_epoch = Some(9);
    assert!(ancestry.validate().is_err());

    let mut provenance = state.clone();
    provenance
        .board
        .as_mut()
        .unwrap()
        .latest_provenance
        .adapter
        .capabilities
        .clear();
    assert!(provenance.validate().is_err());

    let mut observation_swap = state.clone();
    let board = observation_swap.board.as_mut().unwrap();
    board
        .frontier
        .source_accessible_base
        .as_mut()
        .unwrap()
        .selected_from_observation_id = "other-observation".into();
    board.decision_sha256 = board.frontier.decision_sha256(&board.comparator).unwrap();
    observation_swap
        .candidates
        .as_mut()
        .unwrap()
        .current_board_decision_sha256 = board.decision_sha256.clone();
    assert!(observation_swap.validate().is_err());

    let mut swapped = state;
    let board = swapped.board.as_mut().unwrap();
    let source = board
        .entries
        .iter()
        .find(|entry| entry.entry_id == "source-a")
        .unwrap();
    board.frontier.global_target = Some(GlobalTargetV1 {
        entry_id: source.entry_id.clone(),
        participant_id: source.participant_id.clone(),
        submission_id: source.submission_id.clone(),
        rank: source.rank,
        score: source.score.clone(),
    });
    board.decision_sha256 = board.frontier.decision_sha256(&board.comparator).unwrap();
    swapped
        .candidates
        .as_mut()
        .unwrap()
        .current_board_decision_sha256 = board.decision_sha256.clone();
    assert!(swapped.candidates.as_ref().unwrap().validate().is_ok());
    assert!(swapped.validate().is_err());
}
#[test]
fn all_nonofficial_evidence_requires_a_director_episode() {
    let mut shaping = installed();
    shaping
        .rewards
        .record_shaping(ProvisionalShapingV1 {
            shaping_id: "orphan-shaping".into(),
            episode_id: "orphan-episode".into(),
            value_millis: 1,
            provenance_sha256: digest("orphan-shaping"),
        })
        .unwrap();
    assert!(shaping.rewards.validate().is_ok());
    assert!(shaping.validate().is_err());
    let mut transfer = installed();
    transfer
        .patterns
        .record_transfer(
            "orphan-transfer".into(),
            "orphan-episode".into(),
            pattern_key("source"),
            pattern_key("target"),
            PatternOutcomeV1::Improvement,
            1,
            1,
        )
        .unwrap();
    assert!(transfer.patterns.validate().is_ok());
    assert!(transfer.validate().is_err());
    let mut operational = installed();
    operational
        .patterns
        .record_operational(
            "orphan-operational".into(),
            "orphan-episode".into(),
            pattern_key("failure"),
            PatternOutcomeV1::Failure,
            1,
            1,
        )
        .unwrap();
    assert!(operational.patterns.validate().is_ok());
    assert!(operational.validate().is_err());
}

#[test]
fn whole_snapshot_rejects_valid_component_removal_and_swap() {
    let (mut complete, context, result) = ready_director();
    complete
        .bind_result(
            &context,
            result,
            "memory-bandwidth".into(),
            "vectorized-load".into(),
            None,
            0,
            10,
            3,
        )
        .unwrap();

    let mut missing_official = complete.clone();
    missing_official.submissions.official.results.clear();
    missing_official
        .submissions
        .official
        .latest_by_submission
        .clear();
    assert!(missing_official.submissions.validate().is_ok());
    assert!(missing_official.validate().is_err());

    let mut missing_reward = complete.clone();
    missing_reward.rewards = RewardLedgerV1::default();
    assert!(missing_reward.patterns.validate().is_ok());
    assert!(missing_reward.validate().is_err());

    let mut missing_episode = complete.clone();
    missing_episode.episodes.clear();
    assert!(missing_episode.validate().is_err());

    let mut missing_candidate = complete.clone();
    let repository = missing_candidate.candidates.as_mut().unwrap();
    repository.catalog.remove("candidate-result");
    repository.eligibility.remove("candidate-result");
    assert!(repository.validate().is_ok());
    assert!(missing_candidate.validate().is_err());

    let board = complete.board.clone().unwrap();
    let second = originate_candidate(
        &complete.candidates.as_ref().unwrap().catalog,
        "candidate-other",
        "hypothesis-other",
        "episode-other",
        &board,
        artifact("other"),
        evidence("other"),
    )
    .unwrap();
    complete
        .candidates
        .as_mut()
        .unwrap()
        .insert(second)
        .unwrap();
    let repository = complete.candidates.as_ref().unwrap();
    let mut other_spool = SubmissionSpoolV1::new();
    other_spool
        .enqueue(repository, "campaign", "candidate-other")
        .unwrap();
    acknowledge(&mut other_spool, repository);
    let item = other_spool.items[&0].clone();
    other_spool
        .bind_official(
            SubmissionOfficialResultV1::new(
                &item,
                "result-other",
                1,
                super::schema::ScoreV1::new("61").unwrap(),
                digest("result-other"),
                None,
            )
            .unwrap(),
        )
        .unwrap();
    assert!(other_spool.validate().is_ok());
    let mut swapped = complete;
    swapped.submissions = other_spool;
    assert!(swapped.validate().is_err());
}

#[test]
fn nonfresh_retry_is_stable_and_exact_registration_replay_is_noop() {
    let (mut director, context, result) = ready_director();
    director
        .apply_board_failure(&make_stale(&director, 60), 10)
        .unwrap();
    let episode = director.episodes["episode-result"].clone();
    let before_episode_replay = director.clone();
    let transition = director.register_episode(episode, 100).unwrap();
    assert_eq!(director, before_episode_replay);
    assert_eq!(transition.health, before_episode_replay.health);

    let worker = DirectorWorkerRefV1 {
        work_item_id: "work-one".into(),
        lane: LaneIdV1::DeepCut,
        lease_id: "lease-one".into(),
        fencing_generation: 1,
        checkpoint_revision: 0,
    };
    let preserved_health = director.health.clone();
    director.register_worker(worker.clone(), 101).unwrap();
    assert_eq!(director.health, preserved_health);
    let before_worker_replay = director.clone();
    director.register_worker(worker, 102).unwrap();
    assert_eq!(director, before_worker_replay);

    let before_result_health = director.health.clone();
    director
        .bind_result(
            &context,
            result,
            "memory-bandwidth".into(),
            "vectorized-load".into(),
            None,
            0,
            10,
            103,
        )
        .unwrap();
    assert_eq!(director.health, before_result_health);
    assert_eq!(director.health.next.next_attempt_at_ms, 60);
    director.validate().unwrap();
}
