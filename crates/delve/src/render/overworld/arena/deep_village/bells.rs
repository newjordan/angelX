//! The Chimefolk's cavern: a cathedral half under water. The floor is the
//! old flagstones under a hand's depth of still black water; the houses are
//! stalagmites hollowed into spires, a pointed window or two lit green and
//! a bell in every belfry; the far wall is an arcade of pointed arches with
//! a rose window high in it. In the plaza the Great Bell hangs from an arch
//! of two stalagmites grown together, and when it is awake every ring of
//! it goes out across the water.

use super::super::super::ink::{Img, hash, vnoise};
use super::super::super::light::Light;
use super::super::super::material as mat;
use super::super::{TILE, fire};
use super::{Look, all, px_rect};
use crate::rules::{
    Run,
    deep::{Village, village as v},
};

/// The cathedral's stone, dark to light.
const STONE: [char; 7] = ['k', 'K', 'g', 'j', 'G', 'J', 'h'];
/// Bell bronze, dark to light, and its verdigris.
const BRONZE: [char; 7] = ['n', 'b', 'I', 'R', 'o', 'O', 't'];
const VERDIGRIS: [char; 3] = ['e', 'C', 'y'];

/// The drowned floor: still black water a hand deep over the old
/// flagstones, their seams just showing through it, long low ripples
/// catching what light there is.
pub(super) fn water_floor(cv: &mut Img) {
    let (w, h) = (cv.w, cv.h);
    for y in (v::BACK as i32 * TILE)..h - TILE {
        for x in TILE..w - TILE {
            let n = vnoise(x as f32 / 23.0, y as f32 / 7.0, 9801);
            let row = y.div_euclid(18);
            let seam = y.rem_euclid(18) == 0 || (x + row * 13).rem_euclid(30) == 0;
            // Ripples run long and low across the water.
            let ripple = ((x as f32 / 13.0 + n * 7.0).sin() * (y as f32 / 3.0 + n).cos()) > 0.93;
            let ink = if ripple {
                if hash(x, y, 9805) % 3 == 0 { 'u' } else { 'S' }
            } else if seam {
                'x'
            } else if n > 0.7 {
                's'
            } else {
                'k'
            };
            let keep = ripple || (seam && hash(x, y, 9802) % 2 == 0) || (ink != 'k' && hash(x, y, 9803) % 4 == 0) || hash(x, y, 9806) % 9 == 0;
            if keep {
                cv.put(x, y, ink);
            }
        }
    }
}

/// A spire's reflection in the water under it: its dark shape upside down,
/// broken up by the ripples, its lit windows as streaks.
fn reflect(cv: &mut Img, cx: i32, foot: i32, half: i32, deep: i32, lit: bool) {
    for d in 1..deep {
        let f = d as f32 / deep as f32;
        let hw = ((half as f32) * (1.0 - f).powf(0.8)) as i32;
        let wob = ((d as f32 * 0.9).sin() * 1.5) as i32;
        for o in -hw..=hw {
            if hash(cx + o, foot + d, 9807) % 3 == 0 || d % 3 == 1 {
                continue;
            }
            cv.put(cx + o + wob, foot + d, if o.abs() < 2 && lit && d < deep / 2 { 'C' } else { 'K' });
        }
    }
}

