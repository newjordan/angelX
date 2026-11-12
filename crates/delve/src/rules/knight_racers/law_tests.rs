//! The racer's law, tested a mechanic at a time: engine heat and wear, the limp, the blowout and
//! the squires' patch, the draft, the slam and the shove, the pilots' overtakes and the start.
//! Every pod here races on a straight, clear stretch of road found in the course itself, so a
//! number that moves, moved because of the mechanic under test.
use super::course::*;
use super::race::{contact, engines, pilot, Car, Drive, Session, COUNTDOWN};
use std::collections::BTreeMap;

const GAS: Drive = Drive { steer: 0, gas: true, brake: false, boost: false, interact: false };
const BOOST: Drive = Drive { steer: 0, gas: true, brake: false, boost: true, interact: false };
const COAST: Drive = Drive { steer: 0, gas: false, brake: false, boost: false, interact: false };

/// The longest clear straight in the courses: the course, where it starts and how long it is.
/// Clear means no bend worth the name, no hazard, jump, fork or boost strip on it or just before
/// or after it, and room for three pods abreast. Tests that need a long run keep their pods on it
/// by `step`, which slides the whole field back along it (a treadmill) when it nears the end.
fn stretch() -> (CourseId, f32, f32) {
    static BEST: std::sync::OnceLock<(CourseId, f32, f32)> = std::sync::OnceLock::new();
    *BEST.get_or_init(|| {
        let mut best = (CourseId::Emberfall, 0., 0.);
        for &id in cup().iter().chain([&CourseId::Proving]) {
            let c = course(id);
            let clear = |q: f32| {
                let f = c.frame(0, q);
                f.k.abs() < 0.003 && f.g.abs() < 0.04 && f.wl.min(f.wr) > 8.5
                    && !c.hazards().iter().any(|h| (h.s - q).abs() < 16.)
                    && !c.gaps().iter().any(|g| q > g.s0 - 70. && q < g.s1 + 30.)
                    && !c.forks().iter().any(|f| q > f.s0 - 50. && q < f.s1 + 40.)
                    && !c.strips().iter().any(|p| q > p.s0 - 8. && q < p.s1 + 8.)
            };
            let (mut run_from, mut q) = (None, 0.);
            while q < c.lap() {
                if clear(q) { run_from.get_or_insert(q); }
                else if let Some(from) = run_from.take() { if q - from > best.2 { best = (id, from, q - from); } }
                q += 5.;
            }
        }
        assert!(best.2 >= 300., "no clear straight in the courses: the longest is {best:?}");
        // Keep the ends out of it: the first and last stretches of road are where the clear run
        // blends into whatever stops it.
        (best.0, best.1 + 25., best.2 - 50.)
    })
}

/// Slide the whole field back along the straight when its leader nears the end of it.
fn rebase(s: &mut Session) {
    let (id, at, len) = stretch();
    if s.course != id { return; }
    let lead = s.cars.values().map(|c| c.d).fold(f32::MIN, f32::max);
    if lead > at + len - 60. && lead < at + len + 40. {
        let back = len - 200.;
        for c in s.cars.values_mut() { c.d -= back; }
    }
}

/// One tick, checked valid, and the field kept on the straight.
fn step(s: &mut Session, drives: &BTreeMap<u32, Drive>) {
    s.step_drive(drives);
    assert!(s.valid(), "invalid at tick {}", s.ticks);
    rebase(s);
}

/// A race on that straight with the lights out and the given pods placed: (seat, pod, metres along
/// the straight, lane, speed). Nobody is a CPU rival unless a test says so.
fn race(pods: &[(u32, u8, f32, f32, f32)]) -> Session {
    let (id, at, len) = stretch();
    let mut s = Session::blank_on(id);
    s.join_ids(pods.iter().map(|p| p.0));
    s.ticks = COUNTDOWN + 200;
    for &(pid, pod, d, y, v) in pods {
        assert!(d < len - 40., "{d} is past the {len} m straight");
        let c = s.cars.get_mut(&pid).unwrap();
        c.pod = pod; c.d = at + d; c.y = y; c.v = v; c.vy = 0.;
    }
    s
}

const WARDEN: u8 = 0;
const BELLFOUNDER: u8 = 1;
const LANCEWAIN: u8 = 3;
const GATEHOUSE: u8 = 4;
const WYVERN: u8 = 5;

fn run(s: &mut Session, ticks: u32, d: impl Fn(&Session) -> BTreeMap<u32, Drive>) {
    for _ in 0..ticks { let drives = d(s); step(s, &drives); }
}
fn hold(drive: Drive) -> impl Fn(&Session) -> BTreeMap<u32, Drive> { move |s| s.cars.keys().map(|&id| (id, drive)).collect() }
/// The same, with every pod steering to stay in the lane it started in (a long run on a straight that is not quite straight).
fn keep(s: &Session, drive: Drive) -> impl Fn(&Session) -> BTreeMap<u32, Drive> + use<> {
    let lanes: BTreeMap<u32, f32> = s.cars.iter().map(|(id, c)| (*id, c.y)).collect();
    move |s| s.cars.iter().map(|(id, c)| {
        let to = lanes[id];
        (*id, Drive { steer: if c.y > to + 0.5 { -1 } else if c.y < to - 0.5 { 1 } else { 0 }, ..drive })
    }).collect()
}
/// Metres a pod covers in `ticks` under `drive`, kept in its lane: the speed summed, so a slide back along the straight does not count.
fn travelled(s: &mut Session, id: u32, ticks: u32, drive: Drive) -> f32 {
    let hands = keep(s, drive);
    let mut metres = 0.;
    for _ in 0..ticks { let d = hands(s); step(s, &d); metres += s.cars[&id].v / super::race::HZ; }
    metres
}

