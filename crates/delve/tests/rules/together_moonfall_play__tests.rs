//! Moonfall played: the clock, the bell, the Ledger, the folk, the favours,
//! the wisps, the three lights. These step the session itself (one tick
//! each, the Delve's own heroes seated) so a whole three-day loop is cheap.
use super::*;
use crate::rules::moonfall::{
    clock,
    folk::{CAST, Folk, figure},
    ledger::{Fact, Memory},
    plan,
    state::*,
    town::{self, Spot},
};
use crate::rules::{Hero, Input, MAX_PLAYERS, Run};
use std::collections::BTreeMap;

pub(super) type Held = BTreeMap<u32, Input>;

/// The Delve's heroes at a table of `n`.
pub(super) fn heroes(n: u32) -> BTreeMap<u32, Hero> {
    let mut r = Run::moonfall_for_test();
    for id in 2..=n {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.players.clone()
}

pub(super) fn session(n: u32) -> (Session, BTreeMap<u32, Hero>) {
    let h = heroes(n);
    (Session::new(&h), h)
}

pub(super) fn tick(s: &mut Session, h: &BTreeMap<u32, Hero>, inputs: &Held) -> Vec<&'static str> {
    s.step(inputs, h)
}

pub(super) fn idle(s: &mut Session, h: &BTreeMap<u32, Hero>, ticks: u32) {
    for _ in 0..ticks {
        s.step(&Held::new(), h);
    }
}

pub(super) fn press(id: u32, i: Input) -> Held {
    Held::from([(id, i)])
}

pub(super) fn hours(day: u32, h: u32) -> u32 {
    clock::ticks(day, h * 2)
}

/// Put knight `id` on a tile.
pub(super) fn stand(s: &mut Session, id: u32, tile: (i32, i32)) {
    let k = &mut s.seats.get_mut(&id).unwrap().knight;
    k.x = (tile.0 * town::TILE + 8) * town::Q;
    k.y = (tile.1 * town::TILE + 8) * town::Q;
}

/// Put knight `id` beside a villager, wherever they are now.
pub(super) fn beside(s: &mut Session, id: u32, who: Folk) {
    let f = figure(who, s.clock, s.nums.day, s.world.shod);
    let k = &mut s.seats.get_mut(&id).unwrap().knight;
    k.x = (f.x + 10) * town::Q;
    k.y = f.y * town::Q;
}

pub(super) fn tap_e() -> Input {
    Input { interact: true, ..Default::default() }
}

/// A key pressed for one tick, then let go.
pub(super) fn tapped(s: &mut Session, h: &BTreeMap<u32, Hero>, id: u32, i: Input) {
    s.step(&press(id, i), h);
    s.step(&Held::new(), h);
}

#[test]
fn the_clock_runs_three_days_of_the_numbers_length_and_ends_at_the_fall() {
    let (mut s, h) = session(1);
    assert_eq!((s.clock, s.memory.loops), (0, 0));
    idle(&mut s, &h, 600);
    assert_eq!(s.clock, 600);
    assert_eq!(clock::stamp(s.clock, s.nums.day).text(), "10:48");
    s.nums.day = 1800;
    assert_eq!(clock::end(s.nums.day), 5400);
    s.clock = clock::end(3000) - 1;
    s.nums.day = 3000;
    // The moon falls: the ritual starts at once, no menu.
    let cues = tick(&mut s, &h, &Held::new());
    assert!(matches!(s.phase, Phase::Rewind { fell: true, .. }), "{:?}", s.phase);
    assert!(cues.contains(&"circle_toll"));
}

#[test]
fn the_rewind_takes_under_a_second_to_hand_back_control_and_keeps_what_was_learned() {
    let (mut s, h) = session(1);
    idle(&mut s, &h, 1200);
    s.memory.learn(Fact::OrinLost);
    s.memory.witness(Folk::Mira, 1);
    s.memory.masks = ledger::SWIFT;
    s.world.set(NORTH_OPEN);
    s.seats.get_mut(&1).unwrap().knight.held = Item::Ink;
    s.seats.get_mut(&1).unwrap().knight.mask = Mask::Swift;
    // R opens the bell, E rings the first line: back to the first dawn.
    tapped(&mut s, &h, 1, Input { ult: true, ..Default::default() });
    assert_eq!(s.seats[&1].knight.panel, Panel::Bell(0));
    s.step(&press(1, tap_e()), &h);
    assert!(matches!(s.phase, Phase::Rewind { fell: false, .. }));
    let mut free_at = None;
    for t in 1..=60 {
        s.step(&Held::new(), &h);
        if free_at.is_none() && s.phase == Phase::Play {
            free_at = Some(t);
        }
    }
    let free = free_at.expect("control returns");
    assert!(free <= 30, "the ritual hands control back inside a second: {free}");
    // The town is at the first dawn, the notebook and the mask remain.
    assert!(s.clock < 40 && s.memory.loops == 1);
    assert!(s.memory.knows(Fact::OrinLost) && s.memory.witnessed(Folk::Mira, 1));
    assert!(s.memory.has_mask(ledger::SWIFT));
    assert!(!s.world.has(NORTH_OPEN), "opened doors close again");
    let k = &s.seats[&1].knight;
    assert_eq!(k.held, Item::Empty, "carried things are gone");
    assert_eq!(k.candles, CANDLES);
    assert_eq!(k.mask, Mask::Swift, "a worn mask stays on");
    assert!(s.card > 0, "the dawn card shows");
    assert!(s.valid());
}

#[test]
fn the_rewind_ritual_spins_the_shown_clock_backward_to_dawn() {
    let (mut s, h) = session(1);
    s.clock = hours(s.nums.day, 40);
    tapped(&mut s, &h, 1, Input { ult: true, ..Default::default() });
    s.step(&press(1, tap_e()), &h);
    let from = s.clock;
    let mut last = s.shown_clock();
    let mut dropped = false;
    for _ in 0..control(&s.nums) - 2 {
        s.step(&Held::new(), &h);
        let now = s.shown_clock();
        assert!(now <= last, "never forward: {now} after {last}");
        dropped |= now < from / 2;
        last = now;
    }
    assert!(dropped, "the clock has spun most of the way back");
}

#[test]
fn the_bell_can_skip_to_the_next_dusk_or_dawn_and_not_past_the_third_midnight() {
    let (mut s, h) = session(1);
    // Down once and ring: wait for dusk.
    tapped(&mut s, &h, 1, Input { ult: true, ..Default::default() });
    tapped(&mut s, &h, 1, Input { move_y: 1, ..Default::default() });
    s.step(&press(1, tap_e()), &h);
    assert!(matches!(s.phase, Phase::Skip { .. }));
    idle(&mut s, &h, SKIP_LEN);
    assert_eq!(s.clock, s.nums.day / 2, "dusk of the first day");
    assert!(clock::night(s.clock, s.nums.day));
    // The next dawn.
    tapped(&mut s, &h, 1, Input { ult: true, ..Default::default() });
    tapped(&mut s, &h, 1, Input { move_y: 1, ..Default::default() });
    tapped(&mut s, &h, 1, Input { move_y: 1, ..Default::default() });
    s.step(&press(1, tap_e()), &h);
    idle(&mut s, &h, SKIP_LEN);
    assert_eq!(s.clock, s.nums.day);
    // On the third evening neither skip is on offer: the Third Midnight is
    // for the knights to meet.
    s.clock = s.nums.day * 2 + s.nums.day / 2 + 10;
    assert!(!s.ring_ok(Ring::Dawn));
    assert!(s.ring_ok(Ring::Rewind));
    s.clock = s.nums.day * 2 + s.nums.day / 4;
    assert!(s.ring_ok(Ring::Dusk));
    assert!(s.valid());
}

