use super::*;
/// A tool error: the tool itself failed (bad path, missing arg, spawn failure,
/// panic), or a command it ran exited non-zero. The shell and test runners
/// report a red run this way too, so the model cannot read it as success;
/// [`is_red_verifier_run`] separates the two where that matters.
pub(crate) fn is_error_result(s: &str) -> bool {
    s.starts_with("tool error:")
}

/// A verifier that ran and came back red: the tests or the compiler reported,
/// and the process exited with a status (`<runner> failed (exit N)`). That is
/// evidence about the code, not a call that failed to run. Exit 126/127 (not
/// executable, not found) and deaths by signal never reached a verdict.
pub(crate) fn is_red_verifier_run(call: &ToolCall, result: &str) -> bool {
    if !is_error_result(result) || !is_verification_call(call) {
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

/// A call that gave the model nothing to act on: a tool error that is not a
/// red verifier verdict. The consecutive-error breaker counts these. Counting
/// red test runs as well stopped polyglot-v1 rust-decimal (DeepSeek V4.1
/// Flash) at 67 s of 600 s, telling it to fix a path while its tests were
/// simply failing.
pub(crate) fn is_dispatch_failure(call: &ToolCall, result: &str) -> bool {
    is_error_result(result) && !is_red_verifier_run(call, result)
}

/// Provider errors that mean retrying the exact same serialized request cannot
/// succeed. These get one bounded compact-and-rebuild opportunity before the
/// ordinary transport retry policy is considered.
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

/// Provider replies that arrived structurally intact but carried no usable
/// content (HTTP 200, no text, no tool calls). Unlike transport failures these
/// can repeat deterministically on identical bytes from a greedy local backend,
/// so the retry path re-prompts once instead of only replaying the request.
pub(crate) fn is_empty_reply_error(error: &str) -> bool {
    error.contains("empty reply (no text and no tool calls)")
        || error.contains("club stream produced no data")
}

/// Provider failures for which replaying the same request cannot help. The
/// HTTP client already classifies/retries transient statuses; this turn-level
/// fallback must not add two more paid calls for auth, invalid request/model,
/// or exhausted long-window quota failures. Request timeout (408), Too Early
/// (425), and ordinary rate limiting (429) remain transient; auth/quota
/// detectors above can still make a specific 429 permanent.
pub(crate) fn is_permanent_provider_error(error: &str) -> bool {
    if crate::agent::club::error_indicates_auth_failed(error)
        || crate::agent::club::error_indicates_quota_exhausted(error)
    {
        return true;
    }
    let lower = error.to_ascii_lowercase();
    if lower.contains("invalid api key")
        || lower.contains("invalid_api_key")
        || lower.contains("model not found")
        || lower.contains("unknown model")
        || lower.contains("does not exist or you do not have access")
    {
        return true;
    }
    let status = lower
        .split_once("http ")
        .and_then(|(_, tail)| tail.get(..3))
        .and_then(|digits| digits.parse::<u16>().ok());
    status.is_some_and(|status| (400..=499).contains(&status) && !matches!(status, 408 | 425 | 429))
}

/// A stream that died without a terminal event: the server stalled behind
/// keep-alives, a mid-stream read timed out, or the body ended early. Nothing
/// was committed — no tool call ran and any streamed text is speculative — so
/// the same bytes may be replayed after retracting the display. One predicate
/// covers the turn loop and the agent-graph seat so neither can call the same
/// physical fault fatal from one branch and recoverable from another.
pub(crate) fn is_recoverable_stream_error(error: &str) -> bool {
    error.starts_with(crate::agent::club::INCOMPLETE_STREAM_ERR) || error.contains("stream stalled")
}

/// Per-hop provider retry budget. An absent or unparsable
/// `ANGEL_PROVIDER_RETRIES` means unbounded (L01 unattended policy): a routine
/// transient outage must not end a productive turn because an implicit count
/// ran out. An explicit integer keeps the bounded policy, `0` included (one
/// attempt, no retry). Permanent account/config failures never reach the retry
/// loop regardless of this budget.
pub(crate) fn provider_retry_budget() -> Option<usize> {
    std::env::var("ANGEL_PROVIDER_RETRIES")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
}

/// Whether one more provider attempt fits inside `budget` (`None` = unbounded).
pub(crate) fn retry_budget_allows(budget: Option<usize>, attempt: usize) -> bool {
    budget.is_none_or(|limit| attempt < limit)
}

/// Whether an in-hand provider failure may replay the same hop. A permanent
/// account/configuration failure always outranks the stream classification: an
/// error that carries both a rejected credential and stall context ("HTTP 401
/// Unauthorized … stream stalled") is actionable, never a retry loop, and the
/// recoverable spelling must not launder it. Text already streamed is only
/// recoverable inside the incomplete-stream class, where the display is
/// retracted before the replay.
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

/// Source-changing calls for the bounded verify-before-done policy. Shell is
/// deliberately absent: a shell command is opaque, while direct mutation tools
/// have an honest contract. Code mode and integration are conservative because
/// either may write through nested tools or Git.
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

/// Dependency-install / version-bump shells that should disarm first-write
/// starvation (Roll 09 Hugo gold was a minify version bump, not a source edit).
pub(crate) fn is_dependency_mutation_call(call: &ToolCall) -> bool {
    if call.name != "shell" {
        return false;
    }
    let command = crate::agent::tools::shell::shell_command_arg(&call.args).unwrap_or("");
    dependency_mutation_command(command)
}

/// Package-manager argv that mutates the tree. Scans ASCII in place so
/// ordinary shell hops do not copy the command to hunt `npm`/`cargo add`.
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
    // bare `npm install` / `yarn` at end of pipeline still mutates node_modules
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

/// First-write progress: direct product mutations or dependency shells.
/// Meta notes / living-handoff edits do **not** count — they are board
/// bookkeeping, not a candidate change (competition agents were "clearing"
/// the inspection budget by touching LIVING_HANDOFF while the slot still
/// forbade a real edit).
pub(crate) fn is_first_write_progress_call(call: &ToolCall) -> bool {
    if is_dependency_mutation_call(call) {
        return true;
    }
    if !is_mutation_call(call) {
        return false;
    }
    mutation_call_has_product_path(call)
}

/// Borrowed mutation path. Classify hops must not copy this.
pub(crate) fn mutation_arg_path(args: &Value) -> Option<&str> {
    args.get("path")
        .or_else(|| args.get("file_path"))
        .or_else(|| args.get("file"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// First-write progress without allocating `mutation_call_paths`.
/// Unknown / pathless mutation shapes stay historical yes; meta-only paths
/// stay board bookkeeping.
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

/// Paths a mutation tool intends to touch (best-effort for thrash + progress).
#[cfg(test)]
pub(crate) fn mutation_call_paths(call: &ToolCall) -> Vec<String> {
    let path_of = |args: &Value| mutation_arg_path(args).map(str::to_string);
    match call.name.as_str() {
        "write_file" | "str_replace" => path_of(&call.args).into_iter().collect(),
        // apply_patch rarely carries a top-level path — extract targets from the
        // diff/freeform body so meta-only patches (living handoff / board notes)
        // do not count as first-write progress (A4).
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

/// True when an object must sort keys for a stable fingerprint.
/// 0-1 key maps (the default hop) skip that Vec.
pub(crate) fn payload_object_needs_key_sort(len: usize) -> bool {
    len > 1
}

/// Fingerprint a mutation payload without `Value::to_string()`.
/// String bodies hash in place; other JSON shapes walk without Display.
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

pub(super) fn feed_payload_value(hasher: &mut DefaultHasher, value: &Value) {
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
                // 0-1 keys have a unique order — skip the sort Vec on the
                // common read_file / list_dir hop.
                for (key, val) in map {
                    key.hash(hasher);
                    feed_payload_value(hasher, val);
                }
            }
        }
    }
}

/// Stable per-edit identities for thrash detection. Short snippets that only
/// differ by multi_edit batch packing still collide when old/new match.
#[cfg(test)]
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

/// Paths that usually hold generated fixtures rather than the implementing code.
/// Matches ASCII case and `/`/`\` in place so mutation hops do not copy
/// every path to hunt `/docs/` / `testdata/`.
#[cfg(test)]
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

/// True when `marker` appears in `hay` as its own path/token, not as a prefix of
/// a longer identifier (`living_handoff` must not match `living_handoff_parser.rs`).
/// Neighbors may be path separators, extensions (`.md`), or string edges — not
/// ASCII alphanumerics / `_` / `-` that glue compound product paths (A4/A8).
///
/// Absolute path prefixes (`/tmp/living…`) keep substring match so
/// `/tmp/living-handoff.md` still counts while product identifiers do not.
/// ASCII case and `/`/`\` are compared in place — no lowercase copy.
pub(super) fn board_marker_as_token(hay: &str, marker: &str) -> bool {
    if marker.is_empty() || hay.len() < marker.len() {
        return false;
    }
    // Path-prefix markers intentionally cover a directory tree of board digests.
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

/// Handoff / notes / living-board files and nested worktree scratch artifacts.
/// Editing them is bookkeeping, not candidate progress — counting them as
/// first-write progress made competition agents "mutate" a note or placeholder
/// then keep inspecting while the real slot remained unchanged.
///
/// A4: basename bookkeeping files match **exactly** (not `contains`) so product
/// paths like `footnotes.md`, `keynote.md`, or `host-slot.json` still count as
/// real mutations. Angel notes/handoff stores match relative *and* absolute
/// prefixes (`.angel/notes/…` used to under-match without a leading `/`).
///
/// A8: living-board tokens use path-token boundaries so `living_handoff_parser.rs`
/// is a product path, not bookkeeping.
///
/// Matches ASCII case and `/`/`\` in place so mutation hops do not copy
/// every path to hunt `LIVING_HANDOFF` / `.angel/notes`.
pub(crate) fn is_meta_note_mutation_path(path: &str) -> bool {
    // Competition worktrees commonly live below one owner-controlled `.scratch`
    // directory. A second `.scratch` component is the candidate's private dump /
    // probe area, not a product path. Keep the single outer component legal so
    // real edits in `.scratch/worktrees/crown/src/...` (and Mini-style source
    // worktrees directly below `.scratch`) still satisfy first-write.
    if path
        .split(['/', '\\'])
        .filter(|component| component.eq_ignore_ascii_case(".scratch"))
        .nth(1)
        .is_some()
    {
        return true;
    }
    // Distinctive living-board tokens as whole path segments / file stems.
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
    // Exact basenames only — `contains("notes.md")` falsely hit footnotes.md /
    // keynote.md; `contains("slot.json")` hit host-slot.json / dataslot.json.
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
    // Angel notes / handoff store (relative or absolute; with or without slash).
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

/// Paths that typically own product behavior.
/// Matches ASCII case and `/`/`\` in place so mutation hops do not copy
/// every path to hunt `/src/` / `pkg/`.
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

/// True when `path` begins with `component/` or `component\`.
#[cfg(test)]
fn path_starts_with_dir_ignore_case(path: &str, component: &str) -> bool {
    let hay = path.as_bytes();
    let needle = component.as_bytes();
    if needle.is_empty() || hay.len() < needle.len() + 1 {
        return false;
    }
    hay[..needle.len()].eq_ignore_ascii_case(needle)
        && (hay[needle.len()] == b'/' || hay[needle.len()] == b'\\')
}

/// True when `component` sits between `/` or `\` separators (`/test/`,
/// `\tests\`). Scans in place so mutation hops do not copy the path.
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

/// Compare two path slices treating `\` as `/` and ignoring ASCII case.
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

/// Whether a mutated path looks like a test the agent may have invented.
/// Matches ASCII case and `/`/`\` in place so mutation hops do not copy
/// every path to hunt `_test.go` / `/tests/`.
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

/// True when a green verifier command only names basenames the agent authored.
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

/// True when a verifier only names files this turn authored.
/// Scans tool name + arg strings in place so a green `cargo test` hop does
/// not Display-serialize or lowercase the whole argv object.
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
    // Generic full-suite commands are not "only self-authored".
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

/// True when a mutation target is known prose/data. Matches ASCII case
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
