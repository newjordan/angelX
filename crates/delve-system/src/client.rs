//! The host's read-only side of an owned Delve session. Working Run/Journal
//! state stays with the implementation; this adapter owns only accepted views,
//! sequencing and a speculative receiver transaction. It dispatches no effects.
use angel_delve::rules::session::{self, checkpoint, view};
use angel_live::system::owned::{Validator, semantic_witness};
use serde_json::Value;
use std::sync::Arc;

struct Staged {
    receiver: view::Receiver,
    seq: u64,
    reply: Option<session::Reply>,
}

pub struct Client {
    receiver: view::Receiver,
    seq: u64,
    staged: Option<Staged>,
    reply: Option<session::Reply>,
}

fn decode<T: serde::de::DeserializeOwned>(value: &Value) -> Result<T, String> {
    T::deserialize(value).map_err(|error| error.to_string())
}

fn projection(checkpoint: checkpoint::SessionCheckpoint) -> Result<view::Receiver, String> {
    let mut owner = session::Session::restore(checkpoint)?;
    let baseline = owner.baseline()?;
    let mut receiver = view::Receiver::new(
        baseline.session.clone(),
        baseline.raid,
        view::Limits::default(),
    )?;
    receiver.baseline(&baseline)?;
    Ok(receiver)
}

impl Client {
    /// Construct the initial immutable view at an explicit checkpoint boundary.
    /// No mutable game owner is retained here. Initialization consumes revision
    /// zero once; subsequent baselines preserve the carried revision.
    pub fn new(checkpoint: &checkpoint::SessionCheckpoint) -> Result<Self, String> {
        Ok(Self {
            receiver: projection(checkpoint.clone())?,
            seq: checkpoint.event_seq,
            staged: None,
            reply: None,
        })
    }

    pub fn view(&self) -> &Arc<view::HostView> {
        self.receiver
            .view()
            .expect("Client always has an accepted baseline")
    }
    pub fn event_seq(&self) -> u64 {
        self.seq
    }
    pub fn envelope(&self, event: session::Event) -> Result<session::Envelope, String> {
        Ok(session::Envelope {
            api: session::API,
            session: self.view().session().into(),
            raid: self.view().raid(),
            seq: self
                .seq
                .checked_add(1)
                .ok_or("session sequence exhausted")?,
            expected_view_revision: self.view().revision(),
            event,
        })
    }
    pub fn query(&self, query: session::Query) -> session::QueryEnvelope {
        session::QueryEnvelope {
            api: session::API,
            session: self.view().session().into(),
            raid: self.view().raid(),
            expected_view_revision: Some(self.view().revision()),
            query,
        }
    }
    pub fn read_query(
        &self,
        query: &session::QueryEnvelope,
        value: &Value,
    ) -> Result<session::QueryReply, String> {
        let frame: session::QueryFrame = decode(value)?;
        if query.api != session::API
            || frame.api != session::API
            || query.session != self.view().session()
            || frame.session != query.session
            || query.raid != self.view().raid()
            || frame.raid != query.raid
            || query.expected_view_revision != Some(self.view().revision())
            || frame.event_seq != self.seq
            || frame.view_revision != self.view().revision()
        {
            return Err("foreign or stale owned query response".into());
        }
        match (&query.query, &frame.reply) {
            (
                session::Query::Cursor,
                session::QueryReply::Cursor {
                    next_request,
                    event_seq,
                    view_revision,
                },
            ) if *next_request > 0
                && *event_seq == self.seq
                && *view_revision == self.view().revision() => {}
            (session::Query::Job(request), session::QueryReply::Job(reply))
                if valid_job_reply(self.view().session(), self.view().raid(), request, reply) => {}
            (session::Query::BrewReady, session::QueryReply::BrewReady(_))
            | (session::Query::HomeReady { .. }, session::QueryReply::HomeReady(_))
            | (session::Query::VotesReady, session::QueryReply::VotesReady(_)) => {}
            (session::Query::QueuedWishes { limit }, session::QueryReply::QueuedWishes(asks))
                if (1..=64).contains(limit)
                    && asks.len() <= usize::from(*limit)
                    && asks.iter().all(angel_delve::rules::wish_jobs::valid_ask) => {}
            (session::Query::FramePhase, session::QueryReply::FramePhase(None)) => {}
            (session::Query::FramePhase, session::QueryReply::FramePhase(Some(phase)))
                if phase.begun.frame.begin_seq > 0
                    && phase.begun.frame.begin_seq <= self.seq
                    && phase.begun.frame.tick == self.view().run().tick
                    && phase.begun.frame.seats == session::frame::seats(self.view().run()) => {}
            (session::Query::Abilities, session::QueryReply::Abilities(claims))
                if claims.fence
                    == session::abilities::OwnerFence::capture(
                        self.view().run(),
                        self.view().session(),
                    )
                    && angel_delve::rules::abilities::validate_outbox(&claims.requests).is_ok() => {
            }
            _ => return Err("owned query response kind or ownership differs".into()),
        }
        Ok(frame.reply)
    }
    /// Only a newly accepted live event supplies a reply. Recovery and image
    /// handoff never republish prior effects through this slot.
    pub fn take_reply(&mut self) -> Option<session::Reply> {
        self.reply.take()
    }

