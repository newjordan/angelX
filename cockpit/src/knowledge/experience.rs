//! The experience ledger: one append-only JSONL line per turn recording the
//! *configuration that ran* and the *outcome it produced*.
//!
//! The cockpit already measures its own operation — failover reasons, anti-spin
//! counters, dissent scores, the knobs in force — but every one of those signals
//! evaporates when the turn ends (a local `let mut`, an `eprintln!`, a discarded
//! return value). This module keeps them: a durable, structured record of what
//! angelX did and how it went, keyed by the configuration hash so rows group by
//! the exact seat/knob layout that produced them.
//!
//! It is the substrate for Reflex (`docs/reflex/`): the causal engine mines this
//! ledger for observational hypotheses ("agg=longcat turns show 4/20 raw-markup
//! failovers vs 0/23 for codex-run"), Control Bench experiments conclude them,
//! and the reconfigurator writes winners back with the evidence attached.
//!
//! Design mirrors [`crate::agent::swarm::ledger`] (append-only JSONL, best-effort I/O,
//! test-silent) so a broken disk can never fail a turn. Core record kinds include
//! `turn` (the single-agent harness loop, [`record_turn`]), `moa_turn` (the
//! SOTA-MOA pipeline, [`record_moa`]), and `route_verdict` (one explicit,
//! text-free usefulness label). Pure record *builders* are separated from the
//! side-effecting writers so the schema is unit-tested without touching the real
//! `~/.angel` or the environment.
//!
//! A third kind, `event` (currently one subkind, `cmd`), records individual
//! command executions — text (secret-scrubbed), exit status, duration — and
//! every record carries a `repo` stamp identifying the workspace it ran in.
//! That is the substrate for the Repo Dossier (`docs/plans/repo-dossier.md`):
//! per-repo facts (build rituals, test invocations, traps) mined from what the
//! user's own sessions actually ran and whether it worked.
//!
//! Controls: `ANGEL_EXPERIENCE=0` disables all recording; `ANGEL_EXPERIENCE_LOG`
//! overrides the path (default `~/.angelX/experience/ledger.jsonl`);
//! `ANGEL_EXPERIENCE_EVENTS=0` disables just the per-command `event` records
//! (chattier than turns). The active ledger compacts to its newest complete
//! records before a write would exceed 50 MiB; no new archive files accumulate.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// Schema version stamped on every record. v2 adds exact `{driver, model,
/// reasoning_effort}` route metadata to single-agent turns. **v3 changes what
/// `cmd.exit` means**: before it, a piped command recorded its LAST stage's
/// status (`cargo check … | tail -20` → `tail`'s 0, whether or not the build
/// passed), so no piped `exit` in a v≤2 row is evidence of anything. From v3 the
/// shell propagates a failing stage (`pipefail`) and every `cmd` row carries the
/// provenance to prove it: `shell`, `pipefail`, and the [`cmd_verdict`] tristate.
const SCHEMA_V: u64 = 3;

/// Legacy rotation fixture threshold; production reads store-caps.toml.
#[cfg(test)]
const ROTATE_BYTES: u64 = 50 * 1024 * 1024;

/// Whether experience recording is on (`ANGEL_EXPERIENCE`, default **on**).
pub(crate) fn enabled() -> bool {
    env_flag("ANGEL_EXPERIENCE", true)
}

/// A permissive boolean env flag: unset → `default`; set to `0`/`false`/`off`/
/// `no` (any case) → false; anything else → true.
pub(crate) fn env_flag(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(v) => {
            let t = v.trim();
            !(t == "0"
                || t.eq_ignore_ascii_case("false")
                || t.eq_ignore_ascii_case("off")
                || t.eq_ignore_ascii_case("no"))
        }
        Err(_) => default,
    }
}

/// Ledger file: `ANGEL_EXPERIENCE_LOG`, else `~/.angelX/experience/ledger.jsonl`.
pub(crate) fn ledger_path() -> PathBuf {
    match std::env::var("ANGEL_EXPERIENCE_LOG") {
        Ok(p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => crate::platform::workspace_store::angel_subdir("experience").join("ledger.jsonl"),
    }
}

// ---------------------------------------------------------------------------
// Configuration snapshot — the "what ran" half of every record.
// ---------------------------------------------------------------------------

/// The high-leverage `ANGEL_*` knobs whose values are safe to record and whose
/// effect on turn economics Reflex wants to learn. Deliberately a curated
/// routing/behavior subset (~20), **not** the full 242-var inventory
/// (`cockpit/docs/ENV.md`) — the causal catalog (M2) refines this list. Values
/// are snapshotted only for names that pass [`is_secret_name`].
pub(crate) const KNOB_CATALOG: &[&str] = &[
    // SOTA-MOA seat routing (the four seats Reflex re-seats)
    "ANGEL_SOTA_MOA_PROPOSE_CLUB",
    "ANGEL_SOTA_MOA_JUDGE_CLUB",
    "ANGEL_SOTA_MOA_VERIFY_CLUB",
    "ANGEL_SOTA_MOA_AGG_CLUB",
    // SOTA-MOA behavior toggles
    "ANGEL_SOTA_MOA_ALWAYS",
    "ANGEL_SOTA_MOA_JUDGE",
    "ANGEL_SOTA_MOA_VERIFY",
    "ANGEL_SOTA_CAVEMAN",
    "ANGEL_MOA_JUDGE_FANOUT",
    // SOTA-MOA formation shape — the per-formation knobs Reflex must see to
    // attribute (and later steer) each formation's token economics.
    "ANGEL_SOTA_MOA_WIDTH",
    "ANGEL_SOTA_MOA_MAX_WIDTH",
    "ANGEL_SOTA_MOA_LAYERS",
    "ANGEL_SOTA_MOA_SAMPLES",
    "ANGEL_SOTA_MOA_JUDGE_PANEL",
    "ANGEL_SOTA_MOA_REFLECT",
    "ANGEL_SOTA_MOA_GROK_RESEARCH",
    // pipeline shape / driver selection
    "ANGEL_PXPIPE",
    "ANGEL_DRIVER",
    // harness guardrails
    "ANGEL_YOLO",
    "ANGEL_YOLO_SMART",
    "ANGEL_SPIN_LIMIT",
    "ANGEL_POLL_REPEAT_LIMIT",
    "ANGEL_MAX_HOPS",
    "ANGEL_MACHINE_QUEUE_HOST",
    "ANGEL_MACHINE_QUEUE_RESOURCE",
    "ANGEL_MACHINE_QUEUE_OWNER",
    "ANGEL_MACHINE_QUEUE_REMOTE_CWD",
    "ANGEL_MACHINE_QUEUE_WAIT_SECS",
    "ANGEL_MACHINE_QUEUE_LEASE_SECS",
    "ANGEL_MACHINE_QUEUE_QUANTUM_SECS",
    "ANGEL_COMPETITION_ID",
    "ANGEL_ERROR_LIMIT",
    "ANGEL_NOPROGRESS_LIMIT",
    "ANGEL_FIRST_WRITE_CALLS",
    "ANGEL_VERIFY_BEFORE_DONE",
    "ANGEL_VERIFY_NUDGES",
    "ANGEL_TRUNCATION_RETRY",
    "ANGEL_TRUNCATION_RETRIES",
    "ANGEL_TRUNCATION_RETRY_MAX",
    "ANGEL_PROJECT_DOC",
    "ANGEL_PROJECT_DOC_MAX_BYTES",
    "ANGEL_TOOL_SCHEMA_PROFILE",
    "ANGEL_BOUNDED_TASK_SCHEMAS",
    "ANGEL_SKILL_HINT",
    "ANGEL_TOOL_SEARCH_ACTIVE_MAX",
    "ANGEL_OPENROUTER_ANTHROPIC_CACHE",
    "ANGEL_ANTHROPIC_CACHE_PREFIX_FLOOR",
    "ANGEL_ANTHROPIC_CACHE_TTL",
    // Treebeard / Hi/Q lane (Zhang–Khattab RLM contract)
    "ANGEL_LANE",
    "ANGEL_HANDLE_STORE",
    "ANGEL_HANDLE_READ_TOOL",
    "ANGEL_HANDLE_EAGER_TOOL_MIN_BYTES",
    "ANGEL_HANDLE_CODE_MODE_MIN_BYTES",
    "ANGEL_HANDLE_SUBCALL_MIN_BYTES",
    "ANGEL_TREEBEARD_MAX_DEPTH",
    "ANGEL_ROOT_TRAJECTORY",
    "ANGEL_TOOL_AGING",
];

/// True if a variable name looks like it holds a credential — never recorded,
/// even if it somehow appears in the catalog. Case-insensitive substring match
/// on the standard secret markers (`.angel.env` holds live API keys).
pub(crate) fn is_secret_name(name: &str) -> bool {
    let up = name.to_ascii_uppercase();
    ["KEY", "TOKEN", "SECRET", "AUTH", "PASSWORD"]
        .iter()
        .any(|p| up.contains(p))
}

/// Snapshot the current values of the catalog knobs (secret-named entries and
/// empty/unset values dropped). Deterministic order (BTreeMap) so the hash is
/// stable across runs with the same configuration.
pub(crate) fn snapshot_knobs() -> BTreeMap<String, String> {
    snapshot_from(KNOB_CATALOG, |k| std::env::var(k).ok())
}

/// Pure core of [`snapshot_knobs`]: resolves each name via `lookup`, applying the
/// secret denylist and empty-value drop. Env-free so it's unit-testable without
/// a process-wide `set_var` race.
fn snapshot_from<F>(names: &[&str], lookup: F) -> BTreeMap<String, String>
where
    F: Fn(&str) -> Option<String>,
{
    let mut out = BTreeMap::new();
    for &name in names {
        if is_secret_name(name) {
            continue;
        }
        if let Some(v) = lookup(name) {
            let v = v.trim();
            if !v.is_empty() {
                out.insert(name.to_string(), v.to_string());
            }
        }
    }
    out
}

/// A stable content hash of a knob map, so ledger rows group by configuration.
/// Order-independent (the map is sorted); deterministic across processes on the
/// same build — same guarantee `workspace_store::workspace_key` relies on.
pub(crate) fn cfg_hash(knobs: &BTreeMap<String, String>) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (k, v) in knobs {
        k.hash(&mut h);
        v.hash(&mut h);
    }
    format!("{:016x}", h.finish())
}

