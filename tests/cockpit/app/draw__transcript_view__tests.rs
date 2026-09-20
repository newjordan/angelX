use super::*;

#[test]
fn delegate_status_renders_before_delegate_completion_without_child_process() {
    use std::sync::{Arc, atomic::AtomicBool};
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(Viewer::static_preview());
    app.thinking = Some(crate::turn::Thinking::pending_for_test("test"));
    app.tool_strip.call_event(
        crate::harness::ToolEventId("delegate".into()),
        "delegate",
        "test",
    );
    let cancel = Arc::clone(&app.thinking.as_ref().unwrap().cancel);
    let nested = AtomicBool::new(false);
    let _link = crate::harness::link_child_owner(&nested, &cancel);
    crate::harness::observe_delegate_turn(&nested, "test", |events| {
        events
            .send(crate::harness::TurnEvent::ToolCall {
                id: crate::harness::ToolEventId("read".into()),
                name: "shell".into(),
                args_summary: "private arguments".into(),
            })
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if crate::harness::owned_delegate_snapshot(Arc::as_ptr(&cancel) as usize)
                .is_some_and(|state| state.calls == 1)
            {
                break;
            }
            assert!(Instant::now() < deadline, "missing live delegate update");
            std::thread::yield_now();
        }
        let mut terminal = Terminal::new(TestBackend::new(110, 2)).unwrap();
        terminal
            .draw(|frame| render_tool_strip(frame, &app, frame.area()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let text: String = (0..110)
            .filter_map(|x| buffer.cell((x, 0)).map(|c| c.symbol()))
            .collect();
        assert!(text.contains("delegate · tool shell"), "{text}");
        assert!(text.contains("#1 · update 0s ago"), "{text}");
        assert!(
            !text.contains("awaiting agents") && !text.contains("private arguments"),
            "{text}"
        );
    });
    assert!(crate::harness::owned_delegate_snapshot(Arc::as_ptr(&cancel) as usize).is_none());
}

#[test]
fn worker_status_renders_live_delegated_process_and_releases_after_cancel() {
    use std::sync::{Arc, atomic::Ordering};
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(Viewer::static_preview());
    app.thinking = Some(crate::turn::Thinking::pending_for_test("test"));
    app.tool_strip.call_event(
        crate::harness::ToolEventId("live-worker".into()),
        "delegate",
        "local test",
    );
    let cancel = Arc::clone(&app.thinking.as_ref().unwrap().cancel);
    struct StopOnDrop(Arc<std::sync::atomic::AtomicBool>);
    impl Drop for StopOnDrop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    let stop = StopOnDrop(Arc::clone(&cancel));
    let worker_cancel = Arc::clone(&cancel);
    let worker = std::thread::spawn(move || {
        crate::harness::run_sandboxed_observed_cancellable(
            "bash",
            &[
                "--noprofile",
                "--norc",
                "-c",
                "printf 'worker ready\\n'; while :; do :; done",
            ],
            None,
            &crate::sandbox::SandboxPolicy::permissive(),
            Some(&worker_cancel),
        )
    });
    let owner = Arc::as_ptr(&cancel) as usize;
    let deadline = Instant::now() + Duration::from_secs(10);
    let observed = loop {
        if let Some(child) = crate::harness::owned_child_snapshot(owner)
            && child.cpu_age_secs.is_some()
            && child.output_age_secs.is_some()
        {
            break true;
        }
        if Instant::now() >= deadline {
            break false;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut terminal = Terminal::new(TestBackend::new(100, 2)).unwrap();
    terminal
        .draw(|frame| render_tool_strip(frame, &app, frame.area()))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let text: String = (0..100)
        .filter_map(|x| buffer.cell((x, 0)).map(|cell| cell.symbol()))
        .collect();
    drop(stop);
    let outcome = worker.join().unwrap().unwrap();
    assert!(outcome.cancelled);
    assert!(
        observed,
        "real process CPU/output did not reach the UI snapshot"
    );
    assert!(text.contains("CPU active"), "{text}");
    assert!(!text.contains("awaiting agents"), "{text}");
    assert!(crate::harness::owned_child_snapshot(owner).is_none());
}
use ratatui::{Terminal, backend::TestBackend};

fn assert_verdict_cells(
    outcome: Option<harness::ToolOutcome>,
    verdict: &str,
    expected: ratatui::style::Color,
) {
    let mut app = App::preview(Viewer::static_preview());
    let id = harness::ToolEventId("verified-tests".to_string());
    app.tool_strip
        .call_event(id.clone(), "run_tests", "--no-default-features");
    if let Some(outcome) = outcome {
        app.tool_strip
            .result_event(&id, "run_tests", "tests: 148 passed, 0 failed", outcome);
    }

    let backend = TestBackend::new(100, 2);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render_tool_strip(frame, &app, frame.area()))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let verdict_width = verdict.chars().count() as u16;
    let x = (0..buffer
        .area
        .width
        .saturating_sub(verdict_width.saturating_sub(1)))
        .find(|x| {
            (*x..*x + verdict_width)
                .filter_map(|cell_x| buffer.cell((cell_x, 0)).map(|cell| cell.symbol()))
                .collect::<String>()
                == verdict
        })
        .unwrap_or_else(|| panic!("explicit verifier {verdict} rail"));
    for cell_x in x..x + verdict_width {
        assert_eq!(
            buffer.cell((cell_x, 0)).expect("verdict cell").fg,
            expected,
            "{verdict} cell at x={cell_x} must use its semantic color"
        );
    }
}

#[test]
fn verifier_verdicts_use_semantic_palette_cells() {
    assert_verdict_cells(None, "RUNNING", hud::HUD_PHOSPHOR);
    assert_verdict_cells(
        Some(harness::ToolOutcome {
            execution: harness::ExecutionOutcome::Succeeded,
            verification: harness::VerificationOutcome::Passed,
        }),
        "PASS",
        hud::HUD_VERIFIED,
    );
    assert_verdict_cells(
        Some(harness::ToolOutcome {
            execution: harness::ExecutionOutcome::Failed,
            verification: harness::VerificationOutcome::NotApplicable,
        }),
        "FAIL",
        hud::HUD_DANGER,
    );
    assert_verdict_cells(
        Some(harness::ToolOutcome {
            execution: harness::ExecutionOutcome::Succeeded,
            verification: harness::VerificationOutcome::Inconclusive,
        }),
        "INCONCLUSIVE",
        hud::HUD_GOLD,
    );
}

#[test]
fn wide_unicode_tool_status_preserves_the_right_rail_in_terminal_cells() {
    let mut app = App::preview(Viewer::static_preview());
    app.tool_strip.call_event(
        harness::ToolEventId("wide-path".to_string()),
        "read_file",
        "資料/設計🧪/実装.md",
    );

    let backend = TestBackend::new(40, 2);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render_tool_strip(frame, &app, frame.area()))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let status = (0..40)
        .filter_map(|x| buffer.cell((x, 0)).map(|cell| cell.symbol()))
        .collect::<String>();
    assert!(
        status.contains("#1"),
        "wide operation must yield cells to the call rail: {status:?}"
    );
}

fn strip_note_row(terminal: &Terminal<TestBackend>, width: u16) -> String {
    let buffer = terminal.backend().buffer();
    (0..width)
        .filter_map(|x| buffer.cell((x, 1)).map(|cell| cell.symbol()))
        .collect()
}

#[test]
fn default_core_composer_animates_and_explicit_reading_focus_pauses() {
    let mut app = App::preview(Viewer::static_preview());
    app.visual_motion = MotionMode::Full;
    assert_eq!(
        app.module_host.focused().map(|id| id.as_str()),
        Some("core")
    );
    app.tool_strip
        .note_event("recall: abcdefghijklmnopqrstuvwxyz");
    let mut terminal = Terminal::new(TestBackend::new(20, 3)).unwrap();
    let now = std::time::Instant::now();
    let mut draw = |app: &App, ms| {
        terminal
            .draw(|frame| {
                render_tool_strip_at(
                    frame,
                    app,
                    frame.area(),
                    now + std::time::Duration::from_millis(ms),
                )
            })
            .unwrap();
        strip_note_row(&terminal, 20)
    };
    let initial = draw(&app, 0);
    let mut moving = initial.clone();
    for ms in (160..=1_280).step_by(160) {
        moving = draw(&app, ms);
    }
    assert_ne!(initial, moving, "default Core focus includes the composer");
    assert!(app.handle_module_focus_key(ratatui::crossterm::event::KeyCode::F(2)));
    let held = draw(&app, 1_440);
    for ms in (1_600..=3_200).step_by(160) {
        assert_eq!(held, draw(&app, ms));
    }
    app.focus_pane_module(crate::mouse::PaneId::Input);
    for ms in (3_360..=3_840).step_by(160) {
        moving = draw(&app, ms);
    }
    assert_ne!(held, moving, "returning to the composer resumes the note");
    app.focus_pane_module(crate::mouse::PaneId::Transcript);
    let held = draw(&app, 4_000);
    assert_eq!(held, draw(&app, 4_160));
}

#[test]
fn unfocused_terminal_keeps_activity_motion_running() {
    let mut app = App::preview(Viewer::static_preview());
    app.visual_motion = MotionMode::Full;
    app.terminal_focused = false;
    app.tool_strip
        .note_event("recall: abcdefghijklmnopqrstuvwxyz");
    let mut terminal = Terminal::new(TestBackend::new(20, 3)).unwrap();
    let now = std::time::Instant::now();
    let mut draw = |ms| {
        terminal
            .draw(|frame| {
                render_tool_strip_at(
                    frame,
                    &app,
                    frame.area(),
                    now + std::time::Duration::from_millis(ms),
                )
            })
            .unwrap();
        strip_note_row(&terminal, 20)
    };
    let initial = draw(0);
    let mut moving = initial.clone();
    for ms in (160..=1_280).step_by(160) {
        moving = draw(ms);
    }
    assert_ne!(
        initial, moving,
        "focus loss must not freeze activity motion"
    );
}

#[test]
fn composer_selection_pauses_strip_motion_but_keeps_activity_visible() {
    let mut app = App::preview(Viewer::static_preview());
    app.visual_motion = MotionMode::Full;
    app.module_host
        .focus(&crate::runtime::ModuleId::new("artifacts"))
        .unwrap();
    app.input = "draft text".to_string();
    app.cursor = 5;
    app.tool_strip
        .note_event("recall: abcdefghijklmnopqrstuvwxyz");
    let mut terminal = Terminal::new(TestBackend::new(20, 3)).unwrap();
    let now = std::time::Instant::now();
    let draw_at = |terminal: &mut Terminal<TestBackend>, app: &App, ms| {
        terminal
            .draw(|frame| {
                render_tool_strip_at(
                    frame,
                    app,
                    frame.area(),
                    now + std::time::Duration::from_millis(ms),
                );
            })
            .unwrap();
    };
    draw_at(&mut terminal, &app, 0);
    let initial = strip_note_row(&terminal, 20);
    for ms in (160..=1_280).step_by(160) {
        draw_at(&mut terminal, &app, ms);
    }
    let moving = strip_note_row(&terminal, 20);
    assert_ne!(initial, moving, "the unselected note must actually roll");

    app.composer_selection_anchor = Some(0);
    assert!(app.selection.is_none(), "composer selection is separate");
    draw_at(&mut terminal, &app, 1_440);
    let held = terminal.backend().buffer().clone();
    for ms in (1_600..=3_200).step_by(160) {
        draw_at(&mut terminal, &app, ms);
        assert_eq!(
            &held,
            terminal.backend().buffer(),
            "selection must freeze decorative note and bar phases"
        );
    }

    app.tool_strip.note_event("context compacted");
    draw_at(&mut terminal, &app, 3_360);
    let status: String = (0..20)
        .filter_map(|x| {
            terminal
                .backend()
                .buffer()
                .cell((x, 0))
                .map(|cell| cell.symbol())
        })
        .collect();
    assert!(status.contains("context compacted"), "{status}");
    app.tool_strip
        .note_event("recall: abcdefghijklmnopqrstuvwxyz");
    draw_at(&mut terminal, &app, 3_520);
    let selected = strip_note_row(&terminal, 20);
    app.composer_selection_anchor = None;
    for ms in (3_680..=5_280).step_by(160) {
        draw_at(&mut terminal, &app, ms);
    }
    assert_ne!(selected, strip_note_row(&terminal, 20));
}

#[test]
fn mixed_strip_notes_keep_distinct_prefixes_visible() {
    let mut app = App::preview(Viewer::static_preview());
    app.tool_strip.call_event(
        harness::ToolEventId("mix-shell".to_string()),
        "shell",
        "cmd=cargo test",
    );
    app.tool_strip
        .note_event("trimmed 3 recent tool result(s) to fit the active context window");
    app.tool_strip
        .note_event("storm: suppressed duplicate shell call (x3)");
    app.tool_strip.note_event("context compacted");

    assert_eq!(
        strip_note_row_text(
            "context compacted",
            3,
            &["trimmed".into(), "storm".into(), "context".into()],
        ),
        "\u{00b7} trimmed, storm · context compacted  (3 notes)"
    );

    let mut terminal = Terminal::new(TestBackend::new(100, 3)).unwrap();
    terminal
        .draw(|frame| render_tool_strip(frame, &app, frame.area()))
        .unwrap();
    let row = strip_note_row(&terminal, 100);
    assert!(row.contains("context compacted"), "{row:?}");
    assert!(
        row.contains("trimmed") && row.contains("storm"),
        "mixed strip notes must not collapse to only the latest text: {row:?}"
    );
    assert!(row.contains("3 notes"), "{row:?}");
}

#[test]
fn repeated_strip_notes_still_collapse_to_a_count() {
    let mut app = App::preview(Viewer::static_preview());
    app.tool_strip.call_event(
        harness::ToolEventId("storm-shell".to_string()),
        "shell",
        "cmd=cargo test",
    );
    app.tool_strip
        .note_event("storm: suppressed duplicate shell call (x2)");
    app.tool_strip
        .note_event("storm: suppressed duplicate grep call (x3)");

    assert_eq!(
        strip_note_row_text(
            "storm: suppressed duplicate grep call (x3)",
            2,
            &["storm".into()],
        ),
        "\u{00b7} storm: suppressed duplicate grep call (x3)  (2 notes)"
    );

    let mut terminal = Terminal::new(TestBackend::new(100, 3)).unwrap();
    terminal
        .draw(|frame| render_tool_strip(frame, &app, frame.area()))
        .unwrap();
    let row = strip_note_row(&terminal, 100);
    assert!(
        row.contains("storm: suppressed duplicate grep call (x3)"),
        "{row:?}"
    );
    assert!(row.contains("(2 notes)"), "{row:?}");
    assert!(
        !row.contains("trimmed"),
        "same-family repeats must not invent other prefixes: {row:?}"
    );
}

#[test]
fn stalled_stream_reports_silence_and_watchdog_with_separate_ambience() {
    let _env = crate::tests::env_lock();
    let _pulse = crate::tests::TestEnvGuard::set("ANGEL_STALL_PULSE_SECS", "20");
    let _deadline = crate::tests::TestEnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "600");
    let mut app = App::preview(Viewer::static_preview());
    app.tool_strip.call_event(
        harness::ToolEventId("stalled-shell".to_string()),
        "shell",
        "cmd=cargo build",
    );
    app.tool_strip.result_event(
        &harness::ToolEventId("stalled-shell".to_string()),
        "shell",
        "build complete",
        harness::ToolOutcome {
            execution: harness::ExecutionOutcome::Succeeded,
            verification: harness::VerificationOutcome::NotApplicable,
        },
    );
    app.thinking = Some(Thinking::pending_for_test("practice"));

    let mut terminal = Terminal::new(TestBackend::new(80, 2)).unwrap();
    let row_text = |terminal: &Terminal<TestBackend>, y: u16| -> String {
        let buffer = terminal.backend().buffer();
        (0..80)
            .filter_map(|x| buffer.cell((x, y)).map(|cell| cell.symbol()))
            .collect()
    };
    let fragment_color = |terminal: &Terminal<TestBackend>, fragment: &str| {
        let buffer = terminal.backend().buffer();
        let width = fragment.chars().count() as u16;
        let x = (0..80 - width)
            .find(|x| {
                (*x..*x + width)
                    .filter_map(|cell_x| buffer.cell((cell_x, 0)).map(|cell| cell.symbol()))
                    .collect::<String>()
                    == fragment
            })
            .unwrap_or_else(|| panic!("{fragment} on the status row"));
        buffer.cell((x, 0)).expect("fragment cell").fg
    };

    // Fresh stream: no readout, and the lane keeps its motion style.
    terminal
        .draw(|frame| render_tool_strip(frame, &app, frame.area()))
        .unwrap();
    assert!(!row_text(&terminal, 0).contains("silent"));

    // 25s silent: gold silence fragment + stationary warning marker.
    let stalled_at = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(25))
        .expect("monotonic clock has 25s of history");
    app.thinking.as_mut().expect("in flight").last_stream_at = stalled_at;
    terminal
        .draw(|frame| render_tool_strip(frame, &app, frame.area()))
        .unwrap();
    let status = row_text(&terminal, 0);
    assert!(status.contains("silent 25s"), "{status:?}");
    assert_eq!(fragment_color(&terminal, "silent"), hud::HUD_GOLD);
    let early = row_text(&terminal, 1);
    assert!(early.starts_with("! "), "{early}");
    assert_eq!(
        terminal.backend().buffer().cell((2, 1)).unwrap().fg,
        ratatui::style::Color::Rgb(58, 85, 94)
    );
    let now = std::time::Instant::now();
    terminal
        .draw(|frame| {
            render_tool_strip_at(
                frame,
                &app,
                frame.area(),
                now + std::time::Duration::from_secs(1),
            )
        })
        .unwrap();
    assert_ne!(
        early,
        row_text(&terminal, 1),
        "ambience must survive a provider stall"
    );
    assert!(row_text(&terminal, 1).starts_with("! "));

    // 301s of the 600s deadline: danger watchdog countdown.
    let deep_stall = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(301))
        .expect("monotonic clock has 301s of history");
    app.thinking.as_mut().expect("in flight").last_stream_at = deep_stall;
    terminal
        .draw(|frame| render_tool_strip(frame, &app, frame.area()))
        .unwrap();
    let status = row_text(&terminal, 0);
    assert!(status.contains("watchdog 299s"), "{status:?}");
    assert_eq!(fragment_color(&terminal, "watchdog"), hud::HUD_DANGER);
}

