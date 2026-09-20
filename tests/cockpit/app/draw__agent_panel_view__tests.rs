use super::{
    render_agent_bay, speech_flow_split, treebeard_strip_label, treebeard_strip_label_with_forge,
    treebeard_strip_label_with_open, treebeard_strip_label_with_p1, truncate_control_value,
};
use crate::harness::{ForgeTrainSnap, HandleStoreStats, LastRootHiq};

/// The speech flow keeps whole trailing words in flight, settles the rest,
/// treats a brand-new stream as entirely fresh, and never panics on
/// multibyte or whitespace-free tails.
#[test]
fn speech_flow_split_keeps_whole_words_in_flight() {
    assert_eq!(speech_flow_split("", 24), ("", ""));
    // Stream opening: everything is still pouring out of the portrait.
    assert_eq!(
        speech_flow_split("first thought", 24),
        ("", "first thought")
    );
    let (settled, fresh) =
        speech_flow_split("the fleet topology needs a single tenant on the b70", 24);
    assert!(
        settled.ends_with(' '),
        "settled keeps the join space: {settled:?}"
    );
    assert!(
        fresh.chars().count() <= 24,
        "fresh stays in the window: {fresh:?}"
    );
    assert!(!fresh.starts_with(char::is_whitespace));
    assert!(
        fresh.split_whitespace().count() >= 1
            && format!("{settled}{fresh}") == "the fleet topology needs a single tenant on the b70"
    );
    // Whitespace-free window splits mid-token instead of stalling.
    let long = format!("prefix {}", "x".repeat(60));
    let (settled, fresh) = speech_flow_split(&long, 24);
    assert!(!fresh.is_empty() && fresh.chars().count() <= 24);
    assert_eq!(format!("{settled}{fresh}"), long);
    // Multibyte tail stays on char boundaries.
    let uni = "думать 思考 penser réfléchir überlegen";
    let (settled, fresh) = speech_flow_split(uni, 10);
    assert_eq!(format!("{settled}{fresh}"), uni);
    // Fresh renders on a single row: an explicit newline in the window
    // must never survive into the in-flight strip.
    let (_, fresh) = speech_flow_split("working downward\ntail marker", 24);
    assert_eq!(fresh, "tail marker");
    assert!(!fresh.contains('\n'));
}

#[test]
fn header_capability_paint_skips_full_route_metadata() {
    let src = include_str!("../../../cockpit/src/draw/agent_panel_view.rs");
    let start = src
        .find("pub(crate) fn agent_route_capability_paint")
        .expect("agent_route_capability_paint present");
    let body = &src[start..];
    let end = body
        .find("\npub(crate) fn side_column_kitty_compose_allowed")
        .expect("side_column_kitty_compose_allowed follows paint");
    let body = &body[..end];
    assert!(
        body.contains("header_route_metadata") && !body.contains(".route_metadata()"),
        "header paint must not take HTTP output-budget locks:\n{body}"
    );
}

#[test]
fn comp_mode_skips_speech_flow_without_slowing_default() {
    use crate::tests::{TestEnvGuard, env_lock};

    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = TestEnvGuard::unset("ANGEL_TURBO");
    crate::comp_mode::invalidate_cache();
    assert!(super::speech_flow_allowed());
    let text = "the fleet topology needs a single tenant on the b70";
    let (settled, fresh) = super::maybe_speech_flow_split(text, 24);
    assert!(
        !fresh.is_empty() && settled != text,
        "default still pours the tail from the portrait"
    );
    assert_eq!(format!("{settled}{fresh}"), text);

    let _on = TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::comp_mode::invalidate_cache();
    assert!(!super::speech_flow_allowed());
    let (lean_settled, lean_fresh) = super::maybe_speech_flow_split(text, 24);
    assert_eq!(lean_settled, text);
    assert!(
        lean_fresh.is_empty(),
        "comp/lean must not split a decorative in-flight strip"
    );
}

