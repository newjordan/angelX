//! Lane W3's offline proof: a mirror wish changes the whole knight, in the real game renderer, no window.
//!
//! A party of two in the Crypt's first fight room, shot twice with the game's own machinery (`StageMaker`, its HD drawer, the GPU core
//! offscreen): BEFORE the look lands, and AFTER it has landed in the folder a window watches, found by `LooksWatch` with no restart.
//!
//!   cargo run --profile fast -p angel-glass --example wish_look_proof -- --looks LOOKS --out OUT [--scale 4]
//!
//! LOOKS is a finished looks folder as the host composes it (`.angel/dungeon/looks`: `pack.json` and one folder a card). The example
//! watches `OUT/watched` (empty at first), shoots, copies LOOKS into it (what the host does when a job lands), refreshes the watch (it
//! must install a pack), tells the run (`Run::set_mirror_body`, what the host's `MirrorBody` command does) and shoots again.
//!
//! Seat 2 ("Goose Inspector") is the knight that asked; seat 1 is the other. Writes OUT/<phase>/<state>-<facing>/NN.png (a crop round
//! the two knights), OUT/rooms/<phase>-<facing>.png (the whole room, the idle frame) and OUT/log.tsv (what the game named, and what
//! drew it). States are driven through the game's own Hero fields, as `import_proof` does: idle, walk, run, slash, hurt, fallen.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use angel_delve::render::overworld::arena::{self, NATIVE_H, NATIVE_W, UNIT, hd_pack, stage_frame::StageMaker};
use angel_delve::rules::cosmetic_wish::{Look, key};
use angel_delve::rules::{Pack, RoomKind, Rng, Run, layout, looks};
use angel_gpu::{Core, Draw, Frame, Instance, Picture, Stage};

const STAGE: u32 = 0x00FA_57A4;
const APART: f32 = 34.0;
const WORDS: &str = "a frog knight with a lily-pad shield and a reed spear";
const ASKER: u32 = 2;

const SLASH: [u32; 6] = [5, 6, 5, 12, 11, 6];
const DIE: [u32; 8] = [6, 9, 6, 6, 6, 6, 6, 30];
const FALLEN_AGES: [u32; 16] = [0, 6, 15, 21, 27, 33, 39, 45, 75, 78, 81, 85, 90, 100, 115, 125];
const WALK_TICKS: u32 = 48;
const RUN_TICKS: u32 = 24;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("wish_look_proof: {why}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum State {
    Idle(u32),
    Walk(u32),
    Run(u32),
    Slash(u32),
    Hurt(u32),
    Fallen(u32),
}

impl State {
    fn name(self) -> &'static str {
        match self {
            State::Idle(_) => "idle",
            State::Walk(_) => "walk",
            State::Run(_) => "run",
            State::Slash(_) => "slash",
            State::Hurt(_) => "hurt",
            State::Fallen(_) => "fallen",
        }
    }
}

/// The Crypt's first fight room with two seats side by side, no foes.
fn room(aim: (f32, f32)) -> Option<Run> {
    let mut found = None;
    for seed in 3..11u64 {
        let floor = layout::floor_party(1, Pack::Crypt, 2, &mut Rng::new(seed));
        let Some(at) = floor.rooms.iter().position(|r| r.kind == RoomKind::Fight) else { continue };
        let mut run = Run::new(seed, seed, Some("Friend"));
        run.dungeon = floor;
        run.enter_for_test(at);
        found = Some(run);
        break;
    }
    let mut run = found?;
    run.enemies.clear();
    run.join_seat(ASKER, "Goose Inspector");
    let room = run.room().clone();
    let (w, h) = (room.cols as f32 * 16.0, room.rows as f32 * 16.0);
    let first = (w / 2.0 - APART * 0.5 + 76.0, h / 2.0 + 8.0);
    for (k, hero) in run.players.values_mut().enumerate() {
        (hero.x, hero.y) = ((first.0 + k as f32 * APART) / UNIT, first.1 / UNIT);
        (hero.aim_x, hero.aim_y) = aim;
        hero.camera = Some(((w / 2.0) / UNIT, (h / 2.0) / UNIT));
        hero.hp = hero.max_hp;
        (hero.fire_cooldown, hero.swing, hero.struck, hero.stride) = (0, 0, 0, 0.0);
    }
    Some(run)
}

