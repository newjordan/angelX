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

pub const STEER_CONTEXT: &str = "⠝⠚";

#[derive(Default)]
pub struct SteerQueue {
    inner: Mutex<VecDeque<ChatMsg>>,
}

impl SteerQueue {
    fn queue(&self) -> std::sync::MutexGuard<'_, VecDeque<ChatMsg>> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn push(&self, msg: ChatMsg) {
        self.queue().push_back(msg);
    }

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
