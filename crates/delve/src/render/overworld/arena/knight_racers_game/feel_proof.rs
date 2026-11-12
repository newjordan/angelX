//! The feel proof: five moments of four seconds each (a boost down a straight, a hard bend,
//! a wall hit, a jump landing, a close pass), driven by a script, so the sim is the same
//! whatever the camera and the effects do. Run once on the old picture and once on the new:
//!
//!   ANGEL_FEEL_PROOF=/dir ANGEL_FEEL_TAG=after cargo test -p angel-delve --profile fast --lib feel_moments
//!
//! Frames land in `/dir/<tag>/<moment>/NNNN.png` (384x224, one per tick) with a `meta.txt`
//! naming the tick the moment turns on, for the sheet and GIF builder to centre on.
use super::*;
use crate::rules::Run;
use std::collections::BTreeMap;
use std::path::PathBuf;

const CLIP: u32 = 120;

/// The time this thread has spent on a core, in milliseconds (the kernel's own count). A busy
/// machine makes wall time useless for a 4 ms frame; time spent waiting for a core is not counted here.
fn cpu_ms() -> f64 {
    std::fs::read_to_string("/proc/thread-self/schedstat").ok()
        .and_then(|s| s.split_whitespace().next().and_then(|v| v.parse::<u64>().ok()))
        .map_or(0., |ns| ns as f64 / 1.0e6)
}

struct Rig {
    run: Run,
    race: rules::Session,
}

impl Rig {
    fn on(id: rules::CourseId) -> Rig {
        let mut run = Run::roundtable_for_test("knight_racers").unwrap();
        run.knight_racers = None;
        let mut race = rules::Session::blank_on(id);
        race.join_ids(1..=6);
        // Past the count, with the lap clocks fresh.
        race.ticks = rules::race::COUNTDOWN + 60;
        super::super::worldkit::route::select(id, race.ticks);
        Rig { run, race }
    }

    /// Seat 1 at `d`, `y`, going `v`; the five rivals strewn up and down the road around it.
    fn stage(&mut self, d: f32, y: f32, v: f32, rivals: [(f32, f32, f32); 5]) {
        for (id, car) in self.race.cars.iter_mut() {
            let (dd, yy, vv) = if *id == 1 { (0., y, v) } else { rivals[*id as usize - 2] };
            car.d = d + dd; car.y = yy; car.v = vv; car.vy = 0.; car.boost = 1.; car.boost_locked = false;
            car.lift = 0.; car.vz = 0.; car.flying = false; car.impact = 0; car.yaw = 0.; car.route = 0;
        }
    }

    fn step(&mut self, mine: &dyn Fn(u32, &rules::Session) -> rules::Drive, clip_tick: u32) {
        advance(&mut self.race, mine, clip_tick);
        state::observe(&self.race);
    }

    fn frame(&mut self) -> Img {
        self.run.knight_racers = Some(self.race.clone());
        layers(&self.run, Some(1)).0
    }
}

fn advance(race: &mut rules::Session, mine: &dyn Fn(u32, &rules::Session) -> rules::Drive, clip_tick: u32) {
    let mut all = race.bot_drives_all();
    all.insert(1, mine(clip_tick, race));
    race.step_drive(&all);
    assert!(race.valid());
}

/// The first clip tick at which `turn` holds, running the script from `race` as it stands.
fn probe(race: &rules::Session, script: &dyn Fn(u32, &rules::Session) -> rules::Drive, turn: &dyn Fn(&rules::Session) -> bool) -> u32 {
    let mut r = race.clone();
    for n in 0..CLIP {
        if turn(&r) { return n; }
        advance(&mut r, script, n);
    }
    CLIP / 2
}

fn gas() -> rules::Drive { rules::Drive { gas: true, ..Default::default() } }

fn bot(race: &rules::Session) -> rules::Drive {
    let mut d = race.bot_drives_all()[&1];
    d.gas = true;
    d
}

fn out() -> Option<(PathBuf, String)> {
    let dir = PathBuf::from(std::env::var_os("ANGEL_FEEL_PROOF")?);
    let tag = std::env::var("ANGEL_FEEL_TAG").unwrap_or_else(|_| "after".into());
    Some((dir, tag))
}

