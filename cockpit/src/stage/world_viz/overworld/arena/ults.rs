//! The knights' ultimates, drawn: what is under the figures (a
//! Stillhour's dome, Trebuchet's target rings, Lady's Veil's wings,
//! Phantasm's images) and what is over them (a sniper's crosshair, a ready
//! ultimate's sparkle). Ultimates are live state: their marks are signal
//! inks.

use super::super::ink::{Img, hash};
use super::{UNIT, at, sprites, stand};
use crate::drive::together_shooter::ults::{STRIKE_FALL, ULT_FULL};
use crate::drive::together_shooter::{Hero, Run};

/// Under the figures: domes, target rings, wings and images.
pub(super) fn under(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    for sphere in &run.spheres {
        let (cx, cy) = at(sphere.x, sphere.y);
        let r = sphere.r * UNIT;
        let fading = sphere.left < 20 && (tick / 2).is_multiple_of(2);
        // The dome's rim, turning slowly, and the still air inside.
        let n = (r * 6.0) as i32;
        for k in 0..n {
            let a = k as f32 / n as f32 * std::f32::consts::TAU + tick as f32 * 0.01;
            if k % 3 == 0 || fading {
                continue;
            }
            cv.put(
                cx + (a.cos() * r) as i32,
                cy + (a.sin() * r * 0.8) as i32,
                if k % 2 == 0 { '3' } else { '2' },
            );
        }
        for y in (cy - r as i32)..(cy + r as i32) {
            for x in (cx - r as i32)..(cx + r as i32) {
                let (dx, dy) = ((x - cx) as f32, (y - cy) as f32 / 0.8);
                if dx.hypot(dy) < r - 2.0 && hash(x, y, 51).is_multiple_of(23) {
                    cv.put(x, y, '0');
                }
            }
        }
        // A clock's hands, stopped.
        cv.line(cx, cy, cx, cy - (r * 0.45) as i32, '2');
        cv.line(cx, cy, cx + (r * 0.3) as i32, cy, '2');
    }
    for strike in &run.strikes {
        let (x, y) = at(strike.x, strike.y);
        let k = strike.fall.min(STRIKE_FALL) as f32 / STRIKE_FALL as f32;
        let r = 6.0 + k * 10.0;
        let n = (r * 4.0) as i32;
        for i in 0..n {
            if i % 2 == 0 {
                continue;
            }
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            cv.put(
                x + (a.cos() * r) as i32,
                y + (a.sin() * r * 0.6) as i32,
                '7',
            );
        }
        if strike.fall < STRIKE_FALL {
            // The shell, coming down.
            let drop = (strike.fall as f32 * 2.5) as i32;
            cv.rect(x - 1, y - 2 - drop, 3, 3, 'K');
            cv.put(x, y - 3 - drop, 'J');
        }
    }
    for (&id, hero) in run.players.iter().filter(|(_, h)| h.hp > 0) {
        if hero.angel > 0 {
            wings(cv, hero, tick);
        }
        for phantom in run.phantoms.iter().filter(|p| p.owner == id) {
            let (x, y) = at(phantom.x, phantom.y);
            if phantom.left < 30 && (tick / 2).is_multiple_of(2) {
                continue;
            }
            let body = if hero.colours().is_empty() {
                sprites::knight(id)
            } else {
                sprites::knight_in(hero.colours())
            };
            let mut im = ghost(&body);
            if hero.aim_x < -0.3 {
                im = im.flip_h();
            }
            stand(cv, &im, x, y);
        }
    }
}

/// Over the figures: the sniper's crosshair, and a ready ultimate's
/// sparkle at its knight's feet.
pub(super) fn over(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    for hero in run.players.values().filter(|h| h.hp > 0) {
        if let Some((target, left)) = hero.aiming
            && let Some(enemy) = run.enemies.iter().find(|e| e.id == target)
        {
            let (x, y) = at(enemy.x, enemy.y);
            let r = 6 + (left as i32) / 4;
            for (dx, dy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
                cv.line(x + dx * r, y + dy * r, x + dx * (r - 3), y + dy * r, '7');
                cv.line(x + dx * r, y + dy * r, x + dx * r, y + dy * (r - 3), '7');
            }
            cv.put(x, y, '7');
            cv.line(x - 2, y, x + 2, y, '7');
            cv.line(x, y - 2, x, y + 2, '7');
        }
        if hero.ult_charge >= ULT_FULL && !hero.stone {
            let (x, y) = at(hero.x, hero.y);
            for k in 0..2 {
                let a = tick as f32 * 0.25 + k as f32 * std::f32::consts::PI;
                cv.put(
                    x + (a.cos() * 7.0) as i32,
                    y + 1 + (a.sin() * 2.5) as i32,
                    if k == 0 { '6' } else { '5' },
                );
            }
        }
    }
}

/// Lady's Veil's wings, spread behind a knight.
fn wings(cv: &mut Img, hero: &Hero, tick: u32) {
    let (x, y) = at(hero.x, hero.y);
    let beat = i32::from((tick / 6).is_multiple_of(2));
    for side in [-1, 1] {
        for k in 0..7 {
            let len = 8 - k / 2;
            let (wx, wy) = (x + side * (3 + k), y - 12 + k - beat);
            cv.line(
                wx,
                wy,
                wx + side * len / 3,
                wy + len,
                if k % 2 == 0 { 'w' } else { '3' },
            );
        }
        cv.put(x + side * 9, y - 13 - beat, '6');
    }
}

/// A knight's image: their figure in ghostly blues, by brightness.
fn ghost(im: &Img) -> Img {
    const RAMP: [char; 5] = ['0', 'q', '1', '2', '3'];
    let mut out = Img::new(im.w, im.h);
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some([r, g, b]) = im.get(x, y)
                && (x + y) % 3 != 0
            {
                let l = (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000;
                out.put(
                    x,
                    y,
                    RAMP[(l as usize * RAMP.len() / 256).min(RAMP.len() - 1)],
                );
            }
        }
    }
    out
}

/// A stunned monster, iced over: its figure in the stone blues.
pub(super) fn iced(im: &Img) -> Img {
    const RAMP: [char; 6] = ['s', 'x', 'u', 'U', 'v', 'W'];
    let mut out = Img::new(im.w, im.h);
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some([r, g, b]) = im.get(x, y) {
                let l = (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000;
                out.put(
                    x,
                    y,
                    RAMP[(l as usize * RAMP.len() / 256).min(RAMP.len() - 1)],
                );
            }
        }
    }
    out
}
