//! Composer/caret rendering suites (module-breakup: extracted from the
//! `tests.rs` monolith). Compacted long input, caret ownership, exact-width
//! row motion, draft-preserving shortcuts, danger rails, and the untruncated
//! submit path.

use super::{
    mouse_ev, render_app_text, seed_long_input_app, seed_practice_only_app, seed_preview_app,
    test_backend_text,
};
use crate::app::WorldButton;
use crate::club::ChatRole;
use crate::draw::{composer_cursor_position, ui};
use crate::tests::env_lock;
use crate::{hud, lifecycle_viz, panels, status_view};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::sync::Arc;

#[test]
fn long_chat_input_renders_compacted_composer() {
    let _guard = env_lock();
    let mut app = seed_long_input_app();
    let text = render_app_text(&mut app, 120, 40);
    assert!(text.contains("long draft"), "{text}");
    assert!(text.contains("chars"), "{text}");
    assert!(text.contains("words"), "{text}");
}

#[test]
fn composer_publishes_the_caret_only_while_it_owns_text_input() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.input = "hello realm".to_string();
    app.cursor = 5;
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| ui(frame, &mut app)).unwrap();

    let area = app
        .panel_frames
        .get(panels::PanelKind::Input)
        .expect("composer frame");
    let composer = status_view::composer_view(
        &app.input,
        area.width.saturating_sub(2) as usize,
        area.height.saturating_sub(2),
        app.cursor,
    );
    let expected = crate::draw::composer_cursor_position(&app, area, &composer)
        .expect("the composer owns the caret");
    terminal.backend_mut().assert_cursor_position(expected);
    assert_ne!(expected, ratatui::layout::Position::ORIGIN);

    app.input = "界".repeat(200);
    app.cursor = 100;
    terminal.draw(|frame| ui(frame, &mut app)).unwrap();
    let area = app
        .panel_frames
        .get(panels::PanelKind::Input)
        .expect("composer frame");
    let wide = status_view::composer_view(
        &app.input,
        area.width.saturating_sub(2) as usize,
        area.height.saturating_sub(2),
        app.cursor,
    );
    assert!(wide.compacted);
    let expected = crate::draw::composer_cursor_position(&app, area, &wide)
        .expect("the compact Unicode composer owns the caret");
    terminal.backend_mut().assert_cursor_position(expected);

    app.focus_module("artifacts");
    app.input.clear();
    app.cursor = 0;
    let empty = status_view::composer_view(
        &app.input,
        area.width.saturating_sub(2) as usize,
        area.height.saturating_sub(2),
        app.cursor,
    );
    assert_eq!(
        crate::draw::composer_cursor_position(&app, area, &empty),
        None
    );

    app.input = "draft".to_string();
    app.cursor = app.input.chars().count();
    let draft = status_view::composer_view(
        &app.input,
        area.width.saturating_sub(2) as usize,
        area.height.saturating_sub(2),
        app.cursor,
    );
    assert!(crate::draw::composer_cursor_position(&app, area, &draft).is_some());
}

#[test]
fn exact_width_composer_caret_moves_to_the_next_row() {
    let mut app = seed_preview_app();
    app.input = "abcd".to_string();
    app.cursor = app.input.chars().count();
    let area = Rect::new(0, 0, 6, 4); // 4x2 inner composer
    let view = status_view::composer_view(&app.input, 4, 2, app.cursor);

    assert_eq!(view.cursor_col, 4, "precondition: insertion is after row");
    assert_eq!(view.cursor_row, 0);
    assert_eq!(
        composer_cursor_position(&app, area, &view),
        Some(ratatui::layout::Position::new(1, 2))
    );
}

#[test]
fn focused_scryglass_printable_shortcuts_never_steal_a_nonempty_draft() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    for shortcut in [
        'w', 'v', 'm', '[', ']', 'p', 'r', '0', 'h', 'l', 'k', 'j', '+', '=', '-', ' ',
    ] {
        let mut app = seed_preview_app();
        app.focus_module("artifacts");
        app.input = "draft:".to_string();
        app.cursor = app.input.chars().count();
        app.on_key(KeyEvent::new(KeyCode::Char(shortcut), KeyModifiers::NONE));
        assert_eq!(
            app.input,
            format!("draft:{shortcut}"),
            "Stage consumed printable shortcut {shortcut:?}"
        );
        assert_eq!(app.cursor, app.input.chars().count());
    }

    let mut exit = seed_preview_app();
    exit.focus_module("artifacts");
    exit.scryglass.surface = crate::scryglass::StageSurface::WorldMap;
    exit.input = "exit".to_string();
    exit.cursor = exit.input.chars().count();
    exit.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(
        exit.should_quit,
        "focused World Map must not steal Enter from a nonempty composer"
    );

    let mut empty = seed_preview_app();
    empty.focus_module("artifacts");
    empty.on_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE));
    assert!(empty.input.is_empty(), "an empty focused Stage owns `v`");
    assert_eq!(
        empty.scryglass.controller.route(),
        crate::scryglass::StageRoute::Vault
    );
}

