//! Brain-route deck suites (module-breakup: extracted from the `tests.rs`
//! monolith). Model/think deck opening and selection, portrait previews,
//! keyboard access, prefiltering, offline-row disabling, and keymap
//! collisions.

use super::{mouse_ev, render_app_text, seed_preview_app};
use crate::agent_profile::AgentKey;
use crate::app::AgentButton;
use crate::club::Bag;
use crate::draw;
use crate::tests::env_lock;
use crate::turn::Thinking;
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn agent_panel_model_button_opens_exact_route_deck_and_selects() {
    use crate::agent_controls::AgentMenuAction;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[
        (
            "alpha",
            &[
                ("model-a", true),
                ("model-b", true),
                ("model-offline", false),
            ],
        ),
        ("beta", &[("model-c", true)]),
    ]);
    let text = render_app_text(&mut app, 144, 48);
    assert!(
        text.contains("[MODEL:model-a ▾]") || text.contains("[PREF:model-a ▾]"),
        "{text}"
    );
    assert!(text.contains("[THINK:native]"), "{text}");
    let (rect, _) = app
        .agent_buttons
        .iter()
        .find(|(_, button)| *button == AgentButton::Model)
        .copied()
        .expect("model button hitbox");
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        rect.x,
        rect.y,
    ));
    assert!(app.agent_menu.is_some(), "model click should open deck");
    assert_eq!(app.bag.in_hand_label(), "alpha");
    assert_eq!(app.bag.in_hand_mode().as_deref(), Some("model-a"));

    let deck = render_app_text(&mut app, 144, 48);
    assert!(deck.contains("Brain Route · MODEL"), "{deck}");
    assert!(deck.contains("alpha"), "{deck}");
    assert!(deck.contains("model-b"), "{deck}");
    assert!(!deck.contains("model-offline"), "{deck}");
    app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    let all_routes = render_app_text(&mut app, 144, 48);
    assert!(all_routes.contains("model-offline"), "{all_routes}");
    assert!(
        app.agent_menu_hits.iter().all(|(_, action)| !matches!(
            action,
            AgentMenuAction::SelectRoute {
                agent_index: 0,
                slot_index: 2
            }
        )),
        "offline rows remain inspectable without becoming actionable"
    );
    let (choice, _) = app
        .agent_menu_hits
        .iter()
        .find(|(_, action)| {
            matches!(
                action,
                AgentMenuAction::SelectRoute {
                    agent_index: 0,
                    slot_index: 1
                }
            )
        })
        .cloned()
        .expect("model-b menu row");
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        choice.x,
        choice.y,
    ));
    assert_eq!(app.bag.in_hand_mode().as_deref(), Some("model-b"));
    assert!(app.agent_menu.is_none());

    let _ = render_app_text(&mut app, 144, 48);
    let (rect, _) = app
        .agent_buttons
        .iter()
        .find(|(_, button)| *button == AgentButton::Model)
        .copied()
        .expect("model button hitbox after route change");
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        rect.x,
        rect.y,
    ));
    app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.bag.in_hand_label(), "beta");
    assert_eq!(app.bag.in_hand_mode().as_deref(), Some("model-c"));
}

#[test]
fn wide_unicode_model_control_hitbox_reaches_its_visible_closing_cell() {
    use ratatui::crossterm::event::{MouseButton, MouseEventKind};

    let mut app = seed_preview_app();
    app.bag =
        Bag::for_render_test(&[("atlas", &[("模型🧪-gpt-5.6-sol", true), ("fallback", true)])]);
    let backend = TestBackend::new(64, 1);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw::render_agent_controls(frame, &mut app, frame.area()))
        .unwrap();

    let close_x = (0..64)
        .find(|x| {
            terminal
                .backend()
                .buffer()
                .cell((*x, 0))
                .is_some_and(|cell| cell.symbol() == "]")
        })
        .expect("visible model closing bracket");
    let model = app
        .agent_buttons
        .iter()
        .find(|(_, button)| *button == AgentButton::Model)
        .map(|(rect, _)| *rect)
        .expect("model hitbox");
    assert_eq!(
        model.x + model.width - 1,
        close_x,
        "model hitbox must cover the exact painted terminal-cell span"
    );

    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        close_x,
        0,
    ));
    assert!(
        app.agent_menu.is_some(),
        "clicking the visible closing bracket must open the route deck"
    );
}

