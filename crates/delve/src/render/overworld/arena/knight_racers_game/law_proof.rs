//! Proof reels for the racer's law: the light tree, heat building into a burning
//! engine, a blowout and the squires' patch, a side slam, and an AI pass through
//! the draft. Staged by hand on a clear stretch (the sim runs as it always does);
//! each reel is a folder of 384x224 frames, cut into GIFs by the proof script.
//!
//!   ANGEL_LAW_PROOF=/dir ANGEL_LAW_TAG=after cargo test -p angel-delve --lib law_reels
//!   (ANGEL_LAW_REEL=heat|slam|overtake|countdown for one)
use super::*;
use crate::rules::Run;
use std::collections::BTreeMap;

type Drives = BTreeMap<u32, rules::Drive>;

/// A race staged on one stretch, watched from one seat.
struct Reel {
    run: Run,
    race: rules::Session,
    seat: u32,
    shots: u32,
}

impl Reel {
    fn on(id: rules::CourseId, seat: u32) -> Reel {
        let mut run = Run::roundtable_for_test("knight_racers").unwrap();
        run.knight_racers = None;
        let mut race = rules::Session::blank_on(id);
        race.join_ids(1..=6);
        Reel { run, race, seat, shots: 0 }
    }

    /// Into the race, then every pod where `field` says: (seat, distance, lane, speed).
    fn stage(&mut self, field: &[(u32, f32, f32, f32)]) {
        self.race.ticks = rules::race::COUNTDOWN + 1;
        for &(id, d, y, v) in field {
            let c = self.race.cars.get_mut(&id).unwrap();
            c.d = d; c.y = y; c.v = v; c.vy = 0.;
        }
    }

    fn step(&mut self, drives: &Drives) {
        self.race.step_drive(drives);
        assert!(self.race.valid(), "the staged race went invalid at tick {}", self.race.ticks);
        state::observe(&self.race);
    }

    /// The pilots' hands for everyone but `except`.
    fn pilots(&self, except: &[u32]) -> Drives {
        let mut all = self.race.bot_drives_all();
        for id in except { all.remove(id); }
        all
    }

    fn frame(&mut self) -> Img {
        self.run.knight_racers = Some(self.race.clone());
        layers(&self.run, Some(self.seat)).0
    }

    fn shot(&mut self, dir: &std::path::Path) {
        std::fs::create_dir_all(dir).unwrap();
        let im = self.frame();
        image::save_buffer(dir.join(format!("{:04}.png", self.shots)), &im.rgba_bytes(), 384, 224, image::ColorType::Rgba8).unwrap();
        self.shots += 1;
    }
}

/// Steer a pod back toward lane `to`.
fn hold_lane(c: &rules::Car, to: f32) -> i8 {
    if c.y > to + 0.6 { -1 } else if c.y < to - 0.6 { 1 } else { 0 }
}

fn out_dir() -> Option<std::path::PathBuf> {
    let root = std::env::var_os("ANGEL_LAW_PROOF").map(std::path::PathBuf::from)?;
    let tag = std::env::var("ANGEL_LAW_TAG").unwrap_or_else(|_| "after".into());
    Some(root.join(tag))
}

fn wanted(name: &str) -> bool {
    std::env::var("ANGEL_LAW_REEL").map_or(true, |w| w == name)
}

#[test]
fn law_reels() {
    let Some(out) = out_dir() else { return };
    if wanted("countdown") { countdown(&out.join("countdown")); }
    if wanted("heat") { heat(&out.join("heat")); }
    if wanted("slam") { slam(&out.join("slam")); }
    if wanted("overtake") { overtake(&out.join("overtake")); }
    if wanted("battle") { battle(&out.join("battle")); }
}

/// What a seat saw in one tick of an all-pilot race: for the battle reel's search.
#[derive(Clone, Copy, Default)]
struct Beat { slam: bool, draft: bool, blow_near: bool, place: u8 }