#[test]
fn ambient_bar_survives_empty_idle_and_completed_turns() {
    let _env = crate::tests::env_lock();
    let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = crate::tests::TestEnvGuard::unset("ANGEL_TURBO");
    let mut app = App::preview(Viewer::static_preview());
    app.visual_motion = MotionMode::Full;
    app.messages.clear();
    app.partial.clear();
    assert_eq!(tool_strip_height(&app, 10), 1);
    let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
    terminal
        .draw(|frame| render_transcript(frame, &mut app, frame.area()))
        .unwrap();
    let row_text = |t: &Terminal<TestBackend>, y| {
        (0..80)
            .filter_map(|x| t.backend().buffer().cell((x, y)).map(|c| c.symbol()))
            .collect::<String>()
    };
    assert!(
        row_text(&terminal, 8)
            .chars()
            .any(|c| ('\u{2801}'..='\u{28ff}').contains(&c)),
        "cold empty transcript must paint the ambient bar"
    );
    let now = std::time::Instant::now();
    let area = Rect::new(1, 8, 78, 1);
    terminal
        .draw(|frame| render_tool_strip_at(frame, &app, area, now))
        .unwrap();
    let early = row_text(&terminal, 8);
    terminal
        .draw(|frame| {
            render_tool_strip_at(frame, &app, area, now + std::time::Duration::from_secs(1))
        })
        .unwrap();
    assert_ne!(early, row_text(&terminal, 8));
    let id = harness::ToolEventId("ambient-result".into());
    app.tool_strip
        .call_event(id.clone(), "read_file", "README.md");
    app.tool_strip.result_event(
        &id,
        "read_file",
        "ok",
        harness::ToolOutcome {
            execution: harness::ExecutionOutcome::Succeeded,
            verification: harness::VerificationOutcome::NotApplicable,
        },
    );
    app.thinking = Some(Thinking::pending_for_test("practice"));
    let before = app.tool_strip.snapshot();
    let area = Rect::new(0, 0, 80, 2);
    terminal
        .draw(|frame| {
            render_tool_strip_at(frame, &app, area, now + std::time::Duration::from_secs(2))
        })
        .unwrap();
    let settled = row_text(&terminal, 1);
    assert!(settled.starts_with("✓ "));
    terminal
        .draw(|frame| {
            render_tool_strip_at(frame, &app, area, now + std::time::Duration::from_secs(3))
        })
        .unwrap();
    assert_ne!(settled, row_text(&terminal, 1));
    assert!(row_text(&terminal, 1).starts_with("✓ "));
    assert_eq!(
        before,
        app.tool_strip.snapshot(),
        "display must never mutate RL evidence"
    );
    app.thinking = None;
    app.tool_strip.take_summary();
    assert_eq!(tool_strip_height(&app, 10), 1);
    app.visual_motion = MotionMode::Off;
    terminal
        .draw(|frame| render_tool_strip_at(frame, &app, area, now))
        .unwrap();
    let off = row_text(&terminal, 1);
    terminal
        .draw(|frame| {
            render_tool_strip_at(frame, &app, area, now + std::time::Duration::from_secs(30))
        })
        .unwrap();
    assert_eq!(off, row_text(&terminal, 1));
    crate::comp_mode::set(true);
    assert_eq!(tool_strip_height(&app, 10), 0);
    crate::comp_mode::set(false);
    app.transcript_mode = crate::app::TranscriptMode::Trace;
    assert_eq!(tool_strip_height(&app, 10), 0);
}

