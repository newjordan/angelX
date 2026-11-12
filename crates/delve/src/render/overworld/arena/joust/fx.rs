//! The lists' loud little moves, in the realm's inks and an ordered dither:
//! the dust each hoofbeat kicks up, speed lines streaming behind a charging
//! horse, a lance grinding on a lance, the crack of a hit (a star, rays, a
//! ring, sparks) and the splinters of a lance that breaks, a rider's
//! landing in the sand, and the favour flying from the gallery.
//!
//! All of it is a pure function of an age in ticks and a seed, so the host,
//! a friend's mirror and a browser guest draw the same pixels.

use super::super::super::ink::{Img, bayer, hash};
use super::super::juice;
use super::super::pfx::{self, Pen, Ramp, Shape, Spray, ramps};
use crate::motion::{self, ease};
use crate::rules::juice::Weight;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Sand thrown up: warm browns down to the dark of the lists.
pub(super) const DUST: Ramp = Ramp(['O', 'o', 'P', 'I', 'b', 'n']);
/// Splinters of ash timber.
const WOOD: [char; 4] = ['T', 'O', 'o', 'T'];

fn unit(i: u32, seed: u32, s: u32) -> f32 {
    (hash(i as i32, seed as i32, s) & 0xffff) as f32 / 65536.0
}

/// A puff of dust kicked up at `at`, `age` ticks old, drifting back (`dir`
/// is the way the horse runs).
pub(super) fn puff(im: &mut Img, at: (f32, f32), dir: i32, age: u32, seed: u32, n: u32, tick: u64) {
    let mut pen = Pen::new(im, 320);
    let spray = Spray {
        angle: -FRAC_PI_2 - dir as f32 * 0.7,
        spread: 1.5,
        gravity: -0.012,
        squash: 0.8,
        drag: 0.16,
        ..Spray::round(seed, n, at, (0.6, 1.5), (9.0, 16.0))
    };
    spray.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Smoke, DUST, 3, tick));
}

/// The dust of the last few hoofbeats behind a rider at the gallop. `beats`
/// are `(age, x)` of each beat still in the air.
pub(super) fn hoofbeats(im: &mut Img, beats: &[(u32, f32)], ground: f32, dir: i32, v: f32, seed: u32, tick: u64) {
    if v < 0.12 {
        return;
    }
    let n = 5 + (v * 8.0) as u32;
    for (i, &(age, x)) in beats.iter().enumerate() {
        puff(im, (x - dir as f32 * 18.0, ground - 2.0), dir, age, seed.wrapping_add(i as u32 * 7), n, tick);
    }
}

/// Speed lines trailing a horse at `v` (0 to 1) of full speed: streaks of
/// wind behind it along its flanks, longer and more the faster it goes.
pub(super) fn speed_lines(im: &mut Img, rear: i32, ground: i32, dir: i32, v: f32, tick: u32, seed: u32) {
    if v < 0.25 {
        return;
    }
    let n = 5 + (v * 11.0) as u32;
    for i in 0..n {
        let u = |s: u32| unit(i, seed, s);
        let y = ground - 6 - (u(1) * 50.0) as i32;
        let len = (8.0 + v * 64.0 * (0.45 + u(2) * 0.8)) as i32;
        // Each streak slides along and wraps, so the lines flicker past.
        let slide = ((tick as f32 * (4.0 + u(3) * 6.0)) as i32 + (u(4) * 48.0) as i32) % 36;
        let head = rear - dir * (slide + (u(5) * 12.0) as i32);
        let thick = u(6) > 0.6 && v > 0.6;
        for k in 0..len {
            let x = head - dir * k;
            let t = k as f32 / len as f32;
            if bayer(x, y) < t * 0.85 {
                continue;
            }
            let ink = if k < 3 { 'i' } else if t < 0.4 { 'h' } else if t < 0.75 { 'J' } else { 'G' };
            im.put(x, y, ink);
            if thick && t < 0.6 {
                im.put(x, y + 1, 'G');
            }
        }
    }
}

