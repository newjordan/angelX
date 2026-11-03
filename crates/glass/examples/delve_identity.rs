//! The identity start in the glass window (docs/DELVE.md, "The identity
//! start"; `rules::identity`): knights begun as shapes, made into stages by
//! the Delve (`StageMaker`) and drawn by the GPU core offscreen as the
//! window draws them: the first floor's three rooms on the Atari board
//! (the body, the weapon, the ultimate), the party made, a part wished to
//! its top tier whose painting shows in its inks' place, and the second
//! floor in the ordinary look.
//!
//! cargo run --release -p angel-glass --example delve_identity -- <outdir> [--scale N]

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_delve::render::overworld::arena::{self, paintings, stage_frame::StageMaker};
use angel_delve::render::overworld::ink;
use angel_delve::rules::{Run, home::Home, looks, phrasebook};
use angel_gpu::{Core, Draw, Frame, Instance, Picture, Stage};

const STAGE: u32 = 0x00FA_57A5;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("delve_identity: {why}");
            ExitCode::FAILURE
        }
    }
}

/// A strip of `frames` cut into ink rows `side` square, frames side by side
/// (`a|b`), as a `paint` line cuts it.
fn ink_rows(rgba: &[u8], (w, h): (u32, u32), frames: usize, side: u32) -> Result<String, String> {
    let crop = looks::crop(w, h, frames, |x, y| rgba[((y * w + x) * 4 + 3) as usize] >= 128).ok_or("nothing drawn")?;
    let inks: Vec<(char, [u8; 3])> =
        ('!'..='~').filter(|&c| c != '.' && c != '|').filter_map(|c| ink::ink(c).map(|rgb| (c, rgb))).collect();
    let nearest = |c: [u64; 3]| {
        inks.iter()
            .min_by_key(|(_, p)| (0..3).map(|k| (c[k] as i64 - i64::from(p[k])).pow(2)).sum::<i64>())
            .map_or('.', |(ch, _)| *ch)
    };
    let mut rows = vec![String::new(); side as usize];
    for f in 0..frames as i64 {
        for ty in 0..side {
            let row = &mut rows[ty as usize];
            if f > 0 {
                row.push('|');
            }
            for tx in 0..side {
                let x0 = crop.x + i64::from(tx * crop.span / side);
                let x1 = (crop.x + i64::from(((tx + 1) * crop.span).div_ceil(side))).max(x0 + 1);
                let y0 = crop.y + i64::from(ty * crop.span / side);
                let y1 = (crop.y + i64::from(((ty + 1) * crop.span).div_ceil(side))).max(y0 + 1);
                let (mut n, mut lit, mut sum) = (0u64, 0u64, [0u64; 3]);
                for sy in y0..y1 {
                    for sx in x0..x1 {
                        n += 1;
                        if sx < 0 || sy < 0 || sx >= i64::from(crop.cell) || sy >= i64::from(h) {
                            continue;
                        }
                        let i = ((sy as u32 * w + (f * i64::from(crop.cell) + sx) as u32) * 4) as usize;
                        if rgba[i + 3] >= 128 {
                            lit += 1;
                            for k in 0..3 {
                                sum[k] += u64::from(rgba[i + k]);
                            }
                        }
                    }
                }
                row.push(if lit * 2 >= n { nearest(sum.map(|v| v / lit)) } else { '.' });
            }
        }
    }
    Ok(rows.join("\n"))
}

/// A painted crown, as a player's picture would come: gold on a clear
/// ground, three points and a red stone, 64 pixels square.
fn crown() -> (Vec<u8>, (u32, u32)) {
    let side = 64u32;
    let mut rgba = vec![0u8; (side * side * 4) as usize];
    for y in 0..side {
        for x in 0..side {
            let (fx, fy) = (x as f32 / side as f32, y as f32 / side as f32);
            let band = (0.55..0.85).contains(&fy) && (0.1..0.9).contains(&fx);
            let point = |cx: f32| (0.25..0.55).contains(&fy) && (fx - cx).abs() < (fy - 0.25) * 0.45;
            let gem = ((fx - 0.5).powi(2) + (fy - 0.7).powi(2)).sqrt() < 0.07;
            let i = ((y * side + x) * 4) as usize;
            if gem {
                rgba[i..i + 4].copy_from_slice(&[180, 40, 40, 255]);
            } else if band || point(0.2) || point(0.5) || point(0.8) {
                let shade = 1.0 - fy * 0.5 - fx * 0.2;
                rgba[i..i + 4].copy_from_slice(&[(250.0 * shade) as u8, (200.0 * shade) as u8, (70.0 * shade) as u8, 255]);
            }
        }
    }
    (rgba, (side, side))
}

