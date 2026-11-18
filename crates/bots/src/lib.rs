//! Bots: real WebSocket clients that join a muster and play, each driving
//! the same `angel_net::client::Runtime` a browser does, and measuring
//! what they get. `angel-bots` runs hundreds of them; the end-to-end test
//! runs a few and looks inside their mirrors.

use angel_muster::rules::{FIELD, Knight, SUB};
use angel_muster::{MusterClient, MusterCmd};
use angel_net::client::{Frame, GameClient, Runtime, Status, View};
use angel_net::codec::Malformed;
use angel_net::input::{Hands, button};
use angel_net::proto::{ToClient, Welcome};
use angel_serve::client::WsClient;
use angel_serve::ws::{Message, code};
use std::time::{Duration, Instant};

pub trait Muster: GameClient<Cmd = MusterCmd> {
    fn muster(&self) -> &MusterClient;
}

impl Muster for MusterClient {
    fn muster(&self) -> &MusterClient {
        self
    }
}

pub struct Headless(pub MusterClient);

impl GameClient for Headless {
    type Cmd = MusterCmd;
    const GAME: &'static str = MusterClient::GAME;
    fn boot(welcome: &Welcome) -> Result<Self, String> {
        MusterClient::boot(welcome).map(Headless)
    }
    fn snapshot(&mut self, tick: u32, whole: bool, body: &[u8]) -> Result<(), Malformed> {
        self.0.snapshot(tick, whole, body)
    }
    fn events(&mut self, tick: u32, events: &[&[u8]], lines: &mut Vec<String>) {
        self.0.events(tick, events, lines)
    }
    fn command(&mut self, hands: &Hands) -> MusterCmd {
        self.0.command(hands)
    }
    fn encode_cmd(cmd: &MusterCmd, out: &mut Vec<u8>) {
        MusterClient::encode_cmd(cmd, out)
    }
    fn draw(&mut self, _view: &View<MusterCmd>, _frame: &mut Frame) {}
    fn controls(&self) -> &'static str {
        self.0.controls()
    }
}