/// The mid-pack battle: six pilots race every cup course in every pod rotation until a 20 second window
/// holds, for one seat that is running second to fifth, a pair of slams, a spell in a deep wake and a
/// blowout in the pod beside it; that window is then run again and filmed from that seat, every tick.
/// `ANGEL_LAW_BATTLE=course,rot` skips the search for that race.
fn battle(dir: &std::path::Path) {
    const WINDOW: usize = 600;
    let start = |id: rules::CourseId, rot: u8| {
        let mut race = rules::Session::blank_on(id);
        race.join_ids(1..=6);
        for (n, c) in race.cars.iter_mut() { c.cpu = true; c.pod = ((n - 1) as u8 + rot) % 6; }
        race
    };
    let mut best: Option<(rules::CourseId, u8, u32, usize, i32)> = None;
    // `ANGEL_LAW_AT=course,rotation,seat,tick` films a battle already found (the tick is the one the search prints).
    if let Ok(at) = std::env::var("ANGEL_LAW_AT") {
        let v: Vec<&str> = at.split(',').collect();
        let id = rules::cup().iter().copied().find(|id| format!("{id:?}").to_lowercase() == v[0]).expect("a cup course");
        best = Some((id, v[1].parse().unwrap(), v[2].parse().unwrap(), v[3].parse::<usize>().unwrap() - rules::race::COUNTDOWN as usize, 0));
    }
    'search: for rot in 0..6u8 {
        if best.is_some() && std::env::var_os("ANGEL_LAW_AT").is_some() { break; }
        for &id in rules::cup() {
            // A named battle skips the other seventeen races.
            if let Ok(w) = std::env::var("ANGEL_LAW_BATTLE") {
                let v: Vec<&str> = w.split(',').collect();
                if v.len() == 2 && (format!("{id:?}").to_lowercase() != v[0] || rot.to_string() != v[1]) { continue; }
            }
            let mut race = start(id, rot);
            let mut beats: Vec<BTreeMap<u32, Beat>> = Vec::new();
            let mut prev: BTreeMap<u32, (u32, u32)> = race.cars.iter().map(|(n, c)| (*n, (c.slams, c.blowouts))).collect();
            for _ in 0..30_000 {
                let d = race.bot_drives();
                race.step_drive(&d);
                if race.finished { break; }
                let order = race.order();
                let mut now = BTreeMap::new();
                for (n, c) in &race.cars {
                    let (ps, pb) = prev[n];
                    let blow_near = race.cars.iter().any(|(o, oc)| { let (_, ob) = prev[o]; oc.blowouts > ob && (oc.d - c.d).abs() < 70. });
                    now.insert(*n, Beat { slam: c.slams > ps, draft: c.draft > 0.5, blow_near, place: order.iter().position(|x| x == n).unwrap() as u8 + 1 });
                    let _ = pb;
                }
                prev = race.cars.iter().map(|(n, c)| (*n, (c.slams, c.blowouts))).collect();
                beats.push(now);
            }
            if let Some(w) = std::env::var("ANGEL_LAW_BATTLE").ok() {
                let v: Vec<&str> = w.split(',').collect();
                if v.len() == 2 && (format!("{id:?}").to_lowercase() != v[0] || rot.to_string() != v[1]) { continue; }
            }
            for seat in 1..=6u32 {
                for from in (200..beats.len().saturating_sub(WINDOW)).step_by(30) {
                    let w = &beats[from..from + WINDOW];
                    let slams = w.iter().filter(|b| b[&seat].slam).count();
                    let draft = w.iter().filter(|b| b[&seat].draft).count();
                    let blow = w.iter().any(|b| b[&seat].blow_near);
                    let mid = w.iter().filter(|b| (2..=5).contains(&b[&seat].place)).count();
                    // All three things in the 20 seconds, the seat in the thick of the pack; the more of each the better.
                    if slams >= 1 && draft >= 30 && blow && mid >= WINDOW * 7 / 10 {
                        let score = (slams.min(4) * 60 + draft.min(200) + mid / 6) as i32;
                        if best.is_none_or(|b| score > b.4) { best = Some((id, rot, seat, from, score)); }
                        if std::env::var_os("ANGEL_LAW_BATTLE").is_some() { break 'search; }
                    }
                }
            }
        }
    }
    let (id, rot, seat, from, score) = best.expect("a 20 second mid-pack battle with slams, a draft and a blowout turns up in 18 races");
    println!("filming the battle of seat {seat} on {id:?} (rotation {rot}) from tick {} (score {score})", from + rules::race::COUNTDOWN as usize);
    let mut reel = Reel::on(id, seat);
    reel.race = start(id, rot);
    // The sim has been run `from` ticks past the first step of the race: the race clock, not the filming's.
    for _ in 0..from { let d = reel.race.bot_drives(); reel.step(&d); }
    for _ in 0..WINDOW {
        reel.shot(dir);
        let d = reel.race.bot_drives();
        reel.step(&d);
    }
}

