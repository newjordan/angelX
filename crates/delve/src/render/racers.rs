//! The knight racers' art: armored knights in chariot cockpits, each towed
//! by two gothic heavy-metal engines, and the hell dungeon (or the daylit
//! canyon) they race through. Low-poly world3d meshes and a flat-shaded
//! raster with its own light and palette, so silhouettes stay crisp.
//!
//! The race itself (`examples/knight_racers.rs`) and the cockpit's loop card
//! (`card`) draw from this one copy of the track and the knights.

use super::world3d::math::{V3, v3};
use super::world3d::mesh::{Mesh, Tri, mat};
use super::world3d::raster::View3;
use image::{Rgb, RgbImage};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

pub mod card;

// Hardcore palette mapped onto the game's semantic materials:
// black iron -> STONE_DARK, steel -> ARMOR, Excalibur Blue -> CRYSTAL / CLOTH_BLUE, exhaust -> FLAME / EMBER.
pub const IRON: u8 = mat::STONE_DARK;
pub const STEEL: u8 = mat::ARMOR;
pub const GUN: u8 = mat::HORSE_GREY; // the engines' gunmetal, darker than a knight's steel
pub const BLUE: u8 = mat::CRYSTAL;
pub const PENNANT: u8 = mat::CLOTH_BLUE;
pub const LAVA: u8 = mat::EMBER;
pub const LAVA_HOT: u8 = mat::FLAME;
pub const H_HALF: f32 = 9.0; // hell track half-width
pub const H_LAVA: f32 = 2.6; // lava river width
pub const H_CEIL: f32 = 13.0;
/// The `uv` mark of a tile of a bent track (see `Course::shade`).
pub const BENT: [f32; 2] = [-9999.0, -9999.0];

// ---------------------------------------------------------------- meshes

pub fn tri(m: &mut Mesh, a: V3, b: V3, c: V3, k: u8) {
    m.push_tri([a, b, c], [[0.0; 2]; 3], k);
}

pub fn cuboid(m: &mut Mesh, lo: V3, hi: V3, k: u8) {
    let p = |x: bool, y: bool, z: bool| v3(if x { hi.x } else { lo.x }, if y { hi.y } else { lo.y }, if z { hi.z } else { lo.z });
    let (f, t) = (false, true);
    m.push_quad(p(f, f, t), p(t, f, t), p(t, t, t), p(f, t, t), k);
    m.push_quad(p(f, f, f), p(t, f, f), p(t, t, f), p(f, t, f), k);
    m.push_quad(p(f, f, f), p(t, f, f), p(t, f, t), p(f, f, t), k);
    m.push_quad(p(f, t, f), p(t, t, f), p(t, t, t), p(f, t, t), k);
    m.push_quad(p(f, f, f), p(f, t, f), p(f, t, t), p(f, f, t), k);
    m.push_quad(p(t, f, f), p(t, t, f), p(t, t, t), p(t, f, t), k);
}

/// n-gon prism along +x from x0 to x1, centred (y,z), radii r0 -> r1.
pub fn barrel(m: &mut Mesh, [x0, x1]: [f32; 2], [y, z]: [f32; 2], [r0, r1]: [f32; 2], n: usize, k: u8, caps: bool) {
    let ring = |x: f32, r: f32, i: usize| {
        let a = i as f32 / n as f32 * TAU + PI / n as f32;
        v3(x, y + r * a.cos(), z + r * a.sin())
    };
    for i in 0..n {
        let j = (i + 1) % n;
        m.push_quad(ring(x0, r0, i), ring(x0, r0, j), ring(x1, r1, j), ring(x1, r1, i), k);
        if caps {
            for (x, r) in [(x0, r0), (x1, r1)] {
                tri(m, v3(x, y, z), ring(x, r, i), ring(x, r, j), k);
            }
        }
    }
}

/// A pool of the racer's light on the floor under `c`: two nested ellipses, so
/// it falls off in steps (see `pool`, which dims whatever lies flat on the floor).
pub fn glow_pool(m: &mut Mesh, c: V3, [rx, ry]: [f32; 2], k: u8) {
    for (w, z) in [(1.0f32, 0.03), (0.62, 0.05)] {
        let at = |i: usize| c + v3(rx * w * (i as f32 / 12.0 * TAU).cos(), ry * w * (i as f32 / 12.0 * TAU).sin(), z);
        (0..12).for_each(|i| tri(m, v3(c.x, c.y, z), at(i), at(i + 1), k));
    }
}

/// Whether a racer's triangle is a light pool: only those lie flat on the floor.
pub fn pool(t: &Tri) -> bool {
    t.v.iter().all(|p| p.z < 0.1)
}

/// Four-sided cone from a ring at `base` (radius r) to a tip.
pub fn spike(m: &mut Mesh, base: V3, tip: V3, r: f32, k: u8) {
    let ax = (tip - base).normalize();
    let u = ax.cross(if ax.z.abs() > 0.9 { v3(1.0, 0.0, 0.0) } else { V3::UP }).normalize();
    let w = ax.cross(u);
    let at = |i: usize| {
        let (s, c) = (i as f32 / 4.0 * TAU).sin_cos();
        base + (u * c + w * s) * r
    };
    for i in 0..4 {
        tri(m, at(i), at(i + 1), tip, k);
    }
}

/// A square rod from `a` to `b`, `r` from its axis to a side: an arm, a hilt.
pub fn rod(m: &mut Mesh, a: V3, b: V3, r: f32, k: u8) {
    let ax = (b - a).normalize();
    let u = ax.cross(if ax.z.abs() > 0.9 { v3(1.0, 0.0, 0.0) } else { V3::UP }).normalize();
    let w = ax.cross(u);
    let at = |c: V3, i: usize| {
        let (s, cs) = (i as f32 / 4.0 * TAU + PI / 4.0).sin_cos();
        c + (u * cs + w * s) * r
    };
    for i in 0..4 {
        m.push_quad(at(a, i), at(a, i + 1), at(b, i + 1), at(b, i), k);
    }
    for c in [a, b] {
        m.push_quad(at(c, 0), at(c, 1), at(c, 2), at(c, 3), k);
    }
}

/// A knight on his feet at `feet`, facing +x: greaves and sabatons under the
/// knight `knight_held` builds from the hips up, holding himself as `hold` says.
pub fn standing_knight(m: &mut Mesh, feet: V3, u: f32, r: &Racer, hold: Hold) {
    for sg in [-1.0f32, 1.0] {
        let y = sg * 0.11;
        cuboid(m, feet + v3(-0.08, y - 0.07, 0.04) * u, feet + v3(0.07, y + 0.07, 0.5) * u, STEEL); // greave and cuisse
        cuboid(m, feet + v3(-0.09, y - 0.075, 0.24) * u, feet + v3(0.085, y + 0.075, 0.3) * u, IRON); // the knee
        cuboid(m, feet + v3(-0.1, y - 0.08, 0.0) * u, feet + v3(0.16, y + 0.08, 0.06) * u, IRON); // sabaton
    }
    knight_held(m, feet + v3(0.0, 0.0, 0.48) * u, u, r, hold);
}

