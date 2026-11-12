//! The Works' carts, rails and the Keep's working site, in the realm's own renderer: every
//! frame is the game's stage (`StageMaker`) drawn offscreen by the GPU, in 3D (`NNNN.png`) and,
//! with `--flat`, in the flat view (`NNNN-flat.png`).
//!
//! ANGEL_REALM_PLAN=tabula cargo run --profile fast -p angel-glass --example rails3d_proof -- <outdir> [--scale N] [--flat] [--dry] <scene> [args]
//!
//! Scenes (frames go in `<outdir>/<scene>/`):
//! - `info`: where the crane, the yard and the gate stand, at a few stages
//! - `keep <view> <laid> <frames> [every] [back]`: the Keep's site alone, `every` ticks apart
//! - `site <seconds> <view> <frames> [every] [back] [laid=N]`: the scripted first chain, run `seconds` of
//!   sim time, then a reel of the whole working site (the supply lines feeding the wall)
//! - `tram`, `rope`, `barge`, `road`: one kind of line laid between two works, goods on it
//! - `bench`: a large site, what a frame costs

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_delve::kingdom::{Claim, Command, LineKind, catalog};
use angel_delve::render::overworld::arena::{self, stage_frame::StageMaker};
use angel_delve::rules::{Input, Run, TILE_UNITS};
use angel_delve::world3d::{self, kingdom};
use angel_gpu::{Core, Draw, Frame, Instance, Picture, Stage};

const STAGE: u32 = 0x00FA_57A4;

/// What each 3D frame cost the game before the GPU had it: the scene, the stage maker, and
/// how many nodes the works, the movers and the Keep's life stood as.
static TIMES: std::sync::Mutex<Vec<(u64, u64, usize, u64)>> = std::sync::Mutex::new(Vec::new());
/// The last frame's nodes: the works', the movers', the Keep's life, and the footprints the look dyes.
static LAST: std::sync::Mutex<[usize; 4]> = std::sync::Mutex::new([0; 4]);
/// Draw the works as one node a block, as they were drawn before they were baked (`--legacy`), for the before.
static LEGACY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Whether to time the GPU's draw too (a second, unread, drawing of each frame).
static DRAW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("rails3d_proof: {why}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> String {
    "usage: rails3d_proof <outdir> [--scale N] [--flat] [--dry] (info | keep <view> <laid> <frames> [every] [back] | site <seconds> <view> <frames> [every] [back] [laid=N] | tram | rope | barge | road | mule | pieces | gaps | factory [seconds] [back] | rails [seconds] [back] | bench | bench-lines | bench-moving)".into()
}

/// A view: where a knight stands, in realm tiles (the camera follows it).
fn view(name: &str) -> Option<(f32, f32)> {
    Some(match name {
        "bailey" => (133.0, 87.0),
        "gate" => (133.0, 92.0),
        "crane" => (127.0, 80.5),
        "yard" => (126.0, 84.0),
        "work" => (123.0, 84.0),
        "approach" => (133.0, 99.0),
        "keep" => (133.0, 81.0),
        "bay" => (126.5, 85.5),
        "east" => (142.5, 85.0),
        "works" => (150.0, 84.0),
        "camp" => (140.0, 97.0),
        "fac" => (168.0, 101.5),
        "facwide" => (168.0, 102.0),
        "pieces" => (156.5, 107.5),
        "spur" => (166.0, 105.0),
        _ => return None,
    })
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out: PathBuf = args.next().ok_or_else(usage)?.into();
    let (mut scale, mut dry, mut flat) = (3u32, false, false);
    let mut rest = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--scale" => scale = args.next().and_then(|v| v.parse().ok()).ok_or_else(usage)?,
            "--dry" => dry = true,
            "--flat" => flat = true,
            "--legacy" => LEGACY.store(true, std::sync::atomic::Ordering::Relaxed),
            _ => rest.push(a),
        }
    }
    let scene = rest.first().cloned().ok_or_else(usage)?;
    arena::props_meshes(true);
    if !angel_delve::world::plan::tabula() {
        return Err("needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula".into());
    }
    let dir = out.join(&scene);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let gpu = if dry || scene == "info" { None } else { Some(Gpu::new()?) };
    if gpu.is_some() {
        arena::draw_parts_outside(true);
        arena::props_meshes(true);
    }
    let mut reel = Reel { gpu, dir, scale, flat, every: 15, n: 0, ticks: 0, held: false };
    let w = &rest[1..];
    let num = |i: usize, d: u64| w.get(i).and_then(|v| v.parse().ok()).unwrap_or(d);
    match scene.as_str() {
        "info" => info(),
        "keep" => {
            let name = w.first().ok_or_else(usage)?;
            let tile = view(name).ok_or_else(|| format!("no view {name}"))?;
            reel.held = w.iter().any(|a| a == "back");
            keep(&mut reel, tile, num(1, 5000) as u32, num(2, 24) as u32, num(3, 15))
        }
        "site" => {
            let name = w.get(1).ok_or_else(usage)?;
            let tile = view(name).ok_or_else(|| format!("no view {name}"))?;
            reel.held = w.iter().any(|a| a == "back");
            let laid = w.iter().find_map(|a| a.strip_prefix("laid=").and_then(|v| v.parse().ok()));
            let dry = w.iter().find_map(|a| a.strip_prefix("dry=").and_then(|v| v.parse().ok()));
            let feed = w.iter().any(|a| a == "feed");
            site(&mut reel, num(0, 900), tile, num(2, 24) as u32, num(3, 15), laid, dry, feed)
        }
        "tram" | "rope" | "barge" | "road" | "mule" => line_scene(&mut reel, &scene),
        "pieces" => pieces_scene(&mut reel),
        "gaps" => gaps_scene(&mut reel),
        "factory" => factory_scene(&mut reel, num(0, 90), w.iter().any(|a| a == "back")),
        "rails" => rails_scene(&mut reel, num(0, 90), w.iter().any(|a| a == "back")),
        "bench" => bench(&mut reel),
        "bench-lines" => bench_lines(&mut reel),
        "bench-moving" => bench_moving(&mut reel),
        _ => Err(usage()),
    }
}

