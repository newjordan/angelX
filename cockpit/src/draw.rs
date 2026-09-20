//! Root draw compositor for the cockpit TUI: shared layout/geometry helpers,
//! the `ui` entry point, and headless preview renderers backing `--dump-preview`
//! and draw-proof tests. Cohesive views live in nested modules under
//! `draw/`; all operate on `App` state through this parent module.

use super::*;

mod agent_panel_view;
mod route_caps;
pub(crate) use route_caps::{
    ContextPressure, compact_header_route_capability_line, context_pressure, context_usage_status,
    format_evidence_duration, format_evidence_tokens, model_capability_detail, route_context_badge,
};
mod brain_route_view;
mod stage_view;
mod transcript_view;

#[cfg(test)]
pub(crate) use agent_panel_view::portrait_active;
#[cfg(test)]
pub(crate) use agent_panel_view::{render_agent_bay, render_agent_controls};
pub(crate) use agent_panel_view::{render_agent_header_info, truncate_control_value};
#[cfg(test)]
pub(crate) use stage_view::render_vault_for_test;
#[cfg(test)]
pub(crate) use stage_view::{
    ambient_stage_column_steal_allowed, artifacts_pane_active, maybe_paint_world_scene,
    maybe_run_expensive_world_compose, miniviz_dancer_paint_allowed,
    miniviz_expensive_compose_allowed, scryglass_live_world_title_allowed, scryglass_route_title,
    scryglass_scene_accessories_allowed, scryglass_scene_body_allowed,
    stage_live_route_title_allowed, stage_route_title,
};
pub(crate) use transcript_view::render_transcript;
#[cfg(test)]
pub(crate) use transcript_view::{
    tool_strip_height, transcript_roll_in_allowed, transcript_text_and_rail,
};

pub(crate) fn scan_start(previous_len: usize, current_len: usize) -> usize {
    if previous_len <= current_len {
        previous_len
    } else {
        0
    }
}

fn cockpit_side_width(width: u16) -> u16 {
    let transcript_target = if width >= 180 {
        105
    } else if width >= 140 {
        80
    } else {
        64
    };
    let realm_target = if width >= 180 {
        80
    } else if width >= 140 {
        64
    } else {
        48
    };
    realm_target.min(width.saturating_sub(transcript_target).saturating_add(1))
}

// reasoning_agent_height lives in `surfaces` (layout is role-aware).
/// The inter-panel gap to use for a terminal of the given size: the configured
/// [`PANEL_GAP`] when there's room, else 0 (contiguous) so tiny terminals never
/// lose a panel to spacer cells. Drives both the inter-panel spacing and the
/// outer screen margin, so a non-zero gap means every panel floats.
pub(crate) fn panel_gap(width: u16, height: u16) -> u16 {
    if width >= GAP_MIN_WIDTH && height >= GAP_MIN_HEIGHT {
        PANEL_GAP
    } else {
        0
    }
}

/// Draw a chrome strip's optional bordered box, publish its outer frame rect
/// (the Phase B hook), and return the content rect to render into. The 1-row
/// header/footer strips become their own floating boxes when `boxed` (a tall
/// enough terminal); otherwise they stay flat, spanning the whole rect. Either
/// way the frame rect is published so Phase B can attach a skin/scene to it.
pub(crate) fn frame_panel(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    boxed: bool,
    kind: panels::PanelKind,
) -> Rect {
    app.panel_frames.push(kind, area);
    if boxed && area.height >= 3 && area.width >= 2 {
        let block = hud_block("");
        let inner = block.inner(area);
        frame.render_widget(block, area);
        inner
    } else {
        area
    }
}

/// Route-control chrome shared by every rail of one frame. The bag's mode /
/// effort lookups take club locks (HTTP routes also read env), so `ui` snapshots
/// them once per frame and each rail renders from the same snapshot. Standalone
/// callers (tests) snapshot on entry via the frozen wrapper signatures.
/// Hit path is an `Arc` clone of the Bag snapshot — no per-frame String clones.
pub(crate) struct FrameChrome {
    inner: std::sync::Arc<club::InHandChrome>,
}

impl FrameChrome {
    pub(crate) fn compute(bag: &club::Bag) -> Self {
        Self {
            inner: bag.in_hand_chrome(),
        }
    }

    pub(crate) fn mode(&self) -> Option<&str> {
        self.inner.mode.as_deref()
    }

    pub(crate) fn effort(&self) -> Option<&str> {
        self.inner.effort.as_deref()
    }

    pub(crate) fn effort_selectable(&self) -> bool {
        self.inner.effort_selectable
    }
}

/// The role prefix tag and its style, shown before each message's first line.
pub(crate) fn render_header(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    include_mode: bool,
    chrome: &FrameChrome,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let _ = (include_mode, chrome);
    let version = env!("CARGO_PKG_VERSION");
    let mut spans = vec![
        Span::styled(
            "angel0",
            Style::new().fg(HUD_BLUE).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {version}"), Style::new().fg(HUD_DIM)),
    ];
    let save_degraded = matches!(
        app.session.save_status(),
        crate::session::SessionSaveStatus::Failed(_)
    );
    if save_degraded {
        spans.push(Span::styled(
            "  SESSION SAVE DEGRADED",
            Style::new()
                .fg(hud::HUD_DANGER)
                .add_modifier(Modifier::BOLD),
        ));
    }
    let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(chrome_style()),
        area,
    );
    let context = header_workspace_context(app);
    if !context.is_empty() && usize::from(area.width) > used + context.chars().count() + 4 {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("▸ ", Style::new().fg(HUD_DIM)),
                Span::styled(context, Style::new().fg(hud::HUD_GOLD)),
            ]))
            .alignment(Alignment::Right),
            area,
        );
    }
}

/// Display the same workspace that the live tool registry actually uses.
pub(crate) fn header_workspace_context(app: &App) -> String {
    let workspace = app.tools.current_workspace();
    workspace
        .file_name()
        .unwrap_or(workspace.as_os_str())
        .to_string_lossy()
        .into_owned()
}

pub(crate) fn render_header_controls(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    chrome: &FrameChrome,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    agent_panel_view::render_agent_controls_in(frame, app, area, chrome);
}

/// Render the combined header box (Phase B header+footer merge): the status line
/// on the first content row and, when the box has a second row, the keybind
/// route controls beneath it. A single-row (flat) header keeps the exact route in
/// the status label because there is no room for the dropdown rail.
pub(crate) fn render_combined_header(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    chrome: &FrameChrome,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    if agent_panel_view::header_portrait_bar_fits(area) {
        agent_panel_view::render_header_portrait_bar(frame, app, area, chrome);
        return;
    }
    if area.height >= 2 {
        let [status_area, controls_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
        render_header(frame, app, status_area, false, chrome);
        render_header_controls(frame, app, controls_area, chrome);
    } else {
        render_header(frame, app, area, true, chrome);
    }
}

/// Composer line with dim slash-completion ghost (suffix + alternate matches).
pub(crate) fn input_line_with_slash_ghost<'a>(
    input: &'a str,
    visible: &'a str,
    cursor: usize,
) -> Line<'a> {
    let Some(hint) = crate::input::slash_inline_hint(input, cursor) else {
        return Line::from(visible);
    };
    let mut spans = vec![Span::raw(visible)];
    if !hint.suffix.is_empty() {
        spans.push(Span::styled(
            hint.suffix,
            Style::new().fg(HUD_DIM).add_modifier(Modifier::DIM),
        ));
    }
    if !hint.alternates.is_empty() {
        spans.push(Span::styled(
            hint.alternates,
            Style::new().fg(HUD_DIM).add_modifier(Modifier::DIM),
        ));
    }
    Line::from(spans)
}

/// Register the transcript clipboard hit-target: prose only, excluding the
/// scroll rail and the live tool-strip band (A6). Falls back to a full-inner
/// text half when the frame is too small for a plan copy rect.
fn register_transcript_copy_pane(app: &mut App, transcript_frame: Rect) {
    let inner = mouse::inner_border(transcript_frame);
    let strip_h = transcript_view::tool_strip_height(app, inner.height);
    if let Some(prose) = crate::surfaces::transcript_live_copy_rect(transcript_frame, strip_h) {
        app.panes.push(mouse::PaneId::Transcript, prose);
        return;
    }
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let body = Rect {
        height: inner.height.saturating_sub(strip_h),
        ..inner
    };
    if body.height == 0 {
        return;
    }
    let (transcript_text, _) = transcript_view::transcript_text_and_rail(body);
    if transcript_text.width > 0 && transcript_text.height > 0 {
        app.panes.push(mouse::PaneId::Transcript, transcript_text);
    }
}

