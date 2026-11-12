//! Her pointing limbs drawn by the GPU core offscreen: the arm with its
//! finger out and the leg with its toe pointed, each stretched long, laid
//! along a strip as the window shows them.
//!
//! cargo run -p angel-face --example limb_shot -- <avatar.glb> <outdir>

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_face::limb::LimbKind;
use angel_face::{Drawing, Goddess};
use angel_gpu::Core;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("limb_shot: {why}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let glb = PathBuf::from(
        args.next()
            .ok_or("usage: limb_shot <avatar.glb> <outdir>")?,
    );
    let out = PathBuf::from(args.next().ok_or("an output directory")?);
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let goddess = Arc::new(Goddess::from_glb(
        &std::fs::read(&glb).map_err(|e| format!("{}: {e}", glb.display()))?,
    )?);
    let drawing = Drawing::new(goddess);
    let mut gpu = Gpu::new()?;
    let ppm = 520.0;
    for (name, kind, length) in [("arm", LimbKind::Arm, 1.9), ("leg", LimbKind::Leg, 2.1)] {
        let radius = drawing.limb_radius(kind).ok_or("no limb")?;
        let size = (((length + 0.1) * ppm) as u32, (radius * 2.3 * ppm) as u32);
        let (id, mesh) = drawing.limb_mesh(kind, length).ok_or("no limb")?;
        gpu.core
            .upload_mesh(id, &mesh)
            .map_err(|e| format!("{e}"))?;
        let scene = drawing
            .limb_scene(kind, length, size, ppm, 8.0, 0.05)
            .ok_or("no scene")?;
        let pixels = gpu.shoot(&scene, size, 1.0);
        write_png(
            &out.join(format!("{name}.png")),
            size,
            &over_terminal(&pixels),
        )?;
        println!("{name}: {}x{}", size.0, size.1);
    }
    Ok(())
}

fn over_terminal(rgba: &[u8]) -> Vec<u8> {
    let bg = [8.0f32, 10.0, 14.0];
    let mut out = rgba.to_vec();
    for px in out.chunks_exact_mut(4) {
        let a = px[3] as f32 / 255.0;
        for c in 0..3 {
            px[c] = (px[c] as f32 + bg[c] * (1.0 - a)).min(255.0) as u8;
        }
        px[3] = 255;
    }
    out
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
            label: Some("limb-shot"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;
        device.on_uncaptured_error(Arc::new(|error: wgpu::Error| {
            eprintln!("GPU error: {error}")
        }));
        let core = Core::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
        Ok(Self {
            core,
            device,
            target: None,
        })
    }

    fn shoot(&mut self, scene: &angel_stage::scene::Scene, size: (u32, u32), t: f64) -> Vec<u8> {
        if self.target.as_ref().is_none_or(|(s, _)| *s != size) {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("limb-target"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
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
        self.core.set_scene_clock(t);
        self.core.draw_scene(&view, size, scene, "limb");
        self.core.read_back(texture, size)
    }
}

fn write_png(path: &Path, size: (u32, u32), rgba: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size.0, size.1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    writer
        .write_image_data(rgba)
        .map_err(|e| format!("{}: {e}", path.display()))
}
