//! The kingdom in 3D as the window draws it (docs/WORLD-3D.md, "The
//! kingdom"; docs/REALM-MONOLITH.md, "The realm, lit"): a knight at each of
//! a few places out in the kingdom (`world3d::kingdom::proof_run`), the
//! chunks near its view stood up lit through the realm's look, made into a
//! stage by the Delve (`StageMaker`) and drawn by the GPU core offscreen, as
//! the window draws the Delve's stage. Once with the world in 3D and once
//! flat.
//!
//! cargo run --release -p angel-glass --example delve_kingdom -- <outdir> [--scale N] [--only NAME]... [--lots] [--contact]
//! cargo run --release -p angel-glass --example delve_kingdom -- --walk
//!
//! Writes `<place>.png` and `<place>-flat.png` (the stage at `N` window
//! pixels to a stage pixel, 3 unless said). `--walk` walks a knight east
//! across three chunk borders and prints the frame's cost, flat and in 3D.
//! `--lots` shoots what a run stands that no chunk's set has
//! (`world3d::lots`) instead: the realm's catalog wishes built beside the
//! Village, a knight before them and one behind, and a few of the run's
//! finds, each with a knight by it. `--contact` also writes `contact.png`,
//! each place flat and in 3D side by side, a row a place.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use angel_delve::render::overworld::arena::{self, stage_frame::StageMaker};
use angel_delve::world3d::{self, kingdom};
use angel_gpu::{Core, Draw, Frame, Instance, Picture, Stage};
use angel_stage::set::Kind;

/// The stage's picture id, as the cockpit names the Delve's.
const STAGE: u32 = 0x00FA_57A4;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("delve_kingdom: {why}");
            ExitCode::FAILURE
        }
    }
}

/// A place to shoot: its name, where the knights stand (the first is the
/// one the view follows), and what the party has built.
struct Place {
    name: String,
    feet: Vec<[f32; 2]>,
    wishes: Vec<angel_delve::realm::Wish>,
}

/// The middle of chunk `(cx, cz)`, nudged by `(dx, dz)` pixels.
fn mid(cx: i32, cz: i32, dx: f32, dz: f32) -> [f32; 2] {
    [(cx as f32 + 0.5) * kingdom::STAGE.0 + dx, (cz as f32 + 0.5) * kingdom::STAGE.1 + dz]
}

/// The nearest tile a knight walks to kingdom point `at` (its middle's
/// foot), searching outward a tile at a time.
fn walkable_near(at: [f32; 2]) -> [f32; 2] {
    let land = kingdom::land();
    let (c, r) = ((at[0] / 16.0).floor() as i32, (at[1] / 16.0).floor() as i32);
    for d in 0..12i32 {
        for dy in -d..=d {
            for dx in -d..=d {
                if dx.abs() != d && dy.abs() != d {
                    continue;
                }
                if land.walkable(c + dx, r + dy) {
                    return [(c + dx) as f32 * 16.0 + 8.0, (r + dy) as f32 * 16.0 + 13.0];
                }
            }
        }
    }
    at
}

/// A place's stand (the tile before its door), a knight's feet on it.
fn stand(t: (i32, i32)) -> [f32; 2] {
    [t.0 as f32 * 16.0 + 8.0, t.1 as f32 * 16.0 + 13.0]
}

