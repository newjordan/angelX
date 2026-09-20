use super::reasoning_reveal_boundary;
use unicode_segmentation::UnicodeSegmentation;

#[test]
fn reasoning_roll_keeps_received_graphemes_complete() {
    for text in [
        "12345678Z",
        "1234567e\u{301}Z",
        "a👩‍💻Z",
        "abcd👍🏻Z",
        "1234🇺🇸Z",
        "\u{600}aZ",
        "क्‍षZ",
    ] {
        let mut shown = 0;
        while shown < text.len() {
            let next = reasoning_reveal_boundary(text, shown, 8);
            assert!(next > shown && next <= text.len());
            assert!(
                next == text.len() || text.grapheme_indices(true).any(|(i, _)| i == next),
                "{text:?} shown={shown} next={next}"
            );
            shown = next;
        }
    }
    assert_eq!(reasoning_reveal_boundary("", 0, 8), 0);
    assert_eq!(reasoning_reveal_boundary("12345678Z", 0, 8), 8);
}

#[test]
fn reasoning_roll_handles_extensions_to_a_prior_chunk() {
    for (before, after) in [
        ("a👩", "a👩‍💻Z"),
        ("1234567e", "1234567e\u{301}Z"),
        ("abcd👍", "abcd👍🏻Z"),
        ("1234🇺", "1234🇺🇸Z"),
    ] {
        let next = reasoning_reveal_boundary(after, before.len(), 8);
        assert!(next == after.len() || after.grapheme_indices(true).any(|(i, _)| i == next));
    }
}

#[test]
fn reasoning_roll_boundary_property_sweep() {
    let samples = [
        "".to_string(),
        "ascii".to_string(),
        "1234567e\u{301}Z".to_string(),
        "a👩‍💻Z".to_string(),
        "abcd👍🏻Z".to_string(),
        "1234🇺🇸Z".to_string(),
        "\u{600}aZ".to_string(),
        "क्‍षZ".to_string(),
        "🇺🇸".repeat(64),
        format!("e{}Z", "\u{301}".repeat(128)),
        "e\u{301}".repeat(64),
    ];
    for text in samples {
        let mut boundaries = text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        boundaries.push(text.len());
        let mut previous_by_step = [0; 5];
        for shown in text
            .char_indices()
            .map(|(i, _)| i)
            .chain(std::iter::once(text.len()))
        {
            let mut previous = shown;
            for (index, step) in [0, 1, 2, 8, usize::MAX].into_iter().enumerate() {
                let next = reasoning_reveal_boundary(&text, shown, step);
                assert!(
                    shown <= next
                        && previous <= next
                        && previous_by_step[index] <= next
                        && next <= text.len(),
                    "shown={shown} step={step} next={next} text={text:?}"
                );
                assert!(
                    boundaries.contains(&next),
                    "partial grapheme: shown={shown} step={step} next={next} text={text:?}"
                );
                previous = next;
                previous_by_step[index] = next;
            }
        }
    }
}
