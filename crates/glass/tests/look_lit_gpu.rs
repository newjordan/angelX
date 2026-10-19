//! A look that lights its own scene (`Look::lights`) on a real GPU, read
//! back: the sun's shadow darkens what it falls on, a zone puts its place on
//! its own ramp, a figure keeps its colours, and the reveal opens what
//! stands between the eye and its point. No window. Where the machine has no
//! adapter each test says so and passes.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use angel_gpu::Core;
use angel_stage::look::{Look, Reveal, Shadow, Sun, Zone, luminance, unit_rgb};
use angel_stage::scene::{Camera, Material, MeshRef, Node, Projection, Role, Scene, Transform, quat_axis_angle};

const RAMP: [[u8; 3]; 5] = [[16, 24, 48], [32, 48, 96], [96, 112, 160], [160, 176, 208], [224, 232, 240]];
const REDS: [[u8; 3]; 5] = [[40, 8, 8], [80, 16, 16], [150, 40, 30], [200, 90, 70], [240, 180, 160]];

/// The side of the square target and of the world it shows, top down.
const SIDE: u32 = 64;
const WORLD: f32 = 20.0;

struct Gpu {
    core: Core,
    device: wgpu::Device,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    errors: Arc<Mutex<Vec<String>>>,
}

fn gpu() -> Option<Gpu> {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
        ..Default::default()
    }))
    .ok()?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("lit-look-test"),
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .ok()?;
    let errors = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&errors);
    device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
        sink.lock().unwrap().push(error.to_string());
    }));
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("lit-look-target"),
        size: wgpu::Extent3d { width: SIDE, height: SIDE, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let core = Core::new(device.clone(), queue, format, 1.0);
    Some(Gpu { core, device, texture, view, errors })
}

impl Gpu {
    fn render(&mut self, scene: &Scene) -> Vec<u8> {
        self.core.begin_frame();
        self.core.draw_scene(&self.view, (SIDE, SIDE), scene, "lit-look-test");
        let pixels = self.core.read_back(&self.texture, (SIDE, SIDE));
        let _ = self.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(5)) });
        let errors = self.errors.lock().unwrap();
        assert!(errors.is_empty(), "GPU errors: {errors:#?}");
        pixels
    }
}

/// The colour where world point (x, z) is drawn, looking straight down.
fn sample(pixels: &[u8], x: f32, z: f32) -> [u8; 3] {
    let px = ((x / WORLD + 0.5) * SIDE as f32) as u32;
    let py = ((z / WORLD + 0.5) * SIDE as f32) as u32;
    let i = ((py.min(SIDE - 1) * SIDE + px.min(SIDE - 1)) * 4) as usize;
    [pixels[i], pixels[i + 1], pixels[i + 2]]
}

fn luma(c: [u8; 3]) -> f32 {
    luminance(unit_rgb(c))
}

/// Straight down from 30 units up over a `WORLD` square, -Z at the top.
fn top_down() -> Camera {
    Camera {
        transform: Transform {
            translation: [0.0, 30.0, 0.0],
            rotation: quat_axis_angle([1.0, 0.0, 0.0], -std::f32::consts::FRAC_PI_2),
            scale: [1.0; 3],
        },
        projection: Projection::Orthographic { height: WORLD, near: 0.5, far: 60.0 },
        clear: Some([0.0, 0.0, 0.0, 1.0]),
    }
}

/// A lit grey floor over the whole view, at height `y`.
fn floor(id: u64, y: f32, grey: f32) -> Node {
    let mut node = Node::new(id, MeshRef::Plane, Material::lit([grey, grey, grey, 1.0], 0.0));
    node.transform = Transform { translation: [0.0, y, 0.0], scale: [WORLD * 2.0, 1.0, WORLD * 2.0], ..Transform::IDENTITY };
    node
}

fn lit_look(sun: Sun) -> Look {
    let mut look = Look::new(RAMP.to_vec());
    look.pixel_rows = SIDE;
    look.sun = Some(sun);
    look
}

#[test]
fn the_suns_shadow_darkens_what_it_falls_on_and_stays_on_the_ramp() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    // A tower in the middle; the sun low in the east, so its shadow lies west.
    let mut tower = Node::new(2, MeshRef::Cube, Material::lit([0.6, 0.6, 0.6, 1.0], 0.0));
    tower.transform = Transform { translation: [0.0, 4.0, 0.0], scale: [2.0, 8.0, 2.0], ..Transform::IDENTITY };
    let look = lit_look(Sun {
        toward: [1.0, 1.0, 0.0],
        strength: 1.0,
        ambient: 0.3,
        shadow: Some(Shadow { focus: [0.0; 3], reach: 40.0 }),
        rise: 0.0,
    });
    let scene = Scene { camera: top_down(), nodes: vec![floor(1, 0.0, 0.6), tower], look: Some(look) };
    let pixels = gpu.render(&scene);
    let ramp: HashSet<[u8; 3]> = RAMP.iter().copied().collect();
    let off = pixels.chunks(4).filter(|p| !ramp.contains(&[p[0], p[1], p[2]])).count();
    let (shade, sun_east, sun_north) = (sample(&pixels, -5.0, 0.0), sample(&pixels, 5.0, 0.0), sample(&pixels, -5.0, -6.0));
    println!("in the shadow {shade:?}, in the sun east {sun_east:?} and north {sun_north:?}, {off} pixels off the ramp");
    assert_eq!(off, 0, "every pixel is a ramp colour");
    assert!(luma(shade) < luma(sun_east) && luma(shade) < luma(sun_north), "the shadow is darker than the open ground");
}

