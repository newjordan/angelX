//! `delve-dev-host`: the Delve on angel-serve without the cockpit, for
//! trying its client (`angel_delve::client`) in a browser and natively.
//!
//! ```text
//! delve-dev-host [--port 8820] [--bind 127.0.0.1] [--bundle DIR] [--key KEY]
//!                [--scene fight|boss|kingdom|first] [--art v2] [--snapshot-hz 30]
//!                [--bots N] [--bots-after SECS] [--bots-draw] [--log SECS]
//!                [--dev] [--tune FILE]
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
//! - `--scene boss`: a live Grinwick fight for creature art review.
//! - `--scene kingdom`: the kingdom, at the ranch.
//! - `--scene first`: a fresh run's first room.
//! - `--bots N` joins N native clients (`Runtime<DelveClient>`) after
//!   `--bots-after` seconds, so a browser that joined first holds seat 0.
//!   They walk, aim and fire about, and each prints what it was sent; with
//!   `--bots-draw` they draw every frame too, and print its time.
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
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq)]
enum Scene {
    Fight,
    Boss,
    Kingdom,
    First,
}

impl Scene {
    fn run(self) -> Run {
        match self {
            Scene::Fight => testing::fight(),
            Scene::Boss => {
                let mut run = testing::fight();
                run.enemies.clear();
                run.projectiles.clear();
                let index = run.bosses.iter().position(|b| b.id == angel_delve::rules::signatures::ENGINE).unwrap();
                run.spawn_boss_at_for_test(index,24.0,15.0);
                run.enemies[0].signature.active = true;
                run
            },
            Scene::Kingdom => testing::kingdom(),
            Scene::First => Run::new(7, 1, None),
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
}

impl DevDelve {
    fn new(scene: Scene, snapshot_hz: u32, art_v2: bool) -> DevDelve {
        let run = scene.run();
        DevDelve { room: run.at, run, scene, snapshot_hz, art_v2, publisher: wire::Publisher::default(), tune: Default::default() }
    }

    /// The fight goes on: nobody falls, and a cleared room fills again.
    fn keep_fighting(&mut self) {
        for hero in self.run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        if self.run.phase != Phase::Fighting && self.run.at == self.room {
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
        Params { art_v2: self.art_v2 }.encode()
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

    fn step(&mut self, _tick: u32, cmds: &[Option<DelveCmd>], events: &mut Events) {
        let inputs: BTreeMap<u32, _> = cmds
            .iter()
            .enumerate()
            .filter_map(|(seat, cmd)| cmd.map(|c| (wire::knight_of(seat as u32), c.input())))
            .collect();
        self.run.tune = self.tune;
        self.run.step(&inputs);
        for event in wire::take_sounds(&mut self.run) {
            events.all(event.to_vec());
        }
        self.drop_outboxes();
        if matches!(self.scene,Scene::Fight | Scene::Boss) {
            self.keep_fighting();
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
    art_v2: bool,
    snapshot_hz: u32,
    bots: u32,
    bots_after: u64,
    bots_draw: bool,
    log: Option<u64>,
    dev: bool,
    tune: Option<PathBuf>,
}

fn args() -> Result<Args, String> {
    let mut a = Args {
        port: 8820,
        bind: IpAddr::V4(Ipv4Addr::LOCALHOST),
        bundle: None,
        key: None,
        scene: Scene::Fight,
        art_v2: false,
        snapshot_hz: HZ,
        bots: 0,
        bots_after: 3,
        bots_draw: false,
        log: None,
        dev: false,
        tune: None,
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
                    other => return Err(format!("no scene {other}: fight, boss, kingdom or first")),
                }
            }
            "--art" => a.art_v2 = value()? == "v2",
            "--snapshot-hz" => a.snapshot_hz = value()?.parse().map_err(|_| "--snapshot-hz takes a number")?,
            "--bots" => a.bots = value()?.parse().map_err(|_| "--bots takes a number")?,
            "--bots-after" => a.bots_after = value()?.parse().map_err(|_| "--bots-after takes seconds")?,
            "--bots-draw" => a.bots_draw = true,
            "--log" => a.log = Some(value()?.parse().map_err(|_| "--log takes seconds")?),
            "--dev" => a.dev = true,
            "--tune" => a.tune = Some(PathBuf::from(value()?)),
            "-h" | "--help" => {
                println!(
                    "delve-dev-host [--port 8820] [--bind 127.0.0.1] [--bundle DIR] [--key KEY] [--scene fight|boss|kingdom|first] [--art v2] [--snapshot-hz 30] [--bots N] [--bots-after SECS] [--bots-draw] [--log SECS] [--dev] [--tune FILE]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown flag {other}")),
        }
    }
    if a.snapshot_hz == 0 || !HZ.is_multiple_of(a.snapshot_hz) {
        return Err(format!("--snapshot-hz must divide the tick rate, {HZ}"));
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
    // The host's settings, as the cockpit sets them from its environment.
    settings::set(Settings { art_v2: a.art_v2, feel_ledger: false });
    let mut config = ServeConfig::new(SocketAddr::new(a.bind, a.port));
    if let Some(dir) = &a.bundle {
        config.bundle = Some(ClientBundle::from_dir(dir).map_err(|e| format!("bundle {}: {e}", dir.display()))?);
    }
    config.key = a.key.clone();
    config.log_every = a.log.map(Duration::from_secs);
    config.dev = a.dev;
    config.tune_file = a.tune.clone();
    let game = DevDelve::new(a.scene, a.snapshot_hz, a.art_v2);
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
        self.0.snapshot(tick, whole, body)
    }
    fn events(&mut self, tick: u32, events: &[&[u8]], lines: &mut Vec<String>) {
        self.0.events(tick, events, lines)
    }
    fn command(&mut self, hands: &Hands) -> DelveCmd {
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
