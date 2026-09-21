//! Transcript rendering, in-loop compaction, and auto-recall.

use super::*;

/// Flatten a window of messages into a compact transcript for the summarizer.
///
/// Harness-authored runtime commentary (error/spin/churn/false-start nudges,
/// prefixed with [`TELEMETRY_MARK`]) is skipped entirely: those messages steer
/// the live turn but are transient failure chatter, and letting the summarizer
/// see them turns them into durable "the harness is broken / tool calls keep
/// failing" notes — the exact narrative that later reads as a license to switch
/// to self-repair. They still ride the live tail; they just never get distilled.
pub(crate) fn render_transcript(msgs: &[ChatMsg]) -> String {
    let mut s = String::new();
    for m in msgs {
        if m.role == ChatRole::Harness && m.content.trim_start().starts_with(TELEMETRY_MARK) {
            continue;
        }
        let role = match m.role {
            ChatRole::System => "system",
            ChatRole::User => "user",
            ChatRole::Harness => "harness",
            ChatRole::Assistant => "assistant",
            ChatRole::Tool => "tool",
        };
        if !m.content.is_empty() {
            s.push_str(&format!("[{role}] {}\n", m.content));
        }
        for c in m.tool_calls.iter() {
            s.push_str(&format!("[{role} calls {}] {}\n", c.name, c.args));
        }
    }
    s
}

