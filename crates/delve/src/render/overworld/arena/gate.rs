//! The hell gate and the circles below it. Where a dungeon ends, the floor
//! cracks and glows, two pillars of black stone climb out of it with an arch
//! between them, and the arch fills with flame: the way down into the next
//! circle. Enter one and the circle's name is struck across the screen on
//! a plate of black paper edged in fire, and the rooms of the circle itself
//! are lit from below, redder with each.
//!
//! The gate reads `Run::spectacle.gate` and its own clock; the card reads
//! `Run::circle_card`; the tint reads `Run::circle`.

use super::super::ink::{BLACK, Img, Rgb, bayer, hash, is_signal, nearest_plain, text_width, vnoise};
use super::{NATIVE_H, NATIVE_W, at, enlarge};
use crate::rules::Run;
use crate::rules::spectacle::{circle_name, circle_title};
use std::collections::HashMap;

/// The gate at full height, in pixels.
const W: i32 = 44;
const H: i32 = 58;
const POST: i32 = 9;

fn stone_ink(x: i32, y: i32, edge: i32, wide: i32) -> char {
    let row = (y.rem_euclid(7)) == 0;
    let n = hash(x, y / 7, 3) % 5;
    match edge {
        0 => 'G',
        e if e == wide - 1 => 'g',
        _ if row => 'X',
        _ if n == 0 => 'j',
        _ => 'g',
    }
}

/// A flame field over a rectangle `(x0, y0, w, h)`: the portal.
fn flames(cv: &mut Img, (x0, y0, w, h): (i32, i32, i32, i32), tick: u32, inside: &dyn Fn(i32, i32) -> bool) {
    let t = tick as f32 * 0.42;
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            if !inside(x, y) {
                continue;
            }
            let up = (y0 + h - y) as f32 / h as f32;
            let n = vnoise(x as f32 * 0.19, y as f32 * 0.11 + t, 17) * 0.62
                + vnoise(x as f32 * 0.4, y as f32 * 0.23 + t * 1.6, 29) * 0.38;
            let v = n * 0.85 + (1.0 - up) * 0.5 - 0.1 + (bayer(x, y) - 0.5) * 0.12;
            let ch = if v > 0.92 {
                '5'
            } else if v > 0.74 {
                '@'
            } else if v > 0.55 {
                '7'
            } else if v > 0.36 {
                '8'
            } else {
                'k'
            };
            cv.put(x, y, ch);
        }
    }
}

/// The skull on the keystone.
fn skull() -> Img {
    Img::from_rows(&[".iHHi.", "iHHHHi", "HkHHkH", "iHHHHi", ".ihhi.", "..hh.."])
}

