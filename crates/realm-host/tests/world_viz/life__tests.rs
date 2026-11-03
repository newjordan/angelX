use super::*;

#[test]
fn weather_is_deterministic_and_moored_to_the_clock() {
    for beats in [0, 5, 64, 640, 4096] {
        assert_eq!(
            weather_for_beats(beats, 42),
            weather_for_beats(beats, 42),
            "weather must be a pure function of (beats, seed)"
        );
    }
    // Every realm keeps its own climate.
    let mut differs = false;
    for beats in 0..400 {
        if weather_for_beats(beats, 1) != weather_for_beats(beats, 2) {
            differs = true;
            break;
        }
    }
    assert!(differs, "two seeds should not share one sky forever");
    // The bucket fits in two bits for the terrain key.
    for beats in 0..256 {
        assert!(weather_bucket(beats, 7) < 4);
    }
}

#[test]
fn growth_announcements_fire_once_per_crossing() {
    let mut world = World::new(9);
    world.renown = u64::from(TIER_DOCKS);
    world.note_growth_announcements();
    let first = world.gain_note.clone();
    assert_eq!(first, "a little dock rides at anchor");
    assert!(world.firework_until > world.tick, "an unlock celebrates");
    let fireworks = world.firework_until;
    world.note_growth_announcements();
    assert_eq!(
        world.firework_until, fireworks,
        "the same tier must not re-announce"
    );
    // Boot-time worlds adopt their tier silently (no history replay).
    let mut loaded = World::new(9);
    loaded.renown = u64::from(TIER_KEEP_TOWERS);
    loaded.growth_announced = town_tier(loaded.renown as u32);
    loaded.note_growth_announcements();
    assert_eq!(loaded.firework_until, 0, "a loaded town stays quiet");
}

#[test]
fn orbit_sway_only_turns_once_settled() {
    let mut world = World::new(11);
    // Riding: no sway.
    world.target = Building::Observatory;
    world.avatar = world.building_pos(Building::Keep);
    world.avatar_vis = (world.avatar.0 as f32, world.avatar.1 as f32);
    world.travel_ticks = cinematics::DEPART_FADE_TICKS + 1;
    for _ in 0..4 {
        world.tick();
    }
    assert!(world.riding());
    assert_eq!(world.orbit_sway(), 0.0);
    // Settled past the arrival hold: the camera breathes.
    world.avatar = world.building_pos(Building::Observatory);
    world.avatar_vis = (world.avatar.0 as f32, world.avatar.1 as f32);
    world.settle_ticks = cinematics::CINEMATIC_SETTLE_TICKS + 40;
    let a = world.orbit_sway();
    world.tick += 28;
    let b = world.orbit_sway();
    assert!(
        (a - b).abs() > 0.001,
        "a settled camera should keep shifting: {a} vs {b}"
    );
}
