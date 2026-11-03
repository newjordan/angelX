use super::*;
use crate::rules::hireling::testing::{fight_with, foe_at};

fn field(kind: Kind, level: u8) -> Run {
    let mut run = fight_with(false, &[]);
    let name = run.players[&1].name.clone();
    run.home.squire_company.rosters.insert(
        name,
        Roster {
            squires: vec![Squire {
                id: 7,
                name: "Pip".into(),
                kind,
                level,
                trained_on: None,
            }],
            active: Some(7),
            ..Default::default()
        },
    );
    run.deploy_company_squires();
    run
}

const ROLES: [Kind; 5] = [
    Kind::Ninja,
    Kind::Healer,
    Kind::Ranger,
    Kind::Shield,
    Kind::Alchemist,
];

#[test]
fn every_paid_level_has_fixed_stats_for_all_five_roles_and_invalid_levels_are_capped() {
    for kind in ROLES {
        for depth in [1, 3, 6] {
            for (raw, trained) in
                (1..=10)
                    .map(|n| (n, u32::from(n)))
                    .chain([(0, 1), (11, 10), (u8::MAX, 10)])
            {
                let mut run = field(kind, raw);
                run.dungeon.depth = depth;
                run.deploy_company_squires();
                let actor = &run.squire_field.companions[&1].actor;
                let hp = (48 + 16 * trained + 8 * depth) * if kind == Kind::Shield { 2 } else { 1 };
                assert_eq!((actor.hp, actor.max_hp), (hp, hp), "{kind:?} level {raw}");
                assert_eq!(actor.power, 6 + 3 * trained + 2 * depth);
                // Deployment must not repair or rewrite the persistent book.
                assert_eq!(
                    run.home.squire_company.rosters[&run.players[&1].name].squires[0].level,
                    raw
                );
            }
        }
    }
}

#[test]
fn levels_three_four_and_ten_deliver_real_damage_and_healing_not_just_sheet_stats() {
    for (trained, hp, power, mend) in [(3, 104, 17, 15), (4, 120, 20, 18), (10, 216, 38, 36)] {
        for kind in ROLES {
            let mut run = field(kind, trained);
            let hero = run.players.get_mut(&1).unwrap();
            (hero.hp, hero.max_hp) = (1, 1000);
            let actor = &mut run.squire_field.companions.get_mut(&1).unwrap().actor;
            (actor.x, actor.y) = (14.0, 12.0);
            foe_at(&mut run, EnemyKind::Goblin, (15.5, 12.0), 300);
            let actor = &mut run.squire_field.companions.get_mut(&1).unwrap().actor;
            actor.hurt(100);
            let health = hp * if kind == Kind::Shield { 2 } else { 1 };
            assert_eq!(actor.hp, health - 100, "{kind:?} level {trained} wound");
            let before = run.enemies[0].hp;
            run.tick_company_squires();
            match kind {
                Kind::Healer => assert_eq!(run.players[&1].hp, 1 + mend),
                Kind::Ranger => {
                    let shot = run
                        .projectiles
                        .iter()
                        .find(|p| p.kind == Shot::Arrow)
                        .unwrap();
                    assert_eq!(shot.damage, power * 150 / 100);
                    for _ in 0..8 {
                        fly(&mut run);
                    }
                    assert_eq!(run.enemies[0].hp, before - power * 150 / 100);
                }
                Kind::Ninja | Kind::Shield | Kind::Alchemist => {
                    assert_eq!(
                        run.enemies[0].hp,
                        before - power,
                        "{kind:?} level {trained}"
                    );
                    if kind == Kind::Alchemist {
                        assert_eq!(run.players[&1].hp, 1 + mend);
                    }
                }
            }
        }
    }
}

fn six_companions() -> Run {
    let mut run = field(Kind::Ninja, 3);
    let roster = run.home.squire_company.rosters[&run.players[&1].name].clone();
    for id in 2..=6 {
        let mut hero = run.players[&1].clone();
        hero.name = format!("Knight {id}");
        let mut roster = roster.clone();
        roster.squires[0].kind = ROLES[(id as usize - 1) % ROLES.len()];
        run.home.squire_company.rosters.insert(hero.name.clone(), roster);
        run.players.insert(id, hero);
    }
    run.deploy_company_squires();
    run
}

fn assert_breathing_room(run: &Run) {
    for (&owner, s) in &run.squire_field.companions {
        let at = (s.actor.x, s.actor.y);
        for h in run.players.values() {
            assert!(
                (at.0 - h.x).hypot(at.1 - h.y) >= 1.6,
                "owner {owner}: knight pile at {at:?}"
            );
        }
        for (&peer, other) in &run.squire_field.companions {
            if peer != owner {
                assert!(
                    (at.0 - other.actor.x).hypot(at.1 - other.actor.y) >= 1.6,
                    "owners {owner}/{peer}: companion pile at {at:?}"
                );
            }
        }
    }
}

#[test]
fn six_companions_separate_from_knights_and_peers_deterministically_without_shoving_owners() {
    let mut run = six_companions();
    let players = serde_json::to_value(&run.players).unwrap();
    // An exact pile has no geometric direction: ties must still be stable.
    for s in run.squire_field.companions.values_mut() {
        (s.actor.x, s.actor.y) = (12.0, 12.0);
    }
    let mut restored: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    for _ in 0..180 {
        run.tick_company_squires();
        restored.tick_company_squires();
    }
    assert_breathing_room(&run);
    assert_eq!(run.squires, restored.squires);
    assert_eq!(serde_json::to_value(&run.players).unwrap(), players);
    // Following a doorway must not permanently recreate the deployment pile.
    run.company_squires_follow();
    for _ in 0..120 {
        run.tick_company_squires();
    }
    assert_breathing_room(&run);
}

