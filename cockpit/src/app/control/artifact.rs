//! Streaming-artifact detection: fenced/HTML code documents in replies.
use super::*;

pub(crate) const STREAM_ARTIFACT_THRESHOLD_CHARS: usize = 1_600;
pub(crate) const STREAM_ARTIFACT_EARLY_CHARS: usize = 240;
pub(crate) const ARTIFACT_NOTE_CONTEXT_CHARS: usize = 1_200;

/// Incremental equivalent of the two live artifact predicates. Retains only
/// marker overlap and first-fence metadata; work scales with the new delta.
#[derive(Default)]
pub(crate) struct StreamArtifactDetector {
    chars: usize,
    bytes: usize,
    tail: String,
    html: bool,
    fence: u8, // 0: seeking, 1: info, 2: body, 3: closed
    ticks: usize,
    info: String,
    info_space: bool,
    info_invalid: bool,
    info_html: bool,
    info_tail: String,
    file_like: bool,
    fence_newlines: usize,
    ends_newline: bool,
    body_chars: usize,
}

fn marker_overlap(tail: &mut String, delta: &str, markers: &[&str]) -> bool {
    let mut text = std::mem::take(tail);
    text.push_str(delta);
    text.make_ascii_lowercase();
    let found = markers.iter().any(|marker| text.contains(marker));
    let mut start = text.len().saturating_sub(16);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    tail.push_str(&text[start..]);
    found
}

impl StreamArtifactDetector {
    pub(crate) fn covers(&self, text: &str) -> bool {
        self.bytes == text.len()
    }

    pub(crate) fn push(&mut self, delta: &str) -> bool {
        self.bytes += delta.len();
        self.html |= marker_overlap(
            &mut self.tail,
            delta,
            &[
                "<!doctype html",
                "<html",
                "<canvas",
                "<script",
                "</body></html>",
            ],
        );
        for c in delta.chars() {
            self.chars += 1;
            if self.fence > 0 {
                self.fence_newlines += usize::from(c == '\n');
                self.ends_newline = c == '\n';
            }
            match self.fence {
                0 => {
                    self.ticks = if c == '`' { self.ticks + 1 } else { 0 };
                    if self.ticks == 3 {
                        self.fence = 1;
                        self.ticks = 0;
                    }
                }
                1 => {
                    let mut bytes = [0; 4];
                    self.info_html |=
                        marker_overlap(&mut self.info_tail, c.encode_utf8(&mut bytes), &["html"]);
                    if c == '\n' {
                        self.file_like = !self.info_invalid
                            && matches!(
                                self.info.as_str(),
                                "html"
                                    | "htm"
                                    | "css"
                                    | "js"
                                    | "javascript"
                                    | "ts"
                                    | "typescript"
                                    | "jsx"
                                    | "tsx"
                                    | "rust"
                                    | "rs"
                                    | "python"
                                    | "py"
                                    | "json"
                                    | "toml"
                                    | "yaml"
                                    | "yml"
                                    | "sh"
                                    | "bash"
                            );
                        self.fence = 2;
                    } else if c.is_whitespace() {
                        self.info_space |= !self.info.is_empty();
                    } else {
                        self.info_invalid |= self.info_space || self.info.len() >= 16;
                        if !self.info_invalid {
                            self.info.push(c.to_ascii_lowercase());
                        }
                    }
                }
                2 => {
                    self.body_chars += 1;
                    self.ticks = if c == '`' { self.ticks + 1 } else { 0 };
                    if self.ticks == 3 {
                        self.body_chars -= 3;
                        self.fence = 3;
                    }
                }
                _ => {}
            }
        }
        let lines = self.fence_newlines + usize::from(!self.ends_newline);
        (self.chars >= STREAM_ARTIFACT_EARLY_CHARS
            && (self.html || (self.file_like && lines >= 10)))
            || (self.chars >= STREAM_ARTIFACT_THRESHOLD_CHARS
                && self.fence == 3
                && (self.info_html || self.body_chars >= STREAM_ARTIFACT_THRESHOLD_CHARS))
    }
}

