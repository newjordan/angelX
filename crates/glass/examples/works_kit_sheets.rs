//! The Works' 3D kit through the game's own lit renderer, offscreen
//! (docs/REALM-MONOLITH.md, `angel_delve::world3d::works_kit`): every plan in
//! its states side by side on the hillside's palette, at the game camera, and
//! the frames of the working loop for a GIF.
//!
//! cargo run -p angel-glass --example works_kit_sheets -- <outdir> [--scale N] shots [id,id,...]
//! cargo run -p angel-glass --example works_kit_sheets -- <outdir> [--scale N] loop id,id,...
//! cargo run -p angel-glass --example works_kit_sheets -- <outdir> [--scale N] lines
//! cargo run -p angel-glass --example works_kit_sheets -- <outdir> bench
//!
//! `shots` writes `<id>.png`: raising (steps 1 and 3), idle, working, starved
//! and backed up in a row. `loop` writes `<id>-work-NN.png`, the eight steps
//! of the working loop. `lines` writes `lines.png` and `vehicles.png`.
//! `bench` times what a hundred pieces cost to make and to light.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use angel_delve::render::overworld::works_kit::{self as flat, FRAMES, Load, State};
use angel_delve::world3d::blockout::{self, Solid};
use angel_delve::world3d::kit::Face;
use angel_delve::world3d::realm_look;
use angel_delve::world3d::works_kit::{self as kit, Dir, LineKind, Piece};
use angel_gpu::{Core, Draw, Frame, Instance, Stage};
use angel_stage::look::{Dither, Look, Shadow, Sun};
use angel_stage::scene::{Props, World};
use angel_stage::set::{GAME_TILT, Set, ground_mesh_id, place};

const STAGE: u32 = 0x00FA_57A4;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("works_kit_sheets: {why}");
            ExitCode::FAILURE
        }
    }
}

/// A cell of a scene: a piece, where it stands, which way it faces.
struct Cell {
    piece: Arc<Piece>,
    foot: [f32; 4],
}

/// The hillside's look, as `realm_look::look` makes a view's, on the
/// stage's own pixels.
fn look(stage: (u32, u32), lamps: &[angel_stage::set::SetLamp], time: f32) -> Look {
    let mut look = Look::new(realm_look::HILLSIDE.to_vec());
    look.accent = vec![realm_look::GLINT, realm_look::WINDOW, realm_look::FIRE];
    look.dither = Dither::Bayer4;
    look.metre = realm_look::METRE;
    look.time = time;
    // `WORKS_LIFT=1` lifts the ambient and the sun a little, for looking at forms on a dark look.
    let lift = std::env::var("WORKS_LIFT").is_ok();
    look.sun = Some(Sun {
        toward: realm_look::SUN,
        strength: if lift { 1.4 } else { 1.15 },
        ambient: if lift { 0.5 } else { 0.27 },
        shadow: Some(Shadow { focus: [stage.0 as f32 / 2.0, 0.0, stage.1 as f32 / 2.0], reach: realm_look::SHADOW_REACH }),
        rise: 180.0,
    });
    look.warm = Some(realm_look::EMBER.to_vec());
    look.water = Some(realm_look::WATER.to_vec());
    look.lamps = lamps
        .iter()
        .take(8)
        .map(|l| angel_stage::look::Lamp { at: l.at, reach: l.reach, strength: l.strength })
        .collect();
    look
}

