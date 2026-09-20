//! Pane ownership proofs against the production compositor, not invented rects.
use super::*;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{MouseButton, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::Modifier;

fn fixture(pane: mouse::PaneId) -> App {
    let mut app = seed_thinking_app(
        &(0..100)
            .map(|i| format!("REASON_ONLY {i:03} {}", "reasoning text ".repeat(8)))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    app.reasoning_shown = app.reasoning.len();
    app.visual_motion = crate::viz::lifecycle_viz::MotionMode::Off;
    app.messages = (0..80)
        .map(|i| Message {
            role: Role::Angel,
            text: format!("TRANSCRIPT_ONLY {i:03} {}", "transcript text ".repeat(20)).into(),
        })
        .collect();
    app.invalidate_transcript_layout();
    app.focus_module(if pane == mouse::PaneId::Transcript {
        "core"
    } else {
        "agent"
    });
    app
}

fn paint(app: &mut App, width: u16, height: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| draw::ui(frame, app)).unwrap();
    terminal.backend().buffer().clone()
}

fn drag(app: &mut App, start: (u16, u16), end: (u16, u16)) {
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        start.0,
        start.1,
    ));
    // Multiple drag events must not accidentally start a Stage camera gesture.
    for point in [start, end] {
        app.on_mouse(mouse_ev(
            MouseEventKind::Drag(MouseButton::Left),
            point.0,
            point.1,
        ));
    }
    assert!(app.scryglass_drag.is_none());
    app.on_mouse(mouse_ev(
        MouseEventKind::Up(MouseButton::Left),
        end.0,
        end.1,
    ));
    assert!(app.copy_requested);
}

fn assert_highlight(before: &Buffer, after: &Buffer, selection: &mouse::Selection) {
    let spans = mouse::selection_spans(selection);
    let mut highlighted = 0;
    for y in after.area.y..after.area.bottom() {
        for x in after.area.x..after.area.right() {
            let selected = spans
                .iter()
                .any(|&(row, left, right)| row == y && x >= left && x <= right);
            let cell = after.cell((x, y)).unwrap();
            if selected {
                assert!(mouse::point_in(selection.rect, x, y));
                assert!(cell.modifier.contains(Modifier::REVERSED));
                highlighted += 1;
            } else {
                assert_eq!(
                    cell.modifier.contains(Modifier::REVERSED),
                    before
                        .cell((x, y))
                        .unwrap()
                        .modifier
                        .contains(Modifier::REVERSED),
                    "highlight escaped {:?} at ({x},{y})",
                    selection.pane,
                );
            }
        }
    }
    assert!(highlighted > 1);
}

#[test]
fn cross_pane_drag_copies_and_highlights_only_origin_in_both_directions() {
    let _guard = env_lock();
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "in_process");
    let _world = TestEnvGuard::set("ANGEL_SCRYGLASS", "1");
    for (origin, other, own_text, foreign_text) in [
        (
            mouse::PaneId::Transcript,
            mouse::PaneId::AgentBay,
            "TRANSCRIPT_ONLY",
            "REASON_ONLY",
        ),
        (
            mouse::PaneId::AgentBay,
            mouse::PaneId::Transcript,
            "REASON_ONLY",
            "TRANSCRIPT_ONLY",
        ),
    ] {
        let mut app = fixture(origin);
        paint(&mut app, 240, 60);
        let source = app.panes.rect_of(origin).unwrap();
        let target = app.panes.rect_of(other).unwrap();
        let start = if origin == mouse::PaneId::Transcript {
            (source.x, source.y)
        } else {
            (source.right() - 1, source.bottom() - 1)
        };
        let end = if origin == mouse::PaneId::Transcript {
            (target.right() - 1, target.bottom() - 1)
        } else {
            (target.x, source.y)
        };
        assert!(
            mouse::point_in(target, end.0, end.1),
            "drag must cross into {other:?}"
        );
        drag(&mut app, start, end);
        let selected = app.selection.take().unwrap();
        assert_eq!(selected.pane, origin);
        app.copy_requested = false;
        let before = paint(&mut app, 240, 60);
        app.selection = Some(selected);
        app.copy_requested = true;
        let after = paint(&mut app, 240, 60);
        let selected = app.selection.unwrap();
        assert_eq!(selected.rect, app.panes.rect_of(origin).unwrap());
        let copied = app.pending_clipboard.as_deref().unwrap();
        assert!(copied.contains(own_text), "{origin:?}: {copied:?}");
        assert!(!copied.contains(foreign_text), "{origin:?}: {copied:?}");
        assert_eq!(
            copied,
            mouse::extract_text_in(&before, &selected, selected.rect)
        );
        assert_highlight(&before, &after, &selected);
        assert!(!app.copy_requested);
    }
}