#[test]
fn transcript_layout_fills_body_and_reserves_an_edge_rail() {
    let body = Rect::new(4, 3, 180, 20);
    let (text, rail) = transcript_text_and_rail(body);
    assert_eq!(text, Rect::new(4, 3, 179, 20));
    assert_eq!(rail, Some(Rect::new(183, 3, 1, 20)));

    let tiny = Rect::new(0, 0, 1, 4);
    assert_eq!(transcript_text_and_rail(tiny), (tiny, None));
}

#[test]
fn newly_arrived_text_is_visible_without_waiting_for_motion() {
    let mut app = App::preview(Viewer::static_preview());
    app.visual_motion = MotionMode::Full;
    app.messages = vec![
        Message {
            role: Role::User,
            text: "echo-now-界-e\u{301}".into(),
        },
        Message {
            role: Role::Angel,
            text: "answer-now-🦀".into(),
        },
    ];
    app.transcript_heights = vec![2, 2];
    for now in [0.2, 3_600.0, 86_400.0] {
        app.transcript_spawns.clear();
        sync_transcript_spawns(&mut app, now);
        assert!(now - app.transcript_spawns[0] >= ROLL_IN_SECS);
    }
    app.transcript_spawns = vec![10.0, 10.0];
    let mut terminal = Terminal::new(TestBackend::new(60, 8)).unwrap();
    terminal
        .draw(|frame| render_rolling_blocks(frame, &mut app, frame.area(), 0, 2, 0, 10.0, false))
        .unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(text.contains("echo-now-界"), "{text}");
    assert!(text.contains("answer-now-🦀"), "{text}");
}

