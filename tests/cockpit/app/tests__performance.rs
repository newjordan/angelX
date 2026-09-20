//! Foundational terminal performance budgets and resize-sensitive shell checks.

use super::*;

// ---------------------------------------------------------------------------
// Best-of-N perf sampling — the foundational_terminal_* budgets below.
//
// WHY THIS EXISTS. Do not "simplify" it back into a single timed run.
//
// `cargo test` runs the suite across parallel threads, so ONE wall-clock sample
// measures machine load, not code: a draw that genuinely costs 2ms is observed
// at 60ms whenever the scheduler preempts it mid-frame because five other tests
// are hammering the same cores. Measured one-shot, these budgets failed roughly
// half the time on unchanged code.
//
// That is not cosmetic noise. The Conductor's code rung gates autonomous work on
// "any test fail = reject" (docs/plans/conductor.md, Layer-2 gate semantics,
// mirrored by tools/self_model.rs). A perf test that flakes therefore rejects
// valid autonomous work at random — the coder looks broken while working
// perfectly — and it trains humans and agents alike to wave red away, which is
// how a real regression ships unnoticed.
//
// The fix keeps the perf signal rather than discarding it (raising the budgets
// or #[ignore]-ing them would discard it): take N samples and assert on the
// MINIMUM. The minimum is the least-contended observation, i.e. the closest
// available estimate of the uncontended cost. A genuine regression blows the
// budget on EVERY sample and still fails; only a loaded box — which blows it on
// SOME samples — is forgiven.
//
// Rules for callers, both load-bearing:
//   1. Setup is not timed. Build the App/Bag/Viewer outside the sample closure,
//      or rebuild it inside and start the clock after it.
//   2. Every sample must re-run the SAME code path. If the op is one-shot — a
//      key that consumes the approval modal, an `advance()` that drains a turn's
//      events, a `submit()` that fills the single turn slot — rebuild that
//      precondition per sample (`best_of_one_shot` / the `arm` and `prep`
//      arguments below). Otherwise samples 2..N measure a cheap no-op, the
//      minimum collapses to that no-op, and the budget stops meaning anything.
//   Env vars stay set/removed OUTSIDE the sampling loops, so repeating a body
//   under `env_lock()` can neither deadlock nor double-set/double-remove them.
const PERF_SAMPLES: usize = 5;
const PERF_BUDGET: Duration = Duration::from_millis(50);
const STARTUP_BUDGET: Duration = Duration::from_millis(100);

/// The least-contended of `n` timed samples. See the note above.
fn best_of(n: usize, mut sample: impl FnMut() -> Duration) -> Duration {
    (0..n.max(1))
        .map(|_| sample())
        .min()
        .expect("at least one sample")
}

/// Time one region. The clock starts after the caller's setup, and stops before
/// any teardown the caller does with the result.
fn timed(op: impl FnOnce()) -> Duration {
    let t0 = Instant::now();
    op();
    t0.elapsed()
}

