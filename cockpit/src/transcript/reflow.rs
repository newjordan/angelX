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
mod tests {
    use super::*;
    #[test]
    fn budgets_preserve_progress_and_appended_tail() {
        let mut messages: Vec<_> = (0..7)
            .map(|_| Message::new(super::super::Role::User, "row"))
            .collect();
        let mut work = PendingReflow::new(40);
        work.advance(&messages, 0, || false, |_, _| panic!("zero count"));
        work.advance(&messages, 4, || true, |_, _| panic!("zero time"));
        assert!(work.heights.is_empty());
        work.advance(
            &messages,
            3,
            || false,
            |_, width| {
                assert_eq!(width, 40);
                Some(2)
            },
        );
        assert_eq!(work.prefix, [0, 2, 4, 6]);
        messages.push(Message::new(super::super::Role::User, "tail"));
        let mut calls = 0;
        work.advance(
            &messages,
            10,
            || {
                calls += 1;
                calls > 2
            },
            |_, _| Some(3),
        );
        assert_eq!(work.prefix, [0, 2, 4, 6, 9, 12]);
        assert!(!work.ready(messages.len()));
        work.advance(&messages, 10, || false, |_, _| Some(4));
        assert!(work.ready(messages.len()));
        assert_eq!(work.prefix, [0, 2, 4, 6, 9, 12, 16, 20, 24]);
    }
}
