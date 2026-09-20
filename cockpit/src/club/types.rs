//! Chat / tool-calling protocol types shared with the harness.

use super::*;

/// Owned identity of one concrete route at a point in time. It is safe to keep
/// after a turn: no prompt/response content, credentials, or backend handles.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteIdentity {
    pub driver: String,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
}

/// Backend-supplied, operator-facing capability facts for one concrete route.
/// This is loaded when the Bag is built and cloned into deck snapshots, so the
/// renderer never reads a cache file or asks a backend on the draw path.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RouteMetadata {
    pub description: Option<String>,
    pub context_window: Option<u64>,
    pub input_modalities: Vec<String>,
    pub speed_tiers: Vec<String>,
    pub reasoning_descriptions: Vec<(String, String)>,
    pub output_budget: OutputBudgetPolicy,
    pub output_budget_provenance: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutputBudgetPolicy {
    #[default]
    ProviderNative,
    Explicit {
        tokens: u32,
        source: OutputBudgetSource,
    },
    EndpointManaged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputBudgetSource {
    PerClubEnv,
    GlobalEnv,
}

impl OutputBudgetPolicy {
    pub fn label(self, provenance: Option<&str>) -> String {
        match self {
            Self::ProviderNative => "output: provider-native".to_string(),
            Self::Explicit { tokens, source } => {
                let fallback = match source {
                    OutputBudgetSource::PerClubEnv => "per-club environment",
                    OutputBudgetSource::GlobalEnv => "ANGEL_CLUB_MAX_TOKENS",
                };
                format!("output: {tokens} · {}", provenance.unwrap_or(fallback))
            }
            Self::EndpointManaged => "output: endpoint-managed".to_string(),
        }
    }

    pub fn incomplete_message(self) -> String {
        match self {
            Self::Explicit { tokens, .. } => {
                format!("response incomplete: configured request cap was {tokens} tokens")
            }
            Self::ProviderNative => {
                "response incomplete: provider/model reached its native output limit".to_string()
            }
            Self::EndpointManaged => {
                "response incomplete: endpoint reported max_output_tokens (plan-managed)"
                    .to_string()
            }
        }
    }
}

impl RouteMetadata {
    pub fn reasoning_description(&self, effort: &str) -> Option<&str> {
        self.reasoning_descriptions
            .iter()
            .find(|(level, _)| level.eq_ignore_ascii_case(effort))
            .map(|(_, description)| description.as_str())
    }

    /// Rounded occupancy of this model's trusted context window. Unknown or
    /// zero-sized windows stay unknown instead of inheriting another route's
    /// limits.
    pub fn context_usage_percent(&self, used_tokens: usize) -> Option<u64> {
        let window = self.context_window.filter(|window| *window > 0)?;
        let percent = ((used_tokens as u128 * 100 + u128::from(window) / 2) / u128::from(window))
            .min(u128::from(u64::MAX));
        Some(percent as u64)
    }
}

// ---------------------------------------------------------------------------
// Chat / tool-calling protocol (shared with the harness)
// ---------------------------------------------------------------------------

/// Role of a message in the conversation sent to a club.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatRole {
    System,
    User,
    /// Harness-authored direction that is sent to providers as a user message
    /// but must not be mistaken for a newer operator task during compaction.
    Harness,
    Assistant,
    Tool,
}

