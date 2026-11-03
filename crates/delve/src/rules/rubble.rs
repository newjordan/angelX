//! Stone a guardian breaks: the room a fight ends in is not the one it
//! began in.
//!
//! A heavy enough blow shatters the pillars and boulders (`Tile::Block`)
//! round where it lands into bare floor, nearest first and no more than
//! `SHATTER_MOST` at once. A room's edge, its walls, doors and stairs never
//! break. Guests are sent the room anew (`Floor::world_revision`).
use super::*;

/// The most blocks one blow breaks.
pub const SHATTER_MOST: usize = 6;
/// How far past a blow's own reach the stone round it gives way, in units.
pub const SHATTER_PAST: f32 = 2.0;

impl Run {
    /// Break the blocks whose middles stand within `reach` of `at` in this
    /// room; how many broke.
    pub(super) fn shatter(&mut self, at: (f32, f32), reach: f32) -> usize {
        let Some(room) = self.dungeon.rooms.get_mut(self.at) else {
            return 0;
        };
        let mut struck: Vec<(f32, usize, usize)> = Vec::new();
        for row in 1..room.rows.saturating_sub(1) {
            for col in 1..room.cols.saturating_sub(1) {
                if room.tile(col as i32, row as i32) != Tile::Block {
                    continue;
                }
                let middle = ((col as f32 + 0.5) * TILE_UNITS, (row as f32 + 0.5) * TILE_UNITS);
                let d = (middle.0 - at.0).hypot(middle.1 - at.1);
                if d <= reach {
                    struck.push((d, col, row));
                }
            }
        }
        struck.sort_by(|a, b| a.0.total_cmp(&b.0).then((a.1, a.2).cmp(&(b.1, b.2))));
        struck.truncate(SHATTER_MOST);
        for &(_, col, row) in &struck {
            room.set(col, row, Tile::Floor);
        }
        if struck.is_empty() {
            return 0;
        }
        self.dungeon.world_revision = self.dungeon.world_revision.wrapping_add(1);
        for &(_, col, row) in &struck {
            let middle = ((col as f32 + 0.5) * TILE_UNITS, (row as f32 + 0.5) * TILE_UNITS);
            let way = unit(middle.0 - at.0, middle.1 - at.1);
            self.feel.puff(middle.0, middle.1, way, true);
            self.feel.puff(middle.0, middle.1, (-way.1, way.0), true);
        }
        self.sounds.push("crack_open");
        self.sounds.push("rock_land");
        struck.len()
    }
}

#[cfg(test)]
#[path = "../../tests/rules/rubble__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/rules/rubble__fights.rs"]
mod fights;
