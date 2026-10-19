//! Rooms lit as the window draws them (docs/REALM-MONOLITH.md, "Rooms,
//! lit"): the kingdom's places and interiors, a chamber of every delve, and
//! the seeded floors' fight rooms, stairs, passages and open floors, knights
//! standing in each, every one stood up in the monolith look with no plate
//! (`world3d::rooms_lit`). Made into a stage by the Delve (`StageMaker`) and
//! drawn by the GPU core offscreen, as the window draws the Delve's stage;
//! the same room flat, and a terminal's frame of it, beside.
//!
//! cargo run --release -p angel-glass --example delve_rooms -- <outdir> [--scale N] [--only NAME]...
//!
//! Writes `<case>.png` (the window, lit, at `N` window pixels to a stage
//! pixel, 3 unless said), `<case>-flat.png` (rooms in 3D off) and
//! `<case>-terminal.png` (a terminal's frame, lit, at 3x), and prints each
//! case's room, tone and pieces.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_delve::render::overworld::arena::{self, UNIT, stage_frame::StageMaker};
use angel_delve::rules::{Pack, Rng, Room, RoomKind, Run, Tile, layout};
use angel_delve::world3d::{self, plain, room, rooms_lit};
use angel_gpu::{Core, Draw, Frame, Instance, Picture, Stage};

/// The stage's picture id, as the cockpit names the Delve's.
const STAGE: u32 = 0x00FA_57A4;
/// A tile, in the room's pixels.
const TILE: f32 = 16.0;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("delve_rooms: {why}");
            ExitCode::FAILURE
        }
    }
}

/// A shot: its name and the run as it stands.
struct Case {
    name: String,
    run: Run,
}

/// The places, interiors and chambers shot from their items' proof rooms
/// (`room::proof`: the item's room as a run stands in it, a knight half
/// behind its tallest block and one in front of one).
const PROOFS: [&str; 15] = [
    "village",
    "kings-hall",
    "tavern",
    "priory",
    "smithy",
    "market",
    "sanctuary",
    "college-hall",
    "crypt-bone-chapel",
    "cavern-boulder-rill",
    "hellforge-lava-ford",
    "archive-map-table",
    "fungal-fairy-ring",
    "unknown-void-puddle",
    "wizard-study",
];

/// Where a knight stands in room `room` (on a floor of `pack`): on open floor
/// just behind (north of) the tallest thing its lit blockout stands near
/// `near`, so it hides the knight's legs; the open floor nearest `near` when
/// nothing stands. Its feet, in the room's pixels.
fn spot(room: &Room, pack: Pack, near: (f32, f32), reach: i32) -> (f32, f32) {
    let walk = |c: i32, r: i32| {
        (0..room.cols as i32).contains(&c) && (0..room.rows as i32).contains(&r) && matches!(room.tile(c, r), Tile::Floor | Tile::Door)
    };
    let shapes: Vec<angel_stage::set::Shape> = rooms_lit::lit_for(room, pack)
        .map(|lit| {
            let (c, r) = ((near.0 / TILE) as i32, (near.1 / TILE) as i32);
            let rect = [(c - reach) as f32 * TILE, (r - reach) as f32 * TILE, (c + reach) as f32 * TILE, (r + reach) as f32 * TILE];
            lit.near(rect).iter().flat_map(|p| p.set.shapes.clone()).collect()
        })
        .unwrap_or_default();
    let tall = |c: i32, r: i32| plain::over_tile(&shapes, (c, r)).map_or(0.0, |s| s.at[1] + s.size[1]);
    let (nc, nr) = ((near.0 / TILE) as i32, (near.1 / TILE) as i32);
    let mut best: Option<(f32, i32, i32)> = None;
    for r in (nr - reach).max(1)..(nr + reach).min(room.rows as i32 - 2) {
        for c in (nc - reach).max(1)..(nc + reach).min(room.cols as i32 - 1) {
            if !walk(c, r) || walk(c, r + 1) {
                continue;
            }
            let h = tall(c, r + 1);
            if h < 8.0 {
                continue;
            }
            let d = ((c - nc).pow(2) + (r - nr).pow(2)) as f32;
            let score = h - d * 0.5;
            if best.is_none_or(|b| score > b.0) {
                best = Some((score, c, r));
            }
        }
    }
    if let Some((_, c, r)) = best {
        return ((c as f32 + 0.5) * TILE, (r + 1) as f32 * TILE - 2.0);
    }
    let mut open = None;
    for d in 0..reach * 2 {
        for r in nr - d..=nr + d {
            for c in nc - d..=nc + d {
                if open.is_none() && walk(c, r) {
                    open = Some(((c as f32 + 0.5) * TILE, (r as f32 + 0.5) * TILE));
                }
            }
        }
    }
    open.unwrap_or(near)
}

