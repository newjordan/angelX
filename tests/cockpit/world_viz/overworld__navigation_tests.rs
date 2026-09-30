use super::*;
use crate::agent::harness::ToolEventId;

#[test]
fn every_named_district_can_be_viewed_without_moving_the_knight() {
    let mut world = World::new(7);
    let home = world.overworld_scene().knight;
    for place in Place::ALL {
        let before_key = world.overworld_scene().key();
        assert_eq!(world.visit_overworld(place.label()), Some(place.label()));
        assert_eq!(world.overworld_view_label(), Some(place.label()));
        let (x, y, w, h) = place.footprint_world();
        assert_eq!(
            world.overworld_scene().camera,
            (
                (x as f32 + w as f32 / 2.0) * TILE as f32,
                (y as f32 + h as f32 / 2.0) * TILE as f32,
            )
        );
        assert_ne!(
            world.overworld_scene().key(),
            before_key,
            "view changes repaint immediately"
        );
        assert_eq!(world.overworld_scene().knight, home);
        assert_eq!(world.overworld_goal(), Place::Keep);
    }
}

#[test]
fn district_aliases_work_and_unknown_names_preserve_the_current_view() {
    let mut world = World::new(7);
    for (name, place) in [
        ("artisans", Place::ArtisanQuarter),
        ("artisan-quarter", Place::ArtisanQuarter),
        ("COLOSSEUM", Place::Colosseum),
        ("tournament", Place::Tournament),
        ("round table", Place::RoundTable),
        ("village", Place::Village),
        ("fields", Place::Fields),
    ] {
        assert_eq!(world.visit_overworld(name), Some(place.label()));
    }
    let camera = world.overworld_scene().camera;
    assert_eq!(world.visit_overworld(""), None);
    assert_eq!(world.visit_overworld("missing-district"), None);
    assert_eq!(world.overworld_scene().camera, camera);
    assert_eq!(world.overworld_view_label(), Some(Place::Fields.label()));
}

#[test]
fn a_fixed_view_keeps_work_running_and_follow_restores_the_live_camera() {
    let mut world = World::new(7);
    world.visit_overworld("colosseum").unwrap();
    let shown = world.overworld_scene();
    world.note_tool_call_event(
        ToolEventId("navigation-work".into()),
        "write_file",
        "src/main.rs",
    );
    for _ in 0..32 {
        world.tick();
    }
    assert_eq!(world.overworld_goal(), Place::Smithy);
    assert_eq!(world.overworld_scene().camera, shown.camera);
    assert_ne!(
        world.overworld_scene().knight,
        shown.knight,
        "actual work continues travelling"
    );
    let current_knight = world.overworld_scene().knight;
    world.follow_overworld();
    assert_eq!(world.overworld_view_label(), None);
    assert_ne!(world.overworld_scene().camera, shown.camera);
    assert_eq!(world.overworld_scene().knight, current_knight);
}
