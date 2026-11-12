//! The hall answers the knights by name: the keeper's greeting and pours,
//! the neighbour's toast, the vendor's sale; the hearth stoked by a knight's
//! hands; a place's name on coming into it. Each line is told by something
//! that happened.
use super::*;
use crate::rules::alehouse::{self, PATRONS};
use crate::rules::{bazaar, world};

fn start(seats: u32) -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    for id in 3..=seats {
        r.join_seat(id, &format!("Seat {id}"));
    }
    r.players.retain(|&id, _| id <= seats.max(1));
    r.walk_alehouse();
    r.floor_plates.clear();
    r
}

fn place(r: &mut Run, id: u32, (x, y): (f32, f32)) {
    let h = r.players.get_mut(&id).unwrap();
    (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
}

fn press(id: u32) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, Input { interact: true, ..Default::default() })])
}

fn tap(r: &mut Run, id: u32, at: (f32, f32)) {
    place(r, id, at);
    r.step(&BTreeMap::new());
    r.step(&press(id));
    r.step(&BTreeMap::new());
}

fn hold(r: &mut Run, id: u32, at: (f32, f32)) {
    place(r, id, at);
    r.step(&BTreeMap::new());
    for _ in 0..crate::rules::home::BUY_HOLD + 3 {
        r.step(&press(id));
    }
    r.step(&BTreeMap::new());
}

fn said_by(r: &Run, voice: Voice) -> Option<String> {
    r.alehouse.calls.iter().find(|c| c.voice == voice).map(|c| c.text.clone())
}

/// A knight's name as a line carries it.
fn called(r: &Run, id: u32) -> String {
    r.players[&id].name.to_uppercase()
}

fn patron(kit: &str) -> usize {
    PATRONS.iter().position(|p| p.kit == kit).unwrap()
}

#[test]
fn the_keeper_greets_a_knight_who_comes_to_the_bar_by_name_and_once() {
    let mut r = start(1);
    for _ in 0..10 {
        r.step(&BTreeMap::new());
    }
    assert!(r.alehouse.calls.is_empty(), "nobody at the bar yet, nothing said");
    place(&mut r, 1, alehouse::TAPS[1]);
    r.step(&BTreeMap::new());
    let line = said_by(&r, Voice::Keeper).expect("the keeper speaks");
    assert!(line.contains(&called(&r, 1)), "by name: {line}");
    assert!(r.speaking(Voice::Keeper));
    // The line goes in a few seconds, and he does not say it twice.
    for _ in 0..CALL_TICKS + 5 {
        r.step(&BTreeMap::new());
    }
    assert!(said_by(&r, Voice::Keeper).is_none());
    for _ in 0..60 {
        r.step(&BTreeMap::new());
    }
    assert!(said_by(&r, Voice::Keeper).is_none(), "once for a knight at the bar");
    assert!(r.valid_snapshot());
}

#[test]
fn the_keeper_greets_the_company_one_knight_at_a_time_in_turn() {
    let mut r = start(3);
    for (id, tap_at) in [(1, alehouse::TAPS[0]), (2, alehouse::TAPS[1]), (3, alehouse::TAPS[2])] {
        place(&mut r, id, tap_at);
    }
    let mut heard = Vec::new();
    for _ in 0..4 * (CALL_TICKS + 4) {
        r.step(&BTreeMap::new());
        if let Some(line) = said_by(&r, Voice::Keeper) {
            if heard.last() != Some(&line) {
                heard.push(line);
            }
        }
    }
    assert_eq!(heard.len(), 3, "each knight greeted in turn: {heard:?}");
    let names: Vec<String> = r.players.values().map(|h| h.name.to_uppercase()).collect();
    for name in &names {
        assert!(heard.iter().any(|l| l.contains(name.as_str())), "{name} greeted: {heard:?}");
    }
}

