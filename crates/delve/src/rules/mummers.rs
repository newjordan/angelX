//! The north-mine circuit: a stage, royal laundry and caravan camp.
//! Exploration plus a half-pipe medal unlocks the wagon, not a fetch quest.
use super::*;
use super::countryside::Country;
use crate::realm::{Spoil, Spoils};

pub const PLACES: [RoomKind; 3] = [RoomKind::Mummers, RoomKind::Laundry, RoomKind::Caravan];
pub const VISIT_MARKS: [&str; 3] = [
    "country:mummers:0", "country:mummers:1", "country:mummers:2",
];
pub const PLOT: usize = 11;
pub const WAGON: (i32, i32, i32, i32) = (19, 10, 2, 1);
pub const FAIR_WAGON: (i32, i32, i32, i32) = (21, 10, 1, 1);

fn fill(room: &mut Room, (x, y, w, h): (i32, i32, i32, i32), tile: Tile) {
    for row in y..y+h { for col in x..x+w { room.set(col as usize, row as usize, tile); } }
}
pub fn shape(room: &mut Room) {
    match room.kind {
        RoomKind::Mummers => {
            for rect in [(3, 1, 6, 2), (16, 1, 5, 2), (4, 11, 4, 2)] { fill(room, rect, Tile::Block); }
        }
        RoomKind::Laundry => {
            // Four wash vats leave a cross-shaped road and two inspection aisles.
            for rect in [(4, 2, 4, 3), (16, 2, 4, 3), (4, 9, 4, 3), (16, 9, 4, 3)] {
                fill(room, rect, Tile::Block);
            }
        }
        RoomKind::Caravan => {
            for rect in [(3, 2, 5, 3), (3, 10, 5, 2)] { fill(room, rect, Tile::Block); }
            fill(room, WAGON, Tile::Stairs);
        }
        RoomKind::Fairground => fill(room, FAIR_WAGON, Tile::Stairs),
        _ => {}
    }
}
impl Run {
    pub(super) fn tick_mummers(&mut self) {
        if let Some(i) = PLACES.iter().position(|&k| k == self.room().kind) {
            if self.players.values().any(|h| h.hp > 0 && !h.stone && h.privy == 0)
                && self.home.countryside.mummers_visited & (1 << i) == 0 {
                self.marks.insert(VISIT_MARKS[i].into(), 1);
            }
        }
    }
}
pub fn settle(country: &mut Country, treasury: &mut Spoils, marks: &BTreeMap<String, u32>, news: &mut Vec<String>) {
    for (i, mark) in VISIT_MARKS.iter().enumerate() {
        if marks.contains_key(*mark) { country.mummers_visited |= 1 << i; }
    }
    // Records merge before settlement; the medal has one persistent paid flag.
    if country.games.halfpipe.wins > 0 && !country.halfpipe_paid {
        country.halfpipe_paid = true;
        treasury.add(Spoil::Gold, 80);
        treasury.add(Spoil::Wax, 2);
        news.push("Jester's Half-Pipe medal: 80 gold, 2 wax.".into());
    }
}
