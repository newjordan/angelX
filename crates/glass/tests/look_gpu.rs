//! A scene's look on a real GPU, read back (docs/GLASS_CORE.md, "The world's
//! look"): the scene is drawn at the look's internal size, every pixel is put
//! on the ramp by the Bayer threshold, accents are kept, the distance fades
//! into the fog, and the result is enlarged by nearest. No window, no
//! surface. Where the machine has no adapter each test says so and passes.
//! `cargo test -p angel-glass --test look_gpu -- --nocapture` prints the
//! pixel facts.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use angel_gpu::{Core, Frame};
use angel_stage::look::{Dither, Fog, Look, bayer_threshold, luminance, unit_rgb};
use angel_stage::scene::{Camera, Material, MeshData, MeshRef, Node, Projection, Scene, Transform};
use angel_stage::{Layer, LayerKind, Placement};

/// The ramp the tests use: even channels, so the midpoint of two entries is
/// an exact 8-bit colour.
const RAMP: [[u8; 3]; 5] = [
    [16, 24, 48],
    [32, 48, 96],
    [96, 112, 160],
    [160, 176, 208],
    [224, 232, 240],
];
const ACCENT: [u8; 3] = [240, 224, 136];

struct Gpu {
    core: Core,
    device: wgpu::Device,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: (u32, u32),
    errors: Arc<Mutex<Vec<String>>>,
}

/// A device, a core on it and a `size` target; `None` without an adapter.
fn gpu(size: (u32, u32)) -> Option<Gpu> {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
        ..Default::default()
    }))
    .ok()?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("look-test"),
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
        label: Some("look-test-target"),
        size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let core = Core::new(device.clone(), queue, format, 1.0);
    Some(Gpu { core, device, texture, view, size, errors })
}

impl Gpu {
    /// Draw `scene` into the target and read it back.
    fn render(&mut self, scene: &Scene) -> Vec<u8> {
        self.core.begin_frame();
        self.core.draw_scene(&self.view, self.size, scene, "look-test");
        self.core.read_back(&self.texture, self.size)
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

fn at(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * width + x) * 4) as usize;
    [pixels[i], pixels[i + 1], pixels[i + 2]]
}

fn alpha_at(pixels: &[u8], width: u32, x: u32, y: u32) -> u8 {
    pixels[((y * width + x) * 4 + 3) as usize]
}

/// The tests' look: the ramp, one accent, a 4x4 Bayer dither, `rows` rows.
fn look(rows: u32) -> Look {
    let mut look = Look::new(RAMP.to_vec());
    look.accent = vec![ACCENT];
    look.pixel_rows = rows;
    look
}

/// Looking down -Z from z = 5 at a box 2 units tall: a quad scaled 2 fills it.
fn ortho(clear: Option<[f32; 4]>) -> Camera {
    Camera {
        transform: Transform::from_translation([0.0, 0.0, 5.0]),
        projection: Projection::Orthographic { height: 2.0, near: 0.1, far: 20.0 },
        clear,
    }
}