/// The grid, the lamps one a second, the flag.
fn countdown(dir: &std::path::Path) {
    // From a seat in the back rows, and from the pole: the grid is seen from both.
    for seat in [4, 1] {
        let mut reel = Reel::on(rules::cup()[0], seat);
        let dir = dir.join(format!("seat{seat}"));
        for _ in 0..150 {
            reel.shot(&dir);
            let d = reel.pilots(&[]);
            reel.step(&d);
        }
    }
}

/// Full gas and boost down the straight, a scrape of the left wall: the engines heat, one burns,
/// the pilot holds on, it blows, the squires patch it, the pod goes again.
fn heat(dir: &std::path::Path) {
    let mut reel = Reel::on(rules::cup()[0], 1);
    reel.stage(&[(1, 300., -1., 42.), (2, 440., 3., 44.), (3, 520., -4., 44.), (4, 600., 1., 44.), (5, 250., 4., 44.), (6, 215., -3., 44.)]);
    prime_heat(&mut reel);
    for t in 0..620 {
        if t % 2 == 0 { reel.shot(dir); }
        let mut d = reel.pilots(&[1]);
        let c = &reel.race.cars[&1];
        // Gas and boost throughout; in the third second lean on the left wall and ride it.
        let wall = reel.race.course().frame(0, c.d).wl - 0.3;
        let to = if (75..170).contains(&t) { wall } else { 0. };
        d.insert(1, rules::Drive { gas: true, boost: t < 200, steer: hold_lane(c, to), ..Default::default() });
        reel.step(&d);
    }
}

/// The engines start warm.
fn prime_heat(reel: &mut Reel) {
    let c = reel.race.cars.get_mut(&1).unwrap();
    c.heat = [0.48, 0.42];
}

/// A pod shoulders another across the road.
fn slam(dir: &std::path::Path) {
    let mut reel = Reel::on(rules::cup()[0], 1);
    reel.stage(&[(1, 300., -3.5, 44.), (2, 301., 3.5, 44.), (3, 470., -4., 44.), (4, 520., 0., 44.), (5, 190., 5., 44.), (6, 150., -5., 44.)]);
    // The near engine starts warm, so the slam's heat shows on the gauge.
    reel.race.cars.get_mut(&1).unwrap().heat = [0.2, 0.5];
    for t in 0..150 {
        reel.shot(dir);
        let mut d = reel.pilots(&[1, 2]);
        let c1 = &reel.race.cars[&1];
        // Seat 2 is the brute: it dives across onto seat 1 and holds the line.
        d.insert(1, rules::Drive { gas: true, steer: if t < 45 { 0 } else { hold_lane(c1, -3.5) }, ..Default::default() });
        d.insert(2, rules::Drive { gas: true, steer: if (20..58).contains(&t) { -1 } else { 0 }, ..Default::default() });
        reel.step(&d);
    }
}