/// The start and finish: a chequered line and a gate with a blue banner.
pub fn gate(m: &mut Mesh, course: Course) {
    let half = if course == Course::Hell { H_HALF } else { 11.0 };
    let n = 12;
    for i in 0..n {
        for j in 0..2 {
            let (y0, y1) = (-half + 2.0 * half * i as f32 / n as f32, -half + 2.0 * half * (i + 1) as f32 / n as f32);
            let (x0, x1) = (j as f32 * 1.2, (j + 1) as f32 * 1.2);
            m.push_quad(v3(x0, y0, 0.04), v3(x1, y0, 0.04), v3(x1, y1, 0.04), v3(x0, y1, 0.04), if (i + j) % 2 == 0 { STEEL } else { IRON });
        }
    }
    let edge = half + if course == Course::Hell { 1.4 } else { 0.7 };
    for sd in [-1.0f32, 1.0] {
        cuboid(m, v3(-0.4, sd * edge - 0.5, 0.0), v3(0.4, sd * edge + 0.5, 8.0), IRON);
    }
    cuboid(m, v3(-0.4, -edge - 0.5, 7.4), v3(0.4, edge + 0.5, 8.4), IRON);
    m.push_quad(v3(0.0, -edge, 7.4), v3(0.0, edge, 7.4), v3(0.0, edge, 5.2), v3(0.0, -edge, 5.2), mat::CLOTH_BLUE);
    cuboid(m, v3(-0.45, -edge, 7.0), v3(0.45, edge, 7.4), mat::FIRE);
}

/// What crowns a knight's helm: the shape that tells the four apart above the rim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Crest { Plume, Horns, Hood, Crown }

/// One racer: its engine pair, its chariot and its knight's colours and crest.
pub struct Racer { pub name: &'static str, pub len: f32, pub r: f32, pub sides: usize, pub pipes: usize, pub pipe_h: f32, pub spikes: usize, pub gap: f32, pub fins: bool, pub horns: bool, pub cart: f32, pub lance: bool, pub cloth: u8, pub gleam: u8, pub crest: Crest }

pub const RACERS: [Racer; 4] = [
    Racer { name: "Excalibur", len: 4.2, r: 0.7, sides: 6, pipes: 3, pipe_h: 1.4, spikes: 4, gap: 1.3, fins: false, horns: false, cart: 2.1, lance: true, cloth: mat::BANNER, gleam: BLUE, crest: Crest::Plume },
    Racer { name: "Bastion", len: 3.4, r: 0.95, sides: 4, pipes: 2, pipe_h: 1.9, spikes: 4, gap: 1.6, fins: false, horns: true, cart: 2.3, lance: false, cloth: mat::FOLIAGE, gleam: mat::WATER, crest: Crest::Horns },
    Racer { name: "Wraith", len: 4.8, r: 0.58, sides: 5, pipes: 3, pipe_h: 1.1, spikes: 3, gap: 1.2, fins: true, horns: false, cart: 2.0, lance: true, cloth: mat::MOONLIGHT, gleam: mat::TORCH_POOL, crest: Crest::Hood },
    Racer { name: "Crown", len: 3.8, r: 0.8, sides: 6, pipes: 2, pipe_h: 2.2, spikes: 6, gap: 1.4, fins: true, horns: true, cart: 2.2, lance: false, cloth: mat::CLOTH_GOLD, gleam: mat::BRASS, crest: Crest::Crown },
];

/// One engine, nose at +x.
pub fn engine(m: &mut Mesh, s: &Racer, y: f32, z: f32, t: f32) {
    let l = s.len;
    barrel(m, [-l * 0.5, l * 0.25], [y, z], [s.r * 0.85, s.r], s.sides, IRON, true);
    barrel(m, [l * 0.25, l * 0.4], [y, z], [s.r * 1.08; 2], s.sides, GUN, false); // armoured collar
    barrel(m, [-l * 0.18, -l * 0.12], [y, z], [s.r * 1.06; 2], s.sides, s.cloth, false); // the knight's band
    barrel(m, [l * 0.4, l * 0.5], [y, z], [s.r, s.r * 0.6], s.sides, IRON, false);
    spike(m, v3(l * 0.5, y, z), v3(l * 0.5 + s.r * 2.2, y, z), s.r * 0.55, GUN); // ram
    // ring of radial spikes on the collar
    for i in 0..s.spikes {
        let a = i as f32 / s.spikes as f32 * TAU;
        let d = v3(0.0, a.cos(), a.sin());
        let base = v3(l * 0.33, y, z) + d * s.r;
        spike(m, base, base + d * s.r * 0.9 + v3(s.r * 0.5, 0.0, 0.0), s.r * 0.18, GUN);
    }
    // cathedral-pipe exhaust stacks along the spine: a square stack, a livery band, a spire
    for i in 0..s.pipes {
        let h = s.pipe_h * (1.0 - 0.18 * (i as f32 - (s.pipes as f32 - 1.0) / 2.0).abs());
        let (px, pz, w) = (-l * 0.42 + i as f32 * (l * 0.5 / s.pipes as f32), z + s.r * 0.8, s.r * 0.16);
        cuboid(m, v3(px - w, y - w, pz), v3(px + w, y + w, pz + h), GUN);
        cuboid(m, v3(px - w * 1.3, y - w * 1.3, pz + h * 0.82), v3(px + w * 1.3, y + w * 1.3, pz + h * 0.9), s.cloth);
        spike(m, v3(px, y, pz + h), v3(px, y, pz + h + w * 3.0), w * 1.4, IRON);
    }
    // exhaust plume out the back, flickering
    let fl = 1.0 + 0.25 * (t * 23.0 + y * 3.0).sin();
    barrel(m, [-l * 0.5 - 0.02, -l * 0.5 - 0.15], [y, z], [s.r * 0.7; 2], s.sides, LAVA, true);
    spike(m, v3(-l * 0.5 - 0.15, y, z), v3(-l * 0.5 - s.r * 2.4 * fl, y, z), s.r * 0.6, mat::FIRE);
    spike(m, v3(-l * 0.5 - 0.15, y, z), v3(-l * 0.5 - s.r * 1.3 * fl, y, z), s.r * 0.34, LAVA_HOT); // hot core
    for sgn in [-1.0f32, 1.0] {
        if s.fins {
            let b = v3(-l * 0.3, y + sgn * s.r * 0.7, z);
            m.push_quad(b, b + v3(l * 0.45, 0.0, 0.0), b + v3(l * 0.2, sgn * s.r * 1.6, s.r * 0.6), b + v3(-l * 0.1, sgn * s.r * 1.6, s.r * 0.6), GUN);
        }
        if s.horns {
            let b = v3(l * 0.42, y + sgn * s.r * 0.6, z + s.r * 0.6);
            spike(m, b, b + v3(s.r * 1.4, sgn * s.r * 1.3, s.r * 1.6), s.r * 0.22, GUN);
        }
    }
}

/// Chariot cockpit with its knight standing tall in it, facing +x. The rim and
/// flanks wear the knight's colours.
pub fn chariot(m: &mut Mesh, x: f32, z: f32, s: f32, r: &Racer) {
    // tub: wedge-fronted
    cuboid(m, v3(x - 0.5 * s, -0.4 * s, z), v3(x + 0.3 * s, 0.4 * s, z + 0.45 * s), IRON);
    let (a, b) = (v3(x + 0.3 * s, -0.4 * s, z), v3(x + 0.3 * s, 0.4 * s, z));
    let tip = v3(x + 0.85 * s, 0.0, z + 0.1 * s);
    let (at, bt) = (a + v3(0.0, 0.0, 0.45 * s), b + v3(0.0, 0.0, 0.45 * s));
    tri(m, a, at, tip, GUN);
    tri(m, b, bt, tip, GUN);
    tri(m, at, bt, tip, GUN);
    cuboid(m, v3(x - 0.52 * s, -0.42 * s, z + 0.42 * s), v3(x + 0.32 * s, 0.42 * s, z + 0.47 * s), r.cloth); // rim trim
    for sgn in [-1.0f32, 1.0] {
        cuboid(m, v3(x - 0.45 * s, sgn * 0.4 * s - 0.01 * s, z + 0.12 * s), v3(x + 0.25 * s, sgn * 0.4 * s + 0.01 * s, z + 0.3 * s), r.cloth); // flank stripe
        barrel(m, [x - 0.3 * s, x - 0.1 * s], [sgn * 0.48 * s, z + 0.1 * s], [0.32 * s; 2], 8, GUN, true); // hover discs
    }
    knight(m, v3(x - 0.1 * s, 0.0, z + 0.45 * s), s * 1.45, r);
}

/// How a knight holds himself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hold {
    /// In the cockpit, both hands to the reins, and his lance if he rides with one.
    Reins,
    /// On his feet, his sword arm out ahead (+x) and raised this many radians,
    /// calling his shot at something far off.
    Point(f32),
}

