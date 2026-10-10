use super::*;

#[test]
fn each_mount_rides_differently_at_the_lists() {
    let (b, c, m) = (
        Mount::Bramble.steed(),
        Mount::Cinder.steed(),
        Mount::Mist.steed(),
    );
    // Bramble: the most balance and the widest brace.
    assert!(b.balance > c.balance && b.balance > m.balance);
    assert!(b.brace > c.brace && b.brace > m.brace);
    // Cinder: the soonest meeting and the heaviest hit.
    assert!(c.charge < b.charge && c.charge < m.charge);
    assert!(c.weight > b.weight && c.weight > m.weight);
    // Mist: the widest strike and the earliest read of the rival's guard.
    assert!(m.strike > b.strike && m.strike > c.strike);
    assert!(m.tell > b.tell && m.tell > c.tell);
    for mount in Mount::ALL {
        assert_eq!(Mount::parse(&mount.name().to_lowercase()), Some(mount));
        assert!(!mount.says().is_empty());
    }
}

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
