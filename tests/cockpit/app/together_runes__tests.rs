use super::*;

/// Floor one of the Cavern, in its first fight room, the monsters calmed so
/// nothing interrupts, and a rune of `kind` on the floor beside knight one.
fn by_a_rune(kind: RuneKind) -> Run {
    let mut run = Run::new(4, 1, None);
    run.begin_in(Pack::Cavern);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    let hero = run.players.get_mut(&1).unwrap();
    let (x, y) = (hero.x, hero.y);
    run.rune = Some(Rune {
        kind,
        x: x + 6.0,
        y,
        at: run.tick,
    });
    run
}

/// Knight one walks onto the rune.
fn take(run: &mut Run) {
    let rune = run.rune.unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (rune.x, rune.y);
    hero.invulnerable = 0;
    run.step(&BTreeMap::new());
}

#[test]
fn some_fights_well_up_a_rune_a_few_seconds_in() {
    let mut runes = 0;
    let mut kinds = std::collections::BTreeSet::new();
    for seed in 0..60u64 {
        let mut run = Run::new(seed, 1, None);
        run.begin_in(Pack::Cavern);
        for room in 0..run.dungeon.rooms.len() {
            if run.dungeon.rooms[room].kind != RoomKind::Fight {
                continue;
            }
            run.enter_for_test(room);
            let Some(rune) = run.rune else {
                continue;
            };
            runes += 1;
            kinds.insert(rune.kind.word());
            assert_eq!(rune.at, run.tick + u64::from(RUNE_AFTER));
            let r = run.room();
            let (c, row) = ((rune.x / TILE_UNITS) as i32, (rune.y / TILE_UNITS) as i32);
            assert_eq!(r.tile(c, row), Tile::Floor, "a rune stands on open floor");
        }
    }
    assert!((40..=200).contains(&runes), "{runes} runes");
    assert_eq!(
        kinds.len(),
        RuneKind::ALL.len(),
        "every kind turns up: {kinds:?}"
    );
}

#[test]
fn nothing_wells_up_at_home_or_in_a_won_room() {
    let mut run = Run::at_home(4, 1, None, home::Home::default(), Default::default());
    run.step(&BTreeMap::new());
    assert!(run.rune.is_none(), "the Undercroft keeps no runes");
    let mut run = by_a_rune(RuneKind::Haste);
    let here = run.at;
    run.clear_for_test();
    run.enter_for_test(here);
    assert!(run.rune.is_none(), "a room walked back into has no rune");
}

#[test]
fn a_rune_wells_up_on_its_tick_and_the_herald_says_so() {
    let mut run = by_a_rune(RuneKind::Haste);
    let mut rune = run.rune.unwrap();
    rune.at = run.tick + 3;
    run.rune = Some(rune);
    // Standing on it before it is there takes nothing.
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (rune.x, rune.y);
    run.step(&BTreeMap::new());
    assert!(run.rune.is_some());
    assert!(run.players[&1].rune.is_none());
    let heard = run.cues.len();
    for _ in 0..3 {
        run.step(&BTreeMap::new());
    }
    assert!(run.cues[heard..].iter().any(|c| c == "rune_up"));
    assert!(run.rune.is_none(), "taken the tick it rose");
    assert!(run.players[&1].has_rune(RuneKind::Haste));
}

#[test]
fn haste_runs_faster() {
    let walk = |kind: Option<RuneKind>| {
        let mut run = by_a_rune(RuneKind::Haste);
        run.rune = None;
        let hero = run.players.get_mut(&1).unwrap();
        hero.rune = kind.map(|kind| Held {
            kind,
            left: kind.lasts(),
        });
        let x = hero.x;
        let right = BTreeMap::from([(
            1,
            Input {
                move_x: 1,
                ..Default::default()
            },
        )]);
        for _ in 0..HZ / 2 {
            run.step(&right);
        }
        run.players[&1].x - x
    };
    let (plain, haste) = (walk(None), walk(Some(RuneKind::Haste)));
    assert!(haste > plain * 1.4, "haste {haste} vs {plain}");
}

#[test]
fn double_damage_hits_twice_as_hard() {
    let mut run = by_a_rune(RuneKind::DoubleDamage);
    let plain = run.players[&1].scaled(40);
    take(&mut run);
    assert!(run.players[&1].has_rune(RuneKind::DoubleDamage));
    assert_eq!(run.players[&1].scaled(40), plain * 2);
    assert!(run.cues.iter().any(|c| c == "rune:double_damage"));
    assert_eq!(
        run.found.as_ref().map(|f| f.2.as_str()),
        Some("Double Damage!")
    );
}

