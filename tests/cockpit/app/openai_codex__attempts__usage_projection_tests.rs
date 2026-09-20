use super::*;
#[test]
fn usage_projection_codex_attempts_preserve_cache_only_zero_and_unknown() {
    use crate::agent::club::Club;
    let club = crate::agent::openai_codex::tests::club();
    let before = club.usage_accounting();
    {
        let mut attempt = Attempt::new(&club, br#"{"model":"fixture"}"#);
        attempt.receive(r#"data: {"type":"response.failed","response":{"usage":{"input_tokens_details":{"cached_tokens":80}}}}"#, false);
    }
    {
        let _unknown = Attempt::new(&club, br#"{"model":"fixture"}"#);
    }
    {
        let mut attempt = Attempt::new(&club, br#"{"model":"fixture"}"#);
        attempt.receive(r#"data: {"type":"response.completed","response":{"usage":{"input_tokens":0,"output_tokens":0}}}"#, false);
    }
    let report = crate::agent::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
    assert_eq!(
        (
            report.attempts,
            report.input,
            report.output,
            report.reasoning,
            report.cache_read
        ),
        (3, Some(0), Some(0), None, Some(80))
    );
    assert_eq!(
        (
            report.reported_attempts.input,
            report.reported_attempts.cache_read
        ),
        (1, 1)
    );
    assert_eq!(
        report.raw_field_reports["response.usage.input_tokens_details.cached_tokens"],
        1
    );
    assert_eq!((report.uncached_input, report.cache_hit_pct), (None, None));
    assert!(!report.core_complete);
}
