//! Dusk: the realm's mood lighting.
//!
//! Nothing is dimmed by scaling RGB. Each colour steps down its own palette
//! bank (greens into deeper greens, stone into slate) and the fractional step
//! is resolved against a field of soft blots (`seam`), so a calm field holds
//! one step, the light gives way in grouped patches rather than a checker,
//! and every pixel stays on the palette. Light sources lift the steps
//! back; firelight also pulls colours toward the timber bank; where light
//! falls on bare paper it leaves dithered pools. Signal inks are emissive:
//! state never dims.

use std::sync::OnceLock;

use super::ground::pool_ink;
use super::ink::{BANK, BLACK, Img, PALETTE, Rgb, RgbMap, SIGNAL_BANK, bank_of, bayer, rgb, vnoise};
use super::map::Realm;

/// A light in world pixels. `fire` lights warm what they touch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub s: f32,
    pub fire: bool,
}

/// Dusk ambient level: `1.0` is full daylight, the realm's resting mood sits
/// just over half.
pub const DUSK: f32 = 0.58;
/// How far the ambient wanders across a field, in slow patches: less than
/// half a step either way, so a calm field holds its step and only a light
/// carries it to the next.
pub const DAPPLE: f32 = 0.08;
/// How many bank steps separate full light from darkness.
pub const STEPS: f32 = 4.0;
const CHAIN: usize = 7;

fn lum(c: Rgb) -> f32 {
    0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32
}

fn d2(p: Rgb, t: [f32; 3]) -> f32 {
    (0..3).map(|i| (p[i] as f32 - t[i]).powi(2)).sum()
}

