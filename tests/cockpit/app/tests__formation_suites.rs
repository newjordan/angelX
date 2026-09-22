//! MoA/formation suites (module-breakup: extracted from the `tests.rs`
//! monolith). Formation decks, seat assignment, GPU-comp and Grok-war env
//! contracts, roster lifecycle, and the compatibility-state protections.

use super::{mouse_ev, render_app_text, seed_preview_app};
use crate::agent::club::Bag;
use crate::agent::club::ChatRole;
use crate::agent::formations::{self};
use crate::agent::turn::Thinking;
use crate::app::{AgentButton, WorldButton};
use crate::tests::env_lock;
use crate::ui::scryglass;
use crate::ui::transcript::Role;
use std::sync::Arc;

#[test]
fn moa_command_opens_formation_deck_without_starting_turn() {
    use ratatui::crossterm::event::{MouseButton, MouseEventKind};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[
        ("spark", &[("qwen-local", true)]),
        ("openai", &[("gpt-frontier", true)]),
        ("grok", &[("grok-scout", true)]),
    ]);
    app.input = "/moa".to_string();
    app.cursor = app.input.chars().count();
    app.submit();
    assert!(app.moa_deck.is_some(), "deck should be open");
    assert!(app.thinking.is_none(), "opening the deck is local UI work");

    let text = render_app_text(&mut app, 96, 36);
    assert!(
        text.contains("Agent Formation · Solo · normal turns"),
        "deck title should render\n{text}"
    );
    assert!(
        text.contains("Solo Strike"),
        "normal-turn formation renders at the top\n{text}"
    );
    assert!(
        text.contains("GPU Night Shift"),
        "specialized formation renders\n{text}"
    );
    assert!(
        text.contains("Grok War"),
        "frontier war formation renders\n{text}"
    );
    assert!(
        text.contains("Math God"),
        "lean/math formation renders\n{text}"
    );
    assert!(text.contains("Council"), "formation presets render\n{text}");
    assert!(
        app.world_buttons.iter().any(|(_, button)| matches!(
            button,
            WorldButton::SelectFormation(crate::agent::formations::FormationId::GrokWar)
        )),
        "Grok War row must be mouse-selectable"
    );
    assert!(
        text.contains("Engage next"),
        "roster controls render\n{text}"
    );
    assert!(
        text.contains("[Engage session]"),
        "session control renders\n{text}"
    );
    assert!(
        app.world_buttons.iter().any(|(_, button)| matches!(
            button,
            WorldButton::SelectFormation(crate::agent::formations::FormationId::Recon)
        )),
        "formation rows must be mouse-selectable"
    );
    for action in [
        WorldButton::ArmFormationTurn,
        WorldButton::ArmFormationSession,
        WorldButton::ClearFormation,
        WorldButton::Back,
    ] {
        assert!(
            app.world_buttons
                .iter()
                .any(|(_, button)| *button == action),
            "missing formation action {action:?}"
        );
    }

    let recon = app
        .world_buttons
        .iter()
        .find(|(_, button)| {
            matches!(
                button,
                WorldButton::SelectFormation(crate::agent::formations::FormationId::Recon)
            )
        })
        .map(|(rect, _)| *rect)
        .expect("Recon formation row hitbox");
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        recon.x,
        recon.y,
    ));
    assert_eq!(
        app.moa_deck.as_ref().map(|deck| deck.selected().id),
        Some(crate::agent::formations::FormationId::Recon),
        "first click previews the formation without arming it"
    );
    assert!(
        app.moa_arm_status().contains("Outriders") && app.moa_arm_status().contains("draft"),
        "world rail must show the draft pick, not stale Solo — got {}",
        app.moa_arm_status()
    );
    assert!(app.moa_one_shot.is_none(), "first click must not arm");
    // Re-click the selected card locks it in for the next turn.
    let recon = app
        .world_buttons
        .iter()
        .find(|(_, button)| {
            matches!(
                button,
                WorldButton::SelectFormation(crate::agent::formations::FormationId::Recon)
            )
        })
        .map(|(rect, _)| *rect)
        .expect("Recon formation row hitbox after redraw");
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        recon.x,
        recon.y,
    ));
    assert!(
        app.moa_deck.is_none(),
        "second click arms and closes the deck"
    );
    assert_eq!(
        app.moa_one_shot.as_ref().map(|armed| armed.formation),
        Some(crate::agent::formations::FormationId::Recon)
    );
    assert!(
        app.moa_arm_status().contains("Outriders") && app.moa_arm_status().contains("next"),
        "armed status must leave Solo — got {}",
        app.moa_arm_status()
    );

    // Re-open to inspect the roster graph for the remaining assertions.
    app.open_moa_deck(None);
    app.select_moa_card(crate::agent::formations::FormationId::Recon);
    let graph = render_app_text(&mut app, 144, 48);
    assert!(graph.contains("PROPOSE"), "proposal nodes missing\n{graph}");
    assert!(graph.contains("SCOUT"), "scout node missing\n{graph}");
    assert!(graph.contains("SYNTH"), "synthesis node missing\n{graph}");
    assert!(
        graph.contains("READY 4/4"),
        "roster readiness missing\n{graph}"
    );
    assert!(
        graph.contains("SPEND") && graph.contains("native output"),
        "formation must describe adaptive spend and native generation\n{graph}"
    );
    assert!(
        !graph.contains("plan 35k tokens"),
        "formation must not advertise a fictitious token ceiling\n{graph}"
    );
}

