use super::*;

fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

#[test]
fn comp_mode_skips_moa_deck_body_without_slowing_default() {
    use crate::tests::TestEnvGuard;
    let _lock = crate::tests::env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::drive::comp_mode::invalidate_cache();
    assert!(moa_deck_body_allowed());

    let mut app = crate::seed_preview_app();
    app.open_moa_deck(None);
    let backend = TestBackend::new(144, 48);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| ui(frame, &mut app)).unwrap();
    let standard = test_backend_text(terminal.backend());
    assert!(
        standard.contains("Agent Formation"),
        "default Formation still paints chrome\n{standard}"
    );
    assert!(
        standard.contains("Outriders") || standard.contains("Errant"),
        "default Formation still paints the card list\n{standard}"
    );

    let _on = TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::drive::comp_mode::invalidate_cache();
    assert!(!moa_deck_body_allowed());

    let mut armed = crate::seed_preview_app();
    armed.open_moa_deck(None);
    let mut lean = Terminal::new(TestBackend::new(144, 48)).unwrap();
    lean.draw(|frame| ui(frame, &mut armed)).unwrap();
    let lean_text = test_backend_text(lean.backend());
    assert!(
        lean_text.contains("Agent Formation"),
        "comp-mode keeps Formation chrome\n{lean_text}"
    );
    assert!(
        !lean_text.contains("Outriders") && !lean_text.contains("Tab roster"),
        "comp-mode must not paint the formation list/roster\n{lean_text}"
    );
}

#[test]
fn think_picker_lines_show_ladder_and_env_default_row() {
    let lines = moa_effort_picker_lines(&["low".to_string(), "high".to_string()], 1);
    let text: Vec<String> = lines.iter().map(line_text).collect();
    assert_eq!(text, vec!["  low", "  high", "  env default (clear)"]);
}

#[test]
fn think_picker_on_a_ladder_less_route_shows_the_na_line() {
    let lines = moa_effort_picker_lines(&[], 0);
    let text: Vec<String> = lines.iter().map(line_text).collect();
    assert_eq!(
        text,
        vec!["effort: n/a (route declares none)", "  env default (clear)"]
    );
}

/// Full-board render proof: a ladder-less route's picker shows the n/a
/// line, and a staged effort is worn by its seat node in the graph.
#[test]
fn formation_board_renders_na_picker_and_staged_seat_effort() {
    let _lock = crate::tests::env_lock();
    let mut app = crate::seed_preview_app();
    app.bag = crate::agent::club::Bag::for_reasoning_render_test();
    app.open_moa_deck(None);
    app.select_moa_card(crate::agent::formations::FormationId::Duel);
    // Ladder-less seat: the practice route declares no reasoning levels.
    {
        let deck = app.moa_deck.as_mut().unwrap();
        assert!(deck.select_slot(2)); // J1
        let practice = deck
            .models()
            .iter()
            .position(|choice| choice.route.model != "gpt-5.6-sol")
            .expect("ladder-less route");
        assert!(deck.assign_model(practice));
    }
    app.open_moa_effort_picker();
    let backend = TestBackend::new(144, 48);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| ui(frame, &mut app)).unwrap();
    let screen = test_backend_text(terminal.backend());
    assert!(
        screen.contains("effort: n/a (route declares none)"),
        "{screen}"
    );

    // Stage a level on the ladder route; the J1 seat node wears it.
    {
        let deck = app.moa_deck.as_mut().unwrap();
        let sol = deck
            .models()
            .iter()
            .position(|choice| choice.route.model == "gpt-5.6-sol")
            .expect("ladder route");
        assert!(deck.assign_model(sol));
        assert!(deck.select_slot(2));
        assert!(deck.open_effort_picker(vec![
            "low".to_string(),
            "medium".to_string(),
            "high".to_string()
        ]));
        deck.move_effort(-1);
        assert!(deck.stage_selected_effort().is_some());
    }
    terminal.draw(|frame| ui(frame, &mut app)).unwrap();
    let screen = test_backend_text(terminal.backend());
    assert!(screen.contains("~high"), "{screen}");
}
