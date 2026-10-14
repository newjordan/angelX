//! The monsters' pattern shots where a plain sprite will not do: a beam's
//! warning line and its burn, the marks where rain and crossfire will come
//! from, and the blink of a mine about to pop (`patterns`).

use super::*;
use crate::rules::patterns::{BEAM_WARN, warning_reach};
use crate::rules::{Projectile, Shot};

/// A beam's inks by its shot: warning, rim, core.
fn beam_inks(kind: Shot) -> (char, char, char) {
    match kind {
        Shot::Eye => ('1', '2', 'w'),
        Shot::Wisp => ('2', '3', 'w'),
        Shot::Spore => ('C', 'Y', 'w'),
        Shot::Page => ('$', 'c', 'w'),
        _ => ('7', '@', '6'),
    }
}

/// Draw what a plain sprite cannot. True when the shot is drawn whole
/// (a beam, or a shot still waiting) and wants no sprite.
pub(super) fn draw(cv: &mut Img, run: &Run, shot: &Projectile) -> bool {
    let path = &shot.traits.path;
    if let Some(beam) = path.beam {
        let (warn, rim, core) = beam_inks(shot.kind);
        let (x, y) = at(shot.x, shot.y);
        let length = (beam.reach * UNIT) as i32;
        if beam.burning(path.age) {
            line(cv, (x, y), beam.angle, length, |cv, px, py, (nx, ny)| {
                for k in [-2, -1, 1, 2] {
                    cv.put(px + nx * k, py + ny * k, rim);
                }
                cv.put(px, py, core);
            });
        } else {
            // Marching dashes, quickening as the burn nears; a sweep shows
            // where it will end, fainter.
            let close = path.age + 7 > u16::from(BEAM_WARN);
            let march = (run.tick / 2) as i32;
            line(cv, (x, y), beam.angle, length, |cv, px, py, _| {
                let i = (px - x).abs().max((py - y).abs());
                if close || (i + march) % 4 < 2 {
                    cv.put(px, py, warn);
                }
            });
            if beam.sweep != 0.0 {
                // Dotted arcs fill the warned sector, bounded by stone.
                // Both edges are readable, even where their lengths differ.
                for i in 1..=32 {
                    let a = beam.angle + (beam.end_angle() - beam.angle) * i as f32 / 32.0;
                    let length = (warning_reach(run, (shot.x, shot.y), a) * UNIT) as i32;
                    line(cv, (x, y), a, length, |cv, px, py, _| {
                        let d = ((px - x) as f32).hypot((py - y) as f32) as i32;
                        if (i == 32 && (d + march) % 4 < 2) || (d + march) % 24 == 0 {
                            cv.put(px, py, warn);
                        }
                    });
                }
            }
        }
        return true;
    }
    if path.waiting() {
        if path.mark {
            // Where it will come from: a blinking diamond, solid at the
            // last, and a short arrow the way it will fly.
            let (x, y) = at(shot.x, shot.y);
            let lit = path.wait < 8 || (run.tick / 3).is_multiple_of(2);
            let (rim, core) = if lit { ('@', '6') } else { ('7', '8') };
            for dy in -5i32..=5 {
                for dx in -5i32..=5 {
                    let d = dx.abs() + dy.abs();
                    if d == 5 || d == 4 {
                        cv.put(x + dx, y + dy, rim);
                    } else if d <= 1 {
                        cv.put(x + dx, y + dy, core);
                    }
                }
            }
            let speed = shot.vx.hypot(shot.vy).max(0.001);
            let (ux, uy) = (shot.vx / speed, shot.vy / speed);
            for k in 7..13 {
                for side in [-1.0f32, 0.0, 1.0] {
                    if side != 0.0 && k > 8 {
                        continue;
                    }
                    cv.put(
                        x + (ux * k as f32 - uy * side * (13 - k) as f32 * 0.4).round() as i32,
                        y + (uy * k as f32 + ux * side * (13 - k) as f32 * 0.4).round() as i32,
                        rim,
                    );
                }
            }
        }
        return true;
    }
    if path.bursts() {
        // A shot that will burst: a ring round it, blinking fast once it
        // hangs still.
        let (x, y) = at(shot.x, shot.y);
        let still = shot.vx.hypot(shot.vy) < 0.05;
        let lit = !still || (run.tick / 2).is_multiple_of(2);
        if lit {
            let r = if still { 7.0 } else { 6.0 };
            for i in 0..16 {
                let a = i as f32 / 16.0 * std::f32::consts::TAU;
                cv.put(
                    x + (a.cos() * r).round() as i32,
                    y + (a.sin() * r).round() as i32,
                    if still { '7' } else { '8' },
                );
            }
        }
    }
    false
}

/// Walk a line from `from` at `angle` for `length` pixels, handing each
/// pixel and the line's sideways step to `put`.
fn line(
    cv: &mut Img,
    from: (i32, i32),
    angle: f32,
    length: i32,
    mut put: impl FnMut(&mut Img, i32, i32, (i32, i32)),
) {
    let (ux, uy) = (angle.cos(), angle.sin());
    // Sideways: whichever axis the line crosses most.
    let side = if ux.abs() > uy.abs() { (0, 1) } else { (1, 0) };
    for i in 0..length.max(0) {
        let px = from.0 + (ux * i as f32).round() as i32;
        let py = from.1 + (uy * i as f32).round() as i32;
        put(cv, px, py, side);
    }
}
