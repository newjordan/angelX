//! The sim: the game steps at its fixed tick, takes one command a seat a
//! tick, and publishes each tick once for the writers. It never touches a
//! socket and never waits on a client.
//!
//! It runs on a thread of its own (`run`), or on its caller's thread,
//! stepped by hand (`Manual`): a host that keeps its own clock, and tests
//! that step a game exactly. Both take the same steps (`Stepper`).

use crate::conn::Out;
use crate::server::{Hub, TickOut};
use crate::tune::{self, Engine};
use crate::{AWAY_AFTER_TICKS, Events, GameServer, Presence};
use angel_net::cmd::SeatInputs;
use angel_net::knob::{self, Knob, Side};
use angel_net::proto::ToClient;
use std::sync::Arc;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant};

pub(crate) enum Inbound<C> {
    Joined { seat: u32, conn: u64, name: String, resumed: bool, welcomed: u32, local: bool },
    Dropped { seat: u32, conn: u64 },
    Left { seat: u32 },
    Cmds { seat: u32, conn: u64, applied: u32, cmds: Vec<(u32, C)> },
    Latency { seat: u32, conn: u64, round_trip: Duration },
    Action { seat: Option<u32>, conn: u64, local: bool, kind: u32, body: Vec<u8> },
    Tune { name: String, value: f64 },
}

#[derive(Clone, Copy, Debug, Default)]
struct SeatState {
    welcomed: u32,
    local: bool,
    loaded: bool,
    applied: u32,
    round_trip: Duration,
    last_cmd: u32,
}

const EXPIRE_EVERY: Duration = Duration::from_secs(1);
const MAX_LATE_TICKS: u32 = 5;
const SETTLE_WAIT: Duration = Duration::from_secs(1);

pub(crate) struct Stepper<G: GameServer> {
    game: G,
    hub: Arc<Hub<G>>,
    inbound: Receiver<Inbound<G::Cmd>>,
    inputs: Vec<SeatInputs<G::Cmd>>,
    playing: Vec<Option<u64>>,
    seated: Vec<bool>,
    presence: Vec<Presence>,
    seat_state: Vec<SeatState>,
    cmds: Vec<Option<G::Cmd>>,
    acks: Vec<u32>,
    tick: u32,
    step: Duration,
    every: u32,
    next: Option<Instant>,
    last_expire: Instant,
    engine: Engine,
    repeated: Vec<u32>,
    skipped: Vec<u32>,
    tick_ns: (u64, u64),
    ticks_counted: u32,
    tune_saves: u64,
    tune_written: Arc<std::sync::Mutex<u64>>,
}

impl<G: GameServer> Stepper<G> {
    pub(crate) fn new(game: G, hub: Arc<Hub<G>>, inbound: Receiver<Inbound<G::Cmd>>) -> Stepper<G> {
        let seats = hub.seats as usize;
        let engine = Engine::new(hub.rates.tick_hz, hub.rates.snapshot_hz);
        let step = Duration::from_secs(1) / hub.rates.tick_hz;
        let every = hub.rates.tick_hz / hub.rates.snapshot_hz;
        Stepper {
            game,
            hub,
            inbound,
            inputs: (0..seats).map(|_| SeatInputs::default()).collect(),
            playing: vec![None; seats],
            seated: vec![false; seats],
            presence: vec![Presence::default(); seats],
            seat_state: vec![SeatState::default(); seats],
            cmds: vec![None; seats],
            acks: vec![0; seats],
            tick: 0,
            step,
            every,
            next: None,
            last_expire: Instant::now(),
            engine,
            repeated: vec![0; seats],
            skipped: vec![0; seats],
            tick_ns: (0, 0),
            ticks_counted: 0,
            tune_saves: 0,
            tune_written: Arc::new(std::sync::Mutex::new(0)),
        }
    }

    fn knobs(&self) -> Vec<Knob> {
        let mut knobs = self.engine.knobs();
        knobs.extend(self.game.knobs().into_iter().filter(|k| k.side == Side::Server));
        knobs
    }

    fn set_knob(&mut self, name: &str, value: f64) -> Option<f64> {
        let value = self.knobs().into_iter().find(|k| k.name == name)?.fit(value)?;
        if self.set_engine_knob(name, value) || self.game.set_knob(name, value) {
            self.knobs_moved(true);
            Some(value)
        } else {
            None
        }
    }

    fn set_engine_knob(&mut self, name: &str, value: f64) -> bool {
        if !self.engine.set(name, value) {
            return false;
        }
        self.every = self.engine.snapshot_every;
        self.hub.every.store(self.every, SeqCst);
        true
    }

    fn knobs_moved(&mut self, tell: bool) {
        let knobs = self.knobs();
        if let Some(path) = &self.hub.tune_file
            && tell
        {
            self.tune_saves += 1;
            tune::save(path.clone(), tune::file_json(&knobs), self.tune_saves, Arc::clone(&self.tune_written));
        }
        *self.hub.knobs.lock().expect("knobs lock") = knobs;
        if tell {
            self.hub.tell_knobs();
        }
    }

