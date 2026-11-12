//! Her pointing, as the window shows it: her head leaning out of a corner
//! of a dark terminal-sized stage with a mock composer and transcript, and
//! a long arm (or leg) burst in from an edge to point at each, a small
//! marker on its target to see the tip lands there.
//!
//! cargo run -p angel-face --example tour_shot -- <avatar.glb> <outdir>

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_face::draw::{self, LIMB_PICTURE, SCENE_PICTURE};
use angel_face::limb::LimbKind;
use angel_face::{Cue, Drawing, Goddess, Mind};
use angel_gpu::{Core, Draw, Frame, Instance, Stage};
use angel_stage::{Layer, LayerKind, Placement};

const WINDOW: (u32, u32) = (1280, 760);
const SIDE: f32 = 300.0;
const LEAN: f32 = -0.75;
const STAGE_ID: u32 = 0x00FA_CE02;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("tour_shot: {why}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let glb = PathBuf::from(args.next().ok_or("usage: tour_shot <avatar.glb> <outdir>")?);
    let out = PathBuf::from(args.next().ok_or("an output directory")?);
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let goddess = Arc::new(Goddess::from_glb(&std::fs::read(&glb).map_err(|e| format!("{}: {e}", glb.display()))?)?);
    let mut drawing = Drawing::new(goddess.clone());
    let mut mind = Mind::new(&goddess, drawing.viewer(), 0x70E5);
    let mut gpu = Gpu::new()?;
    mind.cue(Cue::Arrive, 0.0);
    let mut t = 0.0;
    while t < 6.0 {
        mind.moment(t, None);
        t += 1.0 / 30.0;
    }
    let (w, h) = (WINDOW.0 as f32, WINDOW.1 as f32);
    // Mock cockpit: a transcript, a composer, a header strip.
    let ui = vec![
        Layer::new(1, 0, Placement::Pixels([0.0, 0.0, w, h]), LayerKind::Solid([0.03, 0.04, 0.055, 1.0])),
        Layer::new(2, 0, Placement::Pixels([20.0, 50.0, w * 0.62, h - 170.0]), LayerKind::Solid([0.06, 0.08, 0.11, 1.0])),
        Layer::new(3, 0, Placement::Pixels([20.0, h - 100.0, w - 40.0, 60.0]), LayerKind::Solid([0.08, 0.11, 0.16, 1.0])),
        Layer::new(4, 0, Placement::Pixels([0.0, 0.0, w, 30.0]), LayerKind::Solid([0.05, 0.06, 0.09, 1.0])),
    ];
    let ppm = SIDE / drawing.meters_across();
    let shots: [(&str, LimbKind, [f32; 2], [f32; 2]); 3] = [
        ("composer-arm", LimbKind::Arm, [w * 0.3, -80.0], [w * 0.25, h - 70.0]),
        ("transcript-leg", LimbKind::Leg, [w * 0.55, -80.0], [w * 0.33, h * 0.42]),
        ("header-arm", LimbKind::Arm, [-80.0, h * 0.7], [w * 0.1, 15.0]),
    ];
    for (name, kind, entry, target) in shots {
        let moment = mind.moment(t, None);
        for (id, mesh) in drawing.meshes(&moment) {
            gpu.core.upload_mesh(id, &mesh).map_err(|e| format!("mesh {id:#x}: {e}"))?;
        }
        let scene = drawing.scene(&moment, t);
        let reach = ((target[0] - entry[0]).powi(2) + (target[1] - entry[1]).powi(2)).sqrt();
        let radius = drawing.limb_radius(kind).ok_or("no limb")?;
        let tip_room = 10.0;
        let size = ((reach + tip_room + 40.0) as u32, (radius * 2.3 * ppm) as u32);
        let length = (reach + tip_room) / ppm;
        let (mid, mesh) = drawing.limb_mesh(kind, length).ok_or("no limb mesh")?;
        gpu.core.upload_mesh(mid, &mesh).map_err(|e| format!("{e}"))?;
        let limb = drawing.limb_scene(kind, length, size, ppm, tip_room, 0.05).ok_or("no limb scene")?;
        let corner = [w - 30.0, h - 60.0];
        let (sn, cs) = LEAN.sin_cos();
        let neck = [-sn * SIDE * 0.5, cs * SIDE * 0.5];
        let centre = [corner[0] - neck[0] * 0.9, corner[1] - neck[1] * 0.9];
        let mut layers = ui.clone();
        layers.extend(draw::layers(Placement::Pixels([centre[0] - SIDE / 2.0, centre[1] - SIDE / 2.0, SIDE, SIDE]), 60, &moment, false, LEAN));
        let (r, angle) = draw::limb_rect(entry, target, size, tip_room, 0.0);
        let mut arm = Layer::new(10, 61, Placement::Pixels(r), LayerKind::Picture(LIMB_PICTURE));
        arm.angle = angle;
        layers.push(arm);
        let mut mark = Layer::new(11, 62, Placement::Pixels([target[0] - 9.0, target[1] - 9.0, 18.0, 18.0]), LayerKind::Glow([1.0, 0.2, 0.2, 0.9]));
        mark.additive = true;
        layers.push(mark);
        let pixels = gpu.shoot(scene, (SIDE as u32, SIDE as u32), Some((limb, size)), layers, t)?;
        write_png(&out.join(format!("{name}.png")), WINDOW, &pixels)?;
        println!("{name}: {kind:?} {}x{} reach {reach:.0}px", size.0, size.1);
        t += 0.5;
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
            label: Some("tour-shot"),
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
        limb: Option<(angel_stage::scene::Scene, (u32, u32))>,
        layers: Vec<angel_stage::Layer>,
        t: f64,
    ) -> Result<Vec<u8>, String> {
        self.core.begin_frame();
        self.core.set_scene_clock(t);
        self.core.set_scene_target(SCENE_PICTURE, Some((scene, size)));
        if let Some((limb, limb_size)) = limb {
            self.core.set_scene_target(LIMB_PICTURE, Some((limb, limb_size)));
        }
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
            label: Some("tour-target"),
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
            .draw(&view, WINDOW, wgpu::Color::BLACK, &drawn, "tour-shot");
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
