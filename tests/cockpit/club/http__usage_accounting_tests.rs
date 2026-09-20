use super::*;
#[test]
fn provider_usage_http_explicit_zero_is_observed() {
    let c = HttpClub::new("usage-fixture", "http://127.0.0.1:9/v1", "fixture", None);
    c.record_usage_json(&serde_json::json!({"usage":{"prompt_tokens":0,"completion_tokens":0,"completion_tokens_details":{"reasoning_tokens":0}}}));
    assert_eq!(c.token_usage().unwrap_or_default().turns, 1);
    assert_eq!(c.token_usage().unwrap_or_default().total_input, 0);
}

#[test]
fn provider_usage_http_null_aliases_do_not_hide_valid_responses_counts() {
    let c = HttpClub::new("usage-fixture", "http://127.0.0.1:9/v1", "fixture", None);
    c.record_usage_json(&serde_json::json!({"usage":{"prompt_tokens":null,"input_tokens":12,"completion_tokens":null,"output_tokens":5,"prompt_tokens_details":{"cached_tokens":null},"input_tokens_details":{"cached_tokens":7}}}));
    assert_eq!(c.token_usage().unwrap_or_default().total_input, 12);
    assert_eq!(c.token_usage().unwrap_or_default().total_output, 5);
    assert_eq!(c.cache_usage().read_input_tokens, 7);
}

#[test]
fn provider_usage_deepseek_null_detail_keeps_native_cache_hit() {
    let c = HttpClub::new("usage-fixture", "http://127.0.0.1:9/v1", "fixture", None);
    c.record_usage_json(&serde_json::json!({"usage":{"prompt_tokens":100,"completion_tokens":5,"prompt_tokens_details":{"cached_tokens":null},"prompt_cache_hit_tokens":75}}));
    assert_eq!(c.cache_usage().read_input_tokens, 75);
}

#[test]
fn provider_usage_anthropic_null_write_alias_keeps_cache_creation() {
    let c = HttpClub::new("usage-fixture", "http://127.0.0.1:9/v1", "fixture", None);
    c.record_usage_json(&serde_json::json!({"usage":{"input_tokens":100,"output_tokens":5,"cache_write_input_tokens":null,"cache_creation_input_tokens":4,"cache_read_input_tokens":20}}));
    assert_eq!(c.cache_usage().write_input_tokens, 4);
    assert_eq!(c.token_usage().unwrap_or_default().total_input, 100);
}

#[test]
fn provider_usage_http_responses_style_cache_write_detail_is_retained() {
    let c = HttpClub::new("usage-fixture", "http://127.0.0.1:9/v1", "fixture", None);
    c.record_usage_json(&serde_json::json!({"usage":{"input_tokens":100,"output_tokens":5,"input_tokens_details":{"cached_tokens":20,"cache_write_tokens":10}}}));
    assert_eq!(c.cache_usage().write_input_tokens, 10);
}

#[test]
fn provider_usage_http_metadata_only_final_frame_keeps_last_usage() {
    for metadata in [
        serde_json::Value::Null,
        serde_json::json!({}),
        serde_json::json!({"billing":"pending"}),
    ] {
        let c = HttpClub::new("usage-fixture", "http://127.0.0.1:9/v1", "fixture", None);
        {
            let mut commit = StreamUsageCommit::new(&c);
            commit
                .observe(&serde_json::json!({"usage":{"prompt_tokens":12,"completion_tokens":5}}));
            commit.observe(&serde_json::json!({"usage":metadata}));
        }
        assert_eq!(c.token_usage().unwrap_or_default().total_input, 12);
        assert_eq!(c.token_usage().unwrap_or_default().total_output, 5);
        assert_eq!(c.token_usage().unwrap_or_default().turns, 1);
    }
}

#[test]
fn provider_usage_http_partial_cumulative_frame_keeps_previously_reported_input() {
    let c = HttpClub::new("usage-fixture", "http://127.0.0.1:9/v1", "fixture", None);
    {
        let mut commit = StreamUsageCommit::new(&c);
        commit.observe(&serde_json::json!({"usage":{"prompt_tokens":12,"completion_tokens":5}}));
        commit.observe(&serde_json::json!({"usage":{"completion_tokens":6}}));
    }
    assert_eq!(c.token_usage().unwrap_or_default().total_input, 12);
    assert_eq!(c.token_usage().unwrap_or_default().total_output, 6);
    assert_eq!(c.token_usage().unwrap_or_default().turns, 1);
}

#[test]
fn provider_usage_http_explicit_zero_final_frame_overrides_prior_cumulative_counts() {
    let c = HttpClub::new("usage-fixture", "http://127.0.0.1:9/v1", "fixture", None);
    {
        let mut commit = StreamUsageCommit::new(&c);
        commit.observe(&serde_json::json!({"usage":{"prompt_tokens":12,"completion_tokens":5}}));
        commit.observe(&serde_json::json!({"usage":{"prompt_tokens":0,"completion_tokens":0}}));
    }
    assert_eq!(c.token_usage().unwrap_or_default().turns, 1);
    assert_eq!(c.token_usage().unwrap_or_default().total_input, 0);
    assert_eq!(c.token_usage().unwrap_or_default().total_output, 0);
}

#[test]
fn provider_usage_http_cumulative_corrections_and_attempts_do_not_double_count() {
    let c = HttpClub::new("usage-fixture", "http://127.0.0.1:9/v1", "fixture", None);
    for _ in 0..2 {
        let mut commit = StreamUsageCommit::new(&c);
        commit.observe(&serde_json::json!({"usage":{"prompt_tokens":12,"completion_tokens":5}}));
        commit.observe(&serde_json::json!({"usage":{"prompt_tokens":10,"completion_tokens":4}}));
    }
    assert_eq!(c.token_usage().unwrap_or_default().turns, 2);
    assert_eq!(c.token_usage().unwrap_or_default().total_input, 20);
    assert_eq!(c.token_usage().unwrap_or_default().total_output, 8);
}
