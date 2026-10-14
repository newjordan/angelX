use super::*;
use std::collections::BTreeMap;

fn arena(id: &str) -> Run {
    let mut run = Run::new(17, 1, Some("Friend"));
    let boss = bosses::builtin().into_iter().find(|b| b.id == id).unwrap();
    let depth = if boss.only_in == Pack::Fungal { 5 } else { 2 };
    run.dungeon = crawl_floor(depth, boss.only_in, 17);
    let at = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Stairs)
        .unwrap();
    run.enter_for_test(at);
    let room = &mut run.dungeon.rooms[at];
    for y in 1..room.rows - 1 {
        for x in 1..room.cols - 1 {
            room.set_for_test(x, y, Tile::Floor);
        }
    }
    run.enemies.clear();
    run.projectiles.clear();
    run.calm_for_test();
    run.works.clouds.clear();
    run.works.perils.clear();
    let i = run.bosses.iter().position(|b| b.id == id).unwrap();
    run.spawn_boss_at_for_test(i, 24.0, 14.0);
    for (id, h) in &mut run.players {
        h.x = if *id == 1 { 12.0 } else { 36.0 };
        h.y = 20.0;
        h.invulnerable = 10000;
    }
    run.phase = Phase::Fighting;
    run
}
fn act_one(run: &mut Run) -> Deeds {
    let grid = Grid {
        room: &run.dungeon.rooms[run.at],
        barred: true,
    };
    let e = &mut run.enemies[0];
    e.age += 1;
    let b = &run.bosses[e.boss.unwrap() as usize];
    let mut deeds = Deeds::default();
    act(e, b, &[(32.0, 20.0)], &grid, run.seed, &mut deeds);
    deeds
}
#[test]
fn signatures_rip_directions_replay_and_favour_clear_corridors() {
    let run = arena(RIPPER);
    let grid = Grid {
        room: run.room(),
        barred: true,
    };
    let mut e = run.enemies[0].clone();
    e.x = 6.0;
    e.y = 6.0;
    e.size = Some(3.2);
    let mut directions = Vec::new();
    for cycle in 0..32 {
        e.signature.cycle = cycle;
        let a = rip_direction(&e, &grid, 17);
        assert_eq!(a, rip_direction(&e, &grid, 17));
        assert!((a.0.hypot(a.1) - 1.0).abs() < 0.001);
        assert!(grid.clear(e.x + a.0 * 6.0, e.y + a.1 * 6.0, e.radius(), Mover::Walker));
        directions.push(a);
    }
    assert!(directions.windows(2).any(|p| p[0] != p[1]));
}
#[test]
fn signatures_three_breaths_grow_the_hitbox_then_every_rip_is_warned() {
    let mut run = arena(RIPPER);
    run.enemies[0].stage = INHALE;
    run.enemies[0].hp = run.enemies[0].max_hp / 3;
    for (ticks, scale) in [(1, 1.0), (23, 1.35), (24, 1.7), (24, 2.0)] {
        for _ in 0..ticks {
            act_one(&mut run);
        }
        assert!((run.enemies[0].radius() - 1.6 * scale).abs() < 0.01);
    }
    assert_eq!(run.enemies[0].stage, TELL);
    let mut starts = 0;
    for _ in 0..240 {
        let before = run.enemies[0].stage;
        let timer = run.enemies[0].timer;
        let deeds = act_one(&mut run);
        if before == TELL {
            assert!(deeds.sweeps.is_empty());
        }
        if before == TELL && run.enemies[0].stage == DASH {
            assert_eq!(timer + 1, RIP_TELL);
            starts += 1;
        }
        if run.enemies[0].stage == DAZED {
            break;
        }
    }
    assert_eq!(starts, 3);
    assert_eq!(run.enemies[0].stage, DAZED);
    assert_eq!(
        rear_bonus(
            &run.enemies[0],
            &run.bosses,
            (
                run.enemies[0].x - run.enemies[0].dir.0,
                run.enemies[0].y - run.enemies[0].dir.1
            ),
            20
        ),
        10
    );
}
#[test]
fn signatures_wall_lifetime_area_cap_and_fire_flash() {
    let mut run = arena(RIPPER);
    for _ in 0..150 {
        add_wall(
            &mut run.works.clouds,
            skills::Cloud {
                x: 24.0,
                y: 14.0,
                vx: 0.0,
                vy: 0.0,
                age: 0,
                owner: 0,
                power: 4,
                tint: 3,
            },
        );
    }
    let area: f32 = run
        .works
        .clouds
        .iter()
        .map(|c| std::f32::consts::PI * c.wall_radius().powi(2))
        .sum();
    assert!(area <= WALL_AREA);
    assert!(run.works.clouds.len() <= WALL_MAX);
    assert!(run.works.valid());
    run.enemies[0].frozen = 10000;
    let hp = run.enemies[0].hp;
    run.works.clouds.truncate(1);
    run.works.blazes.push(skills::Blaze {
        x: 24.0,
        y: 14.0,
        age: 0,
        life: HZ,
        owner: 1,
        power: 3,
        radius: 2.2,
    });
    run.tick_works(&BTreeMap::new());
    assert!(run.works.clouds.is_empty());
    assert!(run.enemies[0].hp < hp);
    assert!(
        run.works
            .booms
            .iter()
            .any(|b| b.kind == skills::BoomKind::Flash)
    );
    run.works.blazes.clear();
    add_wall(
        &mut run.works.clouds,
        skills::Cloud {
            x: 30.0,
            y: 20.0,
            vx: 0.0,
            vy: 0.0,
            age: 0,
            owner: 0,
            power: 4,
            tint: 2,
        },
    );
    for _ in 0..skills::GAS_LIFE - 1 {
        run.tick_works(&BTreeMap::new());
    }
    assert_eq!(run.works.clouds.len(), 1);
    run.tick_works(&BTreeMap::new());
    assert!(run.works.clouds.is_empty());
}
#[test]
fn signatures_walking_and_rolling_escape_the_pull() {
    let mut run = arena(RIPPER);
    run.enemies[0].stage = INHALE;
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = 18.0;
    hero.y = 14.0;
    let start = hero.x;
    for _ in 0..20 {
        run.step(&BTreeMap::from([(
            1,
            Input {
                move_x: -1,
                ..Default::default()
            },
        )]));
    }
    assert!(run.players[&1].x < start - 2.0, "walking beats the inhale");
    let h = run.players.get_mut(&1).unwrap();
    h.dash_ticks = ROLL_TICKS;
    h.x = 18.0;
    h.y = 14.0;
    run.signature_deeds(vec![], vec![(24.0, 14.0)], vec![]);
    assert_eq!(run.players[&1].x, 18.0, "the pull does not cancel rolls");
}
#[test]
fn signatures_rails_warn_before_any_charge_damage_and_chain_after_jumps() {
    let mut run = arena(ENGINE);
    run.enemies[0].timer = 104;
    run.enemies[0].hp = run.enemies[0].max_hp / 3;
    let deeds = act_one(&mut run);
    assert_eq!(run.enemies[0].stage, TELL);
    assert_eq!(run.enemies[0].signature.rails.len(), 3);
    let rails = &run.enemies[0].signature.rails;
    let reach = |r: &Rail| (r.b.0 - r.a.0).hypot(r.b.1 - r.a.1);
    assert!(rails.windows(2).all(|r| reach(&r[0]) >= reach(&r[1])));
    assert_eq!(deeds.spawns.len(), 4);
    assert!(deeds.sweeps.is_empty());
    let mut charges = 0;
    for _ in 0..330 {
        let before = run.enemies[0].stage;
        let timer = run.enemies[0].timer;
        let deeds = act_one(&mut run);
        if before == TELL || before == JUMP {
            assert!(deeds.sweeps.is_empty());
            assert!(!run.enemies[0].hurts_by_touch() || run.enemies[0].stage == DASH);
        }
        if before == TELL && run.enemies[0].stage == DASH {
            assert_eq!(timer + 1, RAIL_TELL);
            charges += 1;
        }
        if before == DASH {
            assert!(!deeds.sweeps.is_empty());
        }
        if run.enemies[0].stage == DAZED {
            break;
        }
    }
    assert_eq!(charges, 3);
}

