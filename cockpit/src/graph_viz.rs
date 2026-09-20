//! Round Table stage: the live agent-graph run.
//!
//! Renders the latest published `GraphSnapshot` (sticky across completion):
//! DAG layers as pulsing node rows (the rl_viz cells), per-node detail lines,
//! the event tail, and the final-answer preview. Pure render functions over
//! snapshots; no state of its own.

use crate::harness::{GraphNodePhase, GraphRunPhase, GraphSnapshot, current_graph_snapshot};
use crate::hud;
use crate::rl_viz::{NodeState, node_row};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span, Text},
};

pub(crate) fn title() -> String {
    match current_graph_snapshot() {
        None => " round table · idle ".to_string(),
        Some(snapshot) => {
            let status = match snapshot.phase {
                GraphRunPhase::Running => "running",
                GraphRunPhase::Done => "DONE",
                GraphRunPhase::Failed => "FAILED",
                GraphRunPhase::Cancelled => "cancelled",
            };
            format!(" round table · {} · {status} ", snapshot.graph)
        }
    }
}

pub(crate) fn render(time: f32, width: u16, height: u16) -> Text<'static> {
    let width = width as usize;
    let height = height as usize;
    if width < 12 || height == 0 {
        return Text::default();
    }
    let mut lines = match current_graph_snapshot() {
        None => idle_lines(width),
        Some(snapshot) => run_lines(&snapshot, time, width, height),
    };
    lines.truncate(height);
    Text::from(lines)
}

fn idle_lines(width: usize) -> Vec<Line<'static>> {
    vec![
        dim_line("No agent graph has run yet.", width),
        Line::default(),
        text_line(
            "/graph list — installed graphs (bundled + ~/.angel0/graphs)",
            width,
        ),
        text_line("/graph run <name> <task> — launch a run", width),
        text_line("/graph status · /graph stop", width),
        Line::default(),
        dim_line(
            "nodes = specialized agents · edges = depends_on routing · gates loop back",
            width,
        ),
    ]
}

fn phase_state(phase: GraphNodePhase) -> NodeState {
    match phase {
        GraphNodePhase::Pending => NodeState::Pending,
        GraphNodePhase::Running => NodeState::Active,
        GraphNodePhase::Done => NodeState::Done,
        GraphNodePhase::Failed => NodeState::Failed,
        GraphNodePhase::Skipped => NodeState::Rejected,
    }
}

/// Depth = longest dependency chain above the node; layer rows read top-down,
/// parallel nodes share a row.
fn layers(snapshot: &GraphSnapshot) -> Vec<Vec<usize>> {
    let index: std::collections::HashMap<&str, usize> = snapshot
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id.as_str(), i))
        .collect();
    fn depth_of(
        i: usize,
        snapshot: &GraphSnapshot,
        index: &std::collections::HashMap<&str, usize>,
        memo: &mut Vec<Option<usize>>,
        guard: usize,
    ) -> usize {
        if let Some(d) = memo[i] {
            return d;
        }
        // `guard` bounds recursion against malformed (cyclic) snapshots.
        let d = if guard == 0 {
            0
        } else {
            snapshot.nodes[i]
                .deps
                .iter()
                .filter_map(|dep| index.get(dep.as_str()).copied())
                .map(|dep| depth_of(dep, snapshot, index, memo, guard - 1) + 1)
                .max()
                .unwrap_or(0)
        };
        memo[i] = Some(d);
        d
    }
    let mut memo = vec![None; snapshot.nodes.len()];
    let mut out: Vec<Vec<usize>> = Vec::new();
    for i in 0..snapshot.nodes.len() {
        let d = depth_of(i, snapshot, &index, &mut memo, snapshot.nodes.len());
        while out.len() <= d {
            out.push(Vec::new());
        }
        out[d].push(i);
    }
    out
}

