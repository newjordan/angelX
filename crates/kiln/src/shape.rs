//! Shapes with depth: where a part's surface is under a model point.
//!
//! The view is straight down z (the viewer is at +z), so every pixel asks
//! each part one question: does the line through your model point meet
//! you, and if so how near is the surface and which way does it face? The
//! answer is exact for every shape, at any grain: an ellipsoid, a tapered
//! capsule (a sphere swept along a segment, its radius running from one end
//! to the other), a rounded box, a slab (a polygon with thickness and
//! bevelled edges, its face leaning or curving), a stroke (a Catmull-Rom
//! tube of capsules) and a flat (an unlit polygon).
//!
//! A part is written in its own space and posed by a rigid move about z
//! ([`Xf`]); since the view runs along z, posing is exact: the pixel's point
//! is moved back into the part's space, and the normal turned forward.

use crate::num;
use crate::recipe::{Cut, Part, ShapeKind};
use crate::rig::Xf;

/// A surface under a model point: its depth (larger is nearer) and its
/// normal, of unit length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub z: f32,
    pub n: [f32; 3],
}

/// A tapered capsule: a sphere swept from `a` (radius `ra`) to `b` (`rb`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seg {
    pub a: [f32; 3],
    pub ra: f32,
    pub b: [f32; 3],
    pub rb: f32,
}

#[derive(Clone, Debug, PartialEq)]
enum Form {
    Ellipsoid { c: [f32; 3], r: [f32; 3], cos: f32, sin: f32 },
    Tube { segs: Vec<Seg> },
    Box { c: [f32; 3], h: [f32; 3], round: f32, cos: f32, sin: f32 },
    Slab { poly: Vec<[f32; 2]>, face: f32, bevel: f32, lean: [f32; 2], curve: [f32; 2], mid: [f32; 2], half: [f32; 2] },
    Flat { poly: Vec<[f32; 2]>, z: f32 },
}

/// A part's shape, posed.
#[derive(Clone, Debug, PartialEq)]
pub struct Solid {
    form: Form,
    cut: Option<Cut>,
    xf: Xf,
    /// The box round it in its own space, `[x0, y0, x1, y1]`.
    own: [f32; 4],
}

/// Default slab thickness and bevel, in game pixels.
pub const SLAB_THICK: f32 = 0.5;
pub const SLAB_BEVEL: f32 = 0.3;

impl Solid {
    /// A part's shape (the recipe checked it), posed by `xf`.
    pub fn of(part: &Part, xf: Xf) -> Solid {
        let p2 = |v: &Vec<Vec<f32>>| v.iter().map(|p| [p[0], p[1]]).collect::<Vec<_>>();
        let form = match part.shape {
            ShapeKind::Ellipsoid => {
                let r = part.r.clone().unwrap_or_default();
                let (sin, cos) = num::sin_cos_deg(part.turn);
                Form::Ellipsoid { c: part.at.unwrap_or_default(), r: [r[0], r[1], r[2]], cos, sin }
            }
            ShapeKind::Capsule => {
                let r = part.r.clone().unwrap_or_default();
                let (ra, rb) = (r[0], *r.get(1).unwrap_or(&r[0]));
                Form::Tube { segs: vec![Seg { a: part.from.unwrap_or_default(), ra, b: part.to.unwrap_or_default(), rb }] }
            }
            ShapeKind::Box => {
                let s = part.size.unwrap_or_default();
                let h = [s[0] / 2.0, s[1] / 2.0, s[2] / 2.0];
                let (sin, cos) = num::sin_cos_deg(part.turn);
                let round = part.round.unwrap_or(0.0).min(h[0]).min(h[1]).min(h[2]);
                Form::Box { c: part.at.unwrap_or_default(), h, round, cos, sin }
            }
            ShapeKind::Slab => {
                let poly = p2(part.points.as_ref().unwrap());
                let [x0, y0, x1, y1] = poly_box(&poly);
                let thick = part.thick.unwrap_or(SLAB_THICK);
                Form::Slab {
                    face: part.z.unwrap_or(0.0) + thick / 2.0,
                    bevel: part.bevel.unwrap_or(SLAB_BEVEL).min(thick),
                    lean: part.lean.unwrap_or_default(),
                    curve: part.curve.unwrap_or_default(),
                    mid: [(x0 + x1) / 2.0, (y0 + y1) / 2.0],
                    half: [((x1 - x0) / 2.0).max(1e-3), ((y1 - y0) / 2.0).max(1e-3)],
                    poly,
                }
            }
            ShapeKind::Stroke => {
                let pts: Vec<[f32; 3]> = part.points.as_ref().unwrap().iter().map(|p| [p[0], p[1], p[2]]).collect();
                let w = part.width.clone().unwrap_or_default();
                Form::Tube { segs: stroke(&pts, w[0] / 2.0, *w.get(1).unwrap_or(&w[0]) / 2.0) }
            }
            ShapeKind::Flat => Form::Flat { poly: p2(part.points.as_ref().unwrap()), z: part.z.unwrap_or(0.0) },
        };
        let mut own = form_box(&form);
        if let Some(cut) = part.cut {
            if let Some([a, b]) = cut.x {
                own[0] = own[0].max(a);
                own[2] = own[2].min(b);
            }
            if let Some([a, b]) = cut.y {
                own[1] = own[1].max(a);
                own[3] = own[3].min(b);
            }
        }
        Solid { form, cut: part.cut, xf, own }
    }

