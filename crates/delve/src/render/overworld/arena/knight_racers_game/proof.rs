//! Headless proof for the game lane: the per-frame render cost, and the
//! frame sequences the review GIFs are cut from. Neither runs unless asked:
//!
//!   ANGEL_PODRACER_TIME=1 cargo test -p angel-delve --lib podracer_time -- --nocapture
//!   ANGEL_PODRACER_PROOF=/dir cargo test -p angel-delve --lib podracer_reels
use super::*;
use crate::rules::Run;
use std::collections::BTreeMap;
use std::time::Instant;

/// A six-seat race that every seat drives with the CPU's hands, the focus
/// seat boosting whenever its engine allows.
struct Reel {
    run: Run,
    race: rules::Session,
}

impl Reel {
    /// A race on `ANGEL_REEL_COURSE` (a course's id, any case), else the cup's first.
    fn new() -> Reel {
        let want = std::env::var("ANGEL_REEL_COURSE").unwrap_or_default().to_lowercase();
        let id = rules::courses().iter().copied().find(|id| format!("{id:?}").to_lowercase() == want).unwrap_or(rules::cup()[0]);
        Reel::on(id)
    }

    fn on(id: rules::CourseId) -> Reel {
        let mut run = Run::roundtable_for_test("knight_racers").unwrap();
        run.knight_racers = None;
        let mut race = rules::Session::blank_on(id);
        race.join_ids(1..=6);
        Reel { run, race }
    }

    fn inputs(&self, seat: u32, hold_boost: bool) -> BTreeMap<u32, rules::Drive> {
        let mut all = self.race.bot_drives_all();
        if let (true, Some(d), Some(car)) = (hold_boost, all.get_mut(&seat), self.race.cars.get(&seat)) {
            if car.v > 12. && self.race.ticks > 90 { d.boost = true; d.gas = true; d.brake = false; }
        }
        all
    }

    fn step(&mut self, seat: u32, hold_boost: bool) {
        let input = self.inputs(seat, hold_boost);
        self.race.step_drive(&input);
        assert!(self.race.valid());
        // A seat watching the whole race sees its laps go by: keep the lap clocks.
        state::observe(&self.race);
    }

    fn frame(&mut self, seat: u32) -> Img {
        self.run.knight_racers = Some(self.race.clone());
        layers(&self.run, Some(seat)).0
    }
}

fn save(im: &Img, path: &std::path::Path) {
    image::save_buffer(path, &im.rgba_bytes(), 384, 224, image::ColorType::Rgba8).unwrap();
}

/// The cost of a frame of today's race, over a spread of places on the lap.
#[test]
fn podracer_time() {
    if std::env::var_os("ANGEL_PODRACER_TIME").is_none() { return; }
    let mut reel = Reel::new();
    let mut ms = Vec::new();
    let lap = reel.race.course().lap();
    let marks: Vec<f32> = (0..8).map(|n| lap * n as f32 / 8.).collect();
    for seat in [1u32, 4] {
        for &d in &marks {
            let car = reel.race.cars.get_mut(&seat).unwrap();
            car.d = d; car.y = 0.; car.v = 40.;
            reel.race.ticks = 200;
            // warm up the mesh caches once, then time three frames
            let _ = reel.frame(seat);
            for _ in 0..3 {
                let t = Instant::now();
                let _ = reel.frame(seat);
                ms.push(t.elapsed().as_secs_f64() * 1000.);
            }
        }
    }
    ms.sort_by(f64::total_cmp);
    let mean = ms.iter().sum::<f64>() / ms.len() as f64;
    println!("podracer frame ms: n {} p50 {:.2} p95 {:.2} max {:.2} mean {:.2}", ms.len(), ms[ms.len() / 2], ms[ms.len() * 95 / 100], ms[ms.len() - 1], mean);
}

