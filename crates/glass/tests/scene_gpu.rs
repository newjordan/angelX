//! The renderer core's 3D scenes, drawn on a real GPU into a texture and read
//! back (docs/GLASS_CORE.md, step 2.1). No window, no surface: the test makes
//! its own device. Where the machine has no adapter, each test says so and
//! passes, so this runs everywhere and means something where there is a GPU.
//! `cargo test -p angel-glass --test scene_gpu -- --nocapture` prints the
//! pixels checked.

use std::sync::{Arc, Mutex};

use angel_gpu::{Core, Draw, Frame, Instance, Picture};
use angel_gpu::Stage;
use angel_stage::scene::{
    AMBIENT_LIGHT, AlphaMode, Camera, FACE_SHADES, GRAZING, KEY_LIGHT, Material, MeshData, MeshError, MeshRef, Node, Patch,
    Projection, Projector, Props, Sampling, Scene, TextureSource, Transform, mat_mul,
};
use angel_stage::{Layer, LayerKind, Placement, Stand, Tilt};

const SIZE: (u32, u32) = (64, 64);

struct Gpu {
    core: Core,
    device: wgpu::Device,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    errors: Arc<Mutex<Vec<String>>>,
}

/// A device, a core on it and a 64 x 64 target; `None` without an adapter.
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
        label: Some("scene-test"),
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
        label: Some("scene-test-target"),
        size: wgpu::Extent3d { width: SIZE.0, height: SIZE.1, depth_or_array_layers: 1 },
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
    /// A one-colour picture, `w` x `h`.
    fn solid(&mut self, id: u32, rgba: [u8; 4]) {
        self.pixels(id, 1, 1, rgba.to_vec());
    }

    fn pixels(&mut self, id: u32, width: u32, height: u32, rgba: Vec<u8>) {
        self.core.upload(Picture { id, width, height, rgba, cols: 0, rows: 0 });
    }

    /// A picture whose colours are already multiplied by its alpha.
    fn premultiplied_pixels(&mut self, id: u32, width: u32, height: u32, rgba: Vec<u8>) {
        self.core.upload_premultiplied(Picture { id, width, height, rgba, cols: 0, rows: 0 });
    }

    /// Draw `scene` into the target and read it back.
    fn render(&mut self, scene: &Scene) -> Vec<u8> {
        self.core.begin_frame();
        self.core.draw_scene(&self.view, SIZE, scene, "scene-test");
        self.core.read_back(&self.texture, SIZE)
    }

    /// No GPU error was reported.
    fn clean(&self) {
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(5)),
        });
        let errors = self.errors.lock().unwrap();
        assert!(errors.is_empty(), "GPU errors: {errors:#?}");
    }
}

fn at(pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * SIZE.0 + x) * 4) as usize;
    [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
}

/// Within `slack` levels of 255 on every channel.
fn close(got: [u8; 4], want: [u8; 4], slack: u8) -> bool {
    got.iter().zip(want).all(|(&g, w)| g.abs_diff(w) <= slack)
}

fn expect(what: &str, got: [u8; 4], want: [u8; 4], slack: u8) {
    println!("{what}: got {got:?}, want {want:?}");
    assert!(close(got, want, slack), "{what}: got {got:?}, want {want:?}");
}

/// Looking down -Z from z = 5 at a box 2 units tall: one unit is a quarter
/// of the target, the unit cube at the origin its middle half.
fn ortho() -> Camera {
    Camera {
        transform: Transform::from_translation([0.0, 0.0, 5.0]),
        projection: Projection::Orthographic { height: 2.0, near: 0.1, far: 20.0 },
        clear: Some([0.0, 0.0, 0.0, 1.0]),
    }
}

fn node_at(id: u64, mesh: MeshRef, material: Material, at: [f32; 3], scale: f32) -> Node {
    let mut node = Node::new(id, mesh, material);
    node.transform = Transform { translation: at, scale: [scale; 3], ..Transform::IDENTITY };
    node
}

fn rgba(r: u8, g: u8, b: u8) -> [u8; 4] {
    [r, g, b, 255]
}

const FACE_COLOURS: [[u8; 4]; 6] = [
    [255, 0, 0, 255],   // +X red
    [0, 255, 255, 255], // -X cyan
    [0, 255, 0, 255],   // +Y green
    [255, 0, 255, 255], // -Y magenta
    [0, 0, 255, 255],   // +Z blue
    [255, 255, 0, 255], // -Z yellow
];
const FACE_NAMES: [&str; 6] = ["+X", "-X", "+Y", "-Y", "+Z", "-Z"];

/// A cube with each face its own flat picture (ids 100..106).
fn face_cube(gpu: &mut Gpu, turn: Transform) -> Scene {
    for (i, colour) in FACE_COLOURS.iter().enumerate() {
        gpu.solid(100 + i as u32, *colour);
    }
    let mut cube = Node::new(1, MeshRef::Cube, Material::default());
    cube.transform = turn;
    cube.face_materials = (0..6).map(|i| Material::picture(100 + i)).collect();
    Scene { camera: ortho(), nodes: vec![cube], look: None }
}

#[test]
fn a_cube_in_front_of_a_plane_has_its_own_colour_because_depth_works() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    let red = Material::color([1.0, 0.0, 0.0, 1.0]);
    let green = Material::color([0.0, 1.0, 0.0, 1.0]);
    // The default camera: z = 3, 45 degrees, cleared to opaque black. The
    // plane is listed AFTER the cube, so painter's order would bury the cube.
    let camera = Camera { clear: Some([0.0, 0.0, 0.0, 1.0]), ..Camera::default() };
    let scene = Scene {
        camera,
        nodes: vec![
            Node::new(1, MeshRef::Cube, red),
            node_at(2, MeshRef::Quad, green, [0.0, 0.0, -2.0], 12.0),
        ],
        look: None,
    };
    let pixels = gpu.render(&scene);
    expect("cube centre (32, 32)", at(&pixels, 32, 32), rgba(255, 0, 0), 0);
    expect("plane beside the cube (4, 32)", at(&pixels, 4, 32), rgba(0, 255, 0), 0);
    expect("plane in the corner (60, 3)", at(&pixels, 60, 3), rgba(0, 255, 0), 0);

    // The same with a blended sheet behind the cube: opaque draws first and
    // write depth, so the sheet (listed first, drawn last) does not tint it.
    let sheet = Material::color([0.0, 0.0, 1.0, 0.5]);
    let scene = Scene {
        camera,
        nodes: vec![
            node_at(3, MeshRef::Quad, sheet, [0.0, 0.0, -1.0], 12.0),
            Node::new(1, MeshRef::Cube, red),
        ],
        look: None,
    };
    let pixels = gpu.render(&scene);
    expect("cube with a sheet behind it", at(&pixels, 32, 32), rgba(255, 0, 0), 0);
    expect("sheet beside the cube", at(&pixels, 4, 32), [0, 0, 128, 255], 1);
    gpu.clean();
}

#[test]
fn a_cube_turned_a_half_turn_shows_the_opposite_face() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    let front = face_cube(&mut gpu, Transform::IDENTITY);
    let pixels = gpu.render(&front);
    expect("unturned: the +Z face", at(&pixels, 32, 32), FACE_COLOURS[4], 0);
    // The cube's box fills the middle half of the target; outside it is the clear colour.
    expect("beside the cube", at(&pixels, 4, 32), rgba(0, 0, 0), 0);

    let back = face_cube(&mut gpu, Transform::from_rotation_axis_angle([0.0, 1.0, 0.0], std::f32::consts::PI));
    let pixels = gpu.render(&back);
    expect("turned 180 degrees about Y: the -Z face", at(&pixels, 32, 32), FACE_COLOURS[5], 0);
    gpu.clean();
}

