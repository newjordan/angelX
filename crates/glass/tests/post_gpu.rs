//! The stages' post pass on a real GPU (crates/gpu/src/post.rs): a light
//! blooms round itself and nowhere else, black stays black, the screen's own
//! marks come through untouched, the pixel art stays whole stage pixels,
//! a hit splits the colours, and off it draws exactly as it did before.
//! Skipped where there is no adapter.

use std::sync::{Arc, Mutex};

use angel_gpu::{Core, Draw, Frame, Instance, Stage};
use angel_stage::{Layer, LayerKind, Placement, Stand};

const STAGE: u32 = 0x00FA_57A4;
/// The stage, in its own pixels, and the window's pixels to one of them.
const SIZE: (u32, u32) = (96, 64);
const SCALE: u32 = 3;

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
        label: Some("post-test"),
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .ok()?;
    let errors = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&errors);
    device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| sink.lock().unwrap().push(error.to_string())));
    let core = Core::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
    Some(Gpu { core, device, errors })
}

impl Gpu {
    /// The stage drawn as a window shows it, on black, and read back.
    fn shot(&mut self, layers: Vec<Layer>, impact: f32) -> Vec<u8> {
        let size = (SIZE.0 * SCALE, SIZE.1 * SCALE);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("post-test-target"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let stage = Stage { id: STAGE, size: SIZE, cols: size.0 as u16, rows: size.1 as u16, layers, scene: 1, enter: None, tilt: 0.0, props: None, impact };
        self.core.set_stage(STAGE, Some(stage));
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
        let mut frame = Frame::default();
        frame.assemble_into(&mut parts, &mut Vec::new());
        self.core.draw(&view, size, wgpu::Color::BLACK, &frame, "post-test");
        self.core.read_back(&texture, size)
    }

    fn clean(&self) {
        let _ = self.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(5)) });
        let errors = self.errors.lock().unwrap();
        assert!(errors.is_empty(), "GPU errors: {errors:#?}");
    }
}

fn at(pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * SIZE.0 * SCALE + x) * 4) as usize;
    [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
}

fn solid(id: u64, rect: [f32; 4], rgba: [f32; 4]) -> Layer {
    Layer::new(id, 0, Placement::Pixels(rect), LayerKind::Solid(rgba))
}

/// A dark room in black, a flame in its own light on the left, a pale
/// knight in the dark on the right, a mark on the screen over the flame.
fn room() -> Vec<Layer> {
    let mut light = Layer::new(4, 0, Placement::Pixels([4.0, 14.0, 36.0, 36.0]), LayerKind::Glow([1.0, 0.55, 0.2, 0.5]));
    light.additive = true;
    let mut mark = solid(9, [20.0, 2.0, 6.0, 3.0], [0.9, 0.75, 0.35, 1.0]);
    mark.stand = Stand::Screen;
    vec![
        solid(1, [8.0, 8.0, 80.0, 48.0], [0.12, 0.11, 0.13, 1.0]),
        // The flame: bright, warm, small.
        solid(2, [20.0, 30.0, 4.0, 4.0], [1.0, 0.85, 0.55, 1.0]),
        // The knight's steel: as bright, pale, in no light.
        solid(3, [70.0, 30.0, 4.0, 4.0], [0.95, 0.97, 1.0, 1.0]),
        light,
        mark,
    ]
}

/// The brightest red-and-green a ring of window pixels `r` from (x, y)
/// holds, beside the thing itself.
fn ring(pixels: &[u8], (x, y): (u32, u32), r: u32) -> u32 {
    [(x - r, y), (x + r, y), (x, y - r), (x, y + r)].into_iter().map(|(x, y)| {
        let p = at(pixels, x, y);
        u32::from(p[0]) + u32::from(p[1])
    }).max().unwrap_or(0)
}

