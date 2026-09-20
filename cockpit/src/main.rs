//! Angel cockpit — ratatui chat shell.
//!
//! A Club-backed chat. The [`Bag`] holds several model "clubs"; one is in hand
//! and Tab cycles them. Sending runs the in-hand club on a worker thread while a
//! visual/overwatch bay updates, so the UI never blocks on the network.
//! The reply (or an error) arrives back over a channel.
//!
//! Later increments: dotmax braille charts, ratatui-image photos, PTY shell.

mod advisor;
mod agent_controls;
mod agent_profile;
mod agent_view;
mod agentviz;
mod helm;
// Bounded process bridge to the isolated surface-free WebGPU renderer. Visible
// output is admitted only through the calibrated Kitty image adapter.
mod agentviz_portal;
mod app_control;
mod approval;
mod approval_view;
mod artifacts_view;
mod atlas;
mod atlas_clerk;
mod authority_profile;
mod backplane;
mod barrel;
mod bootstrap;
mod campaign;
mod chart;
mod clipboard;
mod club;
mod code_mode;
mod codex_import;
mod compaction;
#[allow(dead_code)]
// Contract-first scaffold; consumers land in Wave 1 (board-sync is the first live path).
mod competition;
mod conductor;
mod continual_harness;
// The Cut: the authored-diff manifest + the machine verdict on every write
// (docs/plans/the-cut.md). Rust only appends JSONL; the Node tick folds it.
mod cut;
// Deli (deep-over-time research loop) folds into the swarm rather than being its
// own selectable agent — kept pending that symbiotic integration.
mod caddy;
#[allow(dead_code)]
mod deli;
mod dossier;
mod evidence;
mod experience;
mod formations;
mod frame_timing;
mod glyphs;
mod goal;
mod habits;
mod handoff_rl;
mod harness;
mod hashline;
mod hearth;
mod hud;
mod identity;
mod input;
mod iterate;
// Curation gate between swarm reports and the memory palace; swarm wiring is
// deferred until its dedup/importance policy is real (see librarian.rs).
mod app;
mod comp_mode;
mod conflict;
mod dot_canvas;
mod dot_protocol;
mod draw;
mod git_commit_split;
mod github_url;
mod graph_ctl;
mod graph_viz;
mod knight_cast;
mod knight_journey;
#[allow(dead_code)]
mod librarian;
mod library;
mod lifecycle_viz;
mod local_command;
mod loop_ctl;
mod loop_dialog;
mod loop_viz;
mod lsp;
mod magic_keywords;
mod markdown;
mod math;
mod mcp;
mod media;
mod memory;
mod memory_store;
mod mission;
mod moa_viz;
mod mouse;
mod observatory;
mod openai_codex;
mod overwatch;
mod pane_motion;
mod panels;
mod pty;
mod questmap;
mod raytrace;
#[allow(dead_code)] // reinforce-loop core; wired to DICE/GEPA in a follow-up
mod reinforce;
mod repos;
mod research_workspace;
mod retro_kit;
mod rl_ctl;
mod rl_viz;
mod route_intelligence;
mod route_preferences;
mod runtime;
mod sandbox;
mod science;
mod scryglass;
mod secrets;
mod self_loop;
mod session;
mod skills;
mod spend_viz;
mod staged_edit;
mod startup_intro;
mod status_view;
mod steer;
mod still_inspector;
mod store_caps;
mod stream_rules;
mod surfaces;
mod swarm;
mod swarm_delegate;
mod term;
mod term_lookup;
mod term_pipe;
mod terminal_art;
mod tools;
mod toolstrip;
mod transcript;
mod turn;
mod turn_event_view;
mod turn_phase;
mod ui_inspect;
mod viewer;
mod village;
mod visual_export;
mod workspace_lang;
mod workspace_store;
mod world_viz;
mod yolo;
mod yukon_fleet;
mod yukon_status;

use agent_profile::{
    AgentKey, AgentProfile, portrait_uses_high_effort, profile_for, profile_for_route,
    specialist_text,
};
use club::{Bag, ChatMsg, ChatRole};
use glyphs::AgentBadge;
use hud::{
    HUD_BLUE, HUD_DIM, HUD_PHOSPHOR, chrome_style, dim_panel_style, hud_block, panel_style,
    transparent_hud_block,
};
use lifecycle_viz::MotionMode;
use media::Media;
use overwatch::Overwatch;
use pty::ShellPane;
use ratatui::{
    Frame, Terminal,
    backend::{CrosstermBackend, TestBackend},
    crossterm::event::{
        DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
        EnableFocusChange, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    crossterm::execute,
    crossterm::terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
    },
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
    },
};

use std::borrow::Cow;
use std::cell::Cell;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};
use transcript::{Message, Role};
use turn::Thinking;
use viewer::Viewer;

use app::*;
use draw::*;
use term::*;

/// Idle event-wait. A plain blocking read would also work; this just caps how
/// long a quit/resize can lag while keeping idle CPU negligible.
const IDLE_POLL: Duration = Duration::from_millis(200);

/// Frame interval while the loading bar is on screen (~30fps).
const TICK: Duration = Duration::from_millis(33);
/// The miniworld is ambient scenery, not a reason to repaint the entire
/// terminal at video cadence. Twelve frames per second keeps travel legible
/// while leaving input and agent output the priority lane.
const SCENERY_TICK: Duration = Duration::from_millis(83);
/// How often the main loop republishes the fleet route snapshot to the
/// backplane. Discovery is minute-scale; per-keystroke was pure waste.
const BACKPLANE_REFRESH: Duration = Duration::from_millis(500);
const RESIZE_SETTLE: Duration = Duration::from_millis(220);
const HUD_BLUE_BORDER_STYLE: Style = Style::new().fg(HUD_BLUE);
const PHOSPHOR_STYLE: Style = Style::new().fg(HUD_PHOSPHOR);
const PHOSPHOR_BOLD_STYLE: Style = Style::new().fg(HUD_PHOSPHOR).add_modifier(Modifier::BOLD);
// ── Condensed-chrome layout ─────────────────────────────────────────────────
// Adjacent cockpit panels overlap by one cell so neighbors share a single
// border line (Layout spacing −1); `hud::merge_panel_borders` repairs the
// shared corners into ├ ┤ ┬ ┴ ┼ each frame. `panel_gap` survives as the
// "roomy terminal" signal for boxed header chrome.

/// Legacy gap unit — now only the roominess signal between cockpit panels.
const PANEL_GAP: u16 = 1;
/// Min terminal size that counts as roomy (boxed header chrome allowed).
const GAP_MIN_WIDTH: u16 = 48;
const GAP_MIN_HEIGHT: u16 = 12;
/// Min height at which the 1-row header/footer strips are promoted to their own
/// bordered boxes. Below this they stay flat strips to spare vertical rows.
const CHROME_BOX_MIN_HEIGHT: u16 = 20;

#[derive(Debug)]
struct ExitAndFlushError {
    original: std::io::Error,
    flush: std::io::Error,
}
impl std::fmt::Display for ExitAndFlushError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}; additionally, {}", self.original, self.flush)
    }
}
impl std::error::Error for ExitAndFlushError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.original)
    }
}

fn finish_exit_results(
    loop_result: std::io::Result<()>,
    flush_result: std::io::Result<()>,
) -> std::io::Result<()> {
    match (loop_result, flush_result) {
        (Ok(()), result) | (result, Ok(())) => result,
        (Err(original), Err(flush)) => Err(std::io::Error::new(
            original.kind(),
            ExitAndFlushError { original, flush },
        )),
    }
}

