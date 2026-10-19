//! A room drawn in 3D on a stage (`angel_stage::scene::World`,
//! docs/WORLD-3D.md): its blocks and the figures standing among them, drawn
//! with depth on a real GPU and read back. A figure behind a wall is hidden
//! by it, one in front is not, and a figure's pixels land where an upright
//! layer's do. Where the machine has no adapter, each test says so and
//! passes. `cargo test -p angel-glass --test world_gpu -- --nocapture`
//! prints what it counted.

use std::sync::{Arc, Mutex};

use angel_gpu::{Core, Draw, Frame, Instance, Picture, Stage};
use angel_stage::scene::{AlphaMode, Material, MeshRef, Node, Props, Transform, World};
use angel_stage::{Layer, LayerKind, Placement, Stand};

struct Gpu {
    core: Core,
    device: wgpu::Device,
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
        label: Some("world-test"),
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
    let core = Core::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
    Some(Gpu { core, device, errors })
}

impl Gpu {
    fn pixels(&mut self, id: u32, width: u32, height: u32, rgba: Vec<u8>) {
        self.core.upload(Picture { id, width, height, rgba, cols: 0, rows: 0 });
    }

    /// The stage drawn as the window draws one, shown 1:1, and read back.
    fn stage(&mut self, stage: Stage) -> Vec<u8> {
        let (id, size) = (stage.id, stage.size);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("world-test-target"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        self.core.set_stage(id, Some(Stage { cols: size.0 as u16, rows: size.1 as u16, ..stage }));
        self.core.begin_frame();
        self.core.render_stages((1, 1), 0.0);
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
        self.core.draw(&view, size, wgpu::Color::BLACK, &frame, "world-test");
        self.core.read_back(&texture, size)
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

const SIZE: (u32, u32) = (192, 128);
/// The Delve's tilt.
const TILT: f32 = 0.3;
const FIGURE: u32 = 0x8000_7001;

fn a_stage(layers: Vec<Layer>, props: Option<Props>) -> Stage {
    Stage { id: 0x00FA_57A4, size: SIZE, cols: 1, rows: 1, layers, scene: 1, enter: None, tilt: TILT, props, impact: 0.0 }
}

/// How many pixels are near `want`.
fn count(pixels: &[u8], want: [u8; 3]) -> usize {
    pixels
        .chunks_exact(4)
        .filter(|p| (0..3).all(|k| p[k].abs_diff(want[k]) < 24))
        .count()
}

const GREEN: [u8; 3] = [40, 140, 60];
const GREY: [u8; 3] = [150, 150, 150];
const RED: [u8; 3] = [230, 20, 20];

fn unit(rgb: [u8; 3]) -> [f32; 4] {
    [f32::from(rgb[0]) / 255.0, f32::from(rgb[1]) / 255.0, f32::from(rgb[2]) / 255.0, 1.0]
}

/// The ground under the whole stage.
fn ground() -> Node {
    let mut node = Node::new(1, MeshRef::Plane, Material::color(unit(GREEN)));
    node.transform = Transform { translation: [96.0, 0.0, 64.0], scale: [400.0, 1.0, 300.0], ..Transform::IDENTITY };
    node
}

/// A wall across the middle of the stage: its foot at floor row 64, its
/// front face at row 69, 30 pixels high.
fn wall() -> Node {
    let mut node = Node::new(2, MeshRef::Cube, Material::color(unit(GREY)));
    node.transform = Transform { translation: [96.0, 15.0, 64.0], scale: [80.0, 30.0, 10.0], ..Transform::IDENTITY };
    node
}

/// A figure's card: picture [`FIGURE`] standing with its foot at floor point
/// `(x, z)`, `w` by `h` pixels, `below` of them under its foot.
fn card(id: u64, (x, z): (f32, f32), (w, h): (f32, f32), below: f32) -> Node {
    let material = Material { alpha: AlphaMode::Blend, double_sided: true, ..Material::picture(FIGURE) };
    let mut node = Node::new(id, MeshRef::Quad, material);
    node.transform = Transform { translation: [x, h / 2.0 - below, z], scale: [w, h, 1.0], ..Transform::IDENTITY };
    node
}

fn world(blocks: Vec<Node>, figures: Vec<Node>) -> Props {
    Props {
        at: 0,
        grain: 2,
        nodes: Vec::new(),
        world: Some(World { floor: vec![ground()], blocks, at: 0, figures, figures_at: 0, ..World::default() }),
    }
}

#[test]
fn a_figure_behind_a_wall_is_hidden_and_one_in_front_is_not() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    gpu.pixels(FIGURE, 1, 1, vec![RED[0], RED[1], RED[2], 255]);
    let size = (12.0, 16.0);
    // In the open, a figure is all there, wherever it stands.
    let open_far = count(&gpu.stage(a_stage(vec![], Some(world(vec![], vec![card(1, (96.0, 50.0), size, 0.0)])))), RED);
    let open_near = count(&gpu.stage(a_stage(vec![], Some(world(vec![], vec![card(1, (96.0, 84.0), size, 0.0)])))), RED);
    println!("in the open: {open_far} red pixels far, {open_near} near");
    assert!(open_far > 100 && open_near > open_far, "{open_far} {open_near}");
    // Behind the wall (its foot at row 50, the wall's back at 59): hidden.
    let pixels = gpu.stage(a_stage(vec![], Some(world(vec![wall()], vec![card(1, (96.0, 50.0), size, 0.0)]))));
    let behind = count(&pixels, RED);
    println!("behind the wall: {behind} red pixels, {} of wall", count(&pixels, GREY));
    assert_eq!(behind, 0, "the wall hides the figure behind it");
    assert!(count(&pixels, GREY) > 1000, "and the wall is there");
    // In front of it (row 84, the wall's front at 69): all there, over it.
    let pixels = gpu.stage(a_stage(vec![], Some(world(vec![wall()], vec![card(1, (96.0, 84.0), size, 0.0)]))));
    let front = count(&pixels, RED);
    println!("in front of the wall: {front} red pixels");
    assert_eq!(front, open_near, "the wall hides nothing of a figure in front of it");
    // Half behind its end: a figure at the wall's left end shows the half
    // that is past it.
    let pixels = gpu.stage(a_stage(vec![], Some(world(vec![wall()], vec![card(1, (56.0, 50.0), size, 0.0)]))));
    let half = count(&pixels, RED);
    println!("at the wall's end: {half} red pixels");
    assert!(half > open_far / 3 && half < open_far * 2 / 3, "{half} of {open_far}");
    gpu.clean();
}

#[test]
fn a_figure_never_sinks_into_the_floor_it_stands_on() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    gpu.pixels(FIGURE, 1, 1, vec![RED[0], RED[1], RED[2], 255]);
    // A figure whose picture reaches 6 pixels below its foot (its shadow):
    // the floor in front of its foot is nearer, and hides none of it.
    let size = (12.0, 16.0);
    let lifted = count(&gpu.stage(a_stage(vec![], Some(world(vec![], vec![card(1, (96.0, 70.0), size, 0.0)])))), RED);
    let sunk = count(&gpu.stage(a_stage(vec![], Some(world(vec![], vec![card(1, (96.0, 70.0), size, 6.0)])))), RED);
    println!("standing on its foot: {lifted} red pixels; reaching below it: {sunk}");
    // (Within a row of its pixels, which the shift may round either way: the
    // floor hiding what is below the foot would take over a third.)
    assert!(lifted > 150);
    assert!(sunk.abs_diff(lifted) <= lifted / 10, "all of it shows: {sunk} against {lifted}");
    gpu.clean();
}

#[test]
fn a_figure_lands_on_the_pixels_its_upright_layer_does() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    // A sprite of 6 x 9 pixels, every pixel its own colour, transparent at
    // two corners.
    let (w, h) = (6u32, 9u32);
    let mut rgba = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let clear = (x, y) == (0, 0) || (x, y) == (w - 1, h - 1);
            rgba.extend_from_slice(&[(40 + x * 30) as u8, (30 + y * 22) as u8, 200, if clear { 0 } else { 255 }]);
        }
    }
    gpu.pixels(FIGURE, w, h, rgba);
    for (foot, scale) in [([96.0f32, 80.0f32], 2.0f32), ([40.5, 30.25], 3.0), ([150.0, 110.0], 1.0)] {
        let (fw, fh) = (w as f32 * scale, h as f32 * scale);
        // The layer's way: the picture placed over its foot, standing up.
        let mut layer = Layer::new(
            1,
            0,
            Placement::Pixels([foot[0] - fw / 2.0, foot[1] - fh, fw, fh]),
            LayerKind::PicturePart(FIGURE, [0.0, 0.0, w as f32, h as f32]),
        );
        layer.stand = Stand::Upright(foot);
        let black = Layer::new(0, 0, Placement::Pixels([0.0, 0.0, SIZE.0 as f32, SIZE.1 as f32]), LayerKind::Solid([0.0, 0.0, 0.0, 1.0]));
        let layered = gpu.stage(a_stage(vec![black.clone(), layer], None));
        // The world's way: a card at the same foot, the same size, over the
        // same black.
        let props = Props {
            at: 0,
            grain: 2,
            nodes: Vec::new(),
            world: Some(World { figures: vec![card(1, (foot[0], foot[1]), (fw, fh), 0.0)], figures_at: 1, ..World::default() }),
        };
        let carded = gpu.stage(a_stage(vec![black], Some(props)));
        let differ = layered.chunks_exact(4).zip(carded.chunks_exact(4)).filter(|(a, b)| a != b).count();
        let lit = layered.chunks_exact(4).filter(|p| p[2] > 100).count();
        println!("foot {foot:?} at {scale}x: {lit} pixels of sprite, {differ} differ");
        assert!(lit > 0);
        assert!(differ <= lit / 50, "foot {foot:?}: {differ} of {lit} pixels differ");
    }
    gpu.clean();
}

