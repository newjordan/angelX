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

const SCHEMA_V: u64 = 3;

#[cfg(test)]
const ROTATE_BYTES: u64 = 50 * 1024 * 1024;

pub(crate) fn enabled() -> bool {
    env_flag("ANGEL_EXPERIENCE", true)
}

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

pub(crate) fn ledger_path() -> PathBuf {
    match std::env::var("ANGEL_EXPERIENCE_LOG") {
        Ok(p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => crate::platform::workspace_store::angel_subdir("experience").join("ledger.jsonl"),
    }
}

pub(crate) const KNOB_CATALOG: &[&str] = &[
    "ANGEL_SOTA_MOA_PROPOSE_CLUB",
    "ANGEL_SOTA_MOA_JUDGE_CLUB",
    "ANGEL_SOTA_MOA_VERIFY_CLUB",
    "ANGEL_SOTA_MOA_AGG_CLUB",
    "ANGEL_SOTA_MOA_ALWAYS",
    "ANGEL_SOTA_MOA_JUDGE",
    "ANGEL_SOTA_MOA_VERIFY",
    "ANGEL_SOTA_CAVEMAN",
    "ANGEL_MOA_JUDGE_FANOUT",
    "ANGEL_SOTA_MOA_WIDTH",
    "ANGEL_SOTA_MOA_MAX_WIDTH",
    "ANGEL_SOTA_MOA_LAYERS",
    "ANGEL_SOTA_MOA_SAMPLES",
    "ANGEL_SOTA_MOA_JUDGE_PANEL",
    "ANGEL_SOTA_MOA_REFLECT",
    "ANGEL_SOTA_MOA_GROK_RESEARCH",
    "ANGEL_PXPIPE",
    "ANGEL_DRIVER",
    "ANGEL_YOLO",
    "ANGEL_YOLO_SMART",
    "ANGEL_SPIN_LIMIT",
    "ANGEL_POLL_REPEAT_LIMIT",
    "ANGEL_MAX_HOPS",
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

pub(crate) use angel_live::host::secrets::is_secret_name;

pub(crate) fn snapshot_knobs() -> BTreeMap<String, String> {
    snapshot_from(KNOB_CATALOG, |k| std::env::var(k).ok())
}

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

pub(crate) fn cfg_hash(knobs: &BTreeMap<String, String>) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (k, v) in knobs {
        k.hash(&mut h);
        v.hash(&mut h);
    }
    format!("{:016x}", h.finish())
}

pub(crate) fn cfg_value() -> serde_json::Value {
    let knobs = snapshot_knobs();
    serde_json::json!({ "hash": cfg_hash(&knobs), "knobs": knobs })
}

thread_local! {
    static TURN_WORKSPACE: std::cell::RefCell<Option<PathBuf>> = const {
        std::cell::RefCell::new(None)
    };
}

pub(crate) fn note_turn_workspace(workspace: &Path) {
    TURN_WORKSPACE.with(|cell| *cell.borrow_mut() = Some(workspace.to_path_buf()));
}

pub(crate) fn current_turn_repo_value() -> Option<serde_json::Value> {
    TURN_WORKSPACE.with(|cell| cell.borrow().as_deref().map(repo_value_for))
}

pub(crate) fn current_turn_workspace() -> Option<PathBuf> {
    TURN_WORKSPACE.with(|cell| cell.borrow().clone())
}

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

const CMD_TEXT_MAX: usize = 400;

const SECRET_MARKERS: [&str; 7] = [
    "key", "token", "secret", "auth", "password", "passwd", "bearer",
];

fn is_marker_word(word: &str) -> bool {
    let low = word.to_ascii_lowercase();
    SECRET_MARKERS.iter().any(|m| low.contains(m))
}

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
                Some((sep, v)) if !v.is_empty() => out.push(format!("{name}{sep}…")),
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

fn mask_url_userinfo(tok: &str) -> String {
    if let Some(scheme_end) = tok.find("://") {
        let rest = &tok[scheme_end + 3..];
        if let Some(at) = rest.find('@') {
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

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct TurnCounters {
    pub(crate) markup_replies: usize,
    pub(crate) spin: usize,
    pub(crate) err_streak: usize,
    pub(crate) duplicate_inspection_results: usize,
    pub(crate) duplicate_inspection_bytes_saved: u64,
    pub(crate) aged_inspection_results: usize,
    pub(crate) aged_inspection_bytes_saved: u64,
    pub(crate) eager_offload_results: usize,
    pub(crate) eager_offload_bytes_saved: u64,
    pub(crate) post_edit_diagnostic_attempts: usize,
    pub(crate) post_edit_diagnostic_findings: usize,
    pub(crate) post_edit_diagnostic_failures: usize,
    pub(crate) post_edit_diagnostic_paths_skipped: usize,
    pub(crate) post_edit_diagnostic_output_bytes: u64,
    pub(crate) post_edit_diagnostic_elapsed_ms: u64,
    pub(crate) tool_argument_shrinks: usize,
    pub(crate) tool_argument_strings_shrunk: usize,
    pub(crate) tool_argument_bytes_saved: u64,
    pub(crate) repeated_inspections: usize,
    pub(crate) code_mode_calls: usize,
    pub(crate) code_mode_nested_calls: usize,
    pub(crate) code_mode_nested_output_bytes: u64,
    pub(crate) code_mode_policy_rejections: usize,
    pub(crate) code_mode_recipe_calls: usize,
    pub(crate) task_recon_context_bytes: usize,
    pub(crate) code_mode_schema_tokens: usize,
    pub(crate) request_overflows: usize,
    pub(crate) cache_control_requests: u64,
    pub(crate) cache_read_input_tokens: u64,
    pub(crate) cache_write_input_tokens: u64,
    pub(crate) cache_read_accounting_responses: u64,
    pub(crate) cache_write_accounting_responses: u64,
    pub(crate) unverified_completion_claims: usize,
    pub(crate) discovered_tool_schema_failures: usize,
    pub(crate) skill_hints: usize,
    pub(crate) provider_truncation_retries: u64,
    pub(crate) provider_truncation_episodes: u64,
    pub(crate) provider_truncation_retained_partials: u64,
    pub(crate) provider_truncation_recoveries: u64,
    pub(crate) provider_truncation_failures: u64,
    pub(crate) action_operations: usize,
    pub(crate) action_previews: usize,
    pub(crate) action_denied: usize,
    pub(crate) action_preflight_us: u64,
    pub(crate) action_approval_wait_ms: u64,
    pub(crate) action_exec_ms: u64,
    pub(crate) tool_errors_by_class: ToolErrorClasses,
}

pub(crate) struct TurnExperience<'a> {
    pub(crate) path: &'a str,
    pub(crate) driver: &'a str,
    pub(crate) model: Option<&'a str>,
    pub(crate) reasoning_effort: Option<&'a str>,
    pub(crate) ok: bool,
    pub(crate) stop: &'a str,
    pub(crate) latency_ms: u128,
    pub(crate) hops: usize,
    pub(crate) time_to_first_mutation_ms: Option<u64>,
    pub(crate) time_to_green_ms: Option<u64>,
    pub(crate) system_prompt_tokens: usize,
    pub(crate) project_doc_bytes: usize,
    pub(crate) tool_schema_count: usize,
    pub(crate) tool_schema_tokens: usize,
    pub(crate) tool_schema_peak_count: usize,
    pub(crate) tool_schema_peak_tokens: usize,
    pub(crate) tool_schema_token_requests: usize,
    pub(crate) counters: TurnCounters,
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

pub(crate) fn turn_record(
    exp: &TurnExperience,
    cfg: &serde_json::Value,
    repo: &serde_json::Value,
    ts: u64,
    session: u32,
    seq: u64,
) -> serde_json::Value {
    let mut counters = serde_json::json!({
        "markup_replies": exp.counters.markup_replies,
        "spin": exp.counters.spin,
        "err_streak": exp.counters.err_streak,
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
        "discovered_tool_schema_failures": exp.counters.discovered_tool_schema_failures,
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
            "hiq": hiq_outcome_json(&exp.counters),
            "failovers": failovers_json(&exp.failovers),
        },
    })
}

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

pub(crate) fn hiq_outcome_json(counters: &TurnCounters) -> serde_json::Value {
    let eager = counters.eager_offload_results;
    let aged = counters.aged_inspection_results;
    let dup = counters.duplicate_inspection_results;
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
    let wmax = std::env::var("FORGE_HIQ_WEIGHT_MAX")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(3.0)
        .clamp(1.0, 5.0);
    hiq_priority = hiq_priority.clamp(0.5, wmax);
    let root = crate::agent::harness::last_root_hiq();
    let (root_offload, root_hiq) = match root {
        Some(h) => (Some(h.offload_ratio), Some(h.hiq_priority)),
        None => (None, None),
    };
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
    serde_json::Value::Object(map)
}

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

pub(crate) fn events_enabled() -> bool {
    env_flag("ANGEL_EXPERIENCE_EVENTS", true)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CmdShell<'a> {
    Direct,
    Shell { name: &'a str, pipefail: bool },
}

pub(crate) const VERDICT_PASS: &str = "pass";
pub(crate) const VERDICT_FAIL: &str = "fail";
pub(crate) const VERDICT_NONE: &str = "no_verdict";

const SIGPIPE_STATUS: i32 = 128 + 13;

pub(crate) fn looks_piped(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'|' {
            if bytes.get(i + 1) == Some(&b'|') {
                i += 2;
                continue;
            }
            return true;
        }
        i += 1;
    }
    false
}

pub(crate) fn cmd_verdict(exp: &CmdExperience) -> (&'static str, Option<&'static str>) {
    if exp.timed_out {
        return (VERDICT_NONE, Some("timed_out"));
    }
    let Some(code) = exp.exit else {
        return (VERDICT_NONE, Some("signal"));
    };
    if let CmdShell::Shell { pipefail, .. } = exp.shell
        && looks_piped(exp.text)
    {
        if !pipefail {
            return (VERDICT_NONE, Some("no_pipefail"));
        }
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

fn asserted_cell() -> &'static Mutex<BTreeMap<String, Vec<String>>> {
    static CELL: OnceLock<Mutex<BTreeMap<String, Vec<String>>>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub(crate) fn note_asserted_commands(workspace: &Path, commands: &[String]) {
    if commands.is_empty() {
        return;
    }
    let key = crate::platform::workspace_store::repo_identity(workspace).key;
    if let Ok(mut all) = asserted_cell().lock() {
        let v = all.entry(key).or_default();
        for c in commands {
            let c = c.trim();
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

pub(crate) struct CmdExperience<'a> {
    pub(crate) tool: &'a str,
    pub(crate) text: &'a str,
    pub(crate) exit: Option<i32>,
    pub(crate) timed_out: bool,
    pub(crate) dur_ms: u128,
    pub(crate) bytes_out: usize,
    pub(crate) shell: CmdShell<'a>,
}

pub(crate) fn cmd_event_record(
    exp: &CmdExperience,
    asserted: &[String],
    repo: &serde_json::Value,
    ts: u64,
    session: u32,
    seq: u64,
) -> serde_json::Value {
    let (verdict, reason) = cmd_verdict(exp);
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
            "source": if independent { "agent" } else { "dossier" },
            "independent": independent,
            "timed_out": exp.timed_out,
            "dur_ms": exp.dur_ms as u64,
            "tool": exp.tool,
            "bytes_out": exp.bytes_out,
        },
    })
}

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

pub(crate) struct SkillExperience<'a> {
    pub(crate) name: &'a str,
    pub(crate) ok: bool,
    pub(crate) dur_ms: u128,
}

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

pub(crate) fn driver_path() -> String {
    match std::env::var("ANGEL_DRIVER") {
        Ok(d) if !d.trim().is_empty() => d.trim().to_string(),
        _ => "single".to_string(),
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct JudgeFacts {
    pub(crate) panel: usize,
    pub(crate) kept: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct VerifyFacts {
    pub(crate) rounds: usize,
    pub(crate) passed: Option<bool>,
}

pub(crate) struct MoaExperience<'a> {
    pub(crate) driver: &'a str,
    pub(crate) route: &'a str,
    pub(crate) ok: bool,
    pub(crate) latency_ms: u128,
    pub(crate) dissent: Option<f64>,
    pub(crate) gate: Option<&'a str>,
    pub(crate) proposed: usize,
    pub(crate) kept: usize,
    pub(crate) effective: Option<(usize, bool, usize, usize)>,
    pub(crate) formation: Option<String>,
    pub(crate) judge: Option<JudgeFacts>,
    pub(crate) verify: Option<VerifyFacts>,
    pub(crate) tokens: Vec<(String, u64, u64)>,
    pub(crate) failovers: Vec<Failover>,
}

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

#[cfg(test)]
fn rotated_path(path: &Path, ts: u64) -> PathBuf {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("ledger");
    path.with_file_name(format!("{stem}-{ts}.jsonl"))
}

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

pub(crate) fn append_jsonl(path: &Path, value: &serde_json::Value) {
    if let Err(error) = append_jsonl_bounded(path, value, ledger_cap(path)) {
        STORE_FAILURES.fetch_add(1, Ordering::Relaxed);
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

#[cfg(test)]
pub(crate) fn clear_jsonl(path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let _lock = acquire_jsonl_lock(path)?;
    std::fs::remove_file(path)
}

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
