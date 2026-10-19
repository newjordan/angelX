//! A knight on foot: eight directions on the fixed-point grid, the masks'
//! stride, the sidestep, and a crate pushed a tile at a time.
use super::state::*;
use super::town::{self, Ground, Q, TILE, Tile};
use crate::rules::Input;

/// The pace of a villager in Q-pixels a tick (`folk::PACE_Q`); a knight's
/// is this times the `moonfall_walk` number, in percent.
pub use super::folk::PACE_Q;

/// Ticks a stout knight leans on a crate before it moves a tile.
pub const PUSH_TICKS: u32 = 10;
/// A sidestep: ticks, and the rest before the next.
pub const ROLL: u32 = 6;
pub const ROLL_REST: u32 = 40;

/// Q-pixels a tick a knight walks at.
pub fn speed(walk_percent: u32, mask: Mask) -> i32 {
    let base = i64::from(PACE_Q) * i64::from(walk_percent) / 100;
    (match mask {
        Mask::Bare => base,
        Mask::Swift => base * 16 / 10,
        Mask::Stout => base * 7 / 10,
    }) as i32
}

/// The key bits a tick's input is read as.
pub mod key {
    pub const USE: u16 = 1;
    pub const LEDGER: u16 = 2;
    pub const BELL: u16 = 4;
    pub const MASK: u16 = 8;
    pub const DROP: u16 = 16;
    pub const SIDESTEP: u16 = 32;
    pub const UP: u16 = 64;
    pub const DOWN: u16 = 128;
    pub const LEFT: u16 = 256;
    pub const RIGHT: u16 = 512;
}

pub fn keys_of(i: Input) -> u16 {
    let mut k = 0;
    if i.interact || i.take {
        k |= key::USE;
    }
    if i.swing {
        k |= key::LEDGER;
    }
    if i.ult {
        k |= key::BELL;
    }
    if i.play > 0 {
        k |= key::MASK;
    }
    if i.bomb {
        k |= key::DROP;
    }
    if i.dash {
        k |= key::SIDESTEP;
    }
    if i.move_y < 0 || i.aim_y < 0 {
        k |= key::UP;
    }
    if i.move_y > 0 || i.aim_y > 0 {
        k |= key::DOWN;
    }
    if i.move_x < 0 || i.aim_x < 0 {
        k |= key::LEFT;
    }
    if i.move_x > 0 || i.aim_x > 0 {
        k |= key::RIGHT;
    }
    k
}

/// Whether `bit` went down this tick.
pub fn pressed(now: u16, before: u16, bit: u16) -> bool {
    now & bit != 0 && before & bit == 0
}

fn crate_at(crates: &[(i32, i32)], x: i32, y: i32) -> Option<usize> {
    for (cx, cy) in [(-4, -3), (3, -3), (-4, 2), (3, 2)] {
        let t = (
            (x.div_euclid(Q) + cx).div_euclid(TILE),
            (y.div_euclid(Q) + cy).div_euclid(TILE),
        );
        if let Some(i) = crates.iter().position(|&c| c == t) {
            return Some(i);
        }
    }
    None
}

/// What a step of walking did to the world.
#[derive(Default)]
pub struct Walked {
    /// A crate that went into the ditch.
    pub filled: bool,
    /// A crate moved a tile.
    pub moved: bool,
    /// Leant on a crate too heavy to move.
    pub heavy: bool,
}

/// One tick of walking for a knight who is free to move.
pub fn step(
    k: &mut Knight,
    input: Input,
    pace: i32,
    ground: Ground,
    crates: &mut Vec<(i32, i32)>,
    rolled: bool,
) -> Walked {
    let mut out = Walked::default();
    let (mut dx, mut dy) = (i32::from(input.move_x), i32::from(input.move_y));
    if k.roll > 0 {
        dx = i32::from(k.roll_dir.0);
        dy = i32::from(k.roll_dir.1);
    } else if rolled && k.roll_rest == 0 && k.up() {
        let dir = if (dx, dy) == (0, 0) { facing(k.face) } else { (dx, dy) };
        k.roll = ROLL;
        k.roll_rest = ROLL_REST;
        k.roll_dir = (dir.0 as i8, dir.1 as i8);
        dx = dir.0;
        dy = dir.1;
    }
    k.roll_rest = k.roll_rest.saturating_sub(1);
    let rolling = k.roll > 0;
    k.roll = k.roll.saturating_sub(1);
    if (dx, dy) == (0, 0) {
        k.stride = 0;
        k.push = 0;
        return out;
    }
    k.stride = (k.stride + 1).min(60_000);
    let speed = if rolling { pace * 22 / 10 } else { pace };
    // A diagonal is not faster: 181/256 of the pace on each axis.
    let each = if dx != 0 && dy != 0 { speed * 181 / 256 } else { speed };
    let mut leaning = false;
    for (ax, ay) in [(dx, 0), (0, dy)] {
        if (ax, ay) == (0, 0) {
            continue;
        }
        let (nx, ny) = (k.x + ax * each, k.y + ay * each);
        if !town::clear(nx, ny, ground, &[]) {
            continue;
        }
        if let Some(i) = crate_at(crates, nx, ny) {
            if k.mask == Mask::Stout {
                leaning = true;
                k.push += 1;
                if k.push >= PUSH_TICKS {
                    k.push = 0;
                    let from = crates[i];
                    let to = (from.0 + ax.signum(), from.1 + ay.signum());
                    let ditch = town::tile(to.0, to.1) == Tile::Ditch && !ground.ditch_filled;
                    let open = ground.open(town::tile(to.0, to.1)) && !crates.contains(&to);
                    if ditch {
                        crates.remove(i);
                        out.filled = true;
                    } else if open {
                        crates[i] = to;
                        out.moved = true;
                    }
                }
            } else {
                out.heavy = true;
            }
            continue;
        }
        k.x = nx;
        k.y = ny;
    }
    if !leaning {
        k.push = 0;
    }
    k.face = if dx != 0 && (dy == 0 || k.face < 2) {
        if dx < 0 { 2 } else { 3 }
    } else if dy != 0 {
        if dy < 0 { 1 } else { 0 }
    } else {
        k.face
    };
    out
}

