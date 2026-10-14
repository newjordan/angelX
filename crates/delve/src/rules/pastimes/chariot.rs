//! Fixed-point, host-stepped chariot racing: one driver, three comic rivals.
//! Free practice pays records, never gold. The player's knight stays on foot.
use super::{Game, Round};
use crate::rules::{HZ, Input, MAX_PLAYERS};
use serde::{Deserialize, Serialize};

pub const LAP: u16 = 4096;
pub const FINISH: u16 = LAP * 3;
pub const RIVALS: [&str; 3] = ["SIR RATTLE", "GOAT MEATLOAF", "ABBOT OVERDRIVE"];
pub const BARRELS: [(u16, u16); 4] = [(560, 0), (1550, 256), (2670, 512), (3500, 256)];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chariot {
    pub owner: u32,
    pub distance: u16,
    pub lane: u16,
    pub speed: u8,
    pub rivals: [u16; 3],
    pub stuns: [u8; 3],
    pub bog: u8,
    pub cabbages: u8,
    pub reload: u8,
}
impl Chariot {
    pub fn new(owner: u32) -> Self {
        Self { owner, distance: 0, lane: 256, speed: 0, rivals: [0; 3], stuns: [0; 3], bog: 0, cabbages: 3, reload: 0 }
    }
    pub fn valid(&self, round: &Round) -> bool {
        round.game == Game::Chariot
            && (1..=MAX_PLAYERS).contains(&self.owner)
            && self.distance < FINISH && self.lane <= 512 && self.speed <= 20
            && self.rivals.iter().all(|&d| d < FINISH)
            && self.stuns.iter().all(|&s| s <= HZ as u8)
            && self.bog <= 45 && self.cabbages <= 3 && self.reload <= HZ as u8
            && round.progress == (self.distance / LAP) as u8
    }
    /// +1 = won, -1 = rival won, 0 = racing. Only the owner supplies controls.
    pub fn step(&mut self, input: Input, tapped: bool) -> i8 {
        let steer = if input.aim_x != 0 { input.aim_x } else { input.move_x }.signum();
        self.lane = (i32::from(self.lane) + i32::from(steer) * 20).clamp(0, 512) as u16;
        self.reload = self.reload.saturating_sub(1);
        if tapped && self.reload == 0 && self.cabbages > 0 {
            self.cabbages -= 1;
            self.reload = HZ as u8;
            // A cabbage hits the nearest opponent ahead, only in its lane.
            if let Some(i) = (0..3).filter(|&i| {
                self.rivals[i] >= self.distance
                    && self.rivals[i] - self.distance < 640
                    && self.lane.abs_diff(i as u16 * 256) < 90
            }).min_by_key(|&i| self.rivals[i]) {
                self.stuns[i] = HZ as u8;
            }
        }
        let throttle = if input.aim_y != 0 { input.aim_y } else { input.move_y };
        let target = if self.bog > 0 { 6 } else if throttle < 0 { 20 } else if throttle > 0 { 8 } else { 15 };
        self.speed = if self.speed < target { self.speed + 1 } else { self.speed.saturating_sub(1).max(target) };
        self.bog = self.bog.saturating_sub(1);
        let before = self.distance;
        self.distance = self.distance.saturating_add(u16::from(self.speed)).min(FINISH);
        for lap in 0..3 {
            for &(pos, lane) in &BARRELS {
                let obstacle = lap * LAP + pos;
                if before < obstacle && self.distance >= obstacle && self.lane.abs_diff(lane) < 90 {
                    self.bog = 45;
                }
            }
        }
        if self.distance / LAP > before / LAP {
            self.cabbages = 3;
        }
        // Equal-tick finishes favour the player; outcomes are deterministic.
        if self.distance == FINISH { return 1; }
        for i in 0..3 {
            if self.stuns[i] > 0 {
                self.stuns[i] -= 1;
            } else {
                self.rivals[i] = self.rivals[i].saturating_add([15, 16, 14][i]).min(FINISH);
            }
        }
        if self.rivals.contains(&FINISH) { -1 } else { 0 }
    }
}
