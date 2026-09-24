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
/// rect, including one ambient row while idle and the herald's open ledger.
/// Shared with pane registration so clipboard selection never covers strip
/// chrome.
pub(crate) fn tool_strip_height(app: &App, inner_height: u16) -> u16 {
    let strip_on = app.thinking.is_some()
        && app.transcript_mode == crate::app::TranscriptMode::Conversation
        && !app.tool_strip.is_empty();
    if inner_height == 0 {
        return 0;
    }
    if strip_on {
        let ledger = (app.tool_strip.ledger_height() as u16).min(inner_height / 2);
        (2 + u16::from(app.tool_strip.note().is_some()) + ledger).min(inner_height)
    } else {
        u16::from(
            app.transcript_mode == crate::app::TranscriptMode::Conversation
                && crate::drive::comp_mode::ambient_stage_sim_allowed(),
        )
    }
}

fn transcript_block(app: &App) -> Block<'static> {
    let block = hud_block(crate::ui::views::status_view::agent_shell_title());
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
        // One column of side margin only: a vertical margin put a blank text
        // row between the water and the pane's bottom border, and the fit
        // pins the frame to the floor of exactly this rect.
        let intro_area =
            crate::app::startup_intro::intro_band(body.inner(ratatui::layout::Margin::new(1, 0)));
        // Banded before the geometry is derived, so the composed canvas, the
        // fine-dot raster and the declared cell rect are all one width: a narrow
        // rect declared over a full-pane raster is what squeezed the clip.
        let geometry = app.viewer.dot_geometry(intro_area);
        app.startup_intro
            .render(frame, intro_area, geometry, app.visual_motion);
        if strip_h > 0 {
            place_tool_strip(
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
            place_tool_strip(frame, app, strip_area);
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
            place_tool_strip(
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
        place_tool_strip(frame, app, strip_area);
    }
}

/// Brief arrival emphasis. Comp / lean keeps settled text. Hidden Stage is
/// unrelated — the transcript is the work surface.
pub(crate) fn transcript_roll_in_allowed() -> bool {
    crate::drive::comp_mode::ambient_stage_sim_allowed()
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

/// Draw the strip and publish its herald row as the ledger's toggle. The
/// idle ambient row and a strip with no calls yet offer nothing to open.
fn place_tool_strip(frame: &mut Frame, app: &mut App, area: Rect) {
    render_tool_strip(frame, app, area);
    app.tool_herald_area =
        (area.height >= 2 && app.tool_strip.count() > 0).then_some(Rect { height: 1, ..area });
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
            crate::agent::harness::owned_child_snapshot(
                std::sync::Arc::as_ptr(&thinking.cancel) as usize
            )
        });
    let delegate = app
        .thinking
        .as_ref()
        .filter(|_| app.tool_strip.has_running_calls())
        .and_then(|thinking| {
            crate::agent::harness::owned_delegate_snapshot(
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
        .map(|child| toolstrip::worker_status_row(child, app.tool_strip.current_herald(), w))
        .or_else(|| {
            delegate.as_ref().map(|delegate| {
                toolstrip::delegate_status_row(delegate, app.tool_strip.current_herald(), w)
            })
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
            let mut lead = row.lead.saturating_sub(marker_len).min(body.len());
            while !body.is_char_boundary(lead) {
                lead -= 1;
            }
            let (verb, object) = body.split_at(lead);
            let verdict_style = if row.verifier {
                state_style
            } else {
                dim_panel_style()
            };
            let mut spans = Vec::with_capacity(7);
            spans.push(Span::styled(marker, state_style));
            spans.push(Span::styled(verb, panel_style()));
            spans.push(Span::styled(object, dim_panel_style()));
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
    let note_row = area.height >= 3 && app.tool_strip.note().is_some();
    let ledger_budget = (area.height as usize).saturating_sub(2 + usize::from(note_row));
    for row in app.tool_strip.ledger_rows(w, ledger_budget) {
        let mut spans = Vec::with_capacity(7);
        spans.push(Span::styled(row.branch, dim_panel_style()));
        if let Some(state) = row.state {
            spans.push(Span::styled(row.mark, ledger_mark_style(state)));
            spans.push(Span::styled(" ", dim_panel_style()));
            spans.push(Span::styled(row.call, panel_style()));
        } else {
            spans.push(Span::styled(row.call, dim_panel_style()));
        }
        spans.push(Span::styled(row.args, dim_panel_style()));
        push_space_pad(&mut spans, row.padding, dim_panel_style());
        if let Some(state) = row.state.filter(|_| !row.note.is_empty()) {
            spans.push(Span::styled(row.note, ledger_mark_style(state)));
        }
        spans.push(Span::styled(row.age, dim_panel_style()));
        lines.push(Line::from(spans));
    }
    if note_row {
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
    if crate::drive::comp_mode::ambient_stage_sim_allowed() && quiet {
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
        if crate::drive::comp_mode::ambient_stage_sim_allowed() {
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

/// A ledger mark keeps its call's semantic colour without the status row's
/// weight: the list reads as record, the herald above it as the live line.
fn ledger_mark_style(state: toolstrip::ToolState) -> Style {
    match state {
        toolstrip::ToolState::Running => PHOSPHOR_STYLE,
        toolstrip::ToolState::Passed => Style::new().fg(hud::HUD_VERIFIED),
        toolstrip::ToolState::Failed => Style::new().fg(hud::HUD_DANGER),
        toolstrip::ToolState::NotStarted | toolstrip::ToolState::Inconclusive => {
            Style::new().fg(hud::HUD_GOLD)
        }
    }
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
#[path = "../../../../tests/cockpit/app/draw__transcript_view__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/draw__transcript_view__reflow_tests.rs"]
mod reflow_tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/draw__transcript_view__async_tests.rs"]
mod async_tests;
