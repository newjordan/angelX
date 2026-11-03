//! A knight's hand rearranged from their seat: a card dragged onto another
//! slot (`Run::move_card`), and the next-card key that puts the first at
//! the back (`Hero::cycle_hand`).

use super::*;

/// A calm, cleared fight room, the knight holding three cards.
fn holding() -> Run {
    let mut run = Run::new(4, 1, None);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.open_room_for_test(room);
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    run.window = None;
    run.works.perils.clear();
    let hero = run.players.get_mut(&1).unwrap();
    hero.invulnerable = 100_000;
    hero.hand = hand(&["potion", "holy-water", "thunder-scroll"]);
    run
}

fn hand(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| id.to_string()).collect()
}

fn keys(cycle: bool) -> BTreeMap<u32, Input> {
    BTreeMap::from([(1, Input { cycle, ..Default::default() })])
}

#[test]
fn a_card_dragged_onto_another_slot_takes_it_and_the_rest_close_up() {
    let mut run = holding();
    // The last card onto the first: it is next.
    assert!(run.move_card(1, 2, 0));
    assert_eq!(run.players[&1].hand, hand(&["thunder-scroll", "potion", "holy-water"]));
    // The first onto the last: the others move up.
    assert!(run.move_card(1, 0, 2));
    assert_eq!(run.players[&1].hand, hand(&["potion", "holy-water", "thunder-scroll"]));
    // The middle onto the first.
    assert!(run.move_card(1, 1, 0));
    assert_eq!(run.players[&1].hand, hand(&["holy-water", "potion", "thunder-scroll"]));
}

#[test]
fn a_move_off_the_hand_or_onto_itself_or_for_nobody_is_refused() {
    let mut run = holding();
    let before = run.players[&1].hand.clone();
    assert!(!run.move_card(1, 1, 1), "onto its own slot");
    assert!(!run.move_card(1, 3, 0), "from past the hand");
    assert!(!run.move_card(1, 0, 3), "onto past the hand");
    assert!(!run.move_card(1, usize::MAX, 0));
    assert!(!run.move_card(9, 0, 1), "no such knight");
    assert_eq!(run.players[&1].hand, before);
    // Each seat's own hand only: a friend's stays as it was.
    let mut run = Run::new(4, 1, Some("Friend"));
    run.players.get_mut(&1).unwrap().hand = hand(&["potion", "holy-water"]);
    run.players.get_mut(&2).unwrap().hand = hand(&["potion", "holy-water"]);
    assert!(run.move_card(2, 1, 0));
    assert_eq!(run.players[&2].hand, hand(&["holy-water", "potion"]));
    assert_eq!(run.players[&1].hand, hand(&["potion", "holy-water"]));
}

#[test]
fn the_next_card_key_puts_the_first_at_the_back_once_a_press() {
    let mut run = holding();
    // Held for a while, it turns the hand once.
    for _ in 0..10 {
        run.step(&keys(true));
    }
    assert_eq!(run.players[&1].hand, hand(&["holy-water", "thunder-scroll", "potion"]));
    // Let go and pressed again, once more.
    run.step(&keys(false));
    run.step(&keys(true));
    run.step(&keys(false));
    assert_eq!(run.players[&1].hand, hand(&["thunder-scroll", "potion", "holy-water"]));
    // The quaff key then drinks the one the key brought up.
    let quaff = BTreeMap::from([(1, Input { play: 1, ..Default::default() })]);
    run.step(&quaff);
    assert_eq!(run.players[&1].hand, hand(&["potion", "holy-water"]));
    // A hand of one, or none, has nothing to turn.
    let hero = run.players.get_mut(&1).unwrap();
    hero.hand = hand(&["potion"]);
    assert!(!hero.cycle_hand());
    hero.hand.clear();
    assert!(!hero.cycle_hand());
}

#[test]
fn the_hud_shows_the_next_card_key_under_the_second_card() {
    let mut run = holding();
    let hud = run.hud(1, &hud::Keys::FIRST).unwrap();
    let keys: Vec<&str> = hud.hand.iter().flatten().map(|s| s.key.as_str()).collect();
    assert_eq!(keys, ["Z", "P", ""]);
    assert!(hud::Keys::FIRST.kit_line().contains("P next card"));
    assert!(hud::Keys::SECOND.kit_line().contains("End next card"));
    // A hand of one has no second to bring up.
    run.players.get_mut(&1).unwrap().hand = hand(&["potion"]);
    let tray = run.tray(1, &hud::Keys::FIRST);
    let cards: Vec<&str> = tray.iter().filter(|i| i.slot == tray::Slot::Card).map(|i| i.key.as_str()).collect();
    assert_eq!(cards, ["Z"]);
}

#[test]
fn a_save_and_a_command_from_before_the_next_card_key_still_load() {
    // A knight saved before the key's latch was kept: the latch up is left
    // out of a save, so a save is as it was before the key.
    let run = holding();
    let hero = serde_json::to_value(&run.players[&1]).unwrap();
    assert!(!hero.as_object().unwrap().contains_key("cycle_key"));
    let back: Hero = serde_json::from_value(hero).unwrap();
    assert_eq!(back.hand, run.players[&1].hand);
    assert!(!back.cycle_key);
    // The key unpressed is left out, so an older host reads the rest; an
    // older seat's input, without it, reads as unpressed.
    assert!(!serde_json::to_string(&Input::default()).unwrap().contains("cycle"));
    assert!(serde_json::to_string(&Input { cycle: true, ..Default::default() }).unwrap().contains("\"cycle\":true"));
    let old: Input = serde_json::from_str(
        r#"{"move_x":0,"move_y":0,"aim_x":0,"aim_y":0,"fire":false,"dash":false,"bomb":false}"#,
    )
    .unwrap();
    assert!(!old.cycle);
}