    fn identity(&self, checkpoint: &checkpoint::SessionCheckpoint) -> Result<(), String> {
        if checkpoint.session != self.view().session() || checkpoint.raid != self.view().raid() {
            return Err("checkpoint belongs to another owned session".into());
        }
        Ok(())
    }
}

fn valid_event_reply(
    event: &session::Event,
    reply: &session::Reply,
    session_id: &str,
    raid: u64,
    seq: u64,
    run: &angel_delve::rules::Run,
) -> bool {
    use session::{Event as E, Reply as R};
    if matches!(reply, R::Refused(_)) {
        return true;
    }
    match (event, reply) {
        (E::Tick { .. }, R::Done)
        | (E::ContextChanged(_), R::Changed(_))
        | (E::ControlsChanged(_), R::Changed(_))
        | (E::CampaignChanged { .. }, R::Changed(_))
        | (E::AuthoredCards { .. }, R::Changed(_)) => true,
        (E::BeginFrame(input), R::FrameBegun(begun)) => {
            begun.frame.begin_seq == seq
                && begun.frame.network_tick == input.network_tick
                && begun.frame.tick <= session::MAX_TICK
                && begun.frame.active == input.controls.active
                && begun.frame.seats.len() <= angel_delve::rules::MAX_PLAYERS as usize
                && begun
                    .frame
                    .seats
                    .keys()
                    .all(|id| (1..=angel_delve::rules::MAX_PLAYERS).contains(id))
        }
        (E::FinishFrame(input), R::FrameFinished(output)) => {
            output.frame.begin_seq == input.begin_seq
                && output.frame.network_tick == input.network_tick
                && output.frame.tick == input.expected_tick
                && output.frame.tick == run.tick
                && output.frame.seats == session::frame::seats(run)
        }
        (E::Commands(request), R::Commands(receipt)) => {
            receipt.id == request.id
                && match &receipt.result {
                    Err(_) => true,
                    Ok(replies) => {
                        replies.len() == request.commands.len()
                            && request
                                .commands
                                .iter()
                                .zip(replies)
                                .all(|(command, reply)| {
                                    valid_command_reply(
                                        command, reply, session_id, raid, request.id,
                                    )
                                })
                    }
                }
        }
        (E::Job(event), R::Job(reply)) => valid_job_event_reply(event, reply, session_id, raid),
        (E::Ability(event), R::Ability(reply)) => match (event, reply) {
            (
                session::abilities::Event::ReplaceSlots { .. },
                session::abilities::Reply::Changed(_),
            ) => true,
            (
                session::abilities::Event::Acknowledge { requests, .. },
                session::abilities::Reply::Acknowledged(n),
            )
            | (
                session::abilities::Event::Cancel { requests, .. },
                session::abilities::Reply::Acknowledged(n),
            ) => *n == requests.len(),
            (
                session::abilities::Event::Batch(batch),
                session::abilities::Reply::Applied {
                    starts,
                    commands,
                    acknowledged,
                },
            ) => {
                *starts == batch.starts.len()
                    && *commands == batch.actions.len()
                    && *acknowledged == batch.acknowledge.len()
            }
            _ => false,
        },
        _ => false,
    }
}