pub(crate) fn render_cockpit_with_agent_panels(
    frame: &mut Frame,
    app: &mut App,
    cockpit_area: Rect,
    chrome: &FrameChrome,
) {
    // Modular surface plan: text-critical transcript vs backdrop bay/scryglass.
    // `ANGEL_BACKDROP=off` expands transcript full-width (speed path) and skips
    // side-column activity probes (raytrace/RL/loop/ceremony) entirely.
    // `ANGEL_SURFACE_GAPS=1` isolates columns so selection cannot straddle.
    let backdrop = crate::surfaces::BackdropMode::from_env();
    let join = crate::surfaces::SurfaceJoin::from_env();

    // A2 lean path: no side-column layout, paint, or hit-test when backdrops are off.
    if !app.scryglass_enabled || !backdrop.shows_side_column() {
        let plan = crate::surfaces::plan_cockpit_surfaces(crate::surfaces::CockpitLayoutInput {
            area: cockpit_area,
            agent_active: false,
            artifacts_active: false,
            reasoning_visible: false,
            side_width: 0,
            idle_agent_height_cap: 0,
            backdrop,
            join,
        });
        let left_area = plan.transcript_frame().unwrap_or(cockpit_area);
        app.panel_frames
            .push(panels::PanelKind::Transcript, left_area);
        app.module_host.set_rect("core", left_area);
        render_transcript(frame, app, left_area);
        register_transcript_copy_pane(app, left_area);
        debug_assert!(
            plan.agent_frame().is_none() && plan.artifacts_frame().is_none(),
            "ANGEL_BACKDROP=off must not allocate side-column frames"
        );
        return;
    }

    let agent_active = app.module_host.is_running("agent");
    let raytrace_active =
        app.scryglass.controller.route() == crate::scryglass::StageRoute::Raytrace;
    // The Reinforce stage holds the column while routed-to, and a live RL run
    // keeps it alive even after the operator navigates elsewhere (like /loop).
    let reinforce_active = app.scryglass.controller.route()
        == crate::scryglass::StageRoute::Reinforce
        || app.tools.rl().running();
    let artifacts_active = stage_view::artifacts_pane_active(
        app.module_host.is_running("artifacts"),
        raytrace_active,
        app.lifecycle_ceremony_active(),
        loop_viz::visible(&app.loop_ctl),
        reinforce_active,
    );

    // Expand the trace pane when it has content (live or previous-turn, or a
    // background job), up to 80% of the column. Empty traces stay capped so a
    // useful miniviz remains.
    let reasoning_visible = !app.reasoning.is_empty()
        || app
            .bg_job
            .as_ref()
            .and_then(app_control::BackgroundJob::live_output)
            .is_some_and(|output| !output.trim().is_empty());
    let side_w = cockpit_side_width(cockpit_area.width);
    let idle_agent_cap = if cockpit_area.width >= 150 { 14 } else { 13 };
    let layout_input = crate::surfaces::CockpitLayoutInput {
        area: cockpit_area,
        agent_active,
        artifacts_active,
        reasoning_visible,
        side_width: side_w,
        idle_agent_height_cap: idle_agent_cap,
        backdrop,
        join,
    };
    let target_plan = crate::surfaces::plan_cockpit_surfaces(layout_input);
    let target_height = target_plan.agent_frame().map_or(0, |r| r.height);
    let animate = target_plan.agent_frame().is_some()
        && target_plan.artifacts_frame().is_some()
        && App::side_column_visuals_allowed()
        && app.terminal_focused
        && app.selection.is_none()
        && app.last_resize_at.elapsed() >= std::time::Duration::from_millis(180);
    let height = app.divider_motion.height(
        target_height,
        cockpit_area.height,
        Instant::now(),
        app.visual_motion,
        animate,
    );
    let plan = crate::surfaces::plan_cockpit_surfaces_at_height(layout_input, Some(height));

    let left_area = plan.transcript_frame().unwrap_or(cockpit_area);
    let agent_area = plan.agent_frame();
    let artifacts_area = plan.artifacts_frame();

    // Register each panel's outer framed rect before rendering its contents.
    app.panel_frames
        .push(panels::PanelKind::Transcript, left_area);
    app.module_host.set_rect("core", left_area);
    if let Some(area) = agent_area {
        app.panel_frames.push(panels::PanelKind::AgentBay, area);
        app.module_host.set_rect("agent", area);
    }
    if let Some(area) = artifacts_area {
        app.panel_frames.push(panels::PanelKind::Artifacts, area);
        app.module_host.set_rect(
            if raytrace_active {
                "graph"
            } else {
                "artifacts"
            },
            area,
        );
    }

    // Side column first, transcript LAST: when panels share a border column the
    // transcript owns the final paint of that junction. Backdrop paint is
    // skipped entirely when `ANGEL_BACKDROP=off` (handled by the lean path above).
    if plan.backdrop_mode.paints_in_process() {
        if let Some(area) = agent_area {
            agent_panel_view::render_agent_bay_in(frame, app, area, chrome);
        }
        if let Some(area) = artifacts_area {
            stage_view::render_artifacts(frame, app, area);
        }
    }
    render_transcript(frame, app, left_area);
    // Clipboard source: live prose only (rail + tool-strip excluded; never
    // side-column cells). AgentBay registers its own text-only viewport.
    register_transcript_copy_pane(app, left_area);
    if plan.backdrop_mode.paints_in_process()
        && let Some(area) = artifacts_area
    {
        app.panes
            .push(mouse::PaneId::Artifacts, mouse::inner_border(area));
    }
}

pub(crate) fn render_selection_and_approval_overlays(frame: &mut Frame, app: &mut App) {
    if app.pending_approval.is_none() {
        // The originating pane owns selection even when the pointer crosses a
        // panel boundary. Rebind only to its CURRENT text viewport: the AgentBay
        // registration excludes portraits/controls, and transcript excludes its
        // rail/tool strip. Missing or disjoint viewports cancel, never fall back
        // to another pane or to a stale rect.
        app.selection = app.selection.take().and_then(|mut sel| {
            if !crate::surfaces::pane_accepts_clipboard(sel.pane) {
                return None;
            }
            let live = app.panes.rect_of(sel.pane)?;
            sel.rebind(live).then_some(sel)
        });
        if let Some(sel) = app.selection {
            let clip = sel.rect;
            if app.copy_requested {
                // Copy and highlight use the same freshly rebound text rect.
                let text = mouse::extract_text_in(frame.buffer_mut(), &sel, clip);
                if text.is_empty() {
                    app.pending_quick_lookup = None;
                    app.pending_clipboard = None;
                } else {
                    // Ordinary copying never starts tutoring.
                    app.pending_clipboard = Some(text);
                }
                app.copy_requested = false;
            }
            if std::mem::take(&mut app.explain_requested) {
                let text = mouse::extract_text_in(frame.buffer_mut(), &sel, clip);
                if !text.trim().is_empty() {
                    app.pending_tutor_selection = Some(text.chars().take(4000).collect());
                }
            }
            if let Some(sel) = app.selection.as_ref() {
                mouse::highlight_in(frame.buffer_mut(), sel, clip);
            }
        } else {
            app.copy_requested = false;
            if std::mem::take(&mut app.explain_requested) {
                app.system_msg(
                    "Select text, then press Ctrl-Alt-E to ask the tutor about it.".to_string(),
                );
            }
        }
        brain_route_view::render_agent_control_menu(frame, app);
        // Loop input owns the keyboard ahead of every picker. Match that
        // precedence in the final compositor pass, independent of Stage route,
        // media overlays, focus, and the side column's available height.
        if let Some(dialog) = app.loop_dialog.as_ref() {
            app.loop_dialog_hits = crate::loop_dialog::render(frame, frame.area(), dialog);
        }
    }

    if let Some(pa) = &app.pending_approval {
        let area = approval_view::modal_area(frame.area(), &pa.prompt, pa.scope_label.as_deref());
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(approval_view::body(&pa.prompt, pa.scope_label.as_deref()))
                .wrap(Wrap { trim: false })
                .block(approval_view::block()),
            area,
        );
    }
}

fn agent_menu_area(
    root: Rect,
    anchor: Option<Rect>,
    desired_width: u16,
    desired_height: u16,
) -> Rect {
    let width = desired_width.min(root.width.saturating_sub(2)).max(1);
    let height = desired_height.min(root.height.saturating_sub(2)).max(1);
    let max_x = root.x + root.width.saturating_sub(width);
    let max_y = root.y + root.height.saturating_sub(height);
    let x = anchor
        .map(|area| area.x.min(max_x))
        .unwrap_or_else(|| root.x + root.width.saturating_sub(width) / 2);
    let y = anchor
        .map(|area| {
            let below = area.y.saturating_add(area.height);
            if below.saturating_add(height) <= root.y.saturating_add(root.height) {
                below
            } else {
                area.y.saturating_sub(height).max(root.y)
            }
        })
        .unwrap_or_else(|| root.y + root.height.saturating_sub(height) / 2)
        .min(max_y);
    Rect::new(x, y, width, height)
}

/// Opaque floating deck anchored to the MODEL/THINK rail. It is intentionally
/// terminal-native: exact runtime routes, exact model-supported effort levels,
/// disabled offline rows, and full keyboard parity.
/// `ANGEL_AGENT_INFO_HEADER` is launch config: read once per process. Tests
/// toggle it at runtime, so the test build keeps the live read (same rule as
/// `media::web_port`).
#[cfg(not(test))]
fn agent_info_header_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(env_agent_info_header)
}

#[cfg(test)]
fn agent_info_header_enabled() -> bool {
    env_agent_info_header()
}

fn env_agent_info_header() -> bool {
    std::env::var("ANGEL_AGENT_INFO_HEADER")
        .is_ok_and(|v| !matches!(v.trim(), "" | "0" | "off" | "false" | "no"))
}