/// Final cleanup must not convert an unconfirmed save into a successful exit.
/// A staging path is an inspection pointer, never an accepted recovery snapshot.
fn flush_session_for_exit(session: &session::Session) -> std::io::Result<()> {
    session.flush_pending().map_err(|error| {
        let staging = session.path().with_extension(format!("json.{}.tmp", std::process::id()));
        std::io::Error::other(format!(
            "session exit flush failed for {}: {error}; committed checkpoint: {}; uncommitted staging, if present: {}. Inspect these paths before attempting manual recovery.",
            session.id, session.path().display(), staging.display()
        ))
    })
}

const PRACTICE_NOTICE: &str =
    "No live model route; interactive practice driver is active (offline echo).";
const NO_ROUTE: &str = "no model route: practice fallback is disabled in headless mode; set ANGEL_DRIVER and, for API providers, ANGEL_API_CLUBS plus ANGEL_<DRIVER>_KEY, ANGEL_<DRIVER>_URL and ANGEL_<DRIVER>_MODEL (for example ANGEL_OPENROUTER_KEY, ANGEL_OPENROUTER_URL, ANGEL_OPENROUTER_MODEL), or explicitly opt in with ANGEL_PRACTICE=1";

fn interactive_practice_notice(label: &str) -> Option<&'static str> {
    (label == "practice").then_some(PRACTICE_NOTICE)
}

fn practice_route_allowed(label: &str, opt_in: Option<&str>) -> bool {
    label != "practice" || opt_in == Some("1")
}

fn check_headless_route(
    club: &dyn club::Club,
    args: &harness::TaskCliArgs,
    workspace: &std::path::Path,
) {
    if practice_route_allowed(
        club.label(),
        std::env::var("ANGEL_PRACTICE").ok().as_deref(),
    ) {
        return;
    }
    let envelope = harness::TaskJsonEnvelope::from_startup_failure(
        args,
        workspace.to_path_buf(),
        0,
        harness::TaskStartupStopReason::NoRoute,
        NO_ROUTE.to_string(),
    );
    eprintln!("{NO_ROUTE}");
    println!(
        "{}",
        serde_json::to_string(&envelope).expect("task result envelope must serialize")
    );
    std::process::exit(2);
}

/// Apply one FIFO input batch before advancing or drawing. Priority events end
/// this frame's batch without reordering earlier literal text.
fn apply_terminal_input(
    app: &mut App,
    input: &input::TerminalInput,
    next_event: Option<Event>,
) -> std::io::Result<()> {
    if let Some(mut event) = next_event {
        let mut budget = 256;
        let mut saw_priority = false;
        loop {
            let priority = matches!(
                &event,
                Event::Key(key)
                    if key.kind == KeyEventKind::Press
                        && (matches!(key.code, KeyCode::Esc)
                            || (key.modifiers.contains(KeyModifiers::CONTROL)
                                && matches!(
                                    key.code,
                                    KeyCode::Char('c') | KeyCode::Char('C')
                                ))
                            || matches!(key.code, KeyCode::Enter))
            );
            match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => app.on_key(key),
                Event::Paste(text) => app.on_paste(&text),
                Event::Mouse(m) => app.on_mouse(m),
                Event::FocusGained => app.set_terminal_focused(true),
                Event::FocusLost => app.set_terminal_focused(false),
                Event::Resize(_, _) => app.viewer.invalidate_still_layout(),
                _ => {}
            }
            if priority {
                saw_priority = true;
            }
            budget -= 1;
            if app.should_quit || budget == 0 || saw_priority {
                break;
            }
            let Some(next) = input.next(std::time::Duration::ZERO)? else {
                break;
            };
            event = next;
        }
    }
    Ok(())
}

/// Drive the TUI until quit. Returns the staged phoenix exec (new binary +
/// session id) when `/self reborn` ended the loop, `None` on a normal quit.
fn run(
    terminal: &mut AngelTerminal,
    viewer: Viewer,
    resume: Option<Option<String>>,
) -> std::io::Result<Option<(std::path::PathBuf, String)>> {
    // Calibration has finished; establish the sole input reader before app
    // initialization so typed-ahead bursts are queued in order.
    let terminal_input = input::TerminalInput::start()?;
    // Bag::standard() assembles the fleet (spark/turbo/atlas + the spark-r1/spark-v4
    // ports) and falls back to the practice swing if none are up. Tab switches agents.
    let mut app = App::new(Bag::standard(), viewer);
    // Cold DNS/TLS/metadata off the first Enter path — fire-and-forget so the
    // event loop opens immediately while the in-hand club warms in the back.
    if let Some(notice) = interactive_practice_notice(app.bag.in_hand_label()) {
        app.system_msg(notice);
    }
    app.bag.warm_background();
    // Interactive sessions echo the operator's message on the very next frame
    // and run the heavy turn pre-flight one tick later (see `PendingTurn`).
    app.submit_deferral = true;
    // `--resume [id]` (the phoenix relight): reload a saved session before the
    // first frame so a restarted self continues the same conversation.
    if let Some(id) = resume {
        app.startup_resume(id);
    }
    eprintln!("RUN: entering event loop");
    let mut applied_title: Option<String> = None;
    let mut frame_timing = frame_timing::FrameTiming::from_env();
    let mut post_draw_us = 0;
    let mut post_draw_finished = frame_timing.as_ref().map(|_| Instant::now());
    let mut painted_once = false;
    // The backplane route snapshot is rebuilt from every fleet slot (locks +
    // allocations per club) just to detect "nothing changed" — pace it here
    // instead of paying it per keystroke. Unchanged bags reuse the generation-
    // keyed `route_choices` snapshot, so this refresh is a pointer compare.
    let mut backplane_refreshed_at = Instant::now()
        .checked_sub(BACKPLANE_REFRESH)
        .unwrap_or_else(Instant::now);
    let loop_result = (|| -> std::io::Result<()> {
        while !app.should_quit {
            let pre_poll_started = post_draw_finished;
            // Keep the PTY sized to the actual pane rect observed on the previous
            // draw. The next draw corrects it again after any terminal resize.
            if app.shell_focused
                && let Some(area) = app.shell_area
            {
                app.resize_shell_to_area(area);
            }
            // Apply a /title override (terminal title is out-of-band, safe mid-TUI).
            if app.title_override != applied_title {
                if let Some(t) = &app.title_override {
                    let _ = ratatui::crossterm::execute!(
                        std::io::stdout(),
                        ratatui::crossterm::terminal::SetTitle(t)
                    );
                }
                applied_title = app.title_override.clone();
            }
            // Input before paint so cancel/steer never wait on a flood redraw.
            let wait = if !painted_once {
                std::time::Duration::ZERO
            } else if app.needs_fast_tick() {
                TICK
            } else if app.needs_responsive_tick() {
                SCENERY_TICK
            } else {
                IDLE_POLL
            };
            let poll_started = frame_timing.as_ref().map(|_| Instant::now());
            let pre_poll_us = pre_poll_started
                .zip(poll_started)
                .map_or(0, |(start, end)| end.duration_since(start).as_micros());
            let next_event = terminal_input.next(wait)?;
            let poll_us = poll_started.map_or(0, |start| start.elapsed().as_micros());
            let input_started = frame_timing.as_ref().map(|_| Instant::now());
            apply_terminal_input(&mut app, &terminal_input, next_event)?;
            if app.should_quit {
                break;
            }
            let input_us = input_started.map_or(0, |start| start.elapsed().as_micros());
            let advance_started = frame_timing.as_ref().map(|_| Instant::now());
            app.advance();
            let advance_us = advance_started.map_or(0, |start| start.elapsed().as_micros());
            let settle_started = frame_timing.as_ref().map(|_| Instant::now());
            if app.take_attention_request() {
                let _ = crate::term::write_attention_signal(&mut std::io::stdout());
            }
            app.bag.drain_discovered();
            if backplane_refreshed_at.elapsed() >= BACKPLANE_REFRESH {
                app.tools.refresh_backplane(&app.bag);
                backplane_refreshed_at = Instant::now();
            }
            if app.thinking.is_none() && app.pending_turn.is_none() {
                app.bag.settle_brain();
            }
            if app.take_redraw_request() {
                terminal.clear()?;
            }
            let settle_us = settle_started.map_or(0, |start| start.elapsed().as_micros());
            let draw_started = frame_timing.as_ref().map(|_| Instant::now());
            terminal.draw(|frame| {
                let ui_broker = std::sync::Arc::clone(&app.ui_broker);
                let prepared =
                    crate::ui_inspect::prepare_next_capture(&mut app, ui_broker.as_ref());
                ui(frame, &mut app);
                if let Some(prepared) = prepared {
                    crate::ui_inspect::capture_after_draw(
                        &app,
                        frame,
                        ui_broker.as_ref(),
                        prepared,
                    );
                }
            })?;
            let post_draw_started =
                if let (Some(timing), Some(start)) = (&mut frame_timing, draw_started) {
                    Some(timing.completed_with_boundaries(
                        start,
                        Instant::now(),
                        frame_timing::Phases {
                            poll_us,
                            input_us,
                            advance_us,
                            settle_us,
                        },
                        post_draw_us,
                        pre_poll_us,
                    ))
                } else {
                    None
                };
            painted_once = true;
            app.flush_clipboard();
            post_draw_finished = frame_timing.as_ref().map(|_| Instant::now());
            post_draw_us = post_draw_started
                .zip(post_draw_finished)
                .map_or(0, |(start, end)| end.duration_since(start).as_micros());
        }
        Ok(())
    })();
    app.flush_pending_atlas_harvest();
    finish_exit_results(loop_result, flush_session_for_exit(&app.session))?;
    eprintln!("RUN: loop exited cleanly (should_quit={})", app.should_quit);
    Ok(app
        .reborn_exec
        .take()
        .map(|exe| (exe, app.session.id.clone())))
}

