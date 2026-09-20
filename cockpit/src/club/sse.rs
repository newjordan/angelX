//! SSE streaming: decoding OpenAI chat-completions `stream: true` bodies.

use super::*;

/// Wake a blocking provider read on operator cancellation. The scoped worker is
/// joined before the attempt scope ends, including retries and errors.
/// Polling observes a flag; it imposes no deadline on the provider or the run.
pub(super) struct CancelReadGuard<'scope> {
    stop: std::sync::mpsc::Sender<()>,
    worker: Option<std::thread::ScopedJoinHandle<'scope, ()>>,
}

impl<'scope> CancelReadGuard<'scope> {
    pub(super) fn new<'env>(
        scope: &'scope std::thread::Scope<'scope, 'env>,
        cancel: &'env AtomicBool,
        abort: ureq::AbortHandle,
    ) -> Self {
        let (stop, stopped) = std::sync::mpsc::channel();
        let worker = Some(scope.spawn(move || {
            loop {
                if cancel.load(Ordering::Relaxed) {
                    abort.abort();
                    break;
                }
                match stopped.recv_timeout(Duration::from_millis(10)) {
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    _ => break,
                }
            }
        }));
        Self { stop, worker }
    }
}

impl Drop for CancelReadGuard<'_> {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

// --- SSE streaming (OpenAI chat-completions `stream: true`) ------------------

/// One decoded line of a `text/event-stream` body.
pub(crate) enum SseEvent {
    /// A `data: {…}` chunk (parsed JSON).
    Chunk(serde_json::Value),
    /// The terminal `data: [DONE]` sentinel.
    Done,
    /// A blank line, a `:` comment / keep-alive, or an unparseable payload.
    Ignore,
}

/// Decode a single SSE line. Robust to keep-alive comments and stray blank lines
/// (servers send those to hold the connection open over a slow link).
pub(crate) fn parse_sse_line(line: &str) -> SseEvent {
    let line = line.trim();
    if line.is_empty() || line.starts_with(':') {
        return SseEvent::Ignore;
    }
    // Lines are `data: <payload>`; tolerate a missing prefix just in case.
    let payload = line.strip_prefix("data:").map(str::trim).unwrap_or(line);
    if payload == "[DONE]" {
        return SseEvent::Done;
    }
    match serde_json::from_str(payload) {
        Ok(v) => SseEvent::Chunk(v),
        Err(_) => SseEvent::Ignore,
    }
}

fn reasoning_object_text(value: &serde_json::Value) -> Option<&str> {
    value
        .as_str()
        .or_else(|| value.get("text").and_then(|v| v.as_str()))
        .or_else(|| value.get("content").and_then(|v| v.as_str()))
        .filter(|text| !text.is_empty())
}

/// DeepSeek uses `reasoning_content`; current vLLM uses `reasoning`.
/// OpenAI-compatible / OpenRouter / z.ai streams may send `reasoning` as an
/// object or `reasoning_details` as an array of `{text|content}` fragments.
/// Prefer a nonempty legacy field when both string aliases are present, without
/// duplicating them. Preserve an explicit empty string as protocol evidence too.
pub(crate) fn response_reasoning(message: &serde_json::Value) -> Option<&str> {
    let legacy = message
        .get("reasoning_content")
        .and_then(|value| value.as_str());
    legacy
        .filter(|text| !text.is_empty())
        .or_else(|| message.get("reasoning").and_then(reasoning_object_text))
        .or_else(|| message.get("reasoning").and_then(|value| value.as_str()))
        .or(legacy)
}

/// Streaming delta text, including OpenAI `reasoning_details` fragments.
/// `Some("")` means a protocol reasoning field was present but empty (lock out
/// marker splitting without emitting a live token).
pub(crate) fn response_reasoning_delta(message: &serde_json::Value) -> Option<String> {
    if let Some(text) = response_reasoning(message) {
        return Some(text.to_string());
    }
    let details = message.get("reasoning_details")?;
    let mut out = String::new();
    match details {
        serde_json::Value::Array(items) => {
            for item in items {
                if let Some(text) = reasoning_object_text(item) {
                    out.push_str(text);
                }
            }
        }
        other => {
            if let Some(text) = reasoning_object_text(other) {
                out.push_str(text);
            }
        }
    }
    if out.is_empty() && details.is_null() {
        return None;
    }
    Some(out)
}

