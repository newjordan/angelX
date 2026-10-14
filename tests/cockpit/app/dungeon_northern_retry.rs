// Additional northern intents through the existing failed-save/checkpoint test.
fn northern_save_retry(app: &mut App, fixture: &RealmSaveFixture) {
    use crate::drive::together_realm::Spoil;
    use crate::drive::together_shooter::{RoomKind, Run, countryside as c, world};
    use std::collections::BTreeMap;
    app.realm().treasury.add(Spoil::Ore, 3);
    app.realm().treasury.add(Spoil::Wax, 2);
    let before = app.realm().treasury.clone();
    let home = app.realm().home.clone();
    app.dungeon.shooter.set(Some(Run::at_home(12, 7, None, home, before.clone())));
    {
        let r = app.dungeon.shooter.edit().unwrap();
        // Real interactions, ordered and spatially separate; no injected
        // completion bits. Restore has to carry all six pending requests.
        for &(kind, at) in &c::FORECAST {
            r.arrive(world::room_of(&r.dungeon, kind).unwrap(), at);
            r.step(&BTreeMap::from([(
                1,
                crate::drive::together_shooter::Input {
                    interact: true,
                    ..Default::default()
                },
            )]));
        }
        let p = &c::PLOTS[6];
        r.arrive(world::room_of(&r.dungeon, p.room).unwrap(), p.at);
        for _ in 0..crate::drive::together_shooter::home::BUY_HOLD {
            r.step(&BTreeMap::from([(
                1,
                crate::drive::together_shooter::Input {
                    interact: true,
                    ..Default::default()
                },
            )]));
        }
        assert_eq!(c::forecast_stage(&r.home.countryside, &r.marks), 6);
        assert_eq!(r.marks.get("country:build:ropeway"), Some(&1));
        // An old settled floor needs all three northern rooms back.
        r.arrive(
            world::room_of(&r.dungeon, RoomKind::Gate).unwrap(),
            (12.0, 9.5),
        );
        r.dungeon
            .rooms
            .retain(|room| !c::NORTH_PLACES.contains(&room.kind));
    }
    std::fs::remove_dir_all(fixture.home.join(".angelX")).unwrap();
    std::fs::write(fixture.home.join(".angelX"), b"not a directory").unwrap();
    assert!(!app.settle_home());
    assert_eq!(app.realm().treasury, before);
    assert_eq!(app.realm().home.countryside.forecast, 0);
    let path = fixture.workspace.join(".angel/dungeon/session.json");
    app.dungeon.save_path = Some(path.clone());
    app.dungeon
        .checkpoint(std::time::Instant::now(), true)
        .unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(saved["country_intents"]["country:forecast:5"], 1);
    assert_eq!(saved["country_intents"]["country:build:ropeway"], 1);
    assert!(saved["run"].get("marks").is_none());
    // The whitelist has one finite entry per authored action. A fully occupied finite journal
    // must not be rejected by the old hardcoded limit of 32 on restart.
    let mut full = saved.clone();
    let journal = full["country_intents"].as_object_mut().unwrap();
    for (prefix, count) in [
        ("visit", 8),
        ("west", 2),
        ("east", 2),
        ("north", 3),
        ("coast", 3),
        ("harbour", 3),
        ("wood", 3),
        ("soup", 5),
        ("wet", 3),
        ("sluice", 6),
        ("picnic", 3),
        ("forecast", 6),
        ("relic", 3),
        ("lamp", 3),
        ("survey", 3),
    ] {
        for i in 0..count {
            let key = format!("country:{prefix}:{i}");
            assert!(c::intent_key(&key));
            journal.insert(key, 1.into());
        }
    }
    for key in crate::drive::together_shooter::aerie::VISIT_MARKS
        .into_iter()
        .chain(crate::drive::together_shooter::aerie::QUEST_MARKS)
        .chain(crate::drive::together_shooter::mummers::VISIT_MARKS)
        .chain(crate::drive::together_shooter::hippodrome::VISIT_MARKS)
        .chain(crate::drive::together_shooter::hippodrome::QUEST_MARKS)
        .chain(crate::drive::together_shooter::hippodrome::WAGER_MARKS)
        .chain(crate::drive::together_shooter::windcountry::VISIT_MARKS)
        .chain(crate::drive::together_shooter::windcountry::QUEST_MARKS)
        .chain(crate::drive::together_shooter::bowcountry::VISIT_MARKS)
        .chain(crate::drive::together_shooter::bowcountry::QUEST_MARKS)
        .chain([crate::drive::together_shooter::race3d::RACE_MARK])
    {
        assert!(c::intent_key(key));
        journal.insert(key.into(), 1.into());
    }
    for p in &c::PLOTS {
        let key = format!("country:build:{}", p.id);
        assert!(c::intent_key(&key));
        journal.insert(key, 1.into());
    }
    // College requests share this bounded journal with the countryside.
    // Include every finite College action when exercising its full limit.
    use crate::drive::together_shooter::college;
    let mut college_keys: Vec<String> = [
        "tome",
        "together",
        "bookshelf",
        "stars",
        "familiar",
        "library-note",
        "landing-note",
    ]
    .into_iter()
    .map(|key| format!("college:{key}"))
    .collect();
    for q in &college::QUESTS {
        for verb in ["accept", "turn-in"] {
            college_keys.push(format!("college:{verb}:{}", q.id));
        }
    }
    for i in 0..3 {
        college_keys.push(format!("college:sigil:{i}"));
        college_keys.push(format!("college:sample:1:3:5:{i}:0"));
    }
    for subject in ["bat", "skeleton", "slime"] {
        college_keys.push(format!("college:observe:{subject}"));
    }
    for i in 0..4 {
        college_keys.push(format!("college:map:1:3:0:{i}:0"));
    }
    college_keys.extend(college::CLUES.iter().map(|(_, _, key, _)| key.to_string()));
    assert_eq!(college_keys.len(), college::INTENT_LIMIT);
    for key in college_keys {
        assert!(c::intent_key(&key));
        journal.insert(key, 1.into());
    }
    assert_eq!(journal.len(), c::INTENT_LIMIT);
    assert!(journal.len() > 32);
    std::fs::write(&path, serde_json::to_vec(&full).unwrap()).unwrap();
    app.dungeon.shooter.set(None);
    app.dungeon.restore(&fixture.workspace).unwrap();
    assert_eq!(
        app.dungeon.shooter.as_ref().unwrap().marks.len(),
        c::INTENT_LIMIT
    );
    // An unknown key is rejected, never admitted by the larger finite limit.
    full["country_intents"]["country:unbounded-string"] = 1.into();
    std::fs::write(&path, serde_json::to_vec(&full).unwrap()).unwrap();
    assert!(app.dungeon.restore(&fixture.workspace).is_err());
    std::fs::write(&path, &bytes).unwrap();
    app.dungeon.shooter.set(None);
    app.dungeon.restore(&fixture.workspace).unwrap();
    let r = app.dungeon.shooter.as_ref().unwrap();
    assert!(r.valid_snapshot());
    for &kind in &c::NORTH_PLACES {
        assert!(world::room_of(&r.dungeon, kind).is_some());
    }
    assert_eq!(r.marks.get("country:forecast:5"), Some(&1));
    assert!(!app.settle_home());
    assert_eq!(app.realm().treasury, before);
    fixture.allow_saves();
    assert!(app.settle_home());
    assert_eq!(
        app.realm().treasury.get(Spoil::Gold),
        before.get(Spoil::Gold) + 55 - 110
    );
    assert_eq!(
        app.realm().treasury.get(Spoil::Ore),
        before.get(Spoil::Ore) + 3 - 3
    );
    assert_eq!(
        app.realm().treasury.get(Spoil::Wax),
        before.get(Spoil::Wax) - 2
    );
    assert!(app.realm().home.countryside.forecast_paid);
    assert!(app.realm().home.countryside.has(6));
    assert!(app.dungeon.shooter.as_ref().unwrap().marks.is_empty());
    let paid = app.realm().treasury.clone();
    assert!(app.settle_home());
    assert_eq!(app.realm().treasury, paid);
}
