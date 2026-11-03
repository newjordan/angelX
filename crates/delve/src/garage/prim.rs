//! Small solids for the garage's meshes: boxes, barrels, cones, beams and
//! chains, in the world3d mesh and its material ids. Written the way the
//! knight racers example builds its racers, which they were copied from.

use crate::render::world3d::math::{V3, v3};
use crate::render::world3d::mesh::Mesh;
use std::f32::consts::{PI, TAU};

pub fn tri(m: &mut Mesh, a: V3, b: V3, c: V3, k: u8) {
    m.push_tri([a, b, c], [[0.0; 2]; 3], k);
}

pub fn quad(m: &mut Mesh, a: V3, b: V3, c: V3, d: V3, k: u8) {
    m.push_quad(a, b, c, d, k);
}

/// An axis-aligned box from `lo` to `hi`.
pub fn cuboid(m: &mut Mesh, lo: V3, hi: V3, k: u8) {
    let p = |x: bool, y: bool, z: bool| {
        v3(if x { hi.x } else { lo.x }, if y { hi.y } else { lo.y }, if z { hi.z } else { lo.z })
    };
    let (f, t) = (false, true);
    m.push_quad(p(f, f, t), p(t, f, t), p(t, t, t), p(f, t, t), k);
    m.push_quad(p(f, f, f), p(t, f, f), p(t, t, f), p(f, t, f), k);
    m.push_quad(p(f, f, f), p(t, f, f), p(t, f, t), p(f, f, t), k);
    m.push_quad(p(f, t, f), p(t, t, f), p(t, t, t), p(f, t, t), k);
    m.push_quad(p(f, f, f), p(f, t, f), p(f, t, t), p(f, f, t), k);
    m.push_quad(p(t, f, f), p(t, t, f), p(t, t, t), p(t, f, t), k);
}

/// The same box centred on `c` with half-sizes `h`.
pub fn block(m: &mut Mesh, c: V3, h: V3, k: u8) {
    cuboid(m, c - h, c + h, k);
}

/// An n-gon prism along +x from `x0` to `x1`, centred on (y, z), radii r0 to
/// r1.
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

/// A frame (u, w) perpendicular to an axis.
fn frame(ax: V3) -> (V3, V3) {
    let u = ax.cross(if ax.z.abs() > 0.9 { v3(1.0, 0.0, 0.0) } else { V3::UP }).normalize();
    (u, ax.cross(u))
}

/// A prism or frustum between two points: `r0` at `a`, `r1` at `b`, `n`
/// sides.
pub fn cyl(m: &mut Mesh, a: V3, b: V3, [r0, r1]: [f32; 2], n: usize, k: u8, caps: bool) {
    let (u, w) = frame((b - a).normalize());
    let ring = |p: V3, r: f32, i: usize| {
        let (s, c) = ((i as f32 + 0.5) / n as f32 * TAU).sin_cos();
        p + (u * c + w * s) * r
    };
    for i in 0..n {
        let j = (i + 1) % n;
        m.push_quad(ring(a, r0, i), ring(a, r0, j), ring(b, r1, j), ring(b, r1, i), k);
        if caps {
            tri(m, a, ring(a, r0, i), ring(a, r0, j), k);
            if r1 > 0.0 {
                tri(m, b, ring(b, r1, i), ring(b, r1, j), k);
            }
        }
    }
}

/// Four-sided cone from a ring at `base` (radius r) to a tip.
pub fn spike(m: &mut Mesh, base: V3, tip: V3, r: f32, k: u8) {
    let (u, w) = frame((tip - base).normalize());
    let at = |i: usize| {
        let (s, c) = (i as f32 / 4.0 * TAU).sin_cos();
        base + (u * c + w * s) * r
    };
    for i in 0..4 {
        tri(m, at(i), at(i + 1), tip, k);
    }
}

/// An n-sided cone.
pub fn cone(m: &mut Mesh, base: V3, tip: V3, r: f32, n: usize, k: u8) {
    cyl(m, base, tip, [r, 0.0], n, k, true);
}

/// A flat n-gon facing +z, centred on `c`.
pub fn disc(m: &mut Mesh, c: V3, r: f32, n: usize, k: u8) {
    for i in 0..n {
        let (a0, a1) = (i as f32 / n as f32 * TAU, (i + 1) as f32 / n as f32 * TAU);
        tri(m, c, c + v3(r * a0.cos(), r * a0.sin(), 0.0), c + v3(r * a1.cos(), r * a1.sin(), 0.0), k);
    }
}

/// A square bar of half-width `w` between two points.
pub fn beam(m: &mut Mesh, a: V3, b: V3, w: f32, k: u8) {
    let (u, v) = frame((b - a).normalize());
    let (u, v) = (u * w, v * w);
    let c = |p: V3, s: f32, t: f32| p + u * s + v * t;
    let sides = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];
    for i in 0..4 {
        let (s0, t0) = sides[i];
        let (s1, t1) = sides[(i + 1) % 4];
        m.push_quad(c(a, s0, t0), c(a, s1, t1), c(b, s1, t1), c(b, s0, t0), k);
    }
    m.push_quad(c(a, 1.0, 1.0), c(a, -1.0, 1.0), c(a, -1.0, -1.0), c(a, 1.0, -1.0), k);
    m.push_quad(c(b, 1.0, 1.0), c(b, -1.0, 1.0), c(b, -1.0, -1.0), c(b, 1.0, -1.0), k);
}

/// A hanging chain: alternating links edge-on and face-on.
pub fn chain(m: &mut Mesh, a: V3, b: V3, w: f32, k: u8) {
    let n = ((b - a).length() / (w * 5.5)).max(1.0) as usize;
    for i in 0..n {
        let (p0, p1) = (a.lerp(b, i as f32 / n as f32), a.lerp(b, (i as f32 + 0.85) / n as f32));
        let side = if i % 2 == 0 { v3(w, 0.0, 0.0) } else { v3(0.0, w, 0.0) };
        m.push_quad(p0 - side, p1 - side, p1 + side, p0 + side, k);
    }
}

/// Move, turn about +z and scale the triangles from `first` on: how a part
/// built at the origin is set where it belongs.
pub fn place(m: &mut Mesh, first: usize, scale: f32, yaw: f32, at: V3) {
    let (s, c) = yaw.sin_cos();
    for t in &mut m.tris[first..] {
        for p in &mut t.v {
            let (x, y) = (p.x * scale, p.y * scale);
            *p = v3(x * c - y * s, x * s + y * c, p.z * scale) + at;
        }
        let (nx, ny) = (t.normal.x, t.normal.y);
        t.normal = v3(nx * c - ny * s, nx * s + ny * c, t.normal.z);
        for uv in &mut t.uv {
            uv[0] *= scale;
            uv[1] *= scale;
        }
    }
}

/// The corners of the box a mesh fills.
pub fn bounds(m: &Mesh) -> Option<(V3, V3)> {
    let mut it = m.tris.iter().flat_map(|t| t.v);
    let first = it.next()?;
    Some(it.fold((first, first), |(lo, hi), p| {
        (v3(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z)), v3(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z)))
    }))
}