#[cfg(test)]
fn seed_preview_app() -> App {
    // Hermetic tests: unset, ANGEL_LSP auto-enables when a real language
    // server is on PATH and pre-warms it against the test workspace — a live
    // rust-analyzer flychecking a scratch worktree raced `/self integrate`'s
    // worktree removal into a coin-flip test. Tests that exercise the LSP set
    // the knob themselves.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_LSP", "0") };
    App::preview(Viewer::new())
}

fn write_private_export(path: &str, value: &serde_json::Value) -> std::io::Result<()> {
    use std::io::Write;

    let mut body = serde_json::to_vec_pretty(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    body.push(b'\n');
    if path == "-" {
        std::io::stdout().write_all(&body)?;
        return Ok(());
    }
    let path = std::path::Path::new(path);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(path)?;
    let write_result = file.write_all(&body).and_then(|()| file.sync_all());
    #[cfg(unix)]
    let write_result = write_result.and_then(|()| {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
    });
    if let Err(error) = write_result {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod rollout_export_output_tests {
    use super::write_private_export;

    #[test]
    fn explicit_export_path_is_private_and_never_overwritten() {
        let path = std::env::temp_dir().join(format!(
            "angel-rollout-export-{}-{}.json",
            std::process::id(),
            crate::cut::sha256_hex(b"private-export-fixture")
        ));
        let _ = std::fs::remove_file(&path);
        write_private_export(
            path.to_str().unwrap(),
            &serde_json::json!({"schema": "fixture/v1"}),
        )
        .unwrap();
        assert!(write_private_export(path.to_str().unwrap(), &serde_json::json!({})).is_err());
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&std::fs::read(&path).unwrap()).unwrap()["schema"],
            "fixture/v1"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let _ = std::fs::remove_file(path);
    }
}

const CLI_HELP: &str = r#"angel0 — terminal cockpit and competition/RL task runner

USAGE
  angel [--yolo] [--turbo|--comp]                     Open the terminal cockpit
  angel [--yolo] [--turbo|--comp] --task-json [OPTIONS] [PROMPT|-]
  angel [--yolo] [--turbo|--comp] --task [OPTIONS] [PROMPT|-]
  angel --export-harness-rollout [--workspace DIR] <ID|--all> <PATH|->
  angel --audit-harness-rollout [--workspace DIR] <ID> <PATH|->
  angel --bind-graph-reward [--workspace DIR] <EPISODE_ID> <TRACE_ID> <REWARD_JSON>
                            --source SRC --contract CTR
  angel --build-info --json
  angel --atlas --workspace DIR JSON
  angel --look-image IMAGE QUESTION
  angel --watch-fixture [PATH]
  angel --yukon-status BENCHMARK_UUID SUBMISSION_UUID
  angel --jev-json                 Read jev_decide JSON from stdin
  angel --dump-rl-preview <branch|research|sankey> [PATH|-] [WxH]
  angel --dump-research-preview <story|ledger|flow> [PATH|-] [WxH]

TASK OPTIONS
  --sandbox-profile sealed Mandatory isolated Linux task sandbox (incompatible with YOLO)
  --workspace DIR          Confined task workspace
  --task-id ID             Stable evaluator task identity
  --run-id ID              Stable trial/cohort identity
  --driver NAME            Route/driver selection for this invocation
  --reasoning-effort LEVEL Provider-supported reasoning level
  --task-pace PACE         auto, rapid, or deep challenge cadence
  --max-hops N             Tool-hop ceiling (default 0, unbounded); positive values opt in
  --deadline-secs N        Wall-clock ceiling (default 0, unbounded); positive values opt in
  --tool-profile PROFILE   auto, essential (or lean), or full
  --rollout MODE           off, shadow metadata, or local semantic capture
  --require-rollout        Fail the turn if requested capture cannot be sealed
  --require-rendered-output  Record unverified rendered-output acceptance (does not certify frames)
  --                       End options; required when PROMPT begins with '-'

  --watch-fixture [PATH]   Drive the built-in submission watcher from a fixture
                           status source (no live API). Omitting PATH uses the
                           shipped built-in fixture. Prints the model-visible
                           terminal injection (id + status + score/reason).

PROMPT may be one quoted argument or stdin when omitted or '-'. Use --task-json
for automation: stdout is exactly one angel.task_result/v1 JSON object and
diagnostics go to stderr. A completed policy answer is not itself a reward;
score it only with an evaluator-owned verifier and an audited rollout.

EXIT STATUS
  0  Answer completed
  1  Provider/runtime/workspace failure
  2  Invalid invocation or empty prompt
  3  Guarded stop without an answer (strict task mode, the default)

Run the source launcher as `bin/angel0 ...` or `bin/AngelTurbo ...`; release installs expose `angel` and `AngelTurbo`.
"#;

fn print_cli_help() {
    print!("{CLI_HELP}");
}

/// Activate the explicit headless sandbox profile before any task work.
fn activate_sealed_sandbox_profile(task_args: &harness::TaskCliArgs, workspace: &std::path::Path) {
    // ANGEL_SANDBOX_PROFILE=sealed and --sandbox-profile sealed select the
    // named S02 posture. Failure is a hard start-up stop, never a silent
    // fall-back to permissive.
    let requested = task_args
        .sandbox_profile
        .clone()
        .or_else(|| std::env::var("ANGEL_SANDBOX_PROFILE").ok())
        .filter(|value| !value.trim().is_empty());
    let Some(name) = requested else { return };
    if name != "sealed" {
        eprintln!("angel: unknown sandbox profile {name:?} (supported: sealed)");
        std::process::exit(2);
    }
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let profile = crate::sandbox::sealed::build(workspace, home.as_deref());
    if let Err(reason) = crate::sandbox::sealed::activate(profile) {
        eprintln!("angel: sealed sandbox profile unavailable: {reason}");
        std::process::exit(2);
    }
}

/// Apply typed `--task` flags before installing headless defaults.
///
/// Ordering is load-bearing: `apply_task_runtime_defaults` derives the
/// Treebeard lane from the hop ceiling and release builds cache that ceiling.
/// Installing `--max-hops` afterward used to leave both the lane and the
/// effective turn guard pinned to the default 64 even though the JSON receipt
/// claimed an invocation-local CLI layer.
fn apply_task_cli_runtime_overrides(task_args: &harness::TaskCliArgs) {
    if let Some(driver) = task_args.driver.as_deref() {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_DRIVER", driver) };
    }
    if let Some(effort) = task_args.reasoning_effort.as_deref() {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_REASONING_EFFORT", effort) };
    }
    if let Some(pace) = task_args.task_pace.as_deref() {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_TASK_PACE", pace) };
    }
    if let Some(max_hops) = task_args.max_hops {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_MAX_HOPS", max_hops.to_string()) };
    }
    if let Some(deadline_secs) = task_args.deadline_secs {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_TURN_DEADLINE_SECS", deadline_secs.to_string()) };
    }
    if let Some(profile) = task_args.tool_profile.as_deref() {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_TOOL_SCHEMA_PROFILE", profile) };
    }
    if let Some(capture) = task_args.rollout_capture.as_deref() {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_HARNESS_ROLLOUTS", capture) };
    }
    if task_args.require_rollout {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_HARNESS_ROLLOUT_REQUIRED", "1") };
    }
    if task_args.require_rendered_output {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_TASK_RENDERED_REQUIREMENT", "1") };
    }
}

/// `angel --bind-graph-reward` — the external scorer's binding surface. Prints
/// the binding receipt JSON to stdout on success; all failures exit non-zero
/// with a diagnostic on stderr. This is the only CLI path that may attach
/// training reward to an agent-graph trace.
fn bind_graph_reward_command(mut args: impl Iterator<Item = String>) -> std::io::Result<()> {
    let first = args.next().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "--bind-graph-reward requires [--workspace DIR] an episode id",
        )
    })?;
    let (workspace, episode_id) = if first == "--workspace" {
        let workspace = args.next().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "--bind-graph-reward --workspace requires a directory",
            )
        })?;
        let episode = args.next().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "--bind-graph-reward requires an episode id",
            )
        })?;
        (PathBuf::from(workspace), episode)
    } else {
        (std::env::current_dir()?, first)
    };
    let trace_id = args.next().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "--bind-graph-reward requires a trace id",
        )
    })?;
    let reward_json = args.next().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "--bind-graph-reward requires a reward JSON value",
        )
    })?;
    let mut source = None;
    let mut contract = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--source" => {
                source = Some(args.next().ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "--source requires a value",
                    )
                })?);
            }
            "--contract" => {
                contract = Some(args.next().ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "--contract requires a value",
                    )
                })?);
            }
            other => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("--bind-graph-reward: unknown argument {other}"),
                ));
            }
        }
    }
    let source = source.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "--bind-graph-reward requires --source SRC",
        )
    })?;
    let contract = contract.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "--bind-graph-reward requires --contract CTR",
        )
    })?;
    let reward: serde_json::Value = serde_json::from_str(&reward_json).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("reward JSON does not parse: {error}"),
        )
    })?;
    let binding = harness::bind_episode_reward(
        &workspace,
        &episode_id,
        &trace_id,
        reward,
        &source,
        &contract,
    )
    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    println!(
        "{}",
        serde_json::to_string_pretty(&binding).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("encode binding receipt: {error}"),
            )
        })?
    );
    Ok(())
}

