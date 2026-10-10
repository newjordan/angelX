use super::*;

/// An open first-floor room of the Crypt, emptied and calmed, with knight
/// one (and two, if `pair`) standing on the left.
fn quiet_room(pair: bool) -> Run {
    let mut run = Run::new(4, 1, pair.then_some("Friend"));
    run.begin_in(Pack::Crypt);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    run.rune = None;
    // A bat frozen in the far corner keeps the fight on (a won room clears
    // every Ravage).
    run.spawn_at_for_test(EnemyKind::Bat, 44.0, 3.0);
    if let Some(bat) = run.enemies.last_mut() {
        bat.frozen = u32::MAX / 2;
    }
    for (id, y) in [(1, 12.0), (2, 17.0)] {
        if let Some(hero) = run.players.get_mut(&id) {
            (hero.x, hero.y, hero.invulnerable) = (10.0, y, 0);
        }
    }
    run
}

/// A Ravage centred `d` east of knight one, its lane on `lane`.
fn ravage_by(run: &mut Run, d: f32, lane: f32) -> Ravage {
    let hero = &run.players[&1];
    let ravage = Ravage {
        x: hero.x + d,
        y: hero.y,
        rings: 4,
        lane,
        width: 50f32.to_radians(),
        step: 8,
        damage: 16,
        age: 0,
    };
    run.ravages.push(ravage);
    ravage
}

#[test]
fn the_rings_burst_outward_each_on_its_beat_and_throw_the_knight() {
    let mut run = quiet_room(false);
    let ravage = ravage_by(&mut run, RING_GAP * 2.0, 0.0);
    let (_, bursts) = ravage.ring(1);
    assert_eq!(
        bursts,
        WARN + 8,
        "the second ring, one step after the first"
    );
    let hp = run.players[&1].hp;
    for _ in 0..bursts - 1 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.players[&1].hp, hp, "only ripples so far");
    let x = run.players[&1].x;
    run.step(&BTreeMap::new());
    let hero = &run.players[&1];
    assert!(hero.hp < hp, "the second ring bursts under the knight");
    assert_eq!(hero.tossed, TOSSED);
    assert!(hero.x < x - 0.5, "thrown outward, away from the middle");
    assert!(run.cues.iter().any(|c| c == "ravaged"));
    // Every ring has burst, and the tentacles have gone back down.
    for _ in 0..3 * 8 + UP + 2 {
        run.step(&BTreeMap::new());
    }
    assert!(run.ravages.is_empty());
}

#[test]
fn the_still_lane_and_a_roll_both_spare_a_knight() {
    let struck = |lane: f32, rolling: bool| {
        let mut run = quiet_room(false);
        // The knight stands west of the middle, on the first ring.
        ravage_by(&mut run, RING_GAP, lane);
        let hp = run.players[&1].hp;
        for _ in 0..WARN + 3 {
            if rolling {
                run.players.get_mut(&1).unwrap().invulnerable = 5;
            }
            run.step(&BTreeMap::new());
        }
        run.players[&1].hp < hp
    };
    assert!(struck(0.0, false), "the lane runs east, the knight is west");
    assert!(
        !struck(std::f32::consts::PI, false),
        "the knight in the lane"
    );
    assert!(!struck(0.0, true), "the knight rolling as it bursts");
}

#[test]
fn the_rings_overlap_so_no_ground_between_them_is_safe() {
    let ravage = Ravage::cast((0.0, 0.0), 5, 50.0, 8.0, 16, 7);
    assert_eq!(ravage.step, 8, "2.2 steps apart at 8 a second");
    let reach = ravage.ring(4).0 + BAND;
    let mut d = BAND;
    while d < reach {
        assert!(
            (0..5).any(|k| (d - ravage.ring(k).0).abs() <= BAND),
            "{d} lies between the rings"
        );
        d += 0.01;
    }
}

#[test]
fn a_thrown_knight_neither_walks_nor_shoots() {
    let mut run = quiet_room(false);
    run.players.get_mut(&1).unwrap().tossed = TOSSED;
    let x = run.players[&1].x;
    let go = BTreeMap::from([(
        1,
        Input {
            move_x: 1,
            fire: true,
            ..Default::default()
        },
    )]);
    for _ in 0..TOSSED - 1 {
        run.step(&go);
    }
    assert_eq!(run.players[&1].x, x);
    assert!(!run.projectiles.iter().any(|p| !p.hostile));
    for _ in 0..10 {
        run.step(&go);
    }
    assert!(run.players[&1].x > x + 1.0, "down again, and walking");
}

#[test]
fn the_leviathan_keeps_the_archive_and_ravages_at_a_knight() {
    let mut run = quiet_room(false);
    let index = run
        .bosses
        .iter()
        .position(|b| b.id == "late-fee-leviathan")
        .expect("a built-in guardian");
    let boss = &run.bosses[index];
    assert_eq!(boss.only_in, Pack::Archive);
    assert!(
        boss.attacks
            .iter()
            .any(|a| a.pattern == bosses::Pattern::Ravage)
    );
    run.spawn_boss(index);
    for _ in 0..12 * HZ {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        run.step(&BTreeMap::new());
        if !run.ravages.is_empty() {
            break;
        }
    }
    assert_eq!(run.ravages.len(), 1);
    assert_eq!(run.ravages[0].rings, 5);
    assert!(run.cues.iter().any(|c| c == "ravage"));
}

#[test]
fn a_boss_file_may_ravage_within_bounds() {
    let raw = "name TEST TIDE\nwhere archive\nhp 500\nmove anchor\nattack ravage shots=9 arc=10 every=100\nart\n.x.\n";
    let (boss, notes) = bosses::check("test-tide", raw).expect("a ravage passes");
    let ravage = boss.attacks[0];
    assert_eq!(ravage.pattern, bosses::Pattern::Ravage);
    assert_eq!((ravage.shots, ravage.arc, ravage.every), (6, 30.0, 150));
    assert!(notes.len() >= 3, "every clamp is noted: {notes:?}");
    assert!(bosses::rules().contains("ravage"));
}
