//! Test only. A game small enough to read in one sitting, for testing the
//! browser runtime without a host: you walk a gold dot, a ring of grey dots
//! goes round, and E or F blips. Its loopback server is compiled into the
//! same wasm and exported as `loopback_*`, so the shell's `?offline` page
//! plays it with a fake socket. No real game has a loopback.

use angel_net::client::{Frame, GameClient, View};
use angel_net::clock::bracket;
use angel_net::cmd::{Command, SeatInputs};
use angel_net::codec::{Bytes, Malformed, put_varint, zigzag};
use angel_net::input::{Hands, button};
use angel_net::proto::{PROTOCOL, ToClient, ToServer, Welcome};
use std::collections::VecDeque;

pub const W: u32 = 320;
pub const H: u32 = 180;
pub const TICK_HZ: u32 = 30;
/// Your own dot: the probe finds it by this colour.
pub const OWN: u32 = 0xffff_d23f;
const OTHER: u32 = 0xff6c_8aa8;
const GROUND: u32 = 0xff10_1418;
const GRID: u32 = 0xff1c_232b;
/// Positions are in sixteenths of a pixel; a full stick walks 3 px a tick.
const SUB: i32 = 16;
const SPEED: i32 = 3 * SUB;
const RING: usize = 24;

/// One tick of one seat's hands.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Step {
    pub held: u32,
    pub pressed: u32,
    pub x: i8,
    pub y: i8,
}

impl From<&Hands> for Step {
    fn from(h: &Hands) -> Step {
        Step { held: h.held, pressed: h.pressed, x: h.x, y: h.y }
    }
}

impl Command for Step {
    fn neutral() -> Self {
        Step::default()
    }
    fn held_only(&self) -> Self {
        Step { pressed: 0, ..self.clone() }
    }
    fn absorb(&mut self, older: &Self) {
        self.pressed |= older.pressed;
    }
}

impl Step {
    fn encode(&self, out: &mut Vec<u8>) {
        put_varint(out, u64::from(self.held));
        put_varint(out, u64::from(self.pressed));
        put_varint(out, zigzag(i64::from(self.x)));
        put_varint(out, zigzag(i64::from(self.y)));
    }

    fn decode(body: &[u8]) -> Result<Step, Malformed> {
        let mut b = Bytes::new(body);
        let axis = |v: i32| i8::try_from(v).map_err(|_| Malformed);
        Ok(Step { held: b.u32()?, pressed: b.u32()?, x: axis(b.i32()?)?, y: axis(b.i32()?)? })
    }
}

/// The one movement rule: the server steps it and the client predicts with it.
pub fn walk(pos: (i32, i32), step: &Step) -> (i32, i32) {
    let x = pos.0 + i32::from(step.x) * SPEED / 127;
    let y = pos.1 + i32::from(step.y) * SPEED / 127;
    (x.clamp(0, (W as i32 - 1) * SUB), y.clamp(0, (H as i32 - 1) * SUB))
}

/// Where the ring's dot `i` is on `tick` (server side).
fn ring(i: usize, tick: u32) -> (i32, i32) {
    let a = tick as f32 * 0.04 + i as f32 * std::f32::consts::TAU / RING as f32;
    let r = 64.0 + (i % 3) as f32 * 8.0;
    let (cx, cy) = (W as f32 / 2.0, H as f32 / 2.0);
    (((cx + r * a.cos()) * SUB as f32) as i32, ((cy + r * a.sin()) * SUB as f32) as i32)
}

pub struct TestGame {
    /// Your dot as the server last said.
    own: (i32, i32),
    /// The ring, by tick, newest last: drawn between two of these.
    ring: VecDeque<(u32, Vec<(i32, i32)>)>,
    ticks: Vec<u32>,
}

impl GameClient for TestGame {
    type Cmd = Step;
    const GAME: &'static str = "testgame";

    fn boot(_: &Welcome) -> Result<Self, String> {
        Ok(TestGame { own: (0, 0), ring: VecDeque::new(), ticks: Vec::new() })
    }

    fn snapshot(&mut self, tick: u32, _whole: bool, body: &[u8]) -> Result<(), Malformed> {
        let mut b = Bytes::new(body);
        self.own = (b.i32()?, b.i32()?);
        let n = b.u32()? as usize;
        if n > 1024 {
            return Err(Malformed);
        }
        let mut dots = Vec::with_capacity(n);
        for _ in 0..n {
            dots.push((b.i32()?, b.i32()?));
        }
        b.end()?;
        self.ring.push_back((tick, dots));
        while self.ring.len() > 8 {
            self.ring.pop_front();
        }
        Ok(())
    }

    fn events(&mut self, _: u32, events: &[&[u8]], lines: &mut Vec<String>) {
        for e in events {
            lines.push(format!("sfx {}", String::from_utf8_lossy(e)));
        }
    }

    fn command(&mut self, hands: &Hands) -> Step {
        Step::from(hands)
    }

