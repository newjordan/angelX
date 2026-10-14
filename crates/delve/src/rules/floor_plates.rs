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
        }
    }
}

/// Every reachable destination, including both orders of the upper delve.
pub const DESTINATIONS: [(Kind, u32); 9] = [
    (Kind::Gate, 0),
    (Kind::Crypt, 1),
    (Kind::Crypt, 2),
    (Kind::Mines, 1),
    (Kind::Mines, 2),
    (Kind::Keep, 3),
    (Kind::Archive, 4),
    (Kind::Fungal, 5),
    (Kind::Unknown, 6),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Playing {
    pub kind: Kind,
    pub depth: u32,
    pub age: u8,
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
                    },
                )
            })
            .collect();
    }

    pub fn floor_plate(&self, seat: Option<u32>) -> Option<Playing> {
        self.floor_plates
            .get(&seat.unwrap_or(1))
            .copied()
            .filter(|p| p.age < DURATION)
    }

    pub fn skip_floor_plate(&mut self, seat: u32) -> bool {
        self.floor_plates.remove(&seat).is_some()
    }

    pub(super) fn tick_floor_plates(&mut self, inputs: &BTreeMap<u32, Input>) {
        self.floor_plates.retain(|id, plate| {
            plate.age = plate.age.saturating_add(1);
            plate.age < DURATION && !inputs.get(id).is_some_and(|i| i.skip_plate)
        });
    }
}
