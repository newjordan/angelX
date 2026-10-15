//! Co-op pressure (`press`, `stakes`' head-count tables) and how the arms
//! feel (`feel`): aggro that spreads, holds that wear off, bombs that clear
//! a circle, a harder rising, tempering, valour, coins at a room's end, and
//! the renders of it all.

use super::*;
use super::testing::*;

/// `quiet`, with knights 2 to `n` beside the first.
fn party(n: u32) -> Run {
    let mut run = quiet();
    for id in 2..=n {
        run.join_seat(id, &format!("K{id}"));
    }
    for hero in run.players.values_mut() {
        hero.invulnerable = 100_000;
    }
    run
}

fn shot(x: f32, y: f32, kind: Shot, vx: f32, hostile: bool) -> Projectile {
    Projectile {
        x,
        y,
        vx,
        vy: 0.0,
        kind,
        hostile,
        look: None,
        damage: 55,
        pierce: if kind == Shot::Bolt { u8::MAX } else { 0 },
        last_hit: None,
        ttl: HZ,
        empowered: false,
        traits: ShotTraits {
            owner: if hostile { 0 } else { 1 },
            arm: !hostile,
            ..Default::default()
        },
    }
}

// ─── aggro ───────────────────────────────────────────────────────────────────

#[test]
fn each_monster_sees_its_own_knight_any_knight_near_it_and_every_decoy() {
    let mut run = quiet();
    let id = a_foe(&mut run, EnemyKind::Skeleton, 24.0, 14.0);
    let enemy = run.enemies.iter().find(|e| e.id == id).unwrap().clone();
    let knights = vec![(1, 4.0, 4.0), (2, 44.0, 24.0), (3, 25.0, 15.0)];
    let decoys = vec![(10.0, 10.0)];
    let mut seen = Vec::new();
    press::sights(&enemy, &knights, &decoys, &mut seen);
    let own = knights[enemy.id as usize % 3];
    assert!(seen.contains(&(own.1, own.2)), "its own knight");
    assert!(seen.contains(&(25.0, 15.0)), "the knight beside it");
    assert!(seen.contains(&(10.0, 10.0)), "the decoy");
    // Hurt by knight 2, it turns on knight 2.
    let mut turned = enemy.clone();
    turned.wits.foe = 2;
    press::sights(&turned, &knights, &decoys, &mut seen);
    assert!(seen.contains(&(44.0, 24.0)), "{seen:?}");
    // Alone, a monster sees only the one knight, as it always did.
    press::sights(&enemy, &knights[..1], &[], &mut seen);
    assert_eq!(seen, vec![(4.0, 4.0)]);
}

#[test]
fn a_party_splits_the_monsters_between_its_knights() {
    let mut run = party(2);
    run.players.get_mut(&1).unwrap().x = 12.0;
    run.players.get_mut(&2).unwrap().x = 40.0;
    // Every imp starts nearer knight 1.
    for k in 0..6 {
        a_foe(&mut run, EnemyKind::Imp, 20.0, 8.0 + k as f32 * 2.0);
    }
    idle(&mut run, 3 * HZ);
    let near = |id: u32| {
        let h = &run.players[&id];
        run.enemies
            .iter()
            .filter(|e| {
                let mine = (e.x - h.x).hypot(e.y - h.y);
                run.players
                    .values()
                    .all(|o| (e.x - o.x).hypot(e.y - o.y) >= mine)
            })
            .count()
    };
    assert!(near(2) >= 2, "imps hunt knight 2 too: {}", near(2));
    assert!(near(1) >= 2, "and knight 1: {}", near(1));
}

// ─── head count ──────────────────────────────────────────────────────────────

#[test]
fn a_party_is_hit_harder_and_shot_at_thicker_on_every_floor() {
    assert_eq!(stakes::EARLY_PARTY_HIT, [100, 115, 130, 145]);
    assert_eq!(stakes::PARTY_FURY, [0, 1, 2, 3]);
    let mut solo = quiet();
    idle(&mut solo, 1);
    assert_eq!(
        solo.players[&1].incoming, 100,
        "a lone knight on floor 1 as ever"
    );
    let mut four = party(4);
    idle(&mut four, 1);
    assert_eq!(four.players[&1].incoming, 145);
    // Thicker fire: fury by head count, a guardian's too.
    let id = a_foe(&mut four, EnemyKind::Skeleton, 30.0, 10.0);
    let fury = four.enemies.iter().find(|e| e.id == id).unwrap().fury;
    assert_eq!(fury, stakes::party_fury(4));
    let id = a_foe(&mut solo, EnemyKind::Skeleton, 30.0, 10.0);
    assert_eq!(solo.enemies.iter().find(|e| e.id == id).unwrap().fury, 0);
    // A knight joining mid-room thickens what is already there.
    solo.join_seat(2, "Late");
    assert_eq!(solo.enemies.iter().find(|e| e.id == id).unwrap().fury, 1);
}

