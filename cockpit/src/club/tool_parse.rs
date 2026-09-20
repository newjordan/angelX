//! Truncation marking, tool-call argument repair, prose tool-call recovery,
//! and message JSON encoding.

use super::*;

/// Surfaced when a reply is cut at the provider's output-token cap and the partial
/// text can't be safely used (a fail-closed club, an empty body, or a half-written
/// tool call). SOTA/metered links opt out of the hard error via `keep_truncated`.
pub(crate) const TRUNCATED_OUTPUT_ERR: &str = "club output was cut off by finish_reason=length; raise the model output token \
     budget or make the request smaller";

/// Surfaced when the model produced private reasoning (raw ` ` markers or a
/// `reasoning_content` field) but no visible answer. The caller retries once
/// with [`ANSWER_DIRECTLY_REMINDER`] and then fails closed — a degenerate
/// reasoning loop must not become a silent blank turn or an unbounded resend.
pub(crate) const EMPTY_REPLY_REASONING_ONLY_ERR: &str =
    "club returned an empty reply: the model produced reasoning but no answer";

/// Appended when transport EOF/error arrives before the provider's terminal
/// stream event. Kept prose remains useful, but must never look complete.
pub(crate) const STREAM_INTERRUPTED_SUFFIX: &str =
    "\n\n[response interrupted: provider stream ended before completion]";

/// Injected into the leading system block on the single recovery retry, so
/// strict chat templates keep every system instruction before conversation
/// history. The caller mutates only its cloned request body, never history.
pub(crate) const ANSWER_DIRECTLY_REMINDER: &str = "Your previous turn produced only internal reasoning and no final answer. \
     Skip the reasoning this time: answer the request directly, without any \
     think block.";

/// Tool-turn variant of the recovery reminder. On an agentic turn the
/// reasoning-only stall is almost always a tool call the model thought about
/// but never emitted; telling it to \"answer directly\" here steers it into
/// prose and converts one flaky turn into a no-action turn. Demand the action
/// instead.
pub(crate) const EMIT_TOOL_CALL_REMINDER: &str = "Your previous turn produced only internal reasoning and no action. Do not \
     reason further. Execute the next step NOW as a structured tool call \
     through the tool interface — no prose, no think block. Only if no tool \
     applies, give the final answer directly.";

/// Append a visible marker so a kept-but-truncated draft is never mistaken for a
/// complete answer downstream (the MoA aggregator, the transcript, the user).
pub(crate) fn mark_truncated(text: &str, policy: OutputBudgetPolicy) -> String {
    format!("{}\n\n[{}]", text.trim_end(), policy.incomplete_message())
}

pub(crate) fn mark_stream_interrupted(text: &str) -> String {
    format!("{}{STREAM_INTERRUPTED_SUFFIX}", text.trim_end())
}

