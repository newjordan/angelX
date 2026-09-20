//! Brain-route deck behavior suites (module-breakup: extracted from the
//! `tests.rs` monolith). Deck capabilities/evidence, transactional model and
//! effort application, the two-step critical commit, slash filtering,
//! star/diamond picks, and the effort recommendation ladder.

use super::{mouse_ev, render_app_text, seed_preview_app};
use crate::agent::club::Bag;
use crate::agent::turn::Thinking;
use crate::tests::env_lock;
use crate::ui::draw::ui;
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn thinking_brackets_preview_the_highlighted_model_without_changing_the_active_route() {
    use crate::ui::agent_panel::controls::AgentMenuKind;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_dual_reasoning_render_test();
    app.open_agent_menu(AgentMenuKind::Model);
    app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    assert_eq!(app.agent_menu.unwrap().route_target, Some((1, 0)));
    assert_eq!(app.agent_menu_effort_preview().as_deref(), Some("deep"));
    assert_eq!(app.bag.in_hand_label(), "openai");
    assert_eq!(app.bag.reasoning_effort().as_deref(), Some("medium"));
    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.bag.reasoning_effort().as_deref(), Some("medium"));

    app.open_agent_menu(AgentMenuKind::Model);
    app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.bag.in_hand_label(), "beta");
    assert_eq!(app.bag.reasoning_effort().as_deref(), Some("deep"));
}

#[test]
fn model_details_toggle_is_clickable_and_does_not_select_a_route() {
    use crate::ui::agent_panel::controls::{AgentMenuAction, AgentMenuKind};
    use ratatui::crossterm::event::{MouseButton, MouseEventKind};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_reasoning_render_test();
    app.open_agent_menu(AgentMenuKind::Model);
    let compact = render_app_text(&mut app, 100, 30);
    assert!(!compact.contains("ops: cold start"), "{compact}");
    let before = app.bag.selected_route_indices();
    let (rect, _) = app
        .agent_menu_hits
        .iter()
        .find(|(_, action)| *action == AgentMenuAction::ToggleDetails)
        .cloned()
        .expect("details control");
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        rect.x,
        rect.y,
    ));
    let details = render_app_text(&mut app, 100, 30);
    assert!(details.contains("ops: cold start"), "{details}");
    assert!(details.contains("[d Less]"), "{details}");
    assert_eq!(app.bag.selected_route_indices(), before);
    assert!(app.agent_menu.is_some());
}

#[test]
fn brain_route_deck_explains_model_capabilities_and_effort() {
    use crate::ui::agent_panel::controls::AgentMenuKind;

    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_reasoning_render_test();
    app.open_agent_menu(AgentMenuKind::Model);
    let model_deck = render_app_text(&mut app, 180, 50);
    assert!(model_deck.contains("372k ctx"), "{model_deck}");
    assert!(model_deck.contains("text+image"), "{model_deck}");
    assert!(model_deck.contains("speed fast available"), "{model_deck}");
    assert!(model_deck.contains("[d Details]"), "{model_deck}");
    assert!(!model_deck.contains("ops: cold start"), "{model_deck}");
    assert!(
        model_deck.contains("Latest frontier agentic coding model."),
        "{model_deck}"
    );

    app.tools
        .gauge
        .used_tokens
        .store(305_000, std::sync::atomic::Ordering::Relaxed);
    let fit_deck = render_app_text(&mut app, 180, 50);
    assert!(fit_deck.contains("ctx ~82% tight"), "{fit_deck}");

    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    let thinking_deck = render_app_text(&mut app, 180, 50);
    assert!(
        thinking_deck.contains("Brain Route · THINK"),
        "{thinking_deck}"
    );
    assert!(
        thinking_deck.contains("Balances speed and reasoning depth."),
        "{thinking_deck}"
    );
}

