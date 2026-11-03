//! The Rootkin's cavern: the root-hall under a tree too big to see the top
//! of. Its back wall is roots, as thick as towers, twisting down out of the
//! dark with moss on their shoulders and gold sap in their seams; roots
//! cross the floor; the houses are knots in them, round doors and round
//! windows lit amber. In the plaza the Heartroot comes up out of the floor
//! with the tree's sap-heart beating in it.

use super::super::super::ink::{Img, hash, vnoise};
use super::super::super::light::Light;
use super::super::super::material as mat;
use super::super::{TILE, fire};
use super::{Look, all, px_rect};
use crate::rules::{
    Run,
    deep::{Village, village as v},
};

/// Bark, dark to light.
const BARK: [char; 7] = ['n', 'b', 'I', 'B', 'p', 'P', 'r'];
/// Moss on a root's shoulder.
const MOSS: [char; 4] = ['E', 'l', 'A', 'M'];

/// The floor: dark earth, leaf litter and moss, and roots across it.
pub(super) fn ground_marks(cv: &mut Img, k: i32, (x, y): (i32, i32)) {
    match k % 5 {
        0 => {
            // A fallen leaf.
            cv.put(x, y, 'L');
            cv.put(x + 1, y, 'A');
            cv.put(x + 1, y - 1, 'N');
        }
        1 => cv.put(x, y, MOSS[(hash(k, 2, 9701) % 4) as usize]),
        _ => cv.put(x, y, if k % 2 == 0 { 'b' } else { 'D' }),
    }
}

/// Roots across the floor, from the back wall out round the plaza: each a
/// raised band of bark lit along its top.
pub(super) fn floor_roots(cv: &mut Img, lights: &mut Vec<Light>) {
    let paths: [&[(i32, i32)]; 4] = [
        &[(60, 112), (70, 170), (40, 230), (30, 300)],
        &[(520, 112), (500, 180), (540, 250), (556, 330)],
        &[(150, 112), (180, 150), (230, 180)],
        &[(430, 112), (400, 150), (350, 182)],
    ];
    for (n, path) in paths.iter().enumerate() {
        for seg in path.windows(2) {
            let ((x0, y0), (x1, y1)) = (seg[0], seg[1]);
            let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
            for s in 0..=steps {
                let (x, y) = (x0 + (x1 - x0) * s / steps, y0 + (y1 - y0) * s / steps);
                let wide = 3 + (vnoise(x as f32 / 11.0, y as f32 / 11.0, 9702 + n as u32) * 4.0) as i32 - n as i32 / 2;
                for o in -wide..=wide {
                    let ink = if o == -wide { 'P' } else if o < -wide / 2 { 'B' } else if o == wide { 'n' } else { 'b' };
                    cv.put(x + o, y + o.abs() / 3, ink);
                }
                // A seam of sap now and then, glowing.
                if s % 37 == 11 {
                    cv.put(x, y + 1, '4');
                    cv.put(x + 1, y + 1, 'a');
                    lights.push(Light { x: x as f32, y: y as f32, r: 18.0, s: 0.18, fire: true });
                }
            }
        }
    }
}