/// The `cfg` object embedded in every record: `{hash, knobs}`. Reads the live
/// environment (the whitelist snapshot).
pub(crate) fn cfg_value() -> serde_json::Value {
    let knobs = snapshot_knobs();
    serde_json::json!({ "hash": cfg_hash(&knobs), "knobs": knobs })
}

// ---------------------------------------------------------------------------
// Repo identity — the "where it happened" stamp on every record.
// ---------------------------------------------------------------------------

thread_local! {
    /// Exact registry workspace for the turn executing on this worker thread.
    /// A process-global "current repo" races as soon as two project-bound turns
    /// run concurrently; thread-local scope keeps deep MoA telemetry attached
    /// to the registry that actually launched it.
    static TURN_WORKSPACE: std::cell::RefCell<Option<PathBuf>> = const {
        std::cell::RefCell::new(None)
    };
}

/// Bind deep pipeline telemetry to the registry driving the current turn.
pub(crate) fn note_turn_workspace(workspace: &Path) {
    TURN_WORKSPACE.with(|cell| *cell.borrow_mut() = Some(workspace.to_path_buf()));
}

/// Repository stamp for the turn executing on this thread. `None` outside a
/// project-bound harness turn; callers then leave records unscoped/inert rather
/// than guessing from process-global state.
pub(crate) fn current_turn_repo_value() -> Option<serde_json::Value> {
    TURN_WORKSPACE.with(|cell| cell.borrow().as_deref().map(repo_value_for))
}

pub(crate) fn current_turn_workspace() -> Option<PathBuf> {
    TURN_WORKSPACE.with(|cell| cell.borrow().clone())
}

/// The `repo` object stamped on records: `{key, root, slug, cwd?}`.
///
/// The identity is the **repository**, not the directory
/// ([`workspace_store::repo_identity`]): a linked git worktree and a
/// subdirectory both stamp their main repo's `key` and `root`, so evidence
/// accumulates in one place instead of scattering across a new key per
/// worktree. Before this, `scripts/cut-forge.mjs` — which drives `angel --task`
/// in a *disposable* worktree per backlog item — filed every command it ran and
/// every diff it authored under a key that died with the worktree.
///
/// `cwd` is present only when the work happened somewhere other than the repo
/// root (a worktree, a subdirectory), so which checkout produced a failure is
/// still recoverable from the row. Node-side consumers never re-derive the key —
/// they group by this stamped value, keeping the hash scheme Rust-only.
pub(crate) fn repo_value_for(folder: &Path) -> serde_json::Value {
    let id = crate::platform::workspace_store::repo_identity(folder);
    let cwd = (id.root != folder).then(|| folder.display().to_string());
    serde_json::json!({
        "key": id.key,
        "root": id.root.display().to_string(),
        "slug": id.slug,
        "cwd": cwd,
    })
}

// ---------------------------------------------------------------------------
// Command-text hygiene — recorded text must never carry a credential.
// ---------------------------------------------------------------------------

/// Recorded command text is capped here (char-boundary safe): the dossier's
/// classifier needs the head of the command, not the payload.
const CMD_TEXT_MAX: usize = 400;

/// Substrings that mark a token as secret-bearing. Deliberately the same
/// vocabulary as [`is_secret_name`] plus the header/flag spellings that show up
/// in command lines (`Bearer …`, `--token …`).
const SECRET_MARKERS: [&str; 7] = [
    "key", "token", "secret", "auth", "password", "passwd", "bearer",
];

fn is_marker_word(word: &str) -> bool {
    let low = word.to_ascii_lowercase();
    SECRET_MARKERS.iter().any(|m| low.contains(m))
}

/// Mask likely credential values in a command line before it is recorded.
/// Three rules, all conservative (over-masking is fine — the record only needs
/// enough text to classify the command; under-masking is the failure mode):
/// 1. A token whose name part contains a secret marker keeps its name but has
///    the value after its `=`/`:` replaced (`API_KEY=…`).
/// 2. A bare marker word (`--token`, `Bearer`, `Authorization:`) masks the next
///    non-marker token.
/// 3. URL userinfo is masked (`https://…@host` → `https://…@host` with the
///    credentials replaced).
///
/// Whitespace is normalized to single spaces (the record is for mining, not
/// replay).
pub(crate) fn scrub_secrets(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut mask_next = false;
    for tok in text.split_whitespace() {
        let tok = mask_url_userinfo(tok);
        let (name, sep_val) = match tok.find(['=', ':']) {
            Some(i) => (&tok[..i], Some((tok.as_bytes()[i] as char, &tok[i + 1..]))),
            None => (tok.as_str(), None),
        };
        let marker = is_marker_word(name);
        if mask_next && !marker {
            out.push("…".to_string());
            mask_next = false;
            continue;
        }
        if marker {
            match sep_val {
                // `NAME=value` / `Header:value` — mask the value in place.
                Some((sep, v)) if !v.is_empty() => out.push(format!("{name}{sep}…")),
                // `--token` / `Authorization:` / `Bearer` — arm for the next
                // token (chained markers keep the arm until a value appears).
                _ => {
                    out.push(tok.clone());
                    mask_next = true;
                }
            }
        } else {
            out.push(tok.clone());
        }
    }
    crate::platform::secrets::redact_str(&out.join(" ")).into_owned()
}

/// Mask `user:pass@` userinfo inside a URL-shaped token.
fn mask_url_userinfo(tok: &str) -> String {
    if let Some(scheme_end) = tok.find("://") {
        let rest = &tok[scheme_end + 3..];
        if let Some(at) = rest.find('@') {
            // Only when the userinfo section is before any path separator.
            if !rest[..at].contains('/') {
                return format!("{}://…@{}", &tok[..scheme_end], &rest[at + 1..]);
            }
        }
    }
    tok.to_string()
}

fn cap_text(s: &str) -> String {
    if s.len() <= CMD_TEXT_MAX {
        return s.to_string();
    }
    let mut end = CMD_TEXT_MAX;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

// ---------------------------------------------------------------------------
// Failover sink — process-global, drained into the next turn record.
// ---------------------------------------------------------------------------

/// One text-stage failover: the club we left, the club we landed on, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Failover {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) reason: String,
}

fn failover_sink() -> &'static Mutex<Vec<Failover>> {
    static SINK: OnceLock<Mutex<Vec<Failover>>> = OnceLock::new();
    SINK.get_or_init(|| Mutex::new(Vec::new()))
}

/// Record a failover into the process-global sink; drained by the next turn
/// record. Best-effort (a poisoned lock is ignored) and capped so a driver that
/// never records turns can't grow it without bound. Recording concurrent turns
/// on one box would blur attribution — the same close-enough caveat the MoA
/// ledger's token snapshot carries — but the cockpit drives one turn at a time.
pub(crate) fn note_failover(from: &str, to: &str, reason: &str) {
    if cfg!(test) || !enabled() {
        return;
    }
    if let Ok(mut v) = failover_sink().lock()
        && v.len() < 256
    {
        v.push(Failover {
            from: from.to_string(),
            to: to.to_string(),
            reason: cap_text(&scrub_secrets(reason)),
        });
    }
}

/// Take and clear every failover accumulated since the last drain.
pub(crate) fn drain_failovers() -> Vec<Failover> {
    match failover_sink().lock() {
        Ok(mut v) => std::mem::take(&mut *v),
        Err(_) => Vec::new(),
    }
}

