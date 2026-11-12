//! Identified host intentions for the game implementation. No caller receives
//! mutable world access through this port. Receipts travel with the game session;
//! an uncertain completion is queried/retried by identity, never re-executed.
use super::{Run, votes, RoomKind, cards, knights, world};
use crate::realm::{Wish, forge};
use super::home_settlement::{
    HomeClaim, HomeTransfer, HomeResult, HomeDelivered, HomeCompletion, HomeObserved, HomeContext,
};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

mod brewing;
pub mod catalogue;
pub mod playtest;
pub mod settlement;
mod reforging;
mod wishing;

const KEPT_RECEIPTS: usize = 64;
const MAX_REQUEST_BYTES: usize = 1024 * 1024;
const MAX_RESULT_BYTES: usize = 512 * 1024;
const MAX_PENDING_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Command {
    Scroll { player: u32, intent: super::scrolls::Intent },
    Speculative { event: super::speculative::Event },
    Catalogue { batch: catalogue::Batch },
    PaintCards { updates: Vec<super::card_paint::Update> },
    ClaimWish { ask: super::genie::Asked, maker: String },
    /// The tavern mirror (`creator`, `cosmetic_wish`): a name typed at the
    /// glass, a look taken off the queue for the worn painter, and the look
    /// it drew. None of them spends or grants a wish.
    MirrorName { player: u32, words: String },
    /// Words said while a found piece of old dwarf-work waits for its wish
    /// go to the piece (`salvage`), free; the reply carries the piece's line.
    WishOnPiece { player: u32, words: String },
    /// The shipwrights' design for build `n` of the salvage pieces `ids`
    /// has landed (`together_salvage`): the vessel is made in the yard.
    BuildVessel { n: u32, ids: Vec<u32>, design: super::salvage::Design, by_model: bool },
    /// A vessel built is shown to the party at home (`salvage`).
    RevealVessel,
    DropMirrorLook { player: u32, words: String },
    MirrorPaint { card: cards::Card },
    /// The body-maker has drawn the whole knight for a mirror look (`card`
    /// the look's card id, `words` the words it was drawn from), or `on`
    /// false: its set is gone. Spends nothing.
    MirrorBody { card: String, words: String, on: bool },
    ClaimRealm,
    ClaimHome,
    ClaimBrew,
    ClaimReforge {
        player: u32,
        part: knights::Part,
        words: String,
    },
    DropQueuedBrew {
        ask: super::cauldron::Brewing,
    },
    SyncHome {
        home: super::home::Home,
        treasury: crate::realm::Spoils,
    },
    Retreat,
    LeaveActivity {
        player: u32,
    },
    DropOffer {
        player: u32,
    },
    /// Opens the genie's typed line. Not the six-row hub.
    OpenWishLine {
        player: u32,
    },
    /// One letter on that line.
    TypeWishLetter {
        player: u32,
        letter: u8,
    },
    ChooseOffer {
        player: u32,
        index: u8,
    },
    TurnOffer {
        player: u32,
    },
    Vote {
        player: u32,
        command: votes::Command,
        local_focus: bool,
    },
    DismissFloorPlate {
        players: Vec<u32>,
    },
    Visit {
        destination: Destination,
        wishes: Option<Arc<Vec<Wish>>>,
    },
    TakeEntrance {
        entrance: String,
    },
    AdmitSettlement {
        site: settlement::Site,
    },
    EnterSettlement {
        site: String,
    },
    BeginDelve {
        pack: super::Pack,
        knight: String,
    },
    ConfigureSession {
        naked_start: bool,
        identity: bool,
        /// A new knight owes the realm a rite (`rite`); a session from
        /// before it asks none.
        #[serde(default)]
        rite: bool,
        host: String,
    },
    JoinSeat {
        player: u32,
        name: String,
        outfit: JoinOutfit,
    },
    EndHosting {
        host: String,
        reclaim_first: bool,
    },
    RemoveSeat {
        player: u32,
        retain_first: bool,
    },
    EquipForged {
        player: u32,
        weapon: Option<forge::Weapon>,
        reset_temper: bool,
    },
    MoveCard {
        player: u32,
        from: usize,
        to: usize,
    },
    /// A rough block a knight brought up from the last delve on their back:
    /// the next run at home begins with it on their shoulders, to carry on
    /// to the stockyard (`quarrying`).
    CarryBlock {
        player: u32,
        stone: String,
    },
    AddCard {
        player: u32,
        card: cards::Card,
    },
    /// A thing the knight made in the Creator and brought to life for the realm (`rite`):
    /// checked again by the run, kept by the realm and taken into the world.
    Contribute {
        player: u32,
        made: super::rite::Made,
    },
    Reforge {
        player: u32,
        part: knights::Part,
        card: cards::Card,
    },
    LearnWish {
        player: u32,
        words: String,
    },
    KnownWish {
        player: u32,
        wish: String,
    },
    PrepareVotes {
        seats: Vec<(u32, String, u64)>,
        records: votes::Records,
        seen: std::collections::BTreeSet<String>,
        couch: bool,
        initialize: bool,
    },
    ClaimVotes {
        records: votes::Records,
        seen: std::collections::BTreeSet<String>,
        changes: Vec<votes::Change>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Destination {
    Keep,
    Alehouse,
    College,
    Stables,
    Lists,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VisitState {
    Opened,
    Underground,
    Busy,
    Unavailable,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "knight", rename_all = "kebab-case")]
pub enum JoinOutfit {
    Keep,
    Company,
    Chosen(String),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub session: String,
    pub raid: u64,
    pub id: u64,
    pub commands: Vec<Command>,
}
impl Request {
    /// Structural admission bounds only. Native captured inputs can classify a
    /// permanent payload refusal before waiting on temporary receipt capacity.
    pub fn validate_payload(&self) -> Result<(), String> {
        admission_bytes(&self.commands).map(|_| ())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case")]
pub enum Reply {
    Done,
    Changed(bool),
    Chosen(String),
    Visited(VisitState),
    Names(Vec<String>),
    Realm(RealmTransfer),
    Home(HomeTransfer),
    Brew(super::brew_jobs::Work),
    Reforge(super::reforge_jobs::Work),
    Wish(super::wish_jobs::Work),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealmTransfer {
    pub hauls: Vec<crate::realm::Haul>,
    pub reclaimed: Vec<super::Pack>,
    pub triumph: Option<super::Triumph>,
    pub names: std::collections::BTreeMap<u32, String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealmClaim {
    pub session: String,
    pub raid: u64,
    pub request: u64,
    pub transfer: RealmTransfer,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub id: u64,
    pub result: Result<Vec<Reply>, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    commands: Vec<Command>,
    receipt: Receipt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    home: Option<HomeCompletion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    brew: Option<super::brew_jobs::Completion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reforge: Option<super::reforge_jobs::Completion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    wish: Option<super::wish_jobs::Completion>,
    #[serde(skip)]
    held_bytes: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct Journal {
    schema: u32,
    session: String,
    raid: u64,
    high_water: u64,
    records: VecDeque<Record>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    home_seen: Option<HomeObserved>,
    #[serde(default)]
    votes_initialized: bool,
    #[serde(skip)]
    held_bytes: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    schema: u32,
    session: String,
    raid: u64,
    high_water: u64,
    records: VecDeque<Record>,
    #[serde(default)]
    home_seen: Option<HomeObserved>,
    #[serde(default)]
    votes_initialized: bool,
}
impl Journal {
    pub fn new(session: String, raid: u64) -> Self {
        Self {
            schema: 6,
            session,
            raid,
            high_water: 0,
            records: VecDeque::new(),
            home_seen: None,
            votes_initialized: false,
            held_bytes: 0,
        }
    }
    pub fn session(&self) -> &str {
        &self.session
    }
    pub fn next_id(&self) -> Result<u64, String> {
        self.high_water
            .checked_add(1)
            .ok_or_else(|| "game command sequence exhausted".into())
    }
    pub fn capture(&self) -> Result<serde_json::Value, String> {
        serde_json::to_value(self).map_err(|e| e.to_string())
    }
    pub fn restore(value: serde_json::Value, session: &str, raid: u64) -> Result<Self, String> {
        let mut saved: Saved = serde_json::from_value(value).map_err(|e| e.to_string())?;
        if !matches!(saved.schema, 1 | 2 | 3 | 4 | 5 | 6)
            || saved.session != session
            || !text(&saved.session, 128)
            || saved.raid != raid
            || saved.records.len() > KEPT_RECEIPTS
            || saved.records.len() as u64 > saved.high_water
        {
            return Err("invalid game command receipt checkpoint".into());
        }
        let mut previous = 0;
        if saved.schema == 1
            && (saved.home_seen.is_some()
                || saved.records.iter().any(|record| {
                    record.home.is_some()
                        || matches!(record.commands.as_slice(), [Command::ClaimHome])
                }))
        {
            return Err("schema-1 journal cannot contain home transactions".into());
        }
        if saved.schema < 3
            && saved.records.iter().any(|record| {
                record.brew.is_some()
                    || record.commands.iter().any(|command| {
                        matches!(command, Command::ClaimBrew | Command::DropQueuedBrew { .. })
                    })
            })
        {
            return Err("old journal schema cannot contain brew jobs".into());
        }
        if saved.schema < 4
            && saved.records.iter().any(|record| {
                record.reforge.is_some()
                    || record
                        .commands
                        .iter()
                        .any(|command| matches!(command, Command::ClaimReforge { .. }))
            })
        {
            return Err("old journal schema cannot contain reforge jobs".into());
        }
        if saved.schema < 5 && saved.records.iter().any(|r| r.wish.is_some() || r.commands.iter().any(|c| matches!(c, Command::ClaimWish { .. }))) {
            return Err("old journal schema cannot contain wish claims".into());
        }
        if saved.schema < 6
            && (saved.votes_initialized
                || saved.records.iter().any(|record| {
                    record.commands.iter().any(|command| {
                        matches!(command, Command::ClaimVotes { .. })
                            || matches!(command, Command::PrepareVotes { initialize: true, .. })
                    })
                }))
        {
            return Err("old journal schema cannot contain playtest vote outboxes".into());
        }
        let mut held_bytes = observed_bytes(saved.home_seen.as_ref())?;
        let mut realm_claims = 0;
        let mut home_claims = 0;
        let mut vote_claims = 0;
        for record in &mut saved.records {
            if matches!(record.commands.as_slice(), [Command::ClaimRealm]) {
                realm_claims += 1;
            }
            if matches!(record.commands.as_slice(), [Command::ClaimHome]) {
                home_claims += 1;
            }
            if record
                .commands
                .iter()
                .any(|command| matches!(command, Command::ClaimVotes { .. }))
                && !matches!(record.commands.as_slice(), [Command::ClaimVotes { .. }])
            {
                return Err("invalid retained playtest vote export".into());
            }
            if let [Command::ClaimVotes { records, seen, changes }] = record.commands.as_slice() {
                vote_claims += 1;
                if !valid_vote_facts(records, seen)
                    || changes.len() > 4096
                    || !changes.iter().all(valid_vote_change)
                    || record.receipt.result.as_ref().is_ok_and(|replies| {
                        !matches!(replies.as_slice(), [Reply::Done])
                    })
                {
                    return Err("invalid retained playtest vote export".into());
                }
            }
            if record.receipt.id <= previous
                || record.receipt.id > saved.high_water
                || !reply_bounded(&record.receipt.result)
                || record.receipt.result.as_ref().is_ok_and(|replies| {
                    replies.len() != record.commands.len()
                        || record.commands.iter().zip(replies).any(|(command, reply)| {
                            (matches!(command, Command::ClaimRealm)
                                != matches!(reply, Reply::Realm(_)))
                                || (matches!(command, Command::ClaimHome)
                                    != matches!(reply, Reply::Home(_)))
                                || (matches!(command, Command::ClaimBrew)
                                    != matches!(reply, Reply::Brew(_)))
                                || (matches!(command, Command::ClaimReforge { .. })
                                    != matches!(reply, Reply::Reforge(_)))
                                || (matches!(command, Command::ClaimWish { .. }) != matches!(reply, Reply::Wish(_)))
                        })
                })
                || record.brew.as_ref().is_some_and(|done| {
                    match record.receipt.result.as_ref().map(Vec::as_slice) {
                        Ok([Reply::Brew(work)])
                            if matches!(record.commands.as_slice(), [Command::ClaimBrew]) =>
                        {
                            !done.valid(work)
                        }
                        _ => true,
                    }
                })
                || matches!(record.receipt.result.as_ref().map(Vec::as_slice),Ok([Reply::Reforge(work)]) if
                    !matches!(record.commands.as_slice(),[Command::ClaimReforge{player,part,words}] if *player==work.player && *part==work.part && words==&work.words)
                    || work.result_id != super::reforge_jobs::Work::result_id(&saved.session,raid,record.receipt.id))
                || record.reforge.as_ref().is_some_and(|done| {
                    match record.receipt.result.as_ref().map(Vec::as_slice) {
                        Ok([Reply::Reforge(work)])
                            if matches!(
                                record.commands.as_slice(),
                                [Command::ClaimReforge { .. }]
                            ) =>
                        {
                            !done.valid(work)
                        }
                        _ => true,
                    }
                })
                || matches!(record.receipt.result.as_ref().map(Vec::as_slice), Ok([Reply::Wish(work)]) if
                    !matches!(record.commands.as_slice(), [Command::ClaimWish { ask, maker }] if ask == &work.ask && maker == &work.maker)
                    || !work.valid() || work.binding != super::wish_jobs::Work::binding(&saved.session, raid, record.receipt.id))
                || record.wish.as_ref().is_some_and(|done| match record.receipt.result.as_ref().map(Vec::as_slice) {
                    Ok([Reply::Wish(work)]) if matches!(record.commands.as_slice(), [Command::ClaimWish { .. }]) => !done.valid(work), _ => true,
                })
                || realm_claims > 1
                || home_claims > 1
                || vote_claims > 1
                || (vote_claims > 0 && !saved.votes_initialized)
                || record.receipt.result.as_ref().is_ok_and(|_| {
                    record.commands.iter().any(|command| {
                        matches!(command, Command::PrepareVotes { initialize: true, .. })
                    }) && !saved.votes_initialized
                })
                || record.home.as_ref().is_some_and(|home| {
                    match record.receipt.result.as_ref().map(Vec::as_slice) {
                        Ok([Reply::Home(transfer)])
                            if matches!(record.commands.as_slice(), [Command::ClaimHome]) =>
                        {
                            !home.delivered.valid_for(transfer, &home.result)
                        }
                        _ => true,
                    }
                })
            {
                return Err("invalid game command receipt checkpoint".into());
            }
            previous = record.receipt.id;
            record.held_bytes = record_bytes(record)?;
            held_bytes += record.held_bytes;
            if held_bytes > MAX_PENDING_BYTES {
                return Err("game command receipt checkpoint exceeds byte capacity".into());
            }
        }
        Ok(Self {
            schema: 6,
            session: saved.session,
            raid,
            high_water: saved.high_water,
            records: saved.records,
            home_seen: saved.home_seen,
            votes_initialized: saved.votes_initialized,
            held_bytes,
        })
    }
    /// Retire one received result. Completions may arrive out of order; a
    /// later acknowledgement must not retire an older unresolved operation.
    pub fn acknowledge(&mut self, raid: u64, id: u64) -> Result<(), String> {
        if raid != self.raid || id == 0 || id > self.high_water {
            return Err("invalid game receipt acknowledgement".into());
        }
        if let Some(index) = self
            .records
            .iter()
            .position(|record| record.receipt.id == id)
        {
            let record = &self.records[index];
            if matches!(record.commands.as_slice(), [Command::ClaimHome])
                && record.receipt.result.is_ok()
                && record.home.is_none()
            {
                return Err(
                    "home claim requires a committed delivery before acknowledgement".into(),
                );
            }
            if matches!(record.commands.as_slice(), [Command::ClaimBrew])
                && record.receipt.result.is_ok()
                && record.brew.is_none()
            {
                return Err("brew claim requires a terminal receipt before acknowledgement".into());
            }
            if matches!(record.commands.as_slice(), [Command::ClaimReforge { .. }])
                && record.receipt.result.is_ok()
                && record.reforge.is_none()
            {
                return Err("reforge requires a terminal receipt before acknowledgement".into());
            }
            if matches!(record.commands.as_slice(), [Command::ClaimWish { .. }]) && record.receipt.result.is_ok() && record.wish.is_none() {
                return Err("wish requires a terminal receipt before acknowledgement".into());
            }
            if matches!(record.commands.as_slice(), [Command::ClaimVotes { .. }])
                && record.receipt.result.is_ok()
            {
                return Err("vote export requires its original durable acknowledgement".into());
            }
            self.held_bytes -= self.records.remove(index).unwrap().held_bytes;
        }
        Ok(())
    }
    pub fn has_pending_realm(&self) -> bool {
        self.records
            .iter()
            .any(|record| matches!(record.commands.as_slice(), [Command::ClaimRealm]))
    }
    pub fn pending_realm(&self) -> Result<Option<RealmClaim>, String> {
        let Some(record) = self
            .records
            .iter()
            .find(|record| matches!(record.commands.as_slice(), [Command::ClaimRealm]))
        else {
            return Ok(None);
        };
        match record
            .receipt
            .result
            .as_ref()
            .map_err(Clone::clone)?
            .as_slice()
        {
            [Reply::Realm(transfer)] => Ok(Some(RealmClaim {
                session: self.session.clone(),
                raid: self.raid,
                request: record.receipt.id,
                transfer: transfer.clone(),
            })),
            _ => Err("invalid retained realm claim result".into()),
        }
    }
    pub fn claim_realm(&mut self, run: &mut Run) -> Result<Option<RealmClaim>, String> {
        if run.raid_id != self.raid {
            return Err("realm claim belongs to another raid".into());
        }
        if let Some(claim) = self.pending_realm()? {
            return Ok(Some(claim));
        }
        if run.bank.is_empty() && run.reclaimed.is_empty() && run.triumph.is_none() {
            return Ok(None);
        }
        let receipt = self.apply(
            run,
            Request {
                session: self.session.clone(),
                raid: self.raid,
                id: self.next_id()?,
                commands: vec![Command::ClaimRealm],
            },
        )?;
        if let Err(error) = receipt.result {
            // Rejected preparation never consumed the world queues.
            self.acknowledge(run.raid_id, receipt.id)?;
            return Err(error);
        }
        self.pending_realm()
    }
    pub fn acknowledge_realm(&mut self, claim: &RealmClaim) -> Result<(), String> {
        if claim.session != self.session || claim.raid != self.raid {
            return Err("realm claim belongs to another session".into());
        }
        if let Some(record) = self
            .records
            .iter()
            .find(|record| record.receipt.id == claim.request)
        {
            if !matches!(record.commands.as_slice(), [Command::ClaimRealm])
                || record.receipt.result.as_ref().is_err()
                || record.receipt.result.as_ref().unwrap().as_slice()
                    != [Reply::Realm(claim.transfer.clone())]
            {
                return Err("realm acknowledgement differs from retained claim".into());
            }
        }
        self.acknowledge(claim.raid, claim.request)
    }
    pub fn sync_home(
        &mut self,
        run: &mut Run,
        home: super::home::Home,
        treasury: crate::realm::Spoils,
    ) -> Result<bool, String> {
        if run.home
            == projected_home(
                run,
                &home,
                self.home_seen.as_ref().map(HomeObserved::view),
                self.pending_home_view(),
            )
            && run.treasury == treasury
        {
            return Ok(false);
        }
        let receipt = self.apply(
            run,
            Request {
                session: self.session.clone(),
                raid: self.raid,
                id: self.next_id()?,
                commands: vec![Command::SyncHome { home, treasury }],
            },
        )?;
        self.acknowledge(run.raid_id, receipt.id)?;
        receipt
            .result
            .map(|replies| replies == [Reply::Changed(true)])
    }
    /// A checkpoint may precede a durable realm company. Reconcile before
    /// simulation only when no retained claim owns the run and the realm has
    /// changed since the last observed transaction (or there is no baseline).
    pub fn needs_company_reconciliation(&self, company: &super::squire_company::Company) -> bool {
        !self.has_pending_home()
            && self.home_seen.as_ref().is_none_or(|seen| !seen.matches_company(company))
    }
    pub fn has_pending_home(&self) -> bool {
        self.records
            .iter()
            .any(|record| matches!(record.commands.as_slice(), [Command::ClaimHome]))
    }
    fn pending_home_view(&self) -> Option<&super::home::Home> {
        self.records
            .iter()
            .find_map(|record| match &record.receipt.result {
                Ok(replies) => match replies.as_slice() {
                    [Reply::Home(transfer)] => Some(&transfer.view),
                    _ => None,
                },
                Err(_) => None,
            })
    }
    pub fn validate_home_world(&self, run: &Run) -> Result<(), String> {
        if self.raid != run.raid_id {
            return Err("game journal belongs to another raid".into());
        }
        let mut company_claims = 0;
        for record in &self.records {
            if record.home.is_none()
                && let Ok(replies) = &record.receipt.result
                && let [Reply::Home(transfer)] = replies.as_slice()
            {
                transfer.validate_world(run)?;
                company_claims += usize::from(transfer.company_reserved);
            }
        }
        if company_claims > 1 || run.squire_company_claimed != (company_claims == 1) {
            return Err("squire company hold differs from pending home claim".into());
        }
        Ok(())
    }
    pub fn pending_home(&self) -> Result<Option<HomeClaim>, String> {
        let Some(record) = self
            .records
            .iter()
            .find(|record| matches!(record.commands.as_slice(), [Command::ClaimHome]))
        else {
            return Ok(None);
        };
        match record
            .receipt
            .result
            .as_ref()
            .map_err(Clone::clone)?
            .as_slice()
        {
            [Reply::Home(transfer)] => Ok(Some(HomeClaim {
                session: self.session.clone(),
                raid: self.raid,
                request: record.receipt.id,
                transfer: transfer.clone(),
            })),
            _ => Err("invalid retained home claim result".into()),
        }
    }
    /// Exact readiness shared by discovery and claim, borrowing retained work.
    pub fn has_home_work(&self, run: &Run, context: Option<&HomeContext>) -> Result<bool, String> {
        if run.raid_id != self.raid { return Err("home claim belongs to another raid".into()); }
        if self.has_pending_home() {
            return self.pending_home_view().map(|_| true).ok_or_else(|| "invalid retained home claim result".into());
        }
        Ok(!(run.orders.is_empty() && run.lane_squire_orders.is_empty() && run.raise_orders.is_empty() && run.peddled.is_empty() && run.feats.is_empty() && run.marks.is_empty()
            && self.home_seen.as_ref().is_some_and(|seen| seen.matches(run) && context.is_none_or(|context| seen.matches_context(context)))))
    }
    pub fn claim_home(&mut self, run: &mut Run) -> Result<Option<HomeClaim>, String> {
        self.claim_home_for(run, None)
    }
    pub fn claim_home_with_context(
        &mut self,
        run: &mut Run,
        context: &HomeContext,
    ) -> Result<Option<HomeClaim>, String> {
        self.claim_home_for(run, Some(context))
    }
    fn claim_home_for(
        &mut self,
        run: &mut Run,
        context: Option<&HomeContext>,
    ) -> Result<Option<HomeClaim>, String> {
        if run.raid_id != self.raid {
            return Err("home claim belongs to another raid".into());
        }
        if let Some(claim) = self.pending_home()? {
            return Ok(Some(claim));
        }
        if !self.has_home_work(run, context)? { return Ok(None); }
        let receipt = self.apply(
            run,
            Request {
                session: self.session.clone(),
                raid: self.raid,
                id: self.next_id()?,
                commands: vec![Command::ClaimHome],
            },
        )?;
        if let Err(error) = receipt.result {
            self.acknowledge(run.raid_id, receipt.id)?;
            return Err(error);
        }
        self.pending_home()
    }
    fn home_record(&self, claim: &HomeClaim) -> Result<usize, String> {
        if claim.session != self.session || claim.raid != self.raid {
            return Err("home claim belongs to another session".into());
        }
        let index = self
            .records
            .iter()
            .position(|record| record.receipt.id == claim.request)
            .ok_or("home claim receipt retired; delivery was not replayed")?;
        let record = &self.records[index];
        if !matches!(record.commands.as_slice(), [Command::ClaimHome])
            || !record.receipt.result.as_ref().is_ok_and(|replies| matches!(replies.as_slice(), [Reply::Home(transfer)] if transfer == &claim.transfer)) {
            return Err("home claim differs from retained request".into());
        }
        Ok(index)
    }
    pub fn home_completion(&self, claim: &HomeClaim) -> Result<Option<HomeCompletion>, String> {
        Ok(self.records[self.home_record(claim)?].home.clone())
    }
    pub fn complete_home(
        &mut self,
        run: &mut Run,
        claim: &HomeClaim,
        result: HomeResult,
    ) -> Result<HomeDelivered, String> {
        if run.raid_id != self.raid {
            return Err("home delivery belongs to another raid".into());
        }
        let index = self.home_record(claim)?;
        if let Some(done) = &self.records[index].home {
            if done.result != result {
                return Err("home result ID reused with different content".into());
            }
            return Ok(done.delivered.clone());
        }
        encoded_bytes(&result, MAX_RESULT_BYTES)?;
        let mut candidate = run.clone();
        claim.transfer.consume(&mut candidate)?;
        let delivered = result.deliver(
            &mut candidate,
            &claim.transfer,
            &format!("{}-{}", self.session, claim.request),
        )?;
        let seen = claim.transfer.observed(&result);
        let mut record = self.records[index].clone();
        record.home = Some(HomeCompletion {
            result,
            delivered: delivered.clone(),
        });
        let new_bytes = record_bytes(&record)?;
        let held = self.held_bytes
            - self.records[index].held_bytes
            - observed_bytes(self.home_seen.as_ref())?
            + new_bytes
            + observed_bytes(Some(&seen))?;
        if held > MAX_PENDING_BYTES {
            return Err("home completion exceeds retained game byte capacity".into());
        }
        record.held_bytes = new_bytes;
        // World effects, exact result, delivery/refund receipt and consumed
        // observations commit together. No fallible operation follows.
        *run = candidate;
        self.records[index] = record;
        self.home_seen = Some(seen);
        self.held_bytes = held;
        Ok(delivered)
    }
    pub fn acknowledge_home(
        &mut self,
        claim: &HomeClaim,
        completed: &HomeCompletion,
    ) -> Result<(), String> {
        if claim.session != self.session || claim.raid != self.raid {
            return Err("home acknowledgement belongs to another session".into());
        }
        if self
            .records
            .iter()
            .any(|record| record.receipt.id == claim.request)
        {
            let index = self.home_record(claim)?;
            if self.records[index].home.as_ref() != Some(completed) {
                return Err("home acknowledgement differs from committed delivery".into());
            }
        }
        self.acknowledge(claim.raid, claim.request)
    }

    /// Admission failures do not consume an ID. Admitted game rejections do:
    /// their receipt is as stable as success, even when the world later changes.
    pub fn apply(&mut self, run: &mut Run, request: Request) -> Result<Receipt, String> {
        self.apply_with_scroll(run, request, |run, player, intent| {
            let capture = super::scrolls::Capture::new(run, player, intent)?;
            let result = capture.request.evaluate()?;
            capture.prepare(run, result)
        })
    }
    /// The journal owns admission/replay and the atomic commit. The selected
    /// implementation prepares a scroll from immutable facts, never mutable Run
    /// access or native model/file resources.
    pub fn apply_with_scroll(
        &mut self, run: &mut Run, request: Request,
        mut prepare: impl FnMut(&Run, u32, &super::scrolls::Intent) -> Result<super::scrolls::Prepared, String>,
    ) -> Result<Receipt, String> {
        if request.session != self.session || !text(&request.session, 128) {
            return Err("game command belongs to another session".into());
        }
        if request.raid != self.raid || run.raid_id != self.raid {
            return Err("game command belongs to another raid".into());
        }
        if request.id == 0 {
            return Err("game command ID must be nonzero".into());
        }
        let request_bytes = admission_bytes(&request.commands)?;
        if request.id <= self.high_water {
            let record = self
                .records
                .iter()
                .find(|record| record.receipt.id == request.id)
                .ok_or("game command receipt retired; command was not replayed")?;
            if record.commands != request.commands {
                return Err("game command ID reused with different content".into());
            }
            return Ok(record.receipt.clone());
        }
        if request.id != self.next_id()? {
            return Err("game command sequence has a gap".into());
        }
        if request.commands.iter().any(|command| {
            matches!(command, Command::PrepareVotes { .. } | Command::ClaimVotes { .. })
        }) && request.commands.len() != 1
        {
            return Err("playtest vote commands must be admitted individually".into());
        }
        if matches!(request.commands.as_slice(), [Command::ClaimRealm]) && self.has_pending_realm()
        {
            return Err("a realm claim is already awaiting acknowledgement".into());
        }
        if matches!(request.commands.as_slice(), [Command::ClaimVotes { .. }])
            && self.pending_votes()?.is_some()
        {
            return Err("a playtest vote export is already awaiting acknowledgement".into());
        }
        if let [Command::PrepareVotes { initialize: true, .. }] = request.commands.as_slice()
            && self.votes_initialized
        {
            return Err("playtest votes are already initialized".into());
        }
        if let [Command::ClaimVotes { records, seen, changes }] = request.commands.as_slice()
            && (!self.votes_initialized
                || !run.playtest.ready
                || run.playtest.records != *records
                || run.playtest.seen != *seen
                || run.playtest.changes != *changes
                || (!run.playtest.dirty && changes.is_empty()))
        {
            return Err("playtest vote claim does not match initialized run facts".into());
        }
        if matches!(request.commands.as_slice(), [Command::ClaimHome]) && self.has_pending_home() {
            return Err("a home claim is already awaiting acknowledgement".into());
        }
        if self.records.len() == KEPT_RECEIPTS
            || self.held_bytes + request_bytes + MAX_RESULT_BYTES > MAX_PENDING_BYTES
        {
            return Err("game receipt capacity reached; acknowledge completed results".into());
        }
        // Host intentions are infrequent. Copy the candidate once so a game
        // rejection cannot partially mutate its world. Tick/ability batches
        // need their own prepared transactions, rather than cloning per action.
        let prepares_votes = request.commands.iter().any(|command| {
            matches!(command, Command::PrepareVotes { .. })
        });
        let staged = (|| {
            for command in &request.commands {
                command.validate()?;
            }
            let mut candidate = run.clone();
            let replies = request
                .commands
                .iter()
                .map(|command| {
                    if let Some((player, intent)) = command.scroll_intent() {
                        // A freedom wish's words are for the place it was
                        // begun at, never the knight (`freedom`).
                        if candidate.players.get(&player).is_some_and(|h| h.place_wish.is_some()) {
                            let said = super::freedom::plaque_words(&match &intent {
                                super::scrolls::Intent::Local { words, .. } | super::scrolls::Intent::Learn { words } => words.clone(),
                                super::scrolls::Intent::Known { wish } => super::phrasebook::get(wish).map_or_else(|| wish.clone(), |w| w.name.clone()),
                            });
                            return candidate.offer_place_wish(player, &said).map(|()| {
                                if matches!(command, Command::Scroll { .. }) { Reply::Chosen(said) } else { Reply::Done }
                            });
                        }
                        // The tavern mirror's look is a wish of its own: no
                        // strength, gear or wish spent (`cosmetic_wish`).
                        let mirror = candidate.players.get(&player).is_some_and(|h| h.mirror_wish);
                        if let (true, super::scrolls::Intent::Local { words, .. } | super::scrolls::Intent::Learn { words }) = (mirror, &intent) {
                            let said = words.trim().to_string();
                            return candidate.offer_mirror_wish(player, &said, None).map(|()| {
                                if matches!(command, Command::Scroll { .. }) { Reply::Chosen(said) } else { Reply::Done }
                            });
                        }
                        // So is a visitor's wish for a city of the Deep
                        // (`deep::city`): the city builds it, the knight
                        // spends nothing.
                        let city = candidate.city_wishing(player);
                        if let (true, super::scrolls::Intent::Local { words, .. } | super::scrolls::Intent::Learn { words }) = (city, &intent) {
                            let said = words.trim().to_string();
                            return candidate.offer_city_wish(player, &said).map(|()| {
                                if matches!(command, Command::Scroll { .. }) { Reply::Chosen(said) } else { Reply::Done }
                            });
                        }
                        prepare(&candidate, player, &intent)?.deliver(&mut candidate).map(|said| {
                            if matches!(command, Command::Scroll { .. }) { Reply::Chosen(said) } else { Reply::Done }
                        })
                    } else if let Command::Catalogue { batch } = command {
                        batch.deliver(&mut candidate, self).map(Reply::Changed)
                    } else if let Command::ClaimWish { ask, maker } = command {
                        super::wish_jobs::Work::capture(&mut candidate, &self.session, request.id, ask.clone(), maker.clone()).map(Reply::Wish)
                    } else if matches!(command, Command::ClaimBrew) {
                        super::brew_jobs::Work::capture(&mut candidate, &self.session, request.id)
                            .map(Reply::Brew)
                    } else if let Command::ClaimReforge {
                        player,
                        part,
                        words,
                    } = command
                    {
                        super::reforge_jobs::Work::capture(
                            &mut candidate,
                            &self.session,
                            request.id,
                            *player,
                            *part,
                            words.clone(),
                        )
                        .map(Reply::Reforge)
                    } else {
                        command.execute(
                            &mut candidate,
                            self.home_seen.as_ref().map(HomeObserved::view),
                            self.pending_home_view(),
                        )
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            if matches!(request.commands.as_slice(), [Command::ClaimHome]) {
                self.validate_home_world(run)?;
                match replies.as_slice() {
                    [Reply::Home(transfer)] => transfer.validate_world(&candidate)?,
                    _ => return Err("invalid new home claim".into()),
                }
            } else {
                self.validate_home_world(&candidate)?;
            }
            self.validate_brew_world(&candidate)?;
            if !request
                .commands
                .iter()
                .any(|c| matches!(c, Command::ClaimReforge { .. }))
            {
                self.validate_reforge_world(&candidate)?;
            }
            if !request.commands.iter().any(|c| matches!(c, Command::ClaimWish { .. })) { self.validate_wish_world(&candidate)?; }
            Ok::<_, String>((candidate, replies))
        })();
        let (mut candidate, mut result) = match staged {
            Ok((candidate, replies)) => (Some(candidate), Ok(replies)),
            Err(error) => (None, Err(error)),
        };
        if !reply_bounded(&result) || encoded_bytes(&result, MAX_RESULT_BYTES).is_err() {
            candidate = None;
            result = Err("game command reply exceeds contract".into());
        }
        let held_bytes = request_bytes + encoded_bytes(&result, MAX_RESULT_BYTES)?;
        let receipt = Receipt {
            id: request.id,
            result,
        };
        // Every fallible preparation is finished before world and receipt commit.
        if let Some(candidate) = candidate {
            *run = candidate;
        }
        self.high_water = request.id;
        self.records.push_back(Record {
            commands: request.commands,
            receipt: receipt.clone(),
            home: None,
            brew: None,
            reforge: None,
            wish: None,
            held_bytes,
        });
        self.held_bytes += held_bytes;
        if prepares_votes && receipt.result.is_ok() {
            self.votes_initialized = true;
        }
        Ok(receipt)
    }

}
fn projected_home(
    run: &Run,
    home: &super::home::Home,
    baseline: Option<&super::home::Home>,
    pending: Option<&super::home::Home>,
) -> super::home::Home {
    let mut home = home.clone();
    for hero in run.players.values() {
        home.adopt(
            &super::talents::knight_key(hero),
            &super::talents::profile_key(hero),
        );
    }
    super::ranch::migrate(&mut home.ranch, &mut home.stable);
    // rebuild_home retains the run's chosen landing. Compare that effective
    // projection here too, avoiding a whole-world transaction on every poll.
    home.landing = run.home.landing;
    if run.squire_company_reserved || run.squire_company_claimed
        || baseline.is_some_and(|old| home.squire_company == old.squire_company)
        || pending.is_some_and(|claim| home.squire_company == claim.squire_company)
    {
        home.squire_company = run.home.squire_company.clone();
    }
    if baseline.is_none()
        || baseline.is_some_and(|old| home.squires_lodge == old.squires_lodge)
        || pending.is_some_and(|claim| home.squires_lodge == claim.squires_lodge)
    {
        home.squires_lodge = run.home.squires_lodge.clone();
    }
    if !run.lane_squire_orders.is_empty() {
        home.lane_squires = run.home.lane_squires.clone();
    }
    // While a payment is pending, either the previous projection or the
    // prepared claim may be the destination's current value. Neither may
    // overwrite a newer local edit made after capture.
    if baseline.is_none()
        || baseline.is_some_and(|old| home.squire_academy == old.squire_academy)
        || pending.is_some_and(|claim| home.squire_academy == claim.squire_academy)
    {
        home.squire_academy = run.home.squire_academy.clone();
    }
    if baseline.is_none()
        || baseline.is_some_and(|old| home.wardrobe == old.wardrobe)
        || pending.is_some_and(|claim| home.wardrobe == claim.wardrobe)
    {
        home.wardrobe = run.home.wardrobe.clone();
    }
    if baseline.is_none()
        || baseline.is_some_and(|old| home.privy_wall == old.privy_wall)
        || pending.is_some_and(|claim| home.privy_wall == claim.privy_wall)
    {
        home.privy_wall = run.home.privy_wall.clone();
    }
    // The run's salvage yard is kept when it is newer than the realm's
    // (`salvage`): a find, a wish or a build since the realm last saw it.
    if run.home.industry.rev > home.industry.rev { home.industry = run.home.industry.clone(); }
    if run.home.kingdom_gear.rev > home.kingdom_gear.rev { home.kingdom_gear = run.home.kingdom_gear.clone(); }
    if run.home.stockyard.rev > home.stockyard.rev { home.stockyard = run.home.stockyard.clone(); }
    if run.home.salvage.rev > home.salvage.rev {
        home.salvage = run.home.salvage.clone();
    }
    if baseline.is_none()
        || baseline.is_some_and(|old| home.wished == old.wished)
        || pending.is_some_and(|claim| home.wished == claim.wished)
    {
        home.wished = run.home.wished.clone();
    }
    if baseline.is_none()
        || baseline.is_some_and(|old| home.sky == old.sky)
        || pending.is_some_and(|claim| home.sky == claim.sky)
    {
        home.sky = run.home.sky.clone();
    }
    home.countryside.games.merge(&run.home.countryside.games);
    home.keep_cave(run.home.cave_seed, &run.home.mines);
    home
}
fn observed_bytes(seen: Option<&HomeObserved>) -> Result<usize, String> {
    seen.map_or(Ok(0), |seen| encoded_bytes(seen, MAX_RESULT_BYTES))
}
fn record_bytes(record: &Record) -> Result<usize, String> {
    Ok(admission_bytes(&record.commands)?
        + encoded_bytes(&record.receipt.result, MAX_RESULT_BYTES)?
        + record
            .home
            .as_ref()
            .map_or(Ok(0), |done| encoded_bytes(done, MAX_RESULT_BYTES))?
        + record
            .brew
            .as_ref()
            .map_or(Ok(0), |done| encoded_bytes(done, MAX_RESULT_BYTES))?
        + record
            .reforge
            .as_ref()
            .map_or(Ok(0), |done| encoded_bytes(done, MAX_RESULT_BYTES))?
        + record.wish.as_ref().map_or(Ok(0), |done| encoded_bytes(done, MAX_RESULT_BYTES))?)
}

// Count JSON bytes without allocating a second copy of large definition data.
// This is an in-memory bound, not an IO operation.
fn encoded_bytes(value: &impl Serialize, limit: usize) -> Result<usize, String> {
    struct Count {
        bytes: usize,
        limit: usize,
    }
    impl std::io::Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > self.limit.saturating_sub(self.bytes) {
                return Err(std::io::Error::other("game command byte limit exceeded"));
            }
            self.bytes += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut count = Count { bytes: 0, limit };
    serde_json::to_writer(&mut count, value).map_err(|error| error.to_string())?;
    Ok(count.bytes)
}
fn admission_bytes(commands: &[Command]) -> Result<usize, String> {
    if commands.len() != 1 && commands.iter().any(Command::requires_acknowledgement) {
        return Err("claims require a dedicated retained request".into());
    }
    if commands.is_empty() || commands.len() > 64 || commands.iter().any(|command| {
        matches!(command, Command::DismissFloorPlate { players } if players.len() > super::MAX_PLAYERS as usize)
    }) { return Err("game command payload exceeds admission bound".into()); }
    encoded_bytes(&commands, MAX_REQUEST_BYTES)
}
fn reply_bounded(reply: &Result<Vec<Reply>, String>) -> bool {
    match reply {
        Ok(replies) => {
            replies.len() <= 64
                && replies.iter().all(|reply| match reply {
                    Reply::Chosen(name) => name.len() <= 4096,
                    Reply::Names(names) => {
                        names.len() <= super::MAX_PLAYERS as usize
                            && names.iter().all(|name| name.len() <= 1024)
                    }
                    Reply::Home(transfer) => transfer.valid(),
                    Reply::Brew(work) => work.valid(),
                    Reply::Reforge(work) => work.valid(),
                    Reply::Wish(work) => work.valid(),
                    Reply::Realm(transfer) => {
                        transfer.hauls.len() <= 64
                            && transfer.reclaimed.len() <= 64
                            && transfer.names.len() <= super::MAX_PLAYERS as usize
                            && transfer.names.values().all(|name| name.len() <= 1024)
                    }
                    _ => true,
                })
        }
        Err(error) => error.len() <= 4096,
    }
}

impl Command {
    fn scroll_intent(&self) -> Option<(u32, super::scrolls::Intent)> {
        match self {
            Self::Scroll { player, intent } => Some((*player, intent.clone())),
            Self::LearnWish { player, words } => Some((*player, super::scrolls::Intent::Learn { words: words.clone() })),
            Self::KnownWish { player, wish } => Some((*player, super::scrolls::Intent::Known { wish: wish.clone() })),
            _ => None,
        }
    }
    pub fn requires_acknowledgement(&self) -> bool {
        matches!(
            self,
            Self::ClaimRealm | Self::ClaimHome | Self::ClaimBrew | Self::ClaimReforge { .. } | Self::ClaimWish { .. } | Self::ClaimVotes { .. }
        )
    }
    pub(crate) fn validate(&self) -> Result<(), String> {
        let player = |id: u32| (1..=super::MAX_PLAYERS).contains(&id);
        let valid = match self {
            Self::Scroll { player: id, intent } => player(*id) && intent.valid(),
            Self::Speculative { event } => event.valid(),
            Self::Catalogue { batch } => batch.valid(),
            Self::PaintCards { updates } => super::card_paint::valid_updates(updates),
            Self::ClaimWish { ask, maker } => super::wish_jobs::valid_ask(ask) && maker.len() <= 1024,
            Self::MirrorName { player: id, words }
            | Self::DropMirrorLook { player: id, words }
            | Self::WishOnPiece { player: id, words } => player(*id) && text(words, 4096),
            Self::BuildVessel { ids, design, .. } => {
                ids.len() <= 64
                    && text(&design.name, 256)
                    && text(&design.says, 4096)
                    && design.traits.len() <= 32
                    && design.quirks.len() <= 32
                    && design.voice.len() <= 64
            }
            Self::RevealVessel => true,
            Self::MirrorPaint { card } => {
                card.id.starts_with("mirror-look-") && super::card_paint::valid_card(card)
            }
            Self::MirrorBody { card, words, on } => {
                card.starts_with("mirror-look-")
                    && text(card, 64)
                    && (text(words, 4096) || (!on && words.is_empty()))
            }
            Self::ClaimReforge {
                player: id, words, ..
            } => player(*id) && text(words, 4096),
            Self::DropQueuedBrew { ask } => {
                super::brew_jobs::valid_outbox(std::slice::from_ref(ask), ask.raid)
            }
            Self::ClaimRealm
            | Self::ClaimHome
            | Self::ClaimBrew
            | Self::SyncHome { .. }
            | Self::Retreat => true,
            Self::Visit { wishes, .. } => wishes
                .as_ref()
                .is_none_or(|wishes| wishes.len() <= crate::realm::MAX_WISHES),
            Self::TakeEntrance { entrance } => text(entrance, 128),
            Self::AdmitSettlement { site } => site.valid(),
            Self::EnterSettlement { site } => text(site, 128),
            Self::BeginDelve { knight, .. } => knights::knight(knight).is_some(),
            Self::ConfigureSession { host, .. } | Self::EndHosting { host, .. } => text(host, 1024),
            Self::JoinSeat {
                player: id,
                name,
                outfit,
            } => {
                player(*id)
                    && text(name, 1024)
                    && match outfit {
                        JoinOutfit::Chosen(knight) => knights::knight(knight).is_some(),
                        _ => true,
                    }
            }
            Self::RemoveSeat { player: id, .. } => player(*id),
            Self::EquipForged {
                player: id, weapon, ..
            } => {
                player(*id)
                    && weapon.as_ref().is_none_or(|weapon| {
                        forge::check(&weapon.runes, forge::START_LOC)
                            .is_ok_and(|checked| checked.weapon == *weapon)
                    })
            }
            Self::MoveCard {
                player: id,
                from,
                to,
            } => player(*id) && *from < 64 && *to < 64,
            Self::CarryBlock { player: id, stone } => player(*id) && super::geology::is_rough(stone),
            Self::Contribute { player: id, made } => player(*id) && made.validate().is_ok(),
            Self::AddCard { player: id, card }
            | Self::Reforge {
                player: id, card, ..
            } => {
                player(*id)
                    && text(&card.id, 128)
                    && text(&card.name, 1024)
                    && card.effects.len() <= 128
            }
            Self::LearnWish { player: id, words } => player(*id) && text(words, 4096),
            Self::KnownWish { player: id, wish } => player(*id) && text(wish, 128),
            Self::PrepareVotes { seats, records, seen, .. } => {
                valid_vote_seats(seats) && valid_vote_facts(records, seen)
            }
            Self::ClaimVotes { records, seen, changes } => {
                valid_vote_facts(records, seen)
                    && changes.len() <= 4096
                    && changes.iter().all(valid_vote_change)
            }
            Self::LeaveActivity { player: id }
            | Self::DropOffer { player: id }
            | Self::OpenWishLine { player: id }
            | Self::TypeWishLetter { player: id, .. }
            | Self::TurnOffer { player: id } => player(*id),
            Self::ChooseOffer { player: id, index } => player(*id) && *index < 3,
            Self::Vote {
                player: id,
                command,
                ..
            } => player(*id) && command.valid(),
            Self::DismissFloorPlate { players } => {
                !players.is_empty()
                    && players.len() <= super::MAX_PLAYERS as usize
                    && players.iter().all(|id| player(*id))
            }
        };
        if valid {
            Ok(())
        } else {
            Err("game command exceeds host port contract".into())
        }
    }
    fn execute(
        &self,
        run: &mut Run,
        baseline: Option<&super::home::Home>,
        pending: Option<&super::home::Home>,
    ) -> Result<Reply, String> {
        match self {
            Self::Speculative { event } => return event.deliver(run).map(Reply::Changed),
            Self::Catalogue { .. } => return Err("catalogue commands require journal admission".into()),
            Self::PaintCards { updates } => return super::card_paint::deliver(run, updates).map(Reply::Changed),
            Self::DropQueuedBrew { ask } => {
                if run.brewing.first() != Some(ask) || super::brew_jobs::Work::capturable(run, ask) {
                    return Err("queued brew is still owned or differs from the cancellation".into());
                }
                run.brewing.remove(0);
            }
            Self::ClaimBrew => return Err("brew claims require journal context".into()),
            Self::ClaimWish { .. } => return Err("wish claims require journal context".into()),
            Self::MirrorName { player, words } => return Ok(Reply::Changed(run.mirror_name(*player, words))),
            Self::WishOnPiece { player, words } => return run.wish_on_piece(*player, words).map(Reply::Chosen),
            Self::BuildVessel { n, ids, design, by_model } => {
                return run.build_vessel(*n, ids, design.clone(), *by_model).map(Reply::Chosen);
            }
            Self::RevealVessel => {
                return Ok(Reply::Changed(run.at_home_now() && run.reveal_vessel().is_some()));
            }
            Self::DropMirrorLook { player, words } => {
                let at = run.asked.iter().position(|a| a.cosmetic && a.knight == *player && a.words == *words);
                if let Some(at) = at {
                    run.asked.remove(at);
                }
                return Ok(Reply::Changed(at.is_some()));
            }
            Self::MirrorPaint { card } => return Ok(Reply::Changed(run.update_mirror_paint(card))),
            Self::MirrorBody { card, words, on } => return Ok(Reply::Changed(run.set_mirror_body(card, words, *on))),
            Self::ClaimReforge { .. } => return Err("reforge claims require journal context".into()),
            Self::ClaimHome => return Ok(Reply::Home(HomeTransfer::capture_and_reserve(run)?)),
            Self::SyncHome { home, treasury } => {
                let projected = projected_home(run, home, baseline, pending);
                let changed = run.home != projected || run.treasury != *treasury;
                if changed {
                    run.rebuild_home(projected, treasury.clone());
                }
                return Ok(Reply::Changed(changed));
            }
            Self::ClaimRealm => {
                let hauls = run.bank.drain(..run.bank.len().min(64)).collect();
                let reclaimed = run.reclaimed.drain(..run.reclaimed.len().min(64)).collect();
                return Ok(Reply::Realm(RealmTransfer {
                    hauls,
                    reclaimed,
                    triumph: run.triumph.take(),
                    names: run
                        .players
                        .iter()
                        .map(|(id, hero)| (*id, hero.name.clone()))
                        .collect(),
                }));
            }
            Self::Retreat => run.retreat(),
            Self::Visit {
                destination,
                wishes,
            } => return Ok(Reply::Visited(visit(run, *destination, wishes.as_ref()))),
            Self::TakeEntrance { entrance } => {
                let entrance = world::ENTRANCES
                    .iter()
                    .find(|item| item.id == entrance)
                    .ok_or("unknown world entrance")?;
                run.take_entrance(entrance);
            }
            Self::AdmitSettlement { site } => site.admit(run)?,
            Self::EnterSettlement { site } => {
                if run.settlement_site.as_ref() != Some(site) {
                    return Err("settlement entrance belongs to another saved site".into());
                }
                let entrance = world::ENTRANCES.iter().find(|entrance| entrance.id == "gate")
                    .ok_or("settlement gate is unavailable")?;
                run.take_entrance(entrance);
            }
            Self::BeginDelve { pack, knight } => {
                let who = knights::knight(knight).ok_or("unknown knight")?;
                run.begin_in(*pack);
                run.outfit(1, who);
                if run.players.contains_key(&2) {
                    let index = knights::COMPANY
                        .iter()
                        .position(|item| item.id == who.id)
                        .unwrap_or(0);
                    run.outfit(2, &knights::COMPANY[(index + 1) % knights::COMPANY.len()]);
                }
            }
            Self::ConfigureSession {
                naked_start,
                identity,
                rite,
                host,
            } => {
                if !run.players.contains_key(&1) {
                    return Err("host knight no longer exists".into());
                }
                run.naked_start = *naked_start;
                run.identity = *identity;
                run.rite = *rite;
                run.rename_host(host);
            }
            Self::JoinSeat {
                player,
                name,
                outfit,
            } => {
                if *player == 1 {
                    run.seat_first(name);
                } else {
                    let new = run
                        .players
                        .get(player)
                        .is_none_or(|hero| hero.knight.is_none());
                    run.join_seat(*player, name);
                    if new && *outfit == JoinOutfit::Company {
                        let mine = run.players.get(&1).and_then(|hero| hero.knight.as_deref());
                        let first = knights::COMPANY
                            .iter()
                            .position(|knight| Some(knight.id) == mine)
                            .unwrap_or(0);
                        run.outfit(
                            *player,
                            &knights::COMPANY
                                [(first + *player as usize - 1) % knights::COMPANY.len()],
                        );
                        return Ok(Reply::Changed(true));
                    }
                }
                if let JoinOutfit::Chosen(knight) = outfit {
                    run.outfit(*player, knights::knight(knight).ok_or("unknown knight")?);
                }
                return Ok(Reply::Changed(false));
            }
            Self::EndHosting {
                host,
                reclaim_first,
            } => {
                let mut names = Vec::new();
                if *reclaim_first
                    && let Some(friend) = run
                        .players
                        .get(&1)
                        .map(|hero| hero.name.clone())
                        .filter(|name| name != host)
                {
                    run.seat_first(host);
                    names.push(friend);
                }
                for player in 2..=super::MAX_PLAYERS {
                    if let Some(hero) = run.players.get(&player) {
                        names.push(hero.name.clone());
                    }
                    run.leave(player);
                }
                return Ok(Reply::Names(names));
            }
            Self::RemoveSeat {
                player,
                retain_first,
            } => {
                let name = run.players.get(player).map(|hero| hero.name.clone());
                if *player != 1 || !retain_first {
                    run.leave(*player);
                }
                return Ok(Reply::Names(name.into_iter().collect()));
            }
            Self::EquipForged {
                player,
                weapon,
                reset_temper,
            } => {
                let hero = run
                    .players
                    .get_mut(player)
                    .ok_or("knight no longer exists")?;
                if weapon.is_some() {
                    hero.arm = None;
                }
                if *reset_temper {
                    hero.temper = 0;
                }
                hero.forged = weapon.clone();
            }
            Self::CarryBlock { player, stone } => {
                run.carry_block(*player, stone)?;
            }
            Self::MoveCard { player, from, to } => {
                run.move_card(*player, *from, *to);
            }
            Self::Contribute { player, made } => {
                return run.contribute(*player, made.clone()).map(Reply::Chosen);
            }
            Self::AddCard { player, card } => {
                if !run.players.contains_key(player) {
                    return Err("knight no longer exists".into());
                }
                return Ok(Reply::Changed(run.add_card(card.clone(), *player)));
            }
            Self::Reforge { player, part, card } => {
                return run.reforge(*player, *part, card.clone()).map(Reply::Chosen);
            }
            Self::Scroll { .. } | Self::LearnWish { .. } | Self::KnownWish { .. } => return Err("scroll commands require prepared policy".into()),
            Self::LeaveActivity { player } => {
                run.leave_activity(*player);
            }
            Self::DropOffer { player } => return Ok(Reply::Changed(run.drop_offer(*player))),
            Self::OpenWishLine { player } => return Ok(Reply::Changed(run.open_wish_line(*player))),
            Self::TypeWishLetter { player, letter } => {
                return Ok(Reply::Changed(run.type_wish_letter(*player, char::from(*letter))));
            }
            Self::ChooseOffer { player, index } => {
                return run.choose(*player, usize::from(*index)).map(Reply::Chosen);
            }
            Self::TurnOffer { player } => return Ok(Reply::Changed(run.turn_wish(*player))),
            Self::Vote {
                player,
                command,
                local_focus,
            } => {
                if *local_focus {
                    run.playtest.local_focus = *player;
                }
                run.vote_command(*player, *command);
            }
            Self::PrepareVotes { seats, records, seen, couch, initialize } => {
                if !valid_vote_seats(seats) {
                    return Err("playtest vote preparation is stale".into());
                }
                let current: Vec<_> = run.players.iter().map(|(&id, hero)| {
                    (id, hero.name.clone(), run.seat_epochs.get(&id).copied().unwrap_or(0))
                }).collect();
                if &current != seats {
                    return Err("playtest seat ownership changed before vote preparation".into());
                }
                if *initialize && !run.playtest.ready {
                    run.playtest.records = records.clone();
                    run.playtest.seen = seen.clone();
                    run.playtest.seats.clear();
                    run.playtest.ready = true;
                    for (id, name, _) in seats {
                        run.playtest.arrive(*id, name);
                    }
                    if *couch && let Some(seat) = run.playtest.seats.get_mut(&2) {
                        seat.couch = true;
                    }
                } else {
                    // `ready` is transient across a private checkpoint. The
                    // Journal marker is durable; restore only the latch and
                    // missing seats, never stale realm facts over the ballot.
                    run.playtest.ready = true;
                    for (id, name, _) in seats {
                        if !run.playtest.seats.contains_key(id) {
                            run.playtest.arrive(*id, name);
                            if *couch && *id == 2 && let Some(seat) = run.playtest.seats.get_mut(id) {
                                seat.couch = true;
                            }
                        }
                    }
                    let recovered = playtest::vote_delta(&run.playtest.records, records);
                    if run.playtest.changes.is_empty() {
                        run.playtest.changes = recovered;
                    }
                    run.playtest.dirty |= run.playtest.records != *records
                        || run.playtest.seen != *seen
                        || !run.playtest.changes.is_empty();
                }
            }
            Self::ClaimVotes { .. } => {
                // The candidate and retained receipt commit together. Later
                // edits own a new queue, even for direct Request admission.
                run.playtest.changes.clear();
                run.playtest.dirty = false;
            }
            Self::DismissFloorPlate { players } => {
                for player in players {
                    run.skip_floor_plate(*player);
                }
            }
        }
        Ok(Reply::Done)
    }
}

fn text(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}
fn valid_vote_seats(seats: &[(u32, String, u64)]) -> bool {
    seats.len() <= super::MAX_PLAYERS as usize
        && seats.iter().all(|(id, name, _)| (1..=super::MAX_PLAYERS).contains(id) && text(name, 1024))
        && seats.windows(2).all(|pair| pair[0].0 < pair[1].0)
}
fn valid_vote_change(change: &votes::Change) -> bool {
    text(&change.player, 1024)
        && text(&change.item, 128)
        && change.rating.is_none_or(|rating| match rating {
            votes::Rating::Stars(stars) => (1..=5).contains(&stars),
            votes::Rating::Untried(votes::Untried::Yes) => true,
        })
}
fn valid_vote_facts(records: &votes::Records, seen: &std::collections::BTreeSet<String>) -> bool {
    records.len() <= 4096
        && records.iter().all(|(player, ratings)| {
            text(player, 1024)
                && ratings.len() <= 128
                && ratings.iter().all(|(item, rating)| {
                    text(item, 128)
                        && match rating {
                            votes::Rating::Stars(stars) => (1..=5).contains(stars),
                            votes::Rating::Untried(votes::Untried::Yes) => true,
                        }
                })
        })
        && seen.len() <= 4096
        && seen.iter().all(|player| text(player, 1024))
}
fn visit(run: &mut Run, destination: Destination, wishes: Option<&Arc<Vec<Wish>>>) -> VisitState {
    if destination == Destination::Keep && run.room().kind == RoomKind::Kingdom {
        return VisitState::Opened;
    }
    if !run.at_home_now() {
        return VisitState::Underground;
    }
    if matches!(destination, Destination::Stables | Destination::Lists) && run.joust.is_some() {
        return VisitState::Busy;
    }
    if let Some(wishes) = wishes {
        run.kingdom.wishes = Arc::clone(wishes);
    }
    let (kind, at) = match destination {
        Destination::Keep => {
            run.walk_kingdom(Some(crate::world::kingdom::keep_stand()));
            return VisitState::Opened;
        }
        Destination::Alehouse => {
            run.walk_alehouse();
            return VisitState::Opened;
        }
        Destination::College => {
            run.raise_home_world();
            (RoomKind::CollegeGrounds, (12.0, 10.5))
        }
        Destination::Stables => (RoomKind::Stables, (18.0, 6.6)),
        Destination::Lists => (RoomKind::Lists, (2.0, 8.5)),
    };
    let Some(index) = world::room_of(&run.dungeon, kind) else {
        return VisitState::Unavailable;
    };
    run.arrive(index, at);
    VisitState::Opened
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(id: u64, command: Command) -> Request {
        Request {
            session: "session-a".into(),
            raid: 9,
            id,
            commands: vec![command],
        }
    }
    fn snapshot(run: &Run) -> serde_json::Value {
        super::super::runtime_state::capture(run)
            .unwrap()
            .into_value()
    }
    #[test]
    fn duplicates_return_original_receipts_after_the_world_changes_and_handoff() {
        let mut run = Run::new(3, 9, None);
        let mut journal = Journal::new("session-a".into(), 9);
        let command = request(1, Command::Retreat);
        let receipt = journal.apply(&mut run, command.clone()).unwrap();
        assert_eq!(receipt.result, Ok(vec![Reply::Done]));
        run.score += 20;
        run.players
            .get_mut(&1)
            .unwrap()
            .carried
            .0
            .insert(crate::realm::Spoil::Gold, 80);
        let before = snapshot(&run);
        let mut journal = Journal::restore(journal.capture().unwrap(), "session-a", 9).unwrap();
        assert_eq!(journal.apply(&mut run, command).unwrap(), receipt);
        assert_eq!(
            snapshot(&run),
            before,
            "duplicate retreat banked new spoils"
        );
        journal
            .apply(&mut run, request(2, Command::Retreat))
            .unwrap();
        assert_ne!(snapshot(&run), before, "fresh retreat did not execute");
    }
    #[test]
    fn rejected_game_commands_have_stable_receipts_without_partial_world_changes() {
        let mut run = Run::new(3, 9, None);
        let mut journal = Journal::new("session-a".into(), 9);
        let before = snapshot(&run);
        let request = request(
            1,
            Command::ChooseOffer {
                player: 1,
                index: 0,
            },
        );
        let receipt = journal.apply(&mut run, request.clone()).unwrap();
        assert!(receipt.result.is_err());
        assert_eq!(snapshot(&run), before);
        run.score = 18;
        assert_eq!(journal.apply(&mut run, request).unwrap(), receipt);
        assert_eq!(run.score, 18);
        assert_eq!(journal.next_id().unwrap(), 2);
    }
    #[test]
    fn a_later_rejection_rolls_back_the_entire_identified_batch() {
        let mut run = Run::new(3, 9, None);
        let mut journal = Journal::new("session-a".into(), 9);
        let before = snapshot(&run);
        let request = Request {
            session: "session-a".into(),
            raid: 9,
            id: 1,
            commands: vec![
                Command::Vote {
                    player: 1,
                    command: votes::Command {
                        serial: 1,
                        action: votes::Action::Toggle,
                    },
                    local_focus: true,
                },
                Command::ChooseOffer {
                    player: 1,
                    index: 0,
                },
            ],
        };
        let receipt = journal.apply(&mut run, request.clone()).unwrap();
        assert!(receipt.result.is_err());
        assert_eq!(
            snapshot(&run),
            before,
            "first command escaped the rejected transaction"
        );
        assert_eq!(journal.apply(&mut run, request).unwrap(), receipt);
    }
    #[test]
    fn fences_reject_wrong_raid_gaps_and_identity_collisions_without_replaying() {
        let mut run = Run::new(3, 9, None);
        let mut journal = Journal::new("session-a".into(), 9);
        assert!(
            journal
                .apply(&mut run, request(2, Command::Retreat))
                .is_err()
        );
        let mut foreign = request(1, Command::Retreat);
        foreign.session = "session-b".into();
        assert!(
            journal
                .apply(&mut run, foreign)
                .unwrap_err()
                .contains("session")
        );
        let mut other = request(1, Command::Retreat);
        other.raid = 10;
        assert!(journal.apply(&mut run, other).is_err());
        journal
            .apply(&mut run, request(1, Command::Retreat))
            .unwrap();
        let before = snapshot(&run);
        assert!(
            journal
                .apply(&mut run, request(1, Command::LeaveActivity { player: 1 }))
                .is_err()
        );
        assert_eq!(snapshot(&run), before);
        journal.acknowledge(9, 1).unwrap();
        for id in 2..=70 {
            journal
                .apply(
                    &mut run,
                    request(id, Command::DismissFloorPlate { players: vec![1] }),
                )
                .unwrap();
            journal.acknowledge(9, id).unwrap();
        }
        let before = snapshot(&run);
        assert!(
            journal
                .apply(&mut run, request(1, Command::Retreat))
                .unwrap_err()
                .contains("retired")
        );
        assert_eq!(snapshot(&run), before);
        assert!(Journal::restore(journal.capture().unwrap(), "session-a", 10).is_err());
        let mut malformed = journal.capture().unwrap();
        malformed["records"] = serde_json::json!([{
            "commands":[{"type":"retreat"}],
            "receipt":{"id":71,"result":{"Ok":[{"type":"done"}]}}
        }]);
        assert!(Journal::restore(malformed, "session-a", 9).is_err());
    }
    #[test]
    fn unacknowledged_results_apply_backpressure_and_survive_handoff() {
        let mut run = Run::new(3, 9, None);
        let mut journal = Journal::new("session-a".into(), 9);
        for id in 1..=64 {
            journal
                .apply(&mut run, request(id, Command::Retreat))
                .unwrap();
        }
        let before = snapshot(&run);
        assert!(
            journal
                .apply(&mut run, request(65, Command::Retreat))
                .unwrap_err()
                .contains("capacity")
        );
        assert_eq!(snapshot(&run), before);
        let mut journal = Journal::restore(journal.capture().unwrap(), "session-a", 9).unwrap();
        assert!(
            journal
                .apply(&mut run, request(1, Command::Retreat))
                .unwrap()
                .result
                .is_ok()
        );
        assert!(journal.acknowledge(10, 32).is_err());
        assert!(journal.acknowledge(9, 65).is_err());
        for id in 1..=32 {
            journal.acknowledge(9, id).unwrap();
        }
        journal.acknowledge(9, 32).unwrap();
        journal
            .apply(&mut run, request(65, Command::Retreat))
            .unwrap();
        assert!(
            journal
                .apply(&mut run, request(1, Command::Retreat))
                .unwrap_err()
                .contains("retired")
        );
        assert!(
            journal
                .apply(&mut run, request(33, Command::Retreat))
                .is_ok()
        );
    }
    #[test]
    fn acknowledging_a_later_result_keeps_older_work_available_after_handoff() {
        let mut run = Run::new(3, 9, None);
        let mut journal = Journal::new("session-a".into(), 9);
        let first = journal
            .apply(&mut run, request(1, Command::Retreat))
            .unwrap();
        journal
            .apply(
                &mut run,
                request(2, Command::DismissFloorPlate { players: vec![1] }),
            )
            .unwrap();
        journal.acknowledge(9, 2).unwrap();
        let mut journal = Journal::restore(journal.capture().unwrap(), "session-a", 9).unwrap();
        assert_eq!(
            journal
                .apply(&mut run, request(1, Command::Retreat))
                .unwrap(),
            first
        );
        assert!(
            journal
                .apply(
                    &mut run,
                    request(2, Command::DismissFloorPlate { players: vec![1] })
                )
                .unwrap_err()
                .contains("retired")
        );
        journal.acknowledge(9, 1).unwrap();
        assert!(
            journal
                .apply(&mut run, request(1, Command::Retreat))
                .unwrap_err()
                .contains("retired")
        );
    }
    #[test]
    fn travel_decisions_keep_combat_in_place_and_move_home_visits_without_renderer_policy() {
        let mut run = Run::new(3, 9, None);
        assert!(!run.at_home_now());
        let mut journal = Journal::new("session-a".into(), 9);
        let before = snapshot(&run);
        let result = journal
            .apply(
                &mut run,
                request(
                    1,
                    Command::Visit {
                        destination: Destination::Keep,
                        wishes: None,
                    },
                ),
            )
            .unwrap();
        assert_eq!(
            result.result,
            Ok(vec![Reply::Visited(VisitState::Underground)])
        );
        assert_eq!(snapshot(&run), before);
        let mut run = Run::at_home(3, 9, None, Default::default(), Default::default());
        let mut journal = Journal::new("session-a".into(), 9);
        journal
            .apply(
                &mut run,
                request(
                    1,
                    Command::Visit {
                        destination: Destination::Keep,
                        wishes: Some(Arc::new(vec![])),
                    },
                ),
            )
            .unwrap()
            .result
            .unwrap();
        assert_eq!(run.room().kind, RoomKind::Kingdom);
        journal
            .apply(
                &mut run,
                request(
                    2,
                    Command::Visit {
                        destination: Destination::College,
                        wishes: None,
                    },
                ),
            )
            .unwrap()
            .result
            .unwrap();
        assert_eq!(run.room().kind, RoomKind::CollegeGrounds);
        let at = run.players[&1].x;
        assert!((at - 12.0 * super::super::TILE_UNITS).abs() < 0.01);
    }
    #[test]
    fn party_handoff_and_checked_equipment_are_atomic_game_intentions() {
        let mut run = Run::new(3, 9, None);
        let mut journal = Journal::new("session-a".into(), 9);
        let weapon = forge::Weapon::starter();
        let setup = Request {
            session: "session-a".into(),
            raid: 9,
            id: 1,
            commands: vec![
                Command::ConfigureSession {
                    naked_start: true,
                    identity: false,
                    rite: false,
                    host: "Owner".into(),
                },
                Command::JoinSeat {
                    player: 2,
                    name: "Friend".into(),
                    outfit: JoinOutfit::Company,
                },
                Command::EquipForged {
                    player: 2,
                    weapon: Some(weapon.clone()),
                    reset_temper: true,
                },
            ],
        };
        journal.apply(&mut run, setup).unwrap().result.unwrap();
        assert_eq!(run.players[&1].name, "Owner");
        assert_eq!(
            run.players[&2].knight.as_deref(),
            Some(knights::COMPANY[1].id)
        );
        assert_eq!(run.players[&2].forged.as_ref(), Some(&weapon));
        journal
            .apply(
                &mut run,
                request(
                    2,
                    Command::JoinSeat {
                        player: 1,
                        name: "Visitor".into(),
                        outfit: JoinOutfit::Keep,
                    },
                ),
            )
            .unwrap()
            .result
            .unwrap();
        let ended = journal
            .apply(
                &mut run,
                request(
                    3,
                    Command::EndHosting {
                        host: "Owner".into(),
                        reclaim_first: true,
                    },
                ),
            )
            .unwrap();
        assert_eq!(
            ended.result,
            Ok(vec![Reply::Names(vec!["Visitor".into(), "Friend".into()])])
        );
        assert_eq!(run.players[&1].name, "Owner");
        assert!(!run.players.contains_key(&2));
        let mut broken = weapon;
        broken.loc += 1;
        let before = snapshot(&run);
        assert!(
            journal
                .apply(
                    &mut run,
                    request(
                        4,
                        Command::EquipForged {
                            player: 1,
                            weapon: Some(broken),
                            reset_temper: true
                        }
                    )
                )
                .unwrap()
                .result
                .is_err()
        );
        assert_eq!(snapshot(&run), before);
    }
    #[test]
    fn retained_payload_bytes_backpressure_before_commit_and_restore_recounts_them() {
        let mut run = Run::new(3, 9, None);
        let mut journal = Journal::new("session-a".into(), 9);
        let before = snapshot(&run);
        let huge = Command::LearnWish {
            player: 1,
            words: "x".repeat(MAX_REQUEST_BYTES + 1),
        };
        assert!(journal.apply(&mut run, request(1, huge)).is_err());
        assert_eq!(journal.next_id().unwrap(), 1);
        let payload = Command::LearnWish {
            player: 1,
            words: "x".repeat(256 * 1024),
        };
        let mut admitted = 0;
        for id in 1..=64 {
            match journal.apply(&mut run, request(id, payload.clone())) {
                Ok(receipt) => {
                    assert!(receipt.result.is_err());
                    admitted = id;
                }
                Err(error) => {
                    assert!(error.contains("capacity"));
                    break;
                }
            }
        }
        assert!(
            admitted > 1 && admitted < 64,
            "count limit replaced byte admission"
        );
        assert_eq!(snapshot(&run), before);
        let mut restored = Journal::restore(journal.capture().unwrap(), "session-a", 9).unwrap();
        assert!(
            restored
                .apply(&mut run, request(admitted + 1, payload))
                .unwrap_err()
                .contains("capacity")
        );
        restored.acknowledge(9, admitted).unwrap();
        restored
            .apply(&mut run, request(admitted + 1, Command::Retreat))
            .unwrap()
            .result
            .unwrap();
    }
    #[test]
    fn bounded_commands_dispatch_votes_and_restore_validates_receipt_sequence() {
        let mut run = Run::new(3, 9, None);
        let mut journal = Journal::new("session-a".into(), 9);
        let command = Command::Vote {
            player: 1,
            command: votes::Command {
                serial: 1,
                action: votes::Action::Toggle,
            },
            local_focus: true,
        };
        journal
            .apply(&mut run, request(1, command))
            .unwrap()
            .result
            .unwrap();
        assert_eq!(run.playtest.local_focus, 1);
        assert!(run.playtest.seats[&1].open);
        let oversized = Command::DismissFloorPlate {
            players: vec![1; 1000],
        };
        assert!(journal.apply(&mut run, request(2, oversized)).is_err());
        assert_eq!(journal.next_id().unwrap(), 2);
        let mut malformed = journal.capture().unwrap();
        malformed["records"][0]["receipt"]["id"] = 8.into();
        assert!(Journal::restore(malformed, "session-a", 9).is_err());
    }
    fn bank(run: &mut Run, gold: u32) {
        let mut spoils = crate::realm::Spoils::default();
        spoils.add(crate::realm::Spoil::Gold, gold);
        run.bank.push(crate::realm::Haul {
            hero: 1,
            spoils,
            why: "claim fixture".into(),
        });
    }
    #[test]
    fn realm_claim_retries_the_same_payload_through_handoff_and_later_command_ack() {
        let mut run = Run::new(3, 9, None);
        let mut journal = Journal::new("session-a".into(), 9);
        bank(&mut run, 19);
        run.reclaimed.push(run.dungeon.pack);
        run.triumph = Some(super::super::Triumph::Dragon);
        let claim = journal.claim_realm(&mut run).unwrap().unwrap();
        assert!(run.bank.is_empty() && run.reclaimed.is_empty() && run.triumph.is_none());
        bank(&mut run, 7);
        assert_eq!(journal.claim_realm(&mut run).unwrap(), Some(claim.clone()));
        let reply = journal
            .apply(&mut run, request(2, Command::LeaveActivity { player: 1 }))
            .unwrap();
        journal.acknowledge(9, reply.id).unwrap();
        let mut restored = Journal::restore(journal.capture().unwrap(), "session-a", 9).unwrap();
        assert_eq!(restored.claim_realm(&mut run).unwrap(), Some(claim.clone()));
        let mut wrong = claim.clone();
        wrong.transfer.hauls.clear();
        assert!(restored.acknowledge_realm(&wrong).is_err());
        assert_eq!(restored.pending_realm().unwrap(), Some(claim.clone()));
        restored.acknowledge_realm(&claim).unwrap();
        restored.acknowledge_realm(&claim).unwrap();
        let next = restored.claim_realm(&mut run).unwrap().unwrap();
        assert_eq!(next.request, 3);
        assert_eq!(
            next.transfer.hauls[0].spoils.get(crate::realm::Spoil::Gold),
            7
        );
        assert_eq!(
            claim.transfer.hauls[0]
                .spoils
                .get(crate::realm::Spoil::Gold),
            19
        );
    }
    #[test]
    fn realm_claims_are_bounded_independent_transactions_with_one_unresolved_identity() {
        let mut run = Run::new(3, 9, None);
        let mut journal = Journal::new("session-a".into(), 9);
        for _ in 0..65 {
            bank(&mut run, 1);
        }
        let mut mixed = request(1, Command::ClaimRealm);
        mixed.commands.push(Command::Retreat);
        assert!(journal.apply(&mut run, mixed).is_err());
        assert_eq!(journal.next_id().unwrap(), 1);
        let first = journal.claim_realm(&mut run).unwrap().unwrap();
        assert_eq!(first.transfer.hauls.len(), 64);
        assert_eq!(run.bank.len(), 1);
        assert!(
            journal
                .apply(&mut run, request(2, Command::ClaimRealm))
                .is_err()
        );
        assert_eq!(run.bank.len(), 1);
        journal.acknowledge_realm(&first).unwrap();
        let last = journal.claim_realm(&mut run).unwrap().unwrap();
        assert_eq!(last.transfer.hauls.len(), 1);
        journal.acknowledge_realm(&last).unwrap();
        assert!(journal.claim_realm(&mut run).unwrap().is_none());
    }
    fn prepared_home(claim: &HomeClaim) -> HomeResult {
        let mut home = super::super::home::Home::default();
        let mut treasury = crate::realm::Spoils::default();
        treasury.add(crate::realm::Spoil::Gold, 100);
        super::super::home_settlement::settle(
            &mut super::super::activity::Realm {
                home: &mut home,
                treasury: &mut treasury,
                reclaimed: &Default::default(),
                raids_won: 0,
            },
            &claim.transfer,
            "Host",
            false,
        )
        .unwrap()
    }
    #[test]
    fn lane_squire_claim_survives_retry_and_charges_once() {
        use super::super::lane_squires::{Action, Building, Order};
        let mut run = Run::at_home(3, 9, None, Default::default(), Default::default());
        let mut journal = Journal::new("session-a".into(), 9);
        run.lane_squire_orders.push(Order::Act("Host".into(), Action::Upgrade(Building::Board)));
        let claim = journal.claim_home(&mut run).unwrap().unwrap();
        assert_eq!(claim.transfer.lane_squire_orders, run.lane_squire_orders);
        let mut restored = Journal::restore(journal.capture().unwrap(), "session-a", 9).unwrap();
        assert_eq!(restored.claim_home(&mut run).unwrap(), Some(claim.clone()));
        let mut home = super::super::home::Home::default();
        let mut treasury = crate::realm::Spoils::default();
        treasury.add(crate::realm::Spoil::Gold, 1000);
        let result = super::super::home_settlement::settle(
            &mut super::super::activity::Realm { home: &mut home, treasury: &mut treasury, reclaimed: &Default::default(), raids_won: 0 },
            &claim.transfer, "Host", false,
        ).unwrap();
        assert_eq!(result.home.lane_squires.board, 1);
        let after = result.treasury.get(crate::realm::Spoil::Gold);
        assert!(after < 1000);
        run.lane_squire_orders.push(Order::Act("Host".into(), Action::Upgrade(Building::Hall)));
        let delivery = restored.complete_home(&mut run, &claim, result.clone()).unwrap();
        assert_eq!(run.lane_squire_orders.len(), 1);
        assert_eq!(run.home.lane_squires.board, 1);
        assert_eq!(run.home.lane_squires.hall, 1);
        assert!(run.treasury.get(crate::realm::Spoil::Gold) < after);
        assert_eq!(restored.complete_home(&mut run, &claim, result).unwrap(), delivery);
        assert_eq!(treasury.get(crate::realm::Spoil::Gold), after);
    }
    fn stock_order(run: &mut Run) {
        run.stall = vec!["potion".into()];
        run.orders.push(super::super::home::Order {
            knight: 1,
            station: super::super::home::Station::StallA,
        });
    }
    #[test]
    fn home_delivery_receipts_survive_handoff_and_later_acks_without_replaying_goods() {
        let mut run = Run::at_home(3, 9, None, Default::default(), Default::default());
        stock_order(&mut run);
        let mut journal = Journal::new("session-a".into(), 9);
        let claim = journal.claim_home(&mut run).unwrap().unwrap();
        assert_eq!(run.stall, [""]);
        assert!(journal.acknowledge(9, claim.request).is_err());
        let ordinary = journal
            .apply(&mut run, request(2, Command::LeaveActivity { player: 1 }))
            .unwrap();
        journal.acknowledge(9, ordinary.id).unwrap();
        let mut restored = Journal::restore(journal.capture().unwrap(), "session-a", 9).unwrap();
        assert_eq!(restored.claim_home(&mut run).unwrap(), Some(claim.clone()));
        let result = prepared_home(&claim);
        let before = run.room().items.len();
        let delivered = restored
            .complete_home(&mut run, &claim, result.clone())
            .unwrap();
        assert!(delivered.refund.is_empty());
        assert_eq!(run.room().items.len(), before + 1);
        let completed_world = snapshot(&run);
        let mut resumed = Journal::restore(restored.capture().unwrap(), "session-a", 9).unwrap();
        assert_eq!(
            resumed.complete_home(&mut run, &claim, result).unwrap(),
            delivered
        );
        assert_eq!(snapshot(&run), completed_world);
        let done = resumed.home_completion(&claim).unwrap().unwrap();
        resumed.acknowledge_home(&claim, &done).unwrap();
        resumed.acknowledge_home(&claim, &done).unwrap();
        assert!(
            resumed.claim_home(&mut run).unwrap().is_none(),
            "global home projection caused an empty-claim loop"
        );
        assert!(
            resumed
                .complete_home(&mut run, &claim, done.result)
                .unwrap_err()
                .contains("retired")
        );
        assert_eq!(snapshot(&run), completed_world);
    }
    #[test]
    fn home_retries_keep_later_facts_and_reject_changed_delivery_results_atomically() {
        let mut run = Run::at_home(3, 9, None, Default::default(), Default::default());
        stock_order(&mut run);
        let mut journal = Journal::new("session-a".into(), 9);
        let claim = journal.claim_home(&mut run).unwrap().unwrap();
        run.marks.insert("xp".into(), 3);
        let result = prepared_home(&claim);
        let mut invalid = result.clone();
        invalid.sales.push((0, 1));
        let before = snapshot(&run);
        assert!(journal.complete_home(&mut run, &claim, invalid).is_err());
        assert_eq!(snapshot(&run), before);
        assert!(journal.home_completion(&claim).unwrap().is_none());
        journal
            .complete_home(&mut run, &claim, result.clone())
            .unwrap();
        let committed = snapshot(&run);
        let mut changed = result;
        changed
            .said
            .push(("changed".into(), "different result".into()));
        assert!(
            journal
                .complete_home(&mut run, &claim, changed)
                .unwrap_err()
                .contains("different content")
        );
        assert_eq!(snapshot(&run), committed);
        let done = journal.home_completion(&claim).unwrap().unwrap();
        journal.acknowledge_home(&claim, &done).unwrap();
        let next = journal.claim_home(&mut run).unwrap().unwrap();
        assert_eq!(next.transfer.marks.get("xp"), Some(&3));
        assert_eq!(next.request, claim.request + 1);
    }
    #[test]
    fn home_refund_outcome_is_retained_after_capacity_changes_and_checkpoint_restore() {
        let mut run = Run::at_home(3, 9, None, Default::default(), Default::default());
        stock_order(&mut run);
        let mut journal = Journal::new("session-a".into(), 9);
        let claim = journal.claim_home(&mut run).unwrap().unwrap();
        let result = prepared_home(&claim);
        let most = if run.open_map() {
            super::super::depths::OPEN_ITEMS
        } else {
            super::super::MAX_ITEMS
        };
        run.dungeon.rooms[run.at].items = (0..most)
            .map(|_| super::super::Item::lying("potion".into(), 4.0, 4.0, 0))
            .collect();
        let delivered = journal
            .complete_home(&mut run, &claim, result.clone())
            .unwrap();
        assert_eq!(
            delivered.refund.get(crate::realm::Spoil::Gold),
            claim.transfer.stall_prices[0]
        );
        run.dungeon.rooms[run.at].items.clear(); // capacity is available on retry
        let before = snapshot(&run);
        let mut restored = Journal::restore(journal.capture().unwrap(), "session-a", 9).unwrap();
        assert_eq!(
            restored.complete_home(&mut run, &claim, result).unwrap(),
            delivered
        );
        assert_eq!(
            snapshot(&run),
            before,
            "a refunded delivery was retried when room became available"
        );
        let mut corrupt = journal.capture().unwrap();
        corrupt["records"][0]["home"]["delivered"]["refund"]["gold"] = 99999.into();
        assert!(Journal::restore(corrupt, "session-a", 9).is_err());
    }
    #[test]
    fn journal_schema_one_migrates_but_cannot_smuggle_home_completion_state() {
        let mut journal = Journal::new("session-a".into(), 9);
        let mut run = Run::new(3, 9, None);
        journal
            .apply(&mut run, request(1, Command::LeaveActivity { player: 1 }))
            .unwrap();
        let mut old = journal.capture().unwrap();
        old["schema"] = 1.into();
        assert_eq!(
            Journal::restore(old, "session-a", 9)
                .unwrap()
                .capture()
                .unwrap()["schema"],
            6
        );
        let mut current = Journal::new("session-a".into(), 9);
        current.claim_home(&mut run).unwrap();
        let mut false_old = current.capture().unwrap();
        false_old["schema"] = 1.into();
        assert!(Journal::restore(false_old, "session-a", 9).is_err());
    }
    #[test]
    fn pending_home_keeps_live_quest_progress_and_consumes_only_its_original_facts() {
        use super::super::home_settlement::HomeOutbox;
        let mut run = Run::at_home(3, 9, None, Default::default(), Default::default());
        stock_order(&mut run);
        run.marks.extend([
            ("xp".into(), 5),
            ("country:visit:0".into(), 1),
            ("country:build:bakery".into(), 1),
            ("college:accept:sigils".into(), 1),
        ]);
        run.feats.push("roadkill");
        let invalid = super::super::merchant::Purchase {
            knight: 999,
            offer: 999,
            shop: super::super::merchant::Shop::Peddler,
        };
        run.peddled.push(invalid);
        let mut journal = Journal::new("session-a".into(), 9);
        let claim = journal.claim_home(&mut run).unwrap().unwrap();
        assert_eq!(run.marks, claim.transfer.marks);
        assert_eq!(run.orders, claim.transfer.orders);
        assert_eq!(run.peddled, [invalid]);
        assert!(run.college_progress().active("sigils"));
        // More play happens while the source/destination cannot yet commit.
        run.marks.insert("xp".into(), 8);
        run.marks.insert("country:visit:1".into(), 1);
        run.marks.insert("country:build:bakery".into(), 2);
        run.orders.push(super::super::home::Order {
            knight: 1,
            station: super::super::home::Station::Forge,
        });
        run.peddled.push(invalid);
        run.feats.push("sticky_fingers");
        let outbox = HomeOutbox::capture(&run).unwrap();
        // A public Run does not contain these transient queues. The private
        // outbox restores the actual world rather than a test-only union.
        let mut restored: Run =
            serde_json::from_value(serde_json::to_value(&run).unwrap()).unwrap();
        assert!(restored.marks.is_empty());
        outbox.install(&mut restored).unwrap();
        assert_eq!(restored.marks, run.marks);
        assert!(restored.college_progress().active("sigils"));
        let mut journal = Journal::restore(journal.capture().unwrap(), "session-a", 9).unwrap();
        journal.validate_home_world(&restored).unwrap();
        let result = prepared_home(&claim);
        journal
            .complete_home(&mut restored, &claim, result.clone())
            .unwrap();
        assert_eq!(restored.marks.get("xp"), Some(&3));
        assert!(!restored.marks.contains_key("country:visit:0"));
        assert_eq!(restored.marks.get("country:visit:1"), Some(&1));
        assert_eq!(restored.marks.get("country:build:bakery"), Some(&2));
        assert!(!restored.marks.contains_key("college:accept:sigils"));
        assert!(restored.college_progress().active("sigils"));
        assert_eq!(restored.orders.len(), 1);
        assert_eq!(
            restored.orders[0].station,
            super::super::home::Station::Forge
        );
        assert_eq!(restored.peddled, [invalid]);
        assert_eq!(restored.feats, ["sticky_fingers"]);
        let completed = snapshot(&restored);
        journal
            .complete_home(&mut restored, &claim, result)
            .unwrap();
        assert_eq!(snapshot(&restored), completed);
    }
    #[test]
    fn an_incompatible_worker_cannot_erase_or_reorder_pending_home_intentions() {
        let mut run = Run::at_home(3, 9, None, Default::default(), Default::default());
        stock_order(&mut run);
        run.marks.insert("xp".into(), 5);
        let mut journal = Journal::new("session-a".into(), 9);
        let claim = journal.claim_home(&mut run).unwrap().unwrap();
        for kind in 0..3 {
            let mut candidate = run.clone();
            match kind {
                0 => candidate.orders.clear(),
                1 => {
                    candidate.orders[0].knight = 2;
                }
                _ => {
                    candidate.marks.insert("xp".into(), 4);
                }
            }
            assert!(journal.validate_home_world(&candidate).is_err());
            let before = snapshot(&candidate);
            assert!(
                journal
                    .complete_home(&mut candidate, &claim, prepared_home(&claim))
                    .is_err()
            );
            assert_eq!(snapshot(&candidate), before);
            assert!(journal.home_completion(&claim).unwrap().is_none());
        }
    }
    #[test]
    fn unchanged_home_projection_does_not_transact_over_a_local_landing_choice() {
        let mut run = Run::at_home(3, 9, None, Default::default(), Default::default());
        run.home.deepest = 4;
        let home = run.home.clone();
        run.home.landing = 2;
        let treasury = run.treasury.clone();
        let mut journal = Journal::new("session-a".into(), 9);
        for _ in 0..3 {
            assert!(
                !journal
                    .sync_home(&mut run, home.clone(), treasury.clone())
                    .unwrap()
            );
            assert_eq!(journal.next_id().unwrap(), 1);
        }
        assert_eq!(run.home.landing, 2);
    }
    #[test]
    fn external_realm_progress_triggers_one_home_claim_without_an_empty_claim_loop() {
        let mut run = Run::at_home(3, 9, None, Default::default(), Default::default());
        let mut journal = Journal::new("session-a".into(), 9);
        let context = HomeContext {
            home: run.home.clone(),
            treasury: run.treasury.clone(),
            reclaimed: Default::default(),
            raids_won: 0,
        };
        let first = journal
            .claim_home_with_context(&mut run, &context)
            .unwrap()
            .unwrap();
        let result = prepared_home(&first);
        journal
            .complete_home(&mut run, &first, result.clone())
            .unwrap();
        let done = journal.home_completion(&first).unwrap().unwrap();
        journal.acknowledge_home(&first, &done).unwrap();
        let mut context = result.context();
        assert!(
            journal
                .claim_home_with_context(&mut run, &context)
                .unwrap()
                .is_none()
        );
        context.raids_won = 7;
        let next = journal
            .claim_home_with_context(&mut run, &context)
            .unwrap()
            .unwrap();
        assert!(next.transfer.orders.is_empty() && next.transfer.marks.is_empty());
        let result = super::super::home_settlement::settle(
            &mut super::super::activity::Realm {
                home: &mut context.home,
                treasury: &mut context.treasury,
                reclaimed: &context.reclaimed,
                raids_won: context.raids_won,
            },
            &next.transfer,
            "Host",
            false,
        )
        .unwrap();
        assert_eq!(result.home.ranch.wins, 7);
        journal
            .complete_home(&mut run, &next, result.clone())
            .unwrap();
        let done = journal.home_completion(&next).unwrap().unwrap();
        journal.acknowledge_home(&next, &done).unwrap();
        let mut resumed = Journal::restore(journal.capture().unwrap(), "session-a", 9).unwrap();
        assert!(
            resumed
                .claim_home_with_context(&mut run, &result.context())
                .unwrap()
                .is_none()
        );
        assert_eq!(resumed.next_id().unwrap(), 3);
    }
    #[test]
    fn syncing_a_prepared_home_result_keeps_edits_made_after_its_claim() {
        let mut run = Run::at_home(3, 9, None, Default::default(), Default::default());
        let mut journal = Journal::new("session-a".into(), 9);
        let first = journal.claim_home(&mut run).unwrap().unwrap();
        journal
            .complete_home(&mut run, &first, prepared_home(&first))
            .unwrap();
        let done = journal.home_completion(&first).unwrap().unwrap();
        journal.acknowledge_home(&first, &done).unwrap();
        run.home.privy_wall = vec!["First edit".into()];
        let pending = journal.claim_home(&mut run).unwrap().unwrap();
        let result = prepared_home(&pending);
        run.home.privy_wall = vec!["Newer edit during persistence".into()];
        journal
            .sync_home(
                &mut run,
                done.result.home.clone(),
                done.result.treasury.clone(),
            )
            .unwrap();
        assert_eq!(run.home.privy_wall, ["Newer edit during persistence"]);
        journal
            .sync_home(&mut run, result.home.clone(), result.treasury.clone())
            .unwrap();
        assert_eq!(run.home.privy_wall, ["Newer edit during persistence"]);
        journal.complete_home(&mut run, &pending, result).unwrap();
        let done = journal.home_completion(&pending).unwrap().unwrap();
        journal.acknowledge_home(&pending, &done).unwrap();
        let next = journal.claim_home(&mut run).unwrap().unwrap();
        assert_eq!(
            next.transfer.view.privy_wall,
            ["Newer edit during persistence"]
        );
    }


}
