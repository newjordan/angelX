//! Time-Traveling Stream Rules (TTSR) — ported from oh-my-pi
//! (github.com/can1357/oh-my-pi).
//!
//! A rule sits dormant until the model's streamed output drifts off-script. When
//! a rule's regex matches the text streamed so far, the club aborts the stream
//! mid-generation, injects the rule's `reminder` as a system message, and
//! retries the request from the same point — course-correcting *without* paying
//! the context tax of prepending every rule to every turn.
//!
//! Rules are **opt-in**: with none configured (the default) the streaming loop's
//! check is a single `is_empty()` short-circuit, so the hot path is untouched.
//! Configure via `ANGEL_STREAM_RULES` (a path to a JSON file) or, failing that,
//! `~/.angel0/stream_rules.json`. Format:
//!
//! ```json
//! [
//!   { "pattern": "(?i)\\bas an ai\\b", "reminder": "Don't hedge — just answer." },
//!   { "pattern": "TODO: implement", "reminder": "Write the real implementation, no stubs." }
//! ]
//! ```

use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;

pub(crate) struct StreamRule {
    pattern: Regex,
    pub(crate) reminder: String,
}

pub(crate) struct StreamRules {
    rules: Vec<StreamRule>,
}

impl StreamRules {
    /// Process-global rules, loaded once.
    pub(crate) fn global() -> &'static StreamRules {
        static RULES: OnceLock<StreamRules> = OnceLock::new();
        RULES.get_or_init(StreamRules::load)
    }

    pub(crate) fn load() -> StreamRules {
        let Some(text) = read_rules_source() else {
            return StreamRules { rules: Vec::new() };
        };
        StreamRules {
            rules: parse_rules(&text),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn from_json_for_test(json: &str) -> Self {
        Self {
            rules: parse_rules(json),
        }
    }

    /// The first rule (by declaration order) whose pattern matches `text` and
    /// whose index is not already in `fired`. `fired` lets the caller avoid
    /// re-tripping a rule it already injected, which bounds the retry loop.
    pub(crate) fn first_new_match(
        &self,
        text: &str,
        fired: &HashSet<usize>,
    ) -> Option<(usize, &StreamRule)> {
        self.rules
            .iter()
            .enumerate()
            .find(|(i, rule)| !fired.contains(i) && rule.pattern.is_match(text))
    }
}

fn read_rules_source() -> Option<String> {
    if let Some(path) = std::env::var_os("ANGEL_STREAM_RULES") {
        return std::fs::read_to_string(path).ok();
    }
    let home = std::env::var_os("HOME")?;
    let default = std::path::Path::new(&home)
        .join(".angel0")
        .join("stream_rules.json");
    std::fs::read_to_string(default).ok()
}

/// Parse a rules JSON array. Malformed entries (bad regex, missing fields) are
/// skipped rather than failing the whole file, so one typo can't disarm TTSR
/// entirely — the skipped rule is simply inert.
fn parse_rules(json: &str) -> Vec<StreamRule> {
    let value: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let Some(array) = value.as_array() else {
        return Vec::new();
    };
    let mut rules = Vec::new();
    for entry in array {
        let Some(pattern_src) = entry.get("pattern").and_then(|v| v.as_str()) else {
            continue;
        };
        let reminder = entry
            .get("reminder")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if reminder.is_empty() {
            continue;
        }
        if let Ok(pattern) = Regex::new(pattern_src) {
            rules.push(StreamRule { pattern, reminder });
        }
    }
    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_rules_and_skips_bad_ones() {
        let json = r#"[
            {"pattern": "(?i)as an ai", "reminder": "answer directly"},
            {"pattern": "TODO", "reminder": ""},
            {"pattern": "(unclosed", "reminder": "bad regex is skipped"},
            {"reminder": "no pattern"},
            {"pattern": "stub", "reminder": "no stubs"}
        ]"#;
        let rules = parse_rules(json);
        // Only the two well-formed rules survive; empty-reminder and bad-regex
        // and missing-pattern entries are dropped.
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].reminder, "answer directly");
        assert_eq!(rules[1].reminder, "no stubs");
    }

    #[test]
    fn first_new_match_respects_fired_set() {
        let rules = StreamRules {
            rules: parse_rules(
                r#"[{"pattern":"foo","reminder":"r1"},{"pattern":"bar","reminder":"r2"}]"#,
            ),
        };
        let mut fired = HashSet::new();
        // Matches rule 0 first.
        let (idx, rule) = rules.first_new_match("xx foo yy", &fired).unwrap();
        assert_eq!(idx, 0);
        assert_eq!(rule.reminder, "r1");
        // After firing 0, the same text no longer trips it.
        fired.insert(0);
        assert!(rules.first_new_match("xx foo yy", &fired).is_none());
        // A different pattern still trips.
        let (idx, _) = rules.first_new_match("foo and bar", &fired).unwrap();
        assert_eq!(idx, 1);
    }

    #[test]
    fn empty_or_garbage_source_yields_no_rules() {
        assert!(parse_rules("").is_empty());
        assert!(parse_rules("not json").is_empty());
        assert!(parse_rules("{}").is_empty());
        assert!(parse_rules("[]").is_empty());
    }
}
