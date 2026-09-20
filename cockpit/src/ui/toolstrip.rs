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

/// How many recent result/note events the warp-drive rate window keeps.
const WARP_EVENT_CAP: usize = 32;
/// Sliding window for event-rate → drive mapping.
const WARP_EVENT_WINDOW: Duration = Duration::from_secs(2);
/// Honest calm glide when nothing is landing.
const WARP_DRIVE_MIN: f32 = 0.35;
/// Parallel results landing — warp streaks.
const WARP_DRIVE_MAX: f32 = 3.0;
/// Event rate (events/sec) that saturates drive at [`WARP_DRIVE_MAX`].
const WARP_RATE_SATURATION: f32 = 5.0;

/// Semantic state for the entry pinned in the live strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolState {
    Running,
    Passed,
    NotStarted,
    Failed,
    Inconclusive,
}

/// One tool call observed this turn.
pub(crate) struct ToolEntry {
    pub(crate) id: ToolEventId,
    pub(crate) name: String,
    pub(crate) args: String,
    pub(crate) done: bool,
    pub(crate) err: bool,
    not_started: bool,
    inconclusive: bool,
    verifier: Option<&'static str>,
    result_detail: String,
    /// Dispatch truth for receipt classification: a verified measurement or
    /// submission receipt requires `Succeeded`; a failed one arms the loop's
    /// verifier-blocked diagnostic.
    execution: ExecutionOutcome,
    /// Redacted command, exit, tail and digest for a failed verifier.
    err_tail: String,
    verifier_failure: Option<VerifierFailure>,
    /// When this call started — drives the live `call Ns` fragment.
    started: Instant,
    /// Wall time from start to result, stamped once in `result_event`.
    elapsed_ms: Option<u64>,
    /// Redacted identity of the bounded result summary. Outcome fingerprints
    /// can distinguish a changed score/status receipt without persisting its
    /// potentially sensitive text.
    result_digest: Option<String>,
}

/// Machine-readable evidence from one turn's tool activity. The visual strip
/// used to be the only consumer, so these facts disappeared as soon as its
/// tally rendered. Long-running loops retain the snapshot for progress
/// admission and anti-repeat checks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ToolStripSnapshot {
    pub(crate) calls: usize,
    pub(crate) errors: usize,
    pub(crate) incomplete: usize,
    pub(crate) diagnostics: usize,
    /// Canonical fingerprints for costly or externally mutating actions. Output
    /// filenames and display-only pipe tails are removed so the same benchmark
    /// cannot look novel merely by writing to a different `/tmp` path.
    pub(crate) costly_actions: Vec<String>,
    /// Successful actions that materially advance a candidate: mutation,
    /// validation/benchmark, submission, or score/status. Long-running loops
    /// deduplicate these receipts and combine them with actual workspace change
    /// instead of treating prose novelty as the sole progress authority.
    pub(crate) outcome_actions: Vec<String>,
    /// Verified measurement/submission receipts: calls whose dispatch
    /// **succeeded** and whose command actually names a benchmark, measure,
    /// verify, validate, or submit action. Bare `git status`, `submissions`
    /// listings, status polls, and failed calls stay out — they are activity,
    /// not a measured candidate. These are execution evidence and activity
    /// counters; an opaque result digest does not prove objective improvement.
    pub(crate) verified_outcome_actions: Vec<String>,
    /// Failed measurement/submission actions with a bounded redacted diagnostic.
    pub(crate) verifier_failures: Vec<(String, String)>,
    pub(crate) verifier_failure_details: Vec<VerifierFailure>,
}

/// Styled status-row content. Keeping the metadata separate lets the draw
/// layer dim it without sacrificing the semantic state color on the left.
pub(crate) struct StatusRow {
    pub(crate) left: String,
    pub(crate) padding: usize,
    pub(crate) right: String,
    /// Stream-silence readout riding the right rail, kept apart from `right`
    /// so the draw layer can color it without re-parsing the rail text.
    pub(crate) stall: Option<StallReadout>,
    pub(crate) state: ToolState,
    pub(crate) verifier: bool,
}

