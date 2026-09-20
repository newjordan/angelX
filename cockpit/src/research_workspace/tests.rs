use super::*;
use ratatui::{Terminal, backend::TestBackend};

fn passed() -> ToolOutcome {
    ToolOutcome {
        execution: ExecutionOutcome::Succeeded,
        verification: VerificationOutcome::Passed,
    }
}

fn workspace(count: usize) -> Workspace {
    let mut workspace = Workspace::default();
    workspace.ensure_session("fixture", &[]);
    for index in 0..count {
        let id = ToolEventId(format!("event-{index}"));
        workspace.start(&id, "cargo", &format!("test candidate-{index}"));
        workspace.result(&id, "cargo", &format!("candidate-{index} result"), passed());
    }
    workspace.project(Vec::new());
    workspace
}

#[test]
fn correlation_is_required_and_duplicates_never_change_a_verdict() {
    let mut workspace = workspace(1);
    let before = workspace.records.len();
    workspace.result(&ToolEventId("unknown".into()), "cargo", "passed", passed());
    workspace.result(
        &ToolEventId("event-0".into()),
        "cargo",
        "failed",
        ToolOutcome {
            execution: ExecutionOutcome::Failed,
            verification: VerificationOutcome::Failed,
        },
    );
    assert_eq!(workspace.records.len(), before);
    assert_eq!(workspace.records[0].state, State::Verified);
    assert_eq!(workspace.diagnostics, 2);
}

#[test]
fn execution_success_does_not_invent_a_verification_pass() {
    let mut workspace = Workspace::default();
    let id = ToolEventId("build".into());
    workspace.start(&id, "shell", "cargo build");
    workspace.result(
        &id,
        "shell",
        "all tests passed",
        ToolOutcome {
            execution: ExecutionOutcome::Succeeded,
            verification: VerificationOutcome::NotApplicable,
        },
    );
    assert_eq!(workspace.records[0].state, State::Succeeded);
}

#[test]
fn selection_survives_new_events_and_lens_changes_until_follow_is_enabled() {
    let mut workspace = workspace(30);
    workspace.move_selection(-20);
    let selected = workspace.selected.clone();
    for lens in Lens::ALL {
        workspace.action(Action::Lens(lens));
        workspace.start(&ToolEventId(format!("new-{lens:?}")), "check", "new work");
        workspace.project(Vec::new());
        assert_eq!(workspace.selected, selected);
    }
    workspace.action(Action::Follow);
    workspace.project(Vec::new());
    assert_ne!(workspace.selected, selected);
    assert_eq!(
        workspace.selected.as_ref(),
        workspace.visible.last().map(|entry| &entry.id)
    );
}

#[test]
fn history_is_bounded_and_session_changes_drop_previous_records() {
    let mut workspace = workspace(RECORD_CAP + 20);
    assert_eq!(workspace.records.len(), RECORD_CAP);
    assert_eq!(workspace.omitted, 20);
    workspace.ensure_session("different-project-session", &[]);
    assert!(workspace.records.is_empty());
    assert!(workspace.visible.is_empty());
    assert_eq!(workspace.omitted, 0);
}

#[test]
fn restored_tool_text_never_supplies_a_structured_verdict() {
    let history = vec![
        ChatMsg::assistant_calls(vec![crate::club::ToolCall {
            id: "restore".into(),
            name: "cargo".into(),
            args: serde_json::json!({"cmd":"test"}),
        }]),
        ChatMsg::tool("restore", "all tests passed"),
    ];
    let mut workspace = Workspace::default();
    workspace.ensure_session("restored", &history);
    assert_eq!(workspace.records.len(), 1);
    assert_eq!(workspace.records[0].state, State::Recorded);
    assert!(workspace.records[0].summary.contains("verdict unavailable"));
}

#[test]
fn large_restored_batches_keep_the_latest_bounded_window() {
    let calls = (0..RECORD_CAP + 20)
        .map(|index| crate::club::ToolCall {
            id: format!("restored-{index}"),
            name: "shell".into(),
            args: serde_json::Value::Null,
        })
        .collect();
    let history = vec![ChatMsg::assistant_calls(calls)];
    let mut workspace = Workspace::default();
    workspace.ensure_session("large-restore", &history);
    assert_eq!(workspace.records.len(), RECORD_CAP);
    assert_eq!(workspace.records.front().unwrap().id, "tool:restored-20");
    assert_eq!(
        workspace.records.back().unwrap().id,
        format!("tool:restored-{}", RECORD_CAP + 19)
    );
    assert_eq!(workspace.omitted, 20);
    assert!(
        workspace
            .records
            .iter()
            .all(|entry| entry.source == "saved conversation")
    );
}

