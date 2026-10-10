//! The world above the Delve: rooms of the realm itself that the party walks
//! with the Delve's own keys, and the entrances that lead down from them.
//!
//! The overworld map is walked by the harness (its knight goes where the
//! agent's work is) and has no doors. These rooms are the walkable side of
//! its places: the Delve's gate courtyard, the stables, the lists, and the
//! Mines' mouth. They sit on the home floor (depth 0) beside the Undercroft,
//! in their own corner of its grid, so co-op, the mirror and checkpoints
//! carry them like any home room.
//!
//! A room of the world may hold **entrances**: stairs or a door that, stood
//! on for a moment, take the party to another room (the gate's stair down to
//! the Undercroft, the Undercroft's stair back up) or straight into a delve
//! (the Mines' shaft). Every entrance is data in `ENTRANCES`, keyed by the
//! room it stands in.
//!
//! The Undercroft's own geometry (`layout::undercroft`) is left alone: a
//! saved loop settlement replays it byte for byte. The world, and the
//! Undercroft's stair up to it, are added to the run's own copy of the floor
//! (`raise_world`), after any settlement is admitted.

use super::*;

/// Where each room of the world sits on the home floor's grid: north of the
/// Undercroft, beyond anything a loop settlement can dig (it stays within
/// six cells of the cellar and never reaches row -5).
pub(crate) const ROOMS: [(RoomKind, (i32, i32)); 5] = [
    (RoomKind::Gate, (2, -8)),
    (RoomKind::Stables, (1, -8)),
    (RoomKind::Lists, (3, -8)),
    (RoomKind::MineHead, (2, -9)),
    // Inside the King's Hall: no doorway joins it to its neighbours on the
    // grid; its great door is an entrance.
    (RoomKind::KingsHall, (2, -11)),
];

/// Where an entrance takes the party.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Dest {
    /// Another room on the home floor, arriving at this point (tiles).
    Room(RoomKind, (f32, f32)),
    /// Straight down into the first floor of a delve.
    Delve(Pack),
}

/// A way down (or up): its tiles in the room it stands in, where it goes,
/// and what its board says.
pub(crate) struct Entrance {
    pub(crate) id: &'static str,
    pub(crate) room: RoomKind,
    /// The walk-on stair tiles: column, row, width, height.
    pub(crate) at: (i32, i32, i32, i32),
    pub(crate) to: Dest,
    pub(crate) label: &'static str,
    /// The barony's work that must stand before it opens, if any.
    pub(crate) needs: Option<&'static str>,
}

/// The Delve's gate in the courtyard: its arch, and the stair in its mouth.
pub(crate) const GATE_ARCH: (i32, i32, i32, i32) = (4, 1, 5, 4);
pub(crate) const GATE_STAIR: (i32, i32, i32, i32) = (5, 4, 2, 1);
/// Where the party comes up into the courtyard, and where a new delve
/// begins: in front of the gate, on the road (tiles).
pub(crate) const GATE_FRONT: (f32, f32) = (6.0, 6.6);
pub(crate) const COURTYARD_ARRIVAL: (f32, f32) = (12.0, 10.4);
/// The Undercroft's stair up to the gate, in its north wall east of the
/// Training Yard's door, and where one arrives below it.
pub(crate) const CELLAR_STAIR: (i32, i32, i32, i32) = (13, 1, 2, 1);
pub(crate) const CELLAR_FRONT: (f32, f32) = (14.0, 3.4);
/// The Mines' mouth at the mine-head: its timbered adit in the north rock,
/// the shaft's stair in its mouth.
pub(crate) const ADIT: (i32, i32, i32, i32) = (10, 1, 4, 3);
pub(crate) const SHAFT: (i32, i32, i32, i32) = (11, 3, 2, 1);

