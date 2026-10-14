use super::*;
use crate::rules::cards::check;
use std::collections::BTreeMap;

fn fight(run: &mut Run) {
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(room);
}

/// A calm room, the knight in the middle, nothing else.
fn quiet() -> Run {
    let mut run = Run::new(4, 1, None);
    fight(&mut run);
    run.calm_for_test();
    run.enemies.clear();
    run.window = None;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 14.0);
    hero.hp = 100_000;
    hero.max_hp = 100_000;
    run
}

fn put(run: &mut Run, kind: EnemyKind, x: f32, y: f32) -> usize {
    run.spawn_at_for_test(kind, x, y);
    let enemy = run.enemies.last_mut().unwrap();
    enemy.age = TELEGRAPH + 5;
    enemy.hp = enemy.max_hp;
    run.enemies.len() - 1
}

fn learn(run: &mut Run, raw: &str) -> String {
    let card = check("", raw).unwrap().card;
    let id = card.id.clone();
    run.book.insert(card);
    id
}

#[test]
fn a_summons_is_bounded_by_the_checker() {
    let swarm = check("d", "name D\nkind spell\ncooldown 2\nsummon 9 swarm\n").unwrap();
    assert_eq!(swarm.card.imps(), Some((6, Behaviour::Swarm)));
    assert_eq!(swarm.card.cooldown, 24, "six swarmers: at least 24s");
    assert!(swarm.notes.iter().any(|n| n.contains("clamped to 6")));
    let turret = check("t", "name T\nkind play\nsummon 5 turret\n").unwrap();
    assert_eq!(turret.card.imps(), Some((2, Behaviour::Turret)));
    let errors = |raw: &str| check("x", raw).unwrap_err().errors.join(" | ");
    assert!(errors("name X\nkind hold\nsummon 2 swarm\n").contains("spell or play"));
    assert!(errors("name X\nkind take\nsummon 2 swarm\n").contains("spell or play"));
    assert!(errors("name X\nkind play\nsummon 2 dragons\n").contains("swarm, guard or turret"));
    assert!(
        errors("name X\nkind play\nsummon 2 swarm run-a-script\n")
            .contains("swarm, guard or turret")
    );
    // A play card's summons is a cast like a spell's; Effect::summon is the
    // hireling's ally slot and stays clear of it.
    assert!(turret.card.summon().is_none());
}

#[test]
fn swarmer_bursts_share_the_area_rule_and_kills_share_the_leech_cap() {
    let mut run = quiet();
    let id = learn(&mut run, "name Devils\nkind play\nsummon 1 swarm\n");
    run.call_imps(1, &id);
    let (x, y) = (run.imps[0].x, run.imps[0].y);
    for _ in 0..5 {
        put(&mut run, EnemyKind::Dummy, x, y);
    }
    let before: Vec<_> = run.enemies.iter().map(|e| e.hp).collect();
    let damage = run.players[&1].scaled(SWARM_BURST);
    run.tick_imps();
    for (rank, (e, hp)) in run.enemies.iter().zip(before).enumerate() {
        assert_eq!(hp - e.hp, aoe::part(damage, rank));
    }
    let hero = run.players.get_mut(&1).unwrap();
    hero.hp = 50;
    hero.bonus.vamp = cards::cap("vamp");
    for e in &mut run.enemies {
        e.hp = 1;
    }
    run.call_imps(1, &id);
    run.imps[0].x = x;
    run.imps[0].y = y;
    run.step(&BTreeMap::new());
    assert!(run.enemies.is_empty());
    assert!(run.players[&1].hp > 50 && run.players[&1].hp <= 50 + stakes::LEECH_PER_SECOND);
}

#[test]
fn summons_expire_and_do_not_continue_for_a_fallen_owner() {
    for behaviour in ["swarm", "guard", "turret"] {
        let mut run = quiet();
        let id = learn(
            &mut run,
            &format!("name Creature\nkind play\nsummon 1 {behaviour}\n"),
        );
        run.call_imps(1, &id);
        let life = run.imps[0].life;
        for _ in 0..life {
            run.tick_imps();
        }
        assert!(run.imps.is_empty(), "{behaviour} expired");
        run.call_imps(1, &id);
        run.players.get_mut(&1).unwrap().hp = 0;
        run.tick_imps();
        assert!(run.imps.is_empty(), "{behaviour} owner fell");
    }
}

