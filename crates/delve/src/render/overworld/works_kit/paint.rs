//! The pen every works piece draws with: earth and smoke, flame, wheels,
//! ropes, posts, scaffolds, sacks. Everything is a pure function of its
//! arguments and the animation step `t` (0 to [`FRAMES`] - 1), and every
//! moving thing is periodic in `FRAMES`, so a working piece loops with no
//! seam. Pixels are palette inks and nothing else.

use std::f32::consts::TAU;

use super::FRAMES;
use crate::render::overworld::ink::{Img, hash, vnoise};
use crate::render::overworld::material;

pub fn phase(t: u32) -> f32 {
    (t % FRAMES) as f32 / FRAMES as f32
}

pub fn turn(t: u32) -> f32 {
    phase(t) * TAU
}

fn round(v: f32) -> i32 {
    v.round() as i32
}

pub fn earth(im: &mut Img, cx: f32, cy: f32, rx: f32, ry: f32, seed: u32) {
    for y in (cy - ry - 2.0).floor() as i32..=(cy + ry + 2.0).ceil() as i32 {
        for x in (cx - rx - 2.0).floor() as i32..=(cx + rx + 2.0).ceil() as i32 {
            let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
            let n = vnoise(x as f32 / 3.0, y as f32 / 2.5, seed);
            let d = dx * dx + dy * dy + (n - 0.5) * 0.45;
            if d < 0.78 {
                let ch = if n > 0.72 {
                    'P'
                } else if n < 0.22 {
                    'b'
                } else if hash(x, y, seed) % 7 == 0 {
                    'B'
                } else {
                    'I'
                };
                im.put(x, y, ch);
            } else if d < 1.0 && (x + y) % 2 == 0 {
                im.put(x, y, 'I');
            }
        }
    }
}

pub fn tufts(im: &mut Img, x0: i32, x1: i32, y: i32, seed: u32) {
    for x in x0..x1 {
        if hash(x, y, seed) % 9 == 0 {
            im.put(x, y, 'E');
            im.put(x, y - 1, 'l');
        }
    }
}

pub fn shade(im: &mut Img, cx: f32, cy: f32, rx: f32, ry: f32) {
    let keep = |_: i32, _: i32| true;
    material::shadow(im, (cx, cy), (rx, ry), &keep);
}

pub fn post(im: &mut Img, x: i32, y0: i32, y1: i32) {
    for y in y0..=y1 {
        im.put(x, y, 'o');
        im.put(x + 1, y, 'B');
    }
}

pub fn beam(im: &mut Img, x0: i32, x1: i32, y: i32) {
    for x in x0..=x1 {
        im.put(x, y, 'O');
        im.put(x, y + 1, 'B');
    }
}

pub fn rope(im: &mut Img, x0: i32, y0: i32, x1: i32, y1: i32, sag: f32, ch: char) {
    let n = ((x1 - x0).abs().max((y1 - y0).abs()) * 2).max(2);
    for k in 0..=n {
        let s = k as f32 / n as f32;
        let x = x0 as f32 + (x1 - x0) as f32 * s;
        let y = y0 as f32 + (y1 - y0) as f32 * s + sag * 4.0 * s * (1.0 - s);
        im.put(round(x), round(y), ch);
    }
}

