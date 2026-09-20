//! Last-answer rating suites (module-breakup: extracted from the `tests.rs`
//! monolith). Single-shot text-free ratings, the +/− non-mutating picker,
//! and the responsive/reclaimed feedback buttons.

use super::{mouse_ev, render_app_text, seed_preview_app};
use crate::app::{AgentButton, LastCompletedRoute};
use crate::club::Bag;
use crate::tests::env_lock;
use crate::turn::Thinking;

#[test]
fn explicit_last_answer_rating_is_single_shot_and_text_free() {
    let mut app = seed_preview_app();
    app.last_completed_route = Some(LastCompletedRoute {
        route: crate::club::RouteIdentity {
            driver: "openai".to_string(),
            model: Some("gpt-5.6-sol".to_string()),
            reasoning_effort: Some("ultra".to_string()),
        },
        completed_ms: 123,
        verdict: None,
    });
    app.input = "/rate useful".to_string();
    app.cursor = app.input.chars().count();
    app.submit();
    assert_eq!(
        app.last_completed_route
            .as_ref()
            .and_then(|last| last.verdict),
        Some(crate::experience::RouteVerdict::Useful)
    );
    let evidence = app
        .route_evidence
        .find("openai", "gpt-5.6-sol", Some("ultra"))
        .expect("rating should update the live evidence snapshot");
    assert_eq!(evidence.user_useful, 1);
    assert!(
        app.messages
            .last()
            .is_some_and(|message| message.text.contains("route metadata only"))
    );

    app.input = "/rate miss".to_string();
    app.cursor = app.input.chars().count();
    app.submit();
    assert!(
        app.messages
            .last()
            .is_some_and(|message| message.text.contains("already rated useful"))
    );
    assert_eq!(
        app.last_completed_route
            .as_ref()
            .and_then(|last| last.verdict),
        Some(crate::experience::RouteVerdict::Useful)
    );
}

#[test]
fn brain_route_plus_minus_rates_last_answer_without_selecting_a_model() {
    use crate::agent_controls::AgentMenuKind;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = seed_preview_app();
    app.bag = Bag::for_reasoning_render_test();
    app.last_completed_route = Some(LastCompletedRoute {
        route: crate::club::RouteIdentity {
            driver: "openai".to_string(),
            model: Some("gpt-5.6-sol".to_string()),
            reasoning_effort: Some("medium".to_string()),
        },
        completed_ms: 123,
        verdict: None,
    });
    app.open_agent_menu(AgentMenuKind::Model);
    let before = app.bag.in_hand_mode();
    app.on_key(KeyEvent::new(KeyCode::Char('-'), KeyModifiers::NONE));
    assert_eq!(app.bag.in_hand_mode(), before);
    assert_eq!(
        app.last_completed_route
            .as_ref()
            .and_then(|last| last.verdict),
        Some(crate::experience::RouteVerdict::Miss)
    );
    assert!(app.agent_menu.is_some());
}

#[test]
fn pending_feedback_buttons_are_responsive_mouseable_and_reclaimed() {
    use ratatui::crossterm::event::{MouseButton, MouseEventKind};

    let _guard = env_lock();
    for (width, height) in [(144, 48), (90, 30), (72, 24)] {
        let mut app = seed_preview_app();
        app.bag = Bag::for_reasoning_render_test();
        app.last_completed_route = Some(LastCompletedRoute {
            route: crate::club::RouteIdentity {
                driver: "openai".to_string(),
                model: Some("gpt-5.6-sol".to_string()),
                reasoning_effort: Some("medium".to_string()),
            },
            completed_ms: 123,
            verdict: None,
        });
        if width == 144 {
            app.thinking = Some(Thinking::pending_for_test("openai"));
        }
        if width < 100 {
            app.focus_module("agent");
        }

        let text = render_app_text(&mut app, width, height);
        assert!(text.contains("[+ useful]"), "{width}x{height}\n{text}");
        assert!(text.contains("[- miss]"), "{width}x{height}\n{text}");
        let useful = app
            .agent_buttons
            .iter()
            .find(|(_, button)| *button == AgentButton::RateUseful)
            .map(|(rect, _)| *rect)
            .expect("useful hitbox");
        let miss = app
            .agent_buttons
            .iter()
            .find(|(_, button)| *button == AgentButton::RateMiss)
            .map(|(rect, _)| *rect)
            .expect("miss hitbox");
        assert_eq!(useful.y, miss.y);
        assert!(useful.x + useful.width <= width);
        assert!(miss.x + miss.width <= width);

        if width == 144 {
            assert!(
                app.thinking.is_some(),
                "rating the prior answer is safe while the next turn owns the slot"
            );
            app.on_mouse(mouse_ev(
                MouseEventKind::Down(MouseButton::Left),
                useful.x,
                useful.y,
            ));
            assert_eq!(
                app.last_completed_route
                    .as_ref()
                    .and_then(|last| last.verdict),
                Some(crate::experience::RouteVerdict::Useful)
            );
            assert!(app.thinking.is_some());
            let rerendered = render_app_text(&mut app, width, height);
            assert!(!rerendered.contains("[+ useful]"), "{rerendered}");
            assert!(app.agent_buttons.iter().all(|(_, button)| !matches!(
                button,
                AgentButton::RateUseful | AgentButton::RateMiss
            )));
        }
    }
}

#[test]
fn practice_only_model_label_is_truthfully_disabled() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[("practice", &[("practice", true)])]);
    let text = render_app_text(&mut app, 144, 48);
    assert!(text.contains("practice"), "{text}");
    assert!(
        app.agent_buttons
            .iter()
            .all(|(_, button)| *button != AgentButton::Model),
        "a no-op model label must not advertise a hitbox"
    );
}

#[test]
fn process_fixed_reasoning_hint_is_visible_but_not_clickable() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_fixed_reasoning_render_test();
    let text = render_app_text(&mut app, 144, 48);
    assert!(text.contains("[THINK:high·fixed]"), "{text}");
    assert!(
        app.agent_buttons
            .iter()
            .all(|(_, button)| *button != AgentButton::ReasoningEffort),
        "process-global reasoning hints must remain read-only"
    );
}