#[test]
fn seed_eleven_floors_three_and_six_keep_six_companions_readable_on_real_ticks() {
    for depth in [3, 6] {
        let mut run = six_companions();
        run.seed = 11;
        run.dungeon = crawl_floor(depth, Pack::Crypt, 11);
        let room = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Fight)
            .unwrap();
        run.enter_for_test(room);
        run.calm_for_test();
        run.enemies.clear();
        let grid = Grid {
            room: run.room(),
            barred: run.barred(),
        };
        // Keep the generated stone: choose a legal pocket, not an open-room fixture.
        let at = (1..run.room().rows - 1)
            .flat_map(|row| {
                (1..run.room().cols - 1).map(move |col| {
                    (
                        (col as f32 + 0.5) * TILE_UNITS,
                        (row as f32 + 0.5) * TILE_UNITS,
                    )
                })
            })
            .find(|&(x, y)| {
                (-2..=2).all(|dy| {
                    (-2..=2).all(|dx| {
                        grid.clear(x + dx as f32, y + dy as f32, HERO_RADIUS, Mover::Hero)
                    })
                })
            })
            .expect("generated fight room has breathing room");
        for hero in run.players.values_mut() {
            (hero.x, hero.y) = at;
        }
        run.deploy_company_squires();
        for s in run.squire_field.companions.values_mut() {
            (s.actor.x, s.actor.y) = at;
        }
        for _ in 0..180 {
            run.step(&BTreeMap::new());
            let grid = Grid {
                room: run.room(),
                barred: run.barred(),
            };
            for (&owner, s) in &run.squire_field.companions {
                let hero = &run.players[&owner];
                assert_eq!(
                    (hero.x, hero.y),
                    at,
                    "separation shoved knight on floor {depth}"
                );
                assert!(grid.clear(s.actor.x, s.actor.y, HERO_RADIUS, Mover::Hero));
                assert!((s.actor.x - hero.x).hypot(s.actor.y - hero.y) <= LEASH);
            }
        }
        assert_eq!(run.squire_field.companions.len(), 6);
        assert_breathing_room(&run);
    }
}

#[test]
fn six_companions_keep_breathing_room_while_fighting_beside_their_knights() {
    let mut run = six_companions();
    foe_at(&mut run, EnemyKind::Goblin, (13.5, 12.0), 1000);
    (run.enemies[0].hp, run.enemies[0].max_hp) = (100000, 100000);
    for s in run.squire_field.companions.values_mut() {
        (s.actor.x, s.actor.y) = (12.0, 12.0);
    }
    for _ in 0..180 {
        run.tick_company_squires();
    }
    assert!(run.enemies[0].hp < 100000);
    assert_breathing_room(&run);
}

#[test]
fn separation_yields_to_walls_barred_doors_and_the_six_tile_leash() {
    for tile in [Tile::Wall, Tile::Block, Tile::Door, Tile::Hazard] {
        let mut run = six_companions();
        // A vertical barrier beside a pile on legal ground.
        for row in 1..run.room().rows - 1 {
            run.dungeon.rooms[run.at].set(7, row, tile);
        }
        foe_at(&mut run, EnemyKind::Goblin, (20.0, 12.0), 300);
        for s in run.squire_field.companions.values_mut() {
            (s.actor.x, s.actor.y) = (13.5, 12.0);
        }
        for _ in 0..180 {
            run.tick_company_squires();
            let grid = Grid {
                room: run.room(),
                barred: run.barred(),
            };
            assert!(grid.barred);
            for (&owner, s) in &run.squire_field.companions {
                let h = &run.players[&owner];
                assert!(grid.clear(s.actor.x, s.actor.y, HERO_RADIUS, Mover::Hero));
                assert!(s.actor.x <= 14.0 - HERO_RADIUS, "clipped {tile:?}");
                assert!((s.actor.x - h.x).hypot(s.actor.y - h.y) <= LEASH);
            }
        }
    }
    let mut run = six_companions();
    for s in run.squire_field.companions.values_mut() {
        (s.actor.x, s.actor.y) = (12.0 + LEASH - 0.01, 12.0);
    }
    for _ in 0..120 {
        run.tick_company_squires();
        for s in run.squire_field.companions.values() {
            assert!((s.actor.x - 12.0).hypot(s.actor.y - 12.0) <= LEASH);
        }
    }
}

#[test]
fn separation_moves_only_squires_away_from_hirelings_and_raised_friendlies() {
    let mut run = field(Kind::Shield, 3);
    run.hireling = Some(Hireling::new(1, (13.0, 12.0)));
    run.minions
        .raised
        .push(Hireling::minion(AllyKind::Ghoul, 7, 1, (12.0, 13.0), 1));
    let hireling = run.hireling.clone();
    let minions = run.minions.clone();
    let actor = &mut run.squire_field.companions.get_mut(&1).unwrap().actor;
    (actor.x, actor.y) = (13.0, 12.0);
    for _ in 0..120 {
        run.tick_company_squires();
    }
    let actor = &run.squire_field.companions[&1].actor;
    for friend in run.hireling.iter().chain(run.minions.raised.iter()) {
        assert!((actor.x - friend.x).hypot(actor.y - friend.y) >= 0.8);
    }
    assert_eq!(run.hireling, hireling);
    assert_eq!(run.minions, minions);
}