impl Muster for Headless {
    fn muster(&self) -> &MusterClient {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct BotConfig {
    pub url: String,
    pub join: String,
    pub name: String,
    pub seed: u64,
    pub wander: bool,
    pub slow: bool,
    pub fps: u32,
    pub warmup: Duration,
    pub stray: i32,
}

pub const STRAY: i32 = 600 * SUB;

#[derive(Clone, Debug, Default)]
pub struct Report {
    pub seat: Option<u32>,
    pub rejected: Option<String>,
    pub error: Option<String>,
    pub secs: f64,
    pub snapshots: u64,
    pub bytes: u64,
    pub intervals: Vec<f32>,
    pub rtts: Vec<f32>,
    pub frames: Vec<f32>,
    pub held: Vec<u32>,
}

const SLOW_BYTES: usize = 256;
const SLOW_EVERY: Duration = Duration::from_millis(250);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const CLOSE_TIMEOUT: Duration = Duration::from_secs(2);

pub struct Rng(pub u64);

impl Rng {
    pub fn draw(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.draw() % n.max(1)
    }
}

struct Thumb {
    rng: Rng,
    until_ms: f64,
    stray: i32,
}

impl Thumb {
    fn drive<G: Muster>(&mut self, rt: &mut Runtime<G>, now: f64) {
        if now < self.until_ms {
            return;
        }
        self.until_ms = now + 800.0 + self.rng.below(2200) as f64;
        let own = rt.game().and_then(|g| {
            let m = g.muster();
            Knight::from_state(m.mirror().get(m.seat()?)?.latest())
        });
        let home = own.map(|k| (FIELD / 2 - k.x, FIELD / 2 - k.y));
        let stick = match home {
            Some((dx, dy)) if dx.abs() > self.stray || dy.abs() > self.stray => {
                let len = f64::from(dx).hypot(f64::from(dy));
                ((f64::from(dx) / len * 127.0) as i8, (f64::from(dy) / len * 127.0) as i8)
            }
            _ if self.rng.below(10) < 3 => (0, 0),
            _ => {
                let a = self.rng.below(360) as f64 * std::f64::consts::PI / 180.0;
                ((a.cos() * 127.0) as i8, (a.sin() * 127.0) as i8)
            }
        };
        let hands = rt.hands();
        hands.stick(Some(stick));
        if self.rng.below(4) == 0 {
            hands.down(button::FIRE);
            hands.up(button::FIRE);
        }
        if self.rng.below(12) == 0 {
            hands.down(button::USE);
            hands.up(button::USE);
        }
    }
}

pub fn run<G: Muster>(cfg: &BotConfig, until: Instant) -> (Report, Runtime<G>) {
    let mut report = Report::default();
    let mut rt = Runtime::<G>::new();
    let mut ws = match WsClient::connect(&cfg.url, CONNECT_TIMEOUT) {
        Ok(ws) => ws,
        Err(e) => {
            report.error = Some(format!("connect: {e}"));
            return (report, rt);
        }
    };
    let start = Instant::now();
    let now_ms = || start.elapsed().as_secs_f64() * 1000.0;
    rt.connected(&cfg.join, "", &cfg.name, 0);
    let mut thumb = Thumb { rng: Rng(cfg.seed | 1), until_ms: 0.0, stray: cfg.stray };
    let step_ms = 1000.0 / f64::from(cfg.fps.max(1));
    let mut next_frame = 0.0;
    let mut last_snapshot: Option<f64> = None;
    let mut measured_from: Option<(Instant, u64)> = None;
    let mut next_held = 0.0;
    let mut last_read = Instant::now() - SLOW_EVERY;
    'run: while Instant::now() < until {
        for out in rt.take_outgoing() {
            if let Err(e) = ws.send(&out) {
                report.error = Some(format!("send: {e}"));
                break 'run;
            }
        }
        let wait = Duration::from_secs_f64(((next_frame - now_ms()) / 1000.0).max(0.0));
        let read = if !cfg.slow {
            ws.fill(usize::MAX, wait)
        } else if last_read.elapsed() >= SLOW_EVERY {
            last_read = Instant::now();
            ws.fill(SLOW_BYTES, wait)
        } else {
            std::thread::sleep(wait);
            Ok(false)
        };
        if let Err(e) = read {
            report.error = Some(format!("read: {e}"));
            break;
        }
        loop {
            let message = match ws.message() {
                Ok(Some(m)) => m,
                Ok(None) => break,
                Err(e) => {
                    report.error = Some(format!("frame: {e}"));
                    break 'run;
                }
            };
            let bytes = match message {
                Message::Binary(b) => b,
                Message::Close(c) => {
                    if !matches!(rt.status(), Status::Rejected(..)) {
                        report.error = Some(format!("closed by the server ({c:?})"));
                    }
                    break 'run;
                }
                _ => continue,
            };
            let now = now_ms();
            let measuring = measured_from.is_some();
            match ToClient::decode(&bytes) {
                Ok(ToClient::Snapshot { .. }) => {
                    if measuring {
                        report.snapshots += 1;
                        if let Some(last) = last_snapshot {
                            report.intervals.push((now - last) as f32);
                        }
                    }
                    last_snapshot = Some(now);
                }
                Ok(ToClient::Pong { t, .. }) if measuring => report.rtts.push((now - t) as f32),
                _ => {}
            }
            if rt.recv(&bytes, now).is_err() {
                report.error = Some("a message did not read".into());
                break 'run;
            }
            if let Status::Rejected(why, text) = rt.status() {
                report.rejected = Some(format!("{why:?}: {text}"));
                break 'run;
            }
        }
        report.seat = rt.seat();
        if measured_from.is_none() && start.elapsed() >= cfg.warmup && rt.status() == &Status::Playing {
            measured_from = Some((Instant::now(), ws.read_bytes));
        }
        let now = now_ms();
        if now >= next_frame {
            next_frame += step_ms;
            if next_frame < now {
                next_frame = now + step_ms;
            }
            if cfg.wander {
                thumb.drive(&mut rt, now);
            }
            let t0 = Instant::now();
            rt.frame(now);
            if measured_from.is_some() {
                report.frames.push(t0.elapsed().as_secs_f32() * 1000.0);
                if now >= next_held {
                    next_held = now + 1000.0;
                    if let Some(g) = rt.game() {
                        report.held.push(g.muster().mirror().live().count() as u32);
                    }
                }
            }
        }
    }
    if let Some((from, bytes)) = measured_from {
        report.secs = from.elapsed().as_secs_f64();
        report.bytes = ws.read_bytes - bytes;
    }
    if report.error.is_none() && report.rejected.is_none() {
        rt.bye();
        for out in rt.take_outgoing() {
            let _ = ws.send(&out);
        }
        ws.close(code::NORMAL);
        let deadline = Instant::now() + CLOSE_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match ws.recv(remaining) {
                Ok(Some(Message::Close(_))) | Ok(None) | Err(_) => break,
                Ok(Some(_)) => {}
            }
        }
    }
    (report, rt)
}

pub fn pct(sorted: &[f32], p: f64) -> f32 {
    if sorted.is_empty() {
        return f32::NAN;
    }
    let rank = ((p * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len());
    sorted[rank - 1]
}