pub fn ring(im: &mut Img, cx: f32, cy: f32, r: f32, ch: char) {
    let n = ((r * TAU) as i32 * 2).max(12);
    for k in 0..n {
        let a = k as f32 / n as f32 * TAU;
        im.put(round(cx + a.cos() * r), round(cy + a.sin() * r), ch);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn wheel(im: &mut Img, cx: f32, cy: f32, r: f32, a: f32, spokes: u32, paddles: bool, ink: (char, char, char)) {
    let (rim, spoke, hub) = ink;
    for k in 0..spokes {
        let s = a + k as f32 / spokes as f32 * TAU;
        im.line(round(cx), round(cy), round(cx + s.cos() * r), round(cy + s.sin() * r), spoke);
    }
    ring(im, cx, cy, r, rim);
    ring(im, cx, cy, r - 1.0, rim);
    if paddles {
        for k in 0..spokes {
            let s = a + (k as f32 + 0.5) / spokes as f32 * TAU;
            let (px, py) = (cx + s.cos() * (r + 1.0), cy + s.sin() * (r + 1.0));
            im.line(round(px - s.sin() * 1.6), round(py + s.cos() * 1.6), round(px + s.sin() * 1.6), round(py - s.cos() * 1.6), rim);
            im.put(round(px + s.cos()), round(py + s.sin()), spoke);
        }
    }
    im.rect(round(cx) - 1, round(cy) - 1, 3, 3, hub);
}

pub fn smoke(im: &mut Img, x: i32, y: i32, t: u32, rise: i32, n: u32, light: bool) {
    for k in 0..n {
        let age = ((phase(t) * n as f32 + k as f32) % n as f32) / n as f32;
        let py = y as f32 - age * rise as f32;
        let px = x as f32 + (age * 5.0) + (turn(t) * 1.0 + k as f32 * 2.3).sin() * (0.6 + age * 1.5);
        let r = 1.4 + age * 3.2;
        let ramp: &[char] = if light { &['h', 'i', 'H'] } else { &['g', 'j', 'G', 'J'] };
        for yy in (py - r).floor() as i32..=(py + r).ceil() as i32 {
            for xx in (px - r).floor() as i32..=(px + r).ceil() as i32 {
                let (dx, dy) = (xx as f32 + 0.5 - px, yy as f32 + 0.5 - py);
                if dx * dx + dy * dy > r * r {
                    continue;
                }
                let keep = age < 0.45 || (xx + yy + k as i32) % 2 == 0 && age < 0.8 || (xx + 2 * yy + k as i32).rem_euclid(4) == 0;
                if !keep {
                    continue;
                }
                let lit = if dx + dy < 0.0 { 1 } else { 0 };
                let idx = ((1.0 - age) * (ramp.len() as f32 - 1.0)).round() as usize + lit;
                im.put(xx, yy, ramp[idx.min(ramp.len() - 1)]);
            }
        }
    }
}

pub fn flame(im: &mut Img, x: i32, foot: i32, w: i32, t: u32, seed: u32) {
    let h = (w * 3 / 2).max(3) + (hash((t % FRAMES) as i32, seed as i32, 66) % 2) as i32;
    let flick = turn(t);
    for r in 0..h {
        let s = r as f32 / h as f32;
        let width = ((w as f32) * (1.0 - s).powf(0.85)).max(1.0);
        let sway = ((flick * 2.0 + seed as f32 * 0.9 + r as f32 * 0.55).sin() * s * 1.6).round() as i32;
        let half = (width / 2.0).ceil() as i32;
        for dx in -half..=half {
            if width < 2.0 && dx != 0 {
                continue;
            }
            let edge = dx.abs() as f32 > width / 2.0 * 0.55;
            let ch = if s > 0.78 {
                '7'
            } else if edge {
                if s > 0.45 { '4' } else { '@' }
            } else if s < 0.25 {
                '6'
            } else {
                '5'
            };
            im.put(x + dx + sway, foot - r, ch);
        }
    }
}

pub fn mouth(im: &mut Img, x: i32, y: i32, w: i32, h: i32, lit: bool, t: u32) {
    for yy in 0..h {
        for xx in 0..w {
            let corner = yy == 0 && (xx == 0 || xx == w - 1);
            if corner {
                continue;
            }
            let edge = xx == 0 || xx == w - 1 || yy == 0;
            let ch = if !lit {
                if edge { 'X' } else if yy > h - 3 && (xx + yy) % 2 == 0 { 'g' } else { 'k' }
            } else if edge {
                'a'
            } else {
                let f = hash(xx, yy + t as i32 * 3, 61) % 5;
                match (yy * 3 / h, f) {
                    (0, _) => '4',
                    (1, 0) => '4',
                    (1, _) => '@',
                    (_, 0) => '@',
                    (_, 1) => '5',
                    _ => '6',
                }
            };
            im.put(x + xx, y + yy, ch);
        }
    }
}

pub fn embers(im: &mut Img, x: i32, y: i32, w: i32, seed: u32) {
    for k in 0..w {
        if hash(k, seed as i32, 62) % 3 == 0 {
            im.put(x + k, y, '8');
        }
    }
}

pub fn sack(im: &mut Img, x: i32, foot: i32, body: [char; 3]) {
    let rows = [" .BB. ", " .bb. ", ".hhhh.", "hHHHHj", "hHHHHj", "hhHHjj", ".jjjj."];
    for (r, row) in rows.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            let ink = match ch {
                'h' => body[1],
                'H' => body[0],
                'j' => body[2],
                'B' => 'B',
                'b' => 'b',
                _ => continue,
            };
            im.put(x + c as i32, foot - 6 + r as i32, ink);
        }
    }
}

