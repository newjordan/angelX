//! The races' meshes: each course built once from its track, and the moving
//! things (balls, carts, griffins, the crossing carts, rotten planks, gems,
//! rings, gargoyles, bolts) built fresh each frame into a reused mesh.
//! Everything is placed in course space (`s` along, `x` across, `z` up)
//! and turned into the world by the track itself.
use crate::rules::race3d::{
    self as r3, BALL_R, Course, Ground, ItemKind, LANES, Race3d, Rider, TIMBER_H, Track, Wreck,
};
use crate::render::world3d::math::{V3, v3};
use crate::render::world3d::mesh::{Mesh, mat};
use std::sync::OnceLock;

fn p(t: &Track, s: f32, x: f32, z: f32) -> V3 {
    let w = t.world(s, x, z);
    v3(w[0], w[1], w[2])
}

/// The course's own frame at `s`: along, across (to the right) and up.
fn frame(t: &Track, s: f32) -> (V3, V3, V3) {
    let yaw = t.at(s).yaw;
    let (sin, cos) = yaw.sin_cos();
    (v3(cos, sin, 0.0), v3(-sin, cos, 0.0), V3::UP)
}

/// A surface lying along the course from `s0` to `s1`, across `x0..x1`, at
/// height `z`, in steps of `step`.
fn floor(m: &mut Mesh, t: &Track, s0: f32, s1: f32, x0: f32, x1: f32, z: f32, step: f32, material: u8) {
    let mut s = s0;
    while s < s1 - 1e-3 {
        let e = (s + step).min(s1);
        m.push_quad(p(t, s, x0, z), p(t, e, x0, z), p(t, e, x1, z), p(t, s, x1, z), material);
        s = e;
    }
}

/// A face standing along the course at `x`, from `z0` to `z1`.
fn wall(m: &mut Mesh, t: &Track, s0: f32, s1: f32, x: f32, z0: f32, z1: f32, step: f32, material: u8) {
    let mut s = s0;
    while s < s1 - 1e-3 {
        let e = (s + step).min(s1);
        m.push_quad(p(t, s, x, z0), p(t, e, x, z0), p(t, e, x, z1), p(t, s, x, z1), material);
        s = e;
    }
}

/// A box set square to the course at `s`: centred at `(x, z)` across and
/// up, `half` its half-extents along, across and up.
pub(super) fn block(m: &mut Mesh, t: &Track, s: f32, x: f32, z: f32, half: (f32, f32, f32), material: u8) {
    let (f, r, u) = frame(t, s);
    let base = p(t, s, x, z);
    cuboid(m, base, f * half.0, r * half.1, u * half.2, material);
}

/// A box from its centre and three half-axes.
pub(super) fn cuboid(m: &mut Mesh, c: V3, a: V3, b: V3, h: V3, material: u8) {
    let corner = |i: f32, j: f32, k: f32| c + a * i + b * j + h * k;
    let faces = [
        [(-1., -1., 1.), (1., -1., 1.), (1., 1., 1.), (-1., 1., 1.)],
        [(-1., -1., -1.), (1., -1., -1.), (1., -1., 1.), (-1., -1., 1.)],
        [(-1., 1., -1.), (1., 1., -1.), (1., 1., 1.), (-1., 1., 1.)],
        [(-1., -1., -1.), (-1., 1., -1.), (-1., 1., 1.), (-1., -1., 1.)],
        [(1., -1., -1.), (1., 1., -1.), (1., 1., 1.), (1., -1., 1.)],
    ];
    for f in faces {
        let q = f.map(|(i, j, k)| corner(i, j, k));
        m.push_quad(q[0], q[1], q[2], q[3], material);
    }
}