#[test]
fn every_face_can_be_brought_to_the_front_and_shows_its_own_picture() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    use std::f32::consts::FRAC_PI_2 as quarter;
    // The turn that takes each face's outward normal to +Z, toward the camera.
    let turns = [
        Transform::from_rotation_axis_angle([0.0, 1.0, 0.0], -quarter), // +X
        Transform::from_rotation_axis_angle([0.0, 1.0, 0.0], quarter),  // -X
        Transform::from_rotation_axis_angle([1.0, 0.0, 0.0], quarter),  // +Y
        Transform::from_rotation_axis_angle([1.0, 0.0, 0.0], -quarter), // -Y
        Transform::IDENTITY,                                            // +Z
        Transform::from_rotation_axis_angle([0.0, 1.0, 0.0], 2.0 * quarter), // -Z
    ];
    for (i, turn) in turns.into_iter().enumerate() {
        let scene = face_cube(&mut gpu, turn);
        let pixels = gpu.render(&scene);
        expect(&format!("face {} to the front", FACE_NAMES[i]), at(&pixels, 32, 32), FACE_COLOURS[i], 0);
    }
    gpu.clean();
}

#[test]
fn a_face_picture_is_upright_and_not_mirrored_from_outside() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    // Quadrants: top left red, top right green, bottom left blue, bottom right yellow.
    let quads: Vec<u8> = [
        [255, 0, 0, 255], [0, 255, 0, 255],
        [0, 0, 255, 255], [255, 255, 0, 255],
    ]
    .concat();
    gpu.pixels(200, 2, 2, quads);
    for (name, face, turn) in [
        ("+Z face, seen from the front", 4usize, Transform::IDENTITY),
        ("-Z face, turned to the front", 5, Transform::from_rotation_axis_angle([0.0, 1.0, 0.0], std::f32::consts::PI)),
    ] {
        let mut cube = Node::new(1, MeshRef::Cube, Material::color([0.5, 0.5, 0.5, 1.0]));
        cube.transform = turn;
        cube.face_materials = vec![Material::default(); 6];
        cube.face_materials[face] = Material::picture(200);
        let scene = Scene { camera: ortho(), nodes: vec![cube], look: None };
        let pixels = gpu.render(&scene);
        expect(&format!("{name}, top left"), at(&pixels, 24, 24), rgba(255, 0, 0), 0);
        expect(&format!("{name}, top right"), at(&pixels, 40, 24), rgba(0, 255, 0), 0);
        expect(&format!("{name}, bottom left"), at(&pixels, 24, 40), rgba(0, 0, 255), 0);
        expect(&format!("{name}, bottom right"), at(&pixels, 40, 40), rgba(255, 255, 0), 0);
    }
    gpu.clean();
}

#[test]
fn a_blended_quad_over_an_opaque_one_mixes_as_premultiplied_blending_predicts() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    let backdrop = node_at(1, MeshRef::Quad, Material::color([1.0, 0.0, 0.0, 1.0]), [0.0, 0.0, 0.0], 4.0);
    // Straight alpha: blue at half cover over red = (0.5, 0, 0.5, 1).
    let straight = node_at(2, MeshRef::Quad, Material::color([0.0, 0.0, 1.0, 0.5]), [0.0, 0.0, 1.0], 1.0);
    let scene = Scene { camera: ortho(), nodes: vec![backdrop.clone(), straight], look: None };
    let pixels = gpu.render(&scene);
    expect("straight blend, middle", at(&pixels, 32, 32), [128, 0, 128, 255], 1);
    expect("straight blend, outside", at(&pixels, 4, 4), rgba(255, 0, 0), 0);

    // The same cover as a premultiplied picture: (0, 0, 0.5, 0.5).
    gpu.premultiplied_pixels(300, 1, 1, vec![0, 0, 128, 128]);
    let material = Material {
        source: TextureSource::Picture(300),
        alpha: AlphaMode::Premultiplied,
        ..Material::default()
    };
    let premultiplied = node_at(2, MeshRef::Quad, material, [0.0, 0.0, 1.0], 1.0);
    let scene = Scene { camera: ortho(), nodes: vec![backdrop.clone(), premultiplied], look: None };
    let pixels = gpu.render(&scene);
    expect("premultiplied blend, middle", at(&pixels, 32, 32), [127, 0, 128, 255], 1);

    // The texture's own alpha, straight: a picture of (0, 0, 255, 128), tint white.
    gpu.pixels(301, 1, 1, vec![0, 0, 255, 128]);
    let material = Material { source: TextureSource::Picture(301), alpha: AlphaMode::Blend, ..Material::default() };
    let textured = node_at(2, MeshRef::Quad, material, [0.0, 0.0, 1.0], 1.0);
    let scene = Scene { camera: ortho(), nodes: vec![backdrop.clone(), textured], look: None };
    let pixels = gpu.render(&scene);
    expect("straight blend by the texture's alpha, middle", at(&pixels, 32, 32), [127, 0, 128, 255], 1);

    // Opaque ignores the texture's alpha.
    let material = Material { source: TextureSource::Picture(301), alpha: AlphaMode::Opaque, ..Material::default() };
    let ignoring = node_at(2, MeshRef::Quad, material, [0.0, 0.0, 1.0], 1.0);
    let scene = Scene { camera: ortho(), nodes: vec![backdrop, ignoring], look: None };
    let pixels = gpu.render(&scene);
    expect("opaque, middle", at(&pixels, 32, 32), rgba(0, 0, 255), 0);
    gpu.clean();
}

#[test]
fn blended_quads_draw_farthest_first_whatever_their_order() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    let far = node_at(1, MeshRef::Quad, Material::color([0.0, 1.0, 0.0, 0.5]), [0.0, 0.0, -1.0], 1.0);
    let near = node_at(2, MeshRef::Quad, Material::color([0.0, 0.0, 1.0, 0.5]), [0.0, 0.0, 1.0], 1.0);
    // Green far, blue near, over opaque black: green (0, .5, 0), then blue over it (0, .25, .5).
    for (name, nodes) in [
        ("far listed first", vec![far.clone(), near.clone()]),
        ("near listed first", vec![near, far]),
    ] {
        let pixels = gpu.render(&Scene { camera: ortho(), nodes, look: None });
        expect(name, at(&pixels, 32, 32), [0, 64, 128, 255], 1);
    }
    gpu.clean();
}

#[test]
fn a_back_face_is_culled_unless_the_material_is_double_sided() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    let turned = |double_sided: bool| {
        let material = Material { double_sided, ..Material::color([1.0, 1.0, 1.0, 1.0]) };
        let mut quad = Node::new(1, MeshRef::Quad, material);
        quad.transform = Transform::from_rotation_axis_angle([0.0, 1.0, 0.0], std::f32::consts::PI);
        Scene { camera: ortho(), nodes: vec![quad], look: None }
    };
    let pixels = gpu.render(&turned(false));
    expect("seen from behind, one sided", at(&pixels, 32, 32), rgba(0, 0, 0), 0);
    let pixels = gpu.render(&turned(true));
    expect("seen from behind, double sided", at(&pixels, 32, 32), rgba(255, 255, 255), 0);
    // Facing the camera it is there either way.
    let mut facing = turned(false);
    facing.nodes[0].transform = Transform::IDENTITY;
    let pixels = gpu.render(&facing);
    expect("facing the camera", at(&pixels, 32, 32), rgba(255, 255, 255), 0);
    gpu.clean();
}

#[test]
fn the_built_in_shapes_and_a_parent_chain_draw() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    // A sphere and a plane (seen edge on from the front, so not at all), under a parent that moves them.
    let mut group = Node::empty(1);
    group.transform = Transform::from_translation([0.5, 0.0, 0.0]);
    let mut ball = node_at(2, MeshRef::Sphere { segments: 24 }, Material::color([1.0, 0.5, 0.0, 1.0]), [0.0; 3], 1.0);
    ball.parent = Some(1);
    let mut flat = node_at(3, MeshRef::Plane, Material::color([1.0, 1.0, 1.0, 1.0]), [0.0; 3], 1.0);
    flat.parent = Some(1);
    // A node whose parent is missing is simply not drawn.
    let mut lost = node_at(4, MeshRef::Quad, Material::color([1.0, 0.0, 1.0, 1.0]), [0.0; 3], 8.0);
    lost.parent = Some(77);
    let pixels = gpu.render(&Scene { camera: ortho(), nodes: vec![group, ball, flat, lost], look: None });
    // The sphere is a unit across, moved half a unit right: its middle is at x = 0.5, a quarter of the target right of centre.
    expect("sphere middle (48, 32)", at(&pixels, 48, 32), [255, 128, 0, 255], 1);
    expect("left of the sphere (8, 32)", at(&pixels, 8, 32), rgba(0, 0, 0), 0);
    gpu.clean();
}

