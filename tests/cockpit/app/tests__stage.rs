//! Stage/realm rendering suites (module-breakup: extracted from the
//! `tests.rs` monolith). Compact-header chrome, the realm pulse stage, the
//! reinforce control stage, cockpit scenario baselines, and the
//! fast-tick/settle contracts.

use super::{render_app_text, seed_preview_app, test_backend_text};
use crate::App;
use crate::ChatMsg;
use crate::Viewer;
use crate::agent::harness;
use crate::app::WorldButton;
use crate::drive::rl_ctl;
use crate::knowledge::session;
use crate::stage::world_viz;
use crate::tests::env_lock;
use crate::ui::draw;
use crate::ui::draw::ui;
use crate::ui::hud;
use crate::ui::mouse;
use crate::ui::panels;
use crate::ui::scryglass;
use crate::ui::surfaces;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, style::Modifier};
use std::time::{Duration, Instant};

fn show_work_wait(app: &mut App, width: u16, height: u16) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let screen = render_app_text(app, width, height);
        if app.scryglass.media_ready() {
            // A decoder can change to Fault at the end of a draw; paint the
            // visible diagnostic too before checking what the operator sees.
            return render_app_text(app, width, height);
        }
        assert!(
            Instant::now() < deadline,
            "Stage did not deliver its requested artifact\n{screen}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn show_work_report_tool_to_stage_scroll_resize_switch_and_dismiss() {
    use crate::agent::harness::Tool;
    let _lock = env_lock();
    let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = crate::tests::TestEnvGuard::unset("ANGEL_TURBO");
    crate::drive::comp_mode::invalidate_cache();
    let root = std::env::temp_dir().join(format!("angel-show-work-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let text = format!(
        "# Measured work\n{}\nFINAL RECEIPT: 173 trials passed\n",
        (0..100)
            .map(|n| format!("measured row {n}\n"))
            .collect::<String>()
    );
    std::fs::write(root.join("report.md"), text).unwrap();
    std::fs::write(root.join("other.json"), "{\"actual_result\":42}\n").unwrap();
    let mut app = seed_preview_app();
    app.tools = std::sync::Arc::new(harness::ToolRegistry::with_team(root.clone(), Vec::new()));
    let result = crate::agent::tools::utilities::PresentTool::new(&root)
        .call(&serde_json::json!({
            "kind": "report", "label": "Actual validation run", "url": "report.md"
        }))
        .unwrap();
    let (kind, label, target) = crate::ui::media::presentation_from_result(&result).unwrap();
    app.present_media(&kind, &label, &target);
    app.focus_module("artifacts");
    let screen = show_work_wait(&mut app, 120, 40);
    assert!(screen.contains("Measured work"), "{screen}");
    assert!(
        screen.contains("Actual validation run") && screen.contains("report.md"),
        "{screen}"
    );
    assert!(screen.contains("SHA256"), "{screen}");
    assert_eq!(app.scryglass.surface, scryglass::StageSurface::Document(0));
    assert!(app.scryglass.active_pinned());
    assert!(
        !app.viewer.has_image(),
        "text must work without an image protocol"
    );
    app.on_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
    let screen = render_app_text(&mut app, 120, 40);
    assert!(
        screen.contains("FINAL RECEIPT: 173 trials passed"),
        "{screen}"
    );
    let resized = render_app_text(&mut app, 60, 24);
    assert!(
        resized.contains("report.md"),
        "resize must preserve source identity\n{resized}"
    );
    app.input = format!("/show {}", root.join("other.json").display());
    app.submit();
    let other = show_work_wait(&mut app, 120, 40);
    assert!(other.contains("actual_result"), "{other}");
    app.on_key(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE));
    let first = show_work_wait(&mut app, 120, 40);
    assert!(
        first.contains("Measured work"),
        "browsing must include reports and reset scroll\n{first}"
    );
    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.scryglass.active_media(), None);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn show_work_missing_and_unsupported_artifacts_keep_the_requested_identity() {
    let _lock = env_lock();
    let _comp = crate::tests::TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::drive::comp_mode::invalidate_cache();
    let root = std::env::temp_dir().join(format!("angel-show-work-errors-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("unsupported.pdf"), b"%PDF-1.7\n").unwrap();
    let mut app = seed_preview_app();
    app.tools = std::sync::Arc::new(harness::ToolRegistry::with_team(root.clone(), Vec::new()));
    app.present_media(
        "resource",
        "Requested missing report",
        &root.join("missing.md").to_string_lossy(),
    );
    app.focus_module("artifacts");
    let missing = show_work_wait(&mut app, 120, 40);
    assert!(
        missing.contains("Cannot read requested file") && missing.contains("missing.md"),
        "{missing}"
    );
    assert_eq!(app.scryglass.surface, scryglass::StageSurface::Fault);
    app.input = format!("/show {}", root.join("unsupported.pdf").display());
    app.submit();
    let unsupported = show_work_wait(&mut app, 120, 40);
    assert!(
        unsupported.contains("Unsupported document format")
            && unsupported.contains("unsupported.pdf"),
        "{unsupported}"
    );
    assert!(!unsupported.contains("Loading requested document"));
    app.present_media(
        "link",
        "Upstream evidence",
        "https://example.test/actual-report",
    );
    let remote = show_work_wait(&mut app, 120, 40);
    assert!(
        remote.contains("no page contents fetched") && remote.contains("actual-report"),
        "{remote}"
    );
    std::fs::remove_dir_all(root).unwrap();
    drop(_comp);
    crate::drive::comp_mode::invalidate_cache();
}

#[test]
fn show_work_image_delivery_keeps_label_and_decodes_without_graphics_protocol() {
    let _lock = env_lock();
    let _protocol = crate::tests::TestEnvGuard::set("ANGEL_IMAGE_PROTOCOL", "halfblocks");
    let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::drive::comp_mode::invalidate_cache();
    let path =
        std::env::temp_dir().join(format!("angel-show-work-image-{}.png", std::process::id()));
    image::RgbImage::from_fn(64, 32, |x, y| {
        image::Rgb([((x * 4) % 255) as u8, ((y * 8) % 255) as u8, 128])
    })
    .save(&path)
    .unwrap();
    let mut app = seed_preview_app();
    app.input = "preserve the operator image draft λ".into();
    app.tools = std::sync::Arc::new(harness::ToolRegistry::with_team(
        path.parent().unwrap().to_path_buf(),
        Vec::new(),
    ));
    app.present_media("image", "Actual rendered output", &path.to_string_lossy());
    app.focus_module("artifacts");
    let screen = show_work_wait(&mut app, 120, 40);
    assert!(screen.contains("Actual rendered output"), "{screen}");
    assert_eq!(app.scryglass.surface, scryglass::StageSurface::Still(0));
    assert!(app.scryglass.active_error().is_none());
    assert!(!app.viewer.has_image());
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal.draw(|frame| ui(frame, &mut app)).unwrap();
    let gradient_pixels = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .filter(|cell| {
            matches!(cell.symbol(), "▀" | "▄")
                && matches!(cell.fg, ratatui::style::Color::Rgb(_, _, 128))
                && matches!(cell.bg, ratatui::style::Color::Rgb(_, _, 128))
        })
        .count();
    assert!(
        gradient_pixels >= 32,
        "expected the fixture's actual RGB gradient, not UI borders ({gradient_pixels} cells)"
    );
    assert_eq!(app.input, "preserve the operator image draft λ");
    std::fs::remove_file(path).unwrap();
}

#[cfg(unix)]
#[test]
fn show_work_captured_reply_rejects_replaced_output_directory() {
    use std::os::unix::fs::symlink;
    let _lock = env_lock();
    let root = std::env::temp_dir().join(format!("angel-capture-authority-{}", std::process::id()));
    let workspace = root.join("work");
    let outside = root.join("outside");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    let mut app = seed_preview_app();
    app.tools = std::sync::Arc::new(harness::ToolRegistry::with_team(
        workspace.clone(),
        Vec::new(),
    ));
    let first = app.display_reply(super::large_html_reply());
    assert!(first.contains("artifact captured"), "{first}");
    assert!(matches!(
        &app.media[0],
        crate::ui::media::Media::Confined { .. }
    ));
    std::fs::rename(
        workspace.join("angel_test_output"),
        workspace.join("original-output"),
    )
    .unwrap();
    symlink(&outside, workspace.join("angel_test_output")).unwrap();
    let failed = app.display_reply(super::large_html_reply());
    assert!(failed.contains("artifact capture failed"), "{failed}");
    assert_eq!(
        std::fs::read_dir(&outside).unwrap().count(),
        0,
        "automatic output must not escape through a replaced directory"
    );
    assert_eq!(
        app.media.len(),
        1,
        "a failed capture must never advertise another artifact"
    );
    assert!(
        app.media[0].source().unwrap().open().is_err(),
        "the earlier card cannot follow the replacement alias on later decode"
    );
    std::fs::remove_file(workspace.join("angel_test_output")).unwrap();
    // Even an in-workspace alias is refused before following its target.
    symlink(
        workspace.join("off-limits"),
        workspace.join("angel_test_output"),
    )
    .unwrap();
    assert!(
        app.display_reply(super::large_html_reply())
            .contains("artifact capture failed")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(not(feature = "scryglass-video"))]
#[test]
fn show_work_video_without_decoder_reports_unavailable() {
    let _lock = env_lock();
    let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::drive::comp_mode::invalidate_cache();
    let mut app = seed_preview_app();
    app.present_media("video", "Requested work recording", "/tmp/actual-work.mp4");
    app.focus_module("artifacts");
    let screen = show_work_wait(&mut app, 120, 40);
    assert!(
        screen.contains("video support is unavailable in this build")
            && screen.contains("actual-work.mp4"),
        "{screen}"
    );
    assert_eq!(app.scryglass.surface, scryglass::StageSurface::Fault);
}

#[test]
fn compact_session_warning_preempts_clip_prone_header_chrome() {
    let _lock = env_lock();
    let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = crate::tests::TestEnvGuard::unset("ANGEL_TURBO");
    let _backdrop = crate::tests::TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::drive::comp_mode::invalidate_cache();
    crate::ui::surfaces::invalidate_backdrop_cache();
    let mut app = App::preview(Viewer::static_preview());
    app.session = session::Session::at(std::env::temp_dir(), "compact-session-warning".to_string());
    assert!(
        app.session
            .save(&[ChatMsg::user("memory-only turn")])
            .is_err(),
        "an unbound sink deterministically enters degraded state"
    );
    app.statusline = Some("operator-status-".repeat(16));

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| ui(frame, &mut app)).unwrap();
    let screen = test_backend_text(terminal.backend());
    assert!(
        screen.contains("SESSION SAVE DEGRADED"),
        "the persistent safety warning must survive compact clipping\n{screen}"
    );
    let buffer = terminal.backend().buffer();
    let warning = "SESSION SAVE DEGRADED";
    // Compare terminal cells, not UTF-8 byte offsets after the box border.
    let (warning_y, warning_x) = (0..buffer.area.height)
        .find_map(|y| {
            (0..buffer.area.width.saturating_sub(warning.len() as u16))
                .find(|x| {
                    (*x..*x + warning.len() as u16)
                        .filter_map(|col| buffer.cell((col, y)).map(|cell| cell.symbol()))
                        .collect::<String>()
                        == warning
                })
                .map(|x| (y, x))
        })
        .expect("visible session warning cells");
    for x in warning_x..warning_x + warning.len() as u16 {
        let cell = buffer.cell((x, warning_y)).expect("session warning cell");
        assert_eq!(cell.fg, hud::HUD_DANGER, "danger cell at x={x}");
        assert!(
            cell.modifier.contains(Modifier::BOLD),
            "persistent warning cell at x={x} must be bold"
        );
    }
}

#[test]
fn compact_header_keeps_project_controls_when_operator_status_is_long() {
    let _lock = env_lock();
    let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = crate::tests::TestEnvGuard::unset("ANGEL_TURBO");
    let _backdrop = crate::tests::TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::drive::comp_mode::invalidate_cache();
    crate::ui::surfaces::invalidate_backdrop_cache();
    let mut app = App::preview(Viewer::static_preview());
    app.statusline = Some("operator-status-".repeat(16));
    app.world.note_tool_call_event(
        harness::ToolEventId("compact-header-pulse".to_string()),
        "exec_command",
        "cmd=cargo test, workdir=/tmp/project",
    );

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| ui(frame, &mut app)).unwrap();
    let buffer = terminal.backend().buffer();
    let row = (0..24)
        .map(|y| {
            (0..80)
                .filter_map(|x| buffer.cell((x, y)).map(|cell| cell.symbol()))
                .collect::<String>()
        })
        .find(|row| row.contains("angel0"))
        .expect("rendered compact header row");
    assert!(
        row.contains("MODEL") && row.contains("THINK") && row.contains("FORMATION"),
        "compact header must retain project controls without long status text\n{row}"
    );
    assert!(
        !row.contains("cmd="),
        "compact pulse must drop summary keys\n{row}"
    );
}

#[test]
fn comp_mode_skips_realm_pulse_paint_without_slowing_default() {
    use crate::tests::TestEnvGuard;
    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::drive::comp_mode::invalidate_cache();
    assert!(crate::drive::comp_mode::realm_pulse_paint_allowed());

    let mut app = seed_preview_app();
    app.world.note_tool_call_event(
        harness::ToolEventId("comp-pulse".to_string()),
        "exec_command",
        "cmd=cargo test, workdir=/tmp/project",
    );
    let standard = render_app_text(&mut app, 80, 24);
    assert!(
        !standard.contains("Smithy · cargo test · running") && standard.contains("MODEL"),
        "compact header keeps controls without redundant world telemetry\n{standard}"
    );

    let _on = TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::drive::comp_mode::invalidate_cache();
    assert!(!crate::drive::comp_mode::realm_pulse_paint_allowed());
    let mut armed = seed_preview_app();
    armed.world.note_tool_call_event(
        harness::ToolEventId("comp-pulse-lean".to_string()),
        "exec_command",
        "cmd=cargo test, workdir=/tmp/project",
    );
    let lean = render_app_text(&mut armed, 80, 24);
    assert!(
        !lean.contains("Smithy · cargo test · running"),
        "comp/lean must not paint the decorative pulse\n{lean}"
    );
}

#[test]
fn agent_summary_uses_structured_cell_bounded_realm_pulse() {
    let _lock = env_lock();
    let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = crate::tests::TestEnvGuard::unset("ANGEL_TURBO");
    let _backdrop = crate::tests::TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::drive::comp_mode::invalidate_cache();
    crate::ui::surfaces::invalidate_backdrop_cache();
    let mut app = App::preview(Viewer::static_preview());
    app.world.note_tool_call_event(
        harness::ToolEventId("agent-summary-pulse".to_string()),
        "functions.read_file",
        "path=資料/設計🧪/実装/検証/結果.md",
    );

    let backend = TestBackend::new(44, 17);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw::render_agent_bay(frame, &mut app, frame.area()))
        .unwrap();
    let text = test_backend_text(terminal.backend());
    assert!(
        !text.contains("Scriptorium") && !text.contains("path="),
        "trace pane omits decorative destination telemetry\n{text}"
    );
    assert!(
        !text.contains("path=") && !text.contains("資料"),
        "Agent summary must drop argument detail before causal state\n{text}"
    );
}

