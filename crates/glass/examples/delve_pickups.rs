//! The floor pickups as the window draws them (crates/delve
//! render/overworld/arena/pickup_cards.rs): every kind of pickup the book
//! has lying in rows on one fight room's floor, each a card turning about its
//! upright over a pool of light, drawn as solid shapes by the GPU core
//! offscreen, as the window draws the Delve's stage. With `--lit` the room is
//! stood up in 3D and its pickups are the older shapes (`pickup_meshes`).
//!
//! cargo run --release -p angel-glass --example delve_pickups -- <outdir> [--scale N] [--room ITEM] [--cards a,b,c] [--turn N] [--lit]
//!
//! Writes `pickups-<n>.png`, the book's cards in groups of forty laid in rows
//! on the room's floor. With `--turn N` it also writes `turn.png`, one row
//! for each of the `--cards` and in it N frames across one whole turn of the
//! card, side by side.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_delve::render::overworld::arena::{self, stage_frame::StageMaker};
use angel_delve::rules::{Tile, cards::Book};
use angel_delve::world3d::{self, room};
use angel_gpu::{Core, Draw, Frame, Instance, Picture, Stage};

/// A card's size in the room's pixels (width, height, thickness) and the
/// ticks one whole turn of it takes, as `pickup_cards` has them.
const CARD: [f32; 3] = [13.0, 18.0, 1.3];
const TURN_TICKS: f32 = 100.0;
/// How far a card floats over the floor at most, with its bob.
const LIFT_MOST: f32 = 4.5;

/// How far above the knight, in the room's units, the filmstrip's card lies,
/// and the pixels of the picture one such unit is at scale 1, up the page.
const AWAY: f32 = 8.0;
const UNIT: f32 = 5.8;

/// The stage's picture id, as the cockpit names the Delve's.
const STAGE: u32 = 0x00FA_57A4;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("delve_pickups: {why}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let usage = "usage: delve_pickups <outdir> [--scale N] [--room ITEM] [--cards a,b,c] [--turn N] [--lit]";
    let mut words = std::env::args().skip(1);
    let (mut out, mut scale, mut room_id, mut only, mut lit) = (None, 3u32, String::from("crypt-processional"), None::<String>, false);
    let mut turn = 0u32;
    while let Some(word) = words.next() {
        match word.as_str() {
            "--scale" => {
                scale = words.next().and_then(|w| w.parse().ok()).filter(|&s| (1..=8).contains(&s)).ok_or("--scale takes 1 to 8")?;
            }
            "--room" => room_id = words.next().ok_or(usage)?,
            // Frames across one whole turn of a card, in a filmstrip.
            "--turn" => {
                turn = words.next().and_then(|w| w.parse().ok()).filter(|&t| (2..=32).contains(&t)).ok_or("--turn takes 2 to 32")?;
            }
            "--cards" => only = Some(words.next().ok_or(usage)?),
            // The room stood up lit in 3D, as a page that draws scenes sees
            // it: pickups as cards with their drawings (`pickup_cards`).
            "--lit" => lit = true,
            _ if out.is_none() => out = Some(PathBuf::from(word)),
            other => return Err(format!("`{other}`: {usage}")),
        }
    }
    let out = out.ok_or(usage)?;
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut gpu = Gpu::new()?;
    arena::draw_parts_outside(true);
    // Flat, the floor's pickups are cards turning in the stage's shapes; lit,
    // the room stands in 3D with the older props.
    arena::props_meshes(lit);
    arena::pickup_cards(!lit);
    world3d::set_enabled(lit);
    let book = Book::builtin();
    let cards: Vec<String> = book
        .cards
        .iter()
        .map(|c| c.id.clone())
        .filter(|id| only.as_ref().is_none_or(|o| o.split(',').any(|w| w == id)))
        .collect();
    eprintln!("{} cards", cards.len());
    let mut proof = room::proof(&room_id)
        .ok_or_else(|| format!("no such proof room; the items are {:?}", angel_delve::world3d::assets::ids()))?;
    let (w, h) = arena::seat_size();
    for (n, group) in cards.chunks(40).chain(cards.is_empty().then_some(&[][..])).enumerate() {
        let (cx, cy) = {
            let at = proof.run.at;
            let room = &mut proof.run.dungeon.rooms[at];
            room.items.clear();
            room.chest = None;
            let (cols, rows) = (room.cols as f32 * 2.0, room.rows as f32 * 2.0);
            let (x0, y0) = (cols / 2.0 - 8.5, rows / 2.0 - 5.5);
            // A clear floor under the whole gallery.
            for row in 0..room.rows as usize {
                for col in 0..room.cols as usize {
                    let (ux, uy) = (col as f32 * 2.0 + 1.0, row as f32 * 2.0 + 1.0);
                    if ux > x0 - 4.0 && ux < x0 + 26.0 && uy > y0 - 4.0 && uy < y0 + 18.0 && room.tile(col as i32, row as i32) == Tile::Block {
                        room.set(col, row, Tile::Floor);
                    }
                }
            }
            for (k, id) in group.iter().enumerate() {
                let (col, row) = ((k % 8) as f32, (k / 8) as f32);
                room.items.push(angel_delve::rules::Item {
                    card: id.clone(),
                    x: x0 + col * 2.4,
                    y: y0 + row * 2.8,
                    held_off: None,
                    took: Vec::new(),
                });
            }
            (x0 + 8.4, y0 + 14.0)
        };
        for hero in proof.run.players.values_mut() {
            (hero.x, hero.y) = (cx + 9.0, cy);
            hero.camera = Some((cx, cy - 6.0));
        }
        proof.run.tick += 1;
        let scene = Arc::new(arena::seat_scene(&proof.run, w as i32, h as i32, Some(1)));
        let mut maker = StageMaker::default();
        let mut frame = maker.make(&scene, |_| None);
        for _ in 0..4 {
            gpu.upload(std::mem::take(&mut frame.pictures));
            frame = maker.make(&scene, |_| None);
        }
        let size = (frame.size.0 * scale, frame.size.1 * scale);
        let pixels = gpu.stage(frame, size, true);
        let name = format!("pickups-{n}.png");
        write_png(&out.join(&name), size, &pixels)?;
        println!("{name}: {}", group.join(" "));
    }
    if turn > 0 {
        filmstrip(&mut gpu, &mut proof, &cards, turn, scale, &out)?;
    }
    Ok(())
}

