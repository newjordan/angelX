use super::*;

/// Floor one's entrance (a plain room, no pillars) as a fight, knight one
/// at its middle, one monster of `kind` at `(x, y)` ready to act.
fn with(kind: EnemyKind, x: f32, y: f32) -> Run {
    let mut run = Run::new(33, 1, None);
    run.enter_for_test(0);
    run.calm_for_test();
    run.enemies.clear();
    run.spawn_at_for_test(kind, x, y);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 14.0);
    hero.invulnerable = 0;
    run.phase = Phase::Fighting;
    run
}

fn tick(run: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        run.step(&BTreeMap::new());
    }
}

/// Step with knight one held at full health and in place.
fn hold(run: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        let hero = run.players.get_mut(&1).unwrap();
        hero.hp = hero.max_hp;
        run.step(&BTreeMap::new());
    }
}

#[test]
fn rooms_keep_to_one_of_each_dangerous_kind() {
    for seed in 0..40 {
        for pack in [Pack::Crypt, Pack::Cavern, Pack::Hellforge] {
            let floor = layout::floor(2, pack, &mut Rng::new(seed));
            for room in &floor.rooms {
                for kind in room.roster.iter().filter(|k| k.elite()) {
                    assert_eq!(
                        room.roster.iter().filter(|k| *k == kind).count(),
                        1,
                        "{pack:?} {:?}",
                        room.roster
                    );
                }
            }
        }
    }
}

#[test]
fn a_sappers_keg_blows_and_a_shot_sets_it_off_early() {
    let mut run = with(EnemyKind::Sapper, 34.0, 14.0);
    hold(&mut run, 150);
    assert!(!run.kegs.is_empty(), "a keg is planted near the knight");
    let keg = run.kegs[0];
    assert!((keg.x - 24.0).hypot(keg.y - 14.0) < 10.0);
    // A knight's shot at a keg sets it off, and a monster beside it feels it.
    run.spawn_at_for_test(EnemyKind::Skeleton, keg.x + 1.0, keg.y);
    let skeleton = run.enemies.last().unwrap().id;
    let hp = run.enemies.last().unwrap().hp;
    run.projectiles.push(Projectile {
        x: keg.x - 0.5,
        y: keg.y,
        vx: 1.0,
        vy: 0.0,
        hostile: false,
        kind: Shot::Arrow,
        look: None,
        damage: 1,
        pierce: 0,
        last_hit: None,
        empowered: false,
        ttl: 30,
        traits: ShotTraits::default(),
    });
    run.players.get_mut(&1).unwrap().hp = 100;
    tick(&mut run, 3);
    assert!(
        run.kegs.iter().all(|k| (k.x, k.y) != (keg.x, keg.y)),
        "blown"
    );
    let after = run
        .enemies
        .iter()
        .find(|e| e.id == skeleton)
        .map_or(0, |e| e.hp);
    assert!(
        after + 30 <= hp,
        "the keg hurts monsters too: {hp} → {after}"
    );
}

#[test]
fn a_keg_left_alone_hurts_a_knight_beside_it() {
    let mut run = with(EnemyKind::Bat, 46.0, 2.0);
    run.enemies.clear();
    run.phase = Phase::Fighting;
    run.kegs.push(foes::Keg {
        x: 25.0,
        y: 14.0,
        fuse: 30,
    });
    let hp = run.players[&1].hp;
    tick(&mut run, 2);
    assert!(run.kegs.is_empty(), "an armed keg goes off under a knight");
    assert!(run.players[&1].hp < hp);
}

#[test]
fn a_necromancer_raises_the_dead() {
    let mut run = with(EnemyKind::Necromancer, 36.0, 10.0);
    let raised = |run: &Run| {
        run.enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::Skeleton)
            .count()
    };
    hold(&mut run, 160);
    assert!(raised(&run) >= 2, "two skeletons rise");
    assert!(run.cues.iter().any(|c| c == "necro_raise"));
}

#[test]
fn a_necromancer_has_bones_for_three_raisings() {
    let mut run = with(EnemyKind::Necromancer, 36.0, 10.0);
    hold(&mut run, 150 * 5);
    let raisings = run.cues.iter().filter(|c| *c == "necro_raise").count();
    assert_eq!(raisings, usize::from(foes::NECRO_RAISES));
}

#[test]
fn what_stands_in_for_a_second_dangerous_kind_is_never_dangerous() {
    for pack in [
        Pack::Crypt,
        Pack::Cavern,
        Pack::Hellforge,
        Pack::Archive,
        Pack::Fungal,
        Pack::Unknown,
    ] {
        for depth in 1..=DEEPEST {
            assert!(!pack.plain_at(depth).elite(), "{pack:?} floor {depth}");
        }
    }
}

