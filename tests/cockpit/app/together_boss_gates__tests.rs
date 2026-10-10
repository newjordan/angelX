use super::*;
use fortune::Mode;

fn on_floor(depth: u32, seed: u64, mode: Mode) -> Run {
    let mut run = Run::new(seed, 17, None);
    run.mode = mode;
    let pack = Pack::at(depth, Pack::Crypt);
    run.dungeon = layout::floor_for(mode, depth, pack, &mut run.rng);
    run.populate_boss_gates();
    run.enter(0, None);
    run
}

fn finish_room(run: &mut Run) {
    // A slain slime splits into bounded descendants, each settled normally.
    for _ in 0..8 {
        run.clear_for_test();
        if run.room().cleared {
            return;
        }
    }
    panic!(
        "fixture failed to clear room: {:?}, {:?}",
        run.phase, run.enemies
    );
}

fn kill_leader(run: &mut Run, which: usize) {
    let w = run.boss_gates.as_ref().unwrap().leaders[which].clone();
    run.enter(w.room, Some(2)); // deliberately use production entry, not fixture bypass
    assert!(
        run.enemies
            .iter()
            .any(|e| e.boss == Some(w.boss) && e.hp > 0)
    );
    finish_room(run); // actual zero-HP removal through step/settle_kills
    assert!(run.boss_gates.as_ref().unwrap().leaders[which].defeated);
    assert!(run.room().cleared);
}

fn reloaded(run: &Run) -> Run {
    let saved = serde_json::to_string(run).unwrap();
    let loaded: Run = serde_json::from_str(&saved).unwrap();
    assert!(loaded.valid_snapshot(), "reload rejected: {saved}");
    loaded
}

fn reach_without_gate(run: &Run) -> Vec<bool> {
    let g = run.boss_gates.as_ref().unwrap();
    let mut seen = vec![false; run.dungeon.rooms.len()];
    seen[0] = true;
    let mut todo = vec![0];
    while let Some(at) = todo.pop() {
        for dir in 0..4 {
            if let Some(next) = run.dungeon.neighbour(at, dir)
                && next != g.guardian
                && !seen[next]
            {
                seen[next] = true;
                todo.push(next);
            }
        }
    }
    seen
}

#[test]
fn boss_gates_profiles_have_checked_independent_leaders_and_reachable_resource_sites() {
    use boss_ecology::Structure;
    let mut structures = std::collections::BTreeSet::new();
    for seed in 0..64 {
        for depth in 1..=DEEPEST {
            let mut run = on_floor(depth, seed, Mode::LongWayDown);
            let g = run
                .boss_gates
                .clone()
                .expect("classic floors have supported profiles");
            structures.insert(format!("{:?}", g.economy.profile.structure));
            assert_eq!(g.depth, depth);
            assert_eq!(
                g.economy.profile,
                boss_population::profile(&run.dungeon, run.seed, run.raid_id)
            );
            let required = g.leaders.iter().filter(|l| l.prerequisite).count();
            match g.economy.profile.structure {
                Structure::Hive => {
                    assert_eq!(required, 1);
                    assert!(g.leaders.len() <= 2);
                }
                Structure::Clans => {
                    assert_eq!(g.access, boss_gates::Access::Any);
                    assert!(required >= 3);
                }
                Structure::Stronghold => {
                    assert_eq!(g.access, boss_gates::Access::All);
                    assert!(required >= 2);
                }
            }
            let seen = reach_without_gate(&run);
            for (i, l) in g.leaders.iter().enumerate() {
                assert!(
                    seen[l.room],
                    "sealed apex cannot be transit to ANY/ALL resource sites"
                );
                assert!(
                    g.leaders[..i]
                        .iter()
                        .all(|prev| prev.room != l.room && prev.boss != l.boss)
                );
                assert_eq!(run.dungeon.rooms[l.room].kind, RoomKind::Fight);
                let def = run.bosses[usize::from(l.boss)].clone();
                assert_eq!(def.only_in, run.dungeon.pack);
                assert!(!def.attacks.is_empty());
                assert!((300..=900).contains(&def.hp));
                run.enter(l.room, Some(2));
                let boss = run.enemies.iter().find(|e| e.boss == Some(l.boss)).unwrap();
                assert_eq!(boss.kind, EnemyKind::Boss);
                assert_eq!(boss.max_hp, def.hp);
                assert_eq!(boss.faction, Some(l.faction));
                assert_eq!(
                    run.room()
                        .tile((boss.x / TILE_UNITS) as i32, (boss.y / TILE_UNITS) as i32),
                    Tile::Floor
                );
                assert!(run.valid_snapshot());
            }
        }
    }
    assert_eq!(structures.len(), 3);
}

fn stronghold(depth: u32, required: usize) -> Run {
    for seed in 0..1000 {
        let run = on_floor(depth, seed, Mode::LongWayDown);
        let g = run.boss_gates.as_ref().unwrap();
        if g.access == boss_gates::Access::All
            && g.leaders.iter().filter(|l| l.prerequisite).count() == required
        {
            return run;
        }
    }
    panic!("missing structure fixture");
}

fn open_route(run: &mut Run) {
    let leaders: Vec<_> = run
        .boss_gates
        .as_ref()
        .unwrap()
        .leaders
        .iter()
        .enumerate()
        .filter(|(_, l)| l.prerequisite)
        .map(|(i, _)| i)
        .collect();
    for i in leaders {
        kill_leader(run, i);
    }
}

#[test]
fn boss_gates_population_is_deterministic_without_consuming_combat_rng() {
    let a = on_floor(5, 200, Mode::LongWayDown);
    let b = on_floor(5, 200, Mode::LongWayDown);
    assert_eq!(a.boss_gates, b.boss_gates);
    assert_eq!(
        serde_json::to_value(&a.dungeon).unwrap(),
        serde_json::to_value(&b.dungeon).unwrap()
    );
    let mut changed = std::collections::BTreeSet::new();
    for raid in 0..12 {
        let run = Run::new(5, raid, None);
        changed.insert(serde_json::to_string(&run.boss_gates).unwrap());
    }
    assert!(changed.len() > 1);
}

#[test]
fn boss_gates_stairs_lair_threshold_are_absent_before_prerequisites_and_allow_retreat() {
    for depth in [1, 3, 6] {
        let mut run = on_floor(depth, 2, Mode::LongWayDown);
        let gate = run.boss_gates.as_ref().unwrap().guardian;
        run.rune = Some(runes::Rune {
            kind: runes::RuneKind::Bounty,
            x: 10.0,
            y: 10.0,
            at: 0,
        });
        run.enter(gate, Some(2));
        assert!(
            run.rune.is_none(),
            "a previous room rune cannot leak into the seal"
        );
        assert_eq!(run.phase, Phase::Exploring);
        assert!(run.enemies.is_empty() && !run.barred() && !run.waves.pending());
        assert!(run.traps.is_empty());
        assert!(run.boss_gate_line().unwrap().contains("sealed"));
        assert!(run.boss_gate_line().unwrap().contains("retreat"));
        let items = run.room().items.len();
        let score = run.score;
        // All damage sources act on enemies, so a sealed guardian has no HP
        // target, and an empty sealed lair cannot issue a clear/reward.
        run.phase = Phase::Fighting;
        run.step(&BTreeMap::new());
        assert!(!run.room().cleared);
        assert_eq!(run.room().items.len(), items);
        assert_eq!(run.score, score);
        assert!(
            !run.cues
                .iter()
                .any(|c| c == "room_clear" || c == "flawless" || c == "grail")
        );
        run.enter(gate, Some(2));
        let (sx, sy) = (run.room().width() / 2.0, run.room().height() / 2.0);
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (sx, sy);
        run.step(&BTreeMap::new());
        assert_eq!(run.dungeon.depth, depth, "stair tile cannot descend early");
        run.descend();
        assert_eq!(run.dungeon.depth, depth, "transition itself is guarded");
        let dir = (0..4)
            .find(|&d| run.dungeon.neighbour(gate, d).is_some())
            .unwrap();
        let next = run.dungeon.neighbour(gate, dir).unwrap();
        let (w, h) = (run.room().width(), run.room().height());
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = match dir {
            0 => (w / 2.0, 0.2),
            1 => (w - 0.2, h / 2.0),
            2 => (w / 2.0, h - 0.2),
            _ => (0.2, h / 2.0),
        };
        run.travel();
        assert_eq!(run.at, next, "the early approach must allow retreat");
        assert!(run.valid_snapshot());
    }
}

#[test]
fn boss_gates_all_and_any_handle_defeat_order_and_roundtrip() {
    for depth in [1, 2, 3, 6] {
        for reverse in [false, true] {
            let mut run = on_floor(depth, 7, Mode::LongWayDown);
            let g = run.boss_gates.clone().unwrap();
            let mut order: Vec<_> = g
                .leaders
                .iter()
                .enumerate()
                .filter(|(_, l)| l.prerequisite)
                .map(|(i, _)| i)
                .collect();
            if reverse {
                order.reverse();
            }
            for (n, &i) in order.iter().enumerate() {
                kill_leader(&mut run, i);
                run = reloaded(&run);
                let open = g.access == boss_gates::Access::Any || n + 1 == order.len();
                assert_eq!(run.boss_gates.as_ref().unwrap().unlocked(), open);
                run.enter(g.guardian, Some(2));
                assert_eq!(!run.guardian_locked(), open);
                if !open {
                    assert!(run.enemies.is_empty());
                }
                run = reloaded(&run);
            }
            // Cues are tick-local, while access/progress persist across travel.
            assert!(run.boss_gates.as_ref().unwrap().unlocked());
            run.enter(g.guardian, Some(2));
            finish_room(&mut run);
            assert!(run.room().cleared);
            assert_eq!(
                run.boss_gates.as_ref().unwrap().guardian_defeated,
                g.guardian_boss.is_some() || depth == FLOORS
            );
            assert!(run.valid_snapshot());
        }
    }
}