    fn encode_cmd(cmd: &Step, out: &mut Vec<u8>) {
        cmd.encode(out);
    }

    fn draw(&mut self, view: &View<Step>, frame: &mut Frame) {
        frame.resize(W, H);
        for (y, row) in frame.px.chunks_exact_mut(W as usize).enumerate() {
            for (x, px) in row.iter_mut().enumerate() {
                *px = if x % 16 == 0 || y % 16 == 0 { GRID } else { GROUND };
            }
        }
        // Everyone else, between the two snapshots around the render tick.
        self.ticks.clear();
        self.ticks.extend(self.ring.iter().map(|(t, _)| *t));
        if let Some((a, b, t)) = bracket(&self.ticks, view.render_tick) {
            let (da, db) = (&self.ring[a].1, &self.ring[b].1);
            for (pa, pb) in da.iter().zip(db) {
                let x = pa.0 as f32 + (pb.0 - pa.0) as f32 * t;
                let y = pa.1 as f32 + (pb.1 - pa.1) as f32 * t;
                square(frame, x / SUB as f32, y / SUB as f32, 2, OTHER);
            }
        }
        // Your own: the server's word, the commands it has not taken, and
        // the part of a tick the keys held now have walked.
        let mut own = self.own;
        for (_, step) in view.pending {
            own = walk(own, step);
        }
        let next = walk(own, &Step::from(&view.hands));
        let x = own.0 as f32 + (next.0 - own.0) as f32 * view.partial;
        let y = own.1 as f32 + (next.1 - own.1) as f32 * view.partial;
        square(frame, x / SUB as f32, y / SUB as f32, 3, OWN);
    }

    fn controls(&self) -> &'static str {
        r#"{"stick":"left","buttons":[{"id":"use","label":"E","bit":32},{"id":"fire","label":"F","bit":16}]}"#
    }
}

fn square(frame: &mut Frame, cx: f32, cy: f32, r: i32, colour: u32) {
    let (cx, cy) = (cx.round() as i32, cy.round() as i32);
    for y in (cy - r).max(0)..(cy + r).min(frame.h as i32) {
        for x in (cx - r).max(0)..(cx + r).min(frame.w as i32) {
            frame.px[(y as u32 * frame.w + x as u32) as usize] = colour;
        }
    }
}

/// The loopback server: one seat, the ring, a fixed one-way delay each way.
pub struct Loopback {
    delay_ms: f64,
    to_server: VecDeque<(f64, Vec<u8>)>,
    to_client: VecDeque<(f64, Vec<u8>)>,
    joined: bool,
    seat: SeatInputs<Step>,
    own: (i32, i32),
    tick: u32,
    next_ms: Option<f64>,
    packed: Vec<u8>,
}

pub const LOOPBACK_TOKEN: &str = "loopback-seat";

impl Loopback {
    pub fn new(delay_ms: f64) -> Loopback {
        Loopback {
            delay_ms,
            to_server: VecDeque::new(),
            to_client: VecDeque::new(),
            joined: false,
            seat: SeatInputs::default(),
            own: (W as i32 / 2 * SUB, H as i32 / 2 * SUB),
            tick: 0,
            next_ms: None,
            packed: Vec::new(),
        }
    }

    pub fn send(&mut self, msg: &[u8], now_ms: f64) {
        self.to_server.push_back((now_ms + self.delay_ms, msg.to_vec()));
    }

    /// Run the server to `now_ms`; the messages that reach the client by
    /// then, packed `[u32 len LE][bytes]`.
    pub fn take(&mut self, now_ms: f64) -> &[u8] {
        // Tick by tick, taking each message at the tick it arrived by, as a
        // server on its own clock would, however late the page calls this.
        let step_ms = 1000.0 / f64::from(TICK_HZ);
        let mut next = *self.next_ms.get_or_insert(now_ms);
        while next <= now_ms {
            self.deliver(next);
            self.step(next);
            next += step_ms;
        }
        self.next_ms = Some(next);
        self.deliver(now_ms);
        self.packed.clear();
        while self.to_client.front().is_some_and(|(at, _)| *at <= now_ms) {
            let (_, msg) = self.to_client.pop_front().expect("front is there");
            self.packed.extend_from_slice(&(msg.len() as u32).to_le_bytes());
            self.packed.extend_from_slice(&msg);
        }
        &self.packed
    }

    fn deliver(&mut self, until_ms: f64) {
        while self.to_server.front().is_some_and(|(at, _)| *at <= until_ms) {
            let (at, msg) = self.to_server.pop_front().expect("front is there");
            self.serve(&msg, at);
        }
    }

    fn reply(&mut self, at_ms: f64, msg: ToClient) {
        self.to_client.push_back((at_ms + self.delay_ms, msg.to_vec()));
    }

