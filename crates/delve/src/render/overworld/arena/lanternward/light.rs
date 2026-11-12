//! The lantern's light: warm pools that settle on a lit floor and breathe, the
//! flare's pulse, and the Gloam's edge shimmering where the dark gives way.
//! Light only warms and clears the floor, in an ordered dither of the realm's
//! own inks; it never washes over a figure (pools go down before any body is
//! drawn, and the Gloam is the rules' own idea of who can be seen).
use super::super::super::ink::{Img, Rgb, bayer, hash, ink};
use super::scene::View;
use crate::rules::lanternward::{self as lw, Pk, world::Q};
use std::collections::BTreeMap;

/// Lamplight on dark stone, as the realm's own inks: brown at the dim end
/// of the ramp, amber and gold toward the heart of a pool.
const LAMPLIGHT: [char; 9] = ['n', 'b', 'I', 'B', 'P', 'r', 'R', 'o', 'O'];

/// A floor colour pulled toward lamplight by `k` (0 to 1), as an ink of the
/// ramp: how bright the floor was and how strong the light is pick the step.
/// A frame meets a handful of floor colours, so the answers are kept in a
/// small table (colour and strength to ink) as it goes.
struct Warm {
    seen: [(u32, u8, Rgb); 64],
}

impl Warm {
    fn new() -> Warm {
        Warm { seen: [(u32::MAX, 0, [0; 3]); 64] }
    }

    fn of(&mut self, c: Rgb, k: f32) -> Rgb {
        let level = (k * 20.0) as u8;
        let key = u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2]);
        let slot = (key.wrapping_mul(0x9e37_79b1) >> 26) as usize ^ usize::from(level & 3);
        let slot = slot & 63;
        let (have, at, out) = self.seen[slot];
        if have == key && at == level {
            return out;
        }
        let lum = (0.3 * f32::from(c[0]) + 0.59 * f32::from(c[1]) + 0.11 * f32::from(c[2])) / 255.0;
        let want = lum * 0.5 + (f32::from(level) / 20.0) * 1.05;
        let step = (want * 8.0 + 0.5).clamp(0.0, 8.0) as usize;
        let out = ink(LAMPLIGHT[step]).unwrap_or(c);
        self.seen[slot] = (key, level, out);
        out
    }
}

/// A flicker in `0..=1`, quick and uneven, for how brightly a light burns.
pub(super) fn flicker(tick: u32, seed: i32) -> f32 {
    let (a, b) = (hash(tick as i32 / 2, seed, 5) & 7, hash(tick as i32 / 5, seed, 9) & 7);
    (a + b) as f32 / 14.0
}

/// The dots of a pool, relative to its middle: `(dx, dy, strength in
/// twentieths)`. A pool's pattern depends only on its size, how warm it is and
/// where its middle falls on the dither's four-pixel cycle, so each is worked
/// out once and kept.
type Dots = std::rc::Rc<Vec<(i16, i16, u8)>>;

thread_local! {
    static DOTS: std::cell::RefCell<BTreeMap<(i16, u8, u8, u8), Dots>> = const { std::cell::RefCell::new(BTreeMap::new()) };
}

fn dots(r: i32, power20: u8, (cx, cy): (i32, i32)) -> Dots {
    let key = (r as i16, power20, (cx & 3) as u8, (cy & 3) as u8);
    if let Some(d) = DOTS.with(|m| m.borrow().get(&key).cloned()) {
        return d;
    }
    let (rf, ry) = (r as f32, r as f32 * 0.82);
    let power = f32::from(power20) / 20.0;
    let mut out = Vec::new();
    for dy in -r..=r {
        let ny = dy as f32 / ry;
        if ny.abs() >= 1.0 {
            continue;
        }
        let across = (rf * (1.0 - ny * ny).sqrt()) as i32;
        for dx in -across..=across {
            let nx = dx as f32 / rf;
            let d2 = nx * nx + ny * ny;
            if d2 >= 1.0 {
                continue;
            }
            // How lit this dot is: bright at the heart, nothing at the rim.
            let t = 1.0 - d2;
            let k = t * t.sqrt() * power;
            if bayer(cx + dx, cy + dy) < k * 0.95 {
                out.push((dx as i16, dy as i16, (k * 1.3 * 20.0).min(13.0) as u8));
            }
        }
    }
    let d = std::rc::Rc::new(out);
    DOTS.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() > 600 {
            m.clear();
        }
        m.insert(key, d.clone());
    });
    d
}

/// One pool of lamplight on the floor at `(cx, cy)`: `r` pixels wide, `power`
/// how warm its heart is (0.3 is a gentle hint). Dithered, so the edge is a
/// stipple the floor shows through; it breathes with the flame it belongs to.
fn pool(im: &mut Img, warm: &mut Warm, (cx, cy): (i32, i32), r: i32, power: f32, seed: i32, tick: u32) {
    if r <= 0 || cx < -r || cy < -r || cx > 384 + r || cy > 224 + r {
        return;
    }
    let live = 0.86 + 0.28 * flicker(tick, seed);
    // Strength in twentieths, in coarse steps so the same few patterns come round again.
    let p20 = (((power * live * 20.0) / 2.0).round() as u8) * 2;
    for &(dx, dy, k) in dots(r, p20, (cx, cy)).iter() {
        let (x, y) = (cx + i32::from(dx), cy + i32::from(dy));
        if let Some(c) = im.get(x, y) {
            im.set(x, y, warm.of(c, f32::from(k) / 20.0));
        }
    }
}