#[test]
fn boss_gates_visits_ordinary_clears_and_removal_without_death_do_not_count() {
    let mut run = (0..128)
        .find_map(|seed| {
            let run = on_floor(1, seed, Mode::LongWayDown);
            let g = run.boss_gates.as_ref().unwrap();
            run.dungeon
                .rooms
                .iter()
                .enumerate()
                .any(|(i, r)| r.kind == RoomKind::Fight && !g.leaders.iter().any(|l| l.room == i))
                .then_some(run)
        })
        .expect("ordinary camp fixture");
    let g = run.boss_gates.clone().unwrap();
    let ordinary = run
        .dungeon
        .rooms
        .iter()
        .enumerate()
        .find(|(i, r)| r.kind == RoomKind::Fight && !g.leaders.iter().any(|l| l.room == *i))
        .unwrap()
        .0;
    run.enter(ordinary, Some(2));
    finish_room(&mut run);
    assert_eq!(
        run.boss_gates.as_ref().unwrap().leaders,
        g.leaders,
        "ordinary casualties have no leader death evidence"
    );
    run.enter(g.leaders[0].room, Some(2));
    run.calm_for_test();
    for e in &mut run.enemies {
        if e.boss.is_none() {
            e.hp = 0;
        }
    }
    run.step(&BTreeMap::new());
    assert!(!run.boss_gates.as_ref().unwrap().leaders[0].defeated);
    run.enemies.clear(); // not a zero-HP kill; must not clear this boss room
    run.step(&BTreeMap::new());
    assert!(!run.room().cleared);
    assert_eq!(run.boss_gates.as_ref().unwrap().leaders, g.leaders);
}

#[test]
fn boss_gates_last_leader_death_with_wipe_reentry_and_replay_do_not_duplicate_rewards() {
    let mut run = stronghold(1, 2);
    kill_leader(&mut run, 0);
    let w = run.boss_gates.as_ref().unwrap().leaders[1].clone();
    run.enter(w.room, Some(2));
    run.calm_for_test();
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    let corpse = run
        .enemies
        .iter()
        .find(|e| e.boss == Some(w.boss))
        .unwrap()
        .clone();
    run.players.get_mut(&1).unwrap().hp = 0;
    run.players.get_mut(&1).unwrap().winds = 0;
    run.step(&BTreeMap::new());
    assert_eq!(run.phase, Phase::Wiped);
    assert!(run.boss_gates.as_ref().unwrap().unlocked());
    assert!(
        !run.room().cleared,
        "wipe interrupted room clear, not boss defeat"
    );
    let items = run.room().items.len();
    let score = run.score;
    run = reloaded(&run);
    run.enemies.push(corpse);
    assert!(
        !run.valid_snapshot(),
        "a replayed boss cannot be an active snapshot"
    );
    run.settle_kills(); // replay after the corpse was removed
    assert_eq!(run.room().items.len(), items);
    assert_eq!(run.score, score);
    run.players.get_mut(&1).unwrap().hp = 100;
    run.enter(w.room, Some(2));
    assert!(!run.enemies.iter().any(|e| e.boss == Some(w.boss)));
    run.clear_for_test();
    assert!(
        !run.cues
            .iter()
            .any(|c| c == "boss_unsealed" || c.starts_with("boss_fall:"))
    );
    assert!(run.valid_snapshot());
}

#[test]
fn boss_gates_guardian_death_with_wipe_cannot_respawn_or_double_award_the_boss() {
    let mut run = on_floor(3, 8, Mode::LongWayDown);
    open_route(&mut run);
    let gate = run.boss_gates.as_ref().unwrap().guardian;
    run.enter(gate, Some(2));
    run.calm_for_test();
    for e in &mut run.enemies {
        e.hp = 0;
    }
    let hero = run.players.get_mut(&1).unwrap();
    hero.hp = 0;
    hero.winds = 0;
    run.step(&BTreeMap::new());
    assert_eq!(run.phase, Phase::Wiped);
    assert!(run.boss_gates.as_ref().unwrap().guardian_defeated);
    run = reloaded(&run);
    run.players.get_mut(&1).unwrap().hp = 100;
    run.enter(gate, Some(2));
    assert!(!run.enemies.iter().any(|e| e.kind == EnemyKind::Dragon));
    finish_room(&mut run);
    assert!(!run.cues.iter().any(|c| c.starts_with("boss_fall:")));
}

#[test]
fn boss_gates_new_floor_home_and_restart_reset_only_current_floor_state() {
    let mut run = on_floor(1, 11, Mode::LongWayDown);
    open_route(&mut run);
    let gate = run.boss_gates.as_ref().unwrap().guardian;
    run.enter(gate, Some(2));
    run.clear_for_test();
    run.descend();
    assert_eq!(run.dungeon.depth, 2);
    let g = run.boss_gates.as_ref().unwrap();
    assert_eq!(g.depth, 2);
    assert!(!g.unlocked() && g.leaders.iter().all(|w| !w.defeated));
    assert!(run.valid_snapshot());
    let home = Run::at_home(11, 17, None, home::Home::default(), Spoils::default());
    assert!(home.boss_gates.is_none());
    assert!(home.valid_snapshot());
    let fresh = Run::new(11, 17, None);
    assert!(!fresh.boss_gates.as_ref().unwrap().unlocked());
}

#[test]
fn boss_gates_legacy_active_and_cleared_guardians_are_never_retroactively_sealed() {
    for cleared in [false, true] {
        let mut run = on_floor(1, 1, Mode::LongWayDown);
        run.clear_prerequisites_for_test();
        let gate = run.boss_gates.as_ref().unwrap().guardian;
        run.enter(gate, Some(2));
        if cleared {
            run.clear_for_test();
        }
        let mut value = serde_json::to_value(&run).unwrap();
        value.as_object_mut().unwrap().remove("boss_gates");
        let mut legacy: Run = serde_json::from_value(value).unwrap();
        assert!(legacy.valid_snapshot());
        assert!(legacy.boss_gates.is_none() && !legacy.guardian_locked());
        if !cleared {
            legacy.clear_for_test();
        }
        legacy.descend();
        assert_eq!(legacy.dungeon.depth, 2);
        assert!(legacy.boss_gates.is_some(), "next generated floor opts in");
    }
}

#[test]
fn boss_gates_empty_catalog_and_checked_custom_bosses_are_safe() {
    for depth in 1..=DEEPEST {
        let mut run = on_floor(depth, 3, Mode::LongWayDown);
        run.bosses.clear();
        run.dungeon = layout::floor(depth, Pack::at(depth, Pack::Crypt), &mut run.rng);
        run.populate_boss_gates();
        run.enter(0, None);
        assert!(run.boss_gates.is_some());
        assert!(run.valid_snapshot());
        open_route(&mut run);
        let gate = run.boss_gates.as_ref().unwrap().guardian;
        run.enter(gate, Some(2));
        assert!(
            !run.enemies.is_empty(),
            "open end encounter has a finite escort, even without apex"
        );
    }
    for (pack, where_, depth) in [
        (Pack::Crypt, "crypt", 1),
        (Pack::Hellforge, "hellforge", 3),
        (Pack::Unknown, "unknown", 6),
    ] {
        let mut run = on_floor(depth, 9, Mode::LongWayDown);
        let raw = format!(
            "name CUSTOM WARDEN\nwhere {where_}\nhp 500\nattack aimed shots=1 speed=5 every=80 damage=10\nart\n77\nHH\n"
        );
        let custom = bosses::check("custom", &raw).unwrap().0;
        assert_eq!(custom.only_in, pack);
        run.bosses = vec![custom];
        run.dungeon = layout::floor(depth, pack, &mut run.rng);
        run.populate_boss_gates();
        run.enter(0, None);
        let g = run.boss_gates.as_ref().unwrap();
        assert!(
            g.leaders
                .iter()
                .any(|w| run.bosses[usize::from(w.boss)].id == "custom")
                || g.guardian_boss
                    .is_some_and(|b| run.bosses[usize::from(b)].id == "custom")
        );
        assert!(run.valid_snapshot());
    }
}

