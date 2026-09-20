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

// ---------------------------------------------------------------- face masks

/// Face selector bits for [`boxed`].
pub(crate) const F_NX: u8 = 1 << 0;
pub(crate) const F_PX: u8 = 1 << 1;
pub(crate) const F_NY: u8 = 1 << 2;
pub(crate) const F_PY: u8 = 1 << 3;
pub(crate) const F_NZ: u8 = 1 << 4;
pub(crate) const F_PZ: u8 = 1 << 5;
pub(crate) const F_SIDES: u8 = F_NX | F_PX | F_NY | F_PY;
pub(crate) const F_SIDES_TOP: u8 = F_SIDES | F_PZ;
pub(crate) const F_ALL: u8 = F_SIDES | F_NZ | F_PZ;

// ------------------------------------------------------------- guarded pushes

/// Push a triangle, dropping degenerate (zero-area or non-finite) faces so the
/// mesh never carries a zero or NaN normal.
pub(crate) fn tri(m: &mut Mesh, v: [V3; 3], uv: [[f32; 2]; 3], mat: u8) {
    let n = (v[1] - v[0]).cross(v[2] - v[0]);
    // Negate the whole predicate rather than flipping the comparison, so a NaN
    // area (from a NaN vertex) also fails and the face is dropped.
    let area2 = n.length();
    if !(area2.is_finite() && area2 > 1e-6) {
        return;
    }
    m.push_tri(v, uv, mat);
}

/// Quad a→b→c→d around the face — same UV convention as `Mesh::push_quad`, but
/// degenerate-safe (a collapsed edge drops only the bad half).
pub(crate) fn quad(m: &mut Mesh, a: V3, b: V3, c: V3, d: V3, mat: u8) {
    let u = (b - a).length();
    let v = (d - a).length();
    tri(m, [a, b, c], [[0.0, 0.0], [u, 0.0], [u, v]], mat);
    tri(m, [a, c, d], [[0.0, 0.0], [u, v], [0.0, v]], mat);
}

fn planar_uv(p: V3, o: V3, right: V3, up: V3) -> [f32; 2] {
    let d = p - o;
    [d.dot(right), d.dot(up)]
}