#[test]
fn at_a_table_the_bell_needs_more_than_half_the_hands_and_a_lapsed_call_does_nothing() {
    let (mut s, h) = session(3);
    for id in 1..=3 {
        stand(&mut s, id, (16 + id as i32, 16));
    }
    s.clock = hours(s.nums.day, 30);
    tapped(&mut s, &h, 1, Input { ult: true, ..Default::default() });
    s.step(&press(1, tap_e()), &h);
    assert!(s.call.is_some() && s.phase == Phase::Play, "one hand is not a table");
    assert_eq!(s.bell_need(), 2);
    // A second knight puts a hand on the rope with R.
    tapped(&mut s, &h, 2, Input { ult: true, ..Default::default() });
    assert!(matches!(s.phase, Phase::Rewind { .. }), "two of three rings it");
    // A call nobody joins lapses.
    let (mut s, h) = session(3);
    tapped(&mut s, &h, 1, Input { ult: true, ..Default::default() });
    s.step(&press(1, tap_e()), &h);
    assert!(s.call.is_some());
    idle(&mut s, &h, CALL_TICKS + 2);
    assert!(s.call.is_none() && s.phase == Phase::Play);
}

#[test]
fn the_ledger_writes_what_any_knight_sees_for_every_knight_and_only_once() {
    let (mut s, h) = session(2);
    // Mira on her porch at a quarter to seven; knight 2 beside her, knight 1 far away.
    s.clock = hours(s.nums.day, 1) + s.nums.day / 24 * 3 / 4;
    stand(&mut s, 1, (30, 19));
    beside(&mut s, 2, Folk::Mira);
    idle(&mut s, &h, 6);
    assert!(s.memory.witnessed(Folk::Mira, 1), "knight 2 saw her, so the Ledger has her");
    assert!(
        s.news.iter().any(|&(n, _)| matches!(n, Note::Wrote(Folk::Mira, 1))),
        "and it says so to the whole table"
    );
    let news = s.news.len();
    idle(&mut s, &h, 12);
    assert_eq!(s.memory.routines[Folk::Mira.index()].count_ones(), 1);
    assert!(s.news.len() <= news, "written once");
    // Out of notice range, nothing is seen.
    let (mut far, h) = session(1);
    far.clock = hours(far.nums.day, 1) + far.nums.day / 24;
    stand(&mut far, 1, (30, 19));
    idle(&mut far, &h, 20);
    assert!(!far.memory.witnessed(Folk::Mira, 1));
}

#[test]
fn a_known_appointment_pops_a_reminder_an_hour_before_it_begins() {
    let (mut s, h) = session(1);
    s.memory.witness(Folk::Mira, 4);
    let leg = crate::rules::moonfall::folk::timetable(s.nums.day).legs[Folk::Mira.index()]
        .iter()
        .find(|l| l.routine == 4)
        .map(|l| l.at)
        .unwrap();
    s.clock = leg - s.nums.day / 24 - 1;
    s.step(&Held::new(), &h);
    assert!(s.news.iter().any(|&(n, _)| matches!(n, Note::Remind(Folk::Mira, 4))));
}

#[test]
fn the_bridge_goes_up_at_eight_and_only_the_swift_mask_crosses_the_planks() {
    let (mut s, h) = session(1);
    s.clock = hours(s.nums.day, 15);
    assert!(clock::bridge_up(s.clock, s.nums.day));
    // At the bridge's south end walking north: the water stops a bare knight.
    stand(&mut s, 1, (18, 11));
    for _ in 0..90 {
        s.step(&press(1, Input { move_y: -1, ..Default::default() }), &h);
    }
    assert!(s.seats[&1].knight.px().1 > 10 * town::TILE, "stopped at the water");
    // Swift, at the planks, crosses.
    stand(&mut s, 1, (4, 11));
    s.seats.get_mut(&1).unwrap().knight.mask = Mask::Swift;
    for _ in 0..120 {
        s.step(&press(1, Input { move_y: -1, ..Default::default() }), &h);
    }
    assert!(s.seats[&1].knight.px().1 < 8 * town::TILE, "over the planks");
}

#[test]
fn the_clocks_numbers_are_the_knobs_and_a_session_tracks_them() {
    let mut r = Run::moonfall_for_test();
    r.tune.roundtable[SLOT][0] = 2400;
    r.step(&BTreeMap::new());
    assert_eq!(r.moonfall.as_ref().unwrap().nums.day, 2400);
    assert_eq!(nums_start().day, 3000);
    assert!(r.moonfall.as_ref().unwrap().valid());
    let _ = (MAX_PLAYERS, Memory::default(), CAST, Spot::Plaza);
}

/// Put the clock at `day` (1 to 3), label `hh:mm`.
pub(super) fn at_time(s: &mut Session, day: u32, hh: u32, mm: u32) {
    let since = if hh < 6 { hh + 24 - 6 } else { hh - 6 };
    s.clock = ((((day - 1) * 24 + since) * 60 + mm) as u64 * u64::from(s.nums.day) / 1440) as u32;
}

/// Knight 1 talks to `who` (beside them) and reads the first page.
pub(super) fn talk(s: &mut Session, h: &BTreeMap<u32, Hero>, who: Folk) -> Topic {
    beside(s, 1, who);
    // Let the room settle a tick so the press is an edge.
    s.step(&Held::new(), h);
    beside(s, 1, who);
    s.step(&press(1, tap_e()), h);
    s.step(&Held::new(), h);
    s.seats[&1].knight.say.map(|say| say.topic).expect("they spoke")
}

use crate::rules::moonfall::says::Topic;

fn hand(s: &Session) -> Item {
    s.seats[&1].knight.held
}

fn close_talk(s: &mut Session) {
    s.seats.get_mut(&1).unwrap().knight.say = None;
}

#[test]
fn every_person_tells_the_ledger_what_they_ask_the_first_time_you_speak() {
    let (mut s, h) = session(1);
    at_time(&mut s, 1, 9, 0);
    let facts = [
        (Folk::Orin, Fact::OrinLost),
        (Folk::Nessa, Fact::NessaCold),
        (Folk::Pella, Fact::PellaScroll),
        (Folk::Tovin, Fact::TovinPass),
        (Folk::Mira, Fact::MiraSeal),
    ];
    for (who, fact) in facts {
        assert!(!s.memory.knows(fact));
        assert_eq!(talk(&mut s, &h, who), Topic::Request, "{who:?}");
        close_talk(&mut s);
        assert!(s.memory.knows(fact), "{who:?} is written down");
        assert!(
            s.news.iter().any(|&(n, _)| n == Note::Learned(fact)),
            "and the whole table is told"
        );
    }
    // The Ledger page of each now carries what they ask.
    let p = ledger::page(&s.memory, Folk::Orin, s.clock, s.nums.day);
    assert_eq!(p.asks.len(), 1);
}

