use super::*;
use crate::render::overworld::kingdom as geography;
use crate::rules::{Input, RoomKind, Run};
use std::collections::BTreeMap;

/// What `played` hashes to on the code before the knobs (world/engine
/// 269ed115), with the reviewed effect/card art. The JSON includes the
/// built-in card book, so its art rows deliberately change these hashes.
const TODAY: [u64; 2] = [0xab3318167ad4ed, 0x380282c82f86ef6b];

/// Today's rules played the same way every time, with `tune`: two knights
/// walking, rolling and firing through a fight room as its roster spawns
/// and comes on, and two walking the kingdom's roads as their cameras
/// follow. Each run as JSON, hashed (FNV-1a), after.
fn played(tune: Tune) -> [u64; 2] {
    let input = |t: u64, who: u32| -> Input {
        let phase = ((t / 20) + u64::from(who)) % 4;
        let (mx, my) = [(1, 0), (0, 1), (-1, 0), (0, -1)][phase as usize];
        Input {
            move_x: mx,
            move_y: my,
            aim_x: if t % 60 < 30 { 1 } else { -1 },
            fire: t % 3 != 0,
            dash: t % 45 == 0,
            ..Default::default()
        }
    };
    let step = |run: &mut Run, t: u64| {
        run.step(&BTreeMap::from([(1, input(t, 1)), (2, input(t, 2))]));
    };
    let mut fight = Run::new(11, 1, Some("Friend"));
    fight.tune = tune;
    let i = fight
        .dungeon
        .rooms
        .iter()
        .position(|room| room.kind == RoomKind::Fight && !room.great())
        .unwrap();
    fight.enter_for_test(i);
    for t in 0..400u64 {
        step(&mut fight, t);
    }
    let mut kingdom = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    kingdom.tune = tune;
    kingdom.walk_kingdom(Some(geography::keep_stand()));
    for t in 0..240u64 {
        step(&mut kingdom, t);
    }
    // The tune itself is no part of what today's code wrote.
    fight.tune = Tune::default();
    kingdom.tune = Tune::default();
    let fnv = |s: String| s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    // Nor is who is here (`presence`: the room's hold and each knight's
    // away mark, at their defaults), which today's code did not write.
    let written = |run: &Run| {
        serde_json::to_string(run).unwrap().replace("\"held\":\"open\",", "").replace("\"away\":false,", "")
    };
    [fnv(written(&fight)), fnv(written(&kingdom))]
}

#[test]
fn the_default_tune_plays_todays_rules_tick_for_tick() {
    assert_eq!(played(Tune::default()), TODAY);
}

#[test]
fn every_knob_moved_changes_the_play() {
    for knob in Tune::default().knobs() {
        let moved = if knob.max > knob.default { knob.max } else { knob.min };
        let mut tune = Tune::default();
        assert!(tune.set(knob.name, moved), "{}", knob.name);
        assert_ne!(played(tune), TODAY, "{} at {moved} changed nothing in the play", knob.name);
    }
}

#[test]
fn the_knobs_stand_at_todays_numbers_and_set_by_name() {
    let mut tune = Tune::default();
    assert!(tune.is_default() && tune.valid());
    let knobs = tune.knobs();
    let names: Vec<_> = knobs.iter().map(|k| k.name).collect();
    assert_eq!(
        names,
        ["hero_speed", "road_pace", "roll_speed", "fire_rate", "shot_speed", "foe_pace", "foe_hit", "foe_health", "foe_count", "camera_follow", "hit_ward_s"]
    );
    for k in &knobs {
        assert_eq!(k.value, k.default, "{}", k.name);
        assert_eq!(k.fit(k.default), Some(k.default), "{}", k.name);
        assert!(k.min <= k.default && k.default <= k.max, "{}", k.name);
    }
    let pace = knobs.iter().find(|k| k.name == "camera_follow").unwrap();
    assert_eq!(pace.default, 0.22, "shown as the code writes it");
    assert_eq!((knobs[0].default, knobs[1].default, knobs[2].default), (13.0, 2.0, 27.0));
    assert!(tune.set("hero_speed", 15.5));
    assert!(!tune.set("nope", 1.0));
    assert_eq!(tune.knobs()[0].value, 15.5);
    // Pressure is added, never health cut: the floor holds.
    assert!(tune.set("foe_health", 50.0));
    assert_eq!(tune.foe_health, 100);
    // Back to the default is today's run exactly.
    assert!(tune.set("hero_speed", 13.0));
    assert!(tune.set("foe_health", 100.0));
    assert!(tune.is_default());
}

#[test]
fn a_run_writes_its_tune_only_once_a_knob_moved_and_reads_old_saves() {
    let mut run = Run::new(3, 1, None);
    let plain = serde_json::to_string(&run).unwrap();
    assert!(!plain.contains("\"tune\""), "an untouched run is written as today's");
    run.tune.hero_speed = 20.0;
    let tuned = serde_json::to_string(&run).unwrap();
    assert!(tuned.contains("\"tune\":{"));
    let back: Run = serde_json::from_str(&tuned).unwrap();
    assert_eq!(back.tune.hero_speed, 20.0);
    let old: Run = serde_json::from_str(&plain).unwrap();
    assert!(old.tune.is_default());
    // A tune that is no number is no run to play.
    run.tune.hero_speed = f32::NAN;
    assert!(!run.valid_snapshot());
}

#[test]
fn the_pace_knob_walks_a_knight_that_much_faster() {
    let walk = |speed: f32| {
        let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
        r.walk_kingdom(Some(geography::keep_stand()));
        r.tune.hero_speed = speed;
        let x = r.players[&1].x;
        r.step(&BTreeMap::from([(1, Input { move_x: 1, ..Default::default() })]));
        r.players[&1].x - x
    };
    let (slow, fast) = (walk(13.0), walk(19.5));
    assert!(slow > 0.0);
    assert!((fast / slow - 1.5).abs() < 1e-4, "{slow} {fast}");
}

#[test]
fn a_hit_wards_a_knight_for_the_tuned_ward_and_it_stands_at_todays() {
    // Today's ward is the default: 24 ticks, 0.8 s.
    let d = Tune::default();
    assert_eq!(d.hit_ward_ticks(), crate::rules::hero::HIT_WARD);
    let ward = |secs: f64| {
        let mut run = Run::new(3, 1, None);
        assert!(run.tune.set("hit_ward_s", secs));
        run.step(&BTreeMap::new());
        let hero = run.players.get_mut(&1).unwrap();
        hero.invulnerable = 0;
        let full = hero.hp;
        hero.hurt(10);
        let hit = hero.hp;
        assert!(hit < full, "the first hit lands");
        // Another at once is turned aside while the ward lasts.
        hero.hurt(10);
        assert_eq!(hero.hp, hit);
        hero.invulnerable
    };
    assert_eq!(ward(0.8), 24);
    assert_eq!(ward(0.25), 8, "a quarter second, to the tick");
    // At no ward a second hit right after lands too.
    let mut run = Run::new(3, 1, None);
    assert!(run.tune.set("hit_ward_s", 0.0));
    run.step(&BTreeMap::new());
    let hero = run.players.get_mut(&1).unwrap();
    hero.invulnerable = 0;
    hero.hurt(10);
    let hit = hero.hp;
    hero.hurt(10);
    assert!(hero.hp < hit);
}