#[test]
fn a_light_blooms_round_itself_and_black_and_the_screen_stay_as_drawn() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    gpu.core.set_post(false);
    let plain = gpu.shot(room(), 0.0);
    gpu.core.set_post(true);
    assert!(gpu.core.post());
    let post = gpu.shot(room(), 0.0);
    // Black (outside the room, in a corner and by its edge) stays black.
    for (x, y) in [(1, 1), (285, 190), (285, 100), (280, 12), (150, 189)] {
        assert_eq!(at(&post, x, y), [0, 0, 0, 255], "black at ({x}, {y})");
    }
    // Round the flame (its middle at stage (22, 32)) the light spreads.
    let flame = (22 * SCALE, 32 * SCALE);
    let near = 5 * SCALE;
    let (was, now) = (ring(&plain, flame, near), ring(&post, flame, near));
    println!("round the flame: {was} -> {now}");
    assert!(now > was + 6, "the flame blooms: {was} -> {now}");
    // The knight's steel, as bright but in the dark, does not.
    let steel = (72 * SCALE, 32 * SCALE);
    let (was, now) = (ring(&plain, steel, near), ring(&post, steel, near));
    println!("round the steel: {was} -> {now}");
    assert!(now <= was + 2, "paint in the dark is not a light: {was} -> {now}");
    // The flame's own heart is not burnt to white.
    let heart = at(&post, flame.0, flame.1);
    assert!(heart[2] < 235, "the flame keeps its colour: {heart:?}");
    // The screen's mark is exactly as drawn, over the bloom.
    for (x, y) in [(21 * SCALE, 3 * SCALE), (25 * SCALE + 2, 4 * SCALE + 2)] {
        assert_eq!(at(&post, x, y), at(&plain, x, y), "the screen's mark at ({x}, {y})");
    }
    gpu.clean();
}

#[test]
fn the_stage_s_own_changes_are_whole_stage_pixels() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    // A room with no light: the vignette, the grain and the dither alone.
    let layers = vec![solid(1, [0.0, 0.0, 96.0, 64.0], [0.3, 0.27, 0.24, 1.0])];
    gpu.core.set_post(true);
    let post = gpu.shot(layers.clone(), 0.0);
    let mut differ = 0;
    for by in 0..SIZE.1 {
        for bx in 0..SIZE.0 {
            let first = at(&post, bx * SCALE, by * SCALE);
            for k in 1..SCALE * SCALE {
                differ += usize::from(at(&post, bx * SCALE + k % SCALE, by * SCALE + k / SCALE) != first);
            }
        }
    }
    assert_eq!(differ, 0, "every stage pixel one colour");
    // The corners darker than the middle, never black.
    let (corner, middle) = (at(&post, 0, 0), at(&post, 144, 96));
    println!("corner {corner:?} middle {middle:?}");
    assert!(corner[0] < middle[0] && corner[0] > middle[0] / 2, "a vignette: {corner:?} against {middle:?}");
    gpu.clean();
}

#[test]
fn a_hit_splits_the_colours_by_whole_pixels_and_off_is_as_before() {
    let Some(mut gpu) = gpu() else {
        println!("no GPU adapter: skipped");
        return;
    };
    // A white bar right of the middle: hit, its blue goes out and its red in.
    let layers = vec![solid(1, [80.0, 20.0, 4.0, 24.0], [1.0, 1.0, 1.0, 1.0])];
    gpu.core.set_post(true);
    let calm = gpu.shot(layers.clone(), 0.0);
    let hit = gpu.shot(layers.clone(), 1.0);
    let (y, inner, outer) = (32 * SCALE, 80 * SCALE - 1, 84 * SCALE);
    assert_eq!(at(&calm, outer, y), [0, 0, 0, 255], "calm, nothing past the bar");
    assert_eq!(at(&calm, inner, y), [0, 0, 0, 255], "nor short of it");
    let (out, ins) = (at(&hit, outer, y), at(&hit, inner, y));
    assert!(out[2] > 200 && out[0] == 0, "hit, its blue past its edge: {out:?}");
    assert!(ins[0] > 200 && ins[2] == 0, "and its red short of it: {ins:?}");
    // Off, the stage is drawn as it was before there was a post pass: no
    // vignette, no split, whatever the impact.
    gpu.core.set_post(false);
    let off = gpu.shot(layers.clone(), 1.0);
    assert_eq!(at(&off, outer, y), [0, 0, 0, 255]);
    assert_eq!(at(&off, inner + 1, y), [255, 255, 255, 255]);
    assert_eq!(off, gpu.shot(layers, 0.0), "the impact is the post pass's alone");
    gpu.clean();
}