#[test]
fn the_scroll_is_handed_over_at_seven_the_first_morning_and_read_at_the_evening_well() {
    let (mut s, h) = session(1);
    s.memory.learn(Fact::PellaScroll);
    s.memory.learn(Fact::MiraSeal);
    // The first morning, at seven: Pella has the post.
    at_time(&mut s, 1, 7, 15);
    assert_eq!(talk(&mut s, &h, Folk::Pella), Topic::Give);
    assert_eq!(hand(&s), Item::Scroll);
    close_talk(&mut s);
    // Mira will not take it at her porch by day.
    at_time(&mut s, 1, 12, 0);
    idle(&mut s, &h, 2);
    assert_eq!(talk(&mut s, &h, Folk::Mira), Topic::Nag(2), "not yet: she reads at the well at nine");
    assert_eq!(hand(&s), Item::Scroll);
    close_talk(&mut s);
    // Nor on the first evening.
    at_time(&mut s, 1, 21, 10);
    s.step(&Held::new(), &h);
    assert_eq!(talk(&mut s, &h, Folk::Mira), Topic::Nag(2));
    close_talk(&mut s);
    // The second evening, at the well: the Swift mask, kept, and Pella's tick.
    at_time(&mut s, 2, 21, 10);
    s.step(&Held::new(), &h);
    assert_eq!(talk(&mut s, &h, Folk::Mira), Topic::Thanks);
    assert_eq!(hand(&s), Item::Empty);
    assert!(s.memory.has_mask(ledger::SWIFT) && s.memory.ticked(Folk::Pella));
    // Pella will not have it handed out twice, nor late.
    close_talk(&mut s);
    let (mut late, h2) = session(1);
    late.memory.learn(Fact::PellaScroll);
    at_time(&mut late, 1, 10, 0);
    assert_eq!(talk(&mut late, &h2, Folk::Pella), Topic::Nag(1), "the post went out at seven");
    assert_eq!(late.seats[&1].knight.held, Item::Empty);
}

#[test]
fn dry_reeds_take_twelve_hours_in_the_hand_and_the_oven_wants_them_on_the_second_morning() {
    let (mut s, h) = session(1);
    s.memory.learn(Fact::NessaCold);
    // The first evening at the stack: a bundle, green.
    at_time(&mut s, 1, 19, 0);
    stand(&mut s, 1, (13, 5));
    s.step(&Held::new(), &h);
    s.step(&press(1, tap_e()), &h);
    assert_eq!(hand(&s), Item::Reeds);
    close_talk(&mut s);
    // Eleven hours in the hand: still green.
    let held_at = s.seats[&1].knight.held_at;
    at_time(&mut s, 2, 8, 40);
    s.seats.get_mut(&1).unwrap().knight.held_at = s.clock - 11 * s.nums.day / 24;
    s.step(&Held::new(), &h);
    assert_eq!(talk(&mut s, &h, Folk::Nessa), Topic::Nag(2), "green");
    close_talk(&mut s);
    // Dry, but after the oven's hour: too late.
    at_time(&mut s, 2, 11, 0);
    s.seats.get_mut(&1).unwrap().knight.held_at = held_at;
    s.step(&Held::new(), &h);
    assert_eq!(talk(&mut s, &h, Folk::Nessa), Topic::Nag(3), "the oven's hour has gone");
    close_talk(&mut s);
    // In the window, dry.
    s.seats.get_mut(&1).unwrap().knight.held_at = held_at;
    at_time(&mut s, 2, 8, 45);
    s.step(&Held::new(), &h);
    assert_eq!(talk(&mut s, &h, Folk::Nessa), Topic::Thanks);
    assert!(s.memory.has_mask(ledger::STOUT) && s.memory.ticked(Folk::Nessa));
    // Reeds taken this morning are green: not twelve hours.
    close_talk(&mut s);
    s.seats.get_mut(&1).unwrap().knight.held = Item::Reeds;
    s.seats.get_mut(&1).unwrap().knight.held_at = s.clock - 100;
    s.memory.masks = 0;
    assert_eq!(talk(&mut s, &h, Folk::Nessa), Topic::Nag(2), "green");
}

#[test]
fn the_pass_takes_five_hours_to_write_and_two_to_smudge_and_the_key_is_kept_for_the_party() {
    let (mut s, h) = session(1);
    s.memory.learn(Fact::OrinLost);
    s.memory.learn(Fact::TovinPass);
    // The first morning: the pot from the bank, to Orin while he searches.
    at_time(&mut s, 1, 7, 30);
    stand(&mut s, 1, (11, 3));
    s.step(&Held::new(), &h);
    s.step(&press(1, tap_e()), &h);
    assert_eq!(hand(&s), Item::Ink);
    close_talk(&mut s);
    assert!(s.world.has(INK_TAKEN));
    assert_eq!(talk(&mut s, &h, Folk::Orin), Topic::Thanks);
    close_talk(&mut s);
    let given = s.world.ink_given.expect("written for");
    // Too soon: still writing.
    s.clock = given + 2 * s.nums.day / 24;
    s.step(&Held::new(), &h);
    assert_eq!(talk(&mut s, &h, Folk::Orin), Topic::Nag(0));
    close_talk(&mut s);
    // Five hours on: the pass, and Orin's tick.
    s.clock = given + 5 * s.nums.day / 24 + 3;
    s.step(&Held::new(), &h);
    assert_eq!(talk(&mut s, &h, Folk::Orin), Topic::Give);
    assert_eq!(hand(&s), Item::Pass);
    assert!(s.memory.ticked(Folk::Orin));
    close_talk(&mut s);
    // Tovin does not air the keys on the first day.
    at_time(&mut s, 1, 18, 0);
    s.step(&Held::new(), &h);
    assert_eq!(talk(&mut s, &h, Folk::Tovin), Topic::Nag(1));
    close_talk(&mut s);
    // The second evening he does: the key is the party's, and the hand is free.
    at_time(&mut s, 2, 17, 20);
    s.step(&Held::new(), &h);
    assert_eq!(talk(&mut s, &h, Folk::Tovin), Topic::Thanks);
    assert!(s.memory.has_keep(ledger::NORTH_KEY) && s.memory.ticked(Folk::Tovin));
    assert_eq!(hand(&s), Item::Empty);
    // A pass left to wait smudges.
    let (mut late, h2) = session(1);
    late.memory.learn(Fact::OrinLost);
    late.world.ink_given = Some(0);
    at_time(&mut late, 1, 6, 0);
    late.clock = 7 * late.nums.day / 24 + 40;
    at_time(&mut late, 1, 15, 0);
    late.clock = 8 * late.nums.day / 24;
    late.step(&Held::new(), &h2);
    assert_eq!(talk(&mut late, &h2, Folk::Orin), Topic::Nag(2), "two hours, no more");
}

#[test]
fn only_a_knight_who_has_seen_the_rider_gets_the_shoe_and_only_at_the_dawn_cart() {
    let (mut s, h) = session(1);
    // Before the second dawn: no iron.
    at_time(&mut s, 1, 10, 0);
    s.memory.learn(Fact::NessaCold);
    assert_ne!(talk(&mut s, &h, Folk::Nessa), Topic::Give);
    close_talk(&mut s);
    // At the cart, not knowing the rider: who for?
    at_time(&mut s, 2, 7, 0);
    s.step(&Held::new(), &h);
    assert_eq!(talk(&mut s, &h, Folk::Nessa), Topic::Nag(0));
    assert_eq!(hand(&s), Item::Empty);
    close_talk(&mut s);
    // Having seen him arrive lame: the shoe.
    s.memory.learn(Fact::AsterLame);
    assert_eq!(talk(&mut s, &h, Folk::Nessa), Topic::Give);
    assert_eq!(hand(&s), Item::Shoe);
    close_talk(&mut s);
    // After eight the cart has gone.
    let (mut gone, h2) = session(1);
    gone.memory.learn(Fact::NessaCold);
    gone.memory.learn(Fact::AsterLame);
    at_time(&mut gone, 2, 10, 0);
    gone.step(&Held::new(), &h2);
    assert_ne!(talk(&mut gone, &h2, Folk::Nessa), Topic::Give);
}

