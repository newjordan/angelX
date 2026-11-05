//! Model-free production-compositor fixtures for the operator overlap report.
use super::*;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn fixture() -> App {
    let mut app = seed_preview_app();
    app.scryglass_enabled = true;
    app.scryglass.controller.reset(scryglass::StageRoute::Quest);
    app.focus_module("artifacts");
    let id = harness::ToolEventId("u09-failed-shell".into());
    let command = format!(
        "cmd=echo U09_COMMAND {}; echo END",
        "long-command-界 ".repeat(40)
    );
    app.realm
        .world
        .note_tool_call_event(id.clone(), "shell", &command);
    app.realm.world.note_tool_result_event(
        &id,
        "shell",
        "scripted failure",
        harness::ToolOutcome {
            execution: harness::ExecutionOutcome::Failed,
            verification: harness::VerificationOutcome::NotApplicable,
        },
    );
    app
}

fn capture(app: &mut App, width: u16, height: u16, name: &str) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| draw::ui(frame, app)).unwrap();
    let text = test_backend_text(terminal.backend());
    if let Ok(out) = std::env::var("ANGEL_T_U09_CAPTURE_DIR") {
        let out = PathBuf::from(out);
        std::fs::create_dir_all(&out).unwrap();
        let stem = format!("{width}x{height}-{name}");
        std::fs::write(out.join(format!("{stem}.txt")), &text).unwrap();
        let mut palette = Vec::new();
        let cells: Vec<_> = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| {
                let style = (
                    format!("{:?}", cell.fg),
                    format!("{:?}", cell.bg),
                    format!("{:?}", cell.modifier),
                );
                let index = palette.iter().position(|s| s == &style).unwrap_or_else(|| {
                    palette.push(style);
                    palette.len() - 1
                });
                (cell.symbol(), index)
            })
            .collect();
        let value =
            serde_json::json!({"width":width, "height":height, "palette":palette, "cells":cells});
        std::fs::write(
            out.join(format!("{stem}.json")),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }
    text
}

#[test]
fn u09_review_capture() {
    let _lock = env_lock();
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "on");
    let _info = TestEnvGuard::unset("ANGEL_AGENT_INFO_HEADER");
    let _comp = TestEnvGuard::unset("ANGEL_COMP_MODE");
    comp_mode::invalidate_cache();
    for (w, h) in [(284, 54), (120, 40), (80, 24)] {
        let mut app = fixture();
        capture(&mut app, w, h, "quest");
        app.loop_command(Some("start U09 local fixture".into()));
        capture(&mut app, w, h, "loop-quest");
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        capture(&mut app, w, h, "loop-esc");
        app.open_moa_deck(None);
        capture(&mut app, w, h, "formation-quest");
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        // A resident PNG is the other reported obstruction. Generated locally;
        // no provider, screenshot, or archived asset is needed.
        let dir = std::env::temp_dir().join(format!("angel-u09-{}-{w}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old-map.png");
        image::RgbImage::from_fn(64, 32, |x, y| {
            image::Rgb([40, (x * 3) as u8, (y * 6) as u8])
        })
        .save(&path)
        .unwrap();
        app.tools = Arc::new(harness::ToolRegistry::with_team(dir.clone(), Vec::new()));
        app.present_media("image", "U09 old map PNG", path.to_str().unwrap());
        let deadline = Instant::now() + Duration::from_secs(5);
        while !app.scryglass.media_ready() {
            render_app_text(&mut app, w, h);
            assert!(
                Instant::now() < deadline,
                "local fixture PNG did not decode"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        capture(&mut app, w, h, "png");
        app.loop_command(Some("start U09 local fixture".into()));
        capture(&mut app, w, h, "loop-png");
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        capture(&mut app, w, h, "loop-png-esc");
        // Re-present for an independent Formation/media reproduction even if
        // the old Esc implementation accidentally dismissed the resident PNG.
        app.present_media("image", "U09 old map PNG", path.to_str().unwrap());
        app.open_moa_deck(None);
        capture(&mut app, w, h, "formation-png");
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        capture(&mut app, w, h, "formation-esc");
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn u09_header_activity_never_shares_the_picker_row() {
    let _lock = env_lock();
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "on");
    let _info = TestEnvGuard::unset("ANGEL_AGENT_INFO_HEADER");
    let _comp = TestEnvGuard::unset("ANGEL_COMP_MODE");
    comp_mode::invalidate_cache();
    for (w, h) in [(284, 54), (120, 40), (80, 24)] {
        let mut app = fixture();
        let text = capture(&mut app, w, h, "header-test");
        let controls = app.agent_control_area.expect("header controls");
        let lines: Vec<_> = text.lines().collect();
        let row = lines[usize::from(controls.y)];
        assert!(row.contains("[MODEL:"), "{text}");
        assert!(
            !row.contains("failed") && !row.contains("界") && !row.contains("U09_COMMAND"),
            "picker row contains tool text: {text}"
        );
        assert!(
            app.realm
                .world
                .causal_ribbon()
                .is_some_and(|ribbon| ribbon.contains("failed")),
            "failure remains in the event state"
        );
    }
}

#[test]
fn u09_loop_modal_is_topmost_and_esc_preserves_the_underlying_surface() {
    let _lock = env_lock();
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "on");
    for (w, h) in [(284, 54), (120, 40), (80, 24)] {
        let mut app = fixture();
        app.scryglass
            .controller
            .show_overlay(scryglass::StageOverlay::Catalog);
        app.loop_command(Some("start U09 local fixture".into()));
        let mut actual = Terminal::new(TestBackend::new(w, h)).unwrap();
        actual.draw(|f| draw::ui(f, &mut app)).unwrap();
        let mut expected = Terminal::new(TestBackend::new(w, h)).unwrap();
        let dialog = app.loop_dialog.as_ref().unwrap();
        expected
            .draw(|f| {
                let area = f.area();
                crate::ui::loop_card::render_terminal(f.buffer_mut(), area, dialog);
            })
            .unwrap();
        // 4 lap presets plus the always-present custom length slot, 5
        // clocks, 4 fuel caps, an arrow each side of every picker, then
        // DROP THE FLAG and cancel.
        assert_eq!(
            app.loop_dialog_hits.len(),
            (4 + 1) + 5 + 4 + 3 * 2 + 2,
            "every choice and action must have a visible hit target"
        );
        let card = crate::ui::loop_card::terminal_area(ratatui::layout::Rect::new(0, 0, w, h));
        for y in card.top()..card.bottom() {
            for x in card.left()..card.right() {
                assert_eq!(
                    actual.backend().buffer()[(x, y)],
                    expected.backend().buffer()[(x, y)],
                    "overlay cell {x},{y} at {w}x{h}"
                );
            }
        }
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.loop_dialog.is_none());
        assert!(app.loop_dialog_hits.is_empty());
        assert_eq!(
            app.scryglass.controller.route(),
            scryglass::StageRoute::Quest
        );
        assert_eq!(
            app.scryglass.controller.overlay(),
            Some(&scryglass::StageOverlay::Catalog)
        );
    }
}

#[test]
fn u09_formation_overlays_resident_media_and_restores_the_route() {
    let _lock = env_lock();
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "on");
    let _comp = TestEnvGuard::unset("ANGEL_COMP_MODE");
    comp_mode::invalidate_cache();
    for (w, h) in [(284, 54), (120, 40), (80, 24)] {
        let mut app = fixture();
        app.scryglass
            .controller
            .show_overlay(scryglass::StageOverlay::Media { index: 0 });
        app.open_moa_deck(None);
        let text = capture(&mut app, w, h, "formation-test");
        assert!(app.moa_deck_owns_input());
        assert_eq!(app.scryglass.surface, scryglass::StageSurface::Moa);
        assert!(
            !app.world_pane_visible,
            "covered world must not claim visibility"
        );
        assert!(text.contains("Agent Formation"), "{text}");
        let area = app.panes.rect_of(mouse::PaneId::Artifacts).unwrap();
        assert_eq!(
            area.width,
            w - 2,
            "Formation uses the body, not the narrow world column"
        );
        assert!(
            app.agent_buttons
                .iter()
                .all(|(rect, _)| !rect.intersects(area)),
            "covered Agent buttons must not intercept Formation clicks"
        );
        app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.moa_deck.as_ref().unwrap().selected_index(), 1);
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.moa_deck.is_none());
        assert_eq!(
            app.scryglass.controller.route(),
            scryglass::StageRoute::Quest
        );
        assert_eq!(
            app.scryglass.controller.overlay(),
            Some(&scryglass::StageOverlay::Media { index: 0 })
        );
    }
}