#[test]
fn an_uploaded_mesh_draws_in_groups_and_oversize_is_refused() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    // Two quads side by side: left (-1..0) and right (0..1), two groups.
    let quad = |x0: f32, x1: f32| {
        [[x0, -0.5, 0.0], [x1, -0.5, 0.0], [x1, 0.5, 0.0], [x0, 0.5, 0.0]]
    };
    let mut mesh = MeshData::default();
    for (x0, x1) in [(-1.0, 0.0), (0.0, 1.0)] {
        let base = mesh.positions.len() as u32;
        mesh.positions.extend(quad(x0, x1));
        mesh.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    mesh.groups = vec![(0, 6), (6, 6)];
    assert_eq!(gpu.core.upload_mesh(9, &mesh), Ok(()));
    assert!(gpu.core.has_mesh(9));
    let mut node = Node::new(1, MeshRef::Uploaded(9), Material::color([1.0, 1.0, 1.0, 1.0]));
    node.face_materials = vec![Material::color([0.0, 0.0, 1.0, 1.0]), Material::color([1.0, 1.0, 0.0, 1.0])];
    let scene = Scene { camera: ortho(), nodes: vec![node.clone()], look: None };
    let pixels = gpu.render(&scene);
    expect("left group", at(&pixels, 20, 32), rgba(0, 0, 255), 0);
    expect("right group", at(&pixels, 44, 32), rgba(255, 255, 0), 0);
    // Past the end of face_materials a group falls back to the node's material.
    node.face_materials.truncate(1);
    let pixels = gpu.render(&Scene { camera: ortho(), nodes: vec![node.clone()], look: None });
    expect("right group with no material of its own", at(&pixels, 44, 32), rgba(255, 255, 255), 0);

    // Over the same id it is replaced (in place, the shapes being the same size).
    let mut moved = mesh.clone();
    moved.positions.iter_mut().for_each(|p| p[1] += 10.0);
    assert_eq!(gpu.core.upload_mesh(9, &moved), Ok(()));
    let pixels = gpu.render(&Scene { camera: ortho(), nodes: vec![node.clone()], look: None });
    expect("replaced mesh moved out of view", at(&pixels, 20, 32), rgba(0, 0, 0), 0);

    // A mesh that is not there draws nothing; refused ones are not kept.
    let gone = Node::new(2, MeshRef::Uploaded(1234), Material::color([1.0; 4]));
    let pixels = gpu.render(&Scene { camera: ortho(), nodes: vec![gone], look: None });
    expect("a mesh that was never uploaded", at(&pixels, 32, 32), rgba(0, 0, 0), 0);
    let mut huge = mesh.clone();
    huge.positions = vec![[0.0; 3]; angel_stage::scene::MAX_MESH_VERTICES + 1];
    assert_eq!(gpu.core.upload_mesh(10, &huge), Err(MeshError::TooManyVertices));
    let mut bad = mesh.clone();
    bad.indices[0] = 99;
    assert_eq!(gpu.core.upload_mesh(11, &bad), Err(MeshError::IndexOutOfRange));
    assert!(!gpu.core.has_mesh(10) && !gpu.core.has_mesh(11));
    gpu.core.remove_mesh(9);
    assert!(!gpu.core.has_mesh(9));
    gpu.clean();
}

#[test]
fn uploaded_meshes_stay_under_their_count() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    let tri = MeshData {
        positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        indices: vec![0, 1, 2],
        ..MeshData::default()
    };
    for id in 0..4200u32 {
        gpu.core.begin_frame();
        assert_eq!(gpu.core.upload_mesh(id, &tri), Ok(()));
    }
    let kept = (0..4200u32).filter(|&id| gpu.core.has_mesh(id)).count();
    println!("4200 uploaded, {kept} kept");
    assert!(kept <= 4096 && kept > 4000, "kept {kept}");
    assert!(gpu.core.has_mesh(4199), "the newest stays");
    assert!(!gpu.core.has_mesh(0), "the oldest went");
    gpu.clean();
}

#[test]
fn a_camera_that_cannot_see_and_a_missing_picture_draw_nothing_and_do_no_harm() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    let mut blind = ortho();
    blind.projection = Projection::Perspective { fov_y: 0.0, near: 0.1, far: 10.0 };
    let cube = Node::new(1, MeshRef::Cube, Material::color([1.0; 4]));
    let pixels = gpu.render(&Scene { camera: blind, nodes: vec![cube.clone()], look: None });
    expect("a camera that cannot project: the clear colour", at(&pixels, 32, 32), rgba(0, 0, 0), 0);

    // A picture that is not on the GPU: that part draws nothing.
    let missing = Node::new(1, MeshRef::Quad, Material::picture(999_999));
    let pixels = gpu.render(&Scene { camera: ortho(), nodes: vec![missing], look: None });
    expect("a picture that was never uploaded", at(&pixels, 32, 32), rgba(0, 0, 0), 0);

    // Nothing but NaN.
    let mut nan = Node::new(1, MeshRef::Cube, Material::color([1.0; 4]));
    nan.transform.translation = [f32::NAN, 0.0, 0.0];
    let mut bad_tint = Node::new(2, MeshRef::Quad, Material { tint: [f32::NAN; 4], ..Material::default() });
    bad_tint.transform.scale = [0.5; 3];
    let pixels = gpu.render(&Scene { camera: ortho(), nodes: vec![nan, bad_tint], look: None });
    // The bad tint is made white (a bad number is 1), and the NaN node is not drawn.
    expect("NaN node and NaN tint", at(&pixels, 32, 32), rgba(255, 255, 255), 0);

    // No clear: drawn over what the target holds.
    let mut over = ortho();
    over.clear = None;
    let quad = node_at(1, MeshRef::Quad, Material::color([0.0, 0.0, 1.0, 1.0]), [0.0, 0.0, 0.0], 1.0);
    let scene = Scene { camera: over, nodes: vec![quad], look: None };
    gpu.core.begin_frame();
    gpu.core.draw_scene(&gpu.view, SIZE, &scene, "scene-test");
    let pixels = gpu.core.read_back(&gpu.texture, SIZE);
    expect("drawn over the last frame (blue over white)", at(&pixels, 32, 32), rgba(0, 0, 255), 0);
    gpu.clean();
}

#[test]
fn the_scene_follows_the_targets_size_and_a_resize_keeps_drawing() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    // Draw into a differently sized and shaped target: the depth buffer follows.
    let mut results = Vec::new();
    for size in [(64u32, 64u32), (96, 32), (32, 96), (64, 64), (128, 128)] {
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene-test-resize"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let cube = Node::new(1, MeshRef::Cube, Material::color([1.0, 0.0, 0.0, 1.0]));
        let wall = node_at(2, MeshRef::Quad, Material::color([0.0, 1.0, 0.0, 1.0]), [0.0, 0.0, -2.0], 20.0);
        let camera = Camera { clear: Some([0.0, 0.0, 0.0, 1.0]), ..Camera::default() };
        gpu.core.begin_frame();
        gpu.core.draw_scene(&view, size, &Scene { camera, nodes: vec![cube, wall], look: None }, "scene-test");
        let pixels = gpu.core.read_back(&texture, size);
        let centre = ((size.1 / 2 * size.0 + size.0 / 2) * 4) as usize;
        results.push((size, [pixels[centre], pixels[centre + 1], pixels[centre + 2], pixels[centre + 3]]));
    }
    for (size, centre) in results {
        expect(&format!("centre of a {}x{} target", size.0, size.1), centre, rgba(255, 0, 0), 0);
    }
    gpu.clean();
}

