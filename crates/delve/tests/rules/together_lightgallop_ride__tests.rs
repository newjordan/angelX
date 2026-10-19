//! The whole ride, played by bots: the Moonwell Road cleared at every seat
//! count and by both routes, with the tick counts recorded.
use super::bot::Route;
use super::level::level;
use super::*;

/// A table of `seats` knights, each bot taking the route `route(id)` names.
fn table(seats: u32) -> Run {
    let mut r = Run::lightgallop_for_test();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.step(&BTreeMap::new());
    r
}

/// The keys every bot presses this tick.
fn bots(r: &Run, route: &dyn Fn(u32) -> Route) -> BTreeMap<u32, Input> {
    let s = r.lightgallop.as_ref().unwrap();
    let n = Numbers::default();
    s.seats.keys().map(|&id| (id, s.drive(id, route(id), &n))).collect()
}

struct Ride {
    ticks: u32,
    cleared: bool,
}

/// Ride until the tally or `limit` ticks, printing a line every `every`.
fn ride(r: &mut Run, route: &dyn Fn(u32) -> Route, limit: u32, every: u32) -> Ride {
    for tick in 0..limit {
        let inputs = bots(r, route);
        r.step(&inputs);
        let s = r.lightgallop.as_ref().unwrap();
        if every > 0 && tick % every == 0 {
            let line: Vec<String> = s
                .seats
                .iter()
                .map(|(id, st)| {
                    let k = &st.rider;
                    format!(
                        "{id}@{}/{} v{} sp{} g{} {}{}{}",
                        k.x / 256,
                        k.y / 256,
                        k.speed(),
                        k.sparks,
                        k.gauge,
                        if k.roll { "R" } else { "" },
                        if k.ground.is_none() { "A" } else { "" },
                        if k.down > 0 { "D" } else { "" }
                    )
                })
                .collect();
            eprintln!("t{tick} dim{} posts{:b} | {}", s.dim / 256, s.posts, line.join("  "));
        }
        if s.tally.is_some() {
            return Ride { ticks: s.elapsed, cleared: true };
        }
    }
    Ride { ticks: limit, cleared: false }
}

#[test]
#[ignore = "prints a solo bot's ride for tuning"]
fn the_ride_report() {
    let mut r = table(1);
    let out = ride(&mut r, &|_| Route::Upper, 9000, 60);
    eprintln!("cleared {} in {} ticks", out.cleared, out.ticks);
}

#[test]
#[ignore = "prints the tick counts of every table and route"]
fn the_clear_report() {
    for seats in [1, 2, 4, 6] {
        for (name, route) in [
            ("upper", (&|_| Route::Upper) as &dyn Fn(u32) -> Route),
            ("lower", &|_| Route::Lower),
            ("mixed", &Route::of_seat),
        ] {
            let mut r = table(seats);
            let out = ride(&mut r, route, 12000, 0);
            let s = r.lightgallop.as_ref().unwrap();
            let y = s.tally.as_ref();
            eprintln!(
                "{seats} seats {name}: cleared {} in {} ticks ({}s) rank {:?} points {:?} falls {:?} posts {}",
                out.cleared,
                out.ticks,
                out.ticks / 30,
                y.map(|y| y.rank),
                y.map(|y| y.points),
                y.map(|y| y.falls),
                s.posts.count_ones()
            );
        }
    }
}

/// Every bot's gold and laurel, as the run holds them.
fn gold(r: &Run, id: u32) -> u32 {
    r.players[&id].carried.get(Spoil::Gold)
}

