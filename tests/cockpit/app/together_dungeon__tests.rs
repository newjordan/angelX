use super::*;

fn blueprint(slot: Slot) -> Blueprint {
    Blueprint {
        version: 1,
        id: "test-item".into(),
        title: "Test Item".into(),
        slot,
        power: 5,
        speed: 3,
        range: 8,
        pattern: Pattern::Bolt,
        effect: Effect::None,
    }
}

fn loadout() -> Loadout {
    let mut gear = Loadout::default();
    gear.equip(blueprint(Slot::Weapon).compile().unwrap());
    gear.equip(blueprint(Slot::Spell).compile().unwrap());
    gear
}

fn run() -> Run {
    Run::new(
        123,
        [(1, "One".into(), loadout()), (2, "Two".into(), loadout())],
    )
}

fn party_turn(run: &mut Run, one: CombatAction, two: CombatAction) {
    run.queue(1, one).unwrap();
    run.queue(2, two).unwrap();
}

#[test]
fn matrix_clamps_extreme_requests_and_final_pickup_effects() {
    for power in [0, 1, 4, 8, 9, u32::MAX] {
        for speed in [0, 1, 3, 6, u32::MAX] {
            for range in [0, 1, 4, 8, u32::MAX] {
                for pattern in [Pattern::Bolt, Pattern::Spread] {
                    for effect in [Effect::None, Effect::Pierce] {
                        let mut request = blueprint(Slot::Weapon);
                        request.power = power;
                        request.speed = speed;
                        request.range = range;
                        request.pattern = pattern;
                        request.effect = effect;
                        let item = request.compile().unwrap();
                        assert!((1..=8).contains(&item.power));
                        assert!((1..=6).contains(&item.speed));
                        assert!((1..=8).contains(&item.range));
                        assert!(item.points() <= ITEM_BUDGET);
                        for boost in [
                            None,
                            Some(Pickup::Power),
                            Some(Pickup::Haste),
                            Some(Pickup::Spread),
                        ] {
                            let stats = item.stats(boost);
                            assert!(stats.cooldown >= 2);
                            assert!(stats.damage <= 36 && stats.damage <= 8 * stats.cooldown);
                            assert!((1..=3).contains(&stats.targets));
                            assert!(stats.energy >= stats.damage.div_ceil(4));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn unsupported_item_behavior_is_rejected() {
    let mut request = blueprint(Slot::Weapon);
    request.effect = Effect::Heal;
    assert!(request.compile().is_err());
    request.slot = Slot::Spell;
    request.pattern = Pattern::Spread;
    assert!(request.compile().is_err());
    request.pattern = Pattern::Bolt;
    assert!(request.compile().is_ok());
    request.title = "escape\u{1b}[31m".into();
    assert!(request.compile().is_err());
    let json = serde_json::to_value(blueprint(Slot::Spell)).unwrap();
    let mut json = json.as_object().unwrap().clone();
    json.insert("script".into(), "shell('rm')".into());
    assert!(serde_json::from_value::<Blueprint>(json.into()).is_err());
}

#[test]
fn combat_waits_for_every_player_and_prevents_extra_actions() {
    let mut run = run();
    run.queue(1, CombatAction::Wait).unwrap();
    assert_eq!(run.tick, 0);
    let state = serde_json::to_string(&run).unwrap();
    assert!(run.queue(1, CombatAction::Fire).is_err());
    assert_eq!(serde_json::to_string(&run).unwrap(), state);
    run.queue(2, CombatAction::Wait).unwrap();
    assert_eq!(run.tick, 1);
    assert!(run.pending.is_empty());
}

#[test]
fn simultaneous_swaps_and_following_moves_do_not_depend_on_queue_order() {
    for reverse in [false, true] {
        let mut run = run();
        let mut queue = |one, two| {
            let actions = if reverse {
                [(2, two), (1, one)]
            } else {
                [(1, one), (2, two)]
            };
            for (id, action) in actions {
                run.queue(id, action).unwrap();
            }
        };
        queue(
            CombatAction::Move(Direction::South),
            CombatAction::Move(Direction::North),
        );
        assert_eq!((run.heroes[&1].x, run.heroes[&1].y), (2, 2));
        assert_eq!((run.heroes[&2].x, run.heroes[&2].y), (2, 1));
        run.queue(1, CombatAction::Move(Direction::South)).unwrap();
        run.queue(2, CombatAction::Move(Direction::South)).unwrap();
        assert_eq!((run.heroes[&1].x, run.heroes[&1].y), (2, 3));
        assert_eq!((run.heroes[&2].x, run.heroes[&2].y), (2, 2));
    }
}

#[test]
fn conflicting_destinations_cancel_and_blocked_convoys_do_not_overlap() {
    let mut run = run();
    run.heroes.get_mut(&2).unwrap().y = 3;
    party_turn(
        &mut run,
        CombatAction::Move(Direction::South),
        CombatAction::Move(Direction::North),
    );
    assert_eq!(run.heroes[&1].y, 1);
    assert_eq!(run.heroes[&2].y, 3);
    run.heroes.get_mut(&2).unwrap().y = 2;
    party_turn(
        &mut run,
        CombatAction::Move(Direction::South),
        CombatAction::Wait,
    );
    assert_eq!(run.heroes[&1].y, 1);
    assert_eq!(run.heroes[&2].y, 2);
    assert!(run.pending.is_empty());
    assert_eq!(run.tick, 2);
}

#[test]
fn fallen_heroes_do_not_consume_pickups_or_hold_the_next_turn_open() {
    let mut run = run();
    run.heroes.get_mut(&1).unwrap().hp = 1;
    run.danger.insert((2, 1));
    run.pickups.push((2, 1, Pickup::Energy));
    party_turn(&mut run, CombatAction::Wait, CombatAction::Wait);
    assert_eq!(run.heroes[&1].hp, 0);
    assert_eq!(run.pickups, vec![(2, 1, Pickup::Energy)]);
    let before = serde_json::to_string(&run).unwrap();
    assert!(run.queue(1, CombatAction::Wait).is_err());
    assert_eq!(serde_json::to_string(&run).unwrap(), before);
    run.queue(2, CombatAction::Move(Direction::North)).unwrap();
    assert_eq!(run.tick, 2);
    assert!(run.pending.is_empty());
    assert!(run.pickups.is_empty());
    run.enemies.clear();
    run.phase = Phase::Stairs;
    run.descend().unwrap();
    assert_eq!(run.heroes[&1].hp, 0);
    run.queue(2, CombatAction::Wait).unwrap();
    assert_eq!(run.tick, 3);
}

#[test]
fn a_party_wipe_ends_the_run_without_stale_pending_inputs() {
    let mut run = run();
    for hero in run.heroes.values_mut() {
        hero.hp = 1;
        run.danger.insert((hero.x, hero.y));
    }
    party_turn(&mut run, CombatAction::Wait, CombatAction::Wait);
    assert_eq!(run.phase, Phase::Wiped);
    assert!(run.pending.is_empty());
    let before = serde_json::to_string(&run).unwrap();
    assert!(run.queue(2, CombatAction::Wait).is_err());
    assert!(run.descend().is_err());
    assert_eq!(serde_json::to_string(&run).unwrap(), before);
}

#[test]
fn spread_splits_the_total_damage_and_charges_energy_once() {
    let mut run = run();
    run.heroes.get_mut(&1).unwrap().x = 15;
    run.heroes.get_mut(&1).unwrap().y = 5;
    run.heroes
        .get_mut(&1)
        .unwrap()
        .loadout
        .weapon
        .as_mut()
        .unwrap()
        .pattern = Pattern::Spread;
    for enemy in &mut run.enemies {
        enemy.hp = 100;
    }
    let item = run.heroes[&1].loadout.weapon.as_ref().unwrap();
    let stats = item.stats(None);
    let before: u32 = run.enemies.iter().map(|enemy| enemy.hp).sum();
    party_turn(&mut run, CombatAction::Fire, CombatAction::Wait);
    let after: u32 = run.enemies.iter().map(|enemy| enemy.hp).sum();
    assert_eq!(before - after, stats.damage);
    assert_eq!(run.heroes[&1].energy, 100 - stats.energy);
    assert_eq!(run.heroes[&1].weapon_cooldown, stats.cooldown - 1);
    assert!(run.queue(1, CombatAction::Fire).is_err());
}

#[test]
fn telegraphed_attacks_can_be_dodged_and_dash_cannot_cross_walls() {
    let mut run = run();
    for _ in 0..3 {
        party_turn(&mut run, CombatAction::Wait, CombatAction::Wait);
    }
    assert!(run.danger.contains(&(2, 1)));
    party_turn(
        &mut run,
        CombatAction::Move(Direction::East),
        CombatAction::Move(Direction::East),
    );
    assert_eq!(run.heroes[&1].hp, 100);
    assert_eq!(run.heroes[&2].hp, 100);
    for _ in 0..3 {
        party_turn(&mut run, CombatAction::Wait, CombatAction::Wait);
    }
    assert!(run.heroes[&1].hp < 100);
    run.heroes.get_mut(&1).unwrap().x = 8;
    run.heroes.get_mut(&1).unwrap().y = 3;
    let state = serde_json::to_string(&run).unwrap();
    assert!(run.queue(1, CombatAction::Dash(Direction::East)).is_err());
    assert_eq!(serde_json::to_string(&run).unwrap(), state);
}

#[test]
fn support_spells_use_the_same_budget_and_restore_only_one_nearby_ally() {
    let mut run = run();
    let mut request = blueprint(Slot::Spell);
    request.effect = Effect::Heal;
    run.heroes.get_mut(&1).unwrap().loadout.spell = Some(request.compile().unwrap());
    run.heroes.get_mut(&1).unwrap().hp = 80;
    run.heroes.get_mut(&2).unwrap().hp = 50;
    party_turn(&mut run, CombatAction::Cast, CombatAction::Wait);
    assert_eq!(run.heroes[&1].hp, 80);
    assert!(run.heroes[&2].hp > 50 && run.heroes[&2].hp <= 100);
}

#[test]
fn clear_room_allows_pickup_collection_then_resets_for_next_floor() {
    let mut run = run();
    run.enemies.clear();
    run.phase = Phase::Stairs;
    run.pickups = vec![(3, 1, Pickup::Spread)];
    party_turn(
        &mut run,
        CombatAction::Move(Direction::East),
        CombatAction::Wait,
    );
    assert_eq!(run.heroes[&1].boost, Some(Pickup::Spread));
    assert_eq!(run.heroes[&1].boost_turns, 8);
    assert!(run.pickups.is_empty());
    assert!(run.queue(1, CombatAction::Fire).is_err());
    run.descend().unwrap();
    assert_eq!(run.floor, 2);
    assert_eq!(run.phase, Phase::Fighting);
    assert_eq!(run.enemies.len(), 3);
    assert!(run.danger.is_empty());
}

#[test]
fn stairs_do_not_discard_an_unfinished_party_turn() {
    let mut run = run();
    run.enemies.clear();
    run.phase = Phase::Stairs;
    run.queue(2, CombatAction::Wait).unwrap();
    let before = serde_json::to_string(&run).unwrap();
    assert!(run.descend().unwrap_err().contains("queued party turn"));
    assert_eq!(serde_json::to_string(&run).unwrap(), before);
    run.queue(1, CombatAction::Wait).unwrap();
    run.descend().unwrap();
    assert_eq!(run.floor, 2);
    assert!(run.pending.is_empty());
}

#[test]
fn seeded_runs_replay_identically() {
    let mut one = run();
    let mut two = run();
    for _ in 0..4 {
        party_turn(
            &mut one,
            CombatAction::Move(Direction::East),
            CombatAction::Move(Direction::East),
        );
        party_turn(
            &mut two,
            CombatAction::Move(Direction::East),
            CombatAction::Move(Direction::East),
        );
    }
    assert_eq!(
        serde_json::to_string(&one).unwrap(),
        serde_json::to_string(&two).unwrap()
    );
    assert_eq!(one.board().len(), HEIGHT as usize);
    assert!(one.board().iter().all(|row| row.len() == WIDTH as usize));
}

#[test]
fn blueprint_loader_rejects_traversal_and_files_outside_the_workspace() {
    let root = std::env::temp_dir().join(format!("together-blueprints-{}", std::process::id()));
    std::fs::create_dir_all(root.join("together-items")).unwrap();
    assert!(Blueprint::load(&root, "../other").is_err());
    let raw = serde_json::to_vec(&blueprint(Slot::Weapon)).unwrap();
    std::fs::write(root.join("together-items/test-item.json"), raw).unwrap();
    assert!(
        Blueprint::load(&root, "test-item")
            .unwrap()
            .compile()
            .is_ok()
    );
    std::fs::write(
        root.join("together-items/oversized.json"),
        vec![b' '; 16_385],
    )
    .unwrap();
    assert!(
        Blueprint::load(&root, "oversized")
            .unwrap_err()
            .contains("16 KiB")
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("/etc/hosts", root.join("together-items/outside.json")).unwrap();
        assert!(
            Blueprint::load(&root, "outside")
                .unwrap_err()
                .contains("inside")
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn walls_stop_attacks_and_enemies_block_movement() {
    let mut run = run();
    let hero = run.heroes.get_mut(&1).unwrap();
    hero.x = 8;
    hero.y = 3;
    run.enemies = vec![Enemy {
        id: 1,
        x: 12,
        y: 3,
        hp: 100,
    }];
    party_turn(&mut run, CombatAction::Fire, CombatAction::Wait);
    assert_eq!(run.enemies[0].hp, 100);
    run.heroes.get_mut(&1).unwrap().x = 11;
    assert!(run.queue(1, CombatAction::Move(Direction::East)).is_err());
}

#[test]
fn a_party_with_starter_items_can_finish_three_floors_by_attacking_and_dodging() {
    let mut gear = Loadout::default();
    for id in ["spark-wand", "ember-spell"] {
        gear.equip(
            Blueprint::load(std::path::Path::new("."), id)
                .unwrap()
                .compile()
                .unwrap(),
        );
    }
    // Four seed classes cover every pickup permutation. Include the extreme
    // seed so wrapped pickup arithmetic is exercised through a complete raid.
    for seed in [0, 1, 2, 3, 123, u64::MAX] {
        let mut run = Run::new(
            seed,
            [
                (1, "One".into(), gear.clone()),
                (2, "Two".into(), gear.clone()),
            ],
        );
        let mut cleared = Vec::new();
        for _ in 0..300 {
            match run.phase {
                Phase::Won => break,
                Phase::Wiped => panic!("starter party wiped at turn {}", run.tick),
                Phase::Stairs => {
                    cleared.push(run.floor);
                    run.descend().unwrap();
                    continue;
                }
                Phase::Fighting => {}
            }
            let players: Vec<_> = run
                .heroes
                .iter()
                .filter(|(_, hero)| hero.hp > 0)
                .map(|(id, _)| *id)
                .collect();
            for id in players {
                let hero = &run.heroes[&id];
                let origin = (hero.x, hero.y);
                let danger = run.danger.contains(&origin);
                let nearest = run
                    .enemies
                    .iter()
                    .min_by_key(|enemy| distance(origin, (enemy.x, enemy.y)))
                    .unwrap();
                let target = (nearest.x, nearest.y);
                let mut candidates = Vec::new();
                if !danger {
                    for action in [CombatAction::Fire, CombatAction::Cast] {
                        let item = if action == CombatAction::Fire {
                            hero.loadout.weapon.as_ref()
                        } else {
                            hero.loadout.spell.as_ref()
                        }
                        .unwrap();
                        if distance(origin, target) <= item.range && clear_shot(origin, target) {
                            candidates.push((0, action));
                        }
                    }
                }
                for direction in [
                    Direction::East,
                    Direction::South,
                    Direction::West,
                    Direction::North,
                ] {
                    for steps in [1, 3] {
                        let (dx, dy) = direction.delta();
                        let destination = (origin.0 + dx * steps, origin.1 + dy * steps);
                        if run.danger.contains(&destination) {
                            continue;
                        }
                        let action = if steps == 1 {
                            CombatAction::Move(direction)
                        } else {
                            CombatAction::Dash(direction)
                        };
                        candidates.push((1 + distance(destination, target), action));
                    }
                }
                if !danger {
                    candidates.push((50, CombatAction::Wait));
                }
                candidates.sort_by_key(|(priority, _)| *priority);
                let action = candidates
                    .into_iter()
                    .map(|(_, action)| action)
                    .find(|action| run.clone().queue(id, action.clone()).is_ok())
                    .expect("party has a legal action");
                run.queue(id, action).unwrap();
            }
        }
        assert_eq!(
            run.phase,
            Phase::Won,
            "party did not finish (seed {seed}): {run:?}"
        );
        assert_eq!(run.floor, 3);
        assert_eq!(cleared, vec![1, 2]);
    }
}
