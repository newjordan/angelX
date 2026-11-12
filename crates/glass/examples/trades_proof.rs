//! The Keep's work by hand, in the realm's own renderer (the Works,
//! `rules::hands`): a knight learning a trade from one of Bomrekedan's
//! teachers, cutting at the banker, carrying a stone up the scaffold, and
//! three knights setting a course together with the mark tally rising, flat
//! and in 3D, as the window draws a Delve stage, offscreen. Each scene steps
//! the real run with knights who play the games by the clock, as the tests'
//! bots do.
//!
//! ANGEL_REALM_PLAN=tabula cargo run --profile fast -p angel-glass --example trades_proof -- <outdir> [--scale N] <scene>
//!
//! Scenes: `learn`, `cut`, `carry`, `set`, `wheel`. Each writes `NNNN.png` (3D) and
//! `NNNN-flat.png` frames into `<outdir>/<scene>/`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_delve::render::overworld::arena::{self, stage_frame::StageMaker};
use angel_delve::rules::hands::{self, Carry, Work};
use angel_delve::rules::trades::{self, Trade};
use angel_delve::rules::{Input, Run};
use angel_delve::world::works;
use angel_delve::world3d::{self, kingdom};
use angel_gpu::{Core, Draw, Frame, Instance, Picture, Stage};

/// The stage's picture id, as the cockpit names the Delve's.
const STAGE: u32 = 0x00FA_57A4;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("trades_proof: {why}");
            ExitCode::FAILURE
        }
    }
}

fn feet(tile: (f32, f32)) -> [f32; 2] {
    [tile.0 * 16.0, tile.1 * 16.0 + 6.0]
}

/// A castle's point (metres) as a realm tile.
fn tile_of(p: [f32; 2]) -> (f32, f32) {
    (works::ORIGIN.0 as f32 + p[0] / 1.5, works::ORIGIN.1 as f32 - p[1] / 1.5)
}

fn castle() -> &'static works::Castle {
    works::castle(works::KEEP).expect("the great tower")
}

/// The map tile a point of the castle's frame (metres) lies in.
fn tile_at(p: [f32; 2]) -> (i32, i32) {
    (works::ORIGIN.0 + (p[0] / 1.5).floor() as i32, works::ORIGIN.1 + (-p[1] / 1.5).floor() as i32)
}

/// The middle of a tile, in the castle's frame.
fn middle_of(t: (i32, i32)) -> [f32; 2] {
    [((t.0 - works::ORIGIN.0) as f32 + 0.5) * 1.5, -((t.1 - works::ORIGIN.1) as f32 + 0.5) * 1.5]
}

/// Whether a knight can stand on tile `t` with `laid` stones up.
fn open(laid: u32, t: (i32, i32)) -> bool {
    let (x0, y0, w, h) = castle().tiles();
    t.0 >= x0 && t.1 >= y0 && t.0 < x0 + w && t.1 < y0 + h && !castle().blocked(t.0, t.1, laid)
}

/// Open ground within `reach` metres of `p`, the nearest to `toward`.
fn spot_near(laid: u32, p: [f32; 2], reach: f32, toward: [f32; 2]) -> [f32; 2] {
    spot_apart(laid, p, reach, toward, &[])
}

/// [`spot_near`], at least two metres from every spot in `apart`.
fn spot_apart(laid: u32, p: [f32; 2], reach: f32, toward: [f32; 2], apart: &[[f32; 2]]) -> [f32; 2] {
    let (tx, ty) = tile_at(p);
    let mut best: Option<([f32; 2], f32)> = None;
    for dy in -5..=5 {
        for dx in -5..=5 {
            let t = (tx + dx, ty + dy);
            let m = middle_of(t);
            let d = (m[0] - p[0]).hypot(m[1] - p[1]);
            if !open(laid, t) || d > reach || apart.iter().any(|a| (a[0] - m[0]).hypot(a[1] - m[1]) < 2.0) {
                continue;
            }
            let score = (m[0] - toward[0]).hypot(m[1] - toward[1]);
            if best.is_none_or(|(_, s)| score < s) {
                best = Some((m, score));
            }
        }
    }
    best.map_or(p, |(m, _)| m)
}