#[test]
fn brain_route_transactionally_applies_highlighted_model_and_effort() {
    use crate::ui::agent_panel::controls::AgentMenuKind;

    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_dual_reasoning_render_test();
    app.open_agent_menu(AgentMenuKind::Model);
    app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.agent_menu.map(|menu| menu.selected), Some(1));
    assert_eq!(app.bag.in_hand_label(), "openai");
    assert_eq!(app.bag.reasoning_effort().as_deref(), Some("medium"));

    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(
        app.agent_menu.map(|menu| (menu.kind, menu.route_target)),
        Some((AgentMenuKind::Thinking, Some((1, 0))))
    );
    let thinking_deck = render_app_text(&mut app, 144, 48);
    assert!(thinking_deck.contains("shallow"), "{thinking_deck}");
    assert!(thinking_deck.contains("deep"), "{thinking_deck}");
    assert!(
        thinking_deck.contains("route: beta ▸ model-b"),
        "{thinking_deck}"
    );
    assert!(
        thinking_deck.contains("Short deliberation for routine work."),
        "{thinking_deck}"
    );
    assert_eq!(
        app.bag.in_hand_label(),
        "openai",
        "inspecting a route must not activate it before Enter"
    );

    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(
        app.agent_menu.map(|menu| (menu.kind, menu.selected)),
        Some((AgentMenuKind::Model, 1)),
        "Tab back must preserve the highlighted model"
    );
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.bag.in_hand_label(), "beta");
    assert_eq!(app.bag.in_hand_mode().as_deref(), Some("model-b"));
    assert_eq!(app.bag.reasoning_effort().as_deref(), Some("deep"));
    assert!(app.agent_menu.is_none());
    assert_eq!(
        app.brain_route_receipt
            .as_ref()
            .map(|receipt| receipt.label.as_str()),
        Some("beta ▸ model-b@deep")
    );
    let receipt = render_app_text(&mut app, 144, 48);
    assert!(
        receipt.contains("route set · beta ▸ model-b@deep"),
        "{receipt}"
    );

    app.reasoning = "Prior reasoning remains inspectable.".to_string();
    app.reasoning_shown = app.reasoning.len();
    let reasoning_visible = render_app_text(&mut app, 144, 48);
    assert!(
        reasoning_visible.contains("[SET:model-b ▾]"),
        "{reasoning_visible}"
    );
    assert!(
        reasoning_visible.contains("[THINK:deep ▾]"),
        "{reasoning_visible}"
    );
    app.reasoning.clear();
    app.reasoning_shown = 0;

    app.brain_route_receipt
        .as_mut()
        .expect("fresh route receipt")
        .applied_at = std::time::Instant::now() - std::time::Duration::from_secs(4);
    let expired = render_app_text(&mut app, 144, 48);
    assert!(!expired.contains("route set ·"), "{expired}");
    assert!(app.brain_route_receipt.is_none());
}