pub fn scaffold(im: &mut Img, xs: &[i32], top: i32, foot: i32, ys: &[i32]) {
    for &x in xs {
        post(im, x, top, foot);
    }
    if let (Some(&a), Some(&b)) = (xs.first(), xs.last()) {
        for &y in ys {
            beam(im, a, b + 1, y);
            for &x in xs {
                im.put(x, y, 't');
            }
        }
    }
}

pub fn ladder(im: &mut Img, x: i32, y0: i32, y1: i32) {
    for y in y0..=y1 {
        let lean = (y - y0) / 8;
        im.put(x + lean, y, 'o');
        im.put(x + lean + 4, y, 'o');
        if (y - y0) % 3 == 0 {
            for k in 1..4 {
                im.put(x + lean + k, y, 'O');
            }
        }
    }
}

pub fn banner(im: &mut Img, x: i32, top: i32, foot: i32, t: u32, field: char, charge: char) {
    for y in top..=foot {
        im.put(x, y, 'o');
    }
    im.put(x, top - 1, 'O');
    for c in 0..8 {
        let wave = ((turn(t) * 1.0 - c as f32 * 0.7).sin() * 1.2).round() as i32;
        for r in 0..6 {
            let taper = if c > 4 { r.min(5 - r) < c - 4 } else { false };
            if !taper {
                im.put(x + 1 + c, top + r + wave, if r == 2 && c % 4 < 2 && c > 1 { charge } else { field });
            }
        }
    }
}

pub fn pile(im: &mut Img, cx: i32, foot: i32, w: i32, h: i32, load: super::Load, seed: u32) {
    super::loads::heap(im, cx, foot, w, h, load, seed);
}

pub fn bucket(im: &mut Img, x: i32, y: i32, fill: char) {
    im.rect(x, y, 5, 1, 'B');
    im.rect(x, y + 1, 5, 3, 'r');
    im.line(x, y + 1, x, y + 3, 'o');
    im.line(x + 4, y + 1, x + 4, y + 3, 'b');
    im.rect(x + 1, y, 3, 1, fill);
    im.put(x + 1, y + 2, 'U');
    im.put(x + 3, y + 2, 'U');
}

pub fn darken(im: &mut Img, x0: i32, y0: i32, w: i32, h: i32) {
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            if let Some(c) = im.get(x, y) {
                im.set(x, y, material::darker(c));
            }
        }
    }
}

pub fn spoil(im: &mut Img, cx: i32, foot: i32, rx: f32, ry: f32, seed: u32) {
    let keep = |_: i32, _: i32| true;
    material::lobe(im, (cx as f32, foot as f32 - ry * 0.6), (rx, ry), &['b', 'B', 'P', 'o', 'O'], seed, 1.5, 0.3, &keep);
}

