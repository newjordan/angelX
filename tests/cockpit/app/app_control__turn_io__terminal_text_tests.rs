use super::*;

#[test]
fn streamed_provider_text_strips_split_terminal_controls_and_bidi() {
    let mut sanitizer = TerminalTextSanitizer::default();
    assert_eq!(sanitizer.push("safe\u{1b}[3"), "safe");
    assert_eq!(sanitizer.push("1m red\u{1b}]0;host"), " red");
    assert_eq!(sanitizer.push("ile title\u{7} tail\u{202e}"), " tail");
    assert_eq!(sanitizer.push("plain"), "plain");
}

#[test]
fn streamed_provider_text_normalizes_split_carriage_returns() {
    let mut sanitizer = TerminalTextSanitizer::default();
    assert_eq!(sanitizer.push("one\r"), "one");
    assert_eq!(sanitizer.push("\ntwo\rthree"), "\ntwo\nthree");
    assert_eq!(sanitizer.finish(), "");
    assert_eq!(sanitize_complete_terminal_text("tail\r"), "tail\n");
}

#[test]
fn reset_releases_an_unterminated_sequence_for_the_next_turn() {
    let mut sanitizer = TerminalTextSanitizer::default();
    assert_eq!(sanitizer.push("\u{1b}]0;never-ended"), "");
    sanitizer.reset();
    assert_eq!(sanitizer.push("next answer"), "next answer");
}
