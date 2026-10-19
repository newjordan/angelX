//! Six seats (`MAX_PLAYERS`): the host's and five friends', or, on a server
//! whose host plays no knight, seats 2 to 6 for five friends. Everything
//! sized to a head count clamps to its table; a save written when the party
//! was smaller still loads.

use super::feel::testing::*;
use super::*;

/// `quiet`, with knights 2 to `n` beside the first, none able to be hurt.
fn party(n: u32) -> Run {
    let mut run = quiet();
    for id in 2..=n {
        run.join_seat(id, &format!("K{id}"));
    }
    for hero in run.players.values_mut() {
        hero.invulnerable = 100_000;
    }
    run
}

/// A fresh delve whose host plays no knight, five friends in seats 2 to 6.
fn five_friends(seed: u64) -> Run {
    let mut run = Run::new(seed, 1, None);
    run.set_absent(1, true);
    for id in 2..=6 {
        run.join_seat(id, &format!("Friend{id}"));
    }
    run
}

#[test]
fn a_party_has_six_seats_and_no_seventh() {
    assert_eq!(MAX_PLAYERS, 6);
    assert_eq!(crate::wire::SEATS, 6);
    let mut run = Run::new(3, 1, None);
    for id in 2..=6 {
        run.join_seat(id, &format!("K{id}"));
    }
    assert_eq!(run.players.len(), 6);
    assert_eq!(run.party_len(), 6);
    run.join_seat(7, "Late");
    run.join_seat(0, "Zero");
    assert_eq!(run.players.len(), 6, "no seat past the sixth");
    assert!(run.valid_snapshot());
    // The seats on the wire: a seat a knight, six of them.
    for knight in 1..=6 {
        assert_eq!(crate::wire::seat_of(knight), Some(knight - 1));
        assert_eq!(crate::wire::knight_of(knight - 1), knight);
    }
    assert_eq!(crate::wire::seat_of(7), None);
    assert_eq!(crate::wire::seat_of(0), None);
}

#[test]
fn the_per_head_tables_hold_six_and_clamp_past_them() {
    assert_eq!(stakes::PARTY_HIT, [60, 100, 120, 135, 145, 155]);
    assert_eq!(stakes::EARLY_PARTY_HIT, [100, 115, 130, 145, 160, 175]);
    assert_eq!(stakes::PARTY_FURY, [0, 1, 2, 3, 3, 3]);
    // The tables cover every seat, and a count past them is the last entry.
    assert_eq!(stakes::PARTY_HIT.len(), MAX_PLAYERS as usize);
    assert_eq!(stakes::EARLY_PARTY_HIT.len(), MAX_PLAYERS as usize);
    assert_eq!(stakes::PARTY_FURY.len(), MAX_PLAYERS as usize);
    for knights in [0usize, 1, 2, 3, 4, 5, 6, 7, 12, 1000, usize::MAX] {
        let fury = stakes::party_fury(knights);
        assert_eq!(fury, stakes::PARTY_FURY[knights.clamp(1, 6) - 1], "{knights} knights");
        assert!(fury <= 3, "the extra fire per knight caps at 3");
    }
    assert_eq!(stakes::party_fury(6), 3);
    assert_eq!(stakes::party_fury(5), 3);
    // Each added head lands a monster's hit harder on the first floors, to
    // the sixth, and the sixth is no bullet hell.
    let mut last = 0;
    for n in 1..=6u32 {
        let mut run = party(n);
        assert_eq!(run.party_len() as u32, n);
        idle(&mut run, 1);
        let incoming = run.players[&1].incoming;
        assert_eq!(incoming, stakes::EARLY_PARTY_HIT[n as usize - 1], "{n} knights on floor 1");
        assert!(incoming > last, "{n} knights hit harder than {}", n - 1);
        last = incoming;
    }
    // From the third floor the table is PARTY_HIT's.
    let depth = 3;
    let mut last = 0;
    for n in 1..=6u32 {
        let mut run = party(n);
        run.dungeon.depth = depth;
        idle(&mut run, 1);
        let incoming = run.players[&1].incoming;
        assert!(incoming > last, "{n} knights on floor {depth}: {incoming} after {last}");
        last = incoming;
    }
    // Fury follows the head count into the monsters, capped at three.
    let mut six = party(6);
    let id = a_foe(&mut six, EnemyKind::Skeleton, 30.0, 10.0);
    assert_eq!(six.enemies.iter().find(|e| e.id == id).unwrap().fury, 3);
}

