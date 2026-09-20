//! Landscape: displaced ground patches (with PATH and WATER bands) and trees.
//!
//! The ground is deliberately low-frequency — broad swells, no fine bumpiness.
//! At braille-dot scale a noisy heightfield turns into dither mush, whereas a
//! few slow rolls give the silhouette of a hill against the sky and a place for
//! buildings to sit. Trees are chunky cone/prism stacks for the same reason:
//! a foliage blob must survive as a mass, not as speckle.

use super::super::math::v3;
use super::super::mesh::{Mesh, mat};
use super::prim;
use super::rng::{Rng, fbm2, sub_seed};

/// Which way a [`Strip`] runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Axis {
    /// Runs along +x; its width is measured in y.
    X,
    /// Runs along +y; its width is measured in x.
    Y,
}

/// A straight band across the patch — a road or a river.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Strip {
    pub axis: Axis,
    /// Position on the perpendicular axis.
    pub centre: f32,
    /// Half-width in tiles.
    pub half: f32,
}

impl Strip {
    pub(crate) fn along_x(centre: f32, half: f32) -> Strip {
        Strip {
            axis: Axis::X,
            centre,
            half,
        }
    }
    pub(crate) fn along_y(centre: f32, half: f32) -> Strip {
        Strip {
            axis: Axis::Y,
            centre,
            half,
        }
    }
    fn dist(&self, x: f32, y: f32) -> f32 {
        match self.axis {
            Axis::X => (y - self.centre).abs(),
            Axis::Y => (x - self.centre).abs(),
        }
    }
}

/// Terrain shaping knobs. `Default` is a plain grassy patch.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TerrainOpts {
    /// Target grid cell size in tiles. The grid is clamped to 20 × 20 cells so
    /// a patch can never blow the triangle budget (max 800 tris).
    pub cell: f32,
    /// Peak height of the ground swell, in tiles.
    pub relief: f32,
    /// Optional flattened PATH band.
    pub path: Option<Strip>,
    /// Optional WATER band; the bed is cut to `water_depth` below z = 0.
    pub river: Option<Strip>,
    pub water_depth: f32,
}

impl Default for TerrainOpts {
    fn default() -> TerrainOpts {
        TerrainOpts {
            cell: 2.5,
            relief: 0.34,
            path: None,
            river: None,
            water_depth: 0.30,
        }
    }
}

const MAX_CELLS: usize = 20;

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    if e1 <= e0 {
        return if x < e0 { 0.0 } else { 1.0 };
    }
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Ground height at a point. Grass is always >= 0; only a river bed dips below,
/// and those cells are tagged WATER.
fn vertex_z(seed: u64, x: f32, y: f32, o: &TerrainOpts) -> f32 {
    let mut z = o.relief.max(0.0) * fbm2(seed, x * 0.11, y * 0.11);
    if let Some(p) = o.path {
        let t = smoothstep(p.half, p.half + 1.6, p.dist(x, y));
        z *= 0.25 + 0.75 * t;
    }
    if let Some(rv) = o.river {
        let t = smoothstep(rv.half, rv.half + 1.8, rv.dist(x, y));
        let bed = -o.water_depth.max(0.0);
        z = bed + (z - bed) * t;
    }
    z
}

/// Ground height under a point for a patch built with the same `seed`/`o` —
/// use it to sit a building *on* the terrain instead of floating it at z = 0.
pub(crate) fn ground_z(seed: u64, x: f32, y: f32, o: &TerrainOpts) -> f32 {
    vertex_z(sub_seed(seed, 0x4C_41_4E_44), x, y, o)
}

/// Gently displaced ground grid, `w` × `d` tiles, centred on the origin, with
/// the mean surface at z = 0. **Footprint `w` × `d`**, height within
/// `[-water_depth, relief]`. Default options: plain GRASS, no road, no water.
pub(crate) fn terrain_patch(w: f32, d: f32, seed: u64) -> Mesh {
    terrain_patch_with(w, d, seed, &TerrainOpts::default())
}

