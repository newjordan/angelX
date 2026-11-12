//! The shapes the pods are drawn from beyond the example's own (`cuboid`, `barrel`,
//! `spike`, `tri`), and the lights they burn.
//!
//! A light is a *flat* colour: a block of the sky table, drawn unshaded and held through fog
//! like a window, so an engine's glow, the arc and the seat's lamp still show at 150 m where
//! the iron has gone to haze. The land keeps kinds 0..48 of the table's special rows; the
//! pods take 64 and up (see [`Glow::kind`] and `palette` in `pods.rs`).
use super::*;
use super::super::super::worldkit::palette as land;

/// The lights a pod burns.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Glow {
    /// The arc's white-hot core and its blue edge.
    Arc,
    ArcEdge,
    /// A cold mouth: the engines' intakes.
    Intake,
    /// A nozzle's throat: white-yellow at the middle, orange round it.
    Core,
    Hot,
    /// A seat's own lamp (0..6, the livery's order): tail lights, the pennant's finial, a wyvern's eyes.
    Lamp(u8),
}

/// First block of the sky table's special rows the pods use.
pub const GLOW_BASE: usize = 64;

impl Glow {
    /// Which special block of the sky table holds this light.
    pub fn kind(self) -> usize {
        GLOW_BASE + match self {
            Glow::Arc => 0,
            Glow::ArcEdge => 1,
            Glow::Intake => 2,
            Glow::Core => 3,
            Glow::Hot => 4,
            Glow::Lamp(n) => 8 + n as usize,
        }
    }
}

/// Every light's colour, by block: what `palette` paints into the sky table.
pub const GLOWS: [(usize, [u8; 3]); 11] = [
    (GLOW_BASE, [236, 252, 255]),
    (GLOW_BASE + 1, [74, 172, 255]),
    (GLOW_BASE + 2, [156, 238, 255]),
    (GLOW_BASE + 3, [255, 248, 208]),
    (GLOW_BASE + 4, [255, 132, 36]),
    // The six seats, bright: gold, red, blue, green, violet, rose.
    (GLOW_BASE + 8, [255, 226, 64]),
    (GLOW_BASE + 9, [255, 70, 52]),
    (GLOW_BASE + 10, [84, 152, 255]),
    (GLOW_BASE + 11, [86, 255, 124]),
    (GLOW_BASE + 12, [186, 112, 255]),
    (GLOW_BASE + 13, [255, 128, 176]),
];

/// Make every triangle from `from` on a light of colour `g`.
pub fn lit(m: &mut Mesh, from: usize, g: Glow) {
    let uv = land::special_uv(g.kind());
    for t in &mut m.tris[from..] {
        t.mat = land::SKY;
        t.uv = [uv; 3];
    }
}

/// Draw with `f`, then make what it drew a light.
pub fn glowing(m: &mut Mesh, g: Glow, f: impl FnOnce(&mut Mesh)) {
    let from = m.tris.len();
    f(m);
    lit(m, from, g);
}

/// A flat n-gon at `x` facing along x.
pub fn disc(m: &mut Mesh, x: f32, [y, z]: [f32; 2], r: f32, n: usize, k: u8) {
    let at = |i: usize| {
        let a = i as f32 / n as f32 * TAU + PI / n as f32;
        v3(x, y + r * a.cos(), z + r * a.sin())
    };
    for i in 0..n { tri(m, v3(x, y, z), at(i), at(i + 1), k); }
}

/// A round bar from `a` to `b`, any way it points.
pub fn tube(m: &mut Mesh, a: V3, b: V3, r: f32, n: usize, k: u8) {
    let ax = (b - a).normalize();
    let u = ax.cross(if ax.z.abs() > 0.9 { v3(1., 0., 0.) } else { V3::UP }).normalize();
    let w = ax.cross(u);
    let at = |p: V3, i: usize| {
        let (s, c) = (i as f32 / n as f32 * TAU).sin_cos();
        p + (u * c + w * s) * r
    };
    for i in 0..n {
        m.push_quad(at(a, i), at(a, i + 1), at(b, i + 1), at(b, i), k);
    }
}

/// A box that is smaller (or larger) on top: `[x0, x1, y0, y1]` at the foot and at the head.
pub fn slab(m: &mut Mesh, foot: [f32; 4], z0: f32, head: [f32; 4], z1: f32, k: u8) {
    let p = |r: [f32; 4], z: f32, i: usize| v3(if i & 1 == 0 { r[0] } else { r[1] }, if i & 2 == 0 { r[2] } else { r[3] }, z);
    let (f, h) = (|i| p(foot, z0, i), |i| p(head, z1, i));
    m.push_quad(h(0), h(1), h(3), h(2), k);
    m.push_quad(f(0), f(1), f(3), f(2), k);
    m.push_quad(f(0), f(1), h(1), h(0), k);
    m.push_quad(f(2), f(3), h(3), h(2), k);
    m.push_quad(f(0), f(2), h(2), h(0), k);
    m.push_quad(f(1), f(3), h(3), h(1), k);
}

/// A ring round the x axis: `r_in` to `r_out`, from `x0` to `x1`.
pub fn band(m: &mut Mesh, [x0, x1]: [f32; 2], [y, z]: [f32; 2], [r_in, r_out]: [f32; 2], n: usize, k_out: u8, k_in: u8, k_face: u8) {
    let at = |x: f32, r: f32, i: usize| {
        let a = i as f32 / n as f32 * TAU + PI / n as f32;
        v3(x, y + r * a.cos(), z + r * a.sin())
    };
    for i in 0..n {
        let j = i + 1;
        m.push_quad(at(x0, r_out, i), at(x0, r_out, j), at(x1, r_out, j), at(x1, r_out, i), k_out);
        if k_in != k_out {
            m.push_quad(at(x0, r_in, i), at(x0, r_in, j), at(x1, r_in, j), at(x1, r_in, i), k_in);
        }
        for x in [x0, x1] {
            m.push_quad(at(x, r_in, i), at(x, r_in, j), at(x, r_out, j), at(x, r_out, i), k_face);
        }
    }
}

/// A diamond of light: two spikes base to base, `r` across, `h` each way along `ax`.
pub fn gem(m: &mut Mesh, c: V3, r: f32, h: f32, ax: V3, k: u8) {
    spike(m, c, c + ax * h, r, k);
    spike(m, c, c - ax * h, r, k);
}

/// The dark a pod throws on the road under `c`: a core and a wider penumbra, flat on the road.
pub fn shade_pool(m: &mut Mesh, c: V3, [rx, ry]: [f32; 2]) {
    for (w, z, k) in [(1.0f32, 0.03, PENUMBRA), (0.68, 0.05, SHADE)] {
        let at = |i: usize| c + v3(rx * w * (i as f32 / 12.0 * TAU).cos(), ry * w * (i as f32 / 12.0 * TAU).sin(), z);
        for i in 0..12 { tri(m, v3(c.x, c.y, z), at(i), at(i + 1), k); }
    }
}

/// A cheap, repeatable pseudo-random in -0.5..0.5, from a clock and two seeds.
pub fn jitter(t: f32, a: f32, b: f32) -> f32 {
    ((t * a + b).sin() * 43758.547).fract() - 0.5
}
