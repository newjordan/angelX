//! Belfry Bat-and-Ball: a shield paddle, a turnip, and four absurd tax bells.
//! Fixed-point motion and input edges are simulation-owned and checkpointable.
use super::{Game, Round};
use crate::rules::{Input, MAX_PLAYERS};
use serde::{Deserialize, Serialize};

pub const COURT_LEFT: i32 = 6 * 256;
pub const COURT_RIGHT: i32 = 18 * 256;
pub const COURT_TOP: i32 = 4 * 256;
pub const PADDLE_Y: i32 = 10 * 256;
pub const PADDLE_REACH: i32 = 320;
pub const BELL_X: [i32; 4] = [1920, 2688, 3456, 4224];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ball {
    pub x: i32,
    pub y: i32,
    pub dx: i32,
    pub dy: i32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Volley {
    pub owner: u32,
    pub paddle: i32,
    pub ball: Option<Ball>,
    pub bells: u8,
    pub lives: u8,
}
impl Volley {
    pub fn new(owner: u32) -> Self {
        Self {
            owner,
            paddle: 12 * 256,
            ball: None,
            bells: 0,
            lives: 3,
        }
    }
    pub fn valid(&self, round: &Round) -> bool {
        (1..=MAX_PLAYERS).contains(&self.owner)
            && (COURT_LEFT..=COURT_RIGHT).contains(&self.paddle)
            && self.bells & !15 == 0
            && self.bells.count_ones() == u32::from(round.progress)
            && (1..=3).contains(&self.lives)
            && round.mistakes == 3 - self.lives
            && round.game == Game::Volley
            && self.ball.as_ref().is_none_or(|b| {
                (COURT_LEFT..=COURT_RIGHT).contains(&b.x)
                    && (COURT_TOP..=PADDLE_Y).contains(&b.y)
                    && (8..=48).contains(&b.dx.unsigned_abs())
                    && b.dy.unsigned_abs() == 48
            })
    }
    /// None: life lost; Some(new bells): live tick (zero may mean waiting).
    pub fn step(&mut self, input: Input, tapped: bool) -> Option<u8> {
        let axis = if input.aim_x != 0 {
            input.aim_x
        } else {
            input.move_x
        };
        self.paddle = (self.paddle + i32::from(axis.signum()) * 40).clamp(COURT_LEFT, COURT_RIGHT);
        let Some(mut ball) = self.ball.take() else {
            if tapped {
                self.ball = Some(Ball {
                    x: self.paddle,
                    y: PADDLE_Y - 1,
                    dx: 24,
                    dy: -48,
                });
            }
            return Some(0);
        };
        ball.x += ball.dx;
        ball.y += ball.dy;
        if ball.x < COURT_LEFT {
            ball.x = 2 * COURT_LEFT - ball.x;
            ball.dx = ball.dx.abs();
        } else if ball.x > COURT_RIGHT {
            ball.x = 2 * COURT_RIGHT - ball.x;
            ball.dx = -ball.dx.abs();
        }
        let mut added = 0;
        if ball.y <= COURT_TOP && ball.dy < 0 {
            ball.y = 2 * COURT_TOP - ball.y;
            ball.dy = 48;
            let i = ((ball.x - COURT_LEFT) / (3 * 256)).min(3);
            let bit = 1 << i;
            if self.bells & bit == 0 {
                added = 1;
                self.bells |= bit;
            }
        }
        if ball.y >= PADDLE_Y && ball.dy > 0 {
            if (ball.x - self.paddle).abs() > PADDLE_REACH {
                self.lives -= 1;
                return None;
            }
            ball.y = 2 * PADDLE_Y - ball.y;
            ball.dy = -48;
            let offset = (ball.x - self.paddle).clamp(-288, 288);
            let dx = offset / 6;
            ball.dx = if dx.abs() < 8 {
                if ball.dx < 0 { -8 } else { 8 }
            } else {
                dx
            };
        }
        self.ball = Some(ball);
        Some(added)
    }
}
