//! The Jester's Half-Pipe: pump a barrel-board, spin, then straighten to land.
//! Bounded integer phases are shared by the host, mirrors, saves and renderer.
use super::{Game, Round};
use crate::rules::{Input, MAX_PLAYERS};
use serde::{Deserialize, Serialize};

pub const LIP_OPEN: u16 = 72;
pub const LIP_CLOSE: u16 = 96;
pub const FLIGHT_TICKS: u8 = 36;
pub const SPIN_STEP: u16 = 16;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Halfpipe {
    pub owner: u32,
    pub phase: u16,
    pub momentum: u8,
    pub flight: u8,
    pub spin: u16,
}
impl Halfpipe {
    pub fn new(owner: u32) -> Self {
        Self { owner, phase: 0, momentum: 20, flight: 0, spin: 0 }
    }
    pub fn direction(progress: u8, mistakes: u8) -> i8 {
        if (progress + mistakes).is_multiple_of(2) { 1 } else { -1 }
    }
    pub fn valid(&self, round: &Round) -> bool {
        round.game == Game::Halfpipe
            && (1..=MAX_PLAYERS).contains(&self.owner)
            && self.phase < LIP_CLOSE
            && self.momentum <= 100
            && self.flight < FLIGHT_TICKS
            && self.spin <= 576
            && self.spin.is_multiple_of(SPIN_STEP)
            && (self.flight != 0 || self.spin == 0)
            && (self.flight == 0 || (self.phase >= LIP_OPEN && self.momentum >= 60))
            && self.spin <= u16::from(self.flight.saturating_sub(1)) * SPIN_STEP
            && round.mistakes < 3
    }
    fn next_pass(&mut self) {
        self.phase = 0;
        self.momentum = 20;
        self.flight = 0;
        self.spin = 0;
    }
    /// None = still rolling/flying; Some = landed a trick or lost a helmet.
    pub fn step(&mut self, input: Input, tapped: bool, direction: i8) -> Option<bool> {
        let axis = if input.aim_x != 0 { input.aim_x } else { input.move_x }.signum();
        if self.flight > 0 {
            self.flight += 1;
            if axis == direction {
                self.spin = self.spin.saturating_add(SPIN_STEP).min(576);
            } else if axis != 0 {
                self.spin = self.spin.saturating_sub(SPIN_STEP);
            }
            if self.flight == FLIGHT_TICKS {
                // A full rotation is not enough: release arrows to straighten.
                let landed = (320..=400).contains(&self.spin) && axis == 0;
                self.next_pass();
                return Some(landed);
            }
            return None;
        }
        // Test the published phase before advancing it: every visible gold
        // frame is a real launch window, including its last frame.
        if tapped {
            if self.phase >= LIP_OPEN && self.phase < LIP_CLOSE && self.momentum >= 60 {
                self.flight = 1;
                return None;
            }
            self.next_pass();
            return Some(false);
        }
        if axis == direction {
            self.momentum = self.momentum.saturating_add(2).min(100);
        } else {
            self.momentum = self.momentum.saturating_sub(if axis == 0 { 1 } else { 3 });
        }
        self.phase += 1;
        if self.phase == LIP_CLOSE {
            self.next_pass();
            return Some(false);
        }
        None
    }
}
