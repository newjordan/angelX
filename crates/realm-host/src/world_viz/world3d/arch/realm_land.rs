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

const ROAD_HALF: f32 = 2.0;

const ROADS: &[(&str, &[(f32, f32)])] = &[
    (
        "court-east",
        &[(0.0, -8.0), (3.0, -7.0), (5.0, -4.0), (11.0, -4.0)],
    ),
    (
        "court-west",
        &[(0.0, -8.0), (-3.0, -7.0), (-5.0, -4.0), (-11.0, -4.0)],
    ),
    (
        "west-library",
        &[(-11.0, -4.0), (-15.0, -4.0), (-20.0, -2.0), (-20.0, 1.0)],
    ),
    (
        "east-chapel",
        &[(11.0, -4.0), (15.0, -4.0), (18.0, -2.0), (17.0, 3.5)],
    ),
    (
        "observatory",
        &[(15.0, -4.0), (24.0, -4.0), (24.0, -14.0), (20.5, -13.7)],
    ),
    (
        "forge-spur",
        &[(24.0, -14.0), (17.0, -14.0), (13.0, -14.0), (10.2, -13.7)],
    ),
    (
        "stable-footpath",
        &[(24.0, -4.0), (25.0, 10.0), (23.1, 20.5), (23.1, 21.5)],
    ),
];

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

pub fn scenery_excluded(x: f32, y: f32) -> bool {
    near_road(x, y, ROAD_HALF + 2.8)
}

pub fn terrain(seed: u64) -> Mesh {
    let mut m = terrain_patch_with(
        COURT_EXTENT * 2.0,
        COURT_EXTENT * 2.0,
        seed,
        &court_terrain(),
    );
    for pair in m.tris.chunks_exact_mut(2) {
        if pair[0].mat != mat::GRASS {
            continue;
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
#[path = "../../../../tests/world_viz/world3d__arch__realm_land__tests.rs"]
mod tests;
