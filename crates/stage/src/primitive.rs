//! CPU projection for hosts without the procedural GPU surface.

pub fn distance(shape: u32, [x, y, z]: [f32; 3]) -> f32 {
    let radius = x.hypot(z);
    match shape {
        0 => {
            let q = [x.abs() - 0.62, y.abs() - 0.62, z.abs() - 0.62];
            q[0].max(0.0).hypot(q[1].max(0.0)).hypot(q[2].max(0.0))
                + q[0].max(q[1]).max(q[2]).min(0.0) - 0.04
        }
        1 => x.hypot(y).hypot(z) - 0.82,
        2 => ((x.abs().max(z.abs()) + 0.48 * y - 0.39) / 1.11)
            .max(-y - 0.72).max(y - 0.82),
        3 => (radius - 0.62).max(y.abs() - 0.73),
        4 => ((radius + 0.45 * y - 0.37) / 1.10).max(-y - 0.74).max(y - 0.82),
        5 => (radius - 0.57).hypot(y) - 0.25,
        _ => (x.abs() + y.abs() + z.abs() - 1.12) / 3.0_f32.sqrt(),
    }
}

pub fn sample(shape: u32, phase: f32, x: f32, y: f32) -> f32 {
    let sdf = |[x, y, z]: [f32; 3]| {
        let (s, c) = phase.sin_cos();
        let (x, z) = (c * x + s * z, -s * x + c * z);
        let (s, c) = (0.36 + 0.18 * phase.sin()).sin_cos();
        distance(shape, [x, c * y - s * z, s * y + c * z])
    };
    let mut z = 2.5;
    for _ in 0..72 {
        let d = sdf([x, y, z]);
        if d < 0.003 {
            let e = 0.004;
            let n = [sdf([x + e, y, z]) - sdf([x - e, y, z]),
                sdf([x, y + e, z]) - sdf([x, y - e, z]),
                sdf([x, y, z + e]) - sdf([x, y, z - e])];
            let len = n[0].hypot(n[1]).hypot(n[2]).max(0.0001);
            return 0.24 + 0.76 * ((-0.45*n[0] + 0.65*n[1] + 0.61*n[2]) / len).max(0.0);
        }
        z -= d.max(0.002);
        if z < -1.4 { break; }
    }
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Layer, LayerKind, Placement};

    fn layer(shape: u32, phase: f32, speed: f32) -> Layer {
        Layer::new(1, 3, Placement::Pixels([0.0, 0.0, 96.0, 96.0]),
            LayerKind::Primitive { shape, color: [0.2, 0.8, 1.0, 1.0], phase, speed })
    }

    #[test]
    fn primitive_validation_rejects_nonfinite_and_unsupported_geometry() {
        for input in [layer(7, 0.0, 0.4), layer(0, f32::NAN, 0.4),
            layer(0, 0.0, f32::INFINITY)] {
            assert!(input.clone().sanitized().is_none());
            assert!(input.sanitized_cow().is_none());
        }
        let clean = layer(0, -0.5, 99.0).sanitized().unwrap();
        assert!(matches!(clean.kind, LayerKind::Primitive { phase, speed: 2.0, .. }
            if (0.0..std::f32::consts::TAU).contains(&phase)));
        assert!(clean.is_sound());
    }

    #[test]
    fn compositor_motion_and_quad_fallback_respect_the_freeze_switch() {
        let moving = layer(0, 0.4, 0.45);
        let still = layer(0, 0.4, 0.0);
        assert!(moving.animating());
        assert!(!still.animating());
        let project = |l: &Layer, t| crate::expand(std::slice::from_ref(l), t, |_| None);
        assert_ne!(project(&moving, 0.0), project(&moving, 2.0));
        assert_eq!(project(&still, 0.0), project(&still, 2.0));
        assert!((1..=576).contains(&project(&moving, 0.0).len()));
    }

    #[test]
    fn every_solid_has_finite_lit_surfaces_and_turns_in_depth() {
        for shape in 0..7 {
            let image = |phase| (0..768).map(|i| sample(shape, phase,
                (i % 32) as f32 / 32.0 * 2.8 - 1.4,
                1.4 - (i / 32) as f32 / 24.0 * 2.8)).collect::<Vec<_>>();
            let a = image(0.35);
            assert!(a.iter().all(|v| v.is_finite() && (0.0..=1.01).contains(v)));
            assert!(a.iter().filter(|&&v| v > 0.0).count() > 40);
            assert!(a.iter().any(|&v| v > 0.65));
            if shape != 1 { assert_ne!(a, image(1.4), "shape {shape} did not turn in depth"); }
        }
    }
}
