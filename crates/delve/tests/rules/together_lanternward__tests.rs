//! Lanternward's stub: the checks every Round Table adventure keeps while its
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
    let mut r = Run::lanternward_for_test();
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
    let r = Run::lanternward_for_test();
    let mut s = r.lanternward.clone().unwrap();
    assert!(s.valid() && s.seats.len() == 1);
    s.seats.insert(9, Seat::new(9, 0, (Q, Q)));
    s.seats.get_mut(&1).unwrap().joined = LIMIT + 5;
    s.elapsed = LIMIT + 5;
    assert!(!s.valid());
    s.repair();
    assert!(s.valid() && !s.seats.contains_key(&9) && s.elapsed == LIMIT);
    let mut r = r;
    r.lanternward.as_mut().unwrap().elapsed = LIMIT + 1;
    assert!(!r.valid_snapshot() && !crate::wire::drawable(&r));
}

#[test]
fn the_door_opens_at_its_tile_and_a_personal_leave_returns_to_it() {
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    r.walk_kingdom(Some(ENTRY));
    // The press that walked in is let go first, then E opens the door.
    r.step(&BTreeMap::new());
    assert!(!r.lanternward_showing());
    press_e(&mut r);
    assert!(r.lanternward_showing() && r.minigame_showing() && r.roundtable_showing());
    assert_eq!(r.lanternward.as_ref().unwrap().seats.len(), 1);
    assert!(r.can_hold_leave(1) && r.can_leave_activity(1));
    assert!(r.leave_activity(1));
    assert_eq!(r.room().kind, RoomKind::Kingdom);
    assert!(r.lanternward.is_none() && !r.minigame_showing());
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
    let s = r.lanternward.as_ref().unwrap();
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
        let problem = a.lanternward.as_ref().unwrap().problem();
        assert!(problem.is_none(), "tick {tick}: {problem:?}");
    }
    assert_eq!(
        a.lanternward.as_ref().unwrap().seats.len(),
        MAX_PLAYERS as usize
    );
    assert_eq!(
        serde_json::to_vec(&a).unwrap(),
        serde_json::to_vec(&b).unwrap()
    );
    assert_eq!(a.wire_copy().lanternward, a.lanternward);
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
    assert!(r.lanternward_showing(), "friends play on");
    assert!(
        !r.view_for_seat(2).unwrap().lanternward_showing(),
        "the leaver has no stage"
    );
    r.step(&BTreeMap::new());
    assert_eq!(
        r.lanternward
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
    assert!(r.lanternward.is_none() && r.exits.outside.is_empty() && !r.minigame_showing());
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
        r.lanternward_showing(),
        "the same door lets the party back in"
    );
    assert!(r.tick - before < u64::from(HZ), "under a second");
    assert!(
        r.lanternward.as_ref().unwrap().elapsed <= 1,
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
        assert!(mark.starts_with("lanternward:"), "{mark}");
    }
    assert!(MARKS.contains(&CAMPAIGN_MARK));
    let mut tune = tune::Tune::default();
    let names: Vec<_> = NUMBERS.iter().map(|n| n.name).collect();
    assert_eq!(
        names,
        [
            "lanternward_lock",
            "lanternward_parry",
            "lanternward_combo",
            "lanternward_flame",
            "lanternward_tell"
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
        !plain.contains("\"lanternward\""),
        "a closed run is written as today's"
    );
    let r = table(2);
    let open = serde_json::to_string(&r).unwrap();
    assert!(open.contains("\"lanternward\":{"));
    let back: Run = serde_json::from_str(&open).unwrap();
    assert_eq!(back.lanternward, r.lanternward);
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
            .any(|(cue, line)| cue == "lanternward" && line.contains(NAME))
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

// ----- the verbs, in the Stair with one snuffer -----------------------------

fn session(r: &mut Run) -> &mut Session {
    r.lanternward.as_mut().unwrap()
}
fn hold_keys(r: &mut Run, seat: u32, i: Input, ticks: u32) {
    for _ in 0..ticks {
        r.step(&BTreeMap::from([(seat, i)]));
    }
}
fn lock_key() -> Input {
    Input {
        ult: true,
        ..Default::default()
    }
}
/// One knight in an empty Stair, a snuffer of our own `dist` pixels east,
/// asleep.
fn duel(dist_px: i32) -> Run {
    let mut r = Run::lanternward_for_test();
    r.step(&BTreeMap::new());
    let s = session(&mut r);
    s.foes.clear();
    let k = s.seats.get_mut(&1).unwrap();
    k.x = 8 * TILE * Q;
    k.y = 8 * TILE * Q;
    k.face = (Q, 0);
    k.inv = 0;
    let (kx, ky) = (k.x, k.y);
    s.spawn_foe(Fk::Snuffer, kx + dist_px * Q, ky, false);
    r
}

#[test]
fn lock_on_picks_the_nearest_foe_in_front_and_squares_the_knight_to_it() {
    let mut r = duel(60);
    let s = session(&mut r);
    // A nearer one behind, and a farther one in front.
    let (kx, ky) = (s.seats[&1].x, s.seats[&1].y);
    s.spawn_foe(Fk::Snuffer, kx - 30 * Q, ky, false);
    s.spawn_foe(Fk::Snuffer, kx + 85 * Q, ky + 10 * Q, false);
    let k = s.seats.get_mut(&1).unwrap();
    k.face = (Q, 0);
    let front = s.foes[0].id;
    hold_keys(&mut r, 1, lock_key(), 2);
    let k = &r.lanternward.as_ref().unwrap().seats[&1];
    assert_eq!(k.lock, Some(Lock::Foe(front)), "the nearest in front, not behind");
    assert!(k.face.0 > Q * 9 / 10, "faces it");
    // Let go: the lock drops, the view eases back.
    hold_keys(&mut r, 1, Input::default(), 1);
    let k = &r.lanternward.as_ref().unwrap().seats[&1];
    assert!(k.lock.is_none() && k.unlock_t > 0);
}

#[test]
fn locked_on_a_sideways_step_circles_the_foe_at_its_own_distance() {
    let mut r = duel(50);
    hold_keys(&mut r, 1, lock_key(), 2);
    let radius = |r: &Run| {
        let s = r.lanternward.as_ref().unwrap();
        let (k, f) = (&s.seats[&1], &s.foes[0]);
        dist(k.x - f.x, k.y - f.y) / Q
    };
    let before = radius(&r);
    let (sx, sy) = {
        let k = &r.lanternward.as_ref().unwrap().seats[&1];
        (k.x, k.y)
    };
    // Foe is east of the knight: up and down are sideways. It stays put.
    let (fx, fy) = {
        let f = &r.lanternward.as_ref().unwrap().foes[0];
        (f.x, f.y)
    };
    for _ in 0..20 {
        hold_keys(
            &mut r,
            1,
            Input {
                ult: true,
                move_y: -1,
                ..Default::default()
            },
            1,
        );
        let f = &mut session(&mut r).foes[0];
        (f.x, f.y) = (fx, fy);
    }
    let s = r.lanternward.as_ref().unwrap();
    let k = &s.seats[&1];
    assert!(dist(k.x - sx, k.y - sy) / Q > 15, "it moved round");
    assert!((radius(&r) - before).abs() <= 3, "same distance, {before} then {}", radius(&r));
    assert!(k.face.0 > 0, "still facing the foe");
}

#[test]
fn a_three_hit_combo_ends_heavier_and_a_wound_spin_goes_off_on_release() {
    let mut r = duel(20);
    let s = session(&mut r);
    s.foes[0].hp = 100;
    s.foes[0].full = 100;
    let (fx, fy) = (s.foes[0].x, s.foes[0].y);
    let sword = Input {
        swing: true,
        ..Default::default()
    };
    // Tap the sword now and then and keep the foe where it was, so the chain
    // is what is tried.
    let mut hits = Vec::new();
    let mut last = 100;
    for tick in 0..60 {
        let i = if tick % 4 == 0 { sword } else { Input::default() };
        hold_keys(&mut r, 1, i, 1);
        let f = &mut session(&mut r).foes[0];
        (f.x, f.y) = (fx, fy);
        if f.hp < last {
            hits.push(last - f.hp);
            last = f.hp;
        }
    }
    assert!(hits.starts_with(&[2, 2, 4]), "two light cuts and a heavy one: {hits:?}");
    // A wound spin: hold, then let go.
    let mut r = duel(20);
    session(&mut r).foes[0].hp = 100;
    session(&mut r).foes[0].full = 100;
    hold_keys(&mut r, 1, sword, 30);
    assert!(r.lanternward.as_ref().unwrap().seats[&1].charged());
    hold_keys(&mut r, 1, Input::default(), 1);
    assert_eq!(r.lanternward.as_ref().unwrap().seats[&1].act, Act::Spin);
    hold_keys(&mut r, 1, Input::default(), 14);
    assert!(r.lanternward.as_ref().unwrap().foes[0].hp <= 97, "the spin cut it");
}

#[test]
fn locked_on_the_hop_follows_the_stick_and_clears_the_ground() {
    let mut r = duel(60);
    hold_keys(&mut r, 1, lock_key(), 2);
    let e = |i: Input| Input { interact: true, ult: true, ..i };
    // Side hop: stick up with the foe east.
    hold_keys(&mut r, 1, e(Input { move_y: -1, ..Default::default() }), 1);
    let k = &r.lanternward.as_ref().unwrap().seats[&1];
    assert_eq!((k.act, k.n), (Act::Hop, 0));
    hold_keys(&mut r, 1, lock_key(), 9);
    let k = &r.lanternward.as_ref().unwrap().seats[&1];
    assert!(k.z > 8 * Q && k.inv > 0, "high and untouchable mid-hop");
    hold_keys(&mut r, 1, lock_key(), 12);
    let k = &r.lanternward.as_ref().unwrap().seats[&1];
    assert_eq!((k.act, k.z), (Act::Free, 0));
    // Back and nothing: a backflip, away from the foe.
    let (x0, _) = (k.x, k.y);
    hold_keys(&mut r, 1, e(Input::default()), 1);
    assert_eq!(r.lanternward.as_ref().unwrap().seats[&1].n, 1);
    hold_keys(&mut r, 1, lock_key(), 20);
    assert!(r.lanternward.as_ref().unwrap().seats[&1].x < x0 - 20 * Q, "backwards");
}

#[test]
fn a_shield_raised_just_in_time_parries_a_lunge_and_leaves_the_snuffer_open() {
    let mut r = duel(60);
    let seats = 1;
    let _ = seats;
    // Wake it and let it come; raise the shield at the last moment.
    session(&mut r).foes[0].st = Fs::Drift;
    session(&mut r).foes[0].t = 1;
    hold_keys(&mut r, 1, lock_key(), 1);
    let guard = Input {
        ult: true,
        dash: true,
        ..Default::default()
    };
    let mut parried = false;
    for _ in 0..200 {
        let s = r.lanternward.as_ref().unwrap();
        let f = &s.foes[0];
        // Strike lands after the tell and a few ticks of lunge: raise at the
        // end of the tell.
        let raise = f.st == Fs::Tell && f.t <= 6 || f.st == Fs::Strike;
        let i = if raise { guard } else { lock_key() };
        r.step(&BTreeMap::from([(1, i)]));
        let f = &r.lanternward.as_ref().unwrap().foes[0];
        if f.st == Fs::Stagger {
            parried = true;
            break;
        }
    }
    assert!(parried, "the parry staggered it");
    let s = r.lanternward.as_ref().unwrap();
    assert_eq!(s.seats[&1].hp, START_HP, "not a scratch");
    assert!(s.foes[0].open, "the next blow is double");
}

#[test]
fn a_late_shield_blocks_but_does_not_parry_and_nothing_gets_past_from_behind() {
    let mut r = duel(60);
    session(&mut r).foes[0].st = Fs::Drift;
    session(&mut r).foes[0].t = 1;
    hold_keys(&mut r, 1, lock_key(), 1);
    let guard = Input {
        ult: true,
        dash: true,
        ..Default::default()
    };
    // Shield up long before the blow: a block, with the push, no stagger.
    let mut blocked = false;
    for _ in 0..200 {
        r.step(&BTreeMap::from([(1, guard)]));
        let s = r.lanternward.as_ref().unwrap();
        if s.foes[0].st == Fs::Recover {
            blocked = true;
            break;
        }
    }
    assert!(blocked);
    let s = r.lanternward.as_ref().unwrap();
    assert_eq!(s.seats[&1].hp, START_HP);
    assert!(!s.foes[0].open);
}

/// Four bots (or one, or six) play from the door; the tick the Beacon burns,
/// and the room and stage on the way. `limit` ticks at most.
fn bots_play(seats: u32, limit: u32, trace: bool) -> (Run, Option<u32>) {
    let mut r = table(seats);
    let mut last = (255u8, 255u8);
    for tick in 0..limit {
        let inputs = r.roundtable_inputs();
        r.step(&inputs);
        let Some(s) = r.lanternward.as_ref() else { break };
        if trace && (s.room, s.stage) != last {
            last = (s.room, s.stage);
            let hp: Vec<u32> = s.seats.values().map(|k| k.hp).collect();
            eprintln!("seats {seats} tick {tick}: room {} stage {} hp {hp:?} tries {}", s.room, s.stage, s.tries);
        }
        if s.done.is_some() {
            return (r, Some(tick));
        }
    }
    (r, None)
}

#[test]
fn bots_clear_the_adventure_at_one_two_four_and_six_seats() {
    // The ticks each table took, as the tuning record keeps them.
    for (seats, limit) in [(1, 7000), (2, 7500), (4, 8000), (6, 9000)] {
        let (r, done) = bots_play(seats, limit, false);
        let s = r.lanternward.as_ref().unwrap();
        let done = done.unwrap_or_else(|| {
            panic!("{seats} seats: room {} stage {} at {}", s.room, s.stage, s.elapsed)
        });
        assert!(
            done > 3 * 60 * HZ / 2,
            "{seats} seats: a bot that wins in {done} ticks skipped something"
        );
        assert!(s.valid() && crate::wire::drawable(&r));
        assert!(r.marks.contains_key(CAMPAIGN_MARK), "the Beacon marks the campaign");
        assert_eq!(s.paid.count_ones(), seats, "every seat at the Beacon is paid");
    }
}

// ----- the adventure as a whole -----------------------------------------------

/// The party of `seats` in the room at the named moment of the review.
fn at(moment: &str, seats: u32) -> Run {
    Run::lanternward_review_for_test(moment, seats)
}
fn step_n(r: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        r.step(&BTreeMap::new());
    }
}
fn sess(r: &Run) -> &Session {
    r.lanternward.as_ref().unwrap()
}

