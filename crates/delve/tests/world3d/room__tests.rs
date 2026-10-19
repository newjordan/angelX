//! The dungeon stands up in 3D as a function of depth (docs/WORLD-3D.md,
//! "Deeper floors"): the upper floors keep the 2D black look, the deep ones
//! stand up, the dragon's floor its guardian's hall only; one host setting
//! says so for every seat; the party's first steps into the deep play a beat
//! the run's clock drives; and a save from before any of it loads.

use super::*;
use crate::rules::{DEEP_FLOOR, FLOORS, HZ, RoomKind, Run};

const KINDS: [RoomKind; 6] = [RoomKind::Start, RoomKind::Fight, RoomKind::Treasure, RoomKind::Stairs, RoomKind::Lair, RoomKind::Hall];

#[test]
fn the_upper_floors_stay_in_2d_and_every_room_of_the_deep_stands_up() {
    for kind in KINDS {
        for depth in 0..TRANSITION {
            assert!(!stands_in_mode(Mode::Deep, depth, kind), "floor {depth}, {kind:?}: the black look");
        }
        for depth in DEEP..=crate::rules::DEEPEST {
            assert!(stands_in_mode(Mode::Deep, depth, kind), "floor {depth}, {kind:?}: in 3D");
        }
    }
    assert_eq!(DEEP, TRANSITION + 1);
    assert_eq!(TRANSITION, FLOORS, "the change begins on the dragon's floor");
}

#[test]
fn on_the_dragons_floor_only_the_guardians_halls_stand_up() {
    for kind in KINDS {
        let guardian = matches!(kind, RoomKind::Lair);
        assert_eq!(stands_in_mode(Mode::Deep, TRANSITION, kind), guardian, "{kind:?}");
    }
    assert!(stands_in_mode(Mode::Deep, TRANSITION, RoomKind::Pit), "the Pit Tyrant's");
    // The home is never deep, whatever its rooms are.
    assert!(!stands_in_mode(Mode::Deep, 0, RoomKind::Home));
}

#[test]
fn the_override_stands_every_room_up_and_off_none() {
    for kind in KINDS.into_iter().chain([RoomKind::Home, RoomKind::Kingdom]) {
        for depth in 0..=crate::rules::DEEPEST {
            assert!(stands_in_mode(Mode::Always, depth, kind));
            assert!(!stands_in_mode(Mode::Off, depth, kind));
        }
    }
}

#[test]
fn a_host_stands_the_deep_floors_up_unless_its_environment_says_otherwise() {
    assert_eq!(Mode::from_env(None), Mode::Deep);
    assert_eq!(Mode::from_env(Some("")), Mode::Deep);
    assert_eq!(Mode::from_env(Some("1")), Mode::Always);
    assert_eq!(Mode::from_env(Some("0")), Mode::Off);
}

#[test]
fn the_kingdom_in_3d_waits_for_the_override() {
    set_mode(Mode::Deep);
    assert!(!enabled(), "the deep floors only: the kingdom stays 2D");
    set_mode(Mode::Always);
    assert!(enabled());
    set_enabled(false);
    assert_eq!(mode(), Mode::Off);
}

/// A run taken down to floor `depth`.
fn run_at(depth: u32) -> Run {
    let mut run = Run::new(9, 1, Some("Friend"));
    while run.dungeon.depth < depth {
        run.descend_for_test();
    }
    assert_eq!(run.dungeon.depth, depth);
    run
}

#[test]
fn a_rooms_world_is_its_floors_to_give() {
    set_mode(Mode::Deep);
    let shallow = run_at(2);
    assert!(!stands_here(&shallow));
    assert!(world_in(&shallow, Some(1), (384, 224)).is_none(), "the black look");
    let deep = run_at(DEEP);
    assert!(stands_here(&deep));
    let world = world_in(&deep, Some(1), (384, 224)).expect("the deep stands up");
    assert!(world.lit().is_some(), "lit, in the monolith look");
    set_mode(Mode::Off);
    assert!(world_in(&deep, Some(1), (384, 224)).is_none());
}

#[test]
fn the_party_comes_down_into_the_deep_once_to_a_beat_its_clock_plays() {
    set_mode(Mode::Deep);
    let mut run = Run::new(9, 1, Some("Friend"));
    while run.dungeon.depth < TRANSITION {
        run.descend_for_test();
        assert_eq!(run.deep_beat, None, "floor {}: no beat above the deep", run.dungeon.depth);
    }
    run.descend_for_test();
    let at = run.deep_beat.expect("down into the deep");
    assert_eq!(run.dungeon.depth, DEEP_FLOOR);
    assert_eq!(beat(&run), Some(0.0));
    // Further down, the deep is already the deep.
    run.descend_for_test();
    assert_eq!(run.deep_beat, Some(at), "one beat, at the way in");
    // The beat is the clock's: the same tick, the same moment, on any seat.
    run.tick = at + u64::from(HZ);
    assert_eq!(beat(&run), Some(1.0));
    run.tick = at + (BEAT * HZ as f32) as u64;
    assert_eq!(beat(&run), None, "over");
    // A page drawing it all in 2D has no beat to play.
    run.tick = at;
    set_mode(Mode::Off);
    assert_eq!(beat(&run), None);
}

#[test]
fn the_beat_holds_the_cut_black_leans_in_and_lights_the_nearest_lamps_first() {
    assert_eq!(beat_dark(0.0), 1.0, "the cut is black");
    assert_eq!(beat_dark(1.0), 0.0, "and the room shows by a second in");
    assert!(beat_zoom(0.0) > 1.3 && beat_zoom(BEAT) == 1.0, "close on the party, then the room's own view");
    assert!((0..30).all(|n| beat_zoom(n as f32 * 0.1) >= beat_zoom(n as f32 * 0.1 + 0.1)), "only ever easing back");
    assert_eq!(beat_rise(0.0), 0.0);
    assert_eq!(beat_rise(BEAT), 1.0);
    let mid = beat_rise(1.4);
    assert!(lamp_up(mid, 0.0) > lamp_up(mid, 0.5) && lamp_up(mid, 0.5) >= lamp_up(mid, 1.0), "the nearest first");
    assert_eq!(lamp_up(1.0, 1.0), 1.0, "every lamp up at the end");
    let moment = moment_of(&run_at(1), Some(1));
    assert_eq!(moment.rise, 1.0, "outside the beat a room burns as it burns");
}

#[test]
fn a_save_from_before_the_deep_stood_up_loads() {
    let mut run = run_at(1);
    let mut json = serde_json::to_value(&run).expect("it saves");
    assert!(json.get("deep_beat").is_none(), "nothing written while there is no beat");
    json.as_object_mut().unwrap().remove("deep_beat");
    let back: Run = serde_json::from_value(json).expect("an old save loads");
    assert_eq!(back.deep_beat, None);
    run.deep_beat = Some(77);
    let back: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    assert_eq!(back.deep_beat, Some(77), "and a new one keeps its beat");
}
