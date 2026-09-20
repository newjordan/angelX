use super::*;
use std::io::Read;

#[test]
fn trajectory_c03c_attempt_counts_utf8_partial_stream_and_unknown() {
    let _lock = crate::tests::env_lock();
    let club = HttpClub::new("fixture", "http://127.0.0.1:9/v1", "fixture", None);
    crate::agent::harness::reset_turn_ledger(&club);
    crate::agent::harness::note_timing_origin(Instant::now());
    crate::agent::harness::begin_model_request();
    let wire = "data: π\n\ndata: [DONE]\n\n".as_bytes();
    {
        let mut attempt = club.accounting.attempt();
        attempt.request_bytes("{\"prompt\":\"π\"}".len());
        let mut reader = attempt.response_reader(wire);
        let mut prefix = [0; 7];
        reader.read_exact(&mut prefix).unwrap();
    }
    {
        let _unobserved = club.accounting.attempt();
    }
    let samples = crate::agent::harness::provider_call_samples();
    assert_eq!(samples[0]["request_bytes"], "{\"prompt\":\"π\"}".len());
    assert_eq!(samples[0]["response_bytes"], 7);
    assert!(samples[1]["request_bytes"].is_null());
    assert!(samples[1]["response_bytes"].is_null());
    crate::agent::harness::end_model_request();
}

#[test]
fn trajectory_c03d_usage_missing_retry_is_unreported_and_not_a_partial_sum() {
    let _lock = crate::tests::env_lock();
    let club = HttpClub::new(
        "openrouter",
        "https://api.z.ai/api/coding/paas/v4",
        "fixture",
        None,
    );
    crate::agent::harness::reset_turn_ledger(&club);
    crate::agent::harness::note_timing_origin(Instant::now());
    crate::agent::harness::begin_model_request();
    let before = club.usage_accounting();
    {
        let mut missing = club.accounting.attempt();
        missing.request_bytes(300_000);
    }
    {
        let mut accounting = club.accounting.attempt();
        accounting.request_bytes(300_000);
        let mut reader = accounting.response_reader("fixture".as_bytes());
        let mut body = Vec::new();
        reader.read_to_end(&mut body).unwrap();
        let mut commit = StreamUsageCommit::from_attempt(&club, accounting);
        commit.observe(
            &serde_json::json!({"usage":{"prompt_tokens":100,"completion_tokens":5,
                "prompt_tokens_details":{"cached_tokens":40}}}),
        );
        commit.observe(&serde_json::json!({"usage":{"completion_tokens":6}}));
        commit.observe(&serde_json::json!({"usage":null}));
    }
    let samples = crate::agent::harness::provider_call_samples();
    assert_eq!(samples[0]["accounting_status"], "unreported");
    for key in [
        "raw_input",
        "paid_input",
        "cached_input",
        "output",
        "total_tokens",
        "response_bytes",
    ] {
        assert!(samples[0]["counters"][key].is_null());
    }
    assert_eq!(samples[1]["accounting_status"], "reported");
    assert_eq!(samples[1]["retry_of"], 0);
    assert_eq!(
        samples[1]["counters"],
        serde_json::json!({"raw_input":100,"paid_input":60,
            "cached_input":40,"output":6,"generation_output":6,"total_tokens":106,"response_bytes":7})
    );
    let report = crate::agent::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
    assert!(!report.core_complete);
    // The shared UI report intentionally retains observed subtotals;
    // only the durable ledger requires complete measurement coverage.
    assert_eq!(report.total_prompt, Some(100));
    let ledger_usage = crate::agent::harness::complete_ledger_usage(&report);
    for field in [
        "input",
        "output",
        "cache_read",
        "total_prompt",
        "generation_output",
    ] {
        assert!(
            ledger_usage[field].is_null(),
            "{field} must not be a partial sum"
        );
    }
    assert_eq!(ledger_usage["accounting_status"], "partial");
    assert_eq!(ledger_usage["reported_attempts"]["input"], 1);
    crate::agent::harness::end_model_request();
}

#[test]
fn trajectory_c03c_zai_chat_contract_is_known_on_openrouter_dialect() {
    let _lock = crate::tests::env_lock();
    for base in [
        "https://api.z.ai/api/coding/paas/v4",
        "https://openrouter.ai/api/v1",
        "http://127.0.0.1:9/v1",
    ] {
        let club = HttpClub::new("openrouter", base, "z-ai/glm-5", None);
        let usage = super::super::usage::parse_http_usage(&serde_json::json!({
            "prompt_tokens":100,"completion_tokens":12,
            "prompt_tokens_details":{"cached_tokens":40},
            "completion_tokens_details":{"reasoning_tokens":3}
        }))
        .unwrap();
        let observation = club.accounting_observation(usage);
        assert_eq!(
            observation.contract.cache,
            super::super::CacheConvention::Included
        );
        assert_eq!(
            observation.contract.reasoning,
            super::super::ReasoningConvention::Included
        );
    }
}
