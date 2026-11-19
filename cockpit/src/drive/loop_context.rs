//! Standing context files for a loop's fresh iterations.
//!
//! A fresh iteration opens with curated state, but the run's own journal and
//! handoff live in workspace files the model must spend tool calls re-reading
//! before it can work. Measured on a long optimisation task, that
//! re-orientation took 16% to 30% of the spend where a harness that inlined
//! its journal spent 3%. `ANGEL_LOOP_CONTEXT_FILES` names workspace files
//! (comma-separated, relative to the workspace unless absolute) whose current
//! contents ride the iteration's system prompt inline, bounded so a grown
//! journal cannot crowd out the task. Carried iterations keep the system
//! prompt they started with, so the files are read once per fresh start —
//! the model that has been writing the journal all along does not need it
//! pasted back every iteration.
//!
//! `ANGEL_LOOP_CONTEXT_CHARS` sets the total character budget across all
//! files (default 96,000). A file over its share keeps its head and tail
//! with the elision marked; a missing or unreadable file is skipped, never
//! fatal — the loop must start even before its journal exists.

use std::path::{Path, PathBuf};

const DEFAULT_TOTAL_CHARS: usize = 96_000;
const ELISION: &str = "\n[... elided by the loop to fit its context budget ...]\n";

fn total_chars() -> usize {
    std::env::var("ANGEL_LOOP_CONTEXT_CHARS")
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .filter(|limit| *limit > 0)
        .unwrap_or(DEFAULT_TOTAL_CHARS)
}

pub(crate) fn context_file_paths(workspace: &Path) -> Vec<PathBuf> {
    let Ok(raw) = std::env::var("ANGEL_LOOP_CONTEXT_FILES") else {
        return Vec::new();
    };
    raw.split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| {
            let path = PathBuf::from(name);
            if path.is_absolute() {
                path
            } else {
                workspace.join(path)
            }
        })
        .collect()
}

/// Keeps the head and tail of `text` within `budget` characters, marking the
/// cut. Journals grow at the end, so the tail gets the larger share.
pub(crate) fn bounded(text: &str, budget: usize) -> String {
    let count = text.chars().count();
    if count <= budget {
        return text.to_string();
    }
    let keep = budget.saturating_sub(ELISION.chars().count());
    let head = keep / 4;
    let tail = keep - head;
    let head_text: String = text.chars().take(head).collect();
    let tail_text: String = text.chars().skip(count - tail).collect();
    format!("{head_text}{ELISION}{tail_text}")
}

/// The inline block for a fresh iteration's system prompt: each readable
/// context file under its own label, sharing one character budget in the
/// order named. Empty when no files are configured or none can be read.
pub(crate) fn standing_context(workspace: &Path) -> String {
    let paths = context_file_paths(workspace);
    if paths.is_empty() {
        return String::new();
    }
    let mut budget = total_chars();
    let mut block = String::new();
    for path in &paths {
        if budget == 0 {
            break;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let text = text.trim_end();
        if text.is_empty() {
            continue;
        }
        let label = path
            .strip_prefix(workspace)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();
        let header = format!("\n\n[standing context: {label}]\n");
        let share = budget.saturating_sub(header.chars().count());
        if share == 0 {
            break;
        }
        let body = bounded(text, share);
        budget -= header.chars().count() + body.chars().count();
        block.push_str(&header);
        block.push_str(&body);
    }
    block
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/loop_context__tests.rs"]
mod tests;
