//! Angel's Share barrel (AS1) — capped, redacted capture of (context,
//! teacher answer) pairs at turn finalization. Plan: docs/plans/angels-share.md.
//!
//! Capture is default-OFF and doubly gated: `ANGEL_BARREL=1` arms the writer,
//! and only clubs named in `ANGEL_BARREL_TEACHERS` are barreled (the ToS seam
//! — the operator chooses which providers' outputs may feed training). The
//! turn thread only gates, clones, and `try_send`s; one background thread owns
//! all serialization, redaction, dedupe, and disk work, and a full channel
//! drops the capture rather than ever stalling a turn.
//!
//! Raw shards are day files (`barrel-YYYYMMDD.jsonl`) hard-capped in total by
//! `ANGEL_BARREL_MAX_MB`: at cap the oldest *closed* shard is evicted first,
//! and today's shard is never evicted — new captures drop instead. The Node
//! still tick owns scoring, weekly distillation, and the unconditional TTL
//! evaporation; by convention it rewrites only closed (previous-day) shards,
//! so this writer is the sole author of today's file.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};

use crate::club::{ChatMsg, ChatRole, Club};

/// Credential nouns that mark a line as sensitive (the house denylist).
const DENY: [&str; 5] = ["key", "token", "secret", "auth", "password"];
/// Sender-side channel capacity; a slow disk drops copies, never blocks.
const CHANNEL_CAP: usize = 8;
/// How many recent context digests the writer remembers for dedupe.
const DEDUPE_WINDOW: usize = 4096;

pub(crate) struct BarrelCfg {
    pub(crate) enabled: bool,
    /// Lowercased club labels allowed as teachers; empty = capture nothing.
    pub(crate) teachers: Vec<String>,
    pub(crate) dir: PathBuf,
    pub(crate) max_bytes: u64,
}

impl BarrelCfg {
    pub(crate) fn from_env() -> Self {
        // Exact-string arming (the conductor/forge-latch posture): anything
        // but a literal "1" leaves capture off.
        let enabled = std::env::var("ANGEL_BARREL")
            .map(|v| v.trim() == "1")
            .unwrap_or(false);
        let teachers = std::env::var("ANGEL_BARREL_TEACHERS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        let dir = std::env::var_os("ANGEL_BARREL_DIR")
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| crate::workspace_store::angel_subdir("barrel"));
        let max_mb = std::env::var("ANGEL_BARREL_MAX_MB")
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(1024);
        Self {
            enabled,
            teachers,
            dir,
            max_bytes: max_mb.saturating_mul(1024 * 1024),
        }
    }

    /// A club is an armed teacher when either its own label or its resolved
    /// route driver appears in the allowlist (case-insensitive).
    fn teacher_armed(&self, label: &str, resolved_driver: &str) -> bool {
        let l = label.to_ascii_lowercase();
        let d = resolved_driver.to_ascii_lowercase();
        self.teachers.iter().any(|t| *t == l || *t == d)
    }
}

struct Job {
    ts: u64,
    repo: serde_json::Value,
    club: String,
    model: Option<String>,
    history: Vec<ChatMsg>,
    answer: String,
}

static SENDER: OnceLock<SyncSender<Job>> = OnceLock::new();
static DROPPED_BACKPRESSURE: AtomicU64 = AtomicU64::new(0);

/// Hot-path entry, called from the turn loop's answer exit with the final-hop
/// history (pre-answer) and the answer separately, so the record never
/// duplicates the answer inside its own context. Best-effort and test-silent,
/// like the experience ledger: it can never fail or slow a turn.
pub(crate) fn capture_answer(club: &dyn Club, workspace: &Path, history: &[ChatMsg], answer: &str) {
    if cfg!(test) {
        return;
    }
    let cfg = BarrelCfg::from_env();
    if !cfg.enabled || answer.trim().is_empty() {
        return;
    }
    let route = club.resolved_route_identity();
    if !cfg.teacher_armed(club.label(), &route.driver) {
        return;
    }
    let tx = SENDER.get_or_init(|| {
        let (tx, rx) = sync_channel::<Job>(CHANNEL_CAP);
        let _ = std::thread::Builder::new()
            .name("angel-barrel".into())
            .spawn(move || writer_loop(rx));
        tx
    });
    let job = Job {
        ts: now_secs(),
        repo: crate::experience::repo_value_for(workspace),
        club: club.label().to_string(),
        model: route.model,
        history: history.to_vec(),
        answer: answer.to_string(),
    };
    if tx.try_send(job).is_err() {
        DROPPED_BACKPRESSURE.fetch_add(1, Ordering::Relaxed);
    }
}

