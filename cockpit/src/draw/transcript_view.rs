//! Transcript work-surface renderer.
//!
//! This view owns viewport windowing, per-block roll-in, the live Tool Strip,
//! and incremental wrapped-height caching. Root layout remains in the parent
//! draw compositor.

use super::*;

/// Use the full transcript body, leaving only its final column for the scroll
/// rail. The rail stays at the panel edge even in fullscreen/ultrawide layouts.
pub(crate) fn transcript_text_and_rail(body: Rect) -> (Rect, Option<Rect>) {
    if body.width < 2 || body.height == 0 {
        return (body, None);
    }
    let text_width = body.width.saturating_sub(1);
    let text = Rect {
        width: text_width,
        ..body
    };
    let rail = Rect {
        x: text.x.saturating_add(text.width),
        width: 1,
        ..body
    };
    (text, Some(rail))
}

/// Rows claimed by the live tool strip at the bottom of the transcript inner
/// rect, including one ambient row while idle. Shared with pane registration so clipboard
/// selection never covers strip chrome.
pub(crate) fn tool_strip_height(app: &App, inner_height: u16) -> u16 {
    let strip_on = app.thinking.is_some()
        && app.transcript_mode == crate::app::TranscriptMode::Conversation
        && !app.tool_strip.is_empty();
    if inner_height == 0 {
        return 0;
    }
    if strip_on {
        (2 + u16::from(app.tool_strip.note().is_some())).min(inner_height)
    } else {
        u16::from(
            app.transcript_mode == crate::app::TranscriptMode::Conversation
                && crate::comp_mode::ambient_stage_sim_allowed(),
        )
    }
}

fn transcript_block(app: &App) -> Block<'static> {
    let block = hud_block(status_view::agent_shell_title());
    if app
        .module_host
        .focused()
        .is_some_and(|module| module.as_str() == "core")
    {
        block.border_style(HUD_BLUE_BORDER_STYLE)
    } else {
        block
    }
}

