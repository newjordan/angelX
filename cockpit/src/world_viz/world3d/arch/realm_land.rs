//! Realm land: the castle-court ground with a coherent road network painted
//! onto the terrain's *existing* triangles. The mesh is built by the same
//! `terrain_patch_with` call `court_scene` always used — same vertices,
//! same 722 triangles — and then GRASS cells along the road centrelines are
//! retagged PATH. WATER (the river bed) and the existing central approach road
//! are never touched, no triangle is added, and nothing is raised, so the
//! roads cannot float, z-fight, or drift from the camera's ground height.
//!
//! Roads run ground-level through the two four-tile postern gaps the parent
//! opens in the east/west curtain walls at (±11, -4), spanning y = -6..-2;
//! no other wall is crossed, and the only river crossing stays the existing
//! approach and bridge at (0, -22).

use super::super::mesh::{Mesh, mat};
use super::compose::{COURT_EXTENT, court_terrain};
use super::land::terrain_patch_with;

/// Half-width of a painted road, in tiles. The ground grid is 19 × 19 cells
/// (~2.95 tiles each); under ~1.5 a centreline can slip between cell centres
/// and vanish. This width keeps the authored branches connected by full
/// cell edges in the mesh test. Recheck connectivity when moving a waypoint;
/// diagonals can otherwise meet only at a corner.
const ROAD_HALF: f32 = 2.0;

/// Named road centrelines, world (x, y). Legs join at shared waypoints so the
/// whole network is one connected component rooted on the courtyard junction
/// at (0, -8), which itself sits on the pre-existing central road column.
const ROADS: &[(&str, &[(f32, f32)])] = &[
    // Inner courtyard: from the central road around the well to the posterns.
    (
        "court-east",
        &[(0.0, -8.0), (3.0, -7.0), (5.0, -4.0), (11.0, -4.0)],
    ),
    (
        "court-west",
        &[(0.0, -8.0), (-3.0, -7.0), (-5.0, -4.0), (-11.0, -4.0)],
    ),
    // West outer branch to the library front.
    (
        "west-library",
        &[(-11.0, -4.0), (-15.0, -4.0), (-20.0, -2.0), (-20.0, 1.0)],
    ),
    // East outer branch to the chapel front.
    (
        "east-chapel",
        &[(11.0, -4.0), (15.0, -4.0), (18.0, -2.0), (17.0, 3.5)],
    ),
    // East branch south to the observatory front (joins the chapel leg at (15,-4)).
    (
        "observatory",
        &[(15.0, -4.0), (24.0, -4.0), (24.0, -14.0), (20.5, -13.7)],
    ),
    // Dry-bank service lane to the forge rear door. The original y=-17
    // spur landed on WATER cells and disconnected from the road network.
    (
        "forge-spur",
        &[(24.0, -14.0), (17.0, -14.0), (13.0, -14.0), (10.2, -13.7)],
    ),
    // Perimeter footpath to the stable-yard gate.
    (
        "stable-footpath",
        &[(24.0, -4.0), (25.0, 10.0), (23.1, 20.5), (23.1, 21.5)],
    ),
];

