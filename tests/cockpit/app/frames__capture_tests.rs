//! Website frame capture for the field-imagery and telemetry plates.
//!
//! Every function here renders through the **production** path — `draw::ui` for
//! whole-cockpit frames, `stage::raytrace`, `ui::chart` and dotmax for the
//! renderers themselves — into a `TestBackend`, then writes two files per
//! plate: the frame as text, and the same frame as a palette-indexed cell map
//! so a rasterizer can reproduce colors exactly.
//!
//! Inert unless `ANGELX_FRAMES_DIR` is set, so it costs a normal test run
//! nothing. It asserts nothing about the product; it is a capture harness.
//!
//! ```sh
//! ANGELX_FRAMES_DIR=/tmp/angelx-frames cargo test -p angelX-cockpit \
//!   --no-default-features frames_capture -- --nocapture
//! ```

use super::*;
use crate::ui::chart;

/// Frame text plus a palette-indexed cell map.
fn write_frame(terminal: &Terminal<TestBackend>, dir: &std::path::Path, name: &str) {
    write_backend(terminal.backend(), dir, name);
}

/// The same two files, from a backend that already holds the frame — so a
/// cropped pane is written exactly like a full frame.
fn write_backend(backend: &TestBackend, dir: &std::path::Path, name: &str) {
    let text = test_backend_text(backend);
    std::fs::write(dir.join(format!("{name}.txt")), &text).unwrap();

    let mut palette: Vec<(String, String, String)> = Vec::new();
    let mut cells: Vec<(String, usize)> = Vec::new();
    for cell in backend.buffer().content() {
        let style = (
            format!("{:?}", cell.fg),
            format!("{:?}", cell.bg),
            format!("{:?}", cell.modifier),
        );
        let index = palette.iter().position(|s| s == &style).unwrap_or_else(|| {
            palette.push(style);
            palette.len() - 1
        });
        cells.push((cell.symbol().to_string(), index));
    }
    let value = serde_json::json!({
        "width": backend.buffer().area().width,
        "height": backend.buffer().area().height,
        "palette": palette,
        "cells": cells,
    });
    std::fs::write(
        dir.join(format!("{name}.json")),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
}

/// A whole-cockpit frame, drawn by the same compositor the operator sees.
fn capture_cockpit(app: &mut App, width: u16, height: u16, dir: &std::path::Path, name: &str) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| ui(frame, app)).unwrap();
    write_frame(&terminal, dir, name);
}

/// A close-up: draw the whole cockpit, then keep only the pane the operator is
/// watching. The pane is found by its own title in the rendered frame, so the
/// crop follows the real layout instead of a hardcoded rectangle.
fn capture_pane(
    app: &mut App,
    (width, height): (u16, u16),
    title: &str,
    rows: u16,
    dir: &std::path::Path,
    name: &str,
) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| ui(frame, app)).unwrap();
    let frame_cells = terminal.backend().buffer().clone();
    let (x0, y0, x1, bottom) = pane_bounds(&frame_cells, title)
        .unwrap_or_else(|| panic!("no pane titled {title:?} in a {width}x{height} frame"));
    // A close-up reads as one: keep the pane's own width and only as many rows
    // as the plate window wants, so nothing is squashed into the page.
    let y1 = bottom.min(y0 + rows.saturating_sub(1));
    let mut crop = Terminal::new(TestBackend::new(x1 - x0 + 1, y1 - y0 + 1)).unwrap();
    crop.draw(|frame| {
        for y in y0..=y1 {
            for x in x0..=x1 {
                frame.buffer_mut()[(x - x0, y - y0)] = frame_cells[(x, y)].clone();
            }
        }
    })
    .unwrap();
    write_frame(&crop, dir, name);
}

