//! Repo Dossier warm-start injection from compiled experience-ledger facts.
//!
//! The dossier compiler (`scripts/runtime/repo-dossier.mjs`) mines the experience
//! ledger's per-command events into per-repo facts with beliefs and writes one
//! artifact per workspace to `~/.angel0/dossier/<workspace_key>.json`. This
//! module renders that artifact into a small context block so a session opens
//! already knowing the repo's verified rituals ("`cargo test` is the test
//! command and passes"), standing traps ("`npm test` always fails here"), and
//! where the last session left off.
//!
//! Confidence-weighted by construction: facts below the belief threshold are
//! *withheld* (and counted), not asserted — a stale fact decays out of the
//! block and becomes a probe target (D4) instead of a wrong instruction. The
//! block is injected at the two existing workspace-context points: the startup
//! system prompt (`bootstrap::build_system_prompt`) and the one-shot `/cd`
//! context ride-along. Missing/unparseable artifact → empty string; reading
//! is best-effort and never blocks startup.
//!
//! Controls: `ANGEL_DOSSIER=0` kills the block; `ANGEL_DOSSIER_DIR` overrides
//! the artifact directory; `ANGEL_DOSSIER_MIN_BELIEF` (default 0.70) and
//! `ANGEL_DOSSIER_MAX_BYTES` (default 2048) tune the filter and size cap.

use std::path::{Path, PathBuf};

pub(crate) const DOSSIER_BLOCK_HEADER: &str =
    "[repo dossier — what angel has verified about this workspace]";
pub(crate) const DOSSIER_BLOCK_SENTINEL: &str = "[/dossier]";

const DEFAULT_MIN_BELIEF: f64 = 0.70;
const DEFAULT_MAX_BYTES: usize = 2048;
const DEFAULT_BROKER_MAX_AGE_DAYS: f64 = 30.0;

/// Whether dossier injection is on (`ANGEL_DOSSIER`, default **on**).
fn enabled() -> bool {
    match std::env::var("ANGEL_DOSSIER") {
        Ok(v) => {
            let t = v.trim();
            !(t == "0"
                || t.eq_ignore_ascii_case("false")
                || t.eq_ignore_ascii_case("off")
                || t.eq_ignore_ascii_case("no"))
        }
        Err(_) => true,
    }
}

/// Artifact directory: `ANGEL_DOSSIER_DIR`, else `~/.angel0/dossier`.
fn dossier_dir() -> PathBuf {
    match std::env::var("ANGEL_DOSSIER_DIR") {
        Ok(p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => crate::workspace_store::angel_subdir("dossier"),
    }
}

fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(default)
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(default)
}

/// The dossier context block for `workspace`, or an empty string when there is
/// nothing confident to say (no artifact, no facts over the threshold, killed
/// by env). Best-effort I/O — never errors, never blocks.
pub(crate) fn context_block(workspace: &Path) -> String {
    if !enabled() {
        return String::new();
    }
    let block = context_block_in(&dossier_dir(), workspace, now_secs());
    // Everything this block asserts is now in the agent's mouth. A later command
    // matching one of them is the dossier's own echo coming back, and must not be
    // counted as independent evidence for the fact that suggested it — see
    // `experience::command_is_independent`. This is the *only* site that injects
    // the block (startup, `/cd`, `--task` all route through here), so noting it
    // here is an observation of what was actually asserted, not a guess.
    crate::experience::note_asserted_commands(workspace, &asserted_commands(&block));
    block
}

/// Read-time freshness gate used by the unified Knowledge Broker. The legacy
/// renderer intentionally remains byte-compatible for `ANGEL_BACKPLANE=0`;
/// active brokerage recomputes age from `generatedAt + ageDays`, so an old
/// artifact cannot stay authoritative merely because the external compiler
/// stopped ticking.
pub(crate) fn broker_context_block(workspace: &Path) -> String {
    if !enabled() {
        return String::new();
    }
    let block = broker_context_block_in(&dossier_dir(), workspace, now_secs());
    crate::experience::note_asserted_commands(workspace, &asserted_commands(&block));
    block
}