#[test]
fn a_knight_with_a_calling_is_greeted_by_it() {
    let mut r = start(1);
    tap(&mut r, 1, (PATRONS[patron("scryglass")].at.0, PATRONS[patron("scryglass")].at.1 - 1.0));
    assert_eq!(r.players[&1].knight.as_deref(), Some("scryglass"));
    r.alehouse.calls.clear();
    for _ in 0..CALL_TICKS + 2 {
        r.step(&BTreeMap::new());
    }
    place(&mut r, 1, alehouse::TAPS[1]);
    r.step(&BTreeMap::new());
    let line = said_by(&r, Voice::Keeper).unwrap();
    assert!(line.contains(&format!("{} THE MAGE", called(&r, 1))), "{line}");
}

#[test]
fn taking_a_calling_up_has_the_next_bench_raise_a_mug_to_its_knight() {
    let mut r = start(1);
    let mage = patron("scryglass");
    tap(&mut r, 1, (PATRONS[mage].at.0, PATRONS[mage].at.1 - 1.0));
    let toast = r
        .alehouse
        .calls
        .iter()
        .find(|c| matches!(c.voice, Voice::Patron(_)))
        .expect("a patron toasts");
    assert_eq!(toast.text, format!("TO {}, THE MAGE!", called(&r, 1)));
    let Voice::Patron(j) = toast.voice else { unreachable!() };
    let j = usize::from(j);
    assert_ne!(j, mage);
    assert!(r.patron_taken(j, 0).is_none(), "one who is still on their bench");
    let (a, b) = (PATRONS[mage].at, PATRONS[j].at);
    assert!((a.0 - b.0).hypot(a.1 - b.1) < 3.0, "beside the bench taken");
    assert!(r.sounds.contains(&"hit_note"), "a mug clinks");
}

#[test]
fn a_pour_is_called_to_the_knight_who_paid_and_a_refusal_is_not() {
    let mut r = start(1);
    r.step(&BTreeMap::new());
    r.alehouse.calls.clear();
    r.alehouse.greeted = u32::MAX;
    r.players.get_mut(&1).unwrap().carried = Default::default();
    hold(&mut r, 1, alehouse::TAPS[0]);
    assert!(said_by(&r, Voice::Keeper).is_none(), "no gold, no pour, nothing called");
    r.players.get_mut(&1).unwrap().carried.add(Spoil::Gold, 100);
    hold(&mut r, 1, alehouse::TAPS[0]);
    assert_eq!(said_by(&r, Voice::Keeper), Some(format!("MILD COURAGE FOR {}!", called(&r, 1))));
}

#[test]
fn a_vendor_says_who_bought_and_what() {
    let mut r = start(1);
    r.step(&BTreeMap::new());
    let to = world::room_of(&r.dungeon, RoomKind::Bazaar).unwrap();
    r.arrive(to, bazaar::ARRIVAL);
    r.step(&BTreeMap::new());
    r.players.get_mut(&1).unwrap().carried.add(Spoil::Gold, 500);
    let slot = 4u8;
    hold(&mut r, 1, bazaar::stand_of(slot).unwrap());
    let ware = r.bazaar_stock()[usize::from(slot)].curio().name.to_uppercase();
    let line = said_by(&r, Voice::Vendor(1)).expect("the vendor speaks");
    assert_eq!(line, format!("{}: {ware} SOLD. NO REFUNDS.", called(&r, 1)));
    assert_eq!(Voice::Vendor(1).room(), RoomKind::Bazaar);
    assert_eq!(r.calls_up().count(), 1, "up in the bazaar");
    r.alehouse.calls.iter_mut().for_each(|c| c.at = 0);
    r.tick += CALL_TICKS;
    assert_eq!(r.calls_up().count(), 0, "gone when its few seconds are up");
}

#[test]
fn the_fence_says_who_bought_too() {
    let mut r = start(1);
    r.step(&BTreeMap::new());
    r.players.get_mut(&1).unwrap().carried.add(Spoil::Gold, 500);
    hold(&mut r, 1, (alehouse::FENCE_AT.0 + 1.4, alehouse::FENCE_AT.1));
    let line = said_by(&r, Voice::Fence).expect("the fence speaks");
    assert!(line.starts_with(&format!("{}: ", called(&r, 1))) && line.ends_with("SOLD. NO REFUNDS."), "{line}");
}

