//! The host's Delve on the engine: `DelveServer`, the game angel-serve
//! steps, and `Sim`, the cockpit's handle on it.
//!
//! The run lives in `Core`, behind one lock. angel-serve's sim takes it once
//! a tick (`DelveServer::step`): one command a seat, the run's step, the
//! tick's sounds as events, and everything the realm takes in left for the
//! cockpit to settle. Then `publish` makes the tick's snapshot for the
//! cockpit to read (one copy of the run) and its value tree for the
//! writers, built once (`angel_delve::wire`), each seat's share filtered
//! by `wire::seat_tree`.
//!
//! The cockpit only reads the newest snapshot, latched once a frame
//! (`as_ref`). Identified intentions and native resource installation take the
//! core lock between ticks; the next read and publication see the change.
//! Nothing does IO holding the lock: the checkpoint, the feel ledger and
//! the spell cards' files are the service's (`Service`), on a thread of its
//! own. Private checkpoints capture world and command journal together under
//! the core lock; file writes and fsyncs happen after that lock is released.
//!
//! Every seat is a client, the host's own included (`together_seat`): its
//! in-process connection goes through the same seats and writers as a
//! friend's socket. A solo delve listens on loopback only; hosting
//! (`Sim::host`) moves the same run onto a server friends can reach.
//!
//! Without threads (`threaded()` false: the tests), the cockpit steps the
//! same server by hand with its own clock (`angel_serve::Manual`), and runs
//! the service in the same pump, so a test steps exactly as the threads do.

mod brewing;
mod reforging;
mod wishing;
mod painting;
mod catalogue;
mod speculative;
pub use speculative::Pending as Speculative;
mod scrolls;
use scrolls::prepare_scroll;

use crate::together_seat::Seat;
use crate::together_shooter::command::DelveCmd;
use crate::together_shooter::{self as shooter, Input, Run};
use angel_delve::rules::{tune::Tune, cards};
use angel_delve::wire;
use angel_live::GamePolicy;
use angel_net::doc::value::Value;
use angel_net::knob::Knob;
use angel_serve::{
    ClientBundle, DocRep, DocTick, DocView, Events, GameServer, Manual, Presence, Rates, ServeConfig,
    Server,
};
use serde::Serialize;
use std::borrow::Cow;
use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// One fixed step: 30 a second.
pub const STEP: Duration = Duration::from_nanos(1_000_000_000 / shooter::HZ as u64);
/// The spell cards are looked for on disk this often.
const SPELL_SCAN: Duration = Duration::from_millis(500);
/// A checkpoint is written at most this often.
const CHECKPOINT: Duration = Duration::from_secs(3);
/// The service wakes at least this often, for the spell cards.
const SERVICE_WAKE: Duration = Duration::from_millis(250);
/// How often the frame-timing log gets a line of the server's own counts.
const SERVE_STATS: Duration = Duration::from_secs(4);
/// A room the party has just entered holds (`shooter::Hold::Arriving`)
/// until every player here has it on screen, three seconds at most: a page
/// that never shows it stalls nobody for longer.
const ARRIVAL_CAP: u32 = 3 * shooter::HZ;

static THREADED: AtomicBool = AtomicBool::new(false);

/// Validate the entire candidate before it can replace live game rules.
/// Use the engine's own names/ranges; the loader never invents a second set.
pub fn validate_live_rules(policy: &GamePolicy) -> Result<(), String> {
    let knobs = Tune::default().knobs();
    for (name, value) in &policy.knobs {
        let knob = knobs.iter().find(|knob| knob.name == name).ok_or_else(|| {
            format!("unknown Delve rule {name}; /dungeon tune lists available rules")
        })?;
        let fitted = knob
            .fit(*value)
            .ok_or_else(|| format!("invalid Delve rule {name}"))?;
        if (fitted - value).abs() > 0.000001 {
            return Err(format!(
                "{name} must be {}..{}, in steps of {} (nearest {fitted})",
                knob.min, knob.max, knob.step
            ));
        }
    }
    Ok(())
}

/// A cached overlay over normal host tuning. One lock publishes the complete
/// rule set; the simulation never sees half a saved JavaScript generation.
#[derive(Default)]
struct LiveTune {
    policy: Arc<GamePolicy>,
    base: Option<Tune>,
    effective: Tune,
}

impl LiveTune {
    fn replace(&mut self, policy: Arc<GamePolicy>) {
        self.policy = policy;
        self.base = None;
    }

    fn effective(&mut self, base: Tune) -> Tune {
        if self.base != Some(base) {
            self.effective = base;
            let knobs = base.knobs();
            for (name, value) in &self.policy.knobs {
                if let Some(value) = knobs
                    .iter()
                    .find(|knob| knob.name == name)
                    .and_then(|knob| knob.fit(*value))
                {
                    self.effective.set(name, value);
                }
            }
            self.base = Some(base);
        }
        self.effective
    }
}

/// The cockpit turns the threads on at startup; tests step by hand.
pub fn set_threaded(on: bool) {
    THREADED.store(on, Ordering::Relaxed);
}

pub fn threaded() -> bool {
    THREADED.load(Ordering::Relaxed)
}

/// The cockpit's side of the game, set each frame: whether the run plays,
/// whose keys are on it, where it is saved.
#[derive(Clone, Default)]
pub struct Controls {
    /// The game plays (shown and focused, or friends seated).
    pub active: bool,
    /// This terminal's keys are on its seats.
    pub keyboard: bool,
    /// Friends are seated: the host's knight turns to stone while away.
    pub hosted: bool,
    /// A wish is being forged.
    pub wishing: bool,
    /// No friends are hosted: knight 2 is a couch partner.
    pub couch: bool,
    /// The host hosts without a knight of its own (`/dungeon_host --watch`,
    /// a dedicated machine): knight 1's place stands empty until a friend
    /// takes its seat, the last a newcomer is given.
    pub watching: bool,
    /// The workspace's spell cards, hot-loaded while the game runs.
    pub cards: Option<PathBuf>,
    /// Where the checkpoint goes, and the raid serial it records.
    pub save: Option<(PathBuf, u64)>,
    /// The campaign's board the heart questline mirrors
    /// (`together_files::campaign_board`), read by the service.
    pub campaign: Option<PathBuf>,
}

/// One tick of the delve as the cockpit reads it: never changed once
/// published.
pub struct Snapshot {
    pub run: Run,
}

/// What the server tells the cockpit, in the order it happened.
#[derive(Clone, Debug, PartialEq)]
pub enum News {
    /// A player took knight `knight`'s seat (a newcomer: a resume says nothing).
    Joined { knight: u32, name: String },
    /// Knight `knight`'s seat was given up for good: the knight has left
    /// the party (`DelveServer::leave`).
    Left { knight: u32 },
    /// The run is over and the party asked to go back (`rules::again`): the
    /// host starts the next run, as its own R does.
    Again,
    /// A moment for the host's chorus (`Run::cues`).
    Cue(String),
    /// A one-off from a seat (`together_gear`), whole, during raid `raid`.
    Action {
        raid: u64,
        knight: u32,
        kind: u32,
        body: Vec<u8>,
    },
}

/// The sim's own counters, for the frame-timing log and the bench.
#[derive(Clone, Copy, Debug, Default)]
pub struct Telemetry {
    pub ticks: u64,
    /// The last tick's step time, and the worst since the last read.
    pub step_us: u64,
    pub worst_step_us: u64,
    /// The last snapshot's copy and tree time.
    pub publish_us: u64,
}

/// The run, and what the cockpit's edits leave for the next read.
struct Core {
    run: Run,
    commands: angel_delve::rules::host_commands::Journal,
    abilities: crate::together_abilities::Runtime,
    engine: Option<angel_live::system::System>,
    engine_error: Option<String>,
    /// The run changed outside a tick: the next read takes a new snapshot.
    edited: bool,
}

