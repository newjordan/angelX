use super::*;

/// The atomic writers' one-shot fault in the caller's thread, armed as the
/// cockpit's storage regression tests arm it.
mod storage_atomic_tests {
    use angel_live::host::workspace_store::fault::AFTER_RENAME;
    use std::path::PathBuf;

    pub fn fail_after_publish(path: PathBuf) {
        AFTER_RENAME.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(move |published| {
                if published != path {
                    fail_after_publish(path);
                    return Ok(());
                }
                Err("injected directory sync failure after publish".into())
            }));
        });
    }
}

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
    assert!(text.contains("image tool") && text.contains("sheet art"));
    assert!(text.contains(".angel/realm/sheets"));
    assert!(text.contains("summon N swarm|guard|turret") && text.contains("landmark"));
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

struct ClaimFixture(PathBuf);
impl ClaimFixture {
    fn new() -> Self {
        static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "angel-realm-claim-{}-{serial}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn rewards(&self) -> PathBuf {
        self.0.join("island.json")
    }
    fn path(&self) -> PathBuf {
        self.rewards().with_extension("realm.json")
    }
}
impl Drop for ClaimFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn bank_claim(request: u64, gold: u32) -> angel_delve::rules::host_commands::RealmClaim {
    angel_delve::rules::host_commands::RealmClaim {
        session: "realm-session-a".into(),
        raid: 9,
        request,
        transfer: angel_delve::rules::host_commands::RealmTransfer {
            hauls: vec![Haul {
                hero: 1,
                spoils: spoils(&[(Spoil::Gold, gold)]),
                why: "claim fixture".into(),
            }],
            reclaimed: vec![crate::together_shooter::Pack::Hellforge],
            triumph: Some(crate::together_shooter::Triumph::Dragon),
            names: [(1, "You".into())].into_iter().collect(),
        },
    }
}

#[test]
fn a_published_but_unacknowledged_realm_claim_is_confirmed_once_and_stale_saves_cannot_erase_it() {
    let fixture = ClaimFixture::new();
    let mut realm = Realm::beside(Some(&fixture.rewards()));
    let mut stale = realm.clone();
    let claim = bank_claim(1, 19);
    storage_atomic_tests::fail_after_publish(fixture.path());
    assert!(
        realm
            .settle_claim(&claim, "Host")
            .unwrap_err()
            .contains("after publish")
    );
    assert!(realm.treasury.is_empty());
    let published = Realm::beside(Some(&fixture.rewards()));
    assert_eq!(published.treasury.get(Spoil::Gold), 19);
    assert_eq!(published.raids_won, 1);
    stale.bank("Stale", &spoils(&[(Spoil::Gold, 99)]));
    assert!(
        stale
            .save()
            .unwrap_err()
            .to_string()
            .contains("changed on disk")
    );
    assert_eq!(
        Realm::beside(Some(&fixture.rewards()))
            .treasury
            .get(Spoil::Gold),
        19
    );
    let confirmed = realm.settle_claim(&claim, "Host").unwrap();
    assert!(!confirmed.applied && !confirmed.tribute_paid && confirmed.lines.is_empty());
    assert_eq!(realm.treasury.get(Spoil::Gold), 19);
    assert_eq!(realm.reclaimed.values().sum::<u32>(), 1);
    assert_eq!(realm.raids_won, 1);
    let mut reused = claim.clone();
    reused.transfer.hauls[0].spoils.add(Spoil::Gold, 1);
    assert!(
        realm
            .settle_claim(&reused, "Host")
            .unwrap_err()
            .contains("different content")
    );
    let mut next = bank_claim(4, 7); // ordinary acknowledged commands can leave gaps
    next.transfer.reclaimed.clear();
    next.transfer.triumph = None;
    assert!(realm.settle_claim(&next, "Host").unwrap().applied);
    assert_eq!(realm.treasury.get(Spoil::Gold), 26);
    assert!(
        realm
            .settle_claim(&claim, "Host")
            .unwrap_err()
            .contains("predates")
    );
    let mut restored = Realm::beside(Some(&fixture.rewards()));
    assert!(!restored.settle_claim(&next, "Host").unwrap().applied);
    assert_eq!(restored.treasury.get(Spoil::Gold), 26);
    std::fs::remove_file(fixture.path()).unwrap();
    assert!(
        restored
            .save()
            .unwrap_err()
            .to_string()
            .contains("changed on disk")
    );
    assert!(
        !fixture.path().exists(),
        "a missing authoritative receipt was silently recreated"
    );
}