fn writer_loop(rx: Receiver<Job>) {
    let mut state = WriterState::default();
    while let Ok(job) = rx.recv() {
        // Re-read config per job so a live env change (re-arm, cap bump)
        // takes effect without a restart.
        let cfg = BarrelCfg::from_env();
        process_job(&cfg, job, &mut state);
    }
}

#[derive(Default)]
struct WriterState {
    seen: HashSet<u64>,
    order: VecDeque<u64>,
    appended: u64,
    dropped_dupe: u64,
    dropped_cap: u64,
    io_errors: u64,
    redacted_lines: u64,
    evicted_shards: u64,
}

fn process_job(cfg: &BarrelCfg, job: Job, state: &mut WriterState) {
    let (context, redacted_ctx) = context_value(&job.history);
    let (answer, redacted_ans) = redact_text(&job.answer);
    state.redacted_lines += (redacted_ctx + redacted_ans) as u64;
    let digest = digest_of(&context, &answer);
    if !remember(state, digest) {
        state.dropped_dupe += 1;
        write_status(cfg, state, job.ts);
        return;
    }
    let rec = serde_json::json!({
        "v": 1,
        "kind": "capture",
        "ts": job.ts,
        "repo": job.repo,
        "teacher": { "club": job.club, "model": job.model },
        "digest": format!("{digest:016x}"),
        "msgs": context.as_array().map(|a| a.len()).unwrap_or(0),
        "context": context,
        "answer": answer,
    });
    let shard = cfg
        .dir
        .join(format!("barrel-{}.jsonl", utc_yyyymmdd(job.ts)));
    match append_capped(cfg, &shard, &rec.to_string(), state) {
        Append::Ok => state.appended += 1,
        Append::DroppedCap => state.dropped_cap += 1,
        Append::IoError => state.io_errors += 1,
    }
    write_status(cfg, state, job.ts);
}

/// Serialize the message history to the barrel's context shape: role +
/// redacted content, tool calls as (name, redacted args), tool_call_id
/// passthrough, attachments reduced to a count (media never enters the
/// barrel).
fn context_value(history: &[ChatMsg]) -> (serde_json::Value, usize) {
    let mut redacted = 0usize;
    let msgs: Vec<serde_json::Value> = history
        .iter()
        .map(|m| {
            let (content, n) = redact_text(&m.content);
            redacted += n;
            let mut obj = serde_json::json!({
                "role": role_str(&m.role),
                "content": content,
            });
            if let Some(id) = &m.tool_call_id {
                obj["tool_call_id"] = id.clone().into();
            }
            if !m.tool_calls.is_empty() {
                obj["tool_calls"] = m
                    .tool_calls
                    .iter()
                    .map(|c| {
                        let (args, n) = redact_text(&c.args.to_string());
                        redacted += n;
                        serde_json::json!({ "name": c.name, "args": args })
                    })
                    .collect::<Vec<_>>()
                    .into();
            }
            if !m.attachments.is_empty() {
                obj["attachments"] = m.attachments.len().into();
            }
            obj
        })
        .collect();
    (serde_json::Value::Array(msgs), redacted)
}

fn role_str(role: &ChatRole) -> &'static str {
    match role {
        ChatRole::System => "system",
        ChatRole::User => "user",
        ChatRole::Harness => "harness",
        ChatRole::Assistant => "assistant",
        ChatRole::Tool => "tool",
    }
}

