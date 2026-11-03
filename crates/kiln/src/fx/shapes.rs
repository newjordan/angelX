//! The shapes: every layer that is not an emitter, each a field of heat
//! worked out for one frame from how far through its window it is.

use super::Plan;
use super::pen::{Mode, Pen, bounds, ellipse_gap, seg, tone};
use super::recipe::Kind;
use super::rng::{bayer, hash, noise, unit};
use super::time::Ease;
use crate::palette::Palette;
use std::f32::consts::{FRAC_PI_2, TAU};

/// The characters `text` draws.
pub const FONT_CHARS: &str = "0123456789+-! ";

/// Three by five, a row a string.
const FONT: [(char, [&str; 5]); 14] = [
    ('0', ["111", "101", "101", "101", "111"]),
    ('1', ["010", "110", "010", "010", "111"]),
    ('2', ["111", "001", "111", "100", "111"]),
    ('3', ["111", "001", "111", "001", "111"]),
    ('4', ["101", "101", "111", "001", "001"]),
    ('5', ["111", "100", "111", "001", "111"]),
    ('6', ["111", "100", "111", "101", "111"]),
    ('7', ["111", "001", "010", "010", "010"]),
    ('8', ["111", "101", "111", "101", "111"]),
    ('9', ["111", "101", "111", "001", "111"]),
    ('+', ["000", "010", "111", "010", "000"]),
    ('-', ["000", "000", "111", "000", "000"]),
    ('!', ["010", "010", "010", "000", "010"]),
    (' ', ["000", "000", "000", "000", "000"]),
];

/// A stroke between two points of a rune's three by three grid.
type Stroke = ((u8, u8), (u8, u8));

/// A thrust's run: its head, its tail, its width and its heat.
type Run = ((f32, f32), (f32, f32), f32, f32);

/// The marks round a rune circle's band, a few strokes each.
const RUNES: [&[Stroke]; 8] = [
    &[((0, 0), (2, 0)), ((1, 0), (1, 2))],
    &[((0, 0), (2, 2)), ((2, 0), (0, 2))],
    &[((1, 0), (1, 2)), ((0, 1), (2, 1))],
    &[((0, 0), (0, 2)), ((0, 2), (2, 2)), ((0, 0), (1, 0))],
    &[((2, 0), (1, 0)), ((1, 0), (1, 2)), ((1, 2), (0, 2))],
    &[((0, 0), (0, 2)), ((0, 1), (2, 1)), ((2, 1), (2, 2))],
    &[((1, 0), (0, 1)), ((0, 1), (1, 2)), ((1, 2), (2, 1)), ((2, 1), (1, 0))],
    &[((0, 0), (2, 0)), ((0, 0), (0, 2)), ((2, 0), (2, 2))],
];

