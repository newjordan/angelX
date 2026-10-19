//! What a Moonfall run is made of, in plain integers. Two halves: the
//! world, which the bell winds back to the first dawn, and the memory
//! (`ledger::Memory`), which it does not.
use super::clock;
use super::folk::Folk;
use super::ledger::Fact;
use super::says::{Sign, Topic};
use super::town::{self, Q, TILE};
use serde::{Deserialize, Serialize};

/// A thing a knight carries, one at a time.
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

/// The masks a knight wears: one at a time, and each changes the walk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mask {
    #[default]
    Bare,
    Swift,
    Stout,
}

/// Which page or menu a knight has open. The town keeps running behind it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Panel {
    #[default]
    Shut,
    /// The Ledger at a page: 0 is the town's, 1 to 6 are the folk's.
    Ledger(u8),
    /// The bell's menu at a line: rewind, wait for dusk, wait for dawn.
    Bell(u8),
}

/// A short word shown to a knight (or the table) for a moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Note {
    /// The Ledger wrote a routine line: folk, routine.
    Wrote(Folk, u8),
    /// The Ledger learned a fact.
    Learned(Fact),
    /// A favour is ticked.
    Ticked(Folk),
    /// An hour before an appointment.
    Remind(Folk, u8),
    Sign(Sign),
    Got(Item),
    /// A mask is earned.
    Mask(Mask),
    /// A candle is lost: how many are left.
    Candle(u8),
    /// A knight was snuffed out and carried to the well.
    Snuffed,
    /// The tower or the well is alight.
    Lit(u8),
    /// A lantern went out.
    Out,
    /// A knight rang or asks the bell.
    Ring(u32),
}

/// The bell's three rings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ring {
    /// Back to the first dawn.
    Rewind,
    /// Forward to the next dusk.
    Dusk,
    /// Forward to the next dawn.
    Dawn,
}

/// A knight asking for the bell, waiting for the table to agree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Call {
    pub ring: Ring,
    pub by: u32,
    /// A bit for each seat that has put its hand on the rope.
    pub ayes: u8,
    pub left: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Play,
    /// The ritual: the clock spins back from `from`, the bell tolls, the
    /// town returns to its first dawn. `fell`: the moon came down.
    Rewind { t: u32, from: u32, fell: bool },
    /// Time runs on to `to` while the sky turns.
    Skip { t: u32, from: u32, to: u32 },
    /// The three lights hold: the moon turns back.
    Won { t: u32 },
}

/// What a knight is saying to or hearing from a villager.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Say {
    pub folk: Folk,
    pub topic: Topic,
    pub page: u8,
    pub age: u32,
}

/// One knight at the table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Knight {
    /// Feet, in pixels times `Q`.
    pub x: i32,
    pub y: i32,
    /// 0 south, 1 north, 2 west, 3 east.
    pub face: u8,
    /// Ticks spent walking, for the stride.
    pub stride: u32,
    pub held: Item,
    /// The tick of the loop the thing in hand was taken.
    pub held_at: u32,
    pub mask: Mask,
    pub candles: u8,
    /// Warded from the next touch for this long.
    pub hurt: u32,
    /// Snuffed out: ticks until carried back to the well (0: on their feet).
    pub down: u32,
    /// A sidestep under way, and the rest before the next.
    pub roll: u32,
    pub roll_rest: u32,
    pub roll_dir: (i8, i8),
    /// The keys down last tick, for the presses.
    pub keys: u16,
    pub panel: Panel,
    pub say: Option<Say>,
    /// How long E has been held at a thing that wants holding.
    pub hold: u32,
    /// Ticks leant on a crate.
    pub push: u32,
    pub note: Option<(Note, u32)>,
    /// How much of the Ledger this knight had read when they last closed it.
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

/// Candles a knight starts each dawn with.
pub const CANDLES: u8 = 3;

/// Where the knights stand at each dawn: around the Moonwell.
pub const SPAWN: [(i32, i32); 6] = [(16, 15), (19, 15), (15, 17), (20, 17), (16, 19), (19, 19)];

/// A seat at the table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seat {
    /// The tick of the run this knight sat down at (0: from the start).
    pub joined: u32,
    pub knight: Knight,
}

/// A hooded wisp of the Gloam.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wisp {
    pub x: i32,
    pub y: i32,
    pub mode: WispMode,
    /// The light it is after: a lantern 0 to 5, or 6 + a tower.
    pub aim: u8,
    pub born: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WispMode {
    /// Drifting toward the nearest lit light.
    Hunt,
    /// At the light, putting it out: ticks left.
    Snuff(u32),
    /// Driven off by a knight's lantern: ticks left.
    Shy(u32),
}

/// Flags of the loop (all wound back at dawn).
pub const NORTH_OPEN: u16 = 1;
pub const SOUTH_OPEN: u16 = 2;
pub const INK_TAKEN: u16 = 4;
pub const DITCH_FILLED: u16 = 8;

/// The town as the bell winds it back.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct World {
    pub flags: u16,
    /// The tick the ink pot went to Orin; his pass is ready two hours on.
    pub ink_given: Option<u32>,
    /// The tick the horse was shod.
    pub shod: Option<u32>,
    /// Crates, by tile.
    pub crates: Vec<(i32, i32)>,
    /// Lanterns the Gloam has put out (a bit each).
    pub snuffed: u8,
    /// Until what tick each flame burns: north tower, south tower, well.
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

/// The most wisps the Gloam sends at once.
pub const MAX_WISPS: usize = 24;

/// The numbers the five knobs set, copied in each tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Nums {
    pub day: u32,
    pub walk: u32,
    pub wisps: u32,
    pub rewind: u32,
    pub notice: u32,
}
