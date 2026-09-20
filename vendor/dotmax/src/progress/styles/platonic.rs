//! Platonic solids and sacred-3D-form progress bars.
//!
//! Each bar is a rotating 3-D wireframe of a genuine geometric solid, drawn via
//! orthographic projection onto the dot lattice.  `ctx.time` drives the spin;
//! `ctx.eased` (0 → 1) reveals edges one-by-one so the solid assembles itself
//! as progress advances.
//!
//! A single `project` helper (identical signature to the topology module's) and
//! a bounded Bresenham edge-drawer are shared by every style.  Scale is
//! auto-fitted so the circumscribed sphere just fills `min(w/2, h)` dot-radii.
//!
//! # Styles
//!
//! | Name | Geometry |
//! |---|---|
//! | `tetrahedron`          | 4 vertices, 6 edges — the fire solid |
//! | `cube`                 | 8 vertices, 12 edges — the earth solid |
//! | `octahedron`           | 6 vertices, 12 edges — the air solid |
//! | `dodecahedron`         | 20 vertices, 30 edges — golden-ratio faces |
//! | `icosahedron`          | 12 vertices, 30 edges — dual of dodecahedron |
//! | `merkaba`              | Two counter-rotating tetrahedra (star-tetrahedron) |
//! | `star-octangulum`      | Stella octangula — two interlocked tetrahedra (octahedron dual) |
//! | `cuboctahedron`        | Vector equilibrium / Metatron's core, 12 vertices 24 edges |
//! | `nested-solids`        | Cube inside its dual octahedron, both spinning |
//! | `stellated-dodecahedron` | Great stellated dodecahedron — spiky star |
//! | `unfolding-net`        | Cube net folds up into 3-D as eased → 1 |

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, Color, DotmaxError};

// ── Shared 3-D helpers ────────────────────────────────────────────────────────

/// Rotate `(x, y, z)` about X by `ax` then Y by `ay` (extrinsic Euler XY),
/// then orthographically project onto the dot lattice centred at `(cx, cy)`.
///
/// Returns `(screen_x, screen_y)` as `i32` for `draw::dot_i`.
#[inline]
fn project(x: f32, y: f32, z: f32, ax: f32, ay: f32, cx: i32, cy: i32, scale: f32) -> (i32, i32) {
    let (sax, cax) = ax.sin_cos();
    let y1 = y * cax - z * sax;
    let z1 = y * sax + z * cax;
    let (say, cay) = ay.sin_cos();
    let x2 = x * cay + z1 * say;
    let y2 = y1;
    let sx = cx + (x2 * scale).round() as i32;
    let sy = cy - (y2 * scale).round() as i32;
    (sx, sy)
}

/// Draw a Bresenham line between two projected points.  Step count is capped
/// at 400 so wide grids stay snappy; `draw::dot_i` silently ignores OOB.
#[inline]
fn draw_edge(grid: &mut BrailleGrid, x0: i32, y0: i32, x1: i32, y1: i32) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1i32 } else { -1i32 };
    let sy = if y0 < y1 { 1i32 } else { -1i32 };
    let mut x = x0;
    let mut y = y0;
    let mut err = dx - dy;
    let max_steps = (dx + dy + 2).min(400);
    for _ in 0..max_steps {
        draw::dot_i(grid, x, y);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
}

/// Compute grid centre in dot coords and a uniform scale so a unit-radius
/// solid fits `min(w/2, h)` dot-radii.  The `shrink` factor lets a style
/// reduce the scale further (e.g. 0.85 for dodecahedron which has circumradius > 1).
fn grid_centre_scale(grid: &BrailleGrid, shrink: f32) -> (i32, i32, f32) {
    let (dw, dh) = draw::dot_dims(grid);
    let cx = (dw / 2) as i32;
    let cy = (dh / 2) as i32;
    // Fit to the smaller of half-width and full-height (dots), with margin.
    let r = (dw / 2).min(dh) as f32;
    let scale = (r * 0.82 * shrink).max(1.0);
    (cx, cy, scale)
}

/// Project a slice of 3-D vertices with given rotation angles, returning
/// screen-space `(i32, i32)` for each.
fn project_verts(
    verts: &[[f32; 3]],
    ax: f32,
    ay: f32,
    cx: i32,
    cy: i32,
    scale: f32,
) -> Vec<(i32, i32)> {
    verts
        .iter()
        .map(|&[x, y, z]| project(x, y, z, ax, ay, cx, cy, scale))
        .collect()
}