#[test]
fn moa_graph_slot_opens_model_picker_and_assigns_only_that_seat() {
    use ratatui::crossterm::event::{MouseButton, MouseEventKind};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[
        ("alpha", &[("model-a", true)]),
        ("beta", &[("model-b", true)]),
    ]);
    app.open_moa_deck(None);
    app.select_moa_card(crate::agent::formations::FormationId::Duel);
    let _ = render_app_text(&mut app, 144, 48);
    let seat = app
        .world_buttons
        .iter()
        .find(|(_, button)| matches!(button, WorldButton::SelectFormationSlot(0)))
        .map(|(rect, _)| *rect)
        .expect("first graph seat hitbox");
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        seat.x,
        seat.y,
    ));
    let picker = render_app_text(&mut app, 144, 48);
    assert!(picker.contains("Assign model · P1"), "{picker}");
    assert!(picker.contains("alpha / model-a"), "{picker}");
    assert!(picker.contains("beta / model-b"), "{picker}");

    let model_b_index = app
        .moa_deck
        .as_ref()
        .unwrap()
        .models()
        .iter()
        .position(|choice| choice.route.model == "model-b")
        .expect("model-b choice");
    let model_b = app
        .world_buttons
        .iter()
        .find(|(_, button)| *button == WorldButton::AssignFormationModel(model_b_index))
        .map(|(rect, _)| *rect)
        .expect("model-b picker hitbox");
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        model_b.x,
        model_b.y,
    ));
    let deck = app.moa_deck.as_ref().unwrap();
    assert_eq!(
        deck.selected_roster()
            .assignment(0)
            .map(|route| route.model.as_str()),
        Some("model-b")
    );
    assert_eq!(
        deck.selected_roster()
            .assignment(1)
            .map(|route| route.model.as_str()),
        Some("model-a"),
        "assigning P1 must not rewrite P2"
    );
}

#[test]
fn mathgod_bag_club_cannot_be_a_seat_inside_another_formation() {
    let _guard = env_lock();
    let mut bag = Bag::for_render_test(&[
        ("mathgod", &[("mathgod", true)]),
        ("openai", &[("gpt-5.6-sol", true)]),
        ("glm", &[("glm-5.3", true)]),
    ]);
    let models = bag.moa_model_choices();
    assert!(
        models
            .iter()
            .all(|choice| !choice.route.agent.eq_ignore_ascii_case("mathgod")),
        "mathgod is a formation wrapper, so it must not be offered as a seat: {models:?}"
    );
    // Fail-closed even if a roster is hand-built around the wrapper tab.
    let mathgod = formations::MoaModelRef {
        agent_index: 0,
        slot_index: 0,
        agent: "mathgod".into(),
        driver: "practice".into(),
        model: "mathgod".into(),
        route_id: crate::agent::backplane::RouteId::chat("mathgod", "practice", None),
        expected_revision: crate::agent::backplane::ModelRevision::chat("mathgod"),
        metered: false,
    };
    let mut roster = formations::FormationRoster::new(formations::FormationId::Duel, &models);
    assert!(roster.assign(0, mathgod));
    let err = bag
        .activate_sota_moa_with_roster(&roster)
        .expect_err("mathgod is a formation, not a seat inside another formation");
    assert!(
        err.contains("not a concrete MoA seat"),
        "unexpected rejection: {err}"
    );
}

#[test]
fn exact_roster_installs_internal_moa_wrapper_from_concrete_routes() {
    let mut bag = Bag::for_render_test(&[
        ("alpha", &[("model-a", true)]),
        ("beta", &[("model-b", true)]),
    ]);
    let models = bag.moa_model_choices();
    let roster = formations::FormationRoster::new(formations::FormationId::Duel, &models);
    let seats = bag
        .activate_sota_moa_with_roster(&roster)
        .expect("exact roster builds an MoA wrapper");
    assert_eq!(seats, roster.slot_count());
    assert_eq!(bag.in_hand_label(), "sota");
    assert_eq!(bag.in_hand_mode().as_deref(), Some("sota-moa"));
    assert!(
        bag.route_choices()
            .iter()
            .all(|choice| choice.driver != "sota-moa" && choice.model != "sota-moa"),
        "the internal wrapper must not bypass the Formation board via model selection"
    );
}