pub(crate) struct CodeDocument<'a> {
    kind: CodeDocumentKind,
    pub(crate) body: &'a str,
    before: &'a str,
    after: &'a str,
}

#[derive(Clone, Copy)]
pub(crate) enum CodeDocumentKind {
    Html,
    Code,
}

impl CodeDocumentKind {
    pub(crate) fn extension(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Code => "txt",
        }
    }

    pub(crate) fn label(self) -> String {
        match self {
            Self::Html => "generated HTML document".to_string(),
            Self::Code => "generated code document".to_string(),
        }
    }
}

impl CodeDocument<'_> {
    pub(crate) fn extension(&self) -> &'static str {
        self.kind.extension()
    }

    pub(crate) fn label(&self) -> String {
        self.kind.label()
    }

    pub(crate) fn context_note(&self) -> Option<String> {
        let context = [self.before.trim(), self.after.trim()]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        if context.is_empty() {
            None
        } else {
            Some(cap_chars(&context, ARTIFACT_NOTE_CONTEXT_CHARS))
        }
    }
}

pub(crate) fn large_code_document(text: &str) -> Option<CodeDocument<'_>> {
    if text.chars().count() < STREAM_ARTIFACT_THRESHOLD_CHARS {
        return None;
    }
    if let Some(doc) = fenced_code_document(text) {
        return Some(doc);
    }
    let lower = text.to_ascii_lowercase();
    let htmlish = lower.contains("<!doctype html")
        || (lower.contains("<html") && (lower.contains("<script") || lower.contains("<canvas")));
    htmlish.then_some(CodeDocument {
        kind: CodeDocumentKind::Html,
        body: text,
        before: "",
        after: "",
    })
}

#[cfg(test)]
pub(crate) fn probable_streaming_artifact(text: &str) -> bool {
    if text.chars().count() < STREAM_ARTIFACT_EARLY_CHARS {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    let complete_html = lower.contains("<!doctype html")
        || lower.contains("<html")
        || lower.contains("<canvas")
        || lower.contains("<script")
        || lower.contains("</body></html>");
    if complete_html {
        return true;
    }
    let Some(fence_start) = lower.find("```") else {
        return false;
    };
    let info = lower[fence_start + 3..].lines().next().unwrap_or("").trim();
    let file_like_fence = matches!(
        info,
        "html"
            | "htm"
            | "css"
            | "js"
            | "javascript"
            | "ts"
            | "typescript"
            | "jsx"
            | "tsx"
            | "rust"
            | "rs"
            | "python"
            | "py"
            | "json"
            | "toml"
            | "yaml"
            | "yml"
            | "sh"
            | "bash"
    );
    file_like_fence && text[fence_start..].lines().count() >= 10
}

pub(crate) fn fenced_code_document(text: &str) -> Option<CodeDocument<'_>> {
    let fence_start = text.find("```")?;
    let info_start = fence_start + 3;
    let rel_info_end = text[info_start..].find('\n')?;
    let info_end = info_start + rel_info_end;
    let info = text[info_start..info_end].trim().to_ascii_lowercase();
    let body_start = info_end + 1;
    let rel_body_end = text[body_start..].find("```")?;
    let body_end = body_start + rel_body_end;
    let body = &text[body_start..body_end];
    let body_lower = body.to_ascii_lowercase();
    let kind = if info.contains("html")
        || body_lower.contains("<!doctype html")
        || (body_lower.contains("<html") && body_lower.contains("<script"))
    {
        CodeDocumentKind::Html
    } else if body.chars().count() >= STREAM_ARTIFACT_THRESHOLD_CHARS {
        CodeDocumentKind::Code
    } else {
        return None;
    };
    Some(CodeDocument {
        kind,
        body,
        before: &text[..fence_start],
        after: &text[body_end + 3..],
    })
}

