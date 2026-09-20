use super::{LessonWrapMemo, ReasoningWrapKey, ReasoningWrapMemo};
use ratatui::widgets::{Paragraph, Wrap};

#[test]
fn unicode_replacements_and_large_shrinks_preserve_exact_bounded_snapshots() {
    let mut memo = ReasoningWrapMemo::default();
    let mut lesson = LessonWrapMemo::default();
    let key = ReasoningWrapKey {
        width: 31,
        ..Default::default()
    };
    for (middle, expected) in [("界a", 3), ("🦀", 2), ("e\u{301}a", 2)] {
        let text = format!("{}{}{}", "a".repeat(7), middle, "a".repeat(53));
        assert_eq!(text.len(), 64);
        let measure = || {
            Paragraph::new(text.as_str())
                .wrap(Wrap { trim: false })
                .line_count(key.width)
        };
        assert_eq!(memo.settled_lines(key, &text, measure), expected);
        assert_eq!(
            usize::from(lesson.wrapped_lines(key.width, false, &text, || measure() as u16)),
            expected
        );
    }
    let large = "x".repeat(1024 * 1024);
    memo.settled_lines(key, &large, || 1);
    lesson.wrapped_lines(key.width, false, &large, || 1);
    memo.settled_lines(key, "", || 0);
    lesson.wrapped_lines(key.width, false, "", || 0);
    assert!(memo.observed.capacity() <= 64 * 1024);
    assert!(lesson.observed.capacity() <= 64 * 1024);
}

#[test]
fn same_length_edits_between_old_samples_invalidate_wrapping() {
    let original = "a".repeat(64);
    let mut replacement = original.clone();
    replacement.replace_range(7..8, "\n");
    for index in [0, 63, 21, 42, 32] {
        assert_eq!(original.as_bytes()[index], replacement.as_bytes()[index]);
    }
    let key = ReasoningWrapKey {
        width: 16,
        ..Default::default()
    };
    let measure = |text: &str| {
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .line_count(key.width)
    };
    assert_eq!(measure(&original), 4);
    assert_eq!(measure(&replacement), 5);
    let mut memo = ReasoningWrapMemo::default();
    assert_eq!(memo.settled_lines(key, &original, || measure(&original)), 4);
    assert_eq!(
        memo.settled_lines(key, &original, || panic!("unchanged text rewrapped")),
        4
    );
    assert_eq!(
        memo.settled_lines(key, &replacement, || measure(&replacement)),
        5
    );
    let mut lesson = LessonWrapMemo::default();
    assert_eq!(
        lesson.wrapped_lines(16, false, &original, || measure(&original) as u16),
        4
    );
    assert_eq!(
        lesson.wrapped_lines(16, false, &original, || panic!(
            "unchanged lesson rewrapped"
        )),
        4
    );
    assert_eq!(
        lesson.wrapped_lines(16, false, &replacement, || measure(&replacement) as u16),
        5
    );
    let resized = ReasoningWrapKey { width: 32, ..key };
    assert_eq!(memo.settled_lines(resized, &original, || 2), 2);
}
