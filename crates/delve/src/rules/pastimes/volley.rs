//! Belfry Bell Rally: a shield, a turnip, and four bells in sequence.
//!
//! This is a timing game, not passive Breakout. A fresh F press swings; an
//! early swing still saves the turnip, but a well-timed one makes it fly
//! faster. Shield contact and the aim keys shape the return. Every bell adds
//! pace, a miss costs a turnip but keeps the bells already won, and the next
//! serve arrives automatically after a short breath. Fixed-point motion and
//! input edges remain simulation-owned, deterministic and checkpointable.
use super::{Game, Round};
use crate::rules::{Input, MAX_PLAYERS};
use serde::{Deserialize, Serialize};

pub const COURT_LEFT: i32 = 6 * 256;
pub const COURT_RIGHT: i32 = 18 * 256;
pub const COURT_TOP: i32 = 4 * 256;
pub const PADDLE_Y: i32 = 10 * 256;
pub const PADDLE_REACH: i32 = 384;
pub const BELL_X: [i32; 4] = [1920, 2688, 3456, 4224];
/// Ticks a bell's ring animation lasts.
pub const RING_TICKS: u8 = 12;
/// How many ball positions the trail remembers.
pub const TRAIL_LEN: usize = 10;
/// Ticks a miss-flash screen effect lasts.
pub const MISS_FLASH_TICKS: u8 = 8;
/// Ticks a single target-bell shimmer cycle lasts.
pub const SHIMMER_TICKS: u8 = 8;
/// A swing lives this long. The first three ticks are the perfect window.
pub const SWING_TICKS: u8 = 8;
pub const PERFECT_TICKS: u8 = 3;
pub const BAT_FLASH_TICKS: u8 = 14;
pub const PERFECT_FLASH_TICKS: u8 = 14;
/// A short breath after a miss, not a trip back to a menu.
pub const SERVE_WAIT: u8 = 18;
pub const SERVE_SPEED: i32 = 64;
pub const BASE_SPEED: i32 = 72;
pub const MAX_SPEED: i32 = 104;

