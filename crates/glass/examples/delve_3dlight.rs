//! The deep floors' light, reviewed (docs/WORLD-3D.md, "Deeper floors"):
//! rooms of the floors that stand up in 3D, each with what a party has to
//! see in it (walls, pillars, hazards, pickups on the floor, monsters,
//! the way out), made into a stage by the Delve (`StageMaker`) and drawn
//! offscreen by the GPU core as the window and a browser seat draw it.
//!
//! cargo run --release -p angel-glass --example delve_3dlight -- <outdir> [--scale N] [--only NAME]
//!
//! Writes `<name>.png` for each scene and prints, for each, how much of the
//! play space sits against the brand guide's frame numbers (docs/BRAND.md,
//! `angel_stage::brand::audit_frame`): its darkest, its median, its bulk.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_delve::render::overworld::arena::{self, stage_frame::StageMaker};
use angel_delve::rules::{EnemyKind, HZ, Input, Item, Pack, Phase, RoomKind, Run, Tile};
use angel_delve::world3d::room::{self, Mode};
use angel_gpu::{Core, Draw, Frame, Instance, Picture, Stage};

const STAGE: u32 = 0x00FA_57A4;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("delve_3dlight: {why}");
            ExitCode::FAILURE
        }
    }
}

/// One scene to shoot.
struct Scene {
    name: &'static str,
    depth: u32,
    kind: RoomKind,
    /// The room is a fight under way (monsters, shots) or won (embers, loot).
    fighting: bool,
    /// Seeds to try: the room of most stone and hazard wins.
    seeds: &'static [u64],
}

const SCENES: &[Scene] = &[
    Scene { name: "d4-archive-fight", depth: 4, kind: RoomKind::Fight, fighting: true, seeds: &[11, 12, 13, 14] },
    Scene { name: "d4-archive-won", depth: 4, kind: RoomKind::Fight, fighting: false, seeds: &[11, 12, 13, 14] },
    Scene { name: "d5-fungal-fight", depth: 5, kind: RoomKind::Fight, fighting: true, seeds: &[21, 22, 23, 24] },
    Scene { name: "d5-fungal-treasure", depth: 5, kind: RoomKind::Treasure, fighting: false, seeds: &[21, 22, 23, 24] },
    Scene { name: "d6-unknown-fight", depth: 6, kind: RoomKind::Fight, fighting: true, seeds: &[31, 32, 33, 34] },
    Scene { name: "d3-hell-lair", depth: 3, kind: RoomKind::Lair, fighting: true, seeds: &[9] },
    Scene { name: "d4-archive-stairs", depth: 4, kind: RoomKind::Stairs, fighting: false, seeds: &[11, 12, 13, 14] },
];

/// Tiles of stone and of hazard in a room.
fn stuff(room: &angel_delve::rules::Room) -> (usize, usize) {
    let (mut blocks, mut hazards) = (0, 0);
    for row in 0..room.rows as i32 {
        for col in 0..room.cols as i32 {
            match room.tile(col, row) {
                Tile::Block => blocks += 1,
                Tile::Hazard => hazards += 1,
                _ => {}
            }
        }
    }
    (blocks, hazards)
}

/// The run for a scene: four knights in the best room of the kind on the
/// floor, among the seeds.
fn build(scene: &Scene) -> Result<Run, String> {
    let mut best: Option<(usize, Run, usize)> = None;
    for &seed in scene.seeds {
        let mut run = Run::new(seed, 1, Some("Knight"));
        for (id, name) in [(2, "Goose"), (3, "Wren"), (4, "Ash")] {
            run.join_seat(id, name);
        }
        while run.dungeon.depth < scene.depth {
            run.descend_for_test();
        }
        run.calm_for_test();
        for (at, room) in run.dungeon.rooms.iter().enumerate() {
            if room.kind != scene.kind {
                continue;
            }
            let (blocks, hazards) = stuff(room);
            let score = blocks.min(40) + hazards.min(40) * 2;
            if best.as_ref().is_none_or(|(s, _, _)| score > *s) {
                best = Some((score, run.clone(), at));
            }
        }
    }
    let (_, mut run, at) = best.ok_or_else(|| format!("{}: no {:?} room on floor {}", scene.name, scene.kind, scene.depth))?;
    run.dungeon.pack = Pack::at(scene.depth, Pack::Crypt);
    if scene.fighting {
        run.dungeon.rooms[at].roster = [EnemyKind::Skeleton, EnemyKind::Wraith, EnemyKind::Imp, EnemyKind::Bat].repeat(3);
    }
    run.clear_prerequisites_for_test();
    run.enter_for_test(at);
    run.deep_beat = None;
    // What lies on the floor: arms, a heart, a shield, a little way up the
    // room from the knights.
    let feet: Vec<(f32, f32)> = run.players.values().map(|h| (h.x, h.y)).collect();
    for (n, card) in ["bow", "knights-blade", "heart", "shield"].iter().enumerate() {
        let (x, y) = feet.get(n).copied().unwrap_or((20.0, 20.0));
        run.dungeon.rooms[at].items.push(Item::lying((*card).into(), x + 6.0 + 2.0 * n as f32, y - 9.0, 1));
    }
    if scene.fighting {
        for _ in 0..90 {
            step(&mut run, at);
        }
    } else {
        run.clear_for_test();
        run.dungeon.rooms[at].cleared = true;
        for _ in 0..30 {
            run.step(&BTreeMap::new());
        }
    }
    run.dare = None;
    Ok(run)
}

