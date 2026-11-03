use super::*;

#[test]
fn instruments_are_bounded_finite_and_do_not_emit_status_light() {
    for make in [
        |mesh: &mut Mesh| telescope(mesh, v3(0.55, -0.60, 1.0), v3(4.05, -0.12, 3.65)),
        |mesh: &mut Mesh| armillary(mesh, v3(3.30, 2.55, 1.42)),
        council_map,
        survey_dial,
    ] {
        let mut mesh = Mesh::new();
        make(&mut mesh);
        assert!(!mesh.tris.is_empty() && mesh.tris.len() < 180);
        for tri in &mesh.tris {
            assert!(!matches!(
                tri.mat,
                mat::WINDOW | mat::FIRE | mat::EMBER | mat::MOONLIGHT
            ));
            assert!(tri.normal.length() > 0.9);
            assert!(
                tri.v
                    .iter()
                    .all(|p| p.x.is_finite() && p.y.is_finite() && p.z.is_finite())
            );
        }
    }
}

#[test]
fn bridge_lanterns_are_small_steady_lights_with_a_visible_hood() {
    let mut mesh = Mesh::new();
    bridge_lantern(&mut mesh);
    assert!(mesh.tris.len() <= 32);
    assert!(mesh.tris.iter().any(|tri| tri.mat == mat::WINDOW));
    assert!(mesh.tris.iter().any(|tri| tri.mat == mat::BRASS));
    for tri in &mesh.tris {
        assert!(!matches!(tri.mat, mat::FIRE | mat::EMBER));
        assert!(tri.normal.length() > 0.9);
        assert!(
            tri.v
                .iter()
                .all(|p| p.x.abs() <= 0.31 && p.y.abs() <= 0.31 && (0.0..=1.31).contains(&p.z))
        );
    }
}

#[test]
fn scrying_glass_has_bounded_geometry_and_never_signals_activity() {
    let mut mesh = Mesh::new();
    scrying_globe(&mut mesh, v3(1.55, -0.45, 1.92));
    assert!(mesh.tris.len() <= 210);
    assert!(mesh.tris.iter().any(|tri| tri.mat == mat::CRYSTAL));
    assert!(mesh.tris.iter().any(|tri| tri.mat == mat::BRASS));
    for tri in &mesh.tris {
        assert!(!matches!(tri.mat, mat::WINDOW | mat::FIRE | mat::EMBER));
        assert!(tri.normal.length() > 0.9);
        assert!(tri.v.iter().all(|p| {
            p.x.is_finite() && p.y.is_finite() && p.z.is_finite() && (0.0..=3.0).contains(&p.z)
        }));
    }
}
