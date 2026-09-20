//! Backdrop-off tax suites (module-breakup: extracted from the `tests.rs`
//! monolith). `ANGEL_BACKDROP=off` must skip Stage/agent-bay paint and the
//! miniworld sim taxes — plus the visible-stage positive control.

use super::{render_app_text, seed_live_streaming_app, seed_preview_app};
use crate::agent::harness;
use crate::drive::loop_ctl;
use crate::tests::{TestEnvGuard, env_lock};
use crate::ui::scryglass;
use crate::ui::surfaces;
use std::time::{Duration, Instant};

/// A2: text-only mode must not paint Stage/agent bay and must not advance the
/// miniworld sim clock on the UI loop (no invisible scenery tax).
#[test]
fn backdrop_off_skips_stage_paint_and_world_sim_ticks() {
    let _lock = env_lock();
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "off");
    let mut app = seed_preview_app();
    assert!(
        !app.world_pane_visible,
        "preview must not start with an earned world pane"
    );

    let wide = render_app_text(&mut app, 144, 48);
    assert!(
        !wide.contains("Scryglass"),
        "ANGEL_BACKDROP=off must omit Stage chrome\n{wide}"
    );
    assert!(!app.world_pane_visible);
    assert!(!app.scryglass.visible);
    assert!(!app.scryglass.renderable);

    let before = app.world.sim_tick_for_test();
    app.world_ticked_at = Instant::now()
        .checked_sub(Duration::from_secs(1))
        .unwrap_or_else(Instant::now);
    app.advance();
    app.advance();
    assert_eq!(
        app.world.sim_tick_for_test(),
        before,
        "hidden world must not earn scenery ticks under ANGEL_BACKDROP=off"
    );

    // Compact F3 agent focus used to full-body paint the bay even when off.
    app.focus_module("agent");
    let compact = render_app_text(&mut app, 80, 24);
    assert!(
        !compact.contains("Scryglass"),
        "compact agent focus must stay transcript-only when backdrops are off\n{compact}"
    );
    assert!(!app.world_pane_visible);
    assert_eq!(app.scryglass.surface, scryglass::StageSurface::Hidden);
}

/// A2: ANGEL_AGENT_INFO_HEADER must not open a side-column header plate when
/// backdrops are off (bay is not painted — no twin chrome tax).
#[test]
fn backdrop_off_skips_agent_info_header_side_plate() {
    let _lock = env_lock();
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "off");
    let _info = TestEnvGuard::set("ANGEL_AGENT_INFO_HEADER", "1");
    let mut app = seed_preview_app();
    let _ = app.open_module("agent");
    let rendered = render_app_text(&mut app, 144, 48);
    assert!(
        app.agent_info_area.is_none(),
        "text-only mode must not allocate agent info header rect"
    );
    assert!(
        !rendered.contains("Scryglass"),
        "backdrop off stays transcript-first\n{rendered}"
    );
}

/// A2: quick-lookup roll and scryglass poll must not keep the UI on the fast
/// tick (or run Stage presentation work) when the Stage was not painted.
#[test]
fn hidden_advance_clears_expired_lifecycle_ceremony() {
    use crate::ui::viz::lifecycle_viz::CeremonyKind;
    let _lock = env_lock();
    let mut app = seed_preview_app();
    let standard = render_app_text(&mut app, 120, 40);
    assert!(
        crate::tests::contains_dotmax(&standard),
        "visible world must paint dots"
    );
    assert!(app.start_lifecycle_ceremony(CeremonyKind::GoalDone, "goal"));
    assert!(app.lifecycle_ceremony.is_some());
    assert!(matches!(
        app.scryglass.controller.overlay(),
        Some(scryglass::StageOverlay::Lifecycle { .. })
    ));

    app.lifecycle_ceremony.as_mut().unwrap().started =
        Instant::now() - Duration::from_secs_f32(30.0);
    assert!(!app.lifecycle_ceremony_active());

    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "off");
    crate::ui::surfaces::invalidate_backdrop_cache();
    let hidden = render_app_text(&mut app, 120, 40);
    assert!(!app.world_pane_visible);
    assert!(
        !hidden.contains("tourney"),
        "backdrop-off must not paint the ceremony\n{hidden}"
    );
    // Draw skipped artifacts, so expiry was not settled until advance.
    app.advance();
    assert!(
        app.lifecycle_ceremony.is_none(),
        "hidden advance must drop the expired ceremony"
    );
    assert!(
        !matches!(
            app.scryglass.controller.overlay(),
            Some(scryglass::StageOverlay::Lifecycle { .. })
        ),
        "hidden advance must clear the leftover Lifecycle overlay"
    );
}