#[test]
fn resize_rebinds_copy_and_highlight_to_current_origin_in_both_panes() {
    let _guard = env_lock();
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "in_process");
    let _world = TestEnvGuard::set("ANGEL_SCRYGLASS", "1");
    for origin in [mouse::PaneId::Transcript, mouse::PaneId::AgentBay] {
        let mut app = fixture(origin);
        paint(&mut app, 240, 60);
        let old = app.panes.rect_of(origin).unwrap();
        drag(
            &mut app,
            (old.x, old.y),
            (old.right() - 1, old.bottom() - 1),
        );
        let after = paint(&mut app, 220, 56);
        let selected = app.selection.unwrap();
        let current = app.panes.rect_of(origin).unwrap();
        assert_ne!(old, current);
        assert_eq!(selected.rect, current);
        let copied = app.pending_clipboard.take().unwrap();
        app.selection = None;
        let before = paint(&mut app, 220, 56);
        // Selection affects transcript reader anchoring; compare the payload
        // with the actual copy frame, not a later unselected redraw. Highlight
        // changes modifiers only, so buffer symbols remain the exact oracle.
        assert_eq!(copied, mouse::extract_text_in(&after, &selected, current));
        assert_highlight(&before, &after, &selected);

        // Also exercise mouse-side rebinding against a newly rendered registry.
        let mut stale = mouse::Selection::new(origin, old, old.x, old.y);
        stale.extend(old.right() - 1, old.bottom() - 1);
        app.selection = Some(stale);
        app.on_mouse(mouse_ev(
            MouseEventKind::Drag(MouseButton::Left),
            current.x + 8,
            current.y + 1,
        ));
        assert_eq!(app.selection.unwrap().rect, current);
        assert_eq!(app.selection.unwrap().pane, origin);
    }
}

#[test]
fn disappeared_or_disjoint_origin_cancels_pending_copy_and_drag() {
    let _guard = env_lock();
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "in_process");
    let _world = TestEnvGuard::set("ANGEL_SCRYGLASS", "1");
    for origin in [mouse::PaneId::Transcript, mouse::PaneId::AgentBay] {
        for disappeared in [false, true] {
            let mut app = fixture(origin);
            paint(&mut app, 240, 60);
            let old = app.panes.rect_of(origin).unwrap();
            drag(
                &mut app,
                (old.right() - 12, old.y),
                (old.right() - 1, old.y),
            );
            let stale = app.selection.unwrap();
            let size = if disappeared {
                app.focus_module("artifacts");
                (60, 24)
            } else {
                (144, 48)
            };
            let cancelled = paint(&mut app, size.0, size.1);
            assert_eq!(app.panes.rect_of(origin).is_none(), disappeared);
            assert!(
                app.selection.is_none(),
                "{origin:?}, disappeared={disappeared}"
            );
            assert!(!app.copy_requested);
            assert!(app.pending_clipboard.is_none());
            assert!(app.pending_quick_lookup.is_none());
            let clean = paint(&mut app, size.0, size.1);
            for (cancelled, clean) in cancelled.content.iter().zip(&clean.content) {
                assert_eq!(
                    cancelled.modifier.contains(Modifier::REVERSED),
                    clean.modifier.contains(Modifier::REVERSED),
                    "cancelled selection left a highlight"
                );
            }

            app.selection = Some(stale);
            app.copy_requested = true;
            app.on_mouse(mouse_ev(MouseEventKind::Drag(MouseButton::Left), 10, 10));
            assert!(app.selection.is_none());
            assert!(!app.copy_requested);
            app.on_mouse(mouse_ev(MouseEventKind::Up(MouseButton::Left), 10, 10));
            assert!(!app.copy_requested);
        }
    }
}

