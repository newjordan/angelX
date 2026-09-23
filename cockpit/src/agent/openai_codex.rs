//! OpenAI agent over **ChatGPT OAuth** — no API key. It reuses the tokens the
//! Codex CLI already obtained (`~/.codex/auth.json`), refreshes them when they
//! expire, and talks to the ChatGPT-backed **Responses API**
//! (`https://chatgpt.com/backend-api/codex/responses`) — the same endpoint /
//! headers / client-id Codex uses, extracted from the installed Codex binary.
//!
//! The Responses wire format differs from the OpenAI-compatible chat/completions
//! every other club speaks, so this is its own [`Club`]. Surfaced as the `openai`
//! agent in the bag, present only when a usable token is on disk.

use crate::agent::club::{
    CacheUsage, CacheUsageCell, ChatMsg, ChatRole, Club, ClubReply, Media,
    STREAM_INTERRUPTED_SUFFIX, StreamDelta, TokenUsage, ToolDef, UsageCell,
    mark_stream_interrupted, mark_truncated,
};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

#[cfg(test)]
use crate::agent::club::ToolCall;
use base64::Engine as _;

mod attempts;
mod events;
mod usage;
pub(crate) use events::{ResponseEvent, ResponseToolCalls, parse_responses_event};
pub(crate) use usage::{Usage, UsageStats, collect_rate_limits, format_usage};

/// The public Codex CLI OAuth client (from the installed Codex binary).
const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
const ORIGINATOR: &str = "codex_cli_rs";
const AUTH_JSON_ENV: &str = "ANGEL_OPENAI_AUTH_JSON";
/// Responses-route stream stall bound: give up on a stream that has sent no
/// events for this many seconds (`ANGEL_CODEX_STREAM_STALL_SECS`, default 120;
/// `0` disables the stall bound and keeps the plain socket read timeout below).
const DEFAULT_CODEX_STREAM_STALL_SECS: u64 = 120;
/// The plain per-read socket deadline used when the stall bound is off — the
/// historical fixed value this route always shipped.
const CODEX_PLAIN_READ_TIMEOUT_SECS: u64 = 300;
const CODEX_STREAM_STALL_ENV: &str = "ANGEL_CODEX_STREAM_STALL_SECS";

impl CodexClub {
    /// Responses-route stall: stamp the receipt, say it on stderr the way the
    /// chat route does, and return the error whose `stream stalled` substring
    /// the turn loop already classifies as transient.
    fn stall_error(&self, attempt: &mut attempts::Attempt<'_>, what: &str) -> String {
        attempt.outcome("stalled");
        let n = self.stream_stall_secs;
        eprintln!(
            "[club:{}] stream stalled — {what} for {n}s, giving up (ANGEL_CODEX_STREAM_STALL_SECS)",
            self.name
        );
        format!(
            "stream stalled: {what} for {n}s on {} (bound: ANGEL_CODEX_STREAM_STALL_SECS)",
            self.model
        )
    }
}

/// Resolve the Responses-route stall window once (same pattern as the chat
/// route's `env_secs` knobs in `club/http.rs`). Returns seconds; `0` = off.
fn codex_stream_stall_secs() -> u64 {
    crate::agent::club::env_secs(CODEX_STREAM_STALL_ENV, DEFAULT_CODEX_STREAM_STALL_SECS).as_secs()
}

/// Mirror of the chat route's `stream_read_timed_out` (`club/http.rs`): a
/// per-read socket deadline fires as `TimedOut`, or `WouldBlock` on platforms
/// whose SO_RCVTIMEO surfaces as EAGAIN.
fn stream_read_timed_out(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
    )
}

// ---------------------------------------------------------------------------
// auth — read & refresh the Codex ChatGPT tokens
// ---------------------------------------------------------------------------

/// The ChatGPT OAuth credentials Codex stores in `auth.json`. Cheap to clone.
#[derive(Clone, PartialEq, Eq)]
struct CredentialSnapshot {
    access_token: String,
    refresh_token: String,
    account_id: String,
}

#[derive(Clone)]
pub struct ChatGptAuth {
    access_token: String,
    refresh_token: String,
    account_id: String,
    /// Path to `auth.json`, so a rotated token can be written back in sync with Codex.
    path: PathBuf,
    /// Credentials last observed on disk. If a best-effort persist fails, the
    /// unchanged stale file must not replace the fresher in-memory token.
    disk_snapshot: Option<CredentialSnapshot>,
}

/// `~/.codex` (honoring `CODEX_HOME`).
fn codex_home_path() -> PathBuf {
    if let Ok(home) = std::env::var("CODEX_HOME")
        && !home.trim().is_empty()
    {
        return PathBuf::from(home);
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".codex")
}

/// `~/.codex/auth.json` (honoring `CODEX_HOME`).
fn auth_path() -> PathBuf {
    codex_home_path().join("auth.json")
}

fn auth_temp_path(path: &Path) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nonce = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("auth.json");
    path.with_file_name(format!(".{name}.angel-{}-{nonce}.tmp", std::process::id()))
}

fn config_path() -> PathBuf {
    codex_home_path().join("config.toml")
}