/// The far wall: an arcade of pointed arches along its foot, tall shafts
/// between them, and the rose window high in the middle, lit green as the
/// cathedral wakes.
pub(super) fn back(cv: &mut Img, vl: &Village, lights: &mut Vec<Light>) {
    let face = v::BACK as i32 * TILE;
    // The arcade.
    for k in 0..12 {
        let cx = 24 + k * 48;
        let (top, foot) = (face - 46, face + 2);
        arch(cv, cx, top, foot, 14, vl.hope >= 3 && k % 3 == 1, lights);
        // A shaft between the arches, up into the dark.
        for y in 30..foot {
            let half = 3 + (y - 30) / 40;
            for o in -half..=half {
                cv.put(cx + 24 + o, y, STONE[(3 + o.signum() + (y / 9 % 2)) as usize]);
            }
        }
    }
    // The rose window.
    let (rx, ry) = (288, 44);
    let lit = vl.hope >= 1;
    cv.ellipse(rx as f32, ry as f32, 26.0, 26.0, 'K');
    cv.ellipse(rx as f32, ry as f32, 23.0, 23.0, if lit { 'e' } else { 'k' });
    for k in 0..12 {
        let a = k as f32 / 12.0 * std::f32::consts::TAU;
        for r in 4..23 {
            cv.put(rx + (a.cos() * r as f32) as i32, ry + (a.sin() * r as f32) as i32, 'K');
        }
        // The petals between the spokes.
        let b = a + std::f32::consts::PI / 12.0;
        let (px_, py) = (rx + (b.cos() * 15.0) as i32, ry + (b.sin() * 15.0) as i32);
        cv.ellipse(px_ as f32, py as f32, 3.0, 3.0, if lit { if vl.hope >= 4 { '3' } else { '2' } } else { 's' });
    }
    cv.ellipse(rx as f32, ry as f32, 5.0, 5.0, 'K');
    cv.ellipse(rx as f32, ry as f32, 3.0, 3.0, if lit { '3' } else { 'x' });
    if lit {
        lights.push(Light { x: rx as f32, y: ry as f32, r: 70.0, s: 0.3 + vl.light() * 0.2, fire: false });
    }
    // The water line along the wall's foot.
    for x in 0..cv.w {
        if hash(x, 1, 9804) % 2 == 0 {
            cv.put(x, face + 3, 'S');
        }
        cv.put(x, face + 4, 'x');
    }
}

/// A pointed arch, `half` wide each side of `cx`, from `top` to `foot`:
/// its dark mouth, its rim of stone, lit green within if `lit`.
fn arch(cv: &mut Img, cx: i32, top: i32, foot: i32, half: i32, lit: bool, lights: &mut Vec<Light>) {
    for y in top..foot {
        let t = (y - top) as f32 / (half as f32 * 1.6);
        let w = if t < 1.0 { (half as f32 * (1.0 - (1.0 - t) * (1.0 - t)).sqrt()) as i32 } else { half };
        for o in -w - 2..=w + 2 {
            let rim = o.abs() > w;
            let inner = o.abs() < w / 2 && y > top + half;
            let ink = if rim { STONE[if o < 0 { 5 } else { 3 }] } else if lit { if inner { '2' } else { 'e' } } else { 'k' };
            cv.put(cx + o, y, ink);
        }
    }
    if lit {
        lights.push(Light { x: cx as f32, y: (foot - 10) as f32, r: 34.0, s: 0.25, fire: false });
    }
}