fn failovers_json(failovers: &[Failover]) -> Vec<serde_json::Value> {
    failovers
        .iter()
        .map(|f| serde_json::json!({ "from": f.from, "to": f.to, "reason": f.reason }))
        .collect()
}

fn tokens_json(tokens: &[(String, u64, u64)]) -> serde_json::Map<String, serde_json::Value> {
    let mut m = serde_json::Map::new();
    for (label, t_in, t_out) in tokens {
        m.insert(
            label.clone(),
            serde_json::json!({ "in": t_in, "out": t_out }),
        );
    }
    m
}

// ---------------------------------------------------------------------------
// `turn` records — the single-agent harness loop (run_turn).
// ---------------------------------------------------------------------------

/// The four bounded tool-error classes a dispatch-level failure
/// (`tool error: …`) falls into. `schema` is the pure argument-misuse class
/// (missing/malformed/exclusive-choice arguments) the waste audit tracks: if
/// the tolerant-validators work lands, that bucket should collapse toward zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ToolErrorClasses {
    pub(crate) schema: u32,
    pub(crate) exec: u32,
    pub(crate) timeout: u32,
    pub(crate) other: u32,
}

impl ToolErrorClasses {
    pub(crate) fn record(&mut self, class: ToolErrorClass) {
        match class {
            ToolErrorClass::Schema => self.schema = self.schema.saturating_add(1),
            ToolErrorClass::Exec => self.exec = self.exec.saturating_add(1),
            ToolErrorClass::Timeout => self.timeout = self.timeout.saturating_add(1),
            ToolErrorClass::Other => self.other = self.other.saturating_add(1),
        }
    }

    pub(crate) fn to_json(self) -> serde_json::Value {
        serde_json::json!({
            "schema": self.schema,
            "exec": self.exec,
            "timeout": self.timeout,
            "other": self.other,
        })
    }
}

pub(crate) enum ToolErrorClass {
    Schema,
    Exec,
    Timeout,
    Other,
}

impl ToolErrorClass {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            ToolErrorClass::Schema => "schema",
            ToolErrorClass::Exec => "exec",
            ToolErrorClass::Timeout => "timeout",
            ToolErrorClass::Other => "other",
        }
    }
}

/// Classify one dispatch-level tool error by the harness vocabulary that
/// produced it. Heuristic substring matching over a lowercased copy — bounded
/// (four buckets, no per-error storage) and good enough to tell whether the
/// schema class is dying. Order matters: timeouts first (unambiguous), then
/// argument-shape phrases, then process-failure markers.
pub(crate) fn classify_tool_error(result: &str) -> ToolErrorClass {
    let text = result.to_ascii_lowercase();
    if text.contains("timed out") || text.contains("timeout") {
        ToolErrorClass::Timeout
    } else if text.contains("missing '")
        || text.contains("must be")
        || text.contains("must not")
        || text.contains("must contain")
        || text.contains("accepts at most")
        || text.contains("at most one")
        || text.contains("exactly one of")
        || text.contains("requires a ")
        || (text.contains("exceeds") && text.contains("limit"))
    {
        ToolErrorClass::Schema
    } else if text.contains("exit")
        || text.contains("no such file")
        || text.contains("permission denied")
        || text.contains("signal")
        || text.contains("spawn")
        || text.contains("connect")
    {
        ToolErrorClass::Exec
    } else {
        ToolErrorClass::Other
    }
}

/// The anti-spin / progress counters a turn accumulated, snapshotted at exit.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct TurnCounters {
    pub(crate) deferred_nudges: usize,
    pub(crate) spin: usize,
    pub(crate) err_streak: usize,
    pub(crate) churn: usize,
    /// Inspection batches rejected after the bounded first-write budget.
    pub(crate) first_write_rejections: usize,
    /// Exact read/search outputs replaced by newest-full duplicate receipts.
    pub(crate) duplicate_inspection_results: usize,
    pub(crate) duplicate_inspection_bytes_saved: u64,
    /// Older reconstructable inspection outputs replaced by hop- and
    /// token-budget-aware provenance receipts.
    pub(crate) aged_inspection_results: usize,
    pub(crate) aged_inspection_bytes_saved: u64,
    /// Large tool results offloaded to handles *before* entering root history
    /// (Treebeard/HiQ bulk veto — not the later aging pass).
    pub(crate) eager_offload_results: usize,
    pub(crate) eager_offload_bytes_saved: u64,
    /// Bounded same-hop language-server feedback after successful mutations.
    pub(crate) post_edit_diagnostic_attempts: usize,
    pub(crate) post_edit_diagnostic_findings: usize,
    pub(crate) post_edit_diagnostic_failures: usize,
    pub(crate) post_edit_diagnostic_paths_skipped: usize,
    pub(crate) post_edit_diagnostic_output_bytes: u64,
    pub(crate) post_edit_diagnostic_elapsed_ms: u64,
    /// Older successfully completed edit calls whose schema-known payload
    /// strings were replaced by valid bounded receipts before a later hop.
    pub(crate) tool_argument_shrinks: usize,
    pub(crate) tool_argument_strings_shrunk: usize,
    pub(crate) tool_argument_bytes_saved: u64,
    /// Read/search operations whose stable semantic identity had already been
    /// inspected earlier in the turn. Measured even when the churn guard is off.
    pub(crate) repeated_inspections: usize,
    /// Programmatic read-only reconnaissance use and its bounded nested work.
    pub(crate) code_mode_calls: usize,
    pub(crate) code_mode_nested_calls: usize,
    pub(crate) code_mode_nested_output_bytes: u64,
    pub(crate) code_mode_policy_rejections: usize,
    /// Fixed trusted recipes, separated from free-form scripts.
    pub(crate) code_mode_recipe_calls: usize,
    /// Exact bounded evidence bytes injected before the first provider request.
    pub(crate) task_recon_context_bytes: usize,
    /// Marginal fixed schema cost on each provider request for code mode.
    pub(crate) code_mode_schema_tokens: usize,
    /// Provider requests rejected for exceeding the context window, whether or
    /// not bounded compaction recovered the turn.
    pub(crate) request_overflows: usize,
    /// Prompt-cache request markers and provider-reported read/write token
    /// deltas observed during this turn.
    pub(crate) cache_control_requests: u64,
    pub(crate) cache_read_input_tokens: u64,
    pub(crate) cache_write_input_tokens: u64,
    pub(crate) cache_read_accounting_responses: u64,
    pub(crate) cache_write_accounting_responses: u64,
    /// Unsupported completion claims accepted after verification policy was
    /// disabled or its bounded denial budget was exhausted.
    pub(crate) unverified_completion_claims: usize,
    /// Identical conclusive verifier calls reused on an unchanged Git-backed
    /// workspace instead of re-executing redundant work.
    pub(crate) redundant_verifier_skips: usize,
    /// Activated hidden-tool schemas missing from the actual next provider
    /// definition set.
    pub(crate) discovered_tool_schema_failures: usize,
    /// Unsupported completion claims denied after a behavior-changing edit.
    pub(crate) verification_denials: usize,
    /// Deterministic high-confidence skill hints injected before this turn.
    pub(crate) skill_hints: usize,
    /// Extra provider requests spent recovering unusable truncated output.
    pub(crate) provider_truncation_retries: u64,
    /// Provider/model cutoff episodes observed during this turn.
    pub(crate) provider_truncation_episodes: u64,
    /// Usable partial prose retained without replaying a visible stream.
    pub(crate) provider_truncation_retained_partials: u64,
    /// Truncation episodes that eventually returned one usable reply.
    pub(crate) provider_truncation_recoveries: u64,
    /// Truncation episodes that exhausted/aborted recovery.
    pub(crate) provider_truncation_failures: u64,
    /// Action-capsule batches previewed in this turn. Zero when the feature is
    /// off, preserving the ordinary fast path.
    pub(crate) action_operations: usize,
    pub(crate) action_previews: usize,
    /// Scoped operations the operator denied before dispatch.
    pub(crate) action_denied: usize,
    /// Local-only preview/approval preparation time; no model or subprocess
    /// work is included. Lets the ledger prove the control layer stays cheap.
    pub(crate) action_preflight_us: u64,
    /// Explicit human decision time, deliberately separated from local
    /// overhead and model/tool latency.
    pub(crate) action_approval_wait_ms: u64,
    /// Sum of elapsed time for capsule-scoped tool dispatches. This is not an
    /// extra timer around non-action calls and does not assert command success.
    pub(crate) action_exec_ms: u64,
    /// Dispatch-level tool errors this turn, split into four bounded classes
    /// (schema / exec / timeout / other) so audits can see whether tolerant
    /// validators removed the pure argument-schema waste class.
    pub(crate) tool_errors_by_class: ToolErrorClasses,
}