fn broker_context_block_in(dir: &Path, workspace: &Path, now: u64) -> String {
    let workspace_key = artifact_key(workspace);
    let path = dir.join(format!("{workspace_key}.json"));
    // Submit reads this on the UI thread. Key on file identity + unix-day so an
    // unchanged artifact is not re-parsed, while elapsed_days still advances at
    // midnight. Workspace key is part of the identity because the path is built
    // from it and two repos must never share a slot.
    type CacheKey = (PathBuf, std::time::SystemTime, u64, u64, String);
    static CACHE: std::sync::Mutex<Option<(CacheKey, String)>> = std::sync::Mutex::new(None);

    let Ok(meta) = std::fs::metadata(&path) else {
        return String::new();
    };
    let key: Option<CacheKey> = meta
        .modified()
        .ok()
        .map(|mtime| (path.clone(), mtime, meta.len(), now / 86_400, workspace_key));
    if let Some(ref key) = key
        && let Ok(guard) = CACHE.lock()
        && let Some((cached_key, block)) = guard.as_ref()
        && cached_key == key
    {
        return block.clone();
    }
    let block = render_broker_artifact(&path, now);
    if let Some(key) = key
        && let Ok(mut guard) = CACHE.lock()
    {
        *guard = Some((key, block.clone()));
    }
    block
}

fn render_broker_artifact(path: &Path, now: u64) -> String {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let Ok(mut artifact) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return String::new();
    };
    let generated_day = artifact
        .get("generatedAt")
        .and_then(serde_json::Value::as_str)
        .and_then(iso_day);
    let elapsed_days = generated_day.map(|day| ((now / 86_400) as i64 - day).max(0) as f64);
    let max_age = env_f64("ANGEL_DOSSIER_MAX_AGE_DAYS", DEFAULT_BROKER_MAX_AGE_DAYS).max(0.0);
    if let Some(facts) = artifact
        .get_mut("facts")
        .and_then(serde_json::Value::as_array_mut)
    {
        facts.retain(|fact| {
            elapsed_days.is_some_and(|elapsed| {
                fact.get("ageDays")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(f64::INFINITY)
                    + elapsed
                    <= max_age
            })
        });
    }
    let fresh_thread = artifact
        .get("thread")
        .and_then(serde_json::Value::as_object)
        .and_then(|thread| thread.get("ts"))
        .and_then(serde_json::Value::as_u64)
        .is_some_and(|ts| now.saturating_sub(ts) as f64 <= max_age * 86_400.0);
    if !fresh_thread {
        artifact["thread"] = serde_json::Value::Null;
    }
    render_block(
        &artifact,
        env_f64("ANGEL_DOSSIER_MIN_BELIEF", DEFAULT_MIN_BELIEF),
        env_usize("ANGEL_DOSSIER_MAX_BYTES", DEFAULT_MAX_BYTES),
        now,
    )
}

fn iso_day(value: &str) -> Option<i64> {
    let date = value.get(..10)?;
    let mut fields = date.split('-');
    let year = fields.next()?.parse::<i64>().ok()?;
    let month = fields.next()?.parse::<i64>().ok()?;
    let day = fields.next()?.parse::<i64>().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Howard Hinnant's civil-date transform, yielding Unix epoch days.
    let adjusted_year = year - i64::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let shifted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(era * 146_097 + day_of_era - 719_468)
}

/// Every command a rendered block asserts — its rituals and its traps.
///
/// Pure, and it reads the **final** block text, so a line the byte cap dropped is
/// correctly not counted as asserted. Traps ride along with rituals deliberately:
/// ``trap: `X` fails here`` puts `X` in the agent's mouth exactly as a ritual
/// does, and a run of it is no more independent.
pub(crate) fn asserted_commands(block: &str) -> Vec<String> {
    block
        .lines()
        .filter_map(|line| {
            // Fact lines are `<label>: \`<command>\` (…)`. The prose lines ("last
            // session here: …"), the header, and the sentinel have no such shape.
            let (label, rest) = line.split_once(": ")?;
            let labelled = !label.is_empty()
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
            if !labelled {
                return None;
            }
            let (cmd, _) = rest.strip_prefix('`')?.split_once('`')?;
            let cmd = cmd.trim();
            (!cmd.is_empty()).then(|| cmd.to_string())
        })
        .collect()
}

