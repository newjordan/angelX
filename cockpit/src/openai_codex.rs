//! OpenAI agent over **ChatGPT OAuth** — no API key. It reuses the tokens the
//! Codex CLI already obtained (`~/.codex/auth.json`), refreshes them when they
//! expire, and talks to the ChatGPT-backed **Responses API**
//! (`https://chatgpt.com/backend-api/codex/responses`) — the same endpoint /
//! headers / client-id Codex uses, extracted from the installed Codex binary.
//!
//! The Responses wire format differs from the OpenAI-compatible chat/completions
//! every other club speaks, so this is its own [`Club`]. Surfaced as the `openai`
//! agent in the bag, present only when a usable token is on disk.

use crate::club::{
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
use crate::club::ToolCall;
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
    crate::club::env_secs(CODEX_STREAM_STALL_ENV, DEFAULT_CODEX_STREAM_STALL_SECS).as_secs()
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
    pub(crate) fn route_metadata(&self) -> crate::club::RouteMetadata {
        crate::club::RouteMetadata {
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
            output_budget: crate::club::OutputBudgetPolicy::EndpointManaged,
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
    selection: Option<crate::club::codex_selection::Selection>,
    route_state_revision: AtomicU64,
    reasoning_levels: Vec<String>,
    route_metadata: crate::club::RouteMetadata,
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
    accounting: crate::club::AccountingCell,
    cache_usage_view: CacheUsageCell,
}

impl CodexSharedState {
    fn new(auth: ChatGptAuth) -> Arc<Self> {
        Arc::new(Self {
            auth: Mutex::new(auth),
            refresh_gate: (Mutex::new(false), Condvar::new()),
            usage: Mutex::new(UsageStats::default()),
            usage_view: UsageCell::default(),
            accounting: crate::club::AccountingCell::default(),
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
            crate::club::RouteMetadata::default(),
        )
    }

    pub(crate) fn new_with_route_metadata_shared(
        name: impl Into<String>,
        model: impl Into<String>,
        shared: Arc<CodexSharedState>,
        reasoning_effort: Option<String>,
        reasoning_levels: Vec<String>,
        mut route_metadata: crate::club::RouteMetadata,
    ) -> Self {
        let reasoning_effort =
            reasoning_effort.map(|effort| responses_wire_effort(&effort).to_string());
        let reasoning_levels = reasoning_levels
            .into_iter()
            .map(|effort| responses_wire_effort(&effort).to_string())
            .collect();
        route_metadata.output_budget = crate::club::OutputBudgetPolicy::EndpointManaged;
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
            .user_agent(concat!("angel0-cockpit/", env!("CARGO_PKG_VERSION")))
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
            #[cfg(test)]
            responses_url_override: None,
        }
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

    pub(crate) fn resolve_selection() -> crate::club::codex_selection::Selection {
        let config = config_from_path(config_path()).unwrap_or_default();
        let env = |key| std::env::var(key).ok().filter(|s| !s.trim().is_empty());
        crate::club::codex_selection::resolve(
            env("ANGEL_OPENAI_MODEL"),
            env("ANGEL_OPENAI_REASONING_EFFORT").or_else(|| env("ANGEL_REASONING_EFFORT")),
            config.model,
            config.model_reasoning_effort,
            &Self::model_catalog(),
        )
    }

    pub(crate) fn with_selection(
        mut self,
        selection: crate::club::codex_selection::Selection,
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
    fn token(&self) -> Result<(String, String), String> {
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
            crate::club::sota_caveman_insert(messages)
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
            if crate::club::final_response_requested(messages) {
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
        let (token, account) = self.token()?;
        let body = self.build_request_with_effort(messages, tools, effort);
        let bytes = serde_json::to_vec(&body).map_err(|e| format!("encode request: {e}"))?;
        let bytes = if self.pxpipe_candidate {
            crate::club::maybe_pxpipe_transform(
                crate::club::PxpipeApi::Responses,
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
            .unwrap_or(RESPONSES_URL);
        #[cfg(not(test))]
        let responses_url = RESPONSES_URL;
        // Observation only: binding the run identity must never change the
        // request path (see HttpClub::send_with_retry). Failures leave the
        // identity unbound and are reported as such by the coverage report.
        if crate::harness::run_identity::current().is_none()
            && let Ok(wire) = serde_json::from_slice::<serde_json::Value>(&bytes)
        {
            let mut defaults = self.resolved_model_defaults();
            defaults["model"] = wire["model"].clone();
            defaults["reasoning_effort"] = wire["reasoning"]["effort"].clone();
            if effort.is_some() {
                defaults["reasoning_effort_source"] = serde_json::json!("env");
            }
            crate::harness::run_identity::prepare_model_defaults(defaults);
            let _ = crate::harness::run_identity::bind(
                crate::harness::run_identity::Model {
                    club: self.name.clone(),
                    id: wire["model"].as_str().unwrap_or(&self.model).to_string(),
                    driver: self.name.clone(),
                    base_url: crate::harness::run_identity::endpoint_identity(responses_url),
                },
                crate::harness::run_identity::wire_effort(&wire),
                wire.get("max_output_tokens")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!("endpoint-managed")),
                Some(self.stream_stall_secs),
            );
        }
        let resp = self
            .agent
            .post(responses_url)
            .set("Content-Type", "application/json")
            .set("Accept", "text/event-stream")
            .set("Authorization", &format!("Bearer {token}"))
            .set("ChatGPT-Account-Id", &account)
            .set("OpenAI-Beta", "responses=experimental")
            .set("originator", ORIGINATOR)
            .set("session_id", &self.session_id)
            .send_bytes(&bytes)
            .map_err(|e| {
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
                format!("openai responses: {detail}")
            })?;

        // Plan rate-limits ride on the response headers; capture them before the
        // body is consumed. Best-effort — absent on endpoints that don't send them.
        let rate = collect_rate_limits(&resp);
        if !rate.is_empty()
            && let Ok(mut s) = self.shared.usage.lock()
        {
            s.rate_limits = rate;
        }

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
            // A blocking read may yield a terminal usage frame just as cancel
            // flips. Account for the received frame before suppressing output.
            let event = attempt.receive(&line, cancelled);
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
                ResponseEvent::Failed(msg, _) => return Err(format!("openai: {msg}")),
                ResponseEvent::Incomplete(msg, _) => {
                    // Cut off at the output cap. A SOTA-tuned link keeps the prose
                    // streamed so far (when no tool call is mid-flight — its args
                    // would be half-written) as usable MoA material, marked so it's
                    // never mistaken for a complete answer. Otherwise fail closed.
                    if self.keep_truncated && !content.trim().is_empty() && !tool_calls.pending() {
                        return Ok(ClubReply::Text(mark_truncated(
                            &content,
                            crate::club::OutputBudgetPolicy::EndpointManaged,
                        )));
                    }
                    return Err(format!("openai: {msg}"));
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
        let calls = tool_calls.into_calls();
        if !calls.is_empty() {
            Ok(ClubReply::Calls(calls))
        } else if tools.is_empty() {
            // Tool-less request (swarm/deli worker): nothing a "recovered" call
            // could execute, so wrapper-looking text stays a prose answer.
            Ok(ClubReply::Text(content))
        } else {
            let recovered = crate::club::extract_prose_tool_calls(&content);
            if recovered.is_empty() {
                Ok(ClubReply::Text(content))
            } else {
                Ok(ClubReply::Calls(recovered))
            }
        }
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
        let mut defaults = crate::club::model_defaults::budgets(&self.model, "ANGEL_OPENAI");
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

    fn route_metadata(&self) -> crate::club::RouteMetadata {
        self.route_metadata.clone()
    }

    fn header_route_metadata(&self) -> crate::club::RouteMetadata {
        crate::club::RouteMetadata {
            context_window: self.route_metadata.context_window,
            input_modalities: self.route_metadata.input_modalities.clone(),
            speed_tiers: self.route_metadata.speed_tiers.clone(),
            ..crate::club::RouteMetadata::default()
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

    fn usage_accounting(&self) -> crate::club::AccountingView {
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
fn accounting_observation(usage: Usage) -> crate::club::UsageObservation {
    use crate::club::{CacheConvention, ReasoningConvention, UsageContract, UsageObservation};
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
        ureq::Error::Transport(t) => crate::secrets::redact_error(&format!("transport: {t}")),
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
    crate::secrets::redact_error(&format!("HTTP {code}: {detail}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Delegates to the crate-wide test env lock (process env is global — a
    /// module-local lock can't serialize against other modules' env tests).
    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::tests::env_lock()
    }

    /// Build a JWT (header.payload.signature) with the given payload JSON. Only the
    /// payload segment is real base64url; header/sig are placeholders.
    fn fake_jwt(payload: serde_json::Value) -> String {
        let p = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&payload).unwrap());
        format!("aaa.{p}.bbb")
    }

    #[test]
    fn jwt_exp_and_account_id_decode() {
        let exp = now_secs() + 3600;
        let tok = fake_jwt(serde_json::json!({
            "exp": exp,
            "https://api.openai.com/auth": { "chatgpt_account_id": "acct_123" }
        }));
        assert_eq!(jwt_exp(&tok), Some(exp));
        assert_eq!(account_id_from_jwt(&tok).as_deref(), Some("acct_123"));
        assert_eq!(jwt_exp("not.a.jwt"), None);
    }

    #[test]
    fn load_parses_chatgpt_auth_and_falls_back_to_id_token_account() {
        // account_id only present inside the id_token claim.
        let id = fake_jwt(serde_json::json!({
            "https://api.openai.com/auth": { "chatgpt_account_id": "acct_from_id" }
        }));
        let v = serde_json::json!({
            "auth_mode": "chatgpt",
            "tokens": { "id_token": id, "access_token": "AT", "refresh_token": "RT", "account_id": "" },
            "last_refresh": "2026-06-17T19:34:00Z",
        });
        let auth = ChatGptAuth::from_value(&v, PathBuf::from("/tmp/x")).expect("loads");
        assert_eq!(auth.access_token, "AT");
        assert_eq!(auth.refresh_token, "RT");
        assert_eq!(auth.account_id, "acct_from_id");
    }

    #[test]
    fn load_rejects_missing_or_empty_token() {
        // No tokens object.
        assert!(ChatGptAuth::from_value(&serde_json::json!({}), PathBuf::from("/x")).is_none());
        // Empty access token (signed out).
        let v = serde_json::json!({ "tokens": { "access_token": "" } });
        assert!(ChatGptAuth::from_value(&v, PathBuf::from("/x")).is_none());
    }

    #[test]
    fn minimal_in_memory_auth_snapshot_loads_without_a_disk_path() {
        let raw = serde_json::json!({
            "tokens": {
                "access_token": "AT",
                "refresh_token": "RT",
                "account_id": "acct"
            }
        })
        .to_string();
        let auth = ChatGptAuth::from_json(&raw, PathBuf::new()).expect("loads");
        assert_eq!(auth.access_token, "AT");
        assert_eq!(auth.refresh_token, "RT");
        assert_eq!(auth.account_id, "acct");
        assert!(auth.path.as_os_str().is_empty());
    }

    #[test]
    fn expired_when_exp_past_or_unreadable() {
        let stale = ChatGptAuth {
            access_token: fake_jwt(serde_json::json!({ "exp": now_secs() - 10 })),
            refresh_token: "RT".into(),
            account_id: "a".into(),
            path: PathBuf::from("/x"),
            disk_snapshot: None,
        };
        assert!(stale.is_expired());
        let fresh = ChatGptAuth {
            access_token: fake_jwt(serde_json::json!({ "exp": now_secs() + 3600 })),
            ..stale.clone()
        };
        assert!(!fresh.is_expired());
        let opaque = ChatGptAuth {
            access_token: "opaque".into(),
            ..stale
        };
        assert!(
            opaque.is_expired(),
            "an unreadable token is treated as expired"
        );
    }

    pub(super) fn club() -> CodexClub {
        let auth = ChatGptAuth {
            access_token: "AT".into(),
            refresh_token: "RT".into(),
            account_id: "acct".into(),
            path: PathBuf::from("/x"),
            disk_snapshot: None,
        };
        CodexClub::new("openai", "test-openai-model", auth)
    }

    #[test]
    fn responses_request_preserves_optional_shell_scope() {
        use crate::harness::Tool;

        let shell = crate::tools::shell::ShellTool::in_dir(PathBuf::from("/workspace"));
        let definition = shell.def();
        let original = definition.params.clone();
        let body = club().build_request(&[ChatMsg::user("inspect")], &[definition]);
        let tool = &body["tools"][0];
        assert_eq!(tool["name"], "shell");
        assert_eq!(tool["strict"], false);
        assert_eq!(tool["parameters"], original);
        assert_eq!(
            tool["parameters"]["required"],
            serde_json::json!(["command"])
        );
        assert_eq!(
            tool["parameters"]["properties"]["write_paths"]["type"],
            "array"
        );
        assert!(
            tool["parameters"]["properties"]["write_paths"]
                .get("default")
                .is_none()
        );
    }

    fn reasoning_club() -> CodexClub {
        let auth = ChatGptAuth {
            access_token: "AT".into(),
            refresh_token: "RT".into(),
            account_id: "acct".into(),
            path: PathBuf::from("/x"),
            disk_snapshot: None,
        };
        CodexClub::new_with_reasoning(
            "openai",
            "gpt-test",
            auth,
            Some("high".into()),
            vec!["low".into(), "medium".into(), "high".into()],
        )
    }

    #[test]
    fn openai_codex_resolved_selection_reaches_responses_fixture() {
        let _guard = env_lock();
        use crate::tests::TestEnvGuard;
        let dir = std::env::temp_dir().join(format!("codex-selection-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("config.toml"),
            "model='gpt-6-astra'\nmodel_reasoning_effort='medium'\n",
        )
        .unwrap();
        std::fs::write(dir.join("models_cache.json"), serde_json::json!({"models":[
            {"slug":"gpt-5.6-luna","display_name":"Luna","supported_reasoning_levels":[{"effort":"max"}]},
            {"slug":"gpt-6-astra","display_name":"Astra","supported_reasoning_levels":[{"effort":"medium"}]}
        ]}).to_string()).unwrap();
        let _codex_home = TestEnvGuard::set("CODEX_HOME", dir.to_str().unwrap());
        let _driver = TestEnvGuard::unset("ANGEL_DRIVER");
        let _global = TestEnvGuard::unset("ANGEL_REASONING_EFFORT");
        for (pins, model, effort, source) in [
            (true, "gpt-5.6-luna", "max", "env"),
            (false, "gpt-6-astra", "medium", "codex-config"),
        ] {
            let _model = if pins {
                TestEnvGuard::set("ANGEL_OPENAI_MODEL", model)
            } else {
                TestEnvGuard::unset("ANGEL_OPENAI_MODEL")
            };
            let _effort = if pins {
                TestEnvGuard::set("ANGEL_OPENAI_REASONING_EFFORT", effort)
            } else {
                TestEnvGuard::unset("ANGEL_OPENAI_REASONING_EFFORT")
            };
            let selection = CodexClub::resolve_selection();
            assert!(selection.error.is_none(), "{:?}", selection.error);
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut sock, _) = listener.accept().unwrap();
                let request = read_http_request(&mut sock);
                let body: serde_json::Value =
                    serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
                sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\n\ndata: {\"type\":\"response.completed\",\"response\":{}}\n\n").unwrap();
                body
            });
            let auth = ChatGptAuth {
                access_token: fake_jwt(serde_json::json!({"exp":now_secs()+3600})),
                refresh_token: "fixture".into(),
                account_id: "fixture".into(),
                path: dir.join("unused-auth.json"),
                disk_snapshot: None,
            };
            let mut club = CodexClub::new_with_reasoning(
                "openai",
                model,
                auth,
                Some(effort.into()),
                vec![effort.into()],
            )
            .with_selection(selection);
            club.responses_url_override = Some(format!("http://{addr}"));
            let defaults = club.resolved_model_defaults();
            assert_eq!(defaults["model_source"], source);
            assert_eq!(defaults["reasoning_effort_source"], source);
            assert_eq!(club.respond("fixture").unwrap(), "ok");
            let wire = server.join().unwrap();
            assert_eq!(wire["model"], model);
            assert_eq!(wire["model"], defaults["model"]);
            assert_eq!(wire["reasoning"]["effort"], effort);
            assert_eq!(wire["reasoning"]["effort"], defaults["reasoning_effort"]);
            assert_eq!(wire["reasoning"]["summary"], "auto");
        }
        let _model = TestEnvGuard::set("ANGEL_OPENAI_MODEL", "gpt-5.6-luna");
        let _effort = TestEnvGuard::set("ANGEL_OPENAI_REASONING_EFFORT", "invented");
        // Invalid selection must return before auth refresh or any POST.
        let club = club().with_selection(CodexClub::resolve_selection());
        let error = club.respond("fixture").unwrap_err();
        assert!(error.contains("supported efforts: [max]"), "{error}");
        assert_eq!(
            club.resolved_model_defaults()["reasoning_effort"],
            "invented"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn codex_config_reads_model_and_reasoning_effort() {
        let config = config_from_str(
            r#"
model = "gpt-5.6-sol"
model_reasoning_effort = "ultra"
"#,
        )
        .unwrap();
        assert_eq!(config.model.as_deref(), Some("gpt-5.6-sol"));
        assert_eq!(config.model_reasoning_effort.as_deref(), Some("ultra"));
    }

    #[test]
    fn model_catalog_is_visible_sorted_and_carries_exact_efforts() {
        let models = model_catalog_from_str(
            r#"{
  "models": [
    {"slug":"hidden","display_name":"Hidden","visibility":"hide","priority":0},
    {"slug":"terra","display_name":"Terra","visibility":"list","priority":2,
     "default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"low","description":"fast"},{"effort":"medium","description":"balanced"}]},
    {"slug":"sol","display_name":"Sol","visibility":"list","priority":1,
     "description":"  Frontier\n agentic coding model.  ","context_window":372000,
     "input_modalities":["text","image"],"additional_speed_tiers":["fast"],
     "default_reasoning_level":"ultra","supported_reasoning_levels":[{"effort":"high","description":"deep"},{"effort":"ultra","description":"deepest"}]}
  ]
}"#,
        );
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].slug, "sol");
        assert_eq!(models[0].default_reasoning_level, "ultra");
        assert_eq!(models[0].supported_reasoning_levels[1].effort, "ultra");
        let metadata = models[0].route_metadata();
        assert_eq!(
            metadata.description.as_deref(),
            Some("Frontier agentic coding model.")
        );
        assert_eq!(metadata.context_window, Some(372_000));
        assert_eq!(metadata.input_modalities, ["text", "image"]);
        assert_eq!(metadata.speed_tiers, ["fast"]);
        assert_eq!(metadata.reasoning_description("ULTRA"), Some("deepest"));
        assert_eq!(models[1].slug, "terra");
    }

    #[test]
    fn reasoning_effort_is_backend_owned_and_selects_only_supported_levels() {
        let club = reasoning_club();
        let body = club.build_request(&[ChatMsg::user("hi")], &[]);
        assert_eq!(body["reasoning"]["effort"], "high");
        assert_eq!(
            body["reasoning"]["summary"], "auto",
            "hosted models expose only summaries — dropping them blanks the thinking panel"
        );
        assert_eq!(club.reasoning_effort().as_deref(), Some("high"));
        assert_eq!(club.set_reasoning_effort("low").as_deref(), Some("low"));
        assert_eq!(club.set_reasoning_effort("invented"), None);
        let body = club.build_request(&[ChatMsg::user("again")], &[]);
        assert_eq!(body["reasoning"]["effort"], "low");
    }

    #[test]
    fn per_call_reasoning_effort_does_not_mutate_codex_route_state() {
        let club = reasoning_club();
        let body = club.build_request_with_effort(&[ChatMsg::user("seat")], &[], Some("medium"));
        assert_eq!(body["reasoning"]["effort"], "medium");
        assert_eq!(club.reasoning_effort().as_deref(), Some("high"));

        let body = club.build_request(&[ChatMsg::user("ordinary turn")], &[]);
        assert_eq!(body["reasoning"]["effort"], "high");
    }

    #[test]
    fn legacy_ultra_effort_uses_the_responses_api_xhigh_spelling() {
        let auth = ChatGptAuth {
            access_token: "AT".into(),
            refresh_token: "RT".into(),
            account_id: "acct".into(),
            path: PathBuf::from("/x"),
            disk_snapshot: None,
        };
        let club = CodexClub::new_with_reasoning(
            "openai",
            "gpt-test",
            auth,
            Some("ultra".into()),
            vec!["high".into(), "ultra".into()],
        );
        let body = club.build_request(&[ChatMsg::user("hi")], &[]);
        assert_eq!(body["reasoning"]["effort"], "xhigh");
        assert_eq!(club.reasoning_effort().as_deref(), Some("xhigh"));
        assert_eq!(
            club.resolved_model_defaults()["reasoning_effort"],
            body["reasoning"]["effort"]
        );
    }

    #[test]
    fn selectable_models_share_rotating_auth_and_session_usage() {
        let auth = ChatGptAuth {
            access_token: "AT".into(),
            refresh_token: "RT".into(),
            account_id: "acct".into(),
            path: PathBuf::from("/x"),
            disk_snapshot: None,
        };
        let shared = CodexClub::shared_state(auth);
        let sol = CodexClub::new_with_reasoning_shared(
            "openai",
            "sol",
            Arc::clone(&shared),
            Some("low".into()),
            vec!["low".into(), "high".into()],
        );
        let terra = CodexClub::new_with_reasoning_shared(
            "openai",
            "terra",
            Arc::clone(&shared),
            Some("medium".into()),
            vec!["medium".into(), "high".into()],
        );
        assert!(Arc::ptr_eq(&sol.shared, &terra.shared));
        sol.record_usage(Usage {
            input: Some(11),
            output: Some(3),
            reasoning: Some(2),
            ..Usage::default()
        });
        assert_eq!(terra.token_usage().map(|usage| usage.turns), Some(1));
        sol.shared.auth.lock().unwrap().refresh_token = "ROTATED".into();
        assert_eq!(
            terra.shared.auth.lock().unwrap().refresh_token,
            "ROTATED",
            "every model must see the latest rotating refresh token"
        );
    }

    #[test]
    fn token_adopts_credentials_rotated_by_another_process() {
        static NEXT_PATH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let nonce = NEXT_PATH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "angel_codex_auth_adopt_{}_{}",
            std::process::id(),
            nonce
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("auth.json");
        let fresh_access = fake_jwt(serde_json::json!({ "exp": now_secs() + 3600 }));
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "auth_mode": "chatgpt",
                "tokens": {
                    "access_token": fresh_access,
                    "refresh_token": "disk-rotated-refresh",
                    "account_id": "disk-account"
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let stale = ChatGptAuth {
            access_token: fake_jwt(serde_json::json!({ "exp": now_secs() - 10 })),
            refresh_token: "stale-refresh".into(),
            account_id: "stale-account".into(),
            path: path.clone(),
            disk_snapshot: None,
        };
        let club = CodexClub::new("openai", "test-openai-model", stale);
        let (access, account) = club.token().expect("fresh disk token avoids refresh");
        assert_eq!(access, fresh_access);
        assert_eq!(account, "disk-account");
        assert_eq!(
            club.shared.auth.lock().unwrap().refresh_token,
            "disk-rotated-refresh"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn concurrent_token_reads_on_a_valid_token_share_the_fast_path() {
        // A fresh disk token: every concurrent caller returns it via the fast
        // path (cheap lock, no refresh gate) with no deadlock or lock inversion —
        // guards the single-flight refactor against a concurrency regression.
        let dir = std::env::temp_dir().join(format!(
            "angel_codex_auth_concurrent_{}_{}",
            std::process::id(),
            now_secs()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("auth.json");
        let fresh_access = fake_jwt(serde_json::json!({ "exp": now_secs() + 3600 }));
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "auth_mode": "chatgpt",
                "tokens": {
                    "access_token": fresh_access,
                    "refresh_token": "r",
                    "account_id": "acct"
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let auth = ChatGptAuth::from_path(path.clone()).unwrap();
        let club = Arc::new(CodexClub::new("openai", "test-openai-model", auth));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let c = Arc::clone(&club);
                std::thread::spawn(move || c.token())
            })
            .collect();
        for h in handles {
            let (access, account) = h.join().unwrap().expect("valid token returns");
            assert_eq!(access, fresh_access);
            assert_eq!(account, "acct");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unchanged_stale_disk_does_not_undo_an_in_memory_refresh() {
        let dir = std::env::temp_dir().join(format!(
            "angel_codex_auth_persist_{}_{}",
            std::process::id(),
            now_secs()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("auth.json");
        let write_auth = |access: &str, refresh: &str| {
            std::fs::write(
                &path,
                serde_json::to_vec(&serde_json::json!({
                    "auth_mode": "chatgpt",
                    "tokens": {
                        "access_token": access,
                        "refresh_token": refresh,
                        "account_id": "account"
                    }
                }))
                .unwrap(),
            )
            .unwrap();
        };
        write_auth("disk-old-access", "disk-old-refresh");
        let mut auth = ChatGptAuth::from_path(path.clone()).unwrap();

        // Model a successful refresh whose best-effort disk write failed.
        auth.access_token = "memory-new-access".into();
        auth.refresh_token = "memory-new-refresh".into();
        assert!(!auth.adopt_disk_credentials());
        assert_eq!(auth.access_token, "memory-new-access");
        assert_eq!(auth.refresh_token, "memory-new-refresh");

        // A genuinely different later disk snapshot is still adopted.
        write_auth("external-new-access", "external-new-refresh");
        assert!(auth.adopt_disk_credentials());
        assert_eq!(auth.access_token, "external-new-access");
        assert_eq!(auth.refresh_token, "external-new-refresh");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn refresh_conflict_guard_preserves_a_later_disk_rotation() {
        let dir = std::env::temp_dir().join(format!(
            "angel_codex_auth_race_{}_{}",
            std::process::id(),
            now_secs()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("auth.json");
        let source = ChatGptAuth {
            access_token: "source-access".into(),
            refresh_token: "source-refresh".into(),
            account_id: "source-account".into(),
            path: path.clone(),
            disk_snapshot: None,
        };
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "auth_mode": "chatgpt",
                "tokens": {
                    "access_token": "later-access",
                    "refresh_token": "later-refresh",
                    "account_id": "later-account"
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let mut refresh_result = source.clone();
        refresh_result.access_token = "network-response-access".into();
        refresh_result.refresh_token = "network-response-refresh".into();
        assert!(refresh_result.adopt_disk_credentials_changed_since(&source));
        assert_eq!(refresh_result.access_token, "later-access");
        assert_eq!(refresh_result.refresh_token, "later-refresh");
        assert_eq!(refresh_result.account_id, "later-account");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn persist_conflict_guard_never_overwrites_later_disk_credentials() {
        let dir = std::env::temp_dir().join(format!(
            "angel_codex_auth_persist_race_{}_{}",
            std::process::id(),
            now_secs()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("auth.json");
        let source = ChatGptAuth {
            access_token: "source-access".into(),
            refresh_token: "source-refresh".into(),
            account_id: "source-account".into(),
            path: path.clone(),
            disk_snapshot: None,
        };
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "auth_mode": "chatgpt",
                "tokens": {
                    "access_token": "later-access",
                    "refresh_token": "later-refresh",
                    "account_id": "later-account"
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let mut refresh_result = source.clone();
        refresh_result.access_token = "network-response-access".into();
        refresh_result.refresh_token = "network-response-refresh".into();
        refresh_result.persist(None, &source);
        assert_eq!(refresh_result.access_token, "later-access");
        assert_eq!(refresh_result.refresh_token, "later-refresh");
        let disk = ChatGptAuth::from_path(path.clone()).unwrap();
        assert_eq!(disk.access_token, "later-access");
        assert_eq!(disk.refresh_token, "later-refresh");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn auth_persist_preserves_permissions_and_leaves_no_shared_temp_file() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!(
            "angel_codex_auth_mode_{}_{}",
            std::process::id(),
            now_secs()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("auth.json");
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "auth_mode": "chatgpt",
                "tokens": {
                    "access_token": "source-access",
                    "refresh_token": "source-refresh",
                    "account_id": "source-account"
                }
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
        let source = ChatGptAuth::from_path(path.clone()).unwrap();
        let mut refreshed = source.clone();
        refreshed.access_token = "refreshed-access".into();
        refreshed.refresh_token = "refreshed-refresh".into();

        refreshed.persist(None, &source);

        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        let disk = ChatGptAuth::from_path(path.clone()).unwrap();
        assert_eq!(disk.access_token, "refreshed-access");
        assert_eq!(disk.refresh_token, "refreshed-refresh");
        let entries = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(entries, [std::ffi::OsString::from("auth.json")]);

        let _ = std::fs::remove_dir_all(dir);
    }

    /// The ChatGPT-backed Codex Responses endpoint rejects `max_output_tokens`
    /// ("HTTP 400: Unsupported parameter"), so the request must never carry it —
    /// tuned or not. A SOTA link relies on `keep_truncated` for a cut-off reply.
    #[test]
    fn build_request_never_sends_max_output_tokens() {
        for club in [club(), club().sota_tuned()] {
            let body = club.build_request(&[ChatMsg::user("hi")], &[]);
            assert!(
                body.get("max_output_tokens").is_none(),
                "Codex Responses body must not carry max_output_tokens: {body}"
            );
            assert_eq!(
                club.route_metadata().output_budget,
                crate::club::OutputBudgetPolicy::EndpointManaged
            );
        }
    }

    /// Read one full HTTP request (headers + `Content-Length` body) off a
    /// socket, mirroring the local-server helpers the chat-route tests use.
    fn read_http_request(sock: &mut std::net::TcpStream) -> String {
        use std::io::Read;
        let mut req = Vec::new();
        let mut buf = [0u8; 1024];
        while let Ok(n) = sock.read(&mut buf) {
            if n == 0 {
                break;
            }
            req.extend_from_slice(&buf[..n]);
            if let Some(pos) = req.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&req[..pos]).to_ascii_lowercase();
                let need = headers
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if req.len() >= pos + 4 + need {
                    break;
                }
            }
        }
        String::from_utf8_lossy(&req).into_owned()
    }

    #[test]
    fn codex_stream_stall_knob_defaults_disables_and_falls_back() {
        let _guard = env_lock();
        {
            let _unset = crate::tests::TestEnvGuard::unset(CODEX_STREAM_STALL_ENV);
            assert_eq!(codex_stream_stall_secs(), 120);
        }
        {
            let _off = crate::tests::TestEnvGuard::set(CODEX_STREAM_STALL_ENV, "0");
            assert_eq!(codex_stream_stall_secs(), 0);
        }
        {
            let _invalid = crate::tests::TestEnvGuard::set(CODEX_STREAM_STALL_ENV, "not-secs");
            assert_eq!(codex_stream_stall_secs(), 120);
        }
    }

    /// A Responses stream that answers HTTP 200, emits one `response.created`
    /// event, and then goes silent must fail at the stall bound instead of
    /// holding the turn until the plain 300 s socket timeout (observed live:
    /// `openai/gpt-6-astra@high` silent for 8+ minutes while steers queued).
    #[test]
    fn codex_stream_stall_bound_fires_on_a_silent_responses_stream() {
        let _guard = env_lock();
        {
            // Must be set before construction: the knob is resolved where the
            // ureq agent (and its read deadline) is built.
            let _stall = crate::tests::TestEnvGuard::set(CODEX_STREAM_STALL_ENV, "1");
            use std::io::Write;
            use std::net::TcpListener;
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let handle = std::thread::spawn(move || {
                let Ok((mut sock, _)) = listener.accept() else {
                    return;
                };
                let _ = read_http_request(&mut sock);
                let _ = sock.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                );
                // One real event, then silence: the 1 s stall bound must fire
                // long before the plain 300 s read timeout.
                let _ = sock.write_all(
                    b"event: response.created\n\
                       data: {\"type\":\"response.created\",\"response\":{}}\n\n",
                );
                let _ = sock.flush();
                std::thread::sleep(std::time::Duration::from_secs(3));
            });
            let auth = ChatGptAuth {
                access_token: fake_jwt(serde_json::json!({ "exp": now_secs() + 3600 })),
                refresh_token: "RT".into(),
                account_id: "acct".into(),
                path: PathBuf::from("/x"),
                disk_snapshot: None,
            };
            let mut club = CodexClub::new("openai", "test-openai-model", auth);
            club.responses_url_override = Some(format!("http://{addr}"));
            let started = std::time::Instant::now();
            let err = club
                .chat_streaming(
                    &[ChatMsg::user("hi")],
                    &[],
                    &AtomicBool::new(false),
                    &mut |_| {},
                )
                .expect_err("a silent Responses stream must fail, not hang");
            assert!(err.contains("stream stalled"), "{err}");
            assert!(err.contains(CODEX_STREAM_STALL_ENV), "{err}");
            assert!(err.contains("test-openai-model"), "{err}");
            assert!(
                started.elapsed() < std::time::Duration::from_secs(3),
                "the stall bound must fire well before the 300 s plain timeout ({:?})",
                started.elapsed()
            );
            let stats = club.shared.usage.lock().unwrap();
            assert_eq!(
                stats.recent_attempts.back().expect("one receipt").outcome,
                "stalled"
            );
            drop(handle.join());
        }
    }

    /// A stream that keeps sending keep-alive comments and bookkeeping frames
    /// but never produces output must also stall out: bytes are not progress.
    #[test]
    fn codex_stream_stall_bound_fires_on_a_heartbeat_only_responses_stream() {
        let _guard = env_lock();
        {
            let _stall = crate::tests::TestEnvGuard::set(CODEX_STREAM_STALL_ENV, "1");
            use std::io::Write;
            use std::net::TcpListener;
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let handle = std::thread::spawn(move || {
                let Ok((mut sock, _)) = listener.accept() else {
                    return;
                };
                let _ = read_http_request(&mut sock);
                let _ = sock.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                );
                let _ =
                    sock.write_all(b"data: {\"type\":\"response.created\",\"response\":{}}\n\n");
                let _ = sock.flush();
                // Keep-alives every 200 ms for 4 s: the socket never times out,
                // yet nothing meaningful arrives.
                for _ in 0..20 {
                    if sock
                        .write_all(
                            b": ping\n\ndata: {\"type\":\"response.in_progress\",\"response\":{}}\n\n",
                        )
                        .is_err()
                    {
                        break;
                    }
                    let _ = sock.flush();
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            });
            let auth = ChatGptAuth {
                access_token: fake_jwt(serde_json::json!({ "exp": now_secs() + 3600 })),
                refresh_token: "RT".into(),
                account_id: "acct".into(),
                path: PathBuf::from("/x"),
                disk_snapshot: None,
            };
            let mut club = CodexClub::new("openai", "test-openai-model", auth);
            club.responses_url_override = Some(format!("http://{addr}"));
            let started = std::time::Instant::now();
            let err = club
                .chat_streaming(
                    &[ChatMsg::user("hi")],
                    &[],
                    &AtomicBool::new(false),
                    &mut |_| {},
                )
                .expect_err("a heartbeat-only Responses stream must fail, not hang");
            assert!(err.contains("stream stalled"), "{err}");
            assert!(err.contains("keep-alives"), "{err}");
            assert!(err.contains(CODEX_STREAM_STALL_ENV), "{err}");
            assert!(
                started.elapsed() < std::time::Duration::from_secs(3),
                "heartbeats must not defeat the stall bound ({:?})",
                started.elapsed()
            );
            let stats = club.shared.usage.lock().unwrap();
            assert_eq!(
                stats.recent_attempts.back().expect("one receipt").outcome,
                "stalled"
            );
            drop(handle.join());
        }
    }

    #[test]
    fn build_request_maps_system_to_instructions_and_roles() {
        let msgs = vec![
            ChatMsg::system("be terse"),
            ChatMsg::user("hi"),
            ChatMsg::assistant("hello"),
            ChatMsg::user("more"),
        ];
        let body = club().build_request(&msgs, &[]);
        assert_eq!(body["model"], "test-openai-model");
        assert_eq!(body["instructions"], "be terse");
        assert_eq!(body["stream"], true);
        assert_eq!(body["store"], false);
        let input = body["input"].as_array().unwrap();
        assert_eq!(input.len(), 3, "system is lifted out of input");
        assert_eq!(input[0]["role"], "user");
        assert_eq!(input[0]["content"][0]["type"], "input_text");
        assert_eq!(input[0]["content"][0]["text"], "hi");
        assert_eq!(input[1]["role"], "assistant");
        assert_eq!(input[1]["content"][0]["type"], "output_text");
    }

    #[test]
    fn sota_tuned_build_request_keeps_caveman_opt_in() {
        let _guard = env_lock();
        let saved = std::env::var_os("ANGEL_SOTA_CAVEMAN");
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_SOTA_CAVEMAN") };

        let body = club()
            .sota_tuned()
            .build_request(&[ChatMsg::user("hi")], &[]);
        assert_eq!(body["instructions"], "");

        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_SOTA_CAVEMAN", "1") };
        let body = club()
            .sota_tuned()
            .build_request(&[ChatMsg::user("hi")], &[]);
        assert!(
            body["instructions"]
                .as_str()
                .unwrap_or_default()
                .contains("angel0 SOTA brevity mode"),
            "{body}"
        );
        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["input"][0]["content"][0]["text"], "hi");

        match saved {
            // TODO: Audit that the environment access only happens in single-threaded code.
            Some(v) => unsafe { std::env::set_var("ANGEL_SOTA_CAVEMAN", v) },
            // TODO: Audit that the environment access only happens in single-threaded code.
            None => unsafe { std::env::remove_var("ANGEL_SOTA_CAVEMAN") },
        }
    }

    #[test]
    fn build_request_preserves_image_attachments_for_responses_api() {
        let msg = ChatMsg::user_with_media(
            "what is this",
            vec![Media::Image {
                mime: "image/png".into(),
                b64: "AAAA".into(),
            }],
        );
        let body = club().build_request(&[msg], &[]);
        let content = body["input"][0]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "input_text");
        assert_eq!(content[0]["text"], "what is this");
        assert_eq!(content[1]["type"], "input_image");
        assert_eq!(content[1]["image_url"], "data:image/png;base64,AAAA");
    }

    #[test]
    fn final_mile_codex_retains_schemas_and_disables_calls() {
        let _guard = crate::tests::env_lock();
        let tools = [ToolDef {
            name: "read_file".into(),
            description: "Read".into(),
            params: serde_json::json!({"type":"object", "properties":{}}),
        }];
        let mut messages = vec![ChatMsg::system("stable"), ChatMsg::user("work")];
        let before = club().build_request(&messages, &tools);
        messages.push(ChatMsg::harness(crate::club::FINAL_MILE_ANSWER_NUDGE));
        let after = club().build_request(&messages, &tools);
        assert_eq!(before["tools"], after["tools"]);
        assert_eq!(before["instructions"], after["instructions"]);
        assert_eq!(after["tool_choice"], "none");
        let old = before["input"].as_array().unwrap();
        assert_eq!(
            old.as_slice(),
            &after["input"].as_array().unwrap()[..old.len()]
        );
        messages.push(ChatMsg::user("continue"));
        assert!(
            club()
                .build_request(&messages, &tools)
                .get("tool_choice")
                .is_none()
        );
    }

    #[test]
    fn build_request_advertises_tools_and_preserves_tool_history() {
        let tools = [ToolDef {
            name: "list_dir".into(),
            description: "List files".into(),
            params: serde_json::json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }),
        }];
        let msgs = vec![
            ChatMsg::user("inspect"),
            ChatMsg::assistant_calls(vec![ToolCall {
                id: "call_1".into(),
                name: "list_dir".into(),
                args: serde_json::json!({ "path": "." }),
            }]),
            ChatMsg::tool("call_1", "README.md\nsrc"),
        ];
        let body = club().build_request(&msgs, &tools);
        assert_eq!(body["tools"][0]["type"], "function");
        assert_eq!(body["tools"][0]["name"], "list_dir");
        assert_eq!(body["tools"][0]["parameters"]["required"][0], "path");
        let input = body["input"].as_array().unwrap();
        assert_eq!(input[1]["type"], "function_call");
        assert_eq!(input[1]["call_id"], "call_1");
        assert_eq!(input[1]["name"], "list_dir");
        assert_eq!(input[1]["arguments"], r#"{"path":"."}"#);
        assert_eq!(input[2]["type"], "function_call_output");
        assert_eq!(input[2]["call_id"], "call_1");
        assert_eq!(input[2]["output"], "README.md\nsrc");
    }

    #[test]
    fn sse_events_decode_text_reasoning_done_and_error() {
        let text =
            parse_responses_event(r#"data: {"type":"response.output_text.delta","delta":"Hel"}"#);
        assert!(matches!(text, ResponseEvent::Text(t) if t == "Hel"));
        let summary = parse_responses_event(
            r#"data: {"type":"response.reasoning_summary_text.delta","delta":"think"}"#,
        );
        assert!(matches!(summary, ResponseEvent::ReasoningSummary(s) if s == "think"));
        let reason = parse_responses_event(
            r#"data: {"type":"response.reasoning_text.delta","delta":"raw"}"#,
        );
        assert!(matches!(reason, ResponseEvent::Reasoning(r) if r == "raw"));
        let start = parse_responses_event(
            r#"data: {"type":"response.output_item.added","item":{"type":"function_call","id":"fc_1","call_id":"call_1","name":"list_dir"}}"#,
        );
        assert!(matches!(
            start,
            ResponseEvent::ToolCallStart { key, call_id, name }
                if key == "fc_1" && call_id.as_deref() == Some("call_1") && name.as_deref() == Some("list_dir")
        ));
        let delta = parse_responses_event(
            r#"data: {"type":"response.function_call_arguments.delta","item_id":"fc_1","delta":"{\"path\""}"#,
        );
        assert!(matches!(
            delta,
            ResponseEvent::ToolArgumentsDelta { key, delta }
                if key == "fc_1" && delta == "{\"path\""
        ));
        let done = parse_responses_event(
            r#"data: {"type":"response.output_item.done","item":{"type":"function_call","id":"fc_1","call_id":"call_1","name":"list_dir","arguments":"{\"path\":\".\"}"}}"#,
        );
        assert!(matches!(
            done,
            ResponseEvent::ToolCallDone { key, call }
                if key == "fc_1" && call.id == "call_1" && call.name == "list_dir" && call.args["path"] == "."
        ));
        // A bare complete carries no usage; one with `usage` is parsed through.
        assert!(matches!(
            parse_responses_event(r#"data: {"type":"response.completed","response":{}}"#),
            ResponseEvent::Done(None)
        ));
        let done_usage = parse_responses_event(
            r#"data: {"type":"response.completed","response":{"usage":{"input_tokens":12,"output_tokens":34,"output_tokens_details":{"reasoning_tokens":8}}}}"#,
        );
        assert!(matches!(
            done_usage,
            ResponseEvent::Done(Some(u)) if u.input == Some(12) && u.output == Some(34) && u.reasoning == Some(8)
        ));
        let incomplete = parse_responses_event(
            r#"data: {"type":"response.completed","response":{"status":"incomplete","incomplete_details":{"reason":"max_output_tokens"}}}"#,
        );
        assert!(matches!(
            incomplete,
            ResponseEvent::Incomplete(m, _)
                if m == "response incomplete: endpoint reported max_output_tokens (plan-managed)"
        ));
        let fail = parse_responses_event(
            r#"data: {"type":"response.failed","response":{"error":{"message":"nope"}}}"#,
        );
        assert!(matches!(fail, ResponseEvent::Failed(m, _) if m == "nope"));
        // event: lines, comments, blanks, and unrelated events are ignored.
        assert!(matches!(
            parse_responses_event("event: response.output_text.delta"),
            ResponseEvent::Ignore
        ));
        assert!(matches!(
            parse_responses_event(": keep-alive"),
            ResponseEvent::Ignore
        ));
        assert!(matches!(
            parse_responses_event(r#"data: {"type":"response.created"}"#),
            ResponseEvent::Ignore
        ));
    }

    #[test]
    fn response_tool_call_accumulator_builds_calls_from_deltas() {
        let mut calls = ResponseToolCalls::default();
        calls.start(
            "fc_1".into(),
            Some("call_1".into()),
            Some("list_dir".into()),
        );
        calls.push_args("fc_1", r#"{"path""#);
        calls.push_args("fc_1", r#":"."}"#);
        let out = calls.into_calls();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "call_1");
        assert_eq!(out[0].name, "list_dir");
        assert_eq!(out[0].args["path"], ".");
    }

    #[test]
    fn response_tool_call_accumulator_merges_item_and_call_ids() {
        // Responses streams identify the output item as `fc_*`, while the
        // dispatch/result protocol uses `call_*`. A completed item must finish
        // the started call, not create a second callable entry.
        let mut calls = ResponseToolCalls::default();
        calls.start(
            "fc_1".into(),
            Some("call_1".into()),
            Some("list_dir".into()),
        );
        // Some gateways key the final argument event by call_id instead of
        // item_id; this must still join the original entry.
        calls.set_args("call_1", r#"{"path":"."}"#.into());
        calls.done(
            "fc_1".into(),
            ToolCall {
                id: "call_1".into(),
                name: "list_dir".into(),
                args: serde_json::json!({"path":"."}),
            },
        );

        let out = calls.into_calls();
        assert_eq!(out.len(), 1, "one logical stream item must dispatch once");
        assert_eq!(out[0].id, "call_1");
        assert_eq!(out[0].name, "list_dir");
        assert_eq!(out[0].args["path"], ".");
    }

    #[test]
    fn response_tool_call_accumulator_keeps_multi_call_item_order() {
        let mut calls = ResponseToolCalls::default();
        calls.start(
            "fc_first".into(),
            Some("call_first".into()),
            Some("read_file".into()),
        );
        calls.start(
            "fc_second".into(),
            Some("call_second".into()),
            Some("list_dir".into()),
        );
        // Completion order is allowed to differ from output-item order.
        calls.done(
            "fc_second".into(),
            ToolCall {
                id: "call_second".into(),
                name: "list_dir".into(),
                args: serde_json::json!({"path":"src"}),
            },
        );
        calls.done(
            "fc_first".into(),
            ToolCall {
                id: "call_first".into(),
                name: "read_file".into(),
                args: serde_json::json!({"path":"Cargo.toml"}),
            },
        );

        let out = calls.into_calls();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].id, "call_first");
        assert_eq!(out[1].id, "call_second");
    }

    #[test]
    fn format_usage_renders_tokens_and_limits() {
        // Nothing reported yet → no status noise.
        assert!(format_usage(&UsageStats::default()).is_none());

        let stats = UsageStats {
            turns: 2,
            last: Usage {
                input: Some(100),
                output: Some(50),
                reasoning: Some(20),
                ..Usage::default()
            },
            total_input: 300,
            total_output: 180,
            total_reasoning: 40,
            rate_limits: vec![
                ("x-codex-plan-type".into(), "pro".into()),
                ("x-codex-active-limit".into(), "premium".into()),
                ("x-codex-primary-used-percent".into(), "37".into()),
                ("x-codex-primary-reset-after-seconds".into(), "1185".into()),
                ("x-codex-primary-window-minutes".into(), "300".into()),
            ],
            ..UsageStats::default()
        };
        let out = format_usage(&stats).expect("usage present");
        assert!(out.contains("turns     2"));
        assert!(out.contains("last      in 100 · out 50 · reasoning 20"));
        assert!(out.contains("total 480"));
        assert!(out.contains("mix       in [########....]  63% · out [#####.......]  38%"));
        assert!(out.contains("plan"));
        assert!(out.contains("type      pro · active premium"));
        assert!(out.contains("primary   [####........]  37% · reset 19m · window 5h"));
    }

    #[test]
    fn parse_usage_handles_absent_partial_and_all_zero() {
        // Absent usage object → None.
        assert!(parse_usage(None).is_none());
        // Explicit zero is reported usage, distinct from absent/unknown usage.
        let zero = serde_json::json!({ "input_tokens": 0, "output_tokens": 0 });
        assert_eq!(parse_usage(Some(&zero)).unwrap().input, Some(0));
        // Only reasoning tokens present → still a Usage.
        let only_reasoning = serde_json::json!({
            "output_tokens_details": { "reasoning_tokens": 7 }
        });
        let u = parse_usage(Some(&only_reasoning)).expect("reasoning-only counts");
        assert_eq!((u.input, u.output, u.reasoning), (None, None, Some(7)));
        // Input only, with a misshaped reasoning detail (ignored).
        let input_only = serde_json::json!({ "input_tokens": 5, "output_tokens_details": 9 });
        let u2 = parse_usage(Some(&input_only)).expect("input counts");
        assert_eq!((u2.input, u2.output, u2.reasoning), (Some(5), None, None));
    }

    #[test]
    fn record_usage_accumulates_and_usage_report_renders() {
        let c = club();
        // No turns yet → no report.
        assert!(c.usage_report().is_none());
        assert!(c.token_usage().is_none());
        c.record_usage(Usage {
            input: Some(10),
            output: Some(4),
            reasoning: Some(0),
            ..Usage::default()
        });
        c.record_usage(Usage {
            input: Some(20),
            output: Some(6),
            reasoning: Some(3),
            ..Usage::default()
        });
        let report = c.usage_report().expect("usage after two turns");
        assert!(report.contains("turns     2"));
        // `last` reflects the most recent turn; totals sum across both.
        assert!(report.contains("last      in 20 · out 6 · reasoning 3"));
        assert!(report.contains("session   in 30 · out 10 · total 40"));
        let usage = c.token_usage().expect("structured usage after two turns");
        assert_eq!(usage.turns, 2);
        assert_eq!(
            (usage.last_input, usage.last_output, usage.last_reasoning),
            (20, 6, 3)
        );
        assert_eq!(
            (usage.total_input, usage.total_output, usage.total_reasoning),
            (30, 10, 3)
        );
    }

    #[test]
    fn format_usage_omits_reasoning_when_zero_and_keeps_plain_limit_labels() {
        let stats = UsageStats {
            turns: 1,
            last: Usage {
                input: Some(8),
                output: Some(2),
                reasoning: Some(0),
                ..Usage::default()
            },
            total_input: 8,
            total_output: 2,
            total_reasoning: 0,
            // A non-vendor-prefixed limit header keeps its name (dashes → spaces).
            rate_limits: vec![("ratelimit-remaining".into(), "9".into())],
            ..UsageStats::default()
        };
        let out = format_usage(&stats).expect("present");
        assert!(
            !out.contains("(reasoning"),
            "no reasoning suffix when zero:\n{out}"
        );
        assert!(out.contains("limits    ratelimit remaining 9"));
    }

    #[test]
    fn synth_session_id_is_uuid_shaped() {
        let id = synth_session_id();
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(
            parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
            vec![8, 4, 4, 4, 12]
        );
        assert!(id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
    }

    #[test]
    fn default_model_comes_from_env_or_codex_config() {
        let _guard = env_lock();
        let old_env = std::env::var("ANGEL_OPENAI_MODEL").ok();
        let old_home = std::env::var("CODEX_HOME").ok();
        let old_effort = std::env::var("ANGEL_OPENAI_REASONING_EFFORT").ok();
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_OPENAI_MODEL", " env-model ") };
        assert_eq!(CodexClub::default_model().as_deref(), Some("env-model"));

        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_OPENAI_MODEL") };
        let dir = std::env::temp_dir().join(format!(
            "angel_codex_home_{}_{}",
            std::process::id(),
            now_secs()
        ));
        std::fs::create_dir_all(&dir).expect("create temp codex home");
        std::fs::write(dir.join("config.toml"), "model = \"config-model\"\n")
            .expect("write temp config");
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("CODEX_HOME", &dir) };
        assert_eq!(CodexClub::default_model().as_deref(), Some("config-model"));
        let _ = std::fs::remove_dir_all(dir);

        // A forbidden pin stays visible so validation can refuse it truthfully.
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_OPENAI_MODEL", "gpt-5.3-codex-spark") };
        assert_eq!(
            CodexClub::default_model().as_deref(),
            Some("gpt-5.3-codex-spark")
        );
        assert!(CodexClub::resolve_selection().error.is_some());
        assert!(is_private_test_codex_model("gpt-5.3-codex-spark"));
        assert_eq!(
            canonicalize_openai_model("gpt-5.3-codex-spark"),
            OPENAI_LUNA_MODEL
        );

        match old_env {
            // TODO: Audit that the environment access only happens in single-threaded code.
            Some(v) => unsafe { std::env::set_var("ANGEL_OPENAI_MODEL", v) },
            // TODO: Audit that the environment access only happens in single-threaded code.
            None => unsafe { std::env::remove_var("ANGEL_OPENAI_MODEL") },
        }
        match old_home {
            // TODO: Audit that the environment access only happens in single-threaded code.
            Some(v) => unsafe { std::env::set_var("CODEX_HOME", v) },
            // TODO: Audit that the environment access only happens in single-threaded code.
            None => unsafe { std::env::remove_var("CODEX_HOME") },
        }
        match old_effort {
            // TODO: Audit that the environment access only happens in single-threaded code.
            Some(v) => unsafe { std::env::set_var("ANGEL_OPENAI_REASONING_EFFORT", v) },
            // TODO: Audit that the environment access only happens in single-threaded code.
            None => unsafe { std::env::remove_var("ANGEL_OPENAI_REASONING_EFFORT") },
        }
    }

    #[test]
    fn model_catalog_strips_codex_spark_surfaces() {
        let models = model_catalog_from_str(
            r#"{
  "models": [
    {"slug":"gpt-5.6-luna","display_name":"Luna","visibility":"list","priority":1,
     "default_reasoning_level":"max","supported_reasoning_levels":[{"effort":"max","description":"max"}]},
    {"slug":"gpt-5.3-codex-spark","display_name":"Spark","visibility":"list","priority":0,
     "default_reasoning_level":"high","supported_reasoning_levels":[{"effort":"high","description":"h"}]}
  ]
}"#,
        );
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].slug, "gpt-5.6-luna");
        assert!(!models.iter().any(|m| m.slug.contains("spark")));
    }
}