pub(crate) fn render_transcript(frame: &mut Frame, app: &mut App, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    if app.messages.is_empty() && app.partial.is_empty() {
        app.scroll = 0;
        app.transcript_last_bottom = 0;
        app.transcript_anchor_w = 0;
        app.transcript_rolling = false;
        let block = transcript_block(app);
        let inner = block.inner(area);
        frame.render_widget(Paragraph::new("").style(panel_style()).block(block), area);
        let strip_h = tool_strip_height(app, inner.height);
        let (body, _) = transcript_text_and_rail(Rect {
            height: inner.height.saturating_sub(strip_h),
            ..inner
        });
        let intro_area = body.inner(ratatui::layout::Margin::new(1, 1));
        let geometry = app.viewer.dot_geometry(intro_area);
        app.startup_intro
            .render(frame, intro_area, geometry, app.visual_motion);
        if strip_h > 0 {
            render_tool_strip(
                frame,
                app,
                Rect {
                    y: inner.bottom() - strip_h,
                    height: strip_h,
                    ..inner
                },
            );
        }
        return;
    }
    let block = transcript_block(app);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let thinking = app.thinking.is_some();
    // While tools run, the live activity strip (status row + lateral loading
    // bar) claims the bottom rows of the pane; the scrollback keeps only the
    // agent's directed output. `/trace` restores the old full trace instead.
    // Status row + lateral bar, plus one in-place note row when the harness
    // has murmured (guards, compaction, recall) — the play-by-play lives here
    // instead of stacking in the scrollback.
    let strip_h = tool_strip_height(app, inner.height);
    let full_body = Rect {
        height: inner.height - strip_h,
        ..inner
    };
    let (body, scroll_rail) = transcript_text_and_rail(full_body);
    if body.width == 0 || body.height == 0 {
        app.transcript_rolling = false;
        if strip_h > 0 {
            let strip_area = Rect {
                y: inner.y + inner.height - strip_h,
                height: strip_h,
                ..inner
            };
            render_tool_strip(frame, app, strip_area);
        }
        return;
    }
    let inner_w = body.width as usize;
    let inner_h = body.height;

    // Total wrapped height = cached committed-message rows + the live partial.
    // The cumulative cache makes the committed total O(1) instead of summing or
    // re-wrapping the whole history every frame.
    let old_width = app.transcript_heights_w;
    let reader_anchor = (app.scroll > 0).then(|| {
        let top = app.transcript_last_bottom.saturating_sub(app.scroll);
        let (message, intra, _) = transcript_window(
            &app.transcript_height_prefix,
            u32::from(top),
            1,
            app.messages.len(),
        );
        (message, intra as u32)
    });
    let hist_total = sync_transcript_heights(app, inner_w);
    let selected = app
        .selection
        .as_ref()
        .is_some_and(|selection| selection.pane == mouse::PaneId::Transcript);
    if selected && let Some(painted) = &app.transcript_layouts.painted {
        transcript::oversized::paint(painted, frame.buffer_mut(), body);
        app.transcript_rolling = false;
        if strip_h > 0 {
            render_tool_strip(
                frame,
                app,
                Rect {
                    y: inner.y + inner.height - strip_h,
                    height: strip_h,
                    ..inner
                },
            );
        }
        return;
    }
    let oversized_partial =
        thinking && transcript::oversized_partial_window(&app.partial).is_some();
    let partial_h = if oversized_partial {
        app.transcript_layouts
            .partial_height(&app.partial, inner_w, app.scroll == 0)
    } else {
        app.transcript_layouts.clear_partial();
        transcript::partial_height_cached(
            &mut app.partial_height_cache,
            &app.partial,
            thinking,
            inner_w,
        )
    } as u32;
    let total = (hist_total + partial_h).min(u16::MAX as u32) as u16;

    let bottom = total.saturating_sub(inner_h);
    // `scroll` is measured from the bottom. When content grows beneath a reader
    // who has scrolled back, preserve the absolute top row instead of letting
    // each token drag the viewport toward the live tail. Publishing a rebuilt
    // index remaps the current message (or partial) and its clamped intra-row.
    if old_width != 0
        && old_width != app.transcript_heights_w
        && let Some((message, intra)) = reader_anchor
    {
        let height = app
            .transcript_heights
            .get(message)
            .copied()
            .map(u32::from)
            .unwrap_or(partial_h);
        let top = app
            .transcript_height_prefix
            .get(message)
            .copied()
            .unwrap_or(hist_total)
            .saturating_add(intra.min(height.saturating_sub(1)));
        app.scroll = bottom.saturating_sub(top.min(u32::from(u16::MAX)) as u16);
    } else if app.scroll > 0 && app.transcript_anchor_w == body.width {
        let previous_top = app
            .transcript_last_bottom
            .saturating_sub(app.scroll.min(app.transcript_last_bottom));
        app.scroll = bottom.saturating_sub(previous_top);
    }
    app.scroll = app.scroll.min(bottom);
    app.transcript_last_bottom = bottom;
    app.transcript_anchor_w = body.width;
    let y = bottom - app.scroll; // first visible row from the top of the transcript

    // Window to messages intersecting [y, y + inner_h). The cumulative height
    // index finds both boundaries with binary search, so an old scrolled-back
    // viewport does not walk the entire conversation on every frame.
    let (first, intra, end) = transcript_window(
        &app.transcript_height_prefix,
        y as u32,
        inner_h as u32,
        app.transcript_heights.len().min(app.messages.len()),
    );
    // The partial renders at the very bottom; include it only when the window
    // reaches the end of committed messages (so its rows can fall in view).
    let at_bottom = end == app.messages.len();

    // Brief arrival emphasis keeps all text visible on the first frame.
    // Only the pinned-to-newest view animates; scrollback and reduced motion
    // settle immediately. Output must never wait for an entrance animation.
    let now = app.started.elapsed().as_secs_f32();
    sync_transcript_spawns(app, now);
    let rolling = at_bottom
        && app.scroll == 0
        // Streaming content wins over decorative entry motion. Otherwise a
        // fresh user block can clip a fast-growing partial until its roll-in
        // finishes, which makes the reply look frozen.
        && app.partial.is_empty()
        && matches!(app.visual_motion, MotionMode::Full)
        && transcript_roll_in_allowed()
        && app.transcript_spawns[first..end]
            .iter()
            .any(|&spawn| now - spawn < ROLL_IN_SECS);
    app.transcript_rolling = rolling;

    if rolling
        || (oversized_partial && app.scroll > 0 && at_bottom)
        || (!(oversized_partial && app.scroll == 0 && at_bottom)
            && app.messages[first..end]
                .iter()
                .any(transcript::oversized::needs_worker))
    {
        render_rolling_blocks(frame, app, body, first, end, intra, now, thinking);
    } else {
        // Holds the window's markdown renders for the frame so `lines` can borrow
        // span text out of the cache instead of deep-cloning per visible message.
        let mut md_renders: Vec<transcript::MdRender> = Vec::new();
        let pinned_partial_window = (at_bottom && app.scroll == 0)
            .then(|| transcript::oversized_partial_window(&app.partial))
            .flatten();
        let lines = if let Some(partial) = pinned_partial_window {
            transcript::plain_partial_lines(partial)
        } else {
            transcript::lines(
                &app.messages[first..end],
                if at_bottom { &app.partial } else { "" },
                thinking && at_bottom,
                inner_w,
                &mut md_renders,
            )
        };
        let visible_intra = pinned_partial_window.map_or(intra as u16, |partial| {
            transcript::plain_partial_height(partial, inner_w).saturating_sub(inner_h)
        });

        let para = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .style(panel_style());
        frame.render_widget(para.scroll((visible_intra, 0)), body);
    }
    if app.transcript_heights.len() < app.messages.len() {
        let status = app
            .transcript_layouts
            .failure()
            .unwrap_or("Preparing transcript layout…");
        frame.render_widget(
            Paragraph::new(status).style(Style::new().fg(HUD_DIM)),
            Rect {
                y: body.y + body.height - 1,
                height: 1,
                ..body
            },
        );
    }
    if !selected {
        app.transcript_layouts.painted =
            Some(transcript::oversized::snapshot(frame.buffer_mut(), body));
    }
    if let Some(scroll_rail) = scroll_rail.filter(|_| total > inner_h) {
        let mut sb = ScrollbarState::new(bottom as usize + 1)
            .viewport_content_length(inner_h as usize)
            .position(y as usize);
        let thumb_style = if app.scroll > 0 {
            Style::new().fg(hud::HUD_GOLD).add_modifier(Modifier::BOLD)
        } else if thinking {
            PHOSPHOR_STYLE.add_modifier(Modifier::BOLD)
        } else {
            HUD_BLUE_BORDER_STYLE.add_modifier(Modifier::BOLD)
        };
        // Dotted track + soft disc thumb: a delicate progress chrome that stays
        // inside the 1-cell rail without the heavy continuous box-drawing bar.
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(Some("·"))
                .track_style(Style::new().fg(HUD_DIM))
                .thumb_symbol("●")
                .thumb_style(thumb_style),
            scroll_rail,
            &mut sb,
        );
    }
    if strip_h > 0 {
        let strip_area = Rect {
            y: inner.y + inner.height - strip_h,
            height: strip_h,
            ..inner
        };
        render_tool_strip(frame, app, strip_area);
    }
}