pub(crate) fn cap_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push_str("\n…");
    out
}

/// One named background job owned by the cockpit's single flight slot.
///
/// Keeping the receiver together with trusted operation/retry labels lets the UI
/// report a vanished worker instead of silently releasing the slot. Cancellation
/// is logical: the receiver is dropped immediately and the shared flag prevents a
/// late worker reply from becoming observable after Esc/^C.
pub(crate) struct BackgroundJob {
    receiver: mpsc::Receiver<BgOutcome>,
    operation: &'static str,
    retry: &'static str,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    phase: Arc<std::sync::atomic::AtomicUsize>,
    phase_labels: &'static [&'static str],
    started: std::time::Instant,
    live_output: Arc<std::sync::Mutex<BackgroundLiveOutput>>,
}

/// Cancellation-aware sender paired with [`BackgroundJob`].
pub(crate) struct BackgroundReply {
    sender: mpsc::Sender<BgOutcome>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    phase: Arc<std::sync::atomic::AtomicUsize>,
    live_output: Arc<std::sync::Mutex<BackgroundLiveOutput>>,
}

const BACKGROUND_LIVE_STREAM_BYTES: usize = 6 * 1024;
const BACKGROUND_LIVE_RENDER_BYTES: usize = BACKGROUND_LIVE_STREAM_BYTES * 2;

#[derive(Default)]
struct BackgroundByteTail {
    bytes: std::collections::VecDeque<u8>,
    omitted: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BackgroundOutputStatus {
    pub(crate) stdout_retained: usize,
    pub(crate) stdout_omitted: u64,
    pub(crate) stderr_retained: usize,
    pub(crate) stderr_omitted: u64,
    pub(crate) last_chunk_age: Option<std::time::Duration>,
}

impl BackgroundByteTail {
    fn push(&mut self, bytes: &[u8]) {
        self.bytes.extend(bytes);
        let overflow = self
            .bytes
            .len()
            .saturating_sub(BACKGROUND_LIVE_STREAM_BYTES);
        if overflow > 0 {
            self.bytes.drain(..overflow);
            self.omitted = self.omitted.saturating_add(overflow as u64);
        }
    }

    fn retained_bytes(&self) -> Vec<u8> {
        self.bytes.iter().copied().collect()
    }

    fn seen_bytes(&self) -> u64 {
        self.omitted.saturating_add(self.bytes.len() as u64)
    }

    fn status(&self) -> (usize, u64) {
        (self.bytes.len(), self.omitted)
    }
}

fn sanitize_live_terminal_bytes(bytes: &[u8]) -> String {
    #[derive(Clone, Copy)]
    enum Escape {
        Ground,
        Start,
        Intermediate,
        Csi,
        Osc,
        OscEsc,
        String,
        StringEsc,
    }

    let mut plain = Vec::with_capacity(bytes.len());
    let mut state = Escape::Ground;
    for &byte in bytes {
        // CAN/SUB terminate an in-flight control sequence. The chronological
        // stream merger inserts CAN at pipe boundaries so an unterminated
        // escape from one child pipe cannot consume another pipe's label/data.
        if matches!(byte, 0x18 | 0x1a) {
            state = Escape::Ground;
            continue;
        }
        state = match state {
            Escape::Ground if byte == 0x1b => Escape::Start,
            Escape::Ground => {
                plain.push(byte);
                Escape::Ground
            }
            Escape::Start => match byte {
                b'[' => Escape::Csi,
                b']' => Escape::Osc,
                b'P' | b'X' | b'^' | b'_' => Escape::String,
                0x20..=0x2f => Escape::Intermediate,
                _ => Escape::Ground,
            },
            Escape::Intermediate => {
                if (0x30..=0x7e).contains(&byte) {
                    Escape::Ground
                } else {
                    Escape::Intermediate
                }
            }
            Escape::Csi => {
                if (0x40..=0x7e).contains(&byte) {
                    Escape::Ground
                } else {
                    Escape::Csi
                }
            }
            Escape::Osc if byte == 0x07 => Escape::Ground,
            Escape::Osc if byte == 0x1b => Escape::OscEsc,
            Escape::Osc => Escape::Osc,
            Escape::OscEsc if byte == b'\\' => Escape::Ground,
            Escape::OscEsc => Escape::Osc,
            Escape::String if byte == 0x1b => Escape::StringEsc,
            Escape::String => Escape::String,
            Escape::StringEsc if byte == b'\\' => Escape::Ground,
            Escape::StringEsc => Escape::String,
        };
    }

    let decoded = String::from_utf8_lossy(&plain);
    let mut out = String::with_capacity(decoded.len());
    let mut chars = decoded.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '\n' | '\t' => out.push(character),
            '\r' => {
                if chars.peek() != Some(&'\n') {
                    out.push('\n');
                }
            }
            character if character.is_control() || unsafe_live_format(character) => {}
            _ => out.push(character),
        }
    }
    out
}

