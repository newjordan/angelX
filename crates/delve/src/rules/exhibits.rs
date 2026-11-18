//! A settlement's exhibits as the run knows them: only where each LOCAL
//! RESEARCH stand is, and whether a knight stands beside one. What is shown
//! there (its title, source and claims) stays with the host's settlement.

use super::{Floor, Run, TILE_UNITS, Tile};
use serde::{Deserialize, Serialize};

pub struct Admission<'a> {
    pub id: &'a str,
    pub player: &'a str,
    pub floor: &'a Floor,
    pub exhibits: Vec<Marker>,
}

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

    pub fn beside(&self, at: usize, x: f32, y: f32) -> bool {
        self.room == at
            && (x - (self.stand.0 as f32 + 0.5) * TILE_UNITS).abs() <= TILE_UNITS * 0.7
            && (y - (self.stand.1 as f32 + 0.5) * TILE_UNITS).abs() <= TILE_UNITS * 0.7
    }
}

impl Run {
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
