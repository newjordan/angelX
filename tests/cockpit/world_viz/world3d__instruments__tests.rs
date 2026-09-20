use super::*;

#[test]
fn instruments_are_bounded_finite_and_do_not_emit_status_light() {
    for make in [
        |mesh: &mut Mesh| telescope(mesh, v3(0.55, -0.60, 1.0), v3(4.05, -0.12, 3.65)),
        |mesh: &mut Mesh| armillary(mesh, v3(3.30, 2.55, 1.42)),
        council_map,
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