pub(super) fn draw(cv: &mut Img, run: &Run) {
    let Some(gate) = run.spectacle.gate.filter(|g| g.room == run.at) else {
        return;
    };
    let tick = run.tick as u32;
    let r = gate.risen(run.tick);
    let (gx, gy) = at(gate.x, gate.y);
    // Ground glow and cracks, from the first tick.
    let k = (r * 4.0).min(1.0);
    let (rx, ry) = (34.0 * (0.4 + 0.6 * k), 12.0 * (0.4 + 0.6 * k));
    let pulse = 0.85 + 0.15 * (tick as f32 * 0.2).sin();
    for y in gy - ry as i32..=gy + ry as i32 {
        for x in gx - rx as i32..=gx + rx as i32 {
            let d = (((x - gx) as f32 / rx).powi(2) + ((y - gy) as f32 / ry).powi(2)).sqrt();
            if d <= 1.0 && bayer(x, y) < (1.0 - d) * k * pulse * 1.3 {
                let heat = (1.0 - d) * k * pulse;
                cv.put(x, y, if heat > 0.6 { '@' } else if heat > 0.3 { '7' } else { '8' });
            }
        }
    }
    for c in 0..6 {
        let ang = c as f32 * 1.05 + 0.4;
        let len = (18.0 + (hash(c, 2, 5) % 14) as f32) * k;
        let (ex, ey) = (gx as f32 + ang.cos() * len * 1.4, gy as f32 + ang.sin() * len * 0.5);
        cv.line(gx, gy, ex as i32, ey as i32, 'k');
        cv.line(gx, gy, ex as i32, ey as i32 - 1, '7');
    }
    if r < 0.12 {
        return;
    }
    // The gate itself climbs from the floor.
    let eased = ((r - 0.12) / 0.88).clamp(0.0, 1.0);
    let eased = eased * eased * (3.0 - 2.0 * eased);
    let hh = (H as f32 * eased) as i32;
    let top = gy - hh;
    let (left, right) = (gx - W / 2, gx + W / 2);
    let arch_r = W / 2;
    let arch_cy = gy - H + arch_r - (H - hh);
    // Portal first: it sits behind the stone.
    let inner_l = left + POST;
    let inner_r = right - POST;
    let inside = |x: i32, y: i32| {
        if x < inner_l || x >= inner_r || y > gy || y < top {
            return false;
        }
        if y >= arch_cy {
            return true;
        }
        let dx = (x - gx) as f32 / (arch_r - POST) as f32;
        let dy = (y - arch_cy) as f32 / (arch_r as f32 * 1.15);
        dx * dx + dy * dy <= 1.0
    };
    flames(cv, (inner_l, top, inner_r - inner_l, hh + 1), tick, &inside);
    // The posts, up to where the arch springs.
    for x in left..right {
        let edge = if x < left + POST { x - left } else if x >= right - POST { right - 1 - x } else { -1 };
        if edge < 0 {
            continue;
        }
        let stop = arch_cy.max(top);
        for y in stop..gy {
            let mut ch = stone_ink(x, y - top, edge, POST);
            // Lava running in the joints.
            if hash(x, y / 5, 11) % 11 == 0 && (y + tick as i32 / 6) % 3 != 0 {
                ch = '7';
            }
            cv.put(x, y, ch);
        }
    }
    // The arch: a ring of stone round the portal's head.
    for y in top.min(arch_cy - arch_r)..=arch_cy {
        for x in left..right {
            let dx = (x - gx) as f32;
            let dy = (y - arch_cy) as f32;
            let outer = (dx / arch_r as f32).powi(2) + (dy / (arch_r as f32 * 1.15)).powi(2);
            let innerr = (dx / (arch_r - POST) as f32).powi(2) + (dy / (arch_r as f32 * 1.15)).powi(2);
            if outer <= 1.0 && innerr > 1.0 && y >= top {
                let ring = ((outer.sqrt() - 0.78) * 20.0) as i32;
                let mut ch = if ring <= 0 { 'g' } else { stone_ink(x + y, ring, 1, 3) };
                if y < arch_cy - arch_r + 4 {
                    ch = 'G';
                }
                if hash(x / 3, y / 3, 13) % 13 == 0 {
                    ch = '7';
                }
                cv.put(x, y, ch);
            }
        }
    }
    // A black edge down the posts.
    for y in (top - 2)..gy {
        for &x in &[left - 1, right] {
            if y >= arch_cy.max(top) {
                cv.put(x, y, 'k');
            }
        }
    }
    // The keystone's skull, and horns on the posts.
    if eased > 0.9 {
        let s = skull();
        cv.stamp(&s, gx - s.w / 2, top - 1);
        for &px in &[left + 1, right - POST + 1] {
            for h in 0..6 {
                cv.rect(px + h / 3, arch_cy.max(top) - 1 - h, POST - 2 - h / 3 * 2, 1, 'X');
            }
        }
    }
    // Fire licking the posts and the ground before the gate.
    if eased > 0.6 {
        for &px in &[left + POST / 2, right - POST / 2] {
            let base = arch_cy.max(top) - 5;
            for i in 0..9 {
                let flick = (vnoise(px as f32 * 0.5, i as f32 * 0.6 - tick as f32 * 0.5, 41) * 6.0) as i32;
                let h = 7 - i;
                if h > 0 {
                    cv.put(px - 1 + (i % 3) - flick / 3, base - i, if i < 3 { '5' } else if i < 6 { '@' } else { '7' });
                }
            }
        }
        for n in 0..16 {
            let h = hash(n, 9, 31);
            let life = 40 + h % 30;
            let t = (tick + h % 50) % life;
            let x = gx - W / 2 + (h % W as u32) as i32;
            let y = gy - 4 - (t as f32 * 0.95) as i32;
            cv.put(x, y, if t < life / 3 { '5' } else if t < life * 2 / 3 { '@' } else { '7' });
        }
    }
    // Its name, once it stands: the circle it opens.
    if r >= 1.0 {
        let name = circle_name(run.circle() + 1).to_uppercase();
        let sign = super::super::kit::district_sign(&name);
        cv.stamp(&sign, gx - sign.w / 2, top - 16);
    }
}