/// A spire house: a stalagmite hollowed out, flowstone banding down its
/// sides, a pointed door at the water, pointed windows lit green, and a
/// bell in an open belfry near its tip.
pub(super) fn house(cv: &mut Img, (x, y, w, h): (i32, i32, i32, i32), seed: u32, lit: bool, lights: &mut Vec<Light>) {
    let cx = x + w / 2;
    let foot = y + h;
    let tall = h + 46 + (seed % 3) as i32 * 6;
    let tip = foot - tall;
    // The water round its foot.
    for yy in foot - 6..foot + 6 {
        for xx in x - 6..x + w + 6 {
            let (dx, dy) = ((xx - cx) as f32 / (w / 2 + 6) as f32, (yy - foot) as f32 / 6.0);
            if dx * dx + dy * dy < 1.0 && hash(xx / 3, yy, 9810) % 4 != 0 {
                cv.put(xx, yy, if (xx + yy) % 7 == 0 { 'S' } else { 'x' });
            }
        }
    }
    reflect(cv, cx, foot + 2, w / 2 - 4, 14, lit);
    // The spire: concave sides narrowing to its tip.
    for yy in tip..foot {
        let f = (yy - tip) as f32 / tall as f32;
        let half = ((w / 2 - 2) as f32 * f.powf(0.75)).max(1.0) as i32;
        for o in -half..=half {
            let u = o as f32 / half as f32;
            let light = (1.0 - (u + 0.4).abs() * 0.95).clamp(0.0, 1.0);
            let band = vnoise(o as f32 / 6.0, yy as f32 / 2.2, 9811 + seed);
            let mut i = (light * 5.2 + (band - 0.5) * 1.6) as i32 + 1;
            if u.abs() > 0.88 {
                i -= 1;
            }
            cv.put(cx + o, yy, STONE[i.clamp(0, 6) as usize]);
        }
        // Drips of flowstone down its flanks.
        if (yy + seed as i32 * 3) % 9 == 0 {
            cv.put(cx - half, yy + 1, STONE[4]);
            cv.put(cx + half, yy + 1, STONE[1]);
        }
    }
    // The belfry, an open arch near the tip, its bell inside.
    let (by, bw) = (tip + tall / 4, 6);
    pointed(cv, (cx, by), (bw, 10), 'k');
    bell(cv, cx, by - 4, 4, false, lit && seed % 2 == 0);
    // Pointed windows, lit green.
    for (k, (wx, wy)) in [(cx - w / 5, foot - h + 2), (cx + w / 5, foot - h + 6), (cx, tip + tall / 2)].into_iter().enumerate() {
        if k == 2 && seed % 2 == 1 {
            continue;
        }
        pointed(cv, (wx, wy), (3, 8), if lit { '2' } else { 'k' });
        if lit {
            cv.line(wx, wy - 4, wx, wy + 3, '3');
            lights.push(Light { x: wx as f32, y: wy as f32, r: 40.0, s: 0.34, fire: false });
        }
    }
    // The door at the water, two steps down into it.
    pointed(cv, (cx, foot - 7), (6, 16), 'k');
    cv.line(cx, foot - 18, cx, foot - 1, 'K');
    cv.line(cx - 8, foot - 1, cx + 8, foot - 1, STONE[4]);
    cv.line(cx - 9, foot + 1, cx + 9, foot + 1, STONE[2]);
    // A drowned candle on the step.
    candles(cv, cx + w / 2 - 4, foot + 1, 2, lit, lights);
}

/// A pointed opening centred at `(x, y)`, `hw` wide each side and `hh`
/// tall each way, in `ink`.
fn pointed(cv: &mut Img, (x, y): (i32, i32), (hw, hh): (i32, i32), ink: char) {
    for yy in y - hh..=y + hh {
        let t = (yy - (y - hh)) as f32 / hh as f32;
        let half = if t < 1.0 { (hw as f32 * t.sqrt()) as i32 } else { hw };
        for o in -half..=half {
            cv.put(x + o, yy, ink);
        }
    }
    // Its rim: a pale stone edge.
    for yy in y - hh..=y + hh {
        let t = (yy - (y - hh)) as f32 / hh as f32;
        let half = if t < 1.0 { (hw as f32 * t.sqrt()) as i32 } else { hw };
        cv.put(x - half - 1, yy, STONE[5]);
        cv.put(x + half + 1, yy, STONE[2]);
    }
}

/// A bell hanging from `(x, y)`, `r` wide each side at its lip: shoulder,
/// waist and flared lip, bronze lit on the left, verdigris on it; struck
/// or lit, it gleams.
pub(super) fn bell(cv: &mut Img, x: i32, y: i32, r: i32, struck: bool, gleam: bool) {
    let tall = r * 2 + 2;
    cv.line(x, y - 2, x, y, 'K');
    for d in 0..tall {
        let f = d as f32 / tall as f32;
        // Shoulder, a gentle waist, then the flare to the lip.
        let half = (r as f32 * (0.55 + 0.25 * f + 0.35 * f.powi(4))) as i32;
        for o in -half..=half {
            let u = o as f32 / half.max(1) as f32;
            let mut i = ((1.0 - (u + 0.4).abs()) * 6.0) as i32;
            if d == tall - 1 {
                i -= 2;
            }
            let ink = if hash(x + o, y + d, 9812) % 7 == 0 && !struck { VERDIGRIS[(hash(o, d, 9813) % 3) as usize] } else { BRONZE[i.clamp(0, 6) as usize] };
            cv.put(x + o, y + 1 + d, ink);
        }
    }
    cv.put(x, y + tall + 1, 'b');
    if gleam || struck {
        cv.put(x - r / 2, y + 2 + r / 2, '6');
        cv.put(x - r / 2, y + 3 + r / 2, '5');
    }
}

