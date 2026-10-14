// Called from the established host/mirror world test, not a separate verifier.
fn eastern_adventure_regression(mut run: Run) {
    use super::super::{
        countryside as c,
        pastimes::{self, Game},
    };
    use crate::realm::Spoil;
    let place = |r: &mut Run, id, at: (f32, f32)| {
        let h = r.players.get_mut(&id).unwrap();
        (h.x, h.y) = (at.0 * TILE_UNITS, at.1 * TILE_UNITS);
    };
    let fire = |id| {
        BTreeMap::from([(
            id,
            Input {
                fire: true,
                ..Input::default()
            },
        )])
    };
    // The quest folk answer the interact key, not the fire key.
    let meet = |id| {
        BTreeMap::from([(
            id,
            Input {
                interact: true,
                ..Input::default()
            },
        )])
    };
    let empty = BTreeMap::new();
    run.arrive(
        room_of(&run.dungeon, RoomKind::Causeway).unwrap(),
        Game::Hobbyhorse.start(),
    );
    place(&mut run, 2, (3.0, 9.5));
    run.tick_pastimes(&empty);
    assert_eq!(run.home.countryside.games.hobbyhorse.plays, 1);
    let fence = pastimes::HURDLES[0];
    place(&mut run, 1, (fence - 0.2, 7.0));
    run.tick_pastimes(&empty);
    place(&mut run, 1, (fence + 0.1, 7.0));
    run.tick_pastimes(&fire(2)); // friend's jump cannot lift the host
    // The host started it; the friend rides a heat of their own.
    assert_eq!(run.pastimes.round.as_ref().unwrap().progress, 0);
    assert_eq!(run.pastimes.round.as_ref().unwrap().mistakes, 1);
    for _ in 0..pastimes::JUMP_TICKS + 10 {
        run.tick_pastimes(&fire(2));
    }
    assert_eq!(
        run.pastimes.heats[&2].riders[&2].air_left,
        0,
        "holding F cannot auto-jump"
    );
    let saved: Run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
    assert!(saved.valid_snapshot());
    assert_eq!(saved.pastimes, run.pastimes);
    assert_eq!(run.wire_copy().pastimes, run.pastimes);
    let best_before = run.home.countryside.games.clone();
    for &x in &pastimes::HURDLES {
        place(&mut run, 1, (x - 0.2, 7.0));
        run.tick_pastimes(&empty);
        place(&mut run, 1, (x + 0.1, 7.0));
        run.tick_pastimes(&fire(1));
        if run.pastimes.heat.is_some() {
            for _ in 0..pastimes::JUMP_TICKS + 8 {
                run.tick_pastimes(&empty);
            }
        }
        assert!(run.pastimes.valid());
    }
    // First home wins: the host's clear course ends the friend's heat too.
    assert!(run.pastimes.round.is_none() && run.pastimes.heat.is_none());
    assert_eq!(run.home.countryside.games.hobbyhorse.wins, 1);
    assert!(run.home.countryside.games.hobbyhorse.best > 0);
    assert_eq!(run.home.countryside.games.best, best_before.best);
    assert_eq!(run.home.countryside.games.cheese, best_before.cheese);
    let score = run.score;
    run.tick_pastimes(&fire(1));
    assert_eq!(run.score, score);
    // The advertised rhythm must work at the real movement/tick speed, not
    // just through position injection: six jumps without stopping to wait.
    let mut riding = run.clone();
    riding.players.retain(|&id, _| id == 1);
    riding.pastimes = Default::default();
    place(&mut riding, 1, Game::Hobbyhorse.start());
    riding.step(&empty);
    let wins = riding.home.countryside.games.hobbyhorse.wins;
    for _ in 0..HZ * 8 {
        let Some(round) = riding.pastimes.round.as_ref() else {
            break;
        };
        let distance =
            pastimes::HURDLES[usize::from(round.progress)] * TILE_UNITS - riding.players[&1].x;
        riding.step(&BTreeMap::from([(
            1,
            Input {
                move_x: 1,
                fire: distance > 0.0 && distance < 0.8,
                ..Input::default()
            },
        )]));
    }
    assert!(
        riding.pastimes.round.is_none(),
        "continuous walking finishes the course"
    );
    assert_eq!(riding.home.countryside.games.hobbyhorse.wins, wins + 1);
    assert_eq!(
        riding
            .found
            .as_ref()
            .map(|(_, _, s)| s.contains("complete!")),
        Some(true)
    );
    place(&mut run, 1, Game::Hobbyhorse.start());
    run.tick_pastimes(&empty);
    run.pastimes.round.as_mut().unwrap().left = 1;
    run.tick_pastimes(&fire(1));
    assert!(run.pastimes.round.is_none());
    run.tick_pastimes(&empty);
    assert!(
        run.pastimes.round.is_none(),
        "parked rider cannot auto-replay"
    );
    run.arrive(
        room_of(&run.dungeon, RoomKind::Priory).unwrap(),
        c::PICNIC[1].1,
    );
    run.tick_countryside(&meet(1));
    assert_eq!(
        c::picnic_stage(&run.home.countryside, &run.marks),
        0,
        "wrong order"
    );
    let plot = &c::PLOTS[5];
    assert!(plot.blocked(&run.home.countryside).is_some());
    for (i, &(kind, point)) in c::PICNIC.iter().enumerate() {
        run.arrive(room_of(&run.dungeon, kind).unwrap(), point);
        if i == 0 {
            run.players.get_mut(&1).unwrap().stone = true;
            place(&mut run, 2, (12.0, 9.0));
            run.tick_countryside(&meet(1));
            assert_eq!(c::picnic_stage(&run.home.countryside, &run.marks), 0);
            run.players.get_mut(&1).unwrap().stone = false;
            let bad = BTreeMap::from([(
                1,
                Input {
                    interact: true,
                    move_x: 2,
                    ..Input::default()
                },
            )]);
            run.tick_countryside(&bad);
            assert_eq!(c::picnic_stage(&run.home.countryside, &run.marks), 0);
        }
        let who = 1 + i as u32 % 2;
        place(&mut run, who, point);
        run.tick_countryside(&meet(who));
        assert_eq!(
            c::picnic_stage(&run.home.countryside, &run.marks),
            i as u8 + 1
        );
    }
    run.home.countryside.visited = 255;
    run.home.countryside.ramble_paid = true;
    let mut treasury = crate::realm::Spoils::default();
    c::settle(&mut run.home.countryside, &mut treasury, &run.marks);
    assert_eq!(treasury.get(Spoil::Gold), 60);
    assert_eq!(treasury.get(Spoil::Wax), 3);
    assert!(run.home.countryside.picnic_paid && run.home.countryside.ramble_paid);
    let paid = treasury.clone();
    c::settle(&mut run.home.countryside, &mut treasury, &run.marks);
    assert_eq!(treasury, paid);
    assert!(plot.blocked(&run.home.countryside).is_none());
    assert!(!open(entrance("priory-post"), &run.home));
    for &(s, n) in plot.price {
        treasury.add(s, n);
    }
    run.marks.clear();
    run.marks.insert("country:build:post".into(), 1);
    c::settle(&mut run.home.countryside, &mut treasury, &run.marks);
    assert!(run.home.countryside.has(5));
    let built = treasury.clone();
    c::settle(&mut run.home.countryside, &mut treasury, &run.marks);
    assert_eq!(treasury, built);
    run.join_seat(3, "Third");
    run.join_seat(4, "Fourth");
    assert_eq!(run.players.len(), 4);
    for (start, id, target) in [
        (RoomKind::Priory, "priory-post", RoomKind::Lists),
        (RoomKind::Lists, "lists-post", RoomKind::Priory),
    ] {
        run.arrive(room_of(&run.dungeon, start).unwrap(), (12.0, 9.0));
        take(&mut run, entrance(id));
        assert_eq!(run.room().kind, target);
        for hero in run.players.values() {
            assert!(walkable(run.room(), hero.x, hero.y), "safe {id} arrival");
            assert!(
                entrance_at(
                    run.room().kind,
                    (hero.x / TILE_UNITS) as i32,
                    (hero.y / TILE_UNITS) as i32
                )
                .is_none()
            );
        }
        run.step(&empty);
        assert_eq!(run.room().kind, target);
    }
    let old = serde_json::from_str::<c::Country>(r#"{"visited":63,"built":1,"ramble_paid":true}"#)
        .unwrap();
    assert!(old.valid());
    assert_eq!(old.picnic, 0);
    assert_eq!(old.games.hobbyhorse.plays, 0);
    assert!(
        !serde_json::from_str::<c::Country>(r#"{"picnic":4}"#)
            .unwrap()
            .valid()
    );
    assert!(
        !serde_json::from_str::<c::Country>(r#"{"picnic_paid":true}"#)
            .unwrap()
            .valid()
    );
    assert!(
        !serde_json::from_str::<c::Country>(r#"{"built":32}"#)
            .unwrap()
            .valid()
    );
}
