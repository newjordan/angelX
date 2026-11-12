// A big loop settlement fills the home floor with its own rooms. The world
// stands on top of whatever it holds: no room of the world is dropped.

/// A fresh run whose home floor holds `rooms` rooms before the world rises
/// on it: the Undercroft, then settlement rooms in cells clear of the world.
fn crowded_run(rooms: usize) -> Run {
    let mut run = Run::at_home(7, 1, None, Home::default(), Default::default());
    let mut floor = below_the_world(&run.dungeon);
    let mut n = 0;
    while floor.rooms.len() < rooms {
        floor.rooms.push(Room::solid_rock((100 + n, 100)));
        n += 1;
    }
    run.dungeon = floor;
    run.at = 0;
    run
}

#[test]
fn a_crowded_settlement_loses_none_of_the_worlds_rooms() {
    for rooms in [SETTLEMENT_ROOM_LIMIT - 20, SETTLEMENT_ROOM_LIMIT] {
        let mut run = crowded_run(rooms);
        let settled = run.dungeon.rooms.len();
        run.walk_alehouse();
        assert_eq!(run.room().kind, RoomKind::Alehouse, "{settled} settlement rooms: the party begins in the tavern");
        for &(kind, cell) in &ROOMS {
            assert!(
                run.dungeon.rooms.iter().any(|r| r.kind == kind && r.cell == cell),
                "{settled} settlement rooms: {kind:?} was dropped"
            );
        }
        for kind in [RoomKind::Kingdom, RoomKind::RealmHall] {
            assert!(room_of(&run.dungeon, kind).is_some(), "{settled} settlement rooms: {kind:?} was dropped");
        }
        assert!(run.dungeon.rooms.len() <= HOME_ROOM_LIMIT);
        assert!(run.valid_snapshot(), "{settled} settlement rooms: the raised floor must stand as a snapshot");
        // Raised once: a second pass adds nothing.
        let raised = run.dungeon.rooms.len();
        assert!(!raise_world(&mut run.dungeon));
        run.raise_kingdom();
        assert_eq!(run.dungeon.rooms.len(), raised);
    }
}

#[test]
fn the_home_floor_has_room_for_the_largest_settlement_and_the_whole_world() {
    assert!(SETTLEMENT_ROOM_LIMIT + ROOMS.len() + 2 <= HOME_ROOM_LIMIT);
}