pub(crate) fn ui(frame: &mut Frame, app: &mut App) {
    app.startup_intro.begin_frame(
        !app.messages.is_empty()
            || !app.partial.is_empty()
            || app.thinking.is_some()
            || app.pending_turn.is_some()
            || !App::side_column_visuals_allowed(),
        !app.input.is_empty(),
        app.visual_motion,
    );
    app.overwatch.refresh();
    let root = frame.area();
    // Visual panes re-earn visibility every frame. Reset before the zero-area
    // return too: a resize, shell, image, or compact Core frame must pause
    // hidden reveal lifetime and cannot retain the preceding fast tick.
    app.world_pane_visible = false;
    app.scryglass.begin_frame();
    app.panes.clear();
    if root.width == 0 || root.height == 0 {
        app.scryglass_drag = None;
        app.scryglass.finish_frame();
        app.viewer.clear_still();
        return;
    }
    // The render paths register their selectable content rects as they lay them out.
    app.agent_buttons.clear();
    app.agent_menu_hits.clear();
    app.agent_menu_area = None;
    app.agent_control_area = None;
    app.header_portrait_area = None;
    app.header_card_area = None;
    app.bay_portrait_area = None;
    app.agent_info_area = None;
    // Likewise rebuild the panel-frame registry used for border repair.
    app.panel_frames.clear();
    // Button hitboxes are frame-local. In particular, a focused game surface
    // must not inherit a stale Scryglass action from the preceding draw.
    app.world_buttons.clear();
    app.loop_dialog_hits.clear();
    // One route-control snapshot serves every rail this frame (header strip,
    // header rail, agent bay) instead of re-taking club locks per rail.
    let chrome = FrameChrome::compute(&app.bag);
    if app.last_root_area != Some(root) {
        app.last_root_area = Some(root);
        app.last_resize_at = Instant::now();
    }
    // Condensed chrome: panels overlap by one cell so neighbors SHARE a single
    // border line (no gaps, no outer margin, no double borders). The junction
    // pass at the end of this fn repairs the shared corners into ├ ┤ ┬ ┴ ┼.
    let gap = panel_gap(root.width, root.height);
    let framed = root;
    // The header and footer are ONE combined box. With vertical room the header
    // is a bordered box holding the status line + keybind legend; its bottom
    // border is shared with the cockpit body below. On short terminals it is a
    // flat status row whose second row IS the body's top border (header_h = 2
    // keeps the row budget identical to the old flat strip).
    let box_chrome = gap > 0 && framed.height >= CHROME_BOX_MIN_HEIGHT;
    // Off by default (2026-07-22 operator request): the 6-row host/telemetry
    // info header never earned its space — the bay caption carries identity and
    // the bay footer carries telemetry. ANGEL_AGENT_INFO_HEADER=1 restores it.
    // A2: when ANGEL_BACKDROP=off the side bay is gone — do not spend layout or
    // paint on a side-column header plate that only mirrors that bay.
    let agent_info_header = box_chrome
        && framed.width >= 100
        && framed.height >= 30
        && app.module_host.is_running("agent")
        && app.scryglass_enabled
        && agent_info_header_enabled()
        && crate::surfaces::BackdropMode::from_env().shows_side_column();
    // Thin default: one content row (angel0/version, route chips, workspace).
    // The retired 6-row info header (`ANGEL_AGENT_INFO_HEADER=1`) keeps its split.
    let header_h = if box_chrome {
        if agent_info_header {
            6
        } else {
            agent_panel_view::HEADER_PORTRAIT_BOX_H
        }
    } else {
        2
    };
    let sections = Layout::vertical([
        Constraint::Length(header_h),
        Constraint::Min(1),
        Constraint::Length(status_view::composer_height(
            &app.input,
            framed.width,
            framed.height.saturating_sub(header_h),
        )),
    ])
    .spacing(-1)
    .split(framed);
    let header_area = sections[0];
    let cockpit_area = sections[1];
    let message_area = sections[2];

    let header_inner = frame_panel(
        frame,
        app,
        header_area,
        box_chrome,
        panels::PanelKind::Header,
    );
    app.module_host.set_rect("core", cockpit_area);
    if agent_info_header {
        let side_w = cockpit_side_width(cockpit_area.width);
        let divider_x = header_area.x + header_area.width.saturating_sub(side_w);
        let header_right = header_inner.x + header_inner.width;
        let left_area = Rect::new(
            header_inner.x,
            header_inner.y,
            divider_x.saturating_sub(header_inner.x),
            header_inner.height,
        );
        let info_area = Rect::new(
            divider_x.saturating_add(1),
            header_inner.y,
            header_right.saturating_sub(divider_x.saturating_add(1)),
            header_inner.height,
        );
        render_combined_header(frame, app, left_area, &chrome);
        app.agent_info_area = Some(info_area);
        render_agent_header_info(frame, app, info_area);
        frame.render_widget(
            Paragraph::new(
                (0..header_inner.height)
                    .map(|_| Line::from("│"))
                    .collect::<Vec<_>>(),
            )
            .style(Style::new().fg(HUD_DIM)),
            Rect::new(divider_x, header_inner.y, 1, header_inner.height),
        );
    } else {
        render_combined_header(frame, app, header_inner, &chrome);
    }

    // Register the input box's frame rect before the body lays out. Its content
    // rect is hit-tested below.
    app.panel_frames
        .push(panels::PanelKind::Input, message_area);

    if app.shell_focused && app.shell.is_some() {
        // The shell owns the whole cockpit body — its own floating box.
        app.panel_frames
            .push(panels::PanelKind::Shell, cockpit_area);
        app.module_host.set_rect("shell", cockpit_area);
        let block = Block::default()
            .borders(Borders::ALL)
            .style(panel_style())
            .border_style(HUD_BLUE_BORDER_STYLE)
            .title(status_view::shell_focus_title());
        let inner = block.inner(cockpit_area);
        frame.render_widget(block, cockpit_area);
        app.resize_shell_to_area(inner);
        if let Some(shell) = app.shell.as_ref() {
            shell.render(frame, inner);
        }
        // The shell's content rect is selectable too (used only when the PTY
        // program isn't tracking the mouse — see `on_mouse`).
        app.panes.push(mouse::PaneId::Shell, inner);
    } else if app.viewer.has_image() {
        // `/show` loaded an image → full-area view until `/hide`.
        app.panel_frames
            .push(panels::PanelKind::Image, cockpit_area);
        app.module_host.set_rect("image", cockpit_area);
        let inner = {
            let title = app.image_viewer_title();
            let block = Block::default()
                .borders(Borders::ALL)
                .style(panel_style())
                .border_style(HUD_BLUE_BORDER_STYLE)
                .title(title.as_ref());
            let inner = block.inner(cockpit_area);
            frame.render_widget(block, cockpit_area);
            inner
        };
        app.viewer.render(frame, inner);
    } else if app.scryglass_enabled
        && app.research.expanded
        && app
            .scryglass
            .controller
            .resolved_scene(false, false, app.world.quest_owns_pane())
            == crate::scryglass::StageSurface::Research
        && app
            .module_host
            .focused()
            .is_some_and(|id| id.as_str() == "artifacts")
        && crate::surfaces::BackdropMode::from_env().paints_in_process()
    {
        app.panel_frames
            .push(panels::PanelKind::Artifacts, cockpit_area);
        app.module_host.set_rect("artifacts", cockpit_area);
        stage_view::render_artifacts(frame, app, cockpit_area);
        app.panes
            .push(mouse::PaneId::Artifacts, mouse::inner_border(cockpit_area));
    } else if root.width < 100 || root.height < 30 {
        // On compact terminals F3/F4 become true full-body focus modes. This
        // keeps the portrait and Scryglass useful instead of leaving the focus
        // key pointed at a side column the layout has collapsed away.
        // A2: ANGEL_BACKDROP=off still means no backdrop paint — even on compact
        // agent/artifacts focus, so the text path stays lean and consistent.
        let paint_backdrops =
            app.scryglass_enabled && crate::surfaces::BackdropMode::from_env().paints_in_process();
        match app.module_host.focused().map(|id| id.as_str()) {
            Some("artifacts") if paint_backdrops => {
                app.panel_frames
                    .push(panels::PanelKind::Artifacts, cockpit_area);
                app.module_host.set_rect("artifacts", cockpit_area);
                stage_view::render_artifacts(frame, app, cockpit_area);
                app.panes
                    .push(mouse::PaneId::Artifacts, mouse::inner_border(cockpit_area));
            }
            Some("agent") if paint_backdrops => {
                app.panel_frames
                    .push(panels::PanelKind::AgentBay, cockpit_area);
                app.module_host.set_rect("agent", cockpit_area);
                agent_panel_view::render_agent_bay_in(frame, app, cockpit_area, &chrome);
            }
            _ => {
                app.panel_frames
                    .push(panels::PanelKind::Transcript, cockpit_area);
                app.module_host.set_rect("core", cockpit_area);
                render_transcript(frame, app, cockpit_area);
                // A6: compact Core must register the same prose-only copy rect as
                // the wide path (exclude scroll rail + live tool strip). Full
                // inner_border used to let selection sample chrome cells.
                register_transcript_copy_pane(app, cockpit_area);
            }
        }
    } else {
        render_cockpit_with_agent_panels(frame, app, cockpit_area, &chrome);
    }

    // The bottom strip (input / working metrics / shell hint) is its own floating
    // box. Register its chrome-free content rect for mouse hit-testing now that
    // the body has laid out.
    app.panes
        .push(mouse::PaneId::Input, mouse::inner_border(message_area));

    if app.shell_focused {
        let hint = Paragraph::new(status_view::shell_hint())
            .style(dim_panel_style())
            .block(hud_block(" shell "));
        frame.render_widget(hint, message_area);
    } else {
        render_message_composer(frame, app, message_area);
    }

    merge_frame_borders(frame, app, box_chrome);
    // Formation is an operator control surface, not a Stage route that a
    // resident image can cover. Paint it after every body pane and rebuild
    // hit targets for the visible board rather than the covered world.
    if app.moa_deck_owns_input() {
        app.world_pane_visible = false;
        app.scryglass.begin_frame();
        app.scryglass.surface = crate::scryglass::StageSurface::Moa;
        app.agent_buttons
            .retain(|(rect, _)| !rect.intersects(cockpit_area));
        app.world_buttons.clear();
        app.panes.clear();
        app.panes
            .push(mouse::PaneId::Artifacts, mouse::inner_border(cockpit_area));
        app.module_host.set_rect("artifacts", cockpit_area);
        frame.render_widget(Clear, cockpit_area);
        render_moa_deck(frame, app, cockpit_area);
    }
    render_selection_and_approval_overlays(frame, app);
    // The frame that carries a just-submitted turn's echo has now been built;
    // `advance` may launch the deferred pre-flight on the next tick.
    if let Some(pending) = app.pending_turn.as_mut() {
        pending.echo_drawn = true;
    }
    app.scryglass.finish_frame();
    // Validate against the completed frame registry, not halfway through
    // layout while panes have been cleared but not yet re-registered.
    if !app.scryglass.visible
        || !app.scryglass.renderable
        || app.scryglass_drag.is_some_and(|drag| {
            Some(drag.rect) != app.panes.rect_of(mouse::PaneId::Artifacts)
                || drag.surface != app.scryglass.surface
                || app
                    .module_host
                    .focused()
                    .is_none_or(|id| id.as_str() != "artifacts")
        })
    {
        app.scryglass_drag = None;
    }
    if !app.still_active() {
        app.viewer.clear_still();
    }
    app.viewer.flush_dot_upload(frame);
    app.startup_intro.flush_upload(frame);
}