// ----------------------------------------------------------------- the reel ----

/// Frames of a reel: a frame every `every` ticks, 3D (and flat with `--flat`).
struct Reel {
    gpu: Option<Gpu>,
    dir: PathBuf,
    scale: u32,
    flat: bool,
    every: u64,
    n: u32,
    ticks: u64,
    /// The stand-back camera, the map key held.
    held: bool,
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
            frame_png(gpu, &self.dir, &name, run, true, self.scale, self.held)?;
            if self.flat {
                frame_png(gpu, &self.dir, &name, run, false, self.scale, self.held)?;
            }
        }
        self.n += 1;
        Ok(())
    }

    fn wait(&mut self, run: &mut Run, ticks: u64) -> Result<(), String> {
        for _ in 0..ticks {
            self.tick(run, Input::default())?;
        }
        Ok(())
    }
}

fn put_knight(run: &mut Run, tile: (f32, f32)) {
    let h = run.players.get_mut(&1).expect("a knight");
    (h.x, h.y) = (tile.0 * TILE_UNITS, tile.1 * TILE_UNITS);
    h.camera = Some((h.x, h.y - 1.0));
    h.invulnerable = 1_000_000;
    h.stone = false;
}

// -------------------------------------------------------------------- scenes ----

fn info() -> Result<(), String> {
    use angel_delve::world::{works, works_life};
    let castle = works::castle("great-tower").ok_or("no great tower")?;
    println!("castle {} stones; origin tile {:?}; px per m {}", castle.len(), works::ORIGIN, works::PX_PER_M);
    for laid in [1500u32, 3000, 5000, 7000, 9000, 11000] {
        match works_life::crane(castle, laid, 0.0) {
            Some(c) => {
                let w = castle.world([c.at[0], c.at[1], 0.0]);
                println!("laid {laid}: crane at {:?} m = kingdom px ({:.0}, {:.0}) tile ({:.1}, {:.1}), top {:.1} m, reach {:.1}", c.at, w[0], w[2], w[0] / 16.0, w[2] / 16.0, c.top, c.reach);
            }
            None => println!("laid {laid}: no crane"),
        }
    }
    let y = castle.world([works_life::YARD[0], works_life::YARD[1], 0.0]);
    println!("yard at {:?} m = tile ({:.1}, {:.1})", works_life::YARD, y[0] / 16.0, y[2] / 16.0);
    let g = castle.world([0.0, -14.0, 0.0]);
    println!("gate (0,-14) m = tile ({:.1}, {:.1})", g[0] / 16.0, g[2] / 16.0);
    // The stones' reach near the cranes: west face and east face over the crane's rows.
    for (name, y0, y1) in [("west crane rows", 0.0f32, 7.0f32), ("east crane rows", -3.0, 3.0), ("yard rows", -6.0, 0.0), ("gate rows", -16.0, -8.0)] {
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for st in castle.stones.iter().take(7190) {
            let b = st.aabb().map(|v| f32::from(v) / 100.0);
            if b[4] > y0 && b[1] < y1 {
                lo = lo.min(b[0]);
                hi = hi.max(b[3]);
            }
        }
        println!("{name} y {y0}..{y1}: first 7190 stones x {lo:.1} .. {hi:.1}");
    }
    for laid in [1500u32, 5000, 9000] {
        let last = castle.stones[(laid - 1) as usize].corners();
        let face = [0, 1, 2].map(|k| last.iter().map(|q| q[k]).sum::<f32>() / 8.0);
        let top = last.iter().map(|q| q[2]).fold(f32::MIN, f32::max);
        let bays: Vec<_> = castle.falsework.iter().filter(|f| f.kind == works::Work::Scaffold && f.appears <= laid + 320 && laid < f.gone).collect();
        let hi = bays.iter().map(|f| f32::from(f.bx[5]) / 100.0).fold(0.0f32, f32::max);
        let near = bays.iter().filter(|f| f32::from(f.bx[5]) / 100.0 > top - 1.0).count();
        println!("laid {laid}: last stone at {:?} top {top:.1}; {} scaffold bays standing, highest {hi:.1}, {} over the course", face.map(|v| (v * 10.0).round() / 10.0), bays.len(), near);
    }
    for f in castle.falsework.iter().filter(|f| f.kind == works::Work::Crane) {
        let b = f.bx.map(|v| f32::from(v) / 100.0);
        println!("crane falsework appears {} gone {} box {:?}", f.appears, f.gone, b);
    }
    Ok(())
}

/// The Keep's site alone at `laid`, a knight at `tile`, time passing.
fn keep(reel: &mut Reel, tile: (f32, f32), laid: u32, frames: u32, every: u64) -> Result<(), String> {
    let mut run = kingdom::proof_run(&[[tile.0 * 16.0, tile.1 * 16.0 + 6.0]]);
    run.lay_to("great-tower", laid);
    for _ in 0..20 {
        run.step(&BTreeMap::new());
    }
    reel.every = every;
    for _ in 0..frames {
        reel.shoot(&run)?;
        for _ in 0..every {
            run.tick += 1;
        }
    }
    report_times();
    Ok(())
}

/// A fresh run on the hillside, rich, the Great Shaft a ley lift, a knight at `tile`.
fn hill(tile: (f32, f32)) -> Run {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(None);
    run.home.industry.found_hillside(catalog(), 7);
    let k = &mut run.home.industry;
    for r in catalog().resources.keys() {
        k.stock.insert(r.clone(), 600);
    }
    k.stock.insert("fodder".into(), 100_000);
    k.researched = catalog().all_techs().map(|(k, _)| k.clone()).collect();
    if let Some(b) = k.buildings.values_mut().find(|b| b.plan == "shaft-hand") {
        b.plan = "shaft-ley".into();
    }
    put_knight(&mut run, tile);
    for _ in 0..4 {
        run.step(&BTreeMap::from([(1, Input::default())]));
    }
    run
}