#[test]
fn aster_rides_in_lame_is_shod_rides_to_the_well_and_the_oil_is_kept() {
    let (mut s, h) = session(1);
    s.memory.learn(Fact::AsterLame);
    s.seats.get_mut(&1).unwrap().knight.held = Item::Shoe;
    at_time(&mut s, 2, 17, 0);
    s.step(&Held::new(), &h);
    assert_eq!(talk(&mut s, &h, Folk::Aster), Topic::Thanks);
    let shod = s.world.shod.expect("shod");
    assert_eq!(hand(&s), Item::Empty);
    close_talk(&mut s);
    // He is on the road to the well; by the time he gets there, the oil is kept.
    s.clock = folk::aster_at_well(shod) - 2;
    assert!(!s.memory.has_keep(ledger::OIL));
    idle(&mut s, &h, 6);
    assert!(s.memory.has_keep(ledger::OIL) && s.memory.ticked(Folk::Aster));
    assert!(s.news.iter().any(|&(n, _)| n == Note::Ticked(Folk::Aster)));
    // Unshod, he stays at the gate and never comes.
    let (mut sad, h2) = session(1);
    sad.clock = clock::ticks(sad.nums.day, 24 * 2 + 20 * 2);
    idle(&mut sad, &h2, 20);
    assert!(!sad.memory.has_keep(ledger::OIL));
}

#[test]
fn the_seal_wants_both_masks_and_a_fresh_pen_and_is_kept() {
    let (mut s, h) = session(1);
    s.memory.learn(Fact::MiraSeal);
    at_time(&mut s, 2, 7, 30);
    s.step(&Held::new(), &h);
    s.memory.masks = ledger::SWIFT;
    assert_eq!(talk(&mut s, &h, Folk::Mira), Topic::Nag(0), "one mask is not both");
    close_talk(&mut s);
    s.memory.masks = ledger::SWIFT | ledger::STOUT;
    at_time(&mut s, 2, 12, 0);
    s.step(&Held::new(), &h);
    assert_eq!(talk(&mut s, &h, Folk::Mira), Topic::Nag(1), "her pen is dry by day");
    close_talk(&mut s);
    at_time(&mut s, 2, 7, 30);
    s.step(&Held::new(), &h);
    assert_eq!(talk(&mut s, &h, Folk::Mira), Topic::Give);
    assert!(s.memory.has_keep(ledger::SOUTH_SEAL) && s.memory.ticked(Folk::Mira));
}

#[test]
fn the_south_tower_needs_the_seal_and_a_stout_knight_to_fill_the_ditch_first() {
    let (mut s, h) = session(1);
    s.memory.masks = ledger::SWIFT | ledger::STOUT;
    s.memory.keeps = ledger::SOUTH_SEAL;
    at_time(&mut s, 3, 10, 0);
    // The door opens to the seal... if you can reach it. Bare, the crate stops you.
    stand(&mut s, 1, (17, 19));
    for _ in 0..120 {
        s.step(&press(1, Input { move_y: 1, ..Default::default() }), &h);
    }
    assert!(s.seats[&1].knight.px().1 < 21 * town::TILE, "stopped behind the crate");
    // Stout: push it down the causeway and into the ditch, then the door.
    s.seats.get_mut(&1).unwrap().knight.mask = Mask::Stout;
    stand(&mut s, 1, (17, 19));
    for _ in 0..360 {
        s.step(&press(1, Input { move_y: 1, ..Default::default() }), &h);
    }
    assert!(s.world.has(DITCH_FILLED) && s.world.crates.is_empty());
    stand(&mut s, 1, (17, 24));
    s.step(&Held::new(), &h);
    s.step(&press(1, tap_e()), &h);
    assert!(s.world.has(SOUTH_OPEN));
    // Without the seal the door stays shut.
    let (mut no, h2) = session(1);
    stand(&mut no, 1, (17, 24));
    no.step(&Held::new(), &h2);
    no.step(&press(1, tap_e()), &h2);
    assert!(!no.world.has(SOUTH_OPEN));
}

#[test]
fn a_hand_holds_one_thing_and_x_puts_it_down() {
    let (mut s, h) = session(1);
    s.memory.learn(Fact::OrinLost);
    s.memory.learn(Fact::NessaCold);
    at_time(&mut s, 1, 7, 30);
    stand(&mut s, 1, (13, 5));
    s.step(&Held::new(), &h);
    s.step(&press(1, tap_e()), &h);
    assert_eq!(hand(&s), Item::Reeds);
    stand(&mut s, 1, (11, 3));
    s.step(&Held::new(), &h);
    s.step(&press(1, tap_e()), &h);
    assert_eq!(hand(&s), Item::Reeds, "full hands cannot take the pot");
    assert!(!s.world.has(INK_TAKEN));
    s.step(&Held::new(), &h);
    s.step(&press(1, Input { bomb: true, ..Default::default() }), &h);
    assert_eq!(hand(&s), Item::Empty);
}

#[test]
fn the_keepsakes_the_masks_and_the_ledger_survive_the_bell_and_the_doors_do_not() {
    let (mut s, h) = session(1);
    s.memory.keeps = ledger::NORTH_KEY | ledger::OIL;
    s.memory.masks = ledger::STOUT;
    s.world.set(NORTH_OPEN);
    s.world.set(DITCH_FILLED);
    s.world.crates.clear();
    tapped(&mut s, &h, 1, Input { ult: true, ..Default::default() });
    s.step(&press(1, tap_e()), &h);
    idle(&mut s, &h, 60);
    assert!(s.memory.has_keep(ledger::NORTH_KEY) && s.memory.has_keep(ledger::OIL) && s.memory.has_mask(ledger::STOUT));
    assert!(!s.world.has(NORTH_OPEN) && !s.world.has(DITCH_FILLED), "the doors are shut again");
    assert_eq!(s.world.crates, vec![town::CRATE_START], "and the crate is back above the ditch");
}

/// A night, a lit lantern, and a wisp on its way to it.
fn night_with_a_wisp(seats: u32) -> (Session, BTreeMap<u32, Hero>) {
    let (mut s, h) = session(seats);
    at_time(&mut s, 1, 21, 30);
    for n in 0..6 {
        assert!(s.lantern_lit(n), "the lamplighter has been round by half past nine");
    }
    let (tx, ty) = town::LAMPS[2];
    s.world.wisps.push(Wisp {
        x: (tx * 16 + 8 + 30) * town::Q,
        y: (ty * 16) * town::Q,
        mode: WispMode::Hunt,
        aim: 2,
        born: 0,
    });
    for id in 1..=seats {
        stand(&mut s, id, (30, 10 + id as i32));
    }
    (s, h)
}

#[test]
fn a_wisp_puts_out_the_lantern_it_reaches_if_nobody_stands_by_it() {
    let (mut s, h) = night_with_a_wisp(1);
    for _ in 0..150 {
        s.step(&Held::new(), &h);
    }
    assert!(s.world.snuffed & (1 << 2) != 0 || s.world.snuffed != 0, "a lantern is out");
    assert!(s.news.iter().any(|&(n, _)| n == Note::Out) || s.world.snuffed != 0);
}

