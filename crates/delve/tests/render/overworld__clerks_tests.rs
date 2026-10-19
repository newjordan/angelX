use super::*;
use crate::render::overworld::{Scene, View, map, render_view};

/// The residents as the renderer used to make them on its first frame,
/// from the whole paintings: cropped to their solid pixels, resized to
/// `HEIGHT` nearest-neighbour, put in the plain palette.
fn from_painting(png: &[u8]) -> Img {
    let source = image::load_from_memory(png).expect("bundled town resident PNG").to_rgba8();
    let (mut left, mut top) = source.dimensions();
    let (mut right, mut bottom) = (0, 0);
    for (x, y, pixel) in source.enumerate_pixels() {
        if pixel[3] >= SOLID {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    if right <= left || bottom <= top {
        return Img::new(1, 1);
    }
    let width = ((right - left) * HEIGHT / (bottom - top)).max(1);
    let cropped = image::imageops::crop_imm(&source, left, top, right - left, bottom - top);
    let small = image::imageops::resize(&cropped.to_image(), width, HEIGHT, image::imageops::FilterType::Nearest);
    let mut image = Img::new(width as i32, HEIGHT as i32);
    for (x, y, pixel) in small.enumerate_pixels() {
        if pixel[3] >= SOLID {
            image.set(x as i32, y as i32, nearest_plain([pixel[0] as f32, pixel[1] as f32, pixel[2] as f32]));
        }
    }
    image
}

#[test]
fn the_build_shrinks_the_paintings_to_the_sprites_the_renderer_made() {
    let paintings: [&[u8]; 2] = [
        include_bytes!("../../../../cockpit/assets/realm/inhabitants/archivist.png"),
        include_bytes!("../../../../cockpit/assets/realm/inhabitants/courier.png"),
    ];
    for (poses, painting) in sprites().iter().zip(paintings) {
        assert_eq!(poses[0], from_painting(painting));
    }
}

#[test]
fn residents_are_plain_visible_sprites_and_motion_preferences_hold() {
    let cast = sprites();
    for poses in cast {
        assert!(poses[0].w > 5 && poses[0].w <= 20);
        assert_eq!(poses[0].h, 20);
        let mut opaque = 0;
        for y in 0..poses[0].h {
            for x in 0..poses[0].w {
                if let Some(color) = poses[0].get(x, y) {
                    opaque += 1;
                    assert!(!super::super::ink::is_signal(color));
                }
            }
        }
        assert!(opaque > 50, "approved resident art must be visible");
    }
    for motion in [Motion::Off, Motion::Reduced] {
        let start = stage(0, motion);
        for tick in [1, 17, 95, 100_000] {
            let later = stage(tick, motion);
            for (a, b) in start.iter().zip(later) {
                assert_eq!((a.x, a.base, &a.img), (b.x, b.base, &b.img));
            }
        }
    }
    let a = stage(0, Motion::Full);
    let b = stage(24, Motion::Full);
    assert_eq!(a[0].x, b[0].x);
    assert_ne!(a[1].x, b[1].x);
    for tick in 0..192 {
        let clerk = &stage(tick, Motion::Full)[1];
        let (x, y) = map::place_px(clerk.x + clerk.img.w / 2, clerk.base);
        assert_eq!(
            map::Realm::get().at(x / TILE, y / TILE),
            b'=',
            "courier stays on the town road"
        );
    }
}

#[test]
fn residents_stay_in_town_and_outside_distant_views() {
    // The Keep's set piece, and the Observatory's far off to the north-west.
    let piece = |ax: i32, ay: i32| {
        let (x, y) = map::screen_origin(ax, ay);
        View { x: x * map::TILE, y: y * map::TILE, w: map::SCREEN_W * map::TILE, h: map::SCREEN_H * map::TILE }
    };
    let town = piece(1, 1);
    let distant = piece(0, 0);
    for tick in 0..192 {
        for prop in stage(tick, Motion::Full) {
            let (x, y) = map::place_px(prop.x + prop.img.w / 2, prop.base - 1);
            let (left, top) = (x - prop.img.w / 2, y + 1 - prop.img.h);
            assert!(town.touches(left, top, prop.img.w, prop.img.h));
            assert!(!distant.touches(left, top, prop.img.w, prop.img.h));
        }
    }
}

#[test]
#[ignore = "writes an approved-asset town frame for visual review"]
fn write_clerk_review_frame() {
    let Some(path) = std::env::var_os("ANGEL_CLERK_REVIEW") else {
        return;
    };
    let cold = std::time::Instant::now();
    for clerk in &CLERKS {
        std::hint::black_box(sprite(clerk.rgba));
    }
    let cold = cold.elapsed();
    let _ = sprites();
    let warm = std::time::Instant::now();
    for tick in 0..10_000 {
        std::hint::black_box(stage(tick, Motion::Full));
    }
    eprintln!(
        "clerk diagnostic: uncached two-sprite decode/prep={cold:?}; 10000 warm two-resident stages={:?}",
        warm.elapsed()
    );
    let path = std::path::PathBuf::from(path);
    let save = |path: &std::path::Path, image: Img, scale| {
        image::save_buffer(
            path,
            &image.rgba_scaled(scale),
            image.w as u32 * scale,
            image.h as u32 * scale,
            image::ColorType::Rgba8,
        )
        .unwrap();
    };
    let mut scene = Scene::resting();
    save(&path, render_view(&scene, View::screen(2, 2)), 4);
    scene.tick = 24;
    save(
        &path.with_extension("midwalk.png"),
        render_view(&scene, View::screen(2, 2)),
        4,
    );
    let mut cast = Img::black(60, 28);
    for (index, poses) in sprites().iter().enumerate() {
        cast.stamp(&poses[0], 8 + index as i32 * 28, 4);
    }
    save(&path.with_extension("sprites.png"), cast, 8);
}
