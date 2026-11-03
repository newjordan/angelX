//! A minion's finisher (`gore`): chosen from the blow, the distance and the
//! stone behind; cosmetic only; the same on a seat; capped.

use super::*;
use crate::rules::feel::testing::{a_foe, bare, quiet};
use std::collections::BTreeMap;

/// A calm bare room: a knight at `hero` with `weapon` and a monster `gap`
/// units east of them, one blow from death and holding still.
fn scene(kind: EnemyKind, weapon: Weapon, hero: (f32, f32), gap: f32) -> (Run, u32) {
    let mut run = quiet();
    bare(&mut run);
    let h = run.players.get_mut(&1).unwrap();
    (h.x, h.y) = hero;
    h.weapon = weapon;
    let id = a_foe(&mut run, kind, hero.0 + gap, hero.1);
    for _ in 0..40 {
        let e = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
        (e.hp, e.max_hp, e.frozen) = (1, 1, 100_000);
        run.step(&BTreeMap::new());
    }
    (run, id)
}

fn blow(run: &mut Run, id: u32, swing: bool) {
    for _ in 0..40 {
        if !run.enemies.iter().any(|e| e.id == id && e.hp > 0) {
            return;
        }
        let input = Input { aim_x: 1, fire: !swing, swing, ..Input::default() };
        run.step(&BTreeMap::from([(1, input)]));
    }
    panic!("the monster did not fall");
}

#[test]
fn a_point_blank_arrow_kicks_the_monster_back_and_pins_it_to_the_wall() {
    let (mut run, id) = scene(EnemyKind::Skeleton, Weapon::Bow, (38.5, 14.0), 3.2);
    blow(&mut run, id, false);
    let g = run.gore.last().expect("a finisher");
    assert_eq!((g.cut, g.pin, g.shaft), (Cut::Kick, Pin::Wall, Shot::Arrow));
    assert!(g.big);
    // It ends against the east stone, a little short of it, on the same line.
    assert!(g.to_x > g.x + 1.0 && g.to_x < 46.0, "{g:?}");
    assert!((g.to_y - g.y).abs() < 0.2);
    assert_eq!(g.id, id);
}

#[test]
fn a_point_blank_bolt_with_no_wall_in_reach_pins_the_monster_to_the_floor() {
    let (mut run, id) = scene(EnemyKind::Goblin, Weapon::Crossbow, (22.0, 14.0), 3.2);
    blow(&mut run, id, false);
    let g = run.gore.last().expect("a finisher");
    assert_eq!((g.cut, g.pin, g.shaft), (Cut::Kick, Pin::Floor, Shot::Bolt));
    assert!(g.to_x > g.x);
}

#[test]
fn a_shot_from_range_knocks_the_body_back_along_its_line() {
    let (mut run, id) = scene(EnemyKind::Hexer, Weapon::Crossbow, (12.0, 14.0), 11.0);
    blow(&mut run, id, false);
    let g = run.gore.last().expect("a finisher");
    assert_eq!((g.cut, g.pin), (Cut::Punch, Pin::No));
    assert!(g.to_x > g.x && (g.to_y - g.y).abs() < 0.2);
}

#[test]
fn a_sword_takes_the_head_or_cuts_across_and_a_slime_splats() {
    let mut seen = std::collections::BTreeSet::new();
    for (i, kind) in [EnemyKind::Goblin, EnemyKind::Skeleton, EnemyKind::Necromancer, EnemyKind::Hexer, EnemyKind::Marksman, EnemyKind::Hob, EnemyKind::Bat]
        .into_iter()
        .enumerate()
    {
        let (mut run, id) = scene(kind, Weapon::Bow, (22.0 + i as f32 * 0.31, 14.0), 1.9);
        blow(&mut run, id, true);
        let cut = run.gore.last().expect("a finisher").cut;
        assert!(matches!(cut, Cut::Behead | Cut::Halve), "{kind:?}: {cut:?}");
        if !gore::headed(kind) {
            assert_eq!(cut, Cut::Halve, "{kind:?} has no head");
        }
        seen.insert(format!("{cut:?}"));
    }
    assert_eq!(seen.len(), 2, "some of each: {seen:?}");
    let (mut run, id) = scene(EnemyKind::Slime, Weapon::Bow, (22.0, 14.0), 2.2);
    blow(&mut run, id, true);
    assert_eq!(run.gore.last().unwrap().cut, Cut::Splat);
}