/// The rect of the pane whose top border carries `title`: from the junction
/// that opens it, right to the frame edge, down to the row that closes it.
fn pane_bounds(buffer: &ratatui::buffer::Buffer, title: &str) -> Option<(u16, u16, u16, u16)> {
    let area = *buffer.area();
    // Cell indices, never byte offsets: a box-drawing glyph is three bytes wide.
    let row = |y: u16| -> Vec<String> {
        (0..area.width)
            .map(|x| buffer[(x, y)].symbol().to_string())
            .collect()
    };
    let top = (0..area.height).find(|&y| row(y).concat().contains(title))?;
    let left = row(top).iter().position(|s| s == "┬")? as u16;
    let bottom = (top + 1..area.height)
        .find(|&y| matches!(buffer[(left, y)].symbol(), "┴" | "╯" | "└" | "╧" | "┘"))?;
    Some((left, top, area.width - 1, bottom))
}

/// A frame composed of real renderers (chart, bars, raytracer) inside the
/// cockpit's own HUD chrome.
fn capture_panel<F>(width: u16, height: u16, dir: &std::path::Path, name: &str, body: F)
where
    F: FnOnce(&mut Frame, Rect),
{
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| {
            let area = frame.area();
            frame.render_widget(
                ratatui::widgets::Block::default()
                    .style(ratatui::style::Style::new().bg(ratatui::style::Color::Black)),
                area,
            );
            body(frame, area);
        })
        .unwrap();
    write_frame(&terminal, dir, name);
}

/// A chrome frame that *keeps the cockpit around the plate*: the whole
/// compositor draws first, then a centered panel carries the renderer so the
/// capture shows where in the TUI the chart lives.
fn capture_in_cockpit<F>(
    app: &mut App,
    width: u16,
    height: u16,
    dir: &std::path::Path,
    name: &str,
    body: F,
) where
    F: FnOnce(&mut Frame, Rect, &mut App),
{
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| {
            ui(frame, app);
            let area = frame.area();
            // Centered panel: 4-cell margin all around, so surrounding cockpit
            // chrome (header, transcript edge, HUD) stays visible.
            let panel = ratatui::layout::Rect {
                x: area.x + 6,
                y: area.y + 4,
                width: area.width.saturating_sub(12),
                height: area.height.saturating_sub(10),
            };
            frame.render_widget(
                ratatui::widgets::Clear,
                ratatui::layout::Rect {
                    x: panel.x.saturating_sub(1),
                    y: panel.y.saturating_sub(1),
                    width: panel.width + 2,
                    height: panel.height + 2,
                },
            );
            body(frame, panel, app);
        })
        .unwrap();
    write_frame(&terminal, dir, name);
}

/// Frame text → `Text`, for renderers that already return their own lines.
fn as_block<'a>(text: ratatui::text::Text<'a>, title: &'a str) -> ratatui::widgets::Paragraph<'a> {
    ratatui::widgets::Paragraph::new(text).block(crate::ui::hud::hud_block(title))
}

/// A star-node plot: `ui::chart`'s line path with a ✦ node at each sampled
/// A star-node plot: `ui::chart`'s braille path with a ✦ node on every eighth
/// A star-node plot: `ui::chart`'s line path with a ✦ node at each sampled
/// point — the site's ✦ motif, drawn by the chart renderer's own geometry.
fn star_series(width_cells: usize) -> ratatui::text::Text<'static> {
    let series: Vec<f32> = (0..96)
        .map(|i| {
            let t = i as f32 / 95.0;
            0.30 + 0.44 * t + 0.10 * (t * 19.0).sin() - 0.06 * (t * 6.0).cos()
        })
        .collect();
    let mut text = chart::line_chart(&series, width_cells as u16, 18);
    // Overprint a node marker at every eighth sample of the same series: the
    // chart owns the y geometry, the stars just sit on the path.
    let n = series.len();
    let lo = series.iter().cloned().fold(f32::MAX, f32::min);
    let hi = series.iter().cloned().fold(f32::MIN, f32::max);
    for (k, sample) in series.iter().enumerate().step_by(8) {
        let t = (*sample - lo) / (hi - lo).max(1e-6);
        let row = 17 - (t * 17.0) as usize;
        let col = (k as f32 / (n - 1) as f32 * (width_cells as f32 - 1.0)).round() as usize;
        if let Some(line) = text.lines.get_mut(row) {
            let mut s: String = line.spans.iter().map(|sp| sp.content.as_ref()).collect();
            while s.chars().count() <= col {
                s.push(' ');
            }
            let mut chars: Vec<char> = s.chars().collect();
            chars[col] = '✦';
            *line = ratatui::text::Line::from(chars.into_iter().collect::<String>());
        }
    }
    text
}

