//! The monolith (`angel_glass::monolith`) shot offscreen by the GPU core, the
//! way the window would draw it: stills from a few places along the walk, or
//! the whole walk as numbered frames for a film.
//!
//! cargo run --release -p angel-glass --example monolith_shots -- <outdir> [--size WxH] [--rows N]
//! cargo run --release -p angel-glass --example monolith_shots -- <outdir> --walk [--fps N] [--every K]
//!
//! Stills are `<name>.png`; the walk is `walk-00000.png` on.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_glass::monolith::{self, Follow, Walker};
use angel_gpu::{Core, Picture};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("monolith_shots: {why}");
            ExitCode::FAILURE
        }
    }
}

/// The stills: a name, where the knight is, how the eye follows, the clock.
fn stills() -> Vec<(&'static str, Walker, Follow, f32)> {
    let on_road = |z: f32| {
        let x = monolith::road_x(z);
        let ahead = monolith::road_x(z - 2.0);
        Walker { x, z, heading: (ahead - x).atan2(2.0) }
    };
    // `MONOLITH_FOLLOW=back,up,fov,orbit` tries another eye.
    let near = std::env::var("MONOLITH_FOLLOW")
        .ok()
        .and_then(|v| {
            let p: Vec<f32> = v.split(',').filter_map(|c| c.trim().parse().ok()).collect();
            (p.len() == 4).then(|| Follow { back: p[0], up: p[1], fov_deg: p[2], orbit: p[3] })
        })
        .unwrap_or_default();
    vec![
        ("01-meadow-road", on_road(110.0), near, 2.0),
        ("02-the-stones", on_road(70.0), near, 4.0),
        ("03-to-the-river", on_road(24.0), near, 6.0),
        ("04-on-the-bridge", on_road(-12.0), near, 8.0),
        ("05-up-to-the-wall", on_road(-50.0), near, 10.0),
        ("06-at-the-gate", on_road(-84.0), near, 12.0),
        ("07-river-bank-west", Walker { x: -40.0, z: monolith::river_z(-40.0) + 22.0, heading: -0.4 }, Follow { orbit: 0.5, ..near }, 14.0),
        ("08-far-from-the-wall", on_road(10.0), Follow { back: 60.0, up: 30.0, fov_deg: 38.0, orbit: 0.0 }, 16.0),
        ("09-wide-low", on_road(30.0), Follow { back: 70.0, up: 12.0, fov_deg: 42.0, orbit: 0.35 }, 18.0),
        ("10-under-the-wall", on_road(-80.0), Follow { back: 26.0, up: 8.0, fov_deg: 50.0, orbit: -0.5 }, 20.0),
        ("11-moat-edge", Walker { x: 24.0, z: monolith::MOAT_EDGE + 4.0, heading: -1.2 }, Follow { back: 28.0, up: 14.0, fov_deg: 44.0, orbit: 0.9 }, 22.0),
        ("12-quay-from-meadow", Walker { x: 60.0, z: monolith::river_z(60.0) + 26.0, heading: 0.0 }, Follow { back: 30.0, up: 10.0, fov_deg: 44.0, orbit: -0.4 }, 24.0),
    ]
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().ok_or("usage: monolith_shots <outdir> [--size WxH] [--rows N] [--walk]")?);
    let mut size = (1920u32, 1080u32);
    let mut rows = monolith::ROWS;
    let mut walk = false;
    let mut fps = 24.0f32;
    let mut every = 1usize;
    let mut pipe = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--size" => {
                let v = args.next().ok_or("--size WxH")?;
                let (w, h) = v.split_once('x').ok_or("--size WxH")?;
                size = (w.parse().map_err(|_| "--size WxH")?, h.parse().map_err(|_| "--size WxH")?);
            }
            "--rows" => rows = args.next().and_then(|v| v.parse().ok()).ok_or("--rows N")?,
            "--walk" => walk = true,
            "--fps" => fps = args.next().and_then(|v| v.parse().ok()).ok_or("--fps N")?,
            "--every" => every = args.next().and_then(|v| v.parse().ok()).ok_or("--every K")?,
            // The walk's frames as raw RGBA on stdout, for ffmpeg.
            "--pipe" => pipe = true,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut gpu = Gpu::new()?;
    for (id, mesh) in monolith::meshes() {
        gpu.core.upload_mesh(id, &mesh).map_err(|e| format!("mesh {id:#x}: {e}"))?;
    }
    for (id, width, height, rgba) in monolith::sprites() {
        gpu.core.upload(Picture { id, width, height, rgba, cols: 0, rows: 0 });
    }
    if walk {
        let seconds = (140.0 - (monolith::WALL_Z + 4.0)) / monolith::WALK_SPEED + 4.0;
        let frames = (seconds * fps) as usize;
        let mut written = 0;
        let mut stdout = std::io::stdout().lock();
        for f in (0..frames).step_by(every.max(1)) {
            let t = f as f32 / fps;
            let walker = monolith::walk(t);
            let scene = monolith::scene(walker, Follow::default(), t, rows);
            let pixels = gpu.shoot(&scene, size);
            if pipe {
                use std::io::Write;
                stdout.write_all(&pixels).map_err(|e| format!("stdout: {e}"))?;
            } else {
                write_png(&out.join(format!("walk-{written:05}.png")), size, &pixels)?;
            }
            written += 1;
        }
        eprintln!("{written} frames");
        return Ok(());
    }
    for (name, walker, follow, t) in stills() {
        let scene = monolith::scene(walker, follow, t, rows);
        let pixels = gpu.shoot(&scene, size);
        write_png(&out.join(format!("{name}.png")), size, &pixels)?;
        println!("{name}");
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
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("monolith-shots"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;
        device.on_uncaptured_error(Arc::new(|error: wgpu::Error| eprintln!("GPU error: {error}")));
        let core = Core::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
        Ok(Self { core, device, target: None })
    }

    fn shoot(&mut self, scene: &angel_stage::scene::Scene, size: (u32, u32)) -> Vec<u8> {
        if self.target.as_ref().is_none_or(|(s, _)| *s != size) {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("monolith-target"),
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
        self.core.draw_scene(&view, size, scene, "monolith");
        self.core.read_back(texture, size)
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