#[test]
fn a_handgonne_ball_point_blank_bursts_the_monster_into_chunks() {
    let (mut run, id) = scene(EnemyKind::Goblin, Weapon::Handgonne, (22.0, 14.0), 3.0);
    blow(&mut run, id, false);
    assert_eq!(run.gore.last().unwrap().cut, Cut::Burst);
}

#[test]
fn bosses_spirits_and_the_great_have_none() {
    use EnemyKind as K;
    for kind in [K::Dragon, K::Boss, K::PitTyrant, K::Risen, K::Broodmarrow, K::Dummy, K::Ward, K::Wraith, K::Wisp, K::Hollow] {
        assert!(gore::body(kind).is_none(), "{kind:?}");
    }
    for kind in [K::Skeleton, K::Slime, K::Imp, K::Goblin, K::Geode, K::Mimic] {
        assert!(gore::body(kind).is_some(), "{kind:?}");
    }
}

#[test]
fn what_the_picture_holds_never_changes_the_rules() {
    // The same fight twice; one run forgets its finishers every tick. The
    // rules, written out, agree to the byte: nothing reads them back.
    let play = |forget: bool| {
        let (mut run, id) = scene(EnemyKind::Skeleton, Weapon::Bow, (38.5, 14.0), 3.2);
        let mut written = Vec::new();
        for _ in 0..90 {
            let input = Input { aim_x: 1, fire: true, ..Input::default() };
            let live = run.enemies.iter().any(|e| e.id == id && e.hp > 0);
            run.step(&BTreeMap::from([(1, if live { input } else { Input::default() })]));
            if forget {
                run.gore.clear();
                run.feel.echoes.clear();
            }
            written.push(serde_json::to_string(&run).unwrap());
        }
        (written, run.gore.len())
    };
    let (kept, n) = play(false);
    let (forgot, m) = play(true);
    assert_eq!((n, m), (1, 0));
    assert_eq!(kept, forgot);
    assert!(!kept.last().unwrap().contains("\"gore\""), "a save leaves them out");
}

#[test]
fn a_seat_reads_the_same_finisher_off_the_wire() {
    let (mut run, id) = scene(EnemyKind::Skeleton, Weapon::Bow, (38.5, 14.0), 3.2);
    blow(&mut run, id, false);
    let tree = crate::wire::publish(&run).unwrap();
    let seat = crate::wire::read(&tree).unwrap();
    assert_eq!(seat.gore, run.gore);
    assert_eq!(seat.gore.len(), 1);
}

#[test]
fn the_kick_sounds_now_and_the_thunk_when_the_body_arrives() {
    let (mut run, id) = scene(EnemyKind::Skeleton, Weapon::Bow, (38.5, 14.0), 3.2);
    let mut heard: Vec<(u64, &str)> = Vec::new();
    for _ in 0..40 {
        let live = run.enemies.iter().any(|e| e.id == id && e.hp > 0);
        let input = if live { Input { aim_x: 1, fire: true, ..Input::default() } } else { Input::default() };
        run.step(&BTreeMap::from([(1, input)]));
        for name in std::mem::take(&mut run.sounds) {
            if gore::SOUNDS.contains(&name) {
                heard.push((run.tick, name));
            }
        }
    }
    let kick = heard.iter().find(|h| h.1 == "gore_kick").expect("the kick").0;
    let thunk = heard.iter().find(|h| h.1 == "gore_thunk").expect("the thunk").0;
    let span = u64::from(run.gore.last().unwrap().span);
    assert_eq!(thunk - kick, gore::HOLD + span);
}

#[test]
fn a_room_of_deaths_adds_a_few_finishers_and_keeps_to_the_cap() {
    let mut run = quiet();
    bare(&mut run);
    for i in 0..24 {
        let id = a_foe(&mut run, EnemyKind::Skeleton, 8.0 + (i % 8) as f32 * 2.0, 6.0 + (i / 8) as f32 * 4.0);
        run.enemies.iter_mut().find(|e| e.id == id).unwrap().frozen = 100_000;
    }
    for e in &mut run.enemies {
        (e.hp, e.wits.foe) = (0, 1);
    }
    run.step(&BTreeMap::new());
    assert!(run.enemies.iter().all(|e| e.hp > 0) && run.gore.len() <= gore::MAX_NEW, "{}", run.gore.len());
    for _ in 0..4 {
        for i in 0..24 {
            let id = a_foe(&mut run, EnemyKind::Skeleton, 8.0 + (i % 8) as f32 * 2.0, 6.0 + (i / 8) as f32 * 4.0);
            let e = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
            (e.hp, e.wits.foe, e.frozen) = (0, 1, 100_000);
        }
        run.step(&BTreeMap::new());
    }
    assert!(run.gore.len() <= gore::MAX_LIVE, "{}", run.gore.len());
}