#[test]
fn the_world_and_its_figures_lie_where_they_were_put_among_the_layers() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    gpu.pixels(FIGURE, 1, 1, vec![RED[0], RED[1], RED[2], 255]);
    // A blue veil over the left half of the stage, between the world and
    // its figures: it covers the ground there, never the figure.
    let veil = Layer::new(1, 0, Placement::Pixels([0.0, 0.0, 96.0, 128.0]), LayerKind::Solid([0.0, 0.0, 1.0, 1.0]));
    let props = |figures_at: usize| Props {
        at: 0,
        grain: 2,
        nodes: Vec::new(),
        world: Some(World {
            floor: vec![ground()],
            blocks: vec![],
            at: 0,
            figures: vec![card(1, (96.0, 100.0), (12.0, 16.0), 0.0)],
            figures_at,
            ..World::default()
        }),
    };
    let over = gpu.stage(a_stage(vec![veil.clone()], Some(props(1))));
    let under = gpu.stage(a_stage(vec![veil], Some(props(0))));
    let (red_over, red_under) = (count(&over, RED), count(&under, RED));
    println!("figures over the veil: {red_over} red; under it: {red_under}");
    assert!(red_over > 150, "over the veil, all of the figure");
    assert!(red_under <= red_over * 3 / 5 && red_under >= red_over * 2 / 5, "under it, only its half past the veil: {red_under}");
    assert!(count(&over, GREEN) > 3000 && count(&over, [0, 0, 255]) > 3000, "the ground on the right, the veil on the left");
    // No world, no props: the layers alone.
    let alone = gpu.stage(a_stage(vec![], Some(Props { at: 0, grain: 2, nodes: vec![], world: None })));
    assert_eq!(count(&alone, GREEN), 0);
    gpu.clean();
}

