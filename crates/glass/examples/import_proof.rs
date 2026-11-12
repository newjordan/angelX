//! Lanes I1 and I2's offline proof: an imported knight in the real game renderer, through the HD art pack, at rung 64.
//!
//! The same machinery `delve_ladder` shoots with (the Delve's `StageMaker` and its HD drawer, the GPU core offscreen, no window and no
//! surface): the Crypt's first fight room with four seats in a row, seat 1 the knight the pack draws (its design's key), seats 2 to 4 the
//! game's own 16 px knights; and Cinderjaw's room with the same four beside the boss.
//!
//! cargo run --profile fast -p angel-glass --example import_proof -- --pack PACK --out OUT [--scale 4] [--rung 64]
//!
//! Writes OUT/<room>/<state>-<r|l|u|d>/NN.png (a crop of the room round the four knights, `scale` window pixels to a room pixel; the knight's
//! own pixels are 1:1 at scale 4 on rung 64), OUT/log.tsv (what the game named, the key the pack drew it by, and what it drew), OUT/rooms/*.png.
//!
//! States are driven through the game's own Hero fields, so the game itself picks the clip and frame; the log says which key it asked for:
//!   idle  run.tick           walk  stride           run  stride + flee_ticks       slash  swing countdown with the sword
//!   hurt  struck, as played (the game names the rig's hurt frame; the flinch's step back is the game's)
//!   die   hp 0 and fell `age` ticks ago: the rig's die clip through the pack for its 75 ticks, then the game's bones as for any fallen knight
//! Facings: r and l aim along x, u and d aim along y (the knight faces up or down the room: the pack's `u`/`d` entries, else its side's).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_delve::render::overworld::arena::{self, NATIVE_H, NATIVE_W, UNIT, hd_pack, stage_frame::StageMaker, testing};
use angel_delve::rules::resolution::{self, Rule, Rung};
use angel_delve::rules::{Pack, RoomKind, Rng, Run, layout};
use angel_gpu::{Core, Draw, Frame, Instance, Picture, Stage};

const STAGE: u32 = 0x00FA_57A4;
const APART: f32 = 30.0;

/// The rig's clip lengths in ticks, as `knight-great-helm.json` has them (checked by the pack's converter against the file).
const SLASH: [u32; 6] = [5, 6, 5, 12, 11, 6];
const DIE: [u32; 8] = [6, 9, 6, 6, 6, 6, 6, 30];
/// The ages (ticks since the fall) the dying knight is shot at: the start of each rig frame of the die clip, then the bones coming
/// (the clip's 75 ticks over) and settled.
const FALLEN_AGES: [u32; 16] = [0, 6, 15, 21, 27, 33, 39, 45, 75, 78, 81, 85, 90, 100, 115, 125];
/// The ages at which each of the rig's cast frames 0..4 begins (frames of 2, 2, 3, 2 and 6 ticks): the game shows the clip for 15 ticks only.
const CAST_AGES: [u32; 5] = [0, 2, 4, 7, 9];
const WALK_TICKS: u32 = 48;
const RUN_TICKS: u32 = 24;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("import_proof: {why}");
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
    /// Loosing the bow: `fire_cooldown` ticks left (the default knight's every shot).
    Bow(u32),
    /// A cast `age` ticks ago (the game shows it for its first 15 ticks: the rig's cast frames 0 to 4).
    Cast(u32),
    /// The game's flinch: `struck` countdown.
    Hurt(u32),
    /// Fell `age` ticks ago.
    Fallen(u32),
}

impl State {
    fn name(self) -> &'static str {
        match self {
            State::Idle(_) => "idle",
            State::Walk(_) => "walk",
            State::Run(_) => "run",
            State::Slash(_) => "slash",
            State::Bow(_) => "bow",
            State::Cast(_) => "cast",
            State::Hurt(_) => "hurt",
            State::Fallen(_) => "fallen",
        }
    }
}