/// The back wall is roots: great twisting columns of bark coming down out
/// of the dark, moss on their shoulders, sap in their seams.
pub(super) fn back(cv: &mut Img, vl: &Village, lights: &mut Vec<Light>) {
    let face = v::BACK as i32 * TILE;
    for k in 0..11 {
        let h = hash(k, 2, 9710);
        let x0 = -10 + k * 56 + (h % 20) as i32;
        let wide = 14 + (h >> 8) as i32 % 12;
        for y in 0..face + 8 {
            let twist = (y as f32 / 23.0 + k as f32).sin() * 7.0 + (y as f32 / 9.0 + k as f32 * 2.0).sin() * 2.0;
            let cx = x0 + twist as i32;
            let half = wide / 2 + (y as f32 / face as f32 * 4.0) as i32;
            for o in -half..=half {
                let u = o as f32 / half as f32;
                let light = (1.0 - (u + 0.35).abs()).clamp(0.0, 1.0);
                let grain = vnoise((cx + o) as f32 / 2.5, y as f32 / 9.0, 9711 + k as u32);
                let mut i = (light * 5.0 + (grain - 0.5) * 2.4) as i32;
                if (y + o * 3 + k * 7).rem_euclid(17) == 0 {
                    i -= 2;
                }
                cv.put(cx + o, y, BARK[i.clamp(0, 6) as usize]);
            }
            // Moss on its shoulder where it bends toward the light.
            if (y + k * 13) % 41 < 9 {
                for o in -half..-half / 3 {
                    if hash(cx + o, y, 9712) % 3 != 0 {
                        cv.put(cx + o, y, MOSS[(hash(o, y, 9713) % 4) as usize]);
                    }
                }
            }
            // A seam of sap down its middle, lit as the tree wakes.
            if (y + k * 31) % 53 < 14 && y > 20 {
                let lit = (k as u8 % 8) < vl.hope;
                cv.put(cx + 1, y, if lit { '4' } else { 'a' });
                if lit && y % 14 == 0 {
                    lights.push(Light { x: (cx + 1) as f32, y: y as f32, r: 22.0, s: 0.22, fire: true });
                }
            }
        }
    }
}

