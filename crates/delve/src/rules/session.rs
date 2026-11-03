//! Pure, exclusive game authority shared by embedded and worker execution.
//!
//! This reducer is preparatory: low-level Tick means only Run::step. Native
//! frame adapters must adopt BeginFrame/AbilityBatch/FinishFrame before replacing
//! the native owner. The read view is never a checkpoint.
use super::{Input, Run, card_paint, host_commands, phrasebook, scrolls};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{self, Write},
    sync::Arc,
};

pub mod abilities;
pub mod checkpoint;
pub mod frame;
pub mod jobs;
pub mod view;

pub const API: u32 = 1;
pub const CONTRACT: &str = "delve.session";
pub const MAX_TICK: u64 = super::runtime_state::MAX_TICK;
pub const MAX_EVENT_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024 - 4096;
const MAX_CONTEXT_BYTES: usize = 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub settings: crate::settings::Settings,
    pub phrasebook: (String, Vec<String>),
    pub wishes: Arc<Vec<crate::realm::Wish>>,
}
impl Context {
    pub fn validate(&self) -> Result<(), String> {
        if self.phrasebook.0.len() > MAX_CONTEXT_BYTES
            || self.phrasebook.1.len() > 256
            || self.phrasebook.1.iter().any(|s| s.len() > 8192)
            || self.wishes.len() > 4096
        {
            return Err("session source context exceeds bounds".into());
        }
        bounded(self, MAX_CONTEXT_BYTES).map(|_| ())
    }
}
impl Default for Context {
    fn default() -> Self {
        Self {
            settings: Default::default(),
            phrasebook: phrasebook::snapshot(),
            wishes: Arc::new(Vec::new()),
        }
    }
}