/// A pass the pilots make by themselves, through the draft: the sim is run as a six-CPU race until a
/// pod settles in front of another having ridden its wake in the seconds before, and the same race is
/// then run again and filmed from the passer's seat, from five seconds before to three after.
fn overtake(dir: &std::path::Path) {
    let id = rules::cup()[0];
    let start = |all_cpu: bool| {
        let mut race = rules::Session::blank_on(id);
        race.join_ids(1..=6);
        for c in race.cars.values_mut() { c.cpu = all_cpu; }
        race
    };
    // Find it.
    let mut race = start(true);
    let mut settled: std::collections::BTreeMap<(u32, u32), bool> = Default::default();
    let mut pending: std::collections::BTreeMap<(u32, u32), (bool, u32)> = Default::default();
    let mut recent: std::collections::BTreeMap<u32, Vec<(u32, f32)>> = Default::default();
    let mut found = None;
    for _ in 0..30_000 {
        let d = race.bot_drives();
        race.step_drive(&d);
        if race.ticks < 700 { continue; }
        for (n, c) in &race.cars { let r = recent.entry(*n).or_default(); r.push((race.ticks, c.draft)); if r.len() > 150 { r.remove(0); } }
        let order = race.order();
        for (i, &a) in order.iter().enumerate() {
            for &b in &order[i + 1..] {
                let (lo, hi) = (a.min(b), a.max(b));
                let lo_ahead = a == lo;
                let was = *settled.entry((lo, hi)).or_insert(lo_ahead);
                if lo_ahead == was { pending.remove(&(lo, hi)); continue; }
                let p = *pending.entry((lo, hi)).or_insert((lo_ahead, race.ticks));
                let p = if p.0 != lo_ahead { pending.insert((lo, hi), (lo_ahead, race.ticks)); (lo_ahead, race.ticks) } else { p };
                if race.ticks - p.1 >= 60 {
                    settled.insert((lo, hi), lo_ahead);
                    pending.remove(&(lo, hi));
                    let peak = recent[&a].iter().filter(|(t, _)| *t + 240 > p.1).map(|(_, d)| *d).fold(0., f32::max);
                    let (ca, cb) = (&race.cars[&a], &race.cars[&b]);
                    let straight = race.course().curve(ca.d.rem_euclid(race.course().lap())).abs() < 0.004;
                    if peak > 0.6 && ca.route == 0 && cb.route == 0 && straight && ca.crash == 0 && cb.crash == 0 && found.is_none() { found = Some((p.1, a)); }
                }
            }
        }
        if found.is_some() { break; }
    }
    let (at, passer) = found.expect("the pilots pass through a draft somewhere on the first course");
    println!("filming the pass of seat {passer}, which settled at tick {at}");
    // Film it.
    let mut reel = Reel::on(id, passer);
    reel.race = start(true);
    let from = at.saturating_sub(150 + 60);
    while reel.race.ticks < from { let d = reel.race.bot_drives(); reel.step(&d); }
    for _ in 0..330 {
        reel.shot(dir);
        let d = reel.race.bot_drives();
        reel.step(&d);
    }
}

/// `ANGEL_LAW_TIMES=1`: the cost of a frame at eight places round each cup course, every pod sound and
/// then every pod on fire, round after round so a busy machine slows both alike.
#[test]
fn law_frame_time() {
    if std::env::var_os("ANGEL_LAW_TIMES").is_none() { return; }
    for &id in rules::cup() {
        let mut run = Run::roundtable_for_test("knight_racers").unwrap();
        let mut means = [Vec::new(), Vec::new()];
        for round in 0..4 {
            let damaged = round % 2 == 1;
            let mut race = rules::Session::blank_on(id);
            race.join_ids(1..=6);
            race.ticks = rules::race::COUNTDOWN + 300;
            let lap = rules::course(id).lap();
            let mut medians = Vec::new();
            for k in 0..8 {
                let d = lap * k as f32 / 8. + 30.;
                for (slot, car) in race.cars.values_mut().enumerate() {
                    car.d = d - slot as f32 * 7.; car.y = if slot % 2 == 0 { -3.2 } else { 3.2 }; car.v = 40.;
                    if damaged { car.heat = [0.9, 0.55]; car.wear = [0.3, 0.1]; }
                }
                run.knight_racers = Some(race.clone());
                let _ = layers(&run, Some(1));
                let mut ms: Vec<f64> = (0..12).map(|_| { let t = std::time::Instant::now(); let _ = layers(&run, Some(1)); t.elapsed().as_secs_f64() * 1000. }).collect();
                ms.sort_by(f64::total_cmp);
                medians.push(ms[ms.len() / 2]);
            }
            means[usize::from(damaged)].push(medians.iter().sum::<f64>() / medians.len() as f64);
        }
        let m = |v: &Vec<f64>| v.iter().sum::<f64>() / v.len() as f64;
        println!("{:?}: sound {:.2} ms, every pod on fire {:.2} ms ({:+.1}%)", id, m(&means[0]), m(&means[1]), (m(&means[1]) / m(&means[0]) - 1.) * 100.);
    }
}