#[test]
fn a_party_meets_more_monsters_in_every_room() {
    let solo = {
        let mut run = Run::new(9, 3, None);
        let room = fight_room(&run);
        run.enter_for_test(room);
        run.enemies.len()
    };
    let three = {
        let mut run = Run::new(9, 3, None);
        run.join_seat(2, "B");
        run.join_seat(3, "C");
        let room = fight_room(&run);
        run.enter_for_test(room);
        run.enemies.len()
    };
    assert!(
        three >= solo + 2 * stakes::EXTRA_PER_KNIGHT - 1,
        "solo {solo}, three {three}"
    );
}

#[test]
fn head_count_damage_tables_cover_all_six_floors_without_cutting_health() {
    let hits = [
        [100, 115, 130, 145],
        [100, 115, 130, 145],
        [102, 170, 204, 229],
        [144, 240, 288, 324],
        [180, 300, 360, 405],
        [204, 340, 408, 459],
    ];
    for n in 1..=4 {
        let mut run = party(n);
        for (i, row) in hits.iter().enumerate() {
            run.dungeon.depth = i as u32 + 1;
            run.set_stakes();
            assert_eq!(run.players[&1].incoming, row[n as usize - 1]);
        }
    }
    let health = [70, 140, 210, 280];
    for n in 1..=4 {
        let mut run = party(n);
        run.spawn_at_for_test(EnemyKind::Skeleton, 34.0, 14.0);
        assert_eq!(run.enemies.last().unwrap().max_hp, health[n as usize - 1]);
    }
}

#[test]
fn a_guardians_room_gets_the_partys_extra_bodies_too() {
    let mut solo = quiet();
    solo.spawn_boss(0);
    let solo_hp = solo.enemies[0].max_hp;
    solo.deepen();
    assert_eq!(solo.enemies.len(), 1);
    let mut four = party(4);
    bare(&mut four);
    four.spawn_boss(0);
    four.deepen();
    assert!(four.enemies.len() >= 5, "{} foes", four.enemies.len());
    assert_eq!(four.enemies[0].max_hp, solo_hp * 4);
}

#[test]
fn a_party_floor_provisions_more_bodies_than_a_lone_knights() {
    let run = Run::new(9, 3, None);
    let economy = &run.boss_gates.as_ref().expect("a gated floor").economy;
    let mut spent = economy.clone();
    let mut solo = 0;
    while spent.can_emit_for(1) {
        spent.spawn_spent += 1;
        solo += 1;
    }
    let mut more = 0;
    while spent.can_emit_for(4) {
        spent.spawn_spent += 1;
        more += 1;
    }
    assert!(
        more > 0,
        "a party of four may raise more: {solo} then {more}"
    );
}

// ─── crowd control ───────────────────────────────────────────────────────────

#[test]
fn a_long_hold_leaves_a_monster_steady_for_a_while_but_a_jolt_still_lands() {
    let mut run = quiet();
    let id = a_foe(&mut run, EnemyKind::Skeleton, 34.0, 8.0);
    let foe = |run: &mut Run| run.enemies.iter_mut().find(|e| e.id == id).unwrap().clone();
    run.enemies.iter_mut().find(|e| e.id == id).unwrap().frozen = 75;
    idle(&mut run, 76);
    let held = foe(&mut run);
    assert_eq!(held.frozen, 0);
    assert!(held.wits.steady > 0, "{:?}", held.wits);
    // A second long stun slides off.
    run.enemies.iter_mut().find(|e| e.id == id).unwrap().frozen = 60;
    idle(&mut run, 1);
    assert_eq!(foe(&mut run).frozen, 0);
    // A hit's jolt still lands.
    run.enemies.iter_mut().find(|e| e.id == id).unwrap().frozen = feel::JOLT_HIT;
    idle(&mut run, 1);
    assert_eq!(foe(&mut run).frozen, feel::JOLT_HIT - 1);
    // Steady wears off.
    idle(&mut run, u32::from(press::STEADY));
    run.enemies.iter_mut().find(|e| e.id == id).unwrap().frozen = 60;
    idle(&mut run, 1);
    assert_eq!(foe(&mut run).frozen, 59);
}

#[test]
fn a_second_party_wide_hold_soon_after_the_first_is_short() {
    let mut run = party(2);
    run.players.get_mut(&1).unwrap().knight = Some("composer".into());
    run.players.get_mut(&2).unwrap().knight = Some("loop".into());
    a_foe(&mut run, EnemyKind::Skeleton, 26.0, 13.0);
    for hero in run.players.values_mut() {
        hero.ult_charge = ults::ULT_FULL;
    }
    let cast = |id: u32| {
        BTreeMap::from([(
            id,
            Input {
                ult: true,
                ..Input::default()
            },
        )])
    };
    run.step(&cast(1));
    assert_eq!(run.enemies[0].frozen, press::CHORD_HOLD.0);
    run.step(&cast(2));
    assert_eq!(run.spheres[0].left, press::STILL_TICKS.1 - 1, "an echo");
    assert!(run.cues.iter().any(|c| c == "ult_echo"));
}