/// A line is dropped when it names a credential noun AND carries an
/// assignment/bearer shape (`KEY=…`, `token: …`, `Authorization: Bearer …`).
/// Over-redaction is the accepted trade: barrel data feeds training, so a
/// shredded line is always safer than a leaked one.
fn line_is_sensitive(line: &str) -> bool {
    let l = line.to_ascii_lowercase();
    if !DENY.iter().any(|w| l.contains(w)) {
        return false;
    }
    l.contains('=') || l.contains(':') || l.contains("bearer")
}

/// The house redactor: drop credential-bearing lines, report how many. Shared
/// with The Cut's authored manifest (`cut.rs`) — one redactor, one denylist, so
/// a rule tightened here tightens everywhere captured text is written to disk.
pub(crate) fn redact_text(text: &str) -> (String, usize) {
    let mut count = 0usize;
    let redactor = crate::secrets::Redactor::new();
    let out = text
        .lines()
        .map(|line| {
            if line_is_sensitive(line) || redactor.redact_str(line) != line {
                count += 1;
                "«redacted»"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    (out, count)
}

fn digest_of(context: &serde_json::Value, answer: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    context.to_string().hash(&mut h);
    answer.hash(&mut h);
    h.finish()
}

/// Session-scoped dedupe over a bounded window of recent context digests.
fn remember(state: &mut WriterState, digest: u64) -> bool {
    if state.seen.contains(&digest) {
        return false;
    }
    state.seen.insert(digest);
    state.order.push_back(digest);
    if state.order.len() > DEDUPE_WINDOW
        && let Some(old) = state.order.pop_front()
    {
        state.seen.remove(&old);
    }
    true
}

/// Civil date (UTC) from a unix timestamp, no chrono dependency
/// (Howard Hinnant's civil_from_days). Shared with The Cut, which shards its
/// manifest by the same day boundary.
pub(crate) fn utc_yyyymmdd(ts: u64) -> String {
    let z = (ts / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}{m:02}{d:02}")
}

fn shard_files(dir: &Path) -> Vec<(PathBuf, u64)> {
    shard_files_named(dir, "barrel-")
}

/// Day shards under `dir` with the given name prefix, oldest first
/// (lexicographic = chronological for the `<prefix>YYYYMMDD.jsonl` names both
/// the barrel and The Cut use), paired with their sizes.
pub(crate) fn shard_files_named(dir: &Path, prefix: &str) -> Vec<(PathBuf, u64)> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with(prefix) && name.ends_with(".jsonl") {
                let len = e.metadata().map(|m| m.len()).unwrap_or(0);
                out.push((p, len));
            }
        }
    }
    out.sort();
    out
}

pub(crate) enum Append {
    Ok,
    DroppedCap,
    IoError,
}

fn append_capped(cfg: &BarrelCfg, shard: &Path, line: &str, state: &mut WriterState) -> Append {
    append_capped_shard(
        &cfg.dir,
        "barrel-",
        shard,
        line,
        cfg.max_bytes,
        &mut state.evicted_shards,
    )
}

/// Append one line to a day shard under a hard *total* size cap: at cap the
/// oldest closed shard is evicted first, and today's shard is never evicted —
/// the new record drops instead, so a live writer can never lose the file it is
/// appending to. The barrel's discipline, shared with The Cut's manifest.
pub(crate) fn append_capped_shard(
    dir: &Path,
    prefix: &str,
    shard: &Path,
    line: &str,
    max_bytes: u64,
    evicted: &mut u64,
) -> Append {
    let Ok(directory) = crate::workspace_store::private_io::PrivateDirectory::open(dir) else {
        return Append::IoError;
    };
    let Some(shard_name) = shard.file_name() else {
        return Append::IoError;
    };
    append_capped_directory(&directory, prefix, shard_name, line, max_bytes, evicted)
}