/// Serialize our conversation into OpenAI chat-completions message JSON.
/// `pub(crate)` so the harness can reuse it for trajectory logging.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolArgRepairKind {
    FenceStripped,
    Balanced,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ToolArgsParse {
    Exact {
        value: serde_json::Value,
        digest: String,
        length: usize,
    },
    Repaired {
        value: serde_json::Value,
        kind: ToolArgRepairKind,
        digest: String,
        length: usize,
    },
    Unrecoverable {
        digest: String,
        length: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ToolArgRepairRecord {
    pub(crate) call_id: String,
    pub(crate) kind: String,
    pub(crate) digest: String,
    pub(crate) length: usize,
}

const INVALID_TOOL_ARGS_KEY: &str = "__angel_unrecoverable_tool_args";

fn tool_args_digest(raw: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in raw.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    format!("fnv64:{hash:016x}")
}

fn repair_records() -> &'static std::sync::Mutex<std::collections::VecDeque<ToolArgRepairRecord>> {
    static RECORDS: std::sync::OnceLock<
        std::sync::Mutex<std::collections::VecDeque<ToolArgRepairRecord>>,
    > = std::sync::OnceLock::new();
    RECORDS.get_or_init(|| std::sync::Mutex::new(std::collections::VecDeque::new()))
}

pub(crate) fn tool_arg_repair_records() -> Vec<ToolArgRepairRecord> {
    repair_records()
        .lock()
        .map(|records| records.iter().cloned().collect())
        .unwrap_or_default()
}

fn record_tool_args(call_id: &str, parsed: &ToolArgsParse) {
    let (kind, digest, length) = match parsed {
        ToolArgsParse::Exact { .. } => return,
        ToolArgsParse::Repaired {
            kind,
            digest,
            length,
            ..
        } => (format!("{kind:?}"), digest.clone(), *length),
        ToolArgsParse::Unrecoverable { digest, length } => {
            ("Unrecoverable".to_string(), digest.clone(), *length)
        }
    };
    if let Ok(mut records) = repair_records().lock() {
        records.push_back(ToolArgRepairRecord {
            call_id: call_id.to_string(),
            kind,
            digest,
            length,
        });
        while records.len() > 256 {
            records.pop_front();
        }
    }
}

pub(crate) fn parsed_tool_args(call_id: &str, raw: &str) -> serde_json::Value {
    let parsed = repair_tool_args(raw);
    record_tool_args(call_id, &parsed);
    match parsed {
        ToolArgsParse::Exact { value, .. } | ToolArgsParse::Repaired { value, .. } => value,
        ToolArgsParse::Unrecoverable { digest, length } => serde_json::json!({
            INVALID_TOOL_ARGS_KEY: { "digest": digest, "length": length }
        }),
    }
}

pub(crate) fn invalid_tool_args_error(args: &serde_json::Value) -> Option<String> {
    let invalid = args.get(INVALID_TOOL_ARGS_KEY)?;
    Some(format!(
        "unrecoverable tool arguments ({} bytes, {})",
        invalid
            .get("length")
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
        invalid
            .get("digest")
            .and_then(|value| value.as_str())
            .unwrap_or("digest unavailable"),
    ))
}

/// Parse tool-call arguments as exact, conservatively repaired, or explicitly
/// unrecoverable. Empty arguments are an exact empty object; unrecoverable input
/// never becomes `{}` and therefore can never dispatch accidentally.
pub(crate) fn repair_tool_args(raw: &str) -> ToolArgsParse {
    let s = raw.trim();
    let digest = tool_args_digest(raw);
    let length = raw.len();
    if s.is_empty() {
        return ToolArgsParse::Exact {
            value: serde_json::json!({}),
            digest,
            length,
        };
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
        return ToolArgsParse::Exact {
            value: v,
            digest,
            length,
        };
    }
    // Strip ```json … ``` fences a model sometimes wraps args in.
    let s2 = s
        .strip_prefix("```json")
        .or_else(|| s.strip_prefix("```"))
        .unwrap_or(s)
        .trim()
        .strip_suffix("```")
        .unwrap_or(s)
        .trim();
    if s2 != s
        && let Ok(v) = serde_json::from_str::<serde_json::Value>(s2)
    {
        return ToolArgsParse::Repaired {
            value: v,
            kind: ToolArgRepairKind::FenceStripped,
            digest,
            length,
        };
    }
    // Close truncated strings/brackets + strip trailing commas, then retry.
    let fixed = close_and_clean(s2);
    if fixed != s2
        && let Ok(v) = serde_json::from_str::<serde_json::Value>(&fixed)
    {
        return ToolArgsParse::Repaired {
            value: v,
            kind: ToolArgRepairKind::Balanced,
            digest,
            length,
        };
    }
    ToolArgsParse::Unrecoverable { digest, length }
}

/// String-aware repair of a JSON fragment: balances unterminated strings and
/// unclosed `{`/`[` (truncation) and removes trailing commas before a closer or
/// at the end. Conservative — only appends what's needed; never rewrites content.
pub(crate) fn close_and_clean(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    let mut stack: Vec<char> = Vec::new();
    let mut in_str = false;
    let mut esc = false;
    for c in s.chars() {
        if in_str {
            out.push(c);
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_str = true;
                out.push(c);
            }
            '{' => {
                stack.push('}');
                out.push(c);
            }
            '[' => {
                stack.push(']');
                out.push(c);
            }
            '}' | ']' => {
                trim_trailing_comma(&mut out);
                if stack.last().copied() == Some(c) {
                    stack.pop();
                }
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    if in_str {
        out.push('"');
    }
    trim_trailing_comma(&mut out);
    while let Some(close) = stack.pop() {
        out.push(close);
    }
    out
}

/// Drop trailing whitespace then a single trailing `,` from `out` (in place).
pub(crate) fn trim_trailing_comma(out: &mut String) {
    while out.ends_with(|c: char| c.is_whitespace()) {
        out.pop();
    }
    if out.ends_with(',') {
        out.pop();
    }
}

/// Recover tool calls a model emitted in **prose** (the `content`) rather than
/// the structured `tool_calls` field — only from explicit wrappers a genuine
/// answer would never contain (`<tool_call>…</tool_call>`, `[TOOL_CALLS][…]`), so
/// it can't hijack a real final answer. Args go through [`repair_tool_args`].
/// Empty when nothing is found. The fallback for weak / non-jinja models whose
/// calls the server didn't parse into the structured field.
pub(crate) fn extract_prose_tool_calls(content: &str) -> Vec<ToolCall> {
    let mut calls: Vec<ToolCall> = Vec::new();
    // <tool_call> … </tool_call>, possibly several (an unterminated last is ok).
    // The open tag may carry attributes — some models put the tool name there
    // (`<tool_call name="shell">{args}</tool_call>`) with the bare args object
    // as the body.
    let mut rest = content;
    while let Some(start) = rest.find("<tool_call") {
        let after_kw = &rest[start + "<tool_call".len()..];
        // Require `>` or whitespace next, so a longer word in a genuine answer
        // (`<tool_calls>`, `<tool_callback>`) can't open a wrapper.
        let Some((attrs, after_open)) = split_tag_open(after_kw) else {
            rest = after_kw;
            continue;
        };
        let (body, next) = match after_open.find("</tool_call>") {
            Some(end) => (
                &after_open[..end],
                &after_open[end + "</tool_call>".len()..],
            ),
            None => (after_open, ""),
        };
        match tag_attr(attrs, "name") {
            Some(name) if !name.trim().is_empty() => {
                push_named_prose_call(name.trim(), body.trim(), &mut calls)
            }
            _ => push_prose_call(body.trim(), &mut calls),
        }
        rest = next;
        if rest.is_empty() {
            break;
        }
    }
    // <function= shell>{"cmd":"..."}</function> and
    // <function=delegate>{"club":"coder","task":"..."}</function>. Some
    // OpenAI-compatible backends emit this XML-ish tool dialect as streamed
    // content instead of a structured `tool_calls` field. Keep this explicit:
    // only a function envelope with JSON body is executable.
    let mut rest = content;
    while let Some(start) = rest.find("<function=") {
        let after = &rest[start + "<function=".len()..];
        let Some(gt) = after.find('>') else {
            break;
        };
        let raw_name = after[..gt].trim();
        let name: String = raw_name
            .chars()
            .skip_while(|c| c.is_whitespace())
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        let body_start = &after[gt + 1..];
        let (body, next) = match body_start.find("</function>") {
            Some(end) => (&body_start[..end], &body_start[end + "</function>".len()..]),
            None => (body_start, ""),
        };
        if !name.is_empty() {
            push_named_prose_call(&name, body.trim(), &mut calls);
        }
        rest = next;
        if rest.is_empty() {
            break;
        }
    }
    // [TOOL_CALLS] [ {…}, {…} ] (Mistral-style array).
    if let Some(idx) = content.find("[TOOL_CALLS]") {
        let after = content[idx + "[TOOL_CALLS]".len()..].trim();
        if let Ok(serde_json::Value::Array(arr)) = serde_json::from_str::<serde_json::Value>(after)
        {
            for v in &arr {
                push_prose_value(v, &mut calls);
            }
        }
    }
    // <SHELL>{args} — a shouted tag with the bare args object right after it,
    // closing tag optional, prose allowed to follow the object (seen live).
    // The ALL-CAPS requirement is the anti-hijack guard: genuine markup in an
    // answer (HTML, JSX, XML) is lower- or TitleCase.
    let mut rest = content;
    while let Some(lt) = rest.find('<') {
        let after_lt = &rest[lt + 1..];
        rest = after_lt;
        let name_len = after_lt
            .chars()
            .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_')
            .map(char::len_utf8)
            .sum::<usize>();
        if name_len < 2 || !after_lt.starts_with(|c: char| c.is_ascii_uppercase()) {
            continue;
        }
        let name = after_lt[..name_len].to_ascii_lowercase();
        let Some(after_gt) = after_lt[name_len..].strip_prefix('>') else {
            continue;
        };
        let json_start = after_gt.trim_start();
        if !json_start.starts_with('{') {
            continue;
        }
        // Cut at the end of the first JSON value so trailing prose can't
        // poison the parse; a truncated object (stream cut) goes through the
        // usual repair instead.
        let mut it =
            serde_json::Deserializer::from_str(json_start).into_iter::<serde_json::Value>();
        match it.next() {
            Some(Ok(v)) if v.is_object() => {
                calls.push(ToolCall {
                    id: format!("prose_{}", calls.len()),
                    name,
                    args: v,
                });
                rest = &json_start[it.byte_offset()..];
            }
            Some(Err(e)) if e.is_eof() => {
                push_named_prose_call(&name, json_start, &mut calls);
                break;
            }
            _ => {}
        }
    }
    if calls.is_empty() {
        calls.extend(extract_bare_invocation_tool_calls(content));
    }
    calls
}

/// Candidate `{` positions [`scavenge_stranded_tool_calls`] will try to parse.
/// Bounded so a brace-heavy prose answer (code samples, JSON docs) can't turn
/// the scan quadratic.
const SCAVENGE_MAX_CANDIDATES: usize = 64;
const SCAVENGE_NAME_KEYS: [&str; 3] = ["name", "tool", "tool_name"];
const SCAVENGE_ARG_KEYS: [&str; 3] = ["arguments", "args", "parameters"];

/// Recover a tool call stranded in the visible text with an EMPTY structured
/// `tool_calls` array and **no wrapper** — the DeepSeek-reasoner failure mode
/// [`extract_prose_tool_calls`] deliberately won't touch: a bare
/// `{"name":"shell","arguments":{…}}` object sitting in the answer. Without a
/// wrapper the only thing separating a real call from a model *describing* one
/// is shape, so the bar is exact: valid JSON, a name that matches a tool
/// actually offered this request, and an arguments object (a stringified one is
/// the OpenAI wire shape and is parsed once). A missing arguments key, a
/// non-object, or an unoffered name leaves the text as prose. Opt-in at the
/// call site (`ANGEL_TOOLCALL_SCAVENGE`); empty when nothing qualifies.
pub(crate) fn scavenge_stranded_tool_calls(content: &str, offered: &[ToolDef]) -> Vec<ToolCall> {
    if offered.is_empty() {
        return Vec::new();
    }
    let mut calls: Vec<ToolCall> = Vec::new();
    let mut rest = content;
    let mut candidates = 0usize;
    while let Some(open) = rest.find('{') {
        if candidates >= SCAVENGE_MAX_CANDIDATES {
            break;
        }
        candidates += 1;
        let candidate = &rest[open..];
        let mut values =
            serde_json::Deserializer::from_str(candidate).into_iter::<serde_json::Value>();
        match values.next() {
            Some(Ok(value)) => match scavenged_call(&value, offered, calls.len()) {
                Some(call) => {
                    calls.push(call);
                    rest = &candidate[values.byte_offset()..];
                }
                // Advance by one brace rather than past the whole value: a call
                // nested inside an envelope object is still reachable.
                None => rest = &candidate[1..],
            },
            _ => rest = &candidate[1..],
        }
    }
    calls
}

fn scavenged_call(
    value: &serde_json::Value,
    offered: &[ToolDef],
    index: usize,
) -> Option<ToolCall> {
    let object = value.as_object()?;
    let name = SCAVENGE_NAME_KEYS
        .iter()
        .find_map(|key| object.get(*key).and_then(|v| v.as_str()))?
        .trim();
    if !offered.iter().any(|def| def.name == name) {
        return None;
    }
    let args = match SCAVENGE_ARG_KEYS.iter().find_map(|key| object.get(*key))? {
        serde_json::Value::String(raw) => serde_json::from_str::<serde_json::Value>(raw).ok()?,
        other => other.clone(),
    };
    if !args.is_object() {
        return None;
    }
    Some(ToolCall {
        id: format!("scavenge_{index}"),
        name: name.to_string(),
        args,
    })
}

/// True when chat text carries raw tool-call markup **outside** code fences
/// and inline code spans — a wrapper the model should have issued as a
/// structured call. Used by the harness to refuse such text as a final
/// answer: the "tool call" never ran, so anything it claims is invented. Code
/// spans are exempt so an answer genuinely quoting the syntax (docs, this
/// codebase's own parser) is not flagged.
pub(crate) fn contains_raw_tool_markup(content: &str) -> bool {
    let stripped = strip_code_spans(content);
    let lowered = stripped.to_ascii_lowercase();
    if [
        "<tool_call",
        "<function=",
        "</function>",
        "[tool_calls]",
        "<longcat_tool_call",
        "<longcat_arg_key>",
        "<longcat_arg_value>",
        "<parameter=",
    ]
    .iter()
    .any(|m| lowered.contains(m))
    {
        return true;
    }
    // <SHELL>{ — shouted-tag dialect, flagged even when its JSON is too
    // mangled for extract_prose_tool_calls to recover.
    let mut rest = stripped.as_str();
    while let Some(lt) = rest.find('<') {
        let after_lt = &rest[lt + 1..];
        rest = after_lt;
        let name_len = after_lt
            .chars()
            .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_')
            .map(char::len_utf8)
            .sum::<usize>();
        if name_len < 2 || !after_lt.starts_with(|c: char| c.is_ascii_uppercase()) {
            continue;
        }
        if after_lt[name_len..]
            .strip_prefix('>')
            .is_some_and(|t| t.trim_start().starts_with('{'))
        {
            return true;
        }
    }
    false
}

/// Drop ``` fenced blocks and `inline` code spans, keeping everything else.
/// An unterminated fence/span swallows the rest of the text (same reading a
/// markdown renderer gives it).
pub(crate) fn strip_code_spans(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    // Even-indexed segments are outside ``` fences.
    for (i, seg) in content.split("```").enumerate() {
        if i % 2 != 0 {
            continue;
        }
        for (j, span) in seg.split('`').enumerate() {
            if j % 2 == 0 {
                out.push_str(span);
            }
        }
    }
    out
}

/// Parse one `<tool_call>` body (a `{name, arguments}` object, possibly
/// truncated) and append it if it names a tool.
pub(crate) fn push_prose_call(body: &str, calls: &mut Vec<ToolCall>) {
    if push_named_xml_tool_call(body, calls) {
        return;
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        push_prose_value(&v, calls);
    } else {
        let fixed = close_and_clean(body);
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&fixed) {
            push_prose_value(&v, calls);
        }
    }
}

pub(crate) fn push_named_xml_tool_call(body: &str, calls: &mut Vec<ToolCall>) -> bool {
    let Some(name) = xml_tag_text(body, "name") else {
        return false;
    };
    let Some(args) = xml_tag_text(body, "args").or_else(|| xml_tag_text(body, "arguments")) else {
        return false;
    };
    let name = name.trim();
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return false;
    }
    push_named_prose_call(name, args.trim(), calls);
    true
}

pub(crate) fn xml_tag_text<'a>(body: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = body.find(&open)? + open.len();
    let tail = &body[start..];
    let end = tail.find(&close)?;
    Some(&tail[..end])
}