pub(crate) const TOOL_AGED_MARK: &str = "[tool output elided";
pub(crate) const TOOL_DUPLICATE_MARK: &str = "[duplicate inspection output elided";
pub(crate) const TOOL_ARGUMENT_SHRINK_MARK: &str = "[tool argument elided";
/// Excerpt marker carried only by effect receipts (`… — head: … … tail: …`).
pub(crate) const TOOL_EXCERPT_MARK: &str = " — head: ";
pub(crate) const PROTECTED_SKILL_RESULTS: usize = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct InspectionDedup {
    pub(crate) results: usize,
    pub(crate) bytes_saved: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct InspectionAging {
    pub(crate) results: usize,
    pub(crate) bytes_saved: u64,
    /// Effect receipts rewritten to their excerpt-free form by the second-stage
    /// pass; their saved bytes are already included in `bytes_saved`.
    pub(crate) excerpts_dropped: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ToolArgumentShrink {
    pub(crate) calls: usize,
    pub(crate) strings: usize,
    pub(crate) bytes_saved: u64,
}

fn shrink_argument_value(value: &mut Value, min_bytes: usize, strings: &mut usize) {
    match value {
        Value::String(text)
            if text.len() >= min_bytes && !text.starts_with(TOOL_ARGUMENT_SHRINK_MARK) =>
        {
            let receipt = format!("{TOOL_ARGUMENT_SHRINK_MARK}: {} bytes]", text.len());
            if receipt.len() < text.len() {
                *text = receipt;
                *strings = strings.saturating_add(1);
            }
        }
        Value::Array(items) => {
            for item in items {
                shrink_argument_value(item, min_bytes, strings);
            }
        }
        Value::Object(fields) => {
            for value in fields.values_mut() {
                shrink_argument_value(value, min_bytes, strings);
            }
        }
        _ => {}
    }
}

fn shrink_completed_call_payload(call: &mut ToolCall, min_bytes: usize, strings: &mut usize) {
    match call.name.as_str() {
        "write_file" => {
            if let Some(content) = call.args.get_mut("content") {
                shrink_argument_value(content, min_bytes, strings);
            }
        }
        "str_replace" => {
            for key in ["old", "new"] {
                if let Some(value) = call.args.get_mut(key) {
                    shrink_argument_value(value, min_bytes, strings);
                }
            }
        }
        "multi_edit" => {
            if let Some(edits) = call.args.get_mut("edits") {
                shrink_argument_value(edits, min_bytes, strings);
            }
        }
        // Shell commands and patch bodies are machine evidence: verification
        // classification parses the former and the workspace ledger extracts
        // touched paths from the latter. Unknown tools are likewise preserved
        // until they declare a safe payload schema.
        _ => {}
    }
}

/// Shrink only known edit-payload string leaves in older, successfully completed
/// tool-call arguments. Historical assistant calls must remain paired and
/// syntactically valid for provider APIs, so call IDs, names, object keys,
/// scalar types, and recent calls are retained. Commands, patch bodies, unknown
/// tool schemas, errors, denied actions, and unmatched calls stay byte-identical
/// because their original arguments may still be needed for typed ledger
/// reconstruction or recovery. This adapts Hermes' completed-call argument
/// compaction without making unverifiable assumptions about arbitrary tools.
pub(crate) fn shrink_completed_tool_arguments(
    history: &mut [ChatMsg],
    keep_calls: usize,
    min_bytes: usize,
) -> ToolArgumentShrink {
    let mut pending = HashMap::<String, (usize, usize)>::new();
    let mut completed = Vec::<(usize, usize)>::new();
    for (message_index, message) in history.iter().enumerate() {
        for (call_index, call) in message.tool_calls.iter().enumerate() {
            pending.insert(call.id.clone(), (message_index, call_index));
        }
        if message.role != ChatRole::Tool {
            continue;
        }
        let call = message
            .tool_call_id
            .as_deref()
            .and_then(|call_id| pending.remove(call_id));
        if is_error_result(&message.content) || message.content.starts_with("action capsule denied")
        {
            continue;
        }
        if let Some(call) = call {
            completed.push(call);
        }
    }

    let shrink_count = completed.len().saturating_sub(keep_calls);
    let mut result = ToolArgumentShrink::default();
    for (message_index, call_index) in completed.into_iter().take(shrink_count) {
        let calls = Arc::make_mut(&mut history[message_index].tool_calls);
        let call = &mut calls[call_index];
        let before = call.args.to_string().len();
        let mut strings = 0usize;
        shrink_completed_call_payload(call, min_bytes, &mut strings);
        let after = call.args.to_string().len();
        if after >= before {
            continue;
        }
        result.calls = result.calls.saturating_add(1);
        result.strings = result.strings.saturating_add(strings);
        result.bytes_saved = result
            .bytes_saved
            .saturating_add(before.saturating_sub(after) as u64);
    }
    result
}

pub(crate) fn maybe_shrink_tool_arguments(history: &mut [ChatMsg]) -> ToolArgumentShrink {
    if !env_flag("ANGEL_TOOL_ARGUMENT_SHRINK", true) {
        return ToolArgumentShrink::default();
    }
    let keep_calls = env_usize("ANGEL_TOOL_ARGUMENT_KEEP_CALLS", 4);
    let min_bytes = env_usize("ANGEL_TOOL_ARGUMENT_MIN_BYTES", 1024).max(64);
    shrink_completed_tool_arguments(history, keep_calls, min_bytes)
}

/// Result indices whose full payload carries active, non-reconstructable
/// working state. This is a semantic exemption from ordinary age/size pruning,
/// not an escape from the aggregate context ceiling: the final context-fit pass
/// may still trim these results when a request otherwise cannot fit.
///
/// Keep the latest successful load of each of at most eight skills (OpenCode's
/// protected-skill rule, but bounded) and the latest successful todo snapshot
/// (the current plan, not every historical plan mutation).
pub(crate) fn protected_tool_result_indices(
    history: &[ChatMsg],
) -> std::collections::HashSet<usize> {
    // Resolve calls in protocol order rather than building one global id map:
    // provider-reused call ids must not let a later call relabel an older result.
    let mut pending: HashMap<String, &ToolCall> = HashMap::new();
    let mut completed = Vec::new();
    for (index, message) in history.iter().enumerate() {
        for call in message.tool_calls.iter() {
            pending.insert(call.id.clone(), call);
        }
        if message.role != ChatRole::Tool
            || is_error_result(&message.content)
            || message.content.starts_with("action capsule denied")
        {
            continue;
        }
        if let Some(call) = message
            .tool_call_id
            .as_deref()
            .and_then(|call_id| pending.remove(call_id))
        {
            completed.push((index, call));
        }
    }

    let mut protected = std::collections::HashSet::new();
    let mut skills = std::collections::HashSet::new();
    let mut todo_kept = false;
    let mut handoff_kept = false;
    for (index, call) in completed.into_iter().rev() {
        match call.name.as_str() {
            "skill" if skills.len() < PROTECTED_SKILL_RESULTS => {
                let name = call
                    .args
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_ascii_lowercase();
                if !name.is_empty() && skills.insert(name) {
                    protected.insert(index);
                }
            }
            "todo" if !todo_kept => {
                todo_kept = true;
                protected.insert(index);
            }
            "handoff" if !handoff_kept => {
                handoff_kept = true;
                protected.insert(index);
            }
            _ => {}
        }
    }
    protected
}

/// Opt-in effect-aging identity: only `Footprint::Effect` calls qualify.
/// Writes keep their tiny mutation receipts and inspection reads already have
/// identities, so neither ever routes through here.
fn effect_aging_identity(call: &ToolCall) -> Option<(String, bool)> {
    if !matches!(footprint(&call.name, &call.args, true), Footprint::Effect) {
        return None;
    }
    let head = match call.args.get("command") {
        Some(Value::String(command)) => command
            .replace(['\n', '\r'], " ")
            .chars()
            .take(120)
            .collect::<String>(),
        _ => payload_fingerprint(&call.args),
    };
    Some((format!("{}|{}", call.name, head), true))
}

fn excerpt_head(content: &str, max_bytes: usize) -> String {
    let mut end = max_bytes.min(content.len());
    while end > 0 && !content.is_char_boundary(end) {
        end -= 1;
    }
    content[..end].replace(['\n', '\r'], " ")
}

fn excerpt_tail(content: &str, max_bytes: usize) -> String {
    let mut start = content.len().saturating_sub(max_bytes);
    while start < content.len() && !content.is_char_boundary(start) {
        start += 1;
    }
    content[start..].replace(['\n', '\r'], " ")
}

/// Effect receipt: keeps head+tail excerpts so scores/verdicts stay visible
/// without a re-run, plus a handle when the session store could park the bulk.
fn effect_excerpt_receipt(
    identity: &str,
    bounded_identity: &str,
    original_bytes: usize,
    content: &str,
) -> String {
    let handle = age_receipt_for(identity, original_bytes, content)
        .and_then(|text| {
            text.rsplit_once(" handle=")
                .and_then(|(_, rest)| rest.split_whitespace().next())
                .filter(|handle| handle.starts_with("hnd_"))
                .map(|handle| format!(" handle={handle}"))
        })
        .unwrap_or_default();
    format!(
        "{TOOL_AGED_MARK}: {bounded_identity} ({original_bytes} bytes){handle}{TOOL_EXCERPT_MARK}{} … tail: {} — re-run the tool if needed]",
        excerpt_head(content, 240),
        excerpt_tail(content, 240)
    )
}

/// Rewrite an effect excerpt receipt into its excerpt-free inspection shape,
/// parsing the identity, original byte count, and `handle=` text back out of
/// the receipt itself. The handle store is never re-consulted: the bulk stays
/// parked exactly where the first-stage receipt already put it. Fails closed
/// (returns `None`) on anything that does not parse as one of our receipts.
fn strip_excerpt_receipt(text: &str) -> Option<String> {
    // Durable parking appends provenance after the textual receipt. Preserve
    // that suffix while parsing the excerpt; park_tool_result subsequently
    // verifies the store-authored association for the complete original receipt.
    let (receipt_text, provenance) = match text.rsplit_once(" [trajectory-sha256:") {
        Some((receipt, suffix)) => {
            let digest = suffix.strip_suffix(']')?;
            if digest.len() != 64
                || !digest
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return None;
            }
            (receipt, &text[receipt.len()..])
        }
        None => (text, ""),
    };
    let body = receipt_text.strip_suffix(" — re-run the tool if needed]")?;
    let (prefix, _) = body.split_once(TOOL_EXCERPT_MARK)?;
    let (base, handle) = match prefix.rsplit_once(" handle=") {
        Some((base, rest)) if rest.starts_with("hnd_") && !rest.contains(' ') => {
            (base, format!(" handle={rest}"))
        }
        _ => (prefix, String::new()),
    };
    // Validate the `({n} bytes)` tail before rewriting: an impostor string
    // that merely embeds both markers must not be mangled.
    let bytes_end = base.strip_suffix(')')?;
    let (_, count) = bytes_end.rsplit_once(" (")?;
    count.strip_suffix(" bytes")?.parse::<usize>().ok()?;
    let receipt = format!("{base}{handle}{provenance} — re-run the tool if needed]");
    (receipt.len() < text.len()).then_some(receipt)
}

#[cfg(test)]
include!("../../../../tests/cockpit/harness/compact__standalone_tests.rs");

/// Replace old, reconstructable inspection results with provenance-bearing
/// receipts. Recent protection is expressed in actual assistant tool-call
/// batches plus a token budget, not a count of individual results: a parallel
/// batch therefore cannot evict its siblings from the claimed "hop" tail.
///
/// Only successful read/search results are eligible. Mutation, verifier,
/// effect, error, denial, skill, and todo evidence remains complete for the
/// later semantic compactor or emergency context-fit pass — except under the
/// opt-in `ANGEL_TOOL_AGE_EFFECTS` knob, where old effect results also age
/// into head+tail excerpt receipts.
///
/// Second stage: an effect receipt keeps its excerpt only for the next few
/// hops (scores/verdicts stay visible), then — past `excerpt_hops` tool-call
/// hops behind the newest hop, and outside the same protected windows — is
/// rewritten to the excerpt-free inspection shape keeping its identity, size,
/// and handle text. `excerpt_hops == 0` keeps excerpts forever.
pub(crate) fn age_tool_results(
    history: &mut [ChatMsg],
    keep_hops: usize,
    protect_tokens: usize,
    min_bytes: usize,
    age_effects: bool,
    excerpt_hops: usize,
) -> InspectionAging {
    let _span = super::turn::background::span("aging_ms");
    let result = age_tool_results_measured(
        history,
        keep_hops,
        protect_tokens,
        min_bytes,
        age_effects,
        excerpt_hops,
    );
    super::turn::background::observe(history);
    result
}

fn age_tool_results_measured(
    history: &mut [ChatMsg],
    keep_hops: usize,
    protect_tokens: usize,
    min_bytes: usize,
    age_effects: bool,
    excerpt_hops: usize,
) -> InspectionAging {
    let protected = protected_tool_result_indices(history);
    let mut pending = HashMap::<String, (Option<(String, bool)>, usize)>::new();
    let mut completed = Vec::<(usize, Option<(String, bool)>, usize)>::new();
    let mut tool_hops = 0usize;
    for (index, message) in history.iter().enumerate() {
        if !message.tool_calls.is_empty() {
            let hop = tool_hops;
            tool_hops = tool_hops.saturating_add(1);
            for call in message.tool_calls.iter() {
                let identity = inspection_identities(std::slice::from_ref(call))
                    .into_iter()
                    .next()
                    .map(|identity| (identity, false))
                    .or_else(|| match call.name.as_str() {
                        "skill" => call
                            .args
                            .get("name")
                            .and_then(Value::as_str)
                            .map(|name| format!("skill|{}", name.trim().to_ascii_lowercase()))
                            .map(|identity| (identity, false)),
                        "todo" => Some(("todo|prior-snapshot".to_string(), false)),
                        "handoff" => Some(("handoff|prior-note".to_string(), false)),
                        _ => age_effects.then(|| effect_aging_identity(call)).flatten(),
                    });
                pending.insert(call.id.clone(), (identity, hop));
            }
        }
        if message.role != ChatRole::Tool {
            continue;
        }
        let Some((identity, hop)) = message
            .tool_call_id
            .as_deref()
            .and_then(|call_id| pending.remove(call_id))
        else {
            continue;
        };
        completed.push((index, identity, hop));
    }

    let protected_hop_start = tool_hops.saturating_sub(keep_hops);
    let receipt_hops: Vec<(usize, usize)> = completed
        .iter()
        .map(|(index, _, hop)| (*index, *hop))
        .collect();
    let mut tail_tokens = 0usize;
    let mut age_indices = Vec::<(usize, String, bool)>::new();
    for (index, identity, hop) in completed.into_iter().rev() {
        let content = &history[index].content;
        let estimated_tokens = content.len().saturating_add(3) / 4;
        let inside_token_tail = tail_tokens < protect_tokens;
        tail_tokens = tail_tokens.saturating_add(estimated_tokens);
        if hop >= protected_hop_start
            || inside_token_tail
            || protected.contains(&index)
            || is_error_result(content)
            || content.starts_with("action capsule denied")
            || content.contains(TOOL_AGED_MARK)
            || content.contains(TOOL_DUPLICATE_MARK)
            || content.len() < min_bytes
        {
            continue;
        }
        let Some((identity, is_effect)) = identity else {
            continue;
        };
        age_indices.push((index, identity, is_effect));
    }

    let mut aged = InspectionAging::default();
    for (index, identity, is_effect) in age_indices {
        let original_bytes = history[index].content.len();
        let mut bounded_identity = identity.chars().take(200).collect::<String>();
        if identity.chars().count() > 200 {
            bounded_identity.push('…');
        }
        // Prefer handle-backed receipts when the session store is enabled so
        // bulk remains addressable without re-entering root history by default.
        let receipt = if is_effect {
            effect_excerpt_receipt(
                &identity,
                &bounded_identity,
                original_bytes,
                &history[index].content,
            )
        } else {
            age_receipt_for(&identity, original_bytes, &history[index].content).unwrap_or_else(
                || {
                    format!(
                        "{TOOL_AGED_MARK}: {bounded_identity} ({original_bytes} bytes) — re-run the tool if needed]"
                    )
                },
            )
        };
        if receipt.len() >= original_bytes {
            continue;
        }
        let Ok(receipt) = park_tool_result(&history[index], &receipt, "aging") else {
            continue;
        };
        if receipt.len() >= original_bytes {
            continue;
        }
        aged.results = aged.results.saturating_add(1);
        aged.bytes_saved = aged
            .bytes_saved
            .saturating_add(original_bytes.saturating_sub(receipt.len()) as u64);
        history[index].content = receipt.into();
    }

    // Second stage over messages that already carry an excerpt receipt: drop
    // the excerpt once the receipt is more than `excerpt_hops` tool-call hops
    // behind the newest hop, respecting the same protected windows the first
    // stage used. Receipts without the excerpt marker (inspection receipts,
    // already-stripped ones) parse-fail or match-fail and stay untouched, so a
    // second pass over the same history changes nothing.
    if excerpt_hops > 0 {
        let newest_hop = tool_hops.saturating_sub(1);
        let mut tail_tokens = 0usize;
        for (index, hop) in receipt_hops.into_iter().rev() {
            let receipt_len = history[index].content.len();
            let estimated_tokens = receipt_len.saturating_add(3) / 4;
            let inside_token_tail = tail_tokens < protect_tokens;
            tail_tokens = tail_tokens.saturating_add(estimated_tokens);
            let content = &history[index].content;
            if hop >= protected_hop_start
                || inside_token_tail
                || protected.contains(&index)
                || newest_hop.saturating_sub(hop) <= excerpt_hops
                || !content.contains(TOOL_AGED_MARK)
                || !content.contains(TOOL_EXCERPT_MARK)
            {
                continue;
            }
            let Some(stripped) = strip_excerpt_receipt(content) else {
                continue;
            };
            let Ok(stripped) = park_tool_result(&history[index], &stripped, "excerpt_trim") else {
                continue;
            };
            if stripped.len() >= receipt_len {
                continue;
            }
            aged.excerpts_dropped = aged.excerpts_dropped.saturating_add(1);
            aged.bytes_saved = aged
                .bytes_saved
                .saturating_add(receipt_len.saturating_sub(stripped.len()) as u64);
            history[index].content = stripped.into();
        }
    }
    aged
}

/// Replace older exact copies of the same read/search result with a compact
/// receipt while retaining the newest full result. This adapts Hermes' cheap
/// duplicate-tool-output pruning, but keys by Angel's semantic inspection
/// identity and an effect epoch: a mutation, command, or other effectful call
/// prevents reuse across a possible workspace-state change.
pub(crate) fn dedupe_identical_inspection_results(
    history: &mut [ChatMsg],
    min_bytes: usize,
) -> InspectionDedup {
    let _span = super::turn::background::span("aging_ms");
    let result = dedupe_identical_inspection_results_measured(history, min_bytes);
    super::turn::background::observe(history);
    result
}

fn dedupe_identical_inspection_results_measured(
    history: &mut [ChatMsg],
    min_bytes: usize,
) -> InspectionDedup {
    let protected = protected_tool_result_indices(history);
    let mut pending: HashMap<String, (String, usize)> = HashMap::new();
    let mut completed = Vec::<(usize, String, usize)>::new();
    let mut effect_epoch = 0usize;

    for (index, message) in history.iter().enumerate() {
        if !message.tool_calls.is_empty() {
            let identities = message
                .tool_calls
                .iter()
                .map(|call| {
                    inspection_identities(std::slice::from_ref(call))
                        .into_iter()
                        .next()
                })
                .collect::<Vec<_>>();
            if identities.iter().any(Option::is_none) {
                effect_epoch = effect_epoch.saturating_add(1);
            }
            for (call, identity) in message.tool_calls.iter().zip(identities) {
                if let Some(identity) = identity {
                    pending.insert(call.id.clone(), (identity, effect_epoch));
                }
            }
        }
        if message.role != ChatRole::Tool
            || is_error_result(&message.content)
            || message.content.starts_with("action capsule denied")
        {
            continue;
        }
        if let Some((identity, epoch)) = message
            .tool_call_id
            .as_deref()
            .and_then(|call_id| pending.remove(call_id))
        {
            completed.push((index, identity, epoch));
        }
    }

    let mut newest = HashMap::<(usize, String, u64, usize), usize>::new();
    let mut dedup = InspectionDedup::default();
    for (index, identity, epoch) in completed.into_iter().rev() {
        if protected.contains(&index) {
            continue;
        }
        let content = &history[index].content;
        if content.len() < min_bytes
            || content.contains(TOOL_AGED_MARK)
            || content.contains(TOOL_DUPLICATE_MARK)
        {
            continue;
        }
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        let key = (epoch, identity, hasher.finish(), content.len());
        let Some(&newer_index) = newest.get(&key) else {
            newest.insert(key, index);
            continue;
        };
        if history[newer_index].content != *content {
            // Hash collision: fail closed by retaining both full outputs.
            continue;
        }
        let original_bytes = content.len();
        let newer_call = history[newer_index]
            .tool_call_id
            .as_deref()
            .unwrap_or("newer call");
        let receipt = format!(
            "{TOOL_DUPLICATE_MARK} ({original_bytes} bytes) — newest identical result retained at {newer_call}]"
        );
        if receipt.len() >= original_bytes {
            continue;
        }
        dedup.results = dedup.results.saturating_add(1);
        dedup.bytes_saved = dedup
            .bytes_saved
            .saturating_add(original_bytes.saturating_sub(receipt.len()) as u64);
        history[index].content = receipt.into();
    }
    dedup
}

pub(crate) fn maybe_dedupe_inspection_results(history: &mut [ChatMsg]) -> InspectionDedup {
    if !env_flag("ANGEL_TOOL_RESULT_DEDUP", true) {
        return InspectionDedup::default();
    }
    let min_bytes = env_usize("ANGEL_TOOL_RESULT_DEDUP_MIN_BYTES", 200);
    dedupe_identical_inspection_results(history, min_bytes)
}

/// Whether the serving club runs in **cache-stable** mode: the request prefix is
/// kept byte-identical as a turn grows, so a provider's automatic prefix cache
/// keeps hitting instead of being invalidated from the first rewritten message
/// on. DeepSeek's cache is byte-exact and bills a hit at a small fraction of a
/// miss, which makes the hit rate a property of *this* client — every in-place
/// history rewrite pays for the whole prefix again.
///
/// Default on, including unknown providers: cache capability discovery must
/// not decide whether previously sent bytes may be rewritten. Explicit global
/// and per-club pins retain the old token-first ablation. Deferred rewrites run
/// at compaction boundaries, never merely because an operator turn ended.
pub(crate) fn cache_stable_mode(club: &dyn Club) -> bool {
    let club_pin = club
        .env_namespace()
        .and_then(|up| std::env::var(format!("ANGEL_{up}_CACHE_STABLE")).ok())
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .map(|v| !matches!(v.as_str(), "0" | "off" | "false" | "no"));
    club_pin.unwrap_or_else(|| {
        // The global pin is launch config; unknown providers also default on.
        cache_stable_global_pin().unwrap_or(true)
    })
}

/// `Some(on/off)` when `ANGEL_CACHE_STABLE` is set; `None` when unset so the
/// prefix-preserving default applies.
fn cache_stable_global_pin() -> Option<bool> {
    #[cfg(not(test))]
    {
        static PIN: std::sync::OnceLock<Option<bool>> = std::sync::OnceLock::new();
        *PIN.get_or_init(cache_stable_global_pin_from_env)
    }
    #[cfg(test)]
    cache_stable_global_pin_from_env()
}

fn cache_stable_global_pin_from_env() -> Option<bool> {
    std::env::var("ANGEL_CACHE_STABLE")
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .map(|v| !matches!(v.as_str(), "0" | "off" | "false" | "no"))
}

/// Per-call ceiling for model-backed automatic compaction. Keep the dedicated
/// summarizer's configured limit, but never let one call consume more than half
/// of the in-hand route's total compaction budget.
fn auto_compact_chunk_threshold(budget: usize) -> usize {
    crate::agent::compaction::configured_compact_chunk_tokens().min((budget / 2).max(1_024))
}

/// Called only after the caller has rebuilt the request prefix by compaction.
/// Append a bounded receipt after the retained tail; never rewrite the marker.
pub(crate) fn age_tool_results_at_boundary(history: &mut Vec<ChatMsg>) -> InspectionAging {
    let mut aged = maybe_age_tool_results(history);
    if aged.results > 0 || aged.excerpts_dropped > 0 {
        let marker = format!(
            "[tool-aging boundary: aged {} result(s), dropped {} excerpt(s), saved {} payload bytes]",
            aged.results, aged.excerpts_dropped, aged.bytes_saved,
        );
        // The per-turn savings account for this appended receipt as well.
        if aged.bytes_saved > marker.len() as u64 {
            aged.bytes_saved -= marker.len() as u64;
            history.push(ChatMsg::harness(marker));
        }
    }
    aged
}

pub(crate) fn maybe_age_tool_results(history: &mut [ChatMsg]) -> InspectionAging {
    if !env_flag("ANGEL_TOOL_AGING", true) {
        return InspectionAging::default();
    }
    let keep_hops = env_usize("ANGEL_TOOL_AGE_KEEP_HOPS", 4);
    let protect_tokens = env_usize("ANGEL_TOOL_AGE_PROTECT_TOKENS", 8_000);
    let min_bytes = env_usize("ANGEL_TOOL_AGE_MIN_BYTES", 512).max(128);
    let age_effects = env_flag("ANGEL_TOOL_AGE_EFFECTS", false);
    let excerpt_hops = env_usize("ANGEL_TOOL_AGE_EXCERPT_HOPS", 12);
    age_tool_results(
        history,
        keep_hops,
        protect_tokens,
        min_bytes,
        age_effects,
        excerpt_hops,
    )
}

/// File a club's synthesized answer to the long-form palace as a curated report,
/// off-thread (fire-and-forget, like compaction's deposits) so a slow MemPalace
/// never adds latency to the turn's return. No-op unless the club opts in
/// ([`Club::reports_to_palace`] — only the swarm does), the store is live,
/// `ANGEL_SWARM_PALACE` is enabled (default on), and the answer is substantive.
///
/// Routing goes through the [`crate::knowledge::librarian::Librarian`] — the gate that will
/// eventually dedup / score / split reports before they reach the store. That
/// curation is still a pass-through today, so this currently files **one drawer
/// per qualifying turn, verbatim**. That's deliberate and bounded: the flood the
/// Librarian was created to prevent is filing every *worker draft*; here only the
/// final synthesis (one per turn) is filed.
pub(crate) fn file_report(
    club: &dyn Club,
    workspace: &std::path::Path,
    history: &[ChatMsg],
    answer: &str,
    store: &Arc<dyn crate::knowledge::memory::store::MemoryStore>,
    session_id: &str,
) {
    if !club.reports_to_palace() || !store.is_live() || !env_flag("ANGEL_SWARM_PALACE", true) {
        return;
    }
    let body = answer.trim();
    // Skip thin answers (acks, one-liners) — not worth a drawer.
    if body.chars().count() < 80 {
        return;
    }
    let report = crate::knowledge::librarian::Report {
        wing: crate::agent::compaction::project_wing_for(workspace),
        topic: report_topic(history),
        body: body.to_string(),
        source: crate::agent::compaction::provenance(session_id, "swarm"),
    };
    let store = Arc::clone(store);
    enqueue_memory_write(MemoryWriteJob::Report { store, report });
}

const MEMORY_WRITE_QUEUE_CAPACITY: usize = 8;

enum MemoryWriteJob {
    Report {
        store: Arc<dyn crate::knowledge::memory::store::MemoryStore>,
        report: crate::knowledge::librarian::Report,
    },
    Drawers {
        store: Arc<dyn crate::knowledge::memory::store::MemoryStore>,
        drawers: Vec<crate::knowledge::memory::store::Drawer>,
        completion: Option<Box<dyn FnOnce(MemoryWriteSummary) + Send + 'static>>,
    },
}

pub(crate) struct MemoryWriteSummary {
    pub(crate) attempted: usize,
    pub(crate) filed: usize,
    pub(crate) first_error: Option<String>,
}

impl MemoryWriteJob {
    fn run(self) {
        match self {
            Self::Report { store, report } => {
                let _ = crate::knowledge::librarian::Librarian::new(store).file(&report);
            }
            Self::Drawers {
                store,
                drawers,
                completion,
            } => {
                let attempted = drawers.len();
                let mut filed = 0usize;
                let mut first_error = None;
                for drawer in &drawers {
                    match crate::ui::term::catch_background_unwind(|| store.deposit(drawer)) {
                        Ok(Ok(_)) => filed = filed.saturating_add(1),
                        Ok(Err(error)) if first_error.is_none() => first_error = Some(error),
                        Ok(Err(_)) => {}
                        Err(_) if first_error.is_none() => {
                            first_error = Some(
                                "memory store panicked while filing a compacted note".to_string(),
                            );
                        }
                        Err(_) => {}
                    }
                }
                if let Some(completion) = completion {
                    completion(MemoryWriteSummary {
                        attempted,
                        filed,
                        first_error,
                    });
                }
            }
        }
    }

    fn reject(self, error: &'static str) {
        if let Self::Drawers {
            drawers,
            completion: Some(completion),
            ..
        } = self
        {
            let _ = crate::ui::term::catch_background_unwind(|| {
                completion(MemoryWriteSummary {
                    attempted: drawers.len(),
                    filed: 0,
                    first_error: Some(error.to_string()),
                });
            });
        }
    }
}

fn spawn_memory_write_queue(capacity: usize) -> std::sync::mpsc::SyncSender<MemoryWriteJob> {
    let (tx, rx) = mpsc::sync_channel::<MemoryWriteJob>(capacity);
    let _ = std::thread::Builder::new()
        .name("palace-writes".to_string())
        .spawn(move || {
            while let Ok(job) = rx.recv() {
                // A third-party store may panic as well as block. Contain a
                // panic per job so later queued persistence still gets a turn.
                let _ = crate::ui::term::catch_background_unwind(|| job.run());
            }
        });
    tx
}

fn try_enqueue_memory_write(
    queue: &std::sync::mpsc::SyncSender<MemoryWriteJob>,
    job: MemoryWriteJob,
) -> bool {
    match queue.try_send(job) {
        Ok(()) => true,
        Err(std::sync::mpsc::TrySendError::Full(job)) => {
            job.reject("memory filing queue is full; compacted notes were not filed");
            false
        }
        Err(std::sync::mpsc::TrySendError::Disconnected(job)) => {
            job.reject("memory filing worker is unavailable; compacted notes were not filed");
            false
        }
    }
}

/// Best-effort asynchronous persistence through one bounded process-wide
/// worker. If a store hangs, at most the active job plus eight queued batches
/// remain; later reports are dropped instead of leaking one thread per turn.
fn enqueue_memory_write(job: MemoryWriteJob) {
    static QUEUE: std::sync::OnceLock<std::sync::mpsc::SyncSender<MemoryWriteJob>> =
        std::sync::OnceLock::new();
    let queue = QUEUE.get_or_init(|| spawn_memory_write_queue(MEMORY_WRITE_QUEUE_CAPACITY));
    let _ = try_enqueue_memory_write(queue, job);
}

/// Queue a detached drawer batch on the same bounded process-wide worker used
/// by automatic compaction and swarm reports, then return one completion
/// summary without keeping the caller's foreground flight slot occupied.
pub(crate) fn enqueue_memory_drawers_with_feedback<F>(
    store: Arc<dyn crate::knowledge::memory::store::MemoryStore>,
    drawers: Vec<crate::knowledge::memory::store::Drawer>,
    completion: F,
) where
    F: FnOnce(MemoryWriteSummary) + Send + 'static,
{
    enqueue_memory_write(MemoryWriteJob::Drawers {
        store,
        drawers,
        completion: Some(Box::new(completion)),
    });
}

/// A short room/topic for a swarm report: the latest user ask, first non-empty
/// line, truncated. Generic fallback when there's no user turn.
pub(crate) fn report_topic(history: &[ChatMsg]) -> String {
    let ask = history
        .iter()
        .rev()
        .find(|m| m.role == ChatRole::User)
        .map(|m| m.content.as_ref())
        .unwrap_or("");
    let line = ask
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    if line.is_empty() {
        return "Swarm synthesis".to_string();
    }
    let mut topic: String = line.chars().take(64).collect();
    if line.chars().count() > 64 {
        topic.push('…');
    }
    topic
}

/// Resolve the effective auto-compaction budget for the in-hand club.
///
/// An explicit `ANGEL_CONTEXT_BUDGET_TOKENS` always wins. Otherwise, angelX uses
/// a 333k target budget and caps it to the model's reported context window with
/// ~20% headroom when that metadata is available. `ANGEL_CONTEXT_SOFT_CAP`
/// overrides the 333k target; `ANGEL_NO_AUTOCOMPACT=1` forces compaction off.
///
/// SOTA links budget for request volume as well as context fit (the 120k
/// `ANGEL_SOTA_CONTEXT_BUDGET` target): the stateless chat API re-sends the whole
/// history on every hop. Provider cache hits make that resend cheaper, but the
/// tokens are still transmitted, counted, and billed, so cache capability does
/// not raise the default budget.
///
/// Operators who deliberately prefer cache-hit economics over request volume can
/// set `ANGEL_SOTA_CACHE_BUDGET_LIFT=1`. A byte-exact provider prefix cache
/// (`Club::prompt_cache_capable`) running in cache-stable mode then lifts back to
/// the base 333k target, still capped by the model's real window −20%. An
/// explicitly set `ANGEL_SOTA_CONTEXT_BUDGET` wins over the lift. The first lift
/// per session queues a spoken notice (drained at the next compaction boundary)
/// — a budget that moved must say so.
pub(crate) fn compaction_budget(club: &dyn Club, env_budget: usize) -> usize {
    if std::env::var("ANGEL_NO_AUTOCOMPACT")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        return 0;
    }
    if env_budget > 0 {
        return env_budget;
    }
    let base_target = env_usize("ANGEL_CONTEXT_SOFT_CAP", DEFAULT_CONTEXT_BUDGET_TOKENS);
    let sota_target = env_usize(
        "ANGEL_SOTA_CONTEXT_BUDGET",
        DEFAULT_SOTA_CONTEXT_BUDGET_TOKENS,
    );
    let is_sota = crate::agent::club::is_sota_label(club.label());
    // The cache-first lift only applies where it can raise the budget: an
    // operator-set ANGEL_SOTA_CONTEXT_BUDGET is an explicit cost order, and a
    // soft cap at/below the SOTA target already binds tighter than the lift.
    let sota_budget_pinned = std::env::var("ANGEL_SOTA_CONTEXT_BUDGET")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);
    let cache_lift = is_sota
        && sota_target > 0
        && !sota_budget_pinned
        && base_target > sota_target
        && env_flag("ANGEL_SOTA_CACHE_BUDGET_LIFT", false)
        && club.prompt_cache_capable()
        && cache_stable_mode(club);
    let target_budget = if is_sota && sota_target > 0 && !cache_lift {
        sota_target
    } else {
        base_target
    };
    if target_budget == 0 {
        return 0;
    }
    let model_budget = club
        .metadata()
        .map(|m| m.context_window)
        .filter(|&c| c > 0)
        .map(|c| c.saturating_sub(c / 5).max(2048))
        .unwrap_or(target_budget);
    let effective = target_budget.min(model_budget);
    if cache_lift {
        // A small model window can cap the lifted target back to (or below) the
        // old cost-sized value — only an actual raise is worth announcing.
        let legacy = sota_target.min(model_budget);
        if effective > legacy {
            note_cache_budget_lift(club.label(), legacy, effective);
        }
    }
    effective
}