#[test]
fn boss_gates_malformed_graphs_and_active_snapshots_are_rejected() {
    let base = on_floor(1, 12, Mode::LongWayDown);
    let rejected = |run: Run| assert!(!run.valid_snapshot());
    let mut run = base.clone();
    run.boss_gates.as_mut().unwrap().guardian = usize::MAX;
    rejected(run);
    let mut run = base.clone();
    run.boss_gates.as_mut().unwrap().depth = 2;
    rejected(run);
    let mut run = base.clone();
    run.boss_gates.as_mut().unwrap().leaders[0].boss = u8::MAX;
    rejected(run);
    let mut run = base.clone();
    run.boss_gates.as_mut().unwrap().leaders[0].room = usize::MAX;
    rejected(run);
    let mut run = base.clone();
    run.boss_gates.as_mut().unwrap().leaders[1] =
        run.boss_gates.as_ref().unwrap().leaders[0].clone();
    rejected(run);
    let mut run = base.clone();
    run.boss_gates.as_mut().unwrap().leaders[0].room = run.boss_gates.as_ref().unwrap().guardian;
    rejected(run);
    let mut run = base.clone();
    let gate = run.boss_gates.as_ref().unwrap().guardian;
    run.dungeon.rooms[gate].cleared = true;
    rejected(run);
    let mut run = base.clone();
    let w = run.boss_gates.as_ref().unwrap().leaders[0].room;
    run.dungeon.rooms[w].cleared = true;
    rejected(run);
    let mut run = base.clone();
    let w = run.boss_gates.as_ref().unwrap().leaders[0].room;
    run.dungeon.rooms[w].doors = [false; 4];
    rejected(run);
    let mut run = base.clone();
    let gate = run.boss_gates.as_ref().unwrap().guardian;
    run.enter(gate, Some(2));
    // Production emission now refuses a locked guardian. Deliberately inject
    // an unadmitted raw body to test snapshot rejection rather than a no-op.
    assert!(run.spawn_at(EnemyKind::Skeleton, 10.0, 10.0).is_none());
    run.spawn_body(EnemyKind::Skeleton, 10.0, 10.0);
    rejected(run);
    let mut run = base.clone();
    let w = run.boss_gates.as_ref().unwrap().leaders[0].room;
    run.enter(w, Some(2));
    run.enemies.clear();
    rejected(run);
    let mut run = base.clone();
    let w = run.boss_gates.as_ref().unwrap().leaders[0].room;
    run.enter(w, Some(2));
    run.boss_gates.as_mut().unwrap().leaders[0].defeated = true;
    rejected(run);
    let mut run = base.clone();
    let gate = run.boss_gates.as_ref().unwrap().guardian;
    run.dungeon.rooms[gate].cell = (i32::MAX, i32::MAX);
    rejected(run);
    let mut run = base.clone();
    let gate = run.boss_gates.as_ref().unwrap().guardian;
    run.dungeon.rooms[gate].doors = [false; 4];
    rejected(run);
    let mut oversized = serde_json::to_value(&base).unwrap();
    let l = oversized["boss_gates"]["leaders"][0].clone();
    oversized["boss_gates"]["leaders"] =
        serde_json::json!([l.clone(), l.clone(), l.clone(), l.clone(), l]);
    assert!(
        !serde_json::from_value::<Run>(oversized)
            .unwrap()
            .valid_snapshot(),
        "bounded leader list validated at admission"
    );
    let mut unknown = serde_json::to_value(&base).unwrap();
    unknown["boss_gates"]["client_defeat"] = true.into();
    assert!(serde_json::from_value::<Run>(unknown).is_err());
}

#[test]
fn boss_gates_gauntlet_boss_rush_and_hold_the_stair_stay_deliberately_ungated() {
    for mode in [Mode::Gauntlet, Mode::HoldTheStair] {
        for depth in 1..=DEEPEST {
            let mut run = on_floor(depth, 4, mode);
            assert!(run.boss_gates.is_none());
            let gate = run.dungeon.rooms.len() - 1;
            run.enter(gate, Some(2));
            assert!(!run.guardian_locked() && !run.enemies.is_empty());
            finish_room(&mut run);
            if depth < DEEPEST {
                run.descend();
                assert_eq!(run.dungeon.depth, depth + 1);
            }
        }
    }
}

#[test]
fn boss_gates_giants_feast_keeps_native_boss_scaling_and_normal_giant_mobs() {
    let mut run = on_floor(2, 10, Mode::GiantsFeast);
    let w = run.boss_gates.as_ref().unwrap().leaders[0].clone();
    run.enter(w.room, Some(2));
    let boss = run.enemies.iter().find(|e| e.boss == Some(w.boss)).unwrap();
    assert_eq!(boss.max_hp, run.bosses[usize::from(w.boss)].hp);
    assert!(
        run.enemies
            .iter()
            .filter(|e| e.boss.is_none())
            .all(|e| e.max_hp == e.kind.hp() * 9 / 5)
    );
    assert!(run.valid_snapshot());
}

#[test]
fn boss_gates_pit_remains_optional_and_never_counts_as_a_leader() {
    let mut run = (0..32)
        .map(|seed| on_floor(2, seed, Mode::LongWayDown))
        .find(|run| run.dungeon.rooms.iter().any(|r| r.kind == RoomKind::Pit))
        .unwrap();
    let graph = run.boss_gates.clone();
    let pit = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Pit)
        .unwrap();
    run.enter(pit, Some(2));
    run.clear_for_test();
    let state = run.boss_gates.as_ref().unwrap();
    assert_eq!(state.leaders, graph.as_ref().unwrap().leaders);
    assert_eq!(state.economy, graph.as_ref().unwrap().economy);
    assert!(state.pit_defeated);
    assert!(!state.unlocked());
}

#[test]
fn boss_gates_guest_state_carries_progress_and_input_claims_cannot_grant_defeat() {
    let mut run = stronghold(1, 2);
    kill_leader(&mut run, 0);
    let gate = run.boss_gates.as_ref().unwrap().guardian;
    run.enter(gate, Some(2));
    let snapshot =
        super::super::together_guest::settlement_snapshot_for_test(&run, "ordinary notice");
    let guest: serde_json::Value = serde_json::from_slice(&snapshot).unwrap();
    assert!(guest["notice"].as_str().unwrap().contains("1/2"));
    assert!(
        guest["boss_gates"]["leaders"][0]["defeated"]
            .as_bool()
            .unwrap()
    );
    assert!(
        !guest["boss_gates"]["leaders"][1]["defeated"]
            .as_bool()
            .unwrap()
    );
    let forged = serde_json::json!({ "boss_gates": { "defeated": true }, "boss_defeat": true });
    assert!(serde_json::from_value::<Input>(forged).is_err());
    run.step(&BTreeMap::from([(1, Input::default())]));
    assert!(run.guardian_locked());
    assert!(!run.boss_gates.as_ref().unwrap().leaders[1].defeated);
}

#[test]
fn boss_gates_native_mirror_live_updates_carry_progress_and_reject_bad_active_state() {
    let mut host = stronghold(1, 2);
    let mut guest = reloaded(&host);
    kill_leader(&mut host, 0);
    let live: mirror::Live =
        serde_json::from_slice(&serde_json::to_vec(&host.live()).unwrap()).unwrap();
    assert!(guest.apply_live(live));
    assert_eq!(guest.boss_gates, host.boss_gates);
    assert!(guest.valid_snapshot());
    let gate = host.boss_gates.as_ref().unwrap().guardian;
    host.enter(gate, Some(2));
    assert!(guest.apply_live(host.live()));
    assert!(guest.guardian_locked());
    kill_leader(&mut host, 1);
    host.enter(gate, Some(2)); // guest may miss the chief's last tick entirely
    assert!(guest.apply_live(host.live()));
    assert_eq!(guest.boss_gates, host.boss_gates);
    assert!(guest.valid_snapshot());
    let unchanged = serde_json::to_value(&guest).unwrap();
    let mut bad = host.live();
    bad.boss_gates.as_mut().unwrap().leaders[0].room = usize::MAX;
    assert!(!guest.apply_live(bad));
    assert_eq!(serde_json::to_value(&guest).unwrap(), unchanged);
    let mut bad = host.live();
    bad.enemies.clear(); // guardian claims fighting but has no real boss
    assert!(!guest.apply_live(bad));
    assert_eq!(serde_json::to_value(&guest).unwrap(), unchanged);
    let mut bad = host.live();
    bad.boss_gates = None; // cannot drop a current floor's active graph
    assert!(!guest.apply_live(bad));
    assert_eq!(serde_json::to_value(&guest).unwrap(), unchanged);
}

#[test]
fn boss_gates_duplicate_dead_bosses_in_one_settlement_award_only_once() {
    let mut run = on_floor(2, 97, Mode::LongWayDown);
    let w = run.boss_gates.as_ref().unwrap().leaders[0].clone();
    run.enter(w.room, Some(2));
    run.calm_for_test();
    for e in &mut run.enemies {
        e.hp = 0;
    }
    let mut once = run.clone();
    once.settle_kills();
    let mut duplicate = run
        .enemies
        .iter()
        .find(|e| e.boss == Some(w.boss))
        .unwrap()
        .clone();
    duplicate.id += 1000;
    run.enemies.push(duplicate);
    assert!(!run.valid_snapshot());
    run.settle_kills();
    assert_eq!(run.score, once.score);
    assert_eq!(
        serde_json::to_value(&run.room().items).unwrap(),
        serde_json::to_value(&once.room().items).unwrap()
    );
    assert_eq!(
        run.cues
            .iter()
            .filter(|c| c.starts_with("leader_fall:"))
            .count(),
        1
    );
}

