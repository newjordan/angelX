//! The one context button. E opens, reads, lifts, throws, takes a flame,
//! lights, unlocks, relights; its label (`Ctx::label`) changes with what the
//! knight faces. With nothing to do it is the roll, or, locked on, the hop.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hold {
    Unfound,
    Carried(u8, i32, i32),
    Lying(i32, i32),
    Spent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Party {
    pub map: bool,
    pub lantern: bool,
    pub small_keys: u8,
    pub boss: Hold,
    pub containers: u8,
}

impl Default for Party {
    fn default() -> Self {
        Party { map: false, lantern: false, small_keys: 0, boss: Hold::Unfound, containers: 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Banner {
    pub item: Item,
    pub seat: u8,
    pub t: u16,
}