// ---- Heat -------------------------------------------------------------------------------------

#[test]
fn gas_alone_heats_both_engines_slowly_and_lifting_cools_them() {
    let mut s = race(&[(1, WARDEN, 0., 0., 40.)]);
    let hands = keep(&s, GAS);
    run(&mut s, 30 * 30, hands);
    let [l, r] = s.cars[&1].heat;
    assert!(l > 0.2 && l < 0.5 && (l - r).abs() < 0.02, "half a minute of gas: {l} {r}");
    s.cars.get_mut(&1).unwrap().heat = [0.6, 0.6];
    run(&mut s, 45, hold(COAST));
    assert!(s.cars[&1].heat[0] < 0.6 - 0.2 * 1.4 + 0.05, "a second and a half off the gas cools: {:?}", s.cars[&1].heat);
}

#[test]
fn a_boost_heats_the_engines_several_times_faster_than_gas() {
    let mut gas = race(&[(1, WARDEN, 0., 0., 40.)]);
    let mut boost = race(&[(1, WARDEN, 0., 0., 40.)]);
    run(&mut gas, 90, hold(GAS));
    run(&mut boost, 90, hold(BOOST));
    let (g, b) = (gas.cars[&1].hottest(), boost.cars[&1].hottest());
    assert!(b > g * 4., "three seconds of boost {b} against gas {g}");
    assert!(b > 0.2 && b < 0.45, "a full boost bar is a fair share of the engine's margin: {b}");
}

#[test]
fn braking_cools_less_than_coasting_and_both_cool() {
    let mut a = race(&[(1, WARDEN, 0., 0., 40.)]);
    let mut b = race(&[(1, WARDEN, 0., 0., 40.)]);
    for s in [&mut a, &mut b] { s.cars.get_mut(&1).unwrap().heat = [0.6, 0.6]; }
    run(&mut a, 30, hold(COAST));
    run(&mut b, 30, hold(Drive { brake: true, ..COAST }));
    let (ca, cb) = (a.cars[&1].hottest(), b.cars[&1].hottest());
    assert!(ca < cb && cb < 0.6, "coast {ca}, brake {cb}");
}

#[test]
fn lifting_off_the_gas_patches_wear_and_a_pod_on_the_gas_does_not() {
    let mut s = race(&[(1, WARDEN, 0., 0., 40.)]);
    s.cars.get_mut(&1).unwrap().wear = [0.3, 0.1];
    s.cars.get_mut(&1).unwrap().heat = [0.3, 0.1];
    run(&mut s, 60, hold(GAS));
    assert!(s.cars[&1].wear[0] >= 0.3 - 1e-4, "gas patches nothing: {:?}", s.cars[&1].wear);
    run(&mut s, 150, hold(COAST));
    assert!(s.cars[&1].wear[0] < 0.3 - 0.1 && s.cars[&1].wear[1] < 0.02, "five seconds of lift patches: {:?}", s.cars[&1].wear);
}

#[test]
fn an_engine_cannot_cool_below_its_wear() {
    let mut s = race(&[(1, WARDEN, 0., 0., 40.)]);
    s.cars.get_mut(&1).unwrap().wear = [0.85, 0.0];
    s.cars.get_mut(&1).unwrap().heat = [0.9, 0.9];
    run(&mut s, 60, hold(COAST));
    let c = &s.cars[&1];
    assert!(c.heat[0] >= c.wear[0] - 1e-4 && c.heat[0] > c.heat[1] + 0.15, "{:?} over wear {:?}", c.heat, c.wear);
}

#[test]
fn pods_run_hot_or_cool_by_their_own_numbers() {
    let heat_of = |pod: u8| { let mut s = race(&[(1, pod, 0., 0., 40.)]); run(&mut s, 120, hold(BOOST)); s.cars[&1].hottest() };
    assert!(heat_of(BELLFOUNDER) > heat_of(WARDEN) && heat_of(WARDEN) > heat_of(GATEHOUSE), "bells run hot, the keep runs cool");
}

// ---- The limp -----------------------------------------------------------------------------------

#[test]
fn a_burning_engine_gives_less_push_and_a_lower_top_speed() {
    let speed_after = |heat: [f32; 2], ticks: u32| {
        let mut s = race(&[(1, WARDEN, 0., 0., 30.)]);
        s.cars.get_mut(&1).unwrap().heat = heat;
        s.cars.get_mut(&1).unwrap().wear = [0.; 2];
        for _ in 0..ticks {
            // Hold the engines at that heat so the test is of the limp alone, and the pod in its lane.
            s.cars.get_mut(&1).unwrap().heat = heat;
            let steer = if s.cars[&1].y > 0.5 { -1 } else if s.cars[&1].y < -0.5 { 1 } else { 0 };
            step(&mut s, &[(1, Drive { steer, gas: true, ..Drive::default() })].into());
        }
        s.cars[&1].v
    };
    let cool = speed_after([0.2, 0.2], 300);
    let one_burning = speed_after([0.9, 0.2], 300);
    let both_burning = speed_after([0.95, 0.95], 300);
    assert!(cool > 44.5, "a cool pod reaches its top speed: {cool}");
    assert!(one_burning < cool - 3. && one_burning > both_burning, "one engine afire limps, two limp harder: {cool} {one_burning} {both_burning}");
    assert!(both_burning < cool * 0.8, "{both_burning}");
    // Push too: from the same speed the cool pod gains more in the first second.
    let (a, b) = (speed_after([0.2, 0.2], 30), speed_after([0.95, 0.95], 30));
    assert!(a > b + 1., "{a} {b}");
}