#[test]
fn chat_role_edges_and_unicode_copy_match_the_live_transcript_rectangle() {
    let _guard = crate::tests::env_lock();
    for width in [32, 180] {
        let mut app = App::preview(Viewer::static_preview());
        app.messages = vec![
            Message::new(Role::User, "界e\u{301}👩‍💻"),
            Message::new(Role::Angel, "answer"),
        ];
        app.settle_transcript_spawns();
        let area = Rect::new(0, 0, width, 10);
        let (body, rail) =
            transcript_text_and_rail(hud_block(crate::views::status_view::agent_shell_title()).inner(area));
        let mut terminal = Terminal::new(TestBackend::new(width, 10)).unwrap();
        terminal
            .draw(|frame| render_transcript(frame, &mut app, area))
            .unwrap();
        let buf = terminal.backend().buffer();
        let user_x = body.right() - 5;
        let user_y = (body.y..body.bottom())
            .find(|&y| buf[(user_x, y)].symbol() == "界")
            .expect("operator body is right-aligned within the shared reading measure");
        assert_eq!(buf[(user_x, user_y)].fg, hud::HUD_PHOSPHOR);
        assert_eq!(buf[(body.x, user_y + 2)].symbol(), "a");
        assert_eq!(buf[(body.x, user_y + 2)].fg, hud::HUD_GOLD);
        let mut selection = mouse::Selection::new(mouse::PaneId::Transcript, body, user_x, user_y);
        selection.extend(body.right() - 1, user_y);
        assert_eq!(
            mouse::extract_text_in(buf, &selection, body),
            "界e\u{301}👩‍💻"
        );
        assert_eq!(rail.unwrap().x, body.right());
        assert_ne!(buf[(rail.unwrap().x, user_y)].fg, hud::HUD_PHOSPHOR);
    }
}