#[test]
fn a_scene_shown_as_a_picture_layer_composites_premultiplied() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    // A scene that is mostly nothing, with a half-covering blue square in the
    // middle, drawn into picture 77 (its clear colour is nothing).
    let mut camera = ortho();
    camera.clear = Some([0.0; 4]);
    let square = node_at(1, MeshRef::Quad, Material::color([0.0, 0.0, 1.0, 0.5]), [0.0; 3], 1.0);
    gpu.core.set_scene_target(77, Some((Scene { camera, nodes: vec![square], look: None }, SIZE)));
    gpu.core.begin_frame();
    gpu.core.render_scenes();
    assert!(gpu.core.has_picture(77), "the scene's texture is a picture");

    // Shown by a layer over a red floor, at the picture's own size.
    let layer = Layer::new(5, 0, Placement::Pixels([0.0, 0.0, 64.0, 64.0]), LayerKind::Picture(77));
    let mut parts = vec![(Draw::Atlas, vec![Instance::solid(0.0, 0.0, 64.0, 64.0, [1.0, 0.0, 0.0, 1.0])])];
    parts.extend(gpu.core.layer_parts([&layer].into_iter(), (0.0, 0.0), 8.0, 16.0, None, 0.0));
    let mut frame = Frame::default();
    frame.assemble_into(&mut parts, &mut Vec::new());
    gpu.core.draw(&gpu.view, SIZE, wgpu::Color::BLACK, &frame, "scene-test-layer");
    let pixels = gpu.core.read_back(&gpu.texture, SIZE);
    // Half-covering blue over red, the same as drawing it straight there; a
    // straight-alpha read of the premultiplied texture would give (127, 0, 64).
    expect("scene layer over red, middle", at(&pixels, 32, 32), [127, 0, 128, 255], 1);
    expect("scene layer over red, outside the square", at(&pixels, 4, 4), rgba(255, 0, 0), 0);

    // A scene may not read its own texture; it just leaves that part out.
    let selfish = node_at(1, MeshRef::Quad, Material::picture(78), [0.0; 3], 1.0);
    gpu.core.set_scene_target(78, Some((Scene { camera: ortho(), nodes: vec![selfish], look: None }, (16, 16))));
    gpu.core.begin_frame();
    gpu.core.render_scenes();

    // Taken down, the picture goes.
    gpu.core.set_scene_target(77, None);
    gpu.core.set_scene_target(78, None);
    assert!(!gpu.core.has_picture(77) && !gpu.core.has_picture(78));
    gpu.clean();
}

#[test]
fn linear_sampling_blends_texels_and_nearest_keeps_them() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    // A 2 x 1 picture, black then white, across the whole target.
    gpu.pixels(400, 2, 1, vec![0, 0, 0, 255, 255, 255, 255, 255]);
    let on = |sampling: Sampling| {
        let material = Material { sampling, ..Material::picture(400) };
        node_at(1, MeshRef::Quad, material, [0.0; 3], 2.0)
    };
    let pixels = gpu.render(&Scene { camera: ortho(), nodes: vec![on(Sampling::Nearest)], look: None });
    expect("nearest, left of the seam", at(&pixels, 30, 32), rgba(0, 0, 0), 0);
    expect("nearest, right of the seam", at(&pixels, 34, 32), rgba(255, 255, 255), 0);
    let pixels = gpu.render(&Scene { camera: ortho(), nodes: vec![on(Sampling::Linear)], look: None });
    let mid = at(&pixels, 32, 32);
    println!("linear, at the seam: {mid:?}");
    assert!(mid[0] > 64 && mid[0] < 192, "blended across the seam, got {mid:?}");
    gpu.clean();
}

// ---- 3D shots (docs/GLASS_CORE.md, "3D shots"): light, gems and cones, props on a stage ----

/// The shade a lit surface with unit `normal` gets, as 0..255 of a white one.
fn lambert_level(normal: [f32; 3], emissive: f32) -> u8 {
    let len = (KEY_LIGHT.iter().map(|v| v * v).sum::<f32>()).sqrt();
    let n = (normal.iter().map(|v| v * v).sum::<f32>()).sqrt();
    let dot = (0..3).map(|k| normal[k] / n * KEY_LIGHT[k] / len).sum::<f32>().max(0.0);
    let shade = AMBIENT_LIGHT + (1.0 - AMBIENT_LIGHT) * dot;
    ((shade + (1.0 - shade) * emissive) * 255.0).round() as u8
}

fn grey(level: u8) -> [u8; 4] {
    rgba(level, level, level)
}

#[test]
fn a_lit_surface_is_shaded_by_the_key_light_and_an_unlit_one_is_not() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    let quad = |turn: f32, material: Material| {
        let mut node = Node::new(1, MeshRef::Quad, Material { double_sided: true, ..material });
        node.transform = Transform::from_rotation_axis_angle([1.0, 0.0, 0.0], turn);
        Scene { camera: ortho(), nodes: vec![node], look: None }
    };
    let white = |emissive: f32| Material::lit([1.0; 4], emissive);
    // Facing the viewer, tipped back toward the light, tipped away from it.
    for (turn, normal) in [
        (0.0f32, [0.0, 0.0, 1.0]),
        (-std::f32::consts::FRAC_PI_4, [0.0, std::f32::consts::FRAC_1_SQRT_2, std::f32::consts::FRAC_1_SQRT_2]),
        (std::f32::consts::FRAC_PI_3, [0.0, -0.866, 0.5]),
    ] {
        let pixels = gpu.render(&quad(turn, white(0.0)));
        let want = lambert_level(normal, 0.0);
        expect(&format!("lit quad turned {turn:.2}"), at(&pixels, 32, 32), grey(want), 2);
    }
    // A quad tipped away from the light is the ambient floor, never black.
    let pixels = gpu.render(&quad(std::f32::consts::FRAC_PI_3, white(0.0)));
    assert_eq!(at(&pixels, 32, 32)[0], (AMBIENT_LIGHT * 255.0).round() as u8);
    // Seen from behind, a double-sided card is lit from the side it shows.
    let mut back = quad(0.0, white(0.0));
    back.nodes[0].transform = Transform::from_rotation_axis_angle([0.0, 1.0, 0.0], std::f32::consts::PI);
    let pixels = gpu.render(&back);
    expect("lit, seen from behind", at(&pixels, 32, 32), grey(lambert_level([0.0, 0.0, 1.0], 0.0)), 2);
    // Emissive takes the colour back from the light: half way, and all the way.
    let pixels = gpu.render(&quad(0.0, white(0.5)));
    expect("half emissive", at(&pixels, 32, 32), grey(lambert_level([0.0, 0.0, 1.0], 0.5)), 2);
    let pixels = gpu.render(&quad(std::f32::consts::FRAC_PI_3, white(1.0)));
    expect("emissive, turned away", at(&pixels, 32, 32), grey(255), 1);
    // Unlit is the colour as it is, whatever the turn; and the tint still multiplies.
    let pixels = gpu.render(&quad(std::f32::consts::FRAC_PI_3, Material::color([1.0; 4])));
    expect("unlit, turned away", at(&pixels, 32, 32), grey(255), 0);
    let tinted = Material::lit([1.0, 0.5, 0.0, 1.0], 0.0);
    let pixels = gpu.render(&quad(0.0, tinted));
    let level = f32::from(lambert_level([0.0, 0.0, 1.0], 0.0));
    expect("a tinted lit quad", at(&pixels, 32, 32), [level as u8, (level * 0.5).round() as u8, 0, 255], 2);
    gpu.clean();
}

#[test]
fn a_stretched_shape_keeps_its_normals_square_to_its_faces() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    // A gem four times as long as wide: the face over the upper right of the
    // middle has the normal (1/4, 1, 1) once stretched, not (4, 1, 1).
    let mut gem = Node::new(1, MeshRef::Gem, Material::lit([1.0; 4], 0.0));
    gem.transform = Transform::from_scale([4.0, 1.0, 1.0]);
    let pixels = gpu.render(&Scene { camera: ortho(), nodes: vec![gem], look: None });
    // Up and right of the middle by 0.3 and 0.1 of a unit (32 pixels a unit).
    expect("stretched gem", at(&pixels, 41, 28), grey(lambert_level([0.25, 1.0, 1.0], 0.0)), 2);
    let naive = lambert_level([4.0, 1.0, 1.0], 0.0);
    assert!(naive.abs_diff(lambert_level([0.25, 1.0, 1.0], 0.0)) > 40, "the two differ enough to tell");
    gpu.clean();
}

