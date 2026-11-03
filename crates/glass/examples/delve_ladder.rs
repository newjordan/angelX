//! The resolution ladder (docs/LADDER.md) shot as the window draws it: the
//! same room with the same four knights, one a seat, at every rung (16, 32,
//! 64, 128), by the machinery the window draws a Delve frame with (the
//! Delve's `StageMaker` and its HD drawer, the GPU core offscreen). The rung
//! is fixed for each shot (`rules::resolution::set_rule`); the knights' names
//! find their drawings in the pack (`--hd`, as `ANGEL_GLASS_ART` gives the
//! window), the finest each rung allows.
//!
//! cargo run --release -p angel-glass --example delve_ladder -- <outdir> --hd PACK [--scale N] [--left] [--lit] [--stride S] [--swing N] [--time N]
//!
//! Writes `room-<rung>.png` (the room's view at `N` window pixels to a pixel
//! of the Delve's 384 x 224, 8 unless said: a 128 rung's drawing shows every
//! pixel), `seats-<rung>.png` (the four knights cut out of it), and
//! `ladder.png` (the four knights at each rung, a row a rung). `--left`
//! turns the knights to face left (their drawings the right's mirrored).
//! `--lit` stands the room up in 3D, lit (what the window shows from the
//! deep below the dragon, floor four, where the 64 rung begins): the
//! knights are lit cards, their drawings cards of their own. `--stride S`
//! has them on their walk, `S` units along it (3.4 a cycle, eight frames);
//! `--swing N` mid-stroke, `N` ticks of the slash left (of 6).
//! `--time N` makes each rung's frame `N` times and says what it costs, and
//! what its drawings weigh.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_delve::render::overworld::arena::{self, NATIVE_H, NATIVE_W, UNIT, hd_pack, stage_frame::StageMaker};
use angel_delve::rules::resolution::{self, Rule, Rung};
use angel_delve::rules::{Pack, RoomKind, Rng, Run, layout};
use angel_gpu::{Core, Draw, Frame, Instance, Picture, Stage};

/// The stage's picture id, as the cockpit names the Delve's.
const STAGE: u32 = 0x00FA_57A4;
/// How far apart the knights stand, in the room's pixels.
const APART: f32 = 30.0;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("delve_ladder: {why}");
            ExitCode::FAILURE
        }
    }
}

/// The Crypt's first fight room, four seats in it standing in a row, idle,
/// facing right (or left), nothing else alive.
fn four_knights(left: bool, stride: f32, swing: u32) -> Option<Run> {
    for seed in 3..11u64 {
        let floor = layout::floor_party(1, Pack::Crypt, 4, &mut Rng::new(seed));
        let Some(at) = floor.rooms.iter().position(|r| r.kind == RoomKind::Fight) else {
            continue;
        };
        let mut run = Run::new(seed, seed, Some("Friend"));
        for (id, name) in [(2, "Goose Inspector"), (3, "Marrow"), (4, "Tansy")] {
            run.join_seat(id, name);
        }
        run.dungeon = floor;
        run.enter_for_test(at);
        run.enemies.clear();
        let room = run.room().clone();
        let (w, h) = (room.cols as f32 * 16.0, room.rows as f32 * 16.0);
        let first = (w / 2.0 - APART * 1.5, h / 2.0 + 8.0);
        for (k, hero) in run.players.values_mut().enumerate() {
            (hero.x, hero.y) = ((first.0 + k as f32 * APART) / UNIT, first.1 / UNIT);
            (hero.aim_x, hero.aim_y) = (if left { -1.0 } else { 1.0 }, 0.0);
            hero.stride = stride;
            hero.swing = swing;
            hero.camera = Some(((w / 2.0) / UNIT, (h / 2.0) / UNIT));
        }
        return Some(run);
    }
    None
}