#[test]
fn a_burning_engine_drags_the_pod_toward_its_own_side() {
    let drift = |heat: [f32; 2]| {
        let mut s = race(&[(1, WARDEN, 0., 0., 40.)]);
        for _ in 0..45 {
            s.cars.get_mut(&1).unwrap().heat = heat;
            step(&mut s, &[(1, Drive { gas: true, ..Drive::default() })].into());
        }
        s.cars[&1].y
    };
    let (even, left_weak, right_weak) = (drift([0.3, 0.3]), drift([0.95, 0.3]), drift([0.3, 0.95]));
    // Engine 0 is the pod's lesser-y side: a burning left engine drags the pod toward lesser y.
    assert!(left_weak < even - 1.5 && right_weak > even + 1.5, "the pull is toward the weak engine: {left_weak} {even} {right_weak}");
    // Steering can hold it: a limp is a handling problem, not a wreck.
    let mut s = race(&[(1, WARDEN, 0., 0., 40.)]);
    for _ in 0..90 {
        s.cars.get_mut(&1).unwrap().heat = [0.95, 0.3];
        let steer = if s.cars[&1].y > 0.5 { -1 } else if s.cars[&1].y < -0.5 { 1 } else { 0 };
        step(&mut s, &[(1, Drive { gas: true, steer, ..Drive::default() })].into());
    }
    assert!(s.cars[&1].y.abs() < 1.5, "a pilot holds a limping pod on its line: {}", s.cars[&1].y);
}

#[test]
fn the_pull_is_zero_off_the_gas() {
    let off_gas = |heat: [f32; 2]| {
        let mut s = race(&[(1, WARDEN, 0., 0., 40.)]);
        for _ in 0..30 { s.cars.get_mut(&1).unwrap().heat = heat; step(&mut s, &[(1, COAST)].into()); }
        s.cars[&1].y
    };
    assert!((off_gas([0.95, 0.3]) - off_gas([0.3, 0.3])).abs() < 0.05, "a limp pulls only under power");
}

// ---- Damage: walls, slag, jets -----------------------------------------------------------------

#[test]
fn a_wall_strike_dumps_heat_into_the_engine_beside_it_only() {
    // Engine 0 is the lesser-y side of the pod (the left of the picture), engine 1 the greater.
    for (side, near, far) in [(1., 1usize, 0usize), (-1., 0, 1)] {
        let mut s = race(&[(1, WARDEN, 0., 0., 40.)]);
        let (id, at, _) = stretch();
        let edge = { let f = course(id).frame(0, at); if side > 0. { f.wl } else { f.wr } };
        { let c = s.cars.get_mut(&1).unwrap(); c.y = side * (edge - 0.1); c.vy = side * 13.; }
        step(&mut s, &[(1, Drive { gas: true, steer: side as i8, ..Drive::default() })].into());
        let c = &s.cars[&1];
        assert_eq!(c.impact, 12);
        assert!(c.heat[near] > 0.04 && c.heat[far] < 0.01, "side {side}: {:?}", c.heat);
        assert!(c.wear[near] > 0., "a hard strike scars the engine: {:?}", c.wear);
    }
}

#[test]
fn a_light_pod_takes_more_of_a_wall_than_a_heavy_one() {
    let after = |pod: u8| {
        let mut s = race(&[(1, pod, 0., 0., 40.)]);
        let (id, at, _) = stretch();
        let edge = course(id).frame(0, at).wl;
        { let c = s.cars.get_mut(&1).unwrap(); c.y = edge - 0.1; c.vy = 13.; }
        step(&mut s, &[(1, Drive { gas: true, steer: 1, ..Drive::default() })].into());
        s.cars[&1].heat[1]
    };
    assert!(after(LANCEWAIN) > after(WARDEN) && after(WARDEN) > after(GATEHOUSE), "{} {} {}", after(LANCEWAIN), after(WARDEN), after(GATEHOUSE));
}

#[test]
fn grinding_along_a_wall_keeps_heating_the_near_engine() {
    let mut s = race(&[(1, WARDEN, 0., 0., 40.)]);
    let (id, at, _) = stretch();
    let edge = course(id).frame(0, at).wl;
    s.cars.get_mut(&1).unwrap().y = edge - 0.2;
    run(&mut s, 60, |s| [(1, Drive { gas: true, steer: if s.cars[&1].y > 0. { 1 } else { 0 }, ..Drive::default() })].into());
    let c = &s.cars[&1];
    assert!(c.heat[1] > 0.12 && c.heat[0] < 0.05, "two seconds riding the greater-y wall: {:?}", c.heat);
}

#[test]
fn a_slag_block_and_a_flame_jet_dump_heat() {
    let c = course(CourseId::Emberfall);
    let slag = c.hazards().iter().find(|h| h.kind == HazardKind::Slag && h.y.abs() > 2.).unwrap();
    let mut s = Session::blank_on(CourseId::Emberfall);
    s.join_ids([1]);
    s.ticks = COUNTDOWN + 5;
    { let car = s.cars.get_mut(&1).unwrap(); car.d = slag.s - 4.; car.y = slag.y; car.v = 40.; }
    for _ in 0..6 { step(&mut s, &[(1, GAS)].into()); }
    let car = &s.cars[&1];
    let near = usize::from(slag.y < 0.);
    // The block was dead ahead of the pod's centre, so it is the side the pod got shoved off toward that cooks.
    assert!(car.heat[0] + car.heat[1] > 0.12 && car.wear[0] + car.wear[1] > 0., "slag cooks the engines: {:?} {:?} (near {near})", car.heat, car.wear);
    // A jet licks both.
    if let Some(jet) = c.hazards().iter().find(|h| h.kind == HazardKind::Jet) {
        let tick = (0..jet.period.max(1)).find(|t| jet.live(*t)).unwrap_or(0);
        let mut s = Session::blank_on(CourseId::Emberfall);
        s.join_ids([1]);
        s.ticks = COUNTDOWN + tick;
        { let car = s.cars.get_mut(&1).unwrap(); car.d = jet.s - 4.; car.y = jet.y; car.v = 40.; }
        for _ in 0..4 { step(&mut s, &[(1, GAS)].into()); }
        let car = &s.cars[&1];
        assert!(car.heat[0] > 0.1 && car.heat[1] > 0.1, "a jet heats both engines: {:?}", car.heat);
    }
}