/// Per-turn tool activity, reset when a new turn is submitted.
#[derive(Default)]
pub(crate) struct ToolStrip {
    entries: Vec<ToolEntry>,
    /// When the first tool of the turn started — drives the elapsed readout.
    started: Option<Instant>,
    diagnostics: usize,
    /// Latest routine harness murmur (guards, capsules, compaction, recall).
    /// In Conversation mode these ride here — one in-place line under the
    /// status row — instead of stacking in the scrollback; `/trace` restores
    /// the full stream.
    note: Option<String>,
    notes: usize,
    /// Distinct `notice_strip_prefix` labels seen this turn, arrival order.
    /// The draw path keeps mixed facts visible on the single note row.
    note_prefixes: Vec<String>,
    /// Recent result/note event times for warp-drive pulse (capped + windowed).
    event_times: VecDeque<Instant>,
    /// Virtual clock the pulse bar advances with — `dt * drive` per frame.
    /// Interior mutability so the draw path (`&App`) can tick it.
    warp_clock: Cell<f32>,
    note_roll: RefCell<rolling_text::RollingText>,
    /// Explicit transcript reading focus. Core also owns the composer, so its
    /// module focus alone cannot distinguish reading from ordinary typing.
    reading_paused: bool,
    warp_last_tick: Cell<Option<Instant>>,
    ambience: RefCell<ambient::Ambient>,
}

impl ToolStrip {
    /// A new turn begins: forget the previous turn's activity.
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

    /// A routine notice routed to the strip's note line (Conversation mode).
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

    /// Ambient time is independent of event-rate and provider silence. It
    /// survives turn boundaries, while the status row keeps its real receipt.
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

    /// Whether a dispatched tool still owns work for this turn. The provider
    /// stream is expected to be quiet between its call and result events, so
    /// the provider-idle watchdog must not mistake that silence for a hung
    /// socket. Tool implementations retain their own bounded deadlines.
    pub(crate) fn has_running_calls(&self) -> bool {
        self.entries.iter().any(|entry| !entry.done)
    }

    pub(crate) fn is_waiting_on_agents(&self) -> bool {
        self.current()
            .is_some_and(|entry| !entry.done && is_agent_wait_tool(&entry.name))
    }

    /// A tool call started.
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
        });
    }

    /// A tool result closes exactly its provider call id. Unknown and duplicate
    /// ids are neutral diagnostics: they cannot mutate another entry or count a
    /// second outcome.
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
        if e.verifier.is_some() {
            e.result_detail = verifier_result_detail(summary);
        }
        if matches!(
            outcome.execution,
            ExecutionOutcome::Failed | ExecutionOutcome::Panicked
        ) && measured_submission_fingerprint(&e.name, &e.args).is_some()
        {
            let failure = VerifierFailure::new(&e.args, summary);
            e.err_tail = failure.diagnostic();
            e.verifier_failure = Some(failure);
        }
        self.record_warp_event();
    }

    /// Stamp a result/note into the warp-drive event window.
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

    /// Event-rate → drive factor. Silent strip glides at [`WARP_DRIVE_MIN`];
    /// a burst of parallel results approaches [`WARP_DRIVE_MAX`]. `reduced`
    /// clamps to 1.0 (MotionMode::Reduced). Pure display — no agent feedback.
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
        // Smoothstep ease so the bar doesn't hard-jump between drive bands.
        let t = t * t * (3.0 - 2.0 * t);
        WARP_DRIVE_MIN + t * (WARP_DRIVE_MAX - WARP_DRIVE_MIN)
    }

    /// Advance the virtual pulse clock by `dt * drive` and return it. Safe to
    /// call from the draw path via interior mutability.
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

    /// The entry the status row shows: the newest still-running call, else the
    /// newest overall (a just-finished tool lingers until the next starts).
    pub(crate) fn current(&self) -> Option<&ToolEntry> {
        self.entries
            .iter()
            .rev()
            .find(|e| !e.done)
            .or_else(|| self.entries.last())
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
                .filter(|entry| entry.done && !entry.err)
                .filter_map(|entry| {
                    let action = outcome_action_fingerprint(&entry.name, &entry.args)?;
                    let digest = entry.result_digest.as_deref().unwrap_or("no-result");
                    Some(format!("{action}:result={digest}"))
                })
                .collect(),
            verified_outcome_actions: self
                .entries
                .iter()
                .filter(|entry| entry.done && entry.execution == ExecutionOutcome::Succeeded)
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
            verifier_failures: self
                .entries
                .iter()
                .filter(|entry| {
                    entry.done
                        && matches!(
                            entry.execution,
                            ExecutionOutcome::Failed | ExecutionOutcome::Panicked
                        )
                })
                .filter_map(|entry| {
                    let (action, _) = measured_submission_fingerprint(&entry.name, &entry.args)?;
                    Some((action, entry.err_tail.clone()))
                })
                .collect(),
        }
    }

    /// Collapse the turn's activity into the one transcript line that survives
    /// it, then reset. `None` when no tools ran (nothing worth a line).
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
        // Tally per tool name, ordered by first appearance.
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
        // Include incomplete calls: cancellation can suppress late result events.
        // An hour-long delegate must not disappear behind a 56s completed read.
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