fn run() -> Result<(), String> {
    let usage = "usage: delve_ladder <outdir> --hd PACK [--scale N] [--left] [--lit] [--time N]";
    let mut words = std::env::args().skip(1);
    let (mut out, mut scale, mut left, mut time, mut pack, mut lit) = (None, 8u32, false, 0u32, None, false);
    let (mut stride, mut swing) = (0.0f32, 0u32);
    while let Some(word) = words.next() {
        match word.as_str() {
            "--scale" => scale = words.next().and_then(|w| w.parse().ok()).filter(|&s| (1..=8).contains(&s)).ok_or("--scale takes 1 to 8")?,
            "--left" => left = true,
            "--lit" => lit = true,
            "--stride" => stride = words.next().and_then(|w| w.parse().ok()).ok_or("--stride takes units")?,
            "--swing" => swing = words.next().and_then(|w| w.parse().ok()).ok_or("--swing takes ticks")?,
            "--time" => time = words.next().and_then(|w| w.parse().ok()).filter(|&n| n > 0).ok_or("--time takes a count")?,
            "--hd" => pack = Some(PathBuf::from(words.next().ok_or(usage)?)),
            _ if out.is_none() => out = Some(PathBuf::from(word)),
            other => return Err(format!("`{other}`: {usage}")),
        }
    }
    let out = out.ok_or(usage)?;
    let dir = pack.ok_or(usage)?;
    let opened = std::time::Instant::now();
    let pack = hd_pack::Pack::open(&dir).ok_or_else(|| format!("{}: no HD art pack", dir.display()))?;
    let opened = opened.elapsed().as_secs_f64() * 1e3;
    for warning in pack.warnings().iter().take(8) {
        eprintln!("hd pack: left out {warning}");
    }
    println!("hd pack {}: {} named drawings of {} keys, read in {opened:.1} ms", dir.display(), pack.named_len(), pack.keys().count());
    let pack = Arc::new(pack);
    hd_pack::install(Some(pack.clone()));
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut gpu = Gpu::new()?;
    arena::draw_parts_outside(true);
    angel_delve::world3d::set_enabled(lit);
    arena::props_meshes(lit);
    let run = four_knights(left, stride, swing).ok_or("no fight room on the Crypt's first floor")?;
    let size = (NATIVE_W as u32 * scale, NATIVE_H as u32 * scale);
    let mut rows = Vec::new();
    for rung in Rung::ALL {
        resolution::set_rule(Some(Rule::Fixed(rung)));
        let scene = Arc::new(arena::in_seat(|| arena::scene_for(&run, NATIVE_W, NATIVE_H, Some(1))));
        // Where the knights stand on the view, for the cut-out.
        let named: Vec<_> = scene.figures.iter().flat_map(|f| &f.bodies).filter_map(|b| Some((b, b.named.as_ref()?))).collect();
        let mut art = hd_pack::HdArt::default();
        art.normals = lit;
        let mut drawn = Vec::new();
        let mut frame = StageMaker::default().make(&scene, |body| art.picture_from(&pack, body, &mut drawn));
        let bytes: usize = drawn.iter().map(|p| p.rgba.len()).sum();
        let shown: Vec<String> = drawn.iter().map(|p| format!("{}x{}", p.width, p.height)).collect();
        println!(
            "{rung}: {} named knights ({}), {} drawings sent ({}; {} KB of pixels)",
            named.len(),
            named.iter().map(|(_, n)| n.key.to_string()).collect::<Vec<_>>().join(", "),
            drawn.len(),
            shown.join(" "),
            bytes / 1024
        );
        frame.pictures.extend(drawn);
        if time > 0 {
            let start = std::time::Instant::now();
            for _ in 0..time {
                let mut art = hd_pack::HdArt::default();
                let mut drawn = Vec::new();
                std::hint::black_box(StageMaker::default().make(&scene, |body| art.picture_from(&pack, body, &mut drawn)));
            }
            let first = start.elapsed().as_secs_f64() * 1e3 / f64::from(time);
            let mut maker = StageMaker::default();
            let mut art = hd_pack::HdArt::default();
            let mut drawn = Vec::new();
            let _ = maker.make(&scene, |body| art.picture_from(&pack, body, &mut drawn));
            let start = std::time::Instant::now();
            for _ in 0..time {
                std::hint::black_box(maker.make(&scene, |body| art.picture_from(&pack, body, &mut drawn)));
            }
            let kept = start.elapsed().as_secs_f64() * 1e3 / f64::from(time);
            println!("{rung}: a stage frame {first:.3} ms new (drawings sent), {kept:.3} ms after (drawings kept), {time} frames");
        }
        let pixels = gpu.stage(frame, size);
        write_png(&out.join(format!("room-{}.png", rung.px())), size, &pixels)?;
        // The four knights cut out: from the first's left to the last's
        // right, head room over them.
        let (cx, cy) = scene.camera;
        let xs = named.iter().map(|(b, _)| b.x as f32 - cx);
        let x0 = xs.clone().fold(f32::INFINITY, f32::min) - 4.0;
        let x1 = named.iter().map(|(b, _)| (b.x + b.img.w) as f32 - cx).fold(f32::NEG_INFINITY, f32::max) + 4.0;
        let y1 = named.iter().map(|(b, _)| (b.y + b.img.h) as f32 - cy).fold(f32::NEG_INFINITY, f32::max) + 4.0;
        let y0 = y1 - 52.0;
        let cut = [x0.max(0.0), y0.max(0.0), x1.min(NATIVE_W as f32), y1.min(NATIVE_H as f32)].map(|v| (v * scale as f32) as u32);
        let (cw, ch) = (cut[2] - cut[0], cut[3] - cut[1]);
        let mut cropped = Vec::with_capacity((cw * ch * 4) as usize);
        for y in cut[1]..cut[3] {
            let at = ((y * size.0 + cut[0]) * 4) as usize;
            cropped.extend_from_slice(&pixels[at..at + (cw * 4) as usize]);
        }
        write_png(&out.join(format!("seats-{}.png", rung.px())), (cw, ch), &cropped)?;
        rows.push(((cw, ch), cropped));
    }
    resolution::set_rule(None);
    // A row a rung, the rung's pixels on its left.
    let width = rows.iter().map(|((w, _), _)| *w).max().unwrap_or(1);
    let height: u32 = rows.iter().map(|((_, h), _)| *h + 4).sum();
    let mut sheet = vec![0u8; (width * height * 4) as usize];
    let mut y0 = 0;
    for ((w, h), px) in &rows {
        for y in 0..*h {
            let from = (y * w * 4) as usize;
            let to = (((y0 + y) * width) * 4) as usize;
            sheet[to..to + (*w * 4) as usize].copy_from_slice(&px[from..from + (*w * 4) as usize]);
        }
        y0 += h + 4;
    }
    write_png(&out.join("ladder.png"), (width, height), &sheet)?;
    println!("wrote the ladder to {}", out.display());
    Ok(())
}

struct Gpu {
    core: Core,
    device: wgpu::Device,
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
            label: Some("delve-ladder"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;
        device.on_uncaptured_error(Arc::new(|error: wgpu::Error| eprintln!("GPU error: {error}")));
        let core = Core::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
        Ok(Self { core, device })
    }

    /// A Delve frame as a stage, drawn into a texture `size` pixels as the
    /// window draws a stage in its cells, on black, and read back.
    fn stage(&mut self, frame: angel_stage::Frame, size: (u32, u32)) -> Vec<u8> {
        for p in frame.pictures {
            self.core.upload(Picture { id: p.id, width: p.width, height: p.height, rgba: p.rgba, cols: 0, rows: 0 });
        }
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
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("delve-ladder-target"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
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
        self.core.draw(&view, size, wgpu::Color::BLACK, &drawn, "delve-ladder");
        self.core.read_back(&texture, size)
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