fn valid_command_reply(
    command: &angel_delve::rules::host_commands::Command,
    reply: &angel_delve::rules::host_commands::Reply,
    session_id: &str,
    raid: u64,
    request: u64,
) -> bool {
    use angel_delve::rules::host_commands::{Command as C, Reply as R};
    match command {
        C::ClaimBrew => {
            matches!(reply, R::Brew(work) if work.valid() && work.ask.raid == raid && work.result_id == angel_delve::rules::brew_jobs::Work::result_id(session_id, raid, request))
        }
        C::ClaimReforge {
            player,
            part,
            words,
        } => {
            matches!(reply, R::Reforge(work) if work.valid() && work.player == *player && work.part == *part && work.words == *words && work.result_id == angel_delve::rules::reforge_jobs::Work::result_id(session_id, raid, request))
        }
        C::ClaimWish { ask, maker } => {
            matches!(reply, R::Wish(work) if work.valid() && work.ask == *ask && work.maker == *maker && work.binding == angel_delve::rules::wish_jobs::Work::binding(session_id, raid, request))
        }
        C::ClaimHome => matches!(reply, R::Home(transfer) if transfer.valid()),
        C::ClaimRealm => matches!(reply, R::Realm(_)),
        C::Scroll { .. } | C::Reforge { .. } | C::ChooseOffer { .. }
        | C::WishOnPiece { .. } | C::BuildVessel { .. } | C::Contribute { .. } => {
            matches!(reply, R::Chosen(_))
        }
        C::Catalogue { .. }
        | C::Speculative { .. }
        | C::PaintCards { .. }
        | C::SyncHome { .. }
        | C::JoinSeat { .. }
        | C::AddCard { .. }
        | C::DropOffer { .. }
        | C::TurnOffer { .. }
        | C::OpenWishLine { .. }
        | C::TypeWishLetter { .. }
        | C::MirrorName { .. }
        | C::RevealVessel
        | C::DropMirrorLook { .. }
        | C::MirrorPaint { .. } | C::MirrorBody { .. } => matches!(reply, R::Changed(_)),
        C::Visit { .. } => matches!(reply, R::Visited(_)),
        C::EndHosting { .. } | C::RemoveSeat { .. } => matches!(reply, R::Names(_)),
        C::Retreat
        | C::LearnWish { .. }
        | C::KnownWish { .. }
        | C::DropQueuedBrew { .. }
        | C::TakeEntrance { .. }
        | C::AdmitSettlement { .. }
        | C::EnterSettlement { .. }
        | C::BeginDelve { .. }
        | C::ConfigureSession { .. }
        | C::EquipForged { .. }
        | C::MoveCard { .. }
        | C::CarryBlock { .. }
        | C::LeaveActivity { .. }
        | C::Vote { .. }
        | C::DismissFloorPlate { .. }
        | C::ClaimVotes { .. }
        | C::PrepareVotes { .. } => matches!(reply, R::Done),
    }
}

fn valid_job_event_reply(
    event: &session::jobs::Event,
    reply: &session::jobs::Reply,
    session_id: &str,
    raid: u64,
) -> bool {
    use session::jobs::{Event as E, Reply as R};
    let scope = |session: &str, claim_raid: u64, request: u64| {
        session == session_id && claim_raid == raid && request > 0
    };
    match (event, reply) {
        (
            E::Acknowledge {
                raid: ack_raid,
                request,
            },
            R::Done,
        ) => *ack_raid == raid && *request > 0,
        (E::AcknowledgeRealm(claim), R::Done) => scope(&claim.session, claim.raid, claim.request),
        (E::AcknowledgeHome { claim, completed }, R::Done) => {
            scope(&claim.session, claim.raid, claim.request)
                && claim.transfer.valid()
                && completed.result.valid_for(&claim.transfer)
                && completed
                    .delivered
                    .valid_for(&claim.transfer, &completed.result)
        }
        (E::AcknowledgeBrew { claim, completed }, R::Done) => {
            valid_brew_claim(session_id, raid, claim) && completed.valid(&claim.work)
        }
        (E::AcknowledgeReforge { claim, completed }, R::Done) => {
            valid_reforge_claim(session_id, raid, claim) && completed.valid(&claim.work)
        }
        (E::AcknowledgeWish { claim, completed }, R::Done) => {
            valid_wish_claim(session_id, raid, claim) && completed.valid(&claim.work)
        }
        (E::PrepareVotes { .. }, R::Done) => true,
        (E::AcknowledgeVotes(batch), R::Done) => scope(&batch.session, batch.raid, batch.request),
        (E::ClaimRealm, R::Realm(claim)) => claim
            .as_ref()
            .is_none_or(|c| scope(&c.session, c.raid, c.request)),
        (E::ClaimHome { .. }, R::Home(claim)) => claim
            .as_ref()
            .is_none_or(|c| scope(&c.session, c.raid, c.request) && c.transfer.valid()),
        (E::EvaluateHome(request), R::HomeResult(result)) => {
            request.validate_result(result).is_ok()
        }
        (E::CompleteHome { claim, result }, R::HomeDelivered(delivered)) => {
            scope(&claim.session, claim.raid, claim.request)
                && claim.transfer.valid()
                && delivered.valid_for(&claim.transfer, result)
        }
        (E::ClaimBrew, R::Brew(claim)) => claim
            .as_ref()
            .is_none_or(|c| valid_brew_claim(session_id, raid, c)),
        (E::CompleteBrew { claim, input }, R::BrewCompleted(Some(done))) => {
            valid_brew_claim(session_id, raid, claim)
                && done.input.as_ref() == Some(input)
                && done.valid(&claim.work)
        }
        (E::CancelBrew { claim, .. }, R::BrewCompleted(Some(done))) => {
            // A terminal claim returns its original completion on cancellation.
            valid_brew_claim(session_id, raid, claim) && done.valid(&claim.work)
        }
        (
            E::ClaimReforge {
                player,
                part,
                words,
            },
            R::Reforge(claim),
        ) => {
            valid_reforge_claim(session_id, raid, claim)
                && claim.work.player == *player
                && claim.work.part == *part
                && claim.work.words == *words
        }
        (E::CompleteReforge { claim, input }, R::ReforgeCompleted(Some(done))) => {
            valid_reforge_claim(session_id, raid, claim)
                && done.input.as_ref() == Some(input)
                && done.valid(&claim.work)
        }
        (E::CancelReforge { claim, .. }, R::ReforgeCompleted(Some(done))) => {
            valid_reforge_claim(session_id, raid, claim) && done.valid(&claim.work)
        }
        (E::ClaimWish { ask, maker }, R::Wish(claim)) => {
            valid_wish_claim(session_id, raid, claim)
                && claim.work.ask == *ask
                && claim.work.maker == *maker
        }
        (
            E::CompleteWish {
                claim,
                input,
                authored,
                reason,
            },
            R::WishCompleted(Some(done)),
        ) => {
            valid_wish_claim(session_id, raid, claim)
                && done.input == *input
                && done.authored == *authored
                && (input.is_some() || done.reason == *reason)
                && done.valid(&claim.work)
        }
        (E::ClaimVotes, R::Votes(batch)) => batch
            .as_ref()
            .is_none_or(|b| scope(&b.session, b.raid, b.request) && b.changes.len() <= 4096),
        (E::CompleteBrew { .. } | E::CompleteReforge { .. }, R::Rejected(errors)) => {
            !errors.is_empty()
                && errors.len() <= 64
                && errors.iter().all(|error| error.len() <= 8192)
        }
        _ => false,
    }
}

