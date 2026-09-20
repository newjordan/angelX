use super::*;

#[test]
fn draw_scryglass_off_reclaims_full_width_at_all_geometries() {
    let _guard = env_lock();
    let _env = TestEnvGuard::set("ANGEL_SCRYGLASS", "0");
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "in_process");
    let mut app = seed_preview_app();
    assert!(!app.scryglass_enabled);
    for (width, height) in [(80, 24), (120, 40), (220, 60)] {
        app.focus_module("artifacts");
        render_app_text(&mut app, width, height);
        assert!(app.panes.rect_of(mouse::PaneId::Artifacts).is_none());
        let transcript = app.panel_frames.get(panels::PanelKind::Transcript).unwrap();
        let composer = app.panel_frames.get(panels::PanelKind::Input).unwrap();
        assert_eq!(transcript.width, width);
        assert_eq!(composer.width, width);
        assert!(!app.scryglass.visible);
    }
    app.input = "/world on".into();
    app.submit();
    render_app_text(&mut app, 120, 40);
    assert!(app.panes.rect_of(mouse::PaneId::Artifacts).is_some());
    app.input = "/world off".into();
    app.submit();
    app.input = "/world weather".into();
    app.submit();
    render_app_text(&mut app, 120, 40);
    assert!(!app.scryglass_enabled);
    assert!(app.panes.rect_of(mouse::PaneId::Artifacts).is_none());
}

#[test]
fn draw_scryglass_show_queues_at_narrow_width_and_reveals_after_resize() {
    let _guard = env_lock();
    let _env = TestEnvGuard::set("ANGEL_SCRYGLASS", "1");
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "in_process");
    let mut app = seed_preview_app();
    render_app_text(&mut app, 80, 24);
    app.input = format!(
        "/show {}/assets/realm/scriptorium-baseline.png",
        env!("CARGO_MANIFEST_DIR")
    );
    app.submit();
    let notice = &app.messages.last().unwrap().text;
    assert!(notice.contains("minimum width 100 columns"), "{notice}");
    assert!(notice.contains("queued for the next layout"), "{notice}");
    assert!(notice.contains(&format!(
        "video_decode={}",
        cfg!(feature = "scryglass-video")
    )));
    let selected = app.scryglass.active_media().expect("queued media retained");
    render_app_text(&mut app, 80, 24);
    assert!(app.panes.rect_of(mouse::PaneId::Artifacts).is_none());
    render_app_text(&mut app, 120, 40);
    assert!(app.panes.rect_of(mouse::PaneId::Artifacts).is_some());
    assert_eq!(app.scryglass.active_media(), Some(selected));
    assert!(app.scryglass.visible);
    assert_eq!(
        app.scryglass.surface,
        scryglass::StageSurface::Still(selected)
    );
}

#[test]
fn reasoning_checkpoint_compact_and_large_bays_keep_full_width_text_above_portrait() {
    let _guard = env_lock();
    let _comp = TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = TestEnvGuard::unset("ANGEL_TURBO");
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "in_process");
    crate::drive::comp_mode::invalidate_cache();
    crate::ui::surfaces::invalidate_backdrop_cache();
    for (width, height) in [(24, 10), (42, 16), (90, 36)] {
        for header_card in [false, true] {
            for live in [false, true] {
                let mut previous_geometry = None;
                for focused in [false, true] {
                    let mut app = seed_thinking_app("checkpoint reasoning remains readable");
                    app.reasoning_shown = app.reasoning.len();
                    app.viewer = Viewer::portrait_preview();
                    if !live {
                        app.thinking = None;
                    }
                    if header_card {
                        app.header_card_area = Some(Rect::new(0, 0, width, 1));
                        app.agent_control_area = app.header_card_area;
                    }
                    app.focus_module(if focused { "agent" } else { "core" });
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    terminal
                        .draw(|frame| draw::render_agent_bay(frame, &mut app, frame.area()))
                        .unwrap();
                    let flow = app.panes.rect_of(mouse::PaneId::AgentBay).unwrap();
                    let portrait = app.bay_portrait_area.unwrap();
                    assert_eq!(flow.x, 1);
                    assert_eq!(flow.width, width - 3, "only scrollbar reserves width");
                    assert_eq!(flow.bottom(), portrait.y);
                    assert_eq!(portrait.right(), width - 1);
                    assert_eq!(portrait.bottom(), height - 1);
                    assert!(portrait.width <= 40 && portrait.height <= 14);
                    assert!(flow.height >= 2, "portrait must preserve the trace strip");
                    let buffer = terminal.backend().buffer();
                    assert!(
                        (portrait.y..portrait.bottom()).any(|y| {
                            (portrait.x..portrait.right())
                                .any(|x| matches!(buffer[(x, y)].symbol(), "▀" | "▄" | "█"))
                        }),
                        "portrait pixels must occupy the anchored rectangle"
                    );
                    assert_eq!(app.reasoning_wrap.viewport_height, usize::from(flow.height));
                    assert!(test_backend_text(terminal.backend()).contains("checkpoint"));
                    assert_eq!(app.panes.pane_at(portrait.x, portrait.y), None);
                    assert_eq!(app.panes.pane_at(flow.right(), flow.y), None);
                    assert_eq!(app.panes.pane_at(flow.x, flow.y - 1), None);
                    if let Some(geometry) = previous_geometry {
                        assert_eq!(
                            (flow, portrait),
                            geometry,
                            "focus must not reflow reasoning"
                        );
                    }
                    previous_geometry = Some((flow, portrait));
                }
            }
        }
    }
}

