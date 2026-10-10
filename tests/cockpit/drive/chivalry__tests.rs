use super::*;
#[test]
fn chivalry_three_real_passes_reject_repeats_lock_mount_and_end_without_rewards() {
    let mut s = Chivalry::default();
    assert!(s.choose(1, Choice::Guard).is_err());
    s.select(Mount::Cinder).unwrap();
    s.tend().unwrap();
    assert!(s.tend().is_err());
    s.start().unwrap();
    assert!(!s.tended[1]);
    assert!(s.start().is_err());
    assert!(s.select(Mount::Mist).is_err());
    assert!(s.tend().is_err());
    assert!(s.choose(0, Choice::Guard).is_err());
    assert!(s.choose(2, Choice::Guard).is_err());
    s.choose(1, Choice::Guard).unwrap();
    let one = s.clone();
    assert!(s.choose(1, Choice::Guard).is_err());
    assert_eq!(s, one);
    s.choose(2, Choice::Aim).unwrap();
    s.choose(3, Choice::Charge).unwrap();
    assert_eq!(s.tournament.score(), 11);
    assert_eq!(s.tournament.phase, Phase::Finished);
    assert_eq!(s.tournament.result(), "WIN");
    assert!(s.choose(3, Choice::Charge).is_err());
    assert!(s.leave().is_err());
    let mut realm = crate::drive::together_realm::Realm::default();
    let treasury = realm.treasury.clone();
    realm.chivalry = s;
    realm.chivalry.start().unwrap();
    for round in 1..=3 {
        realm.chivalry.choose(round, Choice::Charge).unwrap();
    }
    assert_eq!(realm.treasury, treasury);
    assert!(realm.home.trophies.is_empty());
    assert_eq!(realm.raids_won, 0);
}
#[test]
fn chivalry_loss_draw_leave_and_old_save_defaults() {
    let old: crate::drive::together_realm::Realm =
        serde_json::from_str(r#"{"treasury":{}}"#).unwrap();
    assert_eq!(old.chivalry, Chivalry::default());
    let mut s = old.chivalry;
    s.start().unwrap();
    for (i, c) in [Choice::Aim, Choice::Charge, Choice::Guard]
        .into_iter()
        .enumerate()
    {
        s.choose(i + 1, c).unwrap();
    }
    assert_eq!(s.tournament.result(), "LOSS");
    // Bramble guard earns specialty +1: charge tie + guard tie + charge win = 6.
    s.start().unwrap();
    for (i, c) in [Choice::Charge, Choice::Guard, Choice::Charge]
        .into_iter()
        .enumerate()
    {
        s.choose(i + 1, c).unwrap();
    }
    assert_eq!(s.tournament.result(), "DRAW");
    s.start().unwrap();
    s.choose(1, Choice::Guard).unwrap();
    s.leave().unwrap();
    assert_eq!(s.tournament.phase, Phase::Left);
    assert!(s.leave().is_err());
    let encoded = serde_json::to_string(&s).unwrap();
    assert_eq!(s, serde_json::from_str(&encoded).unwrap());
    s.tournament.phase = Phase::Finished;
    s.normalize();
    assert_eq!(s.tournament, Tournament::default());
}

#[test]
fn chivalry_malformed_subtree_preserves_nonempty_realm_across_load_action_save() {
    use crate::drive::together_realm::Realm;
    let _guard = crate::tests::env_lock();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("chivalry-subtree-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let rewards = dir.join("owner.json");
    let path = rewards.with_extension("realm.json");
    let mut realm = Realm::beside(Some(&rewards));
    realm
        .treasury
        .add(crate::drive::together_realm::Spoil::Gold, 321);
    realm.home.deepest = 8;
    realm
        .home
        .levels
        .insert(crate::drive::together_shooter::home::Station::Wing, 1);
    realm.raids_won = 7;
    realm.grails = 2;
    realm.reclaimed.insert("Cavern".into(), 9);
    // Seed the catalog normally unlocked by this progression before capturing
    // the invariant; Realm::beside legitimately offers it on every load.
    realm.offer_catalog();
    realm.chivalry.select(Mount::Cinder).unwrap();
    realm.chivalry.tend().unwrap();
    realm.chivalry.start().unwrap();
    realm.chivalry.choose(1, Choice::Guard).unwrap();
    realm.save().unwrap();
    let before = serde_json::to_value(&realm).unwrap();
    assert_eq!(Realm::beside(Some(&rewards)).chivalry, realm.chivalry);
    let mut cases = vec![serde_json::Value::Null, serde_json::json!("broken")];
    for (field, bad) in [
        ("phase", serde_json::json!("future-phase")),
        ("passes", serde_json::json!([null])),
        ("mount", serde_json::json!("unknown-horse")),
    ] {
        let mut state = before["chivalry"].clone();
        state["tournament"][field] = bad;
        cases.push(state);
    }
    for malformed in cases {
        let mut saved = before.clone();
        saved["chivalry"] = malformed;
        std::fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();
        let mut loaded = Realm::beside(Some(&rewards));
        assert_eq!(loaded.chivalry, Chivalry::default());
        loaded.chivalry.select(Mount::Mist).unwrap();
        loaded.chivalry.tend().unwrap();
        loaded.save().unwrap();
        let reloaded = Realm::beside(Some(&rewards));
        assert_eq!(reloaded.chivalry, loaded.chivalry);
        let mut actual = serde_json::to_value(&reloaded).unwrap();
        // Every unrelated serialized field, including the treasury, home and catalog,
        // must survive both the fallback and the next successful game action.
        actual["chivalry"] = before["chivalry"].clone();
        assert_eq!(actual, before);
    }
    std::fs::remove_dir_all(dir).unwrap();
}
