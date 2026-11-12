//! The Shaft Run's tick, held to the code it replaced.
//!
//! The tick lives in Bend now (`cockpit/live/race3d-rules`) and `crates/delve/
//! src/rules/race3d/leg.rs` only asks for it. What is left of the native tick
//! is `crates/delve/src/rules/race3d/oracle.rs`, included here verbatim, and
//! these tests run the shipped leg and that oracle side by side over scripted
//! sessions, comparing every bit of the state every tick.
//!
//! The race's own tests run in `crates/delve`, against the same oracle.

pub use angel_delve::rules::race3d::*;
pub use angel_delve::rules::{Input, HZ};

// Its `answer` is the engine half that serves crates/delve's own tests.
#[allow(dead_code)]
#[path = "../../../crates/delve/src/rules/race3d/oracle.rs"]
mod oracle;

use std::collections::{BTreeMap, BTreeSet};

// ── the race the parity drives ─────────────────────────────────────────────

fn rider(s: f32, x: f32, lane: u8) -> Rider {
    Rider {
        drift: Drift::default(),
        s,
        x,
        z: 0.0,
        vs: 0.0,
        vx: 0.0,
        vz: 0.0,
        lane,
        spin: 0.0,
        crash: 0,
        wreck: Wreck::None,
        crashes: 0,
        stun: 0,
        burst: 0,
        rolling: 0,
        rest: 0,
        reload: 0,
        haul: 0,
        rings: 0,
        finish: None,
        keys: 0,
    }
}

fn field(seats: u32) -> Race3d {
    let riders = (0..seats)
        .map(|k| {
            let id = k + 1;
            let s = 2.0 + (k / 2) as f32 * 1.4;
            let x = if k % 2 == 0 { -1.0 } else { 1.0 };
            (id, rider(s, x, 1))
        })
        .collect();
    Race3d {
        course: Course::Shaft,
        room: 0,
        t: TITLE,
        riders,
        ante: BTreeMap::new(),
        taken: BTreeSet::new(),
        planks: BTreeMap::new(),
        bolts: Vec::new(),
        done: false,
        ended: 0,
        won: BTreeMap::new(),
        practice: true,
        crossroads: false,
        show: Default::default(),
    }
}

/// A scripted hand: a small generator so every branch is played over the
/// sessions, with the odd held key and a hop now and then.
fn hand(tick: u32, id: u32) -> Input {
    // A hash: the products are meant to wrap.
    let mut h = u64::from(tick).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ u64::from(id).wrapping_mul(0x1234_5678_9abc_def);
    h ^= h >> 29;
    let axis = |bits: u64| match bits % 5 {
        0 => -1,
        1 => 1,
        _ => 0,
    };
    Input {
        move_x: axis(h),
        move_y: axis(h >> 3),
        fire: (h >> 7) % 9 == 0,
        interact: (h >> 11) % 23 == 0,
        dash: (h >> 13) % 7 == 0,
        ..Input::default()
    }
}

fn differ(a: &Rider, b: &Rider) -> Vec<&'static str> {
    let mut out = Vec::new();
    let pairs = [
        ("s", a.s.to_bits() == b.s.to_bits()),
        ("x", a.x.to_bits() == b.x.to_bits()),
        ("z", a.z.to_bits() == b.z.to_bits()),
        ("vs", a.vs.to_bits() == b.vs.to_bits()),
        ("vx", a.vx.to_bits() == b.vx.to_bits()),
        ("vz", a.vz.to_bits() == b.vz.to_bits()),
        ("spin", a.spin.to_bits() == b.spin.to_bits()),
        ("heading", a.drift.heading.to_bits() == b.drift.heading.to_bits()),
        ("travel", a.drift.travel.to_bits() == b.drift.travel.to_bits()),
        ("lane", a.lane == b.lane),
        ("crash", a.crash == b.crash),
        ("wreck", a.wreck == b.wreck),
        ("crashes", a.crashes == b.crashes),
        ("stun", a.stun == b.stun),
        ("burst", a.burst == b.burst),
        ("rolling", a.rolling == b.rolling),
        ("rest", a.rest == b.rest),
        ("reload", a.reload == b.reload),
        ("haul", a.haul == b.haul),
        ("rings", a.rings == b.rings),
        ("finish", a.finish == b.finish),
        ("keys", a.keys == b.keys),
    ];
    for (name, same) in pairs {
        if !same {
            out.push(name);
        }
    }
    out
}

/// The whole state, tick by tick: the leg and the moved oracle must agree on
/// every bit of it.
fn parity(seats: u32, ticks: u32) {
    parity_of(field(seats), ticks);
}