#[test]
fn breathing_room_does_not_stop_shield_or_ninja_melee() {
    for kind in [Kind::Shield, Kind::Ninja] {
        let mut run = field(kind, 4);
        foe_at(&mut run, EnemyKind::Goblin, (13.0, 12.0), 1000);
        let foe = &mut run.enemies[0];
        (foe.hp, foe.max_hp) = (10000, 10000);
        let actor = &mut run.squire_field.companions.get_mut(&1).unwrap().actor;
        (actor.x, actor.y) = (12.0, 12.0);
        for _ in 0..180 {
            run.tick_company_squires();
        }
        assert!(
            run.enemies[0].hp < 10000 - 2 * 20,
            "{kind:?} lost melee access"
        );
        let actor = &run.squire_field.companions[&1].actor;
        assert!((actor.x - 13.0).hypot(actor.y - 12.0) <= REACH + run.enemies[0].radius());
        assert_breathing_room(&run);
    }
}

#[test]
fn deployment_is_one_per_owner_and_leaves_summons_alone() {
    let mut run = field(Kind::Ninja, 1);
    run.hireling = Some(Hireling::new(1, (11.0, 12.0)));
    run.minions
        .raised
        .push(Hireling::minion(AllyKind::Ghoul, 7, 1, (11.0, 11.0), 1));
    let minions = run.minions.clone();
    let hireling = run.hireling.clone();
    for id in 2..=6 {
        let mut hero = run.players[&1].clone();
        hero.name = format!("Knight {id}");
        run.home.squire_company.rosters.insert(
            hero.name.clone(),
            run.home.squire_company.rosters[&run.players[&1].name].clone(),
        );
        run.players.insert(id, hero);
    }
    // Even an invalid seventh seat cannot bring a seventh companion.
    let mut seventh = run.players[&1].clone();
    seventh.name = "Seventh".into();
    run.home.squire_company.rosters.insert(
        seventh.name.clone(),
        run.home.squire_company.rosters[&run.players[&1].name].clone(),
    );
    run.players.insert(7, seventh);
    run.deploy_company_squires();
    run.deploy_company_squires();
    assert_eq!(run.squire_field.companions.len(), 6);
    for (&owner, s) in &run.squire_field.companions {
        assert_eq!(s.actor.summoner, Some(owner));
    }
    assert_eq!(run.minions, minions);
    assert_eq!(run.hireling, hireling);
}

#[test]
fn all_five_roles_have_distinct_work_without_a_hireling_slot() {
    for kind in [
        Kind::Ninja,
        Kind::Healer,
        Kind::Ranger,
        Kind::Shield,
        Kind::Alchemist,
    ] {
        let mut run = field(kind, 3);
        run.players.get_mut(&1).unwrap().hp = 20;
        let actor = &mut run.squire_field.companions.get_mut(&1).unwrap().actor;
        (actor.x, actor.y) = (14.0, 12.0);
        foe_at(&mut run, EnemyKind::Goblin, (15.5, 12.0), 300);
        let hp = run.enemies[0].hp;
        run.tick_hireling();
        assert!(run.hireling.is_none());
        match kind {
            Kind::Healer => assert!(run.players[&1].hp > 20),
            Kind::Ninja | Kind::Shield => assert!(run.enemies[0].hp < hp),
            Kind::Ranger => {
                assert_eq!(run.squire_field.companions[&1].mark, Some(run.enemies[0].id));
                assert!(
                    run.projectiles
                        .iter()
                        .any(|p| !p.hostile && p.traits.owner == 1)
                );
            }
            Kind::Alchemist => assert!(run.players[&1].hp > 20 && run.enemies[0].hp < hp),
        }
    }
}

#[test]
fn healer_chooses_health_ratio_not_missing_or_absolute_health() {
    let mut run = field(Kind::Healer, 2);
    let mut other = run.players[&1].clone();
    other.name = "Other".into();
    (other.hp, other.max_hp) = (80, 400);
    run.players.insert(2, other);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.hp, hero.max_hp) = (30, 100);
    run.players.get_mut(&2).unwrap().hexed = 90;
    run.tick_company_squires();
    assert_eq!(run.players[&1].hp, 30);
    assert!(run.players[&2].hp > 80);
    assert_eq!(run.players[&2].hexed, 0);
}

#[test]
fn fallen_squires_recover_only_after_the_fight_or_on_a_new_floor() {
    let mut run = field(Kind::Shield, 1);
    foe_at(&mut run, EnemyKind::Goblin, (18.0, 12.0), 300);
    run.squire_field.companions.get_mut(&1).unwrap().actor.hp = 0;
    run.tick_company_squires();
    assert_eq!(run.squire_field.companions[&1].actor.hp, 0);
    run.enemies.clear();
    run.tick_company_squires();
    assert!(run.squire_field.companions[&1].actor.hp > 0);
    run.squire_field.companions.get_mut(&1).unwrap().actor.hp = 0;
    run.dungeon.depth += 1;
    run.deploy_company_squires();
    assert_eq!(
        run.squire_field.companions[&1].actor.hp,
        run.squire_field.companions[&1].actor.max_hp
    );
}

