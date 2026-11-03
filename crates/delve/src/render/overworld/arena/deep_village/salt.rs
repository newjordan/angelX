//! The Saltkin's cavern: white salt, cut in terraces. The floor is a salt
//! flat crazed into plates, brine pools standing in it in steps with white
//! rims; the back wall climbs in terraces, each lip scalloped like a frozen
//! fall and a pool of brine on every ledge; the houses are cut blocks of
//! salt with salt-lamps in their windows the colour of dawn. In the plaza
//! the Salt Spire stands in its brine, a stack of great cubes lit from
//! inside.

use super::super::super::ink::{Img, hash, vnoise};
use super::super::super::light::Light;
use super::super::super::material as mat;
use super::super::TILE;
use super::{Look, all, px_rect};
use crate::rules::{
    Run,
    deep::{Village, village as v},
};

/// Salt, dark to light.
const SALT: [char; 7] = ['g', 'j', 'G', 'J', 'h', 'i', 'H'];

/// The brine pools on the floor, as `(x, y, w, h)` in pixels: where the
/// ways do not run.
pub(super) const POOLS: [(i32, i32, i32, i32); 6] = [(108, 186, 40, 14), (420, 186, 44, 14), (60, 312, 50, 16), (468, 330, 34, 12), (220, 330, 36, 12), (340, 336, 30, 10)];

/// The salt flat: plates crazed apart, their seams pale, crystals catching
/// the light; brine pools in it with white rims.
pub(super) fn salt_floor(cv: &mut Img) {
    let (w, h) = (cv.w, cv.h);
    for y in (v::BACK as i32 * TILE)..h - TILE {
        for x in TILE..w - TILE {
            // A plate is the nearest of a scatter of points: its seam is
            // where two are nearly as near.
            let (cell, edge) = plates(x, y);
            let fine = hash(x, y, 9901) % 100;
            // Each plate one even crust, a little raised and paler at its
            // edge, laid in an ordered dither so it reads as a surface.
            let rim = edge < 3.5;
            let tone = (hash(cell, 0, 9902) % 3) as usize;
            let density = if rim { 0.62 } else { 0.34 + tone as f32 * 0.06 };
            if edge < 1.3 {
                cv.put(x, y, if fine < 50 { 'g' } else { 'X' });
            } else if super::super::super::ink::bayer(x, y) < density {
                cv.put(x, y, ['j', 'G', 'J', 'h'][(tone + usize::from(rim)).min(3)]);
            } else if fine > 98 {
                cv.put(x, y, 'i');
            }
        }
    }
    for (k, &(x, y, pw, ph)) in POOLS.iter().enumerate() {
        brine(cv, (x, y, pw, ph), k as u32);
    }
}

/// Which plate `(x, y)` is on, and how far it is from that plate's seam.
fn plates(x: i32, y: i32) -> (i32, f32) {
    let (gx, gy) = (x.div_euclid(28), y.div_euclid(22));
    let mut best = (f32::MAX, f32::MAX, 0);
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (cx, cy) = (gx + dx, gy + dy);
            let h = hash(cx, cy, 9903);
            let (px_, py) = (cx * 28 + (h % 28) as i32, cy * 22 + ((h >> 8) % 22) as i32);
            let d = ((x - px_) as f32).hypot((y - py) as f32);
            let id = cx * 977 + cy;
            if d < best.0 {
                best = (d, best.0, id);
            } else if d < best.1 {
                best.1 = d;
            }
        }
    }
    (best.2, best.1 - best.0)
}

/// A brine pool in a white rim: pale water, a darker deep, a glint.
pub(super) fn brine(cv: &mut Img, (x, y, w, h): (i32, i32, i32, i32), seed: u32) {
    let (cx, cy) = (x + w / 2, y + h / 2);
    for yy in y - 2..y + h + 3 {
        for xx in x - 3..x + w + 3 {
            let (dx, dy) = ((xx - cx) as f32 / (w / 2 + 3) as f32, (yy - cy) as f32 / (h / 2 + 2) as f32);
            let d = dx * dx + dy * dy;
            if d >= 1.0 {
                continue;
            }
            let inner = d < 0.62;
            let ink = if !inner {
                // The rim, scalloped where it overflowed and dried.
                if dy > 0.3 && (xx / 3) % 2 == 0 { 'H' } else if dy < -0.3 { 'h' } else { 'i' }
            } else if d < 0.2 {
                'Q'
            } else if vnoise(xx as f32 / 4.0, yy as f32 / 2.0, 9904 + seed) > 0.6 {
                'z'
            } else {
                'U'
            };
            cv.put(xx, yy, ink);
        }
    }
    cv.line(cx - w / 4, cy - 1, cx - w / 4 + 4, cy - 1, 'W');
}