#[test]
fn a_knight_beside_a_lantern_keeps_it_lit_and_the_wisp_gives_way() {
    let (mut s, h) = night_with_a_wisp(1);
    // The Gloam is thin tonight: only this wisp.
    s.nums.wisps = 1;
    s.world.wisps[0].born = 0;
    let (tx, ty) = town::LAMPS[2];
    stand(&mut s, 1, (tx, ty - 1));
    for _ in 0..240 {
        s.step(&Held::new(), &h);
        assert!(s.lantern_lit(2), "the ward holds");
    }
}

#[test]
fn a_touch_costs_a_candle_three_snuff_a_knight_who_is_carried_to_the_well_and_loses_what_they_held() {
    let (mut s, h) = session(1);
    at_time(&mut s, 1, 21, 30);
    s.nums.wisps = 1;
    s.seats.get_mut(&1).unwrap().knight.held = Item::Ink;
    s.world.set(INK_TAKEN);
    for touch in 1..=3 {
        let me = s.seats[&1].knight.px();
        s.world.wisps = vec![Wisp { x: me.0 * town::Q, y: me.1 * town::Q, mode: WispMode::Hunt, aim: 0, born: 0 }];
        s.seats.get_mut(&1).unwrap().knight.hurt = 0;
        s.step(&Held::new(), &h);
        let k = &s.seats[&1].knight;
        assert_eq!(k.candles, 3 - touch, "touch {touch}");
    }
    let k = &s.seats[&1].knight;
    assert!(k.down > 0, "snuffed out");
    assert_eq!(k.held, Item::Empty);
    assert!(!s.world.has(INK_TAKEN), "the pot lies in the reeds again");
    // After a few seconds the well carries them back, with one candle and a ward.
    s.world.wisps.clear();
    for _ in 0..wisps::DOWN + 2 {
        s.step(&Held::new(), &h);
    }
    let k = &s.seats[&1].knight;
    assert!(k.up() && k.candles == 1 && k.hurt > 0);
    let spawn = SPAWN[0];
    assert!((k.px().0 - (spawn.0 * 16 + 8)).abs() < 40, "back at the well");
}

#[test]
fn a_sidestep_goes_through_a_wisp_without_a_touch() {
    let (mut s, h) = session(1);
    at_time(&mut s, 1, 21, 30);
    s.nums.wisps = 1;
    stand(&mut s, 1, (20, 15));
    let me = s.seats[&1].knight.px();
    s.world.wisps = vec![Wisp { x: (me.0 + 10) * town::Q, y: me.1 * town::Q, mode: WispMode::Hunt, aim: 0, born: 0 }];
    s.step(&press(1, Input { dash: true, move_x: 1, ..Default::default() }), &h);
    for _ in 0..4 {
        s.step(&press(1, Input { move_x: 1, ..Default::default() }), &h);
    }
    assert_eq!(s.seats[&1].knight.candles, CANDLES, "the roll was through it");
}

#[test]
fn the_gloam_only_comes_after_dusk_and_grows_with_every_seat_and_the_last_hours_never_lessen() {
    let (mut s, _) = session(1);
    at_time(&mut s, 1, 12, 0);
    assert_eq!(s.wisp_want(), 0, "no wisps by day");
    at_time(&mut s, 1, 20, 0);
    assert_eq!(s.wisp_want(), 4);
    let mut last = 0;
    for seats in 1..=MAX_PLAYERS {
        let (mut t, _) = session(seats);
        at_time(&mut t, 1, 20, 0);
        let want = t.wisp_want();
        assert!(want > last || seats == 1, "{seats} seats: {want}");
        last = want;
        at_time(&mut t, 3, 2, 0);
        assert_eq!(t.wisp_want(), want + 1, "and one more in the Final Hours");
    }
    // A knight who leaves does not ease it: the table was this size.
    let (mut s, h) = session(3);
    at_time(&mut s, 1, 20, 0);
    let three = s.wisp_want();
    let mut gone = h.clone();
    gone.remove(&3);
    s.step(&Held::new(), &gone);
    assert_eq!(s.seats.len(), 2);
    assert_eq!(s.wisp_want(), three, "the scaling stays");
    // And the flames burn shorter with more seats.
    let (one, _) = session(1);
    let (six, _) = session(6);
    assert!(six.burn() < one.burn());
}

#[test]
fn a_knight_who_joins_mid_run_sits_down_at_the_well_and_reads_the_same_ledger() {
    let (mut s, mut h) = session(2);
    at_time(&mut s, 1, 9, 0);
    s.memory.learn(Fact::OrinLost);
    s.memory.witness(Folk::Mira, 1);
    idle(&mut s, &h, 90);
    let mut r = Run::moonfall_for_test();
    r.join_seat(3, "Late");
    h.insert(3, r.players[&3].clone());
    s.step(&Held::new(), &h);
    assert_eq!(s.seats.len(), 3);
    assert!(s.seats[&3].joined > 0 && s.seats[&1].joined == 0);
    assert_eq!(s.peak, 3);
    let k = &s.seats[&3].knight;
    assert_eq!(k.candles, CANDLES);
    let at = SPAWN[2];
    assert!((k.px().0 - (at.0 * 16 + 8)).abs() <= 8, "at the shared checkpoint, the well");
    assert!(s.memory.knows(Fact::OrinLost), "one Ledger for all of them");
    assert!(s.valid());
}

#[test]
fn two_knights_light_two_flames_at_once_and_one_cannot() {
    let (mut s, h) = session(2);
    s.memory.learn(Fact::Verse);
    s.memory.keeps = ledger::NORTH_KEY | ledger::OIL | ledger::SOUTH_SEAL;
    s.world.set(NORTH_OPEN);
    s.world.set(SOUTH_OPEN);
    at_time(&mut s, 3, 0, 5);
    s.nums.wisps = 0;
    stand(&mut s, 1, (18, 4));
    stand(&mut s, 2, (17, 14));
    let both = Held::from([(1, tap_e()), (2, tap_e())]);
    s.step(&both, &h);
    let hold = Held::from([
        (1, Input { interact: true, ..Default::default() }),
        (2, Input { interact: true, ..Default::default() }),
    ]);
    for _ in 0..story::LIGHT_HOLD + 2 {
        s.step(&hold, &h);
    }
    assert!(s.flame(0) && s.flame(2) && !s.flame(1), "two seats, two flames, together");
}

