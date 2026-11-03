use super::episode::{
    post_terminal_evidence, validate_link, validate_terminal, EpisodeEventKindV1, EpisodeEventV1,
    EpisodeEvidenceLinkV1, EpisodeStartedV1, EpisodeTerminalV1, DEEP_CUT_EPISODE_SCHEMA_V1,
};
use super::episode_store::DurableEpisodeJournalV1;
use super::journal::{ActionJournalEventV1, ActionUpdateV1};
use super::schema::ActionPhaseV1;
use crate::agent::harness::live_policy;
use serde::Deserialize;

/// The hot-loaded leg that makes this state machine's decisions.
const LEG: &str = "episode-reducer";

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

/// Every refusal the reducer can return, as the leg names it. Rust keeps the
/// wording: `fixed_message` is the constant half and the two body faults reuse
/// the message the native validation already produced.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum EpisodeLegFaultV1 {
    NotAppendOnly,
    StartFirst,
    DuplicateStart,
    TerminalEffects,
    RangeBackward,
    StartInvalid,
    LinkInvalid,
    TerminalInvalid,
    SequenceExhausted,
    EpisodeClosed,
    NeedsStart,
    NotOfficial,
    ReceiptMismatch,
}

impl EpisodeLegFaultV1 {
    fn fixed_message(self) -> Option<&'static str> {
        match self {
            Self::NotAppendOnly => Some("invalid or non-append-only deep-cut episode event"),
            Self::StartFirst => Some("EpisodeStarted must be durable before episode effects"),
            Self::DuplicateStart => Some("duplicate episode start"),
            Self::TerminalEffects => Some("episode is terminal for effects"),
            Self::RangeBackward => Some("terminal action-journal range moved backward"),
            Self::TerminalInvalid => Some("invalid deep-cut episode terminal"),
            Self::SequenceExhausted => Some("episode sequence exhausted"),
            Self::EpisodeClosed => Some("episode is terminal"),
            Self::NeedsStart => Some("EpisodeStarted is not durable"),
            Self::NotOfficial => Some("terminal episode accepts official evidence only"),
            Self::ReceiptMismatch => Some("durable receipt does not complete this episode start"),
            Self::StartInvalid | Self::LinkInvalid => None,
        }
    }
}

/// The phase an accepted event moves the episode to; `None` leaves it alone.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum EpisodeLegPhaseV1 {
    None,
    Started,
    Closed,
    Completed,
}

/// The leg's answer, deserialized from the compiled decision.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub(super) struct EpisodeLegDecisionV1 {
    accept: bool,
    fault: Option<EpisodeLegFaultV1>,
    phase: Option<EpisodeLegPhaseV1>,
    next_hi: u32,
    next_lo: u32,
    post_terminal: bool,
}

/// The same answer in the shape the native arms produce, so one value can be
/// checked against the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct EpisodeVerdictV1 {
    fault: Option<EpisodeLegFaultV1>,
    phase: EpisodeLegPhaseV1,
    next_sequence: u64,
    post_terminal: bool,
}

impl EpisodeVerdictV1 {
    fn accept(phase: EpisodeLegPhaseV1, next_sequence: u64, post_terminal: bool) -> Self {
        Self {
            fault: None,
            phase,
            next_sequence,
            post_terminal,
        }
    }

    fn deny(fault: EpisodeLegFaultV1) -> Self {
        Self {
            fault: Some(fault),
            phase: EpisodeLegPhaseV1::None,
            next_sequence: 0,
            post_terminal: false,
        }
    }
}

impl EpisodeLegDecisionV1 {
    fn from_verdict(verdict: EpisodeVerdictV1) -> Self {
        Self {
            accept: verdict.fault.is_none(),
            fault: verdict.fault,
            phase: verdict.fault.is_none().then_some(verdict.phase),
            next_hi: (verdict.next_sequence >> 32) as u32,
            next_lo: verdict.next_sequence as u32,
            post_terminal: verdict.post_terminal,
        }
    }

    fn verdict(self) -> EpisodeVerdictV1 {
        match self.fault {
            Some(fault) => EpisodeVerdictV1::deny(fault),
            None => EpisodeVerdictV1::accept(
                self.phase.unwrap_or(EpisodeLegPhaseV1::None),
                ((self.next_hi as u64) << 32) | self.next_lo as u64,
                self.post_terminal,
            ),
        }
    }
}

