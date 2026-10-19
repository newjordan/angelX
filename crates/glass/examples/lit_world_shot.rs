//! A world drawn through a look that lights it, on a game stage, as the
//! window draws the Delve's (docs/REALM-MONOLITH.md): set files lit and
//! toned at the stage's own pixel grain, through the GPU core offscreen.
//! The harness the realm's generators are looked at with.
//!
//! cargo run -p angel-glass --example lit_world_shot -- <out.png> <look.txt> <set>... [--stage W H] [--scale N] [--at X Z]
//!
//! The sets are placed on a stage `W` x `H` (384 x 224 unless said) with the
//! floor point `(X, Z)` at its middle; `--scale` window pixels to a stage
//! pixel (3). `look.txt` is a look as `Look::to_text` writes it; the
//! sets' lamps are added to it. With no look file (`-`), a plain test look.

use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use angel_gpu::{Core, Draw, Frame, Instance, Stage};
use angel_stage::look::{Look, Sun};
use angel_stage::scene::{Props, World};
use angel_stage::set::{GAME_TILT, Set, ground_mesh_id, place};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("lit_world_shot: {why}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out = args.next().ok_or("usage: lit_world_shot <out.png> <look.txt|-> <set>...")?;
    let look_path = args.next().ok_or("a look file, or -")?;
    let (mut stage, mut scale, mut at) = ((384u32, 224u32), 3u32, None);
    let mut sets = Vec::new();
    while let Some(a) = args.next() {
        let mut num = |what: &str| args.next().and_then(|v| v.parse::<f32>().ok()).ok_or(format!("{what} wants a number"));
        match a.as_str() {
            "--stage" => stage = (num("--stage")? as u32, num("--stage")? as u32),
            "--scale" => scale = num("--scale")? as u32,
            "--at" => at = Some([num("--at")?, num("--at")?]),
            path => sets.push(path.to_string()),
        }
    }
    let mut look = if look_path == "-" {
        let mut l = Look::new(vec![
            [0x0e, 0x12, 0x10], [0x17, 0x1f, 0x1b], [0x22, 0x2d, 0x27], [0x2f, 0x3c, 0x33],
            [0x40, 0x4f, 0x42], [0x55, 0x65, 0x52], [0x70, 0x7e, 0x67], [0x96, 0x9f, 0x84],
        ]);
        l.sun = Some(Sun { toward: [-0.66, 0.46, 0.46], strength: 1.15, ambient: 0.26, shadow: None, rise: 0.0 });
        l.metre = 11.0;
        l
    } else {
        let text = std::fs::read_to_string(&look_path).map_err(|e| format!("{look_path}: {e}"))?;
        Look::from_text(&text).ok_or(format!("{look_path}: not a look"))?
    };
    let mut world = World::default();
    let mut centre = [0.0f32; 2];
    let mut count: f32 = 0.0;
    let mut parsed = Vec::new();
    for (k, path) in sets.iter().enumerate() {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        let set = Set::parse(&text).map_err(|e| format!("{path}: {}", e.iter().map(|r| r.to_string()).collect::<Vec<_>>().join("; ")))?;
        let s = set.camera.stage();
        centre[0] += (s[0] + s[2]) / 2.0;
        centre[1] += (s[1] + s[3]) / 2.0;
        count += 1.0;
        parsed.push((k as u64 + 1, set));
    }
    let at = at.unwrap_or([centre[0] / count.max(1.0), centre[1] / count.max(1.0)]);
    let offset = [stage.0 as f32 / 2.0 - at[0], 0.0, stage.1 as f32 / 2.0 - at[1]];
    for (key, set) in &parsed {
        let id = ground_mesh_id(*key);
        let (floor, blocks, mesh) = set.lit_world_nodes(id);
        let tag = |mut n: angel_stage::scene::Node| {
            n.id |= key << 40;
            n
        };
        world.floor.extend(place(&floor, 1.0, offset).into_iter().map(tag));
        world.blocks.extend(place(&blocks, 1.0, offset).into_iter().map(tag));
        if let Some(mesh) = mesh {
            world.meshes.push((id, Arc::new(mesh)));
        }
        for lamp in set.look_lamps() {
            if look.lamps.len() < 8 {
                look.lamps.push(angel_stage::look::Lamp { at: [lamp.at[0] + offset[0], lamp.at[1], lamp.at[2] + offset[2]], ..lamp });
            }
        }
    }
    if let Some(sun) = &mut look.sun
        && sun.shadow.is_none()
    {
        sun.shadow = Some(angel_stage::look::Shadow { focus: [stage.0 as f32 / 2.0, 0.0, stage.1 as f32 / 2.0], reach: stage.0 as f32 * 2.5 });
    }
    world.look = Some(look);
    let frame = angel_stage::Frame {
        size: stage,
        layers: Vec::new(),
        pictures: Vec::new(),
        scene: 1,
        enter: None,
        tilt: GAME_TILT,
        props: Some(Props { at: 0, grain: 1, nodes: Vec::new(), world: Some(world) }),
        impact: 0.0,
    };
    let size = (stage.0 * scale, stage.1 * scale);
    let pixels = shoot(frame, size)?;
    write_png(Path::new(&out), size, &pixels)
}

fn shoot(frame: angel_stage::Frame, size: (u32, u32)) -> Result<Vec<u8>, String> {
    const STAGE: u32 = 0x00FA_57A4;
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
        ..Default::default()
    }))
    .map_err(|e| format!("no GPU adapter: {e}"))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("lit-world-shot"),
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .map_err(|e| format!("no GPU device: {e}"))?;
    device.on_uncaptured_error(Arc::new(|error: wgpu::Error| eprintln!("GPU error: {error}")));
    let mut core = Core::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
    core.set_stage(
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
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("lit-world-target"),
        size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    core.begin_frame();
    core.render_stages((1, 1), 0.0);
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
    core.draw(&view, size, wgpu::Color::BLACK, &drawn, "lit-world-shot");
    Ok(core.read_back(&texture, size))
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
