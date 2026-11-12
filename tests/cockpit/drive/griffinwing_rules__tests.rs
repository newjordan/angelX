//! Griffinwing's tick, held to the code it replaced.
//!
//! The rules live in Bend now (`cockpit/live/griffinwing-rules`) and
//! `crates/delve/src/rules/griffinwing.rs` keeps the session's types, its saves
//! and its wire, and the two asks of the module. What is left of the native rules
//! is `griffinwing_oracle.rs` beside this file, the code as it stood, and these
//! tests tick the shipped module and that oracle side by side: two sessions from
//! one start take the same keys, one through `Session::step` (the module), one
//! through the oracle, and after every tick the whole session, the sounds and the
//! knights who left must agree to the byte. The bots' hands (`bot_with`) are held
//! the same way. The measurements at the end are `--ignored`.
use angel_delve::rules::griffinwing::*;
use angel_delve::rules::{HZ, Hero, Input, MAX_PLAYERS, Run};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[path = "griffinwing_oracle.rs"]
mod oracle;
use oracle::Rules;

type Inputs = BTreeMap<u32, Input>;
type Heroes = BTreeMap<u32, Hero>;

fn engine() {
    crate::agent::harness::live_policy::install_scripted_rules();
}

/// A table of `seats` knights.
fn table(seats: u32) -> Heroes {
    engine();
    let mut run = Run::new(7, 1, None);
    for id in 2..=seats {
        run.join_seat(id, &format!("Knight {id}"));
    }
    run.players.clone()
}

/// What the tests saw the sessions do, to prove the scripts reached the branches they name.
#[derive(Debug, Default)]
struct Seen {
    ticks: u32,
    modes: BTreeSet<String>,
    restarts: u32,
    leaves: u32,
    foes: BTreeSet<String>,
    shots: BTreeSet<String>,
    flares: BTreeSet<String>,
    wyrm: BTreeSet<String>,
    lines: BTreeSet<String>,
    sounds: BTreeSet<&'static str>,
    things: BTreeSet<String>,
    medals: BTreeSet<String>,
    lost: u32,
    downs: u32,
    locks: u32,
    stages: BTreeSet<u8>,
}

impl Seen {
    fn look(&mut self, s: &Session, out: &Out, before_mode: Mode) {
        self.ticks += 1;
        self.modes.insert(format!("{:?}", s.mode));
        self.restarts += u32::from(before_mode == Mode::Retry && s.mode == Mode::Play);
        self.leaves += out.leave.len() as u32;
        self.stages.insert(s.stage);
        for f in &s.foes {
            self.foes.insert(format!("{:?}/{:?}", f.kind, f.st));
        }
        for b in &s.bolts {
            self.shots.insert(format!("{:?}", b.shot));
        }
        for f in &s.fx {
            self.flares.insert(format!("{:?}", f.what));
        }
        for c in &s.calls {
            self.lines.insert(format!("{:?}", c.line));
        }
        for p in &s.props {
            self.things.insert(format!("{:?}", p.thing));
        }
        if let Some(b) = &s.boss {
            self.wyrm
                .insert(format!("{}/{:?}/{}", b.phase, b.st, b.atk));
        }
        for n in &out.sounds {
            self.sounds.insert(n);
        }
        if let Some(m) = s.medal {
            self.medals.insert(format!("{m:?}"));
        }
        self.lost += s.wing.iter().filter(|w| w.lost).count() as u32;
        self.downs += s.seats.values().filter(|x| x.craft.down > 0).count() as u32;
        self.locks += s
            .seats
            .values()
            .map(|x| x.craft.locks.len() as u32)
            .sum::<u32>();
    }
    fn add(&mut self, o: Seen) {
        self.ticks += o.ticks;
        self.restarts += o.restarts;
        self.leaves += o.leaves;
        self.lost += o.lost;
        self.downs += o.downs;
        self.locks += o.locks;
        self.modes.extend(o.modes);
        self.foes.extend(o.foes);
        self.shots.extend(o.shots);
        self.flares.extend(o.flares);
        self.wyrm.extend(o.wyrm);
        self.lines.extend(o.lines);
        self.sounds.extend(o.sounds);
        self.things.extend(o.things);
        self.medals.extend(o.medals);
        self.stages.extend(o.stages);
    }
}

/// The first place two sessions differ, as a path into their JSON.
fn first_difference(a: &Value, b: &Value, path: &str) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for k in x.keys().chain(y.keys()) {
                let (p, q) = (
                    x.get(k).unwrap_or(&Value::Null),
                    y.get(k).unwrap_or(&Value::Null),
                );
                if let Some(d) = first_difference(p, q, &format!("{path}.{k}")) {
                    return Some(d);
                }
            }
            None
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                return Some(format!("{path}: {} rows against {}", x.len(), y.len()));
            }
            x.iter()
                .zip(y)
                .enumerate()
                .find_map(|(i, (p, q))| first_difference(p, q, &format!("{path}[{i}]")))
        }
        _ => (a != b).then(|| format!("{path}: {a} against {b}")),
    }
}

