//! Repo Dossier warm-start injection from compiled experience-ledger facts.
//!
//! The dossier compiler (`scripts/repo-dossier.mjs`) mines the experience
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
        let compiler = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/repo-dossier.mjs");
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
mod tests {
    use super::*;

    #[test]
    fn dossier_compiler_output_reaches_the_native_context_reader() {
        let _lock = crate::tests::env_lock();
        let _enabled = crate::tests::TestEnvGuard::set("ANGEL_DOSSIER", "1");
        let _belief = crate::tests::TestEnvGuard::set("ANGEL_DOSSIER_MIN_BELIEF", "0.5");
        let _age = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER_MAX_AGE_DAYS");
        let _bytes = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER_MAX_BYTES");
        let temp = crate::tests::TestGitWorkspace::new("dossier-compiler");
        let workspace = temp.path().join("project");
        let cut = temp.path().join("cut");
        let out = temp.path().join("dossier");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(&cut).unwrap();
        let key = artifact_key(&workspace);
        let now = now_secs();
        let rows = (1..=3)
            .map(|session| {
                serde_json::json!({
                    "kind":"event", "v":3, "ts":now, "session":session, "seq":0,
                    "event":"cmd", "repo":{"key":key,"root":workspace,"slug":"project"},
                    "cmd":{"text":"cargo check","exit":0,"verdict":"pass",
                        "shell":"bash","pipefail":true,"source":"agent","independent":true,
                        "timed_out":false,"dur_ms":12,"tool":"shell","bytes_out":0}
                })
                .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        let ledger = temp.path().join("ledger.jsonl");
        std::fs::write(&ledger, rows).unwrap();
        let compiler = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/repo-dossier.mjs");
        let result = std::process::Command::new("node")
            .arg(compiler)
            .arg("--refresh")
            .arg("--ledger")
            .arg(&ledger)
            .arg("--cut")
            .arg(&cut)
            .arg("--out")
            .arg(&out)
            .current_dir(temp.path())
            .output()
            .expect("Node.js runs the dossier compiler");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(out.join(format!("{key}.json")).is_file());
        assert!(out.join("graph.json").is_file());
        let context = broker_context_block_in(&out, &workspace, now);
        assert!(context.contains("cargo check"), "{context}");
        assert!(!temp.path().join("public").exists());
    }

    fn artifact() -> serde_json::Value {
        serde_json::json!({
            "v": 1,
            "repo": { "key": "k", "root": "/r", "slug": "u/p" },
            "generatedAt": "2026-07-06T00:00:00.000Z",
            "facts": [
                { "kind": "ritual", "class": "test", "text": "cargo test",
                  "meanDurMs": 5000, "belief": 0.82, "ageDays": 1.0,
                  "evidence": "9 runs, 9 pass, 4 session(s), last 2026-07-06" },
                { "kind": "ritual", "class": "build", "text": "cargo build",
                  "meanDurMs": 200, "belief": 0.55, "ageDays": 1.0,
                  "evidence": "3 runs, 2 pass, 2 session(s), last 2026-07-01" },
                { "kind": "trap", "text": "npm test", "meanDurMs": null, "belief": 0.78, "ageDays": 1.0,
                  "evidence": "3 runs, 0 pass (3 fail), 3 session(s), last 2026-07-05" },
            ],
            "thread": { "ts": 1_783_300_000u64, "stop": "answer", "driver": "gemma", "ok": true },
        })
    }

    #[test]
    fn renders_confident_facts_and_withholds_the_rest() {
        let block = render_block(&artifact(), 0.70, 2048, 1_783_300_000 + 7_200);
        assert!(block.contains(DOSSIER_BLOCK_HEADER));
        assert!(block.ends_with(&format!("{DOSSIER_BLOCK_SENTINEL}\n")));
        // The confident ritual renders with its class, duration, and evidence.
        assert!(
            block.contains("test: `cargo test` (9 runs, 9 pass"),
            "{block}"
        );
        assert!(block.contains("~5s"));
        // The trap renders as a trap.
        assert!(block.contains("trap: `npm test` fails here"));
        // The 0.55 build ritual is withheld, and the block says so.
        assert!(!block.contains("cargo build"));
        assert!(block.contains("1 fact(s) below 0.70 belief withheld"));
        // The thread line is humanized.
        assert!(block.contains("last session here: 2h ago, ended with answer (gemma)"));
    }

    /// What the block asserts is what the agent was told to run — the input to the
    /// self-confirmation guard. Rituals *and* traps count; prose lines do not.
    #[test]
    fn asserted_commands_are_every_command_the_block_names() {
        let block = render_block(&artifact(), 0.70, 2048, 1_783_300_000 + 7_200);
        let asserted = asserted_commands(&block);
        assert!(asserted.contains(&"cargo test".to_string()), "{asserted:?}");
        // A trap puts its command in the agent's mouth exactly as a ritual does.
        assert!(asserted.contains(&"npm test".to_string()), "{asserted:?}");
        // The withheld build ritual was never asserted, so it never contaminates.
        assert!(!asserted.contains(&"cargo build".to_string()));
        // Prose lines ("last session here: …"), the header, and the sentinel are
        // not commands — a `: ` alone must not be mistaken for a fact line.
        assert!(!asserted.iter().any(|c| c.contains("last session")));
        assert_eq!(asserted.len(), 2, "{asserted:?}");
        // Nothing asserted from an empty block.
        assert!(asserted_commands("").is_empty());
    }

    #[test]
    fn nothing_confident_and_no_thread_renders_nothing() {
        let a = serde_json::json!({
            "facts": [ { "kind": "ritual", "class": "test", "text": "x", "belief": 0.5,
                         "evidence": "" } ],
            "thread": null,
        });
        assert_eq!(render_block(&a, 0.70, 2048, 0), "");
        assert_eq!(render_block(&serde_json::json!({}), 0.70, 2048, 0), "");
    }

    #[test]
    fn size_cap_truncates_but_always_closes_the_block() {
        let block = render_block(&artifact(), 0.70, 96, 1_783_300_000);
        assert!(block.len() <= 96 + DOSSIER_BLOCK_HEADER.len() + 8);
        assert!(block.contains('…'));
        assert!(block.trim_end().ends_with(DOSSIER_BLOCK_SENTINEL));
    }

    #[test]
    fn context_block_in_reads_by_workspace_key_and_survives_garbage() {
        let base = std::env::temp_dir().join(format!("angel-dossier-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let ws = Path::new("/home/u/proj");
        let key = crate::tools::work_landing::workspace_key(ws);

        // No artifact → empty.
        assert_eq!(context_block_in(&base, ws, 0), "");
        // Garbage artifact → empty, no panic.
        std::fs::write(base.join(format!("{key}.json")), "{not json").unwrap();
        assert_eq!(context_block_in(&base, ws, 0), "");
        // Real artifact → rendered block.
        std::fs::write(
            base.join(format!("{key}.json")),
            serde_json::to_string(&artifact()).unwrap(),
        )
        .unwrap();
        let block = context_block_in(&base, ws, 1_783_300_000);
        assert!(block.contains("cargo test"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn broker_recomputes_freshness_when_external_tick_stops() {
        let _guard = crate::tests::env_lock();
        let _max_age = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER_MAX_AGE_DAYS");
        let _belief = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER_MIN_BELIEF");
        let _bytes = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER_MAX_BYTES");
        let base = isolated_dir("broker-freshness");
        let ws = fake_ws("freshness");
        let path = write_workspace_artifact(&base, &ws, &artifact());
        let generated = iso_day("2026-07-06").unwrap() as u64 * 86_400;
        assert!(broker_context_block_in(&base, &ws, generated + 5 * 86_400).contains("cargo test"));
        assert_eq!(
            broker_context_block_in(&base, &ws, generated + 40 * 86_400),
            ""
        );
        let mut unverifiable = artifact();
        unverifiable.as_object_mut().unwrap().remove("generatedAt");
        unverifiable["thread"] = serde_json::Value::Null;
        std::fs::write(&path, serde_json::to_string(&unverifiable).unwrap()).unwrap();
        assert_eq!(
            broker_context_block_in(&base, &ws, generated + 5 * 86_400),
            "",
            "facts without a generation time cannot prove read-time freshness"
        );
        let _ = std::fs::remove_dir_all(base);
    }

    fn isolated_dir(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("angel-dossier-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        base
    }

    fn fake_ws(tag: &str) -> PathBuf {
        PathBuf::from(format!("/home/u/proj-dossier-{tag}-{}", std::process::id()))
    }

    fn write_workspace_artifact(
        dir: &Path,
        workspace: &Path,
        artifact: &serde_json::Value,
    ) -> PathBuf {
        let path = dir.join(format!("{}.json", artifact_key(workspace)));
        std::fs::write(&path, serde_json::to_string(artifact).unwrap()).unwrap();
        path
    }

    fn file_identity(path: &Path) -> (std::time::SystemTime, u64) {
        let meta = std::fs::metadata(path).unwrap();
        (meta.modified().unwrap(), meta.len())
    }

    fn overwrite_preserving_identity(path: &Path, bytes: &[u8]) {
        let (mtime, len) = file_identity(path);
        assert_eq!(len, bytes.len() as u64, "replacement must keep len");
        std::fs::write(path, bytes).unwrap();
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(mtime)
            .unwrap();
        assert_eq!(file_identity(path), (mtime, len));
    }

    #[test]
    fn broker_block_caches_by_mtime_len_and_unix_day() {
        let _guard = crate::tests::env_lock();
        let _max_age = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER_MAX_AGE_DAYS");
        let _belief = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER_MIN_BELIEF");
        let _bytes = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER_MAX_BYTES");
        let base = isolated_dir("broker-cache");
        let ws = fake_ws("cache");
        let generated = iso_day("2026-07-06").unwrap() as u64 * 86_400;
        let now = generated + 5 * 86_400;

        assert_eq!(
            broker_context_block_in(&base, &ws, now),
            "",
            "missing file → empty"
        );

        let path = write_workspace_artifact(&base, &ws, &artifact());
        let first = broker_context_block_in(&base, &ws, now);
        assert!(first.contains("cargo test"), "{first}");

        let original = std::fs::read(&path).unwrap();
        let mut garbage = vec![b'x'; original.len()];
        let last = garbage.len() - 1;
        garbage[0] = b'{';
        garbage[last] = b'}';
        overwrite_preserving_identity(&path, &garbage);
        let hit = broker_context_block_in(&base, &ws, now);
        assert_eq!(
            hit, first,
            "unchanged mtime/len must keep the cached render even if bytes changed"
        );
        assert_eq!(
            broker_context_block_in(&base, &ws, now + 86_400),
            "",
            "unix-day is part of the key: next day re-reads the now-unparseable file"
        );

        let mut updated = artifact();
        updated["facts"][0]["text"] = serde_json::json!("pytest -q");
        std::fs::write(&path, serde_json::to_string(&updated).unwrap()).unwrap();
        let bumped = file_identity(&path).0 + std::time::Duration::from_secs(2);
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(bumped)
            .unwrap();
        let busted = broker_context_block_in(&base, &ws, now);
        assert!(
            busted.contains("pytest -q"),
            "mtime/len change must re-read: {busted}"
        );
        assert!(
            !busted.contains("cargo test"),
            "stale cached ritual must not survive an identity change: {busted}"
        );

        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            broker_context_block_in(&base, &ws, now),
            "",
            "deleted artifact must not leak a previous cache"
        );
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn broker_block_disabled_does_not_leak_cache() {
        let _guard = crate::tests::env_lock();
        let _max_age = crate::tests::TestEnvGuard::set("ANGEL_DOSSIER_MAX_AGE_DAYS", "36500");
        let _belief = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER_MIN_BELIEF");
        let _bytes = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER_MAX_BYTES");
        let _on = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER");
        let base = isolated_dir("broker-disabled");
        let dir_s = base.to_str().expect("utf8 temp dir").to_string();
        let _dir = crate::tests::TestEnvGuard::set("ANGEL_DOSSIER_DIR", &dir_s);
        let ws = fake_ws("disabled");
        write_workspace_artifact(&base, &ws, &artifact());

        let live = broker_context_block(&ws);
        assert!(
            live.contains("cargo test"),
            "enabled path must populate a cached block: {live}"
        );
        let _off = crate::tests::TestEnvGuard::set("ANGEL_DOSSIER", "0");
        assert_eq!(
            broker_context_block(&ws),
            "",
            "ANGEL_DOSSIER=0 must not return a previously cached block"
        );
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn build_ritual_is_picked_by_belief_and_never_a_test_command() {
        // The best-believed fact in this artifact is `cargo test` (0.82). The
        // Cut runs its verify on EVERY write, so a test ritual must never be
        // eligible however confident it is — only the `build` class is.
        assert_eq!(
            pick_build_ritual(&artifact(), 0.50).as_deref(),
            Some("cargo build")
        );
        // Below the belief threshold the repo has taught us nothing usable, and
        // the caller falls through to the project-type default.
        assert_eq!(pick_build_ritual(&artifact(), 0.70), None);
        // No artifact / no facts → nothing.
        assert_eq!(pick_build_ritual(&serde_json::json!({}), 0.0), None);
    }

    #[test]
    fn ago_is_humane() {
        assert_eq!(ago(1_000_000, 1_000_000 - 30), "1m ago");
        assert_eq!(ago(1_000_000, 1_000_000 - 1_800), "30m ago");
        assert_eq!(ago(1_000_000, 1_000_000 - 7_200), "2h ago");
        assert_eq!(ago(1_000_000, 1_000_000 - 300_000), "3d ago");
    }
}