#[test]
fn signatures_engine_footprint_sets_wall_clearance_and_charge_damage() {
    let mut run = arena(ENGINE);
    let radius = run.enemies[0].radius();
    assert_eq!(radius, 4.8);
    assert_eq!(
        radius,
        run.bosses[run.enemies[0].boss.unwrap() as usize].radius
    );
    let grid = Grid {
        room: run.room(),
        barred: true,
    };
    assert!(grid.clear(24.0, 14.0, radius, Mover::Walker));
    assert!(!grid.clear(4.0, 14.0, radius, Mover::Walker));
    run.enemies[0].stage = DASH;
    run.enemies[0].dir = (1.0, 0.0);
    let deeds = act_one(&mut run);
    assert_eq!(deeds.sweeps[0].2, radius + HERO_RADIUS);
    for (id, h) in &mut run.players {
        h.x = 24.5;
        h.y = 14.0 + radius + if *id == 1 { 0.2 } else { 1.0 };
        h.invulnerable = 0;
    }
    let hp = [run.players[&1].hp, run.players[&2].hp];
    run.signature_deeds(vec![], vec![], deeds.sweeps);
    assert!(run.players[&1].hp < hp[0], "the outer leg corridor hits");
    assert_eq!(run.players[&2].hp, hp[1], "outside the rail remains safe");
    let id = run
        .spawn_staged(EnemyKind::Spiderling, 12.0, 14.0, TENDER)
        .unwrap();
    assert_eq!(
        run.enemies.iter().find(|e| e.id == id).unwrap().radius(),
        1.6
    );
    let loaded: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    assert_eq!(loaded.enemies[0].radius(), radius);
    assert!(loaded.valid_snapshot());
}