/// The square knight's figure close, from a stage drawn at `scale`: the
/// stage's pixels round native `(x, y)`, four times as near.
fn near(pixels: &[u8], size: (u32, u32), scale: u32, at: (f32, f32)) -> (Vec<u8>, (u32, u32)) {
    let span = 48 * scale;
    let (cx, cy) = ((at.0 * scale as f32) as i64, (at.1 * scale as f32) as i64);
    let x0 = (cx - i64::from(span) / 2).clamp(0, i64::from(size.0 - span)) as u32;
    let y0 = (cy - i64::from(span) * 2 / 3).clamp(0, i64::from(size.1 - span)) as u32;
    let mut out = Vec::with_capacity((span * span * 16 * 4) as usize);
    for y in 0..span * 4 {
        for x in 0..span * 4 {
            let i = (((y0 + y / 4) * size.0 + x0 + x / 4) * 4) as usize;
            out.extend_from_slice(&pixels[i..i + 4]);
        }
    }
    (out, (span * 4, span * 4))
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().ok_or("usage: <outdir> [--scale N]")?);
    let mut scale = 3u32;
    while let Some(arg) = args.next() {
        if arg == "--scale" {
            scale = args.next().and_then(|v| v.parse().ok()).ok_or("--scale N")?;
        }
    }
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    arena::atari::set(true);
    let mut run = Run::at_home(17, 1, None, Home::default(), Default::default());
    run.naked_start = true;
    run.identity = true;
    run.rename_host("Ada");
    for (id, name) in [(2, "Bors"), (3, "Cai"), (4, "Dag")] {
        run.join_seat(id, name);
    }
    run.descend_for_test();
    let line_up = |run: &mut Run| -> Result<(), String> {
        run.calm_for_test();
        run.enemies.clear();
        run.projectiles.clear();
        let (w, h) = (run.room().width(), run.room().height());
        for (n, id) in [1u32, 2, 3, 4].into_iter().enumerate() {
            let k = run.players.get_mut(&id).ok_or("no knight")?;
            (k.x, k.y, k.aim_x, k.aim_y, k.invulnerable) = (w / 2.0 - 6.0 + n as f32 * 4.0, h * 0.62, 1.0, 0.0, 0);
        }
        Ok(())
    };
    line_up(&mut run)?;
    let mut gpu = Gpu::new()?;
    let mut shoot = |run: &Run, name: &str, focus: u32, pictures: Option<(&str, &(Vec<u8>, (u32, u32)))>| -> Result<(), String> {
        arena::draw_parts_outside(true);
        let (w, h) = arena::seat_size();
        let scene = Arc::new(arena::seat_scene(run, w as i32, h as i32, Some(focus)));
        let mut page = paintings::PagePaintings::default();
        let mut sent = Vec::new();
        let mut maker = StageMaker::default();
        let _ = maker.make_with(&scene, |_| None, |p| page.picture(p, scale as f32, &mut sent));
        while let Some(sha) = page.wanted() {
            let (want, (rgba, dims)) = pictures.ok_or(format!("{name}: a painting no shot has: {sha}"))?;
            if sha != want || !page.put(&sha, *dims, rgba) {
                return Err(format!("{sha} did not cut"));
            }
        }
        let mut maker = StageMaker::default();
        let mut frame = maker.make_with(&scene, |_| None, |p| page.picture(p, scale as f32, &mut sent));
        frame.pictures.extend(sent);
        let size = (frame.size.0 * scale, frame.size.1 * scale);
        let pixels = gpu.stage(frame, size);
        write_png(&out.join(format!("glass-{name}.png")), size, &pixels)?;
        let hero = run.players.get(&focus).ok_or("no knight")?;
        let at = (hero.x * arena::UNIT - scene.camera.0, hero.y * arena::UNIT - scene.camera.1);
        let (close, dims) = near(&pixels, size, scale, at);
        write_png(&out.join(format!("glass-{name}-near.png")), dims, &close)?;
        Ok(())
    };
    shoot(&run, "01-room-one-the-body", 1, None)?;
    let grant = |run: &mut Run, id: u32, wish: &str| -> Result<(), String> {
        run.wish(id, &phrasebook::get(wish).ok_or(format!("no wish {wish}"))?).map(|_| ())
    };
    for (id, wish) in [(1, "breastplate"), (2, "plain-frame"), (3, "great-helm"), (4, "boots")] {
        grant(&mut run, id, wish)?;
    }
    run.enter(1, Some(2));
    line_up(&mut run)?;
    shoot(&run, "02-room-two-the-weapon", 1, None)?;
    for (id, wish) in [(1, "bright-blade"), (2, "yew-bow"), (3, "hand-cannon"), (4, "war-crossbow")] {
        grant(&mut run, id, wish)?;
    }
    run.enter(2, Some(2));
    line_up(&mut run)?;
    shoot(&run, "03-room-three-the-ultimate", 1, None)?;
    for (id, wish) in [(1, "trebuchet"), (2, "pilgrims-arrow"), (3, "stillhour"), (4, "phantasm")] {
        grant(&mut run, id, wish)?;
    }
    shoot(&run, "04-party-made", 1, None)?;
    // A body wished to its top tier keeps its painting: a crown on the
    // circle knight's head, painted, in the inks' place.
    let picture = crown();
    let sha = "c".repeat(64);
    let card = format!(
        "name Painted Crown III\nkind hold\nmax_hp 10\ndraw worn at=head paint={sha} move=bob\n{}\n",
        ink_rows(&picture.0, picture.1, 1, 10)?
    );
    let card = angel_delve::rules::cards::check("wish-crown-3", &card).map_err(|e| format!("crown: {e:?}"))?.card;
    run.book.insert(card);
    run.players.get_mut(&3).ok_or("no knight")?.deck.push("wish-crown-3".into());
    shoot(&run, "05-painted-crown-top-tier", 3, Some((&sha, &picture)))?;
    // The second floor: the ordinary game, in its own look.
    run.descend_for_test();
    line_up(&mut run)?;
    shoot(&run, "06-floor-two", 1, Some((&sha, &picture)))?;
    println!("wrote the shots to {}", out.display());
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
        println!("adapter: {:?}", adapter.get_info().name);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("delve-identity"),
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
            label: Some("delve-identity-target"),
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
        self.core.draw(&view, size, wgpu::Color::BLACK, &drawn, "delve-identity");
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
