//! `delve-dev-host`: the Delve on angel-serve without the cockpit, for
//! trying its client (`angel_delve::client`) in a browser and natively.
//!
//! ```text
//! delve-dev-host [--port 8820] [--bind 127.0.0.1] [--bundle DIR] [--key KEY]
//!                [--scene fight|boss|kingdom|first|volley|cinder|latch|mossbell|brasswake|rafterwake|pennon|forest|lanternward|moonfall|bannerpeak|griffinwing|lightgallop|hall|book-hop|drift] [--boss ID] [--art v2] [--snapshot-hz 30]
//!                [--bots N] [--bots-after SECS] [--bots-draw] [--log SECS]
//!                [--dev] [--tune FILE] [--snapshot FILE] [--idle-bots]
//!                [--depth N] [--beat-every SECS]
//! ```
//!
//! **Development only.** It runs a fresh `Run` and steps it with each
//! seat's `DelveCmd`, publishes `wire::publish`'s tree through
//! `wire::seat_tree` and sends the run's sounds as events. It settles
//! nothing into a realm: the run's outboxes (bank, orders, feats, the
//! triumph) are emptied and dropped each tick, the chorus is not run, and
//! no checkpoint is written. The cockpit's own `DelveServer` does all that.
//!
//! - `--scene fight`: the art kit's busy crypt fight. Knights do not fall
//!   (their health is topped up each tick) and the room fills again when it
//!   is cleared, so a fight can be measured for as long as it takes.
//! - `--scene boss`: a live guardian fight; `--boss ID` selects it (default Grinwick).
//! - `--scene kingdom`: the kingdom, at the ranch.
//! - `--scene first`: a fresh run's first room.
//! - `--bots N` joins N native clients (`Runtime<DelveClient>`) after
//!   `--bots-after` seconds, so a browser that joined first holds seat 0.
//!   They walk, aim and fire about, and each prints what it was sent; with
//!   `--bots-draw` they draw every frame too, and print its time.
//! - `--depth N`: the scene's room on floor N of the delve, in that floor's
//!   pack (floor 4 and below stand up in 3D, docs/WORLD-3D.md "Deeper
//!   floors"); `--beat-every SECS` plays the beat of coming down into the
//!   deep again every SECS seconds, to watch and measure it.
//! - `--dev`: any page may set the Delve's knobs from its dev panel (F2),
//!   not only the host; `--tune FILE` keeps the knobs moved there
//!   (`angel_delve::rules::tune`) and starts from them next time.

use angel_delve::client::DelveClient;
use angel_delve::render::overworld::arena::art_v2::testing;
use angel_delve::rules::command::DelveCmd;
use angel_delve::rules::{HZ, Phase, Run};
use angel_delve::settings::{self, Settings};
use angel_delve::wire::{self, Event, Hear, Params};
use angel_net::client::{Frame, GameClient, Runtime, Status, View};
use angel_net::codec::Malformed;
use angel_net::doc::value::Value;
use angel_net::input::{Hands, button};
use angel_net::proto::{ToClient, Welcome};
use angel_serve::client::WsClient;
use angel_serve::link::qr;
use angel_serve::ws::{Message, code};
use angel_serve::{ClientBundle, DocRep, DocTick, DocView, Events, GameServer, Rates, ServeConfig, Server};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