// --- Raw reasoning-marker splitting (local DeepSeek-R1 / Qwen3 distills) -----
//
// Self-hosted reasoning distills (llama.cpp, Ollama, vLLM without a reasoning
// parser) do NOT emit the DeepSeek API's `reasoning_content` field. They write
// the chain-of-thought into `content` itself, in the marker dialect the R1
// chat template defines: the assistant turn is prefilled with `<think>\n`, the
// model thinks, then emits `</think>` and the visible answer follows
// (optionally wrapped in `<response>…</response>`). The official DeepSeek-R1
// and R1-Distill-Qwen tokenizer templates rebuild history as
// `content.split('</think>')[-1]` — everything before the close tag is
// reasoning and must not become visible reply text or persisted history.
// Without this split, raw distills leak their chain-of-thought into the
// transcript and trip the TTSR stream-rule matcher on thinking prose.

const THINK_OPEN: &str = "<think>";
const THINK_CLOSE: &str = "</think>";
const RESPONSE_OPEN: &str = "<response>";
const RESPONSE_CLOSE: &str = "</response>";
const MARKERS: [&str; 4] = [THINK_OPEN, THINK_CLOSE, RESPONSE_OPEN, RESPONSE_CLOSE];
/// Longest proper prefix of any marker that can straddle a chunk boundary
/// (`len("</response>") - 1`).
const MAX_MARKER_PREFIX: usize = RESPONSE_CLOSE.len() - 1;

/// Process-wide `ANGEL_REASONING_MARKERS` gate. Default on; the stream path
/// consults this on the first content delta of every hop, so the env var is
/// read once. Tests resync under `env_lock` the same way markdown syntax /
/// FALLBACK_ARMED do.
static MARKER_SPLITTING_ENABLED: AtomicBool = AtomicBool::new(true);
static MARKER_SPLITTING_SEEDED: AtomicBool = AtomicBool::new(false);

fn marker_splitting_from_env() -> bool {
    marker_splitting_enabled_from(std::env::var("ANGEL_REASONING_MARKERS").ok().as_deref())
}

fn seed_marker_splitting_from_env() {
    if MARKER_SPLITTING_SEEDED
        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
    {
        MARKER_SPLITTING_ENABLED.store(marker_splitting_from_env(), Ordering::Relaxed);
    }
}

/// `ANGEL_REASONING_MARKERS` — default on (auto-detect); `0/false/no/off`
/// disables marker splitting entirely so a model that *quotes* the tags in
/// prose is never misread.
pub(crate) fn marker_splitting_enabled() -> bool {
    seed_marker_splitting_from_env();
    MARKER_SPLITTING_ENABLED.load(Ordering::Relaxed)
}

fn marker_splitting_enabled_from(value: Option<&str>) -> bool {
    !value.map(str::trim).is_some_and(|value| {
        matches!(
            value.to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        )
    })
}

/// Sync the process-wide bit after `set_var` / `remove_var`. Marks seeded so
/// the stream path does not re-hit env. Same contract as `set_fallback_armed`.
#[cfg(test)]
fn set_marker_splitting_enabled(on: bool) {
    MARKER_SPLITTING_SEEDED.store(true, Ordering::Relaxed);
    MARKER_SPLITTING_ENABLED.store(on, Ordering::Relaxed);
}

/// Re-read `ANGEL_REASONING_MARKERS` into the cache. Tests that hold
/// `crate::tests::env_lock()` and mutate the var must call this so the cache
/// observes the override; call again after the env guard drops to restore.
#[cfg(test)]
fn resync_marker_splitting_from_env() {
    MARKER_SPLITTING_SEEDED.store(false, Ordering::Relaxed);
    seed_marker_splitting_from_env();
}

/// One split's contribution: private reasoning and visible content.
#[derive(Default)]
pub(crate) struct MarkerSplit {
    pub(crate) reasoning: String,
    pub(crate) content: String,
}

