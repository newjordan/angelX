//! A settlement's exhibits as the run knows them: only where each LOCAL
//! RESEARCH stand is, and whether a knight stands beside one. What is shown
//! there (its title, source and claims) stays with the host's settlement.

use super::{Floor, Run, TILE_UNITS, Tile};
use serde::{Deserialize, Serialize};

/// A saved loop site as the run admits it (`Run::enter_settlement`): the
/// host's settlement lends its id, the player it was saved for, its floor,
/// and where its exhibits stand.
pub struct Admission<'a> {
    pub id: &'a str,
    pub player: &'a str,
    pub floor: &'a Floor,
    pub exhibits: Vec<Marker>,
}

/// Only public geometry; never put private artifact metadata in a Run, guest
/// HUD, or pixels. Its generic LOCAL RESEARCH sign is shared with guests.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Marker {
    pub room: usize,
    pub col: usize,
    pub row: usize,
    pub stand: (usize, usize),
}

impl Marker {
    pub fn valid(&self, floor: &Floor) -> bool {
        floor.rooms.get(self.room).is_some_and(|r| {
            self.col < r.cols
                && self.row < r.rows
                && self.stand.0 < r.cols
                && self.stand.1 < r.rows
                && self.col.abs_diff(self.stand.0) + self.row.abs_diff(self.stand.1) == 1
                && r.tile(self.col as i32, self.row as i32) == Tile::Block
                && r.tile(self.stand.0 as i32, self.stand.1 as i32) == Tile::Floor
        })
    }

    /// A knight at `(x, y)` in room `at` stands on this exhibit's spot.
    pub fn beside(&self, at: usize, x: f32, y: f32) -> bool {
        self.room == at
            && (x - (self.stand.0 as f32 + 0.5) * TILE_UNITS).abs() <= TILE_UNITS * 0.7
            && (y - (self.stand.1 as f32 + 0.5) * TILE_UNITS).abs() <= TILE_UNITS * 0.7
    }
}

impl Run {
    /// Knight `id` stands beside one of this settlement's LOCAL RESEARCH
    /// exhibits, where E reads it rather than meeting the world.
    pub fn beside_exhibit(&self, id: u32) -> bool {
        self.at_home_now()
            && self.settlement_site.is_some()
            && self.players.get(&id).is_some_and(|h| {
                self.settlement_exhibits
                    .iter()
                    .any(|m| m.beside(self.at, h.x, h.y))
            })
    }
}
