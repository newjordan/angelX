use super::*;
use crate::render::overworld::kingdom as geography;
use crate::rules::{Input, RoomKind, Run, roundtable};
use std::collections::BTreeMap;

#[path = "together_tune__pin_fixture.rs"]
mod pin_fixture;

const TODAY: [u64; 2] = [3226237403750841178, 14999891280965652695];
const BEFORE_SHOT_SIZE: [u64; 2] = [1325769177011145675, 1924171319231336929];

fn played(tune: Tune) -> [u64; 2] {
    played_with(tune, false)
}

fn played_with(tune: Tune, without_beats: bool) -> [u64; 2] {
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
    fight.tune = Tune::default();
    kingdom.tune = Tune::default();
    let fnv = |s: String| s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    let written = |run: &Run| {
        assert!(run.players.values().all(|h| h.weapon != crate::rules::Weapon::Notes));
        assert!(run.bard_songs.is_empty());
        let mut run = run.clone();
        assert!(run.feel.ranged.iter().all(crate::rules::ranged::Event::valid));
        run.feel.ranged.clear();
        let note = run.book.get("note-stream").unwrap();
        assert_eq!((note.drop, note.chest), (0, 0));
        let stones = run.book.get("thrown-stones").unwrap();
        assert_eq!((stones.drop, stones.chest), (0, 0));
        assert!(run.scarecrows.is_empty());
        assert!(run.players.values().all(|h| h.weapon != crate::rules::Weapon::Stones && h.flee_ticks == 0));
        assert!(run.players.values().all(|h| h.arm.as_deref() != Some("the-ball")));
        const GRAVE: [&str; 5] = ["grave-staff", "raise-ghoul", "barrow-warden", "bone-totem", "soul-rip"];
        for id in GRAVE {
            let card = run.book.get(id).unwrap();
            assert_eq!((card.drop, card.chest), (0, 0));
        }
        assert!(run.players.values().all(|h| h.arm.as_deref() != Some("grave-staff")));
        assert!(run.minions.is_empty());
        run.book.cards.retain(|c| !matches!(c.id.as_str(), "note-stream" | "thrown-stones" | "the-ball") && !GRAVE.contains(&c.id.as_str()));
        run.book.revision -= 3 + GRAVE.len();
        let private: Vec<_> = crate::rules::knights::COMPANY.iter()
            .filter(|k| crate::rules::heroes::KITS.contains(&k.id))
            .flat_map(|k| std::iter::once(k.arm).chain(k.spells.iter().copied()))
            .collect();
        assert_eq!(private.len(), 12);
        assert!(run.heroes.is_empty());
        assert!(run.players.values().all(|h| h.hero_kit.is_empty()
            && !h.knight.as_deref().is_some_and(|k| crate::rules::heroes::KITS.contains(&k))));
        for id in &private {
            let card = run.book.get(id).unwrap();
            assert_eq!((card.drop, card.chest), (0, 0));
        }
        run.book.cards.retain(|c| !private.contains(&c.id.as_str()));
        run.book.revision -= private.len();
        const STOCK: [&str; 3] = ["stock-blade", "stock-staff", "stock-maul"];
        for id in STOCK {
            let card = run.book.get(id).unwrap();
            assert_eq!(card.kind, crate::rules::cards::Kind::Arm);
            assert_eq!((card.drop, card.chest), (0, 0));
        }
        assert!(run.players.values().all(|h| h.arm.as_deref().is_none_or(|id| !STOCK.contains(&id))));
        assert_eq!(run.alehouse.company_page, 0);
        let count = run.book.cards.len();
        run.book.cards.retain(|c| !STOCK.contains(&c.id.as_str()));
        assert_eq!(count - run.book.cards.len(), STOCK.len());
        run.book.revision -= STOCK.len();
        run.floor_results = None;
        run.rewards = Default::default();
        run.ledger = Default::default();
        run.home.sky = Default::default();
        run.dungeon.altar_at.clear();
        let tally = format!("\"floor_tally\":{},", serde_json::to_string(&run.floor_tally).unwrap());
        let snapshot = pin_fixture::snapshot(&run);
        let json = serde_json::to_string(&snapshot).unwrap();
        let written = crate::rules::layout::expand_saved_tiles(&json);
        let written = if without_beats { without_beat(&written) } else { written };
        let written = written
            .replace("\"patience\":0,", "")
            .replace("\"sacs\":[],", "")
            .replace(",\"thick\":false", "")
            .replace("\"grounded\":0,\"frog\":0,\"shunned\":false,", "");
        written.replace(&tally, "").replace("\"held\":\"open\",", "").replace("\"away\":false,", "").replace("\"cinder\":null,", "").replace("\"latch\":null,", "").replace("\"mossbell\":null,", "").replace("\"brasswake\":null,", "").replace("\"rafterwake\":null,", "").replace("\"pennon\":null,", "")
    };
    [fnv(written(&fight)), fnv(written(&kingdom))]
}