#[test]
fn transcript_scrollbar_is_inside_the_border_and_reaches_both_ends() {
    let mut app = App::preview(Viewer::static_preview());
    app.messages = (0..36)
        .map(|index| Message {
            role: Role::User,
            text: format!("scroll row {index:02}").into(),
        })
        .collect();
    app.settle_transcript_spawns();
    let area = Rect::new(0, 0, 32, 10);
    let mut inner = hud_block(crate::views::status_view::agent_shell_title()).inner(area);
    inner.height -= tool_strip_height(&app, inner.height);
    let (body, rail) = transcript_text_and_rail(inner);
    let rail = rail.expect("normal transcript has a scroll rail");

    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| render_transcript(frame, &mut app, area))
        .unwrap();
    assert_eq!(
        terminal
            .backend()
            .buffer()
            .cell((rail.x, body.y + body.height - 1))
            .unwrap()
            .symbol(),
        "●",
        "newest position reaches the rail bottom"
    );
    assert_eq!(
        terminal
            .backend()
            .buffer()
            .cell((area.width - 1, body.y + 2))
            .unwrap()
            .symbol(),
        "│",
        "scroll rail must not overwrite the panel border"
    );
    // Track cells between the thumb and the ends stay dotted, not solid bars.
    let mid = body.y + 1;
    if mid < body.y + body.height - 1 {
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((rail.x, mid))
                .unwrap()
                .symbol(),
            "·",
            "scroll track uses a dotted glyph"
        );
    }

    app.scroll = u16::MAX;
    terminal
        .draw(|frame| render_transcript(frame, &mut app, area))
        .unwrap();
    assert_eq!(
        terminal
            .backend()
            .buffer()
            .cell((rail.x, body.y))
            .unwrap()
            .symbol(),
        "●",
        "oldest position reaches the rail top"
    );
}