#[test]
fn realm_keyboard_and_mouse_choose_landmarks() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};

    let mut app = seed_preview_app();
    app.focus_module("artifacts");
    app.scryglass.surface = crate::scryglass::StageSurface::WorldMap;
    app.world.select_landmark(crate::world_viz::Building::Keep);

    app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    assert_eq!(
        app.world.destination(),
        crate::world_viz::Building::Scriptorium
    );

    let hit = Rect::new(11, 7, 5, 4);
    app.world_buttons = vec![(
        hit,
        WorldButton::ScryglassLandmark(crate::world_viz::Building::Observatory),
    )];
    app.on_mouse(mouse_ev(
        MouseEventKind::Down(MouseButton::Left),
        hit.x + 1,
        hit.y + 1,
    ));
    assert_eq!(
        app.world.destination(),
        crate::world_viz::Building::Observatory
    );
    assert_eq!(
        app.scryglass.controller.route(),
        crate::scryglass::StageRoute::Realm
    );
}

#[test]
fn composer_gives_high_impact_commands_a_danger_signal_rail() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.focus_module("artifacts");
    app.visual_motion = lifecycle_viz::MotionMode::Off;
    app.input = "/clear".to_string();
    app.cursor = app.input.chars().count();
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| ui(frame, &mut app)).unwrap();

    let area = app
        .panel_frames
        .get(panels::PanelKind::Input)
        .expect("composer frame");
    let rail = terminal
        .backend()
        .buffer()
        .cell((area.x + area.width - 1, area.y + 1))
        .expect("composer rail cell");
    assert_eq!(rail.fg, hud::HUD_DANGER);
    let text = test_backend_text(terminal.backend());
    assert!(text.contains("review command"), "{text}");
}

#[test]
fn empty_composer_offers_a_plain_language_next_action() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    let text = render_app_text(&mut app, 120, 40);
    assert!(text.contains("Write a message…"), "{text}");
    assert!(!text.contains("Enter sends"), "{text}");
    assert!(!text.contains("Shift+Enter"), "{text}");
}

#[test]
fn submit_preserves_full_long_prompt_for_model_history() {
    let mut app = seed_practice_only_app();
    let raw = format!(
        "please inspect this whole payload: {} THE_UNTRUNCATED_TAIL",
        "x".repeat(12_000)
    );
    app.input = raw.clone();
    app.cursor = raw.chars().count();
    app.submit();
    let displayed = &app.messages.last().unwrap().text;
    assert_eq!(displayed.len(), raw.len());
    assert!(displayed.ends_with("THE_UNTRUNCATED_TAIL"));
    let sent = &app
        .history
        .iter()
        .rev()
        .find(|message| message.role == ChatRole::User)
        .expect("operator task remains in model history")
        .content;
    assert!(
        sent.contains(&raw),
        "full raw prompt must be embedded in model-facing history"
    );
    assert!(
        sent.ends_with("THE_UNTRUNCATED_TAIL"),
        "tail marker must reach the model history"
    );
    assert!(
        Arc::ptr_eq(displayed, sent),
        "echo row and model history must share one operator text buffer"
    );
}

#[test]
fn composer_hides_native_caret_while_terminal_is_unfocused() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.input = "focus regression".to_string();
    app.cursor = app.input.len();
    let area = Rect::new(1, 1, 40, 4);
    let view = status_view::composer_view(&app.input, 38, 2, app.cursor);
    assert!(composer_cursor_position(&app, area, &view).is_some());
    app.set_terminal_focused(false);
    assert!(composer_cursor_position(&app, area, &view).is_none());
    app.set_terminal_focused(true);
    assert!(composer_cursor_position(&app, area, &view).is_some());
}

