//! Mind's eye: sacred geometry streamed live through glass's own dialect.
//!
//! Every frame is built here and sent over the pty as `group` ops of ring,
//! line, glow and lit-primitive layers, inside a synchronized update, and
//! then the program waits for glass to answer a DA1 before it builds the
//! next. Nothing is pre-rendered and nothing is uploaded: the window
//! parses, lays out and draws thousands of strokes a frame. The numbers
//! in the corner are measured as it runs: strokes and glows in the frame,
//! the bytes of escape codes a second, how long glass took to answer the
//! frame, and the frames a second that came of it.
//!
//! The look follows Sacred-grid: a black field, a breathing grid of dots
//! and faint connections, the flower of life blooming into a fractal,
//! Metatron's cube lifting into the platonic solids, the eye itself, and a
//! torus, with colours cycling through gradients on an ease-in-out-cubic.
//!
//! ```text
//! cargo run --release --example glass_mindseye
//! cargo run --release --example glass_mindseye -- --from 44
//! cargo run --release --example glass_mindseye -- --surreal
//! ```
//!
//! `--from s` starts the show `s` seconds in, `--until s` ends it there. `--surreal` turns it on its
//! head (`surreal.rs`). `q` or Ctrl+C ends it.

mod board;
mod surreal;

use std::f32::consts::{PI, TAU};
use std::fmt::Write as _;
use std::io::{self, Read, Write};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// The design space; the window's pixels are fitted to it.
const W: f32 = 1920.0;
const H: f32 = 1080.0;
const FPS: f64 = 60.0;
/// The show's length in seconds.
const LENGTH: f32 = 86.0;
/// The decoded payload a group op may carry is 1 MiB; stay clear of it.
const GROUP_BYTES: usize = 900_000;
/// Ids per group, so no two groups share one.
const GROUP_IDS: u32 = 1 << 20;
/// How long a frame waits for glass's answer before it goes on.
const ACK_WAIT: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug)]
struct Rgb(f32, f32, f32);

fn hex(s: &str) -> Rgb {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0xffffff);
    Rgb(
        ((v >> 16) & 0xff) as f32 / 255.0,
        ((v >> 8) & 0xff) as f32 / 255.0,
        (v & 0xff) as f32 / 255.0,
    )
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    Rgb(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, a.2 + (b.2 - a.2) * t)
}

fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

fn ease_out_back(t: f32) -> f32 {
    let (c1, c3) = (1.70158, 2.70158);
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

fn smooth(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp01((x - edge0) / (edge1 - edge0));
    t * t * (3.0 - 2.0 * t)
}

/// Sacred-grid's gradient cycle: through the colours in turn, each blend
/// eased, once every `cycle` seconds.
fn gradient(colors: &[Rgb], phase: f32) -> Rgb {
    let n = colors.len() as f32;
    let p = phase.rem_euclid(1.0) * n;
    let i = p.floor() as usize % colors.len();
    mix(colors[i], colors[(i + 1) % colors.len()], ease_in_out_cubic(p.fract()))
}

/// A scene's strength at `t`: in over `fade_in` from `start`, out over
/// `fade_out` to `end`, eased both ways.
fn envelope(t: f32, start: f32, end: f32, fade_in: f32, fade_out: f32) -> f32 {
    smooth(start, start + fade_in, t) * (1.0 - smooth(end - fade_out, end, t))
}

struct Palette {
    lines: Vec<Rgb>,
    dots: Vec<Rgb>,
    shapes: Vec<Rgb>,
    eye: Vec<Rgb>,
}

impl Palette {
    fn new() -> Self {
        let p = |v: &[&str]| v.iter().map(|s| hex(s)).collect();
        Self {
            lines: p(&["#0077ff", "#00e5ff", "#461cba", "#ff3fa4"]),
            dots: p(&["#0c7b97", "#461cba", "#0077ff", "#00ffff"]),
            shapes: p(&["#39d5ae", "#2aea6a", "#0077ff", "#00ffff", "#8a5cff"]),
            eye: p(&["#ffc861", "#ff3fa4", "#8a2be2", "#3b2bff", "#00e5ff"]),
        }
    }
}

/// One frame's layers, in window pixels, as `group` records.
struct Canvas {
    scale: f32,
    ox: f32,
    oy: f32,
    records: Vec<String>,
    strokes: u32,
    glows: u32,
    /// Every colour drawn is pulled this far toward this one (the
    /// surreal show's infection); none by default.
    tint: Option<(Rgb, f32)>,
    /// Where the eye looks, from its middle, in design units.
    look: (f32, f32),
    /// How many levels the flower's fractal goes down.
    fractal_depth: u32,
}

// A stroke is a place, a colour and a look; spelling each out reads
// better here than a struct per call.
#[allow(clippy::too_many_arguments)]
impl Canvas {
    fn new(width: f32, height: f32) -> Self {
        let scale = (width / W).min(height / H);
        Self {
            scale,
            ox: (width - W * scale) / 2.0,
            oy: (height - H * scale) / 2.0,
            records: Vec::with_capacity(12_000),
            strokes: 0,
            glows: 0,
            tint: None,
            look: (0.0, 0.0),
            fractal_depth: 3,
        }
    }

    fn clear(&mut self) {
        self.records.clear();
        self.strokes = 0;
        self.glows = 0;
    }

    fn at(&self, x: f32, y: f32) -> (f32, f32) {
        (self.ox + x * self.scale, self.oy + y * self.scale)
    }

    fn colour(&self, c: Rgb, a: f32) -> String {
        let c = match self.tint {
            Some((to, k)) => mix(c, to, k),
            None => c,
        };
        let b = |v: f32| (clamp01(v) * 255.0).round() as u8;
        format!("{:02x}{:02x}{:02x}{:02x}", b(c.0), b(c.1), b(c.2), b(a))
    }

    fn push(&mut self, record: String) {
        self.push_at(-1, true, record);
    }

    /// A record at depth `z`, adding light or covering what is under it.
    fn push_at(&mut self, z: i32, add: bool, mut record: String) {
        let id = self.records.len();
        record.insert_str(0, &format!("layer;id={id};z={z};{}", if add { "add=1;" } else { "" }));
        self.records.push(record);
    }

    /// A turned block: `(x, y)` its middle, design units.
    fn solid(&mut self, x: f32, y: f32, w: f32, h: f32, angle: f32, c: Rgb, a: f32, z: i32, add: bool) {
        if a < 0.004 || w <= 0.0 || h <= 0.0 {
            return;
        }
        let (px, py) = self.at(x, y);
        let (w, h) = (w * self.scale, h * self.scale);
        let rec = format!(
            "k=solid;c={};a={:.4};px={:.1},{:.1},{:.1},{:.1}",
            self.colour(c, a),
            angle.rem_euclid(TAU),
            px - w / 2.0,
            py - h / 2.0,
            w,
            h
        );
        self.push_at(z, add, rec);
        self.strokes += 1;
    }

    /// An uploaded picture, `(x, y)` its middle, turned and faded.
    fn picture(&mut self, id: u32, x: f32, y: f32, w: f32, h: f32, angle: f32, o: f32, z: i32) {
        if o < 0.004 || w <= 0.0 || h <= 0.0 {
            return;
        }
        let (px, py) = self.at(x, y);
        let (w, h) = (w * self.scale, h * self.scale);
        let rec = format!(
            "k=picture;p={id};o={:.3};a={:.4};px={:.1},{:.1},{:.1},{:.1}",
            clamp01(o),
            angle.rem_euclid(TAU),
            px - w / 2.0,
            py - h / 2.0,
            w,
            h
        );
        self.push_at(z, false, rec);
        self.glows += 1;
    }

    /// A ring round (x, y) of radius r, in design units.
    fn ring(&mut self, x: f32, y: f32, r: f32, c: Rgb, a: f32, lw: f32, glow: f32) {
        if a < 0.004 || r <= 0.3 {
            return;
        }
        let (px, py) = self.at(x, y);
        let (r, s) = (r * self.scale, self.scale);
        let mut rec = String::with_capacity(96);
        let _ = write!(
            rec,
            "k=ring;c={};lw={:.2};glow={:.1};px={:.1},{:.1},{:.1},{:.1}",
            self.colour(c, a),
            (lw * s).max(0.25),
            glow * s,
            px - r,
            py - r,
            2.0 * r,
            2.0 * r
        );
        self.push(rec);
        self.strokes += 1;
    }

    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: Rgb, a: f32, lw: f32, glow: f32) {
        self.line_at(-1, x0, y0, x1, y1, c, a, lw, glow);
    }

    /// A line at depth `z`, adding light.
    fn line_at(&mut self, z: i32, x0: f32, y0: f32, x1: f32, y1: f32, c: Rgb, a: f32, lw: f32, glow: f32) {
        if a < 0.004 {
            return;
        }
        let (ax, ay) = self.at(x0, y0);
        let (bx, by) = self.at(x1, y1);
        let length = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
        if length < 0.3 {
            return;
        }
        let angle = (by - ay).atan2(bx - ax).rem_euclid(TAU);
        let (cx, cy) = ((ax + bx) / 2.0, (ay + by) / 2.0);
        let mut rec = String::with_capacity(96);
        let _ = write!(
            rec,
            "k=line;c={};lw={:.2};glow={:.1};a={:.4};px={:.1},{:.1},{:.1},1",
            self.colour(c, a),
            (lw * self.scale).max(0.25),
            glow * self.scale,
            angle,
            cx - length / 2.0,
            cy - 0.5,
            length
        );
        self.push_at(z, true, rec);
        self.strokes += 1;
    }

    fn polyline(&mut self, points: &[(f32, f32)], closed: bool, c: Rgb, a: f32, lw: f32, glow: f32) {
        for pair in points.windows(2) {
            self.line(pair[0].0, pair[0].1, pair[1].0, pair[1].1, c, a, lw, glow);
        }
        if closed && points.len() > 2 {
            let (first, last) = (points[0], points[points.len() - 1]);
            self.line(last.0, last.1, first.0, first.1, c, a, lw, glow);
        }
    }

    fn glow(&mut self, x: f32, y: f32, r: f32, c: Rgb, a: f32) {
        self.glow_at(-1, x, y, r, c, a);
    }

    /// A glow at depth `z`, adding light.
    fn glow_at(&mut self, z: i32, x: f32, y: f32, r: f32, c: Rgb, a: f32) {
        if a < 0.004 || r <= 0.3 {
            return;
        }
        let (px, py) = self.at(x, y);
        let r = r * self.scale;
        self.push_at(z, true, format!(
            "k=glow;c={};px={:.1},{:.1},{:.1},{:.1}",
            self.colour(c, a),
            px - r,
            py - r,
            2.0 * r,
            2.0 * r
        ));
        self.glows += 1;
    }

    /// A lit solid (`k=prim`), not additive: it occludes like a body.
    fn prim(&mut self, x: f32, y: f32, r: f32, shape: u32, c: Rgb, a: f32, phase: f32) {
        if a < 0.004 {
            return;
        }
        let (px, py) = self.at(x, y);
        let r = r * self.scale;
        let id = self.records.len();
        self.records.push(format!(
            "layer;id={id};z=-1;k=prim;shape={shape};speed=0;phase={:.4};c={};px={:.1},{:.1},{:.1},{:.1}",
            phase.rem_euclid(TAU),
            self.colour(c, a),
            px - r,
            py - r,
            2.0 * r,
            2.0 * r
        ));
        self.strokes += 1;
    }
}