/// A way on foot from `a` to `b`: the middles of open tiles, found breadth first.
fn path(laid: u32, a: [f32; 2], b: [f32; 2]) -> Vec<[f32; 2]> {
    use std::collections::{HashMap, VecDeque};
    let (from, to) = (tile_at(a), tile_at(b));
    let mut came: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    let mut queue = VecDeque::from([from]);
    came.insert(from, from);
    while let Some(t) = queue.pop_front() {
        if t == to {
            break;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (t.0 + dx, t.1 + dy);
            if !came.contains_key(&n) && open(laid, n) {
                came.insert(n, t);
                queue.push_back(n);
            }
        }
    }
    let mut out = vec![b];
    let mut at = to;
    while at != from && came.contains_key(&at) {
        out.push(middle_of(at));
        at = came[&at];
    }
    out.reverse();
    out
}

/// A knight who walks a path to its end, a waypoint at a time (`at` is the
/// waypoint they are on); true when they are there.
fn walk(run: &Run, id: u32, way: &[[f32; 2]], at: &mut usize) -> (Input, bool) {
    while *at < way.len() {
        let last = *at + 1 == way.len();
        let (keys, there) = steer(run, id, way[*at], if last { 0.4 } else { 0.5 });
        if !there {
            return (keys, false);
        }
        *at += 1;
    }
    (Input::default(), true)
}

/// A run with knights at `at` (castle metres), the Keep `laid` stones up, the
/// bays raised well ahead so the masons are not kept waiting.
fn site_run(at: &[[f32; 2]], laid: u32) -> Run {
    let feets: Vec<[f32; 2]> = at.iter().map(|&p| feet(tile_of(p))).collect();
    let mut run = kingdom::proof_run(&feets);
    run.lay_to("great-tower", laid);
    run.hands.yard.timber = 4_000;
    for h in run.players.values_mut() {
        h.stone = false;
        h.idle = false;
    }
    run
}

/// The keys of a knight walking to `to` (castle metres); true when they are there.
fn steer(run: &Run, id: u32, to: [f32; 2], stop: f32) -> (Input, bool) {
    let h = &run.players[&id];
    let (tx, ty) = hands::units(to);
    let (dx, dy) = (tx - h.x, ty - h.y);
    if dx.hypot(dy) <= stop {
        return (Input::default(), true);
    }
    let sign = |d: f32| if d > 0.35 { 1 } else if d < -0.35 { -1 } else { 0 };
    (Input { move_x: sign(dx), move_y: sign(dy), ..Default::default() }, false)
}

/// What a setter who knows the timing does: `tap_off` ticks late on the plumb
/// and the level (0 is dead on the mark, a few more is merely good, a lot is
/// a miss).
fn setter_keys(run: &Run, id: u32, tap_off: (u64, u64)) -> Input {
    let now = run.tick + 1;
    let press = match run.hands.seats.get(&id).and_then(|h| h.work) {
        None => run.tick % 2 == 0,
        Some(Work::Set { began, plumb: None, .. }) => {
            let since = now - began;
            since >= hands::FLINCH && hands::plumb_dev(since.saturating_sub(tap_off.0)) == 0 && since >= 18 + tap_off.0
        }
        Some(Work::Set { tapped, plumb: Some(_), lands: 0, .. }) => {
            let since = now - tapped;
            since >= 22 + tap_off.1 && hands::level_dev(since - tap_off.1) <= 1 && since < 22 + tap_off.1 + 2
        }
        _ => false,
    };
    Input { swing: press, ..Default::default() }
}

/// A knight striking every perfect beat of what they work at.
fn strike_keys(run: &Run, id: u32) -> Input {
    let now = run.tick + 1;
    let press = match run.hands.seats.get(&id).and_then(|h| h.work) {
        None => run.tick % 2 == 0,
        Some(Work::Strike { job, began, .. }) => now > began && hands::beat_dev(job, now - began) == 0,
        _ => false,
    };
    Input { swing: press, ..Default::default() }
}

fn usage() -> String {
    "usage: trades_proof <outdir> [--scale N] (learn | cut | carry | set | wheel)".into()
}

/// Frames of a scene: a frame every `every` ticks, flat and in 3D, into `dir`.
struct Reel<'a> {
    gpu: &'a mut Gpu,
    dir: PathBuf,
    scale: u32,
    every: u64,
    n: u32,
    ticks: u64,
}