#[test]
fn corrective_composer_grapheme_keys_copy_delete_and_insert_agree_with_paint() {
    use ratatui::crossterm::event::{KeyCode as K, KeyEvent, KeyModifiers as M};
    let _guard = env_lock();
    let mut app = seed_preview_app();
    for cluster in ["e\u{301}", "👩‍💻", "🇺🇸", "क्\u{200d}ष"] {
        app.input = format!("{cluster}!");
        let end = cluster.chars().count();
        app.cursor = end;
        app.composer_selection_anchor = None;
        app.on_key(KeyEvent::new(K::Left, M::SHIFT));
        assert_eq!(app.cursor, 0, "{cluster}");
        assert_eq!(app.composer_selection_range(), Some((0, end)));
        let view = status_view::composer_view_with_selection(
            &app.input,
            20,
            3,
            app.cursor,
            app.composer_selection_range(),
        );
        let painted: String = view
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .filter(|span| span.style.bg == Some(hud::HUD_BLUE))
            .map(|span| span.content.as_ref())
            .collect();
        app.on_key(KeyEvent::new(K::Char('c'), M::CONTROL | M::SHIFT));
        assert_eq!(app.pending_clipboard.as_deref(), Some(cluster));
        assert_eq!(painted, cluster);
        app.on_key(KeyEvent::new(K::Backspace, M::NONE));
        assert_eq!(app.input, "!");
        assert_eq!(app.cursor, 0);

        app.input = format!("{cluster}!");
        app.cursor = 0;
        app.on_key(KeyEvent::new(K::Right, M::NONE));
        assert_eq!(app.cursor, end);
        app.on_key(KeyEvent::new(K::Char('X'), M::NONE));
        assert_eq!(app.input, format!("{cluster}X!"));
        app.on_key(KeyEvent::new(K::Backspace, M::NONE));
        app.on_key(KeyEvent::new(K::Backspace, M::NONE));
        assert_eq!(app.input, "!");
    }
    // A restored legacy scalar offset snaps the source insertion to the same
    // boundary the renderer paints, not after the base but before its accent.
    app.input = "e\u{301}".into();
    app.cursor = 1;
    let view = status_view::composer_view(&app.input, 8, 2, app.cursor);
    assert_eq!(view.cursor_col, 0);
    app.on_key(KeyEvent::new(K::Char('X'), M::NONE));
    assert_eq!(app.input, "Xe\u{301}");
    assert_eq!(app.cursor, 1);
    // Insertion can join the following cluster; the caret must not end inside it.
    app.input = "\u{301}!".into();
    app.cursor = 0;
    app.on_paste("e");
    assert_eq!(app.input, "e\u{301}!");
    assert_eq!(app.cursor, 2);
    // Legacy partial selection expands in the clipboard and deletion too.
    app.composer_selection_anchor = Some(1);
    app.cursor = 2;
    app.on_key(KeyEvent::new(K::Char('c'), M::CONTROL | M::SHIFT));
    assert_eq!(app.pending_clipboard.as_deref(), Some("e\u{301}"));
    app.on_key(KeyEvent::new(K::Backspace, M::NONE));
    assert_eq!(app.input, "!");
}

#[test]
fn corrective_composer_vim_and_word_edits_do_not_split_clusters() {
    use ratatui::crossterm::event::{KeyCode as K, KeyEvent, KeyModifiers as M};
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.vim_mode = true;
    app.vim_normal = true;
    app.input = "e\u{301}👩‍💻!".into();
    app.cursor = 0;
    app.on_key(KeyEvent::new(K::Char('l'), M::NONE));
    assert_eq!(app.cursor, 2);
    app.on_key(KeyEvent::new(K::Char('x'), M::NONE));
    assert_eq!(app.input, "e\u{301}!");
    app.on_key(KeyEvent::new(K::Char('h'), M::NONE));
    assert_eq!(app.cursor, 0);
    app.on_key(KeyEvent::new(K::Char('a'), M::NONE));
    assert_eq!(app.cursor, 2);
    app.on_key(KeyEvent::new(K::Char('Z'), M::NONE));
    assert_eq!(app.input, "e\u{301}Z!");
    app.vim_mode = false;
    // The combining mark on a space belongs to that space's grapheme; scalar
    // word movement used to stop between the two.
    app.input = "a \u{301}b".into();
    app.cursor = 4;
    app.on_key(KeyEvent::new(K::Left, M::CONTROL | M::SHIFT));
    assert_eq!(app.cursor, 1);
    app.on_key(KeyEvent::new(K::Char('c'), M::CONTROL | M::SHIFT));
    assert_eq!(app.pending_clipboard.as_deref(), Some(" \u{301}b"));
    app.on_key(KeyEvent::new(K::Char('w'), M::CONTROL));
    assert_eq!(app.input, "a");
    app.on_key(KeyEvent::new(K::Char('y'), M::CONTROL));
    assert_eq!(app.input, "a \u{301}b");
}

