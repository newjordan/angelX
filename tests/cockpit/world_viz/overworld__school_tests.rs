use super::*;
use crate::agent::harness::ToolEventId;
use crate::stage::world_viz::World;
use crate::stage::world_viz::overworld::{Scene, View, frame_sized, map, render_view};

fn evidence() -> SchoolSnapshot {
    SchoolSnapshot {
        active: true,
        observations: 12,
        checks: 5,
        contrasts: 2,
        inconclusive: 1,
    }
}

#[test]
fn school_has_a_reachable_front_door_without_relocating_existing_landmarks() {
    let target = Place::School.stand_world();
    assert!(map::walkable(map::Realm::get().at(target.0, target.1)));
    let route = super::super::live::route(Place::Keep.stand_world(), target);
    assert_eq!(route.last(), Some(&target));
    assert_eq!(Place::School.screen(), (0, 6));
    assert_eq!(Place::Scriptorium.stand(), (30, 19));
    assert_eq!(Place::Observatory.stand(), (9, 4));
}

#[test]
fn school_visits_have_depth_and_leave_the_working_knight_alone() {
    let mut world = World::new(7);
    world.note_tool_call_event(
        ToolEventId("school-research-work".into()),
        "write_file",
        "src/main.rs",
    );
    let goal = world.overworld_goal();
    for (name, room) in [
        ("school", None),
        ("school-study", Some(Room::Study)),
        ("school-vault", Some(Room::Vault)),
    ] {
        let knight = world.overworld_scene().knight;
        assert!(world.visit_overworld(name).is_some());
        assert_eq!(world.overworld_scene().school_room, room);
        assert_eq!(world.overworld_scene().knight, knight);
        assert_eq!(world.overworld_goal(), goal);
    }
    let selected = world.overworld_scene();
    assert_eq!(world.visit_overworld("nonexistent-room"), None);
    assert_eq!(world.overworld_scene().school_room, Some(Room::Vault));
    assert_eq!(world.overworld_scene().camera, selected.camera);
    for _ in 0..24 {
        world.tick();
    }
    assert_ne!(world.overworld_scene().knight, selected.knight);
    world.visit_overworld("garden").unwrap();
    assert_eq!(world.overworld_scene().school_room, None);
    world.visit_overworld("school-vault").unwrap();
    world.follow_overworld();
    assert_eq!(world.overworld_scene().school_room, None);
    assert_eq!(world.overworld_view_label(), None);
    world.visit_overworld("school").unwrap();
    assert!(world.enter_interior());
    assert!(world.inside_interior());
    assert_eq!(world.overworld_scene().school_room, Some(Room::Study));
    assert!(world.leave_interior());
    assert!(!world.inside_interior());
    assert_eq!(world.overworld_view_label(), Some(Place::School.label()));
}

#[test]
fn receipts_repaint_the_school_without_awarding_progress_or_changing_research() {
    let mut world = World::new(7);
    world.visit_overworld("school-study").unwrap();
    let before = world.overworld_scene();
    let renown = world.renown;
    let goal = world.overworld_goal();
    let snapshot = evidence();
    world.note_school(snapshot);
    let after = world.overworld_scene();
    assert_ne!(before.key(), after.key());
    assert_ne!(
        frame_sized(&before, 256, 176),
        frame_sized(&after, 256, 176)
    );
    assert_eq!(after.school, snapshot);
    assert_eq!(world.renown, renown);
    assert_eq!(world.overworld_goal(), goal);
    world.note_school(snapshot);
    assert_eq!(
        after.key(),
        world.overworld_scene().key(),
        "replaying a snapshot is idempotent"
    );
    world.note_school(SchoolSnapshot {
        active: false,
        ..snapshot
    });
    assert_eq!(
        world.overworld_scene().school.observations,
        12,
        "ending a turn does not erase evidence"
    );
}

#[test]
fn art_is_deterministic_and_motion_preferences_hold_in_both_levels() {
    for room in [Room::Study, Room::Vault] {
        for (w, h) in [(96, 72), (160, 120), (256, 176), (640, 320)] {
            let image = render_room(room, evidence(), 20, MotionMode::Full, w, h);
            assert_eq!((image.w, image.h), (w, h));
            assert_eq!(
                image,
                render_room(room, evidence(), 20, MotionMode::Full, w, h)
            );
        }
        for motion in [MotionMode::Off, MotionMode::Reduced] {
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
        render_room(Room::Study, evidence(), 0, MotionMode::Full, 256, 176),
        render_room(Room::Vault, evidence(), 0, MotionMode::Full, 256, 176)
    );
}

#[test]
fn a_quiet_school_does_not_paint_receipts_or_signal_progress() {
    for room in [Room::Study, Room::Vault] {
        let image = render_room(
            room,
            SchoolSnapshot::default(),
            0,
            MotionMode::Off,
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