thread_local! {
    // Junction repair walks a handful of panel boxes every frame. Keep the
    // filtered Rect list in TLS so a steady draw does not heap-allocate it.
    static MERGE_RECTS: std::cell::RefCell<Vec<Rect>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Run the collapsed-border junction repair over this frame's panel boxes.
/// The header frame only participates when it actually drew a border (a flat
/// header strip must not inject phantom junction arms into its neighbor).
fn merge_frame_borders(frame: &mut Frame, app: &App, header_boxed: bool) {
    MERGE_RECTS.with(|scratch| {
        let mut rects = scratch.borrow_mut();
        rects.clear();
        rects.extend(
            app.panel_frames
                .iter()
                .filter(|(kind, _)| header_boxed || *kind != panels::PanelKind::Header)
                .map(|(_, r)| r),
        );
        hud::merge_panel_borders(frame.buffer_mut(), &rects);
    });
}

pub(crate) fn render_message_composer(frame: &mut Frame, app: &mut App, area: Rect) {
    let intent = status_view::composer_intent(&app.input);
    let busy = app.thinking.is_some() || app.bg_job.is_some();
    let steerable = matches!(
        intent,
        status_view::ComposerIntent::Empty | status_view::ComposerIntent::Message
    );
    let composer_selection = app.composer_selection_range();
    // Staged Ctrl-V screenshots ride the composer title. The removal hint holds
    // only while Esc has no interrupt or cancel to make (that is when Esc
    // removes them).
    let attachment_chip = app.clipboard_paste.chip(!busy);
    // Steer hint rides the title while a turn/bg job holds the slot (steer.rs).
    let mut title = status_view::composer_frame_title(
        intent,
        area.width.saturating_sub(2) as usize,
        status_view::ComposerTitleState {
            busy,
            steerable,
            vim_normal: app.vim_mode.then_some(app.vim_normal),
            selection_len: composer_selection.map(|(start, end)| end - start),
            steer_queued: app.steer_queue.len(),
            moa_chip: app.moa_arm_chip().as_deref(),
            attachment_chip: attachment_chip.as_deref(),
        },
    );
    if let Some(draft) = &app.tutor_draft {
        title = format!(" Tutor · {} · Esc returns to work ", draft.name).into();
    }
    // A live response can push a transcript acknowledgement off screen. Keep
    // operator cancellation visible in the fixed composer chrome while the
    // worker settles, without changing the worker's drain semantics.
    if let Some(thinking) = app.thinking.as_ref()
        && thinking.cancel.load(std::sync::atomic::Ordering::Relaxed)
    {
        let acknowledgement = if thinking.is_draining() {
            "hard-stopped"
        } else {
            "interrupting"
        };
        let existing = title.trim().strip_prefix("A> ").unwrap_or(title.trim());
        title = if existing.is_empty() {
            format!(" A> {acknowledgement} ").into()
        } else {
            format!(" A> {acknowledgement} · {existing} ").into()
        };
    }
    let inner_w = area.width.saturating_sub(2) as usize;
    let inner_h = area.height.saturating_sub(2);
    let cursor = app.cursor;
    let input = app.input.as_str();
    let mut composer: status_view::ComposerView<'_> = status_view::composer_view_with_selection(
        input,
        inner_w,
        inner_h,
        cursor,
        composer_selection,
    );
    let cursor_position = composer_cursor_position(app, area, &composer);
    // Dim ghost completion while composing a bare slash command (Tab still
    // commits). Also covers the old /loop and /handoff-rl usage ghosts.
    if app.tutor_draft.is_none()
        && input.starts_with('/')
        && cursor == input.chars().count()
        && !input.chars().any(char::is_whitespace)
    {
        if let Some(first) = composer.lines.get_mut(0) {
            *first = input_line_with_slash_ghost(input, input, cursor);
        }
    } else if intent == status_view::ComposerIntent::Empty {
        if composer.lines.is_empty() {
            composer.lines.push(if app.tutor_draft.is_some() {
                Line::from(Span::styled("Ask a question…", Style::new().fg(HUD_DIM)))
            } else {
                status_view::composer_placeholder(busy, inner_w)
            });
        } else if let Some(first) = composer.lines.get_mut(0) {
            *first = if app.tutor_draft.is_some() {
                Line::from(Span::styled("Ask a question…", Style::new().fg(HUD_DIM)))
            } else {
                status_view::composer_placeholder(busy, inner_w)
            };
        }
        // Empty drafts only need the placeholder row; drop any residual rows so
        // a previously multi-line ghost cannot keep a second bright line.
        composer.lines.truncate(1);
    }
    // Full-width pad: short lines must rewrite every cell. Without this, a
    // cleared draft leaves the right half of a prior long message glowing in
    // the composer (bright teal residue under the dim "Add guidance…" hint).
    status_view::pad_composer_lines(&mut composer.lines, inner_w);
    // Fill unused rows of the composer pane with blank padded lines so a
    // multi-line draft that shrinks cannot leave lower-row ghosts either.
    while composer.lines.len() < inner_h as usize {
        composer.lines.push(Line::from(Span::styled(
            status_view::pad_composer_row("", inner_w),
            Style::new().fg(crate::hud::HUD_TEXT),
        )));
    }

    // The composer frame is a state rail: blue means a message is ready, gold
    // means a local command, coral asks for a deliberate second look, and live
    // phosphor means Enter will steer the running agent. Critical emphasis uses
    // opacity only and settles completely when motion is reduced or disabled.
    let (mut border_style, mut title_style) = match intent {
        status_view::ComposerIntent::Empty => (
            Style::new().fg(HUD_DIM),
            Style::new().fg(HUD_BLUE).add_modifier(Modifier::BOLD),
        ),
        status_view::ComposerIntent::Message => (
            HUD_BLUE_BORDER_STYLE,
            Style::new().fg(HUD_BLUE).add_modifier(Modifier::BOLD),
        ),
        status_view::ComposerIntent::Command => (
            Style::new().fg(hud::HUD_GOLD),
            Style::new().fg(hud::HUD_GOLD).add_modifier(Modifier::BOLD),
        ),
        status_view::ComposerIntent::CriticalCommand => (
            Style::new().fg(hud::HUD_DANGER),
            Style::new()
                .fg(hud::HUD_DANGER)
                .add_modifier(Modifier::BOLD),
        ),
    };
    if busy && intent != status_view::ComposerIntent::CriticalCommand {
        border_style = Style::new().fg(HUD_PHOSPHOR);
        title_style = PHOSPHOR_BOLD_STYLE;
    }
    if intent == status_view::ComposerIntent::CriticalCommand
        && app.visual_motion == lifecycle_viz::MotionMode::Full
        && (app
            .visual_motion
            .chrome_elapsed(app.started, app.started)
            .as_millis()
            / 450)
            .is_multiple_of(2)
    {
        border_style = border_style.add_modifier(Modifier::DIM);
        title_style = title_style.add_modifier(Modifier::DIM);
    }
    let mut block = hud_block(title.as_ref())
        .border_style(border_style)
        .title_style(title_style);
    if title.trim().is_empty() {
        block = hud_block("")
            .border_style(border_style)
            .title_style(title_style);
    }
    let input = Paragraph::new(composer.lines)
        .style(panel_style())
        .block(block);
    frame.render_widget(input, area);
    if let Some(position) = cursor_position {
        frame.set_cursor_position(position);
    }
}

/// Publish a caret only while the composer owns text input. Ratatui hides the
/// terminal cursor when a frame does not set a position, so modal surfaces and
/// an empty focused Stage deliberately return `None`.
pub(crate) fn composer_cursor_position(
    app: &App,
    area: Rect,
    composer: &status_view::ComposerView<'_>,
) -> Option<ratatui::layout::Position> {
    let inner_width = area.width.saturating_sub(2);
    let inner_height = area.height.saturating_sub(2);
    if inner_width == 0 || inner_height == 0 || !app.terminal_focused {
        return None;
    }
    if app.pending_approval.is_some()
        || app.agent_menu.is_some()
        || app.moa_deck_owns_input()
        || app.loop_dialog.is_some()
        || app.shell_focused
    {
        return None;
    }
    let stage_focused = app
        .module_host
        .focused()
        .is_some_and(|id| id.as_str() == "artifacts");
    if stage_focused && app.input.is_empty() {
        return None;
    }

    let (cursor_col, cursor_row) = if composer.cursor_col >= inner_width
        && composer.cursor_row.saturating_add(1) < inner_height
    {
        // An insertion point immediately after an exact-width row belongs at
        // the start of the next row. Clamping it onto the last occupied cell
        // makes the caret visibly cover the final character.
        (0, composer.cursor_row + 1)
    } else {
        (
            composer.cursor_col.min(inner_width - 1),
            composer.cursor_row.min(inner_height - 1),
        )
    };
    Some(ratatui::layout::Position::new(
        area.x.saturating_add(1).saturating_add(cursor_col),
        area.y.saturating_add(1).saturating_add(cursor_row),
    ))
}

