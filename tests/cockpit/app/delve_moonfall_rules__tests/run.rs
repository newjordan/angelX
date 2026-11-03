//! Moonfall on a whole Run: the tests of the shared Round Table shell that tick the town.
//! They sat in `crates/delve` while its rules did; the rules are the module now, so they
//! run here, where the host's engine is.
use crate::agent::harness::live_policy::install_scripted_rules;
use angel_delve::rules::exits::LEAVE_TICKS;
use angel_delve::rules::layout::RoomKind;
use angel_delve::rules::{HZ, Input, MAX_PLAYERS, Run};
use std::collections::BTreeMap;

fn table(seats: u32) -> Run {
    install_scripted_rules();
    let mut r = Run::moonfall_for_test();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.step(&BTreeMap::new());
    r
}

fn hold(r: &mut Run, seat: u32) {
    for _ in 0..LEAVE_TICKS {
        r.step(&BTreeMap::from([(
            seat,
            Input {
                leave_held: true,
                ..Default::default()
            },
        )]));
    }
}

#[test]
fn two_seats_play_at_once_and_a_late_knight_sits_down() {
    let mut r = table(2);
    assert_eq!(r.roundtable_seated(), Some(vec![1, 2]));
    for _ in 0..HZ {
        r.step(&BTreeMap::new());
    }
    r.join_seat(3, "Knight 3");
    r.step(&BTreeMap::new());
    let s = r.moonfall.as_ref().unwrap();
    assert_eq!(s.seats.len(), 3);
    assert!(
        s.seats[&3].joined > 0 && s.seats[&1].joined == 0,
        "joined mid-run, not from the start"
    );
    assert!(s.valid() && angel_delve::wire::drawable(&r));
}


#[test]
fn six_seats_play_at_once_and_replay_byte_identically() {
    let (mut a, mut b) = (table(MAX_PLAYERS), table(MAX_PLAYERS));
    for tick in 0..HZ * 5 {
        let (ia, ib) = (a.roundtable_inputs(), b.roundtable_inputs());
        a.step(&ia);
        b.step(&ib);
        assert!(a.moonfall.as_ref().unwrap().valid(), "tick {tick}");
    }
    assert_eq!(
        a.moonfall.as_ref().unwrap().seats.len(),
        MAX_PLAYERS as usize
    );
    assert_eq!(
        serde_json::to_vec(&a).unwrap(),
        serde_json::to_vec(&b).unwrap()
    );
    assert_eq!(a.wire_copy().moonfall, a.moonfall);
}


#[test]
fn holding_leave_stands_one_seat_up_and_the_last_out_closes_the_run() {
    let mut r = table(3);
    for _ in 0..LEAVE_TICKS - 1 {
        r.step(&BTreeMap::from([(
            2,
            Input {
                leave_held: true,
                ..Default::default()
            },
        )]));
    }
    assert!(r.exits.outside.is_empty(), "fifteen ticks, not fewer");
    r.step(&BTreeMap::from([(
        2,
        Input {
            leave_held: true,
            ..Default::default()
        },
    )]));
    assert!(r.exits.outside.contains_key(&2));
    assert!(r.moonfall_showing(), "friends play on");
    assert!(
        !r.view_for_seat(2).unwrap().moonfall_showing(),
        "the leaver has no stage"
    );
    r.step(&BTreeMap::new());
    assert_eq!(
        r.moonfall
            .as_ref()
            .unwrap()
            .seats
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![1, 3]
    );
    hold(&mut r, 1);
    hold(&mut r, 3);
    assert_eq!(r.room().kind, RoomKind::Kingdom);
    assert!(r.moonfall.is_none() && r.exits.outside.is_empty() && !r.minigame_showing());
}