/// One-shot pending notice for the cache-first budget lift. `compaction_budget`
/// is a pure resolver with no event channel, so the first qualifying lift stashes
/// its message here and the next compaction boundary with an event sender speaks
/// it ([`speak_cache_budget_lift`]). Spoken at most once per session.
static CACHE_BUDGET_LIFT_SPOKEN: AtomicBool = AtomicBool::new(false);
static CACHE_BUDGET_LIFT_NOTICE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn note_cache_budget_lift(label: &str, from: usize, to: usize) {
    if CACHE_BUDGET_LIFT_SPOKEN.swap(true, Ordering::AcqRel) {
        return;
    }
    *CACHE_BUDGET_LIFT_NOTICE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(format!(
        "cache-first: compaction budget {}k -> {}k on {label}",
        from / 1000,
        to / 1000
    ));
}

/// Drain and speak the pending cache-first budget-lift notice, if any. Called at
/// the turn loop's compaction boundaries (sync + background), which run on every
/// hop — so the notice lands on the first hop after the lifted budget resolves.
pub(crate) fn speak_cache_budget_lift(events: &mpsc::Sender<TurnEvent>) {
    let pending = CACHE_BUDGET_LIFT_NOTICE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();
    if let Some(msg) = pending {
        let _ = events.send(TurnEvent::Notice(msg));
    }
}

