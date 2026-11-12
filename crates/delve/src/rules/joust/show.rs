//! The lists' show: what lately happened at the tilt, kept so the picture
//! can make a fuss of it (`render::overworld::arena::joust` draws it).
//!
//! The rules (`joust`) tell it what happened: a rider spurred, a lance
//! aimed, a shield turned, the riders met (what each lance did, how hard it
//! struck, who went over, whether the crowd steadied a seat) and a favour
//! thrown from the gallery. It moves nobody: the bout stays what it was, and
//! the show is only memory, with the juice kit's own weights
//! (`juice::Weight`) saying how hard each meeting hits.
//!
//! Time is the show's own tick count (`Show::t`), the bout's, which stands
//! still through a hit-stop like the rest of the room, so the host, a
//! friend's mirror and a browser guest draw the same moment. Every list is
//! capped (`valid`).

use super::{Aim, Hit};
use crate::rules::MAX_PLAYERS;
use crate::rules::juice::Weight;
use serde::{Deserialize, Serialize};

/// Ticks a favour stays in the air and on the books.
pub const FAVOUR_LIFE: u32 = 36;
/// The most favours the books keep.
const MOST_FAVOURS: usize = 12;

/// How hard a lance struck the rider who took it: a shield takes a solid
/// blow, a lance that lands clean a crushing one, one at the very moment a
/// killing one, and a fall is the finale.
pub fn weight_of(hit: Hit, down: bool) -> Weight {
    match (hit, down) {
        (Hit::Miss, _) => Weight::Graze,
        (_, true) => Weight::Finale,
        (Hit::Shield, _) => Weight::Solid,
        (Hit::Broke | Hit::Clean, _) => Weight::Crushing,
        (Hit::Smite, _) => Weight::Kill,
    }
}

/// One meeting of the riders, as it was.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Meet {
    /// The show's tick it happened on.
    pub born: u32,
    pub course: u8,
    /// What each lance did: the knight's, the other rider's.
    pub hits: (Hit, Hit),
    /// Where each lance pointed.
    pub aims: (Aim, Aim),
    /// How hard each rider was struck (by the other's lance).
    pub taken: (Weight, Weight),
    /// Whether the crowd steadied each rider, taking a knock off him.
    pub steady: (bool, bool),
    /// Who went over.
    pub down: (bool, bool),
    /// Where along the lists they met (arena units).
    pub x: f32,
}

impl Meet {
    pub fn age(&self, t: u32) -> u32 {
        t.saturating_sub(self.born)
    }
    /// The heaviest blow of the two.
    pub fn heaviest(&self) -> Weight {
        self.taken.0.max(self.taken.1)
    }
    /// Whether a lance landed at all.
    pub fn landed(&self) -> bool {
        self.hits.0 != Hit::Miss || self.hits.1 != Hit::Miss
    }
}

/// A favour thrown from the gallery to a rider.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Favour {
    /// The knight in the gallery who threw it.
    pub by: u32,
    /// The rider it was thrown to: 0 the knight, 1 the other.
    pub side: u8,
    pub born: u32,
}

impl Favour {
    pub fn age(&self, t: u32) -> u32 {
        t.saturating_sub(self.born)
    }
}

/// The lists' whole memory for the picture.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Show {
    /// Ticks the bout has run (not through a hit-stop).
    pub t: u32,
    /// Every meeting so far: one a pass, three at most.
    pub meets: Vec<Meet>,
    pub favours: Vec<Favour>,
    /// When each rider last turned his lance, and his shield.
    pub aimed: [Option<u32>; 2],
    pub shielded: [Option<u32>; 2],
    /// When each rider last spurred.
    pub spurred: [Option<u32>; 2],
}

impl Show {
    pub fn quiet(&self) -> bool {
        *self == Self::default()
    }

    pub fn valid(&self) -> bool {
        let seat = |id: u32| (1..=MAX_PLAYERS).contains(&id);
        self.t <= 120 * crate::rules::HZ * 4
            && self.meets.len() <= 3
            && self.meets.iter().all(|m| m.course < 3 && m.x.is_finite())
            && self.favours.len() <= MOST_FAVOURS
            && self.favours.iter().all(|f| seat(f.by) && f.side < 2)
    }

    /// The meeting of the pass being ridden or just ridden, if there is one.
    pub fn last(&self) -> Option<&Meet> {
        self.meets.last()
    }

    /// A favour thrown: kept while it flies.
    pub fn throw(&mut self, by: u32, side: u8) {
        if self.favours.len() >= MOST_FAVOURS {
            self.favours.remove(0);
        }
        self.favours.push(Favour { by, side, born: self.t });
    }

    /// A meeting recorded.
    pub fn meet(&mut self, m: Meet) {
        if self.meets.len() >= 3 {
            self.meets.remove(0);
        }
        self.meets.push(m);
    }

    /// Let go the favours that have landed.
    pub fn tick(&mut self) {
        self.t += 1;
        let t = self.t;
        self.favours.retain(|f| f.age(t) < FAVOUR_LIFE);
    }
}
