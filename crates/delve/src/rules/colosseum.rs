//! The Colosseum: the kingdom's hall for the knight-against-knight games.
//! Its door on the kingdom's map opens on an entrance hall with two arched
//! gates; through each is that game's own arena, and the arena is the whole
//! room. Every knight inside plays the heat together, as the games always
//! have (`pastimes`).
//!
//! The rooms and their doors are the world's own: `RoomKind` interiors on
//! the home floor, in their own cells, joined by `world::Entrance` plates the
//! way the College's are. The entrance hall's great door goes back out to
//! the kingdom (`Run::returns_to_kingdom`); each arena's gate goes back to
//! the hall, in front of the gate it was entered by.

use super::pastimes::Game;
use super::world::{Dest, Entrance};
use super::*;

pub const NAME: &str = "Colosseum";

/// The entrance hall and its two arenas, each in its own cell north east of
/// the College. No doorway joins them on the grid; the plates do.
pub const CELLS: [(RoomKind, (i32, i32)); 3] = [
    (RoomKind::ColosseumHall, (12, -12)),
    (RoomKind::KegYard, (13, -12)),
    (RoomKind::GanderHall, (14, -12)),
];
pub const ROOMS: [RoomKind; 3] = [RoomKind::ColosseumHall, RoomKind::KegYard, RoomKind::GanderHall];

/// The arena games, by gate: west, then east.
pub const GAMES: [Game; 2] = [Game::Powder, Game::Gander];

pub fn is_room(kind: RoomKind) -> bool {
    ROOMS.contains(&kind)
}

/// One of the arenas behind the gates.
pub fn is_arena(kind: RoomKind) -> bool {
    matches!(kind, RoomKind::KegYard | RoomKind::GanderHall)
}

/// The arena a game is played in, if it is one of the Colosseum's.
pub fn arena(game: Game) -> Option<RoomKind> {
    match game {
        Game::Powder => Some(RoomKind::KegYard),
        Game::Gander => Some(RoomKind::GanderHall),
        _ => None,
    }
}

/// The game played in an arena room.
pub fn game_in(kind: RoomKind) -> Option<Game> {
    GAMES.into_iter().find(|&g| arena(g) == Some(kind))
}

pub fn label(kind: RoomKind) -> &'static str {
    match kind {
        RoomKind::KegYard => "POWDER KEG YARD",
        RoomKind::GanderHall => "GANDER JOUST HALL",
        _ => "THE COLOSSEUM",
    }
}

/// Three words of how to play, for the gate's board.
pub fn words(game: Game) -> &'static str {
    match game {
        Game::Gander => "FLAP, CLIMB, STOMP",
        _ => "DROP, DODGE, OUTLAST",
    }
}

/// The entrance hall's north wall (an arcade), and the mouths of its two
/// gates cut through it: the plates stand in the mouths.
pub const WALL: (i32, i32, i32, i32) = (1, 1, 22, 2);
pub const GATES: [(i32, i32, i32, i32); 2] = [(4, 2, 2, 1), (18, 2, 2, 1)];
/// The great door back out to the kingdom, in the south wall.
pub const GREAT_DOOR: (i32, i32, i32, i32) = (11, 12, 2, 1);
/// Where the party stands coming in from the kingdom, and back out of each
/// arena (before the gate it went through).
pub const ARRIVAL: (f32, f32) = (12.0, 10.4);
pub const GATE_FRONT: [(f32, f32); 2] = [(5.0, 4.6), (19.0, 4.6)];
/// Each gate's board stands on two posts on the floor before it.
pub const BOARDS: [(i32, i32, i32, i32); 2] = [(2, 7, 7, 1), (15, 7, 7, 1)];

/// The Powder Keg yard: stands to the north, east and south of the
/// courtyard, the knights' bench to the west with the start mark on it, and
/// the gate back to the hall in the south wall below the bench.
pub const YARD_STANDS: [(i32, i32, i32, i32); 3] = [(6, 1, 17, 1), (18, 2, 5, 10), (6, 12, 17, 1)];
pub const YARD_GATE: (i32, i32, i32, i32) = (2, 12, 2, 1);
pub const YARD_ARRIVAL: (f32, f32) = (3.0, 10.4);