/// A diamond: a gem shard, a nugget.
pub(super) fn diamond(m: &mut Mesh, c: V3, r: f32, turn: f32, material: u8) {
    let (s, co) = turn.sin_cos();
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

/// A hoop facing along `f`: a ring of `n` flat quads.
pub(super) fn hoop(m: &mut Mesh, c: V3, r_axis: V3, u_axis: V3, radius: f32, thick: f32, n: usize, material: u8) {
    for i in 0..n {
        let a0 = i as f32 / n as f32 * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / n as f32 * std::f32::consts::TAU;
        let at = |a: f32, rad: f32| c + r_axis * (a.cos() * rad) + u_axis * (a.sin() * rad);
        m.push_quad(at(a0, radius), at(a1, radius), at(a1, radius + thick), at(a0, radius + thick), material);
    }
}

// ─── the courses ────────────────────────────────────────────────────────────

/// The course's fixed mesh, built once.
pub(super) fn course(course: Course) -> &'static Mesh {
    static MESHES: [OnceLock<Mesh>; 3] = [OnceLock::new(), OnceLock::new(), OnceLock::new()];
    let t = r3::track(course);
    match course {
        Course::Shaft => MESHES[0].get_or_init(|| shaft(t)),
        Course::Cart => MESHES[1].get_or_init(|| gallery(t)),
        Course::Griffin => MESHES[2].get_or_init(|| gorge(t)),
    }
}

/// A mine timbering set: two posts and a cap across, square to the course.
fn timbering(m: &mut Mesh, t: &Track, s: f32, half_w: f32, high: f32) {
    for side in [-1.0, 1.0] {
        block(m, t, s, side * half_w, high * 0.5 - 1.0, (0.18, 0.18, high * 0.5 + 1.0), mat::WOOD);
    }
    block(m, t, s, 0.0, high, (0.2, half_w + 0.2, 0.2), mat::WOOD);
}

/// A hung lantern and the pool it throws on the floor.
fn lantern(m: &mut Mesh, t: &Track, s: f32, x: f32, high: f32, pool: bool) {
    block(m, t, s, x, high, (0.16, 0.16, 0.2), mat::FIRE);
    block(m, t, s, x, high + 0.3, (0.05, 0.05, 0.12), mat::WOOD);
    if pool {
        let (f, r, _) = frame(t, s);
        let c = p(t, s, x * 0.5, 0.02);
        let k = 1.6;
        let q = [c - f * k - r * k, c + f * k - r * k, c + f * k + r * k, c - f * k + r * k];
        let uv = |v: V3| {
            let d = v - c;
            [d.dot(f), d.dot(r)]
        };
        m.push_tri([q[0], q[1], q[2]], [uv(q[0]), uv(q[1]), uv(q[2])], mat::TORCH_POOL);
        m.push_tri([q[0], q[2], q[3]], [uv(q[0]), uv(q[2]), uv(q[3])], mat::TORCH_POOL);
    }
}