#[test]
fn control_truncation_preserves_route_ends_within_terminal_cells() {
    let value = "模型/資料🧪/gpt-5.6-sol";
    let fitted = truncate_control_value(value, 18);
    assert!(
        unicode_width::UnicodeWidthStr::width(fitted.as_str()) <= 18,
        "wide route overflowed its control: {fitted}"
    );
    assert!(fitted.starts_with("模型"), "family prefix lost: {fitted}");
    assert!(
        fitted.ends_with("5.6-sol"),
        "route slug tail lost: {fitted}"
    );
    assert!(fitted.contains('…'));
    assert_eq!(truncate_control_value(value, 1), "…");
    assert_eq!(truncate_control_value(value, 0), "");
}

#[test]
fn treebeard_header_strip_cache_compares_open_key_without_cloning() {
    let src = include_str!("../../../cockpit/src/draw/agent_panel_view.rs");
    let start = src
        .find("fn treebeard_header_strip_label")
        .expect("header strip");
    let body = src[start..]
        .split("pub(crate) fn treebeard_strip_label(")
        .next()
        .expect("strip cache body");
    assert!(
        body.contains("cached.4.as_deref() == top_open_key"),
        "hit path must compare the open key without cloning: {body}"
    );
    assert!(
        !body.contains("|(key, _)| key.clone()"),
        "cache key must not clone the open lever on every frame: {body}"
    );
}

