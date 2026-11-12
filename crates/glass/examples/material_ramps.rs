//! The Works' materials measured (docs/REALM-MONOLITH.md; `angel_delve::world3d::
//! works_look`): a screen full of the hillside's buildings in the game's own
//! lit renderer, offscreen, at the game camera, through the realm's look with
//! no zones for the buildings, with the old four, and with the new budget; and
//! what the zones cost a frame.
//!
//! cargo run -p angel-glass --example material_ramps -- <outdir> shots
//! cargo run -p angel-glass --example material_ramps -- <outdir> time [frames]
//!
//! `shots` lays the works twice, by district (neighbours alike: a few streets)
//! and mixed (every neighbour another material: a zone a building), a 480 x 270
//! stage drawn at its own pixel grain and enlarged twice to 960 x 540, and
//! writes for each plan `<plan>-old.png` (the buildings on the one green ramp),
//! `<plan>-zones4.png` (the old cap), `<plan>-zonesN.png` (the budget),
//! `<plan>-markers.png` (the same scene in one flat colour a building, for
//! masks) with `<plan>-markers.tsv`, `<plan>-layout.tsv`, and `ramps.tsv`: what a
//! script reads to count how far the buildings' colours are from the grass's.
//!
//! `time` prints what drawing the stage costs, wall clock around
//! `render_stages` + `draw` + the device poll, 200 frames each, with none,
//! four and sixteen zones: first the real screen at 540 rows (a CPU's frame:
//! hundreds of nodes to encode), then a fill-bound one at 2160 rows, where the
//! pixels are the work and a shader's cost shows.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use angel_delve::render::overworld::works_kit::State;
use angel_delve::world3d::blockout;
use angel_delve::world3d::kit::Face;
use angel_delve::world3d::realm_look::{self, View};
use angel_delve::world3d::works_kit::{self as kit, Piece};
use angel_delve::world3d::works_look::{self, Footprint, Material};
use angel_gpu::{Core, Draw, Frame, Instance, Stage};
use angel_stage::look::{Look, Zone};
use angel_stage::scene::{Material as Surface, Props, World};
use angel_stage::set::{GAME_TILT, Set, ground_mesh_id, place};

const STAGE: u32 = 0x00FA_57A4;
/// The stage's size in its own pixels, and the window pixels to each: 960 x 540,
/// the internal size of a 540-row look, so a frame is read 1:1.
const SIZE: (u32, u32) = (480, 270);
const SCALE: u32 = 2;
/// A tile, the gap between neighbours in a row, and between rows.
const TILE: f32 = 16.0;
const GAP: f32 = 16.0;
const STREET: f32 = 20.0;

/// The most pieces of one material a screen is given: seven materials, so a
/// screen of up to thirty-five, as many of them as fit; and the biggest a
/// piece may be, in tiles (the great halls would leave no room for a street).
const PER_MATERIAL: usize = 5;
const BIGGEST: i32 = 9;

/// How the pieces are laid: by district (a timber yard, a farm, the quarry,
/// the forges: neighbours alike, streets that merge into a few zones) or mixed
/// (every neighbour a different material: as many zones as buildings).
#[derive(Clone, Copy, PartialEq)]
enum Plan {
    Districts,
    Mixed,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("material_ramps: {why}");
            ExitCode::FAILURE
        }
    }
}

/// A building laid on the stage.
struct Placed {
    id: &'static str,
    /// `[x0, z0, x1, z1]` in the stage's pixels.
    rect: [f32; 4],
    material: Material,
    piece: Arc<Piece>,
}

