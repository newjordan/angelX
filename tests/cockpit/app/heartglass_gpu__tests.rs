//! The Heartglass's heart drawn on a real GPU into a texture and read back,
//! through the same `angel-gpu` scene path the window uses
//! (docs/HEARTGLASS.md). No window: the test makes its own device, and
//! where the machine has no adapter each test says so and passes, as
//! `crates/glass/tests/scene_gpu.rs` does. The clouds are made in code.
//! `cargo test --bin angel heartglass::gpu_tests -- --nocapture` prints
//! the pixels checked.

use std::sync::{Arc, Mutex};

use angel_glass::gpu::Core;
use angel_glass::wgpu;

use super::campaign::{Cloud, decode_cloud};
use super::heart::{self, Pose};
use super::tests::{StageSpec, encode, three_stages};

const SIZE: (u32, u32) = (384, 384);

struct Gpu {
    core: Core,
    device: wgpu::Device,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    errors: Arc<Mutex<Vec<String>>>,
}

/// Run a future that a native wgpu answers at once.
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
        std::thread::yield_now();
    }
}

/// A device, a core on it and a target; `None` without an adapter.
fn gpu() -> Option<Gpu> {
    let instance = wgpu::Instance::default();
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
        ..Default::default()
    }))
    .ok()?;
    let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("heartglass-test"),
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .ok()?;
    println!("adapter: {}", adapter.get_info().name);
    let errors = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&errors);
    device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
        sink.lock().unwrap().push(error.to_string());
    }));
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("heartglass-test-target"),
        size: wgpu::Extent3d {
            width: SIZE.0,
            height: SIZE.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let core = Core::new(device.clone(), queue, format, 1.0);
    Some(Gpu {
        core,
        device,
        texture,
        view,
        errors,
    })
}

impl Gpu {
    /// Upload every stage's meshes and the ring, as the panel's thread does.
    fn upload(&mut self, cloud: &Cloud) -> Vec<heart::Built> {
        self.core
            .upload_mesh(heart::RING_MESH, &heart::ring_mesh())
            .expect("the ring uploads");
        (0..cloud.stages.len())
            .map(|i| {
                let (built, meshes) = heart::stage_meshes(cloud, i).unwrap();
                for (id, mesh) in meshes {
                    self.core.upload_mesh(id, &mesh).expect("a slab uploads");
                }
                built
            })
            .collect()
    }

    fn render(&mut self, scene: &angel_stage::scene::Scene) -> Vec<u8> {
        self.core.begin_frame();
        self.core
            .draw_scene(&self.view, SIZE, scene, "heartglass-test");
        self.core.read_back(&self.texture, SIZE)
    }

    fn clean(&self) {
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(5)),
        });
        let errors = self.errors.lock().unwrap();
        assert!(errors.is_empty(), "GPU errors: {errors:#?}");
    }
}

fn at(pixels: &[u8], [x, y]: [u32; 2]) -> [u8; 4] {
    let i = ((y * SIZE.0 + x) * 4) as usize;
    [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
}

/// The pixel world point `p` lands on.
fn pixel_of(p: [f32; 3]) -> [u32; 2] {
    let f = heart::project(p, SIZE).expect("in front of the eye");
    [
        (f[0] * SIZE.0 as f32)
            .floor()
            .clamp(0.0, SIZE.0 as f32 - 1.0) as u32,
        (f[1] * SIZE.1 as f32)
            .floor()
            .clamp(0.0, SIZE.1 as f32 - 1.0) as u32,
    ]
}

/// A time at which the heart has turned whole turns and its one stage is
/// built and held: the cells are where the cloud put them.
fn unturned() -> f32 {
    std::f32::consts::TAU / heart::SPIN * 4.0
}

fn stage(points: Vec<([f32; 3], u8)>) -> Cloud {
    let spec: StageSpec = ("E9.5".to_string(), 0, String::new(), points);
    decode_cloud(&encode(&[spec])).unwrap()
}

#[test]
fn the_nearer_cell_covers_the_farther_whichever_comes_first() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    // Myocardium (red) near the eye, endocardium (cyan) behind it on the
    // same line of sight (the line from the eye through the heart's middle,
    // which the beat's swell, a scaling about that middle, keeps), listed
    // both ways round.
    let eye = heart::camera().transform.translation;
    let on_sight = |k: f32| [eye[0] * k, eye[1] * k, eye[2] * k];
    let (near_at, far_at) = (on_sight(0.07), on_sight(-0.07));
    let near = (near_at, 0u8);
    let far = (far_at, 1u8);
    let t = unturned();
    let target = pixel_of(near_at);
    assert_eq!(
        target,
        pixel_of(far_at),
        "both cells are on one pixel's line of sight"
    );
    let mut seen = Vec::new();
    for points in [vec![near, far], vec![far, near]] {
        let cloud = stage(points);
        let built = gpu.upload(&cloud);
        let pixels = gpu.render(&heart::scene(&built, Pose { t, flare: 0.0 }));
        let px = at(&pixels, target);
        println!("near red over far cyan at {target:?}: {px:?}");
        assert!(
            px[0] > 170 && px[0] as i32 > px[1] as i32 + 90 && px[0] as i32 > px[2] as i32 + 70,
            "red in front: {px:?}"
        );
        assert_eq!(px[3], 255, "a core is opaque");
        seen.push(px);
    }
    // Depth, not the listing order, decided.
    assert!(
        seen[0]
            .iter()
            .zip(&seen[1])
            .all(|(a, b)| a.abs_diff(*b) <= 2),
        "{seen:?}"
    );
    // The far cell alone shows cyan there: the red one really covered it.
    let cloud = stage(vec![far]);
    let built = gpu.upload(&cloud);
    let pixels = gpu.render(&heart::scene(&built, Pose { t, flare: 0.0 }));
    let px = at(&pixels, target);
    println!("far cyan alone: {px:?}");
    assert!(px[1] > 160 && px[2] > 170 && px[0] < 110, "cyan: {px:?}");
    gpu.clean();
}