fn unsafe_live_format(character: char) -> bool {
    matches!(
        character,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

#[derive(Clone, Copy)]
struct BackgroundOutputChunk {
    stream: crate::agent::harness::ProcessStream,
    start: u64,
    end: u64,
}

#[derive(Default)]
struct BackgroundLiveOutput {
    stdout: BackgroundByteTail,
    stderr: BackgroundByteTail,
    chunks: std::collections::VecDeque<BackgroundOutputChunk>,
    cached: Option<Arc<str>>,
    dirty: bool,
    last_chunk: Option<std::time::Instant>,
}

impl BackgroundLiveOutput {
    fn push(&mut self, stream: crate::agent::harness::ProcessStream, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let tail = match stream {
            crate::agent::harness::ProcessStream::Stdout => &mut self.stdout,
            crate::agent::harness::ProcessStream::Stderr => &mut self.stderr,
        };
        let start = tail.seen_bytes();
        tail.push(bytes);
        let end = start.saturating_add(bytes.len() as u64);
        let extends_last = self
            .chunks
            .back()
            .is_some_and(|last| last.stream == stream && last.end == start);
        if extends_last {
            self.chunks.back_mut().expect("checked above").end = end;
        } else {
            self.chunks
                .push_back(BackgroundOutputChunk { stream, start, end });
        }
        // Hysteresis keeps a pathological one-byte stream alternation from
        // turning every callback after saturation into an O(n) compaction.
        if self.chunks.len() > BACKGROUND_LIVE_STREAM_BYTES * 4 {
            self.prune_chunks();
        }
        self.dirty = true;
        self.last_chunk = Some(std::time::Instant::now());
    }

    fn snapshot(&mut self) -> Option<Arc<str>> {
        if !self.dirty {
            return self.cached.clone();
        }
        self.prune_chunks();
        let stdout = self.stdout.retained_bytes();
        let stderr = self.stderr.retained_bytes();
        let mut budget = BACKGROUND_LIVE_RENDER_BYTES;
        let mut render_start = 0;
        for (index, chunk) in self.chunks.iter().enumerate().rev() {
            let (retained_len, omitted) = match chunk.stream {
                crate::agent::harness::ProcessStream::Stdout => {
                    (stdout.len() as u64, self.stdout.omitted)
                }
                crate::agent::harness::ProcessStream::Stderr => {
                    (stderr.len() as u64, self.stderr.omitted)
                }
            };
            let retained_end = omitted.saturating_add(retained_len);
            let start = chunk.start.max(omitted);
            let end = chunk.end.min(retained_end);
            let cost = (end.saturating_sub(start) as usize).saturating_add(12);
            if cost > budget {
                render_start = index.saturating_add(1);
                break;
            }
            budget -= cost;
        }
        let mut raw = Vec::with_capacity(BACKGROUND_LIVE_RENDER_BYTES.saturating_add(160));
        if render_start > 0 {
            raw.extend_from_slice("…[earlier interleaved output omitted]…\n".as_bytes());
        }
        let mut rendered_stream = None;
        let mut stdout_omission_announced = false;
        let mut stderr_omission_announced = false;
        for chunk in self.chunks.iter().skip(render_start) {
            let (retained, omitted, omission_announced) = match chunk.stream {
                crate::agent::harness::ProcessStream::Stdout => (
                    stdout.as_slice(),
                    self.stdout.omitted,
                    &mut stdout_omission_announced,
                ),
                crate::agent::harness::ProcessStream::Stderr => (
                    stderr.as_slice(),
                    self.stderr.omitted,
                    &mut stderr_omission_announced,
                ),
            };
            let retained_end = omitted.saturating_add(retained.len() as u64);
            let start = chunk.start.max(omitted);
            let end = chunk.end.min(retained_end);
            if start >= end {
                continue;
            }
            if rendered_stream != Some(chunk.stream) {
                if rendered_stream.is_some()
                    || chunk.stream == crate::agent::harness::ProcessStream::Stderr
                {
                    raw.push(0x18);
                    let label = match (rendered_stream, chunk.stream) {
                        (None, crate::agent::harness::ProcessStream::Stderr) => {
                            b"[stderr]\n".as_slice()
                        }
                        (_, crate::agent::harness::ProcessStream::Stdout) => {
                            b"\n[stdout]\n".as_slice()
                        }
                        (_, crate::agent::harness::ProcessStream::Stderr) => {
                            b"\n[stderr]\n".as_slice()
                        }
                    };
                    raw.extend_from_slice(label);
                }
                rendered_stream = Some(chunk.stream);
            }
            if !*omission_announced && omitted > 0 {
                raw.push(0x18);
                raw.extend_from_slice(
                    format!(
                        "…[{omitted} earlier bytes omitted from {}]…\n",
                        match chunk.stream {
                            crate::agent::harness::ProcessStream::Stdout => "stdout",
                            crate::agent::harness::ProcessStream::Stderr => "stderr",
                        }
                    )
                    .as_bytes(),
                );
                *omission_announced = true;
            }
            let slice_start = (start - omitted) as usize;
            let slice_end = (end - omitted) as usize;
            raw.extend_from_slice(&retained[slice_start..slice_end]);
        }
        let snapshot = (!raw.is_empty()).then(|| sanitize_live_terminal_bytes(&raw));
        self.cached = snapshot.map(Arc::<str>::from);
        self.dirty = false;
        self.cached.clone()
    }

    fn prune_chunks(&mut self) {
        let stdout_start = self.stdout.omitted;
        let stderr_start = self.stderr.omitted;
        self.chunks.retain(|chunk| {
            chunk.end
                > match chunk.stream {
                    crate::agent::harness::ProcessStream::Stdout => stdout_start,
                    crate::agent::harness::ProcessStream::Stderr => stderr_start,
                }
        });
    }

    fn status(&self) -> BackgroundOutputStatus {
        let (stdout_retained, stdout_omitted) = self.stdout.status();
        let (stderr_retained, stderr_omitted) = self.stderr.status();
        BackgroundOutputStatus {
            stdout_retained,
            stdout_omitted,
            stderr_retained,
            stderr_omitted,
            last_chunk_age: self.last_chunk.map(|arrival| arrival.elapsed()),
        }
    }
}

impl BackgroundJob {
    pub(crate) fn channel(operation: &'static str, retry: &'static str) -> (BackgroundReply, Self) {
        Self::channel_with_phases(operation, retry, &[])
    }

    pub(crate) fn channel_with_phases(
        operation: &'static str,
        retry: &'static str,
        phase_labels: &'static [&'static str],
    ) -> (BackgroundReply, Self) {
        let (sender, receiver) = mpsc::channel();
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let phase = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let live_output = Arc::new(std::sync::Mutex::new(BackgroundLiveOutput::default()));
        (
            BackgroundReply {
                sender,
                cancelled: Arc::clone(&cancelled),
                phase: Arc::clone(&phase),
                live_output: Arc::clone(&live_output),
            },
            Self {
                receiver,
                operation,
                retry,
                cancelled,
                phase,
                phase_labels,
                started: std::time::Instant::now(),
                live_output,
            },
        )
    }

    pub(crate) fn try_recv(&self) -> Result<BgOutcome, mpsc::TryRecvError> {
        self.receiver.try_recv()
    }

    pub(crate) fn operation(&self) -> &'static str {
        self.phase_labels
            .get(self.phase.load(std::sync::atomic::Ordering::Acquire))
            .copied()
            .unwrap_or(self.operation)
    }

    pub(crate) fn elapsed_secs(&self) -> u64 {
        self.started.elapsed().as_secs()
    }

    pub(crate) fn live_output(&self) -> Option<Arc<str>> {
        self.live_output.lock().ok()?.snapshot()
    }

    pub(crate) fn output_status(&self) -> Option<BackgroundOutputStatus> {
        Some(self.live_output.lock().ok()?.status())
    }

    pub(crate) fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Release);
    }

    pub(crate) fn cancellation_message(&self) -> String {
        format!(
            "{} cancelled — flight slot released; any late result will be ignored. {} to run it again",
            self.operation, self.retry
        )
    }

    pub(crate) fn disconnected_message(&self) -> String {
        format!(
            "{} failed — its background worker ended without a result; no cockpit state changed. {}",
            self.operation, self.retry
        )
    }
}