/// Film `CLIP` ticks of `script` from the staged race: the tick the moment turns on is noted.
fn film(rig: &mut Rig, dir: &std::path::Path, name: &str, turn: u32, script: &dyn Fn(u32, &rules::Session) -> rules::Drive) {
    let dir = dir.join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for n in 0..CLIP {
        let im = rig.frame();
        image::save_buffer(dir.join(format!("{n:04}.png")), &im.rgba_bytes(), 384, 224, image::ColorType::Rgba8).unwrap();
        rig.step(script, n);
    }
    std::fs::write(dir.join("meta.txt"), format!("{turn}\n")).unwrap();
}

/// The first stretch of at least `len` metres whose bends are all gentler than `k`.
fn straight(course: &rules::Course, from: f32, len: f32, k: f32) -> f32 {
    let mut s = from;
    while s < course.lap() - len {
        if (0..=(len / 10.) as i32).all(|i| course.frame(0, s + i as f32 * 10.).k.abs() < k) { return s; }
        s += 10.;
    }
    from
}

#[test]
fn feel_moments() {
    let Some((dir, tag)) = out() else { return };
    let dir = dir.join(&tag);
    let only = std::env::var("ANGEL_FEEL_ONLY").ok();
    let want = |n: &str| only.as_deref().is_none_or(|o| o == n);

    // A boost down a straight: cruise, then the key goes down at 1.0 s.
    if want("boost") {
        let mut rig = Rig::on(rules::CourseId::Emberfall);
        let s0 = straight(rig.race.course(), 150., 380., 0.0012);
        eprintln!("boost on a straight from {s0}");
        rig.stage(s0, 0., 38., [(-30., 3.5, 40.), (40., -3.2, 40.), (95., 2.5, 40.), (170., -2., 40.), (-80., 0., 40.)]);
        let boost = 30;
        film(&mut rig, &dir, "boost", boost, &|n, _| rules::Drive { boost: n >= boost, ..gas() });
    }

    // A hard bend: the sharpest in the first lap's opening, taken by the CPU pilot at speed.
    if want("bend") {
        let id = rules::CourseId::Glassway;
        let mut rig = Rig::on(id);
        let course = rig.race.course();
        let (mut at, mut best) = (300., 0.);
        let mut s = 300.;
        while s < 2500. { let k = course.frame(0, s).k.abs(); if k > best { best = k; at = s; } s += 5.; }
        eprintln!("hard bend at {at} (k {best:.4})");
        rig.stage(at - 75., 0., 42., [(-30., 3.5, 40.), (40., -3.2, 40.), (95., 2.5, 40.), (170., -2., 40.), (-80., 0., 40.)]);
        film(&mut rig, &dir, "bend", 60, &|_, r| bot(r));
    }

    // A wall: flat out down the straight, the stick hard over until the stone takes the engine.
    if want("wall") {
        let mut rig = Rig::on(rules::CourseId::Emberfall);
        let s0 = straight(rig.race.course(), 150., 300., 0.0012);
        let edge = rig.race.course().frame(0, s0).wl;
        rig.stage(s0, edge - 4.5, 44., [(-30., -3.5, 40.), (40., -3.2, 40.), (95., -2.5, 40.), (170., -2., 40.), (-80., -4., 40.)]);
        let left = |_: u32, _: &rules::Session| rules::Drive { steer: 1, ..gas() };
        let hit = probe(&rig.race, &left, &|r| r.cars[&1].impact >= 12);
        eprintln!("wall: the stone takes the engine at tick {hit}");
        // The stick over for 30 ticks (well past the stone), back for 14, then centred: the same
        // hands whatever the camera and the effects do.
        let steer = |n: u32, _: &rules::Session| rules::Drive { steer: if n < 30 { 1 } else if n < 44 { -1 } else { 0 }, ..gas() };
        film(&mut rig, &dir, "wall", hit, &steer);
    }

    // A jump: the first gap of the cup's first course, taken at speed; the clip turns on the landing.
    if want("jump") {
        let id = rules::CourseId::Emberfall;
        let mut probe = Rig::on(id);
        let gap = probe.race.course().gaps()[0];
        let start = gap.s0 - 140.;
        let rivals = [(-30., 3.5, 44.), (-55., -3.2, 44.), (-95., 2.5, 44.), (-170., -2., 44.), (-220., 0., 44.)];
        probe.stage(start, 0., 44., rivals);
        let script = |n: u32, r: &rules::Session| { let mut d = bot(r); d.boost = n > 8; d };
        let (mut took, mut landed) = (None, None);
        for n in 0..600 {
            probe.step(&script, n);
            let c = &probe.race.cars[&1];
            if c.flying && took.is_none() { took = Some(n); }
            if took.is_some() && !c.flying && landed.is_none() { landed = Some(n); break; }
        }
        let landed = landed.expect("the pod lands");
        eprintln!("jump: gap {:.0}..{:.0}, takes off at tick {:?}, lands at {landed}", gap.s0, gap.s1, took);
        // Cut so the landing falls at 2.4 s into the clip.
        let pre = landed.saturating_sub(72);
        let mut rig = Rig::on(id);
        rig.stage(start, 0., 44., rivals);
        for n in 0..pre { rig.step(&script, n); }
        film(&mut rig, &dir, "jump", landed - pre, &|n, r| script(n + pre, r));
    }

    // A close pass: a rival that has spent its boost, taken by a pod at full burn a lane over.
    if want("pass") {
        let mut rig = Rig::on(rules::CourseId::Emberfall);
        let s0 = straight(rig.race.course(), 150., 380., 0.0012);
        rig.stage(s0, -1.6, 50., [(30., 2.0, 30.), (110., -3.0, 44.), (190., 2.5, 44.), (-60., 3.0, 44.), (-120., -2., 44.)]);
        for (id, car) in rig.race.cars.iter_mut() {
            if *id == 2 { car.boost = 0.; car.boost_locked = true; }
            if *id == 1 { car.boost = 1.; }
        }
        let script = |_: u32, _: &rules::Session| rules::Drive { boost: true, ..gas() };
        let side = probe(&rig.race, &script, &|r| r.cars[&1].d >= r.cars[&2].d);
        eprintln!("pass: abreast at tick {side}");
        film(&mut rig, &dir, "pass", side, &script);
    }
}