fn win_seam(run: &mut Run) {
    let claim = Claim { resource: "limestone".into(), name: "Sound limestone face".into(), source: "The Mines / floor 3".into(), secured: false, rate: 4, yields: [("limestone".to_string(), 4), ("iron-ore".to_string(), 1)].into(), surface: false };
    run.home.industry.discover(catalog(), "mines-3".into(), claim);
    run.home.industry.secure("mines-3");
}

fn raise(run: &mut Run, plan: &str, tile: (i16, i16)) -> Result<u32, String> {
    let k = &mut run.home.industry;
    k.apply(catalog(), Command::Build { plan: plan.into(), tile }).map_err(|e| format!("{plan} at {tile:?}: {e}"))?;
    let id = k.at(tile).ok_or("it did not stand")?;
    k.buildings.get_mut(&id).ok_or("it did not stand")?.construction = 0;
    Ok(id)
}

/// The scripted first chain (as `rules::works_seam`'s test plays it), run `seconds` of sim time
/// with the foreman between jobs, then a reel of the site.
/// A fresh hillside run with a won face and the first chain raised by commands, the knight at `tile`.
fn start_chain(tile: (f32, f32)) -> Result<(Run, angel_delve::kingdom::first_chain::Chain), String> {
    use angel_delve::rules::{HZ, Pack, geology, industry};
    let c = catalog();
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(None);
    run.home.industry.found_hillside(c, 7);
    let k = &mut run.home.industry;
    for (g, n) in [("timber", 600), ("plank", 150), ("stone", 200), ("block", 20), ("coal", 80), ("fodder", 100_000)] {
        k.stock.insert(g.into(), n);
    }
    k.researched = c.all_techs().map(|(k, _)| k.clone()).collect();
    k.receive(c, "coin", 300);
    let seam = geology::seam_at(Pack::Cavern, 3);
    let face = industry::Deposit { id: "1ff-3-1-limestone".into(), x: 0.0, y: 0.0, clear: true, resource: seam.stone(), name: seam.name(), seam };
    let claim = Claim { resource: face.resource.into(), name: face.name.clone(), source: face.seam.source(), secured: false, rate: face.seam.rate(), yields: face.seam.yields(), surface: false };
    if !run.home.industry.discover(c, face.id.clone(), claim) || !run.home.industry.secure(&face.id) {
        return Err("the face is not won".into());
    }
    put_knight(&mut run, tile);
    for _ in 0..u64::from(HZ) * 12 {
        run.step(&BTreeMap::new());
    }
    run.hands.yard.timber = 4_000;
    let chain = angel_delve::kingdom::first_chain::raise(c, &mut run.home.industry, &face.id)?;
    Ok((run, chain))
}

/// The scripted first chain (as `rules::works_seam`'s test plays it), run `seconds` of sim time
/// with the foreman between jobs, then a reel of the site.
fn site(reel: &mut Reel, seconds: u64, tile: (f32, f32), frames: u32, every: u64, laid: Option<u32>, dry: Option<u32>, feed: bool) -> Result<(), String> {
    use angel_delve::rules::HZ;
    let c = catalog();
    let (mut run, mut chain) = start_chain(tile)?;
    let began = std::time::Instant::now();
    for _ in 0..seconds {
        for _ in 0..HZ {
            run.step(&BTreeMap::from([(1, Input::default())]));
        }
        chain.tend(c, &mut run.home.industry);
        put_knight(&mut run, tile);
    }
    println!("sim: {seconds} s in {:.1} s of wall; laid {}", began.elapsed().as_secs_f32(), run.sites.keep().map_or(0, |s| s.laid));
    if let Some(laid) = laid {
        // The wall as far up as the proof wants it, the sim's works round it as they stand.
        run.lay_to("great-tower", laid);
    }
    println!("carts {} lines {} buildings {}", run.home.industry.road_report(c).carts, run.home.industry.lines.len(), run.home.industry.buildings.len());
    reel.every = every;
    let mut cut_off = false;
    for frame in 0..frames {
        // The supply stops at frame `dry`: the yard's stock is gone, the carts empty, the foreman buys nothing more.
        if dry == Some(frame) {
            cut_off = true;
            if let Some(site) = run.keep_live() {
                let yard = run.home.industry.sites[&site].yard;
                if let Some(y) = run.home.industry.buildings.get_mut(&yard) {
                    y.input.clear();
                }
            }
            for b in run.home.industry.buildings.values_mut() {
                b.carts.clear();
                b.output.clear();
            }
            println!("frame {frame}: the supply is cut off");
        }
        // A supply: the makers' shelves are kept full of what the Keep's yard wants, and the sim's own carts take it there.
        if feed && !cut_off && frame % 6 == 0 {
            for (plan, good) in [("banker", "ashlar"), ("banker", "rubble"), ("carpentry", "joist"), ("mortar-mill", "mortar")] {
                for (_, id) in chain.raised.iter().filter(|(p, _)| p == plan) {
                    if let Some(b) = run.home.industry.buildings.get_mut(id) {
                        b.output.insert(good.into(), 24);
                    }
                }
            }
        }
        for _ in 0..every {
            run.step(&BTreeMap::from([(1, Input::default())]));
        }
        if !cut_off {
            chain.tend(c, &mut run.home.industry);
        }
        put_knight(&mut run, tile);
        reel.shoot(&run)?;
    }
    if let Some(hv) = angel_delve::world::hillside::of(&run) {
        if let Some(y) = &hv.keep_yard {
            println!("keep yard: cut {} rough {} mortar {} timber {} can_pay {} carts {}", y.cut, y.rough, y.mortar, y.timber, y.can_pay, y.carts.len());
        }
        println!("hill view: {} buildings, {} road tiles, {} lines, {} carts", hv.buildings.len(), hv.roads.len(), hv.lines.len(), hv.carts.len());
    }
    report_times();
    Ok(())
}