/// The capabilities `--build-info --json` advertises. The coding-harness
/// contract stage (`benchmarks/action-agent/run-contract-stage.mjs`) requires
/// a fixed subset; the drift test below pins that subset so a capability can
/// never silently disappear from the contract again.
pub(crate) const BUILD_CAPABILITIES: &[&str] = &[
    "task-json/v1",
    "task-acceptance-proof/v1",
    "task-runtime-config/v1",
    "task-rollout-binding/v1",
    "external-verifier/v1",
    "harness-rollout-ref/v1",
    "rollout-audit-receipt/v1",
    "required-rollout/v1",
    "finite-task-defaults/v1",
    "agent-graph-episode/v1",
    "agent-graph-reward-binding/v1",
    "scoreable-max-hops/v1",
    "final-mile/v1",
    "yolo/v1",
    "jev-decisions/v1",
    "repeated-poll-stop/v1",
    "benchmark-comparison/v1",
];

fn main() -> std::io::Result<()> {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--atlas")
    {
        return atlas::cli(std::env::args_os().skip(2));
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--look-image")
    {
        return tools::vision::image_cli(std::env::args_os().skip(2));
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--tool-http-helper")
    {
        return tools::http_transport::helper_main();
    }
    let process_started = std::time::Instant::now();
    // `--yolo` is a global leading flag for both the TUI and every headless
    // entrypoint. The launcher consumes it too, but the binary supports direct
    // invocation so automation does not depend on the shell wrapper.
    let raw_os_args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if raw_os_args
        .first()
        .is_some_and(|arg| arg == std::ffi::OsStr::new("--sandbox-exec"))
    {
        return sandbox::exec_helper(raw_os_args.into_iter().skip(1));
    }
    #[cfg(target_os = "linux")]
    if raw_os_args.iter().any(|arg| arg == "--doctor") {
        println!("{}", sandbox::compatibility::doctor());
        return Ok(());
    }
    sandbox::process_owner::initialize()?;
    extern "C" fn finish_background_processes() {
        tools::proc::stop_all_owned();
    }
    // atexit callbacks run in reverse order: persist process outcomes before
    // the generic descendant reaper consumes remaining wait statuses.
    unsafe {
        libc::atexit(finish_background_processes);
    }
    // Hash the executable for the run identity off the request path: the
    // first model request must never wait on it (E03 binds before launch,
    // P04 budgets the first request).
    harness::run_identity::prewarm();
    if raw_os_args
        .first()
        .is_some_and(|arg| arg == "--audit-coding-eval-training")
    {
        if raw_os_args.len() != 3 || raw_os_args[1] != "--store" {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "--audit-coding-eval-training requires exactly --store OPERATOR_ROOT",
            ));
        }
        return reinforce::training::audit_cli(std::path::Path::new(&raw_os_args[2]));
    }
    // Pin the sandbox-helper path while /proc/self/exe is still clean: a
    // release rebuild under a live session renames the binary and would
    // otherwise turn every later tool spawn into ENOENT (os error 2).
    sandbox::prime_helper();
    #[cfg(target_os = "linux")]
    let _ = sandbox::compatibility::detect();
    let mut raw_args = raw_os_args
        .into_iter()
        .map(|arg| {
            arg.into_string().map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "ordinary Angel arguments must be valid UTF-8",
                )
            })
        })
        .collect::<std::io::Result<Vec<_>>>()?;
    if raw_args.first().is_some_and(|arg| arg == "--yolo") {
        yolo::set(true);
        raw_args.remove(0);
    }
    if raw_args.first().is_some_and(|arg| {
        arg == "--comp" || arg == "--lean" || arg == "--turbo" || arg == "--angelturbo"
    }) {
        comp_mode::set(true);
        raw_args.remove(0);
    }
    if raw_args.as_slice() == ["--help"] || raw_args.as_slice() == ["-h"] {
        print_cli_help();
        return Ok(());
    }
    if raw_args.as_slice() == ["--version"] || raw_args.as_slice() == ["-V"] {
        println!("angel0-cockpit {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if yolo::enabled() {
        eprintln!("[angel] {}", yolo::status_text());
    }
    if comp_mode::enabled() {
        eprintln!("[angel] {}", comp_mode::status_text());
    }
    let mut args = raw_args.into_iter();
    let mut resume: Option<Option<String>> = None;
    if let Some(arg) = args.next() {
        if arg == "--build-info" {
            if args.next().as_deref() != Some("--json") || args.next().is_some() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "--build-info requires exactly --json",
                ));
            }
            let build = harness::run_identity::static_identity().map_err(std::io::Error::other)?;
            println!(
                "{}",
                serde_json::json!({
                    "schema": "angel-build-info/v1",
                    "executable_sha256": build.executable_sha256,
                    "executable_path": build.executable_path,
                    "toolchain": build.toolchain,
                    "package_version": env!("CARGO_PKG_VERSION"),
                    "cockpit_source_sha256": build.cockpit_source_sha256,
                    "capabilities": BUILD_CAPABILITIES,
                    "video_decode": cfg!(feature = "scryglass-video"),
                })
            );
            return Ok(());
        }
        if arg == "--bind-graph-reward" {
            return bind_graph_reward_command(args);
        }
        if arg == "--jev-json" {
            if args.next().is_some() {
                return Err(std::io::Error::other(
                    "--jev-json reads JSON from stdin; no arguments",
                ));
            }
            return tools::jev::run_cli().map_err(std::io::Error::other);
        }
        if arg == "--yukon-status" {
            let benchmark = args.next().unwrap_or_default();
            let submission = args.next().unwrap_or_default();
            if args.next().is_some() {
                return Err(std::io::Error::other(
                    "--yukon-status requires exactly two UUIDs",
                ));
            }
            return yukon_status::run_cli(&benchmark, &submission).map_err(std::io::Error::other);
        }
        if arg == "--watch-fixture" {
            let path = args.next();
            if args.next().is_some() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "--watch-fixture accepts at most one fixture path",
                ));
            }
            harness::run_watch_fixture_cli(path.as_deref().map(std::path::Path::new))?;
            return Ok(());
        }
        if arg == "--export-harness-rollout" || arg == "--audit-harness-rollout" {
            let audit_only = arg == "--audit-harness-rollout";
            let command = arg;
            let first = args.next().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("{command} requires [--workspace DIR] a rollout ID"),
                )
            })?;
            let (workspace, target) = if first == "--workspace" {
                let workspace = args.next().ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("{command} --workspace requires a directory"),
                    )
                })?;
                let target = args.next().ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("{command} requires a rollout ID"),
                    )
                })?;
                (PathBuf::from(workspace), target)
            } else {
                (std::env::current_dir()?, first)
            };
            let output = args.next().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("{command} requires an explicit output path or -"),
                )
            })?;
            if args.next().is_some() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("{command} accepts exactly a target and output"),
                ));
            }
            if audit_only && target == "--all" {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "--audit-harness-rollout requires one exact rollout ID",
                ));
            }
            let value = if audit_only {
                harness::audit_workspace_rollout_receipt(&workspace, &target)
            } else if target == "--all" {
                harness::export_workspace_rollout_corpus_v2(&workspace)
            } else {
                harness::export_workspace_rollout_v2(&workspace, &target)
            }
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
            write_private_export(&output, &value)?;
            return Ok(());
        }
        // `--resume [id]`: start the TUI on a saved session (no id = latest).
        // Written by the phoenix step so a reborn self continues its life.
        if arg == "--resume" {
            resume = Some(args.next().filter(|a| !a.starts_with('-')));
        }
        if arg == "--dump-preview" || arg == "--dump-preview-portrait" {
            let include_portrait = arg == "--dump-preview-portrait";
            let path = args.next().unwrap_or_else(|| "-".to_string());
            // Optional WxH (default 144x48) so the disconnected-panel layout can be
            // dumped headlessly at any representative size (e.g. 160x48, 90x30).
            let (w, h) = args
                .next()
                .and_then(|s| {
                    let (a, b) = s.split_once(['x', 'X'])?;
                    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
                })
                .unwrap_or((144u16, 48u16));
            let text = if include_portrait {
                render_portrait_preview_text(w, h)?
            } else {
                render_preview_text(w, h)?
            };
            if path == "-" {
                print!("{text}");
            } else {
                std::fs::write(path, text)?;
            }
            return Ok(());
        }
        if arg == "--dump-research-preview" {
            let lens = match args.next().unwrap_or_default().as_str() {
                "story" => crate::research_workspace::Lens::Story,
                "ledger" => crate::research_workspace::Lens::Ledger,
                "flow" => crate::research_workspace::Lens::Flow,
                _ => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "expected story, ledger, or flow",
                    ));
                }
            };
            let path = args.next().unwrap_or_else(|| "-".into());
            let (width, height) = args
                .next()
                .and_then(|size| {
                    let (width, height) = size.split_once(['x', 'X'])?;
                    Some((width.parse().ok()?, height.parse().ok()?))
                })
                .unwrap_or((144u16, 48u16));
            let text = draw::render_research_preview_text(lens, width, height)?;
            if path == "-" {
                print!("{text}");
            } else {
                std::fs::write(path, text)?;
            }
            return Ok(());
        }
        if arg == "--dump-rl-preview" {
            let view_name = args.next().unwrap_or_default();
            let view = crate::rl_viz::RlView::parse(&view_name).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "--dump-rl-preview requires branch, research, or sankey",
                )
            })?;
            let path = args.next().unwrap_or_else(|| "-".to_string());
            let (width, height) = args
                .next()
                .and_then(|size| {
                    let (width, height) = size.split_once(['x', 'X'])?;
                    Some((width.trim().parse().ok()?, height.trim().parse().ok()?))
                })
                .unwrap_or((144u16, 48u16));
            let text = render_rl_preview_text(view, width, height)?;
            if path == "-" {
                print!("{text}");
            } else {
                std::fs::write(path, text)?;
            }
            return Ok(());
        }
        if arg == "--dump-scryglass" {
            let path = args.next().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "--dump-scryglass requires a local image or MP4 path",
                )
            })?;
            let size = args.next().unwrap_or_else(|| "144x48".to_string());
            let (w, h) = size
                .split_once(['x', 'X'])
                .and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?)))
                .unwrap_or((144, 48));
            print!("{}", render_scryglass_preview_text(&path, w, h)?);
            return Ok(());
        }
        if arg == "--dump-visual" {
            let scene = args.next().unwrap_or_default();
            let elapsed_ms = args
                .next()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0);
            let size = args.next().unwrap_or_else(|| "96x30".to_string());
            let (width, height) = size
                .split_once(['x', 'X'])
                .and_then(|(w, h)| Some((w.parse::<u16>().ok()?, h.parse::<u16>().ok()?)))
                .ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "angel --dump-visual: size must be WxH",
                    )
                })?;
            let output = args.next().unwrap_or_else(|| "-".to_string());
            let value = visual_export::export(&scene, elapsed_ms, width, height)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
            let json =
                serde_json::to_string_pretty(&value).expect("visual export value must serialize");
            if output == "-" {
                println!("{json}");
            } else {
                let path = std::path::PathBuf::from(output);
                if let Some(parent) = path
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(path, format!("{json}\n"))?;
            }
            return Ok(());
        }
        if arg == "--loop" || arg == "--rollout" {
            let bag = Bag::standard();
            check_headless_route(
                bag.in_hand().as_ref(),
                &harness::TaskCliArgs::default(),
                &std::env::current_dir()?,
            );
            eprintln!(
                "angel: unsupported standalone headless option {arg}; use --task or --task-json (--rollout accepts off, shadow, local)"
            );
            std::process::exit(2);
        }
        if arg == "--ask" {
            // Headless one-shot: send a single prompt to the in-hand club (the
            // driver ANGEL_DRIVER selects — e.g. the swarm) and print its reply to
            // stdout, no TUI. The prompt is the next arg, or stdin when that arg is
            // absent or `-`. Only the reply lands on stdout (diagnostics go to
            // stderr) so a bench harness can capture it cleanly. Exit 0 on success,
            // 1 on a club error, 2 on an empty prompt.
            let prompt = match args.next() {
                Some(p) if p != "-" => p,
                _ => {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
                    buf
                }
            };
            let prompt = prompt.trim();
            if prompt.is_empty() {
                eprintln!("angel --ask: empty prompt (pass it as an arg or on stdin)");
                std::process::exit(2);
            }
            let bag = Bag::standard();
            let club = bag.in_hand();
            check_headless_route(
                club.as_ref(),
                &harness::TaskCliArgs::default(),
                &std::env::current_dir()?,
            );
            eprintln!(
                "[angel --ask] driver in hand: {}  {}",
                club.label(),
                club::model_defaults::summary(club.as_ref())
            );
            match club.respond(prompt) {
                Ok(reply) => {
                    print!("{reply}");
                    if !reply.ends_with('\n') {
                        println!();
                    }
                    return Ok(());
                }
                Err(e) => {
                    eprintln!("angel --ask: {e}");
                    std::process::exit(1);
                }
            }
        }
        if arg == "--board-sync" {
            // Headless competition board observation: open the flywheel state
            // dir, engage the board reducer over an in-memory raw journal, and
            // bind official fire results through the reward join. Stdout is
            // exactly ONE angel.board-sync/v1 JSON object; typed failures
            // (including phantom fires) ride in the object with exit 0. Exit 2
            // only when the state dir itself is unreadable (diagnostic on
            // stderr, stdout stays clean for parsers).
            let state_dir = args.next().unwrap_or_default();
            let report = match competition::board_sync::run(std::path::Path::new(&state_dir)) {
                Ok(report) => report,
                Err(failure) => {
                    eprintln!(
                        "angel --board-sync: state dir unreadable: {}",
                        serde_json::to_string(&failure).unwrap_or_else(|_| format!("{failure:?}"))
                    );
                    std::process::exit(2);
                }
            };
            println!("{report}");
            return Ok(());
        }
        if arg == "--task" || arg == "--task-json" {
            // Headless AGENTIC turn: like --ask, but with the full tool loop
            // (run_turn + ToolRegistry::with_team) so the agent actually reads,
            // edits, and runs inside a workspace. For coding/agentic benchmarks.
            // The in-hand club must be a tool-calling model — set ANGEL_DRIVER to a
            // fleet label (spark/gemma/atlas); the text-only swarm won't call tools.
            // Workspace: --workspace DIR, else ANGEL_WORKSPACE, else the default.
            // Bounded by the usual guardians (ANGEL_MAX_HOPS, ANGEL_TURN_DEADLINE_SECS).
            // `--task-json` is accepted as either the entrypoint or a modifier:
            // `angel --task-json --task ...` and `angel --task --task-json ...`.
            harness::begin_task_lifecycle(process_started);
            let startup_started = std::time::Instant::now();
            let task_args = match harness::parse_task_args(&arg, args) {
                Ok(parsed) => parsed,
                Err(failure) => {
                    if failure.parsed.json {
                        let workspace = harness::resolve_workspace(
                            failure.parsed.workspace.clone(),
                            harness::default_workspace,
                        );
                        let envelope = harness::TaskJsonEnvelope::from_startup_failure(
                            &failure.parsed,
                            workspace,
                            startup_started.elapsed().as_millis(),
                            harness::TaskStartupStopReason::InvalidArguments,
                            failure.message,
                        );
                        println!(
                            "{}",
                            serde_json::to_string(&envelope)
                                .expect("task result envelope must serialize")
                        );
                    } else {
                        eprintln!("angel {arg}: {failure}");
                    }
                    std::process::exit(2);
                }
            };
            let workspace =
                harness::resolve_workspace(task_args.workspace.clone(), harness::default_workspace);
            let prompt = match task_args.prompt.clone() {
                Some(p) => p,
                None => {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
                    buf
                }
            };
            let prompt = prompt.trim();
            if prompt.is_empty() {
                let message = "empty prompt (pass it as an arg or on stdin)";
                if task_args.json {
                    let envelope = harness::TaskJsonEnvelope::from_startup_failure(
                        &task_args,
                        workspace,
                        startup_started.elapsed().as_millis(),
                        harness::TaskStartupStopReason::EmptyPrompt,
                        message.to_string(),
                    );
                    println!(
                        "{}",
                        serde_json::to_string(&envelope)
                            .expect("task result envelope must serialize")
                    );
                } else {
                    eprintln!("angel --task: {message}");
                }
                std::process::exit(2);
            }
            harness::run_identity::configure_dataset(
                harness::run_identity::Dataset::new(
                    if task_args.json { "task_json" } else { "task" },
                    None,
                )
                .map_err(std::io::Error::other)?,
            );
            let operator_prompt_sha256 = crate::cut::sha256_hex(prompt.as_bytes());
            // Same precedence as the TUI (--workspace > $ANGEL_WORKSPACE > fallback),
            // but `--task` keeps the isolated `~/.angel0/workspace` fallback that
            // benchmark harnesses rely on rather than defaulting to the cwd.
            if let Err(e) = std::fs::create_dir_all(&workspace) {
                let message = format!("cannot create workspace {}: {e}", workspace.display());
                if task_args.json {
                    let envelope = harness::TaskJsonEnvelope::from_startup_failure(
                        &task_args,
                        workspace,
                        startup_started.elapsed().as_millis(),
                        harness::TaskStartupStopReason::WorkspaceError,
                        message,
                    );
                    println!(
                        "{}",
                        serde_json::to_string(&envelope)
                            .expect("task result envelope must serialize")
                    );
                } else {
                    eprintln!("angel --task: {message}");
                }
                std::process::exit(1);
            }
            // Typed invocation-local flags must land before defaults: the
            // defaults derive lane policy from the hop ceiling and release
            // builds cache that read. Explicit operator/evaluator values retain
            // precedence, and interactive TUI turns never enter this branch.
            activate_sealed_sandbox_profile(&task_args, &workspace);
            apply_task_cli_runtime_overrides(&task_args);
            // Ordinary headless coding uses the same bounded anti-stall policy
            // as the Prime adapter.
            let task_pace = harness::apply_task_runtime_defaults(prompt);
            // Bounded task runs automatically use the discoverable essential
            // schema set. Explicit `full` and `essential` remain reproducible
            // overrides; `auto` is the ordinary task-aware default.
            let tool_schema_profile = std::env::var("ANGEL_TOOL_SCHEMA_PROFILE")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .map(|value| value.trim().to_ascii_lowercase())
                .filter(|value| matches!(value.as_str(), "auto" | "essential" | "lean" | "full"))
                .unwrap_or_else(|| "auto".to_string());
            // Public task mode never executes an operator/model-visible shell
            // command as a verifier. A parent process cannot re-arm the legacy
            // in-turn acceptance hook; reward belongs to the evaluator-owned,
            // read-only proof boundary after the candidate process exits.
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::remove_var("ANGEL_TASK_ACCEPT_CMD") };
            let mut bag = Bag::standard();
            let explicit_driver = task_args
                .driver
                .clone()
                .or_else(|| std::env::var("ANGEL_DRIVER").ok())
                .filter(|driver| !driver.trim().is_empty());
            let club = if let Some(requested) = explicit_driver.as_deref() {
                match bag.require_task_driver(requested) {
                    Ok(club) => club,
                    Err(reason) => {
                        let message = format!("requested task driver is not available: {reason:?}");
                        if task_args.json {
                            let envelope = harness::TaskJsonEnvelope::from_startup_failure(
                                &task_args,
                                workspace,
                                startup_started.elapsed().as_millis(),
                                harness::TaskStartupStopReason::DriverUnavailable,
                                message,
                            );
                            println!(
                                "{}",
                                serde_json::to_string(&envelope)
                                    .expect("task result envelope must serialize")
                            );
                        } else {
                            eprintln!("angel --task: {message}");
                        }
                        std::process::exit(2);
                    }
                }
            } else {
                bag.in_hand()
            };
            if let Some(requested) = task_args.reasoning_effort.as_deref()
                && club.set_reasoning_effort(requested).is_none()
            {
                let message = format!(
                    "requested reasoning effort is not available for {}: {requested}",
                    club.label()
                );
                if task_args.json {
                    let envelope = harness::TaskJsonEnvelope::from_startup_failure(
                        &task_args,
                        workspace,
                        startup_started.elapsed().as_millis(),
                        harness::TaskStartupStopReason::DriverUnavailable,
                        message,
                    );
                    println!(
                        "{}",
                        serde_json::to_string(&envelope)
                            .expect("task result envelope must serialize")
                    );
                } else {
                    eprintln!("angel --task: {message}");
                }
                std::process::exit(2);
            }
            check_headless_route(club.as_ref(), &task_args, &workspace);
            let roster = bootstrap::delegation_roster(&bag);
            eprintln!(
                "[angel --task] driver in hand: {}  workspace: {}  tool schemas: {}  {}",
                club.label(),
                workspace.display(),
                tool_schema_profile,
                club::model_defaults::summary(club.as_ref())
            );
            // Build the agent's system prompt + toolbelt, mirroring the TUI App.
            let specialists: Vec<String> = roster
                .iter()
                .map(|c| c.label().to_string())
                .filter(|l| l != club.label() && l != "mock")
                .collect();
            harness::note_task_startup_phase(
                "identity_prewarm_ms",
                process_started.elapsed().as_millis(),
            );
            let project_doc_started = std::time::Instant::now();
            let skills = harness::load_skills_for(&workspace);
            let vision_hint = crate::tools::vision::vision_sidecar_prompt_hint(club.as_ref());
            let mut history = bootstrap::build_task_history(
                &specialists,
                &skills,
                &workspace,
                vision_hint.as_deref(),
            );
            harness::note_task_startup_phase(
                "project_doc_ms",
                project_doc_started.elapsed().as_millis(),
            );
            let registry_started = std::time::Instant::now();
            let mut registry = harness::ToolRegistry::with_team_self(
                workspace.clone(),
                roster,
                Some(Arc::clone(&club)),
            );
            tools::self_model::SelfMapTool::register_headless(&mut registry);
            registry.set_skill_index(&skills);
            let skill_hint = registry.relevant_skill_hint(prompt);
            if !skills.is_empty() {
                registry.register(Box::new(harness::SkillTool::for_workspace(
                    skills,
                    workspace.clone(),
                )));
            }
            let (lsp_tools, _lsp_notes) = lsp::discover_lsp_tools(workspace.clone());
            for t in lsp_tools {
                registry.register(t);
            }
            // Headless action profiles can advertise the lean core without
            // making the remaining built-ins or LSP tools undiscoverable.
            registry.enable_tool_search();
            harness::note_task_startup_phase("registry_ms", registry_started.elapsed().as_millis());
            let recon_started = std::time::Instant::now();
            let task_recon = harness::task_recon_message(&registry, prompt);
            harness::note_task_startup_phase(
                "workspace_recon_ms",
                recon_started.elapsed().as_millis(),
            );
            let prompt = match skill_hint {
                Some(hint) => format!("{hint}{prompt}"),
                None => prompt.to_string(),
            };
            let atlas_query = prompt.clone();
            // Fresh-session handoff pickup: the previous run's persisted agent-
            // authored brief enters as an Assistant-role handoff snapshot — the
            // same carrier compaction uses, so it survives future compactions
            // natively and never outranks the operator's request below it.
            // `ANGEL_TASK_HANDOFF_WARM=0` opts out (e.g. deliberately-cold ablations).
            let handoff_warm = std::env::var("ANGEL_TASK_HANDOFF_WARM")
                .map(|value| value.trim() != "0")
                .unwrap_or(true);
            if handoff_warm
                && let Some(note) = crate::tools::plan::load_workspace_handoff(&workspace)
            {
                eprintln!(
                    "[angel --task] warm start: prior-session handoff loaded ({} chars)",
                    note.chars().count()
                );
                history.push(ChatMsg::assistant(format!(
                    "{}{note}",
                    crate::compaction::HANDOFF_SNAPSHOT_PREFIX
                )));
            }
            history.push(ChatMsg::user(prompt));
            if std::env::var("ANGEL_RELENTLESS_EXECUTION")
                .map(|v| v != "0")
                .unwrap_or(false)
                && let Some(user) = history.last_mut()
            {
                user.content = format!(
                    "{}\n\n{}",
                    harness::RELENTLESS_EXECUTION_DIRECTIVE,
                    user.content
                )
                .into();
            }
            let atlas = registry.atlas();
            if atlas.enabled() {
                let lens =
                    atlas.build_lens(&atlas_query, std::iter::once(history[0].content.as_ref()));
                crate::atlas::replace_lens_message(&mut history, lens);
            }
            if let Some(recon) = task_recon {
                history.push(recon);
            }
            let cancel = std::sync::atomic::AtomicBool::new(false);
            let max_hops = harness::configured_max_hops();
            let competition = harness::competition_mode_active(&history);
            let deadline_secs = harness::configured_turn_deadline_secs_for(competition);
            let rollout_capture = match std::env::var("ANGEL_HARNESS_ROLLOUTS")
                .unwrap_or_else(|_| "off".to_string())
                .trim()
                .to_ascii_lowercase()
                .as_str()
            {
                "shadow" => "shadow",
                "local" => "local",
                _ => "off",
            }
            .to_string();
            let rollout_required = std::env::var("ANGEL_HARNESS_ROLLOUT_REQUIRED")
                .map(|value| {
                    !matches!(
                        value.trim().to_ascii_lowercase().as_str(),
                        "" | "0" | "false" | "off" | "no"
                    )
                })
                .unwrap_or(false);
            let runtime = harness::TaskRuntimeConfig::new(
                operator_prompt_sha256,
                task_args.driver.clone(),
                task_args.reasoning_effort.clone(),
                task_pace,
                max_hops,
                deadline_secs,
                tool_schema_profile.clone(),
                rollout_capture,
                rollout_required,
                yolo::enabled(),
            );
            let rollout_binding =
                runtime.rollout_binding(task_args.task_id.clone(), task_args.run_id.clone());
            let started = std::time::Instant::now();
            let usage_before = club.usage_accounting();
            // Drain tool-call events so run_turn's sends never block; the final
            // answer alone goes to stdout for the grader to capture. The TUI's
            // live activity trace has no seat in a headless run, so tool
            // activity and harness notices (guard nudges, compaction, club
            // fallbacks) surface on stderr instead of vanishing — an operator
            // tailing the log can see WHY a turn stopped or slowed, not just
            // that it did. High-frequency stream deltas stay dropped.
            // `ANGEL_TASK_EVENT_LOG=0` silences routine events; capture errors
            // remain visible and are retained in the JSON envelope.
            let (event_tx, event_rx) = mpsc::channel::<harness::TurnEvent>();
            let task_event_log = std::env::var("ANGEL_TASK_EVENT_LOG")
                .map(|value| value.trim() != "0")
                .unwrap_or(true);
            let drain = std::thread::spawn(move || {
                let mut capture_errors = Vec::new();
                for event in event_rx.iter() {
                    if let harness::TurnEvent::RolloutCaptureError(error) = event {
                        eprintln!("[task-event] error: {error}");
                        capture_errors.push(error);
                        continue;
                    }
                    if !task_event_log {
                        continue;
                    }
                    match event {
                        harness::TurnEvent::ToolCall {
                            name, args_summary, ..
                        } => eprintln!("[task-event] call {name}: {args_summary}"),
                        harness::TurnEvent::ToolResult {
                            name,
                            summary,
                            outcome,
                            ..
                        } => eprintln!("[task-event] done {name} [{outcome:?}]: {summary}"),
                        harness::TurnEvent::Notice(note) => {
                            eprintln!("[task-event] notice: {note}")
                        }
                        _ => {}
                    }
                }
                capture_errors
            });
            let result = harness::run_task_turn_observed(
                &*club,
                &registry,
                &mut history,
                &cancel,
                max_hops,
                &event_tx,
                &rollout_binding,
                runtime.requested_driver.as_deref(),
            );
            if let Ok(answer) = &result {
                let _ = atlas.enqueue_harvest(&atlas_query, &answer.answer, &[], &[]);
            }
            drop(event_tx);
            let capture_errors = drain.join().unwrap_or_else(|_| {
                let error = "rollout capture diagnostics unavailable: task event drain panicked";
                eprintln!("[task-event] error: {error}");
                vec![error.to_string()]
            });
            let elapsed_ms = started.elapsed().as_millis();
            let usage = harness::task_usage_delta(usage_before, club.usage_accounting());
            let route_identity = club.route_identity();
            let model = route_identity.model.or_else(|| club.live_model_name());
            let reasoning_effort = route_identity
                .reasoning_effort
                .or_else(|| club.reasoning_effort());
            let club_label = club.label().to_string();
            let output_budget =
                harness::TaskOutputBudget::from_route_metadata(&club.route_metadata());
            let task_id = task_args.task_id;
            let run_id = task_args.run_id;
            let json_context = || harness::TaskJsonContext {
                task_id: task_id.clone(),
                run_id: run_id.clone(),
                workspace: workspace.clone(),
                club: Some(club_label.clone()),
                model: model.clone(),
                reasoning_effort: reasoning_effort.clone(),
                output_budget: Some(output_budget.clone()),
                elapsed_ms,
                tools: harness::tool_ledger_snapshot(),
                timing: harness::task_timing_snapshot(),
                usage: usage.clone(),
                runtime: Some(runtime.clone()),
                session_id: None,
                artifacts: Vec::new(),
                memory_health: crate::caddy::StoreHealthSummary::default(),
            };
            match result {
                Ok(outcome) => {
                    let completed = outcome.stop_reason == harness::TurnStopReason::Answer;
                    if task_args.json {
                        let mut envelope = harness::TaskJsonEnvelope::from_outcome(
                            json_context(),
                            outcome,
                            &history,
                        );
                        envelope.rollout_capture_errors = capture_errors;
                        // Sample after envelope construction; stdout emission is measured
                        // independently by the parent harness.
                        if let Some(timing) = envelope.timing.as_mut() {
                            timing.refresh_task_lifecycle();
                            envelope.elapsed_ms = timing.wall_ms;
                        }
                        println!(
                            "{}",
                            serde_json::to_string(&envelope)
                                .expect("task result envelope must serialize")
                        );
                    } else {
                        print!("{}", outcome.answer);
                        if !outcome.answer.ends_with('\n') {
                            println!();
                        }
                    }
                    if !completed && harness::task_strict_exit() {
                        std::process::exit(3);
                    }
                    return Ok(());
                }
                Err(failure) => {
                    if task_args.json
                        && failure.stop_reason == harness::TurnStopReason::CaptureFailure
                    {
                        eprintln!("angel --task: {}", failure.message);
                    }
                    if task_args.json {
                        let mut envelope =
                            harness::TaskJsonEnvelope::from_failure(json_context(), failure);
                        envelope.rollout_capture_errors = capture_errors;
                        // Sample after envelope construction; stdout emission is measured
                        // independently by the parent harness.
                        if let Some(timing) = envelope.timing.as_mut() {
                            timing.refresh_task_lifecycle();
                            envelope.elapsed_ms = timing.wall_ms;
                        }
                        println!(
                            "{}",
                            serde_json::to_string(&envelope)
                                .expect("task result envelope must serialize")
                        );
                    } else {
                        eprintln!("angel --task: {}", failure.message);
                    }
                    std::process::exit(1);
                }
            }
        }
    }

    // Calibrate graphics after entering the alternate screen but before the
    // event loop reads input. This is the safe query window required by
    // ratatui-image and lets generic SSH sessions discover their real backend.
    let mut terminal = init_terminal()?;
    let viewer = Viewer::calibrated();
    let result = run(&mut terminal, viewer, resume);
    restore_terminal();
    // The phoenix step: `/self reborn` staged a freshly-built binary — replace
    // this process with the new self, resuming the same session. Must happen
    // after terminal teardown; the new process sets the terminal up again.
    match result {
        Ok(Some((exe, session_id))) => {
            eprintln!("PHOENIX: exec {} --resume {session_id}", exe.display());
            use std::os::unix::process::CommandExt;
            let err = std::process::Command::new(&exe)
                .arg("--resume")
                .arg(&session_id)
                .exec();
            // exec only returns on failure.
            eprintln!("PHOENIX: exec failed: {err}");
            Err(err)
        }
        Ok(None) => Ok(()),
        Err(e) => Err(e),
    }
}
#[cfg(test)]
mod tests;

