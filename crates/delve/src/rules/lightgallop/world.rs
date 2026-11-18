//! What stands on the road: the level's fixed things (springs, pads, posts,
//! crates, the bell, where the sparks lie, where each foe is posted) and the
//! tiny pieces of live state they need. The module lays the level out once
//! (`level.rs` asks for it); none of this is saved, only the session's own
//! numbers are.
use super::math::{self, ONE};
use super::terrain::Terrain;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum SpringKind {
    Up,
    Diag,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Spring {
    pub x: i32,
    pub y: i32,
    pub kind: SpringKind,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Pad {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Post {
    pub x: i32,
    pub y: i32,
    pub path: usize,
    pub s: i32,
    pub hold: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum CrateKind {
    Shield,
    Sparks,
    Spur,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Crate {
    pub x: i32,
    pub y: i32,
    pub kind: CrateKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum FoeKind {
    Snuffer,
    Floater,
    Bramble,
    Warden,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct FoeSpot {
    pub kind: FoeKind,
    pub x: i32,
    pub y: i32,
    pub path: usize,
    pub range: i32,
    pub min_seats: u32,
    pub phase: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Sign {
    pub x: i32,
    pub y: i32,
    pub words: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct Level {
    pub terrain: Terrain,
    pub width: i32,
    pub dead_y: i32,
    pub acts: [i32; 4],
    pub main: usize,
    pub start: (usize, i32),
    pub sparks: Vec<(i32, i32)>,
    pub springs: Vec<Spring>,
    pub pads: Vec<Pad>,
    pub posts: Vec<Post>,
    pub crates: Vec<Crate>,
    pub foes: Vec<FoeSpot>,
    pub signs: Vec<Sign>,
    pub bell: (i32, i32),
    pub par: u32,
}

pub fn foe_at(level: &Level, spot: &FoeSpot, clock: u32) -> (i32, i32, i32) {
    let t = clock + spot.phase;
    match spot.kind {
        FoeKind::Bramble | FoeKind::Warden => (spot.x, spot.y, -1),
        FoeKind::Snuffer => {
            let beat = (spot.range * 2 * 5 / 3).max(2) as u32;
            let phase = (t % (beat * 2)) as i32;
            let b = beat as i32;
            let along = if phase < b { phase } else { 2 * b - phase };
            let x = spot.x - spot.range + along * 3 / 5;
            let facing = if phase < b { 1 } else { -1 };
            let y = level.terrain.paths[spot.path].y_at_x(x) / 256;
            (x, y, facing)
        }
        FoeKind::Floater => {
            let bob = (i64::from(math::sin((t as i32 * 3) % 256)) * 5 / i64::from(ONE)) as i32;
            let drift = (i64::from(math::sin((t as i32 * 3 / 2) % 256)) * i64::from(spot.range) / i64::from(ONE)) as i32;
            (spot.x + drift, spot.y + bob, -1)
        }
    }
}

pub fn foe_body(kind: FoeKind) -> (i32, i32) {
    match kind {
        FoeKind::Snuffer => (9, 20),
        FoeKind::Floater => (9, 18),
        FoeKind::Bramble => (12, 14),
        FoeKind::Warden => (22, 52),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FoeState {
    pub hp: u8,
    pub pop: u16,
    pub flinch: u8,
}
