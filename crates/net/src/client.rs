//! The client's half of every game, and the runtime that drives it.
//!
//! A game implements `GameClient`: read snapshots, turn hands into a
//! command, draw. `Runtime` does everything else the same way for every
//! game and every device: the hello, the fixed command tick, the commands
//! not yet acknowledged (for prediction), the server clock, pings and the
//! outgoing queue. The browser's wasm module and the native angelX client
//! both wrap one `Runtime`; neither does any of this itself. No IO here:
//! bytes go in through `recv` and come out through `take_outgoing`.

use crate::clock::{CORRECT_MS, FixedStep, INTERP_MS, ServerClock};
use crate::cmd::Command;
use crate::codec::Malformed;
use crate::input::{Hands, Sampler};
use crate::knob::{self, Knob, Side};
use crate::proto::{PROTOCOL, Reject, ToClient, ToServer, Welcome};
use std::collections::VecDeque;

#[derive(Clone, Default)]
pub struct Frame {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u32>,
}

impl Frame {
    pub fn resize(&mut self, w: u32, h: u32) {
        if self.w != w || self.h != h {
            self.w = w;
            self.h = h;
            self.px = vec![0xff00_0000; (w * h) as usize];
        }
    }
}

pub struct View<'a, C> {
    pub render_tick: f64,
    pub newest_tick: u32,
    pub ack: u32,
    pub pending: &'a [(u32, C)],
    pub partial: f32,
    pub hands: Hands,
    pub seat: Option<u32>,
    pub now_ms: f64,
    pub correct_ms: f64,
}

pub trait GameClient: Sized {
    type Cmd: Command;
    const GAME: &'static str;