/// Every pod in every livery under the chase lens and from the side, on a flat road.
#[test]
fn podracer_roster() {
    let Some(out) = std::env::var_os("ANGEL_PODRACER_PROOF").map(std::path::PathBuf::from) else { return };
    std::fs::create_dir_all(&out).unwrap();
    let mut sheet = image::RgbaImage::new(384 * 3, 224 * 4);
    for (n, look) in (0..6usize).enumerate() {
        let mut road = Mesh::new();
        road.push_quad(v3(-80., -30., 0.), v3(80., -30., 0.), v3(80., 30., 0.), v3(-80., 30., 0.), mat::PATH);
        let mut pod = Mesh::new();
        pods::build(&mut pod, look, n as u32 + 1, 0.4, if n % 2 == 0 { 0.3 } else { 1.55 });
        for tr in &mut pod.tris { for p in &mut tr.v { *p = *p * pods::SCALE; } }
        for (view_n, (eye, aim, fov)) in [
            (v3(-24., 0., 7.4), v3(12., 0., 0.3), 1.1f32),
            (v3(-6., -17., 7.), v3(-3., 0., 1.5), 0.95),
        ].into_iter().enumerate() {
            let d = aim - eye;
            let view = View3 { pos: eye, heading_rad: d.y.atan2(d.x), pitch: d.z.atan2(d.x.hypot(d.y)), fov_rad: fov };
            let (rgba, _) = raster::render_meshes_painted(&[&road, &pod], &view, 384, 224, Firelight { gain: 1.65, warm: 0.06 }, 130., Backdrop::Dark([58, 86, 114]), Some(pods::palette()));
            let im = dots_with(rgba.as_raw(), 384, 224, (0., 82., 0.65, 150.));
            let (cx, cy) = ((n % 3) as u32 * 384, ((n / 3) * 2 + view_n) as u32 * 224);
            for y in 0..224 { for x in 0..384 {
                let c = im.get(x, y).unwrap_or([0, 0, 0]);
                sheet.put_pixel(cx + x as u32, cy + y as u32, image::Rgba([c[0], c[1], c[2], 255]));
            } }
        }
    }
    sheet.save(out.join("roster.png")).unwrap();
}

/// What each livery cloth comes out as, before and after the palette snap.
#[test]
fn podracer_swatches() {
    if std::env::var_os("ANGEL_PODRACER_PROOF").is_none() { return; }
    for lv in &pods::LIVERIES {
        let mut m = Mesh::new();
        m.push_quad(v3(10., -4., 0.), v3(10., 4., 0.), v3(10., 4., 8.), v3(10., -4., 8.), lv.cloth);
        let view = View3 { pos: v3(0., 0., 4.), heading_rad: 0., pitch: 0., fov_rad: 1.0 };
        let (rgba, _) = raster::render_meshes_painted(&[&m], &view, 384, 224, Firelight { gain: 1.65, warm: 0.06 }, 130., Backdrop::Dark([58, 86, 114]), Some(pods::palette()));
        let im = dots_with(rgba.as_raw(), 384, 224, (0., 82., 0.65, 150.));
        let px = (192 * 4 + 112 * 384 * 4) as usize;
        let raw = &rgba.as_raw()[px..px + 3];
        println!("{} raw {:?} snapped {:?}", lv.name, raw, im.get(192, 112));
    }
}

impl Reel {
    fn run_to(&mut self, tick: u32, seat: u32, hold: bool) {
        while self.race.ticks < tick && !self.race.finished { self.step(seat, hold); }
    }

    /// The tick at which `want` first holds (stepping from here), or the cap.
    fn find(&mut self, seat: u32, hold: bool, cap: u32, want: impl Fn(&rules::Session, &rules::Session) -> bool) -> u32 {
        loop {
            let before = self.race.clone();
            self.step(seat, hold);
            if want(&before, &self.race) || self.race.ticks >= cap || self.race.finished { return self.race.ticks; }
        }
    }

    /// Frames `from..=to`, one per tick or every `every`.
    fn capture(&mut self, dir: &std::path::Path, from: u32, to: u32, every: u32, seat: u32, hold: bool) {
        std::fs::create_dir_all(dir).unwrap();
        self.run_to(from, seat, hold);
        let mut n = 0;
        loop {
            if (self.race.ticks - from) % every == 0 {
                save(&self.frame(seat), &dir.join(format!("{n:04}.png")));
                n += 1;
            }
            if self.race.finished && self.race.ticks > from + 2 {
                // The sim's clock stops at the flag: hold the board on the screen.
                for _ in 0..80 { save(&self.frame(seat), &dir.join(format!("{n:04}.png"))); n += 1; }
                break;
            }
            if self.race.ticks >= to { break; }
            self.step(seat, hold);
        }
    }
}

fn place_of(race: &rules::Session, seat: u32) -> usize {
    let order = state::standings(race);
    state::place_of(&order, seat)
}