/// Deterministic render fixture that always exercises the real `ui()` path.
/// Scenario methods mutate only display state; the injected instant advances
/// Scryglass visibility without wall-clock sleeps.
struct CockpitScenario {
    app: App,
    width: u16,
    height: u16,
    now: Instant,
}

impl CockpitScenario {
    fn at(width: u16, height: u16) -> Self {
        Self {
            app: App::preview(Viewer::static_preview()),
            width,
            height,
            now: Instant::now(),
        }
    }

    fn render(&mut self) -> String {
        render_app_text(&mut self.app, self.width, self.height)
    }

    fn advance_visible(&mut self, duration: Duration) {
        self.now += duration;
        self.app.scryglass.tick_visible(self.now, false, false);
    }
}

#[test]
fn live_realm_stage_surfaces_degraded_memory_at_the_chapel() {
    use crate::tests::TestEnvGuard;

    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = TestEnvGuard::unset("ANGEL_TURBO");
    let _backdrop = TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::drive::comp_mode::invalidate_cache();
    crate::ui::surfaces::invalidate_backdrop_cache();
    let mut app = App::preview(Viewer::static_preview());
    app.world.note_tool_call_event(
        harness::ToolEventId("memory-health-stage".to_string()),
        "memory__recall",
        "project conventions",
    );
    app.world
        .note_memory_health(crate::knowledge::memory::store::MemoryHealth::Degraded);

    let screen = render_app_text(&mut app, 120, 40);
    assert!(
        app.world.memory_health() == crate::knowledge::memory::store::MemoryHealth::Degraded,
        "memory health remains explicit\n{screen}"
    );
    assert!(
        screen.contains("Memory degraded"),
        "live Realm Stage must expose Chapel backend health\n{screen}"
    );
}