/// `n` drowned candles at `(x, foot)`: stubs of wax guttered down, their
/// flames cold green.
pub(super) fn candles(cv: &mut Img, x: i32, foot: i32, n: i32, lit: bool, lights: &mut Vec<Light>) {
    for k in 0..n {
        let (cx, tall) = (x + k * 3 - n, 3 + (k * 5 % 3));
        cv.rect(cx, foot - tall, 2, tall, 't');
        cv.put(cx + 1, foot - tall, 'T');
        cv.put(cx, foot, 'O');
        cv.put(cx + 1, foot, 'O');
        if lit {
            cv.put(cx, foot - tall - 1, '3');
            cv.put(cx, foot - tall - 2, '2');
            cv.put(cx + 1, foot - tall - 1, '2');
        } else {
            cv.put(cx, foot - tall - 1, 'K');
        }
    }
    if lit {
        lights.push(Light { x: x as f32, y: (foot - 6) as f32, r: 28.0, s: 0.32, fire: false });
    }
}

/// The heart: the Great Bell, hung in a pointed arch of two stalagmites
/// grown together over a round pool; green-gold when the cathedral wakes,
/// and from then on ringing out over the water (`ambient`).
pub(super) fn heart(cv: &mut Img, vl: &Village, lights: &mut Vec<Light>, tick: u32) {
    let (x, y, w, h) = px_rect(v::HEART);
    let (cx, foot) = (x + w / 2, y + h);
    let awake = vl.hope >= 1;
    let light = vl.light();
    // The pool under it, the bell's reflection in it.
    for yy in foot - 12..foot + 6 {
        for xx in cx - 46..cx + 46 {
            let (dx, dy) = ((xx - cx) as f32 / 46.0, (yy - foot + 3) as f32 / 9.0);
            let d = dx * dx + dy * dy;
            if d < 1.0 {
                let ink = if d > 0.82 { STONE[4] } else if hash(xx / 4, yy, 9820) % 6 == 0 { 'S' } else { 'x' };
                cv.put(xx, yy, ink);
            }
        }
    }
    // The two stalagmites, stout at the foot, leaning in to meet in a
    // pointed arch over the bell.
    let apex = foot - 92;
    for side in [-1, 1] {
        for yy in apex..foot - 2 {
            let f = (yy - apex) as f32 / (foot - 2 - apex) as f32;
            let lean = ((1.0 - f).powf(2.2) * 30.0) as i32;
            let px_ = cx + side * (36 - lean);
            let half = (5.0 + f.powf(0.6) * 10.0) as i32;
            for o in -half..=half {
                let u = o as f32 / half as f32;
                let band = vnoise(o as f32 / 5.0, yy as f32 / 2.5, 9822 + side as u32);
                // Lit on the side toward the bell once it glows.
                let toward = if awake { (-u * side as f32).max(0.0) * 0.8 } else { 0.0 };
                let i = ((1.0 - (u + 0.4).abs()) * 5.2 + (band - 0.5) * 1.4 + toward) as i32 + 1;
                cv.put(px_ + o, yy, STONE[i.clamp(0, 6) as usize]);
            }
        }
    }
    // Flowstone where they meet, and the beam the bell hangs from.
    cv.ellipse(cx as f32, (apex + 3) as f32, 9.0, 6.0, STONE[3]);
    cv.ellipse((cx - 2) as f32, (apex + 1) as f32, 5.0, 3.0, STONE[5]);
    cv.rect(cx - 14, apex + 8, 29, 3, 'b');
    cv.line(cx - 14, apex + 8, cx + 14, apex + 8, 'P');
    // The bell, swinging a little once awake.
    let swing = if awake { ((tick as f32 * 0.05).sin() * 2.0) as i32 } else { 0 };
    let (bx, by) = (cx + swing, apex + 11);
    cv.line(cx, by, bx, by + 4, 'K');
    let r = 17;
    let tall = r * 2 + 6;
    let half_at = |d: i32| {
        let f = d as f32 / tall as f32;
        (r as f32 * (0.45 + 0.25 * f + 0.45 * f.powi(5))) as i32
    };
    for d in 0..tall {
        let half = half_at(d);
        for o in -half..=half {
            let u = o as f32 / half.max(1) as f32;
            let mut i = ((1.0 - (u + 0.35).abs()) * 6.6) as i32;
            if d >= tall - 2 {
                i -= 2;
            }
            let rust = vnoise((bx + o) as f32 / 3.0, d as f32 / 3.0, 9821) > 0.7;
            let ink = if rust { VERDIGRIS[((d + o).unsigned_abs() % 3) as usize] } else { BRONZE[i.clamp(0, 6) as usize] };
            cv.put(bx + o, by + 4 + d, ink);
        }
    }
    // Its crown, its bands, and its clapper under the lip.
    cv.rect(bx - 4, by + 1, 9, 4, BRONZE[3]);
    cv.line(bx - 4, by + 1, bx + 4, by + 1, BRONZE[5]);
    for d in [9, tall - 7, tall - 4] {
        let half = half_at(d);
        cv.line(bx - half, by + 4 + d, bx + half, by + 4 + d, 'b');
    }
    cv.rect(bx - 2 - swing / 2, by + 4 + tall, 5, 3, 'K');
    if awake {
        // The light caught down its shoulder.
        for d in 4..tall - 6 {
            let half = half_at(d);
            cv.put(bx - half / 2 - 1, by + 4 + d, if light > 0.5 && d < tall / 2 { '6' } else { '5' });
        }
        lights.push(fire(bx, by + 24, 80.0 + light * 50.0, 0.3 + light * 0.25));
        lights.push(Light { x: cx as f32, y: (foot - 4) as f32, r: 70.0 + light * 40.0, s: 0.25 + light * 0.2, fire: false });
    }
    // Candles round the pool's edge.
    for k in 0..7 {
        let a = std::f32::consts::PI * (0.06 + k as f32 * 0.88 / 6.0);
        let (px_, py) = (cx + (a.cos() * 44.0) as i32, foot + (a.sin() * 5.0) as i32 - 1);
        candles(cv, px_, py, 2, vl.hope as usize > k, lights);
    }
}

