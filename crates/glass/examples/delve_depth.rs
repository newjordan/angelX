//! The deeper floors stand up in 3D (docs/WORLD-3D.md, "Deeper floors"):
//! the same busy fight, four knights firing into a room of monsters, on an
//! upper floor (the 2D black look), down in the deep drawn 2D, and the deep
//! stood up (lit, in 3D), made
//! into a stage by the Delve (`StageMaker`) and drawn by the GPU core
//! offscreen as the window draws it; what each frame costs, and shots.
//!
//! cargo run --release -p angel-glass --example delve_depth -- <outdir> [--frames N] [--scale N]
//!
//! Prints, for each floor, the milliseconds a frame takes (median and 95th
//! percentile): the Delve's scene (`seat_scene`), its stage frame
//! (`StageMaker::make`), the GPU's drawing of it (submitted and waited on),
//! and the stage's nodes, layers and pictures sent. Writes `<floor>.png`
//! mid-fight, and `beat-<t>.png`, the beat of coming down into the deep at
//! `t` seconds.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use angel_delve::render::overworld::arena::{self, stage_frame::StageMaker};
use angel_delve::rules::{EnemyKind, HZ, Input, Pack, Phase, RoomKind, Run};
use angel_delve::world3d::room::{self, Mode};
use angel_gpu::{Core, Draw, Frame, Instance, Picture, Stage};

const STAGE: u32 = 0x00FA_57A4;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("delve_depth: {why}");
            ExitCode::FAILURE
        }
    }
}

/// Four knights in a fight room of floor `depth`, its monsters many.
fn fight(depth: u32) -> Run {
    let mut run = Run::new(11, 1, Some("Knight"));
    for (id, name) in [(2, "Goose"), (3, "Wren"), (4, "Ash")] {
        run.join_seat(id, name);
    }
    run.dungeon.depth = depth;
    run.dungeon.pack = Pack::at(depth, Pack::Crypt);
    let at = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).expect("a fight room");
    run.dungeon.rooms[at].roster = [EnemyKind::Skeleton, EnemyKind::Wraith, EnemyKind::Skeleton, EnemyKind::Wraith].repeat(3);
    run.enter_for_test(at);
    run
}

