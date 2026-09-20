use super::episode::{
    DEEP_CUT_EPISODE_SCHEMA_V1, EpisodeEventKindV1, EpisodeEventV1, EpisodeEvidenceLinkV1,
    EpisodeStartedV1, EpisodeTerminalV1, post_terminal_evidence, validate_event_body,
    validate_link, validate_terminal,
};
use super::episode_store::DurableEpisodeJournalV1;
use super::journal::{ActionJournalEventV1, ActionUpdateV1};
use super::schema::ActionPhaseV1;

#[derive(Clone)]
struct EpisodeAppendCursorV1 {
    episode_id: String,
    sequence: u64,
    previous_sha256: String,
}

pub(crate) struct EpisodeEffectPermitV1(EpisodeAppendCursorV1);
pub(crate) struct EpisodeEvidencePermitV1 {
    cursor: EpisodeAppendCursorV1,
    post_terminal: bool,
}

/// Opaque proof returned only after a durability authority has synced an event.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DurableEpisodeAppendReceiptV1 {
    episode_id: String,
    sequence: u64,
    event_sha256: String,
}

pub(crate) trait EpisodeDurabilityAuthorityV1 {
    fn verify_action_event_durable(&mut self, event: &ActionJournalEventV1) -> Result<(), String>;
    fn verify_action_head_durable(
        &mut self,
        sequence: u64,
        head_sha256: &str,
    ) -> Result<(), String>;
    fn append_and_sync(&mut self, event: &EpisodeEventV1) -> Result<(), String>;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct EpisodeStateV1 {
    episode_id: Option<String>,
    next_sequence: u64,
    head_sha256: String,
    start_journal_sequence: Option<u64>,
    terminal: Option<EpisodeTerminalV1>,
}

impl EpisodeStateV1 {
    pub(crate) fn append_durable(
        &mut self,
        authority: &mut impl EpisodeDurabilityAuthorityV1,
        event: &EpisodeEventV1,
    ) -> Result<DurableEpisodeAppendReceiptV1, String> {
        self.validate_next(event)?;
        let next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| "episode sequence exhausted".to_string())?;
        match &event.event {
            EpisodeEventKindV1::Started(start) => {
                authority.verify_action_event_durable(&start.start_journal_event)?
            }
            EpisodeEventKindV1::Terminal(terminal) => authority.verify_action_head_durable(
                terminal.action_journal_end_sequence,
                &terminal.action_journal_head_sha256,
            )?,
            EpisodeEventKindV1::EvidenceLinked(_) => {}
        }
        authority.append_and_sync(event)?;
        self.reduce(event, next_sequence);
        Ok(DurableEpisodeAppendReceiptV1 {
            episode_id: event.episode_id.clone(),
            sequence: event.sequence,
            event_sha256: event.event_sha256.clone(),
        })
    }

    pub(super) fn recover_durable(journal: &DurableEpisodeJournalV1) -> Result<Self, String> {
        let mut state = Self::default();
        for event in journal.events() {
            state.validate_next(event)?;
            let next_sequence = state
                .next_sequence
                .checked_add(1)
                .ok_or_else(|| "episode sequence exhausted".to_string())?;
            state.reduce(event, next_sequence);
        }
        Ok(state)
    }

    fn reduce(&mut self, event: &EpisodeEventV1, next_sequence: u64) {
        if let EpisodeEventKindV1::Started(start) = &event.event {
            self.episode_id = Some(event.episode_id.clone());
            self.start_journal_sequence = Some(start.start_journal_event.seq);
        }
        if let EpisodeEventKindV1::Terminal(terminal) = &event.event {
            self.terminal = Some(terminal.clone());
        }
        self.next_sequence = next_sequence;
        self.head_sha256 = event.event_sha256.clone();
    }