/// What `DelveServer`, the service and the cockpit share.
struct Shared {
    core: Mutex<Core>,
    catalogue: Mutex<catalogue::Host>,
    /// The newest snapshot.
    newest: Mutex<Arc<Snapshot>>,
    /// Serialize checkpoint snapshots with realm settlement. The sim itself
    /// keeps stepping; no filesystem IO holds Core's lock.
    checkpoint: Arc<Mutex<()>>,
    squire_payment: Mutex<Option<shooter::squires_town::CheckpointPayment>>,
    controls: Mutex<Controls>,
    news: Mutex<Vec<News>>,
    /// Events the cockpit adds (the chorus's lines), sent with the next tick.
    said: Mutex<Vec<wire::Event>>,
    /// The feel ledger's lines, for the service to write.
    feel: Mutex<Vec<String>>,
    /// The run has something for the cockpit's realm to settle.
    outbox: AtomicBool,
    telemetry: Mutex<Telemetry>,
    /// Each tick's step and publish times, while the frame-timing log is on.
    timings: Option<Mutex<Vec<(u64, u64)>>>,
    /// The server's counts over the last while (what each client is sent),
    /// for the frame-timing log.
    serve_stats: Mutex<Vec<String>>,
    /// Published ticks, counted, for the service to wake on.
    published: (Mutex<u64>, Condvar),
    /// The hosted session's door and join key, for the checkpoint to
    /// record (`Sim::host`): a restart opens the same link again.
    invitation: Mutex<Option<(String, String)>>,
    stop: AtomicBool,
    /// A friend holds knight 1's seat: the host only watches, behind a door
    /// opened for that (`Sim::host_as`). Said by the server each tick.
    first_is_friends: AtomicBool,
    /// The knobs moved (`angel_delve::rules::tune`), set by the server
    /// between ticks and laid on the run each tick, so a new delve or a
    /// new door keeps them; and where they are kept.
    tune: Mutex<Tune>,
    live_tune: Mutex<LiveTune>,
    tune_file: Mutex<Option<PathBuf>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn delve_system() -> Option<angel_live::system::System> {
    if std::env::var("ANGEL_HOT_SYSTEMS").as_deref() == Ok("0") {
        return None;
    }
    let explicit = std::env::var_os("ANGEL_DELVE_SYSTEM");
    if cfg!(any(test, feature = "testing")) && explicit.is_none() {
        return None;
    }
    let artifact = explicit.clone().map(PathBuf::from).unwrap_or_else(|| {
        crate::cockpit_dir().join("systems/delve/engine")
    });
    // Older installed bundles can still play their embedded engine. A source
    // launcher publishes the worker before starting the cockpit.
    if explicit.is_none() && !artifact.is_file() {
        return None;
    }
    let contracts = BTreeMap::from([
        (angel_delve::rules::scrolls::CONTRACT.into(), angel_delve::rules::scrolls::API),
        (angel_delve::rules::card_paint::CONTRACT.into(), angel_delve::rules::card_paint::API),
        (angel_delve::rules::wish_jobs::CONTRACT.into(), angel_delve::rules::wish_jobs::API),
        (
            angel_delve::rules::home_settlement::POLICY_CONTRACT.into(),
            angel_delve::rules::home_settlement::POLICY_API,
        ),
        (
            angel_delve::rules::brew_jobs::CONTRACT.into(),
            angel_delve::rules::brew_jobs::API,
        ),
        (angel_delve::rules::reforge_jobs::CONTRACT.into(),angel_delve::rules::reforge_jobs::API),
    ]);
    match angel_live::system::System::watch_with_contracts(
        "delve",
        artifact,
        Duration::from_secs(2),
        contracts,
    ) {
        Ok(system) => Some(system),
        Err(error) => {
            tracing::error!(%error, "Delve system supervisor unavailable");
            None
        }
    }
}

fn step_system(core: &mut Core, inputs: &BTreeMap<u32, Input>, step: bool) {
    core.abilities.before_step(&mut core.run);
    let Some(engine) = core.engine.as_mut() else {
        if step {
            core.run.step(inputs);
            core.abilities.after_step(&mut core.run);
        }
        return;
    };
    let result = (|| {
        use angel_delve::rules::{phrasebook, runtime_state};
        let state = runtime_state::capture_for_rules(&core.run)?.into_value();
        let event = serde_json::json!({"inputs":inputs, "settings":angel_delve::settings::get(), "phrasebook":phrasebook::snapshot(), "step":step});
        let validated = std::cell::RefCell::new(None);
        engine.apply(state, event, |state| {
            let run = runtime_state::restore_value(state)?;
            if !run.module_visuals.is_empty() || run.players.values().any(|hero| hero.avatar.is_some()) {
                return Err("rules worker returned host-owned appearance data".into());
            }
            if !angel_delve::wire::drawable(&run) {
                return Err("system produced a non-drawable Run".into());
            }
            core.commands.validate_home_world(&run)?;
            core.commands.validate_brew_world(&run)?;
            core.commands.validate_reforge_handoff(&core.run,&run)?;
            core.commands.validate_wish_handoff(&core.run, &run)?;
            validated.replace(Some(run));
            Ok(())
        })?;
        let mut run = validated.into_inner().ok_or("rules reply was not validated")?;
        runtime_state::retain_appearance(&mut run, &core.run);
        Ok::<Run, String>(run)
    })();
    match result {
        Ok(run) => {
            core.run = run;
            core.engine_error = None;
            if step {
                core.abilities.after_step(&mut core.run);
            }
        }
        Err(error) => {
            if core.engine_error.as_ref() != Some(&error) {
                tracing::warn!(%error, "Delve rules paused; committed state retained");
            }
            core.engine_error = Some(error);
        }
    }
}

impl Shared {
    fn new(run: Run, commands: Option<angel_delve::rules::host_commands::Journal>) -> Shared {
        let first = Arc::new(Snapshot { run: run.clone() });
        Shared {
            core: Mutex::new(Core {
                commands: commands.unwrap_or_else(|| {
                    angel_delve::rules::host_commands::Journal::new(
                        angel_serve::link::random_key(),
                        run.raid_id,
                    )
                }),
                run,
                abilities: Default::default(),
                engine: delve_system(),
                engine_error: None,
                edited: false,
            }),
            catalogue: Mutex::new(Default::default()),
            newest: Mutex::new(first),
            checkpoint: Arc::new(Mutex::new(())),
            squire_payment: Mutex::new(None),
            controls: Mutex::new(Controls::default()),
            news: Mutex::new(Vec::new()),
            said: Mutex::new(Vec::new()),
            feel: Mutex::new(Vec::new()),
            outbox: AtomicBool::new(true),
            telemetry: Mutex::new(Telemetry::default()),
            timings: TickLog::wanted().then(|| Mutex::new(Vec::new())),
            serve_stats: Mutex::new(Vec::new()),
            published: (Mutex::new(0), Condvar::new()),
            invitation: Mutex::new(None),
            stop: AtomicBool::new(false),
            first_is_friends: AtomicBool::new(false),
            tune: Mutex::new(Tune::default()),
            live_tune: Mutex::new(LiveTune::default()),
            tune_file: Mutex::new(None),
        }
    }

    /// The run as it stands, published for the cockpit's reads. The
    /// snapshot is stored under the run's lock, so snapshots are newest in
    /// the order the run changed.
    fn publish(&self, core: &mut Core) -> Arc<Snapshot> {
        // A run no page could draw froze every seat at once on the play box
        // (2026-10-17): it is mended before it goes out, never sent as it is.
        if !angel_delve::wire::drawable(&core.run) {
            let mended = core.run.mend();
            tracing::warn!(?mended, "the run was not drawable; mended before publishing");
        }
        let snapshot = Arc::new(Snapshot {
            run: core.run.clone(),
        });
        core.edited = false;
        if outbox_pending(&snapshot.run) || core.commands.has_pending_realm() || core.commands.has_pending_home() {
            self.outbox.store(true, Ordering::Relaxed);
        }
        *lock(&self.newest) = Arc::clone(&snapshot);
        snapshot
    }

    /// The newest snapshot, publishing the run first if it was edited.
    fn fresh(&self) -> Arc<Snapshot> {
        let mut core = lock(&self.core);
        if core.edited {
            return self.publish(&mut core);
        }
        drop(core);
        Arc::clone(&lock(&self.newest))
    }
}

/// The Delve as angel-serve runs it. Seat `n` plays knight `n + 1`
/// (`wire::knight_of`): the host's own in-process seat is knight 1.
pub struct DelveServer {
    shared: Arc<Shared>,
    seats: u32,
    /// The door was opened for a host who only watches: seat 0, knight 1,
    /// is a friend's (`Sim::host_as`).
    watch: bool,
    /// Each seat's upload in pieces (`together_gear::Uploads`).
    uploads: crate::together_gear::Uploads,
    /// Builds the tick's tree, keeping each shot's id from tick to tick.
    publisher: wire::Publisher,
    /// Each seat's player this tick, as the engine knows it (`presence`):
    /// empty until the engine has said.
    here: Vec<Presence>,
    /// The party's room as of the last tick (raid, floor, room), and the
    /// arrival the room holds for, if it does.
    room: Option<(u64, u32, usize)>,
    arrival: Option<Arrival>,
    /// Each knight whose seat nobody holds, not even for a resume: ticks
    /// so (`UNSEATED_LEAVE`).
    unseated: BTreeMap<u32, u32>,
    /// The raid the party last asked to leave, and the server tick the host
    /// was last told: told again every `AGAIN_RETELL` until it goes, so a
    /// host busy settling the realm the first time still takes the party
    /// home.
    again_for: Option<(u64, u32)>,
}

/// Ticks between telling the host again that the party is due home.
const AGAIN_RETELL: u32 = 2 * angel_delve::rules::HZ;

/// A knight whose seat nobody holds (no player, nor a player's resume:
/// knights a restarted host restored, whose players' seats went with the
/// old server) leaves the party after this long: a minute and a half, for a
/// friend on a slow line to load the page again and take their seat back by
/// name (`Sessions::keep`). A seat given
/// up while this server runs leaves at once (`DelveServer::leave`).
const UNSEATED_LEAVE: u32 = 90 * shooter::HZ;

/// The party came into a room at tick `tick`: it holds until every player
/// here has applied that tick, or until tick `until`.
#[derive(Clone, Copy, Debug)]
struct Arrival {
    tick: u32,
    until: u32,
}

impl DelveServer {
    fn new(shared: Arc<Shared>, seats: u32, watch: bool) -> DelveServer {
        DelveServer {
            shared,
            seats: seats.clamp(1, wire::SEATS),
            watch,
            uploads: Default::default(),
            publisher: Default::default(),
            here: Vec::new(),
            room: None,
            arrival: None,
            unseated: BTreeMap::new(),
            again_for: None,
        }
    }

    /// Knight `knight` leaves the party, told to everyone: never the host's
    /// own place (knight 1), which a host who plays no knight keeps absent.
    fn remove_knight(run: &mut Run, knight: u32, events: &mut Events) {
        if knight == 1 {
            return;
        }
        let Some(name) = run.players.get(&knight).map(|h| h.name.clone()) else {
            return;
        };
        run.leave(knight);
        send(events, &wire::Event::Notice { hear: wire::Hear::All, text: format!("{name} left the party") });
    }

    /// Knights whose seats nobody holds leave once that has lasted
    /// `UNSEATED_LEAVE` (`unseated`). Until the engine has said who is
    /// here, nobody is counted.
    fn sweep_unseated(&mut self, run: &mut Run, events: &mut Events) {
        if self.here.is_empty() {
            return;
        }
        let knights: Vec<u32> = run.players.keys().copied().filter(|&k| k != 1).collect();
        self.unseated.retain(|k, _| knights.contains(k));
        for knight in knights {
            let seated = wire::seat_of(knight)
                .filter(|&seat| seat < self.seats)
                .and_then(|seat| self.here.get(seat as usize))
                .is_some_and(|here| here.seated);
            if seated {
                self.unseated.remove(&knight);
                continue;
            }
            let ticks = self.unseated.entry(knight).or_insert(0);
            *ticks += 1;
            if *ticks >= UNSEATED_LEAVE {
                self.unseated.remove(&knight);
                Self::remove_knight(run, knight, events);
            }
        }
    }

    /// Who is here, knight by knight: a knight whose seat's player is not
    /// playing (still loading, hidden, gone quiet, or nobody's) is away, and
    /// stands as stone (`Run::set_away`); so is the host's own when it
    /// hosts without one. Until the engine has said, nobody is marked.
    fn mark_away(&self, run: &mut Run, controls: &Controls) {
        if self.here.is_empty() {
            return;
        }
        let knights: Vec<u32> = run.players.keys().copied().collect();
        for knight in knights {
            let playing = wire::seat_of(knight)
                .and_then(|seat| self.here.get(seat as usize))
                .is_some_and(|here| here.playing);
            run.set_away(knight, !playing || (knight == 1 && self.host_place_empty(controls)));
        }
    }