#[test]
fn one_shot_moa_restores_the_previous_concrete_route_after_turn() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[
        ("alpha", &[("model-a", true)]),
        ("beta", &[("model-b", true)]),
    ]);
    let previous = app.bag.selected_route_indices();
    let roster = formations::FormationRoster::new(
        formations::FormationId::Duel,
        &app.bag.moa_model_choices(),
    );
    app.moa_one_shot = formations::MoaEngagement::new(formations::FormationId::Duel, roster);

    assert!(app.apply_armed_moa_to_turn());
    assert_eq!(app.bag.in_hand_mode().as_deref(), Some("sota-moa"));
    assert!(app.moa_restore_after_turn);

    app.restore_moa_after_turn();
    assert_eq!(app.bag.selected_route_indices(), previous);
    assert_eq!(app.bag.in_hand_label(), "alpha");
    assert!(app.moa_restore_route.is_none());
    assert!(!app.moa_restore_after_turn);
}

#[test]
fn armed_moa_submit_keeps_operator_text_exact_and_engages_typed_route() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[
        ("alpha", &[("model-a", true)]),
        ("beta", &[("model-b", true)]),
    ]);
    let roster = formations::FormationRoster::new(
        formations::FormationId::Duel,
        &app.bag.moa_model_choices(),
    );
    app.moa_one_shot = formations::MoaEngagement::new(formations::FormationId::Duel, roster);

    let operator_text = "compare the plans exactly as written";
    app.input = operator_text.to_string();
    app.submit();

    let task_index = app
        .history
        .iter()
        .rposition(|message| message.role == ChatRole::User)
        .expect("operator task");
    assert_eq!(&*app.history[task_index].content, operator_text);
    assert!(!app.history[task_index].content.starts_with("moa:"));
    assert!(
        app.history
            .iter()
            .all(|message| !message.content.starts_with("moa: ")),
        "formation engagement must live in typed route state, not prompt text"
    );
    assert_eq!(app.bag.in_hand_mode().as_deref(), Some("sota-moa"));

    if app.thinking.is_some() {
        app.interrupt();
        app.interrupt();
    }
}

#[test]
fn stale_moa_roster_fails_closed_and_preserves_the_message_draft() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[("alpha", &[("model-a", true)])]);
    let roster = formations::FormationRoster::new(
        formations::FormationId::Duel,
        &app.bag.moa_model_choices(),
    );
    app.moa_one_shot = formations::MoaEngagement::new(formations::FormationId::Duel, roster);
    app.bag.set_route_available_for_test(0, 0, false);
    let history_before = app.history.len();

    app.input = "/moa compare the plans".to_string();
    app.cursor = app.input.chars().count();
    app.submit();

    assert!(
        app.thinking.is_none(),
        "a stale roster must not send a fallback turn"
    );
    assert_eq!(app.history.len(), history_before);
    assert_eq!(app.input, "compare the plans");
    assert!(
        app.moa_deck.is_some(),
        "the roster should reopen for repair"
    );
    assert!(
        app.moa_one_shot.is_some(),
        "the staged roster should remain armed"
    );
    assert!(app.messages.iter().any(|message| {
        message
            .text
            .contains("draft kept for repair; no turn was sent")
    }));
}