/// Split the remainder of an open tag after its keyword: `Some((attrs, rest))`
/// when it really is an open tag (`>` immediately, or whitespace then
/// attributes then `>`), `None` when the keyword is only a prefix of a longer
/// word or the tag never closes.
pub(crate) fn split_tag_open(after_kw: &str) -> Option<(&str, &str)> {
    match after_kw.chars().next() {
        Some('>') => Some(("", &after_kw[1..])),
        Some(c) if c.is_whitespace() => {
            let gt = after_kw.find('>')?;
            Some((&after_kw[..gt], &after_kw[gt + 1..]))
        }
        _ => None,
    }
}

/// Pull a quoted attribute value (`key="…"` / `key='…'`) out of an open tag's
/// attribute list. Deliberately minimal — the wrappers models emit are simple.
pub(crate) fn tag_attr<'a>(attrs: &'a str, key: &str) -> Option<&'a str> {
    let mut rest = attrs;
    while let Some(pos) = rest.find(key) {
        // Word boundary on the left so `key` can't match inside `filekey`.
        let at_boundary = rest[..pos]
            .chars()
            .next_back()
            .is_none_or(|c| c.is_whitespace());
        let after = &rest[pos + key.len()..];
        if at_boundary && let Some(v) = after.trim_start().strip_prefix('=') {
            let v = v.trim_start();
            for q in ['"', '\''] {
                if let Some(inner) = v.strip_prefix(q) {
                    return inner.find(q).map(|end| &inner[..end]);
                }
            }
            return None;
        }
        rest = after;
    }
    None
}