/// A knight from the cockpit floor `k` up, `u` tall-ish: cuirass, tabard and cape
/// and pauldrons in their colours, a great helm, and the crest that is theirs.
pub fn knight(m: &mut Mesh, k: V3, u: f32, r: &Racer) {
    knight_held(m, k, u, r, Hold::Reins);
}

/// A knight as `knight` builds him, holding himself as `hold` says.
pub fn knight_held(m: &mut Mesh, k: V3, u: f32, r: &Racer, hold: Hold) {
    let (cl, gl) = (r.cloth, r.gleam);
    let b = |m: &mut Mesh, lo: [f32; 3], hi: [f32; 3], c: u8| cuboid(m, k + v3(lo[0], lo[1], lo[2]) * u, k + v3(hi[0], hi[1], hi[2]) * u, c);
    b(m, [-0.16, -0.22, 0.0], [0.16, 0.22, 0.5], STEEL); // cuirass
    b(m, [-0.18, -0.25, 0.0], [0.18, 0.25, 0.13], cl); // tassets
    b(m, [0.16, -0.15, 0.1], [0.19, 0.15, 0.5], cl); // tabard on the chest
    let cape = |m: &mut Mesh, w0: f32, w1: f32, c: u8| m.push_quad(k + v3(-0.2, -w0, 0.4) * u, k + v3(-0.2, w0, 0.4) * u, k + v3(-0.2, w1, 0.04) * u, k + v3(-0.2, -w1, 0.04) * u, c);
    cape(m, 0.17, 0.1, cl); // cape on the back, tapering to the hem
    b(m, [-0.215, -0.02, 0.06], [-0.2, 0.02, 0.4], gl);
    let (sh, hi) = if r.crest == Crest::Horns || r.crest == Crest::Crown { (0.38, 0.58) } else { (0.33, 0.54) };
    for sg in [-1.0f32, 1.0] {
        b(m, [-0.14, sg * 0.2 - 0.05, 0.36], [0.12, sg * sh + 0.05, hi], cl); // pauldrons
        b(m, [-0.15, sg * 0.2 - 0.06, hi], [0.13, sg * sh + 0.06, hi + 0.04], STEEL);
    }
    b(m, [-0.09, -0.12, 0.5], [0.09, 0.12, 0.57], STEEL); // gorget
    b(m, [-0.13, -0.13, 0.57], [0.13, 0.13, 0.86], STEEL); // great helm
    b(m, [0.13, -0.09, 0.69], [0.145, 0.09, 0.74], gl); // visor slit glow
    match hold {
        Hold::Reins => {
            // arms to the reins
            b(m, [0.0, -0.3, 0.2], [0.35, -0.21, 0.3], IRON);
            b(m, [0.0, 0.21, 0.2], [0.35, 0.3, 0.3], IRON);
        }
        Hold::Point(raise) => {
            // the sword arm out from the right shoulder, the blade on along it
            let shoulder = k + v3(0.02, -0.27, 0.46) * u;
            let dir = v3(raise.cos(), -0.12, raise.sin()).normalize();
            let hand = shoulder + dir * (0.46 * u);
            rod(m, shoulder, hand, 0.055 * u, STEEL);
            cuboid(m, hand - v3(0.05, 0.05, 0.05) * u, hand + v3(0.05, 0.05, 0.05) * u, IRON); // gauntlet
            let side = dir.cross(V3::UP).normalize();
            rod(m, hand - side * (0.14 * u), hand + side * (0.14 * u), 0.025 * u, IRON); // crossguard
            spike(m, hand + dir * (0.02 * u), hand + dir * (1.25 * u), 0.04 * u, gl); // the blade, in his gleam
            // the other arm down at his side, the hand at the hip
            rod(m, k + v3(0.0, 0.29, 0.44) * u, k + v3(0.06, 0.31, 0.12) * u, 0.05 * u, IRON);
        }
    }
    let top = v3(0.0, 0.0, 0.86);
    let sp = |m: &mut Mesh, from: V3, to: V3, w: f32, c: u8| spike(m, k + from * u, k + to * u, w * u, c);
    match r.crest {
        Crest::Plume => {
            sp(m, top, v3(-0.34, 0.0, 1.22), 0.08, cl);
            sp(m, top + v3(0.04, 0.0, 0.0), v3(-0.12, 0.0, 1.0), 0.06, gl);
        }
        Crest::Horns => {
            for sg in [-1.0f32, 1.0] {
                sp(m, v3(0.0, sg * 0.11, 0.8), v3(0.1, sg * 0.36, 1.12), 0.07, STEEL);
            }
            // tower shield strapped across the back
            b(m, [-0.32, -0.3, 0.02], [-0.25, 0.3, 0.62], STEEL);
            b(m, [-0.34, -0.25, 0.08], [-0.3, 0.25, 0.56], cl);
            b(m, [-0.36, -0.07, 0.24], [-0.3, 0.07, 0.4], gl);
            for sg in [-1.0f32, 1.0] {
                sp(m, v3(0.0, sg * 0.34, 0.58), v3(-0.04, sg * 0.4, 0.86), 0.06, STEEL);
            }
        }
        Crest::Hood => {
            sp(m, v3(0.0, 0.0, 0.8), v3(-0.1, 0.0, 1.32), 0.14, cl);
            // long streaming cloak
            let (a, c) = (k + v3(-0.2, 0.0, 0.5) * u, k + v3(-0.8, 0.0, 0.18) * u);
            m.push_quad(a + v3(0.0, -0.24, 0.0) * u, c + v3(0.0, -0.34, 0.0) * u, c + v3(0.0, 0.34, 0.0) * u, a + v3(0.0, 0.24, 0.0) * u, cl);
        }
        Crest::Crown => {
            for i in 0..5 {
                let a = i as f32 / 5.0 * TAU;
                let at = v3(0.075 * a.cos(), 0.1 * a.sin(), 0.86);
                sp(m, at, at + v3(0.04 * a.cos(), 0.05 * a.sin(), 0.22), 0.04, cl);
            }
            sp(m, top + v3(-0.04, 0.0, 0.0), v3(-0.3, 0.0, 1.3), 0.07, gl);
            b(m, [-0.19, -0.4, 0.3], [-0.12, 0.4, 0.5], cl); // mantle
        }
    }
    if r.lance && hold == Hold::Reins {
        let base = k + v3(-0.2, -0.35, 0.25) * u;
        barrel(m, [base.x, base.x + 1.4 * u], [base.y, base.z + 0.35 * u], [0.03 * u; 2], 4, STEEL, false);
        m.push_quad(base + v3(-0.1, 0.0, 0.0) * u, base + v3(-0.1, 0.0, 1.0) * u, base + v3(-0.55, 0.0, 0.88) * u, base + v3(-0.55, 0.0, 0.52) * u, cl); // banner
        barrel(m, [base.x - 0.1 * u, base.x - 0.08 * u], [base.y, base.z + 0.45 * u], [0.02 * u; 2], 4, IRON, false);
        cuboid(m, base + v3(-0.11, -0.01, 0.0) * u, base + v3(-0.09, 0.01, 1.05) * u, IRON);
    }
}

