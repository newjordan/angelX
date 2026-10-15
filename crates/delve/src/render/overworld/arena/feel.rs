//! How the arms feel, drawn: a rail's fading tracer, sparks where a shot
//! strikes, shafts stuck in the stone, a muzzle's flash and a handgonne's
//! smoke, a death's white frame and burst, Fire Arrows' embers, and a
//! room's end paid out in coins flying home.
//!
//! All of it reads `Run::feel` (`drive::together_shooter::feel`), so a
//! friend's mirror and a browser guest's picture show it as the host does.

use super::super::ink::{Img, hash};
use super::at;
use crate::rules::feel::{DEATH_HOLD, DEATH_TICKS, SMOKE_TICKS, TRACER_TICKS};
use crate::rules::{Run, Shot};

/// A shot's colours, core then rim, for its sparks.
fn inks(kind: Shot) -> (char, char) {
    match kind {
        Shot::Arrow => ('c', 'o'),
        Shot::Bolt => ('c', 'U'),
        Shot::Ball => ('c', 'o'),
        Shot::Sacred => ('9', '@'),
        _ => ('c', 'u'),
    }
}

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

/// Over the figures: tracers, sparks, muzzles, smoke, deaths, embers,
/// coins.
pub(super) fn over(cv: &mut Img, run: &Run) {
    tracers(cv, run);
    embers(cv, run);
    puffs(cv, run);
    sparks(cv, run);
    deaths(cv, run);
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

/// A muzzle's flash, a star along the aim; a handgonne's smoke, a grey
/// puff swelling and thinning.
fn puffs(cv: &mut Img, run: &Run) {
    // Smoke lies behind the bright muzzle, so both read on the first frame.
    for p in run
        .feel
        .puffs
        .iter()
        .filter(|p| p.smoke)
        .chain(run.feel.puffs.iter().filter(|p| !p.smoke))
    {
        let (x, y) = at(p.x, p.y);
        if p.smoke {
            let age = SMOKE_TICKS.saturating_sub(p.ttl) as i32;
            let r = 2 + age / 2;
            let smoke = super::fx::stamp(super::fx::Stamp::Smoke, r, r, (age / 4) as u8);
            cv.stamp(&smoke, x - r, y - r);
        } else {
            super::fx::glow(cv, x, y, super::fx::Hue::Warm);
            let reach = 2 + i32::from(p.ttl) * 2;
            for k in 0..=reach {
                let k = k as f32;
                cv.put(
                    x + (p.dx * k).round() as i32,
                    y + (p.dy * k).round() as i32,
                    if k < 1.0 { '9' } else if k < 3.0 { 'c' } else { '@' },
                );
            }
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                cv.put(x + dx, y + dy, '5');
            }
            cv.put(x, y, '9');
        }
    }
}

/// Sparks where a shot struck: a little star in its colours, flying apart.
fn sparks(cv: &mut Img, run: &Run) {
    for s in &run.feel.impacts {
        let (x, y) = at(s.x, s.y);
        let (core, rim) = inks(s.kind);
        if s.ttl > 3 { super::fx::glow(cv, x, y, super::fx::Hue::Warm); }
        let spread = 7 - i32::from(s.ttl);
        for n in 0..6 {
            let a = n as f32 / 6.0 * std::f32::consts::TAU + (hash(x, y, 5) % 7) as f32 * 0.3;
            let r = spread as f32 + (hash(x, n, 9) % 2) as f32;
            cv.put(
                x + (a.cos() * r).round() as i32,
                y + (a.sin() * r).round() as i32,
                if s.ttl > 3 { core } else { rim },
            );
        }
        if s.ttl > 3 {
            cv.put(x, y, '9');
        }
    }
}

/// A death: a shaded flash disc for the held frames, then its parts flying out
/// and a ring of dust.
fn deaths(cv: &mut Img, run: &Run) {
    for d in &run.feel.deaths {
        let (x, y) = at(d.x, d.y);
        let r = (d.r * super::UNIT).max(2.0);
        let age = DEATH_TICKS.saturating_sub(d.ttl);
        if age < DEATH_HOLD {
            let ri = r.round() as i32;
            let held = super::fx::stamp(super::fx::Stamp::Death, ri, ri, age);
            cv.stamp(&held, x - ri, y - ri);
            continue;
        }
        let k = f32::from(age - DEATH_HOLD);
        for n in 0..10 {
            let a = n as f32 / 10.0 * std::f32::consts::TAU + (hash(x, y, 3) % 9) as f32 * 0.2;
            let fly = r + k * (1.6 + (hash(n, x, 11) % 3) as f32 * 0.5);
            let (px, py) = (
                x + (a.cos() * fly).round() as i32,
                y + (a.sin() * fly).round() as i32 + (k * k * 0.05) as i32,
            );
            let ink = if k < 2.0 { 'c' } else if k < 4.0 { 'V' } else { 'u' };
            cv.put(px, py, ink);
            if n % 3 == 0 {
                cv.put(px + 1, py, 'G');
            }
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