    fn boot(welcome: &Welcome) -> Result<Self, String>;
    fn snapshot(&mut self, tick: u32, whole: bool, body: &[u8]) -> Result<(), Malformed>;
    fn events(&mut self, tick: u32, events: &[&[u8]], lines: &mut Vec<String>);
    fn command(&mut self, hands: &Hands) -> Self::Cmd;
    fn encode_cmd(cmd: &Self::Cmd, out: &mut Vec<u8>);
    fn draw(&mut self, view: &View<Self::Cmd>, frame: &mut Frame);
    fn lines(&mut self, _lines: &mut Vec<String>) {}
    fn own_at(&self) -> Option<(f32, f32)> {
        None
    }
    fn controls(&self) -> &'static str;
    fn knobs(&self) -> Vec<Knob> {
        Vec::new()
    }
    fn set_knob(&mut self, _name: &str, _value: f64) -> Option<f64> {
        None
    }
    fn dev_stats(&self, _out: &mut Vec<(&'static str, f64)>) {}
    fn sheet(&self) -> String {
        String::new()
    }
    fn choices(&self) -> String {
        String::new()
    }
    fn choose(&mut self, _n: u32) {}
    fn pointer(&mut self, _frame_x:u32, _frame_y:u32, _button:u8)->bool { false }
    fn pointer_move(&mut self, _frame_x:u32, _frame_y:u32)->bool { false }
    fn pointer_up(&mut self, _frame_x:u32, _frame_y:u32, _button:u8)->bool { false }
    fn picks(&self) -> String {
        String::new()
    }
    fn stage(&mut self, _now_ms: f64, _out: &mut Vec<u8>) -> bool {
        false
    }
    fn stage_reset(&mut self) {}
    fn keep_action(&mut self, _kind: u32, _body: &[u8]) -> bool {
        false
    }
    fn take_actions(&mut self) -> Vec<(u32, Vec<u8>)> {
        Vec::new()
    }
    fn stage_props(&mut self, _on: bool) {}
    fn painting_wanted(&mut self) -> Option<String> {
        None
    }
    fn put_painting(&mut self, _name: &str, _width: u32, _height: u32, _rgba: &[u8]) -> bool {
        false
    }
    fn painting_failed(&mut self, _name: &str) {}
    fn hd_manifest(&mut self, _json: &str, _scale: u32, _rung: u32) -> bool {
        false
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Joining,
    Playing,
    Rejected(Reject, String),
}

const MAX_PENDING: usize = 64;
const PING_MS: f64 = 1000.0;

pub struct Runtime<G: GameClient> {
    game: Option<G>,
    status: Status,
    seat: Option<u32>,
    seat_token: String,
    step: FixedStep,
    clock: ServerClock,
    sampler: Sampler,
    seq: u32,
    pending: VecDeque<(u32, G::Cmd)>,
    ack: u32,
    newest_tick: u32,
    last_cmd_ms: f64,
    last_ping_ms: f64,
    idling: bool,
    rtt_ms: Option<f64>,
    outgoing: Vec<Vec<u8>>,
    queued: std::collections::VecDeque<(u32, Vec<u8>)>,
    lines: Vec<String>,
    frame: Frame,
    interp_ms: f64,
    correct_ms: f64,
    server_knobs: String,
    knobs_said: u32,
    server_stats: String,
}

impl<G: GameClient> Default for Runtime<G> {
    fn default() -> Self {
        Runtime::new()
    }
}

impl<G: GameClient> Runtime<G> {
    pub fn new() -> Runtime<G> {
        Runtime {
            game: None,
            status: Status::Joining,
            seat: None,
            seat_token: String::new(),
            step: FixedStep::new(30),
            clock: ServerClock::new(30),
            sampler: Sampler::default(),
            seq: 0,
            pending: VecDeque::new(),
            ack: 0,
            newest_tick: 0,
            last_cmd_ms: 0.0,
            last_ping_ms: f64::NEG_INFINITY,
            idling: false,
            rtt_ms: None,
            outgoing: Vec::new(),
            queued: std::collections::VecDeque::new(),
            lines: Vec::new(),
            frame: Frame::default(),
            interp_ms: INTERP_MS,
            correct_ms: CORRECT_MS,
            server_knobs: String::new(),
            knobs_said: 0,
            server_stats: String::new(),
        }
    }

    pub fn connected(&mut self, join: &str, resume: &str, name: &str, caps: u32) {
        self.status = Status::Joining;
        self.seq = 0;
        self.pending.clear();
        self.ack = 0;
        self.newest_tick = 0;
        self.clock.reset();
        self.outgoing.clear();
        let hello = ToServer::Hello { protocol: PROTOCOL, game: G::GAME, join, resume, name, caps };
        self.outgoing.push(hello.to_vec());
    }

    pub fn recv(&mut self, bytes: &[u8], now_ms: f64) -> Result<(), Malformed> {
        match ToClient::decode(bytes)? {
            ToClient::Welcome(welcome) => {
                if welcome.protocol != PROTOCOL || welcome.game != G::GAME {
                    self.status = Status::Rejected(Reject::Protocol, "this page is out of date".into());
                    return Ok(());
                }
                self.step = FixedStep::new(welcome.tick_hz);
                self.clock = ServerClock::new(welcome.tick_hz);
                self.seat = welcome.seat;
                self.seat_token = welcome.seat_token.to_string();
                match G::boot(&welcome) {
                    Ok(game) => {
                        self.game = Some(game);
                        self.status = Status::Playing;
                    }
                    Err(why) => self.status = Status::Rejected(Reject::Game, why),
                }
            }
            ToClient::Reject { why, text } => self.status = Status::Rejected(why, text.to_string()),
            ToClient::Snapshot { tick, ack, whole, body } => {
                let game = self.game.as_mut().ok_or(Malformed)?;
                game.snapshot(tick, whole, body)?;
                self.clock.arrived(tick, now_ms);
                self.newest_tick = self.newest_tick.max(tick);
                self.ack = ack;
                while self.pending.front().is_some_and(|(seq, _)| *seq <= ack) {
                    self.pending.pop_front();
                }
            }
            ToClient::Events { tick, events } => {
                if let Some(game) = self.game.as_mut() {
                    let late = self.idling
                        || self.clock.tick_at(now_ms - 250.0).is_some_and(|cutoff| f64::from(tick) < cutoff);
                    if late {
                        let mut lines = Vec::new();
                        game.events(tick, &events, &mut lines);
                        self.lines.extend(lines.into_iter().filter(|line| !line.starts_with("sfx ")));
                    } else {
                        game.events(tick, &events, &mut self.lines);
                    }
                }
            }
            ToClient::Pong { t, .. } => {
                let rtt = (now_ms - t).max(0.0);
                self.rtt_ms = Some(self.rtt_ms.map_or(rtt, |was| if rtt < was { rtt } else { was + (rtt - was) * 0.2 }));
            }
            ToClient::Notice { text } => self.lines.push(format!("notice {text}")),
            ToClient::Knobs { json } => {
                self.server_knobs = json.to_string();
                self.knobs_said = self.knobs_said.wrapping_add(1);
            }
            ToClient::Stats { json } => self.server_stats = json.to_string(),
        }
        Ok(())
    }

    pub fn hands(&mut self) -> &mut Sampler {
        &mut self.sampler
    }

    pub fn tick(&mut self, now_ms: f64) -> bool {
        if self.game.is_none() || self.status != Status::Playing {
            return false;
        }
        if std::mem::take(&mut self.idling) {
            self.step.restart(now_ms);
        }
        let due = self.step.due(now_ms);
        if due > 0 {
            self.send_commands(due, now_ms);
        }
        self.ping(now_ms);
        true
    }

    pub fn idle(&mut self, now_ms: f64) -> bool {
        if self.game.is_none() || self.status != Status::Playing {
            return false;
        }
        self.idling = true;
        self.lines.retain(|line| !line.starts_with("sfx "));
        self.ping(now_ms);
        true
    }

    fn ping(&mut self, now_ms: f64) {
        if now_ms - self.last_ping_ms >= PING_MS {
            self.last_ping_ms = now_ms;
            self.outgoing.push(ToServer::Ping { t: now_ms }.to_vec());
        }
    }

    pub fn frame(&mut self, now_ms: f64) -> &Frame {
        self.tick(now_ms);
        self.draw(now_ms)
    }

    pub fn draw(&mut self, now_ms: f64) -> &Frame {
        if self.status != Status::Playing {
            return &self.frame;
        }
        let Some(game) = self.game.as_mut() else {
            return &self.frame;
        };
        let render_tick = self.clock.render_tick_behind(now_ms, self.interp_ms).unwrap_or(f64::from(self.newest_tick));
        let partial = (((now_ms - self.last_cmd_ms) / self.step.step_ms()) as f32).clamp(0.0, 1.0);
        let view = View {
            render_tick,
            newest_tick: self.newest_tick,
            ack: self.ack,
            pending: self.pending.make_contiguous(),
            partial,
            hands: self.sampler.peek(),
            seat: self.seat,
            now_ms,
            correct_ms: self.correct_ms,
        };
        game.draw(&view, &mut self.frame);
        game.lines(&mut self.lines);
        &self.frame
    }

    pub fn last_frame(&self) -> &Frame {
        &self.frame
    }

    pub fn command_now(&mut self, now_ms: f64) {
        if self.status != Status::Playing || self.game.is_none() {
            return;
        }
        self.step.restart(now_ms);
        self.send_commands(1, now_ms);
    }

    fn send_commands(&mut self, due: u32, now_ms: f64) {
        let Some(game) = self.game.as_mut() else {
            return;
        };
        let mut bodies = Vec::with_capacity(due as usize);
        for _ in 0..due {
            let hands = self.sampler.sample();
            let cmd = game.command(&hands);
            self.seq = self.seq.wrapping_add(1).max(1);
            let mut body = Vec::new();
            G::encode_cmd(&cmd, &mut body);
            bodies.push((self.seq, body));
            self.pending.push_back((self.seq, cmd));
            if self.pending.len() > MAX_PENDING {
                self.pending.pop_front();
            }
        }
        for (kind, body) in game.take_actions() {
            self.queued.extend(pieces(kind, &body));
        }
        let skip = bodies.len().saturating_sub(crate::proto::MAX_CMDS);
        let cmds = bodies[skip..].iter().map(|(seq, body)| (*seq, body.as_slice())).collect();
        let msg = ToServer::Cmds { applied: self.newest_tick, cmds };
        self.outgoing.push(msg.to_vec());
        if let Some((kind, body)) = self.queued.pop_front() {
            self.outgoing.push(ToServer::Action { kind, body: &body }.to_vec());
        }
        self.last_cmd_ms = now_ms;
    }

    pub fn bye(&mut self) {
        self.outgoing.push(ToServer::Bye.to_vec());
    }

    pub fn action(&mut self, kind: u32, body: &[u8]) {
        if self.game.as_mut().is_some_and(|g| g.keep_action(kind, body)) {
            return;
        }
        self.outgoing.push(ToServer::Action { kind, body }.to_vec());
    }

    pub fn take_outgoing(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.outgoing)
    }

    pub fn take_lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.lines)
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    pub fn seat(&self) -> Option<u32> {
        self.seat
    }

    pub fn seat_token(&self) -> &str {
        &self.seat_token
    }

    pub fn rtt_ms(&self) -> Option<f64> {
        self.rtt_ms
    }

    pub fn game(&self) -> Option<&G> {
        self.game.as_ref()
    }

    pub fn game_mut(&mut self) -> Option<&mut G> {
        self.game.as_mut()
    }

    pub fn controls(&self) -> &'static str {
        self.game.as_ref().map_or("{}", |g| g.controls())
    }

