use super::*;
use std::collections::BTreeMap;

/// Two knights in a fight room on the first floor, foes about them.
fn fight() -> Run {
    let mut run = Run::new(31, 1, Some("Friend"));
    run.descend_for_test();
    let room = (0..run.dungeon.rooms.len())
        .find(|&i| run.dungeon.rooms[i].kind == RoomKind::Fight)
        .expect("a fight room on the first floor");
    run.enter_for_test(room);
    run.hallowed = None;
    run.calm_for_test();
    run.enemies.clear();
    for kind in [EnemyKind::Skeleton, EnemyKind::Bat, EnemyKind::Imp, EnemyKind::Flesher, EnemyKind::Bat] {
        run.spawn(kind);
    }
    run.phase = Phase::Fighting;
    run
}

fn hp(run: &Run, id: u32) -> u32 {
    run.players[&id].hp
}

#[test]
fn a_knight_whose_player_is_away_is_stone_and_nothing_hurts_it() {
    let mut run = fight();
    run.set_away(2, true);
    let full = hp(&run, 2);
    for _ in 0..HZ * 20 {
        run.set_away(2, true);
        run.step(&BTreeMap::new());
    }
    let hero = &run.players[&2];
    assert!(hero.away && hero.stone);
    assert_eq!(hero.hp, full, "twenty seconds among foes, untouched");
    // Knight 1, here and idle, is fought: the foes are real.
    assert!(hp(&run, 1) < run.players[&1].max_hp, "the knight who is here was fought");
}

#[test]
fn coming_back_stands_the_knight_as_stone_until_it_moves_and_a_vigil_stays_stone() {
    let mut run = fight();
    run.set_away(2, true);
    run.step(&BTreeMap::new());
    run.set_away(2, false);
    let hero = &run.players[&2];
    assert!(!hero.away && hero.stone && hero.idle, "back, but stone till it moves");
    assert!(hero.invulnerable >= presence::WAKE_WARD);
    let step = Input { move_y: 1, ..Input::default() };
    run.step(&BTreeMap::from([(2, step)]));
    assert!(!run.players[&2].stone && !run.players[&2].idle, "its first move wakes it");
    // A knight keeping vigil when its player went is still keeping it.
    let hero = run.players.get_mut(&2).unwrap();
    hero.vigil = true;
    hero.stone = true;
    run.set_away(2, true);
    run.set_away(2, false);
    assert!(run.players[&2].stone, "the vigil holds");
    // Said every tick, away holds the knight to stone whatever lifted it.
    run.set_away(1, true);
    run.players.get_mut(&1).unwrap().stone = false;
    run.set_away(1, true);
    assert!(run.players[&1].stone);
}

#[test]
fn a_room_holding_for_its_party_keeps_its_foes_and_shots_still_and_hurts_no_one() {
    let mut run = fight();
    // Let the foes wake and shoot a while first.
    for _ in 0..HZ * 3 {
        run.step(&BTreeMap::new());
    }
    let full = (hp(&run, 1), hp(&run, 2));
    run.hold(Hold::Arriving);
    let foes: Vec<(f32, f32)> = run.enemies.iter().map(|e| (e.x, e.y)).collect();
    let shots: Vec<(f32, f32)> = run.projectiles.iter().filter(|p| p.hostile).map(|p| (p.x, p.y)).collect();
    for _ in 0..HZ * 3 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.enemies.iter().map(|e| (e.x, e.y)).collect::<Vec<_>>(), foes, "the foes wait");
    assert_eq!(
        run.projectiles.iter().filter(|p| p.hostile).map(|p| (p.x, p.y)).collect::<Vec<_>>(),
        shots,
        "their shots hang where they were"
    );
    assert_eq!((hp(&run, 1), hp(&run, 2)), full, "no knight is hurt while the room holds");
    // A knight here may still walk while the room waits.
    let x = run.players[&1].x;
    let right = Input { move_x: 1, ..Input::default() };
    for _ in 0..10 {
        run.step(&BTreeMap::from([(1, right)]));
    }
    assert!(run.players[&1].x > x, "the knight walks");
    // Let go, and the fight is on again.
    run.hold(Hold::Open);
    for _ in 0..HZ * 2 {
        run.step(&BTreeMap::new());
    }
    assert_ne!(run.enemies.iter().map(|e| (e.x, e.y)).collect::<Vec<_>>(), foes, "the foes move again");
}