impl BackgroundReply {
    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::Acquire)
    }

    pub(crate) fn cancellation_flag(&self) -> &std::sync::atomic::AtomicBool {
        &self.cancelled
    }

    pub(crate) fn set_phase(&self, phase: usize) {
        self.phase
            .store(phase, std::sync::atomic::Ordering::Release);
    }

    pub(crate) fn output_progress(&self) -> Arc<crate::agent::harness::ToolOutputProgress> {
        let live_output = Arc::clone(&self.live_output);
        Arc::new(move |stream, bytes| {
            if let Ok(mut output) = live_output.lock() {
                output.push(stream, bytes);
            }
        })
    }

    pub(crate) fn clear_output(&self) {
        if let Ok(mut output) = self.live_output.lock() {
            *output = BackgroundLiveOutput::default();
        }
    }

    pub(crate) fn send(&self, outcome: BgOutcome) -> Result<(), ()> {
        if self.is_cancelled() {
            return Err(());
        }
        self.sender.send(outcome).map_err(|_| ())
    }
}

const BACKGROUND_NOTICE_DETAIL_CHARS: usize = 240;
const BACKGROUND_NOTICE_WORKSPACE_CHARS: usize = 64;

/// A detached compaction deposit batch. The UI accepts the history splice
/// before this enters the shared bounded memory queue, so slow infrastructure
/// never keeps the flight slot busy or creates one worker per `/compact`.
/// Results return through [`BackgroundNotice`] instead of disappearing.
pub(crate) struct MemoryDepositBatch {
    store: Arc<dyn crate::knowledge::memory::store::MemoryStore>,
    drawers: Vec<crate::knowledge::memory::store::Drawer>,
    workspace: PathBuf,
}

