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

/// Warm frames, including the terminal conversion, at the cockpit's real dot
/// sizes. Scene bakes and file output are outside the measured loop.
#[test]
#[ignore]
fn bench_round2_world_frames() {
    use super::{arch::Stage, interior, region};
    use crate::stage::world_viz::ride::frame_to_braille_graded;
    let mut scenes = vec![(
        "court".to_string(),
        scene::scene_for(scene::SceneKey::COURT),
        None,
    )];
    for index in 0..8u8 {
        scenes.push((format!("room{index}"), scene::interior_scene(index), None));
    }
    for stage in Stage::ALL {
        scenes.push((
            format!("{stage:?}"),
            scene::region_scene(stage, 0, 0, 0),
            Some(stage),
        ));
    }
    for (w, h) in [(96, 72), (144, 104), (200, 304)] {
        for (name, mesh, stage) in &scenes {
            let view = if let Some(s) = stage {
                region::settled_camera(*s, 0, w, h)
            } else if let Some(index) = name.strip_prefix("room") {
                interior::review_camera(index.parse().unwrap(), w, h)
            } else {
                proving_view()
            };
            if let Some(dir) = std::env::var_os("ANGEL_WORLD3D_FIXTURES") {
                let dir = std::path::PathBuf::from(dir);
                std::fs::create_dir_all(&dir).unwrap();
                let mut data = Vec::new();
                for n in [w as u32, h as u32, mesh.tris.len() as u32] {
                    data.extend(n.to_le_bytes());
                }
                for n in [
                    view.pos.x,
                    view.pos.y,
                    view.pos.z,
                    view.heading_rad,
                    view.pitch,
                    view.fov_rad,
                ] {
                    data.extend(n.to_le_bytes());
                }
                for tri in &mesh.tris {
                    data.push(tri.mat);
                    for p in tri.v {
                        for n in [p.x, p.y, p.z] {
                            data.extend(n.to_le_bytes());
                        }
                    }
                    for uv in tri.uv {
                        for n in uv {
                            data.extend(n.to_le_bytes());
                        }
                    }
                    for n in [tri.normal.x, tri.normal.y, tri.normal.z] {
                        data.extend(n.to_le_bytes());
                    }
                }
                std::fs::write(dir.join(format!("{name}-{w}-{h}.mesh")), data).unwrap();
                continue;
            }
            for _ in 0..3 {
                std::hint::black_box(render_scene(mesh, &view, w, h));
            }
            for trial in 0..5 {
                let mut times = Vec::new();
                for _ in 0..30 {
                    let t = std::time::Instant::now();
                    let frame = render_scene(mesh, &view, w, h);
                    std::hint::black_box(frame_to_braille_graded(&frame, w / 2, h / 4, false));
                    times.push(t.elapsed().as_secs_f64() * 1e3);
                }
                times.sort_by(f64::total_cmp);
                eprintln!(
                    "ROUND2,{name},{w},{h},{trial},{:.6},{:.6}",
                    times.iter().sum::<f64>() / times.len() as f64,
                    times[28]
                );
            }
        }
    }
}

/// Dot-for-dot terminal review of the production braille bridge. Half-block
/// keeps the source RGB; this dump separately shows the actual braille masks.
#[test]
#[ignore]
fn write_world3d_terminal_shots() {
    use crate::stage::world_viz::ride::frame_to_braille_graded;
    use angel_delve::render::braille::braille_bits;
    let Some(root) = std::env::var_os("ANGEL_WORLD3D_SHOTS") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    let out = root.join("braille");
    std::fs::create_dir_all(&out).unwrap();
    for family in ["rooms", "props", "regions"] {
        let Ok(entries) = std::fs::read_dir(root.join(family)) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .ends_with("-mini")
            {
                continue;
            }
            let frame = image::open(&path).unwrap().to_rgba8();
            let (w, h) = (frame.width() as usize / 2, frame.height() as usize / 4);
            let dots = frame_to_braille_graded(&frame, w, h, false);
            let mut image = image::RgbaImage::from_pixel(
                w as u32 * 8,
                h as u32 * 16,
                image::Rgba([0, 0, 0, 255]),
            );
            for cy in 0..h {
                for cx in 0..w {
                    let cell = dots.cell(cx, cy).unwrap();
                    let bits = braille_bits(cell.glyph);
                    for y in 0..4 {
                        for x in 0..2 {
                            if bits & angel_delve::render::braille::braille_dot_bit(x, y) == 0 {
                                continue;
                            }
                            for dy in 0..2 {
                                for dx in 0..2 {
                                    image.put_pixel(
                                        (cx * 8 + x * 4 + 1 + dx) as u32,
                                        (cy * 16 + y * 4 + 1 + dy) as u32,
                                        image::Rgba([cell.fg[0], cell.fg[1], cell.fg[2], 255]),
                                    );
                                }
                            }
                        }
                    }
                }
            }
            image.save(out.join(path.file_name().unwrap())).unwrap();
        }
    }
}
