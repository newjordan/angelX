//! Mid-run steering: message the model without interrupting its work.
//!
//! While a turn (or `/loop` iteration) holds the flight slot, a plain typed
//! message no longer bounces off the busy gate — it is queued here, shown in
//! the transcript immediately (marked queued), and the turn worker drains the
//! queue at its next hop boundary
//! ([`run_turn_steered`](crate::agent::harness::run_turn_steered)), so the model sees
//! the note as a user message in its very next request while the main
//! objective keeps running. Esc still interrupts (and drops anything queued);
//! a note the turn never got to see is delivered as the immediate next user
//! turn instead of being lost
//! ([`App::flush_queued_steers`](crate::App::flush_queued_steers)). A live
//! `/loop` additionally folds steers into its persistent
//! [`steer_notes`](crate::drive::loop_ctl::LoopState::steer_notes) so fresh-context
//! iterations keep honoring them.

use crate::agent::club::ChatMsg;
use std::collections::VecDeque;
use std::sync::Mutex;

/// Harness-role framing inserted immediately before queued User notes. The
/// guidance explains their mid-run timing without rewriting the operator's
/// text or granting harness-generated prose User provenance.
pub const STEER_CONTEXT: &str = "[harness steer context — the following User message(s) were sent mid-run; \
     take them into account and keep pursuing the main objective]";

/// FIFO of steers queued while a turn holds the flight slot. Shared between
/// the UI thread (pushes on Enter) and the turn worker (drains at hop
/// boundaries); one queue lives on the `App` for the whole session so a steer
/// that misses one worker is picked up by the next.
#[derive(Default)]
pub struct SteerQueue {
    inner: Mutex<VecDeque<ChatMsg>>,
}

impl SteerQueue {
    /// Poison-tolerant lock: the queue holds plain messages with no invariant
    /// to corrupt, and a panicking worker thread must never brick the UI's
    /// ability to queue/drain steers.
    fn queue(&self) -> std::sync::MutexGuard<'_, VecDeque<ChatMsg>> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn push(&self, msg: ChatMsg) {
        self.queue().push_back(msg);
    }

    /// Take everything queued so far, oldest first.
    pub fn drain(&self) -> Vec<ChatMsg> {
        self.queue().drain(..).collect()
    }

    pub fn len(&self) -> usize {
        self.queue().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub fn context_message() -> ChatMsg {
    ChatMsg::harness(STEER_CONTEXT)
}

/// First line of a steer, capped, for the delivery notice in the activity
/// trace.
pub fn snippet(text: &str) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    let capped: String = line.chars().take(60).collect();
    if capped.chars().count() < line.chars().count() {
        format!("{capped}…")
    } else {
        capped
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/steer__tests.rs"]
mod tests;
