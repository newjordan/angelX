//! Book Hop's show: runs, calls, lanterns and caps.
use super::*;

fn board(ids: &[(u32, u16)]) -> BTreeMap<u32, u16> {
    ids.iter().copied().collect()
}

#[test]
fn a_clean_run_calls_first_then_double_and_a_sink_breaks_it() {
    let mut s = HopShow::default();
    let mut tally = board(&[(1, 0), (2, 0)]);
    tally.insert(1, 1);
    s.crossed(10, 1, (100, 40), 1, &tally);
    assert!(s.shouts.iter().any(|c| c.cry == Cry::First && c.who == 1));
    tally.insert(1, 2);
    s.crossed(200, 1, (100, 40), 2, &tally);
    assert!(s.shouts.iter().any(|c| c.cry == Cry::Clean(2)));
    assert_eq!(s.run_of(1), 2);
    s.sank(260, 1, (100, 100), 5, false);
    assert_eq!(s.run_of(1), 0);
    assert_eq!(s.best[&1], 2);
    assert_eq!(s.sunk[&1], 1);
}

#[test]
fn the_table_lights_the_archive_only_when_every_seat_has_crossed() {
    let mut s = HopShow::default();
    let mut tally = board(&[(1, 0), (2, 0), (3, 0)]);
    for id in [1u32, 2] {
        tally.insert(id, 1);
        s.crossed(10 + id, id, (80, 30), 1, &tally);
    }
    assert_eq!(s.waves, 0, "two of three is not the table");
    // The leader crossing again lights nothing more.
    tally.insert(1, 2);
    s.crossed(50, 1, (80, 30), 2, &tally);
    assert_eq!(s.waves, 0);
    tally.insert(3, 1);
    let w = s.crossed(90, 3, (80, 30), 1, &tally);
    assert_eq!(s.waves, 1);
    assert_eq!(w, Weight::Kill, "the whole table crossing holds the room");
    assert!(s.shouts.iter().any(|c| c.cry == Cry::Lit(1)));
    assert_eq!(s.flared, Some(90));
    assert!(s.lit.is_empty(), "the lanterns go dark to be lit again");
    s.tick(90 + LIT_LIFE);
    assert_eq!(s.flared, None);
}

#[test]
fn a_lone_knight_lights_the_archive_in_threes() {
    let mut s = HopShow::default();
    let mut tally = board(&[(1, 0)]);
    for n in 1..=3u16 {
        tally.insert(1, n);
        s.crossed(u32::from(n) * 100, 1, (80, 30), n, &tally);
    }
    assert_eq!(s.waves, 1);
}

#[test]
fn the_lead_is_called_only_when_it_changes_hands() {
    let mut s = HopShow::default();
    s.standings(10, &board(&[(1, 2), (2, 0)]), 50);
    assert_eq!(s.lead, Some(1));
    assert!(s.shouts.iter().all(|c| c.cry != Cry::Lead), "the first leader is not a change");
    s.standings(20, &board(&[(1, 2), (2, 3)]), 50);
    assert!(s.shouts.iter().any(|c| c.cry == Cry::Lead && c.who == 2));
}

#[test]
fn the_clock_calls_half_and_last_once() {
    let mut s = HopShow::default();
    let b = board(&[(1, 0)]);
    s.standings(1, &b, 30);
    s.standings(2, &b, 29);
    s.standings(3, &b, 10);
    s.standings(4, &b, 9);
    let halves = s.shouts.iter().filter(|c| c.cry == Cry::Half).count();
    let lasts = s.shouts.iter().filter(|c| c.cry == Cry::Last).count();
    assert_eq!((halves, lasts), (1, 1));
}

#[test]
fn every_list_is_capped_and_the_show_stays_valid() {
    let mut s = HopShow::default();
    for k in 0..200u32 {
        s.landed(k, 1 + k % 6, (k as i32, 10), (k % 9) as u8, k % 5 == 0);
        s.sank(k, 1 + k % 6, (k as i32, 10), 4, k % 2 == 0);
        s.shout(k, Cry::Clean(2), 1);
        s.stood(1 + k % 6, (k % 14) as i32);
    }
    assert!(s.valid());
    assert!(!s.quiet());
    s.tick(10_000);
    assert!(s.lands.is_empty() && s.sinks.is_empty() && s.shouts.is_empty());
}

#[test]
fn stood_names_the_row_it_left_only_when_it_changes() {
    let mut s = HopShow::default();
    assert_eq!(s.stood(1, 12), None);
    assert_eq!(s.stood(1, 12), None);
    assert_eq!(s.stood(1, 11), Some(12));
}