/// Every piece of rollerway facing every way with goods on it, the swape in each state: the kit's own sheet in 3D.
fn pieces_scene(reel: &mut Reel) -> Result<(), String> {
    use angel_delve::kingdom::{Dir, Piece};
    let c = catalog();
    let mut run = hill((156.5, 107.5));
    for (row, dir) in Dir::ALL.into_iter().enumerate() {
        for (col, piece) in Piece::ALL.into_iter().enumerate() {
            let tile = (150 + col as i16 * 3, 104 + row as i16 * 3);
            run.home.industry.apply(c, Command::Roller { piece, tile, dir }).map_err(|e| format!("{piece:?} {tile:?}: {e}"))?;
        }
    }
    // The swape in a row of its own, facing each way.
    for (row, dir) in Dir::ALL.into_iter().enumerate() {
        let tile = (169, 104 + row as i16 * 3);
        let id = raise(&mut run, "swape", tile)?;
        run.home.industry.apply(c, Command::Face { tile, dir }).map_err(|e| format!("face: {e}"))?;
        let _ = id;
    }
    let k = &mut run.home.industry;
    let names = ["ashlar", "plank", "lime", "timber", "stone", "brick", "flour", "iron-ore"];
    for n in names {
        if !k.rollers.goods.iter().any(|g| g == n) {
            k.rollers.goods.push(n.to_string());
        }
    }
    let tiles: Vec<_> = k.rollers.cells.keys().copied().collect();
    for (i, t) in tiles.iter().enumerate() {
        let cell = k.rollers.cells.get_mut(t).ok_or("no cell")?;
        for slot in 0..3 {
            let g = k.rollers.goods.iter().position(|x| x == names[(i + slot) % names.len()]).map_or(0, |p| p as u8 + 1);
            cell.slots[slot] = g;
            cell.across[slot] = g;
        }
        cell.moved = 4;
        cell.stuck = if i % 5 == 0 { 20 } else { 0 };
    }
    reel.every = 1;
    for _ in 0..16 {
        run.tick += 2;
        reel.shoot(&run)?;
    }
    report_times();
    Ok(())
}

/// The plans the first kit left undrawn, standing side by side (3D, and flat with `--flat`): the hods and ladders, the
/// great crane, the vineyard, the wine press, the aqueduct, and the swape facing each way.
fn gaps_scene(reel: &mut Reel) -> Result<(), String> {
    use angel_delve::kingdom::Dir;
    let c = catalog();
    let mut run = hill((160.0, 106.0));
    for (plan, tile) in [("hod-gang", (150, 104)), ("great-crane", (153, 104)), ("vineyard", (156, 104)), ("wine-press", (159, 104)), ("aqueduct", (150, 109))] {
        raise(&mut run, plan, tile)?;
    }
    for (k, dir) in Dir::ALL.into_iter().enumerate() {
        let tile = (163 + k as i16 * 2, 104);
        raise(&mut run, "swape", tile)?;
        run.home.industry.apply(c, Command::Face { tile, dir }).map_err(|e| format!("face: {e}"))?;
    }
    reel.every = 1;
    for _ in 0..16 {
        run.tick += 2;
        reel.shoot(&run)?;
    }
    report_times();
    Ok(())
}

/// A working factory: a store, a swape, a rollerway of eight tiles, a splitter that deals to a short line out the front and
/// a dead-end spur that backs up, a swape and a second store. Run `seconds` of sim time, then a reel.
fn factory_scene(reel: &mut Reel, seconds: u64, back: bool) -> Result<(), String> {
    use angel_delve::kingdom::{Dir, Piece};
    use angel_delve::rules::HZ;
    let c = catalog();
    reel.held = back;
    let mut run = hill((168.0, 101.5));
    let lay = |run: &mut Run, piece: Piece, tile: (i16, i16), dir: Dir| -> Result<(), String> {
        run.home.industry.apply(c, Command::Roller { piece, tile, dir }).map(|_| ()).map_err(|e| format!("{piece:?} {tile:?}: {e}"))
    };
    let swape = |run: &mut Run, tile: (i16, i16), dir: Dir| -> Result<u32, String> {
        let id = raise(run, "swape", tile)?;
        run.home.industry.apply(c, Command::Face { tile, dir }).map_err(|e| format!("face: {e}"))?;
        Ok(id)
    };
    let a = raise(&mut run, "depot", (160, 100))?;
    swape(&mut run, (163, 100), Dir::E)?;
    for x in 164..172 {
        lay(&mut run, Piece::Roller, (x, 100), Dir::E)?;
    }
    lay(&mut run, Piece::Splitter, (172, 100), Dir::E)?;
    for x in 173..176 {
        lay(&mut run, Piece::Roller, (x, 100), Dir::E)?;
    }
    swape(&mut run, (176, 100), Dir::E)?;
    let b = raise(&mut run, "depot", (177, 100))?;
    // The spur: four tiles south from the splitter's right hand, nothing at its end: it fills and stands.
    for y in 101..105 {
        lay(&mut run, Piece::Roller, (172, y), Dir::S)?;
    }
    // A hand-chain on the other side, crewed or not, and a gate and a merge at its end.
    for x in 164..169 {
        lay(&mut run, Piece::Chain, (x, 97), Dir::E)?;
    }
    lay(&mut run, Piece::Gate, (169, 97), Dir::E)?;
    lay(&mut run, Piece::Merge, (170, 97), Dir::E)?;
    lay(&mut run, Piece::Roller, (171, 97), Dir::E)?;
    lay(&mut run, Piece::Trestle, (172, 97), Dir::E)?;
    lay(&mut run, Piece::Roller, (173, 97), Dir::E)?;
    run.home.industry.buildings.get_mut(&a).ok_or("no depot")?.output.insert("plank".into(), 300);
    println!("factory: {} rollerway tiles, depots {a} and {b}", run.home.industry.rollers.cells.len());
    reel.every = 1;
    for _ in 0..seconds {
        for _ in 0..HZ {
            run.step(&BTreeMap::from([(1, Input::default())]));
        }
        put_knight(&mut run, (168.0, 101.5));
    }
    {
        let k = &run.home.industry;
        for row in k.roller_report(c) {
            println!("line {:?}: {} tiles, {:?} swapes, {}/min, state {:?}, jams {:?}", row.id, row.tiles, row.swapes, row.per_min, row.state, row.jams.iter().map(|j| j.why.clone()).collect::<Vec<_>>());
        }
        println!("depot B holds {} planks", k.buildings[&b].output.get("plank").copied().unwrap_or(0));
        println!("stuck tiles {:?}", k.rollers.cells.iter().filter(|(_, c)| c.stuck > 0).map(|(t, c)| (*t, c.stuck)).collect::<Vec<_>>());
    }
    if let Some(hv) = angel_delve::world::hillside::of(&run) {
        println!("jams {:?}; belts {}", hv.jams, hv.belts.len());
    }
    for _ in 0..40 {
        for _ in 0..3 {
            reel.tick(&mut run, Input::default())?;
        }
        put_knight(&mut run, (168.0, 101.5));
        reel.shoot(&run)?;
    }
    report_times();
    Ok(())
}