#[test]
fn healer_assists_a_settled_skull_but_never_raises_a_knight() {
    let mut run = field(Kind::Healer, 3);
    let mut fallen = run.players[&1].clone();
    fallen.name = "Fallen".into();
    fallen.hp = 0;
    fallen.fell = 1;
    fallen.skull_at = Some((15.0, 12.0));
    run.players.insert(2, fallen);
    run.tick = skulls::SKULL_SETTLE + 1;
    let actor = &mut run.squire_field.companions.get_mut(&1).unwrap().actor;
    (actor.x, actor.y) = (15.0, 12.0);
    run.tick_company_squires();
    assert_eq!(run.players[&2].hp, 0);
    assert!(run.players[&2].skull_at.unwrap().0 < 15.0);
}

fn hostile(at: (f32, f32), velocity: (f32, f32), damage: u32) -> Projectile {
    Projectile {
        x: at.0,
        y: at.1,
        vx: velocity.0,
        vy: velocity.1,
        hostile: true,
        kind: Shot::Ember,
        look: None,
        damage,
        pierce: 0,
        last_hit: None,
        ttl: HZ,
        empowered: false,
        traits: ShotTraits::default(),
    }
}

fn fly(run: &mut Run) {
    let mut sortie = run.knights_turn(&BTreeMap::new());
    run.shots_fly(&mut sortie);
}

#[test]
fn shield_body_blocks_a_fast_projectile_before_its_owner_and_pays_health() {
    let mut run = field(Kind::Shield, 1);
    let actor = &mut run.squire_field.companions.get_mut(&1).unwrap().actor;
    (actor.x, actor.y, actor.invulnerable) = (14.0, 12.0, 0);
    let (hp, knight) = (actor.hp, run.players[&1].hp);
    run.projectiles
        .push(hostile((17.0, 12.0), (-240.0, 0.0), 20));
    fly(&mut run);
    assert_eq!(run.players[&1].hp, knight);
    assert_eq!(run.squire_field.companions[&1].actor.hp, hp - 10);
    assert!(run.projectiles.iter().all(|p| p.ttl == 0));
}

#[test]
fn a_squire_behind_the_owner_cannot_block_a_hit_already_reaching_the_owner() {
    let mut run = field(Kind::Shield, 1);
    let actor = &mut run.squire_field.companions.get_mut(&1).unwrap().actor;
    (actor.x, actor.y, actor.invulnerable) = (9.0, 12.0, 0);
    let (hp, knight) = (actor.hp, run.players[&1].hp);
    run.projectiles
        .push(hostile((17.0, 12.0), (-240.0, 0.0), 20));
    fly(&mut run);
    assert!(run.players[&1].hp < knight);
    assert_eq!(run.squire_field.companions[&1].actor.hp, hp);
}

#[test]
fn all_roles_can_be_downed_by_projectiles_without_roster_loss_or_drops() {
    for kind in [
        Kind::Ninja,
        Kind::Healer,
        Kind::Ranger,
        Kind::Shield,
        Kind::Alchemist,
    ] {
        let mut run = field(kind, 1);
        let roster = run.home.squire_company.clone();
        let drops = serde_json::to_value(&run.room().items).unwrap();
        let actor = &mut run.squire_field.companions.get_mut(&1).unwrap().actor;
        (actor.x, actor.y, actor.invulnerable) = (14.0, 12.0, 0);
        run.projectiles
            .push(hostile((17.0, 12.0), (-120.0, 0.0), 1000));
        fly(&mut run);
        assert!(run.squire_field.companions[&1].actor.down(), "{kind:?}");
        assert_eq!(run.home.squire_company, roster);
        assert_eq!(serde_json::to_value(&run.room().items).unwrap(), drops);
    }
}

#[test]
fn smoke_and_vanish_unlock_in_order_and_only_vanish_hides_the_target() {
    for tier in [1, 2, 3, 4, 10, u8::MAX] {
        let mut run = field(Kind::Ninja, tier);
        foe_at(&mut run, EnemyKind::Goblin, (16.0, 12.0), 300);
        let actor = &mut run.squire_field.companions.get_mut(&1).unwrap().actor;
        (actor.x, actor.y) = (14.0, 12.0);
        run.tick_company_squires();
        let s = &run.squire_field.companions[&1];
        assert_eq!(s.effect > 0, tier >= 2);
        assert_eq!(s.targetable(), tier < 3);
        assert_eq!(s.actor.invulnerable > 0, tier >= 2);
        if tier >= 3 {
            let hp = s.actor.hp;
            run.projectiles
                .push(hostile((18.0, 12.0), (-120.0, 0.0), 1000));
            fly(&mut run);
            assert_eq!(run.squire_field.companions[&1].actor.hp, hp);
        }
    }
}

#[test]
fn ninja_closes_on_the_far_flank_and_backstabs_for_fixed_double_damage() {
    let mut run = field(Kind::Ninja, 1);
    foe_at(&mut run, EnemyKind::Goblin, (15.0, 12.0), 300);
    run.enemies[0].wits.foe = 1;
    run.enemies[0].hp = 1000;
    run.enemies[0].max_hp = 1000;
    let s = run.squire_field.companions.get_mut(&1).unwrap();
    (s.actor.x, s.actor.y) = (16.0, 12.0);
    let weight = s.actor.power;
    run.tick_company_squires();
    assert_eq!(run.enemies[0].hp, 1000 - 2 * weight);
    assert!(run.feel.credits.contains(&(1, 2 * weight)));

    let mut run = field(Kind::Ninja, 1);
    foe_at(&mut run, EnemyKind::Goblin, (20.0, 12.0), 300);
    let s = run.squire_field.companions.get_mut(&1).unwrap();
    (s.actor.x, s.actor.y) = (14.0, 12.0);
    run.tick_company_squires();
    assert!(run.squire_field.companions[&1].actor.x > 14.0);
    assert!(
        run.projectiles
            .iter()
            .any(|p| p.kind == Shot::Blade && p.traits.owner == 1)
    );
}

