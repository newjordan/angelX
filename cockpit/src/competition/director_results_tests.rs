use super::candidate::originate_candidate;
use super::candidate_tests::{artifact, board, digest, evidence};
use super::director::CompetitionDirectorStateV1;
use super::director_services::DirectorEpisodeRefV1;
use super::journal::ActionJournalStateV1;
use super::profile::DeepCutProfileV1;
use super::rewards::{
    OfficialProvenanceV1, OfficialResultSourceV1, OfficialResultV1, RewardContextV1,
};
use super::schema::{AdapterCapabilityV1, AdapterIdentityV1, ScoreV1};
use super::submission::{SubmissionDispositionV1, SubmissionSpoolV1};
use super::submission_reconcile::disposition_origin;
use super::submission_results::OfficialResultV1 as SubmissionOfficialResultV1;
use super::submission_tests::{apply, first_submit};
use std::collections::BTreeSet;

pub(super) fn ready_director() -> (
    CompetitionDirectorStateV1,
    RewardContextV1,
    OfficialResultV1,
) {
    let mut director = CompetitionDirectorStateV1::engage("campaign", 0).unwrap();
    let board = board(1, "100", "source-a", "60");
    director
        .apply_board_outcome(
            &super::director_tests::outcome(
                board.clone(),
                super::board::BoardReduceEffectV1::Initialized,
            ),
            1,
        )
        .unwrap();
    let candidate = originate_candidate(
        &director.candidates.as_ref().unwrap().catalog,
        "candidate-result",
        "hypothesis-result",
        "episode-result",
        &board,
        artifact("result"),
        evidence("result"),
    )
    .unwrap();
    director
        .candidates
        .as_mut()
        .unwrap()
        .insert(candidate)
        .unwrap();
    let repository = director.candidates.as_ref().unwrap();
    director
        .submissions
        .enqueue(repository, "campaign", "candidate-result")
        .unwrap();
    acknowledge(&mut director.submissions, repository);
    let item = director.submissions.items.get(&0).unwrap().clone();
    let result_receipt = digest("director-official-result");
    let platform_result = SubmissionOfficialResultV1::new(
        &item,
        "result-director",
        1,
        ScoreV1::new("105").unwrap(),
        result_receipt.clone(),
        None,
    )
    .unwrap();
    director.submissions.bind_official(platform_result).unwrap();
    director
        .register_episode(
            DirectorEpisodeRefV1 {
                episode_id: "episode-result".into(),
                candidate_id: "candidate-result".into(),
                submission_id: item.submission_id.clone(),
                board_epoch: 1,
                journal_head_sha256: digest("episode-head"),
                terminal: true,
                latest_official_result_id: None,
                latest_reward_binding_sha256: None,
            },
            2,
        )
        .unwrap();
    let context = RewardContextV1 {
        episode_id: "episode-result".into(),
        candidate_id: "candidate-result".into(),
        submission_id: item.submission_id,
        comparable_base_id: "source-a".into(),
        competition: board.competition.clone(),
        objective: board.comparator.clone(),
        profile: DeepCutProfileV1::embedded().identity().unwrap(),
    };
    let result = OfficialResultV1 {
        result_id: "result-director".into(),
        result_revision: 1,
        episode_id: context.episode_id.clone(),
        candidate_id: context.candidate_id.clone(),
        submission_id: context.submission_id.clone(),
        comparable_base_id: context.comparable_base_id.clone(),
        competition: context.competition.clone(),
        objective: context.objective.clone(),
        profile: context.profile.clone(),
        candidate_score: ScoreV1::new("105").unwrap(),
        base_score: ScoreV1::new("60").unwrap(),
        provenance: OfficialProvenanceV1 {
            source: OfficialResultSourceV1::OfficialSubmissionResult,
            adapter: AdapterIdentityV1 {
                adapter_id: "result-adapter".into(),
                adapter_version: "1".into(),
                runtime_sha256: digest("result-adapter"),
                capabilities: BTreeSet::from([AdapterCapabilityV1::Results]),
            },
            receipt_sha256: result_receipt,
        },
        corrects_binding_id: None,
    };
    (director, context, result)
}

pub(super) fn acknowledge(
    spool: &mut SubmissionSpoolV1,
    repository: &super::candidate_store::CandidateRepositoryV1,
) {
    let mut journal = ActionJournalStateV1::default();
    let (planned, started, _, _) = first_submit(spool, repository, &journal);
    let origin = disposition_origin(&started).unwrap();
    apply(&mut journal, planned);
    apply(&mut journal, started);
    spool
        .record_disposition(
            &journal,
            0,
            origin,
            SubmissionDispositionV1::Acknowledged {
                platform_submission_id: "platform-director".into(),
                receipt_sha256: digest("submission-ack"),
            },
        )
        .unwrap();
}

#[test]
fn official_result_is_atomically_attributed_to_episode_candidate_and_submission() {
    let (mut director, context, result) = ready_director();
    director
        .bind_result(
            &context,
            result.clone(),
            "memory-bandwidth".into(),
            "vectorized-load".into(),
            None,
            0,
            10,
            3,
        )
        .unwrap();
    assert_eq!(director.rewards.bindings().len(), 1);
    assert_eq!(director.patterns.evidence_records().len(), 1);
    let episode = &director.episodes["episode-result"];
    assert_eq!(
        episode.latest_official_result_id.as_deref(),
        Some("result-director")
    );
    assert!(episode.latest_reward_binding_sha256.is_some());
    director.validate().unwrap();
    let before_duplicate = director.clone();
    director
        .bind_result(
            &context,
            result,
            "memory-bandwidth".into(),
            "vectorized-load".into(),
            None,
            0,
            10,
            4,
        )
        .unwrap();
    assert_eq!(director, before_duplicate);
}

#[test]
fn foreign_result_attribution_is_rejected_without_partial_reward_or_pattern() {
    let (mut director, context, mut result) = ready_director();
    let before = director.clone();
    result.submission_id = "foreign-submission".into();
    assert!(
        director
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
            .is_err()
    );
    assert_eq!(director, before);
}

#[test]
fn result_revision_and_source_base_must_exactly_match_durable_evidence() {
    let (mut director, context, result) = ready_director();
    let before = director.clone();

    let mut wrong_revision = result.clone();
    wrong_revision.result_revision = 99;
    assert!(
        director
            .bind_result(
                &context,
                wrong_revision,
                "memory-bandwidth".into(),
                "vectorized-load".into(),
                None,
                0,
                10,
                3,
            )
            .is_err()
    );
    assert_eq!(director, before);

    let mut wrong_base = result.clone();
    wrong_base.base_score = ScoreV1::new("59").unwrap();
    assert!(
        director
            .bind_result(
                &context,
                wrong_base,
                "memory-bandwidth".into(),
                "vectorized-load".into(),
                None,
                0,
                10,
                3,
            )
            .is_err()
    );
    assert_eq!(director, before);

    let mut foreign_context = context;
    foreign_context.comparable_base_id = "global".into();
    assert!(
        director
            .bind_result(
                &foreign_context,
                result,
                "memory-bandwidth".into(),
                "vectorized-load".into(),
                None,
                0,
                10,
                3,
            )
            .is_err()
    );
    assert_eq!(director, before);
}