#[test]
fn backdrop_off_skips_invisible_lesson_roll_tax() {
    let _lock = env_lock();
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "off");
    let mut app = seed_preview_app();
    let _ = render_app_text(&mut app, 144, 48);
    assert!(!app.world_pane_visible);

    // Arm a mid-roll lesson as if Stage presentation were still draining.
    assert!(app.scryglass.set_unrolled_lesson(
        "Poisson",
        "Poisson",
        "A discrete probability distribution."
    ));
    assert!(
        app.quick_lookup_roll_pending(),
        "unrolled lesson should still have roll state to drain"
    );
    assert!(
        !app.needs_fast_tick(),
        "hidden Stage must not force fast tick for invisible lesson roll"
    );
    assert!(
        !app.world_animating(),
        "hidden Stage must not report world animation"
    );

    // Advance must not call poll_lesson when the Stage is not laid out — roll
    // progress stays put (no invisible presentation tax).
    let shown_before = app.scryglass.lesson().map(|l| l.shown_chars()).unwrap_or(0);
    app.advance();
    let shown_after = app.scryglass.lesson().map(|l| l.shown_chars()).unwrap_or(0);
    assert_eq!(
        shown_before, shown_after,
        "poll_lesson must not run under ANGEL_BACKDROP=off / hidden Stage"
    );
}

/// A2: successful turn settlement under ANGEL_BACKDROP=off must not credit
/// renown / hit rewards disk — Stage presentation tax only.
#[test]
fn backdrop_off_skips_world_turn_end_renown_tax() {
    let _lock = env_lock();
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "off");
    let mut app = seed_preview_app();
    let _ = render_app_text(&mut app, 144, 48);
    assert!(!app.world_pane_visible);
    assert!(!surfaces::BackdropMode::from_env().paints_in_process());

    app.world.turn_started();
    app.world.note_tool_call_event(
        harness::ToolEventId("hidden-turn".into()),
        "shell",
        "echo hi",
    );
    assert_eq!(app.world.active_work().count(), 1);
    let before = app.world.renown();

    // Same branch `finish_world_turn` takes when backdrop is off.
    app.world.turn_ended_hidden_stage(true);
    assert_eq!(
        app.world.renown(),
        before,
        "hidden Stage must not credit renown (no save_rewards tax)"
    );
    assert_eq!(
        app.world.active_work().count(),
        0,
        "hidden Stage must still clear in-flight tool work"
    );

    // Full turn_ended still pays when Stage paints.
    let _on = TestEnvGuard::unset("ANGEL_BACKDROP");
    assert!(surfaces::BackdropMode::from_env().paints_in_process());
    let mut painted = seed_preview_app();
    painted.world.turn_started();
    let r0 = painted.world.renown();
    painted.world.turn_ended(true);
    assert!(
        painted.world.renown() > r0,
        "visible Stage path still credits renown"
    );
}

/// A2: loop loom / status mirrors must not run when Stage is not painted.
#[test]
fn backdrop_off_skips_world_loop_mirror_tax() {
    let _lock = env_lock();
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "off");
    let mut app = seed_preview_app();
    let _ = render_app_text(&mut app, 144, 48);
    assert!(!app.world_pane_visible);
    assert!(!app.world.loop_mirrored_for_test());

    // Arm a live self-improvement loop as if the operator had `/loop` running.
    app.loop_ctl.status = loop_ctl::LoopStatus::Running;
    app.loop_ctl.iteration = 3;
    app.loop_ctl.task = "invisible loop tax".into();
    app.advance();
    assert!(
        !app.world.loop_mirrored_for_test(),
        "note_loop must not run under ANGEL_BACKDROP=off / hidden Stage"
    );
    assert!(
        !app.quintain_route_presented,
        "quintain latch must clear while Stage is hidden"
    );
}

