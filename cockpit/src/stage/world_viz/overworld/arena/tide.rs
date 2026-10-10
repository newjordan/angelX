//! Ravage on the floor: ripples where every ring will burst, brightening as
//! each ring's moment nears, with one lane left still; then the tentacles,
//! ring after ring, up out of the floor and the flooded aisles alike.

use super::super::ink::Img;
use super::{UNIT, at};
use crate::drive::together_shooter::tide::UP;
use crate::drive::together_shooter::{Run, TILE_UNITS, Tile};
use std::f32::consts::TAU;

/// A tentacle up out of the floor, suckers on the inside of its curl.
const TENTACLE: [&str; 10] = [
    "....z.", "...zQ.", "..zQq.", "..Qq..", ".zQqv.", ".Qqv..", ".zQqv.", "..zQqv", ".zQQq.",
    "zQQqqq",
];

/// Ripples, and the tentacles of a ring that has burst.
pub(super) fn ravages(cv: &mut Img, run: &Run) {
    let room = run.room();
    let open = |ux: f32, uy: f32| {
        let tile = room.tile(
            (ux / TILE_UNITS).floor() as i32,
            (uy / TILE_UNITS).floor() as i32,
        );
        !matches!(tile, Tile::Wall | Tile::Block)
    };
    for ravage in &run.ravages {
        for k in 0..ravage.rings {
            let (radius, bursts) = ravage.ring(k);
            if ravage.age >= bursts + UP {
                continue;
            }
            if ravage.age < bursts {
                // A ring of shivering water, lighter as its moment nears;
                // the lane is where it lies still.
                let left = bursts - ravage.age;
                let ink = match left {
                    0..=6 => 'U',
                    7..=15 => 'u',
                    _ => 'S',
                };
                let n = (TAU * radius * UNIT / 3.0) as usize;
                for i in 0..n {
                    if (i as u32 + ravage.age / 3).is_multiple_of(4) {
                        continue;
                    }
                    let a = i as f32 / n as f32 * TAU;
                    let (ux, uy) = (ravage.x + a.cos() * radius, ravage.y + a.sin() * radius);
                    if ravage.in_lane(ux, uy) || !open(ux, uy) {
                        continue;
                    }
                    let (x, y) = at(ux, uy);
                    cv.put(x, y, ink);
                }
                continue;
            }
            // Burst: a tentacle every step and a half round the ring, each
            // ring turned a little from the last so they don't line up.
            let n = ((TAU * radius / 1.5).ceil() as usize).max(6);
            let t = ravage.age - bursts;
            // Up fast, a moment high, and back down.
            let shown = (t * 3 + 3).min(10).min((UP - t) * 3) as i32;
            for i in 0..n {
                let a = (i as f32 + 0.5 * f32::from(k % 2)) / n as f32 * TAU;
                let (ux, uy) = (ravage.x + a.cos() * radius, ravage.y + a.sin() * radius);
                if ravage.in_lane(ux, uy) || !open(ux, uy) {
                    continue;
                }
                let (x, y) = at(ux, uy);
                for row in 0..shown {
                    let src = TENTACLE[(10 - shown + row) as usize];
                    for (col, c) in src.chars().enumerate().filter(|&(_, c)| c != '.') {
                        cv.put(x - 3 + col as i32, y - shown + 1 + row, c);
                    }
                }
                if t < 4 {
                    for (dx, dy) in [(-4, -1), (4, -1), (-2, -3), (3, -4)] {
                        cv.put(x + dx, y + dy - t as i32, 'z');
                    }
                }
            }
        }
    }
}