/// The repo's *learned* build ritual — the highest-belief `build`-class fact in
/// its dossier, or `None` when there's no artifact, nothing believed, or the
/// dossier is off. This is the middle rung of The Cut's verify precedence
/// (`ANGEL_CUT_VERIFY` → here → project-type default): if angel has watched a
/// repo build itself the same way across sessions, that command is a better
/// verifier than any guess we could make from file extensions.
///
/// Only the `build` class is ever returned. A learned `test` ritual is
/// deliberately *not* eligible: The Cut runs on every write, and a test suite
/// per write is the one thing that would make the agent unusable.
pub(crate) fn build_ritual(workspace: &Path) -> Option<String> {
    if !enabled() {
        return None;
    }
    build_ritual_in(&dossier_dir(), workspace)
}

/// [`build_ritual`] with the artifact directory injected (testable).
pub(crate) fn build_ritual_in(dir: &Path, workspace: &Path) -> Option<String> {
    let key = artifact_key(workspace);
    let raw = std::fs::read_to_string(dir.join(format!("{key}.json"))).ok()?;
    let artifact: serde_json::Value = serde_json::from_str(&raw).ok()?;
    pick_build_ritual(
        &artifact,
        env_f64("ANGEL_DOSSIER_MIN_BELIEF", DEFAULT_MIN_BELIEF),
    )
}

/// Pure picker: the best-believed `build` ritual clearing `min_belief`.
pub(crate) fn pick_build_ritual(artifact: &serde_json::Value, min_belief: f64) -> Option<String> {
    artifact["facts"]
        .as_array()?
        .iter()
        .filter(|f| f["kind"].as_str() == Some("ritual") && f["class"].as_str() == Some("build"))
        .filter(|f| f["belief"].as_f64().unwrap_or(0.0) >= min_belief)
        .max_by(|a, b| {
            a["belief"]
                .as_f64()
                .unwrap_or(0.0)
                .total_cmp(&b["belief"].as_f64().unwrap_or(0.0))
        })
        .and_then(|f| f["text"].as_str())
        .map(str::to_string)
        .filter(|text| !text.trim().is_empty())
}

/// The dossier artifact's key for `workspace` — the **repository** it belongs
/// to, not the directory. A session inside a linked worktree (every
/// `cut-forge` / `delegate` run is one) or a subdirectory therefore warm-starts
/// from its main repo's dossier, which is the only place that repo's facts can
/// have accumulated.
fn artifact_key(workspace: &Path) -> String {
    crate::workspace_store::repo_identity(workspace).key
}

/// [`context_block`] with the directory and clock injected (testable).
pub(crate) fn context_block_in(dir: &Path, workspace: &Path, now: u64) -> String {
    let key = artifact_key(workspace);
    let path = dir.join(format!("{key}.json"));
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return String::new();
    };
    let Ok(artifact) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return String::new();
    };
    render_block(
        &artifact,
        env_f64("ANGEL_DOSSIER_MIN_BELIEF", DEFAULT_MIN_BELIEF),
        env_usize("ANGEL_DOSSIER_MAX_BYTES", DEFAULT_MAX_BYTES),
        now,
    )
}

/// Pure renderer: artifact JSON → the injected block (or empty when nothing
/// clears the belief threshold and there's no thread to report).
pub(crate) fn render_block(
    artifact: &serde_json::Value,
    min_belief: f64,
    max_bytes: usize,
    now: u64,
) -> String {
    let facts = artifact["facts"].as_array().cloned().unwrap_or_default();
    let mut lines: Vec<String> = Vec::new();
    let mut withheld = 0usize;

    for f in &facts {
        let belief = f["belief"].as_f64().unwrap_or(0.0);
        if belief < min_belief {
            withheld += 1;
            continue;
        }
        let text = f["text"].as_str().unwrap_or_default();
        if text.is_empty() {
            continue;
        }
        let evidence = f["evidence"].as_str().unwrap_or_default();
        match f["kind"].as_str() {
            Some("ritual") => {
                let class = f["class"].as_str().unwrap_or("ritual");
                let dur = f["meanDurMs"]
                    .as_u64()
                    .filter(|&ms| ms >= 1_000)
                    .map(|ms| format!(", ~{}s", ms / 1_000))
                    .unwrap_or_default();
                lines.push(format!("{class}: `{text}` ({evidence}{dur})"));
            }
            Some("trap") => lines.push(format!("trap: `{text}` fails here ({evidence})")),
            _ => {}
        }
    }

    if let Some(thread) = artifact["thread"].as_object() {
        let ts = thread.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
        if ts > 0 {
            let stop = thread.get("stop").and_then(|v| v.as_str()).unwrap_or("?");
            let driver = thread
                .get("driver")
                .and_then(|v| v.as_str())
                .map(|d| format!(" ({d})"))
                .unwrap_or_default();
            lines.push(format!(
                "last session here: {}, ended with {stop}{driver}",
                ago(now, ts)
            ));
        }
    }

    if lines.is_empty() {
        return String::new();
    }
    if withheld > 0 {
        lines.push(format!(
            "({withheld} fact(s) below {min_belief:.2} belief withheld pending re-verification)"
        ));
    }

    let mut body = String::new();
    for line in lines {
        // Reserve room for the sentinel so the block always closes.
        if body.len() + line.len() + DOSSIER_BLOCK_SENTINEL.len() + 2 > max_bytes {
            body.push_str("…\n");
            break;
        }
        body.push_str(&line);
        body.push('\n');
    }
    format!("\n\n{DOSSIER_BLOCK_HEADER}\n{body}{DOSSIER_BLOCK_SENTINEL}\n")
}