#[test]
fn reasoning_checkpoint_scrolled_hit_geometry_matches_paint_across_resize() {
    let _guard = env_lock();
    let _comp = TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = TestEnvGuard::unset("ANGEL_TURBO");
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "in_process");
    let _scryglass = TestEnvGuard::set("ANGEL_SCRYGLASS", "1");
    crate::drive::comp_mode::invalidate_cache();
    crate::ui::surfaces::invalidate_backdrop_cache();
    let reasoning = (0..100)
        .map(|i| format!("ROW_{i:03}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut app = seed_thinking_app(&reasoning);
    app.thinking = None;
    app.reasoning_shown = app.reasoning.len();
    app.focus_module("agent");
    for (width, height) in [(70, 28), (144, 48), (220, 60), (96, 32)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| ui(frame, &mut app)).unwrap();
        app.scroll_focused_view_top();
        terminal.draw(|frame| ui(frame, &mut app)).unwrap();
        app.scroll_pane_down(mouse::PaneId::AgentBay, 20);
        terminal.draw(|frame| ui(frame, &mut app)).unwrap();
        let flow = app.panes.rect_of(mouse::PaneId::AgentBay).unwrap();
        let bay = app.panel_frames.get(panels::PanelKind::AgentBay).unwrap();
        assert_eq!(flow.x, bay.x + 1);
        assert_eq!(flow.width, bay.width - 3);
        assert_eq!(flow.bottom(), app.bay_portrait_area.unwrap().y);
        let buffer = terminal.backend().buffer();
        let top = (flow.x..flow.right())
            .map(|x| buffer[(x, flow.y)].symbol())
            .collect::<String>();
        assert!(
            top.starts_with("ROW_020"),
            "scrolled first row at {flow:?}: {top}"
        );
        assert_eq!(
            app.panes.pane_at(flow.x, flow.y),
            Some((mouse::PaneId::AgentBay, flow))
        );
        // The authorized reasoning copy viewport describes exactly the text
        // painted here, not the portrait or surrounding AgentBay chrome.
        let mut selection = mouse::Selection::new(mouse::PaneId::AgentBay, flow, flow.x, flow.y);
        selection.cursor = (flow.right() - 1, flow.y);
        assert_eq!(mouse::extract_text(buffer, &selection), "ROW_020");
        assert!(crate::ui::surfaces::pane_accepts_clipboard(
            mouse::PaneId::AgentBay
        ));
        app.reasoning.push_str("\nAPPENDED");
        app.reasoning_shown = app.reasoning.len();
        terminal.draw(|frame| ui(frame, &mut app)).unwrap();
        let next = app.panes.rect_of(mouse::PaneId::AgentBay).unwrap();
        assert_eq!(next, flow);
        let buffer = terminal.backend().buffer();
        assert_eq!(mouse::extract_text(buffer, &selection), "ROW_020");
    }
}

#[test]
fn reasoning_checkpoint_tiny_bays_prioritize_text_and_empty_routes_stay_honest() {
    let _guard = env_lock();
    for (width, height) in [(4, 3), (12, 5), (19, 8)] {
        let mut app = seed_thinking_app("readable");
        app.thinking = None;
        app.reasoning_shown = app.reasoning.len();
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| draw::render_agent_bay(frame, &mut app, frame.area()))
            .unwrap();
        let flow = app.panes.rect_of(mouse::PaneId::AgentBay).unwrap();
        assert_eq!(flow.width, width - 3);
        assert!(flow.height > 0);
        assert!(app.bay_portrait_area.is_none());
    }
    for live in [false, true] {
        let mut app = seed_thinking_app("");
        if !live {
            app.thinking = None;
        }
        app.header_card_area = Some(Rect::new(0, 0, 70, 1));
        let mut terminal = Terminal::new(TestBackend::new(70, 16)).unwrap();
        terminal
            .draw(|frame| draw::render_agent_bay(frame, &mut app, frame.area()))
            .unwrap();
        let text = test_backend_text(terminal.backend());
        assert!(
            !text.contains("CLI-backed routes") && !text.contains("provider to expose reasoning")
        );
        assert!(
            app.reasoning.is_empty(),
            "empty routes must not invent a trace"
        );
    }
}
