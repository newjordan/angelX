//! Lightgallop's momentum physics, proved on grey curves before any level is
//! built: the flat, the hill, the half-pipe and the full loop. Integer
//! throughout, so every run here replays exactly.
use super::math::{Q, atan2};
use super::rider::*;
use super::terrain::{Builder, Kind, Path, Terrain, attach_ring, ring};

const GROUND: i32 = 500;

fn road(f: impl FnOnce(&mut Builder)) -> Path {
    let mut b = Builder::new(Kind::Road, 0, GROUND);
    f(&mut b);
    b.build()
}

/// A flat road 3000 pixels long.
fn flat() -> Terrain {
    Terrain::new(vec![road(|b| {
        b.line(3000, GROUND);
    })])
}

/// A flat road with a loop of radius `r` standing on it at column 1000.
fn looped(r: i32) -> Terrain {
    let mut paths = vec![road(|b| {
        b.line(3000, GROUND);
    })];
    attach_ring(&mut paths, 0, ring(1000, GROUND, r, 16));
    Terrain::new(paths)
}

/// What a body did on a course.
struct Trace {
    r: Rider,
    ev: Vec<Ev>,
}

fn rider_at(t: &Terrain, x: i32) -> Rider {
    let s = t.paths[0].s_at_x(x);
    Rider::new_on(t, 0, s)
}

fn run(t: &Terrain, r: &mut Rider, k: Keys, ticks: u32) -> Vec<Ev> {
    let n = Numbers::default();
    let mut ev = Vec::new();
    for _ in 0..ticks {
        r.step(k, t, &n, &mut ev);
    }
    ev
}

const RIGHT: Keys = Keys { x: 1, down: false, jump: false, spur: false };
const LEFT: Keys = Keys { x: -1, down: false, jump: false, spur: false };
const NONE: Keys = Keys { x: 0, down: false, jump: false, spur: false };

#[test]
fn a_gallop_builds_to_a_real_top_speed_and_holds_it() {
    let t = flat();
    let mut r = rider_at(&t, 100);
    run(&t, &mut r, RIGHT, 20);
    assert!(r.gsp > 400 && r.gsp < TOP, "still gaining: {}", r.gsp);
    run(&t, &mut r, RIGHT, 80);
    assert_eq!(r.gsp, TOP, "the pace is a ceiling");
    run(&t, &mut r, RIGHT, 80);
    assert_eq!(r.gsp, TOP);
    assert!(r.x / Q > 600 && r.y == GROUND * Q, "on the road the whole way");
}

#[test]
fn letting_go_coasts_to_a_stop_and_never_backs_up() {
    let t = flat();
    let mut r = rider_at(&t, 100);
    run(&t, &mut r, RIGHT, 120);
    let mut last = r.gsp;
    for _ in 0..200 {
        run(&t, &mut r, NONE, 1);
        assert!(r.gsp <= last && r.gsp >= 0, "{}", r.gsp);
        last = r.gsp;
    }
    assert_eq!(r.gsp, 0);
}

#[test]
fn braking_against_the_run_skids_then_turns() {
    let t = flat();
    let mut r = rider_at(&t, 100);
    run(&t, &mut r, RIGHT, 120);
    let ev = run(&t, &mut r, LEFT, 1);
    assert!(r.skid && ev.contains(&Ev::Skid), "the skid shows");
    assert_eq!(r.dir, 1, "still facing the way he was going while he skids");
    let mut ticks = 1;
    while r.gsp > 0 {
        run(&t, &mut r, LEFT, 1);
        ticks += 1;
        assert!(ticks < 20, "a brake from the top takes under a second");
    }
    run(&t, &mut r, LEFT, 4);
    assert!(r.gsp < 0 && r.dir == -1, "then he runs the other way: {} {}", r.gsp, r.dir);
}

/// A road down a 30-degree ramp, level before and after.
fn ramp() -> Terrain {
    Terrain::new(vec![road(|b| {
        b.line(400, GROUND).line(700, GROUND + 173).line(1100, GROUND + 173);
    })])
}

#[test]
fn a_downhill_gives_speed_and_an_uphill_takes_it() {
    let t = ramp();
    // Running down: well past the pace by the bottom.
    let mut r = rider_at(&t, 60);
    run(&t, &mut r, RIGHT, 70);
    assert_eq!(r.gsp, TOP, "on the flat before the slope");
    let mut fastest = 0;
    for _ in 0..80 {
        run(&t, &mut r, RIGHT, 1);
        fastest = fastest.max(r.gsp);
    }
    assert!(fastest > TOP + 160, "the downhill carries him past the pace: {fastest}");
    // Running up the same ramp the other way, already at the pace.
    let mut r = rider_at(&t, 1000);
    r.gsp = -TOP;
    r.dir = -1;
    let mut slowest = TOP;
    for _ in 0..90 {
        run(&t, &mut r, LEFT, 1);
        if r.y > (GROUND + 20) * Q {
            slowest = slowest.min(-r.gsp);
        }
    }
    assert!(slowest < TOP - 100, "the climb takes speed: {slowest}");
    assert!(slowest > 0, "but he still climbs");
}