#[test]
fn uncertain_realm_confirmation_preserves_unrelated_unsaved_edits_and_invalid_files() {
    let fixture = ClaimFixture::new();
    let mut realm = Realm::beside(Some(&fixture.rewards()));
    let claim = bank_claim(1, 19);
    storage_atomic_tests::fail_after_publish(fixture.path());
    assert!(realm.settle_claim(&claim, "Host").is_err());
    realm
        .ask("Ada", "an unsaved bridge over the river")
        .unwrap();
    assert!(
        realm
            .settle_claim(&claim, "Host")
            .unwrap_err()
            .contains("unsaved changes")
    );
    assert!(
        realm
            .wishes
            .iter()
            .any(|wish| wish.words.contains("unsaved bridge"))
    );
    assert_eq!(
        Realm::beside(Some(&fixture.rewards()))
            .treasury
            .get(Spoil::Gold),
        19
    );
    std::fs::write(fixture.path(), b"{broken realm").unwrap();
    assert!(
        realm
            .save()
            .unwrap_err()
            .to_string()
            .contains("invalid realm")
    );
    assert_eq!(std::fs::read(fixture.path()).unwrap(), b"{broken realm");
}

#[test]
fn confirming_a_durable_claim_never_publishes_unrelated_unsaved_wishes() {
    let fixture = ClaimFixture::new();
    let mut realm = Realm::beside(Some(&fixture.rewards()));
    realm
        .draft(check("pending-shrine", SHRINE).unwrap().0)
        .unwrap();
    let mut claim = bank_claim(1, 19);
    claim.transfer.hauls[0]
        .spoils
        .merge(&spoils(&[(Spoil::Bone, 6), (Spoil::Wax, 2)]));
    realm.settle_claim(&claim, "Host").unwrap();
    realm.grant("pending-shrine").unwrap();
    assert!(realm.built().iter().any(|wish| wish.id == "pending-shrine"));
    assert!(
        !standing_wishes()
            .iter()
            .any(|wish| wish.id == "pending-shrine")
    );
    assert!(!realm.settle_claim(&claim, "Host").unwrap().applied);
    assert!(
        realm.built().iter().any(|wish| wish.id == "pending-shrine"),
        "unsaved edits were discarded"
    );
    assert!(
        !standing_wishes()
            .iter()
            .any(|wish| wish.id == "pending-shrine"),
        "confirmation published an unsaved wish"
    );
    assert!(
        Realm::beside(Some(&fixture.rewards()))
            .built()
            .iter()
            .all(|wish| wish.id != "pending-shrine")
    );
}