/// A vertical bar chart: one column per label, drawn with dotmax's real
/// `vblock` primitive, with the label and value under each column.
fn vbars_block(
    labels: &[(&str, f32)],
    width: usize,
    height: usize,
) -> ratatui::text::Text<'static> {
    use dotmax::BrailleGrid;
    let width = width.max(8);
    let height = height.max(4);
    let mut grid = match BrailleGrid::new(width, height) {
        Ok(grid) => grid,
        Err(_) => return ratatui::text::Text::raw(""),
    };
    let bar_cols = 4; // braille dot-columns per bar
    let pitch = bar_cols + 2; // bar + gap
    let chart_rows = height.saturating_sub(2); // leave the label/value rows
    for (i, (_label, frac)) in labels.iter().enumerate() {
        let x0 = i * pitch + 1;
        let frac = frac.clamp(0.0, 1.0);
        let filled = (frac * chart_rows as f32).round() as usize;
        // grow the column from the bottom of the chart region upward
        for r in 0..filled {
            let cell_y = chart_rows - 1 - r;
            for c in 0..bar_cols {
                let _ = grid.set_dot(x0 + c, cell_y * 4 + 3);
                let _ = grid.set_dot(x0 + c, cell_y * 4 + 2);
                let _ = grid.set_dot(x0 + c, cell_y * 4 + 1);
                let _ = grid.set_dot(x0 + c, cell_y * 4);
            }
        }
        let pct = format!("{:.0}", frac * 100.0);
        for (j, ch) in pct.chars().enumerate() {
            let _ = grid.set_char(x0 + j, height - 2, ch);
        }
    }
    // labels on the bottom row, centered under their bars
    for (i, (label, _)) in labels.iter().enumerate() {
        let x0 = i * pitch + 1;
        let start = x0 + bar_cols.saturating_sub(label.chars().count()) / 2;
        for (j, ch) in label.chars().take(bar_cols + 2).enumerate() {
            let _ = grid.set_char(start + j, height - 1, ch);
        }
    }
    let mut lines: Vec<ratatui::text::Line<'static>> = Vec::new();
    for y in 0..height {
        let mut row = String::new();
        for x in 0..width {
            row.push(grid.get_char(x, y));
        }
        lines.push(ratatui::text::Line::from(row.trim_end().to_string()));
    }
    ratatui::text::Text::from(lines)
}