fn shaft(t: &Track) -> Mesh {
    let mut m = Mesh::new();
    let len = t.len();
    let mut s = 0.0f32;
    while s < len - 0.5 {
        let at = t.at(s);
        let e = (s + 1.0).min(len);
        let half = at.width * 0.5;
        match at.ground {
            Ground::Bridge => {
                // Boards across with gaps between, over nothing at all.
                for k in 0..2 {
                    let b0 = s + k as f32 * 0.5;
                    floor(&mut m, t, b0, b0 + 0.42, -half, half, 0.0, 0.42, mat::STABLE_FLOOR);
                }
                if (s as i32) % 4 == 0 {
                    for side in [-1.0, 1.0] {
                        block(&mut m, t, s, side * (half + 0.1), -1.0, (0.1, 0.1, 1.6), mat::TRUNK);
                    }
                }
            }
            Ground::Collapse => {
                // Sound planks only; the rotten ones are drawn as they stand.
                let plank = (s / 2.0) as u16;
                if !r3::rotten(plank) {
                    floor(&mut m, t, s, e, -half, half, 0.0, 1.0, mat::STABLE_FLOOR);
                }
            }
            Ground::Sluice => floor(&mut m, t, s, e, -half, half, -0.05, 1.0, mat::WATER),
            Ground::Lodestone => {
                floor(&mut m, t, s, e, -half, half, 0.0, 1.0, mat::STABLE_WALL);
                floor(&mut m, t, s, e, 1.15, 1.45, 0.06, 1.0, mat::ARMOR);
            }
            _ => floor(&mut m, t, s, e, -half, half, 0.0, 1.0, mat::STABLE_FLOOR),
        }
        // An open edge is roped in gold, so the drop is never a guess.
        if !at.walls {
            for side in [-1.0f32, 1.0] {
                let x = side * half;
                floor(&mut m, t, s, e, x - 0.06, x + 0.06, 0.03, 1.0, mat::HAY);
            }
        }
        // The rails that keep a ball on, where there are any.
        if at.walls {
            for side in [-1.0, 1.0] {
                wall(&mut m, t, s, e, side * half, -0.05, 0.7, 1.0, mat::STABLE_TIMBER);
            }
        }
        s = e;
    }
    // The shaft itself: rock close about the walled runs, far off and deep
    // round the open ones.
    let mut s = 0.0f32;
    while s < len - 0.5 {
        let at = t.at(s);
        let e = (s + 2.0).min(len);
        let half = at.width * 0.5;
        let (out, deep) = if at.walls { (half + 1.6, -3.0) } else { (half + 4.5, -14.0) };
        for side in [-1.0, 1.0] {
            wall(&mut m, t, s, e, side * out, deep, 5.0, 2.0, mat::ROCK);
            if !at.walls {
                // The ledge the open stretch leaves either side.
                floor(&mut m, t, s, e, side * half, side * out, -1.6, 2.0, mat::ROCK);
            }
        }
        s = e;
    }
    // Timbering every six tiles, a lantern every twelve.
    let mut s = 3.0f32;
    while s < len - 2.0 {
        let at = t.at(s);
        if at.walls {
            timbering(&mut m, t, s, at.width * 0.5 + 0.6, 3.4);
        }
        if (s as i32) % 12 == 3 {
            let side = if (s as i32 / 12) % 2 == 0 { -1.0 } else { 1.0 };
            lantern(&mut m, t, s, side * (at.width * 0.5 + 0.2), 2.4, at.walls);
        }
        s += 6.0;
    }
    // The crossings' rails across the way.
    for &post in &t.posts {
        if t.at(post).ground == Ground::Crossing {
            for ds in [6.0, 13.0] {
                for k in [-0.45, 0.45] {
                    let (f, r, _) = frame(t, post + ds);
                    let c = p(t, post + ds + k, 0.0, 0.04);
                    cuboid(&mut m, c, f * 0.06, r * 4.6, V3::UP * 0.04, mat::ARMOR);
                }
            }
        }
    }
    // Brannoc's ore-sorter: a hopper of chutes over a furnace mouth, and
    // the ranked bins it drops into.
    let sorter = t.posts[t.posts.len() - 1];
    block(&mut m, t, sorter + 12.0, 0.0, 2.6, (1.2, 3.0, 2.6), mat::STONE_DARK);
    block(&mut m, t, sorter + 10.7, 0.0, 1.0, (0.1, 1.4, 0.7), mat::EMBER);
    for k in 0..4 {
        let x = -2.25 + k as f32 * 1.5;
        block(&mut m, t, sorter + 9.0, x, 0.4, (0.6, 0.55, 0.4), mat::BRASS);
        block(&mut m, t, sorter + 9.0, x, 3.9, (0.15, 0.15, 1.6), mat::BRASS);
    }
    block(&mut m, t, sorter + 4.0, 0.0, 3.6, (0.15, 3.2, 0.15), mat::BANNER);
    m
}