impl Reel<'_> {
    fn tick(&mut self, run: &mut Run, keys: &[(u32, Input)]) -> Result<(), String> {
        let inputs: BTreeMap<u32, Input> = keys.iter().copied().collect();
        run.step(&inputs);
        self.ticks += 1;
        if self.ticks % self.every == 0 {
            for on in [true, false] {
                frame_png(self.gpu, &self.dir, &format!("{:04}", self.n), run, on, self.scale, false)?;
            }
            self.n += 1;
        }
        Ok(())
    }
}

fn run() -> Result<(), String> {
    let mut words = std::env::args().skip(1);
    let mut out = None;
    let mut scale = 3u32;
    let mut rest = Vec::new();
    while let Some(w) = words.next() {
        match w.as_str() {
            "--scale" => scale = words.next().and_then(|v| v.parse().ok()).filter(|s| (1..=8).contains(s)).ok_or("--scale takes 1 to 8")?,
            _ if out.is_none() => out = Some(PathBuf::from(w)),
            _ => rest.push(w),
        }
    }
    let out = out.ok_or_else(usage)?;
    let scene = rest.first().cloned().ok_or_else(usage)?;
    let dir = out.join(&scene);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    if !works::on_map() {
        return Err("needs the blank-slate plan: ANGEL_REALM_PLAN=tabula".into());
    }
    let mut gpu = Gpu::new()?;
    arena::draw_parts_outside(true);
    arena::props_meshes(true);
    let mut reel = Reel { gpu: &mut gpu, dir, scale, every: 2, n: 0, ticks: 0 };
    // A frame every few ticks: the slower scenes are shown at a coarser step.
    reel.every = match scene.as_str() {
        "set" | "wheel" => 4,
        _ => 3,
    };
    match scene.as_str() {
        "learn" => learn(&mut reel),
        "cut" => cut(&mut reel),
        "carry" => carry(&mut reel),
        "set" => set(&mut reel),
        "wheel" => wheel(&mut reel),
        _ => Err(usage()),
    }?;
    println!("{scene}: {} frames", reel.n);
    Ok(())
}

/// A knight walks to a teacher of Bomrekedan, is offered the setter's trade,
/// holds E for the lesson and has the mallet in hand.
fn learn(reel: &mut Reel) -> Result<(), String> {
    let school = trades::SCHOOLS.iter().position(|s| s.power == "bomrekedan:ratfolk").expect("a school");
    let teacher = trades::SCHOOLS[school].at;
    let mut run = site_run(&[[1.5, -15.5]], 1_500);
    for _ in 0..6 {
        reel.tick(&mut run, &[])?;
    }
    // Walk to the teacher's side.
    let stand = [teacher[0], teacher[1] + 1.1];
    for _ in 0..400 {
        let (keys, there) = steer(&run, 1, stand, 0.6);
        reel.tick(&mut run, &[(1, keys)])?;
        if there {
            break;
        }
    }
    // Wait for the trade they want to be offered, then ask for it.
    let wanted = Trade::Setter;
    for _ in 0..400 {
        let key = run.trade_key(1).expect("a key");
        let reading = run.trades();
        let offered = hands::offer(school, run.tick, |t| reading.holds(&key, t));
        // At the start of its turn, so the lesson is not cut short.
        if offered == Some(wanted) && run.tick % trades::OFFER_TICKS == 0 {
            break;
        }
        reel.tick(&mut run, &[])?;
    }
    let held = Input { interact: true, ..Default::default() };
    for _ in 0..trades::LEARN_HOLD + 4 {
        reel.tick(&mut run, &[(1, held)])?;
    }
    for _ in 0..70 {
        reel.tick(&mut run, &[])?;
    }
    if run.hands.seats[&1].trade != Some(wanted) {
        return Err("the knight did not take up the trade".into());
    }
    Ok(())
}