/// A room with four seats in a row: the Crypt's first fight room (no foes), or Cinderjaw's.
fn room(boss: bool, aim: (f32, f32)) -> Option<Run> {
    let mut run = if boss {
        testing::boss("cinderjaw")
    } else {
        let mut found = None;
        for seed in 3..11u64 {
            let floor = layout::floor_party(1, Pack::Crypt, 4, &mut Rng::new(seed));
            let Some(at) = floor.rooms.iter().position(|r| r.kind == RoomKind::Fight) else { continue };
            let mut run = Run::new(seed, seed, Some("Friend"));
            run.dungeon = floor;
            run.enter_for_test(at);
            found = Some(run);
            break;
        }
        let mut run = found?;
        run.enemies.clear();
        run
    };
    for (id, name) in [(2, "Goose Inspector"), (3, "Marrow"), (4, "Tansy")] {
        run.join_seat(id, name);
    }
    let room = run.room().clone();
    let (w, h) = (room.cols as f32 * 16.0, room.rows as f32 * 16.0);
    // The crypt: the row at the middle. Cinderjaw's room: the row on the left of the boss.
    let first = if boss { (w / 2.0 - 150.0, h / 2.0 + 40.0) } else { (w / 2.0 - APART * 1.5, h / 2.0 + 8.0) };
    for (k, hero) in run.players.values_mut().enumerate() {
        (hero.x, hero.y) = ((first.0 + k as f32 * APART) / UNIT, first.1 / UNIT);
        (hero.aim_x, hero.aim_y) = aim;
        hero.camera = Some(((w / 2.0) / UNIT, (h / 2.0) / UNIT));
        hero.hp = hero.max_hp;
        (hero.fire_cooldown, hero.swing, hero.struck, hero.stride) = (0, 0, 0, 0.0);
    }
    Some(run)
}

/// The tick inside a clip at which rig frame `k` is on, given the frame lengths.
fn start_of(ticks: &[u32], k: usize) -> u32 {
    ticks[..k].iter().sum()
}

/// A countdown `n` ticks from the end of `total` that lands on rig frame `k` of a one-shot of `ticks`,
/// scaled into `duration` game ticks as `Rig::action` does: elapsed * clip / duration.
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
    // no integer step lands on it: the nearest after
    1
}

fn apply(run: &mut Run, state: State) {
    // Seat 1 is the pack's knight; seats 2..4 do the same on the walk and the stroke, so the gaits can be compared, and stand otherwise.
    let everyone = matches!(state, State::Walk(_) | State::Run(_) | State::Slash(_) | State::Idle(_) | State::Bow(_));
    for (&id, hero) in run.players.iter_mut() {
        let mine = id == 1;
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
            State::Bow(c) => hero.fire_cooldown = c,
            State::Cast(_) => {}
            State::Hurt(s) => {
                hero.struck = s;
                // Thrown back from a blow that came from the front.
                hero.struck_dir = (-hero.aim_x.round() as i8, -hero.aim_y.round() as i8);
            }
            State::Fallen(age) => {
                if mine {
                    hero.hp = 0;
                    hero.fell = 1000 - u64::from(age);
                }
            }
        }
    }
    if let State::Fallen(_) = state {
        run.tick = 1000;
    }
    if let State::Cast(age) = state {
        // The knight just called: `minions::casting` holds for CAST_SHOWN ticks and the figure samples the rig's cast clip at its age.
        run.tick = 1000;
        run.minions.casts.insert(1, angel_delve::rules::minions::Cast { tick: 1000 - u64::from(age), rite: angel_delve::rules::minions::Rite::Raise, x: 0.0, y: 0.0, torn: Vec::new() });
    }
    if let State::Idle(k) = state {
        run.tick = u64::from(k) * 6;
    }
}