#[test]
fn ranger_marks_unlock_grow_expire_and_do_not_cut_enemy_health() {
    let mut weights = Vec::new();
    for tier in [1, 2, 3, 4, 10, u8::MAX] {
        let mut run = field(Kind::Ranger, tier);
        foe_at(&mut run, EnemyKind::Goblin, (18.0, 12.0), 300);
        let hp = run.enemies[0].hp;
        run.tick_company_squires();
        assert_eq!(run.enemies[0].hp, hp);
        let s = &run.squire_field.companions[&1];
        assert_eq!(s.mark.is_some(), tier >= 2);
        let shot = run
            .projectiles
            .iter()
            .find(|p| p.kind == Shot::Arrow)
            .unwrap();
        weights.push(shot.damage * 100 / s.actor.power);
        let mark_weight = if tier >= 3 {
            150
        } else if tier >= 2 {
            125
        } else {
            100
        };
        assert_eq!(shot.damage, s.actor.power * mark_weight / 100);
        if tier >= 2 {
            let s = run.squire_field.companions.get_mut(&1).unwrap();
            s.effect = 1;
            s.ability = 90;
            run.tick_company_squires();
            assert_eq!(run.squire_field.companions[&1].mark, None);
        }
    }
    assert!(weights[0] < weights[1] && weights[1] < weights[2]);
}

#[test]
fn healer_unlocks_cleanse_then_skull_help_and_respects_settling() {
    for tier in [1, 2, 3, 4, 10, u8::MAX] {
        let mut run = field(Kind::Healer, tier);
        run.players.get_mut(&1).unwrap().hexed = 100;
        run.tick_company_squires();
        assert_eq!(run.players[&1].hexed == 0, tier >= 2);
        let mut fallen = run.players[&1].clone();
        fallen.name = "Fallen".into();
        fallen.hp = 0;
        fallen.fell = 1;
        fallen.skull_at = Some((14.0, 12.0));
        run.players.insert(2, fallen);
        let s = run.squire_field.companions.get_mut(&1).unwrap();
        (s.actor.x, s.actor.y, s.actor.cooldown) = (14.0, 12.0, 0);
        run.tick = 2;
        run.tick_company_squires();
        assert_eq!(run.players[&2].skull_at, Some((14.0, 12.0)));
        run.tick = skulls::SKULL_SETTLE + 2;
        run.tick_company_squires();
        assert_eq!(run.players[&2].skull_at.unwrap().0 < 14.0, tier >= 3);
        assert_eq!(run.players[&2].hp, 0);
    }
}

#[test]
fn carrier_assist_is_a_short_ward_not_a_free_resurrection() {
    let mut run = field(Kind::Healer, 3);
    let mut fallen = run.players[&1].clone();
    fallen.name = "Fallen".into();
    fallen.hp = 0;
    run.players.insert(2, fallen);
    run.players.get_mut(&1).unwrap().carrying_skull = Some(2);
    run.tick_company_squires();
    assert!(run.players[&1].invulnerable > 0);
    assert_eq!(run.players[&2].hp, 0);
    assert_eq!(run.players[&1].carrying_skull, Some(2));
}

#[test]
fn wall_blocks_attacks_heals_and_bomb_splash() {
    for kind in [
        Kind::Ninja,
        Kind::Healer,
        Kind::Ranger,
        Kind::Shield,
        Kind::Alchemist,
    ] {
        let mut run = field(kind, 3);
        let actor = &mut run.squire_field.companions.get_mut(&1).unwrap().actor;
        (actor.x, actor.y) = (11.0, 11.0);
        run.players.get_mut(&1).unwrap().hp = 20;
        run.dungeon.rooms[run.at].set(6, 5, Tile::Wall);
        run.players.get_mut(&1).unwrap().x = 15.0;
        run.players.get_mut(&1).unwrap().y = 11.0;
        foe_at(&mut run, EnemyKind::Goblin, (15.0, 11.0), 300);
        let hp = run.enemies[0].hp;
        run.tick_company_squires();
        assert_eq!(run.enemies[0].hp, hp, "{kind:?}");
        assert_eq!(run.players[&1].hp, 20, "{kind:?}");
        assert!(run.projectiles.is_empty());
    }
    let mut run = field(Kind::Alchemist, 1);
    let actor = &mut run.squire_field.companions.get_mut(&1).unwrap().actor;
    (actor.x, actor.y) = (11.0, 11.0);
    foe_at(&mut run, EnemyKind::Goblin, (11.0, 9.0), 300);
    foe_at(&mut run, EnemyKind::Goblin, (15.0, 9.0), 300);
    run.dungeon.rooms[run.at].set(6, 4, Tile::Wall);
    let behind = run.enemies[1].hp;
    run.tick_company_squires();
    assert!(run.enemies[0].hp < run.enemies[0].max_hp);
    assert_eq!(run.enemies[1].hp, behind);
}