#[test]
fn home_payment_and_refund_survive_uncertain_writes_without_repeating_either_phase() {
    use angel_delve::rules::{Run, home, host_commands::Journal};
    let fixture = ClaimFixture::new();
    let mut realm = Realm::beside(Some(&fixture.rewards()));
    realm.bank("Host", &spoils(&[(Spoil::Gold, 100)]));
    realm.save().unwrap();
    let mut run = Run::at_home(3, 9, None, Default::default(), Default::default());
    run.join_seat(2, "Guest");
    run.stall = vec!["potion".into()];
    run.orders.push(home::Order {
        knight: 2,
        station: home::Station::StallA,
    });
    run.marks.insert("xp".into(), 5);
    let mut journal = Journal::new("realm-session-a".into(), 9);
    let claim = journal.claim_home(&mut run).unwrap().unwrap();
    let price = claim.transfer.stall_prices[0];
    storage_atomic_tests::fail_after_publish(fixture.path());
    assert!(
        realm
            .prepare_home_claim(&claim, "Host", false, |request| request.evaluate())
            .unwrap_err()
            .contains("after publish")
    );
    assert_eq!(
        realm.treasury.get(Spoil::Gold),
        100,
        "uncertain candidate was not adopted"
    );
    let disk = Realm::beside(Some(&fixture.rewards()));
    assert_eq!(disk.treasury.get(Spoil::Gold), 100 - price);
    assert_eq!(disk.home.knights.values().map(|p| p.xp).sum::<u32>(), 10);
    let result = realm
        .prepare_home_claim(&claim, "Host", false, |_| {
            panic!("prepared payment must not re-evaluate replacement code")
        })
        .unwrap();
    assert_eq!(
        realm
            .prepare_home_claim(&claim, "Changed host", true, |_| panic!(
                "prepared payment must not re-evaluate replacement code"
            ))
            .unwrap(),
        result
    );
    assert_eq!(realm.treasury.get(Spoil::Gold), 100 - price);
    assert_eq!(realm.home.knights.values().map(|p| p.xp).sum::<u32>(), 10);
    // The world changed before delivery, so the paid item cannot be placed.
    run.leave(2);
    let delivered = journal
        .complete_home(&mut run, &claim, result.clone())
        .unwrap();
    assert_eq!(delivered.refund.get(Spoil::Gold), price);
    assert!(run.orders.is_empty() && run.marks.is_empty());
    let completed = journal.home_completion(&claim).unwrap().unwrap();
    let saved = journal.capture().unwrap();
    storage_atomic_tests::fail_after_publish(fixture.path());
    assert!(
        realm
            .finish_home_claim(&claim, &completed)
            .unwrap_err()
            .contains("after publish")
    );
    assert_eq!(realm.treasury.get(Spoil::Gold), 100 - price);
    assert_eq!(
        Realm::beside(Some(&fixture.rewards()))
            .treasury
            .get(Spoil::Gold),
        100
    );
    // Restore uses the source's cached delivery even after the destination
    // retires its large prepared payload. Available capacity cannot replay it.
    let mut restored = Journal::restore(saved, &claim.session, 9).unwrap();
    run.dungeon.rooms[run.at].items.clear();
    assert_eq!(
        restored.complete_home(&mut run, &claim, result).unwrap(),
        delivered
    );
    assert!(run.room().items.is_empty());
    realm.finish_home_claim(&claim, &completed).unwrap();
    realm.finish_home_claim(&claim, &completed).unwrap();
    assert_eq!(realm.treasury.get(Spoil::Gold), 100);
    assert!(
        realm
            .prepare_home_claim(&claim, "Host", false, |request| request.evaluate())
            .unwrap_err()
            .contains("retired")
    );
    let disk: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture.path()).unwrap()).unwrap();
    assert!(disk["settlements"]["realm-session-a/home"]["home"]["result"].is_null());
    let mut altered = completed.clone();
    altered.delivered = Default::default();
    assert!(
        realm
            .finish_home_claim(&claim, &altered)
            .unwrap_err()
            .contains("different content")
    );
    assert_eq!(realm.treasury.get(Spoil::Gold), 100);
    restored.acknowledge_home(&claim, &completed).unwrap();
    assert!(restored.pending_home().unwrap().is_none());
}

#[test]
fn bank_and_home_cursors_can_interleave_without_overwriting_each_others_commits() {
    use angel_delve::rules::{Run, host_commands::Journal};
    let fixture = ClaimFixture::new();
    let mut realm = Realm::beside(Some(&fixture.rewards()));
    let mut run = Run::at_home(3, 9, None, Default::default(), Default::default());
    run.marks.insert("xp".into(), 3);
    let mut journal = Journal::new("realm-session-a".into(), 9);
    let claim = journal.claim_home(&mut run).unwrap().unwrap();
    let result = realm
        .prepare_home_claim(&claim, "Host", false, |request| request.evaluate())
        .unwrap();
    let mut bank = bank_claim(5, 19);
    bank.transfer.reclaimed.clear();
    bank.transfer.triumph = None;
    realm.settle_claim(&bank, "Host").unwrap();
    journal.complete_home(&mut run, &claim, result).unwrap();
    let completed = journal.home_completion(&claim).unwrap().unwrap();
    realm.finish_home_claim(&claim, &completed).unwrap();
    let mut disk = Realm::beside(Some(&fixture.rewards()));
    disk.finish_home_claim(&claim, &completed).unwrap();
    assert!(!disk.settle_claim(&bank, "Host").unwrap().applied);
    assert_eq!(disk.treasury.get(Spoil::Gold), 19);
    assert_eq!(disk.home.knights.values().map(|p| p.xp).sum::<u32>(), 3);
    journal.acknowledge_home(&claim, &completed).unwrap();
    run.marks.insert("xp".into(), 7);
    let next = journal.claim_home(&mut run).unwrap().unwrap();
    disk.prepare_home_claim(&next, "Host", false, |request| request.evaluate())
        .unwrap();
    assert!(
        disk.finish_home_claim(&claim, &completed)
            .unwrap_err()
            .contains("predates")
    );
    assert_eq!(disk.treasury.get(Spoil::Gold), 19);
    assert_eq!(disk.home.knights.values().map(|p| p.xp).sum::<u32>(), 10);
}