    pub(crate) fn restore_knobs(&mut self) {
        if let Some(path) = self.hub.tune_file.clone() {
            for (name, value) in tune::load(&path) {
                let knob = self.knobs().into_iter().find(|k| k.name == name && k.value == k.default);
                if let Some(value) = knob.and_then(|k| k.fit(value)) {
                    let _ = self.set_engine_knob(&name, value) || self.game.set_knob(&name, value);
                }
            }
        }
        self.knobs_moved(false);
    }

    fn send_stats(&mut self) {
        let watching: Vec<u64> = {
            let conns = self.hub.conns.lock().expect("connections lock");
            let mut watching = self.hub.watching.lock().expect("watching lock");
            watching.retain(|id| conns.contains_key(id));
            watching.iter().copied().collect()
        };
        if !watching.is_empty() {
            let n = self.ticks_counted.max(1);
            let tick_ms = self.tick_ns.0 as f64 / f64::from(n) / 1e6;
            let tick_ms_max = self.tick_ns.1 as f64 / 1e6;
            let conns = self.hub.conns.lock().expect("connections lock");
            for id in watching {
                let Some(conn) = conns.get(&id) else { continue };
                let mut json = String::from("{\"tick_ms\":");
                knob::json_num((tick_ms * 1000.0).round() / 1000.0, &mut json);
                json.push_str(",\"tick_ms_max\":");
                knob::json_num((tick_ms_max * 1000.0).round() / 1000.0, &mut json);
                json.push_str(",\"snapshot_every\":");
                knob::json_num(f64::from(self.every), &mut json);
                if let Some(s) = conn.seat.map(|s| s as usize).filter(|&s| s < self.inputs.len()) {
                    json.push_str(&format!(
                        ",\"seat\":{s},\"queued\":{},\"repeated\":{},\"skipped\":{}",
                        self.inputs[s].queued(),
                        self.repeated[s],
                        self.skipped[s]
                    ));
                }
                json.push('}');
                conn.send(Out::Msg(ToClient::Stats { json: &json }.to_vec()));
            }
        }
        self.repeated.iter_mut().for_each(|n| *n = 0);
        self.skipped.iter_mut().for_each(|n| *n = 0);
        self.tick_ns = (0, 0);
        self.ticks_counted = 0;
    }

    pub(crate) fn run_to(&mut self, now: Instant, settle: bool) -> u32 {
        let step = self.step;
        let mut next = *self.next.get_or_insert(now + step);
        if now.saturating_duration_since(next) > step * MAX_LATE_TICKS {
            next = now;
        }
        let mut stepped = 0;
        while next <= now && !self.hub.stop.load(SeqCst) {
            next += step;
            self.next = Some(next);
            if settle {
                self.hub.settle_local(SETTLE_WAIT);
            }
            self.tick();
            stepped += 1;
        }
        self.next = Some(next);
        stepped
    }

    pub(crate) fn next_due(&self) -> Option<Instant> {
        self.next
    }

    pub(crate) fn publish_start(&mut self) {
        self.tick = 1;
        let body = self.game.publish(self.tick);
        self.hub.publish(TickOut {
            tick: self.tick,
            snapshot: true,
            acks: self.acks.clone(),
            playing: self.playing.clone(),
            body,
            events: Vec::new(),
        });
    }

