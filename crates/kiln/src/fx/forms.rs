//! The forms a mote takes, each a small field of heat about the mote's
//! place, sized in game pixels so it reads at every grain: at grain 1 a
//! spark is a pixel and its streak, at grain 4 a tapered stroke with a hot
//! core. The few forms the game draws as marks at its own size (a note, a
//! heart) are those marks at grain 1, as the game has them, and lit shapes
//! above it.

use super::pen::{Pen, bounds, seg};
use super::recipe::Form;
use super::rng::{hash, unit};
use super::time::bump;
use crate::palette::{Palette, Ramp};
use std::f32::consts::PI;

/// One mote as it is now.
#[derive(Clone, Copy, Debug)]
pub struct Mote {
    pub at: (f32, f32),
    /// Game pixels a frame.
    pub vel: (f32, f32),
    /// How far through its life, 0 to 1.
    pub k: f32,
    /// Its size, game pixels.
    pub size: f32,
    /// Its heat, 0 to 1.
    pub heat: f32,
    /// How far it has turned, radians.
    pub turn: f32,
    pub seed: u32,
    /// The frame, for what flickers.
    pub frame: f32,
}

/// The eighth note the game draws a song's mote as.
const NOTE: [&str; 5] = ["..##", "..#.", "..#.", "###.", "##.."];
/// The game's heart.
const HEART: [&str; 3] = ["#.#", "###", ".#."];