#[test]
fn a_swarm_hunts_the_nearest_monster_and_bursts_on_it() {
    let mut run = quiet();
    let id = learn(
        &mut run,
        "name Golden Devils\nkind spell\ncooldown 8\nsummon 2 swarm\ndraw summon\n.5.|5.5\n555|555\n",
    );
    run.players.get_mut(&1).unwrap().spells[0] = Some(id.clone());
    let foe = put(&mut run, EnemyKind::Dummy, 30.0, 14.0);
    let before = run.enemies[foe].hp;
    run.step(&BTreeMap::from([(
        1,
        Input {
            cast: 1,
            ..Default::default()
        },
    )]));
    assert_eq!(run.imps.len(), 2, "two devils answer");
    assert!(run.imps.iter().all(|i| i.card == id && i.owner == 1));
    for _ in 0..90 {
        run.step(&BTreeMap::new());
        if run.imps.is_empty() {
            break;
        }
    }
    assert!(run.imps.is_empty(), "both burst");
    let dealt = before - run.enemies[foe].hp.min(before);
    let hero = &run.players[&1];
    assert!(dealt >= hero.scaled(SWARM_BURST), "{dealt}");
    assert!(
        dealt <= hero.scaled(SWARM_BURST) * 2,
        "two bursts, no more: {dealt}"
    );
}

#[test]
fn guards_catch_shots_and_turrets_loose_bolts() {
    let mut run = quiet();
    let id = learn(&mut run, "name Wardens\nkind play\nsummon 4 guard\n");
    run.call_imps(1, &id);
    assert_eq!(run.imps.len(), 4);
    run.tick_imps();
    let (gx, gy) = (run.imps[0].x, run.imps[0].y);
    // A monster's shot, right on the first guard.
    run.projectiles.push(Projectile {
        x: gx,
        y: gy,
        vx: 0.0,
        vy: 0.0,
        hostile: true,
        kind: Shot::Bolt,
        look: None,
        damage: 10,
        pierce: 0,
        last_hit: None,
        ttl: 30,
        empowered: false,
        traits: ShotTraits::default(),
    });
    let n = run.imps.len();
    // The guards are where they were a tick ago, within a hair.
    run.tick_imps();
    assert_eq!(run.imps.len(), n - 1, "one guard spends itself on the shot");
    assert!(run.projectiles.iter().all(|p| !p.hostile || p.ttl == 0));

    let mut run = quiet();
    let id = learn(
        &mut run,
        "name Ballista\nkind play\nsummon 1 turret\ndraw summon\n.J.\nJhJ\n",
    );
    run.call_imps(1, &id);
    put(&mut run, EnemyKind::Dummy, 30.0, 14.0);
    let mut loosed = 0;
    for _ in 0..60 {
        let before = run.projectiles.len();
        run.tick_imps();
        loosed += run.projectiles.len().saturating_sub(before);
    }
    assert!(loosed >= 2, "{loosed}");
    assert!(run.projectiles.iter().all(|p| !p.hostile));
}

#[test]
fn a_knight_keeps_six_and_a_new_room_leaves_them_behind() {
    let mut run = quiet();
    let id = learn(&mut run, "name Many\nkind play\nsummon 6 swarm\n");
    run.call_imps(1, &id);
    run.call_imps(1, &id);
    assert_eq!(run.imps.len(), MAX_PER_KNIGHT);
    let mut saved = run.book.get(&id).unwrap().clone();
    saved.effects = vec![cards::Effect::Summon(u32::MAX, Behaviour::Swarm)];
    run.book.insert(saved);
    run.call_imps(1, &id);
    assert_eq!(
        run.imps.len(),
        MAX_PER_KNIGHT,
        "saved counts are bounded before allocating"
    );
    fight(&mut run);
    assert!(run.imps.is_empty());
}

#[test]
fn a_play_card_calls_them_too_and_a_friend_sees_them() {
    let mut run = quiet();
    let id = learn(&mut run, "name Imp Bell\nkind play\nsummon 3 guard\n");
    run.players.get_mut(&1).unwrap().hand.push(id);
    run.step(&BTreeMap::from([(
        1,
        Input {
            play: 1,
            ..Default::default()
        },
    )]));
    assert_eq!(run.imps.len(), 3);
    let friend = run.wire_copy();
    assert_eq!(friend.imps, run.imps);
}