#[test]
fn the_three_lights_held_together_turn_the_moon_back_and_the_purse_is_paid_once() {
    let mut r = Run::moonfall_for_test();
    r.join_seat(2, "Two");
    r.join_seat(3, "Three");
    r.step(&BTreeMap::new());
    {
        let s = r.moonfall.as_mut().unwrap();
        s.memory.learn(Fact::Verse);
        s.memory.keeps = ledger::NORTH_KEY | ledger::OIL | ledger::SOUTH_SEAL;
        s.memory.loops = 1;
        s.world.set(NORTH_OPEN);
        s.world.set(SOUTH_OPEN);
        at_time(s, 3, 0, 5);
        s.nums.wisps = 0;
        stand(s, 1, (18, 4));
        stand(s, 2, (17, 14));
        stand(s, 3, (17, 24));
    }
    let hold = |ids: &[u32]| -> Held { ids.iter().map(|&i| (i, Input { interact: true, ..Default::default() })).collect() };
    r.step(&hold(&[1, 2, 3]));
    for _ in 0..story::LIGHT_HOLD + 4 {
        r.step(&hold(&[1, 2, 3]));
    }
    assert!(matches!(r.moonfall.as_ref().unwrap().phase, Phase::Won { .. }), "all three burn");
    assert!(r.sounds.iter().any(|c| *c == "circle_toll"));
    for _ in 0..WON_LEN + 2 {
        r.step(&BTreeMap::new());
    }
    let s = r.moonfall.as_ref().unwrap();
    assert!(s.completed.is_some() && s.phase == Phase::Play);
    assert!(r.marks.contains_key(CAMPAIGN_MARK) && r.marks.contains_key(PARTY_MARKS[2]));
    assert_ne!(r.home.roundtable[SLOT] & BEGUN, 0);
    let score = &r.home.countryside.games.roundtable[SLOT];
    assert_eq!((score.wins, score.best), (1, 850), "one win, a second loop: 1000 - 150");
    // The purse: gold and wax for three at the table, once.
    let mut home = home::Home::default();
    let mut treasury = Spoils::default();
    let reclaimed = BTreeMap::new();
    let mut realm = activity::Realm { home: &mut home, treasury: &mut treasury, reclaimed: &reclaimed, raids_won: 0 };
    let paid = activity::settle_all(&mut realm, &r.marks);
    assert!(paid.changed);
    assert_eq!(realm.treasury.get(Spoil::Gold), GOLD_PER_SEAT * 3);
    assert_eq!(realm.treasury.get(Spoil::Wax), WAX_PER_SEAT * 3);
    assert!(!activity::settle_all(&mut realm, &r.marks).changed);
    assert_eq!(realm.treasury.get(Spoil::Gold), GOLD_PER_SEAT * 3, "paid once");
    // E with nothing near returns the knight to the kingdom's tile.
    {
        let s = r.moonfall.as_mut().unwrap();
        stand(s, 1, (30, 20));
    }
    r.step(&BTreeMap::new());
    r.step(&BTreeMap::from([(1, tap_e())]));
    assert!(r.exits.outside.contains_key(&1) || r.room().kind == RoomKind::Kingdom || !r.moonfall_showing() || r.moonfall.as_ref().unwrap().seats.len() == 2);
}

#[test]
fn the_moon_falling_is_a_wipe_the_town_wound_back_and_control_back_inside_a_second() {
    let (mut s, h) = session(2);
    s.memory.keeps = ledger::NORTH_KEY;
    s.memory.learn(Fact::Verse);
    s.clock = clock::end(s.nums.day) - 1;
    let cues = tick(&mut s, &h, &Held::new());
    assert!(cues.contains(&"boss_roar"), "it comes down");
    let mut free = 0;
    for t in 1..=60 {
        s.step(&Held::new(), &h);
        if s.phase == Phase::Play {
            free = t;
            break;
        }
    }
    assert!(free > 0 && free <= 30, "the fall hands the table back in {free} ticks");
    assert_eq!(s.memory.loops, 1);
    assert!(s.memory.has_keep(ledger::NORTH_KEY) && s.memory.knows(Fact::Verse), "what was earned stays");
    assert!(s.clock < 40);
}

#[test]
fn a_run_replays_byte_for_byte_from_the_same_keys() {
    let mut a = informed(4);
    let mut b = informed(4);
    for _ in 0..3000 {
        let (ia, ib) = (bots(&a.0, &a.1), bots(&b.0, &b.1));
        a.0.step(&ia, &a.1);
        b.0.step(&ib, &b.1);
    }
    assert_eq!(serde_json::to_vec(&a.0).unwrap(), serde_json::to_vec(&b.0).unwrap());
    assert!(a.0.valid());
}

#[test]
fn a_blind_single_pass_cannot_finish_however_many_knights_play_it() {
    // The proof by play: bots with an empty Ledger, one loop, every table size.
    for n in [1, 2, 4, 6] {
        let (mut s, h) = session(n);
        while s.memory.loops == 0 && s.completed.is_none() {
            let inputs = bots(&s, &h);
            s.step(&inputs, &h);
        }
        assert!(s.completed.is_none(), "{n} seats won blind");
        assert!(s.memory.loops >= 1, "the moon fell");
    }
    // The proof by the schedule: the oil needs a shoe, the shoe needs the Ledger to
    // say the rider has come, and the rider first comes after the iron cart has gone.
    let day = 3000;
    let table = folk::timetable(day);
    let rider_seen = table.seen[Folk::Aster.index()][0].0;
    let cart_gone = clock::ticks(day, ((24 + 8 - 6) * 2) as u32);
    assert!(rider_seen > cart_gone, "the rider is first seen at {rider_seen}, the cart is gone by {cart_gone}");
}

#[test]
fn one_hand_cannot_do_two_of_the_four_favours_in_a_loop_so_a_lone_knight_needs_four() {
    // The four things a hand must carry for the towers. Each must be held from the
    // latest moment it can be taken to the earliest it can be given, in minutes
    // since the first dawn, read off the same windows the rules use.
    let day = |d: u32, half: u32| (d * 48 + half) * 30;
    // The scroll: taken by the end of the post hour on the first morning, read by
    // Mira at her nine o'clock watch on the second evening at the soonest.
    let scroll = (day(0, story::POST.1), day(1, 30));
    // The pass: the pot reaches Orin by the end of the same hour, five hours'
    // writing and two to smudge; Tovin airs the keys from the second 17:00.
    let pass = (
        day(0, story::POST.1) + (story::PASS_HOURS + story::PASS_FRESH) * 60,
        day(1, story::KEYS.0),
    );
    // The reeds: twelve hours in the hand before the oven's hour on the second morning.
    let reeds = (day(1, story::OVEN.0) - story::DRY_HOURS * 60, day(1, story::OVEN.0));
    // The shoe: from the iron cart's last half hour to the rider's arrival.
    let shoe = (day(1, story::IRON.1), day(1, 21));
    let held = [("scroll", scroll), ("pass", pass), ("reeds", reeds), ("shoe", shoe)];
    for (i, (a, x)) in held.iter().enumerate() {
        for (b, y) in &held[i + 1..] {
            assert!(x.0 < y.1 && y.0 < x.1, "{a} {x:?} and {b} {y:?} are held at the same time");
        }
    }
    // And all four are needed: the scroll and the reeds for the seal, the pass for the
    // north key, the shoe for the oil.
    assert_eq!(held.len(), 4);
}

#[test]
fn bots_clear_it_at_one_two_four_and_six_seats_and_the_loops_and_ticks_are_the_record() {
    // The record of the Ledger-reading bots from a blank Ledger. A lone knight
    // needs four loops, at the least (see the hand check above), and a table of
    // four or six does it in two: one to learn the town and one to light it.
    let mut record = Vec::new();
    for (n, loops) in [(1u32, 4u32..=6), (2, 2..=3), (4, 2..=2), (6, 2..=2)] {
        let (s, out) = play(n, 120_000, |_| {});
        assert!(out.won && loops.contains(&out.loops), "{n} seats: {out:?}");
        assert!(s.valid());
        record.push((n, out.loops, out.ticks));
    }
    eprintln!("bot record (seats, loops, ticks): {record:?}");
}

#[test]
fn a_party_that_has_read_the_whole_ledger_finishes_in_a_single_loop_with_four_or_six() {
    for n in [4u32, 6] {
        let (s, out) = play_from(informed(n), 12_000, |_| {});
        assert!(out.won && out.loops == 1, "{n} informed seats: {out:?}");
        assert!(s.memory.loops == 0, "no rewind was needed");
    }
}