#[test]
fn finishers_age_out_and_a_new_room_clears_them() {
    let (mut run, id) = scene(EnemyKind::Skeleton, Weapon::Bow, (38.5, 14.0), 3.2);
    blow(&mut run, id, false);
    assert_eq!(run.gore.len(), 1);
    for _ in 0..gore::LIFE + 2 {
        run.step(&BTreeMap::new());
    }
    assert!(run.gore.is_empty());
}

#[test]
fn the_best_finishers_hold_the_room_only_when_the_knob_is_set() {
    // Off by default: the rules' time never stands still for a finisher,
    // which is why no run's outcome moves.
    let (mut run, id) = scene(EnemyKind::Skeleton, Weapon::Bow, (38.5, 14.0), 3.2);
    assert_eq!(run.tune.hitstop_finisher, 0);
    blow(&mut run, id, false);
    assert!(run.gore.last().unwrap().big);
    assert_eq!(run.hitstop.left, 0);
    let (mut run, id) = scene(EnemyKind::Skeleton, Weapon::Bow, (38.5, 14.0), 3.2);
    assert!(run.tune.set("hitstop_finisher", 3.0) && run.tune.valid());
    blow(&mut run, id, false);
    assert_eq!(run.hitstop.left, 3);
    // A punch from range is not one of the best.
    let (mut run, id) = scene(EnemyKind::Hexer, Weapon::Crossbow, (12.0, 14.0), 11.0);
    run.tune.set("hitstop_finisher", 3.0);
    blow(&mut run, id, false);
    assert_eq!(run.hitstop.left, 0);
}

#[test]
fn the_variant_comes_from_the_kill_and_never_the_dice() {
    // The same kill twice agrees on everything, and the run's own dice are
    // where they were: a finisher draws none.
    let play = || {
        let (mut run, id) = scene(EnemyKind::Necromancer, Weapon::Bow, (22.0, 14.0), 1.9);
        blow(&mut run, id, true);
        (run.gore.clone(), serde_json::to_string(&run.rng).unwrap())
    };
    assert_eq!(play(), play());
}

/// Knight 1's sword chain at `n` strokes (the next swing is its `n + 1`th),
/// swung at nothing with the monster `id` stood well aside.
fn chain_to(run: &mut Run, id: u32, n: u32) {
    let at = run.enemies.iter().find(|e| e.id == id).map(|e| (e.x, e.y)).unwrap();
    run.enemies.iter_mut().find(|e| e.id == id).unwrap().x += 20.0;
    for _ in 0..n {
        run.step(&BTreeMap::from([(1, Input { swing: true, ..Input::default() })]));
        while run.players[&1].sword_cooldown > 0 || run.hitstop.frozen() {
            run.step(&BTreeMap::new());
        }
    }
    let e = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
    (e.x, e.y) = at;
}

