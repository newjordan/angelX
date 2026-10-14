//! What the session's deeds put on the map, as plain facts: who is out on
//! the roads (wagons, couriers, ravens, field hands...), and the record
//! each finished deed leaves where it was done. The host decides both from
//! its own work; the scene draws them.

use super::map::TILE;

/// Trials the Lists fence remembers; older pennants come down.
pub const PENNANTS: usize = 8;

/// World ticks the anvil throws sparks after a strike.
pub const SPARKS: u64 = 16;

/// Who goes out for a deed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Errand {
    Wagon,
    Courier,
    Raven,
    Hand,
    Messenger,
    Villager,
    Monk,
    Owl,
}

/// One figure out on a deed, as the map draws it (world pixels, feet).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wayfarer {
    pub errand: Errand,
    pub x: f32,
    pub y: f32,
    pub facing_left: bool,
    pub moving: bool,
    /// `Some(passed)` once every call it went out for has come back.
    pub verdict: Option<bool>,
    /// Bearing its load: a raven's scroll, a villager's sack, a messenger's
    /// sheaf, an owl's finding.
    pub carrying: bool,
}

/// The session's record, kept in the realm where each deed was done.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Record {
    /// Trials, newest last: `true` passed. The fence keeps [`PENNANTS`].
    pub pennants: Vec<bool>,
    /// Commits sealed and flown.
    pub ravens: u32,
    /// Distinct files forged.
    pub blades: u32,
    /// Distinct files studied.
    pub books: u32,
    /// Searches harvested.
    pub stooks: u32,
    /// Errands delivered.
    pub sacks: u32,
    /// Memories kept or recalled.
    pub candles: u32,
    /// Research findings.
    pub stars: u32,
}

/// The Rookery roost in authored pixels: its left end, and the base it
/// stands on in front of the tower.
pub const ROOST: (i32, i32) = (25 * TILE, 15 * TILE - 2);

/// Where the stooks stand in the fields (authored tiles), in the order the
/// harvests set them.
pub const STOOKS: [(i32, i32); 12] = [
    (35, 24),
    (42, 25),
    (36, 26),
    (44, 25),
    (42, 29),
    (34, 26),
    (35, 30),
    (43, 28),
    (37, 24),
    (45, 25),
    (34, 29),
    (44, 29),
];