/// The kit's plans of each material, at most `PER_MATERIAL` of them, taken at
/// an even stride through the list by size so small and large ones both come.
fn by_material() -> Vec<Vec<&'static str>> {
    use angel_delve::render::overworld::works_kit as flat;
    Material::ALL
        .iter()
        .map(|m| {
            let mut all: Vec<(i32, &'static str)> = flat::plans()
                .filter(|id| works_look::material_of(id) == *m)
                .filter_map(|id| flat::size(id).map(|(w, d)| (w * d, id)))
                .filter(|(area, _)| *area <= BIGGEST)
                .collect();
            all.sort();
            let n = all.len().min(PER_MATERIAL);
            (0..n).map(|k| all[k * all.len() / n].1).collect()
        })
        .collect()
}

/// The works: rows of buildings left to right, a tile between neighbours, a
/// street between rows, as many as fit of the plan's order.
fn layout(plan: Plan) -> Result<Vec<Placed>, String> {
    let groups = by_material();
    let order: Vec<&'static str> = match plan {
        Plan::Districts => groups.iter().flatten().copied().collect(),
        // One of each material in turn, so no two neighbours are alike.
        Plan::Mixed => (0..PER_MATERIAL).flat_map(|k| groups.iter().filter_map(move |g| g.get(k).copied())).collect(),
    };
    let mut out = Vec::new();
    let (mut x, mut z, mut row) = (10.0f32, 26.0f32, 0.0f32);
    for id in order {
        let (w, d) = kit::extent(id).ok_or(format!("no plan {id}"))?;
        if x + w > SIZE.0 as f32 - 10.0 {
            x = 10.0;
            z += row + STREET;
            row = 0.0;
        }
        if z + d > SIZE.1 as f32 - 4.0 {
            continue;
        }
        let piece = kit::solids(id, State::Idle, 0).ok_or(format!("no piece {id}"))?;
        out.push(Placed { id, rect: [x, z, x + w, z + d], material: works_look::material_of(id), piece });
        x += w + GAP;
        row = row.max(d);
    }
    Ok(out)
}

/// A place on the map where the stage sees no camp and no other biome: the
/// view's origin in the kingdom's pixels. The realm's own look then has no
/// zones of its own, so the zones in a frame are the buildings'.
fn clear_origin() -> Result<[f32; 2], String> {
    for ty in (4..150).step_by(7) {
        for tx in (4..210).step_by(7) {
            let origin = [tx as f32 * TILE, ty as f32 * TILE];
            let view = View { origin, size: [SIZE.0 as f32, SIZE.1 as f32], knight: None, others: [None; 3], time: 0.0 };
            if realm_look::camp_zones(&view).is_empty() && realm_look::look(&view, &[]).zones.is_empty() {
                return Ok(origin);
            }
        }
    }
    Err("no empty hillside on the map".into())
}

fn view_at(origin: [f32; 2]) -> View {
    View { origin, size: [SIZE.0 as f32, SIZE.1 as f32], knight: None, others: [None; 3], time: 0.0 }
}

/// How the buildings are dyed in a frame.
#[derive(Clone, Copy, Debug)]
enum Dye {
    /// The one green ramp.
    Off,
    /// The nearest `n` zones, as a look that kept four did.
    Nearest(usize),
    /// The budget: `dye_buildings`.
    Budget,
}

fn footprints(placed: &[Placed], origin: [f32; 2]) -> Vec<Footprint> {
    placed
        .iter()
        .map(|p| Footprint { rect: [p.rect[0] + origin[0], p.rect[1] + origin[1], p.rect[2] + origin[0], p.rect[3] + origin[1]], material: p.material })
        .collect()
}

fn look_of(placed: &[Placed], origin: [f32; 2], dye: Dye, rows: u32) -> Look {
    let view = view_at(origin);
    let mut look = realm_look::look(&view, &[]);
    look.pixel_rows = rows;
    let feet = footprints(placed, origin);
    match dye {
        Dye::Off => {}
        Dye::Nearest(n) => {
            let zones = works_look::building_zones(&view, &feet);
            look.zones.extend(zones[zones.len().saturating_sub(n)..].iter().cloned());
        }
        Dye::Budget => works_look::dye_buildings(&mut look, &view, &feet),
    }
    look
}