/// A root-knot house: a bole of root on its footprint, its bark grained
/// and grooved, buttress roots flaring from it into the floor, roots
/// arching over it from the wall behind; a round green door, round
/// windows lit amber, moss on its crown with a sprout of leaves, and a
/// sap-lamp by the door.
pub(super) fn house(cv: &mut Img, (x, y, w, h): (i32, i32, i32, i32), seed: u32, lit: bool, lights: &mut Vec<Light>) {
    let cx = x + w / 2;
    let foot = y + h;
    let top = foot - h - 20;
    mat::shadow(cv, ((cx + 8) as f32, (foot + 2) as f32), (w as f32 * 0.55, 4.0), &all);
    // Roots arching over it from the wall behind, down into the floor.
    for side in [-1i32, 1] {
        let (sx, ex) = (cx + side * 4, cx + side * (w / 2 + 8));
        for t in 0..=48 {
            let f = t as f32 / 48.0;
            let px_ = sx as f32 + (ex - sx) as f32 * f;
            let py = (top - 14) as f32 + f * f * (foot - top + 12) as f32 - (1.0 - f) * f * 18.0;
            let half = 4 - (f * 2.5) as i32;
            for o in -half..=half {
                let ink = if o == -half { 'P' } else if o < 0 { 'B' } else if o == half { 'n' } else { 'b' };
                cv.put(px_ as i32 + o, py as i32, ink);
            }
        }
    }
    // The bole: flared at its foot, round at its crown.
    let tall = (foot - top) as f32;
    let half_at = |yy: i32| {
        let t = (foot - yy) as f32 / tall;
        let base = w as f32 / 2.0 * (0.74 + 0.3 * (1.0 - t).powi(3));
        if t > 0.72 { base * (1.0 - ((t - 0.72) / 0.28).powi(2)).max(0.0).sqrt() } else { base }
    };
    for yy in top..foot {
        let half = half_at(yy) as i32;
        for o in -half..=half {
            let u = o as f32 / half.max(1) as f32;
            let z = (1.0 - u * u).max(0.0).sqrt();
            let light = (z * 0.75 - u * 0.4).clamp(0.0, 1.0);
            // Grain running up it, bending round the crown.
            let wob = (vnoise(yy as f32 / 9.0, o as f32 / 30.0, 9723 + seed) * 6.0) as i32;
            let groove = (o * 3 / 2 + wob + seed as i32).rem_euclid(7) == 0;
            let grain = vnoise((cx + o) as f32 / 1.8, yy as f32 / 10.0, 9724 + seed);
            let mut i = (light * 5.4 + (grain - 0.5) * 1.6) as i32;
            if groove {
                i -= 2;
            }
            if u.abs() > 0.9 {
                i -= 1;
            }
            cv.put(cx + o, yy, BARK[i.clamp(0, 6) as usize]);
        }
    }
    // Buttress roots flaring out into the floor.
    for side in [-1, 1] {
        for (k, reach) in [(0, 14), (1, 9)] {
            let y0 = foot - 14 + k * 6;
            let x0 = cx + side * (half_at(y0) as i32 - 2);
            for t in 0..=reach {
                let f = t as f32 / reach as f32;
                let (px_, py) = (x0 + side * t, y0 + (f * f * (foot + 2 - y0) as f32) as i32);
                let half = (3.0 * (1.0 - f)).ceil() as i32;
                for o in 0..=half {
                    cv.put(px_, py - o, if o == half { 'P' } else { 'b' });
                }
            }
        }
    }
    // Moss on its crown, and a sprout of leaves.
    for yy in top..top + 10 {
        let half = half_at(yy) as i32;
        for o in -half..=half / 2 {
            if hash(cx + o, yy, 9721) % 3 != 0 && (yy - top) < 3 + (hash(o, 0, 9725) % 6) as i32 {
                cv.put(cx + o, yy, MOSS[(hash(cx + o, yy, 9722) % 4) as usize]);
            }
        }
    }
    let (sx, sy) = (cx + 4 + (seed % 3) as i32 * 4, top + 1);
    cv.line(sx, sy, sx, sy - 7, 'l');
    for (dx, dy) in [(-4, -6), (4, -8), (-2, -10), (3, -4)] {
        cv.line(sx, sy - 4 + dy / 3, sx + dx, sy + dy, 'A');
        cv.put(sx + dx, sy + dy, 'M');
    }
    // The round door, a green one, with a knot for a handle.
    cv.ellipse(cx as f32, (foot - 11) as f32, 8.0, 9.0, 'n');
    cv.rect(cx - 8, foot - 11, 17, 11, 'n');
    cv.ellipse(cx as f32, (foot - 11) as f32, 6.5, 7.5, 'l');
    cv.rect(cx - 6, foot - 11, 13, 10, 'l');
    cv.line(cx, foot - 18, cx, foot - 2, 'E');
    cv.line(cx - 4, foot - 15, cx - 4, foot - 2, 'A');
    cv.put(cx + 3, foot - 7, 'O');
    // Round windows of four panes, lit amber.
    for wx in [cx - w / 2 + 12, cx + w / 2 - 12] {
        let wy = foot - h + 6;
        cv.ellipse(wx as f32, wy as f32, 5.0, 5.0, 'n');
        cv.ellipse(wx as f32, wy as f32, 3.5, 3.5, if lit { '4' } else { 'k' });
        if lit {
            cv.put(wx - 1, wy - 1, '6');
            cv.put(wx - 2, wy - 1, '5');
            lights.push(fire(wx, wy, 44.0, 0.38));
        }
        cv.line(wx - 4, wy, wx + 4, wy, 'n');
        cv.line(wx, wy - 4, wx, wy + 4, 'n');
    }
    // The sap-lamp by its door.
    let lx = cx + w / 2 - 4;
    cv.line(lx, foot, lx - 2, foot - 22, 'b');
    cv.line(lx - 2, foot - 22, lx + 3, foot - 26, 'b');
    sap_lamp(cv, lx + 3, foot - 26, lit, lights);
}

/// An amber gourd of sap hanging from a hook at `(x, y)`, glowing if lit.
pub(super) fn sap_lamp(cv: &mut Img, x: i32, y: i32, lit: bool, lights: &mut Vec<Light>) {
    cv.line(x, y, x, y + 2, 'n');
    cv.put(x, y + 3, 'b');
    cv.ellipse(x as f32, (y + 7) as f32, 3.0, 4.0, if lit { '4' } else { 'a' });
    if lit {
        cv.ellipse(x as f32, (y + 7) as f32, 1.5, 2.5, '5');
        cv.put(x - 1, y + 5, '6');
        lights.push(fire(x, y + 7, 36.0, 0.36));
    } else {
        cv.put(x - 1, y + 5, 'I');
    }
    cv.put(x, y + 11, if lit { '4' } else { 'a' });
}

