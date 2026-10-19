//! How the arms feel, drawn: a rail's fading tracer, shafts stuck in the
//! stone, Fire Arrows' embers, and a room's end paid out in coins flying
//! home. The sparks, muzzles, smoke and deaths are the fight's bursts,
//! drawn under the shots in flight (`vfx`).
//!
//! All of it reads `Run::feel` (`drive::together_shooter::feel`), so a
//! friend's mirror and a browser guest's picture show it as the host does.

use super::super::ink::{Img, hash};
use super::at;
use crate::rules::feel::TRACER_TICKS;
use crate::rules::{Run, Shot};

/// Under the figures: what stuck in the stone.
pub(super) fn under(cv: &mut Img, run: &Run) {
    for shaft in &run.feel.stuck {
        let (x, y) = at(shaft.x, shaft.y);
        // The head buried; the shaft and fletching stand out of the wall.
        let inks: &[char] = match shaft.kind {
            Shot::Bolt => &['V', 'R', 'R', 'R', 'b'],
            _ => &['J', 'o', 'o', 'o', 'T'],
        };
        for (k, &ink) in inks.iter().enumerate() {
            let k = k as f32;
            cv.put(
                x - (shaft.dx * k).round() as i32,
                y - (shaft.dy * k).round() as i32,
                ink,
            );
        }
    }
}

/// Over the figures: tracers, embers, coins. (Sparks, muzzles, smoke and
/// deaths are drawn under the shots in flight: `vfx`.)
pub(super) fn over(cv: &mut Img, run: &Run) {
    tracers(cv, run);
    embers(cv, run);
    coins(cv, run);
}

/// A rail's line: bright and whole when new, then thinning to dots.
fn tracers(cv: &mut Img, run: &Run) {
    for t in &run.feel.tracers {
        let (a, b) = (at(t.from.0, t.from.1), at(t.to.0, t.to.1));
        let steps = (b.0 - a.0).abs().max((b.1 - a.1).abs()).max(1);
        let fresh = t.ttl * 2 > TRACER_TICKS;
        let ink = if t.ttl + 2 >= TRACER_TICKS {
            'c'
        } else if fresh {
            'V'
        } else {
            'U'
        };
        for i in 0..=steps {
            // As it fades, the line breaks up.
            if !fresh && i % 2 == 1 {
                continue;
            }
            let (x, y) = (a.0 + (b.0 - a.0) * i / steps, a.1 + (b.1 - a.1) * i / steps);
            cv.put(x, y, ink);
            if t.thick {
                cv.put(x, y + 1, if fresh { 'U' } else { 'S' });
                cv.put(x + 1, y, if fresh { 'U' } else { 'S' });
            }
        }
    }
}

/// Fire Arrows: embers trailing each burning arrow.
fn embers(cv: &mut Img, run: &Run) {
    for shot in run.projectiles.iter().filter(|p| p.traits.fire) {
        let (x, y) = at(shot.x, shot.y);
        let speed = shot.vx.hypot(shot.vy).max(1e-3);
        let (ux, uy) = (shot.vx / speed, shot.vy / speed);
        for k in 0..4 {
            let flick = (hash(x, k, run.tick as u32) % 3) as i32 - 1;
            let back = 3.0 + k as f32 * 2.0;
            cv.put(
                x - (ux * back).round() as i32 - (uy * flick as f32).round() as i32,
                y - (uy * back).round() as i32 + (ux * flick as f32).round() as i32,
                ['6', '@', '7', '8'][k as usize],
            );
        }
    }
}

/// A room's end: coins, each a small gold disc with a glint, flying home.
fn coins(cv: &mut Img, run: &Run) {
    for (n, c) in run.feel.coins.iter().enumerate() {
        let (x, y) = at(c.x, c.y);
        // A coin turns as it flies: wide, then edge-on.
        let edge = (c.age / 3 + n as u32).is_multiple_of(3);
        let rim = if edge {
            [(0, -1), (0, 1)].as_slice()
        } else {
            &[(-1, 0), (1, 0), (0, -1), (0, 1)]
        };
        for &(dx, dy) in rim {
            cv.put(x + dx, y + dy, if dy < 0 { '6' } else { 'o' });
        }
        cv.put(x, y, if edge { '@' } else { 'c' });
        if !edge {
            cv.put(x - 1, y - 1, 'T');
        }
    }
}
