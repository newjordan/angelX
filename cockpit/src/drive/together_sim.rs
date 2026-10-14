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
//! (`as_ref`). An edit (`as_mut`) takes the run under the lock, between
//! ticks; the next read sees it, and the next tick sends it to everyone.
//! Nothing does IO holding the lock: the checkpoint, the feel ledger and
//! the spell cards' files are the service's (`Service`), on a thread of its
//! own, from a published snapshot.
//!
//! Every seat is a client, the host's own included (`together_seat`): its
//! in-process connection goes through the same seats and writers as a
//! friend's socket. A solo delve listens on loopback only; hosting
//! (`Sim::host`) moves the same run onto a server friends can reach.
//!
//! Without threads (`threaded()` false: the tests), the cockpit steps the
//! same server by hand with its own clock (`angel_serve::Manual`), and runs
//! the service in the same pump, so a test steps exactly as the threads do.

use crate::drive::together_seat::Seat;
use crate::drive::together_shooter::command::DelveCmd;
use crate::drive::together_shooter::{self as shooter, Input, Run};
use angel_delve::rules::tune::Tune;
use angel_delve::wire;
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
pub(crate) const STEP: Duration = Duration::from_nanos(1_000_000_000 / shooter::HZ as u64);
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

/// The cockpit turns the threads on at startup; tests step by hand.
pub(crate) fn set_threaded(on: bool) {
    THREADED.store(on, Ordering::Relaxed);
}

pub(crate) fn threaded() -> bool {
    THREADED.load(Ordering::Relaxed)
}

/// The cockpit's side of the game, set each frame: whether the run plays,
/// whose keys are on it, where it is saved.
#[derive(Clone, Default)]
pub(crate) struct Controls {
    /// The game plays (shown and focused, or friends seated).
    pub(crate) active: bool,
    /// This terminal's keys are on its seats.
    pub(crate) keyboard: bool,
    /// Friends are seated: the host's knight turns to stone while away.
    pub(crate) hosted: bool,
    /// A wish is being forged.
    pub(crate) wishing: bool,
    /// No friends are hosted: knight 2 is a couch partner.
    pub(crate) couch: bool,
    /// The host hosts without a knight of its own (`/dungeon_host --watch`,
    /// a dedicated machine): knight 1 stands as stone with the party.
    pub(crate) watching: bool,
    /// The workspace's spell cards, hot-loaded while the game runs.
    pub(crate) cards: Option<PathBuf>,
    /// Where the checkpoint goes, and the raid serial it records.
    pub(crate) save: Option<(PathBuf, u64)>,
}

/// One tick of the delve as the cockpit reads it: never changed once
/// published.
pub(crate) struct Snapshot {
    pub(crate) run: Run,
}

/// What the server tells the cockpit, in the order it happened.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum News {
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
pub(crate) struct Telemetry {
    pub(crate) ticks: u64,
    /// The last tick's step time, and the worst since the last read.
    pub(crate) step_us: u64,
    pub(crate) worst_step_us: u64,
    /// The last snapshot's copy and tree time.
    pub(crate) publish_us: u64,
}

/// The run, and what the cockpit's edits leave for the next read.
struct Core {
    run: Run,
    /// The run changed outside a tick: the next read takes a new snapshot.
    edited: bool,
}