/// Reserve one row for a persistent escape hatch on visual/game surfaces.
/// Keeping the footer outside the scene prevents a back label from painting
/// over braille art or a progress line.
fn visual_panel_body(inner: Rect) -> (Rect, Option<Rect>) {
    if inner.height < 2 {
        return (inner, None);
    }
    let body = Rect {
        height: inner.height - 1,
        ..inner
    };
    let footer = Rect {
        y: inner.y + inner.height - 1,
        height: 1,
        ..inner
    };
    (body, Some(footer))
}

fn render_panel_back(frame: &mut Frame, app: &mut App, footer: Rect) {
    render_scene_caption(
        frame,
        app,
        footer,
        Line::from(""),
        &[("Back", WorldButton::Back)],
    );
}

pub(crate) fn test_backend_text(backend: &TestBackend) -> String {
    let buffer = backend.buffer();
    let area = *buffer.area();
    let width = area.width as usize;
    let height = area.height as usize;
    let mut out = String::with_capacity((width + 1) * height);
    for row in buffer.content().chunks(width) {
        let visible = row
            .iter()
            .rposition(|cell| cell.symbol() != " ")
            .map(|idx| idx + 1)
            .unwrap_or(0);
        if visible == 0 {
            out.push('\n');
            continue;
        }
        for cell in &row[..visible] {
            out.push_str(cell.symbol());
        }
        out.push('\n');
    }
    out
}

fn render_preview_with_viewer(width: u16, height: u16, viewer: Viewer) -> std::io::Result<String> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("TestBackend is infallible");
    let mut app = App::preview(viewer);
    terminal
        .draw(|frame| ui(frame, &mut app))
        .expect("TestBackend is infallible");
    Ok(test_backend_text(terminal.backend()))
}

pub(crate) fn render_preview_text(width: u16, height: u16) -> std::io::Result<String> {
    render_preview_with_viewer(width, height, Viewer::static_preview())
}

pub(crate) fn render_portrait_preview_text(width: u16, height: u16) -> std::io::Result<String> {
    render_preview_with_viewer(width, height, Viewer::portrait_preview())
}

/// Headless screenshot aid for the three Reinforce evidence lenses. The
/// deterministic record is visibly marked illustrative in every non-branch
/// view and must never be presented as benchmark evidence.
pub(crate) fn render_rl_preview_text(
    view: crate::rl_viz::RlView,
    width: u16,
    height: u16,
) -> std::io::Result<String> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("TestBackend is infallible");
    let mut app = App::preview(Viewer::static_preview());
    let _ = app
        .module_host
        .focus(&crate::runtime::ModuleId::new("artifacts"));
    app.scryglass
        .navigate(crate::scryglass::StageRoute::Reinforce);
    app.rl_view = view;
    app.tools.rl().mode = crate::rl_ctl::RlMode::Campaign;
    {
        let rl = app.tools.rl();
        let mut progress = rl.progress.lock().unwrap_or_else(|lock| lock.into_inner());
        progress.planned_attempts = 81;
        progress.replace_points(
            (0..=80)
                .map(|index| {
                    let phase = index as f32 / 80.0;
                    let reward =
                        (0.14 + phase * 0.78 + (phase * 28.0).sin() * 0.035).clamp(0.0, 1.0);
                    crate::rl_ctl::RunPoint {
                        step: index * 5,
                        reward,
                        latency_ms: 4_000 + (index as u64 * 37) % 900,
                    }
                })
                .collect(),
        );
        progress.outcome = Some(Ok(crate::rl_ctl::CampaignOutcome {
            attempted: 81,
            passed: 74,
            red: 7,
            rounds: 1,
            promoted_rounds: 1,
            policy_version: 1,
            decision: "promoted".to_string(),
            mean_delta: Some(0.50),
            validated: true,
            audit_supplied: true,
            release_sha256: Some("0".repeat(64)),
            solve_rate: Some(0.90),
            advantage_variance: Some(0.0625),
            reflection: true,
            accepted_entry: Some("rl-policy".to_string()),
            accepted_event: Some("preview".to_string()),
            report_path: "illustrative://reinforce/panel.json".to_string(),
            route: crate::club::RouteIdentity {
                driver: "illustrative".to_string(),
                model: None,
                reasoning_effort: None,
            },
            wall_s: 184.0,
        }));
        progress
            .log_tail
            .push_back("ILLUSTRATIVE RL PANEL · replace with a live /rl record".to_string());
    }
    app.statusline =
        Some("ILLUSTRATIVE RL PANEL · use measured /rl evidence for external claims".to_string());
    terminal
        .draw(|frame| ui(frame, &mut app))
        .expect("TestBackend is infallible");
    Ok(test_backend_text(terminal.backend()))
}

pub(crate) fn render_research_preview_text(
    lens: crate::research_workspace::Lens,
    width: u16,
    height: u16,
) -> std::io::Result<String> {
    use crate::harness::{ExecutionOutcome, ToolEventId, ToolOutcome, VerificationOutcome};
    let mut app = App::preview(Viewer::static_preview());
    app.open_research(None);
    app.research.expanded = true;
    app.research.illustrative = true;
    let operations = [
        (
            "read_file",
            "TASK.md · establish the optimization target",
            "Baseline and supported shapes inspected",
            VerificationOutcome::NotApplicable,
        ),
        (
            "delegate",
            "Council · compare memory-layout hypotheses",
            "Three approaches returned with tradeoffs",
            VerificationOutcome::NotApplicable,
        ),
        (
            "apply_patch",
            "candidate_a.cu · coalesced loads",
            "Candidate A written",
            VerificationOutcome::NotApplicable,
        ),
        (
            "run_tests",
            "candidate_a.cu · correctness suite",
            "64 cases passed",
            VerificationOutcome::Passed,
        ),
        (
            "run_tests",
            "candidate_b.cu · boundary shapes",
            "Shape 4097 failed numerical tolerance",
            VerificationOutcome::Failed,
        ),
        (
            "shell",
            "benchmark candidate_a.cu",
            "Illustrative timing: 18.2 µs baseline → 15.7 µs candidate",
            VerificationOutcome::NotApplicable,
        ),
        (
            "research",
            "Review candidate A measurements",
            "Compare repeated observations before selecting an incumbent",
            VerificationOutcome::NotApplicable,
        ),
    ];
    for (index, (name, args, result, verification)) in operations.into_iter().enumerate() {
        let id = ToolEventId(format!("illustrative-{index}"));
        app.research.start(&id, name, args);
        app.research.result(
            &id,
            name,
            result,
            ToolOutcome {
                execution: ExecutionOutcome::Succeeded,
                verification,
            },
        );
    }
    app.research.start(
        &ToolEventId("illustrative-active".into()),
        "cargo",
        "Validate candidate C · alternative tile layout",
    );
    app.refresh_research();
    app.research
        .action(crate::research_workspace::Action::Lens(lens));
    app.research.following = false;
    app.research.selected = app.research.visible.first().map(|entry| entry.id.clone());
    app.statusline = Some("ILLUSTRATIVE RESEARCH WORKSPACE · no benchmark claim".into());
    let mut terminal =
        Terminal::new(TestBackend::new(width, height)).expect("TestBackend is infallible");
    terminal
        .draw(|frame| ui(frame, &mut app))
        .expect("TestBackend is infallible");
    Ok(test_backend_text(terminal.backend()))
}

pub(crate) fn render_scryglass_preview_text(
    path: &str,
    width: u16,
    height: u16,
) -> std::io::Result<String> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("TestBackend is infallible");
    let mut app = App::preview(Viewer::portrait_preview());
    let _ = app
        .module_host
        .focus(&crate::runtime::ModuleId::new("artifacts"));
    let card = crate::media::visual_from_path(path)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    app.media.push(card);
    app.scryglass.reveal_media(0, true);
    for _ in 0..80 {
        terminal
            .draw(|frame| ui(frame, &mut app))
            .expect("TestBackend is infallible");
        if app.scryglass.media_ready() {
            app.scryglass.settle_reveal_for_preview();
            terminal
                .draw(|frame| ui(frame, &mut app))
                .expect("TestBackend is infallible");
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(test_backend_text(terminal.backend()))
}

/// One noir caption row: the scene caption on the left, right-aligned verb
/// buttons (brackets dim, words bright) with hitboxes registered on
/// `app.world_buttons`.
fn fitted_scene_verbs<'a>(
    width: u16,
    verbs: &[(&'a str, WorldButton)],
) -> (Vec<(&'a str, WorldButton)>, u16) {
    let verb_width = |label: &str| {
        unicode_width::UnicodeWidthStr::width(label)
            .saturating_add(2)
            .min(u16::MAX as usize) as u16
    };
    let total = verbs
        .iter()
        .map(|(label, _)| verb_width(label))
        .sum::<u16>()
        + verbs.len().saturating_sub(1).min(u16::MAX as usize) as u16;
    let verbs_fit = total.saturating_add(2) <= width;
    let back = verbs
        .iter()
        .rev()
        .find(|(label, _)| *label == "Back")
        .copied();
    let back_total = back.map(|(label, _)| verb_width(label)).unwrap_or(0);
    let back_only = !verbs_fit && back.is_some() && back_total.saturating_add(2) <= width;
    let shown = if verbs_fit {
        verbs.to_vec()
    } else if back_only {
        vec![back.expect("back_only guarantees a back verb")]
    } else {
        Vec::new()
    };
    let shown_total = shown
        .iter()
        .map(|(label, _)| verb_width(label))
        .sum::<u16>()
        + shown.len().saturating_sub(1).min(u16::MAX as usize) as u16;
    (shown, shown_total)
}

fn scene_caption_budget(width: u16, verbs: &[(&str, WorldButton)]) -> usize {
    let (shown, shown_total) = fitted_scene_verbs(width, verbs);
    if shown.is_empty() {
        width as usize
    } else {
        width.saturating_sub(shown_total).saturating_sub(1) as usize
    }
}

fn render_scene_caption(
    frame: &mut Frame,
    app: &mut App,
    caption_rect: Rect,
    caption: Line<'static>,
    verbs: &[(&str, WorldButton)],
) {
    use crate::hud::{HUD_DIM, HUD_TEXT};
    // On a narrow game pane, keep the escape hatch even when the richer verb
    // set cannot fit. Without this fallback the caption row silently drops all
    // hitboxes—the exact state that leaves a mouse user trapped in the panel.
    let (shown_verbs, shown_total) = fitted_scene_verbs(caption_rect.width, verbs);
    // The verbs are painted over the right end of this same row: clip the
    // caption to the space left of them (one-column gap, dim ellipsis) or a
    // long caption gets overwritten mid-word by the action rail.
    let caption_max = scene_caption_budget(caption_rect.width, verbs);
    frame.render_widget(
        Paragraph::new(clip_caption(caption, caption_max)),
        caption_rect,
    );
    if shown_verbs.is_empty() {
        return;
    }
    let mut x = caption_rect.x + caption_rect.width - shown_total;
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (index, &(label, button)) in shown_verbs.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw(" "));
            x += 1;
        }
        let width = unicode_width::UnicodeWidthStr::width(label)
            .saturating_add(2)
            .min(u16::MAX as usize) as u16;
        app.world_buttons.push((
            Rect {
                x,
                y: caption_rect.y,
                width,
                height: 1,
            },
            button,
        ));
        spans.push(Span::styled("[", Style::new().fg(HUD_DIM)));
        spans.push(Span::styled(label.to_string(), Style::new().fg(HUD_TEXT)));
        spans.push(Span::styled("]", Style::new().fg(HUD_DIM)));
        x += width;
    }
    let verbs_rect = Rect {
        x: caption_rect.x + caption_rect.width - shown_total,
        width: shown_total,
        ..caption_rect
    };
    frame.render_widget(Paragraph::new(Line::from(spans)), verbs_rect);
}

