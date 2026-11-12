//! Polish proof: the jump and the forks at close range, and a full lap, through the
//! real renderer. Writes PNG sequences when `ANGEL_PODRACER_POLISH` names a directory,
//! for the course `ANGEL_PODRACER_COURSE` names; otherwise does nothing.
//!
//! `jump_close`: the chase lens beside a fixed lens by the lip, from the run-up to the
//! landing. `fork_close`: the chase lens on a pod that takes each fork beside a high lens
//! over the split and the rejoin. `lap_chase`: the chase lens round one lap.
use super::super::*;
use super::shots::{aim, course_id, pack, save, view_frame};
use std::path::PathBuf;

fn out() -> Option<PathBuf> {
    let d = std::env::var_os("ANGEL_PODRACER_POLISH").map(PathBuf::from)?;
    std::fs::create_dir_all(&d).unwrap();
    Some(d)
}

fn tag(id: rules::CourseId) -> &'static str {
    match id { rules::CourseId::Proving => "proving", rules::CourseId::Emberfall => "emberfall", rules::CourseId::Glassway => "glassway", rules::CourseId::Moatwalk => "moatwalk" }
}

fn session(id: rules::CourseId) -> (Run, rules::Session) {
    let run = Run::roundtable_for_test("knight_racers").unwrap();
    let mut race = rules::Session::blank_on(id);
    race.join_ids(1..=6);
    // Past the grid's count, so the pods drive.
    race.ticks = 200;
    super::route::select(id, race.ticks);
    (run, race)
}

fn chase(run: &mut Run, race: &rules::Session, seat: u32) -> Img {
    run.knight_racers = Some(race.clone());
    layers(run, Some(seat)).0
}

/// The picture through `layers`, and a second lens, side by side in one file name each.
fn save_pair(dir: &std::path::Path, stem: &str, n: usize, a: &Img, b: &Img) {
    save(dir, &format!("{stem}_chase_{n:04}.png"), a);
    save(dir, &format!("{stem}_wide_{n:04}.png"), b);
}

#[test]
fn jump_close() {
    let Some(dir) = out() else { return };
    let id = course_id();
    let course = rules::course(id);
    let Some(gap) = course.gaps().first().copied() else { return };
    let (mut run, mut race) = session(id);
    for (slot, car) in race.cars.values_mut().enumerate() {
        car.d = gap.s0 - 190. - slot as f32 * 9.;
        car.y = if slot % 2 == 0 { -2.4 } else { 2.4 };
        car.v = 46.;
    }
    let mut n = 0;
    for _ in 0..900 {
        race.step_drive(&race.bot_drives_all());
        let car = &race.cars[&1];
        if car.d < gap.s0 - 120. { continue; }
        if car.d > gap.s1 + 55. { break; }
        let im = chase(&mut run, &race, 1);
        // The wide lens stands beyond the far lip, to the side and high, looking back at the
        // gap: the pod runs at it, leaves the lip, flies and lands by the lens.
        let r = super::route::current();
        let mut eye = super::point(r, gap.s1 + 34., -26., 22.);
        eye.z = eye.z.max(r.ground(eye.x, eye.y) + 12.);
        let target = super::point(r, car.d.clamp(gap.s0 - 90., gap.s1 + 30.), car.y * 0.3, car.lift.max(0.) * 0.6 + 1.5);
        let wide = view_frame(&race, car.d, &aim(eye, target, 1.0));
        save_pair(&dir, &format!("{}_jump", tag(id)), n, &im, &wide);
        if n % 4 == 0 { eprintln!("jump frame {n}: d {:.1} (gap {:.0}..{:.0}) lift {:.2} flying {} v {:.1}", car.d, gap.s0, gap.s1, car.lift, car.flying, car.v); }
        n += 1;
    }
    eprintln!("jump: {n} frames on {}", tag(id));
}