/// Triangle on an arbitrary plane, UVs projected onto the (`right`, `up`) basis
/// measured from `o`. Keeps surface patterning aligned across a facade.
pub(crate) fn tri_planar(m: &mut Mesh, o: V3, right: V3, up: V3, p: [V3; 3], mat: u8) {
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

// -------------------------------------------------------------------- boxes

/// Axis-aligned box between `min` and `max`, emitting only the faces named in
/// `faces` (see the `F_*` bits).
pub(crate) fn boxed(m: &mut Mesh, min: V3, max: V3, mat: u8, faces: u8) {
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

/// Battered (tapered) block: rectangular half-extents shrink from
/// `(hx0, hy0)` at `z0` to `(hx1, hy1)` at `z1`. Sides only — cap it separately
/// if the top is visible. The flare is what makes a keep read as *heavy* in
/// silhouette rather than as an extruded rectangle.
#[allow(clippy::too_many_arguments)]
pub(crate) fn battered(
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

// --------------------------------------------------------- radial primitives

fn ring_pt(cx: f32, cy: f32, r: f32, z: f32, a: f32) -> V3 {
    v3(cx + r * a.cos(), cy + r * a.sin(), z)
}

/// n-sided prism / truncated cone between two rings. `sides` is clamped to >= 3.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prism(
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

/// Flat horizontal ring — roof eaves, corbel tables, well copings.
#[allow(clippy::too_many_arguments)]
pub(crate) fn annulus(
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

/// Flat horizontal disc (triangle fan from the centre).
#[allow(clippy::too_many_arguments)]
pub(crate) fn disc(
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

/// Cone / spire: a fan from `z_tip` down to a ring of radius `r` at `z_base`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn cone(
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
        tri(
            m,
            [p0, p1, tip],
            [[r * a0, 0.0], [r * a1, 0.0], [r * (a0 + a1) * 0.5, slant]],
            mat,
        );
    }
}

/// Regular polygon on an arbitrary plane spanned by unit `right`/`up` — rose
/// windows, gable medallions, anything that is not floor-flat.
#[allow(clippy::too_many_arguments)]
pub(crate) fn poly_fan(
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

// ----------------------------------------------------------------- openings

/// Flat rectangular panel on the plane spanned by unit `right`/`up`, with `o`
/// as the bottom-left corner. Windows, doors, banners, table tops on edge.
pub(crate) fn rect_panel(m: &mut Mesh, o: V3, right: V3, up: V3, w: f32, h: f32, mat: u8) {
    quad(m, o, o + right * w, o + right * w + up * h, o + up * h, mat);
}

/// Round-arched opening panel: a rectangular body plus an `segs`-facet
/// semicircular head. `o` is the bottom-left corner, `w` the full width,
/// `body_h` the height of the straight part. Total height is `body_h + w/2`.
///
/// Five facets is the sweet spot at dot scale: fewer reads as a triangle,
/// more dissolves into the wall under dithering.
#[allow(clippy::too_many_arguments)]
pub(crate) fn arched_panel(
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

/// Points along a faceted round arch, left springing → crown → right springing,
/// expressed as `[across, up]` offsets from the springing midpoint. `segs + 1`
/// points. Used to build gate tunnels and spandrels.
pub(crate) fn arch_profile(segs: usize, hw: f32, rise: f32) -> Vec<[f32; 2]> {
    let segs = segs.max(3);
    (0..=segs)
        .map(|i| {
            let a = std::f32::consts::PI * (1.0 - i as f32 / segs as f32);
            [hw * a.cos(), rise * a.sin()]
        })
        .collect()
}

// -------------------------------------------------------------------- light

/// A wedge of light falling from an opening in a **+x-running wall** down onto
/// the floor: a three-sided prism whose axis runs from `from = (y, z)` at the
/// sill to `to = (y, z)` at the foot, `half_w` wide across `x` at the top and
/// `half_w * spread` at the bottom, capped at the sill.
///
/// A prism, not a sheet, and that is the whole point. The rooms in this realm
/// are shot down their long axis, so a flat slab hung on a side wall contains
/// the view direction and renders as a one-dot line — which is why the first
/// attempt at interior shafts was invisible before it was ever ugly. Two of a
/// prism's three faces always carry an ±x normal, so the beam turns toward an
/// axial camera no matter where along the wall it falls.
///
/// UVs run **0 at the sill**, which is the orientation `mat::MOONLIGHT`'s
/// fall-off is written for.
#[allow(clippy::too_many_arguments)]
pub(crate) fn light_shaft(
    m: &mut Mesh,
    x: f32,
    from: (f32, f32),
    to: (f32, f32),
    half_w: f32,
    spread: f32,
    mat: u8,
) {
    let (w0, w1) = (half_w, half_w * spread.max(0.1));
    // The prism's ridge leads the beam: displaced along the way it travels, so
    // the two lit faces splay off the wall instead of lying flat against it.
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

// -------------------------------------------------------------------- birds

/// A perched raven, in silhouette, on the vertical plane spanned by unit
/// `across` and +z. `at` is where its feet meet the perch; `s` is the body
/// length in tiles. Six triangles.
///
/// The bird is drawn, not boxed. Two stacked boxes make a *blob* with a notch,
/// and at the ranges this realm shows birds at — a rook on a tower eave 12.8
/// tiles out is about ten dots long — a blob reads as damage on the roofline.
/// A profile reads as a bird because three features survive the Bayer screen
/// even when the body is four dots tall: the **beak** poking forward of the
/// head, the **step** where the head sits above the shoulder, and the **notch**
/// under the tail where it lifts clear of the perch. Everything else is mass.
///
/// The profile faces **−across**. Its span is ±0.62 s across and 0 … 0.66 s up,
/// so a caller can place one on an eave rim knowing exactly how far it hangs.
pub(crate) fn bird_profile(m: &mut Mesh, at: V3, across: V3, s: f32, mat: u8) {
    let across = across.normalize();
    if !(s.is_finite() && s > 1e-3 && across.length() > 0.5) {
        return;
    }
    // (across, up), in body lengths, shifted so the silhouette is centred on
    // `at` rather than hanging off one side of it. The outline rises to the
    // crown, **dips at the nape**, rises again over the shoulder and then runs
    // away to the tail: that dip is the single most load-bearing coordinate
    // here. Without it the profile is one long ridge and every bird on the
    // roofline reads as a paper dart.
    const PROFILE: [(f32, f32); 11] = [
        (-0.62, 0.46), // 0  beak tip
        (-0.44, 0.60), // 1  forehead
        (-0.26, 0.66), // 2  crown
        (-0.16, 0.44), // 3  nape — the dip behind the head
        (0.02, 0.54),  // 4  shoulder
        (0.24, 0.48),  // 5  back
        (0.34, 0.34),  // 6  rump
        (0.62, 0.06),  // 7  tail tip
        (0.24, 0.12),  // 8  tail root, under — the notch it lifts clear of
        (-0.10, 0.0),  // 9  feet
        (-0.40, 0.22), // 10 breast
    ];
    const FACES: [[usize; 3]; 9] = [
        [0, 1, 10], // beak into the throat
        [1, 2, 10], // skull
        [2, 3, 10], // nape
        [3, 9, 10], // breast
        [3, 4, 9],  // neck into the body
        [4, 5, 9],  // back
        [5, 8, 9],  // belly
        [5, 6, 8],  // rump
        [6, 7, 8],  // tail
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

/// A raven with body across it: the full profile broadside to `across`, plus a
/// three-triangle **bulk** edge-on to it, so the bird still notches the skyline
/// when the camera walks round the mass it is perched on. Twelve triangles.
///
/// The bulk is deliberately not a second profile. Two crossed profiles at the
/// six-or-so dots a rook gets on a tower read as a *plus sign*, because the
/// beak and tail stick out on both axes at once. Front-on, a perched bird is
/// only ever a rounded lump, so that is what the cross plane draws.
pub(crate) fn raven(m: &mut Mesh, at: V3, across: V3, s: f32, mat: u8) {
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

// ------------------------------------------------------------- crenellations

/// Crenellation teeth along the segment `p0` → `p1` (both points on the OUTER
/// face at walkway height). `inward` is the horizontal unit vector pointing
/// into the wall; `t` is the tooth depth, `h` the tooth height.
///
/// Teeth are ~58% solid / ~42% gap and the count is derived from `pitch` and
/// clamped to `max_teeth`, which is what keeps a long curtain run inside its
/// triangle budget *and* keeps every tooth at least ~2 braille dots wide.
/// Returns the tooth count.
#[allow(clippy::too_many_arguments)]
pub(crate) fn merlons(
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
        quad(m, a, b, b + up, a + up, mat); // outer face
        quad(m, a + up, b + up, b2 + up, a2 + up, mat); // tooth top
        quad(m, a, a + up, a2 + up, a2, mat); // end cheek
        quad(m, b, b + up, b2 + up, b2, mat); // end cheek
    }
    n
}

// -------------------------------------------------------------------- roofs

/// Pitched roof with the ridge running along +x at `y = cy`. `half_x`/`half_y`
/// include the eave overhang. `fascia > 0` hangs a thin vertical board off each
/// eave — cheap, and it is what turns the roofline into a hard dark edge that
/// survives Bayer dithering. `cap > 0` adds a ridge crest of that half-width.
#[allow(clippy::too_many_arguments)]
pub(crate) fn gable_roof(
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
    // South slope, then north slope.
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

/// The triangular gable end wall at `x`, spanning `y = cy ± half_y`, from
/// `z_eave` up to the ridge point at `z_ridge`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn gable_end(
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

// ------------------------------------------------------------------ helpers

/// Axis-aligned bounds of a mesh, or `None` when it is empty.
pub(crate) fn bbox(m: &Mesh) -> Option<(V3, V3)> {
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

/// Build `m` once, then stamp it at each `(x, y, yaw)` placement.
pub(crate) fn stamp(out: &mut Mesh, part: &Mesh, places: &[(f32, f32, f32)]) {
    for &(x, y, yaw) in places {
        out.merge(part.clone().rotated_z(yaw).translated(v3(x, y, 0.0)));
    }
}