#[test]
fn live_reinforce_stage_surfaces_measured_campaign_state_graph() {
    use crate::tests::TestEnvGuard;

    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = TestEnvGuard::unset("ANGEL_TURBO");
    let _backdrop = TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::drive::comp_mode::invalidate_cache();
    crate::ui::surfaces::invalidate_backdrop_cache();
    let mut app = App::preview(Viewer::static_preview());
    app.scryglass
        .navigate(crate::ui::scryglass::StageRoute::Reinforce);
    app.tools.rl().mode = rl_ctl::RlMode::Campaign;
    {
        let rl = app.tools.rl();
        let mut progress = rl.progress.lock().unwrap();
        progress.planned_attempts = 120;
        progress.replace_points(
            (1..=120)
                .map(|step| rl_ctl::RunPoint {
                    step,
                    reward: (step as f32 / 120.0).min(1.0),
                    latency_ms: 2_000 + step as u64,
                })
                .collect(),
        );
        progress.outcome = Some(Ok(rl_ctl::CampaignOutcome {
            attempted: 120,
            passed: 118,
            red: 2,
            rounds: 1,
            promoted_rounds: 1,
            policy_version: 1,
            decision: "promoted".to_string(),
            mean_delta: Some(0.5),
            validated: true,
            audit_supplied: true,
            release_sha256: Some("0".repeat(64)),
            solve_rate: Some(0.9),
            advantage_variance: Some(0.05),
            reflection: true,
            accepted_entry: Some("rl-policy".to_string()),
            accepted_event: Some("r1".to_string()),
            report_path: "runs/run_test".to_string(),
            route: crate::agent::club::RouteIdentity {
                driver: "fixture".to_string(),
                model: None,
                reasoning_effort: None,
            },
            wall_s: 62.0,
        }));
    }

    let screen = render_app_text(&mut app, 120, 40);
    assert!(
        screen.contains("Tiltyard"),
        "the RL stage must own the artifacts pane\n{screen}"
    );
    assert!(
        screen.contains("[attempt]") && screen.contains("[verify]"),
        "campaign state-graph nodes must render\n{screen}"
    );
    assert!(
        screen.contains("RELEASED · audited"),
        "the released outcome banner must render\n{screen}"
    );
}

#[test]
fn reinforce_stage_switches_between_branch_research_and_sankey_lenses() {
    use crate::tests::TestEnvGuard;

    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = TestEnvGuard::unset("ANGEL_TURBO");
    let _backdrop = TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::drive::comp_mode::invalidate_cache();
    crate::ui::surfaces::invalidate_backdrop_cache();
    let mut app = App::preview(Viewer::static_preview());
    app.focus_module("artifacts");
    app.scryglass
        .navigate(crate::ui::scryglass::StageRoute::Reinforce);
    app.tools.rl().mode = rl_ctl::RlMode::Campaign;
    {
        let rl = app.tools.rl();
        let mut progress = rl.progress.lock().unwrap();
        progress.planned_attempts = 3;
        progress.replace_points(
            [0.1, 1.0, 0.0]
                .into_iter()
                .enumerate()
                .map(|(index, reward)| rl_ctl::RunPoint {
                    step: index + 1,
                    reward,
                    latency_ms: 900 + index as u64 * 10,
                })
                .collect(),
        );
    }

    let screen = render_app_text(&mut app, 120, 40);
    assert!(screen.contains("1 BRANCH"), "{screen}");
    assert!(screen.contains("2 RESEARCH"), "{screen}");
    assert!(screen.contains("3 SANKEY"), "{screen}");
    assert_eq!(
        app.world_buttons
            .iter()
            .filter(|(_, button)| matches!(button, WorldButton::RlView(_)))
            .count(),
        3,
        "every visible lens must expose a mouse hit target"
    );

    app.on_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
    assert_eq!(app.rl_view, crate::ui::viz::rl_viz::RlView::Research);
    let research = render_app_text(&mut app, 120, 40);
    assert!(research.contains("OPTIMIZATION LEDGER"), "{research}");

    app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    assert_eq!(app.rl_view, crate::ui::viz::rl_viz::RlView::Sankey);
    let sankey = render_app_text(&mut app, 120, 40);
    assert!(sankey.contains("SANKEY · PROMOTION FLOW"), "{sankey}");
}

#[test]
fn cockpit_scenario_baselines_cover_representative_sizes() {
    // Renders world/scenario ink, which reads ANGEL_WORLD_INK per cell: hold
    // the crate env lock (and pin the default quantizer) so a concurrent
    // env-mutating test can never repaint these baselines mid-run.
    let _guard = env_lock();
    let _mode = crate::tests::TestEnvGuard::unset("ANGEL_WORLD_INK");
    let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = crate::tests::TestEnvGuard::unset("ANGEL_TURBO");
    let _backdrop = crate::tests::TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::drive::comp_mode::invalidate_cache();
    crate::ui::surfaces::invalidate_backdrop_cache();
    for (width, height) in [(80, 24), (120, 40), (180, 50)] {
        let mut scenario = CockpitScenario::at(width, height);
        let mut text = scenario.render();
        if width < 100 || height < 30 {
            assert!(
                text.contains("agent shell"),
                "compact Core owns the body\n{text}"
            );
            assert!(
                !text.contains("Scryglass"),
                "compact stage is explicit\n{text}"
            );
            scenario.app.focus_module("artifacts");
            text = scenario.render();
        }
        assert!(scenario.app.scryglass.visible, "{width}x{height}");
        assert!(
            text.chars()
                .any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch)),
            "startup must render the realm map at {width}x{height}\n{text}"
        );
        assert!(!text.contains("ARRIVAL"), "fresh startup\n{text}");
    }
}

#[test]
fn hidden_comp_skips_arrival_overlay_without_slowing_default() {
    use crate::tests::TestEnvGuard;
    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::drive::comp_mode::invalidate_cache();
    assert!(crate::ui::scryglass::Scryglass::arrival_overlay_allowed());

    let mut visible = crate::ui::scryglass::Scryglass::default();
    visible.sync_arrival(Some(world_viz::Building::Keep));
    visible.sync_arrival(Some(world_viz::Building::Smithy));
    assert_eq!(visible.arrival(), Some(world_viz::Building::Smithy));
    assert!(
        matches!(
            visible.controller.overlay(),
            Some(crate::ui::scryglass::StageOverlay::Arrival {
                destination: world_viz::Building::Smithy
            })
        ),
        "default still arms ARRIVAL after a journey"
    );

    let _on = TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::drive::comp_mode::invalidate_cache();
    assert!(!crate::ui::scryglass::Scryglass::arrival_overlay_allowed());

    let mut armed = crate::ui::scryglass::Scryglass::default();
    armed.sync_arrival(Some(world_viz::Building::Keep));
    armed.sync_arrival(Some(world_viz::Building::Chapel));
    assert!(
        armed.arrival().is_none(),
        "Hidden/comp must not spawn an ARRIVAL plate"
    );
    assert!(
        armed.controller.overlay().is_none(),
        "Hidden/comp must not steal Stage overlay for leftover arrival"
    );

    drop(_on);
    crate::drive::comp_mode::invalidate_cache();
    assert!(crate::ui::scryglass::Scryglass::arrival_overlay_allowed());
    armed.sync_arrival(Some(world_viz::Building::Chapel));
    assert!(
        armed.arrival().is_none(),
        "last_arrived must stay tracked so a later visible frame does not replay Chapel"
    );
    armed.sync_arrival(Some(world_viz::Building::Smithy));
    assert_eq!(armed.arrival(), Some(world_viz::Building::Smithy));
}

#[test]
fn settled_or_hidden_realm_does_not_request_fast_tick() {
    let _lock = env_lock();
    let _off = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = crate::tests::TestEnvGuard::unset("ANGEL_TURBO");
    crate::drive::comp_mode::invalidate_cache();
    let mut scenario = CockpitScenario::at(120, 40);
    scenario.app.world_pane_visible = false;
    scenario.app.scryglass.set_stage_visibility(false, false);
    assert!(!scenario.app.needs_fast_tick());

    scenario
        .app
        .scryglass
        .sync_arrival(Some(world_viz::Building::Keep));
    scenario
        .app
        .scryglass
        .sync_arrival(Some(world_viz::Building::Smithy));
    // Simulate the draw contract: a renderable Scryglass surface re-earns
    // visible-stage cadence for this frame.
    scenario.app.world_pane_visible = true;
    scenario.app.scryglass.set_stage_visibility(true, true);
    assert!(scenario.app.needs_fast_tick());
    scenario.advance_visible(Duration::ZERO);
    scenario.advance_visible(Duration::from_millis(1_250));
    assert!(!scenario.app.scryglass.animating());
}