#[test]
fn truncated_turns_leave_inconclusive_records() {
    let mut workspace = Workspace::default();
    workspace.start(&ToolEventId("cancelled".into()), "shell", "long test");
    workspace.finish_turn();
    assert_eq!(workspace.records[0].state, State::Inconclusive);
}

#[test]
fn production_renderer_scrolls_to_selection_and_drills_into_evidence() {
    let mut workspace = workspace(60);
    workspace.action(Action::Lens(Lens::Ledger));
    let mut terminal = Terminal::new(TestBackend::new(70, 16)).unwrap();
    terminal
        .draw(|frame| {
            view::render(frame, &mut workspace, frame.area(), "60 observed records");
        })
        .unwrap();
    assert!(workspace.offset > 0);
    workspace.move_selection(-59);
    terminal
        .draw(|frame| {
            view::render(frame, &mut workspace, frame.area(), "60 observed records");
        })
        .unwrap();
    assert!(workspace.offset <= 1);
    workspace.action(Action::Inspect);
    terminal
        .draw(|frame| {
            view::render(frame, &mut workspace, frame.area(), "60 observed records");
        })
        .unwrap();
    let screen = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(screen.contains("Evidence"));
    assert!(screen.contains("tool:event-0"));
    workspace.action(Action::Back);
    assert!(!workspace.inspecting);
    assert_eq!(workspace.selected.as_deref(), Some("tool:event-0"));
}

#[test]
fn flow_branches_have_real_counts_and_selectable_filters() {
    let mut workspace = workspace(4);
    workspace.start(&ToolEventId("active".into()), "read_file", "source.rs");
    workspace.project(Vec::new());
    workspace.action(Action::Lens(Lens::Flow));
    workspace.move_selection(-100);
    let mut terminal = Terminal::new(TestBackend::new(75, 24)).unwrap();
    let mut hits = Vec::new();
    terminal
        .draw(|frame| {
            hits = view::render(frame, &mut workspace, frame.area(), "5 observed records");
        })
        .unwrap();
    assert!(
        hits.iter()
            .any(|(_, action)| *action == Action::Filter(State::Verified))
    );
    workspace.action(Action::Filter(State::Verified));
    workspace.project(Vec::new());
    assert_eq!(workspace.visible.len(), 4);
    assert_eq!(workspace.lens, Lens::Ledger);
}

#[test]
fn unicode_and_small_viewports_are_cell_bounded() {
    use unicode_width::UnicodeWidthStr;
    for width in 0..32 {
        assert!(view::fit("資料/検証🧪/結果.md", width).width() <= width);
    }
    for (width, height) in [(1, 1), (12, 4), (30, 12), (110, 32)] {
        let mut workspace = workspace(8);
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| {
                let hits = view::render(frame, &mut workspace, frame.area(), "測定");
                for (rect, _) in hits {
                    assert_eq!(rect.intersection(frame.area()), rect);
                }
            })
            .unwrap();
    }
}

#[test]
fn inspector_keeps_evidence_when_the_bounded_live_window_moves_on() {
    let mut workspace = workspace(RECORD_CAP);
    workspace.move_selection(-1000);
    workspace.action(Action::Inspect);
    let inspected = workspace.selected_entry().unwrap().id.clone();
    workspace.start(&ToolEventId("overflow".into()), "read_file", "new.rs");
    workspace.project(Vec::new());
    assert_eq!(workspace.selected_entry().unwrap().id, inspected);
    assert!(workspace.inspecting);
}

#[test]
fn dependency_navigation_crosses_filters_and_back_restores_the_origin() {
    let mut workspace = Workspace::default();
    let input = Entry::new(
        "input".into(),
        Place::Council,
        "investigator",
        State::Succeeded,
        "",
        "investigator evidence",
        "agent graph episode",
    );
    let mut output = Entry::new(
        "output".into(),
        Place::Council,
        "implementer",
        State::Running,
        "",
        "implementation evidence",
        "agent graph episode",
    );
    output
        .links
        .push((input.id.clone(), "Dependency · investigator".into()));
    workspace.filter = Some(State::Running);
    workspace.project(vec![input, output]);
    workspace.action(Action::Inspect);
    workspace.action(Action::Related(0));
    assert_eq!(workspace.selected_entry().unwrap().id, "input");
    workspace.action(Action::Back);
    assert_eq!(workspace.selected_entry().unwrap().id, "output");
    assert!(workspace.inspecting);
    workspace.action(Action::Back);
    assert!(!workspace.inspecting);
}