/// Brief arrival emphasis. Comp / lean keeps settled text. Hidden Stage is
/// unrelated — the transcript is the work surface.
pub(crate) fn transcript_roll_in_allowed() -> bool {
    crate::comp_mode::ambient_stage_sim_allowed()
}

/// Brief arrival emphasis. All rows are visible even at age zero.
const ROLL_IN_SECS: f32 = 0.16;
/// Cap burst feedback rather than delaying later blocks behind a cascade.
const ROLL_STAGGER_SECS: f32 = 0.0;
const ROLL_DIM_UNTIL: f32 = 0.5;

/// Sync arrival times; user echo and restored history settle immediately.
fn sync_transcript_spawns(app: &mut App, now: f32) {
    if app.transcript_spawns.len() > app.messages.len() {
        app.transcript_spawns.clear();
    }
    let missing = app.messages.len() - app.transcript_spawns.len();
    if missing == 0 {
        return;
    }
    if app.transcript_spawns.is_empty() && missing > 3 {
        app.settle_transcript_spawns();
        return;
    }
    for burst_index in 0..missing {
        let index = app.transcript_spawns.len();
        let spawn = if matches!(app.messages[index].role, Role::User) {
            // Subtracting the duration can round back inside the animation
            // window as the f32 clock grows during a long-running session.
            f32::NEG_INFINITY
        } else {
            now + burst_index as f32 * ROLL_STAGGER_SECS
        };
        app.transcript_spawns.push(spawn);
    }
}