/// The cost of the stages of the feel kit, one switched off at a time, on the same frames:
///   ANGEL_FEEL_STAGES=2 cargo test -p angel-delve --profile fast --lib feel_stages -- --nocapture
#[test]
fn feel_stages() {
    let Some(rounds) = std::env::var("ANGEL_FEEL_STAGES").ok().and_then(|v| v.parse::<usize>().ok()) else { return };
    let names = ["all on", "wash", "streaks", "plume", "marks", "shock", "whoosh", "radial", "vignette", "particles", "ALL OFF"];
    let masks = [0u32, 1, 2, 4, 8, 16, 32, 64, 128, 256, 511];
    let ids = [rules::CourseId::Proving, rules::CourseId::Emberfall, rules::CourseId::Glassway, rules::CourseId::Moatwalk];
    let mut run = Run::roundtable_for_test("knight_racers").unwrap();
    let mut sum = vec![0f64; masks.len()];
    let mut n = 0;
    for k in 0..8 {
        for &id in &ids {
            let mut race = rules::Session::blank_on(id);
            race.join_ids(1..=6);
            race.ticks = rules::race::COUNTDOWN + 30;
            let d = rules::course(id).lap() * k as f32 / 8. + 30.;
            for (slot, car) in race.cars.values_mut().enumerate() {
                car.d = d - slot as f32 * 7.;
                car.y = if slot % 2 == 0 { -3.2 } else { 3.2 };
                car.v = std::env::var("ANGEL_FEEL_V").ok().and_then(|v| v.parse().ok()).unwrap_or(46.);
                car.boosting = slot == 0 && std::env::var_os("ANGEL_FEEL_NOBOOST").is_none();
            }
            for _ in 0..12 {
                race.step_drive(&race.bot_drives_all());
                run.knight_racers = Some(race.clone());
                let _ = layers(&run, Some(1));
            }
            run.knight_racers = Some(race.clone());
            // The fastest of the rounds is the frame's own cost: a busy machine only adds to the rest.
            let mut best = vec![f64::MAX; masks.len()];
            for _ in 0..rounds {
                for (v, &mask) in masks.iter().enumerate() {
                    super::fx::SKIP.with(|s| s.set(mask));
                    let at = cpu_ms();
                    let _ = layers(&run, Some(1));
                    best[v] = best[v].min(cpu_ms() - at);
                }
            }
            for v in 0..masks.len() { sum[v] += best[v]; }
            n += 1;
        }
    }
    super::fx::SKIP.with(|s| s.set(0));
    for (v, name) in names.iter().enumerate() {
        eprintln!("{name:>10}: {:.3} ms   (kit costs {:.3} ms)", sum[v] / n as f64, (sum[0] - sum[v]) / n as f64);
    }
}