#[test]
fn the_blow_chooses_the_cut_a_level_sweep_the_head_the_backhand_the_middle_the_maul_the_length() {
    // A level sweep, the chain's first stroke, takes a goblin's head.
    let (mut run, id) = scene(EnemyKind::Goblin, Weapon::Bow, (22.0, 14.0), 1.9);
    blow(&mut run, id, true);
    let g = run.gore.last().expect("a finisher");
    assert_eq!((g.cut, g.slant), (Cut::Behead, None));
    // The backhand cuts it through the middle, the cut sloping down away
    // from the knight so the upper piece slides off that way.
    let (mut run, id) = scene(EnemyKind::Goblin, Weapon::Bow, (22.0, 14.0), 1.9);
    chain_to(&mut run, id, 1);
    blow(&mut run, id, true);
    let g = run.gore.last().expect("a finisher");
    assert_eq!(g.cut, Cut::Halve);
    assert!(g.slant.is_some_and(|s| (14..=30).contains(&s)), "{:?}", g.slant);
    // The finisher takes the head and throws it furthest.
    let (mut run, id) = scene(EnemyKind::Goblin, Weapon::Bow, (22.0, 14.0), 1.9);
    chain_to(&mut run, id, 2);
    blow(&mut run, id, true);
    let g = run.gore.last().expect("a finisher");
    assert_eq!((g.cut, g.span), (Cut::Behead, 18));
    // A headless thing is cut through the middle by any sweep.
    let (mut run, id) = scene(EnemyKind::Bat, Weapon::Bow, (22.0, 14.0), 1.9);
    blow(&mut run, id, true);
    assert!(run.gore.last().is_some_and(|g| g.cut == Cut::Halve && g.slant.is_some()));
    // The great maul from above splits a goblin nearly down its length.
    let (mut run, id) = scene(EnemyKind::Goblin, Weapon::Bow, (22.0, 14.0), 1.9);
    assert!(run.take_up(1, "hammerer"));
    blow(&mut run, id, false);
    let g = run.gore.last().expect("a finisher");
    assert_eq!(g.cut, Cut::Halve);
    assert!(g.slant.is_some_and(|s| s >= 58), "{:?}", g.slant);
    // A knight to the east cuts the other way.
    let (mut run, id) = scene(EnemyKind::Bat, Weapon::Bow, (26.0, 14.0), -1.9);
    run.players.get_mut(&1).unwrap().aim_x = -1.0;
    for _ in 0..40 {
        if !run.enemies.iter().any(|e| e.id == id && e.hp > 0) {
            break;
        }
        run.step(&BTreeMap::from([(1, Input { aim_x: -1, swing: true, ..Input::default() })]));
    }
    assert!(run.gore.last().is_some_and(|g| g.slant.is_some_and(|s| s < 0)), "{:?}", run.gore.last());
}

#[test]
fn before_the_melee_pass_a_swing_chose_by_its_variant_alone() {
    crate::rules::melee::BEFORE.with(|b| b.set(true));
    let (mut run, id) = scene(EnemyKind::Bat, Weapon::Bow, (22.0, 14.0), 1.9);
    blow(&mut run, id, true);
    crate::rules::melee::BEFORE.with(|b| b.set(false));
    let g = run.gore.last().expect("a finisher");
    assert_eq!((g.cut, g.slant), (Cut::Halve, None));
    let json = serde_json::to_string(&run.gore).unwrap();
    assert!(!json.contains("slant"), "an old record writes no slope");
}

#[test]
fn a_head_taken_thuds_when_it_lands_and_a_body_cut_through_slumps_when_it_falls() {
    for (i, (kind, cut)) in [(EnemyKind::Goblin, Cut::Behead), (EnemyKind::Bat, Cut::Halve)].into_iter().enumerate() {
        let (mut run, id) = scene(kind, Weapon::Bow, (22.0 + i as f32 * 0.31, 14.0), 1.9);
        let mut heard: Vec<(u64, &str)> = Vec::new();
        for _ in 0..60 {
            let live = run.enemies.iter().any(|e| e.id == id && e.hp > 0);
            let input = if live { Input { aim_x: 1, swing: true, ..Input::default() } } else { Input::default() };
            run.step(&BTreeMap::from([(1, input)]));
            for name in std::mem::take(&mut run.sounds) {
                if gore::SOUNDS.contains(&name) {
                    heard.push((run.tick, name));
                }
            }
        }
        let g = *run.gore.last().expect("a finisher");
        assert_eq!(g.cut, cut, "{kind:?}");
        let (now, later) = if cut == Cut::Behead { ("gore_behead", "gore_head_thud") } else { ("gore_bisect", "gore_slump") };
        let at = heard.iter().find(|h| h.1 == now).unwrap_or_else(|| panic!("{kind:?}: {heard:?}")).0;
        let after = heard.iter().find(|h| h.1 == later).unwrap_or_else(|| panic!("{kind:?}: {heard:?}")).0;
        // When the picture has the head land, or the halves reach the floor.
        assert_eq!(after - at, gore::HOLD + u64::from(g.span), "{kind:?}");
        assert!(!heard.iter().any(|h| h.1 == "gore_chop"), "{kind:?}: {heard:?}");
    }
    // Bones still crack and clatter.
    let (mut run, id) = scene(EnemyKind::Skeleton, Weapon::Bow, (22.0, 14.0), 1.9);
    run.sounds.clear();
    blow(&mut run, id, true);
    for _ in 0..30 {
        run.step(&BTreeMap::new());
    }
    assert!(run.sounds.contains(&"gore_chop") && run.sounds.contains(&"gore_bones"), "{:?}", run.sounds);
}