pub(crate) const ENTRANCES: [Entrance; 5] = [
    Entrance {
        id: "gate",
        room: RoomKind::Gate,
        at: GATE_STAIR,
        to: Dest::Room(RoomKind::Home, CELLAR_FRONT),
        label: "DOWN TO THE UNDERCROFT",
        needs: None,
    },
    Entrance {
        id: "cellar",
        room: RoomKind::Home,
        at: CELLAR_STAIR,
        to: Dest::Room(RoomKind::Gate, GATE_FRONT),
        label: "UP TO THE GATE",
        needs: None,
    },
    Entrance {
        id: "shaft",
        room: RoomKind::MineHead,
        at: SHAFT,
        to: Dest::Delve(Pack::Cavern),
        label: "DOWN THE SHAFT INTO THE MINES",
        needs: None,
    },
    Entrance {
        id: "hall",
        room: RoomKind::MineHead,
        at: super::barony::HALL_DOOR,
        to: Dest::Room(RoomKind::KingsHall, (12.0, 10.6)),
        label: "INTO THE KING'S HALL",
        needs: Some("hall"),
    },
    Entrance {
        id: "hall-door",
        room: RoomKind::KingsHall,
        at: super::barony::HALL_EXIT,
        to: Dest::Room(RoomKind::MineHead, (6.0, 8.4)),
        label: "OUT TO THE MINE-HEAD",
        needs: None,
    },
];

/// Does the entrance stand open, by what the realm has built?
pub(crate) fn open(e: &Entrance, home: &super::home::Home) -> bool {
    e.needs.is_none_or(|w| home.barony.built(w))
}

fn inside((col, row, w, h): (i32, i32, i32, i32), c: i32, r: i32) -> bool {
    c >= col && c < col + w && r >= row && r < row + h
}

/// The entrance whose tiles hold tile `(col, row)` of a room of `kind`.
pub(crate) fn entrance_at(kind: RoomKind, col: i32, row: i32) -> Option<&'static Entrance> {
    ENTRANCES
        .iter()
        .find(|e| e.room == kind && inside(e.at, col, row))
}

/// The entrance a standing knight is on, if any.
pub(crate) fn entrance_under(run: &Run) -> Option<(&'static Entrance, u32)> {
    let kind = run.room().kind;
    run.players.iter().find_map(|(&id, h)| {
        (h.hp > 0 && !h.stone)
            .then(|| {
                entrance_at(
                    kind,
                    (h.x / TILE_UNITS).floor() as i32,
                    (h.y / TILE_UNITS).floor() as i32,
                )
            })
            .flatten()
            .map(|e| (e, id))
    })
}

/// The middle of an entrance's tiles, in arena units.
pub(crate) fn entrance_centre(e: &Entrance) -> (f32, f32) {
    let (c, r, w, h) = e.at;
    (
        (c as f32 + w as f32 / 2.0) * TILE_UNITS,
        (r as f32 + h as f32 / 2.0) * TILE_UNITS,
    )
}

impl RoomKind {
    /// A room of the world above the Delve.
    pub(crate) fn in_world(self) -> bool {
        matches!(
            self,
            RoomKind::Gate
                | RoomKind::Stables
                | RoomKind::Lists
                | RoomKind::MineHead
                | RoomKind::KingsHall
        )
    }
}

fn walled(cell: (i32, i32), kind: RoomKind) -> Room {
    let mut room = Room::solid_rock(cell);
    room.kind = kind;
    for row in 1..ROWS - 1 {
        for col in 1..COLS - 1 {
            room.set(col, row, Tile::Floor);
        }
    }
    room
}

fn fill(room: &mut Room, (c, r, w, h): (i32, i32, i32, i32), tile: Tile) {
    for row in r..r + h {
        for col in c..c + w {
            room.set(col as usize, row as usize, tile);
        }
    }
}

/// The well in the courtyard, and the notice post by the road.
pub(crate) const WELL: (i32, i32, i32, i32) = (17, 3, 2, 2);
pub(crate) const POST: (i32, i32, i32, i32) = (15, 10, 1, 1);