impl EpisodeVerdictV1 {
    /// Everything a leg decision must satisfy to be believed: an accepted event
    /// names the phase the state machine defines, carries no fault, and stores
    /// the next sequence and the permit's terminal flag.
    fn check(&self, decision: &EpisodeLegDecisionV1) -> Result<(), String> {
        if decision.accept != self.fault.is_none() {
            return Err("episode-reducer leg disagreed about acceptance".into());
        }
        match self.fault {
            Some(_) => decision
                .fault
                .map(|_| ())
                .ok_or_else(|| "episode-reducer leg refused without a fault".to_string()),
            None => {
                if decision.fault.is_some() {
                    return Err("episode-reducer leg accepted with a fault".into());
                }
                if decision.phase != Some(self.phase) {
                    return Err("episode-reducer leg named the wrong phase".into());
                }
                if decision.verdict().next_sequence != self.next_sequence {
                    return Err("episode-reducer leg stored the wrong sequence".into());
                }
                if decision.post_terminal != self.post_terminal {
                    return Err("episode-reducer leg reported the wrong terminal flag".into());
                }
                Ok(())
            }
        }
    }
}

/// Which event body the state machine is looking at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EpisodeEventTagV1 {
    Start,
    Evidence,
    Terminal,
}

impl EpisodeEventTagV1 {
    fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Evidence => "evidence",
            Self::Terminal => "terminal",
        }
    }
}

/// One decision's whole input: the reduced state, the event's own facts and the
/// chain equalities Rust already compared. Bodies, ids and hashes stay here.
#[derive(Clone, Debug)]
pub(super) struct EpisodeFactsV1 {
    has_episode: bool,
    has_terminal: bool,
    sequence: u64,
    journal_start: u64,
    tag: EpisodeEventTagV1,
    schema_ok: bool,
    sha_ok: bool,
    sequence_ok: bool,
    previous_ok: bool,
    episode_ok: bool,
    start_episode_equal: bool,
    start_error: Option<String>,
    link_error: Option<String>,
    link_post_terminal: bool,
    terminal_error: Option<String>,
    end: u64,
}

impl EpisodeFactsV1 {
    fn of_state(state: &EpisodeStateV1) -> Self {
        Self {
            has_episode: state.episode_id.is_some(),
            has_terminal: state.terminal.is_some(),
            sequence: state.next_sequence,
            journal_start: state.start_journal_sequence.unwrap_or_default(),
            tag: EpisodeEventTagV1::Evidence,
            schema_ok: true,
            sha_ok: true,
            sequence_ok: true,
            previous_ok: true,
            episode_ok: true,
            start_episode_equal: false,
            start_error: None,
            link_error: None,
            link_post_terminal: false,
            terminal_error: None,
            end: 0,
        }
    }

    fn of_event(state: &EpisodeStateV1, event: &EpisodeEventV1) -> Result<Self, String> {
        let mut facts = Self::of_state(state);
        facts.tag = match &event.event {
            EpisodeEventKindV1::Started(_) => EpisodeEventTagV1::Start,
            EpisodeEventKindV1::EvidenceLinked(_) => EpisodeEventTagV1::Evidence,
            EpisodeEventKindV1::Terminal(_) => EpisodeEventTagV1::Terminal,
        };
        facts.schema_ok = event.schema == DEEP_CUT_EPISODE_SCHEMA_V1;
        facts.sha_ok = event.event_sha256 == event.canonical_sha256()?;
        facts.sequence_ok = event.sequence == state.next_sequence;
        facts.previous_ok = event.previous_sha256 == state.head_sha256;
        facts.episode_ok = state
            .episode_id
            .as_deref()
            .is_none_or(|id| id == event.episode_id);
        match &event.event {
            EpisodeEventKindV1::Started(start) => {
                facts.start_episode_equal = start.episode_id == event.episode_id;
                facts.start_error = start.validate().err();
            }
            EpisodeEventKindV1::EvidenceLinked(link) => {
                facts.link_error = validate_link(link).err();
                facts.link_post_terminal = post_terminal_evidence(link.kind);
            }
            EpisodeEventKindV1::Terminal(terminal) => {
                facts.terminal_error = validate_terminal(terminal).err();
                facts.end = terminal.action_journal_end_sequence;
            }
        }
        Ok(facts)
    }