/// One whole turn of each card, `frames` frames of it side by side, a row of
/// them for each card: the card alone on a clear floor, at the same place in
/// every frame, the run's tick stepped so its turn comes all the way round.
fn filmstrip(gpu: &mut Gpu, proof: &mut room::Proof, cards: &[String], frames: u32, scale: u32, out: &Path) -> Result<(), String> {
    let (w, h) = arena::seat_size();
    let (px, py) = {
        let at = proof.run.at;
        let room = &mut proof.run.dungeon.rooms[at];
        room.items.clear();
        room.chest = None;
        let (cols, rows) = (room.cols as f32 * 2.0, room.rows as f32 * 2.0);
        let (x0, y0) = (cols / 2.0 - 8.5, rows / 2.0 - 5.5);
        for row in 0..room.rows as usize {
            for col in 0..room.cols as usize {
                let (ux, uy) = (col as f32 * 2.0 + 1.0, row as f32 * 2.0 + 1.0);
                if ux > x0 - 4.0 && ux < x0 + 26.0 && uy > y0 - 4.0 && uy < y0 + 18.0 && room.tile(col as i32, row as i32) == Tile::Block {
                    room.set(col, row, Tile::Floor);
                }
            }
        }
        (x0 + 8.4, y0 + 8.0)
    };
    for hero in proof.run.players.values_mut() {
        // The knight stands in the picture's middle, the card above him.
        (hero.x, hero.y) = (px, py + AWAY);
        hero.camera = Some((px, py + AWAY));
    }
    let mut rows: Vec<(Vec<u8>, (u32, u32))> = Vec::new();
    for id in cards {
        let mut strip: Vec<Vec<u8>> = Vec::new();
        let mut cell = (0, 0);
        for k in 0..frames {
            let at = proof.run.at;
            proof.run.dungeon.rooms[at].items = vec![angel_delve::rules::Item { card: id.clone(), x: px, y: py, held_off: None, took: Vec::new() }];
            // The tick at which this card, here, is `k` frames of the turn
            // round from its start.
            // (The phase is the card's own place in the turn at tick 0:
            // `pickup_cards::turn`, here for the floor point `(px, py)`.)
            let phase = (((px as i32).wrapping_mul(7).wrapping_add((py as i32).wrapping_mul(13))).rem_euclid(16)) as f32 / 16.0;
            let want = k as f32 / frames as f32;
            proof.run.tick = ((want - phase).rem_euclid(1.0) * TURN_TICKS).round() as u64 + TURN_TICKS as u64 * 4;
            let scene = Arc::new(arena::seat_scene(&proof.run, w as i32, h as i32, Some(1)));
            let mut maker = StageMaker::default();
            let mut frame = maker.make(&scene, |_| None);
            for _ in 0..4 {
                gpu.upload(std::mem::take(&mut frame.pictures));
                frame = maker.make(&scene, |_| None);
            }
            let size = (frame.size.0 * scale, frame.size.1 * scale);
            let pixels = gpu.stage(frame, size, true);
            // A window round the card: its width and height with room to
            // spare, about the middle of the picture.
            let (ww, hh) = (((CARD[0] + 14.0) * scale as f32) as u32, ((CARD[1] + LIFT_MOST + 14.0) * scale as f32) as u32);
            let (x0, y0) = ((size.0 - ww) / 2, ((size.1 - hh) / 2).saturating_sub((AWAY * UNIT * scale as f32) as u32));
            let mut crop = Vec::with_capacity((ww * hh * 4) as usize);
            for y in y0..y0 + hh {
                let a = ((y * size.0 + x0) * 4) as usize;
                crop.extend_from_slice(&pixels[a..a + (ww * 4) as usize]);
            }
            cell = (ww, hh);
            strip.push(crop);
        }
        let mut row = vec![0u8; (cell.0 * frames * cell.1 * 4) as usize];
        for (k, crop) in strip.iter().enumerate() {
            for y in 0..cell.1 as usize {
                let to = (y * (cell.0 * frames) as usize + k * cell.0 as usize) * 4;
                let from = y * cell.0 as usize * 4;
                row[to..to + cell.0 as usize * 4].copy_from_slice(&crop[from..from + cell.0 as usize * 4]);
            }
        }
        rows.push((row, (cell.0 * frames, cell.1)));
    }
    let Some(&(_, (width, height))) = rows.first() else { return Ok(()) };
    let pixels: Vec<u8> = rows.iter().flat_map(|(r, _)| r.iter().copied()).collect();
    write_png(&out.join("turn.png"), (width, height * rows.len() as u32), &pixels)?;
    println!("turn.png: {} x {frames} frames", cards.join(" "));
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
