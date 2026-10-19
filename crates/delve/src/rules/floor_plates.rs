//! Seat-local presentation, deliberately independent of combat Phase.
//! 54 display steps at 30 Hz, even in Turbo. No clock or random state.

use super::{Input, Pack, Run};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DURATION: u8 = 54;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Gate,
    Crypt,
    Mines,
    Keep,
    Archive,
    Fungal,
    Unknown,
    Swamp,
}

#[cfg(test)]
#[path = "../../tests/rules/together_floor_plates__tests.rs"]
mod tests;

impl Kind {
    pub fn of(pack: Pack) -> Self {
        match pack {
            Pack::Crypt => Self::Crypt,
            Pack::Cavern => Self::Mines,
            Pack::Hellforge => Self::Keep,
            Pack::Archive => Self::Archive,
            Pack::Fungal => Self::Fungal,
            Pack::Unknown => Self::Unknown,
            Pack::Swamp => Self::Swamp,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Gate => "THE UNDERCROFT",
            Self::Crypt => "THE CRYPT",
            Self::Mines => "THE MINES",
            Self::Keep => "DRAGON KEEP",
            Self::Archive => "DROWNED ARCHIVE",
            Self::Fungal => "FUNGAL DEEP",
            Self::Unknown => "THE UNKNOWN",
            Self::Swamp => "SWAMP OF HORRORS",
        }
    }
}

/// Every reachable destination, including both orders of the upper delve.
pub const DESTINATIONS: [(Kind, u32); 12] = [
    (Kind::Gate, 0),
    (Kind::Crypt, 1),
    (Kind::Crypt, 2),
    (Kind::Mines, 1),
    (Kind::Mines, 2),
    (Kind::Keep, 3),
    (Kind::Archive, 4),
    (Kind::Fungal, 5),
    (Kind::Unknown, 6),
    (Kind::Swamp, 1),
    (Kind::Swamp, 2),
    (Kind::Swamp, 3),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Playing {
    pub kind: Kind,
    pub depth: u32,
    pub age: u8,
    /// Display steps the floor behind gives its results (`floor_stats`)
    /// before this floor's own title; none for the first floor.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub results: u8,
}

fn is_zero(v: &u8) -> bool {
    *v == 0
}

impl Playing {
    /// The steps the whole plate lasts: the results, then the title.
    pub fn length(self) -> u8 {
        self.results.saturating_add(DURATION)
    }

    /// The floor's own title as it plays now, once the results are done.
    pub fn title(self) -> Option<Playing> {
        (self.age >= self.results).then_some(Playing { age: self.age - self.results, results: 0, ..self })
    }
}

impl Run {
    pub fn begin_floor_plate(&mut self, kind: Kind) {
        self.floor_plates = self
            .players
            .keys()
            .map(|&id| {
                (
                    id,
                    Playing {
                        kind,
                        depth: self.floor(),
                        age: 0,
                        results: 0,
                    },
                )
            })
            .collect();
    }

    pub fn floor_plate(&self, seat: Option<u32>) -> Option<Playing> {
        self.floor_plates
            .get(&seat.unwrap_or(1))
            .copied()
            .filter(|p| p.age < p.length())
    }

    pub fn skip_floor_plate(&mut self, seat: u32) -> bool {
        self.floor_plates.remove(&seat).is_some()
    }

    pub(super) fn tick_floor_plates(&mut self, inputs: &BTreeMap<u32, Input>) {
        // The results never hold a fight up: one breaking out cuts them
        // short, and the floor's title plays.
        let fight = self.fight_on();
        self.floor_plates.retain(|id, plate| {
            if fight && plate.age < plate.results {
                plate.age = plate.results;
            }
            plate.age = plate.age.saturating_add(1);
            plate.age < plate.length() && !inputs.get(id).is_some_and(|i| i.skip_plate)
        });
    }
}
