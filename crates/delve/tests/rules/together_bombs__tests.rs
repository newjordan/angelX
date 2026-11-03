use super::super::*;
use std::collections::BTreeMap;

fn field(depth: u32) -> Run {
    let mut run = Run::new(4, 1, Some("Bree"));
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
    run.projectiles.clear();
    run.dungeon.depth = depth;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.bombs, hero.bomb_cooldown) = (10.0, 14.0, 2, 0);
    run
}

fn foe(run: &mut Run, x: f32, y: f32) -> u32 {
    run.spawn_at_for_test(EnemyKind::Imp, x, y);
    let enemy = run.enemies.last_mut().unwrap();
    (enemy.hp, enemy.max_hp) = (800, 800);
    enemy.id
}

#[test]
fn floor_six_bomb_halves_an_800_hp_demon() {
    let mut run = field(6);
    let id = foe(&mut run, 18.0, 14.0);
    let sortie = run.knights_turn(&BTreeMap::from([(
        1,
        Input {
            bomb: true,
            ..Default::default()
        },
    )]));
    run.bombs_burst(&sortie);
    assert_eq!(run.enemies.iter().find(|e| e.id == id).unwrap().hp, 400);
    assert_eq!(run.players[&1].bombs, 1);
    assert_eq!(run.players[&1].bomb_cooldown, BOMB_REARM);
}

#[test]
fn radius_grows_but_a_bomb_never_hits_the_whole_floor() {
    for (depth, near_damage) in [(1, 0), (6, 400)] {
        let mut run = field(depth);
        let near = foe(&mut run, 35.0, 14.0);
        let far = foe(&mut run, 43.0, 14.0);
        let sortie = run.knights_turn(&BTreeMap::from([(
            1,
            Input {
                bomb: true,
                ..Default::default()
            },
        )]));
        run.bombs_burst(&sortie);
        assert_eq!(
            run.enemies.iter().find(|e| e.id == near).unwrap().hp,
            800 - near_damage
        );
        assert_eq!(run.enemies.iter().find(|e| e.id == far).unwrap().hp, 800);
    }
}

#[test]
fn a_descent_restocks_each_knight_once() {
    let mut run = field(5);
    run.join_seat(2, "Wren");
    for hero in run.players.values_mut() {
        hero.bombs = 0;
    }
    run.finish_descent(6, Pack::Crypt, false);
    assert!(run.players.values().all(|h| h.bombs == 4));
    let room = run.at;
    run.enter_for_test(room);
    assert!(
        run.players.values().all(|h| h.bombs == 4),
        "room entry is no free restock"
    );
}

#[test]
fn the_six_fixed_provisions_clamp_town_and_endless_depths() {
    let expected = [
        (60, 12.0, 5, 2, 1, 1),
        (100, 16.0, 6, 4, 1, 1),
        (160, 20.0, 7, 6, 1, 2),
        (240, 24.0, 8, 8, 2, 2),
        (320, 28.0, 10, 10, 2, 3),
        (400, 32.0, 12, 12, 3, 4),
    ];
    for (depth, row) in (1..=6).zip(expected) {
        let p = bombs::at(depth);
        assert_eq!(
            (
                p.damage,
                p.radius,
                p.cap,
                p.drop_percent,
                p.drop_charges,
                p.restock
            ),
            row
        );
        assert!(p.cap <= MAX_BOMBS);
    }
    assert_eq!(bombs::at(0), bombs::at(1));
    assert_eq!(bombs::at(u32::MAX), bombs::at(6));
    assert_eq!(bombs::at(6).cap, MAX_BOMBS);
}

#[test]
fn only_overlapping_bombs_stack_and_holding_the_key_obeys_rearm() {
    let mut run = field(6);
    run.join_seat(2, "Wren");
    run.join_seat(3, "Bram");
    for (id, x) in [(1, 10.0), (2, 10.0), (3, 90.0)] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y, hero.bombs, hero.bomb_cooldown) = (x, 14.0, 2, 0);
    }
    let near = foe(&mut run, 18.0, 14.0);
    let remote = foe(&mut run, 86.0, 14.0);
    let inputs = BTreeMap::from([1, 2, 3].map(|id| {
        (
            id,
            Input {
                bomb: true,
                ..Default::default()
            },
        )
    }));
    let sortie = run.knights_turn(&inputs);
    run.bombs_burst(&sortie);
    assert_eq!(run.enemies.iter().find(|e| e.id == near).unwrap().hp, 0);
    assert_eq!(
        run.enemies.iter().find(|e| e.id == remote).unwrap().hp,
        400,
        "a distant ally's bombs cannot borrow this bomb's reach"
    );
    let again = run.knights_turn(&inputs);
    assert_eq!(again.bombs, 0);
    assert!(run.players.values().all(|h| h.bombs == 1));
}

