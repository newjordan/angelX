//! Wish paintings on the field (docs/DELVE.md, "Paintings"): a knight in a
//! fight wearing a painted piece and loosing a painted shot, made into a
//! stage by the Delve (`StageMaker`) and drawn by the GPU core offscreen as
//! the window draws it, once with the inks alone (a client without the
//! pictures, or a lower level of the wish) and once with the paintings.
//!
//! cargo run --release -p angel-glass --example delve_paintings -- <outdir> <shot.png> <worn.png> [--scale N] [--floor N] [--middle]
//!
//! `--middle` holds the wish's middle grant (its painting downgraded to
//! four inks, kept as `hd=`), which shows painted from floor
//! `paintings::HD_FLOOR` on (`--floor`, 1 by default) and in inks above it.
//!
//! Each PNG is a strip of four frames, cut into ink rows here the way a
//! `paint` line is (`together_sheets::cut`: the frames' box squared, each
//! ink the nearest of the realm's to what it covers). Writes `inks.png`,
//! `painted.png`, and each at four times round the knight
//! (`inks-near.png`, `painted-near.png`).

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_delve::render::overworld::arena::{self, paintings, stage_frame::StageMaker};
use angel_delve::render::overworld::ink;
use angel_delve::rules::{RoomKind, Run, looks};
use angel_gpu::{Core, Draw, Frame, Instance, Picture, Stage};

const STAGE: u32 = 0x00FA_57A4;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("delve_paintings: {why}");
            ExitCode::FAILURE
        }
    }
}