fn gallery(t: &Track) -> Mesh {
    let mut m = Mesh::new();
    let len = t.len();
    let mut s = 0.0f32;
    while s < len - 0.5 {
        let at = t.at(s);
        let e = (s + 1.0).min(len);
        let trestle = at.ground == Ground::Trestle;
        if !trestle {
            floor(&mut m, t, s, e, -2.8, 2.8, 0.0, 1.0, mat::PATH);
        }
        for lane in 0..3u8 {
            if at.lanes & (1 << lane) == 0 {
                continue;
            }
            let lx = LANES[usize::from(lane)];
            let mid = s + 0.5;
            let broken = r3::broken(t, mid, lane);
            if !broken {
                // A sleeper, and the two rails on it.
                block(&mut m, t, mid, lx, 0.05, (0.14, 0.75, 0.05), mat::WOOD);
                for k in [-0.45, 0.45] {
                    floor(&mut m, t, s, e, lx + k - 0.05, lx + k + 0.05, 0.14, 1.0, mat::ARMOR);
                }
            }
            if trestle && (s as i32) % 3 == 0 && !broken {
                block(&mut m, t, mid, lx, -5.0, (0.12, 0.12, 5.0), mat::TRUNK);
            }
        }
        s = e;
    }
    // Buffer stops where a line ends.
    for (i, w) in t.samples.windows(2).enumerate() {
        for lane in 0..3u8 {
            let bit = 1 << lane;
            if w[0].lanes & bit != 0 && w[1].lanes & bit == 0 {
                let s = i as f32 + 1.0;
                let lx = LANES[usize::from(lane)];
                block(&mut m, t, s - 0.2, lx, 0.5, (0.2, 0.7, 0.5), mat::WOOD);
                block(&mut m, t, s - 0.4, lx, 0.7, (0.05, 0.6, 0.12), mat::BANNER);
            }
        }
    }
    for item in &t.items {
        if item.kind == ItemKind::Timber {
            block(&mut m, t, item.s, item.x, TIMBER_H * 0.5, (0.22, 0.95, TIMBER_H * 0.5), mat::TRUNK);
        }
    }
    let mut s = 0.0f32;
    while s < len - 0.5 {
        let at = t.at(s);
        let e = (s + 2.0).min(len);
        let (out, deep) = if at.ground == Ground::Trestle { (6.5, -14.0) } else { (3.6, -1.0) };
        for side in [-1.0, 1.0] {
            wall(&mut m, t, s, e, side * out, deep, 5.0, 2.0, mat::ROCK);
        }
        s = e;
    }
    let mut s = 3.0f32;
    while s < len - 2.0 {
        let at = t.at(s);
        if at.ground != Ground::Trestle {
            timbering(&mut m, t, s, 3.2, 3.6);
        }
        if (s as i32) % 12 == 3 {
            let side = if (s as i32 / 12) % 2 == 0 { -1.0 } else { 1.0 };
            lantern(&mut m, t, s, side * 3.0, 2.6, at.ground != Ground::Trestle);
        }
        s += 6.0;
    }
    // The end of the line: an ore tip and its lamp.
    let end = t.posts[t.posts.len() - 1];
    block(&mut m, t, end + 15.0, 0.0, 1.6, (0.6, 2.4, 1.6), mat::STONE_DARK);
    block(&mut m, t, end + 14.3, 0.0, 2.4, (0.1, 0.4, 0.4), mat::FIRE);
    m
}

