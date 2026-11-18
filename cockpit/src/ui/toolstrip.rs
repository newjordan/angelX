//! The tool activity strip — the calm replacement for the old T:/R: trace.
//!
//! While a turn runs, every tool call used to land in the transcript as a
//! full-width wrapped `T: name(args…)` / `R: name: …` pair, burying the
//! agent's actual words under plumbing. The strip keeps that plumbing out of
//! the scrollback entirely: live tool activity renders as two pinned rows at
//! the bottom of the agent shell — one compact status line (current tool,
//! call count, elapsed) and one lateral dotmax loading bar streaming while
//! work is in flight. When the turn ends the whole run collapses into a
//! single tally line in the transcript (`⚒ 14 tools · shell×9 read×3 · 38s`).
//! `/trace` flips the old full trace back on for debugging.

use crate::agent::harness::{ExecutionOutcome, ToolEventId, ToolOutcome, VerificationOutcome};
use dotmax::progress::{BarContext, ProgressStyle, RibbonFlow, SyncLock, draw};
use dotmax::{BrailleGrid, DotmaxError};
use std::cell::{Cell, RefCell};

mod ambient;
mod rolling_text;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

use crate::app::control::realm::GameHost;
use angel_plug::commands::{CommandsWorld, Herald};

const WARP_EVENT_CAP: usize = 32;
const WARP_EVENT_WINDOW: Duration = Duration::from_secs(2);
const WARP_DRIVE_MIN: f32 = 0.35;
const WARP_DRIVE_MAX: f32 = 3.0;
const WARP_RATE_SATURATION: f32 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolState {
    Running,
    Passed,
    NotStarted,
    Failed,
    Inconclusive,
}

pub(crate) struct ToolEntry {
    pub(crate) id: ToolEventId,
    pub(crate) name: String,
    pub(crate) args: String,
    herald: Option<Herald>,
    pub(crate) done: bool,
    pub(crate) err: bool,
    not_started: bool,
    inconclusive: bool,
    verifier: Option<&'static str>,
    result_detail: String,
    execution: ExecutionOutcome,
    err_tail: String,
    verifier_failure: Option<VerifierFailure>,
    started: Instant,
    elapsed_ms: Option<u64>,
    result_digest: Option<String>,
    result_excerpt: Option<String>,
    reason: Option<&'static str>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ToolStripSnapshot {
    pub(crate) calls: usize,
    pub(crate) errors: usize,
    pub(crate) incomplete: usize,
    pub(crate) diagnostics: usize,
    pub(crate) costly_actions: Vec<String>,
    pub(crate) outcome_actions: Vec<String>,
    pub(crate) verified_outcome_actions: Vec<String>,
    pub(crate) verifier_failures: Vec<(String, String)>,
    pub(crate) verifier_failure_details: Vec<VerifierFailure>,
    pub(crate) measurement_results: Vec<MeasurementResult>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MeasurementResult {
    pub(crate) receipt: String,
    pub(crate) excerpt: String,
    pub(crate) elapsed_ms: Option<u64>,
}

const EXCERPT_LINES: usize = 4;
const EXCERPT_LINE_CHARS: usize = 160;

fn result_excerpt(summary: &str) -> String {
    let summary = crate::platform::secrets::redact_str(summary);
    let lines: Vec<String> = summary
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .rev()
        .take(EXCERPT_LINES)
        .map(|line| {
            if line.chars().count() > EXCERPT_LINE_CHARS {
                let mut clipped: String = line.chars().take(EXCERPT_LINE_CHARS - 1).collect();
                clipped.push('…');
                clipped
            } else {
                line.to_string()
            }
        })
        .collect();
    lines.into_iter().rev().collect::<Vec<_>>().join(" | ")
}

pub(crate) struct StatusRow {
    pub(crate) left: String,
    pub(crate) lead: usize,
    pub(crate) padding: usize,
    pub(crate) right: String,
    pub(crate) stall: Option<StallReadout>,
    pub(crate) state: ToolState,
    pub(crate) verifier: bool,
}

#[derive(Default)]
pub(crate) struct ToolStrip {
    entries: Vec<ToolEntry>,
    started: Option<Instant>,
    diagnostics: usize,
    note: Option<String>,
    notes: usize,
    note_prefixes: Vec<String>,
    event_times: VecDeque<Instant>,
    warp_clock: Cell<f32>,
    note_roll: RefCell<rolling_text::RollingText>,
    reading_paused: bool,
    warp_last_tick: Cell<Option<Instant>>,
    ambience: RefCell<ambient::Ambient>,
    ledger_open: bool,
}

impl ToolStrip {
    pub(crate) fn begin_turn(&mut self) {
        self.entries.clear();
        self.started = None;
        self.diagnostics = 0;
        self.note = None;
        self.note_roll = RefCell::default();
        self.reading_paused = false;
        self.notes = 0;
        self.note_prefixes.clear();
        self.event_times.clear();
        self.warp_clock.set(0.0);
        self.warp_last_tick.set(None);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.note.is_none()
    }

    pub(crate) fn note_event(&mut self, note: &str) {
        if self.started.is_none() {
            self.started = Some(Instant::now());
        }
        let sanitized = sanitize(note);
        let prefix = crate::ui::views::turn_event_view::notice_strip_prefix(&sanitized);
        if !prefix.is_empty() && !self.note_prefixes.iter().any(|seen| seen == prefix) {
            self.note_prefixes.push(prefix.to_string());
        }
        self.notes = self.notes.saturating_add(1);
        self.note_roll = RefCell::new(rolling_text::RollingText::new(note_row_text(
            &sanitized,
            self.notes,
            &self.note_prefixes,
        )));
        self.note = Some(sanitized);
        self.record_warp_event();
    }

    pub(crate) fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    pub(crate) fn pause_for_reading(&mut self, paused: bool) {
        self.reading_paused = paused;
    }

    pub(crate) fn reading_paused(&self) -> bool {
        self.reading_paused
    }

    pub(crate) fn ambient_row(
        &self,
        width: usize,
        now: Instant,
        motion: crate::ui::viz::lifecycle_viz::MotionMode,
        paused: bool,
    ) -> std::sync::Arc<str> {
        self.ambience.borrow_mut().row(width, now, motion, paused)
    }

    pub(crate) fn bar_is_quiet(&self, stalled: bool) -> bool {
        matches!(
            motion_cue_with_silence(self, stalled),
            MotionCue::Settled(_) | MotionCue::Stalled
        )
    }

    pub(crate) fn rolling_note(
        &self,
        width: usize,
        now: Instant,
        enabled: bool,
        paused: bool,
    ) -> String {
        self.note_roll
            .borrow_mut()
            .render(width, now, enabled, paused)
    }

    #[cfg(test)]
    pub(crate) fn note_count(&self) -> usize {
        self.notes
    }

    #[cfg(test)]
    pub(crate) fn note_prefixes(&self) -> &[String] {
        &self.note_prefixes
    }