/// The Heartroot: the taproot's end coming up out of the plaza floor, its
/// buttress roots spread on the stone, and in its side the tree's
/// sap-heart, dark and hard until hope wakes it, then beating gold.
pub(super) fn heart(cv: &mut Img, vl: &Village, lights: &mut Vec<Light>, tick: u32) {
    let (x, y, w, h) = px_rect(v::HEART);
    let (cx, foot) = (x + w / 2, y + h);
    let awake = vl.hope >= 1;
    let light = vl.light();
    mat::shadow(cv, ((cx + 8) as f32, (foot + 2) as f32), (40.0, 7.0), &all);
    // Buttress roots out across the floor.
    for k in 0..6 {
        let a = std::f32::consts::PI * (0.02 + k as f32 * 0.96 / 5.0);
        let len = 34.0 + (hash(k, 1, 9730) % 10) as f32;
        for t in 0..=len as i32 {
            let f = t as f32 / len;
            let (px_, py) = (cx as f32 + a.cos() * t as f32 * 1.1, (foot - 6) as f32 + a.sin() * t as f32 * 0.3 + f * 4.0);
            let half = (5.0 * (1.0 - f)).max(1.0) as i32;
            for o in -half..=half {
                cv.put(px_ as i32, py as i32 + o / 2, if o < 0 { 'B' } else if o == 0 { 'p' } else { 'b' });
            }
        }
    }
    // The trunk of it, rising and twisting, broken off at its top.
    let top = foot - 74;
    for yy in top..foot - 2 {
        let f = (yy - top) as f32 / (foot - top) as f32;
        let half = (15.0 + f * f * 12.0) as i32;
        let twist = ((yy as f32) / 12.0).sin() * 2.0;
        for o in -half..=half {
            let u = o as f32 / half as f32;
            let light = (1.0 - (u + 0.35).abs()).clamp(0.0, 1.0);
            let grain = vnoise((cx + o) as f32 / 2.2, yy as f32 / 11.0, 9731);
            let mut i = (light * 5.0 + (grain - 0.5) * 2.2) as i32;
            if (o * 2 + yy / 3).rem_euclid(11) == 0 {
                i -= 2;
            }
            cv.put(cx + o + twist as i32, yy, BARK[i.clamp(0, 6) as usize]);
        }
    }
    // Its broken top: jagged, a cup of sap in it.
    for o in -12..=12 {
        let jag = (hash(o, 2, 9732) % 6) as i32;
        for d in 0..jag {
            cv.put(cx + o, top - d, if o < 0 { 'P' } else { 'b' });
        }
    }
    cv.ellipse(cx as f32, (top + 1) as f32, 9.0, 2.5, if awake { '4' } else { 'a' });
    // Moss down its shoulder.
    for yy in top + 4..foot - 20 {
        if hash(yy, 3, 9733) % 3 != 0 {
            cv.put(cx - 12 - ((yy as f32 / 12.0).sin() * 2.0) as i32, yy, MOSS[(hash(yy, 4, 9734) % 4) as usize]);
        }
    }
    // The sap-heart, and the veins of light it sends up and down the root.
    let (hx, hy) = (cx + 2, foot - 36);
    let beat = if awake { ((tick as f32 * 0.12).sin() * 0.5 + 0.5) * (0.4 + light * 0.6) } else { 0.0 };
    let r = 7.0 + beat * 1.0;
    // Its socket in the root: a lip of bark swelling round it.
    cv.ellipse(hx as f32, hy as f32, r + 4.0, r + 5.0, 'b');
    cv.ellipse((hx - 1) as f32, (hy - 1) as f32, r + 3.0, r + 4.0, 'P');
    cv.ellipse(hx as f32, hy as f32, r + 2.0, r + 3.0, 'n');
    // The heart: a knot of amber, faceted, dark at its foot.
    cv.ellipse(hx as f32, hy as f32, r, r + 1.5, if awake { '4' } else { 'a' });
    cv.ellipse(hx as f32, (hy - 1) as f32, r * 0.75, r * 1.0, if awake { '@' } else { '8' });
    cv.ellipse((hx - 1) as f32, (hy - 2) as f32, r * 0.45, r * 0.6, if awake { '5' } else { 'a' });
    for k in -2..=2 {
        cv.put(hx + k * 2, hy + r as i32, if awake { 'a' } else { 'n' });
    }
    if awake {
        cv.ellipse((hx - 2) as f32, (hy - 3) as f32, 2.0, 2.5, '6');
        // Veins of light up and down the root from it, along its grain.
        for k in 0..6 {
            let up = k % 2 == 0;
            let off = [-6, 5, -2, 8, -9, 1][k];
            let reach = (18.0 + light * 30.0) as i32 + (hash(k as i32, 5, 9735) % 8) as i32;
            for s in 0..reach {
                let yy = if up { hy - r as i32 - s } else { hy + r as i32 + s };
                let xx = hx + off + ((yy as f32 / 7.0 + k as f32).sin() * 2.0) as i32;
                if yy < top + 2 || yy > foot - 3 {
                    break;
                }
                if s % 3 != 2 || light > 0.6 {
                    cv.put(xx, yy, if s < 8 { '5' } else { '4' });
                }
            }
        }
        lights.push(fire(hx, hy, 100.0 + light * 60.0, 0.4 + light * 0.3 + beat * 0.1));
    }
}

