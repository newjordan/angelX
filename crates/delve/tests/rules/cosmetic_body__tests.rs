//! A mirror look drawn whole (`Look::body`): the worn ink off the knight and
//! saved, the card's words always the words it is drawn from, nothing spent.

use super::*;
use crate::rules::host_commands::{Command, Journal, Reply, Request};

fn run() -> Run {
    Run::new(4, 2, Some("Matt"))
}

/// The mirror wish as a knight makes it: the handoff, the words, the pick.
fn wished(run: &mut Run, id: u32, words: &str) {
    run.begin_mirror_wish(id).unwrap();
    run.offer_wish(id, words, None).unwrap();
    run.choose(id, 0).unwrap();
}

/// The worn painter's answer for the knight's look: one ink pixel.
fn painted(run: &mut Run, id: u32, words: &str, ink: &str) {
    let mut card = run.mirror_paint_card(id, words).unwrap();
    card.looks.worn = Some(looks::Sprite { frames: vec![vec![ink.into()]], ..Default::default() });
    assert!(run.update_mirror_paint(&card));
}

fn worn(run: &Run, id: u32) -> usize {
    run.players[&id].worn(&run.book).len()
}

fn gameplay(run: &Run) -> serde_json::Value {
    let mut value = serde_json::to_value(&run.players[&1]).unwrap();
    let h = value.as_object_mut().unwrap();
    for field in ["offer", "mirror_wish"] {
        h.remove(field);
    }
    value
}

#[test]
fn a_body_drawn_whole_takes_the_worn_ink_off_the_knight_and_keeps_it_saved() {
    let mut run = run();
    let name = run.players[&1].name.clone();
    wished(&mut run, 1, "a frog knight");
    painted(&mut run, 1, "a frog knight", "7");
    assert_eq!(worn(&run, 1), 1, "the painter's ink is on the knight");
    assert!(run.mirror_bodies().is_empty() && !run.has_mirror_body(1));

    // A result for other words is a late one: nothing changes.
    assert!(!run.set_mirror_body(&key(&name), "a red knight", true));
    assert_eq!(worn(&run, 1), 1);

    assert!(run.set_mirror_body(&key(&name), "a frog knight", true));
    assert_eq!(worn(&run, 1), 0, "the rig's plain knight is what the room stamps");
    assert_eq!(run.mirror_bodies(), vec![(key(&name), 1)]);
    assert!(run.has_mirror_body(1));
    let kept = &run.home.wardrobe.mirror_looks[&name];
    assert_eq!(kept.worn.frames, vec![vec!["7".to_string()]], "the ink stays saved");
    assert!(!run.set_mirror_body(&key(&name), "a frog knight", true), "nothing to change twice");

    // A later answer of the worn painter for the same words keeps the body.
    painted(&mut run, 1, "a frog knight", "8");
    assert!(run.has_mirror_body(1));
    assert_eq!(worn(&run, 1), 0);

    // Off: the ink is the knight's look again.
    assert!(run.set_mirror_body(&key(&name), "", false));
    assert_eq!(worn(&run, 1), 1);
    assert!(run.mirror_bodies().is_empty() && !run.has_mirror_body(1));
}

#[test]
fn a_new_wish_never_wears_the_old_wishs_body() {
    let mut run = run();
    let name = run.players[&1].name.clone();
    wished(&mut run, 1, "a frog knight");
    painted(&mut run, 1, "a frog knight", "7");
    assert!(run.set_mirror_body(&key(&name), "a frog knight", true));
    assert_eq!(worn(&run, 1), 0);

    wished(&mut run, 1, "a red knight");
    assert!(!run.has_mirror_body(1), "the body was of the old words");
    assert_eq!(run.home.wardrobe.mirror_looks[&name].words, "a red knight");
    assert_eq!(worn(&run, 1), 1, "the saved ink is on the knight until the new body lands");
    let card = run.book.get(&key(&name)).unwrap();
    assert_eq!(card.text, "a red knight", "the card's text is the words the knight is drawn from");
    // The old job's answer, landing late, is turned away.
    assert!(!run.set_mirror_body(&key(&name), "a frog knight", true));
    assert!(!run.has_mirror_body(1));
    // The new words' own answer is taken.
    assert!(run.set_mirror_body(&key(&name), "a red knight", true));
    assert!(run.has_mirror_body(1));
}

#[test]
fn the_same_words_again_leave_the_knight_the_look_it_already_is() {
    let mut run = run();
    let name = run.players[&1].name.clone();
    wished(&mut run, 1, "a frog knight");
    painted(&mut run, 1, "a frog knight", "7");
    assert!(run.set_mirror_body(&key(&name), "a frog knight", true));
    wished(&mut run, 1, "a frog knight");
    assert!(run.has_mirror_body(1), "already that look");
    assert_eq!(worn(&run, 1), 0);
    assert_eq!(run.book.get(&key(&name)).unwrap().text, "a frog knight");
    // Other words are another look.
    wished(&mut run, 1, "a frog knight, but red");
    assert!(!run.has_mirror_body(1));
}

#[test]
fn a_body_spends_and_grants_nothing() {
    let mut run = run();
    run.players.get_mut(&1).unwrap().wishes = 3;
    run.players.get_mut(&1).unwrap().great_wishes = 2;
    let name = run.players[&1].name.clone();
    wished(&mut run, 1, "a frog knight");
    // The mirror's ask never owns a spent wish: the host takes it off the queue.
    assert!(run.asked.iter().all(|a| a.cosmetic));
    let (before, genie, wishes) = (gameplay(&run), run.genie.clone(), run.players[&1].wishes);
    assert!(run.set_mirror_body(&key(&name), "a frog knight", true));
    assert_eq!(gameplay(&run), before);
    assert_eq!((run.genie.clone(), run.players[&1].wishes), (genie, wishes));
    assert!(run.asked.iter().all(|a| a.cosmetic));
    let card = run.book.get(&key(&name)).unwrap();
    assert!(card.effects.is_empty() && card.arm.is_none() && (card.drop, card.chest) == (0, 0));
    assert!(run.players[&1].deck.iter().all(|id| id != &card.id), "the look is outside the deck");
}