#[test]
fn u09_every_loop_choice_and_action_is_keyboard_reachable_at_80x24() {
    let _lock = env_lock();
    let mut app = fixture();
    app.scryglass
        .controller
        .show_overlay(scryglass::StageOverlay::Catalog);
    app.loop_command(Some("start U09 local fixture".into()));
    // Preserve the keyboard cycle: clock, laps (including custom), fuel.
    for (field, count) in [(0, 5), (1, 4 + 1), (2, 4)] {
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..count {
            let text = capture(
                &mut app,
                80,
                24,
                &format!("keyboard-{field}-{}", seen.len()),
            );
            assert!(
                text.contains("DROP THE FLAG")
                    && text.contains("cancel")
                    && !text.contains("make the window larger"),
                "{text}"
            );
            if app.loop_dialog.as_ref().unwrap().custom_entry_active() {
                assert!(text.contains('▏') && text.contains("0-9 laps"), "{text}");
                for key in [KeyCode::Char('7'), KeyCode::Enter] {
                    app.on_key(KeyEvent::new(key, KeyModifiers::NONE));
                }
                assert!(!app.loop_dialog.as_ref().unwrap().custom_entry_active());
            }
            let settings = app.loop_dialog.as_ref().unwrap().settings();
            seen.insert(match field {
                0 => settings.deadline_secs,
                1 => settings.max_iters as u64,
                _ => settings.token_budget as u64,
            });
            app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        }
        assert_eq!(seen.len(), count, "all choices in field {field}");
        app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    }
    assert_eq!(
        app.loop_dialog.as_ref().unwrap().focused_action(),
        Some(loop_dialog::LoopDialogAction::Start)
    );
    capture(&mut app, 80, 24, "keyboard-start");
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(
        app.loop_dialog.as_ref().unwrap().focused_action(),
        Some(loop_dialog::LoopDialogAction::Cancel)
    );
    capture(&mut app, 80, 24, "keyboard-back");
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.loop_dialog.is_none());
    assert!(!app.loop_active(), "navigation must not start a run");
}
