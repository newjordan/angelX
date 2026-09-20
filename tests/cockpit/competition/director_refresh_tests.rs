use super::board::{BoardFreshnessV1, BoardReduceEffectV1, CanonicalBoardV1};
use super::candidate::originate_candidate;
use super::candidate_store::CandidateEligibilityStatusV1;
use super::candidate_tests::{artifact, board, digest, evidence};
use super::director::CompetitionDirectorStateV1;
use super::schema::ScoreV1;

fn with_candidate() -> CompetitionDirectorStateV1 {
    let mut director = CompetitionDirectorStateV1::engage("campaign", 0).unwrap();
    let board = board(1, "100", "source-a", "60");
    director
        .apply_board_outcome(
            &super::director_tests::outcome(board.clone(), BoardReduceEffectV1::Initialized),
            1,
        )
        .unwrap();
    let candidate = originate_candidate(
        &director.candidates.as_ref().unwrap().catalog,
        "candidate-refresh",
        "hypothesis-refresh",
        "episode-refresh",
        &board,
        artifact("refresh"),
        evidence("refresh"),
    )
    .unwrap();
    director
        .candidates
        .as_mut()
        .unwrap()
        .insert(candidate)
        .unwrap();
    director
}

fn refreshed(mut board: CanonicalBoardV1) -> CanonicalBoardV1 {
    board.observation_revision += 1;
    board.latest_observation_id = "observation-refresh".into();
    board.latest_provenance.observed_at_ms += 1;
    board.latest_provenance.raw_sha256 = digest("raw-refresh");
    board
        .frontier
        .source_accessible_base
        .as_mut()
        .unwrap()
        .selected_from_observation_id = board.latest_observation_id.clone();
    board.freshness = BoardFreshnessV1::Fresh;
    board.decision_sha256 = board.frontier.decision_sha256(&board.comparator).unwrap();
    board
}

#[test]
fn unchanged_same_epoch_refresh_preserves_candidate_history_and_eligibility() {
    let mut director = with_candidate();
    let historical = director.candidates.as_ref().unwrap().catalog["candidate-refresh"]
        .board
        .source_accessible_base
        .selected_from_observation_id
        .clone();
    let refresh = refreshed(director.board.clone().unwrap());
    director
        .apply_board_outcome(
            &super::director_tests::outcome(refresh, BoardReduceEffectV1::DecisionRefreshed),
            2,
        )
        .unwrap();
    director.validate().unwrap();
    let repository = director.candidates.as_ref().unwrap();
    assert_eq!(
        repository.eligibility["candidate-refresh"].status,
        CandidateEligibilityStatusV1::Eligible
    );
    assert_eq!(
        repository.catalog["candidate-refresh"]
            .board
            .source_accessible_base
            .selected_from_observation_id,
        historical
    );
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
        "observation-refresh"
    );
}

#[test]
fn same_epoch_source_artifact_or_score_change_cannot_retain_eligibility() {
    let director = with_candidate();
    let mut source_changed = refreshed(director.board.clone().unwrap());
    let claim = &mut source_changed
        .frontier
        .source_accessible_base
        .as_mut()
        .unwrap()
        .source;
    claim.commit_oid = digest("changed-source-commit");
    let source_entry = source_changed
        .entries
        .iter_mut()
        .find(|entry| entry.entry_id == "source-a")
        .unwrap();
    source_entry.source = Some(claim.clone());
    source_changed.decision_sha256 = source_changed
        .frontier
        .decision_sha256(&source_changed.comparator)
        .unwrap();

    let mut score_changed = refreshed(director.board.clone().unwrap());
    let score = ScoreV1::new("61").unwrap();
    score_changed
        .frontier
        .source_accessible_base
        .as_mut()
        .unwrap()
        .score = score.clone();
    score_changed
        .entries
        .iter_mut()
        .find(|entry| entry.entry_id == "source-a")
        .unwrap()
        .score = score;
    score_changed.decision_sha256 = score_changed
        .frontier
        .decision_sha256(&score_changed.comparator)
        .unwrap();

    for board in [source_changed, score_changed] {
        let mut attempt = director.clone();
        let before = attempt.clone();
        assert!(
            attempt
                .apply_board_outcome(
                    &super::director_tests::outcome(board, BoardReduceEffectV1::DecisionRefreshed,),
                    2,
                )
                .is_err()
        );
        assert_eq!(attempt, before);
    }
}