impl MemoryDepositBatch {
    pub(crate) fn new(
        store: Arc<dyn crate::knowledge::memory::store::MemoryStore>,
        drawers: Vec<crate::knowledge::memory::store::Drawer>,
        workspace: PathBuf,
    ) -> Self {
        Self {
            store,
            drawers,
            workspace,
        }
    }

    pub(crate) fn spawn(self, notices: mpsc::Sender<BackgroundNotice>) {
        let Self {
            store,
            drawers,
            workspace,
        } = self;
        crate::agent::harness::enqueue_memory_drawers_with_feedback(
            store,
            drawers,
            move |summary| {
                let _ = notices.send(BackgroundNotice::MemoryDeposits {
                    workspace,
                    attempted: summary.attempted,
                    filed: summary.filed,
                    first_error: summary.first_error,
                });
            },
        );
    }
}

/// Completion feedback from detached work that no longer owns [`App::bg_job`].
pub(crate) enum BackgroundNotice {
    MemoryDeposits {
        workspace: PathBuf,
        attempted: usize,
        filed: usize,
        first_error: Option<String>,
    },
}

impl BackgroundNotice {
    pub(crate) fn message(&self, current_workspace: &Path) -> String {
        match self {
            Self::MemoryDeposits {
                workspace,
                attempted,
                filed,
                first_error,
            } => {
                let scope = if workspace == current_workspace {
                    String::new()
                } else {
                    let label = workspace
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(|name| {
                            bounded_notice_fragment(name, BACKGROUND_NOTICE_WORKSPACE_CHARS)
                        })
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| "previous workspace".to_string());
                    format!(" for previous workspace “{label}”")
                };
                let filed = (*filed).min(*attempted);
                let failed = attempted.saturating_sub(filed);
                if failed == 0 {
                    format!("long-term memory{scope} · filed {filed}/{attempted} compacted note(s)")
                } else {
                    let detail = first_error
                        .as_deref()
                        .map(|error| bounded_notice_fragment(error, BACKGROUND_NOTICE_DETAIL_CHARS))
                        .filter(|error| !error.is_empty())
                        .map(|error| format!(" · first failure: {error}"))
                        .unwrap_or_default();
                    format!(
                        "long-term memory filing incomplete{scope} · filed {filed}/{attempted} compacted \
                         note(s); {failed} failed{detail}. The inline history summary remains intact"
                    )
                }
            }
        }
    }
}

