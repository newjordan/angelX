//! Latch Duel's show: marks written where counters land and guards fold, the calls the table hears,
//! the title that stands the bout still, the hold the winning blow asks of the room, and the time to
//! beat a rematch carries.
use super::{BTreeMap, Input, Run};
use crate::rules::juice::Weight;
use crate::rules::latch::*;
use crate::rules::latch_show::{Cry, Kind, Show, Why};

fn game(n: u32, count: bool) -> Run {
    let mut r = Run::latch_for_test();
    if n == 1 {
        r.leave(2);
    }
    for id in 2..=n {
        r.join_seat(id, &format!("Seat {id}"));
    }
    r.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    assert!(r.latch.is_some());
    if !count {
        r.latch.as_mut().unwrap().count = 0;
    }
    r
}

fn counter(d: &Duel) -> Input {
    Input { move_y: d.height, fire: true, ..Default::default() }
}

#[test]
fn the_title_and_the_three_two_one_stand_the_bout_still_and_the_clock_does_not_run() {
    let mut r = game(2, true);
    assert_eq!(r.latch.as_ref().unwrap().count, COUNT);
    let before = r.latch.clone().unwrap();
    for _ in 0..COUNT {
        r.step(&BTreeMap::from([(1, Input { fire: true, move_y: 1, ..Default::default() })]));
        let s = r.latch.as_ref().unwrap();
        assert_eq!(s.left, LENGTH, "the clock waits for the bell");
        assert_eq!(s.duels, before.duels, "no bout is judged while the numbers run");
    }
    assert_eq!(r.latch.as_ref().unwrap().count, 0);
    r.step(&BTreeMap::new());
    assert_eq!(r.latch.as_ref().unwrap().left, LENGTH - 1);
    assert!(r.latch.as_ref().unwrap().valid());
}

#[test]
fn a_counter_and_each_way_a_guard_folds_are_marked_and_weighed() {
    let mut r = game(1, false);
    let t = r.tune.clone();
    // A clean counter on the closing beat.
    {
        let d = r.latch.as_mut().unwrap().duels.get_mut(&1).unwrap();
        d.beat = Beat::Tell;
        d.ticks = 5;
        d.windup = t.latch_tell;
        d.height = -1;
        d.guard = -1;
    }
    let input = counter(&r.latch.as_ref().unwrap().duels[&1]);
    r.step(&BTreeMap::from([(1, input)]));
    let s = r.latch.as_ref().unwrap();
    let m = s.show.marks.last().unwrap();
    assert_eq!(m.who, 1);
    assert!(matches!(m.kind, Kind::Counter { chain: 1, count: 1, error: 4, .. }), "{:?}", m.kind);
    assert_eq!(m.kind.weight(), Weight::Solid, "a sloppy first counter is a solid blow");
    // On the closing beat the same counter is a crushing one.
    let mut q = game(1, false);
    {
        let d = q.latch.as_mut().unwrap().duels.get_mut(&1).unwrap();
        d.beat = Beat::Tell;
        d.ticks = 2;
        d.windup = t.latch_tell;
        d.height = -1;
        d.guard = -1;
    }
    let input = counter(&q.latch.as_ref().unwrap().duels[&1]);
    q.step(&BTreeMap::from([(1, input)]));
    assert_eq!(q.latch.as_ref().unwrap().show.marks.last().unwrap().kind.weight(), Weight::Crushing);
    for (why, ticks, guard, press) in [(Why::Early, 20, -1, true), (Why::Wrong, 3, 1, true), (Why::Slow, 1, -1, false)] {
        let mut r = game(1, false);
        {
            let d = r.latch.as_mut().unwrap().duels.get_mut(&1).unwrap();
            d.beat = Beat::Tell;
            d.ticks = ticks;
            d.windup = t.latch_tell;
            d.height = -1;
            d.guard = guard;
            d.chain = 0;
        }
        r.step(&BTreeMap::from([(1, Input { fire: press, move_y: guard, ..Default::default() })]));
        let s = r.latch.as_ref().unwrap();
        assert_eq!(s.duels[&1].beat, Beat::Miss, "{why:?}");
        let m = s.show.marks.last().unwrap();
        assert_eq!(m.kind, Kind::Miss { why, lost: 0 }, "{why:?}");
        assert_eq!(m.kind.weight(), Weight::Graze);
    }
}

#[test]
fn the_third_counter_is_the_finale_and_the_realm_holds_the_bay_still_for_it_alone() {
    let mut r = game(2, false);
    let t = r.tune.clone();
    for id in [1, 2] {
        let d = r.latch.as_mut().unwrap().duels.get_mut(&id).unwrap();
        d.beat = Beat::Tell;
        d.ticks = 2;
        d.windup = t.latch_tell;
        d.height = 1;
        d.guard = 1;
        d.counters = u8::from(id == 1) * 2 + u8::from(id == 2);
        d.chain = d.counters;
    }
    let d1 = r.latch.as_ref().unwrap().duels[&1].clone();
    r.step(&BTreeMap::from([(1, counter(&d1))]));
    let s = r.latch.as_ref().unwrap();
    assert_eq!(s.duels[&1].counters, 3);
    assert_eq!(s.win_at, Some(1));
    let m = s.show.marks.iter().find(|m| m.who == 1).unwrap();
    assert!(matches!(m.kind, Kind::Counter { count: 3, chain: 3, .. }));
    assert_eq!(m.kind.weight(), Weight::Finale);
    assert!(r.hitstop.left > 0, "the winning blow holds the room");
    // A middling counter holds nothing: seat 2 counters in a fresh round.
    let mut q = game(2, false);
    {
        let d = q.latch.as_mut().unwrap().duels.get_mut(&2).unwrap();
        d.beat = Beat::Tell;
        d.ticks = 2;
        d.windup = t.latch_tell;
        d.height = 1;
        d.guard = 1;
        d.counters = 1;
        d.chain = 1;
    }
    let d2 = q.latch.as_ref().unwrap().duels[&2].clone();
    q.step(&BTreeMap::from([(2, counter(&d2))]));
    assert_eq!(q.latch.as_ref().unwrap().duels[&2].counters, 2);
    assert_eq!(q.hitstop.left, 0, "a counter short of the last never pauses another knight's bout");
}