#[test]
fn critically_full_route_requires_second_keyboard_or_mouse_commit() {
    use crate::App;
    use crate::ui::agent_panel::controls::{AgentMenuAction, AgentMenuKind};
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
    use ratatui::layout::Rect;

    let _guard = env_lock();
    fn beta_route_hit(app: &App) -> Rect {
        app.agent_menu_hits
            .iter()
            .find(|(_, action)| {
                matches!(
                    action,
                    AgentMenuAction::SelectRoute {
                        agent_index: 1,
                        slot_index: 0
                    }
                )
            })
            .map(|(area, _)| *area)
            .expect("beta route hitbox")
    }

    let mut keyboard = seed_preview_app();
    keyboard.bag = Bag::for_dual_reasoning_render_test();
    keyboard
        .tools
        .gauge
        .used_tokens
        .store(190_000, std::sync::atomic::Ordering::Relaxed);
    keyboard.open_agent_menu(AgentMenuKind::Model);
    keyboard.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    keyboard.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    keyboard.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    keyboard.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(keyboard.bag.in_hand_label(), "openai");
    assert_eq!(
        keyboard.agent_menu.and_then(|menu| menu.confirm_target),
        Some((1, 0))
    );
    let confirmation = render_app_text(&mut keyboard, 144, 48);
    assert!(
        confirmation.contains("Brain Route · THINK:deep · CONFIRM"),
        "{confirmation}"
    );
    assert!(confirmation.contains("ctx ~97% full"), "{confirmation}");
    assert!(
        confirmation.contains("/compact recommended"),
        "{confirmation}"
    );
    keyboard.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(keyboard.bag.in_hand_label(), "beta");
    assert_eq!(keyboard.bag.reasoning_effort().as_deref(), Some("deep"));
    assert!(keyboard.agent_menu.is_none());

    let mut mouse = seed_preview_app();
    mouse.bag = Bag::for_dual_reasoning_render_test();
    mouse
        .tools
        .gauge
        .used_tokens
        .store(190_000, std::sync::atomic::Ordering::Relaxed);
    assert!(mouse.open_agent_menu_filtered(AgentMenuKind::Model, "model-b"));
    let _ = render_app_text(&mut mouse, 144, 48);
    let beta_route = beta_route_hit(&mouse);
    mouse.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        beta_route.x,
        beta_route.y,
    ));
    assert_eq!(mouse.bag.in_hand_label(), "openai");
    assert_eq!(
        mouse.agent_menu.and_then(|menu| menu.confirm_target),
        Some((1, 0))
    );
    let armed = render_app_text(&mut mouse, 144, 48);
    assert!(
        armed.contains("Brain Route · MODEL:shallow · CONFIRM"),
        "{armed}"
    );
    mouse.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(
        mouse.agent_menu.is_none(),
        "Esc must leave armed filter mode"
    );
    assert_eq!(mouse.bag.in_hand_label(), "openai");

    mouse.open_agent_menu(AgentMenuKind::Model);
    let _ = render_app_text(&mut mouse, 144, 48);
    let beta_route = beta_route_hit(&mouse);
    mouse.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        beta_route.x,
        beta_route.y,
    ));
    let _ = render_app_text(&mut mouse, 144, 48);
    let beta_route = beta_route_hit(&mouse);
    mouse.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        beta_route.x,
        beta_route.y,
    ));
    assert_eq!(mouse.bag.in_hand_label(), "beta");
    assert!(mouse.agent_menu.is_none());
}

#[test]
fn brain_route_effort_mouse_action_carries_its_highlighted_route() {
    use crate::ui::agent_panel::controls::{AgentMenuAction, AgentMenuKind};

    use ratatui::crossterm::event::{MouseButton, MouseEventKind};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_dual_reasoning_render_test();
    app.open_agent_menu(AgentMenuKind::Model);
    let model_deck = render_app_text(&mut app, 144, 48);
    assert!(model_deck.contains("[think: shallow ▾]"), "{model_deck}");
    let inspect = app
        .agent_menu_hits
        .iter()
        .find(|(_, action)| {
            matches!(
                action,
                AgentMenuAction::InspectEffort {
                    agent_index: 1,
                    slot_index: 0
                }
            )
        })
        .map(|(rect, _)| *rect)
        .expect("model-b effort affordance");
    let select = app
        .agent_menu_hits
        .iter()
        .find(|(_, action)| {
            matches!(
                action,
                AgentMenuAction::SelectRoute {
                    agent_index: 1,
                    slot_index: 0
                }
            )
        })
        .map(|(rect, _)| *rect)
        .expect("model-b route hitbox");
    assert!(select.x + select.width <= inspect.x);
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        inspect.x,
        inspect.y,
    ));
    assert_eq!(
        app.agent_menu.map(|menu| (menu.kind, menu.route_target)),
        Some((AgentMenuKind::Thinking, Some((1, 0))))
    );
    assert_eq!(app.bag.in_hand_label(), "openai");

    let _ = render_app_text(&mut app, 144, 48);
    let deep = app
        .agent_menu_hits
        .iter()
        .find(|(_, action)| {
            matches!(
                action,
                AgentMenuAction::SelectEffort {
                    agent_index: 1,
                    slot_index: 0,
                    effort
                } if effort == "deep"
            )
        })
        .map(|(rect, _)| *rect)
        .expect("deep effort hitbox for highlighted beta route");
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        deep.x,
        deep.y,
    ));
    assert_eq!(app.bag.in_hand_label(), "beta");
    assert_eq!(app.bag.reasoning_effort().as_deref(), Some("deep"));
    assert_eq!(
        app.brain_route_receipt
            .as_ref()
            .map(|receipt| receipt.label.as_str()),
        Some("beta ▸ model-b@deep")
    );
}