/// Append an explicitly named prose tool call. The body is the arguments object,
/// not a wrapper object, so it must parse to JSON after the usual truncation
/// repair.
pub(crate) fn push_named_prose_call(name: &str, body: &str, calls: &mut Vec<ToolCall>) {
    if body.trim().is_empty() {
        return;
    }
    let fixed = close_and_clean(body.trim());
    let args = serde_json::from_str::<serde_json::Value>(body)
        .or_else(|_| serde_json::from_str::<serde_json::Value>(&fixed))
        .unwrap_or_else(|_| serde_json::json!({}));
    if args.is_object() {
        calls.push(ToolCall {
            id: format!("prose_{}", calls.len()),
            name: name.to_string(),
            args,
        });
    }
}

/// Recover a response that is *only* a sequence of bare tool invocations, e.g.
/// `delegate("coder", "inspect")self_map({"module":"swarm"})`. This is stricter
/// than the XML/JSON wrappers above: any prose or malformed tail rejects the
/// whole parse so function names in normal answers cannot become tools.
pub(crate) fn extract_bare_invocation_tool_calls(content: &str) -> Vec<ToolCall> {
    let mut rest = content.trim();
    let mut parsed: Vec<ToolCall> = Vec::new();
    while !rest.is_empty() {
        let name_len = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .map(char::len_utf8)
            .sum::<usize>();
        if name_len == 0 {
            return Vec::new();
        }
        let name = &rest[..name_len];
        let after_name = rest[name_len..].trim_start();
        let Some(after_open) = after_name.strip_prefix('(') else {
            return Vec::new();
        };
        let Some((args_src, after_close)) = split_balanced_call_args(after_open) else {
            return Vec::new();
        };
        let Some(args) = bare_invocation_args(name, args_src.trim()) else {
            return Vec::new();
        };
        parsed.push(ToolCall {
            id: format!("prose_{}", parsed.len()),
            name: name.to_string(),
            args,
        });
        rest = after_close.trim_start();
    }
    parsed
}