#[test]
fn a_bot_clears_the_road_at_one_two_four_and_six_seats_by_either_route() {
    // Recorded tick counts, on the 30 Hz sim: a whole ride with its finale is
    // two minutes or so for a bot that never errs; a knight's first ride runs
    // longer. The bounds hold the road to its 2 to 4 minutes.
    for seats in [1, 2, 4, 6] {
        for (name, route) in [
            ("upper", (&|_| Route::Upper) as &dyn Fn(u32) -> Route),
            ("lower", &|_| Route::Lower),
            ("mixed", &Route::of_seat),
        ] {
            let mut r = table(seats);
            let out = ride(&mut r, route, 9000, 0);
            assert!(out.cleared, "{seats} seats, {name}: cleared");
            assert!(
                (2700..=4800).contains(&out.ticks),
                "{seats} seats, {name}: {} ticks is {} seconds",
                out.ticks,
                out.ticks / 30
            );
            let s = r.lightgallop.as_ref().unwrap();
            assert!(s.valid() && s.seats.values().all(|k| k.rider.finish.is_some()));
            assert_eq!(s.posts.count_ones(), 6, "every lantern lit by the pack");
        }
    }
}

#[test]
fn both_routes_are_really_ridden_the_rails_by_the_fast_and_the_valley_by_the_hop() {
    let rails = |route: Route| {
        let mut r = table(1);
        let mut on_rail = false;
        let mut in_valley = false;
        for _ in 0..4200 {
            let inputs = bots(&r, &|_| route);
            r.step(&inputs);
            let s = r.lightgallop.as_ref().unwrap();
            let k = &s.seats[&1].rider;
            let lvl = level();
            if let Some(g) = k.ground {
                let kind = lvl.terrain.paths[g.path as usize].kind;
                on_rail |= kind == terrain::Kind::Rail && (8720..9900).contains(&(k.x / 256));
                in_valley |= kind == terrain::Kind::Road && (8720 + 330..8720 + 580).contains(&(k.x / 256)) && k.y / 256 > 600 + 140 + 40;
            }
            if s.tally.is_some() {
                break;
            }
        }
        (on_rail, in_valley)
    };
    let (high_rail, high_valley) = rails(Route::Upper);
    assert!(high_rail && !high_valley, "the upper bot rode the rails and not the valley");
    let (low_rail, low_valley) = rails(Route::Lower);
    assert!(!low_rail && low_valley, "the lower bot rode the valley and not the rails");
}

#[test]
fn the_same_ride_replays_byte_for_byte() {
    let play = |seats| {
        let mut r = table(seats);
        ride(&mut r, &Route::of_seat, 1500, 0);
        serde_json::to_vec(&r).unwrap()
    };
    assert_eq!(play(6), play(6));
    assert_eq!(play(2), play(2));
}

#[test]
fn the_purse_is_paid_once_at_the_finale_and_a_repeat_pays_a_token() {
    let mut r = table(3);
    let before: Vec<u32> = (1..=3).map(|id| gold(&r, id)).collect();
    let out = ride(&mut r, &Route::of_seat, 9000, 0);
    assert!(out.cleared);
    let rank = r.lightgallop.as_ref().unwrap().tally.as_ref().unwrap().rank;
    let paid: Vec<u32> = (1..=3).map(|id| gold(&r, id) - before[id as usize - 1]).collect();
    assert_eq!(paid, [PURSE[rank as usize]; 3], "each seat, the rank's purse, the first time");
    assert!(paid[0] <= 40, "bounded: forty gold a seat at the most");
    assert!(r.marks.contains_key(CAMPAIGN_MARK) && r.home.roundtable[SLOT] & BEGUN != 0);
    // More ticks at the tally pay nothing more.
    for _ in 0..200 {
        r.step(&BTreeMap::new());
    }
    assert_eq!((1..=3).map(|id| gold(&r, id) - before[id as usize - 1]).collect::<Vec<_>>(), paid);
    // The records keep the best and the win.
    let rec = &r.home.countryside.games.roundtable[SLOT];
    assert_eq!((rec.plays, rec.wins), (1, 1));
    assert!(rec.best > 400 && rec.best <= 1000);
    // The laurel on the best rider only.
    let best = r.lightgallop.as_ref().unwrap().tally.as_ref().unwrap().best;
    assert!(r.players.iter().all(|(&id, h)| h.laurel == (id == best)));
    // Ride again: a token, not the full purse.
    let again = r.players[&1].carried.get(Spoil::Gold);
    r.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    r.step(&BTreeMap::new());
    assert!(r.lightgallop.as_ref().unwrap().tally.is_none(), "a fresh ride");
    let out = ride(&mut r, &Route::of_seat, 9000, 0);
    assert!(out.cleared);
    let second = r.players[&1].carried.get(Spoil::Gold) - again;
    assert_eq!(second, REPLAY, "a repeat clear pays the token");
    let rec = &r.home.countryside.games.roundtable[SLOT];
    assert_eq!((rec.plays, rec.wins), (2, 2));
}