#[test]
fn a_bomb_clears_the_monsters_shots_around_the_thrower_not_the_whole_room() {
    let mut run = quiet();
    a_foe(&mut run, EnemyKind::Skeleton, 44.0, 4.0);
    for (x, y) in [(27.0, 14.0), (24.0, 20.0), (4.0, 3.0), (44.0, 25.0)] {
        run.projectiles.push(Projectile {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            hostile: true,
            kind: Shot::Bone,
            look: None,
            damage: 4,
            pierce: 0,
            last_hit: None,
            ttl: 100,
            empowered: false,
            traits: ShotTraits::default(),
        });
    }
    run.players.get_mut(&1).unwrap().bombs = 1;
    run.step(&BTreeMap::from([(
        1,
        Input {
            bomb: true,
            ..Input::default()
        },
    )]));
    let left: Vec<(f32, f32)> = run
        .projectiles
        .iter()
        .filter(|p| p.hostile && p.vx == 0.0 && p.vy == 0.0)
        .map(|p| (p.x, p.y))
        .collect();
    assert_eq!(
        left,
        vec![(4.0, 3.0), (44.0, 25.0)],
        "only the far shots fly on"
    );
    assert!(BOMB_CLEAR > 8.0 && BOMB_CLEAR < 10.0);
}

// ─── rising, bonds and vigils ────────────────────────────────────────────────

#[test]
fn a_fallen_knight_rises_at_the_stairs_with_a_quarter_of_their_health() {
    let mut run = party(2);
    run.players.get_mut(&2).unwrap().hp = 0;
    run.descend_for_test();
    // A quarter, and what the next floor's first room gives everyone.
    let (one, two) = (&run.players[&1], &run.players[&2]);
    assert_eq!(REVIVE_SHARE, 25);
    assert!(two.hp < two.max_hp / 2, "{} of {}", two.hp, two.max_hp);
    assert!(two.hp < one.hp);
}

#[test]
fn a_cleared_room_pays_the_party_one_bond_not_one_each() {
    let mut run = Run::new(4, 1, Some("B"));
    run.join_seat(3, "C");
    let room = fight_room(&run);
    run.enter_for_test(room);
    for hero in run.players.values_mut() {
        hero.invulnerable = 100_000;
    }
    run.clear_for_test();
    let bonds: u32 = run
        .players
        .values()
        .map(|h| h.carried.get(Spoil::Bond))
        .sum();
    assert_eq!(bonds, 1);
    run.phase = Phase::Fighting;
    run.step(&BTreeMap::new());
    assert_eq!(
        run.players
            .values()
            .map(|h| h.carried.get(Spoil::Bond))
            .sum::<u32>(),
        1,
        "the room never pays another bond"
    );
}

#[test]
fn no_knight_kneels_to_a_vigil_mid_fight_but_one_may_wake_from_it() {
    let mut run = quiet();
    a_foe(&mut run, EnemyKind::Skeleton, 44.0, 4.0);
    run.phase = Phase::Fighting;
    let vigil = BTreeMap::from([(
        1,
        Input {
            vigil: true,
            ..Input::default()
        },
    )]);
    run.step(&vigil);
    assert!(!run.players[&1].vigil, "not mid-fight");
    run.step(&BTreeMap::new());
    run.players.get_mut(&1).unwrap().vigil = true;
    run.players.get_mut(&1).unwrap().stone = true;
    run.step(&vigil);
    assert!(!run.players[&1].vigil, "but waking is always allowed");
}

// ─── the arms ────────────────────────────────────────────────────────────────

#[test]
fn every_arm_is_fast_and_its_own_shape() {
    let pace = HERO_SPEED;
    for weapon in [Weapon::Bow, Weapon::Crossbow, Weapon::Handgonne] {
        let arms = weapon.arms();
        assert!(arms.speed >= 2.5 * pace, "{weapon:?} {}", arms.speed);
        // Damage a second stays near the old arms' 64-70.
        let dps = (arms.damage * arms.shots) as f32 * HZ as f32 / arms.cooldown as f32;
        assert!((64.0..=90.0).contains(&dps), "{weapon:?} {dps}");
    }
    assert_eq!(Weapon::Crossbow.arms().pierce, u8::MAX, "a rail");
    assert_eq!(Weapon::Handgonne.arms().shots, 5, "a fan");
    // A pull of each, aimed right.
    for (weapon, shots) in [
        (Weapon::Bow, 1),
        (Weapon::Crossbow, 1),
        (Weapon::Handgonne, 5),
    ] {
        let mut run = quiet();
        a_foe(&mut run, EnemyKind::Skeleton, 44.0, 4.0);
        run.players.get_mut(&1).unwrap().weapon = weapon;
        run.step(&BTreeMap::from([(1, fire(1, 0))]));
        let mine: Vec<&Projectile> = run.projectiles.iter().filter(|p| !p.hostile).collect();
        assert_eq!(mine.len(), shots, "{weapon:?}");
    }
    // The handgonne kicks its knight back, smokes and shakes the view.
    let mut run = quiet();
    run.players.get_mut(&1).unwrap().weapon = Weapon::Handgonne;
    let x = run.players[&1].x;
    run.step(&BTreeMap::from([(1, fire(1, 0))]));
    assert!(run.players[&1].x < x - 0.5, "recoil");
    assert!(run.feel.puffs.iter().any(|p| p.smoke));
    assert!(run.shake >= feel::SHAKE_SHOT - 1);
}