#[test]
fn compact_core_draw_resets_prior_stage_visibility_and_fast_tick() {
    use crate::tests::TestEnvGuard;

    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = TestEnvGuard::unset("ANGEL_TURBO");
    let _backdrop = TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::drive::comp_mode::invalidate_cache();
    crate::ui::surfaces::invalidate_backdrop_cache();
    let mut app = seed_preview_app();
    // This fixture isolates Stage cadence after the startup sword is gone.
    app.startup_intro.dismiss(
        std::time::Instant::now(),
        crate::ui::viz::lifecycle_viz::MotionMode::Off,
    );
    app.scryglass.sync_arrival(Some(world_viz::Building::Keep));
    app.scryglass
        .sync_arrival(Some(world_viz::Building::Smithy));

    let standard = render_app_text(&mut app, 120, 40);
    assert!(
        standard
            .chars()
            .any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch)),
        "visible world must paint dots"
    );
    assert!(app.scryglass.visible && app.scryglass.renderable);
    assert!(app.needs_fast_tick());

    let compact = render_app_text(&mut app, 80, 24);
    assert!(compact.contains("agent shell"), "{compact}");
    assert!(!compact.contains("Scryglass"), "{compact}");
    assert!(!app.scryglass.visible);
    assert!(!app.scryglass.renderable);
    assert_eq!(app.scryglass.surface, scryglass::StageSurface::Hidden);
    assert!(!app.needs_fast_tick());
}

/// A6: compact Core (<100 cols or <30 rows) must register the same prose-only
/// transcript copy rect as the wide path — never the full inner (scroll rail /
/// tool-strip chrome used to be clipboard-eligible).
#[test]
fn compact_core_registers_prose_only_transcript_copy_rect() {
    let _lock = env_lock();
    let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = crate::tests::TestEnvGuard::unset("ANGEL_TURBO");
    let _backdrop = crate::tests::TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::drive::comp_mode::invalidate_cache();
    crate::ui::surfaces::invalidate_backdrop_cache();
    let mut app = seed_preview_app();
    assert!(app.thinking.is_none(), "preview is idle");
    // Compact terminal → full-body Core transcript path in `ui`.
    let _ = render_app_text(&mut app, 80, 24);
    let frame = app
        .panel_frames
        .get(panels::PanelKind::Transcript)
        .expect("compact Core publishes a Transcript frame");
    let pane = app
        .panes
        .rect_of(mouse::PaneId::Transcript)
        .expect("transcript copy hit-target");
    let inner = mouse::inner_border(frame);
    // Idle ambience is chrome too: it must never enter copied prose.
    let expected =
        surfaces::transcript_live_copy_rect(frame, 1).expect("usable compact prose rect");
    assert_eq!(
        pane, expected,
        "compact copy pane must match live prose geometry\n pane={pane:?}\n expected={expected:?}\n frame={frame:?}"
    );
    // Must not be the full chrome-free inner (rail is the last column).
    assert_ne!(
        pane, inner,
        "full inner_border must not be the copy hit-target"
    );
    if inner.width >= 2 {
        assert!(
            pane.width < inner.width,
            "compact copy must exclude the scroll rail: pane={pane:?} inner={inner:?}"
        );
        assert_eq!(
            pane.width,
            inner.width.saturating_sub(1).min(103),
            "prose measure matches text half"
        );
    }

    // Wide path stays consistent (same helper).
    let mut wide = seed_preview_app();
    let _ = render_app_text(&mut wide, 144, 48);
    let wframe = wide
        .panel_frames
        .get(panels::PanelKind::Transcript)
        .expect("wide Transcript frame");
    let wpane = wide
        .panes
        .rect_of(mouse::PaneId::Transcript)
        .expect("wide copy hit-target");
    let wexpected = surfaces::transcript_live_copy_rect(wframe, 1).expect("wide prose rect");
    assert_eq!(wpane, wexpected, "wide path still prose-only");
}

#[test]
fn miniviz_hidden_skip_does_not_run_expensive_compose() {
    use crate::ui::scryglass::StageSurface;
    let _lock = env_lock();
    let _off = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::drive::comp_mode::invalidate_cache();
    assert!(!draw::miniviz_expensive_compose_allowed(
        StageSurface::Hidden
    ));
    assert!(draw::miniviz_expensive_compose_allowed(
        StageSurface::Workshop
    ));
    assert!(draw::miniviz_expensive_compose_allowed(
        StageSurface::WorldMap
    ));
    assert!(draw::miniviz_expensive_compose_allowed(
        StageSurface::WorldFirstPerson
    ));

    let mut composed = 0usize;
    let hidden = draw::maybe_paint_world_scene(StageSurface::Hidden, || {
        composed += 1;
        "pixels"
    });
    assert!(hidden.is_none());
    assert_eq!(composed, 0, "hidden/off-stage must not invoke compose");

    let visible = draw::maybe_run_expensive_world_compose(true, || {
        composed += 1;
        "pixels"
    });
    assert_eq!(visible, Some("pixels"));
    assert_eq!(composed, 1);
}

#[test]
fn miniviz_comp_mode_skips_compose_without_dropping_assets() {
    let _lock = env_lock();
    let _g = crate::tests::TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::drive::comp_mode::invalidate_cache();
    assert!(!draw::miniviz_expensive_compose_allowed(
        crate::ui::scryglass::StageSurface::Workshop
    ));
    assert!(!draw::miniviz_dancer_paint_allowed(true, true));
    let mut composed = 0usize;
    assert!(
        draw::maybe_run_expensive_world_compose(false, || {
            composed += 1;
            "x"
        })
        .is_none()
    );
    assert_eq!(composed, 0);
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    assert!(
        root.join(crate::ui::viz::loop_viz::hammertime_asset(0.0))
            .is_file()
    );
    assert!(!crate::drive::comp_mode::ambient_stage_sim_allowed());
}

#[test]
fn comp_mode_skips_ambient_stage_fast_tick_without_slowing_default() {
    use crate::drive::loop_ctl::LoopStatus;
    let _lock = env_lock();
    let _off = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::drive::comp_mode::invalidate_cache();
    let mut app = seed_preview_app();
    app.world_pane_visible = true;
    app.loop_ctl.status = LoopStatus::Running;
    app.loop_ctl.cycle_started_ms = Some(1);
    assert!(crate::drive::comp_mode::ambient_stage_sim_allowed());
    assert!(app.stage_display_wants_fast_tick());

    let _on = crate::tests::TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::drive::comp_mode::invalidate_cache();
    assert!(!crate::drive::comp_mode::ambient_stage_sim_allowed());
    assert!(
        !app.stage_display_wants_fast_tick(),
        "comp/lean must not 33ms-tick ambient loop/raytrace/moa"
    );
    assert!(
        !app.needs_fast_tick(),
        "idle thinking must stay off the fast lane in comp-mode"
    );
}

#[test]
fn comp_mode_skips_lifecycle_rl_graph_scene_without_slowing_default() {
    use crate::tests::TestEnvGuard;
    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::drive::comp_mode::invalidate_cache();
    let mut app = seed_preview_app();
    let standard = render_app_text(&mut app, 120, 40);
    assert!(
        standard
            .chars()
            .any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch)),
        "visible world must paint dots"
    );
    assert!(app.start_lifecycle_ceremony(
        crate::ui::viz::lifecycle_viz::CeremonyKind::LoopDone,
        "visible ceremony"
    ));
    let ceremony = render_app_text(&mut app, 120, 40);
    assert!(
        ceremony.contains("tourney · victory pass"),
        "default Stage still paints the ceremony\n{ceremony}"
    );
    assert!(
        app.lifecycle_ceremony_animating(),
        "default visible ceremony stays on the 33ms lane"
    );
    assert!(app.needs_fast_tick());

    let _on = TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::drive::comp_mode::invalidate_cache();
    assert!(!crate::drive::comp_mode::ambient_stage_sim_allowed());
    assert!(
        app.lifecycle_ceremony_active(),
        "comp-mode must not drop the armed ceremony"
    );
    assert!(
        !app.lifecycle_ceremony_animating(),
        "comp-mode must not 33ms-tick ceremony/rl/graph scenes"
    );
    assert!(
        !app.needs_fast_tick(),
        "idle thinking must stay off the fast lane in comp-mode"
    );
}