/// Off the clock: let any turn this sample started actually land.
///
/// `Thinking::spawn` detaches a worker thread, and that worker calls
/// `log_trajectory`, which reads process-global env (`ANGEL_TRAJECTORY_LOG`) at
/// whatever moment it happens to finish. A worker still running after its test
/// releases `env_lock()` can therefore scribble a row into the file the NEXT
/// env-holding test is counting — an observed cross-test flake. Draining the
/// turn here keeps every worker's side effects inside this test's env_lock
/// window. No-op for scenarios with no live turn (the seeded ones hand `advance`
/// a disconnected channel, which clears `thinking` on the first call).
fn settle_turn(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.thinking.is_some() && Instant::now() < deadline {
        app.advance();
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Best-of-N for a ONE-SHOT op: each sample rebuilds `scenario` (untimed) and
/// times a single `op` against that fresh precondition. Hands back the last
/// scenario so the caller can keep asserting on it.
fn best_of_one_shot(
    n: usize,
    mut scenario: impl FnMut() -> App,
    mut op: impl FnMut(&mut App),
) -> (Duration, App) {
    let mut best: Option<Duration> = None;
    let mut last: Option<App> = None;
    for _ in 0..n.max(1) {
        let mut app = scenario();
        let elapsed = timed(|| op(&mut app));
        settle_turn(&mut app); // clock has stopped; see the note above
        best = Some(best.map_or(elapsed, |b: Duration| b.min(elapsed)));
        last = Some(app); // the previous sample's App drops here, off the clock
    }
    (
        best.expect("at least one sample"),
        last.expect("at least one sample"),
    )
}

fn assert_best_under(name: &str, budget: Duration, best: Duration) {
    assert_best_samples_under(name, budget, best, PERF_SAMPLES);
}

fn assert_best_samples_under(name: &str, budget: Duration, best: Duration, samples: usize) {
    eprintln!("{name} best-of-{samples} took {best:?} (budget {budget:?})");
    assert!(
        best < budget,
        "{name} best-of-{samples} took {best:?}, expected <{budget:?} \
         — the best sample is the least-contended one, so every sample blew the budget"
    );
}

fn assert_best_of_under(name: &str, budget: Duration, sample: impl FnMut() -> Duration) {
    assert_best_under(name, budget, best_of(PERF_SAMPLES, sample));
}

/// Best-of-N of a one-shot op, asserted; returns the last scenario.
fn assert_one_shot_under(
    name: &str,
    budget: Duration,
    scenario: impl FnMut() -> App,
    op: impl FnMut(&mut App),
) -> App {
    let (best, app) = best_of_one_shot(PERF_SAMPLES, scenario, op);
    assert_best_under(name, budget, best);
    app
}

fn assert_draw_under_50ms(name: &str, app: &mut App, width: u16, height: u16) {
    // A draw is repeatable: re-rendering the same frame takes the same path, so
    // the sample is the draw itself. `draw_once` builds its backend before it
    // starts the clock, so the sample is the `ui()` frame and nothing else.
    assert_best_of_under(name, PERF_BUDGET, || {
        draw_once(app, width, height).expect("draw")
    });
}

fn assert_image_ready_under_50ms(name: &str, app: &mut App, width: u16, height: u16) {
    // Liveness, not a budget: the preview is encoded on a background thread, and
    // a loaded box may take a while to hand it over. Generous on purpose — the
    // perf claim is the render-ready draw below, which is what gets sampled.
    let deadline = Instant::now() + Duration::from_secs(5);
    while !app.viewer.preview_ready() {
        draw_once(app, width, height).expect("draw");
        if app.viewer.preview_ready() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "{name} did not become render-ready within 5s"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_best_of_under(name, PERF_BUDGET, || {
        draw_once(app, width, height).expect("draw")
    });
}

fn assert_shell_draw_under_50ms(name: &str, app: &mut App, width: u16, height: u16) {
    assert_draw_under_50ms(name, app, width, height);
    let area = app
        .shell_area
        .unwrap_or_else(|| panic!("{name} did not record a shell render area"));
    let shell_size = app
        .shell
        .as_ref()
        .map(ShellPane::size)
        .unwrap_or_else(|| panic!("{name} lost its shell"));
    assert_eq!(
        shell_size,
        (area.height.max(1), area.width.max(1)),
        "{name} shell PTY size drifted from render area {area:?}"
    );
}

/// A local slash command, sampled best-of-N on one App. `prep` re-establishes
/// the precondition before each timed sample (untimed): `/hide` needs something
/// shown, or samples 2..N would time an already-hidden no-op.
fn assert_submit_under_50ms(name: &str, app: &mut App, prep: &[&str], input: &str) {
    assert_best_of_under(name, PERF_BUDGET, || {
        for cmd in prep {
            app.input = (*cmd).to_string();
            app.submit();
        }
        app.input = input.to_string();
        let t0 = Instant::now();
        app.submit();
        t0.elapsed()
    });
}

/// A turn-starting submit is one-shot: it fills the single turn slot, so a
/// second submit into the same App would take the cheap steer path instead
/// (`App::submit`'s busy gate). Each sample therefore gets a fresh App, built
/// with the input already in the composer so only `submit()` is on the clock.
fn assert_fresh_submit_under_50ms(name: &str, scenario: impl FnMut() -> App) {
    assert_one_shot_under(name, PERF_BUDGET, scenario, |app| app.submit());
}

fn assert_parse_under_50ms(name: &str, raw: &str) {
    assert_best_of_under(name, PERF_BUDGET, || {
        let t0 = Instant::now();
        let parsed = input::parse(raw);
        let elapsed = t0.elapsed();
        assert!(parsed.is_ok(), "{name} parse failed: {parsed:?}");
        elapsed
    });
}

fn assert_key_under_50ms(
    name: &str,
    app: &mut App,
    code: ratatui::crossterm::event::KeyCode,
    modifiers: ratatui::crossterm::event::KeyModifiers,
) {
    assert_key_armed_under_50ms(name, app, |_| {}, code, modifiers);
}

/// A key press whose precondition is consumed by the press itself (`y` answers
/// the approval modal and the modal is gone). `arm` rebuilds that precondition
/// before each sample, untimed — without it, samples 2..N would time a plain
/// character insert and the minimum would no longer measure the approval path.
fn assert_key_armed_under_50ms(
    name: &str,
    app: &mut App,
    mut arm: impl FnMut(&mut App),
    code: ratatui::crossterm::event::KeyCode,
    modifiers: ratatui::crossterm::event::KeyModifiers,
) {
    assert_best_of_under(name, PERF_BUDGET, || {
        arm(app);
        let t0 = Instant::now();
        app.on_key(ratatui::crossterm::event::KeyEvent::new(code, modifiers));
        t0.elapsed()
    });
}

/// `advance()` is one-shot: it drains the turn's event queue, so a second
/// advance on the same App would time an empty queue. Each sample rebuilds the
/// pending turn (untimed) and times a single drain.
fn assert_advance_under_50ms(name: &str, scenario: impl FnMut() -> App) {
    assert_one_shot_under(name, PERF_BUDGET, scenario, |app| app.advance());
}

#[test]
fn foundational_terminal_shell_entry_under_50ms() {
    let _guard = env_lock();
    if !pty_available() {
        eprintln!("skipping: no PTY access in this environment");
        return;
    }
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("SHELL", "/bin/sh") };

    // One-shot: Ctrl-G on an App that already has a shell just toggles focus and
    // never pays for the PTY spawn — which is the thing under budget. So every
    // sample gets a fresh App (built off the clock) and times the first Ctrl-G.
    let mut app =
        assert_one_shot_under("shell-entry-toggle", PERF_BUDGET, seed_preview_app, |app| {
            app.on_key(ratatui::crossterm::event::KeyEvent::new(
                ratatui::crossterm::event::KeyCode::Char('g'),
                ratatui::crossterm::event::KeyModifiers::CONTROL,
            ));
        });
    assert!(app.shell_focused, "shell should be focused after Ctrl-G");
    assert!(app.shell.is_some(), "shell should be spawned after Ctrl-G");

    assert_draw_under_50ms("shell-first-draw", &mut app, 120, 40);
    if let Some(shell) = app.shell.as_mut() {
        shell.send(b"for i in 1 2 3 4 5 6 7 8 9 10; do echo shell-output-$i; done\r");
    }
    std::thread::sleep(Duration::from_millis(20));
    assert_draw_under_50ms("shell-output-draw", &mut app, 120, 40);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("SHELL") };
}

#[test]
fn shell_pane_tracks_render_area_during_resize_sweep() {
    let _guard = env_lock();
    if !pty_available() {
        eprintln!("skipping: no PTY access in this environment");
        return;
    }
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("SHELL", "/bin/sh") };

    let mut app = seed_preview_app();
    app.on_key(ratatui::crossterm::event::KeyEvent::new(
        ratatui::crossterm::event::KeyCode::Char('g'),
        ratatui::crossterm::event::KeyModifiers::CONTROL,
    ));
    assert!(app.shell_focused, "shell should be focused after Ctrl-G");
    for (name, width, height) in [
        ("shell-resize-wide", 144, 48),
        ("shell-resize-compact", 72, 16),
        ("shell-resize-tiny", 8, 6),
        ("shell-resize-return", 120, 40),
    ] {
        assert_shell_draw_under_50ms(name, &mut app, width, height);
    }

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("SHELL") };
}

