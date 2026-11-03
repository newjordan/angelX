//! The sampler's 3D and its motion: small vector maths, meshes lit on the
//! CPU and written as glass scene envelopes, and the shapes the motion
//! scenes move.
//!
//! The window lights a scene node with one fixed key. These meshes carry
//! their light in their vertex colours instead (a key, a fill, a specular
//! and a fresnel rim), so a surface can be glossy or glassy and a planet is
//! lit from its own sun. A translucent node is drawn without writing depth,
//! so its triangles are sorted farthest first here. An opaque node writes
//! depth, so what is translucent lies over it right, pixel by pixel.

use std::f32::consts::{PI, TAU};
use std::fmt::Write as _;
use std::ops::{Add, Mul, Neg, Sub};

/// Inline vertices across one envelope (`glass_scene`'s cap).
pub const MAX_VERTICES: usize = 4096;
/// Inline indices across one envelope.
pub const MAX_INDICES: usize = 12288;

const GOLDEN: f32 = 2.399_963;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub const fn v3(x: f32, y: f32, z: f32) -> V3 {
    V3 { x, y, z }
}

impl Add for V3 {
    type Output = V3;
    fn add(self, o: V3) -> V3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl Sub for V3 {
    type Output = V3;
    fn sub(self, o: V3) -> V3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Mul<f32> for V3 {
    type Output = V3;
    fn mul(self, k: f32) -> V3 {
        v3(self.x * k, self.y * k, self.z * k)
    }
}

impl Neg for V3 {
    type Output = V3;
    fn neg(self) -> V3 {
        v3(-self.x, -self.y, -self.z)
    }
}

impl V3 {
    pub fn dot(self, o: V3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: V3) -> V3 {
        v3(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    pub fn len(self) -> f32 {
        self.dot(self).sqrt()
    }

    /// Unit length. A zero vector comes back as +Y, never NaN.
    pub fn norm(self) -> V3 {
        let len = self.len();
        if len > 1e-8 {
            self * (1.0 / len)
        } else {
            v3(0.0, 1.0, 0.0)
        }
    }

    pub fn rot_x(self, a: f32) -> V3 {
        let (s, c) = a.sin_cos();
        v3(self.x, c * self.y - s * self.z, s * self.y + c * self.z)
    }

    pub fn rot_y(self, a: f32) -> V3 {
        let (s, c) = a.sin_cos();
        v3(c * self.x + s * self.z, self.y, -s * self.x + c * self.z)
    }

    pub fn rot_z(self, a: f32) -> V3 {
        let (s, c) = a.sin_cos();
        v3(c * self.x - s * self.y, s * self.x + c * self.y, self.z)
    }
}

pub type Rgb = [f32; 3];
pub type Rgba = [f32; 4];

pub const WHITE: Rgb = [1.0, 1.0, 1.0];

pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

pub fn scale(c: Rgb, k: f32) -> Rgb {
    [c[0] * k, c[1] * k, c[2] * k]
}

pub fn add(a: Rgb, b: Rgb) -> Rgb {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn rgba(c: Rgb, a: f32) -> Rgba {
    [c[0], c[1], c[2], a]
}

pub fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The sampler's neon ramp, cyclic in `t`: hot pink, violet, cyan, mint,
/// amber, and round again.
pub fn neon(t: f32) -> Rgb {
    const STOPS: [Rgb; 5] = [
        [1.00, 0.20, 0.60],
        [0.56, 0.28, 1.00],
        [0.16, 0.82, 1.00],
        [0.36, 1.00, 0.70],
        [1.00, 0.70, 0.22],
    ];
    let t = t.rem_euclid(1.0) * STOPS.len() as f32;
    let i = (t.floor() as usize) % STOPS.len();
    let f = t - t.floor();
    let f = f * f * (3.0 - 2.0 * f);
    mix(STOPS[i], STOPS[(i + 1) % STOPS.len()], f)
}

/// A colour as the dialect's `rrggbb`.
pub fn hex(c: Rgb) -> String {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("{:02x}{:02x}{:02x}", byte(c[0]), byte(c[1]), byte(c[2]))
}

/// A small integer hash in 0..1, for stars and grain that stay put.
pub fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    (x & 0x00FF_FFFF) as f32 / 16_777_216.0
}

/// The camera an envelope carries, so the window's projection and the
/// CPU's shading agree on where the eye is.
#[derive(Clone, Copy, Debug)]
pub struct Cam {
    pub eye: V3,
    pub target: V3,
    pub fov: f32,
}

impl Cam {
    pub fn forward(&self) -> V3 {
        (self.target - self.eye).norm()
    }

    pub fn right(&self) -> V3 {
        self.forward().cross(v3(0.0, 1.0, 0.0)).norm()
    }

    pub fn up(&self) -> V3 {
        self.right().cross(self.forward())
    }

    pub fn depth(&self, p: V3) -> f32 {
        (p - self.eye).dot(self.forward())
    }
}

/// Triangles with their colour baked per vertex.
#[derive(Default)]
pub struct Mesh {
    pos: Vec<V3>,
    col: Vec<Rgba>,
    tris: Vec<[u32; 3]>,
    /// Pulled toward the eye when sorting, so a line's core is drawn after
    /// its halo at the same depth.
    bias: Vec<f32>,
}

impl Mesh {
    pub fn vertices(&self) -> usize {
        self.pos.len()
    }

    pub fn indices(&self) -> usize {
        self.tris.len() * 3
    }

    pub fn vertex(&mut self, p: V3, c: Rgba) -> u32 {
        self.pos.push(p);
        self.col.push(c);
        (self.pos.len() - 1) as u32
    }

    /// A triangle wound so its face looks along `toward`: the window culls
    /// back faces, counter-clockwise in front.
    pub fn tri(&mut self, a: u32, b: u32, c: u32, toward: V3, bias: f32) {
        let (pa, pb, pc) = (self.pos[a as usize], self.pos[b as usize], self.pos[c as usize]);
        let n = (pb - pa).cross(pc - pa);
        if n.dot(toward) >= 0.0 {
            self.tris.push([a, b, c]);
        } else {
            self.tris.push([a, c, b]);
        }
        self.bias.push(bias);
    }

    /// `a b c d` in order round a quad.
    pub fn quad(&mut self, q: [u32; 4], toward: V3, bias: f32) {
        self.tri(q[0], q[1], q[2], toward, bias);
        self.tri(q[0], q[2], q[3], toward, bias);
    }

    /// Farthest first, for a node the window blends.
    pub fn sort(&mut self, cam: &Cam) {
        let mut order: Vec<(f32, [u32; 3], f32)> = self
            .tris
            .iter()
            .zip(&self.bias)
            .map(|(t, b)| {
                let c = (self.pos[t[0] as usize] + self.pos[t[1] as usize] + self.pos[t[2] as usize])
                    * (1.0 / 3.0);
                (cam.depth(c) - b, *t, *b)
            })
            .collect();
        order.sort_by(|a, b| b.0.total_cmp(&a.0));
        self.tris = order.iter().map(|o| o.1).collect();
        self.bias = order.iter().map(|o| o.2).collect();
    }

    /// A line through `points` (place, colour, width), turned to face the
    /// eye all along. Alpha in a point's colour fades the line there.
    pub fn ribbon(&mut self, cam: &Cam, points: &[(V3, Rgba, f32)], bias: f32) {
        if points.len() < 2 {
            return;
        }
        let base = self.pos.len() as u32;
        let last = points.len() - 1;
        for i in 0..points.len() {
            let (p, c, w) = points[i];
            let along = (points[(i + 1).min(last)].0 - points[i.saturating_sub(1)].0).norm();
            let side = along.cross((cam.eye - p).norm()).norm() * (w * 0.5);
            self.vertex(p + side, c);
            self.vertex(p - side, c);
        }
        for i in 0..last as u32 {
            let a0 = base + 2 * i;
            let toward = cam.eye - points[i as usize].0;
            self.tri(a0, a0 + 2, a0 + 1, toward, bias);
            self.tri(a0 + 1, a0 + 2, a0 + 3, toward, bias);
        }
    }

    /// A line lying in the plane whose normal is `up`, seen from that side.
    pub fn strip(&mut self, points: &[(V3, Rgba, f32)], up: V3, bias: f32) {
        if points.len() < 2 {
            return;
        }
        let base = self.pos.len() as u32;
        let last = points.len() - 1;
        for i in 0..points.len() {
            let (p, c, w) = points[i];
            let along = (points[(i + 1).min(last)].0 - points[i.saturating_sub(1)].0).norm();
            let side = along.cross(up).norm() * (w * 0.5);
            self.vertex(p + side, c);
            self.vertex(p - side, c);
        }
        for i in 0..last as u32 {
            let a0 = base + 2 * i;
            self.tri(a0, a0 + 2, a0 + 1, up, bias);
            self.tri(a0 + 1, a0 + 2, a0 + 3, up, bias);
        }
    }

    /// A soft round light facing the eye: bright in the middle, gone at
    /// `r`. `segments` is how round it is.
    #[allow(clippy::too_many_arguments)]
    pub fn glow(&mut self, cam: &Cam, c: V3, r: f32, color: Rgb, alpha: f32, segments: u32, bias: f32) {
        let segments = segments.max(3);
        let (right, up) = (cam.right(), cam.up());
        let toward = cam.eye - c;
        let centre = self.vertex(c, rgba(color, alpha));
        let first = self.pos.len() as u32;
        for k in 0..segments {
            let a = k as f32 / segments as f32 * TAU;
            let d = right * a.cos() + up * a.sin();
            self.vertex(c + d * (r * 0.38), rgba(color, alpha * 0.42));
            self.vertex(c + d * r, rgba(color, 0.0));
        }
        for k in 0..segments {
            let inner = first + 2 * k;
            let outer = inner + 1;
            let next = first + 2 * ((k + 1) % segments);
            self.tri(centre, inner, next, toward, bias);
            self.tri(inner, outer, next + 1, toward, bias);
            self.tri(inner, next + 1, next, toward, bias);
        }
    }

    /// A small star: a centre and a fading rim of four.
    pub fn star(&mut self, cam: &Cam, c: V3, r: f32, color: Rgb, alpha: f32) {
        let (right, up) = (cam.right(), cam.up());
        let toward = cam.eye - c;
        let centre = self.vertex(c, rgba(color, alpha));
        let first = self.pos.len() as u32;
        for k in 0..4 {
            let a = k as f32 / 4.0 * TAU;
            self.vertex(c + (right * a.cos() + up * a.sin()) * r, rgba(color, 0.0));
        }
        for k in 0..4 {
            self.tri(centre, first + k, first + (k + 1) % 4, toward, 0.0);
        }
    }

    /// A sphere wound outward, each vertex coloured by `shade(point,
    /// normal)`.
    pub fn sphere(&mut self, c: V3, r: f32, segments: u32, rings: u32, shade: impl Fn(V3, V3) -> Rgba) {
        let base = self.pos.len() as u32;
        for j in 0..=rings {
            let lat = PI * j as f32 / rings as f32 - PI / 2.0;
            for i in 0..=segments {
                let lon = TAU * i as f32 / segments as f32;
                let n = v3(lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin());
                let p = c + n * r;
                let color = shade(p, n);
                self.vertex(p, color);
            }
        }
        let row = segments + 1;
        for j in 0..rings {
            for i in 0..segments {
                let a = base + j * row + i;
                let q = [a, a + 1, a + row + 1, a + row];
                let mid = (self.pos[q[0] as usize] + self.pos[q[2] as usize]) * 0.5;
                self.quad(q, mid - c, 0.0);
            }
        }
    }

    /// A tube round the closed `curve`, `sides` round, wound outward. Its
    /// frame is carried along the curve and the twist it gathers is spread
    /// back over the loop, so no seam shows. `shade` takes the point, its
    /// normal and how far round the curve it is, 0..1.
    pub fn tube(&mut self, curve: &[V3], radius: f32, sides: u32, shade: impl Fn(V3, V3, f32) -> Rgba) {
        let n = curve.len();
        if n < 3 {
            return;
        }
        let tangent = |i: usize| (curve[(i + 1) % n] - curve[(i + n - 1) % n]).norm();
        let t0 = tangent(0);
        let seed = if t0.y.abs() < 0.9 { v3(0.0, 1.0, 0.0) } else { v3(1.0, 0.0, 0.0) };
        let mut normals = Vec::with_capacity(n);
        let mut normal = (seed - t0 * seed.dot(t0)).norm();
        for i in 0..n {
            let t = tangent(i);
            normal = (normal - t * normal.dot(t)).norm();
            normals.push(normal);
        }
        // The frame carried once round, back at the start, against where it
        // began: that angle is spread over the loop.
        let back = (normal - t0 * normal.dot(t0)).norm();
        let first = normals[0];
        let twist = t0.cross(first).dot(back).atan2(first.dot(back));
        let base = self.pos.len() as u32;
        for i in 0..n {
            let t = tangent(i);
            let b0 = t.cross(normals[i]);
            let undo = -twist * i as f32 / n as f32;
            let (s, c) = undo.sin_cos();
            let nn = normals[i] * c + b0 * s;
            let bb = t.cross(nn);
            let u = i as f32 / n as f32;
            for k in 0..sides {
                let a = k as f32 / sides as f32 * TAU;
                let dir = nn * a.cos() + bb * a.sin();
                let p = curve[i] + dir * radius;
                let color = shade(p, dir, u);
                self.vertex(p, color);
            }
        }
        for i in 0..n as u32 {
            let j = (i + 1) % n as u32;
            for k in 0..sides {
                let l = (k + 1) % sides;
                let q = [base + i * sides + k, base + j * sides + k, base + j * sides + l, base + i * sides + l];
                let mid = (self.pos[q[0] as usize] + self.pos[q[2] as usize]) * 0.5;
                let centre = (curve[i as usize] + curve[j as usize]) * 0.5;
                self.quad(q, mid - centre, 0.0);
            }
        }
    }
}

/// A scene envelope, written by hand: numbers kept short, every one finite.
pub struct Envelope {
    json: String,
    nodes: usize,
    vertices: usize,
    indices: usize,
}

impl Envelope {
    pub fn new(cam: &Cam) -> Self {
        let mut json = String::with_capacity(1 << 17);
        json.push_str(r#"{"v":1,"scene":{"camera":{"eye":"#);
        vec_json(&mut json, cam.eye);
        json.push_str(r#","target":"#);
        vec_json(&mut json, cam.target);
        json.push_str(r#","fov":"#);
        num(&mut json, cam.fov);
        json.push_str(r#","near":0.05,"far":80},"nodes":["#);
        Self { json, nodes: 0, vertices: 0, indices: 0 }
    }

    /// One node holding `mesh` with its colours as baked. A tint alpha just
    /// under one is what makes the window blend a node. A mesh that would
    /// take the envelope past the window's caps is left out, so the rest
    /// still draws rather than the whole scene being refused.
    pub fn mesh(&mut self, id: u32, mesh: &Mesh, blend: bool) {
        if mesh.tris.is_empty()
            || self.vertices + mesh.vertices() > MAX_VERTICES
            || self.indices + mesh.indices() > MAX_INDICES
        {
            return;
        }
        self.vertices += mesh.vertices();
        self.indices += mesh.indices();
        if self.nodes > 0 {
            self.json.push(',');
        }
        self.nodes += 1;
        let alpha = if blend { "0.999" } else { "1" };
        let _ = write!(
            self.json,
            r#"{{"id":{id},"color":[1,1,1,{alpha}],"mesh":{{"kind":"inline","positions":["#
        );
        for (i, p) in mesh.pos.iter().enumerate() {
            if i > 0 {
                self.json.push(',');
            }
            vec_json(&mut self.json, *p);
        }
        self.json.push_str(r#"],"colors":["#);
        for (i, c) in mesh.col.iter().enumerate() {
            if i > 0 {
                self.json.push(',');
            }
            self.json.push('[');
            for (k, v) in c.iter().enumerate() {
                if k > 0 {
                    self.json.push(',');
                }
                num(&mut self.json, v.clamp(0.0, 1.0));
            }
            self.json.push(']');
        }
        self.json.push_str(r#"],"indices":["#);
        for (i, t) in mesh.tris.iter().enumerate() {
            if i > 0 {
                self.json.push(',');
            }
            let _ = write!(self.json, "{},{},{}", t[0], t[1], t[2]);
        }
        self.json.push_str("]}}");
    }

    pub fn finish(mut self) -> Vec<u8> {
        self.json.push_str("]}}");
        self.json.into_bytes()
    }
}

fn num(out: &mut String, v: f32) {
    let v = if v.is_finite() { (v * 1000.0).round() / 1000.0 } else { 0.0 };
    if v == 0.0 {
        out.push('0');
    } else {
        let _ = write!(out, "{v}");
    }
}

fn vec_json(out: &mut String, p: V3) {
    out.push('[');
    num(out, p.x);
    out.push(',');
    num(out, p.y);
    out.push(',');
    num(out, p.z);
    out.push(']');
}

fn turn(a: f32, b: f32, angle: f32) -> (f32, f32) {
    let (s, c) = angle.sin_cos();
    (a * c - b * s, a * s + b * c)
}

/// A far shell of stars round the origin, fixed by `seed`.
fn starfield(mesh: &mut Mesh, cam: &Cam, count: u32, radius: f32, seed: u32) {
    for i in 0..count {
        let y = hash(seed + i * 3) * 2.0 - 1.0;
        let a = hash(seed + i * 3 + 1) * TAU;
        let r = (1.0 - y * y).sqrt();
        let p = v3(r * a.cos(), y, r * a.sin()) * radius;
        let tint = mix([0.7, 0.8, 1.0], [1.0, 0.85, 0.7], hash(seed + i * 3 + 2));
        let size = radius * (0.004 + 0.006 * hash(seed + i * 7));
        mesh.star(cam, p, size, tint, 0.45 + 0.5 * hash(seed + i * 5));
    }
}

// -------------------------------------------------------------- tesseract --

/// A hypercube turning through the fourth dimension, its 32 edges neon
/// lines (a wide faint halo, a glow and a hot core), its 16 corners soft
/// lights and its 24 faces faint glass, under a far shell of stars. Nearer
/// in the fourth dimension is bigger and pinker.
pub fn tesseract(t: f32) -> Vec<u8> {
    let cam = Cam { eye: v3(0.0, 0.7, 6.2), target: v3(0.0, 0.0, 0.0), fov: 0.72 };
    let mut corners = [(V3::default(), 0.0f32, 0.0f32); 16];
    for (i, corner) in corners.iter_mut().enumerate() {
        let s = |bit: usize| if (i >> bit) & 1 == 1 { 1.0 } else { -1.0 };
        let (mut x, mut y, mut z, mut w) = (s(0), s(1), s(2), s(3));
        (x, w) = turn(x, w, t * 0.52);
        (y, z) = turn(y, z, t * 0.21);
        (z, w) = turn(z, w, t * 0.33 + 0.4);
        let k = 1.0 / (3.4 - w);
        let p = v3(x, y, z) * (k * 3.4);
        *corner = (p.rot_y(t * 0.17).rot_x(0.30), w, k);
    }
    let inner: Rgb = [0.18, 0.80, 1.00];
    let outer: Rgb = [1.00, 0.22, 0.70];
    let tone = |w: f32| mix(inner, outer, smooth(-1.6, 1.6, w));
    let mut mesh = Mesh::default();
    starfield(&mut mesh, &cam, 90, 24.0, 17);
    let pairs = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
    for (f, (a, b)) in pairs.iter().enumerate() {
        let others: Vec<usize> = (0..4).filter(|bit| bit != a && bit != b).collect();
        for rest in 0..4usize {
            let base = ((rest & 1) << others[0]) | (((rest >> 1) & 1) << others[1]);
            let q = [base, base | 1 << a, base | 1 << a | 1 << b, base | 1 << b];
            let w = q.iter().map(|i| corners[*i].1).sum::<f32>() / 4.0;
            let c = mix(tone(w), neon(f as f32 / 6.0), 0.35);
            let alpha = 0.035 + 0.05 * smooth(-1.5, 1.5, w);
            let ids = q.map(|i| mesh.vertex(corners[i].0, rgba(c, alpha)));
            let mid = (corners[q[0]].0 + corners[q[2]].0) * 0.5;
            mesh.quad(ids, cam.eye - mid, -0.02);
        }
    }
    for i in 0..16usize {
        for bit in 0..4 {
            let j = i ^ (1 << bit);
            if j < i {
                continue;
            }
            let (pa, wa, ka) = corners[i];
            let (pb, wb, kb) = corners[j];
            let (ca, cb) = (tone(wa), tone(wb));
            for (width, alpha, white, bias) in
                [(0.30, 0.07, 0.0, 0.0), (0.10, 0.30, 0.15, 0.01), (0.032, 0.95, 0.55, 0.02)]
            {
                mesh.ribbon(
                    &cam,
                    &[
                        (pa, rgba(mix(ca, WHITE, white), alpha), width * ka * 3.0),
                        (pb, rgba(mix(cb, WHITE, white), alpha), width * kb * 3.0),
                    ],
                    bias,
                );
            }
        }
    }
    for (p, w, k) in corners {
        let c = tone(w);
        mesh.glow(&cam, p, 0.42 * k * 3.0, c, 0.55, 10, 0.03);
        mesh.glow(&cam, p, 0.12 * k * 3.0, mix(c, WHITE, 0.7), 1.0, 6, 0.04);
    }
    mesh.sort(&cam);
    let mut env = Envelope::new(&cam);
    env.mesh(1, &mesh, true);
    env.finish()
}

/// How far each of the tesseract's turns has gone at `t`, for a readout.
pub fn tesseract_angles(t: f32) -> [f32; 3] {
    [(t * 0.52) % TAU, (t * 0.21) % TAU, (t * 0.33 + 0.4) % TAU]
}

// ------------------------------------------------------------------- knot --

const KNOT_STEPS: usize = 112;

fn torus_knot(u: f32) -> V3 {
    let phi = u * TAU;
    let r = (3.0 * phi).cos() + 2.25;
    v3(r * (2.0 * phi).cos(), -(3.0 * phi).sin() * 1.15, r * (2.0 * phi).sin()) * 0.64
}

/// Glass: see-through face on, opaque and bright at the grazing edge, a
/// hot specular, and a hue that slides with the angle (thin-film colour).
fn glass_shade(p: V3, n: V3, u: f32, eye: V3, t: f32) -> Rgba {
    let view = (eye - p).norm();
    let facing = n.dot(view).max(0.0);
    let fresnel = (1.0 - facing).powf(2.6);
    let key = v3(-0.5, 0.8, 0.6).norm();
    let fill = v3(0.8, -0.25, 0.5).norm();
    let spec = n.dot((key + view).norm()).max(0.0).powf(80.0);
    let sheen = n.dot((fill + view).norm()).max(0.0).powf(24.0) * 0.35;
    let diffuse = n.dot(key).max(0.0);
    let hue = neon(u * 2.0 + fresnel * 0.45 + t * 0.05);
    let rgb = add(
        add(scale(hue, 0.10 + 0.30 * diffuse), scale(hue, fresnel * 1.1)),
        scale(WHITE, spec * 1.4 + sheen),
    );
    let alpha = 0.10 + 0.70 * fresnel + 0.9 * spec + sheen;
    rgba(rgb.map(|v| v.clamp(0.0, 1.0)), alpha.clamp(0.0, 1.0))
}

/// A trefoil of glass tubing with a hot neon filament inside, two orbit
/// rings with comets, and sparks running along the glass. The filament is
/// opaque, so the glass in front of it blends over it and the glass behind
/// it is hidden, pixel by pixel.
pub fn knot(t: f32) -> Vec<u8> {
    let cam = Cam { eye: v3(0.0, 0.4, 6.6), target: v3(0.0, 0.0, 0.0), fov: 0.70 };
    let turn = |p: V3| p.rot_y(t * 0.42).rot_x(0.55 + 0.18 * (t * 0.37).sin());
    let curve: Vec<V3> = (0..KNOT_STEPS).map(|i| turn(torus_knot(i as f32 / KNOT_STEPS as f32))).collect();
    let mut filament = Mesh::default();
    filament.tube(&curve, 0.085, 4, |_, _, u| {
        let pulse = 0.5 + 0.5 * ((u * 6.0 - t * 1.6) * TAU).sin();
        rgba(mix(neon(u * 2.0 + t * 0.10), WHITE, 0.25 + 0.35 * pulse), 1.0)
    });
    let mut glass = Mesh::default();
    glass.tube(&curve, 0.33, 9, |p, n, u| glass_shade(p, n, u, cam.eye, t));
    for (ring, (radius, tilt, speed)) in [(2.55f32, 0.95f32, 0.25f32), (2.85, -0.7, -0.18)].into_iter().enumerate() {
        let steps = 64;
        let head = (t * speed).rem_euclid(1.0);
        let points: Vec<(V3, Rgba, f32)> = (0..=steps)
            .map(|k| {
                let s = k as f32 / steps as f32;
                let a = s * TAU;
                let p = v3(a.cos() * radius, 0.0, a.sin() * radius).rot_x(tilt).rot_z(0.3 * ring as f32);
                let behind = if speed > 0.0 { (head - s).rem_euclid(1.0) } else { (s - head).rem_euclid(1.0) };
                let tail = (-behind * 9.0).exp();
                let c = mix(neon(0.35 + 0.3 * ring as f32), WHITE, tail * 0.6);
                (p, rgba(c, 0.10 + 0.85 * tail), 0.035 + 0.05 * tail)
            })
            .collect();
        glass.ribbon(&cam, &points, 0.0);
    }
    for k in 0..20u32 {
        let u = (k as f32 / 20.0 + t * 0.06).rem_euclid(1.0);
        let i = ((u * KNOT_STEPS as f32) as usize).min(KNOT_STEPS - 1);
        let out = (curve[i] - cam.target).norm();
        let twinkle = 0.5 + 0.5 * (t * 5.0 + k as f32 * 1.7).sin();
        glass.star(&cam, curve[i] + out * 0.36, 0.10 + 0.08 * twinkle, mix(neon(u * 2.0), WHITE, 0.6), 0.5 + 0.5 * twinkle);
    }
    glass.sort(&cam);
    let mut env = Envelope::new(&cam);
    env.mesh(1, &filament, false);
    env.mesh(2, &glass, true);
    env.finish()
}

// ------------------------------------------------------------------- city --

const CITY: usize = 12;

/// The city's column heights at `t`: a ripple out from the middle and a
/// cross wave over it.
fn city_height(i: usize, j: usize, t: f32) -> f32 {
    let x = i as f32 - (CITY as f32 - 1.0) / 2.0;
    let z = j as f32 - (CITY as f32 - 1.0) / 2.0;
    let d = x.hypot(z);
    let ripple = (0.5 + 0.5 * (d * 0.95 - t * 2.6).sin()).powf(1.6) * (1.0 - d / 11.0).max(0.25);
    let cross = (0.5 + 0.5 * (x * 0.7 + t * 1.7).sin()) * (0.5 + 0.5 * (z * 0.6 - t * 1.1).cos());
    0.18 + 2.1 * ripple + 0.55 * cross
}

/// An equaliser city: 144 columns riding a wave, lit from a key with their
/// tops shining, risen through a sheet of tinted glass that fades at its
/// edges, over a glowing floor grid, the camera circling.
pub fn city(t: f32) -> Vec<u8> {
    let angle = 0.65 + t * 0.16;
    let cam = Cam {
        eye: v3(angle.sin() * 15.5, 9.2 + 0.7 * (t * 0.3).sin(), angle.cos() * 15.5),
        target: v3(0.0, 0.4, 0.0),
        fov: 0.62,
    };
    let key = v3(-0.45, 0.8, 0.4).norm();
    let mut solid = Mesh::default();
    let mut tall: Vec<(f32, V3, Rgb)> = Vec::new();
    let half = 0.39;
    for i in 0..CITY {
        for j in 0..CITY {
            let h = city_height(i, j, t);
            let x = i as f32 - (CITY as f32 - 1.0) / 2.0;
            let z = j as f32 - (CITY as f32 - 1.0) / 2.0;
            let level = (h / 2.9).clamp(0.0, 1.0);
            let base = neon(0.55 + level * 0.6 + t * 0.02);
            let rim = mix(base, WHITE, 0.25 + 0.5 * level.powf(2.0));
            let face = scale(base, 0.35 + 0.35 * level);
            let up = v3(0.0, 1.0, 0.0);
            // The top: a bright rim round a darker inset, so each column
            // reads as a lit tile.
            let inset = half * 0.62;
            let ring = |r: f32| {
                [v3(x - r, h, z - r), v3(x + r, h, z - r), v3(x + r, h, z + r), v3(x - r, h, z + r)]
            };
            let outer = ring(half).map(|p| solid.vertex(p, rgba(rim, 1.0)));
            let inner = ring(inset).map(|p| solid.vertex(p, rgba(face, 1.0)));
            solid.quad(inner, up, 0.0);
            for k in 0..4 {
                let l = (k + 1) % 4;
                solid.quad([outer[k], outer[l], inner[l], inner[k]], up, 0.0);
            }
            for (dx, dz) in [(1.0f32, 0.0f32), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                let n = v3(dx, 0.0, dz);
                let side = v3(dz, 0.0, -dx) * half;
                let c = v3(x, 0.0, z) + n * half;
                let lit = 0.16 + 0.50 * n.dot(key).max(0.0);
                let upper = scale(base, lit);
                let lower = scale(base, 0.03);
                let q = [
                    solid.vertex(c - side, rgba(lower, 1.0)),
                    solid.vertex(c + side, rgba(lower, 1.0)),
                    solid.vertex(c + side + up * h, rgba(upper, 1.0)),
                    solid.vertex(c - side + up * h, rgba(upper, 1.0)),
                ];
                solid.quad(q, n, 0.0);
            }
            tall.push((h, v3(x, h, z), rim));
        }
    }
    let mut glass = Mesh::default();
    let sheet = 1.15;
    let span = 8.0;
    let cells = 8usize;
    let mut ids = vec![0u32; (cells + 1) * (cells + 1)];
    for a in 0..=cells {
        for b in 0..=cells {
            let x = (a as f32 / cells as f32 - 0.5) * 2.0 * span;
            let z = (b as f32 / cells as f32 - 0.5) * 2.0 * span;
            let edge = 1.0 - smooth(0.45, 1.0, x.hypot(z) / span);
            let shimmer = 0.5 + 0.5 * (x * 0.6 + z * 0.4 + t * 1.3).sin();
            let c = mix([0.12, 0.55, 1.0], [0.6, 0.3, 1.0], shimmer);
            ids[a * (cells + 1) + b] = glass.vertex(v3(x, sheet, z), rgba(c, (0.20 + 0.16 * shimmer) * edge));
        }
    }
    for a in 0..cells {
        for b in 0..cells {
            let at = |a: usize, b: usize| ids[a * (cells + 1) + b];
            glass.quad([at(a, b), at(a + 1, b), at(a + 1, b + 1), at(a, b + 1)], v3(0.0, 1.0, 0.0), 0.0);
        }
    }
    let reach = 9.0;
    for line in 0..=CITY {
        let o = line as f32 - CITY as f32 / 2.0;
        let pulse = 0.5 + 0.5 * (o * 0.8 - t * 2.0).sin();
        for across in [false, true] {
            let points: Vec<(V3, Rgba, f32)> = (0..=6)
                .map(|k| {
                    let s = (k as f32 / 6.0 - 0.5) * 2.0 * reach;
                    let p = if across { v3(s, 0.01, o) } else { v3(o, 0.01, s) };
                    let fade = 1.0 - (s.abs() / reach).powf(2.0);
                    let c = mix([1.0, 0.25, 0.7], [0.3, 0.8, 1.0], if across { 0.8 } else { 0.2 });
                    (p, rgba(c, (0.25 + 0.5 * pulse) * fade), 0.05)
                })
                .collect();
            glass.strip(&points, v3(0.0, 1.0, 0.0), 0.0);
        }
    }
    tall.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (h, p, c) in tall.iter().take(8) {
        glass.glow(&cam, *p + v3(0.0, 0.2, 0.0), 0.8 + 0.4 * (h / 3.0), mix(*c, WHITE, 0.3), 0.7, 8, 0.0);
    }
    glass.sort(&cam);
    let mut env = Envelope::new(&cam);
    env.mesh(1, &solid, false);
    env.mesh(2, &glass, true);
    env.finish()
}

// ----------------------------------------------------------------- orrery --

struct Planet {
    orbit: f32,
    size: f32,
    speed: f32,
    phase: f32,
    tilt: f32,
    look: u8,
}

const PLANETS: [Planet; 6] = [
    Planet { orbit: 1.75, size: 0.15, speed: 1.55, phase: 0.4, tilt: 0.05, look: 0 },
    Planet { orbit: 2.55, size: 0.22, speed: 1.10, phase: 2.6, tilt: -0.04, look: 1 },
    Planet { orbit: 3.45, size: 0.26, speed: 0.82, phase: 4.4, tilt: 0.03, look: 2 },
    Planet { orbit: 4.40, size: 0.19, speed: 0.62, phase: 1.2, tilt: 0.06, look: 3 },
    Planet { orbit: 5.65, size: 0.50, speed: 0.40, phase: 5.5, tilt: -0.03, look: 4 },
    Planet { orbit: 7.10, size: 0.40, speed: 0.28, phase: 3.3, tilt: 0.04, look: 5 },
];

fn planet_at(planet: &Planet, t: f32) -> (V3, f32) {
    let a = planet.phase + t * planet.speed * 0.55;
    (v3(a.cos() * planet.orbit, 0.0, a.sin() * planet.orbit).rot_x(planet.tilt), a)
}

/// What a planet is made of at the normal `n`.
fn albedo(look: u8, n: V3) -> Rgb {
    let lat = n.y;
    match look {
        0 => mix([0.45, 0.45, 0.50], [0.70, 0.66, 0.62], 0.5 + 0.5 * (n.x * 9.0).sin() * (n.z * 7.0).cos()),
        1 => mix([0.85, 0.62, 0.35], [0.98, 0.85, 0.60], 0.5 + 0.5 * (lat * 6.0 + n.x * 2.0).sin()),
        2 => {
            let land = (n.x * 4.0).sin() * (n.y * 3.5 + 1.0).sin() * (n.z * 5.0 + 0.5).sin();
            if lat.abs() > 0.84 {
                [0.92, 0.95, 1.0]
            } else if land > 0.08 {
                mix([0.20, 0.55, 0.25], [0.55, 0.45, 0.28], (land * 3.0).clamp(0.0, 1.0))
            } else {
                [0.08, 0.30, 0.75]
            }
        }
        3 => mix([0.70, 0.25, 0.12], [0.88, 0.48, 0.28], 0.5 + 0.5 * (n.x * 6.0 + n.z * 5.0).sin()),
        4 => {
            let bands = (lat * 15.0 + (n.x * 3.0).sin() * 0.8).sin();
            mix([0.78, 0.50, 0.30], [0.97, 0.88, 0.72], 0.5 + 0.5 * bands)
        }
        _ => mix([0.82, 0.72, 0.48], [0.96, 0.90, 0.70], 0.5 + 0.5 * (lat * 11.0).sin()),
    }
}

/// A sun with its corona, six planets lit from it (day side, night side
/// and a rim of atmosphere), a moon, a ringed giant whose ring is
/// translucent, and orbits drawn as lines that brighten into a comet tail
/// behind each planet, under far stars. The camera drifts.
pub fn orrery(t: f32) -> Vec<u8> {
    let swing = 0.35 * (t * 0.11).sin();
    let cam = Cam {
        eye: v3(swing.sin() * 13.0, 5.0 + 0.5 * (t * 0.17).sin(), swing.cos() * 13.0),
        target: v3(0.0, -0.6, 0.0),
        fov: 0.66,
    };
    let sun = v3(0.0, 0.0, 0.0);
    let mut solid = Mesh::default();
    let mut glass = Mesh::default();
    solid.sphere(sun, 0.85, 20, 14, |p, n| {
        let facing = n.dot((cam.eye - p).norm()).max(0.0);
        let grain = 0.5 + 0.5 * (n.x * 13.0 + t * 0.8).sin() * (n.y * 11.0 - t * 0.6).sin();
        let c = mix([1.0, 0.42, 0.08], [1.0, 0.96, 0.75], facing.powf(0.55));
        rgba(mix(c, [1.0, 0.75, 0.3], 0.18 * grain), 1.0)
    });
    glass.glow(&cam, sun, 2.9, [1.0, 0.45, 0.12], 0.32, 24, 0.0);
    glass.glow(&cam, sun, 1.55, [1.0, 0.82, 0.45], 0.55, 24, 0.01);
    for (index, planet) in PLANETS.iter().enumerate() {
        let (p, angle) = planet_at(planet, t);
        let look = planet.look;
        let shade = |q: V3, n: V3| {
            let light = (sun - q).norm();
            let view = (cam.eye - q).norm();
            let day = n.dot(light).max(0.0).powf(0.8);
            let rim = (1.0 - n.dot(view).max(0.0)).powf(3.0) * (0.25 + day);
            let air: Rgb = if look == 2 { [0.35, 0.65, 1.0] } else { [1.0, 0.8, 0.6] };
            let c = add(scale(albedo(look, n), 0.04 + 0.96 * day), scale(air, rim * 0.55));
            rgba(c.map(|v| v.clamp(0.0, 1.0)), 1.0)
        };
        solid.sphere(p, planet.size, 14, 9, shade);
        if look == 2 {
            let m = t * 2.2;
            let moon = p + v3(m.cos() * 0.48, 0.08 * m.sin(), m.sin() * 0.48);
            solid.sphere(moon, 0.065, 8, 6, |q, n| {
                let day = n.dot((sun - q).norm()).max(0.0);
                rgba(scale([0.85, 0.85, 0.82], 0.05 + 0.95 * day), 1.0)
            });
        }
        if look == 5 {
            let up = v3(0.0, 1.0, 0.0).rot_x(0.42).rot_z(0.25);
            let a = up.cross(v3(0.0, 0.0, 1.0)).norm();
            let b = up.cross(a);
            let steps = 48u32;
            let radii = [planet.size * 1.35, planet.size * 1.75, planet.size * 2.05, planet.size * 2.45];
            let alphas = [0.0, 0.55, 0.35, 0.0];
            let base = glass.vertices() as u32;
            for k in 0..=steps {
                let s = k as f32 / steps as f32 * TAU;
                let dir = a * s.cos() + b * s.sin();
                for (r, alpha) in radii.iter().zip(alphas) {
                    let q = p + dir * *r;
                    let lit = 0.35 + 0.65 * (1.0 - (-dir.dot((sun - p).norm())).max(0.0) * 0.8);
                    glass.vertex(q, rgba(scale([0.95, 0.85, 0.62], lit), alpha));
                }
            }
            for k in 0..steps {
                for band in 0..3u32 {
                    let i = base + k * 4 + band;
                    glass.quad([i, i + 4, i + 5, i + 1], cam.eye - p, 0.0);
                }
            }
        }
        let steps = 72;
        let tint = neon(0.15 + index as f32 * 0.14);
        let points: Vec<(V3, Rgba, f32)> = (0..=steps)
            .map(|k| {
                let s = k as f32 / steps as f32 * TAU;
                let q = v3(s.cos() * planet.orbit, 0.0, s.sin() * planet.orbit).rot_x(planet.tilt);
                let behind = (angle - s).rem_euclid(TAU);
                let tail = (-behind * 1.1).exp();
                (q, rgba(mix(tint, WHITE, 0.3 * tail), 0.08 + 0.7 * tail), 0.022 + 0.03 * tail)
            })
            .collect();
        glass.ribbon(&cam, &points, 0.0);
    }
    starfield(&mut glass, &cam, 80, 30.0, 101);
    glass.sort(&cam);
    let mut env = Envelope::new(&cam);
    env.mesh(1, &solid, false);
    env.mesh(2, &glass, true);
    env.finish()
}

// ------------------------------------------------------------------ cloud --

/// How long the cloud holds a shape, and how long it takes to become the
/// next.
pub const CLOUD_HOLD: f32 = 1.5;
pub const CLOUD_MOVE: f32 = 1.0;
const CLOUD_SHAPES: usize = 5;

/// The cloud's shape names, in order.
pub const CLOUD_NAMES: [&str; CLOUD_SHAPES] = ["sphere", "torus", "trefoil", "double helix", "galaxy"];

fn shape_point(shape: usize, i: usize, n: usize) -> V3 {
    let u = (i as f32 + 0.5) / n as f32;
    let spin = i as f32 * GOLDEN;
    match shape {
        0 => {
            let y = 1.0 - 2.0 * u;
            let r = (1.0 - y * y).max(0.0).sqrt();
            v3(r * spin.cos(), y, r * spin.sin()) * 1.25
        }
        1 => {
            let a = u * TAU;
            let b = spin;
            v3((1.05 + 0.42 * b.cos()) * a.cos(), 0.42 * b.sin(), (1.05 + 0.42 * b.cos()) * a.sin())
        }
        2 => {
            let a = u * TAU;
            let c = v3((a).sin() + 2.0 * (2.0 * a).sin(), (a).cos() - 2.0 * (2.0 * a).cos(), -(3.0 * a).sin()) * 0.42;
            c + v3(spin.cos(), spin.sin(), (spin * 1.3).cos()) * 0.13
        }
        3 => {
            let strand = i % 4;
            let y = (u * 2.0 - 1.0) * 1.45;
            let a = u * TAU * 2.6;
            if strand < 3 {
                let a = a + if strand == 1 { PI } else { 0.0 };
                let r = if strand == 2 { 0.6 * (spin * 0.5).cos() } else { 0.62 };
                v3(a.cos() * r, y, a.sin() * r)
            } else {
                let f = hash(i as u32) * 2.0 - 1.0;
                v3(a.cos() * 0.6 * f, y, a.sin() * 0.6 * f)
            }
        }
        _ => {
            let arm = (i % 3) as f32;
            let r = u.sqrt() * 1.55;
            let a = arm * TAU / 3.0 + r * 3.3 + (hash(i as u32 * 7) - 0.5) * 0.5;
            let lift = (hash(i as u32 * 13) - 0.5) * 0.16 * (1.7 - r);
            v3(r * a.cos(), lift, r * a.sin())
        }
    }
}

/// Point `i` of `n` at `t` seconds. The cloud holds a sphere, a torus, a
/// trefoil, a double helix and a spiral galaxy in turn, easing and swirling
/// from one into the next, and the whole of it turns. The second value is
/// the point's place on the palette, the third which shape is held.
pub fn cloud(i: usize, n: usize, t: f32) -> (V3, f32, usize) {
    let span = CLOUD_HOLD + CLOUD_MOVE;
    let k = (t / span).floor().max(0.0) as usize;
    let f = t - k as f32 * span;
    let s = smooth(CLOUD_HOLD, span, f);
    let a = shape_point(k % CLOUD_SHAPES, i, n);
    let b = shape_point((k + 1) % CLOUD_SHAPES, i, n);
    let p = a * (1.0 - s) + b * s;
    let swirl = (s * PI).sin() * (0.8 + 0.6 * (i as f32 / n as f32));
    let p = p.rot_y(t * 0.45 + swirl).rot_x(0.38 + 0.12 * (t * 0.4).sin());
    let held = if s < 0.5 { k % CLOUD_SHAPES } else { (k + 1) % CLOUD_SHAPES };
    (p, i as f32 / n as f32, held)
}

/// Where the seven solids of the ring are at `t`: across (-1..1), up
/// (-1..1) and how near (-1 far .. 1 near).
pub fn ring(i: usize, t: f32) -> (f32, f32, f32) {
    let a = t * 0.55 + i as f32 * TAU / 7.0;
    (a.cos(), a.sin() * 0.55, a.sin())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Counted {
        nodes: usize,
        vertices: usize,
        indices: usize,
    }

    /// Reads an envelope back as the window would and checks what the
    /// wire refuses: unknown shape, a colour outside 0..1, an index past its
    /// mesh, the caps across the envelope.
    fn read(bytes: &[u8]) -> Counted {
        let value: serde_json::Value = serde_json::from_slice(bytes).expect("an envelope is JSON");
        assert_eq!(value["v"], 1);
        let nodes = value["scene"]["nodes"].as_array().expect("nodes");
        assert!(!nodes.is_empty() && nodes.len() <= 256);
        let mut counted = Counted { nodes: nodes.len(), vertices: 0, indices: 0 };
        for node in nodes {
            let tint = node["color"].as_array().unwrap();
            assert!(tint.iter().all(|v| (0.0..=1.0).contains(&v.as_f64().unwrap())));
            let mesh = &node["mesh"];
            assert_eq!(mesh["kind"], "inline");
            let positions = mesh["positions"].as_array().unwrap();
            let colors = mesh["colors"].as_array().unwrap();
            let indices = mesh["indices"].as_array().unwrap();
            assert_eq!(positions.len(), colors.len());
            assert_eq!(indices.len() % 3, 0);
            for p in positions {
                let p = p.as_array().unwrap();
                assert_eq!(p.len(), 3);
                assert!(p.iter().all(|v| v.as_f64().unwrap().is_finite()));
            }
            for c in colors {
                let c = c.as_array().unwrap();
                assert_eq!(c.len(), 4);
                assert!(c.iter().all(|v| (0.0..=1.0).contains(&v.as_f64().unwrap())));
            }
            for i in indices {
                assert!((i.as_u64().unwrap() as usize) < positions.len());
            }
            counted.vertices += positions.len();
            counted.indices += indices.len();
        }
        counted
    }

    #[test]
    fn every_scene_stays_under_the_envelope_caps_all_the_way_through() {
        // Every mesh made it in: the envelope leaves out one past the caps.
        let builders: [(&str, fn(f32) -> Vec<u8>, usize); 4] =
            [("tesseract", tesseract, 1), ("knot", knot, 2), ("city", city, 2), ("orrery", orrery, 2)];
        for (name, build, nodes) in builders {
            for step in 0..24 {
                let t = step as f32 * 0.77;
                let bytes = build(t);
                assert!(bytes.len() < 1 << 20, "{name} at {t}: {} bytes", bytes.len());
                let counted = read(&bytes);
                assert_eq!(counted.nodes, nodes, "{name} at {t} lost a mesh");
                assert!(counted.vertices <= MAX_VERTICES, "{name} at {t}: {} vertices", counted.vertices);
                assert!(counted.indices <= MAX_INDICES, "{name} at {t}: {} indices", counted.indices);
            }
        }
    }

    #[test]
    fn scenes_move() {
        for build in [tesseract, knot, city, orrery] {
            assert_ne!(build(0.0), build(0.5));
        }
    }

    #[test]
    fn a_triangle_is_wound_toward_what_it_faces() {
        let mut mesh = Mesh::default();
        let a = mesh.vertex(v3(0.0, 0.0, 0.0), [1.0; 4]);
        let b = mesh.vertex(v3(1.0, 0.0, 0.0), [1.0; 4]);
        let c = mesh.vertex(v3(0.0, 1.0, 0.0), [1.0; 4]);
        mesh.tri(a, b, c, v3(0.0, 0.0, 1.0), 0.0);
        mesh.tri(a, b, c, v3(0.0, 0.0, -1.0), 0.0);
        let normal = |t: [u32; 3]| {
            let p = |i: u32| mesh.pos[i as usize];
            (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]))
        };
        assert!(normal(mesh.tris[0]).z > 0.0);
        assert!(normal(mesh.tris[1]).z < 0.0);
    }

    #[test]
    fn sorting_draws_the_far_triangle_first() {
        let cam = Cam { eye: v3(0.0, 0.0, 5.0), target: v3(0.0, 0.0, 0.0), fov: 0.7 };
        let mut mesh = Mesh::default();
        for z in [1.0f32, -2.0] {
            let a = mesh.vertex(v3(0.0, 0.0, z), [1.0; 4]);
            let b = mesh.vertex(v3(1.0, 0.0, z), [1.0; 4]);
            let c = mesh.vertex(v3(0.0, 1.0, z), [1.0; 4]);
            mesh.tri(a, b, c, v3(0.0, 0.0, 1.0), 0.0);
        }
        mesh.sort(&cam);
        assert!(mesh.pos[mesh.tris[0][0] as usize].z < mesh.pos[mesh.tris[1][0] as usize].z);
    }

    #[test]
    fn the_tube_closes_without_a_twist() {
        let steps = 160;
        let curve: Vec<V3> = (0..steps).map(|i| torus_knot(i as f32 / steps as f32)).collect();
        let mut mesh = Mesh::default();
        let sides = 6;
        mesh.tube(&curve, 0.2, sides as u32, |_, n, _| rgba([n.x.abs(), n.y.abs(), n.z.abs()], 1.0));
        assert!(mesh.pos.iter().all(|p| p.x.is_finite() && p.y.is_finite() && p.z.is_finite()));
        // Across the seam (last ring to first) vertex k meets vertex k, as
        // it does between any two neighbouring rings: no turn is left over.
        let matched = |from: usize, to: usize, k: usize| {
            let p = mesh.pos[from * sides + k];
            (0..sides)
                .min_by(|a, b| {
                    let da = (mesh.pos[to * sides + a] - p).len();
                    let db = (mesh.pos[to * sides + b] - p).len();
                    da.total_cmp(&db)
                })
                .unwrap()
        };
        for k in 0..sides {
            assert_eq!(matched(10, 11, k), k, "between rings");
            assert_eq!(matched(steps - 1, 0, k), k, "across the seam");
        }
    }

    #[test]
    fn the_cloud_holds_each_shape_then_moves() {
        let n = 400;
        let held: Vec<usize> = (0..CLOUD_SHAPES)
            .map(|k| cloud(0, n, k as f32 * (CLOUD_HOLD + CLOUD_MOVE) + 0.5).2)
            .collect();
        assert_eq!(held, (0..CLOUD_SHAPES).collect::<Vec<_>>());
        let still: Vec<V3> = (0..n).map(|i| shape_point(0, i, n)).collect();
        assert!(still.iter().all(|p| (p.len() - 1.25).abs() < 1e-3));
        for t in [0.0f32, 1.7, 3.3, 9.9] {
            for i in (0..n).step_by(37) {
                let (p, _, _) = cloud(i, n, t);
                assert!(p.len() < 2.5, "{p:?} at {t}");
            }
        }
    }

    #[test]
    fn numbers_are_short_and_finite() {
        let mut out = String::new();
        num(&mut out, 1.234_567);
        out.push(' ');
        num(&mut out, -0.000_1);
        out.push(' ');
        num(&mut out, f32::NAN);
        assert_eq!(out, "1.235 0 0");
        assert_eq!(hex([1.0, 0.5, 0.0]), "ff8000");
    }
}