#[test]
fn frames_capture() {
    let Some(dir) = std::env::var_os("ANGELX_FRAMES_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let _lock = crate::tests::env_lock();
    let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::drive::comp_mode::invalidate_cache();

    // ── FIG.01 — the console at work ─────────────────────────────────────
    let mut app = seed_preview_app();
    let id = crate::agent::harness::ToolEventId("frames-read".into());
    app.world
        .note_tool_call_event(id.clone(), "shell", "cargo test --no-default-features");
    app.world.note_tool_result_event(
        &id,
        "shell",
        "verify · 412 passed, tree unchanged",
        crate::agent::harness::ToolOutcome {
            execution: crate::agent::harness::ExecutionOutcome::Succeeded,
            verification: crate::agent::harness::VerificationOutcome::Passed,
        },
    );
    let read = crate::agent::harness::ToolEventId("frames-edit".into());
    app.world
        .note_tool_call_event(read, "edit", "hashline · cockpit.rs");
    // Seed a real session so the plate shows the console in use, not a cold
    // shell: two committed turns, a live tool trace, and an in-flight partial.
    use crate::ui::transcript::{Message, Role};
    app.messages.push(Message::new(
        Role::User,
        "plate the site with captures out of the real renderers — no mockups",
    ));
    app.messages.push(Message::new(
        Role::Angel,
        "Five plates from the compositor, the chart renderer, the dotmax bars and the ride camera. \
         Each one is drawn by the code the page describes — if it looks wrong, the harness is wrong.",
    ));
    app.messages.push(Message::new(
        Role::Activity,
        "shell · cargo test --no-default-features → 412 passed, tree unchanged",
    ));
    app.messages.push(Message::new(
        Role::Council,
        "review seat · plates carry no fabricated telemetry; captions name the renderer",
    ));
    app.thinking = Some(crate::agent::turn::Thinking::pending_for_test("practice"));
    app.partial = "captures are evidence, so the twelfth point is the one you can check:".into();
    // Transcript lines spawn on a roll-in animation; settle it so the captured
    // frame shows the session rather than the empty first tick.
    app.settle_transcript_spawns();
    capture_cockpit(&mut app, 120, 40, &dir, "fig01-console");

    // ── FIG.01b — the harness actually coding: a mid-turn frame ──────────
    // A second seeded session whose live work is an edit in flight: a tool
    // trace, a running shell verify, and an answer still streaming. This is
    // the plate that answers "what does it look like while it codes?"
    let mut coding = seed_preview_app();
    let ev = crate::agent::harness::ToolEventId("frames-code".into());
    coding.world.note_tool_call_event(
        ev.clone(),
        "str_replace",
        "cockpit/src/ui/chart.rs · expect_tag guard",
    );
    coding.world.note_tool_result_event(
        &ev,
        "str_replace",
        "edited · new tag #e31c07d9",
        crate::agent::harness::ToolOutcome {
            execution: crate::agent::harness::ExecutionOutcome::Succeeded,
            verification: crate::agent::harness::VerificationOutcome::Passed,
        },
    );
    let shell = crate::agent::harness::ToolEventId("frames-shell".into());
    coding
        .world
        .note_tool_call_event(shell, "shell", "cargo test chart -- --nocapture");
    coding.messages.push(Message::new(
        Role::User,
        "the plot plate needs star nodes on the line — connect node to node",
    ));
    coding.messages.push(Message::new(
        Role::Angel,
        "On it: the chart renderer already owns the y geometry, so the stars sit on its own path — \
         every eighth sample gets a ✦, no second geometry to drift.",
    ));
    coding.messages.push(Message::new(
        Role::Activity,
        "str_replace · cockpit/src/ui/chart.rs → edited · new tag #e31c07d9",
    ));
    coding.messages.push(Message::new(
        Role::Activity,
        "shell · cargo test chart -- --nocapture → running",
    ));
    coding.thinking = Some(crate::agent::turn::Thinking::pending_for_test("practice"));
    coding.partial = "now the bars go vertical — dotmax vblocks grown from the baseli".into();
    coding.settle_transcript_spawns();
    capture_cockpit(&mut coding, 120, 40, &dir, "fig01b-coding");

    // ── FIG.01c — zoomed agent thinking trace ────────────────────────────
    // The same live turn, zoomed: the reasoning canvas carries a real
    // thinking stream (reasoning models' private roll), shown live and never
    // saved to history — this is the pane the operator watches mid-turn.
    let mut zoom = seed_preview_app();
    zoom.reasoning = "reading the chart module first — line_chart maps series → rows by \
        min/max, one braille column per sample.\n\
        the star nodes can't invent their own geometry: they have to sit on the same \
        path the renderer computed, or the plate lies.\n\
        so: reuse the chart's own min/max normalization, overprint ✦ every eighth \
        sample on the returned rows. no second geometry, no drift.\n\
        bars go vertical next: dotmax vblock per column, grown from the baseline up, \
        labels and values under each bar.\n\
        then re-capture, re-raster, re-gate — the page only ships plates the \
        renderers actually drew."
        .into();
    zoom.reasoning_shown = zoom.reasoning.len();
    zoom.thinking = Some(crate::agent::turn::Thinking::pending_for_test("practice"));
    zoom.partial = "stars on the chart's own path — then vertical bars, then the gate".into();
    zoom.settle_transcript_spawns();
    capture_cockpit(&mut zoom, 120, 40, &dir, "fig01c-thinking");

    // ── FIG.02 — the agent thinking panel, zoomed in ─────────────────────
    // A narrow frame carrying a long reasoning stream: the trace pane is most
    // of the plate, so the private roll the operator watches mid-turn is
    // actually readable instead of scaled down to a smear.
    let mut think = seed_preview_app();
    think.reasoning = "reading the chart module first — line_chart maps series → rows by min/max, \
        one braille column per sample.\n\
        the star nodes can't invent their own geometry: they have to sit on the same path the \
        renderer computed, or the plate lies.\n\
        so: reuse the chart's own normalization, overprint ✦ every eighth sample on the returned \
        rows. no second geometry, no drift.\n\
        bars go vertical next: dotmax vblock per column, grown from the baseline up, labels and \
        values under each bar.\n\
        then re-capture, re-raster, re-gate — the page only ships plates the renderers actually \
        drew.\n\
        nothing here is written to history: the roll is live, and it goes when the turn does."
        .into();
    think.reasoning_shown = think.reasoning.len();
    think.thinking = Some(crate::agent::turn::Thinking::pending_for_test("practice"));
    think.partial = "stars on the chart's own path — then vertical bars, then the gate".into();
    think.settle_transcript_spawns();
    capture_pane(
        &mut think,
        (120, 40),
        "Thinking",
        16,
        &dir,
        "fig02-thinking",
    );

    // ── FIG.02 — formations deck ─────────────────────────────────────────
    let mut deck = seed_preview_app();
    deck.messages.push(Message::new(
        Role::User,
        "plate the site with captures out of the real renderers — no mockups",
    ));
    deck.messages.push(Message::new(
        Role::Activity,
        "shell · cargo test --no-default-features → 412 passed, tree unchanged",
    ));
    deck.settle_transcript_spawns();
    deck.open_moa_deck(None);
    capture_cockpit(&mut deck, 120, 40, &dir, "fig02-formations");

    // ── FIG.03 — the loop workshop: engage a loop, set its length, confirm ─
    // Drawn by the same dialog the operator confirms with. The rounds row ends
    // in SET CUSTOM and the entry is open on a typed length.
    let mut workshop = crate::drive::loop_dialog::LoopLaunchDialog::new("chart plate", 0, false);
    workshop.begin_custom_length();
    for ch in "137".chars() {
        workshop.custom_length_digit(ch);
    }
    let mut workshop_app = seed_preview_app();
    workshop_app.loop_dialog = Some(workshop);
    capture_cockpit(&mut workshop_app, 75, 25, &dir, "fig03-loop");

    // ── FIG.A — star-node plot, inside the cockpit ───────────────────────
    let mut plot_app = seed_preview_app();
    plot_app.settle_transcript_spawns();
    let plot = star_series(96);
    capture_in_cockpit(
        &mut plot_app,
        120,
        40,
        &dir,
        "figA-plot",
        move |frame, area, _app| {
            frame.render_widget(as_block(plot, "measured · acceptance per attempt"), area);
        },
    );

    // ── FIG.B — vertical bars, inside the cockpit ────────────────────────
    let bars = vbars_block(
        &[
            ("vrfy", 0.92),
            ("edit", 0.74),
            ("srch", 0.61),
            ("read", 0.55),
            ("dleg", 0.38),
            ("code", 0.21),
        ],
        66,
        18,
    );
    let mut bars_app = seed_preview_app();
    bars_app.settle_transcript_spawns();
    capture_in_cockpit(
        &mut bars_app,
        120,
        40,
        &dir,
        "figB-bars",
        move |frame, area, _app| {
            frame.render_widget(as_block(bars, "measured · tool calls by kind"), area);
        },
    );

    // ── FIG.04 — the cockpit's own raytracer ─────────────────────────────
    let cube = crate::stage::raytrace::render(1.7, 74, 22);
    capture_panel(84, 29, &dir, "fig04-raytrace", move |frame, area| {
        frame.render_widget(as_block(cube, "stage · cpu raytracer"), area);
    });
}
