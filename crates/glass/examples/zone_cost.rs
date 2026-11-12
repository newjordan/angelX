//! What a look's zones cost a frame (docs/GLASS_CORE.md, "The world's look"),
//! with nothing but the stage and the GPU core in it, so the same file builds
//! against an older tree and the two shaders can be compared: a stage of boxes
//! on grass through a lit look with a sun and its shadows, `n` zones over it,
//! the frame timed by wall clock round `render_stages` + `draw` + the device's
//! last word.
//!
//! cargo run -p angel-glass --example zone_cost -- [counts] [frames]
//!
//! `counts` is a comma list of zone counts (default 0,4,16). Each is timed at
//! a 960 x 540 stage (the game's size: the frame is the CPU's) and a 3840 x
//! 2160 one (the most a look draws: the pixels are the work), the zones piled
//! over the middle of the stage so every pixel is under several, and, at
//! the big stage, with none of them on it (the shader only weighs them).

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use angel_gpu::{Core, Draw, Frame, Instance, Stage};
use angel_stage::look::{Dither, Look, Shadow, Sun, Zone};
use angel_stage::scene::{Props, World};
use angel_stage::set::{GAME_TILT, Set, ground_mesh_id, place};

const STAGE: u32 = 0x00FA_57A5;

/// The hillside's ramp (`world3d::realm_look::HILLSIDE`).
const GREEN: [[u8; 3]; 8] = [
    [0x0e, 0x13, 0x0c],
    [0x18, 0x22, 0x14],
    [0x24, 0x33, 0x1d],
    [0x31, 0x46, 0x27],
    [0x41, 0x5c, 0x33],
    [0x55, 0x73, 0x42],
    [0x6e, 0x8c, 0x56],
    [0x90, 0xa4, 0x75],
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("zone_cost: {why}");
            ExitCode::FAILURE
        }
    }
}

/// A ramp leaning `k` of the way to a tint, entry for entry as bright as the green.
fn tinted(k: usize) -> Vec<[u8; 3]> {
    let tint = [[1.3f32, 0.9, 0.6], [0.7, 0.95, 1.3], [1.2, 1.0, 0.7], [1.1, 0.8, 0.9]][k % 4];
    GREEN.iter().map(|c| [0, 1, 2].map(|i| (f32::from(c[i]) * tint[i]).clamp(0.0, 255.0) as u8)).collect()
}

fn look(stage: (u32, u32), zones: usize, off: bool) -> Look {
    let mut look = Look::new(GREEN.to_vec());
    look.accent = vec![[0xd9, 0xd6, 0xb8], [0xee, 0xae, 0x55], [0xe2, 0x7a, 0x34]];
    look.dither = Dither::Bayer4;
    look.metre = 11.0;
    look.sun = Some(Sun {
        toward: [-0.66, 0.46, 0.46],
        strength: 1.15,
        ambient: 0.27,
        shadow: Some(Shadow { focus: [stage.0 as f32 / 2.0, 0.0, stage.1 as f32 / 2.0], reach: 960.0 }),
        rise: 180.0,
    });
    let reach = stage.0 as f32 * 0.9;
    let (cx, cz) = if off { (-50.0 * reach, -50.0 * reach) } else { (stage.0 as f32 / 2.0, stage.1 as f32 / 2.0) };
    look.zones = (0..zones)
        .map(|k| Zone {
            ramp: tinted(k),
            at: [cx + 0.05 * reach * (k as f32 - zones as f32 / 2.0), cz + 0.02 * reach * (k % 5) as f32],
            half: [0.3 * reach, 0.25 * reach],
            radius: 0.05 * reach,
            feather: 0.25 * reach,
        })
        .collect();
    look
}