/// Stand the run's knights at `feet` and beside it (the room's pixels), the
/// view on the first.
fn stand(run: &mut Run, feet: (f32, f32)) {
    for (k, hero) in run.players.values_mut().enumerate() {
        let dx = k as f32 * 22.0;
        (hero.x, hero.y) = ((feet.0 + dx) / UNIT, feet.1 / UNIT);
        hero.camera = Some((hero.x, hero.y - 1.0));
    }
}

/// A run on floor `depth` of `pack` (seed `seed`) standing in its first room
/// of kind `kind`, two knights in it.
fn seeded(pack: Pack, depth: u32, seed: u64, kind: RoomKind) -> Option<Run> {
    // The first such room the window stands up (a room it draws as one
    // picture, the Swamp's fog, is passed over), on this seed or the next.
    for seed in seed..seed + 8 {
        let floor = layout::floor_party(depth, pack, 2, &mut Rng::new(seed));
        for at in (0..floor.rooms.len()).filter(|&i| floor.rooms[i].kind == kind) {
            let mut run = Run::new(seed, seed, Some("Friend"));
            run.join_seat(2, "Goose Inspector");
            run.dungeon = floor.clone();
            run.enter_for_test(at);
            let room = run.room().clone();
            let (w, h) = (room.cols as f32 * TILE, room.rows as f32 * TILE);
            let feet = spot(&room, pack, (w / 2.0, h / 2.0), 6);
            stand(&mut run, feet);
            if stood(&run) {
                return Some(run);
            }
        }
    }
    None
}

/// Whether the window stands run `run`'s room up in 3D.
fn stood(run: &Run) -> bool {
    let (w, h) = arena::seat_size();
    world3d::set_enabled(true);
    arena::props_meshes(true);
    let stood = arena::seat_scene(run, w as i32, h as i32, Some(1)).world.is_some();
    world3d::set_enabled(false);
    stood
}

fn cases(only: &[String]) -> Vec<Case> {
    let mut out = Vec::new();
    for id in PROOFS {
        match room::proof(id) {
            Some(proof) => out.push(Case { name: id.to_string(), run: proof.run }),
            None => eprintln!("{id}: no proof room"),
        }
    }
    let seeded_cases = [
        ("crypt-fight", Pack::Crypt, 1u32, 3u64, RoomKind::Fight),
        ("swamp-fight", Pack::Swamp, 1, 19, RoomKind::Fight),
        ("swamp-passage", Pack::Swamp, 1, 19, RoomKind::Hall),
        ("swamp-start", Pack::Swamp, 2, 19, RoomKind::Start),
        ("mines-fight", Pack::Cavern, 2, 5, RoomKind::Fight),
        ("crypt-stairs", Pack::Crypt, 1, 3, RoomKind::Stairs),
        ("hellforge-stairs", Pack::Hellforge, 2, 7, RoomKind::Stairs),
        ("crypt-passage", Pack::Crypt, 2, 9, RoomKind::Hall),
        ("archive-passage", Pack::Archive, 2, 11, RoomKind::Hall),
        ("fungal-fight", Pack::Fungal, 1, 13, RoomKind::Fight),
        ("unknown-fight", Pack::Unknown, 3, 17, RoomKind::Fight),
    ];
    for (name, pack, depth, seed, kind) in seeded_cases {
        match seeded(pack, depth, seed, kind) {
            Some(run) => out.push(Case { name: name.to_string(), run }),
            None => eprintln!("{name}: no {kind:?} room on {pack:?} floor {depth}"),
        }
    }
    // The crypt's fight room once it is won: its braziers sink to embers.
    if let Some(mut run) = seeded(Pack::Crypt, 1, 3, RoomKind::Fight) {
        run.enemies.clear();
        let at = run.at;
        run.dungeon.rooms[at].cleared = true;
        out.push(Case { name: "crypt-fight-cleared".into(), run });
    }
    // An open floor: the map round the arrival.
    let mut run = Run::new(24, 23, Some("Friend"));
    run.first = Pack::Crypt;
    run.open_floor_for_test(1);
    let pack = run.dungeon.pack;
    if let Some(open) = run.dungeon.open.clone() {
        let map = run.room().clone();
        let arrival = (open.arrival.0 * UNIT, open.arrival.1 * UNIT);
        stand(&mut run, spot(&map, pack, arrival, 5));
        out.push(Case { name: "crypt-open-floor".into(), run });
    }
    out.retain(|c| only.is_empty() || only.iter().any(|o| c.name.contains(o.as_str())));
    out
}