/// A dithered rim of `ink` along the edges of the canvas, `k` (0 to 1) how
/// strong: the same shape as the kit's vignette, but only the pixels of the
/// rim are visited, in the order they are taken.
pub(super) fn rim(im: &mut Img, k: f32, ink: char) {
    use std::sync::OnceLock;
    const DEPTH: i32 = 26;
    static RIM: OnceLock<Vec<(i16, i16, f32)>> = OnceLock::new();
    if k <= 0.0 {
        return;
    }
    let rim = RIM.get_or_init(|| {
        let mut v = Vec::new();
        for y in 0..224 {
            for x in 0..384 {
                let edge = x.min(383 - x).min(y).min(223 - y);
                if edge >= DEPTH {
                    continue;
                }
                let near = 1.0 - edge as f32 / DEPTH as f32;
                let ratio = bayer(x, y) / (near * near);
                if ratio < 1.0 {
                    v.push((x as i16, y as i16, ratio));
                }
            }
        }
        v.sort_by(|a, b| a.2.total_cmp(&b.2));
        v
    });
    for &(x, y, r) in rim {
        if r >= k {
            break;
        }
        im.put(i32::from(x), i32::from(y), ink);
    }
}

/// The streaks of wind across the sand at full tilt: thin lines racing
/// along the lanes, low, behind everything.
pub(super) fn wind(im: &mut Img, v: f32, tick: u32) {
    if v < 0.5 {
        return;
    }
    for i in 0..16 {
        let u = |s: u32| unit(i, 5, s);
        let y = 92 + (u(1) * 96.0) as i32;
        let x0 = ((tick as f32 * (11.0 + u(2) * 12.0)) as i32 + (u(3) * 384.0) as i32) % 460 - 50;
        let len = 18 + (u(4) * 40.0 * v) as i32;
        for k in 0..len {
            if bayer(x0 + k, y) < 0.3 {
                continue;
            }
            im.put(x0 + k, y, if k > len - 5 { 'J' } else if k > len - 14 { 'G' } else { 'j' });
        }
    }
}

/// Two lances grinding on each other at the clash: sparks flicking off
/// where they cross.
pub(super) fn grind(im: &mut Img, at: (f32, f32), tick: u64, hot: f32) {
    let mut pen = Pen::new(im, 120);
    let n = 3 + (hot * 6.0) as u32;
    for i in 0..n {
        let u = |s: u32| unit(i, (tick % 29) as u32, s);
        let a = -FRAC_PI_2 + (u(1) - 0.5) * 2.4;
        let l = 3.0 + u(2) * 9.0 * (0.5 + hot);
        let p = (at.0 + (u(3) - 0.5) * 6.0, at.1 + (u(4) - 0.5) * 6.0);
        pen.streak((p.0 + a.cos() * l, p.1 + a.sin() * l), (a.cos(), a.sin()), l, ramps::GOLD, u(5) * 0.4);
    }
    pen.light(at.0 as i32, at.1 as i32, 'w');
    pen.light(at.0 as i32 + 1, at.1 as i32, '9');
}

/// The crack of a hit at `at`, `age` ticks after: a flare of cross-rays at
/// the instant, a star, a ring running out along the ground and sparks, all
/// scaled by `weight`.
pub(super) fn crack(im: &mut Img, at: (f32, f32), age: u32, weight: Weight, seed: u32, tick: u64) {
    let big = weight as i32;
    let (cx, cy) = (at.0 as i32, at.1 as i32);
    if age < 3 {
        // The flare: a plus and an X in pale steel, thick for a heavy blow.
        let r = 6 + big * 5 - age as i32 * 2;
        for k in -r..=r {
            let ink = if k.abs() < r / 2 { 'w' } else { 'W' };
            im.put(cx + k, cy, ink);
            im.put(cx, cy + k, ink);
            if big >= 2 {
                im.put(cx + k * 2 / 3, cy + k * 2 / 3, 'V');
                im.put(cx - k * 2 / 3, cy + k * 2 / 3, 'V');
            }
        }
        im.rect(cx - 2, cy - 2, 5, 5, 'w');
    }
    juice::impact_burst(im, at, age, seed, weight, tick);
    if weight >= Weight::Solid && age < 18 {
        let mut pen = Pen::new(im, 600);
        let k = age as f32 / 18.0;
        pen.shock(at, 4.0 + age as f32 * (3.0 + big as f32 * 1.4), 1.4, 0.55, ramps::BONE, k);
    }
}