fn run_lines(
    snapshot: &GraphSnapshot,
    time: f32,
    width: usize,
    height: usize,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(text_line(
        &format!("task · {}", snapshot.task.replace('\n', " ")),
        width,
    ));

    for layer in layers(snapshot) {
        let cells: Vec<(&str, NodeState)> = layer
            .iter()
            .map(|&i| {
                (
                    snapshot.nodes[i].id.as_str(),
                    phase_state(snapshot.nodes[i].phase),
                )
            })
            .collect();
        lines.push(node_row(&cells, time, width));
    }

    lines.push(Line::default());
    for node in &snapshot.nodes {
        let mut detail = format!(
            "  {} · {}",
            node.id,
            match node.phase {
                GraphNodePhase::Pending => "pending",
                GraphNodePhase::Running => "running",
                GraphNodePhase::Done => "done",
                GraphNodePhase::Failed => "failed",
                GraphNodePhase::Skipped => "skipped",
            }
        );
        if !node.persona.is_empty() {
            detail.push_str(&format!(" · {}", node.persona));
        }
        detail.push_str(&format!(" · {}", node.club));
        if node.grant != "none" {
            detail.push_str(&format!(" · tools:{}", node.grant));
        }
        if let Some(pool) = &node.pool {
            detail.push_str(&format!(" · pool:{pool}"));
            if let Some(lease) = node.lease_id {
                detail.push_str(&format!(" · lease#{lease}"));
            }
            if let Some(limit) = node.pool_limit {
                detail.push_str(&format!(" · cap{limit}"));
            }
        }
        if node.elapsed_ms > 0 {
            detail.push_str(&format!(" · {:.1}s", node.elapsed_ms as f64 / 1000.0));
        }
        if node.output_chars > 0 {
            detail.push_str(&format!(" · {} chars", node.output_chars));
        }
        if let Some(gate) = &node.gate {
            detail.push_str(&format!(" · gate {gate}"));
        }
        let style = match node.phase {
            GraphNodePhase::Running => Style::new().fg(hud::HUD_GOLD),
            GraphNodePhase::Failed => Style::new().fg(hud::HUD_DANGER),
            GraphNodePhase::Done => Style::new().fg(hud::HUD_TEXT),
            _ => Style::new().fg(hud::HUD_DIM),
        };
        lines.push(Line::from(Span::styled(fit(&detail, width), style)));
    }

    if let Some(error) = &snapshot.error {
        lines.push(Line::from(Span::styled(
            fit(&format!("✗ {error}"), width),
            Style::new()
                .fg(hud::HUD_DANGER)
                .add_modifier(Modifier::BOLD),
        )));
    }
    if let Some(answer) = &snapshot.final_answer {
        lines.push(Line::from(Span::styled(
            fit("─ answer ─", width),
            Style::new().fg(hud::HUD_VERIFIED),
        )));
        for answer_line in answer.lines() {
            if lines.len() >= height {
                break;
            }
            lines.push(text_line(answer_line, width));
        }
    }

    // Event tail fills the remaining height.
    let remaining = height.saturating_sub(lines.len() + 1);
    if remaining >= 2 && !snapshot.events.is_empty() {
        lines.push(dim_line("─ timeline ─", width));
        let start = snapshot.events.len().saturating_sub(remaining);
        for event in snapshot.events.iter().skip(start) {
            lines.push(dim_line(event, width));
        }
    }
    lines
}

fn fit(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        text.to_string()
    } else if width <= 1 {
        "~".to_string()
    } else {
        let mut out: String = text.chars().take(width - 1).collect();
        out.push('~');
        out
    }
}

fn text_line(text: &str, width: usize) -> Line<'static> {
    Line::from(Span::styled(
        fit(text, width),
        Style::new().fg(hud::HUD_TEXT),
    ))
}

fn dim_line(text: &str, width: usize) -> Line<'static> {
    Line::from(Span::styled(
        fit(text, width),
        Style::new().fg(hud::HUD_DIM),
    ))
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/graph_viz__tests.rs"]
mod tests;
