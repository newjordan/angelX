//! The silent-repeat guard. With thinking off, a DeepSeek seat's visible text
//! is its only reasoning; when the text stops, the diagnosis stops and the
//! calls repeat. The `⠇` stamps did not break it: on polyglot-v1 (3 seeds ×
//! 136, 2026-11-02) rust-bowling re-ran `cargo test` silently 52 times after a
//! red (37 identical in a row) and rust-robot-name sent `echo hi` 60 times
//! under a pasted-back system prompt, both to the hop cap.
//!
//! The guard watches each hop's batch and its text. A batch repeated
//! [`SAME_CALL_LIMIT`] times in a row with no new text, or
//! [`SILENT_HOP_LIMIT`] tool hops in a row with no text at all, earns one short
//! harness turn in plain words asking for a diagnosis before the next call, and
//! flags the next hop for the effort leg (`stuck`), which wakes thinking on a
//! seat that has it off. At most [`NOTES_PER_TURN`] notes per turn; a note
//! restarts both counts, so the guard can never answer its own note.
use super::*;

/// The same batch with no new text this many hops in a row. In the 808 solved
/// task-runs of that sheet no batch ever came back twice in a row; both 60-call
/// repeat loops passed three by their 8th call.
pub(super) const SAME_CALL_LIMIT: usize = 3;
/// Tool hops in a row with empty text. Every silent loop of the sheet (7/7)
/// reached eight by its 16th call; 5 of 808 solved task-runs did, too, and
/// for them the note costs one short reply.
pub(super) const SILENT_HOP_LIMIT: usize = 8;
/// Notes a turn may give; after the last, the guard only counts.
pub(super) const NOTES_PER_TURN: usize = 3;
const ARGS_CHARS: usize = 160;

#[derive(Default)]
pub(super) struct SilenceGuard {
    last_batch: Option<String>,
    last_text: Option<String>,
    same: usize,
    silent: usize,
    notes: usize,
    wake: bool,
}

impl SilenceGuard {
    /// One tool hop as issued: its visible text and its batch. Answers the
    /// harness note to give after the hop's results, when one is due.
    pub(super) fn observe(&mut self, text: Option<&str>, calls: &[ToolCall]) -> Option<String> {
        if calls.is_empty() {
            return None;
        }
        let text = text.map(str::trim).unwrap_or_default();
        // A reply that only repeats the last one (a pasted-back prompt) says
        // nothing new either.
        let quiet = text.is_empty() || self.last_text.as_deref() == Some(text);
        let batch = calls
            .iter()
            .map(book::l_loops::toolcall_storm_signature)
            .collect::<Vec<_>>()
            .join("\n");
        self.same = if quiet && self.last_batch.as_deref() == Some(batch.as_str()) {
            self.same + 1
        } else {
            1
        };
        self.silent = if text.is_empty() { self.silent + 1 } else { 0 };
        self.last_text = Some(text.to_string());
        self.last_batch = Some(batch);
        let note = if self.same >= SAME_CALL_LIMIT {
            let call = &calls[0];
            let args: String = call.args.to_string().chars().take(ARGS_CHARS).collect();
            let what = if calls.len() == 1 {
                format!("`{} {args}`", call.name)
            } else {
                format!("the same {}-call batch (first `{} {args}`)", calls.len(), call.name)
            };
            format!(
                "{} You have issued {what} {} times in a row with nothing new written between \
                 them. Before your next tool call, write one or two lines: what the last result \
                 shows, and what you will do differently. Do not repeat this call unchanged.",
                book::l_loops::WARNING,
                self.same,
            )
        } else if self.silent >= SILENT_HOP_LIMIT {
            format!(
                "{} Your last {} tool calls came with no text. Before your next tool call, write \
                 one or two lines: what the latest result shows, and what you will change.",
                book::l_loops::WARNING,
                self.silent,
            )
        } else {
            return None;
        };
        self.same = 0;
        self.silent = 0;
        if self.notes >= NOTES_PER_TURN {
            return None;
        }
        self.notes += 1;
        self.wake = true;
        Some(note)
    }

    /// Whether the coming hop follows a note: read once, for one hop.
    pub(super) fn take_wake(&mut self) -> bool {
        std::mem::take(&mut self.wake)
    }
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/silence_guard__tests.rs"]
mod tests;
