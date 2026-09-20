//! Session handle store — opaque bulk evidence with root-visible receipts.
//!
//! Implements the RLM/HiQ structural offload principle for angel0:
//! task-specific bulk and intermediate sub-work live under handle-addressed
//! storage, while the root model sees bounded, strategy-level receipts.
//! Disclosure of handle bodies is explicit and capped.
//!
//! This module is inference-first scaffolding. It does not train models or
//! replace compaction/aging; it stores what aging would otherwise discard so
//! later nested work (code_mode, subcalls, explicit `handle_read`) can inspect
//! slices without replaying bulk into root history by default.
//!
use super::*;
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

/// Root-visible mark for a handle-backed receipt. Distinct from
/// [`TOOL_AGED_MARK`] so consumers can detect re-fetchable offload.
pub(crate) const HANDLE_RECEIPT_MARK: &str = "[handle receipt";

/// Opaque handle identity. Format: `hnd_<base36>`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct HandleId(pub(crate) String);

impl HandleId {
    #[inline]
    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Display for HandleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Producer class for provenance and root-trajectory classification.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum HandleKind {
    /// Successful reconstructable inspection tool result aged out of root.
    ToolResult,
    /// Nested code_mode intermediate / final bulk offload.
    CodeMode,
    /// Programmatic sub-LM / spawn seat bulk return.
    Subcall,
    /// Explicit operator or test deposit.
    #[default]
    Manual,
}

impl HandleKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ToolResult => "tool_result",
            Self::CodeMode => "code_mode",
            Self::Subcall => "subcall",
            Self::Manual => "manual",
        }
    }

    #[allow(dead_code)] // wire/env round-trip and tests
    pub(crate) fn parse(s: &str) -> Option<Self> {
        match s {
            "tool_result" => Some(Self::ToolResult),
            "code_mode" => Some(Self::CodeMode),
            "subcall" => Some(Self::Subcall),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }
}

/// Bounded strategy-level summary shown to the root model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Receipt {
    pub(crate) handle: HandleId,
    pub(crate) kind: HandleKind,
    /// Tool / recipe / persona name (bounded).
    pub(crate) producer: String,
    /// Semantic inspection identity when known (`read_file|path`).
    pub(crate) identity: Option<String>,
    pub(crate) bytes: usize,
    pub(crate) lines: usize,
    /// At most a handful of path stems for strategy context.
    pub(crate) paths: Vec<String>,
    /// Optional short non-bulk preview (first non-empty line, heavily capped).
    pub(crate) preview: Option<String>,
}

impl Receipt {
    /// Stable root-visible text. Kept short so many receipts remain LID-like.
    pub(crate) fn render(&self) -> String {
        let identity = self
            .identity
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("-");
        let path_note = if self.paths.is_empty() {
            String::new()
        } else {
            format!(" paths={}", self.paths.join(","))
        };
        let preview = self
            .preview
            .as_deref()
            .map(|p| format!(" preview={p}"))
            .unwrap_or_default();
        format!(
            "{HANDLE_RECEIPT_MARK}: {} kind={} producer={} identity={identity} bytes={} lines={}{path_note}{preview} — read: handle_read tool (tool_search it if unlisted)]",
            self.handle,
            self.kind.as_str(),
            self.producer,
            self.bytes,
            self.lines,
        )
    }
}

/// Capped disclosure of a stored body slice (never the unbounded full dump by
/// default — callers must pass an explicit budget).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiscloseSlice {
    pub(crate) handle: HandleId,
    pub(crate) offset: usize,
    pub(crate) bytes: usize,
    pub(crate) total_bytes: usize,
    pub(crate) truncated: bool,
    pub(crate) content: String,
}

impl DiscloseSlice {
    pub(crate) fn render(&self) -> String {
        let trunc = if self.truncated {
            " truncated=true"
        } else {
            ""
        };
        format!(
            "[handle disclose: {} offset={} bytes={} total={}{trunc}]\n{}",
            self.handle, self.offset, self.bytes, self.total_bytes, self.content
        )
    }
}

#[derive(Clone, Debug)]
struct HandleEntry {
    receipt: Receipt,
    body: String,
    /// Non-cryptographic integrity fingerprint (length + std hash).
    #[allow(dead_code)]
    content_sha256: String,
    #[allow(dead_code)]
    created_ms: u64,
    touch_ms: u64,
}