fn models_cache_path() -> PathBuf {
    codex_home_path().join("models_cache.json")
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
struct CodexConfig {
    model: Option<String>,
    model_reasoning_effort: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub(crate) struct CodexReasoningLevel {
    pub(crate) effort: String,
    #[serde(default)]
    pub(crate) description: String,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub(crate) struct CodexModelInfo {
    pub(crate) slug: String,
    #[allow(dead_code)]
    pub(crate) display_name: String,
    #[serde(default)]
    pub(crate) description: String,
    #[serde(default)]
    pub(crate) context_window: Option<u64>,
    #[serde(default)]
    pub(crate) input_modalities: Vec<String>,
    #[serde(default)]
    pub(crate) additional_speed_tiers: Vec<String>,
    #[serde(default)]
    pub(crate) default_reasoning_level: String,
    #[serde(default)]
    pub(crate) supported_reasoning_levels: Vec<CodexReasoningLevel>,
    #[serde(default)]
    pub(crate) visibility: String,
    #[serde(default)]
    pub(crate) priority: i64,
}

impl CodexModelInfo {
    pub(crate) fn route_metadata(&self) -> crate::agent::club::RouteMetadata {
        crate::agent::club::RouteMetadata {
            description: compact_metadata_text(&self.description, 320),
            context_window: self.context_window,
            input_modalities: self
                .input_modalities
                .iter()
                .filter_map(|value| compact_metadata_text(value, 32))
                .take(8)
                .collect(),
            speed_tiers: self
                .additional_speed_tiers
                .iter()
                .filter_map(|value| compact_metadata_text(value, 32))
                .take(8)
                .collect(),
            reasoning_descriptions: self
                .supported_reasoning_levels
                .iter()
                .filter_map(|level| {
                    Some((
                        compact_metadata_text(&level.effort, 32)?,
                        compact_metadata_text(&level.description, 320)?,
                    ))
                })
                .take(16)
                .collect(),
            output_budget: crate::agent::club::OutputBudgetPolicy::EndpointManaged,
            output_budget_provenance: Some("provider plan".to_string()),
        }
    }
}

fn compact_metadata_text(raw: &str, max_chars: usize) -> Option<String> {
    let normalized = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() || max_chars == 0 {
        return None;
    }
    if normalized.chars().count() <= max_chars {
        return Some(normalized);
    }
    let mut bounded = normalized
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    bounded.push('…');
    Some(bounded)
}

// Codex historically called its deepest reasoning level `ultra`, while the
// Responses API now accepts the equivalent wire value `xhigh`. Keep the local
// config input compatible, and normalize route state to
// the backend spelling so an older cache cannot break every live turn.
fn responses_wire_effort(effort: &str) -> &str {
    if effort.eq_ignore_ascii_case("ultra") {
        "xhigh"
    } else {
        effort
    }
}

#[derive(Debug, Default, serde::Deserialize)]
struct CodexModelsCache {
    #[serde(default)]
    models: Vec<CodexModelInfo>,
}

fn config_from_path(path: PathBuf) -> Option<CodexConfig> {
    let raw = std::fs::read_to_string(path).ok()?;
    config_from_str(&raw)
}

fn config_from_str(raw: &str) -> Option<CodexConfig> {
    let v = raw.parse::<toml::Value>().ok()?;
    Some(CodexConfig {
        model: v
            .get("model")
            .and_then(|x| x.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        model_reasoning_effort: v
            .get("model_reasoning_effort")
            .and_then(|x| x.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    })
}

fn model_catalog_from_path(path: PathBuf) -> Vec<CodexModelInfo> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    model_catalog_from_str(&raw)
}

fn model_catalog_from_str(raw: &str) -> Vec<CodexModelInfo> {
    let Ok(mut cache) = serde_json::from_str::<CodexModelsCache>(raw) else {
        return Vec::new();
    };
    cache.models.retain(|model| {
        (model.visibility.is_empty() || model.visibility == "list")
            && !is_private_test_codex_model(&model.slug)
    });
    cache.models.sort_by_key(|model| model.priority);
    cache.models
}

/// Private-test Codex surface that must never run competitions or default work.
/// "Spark" here is OpenAI's product name (gpt-5.3-codex-spark), not the DGX box.
pub(crate) fn is_private_test_codex_model(slug: &str) -> bool {
    let l = slug.trim().to_ascii_lowercase();
    l.contains("codex-spark")
        || l.contains("5.3-codex-spark")
        || l == "gpt-5.3-codex-spark"
        || l.ends_with("-spark") && l.contains("codex")
}

/// Operator-preferred OpenAI/Codex surface: Luna at max reasoning.
pub(crate) const OPENAI_LUNA_MODEL: &str = "gpt-5.6-luna";
pub(crate) const OPENAI_LUNA_EFFORT: &str = "max";

/// Historical normalization helper; production pins are validated, never remapped.
#[cfg(test)]
pub(crate) fn canonicalize_openai_model(slug: &str) -> String {
    let trimmed = slug.trim();
    if trimmed.is_empty() {
        return OPENAI_LUNA_MODEL.to_string();
    }
    if is_private_test_codex_model(trimmed) {
        return OPENAI_LUNA_MODEL.to_string();
    }
    trimmed.to_string()
}

impl ChatGptAuth {
    /// No ChatGPT login: an API-key Responses seat never reads or refreshes it.
    fn detached() -> Self {
        Self {
            access_token: String::new(),
            refresh_token: String::new(),
            account_id: String::new(),
            path: PathBuf::new(),
            disk_snapshot: None,
        }
    }

    /// Load an evaluator-scoped in-memory snapshot when explicitly supplied,
    /// otherwise use the ordinary Codex credential file. A present but invalid
    /// snapshot fails closed instead of falling back to a host credential path.
    pub fn load() -> Option<Self> {
        match std::env::var_os(AUTH_JSON_ENV) {
            Some(raw) => raw
                .to_str()
                .and_then(|raw| Self::from_json(raw, PathBuf::new())),
            None => Self::from_path(auth_path()),
        }
    }

    fn from_path(path: PathBuf) -> Option<Self> {
        let raw = std::fs::read_to_string(&path).ok()?;
        Self::from_json(&raw, path)
    }

    fn from_json(raw: &str, path: PathBuf) -> Option<Self> {
        let v: serde_json::Value = serde_json::from_str(raw).ok()?;
        Self::from_value(&v, path)
    }

    fn from_value(v: &serde_json::Value, path: PathBuf) -> Option<Self> {
        let tokens = v.get("tokens")?;
        let access_token = tokens.get("access_token")?.as_str()?.trim().to_string();
        if access_token.is_empty() {
            return None;
        }
        let refresh_token = tokens
            .get("refresh_token")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string();
        // account_id lives on `tokens` directly; fall back to the id_token claim.
        let account_id = tokens
            .get("account_id")
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| {
                tokens
                    .get("id_token")
                    .and_then(|x| x.as_str())
                    .and_then(account_id_from_jwt)
            })
            .unwrap_or_default();
        let disk_snapshot = Some(CredentialSnapshot {
            access_token: access_token.clone(),
            refresh_token: refresh_token.clone(),
            account_id: account_id.clone(),
        });
        Some(Self {
            access_token,
            refresh_token,
            account_id,
            path,
            disk_snapshot,
        })
    }

    /// Whether the access token is within `slack` seconds of expiry (or unreadable,
    /// in which case we refresh to be safe).
    fn is_expired(&self) -> bool {
        match jwt_exp(&self.access_token) {
            Some(exp) => exp - now_secs() < 60,
            None => true,
        }
    }

    fn same_credentials(&self, other: &Self) -> bool {
        self.credential_snapshot() == other.credential_snapshot()
    }

    fn credential_snapshot(&self) -> CredentialSnapshot {
        CredentialSnapshot {
            access_token: self.access_token.clone(),
            refresh_token: self.refresh_token.clone(),
            account_id: self.account_id.clone(),
        }
    }

    /// Adopt the current on-disk credentials when another Codex process has
    /// rotated or replaced them. `auth.json` is the cross-process source of
    /// truth; the shared mutex only coordinates this cockpit process.
    fn adopt_disk_credentials(&mut self) -> bool {
        let Some(disk) = Self::from_path(self.path.clone()) else {
            return false;
        };
        if self.disk_snapshot.as_ref() == disk.disk_snapshot.as_ref() {
            return false;
        }
        if self.same_credentials(&disk) {
            self.disk_snapshot = disk.disk_snapshot;
            return false;
        }
        *self = disk;
        true
    }

    fn adopt_disk_credentials_changed_since(&mut self, source: &Self) -> bool {
        let Some(disk) = Self::from_path(self.path.clone()) else {
            return false;
        };
        if source.same_credentials(&disk) {
            return false;
        }
        *self = disk;
        true
    }

    /// Refresh the access token via the OAuth refresh grant and persist the result
    /// (preserving the rest of `auth.json`) so Codex and the cockpit stay in sync.
    /// Retained for direct callers; the shared-state path splits this into
    /// [`perform_refresh_grant`](Self::perform_refresh_grant) (lock-free network)
    /// and [`commit_refresh`](Self::commit_refresh) so the network I/O runs off
    /// the auth mutex.
    #[allow(dead_code)]
    fn refresh(&mut self, agent: &ureq::Agent) -> Result<(), String> {
        if self.refresh_token.is_empty() {
            return Err("no refresh token in auth.json — run `codex login`".to_string());
        }
        let source = self.clone();
        let (access, refresh, id_token) = Self::perform_refresh_grant(agent, &self.refresh_token)?;
        self.commit_refresh(&source, access, refresh, id_token);
        Ok(())
    }

    /// The network half of a refresh: exchange `refresh_token` for a fresh access
    /// token via the OAuth grant. Pure network — holds no lock, borrows no auth
    /// state — so the single-flight path can run it without the `auth` mutex.
    /// Returns `(access_token, rotated_refresh_token, id_token)`.
    fn perform_refresh_grant(
        agent: &ureq::Agent,
        refresh_token: &str,
    ) -> Result<(String, String, Option<String>), String> {
        let body = serde_json::json!({
            "client_id": CLIENT_ID,
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
            "scope": "openid profile email",
        });
        let resp = agent
            .post(TOKEN_URL)
            .set("Content-Type", "application/json")
            .send_json(body)
            .map_err(|e| format!("token refresh failed: {}", describe_err(e)))?;
        let v: serde_json::Value = resp
            .into_json()
            .map_err(|e| format!("token refresh decode: {e}"))?;
        let access = v
            .get("access_token")
            .and_then(|x| x.as_str())
            .ok_or("token refresh: response had no access_token")?
            .to_string();
        // Refresh tokens rotate; keep the new one if present, else reuse.
        let refresh = v
            .get("refresh_token")
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| refresh_token.to_string());
        let id_token = v
            .get("id_token")
            .and_then(|x| x.as_str())
            .map(str::to_string);
        Ok((access, refresh, id_token))
    }

    /// Commit a completed grant under the `auth` lock. A concurrent Codex CLI may
    /// have rotated the shared refresh token while the request was in flight —
    /// its later disk snapshot wins, so we do not overwrite it with a response
    /// derived from the older snapshot.
    fn commit_refresh(
        &mut self,
        source: &Self,
        access: String,
        refresh: String,
        id_token: Option<String>,
    ) {
        if self.adopt_disk_credentials_changed_since(source) {
            return;
        }
        self.access_token = access;
        self.refresh_token = refresh;
        self.persist(id_token.as_deref(), source);
    }

    /// Best-effort write-back: re-read `auth.json`, update only the token fields,
    /// and replace it atomically (temp + rename). Never fails the caller.
    fn persist(&mut self, new_id_token: Option<&str>, source: &Self) {
        let Ok(raw) = std::fs::read_to_string(&self.path) else {
            return;
        };
        let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&raw) else {
            return;
        };
        let Some(disk) = Self::from_value(&v, self.path.clone()) else {
            return;
        };
        if !source.same_credentials(&disk) {
            *self = disk;
            return;
        }
        if let Some(tokens) = v.get_mut("tokens").and_then(|t| t.as_object_mut()) {
            tokens.insert("access_token".into(), self.access_token.clone().into());
            tokens.insert("refresh_token".into(), self.refresh_token.clone().into());
            if let Some(id) = new_id_token {
                tokens.insert("id_token".into(), id.to_string().into());
            }
        }
        let Ok(out) = serde_json::to_vec_pretty(&v) else {
            return;
        };
        let Ok(permissions) = std::fs::metadata(&self.path).map(|meta| meta.permissions()) else {
            return;
        };
        let tmp = auth_temp_path(&self.path);
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let Ok(mut file) = options.open(&tmp) else {
            return;
        };
        if file.write_all(&out).is_err()
            || file.sync_all().is_err()
            || std::fs::set_permissions(&tmp, permissions).is_err()
        {
            drop(file);
            let _ = std::fs::remove_file(&tmp);
            return;
        }
        drop(file);

        // Narrow the remaining compare/rename race without assuming the Codex
        // CLI cooperates with an Angel-specific advisory lock.
        if std::fs::read_to_string(&self.path).ok().as_deref() != Some(raw.as_str()) {
            let _ = std::fs::remove_file(&tmp);
            self.adopt_disk_credentials();
            return;
        }
        if std::fs::rename(&tmp, &self.path).is_ok() {
            self.disk_snapshot = Some(self.credential_snapshot());
        } else {
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

// ---------------------------------------------------------------------------
// the club — ChatGPT-backed Responses API
// ---------------------------------------------------------------------------

pub struct CodexClub {
    name: String,
    model: String,
    /// Backend-owned session setting. Unlike the old process-env lookup this is
    /// stable for every hop of a turn and can be changed safely between turns.
    reasoning_effort: Mutex<Option<String>>,
    selection: Option<crate::agent::club::codex_selection::Selection>,
    route_state_revision: AtomicU64,
    reasoning_levels: Vec<String>,
    route_metadata: crate::agent::club::RouteMetadata,
    /// Shared across every cache-backed model route. OAuth refresh tokens rotate,
    /// so cloned auth snapshots would make the next model retry a stale token.
    /// Usage is shared for the same reason `/status` calls it session-wide.
    shared: Arc<CodexSharedState>,
    agent: ureq::Agent,
    /// Tolerate an `incomplete: max_output_tokens` stream: keep the prose streamed
    /// so far (when no tool call is pending) instead of failing the turn. Set for
    /// the SOTA/metered link so a long MoA judge/aggregate draft is degraded, not
    /// discarded. Off by default → the main tool loop still fails closed.
    keep_truncated: bool,
    /// Whether this SOTA-tuned club may run pxpipe on eligible image-capable
    /// OpenAI models. Non-tuned instances keep their request bodies unchanged.
    pxpipe_candidate: bool,
    /// Whether this SOTA-tuned club should prepend brevity instructions.
    caveman_candidate: bool,
    /// Stable per-club `session_id` header value. The ChatGPT backend uses it
    /// for cache-affinity routing; a fresh id per request (the old behavior)
    /// lands each hop on a cache-cold worker and the whole conversation is
    /// re-processed at full cost every call. One id per club matches how the
    /// Codex CLI pins a conversation.
    session_id: String,
    /// The resolved `ANGEL_CODEX_STREAM_STALL_SECS` window (seconds; `0` =
    /// off). Snapshotted where the ureq agent is built so the read deadline
    /// and the stall error message always agree.
    stream_stall_secs: u64,
    /// The Responses endpoint: the ChatGPT Codex backend, or a provider's own
    /// `/v1/responses` for an API-key seat.
    responses_url: String,
    /// A provider API key in place of the ChatGPT OAuth login. Such a seat
    /// sends only `Authorization`, none of the ChatGPT account headers.
    api_key: Option<String>,
    /// Test-only endpoint override: points the Responses POST at a local
    /// server so the streaming loop's watchdogs can be exercised without the
    /// real ChatGPT backend. Always `None` in production.
    #[cfg(test)]
    responses_url_override: Option<String>,
}

pub(crate) struct CodexSharedState {
    auth: Mutex<ChatGptAuth>,
    /// Single-flight gate for token refresh: `.0` is "a refresh is in progress",
    /// `.1` wakes the waiters when it lands. Coordinating here keeps the OAuth
    /// grant's network round-trip *off* the `auth` mutex, so a concurrent
    /// valid-token reader is never blocked behind another route's refresh I/O,
    /// and a failed refresh can't stampede every waiter into its own retry.
    refresh_gate: (Mutex<bool>, Condvar),
    usage: Mutex<UsageStats>,
    /// Lock-free meter snapshot published after each `record_usage`. Draw/fold
    /// never wait on the Responses hop that is folding the mutex stats.
    usage_view: UsageCell,
    accounting: crate::agent::club::AccountingCell,
    cache_usage_view: CacheUsageCell,
}

impl CodexSharedState {
    fn new(auth: ChatGptAuth) -> Arc<Self> {
        Arc::new(Self {
            auth: Mutex::new(auth),
            refresh_gate: (Mutex::new(false), Condvar::new()),
            usage: Mutex::new(UsageStats::default()),
            usage_view: UsageCell::default(),
            accounting: crate::agent::club::AccountingCell::default(),
            cache_usage_view: CacheUsageCell::default(),
        })
    }
}

#[cfg(test)]
#[cfg(test)]
fn choose_reasoning_effort(
    levels: &[String],
    preferred: Option<&str>,
    fallback: Option<&str>,
) -> Option<String> {
    let matches = |candidate: &str| {
        levels
            .iter()
            .find(|level| level.eq_ignore_ascii_case(candidate.trim()))
            .cloned()
    };
    preferred
        .and_then(matches)
        .or_else(|| fallback.and_then(matches))
        .or_else(|| levels.first().cloned())
}

impl CodexClub {
    #[cfg(test)]
    pub fn new(name: impl Into<String>, model: impl Into<String>, auth: ChatGptAuth) -> Self {
        let model = model.into();
        let catalog = Self::model_catalog();
        let spec = catalog.iter().find(|candidate| candidate.slug == model);
        let levels = spec
            .map(|candidate| {
                candidate
                    .supported_reasoning_levels
                    .iter()
                    .map(|level| level.effort.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let configured = Self::configured_reasoning_effort();
        let effort = choose_reasoning_effort(
            &levels,
            configured.as_deref(),
            spec.map(|candidate| candidate.default_reasoning_level.as_str()),
        );
        let metadata = spec.map(CodexModelInfo::route_metadata).unwrap_or_default();
        Self::new_with_route_metadata_shared(
            name,
            model,
            CodexSharedState::new(auth),
            effort,
            levels,
            metadata,
        )
    }

    #[cfg(test)]
    pub(crate) fn new_with_reasoning(
        name: impl Into<String>,
        model: impl Into<String>,
        auth: ChatGptAuth,
        reasoning_effort: Option<String>,
        reasoning_levels: Vec<String>,
    ) -> Self {
        Self::new_with_reasoning_shared(
            name,
            model,
            CodexSharedState::new(auth),
            reasoning_effort,
            reasoning_levels,
        )
    }

    pub(crate) fn shared_state(auth: ChatGptAuth) -> Arc<CodexSharedState> {
        CodexSharedState::new(auth)
    }

    #[cfg(test)]
    pub(crate) fn new_with_reasoning_shared(
        name: impl Into<String>,
        model: impl Into<String>,
        shared: Arc<CodexSharedState>,
        reasoning_effort: Option<String>,
        reasoning_levels: Vec<String>,
    ) -> Self {
        Self::new_with_route_metadata_shared(
            name,
            model,
            shared,
            reasoning_effort,
            reasoning_levels,
            crate::agent::club::RouteMetadata::default(),
        )
    }

    pub(crate) fn new_with_route_metadata_shared(
        name: impl Into<String>,
        model: impl Into<String>,
        shared: Arc<CodexSharedState>,
        reasoning_effort: Option<String>,
        reasoning_levels: Vec<String>,
        mut route_metadata: crate::agent::club::RouteMetadata,
    ) -> Self {
        let reasoning_effort =
            reasoning_effort.map(|effort| responses_wire_effort(&effort).to_string());
        let reasoning_levels = reasoning_levels
            .into_iter()
            .map(|effort| responses_wire_effort(&effort).to_string())
            .collect();
        route_metadata.output_budget = crate::agent::club::OutputBudgetPolicy::EndpointManaged;
        route_metadata.output_budget_provenance = Some("provider plan".to_string());
        // Resolve the stall bound here so the agent's per-read deadline carries
        // it: with the knob on, a silent Responses stream surfaces as a read
        // timeout after `stall` seconds instead of the plain 300 s one. Silence
        // is otherwise mistaken for thinking until the socket deadline, and the
        // generic error path never names the stall or the knob.
        let stream_stall_secs = codex_stream_stall_secs();
        let read_timeout = if stream_stall_secs > 0 {
            Duration::from_secs(stream_stall_secs)
        } else {
            Duration::from_secs(CODEX_PLAIN_READ_TIMEOUT_SECS)
        };
        let agent = ureq::AgentBuilder::new()
            // OAuth/account headers are endpoint-bound. Refuse redirects so a
            // compromised endpoint cannot forward them to another origin.
            .redirects(0)
            .timeout_connect(Duration::from_secs(10))
            .timeout_read(read_timeout)
            .timeout_write(Duration::from_secs(60))
            .user_agent(concat!("angelX-cockpit/", env!("CARGO_PKG_VERSION")))
            .build();
        Self {
            name: name.into(),
            model: model.into(),
            reasoning_effort: Mutex::new(reasoning_effort),
            selection: None,
            route_state_revision: AtomicU64::new(0),
            reasoning_levels,
            route_metadata,
            shared,
            agent,
            keep_truncated: false,
            pxpipe_candidate: false,
            caveman_candidate: false,
            session_id: synth_session_id(),
            stream_stall_secs,
            responses_url: RESPONSES_URL.to_string(),
            api_key: None,
            #[cfg(test)]
            responses_url_override: None,
        }
    }

    /// A Responses-API seat on a provider API key (Meta's Muse Spark on
    /// `https://api.meta.ai/v1/responses`), sharing this client's request
    /// building, event decoding and stall handling. The provider's Chat
    /// Completions endpoint redacts the model's reasoning; the Responses API
    /// streams reasoning summaries, which reach the thinking panel.
    pub(crate) fn api_key_seat(
        name: impl Into<String>,
        model: impl Into<String>,
        responses_url: impl Into<String>,
        api_key: impl Into<String>,
        reasoning_effort: Option<String>,
        reasoning_levels: Vec<String>,
        route_metadata: crate::agent::club::RouteMetadata,
    ) -> Self {
        let mut club = Self::new_with_route_metadata_shared(
            name,
            model,
            CodexSharedState::new(ChatGptAuth::detached()),
            reasoning_effort,
            reasoning_levels,
            route_metadata,
        );
        club.responses_url = responses_url.into();
        club.api_key = Some(api_key.into());
        club
    }

    /// Tune this link for the mixture-of-agents path: keep a reply cut off at the
    /// output-token cap as usable material rather than failing the turn. Only the
    /// SOTA link opts in; the value flows into [`Self::run`]'s incomplete handling.
    pub fn sota_tuned(mut self) -> Self {
        self.keep_truncated = true;
        self.pxpipe_candidate = true;
        self.caveman_candidate = true;
        self
    }

    pub fn default_model() -> Option<String> {
        let raw = std::env::var("ANGEL_OPENAI_MODEL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| config_from_path(config_path()).and_then(|config| config.model));
        Some(raw.unwrap_or_else(|| OPENAI_LUNA_MODEL.into()))
    }

    pub(crate) fn resolve_selection() -> crate::agent::club::codex_selection::Selection {
        let config = config_from_path(config_path()).unwrap_or_default();
        let env = |key| std::env::var(key).ok().filter(|s| !s.trim().is_empty());
        crate::agent::club::codex_selection::resolve(
            env("ANGEL_OPENAI_MODEL"),
            env("ANGEL_OPENAI_REASONING_EFFORT").or_else(|| env("ANGEL_REASONING_EFFORT")),
            config.model,
            config.model_reasoning_effort,
            &Self::model_catalog(),
        )
    }

    pub(crate) fn with_selection(
        mut self,
        selection: crate::agent::club::codex_selection::Selection,
    ) -> Self {
        self.model.clone_from(&selection.model);
        *self.reasoning_effort.get_mut().expect("new route lock") = Some(selection.effort.clone());
        if !self.reasoning_levels.contains(&selection.effort) && selection.error.is_none() {
            self.reasoning_levels.push(selection.effort.clone());
        }
        self.selection = Some(selection);
        self
    }

    #[cfg(test)]
    pub(crate) fn configured_reasoning_effort() -> Option<String> {
        let effort = std::env::var("ANGEL_OPENAI_REASONING_EFFORT")
            .ok()
            .or_else(|| std::env::var("ANGEL_REASONING_EFFORT").ok())
            .map(|effort| effort.trim().to_ascii_lowercase())
            .filter(|effort| !effort.is_empty())
            .or_else(|| {
                config_from_path(config_path()).and_then(|config| config.model_reasoning_effort)
            })
            .map(|e| e.to_ascii_lowercase());
        // Luna competition seat: prefer max when nothing is pinned.
        Some(
            effort
                .filter(|e| !e.is_empty())
                .unwrap_or_else(|| OPENAI_LUNA_EFFORT.to_string()),
        )
    }

    pub(crate) fn model_catalog() -> Vec<CodexModelInfo> {
        model_catalog_from_path(models_cache_path())
    }

    /// Fold one turn's reported usage into the session running totals.
    fn record_usage(&self, u: Usage) {
        if let Some(cached) = u.cached_input {
            self.shared.cache_usage_view.add_read(cached);
        }
        if let Some(written) = u.cache_write {
            self.shared.cache_usage_view.add_write(written);
        }
        if let Ok(mut s) = self.shared.usage.lock() {
            s.turns += 1;
            s.last = u;
            s.total_input = s.total_input.saturating_add(u.input.unwrap_or(0));
            s.total_output = s.total_output.saturating_add(u.output.unwrap_or(0));
            s.total_reasoning = s.total_reasoning.saturating_add(u.reasoning.unwrap_or(0));
            self.shared.usage_view.store(TokenUsage {
                turns: s.turns,
                last_input: s.last.input.unwrap_or(0),
                last_output: s.last.output.unwrap_or(0),
                last_reasoning: s.last.reasoning.unwrap_or(0),
                total_input: s.total_input,
                total_output: s.total_output,
                total_reasoning: s.total_reasoning,
            });
        }
    }

    /// A valid `(access_token, account_id)`, refreshing first if it's expired.
    /// Error-message prefix: `openai` for the ChatGPT seat, the seat's own
    /// label for an API-key provider, so a Muse failure is not read as OpenAI's.
    fn provider_label(&self) -> &str {
        if self.api_key.is_some() {
            &self.name
        } else {
            "openai"
        }
    }

    fn token(&self) -> Result<(String, String), String> {
        if let Some(key) = &self.api_key {
            return Ok((key.clone(), String::new()));
        }
        // Fast path: adopt any cross-process rotation and return a still-valid
        // token. The auth lock is held only for this cheap check, never across a
        // network refresh.
        {
            let mut auth = self.shared.auth.lock().map_err(|_| "auth lock poisoned")?;
            auth.adopt_disk_credentials();
            if !auth.is_expired() {
                return Ok((auth.access_token.clone(), auth.account_id.clone()));
            }
        }
        self.refresh_token_single_flight()
    }

    /// Refresh an expired token under a single-flight gate: exactly one thread
    /// performs the OAuth grant while the rest wait on the condvar and reuse its
    /// result, instead of each queuing on the auth mutex behind the network I/O
    /// (or, worse, each firing its own redundant refresh).
    fn refresh_token_single_flight(&self) -> Result<(String, String), String> {
        let (gate, cvar) = &self.shared.refresh_gate;
        let mut refreshing = gate.lock().map_err(|_| "refresh gate poisoned")?;
        if *refreshing {
            // Another thread owns the refresh — wait for it, then reuse the token
            // it committed rather than launching a duplicate grant.
            while *refreshing {
                refreshing = cvar.wait(refreshing).map_err(|_| "refresh gate poisoned")?;
            }
            drop(refreshing);
            let auth = self.shared.auth.lock().map_err(|_| "auth lock poisoned")?;
            if !auth.is_expired() {
                return Ok((auth.access_token.clone(), auth.account_id.clone()));
            }
            return Err(
                "codex token refresh did not yield a valid token — the owning refresh failed"
                    .to_string(),
            );
        }
        // Become the sole refresher, then always clear the gate + wake waiters.
        *refreshing = true;
        drop(refreshing);
        let result = self.perform_owned_refresh();
        {
            let mut refreshing = gate.lock().map_err(|_| "refresh gate poisoned")?;
            *refreshing = false;
            cvar.notify_all();
        }
        result
    }

    /// The owning refresher: snapshot under the lock, run the OAuth grant
    /// lock-free, then commit under the lock. Re-checks expiry first so a token
    /// already refreshed while we waited on the gate skips the network entirely.
    fn perform_owned_refresh(&self) -> Result<(String, String), String> {
        let (source, refresh_token) = {
            let auth = self.shared.auth.lock().map_err(|_| "auth lock poisoned")?;
            if !auth.is_expired() {
                return Ok((auth.access_token.clone(), auth.account_id.clone()));
            }
            (auth.clone(), auth.refresh_token.clone())
        };
        if refresh_token.is_empty() {
            return Err("no refresh token in auth.json — run `codex login`".to_string());
        }
        let (access, refresh, id_token) =
            ChatGptAuth::perform_refresh_grant(&self.agent, &refresh_token)?;
        let mut auth = self.shared.auth.lock().map_err(|_| "auth lock poisoned")?;
        auth.commit_refresh(&source, access, refresh, id_token);
        Ok((auth.access_token.clone(), auth.account_id.clone()))
    }

    /// Build the Responses request body from the conversation: system turns become
    /// `instructions`, the rest become `input` message items.
    #[cfg(test)]
    fn build_request(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> serde_json::Value {
        self.build_request_with_effort(messages, tools, None)
    }

    fn build_request_with_effort(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        effort: Option<&str>,
    ) -> serde_json::Value {
        use serde_json::json;
        let fallback_effort = effort.is_none().then(|| self.reasoning_effort()).flatten();
        let effort = effort.or(fallback_effort.as_deref());
        // Brevity instruction for the metered link: folded into `instructions`
        // right after the leading system block — the history is never cloned
        // to carry it (it used to be, every request).
        let caveman = if self.caveman_candidate {
            crate::agent::club::sota_caveman_insert(messages)
        } else {
            None
        };
        let mut sys_parts: Vec<&str> = messages
            .iter()
            .filter(|m| m.role == ChatRole::System)
            .map(|m| m.content.as_ref())
            .collect();
        let caveman_text;
        if let Some((_, text)) = caveman {
            let lead = messages
                .iter()
                .take_while(|m| m.role == ChatRole::System)
                .count();
            caveman_text = text;
            sys_parts.insert(lead, &caveman_text);
        }
        let instructions = sys_parts.join("\n\n");
        let mut input: Vec<serde_json::Value> = Vec::new();
        for m in messages.iter().filter(|m| m.role != ChatRole::System) {
            match m.role {
                ChatRole::Assistant if !m.tool_calls.is_empty() => {
                    if !m.content.trim().is_empty() {
                        input.push(json!({
                            "type": "message",
                            "role": "assistant",
                            "content": responses_content_parts(m, "output_text"),
                        }));
                    }
                    for call in m.tool_calls.iter() {
                        input.push(json!({
                            "type": "function_call",
                            "call_id": call.id,
                            "name": call.name,
                            "arguments": call.args.to_string(),
                        }));
                    }
                }
                ChatRole::Assistant => input.push(json!({
                    "type": "message",
                    "role": "assistant",
                    "content": responses_content_parts(m, "output_text"),
                })),
                ChatRole::Tool => input.push(json!({
                    "type": "function_call_output",
                    "call_id": m.tool_call_id.as_deref().unwrap_or("tool"),
                    "output": m.content,
                })),
                ChatRole::System | ChatRole::User | ChatRole::Harness => input.push(json!({
                    "type": "message",
                    "role": "user",
                    "content": responses_content_parts(m, "input_text"),
                })),
            }
        }
        let mut body = json!({
            "model": self.model,
            "instructions": instructions,
            "input": input,
            "stream": true,
            "store": false,
        });
        if !tools.is_empty() {
            body["tools"] = json!(responses_tool_defs(tools));
            if crate::agent::club::final_response_requested(messages) {
                body["tool_choice"] = json!("none");
            }
        }
        // Request reasoning summaries alongside the operator-selected effort.
        // Hosted OpenAI models expose ONLY summaries — never the verbatim
        // reasoning stream — so suppressing summaries blanks the Agent thinking
        // panel for the entire seat (2026-07-31 operator report). The panel
        // labels the stream as provider-exposed rather than claiming verbatim.
        if let Some(obj) = body.as_object_mut() {
            let mut reasoning = serde_json::Map::new();
            reasoning.insert("summary".into(), "auto".into());
            if let Some(effort) = effort.and_then(|requested| {
                self.reasoning_levels
                    .iter()
                    .find(|level| {
                        responses_wire_effort(level) == responses_wire_effort(requested.trim())
                    })
                    .map(String::as_str)
            }) {
                reasoning.insert("effort".into(), responses_wire_effort(effort).into());
            }
            obj.insert("reasoning".into(), reasoning.into());
        }
        // NOTE: the ChatGPT-backed Codex Responses endpoint rejects
        // `max_output_tokens` ("HTTP 400: Unsupported parameter") — output length
        // is governed by the plan, not a request field — so we never send it here.
        // The "raise the cap" lever lives on the OpenAI-compatible HTTP clubs
        // (`max_tokens`); this path relies on `keep_truncated` to keep a cut-off
        // reply as usable material instead.
        body
    }

    /// Core streaming call: POST the Responses request, decode the SSE event
    /// stream, push deltas through `on_delta`, and return the full answer text.
    fn run(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        self.run_with_effort(messages, tools, None, cancel, on_delta)
    }

    fn run_with_effort(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        effort: Option<&str>,
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        use crate::agent::club::wire_log::{WireCall, WireWindows};
        let wire = WireCall::new(&self.name, &self.model, "responses");
        wire.arm(WireWindows {
            stall_secs: self.stream_stall_secs,
            first_token_secs: self.stream_stall_secs,
            hard_secs: 0,
            stall_source: if std::env::var_os(CODEX_STREAM_STALL_ENV).is_some() {
                "env".to_string()
            } else {
                "default".to_string()
            },
        });
        let result =
            self.run_with_effort_observed(messages, tools, effort, cancel, on_delta, &wire);
        wire.finish(&result);
        result
    }

    /// The Responses round itself; `wire` observes it (see `club::wire_log`).
    fn run_with_effort_observed(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        effort: Option<&str>,
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
        wire: &crate::agent::club::wire_log::WireCall,
    ) -> Result<ClubReply, String> {
        use std::sync::atomic::Ordering;
        if let Some(error) = self.selection.as_ref().and_then(|s| s.error.as_ref()) {
            return Err(error.clone());
        }
        if let Some(requested) = effort
            && !self
                .reasoning_levels
                .iter()
                .any(|level| responses_wire_effort(level) == responses_wire_effort(requested))
        {
            return Err(format!(
                "OpenAI effort {requested:?} is not supported for {}; supported efforts: [{}]",
                self.model,
                self.reasoning_levels.join(", ")
            ));
        }
        wire.phase("auth");
        let (token, account) = self.token()?;
        wire.phase("request");
        let body = self.build_request_with_effort(messages, tools, effort);
        let bytes = serde_json::to_vec(&body).map_err(|e| format!("encode request: {e}"))?;
        let bytes = if self.pxpipe_candidate {
            crate::agent::club::maybe_pxpipe_transform(
                crate::agent::club::PxpipeApi::Responses,
                &self.name,
                &self.model,
                bytes,
            )?
        } else {
            bytes
        };
        let mut attempt = attempts::Attempt::new(self, &bytes);
        // Tests point the POST at a local TCP server; production always keeps
        // the real ChatGPT Responses endpoint.
        #[cfg(test)]
        let responses_url = self
            .responses_url_override
            .as_deref()
            .unwrap_or(&self.responses_url);
        #[cfg(not(test))]
        let responses_url = self.responses_url.as_str();
        // Observation only: binding the run identity must never change the
        // request path (see HttpClub::send_with_retry). Failures leave the
        // identity unbound and are reported as such by the coverage report.
        if crate::agent::harness::run_identity::current().is_none()
            && let Ok(wire) = serde_json::from_slice::<serde_json::Value>(&bytes)
        {
            let mut defaults = self.resolved_model_defaults();
            defaults["model"] = wire["model"].clone();
            defaults["reasoning_effort"] = wire["reasoning"]["effort"].clone();
            if effort.is_some() {
                defaults["reasoning_effort_source"] = serde_json::json!("env");
            }
            crate::agent::harness::run_identity::prepare_model_defaults(defaults);
            let _ = crate::agent::harness::run_identity::bind(
                crate::agent::harness::run_identity::Model {
                    club: self.name.clone(),
                    id: wire["model"].as_str().unwrap_or(&self.model).to_string(),
                    driver: self.name.clone(),
                    base_url: crate::agent::harness::run_identity::endpoint_identity(responses_url),
                },
                crate::agent::harness::run_identity::wire_effort(&wire),
                wire.get("max_output_tokens")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!("endpoint-managed")),
                Some(self.stream_stall_secs),
            );
        }
        let mut request = self
            .agent
            .post(responses_url)
            .set("Content-Type", "application/json")
            .set("Accept", "text/event-stream")
            .set("Authorization", &format!("Bearer {token}"));
        if self.api_key.is_none() {
            request = request
                .set("ChatGPT-Account-Id", &account)
                .set("OpenAI-Beta", "responses=experimental")
                .set("originator", ORIGINATOR)
                .set("session_id", &self.session_id);
        }
        let resp = request.send_bytes(&bytes).map_err(|e| {
            let detail = match e {
                ureq::Error::Status(code, response) => {
                    let mut body = String::new();
                    let _ = std::io::Read::read_to_string(
                        &mut attempt.response_reader(response.into_reader()),
                        &mut body,
                    );
                    describe_response_error(code, &body)
                }
                other => describe_err(other),
            };
            format!("{} responses: {detail}", self.provider_label())
        })?;

        // Plan rate-limits ride on the response headers; capture them before the
        // body is consumed. Best-effort — absent on endpoints that don't send them.
        let rate = collect_rate_limits(&resp);
        if !rate.is_empty()
            && let Ok(mut s) = self.shared.usage.lock()
        {
            s.rate_limits = rate;
        }

        wire.phase("streaming");
        attempt.outcome("interrupted");
        let reader = BufReader::new(attempt.response_reader(resp.into_reader()));
        let mut content = String::new();
        let mut tool_calls = ResponseToolCalls::default();
        let mut saw_done = false;
        // Heartbeat-aware stall clock: the read deadline only catches a socket
        // that sends no bytes, but a Responses stream can keep sending
        // `response.in_progress` / comment lines while producing nothing for
        // minutes. Like the chat route's "keep-alives but no data" rule, time
        // the last MEANINGFUL event (text, reasoning, tool-call, terminal).
        let mut last_meaningful = std::time::Instant::now();
        let stall_window = Duration::from_secs(self.stream_stall_secs);
        for line in reader.lines() {
            let cancelled = cancel.load(Ordering::Relaxed);
            let line = match line {
                Ok(l) => l,
                Err(_) if cancelled => {
                    attempt.outcome("cancelled");
                    return Ok(ClubReply::Text(content));
                }
                Err(e) => {
                    // A severed stream keeps already-streamed prose (when no
                    // tool call is mid-flight — its args would be half-written)
                    // as an interrupted partial, including a stall-bound
                    // timeout.
                    if !content.is_empty() && !tool_calls.pending() {
                        on_delta(StreamDelta::Content(STREAM_INTERRUPTED_SUFFIX));
                        return Ok(ClubReply::Text(mark_stream_interrupted(&content)));
                    }
                    // Responses-route stall bound: the ureq read deadline
                    // carries `ANGEL_CODEX_STREAM_STALL_SECS`, so a
                    // TimedOut/WouldBlock read with no streamed answer names
                    // the stall and the knob. The "stream stalled" substring
                    // matches the chat route's stall message, so the turn
                    // loop's provider-retry classification treats the two
                    // routes alike (it stays transient — replay can help).
                    if self.stream_stall_secs > 0 && stream_read_timed_out(&e) {
                        return Err(self.stall_error(&mut attempt, "no Responses events"));
                    }
                    return Err(format!("stream read error: {e}"));
                }
            };
            wire.bytes(line.len() + 1);
            observe_responses_line(wire, &line);
            // A blocking read may yield a terminal usage frame just as cancel
            // flips. Account for the received frame before suppressing output.
            let event = attempt.receive(&line, cancelled);
            observe_responses_event(wire, &event);
            if cancelled {
                return Ok(ClubReply::Text(content));
            }
            match event {
                ResponseEvent::Text(d) => {
                    content.push_str(&d);
                    on_delta(StreamDelta::Content(&d));
                }
                ResponseEvent::Reasoning(r) => on_delta(StreamDelta::Reasoning(&r)),
                // Hosted models never expose verbatim reasoning; the summary
                // stream is the only thinking the provider will ever show us.
                // Forward it — an always-empty thinking panel reads as the
                // feature not existing.
                ResponseEvent::ReasoningSummary(r) => on_delta(StreamDelta::Reasoning(&r)),
                ResponseEvent::ToolCallStart { key, call_id, name } => {
                    tool_calls.start(key, call_id, name);
                }
                ResponseEvent::ToolArgumentsDelta { key, delta } => {
                    tool_calls.push_args(&key, &delta);
                }
                ResponseEvent::ToolArgumentsDone { key, arguments } => {
                    tool_calls.set_args(&key, arguments);
                }
                ResponseEvent::ToolCallDone { key, call } => {
                    tool_calls.done(key, call);
                }
                ResponseEvent::Failed(msg, _) => {
                    return Err(format!("{}: {msg}", self.provider_label()));
                }
                ResponseEvent::Incomplete(msg, _) => {
                    // Cut off at the output cap. A SOTA-tuned link keeps the prose
                    // streamed so far (when no tool call is mid-flight — its args
                    // would be half-written) as usable MoA material, marked so it's
                    // never mistaken for a complete answer. Otherwise fail closed.
                    if self.keep_truncated && !content.trim().is_empty() && !tool_calls.pending() {
                        return Ok(ClubReply::Text(mark_truncated(
                            &content,
                            crate::agent::club::OutputBudgetPolicy::EndpointManaged,
                        )));
                    }
                    return Err(format!("{}: {msg}", self.provider_label()));
                }
                ResponseEvent::Done(_) => {
                    saw_done = true;
                    break;
                }
                ResponseEvent::Ignore | ResponseEvent::Usage(_) => {
                    // Keep-alive / bookkeeping frames do not count as progress.
                    if self.stream_stall_secs > 0 && last_meaningful.elapsed() >= stall_window {
                        if !content.is_empty() && !tool_calls.pending() {
                            on_delta(StreamDelta::Content(STREAM_INTERRUPTED_SUFFIX));
                            return Ok(ClubReply::Text(mark_stream_interrupted(&content)));
                        }
                        return Err(
                            self.stall_error(&mut attempt, "keep-alives but no Responses output")
                        );
                    }
                    continue;
                }
            }
            last_meaningful = std::time::Instant::now();
        }
        if !saw_done {
            if tool_calls.pending() {
                return Err(
                    "openai stream ended before response.completed; incomplete tool call discarded"
                        .to_string(),
                );
            }
            if content.is_empty() {
                return Err("openai stream ended before response.completed".to_string());
            }
            on_delta(StreamDelta::Content(STREAM_INTERRUPTED_SUFFIX));
            return Ok(ClubReply::Text(mark_stream_interrupted(&content)));
        }
        let (calls, notes) = tool_calls.into_calls_with_notes();
        for (kind, message) in &notes {
            wire.note(kind, message);
        }
        if !calls.is_empty() {
            Ok(ClubReply::Calls(calls))
        } else if tools.is_empty() {
            // Tool-less request (swarm/deli worker): nothing a "recovered" call
            // could execute, so wrapper-looking text stays a prose answer.
            Ok(ClubReply::Text(content))
        } else {
            let recovered = crate::agent::club::extract_prose_tool_calls(&content);
            if recovered.is_empty() {
                Ok(ClubReply::Text(content))
            } else {
                Ok(ClubReply::Calls(recovered))
            }
        }
    }
}

/// Feed one raw Responses SSE line to the wire log: its event type goes into the
/// call's histogram, and an output item of a type this parser does not dispatch
/// (anything but a function call, message or reasoning) is noted, so a tool call
/// the model made in another shape cannot vanish silently.
fn observe_responses_line(wire: &crate::agent::club::wire_log::WireCall, line: &str) {
    let Some(data) = line.strip_prefix("data:").map(str::trim) else {
        return;
    };
    if data.is_empty() || data == "[DONE]" {
        return;
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(data) else {
        wire.note(
            "unparsed_event",
            &format!("not JSON: {}", data.chars().take(160).collect::<String>()),
        );
        return;
    };
    let kind = v.get("type").and_then(|t| t.as_str()).unwrap_or("untyped");
    wire.event(kind);
    if matches!(
        kind,
        "response.output_item.added" | "response.output_item.done"
    ) && let Some(item) = v.get("item")
    {
        let item_type = item
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("untyped");
        if !matches!(item_type, "function_call" | "message" | "reasoning") {
            wire.note(
                &format!("ignored_item:{item_type}"),
                &format!(
                    "{kind} carried a {item_type} item (name {}, output {}) that is not dispatched",
                    item.get("name").and_then(|n| n.as_str()).unwrap_or("-"),
                    v.get("output_index")
                        .map(|i| i.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                ),
            );
        }
    }
}

fn observe_responses_event(wire: &crate::agent::club::wire_log::WireCall, event: &ResponseEvent) {
    let usage = |usage: &Usage| {
        if let Ok(value) = serde_json::to_value(usage) {
            wire.set_usage(value);
        }
    };
    match event {
        ResponseEvent::Text(text) => wire.text(text.len()),
        ResponseEvent::Reasoning(r) | ResponseEvent::ReasoningSummary(r) => wire.reasoning(r.len()),
        ResponseEvent::ToolCallStart { .. }
        | ResponseEvent::ToolArgumentsDelta { .. }
        | ResponseEvent::ToolArgumentsDone { .. }
        | ResponseEvent::ToolCallDone { .. } => wire.tool_frame(),
        ResponseEvent::Done(u) => {
            wire.set_finish_reason(Some("completed"));
            u.as_ref().map(usage);
        }
        ResponseEvent::Failed(_, u) => {
            wire.set_finish_reason(Some("failed"));
            u.as_ref().map(usage);
        }
        ResponseEvent::Incomplete(_, u) => {
            wire.set_finish_reason(Some("incomplete"));
            u.as_ref().map(usage);
        }
        ResponseEvent::Usage(u) => usage(u),
        ResponseEvent::Ignore => wire.keepalive(),
    }
}

fn responses_tool_defs(tools: &[ToolDef]) -> Vec<serde_json::Value> {
    use serde_json::json;
    tools
        .iter()
        .map(|t| {
            json!({
                "type": "function",
                "name": t.name,
                "description": t.description,
                "parameters": t.params,
                // Tool contracts distinguish omission from explicit values (for
                // shell, write_paths: [] enforces read-only/no-network scope).
                // Responses may normalize omitted strict into required fields.
                // Preserve the registry schema instead of changing its defaults.
                "strict": false,
            })
        })
        .collect()
}

fn responses_content_parts(m: &ChatMsg, text_kind: &str) -> Vec<serde_json::Value> {
    use serde_json::json;
    let mut parts = vec![json!({ "type": text_kind, "text": m.content })];
    if m.role != ChatRole::User {
        return parts;
    }
    parts.extend(m.attachments.iter().map(|media| match media {
        Media::Image { mime, b64 } => json!({
            "type": "input_image",
            "image_url": format!("data:{mime};base64,{b64}"),
        }),
        Media::Audio { format, b64 } => json!({
            "type": "input_audio",
            "input_audio": { "data": b64, "format": format },
        }),
    }));
    parts
}

impl Club for CodexClub {
    fn bind_run_identity(&self, _effort: Option<&str>) -> Result<(), String> {
        Ok(())
    }

    fn respond(&self, prompt: &str) -> Result<String, String> {
        match self.run(
            &[ChatMsg::user(prompt)],
            &[],
            &AtomicBool::new(false),
            &mut |_| {},
        )? {
            ClubReply::Text(text) => Ok(text),
            ClubReply::Calls(_) => Err("openai requested a tool with no tools offered".to_string()),
        }
    }

    fn label(&self) -> &str {
        &self.name
    }

    fn live_model_name(&self) -> Option<String> {
        Some(self.model.clone())
    }

    fn reasoning_effort(&self) -> Option<String> {
        self.reasoning_effort
            .lock()
            .ok()
            .and_then(|effort| effort.clone())
    }

    fn resolved_model_defaults(&self) -> serde_json::Value {
        let mut defaults = crate::agent::club::model_defaults::budgets(&self.model, "ANGEL_OPENAI");
        let selection = self.selection.as_ref();
        defaults["model"] = serde_json::json!(self.model);
        defaults["model_source"] =
            serde_json::json!(selection.map_or("fallback", |s| s.model_source));
        defaults["reasoning_effort"] = serde_json::json!(
            self.reasoning_effort()
                .as_deref()
                .map(responses_wire_effort)
        );
        defaults["reasoning_effort_source"] =
            serde_json::json!(if self.route_state_revision.load(Ordering::Relaxed) > 0 {
                "env"
            } else {
                selection.map_or("fallback", |s| s.effort_source)
            });
        defaults["selection_error"] = serde_json::json!(selection.and_then(|s| s.error.as_ref()));
        defaults
    }

    fn reasoning_levels(&self) -> &[String] {
        &self.reasoning_levels
    }

    fn route_metadata(&self) -> crate::agent::club::RouteMetadata {
        self.route_metadata.clone()
    }

    fn header_route_metadata(&self) -> crate::agent::club::RouteMetadata {
        crate::agent::club::RouteMetadata {
            context_window: self.route_metadata.context_window,
            input_modalities: self.route_metadata.input_modalities.clone(),
            speed_tiers: self.route_metadata.speed_tiers.clone(),
            ..crate::agent::club::RouteMetadata::default()
        }
    }

    fn set_reasoning_effort(&self, requested: &str) -> Option<String> {
        let selected = self
            .reasoning_levels
            .iter()
            .find(|level| level.eq_ignore_ascii_case(responses_wire_effort(requested.trim())))?
            .clone();
        *self.reasoning_effort.lock().ok()? = Some(selected.clone());
        self.route_state_revision.fetch_add(1, Ordering::Relaxed);
        Some(selected)
    }

    fn route_state_revision(&self) -> u64 {
        self.route_state_revision.load(Ordering::Relaxed)
    }

    /// OpenAI's prompt cache is automatic for 1024+-token prefixes, and this
    /// club already pins a stable session id per conversation specifically for
    /// cache-affinity routing — the backend is cache-capable by construction.
    fn prompt_cache_capable(&self) -> bool {
        true
    }

    /// Reachable iff a usable ChatGPT token is still on disk (catches `codex
    /// logout`). Cheap local read — no network probe to rate-limit against.
    fn is_available(&self) -> bool {
        ChatGptAuth::load().is_some()
    }

    fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
        self.run(messages, tools, &AtomicBool::new(false), &mut |_| {})
    }

    fn chat_with_effort(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        effort: Option<&str>,
    ) -> Result<ClubReply, String> {
        self.run_with_effort(
            messages,
            tools,
            effort,
            &AtomicBool::new(false),
            &mut |_| {},
        )
    }

    fn chat_streaming(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        self.run(messages, tools, cancel, on_delta)
    }

    fn chat_streaming_with_effort(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        effort: Option<&str>,
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        self.run_with_effort(messages, tools, effort, cancel, on_delta)
    }

    fn usage_report(&self) -> Option<String> {
        let s = self.shared.usage.lock().ok()?;
        format_usage(&s)
    }

    fn cache_usage(&self) -> CacheUsage {
        self.shared.cache_usage_view.load()
    }

    fn usage_accounting(&self) -> crate::agent::club::AccountingView {
        self.shared.accounting.view()
    }

    fn token_usage(&self) -> Option<TokenUsage> {
        let stats = self.shared.usage_view.load();
        (stats.turns > 0).then_some(stats)
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Extract `{input_tokens, output_tokens, output_tokens_details.reasoning_tokens}`
/// from a Responses `usage` object. `None` if the field is absent/misshaped.
fn accounting_observation(usage: Usage) -> crate::agent::club::UsageObservation {
    use crate::agent::club::{
        CacheConvention, ReasoningConvention, UsageContract, UsageObservation,
    };
    UsageObservation {
        raw: [
            usage.input,
            usage.output,
            usage.reasoning,
            usage.cached_input,
            usage.cache_write,
        ],
        paths: [
            Some("response.usage.input_tokens"),
            Some("response.usage.output_tokens"),
            Some("response.usage.output_tokens_details.reasoning_tokens"),
            Some("response.usage.input_tokens_details.cached_tokens"),
            Some("response.usage.input_tokens_details.cache_write_tokens"),
        ],
        contract: UsageContract {
            cache: CacheConvention::Included,
            reasoning: ReasoningConvention::Included,
        },
    }
}

fn parse_usage(usage: Option<&serde_json::Value>) -> Option<Usage> {
    let u = usage?;
    let input = u.get("input_tokens").and_then(serde_json::Value::as_u64);
    let output = u.get("output_tokens").and_then(serde_json::Value::as_u64);
    let reasoning = u
        .pointer("/output_tokens_details/reasoning_tokens")
        .and_then(serde_json::Value::as_u64);
    let cached_input = u
        .pointer("/input_tokens_details/cached_tokens")
        .and_then(serde_json::Value::as_u64);
    let cache_write = u
        .pointer("/input_tokens_details/cache_write_tokens")
        .and_then(serde_json::Value::as_u64);
    if [input, output, reasoning, cached_input, cache_write]
        .iter()
        .all(Option::is_none)
    {
        return None;
    }
    Some(Usage {
        input,
        output,
        reasoning,
        cached_input,
        cache_write,
    })
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Decode a JWT's payload segment (base64url, no pad) to JSON. No signature check
/// — we only read non-secret claims (`exp`, account id) from our own token.
fn jwt_payload(token: &str) -> Option<serde_json::Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn jwt_exp(token: &str) -> Option<i64> {
    jwt_payload(token)?.get("exp")?.as_i64()
}

/// Codex stores the account id inside the id_token under the OpenAI auth claim.
fn account_id_from_jwt(token: &str) -> Option<String> {
    jwt_payload(token)?
        .get("https://api.openai.com/auth")?
        .get("chatgpt_account_id")?
        .as_str()
        .map(str::to_string)
}

/// A uuid-v4-shaped correlation id (no `uuid`/`rand` dep — process + clock based,
/// good enough for the `session_id` header).
fn synth_session_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id() as u128;
    let a = (nanos ^ (pid << 17)) as u64;
    let b = (nanos.rotate_left(40) ^ pid) as u64;
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        a as u32,
        (a >> 32) as u16,
        (a >> 48) as u16 & 0x0fff,
        ((b as u16) & 0x3fff) | 0x8000,
        b >> 16 & 0xffff_ffff_ffff,
    )
}

/// Pull the body out of a ureq error so HTTP failures show the API's message.
fn describe_err(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            describe_response_error(code, &body)
        }
        ureq::Error::Transport(t) => {
            crate::platform::secrets::redact_error(&format!("transport: {t}"))
        }
    }
}

fn describe_response_error(code: u16, body: &str) -> String {
    let detail = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            v.pointer("/error/message")
                .or_else(|| v.get("detail"))
                .and_then(|m| m.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| body.trim().chars().take(200).collect());
    crate::platform::secrets::redact_error(&format!("HTTP {code}: {detail}"))
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/openai_codex__tests.rs"]
mod tests;