/// A banker mason cuts rough blocks at the bench, blow on the beat.
fn cut(reel: &mut Reel) -> Result<(), String> {
    let bench = angel_delve::world::works_life::BANKERS[1];
    let mut run = site_run(&[[bench[0] + 0.4, bench[1] + 3.0]], 1_837);
    run.teach_trade(1, Trade::BankerMason, 12);
    run.home.stockyard.stone.insert("limestone".into(), 6);
    run.hands.yard.cut = 0;
    for _ in 0..6 {
        reel.tick(&mut run, &[])?;
    }
    for _ in 0..200 {
        let (keys, there) = steer(&run, 1, [bench[0], bench[1] + 1.15], 0.35);
        reel.tick(&mut run, &[(1, keys)])?;
        if there {
            break;
        }
    }
    let cut_before = run.hands.yard.cut;
    for _ in 0..330 {
        let keys = strike_keys(&run, 1);
        reel.tick(&mut run, &[(1, keys)])?;
    }
    if run.hands.yard.cut <= cut_before {
        return Err("no stone was cut".into());
    }
    Ok(())
}

/// Knight 1 walks (a way on open ground) to `to`, the hired hands at rest.
fn walk_to(reel: &mut Reel, run: &mut Run, to: [f32; 2], within: u32) -> Result<(), String> {
    let laid = run.sites.keep().expect("a site").laid;
    let way = path(laid, metres_of(run, 1), to);
    if way.len() < 2 && (metres_of(run, 1)[0] - to[0]).hypot(metres_of(run, 1)[1] - to[1]) > 1.0 {
        return Err(format!("no way on foot from {:?} to {to:?}", metres_of(run, 1)));
    }
    let mut at = 0;
    for _ in 0..within {
        run.hands.yard.acc = [0; 8];
        let (keys, there) = walk(run, 1, &way, &mut at);
        reel.tick(run, &[(1, keys)])?;
        if there {
            return Ok(());
        }
    }
    Err(format!("the knight did not come to {to:?}: at {:?}", metres_of(run, 1)))
}

/// A hodman takes a stone off the yard's heap, carries it slowly to the wall,
/// climbs the scaffold with it and sets it down; then a load of mortar from
/// the tub; then another stone.
fn carry(reel: &mut Reel) -> Result<(), String> {
    // A course in the open before the gate: the stones before it are inside the walls.
    let laid = 1_843;
    let wall = hands::stone_at(castle(), laid);
    let yard = angel_delve::world::works_life::YARD;
    // Open ground by the heap and by the tub, and the nearest open ground to the wall.
    let pick = spot_near(laid, yard, 2.2, [yard[0] - 3.0, yard[1] - 6.0]);
    let tub = spot_near(laid, hands::TUB, 1.8, [hands::TUB[0] - 2.0, hands::TUB[1] + 2.0]);
    let drop = spot_near(laid, wall, 3.8, pick);
    let start = spot_near(laid, yard, 5.0, [yard[0] - 4.0, yard[1] - 7.0]);
    let mut run = site_run(&[start], laid);
    run.teach_trade(1, Trade::Labourer, 12);
    run.hands.yard.cut = 6;
    run.hands.yard.tub = hands::TUB_MAX;
    run.hands.yard.course = 0;
    for _ in 0..6 {
        run.hands.yard.acc = [0; 8];
        reel.tick(&mut run, &[])?;
    }
    for (what, from) in [(Carry::Stone, pick), (Carry::Mortar, tub), (Carry::Stone, pick)] {
        walk_to(reel, &mut run, from, 600)?;
        // E at the heap or the tub: a load on the hod.
        reel.tick(&mut run, &[(1, Input { interact: true, ..Default::default() })])?;
        for _ in 0..8 {
            run.hands.yard.acc = [0; 8];
            reel.tick(&mut run, &[])?;
        }
        if run.hands.seats[&1].carry != what {
            return Err(format!("no {what:?} on the hod (the knight {:?}, said {:?})", metres_of(&run, 1), run.hands.seats[&1].said));
        }
        // The walk: slow, under the weight; the climb; the load set down.
        walk_to(reel, &mut run, drop, 1_200)?;
        for _ in 0..hands::CLIMB + 30 {
            run.hands.yard.acc = [0; 8];
            reel.tick(&mut run, &[(1, Input { interact: true, ..Default::default() })])?;
            if run.hands.seats[&1].carry == Carry::Nothing {
                break;
            }
        }
        for _ in 0..12 {
            run.hands.yard.acc = [0; 8];
            reel.tick(&mut run, &[])?;
        }
        if run.hands.seats[&1].carry != Carry::Nothing {
            return Err(format!("the {what:?} was not set down at the wall (the knight {:?}, said {:?})", metres_of(&run, 1), run.hands.seats[&1].said));
        }
    }
    let y = &run.hands.yard;
    println!("carry: {} stones at the wall, {} dabs of mortar, the hodman's deeds {}", y.course, y.mortar, run.deeds_of(1, Trade::Labourer));
    Ok(())
}