#[cfg(test)]
mod d06b_tests {
    use super::*;

    #[test]
    fn d06b_practice_requires_exact_opt_in() {
        for value in [None, Some("0"), Some("true"), Some(" 1")] {
            assert!(!practice_route_allowed("practice", value));
        }
        assert!(practice_route_allowed("practice", Some("1")));
        assert!(practice_route_allowed("openrouter", None));
        assert_eq!(
            interactive_practice_notice("practice"),
            Some(PRACTICE_NOTICE)
        );
        assert_eq!(interactive_practice_notice("openrouter"), None);
        assert!(PRACTICE_NOTICE.contains("interactive practice"));
        assert!(PRACTICE_NOTICE.contains("offline echo"));
    }

    #[test]
    fn d06b_no_route_envelope() {
        let value = serde_json::to_value(harness::TaskJsonEnvelope::from_startup_failure(
            &harness::TaskCliArgs::default(),
            std::path::PathBuf::from("."),
            0,
            harness::TaskStartupStopReason::NoRoute,
            NO_ROUTE.into(),
        ))
        .unwrap();
        assert_eq!(value["status"], "error");
        assert_eq!(value["error"]["kind"], "no_route");
        assert_eq!(value["hops"], 0);
        for knob in [
            "ANGEL_DRIVER",
            "ANGEL_API_CLUBS",
            "ANGEL_<DRIVER>_KEY",
            "ANGEL_<DRIVER>_URL",
            "ANGEL_<DRIVER>_MODEL",
            "ANGEL_PRACTICE=1",
        ] {
            assert!(value["error"]["message"].as_str().unwrap().contains(knob));
        }
    }
}

#[cfg(test)]
mod retained_world_tests;