fn gorge(t: &Track) -> Mesh {
    let mut m = Mesh::new();
    let len = t.len();
    let mut s = 0.0f32;
    while s < len - 0.5 {
        let at = t.at(s);
        let e = (s + 2.0).min(len);
        let half = at.width * 0.5;
        // The river at the bottom, the cliffs in two steps either side.
        floor(&mut m, t, s, e, -half, half, -9.0, 2.0, mat::WATER);
        for side in [-1.0f32, 1.0] {
            wall(&mut m, t, s, e, side * half, -9.0, 1.5, 2.0, mat::ROCK);
            floor(&mut m, t, s, e, side * half, side * (half + 3.0), 1.5, 2.0, mat::ROCK);
            wall(&mut m, t, s, e, side * (half + 3.0), 1.5, 5.0, 2.0, mat::ROCK);
            floor(&mut m, t, s, e, side * (half + 3.0), side * (half + 14.0), 5.0, 2.0, mat::GRASS);
        }
        s = e;
    }
    // The Keep's towers on the rims, lit windows facing the gorge.
    let mut s = 20.0f32;
    let mut k = 0;
    while s < len - 60.0 {
        let at = t.at(s);
        let side = if k % 2 == 0 { -1.0 } else { 1.0 };
        let x = side * (at.width * 0.5 + 6.5);
        block(&mut m, t, s, x, 9.0, (1.4, 1.4, 4.0), mat::STONE);
        block(&mut m, t, s, x, 13.6, (1.7, 1.7, 0.6), mat::STONE_DARK);
        block(&mut m, t, s, x - side * 1.42, 10.5, (0.25, 0.02, 0.4), mat::WINDOW);
        block(&mut m, t, s, x, 15.0, (0.08, 0.08, 1.0), mat::TRUNK);
        block(&mut m, t, s + 0.4, x, 15.6, (0.4, 0.02, 0.3), mat::BANNER);
        s += 34.0;
        k += 1;
    }
    for b in &t.blocks {
        let (x, z) = ((b.x.0 + b.x.1) * 0.5, (b.z.0 + b.z.1) * 0.5);
        let half = ((b.x.1 - b.x.0) * 0.5, (b.z.1 - b.z.0) * 0.5);
        let rampart = b.x.1 - b.x.0 > 10.0;
        let material = if rampart { mat::STONE } else { mat::STONE_DARK };
        block(&mut m, t, b.s + b.len * 0.5, x, z, (b.len * 0.5, half.0, half.1), material);
        if rampart {
            // Crenellations along its top.
            let mut cx = b.x.0 + 0.6;
            while cx < b.x.1 - 0.4 {
                block(&mut m, t, b.s + b.len * 0.5, cx, b.z.1 + 0.3, (b.len * 0.5, 0.35, 0.3), mat::STONE);
                cx += 1.6;
            }
        } else {
            block(&mut m, t, b.s + b.len * 0.5, x, b.z.1 + 0.25, (b.len * 0.35, half.0 * 0.7, 0.25), mat::STONE);
        }
    }
    // The Keep at the end of the gorge: a great wall, a gate, two towers.
    let gate = t.finish;
    block(&mut m, t, gate + 2.0, 0.0, 4.0, (1.0, 10.0, 13.0), mat::STONE);
    block(&mut m, t, gate + 0.95, 0.0, 0.0, (0.05, 2.2, 3.0), mat::DOOR);
    block(&mut m, t, gate + 0.9, 0.0, 0.0, (0.05, 1.8, 2.6), mat::FIRE);
    for side in [-1.0, 1.0] {
        block(&mut m, t, gate + 1.5, side * 7.0, 8.0, (2.2, 2.2, 13.0), mat::STONE);
        block(&mut m, t, gate - 0.75, side * 7.0, 12.0, (0.05, 0.3, 0.6), mat::WINDOW);
        block(&mut m, t, gate - 0.75, side * 7.0, 6.0, (0.05, 0.3, 0.6), mat::WINDOW);
        block(&mut m, t, gate - 0.8, side * 3.6, 6.0, (0.05, 0.5, 2.0), mat::BANNER);
    }
    m
}

// ─── what moves ─────────────────────────────────────────────────────────────

/// A knight's ball: bronze, silver, white enamel or crystal by seat, and
/// the band colour that goes with it.
pub(super) fn ball_material(seat: u32) -> (u8, u8) {
    // Metals that hold the lamplight, so a ball reads from behind as well
    // as it does with the moon on it.
    match seat {
        1 => (mat::STABLE_TIMBER, mat::CLOTH_GOLD),
        2 => (mat::ARMOR, mat::CLOTH_BLUE),
        3 => (mat::HORSE_GREY, mat::CLOTH_GOLD),
        _ => (mat::CRYSTAL, mat::CLOTH_GOLD),
    }
}

