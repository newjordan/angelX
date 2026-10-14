use super::*;

#[test]
fn places_parse_from_the_words_commands_use() {
    for word in ["stable", "Stables"] {
        assert_eq!(Place::parse(word), Some(Place::Stables));
    }
    for word in ["tournament", "knights", "lists", "joust"] {
        assert_eq!(Place::parse(word), Some(Place::Tournament));
    }
    assert_eq!(Place::parse("tavern"), None);
}

#[test]
fn a_malformed_stable_resets_alone_and_never_costs_the_realm() {
    use crate::drive::together_realm::{Realm, Spoil};
    let dir = std::env::temp_dir().join(format!("stable-lenient-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let rewards = dir.join("island.json");
    std::fs::write(
        rewards.with_extension("realm.json"),
        r#"{"treasury":{"gold":77},"home":{"deepest":2,"stable":{"selected":"unicorn","bouts":"many"}}}"#,
    )
    .unwrap();
    let realm = Realm::beside(Some(&rewards));
    assert_eq!(realm.treasury.get(Spoil::Gold), 77, "the realm kept");
    assert_eq!(realm.home.deepest, 2);
    assert_eq!(realm.home.stable, Stable::default());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_old_practice_games_mount_moves_into_the_stable_once() {
    use crate::drive::together_realm::Realm;
    let dir = std::env::temp_dir().join(format!("stable-migrate-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let rewards = dir.join("island.json");
    // A 0.2.0 save: the practice subtree, its tournament and all.
    std::fs::write(
        rewards.with_extension("realm.json"),
        r#"{"treasury":{},"chivalry":{"selected":"mist","tended":[false,false,true],"tournament":{"phase":"finished","mount":"mist","prepared":true,"passes":["aim","guard","charge"]}}}"#,
    )
    .unwrap();
    let mut realm = Realm::beside(Some(&rewards));
    assert_eq!(realm.home.stable.selected, Mount::Mist);
    assert!(realm.home.stable.is_tended(Mount::Mist));
    // Once moved, the stable is the realm's: later changes stand.
    realm.home.stable.selected = Mount::Cinder;
    realm.save().unwrap();
    let again = Realm::beside(Some(&rewards));
    assert_eq!(again.home.stable.selected, Mount::Cinder);
    std::fs::remove_dir_all(dir).unwrap();
}