/// Draw `n_show` edges from `edges`, using pre-projected `pts`.
fn draw_edges_partial(
    grid: &mut BrailleGrid,
    pts: &[(i32, i32)],
    edges: &[(usize, usize)],
    n_show: usize,
) {
    for &(a, b) in edges.iter().take(n_show) {
        if a < pts.len() && b < pts.len() {
            let (x0, y0) = pts[a];
            let (x1, y1) = pts[b];
            draw_edge(grid, x0, y0, x1, y1);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Tetrahedron — fire solid (4 vertices, 6 edges)
// ─────────────────────────────────────────────────────────────────────────────

/// Tetrahedron inscribed in the unit sphere.
const TETRA_VERTS: [[f32; 3]; 4] = [
    [0.0, 1.0, 0.0],                   // top
    [0.942809, -0.333333, 0.0],        // front-right
    [-0.471405, -0.333333, 0.816497],  // back-left
    [-0.471405, -0.333333, -0.816497], // back-right
];
const TETRA_EDGES: [(usize, usize); 6] = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];

struct Tetrahedron;
impl ProgressStyle for Tetrahedron {
    fn name(&self) -> &str {
        "tetrahedron"
    }
    fn theme(&self) -> &str {
        "platonic"
    }
    fn describe(&self) -> &str {
        "Tetrahedron — the fire solid: 4 vertices and 6 edges assembling as progress grows, spinning on time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_centre_scale(grid, 1.0);
        let ax = ctx.time * 0.41;
        let ay = ctx.time * 0.63;
        let pts = project_verts(&TETRA_VERTS, ax, ay, cx, cy, scale);
        let n_show = (ctx.eased * TETRA_EDGES.len() as f32).ceil() as usize;
        draw_edges_partial(grid, &pts, &TETRA_EDGES, n_show);
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Cube / hexahedron — earth solid (8 vertices, 12 edges)
// ─────────────────────────────────────────────────────────────────────────────

/// Cube with vertices at (±1/√3, ±1/√3, ±1/√3) — unit circumsphere.
const S3: f32 = 0.577_350_3; // 1/√3

const CUBE_VERTS: [[f32; 3]; 8] = [
    [-S3, -S3, -S3],
    [S3, -S3, -S3],
    [S3, S3, -S3],
    [-S3, S3, -S3],
    [-S3, -S3, S3],
    [S3, -S3, S3],
    [S3, S3, S3],
    [-S3, S3, S3],
];
const CUBE_EDGES: [(usize, usize); 12] = [
    // Bottom face.
    (0, 1),
    (1, 2),
    (2, 3),
    (3, 0),
    // Top face.
    (4, 5),
    (5, 6),
    (6, 7),
    (7, 4),
    // Verticals.
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];

struct Cube;
impl ProgressStyle for Cube {
    fn name(&self) -> &str {
        "cube"
    }
    fn theme(&self) -> &str {
        "platonic"
    }
    fn describe(&self) -> &str {
        "Hexahedron — the earth solid: 8-vertex cube with 12 edges revealed face-by-face as progress grows"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_centre_scale(grid, 1.0);
        let ax = ctx.time * 0.37;
        let ay = ctx.time * 0.51;
        let pts = project_verts(&CUBE_VERTS, ax, ay, cx, cy, scale);
        let n_show = (ctx.eased * CUBE_EDGES.len() as f32).ceil() as usize;
        draw_edges_partial(grid, &pts, &CUBE_EDGES, n_show);
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Octahedron — air solid (6 vertices, 12 edges)
// ─────────────────────────────────────────────────────────────────────────────

const OCTA_VERTS: [[f32; 3]; 6] = [
    [1.0, 0.0, 0.0],
    [-1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, -1.0, 0.0],
    [0.0, 0.0, 1.0],
    [0.0, 0.0, -1.0],
];
const OCTA_EDGES: [(usize, usize); 12] = [
    (0, 2),
    (0, 3),
    (0, 4),
    (0, 5),
    (1, 2),
    (1, 3),
    (1, 4),
    (1, 5),
    (2, 4),
    (2, 5),
    (3, 4),
    (3, 5),
];

struct Octahedron;
impl ProgressStyle for Octahedron {
    fn name(&self) -> &str {
        "octahedron"
    }
    fn theme(&self) -> &str {
        "platonic"
    }
    fn describe(&self) -> &str {
        "Octahedron — the air solid: 6-vertex double-pyramid with 12 equilateral-triangle edges spinning on time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_centre_scale(grid, 1.0);
        let ax = ctx.time * 0.44;
        let ay = ctx.time * 0.59;
        let pts = project_verts(&OCTA_VERTS, ax, ay, cx, cy, scale);
        let n_show = (ctx.eased * OCTA_EDGES.len() as f32).ceil() as usize;
        draw_edges_partial(grid, &pts, &OCTA_EDGES, n_show);
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Dodecahedron — aether solid (20 vertices, 30 edges)
//    Circumradius = √3 ⟹ shrink by 1/√3.
// ─────────────────────────────────────────────────────────────────────────────

/// Golden ratio φ = (1+√5)/2.
const PHI: f32 = 1.618_033_9;
/// 1/φ.
const INV_PHI: f32 = 0.618_033_9;
/// Circumradius of the dodecahedron built from these coordinates is √3.
const DODECA_SCALE: f32 = 1.0 / 1.732_050_8; // 1/√3

const DODECA_VERTS: [[f32; 3]; 20] = [
    // ±1 permutations (8 vertices).
    [1.0, 1.0, 1.0],
    [1.0, 1.0, -1.0],
    [1.0, -1.0, 1.0],
    [1.0, -1.0, -1.0],
    [-1.0, 1.0, 1.0],
    [-1.0, 1.0, -1.0],
    [-1.0, -1.0, 1.0],
    [-1.0, -1.0, -1.0],
    // (0, ±1/φ, ±φ) cyclic permutations (12 vertices).
    [0.0, INV_PHI, PHI],
    [0.0, INV_PHI, -PHI],
    [0.0, -INV_PHI, PHI],
    [0.0, -INV_PHI, -PHI],
    [INV_PHI, PHI, 0.0],
    [INV_PHI, -PHI, 0.0],
    [-INV_PHI, PHI, 0.0],
    [-INV_PHI, -PHI, 0.0],
    [PHI, 0.0, INV_PHI],
    [PHI, 0.0, -INV_PHI],
    [-PHI, 0.0, INV_PHI],
    [-PHI, 0.0, -INV_PHI],
];

/// Edges: the 30 edges of the dodecahedron.
/// Each vertex has degree 3.  Edge list derived from the known vertex adjacency.
const DODECA_EDGES: [(usize, usize); 30] = [
    // Vertex 0: (1,1,1) connects to vertices 8,12,16.
    (0, 8),
    (0, 12),
    (0, 16),
    // Vertex 1: (1,1,-1) connects to 9,12,17.
    (1, 9),
    (1, 12),
    (1, 17),
    // Vertex 2: (1,-1,1) connects to 10,13,16.
    (2, 10),
    (2, 13),
    (2, 16),
    // Vertex 3: (1,-1,-1) connects to 11,13,17.
    (3, 11),
    (3, 13),
    (3, 17),
    // Vertex 4: (-1,1,1) connects to 8,14,18.
    (4, 8),
    (4, 14),
    (4, 18),
    // Vertex 5: (-1,1,-1) connects to 9,14,19.
    (5, 9),
    (5, 14),
    (5, 19),
    // Vertex 6: (-1,-1,1) connects to 10,15,18.
    (6, 10),
    (6, 15),
    (6, 18),
    // Vertex 7: (-1,-1,-1) connects to 11,15,19.
    (7, 11),
    (7, 15),
    (7, 19),
    // (0,±INV_PHI,±PHI) ring connections (vertices 8-11).
    (8, 10),
    (9, 11),
    // (±INV_PHI,±PHI,0) ring connections (vertices 12-15).
    (12, 14),
    (13, 15),
    // (±PHI,0,±INV_PHI) ring connections (vertices 16-19).
    (16, 17),
    (18, 19),
];

struct Dodecahedron;
impl ProgressStyle for Dodecahedron {
    fn name(&self) -> &str {
        "dodecahedron"
    }
    fn theme(&self) -> &str {
        "platonic"
    }
    fn describe(&self) -> &str {
        "Dodecahedron — the aether solid: 20 golden-ratio vertices and 30 pentagonal edges spinning and assembling"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_centre_scale(grid, DODECA_SCALE);
        let ax = ctx.time * 0.29;
        let ay = ctx.time * 0.47;
        let pts = project_verts(&DODECA_VERTS, ax, ay, cx, cy, scale);
        let n_show = (ctx.eased * DODECA_EDGES.len() as f32).ceil() as usize;
        draw_edges_partial(grid, &pts, &DODECA_EDGES, n_show);
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Icosahedron — water solid (12 vertices, 30 edges)
//    Vertices from (0, ±1, ±φ) cyclic permutations; circumradius = √(1+φ²).
// ─────────────────────────────────────────────────────────────────────────────

/// Circumradius of icosahedron with these coords = √(1+φ²) ≈ 1.902.
const ICOSA_SCALE: f32 = 1.0 / 1.902_113_0;

const ICOSA_VERTS: [[f32; 3]; 12] = [
    // (0, ±1, ±φ).
    [0.0, 1.0, PHI],
    [0.0, 1.0, -PHI],
    [0.0, -1.0, PHI],
    [0.0, -1.0, -PHI],
    // (±1, ±φ, 0).
    [1.0, PHI, 0.0],
    [1.0, -PHI, 0.0],
    [-1.0, PHI, 0.0],
    [-1.0, -PHI, 0.0],
    // (±φ, 0, ±1).
    [PHI, 0.0, 1.0],
    [PHI, 0.0, -1.0],
    [-PHI, 0.0, 1.0],
    [-PHI, 0.0, -1.0],
];

/// 30 edges of the icosahedron (each vertex connects to 5 neighbours at distance 2).
const ICOSA_EDGES: [(usize, usize); 30] = [
    // Top cap (0).
    (0, 2),
    (0, 4),
    (0, 6),
    (0, 8),
    (0, 10),
    // Upper ring.
    (4, 8),
    (8, 2),
    (2, 10),
    (10, 6),
    (6, 4),
    // Bottom cap (3).
    (3, 1),
    (3, 5),
    (3, 7),
    (3, 9),
    (3, 11),
    // Lower ring.
    (1, 9),
    (9, 5),
    (5, 7),
    (7, 11),
    (11, 1),
    // Equatorial connectors.
    (4, 1),
    (1, 6),
    (6, 11),
    (11, 10),
    (10, 7),
    (7, 5),
    (5, 2),
    (2, 8),
    (8, 9),
    (9, 3),
];

struct Icosahedron;
impl ProgressStyle for Icosahedron {
    fn name(&self) -> &str {
        "icosahedron"
    }
    fn theme(&self) -> &str {
        "platonic"
    }
    fn describe(&self) -> &str {
        "Icosahedron — the water solid: 12 golden-ratio vertices forming 30 equilateral-triangle edges, spinning on time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_centre_scale(grid, ICOSA_SCALE);
        let ax = ctx.time * 0.33;
        let ay = ctx.time * 0.54;
        let pts = project_verts(&ICOSA_VERTS, ax, ay, cx, cy, scale);
        let n_show = (ctx.eased * ICOSA_EDGES.len() as f32).ceil() as usize;
        draw_edges_partial(grid, &pts, &ICOSA_EDGES, n_show);
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Merkaba — star tetrahedron: two counter-rotating tetrahedra
// ─────────────────────────────────────────────────────────────────────────────

/// The merkaba has two interlocked tetrahedra.  One uses the canonical upward
/// tetrahedron; the other is its inversion (downward — dual).  They counter-rotate
/// with time so the Merkaba field animates distinctly even without edge-reveal.

struct Merkaba;
impl ProgressStyle for Merkaba {
    fn name(&self) -> &str {
        "merkaba"
    }
    fn theme(&self) -> &str {
        "platonic"
    }
    fn describe(&self) -> &str {
        "Merkaba (star-tetrahedron): two interlocked tetrahedra counter-rotating — the light-body vehicle of sacred geometry"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_centre_scale(grid, 1.0);

        // Rotation for the upward tetrahedron.
        let ax_up = ctx.time * 0.39;
        let ay_up = ctx.time * 0.57;
        // Counter-rotation for the downward tetrahedron.
        let ax_dn = ctx.time * 0.39;
        let ay_dn = -ctx.time * 0.57;

        // Upward tetrahedron (same as TETRA_VERTS).
        let pts_up = project_verts(&TETRA_VERTS, ax_up, ay_up, cx, cy, scale);
        // Downward tetrahedron (invert y).
        let tetra_down: [[f32; 3]; 4] = TETRA_VERTS.map(|[x, y, z]| [x, -y, z]);
        let pts_dn = project_verts(&tetra_down, ax_dn, ay_dn, cx, cy, scale);

        // Reveal first tetrahedron on eased 0→0.5, second on 0.5→1.
        let total_edges = TETRA_EDGES.len() * 2;
        let n_show = (ctx.eased * total_edges as f32).ceil() as usize;
        let n_up = n_show.min(TETRA_EDGES.len());
        let n_dn = n_show.saturating_sub(TETRA_EDGES.len());

        draw_edges_partial(grid, &pts_up, &TETRA_EDGES, n_up);
        draw_edges_partial(grid, &pts_dn, &TETRA_EDGES, n_dn);
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Star octangulum (stella octangula) — two interlocked tetrahedra as octahedron dual
//    Structurally different from merkaba: both tetrahedra share the SAME rotation
//    axis, offset by 180°, and the stella octangula has 8 spike tips + central octahedron
//    All 12 edges rendered in sequence of 3 groups: top tet, bottom tet, connectors.
// ─────────────────────────────────────────────────────────────────────────────

struct StarOctangulum;
impl ProgressStyle for StarOctangulum {
    fn name(&self) -> &str {
        "star-octangulum"
    }
    fn theme(&self) -> &str {
        "platonic"
    }
    fn describe(&self) -> &str {
        "Stella octangula: two tetrahedra dual to each other inscribed in an octahedron — eight triangular spikes, one rotation axis"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_centre_scale(grid, 1.0);
        // Both tetrahedra share the same rotation (co-rotating, not counter-rotating).
        let ax = ctx.time * 0.35;
        let ay = ctx.time * 0.52;

        // Upward tet — scaled to circumradius 1.
        let pts_up = project_verts(&TETRA_VERTS, ax, ay, cx, cy, scale);
        // Downward tet — invert all three axes for the dual orientation.
        let tetra_dual: [[f32; 3]; 4] = TETRA_VERTS.map(|[x, y, z]| [-x, -y, -z]);
        let pts_dn = project_verts(&tetra_dual, ax, ay, cx, cy, scale);

        // Also draw the inner octahedron formed by the intersection.
        // Octahedron vertices are midpoints of the stella's edges (unit sphere).
        let pts_oct = project_verts(&OCTA_VERTS, ax, ay, cx, cy, scale * 0.577_350_3);

        let total = TETRA_EDGES.len() * 2 + OCTA_EDGES.len();
        let n_show = (ctx.eased * total as f32).ceil() as usize;
        let n_up = n_show.min(TETRA_EDGES.len());
        let n_dn = n_show
            .saturating_sub(TETRA_EDGES.len())
            .min(TETRA_EDGES.len());
        let n_oct = n_show.saturating_sub(TETRA_EDGES.len() * 2);

        draw_edges_partial(grid, &pts_up, &TETRA_EDGES, n_up);
        draw_edges_partial(grid, &pts_dn, &TETRA_EDGES, n_dn);
        draw_edges_partial(grid, &pts_oct, &OCTA_EDGES, n_oct);
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Cuboctahedron — vector equilibrium / Metatron's core
//    12 vertices: (±1, ±1, 0) and all cyclic permutations. 24 edges.
// ─────────────────────────────────────────────────────────────────────────────

const CUBOCTA_VERTS: [[f32; 3]; 12] = [
    // (±1, ±1, 0).
    [1.0, 1.0, 0.0],
    [1.0, -1.0, 0.0],
    [-1.0, 1.0, 0.0],
    [-1.0, -1.0, 0.0],
    // (±1, 0, ±1).
    [1.0, 0.0, 1.0],
    [1.0, 0.0, -1.0],
    [-1.0, 0.0, 1.0],
    [-1.0, 0.0, -1.0],
    // (0, ±1, ±1).
    [0.0, 1.0, 1.0],
    [0.0, 1.0, -1.0],
    [0.0, -1.0, 1.0],
    [0.0, -1.0, -1.0],
];

/// Cuboctahedron circumradius = √2, so shrink.
const CUBOCTA_SCALE: f32 = 1.0 / 1.414_213_6;

/// 24 edges of the cuboctahedron.  Each vertex has degree 4.
/// Two vertices are adjacent iff their distance = √2 (= edge length here).
const CUBOCTA_EDGES: [(usize, usize); 24] = [
    // Vertex 0 ( 1, 1, 0): neighbours 4,5,8,9.
    (0, 4),
    (0, 5),
    (0, 8),
    (0, 9),
    // Vertex 1 ( 1,-1, 0): neighbours 4,5,10,11.
    (1, 4),
    (1, 5),
    (1, 10),
    (1, 11),
    // Vertex 2 (-1, 1, 0): neighbours 6,7,8,9.
    (2, 6),
    (2, 7),
    (2, 8),
    (2, 9),
    // Vertex 3 (-1,-1, 0): neighbours 6,7,10,11.
    (3, 6),
    (3, 7),
    (3, 10),
    (3, 11),
    // Cross-ring edges (between the three "belt" squares).
    (4, 8),
    (4, 10),
    (5, 9),
    (5, 11),
    (6, 8),
    (6, 10),
    (7, 9),
    (7, 11),
];

struct Cuboctahedron;
impl ProgressStyle for Cuboctahedron {
    fn name(&self) -> &str {
        "cuboctahedron"
    }
    fn theme(&self) -> &str {
        "platonic"
    }
    fn describe(&self) -> &str {
        "Cuboctahedron — vector equilibrium / Metatron's core: 12 vertices at edge-midpoints of a cube, 8 triangles and 6 squares"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_centre_scale(grid, CUBOCTA_SCALE);
        let ax = ctx.time * 0.31;
        let ay = ctx.time * 0.49;
        let pts = project_verts(&CUBOCTA_VERTS, ax, ay, cx, cy, scale);
        let n_show = (ctx.eased * CUBOCTA_EDGES.len() as f32).ceil() as usize;
        draw_edges_partial(grid, &pts, &CUBOCTA_EDGES, n_show);
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. Nested solids — cube inside its dual octahedron, both spinning
//    Structurally unique: two DIFFERENT solids rendered simultaneously at
//    different scales and independent rotation speeds.
// ─────────────────────────────────────────────────────────────────────────────

struct NestedSolids;
impl ProgressStyle for NestedSolids {
    fn name(&self) -> &str {
        "nested-solids"
    }
    fn theme(&self) -> &str {
        "platonic"
    }
    fn describe(&self) -> &str {
        "Nested duals: a cube spinning inside its dual octahedron — both rotating at different rates, revealed in two waves"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_centre_scale(grid, 1.0);

        // Outer octahedron — slower rotation, revealed in first half of eased.
        let ax_oct = ctx.time * 0.27;
        let ay_oct = ctx.time * 0.41;
        let pts_oct = project_verts(&OCTA_VERTS, ax_oct, ay_oct, cx, cy, scale);

        // Inner cube — faster rotation, revealed in second half, scaled to inradius.
        // Inradius of octahedron = 1/√3 ≈ 0.577, so cube circumradius ≈ 0.577.
        let cube_inner_scale = scale * 0.577_350_3; // fit cube inside octahedron
        let ax_cube = ctx.time * 0.55;
        let ay_cube = ctx.time * 0.71;
        let pts_cube = project_verts(&CUBE_VERTS, ax_cube, ay_cube, cx, cy, cube_inner_scale);

        let n_show = (ctx.eased * (OCTA_EDGES.len() + CUBE_EDGES.len()) as f32).ceil() as usize;
        let n_oct = n_show.min(OCTA_EDGES.len());
        let n_cube = n_show.saturating_sub(OCTA_EDGES.len());

        draw_edges_partial(grid, &pts_oct, &OCTA_EDGES, n_oct);
        draw_edges_partial(grid, &pts_cube, &CUBE_EDGES, n_cube);
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 10. Stellated dodecahedron — great stellated dodecahedron (spiky star)
//     Built by erecting a pentagonal pyramid on each of the 12 dodecahedron
//     faces.  Approximated here with the 20 original dodecahedron vertices
//     PLUS 12 spike apices, one beyond each face centre, 60 spoke edges.
// ─────────────────────────────────────────────────────────────────────────────

/// Face centres of the dodecahedron (approximate — used as spike bases).
/// The dodecahedron has 12 pentagonal faces; their centres lie along the 12
/// icosahedron vertices (the dual relationship).  We scale icosahedron verts
/// to get spike tips.
struct StellatedDodecahedron;
impl ProgressStyle for StellatedDodecahedron {
    fn name(&self) -> &str {
        "stellated-dodecahedron"
    }
    fn theme(&self) -> &str {
        "platonic"
    }
    fn describe(&self) -> &str {
        "Great stellated dodecahedron: dodecahedron with 12 pentagonal star-pyramids erupting from each face — a cosmic sea-urchin"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_centre_scale(grid, DODECA_SCALE * 0.72);
        let ax = ctx.time * 0.25;
        let ay = ctx.time * 0.43;

        // Draw the dodecahedron base skeleton first.
        let pts_dodeca = project_verts(&DODECA_VERTS, ax, ay, cx, cy, scale);

        // The 12 spike tips — icosahedron vertices scaled outward by φ.
        let spike_scale = scale * PHI * ICOSA_SCALE;
        let pts_spikes = project_verts(&ICOSA_VERTS, ax, ay, cx, cy, spike_scale);

        // Each spike tip connects to the 5 nearest dodecahedron vertices.
        // For simplicity: each icosahedron vertex (face centre) connects to the
        // 5 dodecahedron vertices closest to it.  We store these as a fixed table.
        // (Derived from geometry: each face of the dodecahedron is a pentagon with 5 vertices.)
        let spike_fans: [(usize, [usize; 5]); 12] = [
            (0, [0, 8, 10, 4, 2]),   // +z face centre → vertices around top face
            (1, [1, 9, 11, 5, 3]),   // -z face
            (2, [0, 12, 14, 4, 16]), // +y-adjacent face
            (3, [1, 12, 14, 5, 17]), // another
            (4, [0, 8, 12, 16, 2]),
            (5, [6, 10, 15, 7, 18]),
            (6, [3, 11, 13, 15, 7]),
            (7, [3, 13, 17, 11, 19]),
            (8, [4, 14, 5, 19, 18]),
            (9, [2, 16, 17, 3, 13]),
            (10, [6, 18, 19, 7, 15]),
            (11, [1, 9, 11, 19, 5]),
        ];

        let n_base = DODECA_EDGES.len();
        let n_spokes = 12 * 5; // 60 spoke edges
        let total = n_base + n_spokes;
        let n_show = (ctx.eased * total as f32).ceil() as usize;
        let n_d = n_show.min(n_base);
        let n_s = n_show.saturating_sub(n_base);

        draw_edges_partial(grid, &pts_dodeca, &DODECA_EDGES, n_d);

        // Draw spokes (spike tip → dodecahedron vertex).
        let mut spoke_drawn = 0usize;
        'outer: for &(si, verts_around) in &spike_fans {
            for &di in &verts_around {
                if spoke_drawn >= n_s {
                    break 'outer;
                }
                if si < pts_spikes.len() && di < pts_dodeca.len() {
                    let (x0, y0) = pts_spikes[si];
                    let (x1, y1) = pts_dodeca[di];
                    draw_edge(grid, x0, y0, x1, y1);
                }
                spoke_drawn += 1;
            }
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 11. Unfolding net — a cube's net folds up into 3-D as eased → 1
//     At eased=0, a cross-shaped net lies flat (all faces in the XY plane).
//     At eased=1, the 6 faces have folded up into a cube.
//     Each face is drawn as a square outline (4 edges), folded by lerping
//     the face's normal direction from flat (z=0) to its final orientation.
// ─────────────────────────────────────────────────────────────────────────────

struct UnfoldingNet;
impl ProgressStyle for UnfoldingNet {
    fn name(&self) -> &str {
        "unfolding-net"
    }
    fn theme(&self) -> &str {
        "platonic"
    }
    fn describe(&self) -> &str {
        "Cube net: a cross-shaped flat net folds progressively into a 3-D cube as progress advances — geometry becoming solid"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_centre_scale(grid, 0.9);
        let ax = ctx.time * 0.30;
        let ay = ctx.time * 0.48;
        let t = ctx.eased; // 0 = flat net, 1 = cube

        // A standard cube cross-net has 6 faces.  We define each face by its
        // 4 corners in *net* space (flat) and in *cube* space (3-D).
        // We interpolate corners between flat and folded at parameter t.

        // Face corners in net space (centered at origin, side length 1).
        // Faces: bottom, front, right, left, back, top — standard cross layout.
        // Net layout (2D, z=0), face side = 1:
        //   [4]           top
        //   [2][0][1][3]  left, front, right, back
        //   [5]           bottom

        let half = 0.5_f32;

        // Net positions (flat, z=0) — each face's 4 corners (bl, br, tr, tl).
        let net: [[[f32; 3]; 4]; 6] = [
            // 0: front face (centre of cross, at (0,0)).
            [
                [-half, -half, 0.0],
                [half, -half, 0.0],
                [half, half, 0.0],
                [-half, half, 0.0],
            ],
            // 1: right face (at (1,0)).
            [
                [half, -half, 0.0],
                [3.0 * half, -half, 0.0],
                [3.0 * half, half, 0.0],
                [half, half, 0.0],
            ],
            // 2: left face (at (-1,0)).
            [
                [-3.0 * half, -half, 0.0],
                [-half, -half, 0.0],
                [-half, half, 0.0],
                [-3.0 * half, half, 0.0],
            ],
            // 3: back face (at (2,0)).
            [
                [3.0 * half, -half, 0.0],
                [5.0 * half, -half, 0.0],
                [5.0 * half, half, 0.0],
                [3.0 * half, half, 0.0],
            ],
            // 4: top face (at (0,1)).
            [
                [-half, half, 0.0],
                [half, half, 0.0],
                [half, 3.0 * half, 0.0],
                [-half, 3.0 * half, 0.0],
            ],
            // 5: bottom face (at (0,-1)).
            [
                [-half, -3.0 * half, 0.0],
                [half, -3.0 * half, 0.0],
                [half, -half, 0.0],
                [-half, -half, 0.0],
            ],
        ];

        // Cube corner positions for each face (fully folded).
        let cube: [[[f32; 3]; 4]; 6] = [
            // 0: front face z=+half (normal +z).
            [
                [-half, -half, half],
                [half, -half, half],
                [half, half, half],
                [-half, half, half],
            ],
            // 1: right face x=+half.
            [
                [half, -half, half],
                [half, -half, -half],
                [half, half, -half],
                [half, half, half],
            ],
            // 2: left face x=-half.
            [
                [-half, -half, -half],
                [-half, -half, half],
                [-half, half, half],
                [-half, half, -half],
            ],
            // 3: back face z=-half.
            [
                [half, -half, -half],
                [-half, -half, -half],
                [-half, half, -half],
                [half, half, -half],
            ],
            // 4: top face y=+half.
            [
                [-half, half, half],
                [half, half, half],
                [half, half, -half],
                [-half, half, -half],
            ],
            // 5: bottom face y=-half.
            [
                [-half, -half, -half],
                [half, -half, -half],
                [half, -half, half],
                [-half, -half, half],
            ],
        ];

        // Scale net so it fits the same display window as the folded cube.
        // Net spans from x=-1.5 to x=2.5 (4 units wide), y=-1.5 to y=1.5 (3 tall).
        // Cube spans ±0.5.  Map net scale to roughly 0.4 → 1.0 of display scale.
        let net_s = scale / 3.0;
        let cube_s = scale;

        for fi in 0..6usize {
            // Interpolate each corner between net and cube.
            let corners: Vec<(i32, i32)> = (0..4)
                .map(|ci| {
                    let [nx, ny, nz] = net[fi][ci];
                    let [cx2, cy2, cz] = cube[fi][ci];
                    let x = nx + (cx2 - nx) * t;
                    let y = ny + (cy2 - ny) * t;
                    let z = nz + (cz - nz) * t;
                    // Scale: lerp from net_s to cube_s.
                    let s = net_s + (cube_s - net_s) * t;
                    project(x, y, z, ax, ay, cx, cy, s)
                })
                .collect();

            // Draw the 4 edges of the face square.
            for ei in 0..4usize {
                let (x0, y0) = corners[ei];
                let (x1, y1) = corners[(ei + 1) % 4];
                draw_edge(grid, x0, y0, x1, y1);
            }
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Registry
// ─────────────────────────────────────────────────────────────────────────────

// ---------------------------------------------------------------------------
// Theme tint — crystal blue.
// ---------------------------------------------------------------------------

/// Gradient endpoints for this theme's signature tint.
const TINT_START: Color = Color::rgb(206, 224, 255);
const TINT_END: Color = Color::rgb(92, 142, 232);

/// Sample the theme gradient at `t` in `0.0..=1.0`.
fn sample_tint(t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let lerp = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t) as u8;
    Color::rgb(
        lerp(TINT_START.r, TINT_END.r),
        lerp(TINT_START.g, TINT_END.g),
        lerp(TINT_START.b, TINT_END.b),
    )
}

/// Applies the theme's signature gradient to every cell the inner style drew,
/// drifting slowly with `time`. Styles stay monochrome-safe underneath: drop
/// the wrapper in [`styles`] for uncolored output.
struct Tinted<S>(S);

impl<S: ProgressStyle> ProgressStyle for Tinted<S> {
    fn name(&self) -> &str {
        self.0.name()
    }
    fn theme(&self) -> &str {
        self.0.theme()
    }
    fn describe(&self) -> &str {
        self.0.describe()
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        self.0.render(grid, ctx)?;
        grid.enable_color_support();
        let (w, h) = grid.dimensions();
        for y in 0..h {
            for x in 0..w {
                let ch = grid.get_char(x, y);
                if ch != '\u{2800}' && ch != ' ' {
                    let t = (x as f32 / w.max(1) as f32 + ctx.time * 0.05).fract();
                    let tri = 1.0 - (2.0 * t - 1.0).abs();
                    let _ = grid.set_cell_color(x, y, sample_tint(tri));
                }
            }
        }
        Ok(())
    }
}

/// All styles in the `platonic` theme.
///
/// Returns one `Box<dyn ProgressStyle>` per solid, in the order they appear in
/// the source: tetrahedron, cube, octahedron, dodecahedron, icosahedron,
/// merkaba, star-octangulum, cuboctahedron, nested-solids,
/// stellated-dodecahedron, unfolding-net.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Tinted(Tetrahedron)),
        Box::new(Tinted(Cube)),
        Box::new(Tinted(Octahedron)),
        Box::new(Tinted(Dodecahedron)),
        Box::new(Tinted(Icosahedron)),
        Box::new(Tinted(Merkaba)),
        Box::new(Tinted(StarOctangulum)),
        Box::new(Tinted(Cuboctahedron)),
        Box::new(Tinted(NestedSolids)),
        Box::new(Tinted(StellatedDodecahedron)),
        Box::new(Tinted(UnfoldingNet)),
    ]
}