pub fn bricks(im: &mut Img, x0: i32, y0: i32, w: i32, h: i32, seed: u32) {
    for y in y0..y0 + h {
        let row = (y - y0) / 3;
        let off = if row % 2 == 1 { 3 } else { 0 };
        for x in x0..x0 + w {
            let (lx, ly) = ((x - x0 + off) % 6, (y - y0) % 3);
            let tone = hash((x - x0 + off) / 6, row, seed) % 4;
            let ch = if ly == 2 || lx == 5 {
                'b'
            } else if ly == 0 {
                if tone == 0 { 'o' } else { 'R' }
            } else if tone == 1 {
                'r'
            } else if tone == 2 {
                'p'
            } else {
                'R'
            };
            im.put(x, y, ch);
        }
    }
}

pub fn roof(im: &mut Img, x: i32, y: i32, w: i32, h: i32, ramp: &'static [char], seed: u32) {
    material::roof_hip(im, (x, y, w, h), ramp, seed, (h * 2 / 3).min(w / 5));
}

pub fn lean_roof(im: &mut Img, x: i32, y: i32, w: i32, h: i32, ramp: &'static [char], seed: u32) {
    material::roof(im, (x, y, w, h), ramp, seed);
}

pub fn boards(im: &mut Img, x: i32, y: i32, w: i32, h: i32, seed: u32) {
    material::boards(im, (x, y, w, h), true, seed);
}

pub fn stone(im: &mut Img, x: i32, y: i32, w: i32, h: i32, seed: u32) {
    material::ashlar(im, (x, y, w, h), material::ramp::STONE, seed);
}

pub fn door(im: &mut Img, x: i32, foot: i32, w: i32, h: i32) {
    im.rect(x, foot - h + 1, w, h, 'k');
    im.rect(x + 1, foot - h + 2, w - 2, h - 2, 'K');
    im.line(x - 1, foot - h, x + w, foot - h, 'o');
    im.line(x - 1, foot - h + 1, x - 1, foot, 'r');
    im.line(x + w, foot - h + 1, x + w, foot, 'B');
}

pub fn window(im: &mut Img, x: i32, y: i32, w: i32, h: i32, lit: bool) {
    im.rect(x, y, w, h, 'b');
    im.rect(x + 1, y + 1, w - 2, h - 2, if lit { '5' } else { 'k' });
    if lit {
        im.put(x + 1, y + 1, '6');
    }
}

pub fn chimney(im: &mut Img, x: i32, foot: i32, w: i32, h: i32) {
    for y in foot - h..=foot {
        for dx in 0..w {
            let ch = if y == foot - h {
                'h'
            } else if dx == 0 {
                'J'
            } else if dx == w - 1 {
                'g'
            } else if (y + dx) % 4 == 0 {
                'j'
            } else {
                'G'
            };
            im.put(x + dx, y, ch);
        }
    }
    im.line(x - 1, foot - h - 1, x + w, foot - h - 1, 'i');
}

#[allow(clippy::too_many_arguments)]
pub fn trapezoid(im: &mut Img, y0: i32, y1: i32, (l0, r0): (f32, f32), (l1, r1): (f32, f32), tex: &dyn Fn(i32, i32, i8) -> char) {
    let rows = (y1 - y0).max(1) as f32;
    for y in y0..=y1 {
        let s = (y - y0) as f32 / rows;
        let l = (l0 + (l1 - l0) * s).round() as i32;
        let r = (r0 + (r1 - r0) * s).round() as i32;
        for x in l..=r {
            let side = if x == l { -1 } else if x == r { 1 } else { 0 };
            im.put(x, y, tex(x, y, side));
        }
    }
}

