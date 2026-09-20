use super::candidate_store::CandidateRepositoryV1;
use super::schema::{CompetitionKeyV1, ObjectiveComparatorV1};
use super::schema_validation::{validate_id, validate_sha256};
use super::submission_results::{OfficialResultLedgerV1, OfficialResultV1};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const SUBMISSION_SPOOL_SCHEMA_V1: &str = "angel.competition-submission-spool/v1";
pub(crate) const SUBMIT_INTENT_VERSION_V1: &str = "competition-submit/v1";
pub(crate) const RECONCILE_INTENT_VERSION_V1: &str = "competition-reconcile/v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum SubmissionItemStateV1 {
    Queued,
    Stale {
        reason_code: String,
    },
    Failed {
        reason_code: String,
    },
    NeedsReconcile {
        next_attempt_at_ms: u64,
    },
    Acknowledged {
        platform_submission_id: String,
        receipt_sha256: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "origin", rename_all = "snake_case")]
pub(crate) enum SubmissionDispositionOriginV1 {
    Submit { action_key: String },
    Reconcile { action_key: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SubmissionItemV1 {
    pub(crate) order: u64,
    pub(crate) submission_id: String,
    pub(crate) campaign_id: String,
    pub(crate) competition: CompetitionKeyV1,
    pub(crate) objective: ObjectiveComparatorV1,
    pub(crate) candidate_id: String,
    pub(crate) candidate_record_sha256: String,
    pub(crate) board_epoch: u64,
    pub(crate) board_decision_sha256: String,
    pub(crate) request_sha256: String,
    pub(crate) action_key: String,
    pub(crate) idempotency_key: String,
    pub(crate) reconcile_key: String,
    pub(crate) state: SubmissionItemStateV1,
    pub(crate) disposition_origin: Option<SubmissionDispositionOriginV1>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SubmissionSpoolV1 {
    pub(crate) schema: String,
    pub(crate) revision: u64,
    pub(crate) next_order: u64,
    pub(crate) items: BTreeMap<u64, SubmissionItemV1>,
    pub(crate) official: OfficialResultLedgerV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SubmissionDispositionV1 {
    Acknowledged {
        platform_submission_id: String,
        receipt_sha256: String,
    },
    DefinitelyRejected {
        reason_code: String,
    },
    Ambiguous {
        reconcile_key: String,
        request_sha256: String,
        next_attempt_at_ms: u64,
    },
}

impl SubmissionSpoolV1 {
    pub(crate) fn new() -> Self {
        Self {
            schema: SUBMISSION_SPOOL_SCHEMA_V1.into(),
            official: OfficialResultLedgerV1::default(),
            ..Self::default()
        }
    }

    pub(crate) fn enqueue(
        &mut self,
        repository: &CandidateRepositoryV1,
        campaign_id: &str,
        candidate_id: &str,
    ) -> Result<String, String> {
        validate_id(campaign_id, "invalid submission campaign").map_err(str::to_string)?;
        let candidate = repository.require_eligible(candidate_id)?;
        if let Some(existing) = self.items.values().find(|item| {
            item.candidate_id == candidate_id
                && item.candidate_record_sha256 == candidate.record_sha256
        }) {
            return Ok(existing.submission_id.clone());
        }
        let order = self.next_order;
        let mut item = SubmissionItemV1 {
            order,
            submission_id: String::new(),
            campaign_id: campaign_id.into(),
            competition: candidate.competition.clone(),
            objective: candidate.objective.clone(),
            candidate_id: candidate_id.into(),
            candidate_record_sha256: candidate.record_sha256.clone(),
            board_epoch: candidate.board.board_epoch,
            board_decision_sha256: candidate.board.board_decision_sha256.clone(),
            request_sha256: String::new(),
            action_key: String::new(),
            idempotency_key: String::new(),
            reconcile_key: String::new(),
            state: SubmissionItemStateV1::Queued,
            disposition_origin: None,
        };
        super::submission_pump::seal_item(&mut item)?;
        let mut next = self.clone();
        next.next_order = order
            .checked_add(1)
            .ok_or_else(|| "submission order exhausted".to_string())?;
        next.items.insert(order, item.clone());
        next.bump_revision()?;
        next.validate()?;
        *self = next;
        Ok(item.submission_id)
    }

    pub(crate) fn mark_stale(
        &mut self,
        journal: &super::journal::ActionJournalStateV1,
        order: u64,
        reason: &str,
    ) -> Result<(), String> {
        super::submission_reconcile::require_pre_effect(self.item(order)?, journal)?;
        self.set_state(
            order,
            SubmissionItemStateV1::Stale {
                reason_code: reason.into(),
            },
        )
    }

    pub(crate) fn mark_failed(
        &mut self,
        journal: &super::journal::ActionJournalStateV1,
        order: u64,
        reason: &str,
    ) -> Result<(), String> {
        super::submission_reconcile::require_pre_effect(self.item(order)?, journal)?;
        self.set_state(
            order,
            SubmissionItemStateV1::Failed {
                reason_code: reason.into(),
            },
        )
    }

    pub(crate) fn record_disposition(
        &mut self,
        journal: &super::journal::ActionJournalStateV1,
        order: u64,
        origin: SubmissionDispositionOriginV1,
        disposition: SubmissionDispositionV1,
    ) -> Result<(), String> {
        let item = self.item(order)?;
        super::submission_reconcile::require_disposition_effect(item, journal, &origin)?;
        let state = match disposition {
            SubmissionDispositionV1::Acknowledged {
                platform_submission_id,
                receipt_sha256,
            } => {
                validate_id(&platform_submission_id, "invalid platform submission id")
                    .map_err(str::to_string)?;
                validate_sha256(&receipt_sha256).map_err(str::to_string)?;
                SubmissionItemStateV1::Acknowledged {
                    platform_submission_id,
                    receipt_sha256,
                }
            }
            SubmissionDispositionV1::DefinitelyRejected { reason_code } => {
                validate_id(&reason_code, "invalid rejection reason").map_err(str::to_string)?;
                SubmissionItemStateV1::Failed { reason_code }
            }
            SubmissionDispositionV1::Ambiguous {
                reconcile_key,
                request_sha256,
                next_attempt_at_ms,
            } if reconcile_key == item.reconcile_key && request_sha256 == item.request_sha256 => {
                SubmissionItemStateV1::NeedsReconcile { next_attempt_at_ms }
            }
            SubmissionDispositionV1::Ambiguous { .. } => {
                return Err("ambiguous disposition is not bound to this request".into());
            }
        };
        self.set_disposition_state(order, state, origin)
    }

    pub(crate) fn bind_official(&mut self, result: OfficialResultV1) -> Result<bool, String> {
        let mut next = self.clone();
        let appended = next.official.bind(&next.items, result)?;
        if appended {
            next.bump_revision()?;
            next.validate()?;
            *self = next;
        }
        Ok(appended)
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.schema != SUBMISSION_SPOOL_SCHEMA_V1 {
            return Err("invalid submission spool schema".into());
        }
        let mut ids = BTreeSet::new();
        for (order, item) in &self.items {
            if order != &item.order || !ids.insert(&item.submission_id) {
                return Err("submission order or identity conflict".into());
            }
            super::submission_pump::validate_item(item)?;
        }
        if self
            .items
            .keys()
            .next_back()
            .is_some_and(|order| *order >= self.next_order)
        {
            return Err("submission next order is not monotonic".into());
        }
        self.official.validate(&self.items)
    }

    fn set_state(&mut self, order: u64, state: SubmissionItemStateV1) -> Result<(), String> {
        super::submission_pump::validate_state(&state)?;
        let current = &self.item(order)?.state;
        if current == &state {
            return Ok(());
        }
        if matches!(
            current,
            SubmissionItemStateV1::Acknowledged { .. }
                | SubmissionItemStateV1::Failed { .. }
                | SubmissionItemStateV1::Stale { .. }
        ) {
            return Err("terminal submission disposition cannot be overwritten".into());
        }
        let mut next = self.clone();
        next.items.get_mut(&order).unwrap().state = state;
        next.bump_revision()?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    fn set_disposition_state(
        &mut self,
        order: u64,
        state: SubmissionItemStateV1,
        origin: SubmissionDispositionOriginV1,
    ) -> Result<(), String> {
        let current = self.item(order)?;
        if current.state == state && current.disposition_origin.as_ref() == Some(&origin) {
            return Ok(());
        }
        let mut next = self.clone();
        next.items.get_mut(&order).unwrap().disposition_origin = Some(origin);
        if current.state == state {
            if !matches!(state, SubmissionItemStateV1::NeedsReconcile { .. }) {
                return Err("terminal submission disposition cannot be overwritten".into());
            }
            next.bump_revision()?;
            next.validate()?;
        } else {
            next.set_state(order, state)?;
        }
        *self = next;
        Ok(())
    }

    fn item(&self, order: u64) -> Result<&SubmissionItemV1, String> {
        self.items
            .get(&order)
            .ok_or_else(|| "submission item is absent".to_string())
    }

    fn bump_revision(&mut self) -> Result<(), String> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| "submission spool revision exhausted".to_string())?;
        Ok(())
    }
}
