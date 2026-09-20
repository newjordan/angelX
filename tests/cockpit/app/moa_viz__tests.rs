use super::*;

fn signature(text: &Text<'_>) -> String {
    text.lines
        .iter()
        .flat_map(|line| line.spans.iter())
        .map(|span| format!("{}{:?}", span.content, span.style.fg))
        .collect()
}

#[test]
fn every_card_has_a_distinct_motion_identity() {
    let mut identities = std::collections::BTreeSet::new();
    for formation in formations::built_in_deck() {
        let a = render(*formation, None, 0.0, 24, 12, MotionMode::Full);
        let b = render(*formation, None, 0.75, 24, 12, MotionMode::Full);
        assert_ne!(
            signature(&a),
            signature(&b),
            "{} should move",
            formation.name
        );
        identities.insert(format!("{}:{}", formation.id.slug(), signature(&b)));
    }
    assert_eq!(identities.len(), formations::built_in_deck().len());
}

#[test]
fn ordered_dot_dissolve_has_exact_endpoints() {
    let previous = vec![
        Cell {
            glyph: '\u{28ff}',
            fg: GOLD
        };
        4
    ];
    let current = vec![
        Cell {
            glyph: '\u{2801}',
            fg: CYAN
        };
        4
    ];
    assert_eq!(dissolve(&previous, &current, 2, 2, 0.0), previous);
    assert_eq!(dissolve(&previous, &current, 2, 2, 1.0), current);
    let middle = dissolve(&previous, &current, 2, 2, 0.5);
    assert_ne!(middle, previous);
    assert_ne!(middle, current);
}

#[test]
fn off_and_reduced_modes_hold_static_card_pose() {
    for motion in [MotionMode::Reduced, MotionMode::Off] {
        let formation = *formations::formation(FormationId::Recon);
        let a = render(formation, None, 0.0, 20, 10, motion);
        let b = render(formation, None, 1.0, 20, 10, motion);
        assert_eq!(signature(&a), signature(&b));
    }
}

#[test]
fn warmed_selected_card_stays_under_cockpit_draw_budget() {
    let formation = *formations::formation(FormationId::AllIn);
    let _ = crate::term::art::preload_colored_image(&formation.asset_path());
    // Use the fastest warmed sample, as the larger lifecycle renderer does:
    // a single sample in the parallel suite measures scheduler preemption,
    // not uncontended render cost.
    let mut fastest = std::time::Duration::MAX;
    let mut frame = None;
    for _ in 0..5 {
        let started = std::time::Instant::now();
        frame = Some(render(formation, None, 0.75, 34, 22, MotionMode::Full));
        fastest = fastest.min(started.elapsed());
    }
    let frame = frame.unwrap();
    assert_eq!(frame.lines.len(), 22);
    assert!(
        fastest < std::time::Duration::from_millis(50),
        "fastest of five warmed selected card frames took {fastest:?}"
    );
}