static IDLE_BOTS: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, PartialEq)]
enum Scene {
    Fight,
    Boss,
    Kingdom,
    First,
    /// Every kind of shot thrown out in rings: how shots look in flight.
    Volley,
    Cinder,
    Latch,
    Mossbell,
    Brasswake,
    Rafterwake,
    Pennon,
    Forest,
    /// A Round Table adventure's stub (`rules::roundtable`), by slug.
    Adventure(&'static str),
    Hall,
    BookHop,
    Drift,
}

impl Scene {
    fn run(self, boss: &str) -> Run {
        match self {
            Scene::Fight => testing::fight(),
            Scene::Boss => testing::boss(boss),
            Scene::Kingdom => testing::kingdom(),
            Scene::First => Run::new(7, 1, None),
            Scene::Volley => {
                let mut run = testing::fight();
                run.enemies.clear();
                run.projectiles.clear();
                run
            }
            Scene::Cinder => Run::cinder_for_test(),
            Scene::Latch => Run::latch_for_test(),
            Scene::Brasswake => Run::brasswake_for_test(),
            Scene::Rafterwake => Run::rafterwake_for_test(),
            Scene::Pennon => Run::pennon_for_test(),
            Scene::Forest => Run::forest_road_for_test(),
            Scene::Adventure(slug) => Run::roundtable_for_test(slug).expect("a Round Table adventure"),
            Scene::Mossbell => Run::mossbell_for_test(),
            Scene::Hall => testing::hall(),
            Scene::BookHop => testing::book_hop(),
            Scene::Drift => {
                let mut run = angel_delve::render::overworld::arena::race3d::testing::racing(angel_delve::rules::race3d::Course::Touge, 1);
                run.floor_plates.clear();
                run.race3d.as_mut().unwrap().t = 0;
                run
            },
        }
    }
}

/// The Delve as this host runs it.
struct DevDelve {
    run: Run,
    scene: Scene,
    /// The fight's room, to fill again when it is cleared.
    room: usize,
    snapshot_hz: u32,
    art_v2: bool,
    publisher: wire::Publisher,
    /// The knobs moved (`Run::tune`), kept here so a scene that starts its
    /// run over keeps them.
    tune: angel_delve::rules::tune::Tune,
    freeze_count: u32,
    frozen_ticks: u32,
    freeze_streak: u32,
    longest_freeze: u32,
    /// Play the deep's beat again every so many seconds (`--beat-every`).
    beat_every: Option<u32>,
}

impl DevDelve {
    fn new(scene: Scene, boss: &str, snapshot_hz: u32, art_v2: bool) -> DevDelve {
        let run = scene.run(boss);
        DevDelve { room: run.at, run, scene, snapshot_hz, art_v2, publisher: wire::Publisher::default(), tune: Default::default(), freeze_count: 0, frozen_ticks: 0, freeze_streak: 0, longest_freeze: 0, beat_every: None }
    }

    /// The fight goes on: nobody falls, and a cleared room fills again.
    fn keep_fighting(&mut self) {
        for hero in self.run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        if !self.run.hitstop.frozen() && self.run.phase != Phase::Fighting && self.run.at == self.room {
            self.run.enter_for_test(self.room);
        }
    }

    /// What the rules left for a realm to settle, dropped (dev only).
    fn drop_outboxes(&mut self) {
        self.run.bank.clear();
        self.run.orders.clear();
        self.run.peddled.clear();
        self.run.feats.clear();
        self.run.triumph = None;
        self.run.cues.clear();
    }
}

/// Each seat's share of the tree (`wire::seat_tree`).
struct SeatView;

impl DocView for SeatView {
    fn view<'a>(&self, tree: &'a Value, seat: Option<u32>) -> Cow<'a, Value> {
        wire::seat_tree(tree, seat)
    }

    /// The kingdom's chunks a few a snapshot (`wire::pace`).
    fn pace<'a>(&self, view: Cow<'a, Value>, held: Option<&Value>) -> Cow<'a, Value> {
        wire::pace(view, held)
    }
}

impl GameServer for DevDelve {
    type Cmd = DelveCmd;
    type Rep = DocRep<SeatView>;
    const GAME: &'static str = wire::GAME;

    fn rates(&self) -> Rates {
        Rates { tick_hz: HZ, snapshot_hz: self.snapshot_hz }
    }

    fn seats(&self) -> u32 {
        wire::SEATS
    }

    fn params(&self) -> Vec<u8> {
        Params::with_mode(self.art_v2, angel_delve::world3d::room::mode()).encode()
    }

    fn replicator(&self) -> DocRep<SeatView> {
        DocRep(SeatView)
    }

    fn decode_cmd(bytes: &[u8]) -> Option<DelveCmd> {
        DelveCmd::decode(bytes)
    }

    fn join(&mut self, seat: u32, name: &str, events: &mut Events) {
        let knight = wire::knight_of(seat);
        if knight == 1 {
            // The run's first knight is there from the start: the seat takes
            // it, and its name.
            if let Some(hero) = self.run.players.get_mut(&1) {
                hero.name = name.to_string();
            }
        } else {
            self.run.join_seat(knight, name);
        }
        events.all(Event::Notice { hear: Hear::All, text: format!("{name} joined the party") }.to_vec());
    }