/// Everything one `run_turn` learned about itself, assembled at the exit site.
pub(crate) struct TurnExperience<'a> {
    /// Driver family / mode (`ANGEL_DRIVER`, else "single").
    pub(crate) path: &'a str,
    /// The club that drove the turn (`club.label()`).
    pub(crate) driver: &'a str,
    /// Exact configured/live model identity and backend-local thinking level.
    /// Both are metadata only; prompts and responses never enter this record.
    pub(crate) model: Option<&'a str>,
    pub(crate) reasoning_effort: Option<&'a str>,
    pub(crate) ok: bool,
    /// Why the turn ended: "answer" | "interrupt" | "deadline" | "spin" |
    /// "error_stop" | "churn_stop" | "deferred_stop" | "max_hops".
    pub(crate) stop: &'a str,
    pub(crate) latency_ms: u128,
    pub(crate) hops: usize,
    /// Wall time from turn start to the first successful mutation and to the
    /// first passing task-acceptance check. `None` means the event never happened.
    pub(crate) time_to_first_mutation_ms: Option<u64>,
    pub(crate) time_to_green_ms: Option<u64>,
    /// Stable leading System-role prefix estimate before per-hop history.
    pub(crate) system_prompt_tokens: usize,
    /// Effective project-instruction payload bytes stamped into the exact System
    /// prefix, excluding the fixed explanatory wrapper.
    pub(crate) project_doc_bytes: usize,
    /// Initial fixed tool-schema payload before any dynamic discovery.
    pub(crate) tool_schema_count: usize,
    pub(crate) tool_schema_tokens: usize,
    /// Peak dynamically advertised payload and the sum of schema estimates over
    /// every actual provider request (including transport retries).
    pub(crate) tool_schema_peak_count: usize,
    pub(crate) tool_schema_peak_tokens: usize,
    pub(crate) tool_schema_token_requests: usize,
    pub(crate) counters: TurnCounters,
    /// Per-club (input, output) token spend for this turn.
    pub(crate) tokens: Vec<(String, u64, u64)>,
    pub(crate) failovers: Vec<Failover>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RouteVerdict {
    Useful,
    Miss,
}

impl RouteVerdict {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Useful => "useful",
            Self::Miss => "miss",
        }
    }
}

/// Serialize one finished single-agent turn. Pure — the write is in
/// [`record_turn`]; `ts`/`session`/`seq`/`repo` are injected so the shape is
/// testable.
pub(crate) fn turn_record(
    exp: &TurnExperience,
    cfg: &serde_json::Value,
    repo: &serde_json::Value,
    ts: u64,
    session: u32,
    seq: u64,
) -> serde_json::Value {
    let mut counters = serde_json::json!({
        "deferred_nudges": exp.counters.deferred_nudges,
        "spin": exp.counters.spin,
        "err_streak": exp.counters.err_streak,
        "churn": exp.counters.churn,
        "first_write_rejections": exp.counters.first_write_rejections,
        "duplicate_inspection_results": exp.counters.duplicate_inspection_results,
        "duplicate_inspection_bytes_saved": exp.counters.duplicate_inspection_bytes_saved,
        "aged_inspection_results": exp.counters.aged_inspection_results,
        "aged_inspection_bytes_saved": exp.counters.aged_inspection_bytes_saved,
        "tool_argument_shrinks": exp.counters.tool_argument_shrinks,
        "tool_argument_strings_shrunk": exp.counters.tool_argument_strings_shrunk,
        "tool_argument_bytes_saved": exp.counters.tool_argument_bytes_saved,
        "repeated_inspections": exp.counters.repeated_inspections,
        "code_mode_calls": exp.counters.code_mode_calls,
        "code_mode_nested_calls": exp.counters.code_mode_nested_calls,
        "code_mode_nested_output_bytes": exp.counters.code_mode_nested_output_bytes,
        "code_mode_policy_rejections": exp.counters.code_mode_policy_rejections,
        "code_mode_recipe_calls": exp.counters.code_mode_recipe_calls,
        "task_recon_context_bytes": exp.counters.task_recon_context_bytes,
        "code_mode_schema_tokens": exp.counters.code_mode_schema_tokens,
        "request_overflows": exp.counters.request_overflows,
        "cache_control_requests": exp.counters.cache_control_requests,
        "cache_read_input_tokens": exp.counters.cache_read_input_tokens,
        "cache_write_input_tokens": exp.counters.cache_write_input_tokens,
        "cache_read_accounting_responses": exp.counters.cache_read_accounting_responses,
        "cache_write_accounting_responses": exp.counters.cache_write_accounting_responses,
        "unverified_completion_claims": exp.counters.unverified_completion_claims,
        "redundant_verifier_skips": exp.counters.redundant_verifier_skips,
        "discovered_tool_schema_failures": exp.counters.discovered_tool_schema_failures,
        "verification_denials": exp.counters.verification_denials,
        "skill_hints": exp.counters.skill_hints,
        "provider_truncation_retries": exp.counters.provider_truncation_retries,
        "provider_truncation_recoveries": exp.counters.provider_truncation_recoveries,
        "provider_truncation_failures": exp.counters.provider_truncation_failures,
        "action_operations": exp.counters.action_operations,
        "action_previews": exp.counters.action_previews,
        "action_denied": exp.counters.action_denied,
        "action_preflight_us": exp.counters.action_preflight_us,
        "action_approval_wait_ms": exp.counters.action_approval_wait_ms,
        "action_exec_ms": exp.counters.action_exec_ms,
    });
    let counter_map = counters
        .as_object_mut()
        .expect("turn counters are constructed as an object");
    // Keep the json! payload under the macro recursion ceiling by inserting
    // newer counters here (same pattern as the post-edit / truncation fields).
    counter_map.insert(
        "eager_offload_results".into(),
        exp.counters.eager_offload_results.into(),
    );
    counter_map.insert(
        "eager_offload_bytes_saved".into(),
        exp.counters.eager_offload_bytes_saved.into(),
    );
    counter_map.insert(
        "provider_truncation_episodes".into(),
        exp.counters.provider_truncation_episodes.into(),
    );
    counter_map.insert(
        "provider_truncation_retained_partials".into(),
        exp.counters.provider_truncation_retained_partials.into(),
    );
    counter_map.insert(
        "post_edit_diagnostic_attempts".into(),
        exp.counters.post_edit_diagnostic_attempts.into(),
    );
    counter_map.insert(
        "post_edit_diagnostic_findings".into(),
        exp.counters.post_edit_diagnostic_findings.into(),
    );
    counter_map.insert(
        "post_edit_diagnostic_failures".into(),
        exp.counters.post_edit_diagnostic_failures.into(),
    );
    counter_map.insert(
        "post_edit_diagnostic_paths_skipped".into(),
        exp.counters.post_edit_diagnostic_paths_skipped.into(),
    );
    counter_map.insert(
        "post_edit_diagnostic_output_bytes".into(),
        exp.counters.post_edit_diagnostic_output_bytes.into(),
    );
    counter_map.insert(
        "post_edit_diagnostic_elapsed_ms".into(),
        exp.counters.post_edit_diagnostic_elapsed_ms.into(),
    );
    counter_map.insert(
        "tool_errors_by_class".into(),
        exp.counters.tool_errors_by_class.to_json(),
    );
    serde_json::json!({
        "kind": "turn",
        "v": SCHEMA_V,
        "ts": ts,
        "session": session,
        "seq": seq,
        "path": exp.path,
        "driver": exp.driver,
        "route": {
            "driver": exp.driver,
            "model": exp.model,
            "reasoning_effort": exp.reasoning_effort,
        },
        "cfg": cfg,
        "repo": repo,
        "outcome": {
            "ok": exp.ok,
            "stop": exp.stop,
            "latency_ms": exp.latency_ms as u64,
            "hops": exp.hops,
            "time_to_first_mutation_ms": exp.time_to_first_mutation_ms,
            "time_to_green_ms": exp.time_to_green_ms,
            "system_prompt_tokens": exp.system_prompt_tokens,
            "project_doc_bytes": exp.project_doc_bytes,
            "tool_schema_count": exp.tool_schema_count,
            "tool_schema_tokens": exp.tool_schema_tokens,
            "tool_schema_peak_count": exp.tool_schema_peak_count,
            "tool_schema_peak_tokens": exp.tool_schema_peak_tokens,
            "tool_schema_token_requests": exp.tool_schema_token_requests,
            "tokens": tokens_json(&exp.tokens),
            "counters": counters,
            // Hi/Q / Treebeard telemetry for Reflex + morning dashboards.
            // Counter-derived (history not available here); full
            // root_trajectory lives on reward-labeled trajectory rows.
            "hiq": hiq_outcome_json(&exp.counters),
            "failovers": failovers_json(&exp.failovers),
        },
    })
}