/// Draw shape layer `p` at frame `t`.
pub(crate) fn draw(pen: &mut Pen, p: &Plan, t: f32) {
    let k = p.k(t);
    let e = p.ease.at(k);
    let h = p.heat_at(k);
    if h <= 0.0 {
        return;
    }
    let (cx, cy) = p.anchor(t);
    let sq = p.squash.max(0.05);
    let seed = p.seed;
    let ramp = &p.ramp;
    match p.kind {
        Kind::Particles | Kind::Puffs => {}
        Kind::Burst => {
            let r = (p.r0 + (p.r1 - p.r0) * e).max(0.3);
            let reach = r * 1.3;
            pen.field((cx - reach, cy - reach * sq, cx + reach, cy + reach * sq), ramp, |x, y| {
                let (dx, dy) = (x - cx, (y - cy) / sq);
                let d = dx.hypot(dy);
                let a = dy.atan2(dx);
                let rough = 1.0 + (noise(a.cos() * 2.2 + 5.0, a.sin() * 2.2 + t * 0.15, seed) - 0.5) * 0.5;
                let re = r * rough;
                if d >= re {
                    return 0.0;
                }
                // Holes open in it as it cools, drifting up.
                let holes = noise(x * 0.45, y * 0.45 + t * 0.3, seed ^ 7);
                ((1.0 - d / re).powf(0.55) * h - k * k * holes * 0.9).max(0.0)
            });
        }
        Kind::Ring | Kind::Shock => {
            let r = p.r0 + (p.r1 - p.r0) * e;
            let th = pen.thin(p.th * (1.0 - 0.6 * k));
            if p.kind == Kind::Shock && k < 0.8 {
                // The wake: the floor darkened just inside the edge.
                let dark = Palette::realm().ink('k');
                let was = pen.mode;
                pen.mode = Mode::Under;
                let (inner, outer) = ((r - th * 3.5).max(0.0), (r - th * 0.6).max(0.0));
                pen.field_ink((cx - r, cy - r * sq, cx + r, cy + r * sq), |x, y, px, py| {
                    let rho = (x - cx).hypot((y - cy) / sq);
                    (rho >= inner && rho <= outer && bayer(px, py) < 0.55 * (1.0 - k)).then_some(dark).flatten()
                });
                pen.mode = was;
            }
            let reach = r + th + 0.5;
            pen.field((cx - reach, cy - reach * sq - 0.5, cx + reach, cy + reach * sq + 0.5), ramp, |x, y| {
                let (dx, dy) = (x - cx, y - cy);
                let g = ellipse_gap(dx, dy, r, sq);
                if g > th {
                    return 0.0;
                }
                let outside = dx.hypot(dy / sq) > r;
                (1.0 - g / th).powf(0.8) * if outside { 1.0 } else { 0.82 } * h
            });
        }
        Kind::Flame => {
            let n = p.count.max(1);
            let w = p.r1.max(0.5);
            let tall = (p.size.0 + (p.size.1 - p.size.0) * e).max(1.0);
            let sp = p.speed.0;
            let tongues: Vec<(f32, f32, f32, f32)> = (0..n)
                .map(|j| {
                    let u = |s: u32| unit(j as i32, seed as i32, s);
                    let xj = cx + w * (((j as f32 + 0.5) / n as f32) * 2.0 - 1.0) * 0.75;
                    let flick = 0.8 + 0.2 * (t * sp * 1.3 + u(1) * TAU).sin() + 0.1 * (t * sp * 2.9 + u(2) * TAU).sin();
                    let centre = 1.0 - 0.45 * ((xj - cx) / w).abs();
                    let hj = tall * (0.55 + 0.45 * u(3)) * flick * centre;
                    let wj = w / n as f32 * 1.6 * (0.85 + 0.3 * u(4)) + w * 0.15;
                    (xj, hj, wj, u(5) * TAU)
                })
                .collect();
            pen.field((cx - w * 1.5, cy - tall * 1.2, cx + w * 1.5, cy + w * 0.5), ramp, |x, y| {
                let mut best = 0.0f32;
                for &(xj, hj, wj, ph) in &tongues {
                    let v = (cy - y) / hj;
                    if !(-0.2..=1.0).contains(&v) {
                        continue;
                    }
                    let sway = (v * 2.4 - t * sp * 0.8 + ph).sin() * w * 0.2 * v.max(0.0);
                    let dx = (x - xj - sway).abs();
                    let half = if v < 0.0 { wj * (1.0 - (v / 0.2).powi(2)).max(0.0).sqrt() } else { wj * (1.0 - v).powf(0.75) };
                    if dx >= half {
                        continue;
                    }
                    let centre = 1.0 - 0.4 * ((xj - cx) / w).abs();
                    best = best.max((1.0 - dx / half).powf(0.45) * (1.0 - 0.75 * v.max(0.0)) * centre);
                }
                best * h * 1.1
            });
        }
        Kind::Beam => {
            let b = p.end.unwrap_or((cx + 32.0, cy));
            let open = Ease::Out.at((k * 4.0).min(1.0));
            let hw = pen.thin(p.th * open * (0.85 + 0.15 * (t * p.speed.0 * 2.0).sin()));
            pen.field(bounds(&[(cx, cy), b], hw + 0.5), ramp, |x, y| {
                let (d, s) = seg((x, y), (cx, cy), b);
                if d > hw {
                    return 0.0;
                }
                let ripple = 0.9 + 0.1 * (s * 24.0 - t * 2.0).sin();
                (1.0 - d / hw).powf(0.55) * ripple * h * 1.05
            });
        }
        Kind::Pillar => {
            let top = cy - p.r1;
            let open = Ease::Back.at((k * 3.0).min(1.0));
            let close = 1.0 - Ease::In.at(((k - 0.6) / 0.4).max(0.0));
            let hw = pen.thin(p.th * open * close * (0.9 + 0.1 * (t * p.speed.0 * 2.0).sin()));
            let base = p.th * 2.4;
            pen.field((cx - base - 0.5, top - 0.5, cx + base + 0.5, cy + base * 0.4 + 0.5), ramp, |x, y| {
                let (dx, dy) = (x - cx, y - cy);
                let mut i = 0.0f32;
                if y <= cy && dx.abs() < hw {
                    let v = (cy - y) / p.r1.max(1.0);
                    let stripes = 0.88 + 0.12 * (y * 1.1 + t * p.speed.0 * 2.5).sin();
                    i = (1.0 - dx.abs() / hw).powf(0.6) * (1.0 - 0.45 * v) * stripes;
                }
                let bd = (dx / base).hypot(dy / (base * 0.38));
                if bd < 1.0 {
                    i = i.max((1.0 - bd).powf(0.8) * 0.9 * open.min(1.0) * close);
                }
                i * h
            });
        }
        Kind::Slash => {
            let (r, w) = (p.r1, p.th);
            let a0 = p.angle.to_radians();
            let arc = p.arc.to_radians();
            let (sign, span) = (if arc < 0.0 { -1.0 } else { 1.0 }, arc.abs().max(1e-3));
            // A smear shows where the blade went over the frame, so its
            // leading edge is where the blade is at the frame's end.
            let k_end = ((t + 1.0 - p.t0) / (p.t1 - p.t0).max(1e-3)).clamp(0.0, 1.0);
            let lead = p.ease.at((k_end * 1.7).min(1.0));
            let tail = Ease::In.at(k);
            if lead - tail < 0.01 {
                return;
            }
            pen.field((cx - r - 1.0, cy - (r + 1.0) * sq, cx + r + 1.0, cy + (r + 1.0) * sq), ramp, |x, y| {
                let (dx, dy) = (x - cx, (y - cy) / sq);
                let rho = dx.hypot(dy);
                if rho > r || rho < r - w - 0.5 {
                    return 0.0;
                }
                let rel = ((dy.atan2(dx) - a0) * sign).rem_euclid(TAU);
                let s = rel / span;
                if s < tail || s > lead {
                    return 0.0;
                }
                // Thick and hot at the leading edge, tapering behind it.
                let along = (s - tail) / (lead - tail);
                let wd = w * along.powf(0.7);
                let edge = (r - rho) / wd.max(1e-3);
                if edge > 1.0 {
                    return 0.0;
                }
                // The rim stays bright all along: the edge of the blade.
                let rim = (1.0 - edge / 0.22).max(0.0) * 0.3 * along.powf(0.3);
                ((1.0 - edge).powf(0.5) * (0.45 + 0.55 * along) + rim) * h * 1.15
            });
        }
        Kind::Orb => {
            let r = ((p.r0 + (p.r1 - p.r0) * e) * (1.0 + 0.08 * (t * p.speed.0 * 1.7).sin())).max(0.3);
            let halo = r * 1.7;
            pen.field((cx - halo, cy - halo, cx + halo, cy + halo), ramp, |x, y| {
                let d = (x - cx).hypot(y - cy);
                let core = if d < r { (1.0 - d / r).powf(0.9) * 1.2 } else { 0.0 };
                core.max(0.26 * (1.0 - d / halo).max(0.0)) * h
            });
        }
        Kind::Flash => {
            let r = (p.r0 + (p.r1 - p.r0) * Ease::Out.at((k * 2.5).min(1.0))).max(0.3);
            pen.field((cx - r, cy - r, cx + r, cy + r), ramp, |x, y| {
                let d = (x - cx).hypot(y - cy);
                if d >= r { 0.0 } else { (1.0 - d / r).powf(0.3) * h }
            });
        }
        Kind::Comet => {
            let head = (cx, cy);
            let dir = match p.end {
                Some(_) => {
                    let b = p.anchor(t - 0.5);
                    let (dx, dy) = (cx - b.0, cy - b.1);
                    let l = dx.hypot(dy);
                    if l > 1e-3 { (dx / l, dy / l) } else { (p.angle.to_radians().cos(), p.angle.to_radians().sin()) }
                }
                None => (p.angle.to_radians().cos(), p.angle.to_radians().sin()),
            };
            let r = p.r1.max(0.3);
            let len = p.size.0;
            let end = (cx - dir.0 * len, cy - dir.1 * len);
            pen.field(bounds(&[head, end], r * 1.6), ramp, |x, y| {
                let d = (x - cx).hypot(y - cy);
                let hd = if d < r * 1.5 { (1.0 - d / (r * 1.5)).powf(0.7) * 1.2 } else { 0.0 };
                let (dt, s) = seg((x, y), head, end);
                let tw = r * (1.0 - s).powf(0.8) * 1.1;
                let ti = if dt < tw {
                    let flick = 0.75 + 0.25 * noise(s * 7.0 - t * 0.9, dt * 2.0, seed);
                    (1.0 - dt / tw).powf(0.6) * (1.0 - s).powf(1.1) * 0.9 * flick
                } else {
                    0.0
                };
                hd.max(ti) * h
            });
        }
        Kind::Vortex => {
            let r = p.r1;
            let rot = (p.spin * t).to_radians();
            let arms = p.count.max(1) as f32;
            pen.field((cx - r, cy - r * sq, cx + r, cy + r * sq), ramp, |x, y| {
                let (dx, dy) = (x - cx, (y - cy) / sq);
                let rho = dx.hypot(dy);
                if rho >= r {
                    return 0.0;
                }
                let spiral = (0.5 + 0.5 * (arms * (dy.atan2(dx) - rot) + rho * 0.9).sin()).powi(2);
                let eye = (rho / (r * 0.18)).min(1.0);
                (1.0 - rho / r).powf(0.5) * (0.2 + 0.8 * spiral) * eye * h
            });
        }
        Kind::Rays => {
            let n = p.count.max(1);
            let len = (p.r1 - p.r0).max(0.5);
            let reach = len * Ease::Back.at((k * 2.2).min(1.0));
            let back = len * Ease::Out.at(((k - 0.25) / 0.75).max(0.0));
            let rot = (p.angle + p.spin * t).to_radians();
            let rays: Vec<((f32, f32), (f32, f32))> = (0..n)
                .filter_map(|j| {
                    let a = rot + j as f32 / n as f32 * TAU;
                    let long = if j % 2 == 0 { 1.0 } else { 0.55 };
                    let (ux, uy) = (a.cos(), a.sin() * sq);
                    let (from, to) = (p.r0 + back * long, p.r0 + reach * long);
                    (to > from + 0.05).then_some(((cx + ux * from, cy + uy * from), (cx + ux * to, cy + uy * to)))
                })
                .collect();
            let th = p.th;
            let floor = pen.px() * 0.55;
            let far = p.r1 * 1.3 + th;
            pen.field((cx - far, cy - far * sq - th, cx + far, cy + far * sq + th), ramp, |x, y| {
                let mut best = 0.0f32;
                for &(a, b) in &rays {
                    let (d, s) = seg((x, y), a, b);
                    let w = (th * (1.0 - 0.7 * s)).max(floor);
                    if d < w {
                        best = best.max((1.0 - d / w).powf(0.6) * (1.0 - 0.5 * s));
                    }
                }
                best * h
            });
        }
        Kind::Glyph => glyph(pen, p, t, e, h),
        Kind::Bolt => bolt(pen, p, t, k, h),
        Kind::Cracks => cracks(pen, p, k, h),
        Kind::Spikes => spikes(pen, p, k, h),
        Kind::Star => {
            let n = p.count.max(2) as f32;
            let r = p.r1 * (0.35 + 0.65 * Ease::Back.at((k * 3.0).min(1.0)));
            let rot = (p.spin * t).to_radians();
            pen.field((cx - r, cy - r, cx + r, cy + r), ramp, |x, y| {
                let (dx, dy) = (x - cx, y - cy);
                let rho = dx.hypot(dy);
                let edge = r * (0.3 + 0.7 * ((dy.atan2(dx) - rot) * n / 2.0).cos().abs().powi(8));
                if rho >= edge { 0.0 } else { (1.0 - rho / edge).powf(0.6) * h }
            });
        }
        Kind::Streaks => streaks(pen, p, k, h),
        Kind::Bubble => {
            let r = p.r1 * Ease::Back.at((k * 3.0).min(1.0));
            let th = pen.thin(p.th);
            pen.field((cx - r - th, cy - (r + th) * sq, cx + r + th, cy + (r + th) * sq), ramp, |x, y| {
                let (dx, dy) = (x - cx, y - cy);
                let g = ellipse_gap(dx, dy, r, sq);
                let (ux, uy) = (dx, dy / sq);
                let rho = ux.hypot(uy);
                let a = uy.atan2(ux).rem_euclid(TAU);
                if (3.5..4.4).contains(&a) && rho > r * 0.66 && rho < r * 0.8 {
                    return h;
                }
                if g <= th {
                    return (0.62 + 0.12 * (a * 3.0 + t * 0.5).sin()) * (1.0 - 0.4 * g / th) * h;
                }
                if rho < r { 0.14 * h } else { 0.0 }
            });
        }
        Kind::Text => text(pen, p, k, h),
        Kind::Tornado => {
            let r = p.r1;
            let tall = p.size.0.max(1.0);
            let spin = p.spin.to_radians();
            pen.field((cx - r * 1.6, cy - tall, cx + r * 1.6, cy + 0.5), ramp, |x, y| {
                let v = (cy - y) / tall;
                if !(0.0..=1.0).contains(&v) {
                    return 0.0;
                }
                let mid = cx + (v * 3.0 + t * 0.3).sin() * r * 0.25;
                let w = r * (0.25 + 0.75 * v);
                let dx = x - mid;
                if dx.abs() >= w {
                    return 0.0;
                }
                let stripe = 0.65 + 0.35 * (y * 1.4 + (dx / w).asin() * 3.0 - t * spin).sin();
                ((w - dx.abs()) / (w * 0.4)).min(1.0) * stripe * 0.85 * h
            });
        }
    }
}

