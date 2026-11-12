//! The feel of building, in the realm's own renderer: ghost plans a row at a time and the
//! hands that raise them, a building picked up and a copied group stamped, the card of a
//! starving bench, the production graph and the list of what chokes the works. Every key is a
//! real input to `Run::step`; every frame is drawn by the game's own renderer offscreen
//! (flat `NNNN-flat.png`; with `--3d` also `NNNN.png`).
//!
//! ANGEL_REALM_PLAN=tabula cargo run --profile fast -p angel-glass --example ux_proof -- <outdir> [--scale N] <scene>
//!
//! Scenes: `row`, `stamp`, `card`, `graph`, `chokes`. Frames go in `<outdir>/<scene>/`;
//! `--dry` runs the script without drawing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_delve::kingdom::{Claim, Command, catalog};
use angel_delve::render::overworld::arena::{self, stage_frame::StageMaker};
use angel_delve::rules::builder_input::BuilderAction;
use angel_delve::rules::hillside::{Tool, tools};
use angel_delve::rules::{Input, Run, TILE_UNITS};
use angel_delve::world3d;
use angel_gpu::{Core, Draw, Frame, Instance, Picture, Stage};

const STAGE: u32 = 0x00FA_57A4;

/// What each 3D frame cost the game before the GPU had it: the scene, then the stage maker.
static TIMES: std::sync::Mutex<Vec<(u64, u64)>> = std::sync::Mutex::new(Vec::new());


fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("ux_proof: {why}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> String {
    "usage: ux_proof <outdir> [--scale N] [--dry] [--3d] (row | stamp | card | graph | chokes | gate | goals)".into()
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out: PathBuf = args.next().ok_or_else(usage)?.into();
    let (mut scale, mut dry, mut three, mut scene) = (3u32, false, false, None);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--scale" => scale = args.next().and_then(|v| v.parse().ok()).ok_or_else(usage)?,
            "--dry" => dry = true,
            "--3d" => three = true,
            s if scene.is_none() => scene = Some(s.to_string()),
            _ => return Err(usage()),
        }
    }
    let scene = scene.ok_or_else(usage)?;
    // A window that draws scenes (the kingdom's 3D is only for one).
    arena::props_meshes(true);
    if !angel_delve::world::plan::tabula() {
        return Err("needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula".into());
    }
    let dir = out.join(&scene);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let gpu = if dry { None } else { Some(Gpu::new()?) };
    if !dry {
        // A window that draws scenes: the kingdom's 3D is only for one.
        arena::draw_parts_outside(true);
        arena::props_meshes(true);
    }
    let mut reel = Reel { gpu, dir, scale, every: 20, n: 0, ticks: 0, size: (0, 0), three };
    match scene.as_str() {
        "row" => row(&mut reel),
        "stamp" => stamp(&mut reel),
        "card" => card(&mut reel),
        "graph" => graph(&mut reel),
        "chokes" => chokes(&mut reel),
        "gate" => gate(&mut reel),
        "goals" => goals(&mut reel),
        _ => Err(usage()),
    }
}

/// Frames of a reel: a frame every `every` ticks, 3D and flat.
struct Reel {
    gpu: Option<Gpu>,
    dir: PathBuf,
    scale: u32,
    every: u64,
    n: u32,
    ticks: u64,
    size: (u32, u32),
    three: bool,
}

impl Reel {
    fn tick(&mut self, run: &mut Run, keys: Input) -> Result<(), String> {
        run.step(&BTreeMap::from([(1, keys)]));
        self.ticks += 1;
        if self.ticks % self.every == 0 {
            self.shoot(run)?;
        }
        Ok(())
    }

    fn shoot(&mut self, run: &Run) -> Result<(), String> {
        if let Some(gpu) = self.gpu.as_mut() {
            let name = format!("{:04}", self.n);
            if self.three {
                frame_png(gpu, &self.dir, &name, run, true, self.scale, false)?;
            }
            self.size = frame_png(gpu, &self.dir, &name, run, false, self.scale, false)?;
        }
        self.n += 1;
        Ok(())
    }