#[test]
fn fork_close() {
    let Some(dir) = out() else { return };
    let id = course_id();
    let course = rules::course(id);
    for (i, f) in course.forks().iter().enumerate() {
        let (mut run, mut race) = session(id);
        for (slot, car) in race.cars.values_mut().enumerate() {
            car.d = f.s0 - 200. - slot as f32 * 6.;
            car.y = if slot % 2 == 0 { -2.4 } else { 2.4 };
            car.v = 44.;
        }
        // The seat that takes this fork is the first one found on it.
        let mut seat = 0;
        for _ in 0..900 {
            race.step_drive(&race.bot_drives_all());
            if let Some((&id, _)) = race.cars.iter().find(|(_, c)| c.route == i as u8 + 1) { seat = id; break; }
        }
        if seat == 0 { eprintln!("fork {i}: no pod took it"); continue; }
        // Back to the run-up, with that pod leading, and film it through.
        let (mut run2, mut race2) = session(id);
        for (slot, car) in race2.cars.values_mut().enumerate() {
            car.d = f.s0 - 200. - if car_id(slot) == seat { 0. } else { 14. + slot as f32 * 6. };
            car.y = race.cars[&car_id(slot)].y.signum() * 2.4;
            car.v = 44.;
        }
        let _ = (&mut run, &mut run2);
        let mut n = 0;
        let mut was_on = false;
        for _ in 0..1400 {
            race2.step_drive(&race2.bot_drives_all());
            let car = &race2.cars[&seat];
            if car.d < f.s0 - 110. { continue; }
            if car.d > f.s1 + 50. { break; }
            if car.route != 0 { was_on = true; }
            let im = chase(&mut run2, &race2, seat);
            // The plan: straight down over the middle of the two roads where the pod is, high enough
            // for both, the pod marked. The roads part and join under it.
            let r = super::route::current();
            let at_s = car.d.clamp(f.s0, f.s1);
            let mid = |s: f32| { let (a, b) = (course.frame(0, s), course.frame(i as u8 + 1, s)); v3((a.x + b.x) * 0.5, (a.y + b.y) * 0.5, (a.z + b.z) * 0.5) };
            let c = mid(at_s);
            let (_, _, th) = course.pose(at_s);
            // High enough for the widest the two roads lie apart.
            let apart = (0..=24).map(|k| { let s = f.s0 + (f.s1 - f.s0) * k as f32 / 24.; let (a, b) = (course.frame(0, s), course.frame(i as u8 + 1, s)); (a.x - b.x).hypot(a.y - b.y) }).fold(0., f32::max);
            let height = ((apart * 0.5 + 28.) / 0.58).clamp(90., 340.);
            let view = aim(c + v3(0., 0., height), c + v3(th.cos(), th.sin(), 0.) * 3., 1.1);
            let mut high = view_frame(&race2, car.d, &view);
            // The two roads' centre lines drawn over the picture, so the split can be followed
            // through the dark: the main road light, the fork's gold.
            for (line, ink) in [(0u8, 'h'), (i as u8 + 1, 'O')] {
                let mut prev: Option<(i32, i32)> = None;
                let mut sd = at_s - 260.;
                while sd <= at_s + 260. {
                    let cl = sd.clamp(f.s0, f.s1);
                    let fr = course.frame(line, if line == 0 { sd } else { cl });
                    let here = raster::project_point(&view, 384, 224, v3(fr.x, fr.y, fr.z + 0.3)).map(|(x, y, _)| (x as i32, y as i32));
                    if line == 0 || (sd >= f.s0 && sd <= f.s1) {
                        if let (Some(a), Some(b)) = (prev, here) { high.line(a.0, a.1, b.0, b.1, ink); }
                        prev = here;
                    } else { prev = None; }
                    sd += 8.;
                }
            }
            let me = super::point(r, car.d, car.y, 0.5);
            let me = if car.route != 0 { let fr = course.frame(car.route, car.d); v3(fr.x - car.y * fr.th.sin(), fr.y + car.y * fr.th.cos(), fr.z) } else { me };
            if let Some((x, y, _)) = raster::project_point(&view, 384, 224, me) {
                let (x, y) = (x as i32, y as i32);
                high.rect(x - 4, y - 4, 9, 9, 'k');
                high.rect(x - 3, y - 3, 7, 7, 'T');
            }
            let phase = if car.d < f.s0 + 20. { "THE SPLIT" } else if car.d > f.s1 - 20. { "THE REJOIN" } else if car.route != 0 { "THE CUT" } else { "THE MAIN ROAD" };
            high.text(8, 24, phase, 'T');
            save_pair(&dir, &format!("{}_fork{i}", tag(id)), n, &im, &high);
            n += 1;
        }
        eprintln!("fork {i} ({}): {n} frames, seat {seat}, was on the fork: {was_on}", f.name);
    }
}

fn car_id(slot: usize) -> u32 { slot as u32 + 1 }

