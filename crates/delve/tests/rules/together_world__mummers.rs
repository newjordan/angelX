// Called by the established host/mirror/surface regression, using real inputs.
fn mummers_adventure_regression(mut run: Run) {
    use super::super::{countryside as c, mummers as m, pastimes::{Game, Halfpipe, LIP_OPEN, LIP_CLOSE, FLIGHT_TICKS}};
    use crate::realm::Spoil;
    let empty = BTreeMap::new();
    let keys = |axis, fire| BTreeMap::from([(1, super::super::Input {
        aim_x: axis, fire, ..Default::default()
    })]);
    let go = |run: &mut Run, kind, at| {
        run.arrive(room_of(&run.dungeon, kind).unwrap(), at);
    };
    let mut country = c::Country::default();
    let mut purse = crate::realm::Spoils::default();
    purse.add(Spoil::Gold, 400);
    purse.add(Spoil::Bone, 20);
    purse.add(Spoil::Wax, 20);
    let request = BTreeMap::from([("country:build:caravan".into(), 1)]);
    let untouched = purse.clone();
    assert!(!c::settle(&mut country, &mut purse, &request).0);
    assert_eq!(purse, untouched, "no charge before medal and exploration");
    for kind in m::PLACES {
        go(&mut run, kind, (12.0, 6.5));
        run.tick_countryside(&empty);
    }
    let visits: BTreeMap<_, _> = run.marks.iter()
        .filter(|(key, _)| m::VISIT_MARKS.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), *value)).collect();
    assert_eq!(visits.len(), 3);
    assert!(c::settle(&mut country, &mut purse, &visits).0);
    assert_eq!(country.mummers_visited, 7);
    assert_eq!(purse, untouched, "walking does not mint the medal");
    assert!(!c::settle(&mut country, &mut purse, &request).0);
    run.home.countryside.mummers_visited = 7;
    let original_arrays = (run.home.countryside.games.best,
        run.home.countryside.games.wins, run.home.countryside.games.plays);
    go(&mut run, RoomKind::Mummers, Game::Halfpipe.start());
    run.tick_pastimes(&empty);
    assert_eq!(run.pastimes.round.as_ref().unwrap().game, Game::Halfpipe);
    // A malformed snapshot is rejected rather than accepted into the mirror.
    let mut bad = run.pastimes.clone();
    bad.round.as_mut().unwrap().halfpipe.as_mut().unwrap().spin = 1;
    assert!(!bad.valid());
    let mut restored_mid_air = false;
    for _ in 0..650 {
        let Some(r) = run.pastimes.round.as_ref() else { break; };
        let h = r.halfpipe.as_ref().unwrap();
        let direction = Halfpipe::direction(r.progress, r.mistakes);
        let axis = if h.flight == 0 || h.spin < 352 { direction } else { 0 };
        // Use the LAST visible gold frame to cover the off-by-one boundary.
        let fire = h.flight == 0 && h.phase == LIP_CLOSE - 1;
        run.tick_pastimes(&keys(axis, fire));
        assert!(run.pastimes.valid());
        if !restored_mid_air && run.pastimes.round.as_ref()
            .is_some_and(|r| r.halfpipe.as_ref().unwrap().flight == 12) {
            assert!(run.valid_snapshot());
            let encoded = serde_json::to_vec(&run).unwrap();
            let saved: Run = serde_json::from_slice(&encoded).unwrap();
            assert!(saved.valid_snapshot());
            assert_eq!(saved.pastimes, run.pastimes);
            run = saved;
            restored_mid_air = true;
        }
    }
    assert!(restored_mid_air);
    assert!(run.pastimes.round.is_none(), "input journey ends with four tricks");
    let score = run.home.countryside.games.halfpipe.clone();
    assert_eq!((score.plays, score.wins), (1, 1));
    assert!(score.best > 0);
    assert_eq!((run.home.countryside.games.best, run.home.countryside.games.wins,
        run.home.countryside.games.plays), original_arrays);
    assert!(country.games.merge(&run.home.countryside.games));
    assert!(!country.games.merge(&run.home.countryside.games));
    // No visit marks are needed if the player saved all visits before winning.
    assert!(c::settle(&mut country, &mut purse, &BTreeMap::new()).0);
    assert!(country.halfpipe_paid);
    assert_eq!(purse.get(Spoil::Gold), untouched.get(Spoil::Gold) + 80);
    assert_eq!(purse.get(Spoil::Wax), untouched.get(Spoil::Wax) + 2);
    let paid = purse.clone();
    assert!(!c::settle(&mut country, &mut purse, &BTreeMap::new()).0);
    assert_eq!(purse, paid);
    assert!(c::settle(&mut country, &mut purse, &request).0);
    assert_eq!(purse.get(Spoil::Gold), paid.get(Spoil::Gold) - 110);
    assert_eq!(purse.get(Spoil::Bone), paid.get(Spoil::Bone) - 3);
    assert_eq!(purse.get(Spoil::Wax), paid.get(Spoil::Wax) - 2);
    assert!(country.has(m::PLOT) && country.valid());
    let paid = purse.clone();
    assert!(!c::settle(&mut country, &mut purse, &request).0);
    assert_eq!(purse, paid);
    run.home.countryside = country;
    for (id, destination) in [("caravan-out", RoomKind::Fairground), ("caravan-back", RoomKind::Caravan)] {
        let e = entrance(id);
        go(&mut run, e.room, (12.0, 6.5));
        take(&mut run, e);
        assert_eq!(run.room().kind, destination);
        assert!(walkable(run.room(), run.players[&1].x, run.players[&1].y));
    }
    // Early F and holding F cannot auto-launch or finish a round.
    go(&mut run, RoomKind::Mummers, Game::Halfpipe.start());
    run.tick_pastimes(&empty);
    run.tick_pastimes(&keys(1, true));
    assert_eq!(run.pastimes.round.as_ref().unwrap().mistakes, 1);
    for _ in 0..200 { run.tick_pastimes(&keys(1, true)); }
    assert!(run.pastimes.round.is_none());
    assert_eq!(run.home.countryside.games.halfpipe.wins, 1);
    run.tick_pastimes(&empty);
    assert!(run.pastimes.round.is_none(), "must step off and return");
    go(&mut run, RoomKind::Mummers, (6.0, 7.0));
    run.tick_pastimes(&empty);
    go(&mut run, RoomKind::Mummers, Game::Halfpipe.start());
    run.tick_pastimes(&empty);
    assert!(run.pastimes.round.is_some());
    run.players.get_mut(&1).unwrap().stone = true;
    run.tick_pastimes(&empty);
    assert!(run.pastimes.round.is_none(), "unavailable owner retires the board");
    assert_eq!(run.home.countryside.games.halfpipe.wins, 1);
    assert!(LIP_OPEN < LIP_CLOSE && FLIGHT_TICKS > 23);
}
