//! One unpublished resize index. Oversized messages yield until their worker
//! height is ready; published resize geometry is never partial.
use super::Message;

pub(crate) struct PendingReflow {
    pub(crate) width: u16,
    pub(crate) heights: Vec<u16>,
    pub(crate) prefix: Vec<u32>,
}

impl PendingReflow {
    pub(crate) fn new(width: u16) -> Self {
        Self {
            width,
            heights: Vec::new(),
            prefix: vec![0],
        }
    }

    pub(crate) fn ready(&self, message_count: usize) -> bool {
        self.heights.len() == message_count
    }

    pub(crate) fn advance(
        &mut self,
        messages: &[Message],
        max_messages: usize,
        mut should_yield: impl FnMut() -> bool,
        mut measure: impl FnMut(&Message, usize) -> Option<u16>,
    ) {
        for message in messages.iter().skip(self.heights.len()).take(max_messages) {
            if should_yield() {
                break;
            }
            let Some(height) = measure(message, usize::from(self.width)) else {
                break;
            };
            self.heights.push(height);
            self.prefix.push(
                self.prefix
                    .last()
                    .copied()
                    .unwrap_or(0)
                    .saturating_add(u32::from(height)),
            );
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/transcript__reflow__tests.rs"]
mod tests;