/// The cost of a frame of the race under way, per course, interleaved: every course at eight
/// places round the lap, round after round, ten ticks of racing into each so the lens and the
/// marks are warm. Prints each course's mean of medians and its ratio to the proving run.
///
///   ANGEL_FEEL_COST=5 cargo test -p angel-delve --profile fast --lib feel_cost -- --nocapture
#[test]
fn feel_cost() {
    let Some(rounds) = std::env::var("ANGEL_FEEL_COST").ok().and_then(|v| v.parse::<usize>().ok()) else { return };
    let ids = [rules::CourseId::Proving, rules::CourseId::Emberfall, rules::CourseId::Glassway, rules::CourseId::Moatwalk];
    let mut run = Run::roundtable_for_test("knight_racers").unwrap();
    let mut times: Vec<Vec<Vec<f64>>> = vec![vec![Vec::new(); 8]; ids.len()];
    for k in 0..8 {
        for _ in 0..rounds {
            for (c, &id) in ids.iter().enumerate() {
                let mut race = rules::Session::blank_on(id);
                race.join_ids(1..=6);
                race.ticks = rules::race::COUNTDOWN + 30;
                let d = rules::course(id).lap() * k as f32 / 8. + 30.;
                for (slot, car) in race.cars.values_mut().enumerate() {
                    car.d = d - slot as f32 * 7.;
                    car.y = if slot % 2 == 0 { -3.2 } else { 3.2 };
                    car.v = 40.;
                }
                for t in 0..10 {
                    race.step_drive(&race.bot_drives_all());
                    run.knight_racers = Some(race.clone());
                    let at = cpu_ms();
                    let _ = layers(&run, Some(1));
                    if t == 9 { times[c][k].push(cpu_ms() - at); }
                }
            }
        }
    }
    // The fastest sample of each place is its own cost; a busy machine only adds. Medians too, for the record.
    let stat = |pick: &dyn Fn(&mut Vec<f64>) -> f64| -> Vec<f64> {
        times.iter().map(|pts| {
            let each: Vec<f64> = pts.iter().map(|v| { let mut v = v.clone(); v.sort_by(f64::total_cmp); pick(&mut v) }).collect();
            each.iter().sum::<f64>() / each.len() as f64
        }).collect()
    };
    let best = stat(&|v| v[0]);
    let med = stat(&|v| v[v.len() / 2]);
    for (c, &id) in ids.iter().enumerate() {
        eprintln!("{:?}: mean of mins {:.3} ms ({:.3}x proving), mean of medians {:.2} ms", id, best[c], best[c] / best[0], med[c]);
    }
}

