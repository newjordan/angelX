//! World-status budget rendering (module-breakup: the budget cluster,
//! extracted from `world_viz.rs`).
//!
//! Pure text shaping: the token-budget status line, its health bar, and the
//! span fitting helpers the status strip uses. Consumed by the parent's
//! status path through re-exports.

#![cfg_attr(not(test), allow(dead_code))]

use super::{LoopBudgetSnapshot, cell_width, fit_cells};
use ratatui::text::Span;

pub(crate) fn world_budget_status(budget: LoopBudgetSnapshot, width: usize) -> Option<String> {
    if width < 12 {
        return None;
    }
    let pair = budget_pair(budget.tokens_spent as u64, budget.token_budget as u64);
    if width < 30 {
        return Some(format!("tok {pair}"));
    }
    let bar_w = if width >= 50 { 10 } else { 8 };
    Some(format!(
        "tok [{}] {pair}",
        health_bar(
            remaining_fraction(budget.tokens_spent as u64, budget.token_budget as u64),
            bar_w,
        )
    ))
}

pub(crate) fn budget_pair(used: u64, cap: u64) -> String {
    if cap == 0 {
        format!("{}/inf", compact_count(used))
    } else {
        format!("{}/{}", compact_count(used), compact_count(cap))
    }
}

pub(crate) fn health_bar(remaining: f32, width: usize) -> String {
    let width = width.max(1);
    let filled = (remaining.clamp(0.0, 1.0) * width as f32).round() as usize;
    format!(
        "{}{}",
        "#".repeat(filled.min(width)),
        "-".repeat(width.saturating_sub(filled))
    )
}

pub(crate) fn remaining_fraction(used: u64, cap: u64) -> f32 {
    if cap == 0 {
        1.0
    } else {
        1.0 - (used as f32 / cap.max(1) as f32).clamp(0.0, 1.0)
    }
}

pub(crate) fn compact_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}m", n as f64 / 1_000_000.0).replace(".0m", "m")
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0).replace(".0k", "k")
    } else {
        n.to_string()
    }
}

pub(crate) fn spans_cell_width(spans: &[Span<'_>]) -> usize {
    spans
        .iter()
        .map(|span| cell_width(span.content.as_ref()))
        .sum()
}

pub(crate) fn fit_spans_to_cells(spans: Vec<Span<'static>>, max: usize) -> Vec<Span<'static>> {
    let mut fitted = Vec::with_capacity(spans.len());
    let mut remaining = max;
    for span in spans {
        let width = cell_width(span.content.as_ref());
        if width <= remaining {
            remaining -= width;
            fitted.push(span);
            continue;
        }
        if remaining > 0 {
            fitted.push(Span::styled(
                fit_cells(span.content.as_ref(), remaining),
                span.style,
            ));
        }
        break;
    }
    fitted
}
