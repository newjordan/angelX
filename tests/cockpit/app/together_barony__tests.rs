//! The barony saved with the cockpit's realm.

use crate::drive::together_shooter::*;
use crate::drive::together_realm::Spoil;
use crate::drive::together_shooter::barony::*;
use crate::drive::together_shooter::barony::testing::*;

#[test]
fn an_older_realm_loads_without_a_barony_and_a_bad_one_resets_alone() {
    use crate::drive::together_realm::Realm;
    let dir = std::env::temp_dir().join(format!("barony-save-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let rewards = dir.join("island.json");
    let path = rewards.with_extension("realm.json");
    std::fs::write(
        &path,
        r#"{"treasury":{"gold":5},"home":{"deepest":3,"levels":{"forge":2}}}"#,
    )
    .unwrap();
    let realm = Realm::beside(Some(&rewards));
    assert_eq!(realm.home.barony, Barony::default());
    assert_eq!(realm.home.level(home::Station::Forge), 2);
    std::fs::write(
        &path,
        r#"{"treasury":{"gold":5},"home":{"deepest":3,"barony":{"court":"emperor","works":7}}}"#,
    )
    .unwrap();
    let realm = Realm::beside(Some(&rewards));
    assert_eq!(realm.treasury.get(Spoil::Gold), 5, "the realm kept");
    assert_eq!(realm.home.barony, Barony::default());
    // And a good one goes round whole.
    let mut realm = realm;
    let mut t = rich();
    settle(
        &mut realm.home,
        &mut t,
        1,
        &marks(&[("barony:met", 1)]),
        &who,
    );
    settle(
        &mut realm.home,
        &mut t,
        1,
        &marks(&[("barony:order:hall:1", 1)]),
        &who,
    );
    realm.save().unwrap();
    let again = Realm::beside(Some(&rewards));
    assert_eq!(again.home.barony, realm.home.barony);
    // Station keys in the saved home are only the old ones: an older build
    // reading this realm loses the barony, never the realm.
    let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert!(
        raw["home"]["levels"]
            .as_object()
            .is_none_or(|l| l.is_empty())
    );
    std::fs::remove_dir_all(dir).unwrap();
}