    /// The box round it in model space, `[x0, y0, x1, y1]`; empty when
    /// `x0 > x1`.
    pub fn bounds(&self) -> [f32; 4] {
        let [x0, y0, x1, y1] = self.own;
        if x0 > x1 || y0 > y1 {
            return [1.0, 1.0, 0.0, 0.0];
        }
        let corners = [[x0, y0], [x1, y0], [x0, y1], [x1, y1]].map(|p| self.xf.apply(p));
        let mut b = [f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY];
        for [x, y] in corners {
            b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
        }
        b
    }

    /// The surface under model point `(x, y)`, if any.
    pub fn hit(&self, x: f32, y: f32) -> Option<Hit> {
        let [u, v] = self.xf.unapply([x, y]);
        if let Some(cut) = self.cut {
            let out = |r: Option<[f32; 2]>, t: f32| r.is_some_and(|[a, b]| t < a || t > b);
            if out(cut.x, u) || out(cut.y, v) {
                return None;
            }
        }
        let h = form_hit(&self.form, u, v)?;
        let [nx, ny] = self.xf.turn([h.n[0], h.n[1]]);
        Some(Hit { z: h.z + self.xf.tz, n: [nx, ny, h.n[2]] })
    }

    /// A tube's axis in model space, posed: its points and radii, end to
    /// end; none for other shapes.
    pub fn axis(&self) -> Option<Vec<([f32; 3], f32)>> {
        let Form::Tube { segs } = &self.form else { return None };
        let at = |p: [f32; 3]| {
            let [x, y] = self.xf.apply([p[0], p[1]]);
            [x, y, p[2] + self.xf.tz]
        };
        let mut out = vec![(at(segs[0].a), segs[0].ra)];
        out.extend(segs.iter().map(|s| (at(s.b), s.rb)));
        Some(out)
    }

    /// Whether it is lit (a flat is not).
    pub fn is_lit(&self) -> bool {
        !matches!(self.form, Form::Flat { .. })
    }
}

fn unit([x, y, z]: [f32; 3]) -> [f32; 3] {
    let l = (x * x + y * y + z * z).sqrt();
    if l > 0.0 { [x / l, y / l, z / l] } else { [0.0, 0.0, 1.0] }
}

fn poly_box(poly: &[[f32; 2]]) -> [f32; 4] {
    poly.iter().fold([f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY], |b, &[x, y]| {
        [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)]
    })
}