#[test]
fn foundational_terminal_startup_under_100ms() {
    let _guard = env_lock();
    let _protocol = TestEnvGuard::set("ANGEL_IMAGE_PROTOCOL", "kitty");

    // Prime lazy process caches before measuring the complete bootstrap.
    drop(App::new(Bag::practice_for_test(), Viewer::new()));

    // Here the setup IS the measurement, so the whole bootstrap repeats. It is
    // safely re-runnable: each App is dropped after the clock stops, and the env
    // var is set/removed outside the loop (never double-set under `env_lock`).
    const STARTUP_SAMPLES: usize = 10;
    let best = best_of(STARTUP_SAMPLES, || {
        let t0 = Instant::now();
        let bag = Bag::practice_for_test();
        let viewer = Viewer::new();
        let app = App::new(bag, viewer);
        let elapsed = t0.elapsed();
        drop(app);
        elapsed
    });
    assert_best_samples_under(
        "terminal-startup-bootstrap",
        STARTUP_BUDGET,
        best,
        STARTUP_SAMPLES,
    );
}

#[test]
#[ignore = "manual verifier startup profile; timings are diagnostics, not latency qualification"]
fn profile_verifier_registry_capture() {
    let _guard = env_lock();
    let workspace = std::env::current_dir().unwrap();
    for sample in 0..5 {
        let start = Instant::now();
        let cargo = crate::tools::build::PinnedCargo::capture(&workspace);
        let cargo_us = start.elapsed().as_micros();
        let start = Instant::now();
        let native = crate::tools::build::PinnedNativeRuntimes::capture(&workspace);
        let native_us = start.elapsed().as_micros();
        drop((cargo, native));
        let start = Instant::now();
        let pins = crate::tools::build::capture_verifier_runtimes(&workspace);
        let parallel_us = start.elapsed().as_micros();
        drop(pins);
        eprintln!(
            "{}",
            serde_json::json!({"sample": sample, "cargo_us": cargo_us,
                "native_us": native_us, "parallel_us": parallel_us})
        );
    }
}