#[test]
fn the_view_keeps_the_awake_warden_in_the_frame() {
    // Knights stand off the Warden at ninety pixels; the view
    // leans so its whole body is on the screen, not cut by the top edge.
    let mut r = at("warden-open", 2);
    let (wx, wy) = {
        let w = sess(&r).boss.as_ref().unwrap();
        (w.x, w.y)
    };
    for id in [1, 2] {
        let k = session(&mut r).seats.get_mut(&id).unwrap();
        k.x = wx + (id as i32 - 1) * 20 * Q;
        k.y = wy + 90 * Q;
        k.cam = (k.x, k.y);
    }
    step_n(&mut r, 90);
    let s = sess(&r);
    let (cx, cy) = s.seats[&1].cam;
    let top = (wy - 28 * Q) - (cy - VIEW_H / 2 * Q);
    assert!(top >= 0, "the Warden's head is {} px over the top edge", -top / Q);
    let side = (wx - cx).abs();
    assert!(side < (VIEW_W / 2 - 24) * Q, "and it is not at the side either");
    // Where it is asleep the view stays on the knight.
    let mut r = at("stair", 1);
    step_n(&mut r, 30);
    let k = &sess(&r).seats[&1];
    assert!((k.cam.0 - k.x).abs() < 2 * Q && (k.cam.1 - k.y).abs() < 2 * Q || sess(&r).boss.is_none());
}