/// The same, from any field; the sounds the ticks made, for coverage.
fn parity_of(mut race: Race3d, ticks: u32) -> BTreeSet<&'static str> {
    let mut heard = BTreeSet::new();
    let ids: Vec<u32> = race.riders.keys().copied().collect();
    for _ in 0..ticks {
        let inputs: BTreeMap<u32, Input> = ids
            .iter()
            .map(|&id| (id, hand(race.t, id)))
            .collect();
        let mut oracle = race.clone();
        oracle.t += 1;
        let (oracle_home, oracle_out, oracle_sounds) = oracle::tick(&mut oracle, &inputs, &ids);
        let mut bent = race.clone();
        bent.t += 1;
        let ran = leg::run(&mut bent, &inputs, &ids).expect("the leg answers a tick");
        for &id in &ids {
            let apart = differ(&oracle.riders[&id], &bent.riders[&id]);
            assert!(
                apart.is_empty(),
                "rider {id} parts at tick {} on {apart:?}\nbefore {:?}\nnative {:?}\nbent   {:?}\nholding {:?}",
                bent.t,
                race.riders[&id],
                oracle.riders[&id],
                bent.riders[&id],
                inputs[&id],
            );
        }
        assert_eq!(oracle.taken, bent.taken, "the gems taken part at {}", bent.t);
        assert_eq!(oracle.planks, bent.planks, "the planks part at {}", bent.t);
        assert_eq!(oracle.bolts.len(), bent.bolts.len());
        assert_eq!(ran.sounds, oracle_sounds, "the room's sounds part at {}", bent.t);
        heard.extend(ran.sounds.iter().copied());
        assert_eq!(ran.all_home, oracle_home, "the home flag parts at {}", bent.t);
        assert_eq!(ran.timed_out, oracle_out, "the clock parts at {}", bent.t);
        race = bent;
    }
    heard
}

#[test]
fn the_leg_rolls_the_shaft_exactly_as_the_native_rules_did() {
    let _alone = ENGINE.lock().unwrap_or_else(|p| p.into_inner());
    crate::agent::harness::live_policy::install_scripted_rules();
    parity(1, 240);
    parity(2, 240);
    parity(3, 180);
    parity(4, 180);
}

/// A crowd: knights on top of one another anywhere along the shaft, going every
/// way at once, on a gem, at a crossing or on a rotten plank half the time, so
/// the bump, the pickups, the carts, the planks and the edges are all played at
/// their boundaries.
fn crowd(seed: u64) -> Race3d {
    let mut h = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
    let mut next = move |lo: f32, hi: f32| {
        h ^= h << 13;
        h ^= h >> 7;
        h ^= h << 17;
        lo + (h >> 40) as f32 / (1u64 << 24) as f32 * (hi - lo)
    };
    let course = track(Course::Shaft);
    let mut race = field(4);
    race.t = TITLE + next(0.0, 160.0) as u32;
    let race_t = race.t;
    let (mut at, mut across) = (next(3.0, 470.0), 0.0);
    match next(0.0, 5.0) as u32 {
        0 => {
            let gem = course.items[next(0.0, course.items.len() as f32) as usize];
            (at, across) = (gem.s - 0.4, gem.x);
        }
        1 => {
            let post = course.posts.iter().copied().filter(|&p| course.at(p).ground == Ground::Crossing);
            let posts: Vec<f32> = post.collect();
            at = posts[next(0.0, posts.len() as f32) as usize] + next(4.0, 14.0);
        }
        2 => {
            let post = course.posts.iter().copied().filter(|&p| course.at(p).ground == Ground::Collapse);
            let posts: Vec<f32> = post.collect();
            at = posts[next(0.0, posts.len() as f32) as usize] + next(0.0, 28.0);
            for plank in 0..(at as u16 / 2 + 3) {
                if rotten(plank) && next(0.0, 3.0) < 1.0 {
                    race.planks.insert(plank, race.t - next(0.0, 40.0) as u32);
                }
            }
        }
        3 => at = course.finish - next(0.0, 3.0),
        _ => {}
    }
    for rider in race.riders.values_mut() {
        rider.s = at + next(0.0, 0.9);
        rider.x = across + next(-1.2, 1.2);
        rider.vs = next(-1.0, 9.0);
        rider.vx = next(-2.0, 2.0);
        rider.z = if next(0.0, 1.0) < 0.2 { next(0.0, 0.6) } else { 0.0 };
        rider.stun = if next(0.0, 1.0) < 0.2 { next(1.0, 45.0) as u32 } else { 0 };
        rider.rest = if next(0.0, 1.0) < 0.2 { next(1.0, 45.0) as u32 } else { 0 };
        rider.keys = next(0.0, 4.0) as u8;
        rider.haul = next(0.0, 3.0) as u16;
        match next(0.0, 10.0) as u32 {
            0 => (rider.crash, rider.wreck, rider.z) = (next(1.0, 46.0) as u32, Wreck::Fall, next(-3.0, 1.0)),
            1 => (rider.crash, rider.wreck) = (next(1.0, 21.0) as u32, Wreck::Bonk),
            2 => rider.finish = Some(race_t - next(0.0, 20.0) as u32),
            _ => {}
        }
    }
    race
}

