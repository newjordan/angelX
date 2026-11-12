//! The Deep comes up, in the realm's own 3D renderer: a first raid at night (imps, a works alight), a wall that holds
//! with a watchtower shooting, and the Keep's stones knocked out of the lift in hand. Every frame is the game's stage
//! (`StageMaker`) drawn offscreen by the GPU (`NNNN.png`), the knights' own state stepped by the game's rules.
//!
//! ANGEL_REALM_PLAN=tabula cargo run --profile fast -p angel-glass --example threat_proof -- <outdir> [--scale N] <scene>
//!
//! Scenes (frames go in `<outdir>/<scene>/`): `raid`, `wall`, `siege`, `pieces` (the new works, standing alone).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_delve::kingdom::{Command, Stock, catalog};
use angel_delve::render::overworld::arena::{self, stage_frame::StageMaker};
use angel_delve::rules::{Input, Run, TILE_UNITS, deep_raid};
use angel_delve::world::works;
use angel_delve::world3d;
use angel_gpu::{Core, Draw, Frame, Instance, Picture, Stage};

const STAGE: u32 = 0x00FA_57A4;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("threat_proof: {why}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> String {
    "usage: threat_proof <outdir> [--scale N] (raid | wall | siege | pieces)".into()
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out: PathBuf = args.next().ok_or_else(usage)?.into();
    let mut scale = 3u32;
    let mut rest = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--scale" => scale = args.next().and_then(|v| v.parse().ok()).ok_or_else(usage)?,
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
    let gpu = Gpu::new()?;
    arena::draw_parts_outside(true);
    arena::props_meshes(true);
    let mut reel = Reel { gpu, dir, scale, every: 15, n: 0, ticks: 0, look: None };
    match scene.as_str() {
        "raid" => raid(&mut reel),
        "wall" => wall(&mut reel),
        "siege" => siege(&mut reel),
        "pieces" => pieces(&mut reel),
        _ => Err(usage()),
    }
}

struct Reel {
    gpu: Gpu,
    dir: PathBuf,
    scale: u32,
    every: u64,
    n: u32,
    ticks: u64,
    look: Option<(f32, f32)>,
}

impl Reel {
    fn tick(&mut self, run: &mut Run) -> Result<(), String> {
        run.step(&BTreeMap::from([(1, Input::default())]));
        if let Some((x, y)) = self.look {
            run.players.get_mut(&1).unwrap().camera = Some((x * TILE_UNITS, y * TILE_UNITS));
        }
        self.ticks += 1;
        if self.ticks % self.every == 0 {
            self.shoot(run)?;
        }
        Ok(())
    }

    fn shoot(&mut self, run: &Run) -> Result<(), String> {
        let name = format!("{:04}", self.n);
        frame_png(&mut self.gpu, &self.dir, &name, run, self.scale)?;
        self.n += 1;
        Ok(())
    }

    fn wait(&mut self, run: &mut Run, seconds: u64) -> Result<(), String> {
        for _ in 0..seconds * 30 {
            self.tick(run)?;
        }
        Ok(())
    }
}

/// A rich hillside at the dusk of a day, the Keep staked, a knight at `tile` who cannot be felled.
fn hill(tile: (f32, f32)) -> Run {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(None);
    run.home.industry.found_hillside(catalog(), 7);
    let k = &mut run.home.industry;
    for r in catalog().resources.keys() {
        k.stock.insert(r.clone(), 600);
    }
    k.stock.insert("fodder".into(), 100_000);
    k.stock.remove("coin");
    k.researched = catalog().all_techs().map(|(k, _)| k.clone()).collect();
    let h = run.players.get_mut(&1).expect("a knight");
    (h.x, h.y) = (tile.0 * TILE_UNITS, tile.1 * TILE_UNITS);
    h.camera = Some((h.x, h.y - 1.0));
    h.invulnerable = 1_000_000;
    h.stone = false;
    for _ in 0..30 * 12 {
        run.step(&BTreeMap::from([(1, Input::default())]));
    }
    for b in run.home.industry.buildings.values_mut() {
        b.construction = 0;
    }
    run.home.industry.stock.remove("coin");
    run
}

fn raise(run: &mut Run, plan: &str, tile: (i16, i16)) -> Result<u32, String> {
    let k = &mut run.home.industry;
    k.apply(catalog(), Command::Build { plan: plan.into(), tile }).map_err(|e| format!("{plan} at {tile:?}: {e}"))?;
    let id = k.at(tile).ok_or("it did not stand")?;
    k.buildings.get_mut(&id).ok_or("it did not stand")?.construction = 0;
    Ok(id)
}

fn dusk(run: &mut Run, day: u64) {
    run.home.industry.second = day * deep_raid::DAY + deep_raid::DUSK + deep_raid::JUDGE_AFTER - 3;
}

fn report(run: &Run, what: &str) {
    let k = &run.home.industry;
    println!("{what}: raid {:?} fires {} worn {} cuts {} tally {:?} lost stones {}", run.raid_status(), k.havoc.fires.len(), k.havoc.worn.len(), k.havoc.cuts.len(), k.havoc.tally, run.sites.keep().map_or(0, |s| s.lost.len()));
}