#[test]
fn cardiac_cells_shine_in_their_colour_and_the_rest_are_dim() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    let t = unturned();
    // Epicardium (gold) on the left, neural (a context cell) on the right.
    let cloud = stage(vec![([-0.5, 0.2, 0.0], 2), ([0.5, 0.2, 0.0], 4)]);
    let built = gpu.upload(&cloud);
    let pixels = gpu.render(&heart::scene(&built, Pose { t, flare: 0.0 }));
    let gold = at(&pixels, pixel_of([-0.5, 0.2, 0.0]));
    let dim = at(&pixels, pixel_of([0.5, 0.2, 0.0]));
    println!("epicardium {gold:?}, neural {dim:?}");
    assert!(
        gold[0] > 200 && gold[1] > 140 && gold[2] < 110,
        "gold shines: {gold:?}"
    );
    let brightest = dim[0].max(dim[1]).max(dim[2]);
    assert!(
        brightest > 15 && brightest < 120,
        "a context cell is there but dim: {dim:?}"
    );
    // A cardiac cell glows: its shell lights the pixels beside its core.
    let [x, y] = pixel_of([-0.5, 0.2, 0.0]);
    let beside = at(&pixels, [x + 4, y]);
    println!("beside the gold core: {beside:?}");
    assert!(
        beside[3] > 0 && beside[3] < 255 && beside[0] > beside[2],
        "a soft gold glow: {beside:?}"
    );
    // Nothing else is drawn: the corners are clear.
    for corner in [
        [0, 0],
        [SIZE.0 - 1, 0],
        [0, SIZE.1 - 1],
        [SIZE.0 - 1, SIZE.1 - 1],
    ] {
        assert_eq!(at(&pixels, corner), [0, 0, 0, 0], "clear at {corner:?}");
    }
    gpu.clean();
}

#[test]
fn a_whole_synthetic_heart_draws_held_and_mid_scan() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    let cloud = decode_cloud(&encode(&three_stages())).unwrap();
    let built = gpu.upload(&cloud);
    let covered = |pixels: &[u8]| pixels.chunks(4).filter(|p| p[3] > 0).count();
    let reddish = |pixels: &[u8]| {
        pixels
            .chunks(4)
            .filter(|p| p[3] == 255 && p[0] > 180 && p[1] < 120)
            .count()
    };
    let slot = heart::HOLD + heart::SCAN;
    let held = gpu.render(&heart::scene(
        &built,
        Pose {
            t: slot + heart::SCAN + 1.0,
            flare: 0.0,
        },
    ));
    let mid = gpu.render(&heart::scene(
        &built,
        Pose {
            t: 2.0 * slot + heart::SCAN * 0.5,
            flare: 0.0,
        },
    ));
    let idle = gpu.render(&heart::scene(&[], Pose { t: 1.0, flare: 0.0 }));
    println!(
        "held: {} covered, {} myocardium-red; mid-scan: {} covered; idle ring: {} covered",
        covered(&held),
        reddish(&held),
        covered(&mid),
        covered(&idle)
    );
    assert!(covered(&held) > 4000 && reddish(&held) > 30);
    assert!(covered(&mid) > 2000);
    // The idle ring alone: thin, blended, under where the heart stands.
    assert!(covered(&idle) > 300 && covered(&idle) < covered(&held));
    let front = heart::project([0.0, heart::RING_Y, heart::RING_SCALE], SIZE).unwrap();
    assert!(
        (0.0..0.97).contains(&front[1]),
        "the ring's near edge is in view: {front:?}"
    );
    let ring = pixel_of([0.0, heart::RING_Y, heart::RING_SCALE]);
    let near_ring = (ring[1].saturating_sub(3)..ring[1] + 4)
        .flat_map(|y| (ring[0].saturating_sub(3)..ring[0] + 4).map(move |x| [x, y]))
        .any(|p| at(&idle, p)[3] > 0);
    assert!(near_ring, "the ring's near edge is drawn at {ring:?}");
    gpu.clean();
}
