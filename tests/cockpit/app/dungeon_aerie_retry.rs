// Extends the existing blocked-save/checkpoint/restart/idempotent-retry entry point.
fn aerie_save_retry(app: &mut App, fixture: &RealmSaveFixture) {
    use crate::delve::together_realm::Spoil;
    use crate::delve::together_shooter::{
        Input, RoomKind, Run, aerie as a, countryside as c, pastimes::Game, world,
    };
    use std::collections::BTreeMap;
    app.economy().treasury.add(Spoil::Gold, 100);
    app.economy().treasury.add(Spoil::Bone, 4);
    let before = app.economy().treasury.clone();
    let home = app.economy().home.clone();
    app.realm.dungeon.shooter.set(Some(Run::at_home(14, 7, None, home, before.clone())));
    let fire = BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]);
    {
        let r = app.realm.dungeon.shooter.edit().unwrap();
        r.arrive(
            world::room_of(&r.dungeon, RoomKind::Aviary).unwrap(),
            a::CLERK,
        );
        r.step(&fire);
        for i in [1usize, 2, 0] {
            let (kind, at, _) = a::DOCUMENTS[i];
            r.arrive(world::room_of(&r.dungeon, kind).unwrap(), at);
            let (x, y) = r.bonk_pigeon_for_test(1).expect("a pigeon up");
            let h = r.players.get_mut(&1).unwrap();
            (h.x, h.y) = (x, y);
            r.step(&fire);
        }
        r.arrive(
            world::room_of(&r.dungeon, RoomKind::Aviary).unwrap(),
            a::CLERK,
        );
        r.step(&fire);
        assert_eq!(a::progress(&r.home.countryside, &r.marks), (2, 7));
        let p = &c::PLOTS[10];
        r.arrive(world::room_of(&r.dungeon, p.room).unwrap(), p.at);
        for _ in 0..crate::delve::together_shooter::home::BUY_HOLD {
            r.step(&fire);
        }
        assert_eq!(r.marks.get("country:build:aerie"), Some(&1));
        r.arrive(
            world::room_of(&r.dungeon, RoomKind::Gate).unwrap(),
            (12.0, 9.5),
        );
        r.dungeon
            .rooms
            .retain(|room| !a::PLACES.contains(&room.kind));
    }
    fixture.block_saves();
    assert!(!app.settle_home());
    assert_eq!(app.economy().treasury, before);
    assert_eq!(app.economy().home.countryside.post, 0);
    let path = fixture.workspace.join(".angel/dungeon/session.json");
    app.realm.dungeon.save_path = Some(path.clone());
    app.realm.dungeon
        .checkpoint(std::time::Instant::now(), true)
        .unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    for mark in a::QUEST_MARKS {
        assert_eq!(saved["country_intents"][mark], 1);
    }
    assert_eq!(saved["country_intents"]["country:build:aerie"], 1);
    assert!(saved["run"].get("marks").is_none());
    app.realm.dungeon.shooter.set(None);
    assert!(app.realm.dungeon.restore(&fixture.workspace).is_err(), "unreadable realm company");
    fixture.allow_saves();
    app.realm.dungeon.restore(&fixture.workspace).unwrap();
    fixture.block_saves();
    let r = app.realm.dungeon.shooter.as_ref().unwrap();
    assert!(r.valid_snapshot());
    for kind in a::PLACES {
        assert!(world::room_of(&r.dungeon, kind).is_some());
    }
    assert!(!app.settle_home());
    assert_eq!(app.economy().treasury, before);
    fixture.allow_saves();
    assert!(app.settle_home());
    assert_eq!(
        app.economy().treasury.get(Spoil::Gold),
        before.get(Spoil::Gold) + 75 - 100
    );
    assert_eq!(
        app.economy().treasury.get(Spoil::Bone),
        before.get(Spoil::Bone) - 4
    );
    assert_eq!(app.economy().treasury.get(Spoil::Wax), before.get(Spoil::Wax));
    assert!(app.economy().home.countryside.has(10) && app.economy().home.countryside.post_paid);
    let paid = app.economy().treasury.clone();
    assert!(app.settle_home());
    assert_eq!(app.economy().treasury, paid);
    // Eleventh pad bit and airborne ball survive the real private session restore.
    let r = app.realm.dungeon.shooter.edit().unwrap();
    r.arrive(
        world::room_of(&r.dungeon, RoomKind::Belfry).unwrap(),
        Game::Volley.start(),
    );
    r.step(&BTreeMap::new());
    // The serve is the game's own key: F, not the interact key.
    r.step(&BTreeMap::from([(
        1,
        Input {
            fire: true,
            ..Default::default()
        },
    )]));
    for _ in 0..9 {
        r.step(&BTreeMap::new());
    }
    assert!(r.valid_snapshot());
    let flight = r.pastimes.clone();
    let plays = r.home.countryside.games.plays_for(Game::Volley);
    app.realm.dungeon
        .checkpoint(std::time::Instant::now(), true)
        .unwrap();
    app.realm.dungeon.shooter.set(None);
    app.realm.dungeon.restore(&fixture.workspace).unwrap();
    let r = app.realm.dungeon.shooter.edit().unwrap();
    assert_eq!(r.pastimes, flight);
    assert_eq!(r.home.countryside.games.plays_for(Game::Volley), plays);
    assert!(r.home.countryside.has(10));
    for _ in 0..4 {
        r.step(&BTreeMap::new());
        assert!(r.valid_snapshot());
    }
    assert_ne!(r.pastimes, flight);
    assert!(app.settle_home());
    assert_eq!(
        app.economy().home.countryside.games.plays_for(Game::Volley),
        plays
    );
    assert_eq!(app.economy().treasury, paid);
    assert!(app.realm.dungeon.shooter.as_ref().unwrap().marks.is_empty());
}