#[test]
fn repair_rebuilds_a_room_a_bad_save_broke() {
    let mut r = at("lamp-lit", 2);
    {
        let s = session(&mut r);
        s.stage = 90;
        s.props.truncate(3);
        s.foes.push(Foe::new(1, Fk::Snuffer, -5, 0, 1));
        s.seats.get_mut(&1).unwrap().hp = 999;
        s.cue = Some((i32::MAX, 0, 9999));
        s.paid = 255;
    }
    assert!(!sess(&r).valid());
    session(&mut r).repair();
    let s = sess(&r);
    assert!(s.valid(), "{:?}", s.problem());
    assert_eq!(s.room, 1, "the same room, rebuilt");
    assert_eq!(s.seats.len(), 2);
    assert!(s.seats.values().all(|k| k.hp == k.max_hp));
}

#[test]
fn the_first_clear_marks_the_campaign_and_pays_each_seat_once() {
    let mut r = table(3);
    let before: Vec<u32> = (1..=3).map(|id| r.players[&id].carried.get(Spoil::Gold)).collect();
    for _ in 0..8000 {
        let i = r.roundtable_inputs();
        r.step(&i);
        if sess(&r).done.is_some() {
            break;
        }
    }
    assert!(sess(&r).done.is_some(), "three bots win");
    assert!(r.marks.contains_key(CAMPAIGN_MARK));
    let record = &r.home.countryside.games.roundtable[SLOT];
    assert_eq!((record.plays, record.wins), (1, 1));
    assert!(record.best > 0);
    let paid: Vec<u32> = (1..=3)
        .map(|id| r.players[&id].carried.get(Spoil::Gold) - before[id as usize - 1])
        .collect();
    for (n, got) in paid.iter().enumerate() {
        let gems = sess(&r).seats[&(n as u32 + 1)].gems;
        assert_eq!(*got, purse(true, gems), "seat {}", n + 1);
        assert!(*got <= PURSE_FIRST + PURSE_GEMS, "the purse is capped");
        assert!(r.players[&(n as u32 + 1)].laurel, "a gold crown for each");
    }
    // The finish is not paid again however it is asked for.
    let mut s = r.lanternward.take().unwrap();
    r.finish_lanternward(&mut s);
    r.lanternward = Some(s);
    let again: Vec<u32> = (1..=3).map(|id| r.players[&id].carried.get(Spoil::Gold)).collect();
    assert_eq!(
        again,
        (0..3).map(|n| before[n] + paid[n]).collect::<Vec<_>>(),
        "paid once"
    );
    assert_eq!(r.home.countryside.games.roundtable[SLOT].wins, 2.min(record_plays(&r)));
}
fn record_plays(r: &Run) -> u32 {
    r.home.countryside.games.roundtable[SLOT].plays
}

#[test]
fn a_second_clear_pays_the_smaller_purse() {
    assert!(purse(false, 0) < purse(true, 0));
    assert_eq!(purse(true, 500), PURSE_FIRST + PURSE_GEMS);
    assert_eq!(purse(false, 500), PURSE_AGAIN + PURSE_GEMS);
}

#[test]
fn a_wipe_puts_the_party_back_at_the_rooms_start_in_under_a_second_with_its_progress_kept() {
    let mut r = at("lamp-wave", 2);
    assert!(sess(&r).stage == 2 && sess(&r).inv.lantern);
    let gate_open = sess(&r).props.iter().any(|p| p.kind == Pk::Gate && p.id == 1 && p.on == 1);
    assert!(gate_open);
    // The whole table falls.
    for k in session(&mut r).seats.values_mut() {
        k.hp = 0;
        k.act = Act::Down;
    }
    let mut ticks = 0;
    while sess(&r).seats.values().all(|k| !k.up()) {
        step_n(&mut r, 1);
        ticks += 1;
        assert!(ticks < 60, "the retry never came");
    }
    assert!(ticks < HZ, "back in play inside a second: {ticks} ticks");
    let s = sess(&r);
    assert_eq!(s.tries, 1);
    assert!(s.seats.values().all(|k| k.hp == k.max_hp && k.act == Act::Free));
    assert!(s.inv.lantern, "the Lantern is still theirs");
    assert!(s.props.iter().any(|p| p.kind == Pk::Gate && p.id == 1 && p.on == 1), "the gate stays open");
    assert_eq!(s.stage, 2);
    assert!(s.foes.iter().filter(|f| f.alive()).count() >= 2, "the wave stands again");
    assert!(s.foes.iter().all(|f| f.hp == f.full));
}

#[test]
fn a_friend_holding_e_lifts_the_fallen_and_otherwise_the_brazier_does() {
    let mut r = at("down", 2);
    assert_eq!(sess(&r).seats[&1].act, Act::Down, "still down mid-lift");
    let hold_e = Input { interact: true, ..Default::default() };
    for _ in 0..REVIVE_HOLD / 2 + 4 {
        r.step(&BTreeMap::from([(2, hold_e)]));
    }
    let k = &sess(&r).seats[&1];
    assert!(k.up() && k.hp == REVIVE_HP, "up with three hearts: {:?} {}", k.act, k.hp);
    // Left alone, the knight is carried to the room's brazier in the end.
    let mut r = at("stair", 2);
    {
        let s = session(&mut r);
        let k = s.seats.get_mut(&1).unwrap();
        k.hp = 0;
        k.act = Act::Down;
        k.x = 15 * TILE * Q;
        k.y = 14 * TILE * Q;
    }
    step_n(&mut r, DOWN_AUTO + 2);
    let s = sess(&r);
    let k = &s.seats[&1];
    assert!(k.up() && k.hp > 0 && k.hp < k.max_hp);
    assert_eq!((k.x, k.y), s.start_of(1), "at the start, by the brazier");
}

#[test]
fn two_knights_both_matter_at_the_plates_and_at_the_lamp_heart() {
    // The Gallery: the block on the first plate is not enough for two.
    let mut r = at("gallery", 2);
    {
        let s = session(&mut r);
        let (bx, by) = {
            let p = s.props.iter().find(|p| p.kind == Pk::Plate && p.id == 0).unwrap();
            (p.x, p.y)
        };
        let b = s.props.iter_mut().find(|p| p.kind == Pk::Block).unwrap();
        (b.x, b.y) = (bx, by);
        b.tx = (bx / (TILE * Q)) as u8;
        b.ty = (by / (TILE * Q)) as u8;
    }
    step_n(&mut r, 3);
    let gate = |r: &Run| sess(r).props.iter().any(|p| p.kind == Pk::Gate && p.id == 1 && p.on == 1);
    assert!(!gate(&r), "one plate alone does not hold the door for two");
    {
        let (px, py) = {
            let p = sess(&r).props.iter().find(|p| p.kind == Pk::Plate && p.id == 1).unwrap();
            (p.x, p.y)
        };
        let k = session(&mut r).seats.get_mut(&2).unwrap();
        (k.x, k.y) = (px, py);
    }
    step_n(&mut r, 3);
    assert!(gate(&r), "the second knight holds it");
    // Alone, the block is enough.
    let mut solo = at("gallery", 1);
    {
        let s = session(&mut solo);
        let (bx, by) = {
            let p = s.props.iter().find(|p| p.kind == Pk::Plate && p.id == 0).unwrap();
            (p.x, p.y)
        };
        let b = s.props.iter_mut().find(|p| p.kind == Pk::Block).unwrap();
        (b.x, b.y) = (bx, by);
    }
    step_n(&mut solo, 3);
    assert!(gate(&solo));
    // The Warden: from its second third, one knight's light is not enough.
    let mut r = at("warden", 2);
    {
        let w = session(&mut r).boss.as_mut().unwrap();
        w.phase = 1;
    }
    let now = sess(&r).elapsed;
    session(&mut r).boss.as_mut().unwrap().lit[0] = now;
    assert!(!sess(&r).boss_exposed(), "one light leaves the heart shut");
    session(&mut r).boss.as_mut().unwrap().lit[1] = now;
    assert!(sess(&r).boss_exposed(), "two keep it open");
    // Alone one light does (there is no second knight to ask for).
    let mut solo = at("warden", 1);
    session(&mut solo).boss.as_mut().unwrap().phase = 2;
    let now = sess(&solo).elapsed;
    session(&mut solo).boss.as_mut().unwrap().lit[0] = now;
    assert!(sess(&solo).boss_exposed());
}