#[cfg(test)]
pub(crate) fn reset_cache_budget_lift_notice_for_test() {
    CACHE_BUDGET_LIFT_SPOKEN.store(false, Ordering::Release);
    CACHE_BUDGET_LIFT_NOTICE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();
}

pub(crate) fn recall_query_key(wing: &str, query: &str) -> u64 {
    let mut h = DefaultHasher::new();
    wing.hash(&mut h);
    query.hash(&mut h);
    let key = h.finish();
    if key == 0 { 1 } else { key }
}

/// Whether the in-hand club is the wrong place for bulk summarization: a
/// token-metered SOTA link (every compaction chunk is a paid API call) or a
/// fan-out pipeline like the swarm (every chunk becomes a whole MoA run).
/// Plain local models summarize in-hand as before. `ANGEL_COMPACT_LOCAL=0`
/// disables the local-first rerouting entirely.
pub(crate) fn wants_cheap_summarizer(club: &dyn Club) -> bool {
    env_flag("ANGEL_COMPACT_LOCAL", true)
        && (crate::agent::club::is_sota_label(club.label()) || club.reports_to_palace())
}

/// First reachable local utility club, if any — the compaction summarizer of
/// choice when the in-hand driver is expensive (see [`wants_cheap_summarizer`]).
/// Bag order puts the fast MoE (gemma) first on the standard fleet.
pub(crate) fn pick_local_summarizer(aux: &[Arc<dyn Club>]) -> Option<Arc<dyn Club>> {
    aux.iter().find(|c| c.is_available()).cloned()
}

/// Resolve the summarizer for a compaction pass, cheapest capable option first:
/// an explicit `ANGEL_COMPACT_URL` endpoint, else a reachable local fleet model
/// when the in-hand club is a paid SOTA link or a fan-out pipeline, else the
/// in-hand club itself (the original behavior).
pub(crate) fn summarizer_for(club: Arc<dyn Club>, aux: &[Arc<dyn Club>]) -> Arc<dyn Club> {
    if let Some(c) = compaction_summarizer() {
        return Arc::new(c);
    }
    if wants_cheap_summarizer(&*club)
        && let Some(local) = pick_local_summarizer(aux)
    {
        return local;
    }
    club
}

