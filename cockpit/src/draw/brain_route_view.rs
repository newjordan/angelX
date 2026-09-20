//! Brain Route floating-deck renderer.
//!
//! Menu state and input behavior remain in `app_control::agent_menu`; this
//! module owns only the terminal-native presentation and hit rectangles.

use super::*;

pub(super) fn render_agent_control_menu(frame: &mut Frame, app: &mut App) {
    use crate::agent::controls::{AgentMenuAction, AgentMenuKind};
    use ratatui::style::Color;

    app.agent_menu_hits.clear();
    app.agent_menu_area = None;
    let Some(menu) = app.agent_menu else {
        return;
    };
    if app.thinking.is_some() || app.bg_job.is_some() {
        app.agent_menu = None;
        app.agent_menu_search = None;
        return;
    }
    let root = frame.area();
    if root.width < 18 || root.height < 9 {
        app.agent_menu = None;
        app.agent_menu_search = None;
        return;
    }

    let route_choices = app.bag.route_choices();
    let context_used = app
        .tools
        .gauge
        .used_tokens
        .load(std::sync::atomic::Ordering::Relaxed);
    let thinking_route_index = menu.route_target.and_then(|target| {
        route_choices
            .iter()
            .position(|choice| choice.agent_index == target.0 && choice.slot_index == target.1)
    });
    let effort_levels = thinking_route_index
        .and_then(|index| route_choices.get(index))
        .map(|choice| choice.reasoning_levels.clone())
        .unwrap_or_default();
    // Picks are deliberately separate and advisory. A star is operational
    // reliability; a diamond is explicit user usefulness. Neither changes the
    // selected route without Enter.
    let operational_pick = crate::route_intelligence::operational_pick_for_context(
        &app.route_evidence,
        &route_choices,
        context_used,
    );
    let quality_pick = crate::route_intelligence::quality_pick_for_context(
        &app.route_evidence,
        &route_choices,
        context_used,
    );
    let operational_effort_pick = thinking_route_index
        .and_then(|index| route_choices.get(index))
        .and_then(|choice| {
            crate::route_intelligence::operational_effort_pick(
                &app.route_evidence,
                choice,
                &effort_levels,
            )
        });
    let quality_effort_pick = thinking_route_index
        .and_then(|index| route_choices.get(index))
        .and_then(|choice| {
            crate::route_intelligence::quality_effort_pick(
                &app.route_evidence,
                choice,
                &effort_levels,
            )
        });
    let query = app.agent_menu_search.as_deref().unwrap_or_default();
    let matching_indices = app.agent_menu_indices(menu, false);
    if query.is_empty()
        && match menu.kind {
            AgentMenuKind::Model => route_choices.is_empty(),
            AgentMenuKind::Thinking => effort_levels.is_empty(),
        }
    {
        app.agent_menu = None;
        app.agent_menu_search = None;
        return;
    }
    let no_matches = matching_indices.is_empty();
    let count = matching_indices.len().max(1);
    let chrome_rows = if app.agent_menu_details { 8 } else { 6 };
    let max_visible = root.height.saturating_sub(chrome_rows + 2).max(1) as usize;
    let visible = count.min(max_visible);
    let selected_index = matching_indices
        .iter()
        .copied()
        .find(|index| *index == menu.selected)
        .or_else(|| matching_indices.first().copied());
    let selected_position = selected_index
        .and_then(|selected| matching_indices.iter().position(|index| *index == selected))
        .unwrap_or(0);
    let start = selected_position
        .saturating_sub(visible / 2)
        .min(count.saturating_sub(visible));
    let desired_width = if root.width >= 150 {
        132
    } else if root.width >= 100 {
        112
    } else {
        82
    };
    let area = agent_menu_area(
        root,
        app.agent_control_area,
        desired_width,
        visible as u16 + chrome_rows,
    );
    app.agent_menu_area = Some(area);
    let background = Color::Rgb(2, 7, 12);
    let kind_title = match menu.kind {
        AgentMenuKind::Model => "MODEL",
        AgentMenuKind::Thinking => "THINK",
    };
    let title_effort = match menu.kind {
        AgentMenuKind::Model => selected_index
            .and_then(|index| route_choices.get(index))
            .and_then(|choice| choice.reasoning_effort.clone())
            .or_else(|| app.bag.reasoning_effort())
            .or_else(|| {
                let route = app
                    .bag
                    .in_hand_mode()
                    .unwrap_or_else(|| app.bag.in_hand_label().to_string());
                Some(if super::agent_panel_view::label_has_moa(&route) {
                    "mixed".to_string()
                } else {
                    "native".to_string()
                })
            }),
        AgentMenuKind::Thinking => selected_index
            .and_then(|index| effort_levels.get(index))
            .cloned(),
    };
    let kind_title = title_effort
        .as_deref()
        .map(|effort| format!("{kind_title}:{effort}"))
        .unwrap_or_else(|| kind_title.to_string());
    let title = match (&app.agent_menu_search, menu.confirm_target) {
        (_, Some(_)) => format!(" Brain Route · {kind_title} · CONFIRM "),
        (Some(query), None) => format!(
            " Brain Route · {kind_title} · /{} ",
            truncate_control_value(query, 32)
        ),
        (None, None) => format!(" Brain Route · {kind_title} "),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(HUD_BLUE_BORDER_STYLE)
        .style(Style::new().fg(HUD_BLUE).bg(background));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    for (row, position) in (start..start + visible).enumerate() {
        let row_area = Rect::new(inner.x, inner.y + row as u16, inner.width, 1);
        if no_matches {
            let empty = if query.is_empty() {
                "  No models ready · choose All to inspect connections".to_string()
            } else {
                format!("  no routes match /{query}")
            };
            frame.render_widget(
                Paragraph::new(truncate_control_value(&empty, inner.width as usize))
                    .style(Style::new().fg(HUD_DIM).bg(background)),
                row_area,
            );
            continue;
        }
        let index = matching_indices[position];
        let cursor = selected_index == Some(index);
        let (mut text, enabled, action, secondary, pressure) = match menu.kind {
            AgentMenuKind::Model => {
                let choice = &route_choices[index];
                let state = if choice.selected {
                    "●"
                } else if choice.available {
                    "·"
                } else {
                    "×"
                };
                let ops_pick = if app.agent_menu_details && operational_pick == Some(index) {
                    "★"
                } else {
                    ""
                };
                let quality_pick = if app.agent_menu_details && quality_pick == Some(index) {
                    "◆"
                } else {
                    ""
                };
                let selectable_effort = (!choice.reasoning_levels.is_empty()).then(|| {
                    let effort = choice.reasoning_effort.as_deref().unwrap_or("default");
                    (
                        format!(" [think: {effort} ▾]"),
                        AgentMenuAction::InspectEffort {
                            agent_index: choice.agent_index,
                            slot_index: choice.slot_index,
                        },
                    )
                });
                let context_badge = route_context_badge(&choice.metadata, context_used);
                let connection = crate::agent::controls::connection_label(choice);
                let status = if !choice.available { " · offline" } else { "" };
                (
                    format!(
                        "{} {state}{ops_pick}{quality_pick} {} · {connection}{status}{context_badge}",
                        if cursor { "›" } else { " " },
                        choice.model,
                    ),
                    choice.available,
                    AgentMenuAction::SelectRoute {
                        agent_index: choice.agent_index,
                        slot_index: choice.slot_index,
                    },
                    selectable_effort,
                    context_pressure(&choice.metadata, context_used),
                )
            }
            AgentMenuKind::Thinking => {
                let level = &effort_levels[index];
                let choice = &route_choices
                    [thinking_route_index.expect("THINK count requires a concrete route target")];
                let current = choice
                    .reasoning_effort
                    .as_deref()
                    .is_some_and(|effort| effort.eq_ignore_ascii_case(level));
                let ops_pick = if app.agent_menu_details && operational_effort_pick == Some(index) {
                    "★"
                } else {
                    " "
                };
                let quality_pick = if app.agent_menu_details && quality_effort_pick == Some(index) {
                    "◆"
                } else {
                    " "
                };
                (
                    format!(
                        "{} {}{ops_pick}{quality_pick} {level}",
                        if cursor { "›" } else { " " },
                        if !choice.available {
                            "×"
                        } else if current {
                            "●"
                        } else {
                            "·"
                        }
                    ),
                    choice.available,
                    AgentMenuAction::SelectEffort {
                        agent_index: choice.agent_index,
                        slot_index: choice.slot_index,
                        effort: level.clone(),
                    },
                    None,
                    ContextPressure::Normal,
                )
            }
        };
        let style = if cursor && enabled {
            Style::new().fg(Color::Black).bg(HUD_PHOSPHOR)
        } else if enabled {
            match pressure {
                ContextPressure::Normal => Style::new().fg(HUD_BLUE).bg(background),
                ContextPressure::Warning(_) => Style::new()
                    .fg(Color::Yellow)
                    .bg(background)
                    .add_modifier(Modifier::BOLD),
                ContextPressure::Critical(_) => Style::new()
                    .fg(Color::Red)
                    .bg(background)
                    .add_modifier(Modifier::BOLD),
            }
        } else {
            Style::new().fg(HUD_DIM).bg(background)
        };
        let split_secondary = secondary
            .as_ref()
            .filter(|(label, _)| inner.width as usize >= label.chars().count().saturating_add(18));
        let main_area = split_secondary.map_or(row_area, |(label, _)| {
            Rect::new(
                row_area.x,
                row_area.y,
                row_area.width.saturating_sub(label.chars().count() as u16),
                1,
            )
        });
        if split_secondary.is_none()
            && let Some((label, _)) = &secondary
        {
            text.push_str(label);
        }
        frame.render_widget(
            Paragraph::new(truncate_control_value(&text, main_area.width as usize)).style(style),
            main_area,
        );
        if let Some((label, secondary_action)) = split_secondary {
            let secondary_area = Rect::new(
                main_area.x.saturating_add(main_area.width),
                row_area.y,
                label.chars().count() as u16,
                1,
            );
            let secondary_style = if cursor && enabled {
                style
            } else if enabled {
                Style::new()
                    .fg(HUD_PHOSPHOR)
                    .bg(background)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(HUD_DIM).bg(background)
            };
            frame.render_widget(
                Paragraph::new(label.as_str()).style(secondary_style),
                secondary_area,
            );
            if enabled {
                app.agent_menu_hits
                    .push((secondary_area, secondary_action.clone()));
            }
        }
        if enabled {
            app.agent_menu_hits.push((main_area, action));
        }
    }

    let controls_y = inner.y.saturating_add(visible as u16);
    let unavailable = route_choices
        .iter()
        .filter(|choice| !choice.available)
        .count();
    let controls = [
        (
            if app.agent_menu_details {
                "[d Less]".to_string()
            } else {
                "[d Details]".to_string()
            },
            AgentMenuAction::ToggleDetails,
        ),
        (
            if app.agent_menu_show_unavailable {
                "[a Ready only]".to_string()
            } else {
                format!("[a All · {unavailable} offline]")
            },
            AgentMenuAction::ToggleUnavailable,
        ),
    ];
    let mut controls_x = inner.x;
    for (label, action) in controls {
        if controls_y >= inner.bottom()
            || (action == AgentMenuAction::ToggleUnavailable && menu.kind != AgentMenuKind::Model)
        {
            continue;
        }
        let width = (label.chars().count() as u16).min(inner.right().saturating_sub(controls_x));
        let rect = Rect::new(controls_x, controls_y, width, 1);
        frame.render_widget(
            Paragraph::new(label).style(Style::new().fg(HUD_BLUE).bg(background)),
            rect,
        );
        if width > 0 {
            app.agent_menu_hits.push((rect, action));
        }
        controls_x = controls_x.saturating_add(width + 2).min(inner.right());
    }
    let detail_y = controls_y.saturating_add(1);

    let selected_route_index = match menu.kind {
        AgentMenuKind::Model => selected_index,
        AgentMenuKind::Thinking => thinking_route_index,
    };
    let selected_route = selected_route_index.and_then(|index| route_choices.get(index));
    let selected_effort = match menu.kind {
        AgentMenuKind::Model => {
            selected_route.and_then(|choice| choice.reasoning_effort.as_deref())
        }
        AgentMenuKind::Thinking => selected_index
            .and_then(|index| effort_levels.get(index))
            .map(String::as_str),
    };
    let selected_is_operational_pick = match menu.kind {
        AgentMenuKind::Model => {
            operational_pick.is_some() && operational_pick == selected_route_index
        }
        AgentMenuKind::Thinking => {
            operational_effort_pick.is_some() && operational_effort_pick == selected_index
        }
    };
    let selected_is_quality_pick = match menu.kind {
        AgentMenuKind::Model => quality_pick.is_some() && quality_pick == selected_route_index,
        AgentMenuKind::Thinking => {
            quality_effort_pick.is_some() && quality_effort_pick == selected_index
        }
    };
    let evidence = selected_route.and_then(|choice| {
        app.route_evidence
            .find(&choice.driver, &choice.model, selected_effort)
    });
    if app.agent_menu_details && detail_y < inner.y + inner.height {
        let mut history = evidence
            .filter(|evidence| evidence.samples > 0)
            .map_or_else(
                || "ops: cold start · reliability/latency only, not answer quality".to_string(),
                |evidence| {
                    format!(
                        "ops: n{} · clean {}% · p50 {} · rec {} · avg {} tok",
                        evidence.samples,
                        evidence.clean_percent(),
                        format_evidence_duration(evidence.p50_latency_ms),
                        evidence.recovered_answers,
                        format_evidence_tokens(evidence.mean_tokens),
                    )
                },
            );
        if app.route_evidence.malformed_lines > 0 {
            history.push_str(&format!(
                " · {} malformed skipped",
                app.route_evidence.malformed_lines
            ));
        }
        if selected_is_operational_pick {
            history.insert_str(0, "★ ops pick · ");
        }
        frame.render_widget(
            Paragraph::new(truncate_control_value(&history, inner.width as usize))
                .style(Style::new().fg(HUD_DIM).bg(background)),
            Rect::new(inner.x, detail_y, inner.width, 1),
        );
    }
    let quality_y = detail_y.saturating_add(1);
    if app.agent_menu_details && quality_y < inner.y + inner.height {
        let mut quality = evidence
            .filter(|evidence| evidence.user_quality_samples() > 0)
            .map_or_else(
                || "quality: unrated · explicit user labels only".to_string(),
                |evidence| {
                    format!(
                        "quality: {}/{} useful · {}% · explicit user labels",
                        evidence.user_useful,
                        evidence.user_quality_samples(),
                        evidence.useful_percent(),
                    )
                },
            );
        if selected_is_quality_pick {
            quality.insert_str(0, "◆ user pick · ");
        }
        frame.render_widget(
            Paragraph::new(truncate_control_value(&quality, inner.width as usize))
                .style(Style::new().fg(HUD_DIM).bg(background)),
            Rect::new(inner.x, quality_y, inner.width, 1),
        );
    }
    let capability_y = if app.agent_menu_details {
        quality_y.saturating_add(1)
    } else {
        detail_y
    };
    if capability_y < inner.y + inner.height {
        let capability = match menu.kind {
            AgentMenuKind::Model => selected_route.map_or_else(
                || "model: backend-native metadata unavailable".to_string(),
                |choice| model_capability_detail(&choice.metadata),
            ),
            AgentMenuKind::Thinking => {
                let explanation = selected_route
                    .and_then(|choice| {
                        selected_effort
                            .and_then(|effort| choice.metadata.reasoning_description(effort))
                    })
                    .unwrap_or("backend-supported level");
                format!("effort: {explanation}")
            }
        };
        frame.render_widget(
            Paragraph::new(truncate_control_value(&capability, inner.width as usize))
                .style(Style::new().fg(HUD_DIM).bg(background)),
            Rect::new(inner.x, capability_y, inner.width, 1),
        );
    }
    let route_detail_y = capability_y.saturating_add(1);
    if route_detail_y < inner.y + inner.height {
        let confirming = menu.confirm_target.is_some();
        let detail = if confirming {
            selected_route.map_or_else(
                || "confirm: target unavailable".to_string(),
                |choice| {
                    let pressure = choice
                        .metadata
                        .context_usage_percent(context_used)
                        .map(context_usage_status)
                        .unwrap_or_else(|| "ctx unknown".to_string());
                    format!(
                        "confirm: {} ▸ {} · {pressure} · second commit forces",
                        choice.agent, choice.model
                    )
                },
            )
        } else {
            match menu.kind {
                AgentMenuKind::Model => selected_route.map_or_else(
                    || "No matching model".to_string(),
                    |choice| {
                        let thinking = if choice.reasoning_levels.is_empty() {
                            choice.reasoning_effort.as_ref().map_or_else(
                                || "model-native".to_string(),
                                |effort| format!("{effort} · fixed"),
                            )
                        } else {
                            format!(
                                "{} · Tab to change",
                                choice.reasoning_effort.as_deref().unwrap_or("default")
                            )
                        };
                        format!(
                            "Connection: {} · thinking: {thinking}",
                            crate::agent::controls::connection_label(choice)
                        )
                    },
                ),
                AgentMenuKind::Thinking => selected_route.map_or_else(
                    || "route: target unavailable".to_string(),
                    |choice| {
                        if choice.available {
                            format!(
                                "route: {} ▸ {} · Enter applies model + effort",
                                choice.agent, choice.model
                            )
                        } else {
                            format!(
                                "route: {} ▸ {} · OFFLINE · selection locked",
                                choice.agent, choice.model
                            )
                        }
                    },
                ),
            }
        };
        let detail_style = if confirming {
            Style::new()
                .fg(Color::Red)
                .bg(background)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(HUD_DIM).bg(background)
        };
        frame.render_widget(
            Paragraph::new(truncate_control_value(&detail, inner.width as usize))
                .style(detail_style),
            Rect::new(inner.x, route_detail_y, inner.width, 1),
        );
    }
    let footer_y = route_detail_y.saturating_add(1);
    if footer_y < inner.y + inner.height {
        let footer = if menu.confirm_target.is_some() {
            "FULL CONTEXT · Enter/click again to force · Esc, then /compact recommended"
        } else {
            match (&app.agent_menu_search, menu.kind) {
                (Some(_), _) => "type to filter · Backspace · ↑↓ · Enter · Esc clear search",
                (None, AgentMenuKind::Model) => {
                    if app.agent_menu_details {
                        "o ops · q quality · / search · ↑↓ Enter select · Tab thinking · Esc close"
                    } else {
                        "/ search · ↑↓ choose · Enter select · Tab thinking · Esc close"
                    }
                }
                (None, AgentMenuKind::Thinking) => {
                    if app.agent_menu_details {
                        "o ops · q quality · ↑↓ Enter apply · Tab models · Esc cancel"
                    } else {
                        "↑↓ choose · Enter apply model + thinking · Tab models · Esc cancel"
                    }
                }
            }
        };
        let footer_style = if menu.confirm_target.is_some() {
            Style::new()
                .fg(Color::Red)
                .bg(background)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(HUD_DIM).bg(background)
        };
        frame.render_widget(
            Paragraph::new(footer).style(footer_style),
            Rect::new(inner.x, footer_y, inner.width, 1),
        );
    }
}