fn places() -> Vec<Place> {
    use angel_delve::world::{houses, map::Place as P};
    let stone = [Kind::Stone, Kind::Plaster, Kind::Slate, Kind::Wood];
    let at = |name: &str, feet: Vec<[f32; 2]>| Place { name: name.into(), feet, wishes: Vec::new() };
    let behind = |cx: i32, cz: i32, kinds: &[Kind]| kingdom::spot_behind(cx, cz, kinds).unwrap_or(mid(cx, cz, 0.0, 0.0));
    let castle = houses::ids().nth(3).expect("a house");
    let other = houses::ids().nth(4).expect("a house");
    let mut out = vec![
        at("castle-town", vec![stand(P::Keep.stand_world()), walkable_near(mid(3, 3, 60.0, 50.0))]),
        at("village", vec![behind(3, 5, &stone), walkable_near(mid(3, 5, -50.0, 50.0))]),
        at("coast", vec![walkable_near(mid(9, 4, -40.0, 0.0))]),
        at("fen", vec![walkable_near(mid(0, 6, 0.0, 0.0))]),
        at("wastes", vec![stand(P::DragonKeep.stand_world()), walkable_near(mid(7, 1, 70.0, 60.0))]),
        at("march-castles", vec![stand(houses::stand(castle)), stand(houses::stand(other))]),
        at("greenwood", vec![kingdom::spot_behind_trees(1, 1).or_else(|| kingdom::spot_behind_trees(0, 0)).unwrap_or(mid(1, 1, 0.0, 0.0))]),
        at("highlands-delve-gate", vec![walkable_near(mid(3, 2, 150.0, 40.0))]),
        at("colosseum", vec![walkable_near(mid(5, 5, 0.0, 70.0))]),
        at("behind-building", vec![behind(3, 2, &stone), walkable_near(mid(3, 2, -60.0, 60.0))]),
        // On a bridge over the Kingsriver, the Highlands into the Tilt-Yards.
        at("kingsriver-bridge", vec![[2248.0, 760.0]]),
        // A biome's border: Castle Town into the Tilt-Yards (chunks 4,3 and 5,3).
        at("biome-border", vec![walkable_near([5.0 * kingdom::STAGE.0 - 4.0, 3.5 * kingdom::STAGE.1])]),
    ];
    if let Some((feet, _, _)) = kingdom::road_over_a_border() {
        out.push(at("border-road", vec![feet, [feet[0] + 40.0, feet[1]]]));
    }
    out
}

/// The catalog's wishes the lots' shots build, all beside the Village.
const BUILT: [&str; 4] = ["tavern", "wayside-shrine", "fellowship-stone", "beacon-tower"];

/// What a run stands that no chunk's set has (`world3d::lots`): the
/// catalog's wishes built beside the Village, seen from before each pair of
/// lots with a knight at the door and one behind, and a few of the run's
/// finds with a knight by each.
fn lot_places() -> Vec<Place> {
    use angel_delve::world::finds::Kind as F;
    let wishes = world3d::lots::proof_wishes(&BUILT, "village");
    let lots = angel_delve::world::wishes::lots(&wishes);
    let mut out = Vec::new();
    let px = |t: i32| t as f32 * 16.0;
    // Two lots side by side: a knight at one's door, one behind the other.
    for (name, a, b) in [("lots-tavern-shrine", 1, 0), ("lots-stone-tower", 2, 3)] {
        let (Some(a), Some(b)) = (lots[a], lots[b]) else { continue };
        let door = [px(a.tx) + px(a.tw) / 2.0 + 10.0, px(a.ty + a.th) + 6.0];
        let behind = [px(b.tx) + px(b.tw) / 2.0, px(b.ty) - 3.0];
        out.push(Place { name: name.into(), feet: vec![door, behind], wishes: wishes.clone() });
    }
    let run = kingdom::proof_run(&[[0.0, 0.0]]);
    let finds = run.standing_finds();
    for (name, kind) in [("find-stones", F::Stones), ("find-campfire", F::Campfire), ("find-cave", F::Cave), ("find-hermit", F::Hermit), ("find-ruin", F::Ruin), ("find-shrine", F::Shrine)] {
        let Some((_, find)) = finds.iter().find(|(_, f)| f.kind == kind) else { continue };
        let (x, y) = (px(find.at.0) + 8.0, px(find.at.1 + 1));
        // One before it to the side, one just behind it.
        out.push(Place { name: name.into(), feet: vec![[x + 28.0, y + 8.0], [x - 10.0, y - 15.0]], wishes: Vec::new() });
    }
    out
}

/// The shots side by side, flat then 3D, a row a place, at half their size.
fn contact(path: &Path, shots: &[(Vec<u8>, Vec<u8>)], size: (u32, u32)) -> Result<(), String> {
    let (w, h) = (size.0 / 2, size.1 / 2);
    let gap = 8;
    let (cw, ch) = (2 * w + 3 * gap, shots.len() as u32 * (h + gap) + gap);
    let mut sheet = vec![0u8; (cw * ch * 4) as usize];
    for px in sheet.chunks_exact_mut(4) {
        px.copy_from_slice(&[18, 18, 22, 255]);
    }
    for (row, (flat, three)) in shots.iter().enumerate() {
        for (col, pixels) in [flat, three].into_iter().enumerate() {
            let (ox, oy) = (gap + col as u32 * (w + gap), gap + row as u32 * (h + gap));
            for y in 0..h {
                for x in 0..w {
                    // The average of the 2 x 2 under it.
                    let mut sum = [0u32; 4];
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let i = (((2 * y + dy) * size.0 + 2 * x + dx) * 4) as usize;
                        for k in 0..4 {
                            sum[k] += u32::from(pixels[i + k]);
                        }
                    }
                    let o = (((oy + y) * cw + ox + x) * 4) as usize;
                    for k in 0..3 {
                        sheet[o + k] = (sum[k] / 4) as u8;
                    }
                    sheet[o + 3] = 255;
                }
            }
        }
    }
    write_png(path, (cw, ch), &sheet)
}