fn bounded_notice_fragment(value: &str, max_chars: usize) -> String {
    let mut out = String::new();
    let mut chars = 0usize;
    let mut pending_space = false;
    let mut truncated = false;
    for character in value.chars() {
        if character.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if character.is_control() || unsafe_notice_format(character) {
            continue;
        }
        if pending_space {
            if chars >= max_chars {
                truncated = true;
                break;
            }
            out.push(' ');
            chars += 1;
            pending_space = false;
        }
        if chars >= max_chars {
            truncated = true;
            break;
        }
        out.push(character);
        chars += 1;
    }
    if truncated {
        out.push('…');
    }
    out
}

fn unsafe_notice_format(character: char) -> bool {
    matches!(
        character,
        '\u{061c}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{feff}'
    )
}

/// The result of a [`BackgroundJob`], applied on the UI thread in
/// [`App::advance`]. Keeps potentially slow work — compaction/palace persistence, a
/// palace query for `/memories palace` — off the event loop so the TUI stays
/// responsive (renders, scrolls, accepts an interrupt) while it runs.
pub(crate) enum BgOutcome {
    /// Show a system message in the transcript.
    Note(String),
    /// Apply a completed `/compact`: replace `history[range]` with `note`, persist
    /// the thread, and show `message`. The indices were taken when the job was
    /// spawned; `submit` is gated on `bg_job`, so history hasn't shifted.
    Compact {
        range: std::ops::Range<usize>,
        note: Box<ChatMsg>,
        plan: Option<Box<ChatMsg>>,
        /// Memory deposits are deliberately handed back to the UI with the
        /// splice. A cancelled or disconnected job therefore cannot file notes
        /// after the operator has abandoned it.
        deposits: Option<MemoryDepositBatch>,
        message: String,
    },
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/app_control__artifact__incremental_artifact_tests.rs"]
mod incremental_artifact_tests;