    /// A friend holds knight 1's seat (seat 0, the last a newcomer takes),
    /// behind a door opened for a host who only watches.
    fn friend_holds_one(&self) -> bool {
        self.watch && self.here.first().is_some_and(|here| here.seated)
    }

    /// A host who plays no knight keeps knight 1's place empty until a
    /// friend holds its seat.
    fn host_place_empty(&self, controls: &Controls) -> bool {
        controls.watching && !self.friend_holds_one()
    }

    /// The party's room changed (a step, or an edit between ticks): the
    /// new room holds for its arrival from the next tick.
    fn note_room(&mut self, run: &Run, tick: u32) {
        let room = (run.raid_id, run.dungeon.depth, run.at);
        if self.room.is_some_and(|was| was != room) {
            self.arrival = Some(Arrival { tick, until: tick.wrapping_add(ARRIVAL_CAP) });
        }
        self.room = Some(room);
    }

    /// Whether the room holds this tick: while nobody living is here
    /// (`Empty`), and while a player here is yet to see the room the party
    /// just came into (`Arriving`).
    fn hold_for(&mut self, run: &Run, tick: u32) -> shooter::Hold {
        // A fallen or finished run has nothing to hold.
        if !run.active() {
            self.arrival = None;
            return shooter::Hold::Open;
        }
        let living = run.players.values().any(|h| h.hp > 0);
        if living && !run.players.values().any(|h| h.hp > 0 && !h.stone) {
            return shooter::Hold::Empty;
        }
        if let Some(arrival) = self.arrival {
            let waiting = self.here.iter().enumerate().any(|(seat, here)| {
                here.playing
                    && here.applied < arrival.tick
                    && run.players.get(&wire::knight_of(seat as u32)).is_some_and(|h| !h.away)
            });
            if waiting && tick < arrival.until {
                return shooter::Hold::Arriving;
            }
            self.arrival = None;
        }
        shooter::Hold::Open
    }

    fn news(&self, news: News) {
        lock(&self.shared.news).push(news);
    }
}

/// The cockpit's side of the game, as the run shows it: whether knight 2 is
/// a couch partner, whether a wish is being forged, and the host's knight
/// as stone while friends play on without the host (unless `friend_one`:
/// a friend holds knight 1, and its seat says how it stands). True if it
/// changed.
fn apply_controls(run: &mut Run, controls: &Controls, friend_one: bool) -> bool {
    let mut changed = false;
    if let Some(seat) = run.playtest.seats.get_mut(&2)
        && seat.couch != controls.couch
    {
        seat.couch = controls.couch;
        changed = true;
    }
    if run.wishing != controls.wishing {
        run.wishing = controls.wishing;
        changed = true;
    }
    let continuous = run.at_home_now() || run.room().kind == shooter::RoomKind::Kingdom;
    let companion = run.players.get(&2).filter(|h| h.hp > 0).map(|h| (h.x, h.y));
    let outside = run.exits.outside.contains_key(&1);
    // A friend who took a watching host's place plays knight 1 as any
    // friend's knight: stone while nobody plays it (`DelveServer::mark_away`).
    if let Some(host) = run.players.get_mut(&1).filter(|_| !friend_one) {
        let stone = (controls.hosted && !controls.keyboard) || host.vigil || host.away || controls.watching;
        // F6 rejoins a dungeon fight; at home the host keeps their place,
        // including the tavern and bazaar where a party now begins.
        if host.stone
            && !continuous
            && !outside
            && !stone
            && let Some((x, y)) = companion
        {
            host.x = x;
            host.y = y;
        }
        if host.stone != stone {
            host.stone = stone;
            changed = true;
        }
    }
    changed
}

/// Each seat's share of the tick's tree (`wire::seat_tree`).
pub struct SeatView;

impl DocView for SeatView {
    fn view<'a>(&self, tree: &'a Value, seat: Option<u32>) -> Cow<'a, Value> {
        wire::seat_tree(tree, seat)
    }

    /// The kingdom's chunks a few a snapshot (`wire::pace`).
    fn pace<'a>(&self, view: Cow<'a, Value>, held: Option<&Value>) -> Cow<'a, Value> {
        wire::pace(view, held)
    }
}

/// An event for the clients: one knight's to that seat alone, anything
/// else to everyone (each client keeps what is its seat's to hear).
fn send(events: &mut Events, event: &wire::Event) {
    match event.hear() {
        wire::Hear::Knight(knight) => {
            if let Some(seat) = wire::seat_of(knight) {
                events.seat(seat, event.to_vec());
            }
        }
        wire::Hear::Party | wire::Hear::All => events.all(event.to_vec()),
    }
}

impl GameServer for DelveServer {
    type Cmd = DelveCmd;
    type Rep = DocRep<SeatView>;
    const GAME: &'static str = wire::GAME;

    fn rates(&self) -> Rates {
        Rates {
            tick_hz: shooter::HZ,
            snapshot_hz: shooter::HZ,
        }
    }

    fn seats(&self) -> u32 {
        self.seats
    }

    fn params(&self) -> Vec<u8> {
        wire::Params::of(angel_delve::settings::get()).encode()
    }

    fn replicator(&self) -> DocRep<SeatView> {
        DocRep(SeatView)
    }

    fn decode_cmd(bytes: &[u8]) -> Option<DelveCmd> {
        DelveCmd::decode(bytes)
    }

    fn knobs(&self) -> Vec<Knob> {
        lock(&self.shared.tune).knobs()
    }

    fn set_knob(&mut self, name: &str, value: f64) -> bool {
        lock(&self.shared.tune).set(name, value)
    }

    fn join(&mut self, seat: u32, name: &str, _events: &mut Events) {
        self.uploads.clear(seat);
        self.news(News::Joined {
            knight: wire::knight_of(seat),
            name: name.to_string(),
        });
    }

    fn leave(&mut self, seat: u32, events: &mut Events) {
        self.uploads.clear(seat);
        let knight = wire::knight_of(seat);
        Self::remove_knight(&mut lock(&self.shared.core).run, knight, events);
        self.news(News::Left { knight });
    }

    fn action(&mut self, seat: Option<u32>, kind: u32, body: &[u8], _events: &mut Events) {
        // Watchers have no knight to act for.
        let Some(seat) = seat else {
            return;
        };
        let Some((kind, body)) = self.uploads.take(seat, kind, body) else {
            return;
        };
        let raid = lock(&self.shared.core).run.raid_id;
        self.news(News::Action {
            raid,
            knight: wire::knight_of(seat),
            kind,
            body,
        });
    }

    fn input_age(&mut self, seat: u32, ticks: u64) {
        lock(&self.shared.core).run.latch_input_age.insert(wire::knight_of(seat), ticks);
    }

    fn presence(&mut self, _tick: u32, seats: &[Presence]) {
        self.here.clear();
        self.here.extend_from_slice(seats);
    }

    fn step(&mut self, tick: u32, cmds: &[Option<DelveCmd>], events: &mut Events) {
        let started = Instant::now();
        let controls = lock(&self.shared.controls).clone();
        let wishes = crate::together_realm::standing_wishes();
        let mut news = Vec::new();
        let shared = Arc::clone(&self.shared);
        let mut core = lock(&shared.core);
        let run = &mut core.run;
        let friend_one = self.friend_holds_one();
        self.shared.first_is_friends.store(friend_one, Ordering::Relaxed);
        apply_controls(run, &controls, friend_one);
        // The knobs as the server last set them, on whatever run plays.
        let base_tune = *lock(&self.shared.tune);
        let tune = lock(&self.shared.live_tune).effective(base_tune);
        if run.tune != tune {
            run.tune = tune;
        }
        // Who is here, and whether the room holds for the party (`presence`).
        // A host who plays no knight keeps its place absent: no screen shows
        // it and the party is sized without it.
        run.set_absent(1, self.host_place_empty(&controls));
        self.sweep_unseated(run, events);
        self.mark_away(run, &controls);
        self.note_room(run, tick);
        let hold = self.hold_for(run, tick);
        run.hold(hold);
        let inputs: BTreeMap<u32, Input> = cmds
            .iter()
            .enumerate()
            .filter_map(|(seat, cmd)| Some((wire::knight_of(seat as u32), cmd.as_ref()?.input())))
            .collect();
        if controls.active {
            for (&knight, input) in &inputs {
                if let Some(command) = input.vote {
                    run.vote_command(knight, command);
                }
            }
            // The host retains the seats, transport, effects and committed Run.
            // A complete live checkpoint crosses the implementation boundary;
            // only a validated response replaces it. A failed RPC pauses rules
            // instead of executing an input twice or reverting committed effects.
            step_system(&mut core, &inputs, true);
            self.note_room(&mut core.run, tick);
        } else {
            // Paused, nothing steps; leaving an activity is still taken at once.
            for (&knight, input) in &inputs {
                if input.valid() && input.leave_activity {
                    run.leave_activity(knight);
                }
            }
            step_system(&mut core, &inputs, false);
        }
        for voice in core.abilities.media.take() {
            send(events, &wire::Event::Sound {hear:wire::Hear::Party,name:voice.name()});
            events.lease_last(Arc::clone(&voice.sound.file));
        }
        let run = &mut core.run;
        // Back to the kingdom once the run is over (`rules::again`): any
        // player here asks; the party goes when every one has, or when the
        // wait runs out, and the host is told until it goes.
        for (&knight, input) in &inputs {
            if input.valid() && input.again {
                run.ask_again(knight);
            }
        }
        if run.again_due()
            && self.again_for.is_none_or(|(raid, at)| raid != run.raid_id || tick.wrapping_sub(at) >= AGAIN_RETELL)
        {
            self.again_for = Some((run.raid_id, tick));
            news.push(News::Again);
        }
        // What the tick said: cues for the host's chorus, sounds for every
        // seat in the party's room, the feel ledger's lines.
        news.extend(std::mem::take(&mut run.cues).into_iter().map(News::Cue));
        for event in wire::take_sounds(run).into_iter().chain(wire::take_notices(run)) {
            send(events, &event);
        }
        let feel = std::mem::take(&mut run.feel.lines);
        if run.kingdom.wishes != wishes {
            run.kingdom.wishes = wishes;
            run.dungeon.world_revision = run.dungeon.world_revision.wrapping_add(1);
            run.raise_kingdom();
        }
        drop(core);
        for event in std::mem::take(&mut *lock(&self.shared.said)) {
            send(events, &event);
        }
        if !feel.is_empty() {
            lock(&self.shared.feel).extend(feel);
        }
        if !news.is_empty() {
            lock(&self.shared.news).extend(news);
        }
        let took = started.elapsed().as_micros() as u64;
        let mut telemetry = lock(&self.shared.telemetry);
        telemetry.ticks += 1;
        telemetry.step_us = took;
        telemetry.worst_step_us = telemetry.worst_step_us.max(took);
    }

