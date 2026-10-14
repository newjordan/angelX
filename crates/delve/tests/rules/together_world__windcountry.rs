// Extends the established live-input world journey; not a separate verifier.
fn windcountry_adventure_regression(mut run: Run) {
    use super::super::{countryside as c, windcountry as w, pastimes::{Game, Trencher, TRENCHER_FLIGHT}, Input, HZ};
    use crate::realm::Spoil;
    use std::collections::BTreeMap;
    let empty = BTreeMap::new();
    let keys = |x, fire| BTreeMap::from([(1, Input { aim_x: x, fire, ..Default::default() })]);
    // The kitewright, the sail, the levers and the plot answer the interact key.
    let meet = || BTreeMap::from([(1, Input { interact: true, ..Default::default() })]);
    let go = |r: &mut Run, kind, at| r.arrive(room_of(&r.dungeon, kind).unwrap(), at);
    assert_eq!(Game::Trencher.index(), 13);
    assert_eq!(c::PLOTS[w::PLOT].id, "kitepost");
    // Ordered signals: holding one lever cannot satisfy separated later levers.
    // A lever answers a press: E comes up between them.
    let press = |r: &mut Run| {
        r.step(&empty);
        r.step(&meet());
    };
    go(&mut run, RoomKind::SignalHill, w::FLAGS[0]);
    press(&mut run);
    assert_eq!(w::stage(&run.home.countryside, &run.marks), 0);
    go(&mut run, RoomKind::Kitewright, w::MAKER);
    press(&mut run);
    assert_eq!(w::stage(&run.home.countryside, &run.marks), 1);
    go(&mut run, RoomKind::GooseGreen, w::CLOTH);
    press(&mut run);
    go(&mut run, RoomKind::SignalHill, w::FLAGS[1]);
    press(&mut run);
    assert_eq!(w::stage(&run.home.countryside, &run.marks), 2);
    for (i, flag) in w::ORDER.into_iter().enumerate() {
        go(&mut run, RoomKind::SignalHill, w::FLAGS[flag]);
        press(&mut run);
        // Still holding E at the lever just pulled: no "Wrong flag!".
        for _ in 0..HZ { run.step(&meet()); }
        assert!(!run.found.as_ref().unwrap().2.starts_with("Wrong flag"));
        assert_eq!(w::stage(&run.home.countryside, &run.marks), (i + 3) as u8);
    }
    go(&mut run, RoomKind::Kitewright, w::MAKER);
    press(&mut run);
    let mut treasury = crate::realm::Spoils::default();
    let mut country = c::Country::default();
    assert!(c::settle(&mut country, &mut treasury, &run.marks).0);
    assert_eq!(country.wind_visited, 7);
    assert_eq!(country.signal, 6);
    assert!(country.signal_paid);
    assert_eq!(treasury.get(Spoil::Gold), 60);
    assert_eq!(treasury.get(Spoil::Wax), 3);
    let paid = treasury.clone();
    assert!(!c::settle(&mut country, &mut treasury, &run.marks).0);
    assert_eq!(treasury, paid);
    run.home.countryside = country;
    run.marks.clear();
    // Unaffordable orders do not pay or build; the exact price charges once.
    let order = BTreeMap::from([("country:build:kitepost".into(), 1)]);
    c::settle(&mut run.home.countryside, &mut treasury, &order);
    assert!(!run.home.countryside.has(w::PLOT));
    treasury.add(Spoil::Gold, 30); treasury.add(Spoil::Bone, 3);
    c::settle(&mut run.home.countryside, &mut treasury, &order);
    assert!(run.home.countryside.has(w::PLOT));
    assert_eq!(treasury.get(Spoil::Gold), 0);
    assert_eq!(treasury.get(Spoil::Bone), 0);
    assert_eq!(treasury.get(Spoil::Wax), 0);
    let paid = treasury.clone();
    c::settle(&mut run.home.countryside, &mut treasury, &order);
    assert_eq!(treasury, paid);
    run.treasury = treasury;
    for (id, kind) in [("kite-out", RoomKind::Windmill), ("kite-back", RoomKind::Kitewright)] {
        let e = entrance(id); go(&mut run, e.room, (12.0, 7.0)); take(&mut run, e);
        assert_eq!(run.room().kind, kind);
    }
    // The new separate Score leaves the original fixed-size JSON records readable.
    let old: c::Country = serde_json::from_str(r#"{"games":{"best":[1,2,3,4],"wins":[0,0,0,0],"plays":[1,1,1,1]}}"#).unwrap();
    assert!(old.valid()); assert_eq!(old.games.trencher.plays, 0);
    assert_eq!(old.games.best, [1,2,3,4]);
    go(&mut run, RoomKind::GooseGreen, Game::Trencher.start()); run.step(&empty);
    assert_eq!(run.pastimes.round.as_ref().unwrap().game, Game::Trencher);
    assert_eq!(run.home.countryside.games.trencher.plays, 1);
    // Holding F across flights never auto-launches another plate.
    run.step(&keys(0, true));
    for _ in 0..TRENCHER_FLIGHT + HZ { run.step(&keys(0, true)); }
    assert_eq!(run.pastimes.round.as_ref().unwrap().mistakes, 1);
    assert!(run.pastimes.round.as_ref().unwrap().trencher.as_ref().unwrap().flight.is_none());
    run.step(&empty);
    let paid = run.treasury.clone();
    let mut snapshots = 0;
    for t in 0..10 * HZ {
        let Some(r) = run.pastimes.round.as_ref() else { break; };
        let g = r.trencher.as_ref().unwrap();
        let target = Trencher::target(r.progress);
        let x = if g.goose < target - 24 { 1 } else if g.goose > target + 24 { -1 } else { 0 };
        let fire = g.flight.is_none() && g.wait == 0;
        run.step(&keys(x, fire));
        assert!(run.pastimes.valid());
        if t % 20 == 7 && run.pastimes.round.is_some() {
            let bytes = serde_json::to_vec(&run).unwrap();
            let restored: Run = serde_json::from_slice(&bytes).unwrap();
            assert!(restored.valid_snapshot());
            let friend = run.wire_copy();
            assert_eq!(friend.pastimes, run.pastimes); snapshots += 1;
        }
    }
    assert!(snapshots > 0); assert!(run.pastimes.round.is_none());
    assert_eq!(run.home.countryside.games.trencher.wins, 1);
    assert_eq!(run.treasury, paid, "replay cannot mint treasury rewards");
    // Three misses, timeout, leaving the room and losing the owner all end safely.
    go(&mut run, RoomKind::GooseGreen, (12.0, 6.0)); run.step(&empty);
    go(&mut run, RoomKind::GooseGreen, Game::Trencher.start()); run.step(&empty);
    for _ in 0..3 * HZ * 3 {
        if run.pastimes.round.is_none() { break; }
        let g = run.pastimes.round.as_ref().unwrap().trencher.as_ref().unwrap();
        let fire = g.flight.is_none() && g.wait == 0;
        run.step(&keys(0, fire));
    }
    assert!(run.pastimes.round.is_none());
    assert_eq!(run.home.countryside.games.trencher.wins, 1);
    go(&mut run, RoomKind::GooseGreen, (12.0, 6.0)); run.step(&empty);
    go(&mut run, RoomKind::GooseGreen, Game::Trencher.start()); run.step(&empty);
    for _ in 0..Game::Trencher.time() { run.step(&empty); }
    assert!(run.pastimes.round.is_none());
    go(&mut run, RoomKind::GooseGreen, (12.0, 6.0)); run.step(&empty);
    go(&mut run, RoomKind::GooseGreen, Game::Trencher.start()); run.step(&empty);
    go(&mut run, RoomKind::Kitewright, w::MAKER); assert!(run.pastimes.round.is_none());
    go(&mut run, RoomKind::GooseGreen, Game::Trencher.start()); run.step(&empty);
    run.players.get_mut(&1).unwrap().stone = true; run.step(&empty);
    assert!(run.pastimes.round.is_none()); assert!(run.pastimes.valid());
}

#[test]
fn the_mill_kite_stands_clear_of_the_bakery_and_the_goat_lift_and_keeps_its_own_hold() {
    use super::super::{countryside as c, windcountry as w};
    let near = |a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)| {
        a.0 <= b.0 + b.2 && b.0 <= a.0 + a.2 && a.1 <= b.1 + b.3 && b.1 <= a.1 + a.3
    };
    let bakery = c::PLOTS.iter().find(|p| p.id == "bakery").unwrap();
    assert!(!near(w::MILL_KITE, bakery.footprint), "clear of the bakery");
    assert!(!near(w::MILL_KITE, c::LIFT), "not touching the goat lift");
    // Built lift and kite-post. Most of a hold on the lift, then a step onto
    // the kite: the kite's hold begins again.
    let mut run = at_the_gate();
    let lift = c::PLOTS.iter().position(|p| p.id == "lift").unwrap();
    run.home.countryside.built |= (1 << lift) | (1 << w::PLOT);
    run.home.countryside.survey = 7;
    run.home.countryside.signal = 6;
    run.arrive(room_of(&run.dungeon, RoomKind::Windmill).unwrap(), (12.0, 7.0));
    run.step(&BTreeMap::new());
    let (lx, ly) = entrance_centre(entrance("windmill-lift"));
    for hero in run.players.values_mut() {
        (hero.x, hero.y) = (lx, ly);
    }
    for _ in 0..DESCEND_HOLD - 1 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.room().kind, RoomKind::Windmill);
    let (kx, ky) = entrance_centre(entrance("kite-back"));
    for hero in run.players.values_mut() {
        (hero.x, hero.y) = (kx, ky);
    }
    run.step(&BTreeMap::new());
    assert_eq!(run.room().kind, RoomKind::Windmill, "no borrowed hold");
    for _ in 0..DESCEND_HOLD + 2 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.room().kind, RoomKind::Kitewright);
}
