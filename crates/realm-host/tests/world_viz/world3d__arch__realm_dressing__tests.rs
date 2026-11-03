use super::*;

#[test]
fn stable_yard_stays_bounded_finite_and_inside_its_triangle_budget() {
    for seed in [0, 42, 0x4341_5354_4c45_3344, u64::MAX] {
        let a = dressing(seed);
        let b = dressing(seed);
        assert!(a.tris.len() <= 160);
        assert_eq!(a.tris.len(), b.tris.len());
        for (ta, tb) in a.tris.iter().zip(&b.tris) {
            assert_eq!(ta.v, tb.v);
            assert_eq!(ta.mat, tb.mat);
            assert!(ta.normal.length() > 0.9);
            for v in ta.v {
                assert!(v.x.is_finite() && v.y.is_finite() && v.z.is_finite());
                assert!(v.x.abs() <= 28.0 && v.y.abs() <= 28.0);
            }
        }
    }
}