/// Jagged blue energy arc between two points, re-rolled each frame.
pub fn arc(m: &mut Mesh, a: V3, b: V3, t: f32, seed: f32) {
    let mut prev = a;
    for i in 1..=5 {
        let mut p = a.lerp(b, i as f32 / 5.0);
        if i < 5 {
            let j = ((t * 31.0 + seed + i as f32 * 1.7).sin() * 43758.5).fract() - 0.5;
            let k = ((t * 17.0 + seed * 2.3 + i as f32 * 2.9).sin() * 24634.6).fract() - 0.5;
            p += v3(j * 0.25, 0.0, k * 0.3);
        }
        let w = v3(0.0, 0.0, 0.07);
        m.push_quad(prev - w, p - w, p + w, prev + w, BLUE);
        prev = p;
    }
}

/// A whole racer appended to `m` at `at`: two engines, an arc between them,
/// tow tethers, the chariot behind.
pub fn racer(m: &mut Mesh, i: usize, t: f32, at: V3) {
    let (s, first) = (&RACERS[i], m.tris.len());
    let z = 1.2 + 0.06 * (t * 9.0 + i as f32).sin();
    let gy = s.gap + s.r;
    engine(m, s, -gy, z, t);
    engine(m, s, gy, z + 0.03, t);
    let x = -s.len * 0.05;
    arc(m, v3(x, -gy + s.r * 0.9, z + 0.1), v3(x, gy - s.r * 0.9, z + 0.1), t, i as f32 * 5.0);
    let cx = -s.len * 0.5 - 2.0 * s.cart;
    for sgn in [-1.0f32, 1.0] {
        let (a, b, w) = (v3(-s.len * 0.45, sgn * gy, z), v3(cx + 0.2 * s.cart, sgn * 0.3 * s.cart, 0.55 * s.cart), v3(0.0, 0.0, 0.035));
        m.push_quad(a - w, b - w, b + w, a + w, BLUE); // energy tether
    }
    chariot(m, cx, 0.35 * s.cart, s.cart, s);
    glow_pool(m, v3(cx, 0.0, 0.0), [1.0 * s.cart, 0.75 * s.cart], s.cloth);
    for sgn in [-1.0f32, 1.0] {
        glow_pool(m, v3(0.0, sgn * gy, 0.0), [s.len * 0.75, s.r * 1.7], s.cloth);
    }
    m.tris[first..].iter_mut().flat_map(|tr| &mut tr.v).for_each(|p| *p += at);
}

/// A racer's place in the pack at time `t`: a zigzag down the track, each racer
/// a length behind the one that was two before it and a stride behind the one
/// before, no two on the same line of sight; each breathes a little so the
/// pack lives.
pub fn lane(i: usize, t: f32) -> V3 {
    let ([x, y], speed) = ([[0.0, -5.4], [-17.0, 5.4], [-5.5, 3.4], [-11.5, -1.0]][i], 21.0 + [0.0, 0.1, -0.05, 0.05][i]);
    let (sx, sy) = ((t * 1.1 + i as f32 * 1.7).sin(), (t * 0.9 + i as f32 * 1.9).sin());
    v3(x + speed * t + 0.8 * sx, y + 0.5 * sy, 0.0)
}

// ---------------------------------------------------------------- the dungeon

/// Where the race runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Course { Hell, Canyon }

impl Course {
    pub fn world(self) -> Mesh {
        match self {
            Course::Hell => dungeon(),
            Course::Canyon => canyon(),
        }
    }

    /// What a dot that hits nothing shows, `k` down the frame (0 top, 1 bottom).
    pub fn sky(self, k: f32) -> [u8; 3] {
        match self {
            Course::Hell => [0; 3], // no sky: whatever the vault leaves open is black
            Course::Canyon => {
                let q = 1.0 - k;
                [(236.0 - 30.0 * q) as u8, (240.0 - 22.0 * q) as u8, (246.0 - 8.0 * q) as u8]
            }
        }
    }

