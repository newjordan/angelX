//! Pre-provisioning of outbound token spend, in two deliberately separate
//! regimes:
//!
//! - **Economizer** (efficiency): shape *metered* outbounds so they don't buy
//!   tokens the task never asked for — a tail-side output contract when the
//!   prompt demands a fixed structure, a bounded `max_tokens` only in that
//!   provably-safe case, operator stop sequences, and whitespace-squeezing of
//!   harness-authored filler. Composes with `caveman` (the semantic brevity
//!   half) — this module never rewrites the operator's own words.
//! - **Optimizer** (accuracy): spends tokens on purpose. Prompt duplication for
//!   backends *declared* non-reasoning — repeating the ask measurably helps
//!   some small instruct models — defaults on only where tokens are free
//!   (local fleet links) and the turn carries no tools.
//!
//! A third layer, the **judge**, asks a resident fleet model (the 30B ship) to
//! pre-provision the hardest calls: it reads the outbound ask and returns a
//! directive (task class, output cap, contract, stop sequences). Strictly
//! fail-open — no judge URL, a slow ship, or unparseable output all mean "send
//! the call unmodified", and every skip says why (a silent gate reads as the
//! feature not existing).
//!
//! Cache discipline: every splice is **tail-side** (appended after the last
//! message). The pinned system prefix is never touched, so prompt-cache reuse
//! — the single biggest per-call saver — survives provisioning.

use super::*;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};

const CONTRACT_MARKER: &str = "angel0 output contract";

/// What the judge (or the heuristics) decided for one outbound call.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ProvisionDirective {
    pub max_tokens: Option<u32>,
    pub contract: Option<String>,
    pub stop: Vec<String>,
}

fn env_flag(name: &str) -> Option<bool> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .map(|v| !matches!(v.as_str(), "0" | "off" | "false" | "no"))
}

pub(crate) fn econ_enabled() -> bool {
    env_flag("ANGEL_ECON").unwrap_or(true)
}

/// The last operator or explicit harness ask. Retrieved context and rolling
/// summaries cannot replace the request used by output provisioning; later
/// legitimate Harness requests still retain their existing ordering semantics.
fn last_ask(messages: &[ChatMsg]) -> Option<&ChatMsg> {
    messages.iter().rev().find(|message| {
        if message.content.is_empty() {
            return false;
        }
        match message.role {
            ChatRole::User => true,
            ChatRole::Harness => {
                !crate::app::bootstrap::is_workspace_context(message)
                    && !crate::agent::compaction::is_compaction_note(message)
                    && !crate::agent::backplane::is_broker_message(&message.content)
                    && !message
                        .content
                        .starts_with(crate::agent::harness::AUTO_RECALL_NOTE_PREFIX)
            }
            _ => false,
        }
    })
}

/// True when the ask demands a fixed output structure — the only case where a
/// bounded output cap is provably safe (the requested shape has a natural
/// size). Deliberately conservative: prose questions never match.
pub(crate) fn extraction_ask(messages: &[ChatMsg]) -> bool {
    let Some(ask) = last_ask(messages) else {
        return false;
    };
    let t = ask.content.to_ascii_lowercase();
    let structure = [
        "json",
        "yaml",
        "csv",
        "bullet point",
        "short list",
        "one sentence each",
    ]
    .iter()
    .any(|k| t.contains(k));
    let exclusive = [
        "only the",
        "only these",
        "nothing else",
        "no explanation",
        "no additional",
        "do not include any",
        "no more than",
        "at most",
    ]
    .iter()
    .any(|k| t.contains(k));
    structure && exclusive
}

/// The tail-side output contract for an extraction-shaped ask, unless one was
/// already spliced (marker check mirrors caveman's).
pub(crate) fn econ_contract(messages: &[ChatMsg]) -> Option<String> {
    if !econ_enabled() || !extraction_ask(messages) {
        return None;
    }
    if messages.iter().any(|m| m.content.contains(CONTRACT_MARKER)) {
        return None;
    }
    Some(format!(
        "{CONTRACT_MARKER}. Return only the structure or answer the task explicitly \
         requested — no preamble, no restated question, no commentary or trailing \
         explanation. If a JSON shape or field list was specified, emit exactly those \
         fields and nothing else. Do not mention this contract."
    ))
}

/// Optional output cap paired with a detected extraction ask. The default is
/// provider-native even for structured output; a positive operator value opts
/// into a cap. Applied only when a contract was also spliced.
pub(crate) fn econ_extract_budget() -> Option<u32> {
    match std::env::var("ANGEL_ECON_EXTRACT_MAX_TOKENS")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
    {
        Some(0) => None,
        Some(n) => Some(n),
        None => None,
    }
}