/// The back wall climbs in terraces of salt, each lip scalloped like a
/// frozen fall, brine on every ledge, salt-lamps lit in the face as the
/// village wakes.
pub(super) fn back(cv: &mut Img, vl: &Village, lights: &mut Vec<Light>) {
    let w = cv.w;
    for (k, lip) in [52, 70, 88, 104].into_iter().enumerate() {
        for x in 0..w {
            let wob = (vnoise(x as f32 / 17.0, k as f32 * 3.0, 9910) * 6.0) as i32;
            let y = lip + wob;
            // The ledge's lit top, and the scallops hanging off its lip.
            cv.put(x, y, 'H');
            cv.put(x, y + 1, 'i');
            let drip = 2 + (hash(x / 5, k as i32, 9911) % 5) as i32;
            let scallop = drip - ((x % 5) - 2).abs();
            for d in 2..scallop.max(2) + 2 {
                cv.put(x, y + d, if d < 3 { 'h' } else { 'J' });
            }
            // A pool of brine on the ledge, in places.
            if vnoise(x as f32 / 30.0, k as f32, 9912) > 0.62 {
                cv.put(x, y - 1, if x % 4 == 0 { 'z' } else { 'Q' });
                cv.put(x, y - 2, 'U');
            }
        }
    }
    // Salt-lamp windows cut in the terraces.
    for k in 0..14 {
        let h = hash(k, 5, 9913);
        let x = 20 + (h % 536) as i32;
        let y = [62, 80, 98][((h >> 10) % 3) as usize];
        let lit = (k as u8 % 8) < vl.hope;
        cv.rect(x - 2, y - 3, 5, 6, 'G');
        cv.rect(x - 1, y - 2, 3, 4, if lit { '$' } else { 'j' });
        if lit {
            cv.put(x, y - 1, '@');
            lights.push(Light { x: x as f32, y: y as f32, r: 26.0, s: 0.28, fire: false });
        }
    }
}

/// A cube of salt: lit top, pale left face, shaded right, `s` on a side,
/// its near corner at `(x, foot)`.
pub(super) fn cube(cv: &mut Img, x: i32, foot: i32, s: i32, glow: bool) {
    let top = s / 2;
    for d in 0..s {
        for o in 0..s {
            // Lit from inside, the salt shows rose through its faces, hot
            // at its heart.
            let core = glow && (o - s / 2).abs() < s / 4 && (d - s / 2).abs() < s / 4;
            let ink = if core { if (d + o) % 2 == 0 { '@' } else { '9' } } else if o < s / 2 { if glow && (d + o) % 3 == 0 { 'c' } else { 'H' } } else if glow && (d * 3 + o) % 4 == 0 { '$' } else { 'h' };
            cv.put(x - s / 2 + o, foot - d, ink);
        }
    }
    for d in 0..top {
        let half = s / 2 - d;
        for o in -half..=half {
            cv.put(x + o, foot - s - d, if glow { if o == 0 { '9' } else { 'W' } } else { 'W' });
        }
    }
    cv.line(x - s / 2, foot - s, x + s / 2, foot - s, 'i');
    cv.line(x, foot - s, x, foot, if glow { 'c' } else { 'i' });
}

