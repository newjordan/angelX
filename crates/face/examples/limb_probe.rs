//! Prints her limbs' extents (a probe while building).
fn main() {
    let bytes = std::fs::read(std::env::args().nth(1).expect("glb")).unwrap();
    let g = angel_face::Goddess::from_glb(&bytes).unwrap();
    for (n, l) in [("arm", &g.arm), ("leg", &g.leg)] {
        let Some(l) = l else { continue };
        let ax = |k: usize| {
            let v: Vec<f32> = l.mesh.positions.iter().map(|p| p[k]).collect();
            (
                v.iter().copied().fold(f32::INFINITY, f32::min),
                v.iter().copied().fold(f32::NEG_INFINITY, f32::max),
            )
        };
        let hist: Vec<usize> = (0..12)
            .map(|b| {
                l.mesh
                    .positions
                    .iter()
                    .filter(|p| ((p[0] / 0.1) as i32) == b)
                    .count()
            })
            .collect();
        println!(
            "{n}: x {:?} y {:?} z {:?} reach {:.3} tip {:.3}\n  x histogram per 10cm: {:?}",
            ax(0),
            ax(1),
            ax(2),
            l.reach,
            l.tip,
            hist
        );
    }
}