/// Local implementation hooks. No host RPC, model runner or resource owner is
/// reachable through this interface. The selected worker implements these in
/// process, including its existing injectable step fixture.
pub trait Policies {
    fn step(&mut self, run: &mut Run, inputs: &BTreeMap<u32, Input>) {
        run.step(inputs);
    }
    fn scroll(&mut self, request: &scrolls::Request) -> Result<scrolls::ResultValue, String> {
        request.evaluate()
    }
    fn home(
        &mut self,
        request: &super::home_settlement::HomePolicyRequest,
    ) -> Result<super::home_settlement::HomeResult, String> {
        request.evaluate()
    }
    fn brew(
        &mut self,
        request: &super::brew_jobs::PolicyRequest,
    ) -> Result<super::brew_jobs::PolicyResult, String> {
        request.evaluate()
    }
    fn reforge(
        &mut self,
        request: &super::reforge_jobs::PolicyRequest,
    ) -> Result<super::reforge_jobs::PolicyResult, String> {
        request.evaluate()
    }
    fn wish(
        &mut self,
        request: &super::wish_jobs::PolicyRequest,
    ) -> Result<super::wish_jobs::PolicyResult, String> {
        request.evaluate()
    }
    fn paint(&mut self, request: &card_paint::Request) -> Result<card_paint::Decision, String> {
        request.evaluate()
    }
}
#[derive(Default)]
pub struct DefaultPolicies;
impl Policies for DefaultPolicies {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum Event {
    /// Step-only parity; transport ordering is deliberately not implemented here.
    Tick {
        inputs: BTreeMap<u32, Input>,
        step: bool,
    },
    ContextChanged(Context),
    /// Native authored completion; only selected visual updates enter the book.
    AuthoredCards {
        cards: Vec<super::cards::Card>,
    },
    Commands(host_commands::Request),
    Job(jobs::Event),
    Ability(abilities::Event),
    ControlsChanged(frame::ControlChange),
    CampaignChanged {
        observed: super::campaign::Campaign,
        campaign: super::campaign::Campaign,
    },
    BeginFrame(frame::Begin),
    FinishFrame(frame::Finish),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub api: u32,
    pub session: String,
    pub raid: u64,
    pub seq: u64,
    pub expected_view_revision: u64,
    pub event: Event,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum Reply {
    Done,
    Changed(bool),
    Commands(host_commands::Receipt),
    Job(jobs::Reply),
    Ability(abilities::Reply),
    FrameBegun(frame::Begun),
    FrameFinished(frame::Outputs),
    /// A valid accepted exchange with an application refusal. Journal pressure
    /// and logical rejections do not make the owner uncertain.
    Refused(String),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub api: u32,
    pub session: String,
    pub raid: u64,
    pub seq: u64,
    pub view: view::Frame,
    pub reply: Reply,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum Query {
    Cursor,
    BrewReady,
    Abilities,
    HomeReady {
        context: Option<super::home_settlement::HomeContext>,
    },
    VotesReady,
    QueuedWishes {
        limit: u16,
    },
    FramePhase,
    Job(jobs::Query),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum QueryReply {
    BrewReady(bool),
    HomeReady(bool),
    VotesReady(bool),
    QueuedWishes(Vec<super::genie::Asked>),
    FramePhase(Option<frame::PhaseInfo>),
    Abilities(abilities::Claims),
    Cursor {
        next_request: u64,
        event_seq: u64,
        view_revision: u64,
    },
    Job(jobs::Reply),
}

/// Semantic witness during staged recovery. No dictionary/view bytes and no
/// external output dispatch are produced; supervisor compares retained replies.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayFrame {
    pub api: u32,
    pub session: String,
    pub raid: u64,
    pub seq: u64,
    pub view_revision: u64,
    pub reply: Reply,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryEnvelope {
    pub api: u32,
    pub session: String,
    pub raid: u64,
    pub expected_view_revision: Option<u64>,
    pub query: Query,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryFrame {
    pub api: u32,
    pub session: String,
    pub raid: u64,
    pub event_seq: u64,
    pub view_revision: u64,
    pub reply: QueryReply,
}

pub struct Session {
    run: Run,
    journal: host_commands::Journal,
    context: Context,
    event_seq: u64,
    view_revision: u64,
    publisher: view::Publisher,
    view_limits: view::Limits,
    parsed: Arc<phrasebook::ScopedBook>,
    journal_outbox: view::Outbox,
    job_revision: u64,
    baseline_ready: bool,
    poisoned: bool,
    // One bounded outstanding exchange, not a request receipt ledger. Durable
    // identified commands retain the Journal's independent receipt semantics.
    last: Option<(Envelope, Frame)>,
    pending_frame: Option<frame::Pending>,
}
impl Session {
    pub fn new(
        run: Run,
        journal: host_commands::Journal,
        context: Context,
    ) -> Result<Self, String> {
        // Initialization is a deliberate full validation boundary. Native
        // appearance overlays must remain in their original native owner.
        pure_run(&run)?;
        checkpoint::capture(&run, &journal, &context, 0, 0)?;
        Self::from_parts(run, journal, context, 0, 0, None)
    }
    pub fn restore(checkpoint: checkpoint::SessionCheckpoint) -> Result<Self, String> {
        let state = checkpoint::restore(checkpoint)?;
        Self::from_parts(
            state.run,
            state.journal,
            state.context,
            state.event_seq,
            state.view_revision,
            state.pending_frame,
        )
    }
    fn from_parts(
        run: Run,
        journal: host_commands::Journal,
        context: Context,
        event_seq: u64,
        view_revision: u64,
        pending_frame: Option<frame::Pending>,
    ) -> Result<Self, String> {
        let publisher = view::Publisher::new(
            journal.session().into(),
            run.raid_id,
            view_revision,
            view::Limits::default(),
        )?;
        let parsed =
            phrasebook::prepare_scoped(context.phrasebook.0.clone(), context.phrasebook.1.clone());
        let mut owner = Self {
            run,
            journal,
            context,
            event_seq,
            view_revision,
            publisher,
            view_limits: view::Limits::default(),
            parsed,
            journal_outbox: Default::default(),
            job_revision: event_seq,
            baseline_ready: false,
            poisoned: false,
            last: None,
            pending_frame,
        };
        owner.refresh_outbox()?;
        Ok(owner)
    }
    fn healthy(&self) -> Result<(), String> {
        if self.poisoned {
            Err("session owner is poisoned; restore accepted checkpoint and replay tail".into())
        } else {
            Ok(())
        }
    }
    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }
    pub fn checkpoint(&self) -> Result<checkpoint::SessionCheckpoint, String> {
        self.healthy()?;
        let _settings = crate::settings::scoped(self.context.settings);
        let _book = phrasebook::scoped(self.parsed.clone());
        checkpoint::capture_with_frame(
            &self.run,
            &self.journal,
            &self.context,
            self.event_seq,
            self.view_revision,
            self.pending_frame.as_ref(),
        )
    }
    pub fn query(&self, envelope: QueryEnvelope) -> Result<QueryFrame, String> {
        self.healthy()?;
        let _settings = crate::settings::scoped(self.context.settings);
        let _book = phrasebook::scoped(self.parsed.clone());
        bounded(&envelope, MAX_EVENT_BYTES)?;
        if envelope.api != API
            || envelope.session != self.journal.session()
            || envelope.raid != self.run.raid_id
            || envelope
                .expected_view_revision
                .is_some_and(|revision| revision != self.view_revision)
        {
            return Err("stale or foreign session query".into());
        }
        let reply = match envelope.query {
            Query::Cursor => QueryReply::Cursor {
                next_request: self.journal.next_id()?,
                event_seq: self.event_seq,
                view_revision: self.view_revision,
            },
            Query::Abilities => {
                QueryReply::Abilities(abilities::claim(&self.run, self.journal.session())?)
            }
            Query::HomeReady { context } => {
                QueryReply::HomeReady(self.journal.has_home_work(&self.run, context.as_ref())?)
            }
            Query::BrewReady => QueryReply::BrewReady(!self.run.brewing.is_empty()),
            Query::VotesReady => QueryReply::VotesReady(self.journal.votes_ready(&self.run)),
            Query::QueuedWishes { limit } => {
                if !(1..=64).contains(&limit) {
                    return Err("invalid queued wish query limit".into());
                }
                QueryReply::QueuedWishes(
                    self.run
                        .asked
                        .iter()
                        .take(usize::from(limit))
                        .cloned()
                        .collect(),
                )
            }
            Query::FramePhase => {
                QueryReply::FramePhase(self.pending_frame.as_ref().map(|pending| {
                    frame::PhaseInfo {
                        begun: frame::Begun {
                            frame: pending.frame.clone(),
                            arrival: pending.arrival.clone(),
                        },
                        abilities_applied: pending.abilities_applied,
                    }
                }))
            }
            Query::Job(query) => QueryReply::Job(jobs::query(&self.journal, query)?),
        };
        let frame = QueryFrame {
            api: API,
            session: self.journal.session().into(),
            raid: self.run.raid_id,
            event_seq: self.event_seq,
            view_revision: self.view_revision,
            reply,
        };
        bounded(&frame, MAX_FRAME_BYTES)?;
        Ok(frame)
    }
    fn refresh_outbox(&mut self) -> Result<(), String> {
        self.journal_outbox = view::Outbox {
            realm: self.journal.has_pending_realm(),
            home: self.journal.has_pending_home(),
            brews: self
                .journal
                .pending_brews()?
                .len()
                .try_into()
                .map_err(|_| "too many brew claims")?,
            reforges: self
                .journal
                .pending_reforges()?
                .len()
                .try_into()
                .map_err(|_| "too many reforge claims")?,
            wishes: self
                .journal
                .pending_wishes()?
                .len()
                .try_into()
                .map_err(|_| "too many wish claims")?,
            playtest: self.journal.pending_votes()?.is_some(),
        };
        Ok(())
    }
    fn status(&self) -> Result<view::Status, String> {
        let mut outbox = self.journal_outbox.clone();
        // Queued work can appear during a tick without allocating a Journal ID.
        outbox.brews = (usize::from(outbox.brews) + self.run.brewing.len())
            .try_into()
            .map_err(|_| "too much queued brew work")?;
        outbox.wishes = (usize::from(outbox.wishes) + self.run.asked.len())
            .try_into()
            .map_err(|_| "too much queued wish work")?;
        outbox.playtest |= self.run.playtest.dirty;
        outbox.home = self.journal.has_home_work(&self.run, None)?;
        outbox.realm |= !self.run.bank.is_empty()
            || !self.run.reclaimed.is_empty()
            || self.run.triumph.is_some();
        // Context-sensitive economy changes additionally query HomeReady with
        // captured host HomeContext; this summary reflects exact None-context readiness.
        Ok(view::Status::capture(&self.run, self.job_revision, outbox))
    }
    fn validate_after(&self, event_seq: u64) -> Result<(), String> {
        pure_run(&self.run)?;
        super::abilities::validate_outbox(&self.run.module_casts)?;
        for hero in self.run.players.values() {
            for slot in hero.module_spells.iter().flatten() {
                slot.validate()?;
            }
        }
        if let Some(pending) = &self.pending_frame {
            pending.validate(&self.run, event_seq, self.journal.session())?;
        }
        if self.run.tick > MAX_TICK || !self.run.valid_snapshot() {
            return Err("session mutation produced invalid Run".into());
        }
        self.journal.validate_home_world(&self.run)?;
        self.journal.validate_brew_world(&self.run)?;
        self.journal.validate_reforge_world(&self.run)?;
        self.journal.validate_wish_world(&self.run)?;
        if !super::brew_jobs::valid_outbox(&self.run.brewing, self.run.raid_id)
            || !self.run.asked.iter().all(super::wish_jobs::valid_ask)
        {
            return Err("session mutation produced invalid queued work".into());
        }
        scrolls::validate_run(&self.run)?;
        Ok(())
    }
    fn dispatch(
        &mut self,
        event: Event,
        event_seq: u64,
        policies: &mut impl Policies,
    ) -> Result<Reply, String> {
        // AbilityBatch is the sole mutation permitted in this
        // gap. Its dedicated handler must validate the captured frame proof.
        if self.pending_frame.is_some()
            && !matches!(
                &event,
                Event::FinishFrame(_) | Event::Ability(abilities::Event::Batch(_))
            )
        {
            return Err("a frame is awaiting native abilities and finish".into());
        }
        match event {
            Event::Tick { inputs, step } => {
                if step {
                    policies.step(&mut self.run, &inputs);
                }
                Ok(Reply::Done)
            }
            Event::ContextChanged(context) => {
                if context == self.context {
                    return Ok(Reply::Changed(false));
                }
                if context.phrasebook != self.context.phrasebook {
                    self.parsed = phrasebook::prepare_scoped(
                        context.phrasebook.0.clone(),
                        context.phrasebook.1.clone(),
                    );
                }
                self.context = context;
                Ok(Reply::Changed(true))
            }
            Event::AuthoredCards { cards } => {
                let request = card_paint::Request {
                    candidates: cards
                        .into_iter()
                        .filter_map(|authored| {
                            self.run
                                .book
                                .get(&authored.id)
                                .cloned()
                                .map(|current| card_paint::Candidate { current, authored })
                        })
                        .collect(),
                };
                if request.candidates.is_empty() {
                    return Ok(Reply::Changed(false));
                }
                request.validate()?;
                let decision = policies.paint(&request)?;
                request.validate_result(&decision)?;
                let updates = request.updates(&decision)?;
                if updates.is_empty() {
                    return Ok(Reply::Changed(false));
                }
                let command = host_commands::Request {
                    session: self.journal.session().into(),
                    raid: self.run.raid_id,
                    id: self.journal.next_id()?,
                    commands: vec![host_commands::Command::PaintCards { updates }],
                };
                let receipt = self.journal.apply(&mut self.run, command)?;
                // Exact event replay is Session-owned; the existing Journal
                // receipt is retired only after the atomic catalogue commit.
                self.journal.acknowledge(self.run.raid_id, receipt.id)?;
                match receipt.result?.as_slice() {
                    [host_commands::Reply::Changed(changed)] => Ok(Reply::Changed(*changed)),
                    _ => Err("paint command returned an invalid reply".into()),
                }
            }
            Event::Commands(request) => self
                .journal
                .apply_with_scroll(&mut self.run, request, |run, player, intent| {
                    let captured = scrolls::Capture::new(run, player, intent)?;
                    let result = policies.scroll(&captured.request)?;
                    captured.request.validate_result(&result)?;
                    captured.prepare(run, result)
                })
                .map(Reply::Commands),
            Event::Ability(event) => abilities::apply(
                &mut self.run,
                self.pending_frame.as_mut(),
                self.journal.session(),
                event,
            )
            .map(Reply::Ability),
            Event::ControlsChanged(input) => {
                frame::controls_changed(&mut self.run, input).map(Reply::Changed)
            }
            Event::CampaignChanged { observed, campaign } => {
                if self.run.campaign != observed {
                    return Err("campaign source observation changed".into());
                }
                let changed = self.run.campaign != campaign;
                if changed {
                    self.run.campaign = campaign;
                }
                Ok(Reply::Changed(changed))
            }
            Event::BeginFrame(input) => {
                let pending = frame::begin(
                    &mut self.run,
                    self.journal.session(),
                    event_seq,
                    input,
                    policies,
                )?;
                let begun = frame::Begun {
                    frame: pending.frame.clone(),
                    arrival: pending.arrival.clone(),
                };
                self.pending_frame = Some(pending);
                Ok(Reply::FrameBegun(begun))
            }
            Event::FinishFrame(input) => {
                let pending = self
                    .pending_frame
                    .as_ref()
                    .ok_or("no frame awaits finish")?
                    .clone();
                let output = frame::finish(&mut self.run, pending, input, &self.context)?;
                self.pending_frame = None;
                Ok(Reply::FrameFinished(output))
            }
            Event::Job(event) => {
                jobs::apply(&mut self.run, &mut self.journal, event, policies).map(Reply::Job)
            }
        }
    }
}

impl Session {
    fn preflight(&self, envelope: &Envelope) -> Result<(), String> {
        let session = self;
        if envelope.api != API
            || envelope.session != session.journal.session()
            || envelope.raid != session.run.raid_id
        {
            return Err("session event belongs to another API/session/raid".into());
        }
        if envelope.seq
            != session
                .event_seq
                .checked_add(1)
                .ok_or("session event sequence exhausted")?
            || envelope.expected_view_revision != session.view_revision
        {
            return Err("stale or out-of-order session event".into());
        }
        session
            .view_revision
            .checked_add(1)
            .ok_or("session view revision exhausted")?;
        match &envelope.event {
            Event::Tick { inputs, step } => {
                if inputs.len() > super::MAX_PLAYERS as usize
                    || inputs.keys().any(|p| !(1..=super::MAX_PLAYERS).contains(p))
                {
                    return Err("invalid session input seats".into());
                }
                if *step && session.run.tick >= MAX_TICK {
                    return Err("session tick exhausted".into());
                }
            }
            Event::ContextChanged(context) => context.validate()?,
            Event::AuthoredCards { cards } => {
                if cards.len() > card_paint::MAX_CANDIDATES {
                    return Err("too many authored paint cards".into());
                }
            }
            Event::Commands(request) => {
                if request.session != envelope.session || request.raid != envelope.raid {
                    return Err("nested command ownership differs".into());
                }
                request.validate_payload()?;
            }
            Event::Job(_)
            | Event::Ability(_)
            | Event::ControlsChanged(_)
            | Event::CampaignChanged { .. }
            | Event::BeginFrame(_)
            | Event::FinishFrame(_) => {}
        }
        Ok(())
    }
    fn reduce(
        &mut self,
        envelope: &Envelope,
        policies: &mut impl Policies,
    ) -> Result<Reply, String> {
        let session = self;
        // Once dispatch begins, logical failures are accepted typed replies.
        // A later invariant/publication failure instead poisons this owner.
        let queued_before = (
            session.run.brewing.len(),
            session.run.asked.len(),
            session.run.playtest.dirty,
        );
        let journal_changed = matches!(
            &envelope.event,
            Event::Commands(_) | Event::Job(_) | Event::AuthoredCards { .. }
        );
        // Fail closed even if an embedded policy panics and its caller catches
        // the unwind. TLS scope guards still restore the prior source context.
        session.poisoned = true;
        let reply = {
            let _settings = crate::settings::scoped(session.context.settings);
            let _book = phrasebook::scoped(session.parsed.clone());
            match session.dispatch(envelope.event.clone(), envelope.seq, policies) {
                Ok(reply) => reply,
                Err(error) => Reply::Refused(error),
            }
        };
        // A context event changes the scope used by its resulting publication.
        let _settings = crate::settings::scoped(session.context.settings);
        let _book = phrasebook::scoped(session.parsed.clone());
        session.validate_after(envelope.seq)?;
        if journal_changed {
            session.refresh_outbox()?;
        }
        if journal_changed
            || queued_before
                != (
                    session.run.brewing.len(),
                    session.run.asked.len(),
                    session.run.playtest.dirty,
                )
        {
            session.job_revision = envelope.seq;
        }
        Ok(reply)
    }
    /// Replay only into a staged owner. No view/dictionary/native output is
    /// emitted; caller must compare the retained semantic reply witness before
    /// admitting this candidate. Failure leaves this owner unusable.
    pub fn replay(
        &mut self,
        envelope: Envelope,
        policies: &mut impl Policies,
    ) -> Result<ReplayFrame, String> {
        self.healthy()?;
        if self.baseline_ready {
            return Err("active session cannot replay staged recovery input".into());
        }
        self.poisoned = true;
        let result = (|| {
            bounded(&envelope, MAX_EVENT_BYTES)?;
            self.preflight(&envelope)?;
            let reply = self.reduce(&envelope, policies)?;
            let frame = ReplayFrame {
                api: API,
                session: self.journal.session().into(),
                raid: self.run.raid_id,
                seq: envelope.seq,
                view_revision: self
                    .view_revision
                    .checked_add(1)
                    .ok_or("session view revision exhausted")?,
                reply,
            };
            bounded(&frame, MAX_FRAME_BYTES)?;
            Ok::<_, String>(frame)
        })();
        match result {
            Ok(frame) => {
                self.event_seq = frame.seq;
                self.view_revision = frame.view_revision;
                self.poisoned = false;
                // Deliberately do not invent a view Frame or last-exchange
                // replay cache when its old dictionary was never restored.
                Ok(frame)
            }
            Err(error) => Err(error),
        }
    }
    pub fn baseline(&mut self) -> Result<view::Frame, String> {
        let session = self;
        session.healthy()?;
        if session.baseline_ready {
            return Err("session baseline already published".into());
        }
        let _settings = crate::settings::scoped(session.context.settings);
        let _book = phrasebook::scoped(session.parsed.clone());
        session.poisoned = true;
        let result = (|| {
            // A resync baseline starts a fresh dictionary, not a new accepted
            // game revision. Initial revision zero alone advances to one.
            session.publisher = view::Publisher::new(
                session.journal.session().into(),
                session.run.raid_id,
                session.view_revision.saturating_sub(1),
                session.view_limits,
            )?;
            let status = session.status()?;
            session.publisher.baseline(&session.run, status)
        })();
        match result {
            Ok(frame) => {
                session.view_revision = frame.revision;
                session.baseline_ready = true;
                session.poisoned = false;
                Ok(frame)
            }
            Err(error) => {
                session.poisoned = true;
                Err(error)
            }
        }
    }
    pub fn apply(
        &mut self,
        envelope: Envelope,
        policies: &mut impl Policies,
    ) -> Result<Frame, String> {
        let session = self;
        session.healthy()?;
        bounded(&envelope, MAX_EVENT_BYTES)?;
        // Exact retransmission precedes revision checks: its expected revision
        // is necessarily older than the frame it already committed.
        if let Some((prior, frame)) = &session.last {
            if prior == &envelope {
                return Ok(frame.clone());
            }
        }
        if !session.baseline_ready {
            return Err("session needs explicit view baseline".into());
        }
        session.preflight(&envelope)?;
        let result = (|| {
            let reply = session.reduce(&envelope, policies)?;
            let _settings = crate::settings::scoped(session.context.settings);
            let _book = phrasebook::scoped(session.parsed.clone());
            let status = session.status()?;
            let view = session.publisher.delta(&session.run, status)?;
            let frame = Frame {
                api: API,
                session: session.journal.session().into(),
                raid: session.run.raid_id,
                seq: envelope.seq,
                view,
                reply,
            };
            bounded(&frame, MAX_FRAME_BYTES)?;
            Ok::<_, String>(frame)
        })();
        match result {
            Ok(frame) => {
                session.poisoned = false;
                session.event_seq = envelope.seq;
                session.view_revision = frame.view.revision;
                session.last = Some((envelope, frame.clone()));
                Ok(frame)
            }
            Err(error) => {
                session.poisoned = true;
                Err(error)
            }
        }
    }
}

fn pure_run(run: &Run) -> Result<(), String> {
    if run.module_visuals.revision != 0
        || !run.module_visuals.is_empty()
        || run.players.values().any(|hero| hero.avatar.is_some())
    {
        return Err("rules owner cannot contain native appearance overlays".into());
    }
    Ok(())
}

struct Bounded(Vec<u8>, usize);
impl Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) > self.1 {
            return Err(io::Error::other("session payload exceeds bound"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn bounded(value: &impl Serialize, limit: usize) -> Result<Vec<u8>, String> {
    let mut output = Bounded(Vec::new(), limit);
    serde_json::to_writer(&mut output, value).map_err(|e| e.to_string())?;
    Ok(output.0)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod host_ports_tests;