/// A quad over the whole view with a horizontal grey gradient in its vertex
/// colours, black on the left to white on the right.
fn gradient() -> MeshData {
    MeshData {
        positions: vec![[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [1.0, 1.0, 0.0], [-1.0, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        uvs: vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
        colors: vec![[0.0, 0.0, 0.0, 1.0], [1.0; 4], [1.0; 4], [0.0, 0.0, 0.0, 1.0]],
        indices: vec![0, 1, 2, 0, 2, 3],
        groups: Vec::new(),
    }
}

/// A scene of one full-view node, an uploaded mesh (id 9) or a quad.
fn full_view(gpu: &mut Gpu, mesh: Option<MeshRef>, material: Material, look: Option<Look>) -> Scene {
    // The gradient is already 2 units across; the unit quad is scaled to it.
    let (mesh, scale) = match mesh {
        Some(mesh) => (mesh, [2.0, 2.0, 1.0]),
        None => {
            gpu.core.upload_mesh(9, &gradient()).expect("valid mesh");
            (MeshRef::Uploaded(9), [1.0; 3])
        }
    };
    let mut node = Node::new(1, mesh, material);
    node.transform = Transform::from_scale(scale);
    Scene { camera: ortho(Some([0.0, 0.0, 0.0, 1.0])), nodes: vec![node], look }
}

fn ramp_set() -> HashSet<[u8; 3]> {
    RAMP.iter().copied().collect()
}

#[test]
fn a_gradient_comes_out_in_ramp_colours_only_and_matches_the_reference_maths() {
    let Some(mut gpu) = gpu((64, 64)) else {
        println!("no GPU adapter: skipped");
        return;
    };
    let reference = look(64);
    let scene = full_view(&mut gpu, None, Material::default(), Some(reference.clone()));
    let pixels = gpu.render(&scene);
    let ramp = ramp_set();
    let mut seen = HashSet::new();
    let (mut off_ramp, mut different, mut transparent) = (0, 0, 0);
    for y in 0..64 {
        for x in 0..64 {
            let got = at(&pixels, 64, x, y);
            transparent += usize::from(alpha_at(&pixels, 64, x, y) != 255);
            off_ramp += usize::from(!ramp.contains(&got));
            seen.insert(got);
            // The input at this pixel: the gradient at its centre, as 8 bits.
            let v = ((x as f32 + 0.5) / 64.0 * 255.0).round() / 255.0;
            let want = reference.quantize([v, v, v], false, x, y);
            different += usize::from(got != want);
        }
    }
    println!(
        "gradient 64x64: {} of 5 ramp colours seen, {off_ramp} off-ramp pixels, {transparent} not opaque, {different} of 4096 differ from Look::quantize",
        seen.len()
    );
    assert_eq!((off_ramp, transparent), (0, 0), "every pixel is a ramp colour");
    assert_eq!(seen.len(), 5, "the whole ramp is used by a black-to-white gradient");
    // The only places the GPU may differ from the CPU are where the 8-bit
    // colour the GPU drew was one level off the one the CPU computed.
    assert!(different <= 16, "{different} pixels differ from the reference");
    // The left edge is the dark end, the right edge the light end.
    assert_eq!(at(&pixels, 64, 0, 0), RAMP[0]);
    assert_eq!(at(&pixels, 64, 63, 0), RAMP[4]);
    gpu.clean();
}

#[test]
fn the_halfway_level_is_an_exact_checkerboard_on_the_gpu() {
    let Some(mut gpu) = gpu((64, 64)) else {
        println!("no GPU adapter: skipped");
        return;
    };
    for (dither, name) in [(Dither::Bayer4, "Bayer4"), (Dither::Bayer8, "Bayer8")] {
        for k in 0..4 {
            // Exactly between entries k and k + 1.
            let mid = [0, 1, 2].map(|i| ((u16::from(RAMP[k][i]) + u16::from(RAMP[k + 1][i])) / 2) as u8);
            let tint = unit_rgb(mid);
            let mut l = look(64);
            l.dither = dither;
            let scene = full_view(
                &mut gpu,
                Some(MeshRef::Quad),
                Material::color([tint[0], tint[1], tint[2], 1.0]),
                Some(l),
            );
            let pixels = gpu.render(&scene);
            let mut wrong = 0;
            for y in 0..64 {
                for x in 0..64 {
                    let upper = (x + y) % 2 == 0;
                    let want = if upper { RAMP[k + 1] } else { RAMP[k] };
                    wrong += usize::from(at(&pixels, 64, x, y) != want);
                }
            }
            println!("{name}, entries {k}/{}: {wrong} of 4096 pixels off the checkerboard", k + 1);
            assert_eq!(wrong, 0, "{name} {k}");
        }
    }
    gpu.clean();
}

#[test]
fn a_quarter_level_is_four_pixels_of_each_four_by_four() {
    let Some(mut gpu) = gpu((64, 64)) else {
        println!("no GPU adapter: skipped");
        return;
    };
    // A quarter of the way from entry 1 to entry 2 (8-bit channels, exact: the
    // difference is 64, a quarter is 16).
    let quarter = [0, 1, 2].map(|i| RAMP[1][i] + (RAMP[2][i] - RAMP[1][i]) / 4);
    let tint = unit_rgb(quarter);
    let scene = full_view(
        &mut gpu,
        Some(MeshRef::Quad),
        Material::color([tint[0], tint[1], tint[2], 1.0]),
        Some(look(64)),
    );
    let pixels = gpu.render(&scene);
    let mut tiles = HashSet::new();
    for ty in 0..16 {
        for tx in 0..16 {
            let upper = (0..16)
                .filter(|i| at(&pixels, 64, tx * 4 + i % 4, ty * 4 + i / 4) == RAMP[2])
                .count();
            tiles.insert(upper);
        }
    }
    println!("quarter level: upper-entry pixels per 4x4 tile {tiles:?}");
    assert_eq!(tiles, HashSet::from([4]));
    // The same pixels as the matrix says: those whose threshold is under 1/4.
    for y in 0..64 {
        for x in 0..64 {
            let upper = at(&pixels, 64, x, y) == RAMP[2];
            assert_eq!(upper, bayer_threshold(Dither::Bayer4, x, y) < 0.25, "({x},{y})");
        }
    }
    gpu.clean();
}

#[test]
fn an_accent_is_kept_exactly_and_never_dithered() {
    let Some(mut gpu) = gpu((64, 64)) else {
        println!("no GPU adapter: skipped");
        return;
    };
    // Left half: a plain quad of a colour that would dither. Right half: an
    // accent quad of a nearby warm yellow.
    let tint = [0.93, 0.85, 0.5];
    let plain = {
        let mut node = Node::new(1, MeshRef::Quad, Material::color([tint[0], tint[1], tint[2], 1.0]));
        node.transform = Transform { translation: [-0.5, 0.0, 0.0], scale: [1.0, 2.0, 1.0], ..Transform::IDENTITY };
        node
    };
    let accent = {
        let material = Material { accent: true, ..Material::color([tint[0], tint[1], tint[2], 1.0]) };
        let mut node = Node::new(2, MeshRef::Quad, material);
        node.transform = Transform { translation: [0.5, 0.0, 0.0], scale: [1.0, 2.0, 1.0], ..Transform::IDENTITY };
        node
    };
    let scene = Scene { camera: ortho(Some([0.0, 0.0, 0.0, 1.0])), nodes: vec![plain, accent], look: Some(look(64)) };
    let pixels = gpu.render(&scene);
    let (mut accents, mut plain_on_ramp) = (0, 0);
    for y in 0..64 {
        for x in 0..64 {
            let got = at(&pixels, 64, x, y);
            if x >= 33 {
                accents += usize::from(got == ACCENT);
            } else if x < 31 {
                plain_on_ramp += usize::from(ramp_set().contains(&got));
            }
        }
    }
    println!("accent half: {accents} of {} pixels are exactly {ACCENT:?}; plain half: {plain_on_ramp} of {} on the ramp", 31 * 64, 31 * 64);
    assert_eq!(accents, 31 * 64);
    assert_eq!(plain_on_ramp, 31 * 64);
    gpu.clean();
}

/// A wide floor receding from a camera at the origin, the horizon level with
/// the middle of the view, seen with `fog`; its bright tint is one the ramp
/// dithers between its upper entries.
fn floor_scene(fog: Option<Fog>) -> Scene {
    let mut floor = Node::new(1, MeshRef::Plane, Material::color([0.7, 0.72, 0.78, 1.0]));
    floor.transform = Transform { translation: [0.0, -1.0, 0.0], scale: [400.0, 1.0, 400.0], ..Transform::IDENTITY };
    let camera = Camera {
        transform: Transform::default(),
        projection: Projection::Perspective { fov_y: 60.0_f32.to_radians(), near: 0.1, far: 90.0 },
        // The sky: the fog's colour, so the far plane does not show a seam.
        clear: Some([unit_rgb(RAMP[0])[0], unit_rgb(RAMP[0])[1], unit_rgb(RAMP[0])[2], 1.0]),
    };
    let mut l = look(64);
    l.fog = fog;
    Scene { camera, nodes: vec![floor], look: Some(l) }
}

#[test]
fn fog_darkens_with_distance_and_the_far_is_the_fog_colour() {
    let Some(mut gpu) = gpu((64, 64)) else {
        println!("no GPU adapter: skipped");
        return;
    };
    let band_means = |pixels: &[u8]| -> Vec<f32> {
        // Rows 32..64 are the floor, nearest at the bottom. Eight bands of 4.
        (0..8)
            .rev()
            .map(|b| {
                let mut sum = 0.0;
                for y in (32 + b * 4)..(36 + b * 4) {
                    for x in 0..64 {
                        sum += luminance(unit_rgb(at(pixels, 64, x, y)));
                    }
                }
                sum / 256.0
            })
            .collect()
    };
    let foggy = gpu.render(&floor_scene(Some(Fog { color: 0, start: 3.0, end: 30.0 })));
    let clear = gpu.render(&floor_scene(None));
    let (foggy_bands, clear_bands) = (band_means(&foggy), band_means(&clear));
    println!("mean luminance, near to far, with fog: {foggy_bands:.3?}");
    println!("mean luminance, near to far, no fog:   {clear_bands:.3?}");
    // With fog, every band is at most as bright as the one nearer, and the
    // nearest is clearly brighter than the farthest.
    for pair in foggy_bands.windows(2) {
        assert!(pair[1] <= pair[0] + 1e-4, "{foggy_bands:?}");
    }
    assert!(foggy_bands[0] > foggy_bands[7] + 0.15, "{foggy_bands:?}");
    // The nearest rows are past no fog at all: the same as the unfogged floor.
    assert_eq!(&foggy[(63 * 64 * 4) as usize..], &clear[(63 * 64 * 4) as usize..]);
    // Everything is still a ramp colour, and the farthest rows below the
    // horizon are the fog colour exactly.
    let ramp = ramp_set();
    assert!(foggy.chunks(4).all(|p| ramp.contains(&[p[0], p[1], p[2]])));
    let far_row = |pixels: &[u8]| (0..64).filter(|&x| at(pixels, 64, x, 33) == RAMP[0]).count();
    println!("row 33 (nearly at the horizon): {} of 64 pixels are the fog colour with fog, {} without", far_row(&foggy), far_row(&clear));
    assert_eq!(far_row(&foggy), 64);
    // Without fog the floor does not fade at the far end.
    assert!(clear_bands[7] > 0.3, "{clear_bands:?}");
    assert!(far_row(&clear) < 64);
    gpu.clean();
}

#[test]
fn an_accent_lost_in_the_fog_goes_to_the_ramp() {
    let Some(mut gpu) = gpu((64, 64)) else {
        println!("no GPU adapter: skipped");
        return;
    };
    // A card of accent colour straight ahead, near, then far: near it is the
    // accent; past the fog's midpoint it is a ramp colour.
    for (z, accent_expected) in [(-3.0, true), (-30.0, false)] {
        let material = Material { accent: true, ..Material::color([0.94, 0.88, 0.53, 1.0]) };
        let mut card = Node::new(1, MeshRef::Quad, material);
        card.transform = Transform { translation: [0.0, 0.0, z], scale: [z.abs(), z.abs(), 1.0], ..Transform::IDENTITY };
        let camera = Camera {
            transform: Transform::default(),
            projection: Projection::Perspective { fov_y: 60.0_f32.to_radians(), near: 0.1, far: 90.0 },
            clear: Some([0.0, 0.0, 0.0, 1.0]),
        };
        let mut l = look(64);
        l.fog = Some(Fog { color: 0, start: 4.0, end: 40.0 });
        let pixels = gpu.render(&Scene { camera, nodes: vec![card], look: Some(l) });
        let got = at(&pixels, 64, 32, 32);
        println!("accent card at z {z}: centre pixel {got:?}");
        assert_eq!(got == ACCENT, accent_expected, "z {z}: {got:?}");
        if !accent_expected {
            assert!(ramp_set().contains(&got));
        }
    }
    gpu.clean();
}

#[test]
fn a_look_is_enlarged_by_nearest_into_a_bigger_view() {
    let Some(mut gpu) = gpu((128, 128)) else {
        println!("no GPU adapter: skipped");
        return;
    };
    // 32 rows of art pixels into 128 pixels: each one a 4 x 4 block.
    let scene = full_view(&mut gpu, None, Material::default(), Some(look(32)));
    let pixels = gpu.render(&scene);
    let ramp = ramp_set();
    let mut uneven = 0;
    let mut colours = HashSet::new();
    for by in 0..32 {
        for bx in 0..32 {
            let first = at(&pixels, 128, bx * 4, by * 4);
            colours.insert(first);
            assert!(ramp.contains(&first));
            for i in 0..16 {
                uneven += usize::from(at(&pixels, 128, bx * 4 + i % 4, by * 4 + i / 4) != first);
            }
        }
    }
    println!("32 art rows in a 128 px view: {uneven} of 16384 pixels differ from their block, {} ramp colours", colours.len());
    assert_eq!(uneven, 0);
    assert_eq!(colours.len(), 5);
    // Drawn at the look's size, the dither is the look's own: art pixel (x, y)
    // is bayer_threshold(x, y) however big it is shown.
    let direct = {
        let mut small = gpu_small();
        let scene = full_view(&mut small, None, Material::default(), Some(look(32)));
        small.render(&scene)
    };
    for y in 0..32 {
        for x in 0..32 {
            assert_eq!(at(&pixels, 128, x * 4 + 1, y * 4 + 2), at(&direct, 32, x, y), "({x},{y})");
        }
    }
    gpu.clean();
}

fn gpu_small() -> Gpu {
    gpu((32, 32)).expect("an adapter, as the caller found")
}

#[test]
fn a_scene_with_a_look_shown_by_a_layer_is_a_picture_of_its_internal_size() {
    let Some(mut gpu) = gpu((96, 64)) else {
        println!("no GPU adapter: skipped");
        return;
    };
    // Shown 96 x 64, drawn at 32 rows: 48 x 32, enlarged 2x by the layer.
    let scene = full_view(&mut gpu, None, Material::default(), Some(look(32)));
    gpu.core.set_scene_target(60, Some((scene, (96, 64))));
    gpu.core.begin_frame();
    gpu.core.render_scenes();
    let info = gpu.core.use_picture(60).expect("the scene's picture");
    println!("picture 60: {}x{} (shown at 96x64)", info.width, info.height);
    assert_eq!((info.width, info.height), (48, 32));

    let layer = Layer::new(5, 0, Placement::Pixels([0.0, 0.0, 96.0, 64.0]), LayerKind::Picture(60));
    let mut parts = gpu.core.layer_parts([&layer].into_iter(), (0.0, 0.0), 8.0, 16.0, None, 0.0);
    let mut frame = Frame::default();
    frame.assemble_into(&mut parts, &mut Vec::new());
    gpu.core.draw(&gpu.view, gpu.size, wgpu::Color::BLACK, &frame, "look-test-layer");
    let pixels = gpu.core.read_back(&gpu.texture, gpu.size);
    let ramp = ramp_set();
    let (mut off_ramp, mut uneven) = (0, 0);
    for by in 0..32 {
        for bx in 0..48 {
            let first = at(&pixels, 96, bx * 2, by * 2);
            off_ramp += usize::from(!ramp.contains(&first));
            for i in 0..4 {
                uneven += usize::from(at(&pixels, 96, bx * 2 + i % 2, by * 2 + i / 2) != first);
            }
        }
    }
    println!("through the layer: {off_ramp} off-ramp art pixels, {uneven} of 6144 pixels unlike their 2x2 block");
    assert_eq!((off_ramp, uneven), (0, 0));
    gpu.core.set_scene_target(60, None);
    assert!(!gpu.core.has_picture(60));
    gpu.clean();
}

#[test]
fn a_scene_without_a_look_is_drawn_as_before_and_a_bad_look_is_none() {
    let Some(mut gpu) = gpu((64, 64)) else {
        println!("no GPU adapter: skipped");
        return;
    };
    let plain = full_view(&mut gpu, None, Material::default(), None);
    let pixels = gpu.render(&plain);
    // The smooth gradient: many distinct colours, not five.
    let distinct: HashSet<[u8; 3]> = pixels.chunks(4).map(|p| [p[0], p[1], p[2]]).collect();
    println!("no look: {} distinct colours in the gradient", distinct.len());
    assert!(distinct.len() > 60);
    // A look with one colour cannot dither: the scene is drawn plainly.
    let mut one = look(64);
    one.ramp.truncate(1);
    let scene = full_view(&mut gpu, None, Material::default(), Some(one));
    let same = gpu.render(&scene);
    assert_eq!(same, pixels);
    gpu.clean();
}