#[test]
fn corrective_composer_up_down_use_painted_rows_and_resize_width() {
    use ratatui::crossterm::event::{KeyCode as K, KeyEvent, KeyModifiers as M};
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.panel_frames.clear();
    app.panel_frames
        .push(panels::PanelKind::Input, Rect::new(0, 0, 6, 7));
    app.input = "abcdefghij".into();
    app.cursor = 10;
    app.scroll = 7;
    app.on_key(KeyEvent::new(K::Up, M::NONE));
    assert_eq!(app.cursor, 6);
    app.on_key(KeyEvent::new(K::Up, M::NONE));
    assert_eq!(app.cursor, 2);
    app.on_key(KeyEvent::new(K::Down, M::SHIFT));
    assert_eq!(app.cursor, 6);
    assert_eq!(app.composer_selection_range(), Some((2, 6)));
    app.on_key(KeyEvent::new(K::Char('c'), M::CONTROL | M::SHIFT));
    assert_eq!(app.pending_clipboard.as_deref(), Some("cdef"));
    app.on_key(KeyEvent::new(K::Down, M::NONE));
    assert_eq!(app.cursor, 10);
    assert_eq!(app.composer_selection_range(), None);
    assert_eq!(app.scroll, 7);

    app.panel_frames.clear();
    app.panel_frames
        .push(panels::PanelKind::Input, Rect::new(0, 0, 8, 7));
    app.on_key(KeyEvent::new(K::Up, M::NONE));
    assert_eq!(app.cursor, 4, "uses the last painted width after resize");
    for separator in ['\n', '\u{2028}', '\u{2029}'] {
        app.input = format!("界e\u{301}{separator}abcd");
        app.cursor = 6; // cell column two in the second row
        app.on_key(KeyEvent::new(K::Up, M::NONE));
        assert_eq!(app.cursor, 1, "{separator:?}");
        app.on_key(KeyEvent::new(K::Down, M::NONE));
        assert_eq!(app.cursor, 6);
        assert_eq!(app.scroll, 7);
    }
    app.input.clear();
    app.cursor = 0;
    app.on_key(KeyEvent::new(K::Up, M::NONE));
    assert_eq!(app.scroll, 8);
    app.on_key(KeyEvent::new(K::Down, M::NONE));
    assert_eq!(app.scroll, 7);
    // Empty means the *focused view*, not invariably the transcript.
    app.focus_module("agent");
    app.reasoning_scroll = 3;
    app.on_key(KeyEvent::new(K::Up, M::NONE));
    assert_eq!(app.reasoning_scroll, 4);
    assert_eq!(app.scroll, 7);
    app.on_key(KeyEvent::new(K::Down, M::NONE));
    assert_eq!(app.reasoning_scroll, 3);
    app.input = "abcdefghij".into();
    app.cursor = 10;
    app.on_key(KeyEvent::new(K::Up, M::NONE));
    assert_eq!(app.cursor, 4);
    assert_eq!(app.reasoning_scroll, 3);
}

#[test]
fn corrective_composer_exact_width_hard_break_has_a_real_insertion_cell() {
    use ratatui::crossterm::event::{KeyCode as K, KeyEvent, KeyModifiers as M};
    let _guard = env_lock();
    let mut app = seed_preview_app();
    let area = Rect::new(2, 3, 6, 5);
    app.panel_frames.clear();
    app.panel_frames.push(panels::PanelKind::Input, area);
    for text in ["abcd\nx", "abcd\u{2028}x", "界界\u{2029}x"] {
        app.input = text.into();
        app.cursor = text
            .chars()
            .position(|c| ['\n', '\u{2028}', '\u{2029}'].contains(&c))
            .unwrap();
        let cursor = app.cursor;
        let view = status_view::composer_view(&app.input, 4, 3, cursor);
        let pos = composer_cursor_position(&app, area, &view).unwrap();
        assert_eq!((pos.x, pos.y), (area.x + 1, area.y + 2));
        assert!(view.lines[1].spans.iter().all(|s| s.content.is_empty()));
        app.on_key(KeyEvent::new(K::Down, M::NONE));
        assert_eq!(app.cursor, cursor + 1);
        app.on_key(KeyEvent::new(K::Up, M::NONE));
        assert_eq!(app.cursor, cursor);
        app.on_key(KeyEvent::new(K::Char('!'), M::NONE));
        let mut expected = text.to_string();
        let byte = text.char_indices().nth(cursor).unwrap().0;
        expected.insert(byte, '!');
        assert_eq!(app.input, expected);
    }
}