#[test]
fn brain_route_effort_affordance_is_responsive_and_never_overlaps_route_hitbox() {
    use crate::ui::agent_panel::controls::{AgentMenuAction, AgentMenuKind};

    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let _guard = env_lock();
    for (width, height) in [(90, 30), (72, 24), (48, 20)] {
        let mut app = seed_preview_app();
        app.bag = Bag::for_dual_reasoning_render_test();
        app.open_agent_menu(AgentMenuKind::Model);
        let _ = render_app_text(&mut app, width, height);
        let inspect = app
            .agent_menu_hits
            .iter()
            .find(|(_, action)| {
                matches!(
                    action,
                    AgentMenuAction::InspectEffort {
                        agent_index: 1,
                        slot_index: 0
                    }
                )
            })
            .map(|(rect, _)| *rect)
            .unwrap_or_else(|| panic!("missing effort hitbox at {width}x{height}"));
        let select = app
            .agent_menu_hits
            .iter()
            .find(|(_, action)| {
                matches!(
                    action,
                    AgentMenuAction::SelectRoute {
                        agent_index: 1,
                        slot_index: 0
                    }
                )
            })
            .map(|(rect, _)| *rect)
            .expect("model-b route hitbox");
        assert_eq!(select.y, inspect.y);
        assert!(select.x + select.width <= inspect.x);
        assert!(inspect.x + inspect.width <= width);
    }

    let mut tiny = seed_preview_app();
    tiny.bag = Bag::for_dual_reasoning_render_test();
    tiny.open_agent_menu(AgentMenuKind::Model);
    let _ = render_app_text(&mut tiny, 30, 16);
    assert!(
        tiny.agent_menu_hits
            .iter()
            .all(|(_, action)| !matches!(action, AgentMenuAction::InspectEffort { .. }))
    );
    tiny.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    tiny.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(
        tiny.agent_menu.map(|menu| menu.route_target),
        Some(Some((1, 0))),
        "tiny decks retain the keyboard path when the split affordance cannot fit"
    );
}

#[test]
fn brain_route_slash_filter_selects_exact_model_without_mutating_early() {
    use crate::ui::agent_panel::controls::{AgentMenuAction, AgentMenuKind};

    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_dual_reasoning_render_test();
    app.open_agent_menu(AgentMenuKind::Model);
    app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    for character in "model-b".chars() {
        app.on_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
    }
    assert_eq!(app.agent_menu_search.as_deref(), Some("model-b"));
    assert_eq!(app.bag.in_hand_label(), "openai");

    let deck = render_app_text(&mut app, 144, 48);
    assert!(
        deck.contains("Brain Route · MODEL:shallow · /model-b"),
        "{deck}"
    );
    assert!(app.agent_menu_hits.iter().any(|(_, action)| matches!(
        action,
        AgentMenuAction::SelectRoute {
            agent_index: 1,
            slot_index: 0
        }
    )));
    assert!(app.agent_menu_hits.iter().all(|(_, action)| !matches!(
        action,
        AgentMenuAction::SelectRoute {
            agent_index: 0,
            slot_index: 0
        }
    )));

    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.bag.in_hand_label(), "beta");
    assert!(app.agent_menu.is_none());
    assert!(app.agent_menu_search.is_none());
}

#[test]
fn model_command_hands_its_argument_to_the_safe_deck_filter() {
    use crate::ui::agent_panel::controls::{AgentMenuAction, AgentMenuKind};

    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_dual_reasoning_render_test();
    app.input = "/model model-b".to_string();
    app.cursor = app.input.chars().count();
    app.submit();
    assert_eq!(
        app.agent_menu.map(|menu| menu.kind),
        Some(AgentMenuKind::Model)
    );
    assert_eq!(app.agent_menu_search.as_deref(), Some("model-b"));
    assert_eq!(app.bag.in_hand_label(), "openai");

    let _ = render_app_text(&mut app, 90, 30);
    assert!(app.agent_menu_hits.iter().any(|(_, action)| matches!(
        action,
        AgentMenuAction::SelectRoute {
            agent_index: 1,
            slot_index: 0
        }
    )));
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.bag.in_hand_label(), "beta");
}

