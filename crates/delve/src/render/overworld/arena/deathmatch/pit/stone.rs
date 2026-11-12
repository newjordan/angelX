//! The pit's walls, painted by where on them a dot lands: ashlar with age,
//! arched cells behind iron, banners and sconces, the gate with its raised
//! portcullis, pillars with their capitals, and above the cornice the
//! crowd, rank behind rank, waving.
use super::{Centre, Face, Kind, Texel};
use crate::render::overworld::ink::{hash, vnoise};

const STONE: [f32; 3] = [0.47, 0.47, 0.43];
const CRIMSON: [f32; 3] = [0.55, 0.12, 0.08];
const TEAL: [f32; 3] = [0.10, 0.27, 0.34];
const GOLD: [f32; 3] = [0.82, 0.60, 0.22];
const IRON: [f32; 3] = [0.20, 0.18, 0.17];
const WOOD: [f32; 3] = [0.30, 0.19, 0.10];
const GALLERY: [f32; 3] = [0.58, 0.40, 0.25];
const SKIN: [f32; 3] = [0.46, 0.32, 0.23];
const CLOTH: [[f32; 3]; 6] = [
    [0.30, 0.06, 0.05],
    [0.30, 0.22, 0.09],
    [0.10, 0.11, 0.15],
    [0.12, 0.14, 0.19],
    [0.34, 0.28, 0.22],
    [0.20, 0.10, 0.12],
];

/// A sine good to a thousandth, with no library call (the crowd asks for hundreds a frame).
#[inline]
fn fsin(x: f32) -> f32 {
    let x = x - std::f32::consts::TAU * (x * (1.0 / std::f32::consts::TAU) + 0.5).floor();
    let y = x * (4.0 / std::f32::consts::PI) - x * x.abs() * (4.0 / (std::f32::consts::PI * std::f32::consts::PI));
    0.225 * (y * y.abs() - y) + y
}