#[test]
fn deferred_submit_echoes_before_goal_reload_and_moa_engage() {
    let _guard = env_lock();
    let tmp = std::env::temp_dir().join(format!(
        "angel_defer_submit_goal_{}.json",
        std::process::id()
    ));
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_GOAL_FILE", &tmp) };
    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[
        ("alpha", &[("model-a", true)]),
        ("beta", &[("model-b", true)]),
    ]);
    app.input = "/goal ship from disk".to_string();
    app.submit();
    assert_eq!(
        app.goal.as_ref().map(|goal| goal.text.as_str()),
        Some("ship from disk")
    );
    app.goal = None;
    let roster = formations::FormationRoster::new(
        formations::FormationId::Duel,
        &app.bag.moa_model_choices(),
    );
    app.moa_one_shot = formations::MoaEngagement::new(formations::FormationId::Duel, roster);
    app.submit_deferral = true;
    let operator_text = "do the thing";
    app.input = operator_text.to_string();
    app.submit();

    assert!(
        app.pending_turn.is_some(),
        "interactive submit parks the turn until the echo frame"
    );
    assert!(
        app.thinking.is_none(),
        "worker must not start on the echo tick"
    );
    assert!(
        app.messages.last().is_some_and(|message| {
            matches!(message.role, Role::User) && message.text.as_ref() == operator_text
        }),
        "User echo must paint before goal reload or formation engage"
    );
    assert!(
        app.goal.is_none(),
        "goal store must not be read on the echo tick"
    );
    assert_ne!(
        app.bag.in_hand_mode().as_deref(),
        Some("sota-moa"),
        "armed formation must not engage on the echo tick"
    );
    assert!(
        app.history
            .iter()
            .all(|message| message.content.as_ref() != operator_text),
        "model history stays untouched until launch_pending_turn"
    );
    let echo = Arc::clone(&app.messages.last().unwrap().text);
    let pending = app.pending_turn.as_ref().expect("parked turn");
    assert!(
        Arc::ptr_eq(&echo, &pending.raw)
            && Arc::ptr_eq(&echo, &pending.user_msg.content)
            && Arc::ptr_eq(&echo, &pending.retry_draft),
        "echo, pending raw, history payload, and retry share one operator buffer"
    );

    app.launch_pending_turn();
    assert!(app.pending_turn.is_none());
    assert_eq!(
        app.goal.as_ref().map(|goal| goal.text.as_str()),
        Some("ship from disk")
    );
    assert_eq!(app.bag.in_hand_mode().as_deref(), Some("sota-moa"));
    assert!(
        app.history
            .iter()
            .any(|message| message.role == ChatRole::User
                && message.content.as_ref() == operator_text
                && Arc::ptr_eq(&message.content, &echo)),
        "launched history must keep the echo buffer"
    );

    if app.thinking.is_some() {
        app.interrupt();
        app.interrupt();
    }
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_GOAL_FILE") };
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn deferred_submit_restores_draft_when_moa_engage_fails() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[("alpha", &[("model-a", true)])]);
    let roster = formations::FormationRoster::new(
        formations::FormationId::Duel,
        &app.bag.moa_model_choices(),
    );
    app.moa_one_shot = formations::MoaEngagement::new(formations::FormationId::Duel, roster);
    app.bag.set_route_available_for_test(0, 0, false);
    let history_before = app.history.len();
    app.submit_deferral = true;
    app.input = "/moa compare the plans".to_string();
    app.cursor = app.input.chars().count();
    app.submit();

    assert_eq!(
        app.messages.last().map(|message| message.text.as_ref()),
        Some("/moa compare the plans"),
        "echo paints even when the parked formation will fail to engage"
    );
    assert!(app.pending_turn.is_some());
    assert!(app.thinking.is_none());
    assert_eq!(app.history.len(), history_before);

    app.launch_pending_turn();
    assert!(app.pending_turn.is_none());
    assert!(
        app.thinking.is_none(),
        "a stale roster must not send a fallback turn"
    );
    assert_eq!(app.history.len(), history_before);
    assert_eq!(app.input, "compare the plans");
    assert!(
        !app.messages
            .iter()
            .any(|message| matches!(message.role, Role::User)
                && message.text.as_ref() == "/moa compare the plans"),
        "failed engage must unpaint the echo so the operator is not stuck with a cancelled launch"
    );
    assert!(
        app.moa_deck.is_some(),
        "the roster should reopen for repair"
    );
    assert!(
        app.moa_one_shot.is_some(),
        "the staged roster should remain armed"
    );
    assert!(app.messages.iter().any(|message| {
        message
            .text
            .contains("draft kept for repair; no turn was sent")
    }));
}

#[test]
fn clearing_a_session_moa_restores_the_pre_moa_route() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[
        ("alpha", &[("model-a", true)]),
        ("beta", &[("model-b", true)]),
    ]);
    let previous = app.bag.selected_route_indices();
    let roster = formations::FormationRoster::new(
        formations::FormationId::Duel,
        &app.bag.moa_model_choices(),
    );
    app.moa_session = formations::MoaEngagement::new(formations::FormationId::Duel, roster);
    assert!(app.apply_armed_moa_to_turn());
    assert_eq!(app.bag.in_hand_mode().as_deref(), Some("sota-moa"));

    app.clear_moa_cards();
    assert_eq!(app.bag.selected_route_indices(), previous);
    assert_eq!(app.bag.in_hand_label(), "alpha");
    assert!(app.moa_session.is_none());
    assert!(app.moa_restore_route.is_none());
}

#[test]
fn header_formation_control_opens_the_canonical_deck() {
    use ratatui::crossterm::event::{MouseButton, MouseEventKind};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    let world = render_app_text(&mut app, 144, 48);
    assert!(
        world.contains("FORMATION"),
        "formation rail missing\n{world}"
    );
    assert!(
        !world.contains("horde") && !world.contains("◈75%"),
        "obsolete preset/quorum controls leaked into the world\n{world}"
    );
    let formation = app
        .agent_buttons
        .iter()
        .find(|(_, button)| *button == crate::app::AgentButton::MoaDeck)
        .map(|(rect, _)| *rect)
        .expect("formation rail hitbox");
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        formation.x,
        formation.y,
    ));
    assert!(app.moa_deck.is_some(), "formation rail opens the deck");
}

#[test]
fn moa_selection_motion_never_arms_or_changes_formation_execution() {
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    let mut app = seed_preview_app();
    app.open_moa_deck(None);
    let before_history = app.history.len();
    assert!(app.moa_deck_key(KeyCode::Down, KeyModifiers::NONE));
    let deck = app.moa_deck.as_ref().unwrap();
    assert_eq!(
        deck.selected().id,
        crate::agent::formations::FormationId::Recon
    );
    assert!(deck.transition().is_some());
    assert!(app.moa_one_shot.is_none());
    assert!(app.moa_session.is_none());
    assert_eq!(app.history.len(), before_history);
    assert!(app.thinking.is_none());
}

