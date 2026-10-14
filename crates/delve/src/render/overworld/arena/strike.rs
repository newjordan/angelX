//! A knight who is hit. The whole figure goes white for a few ticks (the one
//! stark white in the Delve: it has to read as "I got hit" at a glance, and
//! a monster's hit is cream and then red), then flinches: thrown back a
//! step against the blow, bent over, and drawn upright again as the pose
//! fades. Sparks fly from the chest; a heavy hit throws more, and rings.
//!
//! All of it reads `Hero::struck`, `struck_big` and `struck_dir`, so a
//! friend's mirror and a browser guest's picture show it as the host does.

use super::super::ink::{Img, hash};
use crate::rules::{Hero, spectacle::STRUCK};

/// Ticks of white. A heavy hit holds it a tick longer.
fn white_ticks(hero: &Hero) -> u32 {
    if hero.struck_big { 5 } else { 4 }
}

/// Ticks since the hit landed.
fn age(hero: &Hero) -> u32 {
    STRUCK.saturating_sub(hero.struck)
}

/// Whether the figure is white this tick: while it is, it never blinks.
pub(super) fn whiteout(hero: &Hero) -> bool {
    hero.struck > 0 && age(hero) < white_ticks(hero)
}

/// Every pixel of `im` white.
fn silhouette(im: &Img) -> Img {
    let mut out = Img::new(im.w, im.h);
    for y in 0..im.h {
        for x in 0..im.w {
            if im.get(x, y).is_some() {
                out.put(x, y, 'w');
            }
        }
    }
    out
}

/// The figure as the hit leaves it, and where it stands: `(picture, dx, dy)`
/// in pixels from where the knight is. Upright and unmoved when not struck.
pub(super) fn pose(im: &Img, hero: &Hero) -> (Img, i32, i32) {
    if hero.struck == 0 {
        return (im.clone(), 0, 0);
    }
    if whiteout(hero) {
        let (_, dx, dy) = lean(hero);
        return (silhouette(im), dx, dy);
    }
    let (bend, dx, dy) = lean(hero);
    (bent(im, bend), dx, dy)
}

/// How far the flinch has thrown the knight: a bend (pixels the top leans),
/// and the step back.
fn lean(hero: &Hero) -> (i32, i32, i32) {
    let a = age(hero);
    let ease = 1.0 - (a as f32 / STRUCK as f32);
    let reach = if hero.struck_big { 4.0 } else { 2.5 };
    let (dirx, diry) = hero.struck_dir;
    let dx = (f32::from(dirx) * reach * ease).round() as i32;
    let dy = (f32::from(diry) * reach * 0.5 * ease).round() as i32;
    let bend = (f32::from(dirx) * (if hero.struck_big { 3.0 } else { 2.0 }) * ease).round() as i32;
    (bend, dx, dy + if a < 8 { 1 } else { 0 })
}

/// `im` with its upper half pushed sideways `bend` pixels: a knight bent
/// over from a blow, the feet staying put.
fn bent(im: &Img, bend: i32) -> Img {
    if bend == 0 {
        return im.clone();
    }
    let mut out = Img::new(im.w + bend.abs(), im.h);
    let base = if bend < 0 { -bend } else { 0 };
    for y in 0..im.h {
        // Nothing moves below the knees; the head moves the most.
        let share = (im.h - 1 - y).saturating_sub(im.h / 4) as f32 / (im.h as f32 * 0.75);
        let shift = (bend as f32 * share.clamp(0.0, 1.0)).round() as i32;
        for x in 0..im.w {
            if let Some(c) = im.get(x, y) {
                out.set(base + x + shift, y, c);
            }
        }
    }
    out
}

/// Sparks thrown from the knight's chest at `(x, y)` (their feet), and for a
/// moment a burst where the blow landed.
pub(super) fn sparks(cv: &mut Img, hero: &Hero, id: u32, x: i32, y: i32) {
    if hero.struck == 0 {
        return;
    }
    let a = age(hero) as f32;
    let (cx, cy) = (x as f32, (y - 7) as f32);
    let seed = hash(hero.hp as i32, id as i32, 17);
    // The burst: a ring of rays at the moment of the blow.
    if a < 3.0 {
        let r = 3.0 + a * 3.0;
        for k in 0..8 {
            let ang = k as f32 * std::f32::consts::FRAC_PI_4 + 0.4;
            for step in 0..3 {
                let rr = r + step as f32;
                cv.put(
                    (cx + ang.cos() * rr).round() as i32,
                    (cy + ang.sin() * rr * 0.8).round() as i32,
                    if a < 1.0 { 'w' } else { '5' },
                );
            }
        }
    }
    let count = if hero.struck_big { 14 } else { 8 };
    let (dirx, diry) = hero.struck_dir;
    for n in 0..count {
        let h = hash(n, seed as i32, 29);
        // Thrown out and up, biased the way the blow came from.
        let ang = (h % 628) as f32 / 100.0;
        let speed = 1.6 + ((h >> 10) % 100) as f32 / 40.0;
        let life = 6.0 + ((h >> 18) % 8) as f32;
        if a >= life {
            continue;
        }
        let drift = f32::from(dirx) * -0.35;
        let px = cx + (ang.cos() * speed + drift) * a;
        let py = cy + ang.sin() * speed * a * 0.8 - f32::from(diry) * 0.2 * a + 0.12 * a * a;
        let ink = if a < life * 0.4 {
            '6'
        } else if a < life * 0.75 {
            '@'
        } else {
            '7'
        };
        let (px, py) = (px.round() as i32, py.round() as i32);
        cv.put(px, py, ink);
        if a < 3.0 {
            cv.put(px + 1, py, ink);
        }
    }
}
