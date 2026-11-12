//! The hillside works in the realm's own renderer (the Works): a small works stood on the
//! slope, a line laid by hand by a knight with the builder in hand, goods running on it, a
//! bottleneck showing in the flow overlay and a second line clearing it. Every frame is
//! drawn by the game's own renderer offscreen, 3D (`NNNN.png`) and flat (`NNNN-flat.png`).
//!
//! ANGEL_REALM_PLAN=tabula cargo run --profile fast -p angel-glass --example hillside_proof -- <outdir> [--scale N] <scene>
//!
//! Scenes: `works` (a small works at work), `line` (a line laid by hand, a bottleneck, a
//! second line), `big` (a large works: the frame times). Frames go in `<outdir>/<scene>/`;
//! `--dry` runs the script without drawing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_delve::kingdom::{Claim, Command, LineKind, catalog};
use angel_delve::render::overworld::arena::{self, stage_frame::StageMaker};
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
            eprintln!("hillside_proof: {why}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> String {
    "usage: hillside_proof <outdir> [--scale N] [--dry] (works | line | big)".into()
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out: PathBuf = args.next().ok_or_else(usage)?.into();
    let (mut scale, mut dry, mut scene) = (3u32, false, None);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--scale" => scale = args.next().and_then(|v| v.parse().ok()).ok_or_else(usage)?,
            "--dry" => dry = true,
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
    let mut reel = Reel { gpu, dir, scale, every: 20, n: 0, ticks: 0, size: (0, 0) };
    match scene.as_str() {
        "works" => works(&mut reel),
        "line" => line(&mut reel),
        "big" => big(&mut reel),
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
            self.size = frame_png(gpu, &self.dir, &name, run, true, self.scale, false)?;
            frame_png(gpu, &self.dir, &name, run, false, self.scale, false)?;
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
    run.kingdom.industry.get_mut(&1).expect("a board").plan = i;
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
    k.stock.insert("fodder".into(), 100_000);
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

/// A small works at work, the knight walking through it: frames of the kit's pieces at their
/// states, carts on the roads, people at their posts.
fn works(reel: &mut Reel) -> Result<(), String> {
    let mut run = hill(150.5, 100.5);
    let c = catalog();
    win_seam(&mut run);
    let yard = raise(&mut run, "carters-yard", (143, 98))?;
    let quarry = raise(&mut run, "claim-quarry", (160, 101))?;
    let kiln = raise(&mut run, "lime-kiln", (153, 100))?;
    let banker = raise(&mut run, "banker", (153, 104))?;
    let wood = raise(&mut run, "woodsman-camp", (143, 103))?;
    let saw = raise(&mut run, "sawpit", (148, 103))?;
    raise(&mut run, "well", (150, 97))?;
    raise(&mut run, "cottage", (137, 99))?;
    raise(&mut run, "mortar-mill", (157, 97))?;
    let k = &mut run.home.industry;
    k.apply(c, Command::Link { building: quarry, claim: "mines-3".into() })?;
    k.apply(c, Command::Recipe { building: banker, recipe: "dress-limestone".into() })?;
    for id in [yard, quarry, kiln, banker, wood, saw] {
        k.apply(c, Command::Connect(id))?;
    }
    k.apply(c, Command::Carts { yard, carts: 4 })?;
    reel.every = 15;
    reel.wait(&mut run, 90)?;
    // The knight walks the length of it.
    go(reel, &mut run, (146, 100), Input::default())?;
    go(reel, &mut run, (146, 99), Input::default())?;
    go(reel, &mut run, (163, 99), Input::default())?;
    reel.wait(&mut run, 4)
}

/// The lead's proof: a knight lays a ropeway (it was a tramway before the rails) from a quarry to a banker by walking it, goods run
/// on it, the flow overlay shows the one wagon is the bottleneck (the quarry backs up amber, the
/// banker waits red), a second line from the quarry to a second banker clears it.
fn line(reel: &mut Reel) -> Result<(), String> {
    let mut run = hill(168.5, 101.5);
    let c = catalog();
    win_seam(&mut run);
    let quarry = raise(&mut run, "claim-quarry", (170, 100))?;
    let a = raise(&mut run, "banker", (154, 100))?;
    let b = raise(&mut run, "banker", (154, 105))?;
    let k = &mut run.home.industry;
    k.apply(c, Command::Link { building: quarry, claim: "mines-3".into() })?;
    k.apply(c, Command::Recipe { building: a, recipe: "dress-limestone".into() })?;
    k.apply(c, Command::Recipe { building: b, recipe: "dress-limestone".into() })?;
    for id in [quarry, a, b] {
        k.apply(c, Command::Connect(id))?;
    }
    // No carts: the lines are the only way the rock goes from the quarry to the cutters.
    reel.every = 20;
    reel.wait(&mut run, 20)?;
    // The builder comes out, the tool is a tramway.
    press(reel, &mut run, Input { cast: 4, ..Default::default() })?;
    tool(&mut run, Tool::Line(LineKind::Rope));
    // Stand by the quarry's west face, looking at it; press E, walk west holding it, let go at the banker.
    go(reel, &mut run, (169, 101), Input::default())?;
    press(reel, &mut run, Input { move_x: 1, ..Default::default() })?;
    reel.tick(&mut run, Input { interact: true, ..Default::default() })?;
    go(reel, &mut run, (158, 101), Input { interact: true, ..Default::default() })?;
    reel.tick(&mut run, Input { move_x: -1, interact: true, ..Default::default() })?;
    reel.tick(&mut run, Input::default())?;
    println!("first line: {}", run.kingdom.industry[&1].message);
    reel.wait(&mut run, 150)?;
    // The overlay.
    if let Some(v) = angel_delve::world::hillside::of(&run) {
        println!("marks {:?}", v.marks.iter().map(|m| (m.heat, m.words.clone())).collect::<Vec<_>>());
        println!("buildings {:?}", v.buildings.iter().map(|b| (b.plan.clone(), format!("{:?}", b.status), b.crew)).collect::<Vec<_>>());
    }
    press(reel, &mut run, Input { dash: true, ..Default::default() })?;
    reel.wait(&mut run, 30)?;
    // The second line: from the quarry's south face to the second banker.
    tool(&mut run, Tool::Line(LineKind::Rope));
    go(reel, &mut run, (169, 104), Input::default())?;
    go(reel, &mut run, (171, 104), Input::default())?;
    press(reel, &mut run, Input { move_y: -1, ..Default::default() })?;
    reel.tick(&mut run, Input { interact: true, ..Default::default() })?;
    go(reel, &mut run, (171, 106), Input { interact: true, ..Default::default() })?;
    go(reel, &mut run, (158, 106), Input { interact: true, ..Default::default() })?;
    reel.tick(&mut run, Input { move_x: -1, interact: true, ..Default::default() })?;
    reel.tick(&mut run, Input::default())?;
    println!("second line: {}", run.kingdom.industry[&1].message);
    reel.wait(&mut run, 90)?;
    for (id, l) in &run.home.industry.lines {
        println!("line {id}: {:?} {} tiles from {:?} to {:?}; state {:?}", l.kind, l.path.len(), l.path.first(), l.path.last(), run.home.industry.line_report(c).iter().find(|r| r.id == *id).map(|r| r.state));
    }
    println!("lines {} frames {}", run.home.industry.lines.len(), reel.n);
    Ok(())
}

/// A large works: sixty buildings, thirty carts, a hundred people, and what a frame costs.
fn big(reel: &mut Reel) -> Result<(), String> {
    let mut run = hill(160.5, 108.5);
    let c = catalog();
    win_seam(&mut run);
    let yard = raise(&mut run, "carters-yard", (143, 98))?;
    let mut ids = vec![yard];
    // Every kind of work the hillside builds that its ground suits, round and round.
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
    let _ = k.apply(c, Command::Carts { yard, carts: 12 });
    println!("{} buildings, {} road tiles", run.home.industry.buildings.len(), run.home.industry.roads.len());
    reel.every = 30;
    reel.wait(&mut run, 120)?;
    press(reel, &mut run, Input { cast: 4, ..Default::default() })?;
    press(reel, &mut run, Input { dash: true, ..Default::default() })?;
    reel.wait(&mut run, 20)?;
    let times = TIMES.lock().unwrap().clone();
    if !times.is_empty() {
        let mut scene: Vec<u64> = times.iter().map(|t| t.0).collect();
        let mut stage: Vec<u64> = times.iter().map(|t| t.1).collect();
        scene.sort_unstable();
        stage.sort_unstable();
        println!("scene us median {} p95 {}; stage us median {} p95 {} over {} frames", scene[scene.len() / 2], scene[scene.len() * 95 / 100], stage[stage.len() / 2], stage[stage.len() * 95 / 100], scene.len());
    }
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
