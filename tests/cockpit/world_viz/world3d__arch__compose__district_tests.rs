use super::super::super::mesh::mat;
use super::*;

#[test]
fn new_posterns_are_clear_at_walking_height_and_the_yard_has_no_tree_trunk() {
    let scene = court_scene(0x4341_5354_4c45_3344);
    eprintln!("Expanded court: {} triangles", scene.tris.len());
    assert!(scene.tris.len() < 5_000);
    for side in [-11.0_f32, 11.0] {
        let blocked = scene.tris.iter().any(|t| {
            !matches!(t.mat, mat::PATH | mat::GRASS | mat::WATER)
                && t.v.iter().any(|v| (v.x - side).abs() < 0.6)
                && t.v.iter().map(|v| v.y).fold(f32::INFINITY, f32::min) < -4.0
                && t.v.iter().map(|v| v.y).fold(f32::NEG_INFINITY, f32::max) > -4.0
                && t.v.iter().map(|v| v.z).fold(f32::INFINITY, f32::min) < 1.5
                && t.v.iter().map(|v| v.z).fold(f32::NEG_INFINITY, f32::max) > 1.5
        });
        assert!(!blocked, "postern at {side} is blocked");
    }
    for tri in scene.tris.iter().filter(|t| t.mat == mat::TRUNK) {
        let center = tri
            .v
            .iter()
            .fold((0.0, 0.0), |a, v| (a.0 + v.x / 3.0, a.1 + v.y / 3.0));
        assert!(
            !(center.0 > 17.0 && center.1 > 20.0),
            "tree grew inside stable yard"
        );
        // Check the rendered road cells, not the exclusion predicate that
        // chose the trees. A small setback keeps the walking strip open.
        for path in scene.tris.iter().filter(|t| t.mat == mat::PATH) {
            let min_x = path.v.iter().map(|v| v.x).fold(f32::INFINITY, f32::min);
            let max_x = path.v.iter().map(|v| v.x).fold(f32::NEG_INFINITY, f32::max);
            let min_y = path.v.iter().map(|v| v.y).fold(f32::INFINITY, f32::min);
            let max_y = path.v.iter().map(|v| v.y).fold(f32::NEG_INFINITY, f32::max);
            let dx = (min_x - center.0).max(center.0 - max_x).max(0.0);
            let dy = (min_y - center.1).max(center.1 - max_y).max(0.0);
            assert!(
                dx.hypot(dy) > 0.4,
                "tree trunk occupies rendered road at {center:?}"
            );
        }
    }
}