#[test]
fn model_route_deck_leads_with_model_and_keeps_connections_explicit() {
    use crate::agent_controls::AgentMenuKind;

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[
        ("spark", &[("checkpoint-a", true), ("checkpoint-b", true)]),
        ("turbo", &[("model-c", true)]),
    ]);
    app.open_agent_menu(AgentMenuKind::Model);
    let deck = render_app_text(&mut app, 144, 48);

    assert!(deck.contains("Brain Route · MODEL:native"), "{deck}");
    assert!(!deck.contains("┌ spark"), "{deck}");
    let mut terminal = Terminal::new(TestBackend::new(144, 48)).unwrap();
    terminal.draw(|frame| draw::ui(frame, &mut app)).unwrap();
    for (agent, slot, model, connection) in [
        (0, 0, "checkpoint-a", "spark"),
        (0, 1, "checkpoint-b", "spark"),
        (1, 0, "model-c", "turbo"),
    ] {
        let rect = app.agent_menu_hits.iter().find_map(|(rect, action)| {
            matches!(action, crate::agent_controls::AgentMenuAction::SelectRoute { agent_index, slot_index }
                if *agent_index == agent && *slot_index == slot).then_some(*rect)
        }).expect("model selection row");
        let buffer = terminal.backend().buffer();
        let row = (rect.x..rect.right())
            .map(|x| buffer[(x, rect.y)].symbol())
            .collect::<String>();
        let model_offset = row.find(model).expect("model identity in its row");
        let connection_offset = row.find(connection).expect("connection in its row");
        assert!(
            model_offset < connection_offset,
            "model precedes its connection: {row}"
        );
    }
}

#[test]
fn model_button_remains_an_inspector_with_only_one_concrete_route() {
    use ratatui::crossterm::event::{MouseButton, MouseEventKind};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[("alpha", &[("model-a", true)])]);
    let text = render_app_text(&mut app, 144, 48);
    assert!(
        text.contains("[MODEL:model-a ▾]") || text.contains("[PREF:model-a ▾]"),
        "{text}"
    );
    let model = app
        .agent_buttons
        .iter()
        .find(|(_, button)| *button == AgentButton::Model)
        .map(|(area, _)| *area)
        .expect("single route should still expose the MODEL inspector");
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        model.x,
        model.y,
    ));
    let deck = render_app_text(&mut app, 144, 48);
    assert!(deck.contains("Brain Route · MODEL"), "{deck}");
    assert!(deck.contains("model-a"), "{deck}");

    let mut practice = seed_preview_app();
    let _ = render_app_text(&mut practice, 144, 48);
    assert!(
        practice
            .agent_buttons
            .iter()
            .all(|(_, button)| *button != AgentButton::Model),
        "practice-only mode has no concrete Brain Route to inspect"
    );
}

#[test]
fn brain_route_cursor_previews_agent_portrait_without_committing() {
    use crate::agent_controls::AgentMenuKind;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[("turbo", &[("turbo", true)]), ("atlas", &[("atlas", true)])]);
    app.open_agent_menu(AgentMenuKind::Model);
    assert_eq!(app.active_profile().key, AgentKey::Turbo);

    app.move_agent_menu(1);
    assert_eq!(app.bag.in_hand_label(), "turbo");
    assert_eq!(app.agent_menu_profile_preview().as_deref(), Some("atlas"));
    assert_eq!(app.active_profile().key, AgentKey::Atlas);

    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.bag.in_hand_label(), "turbo");
    assert_eq!(app.active_profile().key, AgentKey::Turbo);
}

