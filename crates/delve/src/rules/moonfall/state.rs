//! What a Moonfall run is made of, in plain integers. Two halves: the
//! world, which the bell winds back to the first dawn, and the memory
//! (`ledger::Memory`), which it does not.
use super::clock;
use super::folk::Folk;
use super::ledger::Fact;
use super::says::{Sign, Topic};
use super::town::{self, Q, TILE};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Item {
    #[default]
    Empty,
    Ink,
    Reeds,
    Pass,
    Scroll,
    Shoe,
}

impl Item {
    pub fn name(self) -> &'static str {
        match self {
            Self::Empty => "",
            Self::Ink => "INK POT",
            Self::Reeds => "REEDS",
            Self::Pass => "PASS",
            Self::Scroll => "SCROLL",
            Self::Shoe => "HORSESHOE",
        }
    }
    pub fn ink(self) -> char {
        match self {
            Self::Empty => 'k',
            Self::Ink => 'z',
            Self::Reeds => 'M',
            Self::Pass => 'T',
            Self::Scroll => 'W',
            Self::Shoe => 'h',
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mask {
    #[default]
    Bare,
    Swift,
    Stout,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Panel {
    #[default]
    Shut,
    Ledger(u8),
    Bell(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Note {
    Wrote(Folk, u8),
    Learned(Fact),
    Ticked(Folk),
    Remind(Folk, u8),
    Sign(Sign),
    Got(Item),
    Mask(Mask),
    Candle(u8),
    Snuffed,
    Lit(u8),
    Out,
    Ring(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ring {
    Rewind,
    Dusk,
    Dawn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Call {
    pub ring: Ring,
    pub by: u32,
    pub ayes: u8,
    pub left: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Play,
    Rewind { t: u32, from: u32, fell: bool },
    Skip { t: u32, from: u32, to: u32 },
    Won { t: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Say {
    pub folk: Folk,
    pub topic: Topic,
    pub page: u8,
    pub age: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Knight {
    pub x: i32,
    pub y: i32,
    pub face: u8,
    pub stride: u32,
    pub held: Item,
    pub held_at: u32,
    pub mask: Mask,
    pub candles: u8,
    pub hurt: u32,
    pub down: u32,
    pub roll: u32,
    pub roll_rest: u32,
    pub roll_dir: (i8, i8),
    pub keys: u16,
    pub panel: Panel,
    pub say: Option<Say>,
    pub hold: u32,
    pub push: u32,
    pub note: Option<(Note, u32)>,
    #[serde(default)]
    pub read: u32,
}

impl Knight {
    pub fn at(tile: (i32, i32)) -> Self {
        Self {
            x: (tile.0 * TILE + TILE / 2) * Q,
            y: (tile.1 * TILE + TILE / 2) * Q,
            face: 0,
            stride: 0,
            held: Item::Empty,
            held_at: 0,
            mask: Mask::Bare,
            candles: CANDLES,
            hurt: 0,
            down: 0,
            roll: 0,
            roll_rest: 0,
            roll_dir: (0, 0),
            keys: 0,
            panel: Panel::Shut,
            say: None,
            hold: 0,
            push: 0,
            note: None,
            read: 0,
        }
    }
    pub fn px(&self) -> (i32, i32) {
        (self.x.div_euclid(Q), self.y.div_euclid(Q))
    }
    pub fn up(&self) -> bool {
        self.down == 0
    }
}

pub const CANDLES: u8 = 3;
pub const PUSH_TICKS: u32 = 10;
pub const ROLL: u32 = 6;
pub const ROLL_REST: u32 = 40;
pub const WARDED: u32 = 75;
pub const DOWN: u32 = 90;

pub const SPAWN: [(i32, i32); 6] = [(16, 15), (19, 15), (15, 17), (20, 17), (16, 19), (19, 19)];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seat {
    pub joined: u32,
    pub knight: Knight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wisp {
    pub x: i32,
    pub y: i32,
    pub mode: WispMode,
    pub aim: u8,
    pub born: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WispMode {
    Hunt,
    Snuff(u32),
    Shy(u32),
}

pub const NORTH_OPEN: u16 = 1;
pub const SOUTH_OPEN: u16 = 2;
pub const INK_TAKEN: u16 = 4;
pub const DITCH_FILLED: u16 = 8;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct World {
    pub flags: u16,
    pub ink_given: Option<u32>,
    pub shod: Option<u32>,
    pub crates: Vec<(i32, i32)>,
    pub snuffed: u8,
    pub lit_until: [u32; 3],
    pub wisps: Vec<Wisp>,
}

impl World {
    pub fn dawn() -> Self {
        Self {
            flags: 0,
            ink_given: None,
            shod: None,
            crates: vec![town::CRATE_START],
            snuffed: 0,
            lit_until: [0; 3],
            wisps: Vec::new(),
        }
    }
    pub fn has(&self, flag: u16) -> bool {
        self.flags & flag != 0
    }
    pub fn set(&mut self, flag: u16) {
        self.flags |= flag;
    }
    pub fn ground(&self, clock: u32, day: u32, swift: bool) -> town::Ground {
        town::Ground {
            bridge_up: clock::bridge_up(clock, day),
            swift,
            ditch_filled: self.has(DITCH_FILLED),
        }
    }
    pub fn valid(&self) -> bool {
        self.flags < 16
            && self.crates.len() <= 1
            && self.crates.iter().all(|&(x, y)| (0..town::COLS).contains(&x) && (0..town::ROWS).contains(&y))
            && self.snuffed < 64
            && self.wisps.len() <= MAX_WISPS
            && self.wisps.iter().all(|w| {
                (-64 * Q..=(town::COLS * TILE + 64) * Q).contains(&w.x)
                    && (-64 * Q..=(town::ROWS * TILE + 64) * Q).contains(&w.y)
                    && w.aim < 9
            })
    }
}

pub const MAX_WISPS: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Nums {
    pub day: u32,
    pub walk: u32,
    pub wisps: u32,
    pub rewind: u32,
    pub notice: u32,
}