#[test]
fn experiment_focus_keeps_the_story_connected_across_places_and_live_updates() {
    let make = |id: &str, experiment: &str, place: Place| {
        let mut entry = Entry::new(
            id.into(),
            place,
            id,
            State::Running,
            "",
            "",
            "experiment receipt journal",
        );
        entry.experiment_id = Some(experiment.into());
        entry
    };
    let worker = make("worker-a", "a", Place::Council);
    let mut result = make("result-a", "a", Place::Observatory);
    let other = make("worker-b", "b", Place::Council);
    result
        .links
        .push((other.id.clone(), "Dependency · parent run".into()));
    let entries = vec![worker, result, other];
    let mut workspace = Workspace::default();
    workspace.project(entries.clone());
    workspace.action(Action::Select(1));
    workspace.action(Action::FocusExperiment);
    workspace.project(entries.clone());
    assert_eq!(workspace.experiment_filter.as_deref(), Some("a"));
    assert_eq!(workspace.visible.len(), 2);
    assert_eq!(workspace.selected_entry().unwrap().id, "result-a");
    assert_eq!(workspace.active_places[Place::Council as usize], 1);
    workspace.action(Action::Inspect);
    workspace.action(Action::Related(0));
    assert_eq!(workspace.selected_entry().unwrap().id, "worker-b");
    workspace.project(entries.clone());
    workspace.action(Action::Back);
    assert_eq!(workspace.selected_entry().unwrap().id, "result-a");
    workspace.action(Action::Back);
    workspace.action(Action::Place(Place::Council));
    workspace.project(entries.clone());
    assert_eq!(workspace.visible.len(), 1);
    assert_eq!(workspace.selected_entry().unwrap().id, "worker-a");
    workspace.action(Action::ClearExperiment);
    workspace.project(entries);
    assert_eq!(workspace.visible.len(), 2);
    workspace.ensure_session("another-workspace", &[]);
    assert!(workspace.experiment_filter.is_none());
}

#[test]
fn ordinary_cockpit_dispatch_preserves_composer_and_expands_the_workspace() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let _guard = crate::tests::env_lock();
    let _backdrop = crate::tests::TestEnvGuard::unset("ANGEL_BACKDROP");
    let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = crate::tests::TestEnvGuard::unset("ANGEL_TURBO");
    crate::comp_mode::invalidate_cache();
    crate::surfaces::invalidate_backdrop_cache();
    let mut app = crate::App::preview(crate::Viewer::static_preview());
    app.open_research(None);
    let mut terminal = Terminal::new(TestBackend::new(144, 48)).unwrap();
    terminal
        .draw(|frame| crate::draw::ui(frame, &mut app))
        .unwrap();
    assert_eq!(
        app.scryglass.surface,
        crate::scryglass::StageSurface::Research
    );
    app.on_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
    assert_eq!(app.research.lens, Lens::Ledger);
    let mut entry = Entry::new(
        "experiment:a:worker".into(),
        Place::Council,
        "Worker",
        State::Running,
        "",
        "",
        "experiment receipt journal",
    );
    entry.experiment_id = Some("a".into());
    app.research.project(vec![entry]);
    app.on_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
    assert_eq!(app.research.experiment_filter.as_deref(), Some("a"));
    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.research.experiment_filter.is_none());
    app.on_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE));
    terminal
        .draw(|frame| crate::draw::ui(frame, &mut app))
        .unwrap();
    assert!(
        app.panes
            .rect_of(crate::mouse::PaneId::Artifacts)
            .unwrap()
            .width
            > 100
    );
    app.input = "/goal ".into();
    app.cursor = app.input.chars().count();
    app.on_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE));
    assert_eq!(app.input, "/goal 3");
    assert_eq!(app.research.lens, Lens::Ledger);
    assert!(
        app.history.is_empty(),
        "navigation never enters model history"
    );
}

#[test]
fn research_route_keeps_automatic_journeys_behind_the_working_surface() {
    let mut controller = crate::scryglass::StageController::default();
    controller.navigate(crate::scryglass::StageRoute::Research);
    controller.show_overlay(crate::scryglass::StageOverlay::Journey {
        call_id: ToolEventId("working".into()),
        destination: Building::Smithy,
    });
    assert_eq!(
        controller.resolved_scene(false, false, false),
        crate::scryglass::StageSurface::Research
    );
    assert!(controller.back());
    assert_eq!(controller.route(), crate::scryglass::StageRoute::Realm);
    controller.reset(crate::scryglass::StageRoute::Research);
    assert!(
        !controller.back(),
        "the research home returns focus to the composer without an extra Realm detour"
    );
    assert_eq!(controller.route(), crate::scryglass::StageRoute::Research);
}

#[test]
fn live_projection_keeps_the_selected_row_at_the_same_screen_position() {
    let mut workspace = workspace(60);
    workspace.action(Action::Lens(Lens::Ledger));
    workspace.move_selection(-20);
    let mut terminal = Terminal::new(TestBackend::new(100, 18)).unwrap();
    terminal
        .draw(|frame| {
            view::render(frame, &mut workspace, frame.area(), "");
        })
        .unwrap();
    let anchor = workspace.scroll_anchor.clone();
    let offset = workspace.offset;
    let extras = (0..5)
        .map(|index| {
            Entry::new(
                format!("finding:{index}"),
                Place::Library,
                "new finding",
                State::Recorded,
                "",
                "",
                "loop evidence ledger",
            )
        })
        .collect();
    workspace.project(extras);
    terminal
        .draw(|frame| {
            view::render(frame, &mut workspace, frame.area(), "");
        })
        .unwrap();
    assert_eq!(workspace.scroll_anchor, anchor);
    assert_eq!(workspace.offset, offset + 5);
}