#[test]
fn scrolled_transcript_keeps_its_absolute_row_while_partial_grows() {
    let mut app = App::preview(Viewer::static_preview());
    app.messages = (0..40)
        .map(|index| Message {
            role: Role::Angel,
            text: format!("evidence row {index:02}").into(),
        })
        .collect();
    app.settle_transcript_spawns();
    let area = Rect::new(0, 0, 42, 12);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| render_transcript(frame, &mut app, area))
        .unwrap();
    app.scroll = 6;
    terminal
        .draw(|frame| render_transcript(frame, &mut app, area))
        .unwrap();
    let before_top = app.transcript_last_bottom - app.scroll;

    app.thinking = Some(Thinking::pending_for_test("practice"));
    app.partial = "live evidence ".repeat(30);
    terminal
        .draw(|frame| render_transcript(frame, &mut app, area))
        .unwrap();
    let after_top = app.transcript_last_bottom - app.scroll;

    assert_eq!(
        after_top, before_top,
        "live growth moved the reading anchor"
    );
    assert!(app.scroll > 6, "new rows are absorbed below the reader");
}

#[test]
fn live_partial_preempts_entry_motion_and_keeps_its_tail_visible() {
    let mut app = App::preview(Viewer::static_preview());
    app.messages.push(Message {
        role: Role::User,
        text: "fresh prompt".into(),
    });
    app.partial = format!("{}TAIL-MARKER", "streaming words ".repeat(80));
    app.thinking = Some(Thinking::pending_for_test("practice"));
    let area = Rect::new(0, 0, 36, 9);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();

    terminal
        .draw(|frame| render_transcript(frame, &mut app, area))
        .unwrap();
    let text = test_backend_text(terminal.backend());

    assert!(
        !app.transcript_rolling,
        "stream content must preempt motion"
    );
    assert!(
        text.contains("TAIL-MARKER"),
        "live tail was clipped:\n{text}"
    );
}