/// Two sessions from one start, one ticked by the module and one by the oracle.
struct Pair {
    native: Session,
    bent: Session,
    heroes: Heroes,
    nums: Nums,
    seen: Seen,
    at: String,
    /// Set once the oracle's session is one a run may not hold (`Session::valid`): the adapter
    /// refuses such an answer, so the raw answers are held to each other once and the pair stops.
    ended: bool,
    refused: Vec<String>,
}

impl Pair {
    fn new(seats: u32, nums: Nums, at: &str) -> Pair {
        engine();
        let heroes = table(seats);
        let s = Session::new(&heroes);
        Pair {
            native: s.clone(),
            bent: s,
            heroes,
            nums,
            seen: Seen::default(),
            at: at.into(),
            ended: false,
            refused: Vec::new(),
        }
    }
    fn from(s: Session, heroes: Heroes, nums: Nums, at: &str) -> Pair {
        engine();
        Pair {
            native: s.clone(),
            bent: s,
            heroes,
            nums,
            seen: Seen::default(),
            at: at.into(),
            ended: false,
            refused: Vec::new(),
        }
    }
    fn here(&self) -> Vec<u32> {
        self.heroes
            .iter()
            .filter(|(_, h)| !h.away && !h.stone)
            .map(|(&id, _)| id)
            .collect()
    }
    /// One tick on each side, and every word of them compared.
    fn tick(&mut self, inputs: &Inputs) -> Out {
        let keys: Inputs = inputs
            .iter()
            .filter(|(_, i)| i.valid())
            .map(|(&k, &i)| (k, i))
            .collect();
        let before = self.native.mode;
        let asked = self.native.clone();
        if self.native != self.bent {
            let (a, b) = (
                serde_json::to_value(&self.native).unwrap(),
                serde_json::to_value(&self.bent).unwrap(),
            );
            panic!(
                "{} tick {}: the sessions were not the same before the tick: {:?}",
                self.at,
                self.seen.ticks,
                first_difference(&a, &b, "")
            );
        }
        let want = self.native.step_here(&keys, &self.here(), &self.nums);
        if !self.native.valid() {
            return self.refuse(&asked, &keys, want);
        }
        let got = self.bent.step(inputs, &self.heroes, &self.nums);
        // What the picture remembers (`show`) is the session's own: the module
        // is never sent it and the oracle's step never writes it, so it is no
        // part of the parity between them.
        self.bent.show = Default::default();
        if self.native != self.bent || want.sounds != got.sounds || want.leave != got.leave {
            let (a, b) = (
                serde_json::to_value(&self.native).unwrap(),
                serde_json::to_value(&self.bent).unwrap(),
            );
            // GRIFFINWING_DUMP=<file> keeps the tick that parted, as the module is asked it
            if let Ok(path) = std::env::var("GRIFFINWING_DUMP") {
                let ask = serde_json::json!({"op": "tick", "session": asked, "keys": keys, "here": self.here(), "nums": self.nums});
                let sounds: Vec<usize> = want
                    .sounds
                    .iter()
                    .filter_map(|n| SOUNDS.split(' ').position(|s| s == *n))
                    .collect();
                let line = serde_json::json!({"ask": ask, "answer": {"session": a, "sounds": sounds, "leave": want.leave}});
                std::fs::write(path, format!("{line}\n")).unwrap();
            }
            panic!(
                "{} tick {}: the module and the oracle part: {} (sounds {:?} against {:?}, leave {:?} against {:?})",
                self.at,
                self.seen.ticks,
                first_difference(&a, &b, "").unwrap_or_else(|| "the sounds or the leaves".into()),
                want.sounds,
                got.sounds,
                want.leave,
                got.leave
            );
        }
        self.seen.look(&self.native, &want, before);
        want
    }
    /// The oracle made a session `valid` refuses: the module is asked directly, and its answer must be the oracle's.
    fn refuse(&mut self, asked: &Session, keys: &Inputs, want: Out) -> Out {
        let ask = serde_json::json!({"op": "tick", "session": asked, "keys": keys, "here": self.here(), "nums": self.nums});
        let raw: Value = angel_delve::scripted::step(MODULE, &ask).expect("the module answers");
        let native = serde_json::to_value(&self.native).unwrap();
        if let Some(d) = first_difference(&raw["session"], &native, "") {
            panic!(
                "{} tick {}: the module and the oracle part on a session `valid` refuses: {d}",
                self.at, self.seen.ticks
            );
        }
        let sounds: Vec<usize> = want
            .sounds
            .iter()
            .filter_map(|n| SOUNDS.split(' ').position(|s| s == *n))
            .collect();
        assert_eq!(
            raw["sounds"],
            serde_json::json!(sounds),
            "{}: the sounds of a refused tick",
            self.at
        );
        let mut mended = self.native.clone();
        mended.repair();
        self.refused.push(format!(
            "{}: {}",
            self.at,
            first_difference(&native, &serde_json::to_value(&mended).unwrap(), "")
                .unwrap_or_default()
        ));
        self.ended = true;
        want
    }
    /// The bots' hands on this tick, from both sides (and held to each other).
    fn hands(&mut self, skill: bot::Skill) -> Inputs {
        let ids: Vec<u32> = self.native.seats.keys().copied().collect();
        let mut out = Inputs::new();
        for id in ids {
            let want = self.native.bot_native(id, &self.nums, skill);
            let got = self.bent.bot_with(id, &self.nums, skill);
            assert_eq!(
                want, got,
                "{} seat {id} at tick {}: the bot's hands part",
                self.at, self.seen.ticks
            );
            out.insert(id, want);
        }
        out
    }
}