#[test]
fn comp_mode_skips_observatory_quest_vault_scene_without_slowing_default() {
    use crate::tests::TestEnvGuard;
    use crate::ui::scryglass::StageRoute;
    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::drive::comp_mode::invalidate_cache();
    assert!(crate::drive::comp_mode::ambient_stage_sim_allowed());

    let mut app = seed_preview_app();
    app.scryglass.navigate(StageRoute::Observatory);
    let observatory = render_app_text(&mut app, 144, 48);
    assert!(
        observatory.contains("BACKPLANE") && observatory.contains("/rl policy"),
        "default Observatory still paints the backplane catalog\n{observatory}"
    );

    app.scryglass.navigate(StageRoute::Quest);
    let quest = render_app_text(&mut app, 144, 48);
    assert!(
        quest.contains("Quest Board") && quest.contains("No quest trace is open"),
        "default Quest still paints the board body\n{quest}"
    );

    app.scryglass.navigate(StageRoute::Vault);
    let vault = render_app_text(&mut app, 144, 48);
    assert!(
        vault.contains("Vault") || vault.contains("Living Atlas"),
        "default Vault still paints chrome\n{vault}"
    );
    assert!(
        vault.contains("no delivered artifacts") || vault.contains("Review"),
        "default Vault still paints body\n{vault}"
    );

    let _on = TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::drive::comp_mode::invalidate_cache();
    assert!(!crate::drive::comp_mode::ambient_stage_sim_allowed());

    let mut armed = seed_preview_app();
    armed.scryglass.navigate(StageRoute::Observatory);
    let lean_obs = render_app_text(&mut armed, 144, 48);
    assert!(
        lean_obs.contains("Observatory"),
        "comp-mode keeps Observatory chrome\n{lean_obs}"
    );
    assert!(
        !lean_obs.contains("BACKPLANE"),
        "comp-mode must not snapshot the backplane catalog\n{lean_obs}"
    );

    armed.scryglass.navigate(StageRoute::Quest);
    let lean_quest = render_app_text(&mut armed, 144, 48);
    assert!(
        lean_quest.contains("Quest Board"),
        "comp-mode keeps Quest chrome\n{lean_quest}"
    );
    assert!(
        !lean_quest.contains("No quest trace is open"),
        "comp-mode must not render the quest chart\n{lean_quest}"
    );

    armed.scryglass.navigate(StageRoute::Vault);
    let lean_vault = render_app_text(&mut armed, 144, 48);
    assert!(
        lean_vault.contains("Vault") || lean_vault.contains("Living Atlas"),
        "comp-mode keeps Vault chrome\n{lean_vault}"
    );
    assert!(
        !lean_vault.contains("no delivered artifacts"),
        "comp-mode must not list vault artifacts\n{lean_vault}"
    );
}

#[test]
fn comp_mode_skips_scryglass_catalog_and_lesson_body_without_slowing_default() {
    use crate::tests::TestEnvGuard;
    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::drive::comp_mode::invalidate_cache();
    assert!(draw::scryglass_scene_body_allowed());
    assert!(crate::drive::comp_mode::ambient_stage_sim_allowed());

    let mut app = seed_preview_app();
    assert!(app.scryglass.open_catalog());
    let catalog = render_app_text(&mut app, 144, 48);
    assert!(
        app.scryglass.surface == scryglass::StageSurface::Catalog,
        "default Catalog still paints chrome\n{catalog}"
    );
    assert!(
        catalog.contains("College foundations") || catalog.contains("Rowan Compass"),
        "default Catalog still paints the shelf body\n{catalog}"
    );

    assert!(app.scryglass.set_unrolled_lesson(
        "Poisson",
        "Poisson",
        "A discrete probability distribution."
    ));
    app.scryglass.poll_lesson(false);
    let lesson = render_app_text(&mut app, 144, 48);
    assert!(
        lesson.contains("Poisson") && lesson.contains("discrete probability"),
        "default Lesson still paints the teaching body\n{lesson}"
    );

    let _on = TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::drive::comp_mode::invalidate_cache();
    assert!(!draw::scryglass_scene_body_allowed());

    let mut armed = seed_preview_app();
    assert!(armed.scryglass.open_catalog());
    let lean_cat = render_app_text(&mut armed, 144, 48);
    assert!(
        armed.scryglass.visible && armed.scryglass.surface == scryglass::StageSurface::Catalog,
        "comp-mode keeps Catalog chrome\n{lean_cat}"
    );
    assert!(
        !lean_cat.contains("College foundations") && !lean_cat.contains("Rowan Compass"),
        "comp-mode must not list catalog shelves\n{lean_cat}"
    );

    assert!(armed.scryglass.set_unrolled_lesson(
        "Poisson",
        "Poisson",
        "A discrete probability distribution."
    ));
    let lean_lesson = render_app_text(&mut armed, 144, 48);
    assert!(
        lean_lesson.contains("Poisson"),
        "comp-mode keeps Lesson chrome\n{lean_lesson}"
    );
    assert!(
        !lean_lesson.contains("discrete probability"),
        "comp-mode must not wrap the lesson body\n{lean_lesson}"
    );
}

/// Comp / lean keeps Scryglass route identity but must not build world.title()
/// (town / quest / ward / activity / renown) every frame.
#[test]
fn hidden_comp_skips_live_world_title_without_slowing_default() {
    use crate::tests::TestEnvGuard;
    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _backdrop = TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::drive::comp_mode::invalidate_cache();
    crate::ui::surfaces::invalidate_backdrop_cache();
    assert!(draw::scryglass_live_world_title_allowed());
    assert_eq!(
        draw::scryglass_route_title("REALM", Some("Corelot · Keep · resting in the keep"), ""),
        ""
    );
    assert_eq!(draw::scryglass_route_title("REALM", None, ""), "");
    assert!(
        !draw::scryglass_route_title("REALM", None, "").contains("resting in the keep"),
        "lean title must not carry the live activity suffix"
    );

    let mut app = seed_preview_app();
    let town = app.world.town_name().to_string();
    let live = app.world.title();
    let realm = live
        .trim()
        .strip_prefix("Realm · ")
        .unwrap_or_else(|| live.trim());
    assert!(
        realm.contains(&town),
        "default world.title still includes the live town"
    );
    let standard = render_app_text(&mut app, 144, 48);
    assert!(
        !standard.contains("Scryglass · REALM"),
        "default world pane must not keep a persistent Scryglass/realm caption\n{standard}"
    );
    assert!(
        !standard.contains(&town),
        "default world pane must not paint the live town suffix\n{standard}"
    );
    assert!(crate::tests::contains_dotmax(&standard));
    assert!(
        standard.contains("[Library]") && standard.contains("[Back]"),
        "default still paints navigation controls\n{standard}"
    );

    let _on = TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::drive::comp_mode::invalidate_cache();
    assert!(!draw::scryglass_live_world_title_allowed());
    let mut armed = seed_preview_app();
    let lean_town = armed.world.town_name().to_string();
    let lean = render_app_text(&mut armed, 144, 48);
    assert!(
        !crate::tests::contains_dotmax(&lean),
        "comp-mode must not restore the persistent Scryglass/realm caption\n{lean}"
    );
    assert!(
        !lean.contains(&lean_town),
        "comp-mode must not build world.title() live suffix\n{lean}"
    );

    drop(_on);
    crate::drive::comp_mode::invalidate_cache();
    assert!(
        draw::scryglass_live_world_title_allowed(),
        "default cockpit must not stay gated after /comp off"
    );
}