#[test]
fn formation_board_owns_printable_input_but_not_global_focus_or_interrupts() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::Rect;

    for shortcut in ['v', 'w', '[', ']', 'p', 'r', 'x'] {
        let mut app = seed_preview_app();
        app.input = "preserved draft".to_string();
        app.cursor = app.input.chars().count();
        app.open_moa_deck(None);
        app.on_key(KeyEvent::new(KeyCode::Char(shortcut), KeyModifiers::NONE));
        assert_eq!(
            app.scryglass.controller.route(),
            crate::ui::scryglass::StageRoute::Formation,
            "Formation leaked {shortcut:?} into Stage navigation"
        );
        assert_eq!(
            app.input, "preserved draft",
            "Formation leaked {shortcut:?} into the hidden composer"
        );
        assert!(app.moa_deck.is_some());

        let mut empty = seed_preview_app();
        empty.open_moa_deck(None);
        empty.on_key(KeyEvent::new(KeyCode::Char(shortcut), KeyModifiers::NONE));
        assert_eq!(
            empty.scryglass.controller.route(),
            crate::ui::scryglass::StageRoute::Formation,
            "empty Formation leaked {shortcut:?} into Stage navigation"
        );
        assert!(empty.input.is_empty());
        assert!(empty.moa_deck.is_some());
    }

    let mut app = seed_preview_app();
    app.input = "preserved draft".to_string();
    app.cursor = app.input.chars().count();
    app.open_moa_deck(None);
    app.on_key(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::SHIFT));
    app.on_paste(" pasted");
    assert_eq!(app.input, "preserved draft");
    assert_eq!(
        app.scryglass.controller.route(),
        crate::ui::scryglass::StageRoute::Formation
    );

    app.on_key(KeyEvent::new(KeyCode::F(3), KeyModifiers::NONE));
    assert_eq!(
        app.module_host.focused().map(|id| id.as_str()),
        Some("agent"),
        "global focus keys remain available"
    );
    app.on_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    assert_eq!(
        app.input, "preserved draftx",
        "an explicit focus change returns input to the composer"
    );
    let composer = crate::ui::views::status_view::composer_view(&app.input, 38, 2, app.cursor);
    assert!(
        crate::ui::draw::composer_cursor_position(&app, Rect::new(1, 1, 40, 4), &composer)
            .is_some()
    );

    app.focus_module("artifacts");
    assert!(!app.moa_deck_key(KeyCode::Char('g'), KeyModifiers::CONTROL));
    assert!(!app.moa_deck_key(KeyCode::Char('c'), KeyModifiers::CONTROL));
    app.shell_focused = true;
    assert!(!app.moa_deck_key(KeyCode::Char('x'), KeyModifiers::NONE));
    app.shell_focused = false;

    app.thinking = Some(Thinking::pending_for_test("practice"));
    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.moa_deck.is_some(), "busy Esc must not close Formation");
    assert!(
        app.messages
            .last()
            .is_some_and(|message| message.text.contains("interrupting after the current step"))
    );
}

#[test]
fn unarmed_moa_message_opens_roster_and_preserves_the_composer() {
    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[("alpha", &[("model-a", true)])]);
    let history_before = app.history.len();
    app.input = "/moa compare the two plans".to_string();
    app.cursor = app.input.chars().count();
    app.submit();
    assert!(app.moa_deck.is_some());
    assert_eq!(app.input, "compare the two plans");
    assert_eq!(app.history.len(), history_before);
    assert!(app.messages.last().is_some_and(|message| {
        message
            .text
            .contains("Draft and engage a formation roster first")
    }));
}