#[test]
fn a_rider_never_stops_dead_on_a_slope_but_a_gentle_one_holds_him() {
    let steep = Terrain::new(vec![road(|b| {
        b.line(400, GROUND).line(700, GROUND + 210).line(1100, GROUND + 210);
    })]);
    let mut r = rider_at(&steep, 550);
    run(&steep, &mut r, NONE, 12);
    assert!(r.gsp > 30, "a 35 degree slope slides him: {}", r.gsp);
    // A gentle rise: 8 degrees.
    let gentle = Terrain::new(vec![road(|b| {
        b.line(400, GROUND).line(1000, GROUND + 84);
    })]);
    let mut r = rider_at(&gentle, 700);
    run(&gentle, &mut r, NONE, 30);
    assert_eq!(r.gsp, 0, "friction holds him on a gentle slope");
}

#[test]
fn a_half_pipe_swings_him_up_the_far_wall_and_back() {
    // A bowl: a quarter circle down, flat, a quarter circle up (radius 60).
    let t = Terrain::new(vec![road(|b| {
        b.line(300, GROUND - 60)
            .arc(300, GROUND - 60, 60, 128, 64, 0)
            .line(380, GROUND)
            .arc(380, GROUND - 60, 60, 64, 0, 0)
            .line(500, GROUND - 60);
    })]);
    let mut r = rider_at(&t, 100);
    run(&t, &mut r, RIGHT, 40);
    let mut highest_far = i32::MAX;
    let mut bottom_speed = 0;
    for _ in 0..140 {
        run(&t, &mut r, RIGHT, 1);
        if r.x / Q > 330 && r.x / Q < 440 {
            bottom_speed = bottom_speed.max(r.gsp);
        }
        if r.x / Q > 420 {
            highest_far = highest_far.min(r.y);
        }
    }
    eprintln!("half-pipe bottom {bottom_speed} far {}", highest_far / Q);
    assert!(bottom_speed > TOP + 100, "the wall gave speed: {bottom_speed}");
    assert!(highest_far < (GROUND - 30) * Q, "up the far wall: {}", highest_far / Q);
}

/// How a run at a loop ended.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    /// Round the loop and out onto the road beyond.
    Through,
    /// Fell off the loop's wall or ceiling.
    Fell,
    /// Never made the climb and rolled back out the way he came.
    Back,
}

/// Start a rider at column `from` with ground speed `speed` and the keys
/// held, and say what the loop did to him.
fn at_the_loop(r_loop: i32, from: i32, speed: i32, k: Keys) -> (Outcome, Trace) {
    let t = looped(r_loop);
    let mut r = rider_at(&t, from);
    r.gsp = speed;
    let n = Numbers::default();
    let mut ev = Vec::new();
    let mut in_ring = false;
    for _ in 0..400 {
        r.step(k, &t, &n, &mut ev);
        in_ring |= r.looping;
        if !in_ring {
            continue;
        }
        if ev.contains(&Ev::Fell) {
            return (Outcome::Fell, Trace { r, ev });
        }
        if r.grounded() && !r.looping && r.x / Q > 1010 {
            return (Outcome::Through, Trace { r, ev });
        }
        if r.grounded() && !r.looping && r.x / Q < 995 {
            return (Outcome::Back, Trace { r, ev });
        }
    }
    (Outcome::Back, Trace { r, ev })
}

#[test]
#[ignore = "prints where the loop lets a rider through, for tuning"]
fn the_loop_report() {
    for r_loop in [36, 40] {
        for speed in [512, 768, 960, 1024, 1100, 1180, 1280, 1536, 2048] {
            let (o, tr) = at_the_loop(r_loop, 994, speed, RIGHT);
            eprintln!("R{r_loop} v{speed}: {o:?} ends at {} gsp {}", tr.r.x / Q, tr.r.gsp);
        }
    }
}

#[test]
fn a_loop_holds_a_fast_rider_on_the_ceiling_and_drops_a_slow_one_fairly() {
    let (o, tr) = at_the_loop(36, 994, 1536, RIGHT);
    assert_eq!(o, Outcome::Through, "{:?} {}", tr.ev.len(), tr.r.x / Q);
    assert!(tr.ev.contains(&Ev::Loop));
    let (o, _) = at_the_loop(36, 994, 2048, RIGHT);
    assert_eq!(o, Outcome::Through);
    let (o, _) = at_the_loop(36, 994, 900, RIGHT);
    assert_eq!(o, Outcome::Fell);
    let (o, _) = at_the_loop(36, 994, 300, RIGHT);
    assert_eq!(o, Outcome::Back);
}