    fn publish(&mut self, _tick: u32) -> DocTick {
        let started = Instant::now();
        let snapshot = self.shared.publish(&mut lock(&self.shared.core));
        // The tree is built from the snapshot, with the run's lock let go.
        let tree = self.publisher.publish(&snapshot.run).unwrap_or(Value::Unit);
        let took = started.elapsed().as_micros() as u64;
        let step_us = {
            let mut telemetry = lock(&self.shared.telemetry);
            telemetry.publish_us = took;
            telemetry.step_us
        };
        if let Some(timings) = &self.shared.timings {
            lock(timings).push((step_us, took));
        }
        let (count, wake) = &self.shared.published;
        *lock(count) += 1;
        wake.notify_all();
        DocTick { tree }
    }
}

/// Work after a tick that never holds the run while it reads or writes a
/// file: the spell cards, the campaign's board, the feel ledger, the
/// checkpoint and the timing log.
#[derive(Default)]
struct Service {
    last_saved: Option<Instant>,
    last_scan: Option<Instant>,
    campaign: crate::together_files::CampaignWatch,
    log: Option<TickLog>,
}

impl Service {
    fn new() -> Service {
        Service {
            log: TickLog::from_env(),
            ..Service::default()
        }
    }

    fn serve(&mut self, shared: &Shared, now: Instant) {
        let controls = lock(&shared.controls).clone();
        if self.last_scan.is_none_or(|at| now.saturating_duration_since(at) >= SPELL_SCAN) {
            self.last_scan = Some(now);
            catalogue::poll(shared, controls.cards.as_deref());
        }
        // The campaign's board, read with the run let go; the run takes its
        // counts under the lock, between ticks, only when they moved. The
        // counts are the board's alone: a run a save brought back, or a new
        // one, has them again within a second.
        if let Some(campaign) = self.campaign.poll(controls.campaign.as_deref(), now) {
            let mut core = lock(&shared.core);
            if core.run.campaign != campaign {
                core.run.campaign = campaign;
                core.edited = true;
            }
        }
        let feel = std::mem::take(&mut *lock(&shared.feel));
        if let Some((path, _)) = &controls.save
            && !feel.is_empty()
        {
            // The feel ledger (ANGEL_DELVE_FEEL=1): a line per knight per
            // room, beside the delve's save.
            use std::io::Write as _;
            let ledger = path.with_file_name("feel.jsonl");
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&ledger)
            {
                for line in &feel {
                    let _ = writeln!(file, "{line}");
                }
            }
        }
        if let Some((path, raid_serial)) = &controls.save
            && self
                .last_saved
                .is_none_or(|last| now.saturating_duration_since(last) >= CHECKPOINT)
        {
            // Take the snapshot *after* acquiring the fence. A snapshot
            // taken while settlement had drained its marks is not a save.
            let _checkpoint = lock(&shared.checkpoint);
            let snapshot = shared.fresh();
            self.last_saved = Some(now);
            if let Err(error) = write_shared_checkpoint(path, *raid_serial, shared) {
                tracing::warn!(%error, "could not save the delve");
            }
        }
        if let (Some(log), Some(timings)) = (self.log.as_mut(), &shared.timings) {
            let ticked = std::mem::take(&mut *lock(timings));
            log.ticked(&ticked);
            for line in std::mem::take(&mut *lock(&shared.serve_stats)) {
                log.line(&format!("serve {line}"));
            }
        }
    }
}

/// The service's thread: after each published tick, and at least every
/// `SERVICE_WAKE`, until the delve ends.
fn service_thread(shared: &Shared) {
    let mut service = Service::new();
    let (count, wake) = &shared.published;
    let mut seen = *lock(count);
    while !shared.stop.load(Ordering::Relaxed) {
        let guard = lock(count);
        let guard = if *guard == seen {
            wake.wait_timeout(guard, SERVICE_WAKE)
                .map_or_else(|p| p.into_inner().0, |(g, _)| g)
        } else {
            guard
        };
        seen = *guard;
        drop(guard);
        service.serve(shared, Instant::now());
    }
}

/// `ANGEL_DUNGEON_FRAME_TIMING=<file>`: every 120 ticks, one `sim` line on
/// the step and the publish (the snapshot copied and the tree built); every
/// few seconds, one `serve` line of the server's counts (`angel_serve::Stats`:
/// the bytes each client is sent, the encode time).
struct TickLog {
    file: std::fs::File,
    step_us: Vec<u64>,
    publish_us: Vec<u64>,
}

impl TickLog {
    fn path() -> Option<std::ffi::OsString> {
        std::env::var_os("ANGEL_DUNGEON_FRAME_TIMING").filter(|p| !p.is_empty())
    }

    fn wanted() -> bool {
        TickLog::path().is_some()
    }

    fn from_env() -> Option<TickLog> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(TickLog::path()?)
            .ok()?;
        Some(TickLog {
            file,
            step_us: Vec::new(),
            publish_us: Vec::new(),
        })
    }

    fn line(&mut self, line: &str) {
        use std::io::Write as _;
        let _ = writeln!(self.file, "{line}");
    }

    fn ticked(&mut self, ticks: &[(u64, u64)]) {
        for &(step, publish) in ticks {
            self.step_us.push(step);
            self.publish_us.push(publish);
            if self.step_us.len() >= 120 {
                let pct = |v: &mut Vec<u64>, p: f64| {
                    v.sort_unstable();
                    let at = ((v.len() as f64 * p) as usize).min(v.len().saturating_sub(1));
                    v.get(at).copied().unwrap_or(0)
                };
                let line = format!(
                    "sim n={} step_us p50={} max={} publish_us p50={} max={}",
                    self.step_us.len(),
                    pct(&mut self.step_us, 0.5),
                    pct(&mut self.step_us, 1.0),
                    pct(&mut self.publish_us, 0.5),
                    pct(&mut self.publish_us, 1.0),
                );
                self.line(&line);
                self.step_us.clear();
                self.publish_us.clear();
            }
        }
    }
}

/// The checkpoint as it is written (`dungeon_session::Saved` reads it back).
#[derive(Serialize)]
struct Saving<'a> {
    version: u32,
    raid_serial: u64,
    run: &'a Run,
    #[serde(skip_serializing_if = "Option::is_none")]
    commands: Option<&'a angel_delve::rules::host_commands::Journal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    home_outbox: Option<angel_delve::rules::home_settlement::HomeOutbox>,
    #[serde(skip_serializing_if = "Option::is_none")]
    brew_outbox: Option<&'a Vec<shooter::cauldron::Brewing>>,
    /// The hosted session's door and join key.
    invitation: Option<(String, String)>,
    // Historical field name: also carries the pending Squires' Yard debit.
    country_intents: BTreeMap<String, u32>,
    /// Ordinary Run saves omit transient triumphs. An unclaimed later victory
    /// must survive beside an older pending bank claim as well.
    realm_triumph: Option<shooter::Triumph>,
}

pub const MAX_GAME_SAVE_BYTES: usize = 16 * 1024 * 1024;
fn encode_checkpoint(
    raid_serial: u64,
    run: &Run,
    invitation: Option<(String, String)>,
    commands: Option<&angel_delve::rules::host_commands::Journal>,
) -> Result<Vec<u8>, String> {
    if let Some(commands) = commands {
        commands.validate_home_world(run)?;
        commands.validate_brew_world(run)?;
        commands.validate_reforge_world(run)?;
        commands.validate_wish_world(run)?;
        angel_delve::rules::scrolls::validate_run(run)?;
        if !angel_delve::rules::brew_jobs::valid_outbox(&run.brewing, run.raid_id) {
            return Err("invalid private brew outbox".into());
        }
    } else if run.players.values().any(|h| h.reforge_job.is_some() || !h.wish_jobs.is_empty()) {
        return Err("asynchronous reservations require their private journal".into());
    } else if run.players.values().any(|h| h.offer.as_ref().is_some_and(|o| o.binding.is_some())) {
        return Err("retained scroll sources require their private journal".into());
    }
    let saving = Saving {
        version: if commands.is_some() { 7 } else { 1 },
        raid_serial,
        run,
        invitation,
        commands,
        home_outbox: commands
            .map(|_| angel_delve::rules::home_settlement::HomeOutbox::capture(run))
            .transpose()?,
        brew_outbox: commands.map(|_| &run.brewing),
        realm_triumph: run.triumph,
        country_intents: run
            .marks
            .iter()
            .filter(|(key, _)| shooter::squires_town::checkpoint_intent_key(key))
            .map(|(key, value)| (key.clone(), *value))
            .collect(),
    };
    let mut saved = serde_json::to_value(&saving).map_err(|e| e.to_string())?;
    shooter::compact_save(&mut saved);
    let bytes = serde_json::to_vec(&saved).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_GAME_SAVE_BYTES {
        return Err("game checkpoint exceeds byte capacity".into());
    }
    Ok(bytes)
}

fn persist_checkpoint(
    path: &Path,
    capture: impl FnOnce() -> Result<Vec<u8>, String>,
) -> Result<(), String> {
    static WRITING: Mutex<()> = Mutex::new(());
    let _writing = lock(&WRITING);
    let bytes = capture()?;
    // Private game state and invitation/receipt identities must round-trip
    // exactly. This payload contains no harness conversation or credentials.
    angel_live::host::workspace_store::write_private_atomic_exact(path, &bytes)
}
use angel_delve::rules::home_settlement as home_claims;
use angel_delve::rules::host_commands::RealmClaim;

/// The game's shared state for the host's settlement, held off the
/// cockpit's frame: claims, their results and checkpoints go through it
/// while the server steps on its own threads.
#[derive(Clone)]
pub struct Link(Arc<Shared>);