/// Incremental, chunk-boundary-safe state machine for the ` ` marker dialect.
/// Pure (no IO) so it unit-tests against canned chunk sequences.
#[derive(Default)]
pub(crate) struct MarkerSplitter {
    in_think: bool,
    /// A marker has appeared anywhere in this stream (enables the lenient
    /// first-close rule and stray-close verbatim handling).
    saw_any_marker: bool,
    /// A think block just closed and the optional `<response>` open tag may
    /// still arrive (possibly split across chunks). Consumed at the next
    /// feed boundary.
    expect_response_open: bool,
    /// Tail held back because it may be the start of a marker split across
    /// two stream chunks.
    pending: String,
}

fn skip_ascii_ws(buf: &str, mut i: usize) -> usize {
    let bytes = buf.as_bytes();
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\n' | b'\r' | b'\t') {
        i += 1;
    }
    i
}

/// Is `s` a proper prefix of some marker (a marker may be cut mid-tag at a
/// chunk boundary)?
fn is_partial_marker_prefix(s: &str) -> bool {
    !s.is_empty()
        && MARKERS
            .iter()
            .any(|m| m.len() > s.len() && m.starts_with(s))
}

/// First position at/after `from` that starts a proper prefix of some marker
/// (bytes from there must be held until the next chunk), else `buf.len()`.
fn partial_marker_cut(buf: &str, from: usize) -> usize {
    let mut start = from.max(buf.len().saturating_sub(MAX_MARKER_PREFIX));
    while start < buf.len() && !buf.is_char_boundary(start) {
        start += 1;
    }
    // Every marker begins with ASCII `<`. Jump only between viable candidates:
    // this is both UTF-8 safe and bounded to the short tail above.
    for (offset, _) in buf[start..].match_indices('<') {
        let p = start + offset;
        if is_partial_marker_prefix(&buf[p..]) {
            return p;
        }
    }
    buf.len()
}

impl MarkerSplitter {
    /// Advance past a just-consumed close tag: skip whitespace and an optional
    /// `<response>` open tag. When nothing (or only a partial tag) follows in
    /// this chunk, arm the next-boundary check so a tag split across chunks is
    /// still stripped.
    fn advance_after_close(&mut self, buf: &str, i: usize) -> usize {
        let j = skip_ascii_ws(buf, i);
        if buf[j..].starts_with(RESPONSE_OPEN) {
            return skip_ascii_ws(buf, j + RESPONSE_OPEN.len());
        }
        self.expect_response_open = buf[j..].is_empty() || is_partial_marker_prefix(&buf[j..]);
        j
    }

    /// Fold one content delta in. Emitted reasoning/content excludes any tail
    /// that could still become a marker once the next chunk arrives.
    pub(crate) fn feed(&mut self, text: &str) -> MarkerSplit {
        if text.is_empty() {
            return MarkerSplit::default();
        }
        let mut buf = String::with_capacity(self.pending.len() + text.len());
        buf.push_str(&self.pending);
        buf.push_str(text);
        self.pending.clear();

        let mut out = MarkerSplit::default();
        let mut seg_start = 0usize;
        let mut i = 0usize;
        if self.expect_response_open {
            // The previous chunk ended right after (or mid-) `</think>`: an
            // optional `<response>` tag may open this chunk.
            self.expect_response_open = false;
            let mut j = skip_ascii_ws(&buf, 0);
            if buf[j..].starts_with(RESPONSE_OPEN) {
                j = skip_ascii_ws(&buf, j + RESPONSE_OPEN.len());
            }
            i = j;
            seg_start = j;
        }
        while i < buf.len() {
            // All supported markers begin with ASCII `<`. Plain prose is the hot
            // path, so skip directly to the next candidate instead of slicing at
            // every byte (which also panics inside multi-byte UTF-8 characters).
            let Some(offset) = buf[i..].find('<') else {
                break;
            };
            i += offset;
            if self.in_think {
                if buf[i..].starts_with(THINK_CLOSE) {
                    out.reasoning.push_str(&buf[seg_start..i]);
                    self.in_think = false;
                    i = self.advance_after_close(&buf, i + THINK_CLOSE.len());
                    seg_start = i;
                    continue;
                }
                // `<` is one ASCII byte and therefore always ends on a character
                // boundary, even when the surrounding reasoning is Unicode.
                i += 1;
                continue;
            }
            if buf[i..].starts_with(THINK_OPEN) {
                out.content.push_str(&buf[seg_start..i]);
                self.in_think = true;
                self.saw_any_marker = true;
                i += THINK_OPEN.len();
                seg_start = i;
                continue;
            }
            if buf[i..].starts_with(THINK_CLOSE) {
                if !self.saw_any_marker {
                    // Lenient deepseek_r1 dialect (Ollama/llama.cpp): the open
                    // tag may be omitted (or live in the prefill); everything
                    // before the first `</think>` is reasoning.
                    out.reasoning.push_str(&buf[seg_start..i]);
                    self.saw_any_marker = true;
                    i = self.advance_after_close(&buf, i + THINK_CLOSE.len());
                    seg_start = i;
                } else {
                    // A stray close outside a think block (the model quoting
                    // the tag): keep it verbatim as content.
                    i += THINK_CLOSE.len();
                }
                continue;
            }
            if buf[i..].starts_with(RESPONSE_CLOSE) {
                // Trailing `</response>` after the answer: strip silently.
                out.content.push_str(&buf[seg_start..i]);
                i += RESPONSE_CLOSE.len();
                seg_start = i;
                continue;
            }
            i += 1;
        }
        let cut = partial_marker_cut(&buf, seg_start);
        if self.in_think {
            out.reasoning.push_str(&buf[seg_start..cut]);
        } else {
            out.content.push_str(&buf[seg_start..cut]);
        }
        if cut < buf.len() {
            self.pending = buf[cut..].to_string();
        }
        out
    }

