use super::*;
use crate::agent::harness::live_policy;

const RESULT_CLASSIFY_LEG: &str = "result-classify";

const TOOL_ERROR_PREFIX: &str = "tool error:";

const EMPTY_REPLY_MARKER: &str = "empty reply (no text and no tool calls)";
const CLUB_STREAM_MARKER: &str = "club stream produced no data";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ResultFeatures {
    error: bool,
    verifier: bool,
    has_exit: bool,
    exit: u32,
    empty_marker: bool,
    club_stream: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub(crate) struct ResultVerdict {
    error: bool,
    red_verifier: bool,
    dispatch_failure: bool,
    empty_reply: bool,
}

impl ResultFeatures {
    fn of_result(result: &str) -> Self {
        Self {
            error: carries_error_prefix(result),
            ..Self::default()
        }
    }

    fn of_call(call: &ToolCall, result: &str) -> Self {
        let (has_exit, exit) = exit_code_class(result);
        Self {
            error: carries_error_prefix(result),
            verifier: is_verification_call(call),
            has_exit,
            exit,
            ..Self::default()
        }
    }

    fn of_error_text(error: &str) -> Self {
        Self {
            empty_marker: error.contains(EMPTY_REPLY_MARKER),
            club_stream: error.contains(CLUB_STREAM_MARKER),
            ..Self::default()
        }
    }

    fn input(&self) -> Value {
        serde_json::json!({
            "error": self.error,
            "verifier": self.verifier,
            "has_exit": self.has_exit,
            "exit": self.exit,
            "empty_marker": self.empty_marker,
            "club_stream": self.club_stream,
        })
    }
}

impl ResultVerdict {
    fn check(&self, features: &ResultFeatures) -> Result<(), String> {
        if self.red_verifier && !self.error {
            return Err("a red verifier run must be an error result".into());
        }
        if self.dispatch_failure && !self.error {
            return Err("a dispatch failure must be an error result".into());
        }
        if self.error && self.dispatch_failure == self.red_verifier {
            return Err("an error result is a red run or a dispatch failure, not both".into());
        }
        if self.empty_reply && !(features.empty_marker || features.club_stream) {
            return Err("an empty reply needs one of its two markers".into());
        }
        Ok(())
    }
}

fn exit_code_class(result: &str) -> (bool, u32) {
    result
        .lines()
        .next()
        .and_then(|header| header.split_once(" failed (exit "))
        .and_then(|(_, rest)| rest.split_once(')'))
        .and_then(|(code, _)| code.parse::<i32>().ok())
        .map_or((false, 0), |code| (true, code as u32))
}

pub(crate) fn carries_error_prefix(result: &str) -> bool {
    result.starts_with(TOOL_ERROR_PREFIX)
}

fn decide(features: &ResultFeatures) -> ResultVerdict {
    let input = features.input();
    live_policy::decide_shipped(RESULT_CLASSIFY_LEG, &input, |verdict: &ResultVerdict| {
        verdict.check(features)
    })
    .0
}

pub(crate) fn is_error_result(s: &str) -> bool {
    let features = ResultFeatures::of_result(s);
    decide(&features).error
}

pub(crate) fn is_red_verifier_run(call: &ToolCall, result: &str) -> bool {
    let features = ResultFeatures::of_call(call, result);
    decide(&features).red_verifier
}

pub(crate) fn is_dispatch_failure(call: &ToolCall, result: &str) -> bool {
    let features = ResultFeatures::of_call(call, result);
    decide(&features).dispatch_failure
}

/// A shell command that ran a test program and came back red, whatever it was wrapped in.
///
/// The strict classifier sees a bare `cargo test` but not `timeout 600 cargo test`, a
/// subshell, a loop or an environment prefix, so a debugging agent's red tests counted as
/// failed calls. Three in a row raised "Every tool call in your last several turns failed",
/// and in a recorded run the agent's next call dropped a design whose values were already
/// right. A red test is a measurement; this keeps it out of the failed-call streak.
pub(crate) fn is_red_test_shell_run(call: &ToolCall, result: &str) -> bool {
    if call.name != "shell" || !carries_error_prefix(result) {
        return false;
    }
    let ran = result
        .lines()
        .next()
        .and_then(|header| header.split_once(" failed (exit "))
        .and_then(|(_, rest)| rest.split_once(')'))
        .and_then(|(code, _)| code.parse::<i32>().ok())
        .is_some_and(|code| code != 126 && code != 127);
    ran && super::verify::mentions_test_run(
        crate::agent::tools::shell::shell_command_arg(&call.args).unwrap_or(""),
    )
}

pub(crate) fn is_context_overflow_error(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    error.contains("http 413")
        || error.contains("context_length_exceeded")
        || error.contains("maximum context length")
        || error.contains("prompt is too long")
        || error.contains("prompt too long")
        || error.contains("payload too large")
        || error.contains("request too large")
        || error.contains("input token count exceeds")
        || error.contains("exceed_context_size_error")
        || ((error.contains("context window") || error.contains("context size"))
            && (error.contains("exceed") || error.contains("too large")))
}

pub(crate) fn is_empty_reply_error(error: &str) -> bool {
    let features = ResultFeatures::of_error_text(error);
    decide(&features).empty_reply
}

pub(crate) fn is_permanent_provider_error(error: &str) -> bool {
    if carries_provider_veto(error)
        || carries_permanent_transport_configuration(error)
        || crate::agent::club::error_indicates_certificate_rejected(error)
    {
        return true;
    }
    if crate::agent::club::error_indicates_auth_failed(error)
        || crate::agent::club::error_indicates_quota_exhausted(error)
        || crate::agent::club::error_indicates_refusal(error)
        || crate::agent::club::error_indicates_gateway_stop(error)
    {
        return true;
    }
    // A reservation refusal is correctable: the turn makes the request
    // smaller and asks again. Calling it permanent would strand a run with
    // credit left. It is never replayed unchanged either: the failure
    // verdict for it says no replay, and the turn's own correction is the
    // only thing that sends again.
    if crate::agent::club::error_indicates_reservation_exceeds(error) {
        return false;
    }
    let lower = error.to_ascii_lowercase();
    rejected_request_text(&lower) || provider_error_status(&lower).is_some_and(is_client_fault)
}

pub(crate) fn carries_provider_veto(error: &str) -> bool {
    error
        .strip_prefix("HTTP ")
        .and_then(|tail| tail.split_once(' '))
        .is_some_and(|(status, detail)| {
            status.len() == 3
                && status.bytes().all(|byte| byte.is_ascii_digit())
                && detail
                    .strip_prefix(crate::agent::club::PROVIDER_RETRY_VETO)
                    .is_some_and(|tail| tail.starts_with(": "))
        })
}

pub(crate) fn carries_permanent_transport_configuration(error: &str) -> bool {
    error
        .strip_prefix("transport error after ")
        .and_then(|tail| tail.split_once(" attempt(s): "))
        .is_some_and(|(attempts, detail)| {
            !attempts.is_empty()
                && attempts.bytes().all(|byte| byte.is_ascii_digit())
                && detail.starts_with(crate::agent::club::PERMANENT_TRANSPORT_CONFIGURATION)
        })
}

pub(crate) fn rejected_request_text(lower: &str) -> bool {
    lower.contains("invalid api key")
        || lower.contains("invalid_api_key")
        || lower.contains("model not found")
        || lower.contains("unknown model")
        || lower.contains("does not exist or you do not have access")
}

pub(crate) fn provider_error_status(lower: &str) -> Option<u16> {
    lower
        .split_once("http ")
        .and_then(|(_, tail)| tail.get(..3))
        .and_then(|digits| digits.parse::<u16>().ok())
}

pub(crate) fn is_client_fault(status: u16) -> bool {
    (400..=499).contains(&status) && !matches!(status, 408 | 425 | 429)
}

pub(crate) fn is_recoverable_stream_error(error: &str) -> bool {
    error.starts_with(crate::agent::club::INCOMPLETE_STREAM_ERR)
}

pub(crate) fn provider_retry_budget() -> Option<usize> {
    std::env::var("ANGEL_PROVIDER_RETRIES")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
}

pub(crate) fn retry_budget_allows(budget: Option<usize>, attempt: usize) -> bool {
    budget.is_none_or(|limit| attempt < limit)
}

pub(crate) fn retryable_provider_failure(
    error: &str,
    emitted_answer: bool,
    incomplete_stream: bool,
) -> bool {
    if is_permanent_provider_error(error) {
        return false;
    }
    !emitted_answer || incomplete_stream
}

pub(crate) fn is_mutation_call(call: &ToolCall) -> bool {
    match call.name.as_str() {
        "write_file" | "str_replace" | "multi_edit" | "apply_patch" | "integrate" => true,
        "code_mode" => call.args.get("allow_effects").and_then(Value::as_bool) == Some(true),
        "fmt" => call.args.get("check").and_then(Value::as_bool) != Some(true),
        "cargo" => {
            let args = call.args.get("args").and_then(Value::as_str).unwrap_or("");
            args.split_whitespace().next() == Some("fmt") && !args.contains("--check")
        }
        _ => false,
    }
}

pub(crate) fn is_dependency_mutation_call(call: &ToolCall) -> bool {
    if call.name != "shell" {
        return false;
    }
    let command = crate::agent::tools::shell::shell_command_arg(&call.args).unwrap_or("");
    dependency_mutation_command(command)
}

pub(crate) fn dependency_mutation_command(command: &str) -> bool {
    const MARKERS: &[&str] = &[
        "go get ",
        "go get\t",
        "go install ",
        "go mod tidy",
        "go mod download",
        "npm install ",
        "npm i ",
        "npm update ",
        "pnpm add ",
        "pnpm update ",
        "yarn add ",
        "yarn upgrade ",
        "cargo add ",
        "cargo update ",
        "cargo upgrade ",
        "pip install ",
        "pip3 install ",
        "uv add ",
        "uv pip install ",
        "poetry add ",
        "bundle update ",
        "composer require ",
    ];
    if MARKERS
        .iter()
        .any(|marker| ascii_contains_ignore_case(command, marker))
    {
        return true;
    }
    command.split(['|', ';', '&', '\n']).any(|seg| {
        let seg = seg.trim();
        seg.eq_ignore_ascii_case("npm install")
            || seg.eq_ignore_ascii_case("npm i")
            || seg.eq_ignore_ascii_case("yarn")
            || seg.eq_ignore_ascii_case("yarn install")
            || seg.eq_ignore_ascii_case("pnpm install")
            || seg.eq_ignore_ascii_case("go mod tidy")
    })
}

pub(crate) fn is_product_mutation_call(call: &ToolCall) -> bool {
    if is_dependency_mutation_call(call) {
        return true;
    }
    if !is_mutation_call(call) {
        return false;
    }
    mutation_call_has_product_path(call)
}

pub(crate) fn mutation_arg_path(args: &Value) -> Option<&str> {
    args.get("path")
        .or_else(|| args.get("file_path"))
        .or_else(|| args.get("file"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

pub(crate) fn mutation_call_has_product_path(call: &ToolCall) -> bool {
    match call.name.as_str() {
        "write_file" | "str_replace" => match mutation_arg_path(&call.args) {
            None => true,
            Some(path) => !is_meta_note_mutation_path(path),
        },
        "multi_edit" => {
            let mut saw = false;
            if let Some(path) = mutation_arg_path(&call.args) {
                saw = true;
                if !is_meta_note_mutation_path(path) {
                    return true;
                }
            }
            if let Some(edits) = call.args.get("edits").and_then(Value::as_array) {
                for edit in edits {
                    if let Some(path) = mutation_arg_path(edit) {
                        saw = true;
                        if !is_meta_note_mutation_path(path) {
                            return true;
                        }
                    }
                }
            }
            !saw
        }
        "apply_patch" => {
            let mut saw = false;
            if let Some(path) = mutation_arg_path(&call.args) {
                saw = true;
                if !is_meta_note_mutation_path(path) {
                    return true;
                }
            }
            let mut product = false;
            crate::knowledge::cut::for_each_mutation_target_path(&call.name, &call.args, |path| {
                saw = true;
                if !is_meta_note_mutation_path(path.trim()) {
                    product = true;
                    return true;
                }
                false
            });
            if product {
                return true;
            }
            !saw
        }
        _ => match mutation_arg_path(&call.args) {
            None => true,
            Some(path) => !is_meta_note_mutation_path(path),
        },
    }
}

#[cfg(test)]
pub(crate) fn mutation_call_paths(call: &ToolCall) -> Vec<String> {
    let path_of = |args: &Value| mutation_arg_path(args).map(str::to_string);
    match call.name.as_str() {
        "write_file" | "str_replace" => path_of(&call.args).into_iter().collect(),
        "apply_patch" => {
            let mut out: Vec<String> = path_of(&call.args).into_iter().collect();
            for p in crate::knowledge::cut::mutation_targets(&call.name, &call.args) {
                let p = p.trim().to_string();
                if !p.is_empty() && !out.iter().any(|e| e == &p) {
                    out.push(p);
                }
            }
            out
        }
        "multi_edit" => {
            let mut out = Vec::new();
            if let Some(p) = path_of(&call.args) {
                out.push(p);
            }
            if let Some(edits) = call.args.get("edits").and_then(Value::as_array) {
                for edit in edits {
                    if let Some(p) = path_of(edit) {
                        out.push(p);
                    }
                }
            }
            out
        }
        _ => path_of(&call.args).into_iter().collect(),
    }
}

pub(crate) fn short_payload_hash(s: &str) -> String {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub(crate) fn payload_object_needs_key_sort(len: usize) -> bool {
    len > 1
}

pub(crate) fn payload_fingerprint(value: &Value) -> String {
    match value {
        Value::String(s) => short_payload_hash(s),
        other => {
            let mut hasher = DefaultHasher::new();
            feed_payload_value(&mut hasher, other);
            format!("{:016x}", hasher.finish())
        }
    }
}

pub(crate) fn feed_payload_value(hasher: &mut DefaultHasher, value: &Value) {
    match value {
        Value::Null => 0u8.hash(hasher),
        Value::Bool(b) => {
            1u8.hash(hasher);
            b.hash(hasher);
        }
        Value::Number(n) => {
            2u8.hash(hasher);
            if let Some(i) = n.as_i64() {
                0u8.hash(hasher);
                i.hash(hasher);
            } else if let Some(u) = n.as_u64() {
                1u8.hash(hasher);
                u.hash(hasher);
            } else {
                2u8.hash(hasher);
                n.to_string().hash(hasher);
            }
        }
        Value::String(s) => {
            3u8.hash(hasher);
            s.hash(hasher);
        }
        Value::Array(items) => {
            4u8.hash(hasher);
            items.len().hash(hasher);
            for item in items {
                feed_payload_value(hasher, item);
            }
        }
        Value::Object(map) => {
            5u8.hash(hasher);
            map.len().hash(hasher);
            if payload_object_needs_key_sort(map.len()) {
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                for key in keys {
                    key.hash(hasher);
                    feed_payload_value(hasher, &map[key]);
                }
            } else {
                for (key, val) in map {
                    key.hash(hasher);
                    feed_payload_value(hasher, val);
                }
            }
        }
    }
}

pub(crate) fn mutation_edit_signatures(call: &ToolCall) -> Vec<String> {
    if !is_mutation_call(call) {
        return Vec::new();
    }
    let path_of = |args: &Value| -> String {
        args.get("path")
            .or_else(|| args.get("file_path"))
            .or_else(|| args.get("file"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string()
    };
    match call.name.as_str() {
        "write_file" => {
            let path = path_of(&call.args);
            let content = call
                .args
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("");
            vec![format!("write|{}|{}", path, short_payload_hash(content))]
        }
        "str_replace" => {
            let path = path_of(&call.args);
            let old = call
                .args
                .get("old")
                .or_else(|| call.args.get("old_string"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let new = call
                .args
                .get("new")
                .or_else(|| call.args.get("new_string"))
                .and_then(Value::as_str)
                .unwrap_or("");
            vec![format!(
                "replace|{}|{}|{}",
                path,
                short_payload_hash(old),
                short_payload_hash(new)
            )]
        }
        "multi_edit" => {
            let top_path = path_of(&call.args);
            let Some(edits) = call.args.get("edits").and_then(Value::as_array) else {
                return vec![format!("multi|{}|empty", top_path)];
            };
            edits
                .iter()
                .map(|edit| {
                    let path = edit
                        .get("path")
                        .or_else(|| edit.get("file_path"))
                        .and_then(Value::as_str)
                        .unwrap_or(top_path.as_str());
                    let old = edit
                        .get("old")
                        .or_else(|| edit.get("old_string"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    let new = edit
                        .get("new")
                        .or_else(|| edit.get("new_string"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    format!(
                        "multi|{}|{}|{}",
                        path,
                        short_payload_hash(old),
                        short_payload_hash(new)
                    )
                })
                .collect()
        }
        "apply_patch" => {
            let patch = call
                .args
                .get("patch")
                .or_else(|| call.args.get("input"))
                .or_else(|| call.args.get("diff"))
                .and_then(Value::as_str)
                .unwrap_or("");
            vec![format!("patch|{}", short_payload_hash(patch))]
        }
        other => vec![format!("{}|{}", other, payload_fingerprint(&call.args))],
    }
}

pub(crate) fn is_peripheral_mutation_path(path: &str) -> bool {
    const HEAD: &[&str] = &["docs", "testdata", "examples"];
    const MID: &[&str] = &[
        "testdata",
        "test-data",
        "fixtures",
        "golden",
        "__snapshots__",
        "docs",
        "documentation",
        "examples",
        "book",
    ];
    HEAD.iter()
        .any(|dir| path_starts_with_dir_ignore_case(path, dir))
        || MID.iter().any(|dir| path_has_dir_ignore_case(path, dir))
        || is_meta_note_mutation_path(path)
}

pub(super) fn board_marker_as_token(hay: &str, marker: &str) -> bool {
    if marker.is_empty() || hay.len() < marker.len() {
        return false;
    }
    if marker.starts_with('/') {
        return path_contains_slash_ignore_case(hay, marker);
    }
    let hay_b = hay.as_bytes();
    let marker_len = marker.len();
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'-';
    let mut start = 0;
    while start + marker_len <= hay_b.len() {
        if let Some(rel) = ascii_find_ignore_case(&hay[start..], marker) {
            let i = start + rel;
            let before_ok = i == 0 || !is_ident(hay_b[i - 1]);
            let after = i + marker_len;
            let after_ok = after >= hay_b.len() || !is_ident(hay_b[after]);
            if before_ok && after_ok {
                return true;
            }
            start = i + 1;
        } else {
            break;
        }
    }
    false
}

pub(crate) fn is_meta_note_mutation_path(path: &str) -> bool {
    if path
        .split(['/', '\\'])
        .filter(|component| component.eq_ignore_ascii_case(".scratch"))
        .nth(1)
        .is_some()
    {
        return true;
    }
    const PATH_MARKERS: &[&str] = &[
        "living_handoff",
        "living-handoff",
        "living handoff",
        "board tip",
        "board_tip",
        "board-tip",
    ];
    if PATH_MARKERS.iter().any(|m| board_marker_as_token(path, m)) {
        return true;
    }
    let base = path.rsplit(['/', '\\']).next().unwrap_or(path);
    const BASE_MARKERS: &[&str] = &[
        "handoff.md",
        "notes.md",
        "note.md",
        "slot.json",
        "submissions.json",
        "handoff",
        "tip.md",
        "board.md",
        "living.md",
        "status.md",
    ];
    if BASE_MARKERS
        .iter()
        .any(|marker| base.eq_ignore_ascii_case(marker))
    {
        return true;
    }
    path_contains_slash_ignore_case(path, "/.angelX/notes/")
        || path_contains_slash_ignore_case(path, "/.angelX/handoff/")
        || path_starts_with_slash_ignore_case(path, ".angelX/notes/")
        || path_starts_with_slash_ignore_case(path, ".angelX/handoff/")
        || path_ends_with_slash_ignore_case(path, "/.angelX/notes")
        || path_ends_with_slash_ignore_case(path, "/.angelX/handoff")
        || path_eq_slash_ignore_case(path, ".angelX/notes")
        || path_eq_slash_ignore_case(path, ".angelX/handoff")
        || path_ends_with_slash_ignore_case(path, "/notes")
        || path.eq_ignore_ascii_case("notes")
}

#[cfg(test)]
pub(crate) fn is_core_mutation_path(path: &str) -> bool {
    if path.trim().is_empty() || is_peripheral_mutation_path(path) {
        return false;
    }
    const HEAD: &[&str] = &["src", "pkg", "lib"];
    const MID: &[&str] = &[
        "src",
        "pkg",
        "lib",
        "core",
        "internal",
        "crates",
        "app",
        "server",
        "client",
        "machinery",
    ];
    HEAD.iter()
        .any(|dir| path_starts_with_dir_ignore_case(path, dir))
        || MID.iter().any(|dir| path_has_dir_ignore_case(path, dir))
}

fn path_starts_with_dir_ignore_case(path: &str, component: &str) -> bool {
    let hay = path.as_bytes();
    let needle = component.as_bytes();
    if needle.is_empty() || hay.len() < needle.len() + 1 {
        return false;
    }
    hay[..needle.len()].eq_ignore_ascii_case(needle)
        && (hay[needle.len()] == b'/' || hay[needle.len()] == b'\\')
}

fn path_has_dir_ignore_case(path: &str, component: &str) -> bool {
    let hay = path.as_bytes();
    let needle = component.as_bytes();
    if needle.is_empty() || hay.len() < needle.len() + 2 {
        return false;
    }
    let is_sep = |b: u8| b == b'/' || b == b'\\';
    hay.windows(needle.len() + 2).any(|window| {
        is_sep(window[0])
            && window[1..needle.len() + 1].eq_ignore_ascii_case(needle)
            && is_sep(window[needle.len() + 1])
    })
}

fn path_slash_eq_ignore_case(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b.iter()).all(|(&x, &y)| {
            let x = if x == b'\\' { b'/' } else { x };
            let y = if y == b'\\' { b'/' } else { y };
            x.eq_ignore_ascii_case(&y)
        })
}

fn path_starts_with_slash_ignore_case(path: &str, prefix: &str) -> bool {
    let hay = path.as_bytes();
    let needle = prefix.as_bytes();
    hay.len() >= needle.len() && path_slash_eq_ignore_case(&hay[..needle.len()], needle)
}

fn path_ends_with_slash_ignore_case(path: &str, suffix: &str) -> bool {
    let hay = path.as_bytes();
    let needle = suffix.as_bytes();
    hay.len() >= needle.len() && path_slash_eq_ignore_case(&hay[hay.len() - needle.len()..], needle)
}

fn path_eq_slash_ignore_case(path: &str, other: &str) -> bool {
    path_slash_eq_ignore_case(path.as_bytes(), other.as_bytes())
}

fn path_contains_slash_ignore_case(path: &str, needle: &str) -> bool {
    let hay = path.as_bytes();
    let needle = needle.as_bytes();
    !needle.is_empty()
        && hay.len() >= needle.len()
        && hay
            .windows(needle.len())
            .any(|window| path_slash_eq_ignore_case(window, needle))
}

pub(crate) fn looks_like_test_source_path(path: &str) -> bool {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    const SUFFIXES: &[&str] = &[
        "_test.go",
        "_test.rs",
        "_test.py",
        ".test.js",
        ".test.ts",
        ".test.mjs",
        ".spec.js",
        ".spec.ts",
    ];
    let name_b = name.as_bytes();
    SUFFIXES.iter().any(|suffix| {
        let suffix = suffix.as_bytes();
        name_b.len() >= suffix.len()
            && name_b[name_b.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
    }) || (name_b.len() >= 5 && name_b[..5].eq_ignore_ascii_case(b"test_"))
        || name.eq_ignore_ascii_case("model_test.go")
        || path_has_dir_ignore_case(path, "test")
        || path_has_dir_ignore_case(path, "tests")
        || path_has_dir_ignore_case(path, "__tests__")
}

fn json_strings_contain_ignore_case(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(text) => ascii_contains_ignore_case(text, needle),
        Value::Array(items) => items
            .iter()
            .any(|item| json_strings_contain_ignore_case(item, needle)),
        Value::Object(map) => map.iter().any(|(key, item)| {
            ascii_contains_ignore_case(key, needle)
                || json_strings_contain_ignore_case(item, needle)
        }),
        _ => false,
    }
}

pub(crate) fn verification_targets_self_authored(
    call: &ToolCall,
    self_authored_basenames: &std::collections::HashSet<String>,
) -> bool {
    if self_authored_basenames.is_empty() {
        return false;
    }
    let mentions = |needle: &str| {
        ascii_contains_ignore_case(&call.name, needle)
            || json_strings_contain_ignore_case(&call.args, needle)
    };
    const SUITE_MARKERS: &[&str] = &[
        "cargo test",
        "go test ./",
        "go test ./...",
        "npm test",
        "pnpm test",
        "yarn test",
        "pytest",
        "mvn test",
        "gradle test",
    ];
    if SUITE_MARKERS.iter().any(|marker| mentions(marker))
        && !self_authored_basenames.iter().any(|base| mentions(base))
    {
        return false;
    }
    self_authored_basenames.iter().any(|base| mentions(base))
}

const NON_CODE_VERIFY_EXTENSIONS: &[&str] = &[
    "md", "markdown", "mdx", "rst", "txt", "text", "adoc", "asciidoc", "org", "log", "csv", "tsv",
];
const NON_CODE_VERIFY_FILENAMES: &[&str] = &[
    "license",
    "licence",
    "notice",
    "authors",
    "contributors",
    "changelog",
    "codeowners",
    "readme",
];

/// in place so mutation hops do not copy every path to hunt `.MD` / `LICENSE`.
pub(crate) fn is_prose_only_path(raw: &str) -> bool {
    let path = Path::new(raw.trim());
    if path.as_os_str().is_empty() {
        return false;
    }
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            NON_CODE_VERIFY_EXTENSIONS
                .iter()
                .any(|ext| extension.eq_ignore_ascii_case(ext))
        })
    {
        return true;
    }
    path.extension().is_none()
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                NON_CODE_VERIFY_FILENAMES
                    .iter()
                    .any(|base| name.eq_ignore_ascii_case(base))
            })
}

#[cfg(test)]
fn native_of_result(result: &str) -> ResultVerdict {
    let error = is_error_result_native(result);
    ResultVerdict {
        error,
        red_verifier: false,
        dispatch_failure: error,
        empty_reply: false,
    }
}

#[cfg(test)]
fn native_of_call(call: &ToolCall, result: &str) -> ResultVerdict {
    ResultVerdict {
        error: is_error_result_native(result),
        red_verifier: is_red_verifier_run_native(call, result),
        dispatch_failure: is_dispatch_failure_native(call, result),
        empty_reply: false,
    }
}

#[cfg(test)]
fn native_of_error_text(error: &str) -> ResultVerdict {
    ResultVerdict {
        error: false,
        red_verifier: false,
        dispatch_failure: false,
        empty_reply: is_empty_reply_error_native(error),
    }
}

#[cfg(test)]
fn is_error_result_native(s: &str) -> bool {
    s.starts_with(TOOL_ERROR_PREFIX)
}

#[cfg(test)]
fn is_red_verifier_run_native(call: &ToolCall, result: &str) -> bool {
    if !is_error_result_native(result) || !is_verification_call(call) {
        return false;
    }
    result
        .lines()
        .next()
        .and_then(|header| header.split_once(" failed (exit "))
        .and_then(|(_, rest)| rest.split_once(')'))
        .and_then(|(code, _)| code.parse::<i32>().ok())
        .is_some_and(|code| code != 126 && code != 127)
}

#[cfg(test)]
fn is_dispatch_failure_native(call: &ToolCall, result: &str) -> bool {
    is_error_result_native(result) && !is_red_verifier_run_native(call, result)
}

#[cfg(test)]
fn is_empty_reply_error_native(error: &str) -> bool {
    error.contains(EMPTY_REPLY_MARKER) || error.contains(CLUB_STREAM_MARKER)
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/result_classify__tests.rs"]
mod tests;