#[test]
fn a_save_written_with_four_approaches_loads() {
    // The merchant once kept a four-wide array of seat memories; the sixth
    // seat must not turn that save into "invalid length 4, expected 6".
    let run = Run::new(9, 1, Some("Friend"));
    let mut tree = serde_json::to_value(&run).unwrap();
    let four = serde_json::json!({
        "offer": null, "ticks": 0, "spent": false, "x": 0.0, "y": 0.0
    });
    let mut spent = four.clone();
    spent["spent"] = serde_json::json!(true);
    spent["offer"] = serde_json::json!(1);
    spent["ticks"] = serde_json::json!(7);
    tree["merchant"]["approaches"] = serde_json::json!([four, spent, four, four]);
    let loaded: Run = serde_json::from_value(tree.clone()).expect("a four-seat save loads");
    assert!(loaded.valid_snapshot());
    assert_eq!(loaded.merchant.approaches.len(), MAX_PLAYERS as usize);
    // What the save held is kept, seat for seat; the seats it never had are
    // fresh.
    assert!(loaded.merchant.approaches[1].spent);
    assert_eq!(loaded.merchant.approaches[1].offer, Some(1));
    assert_eq!(loaded.merchant.approaches[1].ticks, 7);
    for fresh in &loaded.merchant.approaches[4..] {
        assert_eq!(*fresh, merchant::Approach::default());
    }
    // Shorter still (a save from before the fourth seat) loads too, and one
    // with no memories at all.
    tree["merchant"]["approaches"] = serde_json::json!([four]);
    let loaded: Run = serde_json::from_value(tree.clone()).unwrap();
    assert_eq!(loaded.merchant.approaches.len(), MAX_PLAYERS as usize);
    tree["merchant"] = serde_json::json!({});
    let loaded: Run = serde_json::from_value(tree.clone()).unwrap();
    assert!(loaded.valid_snapshot());
    assert_eq!(loaded.merchant.approaches.len(), MAX_PLAYERS as usize);
    // A save that kept more seats than the party has is not a snapshot.
    let mut wide = serde_json::to_value(&run).unwrap();
    wide["merchant"]["approaches"] = serde_json::Value::Array(vec![four.clone(); MAX_PLAYERS as usize + 1]);
    let wide: Run = serde_json::from_value(wide).unwrap();
    assert!(!wide.valid_snapshot());
    // And a six-seat save written today reads back as it was.
    let mut six = Run::new(9, 1, None);
    six.merchant.approaches[5].spent = true;
    let back: Run = serde_json::from_value(serde_json::to_value(&six).unwrap()).unwrap();
    assert_eq!(back.merchant, six.merchant);
}

#[test]
fn a_whole_four_seat_run_saved_before_the_bump_still_plays() {
    // A run of four knights, the merchant's memory four wide, steps on after
    // it loads, and the fifth and sixth friends can sit down in it.
    let mut run = Run::new(21, 1, Some("Two"));
    run.join_seat(3, "Three");
    run.join_seat(4, "Four");
    let mut tree = serde_json::to_value(&run).unwrap();
    let approaches = tree["merchant"]["approaches"].as_array_mut().unwrap();
    approaches.truncate(4);
    let mut loaded: Run = serde_json::from_value(tree).unwrap();
    assert!(loaded.valid_snapshot());
    idle(&mut loaded, 30);
    loaded.join_seat(5, "Five");
    loaded.join_seat(6, "Six");
    assert_eq!(loaded.players.len(), 6);
    idle(&mut loaded, 30);
    assert!(loaded.valid_snapshot());
}