fn start_of(ticks: &[u32], k: usize) -> u32 {
    ticks[..k].iter().sum()
}

fn countdown_for(ticks: &[u32], duration: u32, k: usize) -> u32 {
    let clip: u32 = ticks.iter().sum();
    for elapsed in 0..duration {
        let at = elapsed * clip / duration;
        let mut acc = 0;
        for (i, t) in ticks.iter().enumerate() {
            if at < acc + t {
                if i == k {
                    return duration - elapsed;
                }
                break;
            }
            acc += t;
        }
    }
    1
}

fn apply(run: &mut Run, state: State) {
    // Walk, run, stroke and idle for both seats, so the gaits can be compared; hurt and fall for the asker alone.
    let everyone = matches!(state, State::Walk(_) | State::Run(_) | State::Slash(_) | State::Idle(_));
    for (&id, hero) in run.players.iter_mut() {
        let mine = id == ASKER;
        if !(mine || everyone) {
            continue;
        }
        match state {
            State::Idle(_) => {}
            State::Walk(k) => hero.stride = ((start_of(&[6; 8], k as usize) + 3) as f32 / WALK_TICKS as f32) * 3.4,
            State::Run(k) => {
                hero.flee_ticks = 5;
                hero.stride = ((start_of(&[3; 8], k as usize) + 1) as f32 / RUN_TICKS as f32) * 3.4 + 0.002;
            }
            State::Slash(k) => {
                hero.sword = true;
                hero.swing = countdown_for(&SLASH, 8, k as usize);
            }
            State::Hurt(s) => {
                hero.struck = s;
                hero.struck_dir = (-hero.aim_x.round() as i8, -hero.aim_y.round() as i8);
            }
            State::Fallen(age) => {
                hero.hp = 0;
                hero.fell = 1000 - u64::from(age);
            }
        }
    }
    if let State::Fallen(_) = state {
        run.tick = 1000;
    }
    if let State::Idle(k) = state {
        run.tick = u64::from(k) * 6;
    }
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let usage = "usage: wish_look_proof --looks LOOKS --out OUT [--scale N]";
    let mut words = std::env::args().skip(1);
    let (mut looks_dir, mut out, mut scale) = (None, None, 4u32);
    while let Some(word) = words.next() {
        match word.as_str() {
            "--looks" => looks_dir = Some(PathBuf::from(words.next().ok_or(usage)?)),
            "--out" => out = Some(PathBuf::from(words.next().ok_or(usage)?)),
            "--scale" => scale = words.next().and_then(|w| w.parse().ok()).filter(|&s| (1..=8).contains(&s)).ok_or("--scale takes 1 to 8")?,
            other => return Err(format!("`{other}`: {usage}")),
        }
    }
    let (looks_src, out) = (looks_dir.ok_or(usage)?, out.ok_or(usage)?);
    let card = std::fs::read_dir(&looks_src)
        .map_err(|e| format!("{}: {e}", looks_src.display()))?
        .flatten()
        .find(|e| e.file_type().is_ok_and(|t| t.is_dir()) && e.file_name().to_string_lossy().starts_with("mirror-look-"))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .ok_or("no mirror-look-<hash> folder in the looks folder")?;
    if card != key("Goose Inspector") {
        return Err(format!("the looks folder is for {card}, not Goose Inspector ({})", key("Goose Inspector")));
    }
    let _ = std::fs::remove_dir_all(&out);
    let watched = out.join("watched");
    std::fs::create_dir_all(&watched).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(out.join("rooms")).map_err(|e| e.to_string())?;

    // The window as the cart sets it up: no pack of its own, a folder it watches.
    hd_pack::install(None);
    let mut watch = hd_pack::LooksWatch::new(None);
    watch.set_dir(watched.clone());
    let mut gpu = Gpu::new()?;
    arena::draw_parts_outside(true);
    angel_delve::world3d::set_enabled(false);
    arena::props_meshes(false);
    let size = (NATIVE_W as u32 * scale, NATIVE_H as u32 * scale);
    let mut log = String::from("phase\tstate\tfacing\tframe\tseat1_key\tseat1_drawn\tseat2_key\tseat2_drawn\n");
    let mut failures: Vec<String> = Vec::new();
    let mut now = Instant::now();

    for phase in ["before", "after"] {
        if phase == "after" {
            // The job lands: the host composes `pack.json` in the folder the window watches. The watch finds it by itself.
            copy_dir(&looks_src, &watched).map_err(|e| e.to_string())?;
            now += Duration::from_secs(5);
            if !watch.refresh(now) {
                return Err("the watch did not pick up the composed looks".into());
            }
            let installed = hd_pack::installed().ok_or("nothing installed after the looks landed")?;
            println!("looks landed: {} named drawings, loader warnings: {:?}", installed.named_len(), installed.warnings());
            if !installed.warnings().is_empty() {
                return Err("the loader left out entries".into());
            }
        }
        for (facing, aim) in [("r", (1.0, 0.0)), ("l", (-1.0, 0.0)), ("d", (0.0, 1.0)), ("u", (0.0, -1.0))] {
            let sides = matches!(facing, "r" | "l");
            let mut states: Vec<State> = (0..6).map(State::Idle).collect();
            states.extend((0..8).map(State::Walk));
            states.extend((0..6).map(State::Slash));
            if sides {
                states.extend((0..8).map(State::Run));
                states.extend((1..=16).rev().map(State::Hurt));
                states.extend(FALLEN_AGES.map(State::Fallen));
            }
            let mut crop: Option<[f32; 4]> = None;
            for state in states {
                let mut run = room(aim).ok_or("no fight room on the Crypt's first floor")?;
                if phase == "after" {
                    // What the host's `MirrorBody` does once the set is in the looks folder.
                    let name = run.players[&ASKER].name.clone();
                    run.home.wardrobe.mirror_looks.insert(name, Look { words: WORDS.into(), worn: looks::Sprite::default(), body: false });
                    if !run.set_mirror_body(&card, WORDS, true) {
                        return Err("the run did not take the body".into());
                    }
                }
                apply(&mut run, state);
                let scene = arena::in_seat(|| arena::scene_for(&run, NATIVE_W, NATIVE_H, Some(1)));
                let named = |n: u32| scene.figures.iter().flat_map(|f| &f.bodies).find_map(|b| b.named.as_ref().filter(|x| x.key.contains(&format!("/seat{n}/"))).map(|x| x.key.to_string())).unwrap_or_default();
                let (key1, key2) = (named(1), named(2));
                let scene = Arc::new(scene);
                let mut art = hd_pack::HdArt::default();
                let mut drawn = Vec::new();
                let mut by: [Option<(u32, u32)>; 2] = [None, None];
                let mut frame = StageMaker::default().make(&scene, |body| {
                    let before = drawn.len();
                    let placed = art.picture(body, &mut drawn);
                    if let Some(n) = body.named.as_ref() {
                        let seat = if n.key.contains("/seat2/") { 1 } else { 0 };
                        by[seat] = placed.map(|_| drawn.get(before).map_or((0, 0), |p| (p.width, p.height)));
                        if placed.is_some() && by[seat] == Some((0, 0)) {
                            by[seat] = Some((1, 1));
                        }
                    }
                    placed
                });
                frame.pictures.extend(drawn);
                let pixels = gpu.stage(frame, size);
                // A crop fixed per facing and phase, from where the two knights are in the room.
                let (cx, cy) = scene.camera;
                let bodies: Vec<_> = scene.figures.iter().flat_map(|f| &f.bodies).filter(|b| b.named.is_some()).collect();
                let room_box = *crop.get_or_insert_with(|| {
                    let x0 = bodies.iter().map(|b| b.x as f32).fold(f32::INFINITY, f32::min) - 30.0;
                    let x1 = bodies.iter().map(|b| (b.x + b.img.w) as f32).fold(f32::NEG_INFINITY, f32::max) + 30.0;
                    let y1 = bodies.iter().map(|b| (b.y + b.img.h) as f32).fold(f32::NEG_INFINITY, f32::max) + 10.0;
                    [x0, y1 - 76.0, x1, y1]
                });
                let crop_box = [room_box[0] - cx, room_box[1] - cy, room_box[2] - cx, room_box[3] - cy].map(|v| (v * scale as f32).round().max(0.0) as u32);
                let crop_box = [crop_box[0].min(size.0), crop_box[1].min(size.1), crop_box[2].min(size.0), crop_box[3].min(size.1)];
                let (cw, ch) = (crop_box[2] - crop_box[0], crop_box[3] - crop_box[1]);
                let mut cropped = Vec::with_capacity((cw * ch * 4) as usize);
                for y in crop_box[1]..crop_box[3] {
                    let at = ((y * size.0 + crop_box[0]) * 4) as usize;
                    cropped.extend_from_slice(&pixels[at..at + (cw * 4) as usize]);
                }
                let frame_no = match state {
                    State::Idle(k) | State::Walk(k) | State::Run(k) | State::Slash(k) | State::Hurt(k) | State::Fallen(k) => k,
                };
                let folder = out.join(phase).join(format!("{}-{facing}", state.name()));
                std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
                let index = std::fs::read_dir(&folder).map(|d| d.count()).unwrap_or(0);
                write_png(&folder.join(format!("{index:02}.png")), (cw, ch), &cropped)?;
                let fmt = |d: Option<(u32, u32)>| d.map(|(w, h)| format!("{w}x{h}")).unwrap_or_default();
                let _ = writeln!(log, "{phase}\t{}\t{facing}\t{frame_no}\t{key1}\t{}\t{key2}\t{}", state.name(), fmt(by[0]), fmt(by[1]));
                if matches!(state, State::Idle(0)) {
                    write_png(&out.join("rooms").join(format!("{phase}-{facing}.png")), size, &pixels)?;
                }
                // The proof's claims, checked on every frame.
                let mut state_ok = true;
                match (phase, by[1].is_some(), by[0].is_some()) {
                    ("before", true, _) | ("before", _, true) => state_ok = false,
                    ("after", false, _) => {
                        // The rig's die clip is 75 ticks; once it is over the game draws bones, which have no name.
                        let bones = matches!(state, State::Fallen(age) if age >= DIE.iter().sum::<u32>());
                        state_ok = bones && key2.is_empty();
                    }
                    ("after", _, true) => state_ok = false,
                    _ => {}
                }
                if !state_ok {
                    failures.push(format!("{phase} {:?} {facing}: seat 2 drawn {:?} (`{key2}`), seat 1 drawn {:?} (`{key1}`)", state, by[1], by[0]));
                }
            }
        }
    }
    std::fs::write(out.join("log.tsv"), log).map_err(|e| e.to_string())?;
    if !failures.is_empty() {
        for f in failures.iter().take(40) {
            eprintln!("FAIL {f}");
        }
        return Err(format!("{} frames did not come out as the proof says", failures.len()));
    }
    println!("wrote {}", out.display());
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
            label: Some("wish-look-proof"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;
        device.on_uncaptured_error(Arc::new(|error: wgpu::Error| eprintln!("GPU error: {error}")));
        let core = Core::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
        Ok(Self { core, device })
    }

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
            label: Some("wish-look-proof-target"),
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
        self.core.draw(&view, size, wgpu::Color::BLACK, &drawn, "wish-look-proof");
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