    pub fn sheet(&self) -> String {
        self.game.as_ref().map_or_else(String::new, G::sheet)
    }

    pub fn choices(&self) -> String {
        self.game.as_ref().map_or_else(String::new, G::choices)
    }

    pub fn choose(&mut self, n: u32) {
        if let Some(game) = self.game.as_mut() {
            game.choose(n);
        }
    }

    pub fn pointer(&mut self, frame_x:u32, frame_y:u32, button:u8)->bool {
        self.game.as_mut().is_some_and(|game|game.pointer(frame_x,frame_y,button))
    }

    pub fn pointer_move(&mut self, frame_x:u32, frame_y:u32)->bool {
        self.game.as_mut().is_some_and(|game|game.pointer_move(frame_x,frame_y))
    }

    pub fn pointer_up(&mut self, frame_x:u32, frame_y:u32, button:u8)->bool {
        self.game.as_mut().is_some_and(|game|game.pointer_up(frame_x,frame_y,button))
    }

    pub fn picks(&self) -> String {
        self.game.as_ref().map_or_else(String::new, G::picks)
    }

    pub fn own_at(&self) -> Option<(f32, f32)> {
        self.game.as_ref().and_then(G::own_at)
    }

    pub fn knobs(&self) -> Vec<Knob> {
        let mut knobs = vec![
            Knob::new("interp_ms", "net", "Others drawn behind (ms)", "INTERP_MS (angel-net clock.rs)", Side::Client)
                .range(0.0, 300.0, 5.0)
                .default(INTERP_MS)
                .at(self.interp_ms),
            Knob::new("correct_ms", "net", "Correction eases (ms)", "CORRECT_MS (angel-net clock.rs)", Side::Client)
                .range(0.0, 500.0, 10.0)
                .default(CORRECT_MS)
                .at(self.correct_ms),
        ];
        if let Some(game) = &self.game {
            knobs.extend(game.knobs());
        }
        knobs
    }