/// One message in the conversation. `tool_calls` is populated only on the
/// assistant message that requested tools; `tool_call_id` only on tool results.
#[derive(Clone, Serialize, Deserialize)]
pub struct ChatMsg {
    pub role: ChatRole,
    /// Message text. Immutable after construction except for explicit in-place
    /// reassignment (context-block strip, aging receipts). History clones share
    /// the buffer instead of copying every content byte on the UI thread.
    pub content: Arc<str>,
    /// Multimodal attachments (images/audio) on a user message. Immutable after
    /// construction, so turn-worker history snapshots share encoded payloads
    /// instead of copying them before the first provider request.
    pub attachments: Arc<[Media]>,
    /// Tool calls are likewise immutable once the assistant message is formed.
    /// Sharing the slice keeps large structured arguments copy-on-write across
    /// the UI's live history and the worker's mutable conversation snapshot.
    pub tool_calls: Arc<[ToolCall]>,
    pub tool_call_id: Option<String>,
    /// Provider-private reasoning attached to an assistant tool-call turn.
    /// This is deliberately transient: it is needed by DeepSeek V4 on the next
    /// wire hop, but must never enter saved sessions, rollouts, or generic
    /// provider history. Cloning a live message shares the immutable bytes.
    #[serde(skip, default)]
    pub(crate) private_reasoning: Option<Arc<str>>,
    /// Execution-bound evidence is transient: provider text and imported
    /// transcripts cannot manufacture a successful tool receipt.
    #[serde(skip, default)]
    pub(crate) tool_receipt: Option<Arc<ToolReceipt>>,
    /// Durable unknown-only origin hints. They cannot authenticate a child or
    /// grant complete coverage, but must survive session restore/compaction.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) recovery_context: Vec<RecoveryContextRef>,
}

#[derive(Clone, Debug)]
pub(crate) struct ToolReceipt {
    pub(crate) recorded_at_ms: u64,
    pub(crate) workspace_state: Option<serde_json::Value>,
    pub(crate) call: ToolCall,
    pub(crate) outcome: crate::harness::ToolOutcome,
    pub(crate) routing: Option<crate::harness::shell_verifier::RoutingReceipt>,
}

impl std::fmt::Debug for ChatMsg {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ChatMsg")
            .field("role", &self.role)
            .field("content", &self.content)
            .field("attachments", &self.attachments)
            .field("tool_calls", &self.tool_calls)
            .field("tool_call_id", &self.tool_call_id)
            .field(
                "private_reasoning",
                &self.private_reasoning.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl ChatMsg {
    pub fn system(content: impl Into<Arc<str>>) -> Self {
        Self::plain(ChatRole::System, content)
    }
    pub fn user(content: impl Into<Arc<str>>) -> Self {
        Self::plain(ChatRole::User, content)
    }
    pub fn harness(content: impl Into<Arc<str>>) -> Self {
        Self::plain(ChatRole::Harness, content)
    }
    /// A user message carrying image/audio attachments (for Gemma & co.).
    pub fn user_with_media(content: impl Into<Arc<str>>, attachments: Vec<Media>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
            attachments: attachments.into(),
            tool_calls: Vec::new().into(),
            tool_call_id: None,
            private_reasoning: None,
            tool_receipt: None,
            recovery_context: Vec::new(),
        }
    }
    pub fn assistant(content: impl Into<Arc<str>>) -> Self {
        Self::plain(ChatRole::Assistant, content)
    }
    /// The assistant turn that requested tool calls (content is usually empty).
    #[cfg(test)]
    pub fn assistant_calls(calls: Vec<ToolCall>) -> Self {
        Self::assistant_calls_with_reasoning(calls, None)
    }
    pub(crate) fn assistant_calls_with_reasoning(
        calls: Vec<ToolCall>,
        private_reasoning: Option<String>,
    ) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: Arc::from(""),
            attachments: Vec::new().into(),
            tool_calls: calls.into(),
            tool_call_id: None,
            private_reasoning: private_reasoning.map(Arc::<str>::from),
            tool_receipt: None,
            recovery_context: Vec::new(),
        }
    }
    /// A tool result answering the call `id`.
    pub fn tool(id: impl Into<String>, content: impl Into<Arc<str>>) -> Self {
        Self {
            role: ChatRole::Tool,
            content: content.into(),
            attachments: Vec::new().into(),
            tool_calls: Vec::new().into(),
            tool_call_id: Some(id.into()),
            private_reasoning: None,
            tool_receipt: None,
            recovery_context: Vec::new(),
        }
    }
    pub(crate) fn with_tool_receipt(
        mut self,
        call: &ToolCall,
        outcome: crate::harness::ToolOutcome,
    ) -> Self {
        // Only Caddy's typed verifiers consume this evidence. Do not duplicate
        // write_file bodies or multimodal payloads for unrelated tool results.
        if !matches!(
            call.name.as_str(),
            "cargo" | "run_tests" | "check" | "lint" | "shell" | "proc_run"
        ) {
            return self;
        }
        self.tool_receipt = Some(Arc::new(ToolReceipt {
            call: call.clone(),
            recorded_at_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            outcome,
            routing: None,
            workspace_state: None,
        }));
        self
    }
    pub(crate) fn with_verified_workspace(
        mut self,
        workspace: &std::path::Path,
        single: bool,
    ) -> Self {
        let verified = self
            .tool_receipt
            .as_ref()
            .is_some_and(|r| crate::caddy::verified_recipe_call(&r.call, &self));
        if single
            && verified
            && let Some(receipt) = self.tool_receipt.as_mut()
        {
            Arc::make_mut(receipt).workspace_state = Some(
                crate::harness::trace_schema::workspace_state(Some(workspace)),
            );
        }
        self
    }
    pub(crate) fn with_routing_receipt(
        mut self,
        routing: Option<crate::harness::shell_verifier::RoutingReceipt>,
    ) -> Self {
        if let Some(receipt) = self.tool_receipt.as_mut() {
            Arc::make_mut(receipt).routing = routing;
        }
        self
    }
    fn plain(role: ChatRole, content: impl Into<Arc<str>>) -> Self {
        Self {
            role,
            content: content.into(),
            attachments: Vec::new().into(),
            tool_calls: Vec::new().into(),
            tool_call_id: None,
            private_reasoning: None,
            tool_receipt: None,
            recovery_context: Vec::new(),
        }
    }
}

