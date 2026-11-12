//! How many tool calls the turn's idle watcher believes are running.

use super::TurnEvent;

/// Tool calls the model has started and not yet seen finish. The idle watcher
/// gives a due tool-idle escalation a few seconds' grace only while this is
/// above zero.
///
/// Calls overlap: a parallel batch runs each call on its own thread, and a
/// `code_mode` call reports every nested call as its own call/result pair
/// inside its own. So "a tool is running" is a count; a flag cleared by the
/// first result would claim nothing is running while a sibling still is.
#[derive(Debug, Default)]
pub(super) struct ToolsInFlight(usize);

impl ToolsInFlight {
    /// Fold one relayed event in. Every call announces itself with one
    /// `ToolCall` and ends with one `ToolResult`. A result that was never
    /// announced (a worker that died before it spoke) cannot take the count
    /// below zero.
    pub(super) fn note(&mut self, event: &TurnEvent) {
        match event {
            TurnEvent::ToolCall { .. } => self.0 += 1,
            TurnEvent::ToolResult { .. } => self.0 = self.0.saturating_sub(1),
            _ => {}
        }
    }

    pub(super) fn any(&self) -> bool {
        self.0 > 0
    }
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/turn__tools_in_flight_tests.rs"]
mod tests;
