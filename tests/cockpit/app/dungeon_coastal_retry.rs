// The existing failed realm-save/checkpoint/restart test also exercises the coast.
fn coastal_save_retry(app: &mut App, fixture: &RealmSaveFixture) {
    use crate::drive::together_realm::Spoil;
    use crate::drive::together_shooter::{
        HZ, Input, RoomKind, Run, coastal as coast, countryside as c, world,
    };
    use std::collections::BTreeMap;
    app.realm().treasury.add(Spoil::Ore, 3);
    app.realm().treasury.add(Spoil::Bone, 3);
    let before = app.realm().treasury.clone();
    let home = app.realm().home.clone();
    app.dungeon.shooter.set(Some(Run::at_home(13, 7, None, home, before.clone())));
    {
        let r = app.dungeon.shooter.edit().unwrap();
        for &(kind, at) in &coast::ERRAND {
            r.arrive(world::room_of(&r.dungeon, kind).unwrap(), at);
            // Await the next low tide through real ticks, never inject quest bits.
            if kind == RoomKind::Strand {
                while !coast::low_tide(r.tick + 1) {
                    r.step(&BTreeMap::new());
                }
            }
            r.step(&BTreeMap::from([(
                1,
                Input {
                    interact: true,
                    ..Default::default()
                },
            )]));
        }
        assert_eq!(coast::stage(&r.home.countryside, &r.marks), 3);
        let p = &c::PLOTS[7];
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
        assert_eq!(r.marks.get("country:build:harbour"), Some(&1));
        r.arrive(
            world::room_of(&r.dungeon, RoomKind::Gate).unwrap(),
            (12.0, 9.5),
        );
        r.dungeon
            .rooms
            .retain(|room| !coast::PLACES.contains(&room.kind));
    }
    std::fs::remove_dir_all(fixture.home.join(".angelX")).unwrap();
    std::fs::write(fixture.home.join(".angelX"), b"not a directory").unwrap();
    assert!(!app.settle_home());
    assert_eq!(app.realm().treasury, before);
    assert_eq!(app.realm().home.countryside.harbour, 0);
    let path = fixture.workspace.join(".angel/dungeon/session.json");
    app.dungeon.save_path = Some(path.clone());
    app.dungeon
        .checkpoint(std::time::Instant::now(), true)
        .unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["country_intents"]["country:harbour:2"], 1);
    assert_eq!(saved["country_intents"]["country:build:harbour"], 1);
    assert!(saved["run"].get("marks").is_none());
    app.dungeon.shooter.set(None);
    app.dungeon.restore(&fixture.workspace).unwrap();
    let r = app.dungeon.shooter.as_ref().unwrap();
    assert!(r.valid_snapshot());
    for &kind in &coast::PLACES {
        assert!(world::room_of(&r.dungeon, kind).is_some());
    }
    assert_eq!(r.marks.get("country:harbour:2"), Some(&1));
    assert!(!app.settle_home());
    assert_eq!(app.realm().treasury, before);
    fixture.allow_saves();
    assert!(app.settle_home());
    assert_eq!(
        app.realm().treasury.get(Spoil::Gold),
        before.get(Spoil::Gold) + 65 - 90
    );
    assert_eq!(
        app.realm().treasury.get(Spoil::Bone),
        before.get(Spoil::Bone) + 3 - 3
    );
    assert_eq!(
        app.realm().treasury.get(Spoil::Ore),
        before.get(Spoil::Ore) - 3
    );
    assert!(app.realm().home.countryside.harbour_paid && app.realm().home.countryside.has(7));
    assert!(app.dungeon.shooter.as_ref().unwrap().marks.is_empty());
    assert_eq!(app.realm().home.countryside.core_visits(), 6);
    let paid = app.realm().treasury.clone();
    assert!(app.settle_home());
    assert_eq!(app.realm().treasury, paid);
    // Checkpoint a charged put through the same private session envelope.
    let r = app.dungeon.shooter.edit().unwrap();
    r.arrive(
        world::room_of(&r.dungeon, RoomKind::Strand).unwrap(),
        (4.0, 7.0),
    );
    r.step(&BTreeMap::new());
    for _ in 0..16 {
        r.step(&BTreeMap::from([(
            1,
            Input {
                fire: true,
                ..Default::default()
            },
        )]));
    }
    let charged = r.pastimes.clone();
    assert!(r.valid_snapshot());
    app.dungeon
        .checkpoint(std::time::Instant::now(), true)
        .unwrap();
    app.dungeon.shooter.set(None);
    app.dungeon.restore(&fixture.workspace).unwrap();
    let r = app.dungeon.shooter.edit().unwrap();
    assert_eq!(r.pastimes, charged);
    r.step(&BTreeMap::new());
    assert!(
        r.pastimes
            .round
            .as_ref()
            .unwrap()
            .put
            .as_ref()
            .unwrap()
            .flight
            .is_some()
    );
    for _ in 0..HZ * 2 {
        r.step(&BTreeMap::new());
    }
    assert_eq!(r.pastimes.round.as_ref().unwrap().progress, 1);
}