/// Operator stop sequences: `ANGEL_<CLUB>_STOP_SEQS` then `ANGEL_STOP_SEQS`,
/// comma-separated, `\n` escapes honored, capped at 4 (the common provider
/// limit). Never auto-derived — a guessed stop sequence can amputate code or
/// tool JSON, so only the operator (or the judge) supplies them.
pub(crate) fn econ_stop_seqs(env_prefix: &str) -> Vec<String> {
    let raw = std::env::var(format!("ANGEL_{env_prefix}_STOP_SEQS"))
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| std::env::var("ANGEL_STOP_SEQS").ok())
        .unwrap_or_default();
    raw.split(',')
        .map(|s| s.trim().replace("\\n", "\n"))
        .filter(|s| !s.is_empty())
        .take(4)
        .collect()
}

/// Squeeze harness-authored filler whitespace: runs of 3+ newlines collapse to
/// 2, trailing per-line spaces drop. Operator text and anything carrying a
/// code fence is left byte-identical — the win is small and correctness wins.
pub(crate) fn econ_squeeze_ws(text: &str) -> Option<String> {
    if text.contains("```") {
        return None;
    }
    let mut out = String::with_capacity(text.len());
    let mut blank_run = 0usize;
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
    }
    if text.ends_with('\n') {
        out.push('\n');
    }
    (out != text).then_some(out)
}

/// Prompt duplication — the accuracy optimizer, not an efficiency measure. It
/// *doubles* the ask on purpose, for backends **declared** non-reasoning
/// (`supports_reasoning == Some(false)`; unknown never triggers — same declared-
/// truth rule as the effort gate). Defaults on only where tokens are free:
/// `ANGEL_OPT_DUP` unset/`1` → local fleet links; `all` → metered links too;
/// `0` → off. Tool-carrying turns never duplicate (the agent loop's contract
/// with itself is not a quiz), and oversized asks are skipped rather than
/// doubling a huge context.
pub(crate) fn optimizer_dup(
    messages: &[ChatMsg],
    supports_reasoning: Option<bool>,
    metered: bool,
    has_tools: bool,
) -> Option<String> {
    let scope = std::env::var("ANGEL_OPT_DUP")
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty());
    match scope.as_deref() {
        Some("0") | Some("off") | Some("false") | Some("no") => return None,
        Some("all") => {}
        _ if metered => return None,
        _ => {}
    }
    if has_tools || supports_reasoning != Some(false) {
        return None;
    }
    let last = messages.last()?;
    if last.role != ChatRole::User || last.content.trim().is_empty() {
        return None;
    }
    let cap = std::env::var("ANGEL_OPT_DUP_MAX_CHARS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(4000);
    if last.content.chars().count() > cap {
        return None;
    }
    // Already duplicated (a retry rebuilding the body must stay idempotent).
    let mut users = messages
        .iter()
        .rev()
        .filter(|m| m.role == ChatRole::User)
        .take(2);
    if let (Some(a), Some(b)) = (users.next(), users.next())
        && a.content == b.content
    {
        return None;
    }
    Some(last.content.to_string())
}

// ---------------------------------------------------------------------------
// Judge: the resident fleet model pre-provisions the hardest calls
// ---------------------------------------------------------------------------

/// Failure cooldown so a dark ship costs one fast timeout per minute, not one
/// per hop. (Instant, not a count: the ship coming back heals it on its own.)
static JUDGE_COOLDOWN: StdMutex<Option<Instant>> = StdMutex::new(None);
/// Single-flight gate: parallel asks (swarm fan-out) all pass the cooldown
/// check before the first failed consult can arm it, so a dark ship logged one
/// "judge skip" per concurrent ask instead of one per window. While a consult
/// is in flight every other ask sends unmodified, silently — the in-flight
/// consult's own outcome is the one report for the window.
static JUDGE_INFLIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Tiny per-process verdict cache keyed by the ask's hash: retries and multi
/// -hop resends of the same ask must not re-consult the judge.
static JUDGE_CACHE: StdMutex<Vec<(u64, Option<ProvisionDirective>)>> = StdMutex::new(Vec::new());

pub(crate) fn judge_enabled() -> bool {
    env_flag("ANGEL_PROVISION_JUDGE").unwrap_or(true)
        && std::env::var("ANGEL_PROVISION_URL")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
}