/// A rune circle drawing itself on: an outer ring clockwise, an inner one
/// against it in dashes, a star growing out of its points once it is half
/// open, and runes round its band.
fn glyph(pen: &mut Pen, p: &Plan, t: f32, e: f32, h: f32) {
    let (cx, cy) = p.anchor(t);
    let sq = p.squash.max(0.05);
    let open = e;
    // Its detail comes in as there are pixels for it: at grain 1 a small
    // floor circle is its rings alone, at grain 4 its star and runes too.
    let tall = p.r1 * sq * pen.grain();
    let (inner_ok, star_ok, runes_ok) = (tall >= 4.0, tall >= 6.0, tall >= 8.0);
    let shown = open.clamp(0.0, 1.0);
    let r = p.r1 * open.max(0.4);
    let rot = (p.angle + p.spin * t).to_radians();
    let lw = pen.thin(0.42);
    let n = p.count.max(3);
    let ri = r * 0.72;
    let skip = if n >= 5 { 2 } else { 1 };
    let grow = ((shown - 0.45) / 0.55).clamp(0.0, 1.0);
    let vert = |j: u32| {
        let a = -rot * 1.3 + j as f32 / n as f32 * TAU - FRAC_PI_2;
        (cx + a.cos() * ri, cy + a.sin() * ri * sq)
    };
    let star: Vec<((f32, f32), (f32, f32))> = if grow > 0.0 && star_ok {
        (0..n)
            .map(|j| {
                let (a, b) = (vert(j), vert((j + skip) % n));
                (a, (a.0 + (b.0 - a.0) * grow, a.1 + (b.1 - a.1) * grow))
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut runes: Vec<((f32, f32), (f32, f32))> = Vec::new();
    if shown > 0.3 && p.r1 >= 6.0 && runes_ok {
        let m = (n * 2).max(6);
        let cell = 0.9;
        for j in 0..m {
            let a = rot + j as f32 / m as f32 * TAU;
            let (x, y) = (cx + a.cos() * r * 0.86, cy + a.sin() * r * 0.86 * sq);
            for &((ax, ay), (bx, by)) in RUNES[(hash(j as i32, p.seed as i32, 0x7e) % RUNES.len() as u32) as usize] {
                let at = |gx: u8, gy: u8| (x + (f32::from(gx) - 1.0) * cell, y + (f32::from(gy) - 1.0) * cell);
                runes.push((at(ax, ay), at(bx, by)));
            }
        }
    }
    let reach = r + 2.0;
    pen.field((cx - reach, cy - reach * sq - 2.0, cx + reach, cy + reach * sq + 2.0), &p.ramp, |x, y| {
        let (dx, dy) = (x - cx, y - cy);
        let a = (dy / sq).atan2(dx);
        let mut i = 0.0f32;
        if ellipse_gap(dx, dy, r, sq) <= lw && ((a - rot).rem_euclid(TAU) / TAU) <= shown {
            let tick = (((a - rot).rem_euclid(TAU) / TAU) * 12.0).fract() < 0.06;
            i = if tick { 1.0 } else { 0.8 };
        }
        if inner_ok && ellipse_gap(dx, dy, ri, sq) <= lw && ((-a - rot * 1.3).rem_euclid(TAU) / TAU) <= shown {
            let dash = (((a + rot * 1.3).rem_euclid(TAU) / TAU) * n as f32 * 4.0).fract() < 0.55;
            if dash {
                i = i.max(0.6);
            }
        }
        for &(a, b) in &star {
            if seg((x, y), a, b).0 <= lw {
                i = i.max(0.55);
            }
        }
        for &(a, b) in &runes {
            if seg((x, y), a, b).0 <= lw {
                i = i.max(0.72);
            }
        }
        i * h
    });
}

/// A jagged run from `a` to `b` by midpoint displacement: worst in the
/// middle, pinned at both ends.
fn bolt_points(a: (f32, f32), b: (f32, f32), seed: u32, jag: f32) -> [(f32, f32); 17] {
    let mut pts = [(0.0f32, 0.0f32); 17];
    pts[0] = a;
    pts[16] = b;
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = dx.hypot(dy).max(1e-3);
    let (nx, ny) = (-dy / l, dx / l);
    let (mut step, mut amp) = (16, jag);
    while step > 1 {
        let half = step / 2;
        let mut i = half;
        while i < 16 {
            let (p, q) = (pts[i - half], pts[i + half]);
            let off = (unit(i as i32, seed as i32, step as u32) - 0.5) * 2.0 * amp;
            pts[i] = ((p.0 + q.0) / 2.0 + nx * off, (p.1 + q.1) / 2.0 + ny * off);
            i += step;
        }
        step = half;
        amp *= 0.55;
    }
    pts
}

/// Lightning: a leader running down its path, then the strike, white at
/// its core with a coloured flank, its forks a step cooler, struck again
/// every `flicker` frames and stuttering as it goes.
fn bolt(pen: &mut Pen, p: &Plan, t: f32, k: f32, h: f32) {
    let a = p.at;
    let b = p.end.unwrap_or((a.0, a.1 + 24.0));
    let strike = (t / p.flicker.max(1.0)).floor() as i32;
    let s = hash(strike, p.seed as i32, 0xb0);
    let main = bolt_points(a, b, s, p.jag);
    let len = (b.0 - a.0).hypot(b.1 - a.1);
    // The leader runs down over the first quarter, seen where it has got
    // to by the frame's end.
    let k_end = k + 1.0 / (p.t1 - p.t0).max(1.0);
    let lead = if k_end < 0.25 { ((k_end / 0.25) * 16.0).ceil() as usize } else { 16 };
    let mut forks: Vec<[(f32, f32); 17]> = Vec::new();
    if lead == 16 {
        let (ux, uy) = ((b.0 - a.0) / len.max(1e-3), (b.1 - a.1) / len.max(1e-3));
        for f in 0..p.count {
            let u = |salt: u32| unit(f as i32, s as i32, salt);
            let from = main[4 + (u(1) * 9.0) as usize];
            let turn = (0.45 + 0.45 * u(2)) * if u(3) < 0.5 { -1.0 } else { 1.0 };
            let (c, sn) = (turn.cos(), turn.sin());
            let l = len * (0.22 + 0.22 * u(4));
            let end = (from.0 + (ux * c - uy * sn) * l, from.1 + (ux * sn + uy * c) * l);
            forks.push(bolt_points(from, end, s ^ (f + 1).wrapping_mul(0x51), p.jag * 0.5));
        }
    }
    // Late in its life it stutters: every other strike dims.
    let h = if k > 0.4 && s & 1 == 1 { h * 0.55 } else { h };
    let core = pen.thin(p.th * 0.3);
    let glow = p.th.max(core * 1.5);
    let mut pts: Vec<(f32, f32)> = main.to_vec();
    for f in &forks {
        pts.extend_from_slice(&f[..9]);
    }
    pen.field(bounds(&pts, glow + 0.5), &p.ramp, |x, y| {
        let near = |run: &[(f32, f32)]| run.windows(2).map(|w| seg((x, y), w[0], w[1]).0).fold(f32::MAX, f32::min);
        let dm = near(&main[..=lead.min(16)]);
        let df = forks.iter().map(|f| near(&f[..9])).fold(f32::MAX, f32::min);
        let lit = |d: f32| if d <= core { 1.0 } else if d < glow { 0.55 * (1.0 - d / glow) } else { 0.0 };
        lit(dm).max(lit(df) * 0.8) * h
    });
}

/// Cracks running out over the ground: a dark seam with a lit lip, opening
/// fast and wearing away at the end.
fn cracks(pen: &mut Pen, p: &Plan, k: f32, h: f32) {
    let (cx, cy) = p.at;
    let sq = p.squash.max(0.05);
    let n = p.count.max(1);
    let reach = p.r1 * Ease::Out.at((k * 3.0).min(1.0));
    if reach < 0.3 {
        return;
    }
    let mut runs: Vec<Vec<(f32, f32)>> = Vec::new();
    for j in 0..n {
        let u = |s: u32| unit(j as i32, p.seed as i32, s);
        let a = (p.angle + (j as f32 + 0.5 + (u(1) - 0.5) * 0.6) / n as f32 * p.spread).to_radians();
        let (ux, uy) = (a.cos(), a.sin());
        let (nx, ny) = (-uy, ux);
        let long = reach * (0.65 + 0.35 * u(2));
        let run: Vec<(f32, f32)> = (0..=5)
            .map(|m| {
                let f = m as f32 / 5.0;
                let off = if m == 0 { 0.0 } else { (unit(m, (p.seed ^ j) as i32, 0xc1) - 0.5) * 2.0 * p.jag };
                (cx + (ux * long * f + nx * off), cy + (uy * long * f + ny * off) * sq)
            })
            .collect();
        // A branch off its middle.
        if u(3) < 0.7 {
            let from = run[2 + (u(4) * 2.0) as usize];
            let b = a + if u(5) < 0.5 { -0.7 } else { 0.7 };
            let l = long * 0.4;
            runs.push(vec![from, (from.0 + b.cos() * l * 0.5, from.1 + b.sin() * l * 0.5 * sq), (from.0 + b.cos() * l, from.1 + b.sin() * l * sq)]);
        }
        runs.push(run);
    }
    let dark = Palette::realm().ink('k');
    let th = p.th;
    let floor = pen.px() * 0.55;
    let all: Vec<(f32, f32)> = runs.iter().flatten().copied().collect();
    let ramp = p.ramp.clone();
    pen.field_ink(bounds(&all, th * 1.6 + 0.5), |x, y, px, py| {
        let mut best: Option<(f32, f32, f32)> = None;
        for run in &runs {
            let total = run.len() as f32 - 1.0;
            for (i, w) in run.windows(2).enumerate() {
                let (d, s) = seg((x, y), w[0], w[1]);
                let along = (i as f32 + s) / total;
                if best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, along, w[0].1 + (w[1].1 - w[0].1) * s));
                }
            }
        }
        let (d, along, seam) = best?;
        let w = (th * (1.0 - 0.75 * along)).max(floor);
        // It wears away through the dither as its heat goes.
        if bayer(px, py) > h / 0.6 {
            return None;
        }
        if d <= w * 0.55 {
            dark
        } else if d <= w * 1.3 && y < seam {
            // The lip on the near side of the seam catches the light.
            tone(&ramp, 0.42 - 0.15 * along, px, py)
        } else {
            None
        }
    });
}