/// A salt house: blocks of white salt cut and laid, a flat roof with salt
/// cubes grown on it, a dark doorway, windows lit rose, a brine pan before
/// it with a rake against the wall.
pub(super) fn house(cv: &mut Img, (x, y, w, h): (i32, i32, i32, i32), seed: u32, lit: bool, lights: &mut Vec<Light>) {
    let foot = y + h;
    mat::shadow(cv, ((x + w / 2 + 8) as f32, (foot + 2) as f32), (w as f32 * 0.5, 4.0), &all);
    // Two terraces of cut salt, the upper set back: big blocks, pale,
    // their seams fine.
    let cut: &[char] = &['G', 'J', 'h', 'i', 'i', 'H', 'W'];
    mat::blocks(cv, (x + 2, y - 8, w - 4, h + 6), cut, (16, 9), 9920 + seed);
    mat::blocks(cv, (x + 10, y - 26, w - 20, 18), cut, (14, 9), 9921 + seed);
    // The light falls on its left: its right is a step down.
    for yy in y - 26..y + h - 2 {
        for xx in [x + w - 4, x + w - 3, x + w - 12, x + w - 11] {
            if let Some(c) = cv.get(xx, yy) {
                cv.set(xx, yy, mat::darker(c));
            }
        }
    }
    for (yy, x0, x1) in [(y - 9, x + 1, x + w - 2), (y - 27, x + 9, x + w - 10)] {
        cv.line(x0, yy, x1, yy, 'W');
        cv.line(x0, yy + 1, x1, yy + 1, 'H');
        // Its lip dripping salt.
        for xx in (x0..x1).step_by(3) {
            cv.put(xx, yy + 2, 'i');
        }
    }
    // Salt grown in cubes on its roof.
    for k in 0..(w / 18) {
        let hk = hash(k, seed as i32, 9922);
        cube(cv, x + 14 + k * 16 + (hk % 5) as i32, y - 28, 6 + (hk >> 4) as i32 % 6, lit && k % 2 == 0);
    }
    // The doorway, deep in the salt.
    let dx = x + w / 2;
    cv.rect(dx - 7, foot - 20, 14, 20, 'G');
    cv.ellipse(dx as f32, (foot - 20) as f32, 7.0, 5.0, 'G');
    cv.rect(dx - 5, foot - 19, 10, 19, 'k');
    cv.ellipse(dx as f32, (foot - 19) as f32, 5.0, 4.0, 'k');
    if lit {
        cv.rect(dx - 3, foot - 8, 6, 8, '$');
        lights.push(Light { x: dx as f32, y: (foot - 6) as f32, r: 36.0, s: 0.3, fire: false });
    }
    // Windows lit rose with salt-lamps.
    for wx in [x + 9, x + w - 17, x + w / 2 - 4] {
        let wy = if wx == x + w / 2 - 4 { y - 21 } else { y + 2 };
        cv.rect(wx - 1, wy - 1, 10, 8, 'G');
        cv.rect(wx, wy, 8, 6, if lit { '$' } else { 'k' });
        if lit {
            cv.rect(wx + 1, wy + 1, 6, 4, 'c');
            cv.rect(wx + 3, wy + 2, 2, 2, '@');
            lights.push(Light { x: (wx + 4) as f32, y: (wy + 3) as f32, r: 40.0, s: 0.34, fire: false });
        } else {
            cv.put(wx + 1, wy + 1, 'j');
        }
    }
    // A brine pan before it, and a rake leaning by the door.
    brine(cv, (x + 4 + (seed % 2) as i32 * (w - 30), foot + 6, 20, 5), seed + 40);
    let rx = dx + 10;
    cv.line(rx, foot, rx + 3, foot - 22, 'P');
    cv.line(rx + 1, foot - 22, rx + 6, foot - 22, 'J');
    for t in 0..3 {
        cv.put(rx + 2 + t * 2, foot - 21, 'G');
    }
}

/// The Salt Spire: great cubes of salt stacked in a spire in a round pool
/// of brine with white terraced rims, the salt lit rose from inside as the
/// village wakes.
pub(super) fn heart(cv: &mut Img, vl: &Village, lights: &mut Vec<Light>, tick: u32) {
    let (x, y, w, h) = px_rect(v::HEART);
    let (cx, foot) = (x + w / 2, y + h);
    let awake = vl.hope >= 1;
    let light = vl.light();
    // The pool, in two terraces.
    brine(cv, (cx - 46, foot - 12, 92, 18), 7);
    brine(cv, (cx - 30, foot - 18, 60, 12), 8);
    // The cubes, biggest at the foot, turned a little each.
    let stack: [(i32, i32, i32); 7] = [(-14, 0, 22), (12, -2, 20), (-2, -6, 18), (-10, -20, 14), (8, -22, 13), (0, -34, 12), (2, -46, 9)];
    for (k, &(dx, dy, s)) in stack.iter().enumerate() {
        cube(cv, cx + dx, foot - 10 + dy, s, awake && (k as u8) < vl.hope + 1);
    }
    // Its crown: a point of salt, brightest of all.
    for d in 0..14 {
        let half = (14 - d) / 4;
        for o in -half..=half {
            cv.put(cx + 2 + o, foot - 56 - 10 - d, if o < 0 { 'W' } else { 'H' });
        }
    }
    if awake {
        let pulse = ((tick as f32 * 0.07).sin() * 0.5 + 0.5) * light;
        cv.put(cx + 2, foot - 70, if pulse > 0.4 { 'w' } else { '9' });
        lights.push(Light { x: cx as f32, y: (foot - 36) as f32, r: 100.0 + light * 60.0, s: 0.34 + light * 0.3, fire: false });
    }
}