struct Xs(u64);
impl Xs {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo + 1) as u64) as i32
    }
    fn chance(&mut self, per: u64) -> bool {
        self.next() % per == 0
    }
    fn pick<T: Copy>(&mut self, all: &[T]) -> T {
        all[(self.next() % all.len() as u64) as usize]
    }
}

fn hand(x: &mut Xs, hold: bool) -> Input {
    let r = x.next();
    let axis = |v: u64| ((v % 3) as i8) - 1;
    Input {
        move_x: axis(r),
        move_y: axis(r >> 3),
        aim_x: axis(r >> 6),
        aim_y: axis(r >> 9),
        fire: if hold {
            (r >> 12) % 20 != 0
        } else {
            (r >> 12) % 3 == 0
        },
        dash: (r >> 14) % 17 == 0,
        bomb: (r >> 18) % 61 == 0,
        interact: (r >> 22) % 5 == 0,
        swing: (r >> 26) % 4 == 0,
        ..Input::default()
    }
}

/// Bots fly the adventure, the tally and the leaving.
fn flown(seats: u32, skill: bot::Skill, nums: Nums, limit: u32) -> Seen {
    let mut pair = Pair::new(seats, nums, &format!("{seats} {skill:?} bots"));
    let mut fin = 0;
    for _ in 0..limit {
        let inputs = pair.hands(skill);
        let out = pair.tick(&inputs);
        assert!(
            !pair.ended,
            "a bot's flight left the sessions a run may hold: {:?}",
            pair.refused
        );
        if pair.native.finished() {
            fin += 1;
            if !out.leave.is_empty() || fin > 200 {
                break;
            }
        }
    }
    assert!(
        pair.native.finished() && pair.native.stage == 2,
        "{} seats did not finish",
        seats
    );
    pair.seen
}