    pub fn set_knob(&mut self, name: &str, value: f64) -> Option<f64> {
        let mine = self.knobs().into_iter().find(|k| k.name == name)?;
        let value = mine.fit(value)?;
        match name {
            "interp_ms" => self.interp_ms = value,
            "correct_ms" => self.correct_ms = value,
            _ => return self.game.as_mut()?.set_knob(name, value),
        }
        Some(value)
    }

    pub fn tune(&mut self, name: &str, value: f64) {
        if knob::valid_name(name) && value.is_finite() {
            self.action(knob::action::TUNE, &knob::tune_body(name, value));
        }
    }

    pub fn watch_stats(&mut self, on: bool) {
        self.action(knob::action::STATS, &[u8::from(on)]);
    }

    pub fn server_knobs(&self) -> &str {
        &self.server_knobs
    }

    pub fn knobs_said(&self) -> u32 {
        self.knobs_said
    }

    pub fn server_stats(&self) -> &str {
        &self.server_stats
    }

    pub fn dev_stats(&self, out: &mut Vec<(&'static str, f64)>) {
        if let Some(game) = &self.game {
            game.dev_stats(out);
        }
    }
}

pub const PIECE: u32 = 100;
pub const PIECE_BYTES: usize = 900;

pub fn pieces(kind: u32, body: &[u8]) -> Vec<(u32, Vec<u8>)> {
    use crate::codec::put_varint;
    if body.len() <= PIECE_BYTES {
        return vec![(kind, body.to_vec())];
    }
    let chunks: Vec<&[u8]> = body.chunks(PIECE_BYTES).collect();
    chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let mut out = Vec::with_capacity(chunk.len() + 12);
            put_varint(&mut out, u64::from(kind));
            put_varint(&mut out, chunks.len() as u64);
            put_varint(&mut out, i as u64);
            out.extend_from_slice(chunk);
            (PIECE, out)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::{Bytes, put_varint};
    use crate::input::button;

    #[derive(Clone, Debug, PartialEq)]
    struct Walk(i32);

    impl Command for Walk {
        fn neutral() -> Self {
            Walk(0)
        }
        fn held_only(&self) -> Self {
            self.clone()
        }
        fn absorb(&mut self, _: &Self) {}
    }

    struct Line {
        x: i32,
        drawn: Option<i32>,
        hands: Hands,
    }

    impl GameClient for Line {
        type Cmd = Walk;
        const GAME: &'static str = "line";
        fn boot(_: &Welcome) -> Result<Self, String> {
            Ok(Line { x: 0, drawn: None, hands: Hands::default() })
        }
        fn snapshot(&mut self, _: u32, _: bool, body: &[u8]) -> Result<(), Malformed> {
            self.x = Bytes::new(body).i32()?;
            Ok(())
        }
        fn events(&mut self, _: u32, events: &[&[u8]], lines: &mut Vec<String>) {
            for e in events {
                lines.push(format!("sfx {}", String::from_utf8_lossy(e)));
                if *e == b"mixed" {
                    lines.push("say Keep this".to_string());
                    lines.push("notice Keep this too".to_string());
                }
            }
        }
        fn command(&mut self, hands: &Hands) -> Walk {
            Walk(i32::from(hands.x.signum()))
        }
        fn encode_cmd(cmd: &Walk, out: &mut Vec<u8>) {
            put_varint(out, crate::codec::zigzag(i64::from(cmd.0)));
        }
        fn draw(&mut self, view: &View<Walk>, frame: &mut Frame) {
            frame.resize(4, 4);
            self.drawn = Some(self.x + view.pending.iter().map(|(_, c)| c.0).sum::<i32>());
            self.hands = view.hands;
        }
        fn controls(&self) -> &'static str {
            "{}"
        }
        fn keep_action(&mut self, kind: u32, _: &[u8]) -> bool {
            kind == 77
        }
    }

