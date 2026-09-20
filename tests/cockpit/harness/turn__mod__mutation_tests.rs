use super::*;
const MARKER: &str = "\n…[post-edit diagnostics truncated]";

#[test]
fn d02_tool_wall_zero_and_equal_totals_preserve_members() {
    assert_eq!(tool_wall_shares(&[0], 0), vec![0]);
    assert_eq!(tool_wall_shares(&[2, 3], 5), vec![2, 3]);
    assert!(tool_wall_shares(&[], 0).is_empty());
}

#[test]
fn d02_diagnostic_oversize_is_bounded() {
    let text = "a".repeat(200);
    assert!(cap_post_edit_diagnostics(text, 7).len() <= 7);
}

#[test]
fn d02_diagnostic_short_output_is_unchanged() {
    assert_eq!(cap_post_edit_diagnostics("ok".into(), 128), "ok");
    assert_eq!(cap_post_edit_diagnostics(String::new(), 0), "");
}

#[test]
fn d02_diagnostic_retains_the_utf8_prefix() {
    assert_eq!(
        cap_post_edit_diagnostics("é".repeat(100), MARKER.len() + 3),
        format!("é{MARKER}")
    );
}

#[test]
fn d02_diagnostic_marker_fills_exact_budget() {
    assert_eq!(
        cap_post_edit_diagnostics("a".repeat(200), MARKER.len()),
        MARKER
    );
}