/// Truncate a styled line to `max` display columns, ending in a dim `…` when
/// anything was cut. Span styles survive the cut.
fn clip_caption(line: Line<'static>, max: usize) -> Line<'static> {
    let width: usize = line
        .spans
        .iter()
        .map(|span| unicode_width::UnicodeWidthStr::width(span.content.as_ref()))
        .sum();
    if width <= max {
        return line;
    }
    let keep = max.saturating_sub(1);
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;
    for span in line.spans {
        if used >= keep {
            break;
        }
        let mut text = String::new();
        for ch in span.content.chars() {
            text.push(ch);
            if used + unicode_width::UnicodeWidthStr::width(text.as_str()) > keep {
                text.pop();
                break;
            }
        }
        if text.is_empty() {
            break;
        }
        used += unicode_width::UnicodeWidthStr::width(text.as_str());
        spans.push(Span::styled(text, span.style));
    }
    if max > 0 {
        spans.push(Span::styled(
            crate::glyphs::chrome_ellipsis(),
            Style::new().fg(crate::hud::HUD_DIM),
        ));
    }
    Line::from(spans)
}

/// One operational status row beneath the living world. Formation is the sole
/// visible swarm control; the obsolete preset/quorum strip no longer competes
/// with the richer deck or masquerades as game navigation.
fn formation_status_bar(app: &mut App, bar: Rect) -> Line<'static> {
    use crate::hud::{HUD_BLUE, HUD_DIM, HUD_PHOSPHOR};

    let busy = app.thinking.is_some() || app.bg_job.is_some();
    let button = if busy {
        "[Formation:edit]"
    } else {
        "[Formation]"
    };
    let mut spans = vec![Span::styled("◇ ", Style::new().fg(HUD_DIM))];
    spans.push(Span::styled(button, Style::new().fg(HUD_BLUE)));
    spans.push(Span::styled(
        format!(" · {}", app.moa_arm_status()),
        Style::new().fg(if app.moa_one_shot.is_some() || app.moa_session.is_some() {
            HUD_PHOSPHOR
        } else {
            HUD_DIM
        }),
    ));
    app.world_buttons.push((
        Rect {
            x: bar.x + 2,
            y: bar.y,
            width: button.chars().count() as u16,
            height: 1,
        },
        WorldButton::FormationDeck,
    ));
    Line::from(spans)
}

/// Formation list + roster graph. Comp / lean keeps chrome; default still paints.
pub(crate) fn moa_deck_body_allowed() -> bool {
    crate::comp_mode::ambient_stage_sim_allowed()
}

fn render_moa_deck(frame: &mut Frame, app: &mut App, area: Rect) {
    let title = format!(" Agent Formation · {} ", app.moa_arm_status());
    let block = hud_block(title.as_str());
    let inner = block.inner(area);
    let (content_area, footer_area) = visual_panel_body(inner);
    frame.render_widget(block, area);
    if content_area.width < 20 || content_area.height < 5 || !moa_deck_body_allowed() {
        if let Some(footer_area) = footer_area {
            render_panel_back(frame, app, footer_area);
        }
        return;
    }

    let selected_idx = app
        .moa_deck
        .as_ref()
        .map(|deck| deck.selected_index())
        .unwrap_or(0);
    let deck = crate::formations::built_in_deck();
    let selected = deck[selected_idx.min(deck.len().saturating_sub(1))];
    let (list_area, detail_area) = if content_area.width >= 58 {
        let [list_area, detail_area] =
            Layout::horizontal([Constraint::Length(22), Constraint::Min(24)])
                .spacing(1)
                .areas(content_area);
        (list_area, detail_area)
    } else {
        let list_h = (deck.len() as u16 + 1).min(content_area.height.saturating_sub(3).max(1));
        let [list_area, detail_area] =
            Layout::vertical([Constraint::Length(list_h), Constraint::Min(1)]).areas(content_area);
        (list_area, detail_area)
    };

    render_moa_deck_list(frame, app, list_area, selected_idx);
    render_moa_deck_detail(frame, app, detail_area, selected);
    if let Some(footer_area) = footer_area {
        render_scene_caption(
            frame,
            app,
            footer_area,
            Line::from(""),
            &[
                ("Engage next", WorldButton::ArmFormationTurn),
                ("Engage session", WorldButton::ArmFormationSession),
                ("Clear", WorldButton::ClearFormation),
                ("Back", WorldButton::Back),
            ],
        );
    }
}

fn render_moa_deck_list(frame: &mut Frame, app: &mut App, area: Rect, selected_idx: usize) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let deck = crate::formations::built_in_deck();
    // Keep the engage hint on the last row when possible, and scroll the
    // formation window so the selected card (including the last Grok War seat)
    // always lands inside the clickable region.
    let hint_rows = usize::from(area.height > 1);
    let list_rows = (area.height as usize)
        .saturating_sub(hint_rows)
        .max(1)
        .min(deck.len().max(1));
    let max_scroll = deck.len().saturating_sub(list_rows);
    let scroll = selected_idx
        .saturating_sub(list_rows.saturating_sub(1))
        .min(max_scroll);
    let lines = moa_deck_list_lines(selected_idx, scroll, list_rows, hint_rows > 0);
    frame.render_widget(Paragraph::new(lines), area);
    for (row, formation) in deck.iter().skip(scroll).take(list_rows).enumerate() {
        app.world_buttons.push((
            Rect {
                x: area.x,
                y: area.y + row as u16,
                width: area.width,
                height: 1,
            },
            WorldButton::SelectFormation(formation.id),
        ));
    }
}

fn moa_deck_list_lines(
    selected_idx: usize,
    scroll: usize,
    list_rows: usize,
    show_hint: bool,
) -> Vec<Line<'static>> {
    use ratatui::style::Color;

    let deck = crate::formations::built_in_deck();
    let mut lines = Vec::new();
    for (offset, formation) in deck.iter().skip(scroll).take(list_rows).enumerate() {
        let idx = scroll + offset;
        let selected = idx == selected_idx;
        let style = if selected {
            Style::new().fg(Color::Black).bg(HUD_PHOSPHOR)
        } else {
            Style::new().fg(HUD_BLUE)
        };
        let marker = if selected { "> " } else { "  " };
        lines.push(Line::from(vec![
            Span::styled(marker, style),
            Span::styled(format!("{:<12}", formation.name), style),
            Span::styled(
                format!(" {}", formation.est_cost_label()),
                if selected {
                    style
                } else {
                    Style::new().fg(HUD_DIM)
                },
            ),
        ]));
    }
    if show_hint {
        lines.push(Line::from(Span::styled(
            "↑↓ · Tab roster · Enter/N next · S session",
            Style::new().fg(HUD_DIM),
        )));
    }
    lines
}