/// One-row strip note. Same-family repeats still collapse to a count. Mixed
/// prefixes lead the row so end-ellipsis cannot hide earlier distinct facts
/// behind only the latest sentence.
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

/// Count plus distinct family prefixes in arrival order, without live sentences.
fn note_tally_text(notes: usize, prefixes: &[String]) -> String {
    let label = if notes == 1 { "note" } else { "notes" };
    if prefixes.is_empty() {
        format!("{notes} {label}")
    } else {
        format!("{notes} {label} ({})", prefixes.join(", "))
    }
}

/// Compact age for the strip right rail / tally (`43s`, `2m10s`, `1h05m`).
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
    // Structured tool summaries separate each count with commas/semicolons.
    // Keeping those clauses distinct avoids stealing the next count from
    // `10 passed, 2 failed` while still accepting `errors: 2`.
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

/// Canonical command shape shared by the action fingerprints: output
/// filenames and display-only pipe tails are removed so the same action
/// cannot look novel merely by writing to a different path or paging its
/// output.
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

    // Normalize GPU-MODE popcorn submits (cli, wrapper, pipes) so Treebeard /
    // gpu-comp agents don't thrash the same B200 benchmark under new paths.
    let joined = canonical_command_key(&args_key);
    if joined.contains("popcorn") {
        let mode = if joined.contains("--mode test") || joined.contains("mode test") {
            "test"
        } else if joined.contains("--mode leaderboard") || joined.contains("mode leaderboard") {
            "leaderboard"
        } else {
            "benchmark"
        };
        // Mode-only identity: filenames, peer flags, and board aliases are noise
        // for "already burning a B200 slot" dedupe during a single turn.
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

// Program-position words that name a real measurement. `bench` covers `benchd`, `bench.sh`
// and `benchmark*`; `baseline`/`score` cover board entrypoints such as `local-baseline.sh` and
// `*-measure-and-score.sh`. Operands never count (see READ_ONLY_PROGRAMS), so `cat score.json`
// and `grep bench …` stay reads. Operator finding 2026-09-11: the qwen38 board loop ran 7
// iterations of real `benchd`/`local-baseline` measurements that never counted as measured.
const MEASUREMENT_WORDS: [&str; 7] = [
    "benchmark",
    "bench",
    "measure",
    "verify",
    "validate",
    "baseline",
    "score",
];

/// Read-only viewers: a measurement word inside their arguments names a file,
/// not an execution. `cat matrices-leader/benchmark.json` minted a "measured
/// candidate" on every iteration of a 19-iteration, zero-submission matrices
/// run (Toymaker, 2026-09-05); the first-candidate clock never fired.
const READ_ONLY_PROGRAMS: [&str; 24] = [
    "cat", "head", "tail", "less", "more", "grep", "rg", "ls", "echo", "printf", "jq", "find",
    "stat", "wc", "diff", "sed", "awk", "cp", "mv", "ln", "file", "tree", "bat", "git",
];

/// Generic launchers: the measurement identity is the token they launch
/// (`timeout 900 python3 bench/measure.py` → `measure.py`).
const RUNNERS: [&str; 22] = [
    "bash", "sh", "zsh", "python", "python3", "uv", "cargo", "npm", "npx", "node", "bun",
    "timeout", "nice", "env", "sudo", "time", "make", "just", "poetry", "pnpm", "yarn", "exec",
];

fn basename(token: &str) -> &str {
    token.rsplit('/').next().unwrap_or(token)
}

fn is_env_assignment(token: &str) -> bool {
    !token.starts_with('-')
        && token.split_once('=').is_some_and(|(key, _)| {
            !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

/// Command segments of a shell line (`;`, `|`, `||`, `&&`, newlines) — split only OUTSIDE
/// single/double quotes, and never inside a heredoc body (`<<TAG` … `TAG`). Operator finding
/// 2026-09-11: `grep -rn "verify_rows\|read_row…"` and a `python3 - <<'py' …` body were split at
/// their inner `|` / newlines, a fragment such as `verify_ro…"` landed in program position, and the
/// loop minted two "measured candidates" from a read and a script body.
fn command_segments(args_key: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut heredoc: Option<String> = None;
    let mut heredoc_line = String::new();
    let mut chars = args_key.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(tag) = heredoc.as_deref() {
            // Inside a heredoc body: swallow everything until a line equal to the terminator.
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
                // `<<-TAG`, `<<'TAG'`, `<<"TAG"`, `<<TAG`
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
                // The rest of this line still belongs to the command; the body starts after it.
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
            }
            '|' => {
                if chars.peek() == Some(&'|') {
                    chars.next();
                }
                segments.push(std::mem::take(&mut current));
            }
            '&' if chars.peek() == Some(&'&') => {
                chars.next();
                segments.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    segments.push(current);
    segments
}

/// A token can be a program only if it looks like one: a bare command or path. Fragments carrying
/// quotes, backslashes, brackets, or `=` are operands (or shell noise), never programs.
fn looks_like_program(token: &str) -> bool {
    !token.is_empty()
        && token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '/' | '-' | '_' | '~' | '+'))
}

/// The program a segment runs (basename) and its first operand, looking
/// through generic launchers, flags, and numeric operands.
fn segment_program(segment: &str) -> Option<(String, Option<String>)> {
    let mut tokens = segment
        .split_whitespace()
        .filter(|token| !is_env_assignment(token));
    let mut program = tokens.next()?;
    if !looks_like_program(program) {
        return None;
    }
    for _ in 0..4 {
        if !RUNNERS.contains(&basename(program)) {
            break;
        }
        let next = tokens
            .by_ref()
            .find(|token| !(token.starts_with('-') || token.chars().all(|c| c.is_ascii_digit())));
        match next {
            Some(token) => program = token,
            None => break,
        }
    }
    let operand = tokens.next().map(str::to_string);
    Some((basename(program).to_string(), operand))
}

/// Does this shell line *execute* a measurement — a benchmark / measure /
/// verify / validate program or script in program position?
fn shell_measurement(args_key: &str) -> bool {
    command_segments(args_key).iter().any(|segment| {
        segment_program(segment).is_some_and(|(program, _)| {
            !READ_ONLY_PROGRAMS.contains(&program.as_str())
                && MEASUREMENT_WORDS.iter().any(|word| program.contains(word))
        })
    })
}

/// Does this shell line *execute* a submission — `submit` as the program or
/// its subcommand (`hilbert submit`, `yukon submit`, `./submit.sh`), and not a
/// help / dry-run invocation?
fn shell_submission(args_key: &str) -> bool {
    command_segments(args_key).iter().any(|segment| {
        if segment
            .split_whitespace()
            .any(|token| matches!(token, "--help" | "-h" | "help" | "--dry-run"))
        {
            return false;
        }
        segment_program(segment).is_some_and(|(program, operand)| {
            // A read-only viewer never submits, whatever its operands name
            // (`ls challenge/…/submissions`, `cat submitting.md` minted a "submission"
            // in the operator's ripe loop, 2026-09-11).
            !READ_ONLY_PROGRAMS.contains(&program.as_str())
                && (program.contains("submit") || operand.as_deref() == Some("submit"))
        })
    })
}

/// Tools whose arguments are a command line the sandbox runs.
fn runs_commands(name_key: &str) -> bool {
    matches!(name_key, "shell" | "proc_run" | "bash" | "terminal")
        || name_key.contains("shell")
        || name_key.contains("proc_run")
}

/// A *verified* outcome receipt must name a real measurement or submission.
/// For command-running tools the judgement is on the program the line
/// executes: a `benchmark*.sh` / `bench*` / `benchd` / `*measure*` / `verify` / `validate` /
/// `*baseline*` / `*score*` program in program position, or `submit` as program / subcommand. A measurement
/// word inside an operand (`cat …/benchmark.json`, `grep score benchmark.log`)
/// is a read, `submit --help` is a read, and `code_mode` plans are reads:
/// none of them are receipts. Other tools qualify by name only (a `benchmark`
/// tool, a `submit` tool). Bare `git status`, `submissions` listings, and
/// status polls deliberately never match, so polling cannot masquerade as a
/// measured candidate. Returns the canonical fingerprint and whether it is a
/// submission.
fn measured_submission_fingerprint(name: &str, args: &str) -> Option<(String, bool)> {
    let name_key = name.trim().to_ascii_lowercase();
    let args_key = args.trim().to_ascii_lowercase();
    let (measurement, submission) = if runs_commands(&name_key) {
        (shell_measurement(&args_key), shell_submission(&args_key))
    } else {
        (
            MEASUREMENT_WORDS.iter().any(|word| name_key.contains(word)),
            name_key
                .split(|c: char| !c.is_ascii_alphanumeric())
                .any(|token| token == "submit"),
        )
    };
    if !submission && !measurement {
        return None;
    }
    Some((
        format!("{name_key}:{}", canonical_command_key(&args_key)),
        submission,
    ))
}

/// Redacted, bounded execution evidence, kept separate from display summaries.
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
        // Shell output labels stderr lines. Prefer its last lines, then fill
        // remaining slots with stdout; a quiet failure remains explicit.
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

/// Fit `text` to `width` terminal cells with a `…` when it overflows.
pub(crate) fn ellipsize(text: &str, width: usize) -> String {
    ellipsize_with_width(text, width, UnicodeWidthStr::width(text))
}

// Rolling notes retain their total width from note arrival. Static/reduced
// frames must use that fact instead of rescanning the entire note.
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

/// Seconds of stream silence before the strip calls the turn stalled.
/// `ANGEL_STALL_PULSE_SECS` overrides; 0 disarms the readout and the
/// flattened lane entirely.
const DEFAULT_STALL_PULSE_SECS: u64 = 20;

pub(crate) fn configured_stall_pulse_secs() -> u64 {
    // Launch config in release: the strip asks every frame while a turn is
    // silent. Tests keep the live read so env guards can exercise overrides.
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

/// How urgent a stream-silence readout is: `Warn` past the pulse threshold,
/// `Watchdog` once the idle-abandon deadline is close enough that its
/// countdown belongs on the strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StallSeverity {
    Warn,
    Watchdog,
}

/// One stream-silence fragment for the status row's right rail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StallReadout {
    pub(crate) text: String,
    pub(crate) severity: StallSeverity,
}

/// Pure stall-threshold logic. `silence_secs` is how long the worker stream
/// has been quiet, `pulse_secs` the operator threshold (0 = off), and
/// `idle_timeout_secs` the turn watchdog deadline. Past the threshold the
/// silence is reported outright; past half the deadline the readout escalates
/// to the abandonment countdown so the watchdog never fires as a surprise.
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

/// A running tool is not a silent provider: the provider has already handed
/// work to a bounded executor and will naturally emit nothing until the result
/// arrives. Keep the wait animation live and leave timeout ownership with the
/// tool instead of displaying a false provider-stall sentence/countdown.
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

/// Structured content for the strip's status row. Verifiers use explicit
/// RUNNING/PASS/FAIL language; ordinary tools retain the quieter legacy form.
/// The live draw path carries the silence readout, so outside tests this
/// stall-free form is currently a convenience wrapper only.
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

/// Worker telemetry takes precedence over the outer delegate's generic wait.
/// Never read /proc here: the process owner samples it off the draw thread.
pub(crate) fn worker_status_row(
    child: &crate::agent::harness::ChildSnapshot,
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
    let left = format!("▸ {operation} · {activity}{workers}");
    let output = child
        .output_age_secs
        .map(|age| format!("{} ago", format_age_secs(age)))
        .unwrap_or_else(|| "none".into());
    let right = format!(
        "run {} · output {output}",
        format_age_secs(child.elapsed_secs)
    );
    activity_status_row(left, right, width)
}

pub(crate) fn delegate_quiet_secs(delegate: &crate::agent::harness::DelegateSnapshot) -> u64 {
    delegate.progress_age_secs.unwrap_or(delegate.elapsed_secs)
}

pub(crate) fn delegate_status_row(
    delegate: &crate::agent::harness::DelegateSnapshot,
    width: usize,
) -> StatusRow {
    let quiet = delegate_quiet_secs(delegate);
    let left = if quiet >= 30 {
        let transport = if delegate.event_age_secs.is_some_and(|age| age <= 2) {
            " · connected"
        } else {
            ""
        };
        format!(
            "▸ delegate · quiet {}{transport} · last: {}",
            format_age_secs(quiet),
            delegate.phase
        )
    } else {
        format!("▸ delegate · {} · {}", delegate.phase, delegate.label)
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
    activity_status_row(left, right, width)
}

fn activity_status_row(left: String, right: String, width: usize) -> StatusRow {
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
        padding,
        right,
        stall: None,
        state: ToolState::Running,
        verifier: false,
    }
}

#[cfg(test)]
include!("../../../tests/cockpit/app/toolstrip__standalone_tests.rs");

/// `status_row_parts` plus the stream-silence readout: a present `stall`
/// fragment joins the right rail (budgeted like the rail, with one separating
/// space) so a parked stream reports itself on the row the operator already
/// watches. Very narrow panes drop it with the rest of the rail rather than
/// overflow; with `stall == None` the layout is byte-identical to the plain
/// form.
pub(crate) fn status_row_parts_with_silence(
    strip: &ToolStrip,
    width: usize,
    stall: Option<StallReadout>,
) -> Option<StatusRow> {
    let stall_width = stall.as_ref().map_or(0, |readout| {
        UnicodeWidthStr::width(readout.text.as_str()) + 1
    });
    let Some(cur) = strip.current() else {
        // No tool has run yet, but harness murmurs have: show the latest note
        // as the row so early-turn progress (recall, compaction, gates) is
        // visible without a scrollback line.
        let note = strip.note()?;
        let left = format!("\u{25b8} {note}");
        if stall_width == 0 || width <= stall_width + 4 {
            return Some(StatusRow {
                left: ellipsize(&left, width),
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
            padding,
            right: String::new(),
            stall,
            state: ToolState::Running,
            verifier: false,
        });
    };
    let state = if !cur.done {
        ToolState::Running
    } else if cur.not_started {
        ToolState::NotStarted
    } else if cur.err {
        ToolState::Failed
    } else if cur.inconclusive {
        ToolState::Inconclusive
    } else {
        ToolState::Passed
    };
    let mark = match state {
        ToolState::Running => "\u{25b8}",
        ToolState::Passed => "\u{2713}",
        ToolState::NotStarted => "\u{2298}",
        ToolState::Failed => "\u{2717}",
        ToolState::Inconclusive => "?",
    };
    let verifier = cur.verifier.is_some();
    let left = if !cur.done && is_agent_wait_tool(&cur.name) {
        if cur.args.is_empty() {
            "◇ COUNCIL · awaiting agents".to_string()
        } else {
            format!("◇ COUNCIL · awaiting agents · {}", cur.args)
        }
    } else if let Some(kind) = cur.verifier {
        if cur.done && !cur.result_detail.is_empty() {
            format!("{mark} VERIFY · {kind} · {}", cur.result_detail)
        } else if cur.args.is_empty() {
            format!("{mark} VERIFY · {kind}")
        } else {
            format!("{mark} VERIFY · {kind} · {}", cur.args)
        }
    } else if cur.not_started && cur.args.is_empty() {
        format!("{mark} NOT STARTED · {}", cur.name)
    } else if cur.not_started {
        format!("{mark} NOT STARTED · {} · {}", cur.name, cur.args)
    } else if cur.args.is_empty() {
        format!("{mark} {}", cur.name)
    } else {
        format!("{mark} {} · {}", cur.name, cur.args)
    };
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
    // Live fan-out pips: while a multi-seat stage has published seat states,
    // the right rail leads with "proposer wave 2 · 3/6 back".
    let seat_pips = crate::ui::viz::agentviz::current_seat_pips().unwrap_or_default();
    // Per-call age on the gating wait: unfinished entries use wall clock since
    // start; a lingering finished entry keeps its stamped duration (≥1s).
    let call_frag = if !cur.done {
        format!(
            "call {} · ",
            format_age_secs(cur.started.elapsed().as_secs())
        )
    } else if let Some(ms) = cur.elapsed_ms.filter(|ms| *ms >= 1000) {
        format!("call {} · ", format_age_secs(ms / 1000))
    } else {
        String::new()
    };
    let right = format!(
        "{seat_pips}{verdict}{call_frag}#{} · {}",
        strip.count(),
        format_age_secs(strip.elapsed_secs())
    );
    let rail_width = UnicodeWidthStr::width(right.as_str()) + stall_width;

    // Very narrow panes cannot carry a stable right rail. Prefer the semantic
    // state and operation over overflowing or wrapping the two-row strip.
    let gap = usize::from(width > 0);
    if width <= rail_width + gap + 4 {
        return Some(StatusRow {
            left: ellipsize(&left, width),
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
        padding,
        right,
        stall,
        state,
        verifier,
    })
}

/// The lateral loading bar under the status row — a dotmax `ProgressStyle`
/// in the house style: a dotted guide rail with data packets streaming
/// left→right on staggered cubic-out phases (each snaps ahead then glides),
/// and a soft shimmer band sweeping through so the lane never looks parked.
/// Indeterminate by design: tools don't report progress, so the bar shows
/// *motion*, not completion.
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

        // 1. Primary flowing sine wave across full width
        for x in 0..w {
            let phase = x as f32 * 0.18 - ctx.time * 4.5;
            let y_val = mid + amp * phase.sin();
            let y_dot = (y_val.round() as i32).clamp(0, h as i32 - 1);
            draw::dot_i(grid, x as i32, y_dot);
        }

        // 2. Traveling pulse / crest highlight along the sine wave
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

/// Flatline for a stalled stream — a static dotted rail at the midline. The
/// missing motion IS the signal: the animated lanes imply progress a parked
/// provider socket is not making, so past the stall threshold the bar parks
/// too.
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

/// Early agent wait: dispatch packets leave the council beacon and return on
/// staggered lanes. The bidirectional motion communicates that workers remain
/// live and that the parent is awaiting replies, rather than implying measured
/// completion.
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

        // Council beacon: a steady center with a quiet breathing halo.
        draw::dot_i(grid, center.round() as i32, mid);
        if (ctx.time * 2.0).sin() >= 0.0 {
            draw::dot_i(grid, center.round() as i32, (mid - 1).max(0));
            draw::dot_i(grid, center.round() as i32, (mid + 1).min(h as i32 - 1));
        }

        // Three seats, phase-staggered. Each packet moves center→edge→center;
        // mirrored seats keep the lane balanced without suggesting a percent.
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

/// Long agent wait: slow reciprocal pings converge on the council beacon. It
/// stays visibly alive while calming the faster dispatch motion after a minute,
/// reducing fatigue during long builds and research runs.
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
    /// The provider stream is silent past the stall threshold: freeze the
    /// lane so the strip stops faking motion nothing is making.
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

/// The strip-derived cue with the stall override: silence outranks whichever
/// motion style the current tool would otherwise pick.
fn motion_cue_with_silence(strip: &ToolStrip, stalled: bool) -> MotionCue {
    if stalled {
        MotionCue::Stalled
    } else {
        motion_cue(strip)
    }
}

/// Render one row of semantic Dotmax motion at `width` cells. Ordinary tools
/// retain the calm packet lane; verifier calls use the live v2
/// `glitch/sync-lock` signal so validation reads as a distinct instrumentation
/// state without adding another row. The live draw path carries the stall
/// flag, so outside tests this form is currently a convenience wrapper only.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn bar_row(strip: &ToolStrip, width: usize, time: f32) -> String {
    bar_row_with_silence(strip, width, time, false)
}

/// `bar_row` with the stall pulse: a `stalled` stream flattens the lane to
/// the static rail so a parked turn cannot masquerade as one doing work.
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
            // A correlated terminal result owns this stationary receipt. It
            // never implies that the completed call is still doing work.
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