/// A tool the club can be offered (name + description + JSON-schema params).
#[derive(Clone, Debug)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub params: serde_json::Value,
}

/// A tool call the club asked us to make.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: serde_json::Value,
}

/// What a club returns from one `chat`: a final answer, or tool calls to run.
#[derive(Clone, Debug)]
pub enum ClubReply {
    Text(String),
    Calls(Vec<ToolCall>),
}

/// Structured token counts from metered SOTA/cloud backends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TokenUsage {
    pub turns: u64,
    pub last_input: u64,
    pub last_output: u64,
    pub last_reasoning: u64,
    pub total_input: u64,
    pub total_output: u64,
    pub total_reasoning: u64,
}

/// Lock-free [`TokenUsage`] snapshot. The draw path and drain fold read this
/// without waiting on the hop that is recording usage.
#[derive(Default)]
pub struct UsageCell {
    turns: AtomicU64,
    last_input: AtomicU64,
    last_output: AtomicU64,
    last_reasoning: AtomicU64,
    total_input: AtomicU64,
    total_output: AtomicU64,
    total_reasoning: AtomicU64,
}

impl UsageCell {
    pub fn load(&self) -> TokenUsage {
        TokenUsage {
            turns: self.turns.load(Ordering::Relaxed),
            last_input: self.last_input.load(Ordering::Relaxed),
            last_output: self.last_output.load(Ordering::Relaxed),
            last_reasoning: self.last_reasoning.load(Ordering::Relaxed),
            total_input: self.total_input.load(Ordering::Relaxed),
            total_output: self.total_output.load(Ordering::Relaxed),
            total_reasoning: self.total_reasoning.load(Ordering::Relaxed),
        }
    }

    pub fn store(&self, usage: TokenUsage) {
        self.turns.store(usage.turns, Ordering::Relaxed);
        self.last_input.store(usage.last_input, Ordering::Relaxed);
        self.last_output.store(usage.last_output, Ordering::Relaxed);
        self.last_reasoning
            .store(usage.last_reasoning, Ordering::Relaxed);
        self.total_input.store(usage.total_input, Ordering::Relaxed);
        self.total_output
            .store(usage.total_output, Ordering::Relaxed);
        self.total_reasoning
            .store(usage.total_reasoning, Ordering::Relaxed);
    }