/// Random hands and a table that comes and goes, from a start somewhere.
fn wandered(seats: u32, seed: u64, start: u8, ticks: u32) -> Seen {
    let mut pair = Pair::new(
        seats,
        Nums::default(),
        &format!("{seats} random hands, seed {seed}"),
    );
    let mut x = Xs(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    for s in [&mut pair.native, &mut pair.bent] {
        match start {
            1 => s.stage = 1,
            2 => {
                s.stage = 2;
                s.t = rail::BOSS_AT - 40;
                s.cp = rail::BOSS_AT;
                s.next = oracle::resume(2, rail::BOSS_AT - 40);
            }
            3 => {
                s.stage = 1;
                s.t = 900;
                s.next = oracle::resume(1, 900);
            }
            _ => {}
        }
    }
    for tick in 0..ticks {
        let hold = (tick / 400) % 2 == 1;
        let mut inputs = Inputs::new();
        for &id in pair.heroes.keys() {
            if !x.chance(9) {
                inputs.insert(id, hand(&mut x, hold));
            }
        }
        if x.chance(50) {
            inputs.insert(
                1,
                Input {
                    move_x: 3,
                    ..Input::default()
                },
            );
        }
        if tick % 300 == 299 {
            let id = 1 + (x.next() % u64::from(seats)) as u32;
            if let Some(h) = pair.heroes.get_mut(&id) {
                h.away = !h.away;
            }
        }
        if tick % 700 == 650 || tick % 700 == 652 {
            let away = tick % 700 == 650;
            pair.heroes.values_mut().for_each(|h| h.away = away);
        }
        pair.tick(&inputs);
        assert!(
            !pair.ended,
            "random hands left the sessions a run may hold: {:?}",
            pair.refused
        );
    }
    pair.seen
}

/// A table that fires and nothing else: it is wiped, again and again.
fn idle(seats: u32, ticks: u32, from_the_roost: bool) -> Seen {
    let mut pair = Pair::new(seats, Nums::default(), &format!("{seats} idle"));
    if from_the_roost {
        for s in [&mut pair.native, &mut pair.bent] {
            s.stage = 2;
            s.t = rail::BOSS_AT - 30;
            s.cp = rail::BOSS_AT;
            s.next = oracle::resume(2, rail::BOSS_AT - 30);
        }
    }
    for tick in 0..ticks {
        let mut inputs = Inputs::new();
        if tick % 3 == 0 {
            for &id in pair.heroes.keys() {
                inputs.insert(
                    id,
                    Input {
                        fire: true,
                        ..Input::default()
                    },
                );
            }
        }
        pair.tick(&inputs);
        assert!(
            !pair.ended,
            "an idle table left the sessions a run may hold: {:?}",
            pair.refused
        );
    }
    pair.seen
}

/// A session of odd, valid states: foes of every kind in every stance, bolts of every arm, the Wyrm in
/// every state, knights down, rolling, holding a lock, a table of any size.
fn odd(x: &mut Xs, seats: u32, heroes: &Heroes) -> Session {
    let nums = Nums::default();
    let mut s = Session::new(heroes);
    s.stage = (x.next() % 3) as u8;
    s.t = match s.stage {
        2 => x.range(rail::BOSS_AT as i32 - 100, rail::BOSS_AT as i32 + 900) as u32,
        k => x.range(0, rail::LEN[k as usize] as i32 - 5) as u32,
    };
    s.next = if x.chance(6) {
        x.range(0, 30) as u16
    } else {
        oracle::resume(s.stage, s.t)
    };
    s.cp = if s.stage == 2 { rail::BOSS_AT } else { 0 };
    s.elapsed = s.t + x.range(0, 5000) as u32;
    s.ids = x.range(1, 59_999) as u16;
    s.retries = x.range(0, 3) as u8;
    s.first = x.chance(2);
    s.bank = Tally {
        pts: x.range(0, 4000) as u32,
        base: x.range(0, 3000) as u32,
        offered: x.range(3000, 5000) as u32,
        kills: x.range(0, 60) as u32,
        saved: x.range(0, 3) as u16,
        lost: x.range(0, 3) as u16,
        gates: x.range(0, 9) as u16,
    };
    s.beat = Tally {
        pts: x.range(0, 900) as u32,
        base: x.range(0, 800) as u32,
        offered: x.range(0, 1000) as u32,
        kills: x.range(0, 20) as u32,
        saved: x.range(0, 2) as u16,
        lost: x.range(0, 2) as u16,
        gates: x.range(0, 4) as u16,
    };
    s.mode = match x.next() % 10 {
        0 => Mode::Retry,
        1 => Mode::Victory,
        2 => Mode::Tally,
        _ => Mode::Play,
    };
    s.mode_t = match s.mode {
        Mode::Retry => x.range(1, 24) as u32,
        Mode::Victory => x.range(0, 215) as u32,
        Mode::Tally => x.range(0, 90) as u32,
        Mode::Play => 0,
    };
    if s.mode != Mode::Play {
        s.stage = 2;
        s.t = rail::BOSS_AT + 700;
        s.done = Some(s.elapsed.saturating_sub(5));
        s.medal =
            (s.mode == Mode::Tally).then(|| x.pick(&[Medal::Bronze, Medal::Silver, Medal::Gold]));
        s.home = s.anchor();
    }
    let craft = |x: &mut Xs, c: &mut Craft| {
        c.x = x.range(-nums.wx(), nums.wx());
        c.z = x.range(-nums.wz(), nums.wz());
        c.vx = x.range(-70, 70);
        c.vz = x.range(-60, 60);
        c.bank = x.range(-256, 256);
        c.lift = x.range(-256, 256);
        c.back = x.range(-dm(45), dm(55));
        c.ds = x.range(40, 140);
        c.rx = x.range(-dm(120), dm(120));
        c.rz = x.range(-dm(80), dm(80));
        c.shield = if x.chance(6) {
            x.range(0, 12)
        } else {
            x.range(1, SHIELD)
        };
        c.hurt = if x.chance(3) {
            x.range(0, 40) as u32
        } else {
            0
        };
        c.down = if x.chance(10) {
            x.range(1, 75) as u32
        } else {
            0
        };
        c.roll = if x.chance(5) {
            x.range(1, 30) as u32
        } else {
            0
        };
        c.roll_len = if c.roll > 0 {
            c.roll + x.range(0, 10) as u32
        } else {
            x.range(0, 30) as u32
        };
        c.dir = if x.chance(2) { 1 } else { -1 };
        c.rest = if x.chance(4) {
            x.range(1, 40) as u32
        } else {
            0
        };
        if x.chance(3) {
            c.held = x.range(1, 60) as u32;
            c.charge = x.range(0, 40) as u32;
            c.dwell = x.range(0, 9) as u32;
        }
        c.cool = if x.chance(4) {
            x.range(1, 12) as u32
        } else {
            0
        };
        c.meter = if x.chance(3) {
            x.range(0, METER)
        } else {
            METER
        };
        c.spent = x.chance(5);
        c.boost = x.chance(6);
        c.brake = x.chance(6);
        c.bombs = x.range(0, 3) as u8;
        c.keys = (x.next() % 32) as u8;
        c.tap_dir = x.range(-1, 1) as i8;
        c.tap_ticks = x.range(0, 9) as u8;
        c.last_x = x.range(-1, 1) as i8;
        c.chain = x.range(0, 6) as u16;
        c.chain_ticks = if c.chain > 0 {
            x.range(1, 75) as u32
        } else {
            0
        };
        c.pts = x.range(0, 3000) as u32;
        c.hits = x.range(0, 90) as u32;
        c.saved = x.range(0, 4) as u16;
        c.gates = x.range(0, 6) as u16;
        c.locks.clear();
    };
    for seat in s.seats.values_mut() {
        craft(x, &mut seat.craft);
    }
    for w in &mut s.wing {
        craft(x, &mut w.craft);
        w.craft.x = x.range(-dm(60), dm(60));
        w.craft.z = x.range(-dm(20), dm(30));
        w.craft.back = x.range(0, dm(80));
        w.craft.down = 0;
        w.craft.shield = if x.chance(5) {
            x.range(0, 10)
        } else {
            x.range(10, SHIELD)
        };
        w.lost = x.chance(5);
        w.fire = x.range(0, 30) as u32;
    }
    let a = s.anchor();
    let ids = |s: &Session| -> Vec<u8> {
        s.seats
            .keys()
            .map(|&k| k as u8)
            .chain(s.wing.iter().map(|w| w.id))
            .collect()
    };
    for _ in 0..x.range(0, 14) {
        let kind = x.pick(&[Kind::Wisp, Kind::Kite, Kind::Turret, Kind::Bomber]);
        let st = match kind {
            Kind::Wisp => x.pick(&[St::Fly, St::Fly, St::Tell, St::Flee]),
            Kind::Kite => x.pick(&[
                St::Fly,
                St::Hunt,
                St::Hunt,
                St::Latched,
                St::Latched,
                St::Flee,
            ]),
            Kind::Turret => x.pick(&[St::Rest, St::Tell]),
            _ => x.pick(&[St::Hide, St::Tell, St::Dive]),
        };
        let mode = if kind == Kind::Wisp {
            x.range(0, 3) as u8
        } else {
            x.range(0, 1) as u8
        };
        let at = (
            a + x.range(-dm(100), dm(700)),
            x.range(-dm(100), dm(100)),
            x.range(-dm(40), dm(60)),
        );
        let vel = (x.range(-30, 10), x.range(-50, 50), x.range(-20, 20));
        if let Some(id) = s.spawn_foe(kind, at, vel, st, mode, x.range(0, 40) as u8) {
            let who = ids(&s);
            let f = s.foes.iter_mut().find(|f| f.id == id).unwrap();
            f.ticks = x.range(0, 110) as u32;
            f.fx = x.range(-dm(80), dm(80));
            f.fz = x.range(-dm(30), dm(50));
            f.flash = if x.chance(6) { x.range(1, 5) as u8 } else { 0 };
            f.hp = x.range(1, f.full as i32) as u32;
            if matches!(st, St::Hunt | St::Latched) && !who.is_empty() {
                f.on = x.pick(&who);
            }
            if x.chance(7) {
                // provisioned for one seat: the table that now sits down scales it up by its share
                f.seats = 1;
                f.full = kind.health();
                f.hp = f.hp.min(f.full);
            }
        }
    }
    let owners = ids(&s);
    let foe_ids: Vec<u16> = s.foes.iter().map(|f| f.id).collect();
    for _ in 0..x.range(0, 36) {
        let shot = x.pick(&[
            Arm::Lance,
            Arm::Star,
            Arm::Ally,
            Arm::Spark,
            Arm::Gloam,
            Arm::Turned,
        ]);
        let friendly = shot != Arm::Spark && shot != Arm::Gloam;
        let aim = if matches!(shot, Arm::Star | Arm::Turned) && !foe_ids.is_empty() && x.chance(2) {
            x.pick(&foe_ids)
        } else {
            0
        };
        let src = if !foe_ids.is_empty() && x.chance(2) {
            x.pick(&foe_ids)
        } else {
            0
        };
        s.bolts.push(Bolt {
            shot,
            owner: if friendly { x.pick(&owners) } else { 0 },
            s: a + x.range(-dm(30), dm(600)),
            x: x.range(-dm(90), dm(90)),
            z: x.range(-dm(40), dm(50)),
            vs: x.range(0, 220),
            vx: x.range(-90, 90),
            vz: x.range(-60, 60),
            dmg: x.range(4, 40) as u16,
            life: x.range(1, 220) as u16,
            aim,
            src,
            delay: if shot == Arm::Star && x.chance(3) {
                x.range(1, 8) as u8
            } else {
                0
            },
            full: x.chance(2),
        });
    }
    for _ in 0..x.range(0, 8) {
        let thing = x.pick(&[
            Thing::Gate,
            Thing::Ring,
            Thing::Arch,
            Thing::Ridge,
            Thing::Shutter,
        ]);
        s.props.push(Prop {
            thing,
            s: a + x.range(-dm(40), dm(300)),
            x: x.range(-dm(60), dm(60)),
            z: x.range(-dm(10), dm(40)),
            w: x.range(dm(10), dm(30)),
            h: x.range(dm(10), dm(50)),
            len: if matches!(thing, Thing::Gate | Thing::Ring) {
                0
            } else {
                x.range(dm(20), dm(60))
            },
            taken: (x.next() % 8) as u8,
            phase: x.range(0, 150) as u16,
        });
    }
    for _ in 0..x.range(0, 4) {
        s.calls.push(Call {
            who: x.pick(&owners),
            line: x.pick(&[Line::Hello, Line::Hold, Line::Low, Line::Open, Line::Chain]),
            ticks: x.range(1, 100) as u8,
        });
    }
    for _ in 0..x.range(0, 12) {
        s.fx.push(Flare {
            what: x.pick(&[Burst::Pop, Burst::Hit, Burst::Lock, Burst::Fall]),
            s: a + x.range(0, dm(300)),
            x: x.range(-dm(50), dm(50)),
            z: x.range(0, dm(30)),
            ticks: x.range(1, 40) as u8,
            n: 1,
        });
    }
    if s.stage == 2 && (s.t >= rail::BOSS_AT || x.chance(3)) {
        s.rise();
        s.snd.clear();
        if let Some(b) = &mut s.boss {
            b.phase = x.range(0, 2) as u8;
            b.st = x.pick(&[
                Wyrm::Rise,
                Wyrm::Idle,
                Wyrm::Idle,
                Wyrm::Tell,
                Wyrm::Tell,
                Wyrm::Coil,
                Wyrm::Roar,
                Wyrm::Hurt,
                Wyrm::Dying,
            ]);
            if b.phase == 2 && b.st == Wyrm::Hurt {
                // the last lamp's fall is the Wyrm's death, not a third phase
                b.st = Wyrm::Dying;
            }
            b.ticks = x.range(0, 70) as u32;
            b.atk = x.range(0, 2) as u8;
            b.seq = x.range(0, 9) as u8;
            b.gap = x.range(-dm(60), dm(60));
            b.col = x.range(-dm(60), dm(60));
            b.side = if x.chance(2) { 1 } else { -1 };
            if b.st == Wyrm::Tell {
                b.marks = vec![
                    (x.range(-dm(60), dm(60)), x.range(-dm(30), dm(30)));
                    x.range(1, 4) as usize
                ];
            }
        }
        let (lamps, phase) = {
            let b = s.boss.as_ref().unwrap();
            (b.lamps, b.phase)
        };
        for (k, id) in lamps.iter().enumerate() {
            if let Some(f) = s.foes.iter_mut().find(|f| f.id == *id) {
                f.st = if (k as u8) < phase {
                    St::Broken
                } else {
                    x.pick(&[St::Shut, St::Shut, St::Open])
                };
                if f.st == St::Broken {
                    f.hp = 0;
                }
                f.ticks = x.range(1, 150) as u32;
            }
        }
        if x.chance(4) {
            if let Some(f) = s
                .foes
                .iter_mut()
                .find(|f| f.id == lamps[usize::from(phase.min(2))])
            {
                f.st = St::Broken;
                f.hp = 0;
            }
        }
    }
    let _ = seats;
    s
}

fn nums_of(x: &mut Xs) -> Nums {
    if x.chance(4) {
        Nums {
            charge: x.range(20, 90) as u32,
            roll: x.range(18, 60) as u32,
            window: x.range(40, 90) as u32,
            spur: x.range(30, 180) as u32,
            tell: x.range(40, 120) as u32,
        }
    } else {
        Nums::default()
    }
}

/// Odd states, a few ticks each, with the bots' hands asked of every one.
fn strange(seed: u64, sessions: u32) -> Seen {
    let mut x = Xs(seed);
    let mut seen = Seen::default();
    let mut refused = Vec::new();
    let mut made = 0;
    while made < sessions {
        let seats = 1 + (x.next() % 6) as u32;
        let heroes = table(seats);
        let s = odd(&mut x, seats, &heroes);
        if !s.valid() {
            continue;
        }
        made += 1;
        let mut pair = Pair::from(s, heroes, nums_of(&mut x), &format!("odd session {made}"));
        for tick in 0..8 {
            let mut inputs = Inputs::new();
            for &id in pair.heroes.keys() {
                if !x.chance(8) {
                    let hold = x.chance(2);
                    inputs.insert(id, hand(&mut x, hold));
                }
            }
            if x.chance(6) {
                let id = 1 + (x.next() % u64::from(seats)) as u32;
                if let Some(h) = pair.heroes.get_mut(&id) {
                    h.away = !h.away;
                }
            }
            if tick % 3 == 0 {
                pair.hands(bot::Skill::Sure);
                pair.hands(bot::Skill::Novice);
            }
            pair.tick(&inputs);
            if pair.ended {
                break;
            }
        }
        refused.extend(pair.refused);
        seen.add(pair.seen);
    }
    eprintln!(
        "{} odd sessions went past what a session may hold, e.g. {:?}",
        refused.len(),
        refused.first()
    );
    seen
}

fn assert_reached(seen: &Seen) {
    for m in ["Play", "Retry", "Victory", "Tally"] {
        assert!(
            seen.modes.contains(m),
            "no tick in the {m} mode: {:?}",
            seen.modes
        );
    }
    assert_eq!(seen.stages, BTreeSet::from([0, 1, 2]));
    assert!(
        seen.restarts > 0 && seen.leaves > 0 && seen.lost > 0 && seen.downs > 0 && seen.locks > 0
    );
    for f in [
        "Wisp/Fly",
        "Wisp/Tell",
        "Wisp/Flee",
        "Kite/Fly",
        "Kite/Hunt",
        "Kite/Latched",
        "Kite/Flee",
        "Turret/Rest",
        "Turret/Tell",
        "Bomber/Hide",
        "Bomber/Tell",
        "Bomber/Dive",
        "Lamp/Shut",
        "Lamp/Open",
        "Lamp/Broken",
    ] {
        assert!(seen.foes.contains(f), "no foe was {f}: {:?}", seen.foes);
    }
    for b in ["Lance", "Star", "Ally", "Spark", "Gloam", "Turned"] {
        assert!(seen.shots.contains(b), "no {b} bolt flew: {:?}", seen.shots);
    }
    for f in [
        "Pop", "Hit", "Turn", "Lock", "Gate", "Scrape", "Fall", "Hurt", "Lamp", "Starfall",
    ] {
        assert!(seen.flares.contains(f), "no {f} flash: {:?}", seen.flares);
    }
    for st in ["Rise", "Idle", "Tell", "Coil", "Roar", "Hurt", "Dying"] {
        assert!(
            seen.wyrm.iter().any(|w| w.contains(&format!("/{st}/"))),
            "the Wyrm was never {st}: {:?}",
            seen.wyrm
        );
    }
    for t in ["Gate", "Ring", "Arch", "Ridge", "Shutter"] {
        assert!(
            seen.things.contains(t),
            "no {t} stood on the rail: {:?}",
            seen.things
        );
    }
    assert!(seen.medals.len() >= 2, "{:?}", seen.medals);
    assert!(
        seen.sounds.len() >= 20,
        "{} sounds: {:?}",
        seen.sounds.len(),
        seen.sounds
    );
}

#[test]
fn a_fresh_table_has_its_riders_and_the_module_leaves_it_as_the_oracle_does() {
    for seats in 1..=6 {
        let mut pair = Pair::new(seats, Nums::default(), "fresh");
        assert_eq!(pair.native.seats.len(), seats as usize);
        assert_eq!(
            pair.native.wing.len(),
            4usize.saturating_sub(seats as usize)
        );
        for _ in 0..30 {
            pair.tick(&Inputs::new());
        }
    }
}

#[test]
fn bots_fly_the_whole_adventure_at_one_four_and_six_seats_tick_for_tick() {
    let mut seen = Seen::default();
    for (seats, skill) in [
        (1, bot::Skill::Sure),
        (4, bot::Skill::Sure),
        (6, bot::Skill::Novice),
    ] {
        seen.add(flown(seats, skill, Nums::default(), 14_000));
    }
    seen.add(idle(2, 3_000, false));
    seen.add(idle(4, 2_500, true));
    eprintln!("flown: {} ticks, {} sounds", seen.ticks, seen.sounds.len());
    for m in ["Play", "Retry", "Victory", "Tally"] {
        assert!(seen.modes.contains(m), "no tick in the {m} mode");
    }
    assert!(seen.restarts > 0 && seen.leaves > 0);
}

#[test]
fn random_hands_and_a_table_that_comes_and_goes_part_nowhere() {
    let mut seen = Seen::default();
    for (n, seats) in [1u32, 2, 3, 4, 5, 6].into_iter().enumerate() {
        seen.add(wandered(seats, 11 * (n as u64 + 1), n as u8 % 4, 2_500));
    }
    eprintln!("wandered: {} ticks", seen.ticks);
    assert!(seen.shots.len() >= 3 && seen.ticks >= 15_000, "{seen:?}");
}

#[test]
fn odd_states_and_the_bots_hands_on_them_part_nowhere() {
    let seen = strange(0xC0FFEE, 300);
    eprintln!("odd states: {} ticks", seen.ticks);
    assert_reached(&seen);
}

#[test]
fn the_numbers_at_their_ends_part_nowhere() {
    let lo = Nums {
        charge: 20,
        roll: 18,
        window: 40,
        spur: 30,
        tell: 40,
    };
    let hi = Nums {
        charge: 90,
        roll: 60,
        window: 90,
        spur: 180,
        tell: 120,
    };
    for (nums, seats) in [(lo, 3), (hi, 2)] {
        flown(seats, bot::Skill::Sure, nums, 14_000);
    }
}

#[test]
fn richer_shared_inputs_keep_validation_and_griffinwing_parity() {
    let mut pair = Pair::new(1, Nums::default(), "shared input fields");
    for mirror in [0, 1, 6, 7, 255] {
        for team in [false, true] {
            let input = Input { mirror, team, fire: true, ..Input::default() };
            assert_eq!(input.valid(), mirror <= 6);
            pair.tick(&BTreeMap::from([(1, input)]));
        }
    }
}

#[test]
fn the_hour_runs_out_and_the_ticks_stop_counting() {
    let heroes = table(2);
    let mut s = Session::new(&heroes);
    s.elapsed = LIMIT - 60;
    s.t = LIMIT - 60;
    s.stage = 2;
    s.cp = rail::BOSS_AT;
    s.next = oracle::resume(2, 0);
    let mut pair = Pair::from(s, heroes, Nums::default(), "the hour");
    for _ in 0..200 {
        pair.tick(&BTreeMap::from([(
            1,
            Input {
                fire: true,
                ..Input::default()
            },
        )]));
    }
    assert_eq!(pair.bent.elapsed, LIMIT);
}

// ───────────────────────── measurements ─────────────────────────

fn percentile(sorted: &[f64], q: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * q) as usize]
}

