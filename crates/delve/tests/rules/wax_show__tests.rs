use super::*;

fn cries(show: &WaxShow) -> Vec<Cry> {
    show.shouts.iter().map(|s| s.cry).collect()
}

#[test]
fn bites_run_into_a_combo_that_a_gap_or_a_touch_breaks() {
    let mut show = WaxShow::default();
    for k in 0..10 {
        assert_eq!(show.bite(100 + k * 5, 1, k as u16), k as u16 + 1, "bites close together run on");
    }
    assert_eq!(show.run_of(1, 150), 10);
    assert_eq!(show.run_of(1, 150 + MUNCH_GAP + 1), 0, "a gap lets the run go");
    assert_eq!(show.bite(400, 1, 3), 1, "a bite after a gap starts again");
    assert_eq!(show.best.get(&1), Some(&10));
    show.touched(402, 1, (0, 0), (16, 0), 5);
    assert_eq!(show.run_of(1, 402), 0, "a touch breaks the run");
}

#[test]
fn a_feast_is_called_once_at_each_size() {
    let mut show = WaxShow::default();
    for k in 0..FEAST[0] {
        show.bite(10 + u32::from(k) * 4, 2, k);
    }
    assert_eq!(cries(&show), vec![Cry::Feast(0)]);
    for k in FEAST[0]..FEAST[1] {
        show.bite(10 + u32::from(k) * 4, 2, k);
    }
    assert_eq!(cries(&show), vec![Cry::Feast(0), Cry::Feast(1)]);
}

#[test]
fn the_lead_is_called_when_it_changes_hands_not_when_it_starts() {
    let mut show = WaxShow::default();
    let mut eaten = BTreeMap::from([(1u32, 14u16), (2, 3)]);
    show.standings(10, &eaten, 200);
    assert!(cries(&show).is_empty(), "the first lead is not a change");
    assert_eq!(show.lead, Some(1));
    eaten.insert(2, 15);
    show.standings(20, &eaten, 190);
    assert_eq!(cries(&show), vec![Cry::Lead]);
    assert_eq!(show.shouts[0].who, 2);
    // A tie holds the lead where it was and calls nothing.
    eaten.insert(1, 15);
    show.standings(30, &eaten, 180);
    assert_eq!(show.lead, Some(2));
    assert_eq!(show.shouts.len(), 1);
}

#[test]
fn half_gone_and_the_last_bites_are_each_called_once() {
    let mut show = WaxShow { total: 100, ..Default::default() };
    let eaten = BTreeMap::from([(1u32, 0u16)]);
    show.standings(1, &eaten, 60);
    assert!(cries(&show).is_empty());
    show.standings(2, &eaten, 50);
    show.standings(3, &eaten, 49);
    assert_eq!(cries(&show), vec![Cry::Half]);
    show.standings(4, &eaten, 10);
    show.standings(5, &eaten, 9);
    assert_eq!(cries(&show), vec![Cry::Half, Cry::Last]);
    show.quicken(6);
    show.quicken(7);
    assert_eq!(show.shouts.iter().filter(|s| s.cry == Cry::Quicken).count(), 1);
}

#[test]
fn old_moments_are_let_go_and_the_lists_stay_capped() {
    let mut show = WaxShow::default();
    for k in 0..200u32 {
        show.bite(k, 1 + k % 4, (k % 90) as u16);
        show.splash(k, 1, 3);
        show.caught(k, 2, (16, 16), 1, 5, 2);
        show.touched(k, 3, (0, 0), (8, 8), 5);
    }
    assert!(show.valid());
    show.tick(10_000);
    assert!(show.pops.is_empty() && show.splashes.is_empty() && show.catches.is_empty() && show.touches.is_empty() && show.shouts.is_empty());
}

#[test]
fn a_catch_weighs_like_a_kill_and_the_fourth_like_the_winning_blow() {
    assert_eq!(catch_weight(1), Weight::Kill);
    assert_eq!(catch_weight(3), Weight::Kill);
    assert_eq!(catch_weight(4), Weight::Finale);
}

#[test]
fn the_show_round_trips_and_stays_quiet_when_nothing_happened() {
    let mut show = WaxShow::default();
    assert!(show.quiet());
    show.bite(5, 1, 7);
    show.finish(900, 1, 40, 80, true);
    assert!(!show.quiet() && show.valid());
    let back: WaxShow = serde_json::from_str(&serde_json::to_string(&show).unwrap()).unwrap();
    assert_eq!(back, show);
    assert_eq!(show.since_end(930), Some(30));
}
