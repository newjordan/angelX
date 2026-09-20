use super::board::{
    BoardFetchFailureV1, BoardFreshnessV1, BoardReduceEffectV1, BoardReduceOutcomeV1,
    CanonicalBoardV1,
};
use super::candidate_store::CandidateRepositoryV1;
use super::director::CompetitionDirectorStateV1;
use super::director_services::{
    DirectorServiceIntentV1, DirectorServiceKindV1, DirectorTransitionV1,
};
use super::schema::{DirectorHealthStateV1, RetryPolicyV1};
use super::schema_validation::validate_sha256;

impl CompetitionDirectorStateV1 {
    pub(crate) fn apply_board_outcome(
        &mut self,
        outcome: &BoardReduceOutcomeV1,
        now_ms: u64,
    ) -> Result<DirectorTransitionV1, String> {
        let board = outcome
            .canonical
            .as_ref()
            .ok_or_else(|| "board outcome lacks canonical state".to_string())?;
        let mut next = self.clone();
        let moved = install_board(&mut next, board)?;
        schedule_independent_services(&mut next, now_ms)?;
        if moved {
            let from_epoch = self.board.as_ref().map_or(0, |prior| prior.board_epoch);
            if from_epoch > 0 {
                next.upsert_service(DirectorServiceIntentV1::new(
                    &self.campaign_id,
                    DirectorServiceKindV1::ReplayCandidates {
                        from_epoch,
                        to_epoch: board.board_epoch,
                    },
                    now_ms,
                )?);
            }
        }
        let (health, reason, action, due) = match &board.freshness {
            BoardFreshnessV1::Fresh => {
                next.remove_service("board");
                (
                    DirectorHealthStateV1::Fresh,
                    None,
                    "director_service_scan",
                    now_ms,
                )
            }
            BoardFreshnessV1::Stale { reason, .. } => {
                schedule_refresh(&mut next, now_ms)?;
                (
                    DirectorHealthStateV1::Degraded,
                    Some(reason.as_str()),
                    "full_board_refresh",
                    now_ms,
                )
            }
            BoardFreshnessV1::Unconfirmed {
                reason,
                refresh_due_ms,
            } => {
                schedule_refresh(&mut next, *refresh_due_ms)?;
                (
                    DirectorHealthStateV1::Retrying,
                    Some(reason.as_str()),
                    "full_board_refresh",
                    *refresh_due_ms,
                )
            }
        };
        if matches!(outcome.effect, BoardReduceEffectV1::GapDetected) && outcome.refresh.is_none() {
            return Err("board gap lacks refresh scheduling".into());
        }
        self.commit(next, health, reason, action, due)
    }

    pub(crate) fn apply_board_failure(
        &mut self,
        failure: &BoardFetchFailureV1,
        now_ms: u64,
    ) -> Result<DirectorTransitionV1, String> {
        failure.next.validate().map_err(str::to_string)?;
        validate_sha256(&failure.failure.detail_sha256).map_err(str::to_string)?;
        validate_sha256(&failure.failure.provenance_sha256).map_err(str::to_string)?;
        let requires_attention = failure.failure.retry == RetryPolicyV1::Never
            || matches!(
                failure.failure.class,
                super::adapters::AdapterFailureClassV1::Unsupported
                    | super::adapters::AdapterFailureClassV1::Invariant
            );
        if (failure.health == DirectorHealthStateV1::NeedsAttention) != requires_attention
            || (!requires_attention && failure.health != DirectorHealthStateV1::Retrying)
        {
            return Err("board failure health disagrees with retry disposition".into());
        }
        if failure.health == DirectorHealthStateV1::Fresh
            || failure
                .canonical
                .as_ref()
                .is_some_and(|board| matches!(board.freshness, BoardFreshnessV1::Fresh))
        {
            return Err("board failure cannot claim fresh state".into());
        }
        validate_failure_snapshot(self.board.as_ref(), failure.canonical.as_ref())?;
        let mut next = self.clone();
        next.board = failure.canonical.clone();
        let (kind, expected_action) = if failure.health == DirectorHealthStateV1::NeedsAttention {
            (
                DirectorServiceKindV1::RemediateBoardAdapter,
                "remediate_board_adapter",
            )
        } else {
            (
                DirectorServiceKindV1::RefreshBoard { full: true },
                "full_board_refresh",
            )
        };
        if failure.next.action != expected_action {
            return Err("board failure action disagrees with recovery class".into());
        }
        next.upsert_service(DirectorServiceIntentV1::new(
            &self.campaign_id,
            kind,
            failure.next.next_attempt_at_ms,
        )?);
        next.upsert_service(DirectorServiceIntentV1::new(
            &self.campaign_id,
            DirectorServiceKindV1::ContinueContext,
            now_ms,
        )?);
        self.commit(
            next,
            failure.health,
            Some("board observation failed; durable last-good remains usable"),
            &failure.next.action,
            failure.next.next_attempt_at_ms,
        )
    }
}