pub(crate) fn split_balanced_call_args(s: &str) -> Option<(&str, &str)> {
    let mut in_str = false;
    let mut esc = false;
    let mut brace_depth = 0usize;
    let mut bracket_depth = 0usize;
    for (idx, c) in s.char_indices() {
        if in_str {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.checked_sub(1)?,
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.checked_sub(1)?,
            ')' if brace_depth == 0 && bracket_depth == 0 => {
                return Some((&s[..idx], &s[idx + c.len_utf8()..]));
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn bare_invocation_args(name: &str, args_src: &str) -> Option<serde_json::Value> {
    if args_src.starts_with('{') {
        let fixed = close_and_clean(args_src);
        return serde_json::from_str::<serde_json::Value>(args_src)
            .or_else(|_| serde_json::from_str::<serde_json::Value>(&fixed))
            .ok()
            .filter(|v| v.is_object());
    }
    if name == "delegate" {
        let arr = serde_json::from_str::<serde_json::Value>(&format!("[{args_src}]")).ok()?;
        let arr = arr.as_array()?;
        let club = arr.first()?.as_str()?;
        let task = arr.get(1)?.as_str()?;
        return Some(serde_json::json!({ "club": club, "task": task }));
    }
    None
}

/// Append a `{name, arguments}` value as a [`ToolCall`] (arguments may be a JSON
/// string or an inline object). Ignored unless it names a tool.
pub(crate) fn push_prose_value(v: &serde_json::Value, calls: &mut Vec<ToolCall>) {
    let Some(name) = v
        .get("name")
        .and_then(|n| n.as_str())
        .filter(|s| !s.is_empty())
    else {
        return;
    };
    let id = format!("prose_{}", calls.len());
    let args = match v.get("arguments") {
        Some(serde_json::Value::String(s)) => parsed_tool_args(&id, s),
        Some(obj) if obj.is_object() => obj.clone(),
        None => serde_json::json!({}),
        _ => parsed_tool_args(&id, "unrecoverable non-object arguments"),
    };
    calls.push(ToolCall {
        id,
        name: name.to_string(),
        args,
    });
}

pub(crate) fn messages_to_json(messages: &[ChatMsg]) -> Vec<serde_json::Value> {
    use serde_json::json;
    let mut out = Vec::with_capacity(messages.len());
    for m in messages {
        out.push(match m.role {
            ChatRole::System => json!({ "role": "system", "content": m.content }),
            ChatRole::User | ChatRole::Harness => {
                if m.attachments.is_empty() {
                    json!({ "role": "user", "content": m.content })
                } else {
                    // OpenAI multimodal: content becomes [text, …media parts].
                    let mut parts = Vec::with_capacity(1 + m.attachments.len());
                    parts.push(json!({ "type": "text", "text": m.content }));
                    parts.extend(m.attachments.iter().map(Media::to_part));
                    json!({ "role": "user", "content": parts })
                }
            }
            ChatRole::Tool => json!({
                "role": "tool",
                "tool_call_id": m.tool_call_id,
                "content": m.content,
            }),
            ChatRole::Assistant => {
                if m.tool_calls.is_empty() {
                    json!({ "role": "assistant", "content": m.content })
                } else {
                    let mut calls = Vec::with_capacity(m.tool_calls.len());
                    for c in m.tool_calls.iter() {
                        // Prefer the raw string when the model already handed us
                        // one — avoids a serialize→String round-trip on the hot
                        // multi-hop path. Objects/arrays still need to_string().
                        let arguments = match &c.args {
                            serde_json::Value::String(s) => s.clone(),
                            other => other.to_string(),
                        };
                        calls.push(json!({
                            "id": c.id,
                            "type": "function",
                            "function": { "name": c.name, "arguments": arguments },
                        }));
                    }
                    json!({ "role": "assistant", "content": m.content, "tool_calls": calls })
                }
            }
        });
    }
    out
}