/// The wishing bell: a small bell on an iron crook over a little pool.
pub(super) fn wishing_place(cv: &mut Img, vl: &Village, lights: &mut Vec<Light>, tick: u32) {
    let (x, y, w, h) = px_rect(v::WELL);
    let (cx, foot) = (x + w / 2, y + h);
    let glow = vl.hope >= 2;
    mat::shadow(cv, ((cx + 4) as f32, (foot + 1) as f32), (14.0, 3.0), &all);
    cv.ellipse(cx as f32, (foot - 3) as f32, 14.0, 5.0, STONE[3]);
    cv.ellipse(cx as f32, (foot - 3) as f32, 11.0, 3.5, 'x');
    cv.ellipse(cx as f32, (foot - 4) as f32, 8.0, 2.0, if glow { 'e' } else { 's' });
    cv.line(cx - 10, foot - 3, cx - 10, foot - 30, 'K');
    cv.line(cx - 9, foot - 3, cx - 9, foot - 30, 'g');
    cv.line(cx - 10, foot - 30, cx, foot - 34, 'K');
    cv.line(cx, foot - 34, cx + 2, foot - 30, 'K');
    let swing = if glow { ((tick as f32 * 0.08).sin() * 1.5) as i32 } else { 0 };
    bell(cv, cx + 2 + swing, foot - 30, 5, false, glow);
    if glow {
        lights.push(Light { x: cx as f32, y: (foot - 18) as f32, r: 44.0, s: 0.32, fire: false });
    }
}

/// A lamp post's head: a stalagmite stump bearing three drowned candles.
pub(super) fn lamp_head(cv: &mut Img, x: i32, y: i32, on: bool, lights: &mut Vec<Light>) {
    cv.rect(x - 3, y - 22, 8, 3, STONE[3]);
    cv.line(x - 3, y - 22, x + 4, y - 22, STONE[5]);
    candles(cv, x + 1, y - 23, 3, on, lights);
}