#[test]
fn oversized_streaming_markdown_renders_one_hundred_frames_under_budget() {
    let mut app = App::preview(Viewer::static_preview());
    app.thinking = Some(Thinking::pending_for_test("practice"));
    let chunk = format!(
        "## streamed section\n\n```rust\n{}\n```\n\n",
        "let value = **not actually emphasis**;\n".repeat(64)
    );
    let area = Rect::new(0, 0, 96, 30);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    let started = std::time::Instant::now();
    for _ in 0..100 {
        app.partial.push_str(&chunk);
        terminal
            .draw(|frame| render_transcript(frame, &mut app, area))
            .unwrap();
    }
    app.partial.push_str("\nFINAL-TAIL-MARKER");
    terminal
        .draw(|frame| render_transcript(frame, &mut app, area))
        .unwrap();
    let elapsed = started.elapsed();
    let text = test_backend_text(terminal.backend());
    assert!(app.partial.len() > 200_000, "fixture must cross the cap");
    assert!(
        elapsed < std::time::Duration::from_secs(3),
        "101 growing transcript frames took {elapsed:?}"
    );
    assert!(
        text.contains("FINAL-TAIL-MARKER"),
        "bounded window hid the live tail:\n{text}"
    );
}

#[test]
fn height_cache_repairs_a_shrink_before_resize_debounce() {
    let mut app = App::preview(Viewer::static_preview());
    app.messages = (0..10)
        .map(|index| Message {
            role: Role::User,
            text: format!("cached row {index}").into(),
        })
        .collect();
    sync_transcript_heights(&mut app, 40);
    assert_eq!(app.transcript_heights.len(), 10);
    assert_eq!(app.transcript_height_prefix.len(), 11);

    app.messages.truncate(2);
    app.last_resize_at = Instant::now();
    sync_transcript_heights(&mut app, 39);

    assert_eq!(app.transcript_heights.len(), 2);
    assert_eq!(app.transcript_height_prefix.len(), 3);
    assert_eq!(
        app.transcript_height_prefix.last().copied(),
        Some(app.transcript_heights.iter().map(|&h| u32::from(h)).sum())
    );
    assert_eq!(app.transcript_heights_w, 39);
}