/// Session-scoped store. Entries are process-local and not durable across
/// restarts (durable evidence still lives in dossier/experience/palace).
#[derive(Debug, Default)]
pub(crate) struct HandleStore {
    entries: HashMap<String, HandleEntry>,
    order: VecDeque<String>,
    next_id: u64,
    total_bytes: usize,
    puts: u64,
    discloses: u64,
    evictions: u64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct HandleStoreLimits {
    pub(crate) max_entries: usize,
    pub(crate) max_total_bytes: usize,
    pub(crate) max_body_bytes: usize,
    pub(crate) max_disclose_bytes: usize,
    pub(crate) max_paths: usize,
    pub(crate) max_preview_chars: usize,
}

impl Default for HandleStoreLimits {
    fn default() -> Self {
        Self {
            max_entries: 256,
            max_total_bytes: 32 * 1024 * 1024,
            max_body_bytes: 2 * 1024 * 1024,
            max_disclose_bytes: 16 * 1024,
            max_paths: 4,
            max_preview_chars: 80,
        }
    }
}

impl HandleStoreLimits {
    pub(crate) fn from_env() -> Self {
        let d = Self::default();
        Self {
            max_entries: env_usize("ANGEL_HANDLE_MAX_ENTRIES", d.max_entries).max(1),
            max_total_bytes: env_usize("ANGEL_HANDLE_MAX_TOTAL_BYTES", d.max_total_bytes).max(4096),
            max_body_bytes: env_usize("ANGEL_HANDLE_MAX_BODY_BYTES", d.max_body_bytes).max(1024),
            max_disclose_bytes: env_usize("ANGEL_HANDLE_MAX_DISCLOSE_BYTES", d.max_disclose_bytes)
                .max(256),
            max_paths: env_usize("ANGEL_HANDLE_MAX_PATHS", d.max_paths).max(1),
            max_preview_chars: env_usize("ANGEL_HANDLE_MAX_PREVIEW_CHARS", d.max_preview_chars)
                .max(16),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct HandleStoreStats {
    pub(crate) entries: usize,
    pub(crate) total_bytes: usize,
    pub(crate) puts: u64,
    pub(crate) discloses: u64,
    pub(crate) evictions: u64,
}

/// Metadata accepted when depositing bulk evidence.
#[derive(Clone, Debug, Default)]
pub(crate) struct PutMeta<'a> {
    pub(crate) kind: HandleKind,
    pub(crate) producer: &'a str,
    pub(crate) identity: Option<&'a str>,
    pub(crate) paths: &'a [String],
    pub(crate) include_preview: bool,
}

impl HandleStore {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn stats(&self) -> HandleStoreStats {
        HandleStoreStats {
            entries: self.entries.len(),
            total_bytes: self.total_bytes,
            puts: self.puts,
            discloses: self.discloses,
            evictions: self.evictions,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.total_bytes = 0;
    }

    /// Receipts in LRU order (oldest first). Used by `agent://` listing.
    pub(crate) fn list_receipts(&self) -> Vec<Receipt> {
        self.order
            .iter()
            .filter_map(|id| self.entries.get(id).map(|e| e.receipt.clone()))
            .collect()
    }

    #[allow(dead_code)] // offline inspection / tests
    pub(crate) fn get_receipt(&self, id: &str) -> Option<&Receipt> {
        self.entries.get(id).map(|e| &e.receipt)
    }

    #[allow(dead_code)] // offline inspection / tests
    pub(crate) fn content_sha256(&self, id: &str) -> Option<&str> {
        self.entries.get(id).map(|e| e.content_sha256.as_str())
    }

    #[allow(dead_code)] // eviction tests + offline inspection
    pub(crate) fn contains(&self, id: &str) -> bool {
        self.entries.contains_key(id)
    }

    /// Store bulk `body` and return a root-visible receipt. Empty bodies are
    /// rejected — there is nothing to offload.
    pub(crate) fn put(
        &mut self,
        body: &str,
        meta: PutMeta<'_>,
        limits: HandleStoreLimits,
        now_ms: u64,
    ) -> Result<Receipt, String> {
        if body.is_empty() {
            return Err("handle store refuses empty body".into());
        }
        let body = if body.len() > limits.max_body_bytes {
            utf8_prefix(body, limits.max_body_bytes)
        } else {
            body.to_string()
        };
        let bytes = body.len();
        if bytes > limits.max_total_bytes {
            return Err(format!(
                "handle body ({bytes} B) exceeds store total budget ({})",
                limits.max_total_bytes
            ));
        }

        self.evict_for(bytes, limits);

        let id = self.alloc_id();
        let producer = bound_token(meta.producer, 64);
        let identity = meta
            .identity
            .map(|s| bound_token(s, 200))
            .filter(|s| !s.is_empty());
        let paths = meta
            .paths
            .iter()
            .filter(|p| !p.trim().is_empty())
            .take(limits.max_paths)
            .map(|p| bound_token(p, 120))
            .collect::<Vec<_>>();
        let lines = body.lines().count();
        let preview = if meta.include_preview {
            first_line_preview(&body, limits.max_preview_chars)
        } else {
            None
        };
        let receipt = Receipt {
            handle: HandleId(id.clone()),
            kind: meta.kind,
            producer,
            identity,
            bytes,
            lines,
            paths,
            preview,
        };
        let content_sha256 = sha256_hex(body.as_bytes());
        self.entries.insert(
            id.clone(),
            HandleEntry {
                receipt: receipt.clone(),
                body,
                content_sha256,
                created_ms: now_ms,
                touch_ms: now_ms,
            },
        );
        self.order.push_back(id);
        self.total_bytes = self.total_bytes.saturating_add(bytes);
        self.puts = self.puts.saturating_add(1);
        Ok(receipt)
    }

    /// Explicit capped disclosure. Default path for root re-materialization.
    pub(crate) fn disclose(
        &mut self,
        id: &str,
        offset: usize,
        max_bytes: usize,
        limits: HandleStoreLimits,
        now_ms: u64,
    ) -> Result<DiscloseSlice, String> {
        let entry = self
            .entries
            .get_mut(id)
            .ok_or_else(|| format!("unknown handle: {id}"))?;
        entry.touch_ms = now_ms;
        let total = entry.body.len();
        if offset >= total {
            return Ok(DiscloseSlice {
                handle: HandleId(id.to_string()),
                offset,
                bytes: 0,
                total_bytes: total,
                truncated: false,
                content: String::new(),
            });
        }
        let budget = max_bytes.min(limits.max_disclose_bytes).max(1);
        let end = (offset + budget).min(total);
        // Snap to char boundary.
        let mut end = end;
        while end > offset && !entry.body.is_char_boundary(end) {
            end -= 1;
        }
        let content = entry.body[offset..end].to_string();
        let truncated = end < total;
        self.discloses = self.discloses.saturating_add(1);
        // Refresh LRU: move id to back.
        if let Some(pos) = self.order.iter().position(|x| x == id)
            && let Some(item) = self.order.remove(pos)
        {
            self.order.push_back(item);
        }
        Ok(DiscloseSlice {
            handle: HandleId(id.to_string()),
            offset,
            bytes: content.len(),
            total_bytes: total,
            truncated,
            content,
        })
    }

    fn alloc_id(&mut self) -> String {
        let n = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        format!("hnd_{}", to_base36(n))
    }

    fn evict_for(&mut self, incoming: usize, limits: HandleStoreLimits) {
        while self.entries.len() >= limits.max_entries
            || self.total_bytes.saturating_add(incoming) > limits.max_total_bytes
        {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.total_bytes = self.total_bytes.saturating_sub(entry.body.len());
                self.evictions = self.evictions.saturating_add(1);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Process session store
// ---------------------------------------------------------------------------

fn session_store() -> &'static Mutex<HandleStore> {
    static STORE: OnceLock<Mutex<HandleStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HandleStore::new()))
}

/// Whether the handle-store treatment is active (default on).
pub(crate) fn handle_store_enabled() -> bool {
    env_flag("ANGEL_HANDLE_STORE", true)
}

/// Deposit bulk into the process session store. No-op path returns `None` when
/// disabled so callers fall back to discard-only receipts.
pub(crate) fn session_put(body: &str, meta: PutMeta<'_>) -> Option<Receipt> {
    if !handle_store_enabled() {
        return None;
    }
    let limits = HandleStoreLimits::from_env();
    let now = now_ms();
    let mut guard = session_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.put(body, meta, limits, now).ok()
}

/// Explicit capped disclosure from the process session store.
pub(crate) fn session_disclose(
    id: &str,
    offset: usize,
    max_bytes: usize,
) -> Result<DiscloseSlice, String> {
    if !handle_store_enabled() {
        return Err("handle store disabled (ANGEL_HANDLE_STORE=0)".into());
    }
    let limits = HandleStoreLimits::from_env();
    let now = now_ms();
    let mut guard = session_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.disclose(id, offset, max_bytes, limits, now)
}

pub(crate) fn session_stats() -> HandleStoreStats {
    session_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .stats()
}

/// List live handle receipts (oldest first). Empty when the store is disabled.
pub(crate) fn session_list_receipts() -> Vec<Receipt> {
    if !handle_store_enabled() {
        return Vec::new();
    }
    session_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .list_receipts()
}

pub(crate) fn session_clear() {
    session_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}

/// Age-path helper: store body when possible and render a handle-backed aged
/// receipt that still carries the legacy [`TOOL_AGED_MARK`] prefix for existing
/// pairing/idempotence checks.
pub(crate) fn age_receipt_for(identity: &str, original_bytes: usize, body: &str) -> Option<String> {
    let receipt = session_put(
        body,
        PutMeta {
            kind: HandleKind::ToolResult,
            producer: identity.split('|').next().unwrap_or("tool"),
            identity: Some(identity),
            paths: &extract_paths_from_identity(identity),
            include_preview: false,
        },
    )?;
    let mut bounded_identity = identity.chars().take(200).collect::<String>();
    if identity.chars().count() > 200 {
        bounded_identity.push('…');
    }
    Some(format!(
        "{TOOL_AGED_MARK}: {bounded_identity} ({original_bytes} bytes) handle={} — re-run the tool if needed]",
        receipt.handle.as_str()
    ))
}

/// Offload a large root-bound tool/subcall body when the store is enabled and
/// `body` meets `min_bytes`. Returns a handle receipt for root history, or
/// `None` when the caller should keep the original body (too small, store off,
/// or put failed).
pub(crate) fn maybe_offload_root_body(
    body: &str,
    kind: HandleKind,
    producer: &str,
    identity: &str,
    min_bytes: usize,
) -> Option<String> {
    if body.len() < min_bytes {
        return None;
    }
    let paths = extract_paths_from_identity(identity);
    let receipt = session_put(
        body,
        PutMeta {
            kind,
            producer,
            identity: Some(identity),
            paths: &paths,
            include_preview: false,
        },
    )?;
    Some(receipt.render())
}

/// Outcome of the pre-history bulk veto (eager offload).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct EagerOffload {
    /// Root-facing content (receipt when offloaded, else original).
    pub(crate) content: String,
    pub(crate) offloaded: bool,
    pub(crate) original_bytes: usize,
    pub(crate) bytes_saved: u64,
}

/// Tools whose results must stay complete in root history (mutations,
/// verifiers, approvals, handle plumbing). Everything else may be eagerly
/// offloaded when large enough.
pub(crate) fn eager_offload_tool_eligible(tool_name: &str) -> bool {
    !matches!(
        tool_name,
        "write_file"
            | "str_replace"
            | "multi_edit"
            | "apply_patch"
            | "integrate"
            | "run_tests"
            | "check"
            | "lint"
            | "fmt"
            | "cargo"
            | "todo"
            | "skill"
            | "handle_read"
            | "handle_put"
            | "present"
            | "ui_inspect"
            | "ui_verify"
            // Already produce strategy receipts / own offload path.
            | "code_mode"
            | "spawn"
            | "delegate"
    )
}

/// Soft bulk veto: before a successful tool result enters root history, park
/// large eligible bodies under a handle so the root only sees a receipt.
/// Errors, denials, existing receipts, tiny bodies, and ineligible tools pass
/// through unchanged. Floor comes from [`eager_tool_offload_min_bytes`].
///
/// Default hops skip [`inspection_identity`] when this is false.
pub(crate) fn eager_offload_can_park(tool_name: &str, result: &str) -> bool {
    handle_store_enabled()
        // A receipt without a registered disclosure tool is a dead end. Aging
        // may still retain a handle for diagnostics, but eager root offload is
        // useful only when the model can recover it deliberately.
        && handle_read_tool_enabled()
        && eager_offload_tool_eligible(tool_name)
        && result.len() >= eager_tool_offload_min_bytes()
        && !result.starts_with("tool error:")
        && !result.starts_with("action capsule denied")
        && !result.contains(HANDLE_RECEIPT_MARK)
        && !result.contains(TOOL_AGED_MARK)
        && !result.contains(TOOL_DUPLICATE_MARK)
}

/// Identity string only when offload can actually park the body.
pub(crate) fn inspection_identity_for_offload(call: &ToolCall, result: &str) -> Option<String> {
    if eager_offload_can_park(&call.name, result) {
        inspection_identity(call)
    } else {
        None
    }
}

pub(crate) fn eager_offload_tool_result(
    tool_name: &str,
    result: String,
    identity: Option<&str>,
) -> EagerOffload {
    let original_bytes = result.len();
    if !eager_offload_can_park(tool_name, &result) {
        return EagerOffload {
            content: result,
            offloaded: false,
            original_bytes,
            bytes_saved: 0,
        };
    }
    let min = eager_tool_offload_min_bytes();
    let identity = identity
        .map(str::to_string)
        .unwrap_or_else(|| tool_name.to_string());
    match maybe_offload_root_body(&result, HandleKind::ToolResult, tool_name, &identity, min) {
        Some(receipt) if receipt.len() < original_bytes => EagerOffload {
            bytes_saved: original_bytes.saturating_sub(receipt.len()) as u64,
            content: receipt,
            offloaded: true,
            original_bytes,
        },
        _ => EagerOffload {
            content: result,
            offloaded: false,
            original_bytes,
            bytes_saved: 0,
        },
    }
}

/// Default floor for spawn/delegate root digests.
/// Treebeard uses a lower floor when the env var is unset (see [`lane`]).
pub(crate) fn subcall_offload_min_bytes() -> usize {
    lane_subcall_offload_min_bytes()
}

fn extract_paths_from_identity(identity: &str) -> Vec<String> {
    // Identities look like `read_file|src/lib.rs` or `grep|pattern|path`.
    let mut parts = identity.split('|');
    let _tool = parts.next();
    parts
        .filter(|p| {
            p.contains('/') || p.ends_with(".rs") || p.ends_with(".ts") || p.ends_with(".md")
        })
        .take(4)
        .map(|p| p.to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// handle_read tool — explicit root disclosure (opt-in registration)
// ---------------------------------------------------------------------------

/// Model-facing capped disclosure. Registered when
/// `ANGEL_HANDLE_READ_TOOL=1`, or automatically under the Treebeard lane
/// (unless the flag is forced off). Keeps the default schema lean outside RLM.
pub struct HandleReadTool;

impl Tool for HandleReadTool {
    fn name(&self) -> &str {
        "handle_read"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: "handle_read".into(),
            description: "Explicitly disclose a capped byte slice from a session \
                 handle previously offloaded out of root history. Prefer strategy \
                 over bulk: only call when the receipt is insufficient. Output is \
                 hard-capped by ANGEL_HANDLE_MAX_DISCLOSE_BYTES."
                .into(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "handle": {
                        "type": "string",
                        "description": "Opaque handle id from a prior receipt (hnd_…)."
                    },
                    "offset": {
                        "type": "integer",
                        "minimum": 0,
                        "default": 0,
                        "description": "Byte offset into the stored body."
                    },
                    "max_bytes": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Requested slice size; further capped by store policy."
                    }
                },
                "required": ["handle"]
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        let handle = args
            .get("handle")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "handle_read requires handle".to_string())?;
        if !handle.starts_with("hnd_") {
            return Err(format!("invalid handle id: {handle}"));
        }
        let offset = args.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
        let max_bytes = args
            .get("max_bytes")
            .and_then(Value::as_u64)
            .map(|n| n as usize)
            .unwrap_or_else(|| HandleStoreLimits::from_env().max_disclose_bytes);
        let slice = session_disclose(handle, offset, max_bytes)?;
        Ok(slice.render())
    }
}

pub(crate) fn maybe_register_handle_read(r: &mut ToolRegistry) {
    if handle_read_tool_enabled() {
        r.register(Box::new(HandleReadTool));
    }
}

// ---------------------------------------------------------------------------
// Root-only trajectory fingerprint (Hi/Q instrumentation)
// ---------------------------------------------------------------------------

/// Compact root-trajectory token used for similarity. Bulk tool bodies and
/// handle contents are reduced to strategy symbols so short and long tasks
/// that share decomposition can score as near-identical.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RootTrajectory {
    /// Ordered strategy tokens (role/tool/receipt class), not raw content.
    pub(crate) tokens: Vec<String>,
    /// Approximate root-visible character mass (receipts + assistant text).
    pub(crate) root_chars: usize,
    pub(crate) tool_hops: usize,
    pub(crate) handle_receipts: usize,
    pub(crate) aged_receipts: usize,
    pub(crate) bulk_tool_results: usize,
}

impl RootTrajectory {
    pub(crate) fn fingerprint(&self) -> String {
        self.tokens.join(" ")
    }
}

/// Project model-facing history into a root-only trajectory representation.
/// Tool bulk is classified, not embedded — this is the LID view.
pub(crate) fn root_trajectory(history: &[ChatMsg]) -> RootTrajectory {
    let mut tokens = Vec::new();
    let mut root_chars = 0usize;
    let mut tool_hops = 0usize;
    let mut handle_receipts = 0usize;
    let mut aged_receipts = 0usize;
    let mut bulk_tool_results = 0usize;

    for message in history {
        match message.role {
            ChatRole::System => {
                // Stable system prefix is intentionally omitted so fingerprints
                // compare task trajectories, not bootstrap identity.
            }
            ChatRole::User | ChatRole::Harness => {
                let kind = if message.role == ChatRole::Harness {
                    "harness"
                } else {
                    "user"
                };
                // Content-blind length bucket keeps short/long tasks comparable.
                let bucket = length_bucket(message.content.len());
                tokens.push(format!("{kind}:{bucket}"));
                root_chars = root_chars.saturating_add(message.content.len());
            }
            ChatRole::Assistant => {
                if !message.tool_calls.is_empty() {
                    tool_hops = tool_hops.saturating_add(1);
                    let mut names = message
                        .tool_calls
                        .iter()
                        .map(|c| c.name.as_str())
                        .collect::<Vec<_>>();
                    names.sort_unstable();
                    tokens.push(format!("call:{}", names.join("+")));
                }
                if !message.content.is_empty() {
                    tokens.push(format!("asst:{}", length_bucket(message.content.len())));
                    root_chars = root_chars.saturating_add(message.content.len());
                }
            }
            ChatRole::Tool => {
                let content = message.content.as_ref();
                if content.contains(HANDLE_RECEIPT_MARK) || content.contains(" handle=hnd_") {
                    handle_receipts = handle_receipts.saturating_add(1);
                    tokens.push("tool:handle".into());
                    root_chars = root_chars.saturating_add(content.len());
                } else if content.contains(TOOL_AGED_MARK) || content.contains(TOOL_DUPLICATE_MARK)
                {
                    aged_receipts = aged_receipts.saturating_add(1);
                    tokens.push("tool:receipt".into());
                    root_chars = root_chars.saturating_add(content.len());
                } else if content.starts_with("tool error:")
                    || content.starts_with("action capsule denied")
                {
                    tokens.push("tool:error".into());
                    root_chars = root_chars.saturating_add(content.len().min(200));
                } else {
                    bulk_tool_results = bulk_tool_results.saturating_add(1);
                    tokens.push(format!("tool:bulk:{}", length_bucket(content.len())));
                    // Count only a token budget of bulk toward root_chars so
                    // metrics reflect the failure mode of stuffing context.
                    root_chars = root_chars.saturating_add(content.len());
                }
            }
        }
    }

    RootTrajectory {
        tokens,
        root_chars,
        tool_hops,
        handle_receipts,
        aged_receipts,
        bulk_tool_results,
    }
}

/// Distance metrics on root fingerprints (1 = identical, 0 = disjoint).
/// Offline RL / Hi/Q analysis API — kept next to fingerprint construction so
/// forge scripts and unit tests share one implementation.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]
pub(crate) struct TrajectorySimilarity {
    pub(crate) jaccard_3gram: f64,
    pub(crate) containment_3gram: f64,
    pub(crate) weighted_jaccard_3gram: f64,
    pub(crate) length_ratio: f64,
    pub(crate) token_levenshtein_sim: f64,
}

#[allow(dead_code)]
pub(crate) fn trajectory_similarity(
    a: &RootTrajectory,
    b: &RootTrajectory,
) -> TrajectorySimilarity {
    let a_fp = a.fingerprint();
    let b_fp = b.fingerprint();
    let a_weighted = word_ngram_counts(&a_fp, 3);
    let b_weighted = word_ngram_counts(&b_fp, 3);
    TrajectorySimilarity {
        jaccard_3gram: jaccard(&a_weighted, &b_weighted),
        containment_3gram: containment(&a_weighted, &b_weighted),
        weighted_jaccard_3gram: weighted_jaccard(&a_weighted, &b_weighted),
        length_ratio: length_ratio(a.tokens.len(), b.tokens.len()),
        token_levenshtein_sim: levenshtein_sim(&a.tokens, &b.tokens),
    }
}

fn length_bucket(bytes: usize) -> &'static str {
    match bytes {
        0 => "0",
        1..=64 => "xs",
        65..=256 => "s",
        257..=1024 => "m",
        1025..=4096 => "l",
        4097..=16384 => "xl",
        _ => "xxl",
    }
}

#[allow(dead_code)]
fn word_ngram_counts(text: &str, n: usize) -> std::collections::BTreeMap<String, usize> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut counts = std::collections::BTreeMap::new();
    if words.len() < n {
        if !text.is_empty() {
            counts.insert(text.to_string(), 1);
        }
        return counts;
    }
    for window in words.windows(n) {
        *counts.entry(window.join(" ")).or_insert(0) += 1;
    }
    counts
}

