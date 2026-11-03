//! The muster tent at the mine-head (the user, 2026-10-30: "put all the new
//! heroes in a bar in a mustering tent outside of the mines area").
//!
//! The recruits who joined the company (`recruit_ults::recruit`) no longer
//! crowd the Sozzled Griffin: they drink in a striped canvas tent pitched on
//! the mine-head, east of the shaft before the spoil heaps, its flap a door.
//! Inside is a bar of trestles and casks under the ridge pole and three rows
//! of long tables, the recruits on the benches behind them. A knight walks up to one from
//! the aisle on the table's north side and presses E, as in the tavern, and
//! the tavern's rules (`alehouse`) pick them: the tent is a room of the
//! house, with only its patrons to reach.
use super::world::{Dest, Entrance};
use super::*;

/// The tent's cell on the home floor, east of the bazaar. No doorway joins
/// it on the grid; its flap and the mine-head's are plates.
pub const CELL: (RoomKind, (i32, i32)) = (RoomKind::MusterTent, (19, -12));

/// The tent inside, in tiles: canvas round the edge.
pub const HALL: (usize, usize) = (30, 20);
/// The bar of trestles under the ridge, the casks beside it, and the
/// keeper behind it (feet, tiles).
pub const BAR: (i32, i32, i32, i32) = (9, 2, 12, 2);
pub const CASKS: (i32, i32, i32, i32) = (22, 1, 3, 2);
pub const KEEPER_AT: (f32, f32) = (15.0, 1.9);
/// The tent's two poles, down its middle aisle.
pub const POLES: [(i32, i32); 2] = [(14, 8), (14, 13)];
/// The long tables, two a row, three rows; the recruits sit on the bench
/// behind each table's north edge, seven a table.
pub const TABLES: [(i32, i32, i32, i32); 6] =
    [(2, 6, 11, 2), (17, 6, 11, 2), (2, 11, 11, 2), (17, 11, 11, 2), (2, 15, 11, 2), (17, 15, 11, 2)];
pub const SEATS_A_TABLE: usize = 7;
/// The flap in the south wall, and where one stands coming in.
pub const FLAP: (i32, i32, i32, i32) = (14, 19, 2, 1);
pub const ARRIVAL: (f32, f32) = (15.0, 17.6);

/// The tent on the mine-head (tiles): its canvas, between the spoil heaps
/// and the peddler's cart's stand, clear of the ropeway's landing and the
/// miners' paths; the flap in its south face; and where one stands coming
/// out before it.
pub const PITCH: (i32, i32, i32, i32) = (13, 4, 6, 2);
pub const OUTER_FLAP: (i32, i32, i32, i32) = (15, 6, 2, 1);
pub const OUTSIDE: (f32, f32) = (15.0, 7.4);

/// The seat of table `t`'s `k`th recruit (feet, tiles): on the table's
/// north edge, a stride and a half apart.
pub const fn seat(t: usize, k: usize) -> (f32, f32) {
    let (c, r, _, _) = TABLES[t];
    (c as f32 + 0.75 + 1.5 * k as f32, r as f32 + 0.25)
}

pub fn is_room(kind: RoomKind) -> bool {
    kind == RoomKind::MusterTent
}

pub const DOORS: [Entrance; 2] = [
    Entrance {
        id: "muster-out",
        room: RoomKind::MusterTent,
        at: FLAP,
        to: Dest::Room(RoomKind::MineHead, OUTSIDE),
        label: "OUT TO THE MINE-HEAD",
        needs: None,
    },
    Entrance {
        id: "muster-in",
        room: RoomKind::MineHead,
        at: OUTER_FLAP,
        to: Dest::Room(RoomKind::MusterTent, ARRIVAL),
        label: "THE MUSTER TENT",
        needs: None,
    },
];

fn fill(room: &mut Room, (c, r, w, h): (i32, i32, i32, i32), tile: Tile) {
    for y in r..r + h {
        for x in c..c + w {
            room.set(x as usize, y as usize, tile);
        }
    }
}

/// The tent laid out: canvas round the edge, a trodden floor, the bar, the
/// casks, the poles and the tables, and the flap's plate.
pub fn room(cell: (i32, i32)) -> Room {
    let mut r = Room::solid_rock(cell);
    r.kind = RoomKind::MusterTent;
    r.regrow(HALL.0, HALL.1);
    for rect in [BAR, CASKS].into_iter().chain(TABLES) {
        fill(&mut r, rect, Tile::Block);
    }
    for (c, row) in POLES {
        fill(&mut r, (c, row, 1, 1), Tile::Block);
    }
    fill(&mut r, FLAP, Tile::Stairs);
    r
}

/// The mine-head's ground the tent stands on: solid, but for its flap.
pub fn pitch(room: &mut Room) -> bool {
    let mut changed = false;
    let (c, r, w, h) = PITCH;
    for y in r..r + h {
        for x in c..c + w {
            if room.tile(x, y) != Tile::Block {
                room.set(x as usize, y as usize, Tile::Block);
                changed = true;
            }
        }
    }
    changed
}

#[cfg(test)]
#[path = "../../tests/rules/muster_tent__tests.rs"]
mod tests;