fn run() -> Result<(), String> {
    let usage = "usage: delve_kingdom <outdir> [--scale N] [--only NAME]... [--lots] [--contact] | --walk";
    let mut words = std::env::args().skip(1);
    let (mut out, mut scale, mut only, mut walk) = (None, 3u32, Vec::new(), false);
    let (mut lots, mut sheet) = (false, false);
    while let Some(word) = words.next() {
        match word.as_str() {
            "--scale" => {
                scale = words.next().and_then(|w| w.parse().ok()).filter(|&s| (1..=8).contains(&s)).ok_or("--scale takes 1 to 8")?;
            }
            "--only" => only.push(words.next().ok_or(usage)?),
            "--walk" => walk = true,
            "--lots" => lots = true,
            "--contact" => sheet = true,
            _ if out.is_none() => out = Some(PathBuf::from(word)),
            other => return Err(format!("`{other}`: {usage}")),
        }
    }
    let mut gpu = Gpu::new()?;
    // As the cockpit's window has the Delve: its frames in parts, its
    // pickups and shots as shapes.
    arena::draw_parts_outside(true);
    arena::props_meshes(true);
    if walk {
        return walk_across(&mut gpu);
    }
    let out = out.ok_or(usage)?;
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let (w, h) = arena::seat_size();
    let mut shots = Vec::new();
    let mut shot_size = (0, 0);
    let all = if lots { lot_places() } else { places() };
    for place in all.into_iter().filter(|p| only.is_empty() || only.contains(&p.name)) {
        let run = kingdom::proof_run_built(&place.feet, place.wishes.clone());
        let mut pair = (Vec::new(), Vec::new());
        for on in [false, true] {
            world3d::set_enabled(on);
            let scene = Arc::new(arena::seat_scene(&run, w as i32, h as i32, Some(1)));
            if on && scene.chunks.is_empty() {
                return Err(format!("{}: the kingdom has no chunks in 3D", place.name));
            }
            let mut maker = StageMaker::default();
            let mut frame = maker.make(&scene, |_| None);
            gpu.upload(std::mem::take(&mut frame.pictures));
            frame = maker.make(&scene, |_| None);
            let size = (frame.size.0 * scale, frame.size.1 * scale);
            if let Ok(what) = std::env::var("KINGDOM_DEBUG")
                && let Some(world) = frame.props.as_mut().and_then(|p| p.world.as_mut())
            {
                match what.as_str() {
                    "floor" => world.blocks.clear(),
                    "blocks" => world.floor.clear(),
                    "nozones" => {
                        if let Some(l) = world.look.as_mut() {
                            l.zones.clear();
                        }
                    }
                    "onlyzone" => {
                        // Only the last (nearest) zone, all else the base.
                        if let Some(l) = world.look.as_mut() {
                            let last = l.zones.pop();
                            l.zones = last.into_iter().collect();
                        }
                    }
                    "unlit" => {
                        for n in world.floor.iter_mut().chain(&mut world.blocks) {
                            n.material.lit = false;
                        }
                    }
                    _ => {}
                }
                eprintln!("debug {what}: floor {} blocks {}", world.floor.len(), world.blocks.len());
                eprintln!("  reveal {:?}", world.look.as_ref().and_then(|l| l.reveal));
                eprintln!("  zones {:?}", world.look.as_ref().map(|l| l.zones.iter().map(|z| (z.at, z.half, z.radius, z.feather, z.ramp[4])).collect::<Vec<_>>()));
                eprintln!("  figures {:?}", world.figures.iter().map(|n| (n.transform.translation, n.transform.scale)).collect::<Vec<_>>());
                eprintln!("  revealed copies {}", world.extra.iter().filter(|n| n.id >= 0x7f00_0000_0000).count());
            }
            let pixels = gpu.stage(frame, size, true);
            let name = if on { format!("{}.png", place.name) } else { format!("{}-flat.png", place.name) };
            write_png(&out.join(&name), size, &pixels)?;
            let lamps: usize = scene.chunks.iter().map(|c| c.set.lamps.len()).sum();
            println!(
                "{name}: knights at {:?}; {} chunks near, {lamps} lamps ({})",
                place.feet,
                scene.chunks.len(),
                scene.chunks.iter().map(|c| c.id()).collect::<Vec<_>>().join(" ")
            );
            if let Some(stood) = &scene.lots {
                let cut: Vec<String> = scene.chunks.iter().filter(|c| c.key != c.plate_key).map(|c| c.id()).collect();
                let extras: usize = scene.chunks.iter().filter_map(|c| stood.share(c.index)).map(|s| s.extras.len()).sum();
                println!("  lots: {extras} extra blocks in the chunks near; cut: {}", if cut.is_empty() { "none".into() } else { cut.join(" ") });
            }
            shot_size = size;
            if on { pair.1 = pixels } else { pair.0 = pixels }
        }
        shots.push(pair);
    }
    world3d::set_enabled(false);
    if sheet && !shots.is_empty() {
        contact(&out.join("contact.png"), &shots, shot_size)?;
        println!("wrote {}", out.join("contact.png").display());
    }
    println!("wrote the kingdom's proof shots to {}", out.display());
    Ok(())
}