#[test]
fn the_hold_and_the_away_mark_reach_a_client() {
    let mut run = fight();
    run.set_away(2, true);
    run.hold(Hold::Empty);
    let copy = run.wire_copy();
    assert!(copy.players[&2].away && copy.players[&2].stone);
    assert_eq!(copy.held, Hold::Empty);
}

/// A run for trying presence live, written as JSON: floor one, the party
/// in a cleared room beside a fight, knight 1 (the host's statue) on the
/// line of the east or west door, so a friend joining beside it walks
/// straight through. Prints the room, the fight and the key to hold.
/// `ANGEL_PRESENCE_SCENE=file cargo test -p angel-delve write_presence_scene -- --ignored`
#[test]
#[ignore]
fn write_presence_scene() {
    let Some(file) = std::env::var_os("ANGEL_PRESENCE_SCENE") else {
        return;
    };
    let mut run = Run::new(31, 1, None);
    run.descend_for_test();
    let rooms = &run.dungeon.rooms;
    let (room, dir, fight) = (0..rooms.len())
        .flat_map(|i| [1usize, 3].map(|d| (i, d)))
        .find_map(|(i, d)| {
            let j = run.dungeon.neighbour(i, d)?;
            (rooms[i].kind != RoomKind::Fight
                && rooms[j].kind == RoomKind::Fight
                && !rooms[j].great()
                && !rooms[j].cleared)
                .then_some((i, d, j))
        })
        .expect("a room beside a fight, east or west");
    run.enter_for_test(room);
    run.clear_for_test();
    let (cols, rows) = (run.room().cols as f32, run.room().rows as f32);
    let x = if dir == 1 { (cols - 7.0) * 2.0 } else { 7.0 * 2.0 };
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (x, (rows / 2.0).floor() * 2.0);
    assert!(run.valid_snapshot());
    std::fs::write(&file, serde_json::to_vec(&run).unwrap()).unwrap();
    println!(
        "room {room} ({:?}) -> fight {fight}, hold {}",
        run.room().kind,
        if dir == 1 { "d" } else { "a" }
    );
}

/// Four knights in the fight room above: the two the test made, and two
/// friends joined.
fn four() -> Run {
    let mut run = fight();
    run.join_seat(3, "Third");
    run.join_seat(4, "Fourth");
    run
}

fn foes_max(run: &Run) -> Vec<u32> {
    run.enemies.iter().map(|e| e.max_hp).collect()
}

#[test]
fn the_party_is_sized_for_the_knights_whose_players_are_here() {
    let mut run = four();
    assert_eq!(run.party_len(), 4);
    let at_four = foes_max(&run);
    // Two of them go away: the monsters are sized for the two who play.
    run.set_away(3, true);
    run.set_away(4, true);
    assert_eq!(run.party_len(), 2);
    run.step(&BTreeMap::new());
    let at_two = foes_max(&run);
    for (four, two) in at_four.iter().zip(&at_two) {
        assert_eq!(four / 2, *two, "health for two knights, not four");
    }
    // A foe that comes on now is sized for two as well.
    run.spawn(EnemyKind::Skeleton);
    let new = run.enemies.last().unwrap();
    assert_eq!(new.max_hp, new.base_hp() * 2);
    // Back: sized for four again, each foe keeping its share of the hurt.
    run.enemies[0].hp = run.enemies[0].max_hp / 2;
    run.set_away(3, false);
    run.set_away(4, false);
    run.step(&BTreeMap::new());
    assert_eq!(run.enemies[0].max_hp, at_four[0]);
    assert!(run.enemies[0].hp.abs_diff(at_four[0] / 2) <= 1, "half its health still");
}

