//! Production rendering and input controls for oversized transcript work.
use super::*;
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};

fn giant(marker: &str) -> Message {
    Message::new(
        Role::User,
        format!(
            "{marker}-HEAD\n{}\n{marker}-TAIL",
            "giant text 界 e\u{301} 👩‍🔬 ".repeat(12_000)
        ),
    )
}

fn app() -> App {
    let mut app = App::preview(Viewer::static_preview());
    app.visual_motion = MotionMode::Off;
    app.messages.clear();
    app.invalidate_transcript_layout();
    app.last_resize_at = Instant::now() - RESIZE_SETTLE;
    app
}

fn draw(app: &mut App, terminal: &mut Terminal<TestBackend>) {
    terminal
        .draw(|frame| render_transcript(frame, app, frame.area()))
        .unwrap();
}

fn settle(app: &mut App, terminal: &mut Terminal<TestBackend>) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        draw(app, terminal);
        if app.transcript_heights.len() == app.messages.len()
            && app.pending_transcript_reflow.is_none()
            && !app.transcript_layouts.busy()
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "oversized transcript did not settle"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(app.transcript_layouts.failure().is_none());
}

fn text(terminal: &Terminal<TestBackend>) -> String {
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

#[test]
fn cold_and_appended_giant_messages_yield_then_show_the_actual_tail() {
    let mut app = app();
    let mut terminal = Terminal::new(TestBackend::new(43, 12)).unwrap();
    app.messages.push(giant("COLD"));
    draw(&mut app, &mut terminal);
    assert!(
        app.transcript_heights.is_empty(),
        "cold giant wrapped synchronously"
    );
    assert!(text(&terminal).contains("Preparing transcript"));
    settle(&mut app, &mut terminal);
    assert!(text(&terminal).contains("COLD-TAIL"));
    let first_height = app.transcript_heights[0];
    app.messages.push(giant("APPEND"));
    draw(&mut app, &mut terminal);
    assert_eq!(app.transcript_heights, [first_height]);
    settle(&mut app, &mut terminal);
    assert!(text(&terminal).contains("APPEND-TAIL"));
    assert_eq!(
        app.transcript_heights[1],
        transcript::message_height(&app.messages[1], 40)
    );
}

#[test]
fn selected_pixels_survive_async_resize_and_appended_content() {
    let mut app = app();
    app.messages.push(giant("SELECTED"));
    let mut terminal = Terminal::new(TestBackend::new(43, 12)).unwrap();
    settle(&mut app, &mut terminal);
    let painted = app.transcript_layouts.painted.clone().unwrap();
    app.selection = Some(mouse::Selection::new(
        mouse::PaneId::Transcript,
        Rect::new(1, 1, 40, painted.area.height),
        1,
        1,
    ));
    app.messages.push(giant("UNSELECTED"));
    terminal.backend_mut().resize(27, 12);
    terminal.autoresize().unwrap();
    for _ in 0..20 {
        terminal
            .draw(|frame| {
                render_transcript(frame, &mut app, frame.area());
                // Selection extraction uses the Frame before backend diffing.
                // TestBackend omits hidden wide-glyph continuation cells during
                // flush, so their post-flush styles are not a visible-pixel oracle.
                // The idle Dotmax row is chrome, outside selected prose.
                let body = Rect::new(1, 1, 24, painted.area.height);
                for y in 0..body.height {
                    for x in 0..body.width {
                        use unicode_width::UnicodeWidthStr;
                        let source = &painted[(x, y)];
                        let actual = &frame.buffer_mut()[(body.x + x, body.y + y)];
                        if source.symbol().width() > usize::from(body.width - x) {
                            assert_eq!(actual.symbol(), " ", "half a glyph must be clipped");
                            assert_eq!(actual.style(), source.style());
                        } else {
                            assert_eq!(actual, source, "selected frame cell {x},{y} changed");
                        }
                    }
                }
            })
            .unwrap();
        assert_eq!(app.transcript_layouts.painted.as_ref(), Some(&painted));
        std::thread::sleep(Duration::from_millis(1));
    }
    app.selection = None;
    settle(&mut app, &mut terminal);
    assert!(text(&terminal).contains("UNSELECTED-TAIL"));
}

#[test]
fn giant_reader_anchor_is_preserved_when_the_complete_resize_index_publishes() {
    let mut app = app();
    app.messages = vec![
        Message::new(Role::User, "before"),
        giant("READER"),
        Message::new(Role::User, "after"),
    ];
    let mut terminal = Terminal::new(TestBackend::new(83, 12)).unwrap();
    settle(&mut app, &mut terminal);
    let intra = 17;
    app.scroll = app.transcript_last_bottom - (app.transcript_height_prefix[1] as u16 + intra);
    settle(&mut app, &mut terminal);
    terminal.backend_mut().resize(27, 12);
    terminal.autoresize().unwrap();
    settle(&mut app, &mut terminal);
    let top = app.transcript_last_bottom - app.scroll;
    let (message, offset, _) =
        transcript_window(&app.transcript_height_prefix, u32::from(top), 1, 3);
    assert_eq!((message, offset), (1, usize::from(intra)));
}

#[test]
#[ignore = "quiet optimized host input/frame qualification; every sample is retained"]
fn oversized_transcript_worst_input_frame_budget() {
    // The existing cockpit input budget is 50 ms. Include one draw that may
    // already own the UI thread when the key arrives, actual key dispatch, and
    // the following transcript paint. Do not select a best sample or average.
    let budget = Duration::from_millis(50);
    for scenario in [
        "cold",
        "append",
        "resize",
        "streaming-single-line",
        "selected-resize",
    ] {
        let mut app = app();
        let mut terminal = Terminal::new(TestBackend::new(83, 24)).unwrap();
        if scenario == "streaming-single-line" {
            app.thinking = Some(Thinking::pending_for_test("practice"));
            app.partial = "streaming 界 👩‍🔬 e\u{301} ".repeat(15_000);
        } else {
            app.messages.push(giant("BUDGET"));
        }
        if matches!(scenario, "append" | "resize" | "selected-resize") {
            settle(&mut app, &mut terminal);
        }
        if matches!(scenario, "append" | "selected-resize") {
            app.messages.push(giant("APPENDED"));
        }
        if matches!(scenario, "resize" | "selected-resize") {
            terminal.backend_mut().resize(27, 24);
            terminal.autoresize().unwrap();
        }
        let mut ordered_us = Vec::new();
        for _ in 0..100 {
            if scenario == "selected-resize" {
                app.selection = Some(mouse::Selection::new(
                    mouse::PaneId::Transcript,
                    Rect::new(1, 1, 24, 22),
                    1,
                    1,
                ));
            }
            let started = Instant::now();
            draw(&mut app, &mut terminal);
            app.on_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
            draw(&mut app, &mut terminal);
            ordered_us.push(started.elapsed().as_micros() as u64);
            std::thread::sleep(Duration::from_millis(3));
        }
        assert_eq!(app.input, "x".repeat(100), "input was lost in {scenario}");
        let worst = ordered_us.iter().copied().max().unwrap();
        eprintln!(
            "OVERSIZED_INPUT_FRAME {}",
            serde_json::json!({
                "scenario": scenario, "ordered_us": ordered_us, "worst_us": worst,
                "budget_us": budget.as_micros(),
                "scope": "production transcript draw + actual on_key + next transcript draw; excludes OS event polling and terminal transport"
            })
        );
        assert!(
            u128::from(worst) < budget.as_micros(),
            "{scenario} worst {worst} us exceeds {budget:?}"
        );
    }
}