    /// A material's flat colour here, and whether it glows (is never shaded).
    /// Racers (`mover`) get a lifted gunmetal, so they stand off the floor.
    pub fn paint(self, m: u8, mover: bool) -> ([f32; 3], bool) {
        // the knights' liveries read the same on both courses
        let livery = match m {
            mat::BANNER => Some(([60.0, 118.0, 234.0], false)),
            mat::FOLIAGE => Some(([40.0, 148.0, 98.0], false)),
            mat::MOONLIGHT => Some(([132.0, 82.0, 208.0], false)),
            mat::CLOTH_GOLD => Some(([214.0, 164.0, 52.0], false)),
            mat::WATER => Some(([96.0, 232.0, 160.0], true)),
            mat::TORCH_POOL => Some(([186.0, 132.0, 255.0], true)),
            mat::BRASS => Some(([255.0, 208.0, 104.0], true)),
            _ => None,
        };
        if let Some(l) = livery {
            return l;
        }
        let glow = matches!(m, BLUE | PENNANT | LAVA | LAVA_HOT | mat::FIRE | mat::WINDOW);
        let rgb = match self {
            Course::Hell => match m {
                IRON if mover => [50.0, 52.0, 66.0],
                IRON => [20.0, 20.0, 24.0],
                STEEL if mover => [166.0, 174.0, 192.0],
                GUN if mover => [82.0, 88.0, 108.0],
                STEEL => [150.0, 158.0, 172.0],
                BLUE | PENNANT => [61.0, 139.0, 255.0],
                LAVA => [225.0, 6.0, 0.0],
                LAVA_HOT => [255.0, 160.0, 48.0],
                mat::FIRE => [226.0, 78.0, 12.0],
                mat::BONE => [196.0, 184.0, 160.0],
                mat::STONE => [44.0, 42.0, 46.0],
                mat::ROCK => [26.0, 24.0, 28.0],
                mat::PATH => [38.0, 34.0, 34.0],
                _ => [60.0, 60.0, 60.0],
            },
            Course::Canyon => match m {
                IRON if mover => [40.0, 44.0, 58.0],
                IRON => [26.0, 28.0, 34.0],
                STEEL if mover => [118.0, 128.0, 148.0],
                GUN if mover => [62.0, 68.0, 86.0],
                STEEL => [176.0, 186.0, 200.0],
                BLUE | PENNANT => [61.0, 139.0, 255.0],
                LAVA_HOT => [255.0, 214.0, 120.0],
                LAVA | mat::FIRE | mat::WINDOW => [255.0, 150.0, 60.0],
                mat::ROCK => [150.0, 156.0, 168.0],
                mat::STONE => [196.0, 200.0, 208.0],
                mat::PATH => [212.0, 214.0, 218.0],
                _ => [128.0, 128.0, 128.0],
            },
        };
        (rgb, glow)
    }

    /// Distance haze: far things sink into the dark of the vault, or the pale of
    /// the canyon. Racers are hazed a third as much, so the pack stays clear.
    pub fn fog(self, c: [f32; 3], dist: f32, mover: bool) -> [f32; 3] {
        let (to, density) = match self {
            Course::Hell => ([14.0, 6.0, 10.0], 0.016),
            Course::Canyon => ([226.0, 230.0, 236.0], 0.010),
        };
        let f = 1.0 - (-dist * density * if mover { 0.33 } else { 1.0 }).exp();
        std::array::from_fn(|i| c[i] + (to[i] - c[i]) * f)
    }

    /// A triangle's colour seen from `eye`, `light` the sun (the key light, for a
    /// racer). The track is lit as it was: a hard sun on the canyon; in hell a
    /// lava rim light from both rivers (only faces turned toward one catch it,
    /// hard) plus a cold overhead fill. Racers are lit in tonal steps, arcade
    /// style: four flat bands from the key light, shadows cooled and lights warmed.
    pub fn shade(self, t: &Tri, eye: V3, light: V3, mover: bool) -> [u8; 3] {
        let (base, glow) = self.paint(t.mat, mover);
        let cen = (t.v[0] + t.v[1] + t.v[2]) / 3.0;
        let flip = t.normal.dot(eye - cen) < 0.0;
        let n = if flip { -t.normal } else { t.normal };
        let c = if glow {
            base
        } else if pool(t) && mover {
            // in hell the racer's colour pooled dim on the floor; in daylight its shadow
            let step = t.v[0].z > 0.04;
            match self {
                Course::Hell => base.map(|c| c * if step { 0.42 } else { 0.24 }),
                Course::Canyon => if step { [150.0, 154.0, 166.0] } else { [178.0, 182.0, 192.0] },
            }
        } else if mover {
            let (k, tint) = match n.dot(light) {
                d if d < 0.12 => (0.45, [0.82, 0.9, 1.12]),
                d if d < 0.4 => (0.68, [0.92, 0.96, 1.05]),
                d if d < 0.7 => (0.9, [1.0, 1.0, 1.0]),
                _ => (1.08, [1.06, 1.02, 0.94]),
            };
            std::array::from_fn(|i| base[i] * k * tint[i])
        } else if self == Course::Canyon {
            base.map(|c| c * (0.55 + 0.6 * n.dot(light).max(0.0)))
        } else {
            let mut c = [base[0] * 0.8, base[1] * 0.8, base[2] * 0.85];
            // across the track: a bent track's tiles carry their pre-bend lateral frame in `uv`
            let (at, face) = if t.uv[2] == BENT { (v3(0.0, t.uv[0][0], t.uv[0][1]), v3(0.0, t.uv[1][0], t.uv[1][1]) * if flip { -1.0 } else { 1.0 }) } else { (cen, n) };
            for side in [-1.0f32, 1.0] {
                let d = v3(at.x, side * (H_HALF + H_LAVA * 0.5), -0.2) - at;
                let k = 2.2 * (-d.length() / 4.5).exp() * face.dot(d.normalize()).max(0.0).powi(2);
                c = [c[0] + 240.0 * k, c[1] + 70.0 * k, c[2] + 10.0 * k];
            }
            let top = n.z.max(0.0) * 14.0;
            [c[0], c[1] + top, c[2] + top * 1.6]
        };
        self.fog(c, (cen - eye).length(), mover).map(|c| c.clamp(0.0, 255.0) as u8)
    }
}

pub fn skull(m: &mut Mesh, c: V3, s: f32, face: f32) {
    cuboid(m, c + v3(-s, -s, 0.0), c + v3(s, s, 1.6 * s), mat::BONE);
    cuboid(m, c + v3(-0.6 * s, -s, -0.6 * s), c + v3(0.6 * s, s, 0.0), mat::BONE); // jaw
    for dx in [-0.45f32, 0.45] {
        // eye sockets glow lava
        cuboid(m, c + v3((dx - 0.22) * s, 0.95 * face * s, 0.7 * s), c + v3((dx + 0.22) * s, 1.05 * face * s, 1.1 * s), LAVA);
    }
}