    pub(crate) fn count(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn has_running_calls(&self) -> bool {
        self.entries.iter().any(|entry| !entry.done)
    }

    pub(crate) fn oldest_running(&self) -> Option<(&str, &str, Duration)> {
        self.entries
            .iter()
            .filter(|entry| !entry.done)
            .min_by_key(|entry| entry.started)
            .map(|entry| {
                (
                    entry.name.as_str(),
                    entry.args.as_str(),
                    entry.started.elapsed(),
                )
            })
    }

    pub(crate) fn is_waiting_on_agents(&self) -> bool {
        self.current()
            .is_some_and(|entry| !entry.done && is_agent_wait_tool(&entry.name))
    }

    pub(crate) fn call_event(&mut self, id: ToolEventId, name: &str, args_summary: &str) {
        if self.entries.iter().any(|entry| entry.id == id) {
            self.diagnostics = self.diagnostics.saturating_add(1);
            return;
        }
        if self.started.is_none() {
            self.started = Some(Instant::now());
        }
        let args = sanitize(args_summary);
        self.entries.push(ToolEntry {
            id,
            herald: <GameHost as CommandsWorld>::herald(name, &args),
            name: name.to_string(),
            verifier: verifier_label(name, &args),
            args,
            done: false,
            err: false,
            not_started: false,
            inconclusive: false,
            result_detail: String::new(),
            execution: ExecutionOutcome::NotStarted,
            err_tail: String::new(),
            verifier_failure: None,
            started: Instant::now(),
            elapsed_ms: None,
            result_digest: None,
            result_excerpt: None,
            reason: None,
        });
    }

    pub(crate) fn result_event(
        &mut self,
        id: &ToolEventId,
        _name: &str,
        summary: &str,
        outcome: ToolOutcome,
    ) {
        let Some(e) = self.entries.iter_mut().find(|entry| &entry.id == id) else {
            self.diagnostics = self.diagnostics.saturating_add(1);
            return;
        };
        if e.done {
            self.diagnostics = self.diagnostics.saturating_add(1);
            return;
        }
        e.done = true;
        e.elapsed_ms = Some(e.started.elapsed().as_millis() as u64);
        e.not_started = outcome.execution == ExecutionOutcome::NotStarted;
        e.execution = outcome.execution;
        e.err = outcome.execution != ExecutionOutcome::Succeeded
            || outcome.verification == VerificationOutcome::Failed;
        e.inconclusive = outcome.verification == VerificationOutcome::Inconclusive;
        e.result_digest =
            Some(crate::knowledge::cut::sha256_hex(summary.as_bytes())[..16].to_string());
        let measurement = measured_submission_fingerprint(&e.name, &e.args).is_some();
        let reported_failure = measurement && echoed_exit_failed(&e.args, summary);
        e.err |= reported_failure;
        if measurement {
            e.result_excerpt = Some(result_excerpt(summary));
        }
        if e.verifier.is_some() {
            e.result_detail = verifier_result_detail(summary);
        }
        if e.execution != ExecutionOutcome::Succeeded {
            e.reason = crate::agent::harness::failure_reason(summary);
        }
        if measurement
            && (reported_failure
                || matches!(
                    outcome.execution,
                    ExecutionOutcome::Failed | ExecutionOutcome::Panicked
                ))
        {
            let failure = VerifierFailure::new(&e.args, summary);
            e.err_tail = failure.diagnostic();
            e.verifier_failure = Some(failure);
        }
        self.record_warp_event();
    }

    fn record_warp_event(&mut self) {
        let now = Instant::now();
        self.event_times.push_back(now);
        while self.event_times.len() > WARP_EVENT_CAP {
            self.event_times.pop_front();
        }
        while self
            .event_times
            .front()
            .is_some_and(|t| now.duration_since(*t) > WARP_EVENT_WINDOW)
        {
            self.event_times.pop_front();
        }
    }

    pub(crate) fn warp_drive(&self, reduced: bool) -> f32 {
        if reduced {
            return 1.0;
        }
        let now = Instant::now();
        let n = self
            .event_times
            .iter()
            .filter(|t| now.duration_since(**t) <= WARP_EVENT_WINDOW)
            .count();
        let rate = n as f32 / WARP_EVENT_WINDOW.as_secs_f32();
        let t = (rate / WARP_RATE_SATURATION).clamp(0.0, 1.0);
        let t = t * t * (3.0 - 2.0 * t);
        WARP_DRIVE_MIN + t * (WARP_DRIVE_MAX - WARP_DRIVE_MIN)
    }

    #[cfg(test)]
    pub(crate) fn warp_time(&self, reduced: bool) -> f32 {
        self.warp_time_at(Instant::now(), reduced, false)
    }

    pub(crate) fn warp_time_at(&self, now: Instant, reduced: bool, paused: bool) -> f32 {
        let dt = self
            .warp_last_tick
            .get()
            .map(|t| now.duration_since(t).as_secs_f32())
            .unwrap_or(0.0)
            .min(0.1);
        self.warp_last_tick.set(Some(now));
        let next = self.warp_clock.get()
            + if paused {
                0.0
            } else {
                dt * self.warp_drive(reduced)
            };
        self.warp_clock.set(next);
        next
    }

    #[cfg(test)]
    fn call(&mut self, name: &str, args_summary: &str) {
        self.call_event(
            ToolEventId(format!("test-call-{}", self.entries.len() + 1)),
            name,
            args_summary,
        );
    }

    #[cfg(test)]
    fn result(&mut self, name: &str, summary: &str) {
        let Some(id) = self
            .entries
            .iter()
            .find(|entry| !entry.done && entry.name == name)
            .map(|entry| entry.id.clone())
        else {
            self.diagnostics = self.diagnostics.saturating_add(1);
            return;
        };
        let failed = test_result_is_error(summary);
        let verification = if verifier_label(name, "").is_some() {
            if failed {
                VerificationOutcome::Failed
            } else {
                VerificationOutcome::Passed
            }
        } else {
            VerificationOutcome::NotApplicable
        };
        self.result_event(
            &id,
            name,
            summary,
            ToolOutcome {
                execution: if summary.trim_start().starts_with("tool error") {
                    ExecutionOutcome::Failed
                } else {
                    ExecutionOutcome::Succeeded
                },
                verification,
            },
        );
    }

    pub(crate) fn current(&self) -> Option<&ToolEntry> {
        self.entries
            .iter()
            .rev()
            .find(|e| !e.done)
            .or_else(|| self.entries.last())
    }

    pub(crate) fn current_herald(&self) -> Option<&Herald> {
        self.current()
            .filter(|entry| !entry.done)
            .and_then(|entry| entry.herald.as_ref())
    }

    pub(crate) fn toggle_ledger(&mut self) {
        self.ledger_open = !self.ledger_open;
    }

    #[cfg(test)]
    pub(crate) fn ledger_open(&self) -> bool {
        self.ledger_open
    }

    pub(crate) fn ledger_height(&self) -> usize {
        if self.ledger_open {
            self.entries.len().min(LEDGER_ROWS)
        } else {
            0
        }
    }

    pub(crate) fn ledger_rows(&self, width: usize, rows: usize) -> Vec<LedgerRow> {
        let rows = rows.min(self.ledger_height());
        if rows == 0 || width == 0 {
            return Vec::new();
        }
        let total = self.entries.len();
        let folded = if total > rows && rows > 1 {
            total - (rows - 1)
        } else {
            total.saturating_sub(rows)
        };
        let mut out = Vec::with_capacity(rows);
        if folded > 0 && rows > 1 {
            out.push(LedgerRow {
                branch: "├ ",
                state: None,
                mark: "",
                call: format!("⋯ {folded} earlier"),
                args: String::new(),
                padding: 0,
                note: String::new(),
                age: String::new(),
            });
        }
        let shown = &self.entries[total - (rows - out.len())..];
        let name_col = shown
            .iter()
            .map(|entry| UnicodeWidthStr::width(entry.name.as_str()))
            .max()
            .unwrap_or(0)
            .min(LEDGER_NAME_COLS);
        for (index, entry) in shown.iter().enumerate() {
            let state = entry_state(entry);
            let age = ledger_age(
                entry
                    .elapsed_ms
                    .unwrap_or_else(|| entry.started.elapsed().as_millis() as u64),
            );
            let note = failure_note(entry)
                .map(|note| format!("{note} "))
                .unwrap_or_default();
            let room = width.saturating_sub(
                4 + 1
                    + UnicodeWidthStr::width(note.as_str())
                    + UnicodeWidthStr::width(age.as_str()),
            );
            let call = ellipsize(&entry.name, room.min(name_col));
            let gap = name_col.saturating_sub(UnicodeWidthStr::width(call.as_str())) + 2;
            let args_room = room.saturating_sub(UnicodeWidthStr::width(call.as_str()) + gap);
            let args = if entry.args.is_empty() || args_room < 2 {
                String::new()
            } else {
                format!("{}{}", " ".repeat(gap), ellipsize(&entry.args, args_room))
            };
            let used = 4
                + UnicodeWidthStr::width(call.as_str())
                + UnicodeWidthStr::width(args.as_str())
                + UnicodeWidthStr::width(note.as_str())
                + UnicodeWidthStr::width(age.as_str());
            out.push(LedgerRow {
                branch: if index + 1 == shown.len() {
                    "└ "
                } else {
                    "├ "
                },
                state: Some(state),
                mark: state_mark(state),
                call,
                args,
                padding: width.saturating_sub(used),
                note,
                age,
            });
        }
        out
    }

    pub(crate) fn elapsed_secs(&self) -> u64 {
        self.started.map(|s| s.elapsed().as_secs()).unwrap_or(0)
    }

    pub(crate) fn snapshot(&self) -> ToolStripSnapshot {
        ToolStripSnapshot {
            calls: self.entries.len(),
            errors: self
                .entries
                .iter()
                .filter(|e| e.err && !e.not_started)
                .count(),
            incomplete: self.entries.iter().filter(|e| !e.done).count(),
            diagnostics: self.diagnostics,
            costly_actions: self
                .entries
                .iter()
                .filter_map(|e| costly_action_fingerprint(&e.name, &e.args))
                .collect(),
            outcome_actions: self
                .entries
                .iter()
                .filter(|entry| {
                    entry.done && !entry.err && !starts_background_action(&entry.name, &entry.args)
                })
                .filter_map(|entry| {
                    let action = outcome_action_fingerprint(&entry.name, &entry.args)?;
                    let digest = entry.result_digest.as_deref().unwrap_or("no-result");
                    Some(format!("{action}:result={digest}"))
                })
                .collect(),
            verified_outcome_actions: self
                .entries
                .iter()
                .filter(|entry| {
                    entry.done
                        && !entry.err
                        && entry.execution == ExecutionOutcome::Succeeded
                        && entry.verifier_failure.is_none()
                })
                .filter_map(|entry| {
                    let (action, submission) =
                        measured_submission_fingerprint(&entry.name, &entry.args)?;
                    let kind = if submission { "submitted" } else { "measured" };
                    let digest = entry.result_digest.as_deref().unwrap_or("no-result");
                    Some(format!("{kind}:{action}:result={digest}"))
                })
                .collect(),
            verifier_failure_details: self
                .entries
                .iter()
                .filter_map(|e| e.verifier_failure.clone())
                .collect(),
            measurement_results: self
                .entries
                .iter()
                .filter(|entry| {
                    entry.done
                        && !entry.err
                        && entry.execution == ExecutionOutcome::Succeeded
                        && entry.verifier_failure.is_none()
                })
                .filter_map(|entry| {
                    let (action, submission) =
                        measured_submission_fingerprint(&entry.name, &entry.args)?;
                    let kind = if submission { "submitted" } else { "measured" };
                    let digest = entry.result_digest.as_deref().unwrap_or("no-result");
                    Some(MeasurementResult {
                        receipt: format!("{kind}:{action}:result={digest}"),
                        excerpt: entry.result_excerpt.clone()?,
                        elapsed_ms: entry.elapsed_ms,
                    })
                })
                .collect(),
            verifier_failures: self
                .entries
                .iter()
                .filter(|entry| entry.done && entry.verifier_failure.is_some())
                .filter_map(|entry| {
                    let (action, _) = measured_submission_fingerprint(&entry.name, &entry.args)?;
                    Some((action, entry.err_tail.clone()))
                })
                .collect(),
        }
    }

    pub(crate) fn take_summary(&mut self) -> Option<String> {
        if self.entries.is_empty() {
            let notes = self.notes;
            let tally = (notes > 0).then(|| {
                format!(
                    "{} {} (/trace for detail)",
                    crate::ui::glyphs::Glyph::Tool.token(),
                    note_tally_text(notes, &self.note_prefixes),
                )
            });
            self.begin_turn();
            return tally;
        }
        let total = self.entries.len();
        let errs = self
            .entries
            .iter()
            .filter(|e| e.err && !e.not_started)
            .count();
        let not_started = self.entries.iter().filter(|e| e.not_started).count();
        let mut names: Vec<(&str, usize)> = Vec::new();
        for e in &self.entries {
            match names.iter_mut().find(|(n, _)| *n == e.name) {
                Some((_, c)) => *c += 1,
                None => names.push((&e.name, 1)),
            }
        }
        names.sort_by_key(|&(_, c)| std::cmp::Reverse(c));
        let shown = names
            .iter()
            .take(4)
            .map(|(n, c)| {
                if *c > 1 {
                    format!("{n}\u{00d7}{c}")
                } else {
                    (*n).to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        let more = names.len().saturating_sub(4);
        let mut out = format!(
            "{} {total} tool{} · {shown}",
            crate::ui::glyphs::Glyph::Tool.token(),
            if total == 1 { "" } else { "s" },
        );
        if more > 0 {
            out.push_str(&format!(" · +{more}"));
        }
        if errs > 0 {
            out.push_str(&format!(" · {errs} err"));
        }
        if not_started > 0 {
            out.push_str(&format!(" · {not_started} not started"));
        }
        if let Some(verifier) = self
            .entries
            .iter()
            .rev()
            .find(|entry| entry.verifier.is_some())
        {
            out.push_str(if verifier.inconclusive {
                " · verification inconclusive"
            } else if verifier.err {
                " · verification failed"
            } else {
                " · verified"
            });
        }
        let secs = self.elapsed_secs();
        if secs > 0 {
            out.push_str(&format!(" · {}", format_age_secs(secs)));
        }
        let incomplete = self.entries.iter().filter(|entry| !entry.done).count();
        if incomplete > 0 {
            out.push_str(&format!(" · {incomplete} unfinished"));
        }
        if let Some((name, ms, done)) = self
            .entries
            .iter()
            .map(|e| {
                (
                    e.name.as_str(),
                    e.elapsed_ms
                        .unwrap_or_else(|| e.started.elapsed().as_millis() as u64),
                    e.done,
                )
            })
            .max_by_key(|(_, ms, _)| *ms)
            && ms >= 1000
        {
            out.push_str(&format!(" · slowest {name} {}", format_age_secs(ms / 1000)));
            if !done {
                out.push_str(" (unfinished)");
            }
        }
        if self.notes > 0 {
            out.push_str(&format!(
                " · {}",
                note_tally_text(self.notes, &self.note_prefixes)
            ));
        }
        self.begin_turn();
        Some(out)
    }
}

const LEDGER_ROWS: usize = 8;
const LEDGER_NAME_COLS: usize = 16;

pub(crate) struct LedgerRow {
    pub(crate) branch: &'static str,
    pub(crate) state: Option<ToolState>,
    pub(crate) mark: &'static str,
    pub(crate) call: String,
    pub(crate) args: String,
    pub(crate) padding: usize,
    pub(crate) note: String,
    pub(crate) age: String,
}

fn failure_note(entry: &ToolEntry) -> Option<String> {
    if !entry.done || !entry.err {
        return None;
    }
    let ended = match entry.execution {
        ExecutionOutcome::NotStarted => "not started",
        ExecutionOutcome::Denied => "denied",
        ExecutionOutcome::Cancelled => "cancelled",
        ExecutionOutcome::Panicked => "panicked",
        ExecutionOutcome::Succeeded | ExecutionOutcome::Failed => "failed",
    };
    Some(match entry.reason {
        Some(reason) if ended == "failed" || ended == "panicked" => format!("{ended} · {reason}"),
        _ => ended.to_string(),
    })
}

fn entry_state(entry: &ToolEntry) -> ToolState {
    if !entry.done {
        ToolState::Running
    } else if entry.not_started {
        ToolState::NotStarted
    } else if entry.err {
        ToolState::Failed
    } else if entry.inconclusive {
        ToolState::Inconclusive
    } else {
        ToolState::Passed
    }
}

fn state_mark(state: ToolState) -> &'static str {
    match state {
        ToolState::Running => "\u{25b8}",
        ToolState::Passed => "\u{2713}",
        ToolState::NotStarted => "\u{2298}",
        ToolState::Failed => "\u{2717}",
        ToolState::Inconclusive => "?",
    }
}

fn ledger_age(ms: u64) -> String {
    if ms < 1000 {
        format!("{}.{}s", ms / 1000, (ms % 1000) / 100)
    } else {
        format_age_secs(ms / 1000)
    }
}

pub(crate) fn note_row_text(note: &str, count: usize, prefixes: &[String]) -> String {
    if count <= 1 {
        return format!("\u{00b7} {note}");
    }
    let current = crate::ui::views::turn_event_view::notice_strip_prefix(note);
    let prior = prefixes
        .iter()
        .map(String::as_str)
        .filter(|prefix| *prefix != current)
        .collect::<Vec<_>>();
    if prior.is_empty() {
        format!("\u{00b7} {note}  ({count} notes)")
    } else {
        format!("\u{00b7} {} · {note}  ({count} notes)", prior.join(", "))
    }
}

fn note_tally_text(notes: usize, prefixes: &[String]) -> String {
    let label = if notes == 1 { "note" } else { "notes" };
    if prefixes.is_empty() {
        format!("{notes} {label}")
    } else {
        format!("{notes} {label} ({})", prefixes.join(", "))
    }
}

fn format_age_secs(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

fn tool_leaf(name: &str) -> &str {
    name.rsplit(['.', ':', '/'])
        .find(|part| !part.is_empty())
        .unwrap_or(name)
}

fn is_agent_wait_tool(name: &str) -> bool {
    matches!(
        tool_leaf(name).to_ascii_lowercase().as_str(),
        "spawn" | "delegate" | "wait" | "wait_agent" | "wait-agent" | "join_agents"
    )
}

fn verifier_label(name: &str, args: &str) -> Option<&'static str> {
    let leaf = tool_leaf(name).to_ascii_lowercase();
    match leaf.as_str() {
        "run_tests" => Some("TEST"),
        "check" => Some("CHECK"),
        "lint" | "clippy" => Some("LINT"),
        "fmt" => Some("FORMAT"),
        "cargo" => {
            let command = args.trim_start().to_ascii_lowercase();
            if command.starts_with("test") {
                Some("TEST")
            } else if command.starts_with("check") || command.starts_with("build") {
                Some("CHECK")
            } else if command.starts_with("clippy") {
                Some("LINT")
            } else if command.starts_with("fmt") {
                Some("FORMAT")
            } else {
                Some("CARGO")
            }
        }
        _ => None,
    }
}

fn metric_pairs(summary: &str) -> Vec<(u64, String)> {
    let is_metric = |word: &str| {
        matches!(
            word,
            "passed" | "failed" | "failures" | "errors" | "error" | "warnings" | "warning"
        )
    };
    let mut pairs = Vec::new();
    for clause in summary.split([',', ';', '\n']) {
        let words: Vec<String> = clause
            .to_ascii_lowercase()
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(str::to_string)
            .collect();
        for (index, word) in words.iter().enumerate() {
            if !is_metric(word) {
                continue;
            }
            let value = index
                .checked_sub(1)
                .and_then(|before| words[before].parse::<u64>().ok())
                .or_else(|| {
                    words
                        .get(index + 1)
                        .and_then(|after| after.parse::<u64>().ok())
                });
            if let Some(n) = value {
                pairs.push((n, word.clone()));
            }
        }
    }
    pairs
}

#[cfg(test)]
fn test_result_is_error(summary: &str) -> bool {
    let lower = summary.trim_start().to_ascii_lowercase();
    lower.starts_with("tool error")
        || lower.starts_with("error")
        || lower.starts_with("failed")
        || metric_pairs(summary).iter().any(|(n, metric)| {
            *n > 0 && matches!(metric.as_str(), "failed" | "failures" | "errors" | "error")
        })
}

fn verifier_result_detail(summary: &str) -> String {
    let metrics = metric_pairs(summary);
    let mut detail = Vec::new();
    for (n, metric) in metrics {
        let label = match metric.as_str() {
            "failures" => "failed".to_string(),
            "error" => "errors".to_string(),
            "warning" => "warnings".to_string(),
            _ => metric,
        };
        if !detail
            .iter()
            .any(|(_, seen): &(u64, String)| *seen == label)
        {
            detail.push((n, label));
        }
    }
    if !detail.is_empty() {
        return detail
            .into_iter()
            .take(3)
            .map(|(n, label)| format!("{n} {label}"))
            .collect::<Vec<_>>()
            .join(" · ");
    }

    let lower = summary.to_ascii_lowercase();
    for (needle, label) in [
        ("rustfmt-clean", "rustfmt clean"),
        ("fmt clean", "format clean"),
        ("up to date", "up to date"),
        ("finished", "finished"),
        ("ok", "complete"),
    ] {
        if lower.contains(needle) {
            return label.to_string();
        }
    }
    ellipsize(
        summary
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("complete"),
        48,
    )
}

fn canonical_command_key(args_key: &str) -> String {
    let base = args_key
        .split("2>&1")
        .next()
        .unwrap_or(args_key)
        .split(" | ")
        .next()
        .unwrap_or(args_key);
    let mut words = Vec::new();
    let mut skip_output = false;
    for word in base.split_whitespace() {
        if skip_output {
            skip_output = false;
            continue;
        }
        if matches!(word, "-o" | "--output" | "--output-file") {
            skip_output = true;
            continue;
        }
        words.push(word);
    }
    words.join(" ")
}

fn costly_action_fingerprint(name: &str, args: &str) -> Option<String> {
    let name_key = name.trim().to_ascii_lowercase();
    let args_key = args.to_ascii_lowercase();
    let named_mutation = ["submit", "deploy", "publish", "upload", "send"]
        .iter()
        .any(|needle| name_key.contains(needle));
    let costly_shell = name_key == "shell"
        && [
            " benchmark",
            "benchmark ",
            " submit",
            "submit ",
            " leaderboard",
            "leaderboard ",
            " deploy",
            "deploy ",
            " publish",
            "publish ",
            " upload",
            "upload ",
            "cargo test",
            "pytest",
            "npm test",
            "pnpm test",
        ]
        .iter()
        .any(|needle| args_key.contains(needle));
    if !named_mutation && !costly_shell {
        return None;
    }

    let joined = canonical_command_key(&args_key);
    if joined.contains("popcorn") {
        let mode = if joined.contains("--mode test") || joined.contains("mode test") {
            "test"
        } else if joined.contains("--mode leaderboard") || joined.contains("mode leaderboard") {
            "leaderboard"
        } else {
            "benchmark"
        };
        return Some(format!("{name_key}:popcorn submit --mode {mode}"));
    }
    Some(format!("{name_key}:{joined}"))
}

fn outcome_action_fingerprint(name: &str, args: &str) -> Option<String> {
    let name_key = name.trim().to_ascii_lowercase();
    let args_key = args.to_ascii_lowercase();
    if ["apply_patch", "write_file", "edit_file", "search_replace"]
        .iter()
        .any(|needle| name_key.contains(needle))
    {
        return Some(format!("mutation:{name_key}"));
    }
    if let Some(costly) = costly_action_fingerprint(name, args) {
        return Some(costly);
    }
    let outcome_query = [
        " submissions",
        "submissions ",
        " status",
        "status ",
        " score",
        "score ",
        " validate",
        "validate ",
        " verify",
        "verify ",
    ]
    .iter()
    .any(|needle| args_key.contains(needle));
    let named_outcome = [
        "submit",
        "status",
        "score",
        "validate",
        "verify",
        "benchmark",
    ]
    .iter()
    .any(|needle| name_key.contains(needle));
    (named_outcome || (name_key == "shell" && outcome_query))
        .then(|| format!("outcome:{name_key}:{}", args_key.trim()))
}

const MEASUREMENT_WORDS: [&str; 7] = [
    "benchmark",
    "bench",
    "measure",
    "verify",
    "validate",
    "baseline",
    "score",
];

const READ_ONLY_PROGRAMS: [&str; 24] = [
    "cat", "head", "tail", "less", "more", "grep", "rg", "ls", "echo", "printf", "jq", "find",
    "stat", "wc", "diff", "sed", "awk", "cp", "mv", "ln", "file", "tree", "bat", "git",
];

const RUNNERS: [&str; 25] = [
    "bash", "sh", "zsh", "python", "python3", "uv", "cargo", "npm", "npx", "node", "bun",
    "timeout", "nice", "env", "sudo", "time", "make", "just", "poetry", "pnpm", "yarn", "exec",
    "nohup", "setsid", "stdbuf",
];

const MEASUREMENT_NAME_WORDS: [&str; 3] = ["ab", "abab", "abba"];

const SSH_VALUE_FLAGS: &str = "BbcDEeFIiJLlmOoPpQRSWw";

const CARRY_DEPTH: usize = 4;

fn basename(token: &str) -> &str {
    token.rsplit('/').next().unwrap_or(token)
}

fn is_env_assignment(token: &str) -> bool {
    !token.starts_with('-')
        && token.split_once('=').is_some_and(|(key, _)| {
            !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

fn command_segments(args_key: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut list_start = 0;
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut heredoc: Option<String> = None;
    let mut heredoc_line = String::new();
    let mut chars = args_key.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(tag) = heredoc.as_deref() {
            if c == '\n' {
                if heredoc_line.trim() == tag {
                    heredoc = None;
                }
                heredoc_line.clear();
            } else {
                heredoc_line.push(c);
            }
            continue;
        }
        if let Some(q) = quote {
            current.push(c);
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '\'' | '"' => {
                quote = Some(c);
                current.push(c);
            }
            '\\' => {
                current.push(c);
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            '<' if chars.peek() == Some(&'<') => {
                chars.next();
                let mut tag = String::new();
                while let Some(&n) = chars.peek() {
                    if n == '-' || n == '\'' || n == '"' || n == ' ' {
                        chars.next();
                        if !tag.is_empty() {
                            break;
                        }
                        continue;
                    }
                    if n.is_ascii_alphanumeric() || n == '_' {
                        tag.push(n);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let mut rest = String::new();
                for n in chars.by_ref() {
                    if n == '\n' {
                        break;
                    }
                    rest.push(n);
                }
                current.push_str(&rest);
                if !tag.is_empty() {
                    heredoc = Some(tag);
                    heredoc_line.clear();
                }
            }
            ';' | '\n' => {
                segments.push(std::mem::take(&mut current));
                list_start = segments.len();
            }
            '|' => {
                if chars.peek() == Some(&'|') {
                    chars.next();
                    segments.push(std::mem::take(&mut current));
                } else {
                    segments.push(std::mem::take(&mut current));
                }
            }
            '&' if chars.peek() == Some(&'&') => {
                chars.next();
                segments.push(std::mem::take(&mut current));
            }
            '&' if !current.ends_with('>') && chars.peek() != Some(&'>') => {
                for segment in &mut segments[list_start..] {
                    segment.push('&');
                }
                current.push('&');
                segments.push(std::mem::take(&mut current));
                list_start = segments.len();
            }
            _ => current.push(c),
        }
    }
    segments.push(current);
    segments
}

fn looks_like_program(token: &str) -> bool {
    !token.is_empty()
        && token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '/' | '-' | '_' | '~' | '+'))
}

fn segment_program(segment: &str) -> Option<(String, Option<String>)> {
    let words = shell_words(segment);
    let mut tokens = words
        .iter()
        .map(String::as_str)
        .filter(|token| !is_env_assignment(token));
    let mut program = tokens.next()?;
    if !looks_like_program(program) {
        return None;
    }
    for _ in 0..4 {
        if !RUNNERS.contains(&basename(program)) {
            break;
        }
        let shell = matches!(basename(program), "bash" | "sh" | "zsh");
        let next = tokens.by_ref().find(|token| {
            !(token.starts_with('-') || token.chars().all(|c| c.is_ascii_digit()))
                || (shell && shell_noexec_flag(token))
        });
        match next {
            Some(token) if shell && (shell_noexec_flag(token) || token == "noexec") => {
                return None;
            }
            Some(token) => program = token,
            None => break,
        }
    }
    let operand = tokens.next().map(str::to_string);
    Some((basename(program).to_ascii_lowercase(), operand))
}

fn shell_noexec_flag(word: &str) -> bool {
    word == "--noexec" || (word.starts_with('-') && !word.starts_with("--") && word.contains('n'))
}

fn segment_detaches(segment: &str) -> bool {
    let mut quote = None;
    let mut chars = segment.chars().peekable();
    let mut previous = None;
    while let Some(c) = chars.next() {
        if c == '\\' && quote != Some('\'') {
            chars.next();
        } else if quote == Some(c) {
            quote = None;
        } else if quote.is_none() && matches!(c, '\'' | '"') {
            quote = Some(c);
        } else if quote.is_none() && c == '&' && previous != Some('>') && chars.peek() != Some(&'>')
        {
            return true;
        }
        previous = Some(c);
    }
    let words = shell_words(segment);
    let mut words = words
        .iter()
        .map(String::as_str)
        .filter(|word| !is_env_assignment(word));
    let Some(mut program) = words.next() else {
        return false;
    };
    for _ in 0..4 {
        let program_name = basename(program);
        if program_name == "ssh" {
            while let Some(flag) = words.next() {
                if !flag.starts_with('-') {
                    break;
                }
                let flags = &flag[1..];
                for (index, option) in flags.char_indices() {
                    if option == 'f' {
                        return true;
                    }
                    if SSH_VALUE_FLAGS.contains(option) {
                        if index + option.len_utf8() == flags.len() {
                            words.next();
                        }
                        break;
                    }
                }
            }
            return false;
        }
        if !RUNNERS.contains(&program_name) {
            return false;
        }
        let mut waited = false;
        let next = words.by_ref().find(|word| {
            waited |= *word == "--wait"
                || (word.starts_with('-') && !word.starts_with("--") && word.contains('w'));
            !(word.starts_with('-') || word.chars().all(|c| c.is_ascii_digit()))
        });
        if program_name == "setsid" && !waited {
            return true;
        }
        match next {
            Some(next) => program = next,
            None => return false,
        }
    }
    false
}

fn starts_background_action(name: &str, args: &str) -> bool {
    fn line_detaches(line: &str, depth: usize) -> bool {
        command_segments(line).iter().any(|segment| {
            segment_detaches(segment)
                || (depth > 0
                    && carried_line(segment).is_some_and(|line| line_detaches(&line, depth - 1)))
        })
    }
    let name = name.trim().to_ascii_lowercase();
    name.contains("proc_run") || (runs_commands(&name) && line_detaches(args, CARRY_DEPTH))
}

fn shell_words(segment: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    let mut chars = segment.chars();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) if c == q => quote = None,
            Some('"') if c == '\\' => word.extend(chars.next()),
            Some(_) => word.push(c),
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                in_word = true;
            }
            None if c == '\\' => {
                word.extend(chars.next());
                in_word = true;
            }
            None if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            None => {
                word.push(c);
                in_word = true;
            }
        }
    }
    if in_word {
        words.push(word);
    }
    words
}

fn carried_line(segment: &str) -> Option<String> {
    let words = shell_words(segment);
    let mut words = words
        .iter()
        .map(String::as_str)
        .filter(|word| !is_env_assignment(word));
    match basename(words.next()?) {
        "ssh" => {
            while let Some(word) = words.next() {
                if let Some(flag) = word.strip_prefix('-') {
                    if flag.len() == 1 && SSH_VALUE_FLAGS.contains(flag) {
                        words.next();
                    }
                    continue;
                }
                let rest: Vec<&str> = words.collect();
                return (!rest.is_empty()).then(|| rest.join(" "));
            }
            None
        }
        "bash" | "sh" | "zsh" => {
            while let Some(word) = words.next() {
                match word {
                    word if shell_noexec_flag(word) => return None,
                    "-o" | "+o" => {
                        if words.next() == Some("noexec") && word == "-o" {
                            return None;
                        }
                    }
                    _ if word.starts_with("--") => {}
                    _ if word.starts_with('-') && word.contains('c') => {
                        return words.next().map(str::to_string);
                    }
                    _ if word.starts_with('-') => {}
                    _ => return None,
                }
            }
            None
        }
        "flock" => {
            while let Some(word) = words.next() {
                if word.starts_with('-') {
                    if matches!(
                        word,
                        "-w" | "--timeout" | "--wait" | "-e" | "--conflict-exit-code"
                    ) {
                        words.next();
                    }
                    continue;
                }
                let rest: Vec<&str> = words.collect();
                return match rest.split_first() {
                    Some((&("-c" | "--command"), tail)) => {
                        tail.first().map(|line| line.to_string())
                    }
                    _ => (!rest.is_empty()).then(|| rest.join(" ")),
                };
            }
            None
        }
        _ => None,
    }
}

fn qualifying_segment(
    line: &str,
    depth: usize,
    qualifies: &dyn Fn(&str) -> bool,
) -> Option<String> {
    command_segments(line).into_iter().find_map(|segment| {
        if segment_detaches(&segment) {
            return None;
        }
        match carried_line(&segment) {
            Some(carried) if depth > 0 => qualifying_segment(&carried, depth - 1, qualifies),
            _ => qualifies(&segment).then_some(segment),
        }
    })
}

fn shell_measurement_segment(args_key: &str) -> Option<String> {
    qualifying_segment(args_key, CARRY_DEPTH, &|segment| {
        if shell_words(segment).iter().any(|token| {
            matches!(
                token.as_str(),
                "--help" | "-h" | "help" | "--dry-run" | "--preflight-only"
            ) || token.split_once('=').is_some_and(|(key, value)| {
                (key.eq_ignore_ascii_case("preflight_only")
                    || key.to_ascii_lowercase().ends_with("_preflight_only")
                    || key == "--preflight-only")
                    && matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes")
            })
        }) {
            return false;
        }
        segment_program(segment).is_some_and(|(program, operand)| {
            if READ_ONLY_PROGRAMS.contains(&program.as_str()) {
                return false;
            }
            MEASUREMENT_WORDS.iter().any(|word| program.contains(word))
                || program
                    .split(|c: char| !c.is_ascii_alphanumeric())
                    .any(|word| MEASUREMENT_NAME_WORDS.contains(&word))
                || (!segment
                    .split_whitespace()
                    .any(|token| matches!(token, "--help" | "-h" | "help"))
                    && crate::agent::harness::cartridges::all()
                        .iter()
                        .any(|cartridge| cartridge.measures(&program, operand.as_deref())))
        })
    })
}

fn echoed_exit_failed(args: &str, summary: &str) -> bool {
    let echoes_exit = qualifying_segment(args, CARRY_DEPTH, &|segment| {
        let words = shell_words(segment);
        words.first().is_some_and(|word| word == "echo")
            && words
                .iter()
                .skip(1)
                .any(|word| word.eq_ignore_ascii_case("exit:$?"))
    })
    .is_some();
    echoes_exit
        && summary.lines().any(|line| {
            line.trim()
                .to_ascii_lowercase()
                .strip_prefix("exit:")
                .and_then(|value| value.trim().parse::<i32>().ok())
                .is_some_and(|status| status != 0)
        })
}

fn shell_submission_segment(args_key: &str) -> Option<String> {
    qualifying_segment(args_key, CARRY_DEPTH, &|segment| {
        if segment
            .split_whitespace()
            .any(|token| matches!(token, "--help" | "-h" | "help" | "--dry-run"))
        {
            return false;
        }
        segment_program(segment).is_some_and(|(program, operand)| {
            !READ_ONLY_PROGRAMS.contains(&program.as_str())
                && (program.contains("submit") || operand.as_deref() == Some("submit"))
        })
    })
}

fn runs_commands(name_key: &str) -> bool {
    matches!(name_key, "shell" | "proc_run" | "bash" | "terminal")
        || name_key.contains("shell")
        || name_key.contains("proc_run")
}

fn measured_submission_fingerprint(name: &str, args: &str) -> Option<(String, bool)> {
    let name_key = name.trim().to_ascii_lowercase();
    let args_key = args.trim();
    if name_key.contains("proc_run") {
        return None;
    }
    if runs_commands(&name_key) {
        let (segment, is_submission) = match (
            shell_submission_segment(args_key),
            shell_measurement_segment(args_key),
        ) {
            (Some(segment), _) => (segment, true),
            (None, Some(segment)) => (segment, false),
            (None, None) => return None,
        };
        return Some((
            format!(
                "{name_key}:{}",
                canonical_command_key(&segment.to_ascii_lowercase())
            ),
            is_submission,
        ));
    }
    let measurement = MEASUREMENT_WORDS.iter().any(|word| name_key.contains(word));
    let submission = name_key
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|token| token == "submit");
    if !submission && !measurement {
        return None;
    }
    Some((
        format!(
            "{name_key}:{}",
            canonical_command_key(&args_key.to_ascii_lowercase())
        ),
        submission,
    ))
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct VerifierFailure {
    pub command: String,
    pub exit_code: String,
    pub tail: String,
    pub failure_digest: String,
}

impl VerifierFailure {
    pub(crate) fn new(command: &str, summary: &str) -> Self {
        let command = crate::platform::secrets::redact_str(command);
        let command: String = command.chars().take(200).collect();
        let summary = crate::platform::secrets::redact_str(summary);
        let exit_code = summary
            .split_once("(exit ")
            .and_then(|(_, rest)| rest.split_once(')'))
            .map(|(code, _)| code.chars().take(20).collect())
            .unwrap_or_else(|| "unknown".to_string());
        let lines: Vec<_> = summary
            .lines()
            .filter(|line| {
                !line.starts_with("tool error: shell command failed")
                    && !line.starts_with("shell command failed")
            })
            .collect();
        let mut stderr: Vec<_> = lines
            .iter()
            .copied()
            .filter(|l| l.starts_with("[stderr] "))
            .rev()
            .take(6)
            .collect();
        stderr.reverse();
        let mut stdout: Vec<_> = lines
            .iter()
            .copied()
            .filter(|l| !l.starts_with("[stderr] "))
            .rev()
            .take(6 - stderr.len())
            .collect();
        stdout.reverse();
        stderr.extend(stdout);
        let tail = stderr.join("\n");
        let tail = if tail.is_empty() {
            "(no output captured)".to_string()
        } else {
            tail
        };
        let mut start = tail.len().saturating_sub(600);
        while !tail.is_char_boundary(start) {
            start += 1;
        }
        let tail = tail[start..].to_string();
        let normalized = tail.split_whitespace().collect::<Vec<_>>().join(" ");
        let failure_digest = crate::knowledge::cut::sha256_hex(
            format!("{command}\0{exit_code}\0{normalized}").as_bytes(),
        );
        Self {
            command,
            exit_code,
            tail,
            failure_digest,
        }
    }

    pub(crate) fn diagnostic(&self) -> String {
        format!(
            "command: {}\nexit code: {}\n{}\nfailure_digest: {}",
            self.command, self.exit_code, self.tail, self.failure_digest
        )
    }
}

pub(crate) fn ellipsize(text: &str, width: usize) -> String {
    ellipsize_with_width(text, width, UnicodeWidthStr::width(text))
}

fn ellipsize_with_width(text: &str, width: usize, total: usize) -> String {
    if total <= width {
        return text.to_string();
    }
    if width == 0 {
        return String::new();
    }
    let ellipsis = "\u{2026}";
    let ellipsis_width = UnicodeWidthStr::width(ellipsis);
    if width <= ellipsis_width {
        return "\u{2026}".to_string();
    }
    let content_width = width - ellipsis_width;
    let mut out = String::new();
    let span = ratatui::text::Span::raw(text);
    let mut used = 0;
    for grapheme in span.styled_graphemes(ratatui::style::Style::new()) {
        let cells = UnicodeWidthStr::width(grapheme.symbol);
        if used + cells > content_width {
            break;
        }
        out.push_str(grapheme.symbol);
        used += cells;
    }
    out.push_str(ellipsis);
    out
}

const DEFAULT_STALL_PULSE_SECS: u64 = 20;

pub(crate) fn configured_stall_pulse_secs() -> u64 {
    #[cfg(not(test))]
    {
        static SECS: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
        *SECS.get_or_init(stall_pulse_secs_from_env)
    }
    #[cfg(test)]
    stall_pulse_secs_from_env()
}

fn stall_pulse_secs_from_env() -> u64 {
    std::env::var("ANGEL_STALL_PULSE_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_STALL_PULSE_SECS)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StallSeverity {
    Warn,
    Watchdog,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StallReadout {
    pub(crate) text: String,
    pub(crate) severity: StallSeverity,
}

pub(crate) fn stall_readout(
    silence_secs: u64,
    pulse_secs: u64,
    idle_timeout_secs: Option<u64>,
) -> Option<StallReadout> {
    if pulse_secs == 0 || silence_secs < pulse_secs {
        return None;
    }
    if let Some(deadline) = idle_timeout_secs
        && silence_secs.saturating_mul(2) >= deadline
    {
        return Some(StallReadout {
            text: format!(
                "\u{00b7} watchdog {}s",
                deadline.saturating_sub(silence_secs)
            ),
            severity: StallSeverity::Watchdog,
        });
    }
    Some(StallReadout {
        text: format!("\u{00b7} silent {silence_secs}s"),
        severity: StallSeverity::Warn,
    })
}

pub(crate) fn effective_stall_readout(
    strip: &ToolStrip,
    silence_secs: u64,
    pulse_secs: u64,
    idle_timeout_secs: Option<u64>,
) -> Option<StallReadout> {
    if strip.has_running_calls() {
        None
    } else {
        stall_readout(silence_secs, pulse_secs, idle_timeout_secs)
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn status_row_parts(strip: &ToolStrip, width: usize) -> Option<StatusRow> {
    status_row_parts_with_silence(strip, width, None)
}

pub(crate) fn child_quiet_secs(child: &crate::agent::harness::ChildSnapshot) -> u64 {
    child
        .cpu_age_secs
        .into_iter()
        .chain(child.output_age_secs)
        .min()
        .unwrap_or(child.elapsed_secs)
}

pub(crate) fn worker_status_row(
    child: &crate::agent::harness::ChildSnapshot,
    herald: Option<&Herald>,
    width: usize,
) -> StatusRow {
    let activity = if child.setting_up {
        "starting".into()
    } else if child.cpu_age_secs.is_some_and(|age| age <= 2) {
        "CPU active".into()
    } else if child.output_age_secs.is_some_and(|age| age <= 2) {
        "output received".into()
    } else {
        format!("quiet {}", format_age_secs(child_quiet_secs(child)))
    };
    let operation = match child.program.as_str() {
        "rustc" => "compiling (rustc)",
        "rust-lld" | "ld" | "ld.lld" | "collect2" => "linking",
        other => other,
    };
    let workers = if child.workers > 1 {
        format!(" · {} workers", child.workers)
    } else {
        String::new()
    };
    let (lead, left) = heralded(herald, format!("{operation} · {activity}{workers}"));
    let output = child
        .output_age_secs
        .map(|age| format!("{} ago", format_age_secs(age)))
        .unwrap_or_else(|| "none".into());
    let right = format!(
        "run {} · output {output}",
        format_age_secs(child.elapsed_secs)
    );
    activity_status_row(left, lead, right, width)
}

pub(crate) fn delegate_quiet_secs(delegate: &crate::agent::harness::DelegateSnapshot) -> u64 {
    delegate.progress_age_secs.unwrap_or(delegate.elapsed_secs)
}

pub(crate) fn delegate_status_row(
    delegate: &crate::agent::harness::DelegateSnapshot,
    herald: Option<&Herald>,
    width: usize,
) -> StatusRow {
    let quiet = delegate_quiet_secs(delegate);
    let detail = if quiet >= 30 {
        let transport = if delegate.event_age_secs.is_some_and(|age| age <= 2) {
            " · connected"
        } else {
            ""
        };
        format!(
            "quiet {}{transport} · last: {}",
            format_age_secs(quiet),
            delegate.phase
        )
    } else {
        format!("{} · {}", delegate.phase, delegate.label)
    };
    let (lead, left) = match herald {
        Some(_) => heralded(herald, detail),
        None => (0, format!("▸ delegate · {detail}")),
    };
    let peers = if delegate.delegates > 1 {
        format!(" · {} agents", delegate.delegates)
    } else {
        String::new()
    };
    let right = format!(
        "#{} · update {} ago · run {}{peers}",
        delegate.calls,
        format_age_secs(quiet),
        format_age_secs(delegate.elapsed_secs)
    );
    activity_status_row(left, lead, right, width)
}

fn heralded(herald: Option<&Herald>, detail: String) -> (usize, String) {
    match herald {
        Some(herald) => (
            herald.lead_len(false),
            format!("{} · {detail}", herald.text(false)),
        ),
        None => (0, format!("▸ {detail}")),
    }
}

fn activity_status_row(left: String, lead: usize, right: String, width: usize) -> StatusRow {
    let rail = UnicodeWidthStr::width(right.as_str());
    let (left, padding, right) = if width > rail + 24 {
        let left = ellipsize(&left, width - rail - 1);
        let padding = width.saturating_sub(UnicodeWidthStr::width(left.as_str()) + rail);
        (left, padding, right)
    } else {
        (
            ellipsize(&format!("{left} · {right}"), width),
            0,
            String::new(),
        )
    };
    StatusRow {
        left,
        lead,
        padding,
        right,
        stall: None,
        state: ToolState::Running,
        verifier: false,
    }
}

#[cfg(test)]
include!("../../../tests/cockpit/app/toolstrip__standalone_tests.rs");

pub(crate) fn status_row_parts_with_silence(
    strip: &ToolStrip,
    width: usize,
    stall: Option<StallReadout>,
) -> Option<StatusRow> {
    let stall_width = stall.as_ref().map_or(0, |readout| {
        UnicodeWidthStr::width(readout.text.as_str()) + 1
    });
    let Some(cur) = strip.current() else {
        let note = strip.note()?;
        let left = format!("\u{25b8} {note}");
        if stall_width == 0 || width <= stall_width + 4 {
            return Some(StatusRow {
                left: ellipsize(&left, width),
                lead: 0,
                padding: 0,
                right: String::new(),
                stall: None,
                state: ToolState::Running,
                verifier: false,
            });
        }
        let left = ellipsize(&left, width - stall_width);
        let padding = width.saturating_sub(UnicodeWidthStr::width(left.as_str()) + stall_width);
        return Some(StatusRow {
            left,
            lead: 0,
            padding,
            right: String::new(),
            stall,
            state: ToolState::Running,
            verifier: false,
        });
    };
    let state = entry_state(cur);
    let settled = state == ToolState::Passed;
    let verifier = cur.verifier.is_some();
    let outcome = if verifier && cur.done && !cur.result_detail.is_empty() {
        format!(" · {}", cur.result_detail)
    } else {
        match state {
            ToolState::NotStarted | ToolState::Failed if !verifier => failure_note(cur)
                .map(|note| format!(" · {note}"))
                .unwrap_or_default(),
            ToolState::NotStarted => " · not started".to_string(),
            ToolState::Inconclusive if !verifier => " · inconclusive".to_string(),
            _ => String::new(),
        }
    };
    let literal;
    let herald = match cur.herald.as_ref() {
        Some(herald) => herald,
        None => {
            literal = Herald::literal(&cur.name, &cur.args);
            &literal
        }
    };
    let left = format!("{}{outcome}", herald.text(settled));
    let lead = herald.lead_len(settled);
    let verdict = if verifier {
        match state {
            ToolState::Running => "RUNNING · ",
            ToolState::Passed => "PASS · ",
            ToolState::NotStarted => "NOT STARTED · ",
            ToolState::Failed => "FAIL · ",
            ToolState::Inconclusive => "INCONCLUSIVE · ",
        }
    } else {
        ""
    };
    let seat_pips = crate::ui::viz::agentviz::current_seat_pips().unwrap_or_default();
    let right = format!(
        "{seat_pips}{verdict}{}",
        format_age_secs(strip.elapsed_secs())
    );
    let rail_width = UnicodeWidthStr::width(right.as_str()) + stall_width;

    let gap = usize::from(width > 0);
    if width <= rail_width + gap + 4 {
        return Some(StatusRow {
            left: ellipsize(&left, width),
            lead,
            padding: 0,
            right: String::new(),
            stall: None,
            state,
            verifier,
        });
    }
    let left_budget = width - rail_width - gap;
    let left = ellipsize(&left, left_budget);
    let left_width = UnicodeWidthStr::width(left.as_str());
    let padding = width.saturating_sub(left_width + rail_width);
    Some(StatusRow {
        left,
        lead,
        padding,
        right,
        stall,
        state,
        verifier,
    })
}

pub(crate) struct ToolPulse;

impl ProgressStyle for ToolPulse {
    fn name(&self) -> &str {
        "sine-wave-pulse"
    }
    fn theme(&self) -> &str {
        "cockpit"
    }
    fn describe(&self) -> &str {
        "Smooth flowing sine wave with traveling pulse for in-flight activity"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let mid = (h as f32 - 1.0) / 2.0;
        let amp = (h as f32 * 0.38).max(1.0);

        for x in 0..w {
            let phase = x as f32 * 0.18 - ctx.time * 4.5;
            let y_val = mid + amp * phase.sin();
            let y_dot = (y_val.round() as i32).clamp(0, h as i32 - 1);
            draw::dot_i(grid, x as i32, y_dot);
        }

        let pulse_pos = (ctx.time * 24.0) % (w as f32 + 16.0) - 8.0;
        for x in 0..w {
            let dx = (x as f32 - pulse_pos).abs();
            if dx <= 4.0 {
                let phase = x as f32 * 0.18 - ctx.time * 4.5;
                let y_val = mid + amp * phase.sin();
                let y_dot_base = (y_val.round() as i32).clamp(0, h as i32 - 1);
                let dy = if phase.cos() > 0.0 { 1 } else { -1 };
                let y_dot_sec = (y_dot_base + dy).clamp(0, h as i32 - 1);
                draw::dot_i(grid, x as i32, y_dot_sec);
            }
        }
        Ok(())
    }
}

pub(crate) struct StallFlatline;

impl ProgressStyle for StallFlatline {
    fn name(&self) -> &str {
        "stall-flatline"
    }
    fn theme(&self) -> &str {
        "cockpit"
    }
    fn describe(&self) -> &str {
        "Static dotted midline while the provider stream is silent"
    }
    fn render(&self, grid: &mut BrailleGrid, _ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let mid = (h as i32 - 1) / 2;
        for x in (0..w).step_by(2) {
            draw::dot_i(grid, x as i32, mid);
        }
        Ok(())
    }
}

pub(crate) struct AgentDispatch;

impl ProgressStyle for AgentDispatch {
    fn name(&self) -> &str {
        "agent-dispatch"
    }
    fn theme(&self) -> &str {
        "cockpit"
    }
    fn describe(&self) -> &str {
        "Staggered council packets travel out to agents and return with replies"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let center = (w.saturating_sub(1)) as f32 / 2.0;
        let reach = center.max(1.0);
        let mid = (h.saturating_sub(1)) as i32 / 2;

        draw::dot_i(grid, center.round() as i32, mid);
        if (ctx.time * 2.0).sin() >= 0.0 {
            draw::dot_i(grid, center.round() as i32, (mid - 1).max(0));
            draw::dot_i(grid, center.round() as i32, (mid + 1).min(h as i32 - 1));
        }

        for seat in 0..3 {
            let phase = (ctx.time * 0.42 + seat as f32 / 3.0).fract();
            let excursion = 1.0 - (phase * 2.0 - 1.0).abs();
            let lane_y = ((seat * 2 + 1) % h.max(1)) as i32;
            let distance = excursion * reach;
            for direction in [-1.0_f32, 1.0] {
                let x = (center + direction * distance).round() as i32;
                draw::dot_i(grid, x, lane_y);
                if phase < 0.5 {
                    draw::dot_i(grid, x - direction as i32, lane_y);
                }
            }
        }
        Ok(())
    }
}

pub(crate) struct AgentRendezvous;

impl ProgressStyle for AgentRendezvous {
    fn name(&self) -> &str {
        "agent-rendezvous"
    }
    fn theme(&self) -> &str {
        "cockpit"
    }
    fn describe(&self) -> &str {
        "Slow reciprocal agent pings converge on the council beacon"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let center = (w.saturating_sub(1)) as f32 / 2.0;
        let mid = (h.saturating_sub(1)) as i32 / 2;
        for x in (0..w).step_by(4) {
            draw::dot_i(grid, x as i32, mid);
        }
        for ping in 0..4 {
            let phase = (ctx.time * 0.18 + ping as f32 / 4.0).fract();
            let x = (phase * center).round() as i32;
            let y = ((ping * 2 + 1) % h.max(1)) as i32;
            draw::dot_i(grid, x, y);
            draw::dot_i(grid, w as i32 - 1 - x, y);
        }
        draw::dot_i(grid, center.round() as i32, mid);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MotionCue {
    Thinking,
    Work,
    VerifierLock,
    AgentDispatch,
    AgentRendezvous,
    Settled(ToolState),
    Stalled,
}

fn motion_cue(strip: &ToolStrip) -> MotionCue {
    let Some(entry) = strip.current() else {
        return MotionCue::Thinking;
    };
    if entry.done {
        return MotionCue::Settled(if entry.not_started {
            ToolState::NotStarted
        } else if entry.err {
            ToolState::Failed
        } else if entry.inconclusive {
            ToolState::Inconclusive
        } else {
            ToolState::Passed
        });
    }
    if is_agent_wait_tool(&entry.name) {
        if entry.started.elapsed() < Duration::from_secs(60) {
            MotionCue::AgentDispatch
        } else {
            MotionCue::AgentRendezvous
        }
    } else if entry.verifier.is_some()
        || entry.name.ends_with("ui_verify")
        || entry.name.ends_with("ui_inspect")
    {
        MotionCue::VerifierLock
    } else {
        MotionCue::Work
    }
}

fn motion_cue_with_silence(strip: &ToolStrip, stalled: bool) -> MotionCue {
    if stalled {
        MotionCue::Stalled
    } else {
        motion_cue(strip)
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn bar_row(strip: &ToolStrip, width: usize, time: f32) -> String {
    bar_row_with_silence(strip, width, time, false)
}

pub(crate) fn bar_row_with_silence(
    strip: &ToolStrip,
    width: usize,
    time: f32,
    stalled: bool,
) -> String {
    if width == 0 {
        return String::new();
    }
    let rendered = match motion_cue_with_silence(strip, stalled) {
        MotionCue::Thinking => {
            let ctx = BarContext::new(1.0, time, width, 1);
            dotmax::progress::render_lines(&RibbonFlow, &ctx)
        }
        MotionCue::Work => {
            let ctx = BarContext::new(1.0, time, width, 1);
            dotmax::progress::render_lines(&ToolPulse, &ctx)
        }
        MotionCue::VerifierLock => {
            let ctx = BarContext::new(0.72, time, width, 1);
            dotmax::progress::render_lines(&SyncLock, &ctx)
        }
        MotionCue::AgentDispatch => {
            let ctx = BarContext::new(1.0, time, width, 1);
            dotmax::progress::render_lines(&AgentDispatch, &ctx)
        }
        MotionCue::AgentRendezvous => {
            let ctx = BarContext::new(1.0, time, width, 1);
            dotmax::progress::render_lines(&AgentRendezvous, &ctx)
        }
        MotionCue::Settled(state) => {
            let marker = match state {
                ToolState::Passed => "✓",
                ToolState::Failed => "✗",
                ToolState::NotStarted => "⊘",
                ToolState::Inconclusive => "?",
                ToolState::Running => unreachable!(),
            };
            let ctx = BarContext::new(0.0, 0.0, width, 1);
            let rail = dotmax::progress::render_lines(&StallFlatline, &ctx)
                .ok()
                .and_then(|lines| lines.into_iter().next())
                .unwrap_or_default();
            return format!("{marker}{}", rail.chars().skip(1).collect::<String>());
        }
        MotionCue::Stalled => {
            let ctx = BarContext::new(0.0, time, width, 1);
            dotmax::progress::render_lines(&StallFlatline, &ctx)
        }
    };
    rendered
        .ok()
        .and_then(|mut lines| (!lines.is_empty()).then(|| lines.remove(0)))
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/toolstrip__tests.rs"]
mod tests;