    pub fn record_turn(&self, input: u64, output: u64, reasoning: u64) {
        self.turns.fetch_add(1, Ordering::Relaxed);
        self.last_input.store(input, Ordering::Relaxed);
        self.last_output.store(output, Ordering::Relaxed);
        self.last_reasoning.store(reasoning, Ordering::Relaxed);
        self.total_input.fetch_add(input, Ordering::Relaxed);
        self.total_output.fetch_add(output, Ordering::Relaxed);
        self.total_reasoning.fetch_add(reasoning, Ordering::Relaxed);
    }
}

/// Cumulative prompt-cache request and provider-reported token economics.
/// Separate from [`TokenUsage`] because cache fields are optional and provider-
/// specific even when ordinary input/output accounting is present.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheUsage {
    pub control_requests: u64,
    pub read_input_tokens: u64,
    pub write_input_tokens: u64,
    pub read_accounting_responses: u64,
    pub write_accounting_responses: u64,
}

/// Lock-free [`CacheUsage`] snapshot. Same contract as [`UsageCell`]: the UI
/// never waits on the hop that is folding cache accounting.
#[derive(Default)]
pub struct CacheUsageCell {
    control_requests: AtomicU64,
    read_input_tokens: AtomicU64,
    write_input_tokens: AtomicU64,
    read_accounting_responses: AtomicU64,
    write_accounting_responses: AtomicU64,
}

impl CacheUsageCell {
    pub fn load(&self) -> CacheUsage {
        CacheUsage {
            control_requests: self.control_requests.load(Ordering::Relaxed),
            read_input_tokens: self.read_input_tokens.load(Ordering::Relaxed),
            write_input_tokens: self.write_input_tokens.load(Ordering::Relaxed),
            read_accounting_responses: self.read_accounting_responses.load(Ordering::Relaxed),
            write_accounting_responses: self.write_accounting_responses.load(Ordering::Relaxed),
        }
    }