#[test]
fn the_tally_waits_for_a_beat_then_one_press_of_e_starts_a_fresh_ride_in_one_tick() {
    let mut r = table(2);
    let out = ride(&mut r, &Route::of_seat, 9000, 0);
    assert!(out.cleared);
    let press = BTreeMap::from([(2, Input { interact: true, ..Default::default() })]);
    // A press at once is the knight's last leap, not a rematch.
    r.step(&press);
    r.step(&BTreeMap::new());
    assert!(r.lightgallop.as_ref().unwrap().tally.is_some());
    for _ in 0..60 {
        r.step(&BTreeMap::new());
    }
    let before = r.tick;
    r.step(&press);
    assert!(r.tick - before <= 1);
    let s = r.lightgallop.as_ref().unwrap();
    assert!(s.tally.is_none() && s.elapsed <= 1 && s.seats.len() == 2, "a new road under both knights");
    assert!(s.seats.values().all(|k| k.rider.sparks == 0 && k.rider.x / 256 < 120));
}

#[test]
fn a_scripted_loss_is_swallowed_carried_home_and_riding_again_inside_a_second() {
    let mut r = table(1);
    // Stand still at the gate until the Dimming comes.
    let mut fell_at = None;
    let mut up_at = None;
    for t in 0..900 {
        r.step(&BTreeMap::new());
        let k = &r.lightgallop.as_ref().unwrap().seats[&1].rider;
        if fell_at.is_none() && k.down > 0 {
            fell_at = Some(t);
        }
        if fell_at.is_some() && up_at.is_none() && k.down == 0 {
            up_at = Some(t);
            break;
        }
    }
    let (fell, up) = (fell_at.expect("swallowed"), up_at.expect("carried home"));
    assert!(up - fell <= 30, "back on the road in {} ticks", up - fell);
    let s = r.lightgallop.as_ref().unwrap();
    assert!(s.valid() && s.seats[&1].rider.falls == 1);
    // And the ride goes on, the world's progress kept: now clear it.
    let out = ride(&mut r, &|_| Route::Upper, 9000, 0);
    assert!(out.cleared, "a loss is a setback, never an ending");
}

#[test]
fn a_whole_table_downed_at_once_is_carried_to_the_post_and_the_ride_goes_on() {
    let mut r = table(4);
    // Everyone rides to the first lantern, then stands and waits.
    for _ in 0..900 {
        let inputs = bots(&r, &Route::of_seat);
        r.step(&inputs);
        if r.lightgallop.as_ref().unwrap().last_post().is_some() {
            break;
        }
    }
    assert_eq!(r.lightgallop.as_ref().unwrap().last_post(), Some(0), "the first post is lit");
    // The Gloam sweeps over the whole table in one tick.
    r.lightgallop.as_mut().unwrap().dim = 9000 * 256;
    for _ in 0..60 {
        r.step(&BTreeMap::new());
    }
    let s = r.lightgallop.as_ref().unwrap();
    assert!(s.seats.values().all(|k| k.rider.falls >= 1), "the Dimming took them all");
    let post = level().posts[0].x;
    assert!(s.seats.values().all(|k| (k.rider.x / 256 - post).abs() < 40), "carried to the lit post");
    let out = ride(&mut r, &Route::of_seat, 9000, 0);
    assert!(out.cleared);
}