#[test]
fn foundational_terminal_preview_dump_under_50ms() {
    let _guard = env_lock();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_IMAGE_PROTOCOL", "kitty") };

    // The dump is a pure render: every sample runs the identical path.
    let mut dumped = String::new();
    let best = best_of(PERF_SAMPLES, || {
        let t0 = Instant::now();
        let text = render_preview_text(144, 48).unwrap();
        let elapsed = t0.elapsed();
        dumped = text;
        elapsed
    });
    assert!(
        dumped.contains("angel0"),
        "preview dump missed cockpit header"
    );
    assert_best_under("preview-dump-render", PERF_BUDGET, best);

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_IMAGE_PROTOCOL") };
}

#[test]
fn foundational_terminal_surfaces_draw_under_50ms() {
    let _guard = env_lock();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_IMAGE_PROTOCOL", "kitty") };
    let session_dir = std::env::temp_dir().join(format!(
        "angel0-terminal-command-perf-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&session_dir);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_SESSION_DIR", &session_dir) };

    let png = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/agents/apollo-neutral.png");
    let reasoning = (0..12)
        .map(|i| format!("reasoning step {i}: checking the cockpit load path"))
        .collect::<Vec<_>>()
        .join("\n");

    let scenarios: Vec<(&str, App, u16, u16)> = vec![
        ("default-wide-first", seed_preview_app(), 144, 48),
        ("default-compact-first", seed_preview_app(), 60, 16),
        ("thinking-first", seed_thinking_app(&reasoning), 120, 40),
        (
            "heavy-transcript-first",
            seed_heavy_transcript_app(),
            120,
            40,
        ),
        ("artifacts-first", seed_artifact_app(), 144, 48),
        ("approval-modal-first", seed_approval_app(), 120, 40),
        ("composer-long-input-first", seed_long_input_app(), 120, 40),
    ];

    for (name, mut app, width, height) in scenarios {
        assert_draw_under_50ms(name, &mut app, width, height);
    }

    let mut resizing_app = seed_preview_app();
    for (name, width, height) in [
        ("default-resize-wide", 144, 48),
        ("default-resize-compact", 72, 16),
        ("default-resize-return", 120, 40),
    ] {
        assert_draw_under_50ms(name, &mut resizing_app, width, height);
    }

    if pty_available() {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("SHELL", "/bin/sh") };
        let mut shell_resize_app = seed_preview_app();
        shell_resize_app.on_key(ratatui::crossterm::event::KeyEvent::new(
            ratatui::crossterm::event::KeyCode::Char('g'),
            ratatui::crossterm::event::KeyModifiers::CONTROL,
        ));
        for (name, width, height) in [
            ("shell-resize-wide", 144, 48),
            ("shell-resize-compact", 72, 16),
            ("shell-resize-return", 120, 40),
        ] {
            assert_shell_draw_under_50ms(name, &mut shell_resize_app, width, height);
        }
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("SHELL") };
    } else {
        eprintln!("skipping shell-resize sweep: no PTY access in this environment");
    }

    let mut thinking_app = seed_thinking_app(&reasoning);
    for (name, width, height) in [
        ("thinking-resize-wide", 144, 48),
        ("thinking-resize-compact", 72, 16),
        ("thinking-resize-return", 120, 40),
    ] {
        assert_draw_under_50ms(name, &mut thinking_app, width, height);
    }

    let mut heavy_app = seed_heavy_transcript_app();
    for (name, width, height) in [
        ("heavy-transcript-resize-wide", 144, 48),
        ("heavy-transcript-resize-compact", 72, 16),
        ("heavy-transcript-resize-return", 120, 40),
    ] {
        assert_draw_under_50ms(name, &mut heavy_app, width, height);
    }

    let mut artifact_app = seed_artifact_app();
    for (name, width, height) in [
        ("artifacts-resize-wide", 144, 48),
        ("artifacts-resize-compact", 72, 16),
        ("artifacts-resize-return", 120, 40),
    ] {
        assert_draw_under_50ms(name, &mut artifact_app, width, height);
    }
    // These local commands are all idempotent — re-running one takes the same
    // path — except `/hide`, which needs an image on screen to have work to do.
    assert_submit_under_50ms("command-media-page", &mut artifact_app, &[], "/media");
    assert_submit_under_50ms("command-open-image", &mut artifact_app, &[], "/open 1");
    assert_submit_under_50ms(
        "command-hide-image",
        &mut artifact_app,
        &["/open 1"],
        "/hide",
    );
    assert_submit_under_50ms(
        "command-sessions-empty",
        &mut artifact_app,
        &[],
        "/sessions",
    );
    assert_submit_under_50ms("command-resume-empty", &mut artifact_app, &[], "/resume");
    let latest_session =
        seed_saved_sessions(&session_dir, artifact_app.tools.current_workspace(), 200);
    assert_submit_under_50ms("command-sessions-many", &mut artifact_app, &[], "/sessions");
    assert_submit_under_50ms(
        "command-resume-explicit",
        &mut artifact_app,
        &[],
        &format!("/resume {latest_session}"),
    );

    let mut many_artifacts = seed_many_image_artifact_app();
    assert_draw_under_50ms("artifacts-many-images-first", &mut many_artifacts, 144, 72);
    many_artifacts.media_scroll = 120;
    assert_draw_under_50ms(
        "artifacts-many-images-scrolled",
        &mut many_artifacts,
        144,
        72,
    );
    let agent_image =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/agents/apollo-neutral.png");
    let audio_path = session_dir.join("tiny.wav");
    std::fs::write(&audio_path, b"RIFF\x24\x00\x00\x00WAVEfmt ").unwrap();
    assert_parse_under_50ms(
        "parse-see-agent-image",
        &format!("/see {} Describe this.", agent_image.display()),
    );
    assert_parse_under_50ms(
        "parse-hear-small-audio",
        &format!("/hear {} Transcribe this.", audio_path.display()),
    );
    // Turn-starting submit: fresh App per sample (the composer is filled off the
    // clock), so every sample times a real turn spawn and not the steer path.
    assert_fresh_submit_under_50ms("submit-plain-message", || {
        let mut app = seed_practice_only_app();
        app.input = "check cockpit responsiveness".to_string();
        app
    });
    // …and the advance that picks the practice reply up: the scenario (submit,
    // then let the worker land) is rebuilt per sample, off the clock.
    assert_advance_under_50ms("advance-practice-reply", || {
        let mut app = seed_practice_only_app();
        app.input = "check cockpit responsiveness".to_string();
        app.submit();
        std::thread::sleep(Duration::from_millis(1));
        app
    });

    let mut key_app = seed_approval_app();
    assert_key_armed_under_50ms(
        "key-approval-approve",
        &mut key_app,
        arm_approval,
        ratatui::crossterm::event::KeyCode::Char('y'),
        ratatui::crossterm::event::KeyModifiers::NONE,
    );
    assert_key_under_50ms(
        "key-tab-agent-switch",
        &mut key_app,
        ratatui::crossterm::event::KeyCode::Tab,
        ratatui::crossterm::event::KeyModifiers::NONE,
    );
    assert_key_under_50ms(
        "key-scroll-page-up",
        &mut key_app,
        ratatui::crossterm::event::KeyCode::PageUp,
        ratatui::crossterm::event::KeyModifiers::NONE,
    );
    assert_key_under_50ms(
        "key-type-character",
        &mut key_app,
        ratatui::crossterm::event::KeyCode::Char('x'),
        ratatui::crossterm::event::KeyModifiers::NONE,
    );
    assert_key_under_50ms(
        "key-backspace",
        &mut key_app,
        ratatui::crossterm::event::KeyCode::Backspace,
        ratatui::crossterm::event::KeyModifiers::NONE,
    );

    // Each advance sample re-seeds its pending turn (untimed) — an advance on an
    // already-drained App would time an empty queue, not the event batch.
    assert_advance_under_50ms("advance-event-batch", || {
        seed_advancing_app(
            vec![
                harness::TurnEvent::Token("streamed preamble ".repeat(10)),
                harness::TurnEvent::Reasoning("private reasoning ".repeat(12)),
                harness::TurnEvent::ToolCall {
                    id: harness::ToolEventId("perf-shell".to_string()),
                    name: "shell".to_string(),
                    args_summary: "cargo test --bin angel".to_string(),
                },
                harness::TurnEvent::ToolResult {
                    id: harness::ToolEventId("perf-shell".to_string()),
                    name: "shell".to_string(),
                    summary: "tests passed".to_string(),
                    outcome: harness::ToolOutcome {
                        execution: harness::ExecutionOutcome::Succeeded,
                        verification: harness::VerificationOutcome::Passed,
                    },
                },
                harness::TurnEvent::Notice("context compacted".to_string()),
            ],
            None,
        )
    });

    let media_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/agents/apollo-neutral.png");
    assert_advance_under_50ms("advance-media-image", || {
        seed_advancing_app(
            vec![harness::TurnEvent::Media {
                kind: "image".to_string(),
                label: "apollo preview".to_string(),
                url: media_path.to_string_lossy().to_string(),
            }],
            None,
        )
    });

    assert_advance_under_50ms("advance-final-reply", || {
        let final_history = vec![
            ChatMsg::system("system"),
            ChatMsg::user("hello"),
            ChatMsg::assistant("done"),
        ];
        seed_advancing_app(Vec::new(), Some(Ok((final_history, "done".to_string()))))
    });

    let mut image_app = seed_preview_app();
    image_app
        .viewer
        .show(&png)
        .expect("decode a real asset png");
    for name in ["image-view-first", "image-view-second"] {
        assert_draw_under_50ms(name, &mut image_app, 120, 40);
    }
    assert_image_ready_under_50ms("image-ready-draw", &mut image_app, 120, 40);
    for (name, width, height) in [
        ("image-resize-wide", 144, 48),
        ("image-resize-compact", 72, 16),
        ("image-resize-tall", 96, 48),
        ("image-resize-return", 120, 40),
    ] {
        assert_draw_under_50ms(name, &mut image_app, width, height);
    }

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_IMAGE_PROTOCOL") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_SESSION_DIR") };
    let _ = std::fs::remove_dir_all(session_dir);
}