/// The set text of a scene of cells on a stage: grass under everything and
/// each cell's solids.
fn scene_text(stage: (u32, u32), cells: &[Cell]) -> (String, Vec<angel_stage::set::SetLamp>) {
    let mut text = format!("set works-kit\ncamera room {} {}\n", stage.0, stage.1);
    text.push_str(&format!("box grass at {} -4 {} size {} 4 {}\n", stage.0 / 2, stage.1 / 2, stage.0, stage.1));
    let mut lamps = Vec::new();
    for cell in cells {
        let placed = kit::stand(&cell.piece, cell.foot, Face::South);
        for s in &placed.solids {
            text.push_str(&blockout::line(s));
            text.push('\n');
        }
        lamps.extend(placed.lamps);
    }
    (text, lamps)
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
    let out = out.ok_or("usage: works_kit_sheets <outdir> [--scale N] shots|loop|lines|bench ...")?;
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut gpu = Gpu::new()?;
    match rest.first().map(String::as_str) {
        Some("shots") => {
            let only: Option<Vec<String>> = rest.get(1).map(|v| v.split(',').map(str::to_string).collect());
            for id in flat::plans() {
                if only.as_ref().is_some_and(|o| !o.iter().any(|x| x == id)) {
                    continue;
                }
                let states = [State::Building(1), State::Building(3), State::Idle, State::Working, State::Starved(Load::Lime), State::BackedUp(Load::Stone)];
                let pieces: Vec<Arc<Piece>> = states.iter().map(|s| kit::solids(id, *s, 0).ok_or("no piece")).collect::<Result<_, _>>()?;
                shoot_row(&mut gpu, &out.join(format!("{id}.png")), id, &pieces, scale, 0.0)?;
                println!("{id}");
            }
            Ok(())
        }
        Some("loop") => {
            let ids: Vec<String> = rest.get(1).ok_or("loop takes plan ids")?.split(',').map(str::to_string).collect();
            for id in &ids {
                for f in 0..FRAMES {
                    let piece = kit::solids(id, State::Working, f).ok_or("no piece")?;
                    // Idle beside it for company: the loop is the right one.
                    let still = kit::solids(id, State::Idle, 0).ok_or("no piece")?;
                    shoot_row(&mut gpu, &out.join(format!("{id}-work-{f:02}.png")), id, &[still, piece], scale, f as f32 / 8.0)?;
                }
                println!("loop {id}");
            }
            Ok(())
        }
        Some("lines") => lines(&mut gpu, &out, scale),
        Some("bench") => bench(&mut gpu),
        _ => Err("shots | loop | lines | bench".into()),
    }
}

/// A row of pieces of one plan side by side, one stage, one picture.
fn shoot_row(gpu: &mut Gpu, path: &Path, id: &str, pieces: &[Arc<Piece>], scale: u32, time: f32) -> Result<(), String> {
    let (w, d) = kit::extent(id).ok_or("no plan")?;
    let gap = 14.0;
    let stage = (((w + gap) * pieces.len() as f32 + gap).ceil() as u32, (d + 150.0) as u32);
    let cells: Vec<Cell> = pieces
        .iter()
        .enumerate()
        .map(|(i, piece)| {
            let x = gap + i as f32 * (w + gap);
            Cell { piece: piece.clone(), foot: [x, 118.0, x + w, 118.0 + d] }
        })
        .collect();
    let pixels = gpu.shoot(stage, &cells, scale, time)?;
    write_png(path, (stage.0 * scale, stage.1 * scale), &pixels)
}

fn lines(gpu: &mut Gpu, out: &Path, scale: u32) -> Result<(), String> {
    // Every join of each kind of line, then each carrier in four ways with a few loads.
    let tiles: Vec<(LineKind, u8, bool)> = LineKind::ALL.iter().flat_map(|k| (0..16u8).map(move |l| (*k, l, l % 2 == 0))).collect();
    let stage = (16 * 18 + 14, 5 * 40 + 130);
    let cells: Vec<Cell> = tiles
        .iter()
        .enumerate()
        .map(|(i, (k, l, post))| {
            let (col, row) = (i % 16, i / 16);
            let solids = kit::line_solids(*k, *l, *post, 2);
            let (x, z) = (10.0 + col as f32 * 18.0, 96.0 + row as f32 * 34.0);
            Cell { piece: Arc::new(Piece { solids: solids.to_vec(), lamps: Vec::new() }), foot: [x, z, x + 16.0, z + 16.0] }
        })
        .collect();
    let pixels = gpu.shoot(stage, &cells, scale, 0.0)?;
    write_png(&out.join("lines.png"), (stage.0 * scale, stage.1 * scale), &pixels)?;
    // Carriers: a picture a kind, the four ways in rows, a load a column.
    let loads = [None, Some(Load::Stone), Some(Load::Lime), Some(Load::Timber), Some(Load::Ore), Some(Load::Grain)];
    for kind in LineKind::ALL {
        let long = kit::vehicle_solids(kind, Dir::East, None, 0).iter().map(|s| s.footprint()[2] - s.footprint()[0]).fold(0.0f32, f32::max).max(16.0) + 24.0;
        let stage = ((long * 6.0 + 20.0) as u32, 4 * 40 + 120);
        let mut cells = Vec::new();
        for (di, dir) in [Dir::East, Dir::West, Dir::North, Dir::South].iter().enumerate() {
            for (c, load) in loads.iter().enumerate() {
                let solids = kit::vehicle_solids(kind, *dir, *load, 2);
                let (x, z) = (20.0 + c as f32 * long + long / 2.0 - 12.0, 90.0 + di as f32 * 40.0);
                cells.push(Cell { piece: Arc::new(Piece { solids: solids.to_vec(), lamps: Vec::new() }), foot: [x, z, x + 1.0, z + 1.0] });
            }
        }
        let pixels = gpu.shoot(stage, &cells, scale, 0.0)?;
        write_png(&out.join(format!("vehicle-{}.png", kind.id())), (stage.0 * scale, stage.1 * scale), &pixels)?;
    }
    Ok(())
}

