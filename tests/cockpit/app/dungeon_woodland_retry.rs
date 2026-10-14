// Extend the established failed-save -> checkpoint -> restore -> retry gate.
fn woodland_save_retry(app: &mut App, fixture: &RealmSaveFixture) {
    use crate::drive::together_realm::Spoil;
    use crate::drive::together_shooter::{
        Input, RoomKind, Run, countryside as c,
        pastimes::{self, Game},
        woodland as wood, world,
    };
    use std::collections::BTreeMap;
    app.realm().treasury.add(Spoil::Bone, 3);
    app.realm().treasury.add(Spoil::Wax, 2);
    let before = app.realm().treasury.clone();
    let home = app.realm().home.clone();
    app.dungeon.shooter.set(Some(Run::at_home(14, 7, None, home, before.clone())));
    {
        let r = app.dungeon.shooter.edit().unwrap();
        // The job, the Boot (it kicks the first try), the Hat, the Beard, the pot.
        for i in [0, 1, 1, 2, 3, 4] {
            let (kind, at) = wood::ERRAND[i];
            r.arrive(world::room_of(&r.dungeon, kind).unwrap(), at);
            r.step(&BTreeMap::from([(
                1,
                Input {
                    interact: true,
                    ..Default::default()
                },
            )]));
        }
        assert_eq!(wood::stage(&r.home.countryside, &r.marks), 5);
        let p = &c::PLOTS[8];
        r.arrive(world::room_of(&r.dungeon, p.room).unwrap(), p.at);
        for _ in 0..crate::drive::together_shooter::home::BUY_HOLD {
            r.step(&BTreeMap::from([(
                1,
                Input {
                    interact: true,
                    ..Default::default()
                },
            )]));
        }
        assert_eq!(r.marks.get("country:build:bothy"), Some(&1));
        r.arrive(
            world::room_of(&r.dungeon, RoomKind::Gate).unwrap(),
            (12.0, 9.5),
        );
        r.dungeon
            .rooms
            .retain(|room| !wood::PLACES.contains(&room.kind));
    }
    std::fs::remove_dir_all(fixture.home.join(".angelX")).unwrap();
    std::fs::write(fixture.home.join(".angelX"), b"not a directory").unwrap();
    assert!(!app.settle_home());
    assert_eq!(app.realm().treasury, before);
    assert_eq!(app.realm().home.countryside.soup, 0);
    let path = fixture.workspace.join(".angel/dungeon/session.json");
    app.dungeon.save_path = Some(path.clone());
    app.dungeon
        .checkpoint(std::time::Instant::now(), true)
        .unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["country_intents"]["country:soup:4"], 1);
    assert_eq!(saved["country_intents"]["country:build:bothy"], 1);
    assert!(saved["run"].get("marks").is_none());
    app.dungeon.shooter.set(None);
    app.dungeon.restore(&fixture.workspace).unwrap();
    assert!(app.dungeon.shooter.as_ref().unwrap().valid_snapshot());
    for kind in wood::PLACES {
        assert!(world::room_of(&app.dungeon.shooter.as_ref().unwrap().dungeon, kind).is_some());
    }
    assert!(!app.settle_home());
    assert_eq!(app.realm().treasury, before);
    fixture.allow_saves();
    assert!(app.settle_home());
    assert_eq!(
        app.realm().treasury.get(Spoil::Gold),
        before.get(Spoil::Gold) + 70 - 95
    );
    assert_eq!(
        app.realm().treasury.get(Spoil::Bone),
        before.get(Spoil::Bone) - 3
    );
    assert_eq!(app.realm().treasury.get(Spoil::Wax), before.get(Spoil::Wax));
    assert!(app.realm().home.countryside.has(8) && app.realm().home.countryside.soup_paid);
    let paid = app.realm().treasury.clone();
    assert!(app.settle_home());
    assert_eq!(app.realm().treasury, paid);
    // Active airborne cart and ninth start-pad bit survive session restart.
    let r = app.dungeon.shooter.edit().unwrap();
    r.arrive(
        world::room_of(&r.dungeon, RoomKind::Timberyard).unwrap(),
        Game::Cart.start(),
    );
    r.step(&BTreeMap::new());
    r.step(&BTreeMap::from([(
        1,
        Input {
            aim_x: 1,
            fire: true,
            ..Default::default()
        },
    )]));
    let flight = r.pastimes.clone();
    let prior = r.home.countryside.games.cart.plays;
    assert!(r.valid_snapshot());
    app.dungeon
        .checkpoint(std::time::Instant::now(), true)
        .unwrap();
    app.dungeon.shooter.set(None);
    app.dungeon.restore(&fixture.workspace).unwrap();
    let r = app.dungeon.shooter.edit().unwrap();
    assert_eq!(r.pastimes, flight);
    assert!(r.home.countryside.has(8));
    let mut held = true;
    for _ in 0..40 * crate::drive::together_shooter::HZ {
        let Some(round) = r.pastimes.round.as_ref() else {
            break;
        };
        let c = round.cart.as_ref().unwrap();
        let fire = pastimes::CART_LOGS[usize::from(round.progress)] * 256 - c.x <= 5 * 36
            && c.air_left == 0
            && c.rest_left == 0
            && !held;
        r.step(&BTreeMap::from([(
            1,
            Input {
                aim_x: 1,
                fire,
                ..Default::default()
            },
        )]));
        held = fire;
        assert!(r.valid_snapshot());
    }
    assert_eq!(r.home.countryside.games.cart.wins, 1);
    assert_eq!(r.home.countryside.games.cart.plays, prior);
    std::fs::remove_dir_all(fixture.home.join(".angelX")).unwrap();
    std::fs::write(fixture.home.join(".angelX"), b"not a directory").unwrap();
    assert!(!app.settle_home());
    assert_eq!(app.realm().home.countryside.games.cart.wins, 0);
    assert_eq!(
        app.dungeon
            .shooter
            .as_ref()
            .unwrap()
            .home
            .countryside
            .games
            .cart
            .wins,
        1
    );
    fixture.allow_saves();
    assert!(app.settle_home());
    assert_eq!(app.realm().home.countryside.games.cart.wins, 1);
    // The cart's first win pays its once-only purse, and nothing else moves.
    let mut expected = paid.clone();
    expected.add(Spoil::Gold, c::FIRST_WIN_GOLD);
    assert_eq!(app.realm().treasury, expected);
}
