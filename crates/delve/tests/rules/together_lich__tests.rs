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
    // every hostile shot).
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

/// Rimeleap, loosed from `(x, y)` at knight one.
fn rimeleap(run: &mut Run, (x, y): (f32, f32)) {
    let hero = &run.players[&1];
    let (ux, uy) = unit(hero.x - x, hero.y - y);
    run.projectiles.push(Projectile {
        x,
        y,
        vx: ux * 5.0,
        vy: uy * 5.0,
        hostile: true,
        look: None,
        kind: Shot::Frost,
        damage: 16,
        pierce: lich::BOUNCES,
        last_hit: None,
        empowered: false,
        traits: Default::default(),
        ttl: 6 * HZ,
    });
}

/// Steps with every knight held still and unhurt by anything but frost.
fn wait(run: &mut Run, ticks: u32) -> u32 {
    let mut struck = 0;
    for _ in 0..ticks {
        let before: Vec<u32> = run.players.values().map(|h| h.chilled).collect();
        for hero in run.players.values_mut() {
            hero.invulnerable = 0;
            hero.hp = hero.max_hp;
        }
        run.step(&BTreeMap::new());
        let after: Vec<u32> = run.players.values().map(|h| h.chilled).collect();
        struck += before.iter().zip(&after).filter(|(b, a)| **a > **b).count() as u32;
    }
    struck
}

#[test]
fn rimeleap_leaps_between_knights_who_stand_together() {
    let mut run = quiet_room(true);
    rimeleap(&mut run, (22.0, 14.0));
    let struck = wait(&mut run, 6 * HZ);
    assert!(struck >= 4, "it went round the company: {struck}");
    assert!(run.cues.iter().any(|c| c == "frost_leap"));
}

#[test]
fn alone_a_knight_takes_rimeleap_once() {
    let mut run = quiet_room(false);
    rimeleap(&mut run, (22.0, 14.0));
    assert_eq!(wait(&mut run, 6 * HZ), 1, "never straight back");
    assert!(
        run.projectiles.iter().all(|p| p.kind != Shot::Frost),
        "the chain ends"
    );
}

#[test]
fn knights_spread_apart_break_the_chain() {
    let mut run = quiet_room(true);
    run.players.get_mut(&2).unwrap().y = 12.0 + lich::BOUNCE_REACH + 4.0;
    rimeleap(&mut run, (22.0, 14.0));
    assert_eq!(wait(&mut run, 6 * HZ), 1);
}

#[test]
fn a_chilled_knight_walks_slowly() {
    let walk = |chilled: bool| {
        let mut run = quiet_room(false);
        if chilled {
            run.players.get_mut(&1).unwrap().chilled = 10 * HZ;
        }
        let x = run.players[&1].x;
        for _ in 0..HZ / 2 {
            run.step(&BTreeMap::from([(
                1,
                Input {
                    move_x: 1,
                    ..Default::default()
                },
            )]));
        }
        run.players[&1].x - x
    };
    let (warm, cold) = (walk(false), walk(true));
    assert!(cold < warm * 0.75, "chilled {cold} vs {warm}");
}

#[test]
fn the_lich_looses_rimeleap_on_his_beat() {
    let mut run = quiet_room(true);
    run.spawn_at_for_test(EnemyKind::Lich, 30.0, 14.0);
    let mut chained = false;
    for _ in 0..8 * HZ {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        run.step(&BTreeMap::new());
        chained |= run
            .projectiles
            .iter()
            .any(|p| p.kind == Shot::Frost && p.pierce == lich::BOUNCES);
    }
    assert!(chained, "a Rimeleap in the air");
    assert!(run.cues.iter().any(|c| c == "rimeleap"));
    let has = |pack: Pack, depth: u32| {
        pack.roster_at(depth)
            .iter()
            .any(|&(kind, _)| kind == EnemyKind::Lich)
    };
    assert!(has(Pack::Archive, 4) && has(Pack::Unknown, 6));
    assert!(EnemyKind::Lich.elite());
}