// ---- The blowout and the squires' patch ----------------------------------------------------------

#[test]
fn an_engine_at_full_heat_blows_and_the_pod_sits_out_then_goes_again() {
    let mut s = race(&[(1, WARDEN, 0., 0., 44.)]);
    s.cars.get_mut(&1).unwrap().heat = [0.2, 0.999];
    let mut blown_at = None;
    let mut sat = 0;
    let mut speeds = Vec::new();
    for t in 0..400 {
        step(&mut s, &[(1, if t < 20 { BOOST } else { GAS })].into());
        let c = &s.cars[&1];
        if c.blown() && blown_at.is_none() { blown_at = Some(t); }
        if blown_at.is_some() { speeds.push(c.v); }
        if c.blown() { sat += 1; }
    }
    let blown_at = blown_at.expect("the engine blew");
    assert!(blown_at < 12, "a boost on a nearly blown engine blows it at once: {blown_at}");
    assert_eq!(s.cars[&1].blowouts, 1);
    // It skids, then stands still while the squires work.
    assert!(speeds[2] < 44. && speeds[2] > 20., "it skids on its momentum first: {:?}", &speeds[..4]);
    let still = speeds.iter().filter(|v| **v == 0.).count();
    assert!(still > 55 && sat >= engines::BLOWOUT_TICKS - 2, "it stands still while the squires patch it: still {still}, sat {sat}");
    // It goes again on patched, cooler engines, and is racing at speed well before the end.
    let c = &s.cars[&1];
    assert!(!c.blown() && c.v > 30. && c.heat[1] < 0.9, "back in the race: v {} heat {:?}", c.v, c.heat);
    assert!(c.blown_side == 0);
}

#[test]
fn a_blowout_costs_a_few_seconds_not_the_race() {
    let go = |heat: [f32; 2]| {
        let mut s = race(&[(1, WARDEN, 0., 0., 44.)]);
        s.cars.get_mut(&1).unwrap().heat = heat;
        travelled(&mut s, 1, 30 * 12, GAS)
    };
    let (clean, blown) = (go([0.1, 0.1]), go([0.1, 1.0]));
    let lost = (clean - blown) / 46.;
    assert!(lost > 4. && lost < 9., "a blowout costs {lost:.1} s of road");
}

#[test]
fn a_blown_pod_presses_nothing_and_makes_no_wake() {
    let mut s = race(&[(1, WARDEN, 0., 0., 40.), (2, WYVERN, -14., 0., 40.)]);
    s.cars.get_mut(&1).unwrap().heat = [0.2, 1.0];
    for _ in 0..4 { step(&mut s, &[(1, BOOST), (2, GAS)].into()); }
    assert!(s.cars[&1].blown());
    let (w, _) = contact::wake(s.course().lap(), &s.cars[&2], &s.cars[&1]);
    assert_eq!(w, 0., "a blown pod throws no wake");
    // Whatever its pilot asks, it sits there.
    let d = s.cars[&1].d;
    for _ in 0..70 { step(&mut s, &[(1, BOOST)].into()); }
    assert!(s.cars[&1].d - d < 40., "a sitting pod stays: {}", s.cars[&1].d - d);
}

#[test]
fn a_pilot_lifts_to_cool_an_engine_that_is_getting_too_hot_and_never_boosts_a_burning_one() {
    let mut s = race(&[(1, WARDEN, 0., 0., 44.)]);
    s.cars.get_mut(&1).unwrap().cpu = true;
    let mut lifts = 0;
    for hot in [0.1f32, 0.6, 0.7, 0.8] {
        s.cars.get_mut(&1).unwrap().heat = [hot, hot];
        let n = (0..40).filter(|t| { s.ticks = COUNTDOWN + 200 + t; !s.bot_drives()[&1].gas }).count();
        if hot == 0.1 { assert_eq!(n, 0, "a cool pilot keeps its foot down"); }
        if hot == 0.7 { assert!(n > 5, "a hot pilot feathers the gas: {n}"); }
        if hot == 0.8 { assert_eq!(n, 40, "a burning engine means a lift"); assert!((0..40).all(|t| { s.ticks = COUNTDOWN + 200 + t; !s.bot_drives()[&1].boost })); }
        lifts += n;
    }
    assert!(lifts > 40);
}

// ---- The draft ----------------------------------------------------------------------------------

fn wake_between(lead: &Car, me: &Car) -> f32 { contact::wake(course(stretch().0).lap(), me, lead).0 }