/// The rail network: three bays, a switch where the ways part, two routes with wagons; the wagons loaded and running.
fn rails_scene(reel: &mut Reel, seconds: u64, back: bool) -> Result<(), String> {
    use angel_delve::kingdom::{Act, Stop};
    use angel_delve::rules::HZ;
    let c = catalog();
    reel.held = back;
    let mut run = hill((163.0, 108.0));
    win_seam(&mut run);
    let q = raise(&mut run, "claim-quarry", (156, 100))?;
    let bench = raise(&mut run, "banker", (172, 100))?;
    let mill = raise(&mut run, "sawpit", (168, 110))?;
    let k = &mut run.home.industry;
    k.apply(c, Command::Link { building: q, claim: "mines-3".into() })?;
    k.apply(c, Command::Recipe { building: bench, recipe: "dress-limestone".into() })?;
    let south = |k: &angel_delve::kingdom::Kingdom, id: u32| {
        let t = k.covered(&k.buildings[&id]).max_by_key(|t| (t.1, t.0)).expect("it covers tiles");
        (t.0, t.1 + 1)
    };
    let (bq, bb, bm) = (south(k, q), south(k, bench), south(k, mill));
    let row = bq.1.max(bb.1) + 2;
    let mut way: Vec<(i16, i16)> = (bq.1..=row).map(|y| (bq.0, y)).collect();
    way.extend((bq.0..=bb.0).map(|x| (x, row)));
    way.extend((bb.1..=row).map(|y| (bb.0, y)));
    // The spur down to the sawpit's bay, off the middle of the row: the switch is where it leaves.
    let mid = (bq.0 + bb.0) / 2;
    way.extend((row..=bm.1).map(|y| (mid, y)));
    way.extend((mid.min(bm.0)..=mid.max(bm.0)).map(|x| (x, bm.1)));
    way.dedup();
    for t in way {
        if !k.rails.cells.contains_key(&t) {
            k.apply(c, Command::Rail(t)).map_err(|why| format!("rail at {t:?}: {why}"))?;
        }
    }
    for (tile, name) in [(bq, "Quarry"), (bb, "Bench"), (bm, "Sawpit")] {
        k.apply(c, Command::Bay { tile, name: Some(name.into()) }).map_err(|why| format!("bay {name}: {why}"))?;
    }
    let stop = |bay: &str, act: Act| Stop { bay: bay.into(), act, good: None };
    k.apply(c, Command::Route { name: None, stops: vec![stop("Quarry", Act::Load), stop("Bench", Act::Unload)], wagons: 2 })?;
    k.apply(c, Command::Route { name: None, stops: vec![stop("Quarry", Act::Load), stop("Sawpit", Act::Unload)], wagons: 2 })?;
    println!("rails: {} tiles, {} wagons, {} bays", k.rails.cells.len(), k.rails.wagons.len(), k.rails.bays.len());
    for t in k.rails.cells.keys().copied().collect::<Vec<_>>() {
        if k.rails.is_switch(t) {
            println!("switch at {t:?}");
        }
    }
    reel.every = 1;
    for _ in 0..seconds {
        for _ in 0..HZ {
            run.step(&BTreeMap::from([(1, Input::default())]));
        }
        put_knight(&mut run, (163.0, 108.0));
    }
    for _ in 0..40 {
        for _ in 0..3 {
            reel.tick(&mut run, Input::default())?;
        }
        put_knight(&mut run, (163.0, 108.0));
        reel.shoot(&run)?;
    }
    report_times();
    Ok(())
}

