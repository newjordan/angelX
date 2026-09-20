use super::*;
use crate::club::Club;
use crate::openai_codex::{parse_responses_event, tests::club};

#[test]
fn all_terminal_paths_keep_cache_and_do_not_double_count_snapshots() {
    let club = club();
    for kind in [
        "response.completed",
        "response.failed",
        "response.incomplete",
    ] {
        let mut attempt = Attempt::new(
            &club,
            br#"{"model":"fixture","reasoning":{"effort":"low"}}"#,
        );
        let payload = format!(
            r#"data: {{"type":"{kind}","response":{{"usage":{{"input_tokens":100,"input_tokens_details":{{"cached_tokens":80}},"output_tokens":20,"output_tokens_details":{{"reasoning_tokens":12}}}}}}}}"#
        );
        let event = parse_responses_event(&payload);
        attempt.observe(&event);
        attempt.observe(&event);
    }
    let usage = club.token_usage().unwrap();
    assert_eq!(
        (
            usage.turns,
            usage.total_input,
            usage.total_output,
            usage.total_reasoning
        ),
        (3, 300, 60, 36)
    );
    let cache = club.cache_usage();
    assert_eq!(
        (cache.read_accounting_responses, cache.read_input_tokens),
        (3, 240)
    );
    let stats = club.shared.usage.lock().unwrap();
    assert_eq!(stats.attempts, 3);
    assert_eq!(stats.unknown_usage_attempts, 0);
    let outcomes: Vec<_> = stats.recent_attempts.iter().map(|r| r.outcome).collect();
    assert_eq!(outcomes, ["completed", "failed", "incomplete"]);
    assert!(
        stats
            .recent_attempts
            .iter()
            .all(|r| r.wire_effort.as_deref() == Some("low"))
    );
}

#[test]
fn interrupted_and_cancelled_attempts_keep_observed_usage_or_explicit_unknown() {
    let club = club();
    {
        let mut attempt = Attempt::new(&club, br#"{"model":"fixture"}"#);
        attempt.outcome("interrupted");
        attempt.observe(&parse_responses_event(r#"data: {"type":"response.in_progress","response":{"usage":{"input_tokens":7,"output_tokens":2}}}"#));
    }
    {
        let mut attempt = Attempt::new(&club, br#"{"model":"fixture"}"#);
        attempt.outcome("cancelled");
    }
    let stats = club.shared.usage.lock().unwrap();
    assert_eq!((stats.attempts, stats.unknown_usage_attempts), (2, 1));
    assert_eq!(stats.recent_attempts[0].usage.unwrap().input, Some(7));
    assert!(stats.recent_attempts[1].usage.is_none());
    assert!(stats.recent_attempts[1].wire_effort.is_none());
}

#[test]
fn cancellation_accounts_for_a_received_terminal_line_and_partial_fields_merge() {
    use std::io::BufRead;
    let club = club();
    {
        let mut attempt = Attempt::new(&club, br#"{"model":"fixture"}"#);
        attempt.receive(r#"data: {"type":"response.in_progress","response":{"usage":{"input_tokens":100,"output_tokens":10,"input_tokens_details":{"cached_tokens":80}}}}"#, false);
        let reader = std::io::Cursor::new(
            br#"data: {"type":"response.completed","response":{"usage":{"output_tokens":20}}}"#,
        );
        for line in reader.lines() {
            attempt.receive(&line.unwrap(), true);
        }
    }
    let stats = club.shared.usage.lock().unwrap();
    let receipt = stats.recent_attempts.back().unwrap();
    assert_eq!(receipt.outcome, "cancelled");
    let usage = receipt.usage.unwrap();
    assert_eq!(
        (usage.input, usage.output, usage.cached_input),
        (Some(100), Some(20), Some(80))
    );
    assert_eq!(
        (stats.unknown_usage_attempts, stats.partial_usage_attempts),
        (0, 0)
    );
}

#[test]
fn cache_only_usage_keeps_ordinary_counts_unknown() {
    let club = club();
    {
        let mut attempt = Attempt::new(&club, br#"{"model":"fixture"}"#);
        attempt.receive(r#"data: {"type":"response.failed","response":{"usage":{"input_tokens_details":{"cached_tokens":80}}}}"#, false);
    }
    let stats = club.shared.usage.lock().unwrap();
    let usage = stats.recent_attempts.back().unwrap().usage.unwrap();
    assert!(usage.input.is_none() && usage.output.is_none());
    assert_eq!(stats.partial_usage_attempts, 1);
    assert_eq!(club.cache_usage().read_input_tokens, 80);
}

#[test]
fn request_failures_are_bounded_and_unknown_is_not_reported_as_zero() {
    let club = club();
    for _ in 0..RECEIPT_CAP + 5 {
        let _attempt = Attempt::new(&club, br#"{"model":"fixture"}"#);
    }
    let stats = club.shared.usage.lock().unwrap();
    assert_eq!(stats.attempts, 69);
    assert_eq!(stats.unknown_usage_attempts, 69);
    assert_eq!(stats.recent_attempts.len(), RECEIPT_CAP);
    assert_eq!(stats.turns, 0);
    let json = serde_json::to_value(stats.recent_attempts.back().unwrap()).unwrap();
    assert!(json["usage"].is_null());
}