pub fn chain(m: &mut Mesh, a: V3, b: V3) {
    let n = ((b - a).length() / 0.5) as usize + 1;
    for i in 0..n {
        let (p0, p1) = (a.lerp(b, i as f32 / n as f32), a.lerp(b, (i as f32 + 0.8) / n as f32));
        let w = if i % 2 == 0 { v3(0.09, 0.0, 0.0) } else { v3(0.0, 0.09, 0.0) };
        m.push_quad(p0 - w, p1 - w, p1 + w, p0 + w, IRON);
    }
}

/// A wave's rate, nudged so it fits a whole number of times into `period`
/// (none: as it is). A track that loops (`play.rs`) closes on itself unseen.
pub fn rate(w: f32, period: f32) -> f32 {
    if period > 0.0 { (w * period / TAU).round().max(1.0) * TAU / period } else { w }
}

/// Hell dungeon along +x: a vaulted tunnel, lava rivers either side of the
/// track, chained gothic pillars topped with braziers, hanging cages, skulls.
pub fn dungeon() -> Mesh {
    dungeon_span(-60.0, 300.0, 0.0)
}

/// The dungeon from `x0` to `x1`; with a `period`, its waves repeat every
/// `period` (a multiple of 48, which the pillars and cages keep to) and the two ends meet.
pub fn dungeon_span(x0: f32, x1: f32, period: f32) -> Mesh {
    let mut m = Mesh::new();
    let (h, lw, ceil) = (H_HALF, H_LAVA, H_CEIL);
    let wall = h + lw;
    for k in 0..((x1 - x0) / 1.5) as usize {
        let x = x0 + k as f32 * 1.5;
        let nx = x + 1.5;
        // flagstone track in six checkered strips
        for j in 0..6 {
            let (ya, yb) = (-h + j as f32 * h / 3.0, -h + (j + 1) as f32 * h / 3.0);
            m.push_quad(v3(x, ya, 0.0), v3(nx, ya, 0.0), v3(nx, yb, 0.0), v3(x, yb, 0.0), if (j + k) % 2 == 0 { mat::PATH } else { mat::STONE });
        }
        for sd in [-1.0f32, 1.0] {
            // lava river, sunk a little, with hot cracks
            let hot = ((x * rate(0.71, period)).sin() + (x * rate(0.23, period)).cos()) > 0.9;
            m.push_quad(v3(x, sd * h, -0.3), v3(nx, sd * h, -0.3), v3(nx, sd * wall, -0.3), v3(x, sd * wall, -0.3), if hot { LAVA_HOT } else { LAVA });
            m.push_quad(v3(x, sd * h, 0.0), v3(nx, sd * h, 0.0), v3(nx, sd * h, -0.3), v3(x, sd * h, -0.3), mat::STONE);
            // jagged wall up to the vault spring line, then the vault
            let (j0, j1) = (0.4 * (x * rate(1.7, period) + sd).sin(), 0.4 * (nx * rate(1.7, period) + sd).sin());
            m.push_quad(v3(x, sd * wall, -0.3), v3(nx, sd * wall, -0.3), v3(nx, sd * (wall + 0.6 + j1), 6.0), v3(x, sd * (wall + 0.6 + j0), 6.0), mat::ROCK);
            let p = |xx: f32, a: f32| v3(xx, sd * (wall + 0.6) * a.cos(), 6.0 + (ceil - 6.0) * a.sin());
            for seg in 0..3 {
                let (a0, a1) = (seg as f32 / 3.0 * FRAC_PI_2, (seg + 1) as f32 / 3.0 * FRAC_PI_2);
                m.push_quad(p(x, a0), p(nx, a0), p(nx, a1), p(x, a1), mat::ROCK);
            }
            // lava cracks reaching into the track
            if k % 7 == 3 {
                tri(&mut m, v3(x, sd * h, 0.02), v3(x + 1.4, sd * h, 0.02), v3(x + 0.5, sd * h - sd * 2.2, 0.02), LAVA);
            }
        }
    }
    // vault ribs, pillars, braziers, chains, skulls, cages
    for n in 0..((x1 - 4.0) / 12.0).ceil() as usize {
        let x = 4.0 + n as f32 * 12.0;
        let p = |xx: f32, a: f32| v3(xx, (wall + 0.3) * a.cos(), 6.0 + (ceil - 6.0) * a.sin() - 0.3);
        for seg in 0..6 {
            let (a0, a1) = (seg as f32 / 6.0 * PI, (seg + 1) as f32 / 6.0 * PI);
            m.push_quad(p(x, a0), p(x + 0.8, a0), p(x + 0.8, a1), p(x, a1), mat::STONE); // rib arch across the ceiling
        }
        for sd in [-1.0f32, 1.0] {
            let py = sd * (h + lw * 0.5);
            // pillar standing in the lava, with buttress and spire capital
            cuboid(&mut m, v3(x - 0.7, py - 0.7, -0.3), v3(x + 0.7, py + 0.7, 6.0), mat::STONE);
            cuboid(&mut m, v3(x - 1.0, py - 1.0, -0.3), v3(x + 1.0, py + 1.0, 0.8), IRON);
            cuboid(&mut m, v3(x - 0.95, py - 0.95, 5.0), v3(x + 0.95, py + 0.95, 5.4), IRON);
            for (dx, dy) in [(-0.8f32, -0.8f32), (0.8, -0.8), (-0.8, 0.8), (0.8, 0.8)] {
                spike(&mut m, v3(x + dx, py + dy, 5.4), v3(x + dx * 1.2, py + dy * 1.2, 6.4), 0.18, IRON);
            }
            // brazier on the capital
            cuboid(&mut m, v3(x - 0.6, py - 0.6, 5.4), v3(x + 0.6, py + 0.6, 5.8), IRON);
            spike(&mut m, v3(x, py, 5.8), v3(x, py, 7.2 + 0.3 * ((n as f32) * 1.3).sin()), 0.5, mat::FIRE);
            spike(&mut m, v3(x, py, 5.8), v3(x + 0.1, py, 6.6), 0.28, LAVA_HOT);
            // chains from pillar to wall
            chain(&mut m, v3(x, py, 4.6), v3(x + 1.5, sd * (wall + 0.6), 2.2));
            chain(&mut m, v3(x, py, 4.6), v3(x - 1.5, sd * (wall + 0.6), 2.2));
            // skull on the pillar base, facing the track
            skull(&mut m, v3(x, py - sd * 1.05, 1.0), 0.32, -sd);
        }
        if n % 2 == 1 {
            // hanging cage over the lava edge
            let cy = if n % 4 == 1 { -h + 0.5 } else { h - 0.5 };
            let (cx, cz) = (x + 5.0, 8.0);
            chain(&mut m, v3(cx, cy, ceil - 0.5), v3(cx, cy, cz + 1.8));
            cuboid(&mut m, v3(cx - 0.7, cy - 0.7, cz), v3(cx + 0.7, cy + 0.7, cz + 0.15), IRON);
            cuboid(&mut m, v3(cx - 0.7, cy - 0.7, cz + 1.7), v3(cx + 0.7, cy + 0.7, cz + 1.85), IRON);
            for (dx, dy) in [(-0.65f32, -0.65f32), (0.0, -0.65), (0.65, -0.65), (-0.65, 0.65), (0.0, 0.65), (0.65, 0.65), (-0.65, 0.0), (0.65, 0.0)] {
                cuboid(&mut m, v3(cx + dx - 0.05, cy + dy - 0.05, cz), v3(cx + dx + 0.05, cy + dy + 0.05, cz + 1.8), IRON);
            }
            skull(&mut m, v3(cx, cy, cz + 0.15), 0.25, -1.0);
        }
    }
    m
}