/// A facing as a step.
pub fn facing(face: u8) -> (i32, i32) {
    match face {
        0 => (0, 1),
        1 => (0, -1),
        2 => (-1, 0),
        _ => (1, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(tx: i32, ty: i32) -> Knight {
        Knight::at((tx, ty))
    }

    fn go(k: &mut Knight, dx: i8, dy: i8, ticks: u32, mask: Mask, crates: &mut Vec<(i32, i32)>, ground: Ground) {
        k.mask = mask;
        for _ in 0..ticks {
            let input = Input { move_x: dx, move_y: dy, ..Default::default() };
            step(k, input, speed(150, mask), ground, crates, false);
        }
    }

    #[test]
    fn the_masks_set_the_stride() {
        let bare = speed(150, Mask::Bare);
        assert_eq!(bare, 576);
        assert!(speed(150, Mask::Swift) > bare * 3 / 2 && speed(150, Mask::Stout) < bare * 3 / 4);
    }

    #[test]
    fn walls_and_water_stop_a_knight_and_open_ground_does_not() {
        let mut k = at(17, 15);
        let mut crates = vec![];
        go(&mut k, 0, -1, 60, Mask::Bare, &mut crates, Ground::default());
        // The Moonwell is in the way north of the plaza's middle.
        assert!(k.y > 14 * TILE * Q, "the well holds {}", k.y / Q);
        let mut k = at(15, 15);
        go(&mut k, 1, 0, 20, Mask::Bare, &mut crates, Ground::default());
        assert!(k.x > 15 * TILE * Q + 40 * Q);
    }

    #[test]
    fn only_a_stout_knight_moves_the_crate_and_the_ditch_swallows_it() {
        let ground = Ground::default();
        let mut crates = vec![town::CRATE_START];
        // A bare knight leans on it and it stays.
        let mut k = at(17, 19);
        go(&mut k, 0, 1, 80, Mask::Bare, &mut crates, ground);
        assert_eq!(crates, vec![town::CRATE_START]);
        // A stout one pushes it south a tile at a time, then into the ditch.
        let mut k = at(17, 19);
        let mut filled = false;
        for _ in 0..400 {
            k.mask = Mask::Stout;
            let input = Input { move_y: 1, ..Default::default() };
            let w = step(&mut k, input, speed(150, Mask::Stout), Ground { ditch_filled: filled, ..ground }, &mut crates, false);
            filled |= w.filled;
        }
        assert!(filled && crates.is_empty());
        assert!(k.y / Q >= 23 * TILE, "walked onto the filled ditch");
    }

    #[test]
    fn a_sidestep_is_quick_and_has_to_rest() {
        let mut k = at(16, 15);
        let mut crates = vec![];
        let ground = Ground::default();
        let go = |k: &mut Knight, crates: &mut Vec<(i32, i32)>, rolled: bool| {
            step(k, Input { move_x: 1, ..Default::default() }, speed(150, Mask::Bare), ground, crates, rolled);
        };
        let x0 = k.x;
        go(&mut k, &mut crates, true);
        for _ in 0..5 {
            go(&mut k, &mut crates, false);
        }
        let dash = k.x - x0;
        assert!(dash > 6 * speed(150, Mask::Bare) * 3 / 2, "{dash}");
        let rest = k.roll_rest;
        assert!(rest > 0);
        go(&mut k, &mut crates, true);
        assert_eq!(k.roll, 0, "still resting");
    }
}