#[test]
fn the_wake_is_a_cone_behind_a_moving_pod() {
    let s = race(&[(1, WARDEN, 100., 0., 44.), (2, WARDEN, 80., 0., 44.)]);
    let (lead, mut me) = (s.cars[&1].clone(), s.cars[&2].clone());
    assert!(wake_between(&lead, &me) > 0.6, "twenty metres behind the nose, in line: {}", wake_between(&lead, &me));
    me.d = lead.d - 14.;
    let near = wake_between(&lead, &me);
    me.d = lead.d - 30.;
    let mid = wake_between(&lead, &me);
    me.d = lead.d - 60.;
    assert_eq!(wake_between(&lead, &me), 0., "sixty metres back is out of the wake");
    me.d = lead.d - 14.; me.y = 7.;
    assert_eq!(wake_between(&lead, &me), 0., "a lane away is out of it");
    me.y = 1.2;
    assert!(wake_between(&lead, &me) > 0. && wake_between(&lead, &me) < near, "the edge of the cone is weaker");
    assert!(near > mid && mid > 0., "the wake fades with distance: {near} {mid}");
    // Ahead of the leader is clean air; a stopped pod throws no wake.
    me.y = 0.; me.d = lead.d + 20.;
    assert_eq!(wake_between(&lead, &me), 0.);
    me.d = lead.d - 14.;
    let mut slow = lead.clone(); slow.v = 8.;
    assert_eq!(wake_between(&slow, &me), 0.);
}

#[test]
fn a_pod_in_a_wake_builds_a_draft_and_runs_above_its_top_speed() {
    let mut s = race(&[(1, WARDEN, 100., 0., 46.), (2, WARDEN, 80., 0., 46.)]);
    let mut seen = 0f32;
    let hands = keep(&s, GAS);
    for _ in 0..90 {
        let d = hands(&s);
        step(&mut s, &d);
        seen = seen.max(s.cars[&2].draft);
    }
    let (lead, me) = (&s.cars[&1], &s.cars[&2]);
    assert!(seen > 0.85 && me.wake == 1, "the draft deepens in the wake: {seen}");
    assert_eq!(lead.draft, 0., "the pod in front sits in clean air");
    assert!(me.v > lead.v + 2., "the trailing pod runs faster than the leader: {} vs {}", me.v, lead.v);
    assert!(me.v > 46. + 2.5, "above the pod's own top speed: {}", me.v);
}

#[test]
fn the_draft_lingers_after_the_pod_leaves_the_wake_the_slingshot() {
    let mut s = race(&[(1, WARDEN, 100., 0., 46.), (2, WARDEN, 80., 0., 46.)]);
    let hands = keep(&s, GAS);
    let mut at = 0f32;
    for _ in 0..90 {
        let d = hands(&s);
        step(&mut s, &d);
        at = at.max(s.cars[&2].draft);
        if at > 0.9 { break; }
    }
    assert!(at > 0.8, "{at}");
    // Pull out to the next lane and keep going.
    for t in 0..60 {
        let steer = if s.cars[&2].y < 5. { 1 } else { 0 };
        step(&mut s, &[(1, GAS), (2, Drive { steer, gas: true, ..Drive::default() })].into());
        if t == 10 { assert!(s.cars[&2].draft > 0.45 && s.cars[&2].draft < at, "still pulling after a third of a second: {}", s.cars[&2].draft); }
    }
    assert!(s.cars[&2].draft < 0.2 && s.cars[&2].wake == 0, "it fades in a couple of seconds: {}", s.cars[&2].draft);
}

#[test]
fn drafting_runs_the_engines_cooler() {
    let mut s = race(&[(1, WARDEN, 100., 0., 46.), (2, WARDEN, 80., 0., 46.), (3, WARDEN, 160., 5., 46.)]);
    let hands = keep(&s, GAS);
    run(&mut s, 30 * 20, hands);
    let (lead, tow, alone) = (s.cars[&1].hottest(), s.cars[&2].hottest(), s.cars[&3].hottest());
    assert!(tow < alone && (lead - alone).abs() < 0.03, "in the wake {tow}, in clean air {lead} and {alone}");
}

// ---- Contact ------------------------------------------------------------------------------------

/// Two pods side by side, B in the lane to A's left, B steering across onto A.
fn slam_pair(a_pod: u8, b_pod: u8) -> Session {
    let mut s = race(&[(1, a_pod, 100., 0., 44.), (2, b_pod, 100., 4.4, 44.)]);
    for _ in 0..30 { step(&mut s, &[(1, GAS), (2, Drive { steer: -1, gas: true, ..Drive::default() })].into()); if s.cars[&1].slams > 0 { break; } }
    s
}

#[test]
fn a_side_slam_throws_the_light_pod_further_than_the_heavy_one() {
    let mut s = race(&[(1, LANCEWAIN, 100., 0., 44.), (2, GATEHOUSE, 100., 4.6, 44.)]);
    for _ in 0..30 {
        step(&mut s, &[(1, GAS), (2, Drive { steer: -1, gas: true, ..Drive::default() })].into());
        if s.cars[&1].slams > 0 { break; }
    }
    let (light, heavy) = (&s.cars[&1], &s.cars[&2]);
    assert!(light.slams >= 1 && heavy.slams >= 1, "it was a hard hit: {} {}", light.slams, heavy.slams);
    assert!(light.kick.abs() > heavy.kick.abs() * 1.5, "the lancewain is thrown further: {} against {}", light.kick, heavy.kick);
    assert!(light.v < heavy.v, "and loses more speed: {} {}", light.v, heavy.v);
}

#[test]
fn a_hard_slam_hurts_the_near_engines_and_marks_the_side() {
    let s = slam_pair(WARDEN, WARDEN);
    let (a, b) = (&s.cars[&1], &s.cars[&2]);
    // B is on A's greater-y side: A's engine 1 and B's engine 0 are the ones that met.
    assert!(a.heat[1] > 0.04 && a.heat[0] < 0.02, "{:?}", a.heat);
    assert!(b.heat[0] > 0.04 && b.heat[1] < 0.02, "{:?}", b.heat);
    assert!(a.wear[1] > 0. && b.wear[0] > 0.);
    assert!(a.slam > 0 && a.slam_side == 1 && b.slam_side == -1, "the HUD is told which side: {} {}", a.slam_side, b.slam_side);
    assert!(a.impact >= 10 && b.impact >= 10, "and the camera shakes");
}