#[test]
fn corrective_composer_changing_megabyte_draft_keeps_edit_and_render_cost_small() {
    use ratatui::crossterm::event::{KeyCode as K, KeyEvent, KeyModifiers as M};
    let _guard = env_lock();
    let mut app = seed_preview_app();
    // Test both wordy and unbroken input: classifying a single huge token must
    // not defeat the bounded render path either.
    for seed in ["word ", "x"] {
        app.input = seed.repeat(1_048_576 / seed.len());
        app.cursor = app.input.len();
        let started = std::time::Instant::now();
        for i in 0..100 {
            app.on_key(KeyEvent::new(K::Char('Z'), M::NONE));
            let width = [40, 80, 160][i % 3];
            assert_eq!(status_view::composer_height(&app.input, width + 2, 30), 7);
            let view = status_view::composer_view_with_selection(
                &app.input,
                width as usize,
                5,
                app.cursor,
                Some((app.cursor - 1, app.cursor)),
            );
            assert!(view.compacted);
            assert_eq!(view.lines.len(), 5);
            assert_eq!(
                view.lines
                    .iter()
                    .flat_map(|line| &line.spans)
                    .filter(|span| span.style.bg == Some(hud::HUD_BLUE))
                    .map(|span| span.content.as_ref())
                    .collect::<String>(),
                "Z"
            );
            app.on_key(KeyEvent::new(K::Backspace, M::NONE));
            let _ = status_view::composer_view(&app.input, width as usize, 5, app.cursor);
        }
        let elapsed = started.elapsed();
        eprintln!(
            "changing 1 MiB {seed:?}: 100 insert/backspace + resize/paint cycles: {elapsed:?}"
        );
        assert!(elapsed < std::time::Duration::from_secs(2), "{elapsed:?}");
        // Changed middle text and navigation must also retain global offsets.
        app.cursor = app.input.len() / 2;
        app.on_key(KeyEvent::new(K::Char('Z'), M::NONE));
        let view = status_view::composer_view_with_selection(
            &app.input,
            80,
            5,
            app.cursor,
            Some((app.cursor - 1, app.cursor)),
        );
        assert!(
            view.lines
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| span.content == "Z" && span.style.bg == Some(hud::HUD_BLUE))
        );
        let before = app.cursor;
        app.on_key(KeyEvent::new(K::Up, M::NONE));
        assert_eq!(app.cursor, before - 80);
        app.on_key(KeyEvent::new(K::Down, M::NONE));
        assert_eq!(app.cursor, before);
    }
}

#[test]
fn final_composer_observatory_arrows_cross_complete_graphemes() {
    use ratatui::crossterm::event::{KeyCode as K, KeyEvent, KeyModifiers as M};
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.focus_module("artifacts");
    app.scryglass
        .navigate(crate::scryglass::StageRoute::Observatory);
    let _ = render_app_text(&mut app, 144, 48);
    assert_eq!(
        app.scryglass.surface,
        crate::scryglass::StageSurface::Observatory
    );
    for cluster in ["e\u{301}", "👩‍💻", "🇺🇸"] {
        app.input = format!("{cluster}!");
        app.cursor = 0;
        let end = cluster.chars().count();
        app.on_key(KeyEvent::new(K::Right, M::NONE));
        assert_eq!(app.cursor, end, "Right must cross {cluster}");
        app.on_key(KeyEvent::new(K::Right, M::NONE));
        assert_eq!(app.cursor, end + 1);
        app.on_key(KeyEvent::new(K::Left, M::NONE));
        assert_eq!(app.cursor, end);
        app.on_key(KeyEvent::new(K::Left, M::NONE));
        assert_eq!(app.cursor, 0);
    }
}

#[test]
fn final_composer_yank_joined_grapheme_agrees_with_caret_and_backspace() {
    use ratatui::crossterm::event::{KeyCode as K, KeyEvent, KeyModifiers as M};
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.input = "e".to_string();
    app.cursor = 1;
    app.on_key(KeyEvent::new(K::Char('u'), M::CONTROL));
    assert!(app.input.is_empty());
    app.input = "\u{301}!".to_string();
    app.cursor = 0;
    app.on_key(KeyEvent::new(K::Char('y'), M::CONTROL));
    assert_eq!(app.input, "e\u{301}!");
    assert_eq!(app.cursor, 2, "yank must finish past the joined grapheme");
    let view = status_view::composer_view(&app.input, 20, 3, app.cursor);
    assert_eq!(view.cursor_col, 1);
    app.on_key(KeyEvent::new(K::Backspace, M::NONE));
    assert_eq!(app.input, "!");
    assert_eq!(app.cursor, 0);
}
