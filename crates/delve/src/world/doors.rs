//! The realm's doors: every place, castle and gate with the tile in front
//! of it, and the ground the roads reach from the Keep. The kingdom's
//! geography builds its sites on them; the cockpit's visitor walks to them.

use std::collections::VecDeque;
use std::sync::OnceLock;

use super::houses::{self, HouseId};
use super::kingdom::step_cost;
use super::map::{MAP_H, MAP_W, Place};
use super::{college, wishes};

/// Where a door leads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Door {
    Place(Place),
    Castle(HouseId),
    Delve,
    College,
}

impl Door {
    /// The name on its sign.
    pub fn sign(self) -> String {
        match self {
            Door::Place(place) => place.label().to_string(),
            Door::Castle(id) => houses::get(id).castle.to_uppercase(),
            Door::Delve => "THE DELVE".to_string(),
            Door::College => crate::rules::college::NAME.to_ascii_uppercase(),
        }
    }

    /// The name in plain words, for a caption.
    pub fn name(self) -> String {
        match self {
            Door::Castle(id) => houses::get(id).castle.clone(),
            door => title_case(&door.sign()),
        }
    }

    /// What stands behind the door, in realm tiles: a click on it walks
    /// him to the door.
    pub fn footprint(self) -> (i32, i32, i32, i32) {
        match self {
            Door::Place(place) => place.footprint_world(),
            Door::Castle(id) => houses::footprint(id),
            Door::College => {
                let lot = college::lot();
                (lot.tx, lot.ty, lot.tw, lot.th)
            }
            Door::Delve => {
                let gate = wishes::delve_gate();
                (gate.tx, gate.ty, gate.tw, gate.th)
            }
        }
    }
}

/// `THE LISTS` as `The Lists`.
fn title_case(text: &str) -> String {
    text.split(' ')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first
                    .to_uppercase()
                    .chain(chars.flat_map(char::to_lowercase))
                    .collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Every door in the realm, with the tile in front of it. A door no road
/// reaches opens from the walkable ground across from what stands behind
/// it: the Delve's gate is in a pocket of rock and stream (the knight, with
/// no road there, walks straight in), so the visitor goes in from the bank
/// across the stream.
pub fn doors() -> &'static [((i32, i32), Door)] {
    static DOORS: OnceLock<Vec<((i32, i32), Door)>> = OnceLock::new();
    DOORS.get_or_init(|| {
        Place::ALL
            .into_iter()
            .map(|place| (place.stand_world(), Door::Place(place)))
            .chain(houses::ids().map(|id| (houses::stand(id), Door::Castle(id))))
            .chain([
                (wishes::delve_stand(), Door::Delve),
                (super::college::stand(), Door::College),
            ])
            .map(|(stand, door)| {
                if walkable(stand) {
                    return (stand, door);
                }
                let (x, y, w, h) = door.footprint();
                (across_from((x + w / 2, y + h / 2)).unwrap_or(stand), door)
            })
            .collect()
    })
}

/// Every tile the roads reach from the Keep: the ground he can walk.
fn reached() -> &'static [bool] {
    static REACHED: OnceLock<Vec<bool>> = OnceLock::new();
    REACHED.get_or_init(|| {
        let idx = |(x, y): (i32, i32)| (y * MAP_W + x) as usize;
        let mut seen = vec![false; (MAP_W * MAP_H) as usize];
        let start = Place::Keep.stand_world();
        let mut open = VecDeque::from([start]);
        seen[idx(start)] = true;
        while let Some((x, y)) = open.pop_front() {
            for next in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                if step_cost(next.0, next.1).is_some() && !seen[idx(next)] {
                    seen[idx(next)] = true;
                    open.push_back(next);
                }
            }
        }
        seen
    })
}

/// Whether the roads reach `tile`.
pub fn walkable((x, y): (i32, i32)) -> bool {
    x >= 0 && y >= 0 && x < MAP_W && y < MAP_H && reached()[(y * MAP_W + x) as usize]
}

/// The walkable ground across from `tile`: nearest it, and level with it
/// before above or below it, since the map is wider than it is tall.
pub fn across_from(tile: (i32, i32)) -> Option<(i32, i32)> {
    (0..MAP_H)
        .flat_map(|y| (0..MAP_W).map(move |x| (x, y)))
        .filter(|&t| walkable(t))
        .min_by_key(|&(x, y)| ((x - tile.0).pow(2) + 4 * (y - tile.1).pow(2), y, x))
}
