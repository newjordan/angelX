//! Transitions: the window animates the whole cockpit, cell by cell.
//!
//! The assembly transition is fantasy engineering, not machinery: the walls
//! (box-drawing borders) slide in from the top and bottom like a stone door,
//! notch by notch as a cuckoo clock's escapement lets them, and lock with a
//! small overshoot. Then every cell's stone tile drops from above, rippling
//! out from the centre, lands with a thunk and a puff of dust, and its
//! content rides on it. Last, the stone tiles fade and leave the cockpit.
//!
//! Everything here is a pure function of time and the cell's place; every
//! number is fixed (seconds, pixels, radians).

use std::f32::consts::PI;
use std::time::Instant;

/// When the walls start moving and how long they take to close.
const WALL_START: f32 = 0.0;
const WALL_TIME: f32 = 0.62;
/// Escapement notches the walls move in; each moves in the first part of
/// its notch and holds for the rest.
const WALL_NOTCHES: f32 = 5.0;
const NOTCH_MOVE: f32 = 0.55;
/// The lock: a small overshoot that settles.
const LOCK_TIME: f32 = 0.14;
const LOCK_PX: f32 = 3.0;

/// The floor: the first tile falls at FLOOR_START, the farthest at
/// FLOOR_START + FLOOR_SPREAD, each a little early or late by up to
/// FLOOR_JITTER, and each falls for FALL_TIME.
const FLOOR_START: f32 = 0.34;
const FLOOR_SPREAD: f32 = 0.70;
const FLOOR_JITTER: f32 = 0.07;
const FALL_TIME: f32 = 0.34;
/// The part of a fall spent in the air; the rest is the thunk.
const AIR: f32 = 0.84;
/// In the air a tile is this much bigger, higher, tilted and drifted.
const LIFT_SCALE: f32 = 0.9;
const LIFT_PX: f32 = 48.0;
const TILT: f32 = 0.22;
const DRIFT_PX: f32 = 16.0;
/// How far the thunk squashes a tile.
const THUNK: f32 = 0.06;

/// The stone tiles fade once everything has landed.
const TILE_FADE_START: f32 = 1.45;
const TILE_FADE_TIME: f32 = 0.35;

/// The whole transition, in seconds.
pub const DURATION: f32 = 1.8;

/// How one cell looks at one moment of a transition. `REST` is no change.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    pub dx: f32,
    pub dy: f32,
    pub scale: f32,
    pub angle: f32,
    /// Opacity of the cell's own content.
    pub alpha: f32,
    /// Opacity of the stone tile under the cell (0: no tile).
    pub tile: f32,
    /// How high the tile still is (1: just appeared, 0: landed).
    pub lift: f32,
    /// Dust from the landing, fading from 1.
    pub dust: f32,
}

impl Motion {
    pub const REST: Self = Self {
        dx: 0.0,
        dy: 0.0,
        scale: 1.0,
        angle: 0.0,
        alpha: 1.0,
        tile: 0.0,
        lift: 0.0,
        dust: 0.0,
    };
}

/// Where a cell sits, for its motion.
#[derive(Clone, Copy, Debug)]
pub struct Place {
    pub col: u16,
    pub row: u16,
    pub cols: u16,
    pub rows: u16,
    pub cell_height: f32,
    /// A wall: a box-drawing border cell.
    pub wall: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Transition {
    started: Instant,
}

impl Transition {
    pub fn start(now: Instant) -> Self {
        Self { started: now }
    }

    /// A transition `seconds` in, for filming fixed moments. The value
    /// comes from the environment, so it is bounded to a sane span first:
    /// `Duration::from_secs_f32` panics on overflow and on NaN. The
    /// `max`/`min` chain (not `clamp`, which passes NaN through) resolves
    /// every non-finite input to a bound.
    #[allow(clippy::manual_clamp)]
    pub fn at(now: Instant, seconds: f32) -> Self {
        Self {
            started: now - std::time::Duration::from_secs_f32(seconds.max(0.0).min(3600.0)),
        }
    }