#[test]
fn a_forged_arm_is_never_slow() {
    let checked = cards::check(
        "slowpoke",
        "name Slowpoke\nkind arm\nrarity common\ntext Slow.\nbolt damage=20 speed=8 every=20 range=30\n",
    )
    .expect("a forged arm card");
    let Some(cards::Arm::Forged(weapon)) = checked.card.arm else {
        panic!("forged");
    };
    assert_eq!(weapon.bolt.unwrap().speed, cards::FORGED_SPEED);
    assert_eq!(cards::FORGED_SPEED, 36.0);
}

#[test]
fn a_hit_jolts_and_shoves_an_ordinary_monster_but_never_a_guardian() {
    let mut run = quiet();
    let id = a_foe(&mut run, EnemyKind::Skeleton, 30.0, 14.0);
    run.players.get_mut(&1).unwrap().weapon = Weapon::Handgonne;
    let before = run.enemies[0].x;
    for _ in 0..6 {
        run.step(&BTreeMap::from([(1, fire(1, 0))]));
    }
    let foe = run.enemies.iter().find(|e| e.id == id).unwrap();
    assert!(foe.x > before + 0.5, "shoved back: {before} -> {}", foe.x);
    assert!(foe.wits.flinch > 0, "jolted");
    assert_eq!(foe.wits.foe, 1, "and it turns on its shooter");
    assert!(!run.feel.impacts.is_empty(), "sparks");
}

#[test]
fn arrows_stick_in_the_stone_until_the_room_ends() {
    let mut run = quiet();
    a_foe(&mut run, EnemyKind::Skeleton, 44.0, 4.0);
    for _ in 0..HZ {
        run.step(&BTreeMap::from([(1, fire(-1, 0))]));
    }
    assert!(!run.feel.stuck.is_empty());
    let next = run.dungeon.rooms.len() - 1;
    run.enter_for_test(next);
    assert!(run.feel.stuck.is_empty());
}

#[test]
fn the_rooms_end_turns_the_monsters_shots_into_gold_for_the_nearest_knight() {
    let mut run = quiet();
    let id = a_foe(&mut run, EnemyKind::Skeleton, 44.0, 4.0);
    run.phase = Phase::Fighting;
    for k in 0..5 {
        run.projectiles.push(Projectile {
            x: 14.0 + k as f32 * 4.0,
            y: 9.0,
            vx: 0.0,
            vy: 0.0,
            hostile: true,
            kind: Shot::Bone,
            look: None,
            damage: 4,
            pierce: 0,
            last_hit: None,
            ttl: 300,
            empowered: false,
            traits: ShotTraits::default(),
        });
    }
    let gold = run.players[&1].carried.get(Spoil::Gold);
    run.step(&BTreeMap::new());
    let flying = run.projectiles.iter().filter(|p| p.hostile).count();
    assert!(flying >= 1, "{flying}");
    run.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 0;
    run.step(&BTreeMap::new());
    assert_eq!(run.feel.coins.len(), flying);
    assert!(run.projectiles.iter().all(|p| !p.hostile));
    idle(&mut run, 3 * HZ);
    assert!(run.feel.coins.is_empty());
    assert_eq!(
        run.players[&1].carried.get(Spoil::Gold),
        gold + flying as u32 * feel::COIN_GOLD
    );
}

// ─── tempering ───────────────────────────────────────────────────────────────

#[test]
fn taking_the_arm_in_hand_tempers_it_up_to_three_times() {
    let mut run = quiet();
    let take = BTreeMap::from([(
        1,
        Input {
            take: true,
            ..Input::default()
        },
    )]);
    for want in 1..=4u8 {
        let at = run.at;
        run.dungeon.rooms[at].items.push(Item {
            card: "bow".into(),
            x: 24.5,
            y: 14.0,
            held_off: None,
        });
        run.step(&take);
        run.step(&BTreeMap::new());
        assert_eq!(run.players[&1].temper, want.min(feel::TEMPER_MAX));
    }
    assert_eq!(run.room().items.len(), 1, "a fourth bow stays where it lay");
    // A thrice-tempered bow looses three streams, the outer two homing.
    a_foe(&mut run, EnemyKind::Skeleton, 44.0, 4.0);
    run.step(&BTreeMap::from([(1, fire(1, 0))]));
    let mine: Vec<&Projectile> = run.projectiles.iter().filter(|p| !p.hostile).collect();
    assert_eq!(mine.len(), 3);
    assert_eq!(mine.iter().filter(|p| p.traits.homing > 0).count(), 2);
    // Another arm starts untempered.
    let at = run.at;
    run.dungeon.rooms[at].items.clear();
    run.dungeon.rooms[at].items.push(Item {
        card: "crossbow".into(),
        x: 24.5,
        y: 14.0,
        held_off: None,
    });
    run.step(&take);
    assert_eq!(run.players[&1].weapon, Weapon::Crossbow);
    assert_eq!(run.players[&1].temper, 0);
    let hud = run.hud(1, &hud::Keys::FIRST).unwrap();
    assert_eq!(hud.temper, 0);
}