/// The set text of the works: grass under it, each building's solids. Also how
/// many shapes each building made, to tell them apart in the world's nodes.
fn scene_text(placed: &[Placed], stage: (u32, u32)) -> Result<(String, Vec<usize>), String> {
    let head = format!("set material-ramps\ncamera room {} {}\n", stage.0, stage.1);
    let mut text = head.clone();
    // Grass well past the view's edges, as the hillside runs on in the game.
    text.push_str(&format!("box grass at {} -4 {} size {} 4 {}\n", stage.0 / 2, stage.1 / 2, stage.0 * 3, stage.1 * 3));
    let mut counts = Vec::new();
    for p in placed {
        let stood = kit::stand(&p.piece, p.rect, Face::South);
        let mut lines = String::new();
        for s in &stood.solids {
            lines.push_str(&blockout::line(s));
            lines.push('\n');
        }
        let parsed = Set::parse(&format!("{head}{lines}")).map_err(|e| e.iter().map(|r| r.to_string()).collect::<Vec<_>>().join("; "))?;
        counts.push(parsed.shapes.len());
        text.push_str(&lines);
    }
    Ok((text, counts))
}

/// The marker colour of building `b`: flat, apart from every other by a margin
/// a rounding cannot close.
fn marker(b: usize) -> [u8; 3] {
    let k = b + 1;
    [(40 + (k % 7) * 28) as u8, (40 + ((k / 7) % 7) * 28) as u8, (40 + ((k / 49) % 5) * 40) as u8]
}

/// The world of the works for `look` (or, with `markers`, no look and each
/// building one flat colour).
fn world_of(placed: &[Placed], look: Option<Look>, markers: bool, stage: (u32, u32)) -> Result<World, String> {
    let (text, counts) = scene_text(placed, stage)?;
    let set = Set::parse(&text).map_err(|e| e.iter().map(|r| r.to_string()).collect::<Vec<_>>().join("; "))?;
    if set.shapes.len() != 1 + counts.iter().sum::<usize>() {
        return Err(format!("{} shapes, expected {}", set.shapes.len(), 1 + counts.iter().sum::<usize>()));
    }
    let (mut floor, mut blocks, mesh) = set.lit_world_nodes(ground_mesh_id(1));
    if markers {
        // A building's shape ids follow the ground's (shape 0, node id 1).
        let mut first = vec![1usize];
        for c in &counts {
            first.push(first.last().copied().unwrap_or(1) + c);
        }
        let paint = |n: &mut angel_stage::scene::Node| {
            let shape = (n.id as usize).saturating_sub(1);
            let [r, g, b] = if shape == 0 {
                [0, 0, 0]
            } else {
                let building = first.iter().rposition(|&f| f <= shape).unwrap_or(0);
                marker(building)
            };
            n.material = Surface::color([f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0]);
            n.face_materials.clear();
        };
        floor.iter_mut().chain(blocks.iter_mut()).for_each(paint);
    }
    let mut world = World::default();
    world.floor.extend(place(&floor, 1.0, [0.0; 3]));
    world.blocks.extend(place(&blocks, 1.0, [0.0; 3]));
    if let Some(mesh) = mesh {
        world.meshes.push((ground_mesh_id(1), Arc::new(mesh)));
    }
    world.look = look;
    Ok(world)
}

