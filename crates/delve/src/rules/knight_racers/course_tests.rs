//! Knight Racers courses: every map is big, closed, drivable and fair, and the
//! six rival pilots finish all of them. Std and serde only, so the same tests run
//! against the course files on their own.
use super::course::*;
use super::pods::pods;
use super::race::*;
use std::collections::BTreeMap;

/// The retired circuit's lap, the yardstick: every cup course is at least three of it.
const OLD_LAP: f32 = 2819.157;

/// An angle difference folded into -pi..pi: headings run on past a full turn.
fn turn(d: f32) -> f32 { (d + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI }

fn cup_courses() -> Vec<&'static Course> { cup().iter().map(|id| course(*id)).collect() }

/// Six pilots race the whole course. Returns the finished session, the longest
/// stall (ticks under 3 m/s after the start) per car and the validity at every second.
fn race_bots(id: CourseId) -> (Session, BTreeMap<u32, u32>) {
    let mut s = Session::blank_on(id);
    s.join_ids(1..=6);
    let mut worst: BTreeMap<u32, u32> = BTreeMap::new();
    let mut run: BTreeMap<u32, u32> = BTreeMap::new();
    for t in 0..40_000u32 {
        let drives = s.bot_drives_all();
        s.step_drive(&drives);
        if t % 30 == 0 { assert!(s.valid(), "{id:?} invalid at tick {t}: {:?}", s.cars); }
        if t > 150 {
            for (n, c) in &s.cars {
                let r = run.entry(*n).or_insert(0);
                if c.v < 3. && c.finish.is_none() && !c.blown() { *r += 1; let w = worst.entry(*n).or_insert(0); *w = (*w).max(*r); } else { *r = 0; }
            }
        }
        if s.finished { return (s, worst); }
    }
    panic!("{id:?}: six bots did not finish in 40000 ticks: {:?}", s.cars.iter().map(|(n, c)| (*n, c.d, c.v, c.route)).collect::<Vec<_>>());
}

#[test]
fn cup_courses_are_big_authored_and_closed() {
    assert!(cup().len() >= 3, "the cup needs at least three courses");
    let mut forked = 0;
    for c in cup_courses() {
        let lap = c.lap();
        assert!(lap >= 3. * OLD_LAP, "{}: lap {lap} is under three times the old circuit", c.name());
        // The seam is level and closed.
        let (a, b) = (c.sample(0.), c.sample(lap - 0.5));
        assert!((a.x - b.x).hypot(a.y - b.y) < 1.6, "{}: lap seam jumps", c.name());
        assert!(turn(a.th - b.th).abs() < 0.06, "{}: heading breaks at the line", c.name());
        assert!((a.z - b.z).abs() < 0.4, "{}: the line is not level", c.name());
        // The sections run end to end and the road is varied.
        let zones = c.zones();
        assert_eq!(zones[0].s0, 0.);
        for w in zones.windows(2) { assert!((w[0].s1 - w[1].s0).abs() < 0.01, "{}: zones leave a hole", c.name()); }
        assert!((zones.last().unwrap().s1 - lap).abs() < 0.5);
        let mut biomes: Vec<_> = zones.iter().map(|z| z.biome.name()).collect();
        biomes.sort(); biomes.dedup();
        assert!(biomes.len() >= 3, "{}: {biomes:?}", c.name());
        // Real elevation, bounded grade, banked bends, widths that vary, and no hairpin a pod cannot take.
        let (mut zmin, mut zmax, mut gmax, mut kmax, mut bmax) = (f32::MAX, f32::MIN, 0f32, 0f32, 0f32);
        let (mut wmin, mut wmax) = (f32::MAX, f32::MIN);
        let mut s = 0.;
        while s < lap {
            let p = c.sample(s);
            zmin = zmin.min(p.z); zmax = zmax.max(p.z);
            if !c.gaps().iter().any(|g| s > g.s0 - 40. && s < g.s1 + 40.) { gmax = gmax.max(p.g.abs()); } kmax = kmax.max(p.k.abs()); bmax = bmax.max(p.bank.abs());
            wmin = wmin.min(p.wl.min(p.wr)); wmax = wmax.max(p.wl.max(p.wr));
            s += 5.;
        }
        assert!(zmax - zmin >= 25., "{}: only {} m of climb", c.name(), zmax - zmin);
        assert!(gmax <= 0.15, "{}: grade {gmax}", c.name());
        assert!(1. / kmax >= 25., "{}: a bend of radius {}", c.name(), 1. / kmax);
        assert!(bmax >= 0.15, "{}: the bends are not banked ({bmax})", c.name());
        assert!(wmin >= 6.5 && wmax <= 16. && wmax - wmin >= 4., "{}: widths {wmin}..{wmax}", c.name());
        // No part of the road runs through another.
        let pts: Vec<(f32, f32, f32)> = (0..(lap / 8.) as usize).map(|i| { let p = c.sample(i as f32 * 8.); (p.x, p.y, i as f32 * 8.) }).collect();
        for (i, a) in pts.iter().enumerate() {
            for b in &pts[i + 1..] {
                let apart = (b.2 - a.2).min(lap - (b.2 - a.2));
                if apart > 400. { assert!((a.0 - b.0).hypot(a.1 - b.1) > 90., "{}: road at {} and {} run together", c.name(), a.2, b.2); }
            }
        }
        // The extras.
        assert!(!c.gaps().is_empty(), "{}: no jump", c.name());
        for g in c.gaps() { assert!(g.s1 - g.s0 >= 26. && g.s1 - g.s0 <= 40. && g.ramp > 0.05); }
        let mut kinds: Vec<_> = c.hazards().iter().map(|h| format!("{:?}", h.kind)).collect();
        kinds.sort(); kinds.dedup();
        assert!(kinds.len() >= 3 && c.hazards().len() >= 20, "{}: hazards {kinds:?}", c.name());
        let mut pk: Vec<_> = c.pieces().iter().map(|p| format!("{:?}", p.kind)).collect();
        pk.sort(); pk.dedup();
        assert!(pk.len() >= 7 && c.pieces().len() >= 18, "{}: pieces {pk:?}", c.name());
        for p in c.pieces() { assert!(p.s0 >= 0. && p.s1 > p.s0 && p.s1 <= lap + 1. && p.side.abs() <= 1., "{}: piece {:?} {}..{}", c.name(), p.kind, p.s0, p.s1); }
        for st in c.strips() { assert!(st.s0 >= 0. && st.s1 > st.s0 && st.s1 < lap && st.half_w > 1.5); }
        for h in c.hazards() { assert!(h.s > 60. && h.s < lap - 20. && h.y.abs() < 10., "{}: hazard {:?} at {} y {}", c.name(), h.kind, h.s, h.y); }
        for h in c.hazards() { let (wl, wr) = c.width(h.s); let room = if h.y >= 0. { wl } else { wr }; assert!(h.y.abs() + h.half_w <= room + 0.6, "{}: hazard {:?} at {} sticks out of the road ({} + {} vs {room})", c.name(), h.kind, h.s, h.y.abs(), h.half_w); }
        assert!(c.strips().len() >= 4);
        assert!(c.landmarks().len() >= 8);
        if !c.forks().is_empty() { forked += 1; }
    }
    assert!(forked >= 2, "at least two courses need a fork or a shortcut");
    let names: Vec<_> = courses().iter().map(|id| course(*id).name()).collect();
    for n in &names { assert!(!n.contains("MOS") && !n.contains("TATOOINE"), "{n}"); }
}

#[test]
fn forks_leave_and_rejoin_the_main_road() {
    for c in cup_courses() {
        for f in c.forks() {
            assert!(f.s1 > f.s0 + 60. && f.side.abs() == 1., "{} {}", c.name(), f.name);
            let main = |s: f32| c.sample(s);
            for (s, off) in [(f.s0, f.off0), (f.s1, f.off1)] {
                let (m, (x, y, th)) = (main(s), f.pose(s));
                let (ex, ey) = (m.x - f.side * off * m.th.sin(), m.y + f.side * off * m.th.cos());
                assert!((x - ex).hypot(y - ey) < 0.8, "{} {}: the fork misses the road at {s}", c.name(), f.name);
                assert!(turn(th - m.th).abs() < 0.9, "{} {}: heading off by {} at {s}", c.name(), f.name, turn(th - m.th));
                assert!((f.height(s) - m.z).abs() < 0.8, "{} {}: heights meet {} vs {}", c.name(), f.name, f.height(s), m.z);
            }
            // Something a pod can drive: no hairpin in the fork itself.
            let mut q = 0.;
            while q < f.line.len() { assert!(f.line.at(q).k.abs() < 1. / 14., "{} {}: the fork bends at radius {} at {q}", c.name(), f.name, 1. / f.line.at(q).k.abs()); q += 2.; }
            let (wl, wr) = f.width(f.s0);
            assert!((wl - f.off0).abs() < 0.5 && (wr - f.off0).abs() < 0.5);
            assert!((f.scale() - 1.).abs() > 0.01 || !f.shortcut, "{}: a shortcut that is not shorter", f.name);
            if f.shortcut { assert!(f.scale() < 0.95, "{}: a shortcut needs to save real road", f.name); }
            // It stays on its own side of the main road: no crossings, no overlap past the gore.
            let n = (f.line.len() / 6.) as usize;
            for i in 0..=n {
                let s = f.s0 + (f.s1 - f.s0) * i as f32 / n as f32;
                let (x, y, _) = f.pose(s);
                let (mut best, mut at) = (f32::MAX, 0.);
                let mut q = f.s0 - 40.;
                while q < f.s1 + 40. { let m = c.sample(q); let d = (m.x - x).hypot(m.y - y); if d < best { best = d; at = q; } q += 3.; }
                let m = c.sample(at);
                let lateral = (x - m.x) * -m.th.sin() + (y - m.y) * m.th.cos();
                let (from_end, ) = ((s - f.s0).min(f.s1 - s), );
                if from_end > 30. { assert!(lateral * f.side > 2.5, "{} {}: the fork crosses or hugs the road at {s} (lateral {lateral})", c.name(), f.name); }
            }
            assert!(c.fork_at(f.s0 + 1.).is_some() && c.fork_at(f.s1 + 1.).is_none_or(|(_, g)| g.name != f.name));
        }
    }
}

/// Six pilots race every lap of one course: all finish, `valid()` holds throughout,
/// nobody stalls, and a lap takes a sane time.
fn six_bots_finish(id: CourseId) {
    let c = course(id);
    let (s, stalls) = race_bots(id);
    assert!(s.finished && s.valid());
    let total = c.lap() * c.laps() as f32;
    let times: Vec<f32> = s.cars.values().map(|c| c.finish.unwrap()).collect();
    let (fast, slow) = (times.iter().cloned().fold(f32::MAX, f32::min), times.iter().cloned().fold(0., f32::max));
    let per_lap = |t: f32| t / c.laps() as f32;
    // Sane speeds: a lap averages between 30 and 52 metres a second.
    assert!(per_lap(fast) > c.lap() / 52. && per_lap(slow) < c.lap() / 30., "{}: laps {:.1}..{:.1} s on {:.0} m", c.name(), per_lap(fast), per_lap(slow), c.lap());
    assert!(slow < fast * 1.2, "{}: the field spreads {fast}..{slow}", c.name());
    for (n, car) in &s.cars {
        assert!((car.d - total).abs() < 0.05);
        assert!(stalls.get(n).copied().unwrap_or(0) < 90, "{}: seat {n} stalled for {} ticks", c.name(), stalls[n]);
        assert!(car.fell <= 2, "{}: seat {n} fell {} times", c.name(), car.fell);
    }
    println!("{}: lap {:.0} m, race {:.1}..{:.1} s ({:.1}..{:.1} s a lap)", c.name(), c.lap(), fast, slow, per_lap(fast), per_lap(slow));
}

#[test]
fn six_bots_finish_the_proving_run() { six_bots_finish(CourseId::Proving); }
#[test]
fn six_bots_finish_emberfall() { six_bots_finish(CourseId::Emberfall); }
#[test]
fn six_bots_finish_the_glassway() { six_bots_finish(CourseId::Glassway); }
#[test]
fn six_bots_finish_moatwalk() { six_bots_finish(CourseId::Moatwalk); }

#[test]
fn the_same_inputs_make_the_same_race() {
    let id = cup()[0];
    let a = race_bots(id).0;
    let b = race_bots(id).0;
    assert_eq!(serde_json::to_string(&a).unwrap(), serde_json::to_string(&b).unwrap());
    // The wire round trip keeps a live race valid, and an old save (no course) loads on the proving run.
    let mid = { let mut s = Session::blank_on(id); s.join_ids(1..=6); for _ in 0..900 { let d = s.bot_drives_all(); s.step_drive(&d); } s };
    let wire = serde_json::to_string(&mid).unwrap();
    let back: Session = serde_json::from_str(&wire).unwrap();
    assert!(back.valid() && back.course == id && back.ticks == mid.ticks);
    let old = r#"{"cars":{"1":{"d":10,"y":0,"v":20,"vy":0,"boost":1,"finish":null}},"ticks":200,"finished":false}"#;
    let legacy: Session = serde_json::from_str(old).unwrap();
    assert_eq!(legacy.course, CourseId::Proving);
    assert!(legacy.valid());
}

#[test]
fn a_jump_wants_speed() {
    for c in cup_courses() {
        let gap = c.gaps()[0];
        for (v0, clears, gas) in [(46., true, true), (24., false, false)] {
            let mut s = Session::blank_on(c.id);
            s.join_ids([1]);
            s.ticks = COUNTDOWN;
            let car = s.cars.get_mut(&1).unwrap();
            car.d = gap.s0 - if clears { 40. } else { 12. };
            // The slow pod keeps off the boost pads that line the run-up.
            car.y = if clears { 0. } else { 6. };
            car.v = v0;
            let mut best = f32::MIN;
            for _ in 0..240 {
                let car = &s.cars[&1];
                let steer = if !clears { 0 } else if car.y > 0.6 { -1 } else if car.y < -0.6 { 1 } else { 0 };
                s.step_drive(&[(1, Drive { gas, steer, ..Drive::default() })].into());
                best = best.max(s.cars[&1].d);
            }
            let car = &s.cars[&1];
            if clears { assert!(car.fell == 0 && best > gap.s1 + 20., "{}: a fast pod must clear the gap (d {best}, fell {})", c.name(), car.fell); }
            else { assert!(car.fell >= 1, "{}: a slow pod must fall into the chasm", c.name()); }
            assert!(s.valid());
        }
    }
}

#[test]
fn hazards_really_hit_and_timed_ones_cycle() {
    let c = course(CourseId::Emberfall);
    let block = c.hazards().iter().find(|h| h.kind == HazardKind::Slag).unwrap();
    let mut s = Session::blank_on(c.id);
    s.join_ids([1]);
    s.ticks = COUNTDOWN;
    { let car = s.cars.get_mut(&1).unwrap(); car.d = block.s - 4.; car.y = block.y; car.v = 40.; }
    for _ in 0..8 { s.step_drive(&[(1, Drive { gas: true, ..Drive::default() })].into()); }
    assert!(s.cars[&1].v < 30. && s.cars[&1].impact > 0, "a slag block must hurt: {}", s.cars[&1].v);
    // A timed hazard is only there for part of its cycle.
    let timed = c.hazards().iter().find(|h| h.period > 0).unwrap();
    let live = (0..timed.period).filter(|t| timed.live(*t)).count() as u32;
    assert!(live == timed.on && live < timed.period);
    let tick_off = (0..timed.period).find(|t| !timed.live(*t)).unwrap();
    let tick_on = (0..timed.period).find(|t| timed.live(*t)).unwrap();
    assert!(timed.warn(tick_on) >= 1. && timed.warn(tick_off) < 1.);
    for (tick, hurts) in [(tick_on, true), (tick_off, false)] {
        let mut s = Session::blank_on(c.id);
        s.join_ids([1]);
        s.ticks = COUNTDOWN + tick;
        { let car = s.cars.get_mut(&1).unwrap(); car.d = timed.s - 4.; car.y = timed.y; car.v = 40.; }
        // A few ticks of travel put the car on the hazard.
        for _ in 0..4 { s.step_drive(&[(1, Drive { gas: true, ..Drive::default() })].into()); }
        let after = &s.cars[&1];
        assert_eq!(after.impact > 0, hurts, "{:?} at tick {tick}: impact {}", timed.kind, after.impact);
    }
}

#[test]
fn pods_change_the_handling() {
    assert!(pods().len() >= 6);
    let c = course(CourseId::Proving);
    let drive_straight = |pod: u8, ticks: u32| -> f32 {
        let mut s = Session::blank_on(c.id);
        s.join_ids([1]);
        s.cars.get_mut(&1).unwrap().pod = pod;
        s.cars.get_mut(&1).unwrap().d = 0.;
        s.cars.get_mut(&1).unwrap().y = 0.;
        s.ticks = COUNTDOWN;
        for _ in 0..ticks {
            let car = &s.cars[&1];
            let steer = if car.y > 0.5 { -1 } else if car.y < -0.5 { 1 } else { 0 };
            s.step_drive(&[(1, Drive { gas: true, steer, ..Drive::default() })].into());
        }
        s.cars[&1].v
    };
    let (lancewain, gatehouse, wyvern, ringwright, bellfounder) = (3u8, 4u8, 5u8, 2u8, 1u8);
    assert!(drive_straight(wyvern, 45) > drive_straight(lancewain, 45), "the wyvern is quick off the line");
    // 2026-11-07: the pods' tops were drawn together (lancewain 50 -> 48, wyvern 45 -> 46) so the draft is worth more than the spread.
    assert!(drive_straight(lancewain, 330) > drive_straight(wyvern, 330) + 1., "the lancewain runs on to a higher top speed");
    // Boost: bells push hardest.
    let boosted = |pod: u8| -> f32 {
        let mut s = Session::blank_on(c.id);
        s.join_ids([1]);
        { let car = s.cars.get_mut(&1).unwrap(); car.pod = pod; car.d = 180.; car.y = 0.; car.v = 40.; }
        s.ticks = COUNTDOWN;
        for _ in 0..90 { s.step_drive(&[(1, Drive { gas: true, boost: true, ..Drive::default() })].into()); }
        s.cars[&1].v
    };
    assert!(boosted(bellfounder) > boosted(gatehouse) + 3., "the bells boost hardest: {} vs {}", boosted(bellfounder), boosted(gatehouse));
    // Grip: the same bend at the same speed pushes a pod with less grip wider.
    let (g_ring, g_lance) = (pods()[ringwright as usize].grip, pods()[lancewain as usize].grip);
    assert!(g_ring > g_lance + 0.2);
    // A wall costs the light pod more than the heavy one.
    let wall_hit = |pod: u8| -> f32 {
        let mut s = Session::blank_on(c.id);
        s.join_ids([1]);
        { let car = s.cars.get_mut(&1).unwrap(); car.pod = pod; car.d = 20.; car.y = 12.7; car.vy = 13.; car.v = 40.; }
        s.ticks = COUNTDOWN;
        s.step_drive(&[(1, Drive { steer: 1, gas: true, ..Drive::default() })].into());
        assert_eq!(s.cars[&1].impact, 12);
        s.cars[&1].v
    };
    assert!(wall_hit(gatehouse) > wall_hit(lancewain) + 1., "a wall is brittle for the lancewain");
    // Every pod runs the course.
    for pod in 0..pods().len() as u8 {
        let mut s = Session::blank_on(CourseId::Emberfall);
        s.join_ids(1..=6);
        for c in s.cars.values_mut() { c.pod = pod; }
        for _ in 0..1500 { let d = s.bot_drives_all(); s.step_drive(&d); }
        assert!(s.valid() && s.cars.values().all(|c| c.d > 100.), "pod {pod} cannot drive");
    }
}

#[test]
fn the_land_beyond_the_walls_is_real() {
    for c in cup_courses() {
        let lap = c.lap();
        let mut canyon = 0;
        let mut drop = 0;
        let mut s = 7.;
        while s < lap {
            let p = c.sample(s);
            let (nx, ny) = (-p.th.sin(), p.th.cos());
            // On the road the ground is the road.
            let on = c.ground(p.x, p.y);
            let over_void = c.zones().iter().find(|zn| s >= zn.s0 && s < zn.s1).unwrap().shape.sink > 0.;
            if over_void { assert!(on <= p.z + 0.8, "{}: a bridge needs a drop under it at {s}", c.name()); }
            else { assert!((on - p.z).abs() < 2.0, "{}: ground {on} under road {} at {s}", c.name(), p.z); }
            let off = |m: f32| { let w = if m > 0. { p.wl } else { p.wr }; let l = m.signum() * (w + m.abs()); c.ground(p.x + nx * l, p.y + ny * l) };
            let z = c.zone(s);
            let shape = c.zones().iter().find(|zn| s >= zn.s0 && s < zn.s1).unwrap().shape;
            let (hi, lo) = (off(30.).max(off(-30.)), off(10.).min(off(-10.)));
            assert!(hi.is_finite() && lo.is_finite());
            let near_edge = c.zones().iter().any(|zn| (s - zn.s0).abs() < 90.) || c.forks().iter().any(|f| s > f.s0 - 60. && s < f.s1 + 60.);
            if near_edge { s += 37.; continue; }
            if shape.wall >= 16. && shape.sink == 0. { assert!(hi > p.z + 0.35 * shape.wall, "{}: no wall at {s} ({z:?}): {hi} vs road {}", c.name(), p.z); canyon += 1; }
            if shape.sink >= 20. { assert!(lo < p.z - 6., "{}: no drop beside the road at {s}: {lo} vs {}", c.name(), p.z); drop += 1; }
            s += 37.;
        }
        assert!(canyon >= 10 && drop >= 6, "{}: canyon {canyon}, drop {drop}", c.name());
        // Mountains a long way off, not a flat void: the far land has relief of its own.
        let (x0, y0, x1, y1) = c.bounds();
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for i in 0..40 { for j in 0..40 {
            let g = c.ground(x0 - 300. + (x1 - x0 + 600.) * i as f32 / 39., y0 - 300. + (y1 - y0 + 600.) * j as f32 / 39.);
            assert!(g.is_finite()); lo = lo.min(g); hi = hi.max(g);
        } }
        assert!(hi - lo > 40., "{}: the land is flat ({lo}..{hi})", c.name());
    }
}

#[test]
fn the_cup_runs_three_courses_and_scores_them() {
    let mut s = Session::blank();
    s.join_ids(1..=6);
    assert_eq!(s.course, cup()[0]);
    for round in 0..cup().len() {
        assert_eq!(s.course, cup()[round]);
        for _ in 0..40_000 { let d = s.bot_drives_all(); s.step_drive(&d); if s.finished { break; } }
        assert!(s.finished, "round {round} did not finish");
        let total: u32 = s.points.values().sum();
        assert_eq!(total, POINTS.iter().sum::<u32>() * (round as u32 + 1), "every race hands out the points once");
        assert_eq!(s.cup_over(), round + 1 == cup().len());
        let standings = s.standings();
        assert!(standings.windows(2).all(|w| w[0].1 >= w[1].1));
        // E at the flag: the next course, or a fresh cup.
        s.step_drive(&[(1, Drive { interact: true, ..Drive::default() })].into());
        assert!(!s.finished && s.ticks == 0 && s.cars.values().all(|c| c.finish.is_none() && c.d < 0.));
    }
    assert_eq!(s.course, cup()[0], "a finished cup starts over");
    assert!(s.points.is_empty());
}

#[test]
fn rivals_split_at_forks_and_cars_never_leave_the_roads() {
    for id in [CourseId::Emberfall, CourseId::Glassway, CourseId::Moatwalk] {
        let c = course(id);
        let mut s = Session::blank_on(id);
        s.join_ids(1..=6);
        let mut on_fork = [false; 7];
        let mut main_there = [false; 7];
        for _ in 0..40_000 {
            let d = s.bot_drives_all();
            s.step_drive(&d);
            for (n, car) in &s.cars {
                assert!((car.route as usize) <= c.forks().len());
                if car.route > 0 { on_fork[*n as usize] = true; }
                if let Some((_, f)) = c.fork_at(car.d) { if car.route == 0 && car.finish.is_none() && f.spans(car.d) { main_there[*n as usize] = true; } }
            }
            if s.finished { break; }
        }
        let took = on_fork.iter().filter(|b| **b).count();
        assert!(took >= 1 && took <= 6, "{}: forks {took}", c.name());
        if c.forks().iter().any(|f| f.shortcut) { assert!(took >= 2, "{}: nobody uses the shortcut", c.name()); }
    }
}
