//! # CHANGE RINGING
//!
//! ## The feeling
//! Standing under a tower bell with the rope in both hands: the swing
//! gathers, the gold moment comes round, and you pull exactly as it
//! arrives — the whole belfry answers. In the first 10 seconds a knight
//! must feel the ring land on the gold and hear the bell speak for their
//! pull, then feel the next swing come round a little sooner.
//!
//! ## The loop (one sentence)
//! Watch the ring close on the bell, tap F as it turns gold to ring that
//! strike, and ring ten strikes of the thirteen-strike peal before the
//! peal runs out — an early pull is a clanger and spoils that strike.
//!
//! ## Words on the board
//! RING ON GOLD
//!
//! ## Who plays
//! Every seated knight, at once, on their own screen (a heat: each
//! knight rings the same peal in their own round, first home is crowned).
//!
//! ## Controls
//! Feet stand at the rope (the start ring). F pulls. Nothing else.
//!
//! ## The five numbers (ticks; HZ = 30)
//! 1. FIRST = 42 — ticks the first swing takes. A breath to learn the pull.
//! 2. STEP = 2 — ticks each later swing takes off the one before. The peal
//!    hurries under you; the last swings arrive at 18 ticks.
//! 3. WINDOW = 10 — gold ticks at the end of a swing where a pull rings
//!    true. Generous on purpose: the 30 Hz tick is the timing grain.
//! 4. STRIKES = 13 — strikes in the peal. Three can be missed or clanged.
//! 5. NEED = 10 (`Game::need`) — strikes rung true to win the peal.
//!
//! ## Fail and retry
//! A clanger or a missed strike costs 25 and the peal rolls on to the
//! next swing inside a second. A fourth spoiled strike ends it ("the
//! peal fell apart"); step off the ring and back on to ring again.
//!
//! ## Version 1 is only
//! One room (the Bashful Belfry), one verb (pull on gold), all seated
//! knights, a start and a finish. Left out: distinct bell parts per seat
//! (every ringer rings the same peal, so a heat race stays honest), rest
//! strikes you must not ring, and any new bell art beyond the ring cue.
//!
//! ## Pays
//! The arcade's bounded purse: score only (25–1,000 by the standing
//! formula), the realm's 25 gold once for the game's first win, and in a
//! heat a laurel for the winner and the bell-hat booby for the rest.
use super::Round;
use crate::rules::MAX_PLAYERS;
use serde::{Deserialize, Serialize};

pub const FIRST: u32 = 42;
pub const STEP: u32 = 2;
pub const WINDOW: u32 = 10;
pub const STRIKES: u8 = 13;
pub const FLASH: u8 = 6;

pub fn interval(struck: u8) -> u32 {
    FIRST - STEP * u32::from(struck.min(STRIKES - 1))
}

pub fn elapsed(struck: u8) -> u32 {
    (0..struck.min(STRIKES)).map(interval).sum()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Peal {
    pub owner: u32,
    pub phase: u32,
    pub struck: u8,
    pub rung: bool,
    pub spoiled: bool,
    #[serde(default)]
    pub ring_flash: u8,
    #[serde(default)]
    pub clang_flash: u8,
}
impl Peal {
    pub fn new(owner: u32) -> Self {
        Self { owner, phase: 0, struck: 0, rung: false, spoiled: false, ring_flash: 0, clang_flash: 0 }
    }
    pub fn span(&self) -> u32 {
        interval(self.struck)
    }
    pub fn in_window(&self) -> bool {
        !self.rung && !self.spoiled && self.phase + WINDOW >= self.span()
    }
    pub fn valid(&self, round: &Round) -> bool {
        (1..=MAX_PLAYERS).contains(&self.owner)
            && self.struck < STRIKES
            && self.phase < self.span()
            && !(self.rung && self.spoiled)
            && self.ring_flash <= FLASH
            && self.clang_flash <= FLASH
            && round.progress <= self.struck
            && u32::from(round.mistakes) == u32::from(self.struck - round.progress)
            && super::Game::Peal.time() - round.left == elapsed(self.struck) + self.phase
    }
    pub fn step(&mut self, tapped: bool) -> Option<bool> {
        if tapped && !self.rung && !self.spoiled {
            if self.phase + WINDOW >= self.span() {
                self.rung = true;
                self.ring_flash = FLASH;
            } else {
                self.spoiled = true;
                self.clang_flash = FLASH;
            }
        }
        self.ring_flash = self.ring_flash.saturating_sub(1);
        self.clang_flash = self.clang_flash.saturating_sub(1);
        self.phase += 1;
        if self.phase < self.span() {
            return None;
        }
        let hit = self.rung;
        self.phase = 0;
        self.rung = false;
        self.spoiled = false;
        self.struck += 1;
        Some(hit)
    }
}