/// The Delve's gate courtyard: the gate's arch over its stair in the north
/// west, a well, the road in from the realm to the south, and doors to the
/// mine-head (north), the stables (west) and the lists (east).
pub(crate) fn gate_room(cell: (i32, i32)) -> Room {
    let mut room = walled(cell, RoomKind::Gate);
    fill(&mut room, GATE_ARCH, Tile::Block);
    fill(&mut room, GATE_STAIR, Tile::Stairs);
    fill(&mut room, WELL, Tile::Block);
    fill(&mut room, POST, Tile::Block);
    room.open_door(0);
    room.open_door(1);
    room.open_door(3);
    room
}

/// The stable block along the north wall, its three stalls' doors open to
/// the yard, and the paddock fence to the south.
pub(crate) const STABLE_BLOCK: (i32, i32, i32, i32) = (2, 1, 20, 3);
pub(crate) const TROUGH: (i32, i32, i32, i32) = (3, 6, 2, 1);
pub(crate) const PADDOCK: (i32, i32, i32, i32) = (4, 10, 16, 1);

pub(crate) fn stables_room(cell: (i32, i32)) -> Room {
    let mut room = walled(cell, RoomKind::Stables);
    fill(&mut room, STABLE_BLOCK, Tile::Block);
    fill(&mut room, TROUGH, Tile::Block);
    fill(&mut room, PADDOCK, Tile::Block);
    room.open_door(1);
    room
}

/// The lists: the tilt down the middle, the stands along the north wall,
/// a pavilion at either end.
pub(crate) const TILT: (i32, i32, i32, i32) = (3, 7, 18, 1);
pub(crate) const STANDS: (i32, i32, i32, i32) = (3, 1, 18, 2);
pub(crate) const PAVILIONS: [(i32, i32, i32, i32); 2] = [(1, 10, 2, 2), (21, 1, 2, 2)];

pub(crate) fn lists_room(cell: (i32, i32)) -> Room {
    let mut room = walled(cell, RoomKind::Lists);
    fill(&mut room, TILT, Tile::Block);
    fill(&mut room, STANDS, Tile::Block);
    for p in PAVILIONS {
        fill(&mut room, p, Tile::Block);
    }
    room.open_door(3);
    room
}

/// The fallen gate-hall of Caer Dwfn at the mine-head, west of the adit;
/// the spoil heaps east of it.
pub(crate) const OLD_HALL: (i32, i32, i32, i32) = (2, 2, 8, 5);
pub(crate) const SPOIL: (i32, i32, i32, i32) = (17, 2, 4, 2);

pub(crate) fn mine_head_room(cell: (i32, i32)) -> Room {
    let mut room = walled(cell, RoomKind::MineHead);
    fill(&mut room, ADIT, Tile::Block);
    fill(&mut room, SHAFT, Tile::Stairs);
    fill(&mut room, OLD_HALL, Tile::Block);
    fill(&mut room, super::barony::HALL_DOOR, Tile::Stairs);
    fill(&mut room, SPOIL, Tile::Block);
    room.open_door(2);
    room
}

fn world_room(kind: RoomKind, cell: (i32, i32)) -> Room {
    match kind {
        RoomKind::Gate => gate_room(cell),
        RoomKind::Stables => stables_room(cell),
        RoomKind::Lists => lists_room(cell),
        RoomKind::KingsHall => super::barony::kings_hall_room(cell),
        _ => mine_head_room(cell),
    }
}