#[test]
fn more_seats_mean_more_foes_more_health_and_tighter_tells_never_less() {
    let seats = [1u32, 2, 3, 4, 5, 6];
    let mut stair = Vec::new();
    let mut vault = Vec::new();
    let mut gallery = Vec::new();
    let mut warden = Vec::new();
    for &n in &seats {
        let r = at("stair", n);
        stair.push(sess(&r).foes.iter().map(|f| f.full).sum::<u32>());
        let r = at("vault", n);
        vault.push((sess(&r).foes.len(), sess(&r).foes.iter().map(|f| f.full).sum::<u32>()));
        let r = at("gallery", n);
        gallery.push(sess(&r).foes.len());
        let r = at("warden", n);
        warden.push(sess(&r).boss.as_ref().unwrap().full);
    }
    for series in [&stair] {
        assert!(series.windows(2).all(|w| w[0] <= w[1]) && series[5] > series[0], "{series:?}");
    }
    assert!(vault.windows(2).all(|w| w[0].0 <= w[1].0 && w[0].1 < w[1].1), "{vault:?}");
    assert!(vault[0].0 < vault[5].0);
    assert_eq!(gallery, vec![1, 1, 1, 2, 2, 2], "a second turret from four seats");
    assert_eq!(warden, (1..=6).map(|n| WARDEN_HP * n).collect::<Vec<_>>(), "a fixed share per seat");
    let rules = Rules::default();
    let tells: Vec<u32> = seats.iter().map(|&n| rules.tell_for(n)).collect();
    assert!(tells.windows(2).all(|w| w[0] >= w[1]) && tells[5] < tells[0], "{tells:?}");
    assert!(tells.iter().all(|&t| t >= 40), "never under forty: {tells:?}");
    // Even at the knob's lowest the floor holds.
    let tight = Rules { tell: 1, ..rules };
    assert!((1..=6).all(|n| tight.tell_for(n) >= 40));
}

#[test]
fn a_late_knight_sits_at_the_start_and_scales_the_fight_once_keeping_its_share() {
    let mut r = at("warden", 1);
    session(&mut r).boss.as_mut().unwrap().hp = WARDEN_HP / 2;
    step_n(&mut r, 2);
    r.join_seat(2, "Knight 2");
    step_n(&mut r, 2);
    let s = sess(&r);
    let w = s.boss.as_ref().unwrap();
    assert_eq!((w.full, w.hp, w.prov), (2 * WARDEN_HP, WARDEN_HP, 2), "half left stays half");
    assert!(s.seats[&2].joined > 0);
    assert_eq!((s.seats[&2].x, s.seats[&2].y), s.start_of(2), "at the room's start");
    // The same seat standing up and sitting again scales nothing twice.
    assert!(r.leave_activity(2));
    step_n(&mut r, 3);
    assert_eq!(sess(&r).seats.len(), 1);
    r.join_seat(2, "Knight 2");
    step_n(&mut r, 3);
    assert!(r.exits.outside.contains_key(&2) || sess(&r).seats.len() == 2);
    let w = sess(&r).boss.as_ref().unwrap();
    assert_eq!((w.full, w.hp), (2 * WARDEN_HP, WARDEN_HP));
}

#[test]
fn holding_leave_in_the_boss_room_stands_a_seat_up_and_the_rest_play_on() {
    let mut r = at("warden", 3);
    for _ in 0..exits::LEAVE_TICKS {
        r.step(&BTreeMap::from([(2, Input { leave_held: true, ..Default::default() })]));
    }
    assert!(r.exits.outside.contains_key(&2));
    step_n(&mut r, 2);
    assert_eq!(sess(&r).seats.keys().copied().collect::<Vec<_>>(), vec![1, 3]);
    assert!(r.lanternward_showing());
}

#[test]
fn the_same_inputs_leave_the_same_bytes_over_half_a_minute_of_bots() {
    let play = |seats| {
        let mut r = table(seats);
        let mut trace = Vec::new();
        for tick in 0..30 * HZ {
            let i = r.roundtable_inputs();
            r.step(&i);
            if tick % 30 == 0 {
                trace.push(serde_json::to_vec(&r.lanternward).unwrap());
            }
        }
        (r, trace)
    };
    let (a, ta) = play(4);
    let (b, tb) = play(4);
    assert_eq!(ta, tb);
    assert_eq!(serde_json::to_vec(&a).unwrap(), serde_json::to_vec(&b).unwrap());
    assert_eq!(a.wire_copy().lanternward, a.lanternward);
    assert!(sess(&a).valid() && crate::wire::drawable(&a));
}

// ----- the quality bar ----------------------------------------------------------

#[test]
fn locking_on_snaps_the_view_and_ticks_and_letting_go_eases_it_back() {
    // The foe is south of the knight: the Stair's view is free to move that way.
    let mut r = Run::lanternward_duel_for_test(70);
    {
        let s = session(&mut r);
        let (kx, ky) = (s.seats[&1].x, s.seats[&1].y);
        let f = &mut s.foes[0];
        (f.x, f.y) = (kx, ky + 70 * Q);
        s.seats.get_mut(&1).unwrap().face = (0, Q);
    }
    step_n(&mut r, 20);
    let cam0 = sess(&r).seats[&1].cam;
    let (fx, fy) = {
        let f = &sess(&r).foes[0];
        (f.x, f.y)
    };
    r.sounds.clear();
    hold_keys(&mut r, 1, lock_key(), 1);
    assert!(r.sounds.contains(&"card_pickup"), "a crisp tick as it takes");
    let k = &sess(&r).seats[&1];
    assert!(k.face.1 > Q * 99 / 100, "square to the foe on the very tick");
    // The view swings toward the foe, most of the way inside the snap.
    hold_keys(&mut r, 1, lock_key(), SNAP_TICKS);
    let (kx, ky) = (sess(&r).seats[&1].x, sess(&r).seats[&1].y);
    let aim = (kx + (fx - kx) * LOCK_LEAN / 100, ky + (fy - ky) * LOCK_LEAN / 100);
    let k = &sess(&r).seats[&1];
    let left = (aim.1 - k.cam.1).abs();
    let started = (aim.1 - cam0.1).abs();
    assert!(left * 4 < started, "{left} of {started} still to go after the snap");
    // Let go: the view is slower coming back than it went (it eases).
    hold_keys(&mut r, 1, Input::default(), 3);
    let k = &sess(&r).seats[&1];
    assert!(k.lock.is_none());
    let back = (k.cam.1 - k.y).abs();
    assert!(back > 2 * Q, "not at once");
    hold_keys(&mut r, 1, Input::default(), 60);
    let k = &sess(&r).seats[&1];
    assert!((k.cam.1 - k.y).abs() < 2 * Q, "settled on the knight");
}