/// A PNG's pixels, straight RGBA, and its size.
fn read_png(path: &Path) -> Result<(Vec<u8>, (u32, u32)), String> {
    let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let file = std::fs::File::open(path).map_err(|e| fail(&e))?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16 | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().map_err(|e| fail(&e))?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("no buffer size")?];
    let info = reader.next_frame(&mut buf).map_err(|e| fail(&e))?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::GrayscaleAlpha => buf.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        other => return Err(fail(&format!("{other:?} pixels"))),
    };
    Ok((rgba, (info.width, info.height)))
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

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().ok_or("usage: <outdir> <shot.png> <worn.png> [--scale N]")?);
    let shot_png = PathBuf::from(args.next().ok_or("no shot strip")?);
    let worn_png = PathBuf::from(args.next().ok_or("no worn strip")?);
    let (mut scale, mut floor, mut middle) = (4u32, 1u32, false);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--scale" => scale = args.next().and_then(|v| v.parse().ok()).ok_or("--scale N")?,
            "--floor" => floor = args.next().and_then(|v| v.parse().ok()).ok_or("--floor N")?,
            "--middle" => middle = true,
            _ => {}
        }
    }
    // The grant shown: the whole gift, or the middle one as the phrasebook
    // gives it (`fidelity`).
    let grant = |block: String| {
        if middle { angel_delve::rules::fidelity::block(&block, angel_delve::rules::fidelity::Step::Plain) } else { block }
    };
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let shot = read_png(&shot_png)?;
    let worn = read_png(&worn_png)?;
    // Stand-ins for their sha256: any 64 hex digits name a painting here,
    // since the pictures are handed to the stage directly.
    let (shot_sha, worn_sha) = ("5".repeat(64), "a".repeat(64));
    let shot_card = format!(
        "name Cheeks\nkind hold\ndamage 5\n{}",
        grant(format!("draw shot paint={shot_sha} move=wobble\n{}\n", ink_rows(&shot.0, shot.1, 4, looks::Slot::Shot.sheet_side())?))
    );
    let worn_card = format!(
        "name Throne\nkind hold\narmor 1\n{}",
        grant(format!("draw worn at=body paint={worn_sha} move=bob\n{}\n", ink_rows(&worn.0, worn.1, 4, looks::Slot::Worn.sheet_side())?))
    );
    let mut run = Run::new(11, 1, Some("Knight"));
    let at = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).ok_or("no fight room")?;
    run.enter_for_test(at);
    run.enemies.clear();
    // The floor counted (all the paintings ask of it is its number).
    run.dungeon.depth = floor;
    for (id, raw) in [("cheeks", &shot_card), ("throne", &worn_card)] {
        let card = angel_delve::rules::cards::check(id, raw).map_err(|e| format!("{id}: {e:?}"))?.card;
        run.book.insert(card);
        run.players.get_mut(&1).ok_or("no knight")?.deck.push(id.into());
    }
    let (kx, ky) = run.players.get(&1).map(|h| (h.x, h.y)).ok_or("no knight")?;
    // Two of the card's shots, flying right from the knight.
    run.volley_for_test(kx, ky, 0.0);
    run.projectiles.truncate(2);
    for (n, shot) in run.projectiles.iter_mut().enumerate() {
        (shot.x, shot.y, shot.vx, shot.vy) = (kx + 3.0 + 3.0 * n as f32, ky - 0.4 + 0.8 * n as f32, 9.0, 0.0);
        shot.kind = angel_delve::rules::Shot::Ember;
        shot.look = Some(("card".into(), "cheeks".into()));
        shot.empowered = false;
        shot.ttl = 30 + 7 * n as u32;
    }
    arena::draw_parts_outside(true);
    let (w, h) = arena::seat_size();
    let scene = Arc::new(arena::seat_scene(&run, w as i32, h as i32, Some(1)));
    println!(
        "{} shots ({} painted), {} figures ({} paintings worn)",
        scene.shots.len(),
        scene.shots.iter().filter(|s| s.paint.is_some()).count(),
        scene.figures.len(),
        scene.figures.iter().map(|f| f.paints.len()).sum::<usize>()
    );
    let mut gpu = Gpu::new()?;
    // The inks alone: a client without the pictures.
    let frame = StageMaker::default().make(&scene, |_| None);
    let size = (frame.size.0 * scale, frame.size.1 * scale);
    let inks = gpu.stage(frame, size);
    // The paintings, cut to twice their size on screen, as a page has them.
    let mut page = paintings::PagePaintings::default();
    let mut maker = StageMaker::default();
    let mut sent = Vec::new();
    let _ = maker.make_with(&scene, |_| None, |p| page.picture(p, scale as f32, &mut sent));
    while let Some(sha) = page.wanted() {
        let (rgba, dims) = if sha == shot_sha { &shot } else { &worn };
        if !page.put(&sha, *dims, rgba) {
            return Err(format!("{sha} did not cut"));
        }
    }
    let mut maker = StageMaker::default();
    let mut frame = maker.make_with(&scene, |_| None, |p| page.picture(p, scale as f32, &mut sent));
    println!("{} paintings' pictures sent", sent.len());
    frame.pictures.extend(sent);
    let painted = gpu.stage(frame, size);
    write_png(&out.join("inks.png"), size, &inks)?;
    write_png(&out.join("painted.png"), size, &painted)?;
    // Round the knight, four times as near.
    let camera = scene.camera;
    let (cx, cy) = ((kx * arena::UNIT - camera.0) * scale as f32, (ky * arena::UNIT - camera.1) * scale as f32);
    let near = 96 * scale;
    let (x0, y0) = ((cx as i64 - i64::from(near) / 3).max(0) as u32, (cy as i64 - i64::from(near) / 2).max(0) as u32);
    let (nw, nh) = (near.min(size.0 - x0), (near * 2 / 3).min(size.1 - y0));
    for (name, pixels) in [("inks-near.png", &inks), ("painted-near.png", &painted)] {
        let mut crop = Vec::with_capacity((nw * nh * 64) as usize);
        for y in 0..nh * 4 {
            for x in 0..nw * 4 {
                let i = (((y0 + y / 4) * size.0 + x0 + x / 4) * 4) as usize;
                crop.extend_from_slice(&pixels[i..i + 4]);
            }
        }
        write_png(&out.join(name), (nw * 4, nh * 4), &crop)?;
    }
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
            label: Some("delve-paintings"),
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
            label: Some("delve-paintings-target"),
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
        self.core.draw(&view, size, wgpu::Color::BLACK, &drawn, "delve-paintings");
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