#[test]
fn treebeard_strip_includes_peer_and_offload() {
    let hiq = LastRootHiq {
        offload_ratio: 1.0,
        hiq_priority: 2.1875,
        handle_receipts: 2,
        aged_receipts: 0,
        bulk_tool_results: 0,
        strategy_token_n: 8,
    };
    let stats = HandleStoreStats {
        entries: 0,
        total_bytes: 0,
        puts: 0,
        discloses: 0,
        evictions: 0,
    };
    let s = treebeard_strip_label_with_p1(
        Some(hiq),
        stats,
        Some((867.91, "c3".into(), None)),
        Some(1685.0),
    );
    assert!(s.contains("offload 100%"), "got: {s}");
    assert!(
        s.contains("hiq 2.19") || s.contains("hiq 2.1875"),
        "got: {s}"
    );
    assert!(s.contains("peer 867.9µs"), "got: {s}");
    assert!(s.contains("P1 1685µs"), "got: {s}");
    let bare = treebeard_strip_label(None, stats, None);
    assert_eq!(bare, "treebeard · Hi/Q");
    let forge = ForgeTrainSnap {
        state: "training".into(),
        train_step: Some(40),
        train_total: Some(200),
        train_eta_sec: Some(2280),
        train_loss: Some(1.23),
        train_loss_min: Some(0.25),
        train_loss_max: Some(1.23),
        prior_train_loss: Some(0.41),
        train_loss_improvement: Some(0.55),
        gpu_free_mib: Some(15000.0),
        free_mib_min: Some(14900.0),
        vram_warn: None,
        train_phase: None,
        version: None,
        gate_pass: None,
        promoted: None,
        adapter_local: false,
        open_lever_top: None,
        free_train_primary_n: None,
        measured_hold_us: None,
        preference_n: Some(96),
        coding_eval_n: Some(24),
        coding_eval_primary_n: Some(4),
    };
    let with_forge = treebeard_strip_label_with_forge(
        Some(hiq),
        stats,
        Some((867.91, "c3".into(), None)),
        Some(1685.0),
        Some(forge),
    );
    assert!(
        with_forge.contains("forge 40/200 ~38m") && with_forge.contains("L1.23"),
        "got: {with_forge}"
    );
    let post = treebeard_strip_label_with_forge(
        None,
        stats,
        None,
        None,
        Some(ForgeTrainSnap {
            state: "training".into(),
            train_step: Some(200),
            train_total: Some(200),
            train_eta_sec: Some(0),
            train_loss: None,
            train_loss_min: None,
            train_loss_max: None,
            prior_train_loss: None,
            train_loss_improvement: None,
            gpu_free_mib: None,
            free_mib_min: None,
            vram_warn: None,
            train_phase: Some("adapter_eval".into()),
            version: None,
            gate_pass: None,
            promoted: None,
            adapter_local: false,
            open_lever_top: None,
            free_train_primary_n: None,
            measured_hold_us: None,
            preference_n: None,
            coding_eval_n: None,
            coding_eval_primary_n: None,
        }),
    );
    assert!(
        post.contains("forge 200/200 eval"),
        "post-step phase on strip: {post}"
    );
    let done = treebeard_strip_label_with_forge(
        None,
        stats,
        None,
        None,
        Some(ForgeTrainSnap {
            state: "done".into(),
            train_step: None,
            train_total: None,
            train_eta_sec: None,
            train_loss: None,
            train_loss_min: None,
            train_loss_max: None,
            prior_train_loss: Some(0.41),
            train_loss_improvement: Some(0.55),
            gpu_free_mib: None,
            free_mib_min: None,
            vram_warn: None,
            train_phase: None,
            version: Some("v1".into()),
            gate_pass: Some(true),
            promoted: Some(false),
            adapter_local: true,
            open_lever_top: Some("32768x1".into()),
            free_train_primary_n: Some(4),
            measured_hold_us: Some(38300.0),
            preference_n: Some(96),
            coding_eval_n: Some(24),
            coding_eval_primary_n: Some(4),
        }),
    );
    assert!(
        done.contains("forge v1 gate✓ local")
            && done.contains("→32k")
            && done.contains("ftP4")
            && done.contains("H38k")
            && done.contains("pref96")
            && done.contains("ce24"),
        "got: {done}"
    );
    let with_open = treebeard_strip_label_with_open(
        None,
        stats,
        Some((867.91, "c3".into(), None)),
        Some(1685.0),
        Some(("32768x1".into(), 38800.0)),
        None,
    );
    assert!(
        with_open.contains("PRIMARY 32k 39ms") || with_open.contains("PRIMARY 32k 38ms"),
        "got: {with_open}"
    );
    assert!(with_open.contains("P1 1685µs"), "got: {with_open}");
    // P1 as top open must not double-print.
    let p1_only = treebeard_strip_label_with_open(
        None,
        stats,
        None,
        Some(1685.0),
        Some(("512x640".into(), 1685.0)),
        None,
    );
    assert!(p1_only.contains("P1 1685µs"), "got: {p1_only}");
    assert!(!p1_only.contains("open "), "got: {p1_only}");
    let dotted_p1 = treebeard_strip_label_with_open(
        None,
        stats,
        None,
        Some(1685.0),
        Some(("512·640".into(), 1685.0)),
        None,
    );
    assert!(dotted_p1.contains("P1 1685µs"), "got: {dotted_p1}");
    assert!(!dotted_p1.contains("open "), "got: {dotted_p1}");
    let src = include_str!("../../../cockpit/src/draw/agent_panel_view.rs");
    let start = src
        .find("fn treebeard_open_key_is_p1")
        .expect("open-key matcher");
    let body = src[start..]
        .split("pub(crate) fn treebeard_strip_label_with_open")
        .next()
        .expect("matcher body");
    assert!(
        !body.contains("to_ascii_lowercase") && !body.contains("replace("),
        "open-key match must not allocate a lowered copy: {body}"
    );
}