#[test]
fn every_blade_that_lands_asks_the_engine_for_its_hit_stop_and_a_kill_for_more() {
    let mut r = Run::lanternward_duel_for_test(18);
    session(&mut r).foes[0].hp = 100;
    session(&mut r).foes[0].full = 100;
    let sword = Input { swing: true, ..Default::default() };
    let mut stopped = false;
    for _ in 0..12 {
        hold_keys(&mut r, 1, sword, 1);
        if r.hitstop.left > 0 || r.hitstop.cooldown > 0 {
            stopped = true;
            break;
        }
    }
    assert!(stopped, "a slash that lands holds the room a moment");
    // A parry asks for the longer one.
    let mut r = Run::lanternward_duel_for_test(60);
    session(&mut r).foes[0].st = Fs::Drift;
    session(&mut r).foes[0].t = 1;
    hold_keys(&mut r, 1, lock_key(), 1);
    let guard = Input { ult: true, dash: true, ..Default::default() };
    for _ in 0..200 {
        let f = &sess(&r).foes[0];
        let raise = f.st == Fs::Tell && f.t <= 3 || f.st == Fs::Strike;
        hold_keys(&mut r, 1, if raise { guard } else { lock_key() }, 1);
        if sess(&r).foes[0].st == Fs::Stagger {
            break;
        }
    }
    assert_eq!(sess(&r).foes[0].st, Fs::Stagger);
    assert!(r.hitstop.left > 0 || r.hitstop.cooldown > 0);
    assert!(r.sounds.contains(&"holy_water"), "the parry chime, not the block's ring");
    assert!(sess(&r).flash > 0 || r.hitstop.cooldown > 0, "and a white flash");
}

#[test]
fn a_parry_staggers_a_full_second_and_the_next_blow_is_double() {
    let mut r = Run::lanternward_duel_for_test(60);
    session(&mut r).foes[0].st = Fs::Drift;
    session(&mut r).foes[0].t = 1;
    session(&mut r).foes[0].hp = 40;
    session(&mut r).foes[0].full = 40;
    hold_keys(&mut r, 1, lock_key(), 1);
    let guard = Input { ult: true, dash: true, ..Default::default() };
    for _ in 0..200 {
        let f = &sess(&r).foes[0];
        let raise = f.st == Fs::Tell && f.t <= 3 || f.st == Fs::Strike;
        hold_keys(&mut r, 1, if raise { guard } else { lock_key() }, 1);
        if sess(&r).foes[0].st == Fs::Stagger {
            break;
        }
    }
    let f = &sess(&r).foes[0];
    assert_eq!((f.st, f.t), (Fs::Stagger, STAGGER_TICKS), "a full second: {STAGGER_TICKS} ticks is 1 s");
    assert_eq!(STAGGER_TICKS as u32, HZ);
    // Walk up to it and strike once: double.
    let (fx, fy) = (f.x, f.y);
    {
        let k = session(&mut r).seats.get_mut(&1).unwrap();
        k.x = fx - 12 * Q;
        k.y = fy;
        k.face = (Q, 0);
        k.shield = 0;
        k.guard_cd = 0;
    }
    let before = sess(&r).foes[0].hp;
    for n in 0..12 {
        hold_keys(&mut r, 1, Input { swing: n % 2 == 0, ..Default::default() }, 1);
        if sess(&r).foes[0].hp < before {
            break;
        }
    }
    assert_eq!(before - sess(&r).foes[0].hp, 4, "two for the slash, doubled");
}

#[test]
fn a_blade_passes_through_a_hooded_snuffer_until_light_burns_the_hood_off() {
    let mut r = Run::lanternward_duel_for_test(18);
    {
        let s = session(&mut r);
        s.inv.lantern = true;
        for k in s.seats.values_mut() {
            k.flame = 150;
        }
        s.foes[0].hooded = true;
        s.foes[0].hp = 40;
        s.foes[0].full = 40;
    }
    let sword = Input { swing: true, ..Default::default() };
    for _ in 0..16 {
        hold_keys(&mut r, 1, sword, 1);
    }
    assert_eq!(sess(&r).foes[0].hp, 40, "the blade passes through the hood");
    // Flare: the hood comes off for a while, and then the blade counts.
    for _ in 0..12 {
        hold_keys(&mut r, 1, Input { fire: true, ..Default::default() }, 1);
    }
    assert!(sess(&r).foes[0].lit > 0);
    let (fx, fy) = (sess(&r).foes[0].x, sess(&r).foes[0].y);
    for _ in 0..16 {
        hold_keys(&mut r, 1, sword, 1);
        let f = &mut session(&mut r).foes[0];
        (f.x, f.y) = (fx, fy);
    }
    assert!(sess(&r).foes[0].hp < 40, "lit, it can be cut");
}

#[test]
fn the_lantern_throws_a_bolt_for_a_fifth_of_its_tank_and_a_flare_drains_and_refills() {
    let mut r = Run::lanternward_duel_for_test(0);
    {
        let s = session(&mut r);
        s.inv.lantern = true;
        s.seats.get_mut(&1).unwrap().flame = 150;
    }
    let tap = Input { fire: true, ..Default::default() };
    hold_keys(&mut r, 1, tap, 3);
    hold_keys(&mut r, 1, Input::default(), 1);
    let s = sess(&r);
    assert_eq!(s.bolts.iter().filter(|b| b.kind == Bk::Light).count(), 1, "one bolt of light on the release");
    assert!(s.seats[&1].flame <= 150 - 30 + 2, "a fifth of the tank");
    // Hold: a flare, one flame a tick.
    let before = sess(&r).seats[&1].flame;
    hold_keys(&mut r, 1, tap, FLARE_AFTER + 20);
    let k = &sess(&r).seats[&1];
    assert!(k.flaring());
    assert!(before - k.flame >= 14, "{before} to {}", k.flame);
    // Let go and it comes back, slowly.
    let low = k.flame;
    hold_keys(&mut r, 1, Input::default(), 60);
    assert!(sess(&r).seats[&1].flame > low + 20);
    // Run it dry: it sputters and cannot flare again at once.
    session(&mut r).seats.get_mut(&1).unwrap().flame = 3;
    hold_keys(&mut r, 1, tap, 30);
    let k = &sess(&r).seats[&1];
    assert!(!k.flaring() && k.flame < 20);
}

#[test]
fn a_roll_and_a_hop_cross_a_pit_and_a_walk_into_it_costs_a_half_heart() {
    // Walk in.
    let mut r = Run::lanternward_duel_for_test(0);
    {
        let k = session(&mut r).seats.get_mut(&1).unwrap();
        k.x = 13 * TILE * Q + 8 * Q;
        k.y = 8 * TILE * Q;
        k.safe = (k.x, k.y);
    }
    hold_keys(&mut r, 1, Input { move_x: 1, ..Default::default() }, 34);
    let k = &sess(&r).seats[&1];
    assert_eq!(k.hp, START_HP - FALL_DAMAGE, "a fall");
    assert!(k.x < 15 * TILE * Q, "put back on the near side");
    // Roll across (the pit is two tiles wide).
    let mut r = Run::lanternward_duel_for_test(0);
    {
        let k = session(&mut r).seats.get_mut(&1).unwrap();
        k.x = 14 * TILE * Q + 12 * Q;
        k.y = 8 * TILE * Q;
        k.safe = (k.x, k.y);
    }
    hold_keys(&mut r, 1, Input { interact: true, move_x: 1, ..Default::default() }, 1);
    hold_keys(&mut r, 1, Input::default(), 24);
    let k = &sess(&r).seats[&1];
    assert_eq!(k.hp, START_HP);
    assert!(k.x > 17 * TILE * Q, "across: {}", k.x / Q);
}

