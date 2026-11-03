//! The squires' rooms off the Training Yard (the user, 2026-10-30, choosing
//! "rooms off the Yard" for a Yard where seven squire games drew over one
//! another).
//!
//! The Yard keeps Sir Ector's squires, his lessons, the countryside squires,
//! the quintains and the stall. Each other squire game has a room of its
//! own, the Yard's size, so its plates stand where they always stood: the
//! Squires Guild, the Academy, the Barracks (the squires' company), the
//! Squire Hall and the Lanes. A door plate along the Yard's east wall leads
//! to each (its north wall is the countryside squires', its west Sir
//! Ector's notices), and a plate in each room's south-west corner leads back.
use super::world::{Dest, Entrance};
use super::*;

/// The rooms, the name on their door, and their cells on the home floor,
/// east of the muster tent. No doorway joins them on the grid.
pub const ROOMS: [(RoomKind, &str, (i32, i32)); 5] = [
    (RoomKind::SquireGuild, "SQUIRES GUILD", (20, -12)),
    (RoomKind::SquireAcademy, "ACADEMY", (21, -12)),
    (RoomKind::SquireBarracks, "BARRACKS", (22, -12)),
    (RoomKind::SquireHall, "SQUIRE HALL", (23, -12)),
    (RoomKind::SquireLanes, "LANES", (24, -12)),
];

/// The Yard's door plates, one a room in `ROOMS` order, just inside its
/// east wall, between Sir Ector's hiring plates (row 3) and the stall and
/// his roster (rows 9 and 10) (tiles).
pub const YARD_DOORS: [(i32, i32, i32, i32); 5] =
    [(22, 4, 1, 1), (22, 5, 1, 1), (22, 6, 1, 1), (22, 7, 1, 1), (22, 8, 1, 1)];
/// Each room's way back, in its south-west corner, clear of every squire
/// game's plates, and where one stands coming in: a party of four fans out
/// side by side on open floor.
pub const BACK: (i32, i32, i32, i32) = (1, 12, 2, 1);
pub const ARRIVAL: (f32, f32) = (4.0, 11.2);

pub fn is_room(kind: RoomKind) -> bool {
    ROOMS.iter().any(|(k, _, _)| *k == kind)
}

/// The name on room `kind`'s door.
pub fn name(kind: RoomKind) -> Option<&'static str> {
    ROOMS.iter().find(|(k, _, _)| *k == kind).map(|(_, n, _)| *n)
}

/// Where one comes out in the Yard from room `i`: before its door, a
/// party's width off the wall.
pub const fn yard_front(i: usize) -> (f32, f32) {
    let (c, r, _, h) = YARD_DOORS[i];
    (c as f32 - 3.0, r as f32 + h as f32 / 2.0)
}

const fn door(id: &'static str, room: RoomKind, at: (i32, i32, i32, i32), to: RoomKind, front: (f32, f32), label: &'static str) -> Entrance {
    Entrance { id, room, at, to: Dest::Room(to, front), label, needs: None }
}

pub const DOORS: [Entrance; 10] = [
    door("squires-guild-in", RoomKind::Yard, YARD_DOORS[0], RoomKind::SquireGuild, ARRIVAL, "THE SQUIRES GUILD"),
    door("squires-academy-in", RoomKind::Yard, YARD_DOORS[1], RoomKind::SquireAcademy, ARRIVAL, "THE ACADEMY"),
    door("squires-barracks-in", RoomKind::Yard, YARD_DOORS[2], RoomKind::SquireBarracks, ARRIVAL, "THE BARRACKS"),
    door("squires-hall-in", RoomKind::Yard, YARD_DOORS[3], RoomKind::SquireHall, ARRIVAL, "THE SQUIRE HALL"),
    door("squires-lanes-in", RoomKind::Yard, YARD_DOORS[4], RoomKind::SquireLanes, ARRIVAL, "THE LANES"),
    door("squires-guild-out", RoomKind::SquireGuild, BACK, RoomKind::Yard, yard_front(0), "BACK TO THE YARD"),
    door("squires-academy-out", RoomKind::SquireAcademy, BACK, RoomKind::Yard, yard_front(1), "BACK TO THE YARD"),
    door("squires-barracks-out", RoomKind::SquireBarracks, BACK, RoomKind::Yard, yard_front(2), "BACK TO THE YARD"),
    door("squires-hall-out", RoomKind::SquireHall, BACK, RoomKind::Yard, yard_front(3), "BACK TO THE YARD"),
    door("squires-lanes-out", RoomKind::SquireLanes, BACK, RoomKind::Yard, yard_front(4), "BACK TO THE YARD"),
];

/// A squire room laid out: stone round the edge, an open floor the Yard's
/// size, and the plate back.
pub fn room(kind: RoomKind, cell: (i32, i32)) -> Room {
    let mut room = Room::solid_rock(cell);
    room.kind = kind;
    for row in 1..ROWS - 1 {
        for col in 1..COLS - 1 {
            room.set(col, row, Tile::Floor);
        }
    }
    let (c, r, w, h) = BACK;
    for y in r..r + h {
        for x in c..c + w {
            room.set(x as usize, y as usize, Tile::Stairs);
        }
    }
    room
}

/// The Yard's door plates, laid on the Yard of an Undercroft that has one
/// (a saved settlement's Yard too).
pub fn doors_in_yard(floor: &mut Floor) -> bool {
    let mut changed = false;
    for yard in floor.rooms.iter_mut().filter(|r| r.kind == RoomKind::Yard) {
        for (c, r, w, h) in YARD_DOORS {
            for y in r..r + h {
                for x in c..c + w {
                    if yard.tile(x, y) == Tile::Floor {
                        yard.set(x as usize, y as usize, Tile::Stairs);
                        changed = true;
                    }
                }
            }
        }
    }
    changed
}

#[cfg(test)]
#[path = "../../tests/rules/squire_rooms__tests.rs"]
mod tests;