#[test]
fn portrait_tracks_the_selected_provider_model_inside_one_sota_agent() {
    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[(
        "sota",
        &[
            ("gpt-5.6-sol", true),
            ("deepseek-v4-pro", true),
            ("grok-4.6", true),
        ],
    )]);

    let openai = app.active_profile();
    assert_eq!(openai.key, AgentKey::Codex);
    assert!(app.active_profile_label.contains("gpt-5.6-sol"));

    assert!(app.bag.select_route(0, 1));
    let deepseek = app.active_profile();
    assert_eq!(deepseek.key, AgentKey::Sparky);
    assert!(app.active_profile_label.contains("deepseek-v4-pro"));
    assert_ne!(openai.asset(false), deepseek.asset(false));

    assert!(app.bag.select_route(0, 2));
    let grok = app.active_profile();
    assert_eq!(grok.key, AgentKey::Turbo);
    assert!(app.active_profile_label.contains("grok-4.6"));
    assert_ne!(deepseek.asset(false), grok.asset(false));
}

#[test]
fn brain_route_cursor_previews_provider_portrait_without_committing() {
    use crate::agent_controls::AgentMenuKind;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = seed_preview_app();
    app.bag =
        Bag::for_render_test(&[("sota", &[("gpt-5.6-sol", true), ("deepseek-v4-pro", true)])]);
    app.open_agent_menu(AgentMenuKind::Model);
    assert_eq!(app.active_profile().key, AgentKey::Codex);

    app.move_agent_menu(1);
    assert_eq!(app.bag.in_hand_mode().as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(app.active_profile().key, AgentKey::Sparky);
    assert!(app.active_profile_label.contains("deepseek-v4-pro"));

    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.active_profile().key, AgentKey::Codex);
    assert!(app.active_profile_label.contains("gpt-5.6-sol"));
}

#[test]
fn live_portrait_uses_the_spawned_requested_route_not_a_shared_box_label() {
    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[("sota", &[("deepseek-v4-pro", true)])]);
    assert_eq!(app.active_profile().key, AgentKey::Sparky);

    let mut thinking = Thinking::pending_for_test("sota");
    thinking.requested_route = crate::club::RouteIdentity {
        driver: "xai".to_string(),
        model: Some("grok-4.6".to_string()),
        reasoning_effort: Some("high".to_string()),
    };
    app.thinking = Some(thinking);

    assert_eq!(app.active_profile().key, AgentKey::Turbo);
    assert!(app.active_profile_label.contains("xai"));
    assert!(app.active_profile_label.contains("grok-4.6"));
}

#[test]
fn active_profile_draw_path_does_not_allocate_a_fresh_route_identity() {
    let src = include_str!("../../../cockpit/src/app.rs");
    let start = src
        .find("pub(crate) fn active_profile(")
        .expect("active_profile present");
    let body = &src[start..];
    let end = body
        .find("\n    fn hit_active_profile_cache(")
        .expect("hit_active_profile_cache follows active_profile");
    let body = &body[..end];
    assert!(
        !body.contains("resolved_route_identity()"),
        "draw must not allocate route identity every frame:\n{body}"
    );
    assert!(
        body.contains("resolved_route_if_known"),
        "failover portrait still needs the published resolved route:\n{body}"
    );
}

#[test]
fn header_tracks_the_tool_workspace_without_caching_an_old_project() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    for name in ["orchard-project", "weather-project"] {
        app.tools = std::sync::Arc::new(crate::harness::ToolRegistry::with_team(
            std::path::PathBuf::from(format!("/tmp/{name}")),
            Vec::new(),
        ));
        let text = render_app_text(&mut app, 160, 48);
        assert!(text.contains(name));
        let other = if name == "orchard-project" {
            "weather-project"
        } else {
            "orchard-project"
        };
        assert!(!text.contains(other));
    }
}

