use super::*;
use ratatui::{Terminal, backend::TestBackend};

fn seeded(count: usize) -> App {
    let mut app = App::preview(Viewer::static_preview());
    app.messages = (0..count)
        .map(|i| {
            Message::new(
                Role::User,
                format!(
                    "message-{i} {}",
                    "bounded Unicode 日本語 👩‍🔬 e\u{301} ".repeat(5)
                ),
            )
        })
        .collect();
    app.last_resize_at = Instant::now() - RESIZE_SETTLE;
    step(&mut app, 80, usize::MAX);
    app
}

fn step(app: &mut App, width: usize, count: usize) {
    sync_transcript_heights_budgeted(app, width, count, || false);
}

fn assert_exact(app: &App, width: usize) {
    let expected: Vec<u16> = app
        .messages
        .iter()
        .map(|message| transcript::message_height(message, width))
        .collect();
    assert_eq!(app.transcript_heights, expected);
    let mut total = 0u32;
    let prefix: Vec<u32> = std::iter::once(0)
        .chain(expected.iter().map(|height| {
            total = total.saturating_add(u32::from(*height));
            total
        }))
        .collect();
    assert_eq!(app.transcript_height_prefix, prefix);
    assert_eq!(usize::from(app.transcript_heights_w), width);
}

#[test]
fn reflow_publishes_atomically_and_includes_append_without_restarting() {
    let mut app = seeded(20);
    let published = app.transcript_height_prefix.clone();
    step(&mut app, 24, 7);
    assert_eq!(app.transcript_height_prefix, published);
    let pending_prefix = app
        .pending_transcript_reflow
        .as_ref()
        .unwrap()
        .prefix
        .clone();
    app.messages
        .push(Message::new(Role::Angel, "appended **answer** ".repeat(20)));
    step(&mut app, 24, 4);
    assert_exact(&app, 80);
    assert_eq!(
        &app.pending_transcript_reflow.as_ref().unwrap().prefix[..8],
        pending_prefix
    );
    assert_eq!(
        app.pending_transcript_reflow
            .as_ref()
            .unwrap()
            .heights
            .len(),
        11
    );
    step(&mut app, 24, 10);
    assert!(app.pending_transcript_reflow.is_none());
    assert_exact(&app, 24);
}

#[test]
fn reflow_latest_width_wins_and_debounce_appends_at_published_width() {
    let mut app = seeded(20);
    step(&mut app, 24, 7);
    step(&mut app, 40, 3);
    let pending = app.pending_transcript_reflow.as_ref().unwrap();
    assert_eq!((pending.width, pending.heights.len()), (40, 3));
    step(&mut app, 80, 3);
    assert!(app.pending_transcript_reflow.is_none());
    app.last_resize_at = Instant::now();
    app.messages
        .push(Message::new(Role::User, "wrapping append ".repeat(10)));
    step(&mut app, 24, 20);
    assert!(app.pending_transcript_reflow.is_none());
    assert_exact(&app, 80);
}

#[test]
fn reflow_rewrite_and_shrink_discard_stale_indices() {
    let mut app = seeded(20);
    step(&mut app, 24, 7);
    app.messages[0] = Message::new(Role::User, "changed same-count content");
    app.invalidate_transcript_layout();
    assert!(app.pending_transcript_reflow.is_none());
    step(&mut app, 24, 1);
    assert_eq!(
        app.transcript_heights.len(),
        1,
        "cold work must obey the count budget"
    );
    step(&mut app, 24, usize::MAX);
    assert_exact(&app, 24);
    step(&mut app, 40, 7);
    app.messages.truncate(10);
    step(&mut app, 40, 1);
    step(&mut app, 40, usize::MAX);
    assert!(app.pending_transcript_reflow.is_none());
    assert_exact(&app, 40);
}

#[test]
fn reflow_selection_holds_complete_geometry_without_fast_tick_spin() {
    let mut app = seeded(20);
    let rect = Rect::new(0, 0, 24, 10);
    app.panes.push(mouse::PaneId::Transcript, rect);
    app.selection = Some(mouse::Selection::new(mouse::PaneId::Transcript, rect, 1, 1));
    step(&mut app, 24, 7);
    assert!(app.transcript_reflow_wants_fast_tick());
    step(&mut app, 24, 20);
    assert_exact(&app, 80);
    assert!(app.pending_transcript_reflow.as_ref().unwrap().ready(20));
    assert!(!app.transcript_reflow_wants_fast_tick());
    app.selection = None;
    step(&mut app, 24, 0);
    assert_exact(&app, 24);
}

#[test]
fn reflow_blur_keeps_progress_and_hidden_or_zero_area_stops_fast_ticks() {
    let mut app = seeded(20);
    step(&mut app, 24, 7);
    app.panes
        .push(mouse::PaneId::Transcript, Rect::new(0, 0, 24, 10));
    assert!(app.transcript_reflow_wants_fast_tick());
    app.terminal_focused = false;
    step(&mut app, 24, 20);
    assert_exact(&app, 24);
    assert!(app.pending_transcript_reflow.is_none());
    assert!(!app.transcript_reflow_wants_fast_tick());
    app.terminal_focused = true;
    step(&mut app, 40, 7);
    assert_eq!(
        app.pending_transcript_reflow
            .as_ref()
            .unwrap()
            .heights
            .len(),
        7
    );
    app.panes.clear();
    assert!(!app.transcript_reflow_wants_fast_tick());
    app.panes
        .push(mouse::PaneId::Transcript, Rect::new(0, 0, 24, 10));
    let mut terminal = Terminal::new(TestBackend::new(0, 0)).unwrap();
    terminal
        .draw(|frame| super::super::ui(frame, &mut app))
        .unwrap();
    assert!(!app.transcript_reflow_wants_fast_tick());
    assert_eq!(
        app.pending_transcript_reflow
            .as_ref()
            .unwrap()
            .heights
            .len(),
        7
    );
}

