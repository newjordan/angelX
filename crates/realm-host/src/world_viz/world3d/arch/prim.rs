//! Low-level mesh primitives for the architecture generators.
//!
//! Two rules hold everywhere in this file:
//! 1. **Degenerate faces are dropped at the door.** Every triangle that reaches
//!    the `Mesh` has a finite, unit-length normal, so the raster shader never
//!    sees a NaN.
//! 2. **No interior/back faces are ever built.** Triangles are double-sided
//!    (the shader flips the flat normal toward the camera), so a wall is one
//!    quad and an open-backed merlon silhouettes exactly like a solid one.

use std::f32::consts::TAU;

use super::super::math::{V3, v3};
use super::super::mesh::Mesh;

pub const F_NX: u8 = 1 << 0;
pub const F_PX: u8 = 1 << 1;
pub const F_NY: u8 = 1 << 2;
pub const F_PY: u8 = 1 << 3;
pub const F_NZ: u8 = 1 << 4;
pub const F_PZ: u8 = 1 << 5;
pub const F_SIDES: u8 = F_NX | F_PX | F_NY | F_PY;
pub const F_SIDES_TOP: u8 = F_SIDES | F_PZ;
pub const F_ALL: u8 = F_SIDES | F_NZ | F_PZ;

pub fn tri(m: &mut Mesh, v: [V3; 3], uv: [[f32; 2]; 3], mat: u8) {
    let n = (v[1] - v[0]).cross(v[2] - v[0]);
    let area2 = n.length();
    if !(area2.is_finite() && area2 > 1e-6) {
        return;
    }
    m.push_tri(v, uv, mat);
}

pub fn quad(m: &mut Mesh, a: V3, b: V3, c: V3, d: V3, mat: u8) {
    let u = (b - a).length();
    let v = (d - a).length();
    tri(m, [a, b, c], [[0.0, 0.0], [u, 0.0], [u, v]], mat);
    tri(m, [a, c, d], [[0.0, 0.0], [u, v], [0.0, v]], mat);
}

fn planar_uv(p: V3, o: V3, right: V3, up: V3) -> [f32; 2] {
    let d = p - o;
    [d.dot(right), d.dot(up)]
}

pub fn tri_planar(m: &mut Mesh, o: V3, right: V3, up: V3, p: [V3; 3], mat: u8) {
    tri(
        m,
        p,
        [
            planar_uv(p[0], o, right, up),
            planar_uv(p[1], o, right, up),
            planar_uv(p[2], o, right, up),
        ],
        mat,
    );
}

