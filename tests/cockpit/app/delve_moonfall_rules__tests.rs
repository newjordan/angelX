//! Moonfall's rules, held to the code they replaced.
//!
//! The rules left `crates/delve/src/rules/moonfall*` for the Bend module
//! `cockpit/live/moonfall-rules`; what stayed there is the session, its saves
//! and wire shape, the words, the sounds' names and the adapter that asks the
//! module (`moonfall/port.rs`). The native bodies are `delve_moonfall_rules__tests/oracle.rs`,
//! moved here unchanged apart from paths, and these tests run them side by
//! side with the module the host installs (the shipped generation compiled into
//! this binary): bots playing whole games, noisy keys on a learned Ledger,
//! thousands of random valid towns, knights coming and going, and the bots'
//! own presses. After every tick the whole session, the sounds, the knights
//! who walk out and the first deliverance must be equal.
//!
//! The Moonfall game tests that used to tick the native rules in
//! `crates/delve` moved here with them (`delve_moonfall_rules__tests/play.rs`)
//! and run through the module.

use crate::agent::harness::live_policy::install_scripted_rules;
use angel_delve::rules::moonfall::{
    Session, clock,
    folk::{CAST, Folk},
    ledger::{self, Fact},
    port::CUES,
    says::{Sign, Topic},
    state::*,
    town,
};
use angel_delve::rules::{Hero, Input, Run};
use oracle::{Old, plan};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

#[path = "delve_moonfall_rules__tests/oracle.rs"]
mod oracle;
#[path = "delve_moonfall_rules__tests/play.rs"]
mod play;
#[path = "delve_moonfall_rules__tests/run.rs"]
mod run;

type Held = BTreeMap<u32, Input>;
type Heroes = BTreeMap<u32, Hero>;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }
    fn range(&mut self, lo: u32, hi: u32) -> u32 {
        lo + self.below(u64::from(hi - lo + 1)) as u32
    }
    fn chance(&mut self, pct: u64) -> bool {
        self.below(100) < pct
    }
    fn axis(&mut self) -> i8 {
        self.below(3) as i8 - 1
    }
}

