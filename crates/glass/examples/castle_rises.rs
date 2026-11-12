//! The Keep rising, in the realm's own renderer (the Works, `world::works`):
//! the great tower on its site at chosen stages of the build, flat and in
//! 3D, as the window draws a Delve stage, offscreen.
//!
//! cargo run --release -p angel-glass --example castle_rises -- <outdir> [--scale N] shots
//! cargo run --release -p angel-glass --example castle_rises -- <outdir> [--scale N] rise <view> <frames> [<from> <to>]
//! cargo run --release -p angel-glass --example castle_rises -- <outdir> bench <view> [<laid>]
//!
//! `shots` writes `<view>-<laid>.png` and `<view>-<laid>-flat.png` for a
//! few views and stages. `rise` writes frames `<view>-NNNN.png` (3D) and
//! `<view>-NNNN-flat.png` with `laid` going from `from` to `to`. `bench`
//! prints what a frame costs at that view, flat and in 3D.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use angel_delve::render::overworld::arena::{self, stage_frame::StageMaker};
use angel_delve::world3d::{self, kingdom};
use angel_gpu::{Core, Draw, Frame, Instance, Picture, Stage};

/// The stage's picture id, as the cockpit names the Delve's.
const STAGE: u32 = 0x00FA_57A4;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("castle_rises: {why}");
            ExitCode::FAILURE
        }
    }
}

/// A view: where a knight stands, in realm tiles (the camera follows it).
fn view(name: &str) -> Option<(f32, f32)> {
    Some(match name {
        // In the bailey before the keep's south face, the gate behind.
        "bailey" => (133.0, 87.0),
        // The spawn in the lee of the gate.
        "spawn" => (133.0, 89.4),
        // Outside the ditch, the gate before the knight.
        "gate" => (133.0, 99.0),
        // The east flank, by the Delve's gate.
        "delve" => (140.0, 88.0),
        // North of the keep.
        "north" => (133.0, 66.0),
        // The west side of the bailey.
        "west" => (123.0, 80.0),
        // Close at the working face: the keep's west flank.
        "work" => (123.0, 84.0),
        // At the crane's foot, the yard and the benches south of it.
        "crane" => (127.0, 80.5),
        _ => return None,
    })
}

/// The tour's camera at `t` (0 to 1): from outside the gate, in across the
/// bailey, round the keep's west flank to its north and east, and back to the
/// bailey before the keep.
fn tour_at(t: f32) -> (f32, f32) {
    const WAY: [(f32, f32); 7] = [(133.0, 99.0), (133.0, 88.0), (124.0, 82.0), (126.0, 70.0), (140.0, 70.0), (141.0, 82.0), (134.0, 88.0)];
    let at = t.clamp(0.0, 1.0) * (WAY.len() - 1) as f32;
    let (i, f) = (at.floor() as usize, at.fract());
    let (a, b) = (WAY[i], WAY[(i + 1).min(WAY.len() - 1)]);
    // Eased between the waypoints: a camera that arrives, not one that glides.
    let e = f * f * (3.0 - 2.0 * f);
    (a.0 + (b.0 - a.0) * e, a.1 + (b.1 - a.1) * e)
}

fn feet(tile: (f32, f32)) -> [f32; 2] {
    [tile.0 * 16.0, tile.1 * 16.0 + 6.0]
}

/// The run: a knight at `tile`, the Keep `laid` stones up, settled.
fn site_run(tile: (f32, f32), laid: u32) -> angel_delve::rules::Run {
    let mut run = kingdom::proof_run(&[feet(tile)]);
    run.lay_to("great-tower", laid);
    run
}