fn render_moa_deck_detail(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    formation: crate::formations::Formation,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let App {
        moa_deck,
        world_buttons,
        ..
    } = app;
    let Some(deck) = moa_deck.as_ref() else {
        return;
    };
    let roster = deck.selected_roster();
    let focus = deck.focus();
    let selected_slot = deck.selected_slot_index();
    let selected_model = deck.selected_model_index();
    let models = deck.models();
    let selected_effort = deck.selected_effort_index();
    let efforts = deck.effort_options();
    let roster_online = deck.selected_roster_online();
    let readiness = if formation.is_resting() {
        "COORDINATOR ONLY".to_string()
    } else if roster.is_ready() && !roster_online {
        "ROUTE OFFLINE".to_string()
    } else if roster.is_ready() {
        format!("READY {}/{}", roster.assigned_count(), roster.slot_count())
    } else {
        format!("NEEDS {}/{}", roster.assigned_count(), roster.slot_count())
    };
    let header = vec![
        Line::from(vec![
            Span::styled(formation.name, PHOSPHOR_BOLD_STYLE),
            Span::styled(
                format!("  {readiness}"),
                Style::new().fg(if roster.is_ready() && roster_online {
                    HUD_PHOSPHOR
                } else {
                    crate::hud::HUD_GOLD
                }),
            ),
        ]),
        Line::from(Span::styled(formation.flavor, Style::new().fg(HUD_DIM))),
        Line::from(vec![
            Span::styled("SPEND  ", Style::new().fg(HUD_DIM)),
            Span::styled(
                format!(
                    "adaptive input · native output · {}",
                    formation.est_cost_label()
                ),
                Style::new().fg(crate::hud::HUD_GOLD),
            ),
            Span::styled(
                format!(
                    " · {} metered/remote · {} local",
                    roster.metered_count(),
                    roster.local_count()
                ),
                Style::new().fg(HUD_DIM),
            ),
        ]),
        Line::from(Span::styled(
            format!("FLOW   {}", formation.stage_summary()),
            Style::new().fg(HUD_BLUE),
        )),
    ];
    let header_h = (header.len() as u16).min(area.height);
    frame.render_widget(
        Paragraph::new(header)
            .wrap(Wrap { trim: false })
            .style(panel_style()),
        Rect::new(area.x, area.y, area.width, header_h),
    );
    let graph_area = Rect::new(
        area.x,
        area.y.saturating_add(header_h),
        area.width,
        area.height.saturating_sub(header_h),
    );
    render_moa_roster_graph(
        frame,
        world_buttons,
        graph_area,
        formation,
        roster,
        selected_slot,
        focus,
    );
    if focus == crate::formations::MoaDeckFocus::Models {
        render_moa_model_picker(
            frame,
            world_buttons,
            graph_area,
            formation,
            selected_slot,
            models,
            selected_model,
        );
    } else if focus == crate::formations::MoaDeckFocus::Efforts {
        render_moa_effort_picker(
            frame,
            world_buttons,
            graph_area,
            formation,
            selected_slot,
            efforts,
            selected_effort,
        );
    }
}

fn render_moa_roster_graph(
    frame: &mut Frame,
    world_buttons: &mut Vec<(Rect, WorldButton)>,
    area: Rect,
    formation: crate::formations::Formation,
    roster: &crate::formations::FormationRoster,
    selected_slot: usize,
    focus: crate::formations::MoaDeckFocus,
) {
    use crate::formations::FormationRole;

    if area.width == 0 || area.height == 0 {
        return;
    }
    let slots = formation.slots();
    if slots.is_empty() {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Coordinator route only — no MoA seats consume project budget.",
                    Style::new().fg(HUD_DIM),
                )),
                Line::from(Span::styled(
                    "Choose Recon, Duel, Council, All-In, GPU Night Shift, Grok War, Tag Team, or Math God to draft a roster.",
                    Style::new().fg(HUD_BLUE),
                )),
            ]),
            area,
        );
        return;
    }

    let roles = [
        FormationRole::Scout,
        FormationRole::Propose,
        FormationRole::Judge,
        FormationRole::Verify,
        FormationRole::Aggregate,
    ];
    let node_width = if area.width >= 82 {
        20usize
    } else if area.width >= 56 {
        16
    } else {
        12
    };
    let prefix_width = 9usize.min(area.width as usize);
    let usable = (area.width as usize).saturating_sub(prefix_width);
    let per_row = (usable / (node_width + 1)).max(1);
    let active_roles = roles
        .into_iter()
        .filter(|role| slots.iter().any(|slot| slot.role == *role))
        .collect::<Vec<_>>();
    let mut y = area.y;
    for (role_index, role) in active_roles.iter().enumerate() {
        let role_slots = slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.role == *role)
            .collect::<Vec<_>>();
        for (chunk_index, chunk) in role_slots.chunks(per_row).enumerate() {
            if y >= area.y + area.height {
                return;
            }
            let prefix = if chunk_index == 0 {
                format!("{:<8} ", role.label())
            } else {
                " ".repeat(prefix_width)
            };
            let mut spans = vec![Span::styled(prefix, Style::new().fg(HUD_DIM))];
            let mut x = area.x.saturating_add(prefix_width as u16);
            for (position, (slot_index, slot)) in chunk.iter().enumerate() {
                if position > 0 {
                    spans.push(Span::raw(" "));
                    x = x.saturating_add(1);
                }
                let model = roster.assignment(*slot_index);
                let model_label = model
                    .map(|model| model.display_label())
                    .unwrap_or_else(|| "UNASSIGNED".to_string());
                // Staged THINK effort is role-wide (one SeatEfforts seat per
                // role), so every seat of the role wears it.
                let think = roster
                    .role_effort(slot.role)
                    .map(|effort| format!(" ~{effort}"))
                    .unwrap_or_default();
                let label_room = node_width
                    .saturating_sub(slot.graph_label().chars().count() + 4 + think.chars().count());
                let node = truncate_control_value(
                    &format!(
                        "[{} {}{think}]",
                        slot.graph_label(),
                        truncate_control_value(&model_label, label_room.max(1))
                    ),
                    node_width,
                );
                let selected = *slot_index == selected_slot;
                let style = if selected && focus == crate::formations::MoaDeckFocus::Slots {
                    Style::new()
                        .fg(ratatui::style::Color::Black)
                        .bg(HUD_PHOSPHOR)
                        .add_modifier(Modifier::BOLD)
                } else if model.is_none() || model.is_some_and(|model| model.metered) {
                    Style::new().fg(crate::hud::HUD_GOLD)
                } else {
                    Style::new().fg(HUD_BLUE)
                };
                spans.push(Span::styled(format!("{node:<node_width$}"), style));
                if focus != crate::formations::MoaDeckFocus::Models {
                    world_buttons.push((
                        Rect::new(x, y, node_width.min(u16::MAX as usize) as u16, 1),
                        WorldButton::SelectFormationSlot(*slot_index),
                    ));
                }
                x = x.saturating_add(node_width as u16);
            }
            // Clickable THINK chip per role row: opens the picker on the
            // chunk's first seat (staged effort is role-wide).
            if focus != crate::formations::MoaDeckFocus::Models
                && let Some((first_index, _)) = chunk.first()
            {
                let chip = match roster.role_effort(*role) {
                    Some(effort) => format!(" [~{effort}]"),
                    None => " [~env]".to_string(),
                };
                let chip_width = chip.chars().count() as u16;
                if x.saturating_add(chip_width) <= area.x.saturating_add(area.width) {
                    spans.push(Span::styled(chip, Style::new().fg(crate::hud::HUD_GOLD)));
                    world_buttons.push((
                        Rect::new(x, y, chip_width, 1),
                        WorldButton::StageFormationSeatThink(*first_index),
                    ));
                    x = x.saturating_add(chip_width);
                }
            }
            frame.render_widget(
                Paragraph::new(Line::from(spans)),
                Rect::new(area.x, y, area.width, 1),
            );
            y = y.saturating_add(1);
        }
        if role_index + 1 < active_roles.len() && y < area.y + area.height {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!("{:>8} ", "│"),
                    Style::new().fg(HUD_DIM),
                ))),
                Rect::new(area.x, y, area.width, 1),
            );
            y = y.saturating_add(1);
        }
    }
    if y < area.y + area.height {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "Tab/←→ focus · ↑↓ seat · Enter/M choose model · T think · N next · S session",
                Style::new().fg(HUD_DIM),
            ))),
            Rect::new(area.x, y, area.width, 1),
        );
    }
}

fn render_moa_model_picker(
    frame: &mut Frame,
    world_buttons: &mut Vec<(Rect, WorldButton)>,
    area: Rect,
    formation: crate::formations::Formation,
    selected_slot: usize,
    models: &[crate::formations::MoaModelChoice],
    selected_model: usize,
) {
    if area.width < 12 || area.height < 5 {
        return;
    }
    let slot_label = formation
        .slots()
        .get(selected_slot)
        .map(|slot| slot.graph_label())
        .unwrap_or_else(|| "seat".to_string());
    let width = area.width.min(72);
    let height = area
        .height
        .min((models.len() as u16).saturating_add(4).max(7));
    let popup = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Assign model · {slot_label} "))
        .border_style(HUD_BLUE_BORDER_STYLE)
        .style(panel_style());
    let inner = block.inner(popup);
    frame.render_widget(Clear, popup);
    frame.render_widget(block, popup);
    if inner.height == 0 {
        return;
    }
    if models.is_empty() {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    "No concrete model routes are available.",
                    Style::new().fg(crate::hud::HUD_GOLD),
                )),
                Line::from(Span::styled(
                    "Bring a model online, then reopen Formation.",
                    Style::new().fg(HUD_DIM),
                )),
            ]),
            inner,
        );
        return;
    }
    let visible = inner.height.saturating_sub(1) as usize;
    let selected_model = selected_model.min(models.len().saturating_sub(1));
    let start = selected_model
        .saturating_sub(visible / 2)
        .min(models.len().saturating_sub(visible));
    for (row, model_index) in (start..models.len().min(start + visible)).enumerate() {
        let choice = &models[model_index];
        let selected = model_index == selected_model;
        let cost = if choice.route.metered {
            "REMOTE"
        } else {
            "LOCAL"
        };
        let status = if choice.available { cost } else { "OFFLINE" };
        let suffix_width = status.chars().count() + 3;
        let label = truncate_control_value(
            &choice.route.display_label(),
            inner.width.saturating_sub(suffix_width as u16) as usize,
        );
        let text = format!(
            "{:<width$} {status}",
            label,
            width = inner.width.saturating_sub(suffix_width as u16) as usize
        );
        let style = if selected {
            Style::new()
                .fg(ratatui::style::Color::Black)
                .bg(HUD_PHOSPHOR)
                .add_modifier(Modifier::BOLD)
        } else if !choice.available {
            Style::new().fg(HUD_DIM)
        } else if choice.route.metered {
            Style::new().fg(crate::hud::HUD_GOLD)
        } else {
            Style::new().fg(HUD_BLUE)
        };
        let row_area = Rect::new(inner.x, inner.y + row as u16, inner.width, 1);
        frame.render_widget(Paragraph::new(text).style(style), row_area);
        if choice.available {
            world_buttons.push((row_area, WorldButton::AssignFormationModel(model_index)));
        }
    }
    let footer = Rect::new(
        inner.x,
        inner.y + inner.height.saturating_sub(1),
        inner.width,
        1,
    );
    frame.render_widget(
        Paragraph::new("↑↓ route · Enter assign · Esc graph").style(Style::new().fg(HUD_DIM)),
        footer,
    );
}