#[test]
fn portrait_controls_stage_and_composer_never_anchor_text_selection() {
    let _guard = env_lock();
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "in_process");
    let _world = TestEnvGuard::set("ANGEL_SCRYGLASS", "1");
    let mut app = fixture(mouse::PaneId::AgentBay);
    paint(&mut app, 144, 48);
    let agent = app.panes.rect_of(mouse::PaneId::AgentBay).unwrap();
    let portrait = app.bay_portrait_area.expect("bay portrait rendered");
    assert!(!agent.intersects(portrait));
    let mut forbidden = vec![
        portrait,
        app.panes.rect_of(mouse::PaneId::Artifacts).unwrap(),
        app.panel_frames.get(panels::PanelKind::Input).unwrap(),
        Rect::new(agent.x, agent.y - 1, agent.width, 1),
        Rect::new(agent.right(), agent.y, 1, agent.height),
    ];
    forbidden.extend(app.agent_buttons.iter().map(|(rect, _)| *rect));
    for rect in forbidden {
        app.agent_menu = None;
        app.selection = None;
        app.on_mouse(mouse_ev(
            MouseEventKind::Down(MouseButton::Left),
            rect.x,
            rect.y,
        ));
        assert!(app.selection.is_none(), "non-text hit at {rect:?}");
        assert!(!app.copy_requested);
        app.on_mouse(mouse_ev(
            MouseEventKind::Up(MouseButton::Left),
            rect.x,
            rect.y,
        ));
        assert!(!app.copy_requested);
        assert!(app.pending_clipboard.is_none());
    }
}

#[test]
fn fullscreen_transcript_fills_body_to_sidebar_with_toolstrip_excluded() {
    let _guard = env_lock();
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "in_process");
    let _world = TestEnvGuard::set("ANGEL_SCRYGLASS", "1");
    for width in [180, 240, 320] {
        let mut app = fixture(mouse::PaneId::Transcript);
        app.tool_strip.call_event(
            harness::ToolEventId("pane-selection-tool".into()),
            "shell",
            "PANE_TOOL_ONLY",
        );
        // Layout work yields after a 2 ms frame budget. This is a settled
        // geometry proof, so do not assume all 80 messages fit in one tick.
        let rendered = (0..=app.messages.len())
            .find_map(|_| {
                let frame = paint(&mut app, width, 48);
                (app.transcript_heights.len() == app.messages.len()).then_some(frame)
            })
            .expect("fixture transcript layout must settle within bounded frames");
        let frame = app.panel_frames.get(panels::PanelKind::Transcript).unwrap();
        let inner = mouse::inner_border(frame);
        let prose = app.panes.rect_of(mouse::PaneId::Transcript).unwrap();
        let strip_h = 2; // running tool: status row + loading bar, no note
        let strip_text = (inner.x..inner.right())
            .map(|x| {
                rendered
                    .cell((x, inner.bottom() - strip_h))
                    .unwrap()
                    .symbol()
            })
            .collect::<String>();
        assert!(strip_text.contains("PANE_TOOL_ONLY"), "{strip_text:?}");
        assert!(strip_h > 0);
        assert_eq!(
            Some(prose),
            surfaces::transcript_live_copy_rect(frame, strip_h)
        );
        assert_eq!(prose.right() + 1, inner.right());
        assert_eq!(prose.bottom() + strip_h, inner.bottom());
        assert!(
            (prose.y..prose.bottom()).any(|y| {
                rendered
                    .cell((prose.right(), y))
                    .is_some_and(|cell| cell.symbol() == "●")
            }),
            "scroll thumb must sit at the sidebar edge, width={width}"
        );
        if prose.width > 103 {
            assert!(
                (prose.y..prose.bottom()).any(|y| {
                    (prose.x + 103..prose.right()).any(|x| {
                        rendered
                            .cell((x, y))
                            .is_some_and(|cell| !cell.symbol().trim().is_empty())
                    })
                }),
                "wide prose must actually paint beyond the former cap"
            );
        }
        drag(
            &mut app,
            (prose.x, prose.y),
            (inner.right() - 1, inner.bottom() - 1),
        );
        let selected_frame = paint(&mut app, width, 48);
        let selected = app.selection.unwrap();
        assert_eq!(selected.rect, prose);
        let copied = app.pending_clipboard.as_deref().unwrap();
        assert_eq!(
            copied,
            mouse::extract_text_in(&selected_frame, &selected, prose)
        );
        assert!(!copied.contains("PANE_TOOL_ONLY"));
        assert_highlight(&rendered, &selected_frame, &selected);
    }
}