#[test]
fn a_run_with_everyone_here_writes_no_party_size() {
    let mut run = four();
    run.step(&BTreeMap::new());
    assert!(!serde_json::to_string(&run).unwrap().contains("scaled_for"), "today's saves, as they were");
    run.set_away(4, true);
    run.step(&BTreeMap::new());
    let saved = serde_json::to_string(&run).unwrap();
    assert!(saved.contains("\"scaled_for\":3"));
    // Read back, it still knows what its foes stand scaled for.
    let mut back: Run = serde_json::from_str(&saved).unwrap();
    let before = foes_max(&back);
    back.step(&BTreeMap::new());
    assert_eq!(foes_max(&back), before, "no second rescale after a load");
}

#[test]
fn a_host_who_plays_no_knight_keeps_an_absent_place_that_counts_for_nothing() {
    let mut run = fight();
    run.set_absent(1, true);
    let host = &run.players[&1];
    assert!(host.absent && host.away && host.stone);
    assert_eq!(run.party_len(), 1);
    run.step(&BTreeMap::new());
    assert!(run.enemies.iter().all(|e| e.max_hp == e.base_hp()), "foes for the one knight who plays");
    // The friend is the party's lead where knight 1 was: it is no part of
    // the tree a client reads.
    let tree = crate::wire::publish(&run).unwrap();
    let back = crate::wire::read(&tree).unwrap();
    assert!(!back.players.contains_key(&1) && back.players.contains_key(&2));
    assert!(crate::wire::drawable(&back));
    // A host who plays again is in the party again.
    run.set_absent(1, false);
    run.set_away(1, false);
    assert_eq!(run.party_len(), 2);
    let tree = crate::wire::publish(&run).unwrap();
    assert!(crate::wire::read(&tree).unwrap().players.contains_key(&1));
}

/// A run for trying the party live (restored by a host that plays no
/// knight): four knights in a room beside a fight, as a host restarted
/// mid-delve would find them.
/// `ANGEL_PARTY_SCENE=file cargo test -p angel-delve write_party_scene -- --ignored`
#[test]
#[ignore]
fn write_party_scene() {
    let Some(file) = std::env::var_os("ANGEL_PARTY_SCENE") else {
        return;
    };
    let mut run = Run::new(31, 1, None);
    run.descend_for_test();
    let rooms = &run.dungeon.rooms;
    let (room, dir, fight) = (0..rooms.len())
        .flat_map(|i| [1usize, 3].map(|d| (i, d)))
        .find_map(|(i, d)| {
            let j = run.dungeon.neighbour(i, d)?;
            (rooms[i].kind != RoomKind::Fight && rooms[j].kind == RoomKind::Fight && !rooms[j].great() && !rooms[j].cleared)
                .then_some((i, d, j))
        })
        .expect("a room beside a fight, east or west");
    run.enter_for_test(room);
    run.clear_for_test();
    for (id, name) in [(2, "Old Two"), (3, "Old Three"), (4, "Old Four")] {
        run.join_seat(id, name);
    }
    let (cols, rows) = (run.room().cols as f32, run.room().rows as f32);
    let x = if dir == 1 { (cols - 7.0) * 2.0 } else { 7.0 * 2.0 };
    // In a row along the door's: whoever joins stands beside knight 1.
    let step = if dir == 1 { -2.6 } else { 2.6 };
    for (i, hero) in run.players.values_mut().enumerate() {
        (hero.x, hero.y) = (x + step * i as f32, (rows / 2.0).floor() * 2.0);
    }
    assert!(run.valid_snapshot());
    std::fs::write(&file, serde_json::to_vec(&run).unwrap()).unwrap();
    println!("room {room} ({:?}) -> fight {fight}, hold {}", run.room().kind, if dir == 1 { "d" } else { "a" });
}