/// One step darker inside the colour's own bank; signal inks never move.
pub fn darker(c: Rgb) -> Rgb {
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
pub fn warm(c: Rgb) -> Rgb {
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

/// What an ink wears at dusk, `(from, to)`: every signal ink goes to a
/// plain neighbour, so decoration never wears state, and the bright and
/// saturated plain inks step one or two down their own bank. Inks not
/// listed keep their colour. In order: a step's target is never a later
/// step's source, so a picture swapped through the list never runs on.
const DUSK_INKS: [(char, char); 28] = [
    // timber: cream and gold down to oak, orange to umber, bottom first
    ('R', 'P'),
    ('o', 'r'),
    ('O', 'R'),
    ('t', 'o'),
    ('T', 'o'),
    // structure and stone: the near-whites stop at a quiet grey
    ('H', 'h'),
    ('i', 'h'),
    ('v', 'U'),
    ('V', 'U'),
    ('W', 'v'),
    ('z', 'Q'),
    // foliage: sunlit leaf down to the realm's own trees
    ('Y', 'M'),
    ('y', 'A'),
    // signal: lamps, heraldry and cloth to plain dyes
    ('0', 'S'),
    ('1', 'q'),
    ('2', 'U'),
    ('3', 'v'),
    ('w', 'h'),
    ('a', 'r'),
    ('4', 'o'),
    ('@', 'R'),
    ('5', 'o'),
    ('6', 'O'),
    ('$', 'J'),
    ('c', 'h'),
    ('9', 'h'),
    ('8', 'B'),
    ('7', 'p'),
];

/// `ch` at dusk (`DUSK_INKS`): its plain, quieter partner, or itself.
pub fn dusk_ink(ch: char) -> char {
    DUSK_INKS.iter().find(|(from, _)| *from == ch).map_or(ch, |&(_, to)| to)
}

/// A finished picture at dusk (`DUSK_INKS`): a building on the kingdom map,
/// a pavilion, the bunting round a yard. What it shows is scenery, so
/// nothing in it lights up as if something were happening there. Once per
/// picture: dusked twice, a step runs on.
pub fn at_dusk(img: &Img) -> Img {
    img.recolor(&DUSK_INKS)
}

struct Tables {
    chains: RgbMap<[Rgb; CHAIN]>,
    warm: RgbMap<Rgb>,
}

fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut chains = RgbMap::default();
        let mut warmed = RgbMap::default();
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
pub fn step_down(c: Rgb, n: usize) -> Rgb {
    match tables().chains.get(&c) {
        Some(chain) => chain[n.min(CHAIN - 1)],
        None => c,
    }
}

fn warmed(c: Rgb) -> Rgb {
    tables().warm.get(&c).copied().unwrap_or(c)
}

/// How sharply a step gives way to the next across its fraction: the blots
/// of `seam` fill only the middle part of the step, so a broad, calm
/// change of light reads as one soft-edged patch rather than a field of
/// blots.
pub const SEAM_EDGE: f32 = 4.5;

/// Where light gives way from one step to the next: a threshold in `0..1`
/// for the fractional step at world pixel `(x, y)`. A field of soft blots
/// some four pixels across (with only a whisper of the ordered dither at
/// their edges), so the boundary between two steps wanders like light on a
/// rough surface and a calm field falls into a few grouped patches, never
/// an all-over checker. One 64-pixel tile, periodic, built once.
pub fn seam(x: i32, y: i32) -> f32 {
    const N: i32 = 64;
    const CELL: i32 = 4;
    static FIELD: OnceLock<Vec<f32>> = OnceLock::new();
    let field = FIELD.get_or_init(|| {
        let cells = N / CELL;
        let corner = |cx: i32, cy: i32| {
            (super::ink::hash(cx.rem_euclid(cells), cy.rem_euclid(cells), 0x5ea4) & 0xffff) as f32 / 65535.0
        };
        (0..N * N)
            .map(|i| {
                let (px, py) = (i % N, i / N);
                let (fx, fy) = ((px % CELL) as f32 / CELL as f32, (py % CELL) as f32 / CELL as f32);
                let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
                let (cx, cy) = (px / CELL, py / CELL);
                let top = corner(cx, cy) + (corner(cx + 1, cy) - corner(cx, cy)) * sx;
                let bot = corner(cx, cy + 1) + (corner(cx + 1, cy + 1) - corner(cx, cy + 1)) * sx;
                let n = top + (bot - top) * sy;
                (0.5 + (n - 0.5) * 0.8 + (bayer(px, py) - 0.5) * 0.06).clamp(0.04, 0.96)
            })
            .collect()
    });
    // A power-of-two tile: the mask is the euclidean remainder, negatives too.
    field[((y & (N - 1)) * N + (x & (N - 1))) as usize]
}

/// `(total, firelight)` falling on a world point.
pub fn light_at(lights: &[Light], x: f32, y: f32) -> (f32, f32) {
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
pub fn dusk(
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
            let dapple = (vnoise(px / 48.0, py / 48.0, 77) - 0.5) * DAPPLE;
            let (local, fire) = light_at(lights, px, py);
            let level = (ambient + dapple + local).min(1.0);
            match cv.get(x, y) {
                Some(c) if c != BLACK => {
                    let th = seam(wx, wy);
                    let c = if fire > 0.16 + th * 0.24 {
                        warmed(c)
                    } else {
                        c
                    };
                    // One step held across a calm field; where it gives
                    // way to the next the boundary wanders in soft blots
                    // (`seam`), never an all-over checker.
                    let v = ((1.0 - level) * STEPS).max(0.0);
                    let t = ((v.fract() - 0.5) * SEAM_EDGE + 0.5).clamp(0.0, 1.0);
                    let steps = (v.floor() + if t > th { 1.0 } else { 0.0 }) as usize;
                    cv.set(x, y, step_down(c, steps));
                }
                _ => {
                    let inside = wx >= 0
                        && wy >= 0
                        && wx < super::map::MAP_W * super::map::TILE
                        && wy < super::map::MAP_H * super::map::TILE;
                    let p = (local - 0.22) * 0.55;
                    if inside && p > 0.0 && bayer(wx, wy) < p {
                        cv.put(x, y, pool_ink(realm, wx, wy, fire > local * 0.5));
                    }
                }
            }
        }
    }
}

/// Dim the rim of the play field below `top`: the eye settles in the middle.
pub fn vignette(f: &mut Img, top: i32) {
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