#[test]
fn boss_ecology_terrain_yields_change_budget_leaders_and_apex_distribution() {
    use boss_ecology::Structure;
    let mut run = on_floor(5, 21, Mode::LongWayDown);
    let mut sparse = run.dungeon.clone();
    let mut rich = sparse.clone();
    for (floor, tile) in [(&mut sparse, Tile::Floor), (&mut rich, Tile::Hazard)] {
        for room in floor.rooms.iter_mut().filter(|r| r.kind == RoomKind::Fight) {
            for y in 1..room.rows - 1 {
                for x in 1..room.cols - 1 {
                    room.set(x, y, tile);
                }
            }
        }
    }
    // Fixtures deliberately change support terrain, not a random label or name.
    let a = boss_population::populate(&sparse, &mut run.bosses, run.seed, run.raid_id, run.mode)
        .unwrap();
    let b =
        boss_population::populate(&rich, &mut run.bosses, run.seed, run.raid_id, run.mode).unwrap();
    assert_eq!(a.economy.profile.structure, Structure::Hive);
    assert_eq!(b.economy.profile.structure, Structure::Hive);
    assert!(a.economy.profile.support < b.economy.profile.support);
    assert!(
        a.economy
            .camps
            .iter()
            .map(|c| u16::from(c.base_budget))
            .sum::<u16>()
            < b.economy
                .camps
                .iter()
                .map(|c| u16::from(c.base_budget))
                .sum::<u16>()
    );
    assert_eq!(a.leaders.len(), 1);
    assert_eq!(
        b.leaders.len(),
        2,
        "productive outer brood supports an optional second leader"
    );
    assert!(a.guardian_boss.is_none());
    assert!(b.guardian_boss.is_some(), "rich brood supports a rare apex");
    assert!(!b.leaders[1].prerequisite);
}

#[test]
fn boss_ecology_supported_structures_have_distinct_density_and_rosters() {
    let mut densities = Vec::new();
    for depth in [1, 2, 5] {
        let mut run = on_floor(depth, 13, Mode::LongWayDown);
        let g = run.boss_gates.clone().unwrap();
        let mean = g
            .economy
            .camps
            .iter()
            .map(|c| usize::from(c.base_budget))
            .sum::<usize>()
            / g.economy.camps.len();
        densities.push(mean);
        run.enter(g.leaders[0].room, Some(2));
        let minions: Vec<_> = run
            .enemies
            .iter()
            .filter(|e| e.camp_slot.is_some())
            .collect();
        assert!(minions.len() >= 3);
        match depth {
            1 => assert!(minions.iter().any(|e| e.kind == EnemyKind::Skeleton)),
            2 => assert!(
                minions
                    .iter()
                    .any(|e| matches!(e.kind, EnemyKind::Sapper | EnemyKind::Hob))
            ),
            5 => assert!(minions.iter().filter(|e| e.kind == EnemyKind::Bat).count() >= 3),
            _ => unreachable!(),
        }
        assert!(
            !run.waves.pending(),
            "ecological camps deploy once, not frame-timed reserves"
        );
    }
    assert!(densities[2] > densities[0] && densities[2] > densities[1]);
}

#[test]
fn boss_ecology_chief_real_death_redirects_capped_reserve_once_and_weakens_future_camps() {
    let mut run = on_floor(2, 41, Mode::LongWayDown);
    let g = run.boss_gates.clone().unwrap();
    let l = g.leaders[0].clone();
    run.enter(l.room, Some(2));
    let before = run.boss_gates.as_ref().unwrap().economy.clone();
    run.settle_kills(); // living chief and mere entry are not a political event
    assert_eq!(run.boss_gates.as_ref().unwrap().economy, before);
    let ordinary = run
        .enemies
        .iter_mut()
        .find(|e| e.camp_slot.is_some())
        .unwrap();
    ordinary.hp = 0;
    run.settle_kills();
    assert!(run.boss_gates.as_ref().unwrap().economy.factions[0].controlled);
    let mut corpse = run
        .enemies
        .iter()
        .find(|e| e.boss == Some(l.boss))
        .unwrap()
        .clone();
    corpse.hp = 0;
    run.enemies
        .iter_mut()
        .find(|e| e.boss == Some(l.boss))
        .unwrap()
        .hp = 0;
    let before_loss = run.boss_gates.as_ref().unwrap().economy.clone();
    let active = before_loss.camp(l.room).unwrap().budget;
    run.settle_kills();
    let after = run.boss_gates.as_ref().unwrap().economy.clone();
    assert!(!after.factions[0].controlled);
    assert_eq!(after.factions[0].reserve, 0);
    assert_eq!(
        after.factions[1].reserve,
        (before_loss.factions[1].reserve + before_loss.factions[0].reserve.min(3)).min(9)
    );
    assert_eq!(
        after.camp(l.room).unwrap().budget,
        active,
        "active escort remains physically real"
    );
    assert!(
        after
            .camps
            .iter()
            .zip(&before_loss.camps)
            .any(|(a, b)| a.faction == 0 && !a.deployed && a.budget < b.budget)
    );
    assert!(run.boss_support_line().unwrap().contains("control lost"));
    let score = run.score;
    let items = run.room().items.len();
    run.enemies.push(corpse);
    run.settle_kills();
    assert_eq!(run.boss_gates.as_ref().unwrap().economy, after);
    assert_eq!((run.score, run.room().items.len()), (score, items));
    run = reloaded(&run);
    run.enter(l.room, Some(2));
    assert!(!run.enemies.iter().any(|e| e.boss == Some(l.boss)));
    assert_eq!(run.boss_gates.as_ref().unwrap().economy, after);
}

#[test]
fn boss_ecology_finite_deployment_casualties_and_active_cap_survive_wipe_reentry() {
    let mut run = on_floor(2, 55, Mode::LongWayDown);
    let room = run.boss_gates.as_ref().unwrap().leaders[0].room;
    run.enter(room, Some(2));
    let mut dead = run
        .enemies
        .iter()
        .find(|e| e.camp_slot.is_some())
        .unwrap()
        .clone();
    let slot = dead.camp_slot;
    dead.hp = 0;
    run.enemies
        .iter_mut()
        .find(|e| e.camp_slot == slot)
        .unwrap()
        .hp = 0;
    run.settle_kills();
    let economy = run.boss_gates.as_ref().unwrap().economy.clone();
    let score = run.score;
    run.enter(room, Some(2));
    assert!(!run.enemies.iter().any(|e| e.camp_slot == slot));
    assert_eq!(run.boss_gates.as_ref().unwrap().economy, economy);
    run.enemies.push(dead);
    run.settle_kills();
    assert_eq!(run.score, score);
    let spot = floor_spot(&run);
    for _ in 0..500 {
        run.spawn_at(EnemyKind::Bat, spot.0, spot.1);
    }
    assert!(run.enemies.len() <= boss_ecology::ENTITY_CAP);
    assert!(
        run.boss_gates.as_ref().unwrap().economy.spawn_spent
            <= run.boss_gates.as_ref().unwrap().economy.spawn_allowance
    );
    assert!(run.valid_snapshot());
}

#[test]
fn boss_ecology_optional_leader_not_required_and_starved_hive_can_finish() {
    let mut run = (0..1000)
        .find_map(|seed| {
            let r = on_floor(5, seed, Mode::LongWayDown);
            (r.boss_gates.as_ref().unwrap().leaders.len() == 2
                && r.boss_gates.as_ref().unwrap().guardian_boss.is_some())
            .then_some(r)
        })
        .expect("rich hive fixture");
    let g = run.boss_gates.clone().unwrap();
    assert!(!g.leaders[1].prerequisite);
    kill_leader(&mut run, 1);
    assert!(
        !run.boss_gates.as_ref().unwrap().unlocked(),
        "optional outer brood is not the core lock"
    );
    kill_leader(&mut run, 0);
    let state = run.boss_gates.as_ref().unwrap();
    assert!(state.unlocked());
    assert!(
        state
            .economy
            .factions
            .iter()
            .all(|f| f.reserve == 0 && !f.controlled)
    );
    run.enter(g.guardian, Some(2));
    assert!(
        run.enemies
            .iter()
            .any(|e| e.boss == g.guardian_boss && e.kind == EnemyKind::Boss)
    );
    finish_room(&mut run);
    assert!(run.room().cleared);
    assert!(run.valid_snapshot());
}