/// One kind of line between two works, goods on it, a few vehicles.
fn line_scene(reel: &mut Reel, which: &str) -> Result<(), String> {
    let c = catalog();
    let (kind, vehicles, a_plan, b_plan, a_at, b_at, cam) = match which {
        "tram" => (LineKind::Tram, 3u16, "claim-quarry", "banker", (170i16, 100i16), (154i16, 100i16), (162.0f32, 101.5f32)),
        "rope" => (LineKind::Rope, 4, "claim-quarry", "banker", (172, 106), (150, 104), (161.0, 106.0)),
        "barge" => {
            // Two works on the river's east bank, a long reach apart, the lane between them the river.
            let at = |y: i16| (angel_delve::kingdom::hillside::river_column(7, y) + 2, y);
            (LineKind::Barge, 2, "quay", "quay", at(92), at(112), (at(102).0 as f32 + 1.0, 102.0))
        }
        "mule" => (LineKind::Mule, 3, "claim-quarry", "banker", (170, 100), (154, 100), (162.0, 101.5)),
        _ => (LineKind::Cart, 3, "claim-quarry", "banker", (170, 100), (154, 100), (162.0, 101.5)),
    };
    let mut run = hill(cam);
    win_seam(&mut run);
    let a = raise(&mut run, a_plan, a_at)?;
    let b = raise(&mut run, b_plan, b_at)?;
    let k = &mut run.home.industry;
    if which == "barge" {
        // Goods on the quay for the barges to take downstream.
        if let Some(q) = k.buildings.get_mut(&a) {
            q.output.insert("limestone".into(), 40);
        }
    } else {
        k.apply(c, Command::Link { building: a, claim: "mines-3".into() })?;
        k.apply(c, Command::Recipe { building: b, recipe: "dress-limestone".into() })?;
        for id in [a, b] {
            k.apply(c, Command::Connect(id))?;
        }
    }
    if which == "road" {
        let yard = raise(&mut run, "carters-yard", (160, 104))?;
        let k = &mut run.home.industry;
        k.apply(c, Command::Connect(yard))?;
        k.apply(c, Command::Carts { yard, carts: 4 })?;
    } else if which == "tram" {
        // A tramway is the rail network now: a bay by each building, rail between them, a route of wagons.
        let k = &mut run.home.industry;
        let south = |k: &angel_delve::kingdom::Kingdom, id: u32| {
            let t = k.covered(&k.buildings[&id]).max_by_key(|t| (t.1, t.0)).expect("it covers tiles");
            (t.0, t.1 + 1)
        };
        let (bay_a, bay_b) = (south(k, a), south(k, b));
        let row = bay_a.1.max(bay_b.1) + 1;
        let mut way: Vec<(i16, i16)> = (bay_a.1..=row).map(|y| (bay_a.0, y)).collect();
        let (lo, hi) = (bay_a.0.min(bay_b.0), bay_a.0.max(bay_b.0));
        way.extend((lo..=hi).map(|x| (x, row)));
        way.extend((bay_b.1..=row).map(|y| (bay_b.0, y)));
        way.dedup();
        for t in way {
            if !k.rails.cells.contains_key(&t) {
                k.apply(c, Command::Rail(t)).map_err(|why| format!("rail at {t:?}: {why}"))?;
            }
        }
        k.apply(c, Command::Bay { tile: bay_a, name: Some("Quarry".into()) }).map_err(|why| format!("bay: {why}"))?;
        k.apply(c, Command::Bay { tile: bay_b, name: Some("Bench".into()) }).map_err(|why| format!("bay: {why}"))?;
        let stops = vec![
            angel_delve::kingdom::Stop { bay: "Quarry".into(), act: angel_delve::kingdom::Act::Load, good: None },
            angel_delve::kingdom::Stop { bay: "Bench".into(), act: angel_delve::kingdom::Act::Unload, good: None },
        ];
        k.apply(c, Command::Route { name: None, stops, wagons: vehicles }).map_err(|why| format!("route: {why}"))?;
        println!("tram: {} rail tiles, {} wagons", k.rails.cells.len(), k.rails.wagons.len());
    } else {
        let k = &mut run.home.industry;
        // The planner's own way (an empty path).
        match k.apply(c, Command::Line { kind, from: a, to: b, path: Vec::new(), vehicles }) {
            Ok(_) => {}
            Err(why) => return Err(format!("{which} line: {why}")),
        }
    }
    println!("{which}: lines {} path {:?}", run.home.industry.lines.len(), run.home.industry.lines.values().next().map(|l| (l.path.len(), l.path.first().copied(), l.path.last().copied())));
    reel.every = 12;
    reel.wait(&mut run, 30 * 25)?;
    {
        let k = &run.home.industry;
        for (id, l) in &k.lines {
            let rows = k.line_report(c);
            let state = rows.iter().find(|r| r.id == *id).map(|r| r.state);
            println!("line {id}: {:?} state {:?} fleet {:?}", l.kind, state, l.fleet.iter().map(|v| (v.phase, v.amount, v.resource.clone())).collect::<Vec<_>>());
        }
        println!("buildings {:?}", k.buildings.values().map(|b| (b.plan.clone(), b.construction)).collect::<Vec<_>>());
        println!("status {:?}", k.report(c).status);
    }
    reel.wait(&mut run, 30 * 45)?;
    report_times();
    Ok(())
}

/// A large site: what a frame costs with the Keep half built and the works crowding it.
fn bench(reel: &mut Reel) -> Result<(), String> {
    DRAW.store(true, std::sync::atomic::Ordering::Relaxed);
    let c = catalog();
    let mut run = hill((140.0, 92.0));
    win_seam(&mut run);
    let yard = raise(&mut run, "carters-yard", (143, 98))?;
    let mut ids = vec![yard];
    let menu: Vec<&String> = c.hillside_menu.iter().filter(|p| !c.plans[*p].yard && c.plans[*p].connector.is_none() && c.plans[*p].stalls == 0 && c.plans[*p].replaces.is_none()).collect();
    run.home.industry.population = angel_delve::kingdom::population::Population::founders_of(60);
    let mut n = 0usize;
    'fill: for row in 0..10i16 {
        for col in 0..12i16 {
            let tile = (148 + col * 7, 98 + row * 6);
            for try_ in 0..menu.len() {
                let plan = menu[(n + try_) % menu.len()];
                if let Ok(id) = raise(&mut run, plan, tile) {
                    ids.push(id);
                    n += 1;
                    break;
                }
            }
            if n >= 60 {
                break 'fill;
            }
        }
    }
    let k = &mut run.home.industry;
    for id in &ids {
        let _ = k.apply(c, Command::Connect(*id));
    }
    let _ = k.apply(c, Command::Carts { yard, carts: 24 });
    run.lay_to("great-tower", 5000);
    println!("{} buildings, {} road tiles", run.home.industry.buildings.len(), run.home.industry.roads.len());
    reel.every = 1000;
    reel.wait(&mut run, 120 * 30)?;
    // The knight beside the Keep, then in front of the works: a frame every few ticks.
    reel.every = 4;
    for tile in [(133.0f32, 92.0f32), (150.0, 100.0)] {
        put_knight(&mut run, tile);
        TIMES.lock().unwrap().clear();
        reel.wait(&mut run, 400)?;
        println!("camera at {tile:?}");
        report_times();
    }
    Ok(())
}