    fn wait(&mut self, run: &mut Run, seconds: u64) -> Result<(), String> {
        for _ in 0..seconds * 30 {
            self.tick(run, Input::default())?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- the pilot ----

fn tile_of(run: &Run) -> (i16, i16) {
    let h = &run.players[&1];
    ((h.x / TILE_UNITS) as i16, (h.y / TILE_UNITS) as i16)
}

/// Walk to the middle of `to`, holding `extra` besides the walking.
fn go(reel: &mut Reel, run: &mut Run, to: (i16, i16), extra: Input) -> Result<(), String> {
    let (tx, ty) = ((f32::from(to.0) + 0.5) * TILE_UNITS, (f32::from(to.1) + 0.5) * TILE_UNITS);
    for _ in 0..1500 {
        let h = &run.players[&1];
        let (dx, dy) = (tx - h.x, ty - h.y);
        if dx.abs() < 0.3 && dy.abs() < 0.3 {
            return Ok(());
        }
        // One axis at a time, the longer first, so the knight walks straight lines.
        let (mx, my) = if dx.abs() >= dy.abs() { (dx.signum() as i8, 0) } else { (0, dy.signum() as i8) };
        reel.tick(run, Input { move_x: mx, move_y: my, ..extra })?;
    }
    Err(format!("the knight did not reach {to:?}, stands at {:?}", tile_of(run)))
}

fn press(reel: &mut Reel, run: &mut Run, input: Input) -> Result<(), String> {
    reel.tick(run, input)?;
    reel.tick(run, Input::default())
}

fn tool(run: &mut Run, tool: Tool) {
    let i = tools().iter().position(|t| *t == tool).expect("a tool");
    let b = run.kingdom.industry.get_mut(&1).expect("a board");
    b.plan = i;
    b.rig.keep(i);
}

/// Hold a key `n` ticks with the knight standing.
fn hold(reel: &mut Reel, run: &mut Run, n: usize, input: Input) -> Result<(), String> {
    for _ in 0..n {
        reel.tick(run, input)?;
    }
    Ok(())
}

/// The mouse: a pointer action as the page sends it.
fn mouse(reel: &mut Reel, run: &mut Run, a: BuilderAction) -> Result<(), String> {
    reel.tick(run, Input { builder: Some(a), ..Default::default() })
}

/// The builder comes out.
fn builder(reel: &mut Reel, run: &mut Run) -> Result<(), String> {
    press(reel, run, Input { cast: 4, ..Default::default() })
}

/// A beat of frames with nothing pressed.
fn beat(reel: &mut Reel, run: &mut Run, ticks: usize) -> Result<(), String> {
    hold(reel, run, ticks, Input::default())
}

/// The camp's works, as the `works` proof raised them: a quarry on a won seam, a kiln, a banker,
/// the carts, a woodsman, a sawpit, a well, a cottage, a mortar mill.
fn the_works(run: &mut Run) -> Result<(), String> {
    let c = catalog();
    win_seam(run);
    let yard = raise(run, "carters-yard", (143, 98))?;
    let quarry = raise(run, "claim-quarry", (160, 101))?;
    let kiln = raise(run, "lime-kiln", (153, 100))?;
    let banker = raise(run, "banker", (153, 104))?;
    let wood = raise(run, "woodsman-camp", (143, 103))?;
    let saw = raise(run, "sawpit", (148, 103))?;
    raise(run, "well", (150, 97))?;
    raise(run, "cottage", (137, 99))?;
    raise(run, "mortar-mill", (157, 97))?;
    let k = &mut run.home.industry;
    k.apply(c, Command::Link { building: quarry, claim: "mines-3".into() })?;
    k.apply(c, Command::Recipe { building: banker, recipe: "dress-limestone".into() })?;
    for id in [yard, quarry, kiln, banker, wood, saw] {
        k.apply(c, Command::Connect(id))?;
    }
    k.apply(c, Command::Carts { yard, carts: 4 })?;
    Ok(())
}

// ----------------------------------------------------------------- the world ----

/// A hillside run on open ground, rich, the Great Shaft a ley lift (the proof is of the
/// lines, not of how slowly three blocks a minute come up).
fn hill(x: f32, y: f32) -> Run {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(None);
    run.home.industry.found_hillside(catalog(), 7);
    let k = &mut run.home.industry;
    for r in catalog().resources.keys() {
        k.stock.insert(r.clone(), 600);
    }
    k.stock.insert("fodder".into(), 5_000);
    k.researched = catalog().all_techs().map(|(k, _)| k.clone()).collect();
    if let Some(b) = k.buildings.values_mut().find(|b| b.plan == "shaft-hand") {
        b.plan = "shaft-ley".into();
    }
    let h = run.players.get_mut(&1).expect("a knight");
    (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
    h.invulnerable = 1_000_000;
    h.stone = false;
    for _ in 0..4 {
        run.step(&BTreeMap::from([(1, Input::default())]));
    }
    run
}

fn raise(run: &mut Run, plan: &str, tile: (i16, i16)) -> Result<u32, String> {
    let k = &mut run.home.industry;
    k.apply(catalog(), Command::Build { plan: plan.into(), tile }).map_err(|e| format!("{plan} at {tile:?}: {e}"))?;
    let id = k.at(tile).ok_or("it did not stand")?;
    k.buildings.get_mut(&id).ok_or("it did not stand")?.construction = 0;
    Ok(id)
}

fn win_seam(run: &mut Run) {
    let c = catalog();
    let claim = Claim { resource: "limestone".into(), name: "Sound limestone face".into(), source: "The Mines / floor 3".into(), secured: false, rate: 4, yields: [("limestone".to_string(), 4), ("iron-ore".to_string(), 1)].into(), surface: false };
    run.home.industry.discover(c, "mines-3".into(), claim);
    run.home.industry.secure("mines-3");
}

// ------------------------------------------------------------------ the scenes ----

/// Ghost plans a row at a time: the builder out, planning on, E held while walking east lays a row
/// of cottages as plans; the camp's hands walk out carrying the materials and raise them, the
/// urgent one first; one plan is made urgent and another cancelled with the mouse and the keys.
fn row(reel: &mut Reel) -> Result<(), String> {
    let mut run = hill(144.5, 99.5);
    reel.every = 3;
    beat(reel, &mut run, 6)?;
    builder(reel, &mut run)?;
    tool(&mut run, Tool::Plan("cottage"));
    // The comma key: lay plans, not buildings.
    press(reel, &mut run, Input { skill: 2, ..Default::default() })?;
    beat(reel, &mut run, 6)?;
    // Face east, then hold E and walk: a row of plans behind the knight.
    hold(reel, &mut run, 3, Input { move_x: 1, ..Default::default() })?;
    hold(reel, &mut run, 90, Input { move_x: 1, interact: true, ..Default::default() })?;
    beat(reel, &mut run, 10)?;
    // Back along the row, so the camp and the whole row are in the view.
    go(reel, &mut run, (151, 99), Input::default())?;
    beat(reel, &mut run, 10)?;
    println!("plans laid: {}", run.home.industry.plans.ghosts.len());
    // The mouse points at the third plan; F makes it urgent; at the fifth, X cancels it.
    let plans: Vec<(u32, (i16, i16))> = run.home.industry.plans.ghosts.iter().map(|(id, g)| (*id, g.tile)).collect();
    let third = plans.get(2).ok_or("no third plan")?;
    mouse(reel, &mut run, BuilderAction::Plot { tile: (third.1.0 as u8, third.1.1 as u8), build: false })?;
    beat(reel, &mut run, 12)?;
    press(reel, &mut run, Input { fire: true, ..Default::default() })?;
    beat(reel, &mut run, 12)?;
    if let Some(fifth) = plans.get(4) {
        mouse(reel, &mut run, BuilderAction::Plot { tile: (fifth.1.0 as u8, fifth.1.1 as u8), build: false })?;
        beat(reel, &mut run, 8)?;
        press(reel, &mut run, Input { bomb: true, ..Default::default() })?;
    }
    // Put the card away, and watch the hands.
    mouse(reel, &mut run, BuilderAction::Slot(0))?;
    press(reel, &mut run, Input { team: true, ..Default::default() })?;
    reel.every = 5;
    beat(reel, &mut run, 30 * 50)?;
    reel.every = 15;
    beat(reel, &mut run, 30 * 90)?;
    println!("left: {} raised: {}", run.home.industry.plans.ghosts.len(), run.home.industry.plans.raised);
    Ok(())
}

/// A building picked up with G, copies of it laid, a group copied with M and stamped, a row dragged
/// out with the mouse, and Z taking the last back.
fn stamp(reel: &mut Reel) -> Result<(), String> {
    let mut run = hill(156.5, 104.5);
    let c = catalog();
    for (plan, tile) in [("sawpit", (146, 101)), ("well", (150, 101)), ("cottage", (146, 105))] {
        raise(&mut run, plan, tile)?;
    }
    for x in 146..=153 {
        let _ = run.home.industry.apply(c, Command::Road((x, 104)));
    }
    reel.every = 3;
    beat(reel, &mut run, 6)?;
    builder(reel, &mut run)?;
    // G: the cursor on the sawpit, it is in hand.
    mouse(reel, &mut run, BuilderAction::Plot { tile: (147, 101), build: false })?;
    beat(reel, &mut run, 24)?;
    press(reel, &mut run, Input { vigil: true, ..Default::default() })?;
    beat(reel, &mut run, 12)?;
    // E on empty ground lays another; Z takes it back.
    mouse(reel, &mut run, BuilderAction::Plot { tile: (157, 101), build: false })?;
    beat(reel, &mut run, 8)?;
    press(reel, &mut run, Input { interact: true, ..Default::default() })?;
    beat(reel, &mut run, 20)?;
    press(reel, &mut run, Input { play: 1, ..Default::default() })?;
    beat(reel, &mut run, 14)?;
    // M at one corner of the group, M at the other: a copy in hand.
    mouse(reel, &mut run, BuilderAction::Plot { tile: (145, 100), build: false })?;
    press(reel, &mut run, Input { skill: 1, ..Default::default() })?;
    beat(reel, &mut run, 10)?;
    for (x, y) in [(148u8, 102u8), (151, 104), (153, 106)] {
        mouse(reel, &mut run, BuilderAction::Plot { tile: (x, y), build: false })?;
        beat(reel, &mut run, 8)?;
    }
    press(reel, &mut run, Input { skill: 1, ..Default::default() })?;
    beat(reel, &mut run, 18)?;
    // Stamp it to the east; then turn it twice and stamp it again, up the slope.
    mouse(reel, &mut run, BuilderAction::Plot { tile: (157, 100), build: false })?;
    beat(reel, &mut run, 14)?;
    press(reel, &mut run, Input { interact: true, ..Default::default() })?;
    beat(reel, &mut run, 24)?;
    press(reel, &mut run, Input { ult: true, ..Default::default() })?;
    press(reel, &mut run, Input { ult: true, ..Default::default() })?;
    mouse(reel, &mut run, BuilderAction::Plot { tile: (153, 114), build: false })?;
    beat(reel, &mut run, 14)?;
    // A road dragged out with the left button.
    tool(&mut run, Tool::Road);
    mouse(reel, &mut run, BuilderAction::Plot { tile: (144, 108), build: true })?;
    for x in 145..=156u8 {
        mouse(reel, &mut run, BuilderAction::Hold((x, 108)))?;
        beat(reel, &mut run, 2)?;
    }
    mouse(reel, &mut run, BuilderAction::Let((156, 108)))?;
    beat(reel, &mut run, 20)?;
    press(reel, &mut run, Input { play: 1, ..Default::default() })?;
    beat(reel, &mut run, 24)?;
    println!("copies {} buildings {} roads {}", run.home.industry.copies.len(), run.home.industry.buildings.len(), run.home.industry.roads.len());
    Ok(())
}

/// What to do now and why, from the first minute: the goal at the view's top right, the whole tree
/// in the ledger, and the goal moving on as the party wins a seam and funds its charters.
fn goals(reel: &mut Reel) -> Result<(), String> {
    let c = catalog();
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(None);
    run.home.industry.found_hillside(c, 7);
    let h = run.players.get_mut(&1).expect("a knight");
    (h.x, h.y) = (146.5 * TILE_UNITS, 99.5 * TILE_UNITS);
    h.invulnerable = 1_000_000;
    h.stone = false;
    for _ in 0..4 {
        run.step(&BTreeMap::from([(1, Input::default())]));
    }
    reel.every = 3;
    beat(reel, &mut run, 20)?;
    // The ledger: C, then P five times for the goals' page, the tree.
    press(reel, &mut run, Input { cast: 4, ..Default::default() })?;
    press(reel, &mut run, Input { ride: true, ..Default::default() })?;
    for _ in 0..4 {
        press(reel, &mut run, Input { cycle: true, ..Default::default() })?;
    }
    beat(reel, &mut run, 20)?;
    for _ in 0..7 {
        press(reel, &mut run, Input { swing: true, ..Default::default() })?;
        beat(reel, &mut run, 10)?;
    }
    // Back to the world; the seam is won and the charters funded, the goal moves on.
    press(reel, &mut run, Input { ride: true, ..Default::default() })?;
    beat(reel, &mut run, 20)?;
    win_seam(&mut run);
    run.home.industry.stock.insert("plank".into(), 80);
    run.home.industry.stock.insert("stone".into(), 80);
    run.home.industry.stock.insert("block".into(), 20);
    run.home.industry.apply(c, Command::Research("masonry".into()))?;
    beat(reel, &mut run, 40)?;
    run.home.industry.apply(c, Command::Research("charter".into()))?;
    beat(reel, &mut run, 40)?;
    press(reel, &mut run, Input { ride: true, ..Default::default() })?;
    for _ in 0..4 {
        press(reel, &mut run, Input { cycle: true, ..Default::default() })?;
    }
    beat(reel, &mut run, 30)?;
    Ok(())
}

/// The Great Shaft's head at the Delve's gate, at each lift: one head, not two (the sim's lift is a
/// building of the works now, drawn with them, and the old head stays away on the hillside).
fn gate(reel: &mut Reel) -> Result<(), String> {
    let mut run = hill(141.5, 85.5);
    reel.every = 4;
    for plan in ["shaft-hand", "shaft-windlass", "shaft-gin", "shaft-water", "shaft-ley"] {
        if let Some(b) = run.home.industry.buildings.values_mut().find(|b| b.plan.starts_with("shaft-")) {
            b.plan = plan.into();
        }
        beat(reel, &mut run, 12)?;
    }
    Ok(())
}

/// A bench with nothing to work from, and the card that says why: what it makes, in and out a
/// minute, stock, crew, what it waits for; then a store's and a plan's.
fn card(reel: &mut Reel) -> Result<(), String> {
    let mut run = hill(150.5, 106.5);
    the_works(&mut run)?;
    {
        // The seam gives out: the banker has no rock.
        let k = &mut run.home.industry;
        k.stock.remove("limestone");
        k.stock.remove("block");
        for b in k.buildings.values_mut().filter(|b| b.plan == "claim-quarry") {
            b.paused = true;
        }
    }
    reel.every = 1_000_000;
    reel.wait(&mut run, 60 * 5)?;
    reel.every = 3;
    builder(reel, &mut run)?;
    beat(reel, &mut run, 6)?;
    mouse(reel, &mut run, BuilderAction::Plot { tile: (153, 104), build: false })?;
    beat(reel, &mut run, 45)?;
    mouse(reel, &mut run, BuilderAction::Plot { tile: (143, 98), build: false })?;
    beat(reel, &mut run, 45)?;
    let camp = run.home.industry.camp();
    mouse(reel, &mut run, BuilderAction::Plot { tile: (camp.0 as u8, camp.1 as u8), build: false })?;
    beat(reel, &mut run, 45)?;
    mouse(reel, &mut run, BuilderAction::Plot { tile: (153, 100), build: false })?;
    beat(reel, &mut run, 45)?;
    Ok(())
}

/// What the party made, used and held: the ledger's output page, a good at a time.
fn graph(reel: &mut Reel) -> Result<(), String> {
    let mut run = hill(150.5, 106.5);
    the_works(&mut run)?;
    reel.every = 1_000_000;
    reel.wait(&mut run, 60 * 14)?;
    reel.every = 4;
    builder(reel, &mut run)?;
    press(reel, &mut run, Input { ride: true, ..Default::default() })?;
    for _ in 0..5 {
        press(reel, &mut run, Input { cycle: true, ..Default::default() })?;
    }
    beat(reel, &mut run, 24)?;
    for _ in 0..4 {
        press(reel, &mut run, Input { swing: true, ..Default::default() })?;
        beat(reel, &mut run, 24)?;
    }
    Ok(())
}

/// The list of what chokes the works, worst first; E sends the view to the first.
fn chokes(reel: &mut Reel) -> Result<(), String> {
    let mut run = hill(150.5, 106.5);
    let c = catalog();
    the_works(&mut run)?;
    {
        let k = &mut run.home.industry;
        // A mortar mill with no lime behind it, and a banker with no rock.
        let mill = k.buildings.iter().find(|(_, b)| b.plan == "mortar-mill").map(|(i, _)| *i).ok_or("a mill")?;
        k.apply(c, Command::Connect(mill))?;
        k.stock.remove("limestone");
        k.stock.remove("block");
        for b in k.buildings.values_mut().filter(|b| b.plan == "claim-quarry") {
            b.paused = true;
        }
    }
    reel.every = 1_000_000;
    reel.wait(&mut run, 60 * 6)?;
    reel.every = 3;
    builder(reel, &mut run)?;
    beat(reel, &mut run, 6)?;
    press(reel, &mut run, Input { ride: true, ..Default::default() })?;
    for _ in 0..6 {
        press(reel, &mut run, Input { cycle: true, ..Default::default() })?;
    }
    beat(reel, &mut run, 24)?;
    press(reel, &mut run, Input { swing: true, ..Default::default() })?;
    beat(reel, &mut run, 18)?;
    press(reel, &mut run, Input { interact: true, ..Default::default() })?;
    beat(reel, &mut run, 60)?;
    // Walking brings the view back.
    hold(reel, &mut run, 40, Input { move_x: -1, ..Default::default() })?;
    beat(reel, &mut run, 20)?;
    Ok(())
}

fn frame_png(gpu: &mut Gpu, out: &Path, name: &str, run: &angel_delve::rules::Run, on: bool, scale: u32, held: bool) -> Result<(u32, u32), String> {
    let (w, h) = arena::seat_size();
    world3d::set_enabled(on);
    let map = arena::MapView { full: held, ..Default::default() };
    let began = std::time::Instant::now();
    let scene = Arc::new(arena::seat_scene_with(run, w as i32, h as i32, Some(1), &map));
    let scene_us = began.elapsed().as_micros() as u64;
    if on && std::env::var("HILL_DEBUG").is_ok() {
        eprintln!("3D scene: chunks {} lots {} works {} hill {} flat {} enabled {}", scene.chunks.len(), scene.lots.is_some(), scene.works.is_some(), scene.hill.is_some(), scene.is_flat(), world3d::enabled());
    }
    let mut maker = StageMaker::default();
    let began = std::time::Instant::now();
    let mut frame = maker.make(&scene, |_| None);
    gpu.upload(std::mem::take(&mut frame.pictures));
    frame = maker.make(&scene, |_| None);
    let back = (frame.size.0 / 384).max(1);
        let size = (frame.size.0 * scale / back, frame.size.1 * scale / back);
    let stage_us = began.elapsed().as_micros() as u64;
    let pixels = gpu.stage(frame, size, true);
    if on {
        TIMES.lock().unwrap().push((scene_us, stage_us));
    }
    write_png(&out.join(format!("{name}{}.png", if on { "" } else { "-flat" })), size, &pixels)?;
    world3d::set_enabled(false);
    Ok(size)
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
