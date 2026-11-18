//! How many tool calls the turn's idle watcher believes are running.

use super::TurnEvent;

#[derive(Debug, Default)]
pub(super) struct ToolsInFlight(usize);

impl ToolsInFlight {
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