/// What `DelveServer`, the service and the cockpit share.
struct Shared {
    core: Mutex<Core>,
    /// The newest snapshot.
    newest: Mutex<Arc<Snapshot>>,
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
    /// The knobs moved (`angel_delve::rules::tune`), set by the server
    /// between ticks and laid on the run each tick, so a new delve or a
    /// new door keeps them; and where they are kept.
    tune: Mutex<Tune>,
    tune_file: Mutex<Option<PathBuf>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Shared {
    fn new(run: Run) -> Shared {
        let first = Arc::new(Snapshot { run: run.clone() });
        Shared {
            core: Mutex::new(Core { run, edited: false }),
            newest: Mutex::new(first),
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
            tune: Mutex::new(Tune::default()),
            tune_file: Mutex::new(None),
        }
    }

    /// The run as it stands, published for the cockpit's reads. The
    /// snapshot is stored under the run's lock, so snapshots are newest in
    /// the order the run changed.
    fn publish(&self, core: &mut Core) -> Arc<Snapshot> {
        let snapshot = Arc::new(Snapshot {
            run: core.run.clone(),
        });
        core.edited = false;
        if outbox_pending(&snapshot.run) {
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
pub(crate) struct DelveServer {
    shared: Arc<Shared>,
    seats: u32,
    /// Each seat's upload in pieces (`together_gear::Uploads`).
    uploads: crate::drive::together_gear::Uploads,
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
    /// The raid the party last asked to leave, told the host once.
    again_for: Option<u64>,
}

/// A knight whose seat nobody holds (no player, nor a player's resume:
/// knights a restarted host restored, whose players' seats went with the
/// old server) leaves the party after this long: ten seconds. A seat given
/// up while this server runs leaves at once (`DelveServer::leave`).
const UNSEATED_LEAVE: u32 = 10 * shooter::HZ;

/// The party came into a room at tick `tick`: it holds until every player
/// here has applied that tick, or until tick `until`.
#[derive(Clone, Copy, Debug)]
struct Arrival {
    tick: u32,
    until: u32,
}

impl DelveServer {
    fn new(shared: Arc<Shared>, seats: u32) -> DelveServer {
        DelveServer {
            shared,
            seats: seats.clamp(1, wire::SEATS),
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
            run.set_away(knight, !playing || (knight == 1 && controls.watching));
        }
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
/// as stone while friends play on without the host. True if it changed.
fn apply_controls(run: &mut Run, controls: &Controls) -> bool {
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
    let continuous = run.room().kind == shooter::RoomKind::Kingdom;
    let companion = run.players.get(&2).filter(|h| h.hp > 0).map(|h| (h.x, h.y));
    let outside = run.exits.outside.contains_key(&1);
    if let Some(host) = run.players.get_mut(&1) {
        let stone = (controls.hosted && !controls.keyboard) || host.vigil || host.away || controls.watching;
        // F6 returns the host to the party, not an abandoned corner.
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
pub(crate) struct SeatView;

impl DocView for SeatView {
    fn view<'a>(&self, tree: &'a Value, seat: Option<u32>) -> Cow<'a, Value> {
        wire::seat_tree(tree, seat)
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

    fn presence(&mut self, _tick: u32, seats: &[Presence]) {
        self.here.clear();
        self.here.extend_from_slice(seats);
    }

    fn step(&mut self, tick: u32, cmds: &[Option<DelveCmd>], events: &mut Events) {
        let started = Instant::now();
        let controls = lock(&self.shared.controls).clone();
        let wishes = crate::drive::together_realm::standing_wishes();
        let mut news = Vec::new();
        let shared = Arc::clone(&self.shared);
        let mut core = lock(&shared.core);
        let run = &mut core.run;
        apply_controls(run, &controls);
        // The knobs as the server last set them, on whatever run plays.
        let tune = *lock(&self.shared.tune);
        if run.tune != tune {
            run.tune = tune;
        }
        // Who is here, and whether the room holds for the party (`presence`).
        // A host who plays no knight keeps its place absent: no screen shows
        // it and the party is sized without it.
        run.set_absent(1, controls.watching);
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
            run.step(&inputs);
            self.note_room(run, tick);
        } else {
            // Paused, nothing steps; leaving an activity is still taken at once.
            for (&knight, input) in &inputs {
                if input.valid() && input.leave_activity {
                    run.leave_activity(knight);
                }
            }
        }
        let run = &mut core.run;
        // Back to the kingdom once the run is over (`rules::again`): any
        // player here asks; the party goes when every one has, or when the
        // wait runs out, and the host is told once.
        for (&knight, input) in &inputs {
            if input.valid() && input.again {
                run.ask_again(knight);
            }
        }
        if run.again_due() && self.again_for != Some(run.raid_id) {
            self.again_for = Some(run.raid_id);
            news.push(News::Again);
        }
        // What the tick said: cues for the host's chorus, sounds for every
        // seat in the party's room, the feel ledger's lines.
        news.extend(std::mem::take(&mut run.cues).into_iter().map(News::Cue));
        for event in wire::take_sounds(run) {
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
/// file: the spell cards, the feel ledger, the checkpoint and the timing log.
#[derive(Default)]
struct Service {
    last_saved: Option<Instant>,
    last_scan: Option<Instant>,
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
        if let Some(dir) = &controls.cards
            && self
                .last_scan
                .is_none_or(|at| now.saturating_duration_since(at) >= SPELL_SCAN)
        {
            self.last_scan = Some(now);
            let spells = crate::drive::together_files::read_spells(dir);
            if !spells.is_empty() {
                let mut core = lock(&shared.core);
                let mut loaded = false;
                for (id, raw) in &spells {
                    loaded |= core.run.hot_load_spell(id, raw);
                }
                core.edited |= loaded;
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
            self.last_saved = Some(now);
            let snapshot = Arc::clone(&lock(&shared.newest));
            let invitation = lock(&shared.invitation).clone();
            if let Err(error) = write_checkpoint(path, *raid_serial, &snapshot.run, invitation) {
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
    /// The hosted session's door and join key.
    invitation: Option<(String, String)>,
    country_intents: BTreeMap<String, u32>,
}

/// Write the delve's checkpoint: private, atomic, one writer at a time.
pub(crate) fn write_checkpoint(
    path: &Path,
    raid_serial: u64,
    run: &Run,
    invitation: Option<(String, String)>,
) -> Result<(), String> {
    use std::io::Write as _;
    static WRITING: Mutex<()> = Mutex::new(());
    let saving = Saving {
        version: 1,
        raid_serial,
        run,
        invitation,
        country_intents: run
            .marks
            .iter()
            .filter(|(key, _)| shooter::countryside::intent_key(key))
            .map(|(key, value)| (key.clone(), *value))
            .collect(),
    };
    let bytes = serde_json::to_vec(&saving).map_err(|e| e.to_string())?;
    let _writing = lock(&WRITING);
    let parent = path.parent().ok_or("invalid dungeon save path")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temp = path.with_extension(format!("{}.next", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    // A stale temp from an interrupted save is not a checkpoint.
    if temp.exists() {
        std::fs::remove_file(&temp).map_err(|e| e.to_string())?;
    }
    let result = (|| -> std::io::Result<()> {
        let mut file = options.open(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        std::fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result.map_err(|e| e.to_string())
}

/// Whether the run holds something the cockpit's realm takes in: spoils
/// banked, orders placed, marks made, votes cast.
pub(crate) fn outbox_pending(run: &Run) -> bool {
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
pub(crate) struct Door {
    pub(crate) bind: SocketAddr,
    /// Seats in all, the host's own included.
    pub(crate) seats: u32,
    /// A fixed join key (a restored session's); a new one without.
    pub(crate) key: Option<String>,
    /// The browser's page and the Delve's wasm.
    pub(crate) bundle: Option<ClientBundle>,
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
    fn start(run: Run, thread: bool, tune_file: Option<PathBuf>) -> Live {
        let mut live = Live {
            host: None,
            couch: None,
            shared: None,
            running: None,
            service: None,
            inline: None,
            view: OnceCell::new(),
        };
        let shared = Shared::new(run);
        *lock(&shared.tune_file) = tune_file;
        if let Err(error) = live.serve(Arc::new(shared), thread, Door::solo(), false) {
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
    ) -> Result<(), String> {
        shared.stop.store(false, Ordering::Relaxed);
        let game = DelveServer::new(Arc::clone(&shared), door.seats);
        let mut config = door.config();
        config.tune_file = lock(&shared.tune_file).clone();
        config.dev = dev_mode();
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
        // The host's own knight is a client like any friend's: seat 0.
        self.host = server
            .connect_local()
            .ok()
            .map(|conn| Seat::local(conn, "host"));
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
/// the snapshot latched for this frame; `as_mut` edits the run itself under
/// its lock, and the next read sees the edit.
#[derive(Default)]
pub(crate) struct Sim {
    live: Option<Live>,
    /// This delve's own choice of threads, over the cockpit's (`threaded`).
    thread: Option<bool>,
    /// Where the knobs moved are kept (the workspace's `.angel/dungeon/tune.json`).
    tune_file: Option<PathBuf>,
}

/// The run, held for an edit. The sim waits for it to be let go.
pub(crate) struct RunMut<'a> {
    core: MutexGuard<'a, Core>,
}

impl std::ops::Deref for RunMut<'_> {
    type Target = Run;
    fn deref(&self) -> &Run {
        &self.core.run
    }
}

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
    pub(crate) fn is_some(&self) -> bool {
        self.live.is_some()
    }

    pub(crate) fn is_none(&self) -> bool {
        self.live.is_none()
    }

    /// A delve on its own threads whatever the cockpit chose (the tests of
    /// the threads themselves).
    #[cfg(test)]
    pub(crate) fn spawn(run: Run) -> Sim {
        let mut sim = Sim {
            live: None,
            thread: Some(true),
            tune_file: None,
        };
        sim.set(Some(run));
        sim
    }

    /// Step the next delve this view starts on threads of its own (or not),
    /// whatever the cockpit chose.
    #[cfg(test)]
    pub(crate) fn prefer_thread(&mut self, on: bool) {
        self.thread = Some(on);
    }

    /// Start a new delve (or end this one with `None`). A new delve while
    /// one runs takes its place on the same server: the seats, the link and
    /// the friends on it stay.
    pub(crate) fn set(&mut self, run: Option<Run>) {
        if let (Some(run), Some(live)) = (run.as_ref(), self.live.as_mut()) {
            live.view.take();
            let shared = live.shared();
            let mut core = lock(&shared.core);
            core.run = run.clone();
            core.edited = true;
            shared.outbox.store(true, Ordering::Relaxed);
            return;
        }
        self.live = None;
        let thread = self.thread.unwrap_or_else(threaded);
        let tune_file = self.tune_file.clone();
        self.live = run.map(|run| Live::start(run, thread, tune_file));
    }

    /// Keep the knobs moved in `path`, and start every server from it.
    pub(crate) fn set_tune_file(&mut self, path: PathBuf) {
        if let Some(live) = &self.live {
            *lock(&live.shared().tune_file) = Some(path.clone());
        }
        self.tune_file = Some(path);
    }

    /// The server's knobs, where they stand (`/dungeon tune`).
    pub(crate) fn knobs(&self) -> Vec<Knob> {
        self.server().map(Server::knobs).unwrap_or_default()
    }

    /// Set a server knob as the host (`/dungeon tune name value`): the value
    /// it takes, or why not.
    pub(crate) fn tune(&self, name: &str, value: f64) -> Result<f64, String> {
        self.server().ok_or_else(|| "no delve is running".to_string())?.tune(name, value)
    }

    /// The run as this frame reads it.
    pub(crate) fn as_ref(&self) -> Option<&Run> {
        self.snapshot().map(|s| &s.run)
    }

    /// The snapshot this frame reads.
    pub(crate) fn snapshot(&self) -> Option<&Arc<Snapshot>> {
        let live = self.live.as_ref()?;
        Some(live.view.get_or_init(|| live.shared().fresh()))
    }

    /// The run itself, for an edit.
    pub(crate) fn as_mut(&mut self) -> Option<RunMut<'_>> {
        let live = self.live.as_mut()?;
        live.view.take();
        Some(RunMut {
            core: lock(&live.shared().core),
        })
    }

    /// The run itself, for a test: stepped by hand, the server's is the
    /// only share, so an edit needs no lock.
    #[cfg(test)]
    pub(crate) fn edit(&mut self) -> Option<&mut Run> {
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
    #[cfg(test)]
    pub(crate) fn rescan_spells(&mut self) {
        if let Some(service) = self.live.as_mut().and_then(|l| l.inline.as_mut()) {
            service.last_scan = None;
        }
    }

    /// Read the newest snapshot for this frame.
    pub(crate) fn latch(&mut self) {
        if let Some(live) = self.live.as_mut() {
            live.view.take();
        }
    }

    /// The cockpit's side of the game, for the next tick. What it shows on
    /// the run (the host's knight as stone, a couch partner, a wish being
    /// forged) is seen by the next read, as an edit's is.
    pub(crate) fn set_controls(&mut self, controls: Controls) {
        let Some(live) = self.live.as_mut() else {
            return;
        };
        let shared = live.shared();
        let mut core = lock(&shared.core);
        if apply_controls(&mut core.run, &controls) {
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
    pub(crate) fn pump(&mut self, now: Instant) -> u32 {
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
    pub(crate) fn threaded(&self) -> bool {
        self.live
            .as_ref()
            .and_then(|l| l.running.as_ref())
            .is_some_and(|r| r.manual.is_none())
    }

    /// What the server said since the last look.
    pub(crate) fn drain_news(&self) -> Vec<News> {
        self.live
            .as_ref()
            .map(|live| std::mem::take(&mut *lock(&live.shared().news)))
            .unwrap_or_default()
    }

    /// A line the host's chorus speaks, for every seat to hear.
    pub(crate) fn say(&self, event: wire::Event) {
        if let Some(live) = &self.live {
            lock(&live.shared().said).push(event);
        }
    }

    /// Whether the run has something for the realm, since the last look.
    pub(crate) fn take_outbox(&self) -> bool {
        self.live
            .as_ref()
            .is_some_and(|live| live.shared().outbox.swap(false, Ordering::Relaxed))
    }

    /// When the clock's next step is due, stepping by hand.
    pub(crate) fn clock(&self) -> Option<Instant> {
        let live = self.live.as_ref()?;
        let next = live.running.as_ref()?.manual.as_ref()?.next_due()?;
        Some(next - STEP)
    }

    /// The server this delve runs on.
    pub(crate) fn server(&self) -> Option<&Server> {
        Some(&self.live.as_ref()?.running.as_ref()?.server)
    }

    /// Whether the server is one friends can reach (`host`).
    #[cfg(test)]
    pub(crate) fn hosted(&self) -> bool {
        self.live
            .as_ref()
            .and_then(|l| l.running.as_ref())
            .is_some_and(|r| r.hosted)
    }

    /// This terminal's seat for `knight`: the host's own (1), or a couch
    /// partner's (2).
    pub(crate) fn seat(&self, knight: u32) -> Option<&Seat> {
        let live = self.live.as_ref()?;
        match knight {
            1 => live.host.as_ref(),
            2 => live.couch.as_ref(),
            _ => None,
        }
    }

    /// This terminal's seats, the host's first.
    pub(crate) fn seats(&self) -> impl Iterator<Item = &Seat> {
        self.live
            .iter()
            .flat_map(|live| live.host.iter().chain(live.couch.iter()))
    }

    /// A couch partner plays knight 2 on this keyboard (or no longer does):
    /// their seat is a client of its own, beside the host's.
    pub(crate) fn set_couch(&mut self, on: bool) {
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
    pub(crate) fn host(&mut self, door: Door) -> Result<(), String> {
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
        if let Err(error) = live.serve(Arc::clone(&shared), thread, door, true) {
            // The run goes back to a solo door rather than standing still.
            if let Err(again) = live.serve(shared, thread, Door::solo(), false) {
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
    pub(crate) fn unhost(&mut self) -> Result<(), String> {
        let thread = self.thread.unwrap_or_else(threaded);
        let Some(live) = self.live.as_mut() else {
            return Err("no delve is running".into());
        };
        let shared = live.stop();
        live.view.take();
        *lock(&shared.invitation) = None;
        live.serve(shared, thread, Door::solo(), false)
    }

    /// Record the hosted session's door and key again (a new key was made).
    pub(crate) fn remember_invitation(&self) {
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
    pub(crate) fn invitation(&self) -> Option<(String, String)> {
        lock(&self.live.as_ref()?.shared().invitation).clone()
    }

    #[cfg(test)]
    pub(crate) fn telemetry(&self) -> Telemetry {
        self.live.as_ref().map_or_else(Telemetry::default, |live| {
            let mut telemetry = lock(&live.shared().telemetry);
            let out = *telemetry;
            telemetry.worst_step_us = 0;
            out
        })
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_sim__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_sim__bench.rs"]
mod bench;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_sim__e2e.rs"]
mod e2e;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_sim__presence.rs"]
mod presence_tests;