    fn welcome() -> Vec<u8> {
        ToClient::Welcome(Welcome {
            protocol: PROTOCOL,
            game: "line",
            seat: Some(2),
            seat_token: "tok",
            tick_hz: 30,
            snapshot_hz: 30,
            tick: 0,
            params: &[],
        })
        .to_vec()
    }

    fn snapshot(tick: u32, ack: u32, x: i32) -> Vec<u8> {
        let mut body = Vec::new();
        put_varint(&mut body, crate::codec::zigzag(i64::from(x)));
        ToClient::Snapshot { tick, ack, whole: true, body: &body }.to_vec()
    }

    #[test]
    fn an_action_the_game_keeps_is_never_sent() {
        let mut rt = Runtime::<Line>::new();
        rt.connected("join", "", "Gwen", 0);
        rt.take_outgoing();
        rt.recv(&welcome(), 0.0).unwrap();
        rt.action(77, b"mine");
        assert!(rt.take_outgoing().is_empty(), "the game took it");
        rt.action(5, b"yours");
        let out = rt.take_outgoing();
        assert_eq!(out.len(), 1);
        match ToServer::decode(&out[0]) {
            Ok(ToServer::Action { kind: 5, body }) => assert_eq!(body, b"yours"),
            other => panic!("{other:?}"),
        }
        let mut idle = Runtime::<Line>::new();
        idle.action(77, b"x");
        assert_eq!(idle.take_outgoing().len(), 1);
    }

