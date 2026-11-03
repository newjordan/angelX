use super::*;
use crate::realm::Spoils;
use crate::rules::home::Home;
use std::collections::BTreeMap;

const NORTH: usize = 0;

/// `seats` knights in a cleared room of the first floor that has a doorway
/// north (and, when there is one, south): nothing to fight, nothing in the way. Returns the run, the room's
/// number and the room beyond the doorway.
fn hall(seats: u32) -> (Run, usize, usize) {
    let mut run = Run::new(31, 1, Some("Friend"));
    for id in 3..=seats {
        run.join_seat(id, &format!("Seat {id}"));
    }
    run.descend_for_test();
    // One with a doorway south as well, for the knight who waits in another.
    let (room, next) = (0..run.dungeon.rooms.len())
        .find_map(|i| run.dungeon.neighbour(i, 2).and(run.dungeon.neighbour(i, NORTH)).map(|n| (i, n)))
        .or_else(|| (0..run.dungeon.rooms.len()).find_map(|i| run.dungeon.neighbour(i, NORTH).map(|n| (i, n))))
        .expect("a room with a doorway north");
    run.enter_for_test(room);
    run.hallowed = None;
    run.calm_for_test();
    run.enemies.clear();
    run.dungeon.rooms[room].cleared = true;
    run.phase = Phase::Exploring;
    for id in 1..=seats {
        middle(&mut run, id);
    }
    (run, room, next)
}

/// Knight `id` in the middle of the room.
fn middle(run: &mut Run, id: u32) {
    let (w, h) = (run.room().width(), run.room().height());
    let hero = run.players.get_mut(&id).unwrap();
    (hero.x, hero.y) = (w / 2.0, h / 2.0 - 2.0);
}

/// Knight `id` in the doorway `dir`.
fn in_the_doorway(run: &mut Run, id: u32, dir: usize) {
    let (w, h) = (run.room().width(), run.room().height());
    let (x, y) = [(w / 2.0, 0.3), (w - 0.3, h / 2.0), (w / 2.0, h - 0.3), (0.3, h / 2.0)][dir];
    let hero = run.players.get_mut(&id).unwrap();
    (hero.x, hero.y) = (x, y);
}

fn play(run: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        run.step(&BTreeMap::new());
    }
}

#[test]
fn one_knight_in_the_doorway_waits_for_the_other() {
    let (mut run, room, _) = hall(2);
    in_the_doorway(&mut run, 1, NORTH);
    let at = (run.players[&1].x, run.players[&1].y);
    play(&mut run, 6);
    assert_eq!(run.at, room, "the party stays while a friend is not in the door");
    assert!(run.waiting_for_fellowship());
    assert_eq!(run.fellowship.waiting, vec![(1, 0)]);
    assert_eq!((run.players[&1].x, run.players[&1].y), at, "nothing pushes the one who waits");
    // The waiting are told to wait, the rest where their friends are.
    assert_eq!(run.fellowship_line(Some(1)), Some(("WAITING FOR FELLOWSHIP".into(), true)));
    assert_eq!(run.fellowship_line(Some(2)), Some(("FRIENDS WAIT AT THE NORTH DOOR".into(), false)));
}

#[test]
fn the_party_moves_when_every_knight_is_in_the_doorway() {
    let (mut run, room, next) = hall(2);
    in_the_doorway(&mut run, 1, NORTH);
    play(&mut run, 2);
    assert_eq!(run.at, room);
    in_the_doorway(&mut run, 2, NORTH);
    play(&mut run, 1);
    assert_eq!(run.at, next, "both in the north doorway: the party goes through");
    assert!(!run.waiting_for_fellowship());
}

#[test]
fn knights_in_different_doorways_move_nobody() {
    let (mut run, room, _) = hall(2);
    in_the_doorway(&mut run, 1, NORTH);
    in_the_doorway(&mut run, 2, 2);
    play(&mut run, 3);
    assert_eq!(run.at, room);
    assert_eq!(run.fellowship.waiting.len(), 2, "each waits in its own doorway");
}

#[test]
fn a_knight_who_steps_back_out_is_no_longer_counted() {
    let (mut run, room, _) = hall(2);
    in_the_doorway(&mut run, 1, NORTH);
    play(&mut run, 3);
    assert!(run.fellowship.is_waiting(1));
    middle(&mut run, 1);
    play(&mut run, 1);
    assert_eq!(run.at, room);
    assert!(run.fellowship.waiting.is_empty(), "nothing is pending once nobody is in a doorway");
    assert!(!run.waiting_for_fellowship());
    assert_eq!(run.fellowship_line(Some(1)), None);
    assert_eq!(run.fellowship_line(Some(2)), None);
    // Now the other knight is the first in; the first is the one awaited.
    in_the_doorway(&mut run, 2, NORTH);
    play(&mut run, 1);
    assert_eq!(run.at, room);
    assert_eq!(run.fellowship.waiting, vec![(2, 0)]);
}

