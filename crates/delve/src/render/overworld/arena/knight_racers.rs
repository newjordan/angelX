//! Knight Racer: the example's armored engines and bent hell course, drawn
//! with Delve's shared 3D raster and per-seat chase camera.
use super::super::ink::Img;
use super::race3d::dots_with;
use super::roundtable::{self as shell, Look};
use crate::rules::{Run, knight_racers as rules};
use crate::render::world3d::math::{V3, v3};
use crate::render::world3d::mesh::{Mesh, mat};
use crate::render::world3d::raster::{self, View3, Firelight, Backdrop};
use std::f32::consts::{PI, FRAC_PI_2, TAU};
const LOOK: Look = Look { name: rules::NAME, backdrop: 'F', edge: 'y', accent: 'T' };
// Hardcore palette mapped onto the game's semantic materials:
// black iron -> STONE_DARK, steel -> ARMOR, Excalibur Blue -> CRYSTAL / CLOTH_BLUE, exhaust -> FLAME / EMBER.
const IRON: u8 = mat::STONE_DARK;
const STEEL: u8 = mat::ARMOR;
const GUN: u8 = mat::HORSE_GREY; // the engines' gunmetal, darker than a knight's steel
const BLUE: u8 = mat::CRYSTAL;
const LAVA: u8 = mat::EMBER;
const LAVA_HOT: u8 = mat::FLAME;
const H_HALF: f32 = 9.0; // hell track half-width
const H_LAVA: f32 = 2.6; // lava river width
const H_CEIL: f32 = 13.0;
/// The `uv` mark of a tile of a bent track (see `Course::shade`).
const BENT: [f32; 2] = [-9999.0, -9999.0];

fn tri(m: &mut Mesh, a: V3, b: V3, c: V3, k: u8) {
    m.push_tri([a, b, c], [[0.0; 2]; 3], k);
}