pub fn stream(im: &mut Img, x0: i32, y0: i32, w: i32, h: i32, t: u32, dir: i32, seed: u32) {
    let shift = (t % FRAMES) as i32 * dir;
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            let (u, v) = ((x - x0 + shift).rem_euclid(8), y - y0);
            let n = hash((x - x0 + shift).div_euclid(8), v, seed) % 5;
            let band = (v + (n as i32 % 2)) % 4;
            let ch = match (band, u) {
                (0, 2..=5) => '3',
                (1, 1..=3) if n == 0 => 'z',
                (2, 5..=7) if n < 2 => 'z',
                (3, _) => 'q',
                (_, _) => 'Q',
            };
            im.put(x, y, ch);
        }
    }
    for x in x0..x0 + w {
        im.put(x, y0, 'u');
    }
}

pub fn pond(im: &mut Img, cx: f32, cy: f32, rx: f32, ry: f32, t: u32, seed: u32) {
    for y in (cy - ry - 1.0).floor() as i32..=(cy + ry + 1.0).ceil() as i32 {
        for x in (cx - rx - 1.0).floor() as i32..=(cx + rx + 1.0).ceil() as i32 {
            let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
            let d = dx * dx + dy * dy + (vnoise(x as f32 / 3.0, y as f32 / 2.0, seed) - 0.5) * 0.25;
            if d >= 1.0 {
                continue;
            }
            let ripple = ((x * 3 + y * 5 + t as i32).rem_euclid(FRAMES as i32 * 3)) == 0;
            let ch = if d > 0.82 {
                'u'
            } else if d > 0.55 {
                if ripple { '1' } else { 'S' }
            } else if ripple {
                'z'
            } else if hash(x, y, seed) % 11 == 0 {
                'Q'
            } else {
                'q'
            };
            im.put(x, y, ch);
        }
    }
}

pub fn chips(im: &mut Img, x: i32, y: i32, t: u32, n: u32, ch: char) {
    let s = (t % FRAMES) as f32;
    if s > 5.0 {
        return;
    }
    for k in 0..n {
        let a = -0.4 - (k as f32 / n.max(1) as f32) * 2.4;
        let v = 2.0 + (k % 3) as f32 * 0.9;
        let (px, py) = (x as f32 + a.cos() * v * s * 0.7, y as f32 + a.sin() * v * s * 0.7 + s * s * 0.28);
        im.put(round(px), round(py), ch);
    }
}

pub fn dust(im: &mut Img, x: i32, y: i32, t: u32, ch: char) {
    for k in 0..5 {
        let age = ((t + k * 2) % FRAMES) as f32 / FRAMES as f32;
        if age > 0.85 {
            continue;
        }
        let dx = (k as i32 - 2) as f32 * (1.0 + age * 2.0);
        im.put(round(x as f32 + dx), round(y as f32 - age * 7.0), ch);
    }
}

pub fn blockish(im: &mut Img, x: i32, y: i32, w: i32, h: i32, ramp: &'static [char], seed: u32) {
    material_block(im, x, y, w, h, ramp, seed);
}

fn material_block(im: &mut Img, x: i32, y: i32, w: i32, h: i32, ramp: &'static [char], seed: u32) {
    material::block(im, (x, y, w, h), ramp, false, seed);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Front {
    Stone,
    Board,
    Brick,
}

#[allow(clippy::too_many_arguments)]
pub fn dwelling(im: &mut Img, x: i32, y: i32, w: i32, roof_h: i32, wall_h: i32, front: Front, ramp: &'static [char], seed: u32) -> i32 {
    let wall_y = y + roof_h - 1;
    match front {
        Front::Stone => stone(im, x + 2, wall_y, w - 4, wall_h, seed),
        Front::Board => boards(im, x + 2, wall_y, w - 4, wall_h, seed),
        Front::Brick => bricks(im, x + 2, wall_y, w - 4, wall_h, seed),
    }
    roof(im, x, y, w, roof_h, ramp, seed);
    for k in 0..w - 4 {
        if let Some(c) = im.get(x + 2 + k, wall_y + 1) {
            im.set(x + 2 + k, wall_y + 1, material::darker(c));
        }
    }
    wall_y
}