fn append_capped_directory(
    directory: &crate::workspace_store::private_io::PrivateDirectory,
    prefix: &str,
    shard_name: &std::ffi::OsStr,
    line: &str,
    max_bytes: u64,
    evicted: &mut u64,
) -> Append {
    use std::io::Write;
    let Ok(names) = directory.names() else {
        return Append::IoError;
    };
    let mut shards = Vec::new();
    for name in names {
        let text = name.to_str().unwrap_or("");
        if !text.starts_with(prefix) || !text.ends_with(".jsonl") {
            continue;
        }
        let Ok(Some(file)) = directory.existing(&name) else {
            return Append::IoError;
        };
        let Ok(metadata) = file.metadata() else {
            return Append::IoError;
        };
        shards.push((name, metadata.len()));
    }
    let mut total: u64 = shards.iter().map(|(_, n)| n).sum();
    let incoming = line.len() as u64 + 1;
    while total + incoming > max_bytes {
        match shards.first() {
            Some((name, bytes)) if name != shard_name => {
                let (name, bytes) = (name.clone(), *bytes);
                let Ok(Some(file)) = directory.existing(&name) else {
                    return Append::IoError;
                };
                if directory.remove_owned(&name, &file).is_err() {
                    return Append::IoError;
                }
                *evicted += 1;
                total = total.saturating_sub(bytes);
                shards.remove(0);
            }
            // Only today's shard is left (or the dir is empty and the record
            // alone exceeds the cap): drop the new capture, never the live
            // shard.
            _ => return Append::DroppedCap,
        }
    }
    match directory.append(shard_name) {
        Ok(mut f) => match writeln!(f, "{line}") {
            Ok(()) => Append::Ok,
            Err(_) => Append::IoError,
        },
        Err(_) => Append::IoError,
    }
}

/// Small status artifact for `/still` and the Node tick — counters only,
/// never record content. Written via tmp+rename so readers never see a
/// partial file.
fn write_status(cfg: &BarrelCfg, state: &WriterState, ts: u64) {
    let shards = shard_files(&cfg.dir);
    let total: u64 = shards.iter().map(|(_, n)| n).sum();
    let status = serde_json::json!({
        "v": 1,
        "ts": ts,
        "bytes": total,
        "max_bytes": cfg.max_bytes,
        "shards": shards.len(),
        "appended": state.appended,
        "dropped_dupe": state.dropped_dupe,
        "dropped_cap": state.dropped_cap,
        "dropped_backpressure": DROPPED_BACKPRESSURE.load(Ordering::Relaxed),
        "io_errors": state.io_errors,
        "redacted_lines": state.redacted_lines,
        "evicted_shards": state.evicted_shards,
    });
    let _ = crate::workspace_store::private_io::replace(
        &cfg.dir.join("writer-status.json"),
        status.to_string().as_bytes(),
    );
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ─── /still — the operator surface (AS4) ────────────────────────────────────
// Rust never parses shards: it reads only the two small status artifacts —
// writer-status.json (written by this module's thread) and
// ~/.angel0/still/status.json (written by scripts/still-tick.mjs) — the
// conductor read-side pattern. Zero interactive-prompt injection: this text
// renders only when the operator types /still.

pub(crate) fn run(arg: Option<&str>) -> String {
    match arg.map(str::trim).unwrap_or("") {
        "" | "status" => {
            let cfg = BarrelCfg::from_env();
            status_text_in(
                &cfg,
                &crate::workspace_store::angel_subdir("still"),
                now_secs(),
            )
        }
        _ => "usage: /still — Angel's Share status (barrel fill, scoring backlog, weekly distill, gap trend)"
            .to_string(),
    }
}

fn read_json(path: &Path) -> Option<serde_json::Value> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
}

fn age_str(now: u64, ts: u64) -> String {
    if ts == 0 || ts > now {
        return "just now".to_string();
    }
    let s = now - ts;
    match s {
        0..=119 => format!("{s}s ago"),
        120..=7199 => format!("{}m ago", s / 60),
        7200..=172_799 => format!("{}h ago", s / 3600),
        _ => format!("{}d ago", s / 86_400),
    }
}