    /// Flush the held tail (stream ended). An unclosed ` ` block ends as
    /// reasoning, never as visible content.
    pub(crate) fn finish(&mut self) -> MarkerSplit {
        if self.pending.is_empty() {
            return MarkerSplit::default();
        }
        let mut out = MarkerSplit::default();
        if self.in_think {
            out.reasoning = std::mem::take(&mut self.pending);
        } else {
            out.content = std::mem::take(&mut self.pending);
        }
        out
    }

    pub(crate) fn saw_markers(&self) -> bool {
        self.saw_any_marker
    }
}

/// One-shot split for a complete (non-streaming) reply body.
pub(crate) fn split_marker_text(text: &str) -> MarkerSplit {
    let mut splitter = MarkerSplitter::default();
    let mut out = splitter.feed(text);
    let tail = splitter.finish();
    out.reasoning.push_str(&tail.reasoning);
    out.content.push_str(&tail.content);
    out
}

/// How a stream carries reasoning: unknown until proven, then either the
/// DeepSeek protocol field or raw markers inside `content`.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MarkerMode {
    #[default]
    Unknown,
    /// A `reasoning_content` delta was seen: content is already clean.
    Protocol,
    /// ` ` markers were seen inside `content`: split them.
    RawMarkers,
}

/// One tool call being assembled from streamed deltas. The id and name usually
/// arrive in the first chunk for a given index; `arguments` streams as a series
/// of string fragments that concatenate into the final JSON.
#[derive(Default, Clone)]
pub(crate) struct PartialToolCall {
    id: String,
    name: String,
    args: String,
}

/// Accumulates a streamed chat completion: text content plus tool-call deltas,
/// keyed by their `index`. Pure (no IO) so it's unit-testable against canned
/// chunk sequences.
#[derive(Default)]
pub(crate) struct StreamAccumulator {
    pub(crate) content: String,
    reasoning: String,
    pub(crate) tool_calls: Vec<PartialToolCall>,
    pub(crate) finish_reason: Option<String>,
    marker_mode: MarkerMode,
    splitter: MarkerSplitter,
    /// Seeded from the process-wide cache on first content delta so a hop
    /// stays consistent even if tests resync mid-stream.
    markers_on: Option<bool>,
}

/// What a single chunk contributed: visible content and/or private reasoning,
/// each surfaced as soon as it streams so the UI updates live.
#[derive(Default)]
pub(crate) struct ChunkDelta {
    pub(crate) content: Option<String>,
    pub(crate) reasoning: Option<String>,
    pub(crate) model_activity: bool,
}