/// Knight `id` made one who does not count, `how`.
fn uncount(run: &mut Run, id: u32, how: &str) {
    match how {
        "fallen" => run.players.get_mut(&id).unwrap().hp = 0,
        "away" => run.set_away(id, true),
        "absent" => run.set_absent(id, true),
        "stone" => run.players.get_mut(&id).unwrap().stone = true,
        _ => {
            run.idle.insert(id, AWAY_TICKS);
        }
    }
}

#[test]
fn a_fallen_an_away_or_an_absent_third_knight_never_holds_the_party() {
    for how in ["fallen", "away", "absent", "stone", "idle"] {
        let (mut run, room, next) = hall(3);
        uncount(&mut run, 3, how);
        in_the_doorway(&mut run, 1, NORTH);
        play(&mut run, 2);
        assert_eq!(run.at, room, "{how}: knight two plays and is not yet in the doorway");
        in_the_doorway(&mut run, 2, NORTH);
        play(&mut run, 1);
        assert_eq!(run.at, next, "{how}: the two who count are the party");
        // With both friends gone from the count, the one left goes at once.
        let (mut run, _, next) = hall(3);
        uncount(&mut run, 2, how);
        uncount(&mut run, 3, how);
        in_the_doorway(&mut run, 1, NORTH);
        play(&mut run, 1);
        assert_eq!(run.at, next, "{how}: alone");
    }
}

#[test]
fn the_knights_who_play_are_all_awaited_even_with_a_fallen_friend() {
    let (mut run, room, next) = hall(3);
    run.players.get_mut(&3).unwrap().hp = 0;
    in_the_doorway(&mut run, 1, NORTH);
    play(&mut run, 2);
    assert_eq!(run.at, room, "knight two plays and is not in the doorway");
    assert_eq!(run.fellowship.waiting, vec![(1, 0)]);
    in_the_doorway(&mut run, 2, NORTH);
    play(&mut run, 1);
    assert_eq!(run.at, next);
}

#[test]
fn an_away_knight_in_a_doorway_moves_nobody() {
    let (mut run, room, _) = hall(2);
    run.set_away(2, true);
    in_the_doorway(&mut run, 2, NORTH);
    play(&mut run, 3);
    // Knight one is the only one who counts, and is not in the doorway.
    assert_eq!(run.at, room);
    assert!(run.fellowship.waiting.is_empty());
}

#[test]
fn a_lone_knight_goes_at_once() {
    let mut run = Run::new(31, 1, None);
    run.descend_for_test();
    let (room, next) = (0..run.dungeon.rooms.len())
        .find_map(|i| run.dungeon.neighbour(i, NORTH).map(|n| (i, n)))
        .unwrap();
    run.enter_for_test(room);
    run.hallowed = None;
    run.calm_for_test();
    run.enemies.clear();
    run.dungeon.rooms[room].cleared = true;
    run.phase = Phase::Exploring;
    in_the_doorway(&mut run, 1, NORTH);
    play(&mut run, 1);
    assert_eq!(run.at, next);
    assert!(!run.waiting_for_fellowship());
}

#[test]
fn the_stairs_down_need_everyone_too() {
    let (mut run, room, _) = hall(2);
    let (cols, rows) = (run.room().cols, run.room().rows);
    // A stair in this room, as a slain guardian leaves one.
    run.dungeon.rooms[room].open_stairs();
    let stair = ((cols / 2) as f32 * TILE_UNITS, (rows / 2 + 2) as f32 * TILE_UNITS);
    let depth = run.dungeon.depth;
    run.stairs_held = false;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = stair;
    play(&mut run, 4);
    assert_eq!(run.dungeon.depth, depth, "one knight on the stairs is not the party");
    assert_eq!(run.fellowship.waiting, vec![(1, fellowship::STAIRS)]);
    assert_eq!(run.fellowship_line(Some(2)), Some(("FRIENDS WAIT AT THE STAIRS".into(), false)));
    let hero = run.players.get_mut(&2).unwrap();
    (hero.x, hero.y) = stair;
    play(&mut run, 1);
    assert_eq!(run.dungeon.depth, depth + 1, "both on the stairs: down they go");
    assert!(!run.waiting_for_fellowship());
}

#[test]
fn the_winding_stair_is_still_held_and_taken_by_the_whole_party() {
    let mut run = Run::at_home(7, 1, Some("Friend"), Home::default(), Spoils::default());
    run.begin_in(Pack::Crypt);
    run.stairs_held = false;
    let (x, y) = (24.0, 14.0);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (x, y);
    // One knight on it: the ring never fills.
    play(&mut run, home::DESCEND_HOLD * 2);
    assert!(run.at_home_now(), "the stair waits for the friend");
    assert_eq!(run.descending, 0);
    assert_eq!(run.fellowship.waiting, vec![(1, fellowship::STAIRS)]);
    // Both on it: a moment on it, then down.
    let hero = run.players.get_mut(&2).unwrap();
    (hero.x, hero.y) = (x, y);
    play(&mut run, home::DESCEND_HOLD / 2);
    assert!(run.at_home_now() && run.descending > 0, "the ring fills");
    play(&mut run, home::DESCEND_HOLD);
    assert!(!run.at_home_now(), "the whole party goes down the Winding Stair");
}