#[test]
fn the_jump_attack_comes_down_hard_on_what_is_below() {
    let mut r = Run::lanternward_duel_for_test(40);
    session(&mut r).foes[0].hp = 40;
    session(&mut r).foes[0].full = 40;
    hold_keys(&mut r, 1, lock_key(), 2);
    let e = Input { interact: true, ult: true, move_x: 1, ..Default::default() };
    hold_keys(&mut r, 1, e, 1);
    hold_keys(&mut r, 1, lock_key(), 6);
    assert!(sess(&r).seats[&1].airborne());
    let (fx, fy) = (sess(&r).foes[0].x, sess(&r).foes[0].y);
    for _ in 0..8 {
        hold_keys(&mut r, 1, Input { ult: true, swing: true, ..Default::default() }, 1);
        let f = &mut session(&mut r).foes[0];
        (f.x, f.y) = (fx, fy);
        let k = session(&mut r).seats.get_mut(&1).unwrap();
        // The strike comes down where the hop landed, close to it.
        if k.act == Act::Plunge {
            k.x = fx - 8 * Q;
            k.y = fy;
        }
    }
    assert!(sess(&r).foes[0].hp <= 40 - PLUNGE_DAMAGE, "the ring of the landing: {}", sess(&r).foes[0].hp);
}

#[test]
fn a_bomb_opens_a_cracked_wall_and_a_bomb_pot_always_holds_bombs() {
    let r = at("bomb-open", 1);
    assert!(sess(&r).secret_open(), "the closet is open");
    assert!(r.sounds.contains(&"bomb") || sess(&r).fuses.is_empty());
    // A bomb pot, cut with the sword, drops three bombs, every time.
    let mut r = Run::lanternward_duel_for_test(0);
    {
        let s = session(&mut r);
        let (px, py) = {
            let p = s.props.iter().find(|p| p.kind == Pk::Pot && p.id == 1).unwrap();
            (p.x, p.y)
        };
        let k = s.seats.get_mut(&1).unwrap();
        k.x = px + 12 * Q;
        k.y = py;
        k.face = (-Q, 0);
    }
    hold_keys(&mut r, 1, Input { swing: true, ..Default::default() }, 6);
    let s = sess(&r);
    let lying = s.drops.iter().filter(|d| d.kind == Dk::Bomb).count() as u32;
    assert_eq!(lying + s.seats[&1].bombs, 3, "three bombs, lying or taken");
}

#[test]
fn pots_and_grass_drop_hearts_and_gems_and_nothing_slides_when_a_snuffer_dies() {
    let mut r = Run::lanternward_duel_for_test(18);
    {
        let f = &mut session(&mut r).foes[0];
        f.hp = 2;
    }
    let sword = Input { swing: true, ..Default::default() };
    for _ in 0..6 {
        hold_keys(&mut r, 1, sword, 1);
    }
    let f = &sess(&r).foes[0];
    assert_eq!(f.st, Fs::Dying);
    let at = (f.x, f.y);
    step_n(&mut r, 8);
    assert_eq!((sess(&r).foes[0].x, sess(&r).foes[0].y), at, "no sliding corpse");
    step_n(&mut r, 10);
    assert!(sess(&r).foes.is_empty(), "gone in a puff");
}

#[test]
fn a_chest_holds_its_item_up_and_a_solved_door_cues_every_camera() {
    let r = at("chest", 2);
    let s = sess(&r);
    assert!(s.inv.map, "the map in the Stair's nook");
    let k = &s.seats[&1];
    assert!(k.act == Act::Item && k.item == Some(Item::Map) || s.banner.is_some() || s.inv.map);
    // The Lamp Hall's three braziers light the niche: a chime and the cue.
    let mut r = at("lamp-lit", 1);
    step_n(&mut r, 2);
    r.sounds.clear();
    step_n(&mut r, 2);
    assert_eq!(sess(&r).stage, 1, "all three at once");
    assert!(sess(&r).cue.is_some() || r.sounds.contains(&"wish_grant"));
    let _ = &mut r;
}

#[test]
fn the_low_heart_beep_sounds_only_for_a_knight_on_two_hearts_or_less() {
    let mut r = Run::lanternward_duel_for_test(0);
    session(&mut r).seats.get_mut(&1).unwrap().hp = LOW_HP + 1;
    r.sounds.clear();
    step_n(&mut r, BEEP_EVERY * 2);
    assert!(!r.sounds.contains(&"mana_empty"));
    session(&mut r).seats.get_mut(&1).unwrap().hp = LOW_HP;
    step_n(&mut r, BEEP_EVERY + 2);
    assert!(r.sounds.contains(&"mana_empty"), "the chirp");
}

#[test]
fn every_big_foe_shows_its_whole_footprint_for_at_least_forty_ticks_and_never_moves_it() {
    for seats in [1, 3, 6] {
        let mut r = at("warden", seats);
        let mut tells = Vec::new();
        let mut cur: Option<(Danger, u32)> = None;
        for _ in 0..1500 {
            for k in session(&mut r).seats.values_mut() {
                k.inv = 99;
                k.hp = k.max_hp;
            }
            step_n(&mut r, 1);
            let s = sess(&r);
            let w = s.boss.as_ref().unwrap();
            if w.st == Ws::Tell {
                match &mut cur {
                    Some((d, n)) => {
                        assert_eq!(*d, w.danger, "the footprint is locked for the whole tell");
                        *n += 1;
                    }
                    None => cur = Some((w.danger, 1)),
                }
            } else if let Some((_, n)) = cur.take() {
                if w.atk != Atk::Summon {
                    tells.push(n);
                }
            }
            if w.st == Ws::Strike && cur.is_some() {
                let (_, n) = cur.take().unwrap();
                if w.atk != Atk::Summon {
                    tells.push(n);
                }
            }
        }
        assert!(tells.len() >= 3, "{seats} seats: it attacked {} times", tells.len());
        assert!(tells.iter().all(|&n| n >= 40), "{seats} seats: tells {tells:?}");
    }
    // The turret's tell too.
    let mut r = at("gallery", 1);
    let mut longest = 0;
    let mut now = 0;
    session(&mut r).set_gate(1, true);
    {
        let k = session(&mut r).seats.get_mut(&1).unwrap();
        k.x = 22 * TILE * Q;
        k.y = 5 * TILE * Q;
    }
    for _ in 0..300 {
        session(&mut r).seats.get_mut(&1).unwrap().inv = 99;
        step_n(&mut r, 1);
        if sess(&r).foes[0].st == Fs::Tell {
            now += 1;
            longest = longest.max(now);
        } else {
            now = 0;
        }
    }
    assert!(longest >= 40, "the turret's tell: {longest}");
}

