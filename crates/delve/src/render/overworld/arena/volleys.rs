//! The monsters' pattern shots where a plain sprite will not do: a beam's
//! warning line and its burn, the marks where rain and crossfire will come
//! from, and the blink of a mine about to pop (`patterns`).

use super::*;
use crate::rules::patterns::{BEAM_WARN, SWEEP_WARN, warning_reach};
use crate::rules::{Projectile, Shot};

/// A beam's inks by its shot: warning, rim, core.
fn beam_inks(kind: Shot) -> (char, char, char) {
    match kind {
        Shot::Eye => ('1', '2', 'c'),
        Shot::Wisp => ('2', '2', 'c'),
        Shot::Spore => ('C', 'A', 'c'),
        Shot::Page => ('$', 'U', 'c'),
        _ => ('7', 'o', 'c'),
    }
}

/// Draw what a plain sprite cannot. True when the shot is drawn whole
/// (a beam, or a shot still waiting) and wants no sprite.
pub(super) fn draw(cv: &mut Img, run: &Run, shot: &Projectile) -> bool {
    let path = &shot.traits.path;
    if let Some(beam) = path.beam {
        if crate::rules::bosses::Art::of_shot(shot) == Some(crate::rules::bosses::Art::Leviathan) {
            super::boss_art::leviathan_laser(cv, run, shot, beam);
            return true;
        }
        let (warn, rim, core) = crate::rules::bosses::Art::of_shot(shot)
            .map(super::boss_art::inks)
            .unwrap_or_else(|| beam_inks(shot.kind));
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
            // where it will end, fainter. A guardian's charged beam (a
            // longer warning) thickens as it charges.
            let close = path.age + 7 > u16::from(beam.warn);
            let march = (run.tick / 2) as i32;
            let art = crate::rules::bosses::Art::of_shot(shot);
            let half = (crate::rules::patterns::BEAM_HALF * UNIT).ceil() as i32;
            let fill =
                i32::from(path.age.min(u16::from(beam.warn))) * half / i32::from(beam.warn.max(1));
            // Only a guardian's wind-up warns longer than a sweep does.
            let charged = beam.warn > BEAM_WARN.max(SWEEP_WARN);
            let charge = f32::from(path.age) / f32::from(beam.warn.max(1));
            let width = if !charged {
                0
            } else if charge > 0.75 {
                2
            } else if charge > 0.4 {
                1
            } else {
                0
            };
            line(cv, (x, y), beam.angle, length, |cv, px, py, (nx, ny)| {
                let i = (px - x).abs().max((py - y).abs());
                if let Some(art) = art {
                    if art == crate::rules::bosses::Art::Index {
                        let stage =
                            (usize::from(path.age) * 4 / usize::from(beam.warn.max(1))).min(3);
                        for side in -half..=half {
                            if let Some(c) =
                                super::boss_art::index_lane_pixel(i, side, half, length, stage)
                            {
                                cv.set(px + nx * side, py + ny * side, c);
                            }
                        }
                        return;
                    }
                    for side in -half..=half {
                        let motif = match art {
                            crate::rules::bosses::Art::Cinderjaw => {
                                (i + side.abs() * 2).rem_euclid(12) < 2
                            }
                            crate::rules::bosses::Art::Index => {
                                i.rem_euclid(12) < 2 || (side == 0 && i.rem_euclid(12) < 6)
                            }
                            crate::rules::bosses::Art::Mother => (i + side * 3).rem_euclid(10) < 2,
                            crate::rules::bosses::Art::Waxen => (i + side.abs()).rem_euclid(11) < 2,
                            crate::rules::bosses::Art::Choir => (i - side * 2).rem_euclid(13) < 2,
                            crate::rules::bosses::Art::Marshal => (i + side * 2).rem_euclid(9) < 2,
                            // These rosters emit travelling shots and rage rings.
                            // Retain the shared lane fallback for tagged snapshots.
                            crate::rules::bosses::Art::TaxKeeper
                            | crate::rules::bosses::Art::RelicLiege
                            | crate::rules::bosses::Art::IronTusk
                            | crate::rules::bosses::Art::RedPick
                            | crate::rules::bosses::Art::Foreman
                            | crate::rules::bosses::Art::Leviathan => i.rem_euclid(12) < 2,
                            _ => i.rem_euclid(12) < 2,
                        };
                        if side.abs() >= half - fill || motif {
                            cv.put(
                                px + nx * side,
                                py + ny * side,
                                if motif { core } else { rim },
                            );
                        }
                    }
                } else if close || (i + march) % 4 < 2 {
                    cv.put(px, py, warn);
                    for k in 1..=width {
                        cv.put(px + nx * k, py + ny * k, warn);
                        cv.put(px - nx * k, py - ny * k, warn);
                    }
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
            let art = crate::rules::bosses::Art::of_shot(shot);
            if art == Some(crate::rules::bosses::Art::Index) {
                index_crossfire_lane(cv, run, shot);
                return true;
            }
            let (rim, core) = art
                .map(|art| {
                    let (warn, rim, core) = super::boss_art::inks(art);
                    (if lit { rim } else { warn }, core)
                })
                .unwrap_or(if lit { ('@', '6') } else { ('7', '8') });
            if art.is_none() {
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
        if crate::rules::bosses::Art::of_shot(shot).is_some() && path.accel < 0.0 && path.top == 0.0
        {
            // Its destination motif owns the stopped mine. A full projectile
            // stamped over it would hide the inward fill and its cracks/web.
            return shot.vx.hypot(shot.vy) < 0.05;
        }
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
                    crate::rules::bosses::Art::of_shot(shot)
                        .map(|art| super::boss_art::inks(art).0)
                        .unwrap_or(if still { '7' } else { '8' }),
                );
            }
        }
    }
    false
}

/// Mark the actual travelling corridor of held catalogue shots. The wall
/// query owns the endpoint and the point-shot hit envelope owns the width.
/// Raster stepping uses integers, including diagonal lanes.
fn index_crossfire_lane(cv: &mut Img, run: &Run, shot: &Projectile) {
    let (x, y) = at(shot.x, shot.y);
    let angle = shot.vy.atan2(shot.vx);
    let length = (warning_reach(run, (shot.x, shot.y), angle) * UNIT) as i32;
    let speed = shot.vx.hypot(shot.vy);
    if speed == 0.0 {
        return;
    }
    let end = (
        x + (shot.vx / speed * length as f32).round() as i32,
        y + (shot.vy / speed * length as f32).round() as i32,
    );
    let half = ((crate::rules::HERO_RADIUS + 0.16) * UNIT).ceil() as i32;
    let side = if shot.vx.abs() > shot.vy.abs() {
        (0, 1)
    } else {
        (1, 0)
    };
    let warning = crate::rules::patterns::CROSS_MARK;
    let stage = (usize::from(warning.saturating_sub(shot.traits.path.wait)) * 4
        / usize::from(warning))
    .min(3);
    let (mut px, mut py) = (x, y);
    let (dx, dy) = ((end.0 - x).abs(), -(end.1 - y).abs());
    let (sx, sy) = ((end.0 - x).signum(), (end.1 - y).signum());
    let mut error = dx + dy;
    loop {
        let i = (px - x).abs().max((py - y).abs());
        for k in -half..=half {
            if let Some(c) = super::boss_art::index_lane_pixel(i, k, half, length, stage) {
                cv.set(px + side.0 * k, py + side.1 * k, c);
            }
        }
        if (px, py) == end {
            break;
        }
        let twice = 2 * error;
        if twice >= dy {
            error += dy;
            px += sx;
        }
        if twice <= dx {
            error += dx;
            py += sy;
        }
    }
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

/// A monster's shot's three tones, head to tail, from its delve's look:
/// the realm's own inks, never white on black.
fn tones(kind: Shot) -> (char, char, char) {
    match kind {
        Shot::Wisp => ('3', '2', 'q'),
        Shot::Bone => ('H', 'h', 'G'),
        Shot::Ore => ('T', 'O', 'r'),
        Shot::Flame | Shot::Ember => ('6', '@', 'o'),
        Shot::Page => ('$', 'U', 'u'),
        Shot::Spore => ('Y', 'C', 'm'),
        Shot::Eye => ('3', '2', '1'),
        Shot::Ball => ('t', 'O', 'r'),
        _ => ('V', 'v', 'u'),
    }
}

/// The glow a monster's shot casts, by its delve's look.
fn hue(kind: Shot) -> fx::Hue {
    match kind {
        Shot::Flame | Shot::Ember | Shot::Ore | Shot::Ball => fx::Hue::Warm,
        Shot::Spore => fx::Hue::Gas,
        _ => fx::Hue::Cold,
    }
}

/// How long a shot's streak is, in native pixels: its speed made visible,
/// so a dart drags a long one, a slowing shot's shortens, a heavy orb has
/// none.
fn streak(speed: f32) -> i32 {
    (speed * 1.1).round() as i32
}

/// A monster's pattern shot drawn as what it does (`patterns::Family`):
/// darts a long thin streak, heavy orbs a wide dark ball with a contact
/// shadow, lobs a body up in the air over a shadow and a ring where they
/// will land; every other moving shot trails a streak as long as it is
/// fast (a wavy one a wavy streak) under its delve's sprite. True when
/// the shot is drawn whole.
pub(super) fn family(cv: &mut Img, run: &Run, shot: &Projectile) -> bool {
    use crate::rules::HERO_RADIUS;
    use crate::rules::patterns::{Family, LOB_SPLASH, family as of, lob_flight};
    if !shot.hostile || shot.look.is_some() || !shot.in_flight() {
        return false;
    }
    let (x, y) = at(shot.x, shot.y);
    let speed = shot.vx.hypot(shot.vy);
    let (ux, uy) = if speed > 1e-3 { (shot.vx / speed, shot.vy / speed) } else { (0.0, 1.0) };
    let (head, mid, tail) = tones(shot.kind);
    let back = |k: f32| (x - (ux * k).round() as i32, y - (uy * k).round() as i32);
    match of(shot) {
        Family::Dart => {
            // A needle: two bright pixels at the point, a long fading line.
            let n = streak(speed).max(6);
            for k in (0..n).rev() {
                let ink = match k {
                    0 | 1 => head,
                    _ if k < n / 2 => mid,
                    _ => tail,
                };
                let (px, py) = back(k as f32);
                cv.put(px, py, ink);
            }
            true
        }
        Family::Orb => {
            // Wide and slow: a contact shadow, a dark ball, a lit rim on the
            // side it rolls toward, a soft highlight up and to the left.
            crate::render::overworld::material::shadow(cv, (x as f32, y as f32 + 5.0), (6.0, 2.0), &|_, _| true);
            fx::glow(cv, x, y, hue(shot.kind));
            let r = 5;
            for dy in -r..=r {
                for dx in -r..=r {
                    let d = ((dx * dx + dy * dy) as f32).sqrt();
                    if d > r as f32 + 0.3 {
                        continue;
                    }
                    let ink = if d > r as f32 - 0.8 { mid } else if (dx + dy) < -3 { mid } else { tail };
                    cv.put(x + dx, y + dy, ink);
                }
            }
            cv.put(x - 2, y - 2, head);
            cv.put(x - 1, y - 2, head);
            cv.put(x - 2, y - 1, head);
            true
        }
        Family::Lob => {
            let Some(((lx, ly), rise)) = lob_flight(shot) else {
                return false;
            };
            // Where it will land: a ring the size of its splash, closing
            // in and filling as it comes down.
            let path = &shot.traits.path;
            let k = f32::from(path.age) / f32::from(path.fuse.max(1));
            let (tx, ty) = at(lx, ly);
            let r = (HERO_RADIUS + LOB_SPLASH) * UNIT;
            let ring = if k > 0.7 { '7' } else { '8' };
            let n = 40;
            let step = if k > 0.7 { 1 } else { 2 };
            for i in (0..n).step_by(step) {
                let a = i as f32 / n as f32 * std::f32::consts::TAU + run.tick as f32 * 0.05;
                cv.put(tx + (a.cos() * r).round() as i32, ty + (a.sin() * r * 0.6).round() as i32, ring);
            }
            let inner = r * (1.0 - k).max(0.15);
            for i in (0..n).step_by(4) {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                cv.put(tx + (a.cos() * inner).round() as i32, ty + (a.sin() * inner * 0.6).round() as i32, '8');
            }
            // Its shadow on the ground, smaller the higher it is, and the
            // body up in the air above it.
            let lift = (rise * UNIT).round() as i32;
            let size = (4.5 - rise * 0.5).max(2.0);
            let (sw, sh) = (size.round() as i32, (size * 0.5).round().max(1.0) as i32);
            for dy in -sh..=sh {
                for dx in -sw..=sw {
                    let (fx_, fy_) = (dx as f32 / size, dy as f32 / (size * 0.5).max(1.0));
                    let d = fx_ * fx_ + fy_ * fy_;
                    if d <= 1.0 {
                        cv.put(x + dx, y + dy, if d > 0.55 { 'j' } else { 'g' });
                    }
                }
            }
            let (bx, by) = (x, y - lift);
            fx::glow(cv, bx, by, hue(shot.kind));
            for (dx, dy) in [(0, -2), (-1, -1), (0, -1), (1, -1), (-2, 0), (-1, 0), (0, 0), (1, 0), (2, 0), (-1, 1), (0, 1), (1, 1), (0, 2)] {
                cv.put(bx + dx, by + dy, if dy < 0 && dx <= 0 { head } else { mid });
            }
            cv.put(bx + 1, by + 1, tail);
            true
        }
        Family::Wave => {
            // A wavy wake, swinging the way it swings.
            let n = streak(speed);
            let phase = f32::from(shot.traits.path.age) * shot.traits.path.wave_rate;
            for k in (2..n + 2).rev() {
                let side = (phase - k as f32 * 0.5).sin() * 2.0 * shot.traits.path.wave.signum();
                let (px, py) = back(k as f32);
                cv.put(px + (-uy * side).round() as i32, py + (ux * side).round() as i32, if k < n / 2 + 2 { mid } else { tail });
            }
            false
        }
        _ => {
            // A streak as long as it is fast: a quickening shot's grows, a
            // slowing one's shrinks to nothing as it hangs.
            let n = streak(speed);
            for k in (2..n + 2).rev() {
                let (px, py) = back(k as f32);
                cv.put(px, py, if k < n / 2 + 2 { mid } else { tail });
            }
            false
        }
    }
}