pub fn boxed(m: &mut Mesh, min: V3, max: V3, mat: u8, faces: u8) {
    let (x0, y0, z0) = (min.x, min.y, min.z);
    let (x1, y1, z1) = (max.x, max.y, max.z);
    if faces & F_NZ != 0 {
        quad(
            m,
            v3(x0, y0, z0),
            v3(x1, y0, z0),
            v3(x1, y1, z0),
            v3(x0, y1, z0),
            mat,
        );
    }
    if faces & F_PZ != 0 {
        quad(
            m,
            v3(x0, y0, z1),
            v3(x1, y0, z1),
            v3(x1, y1, z1),
            v3(x0, y1, z1),
            mat,
        );
    }
    if faces & F_NY != 0 {
        quad(
            m,
            v3(x0, y0, z0),
            v3(x1, y0, z0),
            v3(x1, y0, z1),
            v3(x0, y0, z1),
            mat,
        );
    }
    if faces & F_PY != 0 {
        quad(
            m,
            v3(x0, y1, z0),
            v3(x1, y1, z0),
            v3(x1, y1, z1),
            v3(x0, y1, z1),
            mat,
        );
    }
    if faces & F_NX != 0 {
        quad(
            m,
            v3(x0, y0, z0),
            v3(x0, y1, z0),
            v3(x0, y1, z1),
            v3(x0, y0, z1),
            mat,
        );
    }
    if faces & F_PX != 0 {
        quad(
            m,
            v3(x1, y0, z0),
            v3(x1, y1, z0),
            v3(x1, y1, z1),
            v3(x1, y0, z1),
            mat,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn battered(
    m: &mut Mesh,
    cx: f32,
    cy: f32,
    z0: f32,
    z1: f32,
    hx0: f32,
    hy0: f32,
    hx1: f32,
    hy1: f32,
    mat: u8,
) {
    let a0 = v3(cx - hx0, cy - hy0, z0);
    let b0 = v3(cx + hx0, cy - hy0, z0);
    let c0 = v3(cx + hx0, cy + hy0, z0);
    let d0 = v3(cx - hx0, cy + hy0, z0);
    let a1 = v3(cx - hx1, cy - hy1, z1);
    let b1 = v3(cx + hx1, cy - hy1, z1);
    let c1 = v3(cx + hx1, cy + hy1, z1);
    let d1 = v3(cx - hx1, cy + hy1, z1);
    quad(m, a0, b0, b1, a1, mat);
    quad(m, b0, c0, c1, b1, mat);
    quad(m, c0, d0, d1, c1, mat);
    quad(m, d0, a0, a1, d1, mat);
}

fn ring_pt(cx: f32, cy: f32, r: f32, z: f32, a: f32) -> V3 {
    v3(cx + r * a.cos(), cy + r * a.sin(), z)
}

#[allow(clippy::too_many_arguments)]
pub fn prism(
    m: &mut Mesh,
    cx: f32,
    cy: f32,
    z0: f32,
    z1: f32,
    r0: f32,
    r1: f32,
    sides: usize,
    rot: f32,
    mat: u8,
) {
    let sides = sides.max(3);
    for i in 0..sides {
        let a0 = rot + TAU * (i as f32) / sides as f32;
        let a1 = rot + TAU * ((i + 1) as f32) / sides as f32;
        quad(
            m,
            ring_pt(cx, cy, r0, z0, a0),
            ring_pt(cx, cy, r0, z0, a1),
            ring_pt(cx, cy, r1, z1, a1),
            ring_pt(cx, cy, r1, z1, a0),
            mat,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn annulus(
    m: &mut Mesh,
    cx: f32,
    cy: f32,
    z: f32,
    r_in: f32,
    r_out: f32,
    sides: usize,
    rot: f32,
    mat: u8,
) {
    prism(m, cx, cy, z, z, r_in, r_out, sides, rot, mat);
}

#[allow(clippy::too_many_arguments)]
pub fn disc(
    m: &mut Mesh,
    cx: f32,
    cy: f32,
    z: f32,
    r: f32,
    sides: usize,
    rot: f32,
    mat: u8,
) {
    let sides = sides.max(3);
    let c = v3(cx, cy, z);
    for i in 0..sides {
        let a0 = rot + TAU * (i as f32) / sides as f32;
        let a1 = rot + TAU * ((i + 1) as f32) / sides as f32;
        let p0 = ring_pt(cx, cy, r, z, a0);
        let p1 = ring_pt(cx, cy, r, z, a1);
        tri(
            m,
            [c, p0, p1],
            [[0.0, 0.0], [p0.x - cx, p0.y - cy], [p1.x - cx, p1.y - cy]],
            mat,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn cone(
    m: &mut Mesh,
    cx: f32,
    cy: f32,
    z_base: f32,
    z_tip: f32,
    r: f32,
    sides: usize,
    rot: f32,
    mat: u8,
) {
    let sides = sides.max(3);
    let tip = v3(cx, cy, z_tip);
    let slant = ((z_tip - z_base).powi(2) + r * r).sqrt();
    for i in 0..sides {
        let a0 = rot + TAU * (i as f32) / sides as f32;
        let a1 = rot + TAU * ((i + 1) as f32) / sides as f32;
        let p0 = ring_pt(cx, cy, r, z_base, a0);
        let p1 = ring_pt(cx, cy, r, z_base, a1);
        if mat == super::super::mesh::mat::FLAME {
            let z_mid = z_base + (z_tip - z_base) * 0.43;
            let q0 = ring_pt(cx + r * 0.17, cy - r * 0.05, r * 0.74, z_mid, a0);
            let q1 = ring_pt(cx + r * 0.17, cy - r * 0.05, r * 0.74, z_mid, a1);
            tri(m, [p0, p1, q1], [[0.0, 0.0], [1.0, 0.0], [0.87, 0.43]], mat);
            tri(
                m,
                [p0, q1, q0],
                [[0.0, 0.0], [0.87, 0.43], [0.13, 0.43]],
                mat,
            );
            tri(
                m,
                [q0, q1, tip + v3(r * 0.45, -r * 0.08, 0.0)],
                [[0.13, 0.43], [0.87, 0.43], [0.5, 1.0]],
                mat,
            );
            continue;
        }
        tri(
            m,
            [p0, p1, tip],
            [[r * a0, 0.0], [r * a1, 0.0], [r * (a0 + a1) * 0.5, slant]],
            mat,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn poly_fan(
    m: &mut Mesh,
    centre: V3,
    right: V3,
    up: V3,
    r: f32,
    sides: usize,
    rot: f32,
    mat: u8,
) {
    let sides = sides.max(3);
    for i in 0..sides {
        let a0 = rot + TAU * (i as f32) / sides as f32;
        let a1 = rot + TAU * ((i + 1) as f32) / sides as f32;
        let p0 = centre + right * (r * a0.cos()) + up * (r * a0.sin());
        let p1 = centre + right * (r * a1.cos()) + up * (r * a1.sin());
        tri_planar(m, centre, right, up, [centre, p0, p1], mat);
    }
}

pub fn rect_panel(m: &mut Mesh, o: V3, right: V3, up: V3, w: f32, h: f32, mat: u8) {
    quad(m, o, o + right * w, o + right * w + up * h, o + up * h, mat);
}

#[allow(clippy::too_many_arguments)]
pub fn arched_panel(
    m: &mut Mesh,
    o: V3,
    right: V3,
    up: V3,
    w: f32,
    body_h: f32,
    segs: usize,
    mat: u8,
) {
    let segs = segs.max(3);
    let hw = w * 0.5;
    if body_h > 1e-4 {
        rect_panel(m, o, right, up, w, body_h, mat);
    }
    let springing = o + right * hw + up * body_h;
    for i in 0..segs {
        let a0 = std::f32::consts::PI * (1.0 - i as f32 / segs as f32);
        let a1 = std::f32::consts::PI * (1.0 - (i + 1) as f32 / segs as f32);
        let p0 = springing + right * (hw * a0.cos()) + up * (hw * a0.sin());
        let p1 = springing + right * (hw * a1.cos()) + up * (hw * a1.sin());
        tri_planar(m, o, right, up, [springing, p0, p1], mat);
    }
}

pub fn arch_profile(segs: usize, hw: f32, rise: f32) -> Vec<[f32; 2]> {
    let segs = segs.max(3);
    (0..=segs)
        .map(|i| {
            let a = std::f32::consts::PI * (1.0 - i as f32 / segs as f32);
            [hw * a.cos(), rise * a.sin()]
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub fn light_shaft(
    m: &mut Mesh,
    x: f32,
    from: (f32, f32),
    to: (f32, f32),
    half_w: f32,
    spread: f32,
    mat: u8,
) {
    let (w0, w1) = (half_w, half_w * spread.max(0.1));
    let lead = (to.0 - from.0).signum() * 0.62;
    let top = [
        v3(x - w0, from.0, from.1),
        v3(x + w0, from.0, from.1),
        v3(x, from.0 + lead * w0, from.1),
    ];
    let foot = [
        v3(x - w1, to.0, to.1),
        v3(x + w1, to.0, to.1),
        v3(x, to.0 + lead * w1, to.1),
    ];
    for i in 0..3 {
        let j = (i + 1) % 3;
        quad(m, top[i], top[j], foot[j], foot[i], mat);
    }
    tri(m, top, [[0.0, 0.0], [w0 * 2.0, 0.0], [w0, w0]], mat);
}

pub fn bird_profile(m: &mut Mesh, at: V3, across: V3, s: f32, mat: u8) {
    let across = across.normalize();
    if !(s.is_finite() && s > 1e-3 && across.length() > 0.5) {
        return;
    }
    const PROFILE: [(f32, f32); 11] = [
        (-0.62, 0.46),
        (-0.44, 0.60),
        (-0.26, 0.66),
        (-0.16, 0.44),
        (0.02, 0.54),
        (0.24, 0.48),
        (0.34, 0.34),
        (0.62, 0.06),
        (0.24, 0.12),
        (-0.10, 0.0),
        (-0.40, 0.22),
    ];
    const FACES: [[usize; 3]; 9] = [
        [0, 1, 10],
        [1, 2, 10],
        [2, 3, 10],
        [3, 9, 10],
        [3, 4, 9],
        [4, 5, 9],
        [5, 8, 9],
        [5, 6, 8],
        [6, 7, 8],
    ];
    let point = |(a, up): (f32, f32)| at + across * (a * s) + v3(0.0, 0.0, up * s);
    for face in FACES {
        tri_planar(
            m,
            at,
            across,
            V3::UP,
            [
                point(PROFILE[face[0]]),
                point(PROFILE[face[1]]),
                point(PROFILE[face[2]]),
            ],
            mat,
        );
    }
}

pub fn raven(m: &mut Mesh, at: V3, across: V3, s: f32, mat: u8) {
    let across = across.normalize();
    bird_profile(m, at, across, s, mat);

    let side = v3(-across.y, across.x, 0.0).normalize();
    const BULK: [(f32, f32); 5] = [
        (-0.30, 0.0),
        (-0.26, 0.36),
        (0.0, 0.50),
        (0.26, 0.36),
        (0.30, 0.0),
    ];
    let point = |(a, up): (f32, f32)| at + side * (a * s) + v3(0.0, 0.0, up * s);
    for face in [[0usize, 1, 4], [1, 2, 4], [2, 3, 4]] {
        tri_planar(
            m,
            at,
            side,
            V3::UP,
            [
                point(BULK[face[0]]),
                point(BULK[face[1]]),
                point(BULK[face[2]]),
            ],
            mat,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn merlons(
    m: &mut Mesh,
    p0: V3,
    p1: V3,
    inward: V3,
    h: f32,
    t: f32,
    pitch: f32,
    max_teeth: usize,
    mat: u8,
) -> usize {
    let seg = p1 - p0;
    let len = seg.length();
    if !(len.is_finite() && len > 1e-4 && pitch > 1e-4) || max_teeth == 0 {
        return 0;
    }
    let dir = seg / len;
    let inward = inward.normalize();
    let n = ((len / pitch).round() as i64).clamp(1, max_teeth as i64) as usize;
    let step = len / n as f32;
    let w = step * 0.58;
    let up = v3(0.0, 0.0, h);
    for i in 0..n {
        let c = p0 + dir * (step * (i as f32 + 0.5));
        let a = c - dir * (w * 0.5);
        let b = c + dir * (w * 0.5);
        let a2 = a + inward * t;
        let b2 = b + inward * t;
        quad(m, a, b, b + up, a + up, mat);
        quad(m, a + up, b + up, b2 + up, a2 + up, mat);
        quad(m, a, a + up, a2 + up, a2, mat);
        quad(m, b, b + up, b2 + up, b2, mat);
    }
    n
}

#[allow(clippy::too_many_arguments)]
pub fn gable_roof(
    m: &mut Mesh,
    cx: f32,
    cy: f32,
    half_x: f32,
    half_y: f32,
    z_eave: f32,
    z_ridge: f32,
    fascia: f32,
    cap: f32,
    mat: u8,
) {
    let (x0, x1) = (cx - half_x, cx + half_x);
    let (y0, y1) = (cy - half_y, cy + half_y);
    quad(
        m,
        v3(x0, y0, z_eave),
        v3(x1, y0, z_eave),
        v3(x1, cy, z_ridge),
        v3(x0, cy, z_ridge),
        mat,
    );
    quad(
        m,
        v3(x0, y1, z_eave),
        v3(x1, y1, z_eave),
        v3(x1, cy, z_ridge),
        v3(x0, cy, z_ridge),
        mat,
    );
    if fascia > 1e-4 {
        quad(
            m,
            v3(x0, y0, z_eave - fascia),
            v3(x1, y0, z_eave - fascia),
            v3(x1, y0, z_eave),
            v3(x0, y0, z_eave),
            mat,
        );
        quad(
            m,
            v3(x0, y1, z_eave - fascia),
            v3(x1, y1, z_eave - fascia),
            v3(x1, y1, z_eave),
            v3(x0, y1, z_eave),
            mat,
        );
    }
    if cap > 1e-4 {
        boxed(
            m,
            v3(x0, cy - cap, z_ridge - cap * 0.5),
            v3(x1, cy + cap, z_ridge + cap * 1.2),
            mat,
            F_SIDES_TOP,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn gable_end(
    m: &mut Mesh,
    x: f32,
    cy: f32,
    half_y: f32,
    z_eave: f32,
    z_ridge: f32,
    mat: u8,
) {
    let a = v3(x, cy - half_y, z_eave);
    let b = v3(x, cy + half_y, z_eave);
    let c = v3(x, cy, z_ridge);
    tri(
        m,
        [a, b, c],
        [[0.0, 0.0], [half_y * 2.0, 0.0], [half_y, z_ridge - z_eave]],
        mat,
    );
}

pub fn bbox(m: &Mesh) -> Option<(V3, V3)> {
    let first = m.tris.first()?;
    let mut lo = first.v[0];
    let mut hi = first.v[0];
    for t in &m.tris {
        for p in t.v {
            lo = v3(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = v3(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
    }
    Some((lo, hi))
}

pub fn stamp(out: &mut Mesh, part: &Mesh, places: &[(f32, f32, f32)]) {
    for &(x, y, yaw) in places {
        out.merge(part.clone().rotated_z(yaw).translated(v3(x, y, 0.0)));
    }
}
