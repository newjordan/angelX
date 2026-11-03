use super::*;

#[test]
fn the_clock_is_the_rules_clock() {
    assert_eq!(HZ, crate::rules::HZ);
}

#[test]
fn its_own_sine_and_exponent_agree_with_the_systems() {
    for i in -4000..=4000 {
        let x = i as f32 * 0.0123;
        assert!((math::sin(x) - x.sin()).abs() < 2e-6, "sin {x}");
        assert!((math::cos(x) - x.cos()).abs() < 2e-6, "cos {x}");
    }
    for i in -800..=800 {
        let x = i as f32 * 0.01;
        let (a, b) = (math::exp(x), x.exp());
        assert!((a - b).abs() <= b * 2e-6, "exp {x}: {a} {b}");
    }
    assert_eq!(math::sin(0.0), 0.0);
    assert_eq!(math::exp(0.0), 1.0);
    assert_eq!(math::sin(f32::NAN), 0.0);
}

#[test]
fn every_ease_starts_at_zero_and_ends_at_one() {
    for e in Ease::ALL {
        assert!(e.at(0.0).abs() < 1e-4, "{e:?} at 0: {}", e.at(0.0));
        assert!((e.at(1.0) - 1.0).abs() < 1e-4, "{e:?} at 1: {}", e.at(1.0));
        // Clamped either side, and never NaN.
        assert_eq!(e.at(-3.0), e.at(0.0), "{e:?}");
        assert_eq!(e.at(7.0), e.at(1.0), "{e:?}");
        assert!(!e.at(f32::NAN).is_nan(), "{e:?}");
    }
    assert_eq!(ease::bump(0.0), 0.0);
    assert!(ease::bump(1.0).abs() < 1e-6);
    assert!((ease::bump(0.5) - 1.0).abs() < 1e-6);
}

#[test]
fn the_overshoots_go_past_and_the_anticipation_pulls_back() {
    let most = |e: Ease| (0..=100).map(|i| e.at(i as f32 / 100.0)).fold(f32::MIN, f32::max);
    let least = |e: Ease| (0..=100).map(|i| e.at(i as f32 / 100.0)).fold(f32::MAX, f32::min);
    for e in Ease::ALL {
        let past = most(e) > 1.01 || least(e) < -0.01;
        assert_eq!(past, e.overshoots(), "{e:?}: {} .. {}", least(e), most(e));
    }
    assert!((1.1..1.15).contains(&most(Ease::Back)));
    assert!((-0.12..-0.08).contains(&least(Ease::Anticipate)));
    assert!((1.2..1.35).contains(&most(Ease::Elastic)), "elastic: {}", most(Ease::Elastic));
    assert!((1.12..1.25).contains(&most(Ease::Spring)), "spring: {}", most(Ease::Spring));
    // A bounce lands three times before it rests, never above its floor.
    assert!(most(Ease::Bounce) <= 1.0 && least(Ease::Bounce) >= 0.0);
    // Out leads In everywhere inside.
    for i in 1..100 {
        let t = i as f32 / 100.0;
        assert!(ease::out(t) > ease::inward(t));
        assert!(ease::out_quad(t) > ease::in_quad(t));
    }
}

/// The eases the magic kit drew with before they moved here, written out,
/// so every effect built on them keeps its pixels.
#[test]
fn the_kits_eases_are_unchanged() {
    for i in -10..=110 {
        let t = i as f32 / 100.0;
        let k = t.clamp(0.0, 1.0);
        let u = 1.0 - k;
        assert_eq!(ease::out(t), 1.0 - u * u * u);
        assert_eq!(ease::inward(t), k * k * k);
        assert_eq!(ease::both(t), k * k * (3.0 - 2.0 * k));
        let (v, s) = (k - 1.0, 1.9f32);
        assert_eq!(ease::back(t), 1.0 + v * v * ((s + 1.0) * v + s));
        let spring = if k >= 1.0 { 1.0 } else { 1.0 - (-6.0 * k).exp() * (k * 11.0).cos() };
        assert!((ease::spring(t) - spring).abs() < 2e-6);
        assert!((ease::bump(t) - (k * std::f32::consts::PI).sin()).abs() < 2e-6);
    }
}

