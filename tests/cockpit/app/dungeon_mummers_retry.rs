// Uses the pre-existing blocked-realm/private-checkpoint regression entry point.
fn mummers_save_retry(app: &mut App, fixture: &RealmSaveFixture) {
    use crate::drive::together_realm::Spoil;
    use crate::drive::together_shooter::{Input, RoomKind, Run, countryside as c, mummers as m,
        pastimes::{Game, Halfpipe, LIP_OPEN}, world};
    use std::collections::BTreeMap;
    app.realm().treasury.add(Spoil::Gold, 110);
    app.realm().treasury.add(Spoil::Bone, 3);
    let before = app.realm().treasury.clone();
    let home = app.realm().home.clone();
    app.dungeon.shooter.set(Some(Run::at_home(14, 7, None, home, before.clone())));
    let empty = BTreeMap::new();
    {
        let r = app.dungeon.shooter.edit().unwrap();
        for kind in m::PLACES {
            r.arrive(world::room_of(&r.dungeon, kind).unwrap(), (12.0, 6.5));
            r.step(&empty);
        }
        r.arrive(world::room_of(&r.dungeon, RoomKind::Mummers).unwrap(), Game::Halfpipe.start());
        r.step(&empty);
        for _ in 0..650 {
            let Some(round) = r.pastimes.round.as_ref() else { break; };
            let h = round.halfpipe.as_ref().unwrap();
            let direction = Halfpipe::direction(round.progress, round.mistakes);
            let axis = if h.flight == 0 || h.spin < 352 { direction } else { 0 };
            let fire = h.flight == 0 && h.phase == LIP_OPEN;
            r.step(&BTreeMap::from([(1, Input {aim_x:axis, fire, ..Default::default()})]));
            assert!(r.valid_snapshot());
        }
        assert_eq!(r.home.countryside.games.halfpipe.wins, 1);
        let p = &c::PLOTS[m::PLOT];
        r.arrive(world::room_of(&r.dungeon, p.room).unwrap(), p.at);
        for _ in 0..crate::drive::together_shooter::home::BUY_HOLD {
            r.step(&BTreeMap::from([(1, Input {interact:true, ..Default::default()})]));
        }
        assert_eq!(r.marks.get("country:build:caravan"), Some(&1));
        r.arrive(world::room_of(&r.dungeon, RoomKind::Gate).unwrap(), (12.0,9.5));
        r.dungeon.rooms.retain(|room| !m::PLACES.contains(&room.kind));
    }
    std::fs::remove_dir_all(fixture.home.join(".angelX")).unwrap();
    std::fs::write(fixture.home.join(".angelX"), b"not a directory").unwrap();
    assert!(!app.settle_home());
    assert_eq!(app.realm().treasury, before);
    assert!(!app.realm().home.countryside.has(m::PLOT));
    let path = fixture.workspace.join(".angel/dungeon/session.json");
    app.dungeon.save_path = Some(path.clone());
    app.dungeon.checkpoint(std::time::Instant::now(), true).unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    for mark in m::VISIT_MARKS { assert_eq!(saved["country_intents"][mark],1); }
    assert_eq!(saved["country_intents"]["country:build:caravan"],1);
    assert!(saved["run"].get("marks").is_none());
    app.dungeon.shooter.set(None);
    app.dungeon.restore(&fixture.workspace).unwrap();
    let r = app.dungeon.shooter.as_ref().unwrap();
    assert!(r.valid_snapshot());
    for kind in m::PLACES { assert!(world::room_of(&r.dungeon,kind).is_some()); }
    assert!(!app.settle_home());
    assert_eq!(app.realm().treasury,before);
    fixture.allow_saves();
    assert!(app.settle_home());
    // The half-pipe medal's 80, the plot's 110, and the game's first-win purse.
    assert_eq!(app.realm().treasury.get(Spoil::Gold),before.get(Spoil::Gold)+80-110+c::FIRST_WIN_GOLD);
    assert_eq!(app.realm().treasury.get(Spoil::Bone),before.get(Spoil::Bone)-3);
    assert_eq!(app.realm().treasury.get(Spoil::Wax),before.get(Spoil::Wax));
    assert!(app.realm().home.countryside.has(m::PLOT));
    assert!(app.realm().home.countryside.halfpipe_paid);
    let paid = app.realm().treasury.clone();
    assert!(app.settle_home());
    assert_eq!(app.realm().treasury,paid);
    // Save the twelfth start-pad bit and a real airborne rotation.
    let r = app.dungeon.shooter.edit().unwrap();
    r.arrive(world::room_of(&r.dungeon,RoomKind::Mummers).unwrap(),Game::Halfpipe.start());
    r.step(&empty);
    for _ in 0..84 {
        let h=r.pastimes.round.as_ref().unwrap().halfpipe.as_ref().unwrap();
        let fire=h.flight==0 && h.phase==LIP_OPEN;
        r.step(&BTreeMap::from([(1,Input{aim_x:1,fire,..Default::default()})]));
    }
    assert!(r.pastimes.round.as_ref().unwrap().halfpipe.as_ref().unwrap().flight>0);
    assert!(r.valid_snapshot());
    let flight=r.pastimes.clone();
    let plays=r.home.countryside.games.halfpipe.plays;
    app.dungeon.checkpoint(std::time::Instant::now(),true).unwrap();
    app.dungeon.shooter.set(None);
    app.dungeon.restore(&fixture.workspace).unwrap();
    let r=app.dungeon.shooter.edit().unwrap();
    assert_eq!(r.pastimes,flight);
    assert_eq!(r.home.countryside.games.halfpipe.plays,plays);
    for _ in 0..4 {r.step(&empty);assert!(r.valid_snapshot());}
    assert_ne!(r.pastimes,flight);
    assert!(app.settle_home());
    assert_eq!(app.realm().treasury,paid);
    assert!(app.dungeon.shooter.as_ref().unwrap().marks.is_empty());
}