fn usage() -> String {
    "usage: castle_rises <outdir> [--scale N] (shots | rise <view> <frames> [<from> <to>] | bench <view> [<laid>])".into()
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
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut gpu = Gpu::new()?;
    arena::draw_parts_outside(true);
    arena::props_meshes(true);
    let castle = angel_delve::world::works::castle("great-tower").ok_or("no great tower")?;
    let total = castle.len();
    match rest.first().map(String::as_str) {
        Some("shots") => {
            let stages: Vec<u32> = castle.stages.iter().filter(|s| s.count > 0).map(|s| s.first + s.count).collect();
            // From where it starts, a few lifts up, the roofs, the curtain, done.
            let at = [1_500, 3_000, 5_000, stages[7], stages[10], 9_500, total];
            for name in ["bailey", "crane", "gate", "delve", "north", "work"] {
                let tile = view(name).ok_or("a view")?;
                for &laid in &at {
                    shoot(&mut gpu, &out, name, laid, tile, scale, true)?;
                }
            }
            Ok(())
        }
        Some("rise") => {
            let name = rest.get(1).ok_or_else(usage)?;
            // `tour` walks the camera round the site as it rises.
            let tour = name == "tour";
            let tile = if tour { (133.0, 99.0) } else { view(name).ok_or_else(|| format!("no view {name}"))? };
            let frames: u32 = rest.get(2).and_then(|v| v.parse().ok()).ok_or_else(usage)?;
            let from: u32 = rest.get(3).and_then(|v| v.parse().ok()).unwrap_or(angel_delve::rules::site::START);
            let to: u32 = rest.get(4).and_then(|v| v.parse().ok()).unwrap_or(total);
            for f in 0..frames {
                let t = f64::from(f) / f64::from((frames - 1).max(1));
                let laid = from + ((to - from) as f64 * t) as u32;
                let tile = if tour { tour_at(t as f32) } else { tile };
                shoot_frame(&mut gpu, &out, name, f, laid, tile, scale)?;
            }
            Ok(())
        }
        Some("riseback") => {
            // The whole castle rising, stood back, from where it starts to done.
            let frames: u32 = rest.get(1).and_then(|v| v.parse().ok()).ok_or_else(usage)?;
            let tile = view("bailey").ok_or("a view")?;
            for f in 0..frames {
                let t = f64::from(f) / f64::from((frames - 1).max(1));
                let laid = angel_delve::rules::site::START + ((total - angel_delve::rules::site::START) as f64 * t) as u32;
                let run = site_run(tile, laid);
                for on in [true, false] {
                    frame_png(&mut gpu, &out, &format!("riseback-{f:04}"), &run, on, scale, true)?;
                }
                println!("riseback {f:04}: laid {laid}");
            }
            Ok(())
        }
        Some("bands") => bands(&mut gpu, &out, scale),
        Some("back") => {
            // The stand-back camera, the map key held at the site, at a few stages.
            let name = rest.get(1).ok_or_else(usage)?;
            let tile = view(name).ok_or_else(|| format!("no view {name}"))?;
            for laid in [1_500, 5_000, 8_304, total] {
                let run = site_run(tile, laid);
                frame_png(&mut gpu, &out, &format!("back-{name}-{laid:05}"), &run, true, scale, true)?;
                frame_png(&mut gpu, &out, &format!("back-{name}-{laid:05}"), &run, false, scale, true)?;
                println!("back {name} {laid}");
            }
            Ok(())
        }
        Some("walk") => {
            // A knight walks in at the gate and across the bailey to the Delve's stair.
            let laid: u32 = rest.get(1).and_then(|v| v.parse().ok()).ok_or_else(usage)?;
            let every: usize = rest.get(2).and_then(|v| v.parse().ok()).unwrap_or(4);
            walk(&mut gpu, &out, laid, every, scale)
        }
        Some("chronicle") => {
            let name = rest.get(1).ok_or_else(usage)?;
            let tile = view(name).ok_or_else(|| format!("no view {name}"))?;
            let every: u64 = rest.get(2).and_then(|v| v.parse().ok()).unwrap_or(20);
            chronicle(&mut gpu, &out, name, tile, every, scale)
        }
        Some("bench") => {
            let name = rest.get(1).ok_or_else(usage)?;
            let tile = view(name).ok_or_else(|| format!("no view {name}"))?;
            let laid: u32 = rest.get(2).and_then(|v| v.parse().ok()).unwrap_or(total);
            bench(&mut gpu, name, tile, laid, scale)
        }
        _ => Err(usage()),
    }
}

