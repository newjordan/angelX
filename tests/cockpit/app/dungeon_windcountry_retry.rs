// The existing failed-realm-save entry point also exercises the new private journal.
fn windcountry_save_retry(app: &mut App, fixture: &RealmSaveFixture) {
    use crate::drive::together_realm::{Spoil, Realm};
    use crate::drive::together_shooter::{Input, Run, RoomKind, HZ, world, countryside as c, windcountry as w, pastimes::{Game, Trencher}};
    use std::collections::BTreeMap;
    let empty = BTreeMap::new();
    let keys = |x, fire| BTreeMap::from([(1, Input { aim_x: x, fire, ..Default::default() })]);
    let meet = || BTreeMap::from([(1, Input { interact: true, ..Default::default() })]);
    let go = |r: &mut Run, k, at| r.arrive(world::room_of(&r.dungeon, k).unwrap(), at);
    app.realm().treasury.add(Spoil::Gold, 90); app.realm().treasury.add(Spoil::Bone, 3);
    app.realm().save().unwrap();
    let before = app.realm().treasury.clone(); let home = app.realm().home.clone();
    app.dungeon.shooter.set(Some(Run::at_home(101, 1, None, home, before.clone())));
    {
        let r = app.dungeon.shooter.edit().unwrap();
        // Each lever wants its own press: E comes up between them.
        let press = |r: &mut Run| { r.step(&empty); r.step(&meet()); };
        go(r, RoomKind::Kitewright, w::MAKER); press(r);
        go(r, RoomKind::GooseGreen, w::CLOTH); press(r);
        for i in w::ORDER { go(r, RoomKind::SignalHill, w::FLAGS[i]); press(r); }
        go(r, RoomKind::Kitewright, w::MAKER); press(r);
        assert_eq!(w::stage(&r.home.countryside, &r.marks), 6);
        let p = &c::PLOTS[w::PLOT]; go(r, p.room, p.at);
        for _ in 0..crate::drive::together_shooter::home::BUY_HOLD { r.step(&meet()); }
        assert_eq!(r.marks.get("country:build:kitepost"), Some(&1));
        go(r, RoomKind::GooseGreen, Game::Trencher.start()); r.step(&empty);
        r.step(&keys(-1, true)); for _ in 0..7 { r.step(&keys(-1, false)); }
        assert!(r.pastimes.round.as_ref().unwrap().trencher.as_ref().unwrap().flight.is_some());
        assert!(r.valid_snapshot());
    }
    // A real failed realm write leaves treasury and durable home untouched.
    std::fs::rename(fixture.home.join(".angelX"), fixture.home.join(".angelX-kite-backup")).unwrap();
    std::fs::write(fixture.home.join(".angelX"), b"not a directory").unwrap();
    assert!(!app.settle_home()); assert_eq!(app.realm().treasury, before);
    assert_eq!(app.realm().home.countryside.signal, 0);
    let path = fixture.workspace.join(".angel/dungeon/session.json");
    app.dungeon.save_path = Some(path.clone()); app.dungeon.checkpoint(std::time::Instant::now(), true).unwrap();
    let mut loaded = game(); fixture.bind(&mut loaded); loaded.dungeon.shooter.set(None);
    loaded.dungeon.restore(&fixture.workspace).unwrap();
    {
        let r = loaded.dungeon.shooter.as_ref().unwrap(); assert!(r.valid_snapshot());
        assert_eq!(w::stage(&r.home.countryside, &r.marks), 6);
        assert_eq!(r.marks.get("country:build:kitepost"), Some(&1));
        assert_eq!(r.pastimes, app.dungeon.shooter.as_ref().unwrap().pastimes);
    }
    // Whitelist bounds reject malformed new intents rather than admitting them.
    let encoded = std::fs::read(&path).unwrap(); let mut bad: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    bad["country_intents"][w::QUEST_MARKS[0]] = serde_json::json!(2);
    std::fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
    assert!(loaded.dungeon.restore(&fixture.workspace).is_err());
    std::fs::write(&path, encoded).unwrap(); loaded.dungeon.shooter.set(None);
    loaded.dungeon.restore(&fixture.workspace).unwrap();
    std::fs::remove_file(fixture.home.join(".angelX")).unwrap();
    std::fs::rename(fixture.home.join(".angelX-kite-backup"), fixture.home.join(".angelX")).unwrap();
    loaded.dungeon.realm = None;
    {
        let r = loaded.dungeon.shooter.edit().unwrap();
        for _ in 0..10 * HZ {
            let Some(round) = r.pastimes.round.as_ref() else { break; };
            let t = round.trencher.as_ref().unwrap(); let target = Trencher::target(round.progress);
            let x = if t.goose < target - 24 { 1 } else if t.goose > target + 24 { -1 } else { 0 };
            let fire = t.flight.is_none() && t.wait == 0; r.step(&keys(x, fire));
        }
        assert_eq!(r.home.countryside.games.trencher.wins, 1);
    }
    assert!(loaded.settle_home());
    let country = loaded.realm().home.countryside.clone();
    assert_eq!(country.signal, 6); assert!(country.signal_paid && country.has(w::PLOT));
    assert_eq!(country.games.trencher.wins, 1); assert!(country.valid());
    assert_eq!(loaded.realm().treasury.get(Spoil::Gold), before.get(Spoil::Gold) + 60 - 90 + c::FIRST_WIN_GOLD);
    assert_eq!(loaded.realm().treasury.get(Spoil::Bone), before.get(Spoil::Bone) - 3);
    assert_eq!(loaded.realm().treasury.get(Spoil::Wax), before.get(Spoil::Wax));
    let paid = loaded.realm().treasury.clone(); assert!(loaded.settle_home()); assert_eq!(loaded.realm().treasury, paid);
    let realm = Realm::beside(loaded.world.rewards_path()); assert_eq!(realm.home.countryside, country); assert_eq!(realm.treasury, paid);
    loaded.dungeon.checkpoint(std::time::Instant::now(), true).unwrap();
    let mut restarted = game(); fixture.bind(&mut restarted); restarted.dungeon.restore(&fixture.workspace).unwrap();
    assert!(restarted.settle_home()); assert_eq!(restarted.realm().treasury, paid);
    for (id, kind) in [("kite-out", RoomKind::Windmill), ("kite-back", RoomKind::Kitewright)] {
        let r = restarted.dungeon.shooter.edit().unwrap();
        let e = world::ENTRANCES.iter().find(|e|e.id==id).unwrap(); go(r, e.room, (12.0, 7.0)); r.step(&empty);
        let (x, y) = world::entrance_centre(e); let h = r.players.get_mut(&1).unwrap(); (h.x, h.y) = (x, y);
        for _ in 0..crate::drive::together_shooter::home::DESCEND_HOLD + 1 { r.step(&empty); }
        assert_eq!(r.room().kind, kind);
    }
}