/// The App-level sampler feeds the pure precedence ladder: an unfinished
/// strip entry reads Tool, a pending approval outranks it, a fresh
/// end-of-turn marker glows and an aged one expires — and the bay caption
/// paints the blocked word where the operator actually looks.
#[test]
fn blocked_approval_owns_the_bay_caption_and_sampler_precedence() {
    use crate::views::agent_view::{PortraitMarker, PortraitState};
    use crate::app::{App, PendingApproval};
    use crate::harness::ToolEventId;
    use crate::viewer::Viewer;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::time::{Duration, Instant};

    let mut app = App::preview(Viewer::static_preview());
    assert_eq!(super::portrait_state(&app), PortraitState::Idle);
    app.portrait_turn_marker = Some((PortraitMarker::Victory, Instant::now()));
    assert_eq!(super::portrait_state(&app), PortraitState::Victory);
    if let Some(old) = Instant::now().checked_sub(Duration::from_secs(60)) {
        app.portrait_turn_marker = Some((PortraitMarker::Recovery, old));
        assert_eq!(super::portrait_state(&app), PortraitState::Idle);
    }
    app.portrait_turn_marker = Some((PortraitMarker::Victory, Instant::now()));
    app.tool_strip
        .call_event(ToolEventId("t1".into()), "shell", "cargo check");
    assert_eq!(super::portrait_state(&app), PortraitState::Tool);
    let (reply, _keep_rx) = std::sync::mpsc::channel();
    app.pending_approval = Some(PendingApproval {
        prompt: "swarm wants to phone a SOTA".into(),
        scope_label: None,
        reply,
    });
    assert_eq!(super::portrait_state(&app), PortraitState::Blocked);

    let mut terminal = Terminal::new(TestBackend::new(64, 16)).expect("test terminal");
    terminal
        .draw(|frame| render_agent_bay(frame, &mut app, frame.area()))
        .expect("render agent bay");
    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(
        rendered.contains("Approval needed"),
        "blocked caption missing: {rendered}"
    );
}

#[test]
fn active_webgpu_stage_has_a_visible_agent_pane_card() {
    use crate::app::App;
    use crate::tests::{TestEnvGuard, env_lock};
    use crate::viewer::Viewer;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = TestEnvGuard::unset("ANGEL_TURBO");
    let _backdrop = TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::comp_mode::invalidate_cache();
    crate::surfaces::invalidate_backdrop_cache();
    let mut app = App::preview(Viewer::static_preview());
    app.agentviz_portal = crate::viz::agentviz_portal::PortalRuntime::presentation_for_test("judge", 3);
    let mut terminal = Terminal::new(TestBackend::new(48, 16)).expect("test terminal");
    terminal
        .draw(|frame| render_agent_bay(frame, &mut app, frame.area()))
        .expect("render agent bay");
    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(
        rendered.contains("WebGPU"),
        "portal title missing: {rendered}"
    );
    assert!(
        rendered.contains("judge") && rendered.contains("3 seats"),
        "portal activity summary missing: {rendered}"
    );
    assert!(
        rendered.contains("rendering GPU portal"),
        "portal pending state missing: {rendered}"
    );
}

#[test]
fn hidden_comp_skips_agent_token_meter_without_slowing_default() {
    use crate::app::App;
    use crate::tests::{TestEnvGuard, env_lock};
    use crate::viewer::Viewer;

    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = TestEnvGuard::unset("ANGEL_TURBO");
    let _backdrop = TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::comp_mode::invalidate_cache();
    crate::surfaces::invalidate_backdrop_cache();
    assert!(
        super::agent_token_meter_allowed(),
        "default bay still builds token meters"
    );

    let _on = TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::comp_mode::invalidate_cache();
    assert!(
        !super::agent_token_meter_allowed(),
        "comp/lean must not read the MoA token ledger or build tok meters"
    );
    let app = App::preview(Viewer::static_preview());
    assert!(
        super::agent_token_lines(&app, 48, 5).is_empty(),
        "comp/lean must skip agent_token_lines"
    );

    drop(_on);
    crate::comp_mode::invalidate_cache();
    assert!(
        super::agent_token_meter_allowed(),
        "default cockpit must not stay gated after /comp off"
    );

    let _hidden = TestEnvGuard::set("ANGEL_BACKDROP", "off");
    crate::surfaces::invalidate_backdrop_cache();
    assert!(
        !super::agent_token_meter_allowed(),
        "backdrop-off must not build invisible token meters"
    );
}