/// A cage ball: a rim round its outline as the camera sees it, and two
/// ribs across that turn as it rolls.
pub(super) fn ball(m: &mut Mesh, c: V3, f: V3, r: V3, spin: f32, seat: u32) {
    let (metal, band) = ball_material(seat);
    let (s, co) = spin.sin_cos();
    // The roll turns about the across axis.
    let fwd = f * co + V3::UP * s;
    let up = V3::UP * co - f * s;
    hoop(m, c, r, V3::UP, BALL_R - 0.07, 0.09, 18, metal);
    hoop(m, c, fwd, up, BALL_R - 0.08, 0.08, 14, metal);
    hoop(m, c, r, up, BALL_R - 0.09, 0.08, 14, band);
}

/// An ore cart: a tub on four wheels, tipped by `tip` radians.
pub(super) fn cart(m: &mut Mesh, c: V3, f: V3, r: V3, tip: f32, seat: u32) {
    let (s, co) = tip.sin_cos();
    let fwd = f * co + V3::UP * s;
    let up = V3::UP * co - f * s;
    let (_, band) = ball_material(seat);
    let at = |a: f32, b: f32, h: f32| c + fwd * a + r * b + up * h;
    // Floor, then four sides, open on top: lantern-lit timber.
    cuboid(m, at(0.0, 0.0, 0.2), fwd * 0.6, r * 0.5, up * 0.05, mat::STABLE_TIMBER);
    cuboid(m, at(0.62, 0.0, 0.47), fwd * 0.04, r * 0.52, up * 0.27, mat::STABLE_TIMBER);
    cuboid(m, at(-0.62, 0.0, 0.47), fwd * 0.04, r * 0.52, up * 0.27, mat::STABLE_TIMBER);
    cuboid(m, at(0.0, 0.52, 0.47), fwd * 0.62, r * 0.04, up * 0.27, mat::STABLE_TIMBER);
    cuboid(m, at(0.0, -0.52, 0.47), fwd * 0.62, r * 0.04, up * 0.27, mat::STABLE_TIMBER);
    // Iron bands round the rim, one in the knight's colour.
    cuboid(m, at(0.64, 0.0, 0.73), fwd * 0.05, r * 0.56, up * 0.04, band);
    cuboid(m, at(-0.64, 0.0, 0.73), fwd * 0.05, r * 0.56, up * 0.04, band);
    cuboid(m, at(0.0, 0.54, 0.73), fwd * 0.66, r * 0.04, up * 0.04, mat::ARMOR);
    cuboid(m, at(0.0, -0.54, 0.73), fwd * 0.66, r * 0.04, up * 0.04, mat::ARMOR);
    for (a, b) in [(0.4, 0.5), (-0.4, 0.5), (0.4, -0.5), (-0.4, -0.5)] {
        hoop(m, at(a, b, 0.14), fwd, up, 0.04, 0.1, 6, mat::ARMOR);
    }
}

/// A griffin seen from behind and above: a tawny lion's body tapering to
/// a tufted tail, an eagle's white head, and broad wings beating with
/// `beat`, tipped up a little so they read from the saddle.
pub(super) fn griffin(m: &mut Mesh, c: V3, f: V3, r: V3, beat: f32, roll: f32, seat: u32) {
    let (s, co) = roll.sin_cos();
    let right = r * co + V3::UP * s;
    let up = V3::UP * co - r * s;
    let (_, band) = ball_material(seat);
    cuboid(m, c, f * 0.55, right * 0.24, up * 0.16, mat::HORSE_BAY);
    cuboid(m, c - f * 0.55, f * 0.2, right * 0.16, up * 0.12, mat::HORSE_BAY);
    cuboid(m, c + f * 0.7 + up * 0.14, f * 0.2, right * 0.17, up * 0.17, mat::HORSE_GREY);
    cuboid(m, c + f * 0.95 + up * 0.1, f * 0.1, right * 0.05, up * 0.05, mat::CLOTH_GOLD);
    // The tail: a tuft on a line.
    let tail = c - f * 1.15 + up * 0.05;
    m.push_quad(tail + right * 0.18, tail - right * 0.18, tail - right * 0.1 - f * 0.4, tail + right * 0.1 - f * 0.4, mat::HORSE_CHESTNUT);
    // A saddle cloth in the knight's colour.
    cuboid(m, c + up * 0.17, f * 0.22, right * 0.26, up * 0.02, band);
    let lift = beat.sin() * 0.5;
    for side in [-1.0f32, 1.0] {
        let root = c + right * (side * 0.22) + up * 0.12;
        let elbow = root + right * (side * 0.8) + up * (0.18 + lift * 0.5);
        let tip = elbow + right * (side * 0.75) + up * (0.12 + lift);
        m.push_quad(root + f * 0.4, elbow + f * 0.3, elbow - f * 0.4, root - f * 0.35, mat::HORSE_CHESTNUT);
        m.push_quad(elbow + f * 0.3, tip + f * 0.05, tip - f * 0.55, elbow - f * 0.4, mat::HORSE_BAY);
    }
}