    pub fn add_read(&self, tokens: u64) {
        self.read_input_tokens.fetch_add(tokens, Ordering::Relaxed);
        self.read_accounting_responses
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_write(&self, tokens: u64) {
        self.write_input_tokens.fetch_add(tokens, Ordering::Relaxed);
        self.write_accounting_responses
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_control_request(&self) {
        self.control_requests.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod meter_cell_tests {
    use super::*;

    #[test]
    fn usage_cell_record_turn_accumulates_without_a_mutex() {
        let cell = UsageCell::default();
        assert_eq!(cell.load(), TokenUsage::default());
        cell.record_turn(10, 4, 1);
        cell.record_turn(3, 2, 0);
        assert_eq!(
            cell.load(),
            TokenUsage {
                turns: 2,
                last_input: 3,
                last_output: 2,
                last_reasoning: 0,
                total_input: 13,
                total_output: 6,
                total_reasoning: 1,
            }
        );
    }

    #[test]
    fn cache_usage_cell_adds_read_write_and_control_counts() {
        let cell = CacheUsageCell::default();
        cell.add_control_request();
        cell.add_read(40);
        cell.add_write(8);
        assert_eq!(
            cell.load(),
            CacheUsage {
                control_requests: 1,
                read_input_tokens: 40,
                write_input_tokens: 8,
                read_accounting_responses: 1,
                write_accounting_responses: 1,
            }
        );
    }
}

/// Cumulative output-cap recovery counters for one club. Kept separate from
/// token usage because many OpenAI-compatible failure responses omit `usage`
/// even though the retry itself still matters for reliability and latency.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TruncationUsage {
    pub episodes: u64,
    pub retained_partials: u64,
    pub retries: u64,
    pub recoveries: u64,
    pub failures: u64,
}

/// A multimodal attachment carried by a user message (for vision/audio clubs
/// like Gemma). Stored pre-encoded so serialization is cheap.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Media {
    Image { mime: String, b64: String },
    Audio { format: String, b64: String },
}

pub(crate) const MAX_IMAGE_ATTACHMENT_BYTES: usize = 5 * 1024 * 1024;
pub(crate) const MAX_IMAGE_ATTACHMENT_PIXELS: u64 = 40_000_000;
pub(crate) const MAX_AUDIO_ATTACHMENT_BYTES: usize = 25 * 1024 * 1024;

fn read_attachment_bounded(
    path: &std::path::Path,
    max_bytes: usize,
    kind: &str,
) -> Result<Vec<u8>, String> {
    use std::io::Read as _;
    let file = std::fs::File::open(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    if file
        .metadata()
        .map_err(|e| format!("inspect {}: {e}", path.display()))?
        .len()
        > max_bytes as u64
    {
        return Err(format!(
            "{kind} attachment exceeds the {} MiB safety limit",
            max_bytes / (1024 * 1024)
        ));
    }
    let mut bytes = Vec::new();
    file.take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    if bytes.len() > max_bytes {
        return Err(format!(
            "{kind} attachment exceeded the {} MiB safety limit while reading",
            max_bytes / (1024 * 1024)
        ));
    }
    if bytes.is_empty() {
        return Err(format!("{kind} attachment is empty"));
    }
    Ok(bytes)
}

impl Media {
    /// Load one bounded, fully decoded image and derive MIME from its bytes.
    pub fn image_from_path(path: &std::path::Path) -> Result<Self, String> {
        let bytes = read_attachment_bounded(path, MAX_IMAGE_ATTACHMENT_BYTES, "image")?;
        Self::image_from_bytes(&bytes, &path.display().to_string())
    }

    /// Admit an already-read encoding through that same size/format/pixel/decode
    /// policy. A clipboard screenshot arrives here instead of on disk, so the
    /// one admission path is shared rather than duplicated. `label` only names
    /// the source in failures (a path, or "clipboard image").
    pub fn image_from_bytes(bytes: &[u8], label: &str) -> Result<Self, String> {
        if bytes.is_empty() {
            return Err(format!("{label} is empty"));
        }
        if bytes.len() > MAX_IMAGE_ATTACHMENT_BYTES {
            return Err(format!(
                "{label} exceeds the {} MiB image safety limit",
                MAX_IMAGE_ATTACHMENT_BYTES / (1024 * 1024)
            ));
        }
        let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|error| format!("inspect image {label}: {error}"))?;
        let format = reader
            .format()
            .ok_or_else(|| format!("unsupported image format: {label}"))?;
        let mime = match format {
            image::ImageFormat::Png => "image/png",
            image::ImageFormat::Jpeg => "image/jpeg",
            image::ImageFormat::Gif => "image/gif",
            image::ImageFormat::WebP => "image/webp",
            _ => {
                return Err(format!(
                    "unsupported image format {:?}; use PNG, JPEG, GIF, or WebP",
                    format
                ));
            }
        };
        let (width, height) = reader
            .into_dimensions()
            .map_err(|error| format!("inspect image dimensions {label}: {error}"))?;
        let pixels = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or_else(|| "image pixel count overflow".to_string())?;
        if pixels > MAX_IMAGE_ATTACHMENT_PIXELS {
            return Err(format!(
                "image is {width}x{height} ({pixels} pixels); maximum is {MAX_IMAGE_ATTACHMENT_PIXELS} pixels"
            ));
        }
        // Decode once at admission so malformed/truncated rasters cannot enter
        // durable history and fail repeatedly on every provider replay.
        image::load_from_memory_with_format(bytes, format)
            .map_err(|error| format!("decode image {label}: {error}"))?;
        Ok(Media::Image {
            mime: mime.to_string(),
            b64: base64::engine::general_purpose::STANDARD.encode(bytes),
        })
    }

    /// Load a bounded audio file → base64, format guessed from the extension.
    pub fn audio_from_path(path: &std::path::Path) -> Result<Self, String> {
        let bytes = read_attachment_bounded(path, MAX_AUDIO_ATTACHMENT_BYTES, "audio")?;
        let format = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("wav")
            .to_ascii_lowercase();
        Ok(Media::Audio {
            format,
            b64: base64::engine::general_purpose::STANDARD.encode(&bytes),
        })
    }

    /// OpenAI content-part JSON for this attachment.
    pub(crate) fn to_part(&self) -> serde_json::Value {
        match self {
            Media::Image { mime, b64 } => serde_json::json!({
                "type": "image_url",
                "image_url": { "url": format!("data:{mime};base64,{b64}") }
            }),
            Media::Audio { format, b64 } => serde_json::json!({
                "type": "input_audio",
                "input_audio": { "data": b64, "format": format }
            }),
        }
    }
}

#[cfg(test)]
mod media_tests {
    use super::*;

    fn temp_path(label: &str) -> std::path::PathBuf {
        static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "angel-media-{label}-{}-{}",
            std::process::id(),
            ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ))
    }