fn run() -> Result<(), String> {
    let usage = "usage: import_proof --pack PACK --out OUT [--scale N] [--rung 16|32|64|128]";
    let mut words = std::env::args().skip(1);
    let (mut pack, mut out, mut scale, mut rung) = (None, None, 4u32, 64u32);
    while let Some(word) = words.next() {
        match word.as_str() {
            "--pack" => pack = Some(PathBuf::from(words.next().ok_or(usage)?)),
            "--out" => out = Some(PathBuf::from(words.next().ok_or(usage)?)),
            "--scale" => scale = words.next().and_then(|w| w.parse().ok()).filter(|&s| (1..=8).contains(&s)).ok_or("--scale takes 1 to 8")?,
            "--rung" => rung = words.next().and_then(|w| w.parse().ok()).ok_or("--rung takes 16, 32, 64 or 128")?,
            other => return Err(format!("`{other}`: {usage}")),
        }
    }
    let (dir, out) = (pack.ok_or(usage)?, out.ok_or(usage)?);
    let rung = Rung::of(rung).ok_or("--rung takes 16, 32, 64 or 128")?;
    let opened = hd_pack::Pack::open(&dir).ok_or_else(|| format!("{}: no HD art pack", dir.display()))?;
    // The loader's own verdict: anything it left out is an error here.
    if !opened.warnings().is_empty() {
        for w in opened.warnings().iter().take(10) {
            eprintln!("hd pack: left out {w}");
        }
        return Err(format!("the loader left out {} entries", opened.warnings().len()));
    }
    println!("hd pack {}: {} named drawings of {} keys, loader warnings: none", dir.display(), opened.named_len(), opened.keys().count());
    let pack = Arc::new(opened);
    hd_pack::install(Some(pack.clone()));
    for sub in ["crypt", "boss", "rooms"] {
        let _ = std::fs::remove_dir_all(out.join(sub));
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    std::fs::create_dir_all(out.join("rooms")).map_err(|e| e.to_string())?;
    let mut gpu = Gpu::new()?;
    arena::draw_parts_outside(true);
    angel_delve::world3d::set_enabled(false);
    arena::props_meshes(false);
    resolution::set_rule(Some(Rule::Fixed(rung)));
    let size = (NATIVE_W as u32 * scale, NATIVE_H as u32 * scale);
    let mut log = String::from("room\tstate\tfacing\tframe\tseat1_key\tdress\tdrawn_by_name\tdrawing\tsent_px\n");
    let mut failures: Vec<String> = Vec::new();

    for boss in [false, true] {
        let room_name = if boss { "boss" } else { "crypt" };
        for (facing, aim) in [("r", (1.0, 0.0)), ("l", (-1.0, 0.0)), ("u", (0.0, -1.0)), ("d", (0.0, 1.0))] {
            let sides = matches!(facing, "r" | "l");
            let states: Vec<State> = if boss {
                let mut v = vec![State::Idle(0), State::Walk(1), State::Slash(1)];
                if sides {
                    v.extend([State::Hurt(10), State::Fallen(45), State::Fallen(100)]);
                }
                v
            } else {
                let mut v: Vec<State> = (0..6).map(State::Idle).collect();
                v.extend((0..8).map(State::Walk));
                if sides {
                    v.extend((0..8).map(State::Run));
                }
                v.extend((0..6).map(State::Slash));
                if sides {
                    v.extend([10, 8, 7, 5, 3, 2, 1].map(State::Bow));
                    v.extend(CAST_AGES.map(State::Cast));
                }
                v.extend((1..=16).rev().map(State::Hurt));
                v.extend(FALLEN_AGES.map(State::Fallen));
                v
            };
            let mut crop: Option<[f32; 4]> = None;
            for state in states {
                let mut run = room(boss, aim).ok_or("no fight room on the Crypt's first floor")?;
                apply(&mut run, state);
                let scene = arena::in_seat(|| arena::scene_for(&run, NATIVE_W, NATIVE_H, Some(1)));
                // What the game asked for, for seat 1.
                let seat1 = scene.figures.iter().flat_map(|f| &f.bodies).find_map(|b| b.named.as_ref().filter(|n| n.key.contains("/seat1/")).map(|n| (n.key.to_string(), n.dress.to_string())));
                let (key, dress) = seat1.clone().unwrap_or_default();
                // The game must have asked for the frame the state says (the walk, the stroke, the idle, the hurt, the fall).
                let want: Option<String> = match state {
                    State::Idle(k) => Some(format!("idle/{k}")),
                    State::Walk(k) => Some(format!("walk/{k}")),
                    State::Run(k) => Some(format!("run/{k}")),
                    State::Slash(k) => Some(format!("slash/{k}")),
                    State::Bow(_) => Some("bow".to_string()),
                    State::Cast(age) => Some(format!("cast/{}", CAST_AGES.iter().rposition(|&a| a <= age).unwrap_or(0))),
                    State::Hurt(_) => Some("hurt".to_string()),
                    // The clip's frame at that age; once it is over the game draws bones, which have no name.
                    State::Fallen(age) => (age < DIE.iter().sum::<u32>()).then(|| {
                        let (mut acc, mut frame) = (0, 0);
                        for (i, t) in DIE.iter().enumerate() {
                            if age >= acc {
                                frame = i;
                            }
                            acc += t;
                        }
                        format!("die/{frame}")
                    }),
                };
                match (&want, key.is_empty()) {
                    (Some(w), _) if !key.contains(&format!("/{w}/")) && !(w == "bow" && key.contains("/bow/")) && !(w == "hurt" && key.contains("/hurt/")) => failures.push(format!("{room_name} {:?} {facing}: wanted {w}, the game asked for `{key}`", state)),
                    (None, false) if matches!(state, State::Fallen(_)) => {
                        failures.push(format!("{room_name} {:?} {facing}: the game named `{key}` where the bones were expected", state))
                    }
                    _ => {}
                }
                let scene = Arc::new(scene);
                let named: Vec<_> = scene.figures.iter().flat_map(|f| &f.bodies).filter_map(|b| Some((b, b.named.as_ref()?))).collect();
                let mut art = hd_pack::HdArt::default();
                let mut drawn = Vec::new();
                let mut by_name: Option<(bool, String)> = None;
                let mut frame = StageMaker::default().make(&scene, |body| {
                    let placed = art.picture_from(&pack, body, &mut drawn);
                    if let Some(n) = body.named.as_ref().filter(|n| n.key.contains("/seat1/")) {
                        let drawn_by = pack.key_for(n, n.rung.scale());
                        let hit = pack.named_for(&drawn_by, &n.dress, n.rung.scale(), n.bare.is_some());
                        by_name = Some((placed.is_some() && hit.is_some(), hit.map(|d| format!("{drawn_by} {}x{}@{}", d.drawing.width, d.drawing.height, d.scale)).unwrap_or_default()));
                    }
                    placed
                });
                let sent: Vec<String> = drawn.iter().map(|p| format!("{}x{}", p.width, p.height)).collect();
                frame.pictures.extend(drawn);
                let pixels = gpu.stage(frame, size);
                // A crop fixed per room and facing, in the room's pixels: the four knights, head room over them. The camera follows the party
                // (the living ones once seat 1 has fallen), so the box is cut from where the room is on the screen in this frame.
                let (cx, cy) = scene.camera;
                // Cinderjaw's room is shown whole (the boss beside the four); the Crypt's as a crop.
                let room_box = *crop.get_or_insert_with(|| {
                    if boss {
                        return [cx, cy, cx + NATIVE_W as f32, cy + NATIVE_H as f32];
                    }
                    let x0 = named.iter().map(|(b, _)| b.x as f32).fold(f32::INFINITY, f32::min) - 22.0;
                    let x1 = named.iter().map(|(b, _)| (b.x + b.img.w) as f32).fold(f32::NEG_INFINITY, f32::max) + 22.0;
                    let y1 = named.iter().map(|(b, _)| (b.y + b.img.h) as f32).fold(f32::NEG_INFINITY, f32::max) + 8.0;
                    [x0, y1 - 62.0, x1, y1]
                });
                let crop_box = if boss {
                    [0.0, 0.0, NATIVE_W as f32, NATIVE_H as f32]
                } else {
                    [room_box[0] - cx, room_box[1] - cy, room_box[2] - cx, room_box[3] - cy]
                }
                .map(|v| (v * scale as f32).round().max(0.0) as u32);
                let crop_box = [crop_box[0].min(size.0), crop_box[1].min(size.1), crop_box[2].min(size.0), crop_box[3].min(size.1)];
                let (cw, ch) = (crop_box[2] - crop_box[0], crop_box[3] - crop_box[1]);
                let mut cropped = Vec::with_capacity((cw * ch * 4) as usize);
                for y in crop_box[1]..crop_box[3] {
                    let at = ((y * size.0 + crop_box[0]) * 4) as usize;
                    cropped.extend_from_slice(&pixels[at..at + (cw * 4) as usize]);
                }
                let frame_no = match state {
                    State::Idle(k) | State::Walk(k) | State::Run(k) | State::Slash(k) => k,
                    State::Bow(c) | State::Cast(c) => c,
                    State::Hurt(s) => s,
                    State::Fallen(age) => age,
                };
                let folder = out.join(room_name).join(format!("{}-{facing}", state.name()));
                std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
                let index = std::fs::read_dir(&folder).map(|d| d.count()).unwrap_or(0);
                write_png(&folder.join(format!("{index:02}.png")), (cw, ch), &cropped)?;
                let (hit, drawing) = by_name.clone().unwrap_or((false, String::new()));
                let _ = writeln!(log, "{room_name}\t{}\t{facing}\t{frame_no}\t{key}\t{dress}\t{hit}\t{drawing}\t{}", state.name(), sent.join(" "));
                // The whole room, once a facing: the idle frame.
                if matches!(state, State::Idle(0)) {
                    write_png(&out.join("rooms").join(format!("{room_name}-{facing}.png")), size, &pixels)?;
                }
                // The A6 must be drawn by name wherever the pack has the frame.
                if let (Some(_), false) = (&want, hit) {
                    failures.push(format!("{room_name} {:?} {facing}: `{key}` was asked for and the pack did not draw it", state));
                }
            }
        }
    }
    resolution::set_rule(None);
    std::fs::write(out.join("log.tsv"), log).map_err(|e| e.to_string())?;
    if !failures.is_empty() {
        for f in failures.iter().take(40) {
            eprintln!("FAIL {f}");
        }
        return Err(format!("{} frames did not come out as the state says", failures.len()));
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
            label: Some("import-proof"),
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
            label: Some("import-proof-target"),
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
        self.core.draw(&view, size, wgpu::Color::BLACK, &drawn, "import-proof");
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
