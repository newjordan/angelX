//! The lift as the window draws it (`world3d::lift`): a lit room changing
//! dimension, shot moment by moment. The room is stood up lit as
//! `delve_rooms` stands it, its frame made by the Delve (`StageMaker`), and
//! each moment is that frame with the lift applied at a time, drawn by the
//! GPU core offscreen as the window draws a stage.
//!
//! cargo run --release -p angel-glass --example delve_lift -- <outdir> [--scale N] [--only MOVE]... [--case NAME]... [--ripple doors|centre] [--fps F]
//!
//! The moves are `prop` (the masses lift and the flames catch), `floor` (the
//! relief rises, the sun comes down, the pools bloom), `tip` (the room tips)
//! and `climb` (all three as one); each with the other moves done, so what
//! moves is what is named. Writes `<case>/<move>/NN-tX.XXXs.png`: a strip of
//! frames across the move, more of them where it lands; with `--fps`, a frame
//! every `1/F` second instead (`<case>/<move>-fps/NNNN.png`), to make a GIF.
//! `--case` is `crypt-fight` (the default), `crypt-passage`, `archive-passage`
//! or `mines-fight`; `--ripple` is where the ripples start (the room's doors
//! by default, or its middle).

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_delve::render::overworld::arena::{self, UNIT, stage_frame::StageMaker};
use angel_delve::rules::{Pack, Rng, Room, RoomKind, Run, Tile, layout};
use angel_delve::world3d::lift::{self, Ripple, Times};
use angel_delve::world3d::{self, plain, rooms_lit};
use angel_gpu::{Core, Draw, Frame, Instance, Picture, Stage};

/// The stage's picture id, as the cockpit names the Delve's.
const STAGE: u32 = 0x00FA_57A4;
/// A tile, in the room's pixels.
const TILE: f32 = 16.0;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("delve_lift: {why}");
            ExitCode::FAILURE
        }
    }
}

/// A move of the lift: its name, how long it is, and the times a strip of it
/// is shot at: evenly, and more where it lands (fractions of its length).
struct Move {
    name: &'static str,
    duration: f32,
    landing: (f32, f32),
    times: fn(f32) -> Times,
}

const MOVES: [Move; 4] = [
    Move { name: "prop", duration: lift::PROP_DURATION, landing: (0.20, 0.62), times: |t| Times { prop: t, ..Times::DONE } },
    Move { name: "floor", duration: lift::FLOOR_DURATION, landing: (0.30, 0.55), times: |t| Times { floor: t, ..Times::DONE } },
    Move { name: "tip", duration: lift::TIP_DURATION, landing: (0.55, 0.90), times: |t| Times { tip: t, ..Times::DONE } },
    Move { name: "climb", duration: lift::DURATION, landing: (0.35, 0.80), times: lift::climb },
];

/// How many frames evenly across a strip, and how many more in its landing.
const EVEN: usize = 12;
const LANDING: usize = 8;

/// A shot: its name and the run as it stands.
struct Case {
    name: String,
    run: Run,
}

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
    let all = [
        ("crypt-fight", Pack::Crypt, 1u32, 3u64, RoomKind::Fight),
        ("crypt-passage", Pack::Crypt, 2, 9, RoomKind::Hall),
        ("archive-passage", Pack::Archive, 2, 11, RoomKind::Hall),
        ("mines-fight", Pack::Cavern, 2, 5, RoomKind::Fight),
    ];
    let wanted = |name: &str| if only.is_empty() { name == "crypt-fight" } else { only.iter().any(|o| o == name) };
    all.into_iter()
        .filter(|c| wanted(c.0))
        .filter_map(|(name, pack, depth, seed, kind)| match seeded(pack, depth, seed, kind) {
            Some(run) => Some(Case { name: name.to_string(), run }),
            None => {
                eprintln!("{name}: no {kind:?} room on {pack:?} floor {depth}");
                None
            }
        })
        .collect()
}

/// The times of a strip of `m`: evenly, and more where it lands.
fn strip(m: &Move) -> Vec<f32> {
    let mut times: Vec<f32> = (0..EVEN).map(|i| m.duration * i as f32 / (EVEN - 1) as f32).collect();
    times.extend((0..LANDING).map(|i| m.duration * (m.landing.0 + (m.landing.1 - m.landing.0) * i as f32 / (LANDING - 1) as f32)));
    times.sort_by(f32::total_cmp);
    times.dedup_by(|a, b| (*a - *b).abs() < 0.015);
    times
}

