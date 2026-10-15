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

/// A picture: `px` is `w * h` words, `0xAARRGGBB`.
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

/// What a game needs to draw one display frame.
pub struct View<'a, C> {
    /// The server tick, fractional, everyone else is drawn at.
    pub render_tick: f64,
    /// The newest server tick held.
    pub newest_tick: u32,
    /// The last of this client's commands the server has consumed.
    pub ack: u32,
    /// Commands sent but not yet consumed, oldest first: replayed on the
    /// newest state, they put your own knight where your keys have.
    pub pending: &'a [(u32, C)],
    /// How far into the current command tick the display is (0..1).
    pub partial: f32,
    /// The hands now, not yet sampled into a command. A game that walks its
    /// own knight `partial` of a tick along these shows a key on the very
    /// next display frame instead of at the next command tick (the muster's
    /// commands are 50 ms apart: three frames at 60 Hz).
    pub hands: Hands,
    pub seat: Option<u32>,
    pub now_ms: f64,
    /// How long a correction of your own character is spread over: the
    /// `correct_ms` knob, `CORRECT_MS` unless moved.
    pub correct_ms: f64,
}

pub trait GameClient: Sized {
    type Cmd: Command;
    /// The game's name, as in `Hello` and `Welcome`.
    const GAME: &'static str;

    /// Built from the server's welcome (its params carry what the game
    /// needs before the first snapshot: a world seed, an art set).
    fn boot(welcome: &Welcome) -> Result<Self, String>;
    /// A snapshot arrived: the whole state, or a patch against the last one.
    fn snapshot(&mut self, tick: u32, whole: bool, body: &[u8]) -> Result<(), Malformed>;
    /// A tick's events, turned into lines for the shell to act on:
    /// `sfx <name>`, `music <name>`, `say <text>`, `notice <text>`.
    fn events(&mut self, tick: u32, events: &[&[u8]], lines: &mut Vec<String>);
    /// This command tick's command.
    fn command(&mut self, hands: &Hands) -> Self::Cmd;
    fn encode_cmd(cmd: &Self::Cmd, out: &mut Vec<u8>);
    /// Draw one display frame into `frame` (the game picks its size).
    fn draw(&mut self, view: &View<Self::Cmd>, frame: &mut Frame);
    /// Lines the game makes of its own state rather than of events, after
    /// each drawn frame: the Delve's music follows what its own view shows,
    /// so a reload or a resumed seat hears the right track at once. Most
    /// games make none.
    fn lines(&mut self, _lines: &mut Vec<String>) {}
    /// Where the last frame drew the player's own character, in frame
    /// pixels, for probes that time a key to the character moving.
    fn own_at(&self) -> Option<(f32, f32)> {
        None
    }
    /// The touch layout as JSON (see `angel-web`).
    fn controls(&self) -> &'static str;
    /// This client's own knobs (`knob::Side::Client`), where they stand.
    fn knobs(&self) -> Vec<Knob> {
        Vec::new()
    }
    /// Set one of this client's own knobs: the value it took, or `None` for
    /// a name it has not.
    fn set_knob(&mut self, _name: &str, _value: f64) -> Option<f64> {
        None
    }
    /// Numbers for a developer's panel, counted from the start (the panel
    /// makes rates of them): the Delve's own-knight corrections.
    fn dev_stats(&self, _out: &mut Vec<(&'static str, f64)>) {}
    /// A sheet of the player's own state as plain text, for the page to
    /// show while its key is held up (`controls.sheet`): the Delve's cards
    /// and wishes. Empty for a game without one.
    fn sheet(&self) -> String {
        String::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// Waiting for the welcome.
    Joining,
    Playing,
    /// Turned away; the shell shows why and does not retry.
    Rejected(Reject, String),
}

/// Commands kept unacknowledged at most (a dead link stops piling up).
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
    /// The game went off screen (`idle`): the next `tick` starts the
    /// commands afresh rather than catching up.
    idling: bool,
    rtt_ms: Option<f64>,
    outgoing: Vec<Vec<u8>>,
    lines: Vec<String>,
    frame: Frame,
    /// The client knobs the engine keeps: everyone else drawn this far
    /// behind, and a correction eased over this long.
    interp_ms: f64,
    correct_ms: f64,
    /// The server's knob table as it last said it (JSON), and how many
    /// times it has said one.
    server_knobs: String,
    knobs_said: u32,
    /// The server's numbers for this seat, as it last said them (JSON).
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
            lines: Vec::new(),
            frame: Frame::default(),
            interp_ms: INTERP_MS,
            correct_ms: CORRECT_MS,
            server_knobs: String::new(),
            knobs_said: 0,
            server_stats: String::new(),
        }
    }