/// Render arrival emphasis in the same fixed rectangles as settled text.
/// No translation or timed clipping may delay operator-visible output.
#[allow(clippy::too_many_arguments)]
fn render_rolling_blocks(
    frame: &mut Frame,
    app: &mut App,
    body: Rect,
    first: usize,
    end: usize,
    intra: usize,
    now: f32,
    thinking: bool,
) {
    let inner_w = body.width as usize;
    let body_top = i32::from(body.y);
    let body_bottom = body_top + i32::from(body.height);
    let mut cursor = body_top - intra as i32;
    for index in first..end {
        let height = i32::from(app.transcript_heights[index]);
        let final_top = cursor;
        cursor += height;
        if height == 0 {
            continue;
        }
        let age = now - app.transcript_spawns[index];
        let progress = if app.transcript_rolling {
            (age / ROLL_IN_SECS).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let draw_top = final_top.max(body_top);
        let draw_bottom = (final_top + height).min(body_bottom);
        if draw_bottom <= draw_top {
            continue;
        }
        // Only genuine viewport clipping can hide rows above this rectangle.
        let skip = (draw_top - final_top).max(0) as u16;
        let rect = Rect {
            x: body.x,
            y: draw_top as u16,
            width: body.width,
            height: (draw_bottom - draw_top) as u16,
        };
        if transcript::oversized::needs_worker(&app.messages[index]) {
            if let Some(buffer) = app.transcript_layouts.view(
                &app.messages[index],
                body.width,
                skip,
                rect.height,
                panel_style(),
            ) {
                transcript::oversized::paint(&buffer, frame.buffer_mut(), rect);
            } else {
                frame.render_widget(
                    Paragraph::new(
                        app.transcript_layouts
                            .failure()
                            .unwrap_or("Preparing message viewport…"),
                    )
                    .style(Style::new().fg(HUD_DIM)),
                    rect,
                );
            }
        } else {
            let mut md_render: Vec<transcript::MdRender> = Vec::new();
            let block_lines = transcript::lines(
                &app.messages[index..index + 1],
                "",
                false,
                inner_w,
                &mut md_render,
            );
            frame.render_widget(
                Paragraph::new(block_lines)
                    .wrap(Wrap { trim: false })
                    .style(panel_style())
                    .scroll((skip, 0)),
                rect,
            );
        }
        if progress < ROLL_DIM_UNTIL {
            frame
                .buffer_mut()
                .set_style(rect, Style::new().add_modifier(Modifier::DIM));
        }
    }
    // The live partial streams below the committed blocks as a settled
    // pseudo-block; its reveal is the stream itself, never a slide. With
    // nothing streamed yet, `transcript::lines` renders the pre-stream reply
    // cursor row instead.
    if thinking && cursor < body_bottom {
        let draw_top = cursor.max(body_top);
        let rect = Rect {
            x: body.x,
            y: draw_top as u16,
            width: body.width,
            height: (body_bottom - draw_top) as u16,
        };
        if transcript::oversized_partial_window(&app.partial).is_some() {
            if app.scroll == 0 {
                let tail = transcript::oversized_partial_window(&app.partial).unwrap();
                let top =
                    transcript::plain_partial_height(tail, inner_w).saturating_sub(rect.height);
                frame.render_widget(
                    Paragraph::new(transcript::plain_partial_lines(tail))
                        .wrap(Wrap { trim: false })
                        .style(panel_style())
                        .scroll((top, 0)),
                    rect,
                );
            } else if let Some(buffer) = app.transcript_layouts.partial_view(
                body.width,
                (draw_top - cursor).max(0) as u16,
                rect.height,
                panel_style(),
            ) {
                transcript::oversized::paint(&buffer, frame.buffer_mut(), rect);
            } else {
                frame.render_widget(
                    Paragraph::new("Preparing streamed viewport…").style(Style::new().fg(HUD_DIM)),
                    rect,
                );
            }
        } else {
            let mut md_render: Vec<transcript::MdRender> = Vec::new();
            let partial_lines = transcript::lines(&[], &app.partial, true, inner_w, &mut md_render);
            frame.render_widget(
                Paragraph::new(partial_lines)
                    .wrap(Wrap { trim: false })
                    .style(panel_style()),
                rect,
            );
        }
    }
}

const STATUS_ROW_SPACES: &str = concat!(
    "                                ",
    "                                ",
    "                                ",
    "                                ",
    "                                ",
    "                                ",
    "                                ",
    "                                ",
);

fn push_space_pad(spans: &mut Vec<Span<'_>>, n: usize, style: Style) {
    let mut remaining = n;
    while remaining > 0 {
        let take = remaining.min(STATUS_ROW_SPACES.len());
        spans.push(Span::styled(&STATUS_ROW_SPACES[..take], style));
        remaining -= take;
    }
}

/// The live tool-activity strip: a compact status row (current tool, call
/// count, elapsed) over a lateral dotmax loading bar streaming while the agent
/// works. Pinned under the transcript body; idle retains a quiet catalog loop.
fn render_tool_strip(frame: &mut Frame, app: &App, area: Rect) {
    render_tool_strip_at(frame, app, area, std::time::Instant::now());
}

fn render_tool_strip_at(frame: &mut Frame, app: &App, area: Rect, now: std::time::Instant) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let w = area.width as usize;
    // Operator request: the dotmax lanes are ACTIVITY indicators — they must
    // keep animating even when the terminal window is not focused. Focus loss
    // no longer pauses the strip; only deliberate reading interactions
    // (scroll back, text selection, explicit pause) freeze it.
    let paused = app.scroll > 0
        || app.selection.is_some()
        || app.composer_selection_range().is_some()
        || app.tool_strip.reading_paused();
    // Stall pulse: how long since the worker streamed ANY event, read from
    // the same clock the turn watchdog uses so the strip and the abandonment
    // logic can never disagree. Past the threshold the row gains a silence
    // readout; past half the idle deadline the
    // readout escalates to the watchdog countdown.
    let child = app
        .thinking
        .as_ref()
        .filter(|_| app.tool_strip.has_running_calls())
        .and_then(|thinking| {
            crate::harness::owned_child_snapshot(std::sync::Arc::as_ptr(&thinking.cancel) as usize)
        });
    let delegate = app
        .thinking
        .as_ref()
        .filter(|_| app.tool_strip.has_running_calls())
        .and_then(|thinking| {
            crate::harness::owned_delegate_snapshot(
                std::sync::Arc::as_ptr(&thinking.cancel) as usize
            )
        });
    let stall = app.thinking.as_ref().and_then(|thinking| {
        if let Some(child) = &child {
            let quiet = toolstrip::child_quiet_secs(child);
            return (!child.setting_up && quiet >= 30).then(|| toolstrip::StallReadout {
                text: format!("· no CPU/output {quiet}s"),
                severity: toolstrip::StallSeverity::Warn,
            });
        }
        if let Some(delegate) = &delegate {
            let quiet = toolstrip::delegate_quiet_secs(delegate);
            return (quiet >= 30).then(|| toolstrip::StallReadout {
                text: format!("· no delegate progress {quiet}s"),
                severity: toolstrip::StallSeverity::Warn,
            });
        }
        toolstrip::effective_stall_readout(
            &app.tool_strip,
            thinking.last_stream_at.elapsed().as_secs(),
            toolstrip::configured_stall_pulse_secs(),
            thinking.idle_timeout_secs,
        )
    });
    let stall_severity = stall.as_ref().map(|readout| readout.severity);
    let status = child
        .as_ref()
        .map(|child| toolstrip::worker_status_row(child, w))
        .or_else(|| {
            delegate
                .as_ref()
                .map(|delegate| toolstrip::delegate_status_row(delegate, w))
        })
        .or_else(|| toolstrip::status_row_parts_with_silence(&app.tool_strip, w, stall));
    let waiting_on_agents =
        child.is_none() && delegate.is_none() && app.tool_strip.is_waiting_on_agents();
    let state_style = match status.as_ref().map(|row| row.state) {
        Some(toolstrip::ToolState::Running) if waiting_on_agents => {
            Style::new().fg(hud::HUD_GOLD).add_modifier(Modifier::BOLD)
        }
        Some(toolstrip::ToolState::Running) => PHOSPHOR_STYLE.add_modifier(Modifier::BOLD),
        Some(toolstrip::ToolState::Passed) => Style::new()
            .fg(hud::HUD_VERIFIED)
            .add_modifier(Modifier::BOLD),
        Some(toolstrip::ToolState::Failed) => Style::new()
            .fg(hud::HUD_DANGER)
            .add_modifier(Modifier::BOLD),
        Some(toolstrip::ToolState::NotStarted) => {
            Style::new().fg(hud::HUD_GOLD).add_modifier(Modifier::BOLD)
        }
        Some(toolstrip::ToolState::Inconclusive) => Style::new().fg(hud::HUD_GOLD),
        None => dim_panel_style(),
    };
    let mut lines = Vec::with_capacity(3);
    if area.height >= 2 {
        let status_line = status.as_ref().map_or_else(Line::default, |row| {
            let marker_len = row.left.chars().next().map(char::len_utf8).unwrap_or(0);
            let (marker, body) = row.left.split_at(marker_len);
            let verdict_style = if row.verifier {
                state_style
            } else {
                dim_panel_style()
            };
            let mut spans = Vec::with_capacity(6);
            spans.push(Span::styled(marker, state_style));
            spans.push(Span::styled(body, dim_panel_style()));
            push_space_pad(&mut spans, row.padding, dim_panel_style());
            if !row.right.is_empty() {
                spans.push(Span::styled(row.right.as_str(), verdict_style));
            }
            if let Some(stall) = row.stall.as_ref() {
                let style = stall_style(stall.severity);
                spans.push(Span::styled(" ", style));
                spans.push(Span::styled(stall.text.as_str(), style));
            }
            Line::from(spans)
        });
        lines.push(status_line);
    }
    if area.height >= 3 && app.tool_strip.note().is_some() {
        lines.push(Line::from(Span::styled(
            app.tool_strip.rolling_note(
                w,
                now,
                matches!(app.visual_motion, MotionMode::Full),
                paused,
            ),
            dim_panel_style(),
        )));
    }
    // Warp-drive pulse: advance a virtual clock by dt * event-rate drive so
    // parallel results streak and a silent strip glides honestly. Reduced
    // motion clamps drive to 1.0; silence pauses this work clock. The separate
    // catalog ambience clock keeps quiet states alive.
    let reduced = !matches!(app.visual_motion, MotionMode::Full);
    let time = if matches!(app.visual_motion, MotionMode::Off) {
        0.0
    } else {
        app.tool_strip
            .warp_time_at(now, reduced, paused || stall_severity.is_some())
    };
    // Preserve status colors on the stationary receipt or warning marker.
    // Quiet ambience uses its own dim neutral color.
    let bar_style = match stall_severity {
        Some(toolstrip::StallSeverity::Watchdog) => {
            Style::new().fg(hud::HUD_DANGER).add_modifier(Modifier::DIM)
        }
        Some(toolstrip::StallSeverity::Warn) => dim_panel_style(),
        None => match status.as_ref().map(|row| row.state) {
            Some(toolstrip::ToolState::Failed) => Style::new().fg(hud::HUD_DANGER),
            Some(toolstrip::ToolState::Passed) => Style::new().fg(hud::HUD_VERIFIED),
            Some(toolstrip::ToolState::NotStarted) => Style::new().fg(hud::HUD_GOLD),
            Some(toolstrip::ToolState::Inconclusive) => Style::new().fg(hud::HUD_GOLD),
            Some(toolstrip::ToolState::Running) if waiting_on_agents => {
                Style::new().fg(hud::HUD_GOLD)
            }
            Some(toolstrip::ToolState::Running) | None => PHOSPHOR_STYLE,
        },
    };
    let idle = app.thinking.is_none() || app.tool_strip.is_empty();
    let quiet = idle || app.tool_strip.bar_is_quiet(stall_severity.is_some());
    if crate::comp_mode::ambient_stage_sim_allowed() && quiet {
        // The stationary receipt remains legible; dim catalog motion next to
        // it is ambience, independent of tool progress or provider liveness.
        let marker = if idle {
            String::new()
        } else if stall_severity.is_some() {
            "! ".chars().take(w).collect()
        } else {
            let mut marker = toolstrip::bar_row_with_silence(&app.tool_strip, 1, 0.0, false);
            if w > 1 {
                marker.push(' ');
            }
            marker
        };
        let row = app.tool_strip.ambient_row(
            w.saturating_sub(marker.chars().count()),
            now,
            app.visual_motion,
            paused,
        );
        lines.push(Line::from(vec![
            Span::styled(marker, bar_style),
            Span::styled(
                row.to_string(),
                Style::new().fg(ratatui::style::Color::Rgb(58, 85, 94)),
            ),
        ]));
    } else {
        // Tick the ambience clock while work owns the lane, so returning to
        // idle doesn't reset the loop or borrow a stalled work clock.
        if crate::comp_mode::ambient_stage_sim_allowed() {
            app.tool_strip
                .ambient_row(0, now, app.visual_motion, paused);
        }
        lines.push(Line::from(Span::styled(
            toolstrip::bar_row_with_silence(&app.tool_strip, w, time, stall_severity.is_some()),
            bar_style,
        )));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

#[cfg(test)]
fn strip_note_row_text(note: &str, count: usize, prefixes: &[String]) -> String {
    toolstrip::note_row_text(note, count, prefixes)
}

/// Stall-readout palette: gold while the stream is merely silent, danger once
/// the fragment is counting down to the watchdog abandoning the turn.
fn stall_style(severity: toolstrip::StallSeverity) -> Style {
    match severity {
        toolstrip::StallSeverity::Warn => Style::new().fg(hud::HUD_GOLD),
        toolstrip::StallSeverity::Watchdog => Style::new()
            .fg(hud::HUD_DANGER)
            .add_modifier(Modifier::BOLD),
    }
}

/// Append at the published width, then advance one unpublished resize index.
/// A resize retains its complete old index until the replacement is ready.
/// Cold and appended history share the same budget; giant messages use a worker.
pub(crate) fn sync_transcript_heights(app: &mut App, inner_w: usize) -> u32 {
    let mut started = None;
    sync_transcript_heights_budgeted(app, inner_w, 64, || {
        started.get_or_insert_with(Instant::now).elapsed() >= Duration::from_millis(2)
    })
}

fn sync_transcript_heights_budgeted(
    app: &mut App,
    inner_w: usize,
    max_messages: usize,
    mut should_yield: impl FnMut() -> bool,
) -> u32 {
    let w = inner_w as u16;
    app.transcript_layouts.poll(&app.messages);
    if app.transcript_heights.len() > app.messages.len() {
        app.invalidate_transcript_layout();
    }
    repair_transcript_prefix(&app.transcript_heights, &mut app.transcript_height_prefix);
    if app.transcript_heights.is_empty() {
        app.transcript_heights_w = w;
        app.pending_transcript_reflow = None;
    }
    let published_width = usize::from(app.transcript_heights_w);
    extend_transcript_heights(
        &app.messages,
        published_width,
        &mut app.transcript_heights,
        &mut app.transcript_height_prefix,
        &mut app.transcript_layouts,
        max_messages,
        &mut should_yield,
    );
    if app.transcript_heights_w == w {
        app.pending_transcript_reflow = None;
    } else {
        if app
            .pending_transcript_reflow
            .as_ref()
            .is_some_and(|work| work.width != w)
        {
            app.pending_transcript_reflow = None;
        }
        if app.last_resize_at.elapsed() >= RESIZE_SETTLE {
            let work = app
                .pending_transcript_reflow
                .get_or_insert_with(|| transcript::reflow::PendingReflow::new(w));
            work.advance(
                &app.messages,
                max_messages,
                &mut should_yield,
                |message, width| app.transcript_layouts.height(message, width),
            );
            let selected = app
                .selection
                .as_ref()
                .is_some_and(|selection| selection.pane == mouse::PaneId::Transcript);
            if work.ready(app.messages.len()) && !selected {
                let work = app.pending_transcript_reflow.take().unwrap();
                app.transcript_heights = work.heights;
                app.transcript_height_prefix = work.prefix;
                app.transcript_heights_w = work.width;
            }
        }
    }
    app.transcript_height_prefix.last().copied().unwrap_or(0)
}

fn extend_transcript_heights(
    messages: &[Message],
    inner_w: usize,
    heights: &mut Vec<u16>,
    prefix: &mut Vec<u32>,
    layouts: &mut transcript::oversized::Layouts,
    max_messages: usize,
    mut should_yield: impl FnMut() -> bool,
) {
    for message in messages.iter().skip(heights.len()).take(max_messages) {
        if should_yield() {
            break;
        }
        let Some(height) = layouts.height(message, inner_w) else {
            break;
        };
        heights.push(height);
        let total = prefix.last().copied().unwrap_or(0);
        prefix.push(total.saturating_add(u32::from(height)));
    }
}

fn repair_transcript_prefix(heights: &[u16], prefix: &mut Vec<u32>) {
    if prefix.len() == heights.len().saturating_add(1) && prefix.first() == Some(&0) {
        return;
    }
    prefix.clear();
    prefix.push(0);
    for &height in heights {
        let total = prefix.last().copied().unwrap_or(0);
        prefix.push(total.saturating_add(u32::from(height)));
    }
}

fn transcript_window(
    prefix: &[u32],
    y: u32,
    viewport_height: u32,
    message_len: usize,
) -> (usize, usize, usize) {
    if prefix.is_empty() || message_len == 0 {
        return (0, 0, 0);
    }
    let first = prefix
        .partition_point(|&offset| offset <= y)
        .saturating_sub(1)
        .min(message_len);
    let first_offset = prefix.get(first).copied().unwrap_or(y);
    let intra = y.saturating_sub(first_offset) as usize;
    let end_offset = y.saturating_add(viewport_height);
    let end = prefix
        .partition_point(|&offset| offset < end_offset)
        .min(message_len)
        .max(first);
    (first, intra, end)
}

#[cfg(test)]
mod tests {
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
            .draw(|frame| {
                render_rolling_blocks(frame, &mut app, frame.area(), 0, 2, 0, 10.0, false)
            })
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
                transcript_text_and_rail(hud_block(status_view::agent_shell_title()).inner(area));
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
            let mut selection =
                mouse::Selection::new(mouse::PaneId::Transcript, body, user_x, user_y);
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
        let mut inner = hud_block(status_view::agent_shell_title()).inner(area);
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
}

#[cfg(test)]
mod reflow_tests;

#[cfg(test)]
mod async_tests;
