use super::*;
use super::testing::*;
use crate::render::overworld::{Scene, View, frame_sized, render_view};

#[test]
fn art_is_deterministic_and_motion_preferences_hold_in_both_levels() {
    for room in [Room::Study, Room::Vault] {
        for (w, h) in [(96, 72), (160, 120), (256, 176), (640, 320)] {
            let image = render_room(room, evidence(), 20, Motion::Full, w, h);
            assert_eq!((image.w, image.h), (w, h));
            assert_eq!(
                image,
                render_room(room, evidence(), 20, Motion::Full, w, h)
            );
        }
        for motion in [Motion::Off, Motion::Reduced] {
            let first = render_room(room, evidence(), 0, motion, 256, 176);
            for tick in [1, 24, 333, 100_000] {
                assert_eq!(first, render_room(room, evidence(), tick, motion, 256, 176));
            }
        }
    }
    for resident in residents() {
        for y in 0..resident.h {
            for x in 0..resident.w {
                if let Some(color) = resident.get(x, y) {
                    assert!(
                        !super::super::ink::is_signal(color),
                        "residents cannot impersonate evidence"
                    );
                }
            }
        }
    }
    assert_ne!(
        render_room(Room::Study, evidence(), 0, Motion::Full, 256, 176),
        render_room(Room::Vault, evidence(), 0, Motion::Full, 256, 176)
    );
}

#[test]
fn a_quiet_school_does_not_paint_receipts_or_signal_progress() {
    for room in [Room::Study, Room::Vault] {
        let image = render_room(
            room,
            SchoolSnapshot::default(),
            0,
            Motion::Off,
            256,
            176,
        );
        for y in 0..image.h {
            for x in 0..image.w {
                assert!(!super::super::ink::is_signal(image.get(x, y).unwrap()));
            }
        }
    }
    for (field, changed) in [
        (
            "observations",
            SchoolSnapshot {
                observations: 7,
                ..SchoolSnapshot::default()
            },
        ),
        (
            "checks",
            SchoolSnapshot {
                checks: 3,
                ..SchoolSnapshot::default()
            },
        ),
        (
            "contrasts",
            SchoolSnapshot {
                contrasts: 1,
                ..SchoolSnapshot::default()
            },
        ),
        (
            "inconclusive",
            SchoolSnapshot {
                inconclusive: 1,
                ..SchoolSnapshot::default()
            },
        ),
    ] {
        let mut first = Scene::resting();
        let initial = first.key();
        first.school = changed;
        assert_ne!(
            first.key(),
            initial,
            "{field} must invalidate the rendered frame"
        );
    }
}

#[test]
#[ignore = "writes school grounds and both room levels for visual review"]
fn write_school_review_frames() {
    let Some(directory) = std::env::var_os("ANGEL_SCHOOL_REVIEW") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).unwrap();
    let save = |name: &str, image: Img| {
        image::save_buffer(
            directory.join(name),
            &image.rgba_scaled(4),
            image.w as u32 * 4,
            image.h as u32 * 4,
            image::ColorType::Rgba8,
        )
        .unwrap();
    };
    let mut scene = Scene::resting();
    scene.school = evidence();
    scene.tick = 24;
    save(
        "school-grounds.png",
        render_view(&scene, View::screen(0, 6)),
    );
    scene.school_room = Some(Room::Study);
    save("school-study.png", frame_sized(&scene, 256, 176));
    scene.school_room = Some(Room::Vault);
    save("school-vault.png", frame_sized(&scene, 256, 176));
    scene.school = SchoolSnapshot::default();
    save("school-quiet.png", frame_sized(&scene, 256, 176));
}
