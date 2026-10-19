//! The Delve's proof rooms as the window draws them (docs/WORLD-3D.md,
//! docs/REALM-MONOLITH.md "Rooms, lit"): each world item's room as a run
//! stands in it (`world3d::room::proof`), its own people and props where the
//! game puts them, a knight half behind the tallest block and one in front
//! of one, made into a stage by the Delve (`StageMaker`) and drawn by the GPU
//! core offscreen, as the window draws the Delve's stage. Once with rooms in
//! 3D (lit, in the monolith look, the default; painted with
//! `ANGEL_WORLD3D_PLATES=1`) and once without.
//!
//! cargo run -p angel-glass --example delve_world -- <outdir> [--scale N] [--item ID]...
//!
//! Writes, for each item (every one compiled in, unless `--item` names
//! some), `<item>.png` and `<item>-flat.png` (the stage at `N` window pixels
//! to a stage pixel, 3 unless said), and `<item>-behind.png` and
//! `<item>-front.png`, crops round each knight.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_delve::render::overworld::arena::{self, stage_frame::StageMaker};
use angel_delve::world3d::{self, assets, room};
use angel_gpu::{Core, Draw, Frame, Instance, Picture, Stage};

/// The stage's picture id, as the cockpit names the Delve's.
const STAGE: u32 = 0x00FA_57A4;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("delve_world: {why}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let usage = "usage: delve_world <outdir> [--scale N] [--item ID]... [--variant ID]...";
    let mut words = std::env::args().skip(1);
    let (mut out, mut scale, mut items, mut variants) = (None, 3u32, Vec::new(), Vec::new());
    while let Some(word) = words.next() {
        match word.as_str() {
            "--scale" => {
                scale = words.next().and_then(|w| w.parse().ok()).filter(|&s| (1..=8).contains(&s)).ok_or("--scale takes 1 to 8")?;
            }
            "--item" => items.push(words.next().ok_or(usage)?),
            // The item's proof room laid otherwise (`room::proof_variant`):
            // a doorway walled up, a boulder rolled in.
            "--variant" => variants.push(words.next().ok_or(usage)?),
            _ if out.is_none() => out = Some(PathBuf::from(word)),
            other => return Err(format!("`{other}`: {usage}")),
        }
    }
    let out = out.ok_or(usage)?;
    if items.is_empty() && variants.is_empty() {
        // Lit, every item stands up with no plate; painted, those with one.
        items = if world3d::rooms_lit::on() {
            world3d::pilot::all().iter().map(|i| i.id().to_string()).collect()
        } else {
            assets::ids().into_iter().map(String::from).collect()
        };
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut gpu = Gpu::new()?;
    // As the cockpit's window has the Delve: its frames in parts, its
    // pickups and shots as shapes.
    arena::draw_parts_outside(true);
    arena::props_meshes(true);
    let (w, h) = arena::seat_size();
    let shots = items.iter().map(|id| (id.clone(), false)).chain(variants.iter().map(|id| (id.clone(), true)));
    for (item, varied) in shots {
        let proof = if varied { room::proof_variant(&item) } else { room::proof(&item) }
            .ok_or_else(|| format!("{item}: no such item, or it does not read"))?;
        let id = if varied { format!("{item}-variant") } else { item.clone() };
        for on in [false, true] {
            world3d::set_enabled(on);
            let scene = Arc::new(arena::seat_scene(&proof.run, w as i32, h as i32, Some(1)));
            if on && scene.world.is_none() {
                return Err(format!("{id}: its room has no world in 3D"));
            }
            let frame = StageMaker::default().make(&scene, |_| None);
            let size = (frame.size.0 * scale, frame.size.1 * scale);
            let pixels = gpu.stage(frame, size);
            let name = if on { format!("{id}.png") } else { format!("{id}-flat.png") };
            write_png(&out.join(&name), size, &pixels)?;
            if on {
                // Round each knight: where the stage's tilted floor has its feet
                // (the camera is the room's, under the band).
                let tilt = angel_stage::Tilt::for_stage(arena::stage_frame::TILT, (w, h)).ok_or("no tilt")?;
                for (feet, crop) in [(proof.behind, format!("{id}-behind.png")), (proof.in_front, format!("{id}-front.png"))] {
                    let at = tilt.floor([feet[0] - scene.camera.0, feet[1] - scene.camera.1]);
                    let (cx, cy) = ((at[0] * scale as f32) as i32, (at[1] * scale as f32) as i32);
                    let half = (48 * scale) as i32;
                    let rect = (cx - half, cy - half - (16 * scale) as i32, 2 * half, 2 * half);
                    write_png(&out.join(crop), (rect.2 as u32, rect.3 as u32), &crop_of(&pixels, size, rect))?;
                }
            }
            println!("{name}: {}x{}, the {w}x{h} stage at {scale}x; knights behind {:?}, in front {:?}", size.0, size.1, proof.behind, proof.in_front);
        }
    }
    world3d::set_enabled(false);
    println!("wrote the proof rooms to {}", out.display());
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
            label: Some("delve-world"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;
        device.on_uncaptured_error(Arc::new(|error: wgpu::Error| eprintln!("GPU error: {error}")));
        let core = Core::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
        Ok(Self { core, device })
    }

    /// A Delve frame as a stage, drawn into a texture `size` pixels as the
    /// window draws a stage in its cells, on black (the cockpit blacks the
    /// Delve's cells), and read back.
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
            label: Some("delve-world-target"),
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
        self.core.draw(&view, size, wgpu::Color::BLACK, &drawn, "delve-world");
        self.core.read_back(&texture, size)
    }
}

/// The part of `pixels` (`size`) in `rect`, black past its edges.
fn crop_of(pixels: &[u8], size: (u32, u32), (x0, y0, w, h): (i32, i32, i32, i32)) -> Vec<u8> {
    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = (x0 + x, y0 + y);
            let o = ((y * w + x) * 4) as usize;
            if sx >= 0 && sy >= 0 && (sx as u32) < size.0 && (sy as u32) < size.1 {
                let i = ((sy as u32 * size.0 + sx as u32) * 4) as usize;
                out[o..o + 4].copy_from_slice(&pixels[i..i + 4]);
            } else {
                out[o + 3] = 255;
            }
        }
    }
    out
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