#[test]
fn boss_ecology_invalid_economy_journal_and_mirrored_mutations_are_rejected_atomically() {
    let mut host = on_floor(2, 63, Mode::LongWayDown);
    let mut guest = reloaded(&host);
    let mut corruptions = Vec::new();
    let mut bad = host.clone();
    bad.boss_gates.as_mut().unwrap().economy.factions[0].reserve = 99;
    corruptions.push(bad);
    let mut bad = host.clone();
    bad.boss_gates.as_mut().unwrap().economy.camps[0].budget = 99;
    corruptions.push(bad);
    let mut bad = host.clone();
    bad.boss_gates
        .as_mut()
        .unwrap()
        .economy
        .events
        .push(boss_ecology::Event::Loss(0));
    corruptions.push(bad);
    let mut bad = host.clone();
    bad.boss_gates
        .as_mut()
        .unwrap()
        .economy
        .events
        .push(boss_ecology::Event::Deploy(usize::MAX));
    corruptions.push(bad);
    let mut bad = host.clone();
    bad.boss_gates.as_mut().unwrap().economy.profile.support += 1;
    corruptions.push(bad);
    let mut bad = host.clone();
    bad.boss_gates.as_mut().unwrap().economy.camps[0].capacity += 1;
    corruptions.push(bad);
    let mut bad = host.clone();
    bad.boss_gates.as_mut().unwrap().access = boss_gates::Access::All;
    corruptions.push(bad);
    for bad in corruptions {
        assert!(!bad.valid_snapshot());
        let unchanged = serde_json::to_value(&guest).unwrap();
        assert!(!guest.apply_live(bad.live()));
        assert_eq!(serde_json::to_value(&guest).unwrap(), unchanged);
    }
    let old = host.live();
    kill_leader(&mut host, 0);
    host.enter(host.boss_gates.as_ref().unwrap().guardian, Some(2));
    assert!(
        guest.apply_live(host.live()),
        "guest can skip both deployment and death ticks"
    );
    assert_eq!(guest.boss_gates, host.boss_gates);
    assert!(guest.valid_snapshot());
    let unchanged = serde_json::to_value(&guest).unwrap();
    assert!(!guest.apply_live(old), "cannot rewind political state");
    assert_eq!(serde_json::to_value(&guest).unwrap(), unchanged);
    let mut bad = host.live();
    bad.enemies.clear(); // an unspent escort cannot magically clear a chief-free route
    assert!(!guest.apply_live(bad));
}

#[test]
fn boss_ecology_empty_geometry_full_catalog_and_out_of_range_mode_safe() {
    let mut run = Run::new(1, 2, None);
    run.dungeon.rooms.clear();
    run.populate_boss_gates();
    assert!(run.boss_gates.is_none());
    assert!(!run.valid_snapshot());
    let mut run = on_floor(2, 8, Mode::LongWayDown);
    run.bosses = (0..64)
        .map(|n| {
            let mut b = bosses::builtin()[0].clone();
            b.id = format!("other-{n}");
            b
        })
        .collect();
    run.populate_boss_gates();
    assert!(
        run.boss_gates.is_none(),
        "full foreign catalog falls back, never narrows u8 indices"
    );
    assert_eq!(run.bosses.len(), 64);
    run.dungeon.depth = DEEPEST + 1;
    run.populate_boss_gates();
    assert!(run.boss_gates.is_none());
}

#[test]
fn boss_ecology_together_shooter_native_renderer_compact_normal_and_guest_projection() {
    let mut run = stronghold(1, 2);
    kill_leader(&mut run, 0);
    let guardian = run.boss_gates.as_ref().unwrap().guardian;
    run.enter(guardian, Some(2));
    for (width, height) in [(80, 24), (120, 40)] {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| {
                assert!(crate::ui::viz::shooter_viz::render(
                    frame,
                    &run,
                    frame.area(),
                    "",
                    true,
                    false,
                    false,
                    "",
                    None,
                    None,
                    None,
                    None,
                    false
                ));
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let text = buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("sealed ALL 1/2"));
        assert!(text.contains("tomb tribute"));
        assert!(text.contains("reserve"));
        let snapshot = text.clone();
        terminal
            .draw(|frame| {
                crate::ui::viz::shooter_viz::render(
                    frame,
                    &run,
                    frame.area(),
                    "",
                    true,
                    false,
                    false,
                    "",
                    None,
                    None,
                    None,
                    None,
                    false,
                );
            })
            .unwrap();
        assert_eq!(
            snapshot,
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>()
        );
        println!("ECOLOGY_RENDER {width}x{height} {text}");
    }
    let bytes = super::super::together_guest::settlement_snapshot_for_test(&run, "");
    let guest: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(
        guest["boss_support"]
            .as_str()
            .unwrap()
            .contains("control lost")
    );
    assert_eq!(
        guest["boss_gates"],
        serde_json::to_value(run.boss_gate_state()).unwrap()
    );
}

#[test]
fn boss_ecology_secret_transition_and_immutable_geography_survive_guest_deltas() {
    let mut host = (0..256)
        .find_map(|seed| {
            let r = on_floor(2, seed, Mode::LongWayDown);
            r.dungeon.secret.is_some().then_some(r)
        })
        .unwrap();
    let mut guest = reloaded(&host);
    let secret = host.dungeon.secret.unwrap();
    host.enter(secret.host, Some(2));
    let before = host.boss_gates.clone();
    host.blast_wall(None);
    assert!(host.dungeon.secret.unwrap().found);
    assert_eq!(
        host.boss_gates, before,
        "one native secret transition is not new political support"
    );
    assert!(host.valid_snapshot());
    assert!(guest.apply_live(host.live()));
    assert!(guest.valid_snapshot());
    let old = guest.boss_gates.clone();
    let mut bad = host.live();
    bad.boss_gates.as_mut().unwrap().geography[0].cell = (9, 9);
    assert!(!guest.apply_live(bad));
    assert_eq!(guest.boss_gates, old);
}

#[test]
fn boss_ecology_rarity_and_all_supported_pack_routes_across_both_starting_packs() {
    let (mut rich, mut apex, mut optional_stronghold) = (0, 0, 0);
    for seed in 0..32 {
        for first in [Pack::Crypt, Pack::Cavern] {
            for depth in 1..=DEEPEST {
                let mut run = on_floor(depth, seed, Mode::LongWayDown);
                run.dungeon = layout::floor(depth, Pack::at(depth, first), &mut run.rng);
                run.populate_boss_gates();
                run.enter(0, None);
                let g = run.boss_gates.clone().unwrap();
                let seen = reach_without_gate(&run);
                assert!(g.leaders.iter().all(|l| seen[l.room]));
                if depth == 5 && g.economy.profile.mean() >= 4 {
                    rich += 1;
                    apex += usize::from(g.guardian_boss.is_some());
                }
                if g.economy.profile.structure == boss_ecology::Structure::Stronghold {
                    optional_stronghold += usize::from(g.leaders.iter().any(|l| !l.prerequisite));
                }
                assert!(run.valid_snapshot());
                // ANY never demands every chief, even when some chiefs are alive.
                if g.access == boss_gates::Access::Any {
                    kill_leader(&mut run, g.leaders.len() - 1);
                    assert!(run.boss_gates.as_ref().unwrap().unlocked());
                    assert!(
                        run.boss_gates
                            .as_ref()
                            .unwrap()
                            .leaders
                            .iter()
                            .any(|l| !l.defeated)
                    );
                }
            }
        }
    }
    assert!(
        apex > 0 && apex < rich,
        "rich beds support an apex only on a quarter of seeded hive profiles"
    );
    assert!(
        optional_stronghold > 0,
        "not every stronghold boss belongs to the ALL gate pact"
    );
}

#[test]
fn boss_ecology_browser_guest_sends_only_controls_and_shows_text_as_data() {
    let page = include_str!("../../../cockpit/assets/dungeon/guest.html");
    // The playable page posts a name, held controls and the scroll's wishes
    // (a known one, one to learn, or a reforge); the host decides every
    // political and resource outcome.
    let posts: Vec<&str> = page
        .match_indices("method:'POST'")
        .map(|(at, _)| &page[page[..at].rfind("fetch(").unwrap()..at])
        .collect();
    assert_eq!(posts.len(), 4, "{posts:?}");
    for route in [
        "fetch('/hello'",
        "fetch('/shooter/input'",
        "fetch('/frames/ack'",
        "fetch(route",
    ] {
        assert!(posts.iter().any(|post| post.starts_with(route)), "{route}");
    }
    for route in ["route='/boon'", "route='/learn'", "route='/reforge'"] {
        assert!(page.contains(route), "{route}");
    }
    assert!(!page.contains("'/wish'"), "the realm's wishing stone is not the Delve's scroll");
    assert!(!page.contains("boss_gates"), "no client political claims");
    // All text from the host is data, never HTML.
    assert!(!page.contains("innerHTML"));
    assert!(page.contains("textContent"));
    // Decoded frames are released and the waiting queue is bounded.
    assert!(page.contains("bmp.close()"));
    assert!(page.contains("queue.length>6"));
}

#[test]
fn boss_ecology_optional_pit_death_with_wipe_is_finite_without_route_or_economic_credit() {
    let mut run = (0..128)
        .find_map(|seed| {
            let r = on_floor(2, seed, Mode::LongWayDown);
            r.boss_gates
                .as_ref()
                .unwrap()
                .optional_pit
                .is_some()
                .then_some(r)
        })
        .unwrap();
    let room = run.boss_gates.as_ref().unwrap().optional_pit.unwrap();
    run.enter(room, Some(2));
    run.calm_for_test();
    let before = run.boss_gates.clone().unwrap();
    let mut corpse = run
        .enemies
        .iter()
        .find(|e| e.kind == EnemyKind::PitTyrant)
        .unwrap()
        .clone();
    corpse.hp = 0;
    run.enemies
        .iter_mut()
        .find(|e| e.kind == EnemyKind::PitTyrant)
        .unwrap()
        .hp = 0;
    run.players.get_mut(&1).unwrap().hp = 0;
    run.players.get_mut(&1).unwrap().winds = 0;
    run.step(&BTreeMap::new());
    assert_eq!(run.phase, Phase::Wiped);
    let g = run.boss_gates.as_ref().unwrap();
    assert!(g.pit_defeated);
    assert_eq!(g.leaders, before.leaders);
    assert_eq!(g.economy, before.economy);
    let score = run.score;
    let items = run.room().items.len();
    run.enemies.push(corpse);
    run.settle_kills();
    assert_eq!((run.score, run.room().items.len()), (score, items));
    run = reloaded(&run);
    run.players.get_mut(&1).unwrap().hp = 100;
    run.enter(room, Some(2));
    assert!(!run.enemies.iter().any(|e| e.kind == EnemyKind::PitTyrant));
    run.clear_for_test();
    assert!(run.room().cleared);
    assert!(run.valid_snapshot());
}