#[test]
fn regeneration_mends_until_whole_or_hit() {
    let mut run = by_a_rune(RuneKind::Regeneration);
    run.players.get_mut(&1).unwrap().hp = 40;
    take(&mut run);
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
    }
    let hp = run.players[&1].hp;
    assert!((47..=53).contains(&hp), "about ten a second: {hp}");
    // A hit ends it.
    let hero = run.players.get_mut(&1).unwrap();
    hero.invulnerable = 0;
    hero.hurt(10);
    assert!(hero.rune.is_none(), "a hit breaks the mending");
    // Whole ends it too.
    let mut run = by_a_rune(RuneKind::Regeneration);
    let max = run.players[&1].max_hp;
    run.players.get_mut(&1).unwrap().hp = max - 2;
    take(&mut run);
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.players[&1].hp, max);
    assert!(
        run.players[&1].rune.is_none(),
        "whole again, the rune is done"
    );
}

#[test]
fn arcane_hurries_the_ultimate() {
    let charge = |rune: bool| {
        let mut run = by_a_rune(RuneKind::Arcane);
        if rune {
            take(&mut run);
        } else {
            run.rune = None;
        }
        let before = run.players[&1].ult_charge;
        for _ in 0..HZ {
            run.step(&BTreeMap::new());
        }
        run.players[&1].ult_charge - before
    };
    let (plain, arcane) = (charge(false), charge(true));
    assert!(arcane >= plain + 2 * HZ - 2, "arcane {arcane} vs {plain}");
}

#[test]
fn monsters_cannot_see_an_invisible_knight() {
    // A skeleton across the room looses at a knight it can see, and not at
    // one it can't.
    let shots = |hidden: bool| {
        let mut run = by_a_rune(RuneKind::Invisibility);
        run.rune = None;
        run.enemies.clear();
        run.spawn_at_for_test(EnemyKind::Skeleton, 30.0, 14.0);
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (10.0, 14.0);
        if hidden {
            hero.rune = Some(Held {
                kind: RuneKind::Invisibility,
                left: RuneKind::Invisibility.lasts(),
            });
        }
        let mut loosed = 0;
        for _ in 0..4 * HZ {
            run.step(&BTreeMap::new());
            loosed = loosed.max(run.projectiles.iter().filter(|p| p.hostile).count());
        }
        loosed
    };
    assert!(shots(false) > 0, "a skeleton shoots at a knight it sees");
    assert_eq!(shots(true), 0, "and not at one it can't");
}

#[test]
fn illusion_makes_two_images_at_once() {
    let mut run = by_a_rune(RuneKind::Illusion);
    take(&mut run);
    assert_eq!(run.phantoms.iter().filter(|p| p.owner == 1).count(), 2);
    assert!(
        run.players[&1].rune.is_none(),
        "an Illusion is spent at once"
    );
}

#[test]
fn bounty_pays_every_knight_standing_and_wisdom_teaches_the_party() {
    let mut run = by_a_rune(RuneKind::Bounty);
    let before = run.players[&1].carried.get(Spoil::Gold);
    take(&mut run);
    assert_eq!(run.players[&1].carried.get(Spoil::Gold), before + 25);
    let mut run = by_a_rune(RuneKind::Wisdom);
    run.marks.clear();
    take(&mut run);
    assert_eq!(run.marks.get("xp"), Some(&80));
}

#[test]
fn runes_run_out() {
    let mut run = by_a_rune(RuneKind::Haste);
    take(&mut run);
    for _ in 0..RuneKind::Haste.lasts() {
        run.step(&BTreeMap::new());
    }
    assert!(run.players[&1].rune.is_none());
}

#[test]
fn four_runes_in_a_delve_is_rune_runner() {
    let mut run = by_a_rune(RuneKind::Bounty);
    for _ in 0..4 {
        let hero = &run.players[&1];
        run.rune = Some(Rune {
            kind: RuneKind::Bounty,
            x: hero.x + 6.0,
            y: hero.y,
            at: run.tick,
        });
        take(&mut run);
    }
    assert_eq!(run.runes_taken, 4);
    assert!(run.feats.contains(&"rune_runner"));
}
