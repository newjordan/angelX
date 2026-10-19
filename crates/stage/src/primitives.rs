//! The primitive shapes (docs/WORLD-3D.md): what a world's blockout is built
//! from once the plain cube runs out — towers, roofs, ramps, arches, stairs,
//! rocks and bushes. Each is sized like the others: a unit across, centred on
//! the origin, +Y up, outward normals, UVs with `(0, 0` at the top left and
//! `v` running down, counter-clockwise winding seen from outside
//! (crates/stage/src/scene.rs, `MeshData`). Integer parameters only, so a
//! [`MeshRef`](crate::scene::MeshRef) stays `Copy + Eq`.

use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

use crate::scene::MeshData;

/// The sides a round primitive takes (a cylinder's, a frustum's, an arch's
/// curve), clamped the way a sphere's longitudes are.
pub const RING_SEGMENTS: std::ops::RangeInclusive<u16> = 3..=64;
/// The steps a staircase takes.
pub const STAIR_STEPS: std::ops::RangeInclusive<u16> = 2..=16;
/// The posts a fence takes.
pub const FENCE_POSTS: std::ops::RangeInclusive<u16> = 2..=8;

/// `v` made a unit vector (its length, for a vector that is not zero).
fn norm(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] / len, v[1] / len, v[2] / len]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// A quad as four corners in the order a viewer outside it sees top left,
/// bottom left, bottom right, top right (the picture upright), drawn
/// counter-clockwise from there.
fn quad(mesh: &mut MeshData, corners: [[f32; 3]; 4], normal: [f32; 3]) {
    let base = mesh.positions.len() as u32;
    mesh.positions.extend(corners);
    mesh.normals.extend([normal; 4]);
    mesh.uvs.extend([[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]]);
    mesh.colors.extend([[1.0; 4]; 4]);
    // Counter-clockwise from the front: top left, bottom left, bottom right;
    // then top left, bottom right, top right.
    mesh.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// A triangle, counter-clockwise from outside.
fn tri(mesh: &mut MeshData, corners: [[f32; 3]; 3], normal: [f32; 3]) {
    let base = mesh.positions.len() as u32;
    mesh.positions.extend(corners);
    mesh.normals.extend([normal; 3]);
    mesh.uvs.extend([[0.0, 1.0], [1.0, 1.0], [0.5, 0.0]]);
    mesh.colors.extend([[1.0; 4]; 3]);
    mesh.indices.extend([base, base + 1, base + 2]);
}

/// A box (an axis-aligned one) as six quads, in the order of
/// `scene::CUBE_FACES` without the groups: a fence's posts and rails.
fn box_at(mesh: &mut MeshData, centre: [f32; 3], half: [f32; 3]) {
    let [cx, cy, cz] = centre;
    let [hx, hy, hz] = half;
    quad(mesh, [[cx + hx, cy + hy, cz + hz], [cx + hx, cy - hy, cz + hz], [cx + hx, cy - hy, cz - hz], [cx + hx, cy + hy, cz - hz]], [1.0, 0.0, 0.0]);
    quad(mesh, [[cx - hx, cy + hy, cz - hz], [cx - hx, cy - hy, cz - hz], [cx - hx, cy - hy, cz + hz], [cx - hx, cy + hy, cz + hz]], [-1.0, 0.0, 0.0]);
    quad(mesh, [[cx - hx, cy + hy, cz - hz], [cx - hx, cy + hy, cz + hz], [cx + hx, cy + hy, cz + hz], [cx + hx, cy + hy, cz - hz]], [0.0, 1.0, 0.0]);
    quad(mesh, [[cx - hx, cy - hy, cz + hz], [cx - hx, cy - hy, cz - hz], [cx + hx, cy - hy, cz - hz], [cx + hx, cy - hy, cz + hz]], [0.0, -1.0, 0.0]);
    quad(mesh, [[cx - hx, cy + hy, cz + hz], [cx - hx, cy - hy, cz + hz], [cx + hx, cy - hy, cz + hz], [cx + hx, cy + hy, cz + hz]], [0.0, 0.0, 1.0]);
    quad(mesh, [[cx + hx, cy + hy, cz - hz], [cx + hx, cy - hy, cz - hz], [cx - hx, cy - hy, cz - hz], [cx - hx, cy + hy, cz - hz]], [0.0, 0.0, -1.0]);
}

/// A number in 0..1 from a seed and an index, always the same one: the
/// lumps of a rock, the puffs of a blob.
fn hash01(seed: u16, index: u32) -> f32 {
    let mut h = u32::from(seed).wrapping_mul(0x9E37_79B1) ^ index.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 16;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

impl MeshData {
    /// A cylinder of diameter 1 and height 1, centred: flat caps, smooth
    /// sides (a column, a tower, a log).
    pub fn cylinder(segments: u16) -> Self {
        Self::tube(segments, 0.5, 0.5)
    }

    /// A cylinder whose top diameter is 0.6 (a tower's top, a tree trunk, a
    /// chimney pot).
    pub fn frustum(segments: u16) -> Self {
        Self::tube(segments, 0.5, 0.3)
    }

    /// The round part the cylinder and the frustum share: `segments` sides
    /// (clamped to [`RING_SEGMENTS`]) between a bottom ring of one radius and
    /// a top ring of another, with a flat cap on each.
    fn tube(segments: u16, bottom: f32, top: f32) -> Self {
        let n = u32::from(segments.clamp(*RING_SEGMENTS.start(), *RING_SEGMENTS.end()));
        let mut mesh = Self::default();
        let ring = |i: u32| {
            let a = i as f32 / n as f32 * TAU;
            a.sin_cos()
        };
        // The sides: the top ring then the bottom, and the quads between. A
        // frustum narrows upward, which tilts the side normal by its slant.
        let slant = bottom - top;
        let k = 1.0 / (1.0 + slant * slant).sqrt();
        for j in 0..=1u32 {
            let (y, r) = if j == 0 { (0.5, top) } else { (-0.5, bottom) };
            for i in 0..=n {
                let (s, c) = ring(i);
                mesh.positions.push([r * s, y, r * c]);
                mesh.normals.push([s * k, slant * k, c * k]);
                mesh.uvs.push([i as f32 / n as f32, j as f32]);
                mesh.colors.push([1.0; 4]);
            }
        }
        let at = |i: u32, j: u32| j * (n + 1) + i;
        for i in 0..n {
            let (tl, tr, bl, br) = (at(i, 0), at(i + 1, 0), at(i, 1), at(i + 1, 1));
            mesh.indices.extend([tl, bl, br]);
            mesh.indices.extend([tl, br, tr]);
        }
        // A cap on the top (a fan facing up) and one on the bottom (down).
        for (y, r, up) in [(0.5, top, true), (-0.5, bottom, false)] {
            let facing = [0.0, if up { 1.0 } else { -1.0 }, 0.0];
            let middle = mesh.positions.len() as u32;
            mesh.positions.push([0.0, y, 0.0]);
            mesh.normals.push(facing);
            mesh.uvs.push([0.5, 0.5]);
            mesh.colors.push([1.0; 4]);
            for i in 0..=n {
                let (s, c) = ring(i);
                mesh.positions.push([r * s, y, r * c]);
                mesh.normals.push(facing);
                mesh.uvs.push([0.5 + 0.5 * s, 0.5 + 0.5 * c]);
                mesh.colors.push([1.0; 4]);
            }
            for i in 0..n {
                if up {
                    mesh.indices.extend([middle, middle + 1 + i, middle + 2 + i]);
                } else {
                    mesh.indices.extend([middle, middle + 2 + i, middle + 1 + i]);
                }
            }
        }
        mesh
    }

    /// A ramp: the bottom is the unit square, the back (-Z) is full height,
    /// and it slopes down to nothing at +Z.
    pub fn wedge() -> Self {
        let mut mesh = Self::default();
        // The bottom, the back and the slope.
        quad(&mut mesh, [[-0.5, -0.5, 0.5], [-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5]], [0.0, -1.0, 0.0]);
        quad(&mut mesh, [[0.5, 0.5, -0.5], [0.5, -0.5, -0.5], [-0.5, -0.5, -0.5], [-0.5, 0.5, -0.5]], [0.0, 0.0, -1.0]);
        quad(&mut mesh, [[-0.5, 0.5, -0.5], [-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [0.5, 0.5, -0.5]], norm([0.0, 1.0, 1.0]));
        // The two sides: right triangles.
        tri(&mut mesh, [[-0.5, -0.5, -0.5], [-0.5, -0.5, 0.5], [-0.5, 0.5, -0.5]], [-1.0, 0.0, 0.0]);
        tri(&mut mesh, [[0.5, -0.5, -0.5], [0.5, 0.5, -0.5], [0.5, -0.5, 0.5]], [1.0, 0.0, 0.0]);
        mesh
    }

    /// A pitched roof prism: the ridge runs along X at the top centre, the
    /// two slopes come down to the bottom edges at -Z and +Z, and the gable
    /// ends at -X and +X are triangles.
    pub fn gable() -> Self {
        let mut mesh = Self::default();
        quad(&mut mesh, [[-0.5, 0.5, 0.0], [-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [0.5, 0.5, 0.0]], norm([0.0, 1.0, 2.0]));
        quad(&mut mesh, [[0.5, 0.5, 0.0], [0.5, -0.5, -0.5], [-0.5, -0.5, -0.5], [-0.5, 0.5, 0.0]], norm([0.0, 1.0, -2.0]));
        tri(&mut mesh, [[-0.5, -0.5, -0.5], [-0.5, -0.5, 0.5], [-0.5, 0.5, 0.0]], [-1.0, 0.0, 0.0]);
        tri(&mut mesh, [[0.5, -0.5, -0.5], [0.5, 0.5, 0.0], [0.5, -0.5, 0.5]], [1.0, 0.0, 0.0]);
        quad(&mut mesh, [[-0.5, -0.5, 0.5], [-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5]], [0.0, -1.0, 0.0]);
        mesh
    }

    /// A hipped roof: a ridge along X from x = -0.25 to +0.25 at the top,
    /// four sloped faces down to the unit square.
    pub fn hip() -> Self {
        let mut mesh = Self::default();
        quad(&mut mesh, [[-0.25, 0.5, 0.0], [-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [0.25, 0.5, 0.0]], norm([0.0, 1.0, 2.0]));
        quad(&mut mesh, [[0.25, 0.5, 0.0], [0.5, -0.5, -0.5], [-0.5, -0.5, -0.5], [-0.25, 0.5, 0.0]], norm([0.0, 1.0, -2.0]));
        tri(&mut mesh, [[0.25, 0.5, 0.0], [0.5, -0.5, 0.5], [0.5, -0.5, -0.5]], norm([4.0, 1.0, 0.0]));
        tri(&mut mesh, [[-0.25, 0.5, 0.0], [-0.5, -0.5, -0.5], [-0.5, -0.5, 0.5]], norm([-4.0, 1.0, 0.0]));
        quad(&mut mesh, [[-0.5, -0.5, 0.5], [-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5]], [0.0, -1.0, 0.0]);
        mesh
    }

    /// A square-based spire to a point at the top centre.
    pub fn pyramid() -> Self {
        let mut mesh = Self::default();
        tri(&mut mesh, [[-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [0.0, 0.5, 0.0]], norm([0.0, 1.0, 2.0]));
        tri(&mut mesh, [[0.5, -0.5, -0.5], [-0.5, -0.5, -0.5], [0.0, 0.5, 0.0]], norm([0.0, 1.0, -2.0]));
        tri(&mut mesh, [[0.5, -0.5, 0.5], [0.5, -0.5, -0.5], [0.0, 0.5, 0.0]], norm([2.0, 1.0, 0.0]));
        tri(&mut mesh, [[-0.5, -0.5, -0.5], [-0.5, -0.5, 0.5], [0.0, 0.5, 0.0]], norm([-2.0, 1.0, 0.0]));
        quad(&mut mesh, [[-0.5, -0.5, 0.5], [-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5]], [0.0, -1.0, 0.0]);
        mesh
    }

    /// A wall block with a round-topped opening through it along Z (a
    /// doorway, a window): the opening is 0.6 wide, its straight sides rise
    /// to y = 0 (the middle) and a half circle closes it to y = +0.3. The
    /// opening's inner faces are part of the mesh, facing into it.
    /// `segments` is the half circle's sides (clamped to [`RING_SEGMENTS`]).
    pub fn arch(segments: u16) -> Self {
        let n = u32::from(segments.clamp(*RING_SEGMENTS.start(), *RING_SEGMENTS.end()));
        let mut mesh = Self::default();
        // The half circle, left to right over the top of the opening.
        let arc = |i: u32| {
            let a = PI - i as f32 / n as f32 * PI;
            (0.3 * a.cos(), 0.3 * a.sin())
        };
        // The two flat faces of the wall (at z = ±0.5): the square with the
        // opening taken out — a column each side, and the bell over the
        // opening between the arc and the top edge.
        for front in [true, false] {
            let z = if front { 0.5 } else { -0.5 };
            let facing = [0.0, 0.0, if front { 1.0 } else { -1.0 }];
            let p = |x: f32, y: f32| [x, y, z];
            let face = |mesh: &mut MeshData, corners: [[f32; 3]; 4]| {
                if front {
                    quad(mesh, corners, facing);
                } else {
                    // The same quad seen from behind: each triangle reversed.
                    let [a, b, c, d] = corners;
                    quad(mesh, [c, b, a, d], facing);
                }
            };
            face(&mut mesh, [p(-0.5, 0.5), p(-0.5, -0.5), p(-0.3, -0.5), p(-0.3, 0.5)]);
            face(&mut mesh, [p(0.3, 0.5), p(0.3, -0.5), p(0.5, -0.5), p(0.5, 0.5)]);
            // The bell: over each arc segment, the whole piece between the
            // segment and the top edge — a trapezoid of two triangles, so
            // no part of it is left open however steep or shallow the arc
            // runs. The first segment starts at (-0.3, 0) and the last ends
            // at (0.3, 0), so the bell meets the columns edge to edge.
            for i in 0..n {
                let (x0, y0) = arc(i);
                let (x1, y1) = arc(i + 1);
                if front {
                    tri(&mut mesh, [p(x0, 0.5), p(x0, y0), p(x1, y1)], facing);
                    tri(&mut mesh, [p(x0, 0.5), p(x1, y1), p(x1, 0.5)], facing);
                } else {
                    tri(&mut mesh, [p(x0, 0.5), p(x1, y1), p(x0, y0)], facing);
                    tri(&mut mesh, [p(x0, 0.5), p(x1, 0.5), p(x1, y1)], facing);
                }
            }
        }
        // The opening's straight sides, facing into it.
        quad(&mut mesh, [[-0.3, 0.0, 0.5], [-0.3, -0.5, 0.5], [-0.3, -0.5, -0.5], [-0.3, 0.0, -0.5]], [1.0, 0.0, 0.0]);
        quad(&mut mesh, [[0.3, 0.0, -0.5], [0.3, -0.5, -0.5], [0.3, -0.5, 0.5], [0.3, 0.0, 0.5]], [-1.0, 0.0, 0.0]);
        // The half cylinder over it, its faces looking down into the opening.
        for i in 0..n {
            let (x0, y0) = arc(i);
            let (x1, y1) = arc(i + 1);
            let inward = norm([-(x0 + x1), -(y0 + y1), 0.0]);
            let tl = [x0, y0, -0.5];
            let tr = [x1, y1, -0.5];
            let bl = [x0, y0, 0.5];
            let br = [x1, y1, 0.5];
            let base = mesh.positions.len() as u32;
            mesh.positions.extend([tl, bl, br, tr]);
            mesh.normals.extend([inward; 4]);
            mesh.uvs.extend([[i as f32 / n as f32, 0.0], [i as f32 / n as f32, 1.0], [(i + 1) as f32 / n as f32, 1.0], [(i + 1) as f32 / n as f32, 0.0]]);
            mesh.colors.extend([[1.0; 4]; 4]);
            // Into the opening, so the winding runs the other way from a
            // cylinder's side.
            mesh.indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
        }
        // The rest of the block: the top, the bottom and the two ends.
        quad(&mut mesh, [[-0.5, 0.5, -0.5], [-0.5, 0.5, 0.5], [0.5, 0.5, 0.5], [0.5, 0.5, -0.5]], [0.0, 1.0, 0.0]);
        quad(&mut mesh, [[-0.5, -0.5, 0.5], [-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5]], [0.0, -1.0, 0.0]);
        quad(&mut mesh, [[0.5, 0.5, 0.5], [0.5, -0.5, 0.5], [0.5, -0.5, -0.5], [0.5, 0.5, -0.5]], [1.0, 0.0, 0.0]);
        quad(&mut mesh, [[-0.5, 0.5, -0.5], [-0.5, -0.5, -0.5], [-0.5, -0.5, 0.5], [-0.5, 0.5, 0.5]], [-1.0, 0.0, 0.0]);
        mesh
    }

    /// A staircase of `steps` (clamped to [`STAIR_STEPS`]) rising toward -Z,
    /// filling the unit cube from bottom-front to top-back: a ramp a figure
    /// can walk up.
    pub fn stairs(steps: u16) -> Self {
        let n = u32::from(steps.clamp(*STAIR_STEPS.start(), *STAIR_STEPS.end()));
        let mut mesh = Self::default();
        let depth = 1.0 / n as f32;
        let rise = 1.0 / n as f32;
        // The tread and the riser of each step, then the back, the bottom
        // and the two sides (the staircase's own profile).
        for i in 0..n {
            let z_front = 0.5 - i as f32 * depth;
            let z_back = z_front - depth;
            let top = -0.5 + (i + 1) as f32 * rise;
            let below = -0.5 + i as f32 * rise;
            // The tread, on top.
            quad(&mut mesh, [[-0.5, top, z_back], [-0.5, top, z_front], [0.5, top, z_front], [0.5, top, z_back]], [0.0, 1.0, 0.0]);
            // The riser, facing +Z (the staircase rises toward -Z).
            quad(&mut mesh, [[-0.5, top, z_front], [-0.5, below, z_front], [0.5, below, z_front], [0.5, top, z_front]], [0.0, 0.0, 1.0]);
            // The sides: the profile under this step.
            quad(&mut mesh, [[0.5, top, z_front], [0.5, -0.5, z_front], [0.5, -0.5, z_back], [0.5, top, z_back]], [1.0, 0.0, 0.0]);
            quad(&mut mesh, [[-0.5, top, z_back], [-0.5, -0.5, z_back], [-0.5, -0.5, z_front], [-0.5, top, z_front]], [-1.0, 0.0, 0.0]);
        }
        // The back of the last step and the ground under it all.
        quad(&mut mesh, [[0.5, 0.5, -0.5], [0.5, -0.5, -0.5], [-0.5, -0.5, -0.5], [-0.5, 0.5, -0.5]], [0.0, 0.0, -1.0]);
        quad(&mut mesh, [[-0.5, -0.5, 0.5], [-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5]], [0.0, -1.0, 0.0]);
        mesh
    }

    /// A lumpy boulder (a rock, a cliff): an icosphere with each of its
    /// points pushed in or out by a hash of the seed and its index (a radius
    /// between 0.38 and 0.5), flat shaded, sitting on y = -0.5.
    pub fn rock(seed: u16) -> Self {
        // The icosahedron, subdivided once: 42 points, 80 faces.
        let t = (1.0 + 5.0f32.sqrt()) * 0.5;
        let mut points: Vec<[f32; 3]> = [
            [-1.0, t, 0.0], [1.0, t, 0.0], [-1.0, -t, 0.0], [1.0, -t, 0.0],
            [0.0, -1.0, t], [0.0, 1.0, t], [0.0, -1.0, -t], [0.0, 1.0, -t],
            [t, 0.0, -1.0], [t, 0.0, 1.0], [-t, 0.0, -1.0], [-t, 0.0, 1.0],
        ]
        .iter()
        .map(|&p| norm(p))
        .collect();
        let mut faces: Vec<[u32; 3]> = [
            [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
            [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
            [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
            [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
        ]
        .to_vec();
        {
            let mut midpoints = HashMap::new();
            let midpoint = |points: &mut Vec<[f32; 3]>, midpoints: &mut HashMap<(u32, u32), u32>, a: u32, b: u32| -> u32 {
                let key = (a.min(b), a.max(b));
                if let Some(&m) = midpoints.get(&key) {
                    return m;
                }
                let m = points.len() as u32;
                points.push(norm([(points[a as usize][0] + points[b as usize][0]) * 0.5, (points[a as usize][1] + points[b as usize][1]) * 0.5, (points[a as usize][2] + points[b as usize][2]) * 0.5]));
                midpoints.insert(key, m);
                m
            };
            let mut divided = Vec::with_capacity(faces.len() * 4);
            for [a, b, c] in faces {
                let (ab, bc, ca) = (
                    midpoint(&mut points, &mut midpoints, a, b),
                    midpoint(&mut points, &mut midpoints, b, c),
                    midpoint(&mut points, &mut midpoints, c, a),
                );
                divided.extend([[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
            }
            faces = divided;
        }
        // Each point pushed in or out by its own hash of the seed.
        let radii: Vec<f32> = (0..points.len()).map(|i| 0.38 + 0.12 * hash01(seed, i as u32)).collect();
        let at = |i: u32| [points[i as usize][0] * radii[i as usize], points[i as usize][1] * radii[i as usize], points[i as usize][2] * radii[i as usize]];
        // It sits on y = -0.5.
        let lift = -0.5 - (0..points.len()).map(|i| at(i as u32)[1]).fold(f32::INFINITY, f32::min);
        let mut mesh = Self::default();
        // Flat shaded: three points a face, the face's own normal.
        for [a, b, c] in faces {
            let (pa, pb, pc) = (at(a), at(b), at(c));
            let normal = norm(cross(sub(pb, pa), sub(pc, pa)));
            let base = mesh.positions.len() as u32;
            mesh.positions.extend([[pa[0], pa[1] + lift, pa[2]], [pb[0], pb[1] + lift, pb[2]], [pc[0], pc[1] + lift, pc[2]]]);
            mesh.normals.extend([normal; 3]);
            mesh.uvs.extend([[0.0, 1.0], [1.0, 1.0], [0.5, 0.0]]);
            mesh.colors.extend([[1.0; 4]; 3]);
            mesh.indices.extend([base, base + 1, base + 2]);
        }
        mesh
    }

    /// A soft clump of foliage (a bush, a tree's canopy): a few overlapping
    /// squashed spheres placed by the seed, merged into one mesh and staying
    /// inside the unit cube.
    pub fn blob(seed: u16) -> Self {
        let count = 3 + (hash01(seed, 1000) * 3.0) as u32;
        let mut mesh = Self::default();
        for j in 0..count {
            let centre = [
                (hash01(seed, j * 11 + 1) - 0.5) * 0.28,
                (hash01(seed, j * 11 + 2) - 0.5) * 0.20,
                (hash01(seed, j * 11 + 3) - 0.5) * 0.28,
            ];
            let radii = [
                0.24 + 0.10 * hash01(seed, j * 11 + 4),
                0.20 + 0.10 * hash01(seed, j * 11 + 5),
                0.24 + 0.10 * hash01(seed, j * 11 + 6),
            ];
            ellipsoid(&mut mesh, centre, radii);
        }
        mesh
    }

    /// A fence of `posts` (clamped to [`FENCE_POSTS`]) evenly along X with
    /// two rails between them, all inside the unit cube and thin in Z (0.1).
    pub fn fence(posts: u16) -> Self {
        let n = u32::from(posts.clamp(*FENCE_POSTS.start(), *FENCE_POSTS.end()));
        let mut mesh = Self::default();
        let width = 0.6 / n as f32;
        for i in 0..n {
            let x = -0.5 + (i as f32 + 0.5) / n as f32;
            box_at(&mut mesh, [x, 0.0, 0.0], [width * 0.5, 0.5, 0.05]);
        }
        for y in [0.2, -0.2] {
            box_at(&mut mesh, [0.0, y, 0.0], [0.5, 0.06, 0.05]);
        }
        mesh
    }
}

/// A squashed sphere (an ellipsoid) about `centre`, one blob's puff: the
/// picture wraps it as a sphere's does.
fn ellipsoid(mesh: &mut MeshData, centre: [f32; 3], radii: [f32; 3]) {
    let (cols, rows) = (10u32, 5u32);
    let first = mesh.positions.len() as u32;
    for j in 0..=rows {
        let v = j as f32 / rows as f32;
        let (sin_t, cos_t) = (v * PI).sin_cos();
        for i in 0..=cols {
            let u = i as f32 / cols as f32;
            let (sin_p, cos_p) = (u * TAU).sin_cos();
            let dir = [sin_t * sin_p, cos_t, sin_t * cos_p];
            mesh.positions.push([centre[0] + dir[0] * radii[0], centre[1] + dir[1] * radii[1], centre[2] + dir[2] * radii[2]]);
            mesh.normals.push(norm([dir[0] / radii[0], dir[1] / radii[1], dir[2] / radii[2]]));
            mesh.uvs.push([u, v]);
            mesh.colors.push([1.0; 4]);
        }
    }
    let at = |i: u32, j: u32| first + j * (cols + 1) + i;
    for j in 0..rows {
        for i in 0..cols {
            let (tl, tr, bl, br) = (at(i, j), at(i + 1, j), at(i, j + 1), at(i + 1, j + 1));
            if j + 1 < rows {
                mesh.indices.extend([tl, bl, br]);
            }
            if j > 0 {
                mesh.indices.extend([tl, br, tr]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Material, MeshRef, Node};
    use crate::shapes::{NAMES, by_name};
    use crate::wire::{decode, encode};
    use crate::scene::{Props, MAX_GRAIN};
    use crate::Frame;

    fn cases() -> Vec<(&'static str, MeshData)> {
        vec![
            ("cylinder 3", MeshData::cylinder(3)),
            ("cylinder 12", MeshData::cylinder(12)),
            ("cylinder 200", MeshData::cylinder(200)),
            ("frustum 3", MeshData::frustum(3)),
            ("frustum 12", MeshData::frustum(12)),
            ("wedge", MeshData::wedge()),
            ("gable", MeshData::gable()),
            ("hip", MeshData::hip()),
            ("pyramid", MeshData::pyramid()),
            ("arch 3", MeshData::arch(3)),
            ("arch 8", MeshData::arch(8)),
            ("stairs 2", MeshData::stairs(2)),
            ("stairs 4", MeshData::stairs(4)),
            ("stairs 16", MeshData::stairs(16)),
            ("stairs 99", MeshData::stairs(99)),
            ("rock 1", MeshData::rock(1)),
            ("rock 42", MeshData::rock(42)),
            ("blob 0", MeshData::blob(0)),
            ("blob 9", MeshData::blob(9)),
            ("fence 2", MeshData::fence(2)),
            ("fence 5", MeshData::fence(5)),
            ("fence 8", MeshData::fence(8)),
        ]
    }

    #[test]
    fn every_primitive_is_valid_inside_the_unit_cube_with_unit_normals() {
        for (name, data) in cases() {
            assert_eq!(data.validate(), Ok(()), "{name}");
            assert_eq!(data.normals.len(), data.positions.len(), "{name}");
            assert_eq!(data.uvs.len(), data.positions.len(), "{name}");
            for p in &data.positions {
                for v in p {
                    assert!(v.abs() <= 0.5 + 1e-4, "{name}: {v} is outside the unit cube");
                }
            }
            for n in &data.normals {
                let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                assert!((len - 1.0).abs() < 1e-4, "{name}: a normal {len} long");
            }
        }
    }

    #[test]
    fn the_closed_primitives_face_outward() {
        for (name, data) in cases() {
            // The fence is a frame and the blob a few clumps; the arch keeps
            // a doorway, whose inner faces look into it on purpose.
            if name.starts_with("fence") || name.starts_with("blob") || name.starts_with("arch") {
                continue;
            }
            let centre = data.centre();
            let mut triangles = 0;
            let mut clearly = 0;
            for t in data.indices.chunks(3) {
                let (a, b, c) = (data.positions[t[0] as usize], data.positions[t[1] as usize], data.positions[t[2] as usize]);
                let n = norm(cross(sub(b, a), sub(c, a)));
                let centroid = [(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0, (a[2] + b[2] + c[2]) / 3.0];
                let d = n[0] * (centroid[0] - centre[0]) + n[1] * (centroid[1] - centre[1]) + n[2] * (centroid[2] - centre[2]);
                // A staircase is concave: a low tread honestly faces up from
                // below the middle of the block, so the dot says nothing
                // there (its own test checks each face sees the sky); every
                // other shape must face out of the middle.
                if !name.starts_with("stairs") {
                    assert!(d > -0.05, "{name}: a triangle faces inward (dot {d})");
                }
                triangles += 1;
                if d > 1e-6 {
                    clearly += 1;
                }
            }
            assert!(clearly * 10 >= triangles * 7, "{name}: only {clearly} of {triangles} triangles clearly face outward");
        }
    }

    #[test]
    fn the_arch_opens_inward() {
        let data = MeshData::arch(8);
        let centre = data.centre();
        for t in data.indices.chunks(3) {
            let (a, b, c) = (data.positions[t[0] as usize], data.positions[t[1] as usize], data.positions[t[2] as usize]);
            let n = norm(cross(sub(b, a), sub(c, a)));
            let centroid = [(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0, (a[2] + b[2] + c[2]) / 3.0];
            let d = n[0] * (centroid[0] - centre[0]) + n[1] * (centroid[1] - centre[1]) + n[2] * (centroid[2] - centre[2]);
            // The opening's own faces point into it on purpose; the wall
            // around it must point out.
            let inside = [a, b, c].iter().all(|p| p[0].abs() <= 0.3 + 1e-3 && p[1] <= 0.3 + 1e-3);
            if !inside {
                assert!(d > -0.05, "a triangle of the arch is turned the wrong way (dot {d})");
            }
            // The wall's faces point out of the block; the opening's point in.
            if n[2] > 0.9 {
                assert!(centroid[2] > 0.4, "the front face is not at +Z");
            }
            if n[2] < -0.9 {
                assert!(centroid[2] < -0.4, "the back face is not at -Z");
            }
            let inside = [a, b, c].iter().all(|p| p[0].abs() <= 0.3 + 1e-3 && p[1] >= -1e-3 && p[1] <= 0.3 + 1e-3);
            if inside && n[2].abs() < 0.5 {
                // The straight sides look across, the crown down: every inner
                // face looks at the point of the opening's axis at its height.
                let axis = [0.0, centroid[1], centroid[2]];
                let toward = [axis[0] - centroid[0], axis[1] - centroid[1], axis[2] - centroid[2]];
                let dot = n[0] * toward[0] + n[1] * toward[1] + n[2] * toward[2];
                assert!(dot > 1e-4, "an inner face of the opening looks away from it");
            }
        }
    }

    /// The triangles of one flat face of the arch (the +Z one or the -Z
    /// one): every corner on the face's plane, every stored normal facing
    /// out of it. Each is checked to wind the way its normal says.
    fn arch_face(data: &MeshData, sign: f32) -> Vec<[[f32; 2]; 3]> {
        let mut out = Vec::new();
        for t in data.indices.chunks(3) {
            let corners = [data.positions[t[0] as usize], data.positions[t[1] as usize], data.positions[t[2] as usize]];
            let on_plane = corners.iter().all(|p| (p[2] - 0.5 * sign).abs() < 1e-5);
            let facing = t.iter().all(|&i| data.normals[i as usize] == [0.0, 0.0, sign]);
            if !(on_plane && facing) {
                continue;
            }
            let n = cross(sub(corners[1], corners[0]), sub(corners[2], corners[0]));
            assert!(n[2] * sign > 1e-6, "a triangle of the arch's face winds away from its normal");
            out.push(corners.map(|p| [p[0], p[1]]));
        }
        out
    }

    /// Whether a point is in the arch's opening, the half circle taken as
    /// the polyline the mesh actually draws (its `segments` chords).
    fn in_arch_opening(x: f32, y: f32, segments: u16) -> bool {
        if x.abs() >= 0.3 - 1e-4 {
            return false;
        }
        let n = u32::from(segments);
        let arc = |i: u32| {
            let a = PI - i as f32 / n as f32 * PI;
            (0.3 * a.cos(), 0.3 * a.sin())
        };
        let roof = (0..n)
            .find_map(|i| {
                let ((x0, y0), (x1, y1)) = (arc(i), arc(i + 1));
                (x >= x0 && x <= x1).then(|| y0 + (y1 - y0) * (x - x0) / (x1 - x0))
            })
            .expect("x is inside the arc's span");
        y < roof - 1e-4
    }

    #[test]
    fn the_arch_faces_are_the_square_less_the_opening() {
        let opening = 0.6 * 0.5 + PI * 0.3 * 0.3 / 2.0;
        for segments in [8u16, 16] {
            let data = MeshData::arch(segments);
            for sign in [1.0, -1.0] {
                let face = arch_face(&data, sign);
                let area: f32 = face
                    .iter()
                    .map(|t| 0.5 * ((t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1])).abs())
                    .sum();
                let want = 1.0 - opening;
                assert!(
                    (area - want).abs() <= want * 0.01,
                    "arch {segments}, face {sign}: the faces cover {area}, the square less the opening is {want}"
                );
                // Nothing of the face stands in the opening: sample each
                // triangle on a grid, its corners and edges included.
                for t in &face {
                    for i in 0..=8u32 {
                        for j in 0..=(8 - i) {
                            let (u, v) = (i as f32 / 8.0, j as f32 / 8.0);
                            let w = 1.0 - u - v;
                            let x = t[0][0] * w + t[1][0] * u + t[2][0] * v;
                            let y = t[0][1] * w + t[1][1] * u + t[2][1] * v;
                            assert!(!in_arch_opening(x, y, segments), "arch {segments}: a face triangle covers the opening at ({x}, {y})");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_arch_soffit_runs_the_whole_arc_through_the_wall() {
        for segments in [3u16, 8, 16, 64] {
            let n = u32::from(segments);
            let data = MeshData::arch(segments);
            // The soffit is every triangle whose corners all lie on the
            // half circle; each must run the wall's full depth.
            let mut area = 0.0;
            for t in data.indices.chunks(3) {
                let c = [data.positions[t[0] as usize], data.positions[t[1] as usize], data.positions[t[2] as usize]];
                if !c.iter().all(|p| p[1] >= -1e-5 && ((p[0] * p[0] + p[1] * p[1]).sqrt() - 0.3).abs() < 1e-4) {
                    continue;
                }
                let zs = c.iter().map(|p| p[2]).fold((f32::MAX, f32::MIN), |(lo, hi), z| (lo.min(z), hi.max(z)));
                assert_eq!(zs, (-0.5, 0.5), "arch {segments}: a soffit triangle does not run through the wall");
                let nrm = cross(sub(c[1], c[0]), sub(c[2], c[0]));
                area += 0.5 * (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2]).sqrt();
            }
            // The chords of the half circle, one unit deep.
            let chords: f32 = (0..n)
                .map(|i| {
                    let (a0, a1) = (PI - i as f32 / n as f32 * PI, PI - (i + 1) as f32 / n as f32 * PI);
                    ((0.3 * a0.cos() - 0.3 * a1.cos()).powi(2) + (0.3 * a0.sin() - 0.3 * a1.sin()).powi(2)).sqrt()
                })
                .sum();
            assert!((area - chords).abs() <= chords * 1e-3, "arch {segments}: the soffit is {area} but the arc is {chords} long");
        }
    }

    #[test]
    fn a_seed_repeats_and_a_different_seed_differs() {
        assert_eq!(MeshData::rock(7), MeshData::rock(7));
        assert_ne!(MeshData::rock(7), MeshData::rock(8));
        assert_eq!(MeshData::blob(7), MeshData::blob(7));
        assert_ne!(MeshData::blob(7), MeshData::blob(8));
    }

    #[test]
    fn by_name_round_trips_every_name() {
        for name in NAMES {
            let mesh = by_name(name).unwrap_or_else(|| panic!("{name} is not known"));
            let data = mesh.generate().unwrap_or_else(|| panic!("{name} is not built in"));
            assert_eq!(data.validate(), Ok(()), "{name}");
        }
        for (name, mesh) in [
            ("cylinder", MeshRef::Cylinder { segments: 12 }),
            ("frustum", MeshRef::Frustum { segments: 12 }),
            ("wedge", MeshRef::Wedge),
            ("gable", MeshRef::Gable),
            ("hip", MeshRef::Hip),
            ("pyramid", MeshRef::Pyramid),
            ("arch", MeshRef::Arch { segments: 8 }),
            ("stairs", MeshRef::Stairs { steps: 4 }),
            ("rock", MeshRef::Rock { seed: 1 }),
            ("blob", MeshRef::Blob { seed: 1 }),
            ("fence", MeshRef::Fence { posts: 4 }),
        ] {
            assert_eq!(by_name(name), Some(mesh), "{name}");
        }
    }

    #[test]
    fn the_wire_carries_every_new_variant_as_itself() {
        let meshes = [
            MeshRef::Cylinder { segments: 12 },
            MeshRef::Cylinder { segments: 200 },
            MeshRef::Frustum { segments: 3 },
            MeshRef::Wedge,
            MeshRef::Gable,
            MeshRef::Hip,
            MeshRef::Pyramid,
            MeshRef::Arch { segments: 8 },
            MeshRef::Stairs { steps: 99 },
            MeshRef::Rock { seed: 42 },
            MeshRef::Blob { seed: 9 },
            MeshRef::Fence { posts: 2 },
        ];
        for mesh in meshes {
            let node = Node::new(1, mesh, Material::default());
            let frame = Frame {
                size: (4, 4),
                props: Some(Props { at: 0, grain: 1, nodes: vec![node], world: None }),
                ..Default::default()
            };
            let mut out = Vec::new();
            encode(&frame, &[], &mut out);
            let back = decode(&out).unwrap_or_else(|refused| panic!("{mesh:?}: {refused:?}"));
            let nodes = back.frame.props.expect("the props came back").nodes;
            assert_eq!(nodes[0].mesh, Some(mesh), "{mesh:?}");
        }
    }
}