fn push_delta(slot: &mut Option<String>, add: &str) {
    match slot {
        Some(existing) => existing.push_str(add),
        None => *slot = Some(add.to_string()),
    }
}

impl StreamAccumulator {
    /// Whether this stream has demonstrably begun producing model output. This
    /// deliberately includes private reasoning and a partial tool-call envelope:
    /// local Qwen/SGLang parsers can emit either, then buffer a large string
    /// argument until its closing delimiter before another SSE chunk appears.
    pub(crate) fn has_model_output(&self) -> bool {
        !self.content.is_empty() || !self.reasoning.is_empty() || !self.tool_calls.is_empty()
    }

    /// Whether any partial tool call carries real content (an id, a name, or
    /// argument bytes). A blank envelope (`[{}]`, `[{"index":0}]`) reserves a
    /// slot but holds nothing, so it must not be reported as a severed call —
    /// and, unlike one, it never stands between kept prose and the surface. The
    /// reply builder already drops unnamed slots, so this matches what dispatch
    /// would have done with them.
    pub(crate) fn has_usable_tool_call(&self) -> bool {
        self.tool_calls
            .iter()
            .any(|call| !call.id.is_empty() || !call.name.is_empty() || !call.args.is_empty())
    }

    fn marker_gate(&mut self) -> bool {
        *self.markers_on.get_or_insert_with(marker_splitting_enabled)
    }

    /// Move the private reasoning out before collapsing the public reply. Some
    /// provider protocols (currently DeepSeek V4 thinking+tools) require the
    /// transport to replay it on the next hop even though generic ChatMsg
    /// history deliberately does not persist chain-of-thought.
    pub(crate) fn take_reasoning(&mut self) -> String {
        let tail = self.splitter.finish();
        if !tail.content.is_empty() {
            self.content.push_str(&tail.content);
        }
        if !tail.reasoning.is_empty() {
            self.reasoning.push_str(&tail.reasoning);
        }
        std::mem::take(&mut self.reasoning)
    }