    #[test]
    fn image_admission_uses_magic_not_extension_and_rejects_malformed_bytes() {
        let disguised = temp_path("pixel.jpg");
        let mut png = Vec::new();
        image::DynamicImage::new_rgba8(1, 1)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        std::fs::write(&disguised, &png).unwrap();
        let media = Media::image_from_path(&disguised).unwrap();
        assert!(matches!(media, Media::Image { ref mime, .. } if mime == "image/png"));
        std::fs::write(&disguised, b"not really an image").unwrap();
        assert!(Media::image_from_path(&disguised).is_err());
        std::fs::remove_file(disguised).unwrap();
    }

    #[test]
    fn attachment_reader_rejects_oversized_and_empty_inputs_before_encoding() {
        let path = temp_path("bounded.bin");
        std::fs::write(&path, []).unwrap();
        assert!(read_attachment_bounded(&path, 4, "test").is_err());
        std::fs::write(&path, b"12345").unwrap();
        let error = read_attachment_bounded(&path, 4, "test").unwrap_err();
        assert!(error.contains("safety limit"), "{error}");
        std::fs::remove_file(path).unwrap();
    }
}

// ---------------------------------------------------------------------------
// Club trait
// ---------------------------------------------------------------------------

/// A model backend — one club in the bag. `Send + Sync` so the cockpit can run
/// a swing on a worker thread via `Arc<dyn Club>`.
/// A streamed chunk from a club. Visible answer **content**, the model's private
/// **reasoning** (`reasoning_content`, emitted by reasoning models like SIQ), or
/// a zero-payload transport **heartbeat** while a bounded recovery wait is in
/// progress. Reasoning stays out of the answer/saved transcript; heartbeats stay
/// out of both and exist only to keep the foreground watchdog truthful.
pub enum StreamDelta<'a> {
    Content(&'a str),
    Reasoning(&'a str),
    Heartbeat,
}

/// A club's model capabilities, detected once from the backend rather than
/// guessed. `context_window` is the model's real token limit (llama.cpp `/props`
/// → `n_ctx`, vLLM `/v1/models` → `max_model_len`, or a small static map for
/// cloud models); `0` means "unknown" so callers fall back to their default. The
/// flags gate optional request fields (prompt-cache key, reasoning effort) so we
/// only send what a given backend honors. See `HttpClub::probe_metadata`.
///
/// `supports_reasoning` is three-valued: `Some` only when the capability was
/// *declared* (a catalog's `supported_parameters`, or a name that guarantees a
/// reasoning model), `None` when genuinely unknown. Only a declared `Some(false)`
/// may suppress an operator-requested effort — an unknown never silently gates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Metadata {
    pub context_window: usize,
    pub supports_cache: bool,
    pub supports_reasoning: Option<bool>,
    pub supports_tools: bool,
}

/// Cumulative reasoning-effort gate outcomes for one club. Every time a
/// requested effort (seat, THINK deck, or env) is withheld from the wire — or a
/// backend rejects the reasoning field outright and the club learns to stop
/// sending it — the counters move and `last` records the human-readable reason.
/// The turn loop reads a before/after delta and surfaces it as a notice: a gate
/// that strips operator intent must say so, never silently.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EffortGateUsage {
    /// Requested efforts left off a request body by a declared/learned no-go.
    pub withheld: u64,
    /// Backend rejections of the reasoning field that taught the club to stop.
    pub rejections: u64,
    /// The most recent gate event, in words fit for the operator.
    pub last: Option<String>,
}