#[test]
fn a_gem_and_a_cone_draw_as_their_shapes() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    let red = Material::color([1.0, 0.0, 0.0, 1.0]);
    // A unit gem seen from the front is a diamond: the middle is in, the corner is out.
    let pixels = gpu.render(&Scene { camera: ortho(), nodes: vec![Node::new(1, MeshRef::Gem, red)], look: None });
    expect("gem middle", at(&pixels, 32, 32), rgba(255, 0, 0), 0);
    expect("gem point, up", at(&pixels, 32, 18), rgba(255, 0, 0), 0);
    expect("gem corner", at(&pixels, 19, 19), rgba(0, 0, 0), 0);
    // A cone stood on its base, point up: narrow at the top, wide at the foot.
    let pixels = gpu.render(&Scene { camera: ortho(), nodes: vec![Node::new(1, MeshRef::Cone { segments: 12 }, red)], look: None });
    expect("cone middle", at(&pixels, 32, 32), rgba(255, 0, 0), 0);
    expect("cone, near its point", at(&pixels, 32, 18), rgba(255, 0, 0), 0);
    expect("cone, beside its point", at(&pixels, 26, 18), rgba(0, 0, 0), 0);
    expect("cone, along its foot", at(&pixels, 20, 44), rgba(255, 0, 0), 0);
    // Turned onto its side it points along +X, as an arrowhead is stood.
    let mut head = Node::new(1, MeshRef::Cone { segments: 8 }, red);
    head.transform = Transform::from_rotation_axis_angle([0.0, 0.0, 1.0], -std::f32::consts::FRAC_PI_2);
    let pixels = gpu.render(&Scene { camera: ortho(), nodes: vec![head], look: None });
    expect("pointing right: near the point", at(&pixels, 46, 32), rgba(255, 0, 0), 0);
    expect("pointing right: above the point", at(&pixels, 46, 24), rgba(0, 0, 0), 0);
    expect("pointing right: along the base", at(&pixels, 18, 24), rgba(255, 0, 0), 0);
    gpu.clean();
}

/// A stage `size` pixels, shown 1:1 (a cell is a pixel), the way the window
/// shows one; drawn and read back.
fn stage_pixels(gpu: &mut Gpu, stage: Stage, scale: u32) -> (Vec<u8>, (u32, u32)) {
    let (id, size) = (stage.id, (stage.size.0 * scale, stage.size.1 * scale));
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("props-test-target"),
        size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    gpu.core.set_stage(id, Some(Stage { cols: size.0 as u16, rows: size.1 as u16, ..stage }));
    gpu.core.begin_frame();
    gpu.core.render_stages((1, 1), 0.0);
    let shown = Instance {
        rect: [0.0, 0.0, size.0 as f32, size.1 as f32],
        uv: [0.0, 0.0, size.0 as f32, size.1 as f32],
        color: [1.0; 4],
        kind: angel_gpu::PICTURE,
        ..bytemuck::Zeroable::zeroed()
    };
    let mut parts = vec![(Draw::Picture { id, linear: false }, vec![shown])];
    let mut frame = Frame::default();
    frame.assemble_into(&mut parts, &mut Vec::new());
    gpu.core.draw(&view, size, wgpu::Color::BLACK, &frame, "props-test");
    (gpu.core.read_back(&texture, size), size)
}

fn pixel(pixels: &[u8], size: (u32, u32), x: u32, y: u32) -> [u8; 4] {
    let i = ((y * size.0 + x) * 4) as usize;
    [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
}

/// The middle of the pixels `lit` picks out, and how many there are.
fn centroid(pixels: &[u8], size: (u32, u32), lit: impl Fn([u8; 4]) -> bool) -> Option<([f32; 2], usize)> {
    let (mut sum, mut n) = ([0.0f64; 2], 0usize);
    for y in 0..size.1 {
        for x in 0..size.0 {
            if lit(pixel(pixels, size, x, y)) {
                sum[0] += f64::from(x) + 0.5;
                sum[1] += f64::from(y) + 0.5;
                n += 1;
            }
        }
    }
    (n > 0).then(|| ([(sum[0] / n as f64) as f32, (sum[1] / n as f64) as f32], n))
}

fn a_stage(size: (u32, u32), tilt: f32, layers: Vec<Layer>, props: Option<Props>) -> Stage {
    Stage { id: 0x00FA_57A4, size, cols: 1, rows: 1, layers, scene: 1, enter: None, tilt, props, impact: 0.0 }
}

#[test]
fn a_prop_lands_on_the_pixels_an_upright_layer_lifted_to_it_does() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    let size = (192u32, 128u32);
    let is = |want: [u8; 4]| move |p: [u8; 4]| p[0].abs_diff(want[0]) < 40 && p[1].abs_diff(want[1]) < 40 && p[2].abs_diff(want[2]) < 40 && p[3] > 200;
    for tilt in [0.0f32, 0.3] {
        for (foot, height) in [([96.0f32, 64.0f32], 0.0f32), ([60.0, 40.0], 9.0), ([150.0, 100.0], 4.0), ([30.0, 110.0], 20.0), ([100.0, 20.0], 6.0)] {
            // The layer's way: a white square standing at the foot, lifted.
            let mut square = Layer::new(
                1,
                0,
                Placement::Pixels([foot[0] - 4.0, foot[1] - height - 4.0, 8.0, 8.0]),
                LayerKind::Solid([1.0, 1.0, 1.0, 1.0]),
            );
            square.stand = Stand::Upright(foot);
            let (layered, _) = stage_pixels(&mut gpu, a_stage(size, tilt, vec![square], None), 1);
            // The prop's way: a red card facing the viewer, the same size, at
            // the same floor point and height. (A sphere there would show as
            // taller than wide: the stage's projection is oblique, as its
            // uprights are.)
            let mut card = Node::new(1, MeshRef::Quad, Material::color([1.0, 0.0, 0.0, 1.0]));
            card.transform = Transform { translation: [foot[0], height, foot[1]], scale: [8.0, 8.0, 1.0], ..Transform::IDENTITY };
            let props = Props { at: 0, grain: 1, nodes: vec![card], world: None };
            let (propped, _) = stage_pixels(&mut gpu, a_stage(size, tilt, vec![], Some(props)), 1);
            let (a, na) = centroid(&layered, size, is([255, 255, 255, 255])).expect("the square");
            let (b, nb) = centroid(&propped, size, is([255, 0, 0, 255])).expect("the ball");
            println!("tilt {tilt} foot {foot:?} lifted {height}: layer ({:.2}, {:.2}) {na}px, prop ({:.2}, {:.2}) {nb}px", a[0], a[1], b[0], b[1]);
            assert!((a[0] - b[0]).abs() < 0.2 && (a[1] - b[1]).abs() < 0.2, "tilt {tilt} foot {foot:?} lifted {height}: {a:?} against {b:?}");
            // And as big.
            let ratio = nb as f32 / na as f32;
            assert!((0.95..1.05).contains(&ratio), "size {ratio}");
        }
    }
    gpu.clean();
}

#[test]
fn props_lie_between_the_layers_they_were_put_between() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    let size = (96u32, 64u32);
    let solid = |id: u64, rect: [f32; 4], colour: [f32; 4]| Layer::new(id, 0, Placement::Pixels(rect), LayerKind::Solid(colour));
    let mut ball = Node::new(1, MeshRef::Sphere { segments: 16 }, Material::color([1.0, 0.0, 0.0, 1.0]));
    ball.transform = Transform { translation: [48.0, 0.0, 32.0], scale: [24.0; 3], ..Transform::IDENTITY };
    // A green floor under, a blue bar over the left half of the ball.
    let layers = vec![
        solid(1, [0.0, 0.0, 96.0, 64.0], [0.0, 1.0, 0.0, 1.0]),
        solid(2, [0.0, 0.0, 48.0, 64.0], [0.0, 0.0, 1.0, 1.0]),
    ];
    for (at, left, right) in [(1usize, rgba(0, 0, 255), rgba(255, 0, 0)), (2, rgba(255, 0, 0), rgba(255, 0, 0)), (0, rgba(0, 0, 255), rgba(0, 255, 0))] {
        let props = Props { at, grain: 1, nodes: vec![ball.clone()], world: None };
        let (pixels, size) = stage_pixels(&mut gpu, a_stage(size, 0.0, layers.clone(), Some(props)), 1);
        // The sphere is 24 across at (48, 32): its middle is on the bar's edge.
        expect(&format!("at {at}: just left of the middle"), pixel(&pixels, size, 42, 32), left, 2);
        expect(&format!("at {at}: just right of the middle"), pixel(&pixels, size, 54, 32), right, 2);
    }
    // The props go back to the stage after a draw: the same frame twice is the same pixels.
    let props = Props { at: 1, grain: 1, nodes: vec![ball.clone()], world: None };
    let stage = a_stage(size, 0.0, layers.clone(), Some(props));
    let (first, _) = stage_pixels(&mut gpu, stage.clone(), 1);
    let (second, _) = stage_pixels(&mut gpu, stage, 1);
    assert_eq!(first, second);
    // A stage with no props, or with none to show, draws its layers alone.
    for props in [None, Some(Props { at: 1, grain: 1, nodes: vec![], world: None })] {
        let (pixels, size) = stage_pixels(&mut gpu, a_stage(size, 0.0, layers.clone(), props), 1);
        expect("no props", pixel(&pixels, size, 54, 32), rgba(0, 255, 0), 0);
    }
    gpu.clean();
}