/// A knight walking east along the kingdom's middle across three chunk
/// borders (from chunk 2's middle to chunk 5's), two pixels a frame, its
/// camera on it: each frame made (the scene and the stage, the CPU's part)
/// and drawn by the GPU (waited for), flat and in 3D.
fn walk_across(gpu: &mut Gpu) -> Result<(), String> {
    let (w, h) = arena::seat_size();
    let size = (w * 3, h * 3);
    let row = 4;
    for on in [false, true] {
        world3d::set_enabled(on);
        let (from, to) = (mid(2, row, 0.0, 0.0), mid(5, row, 0.0, 0.0));
        let mut run = kingdom::proof_run(&[from]);
        let mut maker = StageMaker::default();
        let (mut cpu, mut all) = (Vec::new(), Vec::new());
        let mut x = from[0];
        while x <= to[0] {
            let hero = run.players.values_mut().next().ok_or("a knight")?;
            hero.x = x / arena::UNIT;
            hero.camera = Some((hero.x, hero.y - 1.0));
            run.tick += 1;
            let t = Instant::now();
            let scene = Arc::new(arena::seat_scene(&run, w as i32, h as i32, Some(1)));
            let frame = maker.make(&scene, |_| None);
            let made = t.elapsed().as_secs_f64() * 1e3;
            let _ = gpu.stage(frame, size, false);
            cpu.push(made);
            all.push(t.elapsed().as_secs_f64() * 1e3);
            x += 2.0;
        }
        let stats = |v: &mut Vec<f64>| {
            v.sort_by(f64::total_cmp);
            let mean = v.iter().sum::<f64>() / v.len().max(1) as f64;
            (mean, v[v.len() * 95 / 100], v[v.len() - 1])
        };
        let (cm, c95, cmax) = stats(&mut cpu);
        let (am, a95, amax) = stats(&mut all);
        println!(
            "{}: {} frames across chunks 2-5 of row {row}; made {cm:.2} ms mean, {c95:.2} p95, {cmax:.2} most; made and drawn {am:.2} ms mean, {a95:.2} p95, {amax:.2} most",
            if on { "3D" } else { "flat" },
            cpu.len(),
        );
    }
    world3d::set_enabled(false);
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
            label: Some("delve-kingdom"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;
        device.on_uncaptured_error(Arc::new(|error: wgpu::Error| eprintln!("GPU error: {error}")));
        let core = Core::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
        Ok(Self { core, device, target: None })
    }

    fn upload(&mut self, pictures: Vec<angel_stage::Picture>) {
        for p in pictures {
            self.core.upload(Picture { id: p.id, width: p.width, height: p.height, rgba: p.rgba, cols: 0, rows: 0 });
        }
    }

    /// A Delve frame as a stage, drawn into a texture `size` pixels as the
    /// window draws a stage in its cells, on black, and (when `read`) read
    /// back; else waited for.
    fn stage(&mut self, mut frame: angel_stage::Frame, size: (u32, u32), read: bool) -> Vec<u8> {
        self.upload(std::mem::take(&mut frame.pictures));
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
                label: Some("delve-kingdom-target"),
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
        self.core.draw(&view, size, wgpu::Color::BLACK, &drawn, "delve-kingdom");
        if read {
            return self.core.read_back(texture, size);
        }
        let _ = self.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(5)) });
        Vec::new()
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