#[test]
fn model_at_effort_command_prepares_atomic_think_confirmation() {
    use crate::ui::agent_panel::controls::AgentMenuKind;

    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_dual_reasoning_render_test();
    app.input = "/model model-b@deep".to_string();
    app.cursor = app.input.chars().count();
    app.submit();
    assert_eq!(
        app.agent_menu
            .map(|menu| (menu.kind, menu.route_target, menu.selected)),
        Some((AgentMenuKind::Thinking, Some((1, 0)), 1))
    );
    assert_eq!(app.agent_menu_search.as_deref(), Some("deep"));
    assert_eq!(app.bag.in_hand_label(), "openai");
    let deck = render_app_text(&mut app, 90, 30);
    assert!(deck.contains("Brain Route · THINK:deep · /deep"), "{deck}");

    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.bag.in_hand_label(), "beta");
    assert_eq!(app.bag.reasoning_effort().as_deref(), Some("deep"));
}

#[test]
fn model_at_effort_invalid_effort_is_recoverable_and_non_mutating() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_dual_reasoning_render_test();
    app.input = "/model model-b@invented".to_string();
    app.cursor = app.input.chars().count();
    app.submit();
    let deck = render_app_text(&mut app, 90, 30);
    assert!(deck.contains("no routes match /invented"), "{deck}");
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.bag.in_hand_label(), "openai");
    assert!(app.agent_menu.is_some());
}

#[test]
fn model_at_effort_ambiguous_model_stays_in_filtered_model_deck() {
    use crate::ui::agent_panel::controls::{AgentMenuAction, AgentMenuKind};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[("alpha", &[("model-a", true), ("model-b", true)])]);
    app.input = "/model alpha@high".to_string();
    app.cursor = app.input.chars().count();
    app.submit();
    assert_eq!(
        app.agent_menu.map(|menu| (menu.kind, menu.route_target)),
        Some((AgentMenuKind::Model, None))
    );
    assert_eq!(app.agent_menu_search.as_deref(), Some("alpha"));
    let _ = render_app_text(&mut app, 90, 30);
    let route_hits = app
        .agent_menu_hits
        .iter()
        .filter(|(_, action)| matches!(action, AgentMenuAction::SelectRoute { .. }))
        .count();
    assert_eq!(route_hits, 2);
    assert_eq!(app.bag.in_hand_mode().as_deref(), Some("model-a"));
}

#[test]
fn filtered_model_command_reports_when_route_controls_are_locked() {
    let mut app = seed_preview_app();
    app.bag = Bag::for_dual_reasoning_render_test();
    app.thinking = Some(Thinking::pending_for_test("openai"));
    app.input = "/model model-b".to_string();
    app.cursor = app.input.chars().count();
    app.submit();
    assert!(app.agent_menu.is_none());
    assert!(
        app.messages
            .last()
            .is_some_and(|message| message.text.contains("locked while a turn"))
    );
}

#[test]
fn brain_route_filter_keeps_no_match_open_and_escape_clears_before_close() {
    use crate::ui::agent_panel::controls::AgentMenuKind;

    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_dual_reasoning_render_test();
    app.open_agent_menu(AgentMenuKind::Model);
    app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    for character in "zzzq".chars() {
        app.on_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
    }
    assert_eq!(app.agent_menu_search.as_deref(), Some("zzzq"));
    let deck = render_app_text(&mut app, 72, 24);
    assert!(deck.contains("no routes match /zzzq"), "{deck}");
    assert!(app.agent_menu_hits.iter().all(|(_, action)| matches!(
        action,
        crate::ui::agent_panel::controls::AgentMenuAction::ToggleDetails
            | crate::ui::agent_panel::controls::AgentMenuAction::ToggleUnavailable
    )));

    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.agent_menu.is_some());
    assert_eq!(app.bag.in_hand_label(), "openai");
    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.agent_menu.is_some());
    assert!(app.agent_menu_search.is_none());
    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.agent_menu.is_none());
}