#[test]
fn thinking_cursor_previews_effort_without_committing() {
    use crate::agent_controls::AgentMenuKind;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = seed_preview_app();
    app.bag = Bag::for_reasoning_render_test();
    app.open_agent_menu(AgentMenuKind::Thinking);
    assert_eq!(app.bag.reasoning_effort().as_deref(), Some("medium"));
    assert_eq!(app.agent_menu_effort_preview().as_deref(), Some("medium"));

    app.move_agent_menu(1);
    assert_eq!(app.agent_menu_effort_preview().as_deref(), Some("high"));
    assert_eq!(
        app.bag.reasoning_effort().as_deref(),
        Some("medium"),
        "visual preview must not mutate the backend"
    );

    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.agent_menu_effort_preview().is_none());
    assert_eq!(app.bag.reasoning_effort().as_deref(), Some("medium"));
}

#[test]
fn thinking_button_and_model_command_are_keyboard_accessible() {
    use crate::agent_controls::AgentMenuKind;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_reasoning_render_test();
    let text = render_app_text(&mut app, 144, 48);
    assert!(text.contains("[THINK:medium ▾]"), "{text}");
    let (rect, _) = app
        .agent_buttons
        .iter()
        .find(|(_, button)| *button == AgentButton::ReasoningEffort)
        .copied()
        .expect("thinking button hitbox");
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        rect.x,
        rect.y,
    ));
    assert_eq!(
        app.agent_menu.map(|menu| menu.kind),
        Some(AgentMenuKind::Thinking)
    );
    let deck = render_app_text(&mut app, 144, 48);
    assert!(deck.contains("Brain Route · THINK:medium"), "{deck}");
    assert!(deck.contains("low"), "{deck}");
    assert!(deck.contains("high"), "{deck}");
    app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.bag.reasoning_effort().as_deref(), Some("high"));

    app.input = "/model".to_string();
    app.cursor = app.input.chars().count();
    app.submit();
    assert_eq!(
        app.agent_menu.map(|menu| menu.kind),
        Some(AgentMenuKind::Model),
        "/model should open the keyboard-operable route deck"
    );
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(
        app.agent_menu.map(|menu| menu.kind),
        Some(AgentMenuKind::Thinking)
    );
    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.agent_menu.is_none());

    app.input = "/model auto".to_string();
    app.cursor = app.input.chars().count();
    app.submit();
    assert!(app.agent_menu.is_none());
    assert!(
        app.messages
            .last()
            .is_some_and(|message| message.text.contains("already automatic"))
    );
}

#[test]
fn thinking_command_prefilters_without_mutating_until_enter() {
    use crate::agent_controls::AgentMenuKind;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_reasoning_render_test();
    app.input = "/think high".to_string();
    app.cursor = app.input.chars().count();
    app.submit();
    assert_eq!(
        app.agent_menu.map(|menu| (menu.kind, menu.selected)),
        Some((AgentMenuKind::Thinking, 2))
    );
    assert_eq!(app.agent_menu_search.as_deref(), Some("high"));
    assert_eq!(app.bag.reasoning_effort().as_deref(), Some("medium"));
    let deck = render_app_text(&mut app, 144, 48);
    assert!(deck.contains("Brain Route · THINK:high · /high"), "{deck}");
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.bag.reasoning_effort().as_deref(), Some("high"));
    assert!(app.agent_menu.is_none());

    let mut unsupported = seed_preview_app();
    unsupported.input = "/think".to_string();
    unsupported.cursor = unsupported.input.chars().count();
    unsupported.submit();
    assert!(unsupported.agent_menu.is_none());
    assert!(
        unsupported
            .messages
            .last()
            .is_some_and(|message| message.text.contains("no configurable thinking levels"))
    );
}

#[test]
fn thinking_deck_disables_rows_if_its_route_goes_offline() {
    use crate::agent_controls::AgentMenuKind;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_reasoning_render_test();
    app.open_agent_menu(AgentMenuKind::Thinking);
    app.bag.set_route_available_for_test(0, 0, false);

    let deck = render_app_text(&mut app, 144, 48);
    assert!(
        deck.contains("openai ▸ gpt-5.6-sol · OFFLINE · selection locked"),
        "{deck}"
    );
    assert!(
        app.agent_menu_hits.iter().all(|(_, action)| matches!(
            action,
            crate::agent_controls::AgentMenuAction::ToggleDetails
                | crate::agent_controls::AgentMenuAction::ToggleUnavailable
        )),
        "offline effort rows must not retain mouse actions"
    );
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.bag.reasoning_effort().as_deref(), Some("medium"));
    assert!(app.agent_menu.is_some());
}

