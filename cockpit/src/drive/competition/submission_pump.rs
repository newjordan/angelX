use super::candidate_store::CandidateRepositoryV1;
use super::journal::{ActionJournalStateV1, ActionUpdateV1, canonical_action_key};
use super::schema::{
    ActionIntentV1, ActionKindV1, ActionPhaseV1, COMPETITION_CONTRACT_V1, ScheduledActionV1,
};
use super::schema_validation::{validate_id, validate_sha256};
use super::submission::{
    SUBMIT_INTENT_VERSION_V1, SubmissionItemStateV1, SubmissionItemV1, SubmissionSpoolV1,
};
use super::submission_reconcile::{ReconcileActionStateV1, reduce_reconcile_action};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SubmissionPumpWorkV1 {
    Submit {
        submission_id: String,
        planned: Box<ActionUpdateV1>,
        started: Box<ActionUpdateV1>,
        idempotency_key: String,
        reconcile_key: String,
    },
    Reconcile {
        submission_id: String,
        reconcile_key: String,
        action: ReconcileActionStateV1,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SubmissionPumpDecisionV1 {
    pub(crate) work: Option<SubmissionPumpWorkV1>,
    pub(crate) next: ScheduledActionV1,
}

impl SubmissionSpoolV1 {
    pub(crate) fn decide(
        &self,
        repository: &CandidateRepositoryV1,
        journal: &ActionJournalStateV1,
        now_ms: u64,
    ) -> Result<SubmissionPumpDecisionV1, String> {
        self.validate()?;
        let mut next_due = None;
        for item in self.items.values() {
            match item.state {
                SubmissionItemStateV1::Stale { .. }
                | SubmissionItemStateV1::Failed { .. }
                | SubmissionItemStateV1::Acknowledged { .. } => continue,
                SubmissionItemStateV1::NeedsReconcile { next_attempt_at_ms }
                    if next_attempt_at_ms > now_ms =>
                {
                    next_due = Some(
                        next_due.map_or(next_attempt_at_ms, |due: u64| due.min(next_attempt_at_ms)),
                    );
                    continue;
                }
                _ => {}
            }
            if let Some(work) = work_for(item, repository, journal, now_ms)? {
                return Ok(decision(Some(work), now_ms));
            }
        }
        Ok(decision(
            None,
            next_due.unwrap_or(now_ms.saturating_add(60_000)),
        ))
    }
}

pub(super) fn seal_item(item: &mut SubmissionItemV1) -> Result<(), String> {
    #[derive(Serialize)]
    struct Request<'a> {
        contract: &'static str,
        campaign_id: &'a str,
        competition: &'a super::schema::CompetitionKeyV1,
        objective: &'a super::schema::ObjectiveComparatorV1,
        candidate_id: &'a str,
        candidate_record_sha256: &'a str,
        board: (u64, &'a str),
    }
    let body = serde_json::to_vec(&Request {
        contract: COMPETITION_CONTRACT_V1,
        campaign_id: &item.campaign_id,
        competition: &item.competition,
        objective: &item.objective,
        candidate_id: &item.candidate_id,
        candidate_record_sha256: &item.candidate_record_sha256,
        board: (item.board_epoch, &item.board_decision_sha256),
    })
    .map_err(|error| format!("encode submission request: {error}"))?;
    item.request_sha256 = crate::knowledge::cut::sha256_hex(&body);
    item.submission_id = digest_key("submission", &item.request_sha256)?;
    let mut intent = ActionIntentV1 {
        action_key: String::new(),
        campaign_id: item.campaign_id.clone(),
        competition: item.competition.clone(),
        kind: ActionKindV1::SubmitCandidate,
        subject_id: item.submission_id.clone(),
        payload_sha256: item.request_sha256.clone(),
        intent_version: SUBMIT_INTENT_VERSION_V1.into(),
    };
    intent.action_key = canonical_action_key(&intent).map_err(|error| error.to_string())?;
    item.action_key = intent.action_key;
    item.idempotency_key = digest_key("idempotency", &item.action_key)?;
    item.reconcile_key = digest_key("reconcile", &item.action_key)?;
    Ok(())
}

pub(super) fn validate_item(item: &SubmissionItemV1) -> Result<(), String> {
    for (id, label) in [
        (&item.submission_id, "invalid submission id"),
        (&item.campaign_id, "invalid submission campaign"),
        (&item.candidate_id, "invalid submission candidate"),
    ] {
        validate_id(id, label).map_err(str::to_string)?;
    }
    item.competition.validate().map_err(str::to_string)?;
    item.objective.validate().map_err(str::to_string)?;
    if item.board_epoch == 0 {
        return Err("submission board epoch must be positive".into());
    }
    for value in [
        &item.candidate_record_sha256,
        &item.board_decision_sha256,
        &item.request_sha256,
        &item.action_key,
        &item.idempotency_key,
        &item.reconcile_key,
    ] {
        validate_sha256(value).map_err(str::to_string)?;
    }
    validate_state(&item.state)?;
    super::submission_reconcile::validate_disposition_origin(item)?;
    let mut expected = item.clone();
    seal_item(&mut expected)?;
    if expected.submission_id != item.submission_id
        || expected.request_sha256 != item.request_sha256
        || expected.action_key != item.action_key
        || expected.idempotency_key != item.idempotency_key
        || expected.reconcile_key != item.reconcile_key
    {
        return Err("submission canonical identity mismatch".into());
    }
    Ok(())
}

pub(super) fn validate_state(state: &SubmissionItemStateV1) -> Result<(), String> {
    match state {
        SubmissionItemStateV1::Queued | SubmissionItemStateV1::NeedsReconcile { .. } => Ok(()),
        SubmissionItemStateV1::Stale { reason_code }
        | SubmissionItemStateV1::Failed { reason_code } => {
            validate_id(reason_code, "invalid submission reason").map_err(str::to_string)
        }
        SubmissionItemStateV1::Acknowledged {
            platform_submission_id,
            receipt_sha256,
        } => {
            validate_id(platform_submission_id, "invalid platform submission id")
                .map_err(str::to_string)?;
            validate_sha256(receipt_sha256).map_err(str::to_string)
        }
    }
}

fn work_for(
    item: &SubmissionItemV1,
    repository: &CandidateRepositoryV1,
    journal: &ActionJournalStateV1,
    now_ms: u64,
) -> Result<Option<SubmissionPumpWorkV1>, String> {
    if matches!(item.state, SubmissionItemStateV1::NeedsReconcile { .. }) {
        return reconcile_work(item, journal, now_ms);
    }
    let record = journal.actions.get(&item.action_key);
    if let Some(record) = record {
        if record.update.intent != submit_intent(item)? {
            return Err("submission action journal identity conflict".into());
        }
        return match record.update.phase {
            ActionPhaseV1::Planned if eligible(item, repository) => {
                Ok(Some(submit_work(item, record.update.attempt, now_ms)?))
            }
            ActionPhaseV1::Planned => Ok(None),
            ActionPhaseV1::Started | ActionPhaseV1::Ambiguous | ActionPhaseV1::Completed => {
                reconcile_work(item, journal, now_ms)
            }
            ActionPhaseV1::Failed => Ok(None),
        };
    }
    if eligible(item, repository) {
        Ok(Some(submit_work(item, 0, now_ms)?))
    } else {
        Ok(None)
    }
}

fn eligible(item: &SubmissionItemV1, repository: &CandidateRepositoryV1) -> bool {
    repository
        .require_eligible(&item.candidate_id)
        .is_ok_and(|candidate| {
            candidate.record_sha256 == item.candidate_record_sha256
                && candidate.board.board_epoch == item.board_epoch
                && candidate.board.board_decision_sha256 == item.board_decision_sha256
        })
}

fn submit_work(
    item: &SubmissionItemV1,
    attempt: u32,
    now_ms: u64,
) -> Result<SubmissionPumpWorkV1, String> {
    let intent = submit_intent(item)?;
    Ok(SubmissionPumpWorkV1::Submit {
        submission_id: item.submission_id.clone(),
        planned: Box::new(update(
            intent.clone(),
            ActionPhaseV1::Planned,
            attempt,
            now_ms,
        )),
        started: Box::new(update(intent, ActionPhaseV1::Started, attempt, now_ms)),
        idempotency_key: item.idempotency_key.clone(),
        reconcile_key: item.reconcile_key.clone(),
    })
}

fn reconcile_work(
    item: &SubmissionItemV1,
    journal: &ActionJournalStateV1,
    now_ms: u64,
) -> Result<Option<SubmissionPumpWorkV1>, String> {
    let Some(action) = reduce_reconcile_action(item, journal, now_ms)? else {
        return Ok(None);
    };
    Ok(Some(SubmissionPumpWorkV1::Reconcile {
        submission_id: item.submission_id.clone(),
        reconcile_key: item.reconcile_key.clone(),
        action,
    }))
}

pub(super) fn submit_intent(item: &SubmissionItemV1) -> Result<ActionIntentV1, String> {
    let mut intent = ActionIntentV1 {
        action_key: String::new(),
        campaign_id: item.campaign_id.clone(),
        competition: item.competition.clone(),
        kind: ActionKindV1::SubmitCandidate,
        subject_id: item.submission_id.clone(),
        payload_sha256: item.request_sha256.clone(),
        intent_version: SUBMIT_INTENT_VERSION_V1.into(),
    };
    intent.action_key = canonical_action_key(&intent).map_err(|error| error.to_string())?;
    Ok(intent)
}

fn update(
    intent: ActionIntentV1,
    phase: ActionPhaseV1,
    attempt: u32,
    at_ms: u64,
) -> ActionUpdateV1 {
    ActionUpdateV1 {
        intent,
        attempt,
        phase,
        retryable: false,
        at_ms,
        reconcile_key: None,
        receipt_sha256: None,
        next: None,
    }
}

pub(super) fn digest_key(label: &str, value: &str) -> Result<String, String> {
    serde_json::to_vec(&(COMPETITION_CONTRACT_V1, label, value))
        .map(|bytes| crate::knowledge::cut::sha256_hex(&bytes))
        .map_err(|error| format!("encode submission identity: {error}"))
}

fn decision(work: Option<SubmissionPumpWorkV1>, at_ms: u64) -> SubmissionPumpDecisionV1 {
    let action = match &work {
        Some(SubmissionPumpWorkV1::Submit { submission_id, .. }) => {
            format!("submit_candidate:{submission_id}")
        }
        Some(SubmissionPumpWorkV1::Reconcile { submission_id, .. }) => {
            format!("reconcile_submission:{submission_id}")
        }
        None => "scan_submission_spool".into(),
    };
    SubmissionPumpDecisionV1 {
        work,
        next: ScheduledActionV1 {
            action,
            next_attempt_at_ms: at_ms,
        },
    }
}