/// Spikes shooting up out of the ground from the middle out, overshooting,
/// lit on one face, breaking up through the dither at the end.
fn spikes(pen: &mut Pen, p: &Plan, k: f32, h: f32) {
    let (cx, cy) = p.at;
    let n = p.count.max(1);
    let lean_all = p.angle.to_radians().sin();
    let row: Vec<(f32, f32, f32, f32)> = (0..n)
        .map(|j| {
            let u = |s: u32| unit(j as i32, p.seed as i32, s);
            let off = ((j as f32 + 0.5) / n as f32 * 2.0 - 1.0) * p.r1;
            let xj = cx + off + (u(1) - 0.5) * p.r1 / n as f32 * 0.6;
            let delay = (off / p.r1.max(1e-3)).abs() * 0.15;
            let hj = p.size.0 * (0.6 + 0.4 * u(2)) * (1.0 - 0.35 * (off / p.r1.max(1e-3)).abs()) * Ease::Back.at(((k - delay) * 4.0).clamp(0.0, 1.0));
            let wj = p.th * (0.8 + 0.4 * u(3));
            let lean = (u(4) - 0.5) * wj * 1.5 + off / p.r1.max(1e-3) * wj + lean_all * hj * 0.4;
            (xj, hj, wj, lean)
        })
        .collect();
    let ramp = p.ramp.clone();
    let tall = p.size.0 * 1.3;
    let shatter = ((k - 0.7) / 0.3).clamp(0.0, 1.0);
    pen.field_ink((cx - p.r1 - p.th * 3.0, cy - tall, cx + p.r1 + p.th * 3.0, cy + 0.5), |x, y, px, py| {
        if bayer(px, py) < shatter {
            return None;
        }
        let mut best = 0.0f32;
        for &(xj, hj, wj, lean) in &row {
            if hj < 0.3 {
                continue;
            }
            let v = (cy - y) / hj;
            if !(0.0..=1.0).contains(&v) {
                continue;
            }
            let mid = xj + lean * v;
            let half = wj * (1.0 - v);
            let dx = x - mid;
            if dx.abs() > half {
                continue;
            }
            let i = if v > 0.8 { 0.95 } else if dx < -half * 0.6 { 1.0 } else if dx < 0.0 { 0.8 } else { 0.48 };
            best = best.max(i);
        }
        tone(&ramp, best * h.max(0.5), px, py)
    });
}