#[test]
fn gpu_comp_formation_sets_sleep_loop_env_contract() {
    let _guard = env_lock();
    // Clear so formation can install Treebeard living-subject defaults.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_LANE") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_TASK_TREEBEARD") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_TRAJECTORY_LOG") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_ROOT_TRAJECTORY") };
    crate::agent::formations::formation(crate::agent::formations::FormationId::GpuComp)
        .apply_sota_env();
    assert_eq!(
        std::env::var("ANGEL_GPU_COMP_LOCAL_MOA").as_deref(),
        Ok("1")
    );
    assert_eq!(
        std::env::var("GPU_COMP_TURBO_MAX_AGENTS").as_deref(),
        Ok("12")
    );
    assert_eq!(
        std::env::var("GPU_COMP_DICE_ROLE").as_deref(),
        Ok("advisor")
    );
    assert_eq!(
        std::env::var("GPU_COMP_GROK_ROLE").as_deref(),
        Ok("kernel-fix-scout")
    );
    assert_eq!(
        std::env::var("GPU_COMP_OPENAI_INTERVAL_MIN").as_deref(),
        Ok("45")
    );
    // Scout=true enables Grok research on deliberate MoA turns.
    assert_eq!(
        std::env::var("ANGEL_SOTA_MOA_GROK_RESEARCH").as_deref(),
        Ok("1")
    );
    assert_eq!(std::env::var("ANGEL_GROK_TOOL").as_deref(), Ok("1"));
    assert!(
        crate::agent::formations::formation(crate::agent::formations::FormationId::GpuComp).scout,
        "GpuComp formation must include the Grok scout seat"
    );
    assert_eq!(std::env::var("ANGEL_LANE").as_deref(), Ok("treebeard"));
    assert_eq!(std::env::var("ANGEL_TRAJECTORY_LOG").as_deref(), Ok("1"));
    assert_eq!(std::env::var("ANGEL_ROOT_TRAJECTORY").as_deref(), Ok("1"));
    // Phase-4: default popcorn peer scorer for coding seats (explicit wins).
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_RL_REWARD") };
    crate::agent::formations::formation(crate::agent::formations::FormationId::GpuComp)
        .apply_sota_env();
    assert_eq!(
        std::env::var("ANGEL_RL_REWARD").as_deref(),
        Ok("popcorn_peer")
    );
    assert_eq!(
        crate::drive::reinforce::reward_from_env().label(),
        "popcorn_peer",
        "ANGEL_RL_REWARD must resolve to PopcornPeerReward"
    );
    // When living peer state exists, POPCORN_PEER_LOG is pinned for submit-hiq.
    if crate::agent::harness::load_living_peer_snapshot().is_some() {
        // Only assert when peer file is present and path is a real file.
        if let Ok(peer_log) = std::env::var("POPCORN_PEER_LOG") {
            assert!(
                std::path::Path::new(&peer_log).is_file(),
                "POPCORN_PEER_LOG should be a real log file: {peer_log}"
            );
        }
    }
    // Explicit ReAct control pin must not be overwritten on re-apply.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_LANE", "default") };
    crate::agent::formations::formation(crate::agent::formations::FormationId::GpuComp)
        .apply_sota_env();
    assert_eq!(std::env::var("ANGEL_LANE").as_deref(), Ok("default"));
    crate::agent::formations::formation(crate::agent::formations::FormationId::SoloStrike)
        .apply_sota_env();
    assert!(std::env::var("ANGEL_GPU_COMP_LOCAL_MOA").is_err());
    // Formation-default popcorn scorer clears when leaving gpu-comp.
    assert!(
        std::env::var("ANGEL_RL_REWARD").is_err(),
        "leaving gpu-comp should drop popcorn_peer default"
    );
}

#[test]
fn grok_war_command_selects_and_enter_engages_formation() {
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[
        ("glm", &[("glm-5.2", true)]),
        ("deepseek", &[("deepseek-v4-pro", true)]),
        ("openai", &[("gpt-5.6-sol", true)]),
        ("grok", &[("grok-4.5", true)]),
    ]);

    // `/moa war` must open the deck on Grok War — not send a MoA chat message.
    app.input = "/moa war".to_string();
    app.cursor = app.input.chars().count();
    app.submit();
    assert!(app.thinking.is_none(), "opening Grok War is local UI work");
    assert_eq!(
        app.moa_deck.as_ref().map(|deck| deck.selected().id),
        Some(formations::FormationId::GrokWar),
        "war alias must land on the Grok War card"
    );
    let deck = app.moa_deck.as_ref().expect("deck open");
    assert!(
        deck.selected_roster().is_ready(),
        "recommended trio+Grok seats must auto-fill when routes are online"
    );

    // Enter locks the selected card for the next turn (card-battle engage).
    app.focus_module("artifacts");
    assert!(app.moa_deck_key(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.moa_deck.is_none(), "engage closes the deck");
    assert_eq!(
        app.moa_one_shot.as_ref().map(|armed| armed.formation),
        Some(formations::FormationId::GrokWar)
    );
    assert!(app.moa_session.is_none());
    assert!(
        app.messages
            .last()
            .is_some_and(|message| message.text.contains("Grok War")
                && message.text.contains("armed for one turn")),
        "{:?}",
        app.messages.last().map(|message| &message.text)
    );
}

#[test]
fn tag_team_command_selects_the_two_local_corners_not_council() {
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[
        ("spark", &[("leanstral-24b", true)]),
        ("turbo", &[("qwen3-30b-a3b", true)]),
        ("openai", &[("gpt-5.6-sol", true)]),
    ]);

    app.input = "/moa tag-team".to_string();
    app.cursor = app.input.chars().count();
    app.submit();
    assert!(app.thinking.is_none(), "opening Tag Team is local UI work");
    assert_eq!(
        app.moa_deck.as_ref().map(|deck| deck.selected().id),
        Some(formations::FormationId::TagTeam),
        "the canonical tag-team slug must land on Tag Team, not Council"
    );
    let deck = app.moa_deck.as_ref().expect("deck open");
    assert_eq!(
        deck.selected_roster().slot_count(),
        2,
        "Tag Team is two models, not the council panel"
    );
    assert_eq!(deck.selected().name, "Tag Team");
    assert!(
        deck.selected_roster().is_ready(),
        "the two local corners must auto-fill when those boxes are online"
    );
    let labels: Vec<_> = deck
        .selected_roster()
        .assignments()
        .iter()
        .map(|a| a.as_ref().map(|r| r.model.as_str()).unwrap_or("?"))
        .collect();
    assert_eq!(labels, vec!["leanstral-24b", "qwen3-30b-a3b"]);

    app.focus_module("artifacts");
    assert!(app.moa_deck_key(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.moa_deck.is_none(), "engage closes the deck");
    assert_eq!(
        app.moa_one_shot.as_ref().map(|armed| armed.formation),
        Some(formations::FormationId::TagTeam)
    );
}