pub fn rock_h(x: f32, side: f32, period: f32) -> f32 {
    3.5 + 1.8 * (x * rate(0.37, period) + side).sin().abs() + 1.2 * (x * rate(1.13, period) + side * 3.0).sin()
}

/// Canyon run with castle ruins along both rims, track along +x.
pub fn canyon() -> Mesh {
    canyon_span(-60.0, 300.0, 0.0)
}

/// The canyon from `x0` to `x1`, its rims repeating every `period` if there is one.
pub fn canyon_span(x0: f32, x1: f32, period: f32) -> Mesh {
    let mut m = Mesh::new();
    let half = 11.0f32;
    let step = 3.0;
    let mut x = x0;
    while x < x1 {
        let nx = x + step;
        m.push_quad(v3(x, -half, 0.0), v3(nx, -half, 0.0), v3(nx, half, 0.0), v3(x, half, 0.0), mat::PATH);
        for side in [-1.0f32, 1.0] {
            let (h0, h1) = (rock_h(x, side, period), rock_h(nx, side, period));
            let o = 2.0 + (x * rate(0.7, period) + side).sin() * 0.8;
            let o1 = 2.0 + (nx * rate(0.7, period) + side).sin() * 0.8;
            let (a, b) = (v3(x, side * half, 0.0), v3(nx, side * half, 0.0));
            let (c, d) = (v3(nx, side * (half + o1), h1), v3(x, side * (half + o), h0));
            m.push_quad(a, b, c, d, mat::ROCK);
            m.push_quad(d, c, v3(nx, side * 30.0, h1), v3(x, side * 30.0, h0), mat::ROCK);
        }
        x = nx;
    }
    // racing-line markers: blue chevrons every 8 tiles
    let mut x = 0.0;
    while x < x1 {
        for sgn in [-1.0f32, 1.0] {
            tri(&mut m, v3(x, sgn * 10.4, 0.01), v3(x + 2.0, sgn * 9.6, 0.01), v3(x, sgn * 8.8, 0.01), BLUE);
        }
        x += 8.0;
    }
    // ruined towers & arches on the rims
    let mut x = 6.0;
    let mut n = 0;
    while x < x1 {
        let side = if n % 2 == 0 { 1.0 } else { -1.0 };
        let y = side * (half + 4.5);
        let z = rock_h(x, side, period) - 0.5;
        let h = 4.0 + (n % 3) as f32 * 1.6;
        cuboid(&mut m, v3(x - 1.2, y - 1.2, z), v3(x + 1.2, y + 1.2, z + h), mat::STONE);
        for (cx, cy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0), (0.0, -1.0), (0.0, 1.0)] {
            if (n + (cx as i32 + 2) as usize).is_multiple_of(4) {
                continue; // broken merlons
            }
            cuboid(&mut m, v3(x + cx - 0.25, y + cy - 0.25, z + h), v3(x + cx + 0.25, y + cy + 0.25, z + h + 0.6), mat::STONE);
        }
        cuboid(&mut m, v3(x - 0.3, y - side * 1.22, z + h * 0.6), v3(x + 0.3, y - side * 1.25, z + h * 0.8), mat::WINDOW);
        // hanging blue banner toward the track
        let by = y - side * 1.26;
        m.push_quad(v3(x - 0.5, by, z + h - 0.3), v3(x + 0.5, by, z + h - 0.3), v3(x + 0.5, by, z + h - 2.6), v3(x - 0.5, by, z + h - 2.6), PENNANT);
        if n % 3 == 1 {
            // ruined arch spanning the canyon
            let ah = 7.5;
            cuboid(&mut m, v3(x + 6.0, -half - 1.0, 0.0), v3(x + 7.0, -half, ah), mat::STONE);
            cuboid(&mut m, v3(x + 6.0, half, 0.0), v3(x + 7.0, half + 1.0, ah), mat::STONE);
            cuboid(&mut m, v3(x + 6.0, -half - 1.0, ah), v3(x + 7.0, -1.5, ah + 1.0), mat::STONE);
            cuboid(&mut m, v3(x + 6.0, 1.2, ah), v3(x + 7.0, half + 1.0, ah + 1.0), mat::STONE);
            cuboid(&mut m, v3(x + 6.4, -half, 0.0), v3(x + 6.6, -half + 0.3, 2.2), mat::FIRE);
        }
        x += 11.0;
        n += 1;
    }
    m
}

/// The race camera, given the pack's mean position: a swinging chase over the
/// canyon; in hell three shots (high chase, low side tracking with the knights
/// in profile, high three-quarter chase).
pub fn race_view(course: Course, t: f32, mean: V3) -> View3 {
    if course == Course::Canyon {
        let swing = 0.2 * (t * 0.6).sin();
        let pos = mean + v3(-2.0 - 32.0 * swing.cos(), -32.0 * swing.sin(), 7.0);
        return View3 { pos, heading_rad: swing, pitch: -0.15, fov_rad: 0.76 };
    }
    let c = mean + v3(1.0, 0.0, 0.0);
    if t < 1.4 {
        View3 { pos: c + v3(-34.0, -1.5, 10.5), heading_rad: 0.05, pitch: -0.29, fov_rad: 0.72 }
    } else if t < 2.7 {
        View3 { pos: c + v3(28.0, 1.0, 5.5), heading_rad: PI - 0.04, pitch: -0.07, fov_rad: 0.78 }
    } else {
        View3 { pos: c + v3(-32.0, -4.0, 10.0), heading_rad: 0.2, pitch: -0.27, fov_rad: 0.82 }
    }
}

/// The turntable camera for a racer at the origin.
pub fn sheet_view(course: Course) -> View3 {
    match course {
        Course::Hell => View3 { pos: v3(4.0, -8.3, 6.5), heading_rad: 2.1, pitch: -0.42, fov_rad: 1.1 },
        Course::Canyon => View3 { pos: v3(3.0, -11.0, 5.0), heading_rad: 2.0, pitch: -0.2, fov_rad: 1.0 },
    }
}