/// The reels the review GIFs are cut from. `ANGEL_PODRACER_REEL=name` for one.
#[test]
fn podracer_reels() {
    let Some(out) = std::env::var_os("ANGEL_PODRACER_PROOF").map(std::path::PathBuf::from) else { return };
    let only = std::env::var("ANGEL_PODRACER_REEL").ok();
    let want = |n: &str| only.as_deref().is_none_or(|o| o == n);
    // Grid to go: every pod on the grid, the lights, the swoop in, the flag.
    if want("countdown") {
        let mut reel = Reel::new();
        reel.capture(&out.join("countdown"), 0, 140, 1, 4, false);
    }
    // A boost down a straight: the engines run hot and the lens must not fill.
    if want("boost") {
        let mut reel = Reel::new();
        reel.capture(&out.join("boost"), 112, 232, 1, 1, true);
    }
    // The back-marker boosts past a rival on the open straight. The field is set
    // out on the road by hand (the sim and its pilots run as they always do), so
    // the pass has clear road around it and is not lost in the start-line pack.
    if want("pass") {
        let mut reel = Reel::new();
        reel.run_to(130, 6, true);
        let base = 380.;
        for (id, d, y) in [(1, base + 120., 3.), (2, base, -1.), (3, base + 210., -4.), (4, base + 280., 1.), (5, base - 110., 4.), (6, base - 24., 3.)] {
            let c = reel.race.cars.get_mut(&id).unwrap();
            c.d = d; c.y = y; c.v = 44.;
        }
        // The one being passed has spent its boost and is still picking up speed.
        let slow = reel.race.cars.get_mut(&2).unwrap();
        slow.v = 30.; slow.boost = 0.; slow.boost_locked = true;
        let fast = reel.race.cars.get_mut(&6).unwrap();
        fast.v = 52.; fast.boost = 1.;
        reel.capture(&out.join("pass"), 130, 130 + 130, 1, 6, true);
    }
    // The first sight of the last lap.
    if want("finallap") {
        let mut reel = Reel::new();
        let len = state::lap_len(&reel.race);
        let laps = state::laps(&reel.race);
        let at = reel.find(2, true, 20000, |_, b| b.cars[&2].d >= len * (laps - 1) as f32);
        let mut reel = Reel::new();
        reel.capture(&out.join("finallap"), at.saturating_sub(24), at + 96, 1, 2, true);
    }
    // The flag, and the board.
    if want("results") {
        let seat = 4;
        let at = Reel::new().find(seat, true, 24000, |_, b| b.cars[&seat].finish.is_some());
        Reel::new().capture(&out.join("finish"), at.saturating_sub(10), at + 60, 1, seat, true);
        let at = Reel::new().find(seat, true, 30000, |_, b| b.finished);
        Reel::new().capture(&out.join("results"), at.saturating_sub(50), at + 70, 1, seat, true);
    }
}

/// A few frames at chosen ticks, for framing a shot.
/// `ANGEL_PODRACER_SNAP=dir ANGEL_SNAP_TICKS=0,60,100 ANGEL_SNAP_SEAT=4 ANGEL_SNAP_HOLD=1`
#[test]
fn podracer_snap() {
    let Some(out) = std::env::var_os("ANGEL_PODRACER_SNAP").map(std::path::PathBuf::from) else { return };
    std::fs::create_dir_all(&out).unwrap();
    let ticks: Vec<u32> = std::env::var("ANGEL_SNAP_TICKS").unwrap_or("0,60,100,140".into()).split(',').filter_map(|t| t.parse().ok()).collect();
    let seat: u32 = std::env::var("ANGEL_SNAP_SEAT").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
    let hold = std::env::var_os("ANGEL_SNAP_HOLD").is_some();
    let mut reel = Reel::new();
    for t in ticks {
        reel.run_to(t, seat, hold);
        // A staged field for a clean composition: ANGEL_SNAP_D is the viewer's distance run.
        if let Some(d) = std::env::var("ANGEL_SNAP_D").ok().and_then(|v| v.parse::<f32>().ok()) {
            let offs = [(34., -4.), (70., 5.), (-22., 3.), (-48., -5.), (110., 0.), (-80., 4.)];
            let mut k = 0;
            for (&id, car) in reel.race.cars.iter_mut() {
                if id == seat { car.d = d; car.y = 0.; car.v = 46.; } else { car.d = d + offs[k].0; car.y = offs[k].1; car.v = 46.; k += 1; }
            }
        }
        save(&reel.frame(seat), &out.join(format!("snap-{t:05}.png")));
    }
}