#[test]
fn grok_war_formation_sets_frontier_trio_env_contract() {
    let _guard = env_lock();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_LANE") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_TASK_TREEBEARD") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_SOTA_MOA_COST_PROFILE") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_GROK_REASONING_EFFORT") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_RL_REWARD") };
    crate::agent::formations::formation(crate::agent::formations::FormationId::GrokWar)
        .apply_sota_env();
    assert_eq!(
        std::env::var("ANGEL_SOTA_MOA_PROPOSE_CLUB").as_deref(),
        Ok("glm")
    );
    assert_eq!(
        std::env::var("ANGEL_SOTA_MOA_EXTRA_PROPOSERS").as_deref(),
        Ok("deepseek,openai")
    );
    assert_eq!(
        std::env::var("ANGEL_SOTA_MOA_JUDGE_CLUB").as_deref(),
        Ok("grok")
    );
    assert_eq!(
        std::env::var("ANGEL_SOTA_MOA_AGG_CLUB").as_deref(),
        Ok("grok")
    );
    assert_eq!(
        std::env::var("ANGEL_SOTA_MOA_VERIFY_CLUB").as_deref(),
        Ok("none")
    );
    assert_eq!(std::env::var("ANGEL_GROK_TOOL").as_deref(), Ok("1"));
    assert_eq!(
        std::env::var("ANGEL_SOTA_MOA_GROK_RESEARCH").as_deref(),
        Ok("1")
    );
    assert_eq!(
        std::env::var("ANGEL_OPENAI_MODEL").as_deref(),
        Ok("gpt-5.6-sol")
    );
    assert_eq!(
        std::env::var("ANGEL_OPENAI_REASONING_EFFORT").as_deref(),
        Ok("max")
    );
    // Leaving war must drop the role pins so another formation owns routing.
    crate::agent::formations::formation(crate::agent::formations::FormationId::SoloStrike)
        .apply_sota_env();
    assert!(std::env::var("ANGEL_SOTA_MOA_PROPOSE_CLUB").is_err());
    assert!(std::env::var("ANGEL_SOTA_MOA_EXTRA_PROPOSERS").is_err());
    assert!(std::env::var("ANGEL_SOTA_MOA_JUDGE_CLUB").is_err());
    assert!(std::env::var("ANGEL_SOTA_MOA_AGG_CLUB").is_err());
}

#[test]
fn math_god_command_selects_and_enter_engages_formation() {
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[
        ("glm", &[("glm-5.3", true)]),
        ("openai", &[("gpt-5.6-sol", true)]),
        ("grok", &[("grok-4.6", true)]),
        ("deepseek", &[("deepseek-v4-pro", true)]),
        ("spark", &[("leanstral-24b", true)]),
    ]);

    app.input = "/moa math".to_string();
    app.cursor = app.input.chars().count();
    app.submit();
    assert!(app.thinking.is_none(), "opening Math God is local UI work");
    assert_eq!(
        app.moa_deck.as_ref().map(|deck| deck.selected().id),
        Some(formations::FormationId::MathGod),
        "math alias must land on the Math God card"
    );
    let deck = app.moa_deck.as_ref().expect("deck open");
    assert!(
        deck.selected_roster().is_ready(),
        "recommended Sol+GLM+DeepSeek+Grok seats must auto-fill when routes are online"
    );
    assert_eq!(
        deck.selected_roster()
            .role_effort(crate::agent::formations::FormationRole::Propose),
        Some("ultra")
    );
    assert_eq!(
        deck.selected_roster()
            .role_effort(crate::agent::formations::FormationRole::Judge),
        Some("xhigh")
    );

    app.focus_module("artifacts");
    assert!(app.moa_deck_key(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.moa_deck.is_none(), "engage closes the deck");
    assert_eq!(
        app.moa_one_shot.as_ref().map(|armed| armed.formation),
        Some(formations::FormationId::MathGod)
    );
    assert!(
        app.messages
            .last()
            .is_some_and(|message| message.text.contains("Math God")
                && message.text.contains("armed for one turn")),
        "{:?}",
        app.messages.last().map(|message| &message.text)
    );
}