/// Distance from point to segment in the ground plane.
fn seg_dist(px: f32, py: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let t = (((px - a.0) * dx + (py - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    (a.0 + t * dx - px).hypot(a.1 + t * dy - py)
}

fn near_road(x: f32, y: f32, clearance: f32) -> bool {
    ROADS.iter().any(|(_, pts)| {
        pts.windows(2)
            .any(|s| seg_dist(x, y, s[0], s[1]) <= clearance)
    })
}

/// Keep tree trunks and their low crowns out of the coarse path cells.
/// Extra width covers half-cell quantization and a modest canopy radius.
pub(crate) fn scenery_excluded(x: f32, y: f32) -> bool {
    near_road(x, y, ROAD_HALF + 2.8)
}

/// The court terrain with the road network painted on. Drop-in replacement
/// for the bare `terrain_patch_with` ground in `court_scene`.
pub(crate) fn terrain(seed: u64) -> Mesh {
    let mut m = terrain_patch_with(
        COURT_EXTENT * 2.0,
        COURT_EXTENT * 2.0,
        seed,
        &court_terrain(),
    );
    // The patch is one `prim::quad` (two triangles) per grid cell, pushed in
    // row-major order — classify per pair so both halves of a cell agree.
    for pair in m.tris.chunks_exact_mut(2) {
        if pair[0].mat != mat::GRASS {
            continue; // river bed and the central approach road stay as built
        }
        let (mut x0, mut y0) = (f32::INFINITY, f32::INFINITY);
        let (mut x1, mut y1) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
        for v in pair.iter().flat_map(|t| t.v) {
            x0 = x0.min(v.x);
            x1 = x1.max(v.x);
            y0 = y0.min(v.y);
            y1 = y1.max(v.y);
        }
        if near_road((x0 + x1) * 0.5, (y0 + y1) * 0.5, ROAD_HALF) {
            pair[0].mat = mat::PATH;
            pair[1].mat = mat::PATH;
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Grid geometry of `court_terrain`: 56 × 56 tiles at target cell 2.9,
    /// clamped by `terrain_patch_with` to 19 cells a side (722 triangles).
    const CELLS: usize = 19;

    fn path_cell_centres(m: &Mesh) -> HashSet<(i32, i32)> {
        // Cell identity is the integer mesh order, not rounded half-cell
        // floating point coordinates (which created holes in the test graph).
        m.tris
            .chunks_exact(2)
            .enumerate()
            .filter(|(_, pair)| pair[0].mat == mat::PATH)
            .map(|(index, _)| ((index % CELLS) as i32, (index / CELLS) as i32))
            .collect()
    }

    #[test]
    fn deterministic_and_geometry_unchanged() {
        let (a, b) = (terrain(42), terrain(42));
        let plain =
            terrain_patch_with(COURT_EXTENT * 2.0, COURT_EXTENT * 2.0, 42, &court_terrain());
        assert_eq!(a.tris.len(), CELLS * CELLS * 2);
        assert_eq!(a.tris.len(), plain.tris.len());
        for (t, u) in a.tris.iter().zip(b.tris.iter()) {
            assert_eq!(t.mat, u.mat, "painting must not depend on the run");
        }
        for (t, p) in a.tris.iter().zip(plain.tris.iter()) {
            assert_eq!(t.v, p.v, "vertices must be the untouched terrain's");
            assert_eq!(t.uv, p.uv);
            assert_eq!(t.normal, p.normal);
            if p.mat != mat::GRASS {
                assert_eq!(t.mat, p.mat, "WATER and central PATH are preserved");
            } else {
                assert!(t.mat == mat::GRASS || t.mat == mat::PATH);
            }
        }
    }

    #[test]
    fn roads_form_one_connected_network() {
        // Painting is seed-independent (the grid is), so one seed suffices.
        let cells = path_cell_centres(&terrain(7));
        assert!(
            cells.contains(&(9, 6)),
            "courtyard junction (0,-8) is paved"
        );
        // Flood fill from the junction over 4-neighbours.
        let mut seen = HashSet::from([(9, 6)]);
        let mut stack = vec![(9, 6)];
        while let Some((ix, iy)) = stack.pop() {
            for n in [(ix + 1, iy), (ix - 1, iy), (ix, iy + 1), (ix, iy - 1)] {
                if cells.contains(&n) && seen.insert(n) {
                    stack.push(n);
                }
            }
        }
        // Every waypoint of every leg paves a nearby cell on that component.
        let cell = COURT_EXTENT * 2.0 / CELLS as f32;
        for (name, pts) in ROADS {
            for &(x, y) in *pts {
                let reached = seen.iter().any(|&(ix, iy)| {
                    let (cx, cy) = (cell * (ix as f32 + 0.5), cell * (iy as f32 + 0.5));
                    (cx - COURT_EXTENT - x).hypot(cy - COURT_EXTENT - y) <= ROAD_HALF + cell * 0.5
                });
                assert!(reached, "{name} waypoint ({x},{y}) is off the network");
            }
        }
    }
}