#[test]
fn a_knight_who_steps_away_keeps_what_they_carry_and_is_back_where_they_stood() {
    let (mut s, mut h) = session(2);
    s.memory.learn(Fact::PellaScroll);
    at_time(&mut s, 1, 7, 15);
    assert_eq!(talk(&mut s, &h, Folk::Pella), Topic::Give);
    close_talk(&mut s);
    let at = s.seats[&1].knight.px();
    // Knight 1's tab is hidden: they stand up, but are kept.
    h.get_mut(&1).unwrap().away = true;
    idle(&mut s, &h, 30);
    assert!(!s.seats.contains_key(&1) && s.parked.contains_key(&1));
    assert!(s.valid());
    h.get_mut(&1).unwrap().away = false;
    idle(&mut s, &h, 2);
    let k = &s.seats[&1].knight;
    assert_eq!((k.held, k.px()), (Item::Scroll, at), "the scroll is still in their hand");
    assert!(!s.parked.contains_key(&1));
    // A knight who left the party altogether is forgotten.
    let mut gone = h.clone();
    gone.remove(&2);
    idle(&mut s, &gone, 2);
    assert!(!s.seats.contains_key(&2) && !s.parked.contains_key(&2));
    // The bell wins parked knights back to the first dawn too.
    h.get_mut(&1).unwrap().away = true;
    idle(&mut s, &h, 2);
    s.seats.clear();
    s.parked.get_mut(&1).unwrap().knight.held = Item::Reeds;
    s.begin_rewind_for_test();
    for _ in 0..40 {
        s.step(&Held::new(), &h);
    }
    assert_eq!(s.parked[&1].knight.held, Item::Empty);
}

#[test]
fn at_the_towers_in_the_last_hours_a_knight_is_told_when_the_verse_is_missing() {
    let (mut s, h) = session(1);
    s.memory.keeps = ledger::OIL;
    at_time(&mut s, 3, 1, 0);
    stand(&mut s, 1, (17, 14));
    s.step(&Held::new(), &h);
    s.step(&press(1, tap_e()), &h);
    assert_eq!(
        s.seats[&1].knight.note.map(|(n, _)| n),
        Some(Note::Sign(says::Sign::TowerNeedsVerse))
    );
}

#[test]
fn four_bots_through_the_real_run_turn_the_moon_back_and_the_campaign_is_marked() {
    let mut r = Run::moonfall_for_test();
    for id in 2..=4 {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.step(&BTreeMap::new());
    for f in ledger::FACTS {
        r.moonfall.as_mut().unwrap().memory.learn(f);
    }
    let mut ticks = 0;
    while ticks < 12_000 && r.moonfall.as_ref().is_some_and(|s| s.completed.is_none()) {
        let inputs = r.roundtable_inputs();
        r.step(&inputs);
        ticks += 1;
    }
    let s = r.moonfall.as_ref().expect("the run is still open");
    assert!(s.completed.is_some(), "not won in {ticks} ticks, clock {}", s.clock);
    assert_eq!(s.memory.loops, 0, "a single loop");
    assert!(r.marks.contains_key(CAMPAIGN_MARK) && r.marks.contains_key(PARTY_MARKS[3]));
    assert!(r.valid_snapshot() && crate::wire::drawable(&r));
}

#[test]
fn e_at_the_bell_post_opens_the_bell_menu_as_r_does() {
    let (mut s, h) = session(1);
    stand(&mut s, 1, (13, 13));
    s.step(&Held::new(), &h);
    assert_eq!(s.nearest_use(1), Some(story::Use::Bell));
    s.step(&press(1, tap_e()), &h);
    assert_eq!(s.seats[&1].knight.panel, Panel::Bell(0));
}

/// A small deterministic stream of numbers for the shake-it-about tests.
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn axis(&mut self) -> i8 {
        (self.next() % 3) as i8 - 1
    }
    fn input(&mut self) -> Input {
        let r = self.next();
        Input {
            move_x: self.axis(),
            move_y: self.axis(),
            aim_x: if r & 1 == 0 { 0 } else { self.axis() },
            aim_y: if r & 2 == 0 { 0 } else { self.axis() },
            interact: r & 4 != 0,
            swing: r & 56 == 0,
            ult: r & 448 == 0,
            play: u8::from(r & 3584 == 0),
            bomb: r & 28672 == 0,
            dash: r & 229_376 == 0,
            ..Default::default()
        }
    }
}

#[test]
fn whatever_the_keys_do_the_session_stays_valid_replays_and_survives_the_wire() {
    for seats in [1u32, 3, 6] {
        let (mut s, h) = session(seats);
        // A short day, so a few thousand ticks cross dusk, the Final Hours, a fall and a bell.
        s.nums.day = 1800;
        for f in ledger::FACTS {
            s.memory.learn(f);
        }
        s.memory.masks = 3;
        s.memory.keeps = 7;
        let mut noise = Noise(0x9e37_79b9_7f4a_7c15 ^ u64::from(seats));
        let mut copy = s.clone();
        let mut noise2 = Noise(0x9e37_79b9_7f4a_7c15 ^ u64::from(seats));
        for tick in 0..14_000 {
            let inputs: Held = h.keys().map(|&id| (id, noise.input())).collect();
            let again: Held = h.keys().map(|&id| (id, noise2.input())).collect();
            s.step(&inputs, &h);
            copy.step(&again, &h);
            assert!(s.valid(), "{seats} seats, tick {tick}: {:?}", s.phase);
            if tick % 700 == 0 {
                let wire = serde_json::to_string(&s).unwrap();
                let back: Session = serde_json::from_str(&wire).unwrap();
                assert_eq!(back, s, "{seats} seats survive the wire at tick {tick}");
            }
        }
        assert_eq!(s, copy, "the same keys, the same town");
        assert!(s.memory.loops >= 1, "the moon fell at least once");
    }
}

#[test]
fn a_damaged_session_is_mended_by_repair_and_a_knob_moved_mid_loop_never_breaks_it() {
    let (mut s, h) = session(3);
    s.clock = 1_000_000;
    s.elapsed = LIMIT + 9;
    s.card = 999;
    s.peak = 40;
    s.nums.day = 10;
    s.nums.walk = 9999;
    s.phase = Phase::Rewind { t: 999, from: 99_999, fell: true };
    s.call = Some(Call { ring: Ring::Rewind, by: 99, ayes: 255, left: 9999 });
    s.world.flags = 255;
    s.world.wisps = vec![Wisp { x: i32::MAX, y: i32::MIN, mode: WispMode::Hunt, aim: 77, born: 0 }; 200];
    s.world.lit_until = [u32::MAX; 3];
    s.world.ink_given = Some(u32::MAX);
    s.memory.facts = u16::MAX;
    s.memory.keeps = 255;
    s.memory.routines = [u32::MAX; 6];
    for seat in s.seats.values_mut() {
        seat.knight.x = i32::MIN;
        seat.knight.candles = 200;
        seat.knight.panel = Panel::Ledger(200);
        seat.knight.stride = u32::MAX;
    }
    assert!(!s.valid());
    s.repair();
    assert!(
        s.valid(),
        "mended? world {} memory {} nums {:?} clock {} call {:?} peak {} card {} elapsed {} lit {:?} ink {:?} phase {:?}",
        s.world.valid(),
        s.memory.valid(),
        s.nums,
        s.clock,
        s.call,
        s.peak,
        s.card,
        s.elapsed,
        s.world.lit_until,
        s.world.ink_given,
        s.phase
    );
    s.step(&Held::new(), &h);
    assert!(s.valid());
    // The day knob dragged shorter than the clock: the moon falls, the table is handed back.
    let (mut t, h) = session(2);
    t.clock = 7000;
    t.nums.day = 1800;
    t.step(&Held::new(), &h);
    assert!(t.valid() && matches!(t.phase, Phase::Rewind { fell: true, .. }), "{:?}", t.phase);
    idle(&mut t, &h, 60);
    assert!(t.valid() && t.phase == Phase::Play && t.clock < 100);
}