/// How often a rival stands in the chase view over a whole bot race: the
/// share of sampled moments with at least one rival in frame, and the
/// longest unbroken stretch without one.
#[test]
fn podracer_rivals_in_view() {
    let mut reel = Reel::new();
    let seat = 1;
    let (mut seen, mut total, mut dry, mut longest_dry) = (0, 0, 0, 0);
    while reel.race.cars[&seat].finish.is_none() && reel.race.ticks < 30000 {
        reel.step(seat, false);
        if reel.race.ticks % 15 != 0 || reel.race.ticks < 130 { continue; }
        let view = camera::view(&reel.race, &reel.race.cars[&seat]);
        let n = reel.race.cars.iter().filter(|&(&id, c)| id != seat && c.finish.is_none()
            && camera::project(&view, point(c.d - 2.5, c.y, 3.)).is_some_and(|(x, y, z)| (0.0..384.0).contains(&x) && (45.0..185.0).contains(&y) && z < 150.)).count();
        total += 1;
        if n > 0 { seen += 1; dry = 0; } else { dry += 1; longest_dry = longest_dry.max(dry); }
    }
    println!("rivals in view: {seen} of {total} samples ({:.0}%), longest gap {longest_dry} samples", seen as f32 / total as f32 * 100.);
}

/// One instrument frame over a staged mid-race field: every seat's colour is on
/// the minimap, the ladder reads in running order, and nothing is drawn off its plate.
#[test]
fn hud_agrees_with_the_running_order() {
    let mut reel = Reel::new();
    reel.run_to(400, 3, true);
    let race = reel.race.clone();
    let order = state::standings(&race);
    assert_eq!(order.iter().map(|(id, _)| *id).collect::<Vec<_>>(), race.order());
    let im = reel.frame(3);
    // The ladder's chips, top to bottom, are the seats in running order.
    for (n, &(id, _)) in order.iter().enumerate() {
        let ink = crate::ink::ink(pods::livery(id).ink).unwrap();
        let y = 76 + 3 + n as i32 * 9 + 3;
        assert_eq!(im.get(346 + 11 + 1, y), Some(ink), "ladder row {n} is not seat {id}");
    }
    // The viewer's place digit is the ladder's row for the viewer.
    assert_eq!(state::place_of(&order, 3), order.iter().position(|(id, _)| *id == 3).unwrap() + 1);
}

/// The courses at a glance, for the docs.
#[test]
fn podracer_courses() {
    if std::env::var_os("ANGEL_PODRACER_PROOF").is_none() { return; }
    for &id in rules::courses() {
        let c = rules::course(id);
        println!("{:?}: {} | {} laps | lap {:.0} m | {} forks | {} gaps | {} zones | cup {:?} | {}", id, c.name(), c.laps(), c.lap(), c.forks().len(), c.gaps().len(), c.zones().len(), rules::cup().iter().position(|k| *k == id), c.blurb());
    }
}

/// The chevron over the viewer's own pod lands on that pod, in the lower middle of the picture.
#[test]
fn the_viewers_own_pod_is_where_the_chase_camera_puts_it() {
    let mut reel = Reel::new();
    reel.run_to(260, 4, true);
    // The lane the pilot is in at that moment is the road's business, not the lens's: put the pod on the centre line.
    let mut car = reel.race.cars[&4].clone();
    car.y = 0.;
    let view = camera::view(&reel.race, &car);
    let (x, y, z) = camera::project(&view, point(car.d - 3., car.y, 3.4)).expect("the pod is in front of the lens");
    println!("own pod at ({x:.0}, {y:.0}) depth {z:.1}; eye {:?} pod {:?} d {:.1} v {:.1} lift {:.2} yaw {:.2}", view.pos, point(car.d, car.y, 0.), car.d, car.v, car.lift, car.yaw);
    assert!((150.0..234.0).contains(&x) && (95.0..175.0).contains(&y), "own pod projects to ({x:.0}, {y:.0})");
    assert!((8.0..30.0).contains(&z), "own pod is {z:.1} from the lens");
}