/// A thrust: one streak shooting out ahead, thick and hot at its head, its
/// tail catching it up, and thinner speed lines beside it, a little late.
fn streaks(pen: &mut Pen, p: &Plan, k: f32, h: f32) {
    let (cx, cy) = p.at;
    let a = p.angle.to_radians();
    let (ux, uy) = (a.cos(), a.sin());
    let (nx, ny) = (-uy, ux);
    let l = p.r1;
    let mut runs: Vec<Run> = Vec::new();
    // As a smear's, the heads are where they are at the frame's end.
    let step = 1.0 / (p.t1 - p.t0).max(1.0);
    let head = l * Ease::Out.at(((k + step) * 2.0).min(1.0));
    let tail = l * Ease::In.at(k);
    if head - tail > 0.2 {
        runs.push(((cx + ux * head, cy + uy * head), (cx + ux * tail, cy + uy * tail), p.th, 1.0));
    }
    for j in 1..p.count.max(1) {
        let u = |s: u32| unit(j as i32, p.seed as i32, s);
        let side = if j % 2 == 0 { 1.0 } else { -1.0 };
        let o = side * (p.th * 1.2 + (j / 2) as f32 * p.th * 0.75 + u(1) * p.th * 0.4);
        let lag = 0.08 + 0.18 * u(2);
        let sc = 0.45 + 0.35 * u(3);
        let kk = ((k - lag) / (1.0 - lag)).clamp(0.0, 1.0);
        let ke = ((k + step - lag) / (1.0 - lag)).clamp(0.0, 1.0);
        let (hd, tl) = (l * sc * Ease::Out.at((ke * 2.0).min(1.0)), l * sc * Ease::In.at(kk));
        if hd - tl > 0.2 {
            let base = (cx - ux * l * 0.15 + nx * o, cy - uy * l * 0.15 + ny * o);
            runs.push(((base.0 + ux * hd, base.1 + uy * hd), (base.0 + ux * tl, base.1 + uy * tl), p.th * 0.32, 0.62));
        }
    }
    if runs.is_empty() {
        return;
    }
    let pts: Vec<(f32, f32)> = runs.iter().flat_map(|r| [r.0, r.1]).collect();
    let floor = pen.px() * 0.55;
    pen.field(bounds(&pts, p.th + 0.5), &p.ramp, |x, y| {
        let mut best = 0.0f32;
        for &(hd, tl, w, i) in &runs {
            let (d, s) = seg((x, y), hd, tl);
            let w = (w * (1.0 - 0.8 * s)).max(floor);
            if d < w {
                best = best.max((1.0 - d / w).powf(0.6) * (1.0 - 0.6 * s) * i);
            }
        }
        best * h
    });
}