#[test]
fn six_tile_leash_applies_to_movement_targets_and_healing() {
    for kind in [
        Kind::Ninja,
        Kind::Healer,
        Kind::Ranger,
        Kind::Shield,
        Kind::Alchemist,
    ] {
        let mut run = field(kind, 3);
        let mut other = run.players[&1].clone();
        other.name = "Far friend".into();
        (other.x, other.y, other.hp) = (25.0, 12.0, 1);
        run.players.insert(2, other);
        foe_at(&mut run, EnemyKind::Goblin, (25.0, 12.0), 300);
        let actor = &mut run.squire_field.companions.get_mut(&1).unwrap().actor;
        (actor.x, actor.y) = (26.0, 12.0);
        let hp = run.enemies[0].hp;
        for _ in 0..50 {
            run.tick_company_squires();
        }
        let actor = &run.squire_field.companions[&1].actor;
        assert!((actor.x - 12.0).hypot(actor.y - 12.0) <= LEASH, "{kind:?}");
        assert_eq!(run.enemies[0].hp, hp);
        assert_eq!(run.players[&2].hp, 1);
        assert!(run.projectiles.is_empty());
    }
}

#[test]
fn squire_damage_is_credited_and_settled_once_by_normal_rewards() {
    let mut run = field(Kind::Ninja, 1);
    let roster = run.home.squire_company.clone();
    foe_at(&mut run, EnemyKind::Goblin, (15.5, 12.0), 300);
    let foe = run.enemies[0].id;
    run.enemies[0].hp = 1;
    let actor = &mut run.squire_field.companions.get_mut(&1).unwrap().actor;
    (actor.x, actor.y) = (14.0, 12.0);
    let score = run.score;
    run.tick_company_squires();
    assert!(run.feel.credits.contains(&(1, 1)));
    assert!(run.feel.blows.iter().any(|b| b.id == foe && b.by == 1));
    assert_eq!(run.score, score, "AI must not hand out loot itself");
    run.settle_kills();
    assert!(run.enemies.is_empty());
    assert!(run.score > score);
    let settled = run.score;
    run.settle_kills();
    assert_eq!(run.score, settled);
    assert_eq!(run.home.squire_company, roster);
}

#[test]
fn doorway_hook_runs_without_a_hireling_and_preserves_wounds_and_identity() {
    let mut run = field(Kind::Ranger, 2);
    let s = run.squire_field.companions.get_mut(&1).unwrap();
    s.actor.hp = 17;
    s.mark = Some(99);
    let name = s.squire.name.clone();
    run.players.get_mut(&1).unwrap().x = 20.0;
    run.hireling_follows();
    let s = &run.squire_field.companions[&1];
    assert!((s.actor.x - 20.0).hypot(s.actor.y - 12.0) < 3.0);
    assert_eq!(s.actor.hp, 17);
    assert_eq!(s.squire.name, name);
    assert_eq!(s.mark, None);
}

#[test]
fn deployment_uses_name_selection_and_ignores_absent_or_invalid_rosters() {
    let mut run = field(Kind::Ninja, 1);
    run.home
        .squire_company
        .rosters
        .get_mut(&run.players[&1].name)
        .unwrap()
        .active = Some(999);
    run.deploy_company_squires();
    assert!(run.squire_field.companions.is_empty());
    run.home
        .squire_company
        .rosters
        .get_mut(&run.players[&1].name)
        .unwrap()
        .active = Some(7);
    let mut moved = run.players.remove(&1).unwrap();
    moved.absent = true;
    run.players.insert(9, moved);
    run.deploy_company_squires();
    assert!(run.squire_field.companions.is_empty());
    run.players.get_mut(&9).unwrap().absent = false;
    run.deploy_company_squires();
    assert_eq!(run.squire_field.companions.len(), 1);
    assert_eq!(run.squire_field.companions[&9].actor.summoner, Some(9));
}

#[test]
fn field_is_deterministic_and_squire_stats_do_not_scale_with_party_or_enemy_health() {
    let mut one = field(Kind::Alchemist, 3);
    let mut six = field(Kind::Alchemist, 3);
    for id in 2..=6 {
        six.players.insert(id, six.players[&1].clone());
    }
    six.deploy_company_squires();
    assert_eq!(
        one.squire_field.companions[&1].actor.max_hp,
        six.squire_field.companions[&1].actor.max_hp
    );
    assert_eq!(
        one.squire_field.companions[&1].actor.power,
        six.squire_field.companions[&1].actor.power
    );
    foe_at(&mut one, EnemyKind::Goblin, (18.0, 12.0), 300);
    let mut again = one.clone();
    for _ in 0..180 {
        one.tick_company_squires();
        again.tick_company_squires();
    }
    assert_eq!(one.squires, again.squires);
    assert_eq!(
        serde_json::to_value(&one.enemies).unwrap(),
        serde_json::to_value(&again.enemies).unwrap()
    );
    assert_eq!(one.projectiles, again.projectiles);
    assert_eq!(
        serde_json::to_value(&one.players).unwrap(),
        serde_json::to_value(&again.players).unwrap()
    );
}