/// A gargoyle on the wing: a hunched stone body, horns, two coal eyes and
/// beating stone wings.
pub(super) fn gargoyle(m: &mut Mesh, c: V3, f: V3, r: V3, beat: f32) {
    cuboid(m, c, f * 0.45, r * 0.45, V3::UP * 0.5, mat::STONE);
    cuboid(m, c - f * 0.2 + V3::UP * 0.62, f * 0.3, r * 0.3, V3::UP * 0.22, mat::STONE);
    for side in [-1.0f32, 1.0] {
        cuboid(m, c - f * 0.2 + r * (side * 0.22) + V3::UP * 0.92, f * 0.05, r * 0.05, V3::UP * 0.14, mat::STONE_DARK);
        cuboid(m, c - f * 0.51 + V3::UP * 0.66 + r * (side * 0.13), f * 0.02, r * 0.07, V3::UP * 0.06, mat::FIRE);
    }
    let lift = beat.sin() * 0.6;
    for side in [-1.0f32, 1.0] {
        let root = c + r * (side * 0.45) + V3::UP * 0.3;
        let tip = root + r * (side * 1.4) + V3::UP * (0.5 + lift);
        m.push_quad(root + f * 0.3, tip, tip - f * 0.5, root - f * 0.4, mat::STONE_DARK);
    }
}

/// Where a rider is drawn: its course position, except while the rope
/// hauls a fallen knight back up for the last third of the crash.
pub(super) fn drawn_at(t: &Track, rider: &Rider) -> (f32, f32, f32) {
    let hoist = r3::CRASH / 3;
    if matches!(rider.wreck, Wreck::Fall | Wreck::Gap) && rider.crash > 0 && rider.crash <= hoist {
        let k = 1.0 - rider.crash as f32 / hoist as f32;
        let back = (t.post(rider.s) - r3::RUN_UP).max(1.0);
        let lane_x = if rider.wreck == Wreck::Gap { rider.x } else { 0.0 };
        let lift = 2.5;
        return (
            rider.s + (back - rider.s) * k,
            rider.x + (lane_x - rider.x) * k,
            rider.z + (lift - rider.z) * k,
        );
    }
    (rider.s, rider.x, rider.z)
}

/// Whether a rival is left out of `seat`'s view: one just behind or beside
/// the seat's ride is between it and the camera, and would fill the lens.
pub(super) fn hidden(race: &Race3d, seat: Option<u32>, id: u32) -> bool {
    let t = r3::track(race.course);
    let Some(me) = seat.filter(|&s| s != id).and_then(|s| race.riders.get(&s)) else {
        return false;
    };
    let Some(other) = race.riders.get(&id) else {
        return false;
    };
    let (mine, theirs) = (drawn_at(t, me).0, drawn_at(t, other).0);
    theirs < mine - 1.2 && theirs > mine - CAMERA_BACK - 1.0
}

/// How far behind its ride a camera sits, at most.
pub(super) const CAMERA_BACK: f32 = 5.6;