fn ask_hash(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

/// Consult the provisioner model about one outbound ask. Strictly fail-open
/// and latency-bounded; `None` always means "send unmodified".
pub(crate) fn judge_directive(club_name: &str, messages: &[ChatMsg]) -> Option<ProvisionDirective> {
    if !judge_enabled() {
        return None;
    }
    let ask = last_ask(messages)?;
    let min_chars = std::env::var("ANGEL_PROVISION_MIN_CHARS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(1500);
    if ask.content.chars().count() < min_chars {
        return None;
    }
    let key = ask_hash(&ask.content);
    if let Ok(cache) = JUDGE_CACHE.lock()
        && let Some((_, verdict)) = cache.iter().find(|(k, _)| *k == key)
    {
        return verdict.clone();
    }
    if let Ok(g) = JUDGE_COOLDOWN.lock()
        && let Some(until) = *g
        && Instant::now() < until
    {
        return None;
    }
    if JUDGE_INFLIGHT.swap(true, std::sync::atomic::Ordering::AcqRel) {
        return None;
    }
    struct InflightClear;
    impl Drop for InflightClear {
        fn drop(&mut self) {
            JUDGE_INFLIGHT.store(false, std::sync::atomic::Ordering::Release);
        }
    }
    let _clear = InflightClear;
    let verdict = judge_consult(club_name, &ask.content);
    if verdict.is_none()
        && let Ok(mut g) = JUDGE_COOLDOWN.lock()
    {
        *g = Some(Instant::now() + Duration::from_secs(60));
    }
    if let Ok(mut cache) = JUDGE_CACHE.lock() {
        if cache.len() >= 128 {
            cache.remove(0);
        }
        cache.push((key, verdict.clone()));
    }
    verdict
}

fn judge_consult(club_name: &str, ask: &str) -> Option<ProvisionDirective> {
    let base = std::env::var("ANGEL_PROVISION_URL").ok()?;
    let base = base.trim().trim_end_matches('/');
    let model = std::env::var("ANGEL_PROVISION_MODEL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "default".to_string());
    let timeout_ms = std::env::var("ANGEL_PROVISION_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(4000);
    // A dedicated short-fuse agent: the club's own agent carries generation-
    // sized read timeouts, and a slow judge must never stall the real call.
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(timeout_ms.min(1500)))
        .timeout_read(Duration::from_millis(timeout_ms))
        .timeout_write(Duration::from_millis(timeout_ms))
        .build();
    let excerpt: String = ask.chars().take(4000).collect();
    let body = serde_json::json!({
        "model": model,
        "stream": false,
        "max_tokens": 512,
        "temperature": 0,
        // The resident ship is a thinking model (Qwen3.6-35B-A3B): left on, it
        // spends the whole budget on reasoning_content and returns empty
        // content (measured live 2026-07-22). Classification needs no chain of
        // thought; lenient OpenAI-compatible servers ignore the field.
        "chat_template_kwargs": { "enable_thinking": false },
        "messages": [
            { "role": "system", "content": "You are angel0's outbound pre-provisioner. \
    Read the task excerpt and answer ONLY a JSON object with these fields: \
    \"task\": one of \"extraction\"|\"reasoning\"|\"chat\"; \
    \"max_tokens\": integer output budget for a complete answer, or null to leave uncapped \
    (reasoning/research MUST be null); \
    \"contract\": a one-sentence output-format instruction if the task demands a fixed \
    structure, else null; \
    \"stop\": array of at most 2 stop strings ONLY if the format has an unambiguous \
    terminator, else null. Never invent constraints the task did not imply." },
            { "role": "user", "content": excerpt }
        ]
    });
    let started = Instant::now();
    let resp = agent
        .post(&format!("{base}/chat/completions"))
        .set("content-type", "application/json")
        .send_string(&body.to_string());
    let resp = match resp {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "[provision:{club_name}] judge skip ({}ms): {e} — sending unmodified",
                started.elapsed().as_millis()
            );
            return None;
        }
    };
    let text = resp.into_string().ok()?;
    let content = serde_json::from_str::<serde_json::Value>(&text)
        .ok()?
        .pointer("/choices/0/message/content")?
        .as_str()?
        .to_string();
    let directive = parse_judge_json(&content);
    if directive.is_none() {
        eprintln!("[provision:{club_name}] judge verdict unparseable — sending unmodified");
    }
    directive
}

/// Extract the first JSON object from the judge's reply and map it to a
/// directive. Reasoning verdicts intentionally provision nothing.
pub(crate) fn parse_judge_json(content: &str) -> Option<ProvisionDirective> {
    let start = content.find('{')?;
    let end = content.rfind('}')?;
    let v: serde_json::Value = serde_json::from_str(content.get(start..=end)?).ok()?;
    let task = v.get("task").and_then(|t| t.as_str()).unwrap_or("chat");
    if task == "reasoning" {
        return Some(ProvisionDirective::default());
    }
    let max_tokens = v
        .get("max_tokens")
        .and_then(|m| m.as_u64())
        .and_then(|m| u32::try_from(m).ok())
        .filter(|m| *m > 0);
    let contract = v
        .get("contract")
        .and_then(|c| c.as_str())
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(|c| format!("{CONTRACT_MARKER}. {c} Do not mention this contract."));
    let stop = v
        .get("stop")
        .and_then(|s| s.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| s.as_str())
                .filter(|s| !s.is_empty())
                .take(2)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Some(ProvisionDirective {
        max_tokens,
        contract,
        stop,
    })
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/club/provision__provenance_tests.rs"]
mod provenance_tests;