#[test]
fn props_have_the_stages_pixel_grain_at_a_grain_of_one() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    // A stage of 48 x 32 shown at 96 x 64, a stage pixel two window pixels.
    let size = (48u32, 32u32);
    let mut ball = Node::new(1, MeshRef::Sphere { segments: 24 }, Material::lit([1.0, 0.6, 0.2, 1.0], 0.0));
    ball.transform = Transform { translation: [24.0, 4.0, 20.0], scale: [11.0; 3], ..Transform::IDENTITY };
    let blocky = |pixels: &[u8], shown: (u32, u32)| {
        // Pixels the stage's pixel grid would put in one block that differ.
        let mut differ = 0;
        for by in 0..shown.1 / 2 {
            for bx in 0..shown.0 / 2 {
                let first = pixel(pixels, shown, bx * 2, by * 2);
                for (dx, dy) in [(1, 0), (0, 1), (1, 1)] {
                    differ += usize::from(pixel(pixels, shown, bx * 2 + dx, by * 2 + dy) != first);
                }
            }
        }
        differ
    };
    let mut results = Vec::new();
    for grain in [1u8, 2, 4] {
        let props = Props { at: 0, grain, nodes: vec![ball.clone()], world: None };
        let (pixels, shown) = stage_pixels(&mut gpu, a_stage(size, 0.3, vec![], Some(props)), 2);
        let lit = centroid(&pixels, shown, |p| p[0] > 60).map_or(0, |c| c.1);
        results.push((grain, blocky(&pixels, shown), lit));
        println!("grain {grain}: {} pixels of the {} lit differ inside their stage pixel", results.last().unwrap().1, lit);
    }
    assert_eq!(results[0].1, 0, "at a grain of one each stage pixel is one flat colour");
    assert!(results[1].1 > 20, "finer than the stage's pixels it is not blocky, got {}", results[1].1);
    assert!(results.iter().all(|r| r.2 > 100), "and it is there");
    gpu.clean();
}

#[test]
fn a_tilt_of_zero_and_one_of_the_floor_both_keep_depth_among_props() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    let size = (96u32, 64u32);
    // Two balls, the nearer one listed first: it must still be in front.
    for tilt in [0.0f32, 0.3] {
        let ball = |id: u64, colour: [f32; 4], z: f32| {
            let mut node = Node::new(id, MeshRef::Sphere { segments: 16 }, Material::color(colour));
            node.transform = Transform { translation: [48.0 + (z - 32.0) * 0.2, 6.0, z], scale: [14.0; 3], ..Transform::IDENTITY };
            node
        };
        let near = ball(1, [1.0, 0.0, 0.0, 1.0], 36.0);
        let far = ball(2, [0.0, 0.0, 1.0, 1.0], 28.0);
        let props = Props { at: 0, grain: 1, nodes: vec![near, far], world: None };
        let (pixels, size) = stage_pixels(&mut gpu, a_stage(size, tilt, vec![], Some(props)), 1);
        let red = centroid(&pixels, size, |p| p[0] > 200 && p[2] < 60).map_or(0, |c| c.1);
        let blue = centroid(&pixels, size, |p| p[2] > 200 && p[0] < 60).map_or(0, |c| c.1);
        println!("tilt {tilt}: {red} red pixels, {blue} blue");
        // They overlap: the near one is whole, the far one has a bite out of it.
        assert!(red > 80 && blue > 40 && red > blue, "tilt {tilt}: red {red} blue {blue}");
    }
    let _ = Tilt::flat((1, 1));
    gpu.clean();
}

// ---- a picture cast from a projector (docs/WORLD-3D.md, "project back") ----

type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn unit(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    [a[0] / l, a[1] / l, a[2] / l]
}

/// An eye at `eye` looking at `target`, +Y up: its right, up and forward,
/// and the view matrix (world to its own space, looking down -Z).
fn look(eye: V3, target: V3) -> (V3, V3, V3, angel_stage::scene::Mat4) {
    let f = unit(sub(target, eye));
    let r = unit(cross(f, [0.0, 1.0, 0.0]));
    let u = cross(r, f);
    let view = [
        [r[0], u[0], -f[0], 0.0],
        [r[1], u[1], -f[1], 0.0],
        [r[2], u[2], -f[2], 0.0],
        [-dot(r, eye), -dot(u, eye), dot(f, eye), 1.0],
    ];
    (r, u, f, view)
}

/// A box by its middle and size.
struct Block {
    centre: V3,
    size: V3,
}

/// What a ray meets first: the surface (0 the floor, then six faces a
/// block), the point and the face's outward normal.
fn first_hit(origin: V3, dir: V3, floor: f32, blocks: &[Block]) -> Option<(usize, V3, V3)> {
    let mut best: Option<(f32, usize, V3)> = None;
    if dir[1] < 0.0 {
        let t = -origin[1] / dir[1];
        let p = [origin[0] + dir[0] * t, 0.0, origin[2] + dir[2] * t];
        if p[0].abs() <= floor && p[2].abs() <= floor {
            best = Some((t, 0, [0.0, 1.0, 0.0]));
        }
    }
    for (b, block) in blocks.iter().enumerate() {
        let (mut t_in, mut t_out, mut axis) = (f32::NEG_INFINITY, f32::INFINITY, 0);
        for k in 0..3 {
            let lo = block.centre[k] - block.size[k] / 2.0;
            let hi = block.centre[k] + block.size[k] / 2.0;
            let (t0, t1) = ((lo - origin[k]) / dir[k], (hi - origin[k]) / dir[k]);
            let (near, far) = (t0.min(t1), t0.max(t1));
            if near > t_in {
                t_in = near;
                axis = k;
            }
            t_out = t_out.min(far);
        }
        if t_in <= t_out && t_in > 0.0 && best.is_none_or(|(t, _, _)| t_in < t) {
            let mut n = [0.0; 3];
            n[axis] = -dir[axis].signum();
            // Faces in CUBE_FACES order: +X, -X, +Y, -Y, +Z, -Z.
            let face = axis * 2 + usize::from(n[axis] < 0.0);
            best = Some((t_in, 1 + b * 6 + face, n));
        }
    }
    best.map(|(t, id, n)| (id, [origin[0] + dir[0] * t, origin[1] + dir[1] * t, origin[2] + dir[2] * t], n))
}

impl Gpu {
    /// Draw `scene` into a target of its own, `size` pixels, and read it back.
    fn render_at(&mut self, scene: &Scene, size: (u32, u32)) -> Vec<u8> {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene-test-sized"),
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
        self.core.draw_scene(&view, size, scene, "scene-test-sized");
        self.core.read_back(&texture, size)
    }
}

const CHECKER: [[u8; 4]; 2] = [[220, 60, 50, 255], [40, 70, 210, 255]];
const CAST_TINT: [f32; 4] = [0.25, 0.75, 0.35, 1.0];

/// How a projected surface the projector does not see is shaded by the way
/// its unit normal `n` faces, as a plate lights a block (`FACE_SHADES`).
fn face_shade(n: V3) -> f32 {
    let [top, front, side] = FACE_SHADES;
    let a = n.map(f32::abs);
    if a[1] >= a[0].max(a[2]) {
        if n[1] > 0.0 { top } else { side }
    } else if a[2] >= a[0] {
        front
    } else {
        side
    }
}

/// `colour` (straight, 0..1) shaded as a face with normal `n` is, as 0..255.
fn shaded(colour: [f32; 4], n: V3) -> [u8; 4] {
    let s = face_shade(n);
    [(colour[0] * s * 255.0).round() as u8, (colour[1] * s * 255.0).round() as u8, (colour[2] * s * 255.0).round() as u8, 255]
}

