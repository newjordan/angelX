//! A busy Delve fight as the window draws it, timed: four knights firing in
//! a fight room full of monsters, their shots and sparks in the air, pickups
//! on the floor, the frames made by the Delve (`StageMaker`) and drawn by
//! the GPU core offscreen at the window's density, as the window draws the
//! Delve's stage. The renderer's frame-time harness (/work notes: PERF.md).
//!
//! cargo run --release -p angel-glass --example delve_busy -- [<out.png>] [--frames N] [--scale N] [--post off|on] [--shake TICKS]
//!
//! Prints, over N frames (240 unless said), the median and the 95th
//! percentile of: making the stage frame (`make`), the core's CPU side of
//! drawing it (`render`: layers, instances, scenes, the post pass, encoded
//! and submitted) and the wait for the GPU to finish it (`gpu`), and the
//! frame's layers, nodes and picture bytes. With `<out.png>` the last frame
//! is written there.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use angel_delve::render::overworld::arena::{self, stage_frame::StageMaker};
use angel_delve::rules::{Input, Item, RoomKind, Run, Tile, cards::Book};
use angel_gpu::{Core, Draw, Frame, Instance, Picture, Stage};

/// The stage's picture id, as the cockpit names the Delve's.
const STAGE: u32 = 0x00FA_57A4;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("delve_busy: {why}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let usage = "usage: delve_busy [<out.png>] [--frames N] [--scale N] [--post off|on] [--shake TICKS]";
    let (mut out, mut frames, mut scale, mut post, mut shake) = (None::<PathBuf>, 240usize, 3u32, None::<bool>, 0u32);
    let mut words = std::env::args().skip(1);
    while let Some(word) = words.next() {
        match word.as_str() {
            "--frames" => frames = words.next().and_then(|w| w.parse().ok()).ok_or(usage)?,
            "--scale" => scale = words.next().and_then(|w| w.parse().ok()).filter(|&s| (1..=8).contains(&s)).ok_or(usage)?,
            "--post" => post = Some(words.next().ok_or(usage)? == "on"),
            // The last frame drawn with the screen shaking this many ticks
            // more (a guardian's fall is 12): the post pass's impact.
            "--shake" => shake = words.next().and_then(|w| w.parse().ok()).ok_or(usage)?,
            _ if out.is_none() => out = Some(PathBuf::from(word)),
            other => return Err(format!("`{other}`: {usage}")),
        }
    }
    let mut gpu = Gpu::new()?;
    if let Some(on) = post {
        gpu.core.set_post(on);
    }
    arena::draw_parts_outside(true);
    arena::pickup_cards(true);
    let mut run = Run::new(11, 1, Some("Ash"));
    let index = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .ok_or("no fight room")?;
    run.enter_for_test(index);
    for (id, name) in [(2, "Bryn"), (3, "Cass"), (4, "Dain")] {
        run.join_seat(id, name);
    }
    // Pickups on the floor about the party.
    let book = Book::builtin();
    let (hx, hy) = run.players.values().next().map(|h| (h.x, h.y)).ok_or("no knight")?;
    {
        let at = run.at;
        let room = &mut run.dungeon.rooms[at];
        let mut n = 0;
        'rows: for dy in -4..=4 {
            for dx in -7..=7 {
                if (dx + dy) % 3 != 0 || (dx == 0 && dy == 0) {
                    continue;
                }
                let (x, y) = (hx + dx as f32 * 1.4, hy + dy as f32 * 1.2);
                if room.tile((x / 2.0) as i32, (y / 2.0) as i32) == Tile::Floor {
                    let card = book.cards[(n * 7) % book.cards.len()].id.clone();
                    room.items.push(Item { card, x, y, held_off: None, took: Vec::new() });
                    n += 1;
                    if n >= 16 {
                        break 'rows;
                    }
                }
            }
        }
    }
    let (w, h) = arena::seat_size();
    let mut maker = StageMaker::default();
    maker.flat_shots = false;
    let (mut make_ms, mut render_ms, mut gpu_ms) = (Vec::new(), Vec::new(), Vec::new());
    let (mut layers, mut nodes, mut bytes, mut shots) = (0usize, 0usize, 0usize, 0usize);
    let warm = 30;
    let mut total_bytes = 0usize;
    let mut by_id: BTreeMap<u32, (usize, usize, (u32, u32))> = BTreeMap::new();
    let mut last = None;
    for k in 0..frames + warm {
        // Every knight moving and firing, each its own way round.
        let mut inputs = BTreeMap::new();
        for (n, id) in run.players.keys().copied().enumerate() {
            let turn = (k / 20 + n * 2) % 8;
            let dirs = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];
            let (ax, ay) = dirs[turn];
            let (mx, my) = dirs[(turn + 2) % 8];
            inputs.insert(id, Input { aim_x: ax, aim_y: ay, move_x: mx * ((k / 40) % 2) as i8, move_y: my * ((k / 40) % 2) as i8, fire: true, ..Default::default() });
        }
        run.step(&inputs);
        // Keep the fight going: monsters come back when the room empties.
        let at = run.at;
        if run.dungeon.rooms[at].cleared && k % 120 == 0 {
            run.dungeon.rooms[at].cleared = false;
        }
        for hero in run.players.values_mut() {
            hero.hp = hero.hp.max(1);
        }
        if k + 1 == frames + warm && shake > 0 {
            run.shake = shake;
        }
        let scene = Arc::new(arena::seat_scene(&run, w as i32, h as i32, Some(1)));
        let t0 = Instant::now();
        let frame = maker.make(&scene, |_| None);
        let t1 = Instant::now();
        let size = (frame.size.0 * scale, frame.size.1 * scale);
        layers = frame.layers.len();
        nodes = frame.props.as_ref().map_or(0, |p| p.nodes.len());
        bytes = frame.pictures.iter().map(|p| p.rgba.len()).sum();
        if k >= warm {
            total_bytes += bytes;
            for p in &frame.pictures {
                if std::env::var("BBOX").is_ok() {
                    let (w, h) = (p.width as usize, p.height as usize);
                    let (mut x0, mut y0, mut x1, mut y1, mut n) = (w, h, 0, 0, 0usize);
                    for y in 0..h {
                        for x in 0..w {
                            if p.rgba[(y * w + x) * 4 + 3] != 0 {
                                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
                                n += 1;
                            }
                        }
                    }
                    if k % 20 == 0 {
                        eprintln!("frame {k} picture {:x} {w}x{h}: {n} painted, box {}x{}", p.id, x1.saturating_sub(x0), y1.saturating_sub(y0));
                    }
                }
                let e = by_id.entry(p.id).or_insert((0usize, 0usize, (0, 0)));
                e.0 += 1;
                e.1 += p.rgba.len();
                e.2 = (p.width, p.height);
            }
        }
        shots = scene.shots.len();
        let now = k as f64 / 60.0;
        let (cpu, wait) = gpu.stage(frame, size, now);
        if k >= warm {
            make_ms.push((t1 - t0).as_secs_f64() * 1e3);
            render_ms.push(cpu);
            gpu_ms.push(wait);
        }
        last = Some(size);
    }
    let pct = |v: &mut Vec<f64>, p: f64| {
        v.sort_by(f64::total_cmp);
        v[((v.len() - 1) as f64 * p).round() as usize]
    };
    println!(
        "frames {frames} at {:?}: make p50 {:.3} p95 {:.3} ms | render p50 {:.3} p95 {:.3} ms | gpu wait p50 {:.3} p95 {:.3} ms",
        last.unwrap_or_default(),
        pct(&mut make_ms, 0.5),
        pct(&mut make_ms, 0.95),
        pct(&mut render_ms, 0.5),
        pct(&mut render_ms, 0.95),
        pct(&mut gpu_ms, 0.5),
        pct(&mut gpu_ms, 0.95),
    );
    println!("pictures uploaded: {:.0} bytes a frame on average; by picture (frames, bytes, size): {:x?}", total_bytes as f64 / frames as f64, by_id);
    println!("last frame: {layers} layers, {nodes} prop nodes, {shots} shots, {bytes} picture bytes uploaded");
    if let (Some(out), Some(size)) = (out, last) {
        let pixels = gpu.core.read_back(gpu.target.as_ref().map(|(_, t)| t).ok_or("no target")?, size);
        write_png(&out, size, &pixels)?;
        println!("wrote {}", out.display());
    }
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
        eprintln!("adapter: {:?}", adapter.get_info().name);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("delve-busy"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;
        device.on_uncaptured_error(Arc::new(|error: wgpu::Error| eprintln!("GPU error: {error}")));
        let core = Core::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
        Ok(Self { core, device, target: None })
    }

    /// A Delve frame drawn as the window draws its stage, into a texture
    /// `size` pixels on black: the CPU milliseconds of drawing it, and of
    /// waiting for the GPU after.
    fn stage(&mut self, mut frame: angel_stage::Frame, size: (u32, u32), now: f64) -> (f64, f64) {
        let t0 = Instant::now();
        for p in std::mem::take(&mut frame.pictures) {
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
        if self.target.as_ref().is_none_or(|(s, _)| *s != size) {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("delve-busy-target"),
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
        self.core.render_stages((1, 1), now);
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
        self.core.draw(&view, size, wgpu::Color::BLACK, &drawn, "delve-busy");
        let t1 = Instant::now();
        let _ = self.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(5)) });
        let t2 = Instant::now();
        ((t1 - t0).as_secs_f64() * 1e3, (t2 - t1).as_secs_f64() * 1e3)
    }
}

fn write_png(path: &std::path::Path, size: (u32, u32), rgba: &[u8]) -> Result<(), String> {
    let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let file = std::fs::File::create(path).map_err(|e| fail(&e))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size.0, size.1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| fail(&e))?;
    writer.write_image_data(rgba).map_err(|e| fail(&e))
}