    #[test]
    fn a_held_key_is_sent_once_a_tick_and_predicted_until_acknowledged() {
        let mut rt = Runtime::<Line>::new();
        rt.connected("join", "", "Gwen", 0);
        let hello = rt.take_outgoing();
        assert!(matches!(ToServer::decode(&hello[0]), Ok(ToServer::Hello { name: "Gwen", .. })));
        rt.recv(&welcome(), 0.0).unwrap();
        assert_eq!(rt.status(), &Status::Playing);
        assert_eq!(rt.seat_token(), "tok");
        rt.recv(&snapshot(1, 0, 10), 0.0).unwrap();

        rt.hands().down(button::RIGHT);
        let step = 1000.0 / 30.0;
        for i in 0..3 {
            rt.frame(f64::from(i) * step + 1.0);
        }
        let sent: Vec<_> = rt.take_outgoing();
        let cmds: Vec<u32> = sent
            .iter()
            .filter_map(|m| match ToServer::decode(m) {
                Ok(ToServer::Cmds { cmds, .. }) => Some(cmds.iter().map(|(s, _)| *s).collect::<Vec<_>>()),
                _ => None,
            })
            .flatten()
            .collect();
        assert_eq!(cmds, vec![1, 2, 3]);
        assert_eq!(rt.game().unwrap().drawn, Some(13));

        rt.recv(&snapshot(2, 2, 12), 3.0 * step).unwrap();
        rt.frame(3.0 * step + 1.0);
        assert_eq!(rt.game().unwrap().drawn, Some(14));
    }

    #[test]
    fn the_view_carries_the_hands_between_command_ticks() {
        let mut rt = Runtime::<Line>::new();
        rt.connected("join", "", "Gwen", 0);
        rt.recv(&welcome(), 0.0).unwrap();
        rt.frame(1.0);
        rt.hands().down(button::LEFT);
        rt.frame(10.0);
        assert_eq!(rt.game().unwrap().hands.x, -127);
        assert_eq!(rt.game().unwrap().hands.pressed, button::LEFT);
    }

    #[test]
    fn letting_go_is_sent_at_once_and_the_tick_counts_from_there() {
        let mut rt = Runtime::<Line>::new();
        rt.connected("join", "", "Gwen", 0);
        rt.recv(&welcome(), 0.0).unwrap();
        rt.hands().down(button::RIGHT);
        rt.frame(1.0);
        rt.take_outgoing();
        rt.hands().release_all();
        rt.command_now(10.0);
        let sent = rt.take_outgoing();
        assert!(matches!(ToServer::decode(&sent[0]), Ok(ToServer::Cmds { ref cmds, .. }) if cmds.len() == 1 && cmds[0].0 == 2));
        assert_eq!(rt.game().unwrap().drawn, Some(1), "the walk right, then nothing");
        rt.frame(40.0);
        assert!(rt.take_outgoing().iter().all(|m| !matches!(ToServer::decode(m), Ok(ToServer::Cmds { .. }))));
        rt.frame(44.0);
        assert!(rt.take_outgoing().iter().any(|m| matches!(ToServer::decode(m), Ok(ToServer::Cmds { .. }))));
    }