/// The circle's title card across the middle of the view while it shows.
pub(super) fn card(cv: &mut Img, run: &Run) {
    let Some((circle, k)) = run.circle_card() else {
        return;
    };
    let tick = run.tick as u32;
    // Wipes in from the middle, holds, then dithers away.
    let open = (k / 0.1).min(1.0);
    let alive = if k > 0.78 { 1.0 - (k - 0.78) / 0.22 } else { 1.0 };
    let band_h = 64;
    let top = (NATIVE_H - band_h) / 2;
    let half = (NATIVE_W as f32 / 2.0 * open) as i32;
    let shown = |x: i32, y: i32| (x - NATIVE_W / 2).abs() <= half && bayer(x, y) < alive * 1.05;
    // The plate: black paper, a fire along its top and bottom edges.
    for y in top..top + band_h {
        for x in 0..NATIVE_W {
            if shown(x, y) {
                cv.put(x, y, 'k');
            }
        }
    }
    for edge in [top, top + band_h - 1] {
        for x in 0..NATIVE_W {
            if !shown(x, edge) {
                continue;
            }
            cv.put(x, edge, 'B');
            let up = if edge == top { -1 } else { 1 };
            let n = vnoise(x as f32 * 0.16, tick as f32 * 0.3 + if edge == top { 0.0 } else { 40.0 }, 7);
            let flame = (n * 7.0) as i32;
            for f in 1..=flame {
                let ink = if f * 3 < flame * 2 { '7' } else if f < flame { '@' } else { '5' };
                let ink = if f > 4 && f * 5 > flame * 4 { '8' } else { ink };
                if shown(x, edge + up * f) {
                    cv.put(x, edge + up * f, ink);
                }
            }
        }
    }
    let mut title = Img::new(text_width(&circle_name(circle)) + 2, 8);
    title.text(1, 0, &circle_name(circle).to_uppercase(), 'w');
    let big = enlarge(&title, 3.0);
    let sub_text = circle_title(circle).to_uppercase();
    let mut sub = Img::new(text_width(&sub_text) + 2, 8);
    sub.text(1, 0, &sub_text, 'w');
    let sub = enlarge(&sub, 2.0);
    let heat = |im: &Img, ramp: &[char]| {
        let mut out = Img::new(im.w, im.h);
        for y in 0..im.h {
            for x in 0..im.w {
                if im.get(x, y).is_some() {
                    let k = y as f32 / im.h as f32;
                    out.put(x, y, ramp[((k * ramp.len() as f32) as usize).min(ramp.len() - 1)]);
                }
            }
        }
        out
    };
    let big = heat(&big, &['5', '5', '@', '@', '7']);
    let sub = heat(&sub, &['c', 'c', '$', '$']);
    let (bx, by) = ((NATIVE_W - big.w) / 2, top + 8);
    let (sx, sy) = ((NATIVE_W - sub.w) / 2, top + 8 + big.h + 7);
    for (im, ox, oy) in [(&big, bx, by), (&sub, sx, sy)] {
        // A dark edge first, so the letters stand off the flames.
        let mut shadow = im.clone();
        shadow.outline_outside('8');
        for y in 0..shadow.h {
            for x in 0..shadow.w {
                if let Some(c) = shadow.get(x, y)
                    && shown(ox + x, oy + y)
                {
                    cv.set(ox + x, oy + y, c);
                }
            }
        }
    }
}

/// Rooms of a circle lit from below: every colour but black paper pulled
/// toward ember, a little more with each circle, kept on the palette.
pub(super) fn tint(scenery: &mut Img, circle: u32) {
    let strength = (0.30 + 0.07 * circle.min(6) as f32).min(0.72);
    let mut seen: HashMap<Rgb, Rgb> = HashMap::new();
    for y in 0..scenery.h {
        for x in 0..scenery.w {
            let Some(c) = scenery.get(x, y) else {
                continue;
            };
            if c == BLACK {
                continue;
            }
            let to = *seen.entry(c).or_insert_with(|| {
                if is_signal(c) {
                    return c;
                }
                let l = (299.0 * f32::from(c[0]) + 587.0 * f32::from(c[1]) + 114.0 * f32::from(c[2])) / 1000.0;
                let ember = [l * 1.2 + 16.0, l * 0.46, l * 0.3];
                let mix = |a: u8, b: f32| f32::from(a) + (b - f32::from(a)) * strength;
                nearest_plain([mix(c[0], ember[0]), mix(c[1], ember[1]), mix(c[2], ember[2])])
            });
            scenery.set(x, y, to);
        }
    }
}