    /// Fold one chunk in. Returns the deltas (content / reasoning) so the caller
    /// can forward them to the live sinks immediately.
    pub(crate) fn apply_chunk(&mut self, chunk: &serde_json::Value) -> ChunkDelta {
        let mut out = ChunkDelta::default();
        let Some(choice) = chunk.get("choices").and_then(|c| c.get(0)) else {
            return out;
        };
        // Streaming chunks carry `delta`; a server that ignored `stream:true`
        // answers with a full `message` (non-streaming shape). Absorb either with
        // the same code so the streaming path degrades gracefully to one chunk.
        if let Some(delta) = choice.get("delta").or_else(|| choice.get("message")) {
            let reasoning = response_reasoning_delta(delta);
            if reasoning.is_some() && self.marker_mode == MarkerMode::Unknown {
                // Decide before processing content: a buffered response can
                // carry reasoning and literal <think> documentation together.
                self.marker_mode = MarkerMode::Protocol;
                let tail = self.splitter.finish();
                if !tail.content.is_empty() {
                    self.content.push_str(&tail.content);
                    push_delta(&mut out.content, &tail.content);
                }
                if !tail.reasoning.is_empty() {
                    self.reasoning.push_str(&tail.reasoning);
                    push_delta(&mut out.reasoning, &tail.reasoning);
                }
            }
            if let Some(c) = delta.get("content").and_then(|v| v.as_str())
                && !c.is_empty()
            {
                if self.marker_mode == MarkerMode::Protocol || !self.marker_gate() {
                    self.content.push_str(c);
                    push_delta(&mut out.content, c);
                } else {
                    let sp = self.splitter.feed(c);
                    if self.splitter.saw_markers() {
                        self.marker_mode = MarkerMode::RawMarkers;
                    }
                    if !sp.reasoning.is_empty() {
                        self.reasoning.push_str(&sp.reasoning);
                        push_delta(&mut out.reasoning, &sp.reasoning);
                    }
                    if !sp.content.is_empty() {
                        self.content.push_str(&sp.content);
                        push_delta(&mut out.content, &sp.content);
                    }
                }
            }
            // Reasoning models stream their chain-of-thought separately.
            if let Some(r) = reasoning.as_deref()
                && !r.is_empty()
            {
                self.reasoning.push_str(r);
                push_delta(&mut out.reasoning, r);
            }
            if let Some(tcs) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                // Only a frame that actually carries part of a call proves the
                // model is generating, and only *new* information counts: a
                // blank envelope (`[{}]`, `[{"index":0}]`), or the same id/name
                // repeated without another argument byte, is a keep-alive in
                // tool-call clothing. Counting it as progress would hold a
                // silent stream open past its data deadline and let a blank ping
                // pass for completion. This is a liveness rule only — it never
                // stops a task or trims a budget.
                let mut contributed = false;
                // A real response carries a handful of tool calls; a malformed or
                // hostile stream can send `index: 100000000` (or u64::MAX), and the
                // grow loop below would allocate idx+1 slots (~72 bytes each) —
                // gigabytes, aborting the process. Ignore any index past a sane
                // ceiling rather than let the server dictate the allocation.
                const MAX_STREAM_TOOL_CALLS: usize = 1024;
                for tc in tcs {
                    let idx = tc.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    if idx >= MAX_STREAM_TOOL_CALLS {
                        continue;
                    }
                    while self.tool_calls.len() <= idx {
                        self.tool_calls.push(PartialToolCall::default());
                    }
                    let slot = &mut self.tool_calls[idx];
                    if let Some(id) = tc.get("id").and_then(|v| v.as_str())
                        && !id.is_empty()
                    {
                        contributed |= slot.id.is_empty();
                        slot.id = id.to_string();
                    }
                    let func_or_tc = tc.get("function").unwrap_or(tc);
                    if let Some(n) = func_or_tc.get("name").and_then(|v| v.as_str())
                        && !n.is_empty()
                    {
                        contributed |= slot.name.is_empty();
                        slot.name = n.to_string();
                    }
                    if let Some(a) = func_or_tc.get("arguments") {
                        if let Some(s) = a.as_str() {
                            slot.args.push_str(s);
                            contributed |= !s.is_empty();
                        } else if a.is_object() || a.is_array() {
                            slot.args.push_str(&a.to_string());
                            contributed = true;
                        }
                    }
                }
                out.model_activity |= contributed;
            }
        }
        if let Some(fr) = choice.get("finish_reason").and_then(|v| v.as_str()) {
            self.finish_reason = Some(fr.to_string());
            out.model_activity = true;
        }
        out.model_activity |= out.content.is_some() || out.reasoning.is_some();
        out
    }

    /// Collapse the accumulated state into a final reply: tool calls if any were
    /// requested (named), otherwise the text content. `tools_offered` gates the
    /// prose-recovery fallback: when the request carried no tools, nothing the
    /// text "calls" could ever execute, so a wrapper-looking passage (a draft
    /// quoting `<tool_call>` syntax, say) must stay prose — recovering it would
    /// turn a usable answer into an unactionable Calls reply.
    pub(crate) fn into_reply(mut self, tools_offered: bool) -> ClubReply {
        let tail = self.splitter.finish();
        if !tail.content.is_empty() {
            self.content.push_str(&tail.content);
        }
        if !tail.reasoning.is_empty() {
            self.reasoning.push_str(&tail.reasoning);
        }
        let calls: Vec<ToolCall> = self
            .tool_calls
            .into_iter()
            .filter(|t| !t.name.is_empty())
            .map(|t| {
                let args = parsed_tool_args(&t.id, &t.args);
                ToolCall {
                    id: t.id,
                    name: t.name,
                    args,
                }
            })
            .collect();
        if calls.is_empty() {
            // No structured calls — but a weak/non-jinja model may have written
            // the call into the text. Recover it (explicit wrappers only) rather
            // than mistaking a tool call for a final answer.
            if tools_offered {
                let recovered = extract_prose_tool_calls(&self.content);
                if !recovered.is_empty() {
                    return ClubReply::Calls(recovered);
                }
            }
            ClubReply::Text(self.content)
        } else {
            ClubReply::Calls(calls)
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/club/sse__tool_call_index_stress.rs"]
mod tool_call_index_stress;

#[cfg(test)]
#[path = "../../../tests/cockpit/club/sse__marker_tests.rs"]
mod marker_tests;