/// Comp / lean keeps Quintain / Tiltyard / Round Table / Observatory route
/// chrome but must not build loop_viz / rl_viz / graph / observatory titles
/// (status / iter / step / report counts) every frame.
#[test]
fn hidden_comp_skips_live_stage_route_title_without_slowing_default() {
    use crate::drive::loop_ctl::LoopStatus;
    use crate::tests::TestEnvGuard;
    use crate::ui::scryglass::StageRoute;
    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _backdrop = TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::drive::comp_mode::invalidate_cache();
    crate::ui::surfaces::invalidate_backdrop_cache();
    assert!(draw::stage_live_route_title_allowed());
    assert_eq!(
        draw::stage_route_title(
            crate::stage::identity::STAGE_TITLE_QUINTAIN_PREFIX,
            Some("loop · running · iter 3/inf · local"),
            crate::stage::identity::STAGE_TITLE_DYNAMIC_SUFFIX,
        ),
        " Realm / Quintain · loop · running · iter 3/inf · local "
    );
    assert_eq!(
        draw::stage_route_title(
            crate::stage::identity::STAGE_TITLE_QUINTAIN_PREFIX,
            None,
            crate::stage::identity::STAGE_TITLE_DYNAMIC_SUFFIX,
        ),
        " Realm / Quintain ·  "
    );
    assert!(
        !draw::stage_route_title(
            crate::stage::identity::STAGE_TITLE_QUINTAIN_PREFIX,
            None,
            crate::stage::identity::STAGE_TITLE_DYNAMIC_SUFFIX,
        )
        .contains("iter"),
        "lean title must not carry the live loop suffix"
    );

    let mut app = seed_preview_app();
    app.loop_ctl.status = LoopStatus::Running;
    app.loop_ctl.iteration = 3;
    app.scryglass.navigate(StageRoute::Loop);
    let standard = render_app_text(&mut app, 144, 48);
    assert!(
        standard.contains("Quintain"),
        "default still paints route chrome\n{standard}"
    );
    assert!(
        standard.contains("iter 3"),
        "default still paints the live loop suffix\n{standard}"
    );

    let _on = TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::drive::comp_mode::invalidate_cache();
    assert!(!draw::stage_live_route_title_allowed());
    let mut armed = seed_preview_app();
    armed.loop_ctl.status = LoopStatus::Running;
    armed.loop_ctl.iteration = 3;
    armed.scryglass.navigate(StageRoute::Loop);
    let lean = render_app_text(&mut armed, 144, 48);
    assert!(
        lean.contains("Quintain"),
        "comp-mode keeps the Stage escape control\n{lean}"
    );
    assert!(
        !lean.contains("iter 3"),
        "comp-mode must not build crate::ui::viz::loop_viz::title live suffix\n{lean}"
    );

    drop(_on);
    crate::drive::comp_mode::invalidate_cache();
    assert!(
        draw::stage_live_route_title_allowed(),
        "default cockpit must not stay gated after /comp off"
    );
}

/// Comp / lean keeps Scryglass title chrome but must not build ride captions,
/// the Formation strip, or tick reveal/arrival timers.
#[test]
fn hidden_and_comp_mode_skip_scryglass_accessories_without_slowing_default() {
    use crate::tests::TestEnvGuard;
    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _backdrop = TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::drive::comp_mode::invalidate_cache();
    crate::ui::surfaces::invalidate_backdrop_cache();
    assert!(draw::scryglass_scene_accessories_allowed());
    assert!(draw::scryglass_scene_body_allowed());

    let mut app = seed_preview_app();
    let standard = render_app_text(&mut app, 144, 48);
    assert!(
        standard
            .chars()
            .any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch)),
        "visible world must paint dots"
    );
    assert!(
        standard.contains("FORMATION"),
        "Formation remains available in the header\n{standard}"
    );

    let _on = TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::drive::comp_mode::invalidate_cache();
    assert!(!draw::scryglass_scene_accessories_allowed());
    let mut armed = seed_preview_app();
    let lean = render_app_text(&mut armed, 144, 48);
    assert!(
        armed.scryglass.visible,
        "comp-mode keeps the Stage escape control\n{lean}"
    );
    assert!(
        !lean.contains("[Formation]"),
        "comp-mode must not paint the Formation accessory strip\n{lean}"
    );

    drop(_on);
    crate::drive::comp_mode::invalidate_cache();
    assert!(
        draw::scryglass_scene_accessories_allowed(),
        "default cockpit must not stay gated after /comp off"
    );
}

#[test]
fn hidden_and_comp_mode_skip_dotmax_without_slowing_default() {
    let _view =
        crate::stage::world_viz::world3d::pin(crate::stage::world_viz::world3d::WorldView::Mesh3d);
    use crate::stage::world_viz::{Building, take_ride_compose_count};
    use crate::tests::TestEnvGuard;
    use crate::ui::scryglass::{StageRoute, StageSurface};
    let _lock = env_lock();
    // This contract inspects text cells; native generated-dot transport has
    // its own pixel, continuity, geometry and ordinary-terminal checks.
    let _text_dots = TestEnvGuard::set("ANGEL_DOTMAX_PITCH", "text");
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _backdrop = TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::drive::comp_mode::invalidate_cache();
    crate::ui::surfaces::invalidate_backdrop_cache();

    assert!(draw::maybe_paint_world_scene(StageSurface::WorldMap, || 1).is_some());
    assert!(draw::maybe_paint_world_scene(StageSurface::WorldFirstPerson, || 1).is_some());
    assert!(draw::maybe_paint_world_scene(StageSurface::Hidden, || 1).is_none());

    let _ = take_ride_compose_count();
    let mut app = seed_preview_app();
    let standard = render_app_text(&mut app, 144, 48);
    assert!(
        standard
            .chars()
            .any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch)),
        "visible world must paint dots"
    );
    assert!(
        take_ride_compose_count() >= 1,
        "default visible world paints Dotmax"
    );
    app.scryglass.navigate(StageRoute::Explore(Building::Keep));
    app.world_yaw_offset = 0.15;
    let _ = take_ride_compose_count();
    let explore = render_app_text(&mut app, 144, 48);
    assert!(
        app.scryglass.visible
            && explore
                .chars()
                .any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch))
    );
    assert!(
        take_ride_compose_count() >= 1,
        "Explore responds to the Dotmax camera"
    );

    let _on = TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::drive::comp_mode::invalidate_cache();
    assert!(!draw::miniviz_expensive_compose_allowed(
        StageSurface::WorldMap
    ));
    assert!(!draw::miniviz_expensive_compose_allowed(
        StageSurface::WorldFirstPerson
    ));
    assert!(draw::maybe_paint_world_scene(StageSurface::WorldMap, || 1).is_none());
    assert!(draw::maybe_paint_world_scene(StageSurface::WorldFirstPerson, || 1).is_none());

    let _ = take_ride_compose_count();
    let mut armed = seed_preview_app();
    let comp_map = render_app_text(&mut armed, 144, 48);
    assert!(
        armed.scryglass.visible,
        "comp-mode keeps the Stage escape control\n{comp_map}"
    );
    assert_eq!(
        take_ride_compose_count(),
        0,
        "comp-mode must not compose Dotmax"
    );

    armed
        .scryglass
        .navigate(StageRoute::Explore(Building::Keep));
    let _ = take_ride_compose_count();
    let _ = render_app_text(&mut armed, 144, 48);
    assert_eq!(
        take_ride_compose_count(),
        0,
        "comp-mode must not march the first-person ride"
    );

    drop(_on);
    crate::drive::comp_mode::invalidate_cache();
    let _hidden = TestEnvGuard::set("ANGEL_BACKDROP", "off");
    crate::ui::surfaces::invalidate_backdrop_cache();
    let _ = take_ride_compose_count();
    let mut hidden = seed_preview_app();
    hidden
        .scryglass
        .navigate(StageRoute::Explore(Building::Keep));
    let off = render_app_text(&mut hidden, 144, 48);
    assert!(!off.contains("Scryglass"), "{off}");
    assert_eq!(take_ride_compose_count(), 0);
}