#[test]
fn a_tempered_handgonne_fires_seven_balls() {
    let mut run = quiet();
    a_foe(&mut run, EnemyKind::Skeleton, 44.0, 4.0);
    let hero = run.players.get_mut(&1).unwrap();
    hero.weapon = Weapon::Handgonne;
    hero.temper = 1;
    run.step(&BTreeMap::from([(1, fire(1, 0))]));
    assert_eq!(run.projectiles.iter().filter(|p| !p.hostile).count(), 7);
    let hud = run.hud(1, &hud::Keys::FIRST).unwrap();
    assert!(hud.weapon.name.ends_with(" II"), "{}", hud.weapon.name);
}

#[test]
fn a_rail_hits_every_body_crossed_in_a_tick_once_in_flight_order() {
    let mut run = quiet();
    bare(&mut run);
    let h = run.players.get_mut(&1).unwrap();
    (h.x, h.y, h.weapon) = (10.0, 14.0, Weapon::Crossbow);
    for x in [12.6, 12.2, 11.8, 11.4] {
        a_foe(&mut run, EnemyKind::Dummy, x, 14.0);
    }
    run.step(&BTreeMap::from([(1, fire(1, 0))]));
    let hp: Vec<u32> = run.enemies.iter().map(|e| e.hp).collect();
    assert_eq!(hp, vec![4986, 4945, 4945, 4945]);
    idle(&mut run, 3);
    assert_eq!(run.enemies.iter().map(|e| e.hp).collect::<Vec<_>>(), hp);
    assert!(!run.feel.tracers.is_empty());
}

#[test]
fn a_fast_rail_stops_at_a_wall_between_open_endpoints() {
    let mut run = quiet();
    bare(&mut run);
    let at = run.at;
    run.dungeon.rooms[at].set_for_test(10, 7, Tile::Block);
    let id = a_foe(&mut run, EnemyKind::Dummy, 23.0, 14.0);
    run.projectiles
        .push(shot(19.9, 14.0, Shot::Bolt, 70.0, false));
    idle(&mut run, 1);
    assert_eq!(run.enemies.iter().find(|e| e.id == id).unwrap().hp, 5000);
    assert!(run.projectiles.iter().all(|p| p.hostile));
    assert_eq!(run.feel.stuck.len(), 1);
    assert!((20.0..20.3).contains(&run.feel.stuck[0].x));
}

#[test]
fn a_twice_tempered_rail_splits_on_its_first_kill_and_a_third_tier_chains() {
    let mut run = quiet();
    bare(&mut run);
    let id = a_foe(&mut run, EnemyKind::Dummy, 27.0, 14.0);
    run.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 1;
    let next = a_foe(&mut run, EnemyKind::Dummy, 29.0, 16.0);
    a_foe(&mut run, EnemyKind::Skeleton, 44.0, 3.0);
    let h = run.players.get_mut(&1).unwrap();
    (h.weapon, h.temper) = (Weapon::Crossbow, 3);
    run.step(&BTreeMap::from([(1, fire(1, 0))]));
    assert_eq!(run.projectiles.iter().filter(|p| !p.hostile).count(), 3);
    assert!(
        run.enemies.iter().find(|e| e.id == next).unwrap().hp < 5000,
        "chain landed"
    );
    assert!(run.feel.deaths.iter().any(|d| d.ttl == feel::DEATH_TICKS));
    assert!(run.shake >= feel::SHAKE_KILL);
}

#[test]
fn tempered_handgonne_balls_bounce_and_burst() {
    let mut run = quiet();
    bare(&mut run);
    a_foe(&mut run, EnemyKind::Skeleton, 44.0, 3.0);
    let at = run.at;
    run.dungeon.rooms[at].set_for_test(0, 7, Tile::Block);
    let h = run.players.get_mut(&1).unwrap();
    (h.weapon, h.temper) = (Weapon::Handgonne, 2);
    run.step(&BTreeMap::from([(1, fire(-1, 0))]));
    assert!(
        run.projectiles
            .iter()
            .filter(|p| !p.hostile)
            .all(|p| p.traits.bounce == 1)
    );
    let mut ball = shot(2.2, 14.0, Shot::Ball, -34.0, false);
    ball.traits.bounce = 1;
    run.projectiles.clear();
    run.projectiles.push(ball);
    idle(&mut run, 1);
    assert!(run.projectiles[0].vx > 0.0);
    assert_eq!(run.projectiles[0].traits.bounce, 0);
    run.projectiles.clear();
    let first = a_foe(&mut run, EnemyKind::Dummy, 29.0, 14.0);
    let next = a_foe(&mut run, EnemyKind::Dummy, 29.0, 16.0);
    let mut ball = shot(27.0, 14.0, Shot::Ball, 34.0, false);
    ball.traits.temper = 3;
    run.projectiles.push(ball);
    idle(&mut run, 1);
    assert!(run.enemies.iter().find(|e| e.id == first).unwrap().hp < 5000);
    assert!(
        run.enemies.iter().find(|e| e.id == next).unwrap().hp < 5000,
        "burst landed"
    );
}

