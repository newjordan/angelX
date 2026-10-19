//! Griffinwing's stub: the checks every Round Table adventure keeps while its
//! builder fills it in (docs/ROUNDTABLE-ADVENTURES.md). The builder owns this
//! file and adds the game's own checks below these.
use super::*;

fn hold(r: &mut Run, seat: u32) {
    for _ in 0..exits::LEAVE_TICKS {
        r.step(&BTreeMap::from([(
            seat,
            Input {
                leave_held: true,
                ..Default::default()
            },
        )]));
    }
}
fn table(seats: u32) -> Run {
    let mut r = Run::griffinwing_for_test();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.step(&BTreeMap::new());
    r
}
fn press_e(r: &mut Run) {
    r.step(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
}

#[test]
fn a_session_is_bounded_and_repair_mends_it() {
    let r = Run::griffinwing_for_test();
    let mut s = r.griffinwing.clone().unwrap();
    assert!(s.valid() && s.seats.len() == 1);
    let mine = s.seats[&1].clone();
    s.seats.insert(9, mine);
    s.seats.get_mut(&1).unwrap().joined = LIMIT + 5;
    s.elapsed = LIMIT + 5;
    assert!(!s.valid());
    s.repair();
    assert!(s.valid() && !s.seats.contains_key(&9) && s.elapsed == LIMIT);
    let mut r = r;
    r.griffinwing.as_mut().unwrap().elapsed = LIMIT + 1;
    assert!(!r.valid_snapshot() && !crate::wire::drawable(&r));
}

#[test]
fn the_door_opens_at_its_tile_and_a_personal_leave_returns_to_it() {
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    r.walk_kingdom(Some(ENTRY));
    // The press that walked in is let go first, then E opens the door.
    r.step(&BTreeMap::new());
    assert!(!r.griffinwing_showing());
    press_e(&mut r);
    assert!(r.griffinwing_showing() && r.minigame_showing() && r.roundtable_showing());
    assert_eq!(r.griffinwing.as_ref().unwrap().seats.len(), 1);
    assert!(r.can_hold_leave(1) && r.can_leave_activity(1));
    assert!(r.leave_activity(1));
    assert_eq!(r.room().kind, RoomKind::Kingdom);
    assert!(r.griffinwing.is_none() && !r.minigame_showing());
    let (x, y) = (
        (ENTRY.0 as f32 + 0.5) * TILE_UNITS,
        (ENTRY.1 as f32 + 0.5) * TILE_UNITS,
    );
    let h = &r.players[&1];
    assert!(
        (h.x - x).hypot(h.y - y) < 2.1,
        "back at its tile, not the Dark Forest's door"
    );
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
    let s = r.griffinwing.as_ref().unwrap();
    assert_eq!(s.seats.len(), 3);
    assert!(
        s.seats[&3].joined > 0 && s.seats[&1].joined == 0,
        "joined mid-run, not from the start"
    );
    assert!(s.valid() && crate::wire::drawable(&r));
}

#[test]
fn six_seats_play_at_once_and_replay_byte_identically() {
    let (mut a, mut b) = (table(MAX_PLAYERS), table(MAX_PLAYERS));
    for tick in 0..HZ * 5 {
        let (ia, ib) = (a.roundtable_inputs(), b.roundtable_inputs());
        a.step(&ia);
        b.step(&ib);
        assert!(a.griffinwing.as_ref().unwrap().valid(), "tick {tick}");
    }
    assert_eq!(
        a.griffinwing.as_ref().unwrap().seats.len(),
        MAX_PLAYERS as usize
    );
    assert_eq!(
        serde_json::to_vec(&a).unwrap(),
        serde_json::to_vec(&b).unwrap()
    );
    assert_eq!(a.wire_copy().griffinwing, a.griffinwing);
}

#[test]
fn holding_leave_stands_one_seat_up_and_the_last_out_closes_the_run() {
    let mut r = table(3);
    for _ in 0..exits::LEAVE_TICKS - 1 {
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
    assert!(r.griffinwing_showing(), "friends play on");
    assert!(
        !r.view_for_seat(2).unwrap().griffinwing_showing(),
        "the leaver has no stage"
    );
    r.step(&BTreeMap::new());
    assert_eq!(
        r.griffinwing
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
    assert!(r.griffinwing.is_none() && r.exits.outside.is_empty() && !r.minigame_showing());
}

#[test]
fn a_restart_takes_well_under_a_second() {
    let mut r = table(2);
    for _ in 0..HZ * 2 {
        r.step(&BTreeMap::new());
    }
    assert!(r.leave_activity(1) && r.leave_activity(2));
    let before = r.tick;
    r.step(&BTreeMap::new());
    press_e(&mut r);
    assert!(
        r.griffinwing_showing(),
        "the same door lets the party back in"
    );
    assert!(r.tick - before < u64::from(HZ), "under a second");
    assert!(
        r.griffinwing.as_ref().unwrap().elapsed <= 1,
        "a fresh run, not the last one"
    );
}

#[test]
fn its_marks_are_crash_safe_and_its_five_numbers_are_knobs() {
    for mark in MARKS {
        assert!(
            countryside::intent_key(mark) && countryside::intent_valid(mark, 1),
            "{mark}"
        );
        assert!(mark.starts_with("griffinwing:"), "{mark}");
    }
    assert!(MARKS.contains(&CAMPAIGN_MARK));
    let mut tune = tune::Tune::default();
    let names: Vec<_> = NUMBERS.iter().map(|n| n.name).collect();
    assert_eq!(
        names,
        [
            "griffinwing_lock",
            "griffinwing_roll",
            "griffinwing_window",
            "griffinwing_spur",
            "griffinwing_tell"
        ]
    );
    for (i, n) in NUMBERS.iter().enumerate() {
        assert!(
            tune.knobs()
                .iter()
                .any(|k| k.name == n.name && k.default == f64::from(n.start)),
            "{}",
            n.name
        );
        assert!(tune.set(n.name, f64::from(n.max) + 500.0));
        assert_eq!(
            roundtable::number(&tune, SLOT, i),
            n.max,
            "held to its bound"
        );
        assert!(tune.valid() && !tune.is_default());
        assert!(tune.set(n.name, f64::from(n.start)));
    }
    assert!(tune.is_default());
}

#[test]
fn the_run_writes_nothing_while_it_is_closed_and_reads_back_open() {
    let plain = serde_json::to_string(&Run::new(3, 9, None)).unwrap();
    assert!(
        !plain.contains("\"griffinwing\""),
        "a closed run is written as today's"
    );
    let r = table(2);
    let open = serde_json::to_string(&r).unwrap();
    assert!(open.contains("\"griffinwing\":{"));
    let back: Run = serde_json::from_str(&open).unwrap();
    assert_eq!(back.griffinwing, r.griffinwing);
    assert!(back.valid_snapshot());
}

#[test]
fn a_finished_runs_mark_settles_into_the_home_once() {
    let mut r = table(2);
    r.marks.insert(CAMPAIGN_MARK.into(), 1);
    let mut home = home::Home::default();
    let mut treasury = Spoils::default();
    let reclaimed = BTreeMap::new();
    let mut realm = activity::Realm {
        home: &mut home,
        treasury: &mut treasury,
        reclaimed: &reclaimed,
        raids_won: 0,
    };
    let first = activity::settle_all(&mut realm, &r.marks);
    assert!(first.changed && realm.home.roundtable[SLOT] & BEGUN != 0);
    assert!(
        first
            .said
            .iter()
            .any(|(cue, line)| cue == "griffinwing" && line.contains(NAME))
    );
    assert!(
        !activity::settle_all(&mut realm, &r.marks).changed,
        "settled once"
    );
    assert!(
        realm
            .home
            .roundtable
            .iter()
            .enumerate()
            .all(|(i, &w)| (i == SLOT) == (w != 0)),
        "only its own word"
    );
    assert!(
        serde_json::to_string(&home::Home::default())
            .unwrap()
            .find("roundtable")
            .is_none(),
        "an untouched Home is written as today's"
    );
    assert!(
        serde_json::to_string(&home)
            .unwrap()
            .contains("\"roundtable\"")
    );
}
