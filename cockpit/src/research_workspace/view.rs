//! Terminal-cell geometry and presentation for the research workspace.

use super::{Action, Entry, Lens, Place, State, Workspace};
use crate::hud;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub(crate) fn state_color(state: State) -> Color {
    match state {
        State::Running => hud::HUD_PHOSPHOR,
        State::Verified => hud::HUD_VERIFIED,
        State::Failed => hud::HUD_DANGER,
        State::Denied | State::Inconclusive => hud::HUD_GOLD,
        State::Succeeded | State::Recorded => hud::HUD_TEXT,
        State::Pending | State::Cancelled => hud::HUD_DIM,
    }
}

pub(crate) fn fit(text: &str, width: usize) -> String {
    let text = text.replace(['\n', '\r', '\t'], " ");
    if text.width() <= width {
        return text;
    }
    if width == 0 {
        return String::new();
    }
    let mut used = 0;
    let mut out = String::new();
    for ch in text.chars() {
        let w = ch.width().unwrap_or(0);
        if used + w > width - 1 {
            break;
        }
        used += w;
        out.push(ch);
    }
    out.push('…');
    out
}

fn wrapped(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for paragraph in text.lines() {
        let mut row = String::new();
        let mut used = 0;
        for ch in paragraph.chars() {
            let w = ch.width().unwrap_or(0);
            if used + w > width && !row.is_empty() {
                out.push(std::mem::take(&mut row));
                used = 0;
            }
            if w <= width {
                row.push(ch);
                used += w;
            }
        }
        out.push(row);
    }
    out
}

struct Row {
    line: Line<'static>,
    action: Option<Action>,
    entry: Option<usize>,
}

fn row(text: impl Into<String>, color: Color) -> Row {
    Row {
        line: Line::from(Span::styled(text.into(), Style::new().fg(color))),
        action: None,
        entry: None,
    }
}

fn entry_rows(entry: &Entry, index: usize, selected: bool, lens: Lens, width: usize) -> Vec<Row> {
    let prefix = if selected { "▸" } else { " " };
    let label = if lens == Lens::Ledger && width >= 88 {
        let title = fit(&entry.title, 20);
        format!(
            "{prefix} {:>3} {:<12} {:<12} {}{} {}",
            index + 1,
            entry.state.label(),
            entry.place.label(),
            title,
            " ".repeat(20_usize.saturating_sub(title.width())),
            entry.summary
        )
    } else if lens == Lens::Ledger && width >= 62 {
        format!(
            "{prefix} {:>3} {:<12} {:<12} {}",
            index + 1,
            entry.state.label(),
            entry.place.label(),
            entry.title
        )
    } else {
        format!("{prefix} {:<12} {}", entry.state.label(), entry.title)
    };
    let style = Style::new().fg(state_color(entry.state));
    let mut out = vec![Row {
        line: Line::from(Span::styled(
            fit(&label, width),
            if selected {
                style.add_modifier(Modifier::BOLD)
            } else {
                style
            },
        )),
        action: Some(Action::Select(index)),
        entry: Some(index),
    }];
    if lens == Lens::Story {
        out.push(Row {
            line: Line::from(Span::styled(
                fit(
                    &format!("  {} · {}", entry.place.label(), entry.source),
                    width,
                ),
                Style::new().fg(hud::HUD_DIM),
            )),
            action: Some(Action::Select(index)),
            entry: Some(index),
        });
        if !entry.summary.is_empty() {
            out.push(Row {
                line: Line::from(Span::styled(
                    fit(&format!("  {}", entry.summary), width),
                    Style::new().fg(hud::HUD_TEXT),
                )),
                action: Some(Action::Select(index)),
                entry: Some(index),
            });
        }
        out.push(row("", hud::HUD_DIM));
    }
    out
}