#[test]
fn hidden_and_comp_mode_skip_world_mirrors_without_slowing_default() {
    use crate::drive::loop_ctl::LoopStatus;
    use crate::tests::TestEnvGuard;
    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _backdrop = TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::drive::comp_mode::invalidate_cache();
    crate::ui::surfaces::invalidate_backdrop_cache();

    assert!(crate::drive::comp_mode::stage_world_mirrors_allowed(true));
    assert!(!crate::drive::comp_mode::stage_world_mirrors_allowed(false));

    let mut app = seed_preview_app();
    let standard = render_app_text(&mut app, 144, 48);
    assert!(
        standard
            .chars()
            .any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch)),
        "visible world must paint dots"
    );
    assert!(app.world_pane_visible);
    assert!(crate::drive::comp_mode::stage_world_mirrors_allowed(
        app.world_pane_visible
    ));

    assert!(app.scryglass.set_unrolled_lesson(
        "Poisson",
        "Poisson",
        "A discrete probability distribution."
    ));
    let shown_before = app.scryglass.lesson().map(|l| l.shown_chars()).unwrap_or(0);
    app.advance();
    let shown_after = app.scryglass.lesson().map(|l| l.shown_chars()).unwrap_or(0);
    assert!(
        shown_after > shown_before,
        "default visible Stage still rolls the lesson ({shown_before} → {shown_after})"
    );

    app.loop_ctl.status = LoopStatus::Running;
    app.loop_ctl.iteration = 3;
    app.loop_ctl.task = "visible loop mirror".into();
    app.advance();
    assert!(
        app.world.loop_mirrored_for_test(),
        "default visible Stage still mirrors /loop"
    );

    let r0 = app.world.renown();
    app.world.turn_started();
    app.finish_world_turn(true);
    assert!(
        app.world.renown() > r0,
        "default visible Stage still credits turn-end renown"
    );

    let _on = TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::drive::comp_mode::invalidate_cache();
    assert!(!crate::drive::comp_mode::stage_world_mirrors_allowed(true));

    let mut armed = seed_preview_app();
    let lean = render_app_text(&mut armed, 144, 48);
    assert!(
        armed.scryglass.visible,
        "comp-mode keeps the Stage escape control\n{lean}"
    );
    armed.world_pane_visible = true;
    assert!(
        !crate::drive::comp_mode::stage_world_mirrors_allowed(armed.world_pane_visible),
        "comp/lean must not pay mirrors while chrome is on screen"
    );

    assert!(armed.scryglass.set_unrolled_lesson(
        "Poisson",
        "Poisson",
        "A discrete probability distribution."
    ));
    let lean_before = armed
        .scryglass
        .lesson()
        .map(|l| l.shown_chars())
        .unwrap_or(0);
    armed.advance();
    let lean_after = armed
        .scryglass
        .lesson()
        .map(|l| l.shown_chars())
        .unwrap_or(0);
    assert_eq!(
        lean_before, lean_after,
        "comp/lean must not poll_lesson on every advance"
    );

    armed.loop_ctl.status = LoopStatus::Running;
    armed.loop_ctl.iteration = 3;
    armed.loop_ctl.task = "invisible loop tax".into();
    armed.advance();
    assert!(
        !armed.world.loop_mirrored_for_test(),
        "comp/lean must not note_loop"
    );

    let before = armed.world.renown();
    armed.world.turn_started();
    armed.finish_world_turn(true);
    assert_eq!(
        armed.world.renown(),
        before,
        "comp/lean must not credit turn-end fireworks"
    );
    assert_eq!(
        armed.world.active_work().count(),
        0,
        "hidden settlement still clears in-flight tool work"
    );
}

/// Comp / lean still lays Stage chrome, so `world_pane_visible` stays true.
/// Leftover knight/sparkle flags must not pin the scenery cadence — `world.tick`
/// is already skipped, so dest would never catch up.
#[test]
fn hidden_comp_does_not_steal_stage_column_for_ceremony() {
    use crate::tests::TestEnvGuard;
    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::drive::comp_mode::invalidate_cache();
    assert!(draw::ambient_stage_column_steal_allowed());
    assert!(
        draw::artifacts_pane_active(false, false, true, false, false),
        "default leftover ceremony still steals the Stage column"
    );
    assert!(
        draw::artifacts_pane_active(false, false, false, true, false),
        "default live loop still steals the Stage column"
    );
    assert!(
        draw::artifacts_pane_active(true, false, false, false, false),
        "operator-selected artifacts module still lays the Stage"
    );
    assert!(
        draw::artifacts_pane_active(false, true, false, false, false),
        "operator-selected raytrace still lays the Stage"
    );

    let mut app = seed_preview_app();
    let artifacts = crate::platform::runtime::ModuleId::new("artifacts");
    assert!(app.start_lifecycle_ceremony(
        crate::ui::viz::lifecycle_viz::CeremonyKind::LoopDone,
        "column steal"
    ));
    app.module_host
        .suspend(&artifacts)
        .expect("artifacts can be hidden after ceremony arms");
    assert!(!app.module_host.is_running("artifacts"));
    let standard = render_app_text(&mut app, 144, 48);
    assert!(
        standard.contains("tourney · victory pass"),
        "default still pops the Stage for a live ceremony\n{standard}"
    );
    assert!(
        app.panes
            .rect_of(crate::ui::mouse::PaneId::Artifacts)
            .is_some(),
        "default leftover ceremony still lays an artifacts column"
    );

    let _on = TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::drive::comp_mode::invalidate_cache();
    assert!(!draw::ambient_stage_column_steal_allowed());
    assert!(
        !draw::artifacts_pane_active(false, false, true, false, false),
        "Hidden/comp must not steal a column for leftover ceremony"
    );
    assert!(
        !draw::artifacts_pane_active(false, false, false, true, false),
        "Hidden/comp must not steal a column for hammertime"
    );
    assert!(
        draw::artifacts_pane_active(true, false, false, false, false),
        "Hidden/comp still keeps an operator-selected Stage"
    );

    let mut armed = seed_preview_app();
    assert!(armed.start_lifecycle_ceremony(
        crate::ui::viz::lifecycle_viz::CeremonyKind::LoopDone,
        "column steal"
    ));
    armed
        .module_host
        .suspend(&artifacts)
        .expect("artifacts can be hidden after ceremony arms");
    let lean = render_app_text(&mut armed, 144, 48);
    assert!(
        !lean.contains("tourney · victory pass"),
        "Hidden/comp must not pop an invisible ceremony column\n{lean}"
    );
    assert!(
        armed
            .panes
            .rect_of(crate::ui::mouse::PaneId::Artifacts)
            .is_none(),
        "Hidden/comp leftover ceremony must not steal an artifacts column"
    );
    assert!(
        !armed.world_pane_visible,
        "Hidden/comp leftover ceremony must not lay Stage chrome"
    );
}

#[test]
fn hidden_and_comp_mode_skip_world_animating_without_slowing_default() {
    use crate::tests::TestEnvGuard;
    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _backdrop = TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::drive::comp_mode::invalidate_cache();
    crate::ui::surfaces::invalidate_backdrop_cache();

    let mut app = seed_preview_app();
    app.world_pane_visible = true;
    app.world
        .note_tool_call("apply_patch", "move the knight for cadence testing");
    assert!(
        app.world.animating(),
        "default fixture still has on-screen world motion"
    );
    assert!(
        app.world_animating(),
        "default visible Stage still earns the scenery lane"
    );
    assert!(
        app.needs_responsive_tick(),
        "default visible motion still uses the bounded scenery cadence"
    );
    assert!(
        !app.needs_fast_tick(),
        "ambient world motion must not grab the 33ms lane"
    );

    let mut painted = seed_preview_app();
    let standard = render_app_text(&mut painted, 144, 48);
    assert!(
        standard
            .chars()
            .any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch)),
        "visible world must paint dots"
    );
    painted
        .world
        .note_tool_call("apply_patch", "move the knight for cadence testing");
    assert!(painted.world_pane_visible);
    assert!(
        painted.world_animating(),
        "default visible draw still reports world animation"
    );

    app.world_pane_visible = false;
    assert!(
        !app.world_animating(),
        "Hidden / unpainted Stage must not report world animation"
    );
    assert!(!app.needs_responsive_tick());

    let _on = TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::drive::comp_mode::invalidate_cache();
    let mut armed = seed_preview_app();
    armed
        .world
        .note_tool_call("apply_patch", "move the knight for cadence testing");
    let lean = render_app_text(&mut armed, 144, 48);
    assert!(
        armed.scryglass.visible,
        "comp-mode keeps the Stage escape control\n{lean}"
    );
    assert!(
        armed.world_pane_visible,
        "comp-mode still lays the Stage pane"
    );
    assert!(
        armed.world.animating(),
        "comp-mode must not drop leftover motion flags"
    );
    assert!(
        !armed.world_animating(),
        "comp/lean must not keep the scenery cadence while chrome is on screen"
    );
    assert!(
        !armed.needs_responsive_tick(),
        "comp/lean must park the event loop while idle thinking is off"
    );

    drop(_on);
    crate::drive::comp_mode::invalidate_cache();
    let mut restored = seed_preview_app();
    restored
        .world
        .note_tool_call("apply_patch", "move the knight for cadence testing");
    let _ = render_app_text(&mut restored, 144, 48);
    assert!(
        restored.world_animating(),
        "default cockpit must not stay gated after /comp off"
    );
}