/// A large site with rails and rollers: the Keep at 5,000 stones, forty-odd works, a rail ring with a route and a score of
/// wagons, eight rows of rollerway of fifty tiles with goods streaming on them; what a frame costs.
fn bench_lines(reel: &mut Reel) -> Result<(), String> {
    use angel_delve::kingdom::{Dir, Piece};
    use angel_delve::rules::HZ;
    DRAW.store(true, std::sync::atomic::Ordering::Relaxed);
    let c = catalog();
    let mut run = hill((150.0, 112.0));
    win_seam(&mut run);
    let menu: Vec<&String> = c.hillside_menu.iter().filter(|p| !c.plans[*p].yard && c.plans[*p].connector.is_none() && c.plans[*p].stalls == 0 && c.plans[*p].replaces.is_none() && !c.plans[*p].quay && c.plans[*p].swape == 0).collect();
    let mut n = 0usize;
    'fill: for row in 0..6i16 {
        for col in 0..9i16 {
            let tile = (148 + col * 7, 98 + row * 5);
            for try_ in 0..menu.len() {
                if raise(&mut run, menu[(n + try_) % menu.len()], tile).is_ok() {
                    n += 1;
                    break;
                }
            }
            if n >= 30 {
                break 'fill;
            }
        }
    }
    // Eight rows of rollerway, fifty tiles each, a splitter and a trestle in every row.
    let mut laid = 0;
    for row in 0..8i16 {
        let y = 134 + row * 3;
        for x in 150..200i16 {
            let piece = if x == 175 { Piece::Splitter } else if x == 180 { Piece::Trestle } else if row % 3 == 2 { Piece::Chain } else { Piece::Roller };
            if run.home.industry.apply(c, Command::Roller { piece, tile: (x, y), dir: Dir::E }).is_ok() {
                laid += 1;
            }
        }
    }
    run.lay_to("great-tower", 5000);
    println!("{} buildings, {laid} roller tiles", run.home.industry.buildings.len());
    let names = ["ashlar", "plank", "lime", "timber", "stone", "brick", "flour", "iron-ore"];
    for nme in names {
        if !run.home.industry.rollers.goods.iter().any(|g| g == nme) {
            run.home.industry.rollers.goods.push(nme.to_string());
        }
    }
    // The goods are kept streaming: every slot of every tile is filled each second and the rollers say they moved.
    let poke = |run: &mut Run| {
        let goods = run.home.industry.rollers.goods.clone();
        for (i, (_, cell)) in run.home.industry.rollers.cells.iter_mut().enumerate() {
            for slot in 0..3 {
                let g = goods.iter().position(|x| x == names[(i + slot) % names.len()]).map_or(0, |p| p as u8 + 1);
                cell.slots[slot] = g;
                if cell.piece == Piece::Trestle {
                    cell.across[slot] = g;
                }
            }
            cell.moved = 4;
        }
    };
    reel.every = 4;
    for (tile, what) in [((133.0f32, 92.0f32), "the Keep"), ((170.0, 136.0), "the rollers")] {
        put_knight(&mut run, tile);
        TIMES.lock().unwrap().clear();
        for _ in 0..240 {
            if run.tick % u64::from(HZ) == 0 {
                poke(&mut run);
            }
            reel.tick(&mut run, Input::default())?;
        }
        println!("camera at {tile:?} ({what})");
        report_times();
    }
    Ok(())
}

/// A large site with its carts at work: the first chain's roads and a carter's yard of twelve carts, forty more
/// works about it, a tramway and a ropeway, the sim run until the carriers are out on their ways.
fn bench_moving(reel: &mut Reel) -> Result<(), String> {
    use angel_delve::rules::HZ;
    DRAW.store(true, std::sync::atomic::Ordering::Relaxed);
    let c = catalog();
    let (mut run, mut chain) = start_chain((140.0, 92.0))?;
    let yard = chain.carters.ok_or("no carter's yard")?;
    let menu: Vec<&String> = c.hillside_menu.iter().filter(|p| !c.plans[*p].yard && c.plans[*p].connector.is_none() && c.plans[*p].stalls == 0 && c.plans[*p].replaces.is_none() && !c.plans[*p].quay).collect();
    run.home.industry.population = angel_delve::kingdom::population::Population::founders_of(80);
    let mut n = 0usize;
    'fill: for row in 0..12i16 {
        for col in 0..14i16 {
            let tile = (150 + col * 6, 99 + row * 5);
            for try_ in 0..menu.len() {
                if let Ok(id) = raise(&mut run, menu[(n + try_) % menu.len()], tile) {
                    let _ = run.home.industry.apply(c, Command::Connect(id));
                    n += 1;
                    break;
                }
            }
            if n >= 48 {
                break 'fill;
            }
        }
    }
    // What a tramway and a ropeway are made of, in the camp's stores, so they can be laid.
    for (g, n) in [("beam", 40), ("horse", 20), ("nails", 80), ("wagon", 20), ("bucket", 20), ("pole", 40), ("rope", 40)] {
        let have = run.home.industry.stock.get(g).copied().unwrap_or(0);
        run.home.industry.stock.insert(g.into(), have.max(n));
    }
    let mut more_carts = false;
    let mut lines_laid = false;
    let mut last_why = String::new();
    for _ in 0..(10 * 60) {
        for _ in 0..HZ {
            run.step(&BTreeMap::from([(1, Input::default())]));
        }
        chain.tend(c, &mut run.home.industry);
        // The carter's yard buys twelve carts as soon as it stands.
        if !more_carts {
            match run.home.industry.apply(c, Command::Carts { yard, carts: 12 }) {
                Ok(_) => more_carts = true,
                Err(why) => last_why = why,
            }
        }
        if more_carts && !lines_laid {
            lines_laid = true;
            let id_of = |plan: &str, k: usize| chain.raised.iter().filter(|(p, _)| p == plan).nth(k).map(|(_, id)| *id);
            for (kind, from, to, vehicles) in [(LineKind::Tram, id_of("claim-quarry", 0), id_of("banker", 0), 4u16), (LineKind::Tram, id_of("sawpit", 0), id_of("carpentry", 0), 3), (LineKind::Rope, id_of("lime-kiln", 0), id_of("mortar-mill", 0), 4)] {
                let (Some(from), Some(to)) = (from, to) else { continue };
                match run.home.industry.apply(c, Command::Line { kind, from, to, path: Vec::new(), vehicles }) {
                    Ok(_) => println!("{kind:?} line laid"),
                    Err(why) => println!("{kind:?} line: {why}"),
                }
            }
        }
    }
    let hv = angel_delve::world::hillside::of(&run).ok_or("not on the hillside")?;
    let riders: usize = hv.lines.iter().map(|l| l.riders.len()).sum();
    let riding: usize = hv.lines.iter().flat_map(|l| l.riders.iter()).filter(|r| r.moving).count();
    println!("more carts {more_carts} ({last_why}); riders on lines {riders} of which {riding} moving");
    let moving = hv.carts.iter().filter(|r| r.moving).count();
    let loaded = hv.carts.iter().filter(|r| r.load.is_some()).count();
    println!("{} buildings, {} road tiles, {} carts of which {moving} moving and {loaded} loaded", hv.buildings.len(), hv.roads.len(), hv.carts.len());
    println!("carriers: {} in all, {} moving: the many-carriers detail is {}", hv.carts.len() + riders, moving + riding, if hv.carts.len() + riders > angel_delve::world3d::hillside::MANY_CARRIERS { "on" } else { "off" });
    reel.every = 4;
    for tile in [(133.0f32, 92.0f32), (152.0, 100.0), (160.0, 112.0)] {
        put_knight(&mut run, tile);
        TIMES.lock().unwrap().clear();
        for _ in 0..240 {
            reel.tick(&mut run, Input::default())?;
            if run.tick % u64::from(HZ) == 0 {
                chain.tend(c, &mut run.home.industry);
            }
        }
        println!("camera at {tile:?}");
        report_times();
    }
    Ok(())
}