#[test]
fn boss_ecology_site_support_is_measured_from_native_terrain_and_depth_not_labels() {
    for depth in 1..=DEEPEST {
        let run = on_floor(depth, 19, Mode::LongWayDown);
        let profile = &run.boss_gates.as_ref().unwrap().economy.profile;
        for site in &profile.sites {
            let room = &run.dungeon.rooms[site.room];
            let (mut wet, mut works, mut open) = (0, 0, 0);
            for y in 1..room.rows - 1 {
                for x in 1..room.cols - 1 {
                    match room.tile(x as i32, y as i32) {
                        Tile::Hazard => wet += 1,
                        Tile::Block => works += 1,
                        Tile::Floor => open += 1,
                        _ => {}
                    }
                }
            }
            assert_eq!((site.wet, site.works, site.open), (wet, works, open));
            let units = match profile.resource {
                boss_ecology::Resource::Brood | boss_ecology::Resource::EchoBrood => {
                    wet / 8 + open / 180
                }
                boss_ecology::Resource::Ore | boss_ecology::Resource::Relics => {
                    works / 8 + open / 280
                }
                boss_ecology::Resource::Fuel => wet / 8 + works / 16,
                boss_ecology::Resource::Salvage => works / 12 + wet / 12 + open / 280,
            };
            let variation = (mix(profile.salt ^ site.room as u64) % 2) as u16;
            assert_eq!(
                u16::from(site.capacity),
                (units + variation + depth as u16 / 3).min(6)
            );
        }
    }
}

#[test]
fn boss_ecology_factionless_pit_empty_clear_and_empty_sealed_victory_are_rejected() {
    let mut run = (0..128)
        .find_map(|seed| {
            let r = on_floor(2, seed, Mode::LongWayDown);
            r.boss_gates
                .as_ref()
                .unwrap()
                .optional_pit
                .is_some()
                .then_some(r)
        })
        .unwrap();
    let room = run.boss_gates.as_ref().unwrap().optional_pit.unwrap();
    run.enter(room, Some(2));
    assert!(run.valid_snapshot());
    let mut bad = run.clone();
    bad.enemies.clear();
    assert!(
        !bad.valid_snapshot(),
        "missing native Pit Tyrant has no death evidence"
    );
    let guardian = run.boss_gates.as_ref().unwrap().guardian;
    run.enter(guardian, Some(2));
    assert!(run.enemies.is_empty());
    let mut guest = reloaded(&run);
    let mut bad = run.live();
    bad.phase = Phase::Won;
    assert!(!guest.apply_live(bad));
    assert_eq!(guest.phase, Phase::Exploring);
    let mut bad = run.clone();
    let unvisited = bad
        .boss_gates
        .as_ref()
        .unwrap()
        .economy
        .camps
        .iter()
        .find(|c| !c.deployed && c.room != guardian)
        .unwrap()
        .room;
    bad.dungeon.rooms[unvisited].cleared = true;
    assert!(
        !bad.valid_snapshot(),
        "a nonactive camp's clear must spend its finite deployment"
    );
}

#[test]
fn boss_ecology_native_dragon_light_home_still_admits_won_state() {
    let mut run = on_floor(FLOORS, 31, Mode::LongWayDown);
    let count = run.boss_gates.as_ref().unwrap().leaders.len();
    for which in 0..count {
        kill_leader(&mut run, which);
    }
    let guardian = run.boss_gates.as_ref().unwrap().guardian;
    run.enter(guardian, Some(2));
    let mut guest = reloaded(&run); // full init before Dragon death, no light
    assert!(guest.light.is_none());
    finish_room(&mut run);
    assert_eq!(run.phase, Phase::Exploring);
    assert!(run.light.is_some());
    let clear: mirror::Live =
        serde_json::from_slice(&serde_json::to_vec(&run.live()).unwrap()).unwrap();
    assert!(guest.apply_live(clear));
    assert_eq!(guest.light, run.light);
    let light = run.light.unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = light;
    run.step(&BTreeMap::new()); // real travel consumes the exit into Won
    assert_eq!(run.phase, Phase::Won);
    assert!(run.valid_snapshot());
    assert!(
        guest
            .apply_live(serde_json::from_slice(&serde_json::to_vec(&run.live()).unwrap()).unwrap())
    );
    assert_eq!(guest.phase, Phase::Won);
    assert_eq!(guest.light, run.light);
    assert!(guest.valid_snapshot());
    reloaded(&run);
    let mut invalid = run.live();
    invalid.light = Some((f32::NAN, 0.0));
    let before = serde_json::to_vec(&guest).unwrap();
    assert!(!guest.apply_live(invalid));
    assert_eq!(serde_json::to_vec(&guest).unwrap(), before);
    let mut missing = run.clone();
    missing.light = None;
    assert!(
        !missing.valid_snapshot(),
        "native light-home victory needs exit evidence"
    );
}

#[test]
fn boss_ecology_unadmitted_leader_loss_cannot_forge_political_aftermath() {
    let run = on_floor(2, 42, Mode::LongWayDown);
    let mut bad = run.clone();
    let room = bad.boss_gates.as_ref().unwrap().leaders[0].room;
    bad.dungeon.rooms[room].visited = true;
    let g = bad.boss_gates.as_mut().unwrap();
    g.leaders[0].defeated = true;
    g.economy.lose(0);
    assert!(
        !bad.valid_snapshot(),
        "causal leader loss cannot precede that camp's admitted deployment"
    );
    let mut guest = reloaded(&run);
    assert!(!guest.apply_live(bad.live()));
    assert_eq!(guest.boss_gates, run.boss_gates);
    let mut fixture = run;
    fixture.clear_prerequisites_for_test();
    assert!(
        fixture.valid_snapshot(),
        "test completion helpers spend the same finite journal"
    );
}

#[test]
fn boss_ecology_failed_catalog_and_malformed_geometry_admission_is_transactional() {
    let run = on_floor(2, 33, Mode::LongWayDown);
    let mut catalog = vec![bosses::builtin()[0].clone(); 63];
    let before = catalog.clone();
    assert!(
        boss_population::populate(&run.dungeon, &mut catalog, run.seed, run.raid_id, run.mode)
            .is_none()
    );
    assert_eq!(
        catalog, before,
        "failed catalog shortfall cannot leave a partial native addition"
    );
    let mut floor = run.dungeon.clone();
    floor.rooms[0].cols = usize::MAX;
    assert!(
        boss_population::populate(&floor, &mut catalog, run.seed, run.raid_id, run.mode).is_none()
    );
    assert_eq!(catalog, before);
    let mut floor = run.dungeon.clone();
    floor.rooms[1].cell = floor.rooms[0].cell;
    assert!(
        boss_population::populate(&floor, &mut catalog, run.seed, run.raid_id, run.mode).is_none()
    );
    assert_eq!(catalog, before);
}

#[test]
fn boss_ecology_encounter_catalog_ids_are_scoped_to_their_pack() {
    let custom_boss = |id: &str, name: &str, pack: &str| {
        bosses::check(
            id,
            &format!(
                "name {name}\nwhere {pack}\nhp 500\nattack aimed shots=1 speed=5 every=80 damage=10\nart\n77\nHH\n"
            ),
        )
        .unwrap()
        .0
    };

    let crypt_shoggoth = custom_boss("shoggoth-heart", "Crypt Heart", "crypt");
    let crypt_chief = custom_boss("iron-tusk-chief", "Crypt Tusk", "crypt");
    let cavern_red_pick = custom_boss("red-pick-chief", "Custom Red Pick", "mines");
    let mut catalog = vec![crypt_shoggoth, crypt_chief, cavern_red_pick];

    let choices = encounter_catalog::leaders(&mut catalog, Pack::Cavern, 1).unwrap();
    let iron_tusk = choices
        .iter()
        .copied()
        .find(|&i| catalog[i].id == "iron-tusk-chief")
        .expect("the cross-pack id cannot suppress the native Cavern chief");
    assert_eq!(catalog[iron_tusk].only_in, Pack::Cavern);
    let red_pick = choices
        .iter()
        .copied()
        .find(|&i| catalog[i].id == "red-pick-chief")
        .expect("the same-pack custom chief remains a Cavern choice");
    assert_eq!(catalog[red_pick].name, "Custom Red Pick");

    let heart = encounter_catalog::apex(
        &mut catalog,
        Pack::Unknown,
        RoomKind::Threshold,
        &[],
    )
    .unwrap();
    assert_eq!(catalog[heart].id, "shoggoth-heart");
    assert_eq!(catalog[heart].only_in, Pack::Unknown);
}