#[test]
fn beats_fall_in_order() {
    let b = Beats::new(4.0, 4.0, 4.0, 8.0);
    assert!(matches!(b.at(1.0), Beat::Anticipation(_)));
    assert!(matches!(b.at(5.0), Beat::Action(_)));
    assert!(matches!(b.at(9.0), Beat::Follow(_)));
    assert!(matches!(b.at(13.0), Beat::Settle(_)));
    assert_eq!(b.at(20.0), Beat::Done);
    assert_eq!(b.at(-1.0), Beat::Done);
    assert_eq!((b.action_at(), b.follow_at(), b.settle_at(), b.total()), (4.0, 8.0, 12.0, 20.0));
    assert_eq!(b.since_action(2.0), -2.0);
    // A pose winds back, throws past the mark and settles on it.
    let pose: Vec<f32> = (0..20).map(|t| b.pose(t as f32, 0.3)).collect();
    assert!(pose[3] < -0.2);
    assert!(pose[8] > 1.1);
    assert!((pose[13] - 1.0).abs() < 1e-6);
    // No beat at all is no anticipation: the action from the first tick.
    let snap = Beats::new(0.0, 3.0, 3.0, 6.0);
    assert!(matches!(snap.at(0.0), Beat::Action(_)));
}

#[test]
fn a_track_eases_between_its_keys_and_holds_where_they_repeat() {
    let keys = [
        key(0.0, 0.0, Ease::Linear),
        key(4.0, 10.0, Ease::Linear),
        key(10.0, 10.0, Ease::Linear),
        key(14.0, 0.0, Ease::In),
    ];
    assert_eq!(track(&keys, -5.0), 0.0);
    assert_eq!(track(&keys, 2.0), 5.0);
    for t in 4..=10 {
        assert_eq!(track(&keys, t as f32), 10.0, "held at {t}");
    }
    assert!(track(&keys, 12.0) > 5.0, "an in-ease leaves slowly");
    assert_eq!(track(&keys, 99.0), 0.0);
    assert_eq!(track(&[], 3.0), 0.0);
}

#[test]
fn a_spring_comes_to_rest_where_it_is_pulled() {
    for s in [Spring::SNAPPY, Spring::SOFT, Spring::WOBBLY] {
        assert_eq!(s.at(0.0, 3.0, 9.0, 0.0), 3.0);
        assert!((s.at(300.0, 3.0, 9.0, 0.0) - 9.0).abs() < 1e-3, "{s:?}");
        assert!(s.kick(300.0, 1.0).abs() < 1e-3, "{s:?}");
        // Between ticks it is between the ticks' poses.
        let (a, b, m) = (s.ease(5.0), s.ease(6.0), s.ease(5.5));
        assert!((m - (a + b) / 2.0).abs() < 1e-6);
    }
    let peak = |s: Spring| (0..90).map(|t| s.ease(t as f32)).fold(f32::MIN, f32::max);
    assert!(peak(Spring::SNAPPY) < 1.12, "snappy barely passes: {}", peak(Spring::SNAPPY));
    assert!(peak(Spring::WOBBLY) > 1.4, "wobbly rings: {}", peak(Spring::WOBBLY));
    // A kick goes the way it was kicked first.
    assert!(Spring::SOFT.kick(3.0, 1.0) > 0.0);
    // Past ten seconds it is simply at rest.
    assert_eq!(Spring::WOBBLY.at(1.0e6, 0.0, 2.0, 0.0), Spring::WOBBLY.at(1.0e6 + 0.5, 0.0, 2.0, 0.0));
}