#[test]
fn shot_clear_uses_the_same_depth_radius_and_leaves_friendly_shots() {
    for (depth, expected_hostile) in [(1, 2), (6, 1)] {
        let mut run = field(depth);
        for (x, hostile) in [(35.0, true), (43.0, true), (18.0, false)] {
            run.projectiles.push(Projectile {
                x,
                y: 14.0,
                vx: 0.0,
                vy: 0.0,
                hostile,
                kind: Shot::Orb,
                look: None,
                damage: 10,
                pierce: 0,
                last_hit: None,
                ttl: HZ,
                empowered: false,
                traits: Default::default(),
            });
        }
        let sortie = run.knights_turn(&BTreeMap::from([(
            1,
            Input {
                bomb: true,
                ..Default::default()
            },
        )]));
        run.bombs_burst(&sortie);
        assert_eq!(
            run.projectiles.iter().filter(|p| p.hostile).count(),
            expected_hostile
        );
        assert_eq!(run.projectiles.iter().filter(|p| !p.hostile).count(), 1);
    }
}

#[test]
fn six_knights_each_take_their_own_depth_scaled_bomb_share() {
    let mut run = field(6);
    for id in 2..=MAX_PLAYERS {
        run.join_seat(id, "Friend");
    }
    for hero in run.players.values_mut() {
        (hero.x, hero.y, hero.bombs, hero.stone, hero.idle) = (10.0, 14.0, 0, false, false);
    }
    run.provision_bombs(false);
    run.dungeon.rooms[run.at].items.clear();
    run.drop_item("bomb".into(), 10.0, 14.0, None);
    run.gather();
    assert!(
        run.players.values().all(|h| h.bombs == 3),
        "one share each, not one party-wide charge"
    );
    assert!(run.dungeon.rooms[run.at].items.is_empty());
    for hero in run.players.values_mut() {
        hero.bombs = 11;
    }
    run.drop_item("bomb".into(), 10.0, 14.0, None);
    run.gather();
    assert!(run.players.values().all(|h| h.bombs == MAX_BOMBS));
    run.drop_item("bomb".into(), 10.0, 14.0, None);
    run.gather();
    assert_eq!(
        run.dungeon.rooms[run.at].items.len(),
        1,
        "a full belt leaves the shared card for later"
    );
}

#[test]
fn old_heroes_restore_a_small_belt_and_grants_never_overflow_or_confiscate() {
    let mut encoded = serde_json::to_value(Hero::new("Bree", 10.0, 14.0)).unwrap();
    encoded.as_object_mut().unwrap().remove("bomb_capacity");
    let mut hero: Hero = serde_json::from_value(encoded).unwrap();
    assert_eq!(hero.bomb_limit(), 5);
    hero.give_bombs(u32::MAX);
    assert_eq!(hero.bombs, 5);
    hero.bomb_capacity = MAX_BOMBS;
    hero.give_bombs(u32::MAX);
    assert_eq!(hero.bombs, MAX_BOMBS);
    hero.bomb_capacity = 5;
    hero.give_bombs(1);
    assert_eq!(
        hero.bombs, MAX_BOMBS,
        "going shallower must not confiscate bombs"
    );
    hero.bombs = u32::MAX;
    hero.give_bombs(u32::MAX);
    assert_eq!(hero.bombs, MAX_BOMBS);
}

#[test]
fn depth_drop_draw_is_repeatable_more_generous_and_not_farmable_from_dummies() {
    fn drops(depth: u32, kind: EnemyKind) -> usize {
        let mut run = field(depth);
        run.rng = Rng::new(38);
        let mut count = 0;
        for _ in 0..1000 {
            run.dungeon.rooms[run.at].items.clear();
            run.drop_bombs(kind, 12.0, 12.0);
            count += run.dungeon.rooms[run.at].items.len();
        }
        count
    }
    let early = drops(1, EnemyKind::Imp);
    let late = drops(6, EnemyKind::Imp);
    assert!(early > 0 && late > early * 3);
    assert_eq!(late, drops(6, EnemyKind::Imp));
    assert_eq!(drops(6, EnemyKind::Dummy), 0);
    assert_eq!(drops(6, EnemyKind::Ward), 0);
}

#[test]
fn scroll_keeps_its_own_open_floor_reach_when_a_remote_bomb_goes_off() {
    let mut run = field(6);
    run.open_floors = true;
    run.finish_descent(6, Pack::Crypt, false);
    let map = run.dungeon.open.as_ref().unwrap().map;
    run.enter_for_test(map);
    assert!(run.open_map());
    run.enemies.clear();
    run.join_seat(2, "Wren");
    for (id, x) in [(1, 10.0), (2, 90.0)] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y, hero.bombs, hero.bomb_cooldown) = (x, 14.0, 2, 0);
    }
    let far = foe(&mut run, 90.0, 14.0);
    let scroll_only = foe(&mut run, 10.0, 14.0);
    let mut sortie = run.knights_turn(&BTreeMap::from([(
        2,
        Input {
            bomb: true,
            ..Default::default()
        },
    )]));
    sortie.nova = 100;
    sortie.blast_owner = 1;
    run.bombs_burst(&sortie);
    assert_eq!(run.enemies.iter().find(|e| e.id == far).unwrap().hp, 400);
    assert_eq!(
        run.enemies.iter().find(|e| e.id == scroll_only).unwrap().hp,
        700
    );
}

#[test]
fn bomb_hud_shows_the_current_floors_belt_not_the_absolute_ceiling() {
    let run = field(1);
    assert_eq!(run.hud(1, &hud::Keys::FIRST).unwrap().max_bombs, 5);
    let run = field(6);
    assert_eq!(run.hud(1, &hud::Keys::FIRST).unwrap().max_bombs, 12);
}