// ─── valour ──────────────────────────────────────────────────────────────────

#[test]
fn a_skill_spends_a_pip_of_valour_earned_by_hits_and_grazes() {
    let mut run = quiet();
    a_foe(&mut run, EnemyKind::Skeleton, 44.0, 4.0);
    let gas = BTreeMap::from([(
        1,
        Input {
            skill: 1,
            ..Input::default()
        },
    )]);
    run.step(&BTreeMap::new());
    run.step(&gas);
    assert!(run.works.clouds.is_empty(), "no valour, no skill");
    // Five hits earn a pip, whatever their damage.
    run.tick_feel(&[(1, 12); 5], &[]);
    assert_eq!(run.pips(1), 1);
    assert_eq!(run.hud(1, &hud::Keys::FIRST).unwrap().valour, 1);
    run.step(&BTreeMap::new());
    run.step(&gas);
    assert_eq!(run.works.clouds.len(), 1);
    assert_eq!(run.pips(1), 0);
    assert_eq!(
        run.works.kits[&1].rest[skills::GAS],
        skills::REST[skills::GAS]
    );
    assert!(skills::REST.iter().all(|&r| r <= 3 * HZ));
    // Valour tops out at three pips.
    run.tick_feel(&[(1, 12); 50], &[]);
    assert_eq!(run.pips(1), feel::PIPS);
}

#[test]
fn a_shot_that_passes_close_grazes_once() {
    let mut run = quiet();
    let hero = run.players.get_mut(&1).unwrap();
    hero.invulnerable = 0;
    let (hx, hy) = (hero.x, hero.y);
    // A foe held far off, so the room stays a fight.
    a_foe(&mut run, EnemyKind::Skeleton, 44.0, 4.0);
    run.enemies[0].frozen = 10_000;
    run.projectiles.push(Projectile {
        x: hx - 3.0,
        y: hy + 0.9,
        vx: 6.0,
        vy: 0.0,
        hostile: true,
        kind: Shot::Bone,
        look: None,
        damage: 4,
        pierce: 0,
        last_hit: None,
        ttl: 300,
        empowered: false,
        traits: ShotTraits::default(),
    });
    let hp = run.players[&1].hp;
    idle(&mut run, 2 * HZ);
    assert_eq!(run.players[&1].hp, hp, "never hit");
    assert_eq!(run.works.kits[&1].valour, feel::GRAZE_VALOUR, "grazed once");
}

#[test]
fn fire_arrows_burn_after_a_fire_pit_thrown_with_a_bow() {
    let mut run = quiet();
    let id = a_foe(&mut run, EnemyKind::Skeleton, 29.0, 14.0);
    run.tick_feel(&[(1, 12); 5], &[]);
    run.step(&BTreeMap::new());
    run.step(&BTreeMap::from([(
        1,
        Input {
            skill: 2,
            aim_x: -1,
            ..Input::default()
        },
    )]));
    assert!(run.found_line().is_some_and(|l| l.contains("Fire Arrows")));
    for _ in 0..20 {
        run.step(&BTreeMap::from([(1, fire(1, 0))]));
    }
    assert!(
        run.works
            .banes
            .iter()
            .any(|b| b.foe == id && b.kind == skills::Bane::Burn)
    );
}

#[test]
fn ricochet_turns_a_rail_off_a_flying_shield_once() {
    let mut run = quiet();
    bare(&mut run);
    a_foe(&mut run, EnemyKind::Dummy, 30.0, 19.0);
    a_foe(&mut run, EnemyKind::Skeleton, 44.0, 3.0);
    run.works.flights.push(skills::Flight {
        owner: 1,
        x: 26.0,
        y: 14.0,
        vx: 0.0,
        vy: 0.0,
        going: true,
        age: 0,
        power: 1,
        bounces: 0,
        links: 0,
        travelled: 0.0,
        struck: Vec::new(),
    });
    run.projectiles
        .push(shot(24.0, 14.0, Shot::Bolt, 70.0, false));
    idle(&mut run, 1);
    assert!(run.found_line().is_some_and(|l| l.contains("Ricochet")));
    assert!(run.projectiles[0].vy > 0.0);
    assert!(run.projectiles[0].traits.once);
}