#[test]
fn the_same_ticks_replay_the_same_rider() {
    let t = looped(40);
    let play = || {
        let mut r = rider_at(&t, 600);
        let n = Numbers::default();
        let mut ev = Vec::new();
        for tick in 0..300 {
            let k = Keys { x: 1, down: tick % 97 == 90, jump: tick % 53 < 4, spur: (100..180).contains(&tick) };
            r.step(k, &t, &n, &mut ev);
        }
        (serde_json::to_string(&r).unwrap(), ev)
    };
    assert_eq!(play(), play());
}

#[test]
fn the_lean_follows_the_road_and_turns_all_the_way_round_a_loop() {
    let t = looped(40);
    let mut r = rider_at(&t, 800);
    r.gsp = 1800;
    let n = Numbers::default();
    let mut ev = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut last = r.ang;
    let mut biggest_step = 0;
    for _ in 0..140 {
        r.step(RIGHT, &t, &n, &mut ev);
        if r.looping {
            seen.insert(r.ang / 128);
            let d = ((r.ang - last + 512).rem_euclid(1024) - 512).abs();
            biggest_step = biggest_step.max(d);
        }
        last = r.ang;
    }
    assert_eq!(seen.len(), 8, "every eighth of the circle was ridden: {seen:?}");
    assert!(biggest_step <= 40, "never a jump in the lean: {biggest_step}");
    let _ = atan2(0, 1);
}

#[test]
fn a_spur_makes_the_loop_from_a_short_run_and_a_trot_does_not() {
    let t = looped(36);
    let go = |x: i32, spur: bool| {
        let mut r = rider_at(&t, x);
        let n = Numbers::default();
        let mut ev = Vec::new();
        let k = Keys { x: 1, spur, ..Keys::default() };
        for _ in 0..300 {
            r.step(k, &t, &n, &mut ev);
        }
        (r.x / Q, ev.contains(&Ev::Fell))
    };
    // Forty pixels of run-up: a trot at the mouth.
    let (x, fell) = go(960, false);
    assert!(fell, "a trot falls off: {x}");
    // The same forty pixels with the spur held: round and away.
    let (x, fell) = go(960, true);
    assert!(!fell && x > 1300, "the spur makes it: {x} {fell}");
    // A full gallop from a long run makes it with no spur at all.
    let (x, fell) = go(500, false);
    assert!(!fell && x > 1300, "a gallop holds the wall: {x} {fell}");
}

#[test]
fn a_leap_is_variable_and_forgives_a_late_or_early_press() {
    let t = flat();
    let apex = |hold: u32| {
        let mut r = rider_at(&t, 100);
        let n = Numbers::default();
        let mut ev = Vec::new();
        let mut top = r.y;
        for tick in 0..70 {
            let k = Keys { jump: tick < hold, ..Keys::default() };
            r.step(k, &t, &n, &mut ev);
            top = top.min(r.y);
        }
        assert!(r.grounded(), "back on the road");
        (GROUND * Q - top) / Q
    };
    let (tap, held) = (apex(2), apex(40));
    assert!(held >= 44 && held <= 52, "a held leap: {held}");
    assert!(tap >= 14 && tap < held - 12, "a tap is a hop: {tap}");
    // Coyote: a press a moment after running off a ledge still leaps.
    let ledge = Terrain::new(vec![road(|b| {
        b.line(300, GROUND);
    })]);
    let mut r = rider_at(&ledge, 100);
    let mut ev = Vec::new();
    let n = Numbers::default();
    while r.grounded() {
        r.step(RIGHT, &ledge, &n, &mut ev);
    }
    r.step(RIGHT, &ledge, &n, &mut ev);
    ev.clear();
    r.step(Keys { x: 1, jump: true, ..Keys::default() }, &ledge, &n, &mut ev);
    assert!(ev.contains(&Ev::Jump), "coyote: the leap comes");
    // A press just before landing is kept and leaps on the tick he lands.
    let mut r = rider_at(&t, 100);
    let mut ev = Vec::new();
    r.step(Keys { jump: true, ..Keys::default() }, &t, &n, &mut ev);
    while r.vy < 0 || r.y < GROUND * Q - 8 * Q {
        r.step(NONE, &t, &n, &mut ev);
    }
    ev.clear();
    r.step(Keys { jump: true, ..Keys::default() }, &t, &n, &mut ev);
    r.step(NONE, &t, &n, &mut ev);
    let mut leapt = false;
    for _ in 0..8 {
        r.step(NONE, &t, &n, &mut ev);
        leapt |= ev.iter().filter(|e| **e == Ev::Jump).count() == 1;
    }
    assert!(leapt, "the buffered press is not lost");
}

