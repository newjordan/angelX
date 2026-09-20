use super::journal::{ActionJournalStateV1, ActionUpdateV1, canonical_action_key};
use super::schema::{ActionIntentV1, ActionKindV1, ActionPhaseV1};
use super::submission::{
    RECONCILE_INTENT_VERSION_V1, SubmissionDispositionOriginV1, SubmissionItemStateV1,
    SubmissionItemV1,
};
use super::submission_pump::digest_key;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ReconcileActionStateV1 {
    Plan {
        planned: Box<ActionUpdateV1>,
        started: Box<ActionUpdateV1>,
    },
    Start {
        started: Box<ActionUpdateV1>,
    },
    Resume {
        phase: ActionPhaseV1,
    },
}

pub(crate) fn disposition_origin(
    action: &ActionUpdateV1,
) -> Result<SubmissionDispositionOriginV1, String> {
    let action_key = action.intent.action_key.clone();
    match action.intent.kind {
        ActionKindV1::SubmitCandidate => Ok(SubmissionDispositionOriginV1::Submit { action_key }),
        ActionKindV1::ReconcileSubmission => {
            Ok(SubmissionDispositionOriginV1::Reconcile { action_key })
        }
        _ => Err("action cannot produce a submission disposition".into()),
    }
}

pub(super) fn reduce_reconcile_action(
    item: &SubmissionItemV1,
    journal: &ActionJournalStateV1,
    now_ms: u64,
) -> Result<Option<ReconcileActionStateV1>, String> {
    let intent = reconcile_intent(item)?;
    let Some(record) = journal.actions.get(&intent.action_key) else {
        return Ok(Some(ReconcileActionStateV1::Plan {
            planned: Box::new(update(intent.clone(), ActionPhaseV1::Planned, now_ms)),
            started: Box::new(update(intent, ActionPhaseV1::Started, now_ms)),
        }));
    };
    if record.update.intent != intent {
        return Err("reconcile action journal identity conflict".into());
    }
    match record.update.phase {
        ActionPhaseV1::Planned => Ok(Some(ReconcileActionStateV1::Start {
            started: Box::new(update(intent, ActionPhaseV1::Started, now_ms)),
        })),
        ActionPhaseV1::Started | ActionPhaseV1::Ambiguous | ActionPhaseV1::Completed => {
            Ok(Some(ReconcileActionStateV1::Resume {
                phase: record.update.phase,
            }))
        }
        ActionPhaseV1::Failed => Ok(None),
    }
}

pub(super) fn require_disposition_effect(
    item: &SubmissionItemV1,
    journal: &ActionJournalStateV1,
    origin: &SubmissionDispositionOriginV1,
) -> Result<(), String> {
    let intent = match origin {
        SubmissionDispositionOriginV1::Submit { action_key } => {
            let intent = super::submission_pump::submit_intent(item)?;
            if action_key != &intent.action_key {
                return Err("submit disposition action key mismatch".into());
            }
            intent
        }
        SubmissionDispositionOriginV1::Reconcile { action_key } => {
            let intent = reconcile_intent(item)?;
            if action_key != &intent.action_key {
                return Err("reconcile disposition action key mismatch".into());
            }
            intent
        }
    };
    let record = journal
        .actions
        .get(&intent.action_key)
        .ok_or_else(|| "disposition requires its producing durable action".to_string())?;
    if record.update.intent != intent
        || !matches!(
            record.update.phase,
            ActionPhaseV1::Started | ActionPhaseV1::Ambiguous | ActionPhaseV1::Completed
        )
    {
        return Err("disposition is not bound to its durable producing action".into());
    }
    Ok(())
}

pub(super) fn require_pre_effect(
    item: &SubmissionItemV1,
    journal: &ActionJournalStateV1,
) -> Result<(), String> {
    if item.state != SubmissionItemStateV1::Queued || item.disposition_origin.is_some() {
        return Err("submission is no longer pre-effect queued work".into());
    }
    let intent = super::submission_pump::submit_intent(item)?;
    let Some(record) = journal.actions.get(&intent.action_key) else {
        return Ok(());
    };
    if record.update.intent == intent && record.update.phase == ActionPhaseV1::Planned {
        Ok(())
    } else {
        Err("started submission cannot be marked stale or failed".into())
    }
}

pub(super) fn validate_disposition_origin(item: &SubmissionItemV1) -> Result<(), String> {
    match (&item.state, &item.disposition_origin) {
        (SubmissionItemStateV1::Queued | SubmissionItemStateV1::Stale { .. }, None)
        | (SubmissionItemStateV1::Failed { .. }, None) => return Ok(()),
        (
            SubmissionItemStateV1::NeedsReconcile { .. }
            | SubmissionItemStateV1::Acknowledged { .. }
            | SubmissionItemStateV1::Failed { .. },
            Some(_),
        ) => {}
        _ => return Err("submission state lacks exact disposition origin".into()),
    }
    let origin = item.disposition_origin.as_ref().unwrap();
    let expected = match origin {
        SubmissionDispositionOriginV1::Submit { .. } => {
            super::submission_pump::submit_intent(item)?.action_key
        }
        SubmissionDispositionOriginV1::Reconcile { .. } => reconcile_intent(item)?.action_key,
    };
    let actual = match origin {
        SubmissionDispositionOriginV1::Submit { action_key }
        | SubmissionDispositionOriginV1::Reconcile { action_key } => action_key,
    };
    (actual == &expected)
        .then_some(())
        .ok_or_else(|| "submission disposition origin key mismatch".into())
}

pub(super) fn reconcile_intent(item: &SubmissionItemV1) -> Result<ActionIntentV1, String> {
    let payload = digest_key(&item.request_sha256, &item.reconcile_key)?;
    let mut intent = ActionIntentV1 {
        action_key: String::new(),
        campaign_id: item.campaign_id.clone(),
        competition: item.competition.clone(),
        kind: ActionKindV1::ReconcileSubmission,
        subject_id: item.submission_id.clone(),
        payload_sha256: payload,
        intent_version: RECONCILE_INTENT_VERSION_V1.into(),
    };
    intent.action_key = canonical_action_key(&intent).map_err(|error| error.to_string())?;
    Ok(intent)
}

fn update(intent: ActionIntentV1, phase: ActionPhaseV1, at_ms: u64) -> ActionUpdateV1 {
    ActionUpdateV1 {
        intent,
        attempt: 0,
        phase,
        retryable: false,
        at_ms,
        reconcile_key: None,
        receipt_sha256: None,
        next: None,
    }
}