impl Link {
    pub fn claim_realm(&self) -> Result<Option<RealmClaim>, String> {
        claim_realm_on(&self.0)
    }
    pub fn acknowledge_realm(&self, claim: &RealmClaim) -> Result<(), String> {
        acknowledge_realm_on(&self.0, claim)
    }
    pub fn sync_home(&self, home: angel_delve::rules::home::Home, treasury: angel_delve::realm::Spoils) -> Result<(), String> {
        sync_home_on(&self.0, home, treasury)
    }
    pub fn claim_home_with_context(&self, context: &home_claims::HomeContext) -> Result<Option<home_claims::HomeClaim>, String> {
        claim_home_on(&self.0, Some(context))
    }
    pub fn home_completion(&self, claim: &home_claims::HomeClaim) -> Result<Option<home_claims::HomeCompletion>, String> {
        home_completion_on(&self.0, claim)
    }
    pub fn evaluate_home(&self, request: &home_claims::HomePolicyRequest) -> Result<home_claims::HomeResult, String> {
        evaluate_home_on(&self.0, request)
    }
    pub fn complete_home(&self, claim: &home_claims::HomeClaim, result: home_claims::HomeResult) -> Result<(), String> {
        complete_home_on(&self.0, claim, result)
    }
    pub fn acknowledge_home(&self, claim: &home_claims::HomeClaim, completed: &home_claims::HomeCompletion) -> Result<(), String> {
        acknowledge_home_on(&self.0, claim, completed)
    }
    pub fn home_facts_pending(&self) -> bool {
        home_facts_on(&self.0)
    }
    /// A checkpoint, serialized with the service's own under the same gate.
    pub fn checkpoint(&self, path: &Path, raid_serial: u64) -> Result<(), String> {
        let _gate = lock(&self.0.checkpoint);
        write_shared_checkpoint(path, raid_serial, &self.0)
    }
}

fn evaluate_home_on(shared: &Shared, request: &home_claims::HomePolicyRequest) -> Result<home_claims::HomeResult, String> {
    use home_claims::{HomeResult, POLICY_API, POLICY_CONTRACT};
    request.validate()?;
    let mut core = lock(&shared.core);
    let Some(engine) = core.engine.as_mut() else {
        return request.evaluate();
    };

        let validated = std::cell::RefCell::new(None);
        engine.compute(
            POLICY_CONTRACT,
            POLICY_API,
            serde_json::to_value(request).map_err(|e| e.to_string())?,
            |value| {
                let result = <HomeResult as serde::Deserialize>::deserialize(value)
                    .map_err(|e| e.to_string())?;
                request.validate_result(&result)?;
                validated.replace(Some(result));
                Ok(())
            },
        )?;
        validated
            .into_inner()
            .ok_or("home policy reply was not validated".into())
}

/// Whether the run holds facts a home claim carries: orders, sales, deeds,
/// marks. With none, a claim can only re-read what the home already holds.
fn home_facts_on(shared: &Shared) -> bool {
    let core = lock(&shared.core);
    let run = &core.run;
    !(run.orders.is_empty()
        && run.lane_squire_orders.is_empty()
        && run.raise_orders.is_empty()
        && run.peddled.is_empty()
        && run.feats.is_empty()
        && run.marks.is_empty())
}

fn claim_realm_on(shared: &Shared) -> Result<Option<RealmClaim>, String> {
    let mut core = lock(&shared.core);
    let Core { run, commands, edited, .. } = &mut *core;
    let had_pending = commands.has_pending_realm();
    let claim = commands.claim_realm(run)?;
    *edited |= !had_pending && claim.is_some();
    Ok(claim)
}

fn acknowledge_realm_on(shared: &Shared, claim: &RealmClaim) -> Result<(), String> {
    let mut core = lock(&shared.core);
    core.commands.acknowledge_realm(claim)?;
    core.edited = true;
    Ok(())
}

fn claim_home_on(shared: &Shared, context: Option<&home_claims::HomeContext>) -> Result<Option<home_claims::HomeClaim>, String> {
    let mut core = lock(&shared.core);
    let Core { run, commands, edited, .. } = &mut *core;
    let had_pending = commands.has_pending_home();
    let claim = match context {
        Some(context) => commands.claim_home_with_context(run, context)?,
        None => commands.claim_home(run)?,
    };
    *edited |= !had_pending && claim.is_some();
    Ok(claim)
}

fn home_completion_on(shared: &Shared, claim: &home_claims::HomeClaim) -> Result<Option<home_claims::HomeCompletion>, String> {
    lock(&shared.core).commands.home_completion(claim)
}

fn complete_home_on(shared: &Shared, claim: &home_claims::HomeClaim, result: home_claims::HomeResult) -> Result<(), String> {
    let mut core = lock(&shared.core);
    let Core { run, commands, edited, .. } = &mut *core;
    commands.complete_home(run, claim, result)?;
    *edited = true;
    Ok(())
}

fn acknowledge_home_on(shared: &Shared, claim: &home_claims::HomeClaim, completed: &home_claims::HomeCompletion) -> Result<(), String> {
    let mut core = lock(&shared.core);
    core.commands.acknowledge_home(claim, completed)?;
    core.edited = true;
    Ok(())
}

fn sync_home_on(shared: &Shared, home: angel_delve::rules::home::Home, treasury: angel_delve::realm::Spoils) -> Result<(), String> {
    let mut core = lock(&shared.core);
    let Core { run, commands, edited, .. } = &mut *core;
    *edited |= commands.sync_home(run, home, treasury)?;
    Ok(())
}

fn write_shared_checkpoint(path: &Path, raid_serial: u64, shared: &Shared) -> Result<(), String> {
    persist_checkpoint(path, || {
        let (run, invitation, commands) = {
            let core = lock(&shared.core);
            (
                core.run.clone(),
                lock(&shared.invitation).clone(),
                core.commands.clone(),
            )
        };
        encode_checkpoint(raid_serial, &run, invitation, Some(&commands))
    })
}
/// Legacy checkpoint fixture, also proving schema-1 restore compatibility.
#[cfg(any(test, feature = "testing"))]
pub fn write_checkpoint(
    path: &Path,
    raid_serial: u64,
    run: &Run,
    invitation: Option<(String, String)>,
) -> Result<(), String> {
    persist_checkpoint(path, || {
        encode_checkpoint(raid_serial, run, invitation, None)
    })
}
/// Whether the run holds something the cockpit's realm takes in: spoils
/// banked, orders placed, marks made, votes cast.
pub fn outbox_pending(run: &Run) -> bool {
    !run.bank.is_empty()
        || !run.reclaimed.is_empty()
        || run.triumph.is_some()
        || !run.orders.is_empty()
        || !run.peddled.is_empty()
        || !run.feats.is_empty()
        || !run.marks.is_empty()
        || run.playtest.dirty
        || !run.playtest.changes.is_empty()
        || !run.playtest.ready
}

/// Where a delve's server listens, and who may join.
#[derive(Clone, Debug)]
pub struct Door {
    pub bind: SocketAddr,
    /// Seats in all, the host's own included.
    pub seats: u32,
    /// A fixed join key (a restored session's); a new one without.
    pub key: Option<String>,
    /// The browser's page and the Delve's wasm.
    pub bundle: Option<ClientBundle>,
}

impl Door {
    /// A solo delve's: loopback only, with nobody told its key.
    fn solo() -> Door {
        Door {
            bind: SocketAddr::from(([127, 0, 0, 1], 0)),
            seats: wire::SEATS,
            key: None,
            bundle: None,
        }
    }

    fn config(&self) -> ServeConfig {
        let mut config = ServeConfig::new(self.bind);
        config.key = self.key.clone();
        config.bundle = self.bundle.clone();
        config
    }
}

/// The HD art pack the pages are served at `/hd/` (docs/LADDER.md, "The
/// browser"), behind `ANGEL_DELVE_HD_WEB=1`: the window's own pack
/// (`ANGEL_GLASS_ART`, else `~/.angelX/art/delve`), for a page opened with
/// `?hd=1` to draw the knights' finer rungs as the window does.
fn hd_web_dir() -> Option<std::path::PathBuf> {
    if !std::env::var("ANGEL_DELVE_HD_WEB").is_ok_and(|v| v == "1") {
        return None;
    }
    std::env::var_os("ANGEL_GLASS_ART")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".angelX").join("art").join("delve")))
        .filter(|dir| dir.join("pack.json").is_file())
}

/// Dev mode (`ANGEL_DELVE_DEV=1`): any seat may set the Delve's knobs from
/// its page's dev panel, not only the host.
fn dev_mode() -> bool {
    std::env::var("ANGEL_DELVE_DEV").is_ok_and(|v| v == "1")
}

/// The server a delve runs on: stepping on its own thread, or by hand.
struct Running {
    server: Server,
    manual: Option<Manual<DelveServer>>,
    hosted: bool,
    /// The server's counts at the last `serve` line, while the frame-timing
    /// log is on.
    counted: Option<(Instant, angel_serve::Totals)>,
}

impl Running {
    /// Every `SERVE_STATS`, the server's counts since the last, for the
    /// service to write to the frame-timing log.
    fn count(&mut self, shared: &Shared, now: Instant) {
        let Some((at, last)) = self.counted.as_mut() else {
            return;
        };
        if now.saturating_duration_since(*at) < SERVE_STATS {
            return;
        }
        let totals = self.server.totals();
        lock(&shared.serve_stats).push(totals.since(last).to_string());
        *at = now;
        *last = totals;
    }
}

struct Live {
    /// This terminal's own seats: the host's knight, and a couch partner's.
    host: Option<Seat>,
    couch: Option<Seat>,
    /// The threads' share. Stepped by hand, the server's own
    /// (`DelveServer::shared`) is the only one, so a test can edit the run
    /// with no lock (`Sim::edit`).
    shared: Option<Arc<Shared>>,
    running: Option<Running>,
    service: Option<JoinHandle<()>>,
    /// The service, run in the pump when stepping by hand.
    inline: Option<Service>,
    /// The snapshot this frame reads, latched once a frame (or after an
    /// edit, on the first read).
    view: OnceCell<Arc<Snapshot>>,
}