#[test]
fn a_late_knight_joins_the_ride_in_flight_and_the_ride_still_clears() {
    let mut r = table(2);
    ride(&mut r, &Route::of_seat, 900, 0);
    r.join_seat(3, "Late Knight");
    r.step(&BTreeMap::new());
    let s = r.lightgallop.as_ref().unwrap();
    assert_eq!(s.seats.len(), 3);
    assert!(s.seats[&3].joined > 800);
    let post = s.last_post().map(|n| level().posts[n].x);
    assert!(post.is_none_or(|p| (s.seats[&3].rider.x / 256 - p).abs() <= 2), "set down at the pack's post");
    let out = ride(&mut r, &Route::of_seat, 9000, 0);
    assert!(out.cleared);
    assert!(gold(&r, 3) >= 10, "the late knight is paid too");
}

#[test]
fn a_seat_that_retreats_stands_up_and_the_rest_ride_on() {
    let mut r = table(3);
    ride(&mut r, &Route::of_seat, 500, 0);
    for _ in 0..exits::LEAVE_TICKS {
        let mut inputs = bots(&r, &Route::of_seat);
        inputs.insert(2, Input { leave_held: true, ..Default::default() });
        r.step(&inputs);
    }
    r.step(&BTreeMap::new());
    assert!(r.exits.outside.contains_key(&2));
    assert_eq!(r.lightgallop.as_ref().unwrap().seats.keys().copied().collect::<Vec<_>>(), [1, 3]);
    // The knight outside wanders the kingdom (an idle one would be taken back
    // in as away) while the others ride on.
    let mut cleared = false;
    for tick in 0..9000u32 {
        let mut inputs = bots(&r, &Route::of_seat);
        inputs.insert(2, Input { move_x: if tick / 30 % 2 == 0 { 1 } else { -1 }, ..Default::default() });
        r.step(&inputs);
        if r.lightgallop.as_ref().unwrap().tally.is_some() {
            cleared = true;
            break;
        }
    }
    assert!(cleared);
    assert!(r.exits.outside.contains_key(&2) && !r.lightgallop.as_ref().unwrap().seats.contains_key(&2));
    assert_eq!(gold(&r, 2), 0, "the one who left is not paid");
    assert!(gold(&r, 1) > 0 && gold(&r, 3) > 0);
}

#[test]
fn every_state_the_ride_reaches_is_valid_and_survives_the_wire() {
    let mut r = table(6);
    for tick in 0..2400 {
        let inputs = bots(&r, &Route::of_seat);
        r.step(&inputs);
        let s = r.lightgallop.as_ref().unwrap();
        if tick % 60 == 0 {
            assert!(s.valid(), "tick {tick}");
            assert_eq!(r.wire_copy().lightgallop.as_ref(), Some(s));
        }
    }
    let json = serde_json::to_string(&r).unwrap();
    let back: Run = serde_json::from_str(&json).unwrap();
    assert_eq!(back.lightgallop, r.lightgallop);
    assert!(back.valid_snapshot() && crate::wire::drawable(&back));
}

#[test]
fn a_broken_ride_is_refused_and_repair_mends_it() {
    let mut r = table(2);
    ride(&mut r, &Route::of_seat, 300, 0);
    let good = r.lightgallop.clone().unwrap();
    let mut bad = good.clone();
    bad.seats.get_mut(&1).unwrap().rider.gsp = 99_999;
    assert!(!bad.valid());
    bad.repair();
    assert!(bad.valid());
    let mut bad = good.clone();
    bad.foes.pop();
    bad.posts = 1 << 20;
    bad.dim = i32::MAX;
    assert!(!bad.valid());
    bad.repair();
    assert!(bad.valid());
    let mut bad = good.clone();
    bad.tally = Some(Tally { at: 0, time: 0, sparks: 0, chain: 0, posts: 0, falls: 0, points: 9999, rank: 9, best: 1, scores: BTreeMap::new() });
    assert!(!bad.valid());
    bad.repair();
    assert!(bad.valid() && bad.tally.is_none());
    r.lightgallop.as_mut().unwrap().elapsed = LIMIT + 1;
    assert!(!r.valid_snapshot());
}

