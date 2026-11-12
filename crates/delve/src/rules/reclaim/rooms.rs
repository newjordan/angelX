//! The lost halls' rooms: their layouts, the spots their set-pieces use
//! (tiles), and where a floor hangs one off a room.
//!
//! Each hall keeps the middle crosses of its room clear (the columns and
//! rows its doorways open into), so it plays the same through whichever
//! wall its door is in.

use super::*;

/// Where each hall's keeper stands at the start (tile points), by place in
/// `HALLS`.
pub const KEEPER_AT: [(f32, f32); 3] = [(12.0, 3.6), (11.5, 7.0), (12.0, 4.6)];

/// A hall's pillars (tiles), never in a doorway's way.
pub fn pillars(i: usize) -> &'static [(i32, i32)] {
    if i == 1 {
        &[(6, 9), (17, 9)]
    } else {
        &[(4, 3), (4, 10), (19, 3), (19, 10)]
    }
}

/// The Gallery: Nubbin's chair of barrels in the north, the hoard in heaps
/// in either corner.
pub const CHAIR: (i32, i32, i32, i32) = (10, 1, 4, 2);
pub const HEAPS: [(i32, i32, i32, i32); 2] = [(1, 1, 3, 2), (20, 1, 3, 2)];

/// The Hall of Lamps: four lamp stands and the stone behind each, along the
/// north wall.
pub const LAMPS: [(f32, f32); 4] = [(4.5, 4.0), (9.5, 4.0), (14.5, 4.0), (19.5, 4.0)];
pub const SLABS: [(i32, i32, i32, i32); 4] = [(3, 1, 3, 2), (8, 1, 3, 2), (13, 1, 3, 2), (18, 1, 3, 2)];
/// How near a knight stands to kindle a lamp (tiles).
pub const LAMP_REACH: f32 = 1.8;
/// The sons, eldest first, and their winters.
pub const SONS: [(&str, u32); 4] = [("RHODRI", 61), ("CADFAN", 54), ("IESTYN", 47), ("EMRYS", 40)];

/// The Anvil Gallery: three heaps of fallen roof (floor a knight digs on),
/// the smith's anvil and forge-mouth in the north wall.
pub const DIGS: [(i32, i32, i32, i32); 3] = [(3, 4, 3, 2), (18, 4, 3, 2), (10, 8, 4, 2)];
pub const ANVILS: [(i32, i32, i32, i32); 2] = [(1, 1, 3, 1), (20, 1, 3, 1)];
pub const FORGE_MOUTH: (i32, i32, i32, i32) = (9, 1, 6, 1);

/// The plate in each hall once it is the King's, where its rent is read.
pub const RENT_PLATE: (i32, i32, i32, i32) = (11, 9, 2, 1);

fn fill(room: &mut Room, (c, r, w, h): (i32, i32, i32, i32), tile: Tile) {
    for row in r..r + h {
        for col in c..c + w {
            room.set(col as usize, row as usize, tile);
        }
    }
}

/// A lost hall's room: pillars, its own furniture, one doorway.
pub fn lost_hall_room(i: usize, cell: (i32, i32), door: usize) -> Room {
    let mut room = Room::solid_rock(cell);
    room.kind = RoomKind::DwarfHall;
    fill(&mut room, (1, 1, COLS as i32 - 2, ROWS as i32 - 2), Tile::Floor);
    for &(c, r) in pillars(i) {
        fill(&mut room, (c, r, 1, 1), Tile::Block);
    }
    match i {
        0 => {
            fill(&mut room, CHAIR, Tile::Block);
            for heap in HEAPS {
                fill(&mut room, heap, Tile::Block);
            }
        }
        1 => {
            for slab in SLABS {
                fill(&mut room, slab, Tile::Block);
            }
        }
        _ => {
            for anvil in ANVILS {
                fill(&mut room, anvil, Tile::Block);
            }
            fill(&mut room, FORGE_MOUTH, Tile::Block);
        }
    }
    room.open_door(door);
    room
}

/// The sons' winters as the stones show them, left to right: the same four
/// ages every time, in an order the run's seed picks.
pub fn rite_ages(seed: u64) -> [usize; 4] {
    let mut order = [0usize, 1, 2, 3];
    let mut h = mix(seed ^ 0x6c61_6d70_7300_0001);
    for i in (1..4).rev() {
        h = mix(h);
        order.swap(i, (h % (i as u64 + 1)) as usize);
    }
    order
}

/// The stones' places in the order the lamps must be kindled: eldest son
/// first. `rite_ages` names the son each stone carries; this is its
/// inverse.
pub fn rite_order(seed: u64) -> [usize; 4] {
    let at = rite_ages(seed);
    let mut order = [0usize; 4];
    for (place, son) in at.into_iter().enumerate() {
        order[son] = place;
    }
    order
}

/// A hall the King has asked of the party, or has been given by it, hangs
/// off a floor of its delve: one more room, joined to a fight room or a
/// passage by a doorway, chosen without dice (the last such room first, the
/// first free wall: north, east, west) so every other room stays as the seed
/// laid it. Called before the floor's politics are drawn. True if a room was
/// added.
pub fn add_lost_hall(floor: &mut Floor, barony: &Barony) -> bool {
    let Some((i, hall)) = hall_of(floor.pack) else {
        return false;
    };
    let halls = &barony.halls;
    if barony.court < Court::Met
        || !(halls.asked == Some(i as u8) || halls.is_held(hall.id))
        || floor.depth == 0
        || floor.open.is_some()
        || floor.rooms.len() >= 62
        || floor.rooms.iter().any(|r| r.kind == RoomKind::DwarfHall)
    {
        return false;
    }
    let taken = |c: (i32, i32)| floor.rooms.iter().any(|r| r.cell == c);
    for want in [&[RoomKind::Fight, RoomKind::Hall][..], &[RoomKind::Treasure, RoomKind::Start, RoomKind::Sanctuary][..]] {
        for at in (0..floor.rooms.len()).rev() {
            let host = &floor.rooms[at];
            if !want.contains(&host.kind) || host.great() || host.cols != COLS || host.rows != ROWS {
                continue;
            }
            for dir in [0usize, 1, 3] {
                let (dx, dy) = DIRS[dir];
                let cell = (host.cell.0 + dx, host.cell.1 + dy);
                if taken(cell) || !(-16..=16).contains(&cell.0) || !(-16..=16).contains(&cell.1) {
                    continue;
                }
                let mut opened = host.clone();
                opened.open_door(dir);
                if !layout::connected(&opened) {
                    continue;
                }
                floor.rooms[at] = opened;
                floor.rooms.push(lost_hall_room(i, cell, (dir + 2) % 4));
                return true;
            }
        }
    }
    false
}