impl Live {
    fn start(
        run: Run,
        commands: Option<angel_delve::rules::host_commands::Journal>,
        thread: bool,
        tune_file: Option<PathBuf>,
        policy: Arc<GamePolicy>,
    ) -> Live {
        let mut live = Live {
            host: None,
            couch: None,
            shared: None,
            running: None,
            service: None,
            inline: None,
            view: OnceCell::new(),
        };
        let shared = Shared::new(run, commands);
        lock(&shared.live_tune).replace(policy);
        *lock(&shared.tune_file) = tune_file;
        if let Err(error) = live.serve(Arc::new(shared), thread, Door::solo(), false, false) {
            tracing::warn!(%error, "the delve's server did not start");
        }
        live
    }

    /// Start a server for `shared`'s run behind `door`. On failure the run
    /// stays readable here, and nothing steps it.
    fn serve(
        &mut self,
        shared: Arc<Shared>,
        thread: bool,
        door: Door,
        hosted: bool,
        watch: bool,
    ) -> Result<(), String> {
        shared.stop.store(false, Ordering::Relaxed);
        shared.first_is_friends.store(false, Ordering::Relaxed);
        let game = DelveServer::new(Arc::clone(&shared), door.seats, watch);
        let mut config = door.config();
        config.live_assets = lock(&shared.core).abilities.media.files.clone();
        config.tune_file = lock(&shared.tune_file).clone();
        // The workspace's paintings, beside its knobs (`.angel/dungeon/`),
        // served to the pages and seats that draw them.
        config.paint_dir = config
            .tune_file
            .as_deref()
            .and_then(|tune| tune.parent())
            .map(|dungeon| dungeon.join("paint"));
        config.hd_dir = hd_web_dir();
        config.dev = dev_mode();
        // A restored run's knights keep their seats for their players, who
        // come only through a hosted door: a solo door's seats are this
        // terminal's own, and its couch partner takes knight 2's. A host who
        // plays no knight keeps knight 1's seat too, for the name it bears:
        // newcomers take every other seat first, so the host's place goes
        // to the last friend in.
        if hosted {
            config.names = lock(&shared.core)
                .run
                .players
                .iter()
                .filter(|&(&knight, _)| knight != 1 || watch)
                .filter_map(|(&knight, hero)| wire::seat_of(knight).map(|seat| (seat, hero.name.clone())))
                .collect();
        }
        let started = if thread {
            Server::start(game, config).map(|server| (server, None))
        } else {
            Server::start_manual(game, config).map(|(server, manual)| (server, Some(manual)))
        };
        let (server, manual) = match started {
            Ok(started) => started,
            Err(error) => {
                self.shared = Some(shared);
                return Err(error.to_string());
            }
        };
        if manual.is_some() {
            // The server's own share is the only one now.
            drop(shared);
            self.shared = None;
            self.inline = Some(Service::new());
        } else {
            let service = Arc::clone(&shared);
            self.service = std::thread::Builder::new()
                .name("delve-service".into())
                .spawn(move || service_thread(&service))
                .ok();
            self.shared = Some(shared);
        }
        // The host's own knight is a client like any friend's: seat 0. A
        // host who plays no knight only watches, and seat 0 is a friend's.
        self.host = server
            .connect_local()
            .ok()
            .map(|conn| if watch { Seat::watching(conn, "host") } else { Seat::local(conn, "host") });
        self.couch = None;
        let counted = TickLog::wanted().then(|| (Instant::now(), server.totals()));
        self.running = Some(Running {
            server,
            manual,
            hosted,
            counted,
        });
        Ok(())
    }

    /// The share the cockpit reads and edits through.
    fn shared(&self) -> &Arc<Shared> {
        match (
            &self.shared,
            self.running.as_ref().and_then(|r| r.manual.as_ref()),
        ) {
            (Some(shared), _) => shared,
            (None, Some(manual)) => &manual.game().shared,
            (None, None) => unreachable!("a delve keeps its run while it lives"),
        }
    }

    /// Stop the server and the service; the run stays, in the share handed back.
    fn stop(&mut self) -> Arc<Shared> {
        self.host = None;
        self.couch = None;
        let shared = Arc::clone(self.shared());
        shared.stop.store(true, Ordering::Relaxed);
        shared.published.1.notify_all();
        if let Some(running) = self.running.take() {
            running.server.stop();
        }
        if let Some(thread) = self.service.take() {
            let _ = thread.join();
        }
        self.inline = None;
        self.shared = Some(Arc::clone(&shared));
        shared
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The host's delve, wherever it steps: `None` before one starts. Reads see
/// the snapshot latched for this frame. Production changes go through typed
/// intentions or prepared native resources under the core lock.
#[derive(Default)]
pub struct Sim {
    live: Option<Live>,
    /// This delve's own choice of threads, over the cockpit's (`threaded`).
    thread: Option<bool>,
    /// Where the knobs moved are kept (the workspace's `.angel/dungeon/tune.json`).
    tune_file: Option<PathBuf>,
    live_rules: Arc<GamePolicy>,
    live_abilities: Vec<angel_live::kernel::Resource>,
    live_appearances: Vec<angel_live::kernel::Resource>,
}

/// All appearance resources were decoded before selection; installation only
/// changes native presentation and preserves each hero's gameplay state.
fn install_appearances(run: &mut Run, resources: &[angel_live::kernel::Resource]) -> bool {
    let mut changed = false;
    for resource in resources {
        if let Some(appearance) = resource.binding::<crate::together_appearance::Appearance>() {
            changed |= appearance.install(run);
        }
    }
    changed
}

/// Fixture-only direct edits. Production callers use identified intentions or
/// the native presentation/resource owner.
#[cfg(any(test, feature = "testing"))]
pub struct RunMut<'a> {
    core: MutexGuard<'a, Core>,
}

#[cfg(any(test, feature = "testing"))]
impl std::ops::Deref for RunMut<'_> {
    type Target = Run;
    fn deref(&self) -> &Run {
        &self.core.run
    }
}

#[cfg(any(test, feature = "testing"))]
impl std::ops::DerefMut for RunMut<'_> {
    fn deref_mut(&mut self) -> &mut Run {
        self.core.edited = true;
        &mut self.core.run
    }
}

impl From<Option<Run>> for Sim {
    fn from(run: Option<Run>) -> Sim {
        let mut sim = Sim::default();
        sim.set(run);
        sim
    }
}

impl Sim {
    pub fn evaluate_home(
        &mut self,
        request: &angel_delve::rules::home_settlement::HomePolicyRequest,
    ) -> Result<angel_delve::rules::home_settlement::HomeResult, String> {
        let live = self.live.as_mut().ok_or("no active game session")?;
        evaluate_home_on(live.shared(), request)
    }

    pub fn command(
        &mut self,
        command: angel_delve::rules::host_commands::Command,
    ) -> Result<angel_delve::rules::host_commands::Reply, String> {
        let mut replies = self.commands(vec![command])?;
        if replies.len() != 1 {
            return Err("game command returned an invalid result count".into());
        }
        Ok(replies.remove(0))
    }
    pub fn commands(
        &mut self,
        batch: Vec<angel_delve::rules::host_commands::Command>,
    ) -> Result<Vec<angel_delve::rules::host_commands::Reply>, String> {
        self.commands_with_avatars(batch, Vec::new())
    }
    pub fn commands_with_avatars(
        &mut self,
        batch: Vec<angel_delve::rules::host_commands::Command>,
        avatars: Vec<(u32, Option<angel_delve::realm::Avatar>)>,
    ) -> Result<Vec<angel_delve::rules::host_commands::Reply>, String> {
        if batch.iter().any(angel_delve::rules::host_commands::Command::requires_acknowledgement) {
            return Err("claims require explicit acknowledgement after persistence".into());
        }
        let live = self.live.as_mut().ok_or("no active game session")?;
        live.view.take();
        let mut core = lock(&live.shared().core);
        let Core {
            run,
            commands,
            edited,
            engine,
            ..
        } = &mut *core;
        let request = angel_delve::rules::host_commands::Request {
            session: commands.session().to_owned(),
            raid: run.raid_id,
            id: commands.next_id()?,
            commands: batch,
        };
        let receipt = commands.apply_with_scroll(run, request, |candidate, player, intent| {
            prepare_scroll(engine.as_mut(), candidate, player, intent)
        })?;
        if receipt.result.is_ok() {
            for (player, avatar) in avatars {
                if let Some(hero) = run.players.get_mut(&player) {
                    hero.avatar = avatar;
                }
            }
        }
        commands.acknowledge(run.raid_id, receipt.id)?;
        *edited = true;
        receipt.result
    }
    pub fn pending_home(
        &self,
    ) -> Result<Option<angel_delve::rules::home_settlement::HomeClaim>, String> {
        let Some(live) = self.live.as_ref() else {
            return Ok(None);
        };
        lock(&live.shared().core).commands.pending_home()
    }
    pub fn claim_home(
        &mut self,
    ) -> Result<Option<angel_delve::rules::home_settlement::HomeClaim>, String> {
        self.claim_home_for(None)
    }
    pub fn claim_home_with_context(
        &mut self,
        context: &angel_delve::rules::home_settlement::HomeContext,
    ) -> Result<Option<angel_delve::rules::home_settlement::HomeClaim>, String> {
        self.claim_home_for(Some(context))
    }
    fn claim_home_for(
        &mut self,
        context: Option<&angel_delve::rules::home_settlement::HomeContext>,
    ) -> Result<Option<angel_delve::rules::home_settlement::HomeClaim>, String> {
        let Some(live) = self.live.as_mut() else {
            return Ok(None);
        };
        live.view.take();
        claim_home_on(live.shared(), context)
    }
    pub fn home_completion(
        &self,
        claim: &angel_delve::rules::home_settlement::HomeClaim,
    ) -> Result<Option<angel_delve::rules::home_settlement::HomeCompletion>, String> {
        let live = self.live.as_ref().ok_or("no active game session")?;
        home_completion_on(live.shared(), claim)
    }
    pub fn complete_home(
        &mut self,
        claim: &angel_delve::rules::home_settlement::HomeClaim,
        result: angel_delve::rules::home_settlement::HomeResult,
    ) -> Result<(), String> {
        let live = self.live.as_mut().ok_or("no active game session")?;
        live.view.take();
        complete_home_on(live.shared(), claim, result)
    }
    pub fn acknowledge_home(
        &mut self,
        claim: &angel_delve::rules::home_settlement::HomeClaim,
        completed: &angel_delve::rules::home_settlement::HomeCompletion,
    ) -> Result<(), String> {
        let live = self.live.as_mut().ok_or("no active game session")?;
        acknowledge_home_on(live.shared(), claim, completed)
    }
    pub fn sync_home(
        &mut self,
        home: angel_delve::rules::home::Home,
        treasury: angel_delve::realm::Spoils,
    ) -> Result<(), String> {
        let Some(live) = self.live.as_mut() else {
            return Ok(());
        };
        live.view.take();
        sync_home_on(live.shared(), home, treasury)
    }