fn cuboid(m: &mut Mesh, lo: V3, hi: V3, k: u8) {
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
fn barrel(m: &mut Mesh, [x0, x1]: [f32; 2], [y, z]: [f32; 2], [r0, r1]: [f32; 2], n: usize, k: u8, caps: bool) {
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
fn glow_pool(m: &mut Mesh, c: V3, [rx, ry]: [f32; 2], k: u8) {
    for (w, z) in [(1.0f32, 0.03), (0.62, 0.05)] {
        let at = |i: usize| c + v3(rx * w * (i as f32 / 12.0 * TAU).cos(), ry * w * (i as f32 / 12.0 * TAU).sin(), z);
        (0..12).for_each(|i| tri(m, v3(c.x, c.y, z), at(i), at(i + 1), k));
    }
}

/// Four-sided cone from a ring at `base` (radius r) to a tip.
fn spike(m: &mut Mesh, base: V3, tip: V3, r: f32, k: u8) {
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

/// What crowns a knight's helm: the shape that tells the four apart above the rim.
#[derive(Clone, Copy, PartialEq)]
enum Crest { Plume, Horns, Hood, Crown }

/// One racer: its engine pair, its chariot and its knight's colours and crest.
struct Racer { len: f32, r: f32, sides: usize, pipes: usize, pipe_h: f32, spikes: usize, gap: f32, fins: bool, horns: bool, cart: f32, lance: bool, cloth: u8, gleam: u8, crest: Crest }

const RACERS: [Racer; 4] = [
    Racer { len: 4.2, r: 0.7, sides: 6, pipes: 3, pipe_h: 1.4, spikes: 4, gap: 1.3, fins: false, horns: false, cart: 2.1, lance: true, cloth: mat::BANNER, gleam: BLUE, crest: Crest::Plume },
    Racer { len: 3.4, r: 0.95, sides: 4, pipes: 2, pipe_h: 1.9, spikes: 4, gap: 1.6, fins: false, horns: true, cart: 2.3, lance: false, cloth: mat::FOLIAGE, gleam: mat::WATER, crest: Crest::Horns },
    Racer { len: 4.8, r: 0.58, sides: 5, pipes: 3, pipe_h: 1.1, spikes: 3, gap: 1.2, fins: true, horns: false, cart: 2.0, lance: true, cloth: mat::MOONLIGHT, gleam: mat::TORCH_POOL, crest: Crest::Hood },
    Racer { len: 3.8, r: 0.8, sides: 6, pipes: 2, pipe_h: 2.2, spikes: 6, gap: 1.4, fins: true, horns: true, cart: 2.2, lance: false, cloth: mat::CLOTH_GOLD, gleam: mat::BRASS, crest: Crest::Crown },
];

/// One engine, nose at +x.
fn engine(m: &mut Mesh, s: &Racer, y: f32, z: f32, t: f32) {
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
fn chariot(m: &mut Mesh, x: f32, z: f32, s: f32, r: &Racer) {
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

/// A knight from the cockpit floor `k` up, `u` tall-ish: cuirass, tabard and cape
/// and pauldrons in their colours, a great helm, and the crest that is theirs.
fn knight(m: &mut Mesh, k: V3, u: f32, r: &Racer) {
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
    // arms to the reins
    b(m, [0.0, -0.3, 0.2], [0.35, -0.21, 0.3], IRON);
    b(m, [0.0, 0.21, 0.2], [0.35, 0.3, 0.3], IRON);
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
    if r.lance {
        let base = k + v3(-0.2, -0.35, 0.25) * u;
        barrel(m, [base.x, base.x + 1.4 * u], [base.y, base.z + 0.35 * u], [0.03 * u; 2], 4, STEEL, false);
        m.push_quad(base + v3(-0.1, 0.0, 0.0) * u, base + v3(-0.1, 0.0, 1.0) * u, base + v3(-0.55, 0.0, 0.88) * u, base + v3(-0.55, 0.0, 0.52) * u, cl); // banner
        barrel(m, [base.x - 0.1 * u, base.x - 0.08 * u], [base.y, base.z + 0.45 * u], [0.02 * u; 2], 4, IRON, false);
        cuboid(m, base + v3(-0.11, -0.01, 0.0) * u, base + v3(-0.09, 0.01, 1.05) * u, IRON);
    }
}

/// Jagged blue energy arc between two points, re-rolled each frame.
fn arc(m: &mut Mesh, a: V3, b: V3, t: f32, seed: f32) {
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
fn racer(m: &mut Mesh, i: usize, t: f32, at: V3) {
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
fn skull(m: &mut Mesh, c: V3, s: f32, face: f32) {
    cuboid(m, c + v3(-s, -s, 0.0), c + v3(s, s, 1.6 * s), mat::BONE);
    cuboid(m, c + v3(-0.6 * s, -s, -0.6 * s), c + v3(0.6 * s, s, 0.0), mat::BONE); // jaw
    for dx in [-0.45f32, 0.45] {
        // eye sockets glow lava
        cuboid(m, c + v3((dx - 0.22) * s, 0.95 * face * s, 0.7 * s), c + v3((dx + 0.22) * s, 1.05 * face * s, 1.1 * s), LAVA);
    }
}

fn chain(m: &mut Mesh, a: V3, b: V3) {
    let n = ((b - a).length() / 0.5) as usize + 1;
    for i in 0..n {
        let (p0, p1) = (a.lerp(b, i as f32 / n as f32), a.lerp(b, (i as f32 + 0.8) / n as f32));
        let w = if i % 2 == 0 { v3(0.09, 0.0, 0.0) } else { v3(0.0, 0.09, 0.0) };
        m.push_quad(p0 - w, p1 - w, p1 + w, p0 + w, IRON);
    }
}

/// A wave's rate, nudged so it fits a whole number of times into `period`
/// (none: as it is). A track that loops (`play.rs`) closes on itself unseen.
fn rate(w: f32, period: f32) -> f32 {
    if period > 0.0 { (w * period / TAU).round().max(1.0) * TAU / period } else { w }
}

/// The dungeon from `x0` to `x1`; with a `period`, its waves repeat every
/// `period` (a multiple of 48, which the pillars and cages keep to) and the two ends meet.
fn dungeon_span(x0: f32, x1: f32, period: f32) -> Mesh {
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

/// The world point `y` to the left of the centre line at `s`, `z` up.
fn at(s: f32, y: f32, z: f32) -> V3 {
    let (px, py, th) = pose(s);
    v3(px - y * th.sin(), py + y * th.cos(), z)
}

/// A straight dungeon or canyon bent along the oval, tile by tile. The hell
/// rim light is lit across the track, so each tile carries its pre-bend frame.
fn bend(m: &Mesh) -> Mesh {
    let mut out = Mesh::new();
    for t in &m.tris {
        let cen = (t.v[0] + t.v[1] + t.v[2]) / 3.0;
        out.push_tri(t.v.map(|p| at(p.x, p.y, p.z)), [[cen.y, cen.z], [t.normal.y, t.normal.z], BENT], t.mat);
    }
    out
}

/// The start and finish: a chequered line and a gate with a blue banner.
fn gate(m: &mut Mesh) {
    let half = H_HALF;
    let n = 12;
    for i in 0..n {
        for j in 0..2 {
            let (y0, y1) = (-half + 2.0 * half * i as f32 / n as f32, -half + 2.0 * half * (i + 1) as f32 / n as f32);
            let (x0, x1) = (j as f32 * 1.2, (j + 1) as f32 * 1.2);
            m.push_quad(v3(x0, y0, 0.04), v3(x1, y0, 0.04), v3(x1, y1, 0.04), v3(x0, y1, 0.04), if (i + j) % 2 == 0 { STEEL } else { IRON });
        }
    }
    let edge = half + 1.4;
    for sd in [-1.0f32, 1.0] {
        cuboid(m, v3(-0.4, sd * edge - 0.5, 0.0), v3(0.4, sd * edge + 0.5, 8.0), IRON);
    }
    cuboid(m, v3(-0.4, -edge - 0.5, 7.4), v3(0.4, edge + 0.5, 8.4), IRON);
    m.push_quad(v3(0.0, -edge, 7.4), v3(0.0, edge, 7.4), v3(0.0, edge, 5.2), v3(0.0, -edge, 5.2), mat::CLOTH_BLUE);
    cuboid(m, v3(-0.45, -edge, 7.0), v3(0.45, edge, 7.4), mat::FIRE);
}


const LAP: f32 = rules::LAP;
const SCALE: f32 = 0.72;
fn pose(s: f32) -> (f32, f32, f32) { rules::pose(s) }
fn world(s: f32) -> Mesh {
    // Only the 140 units around the eye: the example's whole 864-unit mesh
    // is far too large to rasterize every browser frame.
    let start = ((s - 35.) / 1.5).floor() * 1.5;
    let end = start + 141.;
    let mut m = bend(&dungeon_span(start, end, LAP));
    if start <= 0. && end >= 0. {
        let mut props = Mesh::new();
        gate(&mut props);
        m.merge(bend(&props));
    }
    if start <= LAP && end >= LAP {
        let mut props = Mesh::new();
        gate(&mut props);
        for tr in &mut props.tris { for p in &mut tr.v { p.x += LAP; } }
        m.merge(bend(&props));
    }
    m
}
fn point(s: f32, y: f32, z: f32) -> V3 { at(s, y, z) }
pub(super) fn layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    let Some(s) = &run.knight_racers else { return (Img::black(384, 224), Img::new(384, 224)); };
    let Some((_, car)) = focus.and_then(|id| s.cars.get_key_value(&id)).or_else(|| s.cars.iter().next()) else {
        return (Img::black(384, 224), Img::new(384, 224));
    };
    let anchor = point(car.d, car.y * 0.8, 0.);
    let th = pose(car.d).2;
    let eye = anchor + v3(-19. * th.cos(), -19. * th.sin(), 6.);
    let aim = anchor + v3(8. * th.cos(), 8. * th.sin(), 1.2);
    let d = aim - eye;
    let view = View3 { pos: eye, heading_rad: d.y.atan2(d.x), pitch: (d.z / d.x.hypot(d.y)).atan(), fov_rad: 0.96 };
    let mut movers = Mesh::new();
    for (&other, c) in &s.cars {
        // Four engine liveries from the original example, repeating for seats 5/6.
        let n = movers.tris.len();
        racer(&mut movers, (other as usize - 1) % RACERS.len(), s.ticks as f32 / 30., V3::ZERO);
        let (sn, cs) = pose(c.d).2.sin_cos();
        let o = point(c.d, c.y, 0.);
        // Transform only the just-appended racer, not the ones before it.
        for tr in &mut movers.tris[n..] {
            for p in &mut tr.v { *p = v3(o.x + (p.x * cs - p.y * sn) * SCALE, o.y + (p.x * sn + p.y * cs) * SCALE, p.z * SCALE); }
            tr.normal = v3(tr.normal.x * cs - tr.normal.y * sn, tr.normal.x * sn + tr.normal.y * cs, tr.normal.z);
        }
    }
    let track = world(car.d);
    let (rgba, _) = raster::render_meshes(&[&track, &movers], &view, 384, 224, Firelight::STEADY, 85., Backdrop::Dark([2, 3, 6]));
    let mut im = dots_with(rgba.as_raw(), 384, 224, (0., 90., 0.85, 85.));
    let mut hud = Img::new(384, 224);
    hud.rect(0, 0, 384, 23, 'k');
    hud.text(8, 7, &format!("KNIGHT RACER  1 LAP   {} / 6   {:3}%", s.cars.len(), (car.d.max(0.) / LAP * 100.).min(100.) as u32), 'T');
    hud.text(270, 7, &format!("BOOST {:3}%", (car.boost * 100.) as u32), 'H');
    hud.rect(0, 204, 384, 20, 'k');
    let label = if s.finished { "FLAG! E RACE AGAIN   ESC LEAVE" } else if s.ticks <= 90 { "GRID: W/UP GAS   A/D STEER   SPACE BOOST" } else { "W/UP GAS  S/DOWN BRAKE  SPACE BOOST  ESC LEAVE" };
    hud.text(8, 210, label, 'T');
    if let Some(t) = car.finish { hud.text(120, 100, &format!("FINISHED! {:.2} SECONDS", t), 'H'); }
    if s.finished {
        let mut board: Vec<_> = s.cars.iter().map(|(&seat, car)| (seat, car.finish.unwrap_or(f32::MAX))).collect();
        board.sort_by(|a, b| a.1.total_cmp(&b.1));
        hud.rect(100, 40, 192, 130, 'k');
        hud.text(118, 46, "ONE LAP RESULTS", 'H');
        for (place, (seat, time)) in board.iter().enumerate() {
            hud.text(118, 60 + place as i32 * 17, &format!("{}  KNIGHT {}   {:.2}", place + 1, seat, time), 'T');
        }
    }
    for y in 0..224 { for x in 0..384 { if let Some(c) = hud.get(x, y) { im.set(x, y, c); } } }
    (im, Img::new(384, 224))
}
pub(super) fn kingdom(im: &mut Img, _run: &Run, origin: (i32, i32)) {
    shell::stub_kingdom(im, origin, rules::ENTRY, &LOOK);
}