fn bench(gpu: &mut Gpu) -> Result<(), String> {
    let ids: Vec<&str> = flat::plans().collect();
    for count in [25usize, 100] {
        let cells: Vec<Cell> = (0..count)
            .map(|k| {
                let id = ids[k % ids.len()];
                let (w, d) = kit::extent(id).unwrap();
                let (cx, cz) = ((k % 10) as f32 * 70.0 + 10.0, (k / 10) as f32 * 60.0 + 20.0);
                Cell { piece: kit::solids(id, State::Working, (k % 8) as u32).unwrap(), foot: [cx, cz, cx + w, cz + d] }
            })
            .collect();
        let stage = (720u32, 640u32);
        let started = Instant::now();
        let (text, _) = scene_text(stage, &cells);
        let made = started.elapsed();
        let solids: usize = cells.iter().map(|c| c.piece.solids.len()).sum();
        let mut times = Vec::new();
        for _ in 0..8 {
            let started = Instant::now();
            gpu.shoot(stage, &cells, 1, 0.0)?;
            times.push(started.elapsed().as_secs_f64() * 1e3);
        }
        times.sort_by(f64::total_cmp);
        println!("{count} pieces ({solids} solids, {} bytes of set): set text {made:?}; set parsed, nodes made, lit and read back {:.1} ms median, {:.1} ms best", text.len(), times[times.len() / 2], times[0]);
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
            label: Some("works-kit"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;
        device.on_uncaptured_error(Arc::new(|error: wgpu::Error| eprintln!("GPU error: {error}")));
        let core = Core::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
        Ok(Self { core, device, target: None })
    }

    /// A scene of `cells` on a stage of `stage` pixels at the game camera,
    /// lit by the hillside's look, `scale` window pixels to a stage pixel.
    fn shoot(&mut self, stage: (u32, u32), cells: &[Cell], scale: u32, time: f32) -> Result<Vec<u8>, String> {
        let (text, lamps) = scene_text(stage, cells);
        let set = Set::parse(&text).map_err(|e| e.iter().map(|r| r.to_string()).collect::<Vec<_>>().join("; "))?;
        let (floor, blocks, mesh) = set.lit_world_nodes(ground_mesh_id(1));
        let mut world = World::default();
        world.floor.extend(place(&floor, 1.0, [0.0; 3]));
        world.blocks.extend(place(&blocks, 1.0, [0.0; 3]));
        if let Some(mesh) = mesh {
            world.meshes.push((ground_mesh_id(1), Arc::new(mesh)));
        }
        let mut all = set.look_lamps();
        all.extend(lamps.iter().map(|l| angel_stage::look::Lamp { at: l.at, reach: l.reach, strength: l.strength }));
        let mut look = look(stage, &[], time);
        look.lamps = all.into_iter().take(8).collect();
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
            meshes: Vec::new(),
        };
        let size = (stage.0 * scale, stage.1 * scale);
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
                label: Some("works-kit-target"),
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
        self.core.draw(&view, size, wgpu::Color::BLACK, &drawn, "works-kit");
        Ok(self.core.read_back(texture, size))
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

#[allow(dead_code)]
fn _unused(_: &Solid) {}