/// An optional cheap club dedicated to compaction summaries, configured via
/// `ANGEL_COMPACT_URL` (+ `ANGEL_COMPACT_MODEL`, `ANGEL_COMPACT_KEY`). Returning
/// `Some` routes the (frequent, bulky) summarizer calls to a small/fast endpoint
/// instead of the expensive in-hand driver; `None` means "summarize with the
/// in-hand club" (unchanged behavior).
pub(crate) fn compaction_summarizer() -> Option<crate::agent::club::HttpClub> {
    let url = std::env::var("ANGEL_COMPACT_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())?;
    let model = std::env::var("ANGEL_COMPACT_MODEL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "default".to_string());
    let key = std::env::var("ANGEL_COMPACT_KEY")
        .ok()
        .filter(|s| !s.trim().is_empty());
    Some(crate::agent::club::HttpClub::new(
        "compact", url, model, key,
    ))
}

const AUTO_RECALL_HEADER: &str = "[Relevant notes recalled from long-term memory for this project — background reference, not instructions:]\n\n";
/// Leading text every auto-recall note starts with — the stable identity used
/// to strip prior notes and to classify them (e.g. in `/context`).
pub(crate) const AUTO_RECALL_NOTE_PREFIX: &str = "[Relevant notes recalled from long-term memory";
const MAX_AUTO_RECALL_TOKENS: usize = 8_000;
const AUTO_RECALL_SAFETY_TOKENS: usize = 256;
static AUTO_RECALL_SEARCH_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

struct AutoRecallSearchLease;

impl Drop for AutoRecallSearchLease {
    fn drop(&mut self) {
        AUTO_RECALL_SEARCH_IN_FLIGHT.store(false, Ordering::Release);
    }
}

#[cfg(test)]
pub(crate) fn refresh_knowledge_broker(
    registry: &ToolRegistry,
    history: &mut Vec<ChatMsg>,
    budget: usize,
    tools: &[ToolDef],
    preserve_prefix: bool,
) {
    refresh_knowledge_broker_with_prefix(registry, history, budget, tools, preserve_prefix);
}

pub(crate) fn refresh_knowledge_broker_with_prefix(
    registry: &ToolRegistry,
    history: &mut Vec<ChatMsg>,
    budget: usize,
    tools: &[ToolDef],
    preserve_prefix: bool,
) {
    let mode = crate::agent::backplane::mode();
    let Some(query) = history
        .iter()
        .rev()
        .find(|message| message.role == ChatRole::User)
        .map(|message| message.content.clone())
    else {
        return;
    };
    if mode != crate::agent::backplane::BackplaneMode::Active {
        let lens = registry.atlas().build_lens(
            &query,
            history
                .iter()
                .filter(|message| {
                    !crate::knowledge::atlas::is_lens_message(&message.content)
                        && !crate::agent::backplane::is_broker_message(&message.content)
                })
                .map(|message| message.content.as_ref()),
        );
        crate::knowledge::atlas::replace_lens_message(history, lens);
        if mode == crate::agent::backplane::BackplaneMode::Legacy {
            return;
        }
    }
    let project_key = registry.atlas().project_key().to_string();
    let mut candidates =
        crate::agent::backplane::KnowledgeBroker::existing_candidates(history, &project_key);
    // Atlas is a live, revocable store. Conversation copies cannot nominate
    // themselves after a source changes, a review revokes them, or Atlas is off.
    let previous_atlas = history
        .iter()
        .filter(|message| message.role == ChatRole::Harness)
        .flat_map(|message| {
            if crate::knowledge::atlas::is_lens_message(&message.content) {
                crate::agent::backplane::atlas_lens_candidates(&message.content, &project_key)
            } else {
                crate::agent::backplane::KnowledgeBroker::existing_candidates(
                    std::slice::from_ref(message),
                    &project_key,
                )
            }
        })
        .filter(|item| item.source_id.starts_with("atlas:"))
        .map(|item| (item.source_id, item.digest))
        .collect::<Vec<_>>();
    candidates.retain(|item| !item.source_id.starts_with("atlas:"));
    candidates.extend(
        crate::knowledge::memory::load_for(registry.current_workspace())
            .into_iter()
            .enumerate()
            .map(|(index, memory)| {
                crate::agent::backplane::KnowledgeCandidate::new(
                    format!("memory:{index}"),
                    &project_key,
                    "operator-memory",
                    crate::agent::backplane::KnowledgeAuthority::OperatorApproved,
                    memory,
                )
            }),
    );
    let dossier = crate::knowledge::dossier::broker_context_block(registry.current_workspace());
    if !dossier.trim().is_empty() {
        candidates.push(crate::agent::backplane::KnowledgeCandidate::new(
            format!(
                "dossier:{}",
                &crate::knowledge::cut::sha256_hex(dossier.as_bytes())[..16]
            ),
            &project_key,
            "verified-dossier",
            crate::agent::backplane::KnowledgeAuthority::VerifiedDossier,
            dossier,
        ));
    }
    let caddy = crate::knowledge::caddy::render_card_for_task(
        registry.current_workspace(),
        crate::knowledge::caddy::card_cap(),
    );
    if !caddy.trim().is_empty() {
        candidates.push(crate::agent::backplane::KnowledgeCandidate::new(
            format!(
                "caddy:{}",
                &crate::knowledge::cut::sha256_hex(caddy.as_bytes())[..16]
            ),
            &project_key,
            "caddy-recipes-hazards",
            crate::agent::backplane::KnowledgeAuthority::Episodic,
            caddy,
        ));
    }
    if let Some(lens) = registry.atlas().build_lens(
        &query,
        history
            .iter()
            .filter(|message| {
                !crate::agent::backplane::is_broker_message(&message.content)
                    && !crate::knowledge::atlas::is_lens_message(&message.content)
            })
            .map(|message| message.content.as_ref()),
    ) {
        candidates.extend(crate::agent::backplane::atlas_lens_candidates(
            &lens,
            &project_key,
        ));
    }
    // Compaction notes are deliberately NOT harvested into the broker block:
    // they stay in place in the history verbatim. Absorbing them meant the
    // broker deleted a note sitting at message ~1 and re-homed its content at
    // the tail on every turn — 0% provider prefix-cache reuse for the rest of
    // the session after the first compaction, and the note could even be
    // silently OMITTED under selection budget pressure. In place, the note is
    // byte-stable (perfect cache material) and always complete.
    let remaining = knowledge_broker_route_budget(history, budget, tools);
    let selection = crate::agent::backplane::KnowledgeBroker::select(
        &project_key,
        &query,
        candidates,
        remaining,
    );
    if mode == crate::agent::backplane::BackplaneMode::Shadow {
        eprintln!(
            "BACKPLANE shadow: sources={} omitted={} tokens={}",
            selection.source_ids.len(),
            selection.omitted,
            selection.tokens
        );
        return;
    }
    let revoked = previous_atlas.iter().any(|(id, digest)| {
        !selection
            .source_ids
            .iter()
            .zip(&selection.source_digests)
            .any(|(new_id, new_digest)| id == new_id && digest == new_digest)
    });
    // Correctness takes precedence over byte-stable caching for a revocation.
    apply_broker_selection(history, &selection, preserve_prefix && !revoked);
}

// Cache-stable sessions retain all already-issued request bytes, including
// across operator turns. Changed evidence is appended (KnowledgeBroker::append
// dedupes an identical or empty selection); compaction owns removal.
fn apply_broker_selection(
    history: &mut Vec<ChatMsg>,
    selection: &crate::agent::backplane::BrokerSelection,
    preserve_prefix: bool,
) {
    if preserve_prefix {
        crate::agent::backplane::KnowledgeBroker::append(history, selection);
    } else {
        crate::agent::backplane::KnowledgeBroker::replace(history, selection);
    }
}

/// Prime a turn's context from long-term memory: search the palace for notes
/// relevant to the latest user message, scoped to this project's wing, and splice
/// the hits in just before that message as a background-reference note.
/// This makes compacted memory return to later work instead of staying archived.
/// No-op when the palace is offline, recall is disabled (`ANGEL_AUTO_RECALL=0`),
/// this exact project/query was already recalled, or there is no hit.
pub(crate) fn maybe_auto_recall(
    registry: &ToolRegistry,
    history: &mut Vec<ChatMsg>,
    budget: usize,
    tools: &[ToolDef],
    events: &mpsc::Sender<TurnEvent>,
) {
    if !registry.store.is_live() || !env_flag("ANGEL_AUTO_RECALL", true) {
        return;
    }
    let Some(query) = history
        .iter()
        .rev()
        .find(|m| m.role == ChatRole::User)
        .map(|m| m.content.clone())
    else {
        return;
    };
    if query.trim().is_empty() {
        return;
    }
    let wing = crate::agent::compaction::project_wing_for(registry.current_workspace());
    let key = recall_query_key(&wing, &query);
    if registry.gauge.recall_key.load(Ordering::Relaxed) == key {
        return;
    }
    remove_auto_recall_notes(history);
    let k = env_usize("ANGEL_AUTO_RECALL_K", DEFAULT_AUTO_RECALL_K).clamp(1, 50);
    // The palace search is a remote MCP round-trip that runs before hop 1 of
    // every submit; its transport timeout (ANGEL_MEMPALACE_TIMEOUT, 30s) is
    // sized for deposits, not for holding up the turn. Search on a helper
    // thread with a short recall-specific deadline — a slow palace skips
    // recall this turn instead of stalling the model call, and the straggler
    // result is dropped harmlessly.
    let deadline =
        std::time::Duration::from_secs(env_usize("ANGEL_RECALL_TIMEOUT_SECS", 3).max(1) as u64);
    let Some(blocks) = auto_recall_search_with_timeout(
        Arc::clone(&registry.store),
        query.clone(),
        k,
        wing.clone(),
        deadline,
    ) else {
        return;
    };
    if blocks.is_empty() {
        return;
    }
    if crate::agent::backplane::active() {
        let project_key = registry.atlas().project_key().to_string();
        let mut candidates =
            crate::agent::backplane::KnowledgeBroker::existing_candidates(history, &project_key);
        candidates.extend(blocks.iter().enumerate().map(|(index, block)| {
            crate::agent::backplane::KnowledgeCandidate::new(
                format!(
                    "palace:{}:{index}",
                    &crate::knowledge::cut::sha256_hex(block.as_bytes())[..12]
                ),
                &project_key,
                "episodic-recall",
                crate::agent::backplane::KnowledgeAuthority::Episodic,
                block.as_str(),
            )
        }));
        let remaining = knowledge_broker_route_budget(history, budget, tools);
        let selection = crate::agent::backplane::KnowledgeBroker::select(
            &project_key,
            &query,
            candidates,
            remaining,
        );
        crate::agent::backplane::KnowledgeBroker::replace(history, &selection);
        if selection.block.is_some() {
            registry.gauge.recall_key.store(key, Ordering::Relaxed);
            let _ = events.send(TurnEvent::Notice(format!(
                "knowledge broker selected {} source(s); {} omitted",
                selection.source_ids.len(),
                selection.omitted
            )));
        }
        return;
    }
    let allowance = auto_recall_allowance(history, budget, tools);
    let Some((note, included, omitted)) = bounded_auto_recall_note(&blocks, allowance) else {
        let _ = events.send(TurnEvent::Notice(
            "recalled notes skipped: no complete note fits the active context budget".to_string(),
        ));
        return;
    };
    registry.gauge.recall_key.store(key, Ordering::Relaxed);
    // Bounded invalidation: the note lands just before the latest user message
    // (this turn's fresh tail), not at the head of the conversation. The old
    // index-1 insert re-keyed on every user turn, invalidating the provider's
    // entire cached prefix each submit; here the prefix stays byte-identical
    // and the note still precedes the question it primes.
    let anchor = history
        .iter()
        .rposition(|m| m.role == ChatRole::User)
        .unwrap_or(history.len());
    history.insert(anchor, ChatMsg::harness(note));
    let _ = events.send(TurnEvent::Notice(format!(
        "recalled {included} note(s) from long-term memory{}",
        if omitted > 0 {
            format!("; {omitted} omitted to fit context")
        } else {
            String::new()
        }
    )));
}

/// Run the optional palace lookup behind a process-wide lease. Memory stores
/// do not expose cancellation, so a timed-out worker may continue running; the
/// lease remains with that worker and makes later turns skip recall instead of
/// accumulating one detached thread per submit. The lane reopens when the real
/// search exits or unwinds.
pub(crate) fn auto_recall_search_with_timeout(
    store: Arc<dyn crate::knowledge::memory::store::MemoryStore>,
    query: Arc<str>,
    limit: usize,
    wing: String,
    timeout: std::time::Duration,
) -> Option<Vec<String>> {
    if AUTO_RECALL_SEARCH_IN_FLIGHT
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return None;
    }
    let (tx, rx) = mpsc::channel();
    if std::thread::Builder::new()
        .name("auto-recall".into())
        .spawn(move || {
            let lease = AutoRecallSearchLease;
            let result = store.search(&query, limit, Some(&wing));
            // A successful caller must observe an open lane before it can
            // start the next distinct recall immediately.
            drop(lease);
            let _ = tx.send(result);
        })
        .is_err()
    {
        AUTO_RECALL_SEARCH_IN_FLIGHT.store(false, Ordering::Release);
        return None;
    }
    match rx.recv_timeout(timeout) {
        Ok(Ok(blocks)) => Some(blocks),
        Ok(Err(_)) | Err(_) => None,
    }
}

fn knowledge_broker_route_budget(history: &[ChatMsg], budget: usize, tools: &[ToolDef]) -> usize {
    if budget == 0 {
        return crate::agent::backplane::RECALL_TOKEN_CEILING + AUTO_RECALL_SAFETY_TOKENS;
    }
    let base_history = history
        .iter()
        .filter(|message| {
            !crate::agent::backplane::is_broker_message(&message.content)
                && !message
                    .content
                    .trim_start()
                    .starts_with(crate::agent::compaction::COMPACTION_NOTE_HEADER)
        })
        .cloned()
        .collect::<Vec<_>>();
    budget.saturating_sub(context_tokens(&base_history, tools))
}

/// The recall injection has an independent hard ceiling even against a roomy
/// model: recalled notes are helpful background, not an invitation to spend an
/// arbitrary fraction of every request on a remote store response. Under a
/// tight model budget it additionally leaves a small safety margin for message
/// framing/token-estimation drift.
fn auto_recall_allowance(history: &[ChatMsg], budget: usize, tools: &[ToolDef]) -> usize {
    let budget_left = if budget == 0 {
        MAX_AUTO_RECALL_TOKENS
    } else {
        budget
            .saturating_sub(context_tokens(history, tools))
            .saturating_sub(AUTO_RECALL_SAFETY_TOKENS)
    };
    budget_left.min(MAX_AUTO_RECALL_TOKENS)
}

/// Retain complete, relevance-ordered recall blocks inside a strict context
/// allowance. Never slice a stored note: recall data is background rather than
/// instructions, and an explicit omission marker is safer and more useful than
/// a partial fragment that looks authoritative out of context.
pub(crate) fn bounded_auto_recall_note(
    blocks: &[String],
    max_tokens: usize,
) -> Option<(String, usize, usize)> {
    let max_bytes = max_tokens.saturating_mul(4);
    if max_bytes <= AUTO_RECALL_HEADER.len() {
        return None;
    }
    let mut kept: Vec<&str> = Vec::new();
    let mut used = AUTO_RECALL_HEADER.len();
    for block in blocks {
        let separator = if !kept.is_empty() { 2 } else { 0 };
        if used.saturating_add(separator).saturating_add(block.len()) > max_bytes {
            break;
        }
        used += separator + block.len();
        kept.push(block);
    }
    if kept.is_empty() {
        return None;
    }
    let mut omitted = blocks.len().saturating_sub(kept.len());
    if omitted > 0 {
        // Reserve an explicit marker. If it displaces a final note, recompute
        // its count rather than silently exceeding the promise made above.
        loop {
            let marker = format!(
                "…[{omitted} additional recalled note(s) omitted to fit context; use recall for detail]"
            );
            let separator = 2;
            if used.saturating_add(separator).saturating_add(marker.len()) <= max_bytes {
                break;
            }
            let last = kept.pop()?;
            used = used.saturating_sub(last.len());
            if !kept.is_empty() {
                used = used.saturating_sub(2);
            }
            omitted += 1;
        }
    }
    let mut note = String::with_capacity(max_bytes.min(used.saturating_add(128)));
    note.push_str(AUTO_RECALL_HEADER);
    note.push_str(&kept.join("\n\n"));
    if omitted > 0 {
        note.push_str(&format!(
            "\n\n…[{omitted} additional recalled note(s) omitted to fit context; use recall for detail]"
        ));
    }
    Some((note, kept.len(), omitted))
}

pub(crate) fn remove_auto_recall_notes(history: &mut Vec<ChatMsg>) {
    history.retain(|m| {
        !(matches!(m.role, ChatRole::System | ChatRole::Harness)
            && m.content.starts_with(AUTO_RECALL_NOTE_PREFIX))
    });
}

const TASK_ANCHOR_MAX_TOKENS: usize = 8_000;
const TASK_ANCHOR_MAX_MESSAGES: usize = 16;
const TASK_ANCHOR_OMISSION: &str =
    "\n…[middle of active user task omitted by bounded compaction anchor]…\n";

/// Preserve operator-authored contracts without elevating them into a summary.
/// Explicit directives/constraint blocks are exact protected evidence. Other
/// user prose uses bounded, digest-bearing excerpts when it exceeds its share.
///
/// The latest request alone is insufficient: a long-running task often carries
/// earlier prohibitions ("never benchmark before writing the kernel") that
/// remain in force. Losing those while preserving an assistant-authored plan
/// reverses authority. Keep the latest compacted request plus bounded recent
/// user turns and directive-bearing user turns. Exact text already present in
/// the protected suffix is not duplicated. Replacement preserves chronological
/// User-role ordering, so a later surviving user correction still wins.
pub(crate) fn compaction_task_anchors(
    history: &[ChatMsg],
    window_start: usize,
    window_end: usize,
    budget: usize,
) -> Vec<String> {
    let suffix_user_text = history[window_end..]
        .iter()
        .filter(|message| message.role == ChatRole::User)
        .map(|message| message.content.as_ref())
        .filter(|text| !text.trim().is_empty())
        .collect::<std::collections::HashSet<_>>();
    let users = history[window_start..window_end]
        .iter()
        .enumerate()
        .filter(|(_, message)| message.role == ChatRole::User)
        .filter_map(|(offset, message)| {
            let text = message.content.as_ref();
            (!text.trim().is_empty() && !suffix_user_text.contains(text)).then_some((
                window_start + offset,
                text,
                operator_directive(text),
            ))
        })
        .collect::<Vec<_>>();
    if users.is_empty() {
        return Vec::new();
    }

    let newer_user_survives = history[window_end..]
        .iter()
        .any(|message| message.role == ChatRole::User);
    let mut priority = vec![0]; // The initial operator task survives later user turns.
    for (index, (_, _, directive)) in users.iter().enumerate().rev() {
        if *directive && !priority.contains(&index) {
            priority.push(index);
        }
        if priority.len() >= TASK_ANCHOR_MAX_MESSAGES {
            break;
        }
    }
    if !newer_user_survives && !priority.contains(&(users.len() - 1)) {
        priority.push(users.len() - 1);
    }
    if !newer_user_survives {
        for index in (0..users.len()).rev() {
            if !priority.contains(&index) {
                priority.push(index);
            }
            if priority.len() >= TASK_ANCHOR_MAX_MESSAGES {
                break;
            }
        }
    }

    let token_cap = TASK_ANCHOR_MAX_TOKENS.min((budget / 4).max(1));
    let mut remaining = token_cap.saturating_mul(4);
    // Explicit operator contracts are protected evidence, independent of the
    // prose-anchor allowance. Never cut a forbidden path or answer format out
    // of the middle to satisfy a soft compaction budget.
    let mut kept = users
        .iter()
        .filter(|(_, _, directive)| *directive)
        .map(|(position, text, _)| (*position, (*text).to_string()))
        .collect::<Vec<_>>();
    for index in priority {
        if users[index].2 {
            continue;
        }
        if remaining == 0 {
            break;
        }
        let (position, text, _) = users[index];
        let bounded = bound_task_anchor(text, remaining);
        if !bounded.is_empty() {
            remaining = remaining.saturating_sub(bounded.len());
            kept.push((position, bounded));
        }
    }
    kept.sort_by_key(|(position, _)| *position);
    kept.into_iter().map(|(_, text)| text).collect()
}

/// Preserve the one replaceable Harness context that belongs to the active
/// operator turn. Unlike task anchors this must retain its Harness role and
/// exact delimiters: goals, memories, selected controls, and skill guidance are
/// already bounded when the cockpit builds the message. A context in the
/// protected suffix wins, so repeated compaction never duplicates an anchor.
pub(crate) fn compaction_turn_context_anchor(
    history: &[ChatMsg],
    window_start: usize,
    window_end: usize,
) -> Option<ChatMsg> {
    if history[window_end..]
        .iter()
        .any(crate::app::control::is_turn_context_message)
    {
        return None;
    }
    history[window_start..window_end]
        .iter()
        .rev()
        .find(|message| crate::app::control::is_turn_context_message(message))
        .cloned()
}

pub(crate) fn operator_directive(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "constraints",
        "answer format",
        "answer contract",
        "must",
        "do not",
        "don't",
        "never",
        "only",
        "forbid",
        "banned",
        "not allowed",
        "required",
        "stop ",
        "without ",
        "no more",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

pub(crate) fn bound_task_anchor(task: &str, max_bytes: usize) -> String {
    if task.len() <= max_bytes {
        return task.to_string();
    }
    let omission = format!(
        "{TASK_ANCHOR_OMISSION}[sha256:{}]\n",
        crate::knowledge::cut::sha256_hex(task.as_bytes())
    );
    if max_bytes <= omission.len() + 2 {
        let mut end = max_bytes.min(task.len());
        while end > 0 && !task.is_char_boundary(end) {
            end -= 1;
        }
        return task[..end].to_string();
    }
    let available = max_bytes - omission.len();
    let mut head_end = available / 2;
    while head_end > 0 && !task.is_char_boundary(head_end) {
        head_end -= 1;
    }
    let mut tail_start = task.len().saturating_sub(available - head_end);
    while tail_start < task.len() && !task.is_char_boundary(tail_start) {
        tail_start += 1;
    }
    format!("{}{}{}", &task[..head_end], omission, &task[tail_start..])
}

/// The protected User-role anchors are spliced separately. The summarizer
/// receives only a digest marker, so it cannot replace their exact contract.
pub(crate) fn compaction_summary_window(window: &[ChatMsg]) -> Vec<ChatMsg> {
    window
        .iter()
        .map(|message| {
            if message.role == ChatRole::User {
                ChatMsg::harness(format!(
                    "[Operator task retained separately; sha256:{}]",
                    crate::knowledge::cut::sha256_hex(message.content.as_bytes())
                ))
            } else {
                message.clone()
            }
        })
        .collect()
}

/// Record what the actual splice carries, not what a summarizer claims.
/// The fixed class inventory is bounded; original operator text stays User-role.
pub(crate) fn constraint_retention_note(
    mut note: String,
    window: &[ChatMsg],
    anchors: &[String],
    suffix: &[ChatMsg],
) -> String {
    let boundary = window
        .iter()
        .filter(|m| crate::agent::compaction::is_compaction_note(m))
        .flat_map(|m| m.content.lines())
        .filter_map(|line| line.strip_prefix("[constraints-ledger/v1] "))
        .filter_map(|json| serde_json::from_str::<serde_json::Value>(json).ok())
        .filter_map(|value| value.get("boundary").and_then(serde_json::Value::as_u64))
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    let mut records = Vec::new();
    let mut verbatim = 0usize;
    let mut restated = 0usize;
    let mut dropped = 0usize;
    let mut excerpts = String::new();
    for message in window
        .iter()
        .chain(suffix)
        .filter(|m| m.role == ChatRole::User)
    {
        let retained = anchors.iter().any(|a| a == &*message.content)
            || suffix
                .iter()
                .any(|m| m.role == ChatRole::User && m.content == message.content);
        let digest = crate::knowledge::cut::sha256_hex(message.content.as_bytes());
        let excerpt = anchors.iter().find(|a| {
            a.contains(&format!("[sha256:{digest}]"))
                || (!a.is_empty() && message.content.starts_with(a.as_str()))
        });
        let status = if retained {
            verbatim += 1;
            "verbatim"
        } else if let Some(excerpt) = excerpt {
            restated += 1;
            if excerpts.len() < 1200 {
                excerpts.push_str(&format!(
                    "\n- Task excerpt (source sha256:{digest}): {}",
                    bound_task_anchor(excerpt, 240)
                ));
            }
            "restated"
        } else {
            dropped += 1;
            "dropped"
        };
        // A bounded detail ledger plus complete aggregate counts. Full explicit
        // contracts remain in their User messages regardless of this limit.
        if records.len() < TASK_ANCHOR_MAX_MESSAGES {
            records.push(serde_json::json!({"sha256": digest, "constraints_retained": status}));
        }
    }
    note.push_str("\n\n## Constraints\nProtected operator task and explicit constraints/answer contract follow in User-role anchors. Constraint classes: named identifiers, forbidden paths, numeric limits, required tests, answer format. Exact anchors govern; excerpts and drops are recorded below.");
    note.push_str(&excerpts);
    note.push_str("\n[constraints-ledger/v1] ");
    note.push_str(&serde_json::json!({
        "constraints_retained": if dropped > 0 { "dropped" } else if restated > 0 { "restated" } else { "verbatim" },
        "boundary": boundary,
        "verbatim": verbatim, "restated": restated, "dropped": dropped, "entries": records,
        "omitted_entries": (verbatim + restated + dropped).saturating_sub(records.len()),
    }).to_string());
    note
}

fn compaction_replacement(
    note: String,
    task_anchors: Vec<String>,
    plan_snapshot: Option<String>,
    handoff_snapshot: Option<String>,
    turn_context_anchor: Option<ChatMsg>,
    recovery_context: Vec<crate::agent::club::RecoveryContextRef>,
) -> Vec<ChatMsg> {
    let mut summary = ChatMsg::harness(note);
    summary.recovery_context = recovery_context;
    let mut replacement = vec![summary];
    if let Some(plan) = plan_snapshot {
        replacement.push(ChatMsg::assistant(plan));
    }
    // The agent's own resume brief rides in Assistant role right after the
    // plan: continuity prose, never a fresh instruction.
    if let Some(handoff) = handoff_snapshot {
        replacement.push(ChatMsg::assistant(handoff));
    }
    // Operator-authored anchors come last. An assistant-authored todo snapshot
    // is useful continuity, but it must never become the most recent apparent
    // instruction when it conflicts with a preserved user prohibition.
    for task in task_anchors {
        replacement.push(ChatMsg::user(task));
    }
    // Cockpit-authored standing context follows the operator anchors just as it
    // does on submit, but never changes role or masquerades as fresh User text.
    if let Some(turn_context) = turn_context_anchor {
        replacement.push(turn_context);
    }
    replacement
}

/// thread. Enabled by the 333k default budget unless `ANGEL_NO_AUTOCOMPACT=1`;
/// `ANGEL_CONTEXT_BUDGET_TOKENS` overrides the budget.
///
/// The distilled sections are persisted to the long-form memory palace (`store`,
/// best-effort) so detail survives the session and can be recalled later, and a
/// terse inline note is spliced back in their place — keeping the live turn's
/// thread without the bulk. Preserves the leading system preamble and a bounded
/// recent tail, keeps tool-call pairing intact, and splices the note
/// as system/background context so it cannot masquerade as the operator's latest
/// request. Best effort: an empty/too-small window or any summarizer failure is a
/// no-op and the turn proceeds uncompacted.
/// Returns whether it compacted.
#[allow(clippy::too_many_arguments)]
pub(crate) fn maybe_compact(
    club: &dyn Club,
    history: &mut Vec<ChatMsg>,
    budget: usize,
    keep_recent: usize,
    keep_recent_tokens: usize,
    tools: &[ToolDef],
    registry: &ToolRegistry,
    events: &mpsc::Sender<TurnEvent>,
) -> bool {
    let store = &registry.store;
    let session_id = &registry.session_id;
    // Count the tool schemas as part of the request — they ride along on every
    // call and can be a large, otherwise-invisible slice of the window.
    if budget == 0 || estimate_tokens(history) + estimate_tool_tokens(tools) <= budget {
        return false;
    }
    // Over budget → we're about to summarize the window. Strip any auto-recall note
    // first: now that select_window folds a prior compaction note into the window,
    // a recall note sitting just after that old note would otherwise land inside
    // the window and get baked into the durable summary. Recall is background
    // reference (refreshed per turn by maybe_auto_recall), not summary material.
    remove_auto_recall_notes(history);
    let Some((sys_end, window_end)) = crate::agent::compaction::select_window_with_token_tail(
        history,
        keep_recent,
        keep_recent_tokens,
    ) else {
        return false; // only preamble + protected tail, or too small to be worth it
    };
    // Anti-thrash: don't spend a summarizer call to reclaim a trivial slice. If the
    // compactable middle is a small fraction of the budget (e.g. a huge protected
    // tail keeps us over budget no matter what), skip until it actually grows.
    let window_tokens = estimate_tokens(&history[sys_end..window_end]);
    let min_yield = (budget / 10).max(256);
    if window_tokens < min_yield {
        return false;
    }
    let _ = events.send(TurnEvent::Notice("compacting context…".to_string()));
    // Keep the dedicated summarizer's per-call ceiling independent of the much
    // larger in-hand model budget. The budget half remains a cap for unusually
    // small contexts; ordinary 32k local compactors use the shared 12k default.
    let chunk_threshold = auto_compact_chunk_threshold(budget);
    let task_anchors = compaction_task_anchors(history, sys_end, window_end, budget);
    let turn_context_anchor = compaction_turn_context_anchor(history, sys_end, window_end);
    let wing = crate::agent::compaction::project_wing_for(registry.current_workspace());
    let source = crate::agent::compaction::provenance(session_id, "auto-compact");
    let live = store.is_live();
    // Route summarization to the cheapest capable club: an explicit
    // `ANGEL_COMPACT_URL` endpoint wins; otherwise, when the in-hand driver is
    // a paid SOTA link or a fan-out pipeline (the swarm), a reachable local
    // fleet model takes the bulk work instead of the expensive driver. Only a
    // plain local in-hand club summarizes itself.
    let env_summarizer = compaction_summarizer();
    let local_summarizer = if env_summarizer.is_none() && wants_cheap_summarizer(club) {
        pick_local_summarizer(&registry.aux_clubs)
    } else {
        None
    };
    let summarizer: &dyn Club = env_summarizer
        .as_ref()
        .map(|c| c as &dyn Club)
        .or(local_summarizer.as_deref())
        .unwrap_or(club);
    if let Some(local) = &local_summarizer {
        let _ = events.send(TurnEvent::Notice(format!(
            "compaction summarizing on local {} (in-hand {} stays free)",
            local.label(),
            club.label()
        )));
    }
    registry.auxiliary.utility_entered("sync_compaction");
    let Some(result) = crate::agent::compaction::compact_window_with_state_budget(
        summarizer,
        &compaction_summary_window(&history[sys_end..window_end]),
        &wing,
        &source,
        chunk_threshold,
        budget,
        live,
    ) else {
        return false; // best effort: leave history untouched on any failure
    };
    // Splice the inline note in place of the window first — that alone preserves
    // the turn's thread. Then persist the drawers *off-thread*: a slow or hung
    // MemPalace must never add its per-call timeout × N to the turn's latency
    // (deposits are best-effort durability, not continuity).
    let n = result.drawers.len();
    let recovery_context = crate::agent::club::recovery_context_refs(&history[sys_end..window_end]);
    let Ok(note) = park_compaction_window(&history[sys_end..window_end], &result.inline_note)
    else {
        return false;
    };
    let note = constraint_retention_note(
        note,
        &history[sys_end..window_end],
        &task_anchors,
        &history[window_end..],
    );
    history.splice(
        sys_end..window_end,
        compaction_replacement(
            note,
            task_anchors,
            result.plan_snapshot,
            result.handoff_snapshot,
            turn_context_anchor,
            recovery_context,
        ),
    );
    let done = if live && n > 0 {
        let store = Arc::clone(store);
        let drawers = result.drawers;
        enqueue_memory_write(MemoryWriteJob::Drawers {
            store,
            drawers,
            completion: None,
        });
        format!("context compacted — filing {n} note(s) to memory")
    } else {
        "context compacted".to_string()
    };
    let _ = events.send(TurnEvent::Notice(format!(
        "{done}; operator task/answer contract retention recorded"
    )));
    true
}

/// Hard-latency compaction used on the live turn boundary. The richer LLM
/// compactor remains available behind `ANGEL_COMPACT_SYNC_LLM=1`, and the early
/// background pass still gets first chance to land its model summary. By
/// default, overflow can never make the operator wait on an inference endpoint.
#[allow(clippy::too_many_arguments)]
pub(crate) fn maybe_compact_for_turn(
    club: &dyn Club,
    history: &mut Vec<ChatMsg>,
    budget: usize,
    keep_recent: usize,
    keep_recent_tokens: usize,
    tools: &[ToolDef],
    registry: &ToolRegistry,
    events: &mpsc::Sender<TurnEvent>,
) -> bool {
    let _span = super::turn::background::span("compaction_ms");
    let result = maybe_compact_for_turn_measured(
        club,
        history,
        budget,
        keep_recent,
        keep_recent_tokens,
        tools,
        registry,
        events,
    );
    super::turn::background::observe(history);
    result
}

#[allow(clippy::too_many_arguments)]
fn maybe_compact_for_turn_measured(
    club: &dyn Club,
    history: &mut Vec<ChatMsg>,
    budget: usize,
    keep_recent: usize,
    keep_recent_tokens: usize,
    tools: &[ToolDef],
    registry: &ToolRegistry,
    events: &mpsc::Sender<TurnEvent>,
) -> bool {
    speak_cache_budget_lift(events);
    if crate::agent::compaction::sync_compaction_uses_model() {
        return maybe_compact(
            club,
            history,
            budget,
            keep_recent,
            keep_recent_tokens,
            tools,
            registry,
            events,
        );
    }
    if budget == 0 || estimate_tokens(history) + estimate_tool_tokens(tools) <= budget {
        return false;
    }
    remove_auto_recall_notes(history);
    let Some((sys_end, window_end)) = crate::agent::compaction::select_window_with_token_tail(
        history,
        keep_recent,
        keep_recent_tokens,
    ) else {
        return false;
    };
    let window_tokens = estimate_tokens(&history[sys_end..window_end]);
    if window_tokens < (budget / 10).max(256) {
        return false;
    }

    if cache_stable_mode(club) {
        // Age while the window still holds full tool bodies.
        let _ = age_tool_results_at_boundary(history);
    }
    let started = Instant::now();
    let _ = events.send(TurnEvent::Notice(
        "compacting context locally (model-free latency guard)…".to_string(),
    ));
    let task_anchors = compaction_task_anchors(history, sys_end, window_end, budget);
    let turn_context_anchor = compaction_turn_context_anchor(history, sys_end, window_end);
    let wing = crate::agent::compaction::project_wing_for(registry.current_workspace());
    let source = crate::agent::compaction::provenance(&registry.session_id, "auto-compact");
    let live = registry.store.is_live();
    let Some(result) = crate::agent::compaction::compact_window_fast(
        &compaction_summary_window(&history[sys_end..window_end]),
        &wing,
        &source,
        budget,
        live,
    ) else {
        return false;
    };
    let n = result.drawers.len();
    let recovery_context = crate::agent::club::recovery_context_refs(&history[sys_end..window_end]);
    let Ok(note) = park_compaction_window(&history[sys_end..window_end], &result.inline_note)
    else {
        return false;
    };
    let note = constraint_retention_note(
        note,
        &history[sys_end..window_end],
        &task_anchors,
        &history[window_end..],
    );
    history.splice(
        sys_end..window_end,
        compaction_replacement(
            note,
            task_anchors,
            result.plan_snapshot,
            result.handoff_snapshot,
            turn_context_anchor,
            recovery_context,
        ),
    );
    let elapsed_ms = started.elapsed().as_millis();
    let done = if live && n > 0 {
        let store = Arc::clone(&registry.store);
        let drawers = result.drawers;
        enqueue_memory_write(MemoryWriteJob::Drawers {
            store,
            drawers,
            completion: None,
        });
        format!("context compacted locally in {elapsed_ms}ms — filing {n} note(s) to memory")
    } else {
        format!("context compacted locally in {elapsed_ms}ms")
    };
    let _ = events.send(TurnEvent::Notice(format!(
        "{done}; operator task/answer contract retention recorded"
    )));
    true
}

// ---------------------------------------------------------------------------
// Background compaction — the early pass that hides the summarizer latency.
// ---------------------------------------------------------------------------

/// An early compaction pass running off-thread (perf follow-up: the sync
/// [`maybe_compact`] runs the whole LLM map-reduce inline at a hop boundary —
/// 30s to minutes of dead air). `window` is the exact snapshot the summarizer
/// is distilling; the splice only ever replaces a contiguous run of history
/// that still matches it message-for-message, so a history that moved on
/// (pruned, sync-compacted, edited) simply discards the result. Homed on the
/// registry so a pass started late in one turn lands at the next turn's first
/// boundary.
pub(crate) struct BgCompact {
    window: Vec<ChatMsg>,
    task_anchors: Vec<String>,
    turn_context_anchor: Option<ChatMsg>,
    /// Which club is summarizing — named in operator notices so a failing or
    /// slow compaction route is attributable.
    summarizer_label: String,
    started: Instant,
    timed_out: bool,
    done: Arc<AtomicBool>,
    /// Written once by the worker: `Some(None)` means the summarizer failed.
    result: Arc<std::sync::Mutex<Option<Option<crate::agent::compaction::CompactionResult>>>>,
}

/// Registry-homed background-compaction state: the in-flight pass plus the
/// failure backoff. Without the backoff a degraded summarizer (endpoint down,
/// or the box saturated by real work) produced an endless spawn→fail→respawn
/// loop, each spawn spamming a "compacting context" notice into the transcript.
#[derive(Default)]
pub(crate) struct BgCompactState {
    pub(crate) slot: Option<BgCompact>,
    pub(crate) cooldown_until: Option<Instant>,
    consecutive_failures: u32,
}

impl BgCompactState {
    fn cooling_down(&self, now: Instant) -> bool {
        self.cooldown_until.is_some_and(|until| now < until)
    }

    /// Record a failed/timed-out pass and arm the retry cooldown. Repeated
    /// failures escalate the wait — a summarizer that keeps failing is down or
    /// drowning, and hammering it helps neither side.
    fn note_failure(&mut self, now: Instant) -> Duration {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        let cooldown = bg_retry_cooldown(self.consecutive_failures);
        self.cooldown_until = Some(now + cooldown);
        cooldown
    }

    fn note_success(&mut self) {
        self.consecutive_failures = 0;
        self.cooldown_until = None;
    }
}

/// Wait before re-arming background compaction after a failed pass.
/// `ANGEL_COMPACT_BG_RETRY_SECS` overrides the base (default 120s); from the
/// third consecutive failure the wait is 5× base, capped at an hour. The
/// synchronous latency-guard compactor remains the correctness backstop
/// throughout — backing off here costs nothing but the early head start.
fn bg_retry_cooldown(consecutive_failures: u32) -> Duration {
    let base = env_usize("ANGEL_COMPACT_BG_RETRY_SECS", 120).clamp(10, 3_600) as u64;
    let secs = if consecutive_failures >= 3 {
        (base * 5).min(3_600)
    } else {
        base
    };
    Duration::from_secs(secs)
}

/// A detached summarizer must not reserve the background slot forever. The
/// normal HTTP deadline is much shorter; this is a final harness-level guard
/// for a wedged implementation or transport that ignores its own timeout.
const BG_COMPACT_WORKER_LIMIT: usize = 8;
static BG_COMPACT_WORKERS: AtomicUsize = AtomicUsize::new(0);

fn bg_compact_max_age() -> Duration {
    Duration::from_secs(env_usize("ANGEL_COMPACT_BG_TIMEOUT_SECS", 30).clamp(5, 15 * 60) as u64)
}

struct BgCompactPermit;

impl Drop for BgCompactPermit {
    fn drop(&mut self) {
        BG_COMPACT_WORKERS.fetch_sub(1, Ordering::AcqRel);
    }
}

fn acquire_bg_compact_permit() -> Option<BgCompactPermit> {
    BG_COMPACT_WORKERS
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
            (current < BG_COMPACT_WORKER_LIMIT).then_some(current + 1)
        })
        .ok()
        .map(|_| BgCompactPermit)
}