#[test]
fn hidden_comp_skips_agent_host_metrics_without_slowing_default() {
    use crate::app::App;
    use crate::tests::{TestEnvGuard, env_lock};
    use crate::viewer::Viewer;

    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = TestEnvGuard::unset("ANGEL_TURBO");
    let _backdrop = TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::comp_mode::invalidate_cache();
    crate::surfaces::invalidate_backdrop_cache();
    assert!(
        super::agent_host_metrics_allowed(),
        "default bay still paints CPU/mem/gpu"
    );
    let app = App::preview(Viewer::static_preview());
    assert!(
        super::agent_host_metrics_line(&app, false).is_some(),
        "default still formats the host-metrics line"
    );

    let _on = TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::comp_mode::invalidate_cache();
    assert!(
        !super::agent_host_metrics_allowed(),
        "comp/lean must not format the bay host-metrics strip"
    );
    assert!(
        super::agent_host_metrics_line(&app, false).is_none(),
        "comp/lean must skip the host-metrics string"
    );

    drop(_on);
    crate::comp_mode::invalidate_cache();
    assert!(
        super::agent_host_metrics_allowed(),
        "default cockpit must not stay gated after /comp off"
    );

    let _hidden = TestEnvGuard::set("ANGEL_BACKDROP", "off");
    crate::surfaces::invalidate_backdrop_cache();
    assert!(
        !super::agent_host_metrics_allowed(),
        "backdrop-off must not build invisible host metrics"
    );
    assert!(super::agent_host_metrics_line(&app, false).is_none());
}

#[test]
fn hidden_comp_skips_agent_route_capability_without_slowing_default() {
    use crate::app::App;
    use crate::club::RouteMetadata;
    use crate::tests::{TestEnvGuard, env_lock};
    use crate::viewer::Viewer;

    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = TestEnvGuard::unset("ANGEL_TURBO");
    let _backdrop = TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::comp_mode::invalidate_cache();
    crate::surfaces::invalidate_backdrop_cache();
    assert!(
        super::agent_route_capability_allowed(),
        "default bay still paints ctx/speed capability"
    );
    let meta = RouteMetadata {
        context_window: Some(128_000),
        ..RouteMetadata::default()
    };
    assert!(
        super::compact_header_route_capability_line(&meta, 1_024, 48)
            .is_some_and(|line| line.contains("ctx")),
        "default capability essay still names ctx"
    );
    let app = App::preview(Viewer::static_preview());
    let (_pressure, default_line) = super::agent_route_capability_paint(&app, 48);
    let _ = default_line;

    let _on = TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::comp_mode::invalidate_cache();
    assert!(
        !super::agent_route_capability_allowed(),
        "comp/lean must not walk route metadata for the capability essay"
    );
    let (pressure, line) = super::agent_route_capability_paint(&app, 48);
    assert_eq!(pressure, super::ContextPressure::Normal);
    assert!(
        line.is_none(),
        "comp/lean must skip the capability string build"
    );

    drop(_on);
    crate::comp_mode::invalidate_cache();
    assert!(
        super::agent_route_capability_allowed(),
        "default cockpit must not stay gated after /comp off"
    );

    let _hidden = TestEnvGuard::set("ANGEL_BACKDROP", "off");
    crate::surfaces::invalidate_backdrop_cache();
    assert!(
        !super::agent_route_capability_allowed(),
        "backdrop-off must not build an invisible capability essay"
    );
    assert!(super::agent_route_capability_paint(&app, 48).1.is_none());
}

#[test]
fn hidden_comp_skips_agent_route_title_without_slowing_default() {
    use crate::app::App;
    use crate::tests::{TestEnvGuard, env_lock};
    use crate::viewer::Viewer;

    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = TestEnvGuard::unset("ANGEL_TURBO");
    let _backdrop = TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::comp_mode::invalidate_cache();
    crate::surfaces::invalidate_backdrop_cache();
    assert!(
        super::agent_route_title_allowed(),
        "default bay still paints the tab/clock route title"
    );
    let mut app = App::preview(Viewer::static_preview());
    let _default = super::agent_route_title(&mut app, 48, 8);

    let _on = TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::comp_mode::invalidate_cache();
    assert!(
        !super::agent_route_title_allowed(),
        "comp/lean must not walk bag.tabs() for the bay title"
    );
    assert!(
        super::agent_route_title(&mut app, 48, 8).is_none(),
        "comp/lean must skip the route-title string build"
    );

    drop(_on);
    crate::comp_mode::invalidate_cache();
    assert!(
        super::agent_route_title_allowed(),
        "default cockpit must not stay gated after /comp off"
    );

    let _hidden = TestEnvGuard::set("ANGEL_BACKDROP", "off");
    crate::surfaces::invalidate_backdrop_cache();
    assert!(
        !super::agent_route_title_allowed(),
        "backdrop-off must not build an invisible bay route title"
    );
    assert!(super::agent_route_title(&mut app, 48, 8).is_none());
}