#[test]
fn the_wait_is_saved_and_old_saves_load_without_it() {
    let (mut run, _, _) = hall(2);
    in_the_doorway(&mut run, 1, NORTH);
    play(&mut run, 1);
    let copy: Run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
    assert_eq!(copy.fellowship, run.fellowship);
    let mut old = serde_json::to_value(&run).unwrap();
    old.as_object_mut().unwrap().remove("fellowship");
    let old: Run = serde_json::from_value(old).unwrap();
    assert!(old.fellowship.waiting.is_empty());
}

/// The bells rung this tick, by knight, taken out of the run.
fn bells(run: &mut Run) -> Vec<u32> {
    std::mem::take(&mut run.seat_sounds)
        .into_iter()
        .filter(|&(_, name)| name == fellowship::CALL_SOUND)
        .map(|(id, _)| id)
        .collect()
}

#[test]
fn three_of_four_in_the_doorway_call_the_fourth_alone() {
    let (mut run, room, _) = hall(4);
    for id in 1..=3 {
        in_the_doorway(&mut run, id, NORTH);
    }
    play(&mut run, 1);
    assert_eq!(run.at, room);
    assert_eq!(run.fellowship.called, vec![4]);
    // The fourth is shown where the three wait; the three see no arrow.
    let (x, y) = run.fellowship_call(Some(4)).expect("the fourth is called");
    assert!(y < 1.0 && (x - run.room().width() / 2.0).abs() < 0.5, "at the north door: {x}, {y}");
    for id in 1..=3 {
        assert_eq!(run.fellowship_call(Some(id)), None, "knight {id} is there");
    }
    assert_eq!(run.fellowship_call(None), None);
    // The bell is the fourth's alone, as the wire carries it.
    run.seat_sounds.clear();
    play(&mut run, fellowship::CALL_EVERY as u32);
    let events = crate::wire::take_sounds(&mut run);
    let rung: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, crate::wire::Event::Sound { name, .. } if name == fellowship::CALL_SOUND))
        .collect();
    assert_eq!(rung.len(), 1);
    assert_eq!(rung[0].hear(), crate::wire::Hear::Knight(4));
    assert_eq!(rung[0].line(Some(4), Some(&run)).as_deref(), Some("sfx circle_toll"));
    assert_eq!(rung[0].line(Some(1), Some(&run)), None);
}

#[test]
fn the_bell_rings_at_once_then_every_few_seconds_until_the_knight_joins() {
    let (mut run, _, next) = hall(4);
    for id in 1..=3 {
        in_the_doorway(&mut run, id, NORTH);
    }
    let mut rang = Vec::new();
    for _ in 0..fellowship::CALL_EVERY * 2 + 1 {
        play(&mut run, 1);
        if !bells(&mut run).is_empty() {
            rang.push(run.tick);
        }
    }
    let every = fellowship::CALL_EVERY;
    assert_eq!(rang.len(), 3, "at once, then once each interval: {rang:?}");
    assert_eq!(rang[1] - rang[0], every);
    assert_eq!(rang[2] - rang[1], every);
    // One who walks off is called again at once when the wait comes back.
    middle(&mut run, 1);
    middle(&mut run, 2);
    play(&mut run, 1);
    assert!(run.fellowship.called.is_empty() && run.fellowship.rung.is_empty());
    in_the_doorway(&mut run, 1, NORTH);
    in_the_doorway(&mut run, 2, NORTH);
    bells(&mut run);
    play(&mut run, 1);
    assert_eq!(bells(&mut run), vec![4]);
    // The fourth joins: the party goes, and no bell rings after.
    in_the_doorway(&mut run, 4, NORTH);
    play(&mut run, 1);
    assert_eq!(run.at, next);
    play(&mut run, fellowship::CALL_EVERY as u32 + 1);
    assert!(bells(&mut run).is_empty());
    assert!(run.fellowship.called.is_empty());
}

#[test]
fn nobody_is_called_while_nobody_waits_or_the_waiting_are_not_most() {
    let (mut run, _, _) = hall(4);
    play(&mut run, fellowship::CALL_EVERY as u32 + 1);
    assert!(run.fellowship.called.is_empty());
    assert!(bells(&mut run).is_empty());
    for id in 1..=4 {
        assert_eq!(run.fellowship_call(Some(id)), None);
    }
    // Two of four waiting are half the party, not most of it.
    in_the_doorway(&mut run, 1, NORTH);
    in_the_doorway(&mut run, 2, NORTH);
    play(&mut run, fellowship::CALL_EVERY as u32 + 1);
    assert!(run.waiting_for_fellowship());
    assert!(run.fellowship.called.is_empty());
    assert!(bells(&mut run).is_empty());
    assert_eq!(run.fellowship_call(Some(3)), None);
    // A friend who does not count leaves two of three: most, and the third is called.
    run.set_away(4, true);
    play(&mut run, 1);
    assert_eq!(run.fellowship.called, vec![3]);
    assert_eq!(bells(&mut run), vec![3]);
}
