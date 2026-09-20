//! Agent-identity/route-chip suites (module-breakup: extracted from the
//! `tests.rs` monolith). Responsive-breakpoint model identity, collapsed
//! side-panel dropdowns, the compact trusted-route capabilities bay, the
//! roomy header's identity/telemetry, and route locking under a live job.

use super::{render_app_text, seed_preview_app};
use crate::app::AgentButton;
use crate::club::Bag;
use crate::draw::ui;
use crate::tests::env_lock;
use crate::{Viewer, app_control, mouse, panels};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

#[test]
fn agent_controls_preserve_exact_model_identity_at_responsive_breakpoints() {
    let _guard = env_lock();
    for (width, height) in [(160, 48), (90, 30), (72, 24)] {
        let mut app = seed_preview_app();
        app.bag = Bag::for_reasoning_render_test();
        let text = render_app_text(&mut app, width, height);
        assert!(
            text.contains("gpt-5.6-sol"),
            "model slug lost at {width}x{height}\n{text}"
        );
        assert!(
            text.contains("[THINK:medium ▾]"),
            "thinking control lost at {width}x{height}\n{text}"
        );
        let controls = app.agent_control_area.expect("agent control area");
        let inside_agent_controls = |rect: &Rect| {
            rect.x >= controls.x
                && rect.x < controls.x.saturating_add(controls.width)
                && rect.y >= controls.y
                && rect.y < controls.y.saturating_add(controls.height)
        };
        let model = app
            .agent_buttons
            .iter()
            .find(|(rect, button)| *button == AgentButton::Model && inside_agent_controls(rect))
            .map(|(rect, _)| *rect)
            .expect("model hitbox");
        let thinking = app
            .agent_buttons
            .iter()
            .find(|(rect, button)| {
                *button == AgentButton::ReasoningEffort && inside_agent_controls(rect)
            })
            .map(|(rect, _)| *rect)
            .expect("thinking hitbox");
        assert_eq!(
            thinking.y, model.y,
            "header dropdowns stay on one clean row at {width}x{height}"
        );
    }
}

#[test]
fn collapsed_side_panel_keeps_route_dropdowns_in_the_header() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_reasoning_render_test();
    let text = render_app_text(&mut app, 60, 20);
    assert!(
        text.contains("[MODEL:gpt-5.6-sol ▾]") && text.contains("[THINK:medium ▾]"),
        "collapsed side panel lost the header route controls\n{text}"
    );
    assert!(
        app.agent_control_area.is_some_and(|area| area.y <= 3),
        "route deck should anchor to the header controls"
    );
}

#[test]
fn idle_agent_bay_keeps_compact_trusted_route_capabilities_visible() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_reasoning_render_test();
    let text = render_app_text(&mut app, 144, 48);
    assert!(text.contains("M>00%"), "{text}");
    assert!(text.contains("brain · 372k ctx"), "{text}");
    assert!(text.contains("speed fast available"), "{text}");
    assert!(
        !text.contains("Latest frontier agentic coding model."),
        "long descriptions belong in the Brain Route popup, not the persistent bay\n{text}"
    );
    let narrow = render_app_text(&mut app, 90, 30);
    assert!(narrow.contains("[MODEL:gpt-5.6-sol ▾]"), "{narrow}");
    assert!(narrow.contains("[THINK:medium ▾]"), "{narrow}");
    assert!(
        !narrow.contains("Latest frontier agentic coding model."),
        "compact Core must remain a readable transcript, not an inspector\n{narrow}"
    );

    let mut unknown = seed_preview_app();
    let unknown_text = render_app_text(&mut unknown, 144, 48);
    assert!(
        !unknown_text.contains("brain ·"),
        "backends without trusted metadata must not get invented facts\n{unknown_text}"
    );

    let mut occupied = seed_preview_app();
    occupied.bag = Bag::for_reasoning_render_test();
    occupied
        .tools
        .gauge
        .used_tokens
        .store(93_000, std::sync::atomic::Ordering::Relaxed);
    let occupied_text = render_app_text(&mut occupied, 144, 48);
    assert!(
        occupied_text.contains("brain · ctx ~93k/372k"),
        "{occupied_text}"
    );
    assert!(
        occupied_text.contains("speed fast available"),
        "{occupied_text}"
    );

    occupied
        .tools
        .gauge
        .used_tokens
        .store(305_000, std::sync::atomic::Ordering::Relaxed);
    let warning_text = render_app_text(&mut occupied, 144, 48);
    assert!(
        warning_text.contains("~82% · compact soon"),
        "{warning_text}"
    );

    occupied
        .tools
        .gauge
        .used_tokens
        .store(360_000, std::sync::atomic::Ordering::Relaxed);
    let critical_text = render_app_text(&mut occupied, 144, 48);
    assert!(
        critical_text.contains("~97% · /compact now"),
        "{critical_text}"
    );
}