#[test]
fn squash_keeps_the_area_and_an_arc_lands() {
    for k in [-0.5, -0.2, 0.0, 0.3, 0.6] {
        let (w, h) = squash(k);
        assert!((w * h - 1.0).abs() < 1e-6);
    }
    assert!(squash(0.3).0 > 1.0 && squash(-0.3).1 > 1.0);
    assert_eq!(arc((0.0, 0.0), (10.0, 4.0), 6.0, 0.0), (0.0, 0.0));
    assert_eq!(arc((0.0, 0.0), (10.0, 4.0), 6.0, 1.0), (10.0, 4.0));
    assert_eq!(arc((0.0, 0.0), (10.0, 0.0), 6.0, 0.5), (5.0, -6.0));
    assert_eq!(steps(0.74, 4), 0.5);
    assert_eq!(steps(1.0, 4), 1.0);
    assert_eq!(on(7, 2), 6);
}

/// The camera's shake as it was drawn before it moved here, with the
/// system's sine: the same pixels.
#[test]
fn the_shake_is_the_shake_it_was() {
    let before = |tick: u64, left: u32, full: u32, reach: f32| {
        let trauma = (left as f32 / full.max(1) as f32).min(1.0);
        let amp = reach * trauma * trauma;
        let t = tick as f64;
        let x = (t * 0.47).sin() * 0.7 + (t * 0.19 + 1.3).sin() * 0.3;
        let y = (t * 0.37 + 2.1).sin() * 0.7 + (t * 0.23).sin() * 0.3;
        ((x as f32 * amp).round() as i32, (y as f32 * amp).round() as i32)
    };
    for tick in 0..5000u64 {
        for left in [0, 1, 3, 6, 12] {
            assert_eq!(shake(tick, left, 12, 6.0), before(tick, left, 12, 6.0), "{tick} {left}");
        }
    }
    assert_eq!(shake(77, 0, 12, 6.0), (0, 0));
    let most = (0..2000).map(|t| shake(t, 12, 12, 6.0)).map(|(x, y)| x.abs().max(y.abs())).max();
    assert_eq!(most, Some(6));
}

#[test]
fn an_impact_holds_then_falls_and_a_flash_blinks_out() {
    assert_eq!(impact(-1.0, 2.0, 6.0), 0.0);
    assert_eq!(impact(0.0, 2.0, 6.0), 1.0);
    assert_eq!(impact(1.5, 2.0, 6.0), 1.0);
    assert!(impact(4.0, 2.0, 6.0) < 0.5);
    assert_eq!(impact(8.0, 2.0, 6.0), 0.0);
    let lit: Vec<bool> = (0..8).map(|a| flash(a, 2, 4)).collect();
    assert_eq!(lit, [true, true, false, true, false, true, false, false]);
}

/// Every value this module makes, in bits: a machine that computes any
/// one of them differently fails here, before a friend's mirror draws a
/// different pixel from the host.
#[test]
fn the_same_bits_on_every_machine() {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut eat = |v: f32| {
        for b in v.to_bits().to_le_bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    };
    for i in 0..=256 {
        let t = i as f32 / 256.0;
        for e in Ease::ALL {
            eat(e.at(t));
        }
        eat(ease::bump(t));
        eat(math::sin(t * 40.0 - 20.0));
        eat(math::exp(t * 20.0 - 10.0));
    }
    for s in [Spring::SNAPPY, Spring::SOFT, Spring::WOBBLY] {
        for a in 0..120 {
            eat(s.at(a as f32 * 0.75, -2.0, 5.0, 0.4));
        }
    }
    for tick in 0..300 {
        let (x, y) = shake(tick, 9, 12, 6.0);
        eat(x as f32);
        eat(y as f32);
    }
    assert_eq!(h, PINNED, "motion values: {h:#018x}");
}

const PINNED: u64 = 0xf50d_d596_8cad_b963;
