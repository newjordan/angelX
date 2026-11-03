use super::super::mesh::mat;
use super::*;

#[test]
fn the_court_is_built_once_and_shared() {
    let a = scene_for(SceneKey::COURT);
    let b = scene_for(SceneKey::COURT);
    assert!(
        Arc::ptr_eq(&a, &b),
        "the scene must be cached, not rebuilt per frame"
    );
}

#[test]
fn the_court_is_finite_bounded_and_carries_every_staged_material() {
    let scene = scene_for(SceneKey::COURT);
    assert!(
        scene.tris.len() > 2_000,
        "the vista needs real architecture, got {} triangles",
        scene.tris.len()
    );
    // The whole-scene budget from docs/plans/world3d-spec.md.
    assert!(
        scene.tris.len() < 5_000,
        "the stage must stay inside the toy pane's budget, got {}",
        scene.tris.len()
    );
    let mut seen = [false; 24];
    for tri in &scene.tris {
        for point in tri.v {
            assert!(
                point.x.is_finite() && point.y.is_finite() && point.z.is_finite(),
                "non-finite vertex {point:?}"
            );
            assert!(
                point.x.abs() <= arch::COURT_EXTENT + 1.0
                    && point.y.abs() <= arch::COURT_EXTENT + 1.0,
                "geometry outside the ground patch at {point:?}"
            );
        }
        assert!(
            tri.normal.length() > 0.9,
            "degenerate triangle normal {:?}",
            tri.normal
        );
        if (tri.mat as usize) < seen.len() {
            seen[tri.mat as usize] = true;
        }
    }
    for material in [
        mat::STONE,
        mat::STONE_DARK,
        mat::ROOF,
        mat::WOOD,
        mat::WINDOW,
        mat::DOOR,
        mat::GRASS,
        mat::PATH,
        mat::WATER,
        mat::FOLIAGE,
        mat::TRUNK,
    ] {
        assert!(seen[material as usize], "material {material} never staged");
    }
}

#[test]
fn the_scene_is_deterministic_across_builds() {
    let a = arch::court_scene(SCENE_SEED);
    let b = arch::court_scene(SCENE_SEED);
    assert_eq!(a.tris.len(), b.tris.len());
    for (left, right) in a.tris.iter().zip(b.tris.iter()) {
        assert_eq!(left.mat, right.mat);
        for index in 0..3 {
            assert_eq!(left.v[index], right.v[index]);
            assert_eq!(left.uv[index], right.uv[index]);
        }
    }
}