#[allow(dead_code)]
fn weighted_jaccard(
    a: &std::collections::BTreeMap<String, usize>,
    b: &std::collections::BTreeMap<String, usize>,
) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let mut keys = std::collections::BTreeSet::new();
    keys.extend(a.keys());
    keys.extend(b.keys());
    let intersection = keys
        .iter()
        .map(|key| {
            a.get(*key)
                .copied()
                .unwrap_or(0)
                .min(b.get(*key).copied().unwrap_or(0))
        })
        .sum::<usize>();
    let union = keys
        .iter()
        .map(|key| {
            a.get(*key)
                .copied()
                .unwrap_or(0)
                .max(b.get(*key).copied().unwrap_or(0))
        })
        .sum::<usize>();
    if union == 0 {
        1.0
    } else {
        intersection as f64 / union as f64
    }
}

#[allow(dead_code)]
fn jaccard(
    a: &std::collections::BTreeMap<String, usize>,
    b: &std::collections::BTreeMap<String, usize>,
) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let inter = a.keys().filter(|k| b.contains_key(*k)).count() as f64;
    let union = a.len() as f64 + b.len() as f64 - inter;
    if union == 0.0 { 1.0 } else { inter / union }
}

#[allow(dead_code)]
fn containment(
    train: &std::collections::BTreeMap<String, usize>,
    eval: &std::collections::BTreeMap<String, usize>,
) -> f64 {
    if eval.is_empty() {
        return 1.0;
    }
    let inter = eval.keys().filter(|k| train.contains_key(*k)).count() as f64;
    inter / eval.len() as f64
}