/// Add the world to a home floor that lacks it: its rooms, and the
/// Undercroft's stair up. True if anything was added. Only the home floor
/// (depth 0) with an Undercroft gets a world, and a cell already taken is
/// left as it is.
pub(crate) fn raise_world(floor: &mut Floor) -> bool {
    if floor.depth != 0 {
        return false;
    }
    let Some(cellar) = floor.rooms.iter().position(|r| r.kind == RoomKind::Home) else {
        return false;
    };
    let mut changed = false;
    let (c, r, w, h) = CELLAR_STAIR;
    for row in r..r + h {
        for col in c..c + w {
            if floor.rooms[cellar].tile(col, row) != Tile::Stairs {
                floor.rooms[cellar].set(col as usize, row as usize, Tile::Stairs);
                changed = true;
            }
        }
    }
    for (kind, cell) in ROOMS {
        if floor.rooms.len() >= 64 || floor.rooms.iter().any(|r| r.cell == cell) {
            continue;
        }
        floor.rooms.push(world_room(kind, cell));
        changed = true;
    }
    changed
}

/// The home floor as it lies below the world: without the world's rooms or
/// the Undercroft's stair up to them. A saved loop settlement's floor is
/// this, exactly.
#[cfg(test)]
pub(crate) fn below_the_world(floor: &Floor) -> Floor {
    let mut below = floor.clone();
    below.rooms.retain(|r| !r.kind.in_world());
    if let Some(cellar) = below.rooms.iter().position(|r| r.kind == RoomKind::Home) {
        let (c, r, w, h) = CELLAR_STAIR;
        for row in r..r + h {
            for col in c..c + w {
                below.rooms[cellar].set(col as usize, row as usize, Tile::Floor);
            }
        }
    }
    below
}

/// A room of the home floor by kind.
pub(crate) fn room_of(floor: &Floor, kind: RoomKind) -> Option<usize> {
    floor.rooms.iter().position(|r| r.kind == kind)
}

impl Run {
    /// One tick in a room of the world (the lists tick at the top of the
    /// step, before the saddled knight's keys are taken from them).
    pub(super) fn tick_world(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.room().kind == RoomKind::Stables {
            self.tick_stables(inputs);
        }
    }

    /// Walk the party into room `index` and stand them around `at` (tiles),
    /// side by side.
    pub(crate) fn arrive(&mut self, index: usize, at: (f32, f32)) {
        self.enter(index, None);
        let ids: Vec<u32> = self.players.keys().copied().collect();
        let count = ids.len() as f32;
        for (slot, id) in ids.into_iter().enumerate() {
            let offset = (slot as f32 - (count - 1.0) / 2.0) * 2.4;
            if let Some(hero) = self.players.get_mut(&id) {
                (hero.x, hero.y) = (at.0 * TILE_UNITS + offset, at.1 * TILE_UNITS);
            }
        }
        self.tallow_follows();
        self.hireling_follows();
        // A knight who arrives standing on stairs steps off before they work.
        self.stairs_held = true;
    }

    /// A new delve, begun in the world: in the gate courtyard, the gate's
    /// stair before the party, the Undercroft below it.
    pub(crate) fn come_to_the_gate(&mut self) {
        raise_world(&mut self.dungeon);
        if let Some(gate) = room_of(&self.dungeon, RoomKind::Gate) {
            self.arrive(gate, COURTYARD_ARRIVAL);
            // The welcome home waits for the Undercroft itself.
            self.cues.retain(|c| c != "home");
            self.cues.push("the_gate".into());
        }
    }

    /// Through an entrance: to another room, or down into a delve.
    pub(crate) fn take_entrance(&mut self, e: &Entrance) {
        match e.to {
            Dest::Room(kind, at) => {
                let Some(index) = room_of(&self.dungeon, kind) else {
                    return;
                };
                self.arrive(index, at);
                self.sounds.push("door_open");
                self.cues.push(if kind == RoomKind::Home {
                    "home".to_string()
                } else {
                    format!("entrance:{}", e.id)
                });
            }
            Dest::Delve(pack) => {
                // The shaft goes to the first floor of its delve, whatever
                // landing Wren has drawn on the Winding Stair.
                let landing = self.home.landing;
                self.home.landing = 1;
                self.dungeon.pack = pack;
                self.cues.push(format!("entrance:{}", e.id));
                self.descend();
                self.home.landing = landing;
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_world__tests.rs"]
mod tests;