fn form_box(form: &Form) -> [f32; 4] {
    match form {
        Form::Ellipsoid { c, r, cos, sin } => {
            let ex = ((r[0] * cos).powi(2) + (r[1] * sin).powi(2)).sqrt();
            let ey = ((r[0] * sin).powi(2) + (r[1] * cos).powi(2)).sqrt();
            [c[0] - ex, c[1] - ey, c[0] + ex, c[1] + ey]
        }
        Form::Tube { segs } => segs.iter().fold([f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY], |b, s| {
            [
                b[0].min(s.a[0] - s.ra).min(s.b[0] - s.rb),
                b[1].min(s.a[1] - s.ra).min(s.b[1] - s.rb),
                b[2].max(s.a[0] + s.ra).max(s.b[0] + s.rb),
                b[3].max(s.a[1] + s.ra).max(s.b[1] + s.rb),
            ]
        }),
        Form::Box { c, h, cos, sin, .. } => {
            let ex = (h[0] * cos).abs() + (h[1] * sin).abs();
            let ey = (h[0] * sin).abs() + (h[1] * cos).abs();
            [c[0] - ex, c[1] - ey, c[0] + ex, c[1] + ey]
        }
        Form::Slab { poly, .. } | Form::Flat { poly, .. } => poly_box(poly),
    }
}

fn form_hit(form: &Form, x: f32, y: f32) -> Option<Hit> {
    match form {
        Form::Ellipsoid { c, r, cos, sin } => {
            let (dx, dy) = (x - c[0], y - c[1]);
            let (u, v) = (cos * dx + sin * dy, -sin * dx + cos * dy);
            let (nx, ny) = (u / r[0], v / r[1]);
            let r2 = nx * nx + ny * ny;
            if r2 > 1.0 {
                return None;
            }
            let nz = (1.0 - r2).sqrt();
            let [a, b, n3] = unit([nx / r[0], ny / r[1], nz / r[2]]);
            Some(Hit { z: c[2] + r[2] * nz, n: [cos * a - sin * b, sin * a + cos * b, n3] })
        }
        Form::Tube { segs } => {
            let mut best: Option<Hit> = None;
            for s in segs {
                if let Some(h) = seg_hit(s, x, y)
                    && best.is_none_or(|b| h.z > b.z)
                {
                    best = Some(h);
                }
            }
            best
        }
        Form::Box { c, h, round, cos, sin } => {
            let (dx, dy) = (x - c[0], y - c[1]);
            let (u, v) = (cos * dx + sin * dy, -sin * dx + cos * dy);
            if u.abs() > h[0] || v.abs() > h[1] {
                return None;
            }
            if *round <= 0.0 {
                return Some(Hit { z: c[2] + h[2], n: [0.0, 0.0, 1.0] });
            }
            let du = (u.abs() - (h[0] - round)).max(0.0);
            let dv = (v.abs() - (h[1] - round)).max(0.0);
            let d2 = du * du + dv * dv;
            if d2 > round * round {
                return None;
            }
            let nz = (round * round - d2).sqrt();
            let (a, b) = (du * u.signum() / round, dv * v.signum() / round);
            Some(Hit { z: c[2] + h[2] - round + nz, n: [cos * a - sin * b, sin * a + cos * b, nz / round] })
        }
        Form::Slab { poly, face, bevel, lean, curve, mid, half } => {
            if !inside(poly, x, y) {
                return None;
            }
            let u = ((x - mid[0]) / half[0]).clamp(-1.0, 1.0);
            let v = ((y - mid[1]) / half[1]).clamp(-1.0, 1.0);
            let f = unit([lean[0] + curve[0] * u, lean[1] + curve[1] * v, 1.0]);
            let mut z = face - lean[0] * (x - mid[0]) - lean[1] * (y - mid[1]) - curve[0] * half[0] * u * u / 2.0 - curve[1] * half[1] * v * v / 2.0;
            let mut n = f;
            if *bevel > 0.0 {
                let (d, [ox, oy]) = edge(poly, x, y);
                if d < *bevel {
                    // A quarter round from the face down to the edge.
                    let s = (bevel - d) / bevel;
                    let c = (1.0 - s * s).max(0.0).sqrt();
                    z -= bevel * (1.0 - c);
                    n = unit([ox * s + f[0] * c, oy * s + f[1] * c, f[2] * c]);
                }
            }
            Some(Hit { z, n })
        }
        Form::Flat { poly, z } => inside(poly, x, y).then_some(Hit { z: *z, n: [0.0, 0.0, 1.0] }),
    }
}