#[test]
fn a_knights_hands_at_the_hearth_stoke_it_and_it_settles() {
    let mut r = start(1);
    for _ in 0..40 {
        r.step(&BTreeMap::new());
    }
    assert_eq!(r.alehouse.stoke(r.tick), 0.0, "banked at rest");
    tap(&mut r, 1, alehouse::WARM_AT);
    let lit = r.alehouse.stoke(r.tick);
    assert!(lit > 0.9, "it leaps at once: {lit}");
    for _ in 0..STOKE_TICKS / 2 {
        r.step(&BTreeMap::new());
    }
    let half = r.alehouse.stoke(r.tick);
    assert!(half > 0.0 && half < lit, "settling: {half}");
    for _ in 0..STOKE_TICKS {
        r.step(&BTreeMap::new());
    }
    assert_eq!(r.alehouse.stoke(r.tick), 0.0, "settled");
}

#[test]
fn the_place_is_named_on_coming_into_it_for_three_seconds() {
    let mut r = start(2);
    let (name, line) = r.place_title().unwrap();
    assert_eq!(name, "THE SOZZLED GRIFFIN");
    let free = line.split(' ').next().unwrap().parse::<usize>().unwrap();
    assert!(free > 20, "{line}");
    assert_eq!(r.title_age(), Some(0));
    for _ in 0..TITLE_TICKS {
        r.step(&BTreeMap::new());
    }
    assert_eq!(r.title_age(), None, "gone after its moment");
    // A calling taken up is one fewer free on the next coming-in.
    let mage = patron("scryglass");
    tap(&mut r, 1, (PATRONS[mage].at.0, PATRONS[mage].at.1 - 1.0));
    let to = world::room_of(&r.dungeon, RoomKind::Bazaar).unwrap();
    r.arrive(to, bazaar::ARRIVAL);
    assert_eq!(r.place_title().unwrap().0, "THE BAZAAR");
    assert_eq!(r.title_age(), Some(0));
    let back = world::room_of(&r.dungeon, RoomKind::Alehouse).unwrap();
    r.arrive(back, alehouse::FROM_BAZAAR);
    let after = r.place_title().unwrap().1.split(' ').next().unwrap().parse::<usize>().unwrap();
    assert_eq!(after, free - 1);
    // The keeper greets afresh when the party has come back.
    assert_eq!(r.alehouse.greeted, 0);
}

#[test]
fn the_gate_courtyard_is_named_too() {
    let mut r = start(1);
    r.come_to_the_gate();
    assert_eq!(r.room().kind, RoomKind::Gate);
    assert_eq!(r.place_title().unwrap().0, "THE DELVE'S GATE");
    assert_eq!(r.title_age(), Some(0));
}

#[test]
fn the_talk_survives_a_save_and_an_old_save_without_it_still_loads() {
    let mut r = start(1);
    place(&mut r, 1, alehouse::TAPS[1]);
    r.step(&BTreeMap::new());
    r.alehouse.stoked = r.tick;
    assert!(!r.alehouse.calls.is_empty());
    let json = serde_json::to_string(&r.alehouse).unwrap();
    let back: alehouse::House = serde_json::from_str(&json).unwrap();
    assert_eq!(back, r.alehouse);
    assert!(back.valid());
    // A house saved before any of it carries none of the new fields.
    let old: alehouse::House = serde_json::from_str(r#"{"funded":3,"said":{}}"#).unwrap();
    assert!(old.calls.is_empty() && old.greeted == 0 && old.stoked == 0 && old.came == 0);
    assert!(old.valid());
    // A line over the length is not a valid house.
    let mut bad = back.clone();
    bad.calls[0].text = "X".repeat(MOST_LETTERS + 1);
    assert!(!bad.valid());
}