#[test]
fn a_rub_is_not_a_slam_no_heat_no_shake() {
    let mut s = race(&[(1, WARDEN, 100., 0., 44.), (2, WARDEN, 100., 3.4, 44.)]);
    for _ in 0..5 { step(&mut s, &[(1, GAS), (2, GAS)].into()); }
    let (a, b) = (&s.cars[&1], &s.cars[&2]);
    assert_eq!((a.slams, b.slams), (0, 0));
    assert!(a.heat[1] < 0.01 && b.heat[0] < 0.01 && a.impact == 0, "rubbing is racing: {:?}", a.heat);
    assert!((b.y - a.y) >= contact::bodies(a)[0].half_w + contact::bodies(b)[0].half_w - 0.05, "but they are held apart: {} {}", a.y, b.y);
}

#[test]
fn a_slam_throws_a_pod_on_a_kick_the_steering_cannot_cancel() {
    let mut s = slam_pair(WARDEN, WARDEN);
    let y0 = s.cars[&1].y;
    assert!(s.cars[&1].kick < -1., "A is thrown away from B: {}", s.cars[&1].kick);
    // Steer straight back into it: the kick still carries the pod off for a moment.
    for _ in 0..8 { step(&mut s, &[(1, Drive { steer: 1, gas: true, ..Drive::default() }), (2, GAS)].into()); }
    assert!(s.cars[&1].y < y0 + 1.5);
    run(&mut s, 60, |_| [(1, GAS), (2, GAS)].into());
    assert!(s.cars[&1].kick.abs() < 0.05, "the kick dies away: {}", s.cars[&1].kick);
}

#[test]
fn a_nose_into_a_tail_trades_speed_and_the_leaders_engines_feel_it() {
    let mut s = race(&[(1, WARDEN, 100., 0., 30.), (2, WARDEN, 87., 0., 46.)]);
    let before = (s.cars[&1].v, s.cars[&2].v);
    for _ in 0..20 { step(&mut s, &[(1, GAS), (2, GAS)].into()); if s.cars[&2].slams > 0 { break; } }
    let (lead, rear) = (&s.cars[&1], &s.cars[&2]);
    assert!(rear.slams >= 1 && lead.slams >= 1, "{} {}", rear.slams, lead.slams);
    assert!(rear.v < before.1 - 3. && lead.v > before.0, "the rear pod gives up speed, the leader gains: {} {}", rear.v, lead.v);
    assert!(lead.heat[0] > 0.02 && lead.heat[1] > 0.02, "the tow shocks both engines: {:?}", lead.heat);
    assert!(lead.slam_side == 0 && rear.slam_side == 0);
    assert!(rear.d < lead.d, "nobody went through anybody");
}

#[test]
fn pods_never_tunnel_through_each_other_at_any_closing_speed() {
    // A blown pod stands in the road; another comes down on it as fast as the sim lets a pod go.
    for pod in 0..6u8 {
        for v in [20., 44., 60., 72.] {
            let mut s = race(&[(1, WARDEN, 170., 0., 0.), (2, pod, 90., 0., v)]);
            s.cars.get_mut(&1).unwrap().crash = engines::BLOWOUT_TICKS;
            s.cars.get_mut(&1).unwrap().blown_side = 1;
            for _ in 0..90 {
                step(&mut s, &[(1, COAST), (2, Drive { gas: true, boost: true, ..Drive::default() })].into());
                assert!(s.cars[&2].d < s.cars[&1].d, "pod {pod} at {v} m/s went through: {} {}", s.cars[&2].d, s.cars[&1].d);
            }
        }
    }
    // And the boxes are longer than the fastest closing speed in a tick.
    let fastest = 72. / super::race::HZ;
    for p in super::pods() { assert!(p.len + 0.6 > fastest * 0.8, "{} is too short to catch at 72 m/s", p.name); }
}

#[test]
fn a_pair_that_swapped_places_in_one_step_is_put_back() {
    let mut s = race(&[(1, WARDEN, 100., 0., 0.), (2, WARDEN, 80., 0., 0.)]);
    let lap = s.course().lap();
    let before: BTreeMap<u32, (f32, f32)> = s.cars.iter().map(|(i, c)| (*i, (c.d, c.y))).collect();
    // Teleport the rear pod clean through the front one, as a bug or a fall might.
    s.cars.get_mut(&2).unwrap().d += 40.;
    contact::resolve(lap, &mut s.cars, &before);
    assert!(s.cars[&2].d < s.cars[&1].d + 0.01, "put back behind: {} {}", s.cars[&2].d, s.cars[&1].d);
}

#[test]
fn a_shove_never_pushes_a_pod_through_the_wall() {
    let mut s = race(&[(1, WARDEN, 100., 0., 44.), (2, GATEHOUSE, 100., 4.0, 44.)]);
    let (id, at, _) = stretch();
    let edge_at = |d: f32| course(id).frame(0, d).wl;
    s.cars.get_mut(&1).unwrap().y = edge_at(at + 100.) - 2.;
    s.cars.get_mut(&2).unwrap().y = edge_at(at + 100.) - 6.;
    for _ in 0..30 {
        step(&mut s, &[(1, GAS), (2, Drive { steer: 1, gas: true, ..Drive::default() })].into());
        assert!(s.cars[&1].y <= edge_at(s.cars[&1].d) + 1e-3, "pod 1 at {} past the wall at {}", s.cars[&1].y, edge_at(s.cars[&1].d));
    }
}

