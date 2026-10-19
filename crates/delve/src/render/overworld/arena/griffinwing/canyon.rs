//! Griffinwing's canyon: the three beats' cliffs, causeway ruins and roost,
//! built once as a row of chunks along the rail (the stage hands the
//! rasteriser only the few chunks near the camera). Everything is placed in
//! rail space (`s` along, `x` across to the right, `z` up) and turned into the
//! world by the rail itself. These are labelled block placeholders for the
//! art wave: walls and spires of rock, stone and banner blocks.
use crate::render::world3d::math::{V3, v3};
use crate::render::world3d::mesh::{Mesh, mat};
use crate::rules::griffinwing::rail::{self, Rail};
use crate::rules::griffinwing::trig::Det;
use std::sync::OnceLock;

/// How long a chunk is, in units.
pub const CH: f32 = 40.0;

/// How wide the lit lip along each cliff's top edge is, in units.
const RIM: f32 = 1.2;

fn p(r: &Rail, s: f32, x: f32, z: f32) -> V3 {
    let w = r.world(s, x, z);
    v3(w[0], w[1], w[2])
}

/// A surface lying along the rail from `s0` to `s1`, across `x0..x1`, at
/// height `z`.
fn floor(m: &mut Mesh, r: &Rail, s0: f32, s1: f32, x0: f32, x1: f32, z: f32, material: u8) {
    m.push_quad(
        p(r, s0, x0, z),
        p(r, s1, x0, z),
        p(r, s1, x1, z),
        p(r, s0, x1, z),
        material,
    );
}

/// A face standing along the rail at `x`, from `z0` to `z1`.
fn wall(m: &mut Mesh, r: &Rail, s0: f32, s1: f32, x: f32, z0: f32, z1: f32, material: u8) {
    m.push_quad(
        p(r, s0, x, z0),
        p(r, s1, x, z0),
        p(r, s1, x, z1),
        p(r, s0, x, z1),
        material,
    );
}

/// A box from its centre and three half-axes.
pub fn cuboid(m: &mut Mesh, c: V3, a: V3, b: V3, h: V3, material: u8) {
    let corner = |i: f32, j: f32, k: f32| c + a * i + b * j + h * k;
    let faces = [
        [(-1., -1., 1.), (1., -1., 1.), (1., 1., 1.), (-1., 1., 1.)],
        [
            (-1., -1., -1.),
            (1., -1., -1.),
            (1., -1., 1.),
            (-1., -1., 1.),
        ],
        [(-1., 1., -1.), (1., 1., -1.), (1., 1., 1.), (-1., 1., 1.)],
        [
            (-1., -1., -1.),
            (-1., 1., -1.),
            (-1., 1., 1.),
            (-1., -1., 1.),
        ],
        [(1., -1., -1.), (1., 1., -1.), (1., 1., 1.), (1., -1., 1.)],
    ];
    for f in faces {
        let q = f.map(|(i, j, k)| corner(i, j, k));
        m.push_quad(q[0], q[1], q[2], q[3], material);
    }
}

/// A box set square to the rail at `s`: centred at `(x, z)` across and up,
/// `half` its half-extents along, across and up.
pub fn block(m: &mut Mesh, r: &Rail, s: f32, x: f32, z: f32, half: (f32, f32, f32), material: u8) {
    let at = r.at(s);
    let (sin, cos) = at.yaw.dsin_cos();
    let (f, right) = (v3(cos, sin, 0.0), v3(-sin, cos, 0.0));
    cuboid(
        m,
        p(r, s, x, z),
        f * half.0,
        right * half.1,
        V3::UP * half.2,
        material,
    );
}

/// A hoop of `n` flat quads round `c`, in the plane of `r_axis` and `u_axis`.
pub fn hoop(
    m: &mut Mesh,
    c: V3,
    r_axis: V3,
    u_axis: V3,
    radius: f32,
    thick: f32,
    n: usize,
    material: u8,
) {
    for i in 0..n {
        let a0 = i as f32 / n as f32 * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / n as f32 * std::f32::consts::TAU;
        let at = |a: f32, rad: f32| c + r_axis * (a.dcos() * rad) + u_axis * (a.dsin() * rad);
        m.push_quad(
            at(a0, radius),
            at(a1, radius),
            at(a1, radius + thick),
            at(a0, radius + thick),
            material,
        );
    }
}