#[test]
fn down_while_moving_tucks_into_a_roll_that_runs_a_downhill_faster() {
    let t = ramp();
    let mut r = rider_at(&t, 60);
    run(&t, &mut r, RIGHT, 70);
    let ev = run(&t, &mut r, Keys { x: 1, down: true, ..Keys::default() }, 1);
    assert!(r.roll && ev.contains(&Ev::Roll) && r.body().1 == 14, "tucked and small");
    // A roll and a run side by side down the same ramp.
    let mut a = rider_at(&t, 60);
    let mut b = rider_at(&t, 60);
    run(&t, &mut a, RIGHT, 70);
    run(&t, &mut b, RIGHT, 70);
    run(&t, &mut b, Keys { x: 1, down: true, ..Keys::default() }, 1);
    run(&t, &mut a, RIGHT, 70);
    run(&t, &mut b, NONE, 69);
    assert!(b.gsp > a.gsp + 100, "the roll runs the slope faster: {} against {}", b.gsp, a.gsp);
    // A roll on the flat dies away and the rider stands.
    let f = flat();
    let mut r = rider_at(&f, 100);
    run(&f, &mut r, RIGHT, 70);
    run(&f, &mut r, Keys { down: true, ..Keys::default() }, 1);
    assert!(r.roll);
    run(&f, &mut r, NONE, 250);
    assert!(!r.roll && r.gsp == 0, "unrolled at rest");
}

#[test]
fn a_charge_dash_revs_on_taps_of_jump_and_bursts_on_release() {
    let f = flat();
    let mut r = rider_at(&f, 100);
    let n = Numbers::default();
    let mut ev = Vec::new();
    let down = Keys { down: true, ..Keys::default() };
    r.step(down, &f, &n, &mut ev);
    assert!(r.crouch && r.gsp == 0);
    let mut last = 0;
    for tap in 1..=4 {
        r.step(Keys { down: true, jump: true, ..Keys::default() }, &f, &n, &mut ev);
        r.step(down, &f, &n, &mut ev);
        assert!(r.charge > last - 120, "the rev climbs with each tap {tap}: {}", r.charge);
        last = r.charge;
    }
    assert!(ev.iter().any(|e| matches!(e, Ev::Rev(c) if *c >= 600)));
    assert!(r.grounded() && r.y == GROUND * Q, "the taps are revs, not leaps");
    r.step(NONE, &f, &n, &mut ev);
    assert!(ev.contains(&Ev::Launch) && r.roll);
    assert!(r.gsp >= CHARGE_BASE + 800, "released into a burst: {}", r.gsp);
    assert!(r.gsp > TOP, "past the pace");
}

#[test]
fn the_spur_bursts_past_the_pace_drains_its_gauge_and_stops_when_empty() {
    let f = flat();
    let mut r = rider_at(&f, 100);
    run(&f, &mut r, RIGHT, 70);
    let ev = run(&f, &mut r, Keys { x: 1, spur: true, ..Keys::default() }, 1);
    assert!(ev.contains(&Ev::Spur) && r.gsp > TOP + 200, "the burst: {}", r.gsp);
    let before = r.gauge;
    run(&f, &mut r, Keys { x: 1, spur: true, ..Keys::default() }, 30);
    assert!(r.gauge < before - 300, "it drains: {} -> {}", before, r.gauge);
    assert!(r.gsp > 1900, "near the spur's own cap: {}", r.gsp);
    run(&f, &mut r, Keys { x: 1, spur: true, ..Keys::default() }, 60);
    assert_eq!(r.gauge, 0);
    run(&f, &mut r, Keys { x: 1, spur: true, ..Keys::default() }, 120);
    assert!(r.gsp <= TOP, "an empty gauge is just a gallop again: {}", r.gsp);
}

#[test]
fn a_pack_s_wake_lifts_the_pace_a_little() {
    let f = flat();
    let n = Numbers::default();
    let mut ev = Vec::new();
    let mut alone = rider_at(&f, 100);
    let mut drafted = rider_at(&f, 100);
    drafted.draft = true;
    for _ in 0..140 {
        alone.step(RIGHT, &f, &n, &mut ev);
        drafted.step(RIGHT, &f, &n, &mut ev);
    }
    assert_eq!(alone.gsp, TOP);
    assert_eq!(drafted.gsp, TOP + DRAFT_CAP);
}