#[test]
fn brain_route_effort_filter_preserves_original_level_index() {
    use crate::ui::agent_panel::controls::AgentMenuKind;

    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_dual_reasoning_render_test();
    app.open_agent_menu(AgentMenuKind::Model);
    app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
    assert_eq!(app.agent_menu.map(|menu| menu.selected), Some(1));
    assert_eq!(app.bag.in_hand_label(), "openai");
    let deck = render_app_text(&mut app, 90, 30);
    assert!(deck.contains("Brain Route · THINK:deep · /d"), "{deck}");
    assert!(deck.contains("deep"), "{deck}");

    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.bag.in_hand_label(), "beta");
    assert_eq!(app.bag.reasoning_effort().as_deref(), Some("deep"));
}

#[test]
fn combined_route_effort_rejects_stale_level_without_partial_route_change() {
    let mut bag = Bag::for_dual_reasoning_render_test();
    assert!(!bag.select_route_with_effort(1, 0, "invented"));
    assert_eq!(bag.in_hand_label(), "openai");
    assert_eq!(bag.reasoning_effort().as_deref(), Some("medium"));
    let beta = bag
        .route_choices()
        .iter()
        .find(|choice| choice.agent == "beta")
        .cloned()
        .expect("beta route");
    assert_eq!(beta.reasoning_effort.as_deref(), Some("shallow"));
}

#[test]
fn brain_route_deck_is_opaque_over_terminal_imagery() {
    use crate::ui::agent_panel::controls::{AgentMenuAction, AgentMenuKind};
    use ratatui::style::Color;

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[
        ("alpha", &[("model-a", true), ("model-b", true)]),
        ("beta", &[("model-c", true)]),
    ]);
    app.open_agent_menu(AgentMenuKind::Model);
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| ui(frame, &mut app)).unwrap();
    let area = app.agent_menu_area.expect("rendered route deck area");
    let buffer = terminal.backend().buffer();
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            assert_ne!(
                buffer.cell((x, y)).expect("menu cell inside buffer").bg,
                Color::Reset,
                "transparent cell at ({x},{y}) would let underlying art impair the deck"
            );
        }
    }
    assert!(app.agent_menu_hits.iter().any(|(_, action)| matches!(
        action,
        AgentMenuAction::SelectRoute {
            agent_index: 0,
            slot_index: 1
        }
    )));
}

#[test]
fn brain_route_deck_surfaces_operational_evidence_without_quality_claims() {
    use crate::ui::agent_panel::controls::AgentMenuKind;

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_reasoning_render_test();
    app.open_agent_menu(AgentMenuKind::Model);
    let records = [
        serde_json::json!({
            "kind":"turn", "v":2, "ts":1,
            "route":{"driver":"gpt-5.6-sol","model":"gpt-5.6-sol","reasoning_effort":"medium"},
            "outcome":{"ok":true,"stop":"answer","latency_ms":1000,"tokens":{"openai":{"in":1000,"out":0}},"failovers":[]}
        }),
        serde_json::json!({
            "kind":"turn", "v":2, "ts":2,
            "route":{"driver":"gpt-5.6-sol","model":"gpt-5.6-sol","reasoning_effort":"medium"},
            "outcome":{"ok":true,"stop":"answer","latency_ms":1500,"tokens":{"openai":{"in":2000,"out":0}},"failovers":[]}
        }),
        serde_json::json!({
            "kind":"turn", "v":2, "ts":3,
            "route":{"driver":"gpt-5.6-sol","model":"gpt-5.6-sol","reasoning_effort":"medium"},
            "outcome":{"ok":true,"stop":"answer","latency_ms":2000,"tokens":{"openai":{"in":3000,"out":0}},"failovers":[{"to":"practice"}]}
        }),
    ]
    .into_iter()
    .map(|value| value.to_string())
    .collect::<Vec<_>>()
    .join("\n");
    app.route_evidence = crate::knowledge::route_intelligence::parse_recent_jsonl(&records, false);
    app.apply_agent_menu_action(crate::ui::agent_panel::controls::AgentMenuAction::ToggleDetails);
    let deck = render_app_text(&mut app, 144, 48);
    assert!(deck.contains("ops: n3"), "{deck}");
    assert!(deck.contains("clean 67%"), "{deck}");
    assert!(deck.contains("p50 1.5s"), "{deck}");
    assert!(deck.contains("rec 1"), "{deck}");
    assert!(deck.contains("avg 2.0k tok"), "{deck}");
    assert!(
        !deck.contains("quality 67%"),
        "operational success must never be mislabeled as answer quality"
    );
}