/// A floor and two blocks, every surface cast `picture` from `projector`.
fn cast_scene(camera: Camera, picture: u32, projector: Projector, floor: f32, blocks: &[Block]) -> Scene {
    let material = Material::projected(picture, projector, CAST_TINT);
    let mut nodes = vec![{
        let mut n = Node::new(1, MeshRef::Plane, material);
        n.transform = Transform::from_scale([floor * 2.0, 1.0, floor * 2.0]);
        n
    }];
    for (i, block) in blocks.iter().enumerate() {
        let mut n = Node::new(2 + i as u64, MeshRef::Cube, material);
        n.transform = Transform { translation: block.centre, scale: block.size, ..Transform::IDENTITY };
        nodes.push(n);
    }
    Scene { camera, nodes, look: None }
}

#[test]
fn a_checker_cast_from_an_oblique_projector_lands_where_it_put_it() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    let size = (128u32, 128u32);
    // The picture: an 8 x 8 checker of two colours.
    let rgba: Vec<u8> = (0..64).flat_map(|i| CHECKER[(i % 8 + i / 8) % 2]).collect();
    gpu.pixels(500, 8, 8, rgba);
    let (floor, blocks) = (7.0, [
        Block { centre: [1.5, 0.5, -1.0], size: [1.6, 1.0, 1.6] },
        Block { centre: [-1.5, 0.75, 1.0], size: [1.2, 1.5, 1.2] },
    ]);
    // The camera: orthographic, from the right, above and in front, so it
    // sees the blocks' +X, +Y and +Z faces.
    let (eye, height) = ([6.0, 7.0, 8.0], 10.0);
    let (right, up, forward, view) = look(eye, [0.0; 3]);
    let ortho = Projection::Orthographic { height, near: 0.1, far: 40.0 }.matrix(1.0).unwrap();
    let camera = Camera {
        transform: Transform::IDENTITY,
        projection: Projection::Custom(mat_mul(&ortho, &view)),
        clear: Some([0.0, 0.0, 0.0, 1.0]),
    };
    // The projector: a narrow pinhole from the left, above and in front. It
    // sees -X, +Y and +Z faces, never +X, and not the whole floor.
    let lamp = [-5.0, 6.0, 4.0];
    let (_, _, _, lamp_view) = look(lamp, [0.5, 0.0, -0.5]);
    let pinhole = Projection::Perspective { fov_y: 50f32.to_radians(), near: 0.5, far: 40.0 }.matrix(1.0).unwrap();
    let projector = Projector { view_projection: mat_mul(&pinhole, &lamp_view) };
    let scene = cast_scene(camera, 500, projector, floor, &blocks);
    let pixels = gpu.render_at(&scene, size);

    let ray = |px: i32, py: i32| {
        let x = (px as f32 + 0.5) / size.0 as f32 * 2.0 - 1.0;
        let y = 1.0 - (py as f32 + 0.5) / size.1 as f32 * 2.0;
        let h = height / 2.0;
        let origin = [0, 1, 2].map(|k| eye[k] + right[k] * x * h + up[k] * y * h);
        first_hit(origin, forward, floor, &blocks)
    };
    let (mut checker, mut on_blocks, mut hidden, mut outside, mut colours) = (0, 0, 0, 0, [0; 2]);
    for py in 1..size.1 as i32 - 1 {
        for px in 1..size.0 as i32 - 1 {
            let Some((id, p, n)) = ray(px, py) else {
                continue;
            };
            // Only the middles of surfaces: every neighbour on the same one.
            let same = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .all(|&(dx, dy)| ray(px + dx, py + dy).is_some_and(|(other, _, _)| other == id));
            if !same {
                continue;
            }
            let facing = dot(n, unit(sub(lamp, p)));
            // How squarely the camera sees it: a face the projector sees
            // much more edge-on than that is not cast on (GRAZING).
            let camera_facing = dot(n, forward).abs();
            let tint = shaded(CAST_TINT, n);
            let want = match projector.uv(p) {
                Some(_) if facing.abs() < 0.05 || (facing - GRAZING * camera_facing).abs() < 0.03 => continue,
                Some(_) if facing < GRAZING * camera_facing => {
                    hidden += 1;
                    tint
                }
                Some(uv) => {
                    let (tu, tv) = (uv[0] * 8.0, uv[1] * 8.0);
                    // Not on a texel's edge, where a rounding picks either.
                    if [tu, tv].iter().any(|t| (t.fract() - 0.5).abs() > 0.42) {
                        continue;
                    }
                    let c = (tu as usize + tv as usize) % 2;
                    colours[c] += 1;
                    checker += 1;
                    on_blocks += usize::from(id != 0);
                    CHECKER[c]
                }
                None => {
                    // Outside the picture: not near its edge either.
                    let m = &projector.view_projection;
                    let row = |r: usize| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r];
                    let (u, v) = ((row(0) / row(3) + 1.0) / 2.0, (1.0 - row(1) / row(3)) / 2.0);
                    if row(3) > 0.0 && [u, v].iter().all(|t| (-0.03..=1.03).contains(t)) {
                        continue;
                    }
                    outside += 1;
                    tint
                }
            };
            let got = pixel(&pixels, size, px as u32, py as u32);
            assert!(close(got, want, 1), "({px}, {py}) on surface {id} at {p:?}: got {got:?}, want {want:?}");
        }
    }
    println!("checker {checker} ({colours:?}, {on_blocks} on the blocks), hidden faces {hidden}, outside {outside}");
    assert!(checker > 600 && colours.iter().all(|&c| c > 200), "the checker lands: {checker} {colours:?}");
    assert!(on_blocks > 60, "on the blocks too: {on_blocks}");
    assert!(hidden > 20, "faces turned away from the projector are their tint, shaded: {hidden}");
    assert!(outside > 400, "what the projector does not reach is the tint: {outside}");
    gpu.clean();
}