    fn serve(&mut self, msg: &[u8], now_ms: f64) {
        match ToServer::decode(msg) {
            Ok(ToServer::Hello { .. }) => {
                self.joined = true;
                self.seat.restart();
                let welcome = Welcome {
                    protocol: PROTOCOL,
                    game: TestGame::GAME,
                    seat: Some(0),
                    seat_token: LOOPBACK_TOKEN,
                    tick_hz: TICK_HZ,
                    snapshot_hz: TICK_HZ,
                    tick: self.tick,
                    params: &[],
                };
                self.reply(now_ms, ToClient::Welcome(welcome));
            }
            Ok(ToServer::Cmds { cmds, .. }) => {
                for (seq, body) in cmds {
                    if let Ok(step) = Step::decode(body) {
                        self.seat.push(seq, step);
                    }
                }
            }
            Ok(ToServer::Ping { t }) => self.reply(now_ms, ToClient::Pong { t, server_ms: now_ms }),
            Ok(ToServer::Bye) => self.joined = false,
            Ok(ToServer::Action { .. }) | Err(_) => {}
        }
    }

    fn step(&mut self, at_ms: f64) {
        self.tick += 1;
        if !self.joined {
            return;
        }
        let (step, ack) = self.seat.take();
        self.own = walk(self.own, &step);
        let mut body = Vec::new();
        put_varint(&mut body, zigzag(i64::from(self.own.0)));
        put_varint(&mut body, zigzag(i64::from(self.own.1)));
        put_varint(&mut body, RING as u64);
        for i in 0..RING {
            let (x, y) = ring(i, self.tick);
            put_varint(&mut body, zigzag(i64::from(x)));
            put_varint(&mut body, zigzag(i64::from(y)));
        }
        let tick = self.tick;
        self.reply(at_ms, ToClient::Snapshot { tick, ack, whole: true, body: &body });
        if step.pressed & (button::USE | button::FIRE) != 0 {
            self.reply(at_ms, ToClient::Events { tick, events: vec![b"blip"] });
        }
    }
}

angel_web::export_client!(TestGame);

#[cfg(target_arch = "wasm32")]
mod loopback_exports {
    use super::Loopback;
    use std::cell::RefCell;

    thread_local! {
        static LOOP: RefCell<Loopback> = RefCell::new(Loopback::new(0.0));
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn loopback_open(delay_ms: f64) {
        LOOP.with(|l| *l.borrow_mut() = Loopback::new(delay_ms));
    }

    /// # Safety
    /// `ptr` and `len` are a buffer from `angel_alloc`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn loopback_send(ptr: *const u8, len: usize, now_ms: f64) {
        let msg = unsafe { angel_web::bytes(ptr, len) };
        LOOP.with(|l| l.borrow_mut().send(msg, now_ms));
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn loopback_take(now_ms: f64) -> usize {
        LOOP.with(|l| l.borrow_mut().take(now_ms).len())
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn loopback_ptr() -> *const u8 {
        LOOP.with(|l| l.borrow().packed.as_ptr())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angel_net::client::Runtime;

    fn unpack(mut packed: &[u8]) -> Vec<Vec<u8>> {
        let mut msgs = Vec::new();
        while !packed.is_empty() {
            let len = u32::from_le_bytes(packed[..4].try_into().unwrap()) as usize;
            msgs.push(packed[4..4 + len].to_vec());
            packed = &packed[4 + len..];
        }
        msgs
    }

    fn own_x(frame: &Frame) -> Option<usize> {
        frame.px.iter().position(|&p| p == OWN).map(|i| i % W as usize)
    }

    /// Frames at 60 Hz through the loopback with an 80 ms trip each way.
    #[test]
    fn your_dot_moves_on_the_next_frame_and_the_server_agrees() {
        let mut rt = Runtime::<TestGame>::new();
        let mut server = Loopback::new(80.0);
        rt.connected("", "", "probe", 0);
        let frame_ms = 1000.0 / 60.0;
        let mut now = 0.0;
        let mut run = |rt: &mut Runtime<TestGame>, server: &mut Loopback, frames: usize| {
            let mut xs = Vec::new();
            for _ in 0..frames {
                now += frame_ms;
                for msg in rt.take_outgoing() {
                    server.send(&msg, now);
                }
                for msg in unpack(server.take(now)) {
                    rt.recv(&msg, now).unwrap();
                }
                xs.push(own_x(rt.frame(now)));
            }
            xs
        };
        let still = run(&mut rt, &mut server, 30);
        let start = still.last().copied().flatten().expect("the dot is drawn");
        rt.hands().down(button::RIGHT);
        let walking = run(&mut rt, &mut server, 60);
        assert!(walking[0].unwrap() > start, "moved on the first frame: {walking:?}");
        assert!(walking.windows(2).all(|w| w[1] >= w[0]), "never steps back: {walking:?}");
        rt.hands().up(button::RIGHT);
        let after = run(&mut rt, &mut server, 30);
        // The server walked as far as the prediction did: no correction.
        assert_eq!(after.first(), after.last());
        assert_eq!(rt.status(), &angel_net::client::Status::Playing);
    }
}