/// A point in 3D, turned and projected onto the design space.
#[derive(Clone, Copy)]
struct V3(f32, f32, f32);

impl V3 {
    fn rot_x(self, a: f32) -> V3 {
        let (s, c) = a.sin_cos();
        V3(self.0, self.1 * c - self.2 * s, self.1 * s + self.2 * c)
    }
    fn rot_y(self, a: f32) -> V3 {
        let (s, c) = a.sin_cos();
        V3(self.0 * c + self.2 * s, self.1, -self.0 * s + self.2 * c)
    }
    fn rot_z(self, a: f32) -> V3 {
        let (s, c) = a.sin_cos();
        V3(self.0 * c - self.1 * s, self.0 * s + self.1 * c, self.2)
    }
    fn scale(self, k: f32) -> V3 {
        V3(self.0 * k, self.1 * k, self.2 * k)
    }
    fn norm(self) -> V3 {
        let l = (self.0 * self.0 + self.1 * self.1 + self.2 * self.2).sqrt().max(1e-6);
        V3(self.0 / l, self.1 / l, self.2 / l)
    }
}

/// Perspective onto the screen's middle: (x, y, nearness 0..1).
fn project(p: V3, size: f32) -> (f32, f32, f32) {
    let camera = 4.2;
    let k = camera / (camera - p.2);
    (W / 2.0 + p.0 * size * k, H / 2.0 - p.1 * size * k, clamp01((p.2 + 1.6) / 3.2))
}

type Solid = (Vec<V3>, Vec<(usize, usize)>);

/// Edges: every pair of vertices at the solid's shortest distance.
fn edges_of(vertices: &[V3]) -> Vec<(usize, usize)> {
    let d = |a: V3, b: V3| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)).sqrt();
    let mut shortest = f32::MAX;
    for i in 0..vertices.len() {
        for j in i + 1..vertices.len() {
            shortest = shortest.min(d(vertices[i], vertices[j]));
        }
    }
    let mut edges = Vec::new();
    for i in 0..vertices.len() {
        for j in i + 1..vertices.len() {
            if d(vertices[i], vertices[j]) < shortest * 1.01 {
                edges.push((i, j));
            }
        }
    }
    edges
}