/// Numbers rising and slowing, with a shadow a cell below them.
fn text(pen: &mut Pen, p: &Plan, k: f32, h: f32) {
    let s = p.size.0.max(0.25);
    let chars: Vec<char> = p.text.chars().collect();
    let w = chars.len() as f32 * 4.0 * s - s;
    let (x0, y0) = (p.at.0 - w / 2.0, p.at.1 - 2.5 * s - p.speed.0 * Ease::Out.at(k));
    let bit = |x: f32, y: f32| -> bool {
        let (gx, gy) = (((x - x0) / s).floor(), ((y - y0) / s).floor());
        if gx < 0.0 || !(0.0..5.0).contains(&gy) {
            return false;
        }
        let (ci, cx) = ((gx as usize) / 4, (gx as usize) % 4);
        let Some(c) = chars.get(ci) else { return false };
        cx < 3 && FONT.iter().find(|(f, _)| f == c).is_some_and(|(_, rows)| rows[gy as usize].as_bytes()[cx] == b'1')
    };
    let dark = Palette::realm().ink('k');
    let bbox = (x0 - s, y0 - s, x0 + w + s, y0 + 6.0 * s + s);
    pen.field_ink(bbox, |x, y, _, _| (bit(x, y - s) && !bit(x, y)).then_some(dark).flatten());
    let ramp = p.ramp.clone();
    pen.field_ink(bbox, |x, y, px, py| if bit(x, y) { tone(&ramp, (0.95 - 0.25 * ((y - y0) / (5.0 * s))) * h.max(0.4), px, py) } else { None });
}
