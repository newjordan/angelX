//! Bannerpeak's stub: the checks every Round Table adventure keeps while its
//! builder fills it in (docs/ROUNDTABLE-ADVENTURES.md). The builder owns this
//! file and adds the game's own checks below these.
use super::*;
use super::boss;

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
    let mut r = Run::bannerpeak_for_test();
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
    let r = Run::bannerpeak_for_test();
    let mut s = r.bannerpeak.clone().unwrap();
    assert!(s.valid() && s.seats.len() == 1);
    s.seats.insert(9, Knight::at(0, 100, 100));
    s.seats.get_mut(&1).unwrap().joined = LIMIT + 5;
    s.elapsed = LIMIT + 5;
    assert!(!s.valid());
    s.repair();
    assert!(s.valid() && !s.seats.contains_key(&9) && s.elapsed == LIMIT);
    let mut r = r;
    r.bannerpeak.as_mut().unwrap().elapsed = LIMIT + 1;
    assert!(!r.valid_snapshot() && !crate::wire::drawable(&r));
}

#[test]
fn the_door_opens_at_its_tile_and_a_personal_leave_returns_to_it() {
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    r.walk_kingdom(Some(ENTRY));
    // The press that walked in is let go first, then E opens the door.
    r.step(&BTreeMap::new());
    assert!(!r.bannerpeak_showing());
    press_e(&mut r);
    assert!(r.bannerpeak_showing() && r.minigame_showing() && r.roundtable_showing());
    assert_eq!(r.bannerpeak.as_ref().unwrap().seats.len(), 1);
    assert!(r.can_hold_leave(1) && r.can_leave_activity(1));
    assert!(r.leave_activity(1));
    assert_eq!(r.room().kind, RoomKind::Kingdom);
    assert!(r.bannerpeak.is_none() && !r.minigame_showing());
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
    let s = r.bannerpeak.as_ref().unwrap();
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
        assert!(a.bannerpeak.as_ref().unwrap().valid(), "tick {tick}");
    }
    assert_eq!(
        a.bannerpeak.as_ref().unwrap().seats.len(),
        MAX_PLAYERS as usize
    );
    assert_eq!(
        serde_json::to_vec(&a).unwrap(),
        serde_json::to_vec(&b).unwrap()
    );
    assert_eq!(a.wire_copy().bannerpeak, a.bannerpeak);
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
    assert!(r.bannerpeak_showing(), "friends play on");
    assert!(
        !r.view_for_seat(2).unwrap().bannerpeak_showing(),
        "the leaver has no stage"
    );
    r.step(&BTreeMap::new());
    assert_eq!(
        r.bannerpeak
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
    assert!(r.bannerpeak.is_none() && r.exits.outside.is_empty() && !r.minigame_showing());
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
        r.bannerpeak_showing(),
        "the same door lets the party back in"
    );
    assert!(r.tick - before < u64::from(HZ), "under a second");
    assert!(
        r.bannerpeak.as_ref().unwrap().elapsed <= 1,
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
        assert!(mark.starts_with("bannerpeak:"), "{mark}");
    }
    assert!(MARKS.contains(&CAMPAIGN_MARK));
    let mut tune = tune::Tune::default();
    let names: Vec<_> = NUMBERS.iter().map(|n| n.name).collect();
    assert_eq!(
        names,
        [
            "bannerpeak_run",
            "bannerpeak_hop",
            "bannerpeak_coyote",
            "bannerpeak_slam",
            "bannerpeak_ledge"
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
        !plain.contains("\"bannerpeak\""),
        "a closed run is written as today's"
    );
    let r = table(2);
    let open = serde_json::to_string(&r).unwrap();
    assert!(open.contains("\"bannerpeak\":{"));
    let back: Run = serde_json::from_str(&open).unwrap();
    assert_eq!(back.bannerpeak, r.bannerpeak);
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
            .any(|(cue, line)| cue == "bannerpeak" && line.contains(NAME))
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

// ---------------------------------------------------------------------
// The adventure through the shell: the run, the purse, the Home.

/// A party of `seats` at the door, ready to play.
fn party(seats: u32) -> Run {
    table(seats)
}

/// Bots at the table through `Run::step` until the run has paid, at most
/// `limit` ticks: the ticks taken.
fn play_through(r: &mut Run, limit: u32) -> u32 {
    for n in 0..limit {
        let inputs = r.roundtable_inputs();
        r.step(&inputs);
        if r.bannerpeak.as_ref().is_some_and(|s| s.completed.is_some()) {
            return n;
        }
    }
    panic!("not finished in {limit} ticks");
}

#[test]
fn bots_clear_the_adventure_through_the_run_and_it_pays_each_seat_once() {
    for seats in [1u32, 3, 5] {
        let mut r = party(seats);
        let gold: Vec<u32> = (1..=seats)
            .map(|id| r.players[&id].carried.get(Spoil::Gold))
            .collect();
        assert_eq!(r.home.roundtable[SLOT] & BEGUN, 0, "nothing saved yet");
        let ticks = play_through(&mut r, 12000);
        println!("through the shell: {seats} seats, {ticks} ticks");
        for id in 1..=seats {
            assert_eq!(
                r.players[&id].carried.get(Spoil::Gold),
                gold[id as usize - 1] + FIRST_PURSE,
                "seat {id} is paid the first purse"
            );
        }
        assert!(r.home.roundtable[SLOT] & BEGUN != 0, "the Home remembers");
        assert_eq!(r.marks.get(CAMPAIGN_MARK), Some(&1));
        let record = &r.home.countryside.games.roundtable[SLOT];
        assert_eq!((record.plays, record.wins), (1, 1));
        assert!((100..=1000).contains(&record.best), "a score: {}", record.best);
        // Standing at the Great Pole a while longer pays nothing more.
        for _ in 0..90 {
            r.step(&BTreeMap::new());
        }
        for id in 1..=seats {
            assert_eq!(
                r.players[&id].carried.get(Spoil::Gold),
                gold[id as usize - 1] + FIRST_PURSE,
                "paid once"
            );
        }
        // E takes the party back to the kingdom, and the same door opens a
        // fresh run; its finish pays the smaller later purse.
        let first = r.players[&1].carried.get(Spoil::Gold);
        let all_press: BTreeMap<u32, Input> = (1..=seats)
            .map(|id| {
                (
                    id,
                    Input {
                        interact: true,
                        ..Default::default()
                    },
                )
            })
            .collect();
        r.step(&all_press);
        assert_eq!(r.room().kind, RoomKind::Kingdom);
        assert!(r.bannerpeak.is_none());
        r.step(&BTreeMap::new());
        press_e(&mut r);
        assert!(r.bannerpeak_showing(), "a fresh run");
        let s = r.bannerpeak.as_mut().unwrap();
        s.banners = 0xFF;
        s.sparks = 0xFF;
        s.boss.beat = boss::Beat::Down;
        s.boss.phase = boss::PHASES;
        play_through(&mut r, 600);
        assert_eq!(
            r.players[&1].carried.get(Spoil::Gold),
            first + REPEAT_PURSE,
            "later finishes pay less"
        );
        assert_eq!(r.home.countryside.games.roundtable[SLOT].wins, 2);
    }
}

#[test]
fn a_knight_may_retreat_mid_mountain_and_the_run_goes_on_with_a_friend() {
    let mut r = table(2);
    for _ in 0..HZ * 3 {
        let i = r.roundtable_inputs();
        r.step(&i);
    }
    let at = r.bannerpeak.as_ref().unwrap().seats[&1].body.foot();
    hold(&mut r, 2);
    assert!(r.exits.outside.contains_key(&2));
    assert_eq!(r.room().kind, RoomKind::Forest, "the party's room is still the run's");
    for _ in 0..HZ {
        let i = r.roundtable_inputs();
        r.step(&i);
    }
    let s = r.bannerpeak.as_ref().unwrap();
    assert_eq!(s.seats.keys().copied().collect::<Vec<_>>(), vec![1]);
    assert!(s.seats[&1].body.foot() != at, "the friend plays on");
    assert!(s.valid());
}

#[test]
fn a_run_stays_valid_and_on_the_wire_every_tick_of_a_party_s_adventure() {
    let mut r = table(3);
    for n in 0..HZ * 150 {
        let i = r.roundtable_inputs();
        r.step(&i);
        if n % 97 == 0 {
            let s = r.bannerpeak.as_ref().unwrap();
            assert!(s.valid(), "tick {n}");
            assert!(r.valid_snapshot() && crate::wire::drawable(&r));
            assert_eq!(r.wire_copy().bannerpeak, r.bannerpeak);
        }
    }
    // And it reads back from what it writes.
    let json = serde_json::to_string(&r).unwrap();
    let back: Run = serde_json::from_str(&json).unwrap();
    assert_eq!(back.bannerpeak, r.bannerpeak);
}

#[test]
fn the_same_keys_replay_the_same_run_byte_for_byte_over_a_long_party() {
    let (mut a, mut b) = (table(4), table(4));
    for _ in 0..HZ * 60 {
        let (ia, ib) = (a.roundtable_inputs(), b.roundtable_inputs());
        a.step(&ia);
        b.step(&ib);
    }
    assert_eq!(
        serde_json::to_vec(&a).unwrap(),
        serde_json::to_vec(&b).unwrap()
    );
    assert!(a.bannerpeak.as_ref().unwrap().elapsed > 1500);
}