/// The listening knot: a stump with a knothole you speak your wish into,
/// a little pool of sap at its foot.
pub(super) fn wishing_place(cv: &mut Img, vl: &Village, lights: &mut Vec<Light>, tick: u32) {
    let (x, y, w, h) = px_rect(v::WELL);
    let (cx, foot) = (x + w / 2, y + h);
    let glow = vl.hope >= 2;
    mat::shadow(cv, ((cx + 4) as f32, (foot + 1) as f32), (16.0, 3.5), &all);
    mat::lobe(cv, (cx as f32, (foot - 12) as f32), (14.0, 14.0), &BARK, 9740, 3.0, 0.1, &|_, yy| yy < foot);
    for xx in cx - 12..cx + 10 {
        if hash(xx, 5, 9741) % 2 == 0 {
            cv.put(xx, foot - 24 + (xx - cx).abs() / 4, MOSS[(hash(xx, 6, 9742) % 4) as usize]);
        }
    }
    // The knothole.
    cv.ellipse(cx as f32, (foot - 13) as f32, 5.0, 6.0, 'n');
    cv.ellipse(cx as f32, (foot - 13) as f32, 3.5, 4.5, if glow { 'a' } else { 'k' });
    if glow {
        let pulse = (tick / 10) % 4;
        cv.ellipse(cx as f32, (foot - 12) as f32, 1.5 + pulse as f32 * 0.3, 2.0, '4');
        lights.push(fire(cx, foot - 13, 40.0, 0.32));
    }
    // The sap pool at its foot.
    cv.ellipse((cx + 10) as f32, (foot - 1) as f32, 6.0, 2.0, 'n');
    cv.ellipse((cx + 10) as f32, (foot - 1) as f32, 4.5, 1.2, if glow { '4' } else { 'a' });
}

/// A lamp post's head: a bent root with a sap gourd hanging from it.
pub(super) fn lamp_head(cv: &mut Img, x: i32, y: i32, on: bool, lights: &mut Vec<Light>) {
    cv.line(x + 1, y - 20, x + 5, y - 23, 'b');
    cv.put(x + 2, y - 21, 'A');
    sap_lamp(cv, x + 5, y - 23, on, lights);
}