    /// Seconds since the start.
    pub fn time(&self, now: Instant) -> f32 {
        now.saturating_duration_since(self.started).as_secs_f32()
    }

    pub fn finished(&self, now: Instant) -> bool {
        self.time(now) >= DURATION
    }
}

/// A cell's motion at time `t`.
pub fn motion(t: f32, place: Place) -> Motion {
    if t >= DURATION {
        return Motion::REST;
    }
    let tile = tile_fade(t);
    if place.wall {
        let mut m = wall(t, place);
        m.tile = tile;
        return m;
    }
    floor(t, place, tile)
}

fn tile_fade(t: f32) -> f32 {
    1.0 - ((t - TILE_FADE_START) / TILE_FADE_TIME).clamp(0.0, 1.0)
}

/// Walls: the top half comes down from above the window, the bottom half up
/// from below, notch by notch, and they lock.
fn wall(t: f32, place: Place) -> Motion {
    let p = ((t - WALL_START) / WALL_TIME).clamp(0.0, 1.0);
    let notches = p * WALL_NOTCHES;
    let notch = notches.floor();
    let step = ease_out_cubic(((notches - notch) / NOTCH_MOVE).min(1.0));
    let travelled = if p >= 1.0 {
        1.0
    } else {
        ((notch + step) / WALL_NOTCHES).min(1.0)
    };
    let top_half = place.row < place.rows / 2;
    let distance = if top_half {
        -(f32::from(place.row) + 2.0) * place.cell_height
    } else {
        (f32::from(place.rows - place.row) + 1.0) * place.cell_height
    };
    let mut dy = distance * (1.0 - travelled);
    // The lock: past its stop by a few pixels, then back.
    let k = (t - WALL_START - WALL_TIME) / LOCK_TIME;
    if (0.0..1.0).contains(&k) {
        let direction = if top_half { 1.0 } else { -1.0 };
        dy += direction * LOCK_PX * (PI * k).sin() * (1.0 - k);
    }
    Motion { dy, ..Motion::REST }
}

/// Floor tiles: drop from above with a tilt and a drift, land with a thunk.
fn floor(t: f32, place: Place, tile_fade: f32) -> Motion {
    let start = FLOOR_START
        + FLOOR_SPREAD * distance_from_centre(place)
        + FLOOR_JITTER * hash(place.col, place.row, 1);
    let p = ((t - start) / FALL_TIME).clamp(0.0, 1.0);
    if p <= 0.0 {
        // Not yet falling: nothing there.
        return Motion {
            alpha: 0.0,
            tile: 0.0,
            ..Motion::REST
        };
    }
    let fall = (p / AIR).min(1.0);
    // Height above the ground: a fall that speeds up.
    let height = 1.0 - fall * fall;
    let impact = ((p - AIR) / (1.0 - AIR)).clamp(0.0, 1.0);
    let thunk = if impact > 0.0 && impact < 1.0 {
        THUNK * (PI * impact).sin()
    } else {
        0.0
    };
    let tilt = (hash(place.col, place.row, 2) - 0.5) * 2.0 * TILT;
    let drift = (hash(place.col, place.row, 3) - 0.5) * 2.0 * DRIFT_PX;
    Motion {
        dx: drift * height,
        dy: -LIFT_PX * height,
        scale: 1.0 + LIFT_SCALE * height - thunk,
        angle: tilt * height,
        alpha: (fall * 2.5).min(1.0),
        tile: (fall * 3.0).min(1.0) * tile_fade,
        lift: height,
        dust: if impact > 0.0 && impact < 1.0 {
            1.0 - impact
        } else {
            0.0
        },
    }
}

/// 0 at the centre of the grid, 1 at its farthest corner (cells are about
/// twice as tall as wide, so rows count double).
fn distance_from_centre(place: Place) -> f32 {
    let cx = f32::from(place.cols) / 2.0;
    let cy = f32::from(place.rows) / 2.0;
    let dx = f32::from(place.col) + 0.5 - cx;
    let dy = (f32::from(place.row) + 0.5 - cy) * 2.0;
    let far = (cx * cx + (cy * 2.0) * (cy * 2.0)).sqrt().max(1.0);
    ((dx * dx + dy * dy).sqrt() / far).min(1.0)
}

fn ease_out_cubic(x: f32) -> f32 {
    1.0 - (1.0 - x).powi(3)
}

/// A steady pseudo-random number in [0, 1) for a cell.
pub fn hash(col: u16, row: u16, salt: u32) -> f32 {
    let mut h = u32::from(col).wrapping_mul(0x9E37_79B1)
        ^ u32::from(row).wrapping_mul(0x85EB_CA77)
        ^ salt.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0x00ff_ffff) as f32 / 16_777_216.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(col: u16, row: u16, wall: bool) -> Place {
        Place {
            col,
            row,
            cols: 100,
            rows: 40,
            cell_height: 25.0,
            wall,
        }
    }