fn solids() -> Vec<Solid> {
    let phi = (1.0 + 5f32.sqrt()) / 2.0;
    let mut out = Vec::new();
    let unit = |v: Vec<V3>| -> Vec<V3> { v.into_iter().map(V3::norm).collect() };
    // Cube.
    let mut cube = Vec::new();
    for x in [-1.0, 1.0] {
        for y in [-1.0, 1.0] {
            for z in [-1.0, 1.0] {
                cube.push(V3(x, y, z));
            }
        }
    }
    let cube = unit(cube);
    // Two tetrahedra in the cube: the merkaba.
    let up: Vec<V3> = cube.iter().copied().filter(|v| v.0 * v.1 * v.2 > 0.0).collect();
    let down: Vec<V3> = cube.iter().copied().filter(|v| v.0 * v.1 * v.2 < 0.0).collect();
    // Octahedron.
    let octa = vec![V3(1.0, 0.0, 0.0), V3(-1.0, 0.0, 0.0), V3(0.0, 1.0, 0.0), V3(0.0, -1.0, 0.0), V3(0.0, 0.0, 1.0), V3(0.0, 0.0, -1.0)];
    // Icosahedron.
    let mut ico = Vec::new();
    for a in [-1.0, 1.0] {
        for b in [-phi, phi] {
            ico.push(V3(0.0, a, b));
            ico.push(V3(a, b, 0.0));
            ico.push(V3(b, 0.0, a));
        }
    }
    let ico = unit(ico);
    // Dodecahedron.
    let mut dodeca = cube.iter().map(|v| v.scale(3f32.sqrt())).collect::<Vec<_>>();
    for a in [-1.0, 1.0] {
        for b in [-1.0, 1.0] {
            dodeca.push(V3(0.0, a / phi, b * phi));
            dodeca.push(V3(a / phi, b * phi, 0.0));
            dodeca.push(V3(b * phi, 0.0, a / phi));
        }
    }
    let dodeca = unit(dodeca);
    // The vector equilibrium (cuboctahedron): the middles of the cube's edges.
    let mut ve = Vec::new();
    for a in [-1.0f32, 1.0] {
        for b in [-1.0f32, 1.0] {
            ve.extend([V3(a, b, 0.0), V3(a, 0.0, b), V3(0.0, a, b)]);
        }
    }
    let ve = unit(ve);
    for vertices in [cube, up, down, octa, ico, dodeca, ve] {
        let edges = edges_of(&vertices);
        out.push((vertices, edges));
    }
    out
}

/// The show.
struct Show {
    palette: Palette,
    solids: Vec<Solid>,
    /// Triangular lattice of the grid, in design units.
    lattice: Vec<(f32, f32)>,
    links: Vec<(usize, usize)>,
}

impl Show {
    fn new() -> Self {
        let spacing = 72.0;
        let row_h = spacing * 3f32.sqrt() / 2.0;
        let (cols, rows) = ((W / spacing) as i32 + 3, (H / row_h) as i32 + 3);
        let mut lattice = Vec::new();
        for r in 0..rows {
            for c in 0..cols {
                let x = (c as f32 - 1.0) * spacing + if r % 2 == 1 { spacing / 2.0 } else { 0.0 };
                lattice.push((x - 20.0, (r as f32 - 1.0) * row_h - 10.0));
            }
        }
        let mut links = Vec::new();
        for i in 0..lattice.len() {
            for j in i + 1..lattice.len() {
                let d = ((lattice[i].0 - lattice[j].0).powi(2) + (lattice[i].1 - lattice[j].1).powi(2)).sqrt();
                if d < spacing * 1.05 {
                    links.push((i, j));
                }
            }
        }
        Self { palette: Palette::new(), solids: solids(), lattice, links }
    }

    fn frame(&self, cv: &mut Canvas, t: f32) {
        let grid = envelope(t, 0.3, 22.0, 2.5, 4.0).max(0.45 * envelope(t, 64.0, 80.0, 3.0, 3.0));
        let flower = envelope(t, 10.0, 36.0, 1.0, 4.0).max(0.6 * envelope(t, 66.0, 80.0, 3.0, 3.0));
        let solids = envelope(t, 31.0, 50.0, 1.0, 3.5).max(0.5 * envelope(t, 67.0, 80.0, 3.0, 3.0));
        let eye = envelope(t, 46.0, 81.0, 1.0, 3.5);
        let torus = envelope(t, 59.0, 80.0, 3.0, 3.0);
        if grid > 0.0 {
            self.grid(cv, t, grid);
        }
        if flower > 0.0 {
            self.flower(cv, t, (t - 10.0).max(0.0), flower);
        }
        if solids > 0.0 {
            self.metatron(cv, t, (t - 31.0).max(0.0), solids);
        }
        if torus > 0.0 {
            self.torus(cv, t, torus);
        }
        if eye > 0.0 {
            // The eye's lights stand back while the torus has the stage.
            let lights = 1.0 - 0.65 * envelope(t, 58.0, 68.0, 2.0, 2.5);
            self.eye(cv, t, (t - 46.0).max(0.0), eye, lights);
        }
    }