#[test]
fn comp_mode_skips_side_column_kitty_compose_without_slowing_default() {
    use crate::app::App;
    use crate::tests::{TestEnvGuard, env_lock};
    use crate::viewer::Viewer;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let _lock = env_lock();
    let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = TestEnvGuard::unset("ANGEL_TURBO");
    let _backdrop = TestEnvGuard::unset("ANGEL_BACKDROP");
    crate::comp_mode::invalidate_cache();
    crate::surfaces::invalidate_backdrop_cache();

    assert!(
        super::side_column_kitty_compose_allowed(),
        "default bay still encodes Kitty portraits/portal"
    );
    let mut composed = 0usize;
    assert_eq!(
        super::maybe_paint_side_column_kitty(|| {
            composed += 1;
            "pixels"
        }),
        Some("pixels")
    );
    assert_eq!(composed, 1);

    let mut app = App::preview(Viewer::static_preview());
    app.agentviz_portal = crate::viz::agentviz_portal::PortalRuntime::presentation_for_test("judge", 3);
    let mut terminal = Terminal::new(TestBackend::new(48, 16)).expect("test terminal");
    terminal
        .draw(|frame| render_agent_bay(frame, &mut app, frame.area()))
        .expect("render default agent bay");
    let standard = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(
        standard.contains("WebGPU") && standard.contains("rendering GPU portal"),
        "default visible bay still paints the portal card\n{standard}"
    );

    let _on = TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::comp_mode::invalidate_cache();
    assert!(!super::side_column_kitty_compose_allowed());
    let mut skipped = 0usize;
    assert!(
        super::maybe_paint_side_column_kitty(|| {
            skipped += 1;
            "pixels"
        })
        .is_none()
    );
    assert_eq!(skipped, 0, "comp/lean must not invoke Kitty compose");

    let mut armed = App::preview(Viewer::static_preview());
    armed.agentviz_portal =
        crate::viz::agentviz_portal::PortalRuntime::presentation_for_test("judge", 3);
    let mut lean = Terminal::new(TestBackend::new(48, 16)).expect("test terminal");
    lean.draw(|frame| render_agent_bay(frame, &mut armed, frame.area()))
        .expect("render lean agent bay");
    let lean_text = lean
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(
        lean_text.contains("agent"),
        "comp-mode keeps bay chrome\n{lean_text}"
    );
    assert!(
        !lean_text.contains("WebGPU") && !lean_text.contains("rendering GPU portal"),
        "comp-mode must not compose the portal card\n{lean_text}"
    );

    drop(_on);
    crate::comp_mode::invalidate_cache();
    let _hidden = TestEnvGuard::set("ANGEL_BACKDROP", "off");
    crate::surfaces::invalidate_backdrop_cache();
    assert!(
        !super::side_column_kitty_compose_allowed(),
        "backdrop-off must not encode an invisible side column"
    );
    assert!(super::maybe_paint_side_column_kitty(|| "pixels").is_none());
}

#[test]
fn host_metrics_spans_keep_borrowed_idle_cow() {
    let src = include_str!("../../../cockpit/src/draw/agent_panel_view.rs");
    let prod = src
        .split("fn host_metrics_spans_keep_borrowed_idle_cow")
        .next()
        .expect("prod");
    assert!(
        !prod.contains(concat!("metrics.", "into_owned()")),
        "idle host metrics must keep the borrowed Cow on the span"
    );
    assert!(
        prod.contains("telemetry.push(Span::raw(metrics))"),
        "header telemetry must span the Cow directly"
    );
    assert!(
        prod.contains("Line::from(Span::raw(metrics))"),
        "bay metrics must span the Cow directly"
    );
}