#[test]
fn brain_route_star_is_a_non_mutating_minimum_evidence_ops_pick() {
    use crate::ui::agent_panel::controls::AgentMenuKind;

    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_reasoning_render_test();
    app.open_agent_menu(AgentMenuKind::Model);
    let mut records = Vec::new();
    for seq in 0..5u64 {
        records.push(
            serde_json::json!({
                "kind":"turn", "v":2, "ts":seq,
                "route":{"driver":"gpt-5.6-sol","model":"gpt-5.6-sol","reasoning_effort":"medium"},
                "outcome":{"ok":seq < 4,"stop":if seq < 4 {"answer"} else {"error_stop"},"latency_ms":2000,"tokens":{},"failovers":[]}
            })
            .to_string(),
        );
        records.push(
            serde_json::json!({
                "kind":"turn", "v":2, "ts":seq,
                "route":{"driver":"practice","model":"model-b","reasoning_effort":null},
                "outcome":{"ok":true,"stop":"answer","latency_ms":1000,"tokens":{},"failovers":[]}
            })
            .to_string(),
        );
    }
    app.route_evidence =
        crate::knowledge::route_intelligence::parse_recent_jsonl(&records.join("\n"), false);
    let before = (app.bag.in_hand_label().to_string(), app.bag.in_hand_mode());
    app.apply_agent_menu_action(crate::ui::agent_panel::controls::AgentMenuAction::ToggleDetails);
    let deck = render_app_text(&mut app, 144, 48);
    let picked = deck
        .lines()
        .find(|line| line.contains("model-b"))
        .expect("model-b route row");
    assert!(
        picked.contains('★'),
        "ops pick row should be starred: {picked}"
    );
    assert_eq!(
        before,
        (app.bag.in_hand_label().to_string(), app.bag.in_hand_mode()),
        "an evidence hint must never auto-route"
    );
    app.on_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE));
    assert_eq!(app.agent_menu.map(|menu| menu.selected), Some(1));
    assert_eq!(app.bag.in_hand_label(), "openai");
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(
        app.bag.in_hand_label(),
        "beta",
        "the ops pick routes only after explicit Enter confirmation"
    );
}

#[test]
fn brain_route_diamond_is_a_gated_explicit_user_pick() {
    use crate::ui::agent_panel::controls::AgentMenuKind;

    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_reasoning_render_test();
    app.open_agent_menu(AgentMenuKind::Model);
    let mut records = Vec::new();
    for seq in 0..5u64 {
        records.push(
            serde_json::json!({
                "kind":"route_verdict", "v":2, "ts":seq, "source":"user",
                "route":{"driver":"openai","model":"gpt-5.6-sol","reasoning_effort":"medium"},
                "verdict":if seq < 3 {"useful"} else {"miss"}
            })
            .to_string(),
        );
        records.push(
            serde_json::json!({
                "kind":"route_verdict", "v":2, "ts":seq, "source":"user",
                "route":{"driver":"practice","model":"model-b","reasoning_effort":null},
                "verdict":"useful"
            })
            .to_string(),
        );
    }
    app.route_evidence =
        crate::knowledge::route_intelligence::parse_recent_jsonl(&records.join("\n"), false);
    let before = (app.bag.in_hand_label().to_string(), app.bag.in_hand_mode());
    app.apply_agent_menu_action(crate::ui::agent_panel::controls::AgentMenuAction::ToggleDetails);
    let deck = render_app_text(&mut app, 144, 48);
    let picked = deck
        .lines()
        .find(|line| line.contains("model-b"))
        .expect("model-b route row");
    assert!(
        picked.contains('◆'),
        "explicit user pick row should carry a diamond: {picked}"
    );
    assert!(deck.contains("explicit user labels"), "{deck}");
    assert_eq!(
        before,
        (app.bag.in_hand_label().to_string(), app.bag.in_hand_mode()),
        "a quality hint must never auto-route"
    );
    app.on_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
    assert_eq!(app.agent_menu.map(|menu| menu.selected), Some(1));
    assert_eq!(app.bag.in_hand_label(), "openai");
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(
        app.bag.in_hand_label(),
        "beta",
        "the user-quality pick routes only after explicit Enter confirmation"
    );
}