fn u01(h: u32) -> f32 {
    (h & 0xffff) as f32 / 65535.0
}
fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}
fn scale(a: [f32; 3], k: f32) -> [f32; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn plain(a: [f32; 3]) -> Texel {
    Texel { a, e: [0.0; 3] }
}

/// Dressed stone in courses `course` high, blocks of mixed length, mortar
/// that thickens with distance (so no dot lands in a hair), bevelled edges.
fn ashlar(u: f32, z: f32, course: f32, seed: u32, lod: f32) -> [f32; 3] {
    let rowf = z / course;
    let row = rowf.floor();
    let vz = rowf - row;
    let rh = hash(row as i32, 3, seed);
    let blen = 0.85 + 0.6 * u01(rh);
    let bu = (u + u01(rh >> 7) * blen * 3.0) / blen;
    let col = bu.floor();
    let vu = bu - col;
    let id = hash(col as i32, row as i32, seed ^ 0x55);
    let rnd = u01(id);
    let mut tone = 0.82 + 0.34 * rnd;
    let (mu, mz) = (0.05 * (1.0 + 1.8 * lod) / blen, 0.05 * (1.0 + 1.6 * lod) / course);
    if vz < mz || vu < mu {
        tone *= 0.58 + 0.25 * lod;
    } else if vz > 0.86 {
        tone *= 1.12;
    } else if vz < mz + 0.12 {
        tone *= 0.9;
    } else if vu < mu + 0.08 {
        tone *= 0.94;
    }
    if lod < 0.7 && u01(hash((u * 13.0) as i32, (z * 13.0) as i32, seed ^ 0x99)) < 0.06 {
        tone *= 0.78;
    }
    let warm = rnd - 0.5;
    let mut a = [STONE[0] * tone * (1.0 + 0.10 * warm), STONE[1] * tone, STONE[2] * tone * (1.0 - 0.12 * warm)];
    // Age: weather streaks down from the top, blood splashed low, damp at the foot.
    let streak = ((vnoise(u * 2.3 + (seed & 0xff) as f32, z * 0.2, seed) - 0.56) * 2.2).clamp(0.0, 0.5);
    a = scale(a, 1.0 - streak * 0.6);
    if z < 1.4 {
        let sp = vnoise(u * 1.1, 9.0 + (seed & 0xff) as f32, 0x2b);
        if sp > 0.58 {
            let amt = ((sp - 0.58) * 2.4).min(1.0) * (1.0 - z / 1.4);
            a = mix(a, [0.30, 0.08, 0.05], amt * 0.75);
        }
    }
    a
}

/// A wall sconce's bracket and flame at `du` across and `dz` above its
/// seat, plus the glow it throws on the stone round it.
fn sconce(du: f32, dz: f32, t: u32, seed: u32, base: [f32; 3]) -> Option<Texel> {
    let flick = 0.82 + 0.18 * u01(hash((t / 2) as i32, seed as i32, 0x3c));
    let h = 0.46 * flick;
    if dz > -0.14 && dz < 0.0 && du.abs() < 0.12 {
        return Some(plain(IRON));
    }
    if dz >= 0.0 && dz < h {
        let w = 0.13 * (1.0 - dz / h).powf(0.7) + 0.02;
        if du.abs() < w {
            let core = (du.abs() / w < 0.45) && dz < h * 0.6;
            let c = if core { [1.0, 0.82, 0.40] } else { [1.0, 0.52, 0.14] };
            return Some(Texel { a: [0.0; 3], e: c });
        }
    }
    let r = (du * du + (dz - 0.2) * (dz - 0.2)).sqrt();
    if r < 0.5 {
        let g = (1.0 - r / 0.5).powi(2) * 0.55 * flick;
        return Some(Texel { a: base, e: [g * 0.9, g * 0.42, g * 0.1] });
    }
    None
}

/// A banner hung from the cornice: swallow-tailed, trimmed in gold, a
/// diamond at its heart, swaying a little at the foot.
fn banner(du: f32, z: f32, top: f32, bottom: f32, half: f32, t: u32, seed: u32) -> Option<Texel> {
    let sway = 0.035 * fsin((t as f32 * 0.06) + z * 2.1 + (seed & 0xff) as f32) * ((top - z) / (top - bottom)).clamp(0.0, 1.0);
    let du = du + sway;
    if z > top && z < top + 0.07 && du.abs() < half + 0.12 {
        return Some(plain(WOOD));
    }
    let tail = bottom + 0.34 * (1.0 - (du.abs() / half)).max(0.0);
    if du.abs() >= half || z >= top || z < tail {
        return None;
    }
    let cloth = if seed % 4 == 3 { TEAL } else { CRIMSON };
    let fold = 0.86 + 0.14 * fsin(du * 12.0 + sway * 30.0);
    let mut a = scale(cloth, fold);
    let mid = (top + bottom) * 0.5 + 0.1;
    if du.abs() > half - 0.085 || z > top - 0.08 || z < tail + 0.07 {
        a = scale(GOLD, 0.9 + 0.1 * fold);
    } else {
        let d = du.abs() / (half * 0.42) + (z - mid).abs() / ((top - bottom) * 0.22);
        if d < 1.0 {
            a = if d < 0.45 { scale(cloth, 0.7) } else { scale(GOLD, 0.95) };
        }
    }
    Some(plain(a))
}

/// An arched cell in the wall: voussoirs round a dark mouth, iron bars
/// across it, and now and then a fire burning within (`t` is `None` when
/// baking, which leaves the fire to the live pass).
fn alcove(du: f32, z: f32, t: Option<u32>, seed: u32, lod: f32) -> Option<Texel> {
    let (a, zs, base) = (0.95, 1.30, 0.34);
    if z < base {
        return None;
    }
    let inside = |r: f32| du.abs() < r && (z < zs || (du * du + (z - zs) * (z - zs)) < r * r);
    if inside(a) {
        let lit = seed % 3 == 0;
        let glow = match t {
            Some(t) if lit => {
                let flick = 0.8 + 0.2 * u01(hash((t / 3) as i32, seed as i32, 0x3d));
                ((1.0 - (z - base) / 2.0).max(0.0)).powi(2) * 0.5 * flick
            }
            _ => 0.0,
        };
        let e = [glow, glow * 0.45, glow * 0.12];
        let bar = ((du * 3.4).rem_euclid(1.0) - 0.5).abs() < 0.07 * (1.0 + lod) || (z - 0.7).abs() < 0.05 || (z - 1.45).abs() < 0.05;
        if bar {
            return Some(Texel { a: scale(IRON, 1.3), e });
        }
        return Some(Texel { a: [0.012, 0.010, 0.010], e });
    }
    if inside(a + 0.21) {
        let ang = (du).atan2((z - zs).max(0.0));
        let joint = z > zs && ((ang * 6.0).rem_euclid(1.0) < 0.07);
        return Some(plain(scale(STONE, if joint { 0.6 } else { 1.14 })));
    }
    None
}

/// The crowd, three ranks deep behind the rail, arms up in a wave.
pub(super) fn crowd(u: f32, z: f32, rail: f32, t: u32, fever: f32, lod: f32) -> Texel {
    for r in 0..3u32 {
        let zb = rail - 0.2 + r as f32 * 0.5;
        if z < zb - 0.02 || z > zb + 0.95 {
            continue;
        }
        let sp = 0.62 - 0.04 * r as f32;
        let uu = u + r as f32 * 0.21;
        let k = (uu / sp).floor();
        let h = hash(k as i32, r as i32, 0xc0);
        let uc = (k + 0.5) * sp + (u01(h) - 0.5) * 0.12;
        let du = uu - uc;
        if du.abs() > 0.4 {
            continue;
        }
        let wave = fsin(u * 0.5 - t as f32 * 0.2 + r as f32);
        let cheer = wave > 0.62 || u01(h >> 9) < fever * 0.8 || (h % 9 == 0 && fsin(t as f32 * 0.3 + (h >> 4) as f32) > 0.3);
        let bob = if cheer { 0.05 * fsin(t as f32 * 0.5 + (h & 0xff) as f32).abs() } else { 0.0 };
        let dz = z - zb - bob;
        let head = (du / 0.14).powi(2) + ((dz - 0.50) / 0.165).powi(2) < 1.0;
        let taper = ((dz - 0.06) / 0.34).clamp(0.0, 1.0);
        let body = (dz > 0.06 && dz < 0.40 && du.abs() < 0.24 - 0.12 * taper) || (dz >= 0.3 && dz < 0.42 && du.abs() < 0.06);
        let side = if h & 1 == 0 { 1.0 } else { -1.0 };
        let arm = cheer && ((du - 0.24 * side).abs() < 0.055 && dz > 0.15 && dz < 0.74 || ((du - 0.24 * side).powi(2) + (dz - 0.80).powi(2)) < 0.0055);
        if head {
            let shade = 0.62 - 0.14 * r as f32;
            let lit = if dz > 0.62 { 1.15 } else { 1.0 };
            return plain(scale(SKIN, shade * lit * (0.85 + 0.3 * u01(h >> 3))));
        }
        if body || arm {
            let c = CLOTH[(h >> 12) as usize % CLOTH.len()];
            return plain(scale(c, (0.9 - 0.18 * r as f32) * (1.0 - 0.1 * lod)));
        }
    }
    // The stand behind, lit by the dusk and the smoke.
    let tone = 0.82 + 0.22 * u01(hash((u * 3.0) as i32, (z * 3.0) as i32, 0x5e));
    Texel { a: scale(GALLERY, tone), e: [0.07, 0.04, 0.025] }
}

/// Over the parapet: poles and pennants at the posts, else the sky.
pub(super) fn top(f: &Face, u: f32, z: f32, t: u32) -> Option<Texel> {
    for post in f.pole_posts() {
        let du = u - post;
        if du.abs() < 0.035 && z < 6.3 {
            return Some(plain(scale(WOOD, 1.3)));
        }
        let flutter = 0.06 * fsin((t as f32) * 0.25 + z * 6.0);
        if z > 5.9 && z < 6.3 && du > 0.03 && du < (0.6 * (1.0 - (z - 5.9) / 0.4)).max(0.0) + flutter {
            return Some(plain(CRIMSON));
        }
    }
    None
}

/// Everything on a face that moves: sconces, banners, the fire in a lit cell.
pub(super) fn overlay(f: &Face, u: f32, z: f32, t: u32, base: [f32; 3]) -> Option<Texel> {
    if z < 0.34 || z > f.gal || f.feat.get((u * 10.0) as usize).is_none_or(|&m| m == 0) {
        return None;
    }
    if f.kind == Kind::Pillar {
        if f.centre == Centre::Banner && z > 0.9 {
            return banner(u - (f.hi - f.lo) * 0.5, z, 2.78, 1.0, 0.5, t, f.seed);
        }
        return None;
    }
    if let Some((du, what, id)) = f.post_at(u) {
        match what {
            Centre::Sconce => {
                if z > 1.5 && z < 2.7 {
                    if let Some(s) = sconce(du, z - 1.95, t, id, base) {
                        return Some(s);
                    }
                }
            }
            Centre::Banner => {
                if let Some(b) = banner(du, z, f.gal - 0.12, 1.15, 0.5, t, id) {
                    return Some(b);
                }
            }
            Centre::Wide => {
                if let Some(b) = banner(du, z, f.gal - 0.12, 0.85, 0.82, t, id) {
                    return Some(b);
                }
            }
            Centre::None => {}
        }
    }
    if let Some((du, seed)) = f.alcove_at(u) {
        if seed % 3 == 0 && z < 2.7 {
            if let Some(a) = alcove(du, z, Some(t), seed, 0.0) {
                return Some(a);
            }
        }
    }
    None
}

/// The balustrade, or the crowd's stand (flagged) through its gaps.
fn gallery(u: f32, z: f32, gal: f32, gold: bool) -> ([f32; 3], u8) {
    let rail_lo = gal + 0.34;
    let rail_hi = rail_lo + 0.42;
    let rail = if gold { mix(STONE, GOLD, 0.6) } else { STONE };
    if z >= rail_hi - 0.09 && z < rail_hi {
        return (scale(rail, 1.22), F_STATIC);
    }
    if z < rail_lo + 0.06 {
        return (scale(rail, 0.85), F_STATIC);
    }
    if z < rail_hi - 0.09 {
        let b = (u * 2.5).rem_euclid(1.0);
        if b < 0.36 {
            let round = 1.05 - 0.4 * ((b / 0.36 - 0.5).abs() * 2.0);
            return (scale(rail, round), F_STATIC);
        }
    }
    ([0.0; 3], F_CROWD)
}

fn cornice(u: f32, z: f32, gal: f32) -> [f32; 3] {
    let f = z - gal;
    let tone = if f < 0.07 {
        0.5
    } else if f < 0.26 {
        if (u * 3.4).rem_euclid(1.0) < 0.5 { 1.12 } else { 0.8 }
    } else {
        1.25
    };
    scale(STONE, tone)
}

/// Outer-wall dressing that stays put: the field below the cornice, the
/// gallery's rail above, the parapet.
fn tiers(f: &Face, u: f32, z: f32, gal: f32) -> ([f32; 3], u8) {
    if z >= 5.6 {
        return ([0.0; 3], F_TOP);
    }
    if z >= 5.28 {
        // Crenellated parapet.
        let merlon = (u * 1.25).rem_euclid(1.0) < 0.62;
        if z > 5.42 && !merlon {
            return ([0.0; 3], F_SKY);
        }
        let lip = if z > 5.5 { 1.28 } else { 1.0 };
        return (scale(STONE, lip * (0.9 + 0.2 * u01(hash((u * 1.25) as i32, 1, 0x71)))), F_STATIC);
    }
    if z >= gal + 0.34 {
        return gallery(u, z, gal, f.kind == Kind::Box);
    }
    if z >= gal {
        return (cornice(u, z, gal), F_STATIC);
    }
    if z < 0.3 {
        let mut a = scale(ashlar(u, z, 0.3, f.seed ^ 0x13, 0.0), 0.8);
        if z > 0.26 {
            a = scale(a, 1.25);
        }
        return (a, F_STATIC);
    }
    if let Some((du, seed)) = f.alcove_at(u) {
        if let Some(a) = alcove(du, z, None, seed, 0.0) {
            return (a.a, F_STATIC);
        }
    }
    (ashlar(u, z, 0.5, f.seed, 0.0), F_STATIC)
}

fn pillar(f: &Face, u: f32, z: f32) -> [f32; 3] {
    let len = f.hi - f.lo;
    let edge = u.min(len - u);
    let mut a = if z >= 2.88 {
        let g = z - 2.88;
        scale(STONE, if g < 0.09 { 0.5 } else if g < 0.42 { if (u * 3.3).rem_euclid(1.0) < 0.5 { 1.1 } else { 0.8 } } else { 1.25 })
    } else if z < 0.4 {
        let mut a = scale(ashlar(u, z, 0.4, f.seed ^ 0x21, 0.0), 0.8);
        if z > 0.35 {
            a = scale(a, 1.25);
        }
        a
    } else {
        ashlar(u, z, 0.55, f.seed, 0.0)
    };
    if edge < 0.14 {
        a = scale(a, if edge < 0.05 { 1.24 } else { 1.1 });
    }
    a
}

fn gate(f: &Face, u: f32, z: f32) -> ([f32; 3], u8) {
    let du = u - (f.hi - f.lo) * 0.5;
    let (a, zs) = (1.7, 1.45);
    let inside = |r: f32| du.abs() < r && (z < zs || (du * du + (z - zs) * (z - zs)) < r * r);
    if inside(a) {
        // The mouth is black paper; the raised portcullis hangs in it, its
        // spikes just clear of the sand.
        let (lo, bars) = (0.95, 0.26);
        let b = ((du / bars).rem_euclid(1.0) - 0.5).abs() * bars;
        let on_bar = z >= lo + 0.28 && b < 0.034;
        let cross = z >= lo + 0.28 && [1.45, 2.25, 3.0].iter().any(|&c| (z - c).abs() < 0.05);
        let spike = z >= lo && z < lo + 0.3 && b < 0.05 * (z - lo) / 0.3 + 0.004;
        if on_bar || cross || spike {
            return (scale(IRON, 1.4), F_STATIC);
        }
        return ([0.008, 0.007, 0.007], F_STATIC);
    }
    if inside(a + 0.4) {
        let ang = du.atan2((z - zs).max(0.0));
        let joint = z > zs && (ang * 5.0).rem_euclid(1.0) < 0.06;
        let key = z > zs + a && du.abs() < 0.3;
        let k = if joint { 0.58 } else if key { 1.3 } else { 1.15 };
        return (scale(STONE, k), F_STATIC);
    }
    // Piers either side, fluted.
    if du.abs() < a + 1.05 && z < 3.6 && z > 0.3 {
        let flute = ((du * 7.0).rem_euclid(1.0) - 0.5).abs();
        return (scale(ashlar(u, z, 0.5, f.seed, 0.0), 0.85 + 0.35 * flute), F_STATIC);
    }
    tiers(f, u, z, f.gal)
}

/// How a face is baked: its colour and what lives on it.
fn fixed(f: &Face, u: f32, z: f32) -> ([f32; 3], u8) {
    match f.kind {
        Kind::Pillar => (pillar(f, u, z), F_STATIC),
        Kind::Gate => gate(f, u, z),
        Kind::Outer | Kind::Box => tiers(f, u, z, f.gal),
    }
}

pub(super) const F_STATIC: u8 = 0;
pub(super) const F_CROWD: u8 = 1;
pub(super) const F_SKY: u8 = 2;
pub(super) const F_TOP: u8 = 3;
const RES: f32 = 10.0;

/// A face baked at ten dots to the unit and at two blurrier copies: each
/// texel is its colour and, last, what lives there (`F_*`).
pub(super) struct Tex {
    w: [usize; 3],
    h: [usize; 3],
    px: [Vec<[u8; 4]>; 3],
}

impl Tex {
    pub(super) fn empty() -> Tex {
        Tex { w: [1; 3], h: [1; 3], px: [vec![[0; 4]], vec![[0; 4]], vec![[0; 4]]] }
    }

    pub(super) fn bake(f: &Face) -> Tex {
        let (w, h) = (((f.hi - f.lo) * RES).ceil() as usize, (f.h * RES).ceil() as usize);
        let mut px = Vec::with_capacity(w * h);
        for ty in 0..h {
            for tx in 0..w {
                let (c, flag) = fixed(f, (tx as f32 + 0.5) / RES, (ty as f32 + 0.5) / RES);
                px.push([(c[0] * 255.0).clamp(0.0, 255.0) as u8, (c[1] * 255.0).clamp(0.0, 255.0) as u8, (c[2] * 255.0).clamp(0.0, 255.0) as u8, flag]);
            }
        }
        let l0 = (w, h, px);
        let down = |l: &(usize, usize, Vec<[u8; 4]>)| {
            let (w, h) = ((l.0 / 2).max(1), (l.1 / 2).max(1));
            let mut px = Vec::with_capacity(w * h);
            for y in 0..h {
                for x in 0..w {
                    let mut sum = [0u32; 3];
                    let mut flag = 0u8;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let p = l.2[((y * 2 + dy).min(l.1 - 1)) * l.0 + (x * 2 + dx).min(l.0 - 1)];
                        for c in 0..3 {
                            sum[c] += u32::from(p[c]);
                        }
                        flag = flag.max(p[3]);
                    }
                    px.push([(sum[0] / 4) as u8, (sum[1] / 4) as u8, (sum[2] / 4) as u8, flag]);
                }
            }
            (w, h, px)
        };
        let l1 = down(&l0);
        let l2 = down(&l1);
        Tex { w: [l0.0, l1.0, l2.0], h: [l0.1, l1.1, l2.1], px: [l0.2, l1.2, l2.2] }
    }

    /// The baked colour and flag at `u` along and `z` up, seen from `depth`.
    #[inline]
    pub(super) fn at(&self, u: f32, z: f32, depth: f32) -> ([f32; 3], u8) {
        let lv = if depth < 8.0 { 0 } else if depth < 17.0 { 1 } else { 2 };
        let (w, h) = (self.w[lv], self.h[lv]);
        let k = RES / (1 << lv) as f32;
        let (tx, ty) = (((u * k) as usize).min(w - 1), ((z * k) as usize).min(h - 1));
        let p = self.px[lv][ty * w + tx];
        ([f32::from(p[0]) * (1.0 / 255.0), f32::from(p[1]) * (1.0 / 255.0), f32::from(p[2]) * (1.0 / 255.0)], p[3])
    }
}

/// The wall dressed for one dot: the baked stone, with what moves over it.
#[inline]
pub(super) fn wall(f: &Face, u: f32, z: f32, depth: f32, t: u32, fever: f32) -> Option<Texel> {
    // Close to, the bake would show its dots as blocks: paint the stone itself.
    let (rgb, flag) = if depth < 2.8 { fixed(f, u, z) } else { f.tex.at(u, z, depth) };
    let base = match flag {
        F_SKY => return None,
        F_TOP => return top(f, u, z, t),
        F_CROWD => crowd(u, z, f.gal + 0.76, t, fever, ((depth - 6.0) / 14.0).clamp(0.0, 1.0)),
        _ => Texel { a: rgb, e: [0.0; 3] },
    };
    if let Some(o) = overlay(f, u, z, t, rgb) {
        return Some(o);
    }
    Some(base)
}