/// The stand-back camera, the keep banded as the delves paid, rising.
fn bands(gpu: &mut Gpu, out: &Path, scale: u32) -> Result<(), String> {
    let tile = view("bailey").ok_or_else(|| "no bailey".to_string())?;
    let total = 4_655u32;
    let frames = 24u32;
    for n in 0..=frames {
        let laid = (total * n / frames).max(1);
        let mut run = site_run(tile, laid);
        if let Some(site) = run.sites.keep_mut() {
            let cuts = [laid / 4, laid / 2, laid * 3 / 4, laid];
            site.bands = cuts.iter().zip([0u8, 1, 2, 3]).filter(|(end, _)| **end > 0).map(|(end, kind)| (*end, kind)).collect();
        }
        frame_png(gpu, out, &format!("rise-{n:02}"), &run, true, scale, true)?;
        if n == 0 || n == frames / 4 || n == frames / 2 || n == frames {
            let tag = format!("{:03}", n * 100 / frames);
            frame_png(gpu, out, &format!("still-{tag}"), &run, true, scale, true)?;
            frame_png(gpu, out, &format!("still-{tag}"), &run, false, scale, true)?;
        }
        println!("rise {n} laid {laid}");
    }
    Ok(())
}

/// One stage of the build at one view, flat and in 3D.
fn shoot(gpu: &mut Gpu, out: &Path, name: &str, laid: u32, tile: (f32, f32), scale: u32, both: bool) -> Result<(), String> {
    let run = site_run(tile, laid);
    let (w, h) = arena::seat_size();
    for on in [false, true].into_iter().filter(|&on| on || both) {
        world3d::set_enabled(on);
        let scene = Arc::new(arena::seat_scene(&run, w as i32, h as i32, Some(1)));
        let mut maker = StageMaker::default();
        let mut frame = maker.make(&scene, |_| None);
        gpu.upload(std::mem::take(&mut frame.pictures));
        frame = maker.make(&scene, |_| None);
        let back = (frame.size.0 / 384).max(1);
        let size = (frame.size.0 * scale / back, frame.size.1 * scale / back);
        let pixels = gpu.stage(frame, size, true);
        let file = if on { format!("{name}-{laid:05}.png") } else { format!("{name}-{laid:05}-flat.png") };
        write_png(&out.join(&file), size, &pixels)?;
        println!("{file}");
    }
    world3d::set_enabled(false);
    Ok(())
}