/// What happened in one tick. Presentation and score consume this; the
/// physics does not reach back into the run or allocate events.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VolleyBeat {
    pub served: bool,
    pub returned: bool,
    pub perfect: bool,
    /// A bell physically struck, including an already-won or out-of-order one.
    pub bell: Option<usize>,
    /// This strike won the next bell in the sequence.
    pub scored: bool,
    pub missed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ball {
    pub x: i32,
    pub y: i32,
    pub dx: i32,
    pub dy: i32,
    /// A neutral feed must be returned before it can win a bell.
    #[serde(default)]
    pub served: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Volley {
    pub owner: u32,
    pub paddle: i32,
    pub ball: Option<Ball>,
    pub bells: u8,
    pub lives: u8,
    /// Ticks remaining on each bell's ring animation (0 = silent).
    #[serde(default)]
    pub bell_ring: [u8; 4],
    /// Last N ball positions for the motion trail.
    #[serde(default)]
    pub ball_trail: Vec<(i32, i32)>,
    /// Ticks remaining on the paddle-hit flash.
    #[serde(default)]
    pub paddle_flash: u8,
    /// Ticks remaining on the life-lost screen flash.
    #[serde(default)]
    pub miss_flash: u8,
    /// Ball x position where the miss occurred (for splash placement).
    #[serde(default)]
    pub miss_x: i32,
    /// Shimmer timer for the next bell. Other bells stay visually quiet.
    #[serde(default)]
    pub bell_shimmer: [u8; 4],
    /// Fresh-press swing window; held F never refreshes it.
    #[serde(default)]
    pub swing: u8,
    /// The bright, short afterglow of a perfectly timed return.
    #[serde(default)]
    pub perfect_flash: u8,
    /// Current and best return streak. A miss resets only the current streak.
    #[serde(default)]
    pub combo: u8,
    #[serde(default)]
    pub best_combo: u8,
    /// Recovery before the next serve; bells and best streak are retained.
    #[serde(default)]
    pub serve_wait: u8,
}

impl Volley {
    pub fn new(owner: u32) -> Self {
        Self {
            owner,
            paddle: 12 * 256,
            ball: None,
            bells: 0,
            lives: 3,
            bell_ring: [0; 4],
            ball_trail: Vec::new(),
            paddle_flash: 0,
            miss_flash: 0,
            miss_x: 12 * 256,
            bell_shimmer: [0; 4],
            swing: 0,
            perfect_flash: 0,
            combo: 0,
            best_combo: 0,
            serve_wait: 0,
        }
    }

    /// The next bell is always explicit: ordered progress gives a rally a
    /// beginning, escalation and a final chime instead of four arbitrary hits.
    pub fn next_bell(&self) -> Option<usize> {
        (0..4).find(|&i| self.bells & (1 << i) == 0)
    }

    pub(crate) fn speed(bells: u8, combo: u8) -> i32 {
        (BASE_SPEED + 4 * bells.count_ones() as i32 + 2 * i32::from(combo.min(8))).min(MAX_SPEED)
    }

    /// Contact position supplies the broad angle; aim adds a little english.
    /// This is public within the crate so bots and tests use the real rule,
    /// rather than a second, subtly different model of the paddle.
    pub(crate) fn return_dx(ball_x: i32, paddle: i32, aim: i8, old_dx: i32) -> i32 {
        let offset = ball_x - paddle;
        let raw = offset / 3 + 24 * i32::from(aim.signum());
        if raw == 0 {
            16 * if old_dx < 0 { -1 } else { 1 }
        } else {
            let dx = raw.clamp(-128, 128);
            dx.signum() * dx.abs().max(8)
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
            && round.streak == self.combo
            && self.bell_ring.iter().all(|&r| r <= RING_TICKS)
            && self.ball_trail.len() <= TRAIL_LEN
            && self.paddle_flash <= BAT_FLASH_TICKS
            && self.perfect_flash <= PERFECT_FLASH_TICKS
            && (self.perfect_flash == 0 || self.paddle_flash > 0)
            && self.miss_flash <= MISS_FLASH_TICKS
            && (COURT_LEFT..=COURT_RIGHT).contains(&self.miss_x)
            && self.bell_shimmer.iter().all(|&s| s <= SHIMMER_TICKS)
            && self.swing <= SWING_TICKS
            && self.combo <= 20
            && self.best_combo <= 20
            && self.best_combo >= self.combo
            && self.serve_wait <= SERVE_WAIT
            && (self.ball.is_some() || self.swing == 0 || self.serve_wait > 0)
            && (self.ball.is_none() || self.serve_wait == 0)
            && self.ball.as_ref().is_none_or(|b| {
                (COURT_LEFT..=COURT_RIGHT).contains(&b.x)
                    && (COURT_TOP..=PADDLE_Y).contains(&b.y)
                    && (8..=128).contains(&b.dx.unsigned_abs())
                    && (48..=(MAX_SPEED as u32 + 16)).contains(&b.dy.unsigned_abs())
            })
    }

    pub fn step(&mut self, input: Input, tapped: bool) -> VolleyBeat {
        let mut beat = VolleyBeat::default();
        for ring in &mut self.bell_ring {
            *ring = ring.saturating_sub(1);
        }
        self.paddle_flash = self.paddle_flash.saturating_sub(1);
        self.perfect_flash = self.perfect_flash.saturating_sub(1);
        self.miss_flash = self.miss_flash.saturating_sub(1);
        self.swing = self.swing.saturating_sub(1);
        let next = self.next_bell();
        for (i, shimmer) in self.bell_shimmer.iter_mut().enumerate() {
            if next == Some(i) {
                *shimmer = if *shimmer == 0 {
                    SHIMMER_TICKS
                } else {
                    *shimmer - 1
                };
            } else {
                *shimmer = 0;
            }
        }

        // WASD/the left stick moves; arrows/the right stick only aim. This
        // separation is what lets a knight hold a catch position and shape a
        // return instead of being dragged off it by the aim keys.
        self.paddle =
            (self.paddle + i32::from(input.move_x.signum()) * 40).clamp(COURT_LEFT, COURT_RIGHT);

        let Some(mut ball) = self.ball.take() else {
            self.serve_wait = self.serve_wait.saturating_sub(1);
            if self.serve_wait == 0 && self.lives > 0 {
                let target = self.next_bell().map_or(self.paddle, |i| BELL_X[i]);
                let mut dx = ((target - self.paddle) / 24).clamp(-96, 96);
                if dx.abs() < 8 {
                    dx = if dx < 0 || (dx == 0 && target < self.paddle) {
                        -8
                    } else {
                        8
                    };
                }
                self.swing = 0;
                self.ball = Some(Ball {
                    x: self.paddle,
                    y: PADDLE_Y - 1,
                    dx,
                    dy: -SERVE_SPEED,
                    served: true,
                });
                self.ball_trail.clear();
                self.miss_flash = 0;
                beat.served = true;
            }
            return beat;
        };

        if tapped && self.swing == 0 {
            self.swing = SWING_TICKS;
        }
        self.ball_trail.push((ball.x, ball.y));
        if self.ball_trail.len() > TRAIL_LEN {
            self.ball_trail.remove(0);
        }
        let old_x = ball.x;
        let old_y = ball.y;
        ball.x += ball.dx;
        ball.y += ball.dy;
        if ball.x < COURT_LEFT {
            ball.x = 2 * COURT_LEFT - ball.x;
            ball.dx = ball.dx.abs();
        } else if ball.x > COURT_RIGHT {
            ball.x = 2 * COURT_RIGHT - ball.x;
            ball.dx = -ball.dx.abs();
        }

        if ball.y <= COURT_TOP && ball.dy < 0 {
            // Judge the bell at the ceiling crossing, not beyond it.
            let at_top = old_x + (old_y - COURT_TOP) * ball.dx / -ball.dy;
            ball.y = 2 * COURT_TOP - ball.y;
            ball.dy = ball.dy.abs();
            let i = ((at_top - COURT_LEFT) / (3 * 256)).clamp(0, 3) as usize;
            self.bell_ring[i] = RING_TICKS;
            beat.bell = Some(i);
            if !ball.served && self.next_bell() == Some(i) {
                self.bells |= 1 << i;
                beat.scored = true;
            }
        }

        if ball.y >= PADDLE_Y && ball.dy > 0 {
            let at_paddle = old_x + (PADDLE_Y - old_y) * ball.dx / ball.dy;
            if self.swing == 0 || (at_paddle - self.paddle).abs() > PADDLE_REACH {
                self.lives = self.lives.saturating_sub(1);
                self.combo = 0;
                self.swing = 0;
                self.perfect_flash = 0;
                self.miss_flash = MISS_FLASH_TICKS;
                self.miss_x = ball.x.clamp(COURT_LEFT, COURT_RIGHT);
                self.serve_wait = SERVE_WAIT;
                self.ball_trail.clear();
                beat.missed = true;
                return beat;
            }
            let perfect = self.swing > SWING_TICKS - PERFECT_TICKS;
            if perfect {
                ball.x = at_paddle.clamp(COURT_LEFT, COURT_RIGHT);
            }
            ball.y = 2 * PADDLE_Y - ball.y;
            ball.served = false;
            self.combo = self.combo.saturating_add(1).min(20);
            self.best_combo = self.best_combo.max(self.combo);
            ball.dy = -(Self::speed(self.bells, self.combo) + if perfect { 16 } else { 0 });
            let english = if perfect { 2 } else { 1 };
            ball.dx = Self::return_dx(
                ball.x,
                self.paddle,
                input.aim_x.saturating_mul(english),
                ball.dx,
            );
            self.paddle_flash = BAT_FLASH_TICKS;
            self.perfect_flash = if perfect { PERFECT_FLASH_TICKS } else { 0 };
            self.swing = 0;
            beat.returned = true;
            beat.perfect = perfect;
        }
        self.ball = Some(ball);
        beat
    }
}