#[test]
fn the_table_hears_a_lead_match_point_and_the_whole_table_in_step_once_each() {
    let mut show = Show::default();
    let seats = [1, 2, 3];
    show.standings(5, &[(1, 0), (2, 0), (3, 0)]);
    assert!(show.calls.is_empty());
    show.standings(6, &[(1, 1), (2, 0), (3, 0)]);
    assert_eq!(show.calls.iter().filter(|c| c.cry == Cry::Lead && c.who == 1).count(), 1);
    show.standings(7, &[(1, 1), (2, 0), (3, 0)]);
    assert_eq!(show.calls.iter().filter(|c| c.cry == Cry::Lead).count(), 1, "the same lead is not called again");
    show.standings(8, &[(1, 2), (2, 0), (3, 0)]);
    assert_eq!(show.calls.iter().filter(|c| c.cry == Cry::Point && c.who == 1).count(), 1);
    show.standings(9, &[(1, 2), (2, 0), (3, 0)]);
    assert_eq!(show.calls.iter().filter(|c| c.cry == Cry::Point).count(), 1, "match point is called once");
    // Sync: the whole table counters attack 1, then attack 2: the second makes a streak of two.
    let mut show = Show::default();
    for (attack, now) in [(1u32, 10u32), (2, 50)] {
        for who in seats {
            show.counter(who, 1, 1, 1, attack, now, &seats);
        }
    }
    assert_eq!(show.calls.iter().filter(|c| matches!(c.cry, Cry::Sync(2))).count(), 1);
    // One knight missing an attack breaks the streak.
    let mut show = Show::default();
    for who in seats {
        show.counter(who, 1, 1, 1, 1, 10, &seats);
    }
    show.counter(1, 1, 1, 1, 2, 50, &seats);
    show.counter(2, 1, 1, 1, 2, 50, &seats);
    show.counter(1, 1, 1, 1, 3, 90, &seats);
    assert!(show.calls.is_empty(), "{:?}", show.calls);
    assert!(show.valid());
}

#[test]
fn a_late_press_that_saves_a_tell_takes_the_fold_back_off_the_books() {
    let mut show = Show::default();
    show.miss(2, Why::Slow, 1, 100);
    assert_eq!(show.marks.len(), 1);
    show.counter(2, 2, 2, 1, 4, 103, &[1, 2]);
    assert_eq!(show.marks.len(), 1);
    assert!(matches!(show.marks[0].kind, Kind::Counter { .. }));
}

#[test]
fn a_whole_round_keeps_its_show_valid_and_exact_on_the_wire_and_a_rematch_carries_the_time_to_beat() {
    let mut r = game(3, true);
    let mut finished = None;
    let (mut synced, mut led, mut pointed) = (false, false, false);
    for t in 0..(COUNT + LENGTH + 20) {
        let keys: BTreeMap<u32, Input> = r
            .latch
            .as_ref()
            .filter(|s| s.left > 0 && s.count == 0)
            .map(|s| s.duels.iter().map(|(&id, d)| (id, d.bot_input(1 + id % 3))).collect())
            .unwrap_or_default();
        r.step(&keys);
        let s = r.latch.as_ref().unwrap();
        assert!(s.valid(), "tick {t}");
        synced |= s.show.calls.iter().any(|c| matches!(c.cry, Cry::Sync(_)));
        led |= s.show.calls.iter().any(|c| c.cry == Cry::Lead);
        pointed |= s.show.calls.iter().any(|c| c.cry == Cry::Point);
        if t % 7 == 0 {
            assert_eq!(r.wire_copy().latch, r.latch, "the show survives the wire at tick {t}");
        }
        if s.left == 0 {
            finished = Some(s.clone());
            break;
        }
    }
    let s = finished.expect("the round ended");
    assert!(synced && led && pointed, "three knights on the beat: the table heard sync {synced}, a lead {led} and match point {pointed}");
    assert!(s.won);
    let win = s.win_at.expect("the winning clock was kept");
    assert!(win > 0 && win < LENGTH);
    assert_eq!(s.quickest(), win);
    assert_eq!(s.elapsed(u64::from(s.done)), s.ended, "the clock stands where the round ended");
    assert_eq!(s.elapsed(u64::from(s.done) + 9), s.ended + 9, "and runs on for the end screen");
    // Rematch: E, let go, E.
    r.step(&BTreeMap::new());
    r.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    let next = r.latch.as_ref().unwrap();
    assert_eq!(next.to_beat, win, "the time to beat comes along");
    assert_eq!(next.count, COUNT);
    assert!(next.show.quiet() && next.win_at.is_none());
}

#[test]
fn a_broken_show_is_repaired_and_marks_are_capped() {
    let mut r = game(2, false);
    let s = r.latch.as_mut().unwrap();
    for k in 0..60 {
        s.show.miss(1 + k % 2, Why::Slow, 0, k);
    }
    assert!(s.show.marks.len() <= 24);
    assert!(s.valid());
    s.show.marks[0].who = 9;
    assert!(!s.valid());
    s.repair();
    assert!(s.valid());
    assert!(s.show.quiet());
    let mut d = Duel::new(3);
    d.counters = 3;
    d.chain = 3;
    assert!(d.valid());
}