#[test]
fn powder_keg_detonates_gas_when_a_handgonne_ball_crosses_it() {
    let mut run = quiet();
    bare(&mut run);
    let id = a_foe(&mut run, EnemyKind::Dummy, 30.0, 14.0);
    run.works.clouds.push(skills::Cloud {
        x: 29.0,
        y: 14.0,
        vx: 0.0,
        vy: 0.0,
        age: 60,
        owner: 1,
        power: 3,
        tint: 0,
    });
    run.projectiles
        .push(shot(27.0, 14.0, Shot::Ball, 34.0, false));
    idle(&mut run, 1);
    assert!(run.works.clouds.is_empty());
    assert!(run.found_line().is_some_and(|l| l.contains("Powder Keg")));
    let foe = run.enemies.iter().find(|e| e.id == id).unwrap();
    assert!(foe.hp < 5000);
    assert_eq!(foe.wits.foe, 1, "the flash draws aggro");
}

#[test]
fn feel_effects_and_tempering_survive_save_and_live_mirror() {
    let mut run = quiet();
    a_foe(&mut run, EnemyKind::Skeleton, 44.0, 4.0);
    let h = run.players.get_mut(&1).unwrap();
    (h.weapon, h.temper) = (Weapon::Handgonne, 2);
    run.tick_feel(&[(1, 12); 5], &[]);
    run.step(&BTreeMap::from([(1, fire(1, 0))]));
    let saved: Run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
    assert!(saved.valid_snapshot());
    assert_eq!(saved.feel.puffs, run.feel.puffs);
    assert_eq!(run.wire_copy().feel.puffs, run.feel.puffs);
    assert_eq!(saved.players[&1].temper, 2);
    assert_eq!(saved.pips(1), 1);
}

// ─── renders ─────────────────────────────────────────────────────────────────

#[test]
fn tempered_arms_keep_card_looks_and_the_tray_uses_each_seats_keys() {
    for (weapon, arm, count) in [
        (Weapon::Bow, "bow", 3),
        (Weapon::Crossbow, "crossbow", 1),
        (Weapon::Handgonne, "handgonne", 7),
    ] {
        let mut run = quiet();
        bare(&mut run);
        a_foe(&mut run, EnemyKind::Skeleton, 44.0, 4.0);
        let card = cards::check(
            "comet",
            "name Comet\nkind hold\ndamage 5\ndraw shot\n.2.\n232\n",
        )
        .unwrap()
        .card;
        run.book.insert(card);
        let hero = run.players.get_mut(&1).unwrap();
        (hero.weapon, hero.temper) = (weapon, 2);
        hero.arm = Some(arm.into());
        hero.deck.push("comet".into());
        hero.hand.push("potion".into());
        run.tick_feel(&[(1, 12); 5], &[]);
        run.step(&BTreeMap::from([(1, fire(1, 0))]));
        let shots: Vec<_> = run.projectiles.iter().filter(|p| !p.hostile).collect();
        assert_eq!(shots.len(), count);
        assert!(
            shots
                .iter()
                .all(|p| p.sprite_card() == Some("comet") && p.traits.arm && p.traits.temper == 2)
        );
        for keys in [&hud::Keys::FIRST, &hud::Keys::SECOND] {
            let icons = run.tray(1, keys);
            let weapon = &icons[0];
            assert_eq!(weapon.key, keys.fire);
            assert_eq!((weapon.count, weapon.pips), (2, 1));
            assert!(weapon.name.ends_with(" III"));
            let bomb = icons.iter().find(|i| i.slot == tray::Slot::Bomb).unwrap();
            let card = icons.iter().find(|i| i.slot == tray::Slot::Card).unwrap();
            assert_eq!(bomb.key, keys.bomb);
            assert_eq!(card.key, keys.quaff);
            assert_eq!(
                icons.iter().filter(|i| i.slot == tray::Slot::Bomb).count(),
                1
            );
        }
    }
}