fn status_text_in(cfg: &BarrelCfg, still_dir: &Path, now: u64) -> String {
    let mut out = String::from("angel's share status\n");
    // Arming line — capture is the cockpit's half, so env is authoritative here.
    if !cfg.enabled {
        out.push_str("  capture: disarmed (set ANGEL_BARREL=1 + ANGEL_BARREL_TEACHERS to pour)\n");
    } else if cfg.teachers.is_empty() {
        out.push_str("  capture: armed, but ANGEL_BARREL_TEACHERS is empty — nothing pours in\n");
    } else {
        out.push_str(&format!(
            "  capture: armed · teachers: {}\n",
            cfg.teachers.join(", ")
        ));
    }
    // Barrel line — the writer thread's counters.
    match read_json(&cfg.dir.join("writer-status.json")) {
        Some(w) => {
            let n = |k: &str| w[k].as_u64().unwrap_or(0);
            let mb = |b: u64| b as f64 / (1024.0 * 1024.0);
            out.push_str(&format!(
                "  barrel: {} shard(s) · {:.1} / {:.0} MB · appended {} · dropped {} (dupe {}, cap {}, backpressure {}) · redacted {} line(s) · evicted {} shard(s)\n",
                n("shards"),
                mb(n("bytes")),
                mb(w["max_bytes"].as_u64().unwrap_or(cfg.max_bytes)),
                n("appended"),
                n("dropped_dupe") + n("dropped_cap") + n("dropped_backpressure"),
                n("dropped_dupe"),
                n("dropped_cap"),
                n("dropped_backpressure"),
                n("redacted_lines"),
                n("evicted_shards"),
            ));
        }
        None => out.push_str(&format!(
            "  barrel: no captures yet ({})\n",
            cfg.dir.display()
        )),
    }
    // Still line + gap trend — the Node tick's export.
    let status_path = still_dir.join("status.json");
    match read_json(&status_path) {
        Some(s) => {
            let n = |k: &str| s[k].as_u64().unwrap_or(0);
            out.push_str(&format!(
                "  still: last tick {} · unscored {} · scored this tick {} ({} failures) · last distill {}\n",
                age_str(now, n("ts")),
                n("unscored"),
                n("scored_this_tick"),
                n("score_failures"),
                s["last_distill_week"].as_str().unwrap_or("never"),
            ));
            let trend: Vec<String> = s["gap_trend"]
                .as_array()
                .map(|rows| {
                    rows.iter()
                        .filter_map(|r| {
                            let week = r["week"].as_str()?;
                            match r["mean_gap"].as_f64() {
                                Some(g) => Some(format!("{week} gap {g:.1}")),
                                None => Some(format!("{week} unscored")),
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            if !trend.is_empty() {
                out.push_str(&format!(
                    "  trend: {} (lower gap = the student is closing in)\n",
                    trend.join(" → ")
                ));
            }
        }
        None => out.push_str(&format!(
            "  still: no tick export yet ({}) — install with scripts/install-still-cron.sh\n",
            status_path.display()
        )),
    }
    out.push_str("  plan: docs/plans/angels-share.md");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("angel-barrel-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn cfg_in(dir: &Path, max_bytes: u64) -> BarrelCfg {
        BarrelCfg {
            enabled: true,
            teachers: vec!["sota".into()],
            dir: dir.to_path_buf(),
            max_bytes,
        }
    }

    fn job(ts: u64, user: &str, answer: &str) -> Job {
        Job {
            ts,
            repo: serde_json::json!({ "key": "k" }),
            club: "sota".into(),
            model: Some("m-1".into()),
            history: vec![ChatMsg::system("be brief"), ChatMsg::user(user)],
            answer: answer.into(),
        }
    }

    #[test]
    fn redaction_drops_credential_lines_and_spares_code() {
        let text = "fn keyboard_input() {}\nOPENAI_API_KEY=sk-live-abc123\nAuthorization: Bearer eyJx\nplain prose line";
        let (out, n) = redact_text(text);
        assert_eq!(n, 2, "exactly the two credential lines are redacted");
        assert!(!out.contains("sk-live-abc123"));
        assert!(!out.contains("eyJx"));
        assert!(
            out.contains("fn keyboard_input() {}"),
            "code naming 'key' without an assignment shape survives"
        );
        assert!(out.contains("plain prose line"));
    }

    #[test]
    fn utc_dates_are_civil_correct() {
        assert_eq!(utc_yyyymmdd(0), "19700101");
        assert_eq!(utc_yyyymmdd(86_399), "19700101");
        assert_eq!(utc_yyyymmdd(86_400), "19700102");
        assert_eq!(utc_yyyymmdd(1_000_000_000), "20010909");
        assert_eq!(utc_yyyymmdd(951_782_400), "20000229", "leap day");
    }

    #[test]
    fn process_job_appends_a_redacted_record_and_status() {
        let dir = test_dir("append");
        let cfg = cfg_in(&dir, u64::MAX);
        let mut state = WriterState::default();
        process_job(
            &cfg,
            job(
                86_400,
                "here is my API_KEY=sk-live-zzz please use it",
                "done",
            ),
            &mut state,
        );
        let shard =
            std::fs::read_to_string(dir.join("barrel-19700102.jsonl")).expect("shard written");
        assert!(
            !shard.contains("sk-live-zzz"),
            "planted secret never reaches disk"
        );
        assert!(shard.contains("«redacted»"));
        let rec: serde_json::Value = serde_json::from_str(shard.lines().next().unwrap()).unwrap();
        assert_eq!(rec["kind"], "capture");
        assert_eq!(rec["teacher"]["club"], "sota");
        assert_eq!(rec["msgs"], 2);
        assert_eq!(rec["answer"], "done");
        assert!(rec["digest"].as_str().unwrap().len() == 16);
        let status: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("writer-status.json")).unwrap())
                .unwrap();
        assert_eq!(status["appended"], 1);
        assert_eq!(status["redacted_lines"], 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicate_context_is_dropped() {
        let dir = test_dir("dupe");
        let cfg = cfg_in(&dir, u64::MAX);
        let mut state = WriterState::default();
        process_job(
            &cfg,
            job(86_400, "same question", "same answer"),
            &mut state,
        );
        process_job(
            &cfg,
            job(90_000, "same question", "same answer"),
            &mut state,
        );
        let shard = std::fs::read_to_string(dir.join("barrel-19700102.jsonl")).unwrap();
        assert_eq!(shard.lines().count(), 1, "second identical capture deduped");
        assert_eq!(state.dropped_dupe, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cap_evicts_oldest_closed_shard_first() {
        let dir = test_dir("evict");
        // Big enough for roughly one ~280-byte record, so day two forces eviction.
        let cfg = cfg_in(&dir, 300);
        let mut state = WriterState::default();
        process_job(
            &cfg,
            job(86_400, "day one question", "day one answer"),
            &mut state,
        );
        assert!(dir.join("barrel-19700102.jsonl").exists());
        process_job(
            &cfg,
            job(2 * 86_400, "day two question", "day two answer"),
            &mut state,
        );
        assert!(
            !dir.join("barrel-19700102.jsonl").exists(),
            "oldest shard evicted at cap"
        );
        assert!(
            dir.join("barrel-19700103.jsonl").exists(),
            "current shard written"
        );
        assert_eq!(state.evicted_shards, 1);
        assert_eq!(state.appended, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cap_never_evicts_todays_shard() {
        let dir = test_dir("cap-today");
        let cfg = cfg_in(&dir, 300);
        let mut state = WriterState::default();
        process_job(
            &cfg,
            job(86_400, "first question", "first answer"),
            &mut state,
        );
        process_job(
            &cfg,
            job(86_500, "second question of the day", "second answer"),
            &mut state,
        );
        assert!(
            dir.join("barrel-19700102.jsonl").exists(),
            "live shard survives cap pressure"
        );
        assert_eq!(
            state.dropped_cap, 1,
            "over-cap same-day capture dropped, not evicted"
        );
        assert_eq!(state.appended, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn teacher_arming_matches_label_or_resolved_driver() {
        let cfg = BarrelCfg {
            enabled: true,
            teachers: vec!["grok".into(), "glm".into()],
            dir: PathBuf::from("."),
            max_bytes: 1,
        };
        assert!(cfg.teacher_armed("GROK", "whatever"));
        assert!(cfg.teacher_armed("sota", "GLM"));
        assert!(!cfg.teacher_armed("openai", "codex"));
        let none = BarrelCfg {
            teachers: Vec::new(),
            ..cfg
        };
        assert!(
            !none.teacher_armed("grok", "grok"),
            "empty allowlist captures nothing"
        );
    }

    #[test]
    fn still_status_renders_disarmed_and_armed_fixtures() {
        let dir = test_dir("status");
        let still = dir.join("still");
        // Disarmed, nothing on disk: every line degrades gracefully.
        let cfg = BarrelCfg {
            enabled: false,
            teachers: Vec::new(),
            dir: dir.clone(),
            max_bytes: 1024 * 1024,
        };
        let text = status_text_in(&cfg, &still, 1_000);
        assert!(text.contains("capture: disarmed"));
        assert!(text.contains("no captures yet"));
        assert!(text.contains("no tick export yet"));
        // Armed with both status artifacts present.
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(&still).unwrap();
        std::fs::write(
            dir.join("writer-status.json"),
            serde_json::json!({
                "shards": 3, "bytes": 2 * 1024 * 1024, "max_bytes": 8 * 1024 * 1024,
                "appended": 12, "dropped_dupe": 1, "dropped_cap": 2,
                "dropped_backpressure": 0, "redacted_lines": 7, "evicted_shards": 1
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            still.join("status.json"),
            serde_json::json!({
                "ts": 900, "unscored": 5, "scored_this_tick": 4, "score_failures": 1,
                "last_distill_week": "2026-W27",
                "gap_trend": [
                    { "week": "2026-W26", "mean_gap": 6.4 },
                    { "week": "2026-W27", "mean_gap": null }
                ]
            })
            .to_string(),
        )
        .unwrap();
        let armed = BarrelCfg {
            enabled: true,
            teachers: vec!["grok".into()],
            ..cfg
        };
        let text = status_text_in(&armed, &still, 1_000);
        assert!(text.contains("armed · teachers: grok"));
        assert!(text.contains("3 shard(s) · 2.0 / 8 MB"));
        assert!(text.contains("dropped 3 (dupe 1, cap 2, backpressure 0)"));
        assert!(text.contains("last tick 100s ago"));
        assert!(text.contains("last distill 2026-W27"));
        assert!(text.contains("2026-W26 gap 6.4 → 2026-W27 unscored"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn still_usage_is_returned_for_unknown_args() {
        let text = run(Some("bogus"));
        assert!(text.starts_with("usage: /still"));
    }

    #[test]
    fn from_env_defaults_are_off_and_bounded() {
        let _guard = crate::tests::env_lock();
        for k in [
            "ANGEL_BARREL",
            "ANGEL_BARREL_TEACHERS",
            "ANGEL_BARREL_DIR",
            "ANGEL_BARREL_MAX_MB",
        ] {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::remove_var(k) };
        }
        let cfg = BarrelCfg::from_env();
        assert!(!cfg.enabled, "capture is default-off");
        assert!(cfg.teachers.is_empty());
        assert_eq!(cfg.max_bytes, 1024 * 1024 * 1024);
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_BARREL", "1") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_BARREL_TEACHERS", " Grok , glm ,") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_BARREL_MAX_MB", "2") };
        let cfg = BarrelCfg::from_env();
        assert!(cfg.enabled);
        assert_eq!(cfg.teachers, vec!["grok".to_string(), "glm".to_string()]);
        assert_eq!(cfg.max_bytes, 2 * 1024 * 1024);
        for k in [
            "ANGEL_BARREL",
            "ANGEL_BARREL_TEACHERS",
            "ANGEL_BARREL_MAX_MB",
        ] {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::remove_var(k) };
        }
    }
}

#[cfg(test)]
mod private_io_tests;