/// What moves in the cathedral: drips falling from the stalactites and
/// ringing out on the water, the Great Bell's slow rings once it is awake,
/// mist on the water, and the belfry bells swinging now and then.
pub(super) fn ambient(cv: &mut Img, run: &Run, vl: &Village, lk: &Look) {
    let tick = run.tick as u32;
    let lit = vl.light();
    let face = v::BACK as i32 * TILE;
    // Drips: a bead falling, then a ring going out where it landed.
    for k in 0..12 {
        let h = hash(k, 31, 9830);
        let x = 24 + (h % 528) as i32;
        let land = face + 16 + ((h >> 9) % 200) as i32;
        let life = 90 + h % 60;
        let t = (tick + h % life) % life;
        let y = 20 + (t * t) as i32 * 2 / 9;
        if y < land {
            if y > 20 {
                cv.put(x, y, 'V');
            }
        } else {
            let age = (y - land) as f32 / 8.0;
            ring(cv, (x, land), age * 1.6 + 1.0, 0.4, if age < 3.0 { 'u' } else { 'S' });
        }
    }
    // The Great Bell's rings out over the water.
    if vl.hope >= 1 {
        let (hx, hy, hw, hh) = px_rect(v::HEART);
        let (cx, foot) = (hx + hw / 2, hy + hh);
        // Slow, and fainter the further out: the bell is heard, not seen.
        let period = 120;
        for n in 0..2 {
            let age = ((tick + n * period / 2) % period) as f32;
            let r = 24.0 + age * (0.55 + lit * 0.45);
            let ink = match (age < 40.0, lit > 0.5) {
                (true, true) => lk.glow[2],
                (true, false) | (false, true) => lk.glow[1],
                (false, false) => 'S',
            };
            if age < 100.0 {
                ring(cv, (cx, foot), r, 0.32, ink);
            }
        }
    }
    // Mist low over the water, drifting.
    for k in 0..50 {
        let h = hash(k, 32, 9831);
        let x = ((h % 600) as i32 + (tick as i32 / 4)) % 600 - 12;
        let y = face + 20 + ((h >> 10) % 230) as i32;
        if (h >> 20) % 3 == 0 {
            cv.put(x, y, 'u');
        }
    }
}

/// A ring on the water: a flattened circle of dots, `r` wide.
fn ring(cv: &mut Img, (x, y): (i32, i32), r: f32, squash: f32, ink: char) {
    let steps = (r * 5.0) as i32 + 6;
    for k in 0..steps {
        let a = k as f32 / steps as f32 * std::f32::consts::TAU;
        if hash(k, r as i32, 9832) % 4 == 0 {
            continue;
        }
        cv.put(x + (a.cos() * r) as i32, y + (a.sin() * r * squash) as i32, ink);
    }
}

/// A floor below's drowned bell, silent or rung: a post, a yoke and the
/// bell under it; rung, it gleams.
pub(super) fn sign_lamp(cv: &mut Img, x: i32, head: i32, lit: bool) {
    cv.line(x - 8, head - 1, x + 8, head - 1, 'b');
    cv.line(x - 8, head, x + 8, head, 'n');
    cv.rect(x - 9, head - 3, 3, 4, 'K');
    cv.rect(x + 7, head - 3, 3, 4, 'K');
    bell(cv, x, head + 1, 6, lit, lit);
}

/// A lump of bell-silver on the floor below, humming.
pub(super) fn relic(cv: &mut Img, x: i32, cy: i32) {
    cv.ellipse(x as f32, cy as f32, 4.5, 3.5, 'v');
    cv.ellipse((x - 1) as f32, (cy - 1) as f32, 2.5, 2.0, 'W');
    cv.put(x + 2, cy + 1, 'U');
    cv.put(x - 2, cy - 2, 'w');
}

/// A silver handbell over a door, as promised.
pub(super) fn over_the_door(cv: &mut Img, cx: i32, cy: i32) {
    cv.line(cx, cy - 6, cx, cy - 4, 'K');
    for d in 0..5 {
        let half = 1 + d / 2;
        cv.line(cx - half, cy - 4 + d, cx + half, cy - 4 + d, if d == 0 { 'W' } else { 'v' });
    }
    cv.put(cx - 1, cy - 2, 'w');
}