    pub fn claim_realm(
        &mut self,
    ) -> Result<Option<angel_delve::rules::host_commands::RealmClaim>, String> {
        let Some(live) = self.live.as_mut() else {
            return Ok(None);
        };
        live.view.take();
        claim_realm_on(live.shared())
    }
    pub fn prepare_votes(
        &mut self,
        records: shooter::votes::Records,
        seen: std::collections::BTreeSet<String>,
        couch: bool,
    ) -> Result<(), String> {
        let Some(live) = self.live.as_mut() else {
            return Ok(());
        };
        live.view.take();
        let mut core = lock(&live.shared().core);
        let Core { run, commands, edited, .. } = &mut *core;
        commands.prepare_votes(run, records, seen, couch)?;
        *edited = true;
        Ok(())
    }
    pub fn playtest_votes_ready(&self) -> bool {
        let Some(live) = self.live.as_ref() else { return false; };
        let core = lock(&live.shared().core);
        core.commands.votes_ready(&core.run)
    }
    pub fn claim_votes(&mut self) -> Result<Option<angel_delve::rules::host_commands::playtest::VoteBatch>, String> {
        let Some(live) = self.live.as_mut() else { return Ok(None); };
        live.view.take();
        let mut core = lock(&live.shared().core);
        let Core { run, commands, edited, .. } = &mut *core;
        let batch = commands.claim_votes(run)?;
        *edited |= batch.is_some();
        Ok(batch)
    }
    pub fn acknowledge_votes(
        &mut self,
        batch: &angel_delve::rules::host_commands::playtest::VoteBatch,
    ) -> Result<(), String> {
        let live = self.live.as_mut().ok_or("no active game session")?;
        live.view.take();
        let mut core = lock(&live.shared().core);
        let Core { run, commands, edited, .. } = &mut *core;
        let result = commands.acknowledge_votes(run, batch);
        *edited = true;
        result
    }
    pub fn acknowledge_realm(
        &mut self,
        claim: &angel_delve::rules::host_commands::RealmClaim,
    ) -> Result<(), String> {
        let live = self.live.as_mut().ok_or("no active game session")?;
        live.view.take();
        acknowledge_realm_on(live.shared(), claim)
    }
    pub fn save_checkpoint(&self, path: &Path, raid_serial: u64) -> Result<(), String> {
        let Some(live) = &self.live else {
            return Ok(());
        };
        write_shared_checkpoint(path, raid_serial, live.shared())
    }

    pub fn set_avatar(
        &mut self,
        player: u32,
        avatar: Option<angel_delve::realm::Avatar>,
    ) -> bool {
        let Some(live) = self.live.as_mut() else {
            return false;
        };
        live.view.take();
        let mut core = lock(&live.shared().core);
        let Some(hero) = core.run.players.get_mut(&player) else {
            return false;
        };
        hero.avatar = avatar;
        core.edited = true;
        true
    }
    pub fn live_voice(&self, name: &str) -> Option<crate::together_media::Voice> {
        let shared = self.live.as_ref()?.shared();
        let mut core = lock(&shared.core);
        core.abilities.media.voice(name)
    }
    pub fn media_origin(&self) -> Option<(String, angel_serve::client::Trust)> {
        let mut addr = self.server()?.addr();
        if addr.ip().is_unspecified() {
            addr.set_ip(if addr.is_ipv4() {
                "127.0.0.1".parse().unwrap()
            } else {
                "::1".parse().unwrap()
            });
        }
        Some((format!("ws://{addr}/ws"), Default::default()))
    }
    pub fn system_status(&self) -> String {
        let Some(live) = &self.live else {
            return "Delve system: no active run".into();
        };
        let core = lock(&live.shared().core);
        let abilities = format!(
            "{} controlled-enemies={}",
            core.abilities.status(),
            core.run
                .enemies
                .iter()
                .filter(|enemy| enemy.hp > 0 && enemy.frozen > 0)
                .count()
        );
        let Some(system) = &core.engine else {
            return format!("Delve system: embedded implementation\n{}", abilities);
        };
        let status = system.status();
        format!(
            "Delve system: generation={} worker={} swaps={} rejected={} raid={} tick={} score={}{}\n{}",
            status
                .active
                .as_deref()
                .map(|s| &s[..s.len().min(12)])
                .unwrap_or("preparing"),
            status
                .worker_pid
                .map(|id| id.to_string())
                .unwrap_or_else(|| "none".into()),
            status.swaps,
            status.rejected,
            core.run.raid_id,
            core.run.tick,
            core.run.score,
            status.error.map(|e| format!("; {e}")).unwrap_or_default(),
            abilities
        )
    }
    pub fn is_some(&self) -> bool {
        self.live.is_some()
    }

    pub fn is_none(&self) -> bool {
        self.live.is_none()
    }

    /// A delve on its own threads whatever the cockpit chose (the tests of
    /// the threads themselves).
    #[cfg(any(test, feature = "testing"))]
    pub fn spawn(run: Run) -> Sim {
        let mut sim = Sim {
            live: None,
            thread: Some(true),
            tune_file: None,
            live_rules: Arc::default(),
            live_abilities: vec![],
            live_appearances: vec![],
        };
        sim.set(Some(run));
        sim
    }

    /// Step the next delve this view starts on threads of its own (or not),
    /// whatever the cockpit chose.
    #[cfg(any(test, feature = "testing"))]
    pub fn prefer_thread(&mut self, on: bool) {
        self.thread = Some(on);
    }

    /// Start a new delve (or end this one with `None`). A new delve while
    /// one runs takes its place on the same server: the seats, the link and
    /// the friends on it stay.
    pub fn set(&mut self, run: Option<Run>) {
        self.set_session(run, None);
    }
    pub fn restore_session(
        &mut self,
        run: Run,
        commands: Option<angel_delve::rules::host_commands::Journal>,
    ) {
        self.set_session(Some(run), commands);
    }
    fn set_session(
        &mut self,
        run: Option<Run>,
        commands: Option<angel_delve::rules::host_commands::Journal>,
    ) {
        if let (Some(run), Some(live)) = (run.as_ref(), self.live.as_mut()) {
            live.view.take();
            let shared = live.shared();
            let mut core = lock(&shared.core);
            core.run = run.clone();
            core.commands = commands.unwrap_or_else(|| {
                angel_delve::rules::host_commands::Journal::new(
                    angel_serve::link::random_key(),
                    run.raid_id,
                )
            });
            let Core { run, abilities, .. } = &mut *core;
            abilities.set_sources(self.live_abilities.clone(), run);
            install_appearances(run, &self.live_appearances);
            core.edited = true;
            shared.outbox.store(true, Ordering::Relaxed);
            return;
        }
        self.live = None;
        let thread = self.thread.unwrap_or_else(threaded);
        let tune_file = self.tune_file.clone();
        let policy = Arc::clone(&self.live_rules);
        self.live = run.map(|run| Live::start(run, commands, thread, tune_file, policy));
        if let Some(live) = &self.live {
            let shared = live.shared();
            let mut core = lock(&shared.core);
            let Core { run, abilities, .. } = &mut *core;
            abilities.set_sources(self.live_abilities.clone(), run);
            install_appearances(run, &self.live_appearances);
            core.edited = true;
        }
    }