/// A2: live tool storms must not pay world classify / journey / notice tax when
/// the Stage was not painted (ANGEL_BACKDROP=off). Strip still records activity.
#[test]
fn backdrop_off_skips_world_tool_journey_tax() {
    let _lock = env_lock();
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "off");
    let call_id = harness::ToolEventId("tool-off-1".to_string());
    let (mut app, _tx) = seed_live_streaming_app(vec![
        harness::TurnEvent::ToolCall {
            id: call_id.clone(),
            name: "apply_patch".into(),
            args_summary: "cockpit miniviz".into(),
        },
        harness::TurnEvent::ToolResult {
            id: call_id,
            name: "apply_patch".into(),
            summary: "ok".into(),
            outcome: harness::ToolOutcome {
                execution: harness::ExecutionOutcome::Succeeded,
                verification: harness::VerificationOutcome::NotApplicable,
            },
        },
        harness::TurnEvent::Notice("musing…".into()),
    ]);
    // Earn a definitive hidden Stage (draw sets world_pane_visible from plan).
    let _ = render_app_text(&mut app, 144, 48);
    assert!(!app.world_pane_visible);
    assert_eq!(app.world.active_work().count(), 0);

    app.advance();

    assert_eq!(
        app.world.active_work().count(),
        0,
        "hidden Stage must not mirror tool calls into active_work"
    );
    assert!(
        app.world.latest_active_work().is_none(),
        "no Stage journey work under ANGEL_BACKDROP=off"
    );
    // Tool strip still reflects the call (operator-visible, not Stage).
    let strip = app.tool_strip.snapshot();
    assert!(
        strip.calls >= 1,
        "tool strip must still record activity when Stage is hidden (calls={})",
        strip.calls
    );
}

/// A live `/loop` must not keep the UI at 33 ms when the Stage was not
/// painted (ANGEL_BACKDROP=off / Hidden). Visible Stage still requests it.
#[test]
fn backdrop_off_skips_live_loop_fast_tick() {
    let _lock = env_lock();
    let _backdrop = TestEnvGuard::set("ANGEL_BACKDROP", "off");
    let _comp = TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::drive::comp_mode::invalidate_cache();
    let mut app = seed_preview_app();
    let _ = render_app_text(&mut app, 144, 48);
    assert!(!app.world_pane_visible);
    app.loop_ctl.status = loop_ctl::LoopStatus::Running;
    app.loop_ctl.cycle_started_ms = Some(1);
    assert!(
        !app.stage_display_wants_fast_tick(),
        "hidden Stage must not request loop-viz cadence"
    );
    assert!(
        !app.needs_fast_tick(),
        "hidden live loop must stay on the idle lane"
    );

    app.world_pane_visible = true;
    assert!(
        app.stage_display_wants_fast_tick(),
        "visible Stage still requests loop-viz cadence"
    );
    assert!(app.needs_fast_tick());
}

/// Agent-bay portal must not 33ms-tick when the side column is off.
#[test]
fn backdrop_off_skips_agentviz_fast_tick() {
    let _lock = env_lock();
    let _comp = TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::drive::comp_mode::invalidate_cache();
    let _on = TestEnvGuard::unset("ANGEL_BACKDROP");
    assert!(
        crate::App::side_column_visuals_allowed(),
        "default in-process backdrop still allows side-column visuals"
    );
    let mut app = seed_preview_app();
    app.agentviz_portal.force_rendering_for_test();
    assert!(app.agentviz_wants_fast_tick());
    assert!(app.needs_fast_tick());

    let _off = TestEnvGuard::set("ANGEL_BACKDROP", "off");
    assert!(!crate::App::side_column_visuals_allowed());
    assert!(
        !app.agentviz_wants_fast_tick(),
        "text-only cockpit must not fast-tick an invisible portal"
    );
    assert!(
        !app.needs_fast_tick(),
        "backdrop-off portal must stay on the idle lane"
    );
}

/// A2: when the Stage is painted, scenery ticks resume.
#[test]
fn visible_stage_earns_world_sim_ticks() {
    let _lock = env_lock();
    let _backdrop = TestEnvGuard::unset("ANGEL_BACKDROP");
    let mut app = seed_preview_app();
    let standard = render_app_text(&mut app, 144, 48);
    assert!(
        crate::tests::contains_dotmax(&standard),
        "visible world must paint dots"
    );
    assert!(
        app.world_pane_visible,
        "Stage paint must earn world visibility"
    );

    let before = app.world.sim_tick_for_test();
    app.world_ticked_at = Instant::now()
        .checked_sub(Duration::from_secs(1))
        .unwrap_or_else(Instant::now);
    app.advance();
    assert!(
        app.world.sim_tick_for_test() > before,
        "visible Stage must advance the miniworld sim clock"
    );
}