fn run() -> Result<(), String> {
    let usage = "usage: delve_rooms <outdir> [--scale N] [--only NAME]...";
    let mut words = std::env::args().skip(1);
    let (mut out, mut scale, mut only) = (None, 3u32, Vec::new());
    while let Some(word) = words.next() {
        match word.as_str() {
            "--scale" => {
                scale = words.next().and_then(|w| w.parse().ok()).filter(|&s| (1..=8).contains(&s)).ok_or("--scale takes 1 to 8")?;
            }
            "--only" => only.push(words.next().ok_or(usage)?),
            _ if out.is_none() => out = Some(PathBuf::from(word)),
            other => return Err(format!("`{other}`: {usage}")),
        }
    }
    let out = out.ok_or(usage)?;
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut gpu = Gpu::new()?;
    arena::draw_parts_outside(true);
    let (w, h) = arena::seat_size();
    for case in cases(&only) {
        let id = &case.name;
        // Another room between frames: a room's kept still art is keyed by
        // the room and its torch step, not by the switch.
        let mut elsewhere = case.run.clone();
        if let Some(other) = (0..case.run.dungeon.rooms.len()).find(|&i| i != case.run.at && case.run.dungeon.rooms[i].kind != RoomKind::Depths) {
            elsewhere.enter_for_test(other);
        }
        for on in [false, true] {
            world3d::set_enabled(on);
            arena::props_meshes(true);
            let scene = Arc::new(arena::seat_scene(&case.run, w as i32, h as i32, Some(1)));
            if on {
                let world = scene.world.as_ref().ok_or_else(|| format!("{id}: no world in 3D"))?;
                let lit = world.lit().ok_or_else(|| format!("{id}: not lit"))?;
                let room = case.run.room();
                println!(
                    "{id}: {:?} room {}x{}, tone {}, dressed {}, {} piece(s), {:?}",
                    room.kind,
                    room.cols,
                    room.rows,
                    lit.tone.name(),
                    lit.id,
                    lit.near([scene.camera.0, scene.camera.1, scene.camera.0 + w as f32, scene.camera.1 + h as f32]).len(),
                    world.moment.activity,
                );
            }
            let frame = StageMaker::default().make(&scene, |_| None);
            let size = (frame.size.0 * scale, frame.size.1 * scale);
            let pixels = gpu.stage(frame, size);
            let name = if on { format!("{id}.png") } else { format!("{id}-flat.png") };
            write_png(&out.join(&name), size, &pixels)?;
            if on {
                // A terminal's frame: no scenes, the room lit from above.
                arena::props_meshes(false);
                let _ = arena::frame_for(&elsewhere, arena::NATIVE_W, arena::NATIVE_H, Some(1));
                let img = arena::frame_for(&case.run, arena::NATIVE_W, arena::NATIVE_H, Some(1));
                write_png(&out.join(format!("{id}-terminal.png")), (img.w as u32 * 3, img.h as u32 * 3), &img.rgba_scaled(3))?;
            }
        }
    }
    world3d::set_enabled(false);
    println!("wrote the rooms to {}", out.display());
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
            label: Some("delve-rooms"),
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
            label: Some("delve-rooms-target"),
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
        self.core.draw(&view, size, wgpu::Color::BLACK, &drawn, "delve-rooms");
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
