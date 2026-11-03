use super::*;

fn purse(gold: u32) -> Spoils { fee(gold) }
#[test]
fn recruits_are_deterministic_complete_and_refresh_on_fixed_cadence() {
    let mut g = Guild::default();
    let first = g.recruits();
    assert_eq!(first, g.recruits());
    assert_eq!(first.len(), 5);
    for kind in TYPES { assert!(first.iter().any(|s| s.kind == kind)); }
    g.advance(CADENCE - 1);
    assert_eq!(first, g.recruits());
    g.advance(CADENCE);
    assert_ne!(first, g.recruits());
    assert!(g.recruits().iter().all(|s| !s.name.is_empty() && s.vitality > 0 && s.attack > 0));
}
#[test]
fn works_are_independent_and_treasury_hiring_is_capped() {
    let mut g = Guild::default(); let mut gold = purse(10000);
    let first = g.recruits()[0].clone();
    let hire = |id| Command::Hire { knight: "Ada".into(), recruit: id };
    let before = gold.get(Spoil::Gold);
    g.apply(&hire(first.id), &mut gold).unwrap();
    assert_eq!(gold.get(Spoil::Gold), before - first.wage());
    assert!(g.apply(&hire(first.id), &mut gold).is_err());
    let id = g.recruits()[0].id; g.apply(&hire(id), &mut gold).unwrap();
    let id = g.recruits()[0].id; assert!(g.apply(&hire(id), &mut gold).is_err());
    g.apply(&Command::Upgrade(Work::Hall), &mut gold).unwrap();
    assert_eq!((g.board,g.hall,g.yard), (0,1,0));
    g.apply(&hire(id), &mut gold).unwrap();
    g.apply(&Command::Upgrade(Work::Board), &mut gold).unwrap();
    assert_eq!((g.board,g.hall,g.yard), (1,1,0));
    assert_eq!(g.capacity(), 3);
    let copy = g.clone();
    assert!(g.apply(&Command::Upgrade(Work::Yard), &mut purse(0)).is_err());
    assert_eq!(g, copy);
}
#[test]
fn training_needs_both_paid_time_and_a_delve_and_unlocks_abilities() {
    let mut g = Guild::default(); let mut gold = purse(10000);
    let id = g.recruits()[0].id;
    g.apply(&Command::Hire { knight: "Ada".into(), recruit: id }, &mut gold).unwrap();
    let start = g.rosters["Ada"].members[0].clone();
    let train = Command::Train { knight: "Ada".into(), recruit: id };
    g.apply(&train, &mut gold).unwrap();
    assert!(g.apply(&train, &mut gold).is_err());
    g.advance(100 * HZ);
    assert_eq!(g.rosters["Ada"].members[0].level, 1);
    g.apply(&Command::Depart, &mut gold).unwrap();
    let trained = g.rosters["Ada"].members[0].clone();
    assert_eq!(trained.level, 2); assert!(trained.training.is_none());
    assert!(trained.hp() > start.hp()); assert!(trained.damage() > start.damage());
    assert_ne!(trained.ability(), start.ability());
    g.apply(&Command::Upgrade(Work::Yard), &mut gold).unwrap();
    assert_eq!(g.training_ticks(&trained), 20 * HZ);
}