/// The `/dossier` command body: the full fact list for `workspace` —
/// including withheld (below-threshold) facts, which the injected block
/// deliberately hides — plus where the artifact came from and how to refresh
/// it. Human-facing; the injected block stays minimal.
pub(crate) fn status_text(workspace: &Path) -> String {
    let key = artifact_key(workspace);
    let dir = dossier_dir();
    let path = dir.join(format!("{key}.json"));
    let artifact = std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());
    let Some(artifact) = artifact else {
        let compiler = crate::runtime_paths::script("runtime/repo-dossier.mjs");
        let quoted_compiler = compiler.to_string_lossy().replace('\'', "'\"'\"'");
        return format!(
            "no dossier for this workspace yet ({}).\n\
             It compiles from the experience ledger once commands recur across sessions:\n\
             node '{}' --refresh\n\
             Requires Node.js and Python 3. /dossier reads the resulting workspace facts.",
            path.display(),
            quoted_compiler
        );
    };
    let min_belief = env_f64("ANGEL_DOSSIER_MIN_BELIEF", DEFAULT_MIN_BELIEF);
    let now = now_secs();
    let mut out = format!(
        "repo dossier — {} (key {key})\ngenerated {}\n",
        workspace.display(),
        artifact["generatedAt"].as_str().unwrap_or("?"),
    );
    let facts = artifact["facts"].as_array().cloned().unwrap_or_default();
    if facts.is_empty() {
        out.push_str("no facts yet — needs ≥3 runs across ≥2 sessions per command.\n");
    }
    for f in &facts {
        let belief = f["belief"].as_f64().unwrap_or(0.0);
        let gate = if belief >= min_belief {
            "injected"
        } else {
            "withheld"
        };
        let kind = f["kind"].as_str().unwrap_or("?");
        let class = f["class"]
            .as_str()
            .map(|c| format!("{c} "))
            .unwrap_or_default();
        let text = f["text"].as_str().unwrap_or("?");
        let evidence = f["evidence"].as_str().unwrap_or("");
        let verified = match f["lastVerifiedAt"].as_str() {
            Some(ts) => format!("verified {ts}"),
            None => "never probed".to_string(),
        };
        out.push_str(&format!(
            "  [{belief:.2} {gate}] {class}{kind}: `{text}` — {evidence} · {verified}\n"
        ));
    }
    if let Some(thread) = artifact["thread"].as_object()
        && let Some(ts) = thread.get("ts").and_then(|v| v.as_u64()).filter(|&t| t > 0)
    {
        out.push_str(&format!(
                "last session here: {} · injection threshold {min_belief:.2} (ANGEL_DOSSIER_MIN_BELIEF)\n",
                ago(now, ts)
            ));
    }
    out
}

/// Compact "how long ago" for the thread line.
fn ago(now: u64, ts: u64) -> String {
    let secs = now.saturating_sub(ts);
    match secs {
        0..=3_599 => format!("{}m ago", (secs / 60).max(1)),
        3_600..=86_399 => format!("{}h ago", secs / 3_600),
        _ => format!("{}d ago", secs / 86_400),
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/dossier__tests.rs"]
mod tests;
