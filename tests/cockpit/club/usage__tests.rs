use super::*;
use serde_json::json;

#[test]
fn provider_usage_http_distinguishes_missing_zero_and_malformed_counts() {
    for value in [
        json!(null),
        json!({}),
        json!([]),
        json!({"cost":1}),
        json!({"prompt_tokens":-1,"completion_tokens":"3"}),
        json!({"prompt_tokens":1.5}),
    ] {
        assert!(parse_http_usage(&value).is_none(), "{value}");
    }
    let zero =
        parse_http_usage(&json!({"prompt_tokens":0,"input_tokens":17,"completion_tokens":0}))
            .unwrap();
    assert_eq!(
        zero.input,
        Some(0),
        "reported zero must not fall back to another alias"
    );
    assert_eq!(zero.output, Some(0));
    assert_eq!(zero.reasoning, None);
    let partial = parse_http_usage(
        &json!({"prompt_tokens":null,"input_tokens":12,"completion_tokens":null,"output_tokens":5}),
    )
    .unwrap();
    assert_eq!(
        (partial.input, partial.output, partial.reasoning),
        (Some(12), Some(5), None)
    );
}

#[test]
fn provider_usage_http_cache_aliases_preserve_family_fields() {
    let deepseek = parse_http_usage(&json!({"prompt_tokens":100,"prompt_tokens_details":{"cached_tokens":null},"prompt_cache_hit_tokens":75})).unwrap();
    assert_eq!(deepseek.cache_read, Some(75));
    let anthropic = parse_http_usage(&json!({"input_tokens":100,"cache_read_input_tokens":20,"cache_write_input_tokens":null,"cache_creation_input_tokens":4})).unwrap();
    assert_eq!(
        (anthropic.input, anthropic.cache_read, anthropic.cache_write),
        (Some(100), Some(20), Some(4))
    );
    // These are separate wire schemas, both with a nested cache-write
    // field. Parsing does not guess or recombine their prompt totals.
    for (value, expected) in [
        (
            json!({"input_tokens":100,"input_tokens_details":{"cached_tokens":20,"cache_write_tokens":10}}),
            10,
        ),
        (
            json!({"prompt_tokens":194,"prompt_tokens_details":{"cached_tokens":0,"cache_write_tokens":100}}),
            100,
        ),
    ] {
        let parsed = parse_http_usage(&value).unwrap();
        assert_eq!(parsed.cache_write, Some(expected));
    }
    let cache_only = parse_http_usage(&json!({"cache_read_input_tokens":0})).unwrap();
    assert_eq!(cache_only.cache_read, Some(0));
    assert!(!cache_only.has_tokens());
}

#[test]
fn provider_usage_grok_acp_preserves_partial_and_zero_observations() {
    for value in [
        json!(null),
        json!({}),
        json!([]),
        json!({"inputTokens":"12"}),
    ] {
        assert!(parse_grok_acp_usage(&json!({"_meta":{"usage":value}})).is_none());
    }
    let partial = parse_grok_acp_usage(&json!({"_meta":{"usage":{"inputTokens":17}}})).unwrap();
    assert_eq!(
        (partial.input, partial.output, partial.reasoning),
        (Some(17), None, None)
    );
    let zero = parse_grok_acp_usage(
        &json!({"_meta":{"usage":{"inputTokens":0,"outputTokens":0,"reasoningTokens":0}}}),
    )
    .unwrap();
    assert_eq!(
        (zero.input, zero.output, zero.reasoning),
        (Some(0), Some(0), Some(0))
    );
}