// ---- The pilots ---------------------------------------------------------------------------------

#[test]
fn each_pod_has_a_pilot_with_a_character() {
    let (lance, gate, bell, ring) = (pilot::persona(LANCEWAIN), pilot::persona(GATEHOUSE), pilot::persona(BELLFOUNDER), pilot::persona(2));
    assert!(lance.brave > 0.9 && lance.brave > gate.brave && lance.steady < gate.steady, "Lancewain is brave and brittle");
    assert!(gate.block > 0.9 && gate.block > lance.block * 5., "Gatehouse is a blocker");
    assert!(bell.boost > 0.9 && ring.steady > 0.9 && ring.caution > bell.caution, "bells burn the boost, hoops drive smooth");
}

#[test]
fn a_pilot_tucks_into_a_wake_pulls_out_and_passes_through_the_draft() {
    // A wyvern comes up behind a slower pod (a limping gatehouse) that holds its lane: tuck, draft, pull out, boost by.
    let mut s = race(&[(1, GATEHOUSE, 140., 0., 44.), (2, WYVERN, 100., 0., 44.)]);
    s.cars.get_mut(&2).unwrap().cpu = true;
    let mut peak = 0f32;
    let mut passed_at = None;
    for t in 0..30 * 20 {
        s.cars.get_mut(&1).unwrap().heat = [0.8, 0.8];
        let mut d = s.bot_drives();
        d.insert(1, Drive { gas: true, steer: if s.cars[&1].y > 0.5 { -1 } else if s.cars[&1].y < -0.5 { 1 } else { 0 }, ..Drive::default() });
        step(&mut s, &d);
        assert!(s.valid());
        peak = peak.max(s.cars[&2].draft);
        if passed_at.is_none() && s.cars[&2].d > s.cars[&1].d + 4. { passed_at = Some(t); }
    }
    assert!(peak > 0.45, "the pilot found the wake: {peak}");
    let t = passed_at.expect("the pilot passed");
    assert!(t < 30 * 16, "and did it in good time: {t} ticks");
    assert!(s.cars[&2].passes >= 1, "and it counts as a pass");
}

#[test]
fn a_blocker_covers_its_line_and_a_quick_pod_does_not() {
    // Two pods in the leader's seat with a chaser a lane over: the blocker shifts toward the chaser.
    let shift = |lead_pod: u8| {
        let mut s = race(&[(1, lead_pod, 118., -1., 44.), (2, WARDEN, 100., 2., 44.)]);
        for c in s.cars.values_mut() { c.cpu = true; }
        let y0 = s.cars[&1].y;
        // The chaser holds its lane: only the leader's decisions are in play.
        for _ in 0..45 {
            let mut d = s.bot_drives();
            d.insert(2, Drive { gas: true, steer: if s.cars[&2].y > 2.5 { -1 } else if s.cars[&2].y < 1.5 { 1 } else { 0 }, ..Drive::default() });
            step(&mut s, &d);
        }
        s.cars[&1].y - y0
    };
    let (blocker, quick) = (shift(GATEHOUSE), shift(LANCEWAIN));
    assert!(blocker > 0.8, "the gatehouse moves across to the chaser's line: {blocker}");
    assert!(blocker > quick + 0.5, "Lancewain does not: {blocker} vs {quick}");
}

#[test]
fn a_pilot_steers_round_a_pod_that_has_stopped_in_its_lane() {
    let mut s = race(&[(1, WARDEN, 200., 0., 0.), (2, WYVERN, 40., 0., 44.)]);
    { let c = s.cars.get_mut(&1).unwrap(); c.crash = engines::BLOWOUT_TICKS; c.blown_side = 1; }
    s.cars.get_mut(&2).unwrap().cpu = true;
    for _ in 0..30 * 8 {
        let mut d = s.bot_drives();
        d.insert(1, COAST);
        step(&mut s, &d);
        assert_eq!(s.cars[&2].slams, 0, "the pilot hit a stopped pod at tick {}", s.ticks);
        if s.cars[&2].d > s.cars[&1].d + 10. { return; }
    }
    panic!("the pilot never got by: {} vs {}", s.cars[&2].d, s.cars[&1].d);
}

#[test]
fn pilots_slip_now_and_then_and_a_clean_driver_never_does() {
    let course = course(CourseId::Emberfall);
    for pod in 0..6u8 {
        let count = |slop: f32| (0..9000u32).filter(|w| pilot::slipping(course, 2, pod, COUNTDOWN + w * 40, slop)).count();
        let (cpu, clean) = (count(1.), count(0.));
        assert_eq!(clean, 0, "pod {pod}");
        assert!(cpu >= 40 && cpu <= 400, "pod {pod} slips {cpu} windows in 9000");
    }
    // The lancewain's pilot slips more often than the ringwright's.
    let rate = |pod: u8| (0..9000u32).filter(|w| pilot::slipping(course, 2, pod, COUNTDOWN + w * 40, 1.)).count();
    assert!(rate(LANCEWAIN) > rate(2));
}

#[test]
fn the_rubber_band_is_mild_and_capped() {
    // A human far ahead of a rival: the rival is let run a few percent faster, never more.
    let mut s = race(&[(1, WARDEN, 0., 0., 44.), (2, WARDEN, 0., 4., 44.), (3, WARDEN, 0., -4., 44.)]);
    s.cars.get_mut(&1).unwrap().d += 900.;
    s.cars.get_mut(&3).unwrap().d += 1500.;
    s.cars.get_mut(&2).unwrap().cpu = true;
    s.cars.get_mut(&3).unwrap().cpu = true;
    let paces = s.paces();
    let skill = super::race::CPU_SKILL;
    assert!(paces[&2] > skill && paces[&2] <= skill * (1. + super::race::RUBBER_UP) + 1e-6, "{:?}", paces);
    assert!(paces[&3] < skill && paces[&3] >= skill * (1. - super::race::RUBBER_DOWN) - 1e-6, "{:?}", paces);
    assert!(paces[&2] < 1.02 && paces[&3] > 0.94, "no rival runs more than a couple of percent over its pod's speed");
    assert!(super::race::RUBBER_UP <= 0.04 && super::race::RUBBER_DOWN <= 0.025);
    // A human has no band.
    assert!(!paces.contains_key(&1));
}

