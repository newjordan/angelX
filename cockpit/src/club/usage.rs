//! Provider-family usage parsing that preserves absent versus reported-zero counts.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ReportedUsage {
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub reasoning: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
    pub paths: [Option<&'static str>; 5],
}

impl ReportedUsage {
    pub fn has_tokens(self) -> bool {
        [self.input, self.output, self.reasoning]
            .iter()
            .any(Option::is_some)
    }

    fn any(self) -> bool {
        self.has_tokens() || self.cache_read.is_some() || self.cache_write.is_some()
    }

    /// Each field is a cumulative observation for this one provider attempt.
    /// Missing/null fields are not a reset; an explicit numeric zero is.
    pub fn update(&mut self, newer: Self) {
        self.input = newer.input.or(self.input);
        self.output = newer.output.or(self.output);
        self.reasoning = newer.reasoning.or(self.reasoning);
        self.cache_read = newer.cache_read.or(self.cache_read);
        self.cache_write = newer.cache_write.or(self.cache_write);
        for (path, newer) in self.paths.iter_mut().zip(newer.paths) {
            *path = newer.or(*path);
        }
    }
}

fn first_count(usage: &serde_json::Value, paths: &[&str]) -> Option<u64> {
    paths
        .iter()
        .find_map(|path| usage.pointer(path).and_then(serde_json::Value::as_u64))
}

fn first_path(usage: &serde_json::Value, paths: &[&str]) -> Option<&'static str> {
    let pointer = paths.iter().find(|path| {
        usage
            .pointer(path)
            .and_then(serde_json::Value::as_u64)
            .is_some()
    })?;
    match *pointer {
        "/prompt_tokens" => Some("prompt_tokens"),
        "/input_tokens" => Some("input_tokens"),
        "/completion_tokens" => Some("completion_tokens"),
        "/output_tokens" => Some("output_tokens"),
        "/completion_tokens_details/reasoning_tokens" => {
            Some("completion_tokens_details.reasoning_tokens")
        }
        "/output_tokens_details/reasoning_tokens" => Some("output_tokens_details.reasoning_tokens"),
        "/cache_read_input_tokens" => Some("cache_read_input_tokens"),
        "/prompt_tokens_details/cached_tokens" => Some("prompt_tokens_details.cached_tokens"),
        "/input_tokens_details/cached_tokens" => Some("input_tokens_details.cached_tokens"),
        "/prompt_cache_hit_tokens" => Some("prompt_cache_hit_tokens"),
        "/cache_write_input_tokens" => Some("cache_write_input_tokens"),
        "/cache_creation_input_tokens" => Some("cache_creation_input_tokens"),
        "/prompt_tokens_details/cache_write_tokens" => {
            Some("prompt_tokens_details.cache_write_tokens")
        }
        "/input_tokens_details/cache_write_tokens" => {
            Some("input_tokens_details.cache_write_tokens")
        }
        "/inputTokens" => Some("_meta.usage.inputTokens"),
        "/outputTokens" => Some("_meta.usage.outputTokens"),
        "/reasoningTokens" => Some("_meta.usage.reasoningTokens"),
        _ => None,
    }
}

/// HTTP Chat Completions and its explicit field aliases. Counts are retained
/// as reported; this parser does not guess inclusive/disjoint cache semantics.
pub(super) fn parse_http_usage(usage: &serde_json::Value) -> Option<ReportedUsage> {
    usage.as_object()?;
    let mut counts = ReportedUsage {
        input: first_count(usage, &["/prompt_tokens", "/input_tokens"]),
        output: first_count(usage, &["/completion_tokens", "/output_tokens"]),
        reasoning: first_count(
            usage,
            &[
                "/completion_tokens_details/reasoning_tokens",
                "/output_tokens_details/reasoning_tokens",
            ],
        ),
        cache_read: first_count(
            usage,
            &[
                "/cache_read_input_tokens",
                "/prompt_tokens_details/cached_tokens",
                "/input_tokens_details/cached_tokens",
                "/prompt_cache_hit_tokens",
            ],
        ),
        cache_write: first_count(
            usage,
            &[
                "/cache_write_input_tokens",
                "/cache_creation_input_tokens",
                "/prompt_tokens_details/cache_write_tokens",
                "/input_tokens_details/cache_write_tokens",
            ],
        ),
        ..ReportedUsage::default()
    };
    counts.paths = [
        first_path(usage, &["/prompt_tokens", "/input_tokens"]),
        first_path(usage, &["/completion_tokens", "/output_tokens"]),
        first_path(
            usage,
            &[
                "/completion_tokens_details/reasoning_tokens",
                "/output_tokens_details/reasoning_tokens",
            ],
        ),
        first_path(
            usage,
            &[
                "/cache_read_input_tokens",
                "/prompt_tokens_details/cached_tokens",
                "/input_tokens_details/cached_tokens",
                "/prompt_cache_hit_tokens",
            ],
        ),
        first_path(
            usage,
            &[
                "/cache_write_input_tokens",
                "/cache_creation_input_tokens",
                "/prompt_tokens_details/cache_write_tokens",
                "/input_tokens_details/cache_write_tokens",
            ],
        ),
    ];
    counts.any().then_some(counts)
}

/// Grok ACP exposes a distinct camelCase extension, not HTTP field aliases.
pub(super) fn parse_grok_acp_usage(result: &serde_json::Value) -> Option<ReportedUsage> {
    let usage = result.pointer("/_meta/usage")?;
    usage.as_object()?;
    let mut counts = ReportedUsage {
        input: first_count(usage, &["/inputTokens"]),
        output: first_count(usage, &["/outputTokens"]),
        reasoning: first_count(usage, &["/reasoningTokens"]),
        ..ReportedUsage::default()
    };
    counts.paths = [
        first_path(usage, &["/inputTokens"]),
        first_path(usage, &["/outputTokens"]),
        first_path(usage, &["/reasoningTokens"]),
        None,
        None,
    ];
    counts.any().then_some(counts)
}

#[cfg(test)]
mod tests {
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
        let partial = parse_http_usage(&json!({"prompt_tokens":null,"input_tokens":12,"completion_tokens":null,"output_tokens":5})).unwrap();
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
}
