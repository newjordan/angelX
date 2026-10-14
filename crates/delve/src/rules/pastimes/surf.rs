//! Shield Surf: balance against an announced wave and tap at its crest.
//! Integer state belongs to one living shore-side knight, not to guest pixels.
use super::{HZ, Round};
use crate::rules::{Input, MAX_PLAYERS};
use serde::{Deserialize, Serialize};

pub const WAVE_TICKS: u32 = 3 * HZ;
pub const CREST_WINDOW: u32 = HZ / 2;
pub const WAVE_LEANS: [i32; 6] = [-48, 48, 0, -48, 48, 0];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Surf {
    pub owner: u32,
    pub lean: i32,
    pub phase: u32,
    pub braced: bool,
}
impl Surf {
    pub fn new(owner: u32) -> Self {
        Self {
            owner,
            lean: 0,
            phase: 0,
            braced: false,
        }
    }
    pub fn valid(&self, round: &Round) -> bool {
        (1..=MAX_PLAYERS).contains(&self.owner)
            && (-100..=100).contains(&self.lean)
            && self.phase < WAVE_TICKS
            && (!self.braced || self.phase >= WAVE_TICKS - CREST_WINDOW)
            && round.progress < 6
            && round.mistakes == 0
            && super::Game::Surf.time() - round.left
                == u32::from(round.progress) * WAVE_TICKS + self.phase
    }
    /// Some(true): a wave cleared; Some(false): wipeout; None: still balancing.
    pub fn step(&mut self, input: Input, tapped: bool, wave: u8) -> Option<bool> {
        let axis = if input.aim_x != 0 {
            input.aim_x
        } else {
            input.move_x
        };
        let push = if axis < 0 {
            -4
        } else if axis > 0 {
            4
        } else {
            0
        };
        self.lean = (self.lean + push).clamp(-100, 100);
        let target = WAVE_LEANS[usize::from(wave)];
        // F answers the gold crest in the snapshot the knight just saw. Do not
        // open its window a tick early by advancing the phase before judging.
        if tapped && self.phase >= WAVE_TICKS - CREST_WINDOW && (self.lean - target).abs() <= 16 {
            self.braced = true;
        }
        self.phase += 1;
        if self.phase < WAVE_TICKS {
            return None;
        }
        let cleared = self.braced && (self.lean - target).abs() <= 16;
        self.phase = 0;
        self.braced = false;
        Some(cleared)
    }
}