    /// Sacred-grid's field: dots breathing outward from the middle, faint
    /// connections, and a wandering point that swells what it passes.
    fn grid(&self, cv: &mut Canvas, t: f32, k: f32) {
        let p = &self.palette;
        let (mx, my) = (W / 2.0 + 560.0 * (t * 0.31).sin(), H / 2.0 + 260.0 * (t * 0.62).sin());
        let wave = |x: f32, y: f32| {
            let d = ((x - W / 2.0).powi(2) + (y - H / 2.0).powi(2)).sqrt();
            0.5 + 0.5 * (t * 1.7 - d * 0.017).sin()
        };
        let pull = |x: f32, y: f32| (-((x - mx).powi(2) + (y - my).powi(2)) / (210.0f32 * 210.0)).exp();
        for &(i, j) in &self.links {
            let (a, b) = (self.lattice[i], self.lattice[j]);
            let w = (wave(a.0, a.1) + wave(b.0, b.1)) / 2.0;
            let near = pull((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
            let c = gradient(&p.lines, t / 9.6 + (a.0 + a.1) / 4000.0);
            cv.line(a.0, a.1, b.0, b.1, c, k * (0.07 + 0.13 * w + 0.35 * near), 0.9, 1.5 + 3.0 * near);
        }
        for &(x, y) in &self.lattice {
            let w = wave(x, y);
            let near = pull(x, y);
            let c = gradient(&p.dots, t / 9.6 + (x - y) / 3000.0);
            cv.glow(x, y, (5.0 + 9.0 * w) * (1.0 + near), c, k * (0.3 + 0.5 * w));
        }
        cv.glow(mx, my, 90.0, gradient(&p.eye, t / 12.0), k * 0.25);
    }

    /// The flower of life, circle by circle, then blooming into a fractal
    /// of flowers at each one's six outer petals, levels turning against
    /// each other.
    fn flower(&self, cv: &mut Canvas, t: f32, u: f32, k: f32) {
        let r = 92.0;
        let base = t * 0.06;
        self.spiral(cv, t, u, k);
        self.flower_at(cv, t, u, W / 2.0, H / 2.0, r, base, 0, k);
        // The six petals round the middle (each a vesica: two arcs), lit
        // one after another.
        let petals = k * smooth(3.0, 4.5, u);
        if petals > 0.0 {
            for n in 0..6 {
                let a = base + n as f32 * PI / 3.0;
                let lit = (-((((t * 0.9 - n as f32 / 6.0).rem_euclid(1.0)) - 0.5) * 6.0).powi(2)).exp();
                let c = mix(gradient(&self.palette.eye, t / 9.6 + n as f32 / 6.0), hex("#ffffff"), 0.3 * lit);
                let (ox, oy) = (W / 2.0 + r * a.cos(), H / 2.0 + r * a.sin());
                for (cx, cy, toward) in [(W / 2.0, H / 2.0, a), (ox, oy, a + PI)] {
                    let pts: Vec<(f32, f32)> = (0..=12)
                        .map(|i| {
                            let b = toward - PI / 3.0 + 2.0 * PI / 3.0 * i as f32 / 12.0;
                            (cx + r * b.cos(), cy + r * b.sin())
                        })
                        .collect();
                    cv.polyline(&pts, false, c, petals * (0.15 + 0.85 * lit), 1.6 + 1.6 * lit, 4.0 + 8.0 * lit);
                }
            }
        }
        // The bounding rings.
        let rim = ease_out_back(clamp01((u - 7.0) / 1.5));
        let c = gradient(&self.palette.shapes, t / 9.6);
        cv.ring(W / 2.0, H / 2.0, 3.0 * r * rim, c, k * 0.55, 2.2, 7.0);
        cv.ring(W / 2.0, H / 2.0, (3.0 * r + 10.0) * rim, c, k * 0.25, 1.0, 3.0);
    }

    /// The golden spiral over its Fibonacci squares, drawing itself on
    /// square by square and turning slowly behind the flower.
    fn spiral(&self, cv: &mut Canvas, t: f32, u: f32, k: f32) {
        let fib = [1.0f32, 1.0, 2.0, 3.0, 5.0, 8.0, 13.0, 21.0, 34.0, 55.0];
        let (mut x0, mut y0, mut x1, mut y1) = (0.0f32, 0.0f32, 1.0f32, 1.0f32);
        let mut squares = vec![(0.0f32, 0.0f32, 1.0f32, 3usize)];
        for (n, &side) in fib.iter().enumerate().skip(1) {
            let way = (n - 1) % 4;
            let (x, y) = match way {
                0 => (x1, y0),
                1 => (x0, y1),
                2 => (x0 - side, y0),
                _ => (x0, y0 - side),
            };
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + side);
            y1 = y1.max(y + side);
            squares.push((x, y, side, way));
        }
        // The spiral's eye sits on the screen's middle, turning.
        let (scale, turn) = (8.5f32, t * 0.05 + 0.4);
        let (ex, ey) = (0.62f32, 0.38f32);
        let place = |x: f32, y: f32| {
            let (dx, dy) = ((x - ex) * scale, (y - ey) * scale);
            let (s, c) = turn.sin_cos();
            (W / 2.0 + dx * c - dy * s, H / 2.0 + dx * s + dy * c)
        };
        for (n, &(x, y, side, way)) in squares.iter().enumerate() {
            let show = k * smooth(n as f32 * 0.5, n as f32 * 0.5 + 0.8, u) * 0.6;
            if show <= 0.0 {
                continue;
            }
            let c = gradient(&self.palette.lines, n as f32 / 10.0 + t / 12.0);
            let corners = [place(x, y), place(x + side, y), place(x + side, y + side), place(x, y + side)];
            cv.polyline(&corners, true, c, show * 0.35, 1.0, 2.0);
            let (cx, cy, from) = match way {
                0 => (x, y + side, -PI / 2.0),
                1 => (x, y, 0.0),
                2 => (x + side, y, PI / 2.0),
                _ => (x + side, y + side, PI),
            };
            let arc: Vec<(f32, f32)> = (0..=16)
                .map(|i| {
                    let a = from + PI / 2.0 * i as f32 / 16.0;
                    place(cx + side * a.cos(), cy + side * a.sin())
                })
                .collect();
            cv.polyline(&arc, false, hex("#ffc861"), show, 2.2, 7.0);
        }
    }

    #[allow(clippy::too_many_arguments)]
    /// One flower and, past its time, its children. Circles appear in
    /// order (vesica, seed, flower), each with three stacked pulses.
    fn flower_at(&self, cv: &mut Canvas, t: f32, u: f32, x: f32, y: f32, r: f32, rot: f32, level: u32, k: f32) {
        // Deeper levels are many and small: fainter and tighter, or the
        // light they add washes the structure out.
        const ALPHA: [f32; 4] = [0.85, 0.5, 0.3, 0.17];
        const WIDTH: [f32; 4] = [2.0, 1.3, 0.9, 0.7];
        const GLOW: [f32; 4] = [6.0, 3.5, 2.0, 1.0];
        // When this level starts appearing, and how long a circle takes.
        const START: [f32; 4] = [0.0, 9.0, 12.0, 15.0];
        let l = level as usize;
        let local = u - START[l];
        if local < 0.0 {
            return;
        }
        let centres = flower_centres(r, rot);
        // Each level its own gradient, so the levels read apart.
        let p = &self.palette;
        let colors = [&p.shapes, &p.dots, &p.lines, &p.eye][l];
        let c = gradient(colors, t / 9.6 + level as f32 * 0.22);
        for (n, &(dx, dy)) in centres.iter().enumerate() {
            // Level 0 draws one at a time; deeper levels all at once.
            let at = if level == 0 { [0.0, 0.6, 1.9, 2.25, 2.6, 2.95, 3.3, 5.0][order_slot(n)] + n.saturating_sub(7) as f32 * 0.18 } else { 0.0 };
            let grow = ease_out_back(clamp01((local - at) / 0.9));
            if grow <= 0.0 {
                continue;
            }
            let breathe = 1.0 + 0.04 * (t * 1.3 + n as f32 * 0.7).sin() * (level as f32 + 1.0).recip();
            cv.ring(x + dx, y + dy, r * grow * breathe, c, k * ALPHA[l], WIDTH[l], GLOW[l]);
            if level == 0 {
                // Three stacked pulses, a third of a second apart.
                for s in 0..3 {
                    let p = (local - at - s as f32 * 0.33) / 1.6;
                    if (0.0..1.0).contains(&p) {
                        let pc = gradient(&self.palette.lines, t / 9.6 + 0.3);
                        cv.ring(x + dx, y + dy, r * (1.0 + 0.7 * p), pc, k * 0.5 * (1.0 - p).powi(2), 1.2, 5.0);
                    }
                }
            }
        }
        if level < cv.fractal_depth {
            for i in 0..6 {
                let a = rot + i as f32 * PI / 3.0;
                let (cx, cy) = (x + 2.0 * r * a.cos(), y + 2.0 * r * a.sin());
                let turn = if level.is_multiple_of(2) { -1.6 } else { 1.3 };
                self.flower_at(cv, t, u, cx, cy, r * 0.5, rot * turn + 0.3 * (level + 1) as f32, level + 1, k);
            }
        }
    }

    /// The fruit of life's thirteen circles and the 78 lines between their
    /// centres, then the cube hidden in it turning out into 3D, with the
    /// platonic solids nested round it.
    fn metatron(&self, cv: &mut Canvas, t: f32, u: f32, k: f32) {
        let p = &self.palette;
        let r = 58.0;
        let mut centres = vec![(0.0f32, 0.0f32)];
        for ring in [2.0, 4.0] {
            for i in 0..6 {
                let a = PI / 6.0 + i as f32 * PI / 3.0 + t * 0.03;
                centres.push((ring * r * a.cos(), ring * r * a.sin()));
            }
        }
        let flat = k * (1.0 - smooth(6.0, 8.5, u));
        if flat > 0.0 {
            for (n, &(x, y)) in centres.iter().enumerate() {
                let grow = ease_out_back(clamp01((u - n as f32 * 0.08) / 0.8));
                cv.ring(W / 2.0 + x, H / 2.0 + y, r * grow, gradient(&p.shapes, t / 9.6), flat * 0.7, 1.6, 5.0);
            }
            let mut n = 0;
            for i in 0..centres.len() {
                for j in i + 1..centres.len() {
                    let draw = clamp01((u - 1.0 - n as f32 * 0.045) / 0.4);
                    n += 1;
                    if draw <= 0.0 {
                        continue;
                    }
                    let (a, b) = (centres[i], centres[j]);
                    let (bx, by) = (a.0 + (b.0 - a.0) * draw, a.1 + (b.1 - a.1) * draw);
                    let c = gradient(&p.lines, t / 9.6 + n as f32 / 78.0);
                    cv.line(W / 2.0 + a.0, H / 2.0 + a.1, W / 2.0 + bx, H / 2.0 + by, c, flat * 0.55, 1.1, 3.0);
                }
            }
        }
        // The solids: from the cube seen down its diagonal (Metatron's
        // hexagon) turning out into space.
        let depth = k * smooth(5.0, 8.0, u);
        if depth <= 0.0 {
            return;
        }
        let open = smooth(6.0, 12.0, u);
        // Down the cube's diagonal: (1, 1, 1) onto the eye's axis.
        let iso = |v: V3| v.rot_y(-PI / 4.0).rot_x(0.6155);
        let turn = |v: V3, spin: f32| {
            let v = iso(v).rot_y(spin * open).rot_x(0.4 * open * (t * 0.3).sin());
            v.rot_z(t * 0.05)
        };
        // The cube's hexagon on the outer centres: its corners sit at √(2/3).
        let cube = 4.0 * r / (2.0f32 / 3.0).sqrt();
        let sizes = [cube, cube, cube, 2.2 * r, 5.4 * r, 6.4 * r, 3.4 * r];
        let spins = [t * 0.4, t * 0.9, -t * 0.9, -t * 0.55, t * 0.3, -t * 0.22, t * 0.7];
        for (s, (vertices, edges)) in self.solids.iter().enumerate() {
            let show = if s == 0 { 1.0 } else { smooth(7.0 + s as f32 * 0.8, 9.0 + s as f32 * 0.8, u) };
            if show <= 0.0 {
                continue;
            }
            let points: Vec<(f32, f32, f32)> =
                vertices.iter().map(|&v| project(turn(v, spins[s]), sizes[s])).collect();
            let c = gradient(if s % 2 == 0 { &p.lines } else { &p.shapes }, t / 9.6 + s as f32 * 0.17);
            for (e, &(i, j)) in edges.iter().enumerate() {
                let (a, b) = (points[i], points[j]);
                let near = (a.2 + b.2) / 2.0;
                cv.line(a.0, a.1, b.0, b.1, c, depth * show * (0.22 + 0.6 * near), 1.0 + 1.4 * near, 2.0 + 4.0 * near);
                // A pulse of light running along the edge.
                let at = (t * 0.6 + e as f32 * 0.137 + s as f32 * 0.31).fract();
                let (p0, p1) = (at.max(0.0), (at + 0.16).min(1.0));
                let along = |q: f32| (a.0 + (b.0 - a.0) * q, a.1 + (b.1 - a.1) * q);
                let (x0, y0) = along(p0);
                let (x1, y1) = along(p1);
                cv.line(x0, y0, x1, y1, mix(c, hex("#ffffff"), 0.6), depth * show * (0.5 + 0.5 * near), 2.2, 7.0);
            }
            for &(x, y, near) in &points {
                cv.glow(x, y, 10.0 + 16.0 * near, c, depth * show * (0.3 + 0.5 * near));
            }
        }
    }

    /// The eye: a vesica opening, an iris of rings, lotus and Sri Yantra,
    /// a lit pupil, a phyllotaxis of lights flowing into it, and rays.
    fn eye(&self, cv: &mut Canvas, t: f32, u: f32, k: f32, lights: f32) {
        let p = &self.palette;
        let (cx, cy) = (W / 2.0, H / 2.0);
        // The lids: open, a blink at nine seconds, open.
        let blink = 1.0 - smooth(9.0, 9.18, u) + smooth(9.25, 9.6, u);
        let open = smooth(0.2, 2.6, u) * blink.min(1.0);
        let (width, height) = (820.0f32, 330.0 * open.max(0.02));
        // Phyllotaxis: lights on the golden angle, flowing inward.
        let golden = PI * (3.0 - 5f32.sqrt());
        let flow = u * 9.0;
        let count = 1400;
        for n in 0..count {
            // A light keeps its angle (its index) and moves in a step of
            // the spiral each unit of flow.
            let i = n as f32 + 1.0 - flow.fract();
            let idx = n as f32 + flow.floor();
            let rho = 15.5 * i.sqrt();
            if rho > 600.0 {
                continue;
            }
            let a = idx * golden + t * 0.12;
            let (x, y) = (cx + rho * a.cos(), cy + rho * a.sin() * 0.92);
            let edge = smooth(600.0, 430.0, rho) * smooth(30.0, 90.0, rho);
            let c = gradient(&p.eye, idx / 400.0 + t / 14.0);
            cv.glow(x, y, 3.0 + rho * 0.022, c, k * lights * edge * 0.75);
        }
        // Rays.
        for n in 0..96 {
            let a = n as f32 * TAU / 96.0 + t * 0.02;
            let shimmer = 0.5 + 0.5 * (t * 2.3 + n as f32 * 1.7).sin();
            let (r0, r1) = (430.0, 430.0 + 60.0 + 110.0 * shimmer);
            let c = gradient(&p.eye, n as f32 / 96.0 + t / 10.0);
            cv.line(cx + r0 * a.cos(), cy + r0 * a.sin(), cx + r1 * a.cos(), cy + r1 * a.sin(), c, k * 0.18 * shimmer, 0.8, 2.5);
        }
        // The lids: two arcs, three strokes deep.
        if height > 1.0 {
            let rho = ((width / 2.0).powi(2) + (height / 2.0).powi(2)) / height;
            let half = ((width / 2.0) / rho).asin();
            for (sign, phase) in [(-1.0f32, 0.0), (1.0, 0.5)] {
                for depth in 0..3 {
                    let off = depth as f32 * 7.0;
                    let pts: Vec<(f32, f32)> = (0..=48)
                        .map(|i| {
                            let a = -half + 2.0 * half * i as f32 / 48.0;
                            let (s, c) = a.sin_cos();
                            (cx + (rho + off) * s, cy + sign * (height / 2.0 + off - (rho + off) * (1.0 - c)))
                        })
                        .collect();
                    let col = gradient(&p.eye, t / 9.6 + phase + depth as f32 * 0.08);
                    cv.polyline(&pts, false, col, k * [0.9, 0.4, 0.2][depth], [2.2, 1.4, 1.0][depth], [8.0, 5.0, 3.0][depth]);
                }
            }
        }
        // The iris, held inside the opening.
        let iris = (height / 2.0 * 0.92).min(150.0);
        let ik = k * smooth(0.6, 2.4, u) * blink.min(1.0);
        // Where it looks: the iris leans toward `cv.look`, kept inside the lids.
        let (cx, cy) = (cx + cv.look.0.clamp(-150.0, 150.0) * open, cy + cv.look.1.clamp(-45.0, 45.0) * open);
        if iris > 4.0 && ik > 0.0 {
            for (n, f) in [1.0f32, 0.9, 0.78, 0.6, 0.47].iter().enumerate() {
                let pulse = 1.0 + 0.03 * (t * 2.0 - n as f32).sin();
                let c = gradient(&p.eye, t / 8.0 + n as f32 * 0.15);
                cv.ring(cx, cy, iris * f * pulse, c, ik * 0.8, 1.6, 6.0);
            }
            // Lotus: sixteen petals.
            for n in 0..16 {
                let a = n as f32 * TAU / 16.0 - t * 0.25;
                let c = gradient(&p.eye, n as f32 / 16.0 + t / 6.0);
                cv.ring(cx + iris * 0.72 * a.cos(), cy + iris * 0.72 * a.sin(), iris * 0.2, c, ik * 0.5, 1.0, 4.0);
            }
            // Sri Yantra: four triangles up, five down, turning apart.
            for n in 0..9 {
                let up = n < 4;
                let size = iris * (0.58 - n as f32 * 0.035);
                let a0 = if up { -PI / 2.0 } else { PI / 2.0 } + if up { t * 0.15 } else { -t * 0.12 };
                let pts: Vec<(f32, f32)> =
                    (0..3).map(|i| (cx + size * (a0 + i as f32 * TAU / 3.0).cos(), cy + size * (a0 + i as f32 * TAU / 3.0).sin())).collect();
                let c = gradient(&p.eye, t / 7.0 + n as f32 * 0.11);
                cv.polyline(&pts, true, c, ik * 0.55, 1.1, 3.5);
            }
            // The pupil: a lit torus turning, over a deep glow.
            cv.glow(cx, cy, iris * 1.4, hex("#3b2bff"), ik * 0.5);
            cv.prim(cx, cy, iris * 0.42, 5, gradient(&p.eye, t / 11.0 + 0.5), ik, t * 0.8);
        }
    }

    /// A torus of rings, turning through itself.
    fn torus(&self, cv: &mut Canvas, t: f32, k: f32) {
        let p = &self.palette;
        let (major, minor) = (1.0f32, 0.38f32);
        let turn = |v: V3| v.rot_x(1.1 + 0.25 * (t * 0.2).sin()).rot_y(t * 0.18).rot_z(t * 0.07);
        let size = 330.0;
        let meridians = 48;
        let steps = 36;
        for m in 0..meridians {
            let theta = m as f32 * TAU / meridians as f32 + t * 0.15;
            let pts: Vec<(f32, f32, f32)> = (0..steps)
                .map(|s| {
                    let phi = s as f32 * TAU / steps as f32;
                    let v = V3((major + minor * phi.cos()) * theta.cos(), minor * phi.sin(), (major + minor * phi.cos()) * theta.sin());
                    project(turn(v), size)
                })
                .collect();
            let c = gradient(&p.lines, m as f32 / meridians as f32 + t / 9.6);
            for s in 0..steps {
                let (a, b) = (pts[s], pts[(s + 1) % steps]);
                let near = (a.2 + b.2) / 2.0;
                cv.line(a.0, a.1, b.0, b.1, c, k * (0.12 + 0.6 * near), 1.0 + near, 2.0 + 4.0 * near);
            }
        }
    }
}

/// The nineteen centres of the flower of life, turned by `rot`: the
/// middle, six at r, six at r√3, six at 2r. Ordered so the first two make
/// the vesica and the first seven the seed.
fn flower_centres(r: f32, rot: f32) -> Vec<(f32, f32)> {
    let mut c = vec![(0.0, 0.0)];
    for i in 0..6 {
        let a = rot + i as f32 * PI / 3.0;
        c.push((r * a.cos(), r * a.sin()));
    }
    for i in 0..6 {
        let a = rot + PI / 6.0 + i as f32 * PI / 3.0;
        c.push((r * 3f32.sqrt() * a.cos(), r * 3f32.sqrt() * a.sin()));
    }
    for i in 0..6 {
        let a = rot + i as f32 * PI / 3.0;
        c.push((2.0 * r * a.cos(), 2.0 * r * a.sin()));
    }
    c
}

/// When a level-0 circle starts, by its slot: the first two together (the
/// vesica), the seed's next five in turn, then the rest (offset by index).
fn order_slot(n: usize) -> usize {
    match n {
        0 => 0,
        1 => 1,
        2..=6 => n,
        _ => 7,
    }
}

fn b64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(A[(n >> 18) as usize & 63] as char);
        out.push(A[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { A[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { A[n as usize & 63] as char } else { '=' });
    }
    out
}

/// The frame's records as group ops, each under the payload cap. Groups
/// the last frame used and this one does not are emptied.
fn group_ops(records: &[String], used_before: u32, out: &mut Vec<u8>) -> u32 {
    let mut group = 0u32;
    let mut start = 0;
    while start < records.len() {
        let mut bytes = 0;
        let mut end = start;
        // A record grows by its id's new digits (at most eight) and a newline.
        while end < records.len() && bytes + records[end].len() + 9 < GROUP_BYTES {
            bytes += records[end].len() + 9;
            end += 1;
        }
        group += 1;
        let mut payload = String::with_capacity(bytes);
        for (n, record) in records[start..end].iter().enumerate() {
            if n > 0 {
                payload.push('\n');
            }
            // Ids are unique across groups.
            let id_end = record.find(";z=").unwrap_or(0);
            let local: u32 = record["layer;id=".len()..id_end].parse().unwrap_or(0);
            payload.push_str("layer;id=");
            let _ = write!(payload, "{}", (group - 1) * GROUP_IDS + local % GROUP_IDS);
            payload.push_str(&record[id_end..]);
        }
        let _ = write!(
            ByteWriter(out),
            "\x1b_glass;group;g={group};n={};{}\x1b\\",
            end - start,
            b64(payload.as_bytes())
        );
        start = end;
    }
    for g in group + 1..=used_before {
        let _ = write!(ByteWriter(out), "\x1b_glass;group;g={g};n=0;\x1b\\");
    }
    group
}

struct ByteWriter<'a>(&'a mut Vec<u8>);

impl std::fmt::Write for ByteWriter<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.extend_from_slice(s.as_bytes());
        Ok(())
    }
}