/// The front of a tapered capsule under `(x, y)`: the nearest point on the
/// swept spheres, found in closed form.
fn seg_hit(s: &Seg, x: f32, y: f32) -> Option<Hit> {
    let d = [s.b[0] - s.a[0], s.b[1] - s.a[1], s.b[2] - s.a[2]];
    let dr = s.rb - s.ra;
    let (ex, ey) = (x - s.a[0], y - s.a[1]);
    // A cheap miss: farther from the segment's line in the plane than its
    // widest radius.
    let len2 = d[0] * d[0] + d[1] * d[1];
    let t0 = if len2 > 0.0 { ((ex * d[0] + ey * d[1]) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let (px, py) = (ex - t0 * d[0], ey - t0 * d[1]);
    let rmax = s.ra.max(s.rb);
    if px * px + py * py > rmax * rmax {
        return None;
    }
    // g(t) = r(t)^2 - |e - t d_xy|^2, the squared half-chord of the sphere at t.
    let qa = dr * dr - len2;
    let qb = 2.0 * s.ra * dr + 2.0 * (ex * d[0] + ey * d[1]);
    let qc = s.ra * s.ra - (ex * ex + ey * ey);
    let g = |t: f32| qa * t * t + qb * t + qc;
    let front = |t: f32| s.a[2] + t * d[2] + g(t).max(0.0).sqrt();
    // Where z(t) = a.z + t d.z + sqrt(g(t)) stands still:
    // 4A(A - dz^2) t^2 + 4B(A - dz^2) t + B^2 - 4 dz^2 C = 0.
    let k = qa - d[2] * d[2];
    let (a2, a1, a0) = (4.0 * qa * k, 4.0 * qb * k, qb * qb - 4.0 * d[2] * d[2] * qc);
    let mut ts = [0.0f32, 1.0, f32::NAN, f32::NAN];
    if a2.abs() > 1e-9 {
        let disc = a1 * a1 - 4.0 * a2 * a0;
        let root = disc.max(0.0).sqrt();
        ts[2] = (-a1 + root) / (2.0 * a2);
        ts[3] = (-a1 - root) / (2.0 * a2);
    } else if a1.abs() > 1e-9 {
        ts[2] = -a0 / a1;
    }
    let mut best: Option<(f32, f32)> = None;
    for t in ts {
        if !t.is_finite() {
            continue;
        }
        let t = t.clamp(0.0, 1.0);
        if g(t) < 0.0 {
            continue;
        }
        let z = front(t);
        if best.is_none_or(|(_, bz)| z > bz) {
            best = Some((t, z));
        }
    }
    let (t, z) = best?;
    let r = s.ra + t * dr;
    let c = [s.a[0] + t * d[0], s.a[1] + t * d[1], s.a[2] + t * d[2]];
    Some(Hit { z, n: unit([(x - c[0]) / r, (y - c[1]) / r, (z - c[2]) / r]) })
}

/// A Catmull-Rom tube through `pts`, its radius running from `r0` to `r1`,
/// as capsules about a quarter of a game pixel long.
fn stroke(pts: &[[f32; 3]], r0: f32, r1: f32) -> Vec<Seg> {
    let n = pts.len();
    let at = |i: isize| pts[i.clamp(0, n as isize - 1) as usize];
    let mut samples: Vec<[f32; 3]> = vec![pts[0]];
    for i in 0..n - 1 {
        let (p0, p1, p2, p3) = (at(i as isize - 1), at(i as isize), at(i as isize + 1), at(i as isize + 2));
        let span = ((p2[0] - p1[0]).powi(2) + (p2[1] - p1[1]).powi(2) + (p2[2] - p1[2]).powi(2)).sqrt();
        let steps = ((span * 4.0).ceil() as usize).clamp(1, 64);
        for k in 1..=steps {
            let t = k as f32 / steps as f32;
            let (t2, t3) = (t * t, t * t * t);
            let mut p = [0.0; 3];
            for (c, out) in p.iter_mut().enumerate() {
                *out = 0.5
                    * (2.0 * p1[c]
                        + (-p0[c] + p2[c]) * t
                        + (2.0 * p0[c] - 5.0 * p1[c] + 4.0 * p2[c] - p3[c]) * t2
                        + (-p0[c] + 3.0 * p1[c] - 3.0 * p2[c] + p3[c]) * t3);
            }
            samples.push(p);
        }
    }
    // Radii by length along the stroke.
    let mut along = vec![0.0f32];
    for w in samples.windows(2) {
        let l = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2) + (w[1][2] - w[0][2]).powi(2)).sqrt();
        along.push(along.last().unwrap() + l);
    }
    let total = along.last().copied().unwrap_or(0.0).max(1e-6);
    let r = |i: usize| r0 + (r1 - r0) * along[i] / total;
    if samples.len() == 1 {
        return vec![Seg { a: samples[0], ra: r0, b: samples[0], rb: r1 }];
    }
    (0..samples.len() - 1).map(|i| Seg { a: samples[i], ra: r(i), b: samples[i + 1], rb: r(i + 1) }).collect()
}

