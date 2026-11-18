use super::*;
use std::collections::HashSet;

const CELLS: usize = 19;

fn path_cell_centres(m: &Mesh) -> HashSet<(i32, i32)> {
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
    let plain = terrain_patch_with(COURT_EXTENT * 2.0, COURT_EXTENT * 2.0, 42, &court_terrain());
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
    let cells = path_cell_centres(&terrain(7));
    assert!(
        cells.contains(&(9, 6)),
        "courtyard junction (0,-8) is paved"
    );
    let mut seen = HashSet::from([(9, 6)]);
    let mut stack = vec![(9, 6)];
    while let Some((ix, iy)) = stack.pop() {
        for n in [(ix + 1, iy), (ix - 1, iy), (ix, iy + 1), (ix, iy - 1)] {
            if cells.contains(&n) && seen.insert(n) {
                stack.push(n);
            }
        }
    }
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