    #[test]
    fn everything_is_at_rest_when_it_ends() {
        for (col, row, wall) in [
            (0, 0, true),
            (50, 20, false),
            (99, 39, false),
            (3, 30, true),
        ] {
            assert_eq!(motion(DURATION, place(col, row, wall)), Motion::REST);
        }
    }

    #[test]
    fn walls_start_outside_the_window() {
        let top = motion(0.0, place(10, 2, true));
        assert_eq!(top.dy, -4.0 * 25.0);
        let bottom = motion(0.0, place(10, 39, true));
        assert_eq!(bottom.dy, 2.0 * 25.0);
    }

    #[test]
    fn walls_hold_between_notches() {
        // Late in a notch the wall waits: two moments there are the same.
        let notch = WALL_TIME / WALL_NOTCHES;
        let a = motion(notch * 0.8, place(10, 2, true));
        let b = motion(notch * 0.95, place(10, 2, true));
        assert_eq!(a.dy, b.dy);
        assert!(a.dy < 0.0);
    }

    #[test]
    fn centre_tiles_fall_before_corner_tiles() {
        let t = FLOOR_START + 0.12;
        let centre = motion(t, place(50, 20, false));
        let corner = motion(t, place(0, 0, false));
        assert!(centre.alpha > 0.0);
        assert_eq!(corner.alpha, 0.0);
    }

    #[test]
    fn a_falling_tile_is_bigger_and_higher_then_lands() {
        let p = place(50, 20, false);
        let start = FLOOR_START
            + FLOOR_SPREAD * distance_from_centre(p)
            + FLOOR_JITTER * hash(p.col, p.row, 1);
        let air = motion(start + FALL_TIME * 0.2, p);
        assert!(air.scale > 1.1 && air.dy < -10.0 && air.lift > 0.5);
        let thunk = motion(start + FALL_TIME * 0.9, p);
        assert!(thunk.scale < 1.0 && thunk.dust > 0.0);
        let landed = motion(start + FALL_TIME + 0.01, p);
        assert_eq!((landed.scale, landed.dy, landed.alpha), (1.0, 0.0, 1.0));
    }

    #[test]
    fn filmed_moments_accept_any_float() {
        let now = Instant::now();
        for seconds in [f32::NAN, f32::INFINITY, -5.0, 1.0e30] {
            let t = Transition::at(now, seconds).time(now);
            assert!(t.is_finite() && (0.0..=3600.0).contains(&t), "{seconds} -> {t}");
        }
    }

    #[test]
    fn hash_is_steady_and_in_range() {
        for col in 0..50 {
            let h = hash(col, 7, 2);
            assert!((0.0..1.0).contains(&h));
            assert_eq!(h, hash(col, 7, 2));
        }
    }
}