#[cfg(test)]
pub(crate) fn bg_compact_workers_inflight() -> usize {
    BG_COMPACT_WORKERS.load(Ordering::Acquire)
}

#[cfg(test)]
pub(crate) fn age_bg_compact_for_test(registry: &ToolRegistry) {
    if let Some(bg) = registry
        .bg_compact
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .slot
        .as_mut()
    {
        bg.started = Instant::now()
            .checked_sub(bg_compact_max_age())
            .unwrap_or_else(Instant::now);
    }
}

#[cfg(test)]
pub(crate) fn expire_bg_cooldown_for_test(registry: &ToolRegistry) {
    registry
        .bg_compact
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .cooldown_until = Some(Instant::now());
}

/// Percent of the budget at which compaction starts early in the background.
/// `ANGEL_COMPACT_BG_PCT` overrides; 0 (or ≥100) disables the early pass and
/// leaves the synchronous 100% path as the only compactor.
fn bg_compact_pct() -> usize {
    env_usize("ANGEL_COMPACT_BG_PCT", 80)
}

/// Owned summarizer for a background pass: the explicit `ANGEL_COMPACT_URL`
/// club, else the first reachable local fleet club. `None` when the only
/// candidate would be the in-hand club — it is borrowed for the turn and busy
/// with the live hop, so the caller stays on the sync path instead.
/// `ANGEL_COMPACT_LOCAL=0` ("never reroute my summaries to another model")
/// disables the fleet fallback here too, not just on the sync path.
fn bg_summarizer(aux: &[Arc<dyn Club>]) -> Option<Arc<dyn Club>> {
    if let Some(c) = compaction_summarizer() {
        return Some(Arc::new(c));
    }
    if !env_flag("ANGEL_COMPACT_LOCAL", true) {
        return None;
    }
    pick_local_summarizer(aux)
}