// ---------------------------------------------------------------- one frame

/// What a mesh in a scene is, for its light: the track (lit as its course
/// lights it), a racer (by its place in `RACERS`: tonal bands from a key
/// light, a rim in its colour), or open country under the night (`card`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Track,
    /// The track sunk back behind what is in front of it: dimmer and greyer.
    Backdrop,
    Racer(usize),
    Vista,
}

/// The flat-shaded raster: the world3d triangles and a z-buffer at any size,
/// kept between frames. The race reads it at dot resolution for its braille;
/// the loop card reads it as a picture.
pub struct Raster {
    pub course: Course,
    pub width: usize,
    pub height: usize,
    pub img: RgbImage,
    /// What a dot that hits nothing shows, `k` down the frame, in place of
    /// the course's own sky.
    pub sky: Option<fn(f32) -> [u8; 3]>,
    inv_z: Vec<f32>,
    id: Vec<u8>,
}

impl Raster {
    pub fn new(course: Course, width: usize, height: usize) -> Raster {
        Raster { course, width, height, img: RgbImage::new(width as u32, height as u32), sky: None, inv_z: vec![0.0; width * height], id: vec![0; width * height] }
    }

    /// `scene` is the track, then each racer with its place in RACERS.
    pub fn draw(&mut self, scene: &[(&Mesh, Option<usize>)], v: &View3) {
        let parts: Vec<(&Mesh, Part)> = scene.iter().map(|&(m, r)| (m, r.map_or(Part::Track, Part::Racer))).collect();
        self.draw_parts(&parts, v);
    }

    /// Every mesh of `scene` as its part, seen from `v`.
    pub fn draw_parts(&mut self, scene: &[(&Mesh, Part)], v: &View3) {
        let (w, h) = (self.width as f32, self.height as f32);
        let (sh, ch) = v.heading_rad.sin_cos();
        let (sp, cp) = v.pitch.sin_cos();
        let fwd = v3(ch * cp, sh * cp, sp);
        let right = fwd.cross(V3::UP).normalize();
        let up = right.cross(fwd);
        let focal = w * 0.5 / (v.fov_rad * 0.5).tan();
        // screen x, y and 1/z of a world point; None when it is behind the near plane
        let project = |p: V3| {
            let d = p - v.pos;
            let z = d.dot(fwd);
            (z >= 0.1).then(|| [w * 0.5 + d.dot(right) / z * focal, h * 0.5 - d.dot(up) / z * focal, 1.0 / z])
        };
        let edge = |p: [f32; 3], q: [f32; 3], x: f32, y: f32| (q[0] - p[0]) * (y - p[1]) - (q[1] - p[1]) * (x - p[0]);
        // the canyon track is lit by a fixed sun; a racer by a key light that hangs
        // above and a little behind the camera, so whoever faces us is lit, whichever way we look
        let (course, sun) = (self.course, v3(0.4, -0.55, 0.75).normalize());
        let key = v3(-0.45 * ch - 0.2 * sh, -0.45 * sh + 0.2 * ch, 0.85).normalize();
        for (y, row) in self.img.rows_mut().enumerate() {
            let sky = Rgb(self.sky.map_or_else(|| course.sky(y as f32 / h), |sky| sky(y as f32 / h)));
            row.for_each(|p| *p = sky);
        }
        self.inv_z.fill(0.0);
        self.id.fill(0);
        let stride = self.width;
        for (mi, part, t) in scene.iter().enumerate().flat_map(|(mi, (mesh, part))| mesh.tris.iter().map(move |t| (mi, *part, t))) {
            let (Some(a), Some(b), Some(c)) = (project(t.v[0]), project(t.v[1]), project(t.v[2])) else { continue };
            let (x0, x1) = (a[0].min(b[0]).min(c[0]).max(0.0), a[0].max(b[0]).max(c[0]).min(w - 1.0));
            let (y0, y1) = (a[1].min(b[1]).min(c[1]).max(0.0), a[1].max(b[1]).max(c[1]).min(h - 1.0));
            let area = edge(a, b, c[0], c[1]);
            if x0 > x1 || y0 > y1 || area.abs() < 1e-6 {
                continue;
            }
            let mover = matches!(part, Part::Racer(_));
            let col = match part {
                Part::Vista => Rgb(card::vista_shade(t, v.pos)),
                Part::Backdrop => Rgb(card::backdrop(course.shade(t, v.pos, sun, false))),
                _ => Rgb(course.shade(t, v.pos, if mover { key } else { sun }, mover)),
            };
            let (inv_area, pool) = (1.0 / area, mover && pool(t));
            for py in y0 as usize..=y1 as usize {
                for px in x0 as usize..=x1 as usize {
                    let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
                    let (w0, w1) = (edge(b, c, fx, fy) * inv_area, edge(c, a, fx, fy) * inv_area);
                    let w2 = 1.0 - w0 - w1;
                    let (i, iz) = (py * stride + px, w0 * a[2] + w1 * b[2] + w2 * c[2]);
                    if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 && iz > self.inv_z[i] {
                        self.inv_z[i] = iz;
                        // Racers are marked by their place in the scene, one up;
                        // 0 is everything else, the rim's ground.
                        self.id[i] = if pool || !mover { 0 } else { (mi + 1).min(255) as u8 };
                        self.img.put_pixel(px as u32, py as u32, col);
                    }
                }
            }
        }
        self.halo(scene);
    }

    /// Whether anything was drawn at this dot.
    pub fn covered(&self, x: usize, y: usize) -> bool {
        self.inv_z[y * self.width + x] > 0.0
    }

    /// A dim rim in the knight's colour on the dots just outside each racer's
    /// silhouette, so it lifts off the dark floor.
    fn halo(&mut self, scene: &[(&Mesh, Part)]) {
        let rim: Vec<Option<Rgb<u8>>> = scene
            .iter()
            .map(|(_, part)| match part {
                Part::Racer(i) => Some(Rgb(self.course.paint(RACERS[*i].cloth, true).0.map(|c| (c * 0.4) as u8))),
                _ => None,
            })
            .collect();
        let (w, h) = (self.width, self.height);
        for y in 0..h {
            for x in 0..w {
                if self.id[y * w + x] != 0 {
                    continue;
                }
                let near = [(1, 0), (-1, 0), (0, 1), (0, -1)].into_iter().find_map(|(dx, dy)| {
                    let (nx, ny) = (x.checked_add_signed(dx)?, y.checked_add_signed(dy)?);
                    let id = self.id.get(ny * w + nx).copied().filter(|_| nx < w && ny < h)?;
                    rim.get(usize::from(id).checked_sub(1)?).copied().flatten()
                });
                if let Some(c) = near {
                    self.img.put_pixel(x as u32, y as u32, c);
                }
            }
        }
    }
}