fn run() -> Result<(), String> {
    let mut words = std::env::args().skip(1);
    let out = PathBuf::from(words.next().ok_or("usage: material_ramps <outdir> shots|time [frames]")?);
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mode = words.next().ok_or("shots | time")?;
    let origin = clear_origin()?;
    let mut gpu = Gpu::new()?;
    println!("adapter: {}", gpu.info);
    match mode.as_str() {
        "shots" => {
            for (name, plan) in [("districts", Plan::Districts), ("mixed", Plan::Mixed)] {
                let placed = layout(plan)?;
                println!("== {name}: {} buildings laid on a {}x{} stage; the view's origin is {:?} in the kingdom's pixels", placed.len(), SIZE.0, SIZE.1, origin);
                shots(&mut gpu, &out, name, &placed, origin)?;
            }
            Ok(())
        }
        "time" => {
            let frames = words.next().and_then(|v| v.parse().ok()).unwrap_or(200usize);
            let placed = layout(Plan::Districts)?;
            time(&mut gpu, &placed, origin, frames)
        }
        _ => Err("shots | time".into()),
    }
}

fn shots(gpu: &mut Gpu, out: &Path, plan: &str, placed: &[Placed], origin: [f32; 2]) -> Result<(), String> {
    let view = view_at(origin);
    let feet = footprints(placed, origin);
    let groups = works_look::street_groups(&view, &feet);
    let budget = works_look::building_zones(&view, &feet).len();
    println!("{} buildings in {} streets (zone-worth boxes); the budget dyes the nearest {budget}", feet.len(), groups.len());
    let kinds: Vec<String> = Material::ALL.iter().map(|m| format!("{m:?} {}", placed.iter().filter(|p| p.material == *m).count())).collect();
    println!("by material: {}", kinds.join(", "));
    // layout.tsv: the building, its plan, its material, its box on the stage.
    let mut layout = String::from("index\tplan\tmaterial\tx0\tz0\tx1\tz1\n");
    for (i, p) in placed.iter().enumerate() {
        layout.push_str(&format!("{i}\t{}\t{:?}\t{}\t{}\t{}\t{}\n", p.id, p.material, p.rect[0], p.rect[1], p.rect[2], p.rect[3]));
    }
    write(out.join(format!("{plan}-layout.tsv")), layout.as_bytes())?;
    // Every ramp a pixel may be on: the grass's and each material's, entry by entry.
    let mut ramps = String::new();
    let hex = |r: &[[u8; 3]; 8]| r.iter().map(|c| format!("{:02x}{:02x}{:02x}", c[0], c[1], c[2])).collect::<Vec<_>>().join(" ");
    ramps.push_str(&format!("Grass\t{}\n", hex(&realm_look::HILLSIDE)));
    for m in Material::ALL {
        ramps.push_str(&format!("{m:?}\t{}\n", hex(&works_look::ramp(m))));
    }
    write(out.join("ramps.tsv"), ramps.as_bytes())?;
    let mut tsv = String::from("index\tr\tg\tb\n");
    for i in 0..placed.len() {
        let [r, g, b] = marker(i);
        tsv.push_str(&format!("{i}\t{r}\t{g}\t{b}\n"));
    }
    write(out.join(format!("{plan}-markers.tsv")), tsv.as_bytes())?;
    for (name, dye) in [("old", Dye::Off), ("zones4", Dye::Nearest(4)), (&format!("zones{budget}") as &str, Dye::Budget)] {
        let look = look_of(placed, origin, dye, 540);
        println!("{name}: {} zones in the look", look.zones.len());
        let pixels = gpu.shoot(&world_of(placed, Some(look), false, SIZE)?, 0.0)?;
        write_png(&out.join(format!("{plan}-{name}.png")), gpu.shown(), &pixels)?;
    }
    let pixels = gpu.shoot(&world_of(placed, None, true, SIZE)?, 0.0)?;
    write_png(&out.join(format!("{plan}-markers.png")), gpu.shown(), &pixels)?;
    Ok(())
}