// ---- The start ----------------------------------------------------------------------------------

#[test]
fn a_rocket_start_beats_a_plain_start_and_a_flood_loses_ground() {
    let start = |hold_from: u32| {
        let mut s = Session::blank_on(CourseId::Emberfall);
        s.join_ids([1]);
        for t in 1..=COUNTDOWN + 150 {
            let gas = t >= hold_from;
            step(&mut s, &[(1, Drive { gas, ..Drive::default() })].into());
        }
        (s.cars[&1].d, s.cars[&1].launch, s.cars[&1].hottest())
    };
    let (rocket, rl, _) = start(COUNTDOWN - 6);
    let (plain, pl, _) = start(COUNTDOWN + 1);
    let (flood, fl, fh) = start(COUNTDOWN - 40);
    assert_eq!((rl, pl, fl), (1, 0, -1), "the flag judges the start");
    assert!(rocket > plain + 6. && plain > flood + 6., "rocket {rocket} plain {plain} flooded {flood}");
    assert!(fh > 0.05, "a flood puts heat in: {fh}");
}

#[test]
fn the_pilots_leave_the_line_together_and_the_jumpy_ones_now_and_then_flood() {
    let mut s = Session::blank_on(CourseId::Emberfall);
    s.join_ids(1..=6);
    for c in s.cars.values_mut() { c.cpu = true; }
    for _ in 0..COUNTDOWN + 60 { let d = s.bot_drives(); step(&mut s, &d); }
    let rockets = s.cars.values().filter(|c| c.launch > 0).count();
    assert!(rockets >= 4, "most pilots time the lights: {rockets}");
    assert!(s.cars.values().all(|c| c.d > -60.));
}

// ---- Saves --------------------------------------------------------------------------------------

#[test]
fn a_car_saved_before_the_law_loads_with_cool_sound_engines() {
    let old = r#"{"d":10,"y":0,"v":20,"vy":0,"boost":1,"finish":null}"#;
    let car: Car = serde_json::from_str(old).unwrap();
    assert_eq!((car.heat, car.wear, car.crash, car.draft, car.slams), ([0.; 2], [0.; 2], 0, 0., 0));
    let wire = serde_json::to_string(&race(&[(1, WARDEN, 0., 0., 40.)])).unwrap();
    assert!(serde_json::from_str::<Session>(&wire).unwrap().valid());
}

/// A diagnostic, not a gate: six pilots race every cup course and the first pod that sits still
/// without being blown for four seconds is described, with its neighbours. `cargo test ... stall_hunt -- --ignored --nocapture`.
#[test]
#[ignore]
fn stall_hunt() {
    for &id in cup() {
        let mut s = Session::blank_on(id);
        s.join_ids(1..=6);
        for car in s.cars.values_mut() { car.cpu = true; }
        let c = s.course();
        let lap = c.lap();
        let mut still: BTreeMap<u32, u32> = BTreeMap::new();
        let mut trail: BTreeMap<u32, Vec<String>> = BTreeMap::new();
        for t in 0..40_000u32 {
            let d = s.bot_drives();
            s.step_drive(&d);
            if s.finished { break; }
            for (n, car) in &s.cars {
                let tr = trail.entry(*n).or_default();
                tr.push(format!("t{t} d {:.1} y {:.2} v {:.1} heat {:.3?} wear {:.3?} imp {} slams {} hands s{} g{} b{} bo{}", car.d, car.y, car.v, car.heat, car.wear, car.impact, car.slams, d[n].steer, d[n].gas as u8, d[n].brake as u8, d[n].boost as u8));
                if tr.len() > 140 { tr.remove(0); }
                let r = still.entry(*n).or_insert(0);
                if car.v < 1. && car.finish.is_none() && !car.blown() && s.ticks > COUNTDOWN + 60 { *r += 1; } else { *r = 0; }
                if *r == 120 {
                    println!("{id:?} tick {t}: seat {n} has sat still for 4 s: {car:?}");
                    let fr = c.frame(car.route, car.d);
                    println!("  road here: wl {:.2} wr {:.2} k {:.4} g {:.3}", fr.wl, fr.wr, fr.k, fr.g);
                    for h in c.hazards().iter().filter(|h| ((h.s - car.d.rem_euclid(lap)).abs()) < 25.) { println!("  hazard {:?} s {:.1} y {:.1} half {:.1}x{:.1} period {} on {}", h.kind, h.s, h.y, h.half_l, h.half_w, h.period, h.on); }
                    for (o, oc) in &s.cars { if o != n { println!("  seat {o}: lap pos {:.1} y {:.1} v {:.1} crash {} finish {:?}", oc.d.rem_euclid(lap), oc.y, oc.v, oc.crash, oc.finish); } }
                    println!("  my lap pos {:.1}", car.d.rem_euclid(lap));
                    let tr = &trail[n];
                    for i in [0usize, 30, 60, 90, 110, 125, 133, 137, 139] { if let Some(l) = tr.get(i) { println!("  {l}"); } }
                    return;
                }
            }
        }
        println!("{id:?}: no stall, finished {}", s.finished);
    }
}