/// The new works alone, as a camp would raise them.
fn pieces(reel: &mut Reel) -> Result<(), String> {
    let mut run = hill((150.0, 100.0));
    run.home.industry.stock.insert("timber".into(), 900);
    for (i, plan) in ["palisade", "palisade", "palisade", "wood-gate", "watchtower", "brazier"].iter().enumerate() {
        let x = 144 + 4 * i as i16;
        raise(&mut run, plan, (x, 100))?;
    }
    run.home.industry.buildings.iter_mut().filter(|(_, b)| b.plan == "watchtower").for_each(|(_, b)| {
        b.input.insert("arrow".into(), 24);
    });
    reel.look = Some((152.0, 100.0));
    reel.every = 30;
    reel.wait(&mut run, 6)?;
    Ok(())
}

/// A first raid at night: two imps climb the Great Shaft, march for the kiln and set it alight.
fn raid(reel: &mut Reel) -> Result<(), String> {
    let c = catalog();
    let mut run = hill((146.5, 97.5));
    let kiln = raise(&mut run, "lime-kiln", (140, 100))?;
    raise(&mut run, "sawpit", (134, 100))?;
    {
        let k = &mut run.home.industry;
        k.apply(c, Command::Connect(kiln)).map_err(|e| e.to_string())?;
        k.buildings.get_mut(&kiln).ok_or("no kiln")?.recipe = Some("burn-lime-coal".into());
        k.buildings.get_mut(&kiln).ok_or("no kiln")?.input = Stock::from([("limestone".into(), 32), ("coal".into(), 8)]);
    }
    run.kingdom.stir.level = 80;
    reel.look = Some((141.0, 96.0));
    reel.every = 20;
    reel.wait(&mut run, 4)?;
    dusk(&mut run, 3);
    for _ in 0..60 {
        reel.wait(&mut run, 1)?;
        if !run.home.industry.havoc.fires.is_empty() {
            break;
        }
    }
    report(&run, "alight");
    reel.wait(&mut run, 6)?;
    Ok(())
}

/// A ring of stone wall round a brazier, a watchtower by it with its quiver: five imps come at it.
fn wall(reel: &mut Reel) -> Result<(), String> {
    let c = catalog();
    let mut run = hill((162.0, 108.0));
    let brazier = raise(&mut run, "brazier", (153, 101))?;
    run.home.industry.stock.insert("timber".into(), 800);
    let ring: Vec<(i16, i16)> = (150..=156).flat_map(|x| [(x, 97), (x, 104)]).chain((98..=103).flat_map(|y| [(150, y), (156, y)])).collect();
    for t in &ring {
        raise(&mut run, "wall", *t)?;
    }
    for at in [(158, 100), (147, 100)] {
        let t = raise(&mut run, "watchtower", at)?;
        run.home.industry.apply(c, Command::Connect(t)).ok();
        run.home.industry.buildings.get_mut(&t).ok_or("no tower")?.input.insert("arrow".into(), 24);
    }
    run.kingdom.stir.level = 100;
    run.kingdom.stir.watch.raids = 4;
    reel.look = Some((153.0, 99.0));
    reel.every = 12;
    reel.wait(&mut run, 4)?;
    dusk(&mut run, 3);
    for _ in 0..80 {
        reel.wait(&mut run, 1)?;
        if run.kingdom.stir.watch.on.is_none() && run.kingdom.stir.watch.raids > 4 {
            break;
        }
    }
    report(&run, "after");
    let _ = brazier;
    reel.wait(&mut run, 3)
}

/// Warboars at the Keep's wall: stones knocked out of the lift in hand.
fn siege(reel: &mut Reel) -> Result<(), String> {
    let mut run = hill((140.0, 90.0));
    raise(&mut run, "brazier", (136, 99))?;
    run.home.industry.stock.insert("timber".into(), 800);
    run.kingdom.stir.level = 100;
    run.kingdom.stir.watch.raids = 3;
    let laid = run.sites.keep().ok_or("no keep")?.laid;
    let castle = works::castle(works::KEEP).ok_or("no castle")?;
    let last = castle.stones[(laid - 1) as usize].aabb().map(|v| f32::from(v) / 100.0);
    let (ox, oz) = works::origin_px();
    let at = ((ox + (last[0] + last[3]) / 2.0 * works::PX_PER_M) / 16.0, (oz - (last[1] + last[4]) / 2.0 * works::PX_PER_M) / 16.0);
    reel.look = Some((at.0, at.1 + 3.0));
    reel.every = 20;
    reel.wait(&mut run, 2)?;
    dusk(&mut run, 3);
    for _ in 0..120 {
        reel.wait(&mut run, 1)?;
        if run.sites.keep().map_or(0, |s| s.lost.len()) >= 4 {
            break;
        }
    }
    report(&run, "stones out");
    reel.wait(&mut run, 4)
}

fn frame_png(gpu: &mut Gpu, out: &Path, name: &str, run: &Run, scale: u32) -> Result<(u32, u32), String> {
    let (w, h) = arena::seat_size();
    world3d::set_enabled(true);
    let map = arena::MapView::default();
    let scene = Arc::new(arena::seat_scene_with(run, w as i32, h as i32, Some(1), &map));
    let mut maker = StageMaker::default();
    let mut frame = maker.make(&scene, |_| None);
    gpu.upload(std::mem::take(&mut frame.pictures));
    frame = maker.make(&scene, |_| None);
    let back = (frame.size.0 / 384).max(1);
    let size = (frame.size.0 * scale / back, frame.size.1 * scale / back);
    let pixels = gpu.stage(frame, size, true);
    write_png(&out.join(format!("{name}.png")), size, &pixels)?;
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

