use super::board::BoardFreshnessV1;
use super::candidate_store::CandidateEligibilityStatusV1;
use super::director::CompetitionDirectorStateV1;
use super::frontier::SourceAccessibleBaseV1;
use super::patterns::PatternEvidenceClassV1;
use super::rewards::OfficialRewardBindingV1;

pub(super) fn validate_board_and_candidates(
    state: &CompetitionDirectorStateV1,
) -> Result<(), String> {
    match (&state.board, &state.candidates) {
        (None, None) => Ok(()),
        (Some(board), Some(candidates)) => {
            super::director_board_validation::validate_board(board, &state.campaign_id)?;
            candidates.validate()?;
            if candidates.current_board_epoch != board.board_epoch
                || candidates.current_board_decision_sha256 != board.decision_sha256
            {
                return Err("director candidate eligibility is not on current board".into());
            }
            for (id, eligibility) in &candidates.eligibility {
                if eligibility.status != CandidateEligibilityStatusV1::Eligible {
                    continue;
                }
                let candidate = &candidates.catalog[id];
                if candidate.competition != board.competition
                    || candidate.objective != board.comparator
                    || candidate.board.global_target != board.frontier.global_target
                    || candidate.board.personal_best != board.frontier.personal_best
                    || !same_source_decision(
                        &candidate.board.source_accessible_base,
                        board
                            .frontier
                            .source_accessible_base
                            .as_ref()
                            .ok_or_else(|| {
                                "current board lacks candidate source base".to_string()
                            })?,
                    )
                {
                    return Err(
                        "eligible candidate is not bound to canonical board evidence".into(),
                    );
                }
            }
            Ok(())
        }
        _ => Err("director board and candidate repository must coexist".into()),
    }
}

fn same_source_decision(
    historical: &SourceAccessibleBaseV1,
    current: &SourceAccessibleBaseV1,
) -> bool {
    historical.entry_id == current.entry_id
        && historical.participant_id == current.participant_id
        && historical.submission_id == current.submission_id
        && historical.score == current.score
        && historical.source == current.source
}

pub(super) fn validate_cross_composition(state: &CompetitionDirectorStateV1) -> Result<(), String> {
    for item in state.submissions.items.values() {
        let candidate = state
            .candidates
            .as_ref()
            .and_then(|repository| repository.catalog.get(&item.candidate_id))
            .ok_or_else(|| "submission candidate is absent from director repository".to_string())?;
        if item.campaign_id != state.campaign_id
            || item.candidate_record_sha256 != candidate.record_sha256
            || item.competition != candidate.competition
            || item.objective != candidate.objective
            || item.board_epoch != candidate.board.board_epoch
            || item.board_decision_sha256 != candidate.board.board_decision_sha256
        {
            return Err("submission is not bound to its director candidate".into());
        }
    }
    for binding in state.rewards.bindings() {
        validate_reward_join(state, binding)?;
    }
    for shaping in state.rewards.shaping() {
        if !state.episodes.contains_key(&shaping.episode_id) {
            return Err("provisional shaping episode is absent from director".into());
        }
    }
    for episode in state.episodes.values() {
        match (
            &episode.latest_official_result_id,
            &episode.latest_reward_binding_sha256,
        ) {
            (None, None) => {}
            (Some(result_id), Some(binding_id)) => {
                let binding = state
                    .rewards
                    .canonical_binding(&episode.episode_id)
                    .ok_or_else(|| "episode latest reward is absent".to_string())?;
                if &binding.official_result_id != result_id || &binding.binding_id != binding_id {
                    return Err("episode latest reward pointer is not canonical".into());
                }
            }
            _ => return Err("episode official result and reward pointers are incomplete".into()),
        }
    }
    for evidence in state.patterns.evidence_records() {
        if !state.episodes.contains_key(&evidence.episode_id) {
            return Err("pattern evidence episode is absent from director".into());
        }
        if let PatternEvidenceClassV1::ConfirmedOfficial {
            reward_binding_id,
            corrects_reward_binding_id,
        } = &evidence.class
        {
            let binding = state
                .rewards
                .bindings()
                .iter()
                .find(|binding| &binding.binding_id == reward_binding_id)
                .ok_or_else(|| "confirmed pattern reward is absent".to_string())?;
            if evidence.episode_id != binding.episode_id
                || corrects_reward_binding_id != &binding.corrects_binding_id
            {
                return Err("confirmed pattern is not bound to its reward".into());
            }
        }
    }
    validate_board_service(state)
}

fn validate_reward_join(
    state: &CompetitionDirectorStateV1,
    binding: &OfficialRewardBindingV1,
) -> Result<(), String> {
    let episode = state
        .episodes
        .get(&binding.episode_id)
        .ok_or_else(|| "reward episode is absent from director".to_string())?;
    let candidate = state
        .candidates
        .as_ref()
        .and_then(|repository| repository.catalog.get(&binding.candidate_id))
        .ok_or_else(|| "reward candidate is absent from director".to_string())?;
    let official = state
        .submissions
        .official
        .results
        .iter()
        .find(|result| result.result_id == binding.official_result_id)
        .ok_or_else(|| "reward official result is absent from submission spool".to_string())?;
    let base = &candidate.board.source_accessible_base;
    if episode.candidate_id != binding.candidate_id
        || episode.submission_id != binding.submission_id
        || official.submission_id != binding.submission_id
        || official.candidate_id != binding.candidate_id
        || official.result_revision != binding.official_result_revision
        || official.competition != binding.competition
        || official.objective != binding.objective
        || official.candidate_score != binding.candidate_score
        || official.receipt_sha256 != binding.official_provenance.receipt_sha256
        || binding.comparable_base_id != base.entry_id
        || binding.base_score != base.score
    {
        return Err("reward is not exactly joined to official candidate/base evidence".into());
    }
    match &binding.corrects_binding_id {
        None if official.corrects_result_id.is_none() && binding.official_result_revision == 1 => {}
        Some(parent_id) => {
            let parent = state
                .rewards
                .bindings()
                .iter()
                .find(|candidate| &candidate.binding_id == parent_id)
                .ok_or_else(|| "reward correction parent is absent".to_string())?;
            if official.corrects_result_id.as_deref() != Some(&parent.official_result_id)
                || parent.official_result_revision.checked_add(1)
                    != Some(binding.official_result_revision)
            {
                return Err("reward correction is not joined to official revision chain".into());
            }
        }
        _ => return Err("initial reward does not match initial official result".into()),
    }
    Ok(())
}

fn validate_board_service(state: &CompetitionDirectorStateV1) -> Result<(), String> {
    let needs_board = state
        .board
        .as_ref()
        .is_none_or(|board| !matches!(board.freshness, BoardFreshnessV1::Fresh));
    if !needs_board {
        return Ok(());
    }
    let service = state
        .services
        .iter()
        .find(|service| service.class() == "board")
        .ok_or_else(|| "nonfresh director lacks board service".to_string())?;
    let expected = match service.kind {
        super::director_services::DirectorServiceKindV1::RefreshBoard { full: true } => {
            "full_board_refresh"
        }
        super::director_services::DirectorServiceKindV1::RemediateBoardAdapter => {
            "remediate_board_adapter"
        }
        _ => return Err("nonfresh director has invalid board service".into()),
    };
    if state.health.next.action != expected
        || state.health.next.next_attempt_at_ms != service.due_at_ms
    {
        return Err("director health next action disagrees with board service".into());
    }
    Ok(())
}