#[test]
fn rejected_policy_results_cannot_pay_and_prepared_results_need_no_live_worker() {
    use angel_delve::rules::{Run, host_commands::Journal};
    let fixture = ClaimFixture::new();
    let mut realm = Realm::beside(Some(&fixture.rewards()));
    realm.bank("Host", &spoils(&[(Spoil::Gold, 100)]));
    realm.save().unwrap();
    let before = std::fs::read(fixture.path()).unwrap();
    let mut run = Run::at_home(3, 9, None, Default::default(), Default::default());
    run.marks.insert("xp".into(), 3);
    let mut journal = Journal::new("realm-policy-generation".into(), 9);
    let claim = journal.claim_home(&mut run).unwrap().unwrap();
    assert!(
        realm
            .prepare_home_claim(&claim, "Host", false, |_| Err("worker unavailable".into()))
            .unwrap_err()
            .contains("unavailable")
    );
    assert!(
        realm
            .prepare_home_claim(&claim, "Host", false, |request| {
                let mut result = request.evaluate()?;
                result.treasury.add(Spoil::Gold, 1000);
                result.raids_won += 1;
                Ok(result)
            })
            .unwrap_err()
            .contains("contract")
    );
    assert_eq!(std::fs::read(fixture.path()).unwrap(), before);
    assert_eq!(realm.treasury.get(Spoil::Gold), 100);
    let result = realm
        .prepare_home_claim(&claim, "Host", false, |request| {
            let mut result = request.evaluate()?;
            result.treasury.add(Spoil::Gold, 7);
            Ok(result)
        })
        .unwrap();
    assert_eq!(realm.treasury.get(Spoil::Gold), 107);
    let mut restored = Realm::beside(Some(&fixture.rewards()));
    assert_eq!(
        restored
            .prepare_home_claim(&claim, "Host", false, |_| panic!(
                "the replacement must not recalculate an already persisted result"
            ))
            .unwrap(),
        result
    );
    assert_eq!(restored.treasury.get(Spoil::Gold), 107);
    journal.complete_home(&mut run, &claim, result).unwrap();
    restored
        .finish_home_claim(&claim, &journal.home_completion(&claim).unwrap().unwrap())
        .unwrap();
    assert_eq!(restored.treasury.get(Spoil::Gold), 107);
}

#[test]
fn country_larder_rent_commits_once_with_an_uncertain_bank_claim() {
    use crate::together_shooter::{
        Pack,
        holdings::{Deed, Holding},
    };
    let fixture = ClaimFixture::new();
    let mut realm = Realm::beside(Some(&fixture.rewards()));
    realm.home.countryside.built = 1; // Bakery: one sack per rent day.
    realm.home.holdings.stores = 2;
    realm.home.holdings.owned.push(Holding {
        key: Deed::Floor(Pack::Crypt).key(),
        walls: 10,
        stars: 1,
        ..Default::default()
    });
    realm.save().unwrap();
    let claim = bank_claim(1, 19);
    storage_atomic_tests::fail_after_publish(fixture.path());
    assert!(
        realm
            .settle_claim(&claim, "Host")
            .unwrap_err()
            .contains("after publish")
    );
    assert_eq!(
        realm.home.holdings.stores, 2,
        "uncertain writer keeps its old observation"
    );
    let mut published = Realm::beside(Some(&fixture.rewards()));
    assert_eq!(published.home.holdings.larders, 1);
    assert_eq!(
        published.home.holdings.stores, 3,
        "country update precedes the rent day"
    );
    let committed = serde_json::to_value(&published.home).unwrap();
    assert!(!realm.settle_claim(&claim, "Host").unwrap().applied);
    assert_eq!(serde_json::to_value(&realm.home).unwrap(), committed);
    assert!(!published.settle_claim(&claim, "Host").unwrap().applied);
    assert_eq!(
        serde_json::to_value(&published.home).unwrap(),
        committed,
        "replay cannot issue a second sack or rent day"
    );
}