    /// The permit's own guarantees, restated as facts: a durable start, its
    /// sequence and head, and whether the episode is already closed.
    fn of_permit(cursor: &EpisodeAppendCursorV1, post_terminal: bool) -> Self {
        let mut facts = Self::of_state(&EpisodeStateV1::default());
        facts.has_episode = true;
        facts.has_terminal = post_terminal;
        facts.sequence = cursor.sequence;
        facts
    }

    /// The message the native reducer would have returned for `fault`.
    fn message(&self, fault: EpisodeLegFaultV1) -> String {
        match fault {
            EpisodeLegFaultV1::StartInvalid => self
                .start_error
                .clone()
                .unwrap_or_else(|| "invalid deep-cut episode start".into()),
            EpisodeLegFaultV1::LinkInvalid => self
                .link_error
                .clone()
                .unwrap_or_else(|| "invalid episode evidence identity".into()),
            other => other
                .fixed_message()
                .unwrap_or("invalid or non-append-only deep-cut episode event")
                .to_string(),
        }
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "state": {
                "has_episode": self.has_episode,
                "has_terminal": self.has_terminal,
            },
            "event": {
                "kind": self.tag.as_str(),
                "schema_ok": self.schema_ok,
                "sha_ok": self.sha_ok,
                "sequence_ok": self.sequence_ok,
                "previous_ok": self.previous_ok,
                "episode_ok": self.episode_ok,
                "start_episode_equal": self.start_episode_equal,
                "start_valid": self.start_error.is_none(),
                "link_valid": self.link_error.is_none(),
                "link_post_terminal": self.link_post_terminal,
                "terminal_valid": self.terminal_error.is_none(),
            },
        })
    }

    /// The request for one decision. Only the append-only chain and the
    /// terminal range read numbers, so those two carry the u64 halves and the
    /// rest stay boolean; the leg never marshals a word it does not compare.
    fn request(&self, op: &str) -> serde_json::Value {
        let mut value = self.json();
        if matches!(op, "validate" | "durable") {
            value["state"]["seq_hi"] = serde_json::json!((self.sequence >> 32) as u32);
            value["state"]["seq_lo"] = serde_json::json!(self.sequence as u32);
            value["state"]["journal_hi"] = serde_json::json!((self.journal_start >> 32) as u32);
            value["state"]["journal_lo"] = serde_json::json!(self.journal_start as u32);
            value["event"]["end_hi"] = serde_json::json!((self.end >> 32) as u32);
            value["event"]["end_lo"] = serde_json::json!(self.end as u32);
        }
        value["op"] = serde_json::Value::String(op.into());
        value
    }

    fn validate_verdict(&self) -> EpisodeVerdictV1 {
        use EpisodeLegFaultV1 as Fault;
        if !(self.schema_ok
            && self.sha_ok
            && self.sequence_ok
            && self.previous_ok
            && self.episode_ok)
        {
            return EpisodeVerdictV1::deny(Fault::NotAppendOnly);
        }
        match (self.has_episode, self.has_terminal, self.tag) {
            (false, false, EpisodeEventTagV1::Start) if self.start_episode_equal => {
                match self.start_error.is_none() {
                    true => EpisodeVerdictV1::accept(EpisodeLegPhaseV1::Started, 0, false),
                    false => EpisodeVerdictV1::deny(Fault::StartInvalid),
                }
            }
            (false, _, _) => EpisodeVerdictV1::deny(Fault::StartFirst),
            (true, _, EpisodeEventTagV1::Start) => EpisodeVerdictV1::deny(Fault::DuplicateStart),
            (_, true, EpisodeEventTagV1::Evidence) if self.link_post_terminal => {
                match self.link_error.is_none() {
                    true => EpisodeVerdictV1::accept(EpisodeLegPhaseV1::None, 0, false),
                    false => EpisodeVerdictV1::deny(Fault::LinkInvalid),
                }
            }
            (_, true, _) => EpisodeVerdictV1::deny(Fault::TerminalEffects),
            (_, false, EpisodeEventTagV1::Terminal) => {
                if self.terminal_error.is_some() {
                    return EpisodeVerdictV1::deny(Fault::TerminalInvalid);
                }
                match self.end >= self.journal_start {
                    true => EpisodeVerdictV1::accept(EpisodeLegPhaseV1::Closed, 0, false),
                    false => EpisodeVerdictV1::deny(Fault::RangeBackward),
                }
            }
            (_, false, _) => match self.link_error.is_none() {
                true => EpisodeVerdictV1::accept(EpisodeLegPhaseV1::None, 0, false),
                false => EpisodeVerdictV1::deny(Fault::LinkInvalid),
            },
        }
    }

    /// `append_durable`: the same acceptance, then the checked sequence
    /// increment that a durable append stores.
    fn advance_verdict(&self) -> EpisodeVerdictV1 {
        let verdict = self.validate_verdict();
        if verdict.fault.is_some() {
            return verdict;
        }
        match self.sequence.checked_add(1) {
            Some(next) => EpisodeVerdictV1::accept(verdict.phase, next, false),
            None => EpisodeVerdictV1::deny(EpisodeLegFaultV1::SequenceExhausted),
        }
    }

    /// `EpisodeEventV1::append` behind an effect permit: the permit already
    /// proved a durable start and an open episode, so only the body is asked.
    fn append_verdict(&self) -> EpisodeVerdictV1 {
        match self.tag {
            EpisodeEventTagV1::Start => EpisodeVerdictV1::deny(EpisodeLegFaultV1::DuplicateStart),
            EpisodeEventTagV1::Evidence => match self.link_error.is_none() {
                true => EpisodeVerdictV1::accept(EpisodeLegPhaseV1::None, 0, false),
                false => EpisodeVerdictV1::deny(EpisodeLegFaultV1::LinkInvalid),
            },
            EpisodeEventTagV1::Terminal => match self.terminal_error.is_none() {
                true => EpisodeVerdictV1::accept(EpisodeLegPhaseV1::None, 0, false),
                false => EpisodeVerdictV1::deny(EpisodeLegFaultV1::TerminalInvalid),
            },
        }
    }

    /// `link_evidence`: an already closed episode accepts official evidence only.
    fn link_verdict(&self) -> EpisodeVerdictV1 {
        if self.has_terminal && !self.link_post_terminal {
            return EpisodeVerdictV1::deny(EpisodeLegFaultV1::NotOfficial);
        }
        match self.link_error.is_none() {
            true => EpisodeVerdictV1::accept(EpisodeLegPhaseV1::None, 0, false),
            false => EpisodeVerdictV1::deny(EpisodeLegFaultV1::LinkInvalid),
        }
    }

    /// `effect_permit`: a closed episode grants none, and an episode without a
    /// durable start has no cursor to offer.
    fn effect_verdict(&self) -> EpisodeVerdictV1 {
        if self.has_terminal {
            EpisodeVerdictV1::deny(EpisodeLegFaultV1::EpisodeClosed)
        } else if !self.has_episode {
            EpisodeVerdictV1::deny(EpisodeLegFaultV1::NeedsStart)
        } else {
            EpisodeVerdictV1::accept(EpisodeLegPhaseV1::None, 0, false)
        }
    }

    /// `evidence_permit`: official evidence is the one thing a closed episode
    /// still takes, so the permit reports whether it is already terminal.
    fn evidence_verdict(&self) -> EpisodeVerdictV1 {
        if self.has_episode {
            EpisodeVerdictV1::accept(EpisodeLegPhaseV1::None, 0, self.has_terminal)
        } else {
            EpisodeVerdictV1::deny(EpisodeLegFaultV1::NeedsStart)
        }
    }
}