#[test]
fn a_surface_the_projector_does_not_see_is_its_patch_shaded_as_a_plate_lights_a_block() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    let size = (128u32, 128u32);
    // The plate: the checker. The patch: 2 x 2 colours, none of the checker's.
    let rgba: Vec<u8> = (0..64).flat_map(|i| CHECKER[(i % 8 + i / 8) % 2]).collect();
    gpu.pixels(600, 8, 8, rgba);
    const PATCH: [[u8; 4]; 4] = [[250, 200, 40, 255], [30, 200, 220, 255], [200, 40, 220, 255], [240, 240, 240, 255]];
    // The patch at (1, 1) of a 4 x 3 picture, as a kind's patch lies in its
    // item's swatches; the rest of it a colour the test never wants.
    const JUNK: [u8; 4] = [255, 0, 0, 255];
    let atlas: Vec<u8> = (0..12)
        .flat_map(|i| match (i % 4, i / 4) {
            (x @ 1..=2, y @ 1..=2) => PATCH[(y - 1) * 2 + (x - 1)],
            _ => JUNK,
        })
        .collect();
    gpu.pixels(601, 4, 3, atlas);
    // A pixel of the patch is an eighth of a unit: it repeats, mirrored, every quarter.
    let texel = 0.125;
    // One box, its foot at the origin, a unit a side.
    let block = Block { centre: [0.0, 0.5, 0.0], size: [1.0, 1.0, 1.0] };
    // The camera: orthographic, from the right, above and in front (it sees
    // the +X, +Y and +Z faces).
    let (eye, height) = ([6.0, 7.0, 8.0], 2.4);
    let (right, up, forward, view) = look(eye, [0.0, 0.5, 0.0]);
    let ortho = Projection::Orthographic { height, near: 0.1, far: 40.0 }.matrix(1.0).unwrap();
    let camera = Camera { transform: Transform::IDENTITY, projection: Projection::Custom(mat_mul(&ortho, &view)), clear: Some([0.0, 0.0, 0.0, 1.0]) };
    // The projector: a pinhole high over its left shoulder. It sees the top
    // squarely, the front (+Z) all but edge-on, and the +X side not at all.
    let lamp = [-2.0, 10.0, 1.2];
    let (_, _, _, lamp_view) = look(lamp, [0.0, 0.5, 0.0]);
    let pinhole = Projection::Perspective { fov_y: 50f32.to_radians(), near: 0.5, far: 40.0 }.matrix(1.0).unwrap();
    let projector = Projector { view_projection: mat_mul(&pinhole, &lamp_view) };
    let ray = |px: u32, py: u32| {
        let x = (px as f32 + 0.5) / size.0 as f32 * 2.0 - 1.0;
        let y = 1.0 - (py as f32 + 0.5) / size.1 as f32 * 2.0;
        let h = height / 2.0;
        let origin = [0, 1, 2].map(|k| eye[k] + right[k] * x * h + up[k] * y * h);
        first_hit(origin, forward, 0.0, std::slice::from_ref(&block))
    };
    // The patch's colour at world point `p` on a face of normal `n`, or
    // `None` near a pixel's edge, where a rounding picks either.
    let patch_at = |p: V3, n: V3| -> Option<[u8; 4]> {
        let a = n.map(f32::abs);
        let across = if a[1] >= a[0].max(a[2]) {
            [p[0], p[2]]
        } else if a[2] >= a[0] {
            [p[0], -p[1]]
        } else {
            [p[2], -p[1]]
        };
        let mut at = [0usize; 2];
        for k in 0..2 {
            let t = across[k] / texel / 2.0;
            let m = 1.0 - ((t * 0.5).rem_euclid(1.0) * 2.0 - 1.0).abs();
            let f = m * 2.0;
            if (f.fract() - 0.5).abs() > 0.4 {
                return None;
            }
            at[k] = (f as usize).min(1);
        }
        Some(PATCH[at[1] * 2 + at[0]])
    };
    let cases: [(&str, Option<u32>, f32); 4] = [
        ("a patch", Some(601), 0.0),
        ("no patch", None, 0.0),
        ("a patch that shines", Some(601), 1.0),
        ("a patch not on the GPU", Some(699), 0.0),
    ];
    for (what, picture, emissive) in cases {
        let material = Material {
            emissive,
            patch: picture.map(|picture| Patch { picture, rect: [1, 1, 2, 2], origin: [0.0; 3], texel }),
            ..Material::projected(600, projector, CAST_TINT)
        };
        let mut node = Node::new(1, MeshRef::Cube, material);
        node.transform = Transform { translation: block.centre, scale: block.size, ..Transform::IDENTITY };
        let pixels = gpu.render_at(&Scene { camera, nodes: vec![node], look: None }, size);
        let (mut cast, mut patched, mut tinted) = (0, 0, 0);
        for py in 1..size.1 - 1 {
            for px in 1..size.0 - 1 {
                let Some((id, p, n)) = ray(px, py) else {
                    continue;
                };
                let same = [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .all(|&(dx, dy)| ray((px as i32 + dx) as u32, (py as i32 + dy) as u32).is_some_and(|(other, _, _)| other == id));
                if !same {
                    continue;
                }
                let got = pixel(&pixels, size, px, py);
                let shade = if emissive > 0.0 { 1.0 } else { face_shade(n) };
                let want = if n[1] > 0.5 {
                    // The top: the plate, as the projector casts it.
                    let uv = projector.uv(p).expect("in the picture");
                    let (tu, tv) = (uv[0] * 8.0, uv[1] * 8.0);
                    if [tu, tv].iter().any(|t| (t.fract() - 0.5).abs() > 0.42) {
                        continue;
                    }
                    cast += 1;
                    CHECKER[(tu as usize + tv as usize) % 2]
                } else if picture == Some(601) {
                    let Some(c) = patch_at(p, n) else {
                        continue;
                    };
                    patched += 1;
                    [0, 1, 2].map(|k| (f32::from(c[k]) * shade).round() as u8).into_iter().chain([255]).collect::<Vec<_>>().try_into().unwrap()
                } else {
                    tinted += 1;
                    let mut t = CAST_TINT;
                    t[..3].iter_mut().for_each(|v| *v *= shade);
                    t.map(|v| (v * 255.0).round() as u8)
                };
                assert!(close(got, want, 1), "{what}: ({px}, {py}) on face {id} at {p:?}, normal {n:?}: got {got:?}, want {want:?}");
            }
        }
        println!("{what}: {cast} cast, {patched} of the patch, {tinted} of the tint");
        assert!(cast > 300, "{what}: the top is the plate: {cast}");
        if picture == Some(601) {
            assert!(patched > 300, "{what}: the front (seen edge-on) and the side (turned away) are the patch: {patched}");
        } else {
            assert!(tinted > 600, "{what}: the front and the side are the tint, shaded: {tinted}");
        }
    }
    gpu.clean();
}

#[test]
fn a_picture_cast_from_the_camera_it_is_drawn_with_comes_back_pixel_for_pixel() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    // A picture with no two neighbours alike, the target's size.
    let size = (96u32, 64u32);
    let rgba: Vec<u8> = (0..size.0 * size.1)
        .flat_map(|i| {
            let (x, y) = (i % size.0, i / size.0);
            [(x * 37 + y * 11) as u8, (x * 5 + y * 53) as u8, (x * y * 7 + 90) as u8, 255]
        })
        .collect();
    gpu.pixels(501, size.0, size.1, rgba.clone());
    // The stage's own camera, which a plate is captured and cast through.
    let camera = Camera { clear: Some([0.0, 0.0, 0.0, 1.0]), ..angel_stage::scene::stage_camera(0.3, (384, 224)) };
    let projector = Projector::of(&camera, size.0 as f32 / size.1 as f32).unwrap();
    let blocks = [
        Block { centre: [120.0, 30.0, 150.0], size: [60.0, 60.0, 40.0] },
        Block { centre: [260.0, 20.0, 90.0], size: [40.0, 40.0, 70.0] },
    ];
    let mut scene = cast_scene(camera, 501, projector, 1.0, &blocks);
    scene.nodes[0].transform = Transform { translation: [192.0, 0.0, 112.0], scale: [384.0, 1.0, 224.0], ..Transform::IDENTITY };
    let pixels = gpu.render_at(&scene, size);
    let (mut same, mut covered) = (0, 0);
    for (got, want) in pixels.chunks(4).zip(rgba.chunks(4)) {
        if got == [0, 0, 0, 255] {
            continue;
        }
        covered += 1;
        same += usize::from(got == want);
    }
    println!("{same} of {covered} covered pixels are the picture's own");
    assert!(covered > (size.0 * size.1) as usize / 2, "the floor and blocks cover the view: {covered}");
    assert_eq!(same, covered, "every pixel drawn is the picture's pixel there");
    gpu.clean();
}

#[test]
fn the_depth_a_scene_leaves_is_read_back() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    // The default camera at z = 3: the cube's front 2.5 away, a plane behind
    // it 5 away, too small to reach the corners.
    let camera = Camera { clear: Some([0.0, 0.0, 0.0, 1.0]), ..Camera::default() };
    let mut scene = Scene {
        camera,
        nodes: vec![
            Node::new(1, MeshRef::Cube, Material::color([1.0, 0.0, 0.0, 1.0])),
            node_at(2, MeshRef::Quad, Material::color([0.0, 1.0, 0.0, 1.0]), [0.0, 0.0, -2.0], 3.0),
        ],
        look: None,
    };
    // A perspective depth: far (d - near) / (d (far - near)).
    let want = |d: f32| 100.0 * (d - 0.1) / (d * 99.9);
    for with_look in [false, true] {
        // A look draws small and quantized; the depth is the scene's own, full size.
        scene.look = with_look.then(|| angel_stage::scene::Look::new((0..16).map(|i| [i * 16; 3]).collect()));
        let depth = gpu.core.scene_depth(&scene, SIZE);
        assert_eq!(depth.len(), (SIZE.0 * SIZE.1) as usize);
        let at = |x: u32, y: u32| depth[(y * SIZE.0 + x) as usize];
        println!("look {with_look}: cube {}, plane {}, corner {}", at(32, 32), at(10, 32), at(1, 1));
        assert!((at(32, 32) - want(2.5)).abs() < 1e-3, "the cube's front");
        assert!((at(10, 32) - want(5.0)).abs() < 1e-3, "the plane beside it");
        assert_eq!(at(1, 1), 1.0, "nothing drawn is the far plane");
    }
    assert!(gpu.core.scene_depth(&scene, (0, 4)).is_empty());
    gpu.clean();
}