/// Whether `(x, y)` is inside a polygon (non-zero winding).
fn inside(poly: &[[f32; 2]], x: f32, y: f32) -> bool {
    let mut wind = 0i32;
    for i in 0..poly.len() {
        let ([ax, ay], [bx, by]) = (poly[i], poly[(i + 1) % poly.len()]);
        let side = (bx - ax) * (y - ay) - (x - ax) * (by - ay);
        if ay <= y {
            if by > y && side > 0.0 {
                wind += 1;
            }
        } else if by <= y && side < 0.0 {
            wind -= 1;
        }
    }
    wind != 0
}

/// How far `(x, y)` is from a polygon's edge, and the way out from it.
fn edge(poly: &[[f32; 2]], x: f32, y: f32) -> (f32, [f32; 2]) {
    let mut best = (f32::INFINITY, [0.0, 0.0]);
    for i in 0..poly.len() {
        let ([ax, ay], [bx, by]) = (poly[i], poly[(i + 1) % poly.len()]);
        let (dx, dy) = (bx - ax, by - ay);
        let len2 = dx * dx + dy * dy;
        let t = if len2 > 0.0 { (((x - ax) * dx + (y - ay) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
        let (cx, cy) = (ax + t * dx - x, ay + t * dy - y);
        let d = (cx * cx + cy * cy).sqrt();
        if d < best.0 {
            let out = if d > 1e-6 {
                [cx / d, cy / d]
            } else {
                let l = len2.sqrt().max(1e-6);
                [dy / l, -dx / l]
            };
            best = (d, out);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::Recipe;

    fn solid(json: &str) -> Solid {
        let text = format!(
            r#"{{ "kiln": 0, "name": "t", "canvas": [16, 16], "pivot": [8, 16],
                "materials": {{ "m": {{ "ramp": "sxSuUvVW", "finish": "matte" }} }}, "parts": [ {json} ] }}"#
        );
        let r = Recipe::parse(&text).unwrap();
        Solid::of(&r.src.parts[0], Xf::ID)
    }

    #[test]
    fn an_ellipsoid_is_nearest_at_its_middle_and_faces_out() {
        let s = solid(r#"{ "id": "a", "shape": "ellipsoid", "at": [8, 8, 1], "r": [4, 2, 3], "mat": "m" }"#);
        let mid = s.hit(8.0, 8.0).unwrap();
        assert!((mid.z - 4.0).abs() < 1e-5 && (mid.n[2] - 1.0).abs() < 1e-5);
        let right = s.hit(11.9, 8.0).unwrap();
        assert!(right.n[0] > 0.8 && right.z < mid.z);
        assert!(s.hit(8.0, 10.5).is_none());
        assert_eq!(s.bounds(), [4.0, 6.0, 12.0, 10.0]);
    }

    #[test]
    fn a_capsule_tapers_and_a_tilted_one_stands_proud() {
        let s = solid(r#"{ "id": "a", "shape": "capsule", "from": [4, 8, 0], "to": [12, 8, 0], "r": [2, 1], "mat": "m" }"#);
        let a = s.hit(4.0, 8.0).unwrap();
        let b = s.hit(12.0, 8.0).unwrap();
        assert!((a.z - 2.0).abs() < 1e-4, "{a:?}");
        assert!((b.z - 1.0).abs() < 0.02, "{b:?}");
        assert!(s.hit(12.0, 9.5).is_none() && s.hit(4.0, 9.5).is_some());
        // Tilted toward the viewer, the tube is r / cos(tilt) above its axis.
        let t = solid(r#"{ "id": "a", "shape": "capsule", "from": [4, 8, 0], "to": [12, 8, 4], "r": [1], "mat": "m" }"#);
        let mid = t.hit(8.0, 8.0).unwrap();
        assert!((mid.z - (2.0 + 5f32.sqrt() / 2.0)).abs() < 1e-3, "{mid:?}");
    }

    #[test]
    fn a_turned_box_and_a_bevelled_slab() {
        let b = solid(r#"{ "id": "a", "shape": "box", "at": [8, 8, 0], "size": [6, 2, 2], "round": 0.5, "turn": 90, "mat": "m" }"#);
        assert!(b.hit(8.0, 10.5).is_some() && b.hit(10.5, 8.0).is_none());
        assert!((b.hit(8.0, 8.0).unwrap().z - 1.0).abs() < 1e-5);
        let s = solid(r#"{ "id": "a", "shape": "slab", "points": [[2, 2], [10, 2], [10, 10], [2, 10]], "z": 1, "thick": 1, "bevel": 0.5, "mat": "m" }"#);
        let mid = s.hit(6.0, 6.0).unwrap();
        assert!((mid.z - 1.5).abs() < 1e-5 && mid.n == [0.0, 0.0, 1.0]);
        let rim = s.hit(9.9, 6.0).unwrap();
        assert!(rim.n[0] > 0.5 && rim.z < mid.z);
        assert!(s.hit(10.5, 6.0).is_none());
    }

    #[test]
    fn a_stroke_runs_through_its_points() {
        let s = solid(r#"{ "id": "a", "shape": "stroke", "points": [[2, 8, 0], [8, 4, 0], [14, 8, 0]], "width": [2, 0.5], "mat": "m" }"#);
        for (x, y) in [(2.0, 8.0), (8.0, 4.0), (14.0, 8.0)] {
            assert!(s.hit(x, y).is_some(), "{x},{y}");
        }
        assert!(s.hit(8.0, 8.0).is_none());
        assert!(s.hit(2.0, 8.9).is_some() && s.hit(14.0, 8.4).is_none());
    }

    #[test]
    fn posing_moves_the_shape_exactly() {
        let mut s = solid(r#"{ "id": "a", "shape": "capsule", "from": [8, 8, 0], "to": [8, 2, 0], "r": [1], "mat": "m" }"#);
        s.xf = Xf::turn_about(90.0, [8.0, 8.0]);
        // Up turned clockwise is right.
        assert!(s.hit(13.0, 8.0).is_some() && s.hit(8.0, 3.0).is_none());
        let n = s.hit(13.0, 8.9).unwrap().n;
        assert!(n[1] > 0.5, "{n:?}");
    }
}
