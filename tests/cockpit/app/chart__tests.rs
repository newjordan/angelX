use super::*;

#[test]
fn chart_has_requested_height() {
    let series = [1.0, 3.0, 2.0, 5.0, 4.0];
    assert_eq!(line_chart(&series, 20, 4).lines.len(), 4);
}

#[test]
fn chart_plots_some_dots() {
    // A non-flat series should set at least one non-empty braille cell.
    let series = [0.0, 1.0, 0.0, 1.0, 0.0];
    let text = line_chart(&series, 20, 3);
    let any = text.lines.iter().any(|l| {
        l.spans
            .iter()
            .any(|s| s.content.chars().any(|c| c != '\u{2800}' && c != ' '))
    });
    assert!(any, "expected at least one filled braille cell");
}

#[test]
fn empty_and_single_point_dont_panic() {
    let _ = line_chart(&[], 10, 2);
    let _ = line_chart(&[1.0], 10, 2);
}
