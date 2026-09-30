use super::*;
use crate::stage::world_viz::overworld::{Scene, View, map, render_view};

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
    for motion in [MotionMode::Off, MotionMode::Reduced] {
        let start = stage(0, motion);
        for tick in [1, 17, 95, 100_000] {
            let later = stage(tick, motion);
            for (a, b) in start.iter().zip(later) {
                assert_eq!((a.x, a.base, &a.img), (b.x, b.base, &b.img));
            }
        }
    }
    let a = stage(0, MotionMode::Full);
    let b = stage(24, MotionMode::Full);
    assert_eq!(a[0].x, b[0].x);
    assert_ne!(a[1].x, b[1].x);
    for tick in 0..192 {
        let clerk = &stage(tick, MotionMode::Full)[1];
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
    let town = View::screen(2, 2);
    let distant = View::screen(0, 0);
    for tick in 0..192 {
        for prop in stage(tick, MotionMode::Full) {
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
        std::hint::black_box(sprite(clerk.png));
    }
    let cold = cold.elapsed();
    let _ = sprites();
    let warm = std::time::Instant::now();
    for tick in 0..10_000 {
        std::hint::black_box(stage(tick, MotionMode::Full));
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
