use super::*;
use crate::render::overworld::kingdom as geography;
use crate::rules::{Input, RoomKind, Run, roundtable};
use std::collections::BTreeMap;

/// What `played` hashes to on the code before the knobs (world/engine
/// 269ed115), with the reviewed effect/card art. The JSON includes the
/// built-in card book and boss roster, so their reviewed art rows
/// deliberately change these hashes without changing the simulated rules.
/// It changed again when knights came to face the way they walk, a party's
/// fight rooms were laid out for it, and a floor's extra foes came to ramp
/// up toward its guardian. And when the fight's effects came to carry the
/// way a hit was going, what fell, a roll's dust and a find's glint: the
/// play itself is the same (the runs written without `feel` hash alike
/// before and after), and again when the branches of the 2026-10-17
/// play-test met, and again when the boss library, the Bard and the Peasant
/// came in with the art integration of 2026-10-15, and the play-test
/// branch of 2026-10-14 (its card table, guardian stages and charge-ups,
/// shot families) met them, and again when the six-seat party and the
/// tamed demons (gentler sweeps and beams, a steady climb of floor damage)
/// came in with that line, and again when the dungeon masters hand-drew
/// their rooms (the fight room it plays in is another; the kingdom is not),
/// and again when the viewer economy was retired (no audience, fans, fan
/// boxes or Sponsors' Night in the run, no viewer feats or bounties), and
/// again when the kingdom opened into biomes (the kingdom walk only: a
/// new map under the same two knights, who now note the biome they enter),
/// and again when its castles grew into great walled exteriors (the
/// kingdom walk only: the Keep's wall now stands north of the knights), and
/// again when the play line came in (its wishes, shared loot and fight
/// changes; both runs hash differently). The five Round Table adventures
/// change nothing here: the same hashes come out with and without them.
/// And again (the fight room only) when a hit came to ward a knight a whole
/// second and take nine tenths of one at most, and when the party's shots
/// came to cover a share of the screen, live three seconds and bounce twice
/// at most (2026-10-18).
const TODAY: [u64; 2] = [0x031b9581c83f7269, 0x85dae6459bd08c64];

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
    // The farthest plain fight room from the stairs: all of the floor's
    // pressure (`stakes::RAMP_FIRST` lightens the first rooms).
    let plain: Vec<usize> = (0..fight.dungeon.rooms.len())
        .filter(|&i| fight.dungeon.rooms[i].kind == RoomKind::Fight && !fight.dungeon.rooms[i].great())
        .collect();
    let mut i = plain[0];
    let mut farthest = 0;
    for &room in &plain {
        fight.at = room;
        if fight.floor_progress() > farthest {
            (i, farthest) = (room, fight.floor_progress());
        }
    }
    fight.at = 0;
    fight.enter_for_test(i);
    for t in 0..900u64 {
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
    // away mark, at their defaults), or the absent mini-game activities, which
    // today's code did not write.
    let written = |run: &Run| {
        // The new, zero-drop Bard and Peasant arms is inert in these old two-knight
        // fixtures. Compare their original book, preserving TODAY exactly;
        // no gameplay, RNG or timing fields are removed from the trace.
        assert!(run.players.values().all(|h| h.weapon != crate::rules::Weapon::Notes));
        assert!(run.bard_songs.is_empty());
        let mut run = run.clone();
        let note = run.book.get("note-stream").unwrap();
        assert_eq!((note.drop, note.chest), (0, 0));
        let stones = run.book.get("thrown-stones").unwrap();
        assert_eq!((stones.drop, stones.chest), (0, 0));
        assert!(run.scarecrows.is_empty());
        assert!(run.players.values().all(|h| h.weapon != crate::rules::Weapon::Stones && h.flee_ticks == 0));
        run.book.cards.retain(|c| !matches!(c.id.as_str(), "note-stream" | "thrown-stones"));
        run.book.revision -= 2;
        // The floor's tally and its results count what was played; they
        // play nothing.
        run.floor_results = None;
        let tally = format!("\"floor_tally\":{},", serde_json::to_string(&run.floor_tally).unwrap());
        serde_json::to_string(&run).unwrap().replace(&tally, "").replace("\"held\":\"open\",", "").replace("\"away\":false,", "").replace("\"cinder\":null,", "").replace("\"latch\":null,", "").replace("\"mossbell\":null,", "").replace("\"brasswake\":null,", "").replace("\"rafterwake\":null,", "").replace("\"pennon\":null,", "")
    };
    [fnv(written(&fight)), fnv(written(&kingdom))]
}

#[test]
fn hitstop_off_plays_the_old_rules_tick_for_tick() {
    assert_eq!(played(Tune { hitstop: 0, ..Tune::default() }), TODAY);
}

#[test]
fn hitstop_default_keeps_the_ordinary_two_knight_baseline() {
    assert_eq!(played(Tune::default()), TODAY);
}

#[test]
fn every_knob_moved_changes_the_play() {
    for knob in Tune::default().knobs().into_iter().filter(|k| !k.name.starts_with("hitstop") && !k.name.starts_with("cinder_") && !k.name.starts_with("latch_") && !k.name.starts_with("moss_") && !k.name.starts_with("brass_") && !k.name.starts_with("rafter_") && !k.name.starts_with("pennon_") && !roundtable::ADVENTURES.iter().any(|a| k.group == a.name)) {
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
    // The Round Table adventures' knobs follow, their names their own modules'.
    let mut expected = Vec::from(["hero_speed", "road_pace", "roll_speed", "fire_rate", "shot_speed", "foe_pace", "foe_hit", "foe_health", "foe_count", "camera_follow", "hit_ward_s", "cinder_fall", "cinder_slide", "cinder_lock", "cinder_tell", "cinder_attack", "pennon_gravity", "pennon_jump", "pennon_run", "pennon_coyote", "pennon_buffer", "rafter_gravity", "rafter_jump", "rafter_run", "rafter_coyote", "rafter_buffer", "brass_stride", "brass_combo", "brass_tell", "brass_shove", "brass_stop", "latch_tell", "latch_window", "latch_rest", "latch_recover", "latch_stop", "moss_flipper", "moss_gravity", "moss_bounce", "moss_nudge", "moss_launch", "hitstop", "hitstop_boss_kill", "hitstop_elite_kill", "hitstop_phase", "hitstop_heavy", "hitstop_cooldown"]);
    expected.extend(roundtable::ADVENTURES.iter().flat_map(|a| a.numbers.iter().map(|n| n.name)));
    assert_eq!(names, expected);
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
    // Today's ward is the default: 30 ticks, a whole second.
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
    assert_eq!(ward(1.0), 30);
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

#[test]
fn no_one_hit_takes_more_than_nine_tenths_of_a_knight() {
    let mut run = Run::new(3, 1, None);
    run.step(&BTreeMap::new());
    let hero = run.players.get_mut(&1).unwrap();
    hero.invulnerable = 0;
    let full = hero.max_hp;
    hero.hp = full;
    hero.hurt(full * 50);
    assert!(hero.hp > 0, "a knight at full health lives through any one hit");
    assert!(hero.hp >= full - full * crate::rules::hero::ONE_HIT_MOST / 100);
    assert_eq!(hero.invulnerable, crate::rules::hero::HIT_WARD, "and is warded a whole second");
    // Hurt already, the next big hit after the ward can fell them.
    hero.invulnerable = 0;
    hero.hurt(full * 50);
    assert_eq!(hero.hp, 0);
}