#[test]
fn the_leg_bumps_and_picks_up_exactly_as_the_native_rules_did() {
    let _alone = ENGINE.lock().unwrap_or_else(|p| p.into_inner());
    crate::agent::harness::live_policy::install_scripted_rules();
    let mut heard = BTreeSet::new();
    for seed in 0..600 {
        heard.extend(parity_of(crowd(seed), 4));
    }
    for sound in ["roll", "clang", "gold", "knight_fall", "helmet_pop", "wheel_land", "step_wet"] {
        assert!(heard.contains(sound), "no crowd was ever heard to make {sound}: {heard:?}");
    }
}

/// One test at a time on the engine's pool: the timing below is the leg's own,
/// not the parity's neighbours.
static ENGINE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The Shaft Run ticks at `HZ` (30) inside a 33 ms frame the whole game
/// shares: this holds the leg's own slice of it, and reports the bytes.
#[test]
fn the_shaft_tick_is_a_small_slice_of_a_frame_at_four_riders() {
    let _alone = ENGINE.lock().unwrap_or_else(|p| p.into_inner());
    crate::agent::harness::live_policy::install_scripted_rules();
    let mut race = field(4);
    let ids: Vec<u32> = race.riders.keys().copied().collect();
    let inputs: BTreeMap<u32, Input> = ids.iter().map(|&id| (id, hand(400, id))).collect();
    // warm the pool, then measure a thousand ticks
    for _ in 0..32 {
        leg::run(&mut race, &inputs, &ids).expect("the leg answers");
        race.t += 1;
    }
    let mut micros = Vec::with_capacity(1000);
    for _ in 0..1000 {
        let start = std::time::Instant::now();
        leg::run(&mut race, &inputs, &ids).expect("the leg answers");
        micros.push(start.elapsed().as_secs_f64() * 1e6);
        race.t += 1;
    }
    micros.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let (median, p95) = (micros[micros.len() / 2], micros[micros.len() * 95 / 100]);
    let ask = serde_json::json!({
        "op": "tick", "race": race, "inputs": inputs, "standing": ids, "track": track(Course::Shaft), "sine": {},
    });
    let sent = ask.to_string().len();
    let answered: serde_json::Value = angel_delve::scripted::step(leg::LEG, &ask).expect("the leg answers");
    let answered = answered.to_string().len();
    let frame_us = 1_000_000.0 / f64::from(HZ);
    eprintln!(
        "race3d shaft tick, 4 riders: median {median:.1} us, p95 {p95:.1} us, {:.2}% / {:.2}% of the {frame_us:.0} us frame; wire {sent} B in, {answered} B out",
        median / frame_us * 100.0,
        p95 / frame_us * 100.0
    );
    assert!(median < 4_000.0, "the tick took {median:.0} us of a {frame_us:.0} us frame");
}

/// The shipped leg rolls the Shaft Run alone: it hands an ore cart or a griffin
/// back untouched, which is why `Run::tick_race3d` ticks those two natively. A
/// leg that learns to fly them must pass the parity above on them first, and
/// then the race may ask it.
#[test]
fn the_shipped_leg_rolls_the_shaft_alone_and_leaves_the_cart_and_the_griffin_to_rust() {
    let _alone = ENGINE.lock().unwrap_or_else(|p| p.into_inner());
    crate::agent::harness::live_policy::install_scripted_rules();
    for course in [Course::Shaft, Course::Cart, Course::Griffin] {
        let mut race = Race3d { course, ..field(2) };
        let ids: Vec<u32> = race.riders.keys().copied().collect();
        let up: BTreeMap<u32, Input> = ids.iter().map(|&id| (id, Input { move_y: -1, ..Input::default() })).collect();
        for _ in 0..120 {
            race.t += 1;
            leg::run(&mut race, &up, &ids).expect("the leg answers");
        }
        let moved = race.riders.values().any(|r| r.s > 2.5);
        assert_eq!(moved, course == Course::Shaft, "{course:?}: the leg moved it {moved}");
    }
}
