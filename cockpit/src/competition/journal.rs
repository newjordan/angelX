use super::schema::{ActionIntentV1, ActionPhaseV1, COMPETITION_CONTRACT_V1, ScheduledActionV1};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

pub(crate) const ACTION_JOURNAL_SCHEMA_V1: &str = "angel.competition-action-journal/v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActionUpdateV1 {
    pub(crate) intent: ActionIntentV1,
    pub(crate) attempt: u32,
    pub(crate) phase: ActionPhaseV1,
    pub(crate) retryable: bool,
    pub(crate) at_ms: u64,
    pub(crate) reconcile_key: Option<String>,
    pub(crate) receipt_sha256: Option<String>,
    pub(crate) next: Option<ScheduledActionV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActionJournalEventV1 {
    pub(crate) schema: String,
    pub(crate) seq: u64,
    pub(crate) previous_sha256: String,
    pub(crate) update: ActionUpdateV1,
    pub(crate) event_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActionRecordV1 {
    pub(crate) update: ActionUpdateV1,
    pub(crate) last_seq: u64,
    pub(crate) last_event_sha256: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActionJournalStateV1 {
    pub(crate) campaign_id: Option<String>,
    pub(crate) next_seq: u64,
    pub(crate) head_sha256: String,
    pub(crate) actions: BTreeMap<String, ActionRecordV1>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PrepareActionV1 {
    Append(Box<ActionJournalEventV1>),
    Replay { event_sha256: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum JournalError {
    InvalidIntent(&'static str),
    Conflict(String),
    InvalidTransition(String),
    InvalidEvent(String),
    Encoding(String),
}

impl fmt::Display for JournalError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIntent(reason) => write!(output, "invalid action intent: {reason}"),
            Self::Conflict(key) => write!(output, "action payload conflicts for key {key}"),
            Self::InvalidTransition(key) => write!(output, "invalid action transition for {key}"),
            Self::InvalidEvent(reason) => write!(output, "invalid journal event: {reason}"),
            Self::Encoding(reason) => write!(output, "journal encoding failed: {reason}"),
        }
    }
}

impl ActionJournalStateV1 {
    pub(crate) fn prepare(&self, update: ActionUpdateV1) -> Result<PrepareActionV1, JournalError> {
        if self
            .actions
            .get(&update.intent.action_key)
            .is_some_and(|current| current.update.intent != update.intent)
        {
            return Err(JournalError::Conflict(update.intent.action_key.clone()));
        }
        validate_update(&update)?;
        if let Some(campaign_id) = &self.campaign_id
            && campaign_id != &update.intent.campaign_id
        {
            return Err(JournalError::Conflict(update.intent.action_key.clone()));
        }
        if let Some(current) = self.actions.get(&update.intent.action_key) {
            if equivalent_update(&current.update, &update) {
                return Ok(PrepareActionV1::Replay {
                    event_sha256: current.last_event_sha256.clone(),
                });
            }
            let same_attempt = update.attempt == current.update.attempt;
            let retry_attempt = current.update.phase == ActionPhaseV1::Failed
                && current.update.retryable
                && update.phase == ActionPhaseV1::Planned
                && current.update.attempt.checked_add(1) == Some(update.attempt);
            let allowed = if current.update.phase == ActionPhaseV1::Failed
                && update.phase == ActionPhaseV1::Planned
            {
                retry_attempt
            } else {
                same_attempt
                    && current
                        .update
                        .phase
                        .allows(update.phase, current.update.retryable)
            };
            if !allowed {
                return Err(JournalError::InvalidTransition(
                    update.intent.action_key.clone(),
                ));
            }
        } else if update.phase != ActionPhaseV1::Planned || update.attempt != 0 {
            return Err(JournalError::InvalidTransition(
                update.intent.action_key.clone(),
            ));
        }
        let mut event = ActionJournalEventV1 {
            schema: ACTION_JOURNAL_SCHEMA_V1.to_string(),
            seq: self.next_seq,
            previous_sha256: self.head_sha256.clone(),
            update,
            event_sha256: String::new(),
        };
        event.event_sha256 = event.canonical_sha256()?;
        Ok(PrepareActionV1::Append(Box::new(event)))
    }

    pub(crate) fn apply(&mut self, event: &ActionJournalEventV1) -> Result<(), JournalError> {
        event.validate()?;
        if event.seq != self.next_seq || event.previous_sha256 != self.head_sha256 {
            return Err(JournalError::InvalidEvent(
                "hash-chain order mismatch".into(),
            ));
        }
        let next_seq = self
            .next_seq
            .checked_add(1)
            .ok_or_else(|| JournalError::InvalidEvent("journal sequence exhausted".into()))?;
        match self.prepare(event.update.clone())? {
            PrepareActionV1::Append(expected) if expected.event_sha256 == event.event_sha256 => {
                self.campaign_id
                    .get_or_insert_with(|| event.update.intent.campaign_id.clone());
                self.actions.insert(
                    event.update.intent.action_key.clone(),
                    ActionRecordV1 {
                        update: event.update.clone(),
                        last_seq: event.seq,
                        last_event_sha256: event.event_sha256.clone(),
                    },
                );
            }
            PrepareActionV1::Replay { .. } => {}
            _ => return Err(JournalError::InvalidEvent("event digest mismatch".into())),
        }
        self.next_seq = next_seq;
        self.head_sha256 = event.event_sha256.clone();
        Ok(())
    }
}

impl ActionJournalEventV1 {
    fn canonical_sha256(&self) -> Result<String, JournalError> {
        #[derive(Serialize)]
        struct Material<'a> {
            schema: &'a str,
            seq: u64,
            previous_sha256: &'a str,
            update: &'a ActionUpdateV1,
        }
        serde_json::to_vec(&Material {
            schema: &self.schema,
            seq: self.seq,
            previous_sha256: &self.previous_sha256,
            update: &self.update,
        })
        .map(|bytes| crate::cut::sha256_hex(&bytes))
        .map_err(|error| JournalError::Encoding(error.to_string()))
    }

    fn validate(&self) -> Result<(), JournalError> {
        if self.schema != ACTION_JOURNAL_SCHEMA_V1
            || self.event_sha256 != self.canonical_sha256()?
            || (self.seq == 0 && !self.previous_sha256.is_empty())
            || (self.seq > 0 && !is_sha256(&self.previous_sha256))
        {
            return Err(JournalError::InvalidEvent(
                "schema or digest mismatch".into(),
            ));
        }
        validate_update(&self.update)
    }
}

pub(crate) fn canonical_action_key(intent: &ActionIntentV1) -> Result<String, JournalError> {
    #[derive(Serialize)]
    struct KeyMaterial<'a> {
        contract: &'static str,
        campaign_id: &'a str,
        competition: &'a super::schema::CompetitionKeyV1,
        kind: super::schema::ActionKindV1,
        subject_id: &'a str,
        payload_sha256: &'a str,
        intent_version: &'a str,
    }
    serde_json::to_vec(&KeyMaterial {
        contract: COMPETITION_CONTRACT_V1,
        campaign_id: &intent.campaign_id,
        competition: &intent.competition,
        kind: intent.kind,
        subject_id: &intent.subject_id,
        payload_sha256: &intent.payload_sha256,
        intent_version: &intent.intent_version,
    })
    .map(|bytes| crate::cut::sha256_hex(&bytes))
    .map_err(|error| JournalError::Encoding(error.to_string()))
}

fn validate_update(update: &ActionUpdateV1) -> Result<(), JournalError> {
    let intent = &update.intent;
    if intent.validate().is_err() || canonical_action_key(intent)? != intent.action_key {
        return Err(JournalError::InvalidIntent("identity or digest mismatch"));
    }
    if update
        .next
        .as_ref()
        .is_some_and(|next| next.validate().is_err())
    {
        return Err(JournalError::InvalidIntent("invalid next action"));
    }
    let shape_ok = match update.phase {
        ActionPhaseV1::Planned | ActionPhaseV1::Started => {
            update.reconcile_key.is_none() && update.receipt_sha256.is_none()
        }
        ActionPhaseV1::Ambiguous => {
            update
                .reconcile_key
                .as_ref()
                .is_some_and(|key| !key.is_empty())
                && update.receipt_sha256.is_none()
                && update.next.is_some()
        }
        ActionPhaseV1::Completed => {
            update.receipt_sha256.as_deref().is_some_and(is_sha256)
                && update.reconcile_key.is_none()
                && !update.retryable
        }
        ActionPhaseV1::Failed => !update.retryable || update.next.is_some(),
    };
    if !shape_ok {
        return Err(JournalError::InvalidIntent("phase metadata mismatch"));
    }
    Ok(())
}

fn equivalent_update(left: &ActionUpdateV1, right: &ActionUpdateV1) -> bool {
    left.intent == right.intent
        && left.attempt == right.attempt
        && left.phase == right.phase
        && left.retryable == right.retryable
        && left.reconcile_key == right.reconcile_key
        && left.receipt_sha256 == right.receipt_sha256
        && left.next == right.next
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