fn report_times() {
    let times = TIMES.lock().unwrap().clone();
    if times.is_empty() {
        return;
    }
    let mut scene: Vec<u64> = times.iter().map(|t| t.0).collect();
    let mut stage: Vec<u64> = times.iter().map(|t| t.1).collect();
    scene.sort_unstable();
    stage.sort_unstable();
    let last = *LAST.lock().unwrap();
    println!("last frame: works {} movers {} life {} footprints {}", last[0], last[1], last[2], last[3]);
    let mut draw: Vec<u64> = times.iter().map(|t| t.3).collect();
    draw.sort_unstable();
    println!(
        "scene us median {} p95 {}; stage us median {} p95 {}; gpu draw us median {} p95 {}; extra nodes about {} over {} frames",
        scene[scene.len() / 2],
        scene[scene.len() * 95 / 100],
        stage[stage.len() / 2],
        stage[stage.len() * 95 / 100],
        draw[draw.len() / 2],
        draw[draw.len() * 95 / 100],
        times.last().map_or(0, |t| t.2),
        scene.len()
    );
}

fn frame_png(gpu: &mut Gpu, out: &Path, name: &str, run: &Run, on: bool, scale: u32, held: bool) -> Result<(u32, u32), String> {
    let (w, h) = arena::seat_size();
    world3d::set_enabled(on);
    let map = arena::MapView { full: held, ..Default::default() };
    let began = std::time::Instant::now();
    let mut scene = arena::seat_scene_with(run, w as i32, h as i32, Some(1), &map);
    if LEGACY.load(std::sync::atomic::Ordering::Relaxed)
        && let (Some(hill), Some(realm)) = (scene.hill.clone(), scene.realm)
    {
        let view = [realm.origin[0] - 96.0, realm.origin[1] - 64.0, realm.origin[0] + realm.size[0] + 96.0, realm.origin[1] + realm.size[1] + 192.0];
        let nodes = world3d::hillside::legacy_nodes(run, view);
        scene.hill = Some(Arc::new(world3d::hillside::HillNodes { parts: Vec::new(), lamps: hill.lamps.clone(), footprints: hill.footprints.clone(), ghost: nodes, bounds: hill.bounds }));
    }
    let scene = Arc::new(scene);
    let scene_us = began.elapsed().as_micros() as u64;
    let extra = scene.hill.as_ref().map_or(0, |h| h.parts.len() + h.ghost.len()) + scene.works.as_ref().map_or(0, |w| w.life.len());
    if on {
        *LAST.lock().unwrap() = [
            scene.hill.as_ref().map_or(0, |h| h.parts.len()),
            0,
            scene.works.as_ref().map_or(0, |w| w.life.len()),
            scene.hill.as_ref().map_or(0, |h| h.footprints.len()),
        ];
    }
    let mut maker = StageMaker::default();
    let began = std::time::Instant::now();
    let mut frame = maker.make(&scene, |_| None);
    gpu.upload(std::mem::take(&mut frame.pictures));
    frame = maker.make(&scene, |_| None);
    let back = (frame.size.0 / 384).max(1);
    let size = (frame.size.0 * scale / back, frame.size.1 * scale / back);
    let stage_us = began.elapsed().as_micros() as u64;
    let draw_us = if on && DRAW.load(std::sync::atomic::Ordering::Relaxed) {
        let again = maker.make(&scene, |_| None);
        let began = std::time::Instant::now();
        let _ = gpu.stage(again, size, false);
        began.elapsed().as_micros() as u64
    } else {
        0
    };
    let pixels = gpu.stage(frame, size, true);
    if on {
        TIMES.lock().unwrap().push((scene_us, stage_us, extra, draw_us));
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
        println!("adapter: {:?}", adapter.get_info().name);
        let core = Core::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
        Ok(Self { core, device, target: None })
    }

    fn upload(&mut self, pictures: Vec<angel_stage::Picture>) {
        for p in pictures {
            self.core.upload(Picture { id: p.id, width: p.width, height: p.height, rgba: p.rgba, cols: 0, rows: 0 });
        }
    }

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
