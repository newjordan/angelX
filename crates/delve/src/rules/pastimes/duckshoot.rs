//! The Carnival Duck Range: a BB gun, three rails of very confident ducks.
//! The camera is bolted to the counter, so each rail's place on screen is a
//! constant — the hit test and the painter agree by construction, not by
//! coincidence. Integer state belongs to one gunner; friends share the round.

use super::{Game, Round};
use crate::rules::{Input, MAX_PLAYERS};
use serde::{Deserialize, Serialize};

/// One rail: lane position in 256ths of a tile from the booth's centre, the
/// signed slide speed, and the knocked-over clock (0 = standing proud).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Duck {
    pub x: i16,
    pub vx: i16,
    pub down: u8,
}

pub const LANE: i16 = 1800;
/// Rails at 14, 20 and 26 tiles out, ducks at chest, chin and crown height.
pub const RAIL_Y: [f32; 3] = [14.0, 20.0, 26.0];
pub const RAIL_Z: [f32; 3] = [1.5, 2.2, 2.8];
/// The counter camera: eye height, canvas focal length (78° across 384px).
pub const EYE_Z: f32 = 4.5;
pub const FOCAL: f32 = 237.0;
/// Each rail's lane-to-screen scale and screen height, fixed by the camera.
pub const RAIL_K: [i32; 3] = [17, 12, 9];
pub const RAIL_SY: [i32; 3] = [163, 139, 128];
/// Hit boxes in pixels: generous near, honest far.
pub const HIT_W: [i32; 3] = [10, 8, 7];
pub const HIT_H: [i32; 3] = [12, 10, 9];
/// A knocked duck lies about while the barker reassures it, then stands.
pub const DOWN_RESET: u8 = 30;
pub const MAX_SHOTS: u8 = 40;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuckShoot {
    pub owner: u32,
    /// The crosshair, in stage pixels: the same numbers the painter stamps.
    pub ax: i16,
    pub ay: i16,
    pub shots: u8,
    /// Ticks of muzzle flash left.
    pub flash: u8,
    pub ducks: [Duck; 3],
}

impl DuckShoot {
    pub fn new(owner: u32) -> Self {
        Self {
            owner,
            ax: 192,
            ay: 130,
            shots: 0,
            flash: 0,
            ducks: [
                Duck {
                    x: -900,
                    vx: 150,
                    down: 0,
                },
                Duck {
                    x: 400,
                    vx: -110,
                    down: 0,
                },
                Duck {
                    x: 1200,
                    vx: 75,
                    down: 0,
                },
            ],
        }
    }

    pub fn valid(&self, round: &Round) -> bool {
        (1..=MAX_PLAYERS).contains(&self.owner)
            && (8..=376).contains(&self.ax)
            && (24..=200).contains(&self.ay)
            && self.shots <= MAX_SHOTS
            && self.flash <= 3
            && self.ducks.iter().all(|d| {
                d.x >= -(LANE + 200)
                    && d.x <= LANE + 200
                    && d.vx.unsigned_abs() >= 60
                    && d.vx.unsigned_abs() <= 180
                    && d.down <= DOWN_RESET
            })
            && round.game == Game::DuckShoot
    }

    /// A rail's lane position on screen: the painter's arithmetic, in ints.
    pub fn screen_x(lane: i16, rail: usize) -> i32 {
        192 + i32::from(lane) * RAIL_K[rail] / 256
    }

    /// One tick: slide the rails, swing the gun, and on a tap, take the shot.
    /// `Some(rail)` = a duck on that rail is knocked over this tick.
    pub fn step(&mut self, input: Input, tapped: bool) -> Option<usize> {
        let (ax, ay) = (
            if input.aim_x != 0 {
                input.aim_x
            } else {
                input.move_x
            },
            if input.aim_y != 0 {
                input.aim_y
            } else {
                input.move_y
            },
        );
        self.ax = (self.ax + i16::from(ax) * 6).clamp(8, 376);
        self.ay = (self.ay + i16::from(ay) * 6).clamp(24, 200);
        self.flash = self.flash.saturating_sub(1);
        for duck in &mut self.ducks {
            duck.x += duck.vx;
            if duck.x > LANE {
                duck.x = 2 * LANE - duck.x;
                duck.vx = -duck.vx;
            } else if duck.x < -LANE {
                duck.x = -2 * LANE - duck.x;
                duck.vx = -duck.vx;
            }
            if duck.down > 0 {
                duck.down = (duck.down + 1).min(DOWN_RESET);
                if duck.down == DOWN_RESET {
                    duck.down = 0;
                    // It stands again facing the way it was going, dignity
                    // intact, at the rail's edge.
                    duck.x = if duck.vx < 0 { LANE } else { -LANE };
                }
            }
        }
        if !tapped || self.shots >= MAX_SHOTS {
            return None;
        }
        self.shots += 1;
        self.flash = 3;
        for (rail, duck) in self.ducks.iter_mut().enumerate() {
            if duck.down > 0 {
                continue;
            }
            let sx = Self::screen_x(duck.x, rail);
            if (i32::from(self.ax) - sx).abs() <= HIT_W[rail]
                && (i32::from(self.ay) - RAIL_SY[rail]).abs() <= HIT_H[rail]
            {
                duck.down = 1;
                return Some(rail);
            }
        }
        None
    }
}