    fn leave(&mut self, seat: u32, events: &mut Events) {
        let knight = wire::knight_of(seat);
        let name = self.run.players.get(&knight).map(|h| h.name.clone()).unwrap_or_default();
        // The first knight stays, as the host's does.
        if knight != 1 {
            self.run.leave(knight);
        }
        events.all(Event::Notice { hear: Hear::All, text: format!("{name} left the party") }.to_vec());
    }

    // The dev host grants known phrases locally; learning new phrases belongs
    // to the cockpit. Use the same action kinds and eligibility as its seats.
    fn action(&mut self, seat: Option<u32>, kind: u32, body: &[u8], events: &mut Events) {
        let Some(knight) = seat.map(wire::knight_of) else { return };
        if !matches!(kind, 1 | 2) { return; }
        let result = (|| {
            let json: serde_json::Value = serde_json::from_slice(body).map_err(|_| "Send JSON".to_string())?;
            if !self.run.can_wish(knight) {
                return Err("Wishes open when the room is clear: one each.".into());
            }
            let wish = if kind == 1 {
                json["wish"].as_str().and_then(angel_delve::rules::phrasebook::get)
            } else {
                json["words"].as_str().and_then(|words| {
                    self.run.players.get(&knight).and_then(|h| angel_delve::rules::phrasebook::find_for(words, &h.deck))
                })
            }.ok_or("No available phrasebook tier. Try a known wish from Tab; new phrases need the cockpit.".to_string())?;
            self.run.grant(knight, &wish).map(|name| format!("Granted: {name}."))
        })();
        events.seat(seat.unwrap(), Event::Notice { hear: Hear::Knight(knight), text: result.unwrap_or_else(|why| why) }.to_vec());
    }

    fn input_age(&mut self, seat: u32, ticks: u64) {
        self.run.latch_input_age.insert(wire::knight_of(seat), ticks);
    }

    fn step(&mut self, tick: u32, cmds: &[Option<DelveCmd>], events: &mut Events) {
        let inputs: BTreeMap<u32, _> = cmds
            .iter()
            .enumerate()
            .filter_map(|(seat, cmd)| cmd.map(|c| (wire::knight_of(seat as u32), c.input())))
            .collect();
        self.run.tune = self.tune;
        let before = (self.run.tick, self.run.hitstop.left);
        self.run.step(&inputs);
        if self.run.tick == before.0 && self.run.hitstop.held {
            self.frozen_ticks += 1;
            self.freeze_streak += 1;
            self.longest_freeze = self.longest_freeze.max(self.freeze_streak);
        } else {
            self.freeze_streak = 0;
        }
        if before.1 == 0 && self.run.hitstop.left > 0 {
            self.freeze_count += 1;
            eprintln!("hitstop transport={tick} sim={} seats={} ticks={}", self.run.tick, self.run.players.len(), self.run.hitstop.left);
        }
        if tick > 0 && tick.is_multiple_of(60 * HZ) {
            eprintln!("hitstop minute={} seats={} freezes={} frozen_ticks={} longest={}", tick / (60 * HZ), self.run.players.len(), self.freeze_count, self.frozen_ticks, self.longest_freeze);
        }
        let notices = wire::take_notices(&mut self.run);
        for event in wire::take_sounds(&mut self.run).into_iter().chain(notices) {
            events.all(event.to_vec());
        }
        self.drop_outboxes();
        if matches!(self.scene,Scene::Fight | Scene::Boss) {
            self.keep_fighting();
        }
        if let Some(secs) = self.beat_every
            && secs > 0
            && tick.is_multiple_of(secs * HZ)
        {
            self.run.deep_beat = Some(self.run.tick);
        }
        if self.scene == Scene::Volley {
            // Nothing but the shots: no monsters, and the knights stand.
            self.run.enemies.clear();
            for hero in self.run.players.values_mut() {
                hero.hp = hero.max_hp;
            }
            if tick % 24 == 0 {
                self.run.volley_for_test(24.0, 15.0, tick as f32 * 0.13);
            }
        }
    }