/// Draw `m` as `form` in `ramp`.
pub fn draw(pen: &mut Pen, form: Form, m: &Mote, ramp: &Ramp) {
    let s = m.size.max(0.1);
    let h = m.heat;
    let (cx, cy) = m.at;
    match form {
        Form::Spark => {
            let v = m.vel.0.hypot(m.vel.1);
            let (ux, uy) = if v > 1e-3 { (m.vel.0 / v, m.vel.1 / v) } else { (m.turn.cos(), m.turn.sin()) };
            let len = (v * 1.4).clamp(s * 0.8, s * 6.0);
            let tail = (cx - ux * len, cy - uy * len);
            let r = s * 0.45;
            let b = bounds(&[m.at, tail], pen.thin(r) + 0.5);
            let thin = pen.px() * 0.55;
            pen.field(b, ramp, |x, y| {
                let (d, t) = seg((x, y), m.at, tail);
                let rr = (r * (1.0 - 0.6 * t)).max(thin);
                if d > rr { 0.0 } else { (0.4 + 0.6 * (1.0 - d / rr)) * (1.0 - 0.6 * t) * h }
            });
        }
        Form::Ember | Form::Mote => {
            let ember = form == Form::Ember;
            let r = pen.thin(s * if ember { 0.45 } else { 0.6 });
            let flick = if ember { 0.7 + 0.3 * unit(m.seed as i32, m.frame as i32, 0xe1) } else { 1.0 };
            // A little glow about it, sparse, where there is room for one.
            let glow = if pen.grain() >= 2.0 { r * 2.2 } else { r };
            pen.field((cx - glow, cy - glow, cx + glow, cy + glow), ramp, |x, y| {
                let d = (x - cx).hypot(y - cy);
                let core = if d < r { (1.0 - d / r).powf(if ember { 0.5 } else { 0.9 }) } else { 0.0 };
                let halo = if glow > r { 0.22 * (1.0 - d / glow).max(0.0) } else { 0.0 };
                core.max(halo) * h * flick
            });
        }
        Form::Droplet => {
            let v = m.vel.0.hypot(m.vel.1);
            let (ux, uy) = if v > 1e-3 { (m.vel.0 / v, m.vel.1 / v) } else { (0.0, 1.0) };
            let r = pen.thin(s * 0.5);
            let tail = (cx - ux * r * 2.4, cy - uy * r * 2.4);
            let spot = (cx - r * 0.35, cy - r * 0.35);
            let thin = pen.px() * 0.55;
            pen.field(bounds(&[m.at, tail], r + 0.5), ramp, |x, y| {
                let (d, t) = seg((x, y), m.at, tail);
                let rr = (r * (1.0 - t * 0.85)).max(thin);
                if d > rr {
                    return 0.0;
                }
                let lit = if (x - spot.0).hypot(y - spot.1) < r * 0.4 { 1.0 } else if d > rr * 0.7 { 0.4 } else { 0.6 };
                lit * h.max(0.45)
            });
        }
        Form::Shard => {
            let (l, w) = (s * 1.6, pen.thin(s * 0.5));
            let (c, sn) = (m.turn.cos(), m.turn.sin());
            pen.field((cx - l - 0.5, cy - l - 0.5, cx + l + 0.5, cy + l + 0.5), ramp, |x, y| {
                let (dx, dy) = (x - cx, y - cy);
                let (u, v) = (dx * c + dy * sn, -dx * sn + dy * c);
                let e = u.abs() / l + v.abs() / w;
                if e > 1.0 {
                    return 0.0;
                }
                // Two facets, the near one to the light, and a glint at the tip.
                let lit = if u > l * 0.55 { 1.0 } else if v < 0.0 { 0.85 } else { 0.5 };
                lit * h.max(0.4)
            });
        }
        Form::Flake => {
            let r = pen.thin(s * 0.16);
            let arms: Vec<((f32, f32), (f32, f32))> = (0..3)
                .map(|j| {
                    let a = m.turn + j as f32 * PI / 3.0;
                    let (dx, dy) = (a.cos() * s, a.sin() * s);
                    ((cx - dx, cy - dy), (cx + dx, cy + dy))
                })
                .collect();
            pen.field((cx - s - 0.5, cy - s - 0.5, cx + s + 0.5, cy + s + 0.5), ramp, |x, y| {
                let d = arms.iter().map(|&(a, b)| seg((x, y), a, b).0).fold(f32::MAX, f32::min);
                let core = (x - cx).hypot(y - cy) < r * 1.6;
                if core { h } else if d <= r { 0.75 * h } else { 0.0 }
            });
        }
        Form::Bubble => {
            let rr = s * 0.9;
            let th = pen.thin(s * 0.16);
            let spot = (cx - rr * 0.45, cy - rr * 0.45);
            let pop = m.k > 0.9;
            pen.field((cx - rr * 1.4, cy - rr * 1.4, cx + rr * 1.4, cy + rr * 1.4), ramp, |x, y| {
                let (dx, dy) = (x - cx, y - cy);
                let d = dx.hypot(dy);
                if pop {
                    // It bursts into four drops flung out.
                    let a = dy.atan2(dx).rem_euclid(PI / 2.0) - PI / 4.0;
                    return if (d - rr * 1.25).abs() < th && a.abs() < 0.3 { 0.7 * h } else { 0.0 };
                }
                if (x - spot.0).hypot(y - spot.1) < pen_thin_of(th, rr * 0.22) {
                    return h.max(0.5);
                }
                let e = (d - rr).abs();
                if e <= th { 0.65 * (1.0 - 0.4 * e / th) * h.max(0.4) } else if d < rr { 0.1 } else { 0.0 }
            });
        }
        Form::Note => {
            if pen.grain() < 2.0 && s <= 2.0 {
                mark(pen, &NOTE, (cx - 2.0, cy - 4.0), ramp, h);
                return;
            }
            let head = (cx, cy);
            let (rx, ry) = (s * 0.62, s * 0.46);
            let stem = pen.thin(s * 0.13);
            let top = (cx + rx * 0.8, cy - s * 2.0);
            let flag = (cx + rx * 0.8 + s * 0.75, cy - s * 1.25);
            let tilt: f32 = -0.35;
            pen.field((cx - s, cy - s * 2.3, cx + s * 1.8, cy + s), ramp, |x, y| {
                let (dx, dy) = (x - head.0, y - head.1);
                let (u, v) = (dx * tilt.cos() + dy * tilt.sin(), -dx * tilt.sin() + dy * tilt.cos());
                let e = (u / rx).powi(2) + (v / ry).powi(2);
                if e <= 1.0 {
                    return if v < -ry * 0.3 && u < 0.0 { h } else { 0.78 * h };
                }
                let ds = seg((x, y), (top.0, cy), top).0;
                let df = seg((x, y), top, flag).0;
                if ds <= stem || df <= stem * 1.2 { 0.65 * h } else { 0.0 }
            });
        }
        Form::Star => {
            let p = 0.3 + 0.7 * bump(m.k);
            let l = s * (0.9 + 1.6 * p);
            let w = pen.thin(s * 0.28);
            let rot = m.turn * 0.2;
            let (c, sn) = (rot.cos(), rot.sin());
            let diag = pen.grain() * s >= 3.0;
            pen.field((cx - l - 0.5, cy - l - 0.5, cx + l + 0.5, cy + l + 0.5), ramp, |x, y| {
                let (dx, dy) = (x - cx, y - cy);
                let (u, v) = (dx * c + dy * sn, -dx * sn + dy * c);
                let arm = |a: f32, b: f32, len: f32| 1.0 - (a.abs() / len + b.abs() / w);
                let mut i = arm(u, v, l).max(arm(v, u, l));
                if diag {
                    let (p, q) = ((u + v) * std::f32::consts::FRAC_1_SQRT_2, (u - v) * std::f32::consts::FRAC_1_SQRT_2);
                    i = i.max(arm(p, q, l * 0.45) * 0.8).max(arm(q, p, l * 0.45) * 0.8);
                }
                if i <= 0.0 { 0.0 } else { (0.35 + 0.65 * i) * h }
            });
        }
        Form::Petal => {
            let a = m.turn + (m.frame * 0.7 + (m.seed % 97) as f32).sin() * 0.9;
            let (rx, ry) = (s * 0.7, pen.thin(s * 0.34));
            let (c, sn) = (a.cos(), a.sin());
            pen.field((cx - rx - 0.5, cy - rx - 0.5, cx + rx + 0.5, cy + rx + 0.5), ramp, |x, y| {
                let (dx, dy) = (x - cx, y - cy);
                let (u, v) = (dx * c + dy * sn, -dx * sn + dy * c);
                if (u / rx).powi(2) + (v / ry).powi(2) > 1.0 { 0.0 } else { (0.6 + 0.3 * u / rx) * h.max(0.45) }
            });
        }
        Form::Coin => {
            let face = m.turn.cos().abs();
            let ry = s * 0.8;
            let rx = pen.thin(ry * face);
            let rim = pen.thin(s * 0.2);
            pen.field((cx - ry - 0.5, cy - ry - 0.5, cx + ry + 0.5, cy + ry + 0.5), ramp, |x, y| {
                let (dx, dy) = (x - cx, y - cy);
                let e = (dx / rx).powi(2) + (dy / ry).powi(2);
                if e > 1.0 {
                    return 0.0;
                }
                let edge = (1.0 - e.sqrt()) * ry < rim;
                let glint = face > 0.8 && dx < 0.0 && dy < -ry * 0.25;
                (if glint { 1.0 } else if edge { 0.5 } else { 0.78 }) * h.max(0.5)
            });
        }
        Form::Heart => {
            if pen.grain() < 2.0 && s <= 2.0 {
                mark(pen, &HEART, (cx - 1.0, cy - 1.0), ramp, h);
                return;
            }
            let r = s * 0.55;
            let (l, rt) = ((cx - s * 0.45, cy - s * 0.2), (cx + s * 0.45, cy - s * 0.2));
            let spot = (cx - s * 0.5, cy - s * 0.35);
            pen.field((cx - s - 0.5, cy - s - 0.5, cx + s + 0.5, cy + s * 1.1), ramp, |x, y| {
                let lobe = (x - l.0).hypot(y - l.1) <= r || (x - rt.0).hypot(y - rt.1) <= r;
                let dy = (y - (cy - s * 0.2)) / (s * 1.1);
                let point = (0.0..=1.0).contains(&dy) && (x - cx).abs() <= s * 0.98 * (1.0 - dy);
                if !(lobe || point) {
                    return 0.0;
                }
                (if (x - spot.0).hypot(y - spot.1) < s * 0.22 { 1.0 } else if dy > 0.55 { 0.55 } else { 0.78 }) * h.max(0.5)
            });
        }
        Form::Worm => {
            let v = m.vel.0.hypot(m.vel.1);
            let (ux, uy) = if v > 0.05 { (m.vel.0 / v, m.vel.1 / v) } else { (m.turn.cos(), m.turn.sin()) };
            let n = 4 + s.round().max(0.0) as i32;
            let gap = s * 0.62;
            let seg_at = |j: i32| {
                let w = (j as f32 * 1.7 + m.frame * 1.3 + (m.seed % 61) as f32).sin() * s * 0.5;
                (cx - ux * gap * j as f32 - uy * w, cy - uy * gap * j as f32 + ux * w)
            };
            let pts: Vec<(f32, f32)> = (0..n).map(seg_at).collect();
            let b = bounds(&pts, s);
            let rs: Vec<f32> = (0..n).map(|j| pen.thin(s * 0.5 * (1.0 - 0.07 * j as f32))).collect();
            pen.field(b, ramp, |x, y| {
                let mut best = 0.0f32;
                for (j, p) in pts.iter().enumerate() {
                    let d = (x - p.0).hypot(y - p.1);
                    if d <= rs[j] {
                        // A head, then rings of body, light and dark by turns.
                        let i = if j == 0 { 0.85 } else if j % 2 == 0 { 0.62 } else { 0.5 };
                        best = best.max(i * (1.0 - 0.25 * d / rs[j]));
                    }
                }
                best * h.max(0.5)
            });
        }
        Form::Smoke => {
            let r = s * (0.7 + 1.5 * (1.0 - (1.0 - m.k).powi(2)));
            pen.field((cx - r, cy - r, cx + r, cy + r), ramp, |x, y| {
                let (dx, dy) = (x - cx, y - cy);
                let d = dx.hypot(dy) / r;
                if d > 1.0 {
                    return 0.0;
                }
                // Lit from above: its crown paler, its belly darker.
                ((0.62 - 0.4 * m.k) * (1.0 - 0.45 * d * d) - 0.14 * dy / r) * h
            });
        }
        Form::Fly => {
            let p = Palette::realm();
            let jx = (hash(m.seed as i32, m.frame as i32, 0xf1) % 3) as f32 - 1.0;
            let jy = (hash(m.seed as i32, m.frame as i32, 0xf2) % 3) as f32 - 1.0;
            let (fx, fy) = (cx + jx * s * 0.5, cy + jy * s * 0.5);
            let body = pen.thin(s * 0.35);
            let wing = pen.thin(s * 0.3);
            let (dark, pale) = (p.ink('K'), p.ink(if (m.frame as i32) % 2 == 0 { 'V' } else { 'h' }));
            let (wl, wr) = ((fx - s * 0.5, fy - s * 0.5), (fx + s * 0.5, fy - s * 0.5));
            pen.field_ink((fx - s, fy - s, fx + s, fy + s), |x, y, _, _| {
                if (x - fx).hypot(y - fy) <= body {
                    dark
                } else if (x - wl.0).hypot(y - wl.1) <= wing || (x - wr.0).hypot(y - wr.1) <= wing {
                    pale
                } else {
                    None
                }
            });
        }
    }
}

fn pen_thin_of(floor: f32, r: f32) -> f32 {
    r.max(floor * 0.9)
}

/// A mark drawn at the game's own size: `#` an ink, its first row the
/// hottest; `at` is its top left in game pixels.
fn mark(pen: &mut Pen, rows: &[&str], at: (f32, f32), ramp: &Ramp, h: f32) {
    let w = rows.iter().map(|r| r.len()).max().unwrap_or(0) as f32;
    let (x0, y0) = (at.0.round(), at.1.round());
    pen.field((x0, y0, x0 + w - 0.01, y0 + rows.len() as f32 - 0.01), ramp, |x, y| {
        let (c, r) = ((x - x0).floor() as usize, (y - y0).floor() as usize);
        match rows.get(r).and_then(|row| row.as_bytes().get(c)) {
            Some(b'#') => (if r == 0 { 1.0 } else { 0.75 }) * h.max(0.4),
            _ => 0.0,
        }
    });
}
