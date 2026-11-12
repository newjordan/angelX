//! Her frames drawn by the GPU core offscreen, as the window draws them,
//! over a dark terminal: the harness she is looked at with.
//!
//! cargo run -p angel-face --example goddess_shot -- <avatar.glb> <outdir> [--size WxH] [--track a2f.json [--rest rest.json]] [--only NAME] [--frames N] [--regalia]
//!
//! Each still is a short script of cues played through her mind at 30 fps
//! up to the moment shot. With `--track`, the speaking still speaks it;
//! `--frames N` also writes N frames of that line as `speak-00000.png`...

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_face::{Cue, Drawing, Goddess, Mind, Track};
use angel_gpu::{Core, Picture};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("goddess_shot: {why}");
            ExitCode::FAILURE
        }
    }
}

/// A still: its name, the cues and when, and the moment shot.
struct Still {
    name: &'static str,
    cues: Vec<(f64, Cue)>,
    at: f64,
    speak_from: Option<f64>,
}

fn stills() -> Vec<Still> {
    let typing = |from: f64, to: f64| {
        (0..((to - from) * 7.0) as usize).map(move |i| (from + i as f64 / 7.0, Cue::Typing))
    };
    let working = |from: f64, to: f64| {
        (0..((to - from) * 2.0) as usize).map(move |i| (from + i as f64 / 2.0, Cue::Working))
    };
    vec![
        Still {
            name: "01-forming",
            cues: vec![(0.0, Cue::Arrive)],
            at: 0.9,
            speak_from: None,
        },
        Still {
            name: "02-waking",
            cues: vec![(0.0, Cue::Arrive)],
            at: 2.6,
            speak_from: None,
        },
        Still {
            name: "03-present",
            cues: vec![(0.0, Cue::Arrive)],
            at: 7.0,
            speak_from: None,
        },
        Still {
            name: "04-reading",
            cues: [(0.0, Cue::Arrive)]
                .into_iter()
                .chain(typing(5.0, 9.0))
                .collect(),
            at: 8.6,
            speak_from: None,
        },
        Still {
            name: "05-thinking",
            cues: [(0.0, Cue::Arrive), (5.0, Cue::Submitted)]
                .into_iter()
                .chain(working(5.2, 11.0))
                .collect(),
            at: 10.5,
            speak_from: None,
        },
        Still {
            name: "06-joy",
            cues: [(0.0, Cue::Arrive)]
                .into_iter()
                .chain(working(5.0, 7.0))
                .chain([(7.0, Cue::Finished { ok: true })])
                .collect(),
            at: 7.7,
            speak_from: None,
        },
        Still {
            name: "07-speaking",
            cues: vec![(0.0, Cue::Arrive)],
            at: 7.0,
            speak_from: Some(6.0),
        },
    ]
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let glb = PathBuf::from(
        args.next()
            .ok_or("usage: goddess_shot <avatar.glb> <outdir> [--size WxH] [--track a2f.json]")?,
    );
    let out = PathBuf::from(args.next().ok_or("an output directory")?);
    let mut size = (720u32, 720u32);
    let mut track_path = None;
    let mut rest_path: Option<PathBuf> = None;
    let mut only = None;
    let mut frames = 0usize;
    let mut regalia = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--size" => {
                let v = args.next().ok_or("--size WxH")?;
                let (w, h) = v.split_once('x').ok_or("--size WxH")?;
                size = (
                    w.parse().map_err(|_| "--size WxH")?,
                    h.parse().map_err(|_| "--size WxH")?,
                );
            }
            "--track" => track_path = Some(PathBuf::from(args.next().ok_or("--track a2f.json")?)),
            "--rest" => rest_path = Some(PathBuf::from(args.next().ok_or("--rest rest.json")?)),
            "--only" => only = Some(args.next().ok_or("--only NAME")?),
            "--regalia" => regalia = true,
            "--frames" => {
                frames = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or("--frames N")?
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let bytes = std::fs::read(&glb).map_err(|e| format!("{}: {e}", glb.display()))?;
    let began = std::time::Instant::now();
    let goddess = Arc::new(Goddess::from_glb(&bytes)?);
    eprintln!(
        "her: {} parts, {} channels, eyes at {:?}, head at {:?}, neck at {:?} ({} ms)",
        goddess.parts.len(),
        goddess.channels.len(),
        goddess.eyes_at,
        goddess.head_at,
        goddess.neck_at,
        began.elapsed().as_millis()
    );
    for part in &goddess.parts {
        eprintln!(
            "  {:?}: {} vertices, {} triangles, groups {:?}, {} shapes",
            part.kind,
            part.mesh.positions.len(),
            part.mesh.indices.len() / 3,
            part.mesh.groups,
            part.targets.len()
        );
    }
    let read = |p: &PathBuf| -> Result<Track, String> {
        Track::from_json(
            &std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?,
            &goddess,
        )
    };
    let track = match &track_path {
        Some(p) => {
            let t = read(p)?;
            Some(match &rest_path {
                Some(r) => t.without_rest(&read(r)?),
                None => t,
            })
        }
        None => None,
    };
    let mut gpu = Gpu::new()?;
    let drawing_for = |g: &Arc<Goddess>| {
        let mut d = Drawing::new(g.clone());
        d.regalia = regalia;
        d
    };
    for (id, width, height, rgba) in drawing_for(&goddess).pictures() {
        gpu.core.upload(Picture {
            id,
            width,
            height,
            rgba,
            cols: 0,
            rows: 0,
        });
    }

    for still in stills() {
        if only.as_deref().is_some_and(|o| o != still.name) {
            continue;
        }
        let mut drawing = drawing_for(&goddess);
        let mut mind = Mind::new(&goddess, drawing.viewer(), 0x5EED);
        let mut cues = still.cues.iter().peekable();
        let steps = (still.at * 30.0) as usize;
        let mut moment = None;
        for s in 0..=steps {
            let t = s as f64 / 30.0;
            while let Some((at, cue)) = cues.peek() {
                if *at > t {
                    break;
                }
                mind.cue(*cue, *at);
                cues.next();
            }
            let speech = speech_at(track.as_ref(), still.speak_from, t, goddess.channels.len());
            moment = Some(mind.moment(t, speech.as_ref().map(|(w, v)| (w.as_slice(), *v))));
        }
        let moment = moment.expect("at least one step");
        for (id, mesh) in drawing.meshes(&moment) {
            gpu.core
                .upload_mesh(id, &mesh)
                .map_err(|e| format!("mesh {id:#x}: {e}"))?;
        }
        let scene = drawing.scene(&moment, still.at);
        let pixels = gpu.shoot(&scene, size, still.at);
        write_png(
            &out.join(format!("{}.png", still.name)),
            size,
            &over_terminal(&pixels),
        )?;
        println!(
            "{} presence {:.2} liquid {:.2} glow {:.2} voice {:.2} head {:?}",
            still.name, moment.presence, moment.liquid, moment.glow, moment.voice, moment.head
        );
    }

    if frames > 0 {
        let track = track.as_ref().ok_or("--frames wants a --track")?;
        let mut drawing = drawing_for(&goddess);
        let mut mind = Mind::new(&goddess, drawing.viewer(), 0x5EED);
        mind.cue(Cue::Arrive, -10.0);
        let start = 4.0;
        for s in 0..(start * 30.0) as usize {
            mind.moment(s as f64 / 30.0 - 10.0 + 6.0, None);
        }
        for f in 0..frames.min((track.duration * 30.0) as usize + 15) {
            let t = f as f64 / 30.0;
            let speech = speech_at(Some(track), Some(0.0), t, goddess.channels.len());
            let moment = mind.moment(t, speech.as_ref().map(|(w, v)| (w.as_slice(), *v)));
            for (id, mesh) in drawing.meshes(&moment) {
                gpu.core
                    .upload_mesh(id, &mesh)
                    .map_err(|e| format!("mesh {id:#x}: {e}"))?;
            }
            let scene = drawing.scene(&moment, t);
            let pixels = gpu.shoot(&scene, size, t);
            write_png(
                &out.join(format!("speak-{f:05}.png")),
                size,
                &over_terminal(&pixels),
            )?;
        }
        eprintln!("wrote the line's frames");
    }
    Ok(())
}