#[test]
fn signatures_engine_fits_native_chambers_and_moves_without_clipping_stone() {
    let mut encounters = 0;
    for seed in 0..32 {
        let mut run = Run::new(seed, 17, Some("Friend"));
        run.dungeon = layout::floor(2, Pack::Cavern, &mut run.rng);
        run.populate_boss_gates();
        run.enter(0, None);
        let gates = run.boss_gates.as_ref().unwrap();
        let room = gates
            .leaders
            .iter()
            .find(|l| run.bosses[l.boss as usize].id == ENGINE)
            .map(|l| l.room)
            .or_else(|| {
                gates
                    .guardian_boss
                    .filter(|&i| run.bosses[i as usize].id == ENGINE)
                    .map(|_| gates.guardian)
            });
        let Some(room) = room else { continue };
        if room == gates.guardian {
            run.clear_prerequisites_for_test();
        }
        run.enter(room, None);
        let engine = |run: &Run| {
            run.enemies
                .iter()
                .find(|e| e.boss.is_some_and(|i| run.bosses[i as usize].id == ENGINE))
                .map(|e| (e.x, e.y, e.radius()))
                .unwrap()
        };
        let start = engine(&run);
        let mut moved = false;
        for _ in 0..180 {
            let (x, y, radius) = engine(&run);
            let grid = Grid {
                room: run.room(),
                barred: run.barred(),
            };
            assert!(
                grid.clear(x, y, radius, Mover::Walker),
                "seed {seed}: engine clips stone"
            );
            moved |= (x - start.0).hypot(y - start.1) > 1.0;
            for h in run.players.values_mut() {
                h.invulnerable = 10000;
            }
            run.step(&BTreeMap::new());
        }
        assert!(moved, "seed {seed}: engine is trapped");
        encounters += 1;
    }
    assert!(encounters >= 8, "exercise several native chamber layouts");
}
#[test]
fn signatures_native_floors_spawn_both_with_guardian_health_and_finite_tenders() {
    let mut found = [false, false];
    let mut apex = false;
    for seed in 0..128 {
        for (which, depth, pack, id) in [(0, 2, Pack::Cavern, ENGINE), (1, 5, Pack::Fungal, RIPPER)]
        {
            let mut run = Run::new(seed, 17, Some("Friend"));
            run.dungeon = layout::floor(depth, pack, &mut run.rng);
            run.populate_boss_gates();
            run.enter(0, None);
            let gates = run.boss_gates.clone().unwrap();
            let boss = run.bosses.iter().position(|b| b.id == id).unwrap();
            assert!(
                run.bosses[boss].hp
                    >= run
                        .bosses
                        .iter()
                        .filter(|b| b.only_in == pack)
                        .map(|b| b.hp)
                        .max()
                        .unwrap()
            );
            let room = gates
                .leaders
                .iter()
                .find(|l| l.boss as usize == boss)
                .map(|l| l.room)
                .or_else(|| (gates.guardian_boss == Some(boss as u8)).then_some(gates.guardian));
            if let Some(room) = room {
                if room == gates.guardian {
                    run.clear_prerequisites_for_test();
                    apex = true;
                }
                run.enter(room, None);
                let e = run
                    .enemies
                    .iter()
                    .find(|e| e.boss == Some(boss as u8))
                    .unwrap();
                assert!(e.max_hp >= 1800);
                found[which] = true;
                if id == ENGINE {
                    let spent = run.boss_gates.as_ref().unwrap().economy.spawn_spent;
                    let room = run.room();
                    let spot = (1..room.rows - 1)
                        .flat_map(|y| (1..room.cols - 1).map(move |x| (x, y)))
                        .find(|&(x, y)| room.tile(x as i32, y as i32) == Tile::Floor)
                        .unwrap();
                    let (x, y) = (
                        (spot.0 as f32 + 0.5) * TILE_UNITS,
                        (spot.1 as f32 + 0.5) * TILE_UNITS,
                    );
                    assert!(
                        run.spawn_staged(EnemyKind::Spiderling, x, y, TENDER)
                            .is_some()
                    );
                    assert_eq!(
                        run.boss_gates.as_ref().unwrap().economy.spawn_spent,
                        spent + 1
                    );
                    assert!(run.valid_snapshot());
                }
            }
        }
        if found.iter().all(|f| *f) && apex {
            break;
        }
    }
    assert_eq!(found, [true, true]);
    assert!(apex, "Ripper can be the rare hive apex");
}