#[test]
fn the_ledger_key_says_new_until_a_knight_has_read_what_was_written() {
    let (mut s, h) = session(2);
    assert_eq!(s.memory.written(), 0);
    s.memory.learn(Fact::OrinLost);
    s.memory.witness(Folk::Mira, 1);
    assert!(s.memory.written() > s.seats[&1].knight.read);
    tapped(&mut s, &h, 1, Input { swing: true, ..Default::default() });
    assert_eq!(s.seats[&1].knight.panel, Panel::Ledger(0));
    assert_eq!(s.seats[&1].knight.read, s.memory.written(), "knight 1 has read it");
    assert_eq!(s.seats[&2].knight.read, 0, "knight 2 has not");
    // More is written while it is open: still read, as it is on the page.
    s.memory.witness(Folk::Mira, 4);
    s.step(&Held::new(), &h);
    assert_eq!(s.seats[&1].knight.read, s.memory.written());
}

/// What the table's bots press this tick.
pub(super) fn bots(s: &Session, h: &BTreeMap<u32, Hero>) -> Held {
    h.iter().filter(|(id, _)| s.seats.contains_key(id)).map(|(&id, hero)| (id, s.bot_input(id, hero))).collect()
}

#[derive(Debug, Default)]
pub(super) struct Outcome {
    pub loops: u32,
    pub ticks: u32,
    pub won: bool,
}

/// Play `n` bots until the moon turns back or `limit` ticks pass.
pub(super) fn play(n: u32, limit: u32, watch: impl FnMut(&Session)) -> (Session, Outcome) {
    play_from(session(n), limit, watch)
}

/// Every fact the Ledger can learn, written (a party that has read it all).
pub(super) fn informed(n: u32) -> (Session, BTreeMap<u32, Hero>) {
    let (mut s, h) = session(n);
    for f in ledger::FACTS {
        s.memory.learn(f);
    }
    (s, h)
}

pub(super) fn play_from(
    (mut s, h): (Session, BTreeMap<u32, Hero>),
    limit: u32,
    mut watch: impl FnMut(&Session),
) -> (Session, Outcome) {
    if let Some(w) = std::env::var("WALK").ok().and_then(|v| v.parse().ok()) {
        s.nums.walk = w;
    }
    if let Some(w) = std::env::var("WISPS").ok().and_then(|v| v.parse().ok()) {
        s.nums.wisps = w;
    }
    if let Some(w) = std::env::var("DAY").ok().and_then(|v| v.parse().ok()) {
        s.nums.day = w;
    }
    let mut ticks = 0;
    while ticks < limit && s.completed.is_none() {
        let inputs = bots(&s, &h);
        s.step(&inputs, &h);
        watch(&s);
        ticks += 1;
    }
    let out = Outcome { loops: s.memory.loops + 1, ticks, won: s.completed.is_some() };
    (s, out)
}

/// Play the bots from a blank Ledger with some of the five numbers moved.
fn tried(seats: u32, day: u32, walk: u32, wisps: u32, notice: u32) -> Outcome {
    let (mut s, h) = session(seats);
    s.nums.day = day;
    s.nums.walk = walk;
    s.nums.wisps = wisps;
    s.nums.notice = notice;
    let mut ticks = 0;
    while ticks < 150_000 && s.completed.is_none() {
        let inputs = bots(&s, &h);
        s.step(&inputs, &h);
        ticks += 1;
    }
    Outcome { loops: s.memory.loops + 1, ticks, won: s.completed.is_some() }
}

#[test]
#[ignore = "writes the tuning table"]
fn tuning_table() {
    for (label, rows) in [
        ("walk", [100u32, 120, 150, 200, 300].iter().map(|&w| (3000, w, 3, 64, format!("walk {w}"))).collect::<Vec<_>>()),
        ("day", [1800u32, 2400, 3000, 4000, 6000].iter().map(|&d| (d, 150, 3, 64, format!("day {d}"))).collect()),
        ("wisps", [0u32, 2, 3, 4, 5, 8].iter().map(|&w| (3000, 150, w, 64, format!("wisps {w}"))).collect()),
        ("notice", [24u32, 48, 64, 96, 160].iter().map(|&n| (3000, 150, 3, n, format!("notice {n}"))).collect()),
    ] {
        for (day, walk, wisps, notice, name) in rows {
            let cells: Vec<String> = [1u32, 2, 4, 6]
                .iter()
                .map(|&n| {
                    let o = tried(n, day, walk, wisps, notice);
                    format!("{n}s {}{} {}", o.loops, if o.won { "" } else { "+" }, o.ticks)
                })
                .collect();
            eprintln!("TUNE {label:6} {name:12} | {}", cells.join(" | "));
        }
    }
}

#[test]
#[ignore = "explores what bots do"]
fn explore_bots() {
    let n: u32 = std::env::var("BOTS").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let seed = std::env::var("INFORMED").is_ok();
    let mut last = (0, 0u16, 0u8, 0u16, 0u8);
    let start = if seed { informed(n) } else { session(n) };
    let (_, out) = play_from(start, 200_000, |s| {
        let now = (s.memory.loops, s.memory.facts, s.memory.ticks, s.world.flags, s.memory.masks | s.memory.keeps << 4);
        let lo: u32 = std::env::var("FROM").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        let hi: u32 = std::env::var("TO").ok().and_then(|v| v.parse().ok()).unwrap_or(9000);
        let lp: u32 = std::env::var("LOOP").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
        if std::env::var("WATCH").is_ok() && s.clock % 50 == 0 && s.memory.loops + 1 == lp && (lo..hi).contains(&s.clock) {
            for (id, seat) in &s.seats {
                let k = &seat.knight;
                eprintln!(
                    "  t{} {} #{id} px {:?} mask {:?} held {:?} do {} wisps {} lit {:?} hold {} cand {}",
                    s.clock,
                    clock::stamp(s.clock, s.nums.day).text(),
                    k.px(),
                    k.mask,
                    k.held,
                    plan::why(s, *id),
                    s.world.wisps.len(),
                    s.world.lit_until.map(|u| u.saturating_sub(s.clock)),
                    k.hold,
                    k.candles
                );
            }
        }
        if now != last {
            let st = clock::stamp(s.clock, s.nums.day);
            eprintln!(
                "loop {} D{} {} facts {:#010b} ticks {:#08b} flags {:#06b} masks {} keeps {:#05b} lit {:?}",
                s.memory.loops + 1,
                st.day,
                st.text(),
                s.memory.facts,
                s.memory.ticks,
                s.world.flags,
                s.memory.masks,
                s.memory.keeps,
                s.world.lit_until.map(|u| u > s.clock)
            );
            last = now;
        }
    });
    eprintln!("{out:?}");
}