/// Splinters of a lance breaking at `from`, `age` ticks after: slivers of
/// timber turning as they fly, falling to the sand, and a long stump
/// spinning off.
pub(super) fn splinters(im: &mut Img, from: (f32, f32), age: u32, seed: u32, n: u32, floor: f32) {
    if age > 44 {
        return;
    }
    let a = age as f32;
    for i in 0..n {
        let u = |s: u32| unit(i, seed, s);
        let ang = -FRAC_PI_2 + (u(1) - 0.5) * 3.4;
        let speed = 1.6 + u(2) * 3.6;
        let (vx, vy) = (ang.cos() * speed, ang.sin() * speed);
        let (x, mut y) = (from.0 + vx * a * 0.9, from.1 + vy * a + 0.2 * a * a);
        if y > floor {
            y = floor - ((y - floor) * 0.3).min(3.0);
        }
        let spin = u(3) * TAU + a * (0.35 + u(4) * 0.5) * if i % 2 == 0 { 1.0 } else { -1.0 };
        let len = 3.0 + u(5) * 4.0;
        if age > 30 && bayer(x as i32, y as i32) < (a - 30.0) / 14.0 {
            continue;
        }
        let ink = WOOD[(u(6) * 4.0) as usize % 4];
        let (ex, ey) = (x + spin.cos() * len, y + spin.sin() * len * 0.8);
        im.line(x as i32, y as i32, ex as i32, ey as i32, ink);
        im.put(x as i32, y as i32, 'H');
    }
    // The stump: a long one, cartwheeling.
    let sp = a * 0.28;
    let (sx, sy) = (from.0 + (seed % 7) as f32 - 3.0 + a * 0.9, from.1 - 10.0 * ease::out((a / 8.0).min(1.0)) + 0.17 * a * a);
    if sy < floor + 2.0 {
        let (ex, ey) = (sx + sp.cos() * 14.0, sy + sp.sin() * 8.0);
        im.line(sx as i32, sy as i32, ex as i32, ey as i32, 'T');
        im.line(sx as i32, sy as i32 + 1, ex as i32, ey as i32 + 1, 'o');
        im.put(sx as i32, sy as i32, 'H');
    }
}

/// A rider's landing in the sand: a ring of dust running out along the
/// ground, puffs thrown up and settling, `age` ticks after he lands.
pub(super) fn landing(im: &mut Img, at: (f32, f32), age: u32, seed: u32, tick: u64) {
    if age > 40 {
        return;
    }
    let a = age as f32;
    let mut pen = Pen::new(im, 700);
    if age < 20 {
        pen.shock((at.0, at.1), 3.0 + a * 2.6, 1.6, 0.32, DUST, a / 20.0);
        pen.shock((at.0, at.1 - 1.0), 2.0 + a * 1.7, 1.2, 0.3, ramps::SMOKE.cooler(2), a / 20.0);
    }
    let up = Spray { gravity: -0.01, squash: 0.7, drag: 0.1, ..Spray::round(seed, 16, at, (0.8, 2.6), (16.0, 34.0)) };
    up.each(a, |m| pfx::mote(&mut pen, &m, Shape::Smoke, DUST, 3, tick));
    let grit = Spray { gravity: 0.2, squash: 0.8, ..Spray::round(seed ^ 0x77, 10, at, (1.0, 3.0), (10.0, 20.0)) };
    grit.each(a, |m| pfx::mote(&mut pen, &m, Shape::Dot, DUST, 1, tick));
}

/// Dizzy stars circling a head at `at`.
pub(super) fn dizzy(im: &mut Img, at: (i32, i32), tick: u32) {
    for i in 0..3u32 {
        let ang = tick as f32 * 0.22 + i as f32 * TAU / 3.0;
        let (px, py) = (at.0 + (ang.cos() * 9.0) as i32, at.1 + (ang.sin() * 3.0) as i32);
        let big = (tick / 4 + i) % 2 == 0;
        im.put(px, py, '6');
        if big {
            im.put(px - 1, py, '5');
            im.put(px + 1, py, '5');
            im.put(px, py - 1, '5');
            im.put(px, py + 1, '5');
        }
    }
}