/// A row of tough skeletons across the room's far side, standing still.
fn targets(run: &mut Run) {
    for k in 0..5 {
        let id = a_foe(run, EnemyKind::Skeleton, 36.0, 6.0 + k as f32 * 4.0);
        let e = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
        e.frozen = 10_000;
        e.wits.steady = 0;
    }
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_feel_shots -- --ignored`
#[test]
#[ignore]
fn write_feel_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    // The bow's stream, tempered twice: three lanes of quick arrows.
    let mut run = quiet();
    bare(&mut run);
    targets(&mut run);
    run.players.get_mut(&1).unwrap().x = 12.0;
    run.players.get_mut(&1).unwrap().temper = 2;
    for n in 0..24 {
        run.step(&BTreeMap::from([(1, fire(1, 0))]));
        for e in &mut run.enemies {
            e.frozen = e.frozen.max(100);
        }
        if n == 23 {
            assert!(run.projectiles.iter().filter(|p| !p.hostile).count() >= 6);
            save(&frame(&run), &dir, "stream");
        }
    }
    // The crossbow's rail: the tracer through the line, bolts in the wall.
    let mut run = quiet();
    bare(&mut run);
    targets(&mut run);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.weapon, hero.temper) = (10.0, 14.0, Weapon::Crossbow, 1);
    hero.arm = Some("crossbow".into());
    for n in 0..30 {
        let aim = if n < 12 { fire(1, 0) } else { fire(1, -1) };
        run.step(&BTreeMap::from([(1, aim)]));
        if [3, 10, 26].contains(&n) {
            save(&frame(&run), &dir, &format!("rail_{n:02}"));
        }
    }
    // The handgonne's fan: muzzle flash, smoke, sparks, a shoved line.
    let mut run = quiet();
    bare(&mut run);
    targets(&mut run);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.weapon) = (26.0, 14.0, Weapon::Handgonne);
    hero.arm = Some("handgonne".into());
    for e in &mut run.enemies {
        e.wits.flinch = 0;
    }
    run.step(&BTreeMap::from([(1, fire(1, 0))]));
    save(&frame(&run), &dir, "fan_00");
    run.step(&BTreeMap::new());
    run.step(&BTreeMap::new());
    save(&frame(&run), &dir, "fan_02");
    run.step(&BTreeMap::new());
    save(&frame(&run), &dir, "fan_03");
    for _ in 4..=7 {
        run.step(&BTreeMap::new());
    }
    assert!(!run.feel.impacts.is_empty(), "the fan has landed");
    save(&frame(&run), &dir, "sparks");
    // A death's white frame, then its burst.
    let mut run = quiet();
    bare(&mut run);
    let id = a_foe(&mut run, EnemyKind::Skeleton, 30.0, 14.0);
    run.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 1;
    a_foe(&mut run, EnemyKind::Skeleton, 44.0, 3.0);
    run.enemies.last_mut().unwrap().frozen = 10_000;
    let hero = run.players.get_mut(&1).unwrap();
    hero.weapon = Weapon::Crossbow;
    hero.arm = Some("crossbow".into());
    run.step(&BTreeMap::from([(1, fire(1, 0))]));
    for n in 0..8 {
        if [1, 5].contains(&n) {
            save(&frame(&run), &dir, &format!("death_{n}"));
        }
        run.step(&BTreeMap::new());
    }
    // The room's end: the monsters' shots in the air become coins.
    let mut run = quiet();
    bare(&mut run);
    let id = a_foe(&mut run, EnemyKind::Skeleton, 40.0, 6.0);
    run.phase = Phase::Fighting;
    for k in 0..14 {
        let a = k as f32 / 14.0 * std::f32::consts::TAU;
        run.projectiles.push(Projectile {
            x: 34.0 + a.cos() * 7.0,
            y: 12.0 + a.sin() * 6.0,
            vx: a.cos() * 3.0,
            vy: a.sin() * 3.0,
            hostile: true,
            kind: Shot::Orb,
            look: None,
            damage: 4,
            pierce: 0,
            last_hit: None,
            ttl: 300,
            empowered: false,
            traits: ShotTraits::default(),
        });
    }
    save(&frame(&run), &dir, "coins_00");
    run.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 0;
    run.step(&BTreeMap::new());
    assert!(!run.feel.coins.is_empty(), "the room paid out in coins");
    save(&frame(&run), &dir, "coins_01");
    idle(&mut run, 9);
    save(&frame(&run), &dir, "coins_10");
}

#[test]
fn a_forked_shot_splits_past_its_mark_twice_and_fart_bombs_leave_a_cloud() {
    let mut run = quiet();
    bare(&mut run);
    let mark = a_foe(&mut run, EnemyKind::Dummy, 27.0, 14.0);
    a_foe(&mut run, EnemyKind::Skeleton, 44.0, 3.0);
    let h = run.players.get_mut(&1).unwrap();
    (h.bonus.fork, h.bonus.fart) = (2, 1);
    run.step(&BTreeMap::from([(1, fire(1, 0))]));
    let (mut once, mut clouds) = (0, 0);
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
        let forked: Vec<_> = run.projectiles.iter().filter(|p| !p.hostile && p.traits.fork == 1).collect();
        assert!(forked.iter().all(|p| p.last_hit == Some(mark)), "a fork flies past its mark");
        once = once.max(forked.len());
        clouds = clouds.max(run.works.clouds.iter().filter(|c| c.owner == 1).count());
    }
    assert_eq!(once, 2, "one shot forks into two");
    assert_eq!(clouds, 1, "the hit farts, once per rest");
    // A shot without the card stops where it hits.
    let mut run = quiet();
    bare(&mut run);
    a_foe(&mut run, EnemyKind::Dummy, 27.0, 14.0);
    a_foe(&mut run, EnemyKind::Skeleton, 44.0, 3.0);
    run.step(&BTreeMap::from([(1, fire(1, 0))]));
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
        assert!(run.projectiles.iter().filter(|p| !p.hostile).count() <= 1);
        assert!(run.works.clouds.is_empty());
    }
}
