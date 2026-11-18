//! What the session's deeds put on the map, as plain facts: who is out on
//! the roads (wagons, couriers, ravens, field hands...), and the record
//! each finished deed leaves where it was done. The host decides both from
//! its own work; the scene draws them.

use super::map::TILE;

pub const PENNANTS: usize = 8;

pub const SPARKS: u64 = 16;

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wayfarer {
    pub errand: Errand,
    pub x: f32,
    pub y: f32,
    pub facing_left: bool,
    pub moving: bool,
    pub verdict: Option<bool>,
    pub carrying: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Record {
    pub pennants: Vec<bool>,
    pub ravens: u32,
    pub blades: u32,
    pub books: u32,
    pub stooks: u32,
    pub sacks: u32,
    pub candles: u32,
    pub stars: u32,
}

pub const ROOST: (i32, i32) = (25 * TILE, 15 * TILE - 2);

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