/// Where knight `id` stands, in the castle's frame.
fn metres_of(run: &Run, id: u32) -> [f32; 2] {
    let h = &run.players[&id];
    hands::metres((h.x, h.y))
}

/// Three knights set a course together: each stone one of theirs, the tally
/// rising.
fn set(reel: &mut Reel) -> Result<(), String> {
    let laid = 1_837;
    let c = castle();
    let mut spots: Vec<[f32; 2]> = Vec::new();
    for k in 0..3u32 {
        let stone = hands::stone_at(c, laid + k);
        let spot = spot_apart(laid, stone, 4.2, [stone[0] + (k as f32 - 1.0) * 4.0, stone[1] - 6.0], &spots);
        spots.push(spot);
    }
    let mut run = site_run(&spots, laid);
    for id in 1..=3 {
        run.teach_trade(id, Trade::Setter, if id == 1 { 34 } else { 3 });
    }
    run.hands.yard.course = 12;
    run.hands.yard.mortar = 30;
    for _ in 0..8 {
        reel.tick(&mut run, &[])?;
    }
    let start = run.sites.keep().expect("a site").laid;
    for _ in 0..900 {
        let laid_now = run.sites.keep().expect("a site").laid;
        // The hods keep the course in stone and the tub in mortar, off the film.
        run.hands.yard.course = run.hands.yard.course.max(8);
        run.hands.yard.mortar = run.hands.yard.mortar.max(12);
        run.hands.yard.acc = [0; 8];
        let mut keys = Vec::new();
        for id in 1..=3u32 {
            let stone = hands::stone_at(c, laid_now + id - 1);
            let here = metres_of(&run, id);
            let in_reach = (here[0] - stone[0]).hypot(here[1] - stone[1]) <= hands::REACH_WALL - 0.4;
            if in_reach || run.hands.seats[&id].work.is_some() {
                // Hands at the course: the play is the timing; knight 1 is
                // dead on, 2 a little loose, 3 now and then wide of it.
                let off = match (id, laid_now % 5) {
                    (1, _) => (0, 0),
                    (2, _) => (2, 1),
                    (_, 0) => (8, 10),
                    _ => (2, 2),
                };
                keys.push((id, setter_keys(&run, id, off)));
            } else {
                // Walk along the course to where the next stone can be reached.
                let apart: Vec<[f32; 2]> = (1..=3u32).filter(|&o| o != id).map(|o| metres_of(&run, o)).collect();
                let spot = spot_apart(laid_now, stone, 4.2, here, &apart);
                let way = path(laid_now, here, spot);
                let (walking, _) = steer(&run, id, *way.get(1).unwrap_or(&spot), 0.5);
                keys.push((id, walking));
            }
        }
        reel.tick(&mut run, &keys)?;
        if run.sites.keep().expect("a site").laid >= start + 16 {
            break;
        }
    }
    for _ in 0..60 {
        reel.tick(&mut run, &[])?;
    }
    let site = run.sites.keep().expect("a site");
    println!("set: {} stones, {} true, tally {:?}", site.laid - start, site.true_count(), site.tally());
    if site.laid < start + 6 {
        return Err("too few stones were set".into());
    }
    Ok(())
}

/// A hodman walks the treadwheel and the crane brings a stone round.
fn wheel(reel: &mut Reel) -> Result<(), String> {
    let laid = 1_500;
    let c = castle();
    let stand = spot_near(laid, hands::wheel_stand(c, laid).ok_or("no crane stands")?, 1.5, [-14.0, 3.0]);
    let mut run = site_run(&[spot_near(laid, stand, 4.0, [stand[0] - 4.0, stand[1] - 3.0])], laid);
    run.teach_trade(1, Trade::Labourer, 12);
    run.hands.yard.cut = 3;
    for _ in 0..6 {
        reel.tick(&mut run, &[])?;
    }
    for _ in 0..200 {
        let (keys, there) = steer(&run, 1, stand, 0.35);
        reel.tick(&mut run, &[(1, keys)])?;
        if there {
            break;
        }
    }
    for _ in 0..360 {
        reel.tick(&mut run, &[(1, Input { interact: true, ..Default::default() })])?;
    }
    Ok(())
}