    fn publish(&mut self, _tick: u32) -> DocTick {
        self.run.tune = self.tune;
        DocTick { tree: self.publisher.publish(&self.run).expect("the run is a tree") }
    }

    fn knobs(&self) -> Vec<angel_net::knob::Knob> {
        self.tune.knobs()
    }

    fn set_knob(&mut self, name: &str, value: f64) -> bool {
        self.tune.set(name, value)
    }
}

struct Args {
    port: u16,
    bind: IpAddr,
    bundle: Option<PathBuf>,
    key: Option<String>,
    scene: Scene,
    boss: String,
    art_v2: bool,
    snapshot_hz: u32,
    bots: u32,
    bots_after: u64,
    bots_draw: bool,
    log: Option<u64>,
    dev: bool,
    tune: Option<PathBuf>,
    snapshot: Option<PathBuf>,
    idle_bots: bool,
    depth: Option<u32>,
    beat_every: Option<u32>,
}

fn args() -> Result<Args, String> {
    let mut a = Args {
        port: 8820,
        bind: IpAddr::V4(Ipv4Addr::LOCALHOST),
        bundle: None,
        key: None,
        scene: Scene::Fight,
        boss: angel_delve::rules::signatures::ENGINE.into(),
        art_v2: false,
        snapshot_hz: HZ,
        bots: 0,
        bots_after: 3,
        bots_draw: false,
        log: None,
        dev: false,
        tune: None,
        snapshot: None,
        idle_bots: false,
        depth: None,
        beat_every: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--port" => a.port = value()?.parse().map_err(|_| "--port takes a number")?,
            "--bind" => a.bind = value()?.parse().map_err(|_| "--bind takes an IP address")?,
            "--bundle" => a.bundle = Some(PathBuf::from(value()?)),
            "--key" => a.key = Some(value()?),
            "--scene" => {
                a.scene = match value()?.as_str() {
                    "fight" => Scene::Fight,
                    "boss" => Scene::Boss,
                    "kingdom" => Scene::Kingdom,
                    "first" => Scene::First,
                    "volley" => Scene::Volley,
                    "cinder" => Scene::Cinder,
                    "latch" => Scene::Latch,
                    "brasswake" => Scene::Brasswake,
                    "rafterwake" => Scene::Rafterwake,
                    "pennon" => Scene::Pennon,
                    "forest" => Scene::Forest,
                    slug if angel_delve::rules::roundtable::adventure(slug).is_some() => {
                        Scene::Adventure(angel_delve::rules::roundtable::adventure(slug).map_or("", |a| a.slug))
                    }
                    "mossbell" => Scene::Mossbell,
                    "hall" => Scene::Hall,
                    "book-hop" => Scene::BookHop,
                    "drift" => Scene::Drift,
                    other => return Err(format!("no scene {other}: fight, boss, kingdom, first, volley, cinder, latch, mossbell, brasswake, rafterwake, pennon, forest, lanternward, moonfall, bannerpeak, griffinwing, lightgallop, hall, book-hop or drift")),
                }
            }
            "--art" => a.art_v2 = value()? == "v2",
            "--boss" => a.boss = value()?,
            "--snapshot-hz" => a.snapshot_hz = value()?.parse().map_err(|_| "--snapshot-hz takes a number")?,
            "--bots" => a.bots = value()?.parse().map_err(|_| "--bots takes a number")?,
            "--bots-after" => a.bots_after = value()?.parse().map_err(|_| "--bots-after takes seconds")?,
            "--bots-draw" => a.bots_draw = true,
            "--log" => a.log = Some(value()?.parse().map_err(|_| "--log takes seconds")?),
            "--dev" => a.dev = true,
            "--tune" => a.tune = Some(PathBuf::from(value()?)),
            "--snapshot" => a.snapshot = Some(PathBuf::from(value()?)),
            "--idle-bots" => a.idle_bots = true,
            "--depth" => a.depth = Some(value()?.parse().map_err(|_| "--depth takes a floor number")?),
            "--beat-every" => a.beat_every = Some(value()?.parse().map_err(|_| "--beat-every takes seconds")?),
            "-h" | "--help" => {
                println!(
                    "delve-dev-host [--port 8820] [--bind 127.0.0.1] [--bundle DIR] [--key KEY] [--scene fight|boss|kingdom|first|volley|cinder|latch|mossbell|brasswake|rafterwake|pennon|forest|lanternward|moonfall|bannerpeak|griffinwing|lightgallop|hall|book-hop|drift] [--boss ID] [--art v2] [--snapshot-hz 30] [--bots N] [--bots-after SECS] [--bots-draw] [--log SECS] [--dev] [--tune FILE] [--snapshot FILE] [--idle-bots] [--depth N] [--beat-every SECS]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown flag {other}")),
        }
    }
    if a.snapshot_hz == 0 || !HZ.is_multiple_of(a.snapshot_hz) {
        return Err(format!("--snapshot-hz must divide the tick rate, {HZ}"));
    }
    if a.scene == Scene::Boss && angel_delve::rules::bosses::checked(&a.boss).is_none() {
        return Err(format!("unknown review boss {}", a.boss));
    }
    Ok(a)
}