/// A look with `n` zones, each as big as `reach` stage pixels across, piled
/// over the middle of the stage so every pixel is under many of them (or, with
/// `off`, far off the stage so none covers a pixel and the shader only weighs
/// them): the most the shader does for its zones.
fn crowded(n: usize, origin: [f32; 2], stage: (u32, u32), off: bool) -> Look {
    let reach = stage.0 as f32 * 0.9;
    let mut look = look_of(&[], origin, Dye::Off, 540);
    let ramps: Vec<Vec<[u8; 3]>> = Material::ALL.iter().map(|m| works_look::ramp(*m).to_vec()).collect();
    let (cx, cz) = if off { (-50.0 * reach, -50.0 * reach) } else { (stage.0 as f32 / 2.0, stage.1 as f32 / 2.0) };
    look.zones = (0..n)
        .map(|k| Zone {
            ramp: ramps[k % ramps.len()].clone(),
            at: [cx + 0.05 * reach * (k as f32 - n as f32 / 2.0), cz + 0.02 * reach * (k % 5) as f32],
            half: [0.3 * reach, 0.25 * reach],
            radius: 0.05 * reach,
            feather: 0.25 * reach,
        })
        .collect();
    look
}

/// Milliseconds a frame takes, as a line: median, mean, 95th, worst.
fn report(name: &str, ms: &[f64], base: &mut Option<f64>) {
    let mut sorted = ms.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mean = ms.iter().sum::<f64>() / ms.len() as f64;
    let p95 = sorted[(sorted.len() as f64 * 0.95) as usize - 1];
    let median = sorted[sorted.len() / 2];
    let delta = base.map_or(String::new(), |b| format!("  ({:+.1}% against none)", (median / b - 1.0) * 100.0));
    base.get_or_insert(median);
    println!("{name:<58} {median:7.3} / {mean:7.3} / {p95:7.3} / {:7.3}{delta}", sorted[sorted.len() - 1]);
}

/// What the zones cost a frame. First the real screen (the thirty-odd
/// buildings, shadows, the lot) drawn into a 540-row stage: that frame is
/// the CPU's (a few hundred nodes to encode) and the zones are lost in it.
/// Then a fill-bound one, to see the shader: a few buildings on a ground the
/// size of a 4K screen (2160 rows, the most a look draws), where the pixels
/// are the work.
fn time(gpu: &mut Gpu, placed: &[Placed], origin: [f32; 2], frames: usize) -> Result<(), String> {
    println!("{frames} frames each, after 20 of warm up; ms a frame: median / mean / 95th / worst");
    println!("--- the screen of {} buildings, a {}x{} stage (540 rows)", placed.len(), 960, 540);
    gpu.stage = (960, 540);
    gpu.scale = 1;
    let mut base = None;
    let mut sixteen = look_of(placed, origin, Dye::Budget, 540);
    // The Keep's slot, filled as `works_site::dye` fills it: a big stone zone.
    sixteen.zones.push(Zone { ramp: works_look::ramp(Material::Stone).to_vec(), at: [380.0, 60.0], half: [60.0, 40.0], radius: 18.0, feather: 22.0 });
    let runs = vec![
        ("0 zones".to_string(), look_of(placed, origin, Dye::Off, 540)),
        ("4 zones (the old cap): the nearest buildings'".into(), look_of(placed, origin, Dye::Nearest(4), 540)),
        (format!("{} zones: the budget's, and the Keep's", sixteen.zones.len()), sixteen),
        ("16 zones none of which is on the stage".into(), crowded(16, origin, gpu.stage, true)),
        ("16 zones piled over the middle of the stage".into(), crowded(16, origin, gpu.stage, false)),
    ];
    for (name, look) in runs {
        let ms = gpu.time(&world_of(placed, Some(look), false, gpu.stage)?, frames)?;
        report(&name, &ms, &mut base);
    }
    // Fill-bound: a handful of buildings, the stage 3840 x 2160, zones scaled to it.
    println!("--- fill-bound: {} buildings, a 3840x2160 stage (2160 rows, 8.3 million pixels)", placed.len().min(6));
    gpu.stage = (3840, 2160);
    let few = &placed[..placed.len().min(6)];
    let mut base = None;
    for (name, n, off) in [("0 zones", 0usize, false), ("16 zones none of which is on the stage", 16, true), ("4 zones piled over the middle of the stage", 4, false), ("16 zones piled over the middle of the stage", 16, false)] {
        let mut look = crowded(n, origin, gpu.stage, off);
        look.pixel_rows = 2160;
        let ms = gpu.time(&world_of(few, Some(look), false, gpu.stage)?, frames)?;
        report(name, &ms, &mut base);
    }
    Ok(())
}