#[test]
fn miniviz_dancers_still_select_and_paint_assets_when_visible() {
    use crate::drive::loop_ctl::{LoopState, LoopStatus};
    let _lock = env_lock();
    let _off = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::drive::comp_mode::invalidate_cache();

    let running = LoopState {
        status: LoopStatus::Running,
        ..LoopState::default()
    };
    assert!(draw::miniviz_dancer_paint_allowed(
        true,
        crate::ui::viz::loop_viz::hammertime_active(&running)
    ));
    assert!(!draw::miniviz_dancer_paint_allowed(
        false,
        crate::ui::viz::loop_viz::hammertime_active(&running)
    ));
    // Paused iterations keep the hammerdancers on stage (425e67d8): the loop
    // is still alive, only the cadence is held.
    assert!(draw::miniviz_dancer_paint_allowed(
        true,
        crate::ui::viz::loop_viz::hammertime_active(&LoopState {
            status: LoopStatus::Paused,
            ..LoopState::default()
        })
    ));
    assert!(!draw::miniviz_dancer_paint_allowed(
        true,
        crate::ui::viz::loop_viz::hammertime_active(&LoopState::default())
    ));

    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for t in [0.0_f32, 0.3, 0.6] {
        let asset = crate::ui::viz::loop_viz::hammertime_asset(t);
        let twin = crate::ui::viz::loop_viz::hammertime_twin_asset(t);
        assert!(root.join(asset).is_file(), "dancer asset missing: {asset}");
        assert!(
            root.join(twin).is_file(),
            "twin dancer asset missing: {twin}"
        );
    }
    let area = ratatui::layout::Rect::new(0, 0, 40, 20);
    let [left, right] = crate::ui::viz::loop_viz::hammertime_duo_boxes(area, 0.4);
    assert!(left.width > 0 && right.width > 0);
}

/// Z4: the quest line owns the world pane's caption row while an adventure is
/// running, and the pane's own border carries the quest's weather.
#[test]
fn the_quest_hud_and_its_border_reach_the_world_pane() {
    let _lock = env_lock();
    let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = crate::tests::TestEnvGuard::unset("ANGEL_TURBO");
    let _backdrop = crate::tests::TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::drive::comp_mode::invalidate_cache();
    crate::ui::surfaces::invalidate_backdrop_cache();

    let mut app = seed_preview_app();
    app.focus_module("artifacts");
    app.scryglass.navigate(scryglass::StageRoute::Realm);
    let town = render_app_text(&mut app, 144, 48);
    assert!(
        !town.contains("The Mines"),
        "an idle town shows none of the adventure chrome\n{town}"
    );

    app.world
        .note_adventure(world_viz::AdventureEvent::LoopStarted {
            kind: world_viz::LoopKind::Competition,
            task: "kernel benchmark".to_string(),
        });
    app.world
        .note_adventure(world_viz::AdventureEvent::Iteration { n: 7 });
    app.world
        .note_adventure(world_viz::AdventureEvent::Stall { level: 2 });

    let mut terminal = Terminal::new(TestBackend::new(144, 48)).unwrap();
    terminal.draw(|frame| draw::ui(frame, &mut app)).unwrap();
    let screen = test_backend_text(terminal.backend());
    assert_eq!(app.world.quest().region().label(), "The Swamp");
    assert!(app.world.quest_owns_pane());
    assert!(
        screen.contains("[Back]"),
        "world action rail remains available"
    );
    assert!(
        !screen.contains("≈ The Swamp"),
        "decorative footer labels stay out of miniviz"
    );

    // Deep fog tints the pane's own border amber — chrome, not plate.
    let buffer = terminal.backend().buffer();
    let ambered = (0..buffer.area.height).any(|y| {
        (0..buffer.area.width).any(|x| {
            buffer
                .cell((x, y))
                .is_some_and(|cell| cell.fg == hud::HUD_AMBER && cell.symbol().trim() != "")
        })
    });
    assert!(ambered, "a danger-2 quest ambers the world pane border");
}

/// Z5 §1: off Castle Town the quest owns the pane.
///
/// The live run (Toymaker, 2026-09-06) had a coding loop in The Mines while a
/// `write_file` walked the town knight to the Smithy — and the Smithy's
/// ARRIVAL establishing shot took the pane, with the Mines plate underneath
/// and the Z4 HUD gone. The ride surface has to survive the arrival, and the
/// arrival has to come back the moment the quest is home again.
#[test]
fn a_tool_arrival_never_takes_the_pane_from_a_live_quest() {
    use crate::drive::loop_ctl::LoopStatus;
    let _lock = env_lock();
    let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = crate::tests::TestEnvGuard::unset("ANGEL_TURBO");
    let _backdrop = crate::tests::TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::drive::comp_mode::invalidate_cache();
    crate::ui::surfaces::invalidate_backdrop_cache();

    let mut app = seed_preview_app();
    app.focus_module("artifacts");
    app.scryglass.navigate(scryglass::StageRoute::Realm);
    // Baseline the arrival tracker on the world's parked Keep, exactly as the
    // first live frame does.
    let _first = render_app_text(&mut app, 144, 48);

    // A coding loop puts the party in The Mines (adventure::home_region).
    app.loop_ctl.status = LoopStatus::Running;
    app.loop_ctl.iteration = 3;
    app.loop_ctl.task = "implement the tool-arrival seam".into();
    app.adventure_mirror_drain();
    assert_eq!(
        app.world.quest().region(),
        world_viz::Region::TheMines,
        "a coding loop adventures in the mines"
    );
    assert!(app.world.quest_owns_pane());

    // The live tool path: classify → journey → the knight settles at the
    // landmark, which is what arms the ARRIVAL overlay.
    let call_id = crate::agent::harness::ToolEventId("z5-quest-arrival".into());
    app.world
        .note_tool_call_event(call_id.clone(), "write_file", "cockpit/src/lib.rs");
    app.scryglass
        .begin_journey(call_id, world_viz::Building::Smithy, false);
    for _ in 0..200 {
        app.world.tick();
        if app.world.arrived_building() == Some(world_viz::Building::Smithy) {
            break;
        }
    }
    assert_eq!(
        app.world.arrived_building(),
        Some(world_viz::Building::Smithy),
        "the knight has to reach the landmark for ARRIVAL to arm at all"
    );

    let screen = render_app_text(&mut app, 144, 48);
    assert!(
        matches!(
            app.scryglass.controller.overlay(),
            Some(crate::ui::scryglass::StageOverlay::Arrival { .. })
        ),
        "the arrival cue is still recorded — it is only the surface that changes"
    );
    assert_eq!(
        app.scryglass
            .controller
            .resolved_scene(false, false, app.world.quest_owns_pane()),
        crate::ui::scryglass::StageSurface::WorldFirstPerson,
        "the ride surface stays while the quest is away from town"
    );
    assert!(
        app.world.quest().region().label() == "The Mines" && app.world_pane_visible,
        "quest still owns the world pane after the tool arrival\n{screen}"
    );
    assert!(
        !screen.contains("ARRIVAL"),
        "no town establishing shot over a Mines plate\n{screen}"
    );
    let hud = app
        .world
        .quest_hud(80)
        .expect("a live quest has a HUD line");
    let hud_text: String = hud
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>();
    let head = hud_text.split(" · ").next().expect("the region leads");
    assert!(
        !screen.lines().any(|line| line.contains(head)) && screen.contains("[Back]"),
        "the quiet world keeps its controls and omits decorative quest captions"
    );

    // Home again: the loop finishes, the Homecoming walk expires, and the
    // town's own arrivals resume untouched.
    // (`App::loop_finish` sets exactly this and then persists the loop state
    // to the workspace's store; the mirror only ever reads the status, and a
    // rendering test must not write a shared file.)
    app.loop_ctl.status = LoopStatus::Done;
    app.adventure_mirror_drain();
    for _ in 0..130 {
        app.world.tick();
    }
    assert_eq!(app.world.quest().region(), world_viz::Region::CastleTown);
    assert!(!app.world.quest_owns_pane());

    app.scryglass.navigate(scryglass::StageRoute::Realm);
    app.scryglass.sync_arrival(Some(world_viz::Building::Keep));
    app.scryglass
        .sync_arrival(Some(world_viz::Building::Scriptorium));
    assert_eq!(
        app.scryglass
            .controller
            .resolved_scene(false, false, app.world.quest_owns_pane()),
        crate::ui::scryglass::StageSurface::Arrival(world_viz::Building::Scriptorium),
        "back in town, arrivals are exactly what they were"
    );
}