/// What the reader thread hears from glass.
enum Heard {
    Ack(Instant),
    Size(f32, f32),
    Quit,
}

fn read_replies(tx: mpsc::Sender<Heard>) {
    let mut stdin = io::stdin();
    let mut buf = [0u8; 4096];
    let mut pending: Vec<u8> = Vec::new();
    loop {
        let n = match stdin.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => n,
        };
        let now = Instant::now();
        pending.extend_from_slice(&buf[..n]);
        loop {
            if let Some(i) = pending.iter().position(|&b| b == 3 || b == b'q')
                && pending[..i].iter().all(|&b| b != 0x1b)
            {
                let _ = tx.send(Heard::Quit);
                return;
            }
            let Some(esc) = pending.iter().position(|&b| b == 0x1b) else {
                pending.clear();
                break;
            };
            let rest = &pending[esc..];
            let Some(end) = rest.iter().skip(2).position(|&b| (0x40..=0x7e).contains(&b)).map(|i| i + 2) else {
                pending.drain(..esc);
                break;
            };
            let seq = &rest[..=end];
            if seq.starts_with(b"\x1b[?") && seq.ends_with(b"c") {
                let _ = tx.send(Heard::Ack(now));
            } else if seq.starts_with(b"\x1b[4;") && seq.ends_with(b"t") {
                let body = String::from_utf8_lossy(&seq[4..seq.len() - 1]).to_string();
                let mut parts = body.split(';').filter_map(|v| v.parse::<f32>().ok());
                if let (Some(h), Some(w)) = (parts.next(), parts.next()) {
                    let _ = tx.send(Heard::Size(w, h));
                }
            }
            pending.drain(..esc + end + 1);
        }
    }
}