#[test]
fn roomy_header_owns_agent_identity_telemetry_and_capabilities() {
    // The info header is opt-in since 2026-07-22 (ANGEL_AGENT_INFO_HEADER);
    // this test exercises the restored layout, so it opts in scoped.
    let _guard = env_lock();
    struct HeaderEnv;
    impl Drop for HeaderEnv {
        fn drop(&mut self) {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::remove_var("ANGEL_AGENT_INFO_HEADER") };
        }
    }
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_AGENT_INFO_HEADER", "1") };
    let _header = HeaderEnv;
    let mut app = seed_preview_app();
    app.bag = Bag::for_reasoning_render_test();
    app.viewer = Viewer::portrait_preview();
    let backend = TestBackend::new(144, 48);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| ui(frame, &mut app)).unwrap();

    let info = app.agent_info_area.expect("roomy agent info header bay");
    let agent = app
        .panel_frames
        .get(panels::PanelKind::AgentBay)
        .expect("agent bay frame");
    assert_eq!(info.x, agent.x + 1, "header info aligns with agent content");
    assert_eq!(
        info.width,
        agent.width.saturating_sub(2),
        "header info and agent content share a column"
    );
    assert_eq!(
        info.y + info.height,
        agent.y,
        "agent info sits directly above the agent panel"
    );

    let buffer = terminal.backend().buffer();
    let info_text = (info.y..info.y + info.height)
        .map(|y| {
            (info.x..info.x + info.width)
                .filter_map(|x| buffer.cell((x, y)).map(|cell| cell.symbol()))
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    for needle in [
        "SOTA escalation",
        "ChatGPT-OAuth reasoning agent",
        "idle",
        "C>00%",
        "tok in",
        "brain · 372k ctx",
        "speed fast available",
    ] {
        assert!(
            info_text.contains(needle),
            "agent header info missing {needle:?}\n{info_text}"
        );
    }
    for y in info.y..info.y + info.height {
        assert_eq!(
            buffer.cell((info.x - 1, y)).map(|cell| cell.symbol()),
            Some("│"),
            "agent info bay needs a continuous dividing bar at row {y}"
        );
    }

    let agent_inner = mouse::inner_border(agent);
    let agent_text = (agent_inner.y..agent_inner.y + agent_inner.height)
        .map(|y| {
            (agent_inner.x..agent_inner.x + agent_inner.width)
                .filter_map(|x| buffer.cell((x, y)).map(|cell| cell.symbol()))
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    for moved in ["SOTA escalation", "ChatGPT-OAuth reasoning agent", "C>00%"] {
        assert!(
            !agent_text.contains(moved),
            "{moved:?} must move out of the agent body and into the header\n{agent_text}"
        );
    }
    assert!(
        agent_text.contains('▀') || agent_text.contains('▄') || agent_text.contains('█'),
        "the freed agent body should be portrait space\n{agent_text}"
    );
}

#[test]
fn route_controls_are_locked_while_a_job_owns_the_turn() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[
        ("alpha", &[("model-a", true), ("model-b", true)]),
        ("beta", &[("model-c", true)]),
    ]);
    app.open_agent_menu(crate::agent::controls::AgentMenuKind::Model);
    assert!(app.agent_menu.is_some());
    let (_tx, job) = app_control::BackgroundJob::channel("test background job", "Retry the test");
    app.bg_job = Some(job);
    let text = render_app_text(&mut app, 144, 48);
    assert!(
        app.agent_menu.is_none(),
        "a job taking the flight slot must close a stale route deck"
    );
    assert!(text.contains("[RUN:model-a]"), "{text}");
    assert!(
        app.agent_buttons
            .iter()
            .all(|(_, button)| *button != AgentButton::Model),
        "busy model label must not register a mutating hitbox"
    );
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    app.do_action(crate::app_control::Action::NextBox);
    app.do_action(crate::app_control::Action::NextMode);
    assert_eq!(app.bag.in_hand_label(), "alpha");
    assert_eq!(app.bag.in_hand_mode().as_deref(), Some("model-a"));
}