/// Everything that moves this frame, into `m`.
pub(super) fn movers(m: &mut Mesh, race: &Race3d, seat: Option<u32>) {
    m.tris.clear();
    let t = r3::track(race.course);
    let tick = race.t as f32;
    for (i, item) in t.items.iter().enumerate() {
        let taken = race.taken.contains(&(i as u16));
        match item.kind {
            ItemKind::Gem if !taken => {
                let material = if race.course == Course::Cart { mat::CLOTH_GOLD } else { mat::CRYSTAL };
                let bob = (tick * 0.12 + i as f32).sin() * 0.06;
                diamond(m, p(t, item.s, item.x, item.z + bob), 0.16, tick * 0.08 + i as f32, material);
            }
            ItemKind::Ring => {
                // A ring this knight has flown goes dull.
                let mine = seat.and_then(|id| race.riders.get(&id)).is_some_and(|r| r.rings & (1u64 << i) != 0);
                let (_, rt, up) = frame(t, item.s);
                let material = if mine { mat::STONE_DARK } else { mat::CLOTH_GOLD };
                hoop(m, p(t, item.s, item.x, item.z), rt, up, 1.1, 0.16, 16, material);
            }
            ItemKind::Gargoyle if !taken => {
                let (f, rt, _) = frame(t, item.s);
                let hover = (tick * 0.1 + i as f32).sin() * 0.2;
                gargoyle(m, p(t, item.s, item.x, item.z + hover), f, rt, tick * 0.4 + i as f32);
            }
            _ => {}
        }
    }
    match race.course {
        Course::Shaft => {
            for (cs, cx) in r3::crossing_carts(t, race.t) {
                if cx.abs() < 6.0 {
                    let (f, rt, _) = frame(t, cs);
                    cart(m, p(t, cs, cx, 0.0), rt, f, 0.0, 0);
                    diamond(m, p(t, cs, cx, 0.75), 0.3, 0.3, mat::ROCK);
                }
            }
            // Rotten planks still standing, and the ones going.
            let mut s = 0.0f32;
            while s < t.len() {
                if t.at(s).ground == Ground::Collapse {
                    let plank = (s / 2.0) as u16;
                    if r3::rotten(plank) {
                        let half = t.at(s).width * 0.5;
                        let went = race.planks.get(&plank).map(|&b| race.t.saturating_sub(b + r3::ROT));
                        let drop = match went {
                            Some(0) | None => 0.0,
                            Some(k) => 0.5 * 14.0 * (k as f32 / 30.0).powi(2),
                        };
                        let shake = match race.planks.get(&plank) {
                            Some(&b) if race.t < b + r3::ROT => ((tick * 1.7).sin()) * 0.04,
                            _ => 0.0,
                        };
                        if drop < 14.0 {
                            floor(m, t, s, s + 2.0, -half, half, -drop + shake, 1.0, mat::TACK_LEATHER);
                        }
                    }
                }
                s += 2.0;
            }
        }
        Course::Griffin => {
            for bolt in &race.bolts {
                let (f, rt, _) = frame(t, bolt.s);
                cuboid(m, p(t, bolt.s, bolt.x, bolt.z), f * 0.6, rt * 0.05, V3::UP * 0.05, mat::EMBER);
            }
        }
        Course::Cart => {}
    }
    for (&id, rider) in &race.riders {
        if hidden(race, seat, id) {
            continue;
        }
        let (s, x, z) = drawn_at(t, rider);
        let (f, rt, _) = frame(t, s);
        match race.course {
            Course::Shaft => ball(m, p(t, s, x, z + BALL_R), f, rt, rider.spin, id),
            Course::Cart => {
                let tip = if rider.crash > 0 && rider.wreck != Wreck::Gap { rider.spin } else { 0.0 };
                cart(m, p(t, s, x, z), f, rt, tip, id);
            }
            Course::Griffin => {
                let roll = if rider.rolling > 0 {
                    (1.0 - rider.rolling as f32 / r3::ROLL as f32) * std::f32::consts::TAU
                } else if rider.crash > 0 {
                    rider.spin
                } else {
                    -rider.vx * 0.08
                };
                griffin(m, p(t, s, x, z), f, rt, rider.spin, roll, id);
            }
        }
    }
}
