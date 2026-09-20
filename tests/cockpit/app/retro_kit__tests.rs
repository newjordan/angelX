use super::*;

#[test]
fn dither_extremes_and_coverage() {
    for y in 0..8 {
        for x in 0..8 {
            assert!(!dither(x, y, 0.0), "zero luminance must never light");
            assert!(dither(x, y, 1.0), "full luminance must always light");
        }
    }
    // Mid grey lights about half the matrix — the whole point of Bayer.
    let lit = (0..8)
        .flat_map(|y| (0..8).map(move |x| (x, y)))
        .filter(|&(x, y)| dither(x, y, 0.5))
        .count();
    assert!((24..=40).contains(&lit), "50% grey lit {lit}/64 dots");
}

#[test]
fn ramps_run_dark_to_bright() {
    for ramp in [EMBER, TORCH, MOONLIT, DUSK] {
        let lo = ramp.sample(0.0);
        let hi = ramp.sample(1.0);
        let luma = |c: DotColor| u32::from(c.r) + u32::from(c.g) + u32::from(c.b);
        assert!(
            luma(lo) < luma(hi),
            "ramp must brighten from t=0 to t=1: {lo:?} vs {hi:?}"
        );
        // Out-of-range samples clamp instead of panicking.
        assert_eq!(ramp.sample(-1.0), lo);
        assert_eq!(ramp.sample(2.0), hi);
    }
}

#[test]
fn banded_sampling_quantizes() {
    let mut distinct = std::collections::BTreeSet::new();
    for i in 0..=100 {
        let c = EMBER.banded(i as f32 / 100.0, 4);
        distinct.insert((c.r, c.g, c.b));
    }
    assert!(
        distinct.len() <= 5,
        "4 bands should yield at most 5 inks, got {}",
        distinct.len()
    );
}

#[test]
fn noise_is_deterministic_and_in_range() {
    for i in 0..64 {
        let x = i as f32 * 0.37;
        let y = i as f32 * 0.61;
        let a = value_noise(x, y, 7);
        let b = value_noise(x, y, 7);
        assert_eq!(a, b, "same input must give same noise");
        assert!((0.0..=1.0).contains(&a));
        let f = fbm(x, y, 3, 7);
        assert!((0.0..=1.0).contains(&f));
    }
    // Different seeds decorrelate.
    assert_ne!(value_noise(3.3, 4.4, 1), value_noise(3.3, 4.4, 2));
}

#[test]
fn flame_has_hot_base_cool_tip_and_bounds() {
    let base = flame_heat(0.0, 0.05, 1.0, 3);
    let tip = flame_heat(0.0, 0.95, 1.0, 3);
    assert!(
        base > tip,
        "flame base ({base}) must run hotter than tip ({tip})"
    );
    assert!(base > 0.5, "centre of the base should glow: {base}");
    for step in 0..200 {
        let u = (step % 20) as f32 / 10.0 - 1.0;
        let v = (step / 20) as f32 / 10.0;
        let h = flame_heat(u, v, step as f32 * 0.13, 9);
        assert!((0.0..=1.0).contains(&h));
    }
    // Outside the envelope there is no heat.
    assert_eq!(flame_heat(1.8, 0.5, 0.0, 3), 0.0);
}

#[test]
fn glow_and_flicker_stay_bounded() {
    assert!(glow(0.0, 5.0) > 0.99);
    assert_eq!(glow(5.0, 5.0), 0.0);
    assert_eq!(glow(9.0, 5.0), 0.0);
    assert_eq!(glow(1.0, 0.0), 0.0);
    for i in 0..100 {
        let f = flicker(i as f32 * 0.21, 11);
        assert!((0.8..=1.15).contains(&f), "flicker out of band: {f}");
    }
}