fn stty(args: &[&str]) {
    let _ = Command::new("stty").args(args).stdin(std::fs::File::open("/dev/tty").map_or(std::process::Stdio::null(), Into::into)).status();
}

fn tty_size() -> (u16, u16) {
    let out = Command::new("stty")
        .arg("size")
        .stdin(std::fs::File::open("/dev/tty").map_or(std::process::Stdio::null(), Into::into))
        .output();
    let text = out.map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    let mut it = text.split_whitespace().filter_map(|v| v.parse().ok());
    (it.next().unwrap_or(40), it.next().unwrap_or(150))
}

fn send(bytes: &[u8]) {
    let mut out = io::stdout().lock();
    let _ = out.write_all(bytes);
    let _ = out.flush();
}

/// Type a line as a person would.
fn type_out(text: &str, gap: Duration) {
    for ch in text.chars() {
        send(ch.to_string().as_bytes());
        std::thread::sleep(gap);
    }
}

fn wait_ack(rx: &Receiver<Heard>, quit: &mut bool) -> Option<Instant> {
    let deadline = Instant::now() + ACK_WAIT;
    loop {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Heard::Ack(at)) => return Some(at),
            Ok(Heard::Quit) => {
                *quit = true;
                return None;
            }
            Ok(Heard::Size(..)) => {}
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return None,
        }
    }
}