/// Every steady light on a lit floor: braziers, the resting bowl, the ember,
/// the great lamp, the Beacon, a bolt of light in flight, an open lamp-heart,
/// and the knights' own lanterns, which swell and pulse when they flare.
pub(super) fn pools(im: &mut Img, v: &View) {
    let s = v.s;
    let mut warm = Warm::new();
    for (i, p) in s.props.iter().enumerate() {
        let (x, y) = (v.x(p.x), v.y(p.y) + 4);
        let (r, power) = match p.kind {
            Pk::Brazier if p.lit() => (44, 0.55),
            Pk::Rest => (50, 0.5),
            Pk::Ember => (22, 0.5),
            Pk::Great if p.lit() => (54, 0.55),
            Pk::Beacon if p.on == 1 => (84, 0.7),
            Pk::Beacon if p.t > 0 => (40, 0.45),
            _ => continue,
        };
        // A brazier that is about to burn out shrinks and gutters.
        let low = if p.kind == Pk::Brazier && p.on == 0 { (i32::from(p.t) * 100 / 90).clamp(35, 100) } else { 100 };
        pool(im, &mut warm, (x, y), r * low / 100, power, i as i32, v.tick);
    }
    for b in s.bolts.iter().filter(|b| b.light()) {
        pool(im, &mut warm, (v.x(b.x), v.y(b.y) + 4), 26, 0.5, 90 + i32::from(b.owner), v.tick);
    }
    if let Some(w) = &s.boss
        && s.boss_open()
    {
        let (hx, hy) = w.heart();
        pool(im, &mut warm, (v.x(hx), v.y(hy)), 74, 0.7, 77, v.tick);
    }
    if s.inv.lantern {
        for (&id, k) in &s.seats {
            if k.act == lw::Act::Down || !k.flaring() {
                // A lantern at rest leaves a small warm patch under the knight.
                if k.act != lw::Act::Down {
                    pool(im, &mut warm, (v.x(k.x), v.y(k.y) + 4), 20, 0.3, 200 + id as i32, v.tick);
                }
                continue;
            }
            // A flare: a bigger pool that breathes on the beat.
            let breathe = [0, 2, 4, 2][(v.tick / 3 % 4) as usize];
            pool(im, &mut warm, (v.x(k.x), v.y(k.y) + 4), 54 + breathe, 0.6, 200 + id as i32, v.tick);
        }
    }
}

/// How lit each 2 by 2 cell of the screen is, 0 dark to 3 full, row by row
/// (`CELLS_W` by `CELLS_H`): the rules' own idea of who can be seen.
pub(super) fn levels(v: &View) -> Vec<u8> {
    let lights = v.s.lights();
    let mut out = Vec::with_capacity(CELLS_W * CELLS_H);
    for cy in 0..CELLS_H as i32 {
        for cx in 0..CELLS_W as i32 {
            let (qx, qy) = ((cx * 2 + v.ox) * Q + Q, (cy * 2 + v.oy) * Q + Q);
            out.push(lw::props::light_in(&lights, qx, qy));
        }
    }
    out
}

const CELLS_W: usize = 192;
const CELLS_H: usize = 112;

/// A short table of uneven numbers 0 to 7, for the edge of a light.
const NOISE: [u8; 64] = [
    3, 6, 1, 4, 7, 0, 5, 2, 6, 2, 7, 3, 1, 5, 0, 4, 0, 5, 3, 7, 2, 6, 4, 1, 7, 1, 4, 0, 5, 3, 6, 2,
    2, 4, 0, 6, 3, 7, 1, 5, 5, 0, 6, 2, 4, 1, 7, 3, 1, 7, 2, 5, 0, 4, 3, 6, 4, 3, 5, 1, 6, 2, 0, 7,
];

/// The edge of a light shimmers: the flames, not the camera, move it. `-0.5` to `0.5`.
fn shimmer(v: &View, cx: i32, cy: i32) -> f32 {
    let at = (cx / 3 + (cy / 3) * 5 + v.tick as i32 / 4).rem_euclid(64) as usize;
    f32::from(NOISE[at]) / 7.0 - 0.5
}

/// Before the bodies are drawn: the floor a Gloam light reaches takes the
/// colour of lamplight, thick where it is bright and thinning to a fringe
/// (only floor, so no figure is ever tinted).
pub(super) fn lit_floor(im: &mut Img, v: &View, levels: &[u8]) {
    let mut warm = Warm::new();
    let breathe = 0.04 * flicker(v.tick, 61);
    for cy in 0..CELLS_H {
        for cx in 0..CELLS_W {
            let (share, strength) = match levels[cy * CELLS_W + cx] {
                3 => (0.16 + breathe, 0.36),
                2 => (0.30 + breathe + 0.1 * shimmer(v, cx as i32, cy as i32), 0.44),
                1 => (0.14 + 0.06 * shimmer(v, cx as i32, cy as i32), 0.40),
                _ => continue,
            };
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (x, y) = (cx as i32 * 2 + dx, cy as i32 * 2 + dy);
                if bayer(x, y) > 1.0 - share
                    && let Some(c) = im.get(x, y)
                {
                    im.set(x, y, warm.of(c, strength));
                }
            }
        }
    }
}

/// The Gloam: only light shows the floor. Dithered, so the reach of a lantern
/// is a soft stipple rather than a ring, shimmering along its edge.
pub(super) fn gloam(im: &mut Img, v: &View, levels: &[u8]) {
    for cy in 0..CELLS_H {
        for cx in 0..CELLS_W {
            let dark = match levels[cy * CELLS_W + cx] {
                3 => continue,
                2 => 0.10 + 0.10 * shimmer(v, cx as i32, cy as i32),
                1 => 0.5 + 0.18 * shimmer(v, cx as i32, cy as i32),
                _ => 1.0,
            };
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (x, y) = (cx as i32 * 2 + dx, cy as i32 * 2 + dy);
                if bayer(x, y) < dark {
                    im.put(x, y, 'k');
                }
            }
        }
    }
}