#[test]
#[ignore = "prints the lower route's fork"]
fn the_fork_report() {
    let mut r = table(1);
    for _ in 0..4200 {
        let inputs = bots(&r, &|_| Route::Lower);
        r.step(&inputs);
        let s = r.lightgallop.as_ref().unwrap();
        let k = &s.seats[&1].rider;
        if (8600..9500).contains(&(k.x / 256)) && s.elapsed % 6 == 0 {
            eprintln!(
                "x{} y{} v{} {:?} jump{} air{}",
                k.x / 256,
                k.y / 256,
                k.gsp,
                k.ground.map(|g| g.path),
                k.jumping,
                k.ground.is_none()
            );
        }
    }
}

use super::bot::Skill;
use super::rider::{Ev, Keys, Rider};

/// What a tuning round saw.
#[derive(Default, Debug)]
struct Round {
    cleared: bool,
    ticks: u32,
    falls: u32,
    hits: u32,
    loop_falls: u32,
    best_chain: u32,
    caught: u32,
}

/// One ride with the five numbers as given, every bot of `skill` on `route`.
fn round(seats: u32, skill: Skill, route: Route, set: &[(&str, f64)], limit: u32) -> Round {
    let mut r = table(seats);
    for &(name, v) in set {
        assert!(r.tune.set(name, v));
    }
    let mut out = Round::default();
    let mut was_looping: BTreeMap<u32, bool> = BTreeMap::new();
    for _ in 0..limit {
        let n = r.lightgallop_numbers();
        let s = r.lightgallop.as_ref().unwrap();
        let inputs: BTreeMap<u32, Input> = s.seats.keys().map(|&id| (id, s.drive_as(id, route, &n, skill))).collect();
        let before: u32 = s.seats.values().map(|k| k.rider.falls).sum();
        r.step(&inputs);
        let s = r.lightgallop.as_ref().unwrap();
        for (&id, k) in &s.seats {
            let was = was_looping.insert(id, k.rider.looping).unwrap_or(false);
            if was && !k.rider.looping && !k.rider.grounded() {
                out.loop_falls += 1;
            }
        }
        let after: u32 = s.seats.values().map(|k| k.rider.falls).sum();
        out.caught += after - before;
        if s.tally.is_some() {
            out.cleared = true;
            break;
        }
    }
    let s = r.lightgallop.as_ref().unwrap();
    out.ticks = s.elapsed;
    out.falls = s.seats.values().map(|k| k.rider.falls).sum();
    out.hits = s.seats.values().map(|k| k.rider.hits).sum();
    out.best_chain = s.seats.values().map(|k| k.rider.best_chain).max().unwrap_or(0);
    out
}

#[test]
#[ignore = "plays the tuning rounds and prints the table"]
fn the_tuning_report() {
    let rows = |title: &str, name: &str, values: &[f64]| {
        eprintln!("## {title}");
        for &v in values {
            for (who, skill) in [("ace", Skill::ACE), ("first ride", Skill::FIRST_RIDE)] {
                let a = round(1, skill, Route::Upper, &[(name, v)], 12000);
                let b = round(4, skill, Route::Upper, &[(name, v)], 12000);
                eprintln!(
                    "| {name}={v} | {who} | solo: cleared {} {}s falls {} hits {} loop-falls {} chain {} | four: cleared {} {}s falls {} hits {} loop-falls {} chain {} |",
                    a.cleared, a.ticks / 30, a.falls, a.hits, a.loop_falls, a.best_chain,
                    b.cleared, b.ticks / 30, b.falls, b.hits, b.loop_falls, b.best_chain
                );
            }
        }
    };
    rows("slope", "lightgallop_slope", &[40.0, 70.0, 100.0, 150.0, 250.0]);
    rows("loop", "lightgallop_loop", &[256.0, 512.0, 768.0, 1024.0, 1280.0]);
    rows("spur", "lightgallop_spur", &[1536.0, 1792.0, 2048.0, 2560.0, 3072.0]);
    rows("lock", "lightgallop_lock", &[40.0, 64.0, 96.0, 160.0, 240.0]);
    rows("dimming", "lightgallop_dimming", &[350.0, 560.0, 700.0, 900.0, 1200.0, 1600.0]);
}