fn shoot_frame(gpu: &mut Gpu, out: &Path, name: &str, n: u32, laid: u32, tile: (f32, f32), scale: u32) -> Result<(), String> {
    let run = site_run(tile, laid);
    let (w, h) = arena::seat_size();
    for on in [false, true] {
        world3d::set_enabled(on);
        let scene = Arc::new(arena::seat_scene(&run, w as i32, h as i32, Some(1)));
        let mut maker = StageMaker::default();
        let mut frame = maker.make(&scene, |_| None);
        gpu.upload(std::mem::take(&mut frame.pictures));
        frame = maker.make(&scene, |_| None);
        let back = (frame.size.0 / 384).max(1);
        let size = (frame.size.0 * scale / back, frame.size.1 * scale / back);
        let pixels = gpu.stage(frame, size, true);
        let file = if on { format!("{name}-{n:04}.png") } else { format!("{name}-{n:04}-flat.png") };
        write_png(&out.join(&file), size, &pixels)?;
    }
    world3d::set_enabled(false);
    println!("{name} {n:04}: laid {laid}");
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

/// A knight walking through the site: in at the gate, across the bailey to
/// the Delve's stair, the world stepping as it walks (the crane turns, the
/// crew works), a frame every `every` ticks, flat and in 3D.
fn walk(gpu: &mut Gpu, out: &Path, laid: u32, every: usize, scale: u32) -> Result<(), String> {
    use angel_delve::rules::Input;
    use std::collections::BTreeMap;
    let waypoints = [(133.0f32, 99.0f32), (133.0, 93.0), (133.0, 88.0), (136.0, 88.0), (139.0, 87.5), (140.0, 87.2)];
    let mut run = kingdom::proof_run(&[feet(waypoints[0])]);
    run.lay_to("great-tower", laid);
    for _ in 0..20 {
        run.step(&BTreeMap::new());
    }
    let mut n = 0;
    let mut tick = 0usize;
    for &(wx, wy) in &waypoints[1..] {
        let (gx, gy) = ((wx + 0.5) * 2.0, (wy + 0.7) * 2.0);
        for _ in 0..600 {
            let hero = &run.players[&1];
            let (dx, dy) = (gx - hero.x, gy - hero.y);
            if dx.hypot(dy) < 0.4 {
                break;
            }
            let sign = |d: f32| if d > 0.3 { 1 } else if d < -0.3 { -1 } else { 0 };
            let input = Input { move_x: sign(dx), move_y: sign(dy), ..Default::default() };
            run.step(&BTreeMap::from([(1, input)]));
            tick += 1;
            if tick % every == 0 {
                for on in [true, false] {
                    frame_png(gpu, out, &format!("walk-{n:04}"), &run, on, scale, false)?;
                }
                n += 1;
            }
        }
    }
    let (hx, hy) = (run.players[&1].x / 2.0, run.players[&1].y / 2.0);
    println!("walk: {n} frames, ended at tile ({hx:.1}, {hy:.1}), {} stones laid", run.sites.keep().map_or(0, |s| s.laid));
    // A knight that did not get through is a wall that should not be there.
    let (gx, gy) = (waypoints[waypoints.len() - 1].0 + 0.5, waypoints[waypoints.len() - 1].1 + 0.7);
    if (hx - gx).hypot(hy - gy) > 1.0 {
        return Err(format!("the knight stopped at ({hx:.1}, {hy:.1}), short of ({gx}, {gy})"));
    }
    Ok(())
}

/// The Chronicle: the whole build replayed bare hillside to today in thirty
/// seconds, a frame every `every` ticks, flat and in 3D.
fn chronicle(gpu: &mut Gpu, out: &Path, name: &str, tile: (f32, f32), every: u64, scale: u32) -> Result<(), String> {
    use std::collections::BTreeMap;
    let mut run = site_run(tile, 11_266);
    for _ in 0..10 {
        run.step(&BTreeMap::new());
    }
    run.site_replay = Some(run.tick);
    let mut n = 0;
    for t in 0..angel_delve::rules::site::CHRONICLE_TICKS {
        run.step(&BTreeMap::new());
        if t % every == 0 {
            for on in [true, false] {
                frame_png(gpu, out, &format!("{name}-chronicle-{n:04}"), &run, on, scale, false)?;
            }
            n += 1;
        }
    }
    println!("chronicle: {n} frames");
    Ok(())
}

/// What a frame costs at a view: the scene and the stage made (CPU), and
/// that with the GPU drawn and waited for, flat and in 3D, the first frame
/// and the steady ones.
fn bench(gpu: &mut Gpu, name: &str, tile: (f32, f32), laid: u32, scale: u32) -> Result<(), String> {
    let (w, h) = arena::seat_size();
    for (label, laid) in [("without the works", None), ("the site", Some(laid))] {
        for on in [false, true] {
            world3d::set_enabled(on);
            let mut run = kingdom::proof_run(&[feet(tile)]);
            match laid {
                Some(laid) => run.lay_to("great-tower", laid),
                None => run.sites.0.clear(),
            }
            let mut maker = StageMaker::default();
            let (mut cpu, mut all) = (Vec::new(), Vec::new());
            let mut first = 0.0;
            for k in 0..90 {
                run.tick += 1;
                let t = Instant::now();
                let scene = Arc::new(arena::seat_scene(&run, w as i32, h as i32, Some(1)));
                let frame = maker.make(&scene, |_| None);
                let made = t.elapsed().as_secs_f64() * 1e3;
                if k == 0 {
                    first = made;
                }
                let back = (frame.size.0 / 384).max(1);
        let size = (frame.size.0 * scale / back, frame.size.1 * scale / back);
                let _ = gpu.stage(frame, size, false);
                if k >= 10 {
                    cpu.push(made);
                    all.push(t.elapsed().as_secs_f64() * 1e3);
                }
            }
            let stats = |v: &mut Vec<f64>| {
                v.sort_by(f64::total_cmp);
                (v.iter().sum::<f64>() / v.len().max(1) as f64, v[v.len() * 95 / 100], v[v.len() - 1])
            };
            let (cm, c95, cmax) = stats(&mut cpu);
            let (am, a95, amax) = stats(&mut all);
            println!(
                "{name} laid {} {label} {}: first frame made in {first:.1} ms; steady: made {cm:.2} ms mean, {c95:.2} p95, {cmax:.2} most; made and drawn {am:.2} ms mean, {a95:.2} p95, {amax:.2} most",
                laid.unwrap_or(0),
                if on { "3D" } else { "flat" },
            );
        }
    }
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