/// The tasting pool: a cup of salt on a pedestal, brine in it.
pub(super) fn wishing_place(cv: &mut Img, vl: &Village, lights: &mut Vec<Light>, tick: u32) {
    let (x, y, w, h) = px_rect(v::WELL);
    let (cx, foot) = (x + w / 2, y + h);
    let glow = vl.hope >= 2;
    mat::shadow(cv, ((cx + 4) as f32, (foot + 1) as f32), (14.0, 3.0), &all);
    mat::ashlar(cv, (cx - 4, foot - 14, 9, 14), &SALT, 9930);
    cv.ellipse(cx as f32, (foot - 16) as f32, 14.0, 5.0, 'H');
    cv.ellipse(cx as f32, (foot - 15) as f32, 13.0, 3.5, 'h');
    cv.ellipse(cx as f32, (foot - 16) as f32, 10.0, 3.0, if glow { 'z' } else { 'U' });
    if glow {
        cv.put(cx - 3, foot - 17, 'c');
        // Steam off it, curling.
        for k in 0..6u32 {
            let t = ((tick + k * 13) % 40) as i32;
            cv.put(cx - 4 + (k as i32 * 2) + ((t as f32 * 0.2).sin() * 2.0) as i32, foot - 19 - t / 2, if t < 20 { 'i' } else { 'h' });
        }
        lights.push(Light { x: cx as f32, y: (foot - 16) as f32, r: 40.0, s: 0.32, fire: false });
    }
}

/// A lamp post's head: a block of salt glowing rose.
pub(super) fn lamp_head(cv: &mut Img, x: i32, y: i32, on: bool, lights: &mut Vec<Light>) {
    cube(cv, x + 1, y - 20, 8, on);
    if on {
        cv.put(x + 1, y - 25, '9');
        lights.push(Light { x: x as f32, y: (y - 24) as f32, r: 30.0, s: 0.3, fire: false });
    }
}

/// Steam off the brine, glints off the salt, flakes drifting.
pub(super) fn ambient(cv: &mut Img, run: &Run, vl: &Village, lk: &Look) {
    let tick = run.tick as u32;
    let lit = vl.light();
    for (k, &(x, y, w, _)) in POOLS.iter().enumerate() {
        for m in 0..4u32 {
            let h = hash(k as i32, m as i32, 9940);
            let life = 60 + h % 40;
            let t = (tick + h % life) % life;
            let px_ = x + (h % w as u32) as i32 + ((t as f32 * 0.1 + m as f32).sin() * 3.0) as i32;
            if (h >> 20) % 2 == 0 || t % 3 != 0 {
                cv.put(px_, y - (t as i32) / 3, if t < life / 2 { 'i' } else { 'h' });
            }
        }
    }
    for k in 0..(10 + (lit * 20.0) as i32) {
        let h = hash(k, 41, 9941);
        if (tick / 3 + h) % 50 < 2 {
            let (x, y) = (12 + (h % 552) as i32, 40 + ((h >> 10) % 300) as i32);
            cv.put(x, y, 'w');
            cv.put(x - 1, y, lk.glow[1]);
            cv.put(x + 1, y, lk.glow[1]);
            cv.put(x, y - 1, lk.glow[1]);
            cv.put(x, y + 1, lk.glow[1]);
        }
    }
    for k in 0..16 {
        let h = hash(k, 42, 9942);
        let t = ((tick + h % 260) % 260) as i32;
        let x = (h % 576) as i32 + ((t as f32 * 0.04 + k as f32).sin() * 5.0) as i32;
        cv.put(x, 70 + t, 'H');
    }
}

/// A floor below's salt-lamp: a post and a rough block of salt, dark or
/// glowing rose.
pub(super) fn sign_lamp(cv: &mut Img, x: i32, head: i32, lit: bool) {
    cube(cv, x, head + 2, 11, lit);
    if lit {
        cv.put(x, head - 6, '9');
        cv.put(x - 1, head - 5, 'c');
    }
}

/// A star of salt on the floor below.
pub(super) fn relic(cv: &mut Img, x: i32, cy: i32) {
    for (dx, dy) in [(0, -5), (4, -2), (3, 3), (-3, 3), (-4, -2)] {
        cv.line(x, cy, x + dx, cy + dy, 'W');
    }
    cv.rect(x - 1, cy - 1, 3, 3, 'H');
    cv.put(x, cy, 'w');
}

/// A cube of star-salt over a door, glowing.
pub(super) fn over_the_door(cv: &mut Img, cx: i32, cy: i32) {
    cube(cv, cx, cy, 6, true);
}