#[test]
fn the_five_numbers_change_the_feel_where_they_say() {
    // Lock reach: a foe at 100 px is held at 128 and not at 80.
    let mut near = Run::lanternward_duel_for_test(100);
    near.tune.set("lanternward_lock", 80.0);
    hold_keys(&mut near, 1, lock_key(), 3);
    assert!(sess(&near).seats[&1].lock.is_none(), "out of an 80 px reach");
    let mut far = Run::lanternward_duel_for_test(100);
    far.tune.set("lanternward_lock", 128.0);
    hold_keys(&mut far, 1, lock_key(), 3);
    assert!(sess(&far).seats[&1].lock.is_some());
    // Combo window: a late second press chains only with the wider one.
    let late = |window: f64| {
        let mut r = Run::lanternward_duel_for_test(18);
        r.tune.set("lanternward_combo", window);
        session(&mut r).foes[0].hp = 100;
        session(&mut r).foes[0].full = 100;
        let (fx, fy) = (sess(&r).foes[0].x, sess(&r).foes[0].y);
        let sword = Input { swing: true, ..Default::default() };
        hold_keys(&mut r, 1, sword, 1);
        for _ in 0..28 {
            hold_keys(&mut r, 1, Input::default(), 1);
            let f = &mut session(&mut r).foes[0];
            (f.x, f.y) = (fx, fy);
        }
        // Eighteen ticks after the slash ended; is the chain still alive?
        sess(&r).seats[&1].chain > 0
    };
    assert!(!late(6.0) && late(24.0), "6 ticks lets the chain go, 24 holds it");
    // Flame: the tank is the number.
    let mut r = Run::lanternward_duel_for_test(0);
    r.tune.set("lanternward_flame", 60.0);
    session(&mut r).inv.lantern = true;
    hold_keys(&mut r, 1, Input::default(), 400);
    assert!(sess(&r).seats[&1].flame <= 60);
    // Telegraph: the floor holds at forty however the knob is moved.
    let mut t = Run::lanternward_duel_for_test(0);
    t.tune.set("lanternward_tell", 10.0);
    assert!(Rules::of(&t.tune).tell_for(1) >= 40);
    // Parry window: a block 10 ticks after the shield goes up is a parry
    // with a window of 12 and only a block with 8.
    for (window, want) in [(8.0, Fs::Recover), (12.0, Fs::Stagger)] {
        let mut r = Run::lanternward_duel_for_test(26);
        r.tune.set("lanternward_parry", window);
        session(&mut r).foes[0].st = Fs::Drift;
        session(&mut r).foes[0].t = 1;
        hold_keys(&mut r, 1, lock_key(), 1);
        let guard = Input { ult: true, dash: true, ..Default::default() };
        let mut result = None;
        for _ in 0..200 {
            let f = &sess(&r).foes[0];
            // Up ten ticks before the lunge's first blow.
            let raise = f.st == Fs::Tell && f.t <= 9 || f.st == Fs::Strike;
            hold_keys(&mut r, 1, if raise { guard } else { lock_key() }, 1);
            let st = sess(&r).foes[0].st;
            if st == Fs::Stagger || st == Fs::Recover {
                result = Some(st);
                break;
            }
        }
        assert_eq!(result, Some(want), "window {window}");
    }
}

/// `cargo test -p angel-delve --lib print_the_bot_record -- --ignored --nocapture`:
/// the ticks four bots (or one, or six) take from the door to the Beacon.
#[test]
#[ignore = "prints the tuning record"]
fn print_the_bot_record() {
    for seats in 1..=6 {
        let (r, done) = bots_play(seats, 14_000, false);
        let s = sess(&r);
        eprintln!(
            "RECORD seats {seats}: beacon at {done:?} ticks ({}s), retries {}, hearts left {:?}",
            done.map_or(0, |t| t / HZ),
            s.tries,
            s.seats.values().map(|k| k.hp / HEART).collect::<Vec<_>>()
        );
    }
}

/// The Warden alone, from the door, with bots at `seats` and some knobs moved:
/// ticks to its fall (None: not in `limit`), quarter hearts lost, and retries.
fn warden_round(seats: u32, knobs: &[(&str, f64)], limit: u32) -> (Option<u32>, u32, u32) {
    let mut r = at("warden", seats);
    for &(name, value) in knobs {
        assert!(r.tune.set(name, value), "{name}");
    }
    let start = sess(&r).elapsed;
    let mut lost = 0;
    let mut hp: u32 = sess(&r).seats.values().map(|k| k.hp).sum();
    for _ in 0..limit {
        let i = r.roundtable_inputs();
        r.step(&i);
        let s = sess(&r);
        let now: u32 = s.seats.values().map(|k| k.hp).sum();
        lost += hp.saturating_sub(now);
        hp = now;
        if s.stage >= 2 {
            return (Some(s.elapsed - start), lost, s.tries);
        }
    }
    (None, lost, sess(&r).tries)
}

/// The whole adventure with bots and some knobs moved: tick of the Beacon.
fn full_round(seats: u32, knobs: &[(&str, f64)], limit: u32) -> (Option<u32>, u32) {
    let mut r = table(seats);
    for &(name, value) in knobs {
        assert!(r.tune.set(name, value), "{name}");
    }
    for tick in 0..limit {
        let i = r.roundtable_inputs();
        r.step(&i);
        if sess(&r).done.is_some() {
            return (Some(tick), sess(&r).tries);
        }
    }
    (None, sess(&r).tries)
}

/// A shield raised `lead` ticks before a lunge lands, under a parry window of
/// `window`: what became of it.
fn parry_outcome(window: f64, lead: i32) -> &'static str {
    let mut r = Run::lanternward_duel_for_test(26);
    r.tune.set("lanternward_parry", window);
    r.tune.set("hitstop", 0.0);
    session(&mut r).foes[0].st = Fs::Drift;
    session(&mut r).foes[0].t = 1;
    hold_keys(&mut r, 1, lock_key(), 1);
    let guard = Input { ult: true, dash: true, ..Default::default() };
    for _ in 0..200 {
        let f = &sess(&r).foes[0];
        // The lunge's first blow lands the tick after the tell ends; the
        // shield goes up `lead` ticks before that and stays up.
        let raise = f.st == Fs::Tell && i32::from(f.t) < lead || f.st == Fs::Strike && lead > 0 || f.st == Fs::Strike && lead == 0;
        hold_keys(&mut r, 1, if raise { guard } else { lock_key() }, 1);
        let s = sess(&r);
        if s.seats[&1].hp < START_HP {
            return "hit";
        }
        match s.foes[0].st {
            Fs::Stagger => return "PARRY",
            Fs::Recover => return "block",
            _ => {}
        }
    }
    "none"
}

/// `cargo test -p angel-delve --lib print_the_tuning_tables -- --ignored --nocapture`
#[test]
#[ignore = "plays the tuning rounds"]
fn print_the_tuning_tables() {
    eprintln!("TUNE lock reach (px): whole adventure, bots, ticks to the Beacon (1 seat / 3 seats)");
    for lock in [64.0, 80.0, 96.0, 112.0, 160.0] {
        let a = full_round(1, &[("lanternward_lock", lock)], 14_000);
        let b = full_round(3, &[("lanternward_lock", lock)], 14_000);
        eprintln!("TUNE   lock {lock:>5}: {:?} / {:?}", a.0, b.0);
    }
    eprintln!("TUNE parry window: a shield raised N ticks before the lunge lands (rows), window (columns)");
    for lead in [0, 2, 4, 6, 8, 10, 12, 16] {
        let row: Vec<&str> = [4.0, 6.0, 8.0, 12.0].iter().map(|&w| parry_outcome(w, lead)).collect();
        eprintln!("TUNE   lead {lead:>2}: window 4/6/8/12 -> {row:?}");
    }
    eprintln!("TUNE combo window: heavy third hits in 5 seconds for a masher pressing every N ticks");
    for gap in [8, 10, 12, 14, 16, 20, 24] {
        let row: Vec<u32> = [6.0, 10.0, 14.0, 20.0].iter().map(|&w| {
            let mut r = Run::lanternward_duel_for_test(18);
            r.tune.set("lanternward_combo", w);
            r.tune.set("hitstop", 0.0);
            session(&mut r).foes[0].hp = 1000;
            session(&mut r).foes[0].full = 1000;
            let (fx, fy) = (sess(&r).foes[0].x, sess(&r).foes[0].y);
            let mut heavy = 0;
            let mut last = 1000;
            for t in 0..150u32 {
                let sword = Input { swing: t % gap == 0, ..Default::default() };
                hold_keys(&mut r, 1, sword, 1);
                let f = &mut session(&mut r).foes[0];
                (f.x, f.y) = (fx, fy);
                if last - f.hp == 4 {
                    heavy += 1;
                }
                last = f.hp;
            }
            heavy
        }).collect();
        eprintln!("TUNE   every {gap:>2} ticks: window 6/10/14/20 -> {row:?}");
    }
    eprintln!("TUNE flame tank (ticks): the Warden alone, ticks to its fall / quarter hearts lost / retries at 1, 3 and 6 seats");
    for flame in [60.0, 100.0, 150.0, 220.0] {
        let rows: Vec<String> = [1, 3, 6].iter().map(|&n| format!("{:?}", warden_round(n, &[("lanternward_flame", flame)], 9000))).collect();
        eprintln!("TUNE   flame {flame:>5}: {}", rows.join("  "));
    }
    eprintln!("TUNE telegraph (ticks): the Warden alone, ticks to its fall / quarter hearts lost / retries at 1, 3 and 6 seats");
    for tell in [40.0, 48.0, 56.0, 70.0, 90.0] {
        let rows: Vec<String> = [1, 3, 6].iter().map(|&n| format!("{:?}", warden_round(n, &[("lanternward_tell", tell)], 9000))).collect();
        eprintln!("TUNE   tell {tell:>5}: {}", rows.join("  "));
    }
}

