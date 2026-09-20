use super::adapters::{AdapterFailureClassV1, AdapterFailureV1, CompetitionAdapterV1};
use super::board::*;
use super::frontier::reduce_frontier;
use super::schema::{DirectorHealthStateV1, ObjectiveComparatorV1, RetryPolicyV1};
use std::collections::BTreeSet;

#[derive(Default)]
pub(crate) struct BoardReducerV1 {
    current: Option<CanonicalBoardV1>,
    applied_observations: BTreeSet<String>,
    pending_gap_observations: BTreeSet<String>,
    gap_refresh_due_ms: Option<u64>,
}

impl BoardReducerV1 {
    pub(crate) fn current(&self) -> Option<&CanonicalBoardV1> {
        self.current.as_ref()
    }

    pub(crate) fn restore_from_persisted<A: CompetitionAdapterV1 + ?Sized>(
        &mut self,
        campaign_id: &str,
        comparator: ObjectiveComparatorV1,
        adapter: &A,
        observations: &[BoardObservationV1],
    ) -> Result<Vec<BoardReduceOutcomeV1>, BoardReduceErrorV1> {
        let mut restored = Self::default();
        let outcomes = observations
            .iter()
            .enumerate()
            .map(|(sequence, observation)| {
                restored.reduce_persisted(
                    campaign_id,
                    comparator.clone(),
                    adapter,
                    observation.clone(),
                    replay_receipt(observation, sequence as u64),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        *self = restored;
        Ok(outcomes)
    }

    pub(crate) fn engage<A: CompetitionAdapterV1 + ?Sized, J: RawBoardJournalV1>(
        &mut self,
        campaign_id: &str,
        adapter: &mut A,
        journal: &mut J,
        now_ms: u64,
    ) -> Result<Result<BoardReduceOutcomeV1, BoardFetchFailureV1>, BoardReduceErrorV1> {
        let identity = match adapter.identify_competition() {
            Ok(identity) => identity,
            Err(failure) => return Ok(Err(self.fetch_failure(failure, now_ms))),
        };
        let observation = match adapter.fetch_board(None) {
            Ok(observation) => observation,
            Err(failure) => return Ok(Err(self.fetch_failure(failure, now_ms))),
        };
        if observation.competition != identity.competition {
            return Err(BoardReduceErrorV1::Invalid(
                "identified competition differs from board observation".into(),
            ));
        }
        self.observe(
            campaign_id,
            identity.comparator,
            adapter,
            journal,
            observation,
        )
        .map(Ok)
    }

    pub(crate) fn observe<A: CompetitionAdapterV1 + ?Sized, J: RawBoardJournalV1>(
        &mut self,
        campaign_id: &str,
        comparator: ObjectiveComparatorV1,
        adapter: &A,
        journal: &mut J,
        observation: BoardObservationV1,
    ) -> Result<BoardReduceOutcomeV1, BoardReduceErrorV1> {
        let receipt = journal
            .persist_raw(&observation)
            .map_err(BoardReduceErrorV1::Journal)?;
        self.reduce_persisted(campaign_id, comparator, adapter, observation, receipt)
    }

    fn reduce_persisted<A: CompetitionAdapterV1 + ?Sized>(
        &mut self,
        campaign_id: &str,
        comparator: ObjectiveComparatorV1,
        adapter: &A,
        observation: BoardObservationV1,
        receipt: RawObservationReceiptV1,
    ) -> Result<BoardReduceOutcomeV1, BoardReduceErrorV1> {
        comparator
            .validate()
            .map_err(|error| BoardReduceErrorV1::Invalid(error.into()))?;
        validate_persisted_observation(
            &observation,
            adapter.identity(),
            &receipt,
            self.current.as_ref(),
        )?;
        if self
            .applied_observations
            .contains(&observation.observation_id)
        {
            return Ok(reduce_outcome(
                self.current.as_ref(),
                BoardReduceEffectV1::DuplicateObservation,
                None,
                receipt,
            ));
        }
        let prior_sequence = self.current.as_ref().and_then(|board| board.last_sequence);
        let incoming_sequence = observation.cursor.sequence;
        let reordered = matches!(
            (observation.kind, prior_sequence, incoming_sequence),
            (BoardObservationKindV1::Delta, Some(prior), Some(incoming)) if incoming <= prior
        ) || matches!(
            (observation.kind, prior_sequence, incoming_sequence),
            (BoardObservationKindV1::Full, Some(prior), Some(incoming)) if incoming < prior
        );
        if reordered {
            self.applied_observations
                .insert(observation.observation_id.clone());
            self.pending_gap_observations
                .remove(&observation.observation_id);
            return Ok(reduce_outcome(
                self.current.as_ref(),
                BoardReduceEffectV1::ReorderedIgnored,
                self.gap_refresh_due_ms.map(refresh_action),
                receipt,
            ));
        }
        let gap = (observation.kind == BoardObservationKindV1::Delta
            && self.gap_refresh_due_ms.is_some())
            || matches!(
                (observation.kind, prior_sequence, incoming_sequence),
                (BoardObservationKindV1::Delta, Some(prior), Some(incoming))
                    if incoming > prior.saturating_add(1)
            )
            || (observation.kind == BoardObservationKindV1::Delta && self.current.is_none());
        if gap {
            let due = observation.provenance.observed_at_ms;
            let first_seen = self
                .pending_gap_observations
                .insert(observation.observation_id.clone());
            let refresh_due = *self.gap_refresh_due_ms.get_or_insert(due);
            if let (true, Some(board)) = (first_seen, self.current.as_mut()) {
                board.observation_revision = board.observation_revision.saturating_add(1);
                board.latest_observation_id = observation.observation_id.clone();
                board.latest_provenance = observation.provenance.clone();
                board.freshness = BoardFreshnessV1::Unconfirmed {
                    reason: "board sequence gap requires full refresh".into(),
                    refresh_due_ms: refresh_due,
                };
            }
            return Ok(reduce_outcome(
                self.current.as_ref(),
                BoardReduceEffectV1::GapDetected,
                Some(refresh_action(refresh_due)),
                receipt,
            ));
        }

        let entries = apply_board_changes(self.current.as_ref(), &observation)?;
        let current_epoch = self.current.as_ref().map_or(0, |board| board.board_epoch);
        let provisional = reduce_frontier(
            &entries,
            &comparator,
            current_epoch.saturating_add(1),
            &observation.observation_id,
            |contract, candidate, baseline| adapter.compare_scores(contract, candidate, baseline),
        )
        .map_err(BoardReduceErrorV1::Adapter)?;
        let changed = self.current.as_ref().is_none_or(|board| {
            board.comparator != comparator || !board.frontier.same_decision(&provisional)
        });
        let epoch = current_epoch.saturating_add(u64::from(changed));
        let frontier = if changed {
            provisional
        } else {
            self.current
                .as_ref()
                .expect("current decision")
                .frontier
                .clone()
        };
        let decision_sha256 = frontier
            .decision_sha256(&comparator)
            .map_err(BoardReduceErrorV1::Invalid)?;
        let predecessor_epoch = if changed {
            self.current.as_ref().map(|board| board.board_epoch)
        } else {
            self.current
                .as_ref()
                .and_then(|board| board.predecessor_epoch)
        };
        self.applied_observations
            .insert(observation.observation_id.clone());
        self.pending_gap_observations
            .remove(&observation.observation_id);
        let full_snapshot = observation.kind == BoardObservationKindV1::Full;
        let refresh = if full_snapshot {
            self.gap_refresh_due_ms = None;
            None
        } else {
            self.gap_refresh_due_ms.map(refresh_action)
        };
        let freshness = self
            .gap_refresh_due_ms
            .map_or(BoardFreshnessV1::Fresh, |due| {
                BoardFreshnessV1::Unconfirmed {
                    reason: "board sequence gap requires full refresh".into(),
                    refresh_due_ms: due,
                }
            });
        let board = CanonicalBoardV1 {
            schema: CANONICAL_BOARD_SCHEMA_V1.into(),
            campaign_id: campaign_id.into(),
            competition: observation.competition.clone(),
            comparator,
            board_epoch: epoch,
            predecessor_epoch,
            observation_revision: self
                .current
                .as_ref()
                .map_or(1, |board| board.observation_revision.saturating_add(1)),
            entries,
            frontier,
            decision_sha256,
            latest_observation_id: observation.observation_id,
            latest_provenance: observation.provenance,
            last_sequence: match (prior_sequence, incoming_sequence) {
                (Some(prior), Some(incoming)) => Some(prior.max(incoming)),
                (prior, incoming) => incoming.or(prior),
            },
            freshness,
        };
        let effect = if self.current.is_none() {
            BoardReduceEffectV1::Initialized
        } else if changed {
            BoardReduceEffectV1::DecisionAdvanced
        } else {
            BoardReduceEffectV1::DecisionRefreshed
        };
        self.current = Some(board);
        Ok(reduce_outcome(
            self.current.as_ref(),
            effect,
            refresh,
            receipt,
        ))
    }

    fn fetch_failure(&mut self, failure: AdapterFailureV1, now_ms: u64) -> BoardFetchFailureV1 {
        let terminal = failure.retry == RetryPolicyV1::Never
            || matches!(
                failure.class,
                AdapterFailureClassV1::Unsupported | AdapterFailureClassV1::Invariant
            );
        let (health, next) = if terminal {
            (DirectorHealthStateV1::NeedsAttention, remediation_action())
        } else {
            let at = match failure.retry {
                RetryPolicyV1::Immediate => now_ms,
                RetryPolicyV1::AfterMs(delay) => now_ms.saturating_add(delay),
                RetryPolicyV1::Never => unreachable!("terminal failures handled above"),
            };
            (DirectorHealthStateV1::Retrying, refresh_action(at))
        };
        if let Some(board) = self.current.as_mut()
            && !matches!(board.freshness, BoardFreshnessV1::Unconfirmed { .. })
        {
            board.freshness = BoardFreshnessV1::Stale {
                since_ms: now_ms,
                reason: format!("adapter {:?}", failure.class),
            };
        }
        BoardFetchFailureV1 {
            canonical: self.current.clone(),
            failure,
            health,
            next,
        }
    }
}

#[cfg(test)]
#[path = "board_reducer_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "board_reducer_adversarial_tests.rs"]
mod adversarial_tests;