#[test]
fn flow_exposes_declared_dependencies_without_inventing_causality() {
    let mut workspace = Workspace::default();
    let input = Entry::new(
        "input".into(),
        Place::Council,
        "scout",
        State::Succeeded,
        "",
        "",
        "agent graph episode",
    );
    let mut output = Entry::new(
        "output".into(),
        Place::Council,
        "builder",
        State::Running,
        "",
        "",
        "agent graph episode",
    );
    output
        .links
        .push((input.id.clone(), "Dependency · scout".into()));
    workspace.project(vec![input, output]);
    workspace.action(Action::Lens(Lens::Flow));
    let mut terminal = Terminal::new(TestBackend::new(90, 24)).unwrap();
    terminal
        .draw(|frame| {
            view::render(frame, &mut workspace, frame.area(), "");
        })
        .unwrap();
    let screen = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(screen.contains("declared dependencies"));
    assert!(screen.contains("scout ─→ builder"));
}

#[test]
fn hidden_pane_collects_real_events_and_escape_still_interrupts_work() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::sync::{atomic::AtomicBool, mpsc};
    use std::time::Instant;
    let _guard = crate::tests::env_lock();
    let mut app = crate::App::preview(crate::Viewer::static_preview());
    let (_result_tx, rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();
    event_tx
        .send(crate::harness::TurnEvent::ToolCall {
            id: ToolEventId("hidden".into()),
            name: "cargo".into(),
            args_summary: "test".into(),
        })
        .unwrap();
    event_tx
        .send(crate::harness::TurnEvent::ToolResult {
            id: ToolEventId("hidden".into()),
            name: "cargo".into(),
            summary: "passed".into(),
            outcome: passed(),
        })
        .unwrap();
    app.thinking = Some(crate::Thinking {
        started: Instant::now(),
        club_label: "practice".into(),
        club: None,
        spawn_usage: crate::turn::published_spawn_usage(None, crate::club::CacheUsage::default()),
        requested_route: crate::club::RouteIdentity {
            driver: "practice".into(),
            model: None,
            reasoning_effort: None,
        },
        cancel: Arc::new(AtomicBool::new(false)),
        rx,
        event_rx,
        last_stream_at: Instant::now(),
        idle_timeout_secs: Some(600),
        idle_warned_50: false,
        idle_warned_80: false,
        steer_idle_interrupt_secs: 60,
        steer_interrupt_fired: false,
        draining: false,
    });
    app.world_pane_visible = false;
    app.advance();
    assert_eq!(app.research.records.len(), 1);
    assert_eq!(app.research.records[0].state, State::Verified);
    app.open_research(None);
    app.scryglass.surface = crate::scryglass::StageSurface::Research;
    app.research.project(Vec::new());
    app.research.action(Action::Inspect);
    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(
        app.thinking
            .as_ref()
            .unwrap()
            .cancel
            .load(std::sync::atomic::Ordering::Relaxed),
        "global stop wins over inspector Back"
    );
}

#[test]
fn loop_evidence_from_a_different_workspace_is_not_projected() {
    let mut app = crate::App::preview(crate::Viewer::static_preview());
    app.loop_ctl.id = "foreign-loop".into();
    app.loop_ctl.workspace = Some(std::path::PathBuf::from("/tmp/foreign-research-workspace"));
    app.loop_ctl
        .findings
        .push("Foreign evidence must not appear".into());
    app.refresh_research();
    assert!(
        !app.research
            .visible
            .iter()
            .any(|entry| entry.id.starts_with("loop:foreign-loop"))
    );
}

#[test]
#[ignore = "manual frame-cost measurement; not a target-hardware benchmark"]
fn research_workspace_frame_cost() {
    let mut workspace = workspace(RECORD_CAP);
    let mut terminal = Terminal::new(TestBackend::new(110, 32)).unwrap();
    for lens in Lens::ALL {
        workspace.action(Action::Lens(lens));
        let started = std::time::Instant::now();
        for _ in 0..100 {
            terminal
                .draw(|frame| {
                    view::render(frame, &mut workspace, frame.area(), "512 observed records");
                })
                .unwrap();
        }
        eprintln!(
            "Research {:?}: {} µs/frame · 512 records · TestBackend",
            lens,
            started.elapsed().as_micros() / 100
        );
    }
}