/// A world drawn through a look that lights it (docs/REALM-MONOLITH.md):
/// a figure behind a wall stays hidden until the look keeps it in sight,
/// and then shows through the wall's stippled opening, the depth it is
/// tested against leaving the opening out.
#[test]
fn a_lit_worlds_reveal_shows_a_figure_through_the_wall_before_it() {
    use angel_stage::look::{Look, Reveal, Sun};
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    gpu.pixels(FIGURE, 1, 1, vec![RED[0], RED[1], RED[2], 255]);
    let size = (12.0, 16.0);
    let lit = |reveal: bool| {
        let mut look = Look::new(vec![[10, 10, 14], [40, 44, 60], [90, 96, 120], [170, 176, 200]]);
        look.sun = Some(Sun { toward: [-0.5, 0.7, 0.5], strength: 0.8, ambient: 0.4, shadow: None, rise: 0.0 });
        look.metre = 11.0;
        if reveal {
            look.reveal = Some(Reveal { at: [96.0, 8.0, 50.0], radius: 30.0 });
        }
        let mut wall = wall();
        wall.material = Material::lit([0.5, 0.5, 0.5, 1.0], 0.0);
        let mut ground = ground();
        ground.material = Material::lit([0.3, 0.3, 0.3, 1.0], 0.0);
        let figures = vec![card(1, (96.0, 50.0), size, 0.0)];
        let world = World { floor: vec![ground], blocks: vec![wall], at: 0, figures, figures_at: 0, look: Some(look), ..World::default() };
        Props { at: 0, grain: 2, nodes: Vec::new(), world: Some(world) }
    };
    let hidden = count(&gpu.stage(a_stage(vec![], Some(lit(false)))), RED);
    let shown = count(&gpu.stage(a_stage(vec![], Some(lit(true)))), RED);
    let open = count(&gpu.stage(a_stage(vec![], Some(world(vec![], vec![card(1, (96.0, 50.0), size, 0.0)])))), RED);
    println!("behind a lit wall: {hidden} red pixels; kept in sight: {shown}; in the open: {open}");
    assert_eq!(hidden, 0, "without the reveal the wall hides it");
    assert!(shown > open / 2, "kept in sight, most of it shows through the opening: {shown} of {open}");
    gpu.clean();
}
