use ratatui::crossterm::event::{KeyCode, KeyModifiers};

#[test]
fn t_key_opens_the_think_picker_and_enter_stages_the_level() {
    let _lock = crate::tests::env_lock();
    let mut app = crate::seed_preview_app();
    app.bag = crate::club::Bag::for_reasoning_render_test();
    app.open_moa_deck(None);
    app.select_moa_card(crate::formations::FormationId::Duel);
    {
        let deck = app.moa_deck.as_mut().unwrap();
        assert!(deck.select_slot(2)); // J1
        let sol = deck
            .models()
            .iter()
            .position(|choice| choice.route.model == "gpt-5.6-sol")
            .expect("ladder-declaring route");
        assert!(deck.assign_model(sol));
    }
    assert!(app.moa_deck_key(KeyCode::Char('t'), KeyModifiers::NONE));
    let deck = app.moa_deck.as_ref().unwrap();
    assert_eq!(deck.focus(), crate::formations::MoaDeckFocus::Efforts);
    assert_eq!(deck.effort_options(), ["low", "medium", "high"]);
    // Nothing staged: cursor on env default; Up wraps onto "high".
    assert!(app.moa_deck_key(KeyCode::Up, KeyModifiers::NONE));
    assert!(app.moa_deck_key(KeyCode::Enter, KeyModifiers::NONE));
    let deck = app.moa_deck.as_ref().unwrap();
    assert_eq!(deck.focus(), crate::formations::MoaDeckFocus::Slots);
    assert_eq!(
        deck.selected_roster()
            .role_effort(crate::formations::FormationRole::Judge),
        Some("high")
    );
    assert!(
        app.messages
            .iter()
            .any(|message| message.text.contains("staged at THINK high"))
    );
}

/// Every refusal to open the picker must say why — a silent `t` reads as
/// nonexistence.
#[test]
fn think_gates_speak_for_resting_scout_and_unassigned_seats() {
    let _lock = crate::tests::env_lock();
    let mut app = crate::seed_preview_app();
    app.bag = crate::club::Bag::for_render_test(&[("alpha", &[("model-a", true)])]);
    app.open_moa_deck(None);

    // Solo Strike has no seats at all.
    assert!(app.moa_deck_key(KeyCode::Char('t'), KeyModifiers::NONE));
    assert!(
        app.messages
            .iter()
            .any(|message| message.text.contains("no formation seats"))
    );

    // Recon's scout seat: the swarm carries no scout effort policy.
    app.select_moa_card(crate::formations::FormationId::Recon);
    assert!(app.moa_deck.as_mut().unwrap().select_slot(0));
    assert!(app.moa_deck_key(KeyCode::Char('t'), KeyModifiers::NONE));
    assert!(
        app.messages
            .iter()
            .any(|message| message.text.contains("no scout seat effort"))
    );
    assert_ne!(
        app.moa_deck.as_ref().unwrap().focus(),
        crate::formations::MoaDeckFocus::Efforts
    );

    // Tag Team P1 finds no fleet box here: effort staging needs a route.
    app.select_moa_card(crate::formations::FormationId::TagTeam);
    assert!(app.moa_deck.as_mut().unwrap().select_slot(0));
    assert!(app.moa_deck_key(KeyCode::Char('t'), KeyModifiers::NONE));
    assert!(
        app.messages
            .iter()
            .any(|message| message.text.contains("has no model"))
    );
    assert_ne!(
        app.moa_deck.as_ref().unwrap().focus(),
        crate::formations::MoaDeckFocus::Efforts
    );
}

/// Arming a roster with a staged THINK column names it in the receipt.
#[test]
fn arm_receipt_names_the_staged_think_column() {
    let _lock = crate::tests::env_lock();
    let mut app = crate::seed_preview_app();
    app.bag = crate::club::Bag::for_reasoning_render_test();
    app.open_moa_deck(None);
    app.select_moa_card(crate::formations::FormationId::Duel);
    {
        let deck = app.moa_deck.as_mut().unwrap();
        assert!(deck.select_slot(2)); // J1
        assert!(deck.open_effort_picker(vec![
            "low".to_string(),
            "medium".to_string(),
            "high".to_string()
        ]));
        deck.move_effort(-1);
        assert!(deck.stage_selected_effort().is_some());
    }
    app.play_selected_moa_card(false);
    assert!(app.moa_one_shot.is_some());
    assert!(
        app.messages
            .iter()
            .any(|message| message.text.contains("THINK JUDGE high"))
    );
}