/// Whether a background pass is in flight (running, or done and awaiting its
/// splice). While true the sync path defers, up to its hard ceiling.
pub(crate) fn bg_compact_inflight(registry: &ToolRegistry) -> bool {
    registry
        .bg_compact
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .slot
        .is_some()
}

/// Arm a background compaction pass once usage crosses the early threshold
/// (~80% of budget): snapshot the compactable window and summarize it on a
/// helper thread while the turn's live hops keep flowing. Mirrors the sync
/// path's guards (recall-note strip, window selection, anti-thrash minimum);
/// a no-op when a pass is already in flight or no owned summarizer exists.
pub(crate) fn maybe_start_bg_compact(
    history: &mut Vec<ChatMsg>,
    budget: usize,
    keep_recent: usize,
    keep_recent_tokens: usize,
    tools: &[ToolDef],
    registry: &ToolRegistry,
    events: &mpsc::Sender<TurnEvent>,
) {
    speak_cache_budget_lift(events);
    let pct = bg_compact_pct();
    if budget == 0 || pct == 0 || pct >= 100 {
        return;
    }
    let used = estimate_tokens(history) + estimate_tool_tokens(tools);
    if used < budget.saturating_mul(pct) / 100 {
        return;
    }
    let mut state = registry
        .bg_compact
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if state.slot.is_some() {
        return; // one pass at a time
    }
    if state.cooling_down(Instant::now()) {
        return; // a recent pass failed — wait out the retry cooldown
    }
    let Some(summarizer) = bg_summarizer(&registry.aux_clubs) else {
        return;
    };
    let summarizer_label = summarizer.label().to_string();
    // Same pre-selection as the sync path (see maybe_compact for why a recall
    // note must never bake into a durable summary).
    remove_auto_recall_notes(history);
    let Some((sys_end, window_end)) = crate::agent::compaction::select_window_with_token_tail(
        history,
        keep_recent,
        keep_recent_tokens,
    ) else {
        return;
    };
    let window_tokens = estimate_tokens(&history[sys_end..window_end]);
    if window_tokens < (budget / 10).max(256) {
        return;
    }
    let window: Vec<ChatMsg> = history[sys_end..window_end].to_vec();
    let task_anchors = compaction_task_anchors(history, sys_end, window_end, budget);
    let turn_context_anchor = compaction_turn_context_anchor(history, sys_end, window_end);
    let chunk_threshold = auto_compact_chunk_threshold(budget);
    let wing = crate::agent::compaction::project_wing_for(registry.current_workspace());
    let source = crate::agent::compaction::provenance(&registry.session_id, "auto-compact");
    let live = registry.store.is_live();
    let result = Arc::new(std::sync::Mutex::new(None));
    let worker_window = compaction_summary_window(&window);
    let worker_result = Arc::clone(&result);
    let worker_done = Arc::new(AtomicBool::new(false));
    let done = Arc::clone(&worker_done);
    let Some(permit) = acquire_bg_compact_permit() else {
        return;
    };
    registry.auxiliary.utility_entered("background_compaction");
    let worker_span = super::turn::background::span("compaction_ms");
    let spawned = std::thread::Builder::new()
        .name("bg-compact".into())
        .spawn(move || {
            let _worker_span = worker_span;
            // A third-party/local club implementation can panic. Convert that
            // into an ordinary failed pass so the registry slot is released at
            // the next boundary instead of remaining in-flight forever.
            let out = crate::ui::term::catch_background_unwind(|| {
                crate::agent::compaction::compact_window_with_state_budget(
                    &*summarizer,
                    &worker_window,
                    &wing,
                    &source,
                    chunk_threshold,
                    budget,
                    live,
                )
            })
            .unwrap_or(None);
            *worker_result
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(out);
            // Admission follows actual thread completion, not registry-slot
            // eviction. Release the process-wide permit before publishing done.
            drop(permit);
            worker_done.store(true, Ordering::Release);
        })
        .is_ok();
    if spawned {
        let _ = events.send(TurnEvent::Notice(format!(
            "compacting context in the background via {summarizer_label} ({}% of budget)",
            used.saturating_mul(100) / budget
        )));
        state.slot = Some(BgCompact {
            window,
            task_anchors,
            turn_context_anchor,
            summarizer_label,
            started: Instant::now(),
            timed_out: false,
            done,
            result,
        });
    }
}