fn main() {
    if let Err(why) = run() {
        eprintln!("delve-dev-host: {why}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let a = args()?;
    // The host's settings, as the cockpit sets them from its environment
    // (`ANGEL_DELVE_OPEN=1`: every delve floor laid open).
    settings::set(Settings {
        art_v2: a.art_v2,
        feel_ledger: false,
        open_floors: std::env::var("ANGEL_DELVE_OPEN").is_ok_and(|v| v == "1"),
    });
    // The deep floors in 3D; `ANGEL_WORLD3D=1` every room, `=0` none
    // (docs/WORLD-3D.md, "Deeper floors").
    angel_delve::world3d::room::set_mode(angel_delve::world3d::room::Mode::from_env(std::env::var("ANGEL_WORLD3D").ok().as_deref()));
    let mut config = ServeConfig::new(SocketAddr::new(a.bind, a.port));
    if let Some(dir) = &a.bundle {
        config.bundle = Some(ClientBundle::from_dir(dir).map_err(|e| format!("bundle {}: {e}", dir.display()))?);
    }
    config.key = a.key.clone();
    config.log_every = a.log.map(Duration::from_secs);
    config.dev = a.dev;
    config.tune_file = a.tune.clone();
    IDLE_BOTS.store(a.idle_bots, Ordering::Relaxed);
    let mut game = DevDelve::new(a.scene, &a.boss, a.snapshot_hz, a.art_v2);
    if let Some(path) = &a.snapshot {
        let bytes = std::fs::read(path).map_err(|e| format!("snapshot {}: {e}", path.display()))?;
        game.run = serde_json::from_slice(&bytes).map_err(|e| format!("snapshot {}: {e}", path.display()))?;
        game.room = game.run.at;
        game.tune = game.run.tune;
    }
    if let Some(depth) = a.depth {
        // The same room, down on floor `depth`, dressed as that floor's.
        let first = game.run.dungeon.pack;
        game.run.dungeon.depth = depth;
        game.run.dungeon.pack = angel_delve::rules::layout::Pack::at(depth, first);
    }
    game.beat_every = a.beat_every;
    let server = Server::start(game, config).map_err(|e| format!("cannot serve: {e}"))?;
    println!("the Delve (development host) is open on {}", server.addr());
    let key = server.key();
    println!("join key {key}");
    let links = server.links();
    for link in &links {
        println!("  {link}");
    }
    if let Some(code) = links.first().and_then(|l| qr(l, true)) {
        println!("{code}");
    }
    if a.bots > 0 {
        let url = format!("ws://{}/ws", server.addr());
        thread::sleep(Duration::from_secs(a.bots_after));
        for i in 0..a.bots {
            let (url, key) = (url.clone(), key.clone());
            if a.bots_draw {
                thread::spawn(move || bot::<DelveClient>(&url, &key, i));
            } else {
                thread::spawn(move || bot::<Headless>(&url, &key, i));
            }
            thread::sleep(Duration::from_millis(200));
        }
    }
    loop {
        thread::park();
    }
}

/// The client as a bot runs it: everything but the drawing.
struct Headless(DelveClient);

impl GameClient for Headless {
    type Cmd = DelveCmd;
    const GAME: &'static str = wire::GAME;
    fn boot(welcome: &Welcome) -> Result<Self, String> {
        DelveClient::boot(welcome).map(Headless)
    }
    fn snapshot(&mut self, tick: u32, whole: bool, body: &[u8]) -> Result<(), Malformed> {
        let playing = self.0.newest().and_then(|r|r.cinder.as_ref()).is_some_and(|s|s.left>0);
        let pennon_playing = self.0.newest().and_then(|r|r.pennon.as_ref()).is_some_and(|s|s.left>0);
        let rafter_playing = self.0.newest().and_then(|r|r.rafterwake.as_ref()).is_some_and(|s|s.left>0);
        let brass_playing = self.0.newest().and_then(|r|r.brasswake.as_ref()).is_some_and(|s|s.left>0);
        let latch_playing = self.0.newest().and_then(|r|r.latch.as_ref()).is_some_and(|s|s.left>0);
        self.0.snapshot(tick, whole, body)?;
        if let Some(s)=self.0.newest().and_then(|r|r.pennon.as_ref()).filter(|s|s.left==0 && pennon_playing) {eprintln!("pennon podium {:?}",s.lanes.iter().map(|(id,d)|(id,d.score,d.falls,d.finish)).collect::<Vec<_>>());}
        if let Some(s)=self.0.newest().and_then(|r|r.rafterwake.as_ref()).filter(|s|s.left==0 && rafter_playing) {
            eprintln!("rafterwake podium {:?}",s.lanes.iter().map(|(id,d)|(id,d.score,d.falls,d.finish)).collect::<Vec<_>>());
        }
        if let Some(s)=self.0.newest().and_then(|r|r.brasswake.as_ref()).filter(|s|s.left==0 && brass_playing) {
            eprintln!("brasswake podium {:?}",s.lanes.iter().map(|(id,d)|(id,d.score,d.falls,d.finish)).collect::<Vec<_>>());
        }
        if let Some(s)=self.0.newest().and_then(|r|r.latch.as_ref()).filter(|s|s.left==0 && latch_playing) {
            println!("Latch podium: {:?}; counters/misses/timing {:?}",s.order,s.duels.iter().map(|(id,d)|(*id,d.counters,d.misses,d.error)).collect::<Vec<_>>());
        }
        if let Some(s) = self.0.newest().and_then(|r|r.cinder.as_ref()).filter(|s|s.left==0 && playing) {
            println!("Cinder podium: {:?}; cracks {:?}; retries {:?}",s.order,s.boards.iter().map(|(id,b)|(*id,b.cracks)).collect::<Vec<_>>(),s.boards.iter().map(|(id,b)|(*id,b.misses)).collect::<Vec<_>>());
        }
        Ok(())
    }
    fn events(&mut self, tick: u32, events: &[&[u8]], lines: &mut Vec<String>) {
        self.0.events(tick, events, lines)
    }
    fn command(&mut self, hands: &Hands) -> DelveCmd {
        if let Some(race) = self.0.newest().and_then(|r| r.race3d.as_ref()).filter(|r| r.course == angel_delve::rules::race3d::Course::Touge) {
            let id = self.0.seat().map(wire::knight_of).unwrap_or(1);
            return DelveCmd { held: angel_delve::rules::race3d::drift_bot(race, id), pressed: Default::default() };
        }
        if IDLE_BOTS.load(Ordering::Relaxed) {
            return DelveCmd {held: Default::default(), pressed: Default::default()};
        }
        if let Some(run) = self.0.newest().filter(|r|r.room().kind==angel_delve::rules::RoomKind::CollegeLab) {
            let id = self.0.seat().map(wire::knight_of).unwrap_or(1);
            let input = if let Some(s) = &run.cinder {
                if s.left==0 { Default::default() } else {
                    s.boards.get(&id).map(|b| {
                        let mut i=b.bot_input();
                        if run.tick%18>1 { i.interact=false; }
                        i
                    }).unwrap_or_default()
                }
            } else { angel_delve::rules::Input { interact: run.players.len()>=4, ..Default::default() } };
            return DelveCmd { held: input, pressed: Default::default() };
        }
        if let Some(run) = self.0.newest().filter(|r| r.roundtable_showing()) {
            let id = self.0.seat().map(wire::knight_of).unwrap_or(1);
            let input = run.roundtable_under_way().map(|a| (a.bot)(run, id)).unwrap_or_default();
            return DelveCmd { held: input, pressed: input };
        }
        if let Some(run) = self.0.newest().filter(|r| r.forest_road_showing()) {
            let id = self.0.seat().map(wire::knight_of).unwrap_or(1);
            let input = run.forest_road.as_ref().and_then(|s| run.players.get(&id).map(|h| s.bot_input(id, h))).unwrap_or_default();
            return DelveCmd { held: input, pressed: input };
        }
        if let Some(run)=self.0.newest().filter(|r|r.room().kind==angel_delve::rules::RoomKind::Muster) {
            let id=self.0.seat().map(wire::knight_of).unwrap_or(1);
            let mut input=if let Some(s)=&run.pennon {if s.left==0 {Default::default()}else{s.lanes.get(&id).map(|d|d.bot_input(angel_delve::rules::pennon::LENGTH-s.left)).unwrap_or_default()}}else{angel_delve::rules::Input{interact:run.players.len()>=4,..Default::default()}};
            if input.fire || input.interact {input.latch_tick=Some(run.tick);}
            return DelveCmd{held:input,pressed:input};
        }
        if let Some(run)=self.0.newest().filter(|r|r.room().kind==angel_delve::rules::RoomKind::Workshop) {
            let id=self.0.seat().map(wire::knight_of).unwrap_or(1);
            let mut input=if let Some(s)=&run.rafterwake {if s.left==0 {Default::default()}else{s.lanes.get(&id).map(|d|d.bot_input(angel_delve::rules::rafterwake::LENGTH-s.left)).unwrap_or_default()}}else{angel_delve::rules::Input{interact:run.players.len()>=4,..Default::default()}};
            if input.fire || input.move_y!=0 {input.latch_tick=Some(run.tick);}
            return DelveCmd{held:input,pressed:input};
        }
        if let Some(run)=self.0.newest().filter(|r|r.room().kind==angel_delve::rules::RoomKind::Tavern) {
            let id=self.0.seat().map(wire::knight_of).unwrap_or(1);
            let mut input=if let Some(s)=&run.brasswake {if s.left==0 {Default::default()}else{s.lanes.get(&id).map(|d|d.bot_input(angel_delve::rules::brasswake::LENGTH-s.left)).unwrap_or_default()}}else{angel_delve::rules::Input{interact:run.players.len()>=4,..Default::default()}};
            if input.fire || input.interact {input.latch_tick=Some(run.tick);}
            return DelveCmd{held:input,pressed:input};
        }
        if let Some(run)=self.0.newest().filter(|r|r.room().kind==angel_delve::rules::RoomKind::Bathhouse) {
            let id=self.0.seat().map(wire::knight_of).unwrap_or(1);
            let mut input=if let Some(s)=&run.mossbell {if s.left==0 {Default::default()}else{s.tables.get(&id).map(|t|t.bot_input()).unwrap_or_default()}}else{angel_delve::rules::Input{interact:run.players.len()>=4,..Default::default()}};
            if input.fire || input.move_y!=0 {input.latch_tick=Some(run.tick);}
            return DelveCmd{held:input,pressed:input};
        }
        if let Some(run) = self.0.newest().filter(|r|r.room().kind==angel_delve::rules::RoomKind::Smithy) {
            let id = self.0.seat().map(wire::knight_of).unwrap_or(1);
            let mut input = if let Some(s) = &run.latch {
                if s.left==0 { Default::default() } else {
                    s.duels.get(&id).map(|d| match id {
                        3 => d.late_bot_input(run.tune.latch_recover),
                        4 => d.bot_input(run.tune.latch_window),
                        _ => d.bot_input(1 + id % 3),
                    }).unwrap_or_default()
                }
            } else { angel_delve::rules::Input { interact: run.players.len()>=4, ..Default::default() } };
            if input.fire { input.latch_tick = Some(run.tick); }
            let pressed = angel_delve::rules::Input { fire: input.fire, latch_tick: input.latch_tick, move_y: input.move_y, ..Default::default() };
            return DelveCmd { held: input, pressed };
        }
        self.0.command(hands)
    }
    fn encode_cmd(cmd: &DelveCmd, out: &mut Vec<u8>) {
        DelveClient::encode_cmd(cmd, out)
    }
    fn draw(&mut self, _view: &View<DelveCmd>, _frame: &mut Frame) {}
    fn controls(&self) -> &'static str {
        self.0.controls()
    }
}

/// A bot's frames a second, and how often it prints what it was sent.
const BOT_FPS: f64 = 60.0;
const BOT_REPORT: Duration = Duration::from_secs(10);

/// One bot: joins, walks and aims a random way for a while at a time,
/// fires in bursts, and prints the bytes a second it was sent.
fn bot<G: GameClient<Cmd = DelveCmd>>(url: &str, key: &str, i: u32) {
    let name = format!("bot {}", i + 1);
    let mut ws = match WsClient::connect(url, Duration::from_secs(10)) {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("{name}: {e}");
            return;
        }
    };
    let mut rt = Runtime::<G>::new();
    let start = Instant::now();
    let now_ms = || start.elapsed().as_secs_f64() * 1000.0;
    rt.connected(key, "", &name, angel_net::proto::caps::NATIVE);
    let mut rng = 0x9e37_79b9_7f4a_7c15u64 ^ u64::from(i + 1).wrapping_mul(0x2545_f491_4f6c_dd1d);
    let mut draw = move |n: u64| {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng % n.max(1)
    };
    let directions = [button::UP, button::DOWN, button::LEFT, button::RIGHT];
    let aims = [button::AIM_UP, button::AIM_DOWN, button::AIM_LEFT, button::AIM_RIGHT];
    let mut held = 0u32;
    let mut next_change = 0.0;
    let mut next_frame = 0.0;
    let (mut reported, mut bytes_then) = (Instant::now(), 0u64);
    let mut frames: Vec<f32> = Vec::new();
    loop {
        for out in rt.take_outgoing() {
            if ws.send(&out).is_err() {
                return;
            }
        }
        let wait = Duration::from_secs_f64(((next_frame - now_ms()) / 1000.0).max(0.0));
        if ws.fill(usize::MAX, wait).is_err() {
            eprintln!("{name}: the link closed");
            return;
        }
        while let Ok(Some(message)) = ws.message() {
            match message {
                Message::Binary(bytes) => {
                    if matches!(ToClient::decode(&bytes), Ok(ToClient::Welcome(_))) {
                        println!("{name} joined");
                    }
                    if rt.recv(&bytes, now_ms()).is_err() {
                        eprintln!("{name}: a message did not read");
                    }
                    if let Status::Rejected(why, text) = rt.status() {
                        eprintln!("{name}: turned away ({why:?}: {text})");
                        ws.close(code::NORMAL);
                        return;
                    }
                }
                Message::Close(_) => return,
                _ => {}
            }
        }
        let now = now_ms();
        if now >= next_change {
            next_change = now + 400.0 + draw(1600) as f64;
            let hands = rt.hands();
            hands.up(held);
            held = 0;
            if draw(5) > 0 {
                held |= directions[draw(4) as usize];
                if draw(2) == 0 {
                    held |= directions[draw(4) as usize];
                }
            }
            if draw(3) > 0 {
                held |= aims[draw(4) as usize];
            }
            if draw(4) == 0 {
                held |= button::FIRE;
            }
            hands.down(held);
        }
        if now >= next_frame {
            next_frame = (next_frame + 1000.0 / BOT_FPS).max(now);
            let t = Instant::now();
            rt.frame(now);
            frames.push(t.elapsed().as_secs_f32() * 1000.0);
            rt.take_lines();
        }
        if reported.elapsed() >= BOT_REPORT {
            let secs = reported.elapsed().as_secs_f64();
            frames.sort_by(f32::total_cmp);
            let at = |p: f64| frames.get(((frames.len() as f64 * p) as usize).min(frames.len().saturating_sub(1))).copied().unwrap_or(0.0);
            println!(
                "{name} (seat {:?}): {:.2} KB/s in, frame p50 {:.2} p95 {:.2} max {:.2} ms",
                rt.seat(),
                (ws.read_bytes - bytes_then) as f64 / 1024.0 / secs,
                at(0.5),
                at(0.95),
                frames.last().copied().unwrap_or(0.0)
            );
            (reported, bytes_then) = (Instant::now(), ws.read_bytes);
            frames.clear();
        }
    }
}
