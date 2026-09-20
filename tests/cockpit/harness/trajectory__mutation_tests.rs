use super::*;

#[test]
fn d02_trace_summary_obeys_character_limit() {
    let mut out = "prefix:".to_string();
    push_truncated_chars(&mut out, "é界x", 2);
    assert_eq!(out, "prefix:é界");
    push_truncated_chars(&mut out, "hidden", 0);
    assert_eq!(out, "prefix:é界");
}

#[test]
fn d02_trace_summary_has_only_between_field_separators() {
    assert_eq!(
        summarize_args(&serde_json::json!({"a": "x", "b": "y"})),
        "a=x, b=y"
    );
}

#[test]
fn d02_ledger_distinguishes_unreported_and_partial_accounting() {
    let mut report = crate::club::AccountingReport::from_snapshot(Default::default(), false);
    let absent = complete_ledger_usage(&report);
    assert_eq!(absent["accounting_status"], "unreported");
    assert!(absent["input"].is_null());
    report.attempts = 1;
    report.input = Some(12);
    report.reported_attempts.input = 1;
    let partial = complete_ledger_usage(&report);
    assert_eq!(partial["accounting_status"], "partial");
    assert_eq!(partial["input"], 12);
    assert!(partial["output"].is_null());
}