#[test]
fn math_god_formation_sets_lean_solver_env_contract() {
    let _guard = env_lock();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_SOTA_MOA_COST_PROFILE") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_GROK_REASONING_EFFORT") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_GLM_MODEL") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_GLM_REASONING_EFFORT") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_DEEPSEEK_MODEL") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_DEEPSEEK_REASONING_EFFORT") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_OPENAI_REASONING_EFFORT") };
    crate::agent::formations::formation(crate::agent::formations::FormationId::MathGod)
        .apply_sota_env();
    assert_eq!(
        std::env::var("ANGEL_SOTA_MOA_PROPOSE_CLUB").as_deref(),
        Ok("openai")
    );
    assert_eq!(
        std::env::var("ANGEL_SOTA_MOA_EXTRA_PROPOSERS").as_deref(),
        Ok("glm,deepseek")
    );
    assert_eq!(
        std::env::var("ANGEL_SOTA_MOA_JUDGE_CLUB").as_deref(),
        Ok("grok")
    );
    assert_eq!(
        std::env::var("ANGEL_SOTA_MOA_AGG_CLUB").as_deref(),
        Ok("openai")
    );
    assert_eq!(
        std::env::var("ANGEL_SOTA_MOA_VERIFY_CLUB").as_deref(),
        Ok("none")
    );
    assert_eq!(
        std::env::var("ANGEL_GROK_REASONING_EFFORT").as_deref(),
        Ok("xhigh")
    );
    assert_eq!(
        std::env::var("ANGEL_OPENAI_MODEL").as_deref(),
        Ok("gpt-5.6-sol")
    );
    assert_eq!(
        std::env::var("ANGEL_OPENAI_REASONING_EFFORT").as_deref(),
        Ok("ultra")
    );
    assert_eq!(std::env::var("ANGEL_GLM_MODEL").as_deref(), Ok("glm-5.3"));
    assert_eq!(
        std::env::var("ANGEL_DEEPSEEK_MODEL").as_deref(),
        Ok("deepseek-v4-pro")
    );
    assert_eq!(
        std::env::var("ANGEL_DEEPSEEK_REASONING_EFFORT").as_deref(),
        Ok("high")
    );
    crate::agent::formations::formation(crate::agent::formations::FormationId::SoloStrike)
        .apply_sota_env();
    assert!(std::env::var("ANGEL_SOTA_MOA_PROPOSE_CLUB").is_err());
    assert!(std::env::var("ANGEL_SOTA_MOA_VERIFY_CLUB").is_err());
    assert!(std::env::var("ANGEL_SOTA_MOA_AGG_CLUB").is_err());
    assert!(
        std::env::var("ANGEL_OPENAI_REASONING_EFFORT").is_err(),
        "Math God Sol@ultra must not leak into the next formation"
    );
    assert!(
        std::env::var("ANGEL_GROK_REASONING_EFFORT").is_err(),
        "Math God Grok xhigh must not leak into the next formation"
    );
}

#[test]
fn math_god_pins_do_not_arm_when_another_formation_is_engaged_first() {
    let _guard = env_lock();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_OPENAI_REASONING_EFFORT") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_GROK_REASONING_EFFORT") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_SOTA_MOA_PROPOSE_CLUB") };
    crate::agent::formations::formation(crate::agent::formations::FormationId::GrokWar)
        .apply_sota_env();
    assert_eq!(
        std::env::var("ANGEL_SOTA_MOA_PROPOSE_CLUB").as_deref(),
        Ok("glm")
    );
    assert_ne!(
        std::env::var("ANGEL_OPENAI_REASONING_EFFORT").as_deref(),
        Ok("ultra")
    );
    assert_ne!(
        std::env::var("ANGEL_GROK_REASONING_EFFORT").as_deref(),
        Ok("xhigh")
    );
    crate::agent::formations::formation(crate::agent::formations::FormationId::SoloStrike)
        .apply_sota_env();
}

#[test]
fn agent_panel_moa_button_opens_formation_deck() {
    use ratatui::crossterm::event::{MouseButton, MouseEventKind};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    let text = render_app_text(&mut app, 144, 48);
    assert!(
        text.contains("FORMATION ▾") || text.contains("FORMATION:"),
        "agent panel should expose the formation deck button\n{text}"
    );
    let (rect, _) = app
        .agent_buttons
        .iter()
        .find(|(_, btn)| *btn == AgentButton::MoaDeck)
        .copied()
        .expect("moa button hitbox");
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        rect.x,
        rect.y,
    ));
    assert!(app.moa_deck.is_some(), "click should open the MoA deck");
}

#[test]
fn formation_escape_restores_its_recorded_source_route_in_one_step() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = seed_preview_app();
    app.scryglass.navigate(scryglass::StageRoute::Observatory);
    app.open_moa_deck(None);
    assert_eq!(
        app.scryglass.controller.route(),
        scryglass::StageRoute::Formation
    );

    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.moa_deck.is_none());
    assert_eq!(
        app.scryglass.controller.route(),
        scryglass::StageRoute::Observatory,
        "Formation Esc must restore its recorded owner without an empty intermediate Stage"
    );
}

#[test]
fn render_does_not_replace_an_operator_route_from_compatibility_state() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.open_moa_deck(None);
    assert!(app.moa_deck.is_some());
    app.scryglass.navigate(scryglass::StageRoute::Quest);

    let _ = render_app_text(&mut app, 120, 40);
    assert_eq!(
        app.scryglass.controller.route(),
        scryglass::StageRoute::Quest,
        "draw must resolve the controller, never re-route from a stale compatibility field"
    );
}
