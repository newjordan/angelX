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
//! `~/.angelX/stream_rules.json`. Each stream uses one snapshot; valid edits
//! apply to the next stream. Missing or malformed replacements keep the last
//! valid snapshot for that path. Format:
//!
//! ```json
//! [
//!   { "pattern": "(?i)\\bas an ai\\b", "reminder": "Don't hedge — just answer." },
//!   { "pattern": "TODO: implement", "reminder": "Write the real implementation, no stubs." }
//! ]
//! ```

use regex::{Regex, RegexBuilder};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

pub(crate) struct StreamRule {
    pattern: Regex,
    pub(crate) reminder: String,
}

pub(crate) struct StreamRules {
    rules: Vec<StreamRule>,
}

impl StreamRules {
    /// Snapshot the current file once per streaming request. In-flight retries
    /// retain their original set even if another request observes an edit.
    pub(crate) fn global() -> Arc<StreamRules> {
        static CACHE: OnceLock<Mutex<RulesCache>> = OnceLock::new();
        let mut cache = CACHE
            .get_or_init(|| Mutex::new(RulesCache::default()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        cache.snapshot(rules_source().as_deref())
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

#[derive(Default)]
struct RulesCache {
    source: Option<PathBuf>,
    accepted: Option<String>,
    rejected: Option<String>,
    active: Arc<StreamRules>,
}

impl Default for StreamRules {
    fn default() -> Self {
        Self { rules: Vec::new() }
    }
}

impl RulesCache {
    fn snapshot(&mut self, source: Option<&Path>) -> Arc<StreamRules> {
        // A different configured path is a different policy, not a replacement
        // of the previous file. Do not carry rules across that boundary.
        if self.source.as_deref() != source {
            self.source = source.map(Path::to_path_buf);
            self.accepted = None;
            self.rejected = None;
            self.active = Arc::new(StreamRules::default());
        }
        if let Some(text) = source.and_then(read_bounded_rules) {
            if self.accepted.as_deref() != Some(&text) && self.rejected.as_deref() != Some(&text) {
                if let Some(rules) = parse_candidate(&text, self.accepted.is_some()) {
                    self.active = Arc::new(StreamRules { rules });
                    self.accepted = Some(text);
                    self.rejected = None;
                } else {
                    self.rejected = Some(text);
                }
            }
        }
        Arc::clone(&self.active)
    }
}

const MAX_RULES_BYTES: u64 = 1024 * 1024;
const MAX_RULES: usize = 256;
const MAX_PATTERN_BYTES: usize = 4096;
const MAX_REMINDER_BYTES: usize = 16384;

fn read_bounded_rules(path: &Path) -> Option<String> {
    if !std::fs::metadata(path).ok()?.is_file() {
        return None;
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    // A configured path may be replaced with a FIFO between metadata and open.
    // Never wait for a writer while holding the process-wide cache lock.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(path).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_RULES_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_RULES_BYTES {
        return None;
    }
    String::from_utf8(bytes).ok()
}

fn rules_source() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ANGEL_STREAM_RULES") {
        return Some(path.into());
    }
    let home = std::env::var_os("HOME")?;
    Some(Path::new(&home).join(".angelX").join("stream_rules.json"))
}

/// At startup preserve the legacy behavior of skipping malformed entries. Once
/// a file has been accepted, every replacement must validate as a whole.
#[cfg(test)]
fn parse_rules(json: &str) -> Vec<StreamRule> {
    parse_candidate(json, false).unwrap_or_default()
}

fn parse_candidate(json: &str, strict: bool) -> Option<Vec<StreamRule>> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let array = value.as_array()?;
    if array.len() > MAX_RULES {
        return None;
    }
    let mut rules = Vec::new();
    for entry in array {
        let rule = (|| {
            let source = entry.get("pattern")?.as_str()?;
            let reminder = entry.get("reminder")?.as_str()?;
            if source.len() > MAX_PATTERN_BYTES
                || reminder.is_empty()
                || reminder.len() > MAX_REMINDER_BYTES
            {
                return None;
            }
            let pattern = RegexBuilder::new(source)
                .size_limit(1024 * 1024)
                .build()
                .ok()?;
            Some(StreamRule {
                pattern,
                reminder: reminder.to_owned(),
            })
        })();
        match rule {
            Some(rule) => rules.push(rule),
            None if strict => return None,
            None => {}
        }
    }
    Some(rules)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/stream_rules__tests.rs"]
mod tests;