#[test]
#[ignore = "prints the Dimming's table by seat count and the loop's passing speed by number"]
fn the_dimming_and_loop_report() {
    eprintln!("## dimming by seats");
    for d in [700.0, 900.0, 1000.0, 1100.0, 1300.0] {
        for (who, skill) in [("first ride", Skill::FIRST_RIDE), ("slow", Skill::SLOW)] {
            let rows: Vec<String> = [1, 2, 4, 6]
                .iter()
                .map(|&seats| {
                    let x = round(seats, skill, Route::Upper, &[("lightgallop_dimming", d)], 15000);
                    format!("{seats}: {} {}s caught {} hits {}", if x.cleared { "cleared" } else { "NOT" }, x.ticks / 30, x.caught, x.hits)
                })
                .collect();
            eprintln!("| dimming={d} | {who} | {} |", rows.join(" | "));
        }
    }
    eprintln!("## loop: the slowest entry speed (1/256 px a tick) that goes round");
    for l in [256, 512, 768, 1024, 1280] {
        let lvl = level();
        let mut least = None;
        for v in (400..2600).step_by(20) {
            let mut r = Rider::new_on(&lvl.terrain, lvl.main, lvl.terrain.paths[lvl.main].s_at_x(4690));
            r.gsp = v;
            let n = Numbers { loop_min: l, ..Numbers::default() };
            let mut ev = Vec::new();
            for _ in 0..200 {
                r.step(Keys { x: 1, ..Keys::default() }, &lvl.terrain, &n, &mut ev);
            }
            if !ev.contains(&Ev::Fell) && r.x / 256 > 4800 {
                least = Some(v);
                break;
            }
        }
        eprintln!("| loop={l} | {least:?} ({} px a tick) |", least.map_or(0.0, |v| v as f32 / 256.0));
    }
}

#[test]
#[ignore = "debug"]
fn the_valid_debug() {
    for seats in [1, 2, 4, 6] {
        let mut r = table(seats);
        let out = ride(&mut r, &|_| Route::Upper, 9000, 0);
        let s = r.lightgallop.as_ref().unwrap();
        let lvl = level();
        eprintln!("{seats}: cleared {} valid {} elapsed {} dim {} posts {:b} riders {:?}", out.cleared, s.valid(),
            s.elapsed, s.dim / 256, s.posts, s.seats.values().map(|k| (k.rider.valid(&lvl.terrain), k.rider.finish.is_some(), k.rider.x/256, k.rider.gsp, k.rider.freeze, k.rider.shake)).collect::<Vec<_>>());
    }
}

#[test]
#[ignore = "prints the road's census"]
fn the_census_report() {
    let l = level();
    eprintln!(
        "width {} sparks {} springs {} pads {} posts {} crates {} signs {} paths {}",
        l.width,
        l.sparks.len(),
        l.springs.len(),
        l.pads.len(),
        l.posts.len(),
        l.crates.len(),
        l.signs.len(),
        l.terrain.paths.len()
    );
    for n in 1..=6 {
        let awake = l.foes.iter().filter(|f| f.min_seats <= n).count();
        let kinds = |k: super::world::FoeKind| l.foes.iter().filter(|f| f.min_seats <= n && f.kind == k).count();
        eprintln!(
            "{n} seats: {awake} foes ({} snuffers {} floaters {} brambles)",
            kinds(super::world::FoeKind::Snuffer),
            kinds(super::world::FoeKind::Floater),
            kinds(super::world::FoeKind::Bramble)
        );
    }
}

#[test]
#[ignore = "debug the first seconds"]
fn the_first_seconds_debug() {
    let mut r = table(1);
    for t in 0..60 {
        let s = r.lightgallop.as_ref().unwrap();
        let n = Numbers::default();
        let inputs: BTreeMap<u32, Input> = s.seats.keys().map(|&id| (id, s.drive_as(id, Route::Upper, &n, Skill::FIRST_RIDE))).collect();
        if t % 6 == 0 {
            let k = &s.seats[&1].rider;
            eprintln!("t{t} x{} gsp{} gauge{} spur{} in {:?}", k.x / 256, k.gsp, k.gauge, k.spur, inputs[&1]);
        }
        r.step(&inputs);
    }
}
