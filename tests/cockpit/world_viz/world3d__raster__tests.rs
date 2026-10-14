//! The raster over the cockpit's own 3D world: its court scene, a large real
//! mesh, proves the frame byte for byte.

use super::math::v3;
use super::mesh::Mesh;
use super::raster::{
    Backdrop, FOG_DISTANCE, Firelight, View3, one_pass, render_meshes, render_scene,
};
use super::scene;
use image::RgbaImage;

fn proving_view() -> View3 {
    View3 {
        pos: v3(0.0, 0.0, 0.62),
        heading_rad: 0.0,
        pitch: 0.0,
        fov_rad: 1.05,
    }
}

fn pixel(frame: &RgbaImage, x: u32, y: u32) -> [u8; 3] {
    let p = frame.get_pixel(x, y).0;
    [p[0], p[1], p[2]]
}

#[test]
fn renders_are_byte_identical_for_identical_inputs() {
    let scene = scene::scene_for(scene::SceneKey::COURT);
    let view = proving_view();
    let first = render_scene(&scene, &view, 96, 72);
    let second = render_scene(&scene, &view, 96, 72);
    assert_eq!(
        first.as_raw(),
        second.as_raw(),
        "identical inputs must produce byte-identical frames"
    );
}

#[test]
fn sky_sits_above_the_ground_plane() {
    let scene = scene::scene_for(scene::SceneKey::COURT);
    let frame = render_scene(&scene, &proving_view(), 96, 72);
    // The night sky is blue-dominant; lit ground (grass/path) is not.
    for row in [0u32, 3, 6] {
        let sky = pixel(&frame, 48, row);
        assert!(
            sky[2] > sky[1],
            "row {row} should be sky, got {sky:?} (needs blue over green)"
        );
    }
    for row in [66u32, 69, 71] {
        let ground = pixel(&frame, 48, row);
        assert!(
            ground[1] >= ground[2],
            "row {row} should be lit ground, got {ground:?}"
        );
    }
}

#[test]
fn a_zero_sized_pane_renders_nothing_instead_of_panicking() {
    let scene = scene::scene_for(scene::SceneKey::COURT);
    let frame = render_scene(&scene, &proving_view(), 0, 0);
    assert_eq!(frame.width(), 0);
    assert_eq!(frame.height(), 0);
}

#[test]
fn banded_frames_are_the_same_bytes_as_one_pass() {
    let scene = scene::scene_for(scene::SceneKey::COURT);
    for (k, (w, h)) in [(96usize, 72usize), (384, 224), (200, 304)].into_iter().enumerate() {
        let view = View3 {
            pos: v3(0.4 * k as f32, -0.3 * k as f32, 0.62),
            heading_rad: 0.9 * k as f32,
            pitch: 0.04 * k as f32,
            fov_rad: 1.05,
        };
        assert_eq!(
            render_scene(&scene, &view, w, h).as_raw(),
            &one_pass(&scene, &view, w, h),
            "{w}x{h}: bands must not change a byte"
        );
    }
}

#[test]
fn several_meshes_render_as_one_merged_mesh_does() {
    let scene = scene::scene_for(scene::SceneKey::COURT);
    let half = scene.tris.len() / 2;
    let (a, b) = (
        Mesh { tris: scene.tris[..half].to_vec() },
        Mesh { tris: scene.tris[half..].to_vec() },
    );
    let view = proving_view();
    let (split, depth) = render_meshes(&[&a, &b], &view, 160, 96, Firelight::STEADY, FOG_DISTANCE, Backdrop::Night);
    assert_eq!(split.as_raw(), render_scene(&scene, &view, 160, 96).as_raw());
    assert_eq!(depth.len(), 160 * 96);
    // A dark backdrop paints the misses its own colour.
    let (dark, _) = render_meshes(&[], &view, 32, 16, Firelight::STEADY, FOG_DISTANCE, Backdrop::Dark([3, 4, 5]));
    assert!(dark.pixels().all(|p| p.0 == [3, 4, 5, 255]));
}

/// `cargo test --release raster_frame_bench -- --ignored --nocapture`: ms
/// per frame for the court at the Delve's two frame sizes.
#[test]
#[ignore]
fn raster_frame_bench() {
    let scene = scene::scene_for(scene::SceneKey::COURT);
    eprintln!("court: {} triangles", scene.tris.len());
    for (w, h) in [(384usize, 224usize), (768, 448)] {
        let view = proving_view();
        let _ = render_scene(&scene, &view, w, h);
        let n = 40;
        let t = std::time::Instant::now();
        for _ in 0..n {
            let _ = render_scene(&scene, &view, w, h);
        }
        eprintln!("{w}x{h}: {:.2} ms a frame", t.elapsed().as_secs_f64() * 1000.0 / n as f64);
    }
}