#[allow(dead_code)]
fn length_ratio(a: usize, b: usize) -> f64 {
    let max = a.max(b);
    if max == 0 {
        1.0
    } else {
        a.min(b) as f64 / max as f64
    }
}

#[allow(dead_code)]
fn levenshtein_sim(a: &[String], b: &[String]) -> f64 {
    let max = a.len().max(b.len());
    if max == 0 {
        return 1.0;
    }
    let dist = levenshtein(a, b);
    1.0 - (dist as f64 / max as f64)
}

#[allow(dead_code)]
fn levenshtein(a: &[String], b: &[String]) -> usize {
    let (m, n) = (a.len(), b.len());
    let mut prev = (0..=n).collect::<Vec<_>>();
    let mut cur = vec![0; n + 1];
    for i in 1..=m {
        cur[0] = i;
        for j in 1..=n {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[n]
}

// ---------------------------------------------------------------------------
// Small pure helpers
// ---------------------------------------------------------------------------

fn bound_token(s: &str, max_chars: usize) -> String {
    let trimmed = s.trim();
    let mut out: String = trimmed.chars().take(max_chars).collect();
    if trimmed.chars().count() > max_chars {
        out.push('…');
    }
    // Keep receipts single-line.
    out.replace(['\n', '\r', '\t'], " ")
}

fn first_line_preview(body: &str, max_chars: usize) -> Option<String> {
    let line = body.lines().find(|l| !l.trim().is_empty())?;
    Some(bound_token(line, max_chars))
}

fn utf8_prefix(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

fn to_base36(mut n: u64) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".into();
    }
    let mut buf = Vec::new();
    while n > 0 {
        buf.push(DIGITS[(n % 36) as usize]);
        n /= 36;
    }
    buf.reverse();
    String::from_utf8(buf).unwrap_or_else(|_| "0".into())
}

fn sha256_hex(bytes: &[u8]) -> String {
    // Prefer a lightweight non-crypto fingerprint if sha2 is unavailable;
    // use std hasher + length for store integrity checks only (not security).
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    bytes.len().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> HandleStoreLimits {
        HandleStoreLimits {
            max_entries: 4,
            max_total_bytes: 10_000,
            max_body_bytes: 4_000,
            max_disclose_bytes: 64,
            max_paths: 2,
            max_preview_chars: 40,
        }
    }

    #[test]
    fn put_returns_bounded_receipt_without_body() {
        let mut store = HandleStore::new();
        let body = "SECRET_BULK_MARKER\nfn main() {}\n".repeat(50);
        let receipt = store
            .put(
                &body,
                PutMeta {
                    kind: HandleKind::ToolResult,
                    producer: "read_file",
                    identity: Some("read_file|src/main.rs"),
                    paths: &["src/main.rs".into()],
                    include_preview: false,
                },
                limits(),
                1,
            )
            .unwrap();
        let rendered = receipt.render();
        assert!(rendered.starts_with(HANDLE_RECEIPT_MARK));
        assert!(rendered.contains("hnd_"));
        assert!(rendered.contains("kind=tool_result"));
        assert!(rendered.contains("producer=read_file"));
        assert!(rendered.contains("identity=read_file|src/main.rs"));
        assert!(
            !rendered.contains("SECRET_BULK_MARKER"),
            "body must not leak into receipt: {rendered}"
        );
        assert_eq!(store.stats().entries, 1);
        assert_eq!(store.stats().total_bytes, body.len());
        assert!(store.get_receipt(receipt.handle.as_str()).is_some());
        assert!(store.content_sha256(receipt.handle.as_str()).is_some());
        assert!(store.contains(receipt.handle.as_str()));
        let _ = store
            .entries
            .get(receipt.handle.as_str())
            .map(|e| e.created_ms);
    }

    #[test]
    fn disclose_is_capped_and_offsetable() {
        let mut store = HandleStore::new();
        let body = "abcdefghijklmnopqrstuvwxyz0123456789".repeat(4); // 144 bytes
        let receipt = store
            .put(
                &body,
                PutMeta {
                    kind: HandleKind::Manual,
                    producer: "test",
                    identity: None,
                    paths: &[],
                    include_preview: false,
                },
                limits(),
                1,
            )
            .unwrap();
        let slice = store
            .disclose(receipt.handle.as_str(), 0, 10_000, limits(), 2)
            .unwrap();
        assert!(slice.truncated);
        assert_eq!(slice.bytes, 64);
        assert_eq!(slice.content, &body[..64]);
        let mid = store
            .disclose(receipt.handle.as_str(), 10, 5, limits(), 3)
            .unwrap();
        assert_eq!(mid.content, "klmno");
        assert_eq!(mid.offset, 10);
    }

    #[test]
    fn eviction_respects_entry_and_byte_caps() {
        let mut store = HandleStore::new();
        let lim = HandleStoreLimits {
            max_entries: 2,
            max_total_bytes: 100,
            max_body_bytes: 80,
            max_disclose_bytes: 32,
            max_paths: 1,
            max_preview_chars: 20,
        };
        let r1 = store
            .put(
                &"a".repeat(40),
                PutMeta {
                    kind: HandleKind::Manual,
                    producer: "a",
                    identity: None,
                    paths: &[],
                    include_preview: false,
                },
                lim,
                1,
            )
            .unwrap();
        let _r2 = store
            .put(
                &"b".repeat(40),
                PutMeta {
                    kind: HandleKind::Manual,
                    producer: "b",
                    identity: None,
                    paths: &[],
                    include_preview: false,
                },
                lim,
                2,
            )
            .unwrap();
        assert_eq!(store.stats().entries, 2);
        let _r3 = store
            .put(
                &"c".repeat(40),
                PutMeta {
                    kind: HandleKind::Manual,
                    producer: "c",
                    identity: None,
                    paths: &[],
                    include_preview: false,
                },
                lim,
                3,
            )
            .unwrap();
        assert_eq!(store.stats().entries, 2);
        assert!(!store.contains(r1.handle.as_str()), "oldest evicted");
        assert!(store.stats().evictions >= 1);
    }

    #[test]
    fn root_trajectory_collapses_bulk_and_preserves_strategy() {
        let mut history = vec![
            ChatMsg::system("sys preamble that should be ignored"),
            ChatMsg::user("fix the bug"),
        ];
        history.push(ChatMsg::assistant_calls(vec![ToolCall {
            id: "1".into(),
            name: "read_file".into(),
            args: serde_json::json!({"path":"a.rs"}),
        }]));
        history.push(ChatMsg::tool("1", "x".repeat(5000)));
        history.push(ChatMsg::assistant_calls(vec![ToolCall {
            id: "2".into(),
            name: "str_replace".into(),
            args: serde_json::json!({"path":"a.rs"}),
        }]));
        history.push(ChatMsg::tool("2", "ok"));
        history.push(ChatMsg::assistant("done"));

        let short = root_trajectory(&history);

        // Longer bulk, same tool order → same strategy tokens for calls.
        history[3].content = "y".repeat(50_000).into();
        let long = root_trajectory(&history);

        assert_eq!(
            short
                .tokens
                .iter()
                .filter(|t| t.starts_with("call:"))
                .collect::<Vec<_>>(),
            long.tokens
                .iter()
                .filter(|t| t.starts_with("call:"))
                .collect::<Vec<_>>(),
        );
        assert_eq!(short.tool_hops, long.tool_hops);
        assert!(short.bulk_tool_results >= 1);

        // Handle-backed receipts classify as handle, not bulk.
        history[3].content = format!(
            "{TOOL_AGED_MARK}: read_file|a.rs (5000 bytes) handle=hnd_1 — re-run the tool if needed]"
        )
        .into();
        let handled = root_trajectory(&history);
        assert_eq!(handled.handle_receipts, 1);
        // The small "ok" write result remains a bulk token; the large read does not.
        assert_eq!(handled.bulk_tool_results, 1);
        assert!(handled.tokens.iter().any(|t| t == "tool:handle"));
        assert!(
            !handled
                .tokens
                .iter()
                .any(|t| t.starts_with("tool:bulk:xl") || t.starts_with("tool:bulk:xxl"))
        );
    }

    #[test]
    fn trajectory_similarity_is_high_for_isomorphic_strategies() {
        let a = RootTrajectory {
            tokens: vec![
                "user:s".into(),
                "call:read_file".into(),
                "tool:handle".into(),
                "call:str_replace".into(),
                "tool:bulk:xs".into(),
                "asst:s".into(),
            ],
            root_chars: 200,
            tool_hops: 2,
            handle_receipts: 1,
            aged_receipts: 0,
            bulk_tool_results: 1,
        };
        let b = a.clone();
        let sim = trajectory_similarity(&a, &b);
        assert!((sim.jaccard_3gram - 1.0).abs() < 1e-9);
        assert!((sim.weighted_jaccard_3gram - 1.0).abs() < 1e-9);
        assert!((sim.token_levenshtein_sim - 1.0).abs() < 1e-9);

        let mut c = a.clone();
        c.tokens[2] = "tool:bulk:xxl".into(); // bulk instead of handle
        let sim2 = trajectory_similarity(&a, &c);
        assert!(sim2.token_levenshtein_sim < 1.0);
        assert!(sim2.jaccard_3gram < 1.0);
        assert!(sim2.weighted_jaccard_3gram < 1.0);
    }

    #[test]
    fn handle_kind_round_trips() {
        for kind in [
            HandleKind::ToolResult,
            HandleKind::CodeMode,
            HandleKind::Subcall,
            HandleKind::Manual,
        ] {
            assert_eq!(HandleKind::parse(kind.as_str()), Some(kind));
        }
    }

    #[test]
    fn session_put_and_disclose_round_trip_when_enabled() {
        let _lock = crate::tests::env_lock();
        let _on = EnvGuard::set("ANGEL_HANDLE_STORE", "1");
        session_clear();
        let body = "payload-line-one\npayload-line-two\n";
        let receipt = session_put(
            body,
            PutMeta {
                kind: HandleKind::ToolResult,
                producer: "read_file",
                identity: Some("read_file|x.rs"),
                paths: &["x.rs".into()],
                include_preview: false,
            },
        )
        .expect("store enabled");
        let slice = session_disclose(receipt.handle.as_str(), 0, 1024).unwrap();
        assert_eq!(slice.content, body);
        assert!(!slice.truncated);
        session_clear();
    }

    #[test]
    fn session_put_disabled_returns_none() {
        let _lock = crate::tests::env_lock();
        let _off = EnvGuard::set("ANGEL_HANDLE_STORE", "0");
        session_clear();
        assert!(
            session_put(
                "body",
                PutMeta {
                    kind: HandleKind::Manual,
                    producer: "t",
                    identity: None,
                    paths: &[],
                    include_preview: false,
                },
            )
            .is_none()
        );
    }

    #[test]
    fn maybe_offload_root_body_respects_floor_and_store() {
        let _lock = crate::tests::env_lock();
        let _on = EnvGuard::set("ANGEL_HANDLE_STORE", "1");
        session_clear();
        let small = "tiny";
        assert!(
            maybe_offload_root_body(small, HandleKind::Subcall, "spawn", "spawn|panel", 100)
                .is_none()
        );
        let large = "BULK_SUBCALL_".to_string() + &"z".repeat(200);
        let receipt =
            maybe_offload_root_body(&large, HandleKind::Subcall, "spawn", "spawn|panel", 64)
                .expect("large body offloads");
        assert!(receipt.contains(HANDLE_RECEIPT_MARK) || receipt.contains("hnd_"));
        assert!(!receipt.contains("BULK_SUBCALL_"));
        let handle = receipt
            .split("hnd_")
            .nth(1)
            .and_then(|s| s.split(|c: char| !c.is_ascii_alphanumeric()).next())
            .map(|s| format!("hnd_{s}"))
            .unwrap();
        let slice = session_disclose(&handle, 0, 512).unwrap();
        assert!(slice.content.contains("BULK_SUBCALL_"));
        session_clear();
    }

    #[test]
    fn eager_offload_parks_large_inspection_keeps_mutations() {
        let _lock = crate::tests::env_lock();
        let _store = EnvGuard::set("ANGEL_HANDLE_STORE", "1");
        let _read = EnvGuard::set("ANGEL_HANDLE_READ_TOOL", "1");
        let _min = EnvGuard::set("ANGEL_HANDLE_EAGER_TOOL_MIN_BYTES", "64");
        session_clear();

        let bulk = "READ_BODY_".to_string() + &"x".repeat(200);
        let off = eager_offload_tool_result("read_file", bulk.clone(), Some("read_file|a.rs"));
        assert!(off.offloaded, "large inspection should offload");
        assert!(off.content.contains(HANDLE_RECEIPT_MARK) || off.content.contains("hnd_"));
        assert!(!off.content.contains("READ_BODY_"));
        assert!(off.bytes_saved > 0);

        let write = eager_offload_tool_result("write_file", bulk, Some("write_file|a.rs"));
        assert!(!write.offloaded, "mutations stay complete");
        assert!(write.content.contains("READ_BODY_"));

        let err = eager_offload_tool_result(
            "read_file",
            "tool error: missing".into(),
            Some("read_file|x"),
        );
        assert!(!err.offloaded);
        session_clear();
    }

    #[test]
    fn eager_offload_keeps_body_when_disclosure_tool_is_disabled() {
        let _lock = crate::tests::env_lock();
        let _store = EnvGuard::set("ANGEL_HANDLE_STORE", "1");
        let _read = EnvGuard::set("ANGEL_HANDLE_READ_TOOL", "0");
        let _min = EnvGuard::set("ANGEL_HANDLE_EAGER_TOOL_MIN_BYTES", "64");
        session_clear();

        let body = "READ_BODY_".to_string() + &"x".repeat(200);
        let off = eager_offload_tool_result("read_file", body.clone(), Some("read_file|a.rs"));
        assert!(!off.offloaded);
        assert_eq!(off.content, body);
        session_clear();
    }

    #[test]
    fn inspection_identity_for_offload_skips_when_cannot_park() {
        use crate::club::ToolCall;
        let _lock = crate::tests::env_lock();
        let _store = EnvGuard::set("ANGEL_HANDLE_STORE", "1");
        let _read = EnvGuard::set("ANGEL_HANDLE_READ_TOOL", "1");
        let _min = EnvGuard::set("ANGEL_HANDLE_EAGER_TOOL_MIN_BYTES", "64");

        let read = ToolCall {
            id: "c1".into(),
            name: "read_file".into(),
            args: serde_json::json!({"path": "src/lib.rs"}),
        };
        let write = ToolCall {
            id: "c2".into(),
            name: "write_file".into(),
            args: serde_json::json!({"path": "src/lib.rs", "content": "x"}),
        };
        let grep = ToolCall {
            id: "c3".into(),
            name: "grep".into(),
            args: serde_json::json!({"pattern": "needle".repeat(20_000)}),
        };

        assert!(
            !eager_offload_can_park("write_file", &"x".repeat(200)),
            "mutations never park"
        );
        assert!(
            !eager_offload_can_park("read_file", "tiny"),
            "tiny inspections skip identity"
        );
        assert!(
            !eager_offload_can_park("read_file", "tool error: missing"),
            "errors skip identity"
        );
        assert!(eager_offload_can_park(
            "read_file",
            &("READ_BODY_".to_string() + &"x".repeat(200))
        ));

        assert!(inspection_identity_for_offload(&write, &"x".repeat(200)).is_none());
        assert!(inspection_identity_for_offload(&read, "tiny").is_none());
        assert!(inspection_identity_for_offload(&grep, "tiny").is_none());
        let parked =
            inspection_identity_for_offload(&read, &("READ_BODY_".to_string() + &"x".repeat(200)))
                .expect("large eligible read still identities");
        assert_eq!(parked, "read_file|src/lib.rs");
        assert_eq!(
            inspection_identity_for_offload(&read, &("READ_BODY_".to_string() + &"x".repeat(200))),
            crate::harness::inspection_identity(&read)
        );
    }

    /// Local env guard so this module's tests do not depend on harness::tests
    /// private helpers beyond `env_lock`.
    struct EnvGuard {
        key: &'static str,
        old: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let old = std::env::var_os(key);
            // SAFETY: tests hold env_lock when mutating process env.
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var(key, value) };
            Self { key, old }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(old) = &self.old {
                // TODO: Audit that the environment access only happens in single-threaded code.
                unsafe { std::env::set_var(self.key, old) };
            } else {
                // TODO: Audit that the environment access only happens in single-threaded code.
                unsafe { std::env::remove_var(self.key) };
            }
        }
    }
}