#[test]
fn lap_chase() {
    let Some(dir) = out() else { return };
    let id = course_id();
    let course = rules::course(id);
    let every: u32 = std::env::var("ANGEL_PODRACER_EVERY").ok().and_then(|v| v.parse().ok()).unwrap_or(8);
    let laps: f32 = std::env::var("ANGEL_PODRACER_LAPS").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let mut race = rules::Session::blank_on(id);
    race.join_ids(1..=6);
    let mut run = Run::roundtable_for_test("knight_racers").unwrap();
    let mut n = 0;
    let mut tick = 0;
    loop {
        race.step_drive(&race.bot_drives_all());
        tick += 1;
        let lead = race.cars.values().map(|c| c.d).fold(0., f32::max);
        if tick % every == 0 && race.ticks > 60 {
            let im = chase(&mut run, &race, 1);
            save(&dir, &format!("{}_lap_{n:04}.png", tag(id)), &im);
            n += 1;
        }
        if lead >= course.lap() * laps || race.ticks > 30_000 { break; }
    }
    let _ = pack;
    eprintln!("lap: {n} frames on {}", tag(id));
}

/// Three seconds flat out down the straightest stretch of the lap (or from `ANGEL_PODRACER_AT`), a
/// frame every tick, the boost held: what 600 km/h looks like through the chase lens.
#[test]
fn straight_run() {
    let Some(dir) = out() else { return };
    let id = course_id();
    let course = rules::course(id);
    let at: f32 = std::env::var("ANGEL_PODRACER_AT").ok().and_then(|v| v.split(',').next().and_then(|x| x.parse().ok())).unwrap_or_else(|| {
        // The start of the 220 m window with the least bend in it, off the grid and clear of any gap.
        let (mut best, mut at) = (f32::MAX, 120.);
        let mut s = 120.;
        while s < course.lap() - 320. {
            let bend = (0..22).map(|k| course.curve(s + k as f32 * 10.).abs()).fold(0., f32::max);
            let gapped = course.gaps().iter().any(|g| g.s1 > s - 40. && g.s0 < s + 260.);
            if !gapped && bend < best { best = bend; at = s; }
            s += 20.;
        }
        at
    });
    let (mut run, mut race) = session(id);
    for (slot, car) in race.cars.values_mut().enumerate() {
        car.d = at - slot as f32 * 9.;
        car.y = if slot % 2 == 0 { -3.2 } else { 3.2 };
        car.v = 44.;
    }
    race.cars.get_mut(&1).unwrap().y = 0.;
    for n in 0..92 {
        let mut drives = race.bot_drives_all();
        drives.insert(1, rules::Drive { gas: true, boost: true, ..Default::default() });
        race.step_drive(&drives);
        if n < 2 { continue; }
        save(&dir, &format!("{}_straight_{:04}.png", tag(id), n - 2), &chase(&mut run, &race, 1));
    }
    eprintln!("straight: from {at:.0} m on {}, pod 1 now at {:.0} m doing {:.1} m/s", tag(id), race.cars[&1].d, race.cars[&1].v);
}

/// A pod brakes to a stop, turns right about, drives off the wrong way, and turns back: the
/// banner comes up once the nose has been held against the road, and goes when it is along it.
#[test]
fn wrong_way_turn() {
    let Some(dir) = out() else { return };
    let id = course_id();
    let (mut run, mut race) = session(id);
    race.cars.retain(|id, _| *id == 1 || *id == 2);
    for (slot, car) in race.cars.values_mut().enumerate() {
        car.d = 700. + slot as f32 * 14.;
        car.y = if slot == 0 { 0. } else { 3. };
        car.v = 40.;
    }
    // Pod 2 keeps driving on its own; pod 1 is the knight's.
    let mut n = 0;
    let script: Vec<(u32, rules::Drive)> = [(40, (0, true, false)), (50, (0, false, true)), (80, (1, false, true)), (70, (0, true, false)), (70, (0, false, true)), (80, (-1, false, true)), (70, (0, true, false))]
        .into_iter().flat_map(|(len, (steer, gas, brake))| (0..len).map(move |_| (0u32, rules::Drive { steer, gas, brake, ..Default::default() }))).collect();
    for (_, mine) in script {
        let mut drives = race.bot_drives_all();
        drives.insert(1, mine);
        race.step_drive(&drives);
        super::super::instruments::state::observe(&race);
        if race.ticks % 2 == 0 {
            let im = chase(&mut run, &race, 1);
            save(&dir, &format!("{}_wrongway_{n:04}.png", tag(id)), &im);
            n += 1;
        }
    }
    eprintln!("wrong way: {n} frames on {}", tag(id));
}