/// The Gander Joust hall, side-on: its gate back to the hall is low in its
/// west wall, just above the tar.
pub const HALL_GATE: (i32, i32, i32, i32) = (1, 9, 2, 2);
pub const HALL_ARRIVAL: (f32, f32) = (12.5, 7.7);

const fn door(
    id: &'static str,
    room: RoomKind,
    at: (i32, i32, i32, i32),
    to: RoomKind,
    front: (f32, f32),
    label: &'static str,
) -> Entrance {
    Entrance {
        id,
        room,
        at,
        to: Dest::Room(to, front),
        label,
        needs: None,
    }
}

pub const DOORS: [Entrance; 5] = [
    // Out through the great door: back to the kingdom (the visit's return,
    // `Run::returns_to_kingdom`); a party with no visit to return to (an
    // old save) comes out at the Delve's courtyard.
    door(
        "colosseum-out",
        RoomKind::ColosseumHall,
        GREAT_DOOR,
        RoomKind::Gate,
        world::COURTYARD_ARRIVAL,
        "THE KINGDOM",
    ),
    door(
        "colosseum-kegs",
        RoomKind::ColosseumHall,
        GATES[0],
        RoomKind::KegYard,
        YARD_ARRIVAL,
        "POWDER KEG",
    ),
    door(
        "colosseum-gander",
        RoomKind::ColosseumHall,
        GATES[1],
        RoomKind::GanderHall,
        HALL_ARRIVAL,
        "GANDER JOUST",
    ),
    door(
        "kegs-out",
        RoomKind::KegYard,
        YARD_GATE,
        RoomKind::ColosseumHall,
        GATE_FRONT[0],
        "COLOSSEUM HALL",
    ),
    door(
        "gander-out",
        RoomKind::GanderHall,
        HALL_GATE,
        RoomKind::ColosseumHall,
        GATE_FRONT[1],
        "COLOSSEUM HALL",
    ),
];

/// The solid parts of a room: walls, stands and board posts.
pub fn blocks(kind: RoomKind) -> &'static [(i32, i32, i32, i32)] {
    match kind {
        RoomKind::ColosseumHall => &[WALL, BOARDS[0], BOARDS[1]],
        RoomKind::KegYard => &YARD_STANDS,
        _ => &[],
    }
}

pub fn room(kind: RoomKind, cell: (i32, i32)) -> Room {
    let mut r = Room::solid_rock(cell);
    r.kind = kind;
    for y in 1..ROWS - 1 {
        for x in 1..COLS - 1 {
            r.set(x, y, Tile::Floor);
        }
    }
    for &(x, y, w, h) in blocks(kind) {
        for yy in y..y + h {
            for xx in x..x + w {
                r.set(xx as usize, yy as usize, Tile::Block);
            }
        }
    }
    for e in DOORS.iter().filter(|e| e.room == kind) {
        let (x, y, w, h) = e.at;
        for yy in y..y + h {
            for xx in x..x + w {
                r.set(xx as usize, yy as usize, Tile::Stairs);
            }
        }
    }
    r
}

impl Run {
    /// A heat is on in one of the arenas: its gate stays shut until the heat
    /// is done (the riders and the yard's knights go where the game puts
    /// them, and must not carry the party out mid-heat).
    pub fn arena_heat_on(&self) -> bool {
        is_arena(self.room().kind) && self.pastimes.heat.is_some()
    }

    /// Out of an arena (the leave key, or the last knight out): back to the
    /// entrance hall, before the gate it was entered by.
    pub(super) fn arena_return(&self) -> Option<exits::Entry> {
        let kind = self.room().kind;
        let gate = GAMES.iter().position(|&g| arena(g) == Some(kind))?;
        let at = world::room_of(&self.dungeon, RoomKind::ColosseumHall)?;
        let (x, y) = GATE_FRONT[gate];
        let count = self.players.len() as f32;
        Some(exits::Entry {
            at,
            party: self
                .players
                .keys()
                .enumerate()
                .map(|(slot, &id)| {
                    let offset = (slot as f32 - (count - 1.0) / 2.0) * 2.4;
                    (id, (x * TILE_UNITS + offset, y * TILE_UNITS))
                })
                .collect(),
        })
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_colosseum__tests.rs"]
mod tests;