#[test]
fn beams_and_lob_landings_hurt_squires_but_warning_and_airtime_do_not() {
    let mut run = field(Kind::Healer, 1);
    let actor = &mut run.squire_field.companions.get_mut(&1).unwrap().actor;
    (actor.x, actor.y, actor.invulnerable) = (14.0, 12.0, 0);
    let hp = actor.hp;
    let mut beam = hostile((17.0, 12.0), (0.0, 0.0), 20);
    beam.traits.path.beam = Some(patterns::Beam {
        angle: std::f32::consts::PI,
        reach: 12.0,
        sweep: 0.0,
        warn: 1,
        burn: 2,
        struck: 0,
    });
    run.projectiles.push(beam);
    fly(&mut run);
    assert_eq!(run.squire_field.companions[&1].actor.hp, hp);
    fly(&mut run);
    assert_eq!(
        run.squire_field.companions[&1].actor.hp,
        hp - 20 * patterns::BEAM_SHARE / 100
    );

    let mut run = field(Kind::Shield, 1);
    let actor = &mut run.squire_field.companions.get_mut(&1).unwrap().actor;
    (actor.x, actor.y, actor.invulnerable) = (14.0, 12.0, 0);
    let hp = actor.hp;
    let mut lob = hostile((14.0, 12.0), (0.0, 0.0), 20);
    lob.traits.path.form = patterns::Form::Lob;
    lob.traits.path.fuse = 2;
    run.projectiles.push(lob);
    fly(&mut run);
    assert_eq!(run.squire_field.companions[&1].actor.hp, hp);
    fly(&mut run);
    assert_eq!(run.squire_field.companions[&1].actor.hp, hp - 20);
}

#[test]
fn a_mark_keeps_the_same_quarry_until_it_dies_then_clears() {
    let mut run = field(Kind::Ranger, 3);
    let far = foe_at(&mut run, EnemyKind::Goblin, (21.0, 12.0), 300);
    run.tick_company_squires();
    assert_eq!(run.squire_field.companions[&1].mark, Some(far));
    foe_at(&mut run, EnemyKind::Goblin, (15.0, 12.0), 300);
    run.projectiles.clear();
    run.squire_field.companions.get_mut(&1).unwrap().actor.cooldown = 0;
    run.tick_company_squires();
    assert_eq!(run.squire_field.companions[&1].mark, Some(far));
    run.enemies[0].hp = 0;
    run.tick_company_squires();
    assert_eq!(run.squire_field.companions[&1].mark, None);
}

#[test]
fn arrows_credit_the_owner_through_existing_projectile_damage() {
    let mut run = field(Kind::Ranger, 2);
    foe_at(&mut run, EnemyKind::Goblin, (17.0, 12.0), 300);
    let actor = &mut run.squire_field.companions.get_mut(&1).unwrap().actor;
    (actor.x, actor.y) = (14.0, 12.0);
    run.tick_company_squires();
    let hp = run.enemies[0].hp;
    let mut credited = 0;
    for _ in 0..20 {
        let mut sortie = run.knights_turn(&BTreeMap::new());
        run.shots_fly(&mut sortie);
        credited += sortie
            .credits
            .iter()
            .filter(|(id, _)| *id == 1)
            .map(|(_, d)| d)
            .sum::<u32>();
        run.projectiles.retain(|p| p.ttl > 0);
    }
    assert!(credited > 0);
    assert_eq!(run.enemies[0].hp, hp - credited);
    assert_eq!(run.enemies[0].wits.foe, 1);
}

#[test]
fn warnings_and_foes_see_a_standing_squire_and_forget_a_vanished_one() {
    let mut run = field(Kind::Ninja, 3);
    foe_at(&mut run, EnemyKind::Goblin, (18.0, 12.0), 300);
    run.players.get_mut(&1).unwrap().stone = true;
    let at = (
        run.squire_field.companions[&1].actor.x,
        run.squire_field.companions[&1].actor.y,
    );
    assert_eq!(press::warning_target(&run, &run.enemies[0]), Some(at));
    run.squire_field.companions.get_mut(&1).unwrap().effect = HZ;
    assert_eq!(press::warning_target(&run, &run.enemies[0]), None);
    run.squire_field.companions.get_mut(&1).unwrap().effect = 0;
    run.squire_field.companions.get_mut(&1).unwrap().actor.hp = 0;
    assert_eq!(press::warning_target(&run, &run.enemies[0]), None);
}

#[test]
fn healer_walks_to_the_lowest_ratio_patient_inside_the_owner_leash() {
    let mut run = field(Kind::Healer, 2);
    let mut other = run.players[&1].clone();
    other.name = "Patient".into();
    (other.x, other.y, other.hp) = (21.0, 12.0, 20);
    run.players.insert(2, other);
    let s = run.squire_field.companions.get_mut(&1).unwrap();
    (s.actor.x, s.actor.y) = (12.0, 12.0);
    for _ in 0..80 {
        run.tick_company_squires();
    }
    assert!(run.players[&2].hp > 20);
    assert!((run.squire_field.companions[&1].actor.x - 12.0).abs() <= LEASH);
}

#[test]
fn contact_damage_and_beams_respect_stone_between_foe_and_squire() {
    let mut run = field(Kind::Shield, 1);
    let s = run.squire_field.companions.get_mut(&1).unwrap();
    (s.actor.x, s.actor.y, s.actor.invulnerable) = (11.4, 11.0, 0);
    let hp = s.actor.hp;
    run.dungeon.rooms[run.at].set(6, 5, Tile::Wall);
    foe_at(&mut run, EnemyKind::Goblin, (14.4, 11.0), 0);
    let mut beam = hostile((16.0, 11.0), (0.0, 0.0), 40);
    beam.traits.path.beam = Some(patterns::Beam {
        angle: std::f32::consts::PI,
        reach: 12.0,
        sweep: 0.0,
        warn: 0,
        burn: 2,
        struck: 0,
    });
    run.projectiles.push(beam);
    fly(&mut run);
    run.tick_company_squires();
    assert_eq!(run.squire_field.companions[&1].actor.hp, hp);
}