fn run() -> Result<(), String> {
    let usage = "usage: delve_lift <outdir> [--scale N] [--only MOVE]... [--case NAME]... [--ripple doors|centre] [--fps F]";
    let mut words = std::env::args().skip(1);
    let (mut out, mut scale, mut moves, mut named, mut doors, mut fps) = (None, 3u32, Vec::new(), Vec::new(), true, None::<f32>);
    while let Some(word) = words.next() {
        match word.as_str() {
            "--scale" => {
                scale = words.next().and_then(|w| w.parse().ok()).filter(|&s| (1..=8).contains(&s)).ok_or("--scale takes 1 to 8")?;
            }
            "--only" => moves.push(words.next().ok_or(usage)?),
            "--case" => named.push(words.next().ok_or(usage)?),
            "--ripple" => match words.next().as_deref() {
                Some("centre") => doors = false,
                Some("doors") => doors = true,
                _ => return Err("--ripple takes centre or doors".into()),
            },
            "--fps" => fps = Some(words.next().and_then(|w| w.parse().ok()).filter(|&f| (1.0..=120.0).contains(&f)).ok_or("--fps takes 1 to 120")?),
            _ if out.is_none() => out = Some(PathBuf::from(word)),
            other => return Err(format!("`{other}`: {usage}")),
        }
    }
    let out = out.ok_or(usage)?;
    let mut gpu = Gpu::new()?;
    arena::draw_parts_outside(true);
    let (w, h) = arena::seat_size();
    for case in cases(&named) {
        let id = &case.name;
        world3d::set_enabled(true);
        arena::props_meshes(true);
        let scene = Arc::new(arena::seat_scene(&case.run, w as i32, h as i32, Some(1)));
        let world = scene.world.as_ref().ok_or_else(|| format!("{id}: no world in 3D"))?;
        let lit = world.lit().ok_or_else(|| format!("{id}: not lit"))?;
        let room = lit.room();
        let (rw, rh) = (room.cols as f32 * TILE, room.rows as f32 * TILE);
        let door_tiles: Vec<[f32; 2]> = (0..room.rows as i32)
            .flat_map(|r| (0..room.cols as i32).map(move |c| (c, r)))
            .filter(|&(c, r)| room.tile(c, r) == Tile::Door)
            .map(|(c, r)| [(c as f32 + 0.5) * TILE, (r as f32 + 0.5) * TILE])
            .collect();
        let ripple = if doors { Ripple::doors(&door_tiles, rw, rh) } else { Ripple::centre(rw, rh) };
        println!("{id}: {:?} room {}x{}, {} door(s), tone {}", room.kind, room.cols, room.rows, door_tiles.len(), lit.tone.name());
        let frame = StageMaker::default().make(&scene, |_| None);
        for m in MOVES.iter().filter(|m| moves.is_empty() || moves.iter().any(|o| o == m.name)) {
            let (dir, times): (PathBuf, Vec<(String, f32)>) = match fps {
                Some(f) => {
                    let n = (m.duration * f).ceil() as usize;
                    let times = (0..=n).map(|i| (format!("{i:04}"), (i as f32 / f).min(m.duration))).collect();
                    (out.join(id).join(format!("{}-fps", m.name)), times)
                }
                None => (
                    out.join(id).join(m.name),
                    strip(m).into_iter().enumerate().map(|(i, t)| (format!("{i:02}-t{t:.3}"), t)).collect(),
                ),
            };
            std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            for (name, t) in times {
                let mut moment = frame.clone();
                lift::apply(&mut moment, (m.times)(t), &ripple);
                let size = (moment.size.0 * scale, moment.size.1 * scale);
                let pixels = gpu.stage(moment, size);
                write_png(&dir.join(format!("{name}.png")), size, &pixels)?;
            }
            println!("{id}: {} -> {}", m.name, dir.display());
        }
    }
    world3d::set_enabled(false);
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
            label: Some("delve-lift"),
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
            label: Some("delve-lift-target"),
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
        self.core.draw(&view, size, wgpu::Color::BLACK, &drawn, "delve-lift");
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