/// Land a finished background pass: verify the snapshot window still sits in
/// `history` verbatim, splice the inline note in its place, and file the
/// drawers off-thread — zero summarizer latency on the live turn. A history
/// that changed under the pass discards the result (the sync path remains the
/// backstop); a still-running pass is left alone.
pub(crate) fn try_splice_bg_compact(
    registry: &ToolRegistry,
    history: &mut Vec<ChatMsg>,
    events: &mpsc::Sender<TurnEvent>,
) {
    let _span = super::turn::background::span("compaction_ms");
    try_splice_bg_compact_measured(registry, history, events);
    super::turn::background::observe(history);
}

fn try_splice_bg_compact_measured(
    registry: &ToolRegistry,
    history: &mut Vec<ChatMsg>,
    events: &mpsc::Sender<TurnEvent>,
) {
    let mut state = registry
        .bg_compact
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(bg) = state.slot.as_mut()
        && bg.started.elapsed() >= bg_compact_max_age()
    {
        if !bg.timed_out {
            bg.timed_out = true;
            let _ = events.send(TurnEvent::Notice(format!(
                    "background model compaction via {} timed out; the local latency guard will compact if needed",
                    bg.summarizer_label
                )));
        }
        // Keep the slot while the detached worker is genuinely alive. A
        // replacement can start only after its completion marker releases
        // the admission permit, preventing stale-worker accumulation.
        if bg.done.load(Ordering::Acquire) {
            state.slot.take();
            // A timed-out route is a failing route: back off before the
            // next attempt instead of respawning at the next boundary.
            state.note_failure(Instant::now());
        }
        return;
    }
    let Some(bg) = state.slot.as_ref() else {
        return;
    };
    if !bg.done.load(Ordering::Acquire) {
        return; // still summarizing
    }
    let Some(outcome) = bg
        .result
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
    else {
        return;
    };
    let bg = state.slot.take().expect("slot checked non-empty above");
    let Some(result) = outcome else {
        // Summarizer failed — best effort, the sync path remains. Arm the
        // retry cooldown so a sick route cannot respawn-and-spam every
        // boundary, and tell the operator where compaction is routed.
        let cooldown = state.note_failure(Instant::now());
        drop(state);
        let _ = events.send(TurnEvent::Notice(format!(
            "background compaction via {} failed; next attempt in {}s (ANGEL_COMPACT_URL routes summaries to a dedicated fast endpoint)",
            bg.summarizer_label,
            cooldown.as_secs()
        )));
        return;
    };
    // The summarizer produced a result — the route is healthy either way the
    // splice goes, so clear any failure backoff now.
    state.note_success();
    drop(state);
    let Some(start) = find_window(history, &bg.window) else {
        let _ = events.send(TurnEvent::Notice(
            "background compaction discarded — history changed under it".to_string(),
        ));
        return;
    };
    let n = result.drawers.len();
    let newer_user_survives = history[start + bg.window.len()..]
        .iter()
        .any(|m| m.role == ChatRole::User);
    let task_anchors = bg
        .task_anchors
        .into_iter()
        .filter(|anchor| {
            (!newer_user_survives || operator_directive(anchor))
                && !history[start + bg.window.len()..]
                    .iter()
                    .any(|m| m.role == ChatRole::User && m.content.as_ref() == anchor)
        })
        .collect::<Vec<_>>();
    let newer_turn_context_survives = history[start + bg.window.len()..]
        .iter()
        .any(crate::app::control::is_turn_context_message);
    let turn_context_anchor = if newer_turn_context_survives {
        None
    } else {
        bg.turn_context_anchor
    };
    // A retained worker can have originated before this turn began.
    registry.auxiliary.utility_entered("background_compaction");
    let recovery_context = crate::agent::club::recovery_context_refs(&bg.window);
    let Ok(note) = park_compaction_window(&bg.window, &result.inline_note) else {
        return;
    };
    let note = constraint_retention_note(
        note,
        &bg.window,
        &task_anchors,
        &history[start + bg.window.len()..],
    );
    history.splice(
        start..start + bg.window.len(),
        compaction_replacement(
            note,
            task_anchors,
            result.plan_snapshot,
            result.handoff_snapshot,
            turn_context_anchor,
            recovery_context,
        ),
    );
    let done = if registry.store.is_live() && n > 0 {
        let store = Arc::clone(&registry.store);
        let drawers = result.drawers;
        enqueue_memory_write(MemoryWriteJob::Drawers {
            store,
            drawers,
            completion: None,
        });
        format!("context compacted in the background — filing {n} note(s) to memory")
    } else {
        "context compacted in the background".to_string()
    };
    let _ = events.send(TurnEvent::Notice(format!(
        "{done}; operator task/answer contract retention recorded"
    )));
}

/// Locate `window` as a contiguous run inside `history` (first match).
fn find_window(history: &[ChatMsg], window: &[ChatMsg]) -> Option<usize> {
    if window.is_empty() || history.len() < window.len() {
        return None;
    }
    (0..=history.len() - window.len())
        .find(|&s| (0..window.len()).all(|i| msg_eq(&history[s + i], &window[i])))
}

/// Message identity for the splice check: role + content + tool linkage.
/// Attachments are skipped — media blobs don't decide whether this is still
/// the same stretch of conversation.
fn msg_eq(a: &ChatMsg, b: &ChatMsg) -> bool {
    a.role == b.role
        && a.content == b.content
        && a.tool_call_id == b.tool_call_id
        && a.tool_calls.len() == b.tool_calls.len()
        && a.tool_calls
            .iter()
            .zip(b.tool_calls.iter())
            .all(|(x, y)| x.id == y.id && x.name == y.name && x.args == y.args)
}

// ---------------------------------------------------------------------------
// Orchestrator layer — git-worktree delegation + serialized integration.
// ---------------------------------------------------------------------------

/// Role blurb for a club label, used in the orchestrator system prompt. Keyed by
/// the host-based club labels (see `Bag::standard`).
pub(crate) fn club_role(label: &str) -> &'static str {
    match label {
        "turbo" => "deep reasoning and analysis",
        "spark" => "coding and orchestration",
        "spark-r1" => "mathematics",
        "spark-v4" => "deep reasoning and coding",
        "atlas" => "general work and long context",
        _ => "general work",
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/compact__memory_write_queue_tests.rs"]
mod memory_write_queue_tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/compact__backoff_tests.rs"]
mod backoff_tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/compact__cache_budget_lift_tests.rs"]
mod cache_budget_lift_tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/compact__p06c_prefix_tests.rs"]
mod p06c_prefix_tests;
