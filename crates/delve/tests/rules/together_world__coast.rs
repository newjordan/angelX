// Coastal cases extend the existing world host/mirror/save regression, not a
// private replacement verifier. Throws below use real Input, never flight edits.
fn coastal_adventure_regression(mut run: Run) {
    use super::super::{
        Grid, HERO_RADIUS, Mover, coastal as coast, countryside as c,
        pastimes::{self, Game},
    };
    use crate::realm::{Spoil, Spoils};
    run.join_seat(3, "Crab Counsel");
    run.join_seat(4, "Dry Socks");
    run.home.countryside = c::Country::default();
    run.marks.clear();
    let fire = |id| {
        BTreeMap::from([(
            id,
            Input {
                fire: true,
                ..Default::default()
            },
        )])
    };
    // The keeper's errand answers the interact key; the pot answers F.
    let meet = |id| {
        BTreeMap::from([(
            id,
            Input {
                interact: true,
                ..Default::default()
            },
        )])
    };
    // Every coastal door is walked both ways, with four genuinely distinct seats.
    for &(a, b) in TRAILS
        .iter()
        .filter(|(a, b)| coast::PLACES.contains(a) || coast::PLACES.contains(b))
    {
        for (a, b) in [(a, b), (b, a)] {
            let ai = room_of(&run.dungeon, a).unwrap();
            let bi = room_of(&run.dungeon, b).unwrap();
            let ac = run.dungeon.rooms[ai].cell;
            let bc = run.dungeon.rooms[bi].cell;
            let side = DIRS
                .iter()
                .position(|&(x, y)| (ac.0 + x, ac.1 + y) == bc)
                .unwrap();
            run.enter(ai, Some((side + 2) % 4));
            let (x, y) = match side {
                0 => (run.room().width() / 2.0, 1.9),
                1 => (run.room().width() - 1.9, run.room().height() / 2.0),
                2 => (run.room().width() / 2.0, run.room().height() - 1.9),
                _ => (1.9, run.room().height() / 2.0),
            };
            let h = run.players.get_mut(&1).unwrap();
            (h.x, h.y) = (x, y);
            let (mx, my) = DIRS[side];
            let input = BTreeMap::from([(
                1,
                Input {
                    move_x: mx as i8,
                    move_y: my as i8,
                    ..Default::default()
                },
            )]);
            for _ in 0..20 {
                run.step(&input);
                if run.at == bi {
                    break;
                }
            }
            assert_eq!(run.at, bi, "coastal road {a:?}->{b:?}");
            let grid = Grid {
                room: run.room(),
                barred: false,
            };
            let mut coords = std::collections::BTreeSet::new();
            for h in run.players.values() {
                assert!(
                    grid.clear(h.x, h.y, HERO_RADIUS, Mover::Hero),
                    "arrival {b:?}"
                );
                assert!(coords.insert((h.x.to_bits(), h.y.to_bits())));
            }
            let friend = run.wire_copy();
            assert_eq!(friend.at, run.at);
        }
    }
    // Low-tide pickup is ordered, spatial, party-shared, and repeat safe.
    run.marks.clear();
    run.arrive(
        room_of(&run.dungeon, RoomKind::Strand).unwrap(),
        coast::ERRAND[1].1,
    );
    run.tick_coastal(&meet(2));
    assert_eq!(coast::stage(&run.home.countryside, &run.marks), 0);
    run.arrive(
        room_of(&run.dungeon, RoomKind::Lighthouse).unwrap(),
        coast::ERRAND[0].1,
    );
    run.tick_coastal(&meet(2));
    assert_eq!(coast::stage(&run.home.countryside, &run.marks), 1);
    run.arrive(
        room_of(&run.dungeon, RoomKind::Strand).unwrap(),
        coast::ERRAND[1].1,
    );
    run.tick = u64::from(9 * HZ);
    run.tick_coastal(&meet(2));
    assert_eq!(coast::stage(&run.home.countryside, &run.marks), 1);
    for h in run.players.values_mut() {
        h.stone = true;
    }
    run.tick = u64::from(12 * HZ);
    run.tick_coastal(&meet(2));
    assert_eq!(coast::stage(&run.home.countryside, &run.marks), 1);
    for h in run.players.values_mut() {
        h.stone = false;
    }
    run.tick_coastal(&meet(2));
    assert_eq!(coast::stage(&run.home.countryside, &run.marks), 2);
    run.arrive(
        room_of(&run.dungeon, RoomKind::Lighthouse).unwrap(),
        coast::ERRAND[2].1,
    );
    run.tick_coastal(&meet(2));
    assert_eq!(coast::stage(&run.home.countryside, &run.marks), 3);
    // Optional coast visits cannot reset or add requirements to old six places.
    let mut old: c::Country =
        serde_json::from_value(serde_json::json!({"visited":63,"ramble_paid":true,"built":4}))
            .unwrap();
    let mut purse = Spoils::default();
    let before = purse.clone();
    assert!(c::settle(&mut old, &mut purse, &run.marks).0);
    assert!(old.valid());
    assert_eq!(old.core_visits(), 6);
    assert!(old.ramble_paid && old.has(2));
    assert_eq!(purse.get(Spoil::Gold), before.get(Spoil::Gold) + 65);
    assert_eq!(purse.get(Spoil::Bone), 3);
    let paid = purse.clone();
    assert!(!c::settle(&mut old, &mut purse, &run.marks).0);
    assert_eq!(purse, paid);
    let p = &c::PLOTS[7];
    let mut marks = BTreeMap::from([("country:build:harbour".into(), 1)]);
    assert!(!c::settle(&mut old, &mut purse, &marks).0);
    assert!(!old.has(7));
    let price = c::cost(p);
    purse.add(Spoil::Gold, 90);
    purse.add(Spoil::Ore, 3);
    let full = purse.clone();
    assert!(c::settle(&mut old, &mut purse, &marks).0);
    assert!(old.has(7));
    for k in [Spoil::Gold, Spoil::Bone, Spoil::Ore] {
        assert_eq!(purse.get(k), full.get(k) - price.get(k));
    }
    let paid = purse.clone();
    assert!(!c::settle(&mut old, &mut purse, &marks).0);
    assert_eq!(purse, paid);
    run.rebuild_home(
        {
            let mut h = run.home.clone();
            h.countryside = old;
            h
        },
        purse,
    );
    for (id, dest) in [
        ("saltworks-barge", RoomKind::Riverbank),
        ("quay-barge", RoomKind::Saltworks),
    ] {
        let e = entrance(id);
        run.arrive(
            room_of(&run.dungeon, e.room).unwrap(),
            if e.room == RoomKind::Saltworks {
                (19.5, 8.0)
            } else {
                (4.0, 7.8)
            },
        );
        take(&mut run, e);
        assert_eq!(run.room().kind, dest);
        let grid = Grid {
            room: run.room(),
            barred: false,
        };
        for h in run.players.values() {
            assert!(grid.clear(h.x, h.y, HERO_RADIUS, Mover::Hero));
            assert_ne!(
                run.room().tile((h.x / 2.0) as i32, (h.y / 2.0) as i32),
                Tile::Stairs
            );
        }
    }
    marks.clear();
    // Defaulting one new record doesn't reset the existing arrays or sled.
    let records:pastimes::Records=serde_json::from_value(serde_json::json!({"best":[321,0,0,0],"plays":[1,0,0,0],"wins":[1,0,0,0],"sled":{"best":456,"plays":2,"wins":1}})).unwrap();
    assert!(records.valid());
    assert_eq!(records.best_for(Game::Pot), 0);
    assert_eq!(records.best_for(Game::Sled), 456);
    assert_eq!(records.best[0], 321);
    run.home.countryside.games = records;
    run.arrive(
        room_of(&run.dungeon, RoomKind::Strand).unwrap(),
        Game::Pot.start(),
    );
    // One owner on P; the host stands away. Other people's F cannot charge it.
    for (&id, h) in &mut run.players {
        (h.x, h.y) = if id == 2 {
            (8.0, 14.0)
        } else {
            (30.0 + id as f32, 18.0)
        };
    }
    run.step(&BTreeMap::new());
    assert_eq!(
        run.pastimes
            .round
            .as_ref()
            .unwrap()
            .put
            .as_ref()
            .unwrap()
            .owner,
        2
    );
    for _ in 0..8 {
        run.step(&fire(1));
    }
    assert_eq!(
        run.pastimes
            .round
            .as_ref()
            .unwrap()
            .put
            .as_ref()
            .unwrap()
            .charge,
        0
    );
    // Reviewer counterexample: accepted saved flights at both vertical bounds
    // must either remain valid or finish as a miss, never poison the live mirror.
    for dz in [-240, 240] {
        let mut edge = serde_json::to_value(&run).unwrap();
        let p = &mut edge["pastimes"]["round"]["put"];
        p["shots"] = 1.into();
        p["flight"] = serde_json::json!({"x":1024,"z":3000,"dx":1,"dz":dz,"left":90});
        let mut edge: Run = serde_json::from_value(edge).unwrap();
        assert!(edge.valid_snapshot());
        edge.step(&BTreeMap::new());
        assert!(
            edge.valid_snapshot(),
            "saved flight stays within live bounds"
        );
        let r = edge.pastimes.round.as_ref().unwrap();
        assert_eq!(r.mistakes, 1);
        assert!(r.put.as_ref().unwrap().flight.is_none());
        assert_eq!(edge.wire_copy().pastimes, edge.pastimes);
    }
    let score = run.score;
    // At 45 degrees each target is reachable with distinct powers and winds.
    for charge in [16, 20, 30] {
        for _ in 0..charge {
            run.step(&fire(2));
            assert!(run.valid_snapshot());
        }
        let mut restart: Run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
        assert!(restart.valid_snapshot());
        assert_eq!(restart.pastimes, run.pastimes);
        run.step(&BTreeMap::new());
        restart.step(&BTreeMap::new());
        assert!(
            run.pastimes
                .round
                .as_ref()
                .unwrap()
                .put
                .as_ref()
                .unwrap()
                .flight
                .is_some()
        );
        let friend = run.wire_copy();
        assert_eq!(friend.pastimes, run.pastimes);
        for _ in 0..100 {
            run.step(&BTreeMap::new());
            restart.step(&BTreeMap::new());
            assert_eq!(run.pastimes, restart.pastimes);
            assert!(run.valid_snapshot());
            if run.pastimes.round.is_none()
                || run
                    .pastimes
                    .round
                    .as_ref()
                    .unwrap()
                    .put
                    .as_ref()
                    .unwrap()
                    .flight
                    .is_none()
            {
                break;
            }
        }
        for _ in 0..HZ / 2 {
            run.step(&BTreeMap::new());
        }
    }
    assert!(run.pastimes.round.is_none());
    assert_eq!(run.home.countryside.games.wins_for(Game::Pot), 1);
    assert!(run.score > score);
    assert_eq!(run.home.countryside.games.best[0], 321);
    assert_eq!(run.home.countryside.games.best_for(Game::Sled), 456);
    let after = run.score;
    run.step(&fire(2));
    assert!(run.pastimes.round.is_none());
    assert_eq!(run.score, after, "step off P to retry");
    // A new attempt can fail through poor aim; owner loss and timeout clean up.
    run.players.get_mut(&2).unwrap().x = 12.0;
    run.step(&BTreeMap::new());
    run.players.get_mut(&2).unwrap().x = 8.0;
    run.step(&BTreeMap::new());
    for _ in 0..8 {
        run.step(&fire(2));
        run.step(&BTreeMap::new());
        for _ in 0..HZ * 4 {
            run.step(&BTreeMap::new());
            if run.pastimes.round.is_none() {
                break;
            }
        }
    }
    assert!(run.pastimes.round.is_none());
    assert_eq!(run.home.countryside.games.wins_for(Game::Pot), 1);
    run.players.get_mut(&2).unwrap().x = 12.0;
    run.step(&BTreeMap::new());
    run.players.get_mut(&2).unwrap().x = 8.0;
    run.step(&BTreeMap::new());
    run.players.get_mut(&2).unwrap().stone = true;
    run.step(&BTreeMap::new());
    assert!(run.pastimes.round.is_none());
    run.players.get_mut(&2).unwrap().stone = false;
    run.players.get_mut(&2).unwrap().x = 12.0;
    run.step(&BTreeMap::new());
    run.players.get_mut(&2).unwrap().x = 8.0;
    run.step(&BTreeMap::new());
    for _ in 0..Game::Pot.time() + 1 {
        run.step(&BTreeMap::new());
    }
    assert!(run.pastimes.round.is_none());
    assert!(run.valid_snapshot());
    run.arrive(room_of(&run.dungeon, RoomKind::Gate).unwrap(), (12.0, 9.5));
    assert!(run.pastimes.round.is_none());
}
