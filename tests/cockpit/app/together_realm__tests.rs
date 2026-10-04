use super::*;

const SHRINE: &str = "name Shrine of the Fallen\nwords a little shrine for knights who fell\nby Sam\nnear chapel\nprice bone 6, wax 2\nart\n..hHh..\n.hiHih.\n.h565h.\n.hJJJh.\nGGGGGGG\n";

fn spoils(pairs: &[(Spoil, u32)]) -> Spoils {
    let mut s = Spoils::default();
    for &(spoil, n) in pairs {
        s.add(spoil, n);
    }
    s
}

#[test]
fn a_wish_goes_from_words_to_draft_to_built_only_when_paid() {
    let mut realm = Realm::default();
    let id = realm
        .ask("Sam", "a little shrine for knights who fell")
        .unwrap();
    assert!(realm.grant(&id).unwrap_err().contains("no draft yet"));
    let (draft, _) = check(&id, SHRINE).unwrap();
    realm.draft(draft).unwrap();
    assert_eq!(realm.wishes[0].status, Status::Drafted);
    assert_eq!(realm.wishes[0].by, "Sam", "the wisher keeps the credit");
    let err = realm.grant(&id).unwrap_err();
    assert!(err.contains("need 6 bone · 2 wax"), "{err}");
    realm.bank("Jordan", &spoils(&[(Spoil::Bone, 4), (Spoil::Wax, 2)]));
    realm.bank("Sam", &spoils(&[(Spoil::Bone, 4)]));
    realm.grant(&id).unwrap();
    assert_eq!(realm.treasury, spoils(&[(Spoil::Bone, 2)]));
    let built = realm.built();
    assert_eq!(built.len(), 1);
    assert_eq!(built[0].raised, 1);
    let names: Vec<_> = built[0].paid.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        ["Jordan", "Sam"],
        "both who brought the spoils are on the plaque"
    );
    assert!(realm.grant(&id).unwrap_err().contains("already stands"));
}

#[test]
fn a_cheap_big_wish_has_its_price_raised_and_bad_files_are_explained() {
    let big = format!(
        "name Big\nprice gold 1\nart\n{}\n",
        vec!["H".repeat(20); 20].join("\n")
    );
    let (draft, notes) = check("big", &big).unwrap();
    assert_eq!(draft.price.get(Spoil::Gold), 800);
    assert!(notes[0].contains("raised"));
    let errors = check("x", "name X\nprice diamonds 4\nnear moon\nart\n..!!\n")
        .unwrap_err()
        .join(" | ");
    assert!(
        errors.contains("spoils are") && errors.contains("near `moon`"),
        "{errors}"
    );
    assert!(
        check("x", "name X\n")
            .unwrap_err()
            .join(" ")
            .contains("needs `art`")
    );
}

#[test]
fn a_wiped_party_keeps_half_and_the_realm_roundtrips() {
    let carried = spoils(&[(Spoil::Bone, 5), (Spoil::Gold, 101), (Spoil::Scale, 1)]);
    assert_eq!(
        carried.half(),
        spoils(&[(Spoil::Bone, 2), (Spoil::Gold, 50)])
    );
    let dir = std::env::temp_dir().join(format!("realm-test-{}", std::process::id()));
    let rewards = dir.join("island.json");
    let mut realm = Realm::beside(Some(&rewards));
    realm.bank("Jordan", &carried);
    realm.ask("Ada", "a bridge over the river").unwrap();
    realm.save().unwrap();
    let again = Realm::beside(Some(&rewards));
    assert_eq!(again.treasury, carried);
    assert_eq!(
        again
            .wishes
            .iter()
            .filter(|w| w.status == Status::Asked)
            .count(),
        1
    );
    assert!(dir.join("island.realm.json").is_file());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_brief_names_the_file_the_words_and_where_spoils_come_from() {
    let mut realm = Realm::default();
    let id = realm
        .ask("Ada", "a tavern with a dragon skull over the door")
        .unwrap();
    let text = brief(
        &realm.wishes[0],
        Path::new(".angel/realm/wishes/x.wish"),
        &realm.treasury,
    );
    assert!(text.contains("a tavern with a dragon skull") && text.contains("Ada"));
    assert!(
        text.contains(".angel/realm/wishes/x.wish")
            && text.contains(&format!("/dungeon grant {id}"))
    );
    assert!(text.contains("embers in Dragon Keep"));
}

#[test]
fn the_grail_chapel_is_offered_only_after_the_dragon_falls() {
    let mut realm = Realm::default();
    realm.offer_catalog();
    assert!(realm.wishes.iter().all(|w| w.id != "grail-chapel"));
    realm.raids_won = 1;
    realm.offer_catalog();
    let chapel = realm
        .wishes
        .iter()
        .find(|w| w.id == "grail-chapel")
        .expect("offered after a victory");
    assert_eq!(chapel.by, "Old Blaise");
    assert_eq!(chapel.price.get(Spoil::Scale), 1);
    let (_, notes) = check("grail-chapel", catalog::AFTER_VICTORY[0].1).unwrap();
    assert!(notes.is_empty(), "{notes:?}");
}