/// The track's weights `t` seconds in, for a line that began at `from`.
fn speech_at(
    track: Option<&Track>,
    from: Option<f64>,
    t: f64,
    channels: usize,
) -> Option<(Vec<f32>, f32)> {
    let track = track?;
    let from = from?;
    track.sample((t - from) as f32, channels)
}

/// Premultiplied RGBA laid over a dark terminal's background, opaque.
fn over_terminal(rgba: &[u8]) -> Vec<u8> {
    let bg = [8.0f32, 10.0, 14.0];
    let mut out = rgba.to_vec();
    for px in out.chunks_exact_mut(4) {
        let a = px[3] as f32 / 255.0;
        for c in 0..3 {
            px[c] = (px[c] as f32 + bg[c] * (1.0 - a)).min(255.0) as u8;
        }
        px[3] = 255;
    }
    out
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
            label: Some("goddess-shot"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;
        device.on_uncaptured_error(Arc::new(|error: wgpu::Error| {
            eprintln!("GPU error: {error}")
        }));
        let core = Core::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
        Ok(Self {
            core,
            device,
            target: None,
        })
    }

    fn shoot(&mut self, scene: &angel_stage::scene::Scene, size: (u32, u32), t: f64) -> Vec<u8> {
        if self.target.as_ref().is_none_or(|(s, _)| *s != size) {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("goddess-target"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
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
        self.core.set_scene_clock(t);
        self.core.draw_scene(&view, size, scene, "goddess");
        self.core.read_back(texture, size)
    }
}

fn write_png(path: &Path, size: (u32, u32), rgba: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size.0, size.1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    writer
        .write_image_data(rgba)
        .map_err(|e| format!("{}: {e}", path.display()))
}
