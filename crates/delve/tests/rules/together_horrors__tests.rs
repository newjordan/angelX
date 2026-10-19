use super::*;

/// A fight room of the swamp's first floor, emptied, knight one in the
/// middle and `kind` `dx` units east of them, awake.
fn duel(kind: EnemyKind, dx: f32) -> Run {
    let mut run = Run::new(11, 1, None);
    run.begin_in(Pack::Swamp);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    run.projectiles.clear();
    let (cx, cy) = (run.room().width() / 2.0, run.room().height() / 2.0);
    // Dry ground under the duel, so only the horror's own reach counts.
    let (cols, rows) = (run.room().cols, run.room().rows);
    for row in 1..rows - 1 {
        for col in 1..cols - 1 {
            run.dungeon.rooms[room].set_for_test(col, row, Tile::Floor);
        }
    }
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (cx, cy);
    hero.invulnerable = 0;
    hero.max_hp = 2000;
    hero.hp = 2000;
    run.spawn_at_for_test(kind, cx + dx, cy);
    run.phase = Phase::Fighting;
    run
}

/// Knight one's health, after a first step has settled what they carry.
fn settled(run: &mut Run) -> u32 {
    run.step(&BTreeMap::new());
    run.players[&1].hp
}

fn still() -> BTreeMap<u32, Input> {
    BTreeMap::new()
}

#[test]
fn a_bog_hand_travels_under_the_mire_ripples_then_grabs_and_holds() {
    let mut run = duel(EnemyKind::BogHand, 4.0);
    assert!(run.enemies[0].sunk() && run.enemies[0].buried(), "it starts under the mire");
    assert!(!run.enemies[0].hurts_by_touch());
    let full = settled(&mut run);
    let mut rippled = false;
    for t in 0..200u32 {
        run.step(&still());
        let hand = &run.enemies[0];
        rippled |= hand.stage == 1;
        if hand.stage == 2 {
            break;
        }
        assert_eq!(run.players[&1].hp, full, "nothing lands before the grab (tick {t})");
    }
    assert!(rippled, "it rippled first");
    let hand = &run.enemies[0];
    assert_eq!(hand.stage, 2, "then it rose");
    assert!(!hand.sunk(), "risen, it can be struck");
    let hero = &run.players[&1];
    assert!(hero.hp < full, "the grab hurts");
    assert!(hero.mired > 0, "and holds");
    assert!(run.cues.iter().any(|c| c == "bog_grab"));
    // Held, a knight walks at the grip's pace.
    let held = run.ground_pace_of(1).unwrap();
    run.players.get_mut(&1).unwrap().mired = 0;
    let free = run.ground_pace_of(1).unwrap();
    assert!((held / free - GRIP_PACE).abs() < 1e-4);
}

#[test]
fn the_ripples_last_the_whole_tell_and_a_knight_who_steps_out_is_spared() {
    let mut run = duel(EnemyKind::BogHand, 4.0);
    let full = settled(&mut run);
    while run.enemies[0].stage != 1 {
        run.step(&still());
    }
    // The tell runs its full length.
    for _ in 1..HAND_TELL {
        run.step(&still());
        assert_eq!(run.enemies[0].stage, 1);
    }
    // And whoever leaves the ring is not taken.
    let (x, y) = (run.enemies[0].x, run.enemies[0].y);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (x - HAND_REACH - 2.0, y);
    run.step(&still());
    assert_eq!(run.enemies[0].stage, 2);
    assert_eq!(run.players[&1].hp, full);
    assert_eq!(run.players[&1].mired, 0);
}

#[test]
fn a_drowned_knight_lifts_its_blade_for_the_whole_windup_then_cleaves() {
    let mut run = duel(EnemyKind::Drowned, 3.0);
    let full = settled(&mut run);
    let mut lifted = 0;
    for _ in 0..400u32 {
        let before = run.players[&1].hp;
        run.step(&still());
        let knight = &run.enemies[0];
        if knight.stage == 1 {
            lifted += 1;
            assert_eq!(run.players[&1].hp, before, "the blade is up: nothing has landed");
        }
        if knight.stage == 2 {
            break;
        }
    }
    assert_eq!(lifted, DROWNED_WINDUP, "the tell shows for exactly the wind-up");
    assert!(run.players[&1].hp < full, "the cleave lands on whoever stayed in front");
    assert!(run.cues.iter().any(|c| c == "drowned_cleave"));
    // Then the blade is stuck: a moment to hit back.
    for _ in 0..DROWNED_STUCK - 1 {
        run.step(&still());
        assert_eq!(run.enemies[0].stage, 2);
    }
}

#[test]
fn a_drowned_knight_wades_the_bog_a_leech_does_not() {
    let mut room = Room::solid_rock((0, 0));
    for row in 1..ROWS - 1 {
        for col in 1..COLS - 1 {
            room.set_for_test(col, row, Tile::Bog);
        }
    }
    let grid = Grid { room: &room, barred: false };
    let (heroes, mut shots, mut deeds) = ([(30.0, 14.0)], Vec::new(), foes::Deeds::default());
    let mut run = duel(EnemyKind::Drowned, 3.0);
    let mut drowned = run.enemies.remove(0);
    (drowned.x, drowned.y, drowned.stage, drowned.timer) = (10.0, 14.0, 0, 0);
    act(&mut drowned, &heroes, &grid, &mut shots, &mut deeds);
    assert!((drowned.x - 10.0 - DROWNED_PACE * DT).abs() < 1e-4, "full pace through the mire");
}

#[test]
fn a_wisp_flares_draws_the_knights_in_then_bursts() {
    let mut run = duel(EnemyKind::Wisp, 9.0);
    while run.enemies[0].stage != 1 {
        run.step(&still());
    }
    let start = run.players[&1].x;
    let wisp_x = run.enemies[0].x;
    let mut lured = 0;
    for _ in 0..WISP_LURE + 2 {
        run.step(&still());
        if run.enemies[0].stage == 1 {
            lured += 1;
        } else {
            break;
        }
    }
    assert!(lured >= WISP_LURE - 1, "it lures for the whole tell: {lured}");
    let moved = run.players[&1].x - start;
    assert!(moved * (wisp_x - start).signum() > 0.5, "the knight is drawn toward it: {moved}");
    let pale = run
        .projectiles
        .iter()
        .filter(|p| p.hostile && p.kind == Shot::Wisp)
        .count();
    assert_eq!(pale as u32, WISP_RING, "then it bursts in a ring");
    assert!(run.cues.iter().any(|c| c == "wisp_burst"));
}

#[test]
fn the_horrors_have_their_pages_and_their_numbers() {
    for kind in [EnemyKind::BogHand, EnemyKind::Drowned, EnemyKind::Wisp] {
        assert!(kind.horror());
        assert!(bestiary::ENTRIES.iter().any(|e| e.kind == kind), "{kind:?} has its page");
        assert!(kind.hp() > 0 && kind.bounty() > 0);
    }
    assert!(EnemyKind::Wisp.flies());
    assert!(EnemyKind::Drowned.elite(), "one drowned knight a room");
}