/// Full-control terrain patch. Cells whose corners dip below z = 0 are tagged
/// WATER (so the wet margin of a river reads as water, and nothing but water
/// ever sits below ground); cells centred inside `path` are tagged PATH; the
/// rest are GRASS.
pub(crate) fn terrain_patch_with(w: f32, d: f32, seed: u64, o: &TerrainOpts) -> Mesh {
    let mut m = Mesh::new();
    let w = w.max(1.0);
    let d = d.max(1.0);
    let cell = o.cell.max(0.5);
    let gx = ((w / cell).round() as i64).clamp(2, MAX_CELLS as i64) as usize;
    let gy = ((d / cell).round() as i64).clamp(2, MAX_CELLS as i64) as usize;
    let seed = sub_seed(seed, 0x4C_41_4E_44); // "LAND"

    let px = |i: usize| -> f32 { -w * 0.5 + w * (i as f32) / gx as f32 };
    let py = |j: usize| -> f32 { -d * 0.5 + d * (j as f32) / gy as f32 };

    // A road narrower than one grid cell cannot be drawn at all, and a road
    // that vanishes at coarse resolutions is worse than a slightly fat one —
    // so widen the PATH test to at least half a cell on the perpendicular axis.
    let path_reach = o.path.map(|p| {
        let cell_perp = match p.axis {
            Axis::X => d / gy as f32,
            Axis::Y => w / gx as f32,
        };
        p.half.max(cell_perp * 0.5)
    });

    for j in 0..gy {
        for i in 0..gx {
            let (x0, x1) = (px(i), px(i + 1));
            let (y0, y1) = (py(j), py(j + 1));
            let a = v3(x0, y0, vertex_z(seed, x0, y0, o));
            let b = v3(x1, y0, vertex_z(seed, x1, y0, o));
            let c = v3(x1, y1, vertex_z(seed, x1, y1, o));
            let e = v3(x0, y1, vertex_z(seed, x0, y1, o));

            let wet = a.z < -0.01 || b.z < -0.01 || c.z < -0.01 || e.z < -0.01;
            let cxm = (x0 + x1) * 0.5;
            let cym = (y0 + y1) * 0.5;
            let on_path = match (o.path, path_reach) {
                (Some(p), Some(reach)) => p.dist(cxm, cym) <= reach,
                _ => false,
            };
            let mat_id = if wet {
                mat::WATER
            } else if on_path {
                mat::PATH
            } else {
                mat::GRASS
            };
            prim::quad(&mut m, a, b, c, e, mat_id);
        }
    }
    m
}

/// Tree. **Footprint ≤ 2.6 × 2.6 tiles**, **height 2.9–4.3 tiles**, centred on
/// the origin with the root at z = 0. Two seeded kinds: a conifer (a stack of
/// three cones) and a broadleaf (an inverted cone into a barrel into a cap).
/// Both are built as few, large masses — a tree that is not a solid blob
/// disappears into the dither.
pub(crate) fn tree(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x54_52_45_45)); // "TREE"
    let mut m = Mesh::new();
    let rot = r.range(0.0, std::f32::consts::TAU);

    if r.chance(0.5) {
        // Conifer.
        prim::prism(&mut m, 0.0, 0.0, 0.0, 0.95, 0.15, 0.10, 6, rot, mat::TRUNK);
        prim::cone(&mut m, 0.0, 0.0, 0.65, 2.05, 0.95, 8, rot, mat::FOLIAGE);
        prim::cone(
            &mut m,
            0.0,
            0.0,
            1.45,
            2.75,
            0.72,
            8,
            rot + 0.4,
            mat::FOLIAGE,
        );
        prim::cone(
            &mut m,
            0.0,
            0.0,
            2.25,
            3.55,
            0.48,
            8,
            rot + 0.8,
            mat::FOLIAGE,
        );
    } else {
        // Broadleaf.
        prim::prism(&mut m, 0.0, 0.0, 0.0, 1.25, 0.20, 0.15, 6, rot, mat::TRUNK);
        prim::prism(
            &mut m,
            0.0,
            0.0,
            1.00,
            1.90,
            0.30,
            1.05,
            8,
            rot,
            mat::FOLIAGE,
        );
        prim::prism(
            &mut m,
            0.0,
            0.0,
            1.90,
            2.60,
            1.05,
            0.80,
            8,
            rot,
            mat::FOLIAGE,
        );
        prim::cone(&mut m, 0.0, 0.0, 2.60, 3.50, 0.80, 8, rot, mat::FOLIAGE);
    }

    let s = r.range(0.84, 1.18);
    m.scaled(s)
}
