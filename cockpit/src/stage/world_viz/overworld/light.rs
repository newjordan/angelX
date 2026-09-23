//! Dusk: the realm's mood lighting.
//!
//! Nothing is dimmed by scaling RGB. Each colour steps down its own palette
//! bank (greens into deeper greens, stone into slate) and the fractional step
//! is resolved with the 4x4 Bayer threshold, so the dither itself is the
//! shading and every pixel stays on the palette. Light sources lift the steps
//! back; firelight also pulls colours toward the timber bank; where light
//! falls on bare paper it leaves dithered pools. Signal inks are emissive:
//! state never dims.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::ground::pool_ink;
use super::ink::{BANK, BLACK, Img, PALETTE, Rgb, SIGNAL_BANK, bank_of, bayer, rgb, vnoise};
use super::map::Realm;

/// A light in world pixels. `fire` lights warm what they touch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Light {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) r: f32,
    pub(crate) s: f32,
    pub(crate) fire: bool,
}

/// Dusk ambient level: `1.0` is full daylight, the realm's resting mood sits
/// just over half.
pub(crate) const DUSK: f32 = 0.58;
/// How many bank steps separate full light from darkness.
const STEPS: f32 = 4.0;
const CHAIN: usize = 7;

fn lum(c: Rgb) -> f32 {
    0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32
}

fn d2(p: Rgb, t: [f32; 3]) -> f32 {
    (0..3).map(|i| (p[i] as f32 - t[i]).powi(2)).sum()
}

/// One step darker inside the colour's own bank; signal inks never move.
pub(crate) fn darker(c: Rgb) -> Rgb {
    if c == BLACK {
        return c;
    }
    let Some(bank) = bank_of(c) else {
        return BLACK;
    };
    if bank == SIGNAL_BANK {
        return c;
    }
    let target = [c[0] as f32 * 0.66, c[1] as f32 * 0.66, c[2] as f32 * 0.66];
    let l0 = lum(c);
    PALETTE[bank * BANK..(bank + 1) * BANK]
        .iter()
        .map(|&v| rgb(v))
        .filter(|&p| lum(p) < l0 - 1.0)
        .min_by(|a, b| d2(*a, target).total_cmp(&d2(*b, target)))
        .unwrap_or(BLACK)
}

/// Firelight pulls a colour toward timber and structure — never toward
/// foliage or stone blues, which read as a green cast.
pub(crate) fn warm(c: Rgb) -> Rgb {
    if c == BLACK || bank_of(c) == Some(SIGNAL_BANK) {
        return c;
    }
    let t = [
        (c[0] as f32 * 1.15).min(255.0),
        c[1] as f32 * 0.97,
        c[2] as f32 * 0.72,
    ];
    PALETTE[0..BANK]
        .iter()
        .chain(PALETTE[2 * BANK..3 * BANK].iter())
        .map(|&v| rgb(v))
        .min_by(|a, b| d2(*a, t).total_cmp(&d2(*b, t)))
        .unwrap_or(c)
}

struct Tables {
    chains: HashMap<Rgb, [Rgb; CHAIN]>,
    warm: HashMap<Rgb, Rgb>,
}

fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut chains = HashMap::new();
        let mut warmed = HashMap::new();
        for &v in PALETTE.iter() {
            let c = rgb(v);
            let mut chain = [c; CHAIN];
            for i in 1..CHAIN {
                chain[i] = darker(chain[i - 1]);
            }
            chains.insert(c, chain);
            warmed.insert(c, warm(c));
        }
        Tables {
            chains,
            warm: warmed,
        }
    })
}

/// `c` taken `n` steps down its bank.
pub(crate) fn step_down(c: Rgb, n: usize) -> Rgb {
    match tables().chains.get(&c) {
        Some(chain) => chain[n.min(CHAIN - 1)],
        None => c,
    }
}

fn warmed(c: Rgb) -> Rgb {
    tables().warm.get(&c).copied().unwrap_or(c)
}

/// `(total, firelight)` falling on a world point.
pub(crate) fn light_at(lights: &[Light], x: f32, y: f32) -> (f32, f32) {
    let (mut sum, mut fire) = (0.0, 0.0);
    for li in lights {
        let (dx, dy) = (x - li.x, (y - li.y) * 1.2);
        if dx.abs() >= li.r || dy.abs() >= li.r {
            continue;
        }
        let d = (dx * dx + dy * dy).sqrt();
        if d < li.r {
            let t = 1.0 - d / li.r;
            let c = li.s * t * t;
            sum += c;
            if li.fire {
                fire += c;
            }
        }
    }
    (sum, fire)
}

/// Light a view whose top-left sits at world pixel `(ox, oy)`.
pub(crate) fn dusk(
    cv: &mut Img,
    realm: &Realm,
    lights: &[Light],
    (ox, oy): (i32, i32),
    ambient: f32,
) {
    // Only lights whose reach touches this view.
    let (w, h) = (cv.w as f32, cv.h as f32);
    let lights: Vec<Light> = lights
        .iter()
        .copied()
        .filter(|li| {
            li.x + li.r > ox as f32
                && li.x - li.r < ox as f32 + w
                && li.y + li.r > oy as f32
                && li.y - li.r < oy as f32 + h
        })
        .collect();
    let lights = lights.as_slice();
    for y in 0..cv.h {
        for x in 0..cv.w {
            let (wx, wy) = (ox + x, oy + y);
            let (px, py) = (wx as f32 + 0.5, wy as f32 + 0.5);
            let dapple = (vnoise(px / 48.0, py / 48.0, 77) - 0.5) * 0.14;
            let (local, fire) = light_at(lights, px, py);
            let level = (ambient + dapple + local).min(1.0);
            match cv.get(x, y) {
                Some(c) if c != BLACK => {
                    let c = if fire > 0.16 + bayer(wx, wy) * 0.24 {
                        warmed(c)
                    } else {
                        c
                    };
                    let steps = ((1.0 - level) * STEPS + bayer(wx, wy)).floor().max(0.0) as usize;
                    cv.set(x, y, step_down(c, steps));
                }
                _ => {
                    let p = (local - 0.22) * 0.55;
                    if p > 0.0 && bayer(wx, wy) < p {
                        cv.put(x, y, pool_ink(realm, wx, wy, fire > local * 0.5));
                    }
                }
            }
        }
    }
}

/// Dim the rim of the play field below `top`: the eye settles in the middle.
pub(crate) fn vignette(f: &mut Img, top: i32) {
    let (w, h) = (f.w as f32, (f.h - top) as f32);
    for y in top..f.h {
        for x in 0..f.w {
            let nx = (x as f32 + 0.5 - w / 2.0) / (w / 2.0);
            let ny = (y as f32 - top as f32 + 0.5 - h / 2.0) / (h / 2.0);
            let r = (nx * nx + ny * ny).sqrt();
            let steps = ((r - 0.72).max(0.0) * 3.2 + bayer(x, y) * 0.999).floor() as usize;
            if steps > 0
                && let Some(c) = f.get(x, y)
            {
                f.set(x, y, step_down(c, steps));
            }
        }
    }
}