fn valid_job_reply(
    session_id: &str,
    raid: u64,
    query: &session::jobs::Query,
    reply: &session::jobs::Reply,
) -> bool {
    use session::jobs::{Query as Q, Reply as R};

    let scoped = |claim_session: &str, claim_raid: u64, request: u64| {
        claim_session == session_id && claim_raid == raid && request > 0
    };
    match (query, reply) {
        (Q::PendingRealm, R::Realm(claim)) => claim
            .as_ref()
            .is_none_or(|claim| scoped(&claim.session, claim.raid, claim.request)),
        (Q::PendingHome, R::Home(claim)) => claim.as_ref().is_none_or(|claim| {
            scoped(&claim.session, claim.raid, claim.request) && claim.transfer.valid()
        }),
        (Q::HomeCompletion(claim), R::HomeCompleted(completion))
            if scoped(&claim.session, claim.raid, claim.request) && claim.transfer.valid() =>
        {
            completion.as_ref().is_none_or(|completion| {
                completion.result.valid_for(&claim.transfer)
                    && completion
                        .delivered
                        .valid_for(&claim.transfer, &completion.result)
            })
        }
        (Q::PendingBrews, R::Brews(claims)) => {
            claims.len() <= 64
                && unique_claims(
                    claims,
                    |claim| valid_brew_claim(session_id, raid, claim),
                    |claim| claim.request,
                )
        }
        (Q::BrewCompletion(claim), R::BrewCompleted(completion))
            if valid_brew_claim(session_id, raid, claim) =>
        {
            completion
                .as_ref()
                .is_none_or(|completion| completion.valid(&claim.work))
        }
        (Q::PendingReforges, R::Reforges(claims)) => {
            claims.len() <= 64
                && unique_claims(
                    claims,
                    |claim| valid_reforge_claim(session_id, raid, claim),
                    |claim| claim.request,
                )
        }
        (Q::ReforgeCompletion(claim), R::ReforgeCompleted(completion))
            if valid_reforge_claim(session_id, raid, claim) =>
        {
            completion
                .as_ref()
                .is_none_or(|completion| completion.valid(&claim.work))
        }
        (Q::PendingWishes, R::Wishes(claims)) => {
            claims.len() <= 64
                && unique_claims(
                    claims,
                    |claim| valid_wish_claim(session_id, raid, claim),
                    |claim| claim.request,
                )
        }
        (Q::WishCompletion(claim), R::WishCompleted(completion))
            if valid_wish_claim(session_id, raid, claim) =>
        {
            completion
                .as_ref()
                .is_none_or(|completion| completion.valid(&claim.work))
        }
        (Q::PendingVotes, R::Votes(batch)) => batch
            .as_ref()
            .is_none_or(|batch| scoped(&batch.session, batch.raid, batch.request)),
        _ => false,
    }
}

fn valid_brew_claim(
    session_id: &str,
    raid: u64,
    claim: &angel_delve::rules::brew_jobs::Claim,
) -> bool {
    claim.session == session_id
        && claim.raid == raid
        && claim.request > 0
        && claim.work.valid()
        && claim.work.ask.raid == raid
        && claim.work.result_id
            == angel_delve::rules::brew_jobs::Work::result_id(session_id, raid, claim.request)
}

fn valid_reforge_claim(
    session_id: &str,
    raid: u64,
    claim: &angel_delve::rules::reforge_jobs::Claim,
) -> bool {
    claim.session == session_id
        && claim.raid == raid
        && claim.request > 0
        && claim.work.valid()
        && claim.work.result_id
            == angel_delve::rules::reforge_jobs::Work::result_id(session_id, raid, claim.request)
}