#[test]
fn agent_chrome_scrolls_and_focuses_its_pane_without_becoming_copyable() {
    let _guard = env_lock();
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "in_process");
    let _world = TestEnvGuard::set("ANGEL_SCRYGLASS", "1");
    let mut app = fixture(mouse::PaneId::Transcript);
    app.reasoning = "Long scroll fixture line\n".repeat(160);
    app.reasoning_shown = app.reasoning.len();
    let _ = paint(&mut app, 160, 52);
    let text = app.panes.rect_of(mouse::PaneId::AgentBay).unwrap();
    let frame = app.panel_frames.get(panels::PanelKind::AgentBay).unwrap();
    // The rail, reasoning label and bottom-right portrait are interaction
    // targets, but never clipboard sources.
    for point in [
        (text.right(), text.y),
        (text.x, text.y - 1),
        (frame.right() - 2, frame.bottom() - 2),
    ] {
        assert!(app.panes.pane_at(point.0, point.1).is_none());
        app.focus_module("core");
        app.scroll = 7;
        app.reasoning_scroll = 0;
        app.on_mouse(mouse_ev(MouseEventKind::ScrollUp, point.0, point.1));
        assert!(
            app.reasoning_scroll > 0,
            "agent chrome at {point:?} must scroll reasoning"
        );
        assert_eq!(app.scroll, 7, "agent chrome must not scroll its neighbor");
        assert_eq!(
            app.module_host.focused().map(|id| id.as_str()),
            Some("agent")
        );
        app.focus_module("core");
        app.on_mouse(mouse_ev(
            MouseEventKind::Down(MouseButton::Left),
            point.0,
            point.1,
        ));
        assert_eq!(
            app.module_host.focused().map(|id| id.as_str()),
            Some("agent")
        );
        assert!(
            app.selection.is_none(),
            "chrome cannot start a text selection"
        );
    }
}

#[test]
fn miniviz_world_owns_chrome_zoom_drag_and_never_scrolls_transcript() {
    let _guard = env_lock();
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "in_process");
    let _world = TestEnvGuard::set("ANGEL_SCRYGLASS", "1");
    let _protocol = TestEnvGuard::set("ANGEL_IMAGE_PROTOCOL", "halfblocks");
    let _comp = TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::comp_mode::invalidate_cache();
    let _view = crate::world_viz::world3d::pin(crate::world_viz::world3d::WorldView::Mesh3d);
    let mut app = fixture(mouse::PaneId::Transcript);
    app.scryglass
        .navigate(crate::scryglass::StageRoute::Explore(
            crate::world_viz::Building::Keep,
        ));
    paint(&mut app, 160, 52);
    let frame = app.panel_frames.get(panels::PanelKind::Artifacts).unwrap();
    let body = app.panes.rect_of(mouse::PaneId::Artifacts).unwrap();
    app.scroll = 7;
    app.input = "unsubmitted draft".into();
    for point in [
        (frame.x, frame.y),
        (frame.right() - 1, frame.bottom() - 1),
        (body.x + 3, body.y + 3),
    ] {
        app.focus_module("core");
        app.on_mouse(mouse_ev(MouseEventKind::ScrollUp, point.0, point.1));
        assert_eq!(app.scroll, 7);
        assert_eq!(app.module_host.focused().unwrap().as_str(), "artifacts");
    }
    assert!(app.scryglass.fov < 1.05);
    let start = (body.x + body.width / 2, body.y + body.height / 2);
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        start.0,
        start.1,
    ));
    let before = (app.scryglass.look_yaw, app.scryglass.look_pitch);
    paint(&mut app, 160, 52);
    let scroll_after_layout = app.scroll;
    assert!(
        app.scryglass_drag.is_some(),
        "redraw must not cancel valid gesture"
    );
    app.on_mouse(mouse_ev(
        MouseEventKind::Drag(MouseButton::Left),
        start.0 + 4,
        start.1 + 2,
    ));
    assert_ne!((app.scryglass.look_yaw, app.scryglass.look_pitch), before);
    app.on_mouse(mouse_ev(MouseEventKind::Up(MouseButton::Left), 0, 0));
    assert!(app.scryglass_drag.is_none());
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Right),
        start.0,
        start.1,
    ));
    assert!(app.scryglass.follow_agent);
    assert_eq!(app.scryglass.look_yaw, 0.0);
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        start.0,
        start.1,
    ));
    app.focus_module("core");
    assert!(app.scryglass_drag.is_none());
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        start.0,
        start.1,
    ));
    assert_eq!(
        app.scroll, scroll_after_layout,
        "gestures must not scroll the transcript"
    );
    paint(&mut app, 144, 44);
    // Transcript reflow may legitimately rebase its scroll on resize; that
    // is not wheel leakage. The gesture must nevertheless be gone.
    assert!(app.scryglass_drag.is_none());
    assert_eq!(app.input, "unsubmitted draft");
    assert!(app.selection.is_none());
}