/// A durable start receipt and the start it claims to complete.
#[derive(Clone, Debug)]
pub(super) struct EpisodeReceiptFactsV1 {
    episode_equal: bool,
    sequence: u64,
    digest_equal: bool,
    start_error: Option<String>,
    completed_at_ms: u64,
    started_at_ms: u64,
}

impl EpisodeReceiptFactsV1 {
    fn verdict(&self) -> EpisodeVerdictV1 {
        if self.start_error.is_some() {
            return EpisodeVerdictV1::deny(EpisodeLegFaultV1::StartInvalid);
        }
        if self.episode_equal
            && self.sequence == 0
            && self.digest_equal
            && self.completed_at_ms >= self.started_at_ms
        {
            EpisodeVerdictV1::accept(EpisodeLegPhaseV1::Completed, 0, false)
        } else {
            EpisodeVerdictV1::deny(EpisodeLegFaultV1::ReceiptMismatch)
        }
    }

    fn request(&self) -> serde_json::Value {
        serde_json::json!({
            "op": "complete",
            "receipt": {
                "episode_equal": self.episode_equal,
                "seq_hi": (self.sequence >> 32) as u32,
                "seq_lo": self.sequence as u32,
                "sha_equal": self.digest_equal,
                "start_valid": self.start_error.is_none(),
                "completed_hi": (self.completed_at_ms >> 32) as u32,
                "completed_lo": self.completed_at_ms as u32,
                "started_hi": (self.started_at_ms >> 32) as u32,
                "started_lo": self.started_at_ms as u32,
            },
        })
    }