/// One tick of the fight, as `delve_depth` plays it.
fn step(run: &mut Run, at: usize) {
    let t = run.tick as f32 / HZ as f32;
    let inputs: BTreeMap<u32, Input> = (1..=4u32)
        .map(|id| {
            let a = t * 0.9 + id as f32 * 1.57;
            let input = Input {
                move_x: if (a * 0.5).sin() > 0.6 { 1 } else if (a * 0.5).sin() < -0.6 { -1 } else { 0 },
                aim_x: ((a * 2.3).cos() * 1.4).round() as i8,
                aim_y: -1,
                fire: true,
                ..Default::default()
            };
            (id, input)
        })
        .collect();
    for hero in run.players.values_mut() {
        hero.hp = hero.max_hp;
    }
    run.step(&inputs);
    if run.phase != Phase::Fighting && run.at == at {
        run.enter_for_test(at);
    }
}

fn run() -> Result<(), String> {
    let usage = "usage: delve_3dlight <outdir> [--scale N] [--only NAME]";
    let mut words = std::env::args().skip(1);
    let (mut out, mut scale, mut only) = (None, 3u32, None::<String>);
    while let Some(word) = words.next() {
        match word.as_str() {
            "--scale" => scale = words.next().and_then(|w| w.parse().ok()).filter(|&s| (1..=8).contains(&s)).ok_or("--scale takes 1 to 8")?,
            "--only" => only = Some(words.next().ok_or("--only takes a name")?),
            _ if out.is_none() => out = Some(PathBuf::from(word)),
            other => return Err(format!("`{other}`: {usage}")),
        }
    }
    let out = out.ok_or(usage)?;
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut gpu = Gpu::new()?;
    room::set_mode(Mode::Deep);
    arena::draw_parts_outside(true);
    arena::pickup_cards(true);
    let (w, h) = arena::seat_size();
    for scene in SCENES.iter().filter(|s| only.as_deref().is_none_or(|o| o == s.name)) {
        let run = build(scene)?;
        arena::props_meshes(room::stands_here(&run));
        let mut maker = StageMaker::default();
        maker.flat_shots = false;
        let seen = Arc::new(arena::seat_scene(&run, w as i32, h as i32, Some(1)));
        if std::env::var_os("FIGS").is_some() {
            for f in &seen.figures {
                eprintln!("figure foe={} additive={} height={} x={} frac={:?} depth={} w={}", f.foe, f.additive, f.height, f.x, f.frac, f.depth, f.img.w);
            }
        }
        let frame = maker.make(&seen, |_| None);
        let size = (frame.size.0 * scale, frame.size.1 * scale);
        let pixels = gpu.stage(frame, size, true).ok_or("no read back")?;
        let (blocks, hazards) = stuff(run.room());
        let stats = darkness(&pixels, size);
        println!(
            "{}: {:?} on floor {} ({:?}), {blocks} stone, {hazards} hazard, {} items, {} monsters, stands up {}; {}",
            scene.name,
            run.room().kind,
            run.dungeon.depth,
            run.dungeon.pack,
            run.room().items.len(),
            run.enemies.len(),
            room::stands_here(&run),
            stats,
        );
        write_png(&out.join(format!("{}.png", scene.name)), size, &pixels)?;
    }
    println!("wrote the shots to {}", out.display());
    Ok(())
}

/// The brand guide's frame numbers (`angel_stage::brand::audit_frame`) over
/// the play space: the rows between the HUD's bars, black border and all.
fn darkness(rgba: &[u8], size: (u32, u32)) -> String {
    let (w, h) = (size.0 as usize, size.1 as usize);
    let (y0, y1) = ((h as f32 * 0.10) as usize, (h as f32 * 0.85) as usize);
    let pixels: Vec<[u8; 3]> = rgba[y0 * w * 4..y1 * w * 4].chunks_exact(4).map(|p| [p[0], p[1], p[2]]).collect();
    let (s, breaches) = angel_stage::brand::audit_frame(&pixels);
    let rules: Vec<&str> = breaches.iter().map(|b| b.rule).collect();
    format!("p1 {:.3} median {:.3} p95 {:.3} bright {:.2}% vivid {:.2}%; breaks {rules:?}", s.p1, s.p50, s.p95, s.bright * 100.0, s.vivid * 100.0)
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
        println!("adapter: {:?}", adapter.get_info().name);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("delve-3dlight"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;
        device.on_uncaptured_error(Arc::new(|error: wgpu::Error| eprintln!("GPU error: {error}")));
        let core = Core::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
        Ok(Self { core, device, target: None })
    }

    /// A Delve frame as a stage, drawn into a texture `size` pixels as the
    /// window draws a stage in its cells, on black, and read back.
    fn stage(&mut self, frame: angel_stage::Frame, size: (u32, u32), read: bool) -> Option<Vec<u8>> {
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
        if self.target.as_ref().is_none_or(|(s, _)| *s != size) {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("delve-3dlight-target"),
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
        let texture = &self.target.as_ref().expect("a target").1;
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
        self.core.draw(&view, size, wgpu::Color::BLACK, &drawn, "delve-3dlight");
        if read {
            return Some(self.core.read_back(texture, size));
        }
        let _ = self.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(5)) });
        None
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
