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
use super::ink::{BANK, BLACK, Img, PALETTE, Rgb, RgbMap, SIGNAL_BANK, bank_colours, bank_of, bayer, every_colour, is_dye, rgb, vnoise};
use super::map::Realm;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub s: f32,
    pub fire: bool,
}

pub const DUSK: f32 = 0.58;
pub const DAPPLE: f32 = 0.08;
pub const STEPS: f32 = 4.0;
const CHAIN: usize = 7;

fn lum(c: Rgb) -> f32 {
    0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32
}

fn d2(p: Rgb, t: [f32; 3]) -> f32 {
    (0..3).map(|i| (p[i] as f32 - t[i]).powi(2)).sum()
}

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
    bank_colours(bank)
        .iter()
        .map(|&v| rgb(v))
        .filter(|&p| lum(p) < l0 - 1.0)
        .min_by(|a, b| d2(*a, target).total_cmp(&d2(*b, target)))
        .unwrap_or(BLACK)
}

pub fn warm(c: Rgb) -> Rgb {
    if c == BLACK || bank_of(c) == Some(SIGNAL_BANK) || is_dye(c) {
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

const DUSK_INKS: [(char, char); 30] = [
    ('R', 'P'),
    ('o', 'r'),
    ('O', 'R'),
    ('t', 'o'),
    ('T', 'o'),
    ('H', 'h'),
    ('i', 'h'),
    ('v', 'U'),
    ('V', 'U'),
    ('W', 'v'),
    ('z', 'Q'),
    ('Y', 'M'),
    ('y', 'A'),
    ('^', ']'),
    ('+', '%'),
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

pub fn dusk_ink(ch: char) -> char {
    DUSK_INKS.iter().find(|(from, _)| *from == ch).map_or(ch, |&(_, to)| to)
}

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
        for v in every_colour() {
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

pub fn step_down(c: Rgb, n: usize) -> Rgb {
    match tables().chains.get(&c) {
        Some(chain) => chain[n.min(CHAIN - 1)],
        None => c,
    }
}

fn warmed(c: Rgb) -> Rgb {
    tables().warm.get(&c).copied().unwrap_or(c)
}

pub const SEAM_EDGE: f32 = 4.5;

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
    field[((y & (N - 1)) * N + (x & (N - 1))) as usize]
}

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

struct Memo {
    colour: Rgb,
    chain: Option<&'static [Rgb; CHAIN]>,
}

impl Memo {
    fn of(c: Rgb) -> Memo {
        Memo {
            colour: c,
            chain: tables().chains.get(&c),
        }
    }

    fn down(&self, n: usize) -> Rgb {
        match self.chain {
            Some(chain) => chain[n.min(CHAIN - 1)],
            None => self.colour,
        }
    }
}

pub struct DuskField {
    ox: i32,
    oy: i32,
    w: i32,
    h: i32,
    dapple: Vec<f32>,
    calm: std::sync::Mutex<Option<(u32, std::sync::Arc<Vec<u8>>)>>,
}

#[inline]
fn step_count(level: f32, th: f32) -> usize {
    let v = ((1.0 - level) * STEPS).max(0.0);
    let t = ((v.fract() - 0.5) * SEAM_EDGE + 0.5).clamp(0.0, 1.0);
    (v.floor() + if t > th { 1.0 } else { 0.0 }) as usize
}

impl DuskField {
    pub fn new(ox: i32, oy: i32, w: i32, h: i32) -> DuskField {
        let mut dapple = Vec::with_capacity((w.max(0) * h.max(0)) as usize);
        for y in 0..h {
            let py = (oy + y) as f32 + 0.5;
            for x in 0..w {
                let px = (ox + x) as f32 + 0.5;
                dapple.push((vnoise(px / 48.0, py / 48.0, 77) - 0.5) * DAPPLE);
            }
        }
        DuskField { ox, oy, w, h, dapple, calm: std::sync::Mutex::new(None) }
    }

    fn fits(&self, (ox, oy): (i32, i32), w: i32, h: i32) -> bool {
        (self.ox, self.oy, self.w, self.h) == (ox, oy, w, h)
    }

    fn calm(&self, ambient: f32) -> std::sync::Arc<Vec<u8>> {
        use std::sync::Arc;
        let bits = ambient.to_bits();
        if let Ok(held) = self.calm.lock()
            && let Some((at, steps)) = held.as_ref()
            && *at == bits
        {
            return Arc::clone(steps);
        }
        let mut steps = Vec::with_capacity(self.dapple.len());
        for y in 0..self.h {
            for x in 0..self.w {
                let dapple = self.dapple[(y * self.w + x) as usize];
                let level = (ambient + dapple + 0.0f32).min(1.0);
                steps.push(u8::try_from(step_count(level, seam(self.ox + x, self.oy + y))).unwrap_or(u8::MAX));
            }
        }
        let steps = Arc::new(steps);
        if let Ok(mut held) = self.calm.lock() {
            *held = Some((bits, Arc::clone(&steps)));
        }
        steps
    }
}

pub fn dusk(
    cv: &mut Img,
    realm: &Realm,
    lights: &[Light],
    origin: (i32, i32),
    ambient: f32,
) {
    dusk_over(cv, realm, lights, origin, ambient, None);
}

pub fn dusk_over(
    cv: &mut Img,
    realm: &Realm,
    lights: &[Light],
    (ox, oy): (i32, i32),
    ambient: f32,
    field: Option<&DuskField>,
) {
    let field = field.filter(|f| f.fits((ox, oy), cv.w, cv.h));
    let calm = field.map(|f| f.calm(ambient));
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
    let mut row_lights: Vec<(Light, f32)> = Vec::with_capacity(lights.len());
    let mut memo: Option<Memo> = None;
    for y in 0..cv.h {
        let wy = oy + y;
        let py = wy as f32 + 0.5;
        row_lights.clear();
        row_lights.extend(lights.iter().filter_map(|li| {
            let dy = (py - li.y) * 1.2;
            (!(dy.abs() >= li.r)).then_some((*li, dy))
        }));
        for x in 0..cv.w {
            let wx = ox + x;
            let px = wx as f32 + 0.5;
            let (mut local, mut fire) = (0.0f32, 0.0f32);
            for (li, dy) in &row_lights {
                let dx = px - li.x;
                if dx.abs() >= li.r {
                    continue;
                }
                let d = (dx * dx + dy * dy).sqrt();
                if d < li.r {
                    let t = 1.0 - d / li.r;
                    let c = li.s * t * t;
                    local += c;
                    if li.fire {
                        fire += c;
                    }
                }
            }
            match cv.get(x, y) {
                Some(c) if c != BLACK => {
                    if memo.as_ref().is_none_or(|m| m.colour != c) {
                        memo = Some(Memo::of(c));
                    }
                    let Some(held) = memo.as_ref() else { continue };
                    let at = (y * cv.w + x) as usize;
                    if let Some(calm) = calm.as_ref().filter(|_| local == 0.0 && fire == 0.0) {
                        cv.set(x, y, held.down(usize::from(calm[at])));
                        continue;
                    }
                    let dapple = match field {
                        Some(f) => f.dapple[at],
                        None => (vnoise(px / 48.0, py / 48.0, 77) - 0.5) * DAPPLE,
                    };
                    let level = (ambient + dapple + local).min(1.0);
                    let th = seam(wx, wy);
                    let steps = step_count(level, th);
                    if fire > 0.16 + th * 0.24 {
                        cv.set(x, y, step_down(warmed(c), steps));
                    } else {
                        cv.set(x, y, held.down(steps));
                    }
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

fn vignette_steps(w: i32, h: i32, top: i32) -> std::sync::Arc<Vec<u8>> {
    use std::sync::{Arc, Mutex};
    type Held = Vec<((i32, i32, i32), Arc<Vec<u8>>)>;
    static HELD: Mutex<Held> = Mutex::new(Vec::new());
    let key = (w, h, top);
    if let Ok(held) = HELD.lock()
        && let Some((_, steps)) = held.iter().find(|(k, _)| *k == key)
    {
        return Arc::clone(steps);
    }
    let (wf, hf) = (w as f32, (h - top) as f32);
    let mut steps = Vec::with_capacity((w.max(0) * (h - top).max(0)) as usize);
    for y in top..h {
        for x in 0..w {
            let nx = (x as f32 + 0.5 - wf / 2.0) / (wf / 2.0);
            let ny = (y as f32 - top as f32 + 0.5 - hf / 2.0) / (hf / 2.0);
            let r = (nx * nx + ny * ny).sqrt();
            let n = ((r - 0.72).max(0.0) * 3.2 + bayer(x, y) * 0.999).floor() as usize;
            steps.push(u8::try_from(n).unwrap_or(u8::MAX));
        }
    }
    let steps = Arc::new(steps);
    if let Ok(mut held) = HELD.lock() {
        if held.len() >= 4 {
            held.remove(0);
        }
        held.push((key, Arc::clone(&steps)));
    }
    steps
}

pub fn vignette(f: &mut Img, top: i32) {
    let (w, h) = (f.w, f.h);
    if i64::from(w) * i64::from(h) > 4_000_000 {
        let (wf, hf) = (w as f32, (h - top) as f32);
        for y in top..h {
            for x in 0..w {
                let nx = (x as f32 + 0.5 - wf / 2.0) / (wf / 2.0);
                let ny = (y as f32 - top as f32 + 0.5 - hf / 2.0) / (hf / 2.0);
                let r = (nx * nx + ny * ny).sqrt();
                let steps = ((r - 0.72).max(0.0) * 3.2 + bayer(x, y) * 0.999).floor() as usize;
                if steps > 0
                    && let Some(c) = f.get(x, y)
                {
                    f.set(x, y, step_down(c, steps));
                }
            }
        }
        return;
    }
    let steps = vignette_steps(w, h, top);
    let mut at = 0;
    let mut memo: Option<Memo> = None;
    for y in top..h {
        for x in 0..w {
            let n = steps[at] as usize;
            at += 1;
            if n > 0
                && let Some(c) = f.get(x, y)
            {
                if memo.as_ref().is_none_or(|m| m.colour != c) {
                    memo = Some(Memo::of(c));
                }
                if let Some(held) = memo.as_ref() {
                    f.set(x, y, held.down(n));
                }
            }
        }
    }
}
