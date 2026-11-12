//! Her spectacle (burning eyes, laser beams, a rainbow poured), drawn as
//! the window draws it: her scene into its picture, that picture leaning
//! out of a corner of a dark terminal-sized stage, and her spectacle's
//! layers over it, through the GPU core offscreen.
//!
//! cargo run -p angel-face --example spectacle_shot -- <avatar.glb> <outdir> [--frames]
//!
//! Writes a few stills through the spectacle; with `--frames`, every frame
//! of it at 30 fps as `frame-00000.png`... to make a film of.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_face::draw::{self, SCENE_PICTURE};
use angel_face::mind::SPECTACLE;
use angel_face::{Cue, Drawing, Goddess, Mind};
use angel_gpu::{Core, Draw, Frame, Instance, Stage};

const WINDOW: (u32, u32) = (1280, 760);
/// Her side and lean, as the cockpit places her (`ui/angelface.rs`).
const SIDE: f32 = 300.0;
const LEAN: f32 = -0.75;
const STAGE_ID: u32 = 0x00FA_CE01;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("spectacle_shot: {why}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let glb = PathBuf::from(
        args.next()
            .ok_or("usage: spectacle_shot <avatar.glb> <outdir> [--frames]")?,
    );
    let out = PathBuf::from(args.next().ok_or("an output directory")?);
    let frames = args.any(|a| a == "--frames");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let goddess = Arc::new(Goddess::from_glb(
        &std::fs::read(&glb).map_err(|e| format!("{}: {e}", glb.display()))?,
    )?);
    let mut drawing = Drawing::new(goddess.clone());
    let mut mind = Mind::new(&goddess, drawing.viewer(), 0xB12A);
    let mut gpu = Gpu::new()?;
    let start = 6.0;
    let side = SIDE.round() as u32;
    // She arrives and settles, then her spectacle.
    let mut t = 0.0f64;
    mind.cue(Cue::Arrive, 0.0);
    while t < start {
        mind.moment(t, None);
        t += 1.0 / 30.0;
    }
    mind.cue(Cue::Spectacle, start);
    let stills = [0.6, 1.3, 2.0, 3.3, 4.2, 5.2];
    let mut written = 0;
    let end = start + SPECTACLE + 0.3;
    let target = [WINDOW.0 as f32 * 0.24, WINDOW.1 as f32 * 0.3];
    while t < end {
        let moment = mind.moment(t, None);
        let s = t - start;
        let wanted_still = stills
            .iter()
            .find(|&&w| (s - w).abs() < 0.5 / 30.0)
            .copied();
        if frames || wanted_still.is_some() {
            for (id, mesh) in drawing.meshes(&moment) {
                gpu.core
                    .upload_mesh(id, &mesh)
                    .map_err(|e| format!("mesh {id:#x}: {e}"))?;
            }
            let scene = drawing.scene(&moment, t);
            let anchors = drawing.anchors(&scene, (side, side));
            // Her picture leaning out of the bottom right corner.
            let corner = [WINDOW.0 as f32 - 30.0, WINDOW.1 as f32 - 60.0];
            let (sn, cs) = LEAN.sin_cos();
            let neck = [-sn * SIDE * 0.5, cs * SIDE * 0.5];
            let centre = [corner[0] - neck[0] * 0.9, corner[1] - neck[1] * 0.9];
            let rect = [centre[0] - SIDE / 2.0, centre[1] - SIDE / 2.0, SIDE, SIDE];
            let on_window = |p: [f32; 2]| {
                let (x, y) = (p[0] - SIDE / 2.0, p[1] - SIDE / 2.0);
                [centre[0] + x * cs - y * sn, centre[1] + x * sn + y * cs]
            };
            let mut layers = draw::layers(
                angel_stage::Placement::Pixels(rect),
                60,
                &moment,
                false,
                LEAN,
            );
            if let Some(a) = anchors {
                let pours =
                    draw::pour_toward(corner, [WINDOW.0 as f32 / 2.0, WINDOW.1 as f32 / 2.0]);
                layers.extend(draw::spectacle_layers(
                    &moment,
                    [on_window(a.eyes[0]), on_window(a.eyes[1])],
                    on_window(a.mouth),
                    pours,
                    target,
                    SIDE / 400.0,
                    t,
                    62,
                ));
            }
            let pixels = gpu.shoot(scene, (side, side), layers, t)?;
            if frames {
                write_png(
                    &out.join(format!("frame-{written:05}.png")),
                    WINDOW,
                    &pixels,
                )?;
                written += 1;
            }
            if let Some(w) = wanted_still {
                write_png(&out.join(format!("still-{w:.1}s.png")), WINDOW, &pixels)?;
                println!(
                    "still {w:.1}s heat {:.2} laser {:.2} pour {:.2}",
                    moment.heat, moment.laser, moment.pour
                );
            }
        }
        t += 1.0 / 30.0;
    }
    if frames {
        eprintln!("{written} frames");
    }
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
            label: Some("spectacle-shot"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;
        device.on_uncaptured_error(Arc::new(|error: wgpu::Error| {
            eprintln!("GPU error: {error}")
        }));
        let core = Core::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
        Ok(Self { core, device })
    }

    /// Her scene into its picture, then a window-sized stage of `layers`
    /// over a dark terminal, read back.
    fn shoot(
        &mut self,
        scene: angel_stage::scene::Scene,
        size: (u32, u32),
        layers: Vec<angel_stage::Layer>,
        t: f64,
    ) -> Result<Vec<u8>, String> {
        self.core.begin_frame();
        self.core.set_scene_clock(t);
        self.core
            .set_scene_target(SCENE_PICTURE, Some((scene, size)));
        self.core.render_scenes();
        let mut all = vec![angel_stage::Layer::new(
            1,
            0,
            angel_stage::Placement::Pixels([0.0, 0.0, WINDOW.0 as f32, WINDOW.1 as f32]),
            angel_stage::LayerKind::Solid([0.03, 0.04, 0.055, 1.0]),
        )];
        all.extend(layers);
        self.core.set_stage(
            STAGE_ID,
            Some(Stage {
                id: STAGE_ID,
                size: WINDOW,
                cols: WINDOW.0 as u16,
                rows: WINDOW.1 as u16,
                layers: all,
                scene: 1,
                enter: None,
                tilt: 0.0,
                props: None,
                impact: 0.0,
            }),
        );
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("spectacle-target"),
            size: wgpu::Extent3d {
                width: WINDOW.0,
                height: WINDOW.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        self.core.render_stages((1, 1), t);
        let shown = Instance {
            rect: [0.0, 0.0, WINDOW.0 as f32, WINDOW.1 as f32],
            uv: [0.0, 0.0, WINDOW.0 as f32, WINDOW.1 as f32],
            color: [1.0; 4],
            kind: angel_gpu::PICTURE,
            ..bytemuck::Zeroable::zeroed()
        };
        let mut parts = vec![(
            Draw::Picture {
                id: STAGE_ID,
                linear: false,
            },
            vec![shown],
        )];
        let mut drawn = Frame::default();
        drawn.assemble_into(&mut parts, &mut Vec::new());
        self.core
            .draw(&view, WINDOW, wgpu::Color::BLACK, &drawn, "spectacle-shot");
        Ok(self.core.read_back(&texture, WINDOW))
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