/// Every living knight aims north and fires.
fn volley(run: &Run) -> BTreeMap<u32, Input> {
    run.players
        .iter()
        .filter(|(_, h)| !h.absent)
        .map(|(&id, _)| (id, Input { aim_y: -1, move_x: if id % 2 == 0 { 1 } else { -1 }, fire: true, ..Input::default() }))
        .collect()
}

#[test]
fn five_friends_with_the_host_absent_join_fight_wipe_and_descend() {
    let mut run = five_friends(12);
    assert_eq!(run.players.len(), 6);
    assert_eq!(run.party_size(), 5, "the host's empty seat is no knight");
    assert_eq!(run.party_len(), 5);
    let fight = fight_room(&run);
    run.enter_for_test(fight);
    assert!(run.valid_snapshot());
    // A fight: all five fire into the room's foes for a good while.
    let foes = run.enemies.len();
    assert!(foes > 0, "the room's company is up");
    for tick in 0..600 {
        let keys = volley(&run);
        run.step(&keys);
        for (id, hero) in run.players.iter_mut().filter(|(id, _)| **id != 1) {
            hero.hp = hero.max_hp;
            let _ = id;
        }
        if tick % 50 == 0 {
            assert!(run.valid_snapshot(), "tick {tick}");
        }
    }
    assert!(run.valid_snapshot());
    assert!(
        run.enemies.len() < foes || run.enemies.iter().any(|e| e.hp < e.max_hp) || run.phase != Phase::Fighting,
        "five friends hurt the room"
    );
    // The party falls: every friend down, the host's stone seat not in the
    // way.
    for (id, hero) in run.players.iter_mut() {
        if *id != 1 {
            hero.invulnerable = 0;
            hero.hp = 0;
        }
    }
    idle(&mut run, 2);
    assert_eq!(run.phase, Phase::Wiped, "the party fell");
    assert!(run.over());
    // They ask to go back; the five friends' asking is the whole party's.
    for id in 2..=6 {
        run.ask_again(id);
    }
    assert!(run.again_due(), "every player here asked");
    // And the next delve lays itself out for the five of them.
    let mut next = Run::new(13, 2, None);
    next.set_absent(1, true);
    for id in 2..=6 {
        next.join_seat(id, &format!("Friend{id}"));
    }
    next.descend_for_test();
    assert_eq!(next.party_size(), 5);
    assert!(next.dungeon.depth >= 2);
    let (cols, rows) = layout::party_room(next.party_size());
    for room in next.dungeon.rooms.iter().filter(|r| matches!(r.kind, RoomKind::Fight | RoomKind::Stairs)) {
        assert!(room.cols >= cols && room.rows >= rows, "{}x{}", room.cols, room.rows);
    }
    idle(&mut next, 60);
    assert!(next.valid_snapshot());
    // The wiped run itself goes down the stairs for a test's sake, too.
    run.descend_for_test();
    assert_eq!(run.party_size(), 5);
    assert!(run.valid_snapshot());
}

#[test]
fn the_host_and_five_friends_are_six_knights_in_one_fight() {
    let mut run = Run::new(33, 1, Some("Two"));
    for id in 3..=6 {
        run.join_seat(id, &format!("K{id}"));
    }
    assert_eq!(run.party_len(), 6);
    let fight = fight_room(&run);
    run.enter_for_test(fight);
    for tick in 0..300 {
        let keys = volley(&run);
        run.step(&keys);
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        if tick % 60 == 0 {
            assert!(run.valid_snapshot(), "tick {tick}");
        }
    }
    // More monsters for more knights (the party's own two a head, the top of
    // the table), and the volleys no thicker than three extra shots.
    let alone = {
        let mut one = Run::new(33, 1, None);
        let room = fight_room(&one);
        one.enter_for_test(room);
        one.enemies.len()
    };
    let mut fresh = Run::new(33, 1, Some("Two"));
    for id in 3..=6 {
        fresh.join_seat(id, &format!("K{id}"));
    }
    let room = fight_room(&fresh);
    fresh.enter_for_test(room);
    assert!(fresh.enemies.len() > alone, "{} against {alone}", fresh.enemies.len());
    assert!(fresh.enemies.iter().all(|e| e.fury <= 3));
}