#[test]
fn reflow_commit_anchors_the_readers_current_message() {
    let mut app = seeded(20);
    app.visual_motion = MotionMode::Off;
    app.settle_transcript_spawns();
    let area = Rect::new(0, 0, 27, 10); // 24 text cells
    let mut terminal = Terminal::new(TestBackend::new(27, 10)).unwrap();
    app.selection = Some(mouse::Selection::new(mouse::PaneId::Transcript, area, 1, 1));
    step(&mut app, 24, 20); // ready but held
    terminal
        .draw(|frame| render_transcript(frame, &mut app, area))
        .unwrap();
    let wanted = 6;
    let intra = 1;
    app.scroll = app.transcript_last_bottom - (app.transcript_height_prefix[wanted] as u16 + intra);
    app.selection = None;
    terminal
        .draw(|frame| render_transcript(frame, &mut app, area))
        .unwrap();
    assert_exact(&app, 24);
    let top = app.transcript_last_bottom - app.scroll;
    let (message, offset, _) =
        transcript_window(&app.transcript_height_prefix, u32::from(top), 1, 20);
    assert_eq!((message, offset), (wanted, usize::from(intra)));
    app.scroll = 0;
    step(&mut app, 40, 5);
    let wide = Rect::new(0, 0, 43, 12);
    terminal.backend_mut().resize(43, 12);
    terminal.autoresize().unwrap();
    for _ in 0..20 {
        terminal
            .draw(|frame| render_transcript(frame, &mut app, wide))
            .unwrap();
    }
    assert_exact(&app, 40);
    assert_eq!(app.scroll, 0);
}

#[test]
#[ignore = "explicit resize completion measurement, including asynchronous giant messages"]
fn resize_reflow_completion_measurement() {
    for giant in [false, true] {
        for repetition in 0..5 {
            let mut app = seeded(if giant { 1 } else { 2000 });
            if giant {
                app.messages[0] = Message::new(Role::User, "giant text 界 ".repeat(20_000));
                app.invalidate_transcript_layout();
                let cold_deadline = Instant::now() + Duration::from_secs(15);
                while app.transcript_heights.len() != app.messages.len() {
                    sync_transcript_heights(&mut app, 80);
                    assert!(
                        Instant::now() < cold_deadline,
                        "cold giant index did not settle"
                    );
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
            let mut ordered_us = Vec::new();
            let mut cursors = Vec::new();
            let started = Instant::now();
            for _ in 0..1000 {
                let step_started = Instant::now();
                sync_transcript_heights(&mut app, 24);
                ordered_us.push(step_started.elapsed().as_micros() as u64);
                cursors.push(
                    app.pending_transcript_reflow
                        .as_ref()
                        .map_or(app.messages.len(), |work| work.heights.len()),
                );
                if app.transcript_heights_w == 24 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(33));
            }
            let completion_us = started.elapsed().as_micros() as u64;
            assert_exact(&app, 24);
            eprintln!(
                "REFLOW_COMPLETION {}",
                serde_json::json!({
                    "giant": giant, "repetition": repetition, "messages": app.messages.len(),
                    "ordered_us": ordered_us, "cursors": cursors, "completion_us": completion_us,
                    "cadence": "33 ms between height steps, excludes full UI frame work",
                    "index_capacity_bytes": app.transcript_heights.capacity() * 2 + app.transcript_height_prefix.capacity() * 4,
                })
            );
        }
    }
}

#[test]
fn reflow_commit_keeps_partial_anchor_after_growth_and_body_height_change() {
    let mut app = seeded(20);
    app.visual_motion = MotionMode::Off;
    app.thinking = Some(Thinking::pending_for_test("practice"));
    app.partial = "partial output line\n".repeat(40);
    let mut area = Rect::new(0, 0, 27, 10);
    let mut terminal = Terminal::new(TestBackend::new(27, 10)).unwrap();
    app.selection = Some(mouse::Selection::new(mouse::PaneId::Transcript, area, 1, 1));
    step(&mut app, 24, 20);
    terminal
        .draw(|frame| render_transcript(frame, &mut app, area))
        .unwrap();
    let intra = 5;
    let hist = *app.transcript_height_prefix.last().unwrap() as u16;
    app.scroll = app.transcript_last_bottom - (hist + intra);
    app.partial.push_str(&"more streamed lines\n".repeat(5));
    app.selection = None;
    area.height = 14;
    terminal.backend_mut().resize(27, 14);
    terminal.autoresize().unwrap();
    terminal
        .draw(|frame| render_transcript(frame, &mut app, area))
        .unwrap();
    assert_exact(&app, 24);
    let top = app.transcript_last_bottom - app.scroll;
    assert_eq!(
        u32::from(top),
        app.transcript_height_prefix.last().unwrap() + u32::from(intra)
    );
}