/// Rows for the THINK picker. Capability truth: a route that declares no
/// reasoning ladder gets the explicit n/a line — never an invented ladder —
/// leaving the env-default clear as the only actionable row.
fn moa_effort_picker_lines(efforts: &[String], selected_effort: usize) -> Vec<Line<'static>> {
    let selected_style = Style::new()
        .fg(ratatui::style::Color::Black)
        .bg(HUD_PHOSPHOR)
        .add_modifier(Modifier::BOLD);
    let mut lines = Vec::new();
    if efforts.is_empty() {
        lines.push(Line::from(Span::styled(
            "effort: n/a (route declares none)".to_string(),
            Style::new().fg(crate::hud::HUD_GOLD),
        )));
    }
    for (index, level) in efforts.iter().enumerate() {
        let style = if index == selected_effort {
            selected_style
        } else {
            Style::new().fg(HUD_BLUE)
        };
        lines.push(Line::from(Span::styled(format!("  {level}"), style)));
    }
    let clear_style = if selected_effort >= efforts.len() {
        selected_style
    } else {
        Style::new().fg(HUD_DIM)
    };
    lines.push(Line::from(Span::styled(
        "  env default (clear)".to_string(),
        clear_style,
    )));
    lines
}

/// The THINK picker: the focused seat's route-declared reasoning ladder plus
/// the env-default (clear) row. Keys (T · ↑↓ · Enter) drive it; the picker's
/// mouse rows land with the Formation WorldButton wave.
fn render_moa_effort_picker(
    frame: &mut Frame,
    world_buttons: &mut Vec<(Rect, WorldButton)>,
    area: Rect,
    formation: crate::formations::Formation,
    selected_slot: usize,
    efforts: &[String],
    selected_effort: usize,
) {
    if area.width < 12 || area.height < 5 {
        return;
    }
    let slot_label = formation
        .slots()
        .get(selected_slot)
        .map(|slot| slot.graph_label())
        .unwrap_or_else(|| "seat".to_string());
    let lines = moa_effort_picker_lines(efforts, selected_effort);
    let width = area.width.min(46);
    let height = area
        .height
        .min((lines.len() as u16).saturating_add(3).max(6));
    let popup = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" THINK · {slot_label} "))
        .border_style(HUD_BLUE_BORDER_STYLE)
        .style(panel_style());
    let inner = block.inner(popup);
    frame.render_widget(Clear, popup);
    frame.render_widget(block, popup);
    if inner.height == 0 {
        return;
    }
    let visible = inner.height.saturating_sub(1) as usize;
    // Rows map onto picker option indices; the n/a note line on ladder-less
    // routes offsets them by 1 and is not clickable.
    let note_rows = usize::from(efforts.is_empty());
    for (row, line) in lines.into_iter().take(visible).enumerate() {
        let row_area = Rect::new(inner.x, inner.y + row as u16, inner.width, 1);
        frame.render_widget(Paragraph::new(line), row_area);
        if row >= note_rows {
            world_buttons.push((row_area, WorldButton::StageFormationEffort(row - note_rows)));
        }
    }
    let footer = Rect::new(
        inner.x,
        inner.y + inner.height.saturating_sub(1),
        inner.width,
        1,
    );
    frame.render_widget(
        Paragraph::new("↑↓ level · Enter stage · Esc graph").style(Style::new().fg(HUD_DIM)),
        footer,
    );
}

#[cfg(test)]
mod moa_think_tests {
    use super::*;

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    #[test]
    fn comp_mode_skips_moa_deck_body_without_slowing_default() {
        use crate::tests::TestEnvGuard;
        let _lock = crate::tests::env_lock();
        let _off = TestEnvGuard::unset("ANGEL_COMP_MODE");
        crate::comp_mode::invalidate_cache();
        assert!(moa_deck_body_allowed());

        let mut app = crate::seed_preview_app();
        app.open_moa_deck(None);
        let backend = TestBackend::new(144, 48);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| ui(frame, &mut app)).unwrap();
        let standard = test_backend_text(terminal.backend());
        assert!(
            standard.contains("Agent Formation"),
            "default Formation still paints chrome\n{standard}"
        );
        assert!(
            standard.contains("Outriders") || standard.contains("Errant"),
            "default Formation still paints the card list\n{standard}"
        );

        let _on = TestEnvGuard::set("ANGEL_COMP_MODE", "1");
        crate::comp_mode::invalidate_cache();
        assert!(!moa_deck_body_allowed());

        let mut armed = crate::seed_preview_app();
        armed.open_moa_deck(None);
        let mut lean = Terminal::new(TestBackend::new(144, 48)).unwrap();
        lean.draw(|frame| ui(frame, &mut armed)).unwrap();
        let lean_text = test_backend_text(lean.backend());
        assert!(
            lean_text.contains("Agent Formation"),
            "comp-mode keeps Formation chrome\n{lean_text}"
        );
        assert!(
            !lean_text.contains("Outriders") && !lean_text.contains("Tab roster"),
            "comp-mode must not paint the formation list/roster\n{lean_text}"
        );
    }

    #[test]
    fn think_picker_lines_show_ladder_and_env_default_row() {
        let lines = moa_effort_picker_lines(&["low".to_string(), "high".to_string()], 1);
        let text: Vec<String> = lines.iter().map(line_text).collect();
        assert_eq!(text, vec!["  low", "  high", "  env default (clear)"]);
    }

    #[test]
    fn think_picker_on_a_ladder_less_route_shows_the_na_line() {
        let lines = moa_effort_picker_lines(&[], 0);
        let text: Vec<String> = lines.iter().map(line_text).collect();
        assert_eq!(
            text,
            vec!["effort: n/a (route declares none)", "  env default (clear)"]
        );
    }

    /// Full-board render proof: a ladder-less route's picker shows the n/a
    /// line, and a staged effort is worn by its seat node in the graph.
    #[test]
    fn formation_board_renders_na_picker_and_staged_seat_effort() {
        let _lock = crate::tests::env_lock();
        let mut app = crate::seed_preview_app();
        app.bag = crate::club::Bag::for_reasoning_render_test();
        app.open_moa_deck(None);
        app.select_moa_card(crate::formations::FormationId::Duel);
        // Ladder-less seat: the practice route declares no reasoning levels.
        {
            let deck = app.moa_deck.as_mut().unwrap();
            assert!(deck.select_slot(2)); // J1
            let practice = deck
                .models()
                .iter()
                .position(|choice| choice.route.model != "gpt-5.6-sol")
                .expect("ladder-less route");
            assert!(deck.assign_model(practice));
        }
        app.open_moa_effort_picker();
        let backend = TestBackend::new(144, 48);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| ui(frame, &mut app)).unwrap();
        let screen = test_backend_text(terminal.backend());
        assert!(
            screen.contains("effort: n/a (route declares none)"),
            "{screen}"
        );

        // Stage a level on the ladder route; the J1 seat node wears it.
        {
            let deck = app.moa_deck.as_mut().unwrap();
            let sol = deck
                .models()
                .iter()
                .position(|choice| choice.route.model == "gpt-5.6-sol")
                .expect("ladder route");
            assert!(deck.assign_model(sol));
            assert!(deck.select_slot(2));
            assert!(deck.open_effort_picker(vec![
                "low".to_string(),
                "medium".to_string(),
                "high".to_string()
            ]));
            deck.move_effort(-1);
            assert!(deck.stage_selected_effort().is_some());
        }
        terminal.draw(|frame| ui(frame, &mut app)).unwrap();
        let screen = test_backend_text(terminal.backend());
        assert!(screen.contains("~high"), "{screen}");
    }
}

#[cfg(test)]
mod caption_tests {
    use super::*;

    #[test]
    fn styled_caption_clips_to_terminal_cells_for_wide_unicode() {
        let clipped = clip_caption(
            Line::from(vec![
                Span::styled("資料/設計🧪", Style::new().fg(crate::hud::HUD_TEXT)),
                Span::styled(" → Scriptorium", Style::new().fg(crate::hud::HUD_BLUE)),
            ]),
            12,
        );
        let text = clipped
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert!(
            unicode_width::UnicodeWidthStr::width(text.as_str()) <= 12,
            "clipped caption overflowed 12 cells: {text:?}"
        );
        assert!(text.ends_with('…'), "clipped caption needs an ellipsis");
    }
}

#[cfg(test)]
mod merge_frame_tests {
    #[test]
    fn merge_frame_borders_reuses_scratch_rects() {
        let src = include_str!("draw.rs");
        let start = src
            .find("fn merge_frame_borders(")
            .expect("merge_frame_borders");
        let body = src[start..]
            .split("pub(crate) fn render_message_composer(")
            .next()
            .expect("body");
        assert!(
            src.contains("static MERGE_RECTS"),
            "junction repair must keep a rect scratch"
        );
        assert!(
            body.contains("rects.clear()") && body.contains("rects.extend("),
            "scratch must be reused, not rebuilt: {body}"
        );
        assert!(
            !body.contains(".collect()"),
            "merge_frame_borders must not collect a fresh Vec: {body}"
        );
    }
}