#[test]
fn boss_ecology_removing_living_native_pit_is_not_a_clear_or_reward() {
    let mut run = (0..128)
        .find_map(|seed| {
            let r = on_floor(2, seed, Mode::LongWayDown);
            r.boss_gates
                .as_ref()
                .unwrap()
                .optional_pit
                .is_some()
                .then_some(r)
        })
        .unwrap();
    let room = run.boss_gates.as_ref().unwrap().optional_pit.unwrap();
    run.enter(room, Some(2));
    let score = run.score;
    run.enemies.clear();
    run.step(&BTreeMap::new());
    assert!(!run.room().cleared);
    assert!(!run.boss_gates.as_ref().unwrap().pit_defeated);
    assert_eq!(run.score, score);
    assert!(run.room().items.is_empty());
}

fn floor_spot(run: &Run) -> (f32, f32) {
    let room = run.room();
    for row in 3..room.rows - 3 {
        for col in 3..room.cols - 3 {
            if (-1..=1).all(|dx| {
                (-1..=1).all(|dy| room.tile(col as i32 + dx, row as i32 + dy) == Tile::Floor)
            }) {
                return (
                    (col as f32 + 0.5) * TILE_UNITS,
                    (row as f32 + 0.5) * TILE_UNITS,
                );
            }
        }
    }
    panic!("fixture has no open production emission spot");
}

fn wire_live(run: &Run) -> mirror::Live {
    serde_json::from_slice(&serde_json::to_vec(&run.live()).unwrap()).unwrap()
}

#[test]
fn boss_ecology_actual_raising_brood_kills_wipes_and_reentry_exhaust_shared_provision() {
    // Checked leaders don't have a summon attack in the .boss vocabulary.
    // Native raisers/brood use act -> Deeds -> apply_deeds -> spawn_staged;
    // repeated producer admission is itself paid, not an unbounded loophole.
    for (kind, add, beat) in [
        (EnemyKind::Necromancer, EnemyKind::Skeleton, 60),
        (EnemyKind::Silkmother, EnemyKind::Spiderling, 90),
    ] {
        let mut host = on_floor(2, 55, Mode::LongWayDown);
        let leader = host.boss_gates.as_ref().unwrap().leaders[0].clone();
        host.enter(leader.room, Some(2));
        let allowance = host.boss_gates.as_ref().unwrap().economy.spawn_allowance;
        let mut guest = reloaded(&host);
        let initial_score = host.score;
        let mut rewarded = 0;
        let mut seen_adds = 0;
        let mut replay = Vec::new();
        for cycle in 0..usize::from(allowance) + 3 {
            let spot = floor_spot(&host);
            let producer = host.spawn_at(kind, spot.0, spot.1);
            // This is real combat stepping, not a synthetic journal emission.
            if let Some(id) = producer {
                let e = host.enemies.iter_mut().find(|e| e.id == id).unwrap();
                e.age = TELEGRAPH;
                let period = if kind == EnemyKind::Necromancer {
                    150
                } else {
                    180
                };
                e.timer = (beat + period - 1 - (id * 7) % period) % period;
            }
            host.calm_for_test();
            host.players.get_mut(&1).unwrap().invulnerable = 1000;
            host.step(&BTreeMap::new());
            assert!(guest.apply_live(wire_live(&host)));
            guest = reloaded(&guest); // serialized view preserves observed spawn IDs too
            host = reloaded(&host); // active producer AND adds are admitted on load
            let adds: Vec<_> = host
                .enemies
                .iter()
                .filter(|e| {
                    e.kind == add
                        && e.camp_slot.is_none()
                        && host
                            .boss_gates
                            .as_ref()
                            .unwrap()
                            .economy
                            .emission(e.id)
                            .is_some()
                })
                .cloned()
                .collect();
            seen_adds += adds.len();
            let score = host.score;
            for corpse in &adds {
                host.enemies
                    .iter_mut()
                    .find(|e| e.id == corpse.id)
                    .unwrap()
                    .hp = 0;
                let mut corpse = corpse.clone();
                corpse.hp = 0;
                replay.push(corpse);
            }
            host.step(&BTreeMap::new()); // normal settlement/rewards, producer remains alive
            rewarded += host.score - score;
            assert!(guest.apply_live(wire_live(&host)));
            assert!(
                host.enemies
                    .iter()
                    .any(|e| e.boss == Some(leader.boss) && e.hp > 0)
            );
            assert!(!host.boss_gates.as_ref().unwrap().leaders[0].defeated);
            // Replayed corpses must neither reward, split nor debit again.
            let before = (host.score, host.room().items.len(), host.boss_gates.clone());
            host.enemies.extend(replay.iter().cloned());
            host.settle_kills();
            assert_eq!(
                (host.score, host.room().items.len(), host.boss_gates.clone()),
                before
            );
            if cycle % 2 == 0 {
                host.players.get_mut(&1).unwrap().hp = 0;
                host.step(&BTreeMap::new());
                assert_eq!(host.phase, Phase::Wiped);
                assert!(guest.apply_live(wire_live(&host)));
                let hero = host.players.get_mut(&1).unwrap();
                hero.hp = hero.max_hp;
            }
            host.enter(leader.room, Some(2)); // surviving checked leader is recreated
            assert!(guest.apply_live(wire_live(&host)));
        }
        let economy = &host.boss_gates.as_ref().unwrap().economy;
        assert_eq!(economy.spawn_spent, allowance);
        assert!(
            seen_adds > 0,
            "production AI must actually raise/hatch adds"
        );
        assert!(seen_adds <= usize::from(allowance));
        assert_eq!(host.score, initial_score + rewarded);
        let spot = floor_spot(&host);
        let exhausted = (host.score, host.boss_gates.clone());
        for _ in 0..20 {
            host.apply_deeds(foes::Deeds {
                spawns: vec![(add, spot.0, spot.1, 0)],
                ..Default::default()
            });
            host.settle_kills();
            host.enter(leader.room, Some(2));
        }
        assert_eq!((host.score, host.boss_gates.clone()), exhausted);
        // Provision is not an encounter requirement: the live leader is still
        // killable and the room/prerequisite completes with zero provision.
        finish_room(&mut host);
        assert!(host.boss_gates.as_ref().unwrap().leaders[0].defeated);
        assert!(host.room().cleared);
        reloaded(&host);
    }
}

#[test]
fn boss_ecology_descendant_identity_replay_save_and_mirror_are_finite() {
    let mut host = (0..64)
        .find_map(|seed| {
            let mut r = on_floor(5, seed, Mode::LongWayDown);
            let room = r
                .boss_gates
                .as_ref()?
                .economy
                .camps
                .iter()
                .find(|c| c.budget > 3 && r.dungeon.pack == Pack::Fungal)?
                .room;
            r.enter(room, Some(2));
            r.enemies
                .iter()
                .any(|e| e.kind == EnemyKind::Slime && e.camp_slot.is_some())
                .then_some(r)
        })
        .unwrap();
    let mut guest = reloaded(&host);
    let _baseline = host.boss_gates.as_ref().unwrap().economy.spawn_spent;
    let root = host
        .enemies
        .iter()
        .find(|e| e.kind == EnemyKind::Slime)
        .unwrap()
        .clone();
    // Ensure the split spots are valid while retaining the root's camp identity.
    let spot = floor_spot(&host);
    let e = host.enemies.iter_mut().find(|e| e.id == root.id).unwrap();
    (e.x, e.y, e.hp) = (spot.0, spot.1, 0);
    let mut root = e.clone();
    root.hp = 0;
    host.settle_kills();
    assert_eq!(host.boss_gates.as_ref().unwrap().economy.spawn_spent, 2);
    assert!(guest.apply_live(wire_live(&host)));
    host = reloaded(&host); // living halves have journal identities, no camp slots
    let mut child = host
        .enemies
        .iter()
        .find(|e| e.kind == EnemyKind::Slime)
        .unwrap()
        .clone();
    child.hp = 0;
    host.enemies
        .iter_mut()
        .find(|e| e.id == child.id)
        .unwrap()
        .hp = 0;
    host.settle_kills();
    assert_eq!(host.boss_gates.as_ref().unwrap().economy.spawn_spent, 4);
    assert!(guest.apply_live(wire_live(&host)));
    let before = (host.score, host.boss_gates.clone(), host.enemies.len());
    host.enemies.extend([root, child]);
    host.settle_kills();
    assert_eq!(
        (host.score, host.boss_gates.clone(), host.enemies.len()),
        before
    );
    assert!(guest.apply_live(wire_live(&host)));
    host = reloaded(&host);
    let at = host.at;
    host.enter(at, Some(2)); // discard living descendants, no refund/recreation
    assert!(guest.apply_live(wire_live(&host)));
    assert!(!host.enemies.iter().any(|e| e.kind == EnemyKind::Slime));
    assert_eq!(host.boss_gates.as_ref().unwrap().economy.spawn_spent, 4);
    reloaded(&host);
}