/// A frame of `run` at the stage's size, flat (`on` false) or in 3D, to `<name>.png`.
fn frame_png(gpu: &mut Gpu, out: &Path, name: &str, run: &angel_delve::rules::Run, on: bool, scale: u32, held: bool) -> Result<(), String> {
    let (w, h) = arena::seat_size();
    world3d::set_enabled(on);
    let map = arena::MapView { full: held, ..Default::default() };
    let scene = Arc::new(arena::seat_scene_with(run, w as i32, h as i32, Some(1), &map));
    let mut maker = StageMaker::default();
    let mut frame = maker.make(&scene, |_| None);
    gpu.upload(std::mem::take(&mut frame.pictures));
    frame = maker.make(&scene, |_| None);
    let back = (frame.size.0 / 384).max(1);
        let size = (frame.size.0 * scale / back, frame.size.1 * scale / back);
    let pixels = gpu.stage(frame, size, true);
    write_png(&out.join(format!("{name}{}.png", if on { "" } else { "-flat" })), size, &pixels)?;
    world3d::set_enabled(false);
    Ok(())
}

struct Gpu {
    core: Core,
    device: wgpu::Device,
    target: Option<((u32, u32), wgpu::Texture)>,
}

impl Gpu {
    fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU adapter: {e}"))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("delve-kingdom"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;
        device.on_uncaptured_error(Arc::new(|error: wgpu::Error| eprintln!("GPU error: {error}")));
        let core = Core::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
        Ok(Self { core, device, target: None })
    }

    fn upload(&mut self, pictures: Vec<angel_stage::Picture>) {
        for p in pictures {
            self.core.upload(Picture { id: p.id, width: p.width, height: p.height, rgba: p.rgba, cols: 0, rows: 0 });
        }
    }

    /// A Delve frame as a stage, drawn into a texture `size` pixels as the
    /// window draws a stage in its cells, on black, and (when `read`) read
    /// back; else waited for.
    fn stage(&mut self, mut frame: angel_stage::Frame, size: (u32, u32), read: bool) -> Vec<u8> {
        self.upload(std::mem::take(&mut frame.pictures));
        self.core.set_stage(
            STAGE,
            Some(Stage {
                id: STAGE,
                size: frame.size,
                cols: size.0 as u16,
                rows: size.1 as u16,
                layers: frame.layers,
                scene: frame.scene,
                enter: None,
                tilt: frame.tilt,
                props: frame.props,
                impact: frame.impact,
            }),
        );
        if self.target.as_ref().is_none_or(|(s, _)| *s != size) {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("delve-kingdom-target"),
                size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            self.target = Some((size, texture));
        }
        let texture = &self.target.as_ref().expect("made above").1;
        let view = texture.create_view(&Default::default());
        self.core.begin_frame();
        self.core.render_stages((1, 1), 0.0);
        let shown = Instance {
            rect: [0.0, 0.0, size.0 as f32, size.1 as f32],
            uv: [0.0, 0.0, size.0 as f32, size.1 as f32],
            color: [1.0; 4],
            kind: angel_gpu::PICTURE,
            ..bytemuck::Zeroable::zeroed()
        };
        let mut parts = vec![(Draw::Picture { id: STAGE, linear: false }, vec![shown])];
        let mut drawn = Frame::default();
        drawn.assemble_into(&mut parts, &mut Vec::new());
        self.core.draw(&view, size, wgpu::Color::BLACK, &drawn, "delve-kingdom");
        if read {
            return self.core.read_back(texture, size);
        }
        let _ = self.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(5)) });
        Vec::new()
    }
}

fn write_png(path: &Path, size: (u32, u32), rgba: &[u8]) -> Result<(), String> {
    let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let file = std::fs::File::create(path).map_err(|e| fail(&e))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size.0, size.1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| fail(&e))?;
    writer.write_image_data(rgba).map_err(|e| fail(&e))
}
