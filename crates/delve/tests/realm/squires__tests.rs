use super::*;
fn purse() -> Spoils {
    let mut p = Spoils::default();
    p.add(Spoil::Gold, 10_000);
    p
}
#[test]
fn offers_are_cadenced_and_every_type_visits() {
    let mut c = Company::default();
    let mut seen = Vec::new();
    for day in 0..5 {
        c.refresh(day);
        seen.extend(c.offers.iter().map(|s| s.kind));
        let same = c.clone();
        c.refresh(day);
        assert_eq!(c, same);
    }
    assert!(Kind::ALL.iter().all(|k| seen.contains(k)));
}
#[test]
fn treasury_hire_cap_train_and_one_active_are_atomic() {
    let mut c = Company::default();
    c.refresh(0);
    let id = c.offers[0].id;
    let before = c.clone();
    assert!(c.hire("Kay", id, &mut Spoils::default()).is_err());
    assert_eq!(c, before);
    let mut gold = purse();
    c.hire("Kay", id, &mut gold).unwrap();
    assert_eq!(c.rosters["Kay"].active, Some(id));
    assert!(c.select("Gareth", Some(id)).is_err());
    let hp = c.rosters["Kay"].squires[0].health();
    c.train("Kay", id, &mut gold).unwrap();
    assert!(c.rosters["Kay"].squires[0].health() > hp);
    c.train("Kay", id, &mut gold).unwrap();
    assert!(c.train("Kay", id, &mut gold).is_err());
    c.upgrade(Building::Train, &mut gold).unwrap();
    c.train("Kay", id, &mut gold).unwrap();
    for day in 1..3 {
        c.refresh(day);
        c.hire("Kay", c.offers[0].id, &mut gold).unwrap();
    }
    c.refresh(3);
    assert!(c.hire("Kay", c.offers[0].id, &mut gold).is_err());
    c.upgrade(Building::Hire, &mut gold).unwrap();
    c.hire("Kay", c.offers[0].id, &mut gold).unwrap();
    let other = c.rosters["Kay"].squires[1].id;
    c.select("Kay", Some(other)).unwrap();
    assert_eq!(c.rosters["Kay"].active, Some(other));
    c.select("Kay", None).unwrap();
    assert_eq!(c.rosters["Kay"].active, None);
    assert!(c.valid());
}
#[test]
fn old_and_new_books_round_trip() {
    let old: Company = serde_json::from_str("{}").unwrap();
    assert_eq!(old, Company::default());
    let mut c = old;
    c.refresh(42);
    let mut gold = purse();
    c.hire("Kay", c.offers[0].id, &mut gold).unwrap();
    for b in [Building::Recruit, Building::Hire, Building::Train] {
        c.upgrade(b, &mut gold).unwrap();
    }
    assert_eq!(
        serde_json::from_str::<Company>(&serde_json::to_string(&c).unwrap()).unwrap(),
        c
    );
}
#[test]
fn empty_board_does_not_refill_until_next_delve_and_upgrades_improve_each_step() {
    let mut c = Company::default();
    c.refresh(0);
    let mut gold = purse();
    for who in ["Kay", "Gareth", "Gawain"] {
        c.hire(who, c.offers[0].id, &mut gold).unwrap();
    }
    c.refresh(0);
    assert!(c.offers.is_empty());
    c.refresh(1);
    assert_eq!(c.offers.len(), 3);
    let old = c.offers[0].clone();
    let wage = c.wage(&old);
    let train = c.training_price(&old);
    c.upgrade(Building::Recruit, &mut gold).unwrap();
    assert_eq!(c.offers.len(), 4);
    assert!(c.offers.iter().all(|s| s.talent >= 1));
    c.upgrade(Building::Hire, &mut gold).unwrap();
    assert!(c.wage(&old) < wage);
    assert_eq!(c.cap(), 5);
    c.upgrade(Building::Train, &mut gold).unwrap();
    assert!(c.training_price(&old) < train);
    assert_eq!(c.ceiling(), 5);
}