/// A favour thrown from the gallery at `from` to a rider at `to`, `age`
/// ticks into its `life`: a scarf in the rider's colours turning in the air,
/// a glint where it lands.
pub(super) fn favour(im: &mut Img, from: (i32, i32), to: (i32, i32), age: u32, life: u32, cloth: (char, char)) {
    if age >= life {
        return;
    }
    let t = age as f32 / life as f32;
    let (x, y) = motion::arc((from.0 as f32, from.1 as f32), (to.0 as f32, to.1 as f32), 34.0, ease::out_quad(t));
    let (x, y) = (x as i32, y as i32);
    let flip = (age / 2) % 2 == 0;
    // A ribbon ten long and two thick with a black edge, rippling as it turns.
    let ribbon: Vec<(i32, i32)> = (0..14)
        .map(|k| {
            let w = if flip { (k as f32 * 0.7).sin() } else { -(k as f32 * 0.7).sin() };
            (x - 7 + k, y + (w * 2.5) as i32)
        })
        .collect();
    for &(rx, ry) in &ribbon {
        for dy in -1..=3 {
            im.put(rx, ry + dy, 'k');
        }
        im.put(rx - 1, ry + 1, 'k');
        im.put(rx + 1, ry + 1, 'k');
    }
    for (k, &(rx, ry)) in ribbon.iter().enumerate() {
        let edge = k % 5 == 4;
        im.put(rx, ry, if edge { cloth.1 } else { cloth.0 });
        im.put(rx, ry + 1, if edge { cloth.0 } else { cloth.1 });
        im.put(rx, ry + 2, cloth.0);
    }
    for k in 1..4 {
        let (tx, ty) = motion::arc((from.0 as f32, from.1 as f32), (to.0 as f32, to.1 as f32), 34.0, ease::out_quad((t - k as f32 * 0.05).max(0.0)));
        im.put(tx as i32, ty as i32, ['5', '4', 'o'][k as usize - 1]);
    }
}

/// A glint where something lands: a four-armed twinkle that swells and goes.
pub(super) fn glint(im: &mut Img, at: (i32, i32), age: u32, life: u32) {
    if age >= life {
        return;
    }
    let mut pen = Pen::new(im, 80);
    pen.twinkle((at.0 as f32, at.1 as f32), 5.0, age as f32 / life as f32, ramps::GOLD);
}

/// The ring of a closing strike over the tilt where the lances will meet:
/// it closes as they come and burns gold while a strike would land.
pub(super) fn closing_ring(im: &mut Img, at: (i32, i32), k: f32, hot: bool, tick: u32) {
    let r = 7.0 + (1.0 - k) * 40.0;
    let ink = if hot { if tick / 2 % 2 == 0 { '6' } else { '5' } } else { 'h' };
    let n = (r * 1.4) as i32 + 12;
    for i in 0..n {
        let a = i as f32 / n as f32 * TAU;
        // Dashed, so a far ring is a light touch.
        if !hot && i % 3 == 0 {
            continue;
        }
        let (x, y) = (at.0 + (a.cos() * r) as i32, at.1 + (a.sin() * r * 0.6) as i32);
        im.put(x, y, ink);
        if hot {
            im.put(x + a.cos().signum() as i32, y, 'p');
        }
    }
    if hot {
        // Tick marks: the lances' line.
        for dx in [-4, 4] {
            im.put(at.0 + dx * 2, at.1, '6');
        }
        im.put(at.0, at.1, 'w');
    } else {
        im.put(at.0, at.1, '6');
    }
}

/// The weight of a tumble's thump on the sand: a flat flash of dither at the
/// instant of a hard blow, lighting the ground round `at` (a dim flare, not
/// a glow that dims anyone).
pub(super) fn flare(im: &mut Img, at: (i32, i32), k: f32, r: i32, ink: char) {
    if k <= 0.0 {
        return;
    }
    for y in at.1 - r / 2..=at.1 + r / 2 {
        for x in at.0 - r..=at.0 + r {
            let d = ((x - at.0) as f32 / r as f32).powi(2) + ((y - at.1) as f32 / (r as f32 / 2.0)).powi(2);
            // Only on bare dark sand: a flare never writes over a figure.
            if d < 1.0 && bayer(x, y) < (1.0 - d) * k * 0.55 && im.get(x, y).is_none_or(|c| c[0] < 56 && c[2] < 40) {
                im.put(x, y, ink);
            }
        }
    }
}

#[allow(dead_code)]
const _: f32 = PI;
