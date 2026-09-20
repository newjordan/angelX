use super::journal::{ActionJournalStateV1, ActionRecordV1};
use super::schema::{ActionPhaseV1, ScheduledActionV1};
use super::store::{CompetitionStore, StoreError, StoreRecoveryV1};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RecoveryDirectiveV1 {
    ExecutePlanned {
        attempt: u32,
    },
    ReconcileBeforeReissue {
        attempt: u32,
        reconcile_key: Option<String>,
        next: Option<ScheduledActionV1>,
    },
    RetryScheduled {
        attempt: u32,
        next: ScheduledActionV1,
    },
    TerminalCompleted {
        receipt_sha256: String,
    },
    TerminalFailed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RecoveredActionV1 {
    pub(crate) action_key: String,
    pub(crate) directive: RecoveryDirectiveV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RecoveryReportV1 {
    pub(crate) journal: ActionJournalStateV1,
    pub(crate) torn_tail_discarded: bool,
    pub(crate) actions: Vec<RecoveredActionV1>,
}

pub(crate) fn recover_store(store: &CompetitionStore) -> Result<RecoveryReportV1, StoreError> {
    let StoreRecoveryV1 {
        state,
        torn_tail_discarded,
    } = store.recover()?;
    let actions = state
        .actions
        .iter()
        .map(|(action_key, record)| {
            Ok(RecoveredActionV1 {
                action_key: action_key.clone(),
                directive: directive(record)?,
            })
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    Ok(RecoveryReportV1 {
        journal: state,
        torn_tail_discarded,
        actions,
    })
}

pub(crate) fn directive(record: &ActionRecordV1) -> Result<RecoveryDirectiveV1, StoreError> {
    let update = &record.update;
    Ok(match update.phase {
        ActionPhaseV1::Planned => RecoveryDirectiveV1::ExecutePlanned {
            attempt: update.attempt,
        },
        ActionPhaseV1::Started | ActionPhaseV1::Ambiguous => {
            RecoveryDirectiveV1::ReconcileBeforeReissue {
                attempt: update.attempt,
                reconcile_key: update.reconcile_key.clone(),
                next: update.next.clone(),
            }
        }
        ActionPhaseV1::Completed => RecoveryDirectiveV1::TerminalCompleted {
            receipt_sha256: update
                .receipt_sha256
                .clone()
                .expect("completed journal update is validated"),
        },
        ActionPhaseV1::Failed if update.retryable => RecoveryDirectiveV1::RetryScheduled {
            attempt: update
                .attempt
                .checked_add(1)
                .ok_or_else(|| "action attempt is exhausted".to_string())?,
            next: update
                .next
                .clone()
                .expect("retryable failure is validated with next action"),
        },
        ActionPhaseV1::Failed => RecoveryDirectiveV1::TerminalFailed,
    })
}