struct Gpu {
    core: Core,
    device: wgpu::Device,
    info: String,
    target: Option<((u32, u32), wgpu::Texture)>,
    /// The stage's size in its own pixels, and the window pixels to each.
    stage: (u32, u32),
    scale: u32,
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
        let info = adapter.get_info();
        let info = format!("{} ({:?}, {:?}, driver {} {})", info.name, info.backend, info.device_type, info.driver, info.driver_info);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("material-ramps"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;
        device.on_uncaptured_error(Arc::new(|error: wgpu::Error| eprintln!("GPU error: {error}")));
        let core = Core::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
        Ok(Self { core, device, info, target: None, stage: SIZE, scale: SCALE })
    }

    /// The size of the picture: the stage times the scale.
    fn shown(&self) -> (u32, u32) {
        (self.stage.0 * self.scale, self.stage.1 * self.scale)
    }

    fn put(&mut self, world: World, size: (u32, u32)) {
        let frame = angel_stage::Frame {
            size: self.stage,
            layers: Vec::new(),
            pictures: Vec::new(),
            scene: 1,
            enter: None,
            tilt: GAME_TILT,
            props: Some(Props { at: 0, grain: 1, nodes: Vec::new(), world: Some(world) }),
            impact: 0.0,
            meshes: Vec::new(),
        };
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
    }

    fn target(&mut self, size: (u32, u32)) -> wgpu::TextureView {
        if self.target.as_ref().is_none_or(|(s, _)| *s != size) {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("material-ramps-target"),
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
        self.target.as_ref().expect("made above").1.create_view(&Default::default())
    }

    fn drawn(&self) -> Frame {
        let size = self.shown();
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
        drawn
    }

    /// The stage with `world`, drawn and read back at the stage's size times the scale.
    fn shoot(&mut self, world: &World, time: f32) -> Result<Vec<u8>, String> {
        let size = self.shown();
        let mut world = world.clone();
        if let Some(look) = world.look.as_mut() {
            look.time = time;
        }
        self.put(world, size);
        let view = self.target(size);
        let drawn = self.drawn();
        self.core.begin_frame();
        self.core.render_stages((1, 1), 0.0);
        self.core.draw(&view, size, wgpu::Color::BLACK, &drawn, "material-ramps");
        let texture = &self.target.as_ref().expect("made above").1;
        Ok(self.core.read_back(texture, size))
    }

    /// Wall-clock milliseconds a frame of the stage with `world` takes to draw,
    /// `frames` of them after twenty to settle: each frame's stage is put (a
    /// new clock, so it is drawn afresh), then timed from the stage's draw to
    /// the device's last word.
    fn time(&mut self, world: &World, frames: usize) -> Result<Vec<f64>, String> {
        let size = self.shown();
        let view = self.target(size);
        let drawn = self.drawn();
        let mut ms = Vec::with_capacity(frames);
        for f in 0..frames + 20 {
            let mut w = world.clone();
            if let Some(look) = w.look.as_mut() {
                look.time = f as f32 * 0.016;
            }
            self.put(w, size);
            let started = Instant::now();
            self.core.begin_frame();
            self.core.render_stages((1, 1), 0.0);
            self.core.draw(&view, size, wgpu::Color::BLACK, &drawn, "material-ramps-time");
            let _ = self.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(10)) });
            if f >= 20 {
                ms.push(started.elapsed().as_secs_f64() * 1e3);
            }
        }
        Ok(ms)
    }
}

fn write(path: PathBuf, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))
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