/// What a tick costs at four knights: the oracle's, the module's whole ask, and the share of a frame.
#[test]
#[ignore = "measures"]
fn per_tick_cost_at_four_knights() {
    engine();
    let heroes = table(4);
    let nums = Nums::default();
    let mut s = Session::new(&heroes);
    let mut native = s.clone();
    let (mut bend, mut orc, mut bytes_in, mut bytes_out) = (Vec::new(), Vec::new(), 0usize, 0usize);
    let mut ticks = 0u32;
    while ticks < 4_000 && !s.finished() {
        let inputs: Inputs = s
            .seats
            .keys()
            .map(|&id| (id, s.bot_with(id, &nums, bot::Skill::Sure)))
            .collect();
        let t = std::time::Instant::now();
        let want = native.step_here(&inputs, &heroes.keys().copied().collect::<Vec<_>>(), &nums);
        orc.push(t.elapsed().as_secs_f64() * 1e6);
        let ask = serde_json::to_string(&s).unwrap().len();
        let t = std::time::Instant::now();
        let got = s.step(&inputs, &heroes, &nums);
        bend.push(t.elapsed().as_secs_f64() * 1e6);
        bytes_in = bytes_in.max(ask);
        bytes_out = bytes_out.max(serde_json::to_string(&s).unwrap().len());
        assert_eq!(want.sounds, got.sounds);
        ticks += 1;
    }
    bend.sort_by(f64::total_cmp);
    orc.sort_by(f64::total_cmp);
    let frame = 1e6 / f64::from(HZ);
    eprintln!(
        "{ticks} ticks at 4 knights: module p50 {:.0} us p95 {:.0} us max {:.0} us ({:.1}% / {:.1}% of the {:.1} ms frame); oracle p50 {:.1} us p95 {:.1} us; session json up to {bytes_in} bytes in, {bytes_out} out",
        percentile(&bend, 0.5),
        percentile(&bend, 0.95),
        bend[bend.len() - 1],
        100.0 * percentile(&bend, 0.5) / frame,
        100.0 * percentile(&bend, 0.95) / frame,
        frame / 1000.0,
        percentile(&orc, 0.5),
        percentile(&orc, 0.95),
    );
}

/// The longest run, every table size, both hands: the sweep the default tests sample.
#[test]
#[ignore = "the whole sweep"]
fn the_whole_sweep_parts_nowhere() {
    let mut seen = Seen::default();
    for seats in 1..=6 {
        for skill in [bot::Skill::Sure, bot::Skill::Novice] {
            seen.add(flown(seats, skill, Nums::default(), 14_000));
        }
    }
    for (n, seed) in (1..=12u64).enumerate() {
        seen.add(wandered(1 + (n as u32 % 6), seed * 7, n as u8 % 4, 5_000));
    }
    seen.add(strange(0xFACADE, 900));
    seen.add(idle(1, 4_000, false));
    seen.add(idle(6, 4_000, false));
    seen.add(idle(2, 5_000, true));
    eprintln!("the sweep: {} ticks", seen.ticks);
    assert_reached(&seen);
}