fn heroes(n: u32) -> Heroes {
    install_scripted_rules();
    let mut r = Run::moonfall_for_test();
    for id in 2..=n {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.players.clone()
}

/// What a run of lockstep ticks saw, to prove the scripts reached the branches they name.
#[derive(Default)]
struct Seen {
    ticks: u32,
    cues: BTreeSet<usize>,
    phases: BTreeSet<&'static str>,
    wins: u32,
    leaving: u32,
    downs: u32,
    hurts: u32,
    snuffs: u32,
    shy: u32,
    parked: u32,
    away: u32,
    rings: u32,
    talks: u32,
    notes: BTreeSet<String>,
    bots: u32,
}

impl Seen {
    fn add(&mut self, other: &Seen) {
        self.ticks += other.ticks;
        self.cues.extend(&other.cues);
        self.phases.extend(&other.phases);
        self.wins += other.wins;
        self.leaving += other.leaving;
        self.downs += other.downs;
        self.hurts += other.hurts;
        self.snuffs += other.snuffs;
        self.shy += other.shy;
        self.parked += other.parked;
        self.away += other.away;
        self.rings += other.rings;
        self.talks += other.talks;
        self.notes.extend(other.notes.iter().cloned());
        self.bots += other.bots;
    }

    fn note(
        &mut self,
        before: &Session,
        after: &Session,
        cues: &[&'static str],
        leaving: usize,
        won: bool,
    ) {
        self.ticks += 1;
        for c in cues {
            self.cues.extend(CUES.iter().position(|n| n == c));
        }
        self.phases.insert(match before.phase {
            Phase::Play => "play",
            Phase::Rewind { .. } => "rewind",
            Phase::Skip { .. } => "skip",
            Phase::Won { .. } => "won",
        });
        self.wins += u32::from(won);
        self.leaving += u32::from(leaving > 0);
        self.parked += u32::from(!after.parked.is_empty());
        for (id, seat) in &after.seats {
            let b = before.seats.get(id).map(|s| &s.knight);
            self.downs += u32::from(seat.knight.down > 0);
            self.hurts += u32::from(b.is_some_and(|b| seat.knight.candles < b.candles));
            self.talks += u32::from(seat.knight.say.is_some());
            if let Some((n, _)) = seat.knight.note {
                self.notes.insert(
                    format!("{:?}", n)
                        .split(['(', ' '])
                        .next()
                        .unwrap_or("")
                        .to_string(),
                );
            }
        }
        self.away += u32::from(before.seats.len() != after.seats.len());
        self.rings += u32::from(after.call.is_some());
        for w in &before.world.wisps {
            match w.mode {
                WispMode::Snuff(_) => self.snuffs += 1,
                WispMode::Shy(_) => self.shy += 1,
                WispMode::Hunt => {}
            }
        }
    }
}

fn diff(want: &Value, got: &Value, path: &str) -> Option<String> {
    if want == got {
        return None;
    }
    match (want, got) {
        (Value::Object(a), Value::Object(b)) => {
            for k in a.keys().chain(b.keys()).collect::<BTreeSet<_>>() {
                if let Some(d) = diff(
                    a.get(k).unwrap_or(&Value::Null),
                    b.get(k).unwrap_or(&Value::Null),
                    &format!("{path}.{k}"),
                ) {
                    return Some(d);
                }
            }
            None
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            (0..a.len()).find_map(|i| diff(&a[i], &b[i], &format!("{path}[{i}]")))
        }
        _ => Some(format!("{path}: native {want}, module {got}")),
    }
}

/// One tick through the native rules and through the module, from the same session.
/// `s` becomes the module's session; every part of the answer must equal the oracle's.
fn compare(
    label: &str,
    tick: u32,
    s: &mut Session,
    inputs: &Held,
    heroes: &Heroes,
    seen: &mut Seen,
) {
    let before = s.clone();
    let mut native = s.clone();
    let want = oracle::tick(&mut native, inputs, heroes);
    let got = s
        .advance(inputs, heroes)
        .unwrap_or_else(|error| panic!("{label} tick {tick}: the module refused: {error}"));
    if *s != native {
        let (a, b) = (
            serde_json::to_value(&native).unwrap(),
            serde_json::to_value(&*s).unwrap(),
        );
        let at = diff(&a, &b, "session").unwrap_or_else(|| "pending sounds differ".into());
        panic!("{label} tick {tick}: {at}");
    }
    assert_eq!(got.cues, want.cues, "{label} tick {tick}: sounds");
    assert_eq!(
        got.leaving, want.leaving,
        "{label} tick {tick}: who walks out"
    );
    assert_eq!(
        got.won, want.won,
        "{label} tick {tick}: the first deliverance"
    );
    seen.note(&before, s, &got.cues, got.leaving.len(), got.won.is_some());
}

/// The module's bot against the native one, for every seat at the table.
fn compare_bots(label: &str, tick: u32, s: &Session, heroes: &Heroes, seen: &mut Seen) {
    for (&id, hero) in heroes.iter().filter(|(id, _)| s.seats.contains_key(id)) {
        let want = plan::bot(s, id);
        let got = s.bot_input(id, hero);
        assert_eq!(got, want, "{label} tick {tick}: what knight {id} presses");
        seen.bots += 1;
    }
}

fn noise_input(r: &mut Rng) -> Input {
    let x = r.next();
    let mut i = Input {
        move_x: r.axis(),
        move_y: r.axis(),
        aim_x: if x & 1 == 0 { 0 } else { r.axis() },
        aim_y: if x & 2 == 0 { 0 } else { r.axis() },
        interact: x & 4 != 0,
        take: x & 8 == 0 && x & 16 == 0 && x & 32 == 0,
        swing: x & 448 == 0,
        ult: x & 3584 == 0,
        play: u8::from(x & 28672 == 0),
        bomb: x & 229_376 == 0,
        dash: x & 1_835_008 == 0,
        ..Default::default()
    };
    if r.chance(1) {
        i.move_x = 5;
    }
    i
}

fn walkable(r: &mut Rng, ground: town::Ground) -> (i32, i32) {
    loop {
        let (tx, ty) = (
            r.range(0, town::COLS as u32 - 1) as i32,
            r.range(0, town::ROWS as u32 - 1) as i32,
        );
        let (x, y) = ((tx * 16 + 8) * 256, (ty * 16 + 8) * 256);
        if oracle::town::clear(x, y, ground, &[]) {
            return (x, y);
        }
    }
}

fn some_folk(r: &mut Rng) -> Folk {
    CAST[r.below(6) as usize]
}

fn some_topic(r: &mut Rng) -> Topic {
    match r.below(7) {
        0 => Topic::Hello(r.below(4) as u8),
        1 => Topic::Request,
        2 => Topic::Give,
        3 => Topic::Thanks,
        4 => Topic::Done,
        5 => Topic::Nag(r.below(4) as u8),
        _ => Topic::Cry(r.below(4) as u8),
    }
}

fn some_note(r: &mut Rng) -> Note {
    match r.below(12) {
        0 => Note::Wrote(some_folk(r), r.below(12) as u8),
        1 => Note::Learned(ledger::FACTS[r.below(8) as usize]),
        2 => Note::Ticked(some_folk(r)),
        3 => Note::Remind(some_folk(r), r.below(12) as u8),
        4 => Note::Sign(Sign::TooHeavy),
        5 => Note::Got(
            [Item::Ink, Item::Reeds, Item::Pass, Item::Scroll, Item::Shoe][r.below(5) as usize],
        ),
        6 => Note::Mask([Mask::Bare, Mask::Swift, Mask::Stout][r.below(3) as usize]),
        7 => Note::Candle(r.below(3) as u8),
        8 => Note::Snuffed,
        9 => Note::Lit(r.below(3) as u8),
        10 => Note::Out,
        _ => Note::Ring(r.range(1, 6)),
    }
}

/// A point near something a knight might want: a villager, the pot, the doors, the well, a lantern.
fn near(r: &mut Rng, s: &Session) -> (i32, i32) {
    let ground = s.world.ground(s.clock, s.nums.day, true);
    let mut pts: Vec<(i32, i32)> = vec![
        town::INK_POT,
        town::REED_STACK,
        town::NORTH_DOOR,
        town::SOUTH_DOOR,
        town::WELL,
        town::BELL_POST,
    ];
    for &(tx, ty) in &town::LAMPS {
        pts.push((tx * 16 + 8, ty * 16 + 4));
    }
    for f in CAST {
        let fig = s.figure(f);
        pts.push((fig.x, fig.y));
    }
    let (px, py) = pts[r.below(pts.len() as u64) as usize];
    for _ in 0..20 {
        let (dx, dy) = (r.range(0, 40) as i32 - 20, r.range(0, 40) as i32 - 20);
        let (x, y) = (
            (px + dx) * 256 + r.range(0, 255) as i32,
            (py + dy) * 256 + r.range(0, 255) as i32,
        );
        if oracle::town::clear(x, y, ground, &s.world.crates) {
            return (x, y);
        }
    }
    walkable(r, ground)
}

fn knight_noise(r: &mut Rng, s: &Session, k: &mut Knight) {
    let ground = s.world.ground(s.clock, s.nums.day, true);
    let (x, y) = if r.chance(55) {
        near(r, s)
    } else {
        walkable(r, ground)
    };
    k.x = x;
    k.y = y;
    k.face = r.below(4) as u8;
    k.stride = if r.chance(50) { 0 } else { r.range(0, 70_000) };
    k.held = [
        Item::Empty,
        Item::Ink,
        Item::Reeds,
        Item::Pass,
        Item::Scroll,
        Item::Shoe,
    ][r.below(6) as usize];
    k.held_at = r.range(0, clock::end(s.nums.day));
    k.mask = [Mask::Bare, Mask::Swift, Mask::Stout][r.below(3) as usize];
    k.candles = r.below(4) as u8;
    k.hurt = if r.chance(70) { 0 } else { r.range(0, 120) };
    k.down = if r.chance(85) { 0 } else { r.range(1, 90) };
    if k.down > 0 {
        k.candles = 0;
    }
    k.roll = if r.chance(80) { 0 } else { r.range(1, 6) };
    k.roll_rest = if r.chance(60) { 0 } else { r.range(1, 40) };
    k.roll_dir = (r.axis(), r.axis());
    k.keys = if r.chance(50) {
        0
    } else {
        r.below(1024) as u16
    };
    k.panel = match r.below(6) {
        0 => Panel::Ledger(r.below(7) as u8),
        1 => Panel::Bell(r.below(3) as u8),
        _ => Panel::Shut,
    };
    k.say = if r.chance(70) {
        None
    } else {
        Some(Say {
            folk: some_folk(r),
            topic: some_topic(r),
            page: r.below(4) as u8,
            age: if r.chance(90) {
                r.range(0, 100)
            } else {
                r.range(340, 361)
            },
        })
    };
    k.hold = if r.chance(70) { 0 } else { r.range(0, 24) };
    k.push = if r.chance(80) { 0 } else { r.range(0, 10) };
    k.note = if r.chance(70) {
        None
    } else {
        Some((some_note(r), r.range(1, 120)))
    };
    k.read = if r.chance(50) { 0 } else { r.range(0, 60) };
}

/// Scatter a session over the town's states: any hour, any phase, any Ledger, any
/// door, wisps in every mood, knights wherever a favour may be done. It stays one a run may hold.
fn perturb(r: &mut Rng, s: &mut Session) {
    let before = s.clone();
    s.nums.day = [3000, 3000, 1800, 2400, 6000, 4001, 1801][r.below(7) as usize];
    if r.chance(15) {
        s.nums.day = r.range(1800, 6000);
    }
    if r.chance(30) {
        s.nums.walk = r.range(100, 300);
    }
    if r.chance(30) {
        s.nums.wisps = r.range(0, 8);
    }
    if r.chance(20) {
        s.nums.rewind = r.range(20, 44);
    }
    if r.chance(20) {
        s.nums.notice = r.range(24, 160);
    }
    let day = s.nums.day;
    let end = clock::end(day);
    let fin = clock::final_start(day);
    s.clock = match r.below(10) {
        0 => end - r.range(0, 3).min(end),
        1 => fin - 2 + r.range(0, 4),
        2 => (day / 2).saturating_sub(2) + r.range(0, 4),
        // One draw for the base: separate draws could take more than it holds.
        3 => {
            let at = day * r.range(0, 2);
            at - r.range(0, 2).min(at)
        }
        4 => clock::next_dusk(r.range(0, end), day).min(end),
        _ => r.range(0, end),
    }
    .min(end);
    s.elapsed = r.range(0, 5000);
    s.card = r.range(0, 120);
    let control = angel_delve::rules::moonfall::control(&s.nums);
    s.phase = match r.below(10) {
        0 => Phase::Rewind {
            t: if r.chance(40) {
                control.saturating_sub(r.range(1, 2))
            } else {
                r.range(0, control)
            },
            from: r.range(0, end),
            fell: r.chance(50),
        },
        1 => {
            let a = r.range(0, end);
            let b = r.range(a, end);
            Phase::Skip {
                t: if r.chance(40) { 35 } else { r.range(0, 36) },
                from: a,
                to: b,
            }
        }
        2 => Phase::Won {
            t: if r.chance(40) { 119 } else { r.range(0, 120) },
        },
        _ => Phase::Play,
    };
    s.world.flags = r.below(16) as u16;
    s.world.ink_given = if r.chance(50) {
        None
    } else {
        Some(r.range(0, end))
    };
    s.world.shod = if r.chance(60) {
        None
    } else {
        Some(r.range(0, end))
    };
    s.world.crates = match r.below(4) {
        0 => vec![],
        1 => vec![(r.range(14, 22) as i32, r.range(18, 22) as i32)],
        _ => vec![town::CRATE_START],
    };
    s.world.snuffed = r.below(64) as u8;
    let clock_now = s.clock;
    s.world.lit_until = [0; 3].map(|_| match r.below(4) {
        0 => 0,
        1 => clock_now + r.range(0, 800),
        2 => clock_now.saturating_sub(r.range(0, 3)),
        _ => r.range(0, end * 2),
    });
    s.world.wisps.clear();
    let nw = if r.chance(50) { 0 } else { r.range(0, 10) };
    for _ in 0..nw {
        let mode = match r.below(3) {
            0 => WispMode::Hunt,
            1 => WispMode::Snuff(r.range(1, 60)),
            _ => WispMode::Shy(r.range(1, 90)),
        };
        let (x, y) = if r.chance(40) {
            near(r, s)
        } else {
            (
                r.range(0, 600) as i32 * 256 - 20 * 256,
                r.range(0, 440) as i32 * 256 - 20 * 256,
            )
        };
        s.world.wisps.push(Wisp {
            x: x + r.range(0, 255) as i32,
            y,
            mode,
            aim: r.below(9) as u8,
            born: r.range(0, 4000),
        });
    }
    s.memory.facts = r.below(256) as u16;
    for ri in &mut s.memory.routines {
        *ri = if r.chance(30) {
            0
        } else {
            r.below(1 << 13) as u32
        };
    }
    s.memory.ticks = r.below(64) as u8;
    s.memory.masks = r.below(4) as u8;
    s.memory.keeps = r.below(8) as u8;
    s.memory.loops = if r.chance(70) {
        r.range(0, 3)
    } else {
        r.range(0, 99)
    };
    if r.chance(20) {
        s.memory.facts |= 1 << (Fact::Verse as u16);
        s.memory.keeps = 7;
        s.memory.masks = 3;
    }
    s.call = if r.chance(70) {
        None
    } else {
        Some(Call {
            ring: [Ring::Rewind, Ring::Dusk, Ring::Dawn][r.below(3) as usize],
            by: r.range(1, 6),
            ayes: r.below(64) as u8,
            left: r.range(0, 150),
        })
    };
    s.cry = if r.chance(70) {
        None
    } else {
        Some((some_folk(r), some_topic(r), r.range(0, 151)))
    };
    s.news.clear();
    for _ in 0..r.below(4) {
        s.news.push((some_note(r), r.range(1, 120)));
    }
    s.completed = if r.chance(88) {
        None
    } else {
        Some(r.range(0, s.elapsed))
    };
    s.parked.clear();
    let ids: Vec<u32> = s.seats.keys().copied().collect();
    for id in &ids {
        let mut k = s.seats[id].knight.clone();
        knight_noise(r, s, &mut k);
        s.seats.get_mut(id).unwrap().knight = k;
        s.seats.get_mut(id).unwrap().joined = r.range(0, s.elapsed);
    }
    for id in 1..=6u32 {
        if !s.seats.contains_key(&id) && r.chance(30) {
            let mut k = Knight::at((16, 15));
            knight_noise(r, s, &mut k);
            s.parked.insert(
                id,
                Seat {
                    joined: r.range(0, s.elapsed),
                    knight: k,
                },
            );
        }
    }
    s.peak = s.peak.max(s.seats.len() as u32).min(6);
    if !s.valid() {
        *s = before;
    }
}

fn random_inputs(r: &mut Rng, ids: &[u32]) -> Held {
    let mut out = Held::new();
    for &id in ids {
        if r.chance(85) {
            out.insert(id, noise_input(r));
        }
    }
    out
}

/// Some knights away or turned to stone, as a table is when people come and go.
fn roster(r: &mut Rng, base: &Heroes) -> Heroes {
    let mut h = base.clone();
    for id in 1..=6u32 {
        if let Some(hero) = h.get_mut(&id) {
            match r.below(12) {
                0 => hero.away = true,
                1 => hero.stone = true,
                _ => {
                    hero.away = false;
                    hero.stone = false;
                }
            }
        }
    }
    h
}

/// Bots with a blank Ledger play whole games: the module's session against the
/// native one every tick, and their keys against the native bots' every few.
#[test]
fn bots_play_whole_games_and_every_tick_agrees() {
    let mut all = Seen::default();
    for (seats, day, walk, wisps) in [
        (1u32, 3000u32, 150u32, 4u32),
        (2, 3000, 150, 4),
        (4, 1800, 150, 3),
        (6, 2400, 200, 4),
        (3, 3000, 100, 8),
    ] {
        let h = heroes(seats.max(2));
        let mut s = Session::new(&h);
        s.nums.day = day;
        s.nums.walk = walk;
        s.nums.wisps = wisps;
        let label = format!("{seats} bots, day {day}");
        let mut seen = Seen::default();
        for tick in 0..20_000 {
            let inputs: Held = s.seats.keys().map(|&id| (id, plan::bot(&s, id))).collect();
            if tick % 13 == 0 {
                compare_bots(&label, tick, &s, &h, &mut seen);
            }
            compare(&label, tick, &mut s, &inputs, &h, &mut seen);
            if s.completed.is_some() && tick > 1500 {
                break;
            }
        }
        all.add(&seen);
    }
    assert!(
        all.wins >= 3,
        "bots turned the moon back in {} games",
        all.wins
    );
    assert!(
        all.phases.len() == 3 || all.phases.contains("won"),
        "{:?}",
        all.phases
    );
    assert!(all.bots > 1000, "bot presses compared: {}", all.bots);
}

/// Noisy keys on a learned Ledger and a short day, so a few thousand ticks cross dusk, the Final
/// Hours, a fall and a bell, at every table size.
#[test]
fn noisy_keys_on_a_learned_ledger_agree_every_tick() {
    let mut all = Seen::default();
    for seats in [1u32, 2, 3, 4, 6] {
        let h = heroes(seats.max(2));
        let mut s = Session::new(&h);
        s.nums.day = 1800;
        for f in ledger::FACTS {
            s.memory.learn(f);
        }
        s.memory.masks = 3;
        s.memory.keeps = 7;
        let mut r = Rng(0x9e37_79b9_7f4a_7c15 ^ u64::from(seats));
        let label = format!("{seats} seats, noise");
        let mut seen = Seen::default();
        for tick in 0..6_000 {
            let inputs: Held = h.keys().map(|&id| (id, noise_input(&mut r))).collect();
            compare(&label, tick, &mut s, &inputs, &h, &mut seen);
            if tick % 97 == 0 {
                compare_bots(&label, tick, &s, &h, &mut seen);
            }
        }
        all.add(&seen);
    }
    assert!(all.phases.len() >= 3, "{:?}", all.phases);
}

/// Thousands of random valid towns, one random-keyed tick each: every hour of the loop, every
/// phase, every Ledger, wisps in every mood, knights beside every favour, rosters with people
/// away, sounds still owed from before.
#[test]
fn random_towns_agree_after_one_tick() {
    let mut all = Seen::default();
    let mut r = Rng(0x1234_5678_9abc_def1);
    for round in 0..4_000u32 {
        let seats = [1u32, 2, 3, 4, 6][r.below(5) as usize];
        let h = heroes(seats.max(2));
        let mut s = Session::new(&h);
        if r.chance(50) {
            for _ in 0..r.below(8) {
                let mut n = s.clone();
                oracle::tick(&mut n, &Held::new(), &h);
                s = n;
            }
        }
        perturb(&mut r, &mut s);
        let roster = if r.chance(30) {
            roster(&mut r, &h)
        } else {
            h.clone()
        };
        let ids: Vec<u32> = (1..=6).collect();
        let inputs = random_inputs(&mut r, &ids);
        if r.chance(10) {
            s.announce(Note::Learned(Fact::Verse));
            s.announce(Note::Remind(Folk::Mira, 4));
        }
        compare("a random town", round, &mut s, &inputs, &roster, &mut all);
        if r.chance(15) {
            compare_bots("a random town", round, &s, &roster, &mut all);
        }
    }
    for want in [0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11] {
        assert!(
            all.cues.contains(&want),
            "no tick made sound {} ({:?})",
            CUES[want],
            all.cues
        );
    }
    assert!(all.phases.len() == 4, "{:?}", all.phases);
    assert!(
        all.wins > 20 && all.leaving > 20,
        "wins {} leaving {}",
        all.wins,
        all.leaving
    );
    assert!(
        all.parked > 100 && all.away > 50,
        "parked {} away {}",
        all.parked,
        all.away
    );
    assert!(
        all.snuffs > 50 && all.shy > 50 && all.hurts > 0 && all.downs > 50,
        "{} {} {} {}",
        all.snuffs,
        all.shy,
        all.hurts,
        all.downs
    );
    assert!(all.notes.len() >= 8, "{:?}", all.notes);
}

/// People come and go through a long noisy loop: tabs hidden, a knight turned to stone, a late
/// arrival who sits down at the well, the parked who return where they stood.
#[test]
fn knights_who_come_and_go_agree_every_tick() {
    let base = heroes(6);
    let mut s = Session::new(&heroes(2));
    s.nums.day = 1800;
    let mut r = Rng(0xfeed_f00d_1234_9876);
    let mut seen = Seen::default();
    let mut roster = base.clone();
    for id in 3..=6u32 {
        roster.get_mut(&id).unwrap().away = true;
    }
    for tick in 0..8_000u32 {
        if tick % 211 == 0 {
            roster = self::roster(&mut r, &base);
            for id in 1..=6u32 {
                if r.chance(30) {
                    if let Some(h) = roster.get_mut(&id) {
                        h.away = r.chance(50);
                    }
                }
            }
            if r.chance(25) {
                roster.remove(&(r.range(3, 6)));
            }
        }
        let inputs = random_inputs(&mut r, &(1..=6).collect::<Vec<_>>());
        compare(
            "a table coming and going",
            tick,
            &mut s,
            &inputs,
            &roster,
            &mut seen,
        );
    }
    assert!(
        seen.parked > 200 && seen.away > 20,
        "parked {} away {}",
        seen.parked,
        seen.away
    );
}

/// The module's bot presses what the native bot presses in towns it has never played itself into.
#[test]
fn the_module_bot_presses_what_the_native_bot_presses() {
    let mut r = Rng(0xb07_b07_b07_b07);
    let mut seen = Seen::default();
    for round in 0..1_500u32 {
        let seats = [1u32, 2, 3, 4, 6][r.below(5) as usize];
        let h = heroes(seats.max(2));
        let mut s = Session::new(&h);
        perturb(&mut r, &mut s);
        if r.chance(50) {
            // a bot's world: the Ledger full, so every task is on the table
            s.memory.facts = 255;
            s.memory.masks = r.below(4) as u8;
        }
        compare_bots("a random town", round, &s, &h, &mut seen);
    }
    assert!(seen.bots > 3_000, "{}", seen.bots);
}

/// A knight stood beside each villager in turn, at any hour: the Ledger lines the module
/// writes, and the facts it learns, come from where it places the villager, and the whole
/// session must still be the native one's (the native schedule is `folk::figure`).
#[test]
fn a_knight_beside_any_villager_at_any_hour_agrees() {
    let h = heroes(2);
    let mut r = Rng(0x7a11_7a11);
    let mut seen = Seen::default();
    for round in 0..800u32 {
        let mut s = Session::new(&h);
        perturb(&mut r, &mut s);
        s.phase = Phase::Play;
        s.completed = None;
        s.clock = (s.clock / 5) * 5 + 4;
        let fig = s.figure(CAST[r.below(6) as usize]);
        if let Some(seat) = s.seats.values_mut().next() {
            seat.knight.down = 0;
            seat.knight.x = (fig.x + r.range(0, 8) as i32) * 256;
            seat.knight.y = fig.y * 256;
        }
        if !s.valid() {
            continue;
        }
        compare(
            "a knight beside a villager",
            round,
            &mut s,
            &Held::new(),
            &h,
            &mut seen,
        );
    }
    assert!(seen.ticks > 400, "{}", seen.ticks);
}

fn percentile(sorted: &[u128], p: f64) -> u128 {
    sorted[((sorted.len() as f64 - 1.0) * p).round() as usize]
}

/// `cargo test --profile fast --bin angel -- moonfall_rules_cost --ignored --nocapture`: what a
/// tick costs through the module at one and four seats, against the native tick, and how big
/// the frame and the answer are. The Delve ticks at 30 Hz: 33,333 us a frame for everything.
#[test]
#[ignore = "measures"]
fn moonfall_rules_cost() {
    use angel_delve::scripted;
    for seats in [1u32, 4] {
        let h = heroes(seats.max(2));
        let mut s = Session::new(&h);
        s.nums.day = 1800;
        for f in ledger::FACTS {
            s.memory.learn(f);
        }
        s.memory.masks = 3;
        s.memory.keeps = 7;
        let mut r = Rng(0x5eed_0000 + u64::from(seats));
        let mut bend = Vec::new();
        let mut native = Vec::new();
        let mut frame_bytes = 0;
        let mut answer_bytes = 0;
        for _ in 0..1_500 {
            let inputs: Held = h.keys().map(|&id| (id, noise_input(&mut r))).collect();
            let mut n = s.clone();
            let t = Instant::now();
            oracle::tick(&mut n, &inputs, &h);
            native.push(t.elapsed().as_nanos());
            let frame = s.frame(&inputs, &h);
            frame_bytes = serde_json::to_string(&frame).unwrap().len();
            let t = Instant::now();
            let answer: Value = scripted::step("moonfall-rules", &frame).unwrap();
            bend.push(t.elapsed().as_micros());
            answer_bytes = serde_json::to_string(&answer).unwrap().len();
            s.advance(&inputs, &h).unwrap();
        }
        bend.sort();
        native.sort();
        let (p50, p95) = (percentile(&bend, 0.5), percentile(&bend, 0.95));
        eprintln!(
            "MOONFALL {seats} seats: module p50 {p50} us, p95 {p95} us, max {} us ({:.2}% / {:.2}% of 33,333 us); native p50 {} ns; frame {frame_bytes} B, answer {answer_bytes} B",
            bend[bend.len() - 1],
            p50 as f64 / 333.33,
            p95 as f64 / 333.33,
            percentile(&native, 0.5)
        );
    }
    // The bot: one call per bot per tick.
    let h = heroes(4);
    let mut s = Session::new(&h);
    let mut costs = Vec::new();
    for tick in 0..1_500u32 {
        let inputs: Held = s.seats.keys().map(|&id| (id, plan::bot(&s, id))).collect();
        if tick % 3 == 0 {
            for (&id, hero) in &h {
                let t = Instant::now();
                let _ = s.bot_input(id, hero);
                costs.push(t.elapsed().as_micros());
            }
        }
        s.advance(&inputs, &h).unwrap();
    }
    costs.sort();
    eprintln!(
        "MOONFALL bot call at 4 seats: p50 {} us, p95 {} us, max {} us",
        percentile(&costs, 0.5),
        percentile(&costs, 0.95),
        costs[costs.len() - 1]
    );
}