fn valid_wish_claim(
    session_id: &str,
    raid: u64,
    claim: &angel_delve::rules::wish_jobs::Claim,
) -> bool {
    claim.session == session_id
        && claim.raid == raid
        && claim.request > 0
        && claim.work.valid()
        && claim.work.binding
            == angel_delve::rules::wish_jobs::Work::binding(session_id, raid, claim.request)
}

fn unique_claims<T>(claims: &[T], valid: impl Fn(&T) -> bool, request: impl Fn(&T) -> u64) -> bool {
    let mut ids = std::collections::BTreeSet::new();
    claims
        .iter()
        .all(|claim| valid(claim) && ids.insert(request(claim)))
}

impl Validator for Client {
    fn validate_checkpoint(&mut self, value: &Value) -> Result<(), String> {
        let checkpoint: checkpoint::SessionCheckpoint = decode(value)?;
        self.identity(&checkpoint)?;
        if serde_json::to_value(&checkpoint).map_err(|e| e.to_string())? != *value {
            return Err("owned checkpoint is incomplete or noncanonical".into());
        }
        // Full validation is restricted to checkpoint boundaries, never events.
        checkpoint::restore(checkpoint)?;
        Ok(())
    }
    fn validate_handoff(&mut self, prior: &Value, next: &Value) -> Result<(), String> {
        if prior != next {
            return Err("owned handoff changed the canonical checkpoint".into());
        }
        let prior: checkpoint::SessionCheckpoint = decode(prior)?;
        let next: checkpoint::SessionCheckpoint = decode(next)?;
        self.identity(&prior)?;
        self.identity(&next)?;
        checkpoint::validate_handoff(&prior, &next)
    }
    fn validate_bootstrap(&mut self, initial: &Value, post: &Value) -> Result<(), String> {
        let mut canonical = initial.clone();
        let revision = canonical
            .get("view_revision")
            .and_then(Value::as_u64)
            .ok_or("bootstrap checkpoint has no revision")?
            .max(1);
        canonical["view_revision"] = revision.into();
        if canonical != *post {
            return Err(
                "owned bootstrap changed state beyond the initial baseline revision".into(),
            );
        }
        let mut initial: checkpoint::SessionCheckpoint = decode(initial)?;
        let post: checkpoint::SessionCheckpoint = decode(post)?;
        self.identity(&initial)?;
        self.identity(&post)?;
        initial.view_revision = initial.view_revision.max(1);
        checkpoint::validate_handoff(&initial, &post)?;
        self.validate_recovered(&serde_json::to_value(post).map_err(|e| e.to_string())?)
    }
    fn stage_baseline(&mut self, value: &Value) -> Result<(), String> {
        let frame: view::Frame = decode(value)?;
        let mut receiver = view::Receiver::new(
            self.view().session().into(),
            self.view().raid(),
            view::Limits::default(),
        )?;
        receiver.baseline(&frame)?;
        if !self.receiver.same_public_state(&receiver) {
            return Err("owned baseline changed accepted public state".into());
        }
        self.staged = Some(Staged {
            receiver,
            seq: self.seq,
            reply: None,
        });
        Ok(())
    }
    fn stage_event(&mut self, input: &Value, output: &Value) -> Result<Vec<u8>, String> {
        let event: session::Envelope = decode(input)?;
        let frame: session::Frame = decode(output)?;
        if event.api != session::API
            || frame.api != session::API
            || event.session != self.view().session()
            || frame.session != event.session
            || event.raid != self.view().raid()
            || frame.raid != event.raid
            || self.seq.checked_add(1) != Some(event.seq)
            || frame.seq != event.seq
            || event.expected_view_revision != self.view().revision()
            || self.view().revision().checked_add(1) != Some(frame.view.revision)
        {
            return Err("foreign or stale owned event response".into());
        }
        if !valid_event_reply(
            &event.event,
            &frame.reply,
            &event.session,
            event.raid,
            event.seq,
            self.view().run(),
        ) {
            return Err("owned event response kind or receipt ownership differs".into());
        }
        let mut receiver = self.receiver.clone();
        receiver.delta(&frame.view)?;
        let witness = semantic_witness(&session::ReplayFrame {
            api: frame.api,
            session: frame.session,
            raid: frame.raid,
            seq: frame.seq,
            view_revision: frame.view.revision,
            reply: frame.reply.clone(),
        })?;
        self.staged = Some(Staged {
            receiver,
            seq: frame.seq,
            reply: Some(frame.reply),
        });
        Ok(witness)
    }
    fn validate_replay(
        &mut self,
        input: &Value,
        output: &Value,
        witness: &[u8],
    ) -> Result<(), String> {
        let event: session::Envelope = decode(input)?;
        let frame: session::ReplayFrame = decode(output)?;
        if event.api != session::API
            || frame.api != session::API
            || event.session != self.view().session()
            || frame.session != event.session
            || event.raid != self.view().raid()
            || frame.raid != event.raid
            || frame.seq != event.seq
            || frame.seq > self.seq
            || event.expected_view_revision.checked_add(1) != Some(frame.view_revision)
            || frame.view_revision > self.view().revision()
            || semantic_witness(&frame)? != witness
        {
            return Err("owned replay diverged from accepted semantics".into());
        }
        Ok(())
    }
    fn validate_recovered(&mut self, value: &Value) -> Result<(), String> {
        let checkpoint: checkpoint::SessionCheckpoint = decode(value)?;
        self.identity(&checkpoint)?;
        if checkpoint.event_seq != self.seq || checkpoint.view_revision != self.view().revision() {
            return Err("owned checkpoint differs from accepted cursor".into());
        }
        let recovered = projection(checkpoint)?;
        if !self.receiver.same_public_state(&recovered) {
            return Err("owned checkpoint differs from accepted public state".into());
        }
        Ok(())
    }
    fn commit(&mut self) {
        if let Some(staged) = self.staged.take() {
            self.receiver = staged.receiver;
            self.seq = staged.seq;
            self.reply = staged.reply;
        }
    }
    fn abort(&mut self) {
        self.staged = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angel_delve::rules::{Run, host_commands::Journal};
    use std::collections::BTreeMap;

    #[test]
    fn salvage_commands_validate_their_reply_kinds() {
        use angel_delve::rules::host_commands::{Command, Reply};
        let commands = [
            Command::WishOnPiece { player: 1, words: "wish".into() },
            Command::BuildVessel {
                n: 1, ids: vec![1], design: Default::default(), by_model: false,
            },
        ];
        for command in &commands {
            assert!(valid_command_reply(command, &Reply::Chosen("ok".into()), "s", 9, 1));
            assert!(!valid_command_reply(command, &Reply::Changed(true), "s", 9, 1));
        }
        assert!(valid_command_reply(&Command::RevealVessel, &Reply::Changed(true), "s", 9, 1));
        assert!(!valid_command_reply(&Command::RevealVessel, &Reply::Done, "s", 9, 1));
    }

    fn pair() -> (session::Session, Client) {
        let run = Run::new(13, 9, None);
        let mut owner =
            session::Session::new(run, Journal::new("client".into(), 9), Default::default())
                .unwrap();
        owner.baseline().unwrap();
        let client = Client::new(&owner.checkpoint().unwrap()).unwrap();
        (owner, client)
    }
    #[test]
    fn accepted_view_stages_atomically_and_replay_never_dispatches_old_replies() {
        let (mut owner, mut client) = pair();
        let accepted = Arc::clone(client.view());
        let event = client
            .envelope(session::Event::Tick {
                inputs: BTreeMap::new(),
                step: true,
            })
            .unwrap();
        let input = serde_json::to_value(&event).unwrap();
        let frame = owner
            .apply(event.clone(), &mut session::DefaultPolicies)
            .unwrap();
        let mut bad = frame.clone();
        bad.view.bytes.push(255);
        assert!(
            client
                .stage_event(&input, &serde_json::to_value(bad).unwrap())
                .is_err()
        );
        client.abort();
        assert!(Arc::ptr_eq(client.view(), &accepted));
        assert_eq!(client.event_seq(), 0);
        let witness = client
            .stage_event(&input, &serde_json::to_value(&frame).unwrap())
            .unwrap();
        assert_eq!(witness.len(), 32);
        assert!(Arc::ptr_eq(client.view(), &accepted));
        client.commit();
        assert_eq!(client.event_seq(), 1);
        assert!(client.take_reply().is_some());
        let replay = session::ReplayFrame {
            api: frame.api,
            session: frame.session,
            raid: frame.raid,
            seq: frame.seq,
            view_revision: frame.view.revision,
            reply: frame.reply,
        };
        client
            .validate_replay(&input, &serde_json::to_value(replay).unwrap(), &witness)
            .unwrap();
        client
            .validate_recovered(&serde_json::to_value(owner.checkpoint().unwrap()).unwrap())
            .unwrap();
        assert!(client.take_reply().is_none());
    }
    #[test]
    fn query_cursor_and_bootstrap_use_the_accepted_owner_identity() {
        let (owner, client) = pair();
        let query = client.query(session::Query::Cursor);
        let frame = owner.query(query.clone()).unwrap();
        let value = serde_json::to_value(&frame).unwrap();
        assert!(matches!(
            client.read_query(&query, &value).unwrap(),
            session::QueryReply::Cursor { .. }
        ));
        let mut foreign = frame;
        foreign.event_seq += 1;
        assert!(
            client
                .read_query(&query, &serde_json::to_value(foreign).unwrap())
                .is_err()
        );
    }

    #[test]
    fn unknown_nested_context_cannot_silently_disappear_during_handoff() {
        let (owner, mut client) = pair();
        let canonical = serde_json::to_value(owner.checkpoint().unwrap()).unwrap();
        let mut foreign = canonical.clone();
        foreign["context"]["settings"]["unknown_setting"] = true.into();
        assert!(client.validate_checkpoint(&foreign).is_err());
        assert!(client.validate_handoff(&foreign, &canonical).is_err());
        assert!(client.validate_bootstrap(&foreign, &canonical).is_err());
    }
    fn pending_brew_pair() -> (
        session::Session,
        Client,
        angel_delve::rules::brew_jobs::Claim,
    ) {
        use angel_delve::rules::cauldron;

        let mut run = Run::new(13, 9, None);
        let placeholder = cauldron::brewed_card(
            cauldron::recipe(cauldron::Essence::Garden, cauldron::Essence::Garden),
            cauldron::Tier::Common,
        );
        run.players.get_mut(&1).unwrap().spells[0] = Some(placeholder.id.clone());
        run.brewing.push(cauldron::Brewing {
            knight: 1,
            raid: run.raid_id,
            depth: run.dungeon.depth,
            cards: ["cabbage-storm".into(), "hedge-maze".into()],
            placeholder: placeholder.id.clone(),
        });
        run.book.insert(placeholder);
        let mut journal = Journal::new("client".into(), run.raid_id);
        let claim = journal.claim_brew(&mut run).unwrap().unwrap();
        assert!(claim.work.valid());
        let mut owner = session::Session::new(run, journal, Default::default()).unwrap();
        owner.baseline().unwrap();
        let client = Client::new(&owner.checkpoint().unwrap()).unwrap();
        (owner, client, claim)
    }

    fn query_value(client: &Client, reply: session::QueryReply) -> Value {
        serde_json::to_value(session::QueryFrame {
            api: session::API,
            session: client.view().session().into(),
            raid: client.view().raid(),
            event_seq: client.event_seq(),
            view_revision: client.view().revision(),
            reply,
        })
        .unwrap()
    }

    #[test]
    fn brew_ready_query_accepts_exact_kind_and_rejects_unrelated_boolean_reply() {
        let (owner, client) = pair();
        let query = client.query(session::Query::BrewReady);
        let actual = serde_json::to_value(owner.query(query.clone()).unwrap()).unwrap();
        assert_eq!(
            client.read_query(&query, &actual).unwrap(),
            session::QueryReply::BrewReady(false)
        );
        let wrong = query_value(&client, session::QueryReply::VotesReady(false));
        assert!(client.read_query(&query, &wrong).is_err());
    }

    #[test]
    fn job_queries_reject_wrong_reply_kinds_and_foreign_pending_claims() {
        use angel_delve::rules::session::jobs;

        let (owner, client, claim) = pending_brew_pair();
        let query = client.query(session::Query::Job(jobs::Query::PendingBrews));
        let wrong_kind = query_value(
            &client,
            session::QueryReply::Job(jobs::Reply::Wishes(Vec::new())),
        );
        assert!(client.read_query(&query, &wrong_kind).is_err());

        let mut foreign = claim.clone();
        foreign.session = "another-session".into();
        let foreign_claim = query_value(
            &client,
            session::QueryReply::Job(jobs::Reply::Brews(vec![foreign])),
        );
        assert!(client.read_query(&query, &foreign_claim).is_err());

        let actual = owner.query(query.clone()).unwrap();
        let actual = serde_json::to_value(actual).unwrap();
        assert!(matches!(
            client.read_query(&query, &actual).unwrap(),
            session::QueryReply::Job(jobs::Reply::Brews(claims)) if claims == vec![claim.clone()]
        ));
    }

    #[test]
    fn brew_claim_rejects_foreign_raid_inside_captured_work() {
        use angel_delve::rules::{host_commands, session::jobs};
        let (_, client, claim) = pending_brew_pair();
        let mut foreign = claim.clone();
        foreign.work.ask.raid += 1;
        // Work validation alone checks that an ask is internally consistent;
        // the Client must additionally fence it to the owner that returned it.
        assert!(foreign.work.valid());
        let query = client.query(session::Query::Job(jobs::Query::PendingBrews));
        let response = query_value(&client, session::QueryReply::Job(jobs::Reply::Brews(vec![foreign.clone()])));
        assert!(client.read_query(&query, &response).is_err());

        let event = session::Event::Commands(host_commands::Request {
            session: claim.session.clone(), raid: claim.raid, id: claim.request,
            commands: vec![host_commands::Command::ClaimBrew],
        });
        let response = session::Reply::Commands(host_commands::Receipt {
            id: claim.request, result: Ok(vec![host_commands::Reply::Brew(foreign.work)]),
        });
        assert!(!valid_event_reply(&event, &response, &claim.session, claim.raid, 1, client.view().run()));
    }

    #[test]
    fn completion_queries_validate_against_the_requested_claim_and_keep_none_valid() {
        use angel_delve::rules::{brew_jobs, session::jobs};

        let (owner, client, claim) = pending_brew_pair();
        let query = client.query(session::Query::Job(jobs::Query::BrewCompletion(
            claim.clone(),
        )));
        let actual = owner.query(query.clone()).unwrap();
        let actual = serde_json::to_value(actual).unwrap();
        assert!(matches!(
            client.read_query(&query, &actual).unwrap(),
            session::QueryReply::Job(jobs::Reply::BrewCompleted(None))
        ));

        let malformed = brew_jobs::Completion {
            input: Some(claim.work.inputs[0].clone()),
            accepted: None,
            result: Ok("not a valid completion".into()),
        };
        let malformed = query_value(
            &client,
            session::QueryReply::Job(jobs::Reply::BrewCompleted(Some(malformed))),
        );
        assert!(client.read_query(&query, &malformed).is_err());
    }

    #[test]
    fn cancel_after_terminal_brew_keeps_the_original_completion_and_client_continues() {
        use angel_delve::rules::{brew_jobs, cards, session::jobs};
        struct Accept;
        impl session::Policies for Accept {
            fn brew(
                &mut self,
                request: &brew_jobs::PolicyRequest,
            ) -> Result<brew_jobs::PolicyResult, String> {
                let mut card = request.card.clone();
                card.id = request.work.result_id.clone();
                Ok(Ok(card))
            }
        }
        let (mut owner, mut client, claim) = pending_brew_pair();
        let input = cards::check(
            "answer",
            "name Retained Brew\nkind spell\ncooldown 4\ncast bolt\n",
        )
        .unwrap()
        .card;
        let event = client
            .envelope(session::Event::Job(jobs::Event::CompleteBrew {
                claim: claim.clone(),
                input,
            }))
            .unwrap();
        let raw = serde_json::to_value(&event).unwrap();
        let frame = owner.apply(event, &mut Accept).unwrap();
        assert!(
            matches!(&frame.reply, session::Reply::Job(jobs::Reply::BrewCompleted(Some(done))) if done.input.is_some())
        );
        client
            .stage_event(&raw, &serde_json::to_value(&frame).unwrap())
            .unwrap();
        client.commit();
        let completed = client.take_reply().unwrap();
        let event = client
            .envelope(session::Event::Job(jobs::Event::CancelBrew {
                claim,
                reason: "late cancel".into(),
            }))
            .unwrap();
        let raw = serde_json::to_value(&event).unwrap();
        let retried = owner.apply(event, &mut Accept).unwrap();
        assert_eq!(retried.reply, completed);
        client
            .stage_event(&raw, &serde_json::to_value(&retried).unwrap())
            .unwrap();
        client.commit();
        assert_eq!(client.take_reply(), Some(completed));
        assert_eq!(client.event_seq(), 2);
        client
            .validate_recovered(&serde_json::to_value(owner.checkpoint().unwrap()).unwrap())
            .unwrap();
    }

    #[test]
    fn accepted_event_reply_checks_kind_receipt_id_and_job_session_ownership() {
        use angel_delve::rules::{host_commands, session::jobs};

        let (mut owner, mut client) = pair();
        let tick = session::Event::Tick {
            inputs: BTreeMap::new(),
            step: false,
        };
        assert!(valid_event_reply(
            &tick,
            &session::Reply::Done,
            "client",
            9,
            1,
            client.view().run()
        ));
        let event = client.envelope(tick.clone()).unwrap();
        let input = serde_json::to_value(&event).unwrap();
        let frame = owner.apply(event, &mut session::DefaultPolicies).unwrap();
        let mut wrong_kind = frame;
        wrong_kind.reply = session::Reply::Changed(false);
        assert!(
            client
                .stage_event(&input, &serde_json::to_value(wrong_kind).unwrap())
                .is_err()
        );
        assert!(!valid_event_reply(
            &tick,
            &session::Reply::Changed(false),
            "client",
            9,
            1,
            client.view().run()
        ));

        let request = host_commands::Request {
            session: "client".into(),
            raid: 9,
            id: 17,
            commands: vec![host_commands::Command::Retreat],
        };
        let event = session::Event::Commands(request);
        let wrong_receipt = session::Reply::Commands(host_commands::Receipt {
            id: 18,
            result: Ok(vec![host_commands::Reply::Done]),
        });
        assert!(!valid_event_reply(
            &event,
            &wrong_receipt,
            "client",
            9,
            1,
            client.view().run()
        ));

        let (_, _, mut claim) = pending_brew_pair();
        let completed = angel_delve::rules::brew_jobs::Completion {
            input: None,
            accepted: None,
            result: Err("cancelled".into()),
        };
        claim.session = "foreign-session".into();
        let event = session::Event::Job(jobs::Event::AcknowledgeBrew { claim, completed });
        assert!(!valid_event_reply(
            &event,
            &session::Reply::Job(jobs::Reply::Done),
            "client",
            9,
            1,
            client.view().run(),
        ));
    }
}