pub(crate) fn render(
    frame: &mut Frame,
    workspace: &mut Workspace,
    area: Rect,
    context: &str,
) -> Vec<(Rect, Action)> {
    let mut hits = Vec::new();
    if area.width == 0 || area.height == 0 {
        return hits;
    }
    let width = area.width as usize;
    let mut y = area.y;
    let bottom = area.bottom();
    let heading = format!(
        "{} / {}",
        workspace.place.label(),
        if workspace.inspecting {
            "Evidence"
        } else {
            workspace.place.purpose()
        }
    );
    paint_line(
        frame,
        &mut y,
        bottom,
        area,
        Line::from(Span::styled(
            fit(&heading, width),
            Style::new()
                .fg(hud::HUD_PHOSPHOR)
                .add_modifier(Modifier::BOLD),
        )),
    );
    if area.height < 5 {
        paint_line(frame, &mut y, bottom, area, Line::from(fit(context, width)));
        return hits;
    }

    let mut x = area.x;
    let compact = width < 66;
    let short = ["Keep", "Table", "Smithy", "Scope", "Library"];
    for (index, place) in Place::ALL.iter().copied().enumerate() {
        let label = if compact { short[index] } else { place.label() };
        let label = format!(
            "{}{}",
            if workspace.place == place {
                '▸'
            } else {
                place.building().glyph()
            },
            label
        );
        let w = label.width() as u16;
        if x + w > area.right() {
            break;
        }
        let rect = Rect::new(x, y, w, 1);
        let style = Style::new().fg(
            if place == workspace.place || workspace.active_places[place as usize] > 0 {
                hud::HUD_PHOSPHOR
            } else {
                hud::HUD_DIM
            },
        );
        frame.render_widget(Paragraph::new(label).style(style), rect);
        hits.push((rect, Action::Place(place)));
        x += w + 1;
    }
    y += 1;
    let mut x = area.x;
    let mut controls: Vec<_> = Lens::ALL
        .into_iter()
        .enumerate()
        .map(|(index, lens)| {
            (
                format!("{} {}", index + 1, lens.label()),
                Action::Lens(lens),
            )
        })
        .collect();
    controls.extend([
        (
            (if workspace.following {
                "Live ●"
            } else {
                "Live ○"
            })
            .into(),
            Action::Follow,
        ),
        (
            (if workspace.expanded {
                "Compact"
            } else {
                "Expand"
            })
            .into(),
            Action::Expand,
        ),
        ("World".into(), Action::World),
    ]);
    for (label, action) in controls {
        let w = label.width() as u16;
        if x + w > area.right() {
            break;
        }
        let rect = Rect::new(x, y, w, 1);
        let selected = action == Action::Lens(workspace.lens)
            || (action == Action::Follow && workspace.following);
        frame.render_widget(
            Paragraph::new(label).style(Style::new().fg(if selected {
                hud::HUD_PHOSPHOR
            } else {
                hud::HUD_TEXT
            })),
            rect,
        );
        hits.push((rect, action));
        x += w + 2;
    }
    y += 1;
    paint_line(
        frame,
        &mut y,
        bottom,
        area,
        Line::from(Span::styled(
            fit(context, width),
            Style::new().fg(hud::HUD_DIM),
        )),
    );

    if let Some(experiment) = &workspace.experiment_filter
        && y < bottom.saturating_sub(1)
    {
        let rect = Rect::new(area.x, y, area.width, 1);
        frame.render_widget(
            Paragraph::new(fit(
                &format!("EXPERIMENT · {experiment} · [all experiments]"),
                width,
            ))
            .style(Style::new().fg(hud::HUD_PHOSPHOR)),
            rect,
        );
        hits.push((rect, Action::ClearExperiment));
        y += 1;
    }
    let body_height = bottom.saturating_sub(y).saturating_sub(1) as usize;
    workspace.viewport_height = body_height;
    if body_height == 0 {
        return hits;
    }
    let mut rows = Vec::new();
    if workspace.inspecting {
        if let Some(entry) = workspace.selected_entry() {
            rows.push(row(fit(&entry.title, width), hud::HUD_TEXT));
            rows.push(row(
                format!("{} · {}", entry.state.label(), entry.source),
                state_color(entry.state),
            ));
            rows.extend(
                wrapped(&format!("ID · {}", entry.id), width)
                    .into_iter()
                    .map(|text| row(text, hud::HUD_DIM)),
            );
            if let Some(parent) = &entry.parent {
                rows.extend(
                    wrapped(&format!("From · {parent}"), width)
                        .into_iter()
                        .map(|text| row(text, hud::HUD_PHOSPHOR)),
                );
            }
            if entry.experiment_id.is_some() {
                let mut focus = row("[e] Open this experiment", hud::HUD_PHOSPHOR);
                focus.action = Some(Action::FocusExperiment);
                rows.push(focus);
            }
            for (index, (_, label)) in entry.links.iter().take(9).enumerate() {
                let mut link = row(
                    fit(&format!("[{}] → {label}", index + 1), width),
                    hud::HUD_PHOSPHOR,
                );
                link.action = Some(Action::Related(index));
                rows.push(link);
            }
            rows.push(row("", hud::HUD_DIM));
            rows.extend(
                wrapped(&entry.detail, width)
                    .into_iter()
                    .map(|text| row(text, hud::HUD_TEXT)),
            );
        }
    } else {
        let measurements: Vec<_> = workspace
            .visible
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.measurement.is_some())
            .collect();
        if !measurements.is_empty() && workspace.lens != Lens::Flow {
            rows.push(row(
                "RESEARCH PROGRESS · reported measurements",
                hud::HUD_PHOSPHOR,
            ));
            if width >= 72 {
                rows.push(row(
                    " EXPERIMENT              DATASET           PARENT        VALUE  CHANGE",
                    hud::HUD_DIM,
                ));
            }
            for (index, entry) in measurements.iter().rev().take(12).rev() {
                let sample = entry.measurement.as_ref().unwrap();
                let parent = sample
                    .baseline
                    .as_ref()
                    .map(|score| score.as_str())
                    .unwrap_or("—");
                let text = if width >= 72 {
                    format!(
                        " {:<23} {:<15} {:>10} {:>12}  {}",
                        fit(&entry.title, 23),
                        sample.dataset_label(),
                        fit(parent, 10),
                        fit(sample.value.as_str(), 12),
                        sample.comparison()
                    )
                } else {
                    format!(
                        " {} · {} · {} → {} · {}",
                        entry.title,
                        sample.dataset_label(),
                        parent,
                        sample.value.as_str(),
                        sample.comparison()
                    )
                };
                let mut progress = row(fit(&text, width), hud::HUD_TEXT);
                progress.action = Some(Action::Select(*index));
                rows.push(progress);
            }
            if measurements.len() > 12 {
                rows.push(row(
                    format!(
                        "{} earlier measurements remain in the ledger",
                        measurements.len() - 12
                    ),
                    hud::HUD_DIM,
                ));
            }
            rows.push(row(
                "Datasets stay separate; reports do not bind official rewards.",
                hud::HUD_DIM,
            ));
            rows.push(row("", hud::HUD_DIM));
        }
        if let Some(filter) = workspace.filter {
            let mut filter_row = row(
                format!(
                    "{} · {} records · [clear filter]",
                    filter.label(),
                    workspace.visible.len()
                ),
                state_color(filter),
            );
            filter_row.action = Some(Action::ClearFilter);
            rows.push(filter_row);
        }
        if workspace.lens == Lens::Flow {
            let mut edges = Vec::new();
            for (index, entry) in workspace.visible.iter().enumerate() {
                if !matches!(
                    entry.source,
                    "agent graph episode" | "experiment receipt journal"
                ) {
                    continue;
                }
                for (target, _) in entry
                    .links
                    .iter()
                    .filter(|(_, label)| label.starts_with("Dependency · "))
                {
                    if let Some(input) = workspace.visible.iter().find(|other| &other.id == target)
                    {
                        let mut edge = row(
                            fit(
                                &format!(
                                    " {} ─→ {} · {}",
                                    input.title,
                                    entry.title,
                                    entry.state.label()
                                ),
                                width,
                            ),
                            state_color(entry.state),
                        );
                        edge.action = Some(Action::Select(index));
                        edges.push(edge);
                    }
                }
            }
            if !edges.is_empty() {
                rows.push(row(
                    if workspace
                        .visible
                        .iter()
                        .any(|entry| entry.source == "experiment receipt journal")
                    {
                        "RESEARCH LINEAGE · declared evidence links"
                    } else {
                        "AGENT HANDOFFS · declared dependencies"
                    },
                    hud::HUD_PHOSPHOR,
                ));
                rows.extend(edges);
                rows.push(row("", hud::HUD_DIM));
            }
            rows.push(row("OBSERVED WORK → RECORDED OUTCOME", hud::HUD_TEXT));
            rows.push(row(
                format!(
                    "{} records · widths show record counts",
                    workspace.visible.len()
                ),
                hud::HUD_DIM,
            ));
            let total = workspace.visible.len();
            for state in State::ALL {
                let count = workspace
                    .visible
                    .iter()
                    .filter(|entry| entry.state == state)
                    .count();
                if count == 0 {
                    continue;
                }
                let budget = width.saturating_sub(22).min(32);
                let n = if total == 0 {
                    0
                } else {
                    (count * budget).div_ceil(total)
                };
                let mut branch = row(
                    fit(
                        &format!(" ├─ {:<12} {:>3} {}", state.label(), count, "━".repeat(n)),
                        width,
                    ),
                    state_color(state),
                );
                branch.action = Some(Action::Filter(state));
                rows.push(branch);
            }
            rows.push(row("", hud::HUD_DIM));
            rows.push(row("RECORDS · select a branch to narrow", hud::HUD_DIM));
        } else if workspace.lens == Lens::Ledger {
            rows.push(row(
                if width >= 88 {
                    "    # STATE        PLACE        OPERATION            OBSERVATION"
                } else if width >= 62 {
                    "    # STATE        PLACE        OPERATION"
                } else {
                    "  STATE        OPERATION"
                },
                hud::HUD_DIM,
            ));
        }
        if workspace.visible.is_empty() {
            rows.push(row("Ready for observed work", hud::HUD_TEXT));
            for text in wrapped(
                "Tool activity appears here as your agent works. Existing loop findings and measured RL checkpoints join their places. Select a place above or open World to explore the Realm.",
                width,
            ) {
                rows.push(row(text, hud::HUD_DIM));
            }
        }
        for (index, entry) in workspace.visible.iter().enumerate() {
            rows.extend(entry_rows(
                entry,
                index,
                Some(&entry.id) == workspace.selected.as_ref(),
                workspace.lens,
                width,
            ));
        }
    }
    let max_offset = rows.len().saturating_sub(body_height);
    let offset = if workspace.inspecting {
        workspace.detail_scroll = workspace.detail_scroll.min(max_offset);
        workspace.detail_scroll
    } else {
        if !workspace.reveal_selection
            && !workspace.following
            && let Some((id, relative_line)) = &workspace.scroll_anchor
            && let Some(index) = workspace.visible.iter().position(|entry| &entry.id == id)
            && let Some(line) = rows.iter().position(|row| row.entry == Some(index))
        {
            workspace.offset = (line as isize - relative_line).max(0) as usize;
        }
        if workspace.reveal_selection {
            if let Some(selected) = workspace.selected_index()
                && let Some(line) = rows.iter().position(|row| row.entry == Some(selected))
            {
                if line < workspace.offset {
                    workspace.offset = line;
                }
                if line >= workspace.offset + body_height {
                    workspace.offset = line + 1 - body_height;
                }
            }
            workspace.reveal_selection = false;
        }
        workspace.offset = workspace.offset.min(max_offset);
        workspace.scroll_anchor = workspace.selected_index().and_then(|index| {
            rows.iter()
                .position(|row| row.entry == Some(index))
                .map(|line| {
                    (
                        workspace.visible[index].id.clone(),
                        line as isize - workspace.offset as isize,
                    )
                })
        });
        workspace.offset
    };
    for row in rows.into_iter().skip(offset).take(body_height) {
        let rect = Rect::new(area.x, y, area.width, 1);
        frame.render_widget(Paragraph::new(row.line), rect);
        if let Some(action) = row.action {
            hits.push((rect, action));
        }
        y += 1;
    }
    let footer = Rect::new(area.x, bottom - 1, area.width, 1);
    let hint = if workspace.inspecting {
        "[Back] · ↑↓ scroll · PgUp/PgDn · e experiment"
    } else if width >= 65 {
        "[Inspect] · ↑↓ select · ←→ places · e experiment · v filter · f live · w world"
    } else {
        "[Inspect] · ↑↓ · z expand"
    };
    frame.render_widget(
        Paragraph::new(fit(hint, width)).style(Style::new().fg(hud::HUD_DIM)),
        footer,
    );
    hits.push((
        Rect::new(
            footer.x,
            footer.y,
            (if workspace.inspecting { 6 } else { 9 }).min(footer.width),
            1,
        ),
        if workspace.inspecting {
            Action::Back
        } else {
            Action::Inspect
        },
    ));
    hits
}

fn paint_line(frame: &mut Frame, y: &mut u16, bottom: u16, area: Rect, line: Line<'static>) {
    if *y < bottom {
        frame.render_widget(Paragraph::new(line), Rect::new(area.x, *y, area.width, 1));
        *y += 1;
    }
}