/// Normalize `ANGEL_LANE` the same way the harness does (treebeard aliases).
pub(crate) fn experience_lane_label() -> String {
    match std::env::var("ANGEL_LANE") {
        Ok(v) => {
            let t = v.trim().to_ascii_lowercase();
            if matches!(t.as_str(), "treebeard" | "rlm" | "hiq" | "hi/q") {
                "treebeard".into()
            } else if t.is_empty() {
                "default".into()
            } else {
                t
            }
        }
        Err(_) => "default".into(),
    }
}

/// Compact Hi/Q outcome block for experience turns (no bulk tool bodies).
pub(crate) fn hiq_outcome_json(counters: &TurnCounters) -> serde_json::Value {
    let eager = counters.eager_offload_results;
    let aged = counters.aged_inspection_results;
    let dup = counters.duplicate_inspection_results;
    // Soft LID mass: how much inspection volume left root via offload/age/dedupe
    // receipts. Not identical to trajectory offload_ratio (which classifies
    // history tool messages) but correlated and always available at turn exit.
    let offloaded = eager.saturating_add(aged).saturating_add(dup);
    let bulk_proxy = counters.repeated_inspections.saturating_add(1);
    let total = offloaded.saturating_add(bulk_proxy);
    let offload_signal = if total == 0 {
        0.0
    } else {
        offloaded as f64 / total as f64
    };
    let lane = experience_lane_label();
    let mut hiq_priority = 0.5 + 1.25 * offload_signal.clamp(0.0, 1.0);
    if lane == "treebeard" {
        hiq_priority *= 1.25;
    }
    // Match trajectory/forge FORGE_HIQ_WEIGHT_MAX (default 3.0). Hard 2.5
    // flat-lined PRIMARY open-lever stacks on experience ledgers.
    let wmax = std::env::var("FORGE_HIQ_WEIGHT_MAX")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(3.0)
        .clamp(1.0, 5.0);
    hiq_priority = hiq_priority.clamp(0.5, wmax);
    // Prefer true root LID mass from last trajectory classify when present.
    let root = crate::agent::harness::last_root_hiq();
    let (root_offload, root_hiq) = match root {
        Some(h) => (Some(h.offload_ratio), Some(h.hiq_priority)),
        None => (None, None),
    };
    // Living B200 peer frontier (GpuComp / Treebeard competition subject).
    let peer = crate::agent::harness::load_living_peer_snapshot();
    let (peer_us, peer_name) = match peer {
        Some((geo, name, _)) => (Some(geo), Some(name)),
        None => (None, None),
    };
    let peer_p1 = crate::agent::harness::load_living_peer_p1_us();
    let mut map = serde_json::Map::new();
    map.insert("lane".into(), serde_json::Value::String(lane));
    map.insert("eager_offload_results".into(), eager.into());
    map.insert(
        "eager_offload_bytes_saved".into(),
        counters.eager_offload_bytes_saved.into(),
    );
    map.insert("aged_inspection_results".into(), aged.into());
    map.insert(
        "aged_inspection_bytes_saved".into(),
        counters.aged_inspection_bytes_saved.into(),
    );
    map.insert("duplicate_inspection_results".into(), dup.into());
    map.insert("code_mode_calls".into(), counters.code_mode_calls.into());
    map.insert(
        "offload_signal".into(),
        serde_json::json!((offload_signal * 1000.0).round() / 1000.0),
    );
    map.insert(
        "hiq_priority".into(),
        serde_json::json!((hiq_priority * 1000.0).round() / 1000.0),
    );
    if let Some(r) = root_offload {
        map.insert(
            "root_offload_ratio".into(),
            serde_json::json!((r * 1000.0).round() / 1000.0),
        );
    }
    if let Some(p) = root_hiq {
        map.insert(
            "root_hiq_priority".into(),
            serde_json::json!((p * 1000.0).round() / 1000.0),
        );
    }
    if let Some(us) = peer_us {
        map.insert(
            "living_peer_us".into(),
            serde_json::json!((us * 100.0).round() / 100.0),
        );
    }
    if let Some(n) = peer_name {
        map.insert("living_peer_name".into(), serde_json::Value::String(n));
    }
    if let Some(p1) = peer_p1 {
        map.insert(
            "living_peer_p1_us".into(),
            serde_json::json!((p1 * 10.0).round() / 10.0),
        );
    }
    // Free-train / last LoRA cycle (AUTOPROMOTE=0 handoff + live pulse).
    if let Some(snap) = crate::agent::harness::load_forge_train_snap() {
        map.insert(
            "forge_train_state".into(),
            serde_json::Value::String(snap.state.clone()),
        );
        if let Some(ref ver) = snap.version {
            map.insert(
                "forge_adapter_version".into(),
                serde_json::Value::String(ver.clone()),
            );
        }
        if let Some(g) = snap.gate_pass {
            map.insert("forge_gate_pass".into(), serde_json::Value::Bool(g));
        }
        if snap.adapter_local {
            map.insert("forge_adapter_local".into(), serde_json::Value::Bool(true));
        }
        if let (Some(step), Some(total)) = (snap.train_step, snap.train_total) {
            map.insert("forge_train_step".into(), step.into());
            map.insert("forge_train_total".into(), total.into());
        }
        // Mid-train pulse / last-cycle harvest PRIMARY densify surface.
        if let Some(ref open) = snap.open_lever_top {
            map.insert(
                "forge_open_lever_top".into(),
                serde_json::Value::String(open.clone()),
            );
        }
        if let Some(n) = snap.free_train_primary_n
            && n > 0
        {
            map.insert("forge_free_train_primary_n".into(), n.into());
        }
        if let Some(p) = snap.preference_n
            && p > 0
        {
            map.insert("forge_preference_n".into(), p.into());
        }
        if let Some(c) = snap.coding_eval_n
            && c > 0
        {
            map.insert("forge_coding_eval_n".into(), c.into());
        }
        if let Some(cp) = snap.coding_eval_primary_n
            && cp > 0
        {
            map.insert("forge_coding_eval_primary_n".into(), cp.into());
        }
        if let Some(h) = snap.measured_hold_us
            && h.is_finite()
            && h > 0.0
        {
            map.insert(
                "forge_measured_hold_us".into(),
                serde_json::json!((h * 10.0).round() / 10.0),
            );
        }
        if let Some(loss) = snap.train_loss
            && loss.is_finite()
        {
            map.insert(
                "forge_train_loss".into(),
                serde_json::json!((loss * 10000.0).round() / 10000.0),
            );
        }
        // Pulse/finalize loss extrema — Hi/Q join without re-reading forge logs.
        if let Some(lo) = snap.train_loss_min
            && lo.is_finite()
            && lo > 0.0
        {
            map.insert(
                "forge_train_loss_min".into(),
                serde_json::json!((lo * 10000.0).round() / 10000.0),
            );
        }
        if let Some(hi) = snap.train_loss_max
            && hi.is_finite()
            && hi > 0.0
        {
            map.insert(
                "forge_train_loss_max".into(),
                serde_json::json!((hi * 10000.0).round() / 10000.0),
            );
        }
    }
    let hold_n = crate::agent::harness::load_living_peer_shape_holds(8).len();
    if hold_n > 0 {
        map.insert("living_peer_shape_holds_n".into(), hold_n.into());
    }
    // Open levers ranked by board µs (equal-weight geomean) — same order as
    // Treebeard root / popcorn-open-levers.
    let open = crate::agent::harness::load_living_peer_open_levers(4);
    if !open.is_empty() {
        map.insert(
            "living_peer_open_levers_n".into(),
            serde_json::json!(open.len()),
        );
        let keys: Vec<String> = open.iter().map(|l| l.key.clone()).collect();
        map.insert(
            "living_peer_open_lever_keys".into(),
            serde_json::json!(keys),
        );
        if let Some(top) = open.first() {
            let top_us = (top.board_us * 10.0).round() / 10.0;
            map.insert("living_peer_top_open_us".into(), serde_json::json!(top_us));
            map.insert(
                "living_peer_top_open_key".into(),
                serde_json::Value::String(top.key.clone()),
            );
            // Canonical forge join keys (+ attack alias for older strip readers).
            map.insert(
                "living_peer_primary_key".into(),
                serde_json::Value::String(top.key.clone()),
            );
            map.insert("living_peer_primary_us".into(), serde_json::json!(top_us));
            map.insert(
                "living_peer_primary_attack".into(),
                serde_json::Value::String(top.key.clone()),
            );
            map.insert(
                "living_peer_primary_geo_drop_half_pct".into(),
                serde_json::json!((top.geo_drop_if_half_pct * 1000.0).round() / 1000.0),
            );
            if let Some(b) = top.best_us {
                map.insert(
                    "living_peer_primary_best_us".into(),
                    serde_json::json!((b * 10.0).round() / 10.0),
                );
            }
            if let Some(ref n) = top.best_name {
                map.insert(
                    "living_peer_primary_best_name".into(),
                    serde_json::Value::String(n.clone()),
                );
            }
        }
    }
    serde_json::Value::Object(map)
}