    /// Consume one prepared character-resource catalogue. Slots and art become
    /// visible under the same lock; model work and file decoding stay outside.
    pub fn set_live_characters<'a>(
        &mut self,
        sources: impl Iterator<Item = &'a angel_live::kernel::Resource> + Clone,
    ) {
        let abilities = sources.clone().filter(|source| {
            source.contract == crate::together_abilities::CONTRACT
                || source.contract == crate::together_loadouts::CONTRACT
        });
        let abilities_changed = !self.live_abilities.iter().eq(abilities.clone());
        if abilities_changed {
            self.live_abilities = abilities.cloned().collect();
        }
        let appearances = sources.filter(|source| {
            source.contract == crate::together_appearance::CONTRACT
        });
        if !self.live_appearances.iter().eq(appearances.clone()) {
            self.live_appearances = appearances.cloned().collect();
        }
        self.install_live_characters(abilities_changed);
    }

    fn install_live_characters(&mut self, abilities_changed: bool) {
        let Some(live) = self.live.as_mut() else { return; };
        let changed = {
            let mut core = lock(&live.shared().core);
            let Core { run, abilities, .. } = &mut *core;
            if abilities_changed {
                abilities.set_sources(self.live_abilities.clone(), run);
            }
            let changed = install_appearances(run, &self.live_appearances) | abilities_changed;
            core.edited |= changed;
            changed
        };
        if changed { live.view.take(); }
    }

    pub fn set_live_rules(&mut self, policy: Arc<GamePolicy>) {
        if Arc::ptr_eq(&self.live_rules, &policy) {
            return;
        }
        if let Some(live) = &self.live {
            lock(&live.shared().live_tune).replace(Arc::clone(&policy));
        }
        self.live_rules = policy;
    }

    /// Keep the knobs moved in `path`, and start every server from it.
    pub fn set_tune_file(&mut self, path: PathBuf) {
        if let Some(live) = &self.live {
            *lock(&live.shared().tune_file) = Some(path.clone());
        }
        self.tune_file = Some(path);
    }

    /// The server's knobs, where they stand (`/dungeon tune`).
    pub fn knobs(&self) -> Vec<Knob> {
        let mut knobs = self.server().map(Server::knobs).unwrap_or_default();
        for knob in &mut knobs {
            if let Some(value) = self.live_rules.knobs.get(knob.name) {
                knob.value = *value;
            }
        }
        knobs
    }

    pub fn live_rule(&self, name: &str) -> Option<f64> {
        self.live_rules.knobs.get(name).copied()
    }

    /// Set a server knob as the host (`/dungeon tune name value`): the value
    /// it takes, or why not.
    pub fn tune(&self, name: &str, value: f64) -> Result<f64, String> {
        self.server().ok_or_else(|| "no delve is running".to_string())?.tune(name, value)
    }

    /// The caller may hold this independent fence across realm IO without
    /// borrowing Sim or stopping its ticks. Acquire it before taking Core.
    /// Whether the run holds facts for a home claim (`home_facts_on`).
    pub fn home_facts_pending(&self) -> bool {
        self.live.as_ref().is_some_and(|live| home_facts_on(live.shared()))
    }

    /// The game's shared state for host work off the cockpit's frame, while
    /// the server steps on its own threads.
    pub fn link(&self) -> Option<Link> {
        if !self.threaded() {
            return None;
        }
        self.live.as_ref().map(|live| Link(Arc::clone(live.shared())))
    }

    pub fn checkpoint_gate(&self) -> Option<Arc<Mutex<()>>> {
        self.live.as_ref().map(|live| Arc::clone(&live.shared().checkpoint))
    }

    pub(crate) fn squire_payment(&self) -> Option<shooter::squires_town::CheckpointPayment> {
        self.live.as_ref().and_then(|live| *lock(&live.shared().squire_payment))
    }

    pub(crate) fn set_squire_payment(
        &self,
        payment: Option<shooter::squires_town::CheckpointPayment>,
    ) {
        if let Some(live) = &self.live {
            *lock(&live.shared().squire_payment) = payment;
        }
    }

    /// The run as this frame reads it.
    pub fn as_ref(&self) -> Option<&Run> {
        self.snapshot().map(|s| &s.run)
    }

    /// The snapshot this frame reads.
    pub fn snapshot(&self) -> Option<&Arc<Snapshot>> {
        let live = self.live.as_ref()?;
        Some(live.view.get_or_init(|| live.shared().fresh()))
    }

    /// Direct mutable access is restricted to test fixtures.
    #[cfg(any(test, feature = "testing"))]
    pub fn as_mut(&mut self) -> Option<RunMut<'_>> {
        let live = self.live.as_mut()?;
        live.view.take();
        Some(RunMut {
            core: lock(&live.shared().core),
        })
    }

    /// The run itself, for a test: stepped by hand, the server's is the
    /// only share, so an edit needs no lock.
    #[cfg(any(test, feature = "testing"))]
    pub fn edit(&mut self) -> Option<&mut Run> {
        let live = self.live.as_mut()?;
        live.view.take();
        let manual = live
            .running
            .as_mut()
            .and_then(|r| r.manual.as_mut())
            .expect("tests edit a delve stepped by hand");
        let shared = Arc::get_mut(&mut manual.game_mut().shared)
            .expect("tests edit a delve stepped by hand");
        let core = shared.core.get_mut().unwrap_or_else(|p| p.into_inner());
        core.edited = true;
        Some(&mut core.run)
    }

    /// Look for spell cards on the next pump, however recently it looked.
    #[cfg(any(test, feature = "testing"))]
    pub fn rescan_spells(&mut self) {
        if let Some(service) = self.live.as_mut().and_then(|l| l.inline.as_mut()) {
            service.last_scan = None;
        }
    }

    /// Read the newest snapshot for this frame.
    pub fn latch(&mut self) {
        if let Some(live) = self.live.as_mut() {
            live.view.take();
        }
    }

    /// The cockpit's side of the game, for the next tick. What it shows on
    /// the run (the host's knight as stone, a couch partner, a wish being
    /// forged) is seen by the next read, as an edit's is.
    pub fn set_controls(&mut self, controls: Controls) {
        let Some(live) = self.live.as_mut() else {
            return;
        };
        let shared = live.shared();
        let mut core = lock(&shared.core);
        if apply_controls(&mut core.run, &controls, shared.first_is_friends.load(Ordering::Relaxed)) {
            core.edited = true;
            drop(core);
            live.view.take();
        } else {
            drop(core);
        }
        *lock(&live.shared().controls) = controls;
    }

    /// Stepping by hand, run the clock to `now` here, then the service:
    /// how many ticks it stepped. (Either way, the server's counts go to
    /// the frame-timing log when it is on.)
    pub fn pump(&mut self, now: Instant) -> u32 {
        let Some(live) = self.live.as_mut() else {
            return 0;
        };
        let shared = Arc::clone(live.shared());
        if let Some(running) = live.running.as_mut() {
            running.count(&shared, now);
        }
        let Some(manual) = live.running.as_mut().and_then(|r| r.manual.as_mut()) else {
            return 0;
        };
        let stepped = manual.run_to(now);
        if let Some(service) = live.inline.as_mut() {
            service.serve(&shared, now);
        }
        stepped
    }

    /// Whether the delve steps on threads of its own.
    pub fn threaded(&self) -> bool {
        self.live
            .as_ref()
            .and_then(|l| l.running.as_ref())
            .is_some_and(|r| r.manual.is_none())
    }

    /// What the server said since the last look.
    pub fn drain_news(&self) -> Vec<News> {
        self.live
            .as_ref()
            .map(|live| std::mem::take(&mut *lock(&live.shared().news)))
            .unwrap_or_default()
    }

    /// A line the host's chorus speaks, for every seat to hear.
    pub fn say(&self, event: wire::Event) {
        if let Some(live) = &self.live {
            lock(&live.shared().said).push(event);
        }
    }

    /// Whether the run has something for the realm, since the last look.
    pub fn take_outbox(&self) -> bool {
        self.live
            .as_ref()
            .is_some_and(|live| live.shared().outbox.swap(false, Ordering::Relaxed))
    }

    /// When the clock's next step is due, stepping by hand.
    pub fn clock(&self) -> Option<Instant> {
        let live = self.live.as_ref()?;
        let next = live.running.as_ref()?.manual.as_ref()?.next_due()?;
        Some(next - STEP)
    }

    /// The server this delve runs on.
    pub fn server(&self) -> Option<&Server> {
        Some(&self.live.as_ref()?.running.as_ref()?.server)
    }

    /// Whether the server is one friends can reach (`host`).
    #[cfg(any(test, feature = "testing"))]
    pub fn hosted(&self) -> bool {
        self.live
            .as_ref()
            .and_then(|l| l.running.as_ref())
            .is_some_and(|r| r.hosted)
    }

    /// This terminal's seat for `knight`: the host's own (1), or a couch
    /// partner's (2).
    pub fn seat(&self, knight: u32) -> Option<&Seat> {
        let live = self.live.as_ref()?;
        match knight {
            1 => live.host.as_ref(),
            2 => live.couch.as_ref(),
            _ => None,
        }
    }

    /// This terminal's seats, the host's first.
    pub fn seats(&self) -> impl Iterator<Item = &Seat> {
        self.live
            .iter()
            .flat_map(|live| live.host.iter().chain(live.couch.iter()))
    }

    /// A couch partner plays knight 2 on this keyboard (or no longer does):
    /// their seat is a client of its own, beside the host's.
    pub fn set_couch(&mut self, on: bool) {
        let Some(live) = self.live.as_mut() else {
            return;
        };
        if !on {
            if let Some(couch) = live.couch.take() {
                couch.bye();
            }
            return;
        }
        if live.couch.is_none()
            && let Some(running) = &live.running
            && let Ok(conn) = running.server.connect_local()
        {
            live.couch = Some(Seat::local(conn, "couch"));
        }
    }

    /// Move this delve onto a server behind `door`, for friends to join:
    /// the same run, a new listener. The host's own seats must dial the new
    /// server. When the door cannot be opened, the delve stays as it was.
    #[cfg_attr(not(test), allow(dead_code))] // a host who also plays; a watcher uses `host_as`
    pub fn host(&mut self, door: Door) -> Result<(), String> {
        self.host_as(door, false)
    }

    /// `host`, for a host who plays no knight when `watch`: its own seat
    /// only watches, and every seat behind the door is a friend's.
    pub fn host_as(&mut self, door: Door, watch: bool) -> Result<(), String> {
        let thread = self.thread.unwrap_or_else(threaded);
        let Some(live) = self.live.as_mut() else {
            return Err("no delve is running".into());
        };
        // The new door is tried before the old server stops, so one that
        // cannot be bound leaves the delve where it was.
        let probe = std::net::TcpListener::bind(door.bind)
            .map_err(|e| format!("cannot host the delve: {e}"))?;
        let bind = probe.local_addr().map_err(|e| e.to_string())?;
        drop(probe);
        let door = Door { bind, ..door };
        let shared = live.stop();
        live.view.take();
        if let Err(error) = live.serve(Arc::clone(&shared), thread, door, true, watch) {
            // The run goes back to a solo door rather than standing still.
            if let Err(again) = live.serve(shared, thread, Door::solo(), false, false) {
                tracing::warn!(error = %again, "the delve's server did not start again");
            }
            return Err(format!("cannot host the delve: {error}"));
        }
        drop(shared);
        self.remember_invitation();
        Ok(())
    }

    /// Back to a solo door: the run moves onto a server on loopback, and
    /// the link no longer works.
    pub fn unhost(&mut self) -> Result<(), String> {
        let thread = self.thread.unwrap_or_else(threaded);
        let Some(live) = self.live.as_mut() else {
            return Err("no delve is running".into());
        };
        let shared = live.stop();
        live.view.take();
        *lock(&shared.invitation) = None;
        live.serve(shared, thread, Door::solo(), false, false)
    }

    /// Record the hosted session's door and key again (a new key was made).
    pub fn remember_invitation(&self) {
        let Some(live) = &self.live else {
            return;
        };
        if live.running.as_ref().is_some_and(|r| r.hosted)
            && let Some(server) = self.server()
        {
            *lock(&live.shared().invitation) = Some((server.addr().to_string(), server.key()));
        }
    }

    /// The hosted session's door and join key, as the checkpoint records them.
    pub fn invitation(&self) -> Option<(String, String)> {
        lock(&self.live.as_ref()?.shared().invitation).clone()
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn telemetry(&self) -> Telemetry {
        self.live.as_ref().map_or_else(Telemetry::default, |live| {
            let mut telemetry = lock(&live.shared().telemetry);
            let out = *telemetry;
            telemetry.worst_step_us = 0;
            out
        })
    }
}

#[cfg(test)]
#[path = "../tests/together_sim__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/together_sim__bench.rs"]
mod bench;

#[cfg(test)]
#[path = "../tests/together_sim__presence.rs"]
mod presence_tests;