/// The per-frame chrome snapshot must mirror the live bag reads, and the
/// frozen wrapper (fresh snapshot) must paint the exact same rail as a
/// `ui`-style shared snapshot.
#[test]
fn frame_chrome_snapshot_matches_live_bag_and_paints_identically() {
    use super::{FrameChrome, render_agent_controls, render_agent_controls_in};
    use crate::app::App;
    use crate::club::Bag;
    use crate::viewer::Viewer;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::preview(Viewer::static_preview());
    app.bag = Bag::for_reasoning_render_test();
    let chrome = FrameChrome::compute(&app.bag);
    assert_eq!(chrome.mode(), app.bag.in_hand_mode().as_deref());
    assert_eq!(chrome.effort(), app.bag.reasoning_effort().as_deref());
    assert_eq!(
        chrome.effort_selectable(),
        !app.bag.reasoning_levels().is_empty()
    );

    let mut wrapper = Terminal::new(TestBackend::new(64, 2)).expect("test terminal");
    wrapper
        .draw(|frame| render_agent_controls(frame, &mut app, frame.area()))
        .expect("wrapper render");
    app.agent_control_area = None;
    app.agent_buttons.clear();
    let mut shared = Terminal::new(TestBackend::new(64, 2)).expect("test terminal");
    shared
        .draw(|frame| render_agent_controls_in(frame, &mut app, frame.area(), &chrome))
        .expect("shared render");
    assert_eq!(
        wrapper.backend().buffer(),
        shared.backend().buffer(),
        "wrapper and shared-chrome renders must be byte-identical"
    );
}

#[test]
fn agent_control_rail_reuses_chip_strings_across_unchanged_frames() {
    use super::{FrameChrome, render_agent_controls};
    use crate::app::App;
    use crate::club::Bag;
    use crate::viewer::Viewer;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let src = include_str!("../../../cockpit/src/draw/agent_panel_view.rs");
    let start = src
        .find("pub(crate) fn render_agent_controls_in")
        .expect("render_agent_controls_in present");
    let body = &src[start..];
    let end = body
        .find("\nfn moa_chip_identity")
        .expect("moa_chip_identity follows render_agent_controls_in");
    let body = &body[..end];
    assert!(
        body.contains("agent_control_chips") && body.contains("cache_hit"),
        "draw must reuse MODEL/THINK/FORMATION strings across frames:\n{body}"
    );

    let mut app = App::preview(Viewer::static_preview());
    app.bag = Bag::for_reasoning_render_test();
    let mut first = Terminal::new(TestBackend::new(64, 2)).expect("test terminal");
    first
        .draw(|frame| render_agent_controls(frame, &mut app, frame.area()))
        .expect("first render");
    let cached = app
        .agent_control_chips
        .as_ref()
        .expect("first draw fills the chip cache")
        .clone();
    app.agent_control_area = None;
    app.agent_buttons.clear();
    let mut second = Terminal::new(TestBackend::new(64, 2)).expect("test terminal");
    second
        .draw(|frame| render_agent_controls(frame, &mut app, frame.area()))
        .expect("second render");
    let again = app
        .agent_control_chips
        .as_ref()
        .expect("second draw keeps the chip cache");
    assert_eq!(cached.model_text, again.model_text);
    assert_eq!(cached.think_text, again.think_text);
    assert_eq!(cached.moa_text, again.moa_text);
    assert_eq!(
        first.backend().buffer(),
        second.backend().buffer(),
        "cached chip strings must paint the same rail"
    );
    let chrome = FrameChrome::compute(&app.bag);
    assert_eq!(cached.mode.as_deref(), chrome.mode());
    assert_eq!(cached.effort.as_deref(), chrome.effort());
}