    pub(super) fn validate_next(&self, event: &EpisodeEventV1) -> Result<(), String> {
        if event.schema != DEEP_CUT_EPISODE_SCHEMA_V1
            || event.event_sha256 != event.canonical_sha256()?
            || event.sequence != self.next_sequence
            || event.previous_sha256 != self.head_sha256
            || self
                .episode_id
                .as_deref()
                .is_some_and(|id| id != event.episode_id)
        {
            return Err("invalid or non-append-only deep-cut episode event".into());
        }
        match (&self.episode_id, &self.terminal, &event.event) {
            (None, None, EpisodeEventKindV1::Started(start))
                if start.episode_id == event.episode_id =>
            {
                start.validate()
            }
            (None, _, _) => Err("EpisodeStarted must be durable before episode effects".into()),
            (Some(_), _, EpisodeEventKindV1::Started(_)) => Err("duplicate episode start".into()),
            (_, Some(_), EpisodeEventKindV1::EvidenceLinked(link))
                if post_terminal_evidence(link.kind) =>
            {
                validate_link(link)
            }
            (_, Some(_), _) => Err("episode is terminal for effects".into()),
            (_, None, EpisodeEventKindV1::Terminal(terminal)) => {
                validate_terminal(terminal)?;
                if terminal.action_journal_end_sequence
                    < self
                        .start_journal_sequence
                        .expect("started episode has journal start")
                {
                    return Err("terminal action-journal range moved backward".into());
                }
                Ok(())
            }
            (_, None, body) => validate_event_body(body),
        }
    }

    pub(crate) fn effect_permit(&self) -> Result<EpisodeEffectPermitV1, String> {
        if self.terminal.is_some() {
            return Err("episode is terminal".into());
        }
        Ok(EpisodeEffectPermitV1(self.cursor()?))
    }

    pub(crate) fn evidence_permit(&self) -> Result<EpisodeEvidencePermitV1, String> {
        Ok(EpisodeEvidencePermitV1 {
            cursor: self.cursor()?,
            post_terminal: self.terminal.is_some(),
        })
    }

    fn cursor(&self) -> Result<EpisodeAppendCursorV1, String> {
        Ok(EpisodeAppendCursorV1 {
            episode_id: self
                .episode_id
                .clone()
                .ok_or_else(|| "EpisodeStarted is not durable".to_string())?,
            sequence: self.next_sequence,
            previous_sha256: self.head_sha256.clone(),
        })
    }
}

impl EpisodeEventV1 {
    pub(crate) fn append(
        permit: &EpisodeEffectPermitV1,
        event: EpisodeEventKindV1,
    ) -> Result<Self, String> {
        if matches!(event, EpisodeEventKindV1::Started(_)) {
            return Err("duplicate episode start".into());
        }
        validate_event_body(&event)?;
        Self::from_cursor(&permit.0, event)
    }

    pub(crate) fn link_evidence(
        permit: &EpisodeEvidencePermitV1,
        link: EpisodeEvidenceLinkV1,
    ) -> Result<Self, String> {
        if permit.post_terminal && !post_terminal_evidence(link.kind) {
            return Err("terminal episode accepts official evidence only".into());
        }
        validate_link(&link)?;
        Self::from_cursor(&permit.cursor, EpisodeEventKindV1::EvidenceLinked(link))
    }

    fn from_cursor(
        cursor: &EpisodeAppendCursorV1,
        event: EpisodeEventKindV1,
    ) -> Result<Self, String> {
        Self::new(
            cursor.episode_id.clone(),
            cursor.sequence,
            cursor.previous_sha256.clone(),
            event,
        )
    }
}

impl DurableEpisodeAppendReceiptV1 {
    pub(crate) fn start_action_completion(
        &self,
        start: &EpisodeStartedV1,
        completed_at_ms: u64,
    ) -> Result<ActionUpdateV1, String> {
        start.validate()?;
        let started_event = EpisodeEventV1::started(start.clone())?;
        if self.episode_id != start.episode_id
            || self.sequence != 0
            || self.event_sha256 != started_event.event_sha256
            || completed_at_ms < start.started_at_ms
        {
            return Err("durable receipt does not complete this episode start".into());
        }
        Ok(ActionUpdateV1 {
            intent: start.start_journal_event.update.intent.clone(),
            attempt: start.start_journal_event.update.attempt,
            phase: ActionPhaseV1::Completed,
            retryable: false,
            at_ms: completed_at_ms,
            reconcile_key: None,
            receipt_sha256: Some(self.event_sha256.clone()),
            next: None,
        })
    }
}