    fn tick(&mut self) {
        let started = Instant::now();
        self.tick = self.tick.wrapping_add(1);
        let mut events = Events::default();
        let mut engine_tunes: Vec<(u64, bool, String, f64)> = Vec::new();
        let game = &mut self.game;
        while let Ok(msg) = self.inbound.try_recv() {
            match msg {
                Inbound::Joined { seat, conn, name, resumed, welcomed, local } => {
                    let s = seat as usize;
                    self.playing[s] = Some(conn);
                    self.inputs[s].restart();
                    self.inputs[s].set_cushion(if local { 0 } else { angel_net::cmd::CUSHION });
                    self.seat_state[s] = SeatState { welcomed, local, loaded: local, applied: 0, round_trip: Duration::ZERO, last_cmd: self.tick };
                    if !resumed || !self.seated[s] {
                        if self.seated[s] {
                            game.leave(seat, &mut events);
                        }
                        game.join(seat, &name, &mut events);
                        self.seated[s] = true;
                    }
                }
                Inbound::Dropped { seat, conn } => {
                    if self.playing[seat as usize] == Some(conn) {
                        self.playing[seat as usize] = None;
                        self.seat_state[seat as usize].loaded = false;
                    }
                }
                Inbound::Left { seat } => {
                    let s = seat as usize;
                    if self.seated[s] {
                        game.leave(seat, &mut events);
                    }
                    self.seated[s] = false;
                    self.playing[s] = None;
                    self.seat_state[s] = SeatState::default();
                    self.inputs[s] = SeatInputs::default();
                }
                Inbound::Latency { seat, conn, round_trip } => {
                    if self.playing[seat as usize] == Some(conn) {
                        self.seat_state[seat as usize].round_trip = round_trip;
                    }
                }
                Inbound::Cmds { seat, conn, applied, cmds } => {
                    if self.playing[seat as usize] == Some(conn) {
                        let state = &mut self.seat_state[seat as usize];
                        state.applied = state.applied.max(applied);
                        state.last_cmd = self.tick;
                        if state.applied >= state.welcomed {
                            state.loaded = true;
                        }
                        for (seq, cmd) in cmds {
                            self.inputs[seat as usize].push(seq, cmd);
                        }
                    }
                }
                Inbound::Action { kind, conn, local, body, .. } if knob::action::engine(kind) => {
                    if kind == knob::action::TUNE
                        && let Ok((name, value)) = knob::read_tune(&body)
                    {
                        engine_tunes.push((conn, local, name.to_string(), value));
                    }
                }
                Inbound::Action { seat, conn, kind, body, .. } => {
                    if seat.is_none_or(|s| self.playing[s as usize] == Some(conn)) {
                        game.action(seat, kind, &body, &mut events);
                    }
                }
                Inbound::Tune { name, value } => engine_tunes.push((0, true, name, value)),
            }
        }
        for (conn, local, name, value) in engine_tunes {
            if self.hub.may_tune(local) {
                if self.set_knob(&name, value).is_none() {
                    self.hub.tell_one(conn, &format!("no knob {name} on this server"));
                }
            } else {
                self.hub.tell_one(conn, "only the host sets the game's knobs (or start the session in dev mode)");
                self.hub.tell_knobs_to(conn);
            }
        }
        let game = &mut self.game;
        if started.duration_since(self.last_expire) >= EXPIRE_EVERY {
            self.last_expire = started;
            let mut sessions = self.hub.sessions.lock().expect("sessions lock");
            for seat in sessions.expire(started) {
                let _ = self.hub.inbound.send(Inbound::Left { seat });
            }
        }

        let mut counts = angel_net::cmd::Counts::default();
        for s in 0..self.inputs.len() {
            if self.seated[s] {
                let (cmd, ack) = self.inputs[s].take();
                if self.playing[s].is_some() {
                    let was = self.acks[s];
                    if ack == was {
                        self.repeated[s] += 1;
                    } else if ack > was {
                        self.skipped[s] += ack - was - 1;
                    }
                }
                self.cmds[s] = Some(cmd);
                self.acks[s] = ack;
                counts.add(&self.inputs[s].take_counts());
            } else {
                self.cmds[s] = None;
                self.acks[s] = 0;
            }
        }
        self.hub.tele.inputs.lock().expect("telemetry lock").add(&counts);
        for (s, here) in self.presence.iter_mut().enumerate() {
            let state = &self.seat_state[s];
            let connected = self.playing[s].is_some();
            *here = Presence {
                seated: self.seated[s],
                connected,
                playing: connected
                    && (state.local
                        || (state.loaded && self.tick.wrapping_sub(state.last_cmd) <= AWAY_AFTER_TICKS)),
                applied: state.applied,
            };
        }
        for (seat, state) in self.seat_state.iter().enumerate() {
            let allowance = angel_net::cmd::sample_allowance(
                state.round_trip, game.rates().tick_hz, self.every, state.local,
            );
            game.input_age(seat as u32, allowance);
        }
        game.presence(self.tick, &self.presence);
        game.step(self.tick, &self.cmds, &mut events);
        let stepped = Instant::now();
        let body = game.publish(self.tick);
        self.hub.publish(TickOut {
            tick: self.tick,
            snapshot: self.tick.is_multiple_of(self.every),
            acks: self.acks.clone(),
            playing: self.playing.clone(),
            body,
            events: events.take(),
        });
        let done = Instant::now();
        let tele = &self.hub.tele;
        tele.step.record(stepped - started);
        tele.publish.record(done - stepped);
        tele.tick.record(done - started);
        tele.ticks.fetch_add(1, SeqCst);
        let ns = (done - started).as_nanos().min(u128::from(u64::MAX)) as u64;
        self.tick_ns = (self.tick_ns.0.saturating_add(ns), self.tick_ns.1.max(ns));
        self.ticks_counted += 1;
        if self.ticks_counted >= self.hub.rates.tick_hz {
            self.send_stats();
        }
    }
}

pub(crate) fn run<G: GameServer>(mut stepper: Stepper<G>) {
    while !stepper.hub.stop.load(SeqCst) {
        stepper.run_to(Instant::now(), false);
        let due = stepper.next_due().unwrap_or_else(Instant::now);
        let wait = due.saturating_duration_since(Instant::now());
        if !wait.is_zero() {
            thread::sleep(wait);
        }
    }
}

pub struct Manual<G: GameServer> {
    stepper: Stepper<G>,
}

impl<G: GameServer> Manual<G> {
    pub(crate) fn new(stepper: Stepper<G>) -> Manual<G> {
        Manual { stepper }
    }

    pub fn run_to(&mut self, now: Instant) -> u32 {
        self.stepper.run_to(now, true)
    }

    pub fn next_due(&self) -> Option<Instant> {
        self.stepper.next_due()
    }

    pub fn game(&self) -> &G {
        &self.stepper.game
    }

    pub fn game_mut(&mut self) -> &mut G {
        &mut self.stepper.game
    }
}