    #[test]
    fn the_round_trip_drops_at_once_and_rises_slowly() {
        let mut rt = Runtime::<Line>::new();
        rt.connected("join", "", "Gwen", 0);
        rt.recv(&welcome(), 0.0).unwrap();
        let pong = |t: f64| ToClient::Pong { t, server_ms: 0.0 }.to_vec();
        rt.recv(&pong(0.0), 300.0).unwrap();
        assert_eq!(rt.rtt_ms(), Some(300.0));
        rt.recv(&pong(1000.0), 1050.0).unwrap();
        assert_eq!(rt.rtt_ms(), Some(50.0));
        rt.recv(&pong(2000.0), 2150.0).unwrap();
        assert_eq!(rt.rtt_ms(), Some(70.0));
    }

    #[test]
    fn an_idle_page_sends_pings_and_no_commands_and_starts_afresh() {
        let mut rt = Runtime::<Line>::new();
        rt.connected("join", "", "Gwen", 0);
        rt.recv(&welcome(), 0.0).unwrap();
        rt.frame(1.0);
        rt.take_outgoing();
        let cmds = |out: &[Vec<u8>]| {
            out.iter()
                .filter_map(|m| match ToServer::decode(m) {
                    Ok(ToServer::Cmds { cmds, .. }) => Some(cmds.len()),
                    _ => None,
                })
                .sum::<usize>()
        };
        for second in 1..=3 {
            assert!(rt.idle(f64::from(second) * 1000.0 + 2.0));
        }
        let out = rt.take_outgoing();
        assert_eq!(cmds(&out), 0);
        assert!(out.iter().any(|m| matches!(ToServer::decode(m), Ok(ToServer::Ping { .. }))));
        rt.frame(3100.0);
        assert_eq!(cmds(&rt.take_outgoing()), 0);
        rt.frame(3100.0 + 1000.0 / 30.0 + 1.0);
        assert_eq!(cmds(&rt.take_outgoing()), 1);
    }

    #[test]
    fn the_engines_client_knobs_set_here_and_a_server_knob_is_asked_for() {
        let mut rt = Runtime::<Line>::new();
        rt.connected("join", "", "Gwen", 0);
        rt.recv(&welcome(), 0.0).unwrap();
        rt.take_outgoing();
        let names: Vec<_> = rt.knobs().iter().map(|k| k.name).collect();
        assert_eq!(names, ["interp_ms", "correct_ms"]);
        assert_eq!(rt.set_knob("interp_ms", 52.0), Some(50.0), "on its step");
        assert_eq!(rt.set_knob("correct_ms", 9999.0), Some(500.0), "inside its bounds");
        assert_eq!(rt.set_knob("no_such", 1.0), None);
        assert_eq!(rt.set_knob("interp_ms", f64::NAN), None);
        let k = rt.knobs().into_iter().find(|k| k.name == "interp_ms").unwrap();
        assert_eq!((k.value, k.default), (50.0, crate::clock::INTERP_MS));
        for tick in 1..=4 {
            rt.recv(&snapshot(tick, 0, 0), f64::from(tick) * 1000.0 / 30.0).unwrap();
        }
        let now = 4.0 * 1000.0 / 30.0;
        let at = rt.clock.render_tick_behind(now, 50.0).unwrap();
        assert!((rt.clock.tick_at(now).unwrap() - at - 1.5).abs() < 1e-9, "50 ms is a tick and a half");

        rt.tune("hero_speed", 15.5);
        let sent = rt.take_outgoing();
        let Ok(ToServer::Action { kind, body }) = ToServer::decode(&sent[0]) else { panic!("an action") };
        assert_eq!(kind, crate::knob::action::TUNE);
        assert_eq!(crate::knob::read_tune(body), Ok(("hero_speed", 15.5)));
        rt.tune("not a name", 1.0);
        assert!(rt.take_outgoing().is_empty(), "a name no knob has is not sent");
        let table = "{\"dev\":true,\"may\":true,\"knobs\":[]}";
        rt.recv(&ToClient::Knobs { json: table }.to_vec(), now).unwrap();
        assert_eq!((rt.server_knobs(), rt.knobs_said()), (table, 1));
        rt.recv(&ToClient::Stats { json: "{\"queued\":2}" }.to_vec(), now).unwrap();
        assert_eq!(rt.server_stats(), "{\"queued\":2}");
    }