/// Append one single-agent turn to the ledger. Best-effort and test-silent.
pub(crate) fn record_turn(exp: &TurnExperience, workspace: &Path) {
    #[cfg(test)]
    {
        let rec = turn_record(
            exp,
            &cfg_value(),
            &repo_value_for(workspace),
            now_secs(),
            std::process::id(),
            next_seq(),
        );
        TEST_TURN_CAPTURE.with(|capture| {
            if let Some(rows) = capture.borrow_mut().as_mut() {
                rows.push(rec);
            }
        });
    }
    #[cfg(not(test))]
    if !enabled() {
        return;
    }
    #[cfg(not(test))]
    let rec = turn_record(
        exp,
        &cfg_value(),
        &repo_value_for(workspace),
        now_secs(),
        std::process::id(),
        next_seq(),
    );
    #[cfg(not(test))]
    append_jsonl(&ledger_path(), &rec);
}

#[cfg(test)]
thread_local! {
    static TEST_TURN_CAPTURE: std::cell::RefCell<Option<Vec<serde_json::Value>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) struct TestTurnCapture;

#[cfg(test)]
#[allow(dead_code)]
impl TestTurnCapture {
    pub(crate) fn start() -> Self {
        TEST_TURN_CAPTURE.with(|capture| *capture.borrow_mut() = Some(Vec::new()));
        Self
    }

    pub(crate) fn take(self) -> Vec<serde_json::Value> {
        TEST_TURN_CAPTURE.with(|capture| capture.borrow_mut().take().unwrap_or_default())
    }
}

#[cfg(test)]
impl Drop for TestTurnCapture {
    fn drop(&mut self) {
        TEST_TURN_CAPTURE.with(|capture| *capture.borrow_mut() = None);
    }
}

/// Append the bounded backplane attribution receipt to the existing experience
/// ledger. This is an event projection, not a second canonical turn database.
pub(crate) fn record_backplane_outcome(
    outcome: &crate::agent::backplane::TurnOutcome,
    workspace: &Path,
) {
    if cfg!(test) || !enabled() {
        return;
    }
    append_jsonl(
        &ledger_path(),
        &serde_json::json!({
            "kind": "turn_outcome",
            "ts": now_secs(),
            "session": std::process::id(),
            "seq": next_seq(),
            "repo": repo_value_for(workspace),
            "outcome": outcome,
        }),
    );
}

pub(crate) fn route_verdict_record(
    route: &crate::agent::club::RouteIdentity,
    verdict: RouteVerdict,
    completed_ms: u64,
    repo: &serde_json::Value,
    ts: u64,
    session: u32,
    seq: u64,
) -> serde_json::Value {
    serde_json::json!({
        "kind": "route_verdict",
        "v": SCHEMA_V,
        "ts": ts,
        "session": session,
        "seq": seq,
        "turn_completed_ms": completed_ms,
        "source": "user",
        "route": route,
        "verdict": verdict,
        "repo": repo,
    })
}

/// Record an explicit label for the last completed answer. The record contains
/// only route metadata + verdict; no prompt, response, free-form note, or secret.
pub(crate) fn record_route_verdict(
    route: &crate::agent::club::RouteIdentity,
    verdict: RouteVerdict,
    completed_ms: u64,
    workspace: &Path,
) {
    if cfg!(test) || !enabled() {
        return;
    }
    let record = route_verdict_record(
        route,
        verdict,
        completed_ms,
        &repo_value_for(workspace),
        now_secs(),
        std::process::id(),
        next_seq(),
    );
    append_jsonl(&ledger_path(), &record);
}

// ---------------------------------------------------------------------------
// `event` records — individual observations within a turn (subkind `cmd`).
// ---------------------------------------------------------------------------

/// Whether per-command `event` recording is on (`ANGEL_EXPERIENCE_EVENTS`,
/// default **on**; also requires the master [`enabled`] switch).
pub(crate) fn events_enabled() -> bool {
    env_flag("ANGEL_EXPERIENCE_EVENTS", true)
}

/// How a recorded command was interpreted. This is the fact that decides whether
/// its exit status is evidence about the command the agent meant to run, and it
/// is recorded rather than assumed — the bug this type exists to close was a
/// status silently belonging to a *different* process than the one being judged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CmdShell<'a> {
    /// The tool exec'd the program itself, with no shell in between. There is no
    /// pipeline to steal the status, so `exit` is always the command's own.
    Direct,
    /// A shell interpreted the command line. `pipefail` says whether that shell
    /// propagates a failing pipeline stage into the status. Without it a
    /// pipeline reports only its LAST stage's exit — `cargo check 2>&1 | tail
    /// -20` reports `tail`'s 0 on a broken build — and the number is evidence
    /// about nothing.
    Shell { name: &'a str, pipefail: bool },
}

/// The three things a recorded exit status is allowed to mean. The third is the
/// point: it used to be silently laundered into a pass.
pub(crate) const VERDICT_PASS: &str = "pass";
pub(crate) const VERDICT_FAIL: &str = "fail";
pub(crate) const VERDICT_NONE: &str = "no_verdict";

/// `128 + SIGPIPE` — a stage killed by its own consumer closing the pipe.
const SIGPIPE_STATUS: i32 = 128 + 13;

/// Does this command line contain a pipeline?
///
/// Deliberately over-eager: a `|` inside quotes or an `awk` program counts.
/// Over-detection only ever moves a status into `no_verdict`, which costs a
/// piece of evidence; under-detection would let a *stolen* status be recorded as
/// a verdict, which is the failure mode this module exists to prevent. `||` is a
/// control operator, not a pipe.
pub(crate) fn looks_piped(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'|' {
            if bytes.get(i + 1) == Some(&b'|') {
                i += 2; // `||`
                continue;
            }
            return true;
        }
        i += 1;
    }
    false
}

/// What an observed exit status says about the command the agent *meant* to run:
/// `("pass" | "fail" | "no_verdict", reason)`, with a reason only when there is
/// no verdict. Pure, and the sole authority for the `verdict` field.
///
/// The `no_verdict` arms are the whole reason this function exists. Each one is
/// a status that belongs to something other than the command under judgement,
/// and each used to be recorded as an ordinary exit code — which is how `tail`'s
/// 0 became a passing build across the entire ledger.
pub(crate) fn cmd_verdict(exp: &CmdExperience) -> (&'static str, Option<&'static str>) {
    // We killed it; it never reached a verdict of its own.
    if exp.timed_out {
        return (VERDICT_NONE, Some("timed_out"));
    }
    // Died to a signal — the status is the killer's, not the command's.
    let Some(code) = exp.exit else {
        return (VERDICT_NONE, Some("signal"));
    };
    if let CmdShell::Shell { pipefail, .. } = exp.shell
        && looks_piped(exp.text)
    {
        // The original bug. Without `pipefail` a pipeline's status is its
        // last stage's, so this number is `tail`'s / `head`'s / `wc`'s.
        if !pipefail {
            return (VERDICT_NONE, Some("no_pipefail"));
        }
        // `pipefail` faithfully surfaces a stage that was killed by its own
        // consumer closing the pipe (`seq 1 100000 | head -2` → 141). That
        // is a plumbing artifact: the producer was killed before it could
        // reach a verdict, so we do not know whether it would have passed.
        // Reporting it as a failure would just swap one lie for another.
        if code == SIGPIPE_STATUS {
            return (VERDICT_NONE, Some("sigpipe"));
        }
    }
    if code == 0 {
        (VERDICT_PASS, None)
    } else {
        (VERDICT_FAIL, None)
    }
}

// ---------------------------------------------------------------------------
// Prompt contamination — the commands the dossier put in the agent's mouth.
// ---------------------------------------------------------------------------

/// The commands the Repo Dossier **asserted in a prompt this session** — the
/// rituals and traps of its warm-start block ("build: `cargo check`").
///
/// This closes a self-confirming loop that goes live the moment piped exit
/// statuses become real (i.e. with the `pipefail` fix above). The dossier injects
/// ``build: `cargo check` `` into the system prompt; the agent reads it and
/// dutifully runs `cargo check`; the miner counts that run as fresh, independent
/// evidence for the very fact that produced it. Belief climbs on zero new
/// information, and the dossier ends up confidently certain of whatever it
/// happened to say first. It is a machine for manufacturing confident nonsense,
/// and it was harmless until now only because those piped runs carried no verdict
/// to count.
///
/// We cannot know *causally* whether the agent ran the command **because** it was
/// told to, and this does not pretend to. It knows something weaker and provable:
/// the dossier asserted this command in a prompt this session — observed at the
/// assertion site ([`crate::knowledge::dossier::context_block`]), not inferred.
fn asserted_cell() -> &'static Mutex<BTreeMap<String, Vec<String>>> {
    static CELL: OnceLock<Mutex<BTreeMap<String, Vec<String>>>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Note the commands a freshly-injected dossier block asserts. Called once per
