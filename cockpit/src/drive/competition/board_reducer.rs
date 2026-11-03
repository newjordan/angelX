use super::adapters::{AdapterFailureClassV1, AdapterFailureV1, CompetitionAdapterV1};
use super::board::*;
use super::frontier::reduce_frontier;
use super::schema::{DirectorHealthStateV1, ObjectiveComparatorV1, RetryPolicyV1};
use crate::agent::harness::live_policy;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The shipped Bend leg: one evaluation classifies a persisted observation.
/// The decision lives in `cockpit/live/board-reducer/board-reducer.bend`; Rust
/// keeps the features, the contract and the reducer's own state.
const LEG: &str = "board-reducer";

/// What the reducer does with an observation: ignore it as a duplicate, ignore
/// it as stale, ask for a full refresh because the sequence gapped, refuse it
/// because the observed competition differs, or replace the canonical board.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ObservationDecisionV1 {
    Conflict,
    Duplicate,
    Stale,
    Gap,
    Replace,
}

/// The leg's wire shape: a named verdict and nothing else.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ObservationVerdictV1 {
    decision: ObservationDecisionV1,
}

/// The features a disposition reads. Journals, adapters, hashes, the canonical
/// board and every `Result` stay here; the leg chooses only the verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ObservationDecisionInputV1 {
    identity_conflict: bool,
    duplicate: bool,
    full_snapshot: bool,
    has_prior: bool,
    prior_hi: u32,
    prior_lo: u32,
    has_incoming: bool,
    incoming_hi: u32,
    incoming_lo: u32,
    refresh_pending: bool,
    has_board: bool,
}

impl ObservationDecisionInputV1 {
    fn new(
        identity_conflict: bool,
        duplicate: bool,
        kind: BoardObservationKindV1,
        prior: Option<u64>,
        incoming: Option<u64>,
        refresh_pending: bool,
        has_board: bool,
    ) -> Self {
        Self {
            identity_conflict,
            duplicate,
            full_snapshot: kind == BoardObservationKindV1::Full,
            has_prior: prior.is_some(),
            prior_hi: prior.map_or(0, |value| (value >> 32) as u32),
            prior_lo: prior.map_or(0, |value| value as u32),
            has_incoming: incoming.is_some(),
            incoming_hi: incoming.map_or(0, |value| (value >> 32) as u32),
            incoming_lo: incoming.map_or(0, |value| value as u32),
            refresh_pending,
            has_board,
        }
    }

    /// What this input admits: the leg may not invent a conflict, hide one, or
    /// call a full snapshot gapped.
    fn check(&self, decision: ObservationDecisionV1) -> Result<(), String> {
        match decision {
            ObservationDecisionV1::Conflict if !self.identity_conflict => {
                Err("board reducer leg reported a conflict without a mismatched competition".into())
            }
            ObservationDecisionV1::Conflict => Ok(()),
            _ if self.identity_conflict => {
                Err("a mismatched competition identity must be a conflict".into())
            }
            ObservationDecisionV1::Gap if self.full_snapshot => {
                Err("a full snapshot cannot be a sequence gap".into())
            }
            ObservationDecisionV1::Stale if !(self.has_prior && self.has_incoming) => {
                Err("a stale observation needs both sequences".into())
            }
            _ => Ok(()),
        }
    }
}

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

    fn decision_input(
        &self,
        observation: &BoardObservationV1,
        identity_conflict: bool,
    ) -> ObservationDecisionInputV1 {
        ObservationDecisionInputV1::new(
            identity_conflict,
            self.applied_observations
                .contains(&observation.observation_id),
            observation.kind,
            self.current.as_ref().and_then(|board| board.last_sequence),
            observation.cursor.sequence,
            self.gap_refresh_due_ms.is_some(),
            self.current.is_some(),
        )
    }

    /// Route one disposition through the shipped leg. Any evaluation, decode
    /// or contract failure keeps the shipped decision, so a bad hot edit never
    /// moves the canonical board.
    fn observation_decision(
        &self,
        observation: &BoardObservationV1,
        identity_conflict: bool,
    ) -> ObservationDecisionV1 {
        let input = self.decision_input(observation, identity_conflict);
        let features = serde_json::to_value(input).unwrap_or(serde_json::Value::Null);
        let (verdict, source) =
            live_policy::decide_shipped(LEG, &features, |wire: &ObservationVerdictV1| {
                input.check(wire.decision)
            });
        tracing::debug!(
            leg = LEG,
            source = source.as_str(),
            decision = ?verdict.decision,
            "board observation disposition"
        );
        verdict.decision
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
        let identity_conflict = observation.competition != identity.competition;
        if self.observation_decision(&observation, identity_conflict)
            == ObservationDecisionV1::Conflict
        {
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
        let decision = self.observation_decision(&observation, false);
        match decision {
            ObservationDecisionV1::Conflict => unreachable!("a non-conflicting observation"),
            ObservationDecisionV1::Duplicate => {
                return Ok(reduce_outcome(
                    self.current.as_ref(),
                    BoardReduceEffectV1::DuplicateObservation,
                    None,
                    receipt,
                ));
            }
            ObservationDecisionV1::Stale => {
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
            ObservationDecisionV1::Gap => {
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
            ObservationDecisionV1::Replace => {}
        }
        let prior_sequence = self.current.as_ref().and_then(|board| board.last_sequence);
        let incoming_sequence = observation.cursor.sequence;

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
#[path = "../../../../tests/cockpit/competition/board_reducer_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/board_reducer_adversarial_tests.rs"]
mod adversarial_tests;