    fn message(&self, fault: EpisodeLegFaultV1) -> String {
        match fault {
            EpisodeLegFaultV1::StartInvalid => self
                .start_error
                .clone()
                .unwrap_or_else(|| "invalid deep-cut episode start".into()),
            other => other
                .fixed_message()
                .unwrap_or("durable receipt does not complete this episode start")
                .to_string(),
        }
    }
}

/// One leg evaluation. A pinned generation decides when it is offered and its
/// answer is in contract; otherwise the native arms answer and the source says
/// so.
fn decide(
    leg: Option<angel_live::Program>,
    request: &serde_json::Value,
    expected: EpisodeVerdictV1,
) -> (EpisodeLegDecisionV1, live_policy::LegSource) {
    let native = EpisodeLegDecisionV1::from_verdict(expected);
    let check = move |decision: &EpisodeLegDecisionV1| expected.check(decision);
    match leg {
        Some(program) => live_policy::decide_with(Some(program), LEG, request, check, || native),
        None => live_policy::decide_shipped(LEG, request, check),
    }
}

impl EpisodeStateV1 {
    pub(crate) fn append_durable(
        &mut self,
        authority: &mut impl EpisodeDurabilityAuthorityV1,
        event: &EpisodeEventV1,
    ) -> Result<DurableEpisodeAppendReceiptV1, String> {
        let next_sequence = self.append_with(None, event).0?;
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

    /// Whether this event can be appended, and the sequence that append would
    /// store. `validate_next` plus the checked increment, decided by the leg.
    pub(super) fn append_with(
        &self,
        leg: Option<angel_live::Program>,
        event: &EpisodeEventV1,
    ) -> (Result<u64, String>, live_policy::LegSource) {
        let facts = match EpisodeFactsV1::of_event(self, event) {
            Ok(facts) => facts,
            Err(error) => return (Err(error), live_policy::LegSource::Native),
        };
        let expected = facts.advance_verdict();
        let (decision, source) = decide(leg, &facts.request("durable"), expected);
        let verdict = decision.verdict();
        let result = match verdict.fault {
            Some(fault) => Err(facts.message(fault)),
            None => Ok(verdict.next_sequence),
        };
        (result, source)
    }

    /// The native arms alone, with no leg in reach: the parity oracle.
    #[cfg(test)]
    pub(super) fn append_native(&self, event: &EpisodeEventV1) -> Result<u64, String> {
        let facts = EpisodeFactsV1::of_event(self, event)?;
        let verdict = facts.advance_verdict();
        match verdict.fault {
            Some(fault) => Err(facts.message(fault)),
            None => Ok(verdict.next_sequence),
        }
    }

    pub(super) fn recover_durable(journal: &DurableEpisodeJournalV1) -> Result<Self, String> {
        let mut state = Self::default();
        for event in journal.events() {
            let next_sequence = state.append_with(None, event).0?;
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
        self.validate_next_with(None, event).0
    }

    pub(super) fn validate_next_with(
        &self,
        leg: Option<angel_live::Program>,
        event: &EpisodeEventV1,
    ) -> (Result<(), String>, live_policy::LegSource) {
        let facts = match EpisodeFactsV1::of_event(self, event) {
            Ok(facts) => facts,
            Err(error) => return (Err(error), live_policy::LegSource::Native),
        };
        let expected = facts.validate_verdict();
        let (decision, source) = decide(leg, &facts.request("validate"), expected);
        let verdict = decision.verdict();
        let result = match verdict.fault {
            Some(fault) => Err(facts.message(fault)),
            None => Ok(()),
        };
        (result, source)
    }

    /// The native arms alone, with no leg in reach: the parity oracle.
    #[cfg(test)]
    pub(super) fn validate_next_native(&self, event: &EpisodeEventV1) -> Result<(), String> {
        let facts = EpisodeFactsV1::of_event(self, event)?;
        match facts.validate_verdict().fault {
            Some(fault) => Err(facts.message(fault)),
            None => Ok(()),
        }
    }

    pub(crate) fn effect_permit(&self) -> Result<EpisodeEffectPermitV1, String> {
        self.effect_permit_with(None).0
    }

    pub(super) fn effect_permit_with(
        &self,
        leg: Option<angel_live::Program>,
    ) -> (
        Result<EpisodeEffectPermitV1, String>,
        live_policy::LegSource,
    ) {
        let facts = EpisodeFactsV1::of_state(self);
        let expected = facts.effect_verdict();
        let (decision, source) = decide(leg, &facts.request("effect"), expected);
        let result = match decision.verdict().fault {
            Some(fault) => Err(facts.message(fault)),
            None => self.cursor().map(EpisodeEffectPermitV1),
        };
        (result, source)
    }

    pub(crate) fn evidence_permit(&self) -> Result<EpisodeEvidencePermitV1, String> {
        self.evidence_permit_with(None).0
    }

    pub(super) fn evidence_permit_with(
        &self,
        leg: Option<angel_live::Program>,
    ) -> (
        Result<EpisodeEvidencePermitV1, String>,
        live_policy::LegSource,
    ) {
        let facts = EpisodeFactsV1::of_state(self);
        let expected = facts.evidence_verdict();
        let (decision, source) = decide(leg, &facts.request("evidence"), expected);
        let verdict = decision.verdict();
        let result = match verdict.fault {
            Some(fault) => Err(facts.message(fault)),
            None => self.cursor().map(|cursor| EpisodeEvidencePermitV1 {
                cursor,
                post_terminal: verdict.post_terminal,
            }),
        };
        (result, source)
    }

    /// The native arms alone, with no leg in reach: the parity oracle.
    #[cfg(test)]
    pub(super) fn effect_permit_native(&self) -> Result<EpisodeEffectPermitV1, String> {
        let facts = EpisodeFactsV1::of_state(self);
        match facts.effect_verdict().fault {
            Some(fault) => Err(facts.message(fault)),
            None => self.cursor().map(EpisodeEffectPermitV1),
        }
    }

    /// The native arms alone, with no leg in reach: the parity oracle.
    #[cfg(test)]
    pub(super) fn evidence_permit_native(&self) -> Result<EpisodeEvidencePermitV1, String> {
        let facts = EpisodeFactsV1::of_state(self);
        match facts.evidence_verdict().fault {
            Some(fault) => Err(facts.message(fault)),
            None => self.cursor().map(|cursor| EpisodeEvidencePermitV1 {
                cursor,
                post_terminal: facts.has_terminal,
            }),
        }
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
        Self::append_with(None, permit, event).0
    }

    pub(super) fn append_with(
        leg: Option<angel_live::Program>,
        permit: &EpisodeEffectPermitV1,
        event: EpisodeEventKindV1,
    ) -> (Result<Self, String>, live_policy::LegSource) {
        let facts = EpisodeFactsV1::of_permit(&permit.0, false);
        let expected = facts.append_verdict();
        let (decision, source) = decide(leg, &facts.request("append"), expected);
        let result = match decision.verdict().fault {
            Some(fault) => Err(facts.message(fault)),
            None => Self::from_cursor(&permit.0, event),
        };
        (result, source)
    }

    pub(crate) fn link_evidence(
        permit: &EpisodeEvidencePermitV1,
        link: EpisodeEvidenceLinkV1,
    ) -> Result<Self, String> {
        Self::link_evidence_with(None, permit, link).0
    }

    pub(super) fn link_evidence_with(
        leg: Option<angel_live::Program>,
        permit: &EpisodeEvidencePermitV1,
        link: EpisodeEvidenceLinkV1,
    ) -> (Result<Self, String>, live_policy::LegSource) {
        let mut facts = EpisodeFactsV1::of_permit(&permit.cursor, permit.post_terminal);
        facts.tag = EpisodeEventTagV1::Evidence;
        facts.link_error = validate_link(&link).err();
        facts.link_post_terminal = post_terminal_evidence(link.kind);
        let expected = facts.link_verdict();
        let (decision, source) = decide(leg, &facts.request("link"), expected);
        let result = match decision.verdict().fault {
            Some(fault) => Err(facts.message(fault)),
            None => Self::from_cursor(&permit.cursor, EpisodeEventKindV1::EvidenceLinked(link)),
        };
        (result, source)
    }

    /// The native arms alone, with no leg in reach: the parity oracle.
    #[cfg(test)]
    pub(super) fn append_native(
        permit: &EpisodeEffectPermitV1,
        event: EpisodeEventKindV1,
    ) -> Result<Self, String> {
        let facts = EpisodeFactsV1::of_permit(&permit.0, false);
        match facts.append_verdict().fault {
            Some(fault) => Err(facts.message(fault)),
            None => Self::from_cursor(&permit.0, event),
        }
    }

    /// The native arms alone, with no leg in reach: the parity oracle.
    #[cfg(test)]
    pub(super) fn link_evidence_native(
        permit: &EpisodeEvidencePermitV1,
        link: EpisodeEvidenceLinkV1,
    ) -> Result<Self, String> {
        let mut facts = EpisodeFactsV1::of_permit(&permit.cursor, permit.post_terminal);
        facts.link_error = validate_link(&link).err();
        facts.link_post_terminal = post_terminal_evidence(link.kind);
        match facts.link_verdict().fault {
            Some(fault) => Err(facts.message(fault)),
            None => Self::from_cursor(&permit.cursor, EpisodeEventKindV1::EvidenceLinked(link)),
        }
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

/// A state parked at a chosen next sequence, so the u64 exhaustion branch is
/// reachable in a test.
#[cfg(test)]
impl EpisodeStateV1 {
    pub(super) fn at_sequence_for_test(mut self, sequence: u64) -> Self {
        self.next_sequence = sequence;
        self
    }
}

impl DurableEpisodeAppendReceiptV1 {
    /// The receipt's own facts. The start's canonical digest is the only part
    /// that can fail, and it fails before the leg is asked.
    fn completion_facts(
        &self,
        start: &EpisodeStartedV1,
        completed_at_ms: u64,
    ) -> Result<EpisodeReceiptFactsV1, String> {
        let start_error = start.validate().err();
        let digest_equal = match start_error {
            Some(_) => false,
            None => self.event_sha256 == EpisodeEventV1::started(start.clone())?.event_sha256,
        };
        Ok(EpisodeReceiptFactsV1 {
            episode_equal: self.episode_id == start.episode_id,
            sequence: self.sequence,
            digest_equal,
            start_error,
            completed_at_ms,
            started_at_ms: start.started_at_ms,
        })
    }

    fn completed(&self, start: &EpisodeStartedV1, completed_at_ms: u64) -> ActionUpdateV1 {
        ActionUpdateV1 {
            intent: start.start_journal_event.update.intent.clone(),
            attempt: start.start_journal_event.update.attempt,
            phase: ActionPhaseV1::Completed,
            retryable: false,
            at_ms: completed_at_ms,
            reconcile_key: None,
            receipt_sha256: Some(self.event_sha256.clone()),
            next: None,
        }
    }

    pub(crate) fn start_action_completion(
        &self,
        start: &EpisodeStartedV1,
        completed_at_ms: u64,
    ) -> Result<ActionUpdateV1, String> {
        self.start_action_completion_with(None, start, completed_at_ms)
            .0
    }

    /// The native arms alone, with no leg in reach: the parity oracle.
    #[cfg(test)]
    pub(super) fn start_action_completion_native(
        &self,
        start: &EpisodeStartedV1,
        completed_at_ms: u64,
    ) -> Result<ActionUpdateV1, String> {
        let facts = self.completion_facts(start, completed_at_ms)?;
        match facts.verdict().fault {
            Some(fault) => Err(facts.message(fault)),
            None => Ok(self.completed(start, completed_at_ms)),
        }
    }

    pub(super) fn start_action_completion_with(
        &self,
        leg: Option<angel_live::Program>,
        start: &EpisodeStartedV1,
        completed_at_ms: u64,
    ) -> (Result<ActionUpdateV1, String>, live_policy::LegSource) {
        let facts = match self.completion_facts(start, completed_at_ms) {
            Ok(facts) => facts,
            Err(error) => return (Err(error), live_policy::LegSource::Native),
        };
        let expected = facts.verdict();
        let (decision, source) = decide(leg, &facts.request(), expected);
        let result = match decision.verdict().fault {
            Some(fault) => Err(facts.message(fault)),
            None => Ok(self.completed(start, completed_at_ms)),
        };
        (result, source)
    }
}