fn without_beat(json: &str) -> String {
    let mut out = String::with_capacity(json.len());
    let mut rest = json;
    while let Some(at) = rest.find(",\"beat\":") {
        out.push_str(&rest[..at]);
        let after = &rest[at + ",\"beat\":".len()..];
        rest = &after[after.find(|c: char| !c.is_ascii_digit()).unwrap_or(after.len())..];
    }
    out.push_str(rest);
    out
}

#[test]
fn shots_as_they_were_play_the_rules_before_the_size_tick_for_tick() {
    crate::rules::wishcraft::LEGACY.with(|l| l.set(true));
    let before = played_with(Tune::default(), true);
    crate::rules::wishcraft::LEGACY.with(|l| l.set(false));
    assert_eq!(before, BEFORE_SHOT_SIZE);
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
    for knob in Tune::default().knobs().into_iter().filter(|k| k.group != "first floors" && !k.name.starts_with("hitstop") && !k.name.starts_with("cinder_") && !k.name.starts_with("latch_") && !k.name.starts_with("moss_") && !k.name.starts_with("brass_") && !k.name.starts_with("rafter_") && !k.name.starts_with("pennon_") && !roundtable::ADVENTURES.iter().any(|a| k.group == a.name)) {
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
    let mut expected = Vec::from(["hero_speed", "road_pace", "roll_speed", "fire_rate", "shot_speed", "foe_pace", "foe_shots", "foe_shot_speed", "foe_hit", "foe_health", "foe_count", "camera_follow", "hit_ward_s", "clear_rest", "clear_rest_floors", "dare_after_s", "cinder_fall", "cinder_slide", "cinder_lock", "cinder_tell", "cinder_attack", "pennon_gravity", "pennon_jump", "pennon_run", "pennon_coyote", "pennon_buffer", "rafter_gravity", "rafter_jump", "rafter_run", "rafter_coyote", "rafter_buffer", "brass_stride", "brass_combo", "brass_tell", "brass_shove", "brass_stop", "latch_tell", "latch_window", "latch_rest", "latch_recover", "latch_stop", "moss_flipper", "moss_gravity", "moss_bounce", "moss_nudge", "moss_launch", "hitstop", "hitstop_boss_kill", "hitstop_elite_kill", "hitstop_phase", "hitstop_heavy", "hitstop_cooldown", "hitstop_finisher", "hitstop_melee", "hitstop_melee_kill"]);
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
    assert!(tune.set("foe_health", 50.0));
    assert_eq!(tune.foe_health, 100);
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
        hero.hurt(10);
        assert_eq!(hero.hp, hit);
        hero.invulnerable
    };
    assert_eq!(ward(1.5), 45);
    assert_eq!(ward(2.0), 60, "the panel reaches past today's");
    assert_eq!(ward(1.0), 30);
    assert_eq!(ward(0.8), 24);
    assert_eq!(ward(0.25), 8, "a quarter second, to the tick");
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
    assert_eq!(hero.invulnerable, crate::rules::hero::HIT_WARD, "and is warded a second and a half");
    hero.invulnerable = 0;
    hero.hurt(full * 50);
    assert_eq!(hero.hp, 0);
}

#[test]
fn fewer_faster_foe_bullets_thin_evenly_and_keep_their_reach() {
    let mut tune = Tune::default();
    assert!(tune.set("foe_shots", 50.0) && tune.set("foe_shot_speed", 1.5));
    assert!(tune.valid() && !tune.is_default());
    assert!(tune.set("foe_shots", 5.0));
    assert_eq!(tune.foe_shots, 10, "never fewer than a tenth");
    assert!(tune.set("foe_shots", 50.0));
    let keep = u64::from(tune.foe_shots);
    let kept = (1..=100u64).filter(|n| n * keep / 100 != (n - 1) * keep / 100).count();
    assert_eq!(kept, 50, "every other new bullet flies");
}