#[test]
fn miniviz_follow_survives_shared_composer_border_and_resets_at_its_visible_cells() {
    let _guard = env_lock();
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "in_process");
    let _world = TestEnvGuard::set("ANGEL_SCRYGLASS", "1");
    let _protocol = TestEnvGuard::set("ANGEL_IMAGE_PROTOCOL", "halfblocks");
    let _comp = TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::comp_mode::invalidate_cache();
    let _view = crate::world_viz::world3d::pin(crate::world_viz::world3d::WorldView::Mesh3d);
    for (width, height) in [(114, 44), (120, 46), (160, 52)] {
        let mut app = fixture(mouse::PaneId::Transcript);
        app.scryglass
            .navigate(crate::scryglass::StageRoute::Explore(
                crate::world_viz::Building::Keep,
            ));
        paint(&mut app, width, height);
        app.scryglass.adjust_look(0.5, 0.2);
        app.scryglass.adjust_fov(-0.2);
        app.input = "unsubmitted draft".into();
        let buf = paint(&mut app, width, height);
        let rect = app
            .world_buttons
            .iter()
            .find_map(|(rect, button)| {
                matches!(button, WorldButton::ScryglassFollow).then_some(*rect)
            })
            .expect("visible reset hit target");
        let text: String = (rect.x..rect.right())
            .map(|x| buf.cell((x, rect.y)).unwrap().symbol())
            .collect();
        assert_eq!(
            text, "[Follow]",
            "reset must survive the complete {width}x{height} compositor"
        );
        app.on_mouse(mouse_ev(
            MouseEventKind::Down(MouseButton::Left),
            rect.x + 2,
            rect.y,
        ));
        assert!(app.scryglass.follow_agent);
        assert_eq!(app.scryglass.look_yaw, 0.0);
        assert_eq!(app.input, "unsubmitted draft");
    }
}

#[test]
fn trace_divider_tweens_without_moving_transcript_or_losing_copy_ownership() {
    let _guard = env_lock();
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "in_process");
    let _world = TestEnvGuard::set("ANGEL_SCRYGLASS", "1");
    let mut app = seed_preview_app();
    app.visual_motion = crate::viz::lifecycle_viz::MotionMode::Full;
    app.terminal_focused = true;
    paint(&mut app, 160, 48);
    app.last_resize_at = Instant::now() - Duration::from_secs(1);
    let idle = app.panel_frames.get(panels::PanelKind::AgentBay).unwrap();
    let prose = app.panes.rect_of(mouse::PaneId::Transcript).unwrap();
    app.reasoning = "TRACE_VISIBLE: inspect the evidence before deciding".repeat(20);
    app.reasoning_shown = app.reasoning.len();
    paint(&mut app, 160, 48);
    assert_eq!(
        app.panel_frames
            .get(panels::PanelKind::AgentBay)
            .unwrap()
            .height,
        idle.height
    );
    assert!(app.divider_motion.active(Instant::now()));
    app.divider_motion
        .advance_for_test(Duration::from_millis(160));
    paint(&mut app, 160, 48);
    let between = app.panel_frames.get(panels::PanelKind::AgentBay).unwrap();
    assert!(between.height > idle.height);
    assert_eq!(app.panes.rect_of(mouse::PaneId::Transcript).unwrap(), prose);
    let trace = app.panes.rect_of(mouse::PaneId::AgentBay).unwrap();
    let mini = app.panel_frames.get(panels::PanelKind::Artifacts).unwrap();
    assert!(trace.intersection(mini).is_empty());
    app.divider_motion
        .advance_for_test(Duration::from_millis(500));
    paint(&mut app, 160, 48);
    assert!(!app.divider_motion.active(Instant::now()));
    let settled = app.panel_frames.get(panels::PanelKind::AgentBay).unwrap();
    app.reasoning.clear();
    app.reasoning_shown = 0;
    paint(&mut app, 160, 48);
    assert_eq!(
        app.panel_frames
            .get(panels::PanelKind::AgentBay)
            .unwrap()
            .height,
        settled.height
    );
    app.divider_motion
        .advance_for_test(Duration::from_millis(500));
    paint(&mut app, 160, 48);
    assert_eq!(
        app.panel_frames
            .get(panels::PanelKind::AgentBay)
            .unwrap()
            .height,
        idle.height
    );
}
