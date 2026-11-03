//! The one context button. E opens, reads, lifts, throws, takes a flame,
//! lights, unlocks, relights; its label (`Ctx::label`) changes with what the
//! knight faces. With nothing to do it is the roll, or, locked on, the hop.
use super::*;

/// Where the party stands on keys: shared, like the Delve's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hold {
    /// Not found yet.
    Unfound,
    /// Carried by a seat, last seen here.
    Carried(u8, i32, i32),
    /// Put down where its carrier fell.
    Lying(i32, i32),
    /// Spent on the door.
    Spent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Party {
    pub map: bool,
    pub lantern: bool,
    pub small_keys: u8,
    pub boss: Hold,
    /// Heart containers found.
    pub containers: u8,
}

impl Default for Party {
    fn default() -> Self {
        Party { map: false, lantern: false, small_keys: 0, boss: Hold::Unfound, containers: 0 }
    }
}

/// The item held up for all to see, whose it is and for how long.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Banner {
    pub item: Item,
    pub seat: u8,
    pub t: u16,
}