fn install_board(
    state: &mut CompetitionDirectorStateV1,
    board: &CanonicalBoardV1,
) -> Result<bool, String> {
    if board.campaign_id != state.campaign_id {
        return Err("board campaign differs from director".into());
    }
    let moved = match state.board.as_ref() {
        None => {
            state.candidates = Some(CandidateRepositoryV1::new(
                board.board_epoch,
                board.decision_sha256.clone(),
            )?);
            true
        }
        Some(prior) if board.board_epoch < prior.board_epoch => {
            return Err("director board epoch moved backward".into());
        }
        Some(prior) if board.board_epoch == prior.board_epoch => {
            if board.decision_sha256 != prior.decision_sha256
                || board.observation_revision < prior.observation_revision
            {
                return Err("same board epoch conflicts or rewinds observation".into());
            }
            false
        }
        Some(prior) => {
            if prior.board_epoch.checked_add(1) != Some(board.board_epoch)
                || board.predecessor_epoch != Some(prior.board_epoch)
            {
                return Err("director board movement is not a single linked epoch".into());
            }
            state
                .candidates
                .as_mut()
                .ok_or_else(|| "director candidate repository is absent".to_string())?
                .set_current_board(board.board_epoch, board.decision_sha256.clone())?;
            true
        }
    };
    state.board = Some(board.clone());
    Ok(moved)
}

fn schedule_independent_services(
    state: &mut CompetitionDirectorStateV1,
    now_ms: u64,
) -> Result<(), String> {
    for kind in [
        DirectorServiceKindV1::ContinueContext,
        DirectorServiceKindV1::StepSubmissions,
        DirectorServiceKindV1::AdvanceEpisodes,
        DirectorServiceKindV1::BindOfficialRewards,
        DirectorServiceKindV1::InspectWorkers,
    ] {
        state.upsert_service(DirectorServiceIntentV1::new(
            &state.campaign_id,
            kind,
            now_ms,
        )?);
    }
    Ok(())
}

fn schedule_refresh(state: &mut CompetitionDirectorStateV1, at_ms: u64) -> Result<(), String> {
    state.upsert_service(DirectorServiceIntentV1::new(
        &state.campaign_id,
        DirectorServiceKindV1::RefreshBoard { full: true },
        at_ms,
    )?);
    Ok(())
}

fn validate_failure_snapshot(
    current: Option<&CanonicalBoardV1>,
    incoming: Option<&CanonicalBoardV1>,
) -> Result<(), String> {
    match (current, incoming) {
        (None, None) => Ok(()),
        (Some(current), Some(incoming)) => {
            let mut normalized = incoming.clone();
            normalized.freshness = current.freshness.clone();
            if &normalized == current {
                Ok(())
            } else {
                Err("board failure mutated durable last-good evidence".into())
            }
        }
        _ => Err("board failure does not preserve durable last-good board".into()),
    }
}