/// injection (startup, `/cd`, `--task`); they accumulate rather than replace,
/// because every one of them was in a prompt this session and stays in the
/// transcript the agent is reading from.
pub(crate) fn note_asserted_commands(workspace: &Path, commands: &[String]) {
    if commands.is_empty() {
        return;
    }
    let key = crate::platform::workspace_store::repo_identity(workspace).key;
    if let Ok(mut all) = asserted_cell().lock() {
        let v = all.entry(key).or_default();
        for c in commands {
            let c = c.trim();
            // Capped like the failover sink: a pathological artifact must not
            // grow this without bound.
            if !c.is_empty() && v.len() < 64 && !v.iter().any(|e| e == c) {
                v.push(c.to_string());
            }
        }
    }
}

fn asserted_commands_for(workspace: &Path) -> Vec<String> {
    let key = crate::platform::workspace_store::repo_identity(workspace).key;
    asserted_cell()
        .lock()
        .ok()
        .and_then(|all| all.get(&key).cloned())
        .unwrap_or_default()
}

/// Is `text` **independent** evidence — did it come from somewhere other than the
/// dossier's own mouth? Pure.
///
/// Deliberately conservative: a whitespace-normalized, case-folded *substring*
/// match, so an asserted `cargo check` makes `cd cockpit && cargo check 2>&1 |
/// tail -20` dependent, and so is `cargo check --all-features`. This will mark
/// some genuinely independent runs as dependent, and that asymmetry is the point:
/// under-counting support merely costs time, while inflating a belief out of the
/// system's own echo is unrecoverable.
pub(crate) fn command_is_independent(asserted: &[String], text: &str) -> bool {
    let hay = normalize_ws(text).to_ascii_lowercase();
    !asserted.iter().any(|c| {
        let needle = normalize_ws(c).to_ascii_lowercase();
        !needle.is_empty() && hay.contains(&needle)
    })
}

fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Everything one command execution observed, assembled at the tool wrapper.
pub(crate) struct CmdExperience<'a> {
    /// The tool that ran it ("shell" | "cargo" | "run_tests").
    pub(crate) tool: &'a str,
    /// The raw command text — scrubbed and capped by the record builder, so
    /// callers pass it as-is.
    pub(crate) text: &'a str,
    /// Exit code; `None` when the process died to a signal. Read it **only**
    /// through [`cmd_verdict`]: on its own it does not say whose status it is.
    pub(crate) exit: Option<i32>,
    pub(crate) timed_out: bool,
    pub(crate) dur_ms: u128,
    /// Size of the command's captured output (a cheap "how chatty" signal).
    pub(crate) bytes_out: usize,
    /// What interpreted the command — the provenance that makes `exit`
    /// meaningful (or provably meaningless).
    pub(crate) shell: CmdShell<'a>,
}

/// Serialize one command execution. Pure — the write is in
/// [`record_cmd_event`]; secret-scrubbing and the length cap happen here so no
/// path can record raw text.
///
/// `exit` stays exactly as observed, and `verdict` says what it is worth. A
/// consumer that wants to know whether the code built must read `verdict`: an
/// `exit` of 0 on a `no_verdict` row means "the last thing in the pipeline was
/// fine", not "the build passed". Rows written before schema v3 carry no
/// `verdict` at all, which is how a consumer can tell they are untrustworthy.
///
/// `asserted` is the set of commands the dossier put in this session's prompt
/// (see [`command_is_independent`]); a row matching one of them is the dossier's
/// own echo and is stamped `independent: false` so it cannot vote for the fact
/// that produced it.
pub(crate) fn cmd_event_record(
    exp: &CmdExperience,
    asserted: &[String],
    repo: &serde_json::Value,
    ts: u64,
    session: u32,
    seq: u64,
) -> serde_json::Value {
    let (verdict, reason) = cmd_verdict(exp);
    // Direct exec has no shell and therefore no pipefail question to answer;
    // `null` says that, where `false` would falsely imply a stealable status.
    let (shell, pipefail) = match exp.shell {
        CmdShell::Direct => (None, None),
        CmdShell::Shell { name, pipefail } => (Some(name), Some(pipefail)),
    };
    let independent = command_is_independent(asserted, exp.text);
    serde_json::json!({
        "kind": "event",
        "v": SCHEMA_V,
        "ts": ts,
        "session": session,
        "seq": seq,
        "event": "cmd",
        "repo": repo,
        "cmd": {
            "text": cap_text(&scrub_secrets(exp.text)),
            "exit": exp.exit,
            "verdict": verdict,
            "verdict_reason": reason,
            "shell": shell,
            "pipefail": pipefail,
            // Where the *choice of command* came from, as far as we can honestly
            // tell. "dossier" means the dossier asserted this command in a prompt
            // this session, so it may not be counted as support for its own fact.
            // Mirrors The Cut's `machine.source` so one miner can read both.
            "source": if independent { "agent" } else { "dossier" },
            "independent": independent,
            "timed_out": exp.timed_out,
            "dur_ms": exp.dur_ms as u64,
            "tool": exp.tool,
            "bytes_out": exp.bytes_out,
        },
    })
}

/// Append one command execution to the ledger, stamped with the workspace it
/// ran in (`folder` — the tool's own scope, so worktree shells stamp the
/// worktree). Best-effort and test-silent.
pub(crate) fn record_cmd_event(exp: &CmdExperience, folder: &Path) {
    if cfg!(test) || !enabled() || !events_enabled() {
        return;
    }
    let mut rec = cmd_event_record(
        exp,
        &asserted_commands_for(folder),
        &repo_value_for(folder),
        now_secs(),
        std::process::id(),
        next_seq(),
    );
    if let Some(receipt) = crate::agent::harness::exec::sandbox_receipt() {
        rec["sandbox_profile"] = receipt["sandbox_profile"].clone();
        rec["sandbox"] = receipt;
    }
    append_jsonl(&ledger_path(), &rec);
}

// ---------------------------------------------------------------------------
// `event` records — subkind `skill` (H5 of docs/plans/habitsmith.md): one line
// per `skill(name)` load, so Habitsmith can tell a minted skill is actually
// being USED and flag drift when its workflow's belief decays underneath it.
// ---------------------------------------------------------------------------

/// One skill load observed at the `skill(...)` tool dispatch.
pub(crate) struct SkillExperience<'a> {
    pub(crate) name: &'a str,
    /// Whether the lookup resolved to a known skill.
    pub(crate) ok: bool,
    pub(crate) dur_ms: u128,
}

/// Serialize one skill load. Pure — the write is in [`record_skill_event`].
/// The name rides through the same scrub+cap as command text: it's catalog
/// data today, but no path may record raw text.
pub(crate) fn skill_event_record(
    exp: &SkillExperience,
    repo: &serde_json::Value,
    ts: u64,
    session: u32,
    seq: u64,
) -> serde_json::Value {
    serde_json::json!({
        "kind": "event",
        "v": SCHEMA_V,
        "ts": ts,
        "session": session,
        "seq": seq,
        "event": "skill",
        "repo": repo,
        "skill": {
            "name": cap_text(&scrub_secrets(exp.name)),
            "ok": exp.ok,
            "dur_ms": exp.dur_ms as u64,
        },
    })
}

/// Append one skill load to the ledger, stamped with the workspace the skill
/// catalog was built for. Best-effort and test-silent, exactly
/// [`record_cmd_event`]'s contract (same master + events gates).
pub(crate) fn record_skill_event(exp: &SkillExperience, folder: &Path) {
    if cfg!(test) || !enabled() || !events_enabled() {
        return;
    }
    let rec = skill_event_record(
        exp,
        &repo_value_for(folder),
        now_secs(),
        std::process::id(),
        next_seq(),
    );
    append_jsonl(&ledger_path(), &rec);
}

/// The `path` tag for a single-agent record: the active driver, else "single".
pub(crate) fn driver_path() -> String {
    match std::env::var("ANGEL_DRIVER") {
        Ok(d) if !d.trim().is_empty() => d.trim().to_string(),
        _ => "single".to_string(),
    }
}

// ---------------------------------------------------------------------------
// `moa_turn` records — the SOTA-MOA pipeline.
// ---------------------------------------------------------------------------

/// Judge-stage facts available at the pipeline's record site without threading
/// raw scores through `judge_rank`'s return type (a follow-on enrichment).
#[derive(Clone, Copy, Debug)]
pub(crate) struct JudgeFacts {
    pub(crate) panel: usize,
    pub(crate) kept: usize,
}

/// Verify-stage facts. `passed` is `None` until `verify_revise` reports its
/// verdict (today it returns only the revised text).
#[derive(Clone, Copy, Debug)]
pub(crate) struct VerifyFacts {
    pub(crate) rounds: usize,
    pub(crate) passed: Option<bool>,
}