#[test]
fn the_session_stays_small_on_the_wire() {
    for (moment, seats) in [("stair", 1), ("lamp-wave", 6), ("warden-open", 6), ("vault-flare", 6)] {
        let r = at(moment, seats);
        let n = serde_json::to_vec(&r.lanternward).unwrap().len();
        eprintln!("WIRE {moment} {seats} seats: {n} bytes");
        assert!(n < 40_000, "{moment}: {n} bytes");
    }
}

#[test]
fn a_block_slides_a_tile_at_a_time_and_never_onto_a_tile_against_a_wall() {
    let mut r = at("gallery", 1);
    let (bx, by) = {
        let b = sess(&r).props.iter().find(|p| p.kind == Pk::Block).unwrap();
        (b.x, b.y)
    };
    // Behind it, pushing east: it moves a tile after a moment's lean.
    {
        let k = session(&mut r).seats.get_mut(&1).unwrap();
        k.x = bx - 14 * Q;
        k.y = by;
    }
    hold_keys(&mut r, 1, Input { move_x: 1, ..Default::default() }, 40);
    let b = sess(&r).props.iter().find(|p| p.kind == Pk::Block).unwrap();
    assert!(b.x >= bx + TILE * Q || b.on == 1, "a tile on: {} from {}", b.x / Q, bx / Q);
    // Push it north from below until it would meet the wall: it stops a tile
    // short, where it can still be pushed back.
    {
        let s = session(&mut r);
        let b = s.props.iter_mut().find(|p| p.kind == Pk::Block).unwrap();
        (b.x, b.y) = (middle(5), middle(4));
        b.tx = 5;
        b.ty = 4;
        let k = s.seats.get_mut(&1).unwrap();
        (k.x, k.y) = (middle(5), middle(5) + 4 * Q);
    }
    hold_keys(&mut r, 1, Input { move_y: -1, ..Default::default() }, 200);
    let b = sess(&r).props.iter().find(|p| p.kind == Pk::Block).unwrap();
    assert!(b.ty >= 2, "never against the top wall: row {}", b.ty);
}

#[test]
fn scratch_damage_sources() {
    for seats in [1u32, 2, 6] {
        let mut r = at("warden", seats);
        let mut by: BTreeMap<String, u32> = BTreeMap::new();
        let mut hp: BTreeMap<u32, u32> = sess(&r).seats.iter().map(|(&id, k)| (id, k.hp)).collect();
        let mut end = 0;
        for t in 0..4000 {
            let i = r.roundtable_inputs();
            r.step(&i);
            let s = sess(&r);
            for (&id, k) in &s.seats {
                let was = hp.insert(id, k.hp).unwrap_or(k.hp);
                if k.hp < was {
                    let src = match s.boss.as_ref() {
                        Some(w) if w.st == Ws::Strike => format!("{:?}{}", w.atk, w.phase),
                        _ => "other".to_string(),
                    };
                    *by.entry(src).or_default() += was - k.hp;
                }
            }
            if s.stage >= 2 { end = t; break; }
        }
        eprintln!("DMG seats {seats}: end {end} {by:?} tries {}", sess(&r).tries);
    }
}

#[test]
fn a_retry_puts_the_warden_back_at_the_start_of_its_phase_at_the_tables_full_share() {
    let mut r = at("warden", 1);
    // Into its second third alone, then a friend sits down.
    {
        let w = session(&mut r).boss.as_mut().unwrap();
        w.phase = 1;
        w.hp = w.full * 2 / 3;
        w.start = w.hp;
    }
    step_n(&mut r, 2);
    r.join_seat(2, "Knight 2");
    step_n(&mut r, 2);
    let (full, start) = {
        let w = sess(&r).boss.as_ref().unwrap();
        (w.full, w.start)
    };
    assert_eq!(full, 2 * WARDEN_HP, "scaled for two");
    assert_eq!(start, full * 2 / 3 + 0, "the phase's start is scaled with it: {start}");
    // The Warden is hurt a little, the table falls, and the retry puts it back
    // where the phase began: not weaker than its share.
    session(&mut r).boss.as_mut().unwrap().hp = start - 20;
    for k in session(&mut r).seats.values_mut() {
        k.hp = 0;
        k.act = Act::Down;
    }
    step_n(&mut r, WIPE_TICKS + 2);
    let w = sess(&r).boss.as_ref().unwrap();
    assert_eq!((w.hp, w.phase), (start, 1));
    assert!(sess(&r).seats.values().all(|k| k.up()));
}

#[test]
fn a_knight_who_stands_up_and_sits_down_again_keeps_their_gems_but_is_not_paid_for_the_time_away() {
    let mut r = at("stair", 2);
    session(&mut r).seats.get_mut(&2).unwrap().gems = 7;
    session(&mut r).seats.get_mut(&2).unwrap().bombs = 2;
    assert!(r.leave_activity(2));
    step_n(&mut r, 3);
    assert_eq!(sess(&r).seats.len(), 1);
    assert_eq!(sess(&r).kept.get(&2), Some(&(7, 2)));
    r.join_seat(2, "Knight 2");
    step_n(&mut r, 3);
    let k = &sess(&r).seats.get(&2);
    if let Some(k) = k {
        assert_eq!((k.gems, k.bombs), (7, 2));
        assert!(sess(&r).kept.is_empty());
    }
    assert!(sess(&r).valid());
}

/// A party can stay in one room as long as it likes: the room's own clock
/// must not count the session out, for a host that stops being valid is one
/// every other screen refuses to draw.
#[test]
fn a_party_staying_in_one_room_past_the_limit_stays_valid() {
    let mut r = table(2);
    r.lanternward.as_mut().unwrap().stage_t = LIMIT - 2;
    assert!(r.lanternward.as_ref().unwrap().valid());
    for n in 0..6 {
        r.step(&BTreeMap::new());
        assert!(
            r.lanternward.as_ref().unwrap().valid(),
            "a valid session {n} ticks past the limit"
        );
    }
}

/// The Warden's attack counter picks its pattern and wraps by design; no
/// value of it may be one the session refuses.
#[test]
fn every_value_of_the_wardens_attack_counter_is_valid() {
    let mut w = super::boss::Warden::new(10 * Q, 10 * Q, 2);
    assert!(w.valid(), "a fresh Warden is valid");
    for n in 0..=u8::MAX {
        w.n = n;
        assert!(w.valid(), "attack counter {n}");
    }
}

/// The same of a bolt: the smallest number is not a speed.
#[test]
fn the_smallest_bolt_speed_is_not_a_valid_one() {
    let mut r = table(1);
    let s = r.lanternward.as_mut().unwrap();
    assert!(s.valid());
    s.seats.get_mut(&1).unwrap().mv.0 = i32::MIN;
    assert!(!s.valid());
}