#[test]
fn boss_ecology_failed_emissions_and_exhausted_native_pit_do_not_softlock() {
    let mut run = (0..64)
        .find_map(|seed| {
            let r = on_floor(2, seed, Mode::LongWayDown);
            r.boss_gates.as_ref()?.optional_pit.map(|_| r.clone())
        })
        .unwrap();
    let pit = run.boss_gates.as_ref().unwrap().optional_pit.unwrap();
    run.enter(pit, Some(2));
    let spot = floor_spot(&run);
    let original = run.enemies.clone();
    let before = run.boss_gates.clone();
    assert!(run.spawn_at(EnemyKind::Bat, f32::NAN, spot.1).is_none());
    assert!(run.spawn_at(EnemyKind::Skeleton, -10.0, -10.0).is_none());
    assert!(
        run.spawn_staged(EnemyKind::Slime, spot.0, spot.1, 3)
            .is_none()
    );
    run.enemies
        .resize(boss_ecology::ENTITY_CAP, original[0].clone());
    assert!(run.spawn_at(EnemyKind::Bat, spot.0, spot.1).is_none());
    assert_eq!(
        run.boss_gates, before,
        "failed bodies never debit or journal"
    );
    run.enemies = original;
    // A paid slime root can outlive the rest of the allowance. Exhaustion
    // must suppress its descendants without preventing Pit completion.
    let slime = run.spawn_at(EnemyKind::Slime, spot.0, spot.1).unwrap();
    let economy = &run.boss_gates.as_ref().unwrap().economy;
    let remaining = economy.spawn_allowance - economy.spawn_spent;
    for _ in 0..remaining {
        let id = run.spawn_at(EnemyKind::Bat, spot.0, spot.1).unwrap();
        run.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 0;
        run.settle_kills();
    }
    let spent = run.boss_gates.as_ref().unwrap().economy.spawn_spent;
    run.enemies.iter_mut().find(|e| e.id == slime).unwrap().hp = 0;
    run.settle_kills();
    assert_eq!(run.enemies.len(), 1, "exhausted slime has no descendants");
    assert_eq!(run.boss_gates.as_ref().unwrap().economy.spawn_spent, spent);
    let score = run.score;
    run.enter(pit, Some(2));
    assert!(run.enemies.iter().any(|e| e.kind == EnemyKind::PitTyrant));
    run.apply_deeds(foes::Deeds {
        spawns: vec![(EnemyKind::Skeleton, spot.0, spot.1, 0)],
        ..Default::default()
    });
    assert_eq!(run.enemies.len(), 1);
    assert_eq!(run.score, score);
    finish_room(&mut run);
    assert!(run.room().cleared);
    assert!(run.boss_gates.as_ref().unwrap().pit_defeated);
    reloaded(&run);
    let treasure = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Treasure)
        .unwrap();
    run.enter(treasure, Some(2));
    let chest = run.room().chest.expect("production treasure chest");
    let before = run.boss_gates.clone();
    for _ in 0..100 {
        assert!(!run.wake_mimic());
        assert_eq!(
            serde_json::to_value(run.room().chest).unwrap(),
            serde_json::to_value(chest).unwrap()
        );
    }
    assert_eq!(
        run.boss_gates, before,
        "failed mimic wakes leave provision/chest unchanged"
    );
    reloaded(&run);
}

#[test]
fn boss_ecology_oversized_mutable_rosters_and_journals_reject_before_replay_atomically() {
    let host = on_floor(2, 42, Mode::LongWayDown);
    for which in 0..7 {
        let mut bad = host.clone();
        let g = bad.boss_gates.as_mut().unwrap();
        match which {
            0 => g.economy.camps[0].roster = vec![EnemyKind::Bat; 25],
            1 => g.economy.camps[0].roster = vec![EnemyKind::Bat; 100_000],
            2 => {
                g.economy.events = vec![boss_ecology::Event::Retire(1); boss_ecology::EVENT_CAP + 1]
            }
            3 => g.economy.camps.resize(65, g.economy.camps[0].clone()),
            4 => g
                .economy
                .profile
                .sites
                .resize(65, g.economy.profile.sites[0].clone()),
            5 => g.economy.factions.resize(5, g.economy.factions[0].clone()),
            _ => g.economy.spawn_spent = boss_ecology::SPAWN_CAP + 1,
        }
        assert!(!bad.valid_snapshot());
        let mut view = reloaded(&host);
        let before = serde_json::to_vec(&view).unwrap();
        assert!(!view.apply_live(bad.live()));
        assert_eq!(serde_json::to_vec(&view).unwrap(), before);
        // Already malformed stored economy is bounded/rejected too, not cloned
        // and used as an indexing oracle by an otherwise legitimate delta.
        let before = serde_json::to_vec(&bad).unwrap();
        assert!(!bad.apply_live(host.live()));
        assert_eq!(serde_json::to_vec(&bad).unwrap(), before);
    }
    let mut view = reloaded(&host);
    let mut bad = host.live();
    bad.enemies.resize(
        129,
        host.enemies.first().cloned().unwrap_or_else(|| {
            let mut r = host.clone();
            let room = r.boss_gates.as_ref().unwrap().leaders[0].room;
            r.enter(room, Some(2));
            r.enemies[0].clone()
        }),
    );
    let before = serde_json::to_vec(&view).unwrap();
    assert!(!view.apply_live(bad));
    assert_eq!(serde_json::to_vec(&view).unwrap(), before);
}

#[test]
fn boss_ecology_secret_preflight_rejects_badthenfound_and_poisoned_snapshots_atomically() {
    for gated in [false, true] {
        let mut host = (0..64)
            .find_map(|seed| {
                let r = on_floor(2, seed, Mode::LongWayDown);
                r.dungeon.secret.is_some().then_some(r)
            })
            .expect("a generated floor declares a secret");
        if !gated {
            host.boss_gates = None;
        }
        let secret = host.dungeon.secret.expect("fixture declares a secret");
        host.enter(secret.host, Some(2));
        let mut view = reloaded(&host);
        let before = serde_json::to_vec(&view).unwrap();
        let mut mutations = Vec::new();
        let mut s = secret;
        s.host = usize::MAX;
        mutations.push(s);
        let mut s = secret;
        s.side = 4;
        mutations.push(s);
        let mut s = secret;
        s.vault = usize::MAX;
        mutations.push(s);
        let mut s = secret;
        s.vault = s.host;
        mutations.push(s);
        let mut s = secret;
        s.host = 0;
        mutations.push(s); // not a fight host
        let mut s = secret;
        s.snibbet = !s.snibbet;
        mutations.push(s);
        let mut s = secret;
        s.paid = true;
        mutations.push(s); // not found
        for s in mutations {
            // The first, undiscovered bad packet cannot poison metadata. Its
            // later found packet must also reject before calling open_secret.
            for found in [false, true] {
                let mut bad = host.live();
                bad.secret = Some(layout::Secret { found, ..s });
                bad.tick += 100;
                bad.score += 100;
                assert!(!view.apply_live(bad));
                assert_eq!(serde_json::to_vec(&view).unwrap(), before);
            }
        }
        for which in 0..6 {
            let mut stored = host.clone();
            match which {
                0 => stored.dungeon.secret.as_mut().unwrap().host = usize::MAX,
                1 => stored.dungeon.secret.as_mut().unwrap().side = 4,
                2 => stored.dungeon.secret.as_mut().unwrap().vault = secret.host,
                3 => stored.dungeon.rooms[secret.vault].cell = (100, 100),
                4 => stored.dungeon.rooms[secret.host].doors[secret.side] = true,
                _ => stored.dungeon.secret.as_mut().unwrap().snibbet = !secret.snibbet,
            }
            assert!(!stored.valid_snapshot());
            let before = serde_json::to_vec(&stored).unwrap();
            assert!(!stored.dungeon.open_secret());
            let mut found = host.live();
            found.secret.as_mut().unwrap().found = true;
            assert!(!stored.apply_live(found));
            assert_eq!(serde_json::to_vec(&stored).unwrap(), before);
        }
        host.blast_wall(None); // genuine production discovery
        assert!(host.dungeon.secret.unwrap().found);
        assert!(host.valid_snapshot());
        assert!(view.apply_live(wire_live(&host)));
        assert!(view.dungeon.secret.unwrap().found);
        assert_eq!(
            view.dungeon.rooms[secret.host].doors,
            host.dungeon.rooms[secret.host].doors
        );
        assert_eq!(
            view.dungeon.neighbour(secret.host, secret.side),
            Some(secret.vault)
        );
        // Production traversal through the newly opened doorway is projected.
        finish_room(&mut host);
        let (x, y) = hazards::doorway(host.room(), secret.side).0;
        let hero = host.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = match secret.side {
            0 => (x, 0.1),
            1 => (host.dungeon.rooms[secret.host].width() - 0.1, y),
            2 => (x, host.dungeon.rooms[secret.host].height() - 0.1),
            _ => (0.1, y),
        };
        host.step(&BTreeMap::new());
        assert_eq!(host.at, secret.vault);
        assert!(view.apply_live(wire_live(&host)));
        assert_eq!(view.at, secret.vault);
        reloaded(&host);
    }
}

#[test]
fn boss_ecology_legacy_ungated_emissions_remain_uncapped_by_provision() {
    let mut run = on_floor(2, 55, Mode::LongWayDown);
    run.boss_gates = None;
    for _ in 0..200 {
        assert!(run.spawn_at(EnemyKind::Skeleton, 10.0, 10.0).is_some());
    }
    assert_eq!(run.enemies.len(), 200);
    let mut value = serde_json::to_value(run.live()).unwrap();
    value.as_object_mut().unwrap().remove("light");
    let old: mirror::Live = serde_json::from_value(value).unwrap();
    assert!(
        old.light.is_none(),
        "older Live packets default their absent light"
    );
}
