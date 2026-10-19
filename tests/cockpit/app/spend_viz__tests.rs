use super::*;

#[test]
fn full_motion_tosses_with_fixed_geometry_and_a_flip() {
    let stage = Rect::new(4, 3, 30, 14);
    let start = pose(stage, 0.0, MotionMode::Full).unwrap();
    let apex = pose(stage, FULL_DURATION_SECS / 2.0, MotionMode::Full).unwrap();
    assert_eq!((start.area.width, start.area.height), (6, 3));
    assert_eq!((apex.area.width, apex.area.height), (6, 3));
    assert!(apex.area.y < start.area.y);
    assert!((0..40).any(|step| {
        !pose(
            stage,
            FULL_DURATION_SECS * step as f32 / 40.0,
            MotionMode::Full,
        )
        .unwrap()
        .face_visible
    }));
}

#[test]
fn reduced_and_off_never_rotate_the_coin() {
    let stage = Rect::new(0, 0, 20, 8);
    assert!(pose(stage, 0.4, MotionMode::Reduced).unwrap().face_visible);
    assert!(pose(stage, 0.4, MotionMode::Off).unwrap().face_visible);
}

#[test]
fn pose_expires_and_formats_the_milestone() {
    assert!(pose(Rect::new(0, 0, 20, 8), FULL_DURATION_SECS, MotionMode::Full).is_none());
    assert_eq!(format_input_tokens(1_000_000), "1.0M");
    assert_eq!(format_input_tokens(2_500_000), "2.5M");
}

#[test]
fn each_million_milestone_launches_that_many_staggered_coins() {
    let stage = Rect::new(4, 3, 40, 16);
    assert_eq!(milestone_coins(999_999), 0);
    assert_eq!(milestone_coins(2_500_000), 2);
    for count in 1..=20 {
        assert_eq!(milestone_coins(count * 1_000_000), count);
        for index in 0..count {
            let t = index as f64 * STAGGER_SECS + 0.001;
            let live = poses(stage, t, count, MotionMode::Full);
            assert_eq!(live.iter().filter(|p| p.area.y == 15).count(), 1,
                "coin {index} of {count} must launch from the foot of the arc");
            assert!(live.len() <= MAX_LIVE_COINS as usize);
        }
        assert!(poses(stage, burst_duration_secs(count, MotionMode::Full), count, MotionMode::Full).is_empty());
    }
}

#[test]
fn bursts_are_bounded_even_for_huge_counts_and_small_panes() {
    for motion in [MotionMode::Full, MotionMode::Reduced, MotionMode::Off] {
        for stage in [Rect::new(0, 0, 0, 0), Rect::new(3, 4, 2, 1),
            Rect::new(4, 5, 3, 1), Rect::new(8, 9, 11, 4)] {
            for t in [0.0, 0.5, 1_000_000.0, f64::NAN, f64::INFINITY, -1.0] {
                let live = poses(stage, t, u64::MAX, motion);
                assert!(live.len() <= MAX_LIVE_COINS as usize);
                for p in live {
                    assert_eq!(p.area.intersection(stage), p.area);
                    assert!(p.face_scale.is_finite() && p.face_scale > 0.0);
                }
            }
        }
    }
    assert!(poses(Rect::new(0, 0, 40, 15), 0.0, 0, MotionMode::Full).is_empty());
}

#[test]
fn coin_has_a_gold_face_embossed_rim_and_visible_reeded_edge() {
    assert!(gold_pixel(0.99, 0.99, 1.0).is_none(), "corners must be transparent");
    let mut colors = std::collections::HashSet::new();
    for row in 0..64 {
        for col in 0..64 {
            if let Some([r, g, b]) = gold_pixel(col as f32 / 32.0 - 1.0,
                row as f32 / 32.0 - 1.0, 1.0) {
                assert!(r > g && g > b, "coin must be gold, not a sparkle");
                colors.insert([r, g, b]);
            }
        }
    }
    assert!(colors.len() > 50, "material needs shading and rim detail");
    assert!(gold_pixel(0.0, 0.0, 0.08).is_some(), "edge-on coin must not vanish");
    let face = gold_pixel(0.10, -0.78, 1.0).unwrap();
    assert!(face[0] > 200 && face[1] > 120);
}

#[test]
fn coin_draws_a_gold_disc_not_a_sparkle_and_motion_off_is_static() {
    use ratatui::{Terminal, backend::TestBackend};
    let stage = Rect::new(0, 0, 30, 14);
    assert_eq!(poses(stage, 0.1, 5, MotionMode::Off), poses(stage, 0.8, 5, MotionMode::Off));
    let mut terminal = Terminal::new(TestBackend::new(30, 14)).unwrap();
    terminal.draw(|f| {
        for p in poses(stage, 0.0, 1, MotionMode::Off) { render_coin(f, p); }
    }).unwrap();
    let cells = &terminal.backend().buffer().content;
    let dots = cells.iter().filter(|c| c.symbol().chars().any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch))).count();
    assert!(dots > 6);
    assert!(!cells.iter().any(|c| c.symbol() == "✦" || c.symbol() == "│"));
}
