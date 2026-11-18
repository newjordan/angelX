//! SSE streaming: decoding OpenAI chat-completions `stream: true` bodies.

use super::*;

pub(crate) struct CancelReadGuard<'scope> {
    stop: std::sync::mpsc::Sender<()>,
    worker: Option<std::thread::ScopedJoinHandle<'scope, ()>>,
}

impl<'scope> CancelReadGuard<'scope> {
    pub(crate) fn new<'env>(
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

pub(crate) enum SseEvent {
    Chunk(serde_json::Value),
    Done,
    Ignore,
}

pub(crate) fn parse_sse_line(line: &str) -> SseEvent {
    let line = line.trim();
    if line.is_empty() || line.starts_with(':') {
        return SseEvent::Ignore;
    }
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

const THINK_OPEN: &str = "<think>";
const THINK_CLOSE: &str = "</think>";
const RESPONSE_OPEN: &str = "<response>";
const RESPONSE_CLOSE: &str = "</response>";
const MARKERS: [&str; 4] = [THINK_OPEN, THINK_CLOSE, RESPONSE_OPEN, RESPONSE_CLOSE];
const MAX_MARKER_PREFIX: usize = RESPONSE_CLOSE.len() - 1;

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

#[cfg(test)]
fn set_marker_splitting_enabled(on: bool) {
    MARKER_SPLITTING_SEEDED.store(true, Ordering::Relaxed);
    MARKER_SPLITTING_ENABLED.store(on, Ordering::Relaxed);
}

#[cfg(test)]
fn resync_marker_splitting_from_env() {
    MARKER_SPLITTING_SEEDED.store(false, Ordering::Relaxed);
    seed_marker_splitting_from_env();
}

#[derive(Default)]
pub(crate) struct MarkerSplit {
    pub(crate) reasoning: String,
    pub(crate) content: String,
    pub(crate) lenient_close: bool,
}

#[derive(Default)]
pub(crate) struct MarkerSplitter {
    in_think: bool,
    saw_any_marker: bool,
    expect_response_open: bool,
    skip_leading_ws: bool,
    pending: String,
}

fn skip_ascii_ws(buf: &str, mut i: usize) -> usize {
    let bytes = buf.as_bytes();
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\n' | b'\r' | b'\t') {
        i += 1;
    }
    i
}

fn is_partial_marker_prefix(s: &str) -> bool {
    !s.is_empty()
        && MARKERS
            .iter()
            .any(|m| m.len() > s.len() && m.starts_with(s))
}

fn partial_marker_cut(buf: &str, from: usize) -> usize {
    let mut start = from.max(buf.len().saturating_sub(MAX_MARKER_PREFIX));
    while start < buf.len() && !buf.is_char_boundary(start) {
        start += 1;
    }
    for (offset, _) in buf[start..].match_indices('<') {
        let p = start + offset;
        if is_partial_marker_prefix(&buf[p..]) {
            return p;
        }
    }
    buf.len()
}

impl MarkerSplitter {
    fn advance_after_close(&mut self, buf: &str, i: usize) -> usize {
        let j = skip_ascii_ws(buf, i);
        if buf[j..].starts_with(RESPONSE_OPEN) {
            let k = skip_ascii_ws(buf, j + RESPONSE_OPEN.len());
            self.skip_leading_ws = k == buf.len();
            return k;
        }
        self.expect_response_open = buf[j..].is_empty() || is_partial_marker_prefix(&buf[j..]);
        j
    }

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
            self.expect_response_open = false;
            i = self.advance_after_close(&buf, 0);
            seg_start = i;
        } else if self.skip_leading_ws {
            i = skip_ascii_ws(&buf, 0);
            seg_start = i;
            self.skip_leading_ws = i == buf.len();
        }
        while i < buf.len() {
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
                    out.reasoning.push_str(&buf[seg_start..i]);
                    out.lenient_close = true;
                    self.saw_any_marker = true;
                    i = self.advance_after_close(&buf, i + THINK_CLOSE.len());
                    seg_start = i;
                } else {
                    i += THINK_CLOSE.len();
                }
                continue;
            }
            if buf[i..].starts_with(RESPONSE_CLOSE) {
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

pub(crate) fn split_marker_text(text: &str) -> MarkerSplit {
    let mut splitter = MarkerSplitter::default();
    let mut out = splitter.feed(text);
    let tail = splitter.finish();
    out.reasoning.push_str(&tail.reasoning);
    out.content.push_str(&tail.content);
    out
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MarkerMode {
    #[default]
    Unknown,
    Protocol,
    RawMarkers,
}

#[derive(Default, Clone)]
pub(crate) struct PartialToolCall {
    id: String,
    name: String,
    args: String,
    extra_content: Option<serde_json::Value>,
}

#[derive(Default)]
pub(crate) struct StreamAccumulator {
    pub(crate) content: String,
    reasoning: String,
    pub(crate) tool_calls: Vec<PartialToolCall>,
    pub(crate) finish_reason: Option<String>,
    marker_mode: MarkerMode,
    splitter: MarkerSplitter,
    markers_on: Option<bool>,
    cumulative_tool_args: bool,
}

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

fn absorb_tool_arguments(slot: &mut String, incoming: &str, cumulative: bool) -> bool {
    if incoming.is_empty() {
        return false;
    }
    if !cumulative {
        slot.push_str(incoming);
        return true;
    }
    if slot.is_empty() || incoming.starts_with(slot.as_str()) {
        if incoming.len() == slot.len() {
            return false;
        }
        *slot = incoming.to_string();
        return true;
    }
    if slot.starts_with(incoming) || incoming.len() < slot.len() {
        return false;
    }
    *slot = incoming.to_string();
    true
}

impl StreamAccumulator {
    pub(crate) fn with_cumulative_tool_args(mut self) -> Self {
        self.cumulative_tool_args = true;
        self
    }

    fn marker_gate(&mut self) -> bool {
        *self.markers_on.get_or_insert_with(marker_splitting_enabled)
    }

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

    pub(crate) fn apply_chunk(&mut self, chunk: &serde_json::Value) -> ChunkDelta {
        let mut out = ChunkDelta::default();
        let Some(choice) = chunk.get("choices").and_then(|c| c.get(0)) else {
            return out;
        };
        if let Some(delta) = choice.get("delta").or_else(|| choice.get("message")) {
            let reasoning = response_reasoning_delta(delta);
            if reasoning.is_some() && self.marker_mode == MarkerMode::Unknown {
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
                    if sp.lenient_close && !self.content.is_empty() {
                        let earlier = std::mem::take(&mut self.content);
                        self.reasoning.push_str(&earlier);
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
            if let Some(r) = reasoning.as_deref()
                && !r.is_empty()
            {
                self.reasoning.push_str(r);
                push_delta(&mut out.reasoning, r);
            }
            if let Some(tcs) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                let mut contributed = false;
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
                    if let Some(extra) = tc.get("extra_content")
                        && !extra.is_null()
                    {
                        slot.extra_content = Some(extra.clone());
                    }
                    let func_or_tc = tc.get("function").unwrap_or(tc);
                    if let Some(n) = func_or_tc.get("name").and_then(|v| v.as_str())
                        && !n.is_empty()
                    {
                        contributed |= slot.name.is_empty();
                        slot.name = n.to_string();
                    }
                    if let Some(a) = func_or_tc.get("arguments") {
                        let rendered;
                        let incoming = if let Some(s) = a.as_str() {
                            Some(s)
                        } else if a.is_object() || a.is_array() {
                            rendered = a.to_string();
                            Some(rendered.as_str())
                        } else {
                            None
                        };
                        if let Some(incoming) = incoming {
                            contributed |= absorb_tool_arguments(
                                &mut slot.args,
                                incoming,
                                self.cumulative_tool_args,
                            );
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

    pub(crate) fn tool_call_extras(&self) -> Vec<(String, serde_json::Value)> {
        self.tool_calls
            .iter()
            .filter(|t| !t.id.is_empty())
            .filter_map(|t| t.extra_content.clone().map(|extra| (t.id.clone(), extra)))
            .collect()
    }

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
            if tools_offered {
                let recovered = extract_prose_tool_calls(&self.content);
                if !recovered.is_empty() {
                    return ClubReply::Calls(recovered);
                }
            }
            ClubReply::Text(self.content)
        } else {
            let content = (!self.content.trim().is_empty()).then(|| self.content.clone());
            crate::agent::club::set_pending_tool_content(content);
            ClubReply::Calls(calls)
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/club/sse__tool_call_index_stress.rs"]
mod tool_call_index_stress;

#[cfg(test)]
#[path = "../../../../tests/cockpit/club/sse__marker_tests.rs"]
mod marker_tests;