/// Chase frames through `layers` at the distances in `ANGEL_PODRACER_AT` (comma separated).
#[test]
fn snaps() {
    let Some(dir) = out() else { return };
    let at: Vec<f32> = std::env::var("ANGEL_PODRACER_AT").unwrap_or_default().split(',').filter_map(|v| v.parse().ok()).collect();
    let id = course_id();
    let (mut run, mut race) = session(id);
    for s in at {
        for (slot, car) in race.cars.values_mut().enumerate() {
            car.d = s - slot as f32 * 8.;
            car.y = if slot % 2 == 0 { -3.2 } else { 3.2 };
            car.v = 40.;
        }
        save(&dir, &format!("{}_at_{s:.0}.png", tag(id)), &chase(&mut run, &race, 1));
    }
}

/// How many triangles a frame holds, by what they are, round each course's lap. Prints when
/// `ANGEL_PODRACER_TALLY` is set.
#[test]
fn tally() {
    if std::env::var_os("ANGEL_PODRACER_TALLY").is_none() { return; }
    for &id in rules::courses() {
        let r = super::route::on(id);
        super::route::select(id, 0);
        let (mut sum, mut n, mut worst) = (0, 0, 0);
        let mut by = [0usize; 6];
        let mut s = 3.;
        while s < r.lap() {
            let mut t = super::Tally::default();
            let all = super::gather(r, s, 0, &mut t).tris.len();
            worst = worst.max(all);
            sum += all; n += 1;
            for (m, v) in by.iter_mut().zip([t.sky, t.road, t.props, t.pieces, t.hazards, t.land]) { *m += v; }
            s += 40.;
        }
        eprintln!("TALLY {}: mean {} (sky, road, props, pieces, hazards, land: {:?}), worst {worst}", tag(id), sum / n, by.map(|m| m / n));
    }
}

/// How far each fork's road lies from the main road, along the fork, where they part and join.
#[test]
fn fork_spread() {
    if std::env::var_os("ANGEL_PODRACER_TALLY").is_none() { return; }
    for &id in rules::courses() {
        let course = rules::course(id);
        for (i, f) in course.forks().iter().enumerate() {
            let at = |s: f32| { let (a, b) = (course.frame(0, s), course.frame(i as u8 + 1, s)); (a.x - b.x).hypot(a.y - b.y) };
            let row: Vec<String> = (0..=12).map(|k| { let s = f.s0 + (f.s1 - f.s0) * k as f32 / 12.; format!("{:.0}", at(s)) }).collect();
            eprintln!("SPREAD {} fork {i} {} {:.0}..{:.0}: {}", tag(id), f.name, f.s0, f.s1, row.join(" "));
            let rise: Vec<String> = (0..=12).map(|k| { let s = f.s0 + (f.s1 - f.s0) * k as f32 / 12.; let b = course.frame(i as u8 + 1, s); format!("{:.1}", course.ground(b.x, b.y) - b.z) }).collect();
            eprintln!("SPREAD   land over the fork's road: {}", rise.join(" "));
        }
    }
}

/// A plan of each fork from straight above: the two roads and where they part and join.
#[test]
fn fork_plan() {
    let Some(dir) = out() else { return };
    let id = course_id();
    let course = rules::course(id);
    let (_, race) = session(id);
    for (i, f) in course.forks().iter().enumerate() {
        for (k, frac) in [0.0f32, 0.15, 0.5, 0.85, 1.0].into_iter().enumerate() {
            let s = f.s0 + (f.s1 - f.s0) * frac;
            let (a, b) = (course.frame(0, s), course.frame(i as u8 + 1, s));
            let c = v3((a.x + b.x) * 0.5, (a.y + b.y) * 0.5, (a.z + b.z) * 0.5);
            let eye = c + v3(0., 0., 300.);
            let (_, _, th) = course.pose(s);
            let view = aim(eye, c + v3(th.cos(), th.sin(), 0.) * 4., 0.9);
            save(&dir, &format!("{}_plan{i}_{k}.png", tag(id)), &view_frame(&race, s, &view));
            eprintln!("plan {i} {k}: main ({:.0},{:.0},{:.1}) fork ({:.0},{:.0},{:.1})", a.x, a.y, a.z, b.x, b.y, b.z);
        }
    }
}