/// A diamond: a lamp, a shard.
pub fn diamond(m: &mut Mesh, c: V3, r: f32, turn: f32, material: u8) {
    let (s, co) = turn.dsin_cos();
    let a = v3(co * r, s * r, 0.0);
    let b = v3(-s * r, co * r, 0.0);
    let (top, bottom) = (c + v3(0.0, 0.0, r * 1.4), c - v3(0.0, 0.0, r * 1.4));
    let ring = [c + a, c + b, c - a, c - b];
    for i in 0..4 {
        let (p0, p1) = (ring[i], ring[(i + 1) % 4]);
        m.push_tri([top, p0, p1], [[0.0, 0.0], [r, 0.0], [r, r]], material);
        m.push_tri([bottom, p1, p0], [[0.0, 0.0], [r, 0.0], [r, r]], material);
    }
}

/// A steady scatter in 0 to 1 from two whole numbers.
pub fn scatter(a: i32, b: u32) -> f32 {
    let mut h = (a as u32).wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

/// The open storm gorge of the first beat: a river of cloud far below, cliffs
/// stepping up either side, spires standing out of the deep.
fn gorge(m: &mut Mesh, r: &Rail, s: f32, e: f32, half: f32) {
    let k = (s / 4.0) as i32;
    floor(m, r, s, e, -half, half, -14.0, mat::WATER);
    // A sheet of moonlit mist drifting in the deep, in dashes: they are what
    // streams by below.
    if k % 3 == 0 {
        let w = 2.0 + scatter(k, 21) * 5.0;
        let c = (scatter(k, 22) - 0.5) * half;
        floor(m, r, s, s + 3.4, c - w, c + w, -12.5, mat::MOONLIGHT);
    }
    for side in [-1.0f32, 1.0] {
        // Strata: bands of stone and shadow up the cliff, which change from
        // slice to slice so the wall has a grain going by.
        let bands = [(-14.0, -7.0), (-7.0, -1.0), (-1.0, 3.5), (3.5, 6.0)];
        for (b, &(z0, z1)) in bands.iter().enumerate() {
            let pick = scatter(k * 4 + b as i32, 31 + side as u32);
            // The top band is always lit stone: the wall's pale crest.
            let material = if pick > 0.66 || b == bands.len() - 1 {
                mat::STONE
            } else if pick > 0.3 {
                mat::ROCK
            } else {
                mat::STONE_DARK
            };
            wall(m, r, s, e, side * half, z0, z1, material);
        }
        // The lip of the ledge, in lit stone: the canyon's near edge, a pale
        // line running down to the horizon.
        floor(m, r, s, e, side * half, side * (half + RIM), 6.0, mat::STONE);
        floor(m, r, s, e, side * (half + RIM), side * (half + 4.0), 6.0, mat::ROCK);
        wall(m, r, s, e, side * (half + 4.0), 6.0, 16.0, mat::STONE_DARK);
        floor(
            m,
            r,
            s,
            e,
            side * (half + 4.0),
            side * (half + 30.0),
            16.0,
            mat::FOLIAGE,
        );
    }
}

/// The ruined causeway: a chasm, stone cliffs hung with lit windows, a broken
/// road on its arches below.
fn causeway(m: &mut Mesh, r: &Rail, s: f32, e: f32, half: f32) {
    floor(m, r, s, e, -half, half, -18.0, mat::WATER);
    for side in [-1.0f32, 1.0] {
        wall(m, r, s, e, side * half, -18.0, 14.0, mat::STONE);
        floor(m, r, s, e, side * half, side * (half + RIM), 14.0, mat::STONE);
        floor(
            m,
            r,
            s,
            e,
            side * (half + RIM),
            side * (half + 5.0),
            14.0,
            mat::STONE_DARK,
        );
    }
    // The road, with its missing planks.
    let k = (s / 4.0) as i32;
    if scatter(k, 7) > 0.28 {
        floor(m, r, s, e, -3.2, 3.2, -6.0, mat::PATH);
    }
}

/// The roost's approach and hollow: bare rock, bone ribs, embers in the cracks.
fn roost(m: &mut Mesh, r: &Rail, s: f32, e: f32, half: f32, looks: u8) {
    let drop = if looks == rail::ROOST { -16.0 } else { -14.0 };
    floor(m, r, s, e, -half, half, drop, mat::ROCK);
    for side in [-1.0f32, 1.0] {
        wall(m, r, s, e, side * half, drop, 18.0, mat::ROCK);
        floor(m, r, s, e, side * half, side * (half + RIM), 18.0, mat::STONE);
        floor(
            m,
            r,
            s,
            e,
            side * (half + RIM),
            side * (half + 8.0),
            18.0,
            mat::STONE_DARK,
        );
    }
    let k = (s / 4.0) as i32;
    // Channels of lava glowing in the floor.
    if k % 2 == 0 {
        for side in [-1.0f32, 1.0] {
            floor(
                m,
                r,
                s,
                e + 1.2,
                side * half * 0.35 - 0.7,
                side * half * 0.35 + 0.7,
                drop + 0.15,
                mat::EMBER,
            );
        }
    }
    // Cracks of ember down the walls.
    if scatter(k, 3) > 0.6 {
        let side = if scatter(k, 4) > 0.5 { 1.0f32 } else { -1.0 };
        wall(
            m,
            r,
            s,
            s + 1.4,
            side * (half - 0.05),
            -4.0 + scatter(k, 5) * 6.0,
            8.0,
            mat::EMBER,
        );
    }
}

/// The way out: open sky over a sea of cloud, low far cliffs.
fn exit(m: &mut Mesh, r: &Rail, s: f32, e: f32, half: f32) {
    floor(m, r, s, e, -half * 1.6, half * 1.6, -16.0, mat::WATER);
    for side in [-1.0f32, 1.0] {
        wall(m, r, s, e, side * half * 1.6, -16.0, 4.0, mat::ROCK);
    }
}

/// The things that stand along the canyon, every so often.
fn furniture(m: &mut Mesh, r: &Rail, s0: f32, s1: f32) {
    let first = (s0 / 8.0).ceil() as i32;
    let last = (s1 / 8.0).ceil() as i32;
    for k in first..last {
        let s = k as f32 * 8.0;
        let at = r.at(s);
        let half = at.half;
        let side = if k % 2 == 0 { 1.0f32 } else { -1.0 };
        match at.look {
            rail::GORGE => {
                // Spires out of the cloud, every few units, standing off the
                // window: they are the parallax.
                if k % 3 == 0 {
                    let h = scatter(k, 11);
                    let x = side * (half - 2.0 - h * 2.0);
                    let top = -14.0 + 9.0 + h * 12.0;
                    block(
                        m,
                        r,
                        s,
                        x,
                        (top - 14.0) * 0.5,
                        (1.5 + h, 1.4 + h * 0.6, (top + 14.0) * 0.5),
                        mat::STONE,
                    );
                    block(
                        m,
                        r,
                        s,
                        x,
                        top + 0.4,
                        (1.0 + h * 0.6, 1.0 + h * 0.4, 0.4),
                        mat::STONE_DARK,
                    );
                    // A brazier lit on its top, now and then.
                    if k % 9 == 0 {
                        block(m, r, s, x, top + 1.1, (0.3, 0.3, 0.3), mat::FIRE);
                    }
                }
                // Buttresses jutting from the cliffs, in every size.
                if k % 2 == 1 {
                    let h = scatter(k, 13);
                    let x = -side * (half - 0.8 - h * 1.2);
                    block(
                        m,
                        r,
                        s,
                        x,
                        -4.0 + h * 8.0,
                        (1.6 + h * 2.0, 1.2 + h * 0.8, 3.0 + h * 5.0),
                        mat::STONE_DARK,
                    );
                }
            }
            rail::CAUSEWAY => {
                // Lanterns on the walls, windows lit in the stone, and every
                // so often a tower with a banner and a broken arch over the
                // canyon.
                if k % 2 == 0 {
                    block(
                        m,
                        r,
                        s,
                        side * (half - 0.3),
                        2.0,
                        (0.2, 0.2, 0.3),
                        mat::FIRE,
                    );
                }
                if k % 3 == 1 {
                    block(
                        m,
                        r,
                        s,
                        -side * (half - 0.15),
                        5.0 + scatter(k, 2) * 3.0,
                        (0.5, 0.05, 0.9),
                        mat::WINDOW,
                    );
                }
                if k % 5 == 0 {
                    let x = side * (half + 2.6);
                    block(m, r, s, x, -2.0, (2.0, 2.0, 12.0), mat::STONE);
                    block(m, r, s, x, 10.8, (2.3, 2.3, 0.7), mat::STONE_DARK);
                    block(
                        m,
                        r,
                        s - 2.05,
                        x - side * 2.2,
                        8.0,
                        (0.05, 0.5, 0.8),
                        mat::WINDOW,
                    );
                    block(
                        m,
                        r,
                        s + 0.4,
                        x - side * 2.35,
                        9.2,
                        (0.05, 0.5, 2.2),
                        mat::BANNER,
                    );
                }
                if k % 9 == 4 {
                    // A broken arch over the canyon.
                    for sd in [-1.0f32, 1.0] {
                        block(
                            m,
                            r,
                            s,
                            sd * (half - 1.2),
                            0.0,
                            (1.1, 1.1, 11.0),
                            mat::STONE,
                        );
                    }
                    block(
                        m,
                        r,
                        s,
                        -half * 0.35,
                        11.2,
                        (1.1, half * 0.65, 0.9),
                        mat::STONE,
                    );
                    block(
                        m,
                        r,
                        s,
                        half * 0.65,
                        11.2,
                        (1.1, half * 0.35, 0.9),
                        mat::STONE_DARK,
                    );
                }
                // The road's arches below.
                if k % 2 == 0 {
                    for sd in [-1.0f32, 1.0] {
                        block(m, r, s, sd * 3.0, -12.0, (0.6, 0.5, 6.0), mat::STONE_DARK);
                    }
                }
            }
            rail::APPROACH | rail::ROOST => {
                // Bone ribs arching over the way, braziers on columns.
                if k % 2 == 0 {
                    let n = 9;
                    for i in 0..n {
                        let a0 = (i as f32 / n as f32) * std::f32::consts::PI;
                        let a1 = ((i + 1) as f32 / n as f32) * std::f32::consts::PI;
                        let rad = half * 0.92;
                        let pt = |a: f32, w: f32| {
                            p(r, s, a.dcos() * (rad + w), 1.0 + a.dsin() * (rad * 0.8 + w))
                        };
                        m.push_quad(
                            pt(a0, 0.0),
                            pt(a1, 0.0),
                            pt(a1, 0.5),
                            pt(a0, 0.5),
                            mat::BONE,
                        );
                    }
                }
                if k % 5 == 2 {
                    block(
                        m,
                        r,
                        s,
                        side * (half - 2.0),
                        -4.0,
                        (0.6, 0.6, 10.0),
                        mat::STONE_DARK,
                    );
                    block(
                        m,
                        r,
                        s,
                        side * (half - 2.0),
                        6.4,
                        (0.8, 0.8, 0.4),
                        mat::FIRE,
                    );
                }
            }
            _ => {
                if k % 4 == 0 {
                    block(
                        m,
                        r,
                        s,
                        side * (half * 1.4),
                        0.0,
                        (0.5, 0.5, 9.0),
                        mat::STONE,
                    );
                    block(
                        m,
                        r,
                        s,
                        side * (half * 1.4),
                        9.4,
                        (0.7, 0.7, 0.4),
                        mat::FIRE,
                    );
                }
            }
        }
    }
}

fn build(k: usize) -> Mesh {
    let r = rail::rail();
    let (s0, s1) = (k as f32 * CH, (((k + 1) as f32) * CH).min(r.len()));
    let mut m = Mesh::new();
    let mut s = s0;
    while s < s1 - 1e-3 {
        let e = (s + 4.0).min(s1);
        let at = r.at(s);
        match at.look {
            rail::GORGE => gorge(&mut m, r, s, e, at.half),
            rail::CAUSEWAY => causeway(&mut m, r, s, e, at.half),
            rail::EXIT => exit(&mut m, r, s, e, at.half),
            looks => roost(&mut m, r, s, e, at.half, looks),
        }
        s = e;
    }
    furniture(&mut m, r, s0, s1);
    m
}

/// The canyon, a chunk at a time.
pub fn chunks() -> &'static [Mesh] {
    static CHUNKS: OnceLock<Vec<Mesh>> = OnceLock::new();
    CHUNKS.get_or_init(|| {
        let n = (rail::rail().len() / CH).ceil() as usize;
        (0..n).map(build).collect()
    })
}

/// The chunks that can be in sight from `s` units along: a little behind and
/// well ahead.
pub fn near(s: f32) -> impl Iterator<Item = &'static Mesh> {
    let all = chunks();
    let a = (((s - 12.0).max(0.0)) / CH) as usize;
    let b = (((s + 96.0).max(0.0)) / CH) as usize;
    (a..=b.min(all.len().saturating_sub(1))).map(move |i| &all[i])
}
