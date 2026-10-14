// Part of the existing forced realm-save-failure verifier, not a separate host.
fn eastern_save_retry(app: &mut App, fixture: &RealmSaveFixture) {
    use crate::drive::together_realm::Spoil;
    use crate::drive::together_shooter::{RoomKind, countryside, world};
    app.realm().treasury.add(Spoil::Bone, 4);
    let before = app.realm().treasury.clone();
    let home = app.realm().home.clone();
    app.dungeon.shooter.set(Some(crate::drive::together_shooter::Run::at_home(
        12,
        7,
        None,
        home,
        before.clone(),
    )));
    {
        let run = app.dungeon.shooter.edit().unwrap();
        for i in 0..3 {
            run.marks.insert(format!("country:picnic:{i}"), 1);
        }
        run.marks.insert("country:build:post".into(), 1);
        // An old but otherwise settled floor must be migrated on restore.
        run.dungeon
            .rooms
            .retain(|r| !matches!(r.kind, RoomKind::Causeway | RoomKind::Priory));
    }
    std::fs::remove_dir_all(fixture.home.join(".angelX")).unwrap();
    std::fs::write(fixture.home.join(".angelX"), b"not a directory").unwrap();
    assert!(!app.settle_home());
    assert_eq!(app.realm().treasury, before);
    assert_eq!(app.realm().home.countryside.picnic, 0);
    // Checkpoint uses workspace .angel even while realm saves fail.
    let path = fixture.workspace.join(".angel/dungeon/session.json");
    app.dungeon.save_path = Some(path.clone());
    app.dungeon
        .checkpoint(std::time::Instant::now(), true)
        .unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["country_intents"]["country:picnic:2"], 1);
    assert_eq!(saved["country_intents"]["country:build:post"], 1);
    assert!(saved["run"].get("marks").is_none());
    app.dungeon.shooter.set(None);
    app.dungeon.restore(&fixture.workspace).unwrap();
    let restored = app.dungeon.shooter.as_ref().unwrap();
    assert!(restored.valid_snapshot());
    assert!(world::room_of(&restored.dungeon, RoomKind::Causeway).is_some());
    assert!(world::room_of(&restored.dungeon, RoomKind::Priory).is_some());
    assert_eq!(restored.marks.get("country:picnic:2"), Some(&1));
    assert_eq!(restored.marks.get("country:build:post"), Some(&1));
    assert!(!app.settle_home());
    assert_eq!(app.realm().treasury, before);
    fixture.allow_saves();
    assert!(app.settle_home());
    assert_eq!(
        app.realm().treasury.get(Spoil::Gold),
        before.get(Spoil::Gold) + 60 - 100
    );
    assert_eq!(
        app.realm().treasury.get(Spoil::Wax),
        before.get(Spoil::Wax) + 3 - 2
    );
    assert_eq!(
        app.realm().treasury.get(Spoil::Bone),
        before.get(Spoil::Bone) - 4
    );
    assert!(app.realm().home.countryside.picnic_paid);
    assert!(app.realm().home.countryside.has(5));
    assert!(app.dungeon.shooter.as_ref().unwrap().marks.is_empty());
    let paid = app.realm().treasury.clone();
    assert!(app.settle_home());
    assert_eq!(app.realm().treasury, paid);
    assert!(countryside::intent_key("country:build:post"));
    assert!(!countryside::intent_key("country:unbounded-string"));
}
