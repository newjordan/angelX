//! Compatibility projection for the pre-washroom gameplay pins, not the world
//! the knights play in. The baseline/current trace audit found only this unused
//! room's dimensions/tiles changed; gameplay remained byte-identical. Do not omit
//! whole rooms, hero state, or arbitrary new fields to make a pin pass.
use crate::rules::{RoomKind, Run, Tile};

// The complete 36x20 hall at d59b1096a, independently of today's room builder.
// '#' wall, 'B' fixture, '.' floor, 'S' travel plate. This also pins every old
// fixture/plate while the annex has its own explicit geometry check below.
const LEGACY_HALL: [&str; 20] = [
    "####################################",
    "#BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB#",
    "#BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB#",
    "#...BBBBBB....................BBBB.#",
    "#............BBBBBBBBBBBBBBBB......#",
    "#BB..........BBBBBBBBBBBBBBBB......#",
    "#BB................................#",
    "#............................BBBBBB#",
    "#............................BBBBBB#",
    "#.......BBBB...BBBB...BBBB...BBBBBB#",
    "#.......BBBB...BBBB...BBBB.........#",
    "#..................................#",
    "#..................................#",
    "#.............................BBB..#",
    "#..........BBBB...BBBB........BBB.S#",
    "#..........BBBB...BBBB............S#",
    "#BBB..............................S#",
    "#BBB.BB....................BB......#",
    "#....BB..........SS........BB......#",
    "####################################",
];

pub(super) fn snapshot(run: &Run) -> Run {
    // Neither fixture plays inside the tavern. If it ever does, this projection
    // is no longer justified: add a real tavern gameplay pin instead.
    assert_ne!(
        run.room().kind,
        RoomKind::Alehouse,
        "pin entered the tavern"
    );
    assert!(
        run.exits
            .outside
            .values()
            .all(|e| run.dungeon.rooms[e.at].kind != RoomKind::Alehouse)
    );
    let mut snapshot = run.clone();
    for room in &mut snapshot.dungeon.rooms {
        if room.kind != RoomKind::Alehouse {
            continue;
        }
        assert_eq!(
            (room.cols, room.rows),
            (44, 20),
            "review the room fixture before changing this projection"
        );
        // Keep ALL tiles in the original footprint, except the two new door
        // cells. In particular, don't replace the room with a fresh builder:
        // changes to its old fixtures or its gameplay state must still fail.
        let old = room.clone();
        room.regrow(36, 20);
        room.laid = old.laid; // Dimensions/tiles only, never room metadata.
        for y in 0..20 {
            for x in 0..36 {
                let tile = if x == 35 && matches!(y, 5 | 6) {
                    assert_eq!(old.tile(x as i32, y as i32), Tile::Floor);
                    Tile::Wall
                } else {
                    old.tile(x as i32, y as i32)
                };
                room.set(x, y, tile);
            }
        }
    }
    snapshot
}

#[test]
fn bathroom_geometry_keeps_the_old_hall_and_explicitly_adds_only_the_annex() {
    let room = crate::rules::alehouse::room((0, 0));
    assert_eq!((room.cols, room.rows), (44, 20));
    for (y, row) in LEGACY_HALL.iter().enumerate() {
        assert_eq!(row.len(), 36);
        for (x, byte) in row.bytes().enumerate() {
            let tile = if x == 35 && matches!(y, 5 | 6) {
                Tile::Floor // The two new door cells are the ONLY old changes.
            } else {
                match byte {
                    b'#' => Tile::Wall,
                    b'B' => Tile::Block,
                    b'.' => Tile::Floor,
                    b'S' => Tile::Stairs,
                    _ => unreachable!(),
                }
            };
            assert_eq!(room.tile(x as i32, y as i32), tile, "legacy hall {x},{y}");
        }
        for x in 36..44 {
            let tile = if x == 43 || y == 0 || y >= 8 {
                Tile::Wall
            } else if (y == 2 && matches!(x, 38 | 39)) || (x == 41 && y == 5) {
                Tile::Block // The basin and bucket, not extra walking floor.
            } else {
                Tile::Floor
            };
            assert_eq!(room.tile(x, y as i32), tile, "annex {x},{y}");
        }
    }
}

#[test]
fn pin_projection_keeps_nondefault_mirror_state_and_legacy_room_edits() {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(None);
    run.players.get_mut(&1).unwrap().mirror_wish = true;
    let i = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Alehouse)
        .unwrap();
    run.dungeon.rooms[i].set(3, 3, Tile::Block);
    run.dungeon.rooms[i].cleared = false;
    let before = serde_json::to_string(&run).unwrap();
    let projected = snapshot(&run);
    assert_eq!(projected.dungeon.rooms[i].tile(3, 3), Tile::Block);
    assert!(!projected.dungeon.rooms[i].cleared);
    let json = serde_json::to_string(&projected).unwrap();
    assert!(json.contains("\"mirror_wish\":true"));
    assert_eq!(
        before,
        serde_json::to_string(&run).unwrap(),
        "never alter the played world"
    );
}