/// One tick of the fight: every knight walks a little circle and fires
/// round it; nobody falls and the room fills again when it is won.
fn step(run: &mut Run, at: usize) {
    let t = run.tick as f32 / HZ as f32;
    let inputs: BTreeMap<u32, Input> = (1..=4u32)
        .map(|id| {
            let a = t * 0.9 + id as f32 * 1.57;
            let input = Input {
                move_x: if (a * 0.5).sin() > 0.6 { 1 } else if (a * 0.5).sin() < -0.6 { -1 } else { 0 },
                // Up into the room, sweeping across it.
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

fn percentile(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v[((v.len() as f64 * p) as usize).min(v.len() - 1)]
}

fn run() -> Result<(), String> {
    let usage = "usage: delve_depth <outdir> [--frames N] [--scale N]";
    let mut words = std::env::args().skip(1);
    let (mut out, mut frames, mut scale) = (None, 600usize, 3u32);
    while let Some(word) = words.next() {
        match word.as_str() {
            "--frames" => frames = words.next().and_then(|w| w.parse().ok()).ok_or("--frames takes a number")?,
            "--scale" => scale = words.next().and_then(|w| w.parse().ok()).filter(|&s| (1..=8).contains(&s)).ok_or("--scale takes 1 to 8")?,
            _ if out.is_none() => out = Some(PathBuf::from(word)),
            other => return Err(format!("`{other}`: {usage}")),
        }
    }
    let out = out.ok_or(usage)?;
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut gpu = Gpu::new()?;
    room::set_mode(Mode::Deep);
    arena::draw_parts_outside(true);
    arena::props_meshes(true);
    arena::pickup_cards(true);
    let (w, h) = arena::seat_size();
    // The same deep fight drawn 2D (`Mode::Off`) is the fair measure of
    // what standing it up costs; floor 2 is the black look's own.
    for (name, depth, mode) in [("floor2-2d", 2u32, Mode::Deep), ("floor4-2d", room::DEEP, Mode::Off), ("floor4-3d", room::DEEP, Mode::Deep)] {
        room::set_mode(mode);
        let mut run = fight(depth);
        let at = run.at;
        // Into the fight before measuring: shots in the air, monsters up.
        for _ in 0..90 {
            step(&mut run, at);
        }
        let mut maker = StageMaker::default();
        maker.flat_shots = false;
        let (mut scene_ms, mut make_ms, mut gpu_ms) = (Vec::new(), Vec::new(), Vec::new());
        let (mut nodes, mut layers, mut pictures, mut shots) = (0usize, 0usize, 0usize, 0usize);
        for n in 0..frames {
            step(&mut run, at);
            // The client draws in the props it says it can (`client.rs`).
            arena::props_meshes(room::stands_here(&run));
            let t0 = Instant::now();
            let scene = Arc::new(arena::seat_scene(&run, w as i32, h as i32, Some(1)));
            let t1 = Instant::now();
            let frame = maker.make(&scene, |_| None);
            let t2 = Instant::now();
            nodes += frame.props.as_ref().map_or(0, |p| p.nodes.len() + p.world.as_ref().map_or(0, |w| w.floor.len() + w.blocks.len() + w.extra.len() + w.figures.len()));
            layers += frame.layers.len();
            pictures += frame.pictures.len();
            shots += scene.shots.len();
            let size = (frame.size.0 * scale, frame.size.1 * scale);
            let shot = n == frames / 2;
            let t3 = Instant::now();
            let pixels = gpu.stage(frame, size, shot);
            let t4 = Instant::now();
            scene_ms.push((t1 - t0).as_secs_f64() * 1e3);
            make_ms.push((t2 - t1).as_secs_f64() * 1e3);
            gpu_ms.push((t4 - t3).as_secs_f64() * 1e3);
            if let Some(pixels) = pixels {
                write_png(&out.join(format!("{name}.png")), size, &pixels)?;
            }
        }
        let f = frames as f64;
        println!(
            "{name}: scene {:.2}/{:.2} ms, make {:.2}/{:.2} ms, gpu {:.2}/{:.2} ms (median/p95); per frame {:.0} shots, {:.0} nodes, {:.0} layers, {:.1} pictures; stands up {}",
            percentile(&mut scene_ms.clone(), 0.5),
            percentile(&mut scene_ms, 0.95),
            percentile(&mut make_ms.clone(), 0.5),
            percentile(&mut make_ms, 0.95),
            percentile(&mut gpu_ms.clone(), 0.5),
            percentile(&mut gpu_ms, 0.95),
            shots as f64 / f,
            nodes as f64 / f,
            layers as f64 / f,
            pictures as f64 / f,
            room::stands_here(&run),
        );
    }
    // The beat: coming down into the deep, frame by frame.
    room::set_mode(Mode::Deep);
    let mut run = fight(room::DEEP);
    let at = run.at;
    for _ in 0..60 {
        step(&mut run, at);
    }
    run.deep_beat = Some(run.tick);
    let mut maker = StageMaker::default();
    maker.flat_shots = false;
    arena::props_meshes(true);
    let marks = [0.1f32, 0.5, 0.9, 1.4, 2.0, 2.6, 3.2];
    let mut next = 0;
    for _ in 0..(3.4 * HZ as f32) as usize {
        let s = (run.tick - run.deep_beat.unwrap_or(0)) as f32 / HZ as f32;
        let scene = Arc::new(arena::seat_scene(&run, w as i32, h as i32, Some(1)));
        let frame = maker.make(&scene, |_| None);
        let size = (frame.size.0 * scale, frame.size.1 * scale);
        let due = next < marks.len() && s >= marks[next];
        if let Some(pixels) = gpu.stage(frame, size, due) {
            write_png(&out.join(format!("beat-{:.1}.png", marks[next])), size, &pixels)?;
            next += 1;
        }
        step(&mut run, at);
    }
    // Powers change dimensions: on floor 2, the first knight taught a
    // wish's third tier, their shots raised among the others' low ones.
    {
        let mut run = fight(2);
        let at = run.at;
        if let Some(hero) = run.players.get_mut(&1) {
            hero.deck.push(format!("wish-ember-{}", arena::dimensions::RAISED_TIER));
        }
        for _ in 0..75 {
            step(&mut run, at);
        }
        // A ring of every kind of shot from each of two knights: the
        // taught one's (left) raised, the other's (right) low.
        run.enemies.clear();
        run.projectiles.clear();
        for (owner, x) in [(1u8, 16.0f32), (2, 32.0)] {
            let from = run.projectiles.len();
            run.volley_for_test(x, 13.0, 0.3);
            for p in &mut run.projectiles[from..] {
                (p.x, p.y) = (p.x + p.vx * 0.55, p.y + p.vy * 0.45);
            }
            for p in &mut run.projectiles[from..] {
                p.traits.owner = owner;
            }
        }
        let mut maker = StageMaker::default();
        maker.flat_shots = false;
        arena::props_meshes(false);
        let scene = Arc::new(arena::seat_scene(&run, w as i32, h as i32, Some(1)));
        let frame = maker.make(&scene, |_| None);
        let size = (frame.size.0 * scale, frame.size.1 * scale);
        let pixels = gpu.stage(frame, size, true).expect("read back");
        let raised = scene.shots.iter().filter(|s| s.dimension == arena::dimensions::Dimension::Raised).count();
        let owned = run.projectiles.iter().filter(|p| p.traits.owner == 1 && !p.hostile).count();
        let tier = run.players.get(&1).map(arena::dimensions::wish_tier);
        println!("dimensions: {raised} of {} shots raised on floor 2 ({owned} the first knight's, its wish tier {tier:?})", scene.shots.len());
        write_png(&out.join("dimensions.png"), size, &pixels)?;
    }
    // The way down: each floor's first room as the party comes into it,
    // and the dragon's lair, the first room that stands up.
    let mut run = Run::new(9, 1, Some("Knight"));
    run.join_seat(2, "Goose");
    for depth in 1..=angel_delve::rules::DEEPEST {
        while run.dungeon.depth < depth {
            run.descend_for_test();
        }
        run.calm_for_test();
        let mut maker = StageMaker::default();
        maker.flat_shots = false;
        let mut shoot = |run: &Run, name: String, gpu: &mut Gpu| -> Result<(), String> {
            arena::props_meshes(room::stands_here(run));
            let scene = Arc::new(arena::seat_scene(run, w as i32, h as i32, Some(1)));
            let frame = maker.make(&scene, |_| None);
            let size = (frame.size.0 * scale, frame.size.1 * scale);
            let pixels = gpu.stage(frame, size, true).expect("read back");
            println!("{name}: {:?} on floor {}, {:?}, stands up {}", run.room().kind, run.dungeon.depth, run.dungeon.pack, room::stands_here(run));
            write_png(&out.join(format!("{name}.png")), size, &pixels)
        };
        // Past the beat: the room as it stands.
        run.deep_beat = None;
        shoot(&run, format!("way-{depth}"), &mut gpu)?;
        if depth == angel_delve::rules::FLOORS {
            let mut lair = run.clone();
            if let Some(at) = lair.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Lair) {
                lair.clear_prerequisites_for_test();
                lair.enter_for_test(at);
                for _ in 0..40 {
                    step(&mut lair, at);
                }
                shoot(&lair, format!("way-{depth}-lair"), &mut gpu)?;
            }
        }
    }
    println!("wrote the shots to {}", out.display());
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
        println!("adapter: {:?}", adapter.get_info().name);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("delve-depth"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;
        device.on_uncaptured_error(Arc::new(|error: wgpu::Error| eprintln!("GPU error: {error}")));
        let core = Core::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
        Ok(Self { core, device, target: None })
    }

    /// A Delve frame as a stage, drawn into a texture `size` pixels as the
    /// window draws a stage in its cells, on black, waited on; read back
    /// when `read`.
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
                label: Some("delve-depth-target"),
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
        self.core.draw(&view, size, wgpu::Color::BLACK, &drawn, "delve-depth");
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