/// Sap dripping gold from the roots overhead, fireflies, seed-down
/// drifting in the light.
pub(super) fn ambient(cv: &mut Img, run: &Run, vl: &Village, lk: &Look) {
    let tick = run.tick as u32;
    let lit = vl.light();
    // Drips from the roots of the back wall, each falling then splashing.
    for k in 0..(8 + (lit * 10.0) as i32) {
        let h = hash(k, 21, 9750);
        let x = 10 + (h % 556) as i32;
        let from = 40 + ((h >> 9) % 60) as i32;
        let life = 70 + h % 50;
        let t = (tick + h % life) % life;
        let to = v::BACK as i32 * TILE + 4;
        let y = from + (t * t) as i32 / 40;
        if y < to {
            cv.put(x, y, lk.glow[3]);
            cv.put(x, y - 1, lk.glow[2]);
        } else if y < to + 6 {
            cv.put(x - 1, to, lk.glow[2]);
            cv.put(x + 1, to, lk.glow[2]);
        }
    }
    // Fireflies.
    for k in 0..(10 + (lit * 24.0) as i32) {
        let h = hash(k, 22, 9751);
        let (ox, oy) = ((h % 560) as f32, 120.0 + ((h >> 10) % 220) as f32);
        let a = tick as f32 * 0.02 + k as f32;
        let (x, y) = ((ox + a.cos() * 14.0) as i32, (oy + (a * 1.7).sin() * 8.0) as i32);
        if (tick / 4 + h) % 12 < 7 {
            cv.put(x, y, if (tick / 4 + h) % 12 < 2 { '6' } else { '5' });
        }
    }
    // Seed-down drifting.
    for k in 0..14 {
        let h = hash(k, 23, 9752);
        let life = 240 + h % 120;
        let t = (tick + h % life) % life;
        let x = (h % 576) as i32 + (t as f32 * 0.15) as i32 + ((t as f32 * 0.05).sin() * 6.0) as i32;
        let y = 90 + (t as i32 * 260 / life as i32);
        cv.put(x % 576, y, 'T');
    }
}

/// A floor below's sap-lamp: a root post and its gourd, hard and dark, or
/// woken and dripping gold.
pub(super) fn sign_lamp(cv: &mut Img, x: i32, head: i32, lit: bool) {
    cv.line(x, head + 2, x + 6, head - 3, 'b');
    cv.line(x + 1, head + 2, x + 7, head - 3, 'B');
    cv.put(x + 2, head, 'A');
    let (gx, gy) = (x + 7, head - 2);
    cv.line(gx, gy, gx, gy + 3, 'n');
    cv.ellipse(gx as f32, (gy + 9) as f32, 4.5, 6.0, if lit { '4' } else { 'a' });
    if lit {
        cv.ellipse(gx as f32, (gy + 9) as f32, 2.5, 4.0, '5');
        cv.put(gx - 1, gy + 6, '6');
        cv.put(gx, gy + 16, '4');
    } else {
        cv.ellipse(gx as f32, (gy + 9) as f32, 2.5, 3.5, '8');
        cv.put(gx - 2, gy + 6, 'I');
    }
}

/// A heartseed on the floor below: warm, with a green shoot.
pub(super) fn relic(cv: &mut Img, x: i32, cy: i32) {
    cv.ellipse(x as f32, cy as f32, 4.0, 5.0, 'R');
    cv.ellipse((x - 1) as f32, (cy - 1) as f32, 2.5, 3.0, 'o');
    cv.put(x - 2, cy - 3, 'T');
    cv.put(x + 2, cy + 2, '4');
    cv.line(x, cy - 5, x + 1, cy - 9, 'l');
    cv.put(x + 2, cy - 9, 'A');
    cv.put(x - 1, cy - 8, 'A');
}

/// A heartseed planted over a door, already a sapling.
pub(super) fn over_the_door(cv: &mut Img, cx: i32, cy: i32) {
    cv.ellipse(cx as f32, cy as f32, 3.0, 2.0, 'R');
    cv.line(cx, cy - 1, cx, cy - 7, 'l');
    for (dx, dy) in [(-3, -5), (3, -6), (-2, -8), (2, -9)] {
        cv.line(cx, cy - 4, cx + dx, cy + dy, 'A');
    }
    cv.put(cx, cy - 8, 'M');
    cv.put(cx + 1, cy, '4');
}