#[test]
fn unfocused_terminal_reflows_after_resize_settles() {
    let mut app = App::preview(Viewer::static_preview());
    app.messages = (0..4)
        .map(|index| Message {
            role: Role::User,
            text: format!("a long cached row that wraps after resize {index}").into(),
        })
        .collect();
    sync_transcript_heights_budgeted(&mut app, 40, usize::MAX, || false);
    assert_eq!(app.transcript_heights_w, 40);

    app.terminal_focused = false;
    app.last_resize_at = Instant::now() - RESIZE_SETTLE;
    sync_transcript_heights_budgeted(&mut app, 20, usize::MAX, || false);

    assert_eq!(app.transcript_heights_w, 20);
    assert!(app.pending_transcript_reflow.is_none());
}

#[test]
fn prefix_height_index_finds_visible_windows_and_skips_zero_height_blocks() {
    let prefix = [0, 2, 5, 5, 9];

    assert_eq!(transcript_window(&prefix, 0, 2, 4), (0, 0, 1));
    assert_eq!(transcript_window(&prefix, 2, 3, 4), (1, 0, 2));
    assert_eq!(transcript_window(&prefix, 4, 4, 4), (1, 2, 4));
    assert_eq!(
        transcript_window(&prefix, 5, 2, 4),
        (3, 0, 4),
        "a zero-height block must not become the first visible message"
    );

    let large: Vec<u32> = (0..=100_000).map(|offset| offset * 2).collect();
    assert_eq!(
        transcript_window(&large, 190_001, 20, 100_000),
        (95_000, 1, 95_011)
    );
}

#[test]
fn prefix_window_is_exactly_equivalent_to_the_linear_reference() {
    for message_len in 0..80usize {
        let heights: Vec<u16> = (0..message_len)
            .map(|index| ((index * 17 + message_len * 3) % 9) as u16)
            .collect();
        let mut prefix = vec![0u32];
        for &height in &heights {
            prefix.push(
                prefix
                    .last()
                    .copied()
                    .unwrap()
                    .saturating_add(u32::from(height)),
            );
        }
        let total = prefix.last().copied().unwrap_or(0);
        for y in 0..=total {
            for viewport_height in 1..=12u32 {
                let mut cum = 0usize;
                let mut first = heights.len();
                for (index, &height) in heights.iter().enumerate() {
                    if cum + height as usize > y as usize {
                        first = index;
                        break;
                    }
                    cum += height as usize;
                }
                let intra = (y as usize).saturating_sub(cum);
                let need = intra + viewport_height as usize;
                let mut covered = 0usize;
                let mut end = first;
                while end < heights.len() && covered < need {
                    covered += heights[end] as usize;
                    end += 1;
                }
                assert_eq!(
                    transcript_window(&prefix, y, viewport_height, message_len),
                    (first, intra, end),
                    "len={message_len} y={y} viewport={viewport_height} heights={heights:?}"
                );
            }
        }
    }
}

#[test]
fn prefix_height_index_repairs_and_extends_incrementally() {
    let mut app = App::preview(Viewer::static_preview());
    app.messages = (0..4)
        .map(|index| Message {
            role: Role::User,
            text: format!("row {index}").into(),
        })
        .collect();
    app.transcript_height_prefix.clear();

    let initial_total = sync_transcript_heights(&mut app, 30);
    assert_eq!(app.transcript_height_prefix.len(), 5);
    assert_eq!(
        app.transcript_height_prefix.last().copied(),
        Some(initial_total)
    );

    let old_prefix = app.transcript_height_prefix.clone();
    app.messages.push(Message {
        role: Role::Angel,
        text: "one appended response".into(),
    });
    let appended_total = sync_transcript_heights(&mut app, 30);
    assert_eq!(&app.transcript_height_prefix[..5], old_prefix.as_slice());
    assert_eq!(app.transcript_height_prefix.len(), 6);
    assert!(appended_total > initial_total);
}

#[test]
fn rebuilt_same_length_history_invalidates_cached_heights_and_spawns() {
    let mut app = App::preview(Viewer::static_preview());
    app.messages = vec![
        Message {
            role: Role::User,
            text: "short".into(),
        },
        Message {
            role: Role::Angel,
            text: "short".into(),
        },
    ];
    sync_transcript_heights(&mut app, 24);
    app.transcript_spawns = vec![1.0, 2.0];
    let old_heights = app.transcript_heights.clone();

    app.history = vec![
        ChatMsg::user("a much taller replacement message ".repeat(12)),
        ChatMsg::assistant("another tall replacement answer ".repeat(12)),
    ];
    app.rebuild_display();

    assert!(app.transcript_heights.is_empty());
    assert_eq!(app.transcript_height_prefix, vec![0]);
    assert!(
        app.transcript_spawns
            .iter()
            .all(|spawn| *spawn == f32::NEG_INFINITY)
    );
    sync_transcript_heights(&mut app, 24);
    assert_ne!(app.transcript_heights, old_heights);
}