#[test]
fn the_body_is_saved_with_the_look_and_an_old_save_has_none() {
    let mut run = run();
    let name = run.players[&1].name.clone();
    wished(&mut run, 1, "a frog knight");
    let off = serde_json::to_value(&run.home.wardrobe).unwrap();
    assert!(!off.to_string().contains("\"body\""), "no body, no word in the save");
    painted(&mut run, 1, "a frog knight", "7");
    assert!(run.set_mirror_body(&key(&name), "a frog knight", true));
    let saved = serde_json::to_string(&run.home.wardrobe).unwrap();
    assert!(saved.contains("\"body\":true"));

    // A restart: the same wardrobe, the same knight, drawn whole again.
    let mut next = self::run();
    next.home.wardrobe = serde_json::from_str(&saved).unwrap();
    next.restore_gear(1);
    assert!(next.has_mirror_body(1));
    assert_eq!(next.mirror_bodies(), vec![(key(&name), 1)]);
    assert_eq!(worn(&next, 1), 0);

    // A guest's snapshot of the run agrees.
    let guest: Run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
    assert_eq!(guest.mirror_bodies(), run.mirror_bodies());
    assert_eq!(guest.players[&1].worn(&guest.book).len(), 0);

    // A save from before it (no `body`) reads as no body, with its ink.
    let mut old: serde_json::Value = serde_json::from_str(&saved).unwrap();
    old["mirror_looks"][name.as_str()].as_object_mut().unwrap().remove("body");
    let mut older = self::run();
    older.home.wardrobe = serde_json::from_value(old).unwrap();
    older.restore_gear(1);
    assert!(!older.has_mirror_body(1) && older.mirror_bodies().is_empty());
    assert_eq!(worn(&older, 1), 1);
}

#[test]
fn the_roster_is_the_seated_knights_drawn_whole_by_seat() {
    let mut run = run();
    run.join_seat(2, "Goose Inspector");
    let (matt, goose) = (run.players[&1].name.clone(), run.players[&2].name.clone());
    for (id, words) in [(1, "a frog knight"), (2, "a heron knight")] {
        wished(&mut run, id, words);
    }
    assert!(run.mirror_bodies().is_empty());
    assert!(run.set_mirror_body(&key(&goose), "a heron knight", true));
    assert_eq!(run.mirror_bodies(), vec![(key(&goose), 2)], "seat 2 only");
    assert!(!run.has_mirror_body(1) && run.has_mirror_body(2));
    assert!(run.set_mirror_body(&key(&matt), "a frog knight", true));
    assert_eq!(run.mirror_bodies(), vec![(key(&matt), 1), (key(&goose), 2)]);
}

#[test]
fn a_drawn_knight_is_on_the_64_rung_whatever_the_floor() {
    use crate::rules::resolution::{self, Rung};
    let mut run = run();
    let name = run.players[&1].name.clone();
    run.join_seat(2, "Goose Inspector");
    wished(&mut run, 1, "a frog knight");
    assert_eq!((resolution::drawn_rung(&run, 1), resolution::rung(&run, 1)), (Rung::BASE, Rung::BASE), "floor one");
    assert!(run.set_mirror_body(&key(&name), "a frog knight", true));
    assert_eq!(resolution::drawn_rung(&run, 1), resolution::BODY_RUNG);
    assert_eq!(resolution::rung(&run, 1), Rung::BASE, "the rung a floor gives is not moved");
    assert_eq!(resolution::drawn_rung(&run, 2), Rung::BASE, "the other seat is as it was");
    // A finer rung is the knight's own.
    resolution::set_rule(Some(resolution::Rule::Fixed(Rung::of(128).unwrap())));
    assert_eq!(resolution::drawn_rung(&run, 1), Rung::of(128).unwrap());
    resolution::set_rule(None);
}

#[test]
fn the_host_tells_the_run_through_the_journal_and_a_stranger_cannot() {
    let mut run = run();
    let name = run.players[&1].name.clone();
    wished(&mut run, 1, "a frog knight");
    let mut journal = Journal::new("session-a".into(), 2);
    let ask = |id: u64, command: Command| Request { session: "session-a".into(), raid: 2, id, commands: vec![command] };
    let on = Command::MirrorBody { card: key(&name), words: "a frog knight".into(), on: true };
    let receipt = journal.apply(&mut run, ask(1, on)).unwrap();
    assert_eq!(receipt.result, Ok(vec![Reply::Changed(true)]));
    assert!(run.has_mirror_body(1));
    // A card id that is not a mirror look's is refused outright.
    let not = Command::MirrorBody { card: "iron-helm".into(), words: "x".into(), on: true };
    let refused = journal.apply(&mut run, ask(2, not)).unwrap();
    assert!(refused.result.is_err(), "a receipt that says no: {refused:?}");
    assert!(run.has_mirror_body(1), "and the run is as it was");
    let off = Command::MirrorBody { card: key(&name), words: String::new(), on: false };
    let receipt = journal.apply(&mut run, ask(3, off)).unwrap();
    assert_eq!(receipt.result, Ok(vec![Reply::Changed(true)]));
    assert!(!run.has_mirror_body(1));
}