/// Some boxes on grass: forty a thousand pixels apart in both ways would be
/// nothing on a big stage, so they are laid by the stage's own size.
fn world(stage: (u32, u32), zones: usize, off: bool) -> Result<World, String> {
    let mut text = format!("set zone-cost\ncamera room {} {}\n", stage.0, stage.1);
    text.push_str(&format!("box grass at {} -4 {} size {} 4 {}\n", stage.0 / 2, stage.1 / 2, stage.0 * 3, stage.1 * 3));
    let kinds = ["stone", "wood", "plaster", "roof"];
    for k in 0..40u32 {
        let (cx, cz) = (stage.0 as f32 * (0.08 + 0.84 * (k % 8) as f32 / 7.0), stage.1 as f32 * (0.1 + 0.8 * (k / 8) as f32 / 4.0));
        let side = stage.1 as f32 / 14.0;
        text.push_str(&format!("box {} at {cx} {} {cz} size {side} {} {side}\n", kinds[k as usize % 4], side * 0.4, side * 0.8));
    }
    let set = Set::parse(&text).map_err(|e| e.iter().map(|r| r.to_string()).collect::<Vec<_>>().join("; "))?;
    let (floor, blocks, mesh) = set.lit_world_nodes(ground_mesh_id(1));
    let mut world = World::default();
    world.floor.extend(place(&floor, 1.0, [0.0; 3]));
    world.blocks.extend(place(&blocks, 1.0, [0.0; 3]));
    if let Some(mesh) = mesh {
        world.meshes.push((ground_mesh_id(1), Arc::new(mesh)));
    }
    let mut look = look(stage, zones, off);
    look.pixel_rows = stage.1;
    world.look = Some(look);
    Ok(world)
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let counts: Vec<usize> = args
        .next()
        .map_or(vec![0, 4, 16], |v| v.split(',').filter_map(|n| n.parse().ok()).collect());
    let frames: usize = args.next().and_then(|v| v.parse().ok()).unwrap_or(300);
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
        ..Default::default()
    }))
    .map_err(|e| format!("no GPU adapter: {e}"))?;
    let info = adapter.get_info();
    println!("adapter: {} ({:?}, {:?}, driver {} {})", info.name, info.backend, info.device_type, info.driver, info.driver_info);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("zone-cost"),
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .map_err(|e| format!("no GPU device: {e}"))?;
    device.on_uncaptured_error(Arc::new(|error: wgpu::Error| eprintln!("GPU error: {error}")));
    let mut core = Core::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
    println!("{frames} frames each after 20 of warm up; ms a frame: median / mean / 95th");
    for stage in [(960u32, 540u32), (3840, 2160)] {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("zone-cost-target"),
            size: wgpu::Extent3d { width: stage.0, height: stage.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let shown = Instance {
            rect: [0.0, 0.0, stage.0 as f32, stage.1 as f32],
            uv: [0.0, 0.0, stage.0 as f32, stage.1 as f32],
            color: [1.0; 4],
            kind: angel_gpu::PICTURE,
            ..bytemuck::Zeroable::zeroed()
        };
        let mut parts = vec![(Draw::Picture { id: STAGE, linear: false }, vec![shown])];
        let mut drawn = Frame::default();
        drawn.assemble_into(&mut parts, &mut Vec::new());
        println!("--- a {}x{} stage, 40 boxes", stage.0, stage.1);
        let mut runs: Vec<(String, usize, bool)> = counts.iter().map(|&n| (format!("{n} zones piled over the middle"), n, false)).collect();
        if stage.0 > 960 {
            runs.push(("16 zones, none on the stage".into(), 16, true));
        }
        let mut base: Option<f64> = None;
        for (name, n, off) in runs {
            let base_world = world(stage, n, off)?;
            let mut ms = Vec::new();
            for f in 0..frames + 20 {
                let mut w = base_world.clone();
                if let Some(look) = w.look.as_mut() {
                    look.time = f as f32 * 0.016;
                }
                let frame = angel_stage::Frame {
                    size: stage,
                    layers: Vec::new(),
                    pictures: Vec::new(),
                    scene: 1,
                    enter: None,
                    tilt: GAME_TILT,
                    props: Some(Props { at: 0, grain: 1, nodes: Vec::new(), world: Some(w) }),
                    impact: 0.0,
                    meshes: Vec::new(),
                };
                core.set_stage(
                    STAGE,
                    Some(Stage {
                        id: STAGE,
                        size: frame.size,
                        cols: stage.0 as u16,
                        rows: stage.1 as u16,
                        layers: frame.layers,
                        scene: frame.scene,
                        enter: None,
                        tilt: frame.tilt,
                        props: frame.props,
                        impact: frame.impact,
                    }),
                );
                let started = Instant::now();
                core.begin_frame();
                core.render_stages((1, 1), 0.0);
                core.draw(&view, stage, wgpu::Color::BLACK, &drawn, "zone-cost");
                let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(10)) });
                if f >= 20 {
                    ms.push(started.elapsed().as_secs_f64() * 1e3);
                }
            }
            ms.sort_by(f64::total_cmp);
            let median = ms[ms.len() / 2];
            let delta = base.map_or(String::new(), |b| format!("  ({:+.1}% against the first)", (median / b - 1.0) * 100.0));
            base.get_or_insert(median);
            println!("{name:<42} {median:7.3} / {:7.3} / {:7.3}{delta}", ms.iter().sum::<f64>() / ms.len() as f64, ms[(ms.len() as f64 * 0.95) as usize - 1]);
        }
    }
    Ok(())
}
