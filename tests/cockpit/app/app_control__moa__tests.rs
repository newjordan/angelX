use ratatui::crossterm::event::{KeyCode, KeyModifiers};

#[test]
fn t_key_opens_the_think_picker_and_enter_stages_the_level() {
    let _lock = crate::tests::env_lock();
    let mut app = crate::seed_preview_app();
    app.bag = crate::agent::club::Bag::for_reasoning_render_test();
    app.open_moa_deck(None);
    app.select_moa_card(crate::agent::formations::FormationId::Duel);
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
    assert_eq!(
        deck.focus(),
        crate::agent::formations::MoaDeckFocus::Efforts
    );
    assert_eq!(deck.effort_options(), ["low", "medium", "high"]);
    // Nothing staged: cursor on env default; Up wraps onto "high".
    assert!(app.moa_deck_key(KeyCode::Up, KeyModifiers::NONE));
    assert!(app.moa_deck_key(KeyCode::Enter, KeyModifiers::NONE));
    let deck = app.moa_deck.as_ref().unwrap();
    assert_eq!(deck.focus(), crate::agent::formations::MoaDeckFocus::Slots);
    assert_eq!(
        deck.selected_roster()
            .role_effort(crate::agent::formations::FormationRole::Judge),
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
    app.bag = crate::agent::club::Bag::for_render_test(&[("alpha", &[("model-a", true)])]);
    app.open_moa_deck(None);

    // Solo Strike has no seats at all.
    assert!(app.moa_deck_key(KeyCode::Char('t'), KeyModifiers::NONE));
    assert!(
        app.messages
            .iter()
            .any(|message| message.text.contains("no formation seats"))
    );

    // Recon's scout seat: the swarm carries no scout effort policy.
    app.select_moa_card(crate::agent::formations::FormationId::Recon);
    assert!(app.moa_deck.as_mut().unwrap().select_slot(0));
    assert!(app.moa_deck_key(KeyCode::Char('t'), KeyModifiers::NONE));
    assert!(
        app.messages
            .iter()
            .any(|message| message.text.contains("no scout seat effort"))
    );
    assert_ne!(
        app.moa_deck.as_ref().unwrap().focus(),
        crate::agent::formations::MoaDeckFocus::Efforts
    );

    // Tag Team P1 finds no fleet box here: effort staging needs a route.
    app.select_moa_card(crate::agent::formations::FormationId::TagTeam);
    assert!(app.moa_deck.as_mut().unwrap().select_slot(0));
    assert!(app.moa_deck_key(KeyCode::Char('t'), KeyModifiers::NONE));
    assert!(
        app.messages
            .iter()
            .any(|message| message.text.contains("has no model"))
    );
    assert_ne!(
        app.moa_deck.as_ref().unwrap().focus(),
        crate::agent::formations::MoaDeckFocus::Efforts
    );
}

/// Arming a roster with a staged THINK column names it in the receipt.
#[test]
fn arm_receipt_names_the_staged_think_column() {
    let _lock = crate::tests::env_lock();
    let mut app = crate::seed_preview_app();
    app.bag = crate::agent::club::Bag::for_reasoning_render_test();
    app.open_moa_deck(None);
    app.select_moa_card(crate::agent::formations::FormationId::Duel);
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

fn duel_engagement(app: &crate::App) -> crate::agent::formations::MoaEngagement {
    use crate::agent::formations::{FormationId, FormationRoster, MoaEngagement};
    let roster = FormationRoster::new(FormationId::Duel, &app.bag.moa_model_choices());
    MoaEngagement::new(FormationId::Duel, roster).expect("roster matches its formation")
}

fn duel_app() -> crate::App {
    let mut app = crate::seed_preview_app();
    app.bag = crate::agent::club::Bag::for_render_test(&[
        ("alpha", &[("model-a", true)]),
        ("beta", &[("model-b", true)]),
    ]);
    app
}

/// The words the chip, the label and the status speak for each armed state
/// stay as they are.
#[test]
fn the_formation_chip_words_for_each_armed_state_are_pinned() {
    let _lock = crate::tests::env_lock();
    let mut app = duel_app();
    let name =
        crate::agent::formations::formation(crate::agent::formations::FormationId::Duel).name;
    assert_eq!(app.moa_arm_chip(), None);
    assert_eq!(app.moa_formation_chip_label(), "FORMATION");
    assert_eq!(app.moa_arm_status(), "Solo · normal turns");

    app.moa_one_shot = Some(duel_engagement(&app));
    let chip = app.moa_arm_chip().expect("a one-shot shows a chip");
    assert!(
        chip.starts_with(&format!(" · {name} ")) && chip.ends_with("armed "),
        "{chip}"
    );
    assert_eq!(app.moa_formation_chip_label(), format!("{name}/next"));
    assert_eq!(app.moa_arm_status(), format!("{name} · next"));

    app.moa_one_shot = None;
    app.moa_session = Some(duel_engagement(&app));
    let chip = app.moa_arm_chip().expect("a session shows a chip");
    assert!(
        chip.starts_with(&format!(" · {name} ")) && chip.ends_with("session "),
        "{chip}"
    );
    assert_eq!(app.moa_formation_chip_label(), format!("{name}/sess"));
    assert_eq!(app.moa_arm_status(), format!("{name} · session"));

    app.moa_one_shot = Some(duel_engagement(&app));
    assert!(app.moa_arm_chip().unwrap().ends_with("armed "));
    assert_eq!(app.moa_formation_chip_label(), format!("{name}/next"));
    assert_eq!(
        app.moa_arm_status(),
        format!("{name} next · {name} session")
    );
}

/// A one-shot formation is consumed when its turn starts, but the formation's
/// wrapper stays the route until the worker settles. For that whole run the
/// chip, the label and the status must say a formation is engaged, not idle.
#[test]
fn a_running_one_shot_formation_does_not_read_as_idle() {
    let _lock = crate::tests::env_lock();
    let mut app = duel_app();
    app.moa_one_shot = Some(duel_engagement(&app));
    assert!(app.apply_armed_moa_to_turn());
    assert!(app.moa_one_shot.is_none(), "the turn consumed the one-shot");
    assert_eq!(app.bag.in_hand_mode().as_deref(), Some("sota-moa"));

    let chip = app.moa_arm_chip();
    assert!(
        chip.as_deref().is_some_and(|chip| chip.contains("engaged")),
        "the composer chip is silent while the formation runs: {chip:?}"
    );
    assert_ne!(app.moa_formation_chip_label(), "FORMATION");
    assert!(
        !app.moa_arm_status().contains("Solo"),
        "the rail says {:?} while the formation runs",
        app.moa_arm_status()
    );

    app.restore_moa_after_turn();
    assert_eq!(app.moa_arm_chip(), None);
    assert_eq!(app.moa_formation_chip_label(), "FORMATION");
    assert_eq!(app.moa_arm_status(), "Solo · normal turns");
}

/// A session roster staged behind a running one-shot waits: the run is still
/// engaged, the status names the session that comes after, and the session
/// is what the chip shows once the worker settles.
#[test]
fn a_session_staged_behind_a_running_one_shot_waits_for_the_worker() {
    use super::MoaPhase;
    let _lock = crate::tests::env_lock();
    let mut app = duel_app();
    let name =
        crate::agent::formations::formation(crate::agent::formations::FormationId::Duel).name;
    app.moa_session = Some(duel_engagement(&app));
    app.moa_one_shot = Some(duel_engagement(&app));
    assert_eq!(app.moa_phase(), MoaPhase::NextTurn);
    assert!(app.apply_armed_moa_to_turn());
    assert_eq!(app.moa_phase(), MoaPhase::Engaged);
    assert!(app.moa_arm_chip().unwrap().contains("engaged"));
    assert_eq!(
        app.moa_arm_status(),
        format!("engaged · this turn · {name} session")
    );

    app.restore_moa_after_turn();
    assert_eq!(app.moa_phase(), MoaPhase::Session);
    assert_eq!(app.moa_formation_chip_label(), format!("{name}/sess"));
}

/// Clearing the formation while its turn runs is a restore owed at the
/// settle: the run is still under the formation until then.
#[test]
fn clearing_a_formation_mid_run_reads_engaged_until_the_worker_settles() {
    use super::MoaPhase;
    use crate::agent::turn::Thinking;
    let _lock = crate::tests::env_lock();
    let mut app = duel_app();
    app.moa_one_shot = Some(duel_engagement(&app));
    assert!(app.apply_armed_moa_to_turn());
    app.thinking = Some(Thinking::pending_for_test("practice"));
    app.clear_moa_cards();
    assert_eq!(app.moa_phase(), MoaPhase::Engaged);

    app.thinking = None;
    app.restore_moa_after_turn();
    assert_eq!(app.moa_phase(), MoaPhase::Idle);
    assert_eq!(app.bag.in_hand_label(), "alpha");
}

/// What the operator stages while a one-shot runs is the next turn's arm, and
/// the chip shows it.
#[test]
fn staging_a_one_shot_during_a_run_shows_the_arm() {
    use super::MoaPhase;
    let _lock = crate::tests::env_lock();
    let mut app = duel_app();
    app.moa_one_shot = Some(duel_engagement(&app));
    assert!(app.apply_armed_moa_to_turn());
    assert_eq!(app.moa_phase(), MoaPhase::Engaged);
    app.moa_one_shot = Some(duel_engagement(&app));
    assert_eq!(app.moa_phase(), MoaPhase::NextTurn);
    assert!(app.moa_arm_chip().unwrap().ends_with("armed "));
}

/// The deck's draft pick is staging, not an arm, and the open deck never
/// outranks an engaged run.
#[test]
fn the_open_deck_is_staging_until_something_is_armed() {
    use super::MoaPhase;
    let _lock = crate::tests::env_lock();
    let mut app = duel_app();
    assert_eq!(app.moa_phase(), MoaPhase::Idle);
    assert!(!app.moa_phase().is_live());
    app.open_moa_deck(None);
    assert_eq!(app.moa_phase(), MoaPhase::Staging);
    assert!(!app.moa_phase().is_live());
    app.moa_one_shot = Some(duel_engagement(&app));
    assert!(app.apply_armed_moa_to_turn());
    assert_eq!(app.moa_phase(), MoaPhase::Engaged);
    assert!(app.moa_phase().is_live());
}