/// Where the extra cost of a frame comes from: the new lens or the kit. The same frames four ways,
/// the fastest of the rounds each: the new camera and the old camera's numbers, the kit on and off.
///
///   ANGEL_FEEL_ATTR=8 cargo test -p angel-delve --profile fast --lib feel_attribution -- --nocapture
#[test]
fn feel_attribution() {
    let Some(rounds) = std::env::var("ANGEL_FEEL_ATTR").ok().and_then(|v| v.parse::<usize>().ok()) else { return };
    let old = [("BACK", "22"), ("BACKV", "-2"), ("UP", "8"), ("UPV", "-0.9"), ("FOV0", "1.10"), ("FOV1", "1.16"), ("FOLLOW", "0.45"), ("AIMZ", "0")];
    let variants: [(&str, bool, u32); 4] = [("new lens, kit on", false, 0), ("new lens, kit off", false, 511), ("old numbers, kit off", true, 511), ("old numbers, kit on", true, 0)];
    let ids = [rules::CourseId::Proving, rules::CourseId::Emberfall, rules::CourseId::Glassway, rules::CourseId::Moatwalk];
    let mut run = Run::roundtable_for_test("knight_racers").unwrap();
    let mut sum = [0f64; 4];
    let mut n = 0;
    for k in 0..8 {
        for &id in &ids {
            let mut race = rules::Session::blank_on(id);
            race.join_ids(1..=6);
            race.ticks = rules::race::COUNTDOWN + 30;
            let d = rules::course(id).lap() * k as f32 / 8. + 30.;
            for (slot, car) in race.cars.values_mut().enumerate() {
                car.d = d - slot as f32 * 7.;
                car.y = if slot % 2 == 0 { -3.2 } else { 3.2 };
                car.v = 40.;
            }
            for _ in 0..10 {
                race.step_drive(&race.bot_drives_all());
                run.knight_racers = Some(race.clone());
                let _ = layers(&run, Some(1));
            }
            run.knight_racers = Some(race.clone());
            let mut best = [f64::MAX; 4];
            for _ in 0..rounds {
                for (v, &(_, old_lens, mask)) in variants.iter().enumerate() {
                    for (key, val) in old {
                        let key = format!("ANGEL_CAM_{key}");
                        // Safe here: the test is single threaded and reads the numbers only in `layers`.
                        unsafe { if old_lens { std::env::set_var(&key, val) } else { std::env::remove_var(&key) } }
                    }
                    super::camera::tune_reset();
                    super::fx::SKIP.with(|s| s.set(mask));
                    let at = cpu_ms();
                    let _ = layers(&run, Some(1));
                    best[v] = best[v].min(cpu_ms() - at);
                }
            }
            for v in 0..4 { sum[v] += best[v]; }
            n += 1;
        }
    }
    super::fx::SKIP.with(|s| s.set(0));
    for (v, &(name, _, _)) in variants.iter().enumerate() { eprintln!("{name:>22}: {:.3} ms", sum[v] / n as f64); }
}

/// The processor time of the whole process (every thread, the raster's workers too), in milliseconds,
/// from the kernel's own count in ticks of 10 ms: only a long batch of frames reads well on it. It is
/// not stolen from by a busy machine the way wall time is.
fn process_cpu_ms() -> f64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    // After the command's closing bracket: state, then fields 4.. ; utime and stime are fields 14 and 15.
    let rest = stat.rsplit_once(')').map_or("", |(_, r)| r);
    let f: Vec<&str> = rest.split_whitespace().collect();
    let tick = |i: usize| f.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.);
    (tick(11) + tick(12)) * 10.
}

/// What a frame costs in processor time, every course at eight places, `ANGEL_FEEL_CPU` frames at each
/// (the same state again, so the lens and the marks are warm): per course and over all.
///
///   ANGEL_FEEL_CPU=40 cargo test -p angel-delve --profile fast --lib feel_cpu -- --nocapture
#[test]
fn feel_cpu() {
    let Some(frames) = std::env::var("ANGEL_FEEL_CPU").ok().and_then(|v| v.parse::<usize>().ok()) else { return };
    let ids = [rules::CourseId::Proving, rules::CourseId::Emberfall, rules::CourseId::Glassway, rules::CourseId::Moatwalk];
    let mut run = Run::roundtable_for_test("knight_racers").unwrap();
    let mut spent = [0f64; 4];
    for k in 0..8 {
        for (c, &id) in ids.iter().enumerate() {
            let mut race = rules::Session::blank_on(id);
            race.join_ids(1..=6);
            race.ticks = rules::race::COUNTDOWN + 30;
            let d = rules::course(id).lap() * k as f32 / 8. + 30.;
            for (slot, car) in race.cars.values_mut().enumerate() {
                car.d = d - slot as f32 * 7.;
                car.y = if slot % 2 == 0 { -3.2 } else { 3.2 };
                car.v = 40.;
            }
            for _ in 0..10 {
                race.step_drive(&race.bot_drives_all());
                run.knight_racers = Some(race.clone());
                let _ = layers(&run, Some(1));
            }
            run.knight_racers = Some(race.clone());
            let _ = layers(&run, Some(1));
            let at = process_cpu_ms();
            for _ in 0..frames { std::hint::black_box(layers(&run, Some(1))); }
            spent[c] += process_cpu_ms() - at;
        }
    }
    for (c, &id) in ids.iter().enumerate() {
        let each = spent[c] / (8 * frames) as f64;
        eprintln!("{:?}: {:.3} ms of processor time a frame ({:.3}x proving)", id, each, spent[c] / spent[0]);
    }
    eprintln!("ALL: {:.3} ms a frame", spent.iter().sum::<f64>() / (32 * frames) as f64);
}