/// Everything one MoA pipeline turn learned, built from its `TurnTrace`.
pub(crate) struct MoaExperience<'a> {
    pub(crate) driver: &'a str,
    /// "direct" | "deliberate" | "fallback" (the pipeline route).
    pub(crate) route: &'a str,
    pub(crate) ok: bool,
    pub(crate) latency_ms: u128,
    pub(crate) dissent: Option<f64>,
    pub(crate) gate: Option<&'a str>,
    pub(crate) proposed: usize,
    pub(crate) kept: usize,
    /// Effective knobs after gating: (layers, judge, verify, samples).
    pub(crate) effective: Option<(usize, bool, usize, usize)>,
    /// The engaged formation's name (the FormationId name), when a formation
    /// owned the turn — the per-formation cost attribution key.
    pub(crate) formation: Option<String>,
    pub(crate) judge: Option<JudgeFacts>,
    pub(crate) verify: Option<VerifyFacts>,
    pub(crate) tokens: Vec<(String, u64, u64)>,
    pub(crate) failovers: Vec<Failover>,
}

/// Serialize one finished MoA pipeline turn. Pure — write is in [`record_moa`].
pub(crate) fn moa_record(
    exp: &MoaExperience,
    cfg: &serde_json::Value,
    repo: &serde_json::Value,
    ts: u64,
    session: u32,
    seq: u64,
) -> serde_json::Value {
    let effective = exp.effective.map(|(layers, judge, verify, samples)| {
        serde_json::json!({
            "layers": layers, "judge": judge, "verify": verify, "samples": samples,
        })
    });
    let judge = exp
        .judge
        .map(|j| serde_json::json!({ "panel": j.panel, "kept": j.kept }));
    let verify = exp
        .verify
        .map(|v| serde_json::json!({ "rounds": v.rounds, "passed": v.passed }));
    serde_json::json!({
        "kind": "moa_turn",
        "v": SCHEMA_V,
        "ts": ts,
        "session": session,
        "seq": seq,
        "path": "sota-moa",
        "driver": exp.driver,
        "cfg": cfg,
        "repo": repo,
        "outcome": {
            "ok": exp.ok,
            "route": exp.route,
            "formation": exp.formation,
            "latency_ms": exp.latency_ms as u64,
            "dissent": exp.dissent,
            "gate": exp.gate,
            "drafts": { "proposed": exp.proposed, "kept": exp.kept },
            "knobs": effective,
            "judge": judge,
            "verify": verify,
            "tokens": tokens_json(&exp.tokens),
            "failovers": failovers_json(&exp.failovers),
        },
    })
}

/// Append one MoA pipeline turn to the experience ledger. Best-effort and
/// test-silent. Independent of the legacy `~/.angelX/moa/ledger.jsonl` writer,
/// which stays byte-identical.
pub(crate) fn record_moa(exp: &MoaExperience) {
    if cfg!(test) || !enabled() {
        return;
    }
    let Some(repo) = current_turn_repo_value() else {
        return;
    };
    let rec = moa_record(
        exp,
        &cfg_value(),
        &repo,
        now_secs(),
        std::process::id(),
        next_seq(),
    );
    append_jsonl(&ledger_path(), &rec);
}

// ---------------------------------------------------------------------------
// Shared plumbing: sequence counter, clock, rotation, append.
// ---------------------------------------------------------------------------

/// A per-process monotonic sequence number so rows from one launch order.
fn next_seq() -> u64 {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    SEQ.fetch_add(1, Ordering::Relaxed)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Where a full ledger rotates to: `<stem>-<ts>.jsonl` beside the original.
#[cfg(test)]
fn rotated_path(path: &Path, ts: u64) -> PathBuf {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("ledger");
    path.with_file_name(format!("{stem}-{ts}.jsonl"))
}

/// Rename the active ledger aside if it has reached [`ROTATE_BYTES`].
#[cfg(test)]
fn maybe_rotate(path: &Path) {
    if let Ok((directory, name)) = crate::platform::workspace_store::private_io::parent(path) {
        let rotated = rotated_path(path, now_secs());
        if let Some(rotated_name) = rotated.file_name() {
            let _ = directory.rotate_if(name, rotated_name, ROTATE_BYTES);
        }
    }
}

struct JsonlLock {
    file: std::fs::File,
    directory: crate::platform::workspace_store::private_io::PrivateDirectory,
}

impl Drop for JsonlLock {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        // SAFETY: the descriptor is owned by this guard and remains valid for
        // the syscall. Closing it immediately after Drop would also unlock it.
        unsafe {
            libc::flock(self.file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

fn jsonl_lock_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("ledger.jsonl");
    path.with_file_name(format!("{name}.lock"))
}

fn acquire_jsonl_lock(path: &Path) -> std::io::Result<JsonlLock> {
    let (directory, _) = crate::platform::workspace_store::private_io::parent(path)?;
    let lock_path = jsonl_lock_path(path);
    let file = directory.lock_file(lock_path.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "ledger lock has no file name",
        )
    })?)?;
    crate::platform::workspace_store::lock_store(&file, "experience_lock")?;
    Ok(JsonlLock { file, directory })
}

/// Append one compact JSON value as one JSONL record. A stable companion flock
/// covers rotation and the append across processes; serializing before opening
/// the ledger also makes the record one `write_all` instead of interleavable
/// formatter fragments. Best-effort by design: telemetry never fails a turn.
pub(crate) fn append_jsonl(path: &Path, value: &serde_json::Value) {
    if let Err(error) = append_jsonl_bounded(path, value, ledger_cap(path)) {
        STORE_FAILURES.fetch_add(1, Ordering::Relaxed);
        // Never recurse into this broken ledger to report its own failure.
        eprintln!(
            "[memory-health] experience class=Environment io={:?}",
            error.kind()
        );
    }
}

fn ledger_cap(path: &Path) -> u64 {
    let store = if path
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name == "lessons")
    {
        "atlas_lessons"
    } else {
        "experience"
    };
    crate::platform::store_caps::value(store, "max_bytes")
}

static STORE_FAILURES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub(crate) fn take_store_failures() -> usize {
    STORE_FAILURES.swap(0, Ordering::Relaxed)
}

fn append_jsonl_bounded(path: &Path, value: &serde_json::Value, cap: u64) -> std::io::Result<()> {
    use std::io::{Read, Seek, SeekFrom};
    let mut line =
        crate::platform::secrets::to_redacted_vec(value).map_err(std::io::Error::other)?;
    line.push(b'\n');
    if line.len() as u64 > cap {
        return Err(std::io::Error::from(std::io::ErrorKind::FileTooLarge));
    }
    let lock = acquire_jsonl_lock(path)?;
    let name = path
        .file_name()
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    if let Some(mut file) = lock.directory.existing(name)? {
        let len = file.metadata()?.len();
        if len.saturating_add(line.len() as u64) > cap {
            // Amortize compaction: retain at most half the ceiling. Skip the
            // leading partial row, keep complete newest JSONL records only.
            let keep = (cap / 2).min(cap - line.len() as u64);
            let start = len.saturating_sub(keep);
            file.seek(SeekFrom::Start(start.saturating_sub(1)))?;
            let mut tail = Vec::new();
            file.take(keep + 1).read_to_end(&mut tail)?;
            if start > 0 {
                let drop = tail
                    .iter()
                    .position(|b| *b == b'\n')
                    .map_or(tail.len(), |i| i + 1);
                tail.drain(..drop);
            }
            if !tail.is_empty() && !tail.ends_with(b"\n") {
                return Err(std::io::Error::from(std::io::ErrorKind::InvalidData));
            }
            for row in tail.split(|b| *b == b'\n').filter(|r| !r.is_empty()) {
                serde_json::from_slice::<serde_json::Value>(row).map_err(std::io::Error::other)?;
            }
            tail.extend_from_slice(&line);
            return lock.directory.replace(name, &tail);
        }
    }
    lock.directory.append(name)?.write_all(&line)
}

/// Remove a ledger under the same companion lock used by writers, so `/moa
/// clear` cannot race an append into a renamed/unlinked inode.
#[cfg(test)]
pub(crate) fn clear_jsonl(path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let _lock = acquire_jsonl_lock(path)?;
    std::fs::remove_file(path)
}

/// Atomically replace a JSONL ledger under the same companion lock used by
/// appenders. Used by project-filtered clears that must preserve foreign rows.
pub(crate) fn replace_jsonl(path: &Path, values: &[serde_json::Value]) -> std::io::Result<()> {
    let lock = acquire_jsonl_lock(path)?;
    let mut body = Vec::new();
    for value in values {
        body.extend(
            crate::platform::secrets::to_redacted_vec(value).map_err(std::io::Error::other)?,
        );
        body.push(b'\n');
    }
    if body.len() as u64 > ledger_cap(path) {
        return Err(std::io::Error::from(std::io::ErrorKind::FileTooLarge));
    }
    lock.directory.replace(
        path.file_name().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "ledger has no file name")
        })?,
        &body,
    )
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/experience__tests.rs"]
mod tests;