#[test]
fn a_warboar_marks_its_line_charges_and_runs_a_knight_down() {
    let mut run = with(EnemyKind::Warboar, 38.0, 14.0);
    let mut marked = false;
    let mut hit = false;
    for _ in 0..200 {
        let before = run.players[&1].hp;
        let at = (run.players[&1].x, run.players[&1].y);
        run.step(&BTreeMap::new());
        let boar = run.enemies.iter().find(|e| e.kind == EnemyKind::Warboar);
        marked |= boar.is_some_and(|b| b.stage == 1);
        let hero = run.players.get_mut(&1).unwrap();
        if hero.hp + 20 < before {
            hit = true;
            assert_ne!((hero.x, hero.y), at, "and thrown aside");
            break;
        }
        hero.invulnerable = 0;
    }
    assert!(marked, "it marks its line before it goes");
    assert!(hit, "the charge runs the knight down");
}

#[test]
fn a_warboar_that_hits_a_wall_stands_dazed() {
    let mut run = with(EnemyKind::Warboar, 24.0, 6.0);
    {
        let boar = run.enemies.last_mut().unwrap();
        boar.stage = 2;
        boar.dir = (0.0, -1.0);
    }
    tick(&mut run, 12);
    let boar = run.enemies.last().unwrap();
    assert_eq!(boar.stage, 3, "dazed against the north wall");
}

#[test]
fn slimes_split_twice() {
    let mut run = with(EnemyKind::Slime, 34.0, 14.0);
    for (generation, count) in [(1, 2), (2, 4)] {
        for enemy in &mut run.enemies {
            enemy.hp = 0;
        }
        tick(&mut run, 1);
        let slimes: Vec<&Enemy> = run
            .enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::Slime)
            .collect();
        assert_eq!(slimes.len(), count);
        assert!(slimes.iter().all(|s| s.stage == generation));
        assert!(slimes.iter().all(|s| s.radius() < 1.0));
    }
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    tick(&mut run, 1);
    assert!(
        run.enemies.iter().all(|e| e.kind != EnemyKind::Slime),
        "the smallest just pop"
    );
}

#[test]
fn a_loot_goblin_slips_away_or_pays_out() {
    let mut run = with(EnemyKind::Goblin, 30.0, 14.0);
    let start = run.enemies[0].x;
    hold(&mut run, foes::GOBLIN_STAYS / 2);
    let goblin = run
        .enemies
        .iter()
        .find(|e| e.kind == EnemyKind::Goblin)
        .unwrap();
    assert!(
        (goblin.x - 24.0).hypot(goblin.y - 14.0) > (start - 24.0).abs(),
        "it runs from the knight"
    );
    hold(&mut run, foes::GOBLIN_STAYS);
    assert!(
        run.enemies.iter().all(|e| e.kind != EnemyKind::Goblin),
        "gone"
    );
    assert!(run.cues.iter().any(|c| c == "goblin_escaped"));
    // Caught: gold all round, and a prize.
    let mut run = with(EnemyKind::Goblin, 30.0, 14.0);
    run.enemies[0].hp = 0;
    tick(&mut run, 1);
    let gold = run.room().items.iter().filter(|i| i.card == "gold").count();
    assert!(gold >= 4, "{gold} gold");
    assert!(run.room().items.len() >= 5);
}

#[test]
fn a_hob_lobs_a_bomb_where_the_knight_stood() {
    let mut run = with(EnemyKind::Hob, 36.0, 10.0);
    let mut thrown = None;
    for _ in 0..120 {
        run.players.get_mut(&1).unwrap().hp = 100;
        run.step(&BTreeMap::new());
        if let Some(lob) = run.lobs.first() {
            thrown = Some(*lob);
            break;
        }
    }
    let lob = thrown.expect("a bomb is thrown");
    assert!((lob.x - 24.0).hypot(lob.y - 14.0) < 0.1, "at the knight");
    let hp = run.players[&1].hp;
    run.players.get_mut(&1).unwrap().invulnerable = 0;
    tick(&mut run, foes::LOB_FALL);
    assert!(run.players[&1].hp < hp, "standing still under it hurts");
}

#[test]
fn a_shaman_boxes_a_knight_in_with_wards_that_crumble() {
    let mut run = with(EnemyKind::Shaman, 36.0, 6.0);
    let wards = |run: &Run| {
        run.enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::Ward)
            .count()
    };
    hold(&mut run, 100);
    assert!(wards(&run) >= 3, "{} wards", wards(&run));
    for ward in run.enemies.iter().filter(|e| e.kind == EnemyKind::Ward) {
        assert!(
            (ward.x - 24.0).hypot(ward.y - 14.0) < 5.5,
            "round the knight"
        );
    }
    run.enemies.retain(|e| e.kind == EnemyKind::Ward);
    hold(&mut run, foes::WARD_STANDS + 2);
    assert_eq!(wards(&run), 0, "they crumble in time");
}

#[test]
fn the_mirror_carries_kegs_and_bombs() {
    let mut run = with(EnemyKind::Hob, 36.0, 10.0);
    run.kegs.push(foes::Keg {
        x: 10.0,
        y: 10.0,
        fuse: 50,
    });
    run.lobs.push(foes::Lob {
        from: (36.0, 10.0),
        x: 24.0,
        y: 14.0,
        fall: 20,
    });
    let live = run.live();
    let mut friend: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    friend.kegs.clear();
    friend.lobs.clear();
    assert!(friend.apply_live(live));
    assert_eq!((friend.kegs.len(), friend.lobs.len()), (1, 1));
}