    /// A connection opened: queue the hello. `resume` is the seat token
    /// from an earlier welcome, or empty. Commands start from 1 again.
    pub fn connected(&mut self, join: &str, resume: &str, name: &str, caps: u32) {
        self.status = Status::Joining;
        self.seq = 0;
        self.pending.clear();
        self.ack = 0;
        // A new connection may be to a restarted host whose ticks start over.
        self.newest_tick = 0;
        self.clock.reset();
        self.outgoing.clear();
        let hello = ToServer::Hello { protocol: PROTOCOL, game: G::GAME, join, resume, name, caps };
        self.outgoing.push(hello.to_vec());
    }

    /// One message from the server.
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
                    game.events(tick, &events, &mut self.lines);
                }
            }
            ToClient::Pong { t, .. } => {
                // A pong that queued behind a big message (the whole tick at
                // a join) or a busy frame says nothing of the link: a lower
                // round trip is taken at once, a higher one eased in.
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

    /// The input sampler: feed it key, pad and touch events as they come.
    pub fn hands(&mut self) -> &mut Sampler {
        &mut self.sampler
    }

    /// Send the commands due at `now_ms` (and a ping, when one is due) with
    /// nothing drawn: a client whose game is not on screen keeps its seat's
    /// commands going at the command rate. True while playing.
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

    /// The game is off screen (a hidden tab): no commands, only the pings
    /// that keep the link. The server counts a seat sending none as away
    /// (`angel_serve::Presence`), so a game can keep its character out of
    /// harm's way until the player looks again; the next `tick` starts the
    /// commands afresh. True while playing.
    pub fn idle(&mut self, now_ms: f64) -> bool {
        if self.game.is_none() || self.status != Status::Playing {
            return false;
        }
        self.idling = true;
        self.ping(now_ms);
        true
    }

    fn ping(&mut self, now_ms: f64) {
        if now_ms - self.last_ping_ms >= PING_MS {
            self.last_ping_ms = now_ms;
            self.outgoing.push(ToServer::Ping { t: now_ms }.to_vec());
        }
    }

    /// One display frame at `now_ms`: send the commands due, then draw.
    pub fn frame(&mut self, now_ms: f64) -> &Frame {
        self.tick(now_ms);
        self.draw(now_ms)
    }

    /// Draw one display frame at `now_ms`, with no command sent: for a
    /// driver that samples its commands on one thread (`tick`) and draws
    /// on another.
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

    /// The frame `frame` drew last, as it was: for a driver that sends what
    /// the frame queued before handing the picture on.
    pub fn last_frame(&self) -> &Frame {
        &self.frame
    }

    /// Sample the hands and send a command now, ahead of the command tick,
    /// and count the next tick from here. For letting go at once (a hidden
    /// tab, a window that lost focus): otherwise the server repeats the
    /// keys last held until the next tick's command comes, and a page
    /// hidden for a while sends none for a second.
    pub fn command_now(&mut self, now_ms: f64) {
        if self.status != Status::Playing || self.game.is_none() {
            return;
        }
        self.step.restart(now_ms);
        self.send_commands(1, now_ms);
    }

    /// Sample `due` commands and queue them in one `Cmds`.
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
        // Only the newest `MAX_CMDS` of a pile-up go out.
        let skip = bodies.len().saturating_sub(crate::proto::MAX_CMDS);
        let cmds = bodies[skip..].iter().map(|(seq, body)| (*seq, body.as_slice())).collect();
        let msg = ToServer::Cmds { applied: self.newest_tick, cmds };
        self.outgoing.push(msg.to_vec());
        self.last_cmd_ms = now_ms;
    }

    /// Leaving on purpose: the seat is freed.
    pub fn bye(&mut self) {
        self.outgoing.push(ToServer::Bye.to_vec());
    }

    /// A one-off choice (a vote, a wish, a chat line).
    pub fn action(&mut self, kind: u32, body: &[u8]) {
        self.outgoing.push(ToServer::Action { kind, body }.to_vec());
    }

    /// Messages to send, oldest first.
    pub fn take_outgoing(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.outgoing)
    }

    /// Lines for the shell (sounds, notices), oldest first.
    pub fn take_lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.lines)
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    pub fn seat(&self) -> Option<u32> {
        self.seat
    }

    /// Keep this to take the seat back after a dropped connection.
    pub fn seat_token(&self) -> &str {
        &self.seat_token
    }

    pub fn rtt_ms(&self) -> Option<f64> {
        self.rtt_ms
    }

    pub fn game(&self) -> Option<&G> {
        self.game.as_ref()
    }

    pub fn controls(&self) -> &'static str {
        self.game.as_ref().map_or("{}", |g| g.controls())
    }

    /// The game's sheet (`GameClient::sheet`).
    pub fn sheet(&self) -> String {
        self.game.as_ref().map_or_else(String::new, G::sheet)
    }

    /// Where the last frame drew the own character (`GameClient::own_at`).
    pub fn own_at(&self) -> Option<(f32, f32)> {
        self.game.as_ref().and_then(G::own_at)
    }

    /// This client's own knobs: the engine's, then the game's.
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

    /// Set one of this client's own knobs: the value it took, or `None`
    /// for a name it has not (or a number that is not one).
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

    /// Ask the server to set one of its knobs. It answers with its table,
    /// set or not (only the host may, outside dev mode).
    pub fn tune(&mut self, name: &str, value: f64) {
        if knob::valid_name(name) && value.is_finite() {
            self.action(knob::action::TUNE, &knob::tune_body(name, value));
        }
    }

    /// Ask the server for its numbers once a second (`server_stats`), or stop.
    pub fn watch_stats(&mut self, on: bool) {
        self.action(knob::action::STATS, &[u8::from(on)]);
    }

    /// The server's knob table as JSON (`knob::table_json`), empty before it
    /// said one.
    pub fn server_knobs(&self) -> &str {
        &self.server_knobs
    }

    /// How many tables the server has said: a panel redraws when it moves.
    pub fn knobs_said(&self) -> u32 {
        self.knobs_said
    }

    /// The server's numbers for this seat as JSON, empty until asked.
    pub fn server_stats(&self) -> &str {
        &self.server_stats
    }

    /// The game's numbers for a developer's panel (`GameClient::dev_stats`).
    pub fn dev_stats(&self, out: &mut Vec<(&'static str, f64)>) {
        if let Some(game) = &self.game {
            game.dev_stats(out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::{Bytes, put_varint};
    use crate::input::button;

    /// A one-knight game: the snapshot is the knight's x; a command walks it.
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
            // Your own knight: the server's word, then your keys it has not taken.
            self.drawn = Some(self.x + view.pending.iter().map(|(_, c)| c.0).sum::<i32>());
            self.hands = view.hands;
        }
        fn controls(&self) -> &'static str {
            "{}"
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

        // The server took two of them and walked the knight to 12.
        rt.recv(&snapshot(2, 2, 12), 3.0 * step).unwrap();
        rt.frame(3.0 * step + 1.0);
        // 12, plus command 3 and the new command 4 still pending.
        assert_eq!(rt.game().unwrap().drawn, Some(14));
    }

    #[test]
    fn the_view_carries_the_hands_between_command_ticks() {
        let mut rt = Runtime::<Line>::new();
        rt.connected("join", "", "Gwen", 0);
        rt.recv(&welcome(), 0.0).unwrap();
        rt.frame(1.0);
        // A key between two command ticks: no command yet, but the draw sees it.
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
        // Mid-tick, the page is hidden: everything let go, and said now.
        rt.hands().release_all();
        rt.command_now(10.0);
        let sent = rt.take_outgoing();
        assert!(matches!(ToServer::decode(&sent[0]), Ok(ToServer::Cmds { ref cmds, .. }) if cmds.len() == 1 && cmds[0].0 == 2));
        assert_eq!(rt.game().unwrap().drawn, Some(1), "the walk right, then nothing");
        // The next command tick is a whole step after the one sent now.
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
        // The first pong waited behind the whole tick sent at the join.
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
        // Hidden for three seconds: a ping a second, not one command.
        for second in 1..=3 {
            assert!(rt.idle(f64::from(second) * 1000.0 + 2.0));
        }
        let out = rt.take_outgoing();
        assert_eq!(cmds(&out), 0);
        assert!(out.iter().any(|m| matches!(ToServer::decode(m), Ok(ToServer::Ping { .. }))));
        // Back: no pile of commands for the time away, one a tick from now.
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
        // Everyone else is drawn the knob's distance behind.
        for tick in 1..=4 {
            rt.recv(&snapshot(tick, 0, 0), f64::from(tick) * 1000.0 / 30.0).unwrap();
        }
        let now = 4.0 * 1000.0 / 30.0;
        let at = rt.clock.render_tick_behind(now, 50.0).unwrap();
        assert!((rt.clock.tick_at(now).unwrap() - at - 1.5).abs() < 1e-9, "50 ms is a tick and a half");

        // A server knob: asked for with an engine action, and the table the
        // server says is kept as it said it.
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
    fn events_become_lines_for_the_shell() {
        let mut rt = Runtime::<Line>::new();
        rt.connected("join", "", "x", 0);
        rt.recv(&welcome(), 0.0).unwrap();
        let events = ToClient::Events { tick: 3, events: vec![b"gold"] }.to_vec();
        rt.recv(&events, 0.0).unwrap();
        assert_eq!(rt.take_lines(), vec!["sfx gold".to_string()]);
    }
}
