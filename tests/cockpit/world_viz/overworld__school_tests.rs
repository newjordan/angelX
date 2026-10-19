//! The School of Magic as the live world visits it and its receipts
//! repaint it.

use super::school::testing::*;
use super::school::*;
use super::map::{self, Place};
use super::frame_sized;
use crate::agent::harness::ToolEventId;
use crate::stage::world_viz::World;

#[test]
fn school_has_a_reachable_front_door_without_relocating_existing_landmarks() {
    let target = Place::School.stand_world();
    assert!(map::walkable(map::Realm::get().at(target.0, target.1)));
    let route = super::live::route(Place::Keep.stand_world(), target);
    assert_eq!(route.last(), Some(&target));
    // The School's set piece stands in the market gardens' chunk, (2, 5).
    assert_eq!(Place::School.screen(), (3, 7));
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