#[test]
fn signatures_old_catalogs_replay_without_changing_current_floor_encounters() {
    for (depth, pack) in [(2, Pack::Cavern), (5, Pack::Fungal)] {
        let mut run = Run::new(31, 17, None);
        run.bosses.retain(|b| !special(b));
        run.dungeon = layout::floor(depth, pack, &mut run.rng);
        run.boss_gates = super::super::boss_population::populate(
            &run.dungeon,
            &mut run.bosses,
            run.seed,
            run.raid_id,
            run.mode,
        );
        run.enter(0, None);
        assert!(!run.bosses.iter().any(special));
        assert!(run.valid_snapshot());
        let loaded: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
        assert!(loaded.valid_snapshot());
        run.populate_boss_gates();
        assert!(run.bosses.iter().any(|b| special(b) && b.only_in == pack));
    }
}

#[test]
fn signatures_gas_poison_blocks_slows_and_ember_or_powder_ignites_it() {
    let shot = |kind, arm| Projectile {
        x: 8.0,
        y: 14.0,
        vx: 30.0,
        vy: 0.0,
        hostile: false,
        look: None,
        kind,
        damage: 10,
        pierce: 0,
        last_hit: None,
        empowered: false,
        traits: ShotTraits {
            arm,
            ..Default::default()
        },
        ttl: 3 * HZ,
    };
    let cloud = |age| skills::Cloud {
        x: 9.2,
        y: 14.0,
        vx: 0.0,
        vy: 0.0,
        age,
        owner: 0,
        power: 4,
        tint: 2,
    };
    for (kind, arm, ignites) in [
        (Shot::Arrow, false, false),
        (Shot::Ember, false, true),
        (Shot::Ball, true, true),
    ] {
        let mut r = arena(RIPPER);
        r.enemies[0].frozen = 10000;
        r.works.clouds.push(cloud(0));
        r.projectiles.push(shot(kind, arm));
        r.step(&BTreeMap::new());
        if ignites {
            assert!(r.works.clouds.is_empty());
            assert!(
                r.works
                    .booms
                    .iter()
                    .any(|b| b.kind == skills::BoomKind::Flash)
            );
        } else {
            assert!(r.projectiles.is_empty());
            assert_eq!(r.works.clouds.len(), 1);
        }
    }
    let mut r = arena(RIPPER);
    r.enemies[0].frozen = 10000;
    r.works.clouds.push(cloud(HZ));
    r.projectiles.push(shot(Shot::Arrow, false));
    r.step(&BTreeMap::new());
    assert!((r.projectiles[0].x - 8.55).abs() < 0.01);
    let h = r.players.get_mut(&1).unwrap();
    h.x = 9.2;
    h.y = 14.0;
    h.invulnerable = 0;
    let hp = h.hp;
    r.projectiles.clear();
    r.tick = 14;
    r.step(&BTreeMap::new());
    assert!(r.players[&1].hp < hp);
}