#[test]
fn thinking_deck_recommends_effort_without_applying_until_enter() {
    use crate::ui::agent_panel::controls::AgentMenuKind;

    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_reasoning_render_test();
    let mut records = Vec::new();
    for seq in 0..5u64 {
        records.push(
            serde_json::json!({
                "kind":"turn", "v":2, "ts":seq,
                "route":{"driver":"gpt-5.6-sol","model":"gpt-5.6-sol","reasoning_effort":"low"},
                "outcome":{"ok":true,"stop":"answer","latency_ms":500,"tokens":{},"failovers":[]}
            })
            .to_string(),
        );
        records.push(
            serde_json::json!({
                "kind":"turn", "v":2, "ts":seq,
                "route":{"driver":"gpt-5.6-sol","model":"gpt-5.6-sol","reasoning_effort":"high"},
                "outcome":{"ok":seq < 3,"stop":if seq < 3 {"answer"} else {"error_stop"},"latency_ms":2000,"tokens":{},"failovers":[]}
            })
            .to_string(),
        );
        records.push(
            serde_json::json!({
                "kind":"route_verdict", "v":2, "ts":seq, "source":"user",
                "route":{"driver":"gpt-5.6-sol","model":"gpt-5.6-sol","reasoning_effort":"low"},
                "verdict":if seq < 2 {"useful"} else {"miss"}
            })
            .to_string(),
        );
        records.push(
            serde_json::json!({
                "kind":"route_verdict", "v":2, "ts":seq, "source":"user",
                "route":{"driver":"gpt-5.6-sol","model":"gpt-5.6-sol","reasoning_effort":"high"},
                "verdict":"useful"
            })
            .to_string(),
        );
    }
    app.open_agent_menu(AgentMenuKind::Thinking);
    app.route_evidence =
        crate::knowledge::route_intelligence::parse_recent_jsonl(&records.join("\n"), false);
    app.apply_agent_menu_action(crate::ui::agent_panel::controls::AgentMenuAction::ToggleDetails);
    let deck = render_app_text(&mut app, 144, 48);
    let low = deck
        .lines()
        .find(|line| line.contains(" low"))
        .expect("low row");
    let high = deck
        .lines()
        .find(|line| line.contains(" high"))
        .expect("high row");
    assert!(low.contains('★'), "operational effort pick: {low}");
    assert!(high.contains('◆'), "explicit-quality effort pick: {high}");
    assert!(deck.contains("o ops · q quality"), "{deck}");

    app.on_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE));
    assert_eq!(app.agent_menu.map(|menu| menu.selected), Some(0));
    assert_eq!(app.bag.reasoning_effort().as_deref(), Some("medium"));
    let ops = render_app_text(&mut app, 144, 48);
    assert!(ops.contains("★ ops pick"), "{ops}");

    app.on_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
    assert_eq!(app.agent_menu.map(|menu| menu.selected), Some(2));
    assert_eq!(app.bag.reasoning_effort().as_deref(), Some("medium"));
    let quality = render_app_text(&mut app, 144, 48);
    assert!(quality.contains("◆ user pick"), "{quality}");
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.bag.reasoning_effort().as_deref(), Some("high"));
    assert!(app.agent_menu.is_none());
}