#[test]
fn undersized_terminal_closes_invisible_brain_route_modal() {
    use crate::agent_controls::AgentMenuKind;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_reasoning_render_test();
    app.open_agent_menu(AgentMenuKind::Model);
    let _ = render_app_text(&mut app, 17, 8);
    assert!(app.agent_menu.is_none());

    app.on_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE));
    assert_eq!(app.input, "z", "the hidden modal must not capture input");
}

#[test]
fn header_dropdowns_and_f9_f10_open_route_decks_without_keymap_collisions() {
    use crate::agent_controls::AgentMenuKind;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_reasoning_render_test();
    let wide = render_app_text(&mut app, 200, 48);
    assert!(wide.contains("[MODEL:gpt-5.6-sol ▾]"), "{wide}");
    assert!(wide.contains("[THINK:medium ▾]"), "{wide}");
    assert!(!wide.contains("ENTER >> SEND"), "{wide}");

    let collapsed = render_app_text(&mut app, 60, 20);
    assert!(collapsed.contains("[MODEL:gpt-5.6-sol ▾]"), "{collapsed}");
    assert!(collapsed.contains("[THINK:medium ▾]"), "{collapsed}");
    let collapsed_model = app
        .agent_buttons
        .iter()
        .find(|(_, button)| *button == AgentButton::Model)
        .map(|(area, _)| *area)
        .expect("collapsed header MODEL button");
    let collapsed_think = app
        .agent_buttons
        .iter()
        .find(|(_, button)| *button == AgentButton::ReasoningEffort)
        .map(|(area, _)| *area)
        .expect("collapsed header THINK button");
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        collapsed_model.x,
        collapsed_model.y,
    ));
    assert_eq!(
        app.agent_menu.map(|menu| menu.kind),
        Some(AgentMenuKind::Model)
    );
    let deck = render_app_text(&mut app, 60, 20);
    let toolbar = app.agent_control_area.expect("header toolbar anchor");
    let menu = app.agent_menu_area.expect("opened route deck");
    assert_eq!(
        menu.y,
        toolbar.y + toolbar.height,
        "the route deck should drop directly beneath the header button\n{deck}"
    );
    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        collapsed_think.x,
        collapsed_think.y,
    ));
    assert_eq!(
        app.agent_menu.map(|menu| menu.kind),
        Some(AgentMenuKind::Thinking)
    );
    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    app.on_key(KeyEvent::new(KeyCode::F(9), KeyModifiers::NONE));
    assert_eq!(
        app.agent_menu.map(|menu| menu.kind),
        Some(AgentMenuKind::Model)
    );
    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::F(10), KeyModifiers::NONE));
    assert_eq!(
        app.agent_menu.map(|menu| menu.kind),
        Some(AgentMenuKind::Thinking)
    );

    let mut busy = seed_preview_app();
    busy.bag = Bag::for_reasoning_render_test();
    busy.thinking = Some(Thinking::pending_for_test("openai"));
    let busy_rail = render_app_text(&mut busy, 60, 20);
    assert!(busy_rail.contains("[RUN:gpt-5.6-sol]"), "{busy_rail}");
    assert!(busy_rail.contains("[THINK:medium]"), "{busy_rail}");
    assert!(
        busy.agent_buttons.iter().all(|(_, button)| !matches!(
            button,
            AgentButton::Model | AgentButton::ReasoningEffort
        )),
        "busy route shortcuts must be visible documentation without live mouse targets"
    );
    busy.on_key(KeyEvent::new(KeyCode::F(9), KeyModifiers::NONE));
    assert!(busy.agent_menu.is_none());
}
