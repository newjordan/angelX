// Extend the established failed-save -> checkpoint -> restore -> retry gate.
fn wetland_save_retry(app: &mut App, fixture: &RealmSaveFixture) {
    use crate::delve::together_realm::Spoil;
    use crate::delve::together_shooter::{
        Input, RoomKind, Run, countryside as c,
        pastimes::{self, Game},
        wetland as wet, world,
    };
    use std::collections::BTreeMap;
    app.economy().treasury.add(Spoil::Bone, 4);
    app.economy().treasury.add(Spoil::Ore, 2);
    let before = app.economy().treasury.clone();
    let home = app.economy().home.clone();
    app.realm.dungeon.shooter.set(Some(Run::at_home(14, 7, None, home, before.clone())));
    {
        let r = app.realm.dungeon.shooter.edit().unwrap();
        for &(kind, at) in &wet::ERRAND {
            r.arrive(world::room_of(&r.dungeon, kind).unwrap(), at);
            r.step(&BTreeMap::from([(
                1,
                Input {
                    interact: true,
                    ..Default::default()
                },
            )]));
        }
        assert_eq!(wet::stage(&r.home.countryside, &r.marks), 6);
        let p = &c::PLOTS[9];
        r.arrive(world::room_of(&r.dungeon, p.room).unwrap(), p.at);
        for _ in 0..crate::delve::together_shooter::home::BUY_HOLD {
            r.step(&BTreeMap::from([(
                1,
                Input {
                    interact: true,
                    ..Default::default()
                },
            )]));
        }
        assert_eq!(r.marks.get("country:build:punthouse"), Some(&1));
        r.arrive(
            world::room_of(&r.dungeon, RoomKind::Gate).unwrap(),
            (12.0, 9.5),
        );
        r.dungeon
            .rooms
            .retain(|room| !wet::PLACES.contains(&room.kind));
    }
    fixture.block_saves();
    assert!(!app.settle_home());
    assert_eq!(app.economy().treasury, before);
    assert_eq!(app.economy().home.countryside.sluice, 0);
    let path = fixture.workspace.join(".angel/dungeon/session.json");
    app.realm.dungeon.save_path = Some(path.clone());
    app.realm.dungeon
        .checkpoint(std::time::Instant::now(), true)
        .unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["country_intents"]["country:sluice:5"], 1);
    assert_eq!(saved["country_intents"]["country:build:punthouse"], 1);
    assert!(saved["run"].get("marks").is_none());
    app.realm.dungeon.shooter.set(None);
    assert!(app.realm.dungeon.restore(&fixture.workspace).is_err(), "unreadable realm company");
    fixture.allow_saves();
    app.realm.dungeon.restore(&fixture.workspace).unwrap();
    fixture.block_saves();
    assert!(app.realm.dungeon.shooter.as_ref().unwrap().valid_snapshot());
    for kind in wet::PLACES {
        assert!(world::room_of(&app.realm.dungeon.shooter.as_ref().unwrap().dungeon, kind).is_some());
    }
    assert!(!app.settle_home());
    assert_eq!(app.economy().treasury, before);
    fixture.allow_saves();
    assert!(app.settle_home());
    assert_eq!(
        app.economy().treasury.get(Spoil::Gold),
        before.get(Spoil::Gold) + 65 - 105
    );
    assert_eq!(
        app.economy().treasury.get(Spoil::Bone),
        before.get(Spoil::Bone) - 4
    );
    assert_eq!(app.economy().treasury.get(Spoil::Ore), before.get(Spoil::Ore));
    assert!(app.economy().home.countryside.has(9) && app.economy().home.countryside.sluice_paid);
    let paid = app.economy().treasury.clone();
    assert!(app.settle_home());
    assert_eq!(app.economy().treasury, paid);
    // Active crest phase and tenth pad bit survive the real session restart.
    let r = app.realm.dungeon.shooter.edit().unwrap();
    r.arrive(
        world::room_of(&r.dungeon, RoomKind::Millpond).unwrap(),
        Game::Surf.start(),
    );
    r.step(&BTreeMap::new());
    let mut held = false;
    // The round's clock starts a second after the walk-in, so the drive
    // is bounded by the game's whole time, like the delve-side test, and
    // stops the moment the crest is braced.
    for _ in 0..pastimes::Game::Surf.time() {
        let round = r.pastimes.round.as_ref().unwrap();
        let surf = round.surf.as_ref().unwrap();
        if surf.braced {
            break;
        }
        let target = pastimes::WAVE_LEANS[usize::from(round.progress)];
        // Brace on a published gold frame (the engine merge moved judging
        // one frame later than the old full-course check).
        let fire = surf.phase >= pastimes::WAVE_TICKS - pastimes::CREST_WINDOW && !held;
        r.step(&BTreeMap::from([(
            1,
            Input {
                aim_x: (target - surf.lean).signum() as i8,
                fire,
                ..Default::default()
            },
        )]));
        held = fire;
    }
    let flight = r.pastimes.clone();
    let prior = r.home.countryside.games.surf.plays;
    assert!(r.valid_snapshot());
    assert!(
        r.pastimes
            .round
            .as_ref()
            .unwrap()
            .surf
            .as_ref()
            .unwrap()
            .braced
    );
    app.realm.dungeon
        .checkpoint(std::time::Instant::now(), true)
        .unwrap();
    app.realm.dungeon.shooter.set(None);
    app.realm.dungeon.restore(&fixture.workspace).unwrap();
    let r = app.realm.dungeon.shooter.edit().unwrap();
    assert_eq!(r.pastimes, flight);
    assert!(r.home.countryside.has(9));
    for _ in 0..20 * crate::delve::together_shooter::HZ {
        let Some(round) = r.pastimes.round.as_ref() else {
            break;
        };
        let surf = round.surf.as_ref().unwrap();
        let target = pastimes::WAVE_LEANS[usize::from(round.progress)];
        // Brace on a published gold frame (the engine merge moved judging
        // one frame later than the old full-course check).
        let fire = surf.phase >= pastimes::WAVE_TICKS - pastimes::CREST_WINDOW && !held;
        r.step(&BTreeMap::from([(
            1,
            Input {
                aim_x: (target - surf.lean).signum() as i8,
                fire,
                ..Default::default()
            },
        )]));
        held = fire;
        assert!(r.valid_snapshot());
    }
    assert_eq!(r.home.countryside.games.surf.wins, 1);
    assert_eq!(r.home.countryside.games.surf.plays, prior);
    fixture.block_saves();
    assert!(!app.settle_home());
    assert_eq!(app.economy().home.countryside.games.surf.wins, 0);
    assert_eq!(
        app.realm.dungeon
            .shooter
            .as_ref()
            .unwrap()
            .home
            .countryside
            .games
            .surf
            .wins,
        1
    );
    fixture.allow_saves();
    assert!(app.settle_home());
    assert_eq!(app.economy().home.countryside.games.surf.wins, 1);
    // The surf's first win pays its once-only purse, and nothing else moves.
    let mut expected = paid.clone();
    expected.add(crate::delve::together_realm::Spoil::Gold, c::FIRST_WIN_GOLD);
    assert_eq!(app.economy().treasury, expected);
}