fn group_of_digits(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// The running numbers the corner shows and the close sums up.
#[derive(Default)]
struct Hud {
    groups: u32,
    frames: u64,
    bytes_total: u64,
    peak_layers: usize,
    /// Frames sent in the last second, with their bytes.
    window: std::collections::VecDeque<(Instant, usize)>,
    /// The last thirty answers, in milliseconds.
    acks: std::collections::VecDeque<f32>,
    ack_sum: f64,
    ack_count: u64,
    /// This run's frames for the live graphs: (t, fps, MB/s, answer ms).
    samples: board::Samples,
}

const NAMES: [(f32, &str); 6] =
    [(0.0, "the field"), (10.0, "the flower of life"), (31.0, "metatron"), (46.0, "the mind's eye"), (59.0, "the torus"), (66.0, "all of it")];

/// One frame at `t`, as the bytes that go to glass: the layers in a
/// synchronized update, the corner, the close, and a DA1 to answer.
fn build_frame(
    show: &Show,
    cv: &mut Canvas,
    hud: &mut Hud,
    t: f32,
    rows: u16,
    cols: u16,
    mut surreal: Option<&mut surreal::Surreal>,
    board: Option<&board::Board>,
    end: f32,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 << 20);
    cv.clear();
    if let Some(s) = surreal.as_deref_mut() {
        s.before(cv, t);
    }
    show.frame(cv, t);
    // The surreal show's own: video frames before the groups, its scene
    // and the rain after.
    let (mut pre, mut post) = (Vec::new(), Vec::new());
    if let Some(s) = surreal.as_deref_mut() {
        s.after(cv, t, hud.frames, &mut pre, &mut post);
    }
    // The numbers: glass against kitty, and this run, graphed.
    let now = Instant::now();
    while hud.window.front().is_some_and(|(at, _)| now.duration_since(*at) > Duration::from_secs(1)) {
        hud.window.pop_front();
    }
    let ack_now = if hud.acks.is_empty() { 0.0 } else { hud.acks.iter().sum::<f32>() / hud.acks.len() as f32 };
    let mbps = hud.window.iter().map(|(_, b)| b).sum::<usize>() as f32 / 1e6;
    if hud.samples.back().is_none_or(|s| t > s.0) {
        hud.samples.push_back((t, hud.window.len() as f32, mbps, ack_now));
    }
    while hud.samples.front().is_some_and(|s| t - s.0 > 13.0) {
        hud.samples.pop_front();
    }
    if let Some(b) = board {
        b.draw(cv, t, &hud.samples, end);
    }
    out.extend_from_slice(b"\x1b[?2026h");
    out.extend_from_slice(&pre);
    hud.groups = group_ops(&cv.records, hud.groups, &mut out);
    out.extend_from_slice(&post);
    hud.peak_layers = hud.peak_layers.max(cv.records.len());
    // The corner: what this frame is, measured.
    let now = Instant::now();
    while hud.window.front().is_some_and(|(at, _)| now.duration_since(*at) > Duration::from_secs(1)) {
        hud.window.pop_front();
    }
    let per_second: usize = hud.window.iter().map(|(_, b)| b).sum();
    let fps = hud.window.len();
    let ack = if hud.acks.is_empty() { 0.0 } else { hud.acks.iter().sum::<f32>() / hud.acks.len() as f32 };
    let scene = NAMES.iter().rev().find(|(at, _)| t >= *at).map_or("", |(_, n)| n);
    let fade = (envelope(t, 3.0, LENGTH - 2.0, 1.5, 2.0) * 255.0) as u8;
    let dim = |v: u8| (u16::from(v) * u16::from(fade) / 255) as u8;
    let _ = write!(
        ByteWriter(&mut out),
        "\x1b[1;3H\x1b[2K\x1b[38;2;{};{};{}mglass \x1b[38;2;{};{};{}m\u{00b7} {scene}\x1b[{rows};3H\x1b[2K\x1b[38;2;{};{};{}m{} strokes \u{00b7} {} glows \u{00b7} {:.1} MB/s of escape codes \u{00b7} glass answers in {:.1} ms \u{00b7} {} fps\x1b[0m",
        dim(0), dim(229), dim(255), dim(120), dim(140), dim(190), dim(110), dim(130), dim(175),
        group_of_digits(u64::from(cv.strokes)),
        group_of_digits(u64::from(cv.glows)),
        per_second as f64 / 1e6,
        ack,
        fps
    );
    // The close: what was just seen, in its own numbers.
    let outro = smooth(80.5, 82.0, t) * (1.0 - smooth(LENGTH - 0.8, LENGTH, t));
    if outro > 0.0 {
        let v = |c: u8| (f32::from(c) * outro) as u8;
        let lines = [
            "every stroke you saw was streamed through a pty as escape codes, live".to_string(),
            format!(
                "{} frames \u{00b7} up to {} layers a frame \u{00b7} {:.0} MB sent \u{00b7} glass answered in {:.1} ms on average",
                group_of_digits(hud.frames),
                group_of_digits(hud.peak_layers as u64),
                hud.bytes_total as f64 / 1e6,
                if hud.ack_count > 0 { hud.ack_sum / hud.ack_count as f64 } else { 0.0 }
            ),
        ];
        for (n, line) in lines.iter().enumerate() {
            let row = rows / 2 + n as u16 * 2;
            let col = (cols as usize).saturating_sub(line.chars().count()) / 2 + 1;
            let (r, g, b) = if n == 0 { (v(0), v(229), v(255)) } else { (v(120), v(140), v(190)) };
            let _ = write!(ByteWriter(&mut out), "\x1b[{row};1H\x1b[2K\x1b[{row};{col}H\x1b[38;2;{r};{g};{b}m{line}\x1b[0m");
        }
    }
    out.extend_from_slice(b"\x1b[?2026l\x1b[c");
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let from: f32 = args.iter().position(|a| a == "--from").and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let surreal_on = args.iter().any(|a| a == "--surreal");
    let until: f32 = args.iter().position(|a| a == "--until").and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(LENGTH);
    stty(&["raw", "-echo"]);
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || read_replies(tx));
    let (rows, cols) = tty_size();

    // The window's pixels (XTWINOPS 14).
    send(b"\x1b[14t");
    let mut size = (W, H);
    let deadline = Instant::now() + Duration::from_millis(600);
    while let Ok(heard) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        if let Heard::Size(w, h) = heard {
            size = (w, h);
            break;
        }
    }

    // The surreal show's pictures and generators get ready while the
    // prompt types.
    let preparing = surreal_on.then(|| {
        std::thread::spawn(move || {
            let mut s = surreal::Surreal::new(cols, rows, size);
            let mut bytes = Vec::new();
            s.setup(&mut bytes);
            (s, bytes)
        })
    });

    // The bench's numbers, and their words drawn, while the prompt types.
    // With `MINDSEYE_BENCH` unset there is no board; the film plays without it.
    let bench = std::env::var("MINDSEYE_BENCH").unwrap_or_default();
    let preparing_board = std::thread::spawn(move || {
        board::Board::load(&bench).map(|mut b| {
            let mut bytes = Vec::new();
            b.setup(&mut bytes);
            (b, bytes)
        })
    });

    let gap = Duration::from_millis(38);
    let mut quit = false;
    if from <= 0.0 {
        send(b"\x1b[2J\x1b[H\x1b[38;2;120;140;190m~\x1b[0m \x1b[38;2;0;229;255m\xe2\x9d\xaf\x1b[0m ");
        std::thread::sleep(Duration::from_millis(700));
        type_out(if surreal_on { "glass-mindseye --surreal" } else { "glass-mindseye" }, gap);
        std::thread::sleep(Duration::from_millis(350));
        send(b"\r\n");
        let info = format!(
            "\x1b[38;2;90;110;160mwindow {}x{} px, {}x{} cells. Sacred geometry streamed through the pty as glass dialect\r\nring, line, glow and lit-solid layers, every frame, live. Recorded by glass itself through NVENC.\x1b[0m\r\n",
            size.0 as u32, size.1 as u32, cols, rows
        );
        send(info.as_bytes());
        if surreal_on {
            send(b"\x1b[38;2;255;17;51mand some things that should not be there.\x1b[0m\r\n");
        }
        std::thread::sleep(Duration::from_millis(1600));
    }
    let mut surreal = preparing.and_then(|p| p.join().ok()).map(|(s, bytes)| {
        send(&bytes);
        s
    });
    let board = preparing_board.join().ok().flatten().map(|(b, bytes)| {
        send(&bytes);
        b
    });
    send(b"\x1b[?25l\x1b[2J");

    let show = Show::new();
    let mut cv = Canvas::new(size.0, size.1);
    let started = Instant::now();
    let mut hud = Hud::default();
    let mut next_at = Instant::now();
    let mut built: Option<(f32, Vec<u8>)> = None;
    // A frame's bytes go out on their own thread: the pty takes them only
    // as fast as glass reads, and the next frame is built meanwhile.
    let (writes, to_write) = mpsc::sync_channel::<Vec<u8>>(1);
    let writer = std::thread::spawn(move || {
        for bytes in to_write {
            send(&bytes);
        }
    });
    while !quit {
        let (t, out) = match built.take() {
            Some(frame) => frame,
            None => {
                let t = from + started.elapsed().as_secs_f32();
                (t, build_frame(&show, &mut cv, &mut hud, t, rows, cols, surreal.as_mut(), board.as_ref(), until.min(LENGTH)))
            }
        };
        if t >= until.min(LENGTH) {
            break;
        }
        let sent = Instant::now();
        let length = out.len();
        if writes.send(out).is_err() {
            break;
        }
        hud.bytes_total += length as u64;
        hud.window.push_back((sent, length));
        hud.frames += 1;
        // The next frame on the clock, built while glass takes this one
        // in. Behind the clock, it moves up to now rather than sending a
        // burst to catch up.
        next_at += Duration::from_secs_f64(1.0 / FPS);
        if next_at < Instant::now() {
            next_at = Instant::now();
        }
        let t_next = from + next_at.duration_since(started).as_secs_f32();
        built = Some((t_next, build_frame(&show, &mut cv, &mut hud, t_next, rows, cols, surreal.as_mut(), board.as_ref(), until.min(LENGTH))));
        if let Some(at) = wait_ack(&rx, &mut quit) {
            let ms = at.duration_since(sent).as_secs_f32() * 1000.0;
            hud.acks.push_back(ms);
            if hud.acks.len() > 30 {
                hud.acks.pop_front();
            }
            hud.ack_sum += f64::from(ms);
            hud.ack_count += 1;
        }
        let now = Instant::now();
        if next_at > now {
            std::thread::sleep(next_at - now);
        }
    }
    drop(writes);
    let _ = writer.join();
    let (groups, frames, bytes_total, peak_layers, ack_sum, ack_count) =
        (hud.groups, hud.frames, hud.bytes_total, hud.peak_layers, hud.ack_sum, hud.ack_count);
    let seconds = started.elapsed().as_secs_f64();
    let mut end = Vec::new();
    for g in 1..=groups {
        let _ = write!(ByteWriter(&mut end), "\x1b_glass;group;g={g};n=0;\x1b\\");
    }
    if let Some(s) = surreal.as_mut() {
        s.end(&mut end);
    }
    end.extend_from_slice(b"\x1b[2J\x1b[H\x1b[?25h");
    send(&end);
    drop(surreal);
    stty(&["sane"]);
    let summary = format!(
        "\x1b[38;2;0;229;255mmind's eye\x1b[0m: {} frames in {:.1} s ({:.1} fps), up to {} layers a frame,\r\n{:.1} MB of escape codes ({:.1} MB/s); glass answered each frame in {:.2} ms on average.\r\n",
        frames,
        seconds,
        frames as f64 / seconds,
        group_of_digits(peak_layers as u64),
        bytes_total as f64 / 1e6,
        bytes_total as f64 / 1e6 / seconds,
        if ack_count > 0 { ack_sum / ack_count as f64 } else { 0.0 }
    );
    send(summary.as_bytes());
    eprint!("{}", summary.replace("\x1b[38;2;0;229;255m", "").replace("\x1b[0m", "").replace("\r\n", "\n"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_stay_under_the_cap_and_ids_stay_apart() {
        let mut cv = Canvas::new(W, H);
        for i in 0..30_000 {
            cv.ring(100.0 + (i % 100) as f32, 100.0, 20.0, hex("#ffffff"), 0.5, 1.0, 2.0);
        }
        let mut out = Vec::new();
        let groups = group_ops(&cv.records, 0, &mut out);
        assert!(groups >= 2);
        let text = String::from_utf8_lossy(&out);
        for op in text.split("\x1b_glass;group;").skip(1) {
            let payload = op.rsplit(';').next().unwrap().trim_end_matches("\x1b\\");
            assert!(payload.len() / 4 * 3 <= GROUP_BYTES + 3, "{}", payload.len());
        }
        let mut again = Vec::new();
        assert_eq!(group_ops(&cv.records[..10], groups, &mut again), 1);
        assert!(String::from_utf8_lossy(&again).contains(&format!("g={groups};n=0;")));
    }

    #[test]
    fn the_solids_have_their_edges() {
        let counts: Vec<usize> = solids().iter().map(|(_, e)| e.len()).collect();
        assert_eq!(counts, vec![12, 6, 6, 12, 30, 30, 24]);
    }

    #[test]
    fn the_peak_frame_is_thousands_of_layers() {
        let show = Show::new();
        let mut cv = Canvas::new(W, H);
        show.frame(&mut cv, 30.0);
        assert!(cv.records.len() > 4000, "{}", cv.records.len());
        cv.clear();
        show.frame(&mut cv, 72.0);
        assert!(cv.records.len() > 6000, "{}", cv.records.len());
    }
}