    #[test]
    fn a_reject_stops_the_runtime_and_says_why() {
        let mut rt = Runtime::<Line>::new();
        rt.connected("bad", "", "x", 0);
        let reject = ToClient::Reject { why: Reject::Key, text: "no such session" }.to_vec();
        rt.recv(&reject, 0.0).unwrap();
        assert_eq!(rt.status(), &Status::Rejected(Reject::Key, "no such session".into()));
    }

    #[test]
    fn old_effects_are_dropped_against_the_clock_before_a_catch_up_snapshot() {
        let mut rt = Runtime::<Line>::new();
        rt.recv(&welcome(), 0.0).unwrap();
        rt.recv(&snapshot(30, 0, 10), 1000.0).unwrap();
        rt.lines.push("notice already queued".to_string());
        let old = ToClient::Events { tick: 40, events: vec![b"old"] }.to_vec();
        rt.recv(&old, 2000.0).unwrap();
        let fresh = ToClient::Events { tick: 60, events: vec![b"fresh"] }.to_vec();
        rt.recv(&fresh, 2000.0).unwrap();
        assert_eq!(rt.take_lines(), vec!["notice already queued", "sfx fresh"]);
        rt.recv(&snapshot(60, 0, 10), 2000.0).unwrap();
    }

    #[test]
    fn stale_effect_filter_preserves_subtitles_notices_and_boundary_effects() {
        let mut rt = Runtime::<Line>::new();
        rt.recv(&welcome(), 0.0).unwrap();
        rt.recv(&snapshot(30, 0, 10), 1000.0).unwrap();
        let old = ToClient::Events { tick: 30, events: vec![b"mixed"] }.to_vec();
        let boundary = ToClient::Events { tick: 30, events: vec![b"boundary"] }.to_vec();
        rt.recv(&boundary, 1250.0).unwrap();
        assert_eq!(rt.take_lines(), vec!["sfx boundary"]);
        rt.recv(&old, 1251.0).unwrap();
        assert_eq!(rt.take_lines(), vec!["say Keep this", "notice Keep this too"]);
    }

    #[test]
    fn idle_drops_queued_and_incoming_effects_but_not_durable_lines() {
        let mut rt = Runtime::<Line>::new();
        rt.recv(&welcome(), 0.0).unwrap();
        let events = ToClient::Events { tick: 3, events: vec![b"mixed"] }.to_vec();
        rt.recv(&events, 0.0).unwrap();
        rt.idle(10.0);
        rt.recv(&events, 20.0).unwrap();
        rt.tick(30.0);
        assert_eq!(
            rt.take_lines(),
            vec!["say Keep this", "notice Keep this too", "say Keep this", "notice Keep this too"]
        );
        rt.recv(&events, 30.0).unwrap();
        assert_eq!(rt.take_lines(), vec!["sfx mixed", "say Keep this", "notice Keep this too"]);
    }

    #[test]
    fn events_become_lines_for_the_shell() {
        let mut rt = Runtime::<Line>::new();
        rt.connected("join", "", "x", 0);
        rt.recv(&welcome(), 0.0).unwrap();
        let events = ToClient::Events { tick: 3, events: vec![b"gold"] }.to_vec();
        rt.recv(&events, 0.0).unwrap();
        assert_eq!(rt.take_lines(), vec!["sfx gold".to_string()]);
    }
}