#[test]
fn real_ticks_and_save_restore_keep_companions_deterministic() {
    for kind in [
        Kind::Ninja,
        Kind::Healer,
        Kind::Ranger,
        Kind::Shield,
        Kind::Alchemist,
    ] {
        let mut run = field(kind, 3);
        foe_at(&mut run, EnemyKind::Goblin, (18.0, 12.0), 300);
        run.players.get_mut(&1).unwrap().hp = 40;
        let saved = serde_json::to_string(&run).unwrap();
        let mut restored: Run = serde_json::from_str(&saved).unwrap();
        for _ in 0..120 {
            run.step(&BTreeMap::new());
            restored.step(&BTreeMap::new());
        }
        assert_eq!(run.squires, restored.squires, "{kind:?}");
        assert_eq!(run.projectiles, restored.projectiles, "{kind:?}");
        assert_eq!(
            serde_json::to_value(&run.players).unwrap(),
            serde_json::to_value(&restored.players).unwrap(),
            "{kind:?}"
        );
        assert_eq!(run.score, restored.score, "{kind:?}");
    }
}

#[test]
fn alchemist_potions_have_their_own_rest_while_bombs_burst_in_an_area() {
    let mut run = field(Kind::Alchemist, 2);
    run.players.get_mut(&1).unwrap().hp = 20;
    let s = run.squire_field.companions.get_mut(&1).unwrap();
    (s.actor.x, s.actor.y) = (14.0, 12.0);
    s.actor.cooldown = HZ;
    run.tick_company_squires();
    assert!(run.players[&1].hp > 20);
    assert_eq!(run.squire_field.companions[&1].actor.cooldown, HZ - 1);
    assert_eq!(run.squire_field.companions[&1].ability, SPECIAL_EVERY);
    let hp = run.players[&1].hp;
    run.tick_company_squires();
    assert_eq!(run.players[&1].hp, hp);

    foe_at(&mut run, EnemyKind::Goblin, (17.0, 12.0), 300);
    foe_at(&mut run, EnemyKind::Goblin, (18.0, 12.0), 300);
    run.squire_field.companions.get_mut(&1).unwrap().actor.cooldown = 0;
    let enemy_hp: Vec<_> = run.enemies.iter().map(|e| e.hp).collect();
    run.tick_company_squires();
    assert!(run.enemies.iter().zip(enemy_hp).all(|(e, hp)| e.hp < hp));
    assert_eq!(run.players[&1].hp, hp);
}

#[test]
fn a_turned_beam_and_a_down_squire_do_not_catch_or_damage_friends() {
    let mut run = field(Kind::Healer, 1);
    let s = run.squire_field.companions.get_mut(&1).unwrap();
    (s.actor.x, s.actor.y, s.actor.invulnerable) = (14.0, 12.0, 0);
    let hp = s.actor.hp;
    let mut beam = hostile((17.0, 12.0), (0.0, 0.0), 40);
    beam.hostile = false;
    beam.traits.path.age = 2;
    beam.traits.path.beam = Some(patterns::Beam {
        angle: std::f32::consts::PI,
        reach: 12.0,
        sweep: 0.0,
        warn: 0,
        burn: 4,
        struck: 0,
    });
    run.projectiles.push(beam);
    fly(&mut run);
    assert_eq!(run.squire_field.companions[&1].actor.hp, hp);
    run.projectiles.clear();
    run.squire_field.companions.get_mut(&1).unwrap().actor.hp = 0;
    let hp = run.players[&1].hp;
    run.projectiles
        .push(hostile((17.0, 12.0), (-240.0, 0.0), 20));
    fly(&mut run);
    assert!(run.players[&1].hp < hp);
}

#[test]
fn doorway_placement_uses_open_floor_and_home_clears_only_field_actors() {
    let mut run = field(Kind::Ninja, 1);
    let roster = run.home.squire_company.clone();
    run.dungeon.rooms[run.at].set(5, 6, Tile::Wall);
    run.hireling_follows();
    let s = &run.squire_field.companions[&1];
    let grid = Grid {
        room: run.room(),
        barred: run.barred(),
    };
    assert!(grid.clear(s.actor.x, s.actor.y, HERO_RADIUS, Mover::Hero));
    run.dungeon.depth = 0;
    run.company_squires_follow();
    assert!(run.squire_field.companions.is_empty());
    assert_eq!(run.home.squire_company, roster);
}

#[test]
fn damage_is_fixed_by_level_and_depth_not_a_fraction_of_enemy_health() {
    let mut losses = Vec::new();
    for health in [100, 10000] {
        let mut run = field(Kind::Shield, 2);
        foe_at(&mut run, EnemyKind::Goblin, (15.0, 12.0), 300);
        run.enemies[0].hp = health;
        run.enemies[0].max_hp = health;
        let s = run.squire_field.companions.get_mut(&1).unwrap();
        (s.actor.x, s.actor.y) = (14.0, 12.0);
        let power = s.actor.power;
        run.tick_company_squires();
        assert_eq!(health - run.enemies[0].hp, power);
        losses.push(health - run.enemies[0].hp);
    }
    assert_eq!(losses[0], losses[1]);
}