#[test]
fn a_zone_puts_its_place_on_its_own_ramp_and_the_rest_keeps_the_looks() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    let mut look = lit_look(Sun { toward: [0.0, 1.0, 0.0], strength: 0.7, ambient: 0.3, shadow: None, rise: 0.0 });
    // The west third is the zone's, fading out over two units.
    look.zones = vec![Zone { ramp: REDS.to_vec(), at: [-10.0, 0.0], half: [5.0, 20.0], radius: 0.0, feather: 2.0 }];
    let scene = Scene { camera: top_down(), nodes: vec![floor(1, 0.0, 0.6)], look: Some(look) };
    let pixels = gpu.render(&scene);
    let reds: HashSet<[u8; 3]> = REDS.iter().copied().collect();
    let ramp: HashSet<[u8; 3]> = RAMP.iter().copied().collect();
    let (west, east) = (sample(&pixels, -8.0, 0.0), sample(&pixels, 6.0, 0.0));
    println!("west {west:?}, east {east:?}");
    assert!(reds.contains(&west), "the zone's own ramp: {west:?}");
    assert!(ramp.contains(&east), "the look's ramp: {east:?}");
}

#[test]
fn a_figure_keeps_its_own_colours() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    let look = lit_look(Sun { toward: [0.0, 1.0, 0.0], strength: 0.7, ambient: 0.3, shadow: None, rise: 0.0 });
    let mut card = Node::new(2, MeshRef::Plane, Material { role: Role::Figure, ..Material::color([0.9, 0.1, 0.1, 1.0]) });
    card.transform = Transform { translation: [0.0, 1.0, 0.0], scale: [6.0, 1.0, 6.0], ..Transform::IDENTITY };
    let scene = Scene { camera: top_down(), nodes: vec![floor(1, 0.0, 0.6), card], look: Some(look) };
    let pixels = gpu.render(&scene);
    let c = sample(&pixels, 0.0, 0.0);
    println!("the figure's middle {c:?}");
    assert!(!RAMP.contains(&c), "a figure is not put on the ramp");
    assert!(c[0] > c[1].saturating_add(100), "it stays red: {c:?}");
}

#[test]
fn the_reveal_opens_what_stands_before_its_point_and_nothing_else() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    let mut look = lit_look(Sun { toward: [0.0, 1.0, 0.0], strength: 0.7, ambient: 0.3, shadow: None, rise: 0.0 });
    look.reveal = Some(Reveal { at: [0.0, 0.0, 0.0], radius: 6.0 });
    // A dark floor under a pale roof halfway up to the eye.
    let scene = Scene { camera: top_down(), nodes: vec![floor(1, 0.0, 0.15), floor(2, 15.0, 0.9)], look: Some(look.clone()) };
    let pixels = gpu.render(&scene);
    let (middle, corner) = (sample(&pixels, 0.0, 0.0), sample(&pixels, 8.0, 8.0));
    let closed = Scene { look: Some(Look { reveal: None, ..look }), ..scene };
    let shut = gpu.render(&closed);
    println!("through the opening {middle:?}, the roof at the corner {corner:?}, the roof unopened {:?}", sample(&shut, 0.0, 0.0));
    assert!(luma(middle) < luma(corner), "the floor shows through the middle");
    assert_eq!(sample(&shut, 0.0, 0.0), corner, "without a reveal the roof is whole");
}

#[test]
fn a_brand_ceiling_holds_a_surface_at_its_class_step_however_it_is_lit() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    // White ground square to a strong sun: with no ceiling it reaches the
    // ramp's top; as the brand's ground it stops at its step (docs/BRAND.md).
    let sun = Sun { toward: [0.0, 1.0, 0.0], strength: 1.25, ambient: 0.3, shadow: None, rise: 0.0 };
    let bare = Scene { camera: top_down(), nodes: vec![floor(1, 0.0, 1.0)], look: Some(lit_look(sun)) };
    let top = sample(&gpu.render(&bare), 0.0, 0.0);
    let mut ground = floor(1, 0.0, 1.0);
    ground.material.ceiling = angel_stage::brand::Class::Ground.material_ceiling();
    let held = Scene { camera: top_down(), nodes: vec![ground], look: Some(lit_look(sun)) };
    let pixels = gpu.render(&held);
    let step = angel_stage::brand::Class::Ground.ceiling_on(RAMP.len()).unwrap();
    let brightest = pixels.chunks(4).map(|p| luma([p[0], p[1], p[2]])).fold(0.0f32, f32::max);
    println!("unheld {top:?}; held at step {step} ({:?}), brightest {brightest:.3}", RAMP[step]);
    assert_eq!(top, *RAMP.last().unwrap(), "unheld, the white ground is the ramp's top");
    assert!(brightest <= luma(RAMP[step]) + 1e-3, "held, nothing is brighter than the ground's step");
}
