//! The Cut (T1 + T2 + T4) — what angel authored, whether it built, and `/cut`.
//! Plan: `docs/plans/the-cut.md`.
//!
//! Two labels are attached to every mutation angel makes. This module writes
//! the first one and captures the raw material for the second:
//!
//! * **T1 — the authored-diff manifest.** Every file mutation funnels through
//!   four tools (`write_file`, `str_replace`, `multi_edit`, `apply_patch`),
//!   registered in one place ([`crate::harness::register_file_tools`]). We wrap
//!   them there ([`capture_writes`]) so there is exactly one capture site and no
//!   write path can grow around it — code-mode host calls and delegate seats
//!   dispatch through the same registry, so they are captured too. Records land
//!   in `~/.angel0/cut/authored-YYYYMMDD.jsonl`.
//!
//! * **T2 — the machine verdict.** After a successful mutation the turn loop
//!   ([`crate::harness::post_write_verdict`]) resolves a *cheap, scoped* verify
//!   command for the touched file (`cargo check`, `node --check`, `tsc
//!   --noEmit`, isolated Python compilation — never a test run), runs it under a hard timeout,
//!   feeds a failure back into the tool result so the model self-corrects in the
//!   same turn, and stamps `{"machine": …}` onto the manifest row.
//!
//! * **T4 — `/cut`.** A read-only morning read over the JSONL shards: counts,
//!   machine pass rate, human keep/edited/discarded when those fields exist,
//!   adjacent fail→pass repair trajectories, and compact slices. Observational
//!   only — it never writes the causal graph.
//!
//! Shape of the writer: the capture site *pends* the row in memory and the turn
//! loop [`settle`]s it with the verdict, so the common case is ONE row carrying
//! both the authored text and its machine label. Anything the turn loop never
//! claims (a code-mode script's inner writes) is [`sweep`]-ed out unstamped at
//! the end of the hop — rows are never rewritten, only appended, and a row is
//! never held across turns.
//!
//! Rust NEVER writes the causal graph. This module appends JSONL and nothing
//! else; the Node tick (T3) folds it under the Conductor's lock.
//!
//! Controls: `ANGEL_CUT=0` kills capture; `ANGEL_CUT_DIR` moves the manifest;
//! `ANGEL_CUT_MAX_MB` (default 256) caps total shard bytes (oldest closed shard
//! evicted first, exactly like the barrel); `ANGEL_CUT_VERIFY` selects/kills the
//! verify command; `ANGEL_CUT_VERIFY_TIMEOUT_SECS` (default 60) bounds it.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::club::ToolDef;
use crate::harness::Tool;

mod verification;
pub(crate) use verification::{PostWriteVerification, record_final_verification};

/// Schema version stamped on every manifest row.
const SCHEMA_V: u64 = 1;
/// Verify deadline when `ANGEL_CUT_VERIFY_TIMEOUT_SECS` is unset.
const DEFAULT_VERIFY_TIMEOUT_SECS: u64 = 60;
/// After a verify times out, skip verification for this workspace+command for a
/// while. The priority lane belongs to the agent: instrumentation that starts
/// costing real seconds backs off rather than taxing every write (a repo whose
/// build genuinely takes minutes must not pay that per edit).
const VERIFY_BACKOFF_SECS: u64 = 300;
/// Per-record ceiling on the captured `authored` text. The manifest is evidence
/// for the survival tick, not a file store — a 5 MB generated blob would bloat
/// the shard for no gain. The digest and byte count still describe the FULL text.
const AUTHORED_MAX_BYTES: usize = 64 * 1024;
/// How much of a failing verify's output is kept (in the tool result and in the
/// row). The head, not the tail: compilers print the first — most actionable —
/// error first.
const VERIFY_ERR_MAX_BYTES: usize = 2_500;
/// `/cut` skips a shard larger than this rather than loading it into the TUI.
const STATUS_SHARD_MAX_BYTES: u64 = 32 * 1024 * 1024;
/// A single JSONL line past this bound is treated as torn, not as a row.
const STATUS_LINE_MAX_BYTES: usize = 512 * 1024;
/// Compact slices print at most this many keys per dimension.
const STATUS_SLICE_TOP: usize = 5;
/// Pending rows never survive their hop, but a pathological path (a code-mode
/// script writing in a loop) must not grow the buffer without bound.
const MAX_PENDING: usize = 64;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

pub(crate) struct CutCfg {
    pub(crate) enabled: bool,
    pub(crate) dir: PathBuf,
    pub(crate) max_bytes: u64,
}

impl CutCfg {
    pub(crate) fn from_env() -> Self {
        Self {
            enabled: env_flag("ANGEL_CUT", true),
            dir: std::env::var_os("ANGEL_CUT_DIR")
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| crate::workspace_store::angel_subdir("cut")),
            max_bytes: std::env::var("ANGEL_CUT_MAX_MB")
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .filter(|n| *n > 0)
                .unwrap_or_else(|| crate::store_caps::value("cut", "max_bytes") / (1024 * 1024))
                .saturating_mul(1024 * 1024),
        }
    }
}

/// Permissive boolean env flag, the experience-ledger spelling: unset →
/// `default`; `0`/`false`/`off`/`no` → false; anything else → true.
fn env_flag(key: &str, default: bool) -> bool {
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

// ---------------------------------------------------------------------------
// T1 — the capture chokepoint
// ---------------------------------------------------------------------------

/// Wrap a mutation tool so every *successful* call is captured into the
/// manifest. The wrapper is transparent: same name, same schema, same result —
/// dispatch, `code_mode` binding, and the essential-tool trim all see the inner
/// tool. A capture failure can never fail the write (best-effort, like the
/// experience ledger).
pub(crate) fn capture_writes(inner: Box<dyn Tool>, root: PathBuf) -> Box<dyn Tool> {
    Box::new(CapturedTool { inner, root })
}

struct CapturedTool {
    inner: Box<dyn Tool>,
    root: PathBuf,
}

const TASK_EDITABLE_PATHS_ENV: &str = "ANGEL_TASK_EDITABLE_PATHS_JSON";

fn enforce_task_editable_paths(tool: &str, root: &Path, args: &Value) -> Result<(), String> {
    let Some(raw) = std::env::var_os(TASK_EDITABLE_PATHS_ENV) else {
        return Ok(());
    };
    let raw = raw
        .to_str()
        .ok_or_else(|| format!("{TASK_EDITABLE_PATHS_ENV} is not valid UTF-8"))?;
    let configured: Vec<String> = serde_json::from_str(raw)
        .map_err(|error| format!("invalid {TASK_EDITABLE_PATHS_ENV}: {error}"))?;
    let allowed = configured
        .iter()
        .map(|path| crate::harness::workspace_relative(root, Path::new(path)))
        .collect::<Result<Vec<_>, _>>()?;
    let targets = mutation_targets(tool, args);
    if targets.is_empty() {
        return Err(format!(
            "sealed task could not determine the {tool} target before mutation; edit only one of {configured:?}"
        ));
    }
    for target in targets {
        let relative = crate::harness::workspace_relative(root, Path::new(&target))?;
        let permitted = allowed.iter().any(|path| {
            path.as_os_str().is_empty() || relative == *path || relative.starts_with(path)
        });
        if !permitted {
            return Err(format!(
                "sealed task rejected out-of-scope {tool} target {target:?}; editable paths are {configured:?}"
            ));
        }
    }
    Ok(())
}

impl Tool for CapturedTool {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn def(&self) -> ToolDef {
        self.inner.def()
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        if self.inner.name() != "resolve_edit" {
            enforce_task_editable_paths(self.inner.name(), &self.root, args)?;
        }
        let out = self.inner.call(args);
        if out.is_ok() {
            capture(self.inner.name(), &self.root, args);
        }
        out
    }
}

/// The authored units of one mutation call: `(workspace-relative path, the
/// literal text angel put in the file)`. Pure — the parse the manifest and the
/// verdict both key on.
///
/// `authored` is what the survival tick (T3) looks for in the working tree, so
/// it is always *file text*, never tool syntax: the replacement for an edit, the
/// added lines for a patch. A non-mutating tool yields nothing.
pub(crate) fn authored_units(tool: &str, args: &Value) -> Vec<(String, String)> {
    let path = || args["path"].as_str().unwrap_or_default().to_string();
    match tool {
        "write_file" => match args["content"].as_str() {
            Some(content) if !path().is_empty() => vec![(path(), content.to_string())],
            _ => Vec::new(),
        },
        "str_replace" => match args["new"].as_str() {
            Some(new) if !path().is_empty() => vec![(path(), new.to_string())],
            _ => Vec::new(),
        },
        "multi_edit" => {
            let Some(edits) = args["edits"].as_array() else {
                return Vec::new();
            };
            let new: Vec<&str> = edits.iter().filter_map(|e| e["new"].as_str()).collect();
            if path().is_empty() || new.is_empty() {
                return Vec::new();
            }
            // One row per file, the replacements joined line-wise: the tick
            // scores survival per line, so the join is lossless for its purpose.
            vec![(path(), new.join("\n"))]
        }
        "apply_patch" => match args["diff"].as_str() {
            Some(diff) => patch_units(diff),
            None => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// Visit each path [`mutation_targets`] would return, without allocating
/// authored bodies. `visit` returns true to stop. Apply_patch emits every
/// declared file section, including delete-only and hashline sections; callers
/// that need unique paths must dedup.
pub(crate) fn for_each_mutation_target_path(
    tool: &str,
    args: &Value,
    mut visit: impl FnMut(&str) -> bool,
) {
    match tool {
        "write_file" if args["content"].as_str().is_some() => {
            let path = args["path"].as_str().unwrap_or("");
            if !path.is_empty() {
                let _ = visit(path);
            }
        }
        "str_replace" if args["new"].as_str().is_some() => {
            let path = args["path"].as_str().unwrap_or("");
            if !path.is_empty() {
                let _ = visit(path);
            }
        }
        "multi_edit" => {
            let path = args["path"].as_str().unwrap_or("");
            let Some(edits) = args["edits"].as_array() else {
                return;
            };
            if path.is_empty() || !edits.iter().any(|edit| edit["new"].as_str().is_some()) {
                return;
            }
            let _ = visit(path);
        }
        "apply_patch" => {
            if let Some(diff) = args["diff"].as_str() {
                for_each_apply_patch_target_path(diff, visit);
            }
        }
        _ => {}
    }
}

/// Just the paths a mutation call touches (the settle key, and what the verify
/// resolver types). Does not copy write/patch bodies.
pub(crate) fn mutation_targets(tool: &str, args: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for_each_mutation_target_path(tool, args, |path| {
        if !out.iter().any(|existing| existing == path) {
            out.push(path.to_string());
        }
        false
    });
    out
}

/// Header that starts, replaces, or clears the current apply_patch file.
/// `Some(Some(path))` sets it, `Some(None)` clears it, `None` is not a header.
fn apply_patch_path_header(line: &str) -> Option<Option<&str>> {
    if let Some(rest) = line
        .strip_prefix("*** Add File: ")
        .or_else(|| line.strip_prefix("*** Update File: "))
        .or_else(|| line.strip_prefix("*** Delete File: "))
        .or_else(|| line.strip_prefix("*** Move to: "))
    {
        return Some(Some(rest.trim()));
    }
    if line.starts_with("*** End Patch") {
        return Some(None);
    }
    if let Some(rest) = line
        .strip_prefix("+++ ")
        .or_else(|| line.strip_prefix("--- "))
    {
        let p = rest.split('\t').next().unwrap_or("").trim();
        let p = p.strip_prefix("a/").unwrap_or(p);
        let p = p.strip_prefix("b/").unwrap_or(p);
        return Some((!p.is_empty() && p != "/dev/null").then_some(p));
    }
    if let Some(header) = line
        .strip_prefix('[')
        .and_then(|line| line.strip_suffix(']'))
        && let Some((path, tag)) = header.rsplit_once('#')
        && !path.trim().is_empty()
        && !tag.trim().is_empty()
    {
        return Some(Some(path.trim()));
    }
    None
}

fn for_each_apply_patch_target_path(diff: &str, mut visit: impl FnMut(&str) -> bool) {
    for line in diff.lines() {
        if let Some(Some(path)) = apply_patch_path_header(line)
            && visit(path)
        {
            return;
        }
    }
}

/// The unique apply_patch target path, borrowed from the diff. Multi-file
/// patches return None so classify hops still build a hay.
pub(crate) fn apply_patch_single_target_path(diff: &str) -> Option<&str> {
    let mut found: Option<&str> = None;
    for line in diff.lines() {
        if let Some(Some(path)) = apply_patch_path_header(line) {
            let path = path.trim();
            if path.is_empty() {
                continue;
            }
            match found {
                None => found = Some(path),
                Some(prev) if prev == path => {}
                Some(_) => return None,
            }
        }
    }
    found
}

/// Added lines per file, for both patch dialects `apply_patch` accepts: the
/// Codex freeform envelope (`*** Add File:` / `*** Update File:` with `+` lines)
/// and a plain unified diff (`+++ b/path`, `+` lines). Deletions and context are
/// dropped — only text that ended up *in* the file is authored text.
fn patch_units(diff: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut cur: Option<String> = None;
    let mut push = |file: &Option<String>, line: &str| {
        let Some(file) = file else { return };
        match out.iter_mut().find(|(p, _)| p == file) {
            Some((_, body)) => {
                body.push('\n');
                body.push_str(line);
            }
            None => out.push((file.clone(), line.to_string())),
        }
    };
    for line in diff.lines() {
        if let Some(next) = apply_patch_path_header(line) {
            cur = next.map(str::to_string);
            continue;
        }
        if line.starts_with("+++") || line.starts_with("---") || line.starts_with("@@") {
            continue;
        }
        if let Some(added) = line.strip_prefix('+') {
            push(&cur, added);
        }
    }
    out
}

/// Hot-path capture: build a row per authored unit and pend it for the turn
/// loop to settle. Best-effort and test-silent, exactly the experience ledger's
/// contract — it can never fail or slow a write.
fn capture(tool: &str, root: &Path, args: &Value) {
    if cfg!(test) {
        return;
    }
    let cfg = CutCfg::from_env();
    if !cfg.enabled {
        return;
    }
    let repo = crate::experience::repo_value_for(root);
    let driver = crate::experience::driver_path();
    let ts = now_secs();
    for (path, authored) in authored_units(tool, args) {
        let rec = authored_record(tool, &path, &authored, &repo, &driver, ts, next_seq());
        pend(Pending {
            tool: tool.to_string(),
            path,
            rec,
        });
    }
}

/// Pure record builder (§T1 of the plan). `authored` is redacted with the
/// barrel's redactor and capped; the digest and byte count describe the text as
/// written, so a truncated row still identifies its content exactly.
pub(crate) fn authored_record(
    tool: &str,
    path: &str,
    authored: &str,
    repo: &Value,
    driver: &str,
    ts: u64,
    seq: u64,
) -> Value {
    let (redacted, redacted_lines) = crate::barrel::redact_text(authored);
    let (text, truncated) = cap_text(redacted);
    let mut rec = serde_json::json!({
        "v": SCHEMA_V,
        "kind": "authored",
        "ts": ts,
        "session": std::process::id(),
        "seq": seq,
        "tool": tool,
        "repo": repo,
        "path": path,
        "authored_sha256": sha256_hex(authored.as_bytes()),
        "authored_bytes": authored.len(),
        "authored": text,
        "driver": driver,
        "model": Value::Null,
    });
    if truncated {
        rec["authored_truncated"] = true.into();
    }
    if redacted_lines > 0 {
        rec["redacted_lines"] = redacted_lines.into();
    }
    rec
}

/// Cap on a char boundary, reporting whether anything was dropped.
fn cap_text(mut text: String) -> (String, bool) {
    if text.len() <= AUTHORED_MAX_BYTES {
        return (text, false);
    }
    let mut end = AUTHORED_MAX_BYTES;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    (text, true)
}

// ---------------------------------------------------------------------------
// The pending buffer — one row, both labels
// ---------------------------------------------------------------------------

struct Pending {
    tool: String,
    path: String,
    rec: Value,
}

fn pending() -> &'static Mutex<Vec<Pending>> {
    static CELL: OnceLock<Mutex<Vec<Pending>>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(Vec::new()))
}

fn pend(row: Pending) {
    let Ok(mut buf) = pending().lock() else {
        return;
    };
    // Overflow drains oldest-first, unstamped: a row is never dropped, and the
    // buffer can't grow without bound if some exotic path never settles.
    while buf.len() >= MAX_PENDING {
        let row = buf.remove(0);
        append_row(&row.rec);
    }
    buf.push(row);
}

/// Attach the turn's facts (`hop`, `model`, the machine verdict) to the rows a
/// mutation call pended, and write them. Called once per mutation call from the
/// turn loop's post-write seam. Rows already swept (a nested turn flushed them)
/// simply find no match — the manifest keeps the unstamped row rather than
/// growing a duplicate.
pub(crate) fn settle(
    tool: &str,
    paths: &[String],
    hop: usize,
    route: &crate::club::RouteIdentity,
    machine: Option<&Value>,
) {
    if cfg!(test) {
        return;
    }
    for path in paths {
        let taken = {
            let Ok(mut buf) = pending().lock() else {
                return;
            };
            buf.iter()
                .position(|p| p.tool == tool && p.path == *path)
                .map(|i| buf.remove(i))
        };
        let Some(mut row) = taken else { continue };
        row.rec["hop"] = hop.into();
        // `driver` (stamped at capture) is the configured *path* — the ledger's
        // `ANGEL_DRIVER` sense, e.g. "sota-moa". `route_driver`/`model` are the
        // club that actually served the turn. Keeping both is the experience
        // ledger's own idiom (`path` + `route`), and it stops the tick from
        // attributing a LongCat answer to a "turbo" driver.
        row.rec["route_driver"] = route.driver.as_str().into();
        if let Some(model) = &route.model {
            row.rec["model"] = model.as_str().into();
        }
        if let Some(machine) = machine {
            row.rec["machine"] = machine.clone();
        }
        append_row(&row.rec);
    }
}

/// Write out anything still pending (a code-mode script's inner writes, a call
/// whose postcheck never ran). Called at the end of every tool hop, so a row
/// lives at most one hop in memory.
pub(crate) fn sweep() {
    let rows: Vec<Pending> = match pending().lock() {
        Ok(mut buf) => std::mem::take(&mut buf),
        Err(_) => return,
    };
    for row in rows {
        append_row(&row.rec);
    }
}

/// Append one row to today's shard, evicting the oldest closed shard if the
/// manifest is at its cap (the barrel's shard discipline, shared with it).
fn append_row(rec: &Value) {
    let cfg = CutCfg::from_env();
    if !cfg.enabled {
        return;
    }
    // The authored digest continues to identify the original on-disk source;
    // the manifest is an explicitly scrubbed copy. Cover metadata and attached
    // machine diagnostics as well as the authored text.
    let Ok(body) = crate::secrets::to_redacted_vec(rec) else {
        return;
    };
    let body = String::from_utf8(body).expect("JSON is UTF-8");
    let ts = rec["ts"].as_u64().unwrap_or_else(now_secs);
    // A deferred final check is not another authored fragment. Keep its
    // execution receipt out of authored-corpus readers and bound it separately.
    let verification = rec["kind"] == "verification";
    let prefix = if verification {
        "verification-"
    } else {
        "authored-"
    };
    let shard = cfg
        .dir
        .join(format!("{prefix}{}.jsonl", crate::barrel::utc_yyyymmdd(ts)));
    let mut evicted = 0u64;
    crate::barrel::append_capped_shard(
        &cfg.dir,
        prefix,
        &shard,
        &body,
        if verification {
            (cfg.max_bytes / 16).max(1)
        } else {
            cfg.max_bytes
        },
        &mut evicted,
    );
}

// ---------------------------------------------------------------------------
// T2 — the machine verdict
// ---------------------------------------------------------------------------

/// What the machine said about a write, or why it said nothing.
#[derive(Clone, Debug)]
pub(crate) enum Machine {
    Ran {
        cmd: String,
        /// Where the command ran, relative to the workspace ("." at the root).
        dir: String,
        source: &'static str,
        exit: Option<i32>,
        timed_out: bool,
        dur_ms: u128,
        /// Head of the output when the verify failed; empty when it passed.
        err: String,
    },
    Skipped {
        reason: &'static str,
    },
}

impl Machine {
    pub(crate) fn passed(&self) -> bool {
        matches!(
            self,
            Machine::Ran {
                exit: Some(0),
                timed_out: false,
                ..
            }
        )
    }

    /// The `machine` object stamped onto the manifest row.
    pub(crate) fn to_json(&self) -> Value {
        match self {
            Machine::Ran {
                cmd,
                dir,
                source,
                exit,
                timed_out,
                dur_ms,
                err,
            } => {
                let mut v = serde_json::json!({
                    "cmd": cmd,
                    "dir": dir,
                    "source": source,
                    "exit": exit,
                    "timed_out": timed_out,
                    "dur_ms": *dur_ms as u64,
                });
                if !err.is_empty() {
                    v["err"] = err.as_str().into();
                }
                v
            }
            Machine::Skipped { reason } => serde_json::json!({ "skipped": reason }),
        }
    }

    /// This verdict as `(check identity, passed)`, or `None` when the machine
    /// said nothing.
    ///
    /// The identity is the *check*, not the file: `cargo check` in `cockpit`
    /// covers the whole crate, so every write into that crate produces a verdict
    /// on the same key and the newest one supersedes the older ones. A
    /// file-scoped check (`node --check 'a.js'`) carries its file in the command
    /// and therefore keys itself apart. That is what makes "did this turn end
    /// with anything red?" answerable without guessing which write broke what.
    ///
    /// Only a command that *ran to completion* is a verdict. A skip, a timeout
    /// and a signal-kill are all "no answer" — the plan is explicit that a
    /// timeout must never be read as "your code is broken" — so they yield
    /// `None` and take no part in the reward.
    fn verdict(&self) -> Option<(String, bool)> {
        match self {
            Machine::Ran {
                cmd,
                dir,
                exit: Some(code),
                timed_out: false,
                ..
            } => Some((format!("{dir}|{cmd}"), *code == 0)),
            _ => None,
        }
    }

    /// The failure note appended to the tool result, so the model fixes it in
    /// this turn instead of handing back code that does not compile. Mirrors the
    /// post-edit LSP note's shape.
    pub(crate) fn inline_note(&self) -> Option<String> {
        if self.passed() {
            return None;
        }
        if matches!(self, Machine::Skipped { reason } if reason.starts_with("incomplete-cargo-scaffold"))
        {
            return Some(
                "[post-write verify deferred: Cargo package has no target yet; finish the scaffold. A mandatory check runs before the turn can finish.]".into(),
            );
        }
        // A skipped verify (not source, opted out, backing off) is recorded in
        // the manifest but never editorializes into the turn.
        let Machine::Ran {
            cmd,
            dir,
            exit,
            timed_out,
            err,
            ..
        } = self
        else {
            return None;
        };
        if *timed_out {
            // A verify that outran its deadline is not a verdict — say so, and
            // say nothing about the code.
            return Some(format!(
                "[post-write verify — `{cmd}` timed out; verification skipped for now]"
            ));
        }
        let status = exit
            .map(|c| format!("exit {c}"))
            .unwrap_or_else(|| "killed by signal".to_string());
        Some(format!(
            "[post-write verify — `{cmd}` failed in {dir} ({status}); fix this before continuing]\n{err}"
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct VerifyPlan {
    pub(crate) cmd: String,
    /// Workspace-relative directory the command runs in.
    pub(crate) dir: String,
    /// Where the command came from: env | dossier | default.
    pub(crate) source: &'static str,
}

pub(crate) enum Verify {
    Off,
    Skip(&'static str),
    Run(VerifyPlan),
}

/// Precedence (§T2 of the plan): `ANGEL_CUT_VERIFY` → the dossier's learned
/// build ritual → the project-type default for the edited file.
///
/// Two cheapness rules are load-bearing and deliberate:
/// * only *build/type* checks, never a test run — a `cargo test` per write makes
///   angel unusable, and that is the failure mode this whole phase must avoid;
/// * only source files — a docs edit does not earn a compile.
pub(crate) fn resolve_verify(workspace: &Path, targets: &[String]) -> Verify {
    if let Ok(raw) = std::env::var("ANGEL_CUT_VERIFY") {
        if !env_flag("ANGEL_CUT_VERIFY", true) {
            return Verify::Off;
        }
        let cmd = raw.trim();
        if !cmd.is_empty() {
            return Verify::Run(VerifyPlan {
                cmd: cmd.to_string(),
                dir: ".".to_string(),
                source: "env",
            });
        }
    }
    // This resolver selects one source target. PostWriteVerification calls it
    // separately for every target, then shares only identical concrete plans.
    let Some(target) = targets.iter().find(|p| source_kind(p).is_some()) else {
        return Verify::Skip("not-source");
    };
    if let Some(cmd) = crate::dossier::build_ritual(workspace) {
        let dir = ritual_dir(&cmd, workspace, target);
        return Verify::Run(VerifyPlan {
            cmd,
            dir,
            source: "dossier",
        });
    }
    match default_plan(workspace, target) {
        Some(plan) => Verify::Run(plan),
        None => Verify::Skip("no-verify-command"),
    }
}

/// The language a path implies, or `None` when the file is not source (docs,
/// data, assets — the majority of what a coding agent also writes).
fn source_kind(path: &str) -> Option<&'static str> {
    match Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
    {
        "rs" => Some("rust"),
        "ts" | "tsx" => Some("ts"),
        "js" | "jsx" | "mjs" | "cjs" => Some("js"),
        "py" => Some("py"),
        "toml" if path.ends_with("Cargo.toml") => Some("rust"),
        _ => None,
    }
}

/// The project-type default: the cheapest command that still returns a real
/// verdict on the edited file.
fn default_plan(workspace: &Path, target: &str) -> Option<VerifyPlan> {
    let kind = source_kind(target)?;
    let plan = |cmd: String, dir: String| VerifyPlan {
        cmd,
        dir,
        source: "default",
    };
    match kind {
        // Nearest enclosing crate, not the workspace root: this repo's Cargo
        // manifest lives in `cockpit/`, and a root-only probe would find nothing
        // and never verify anything.
        "rust" => nearest_marker(workspace, target, "Cargo.toml")
            .map(|dir| plan("cargo check".to_string(), dir)),
        "ts" => nearest_marker(workspace, target, "tsconfig.json")
            .map(|dir| plan("npx --no-install tsc --noEmit".to_string(), dir)),
        // A syntax check is what `node`/`python` can give for free; anything
        // deeper is a test run, which is out of budget by construction.
        "js" => Some(plan(
            format!("node --check {}", shell_quote(target)),
            ".".into(),
        )),
        "py" => Some(plan(
            // py_compile writes __pycache__ outside the edited file set. The
            // fixed bootstrap parses source bytes without executing the module;
            // isolation suppresses project/user startup hooks and bytecode.
            format!(
                "python3 -I -S -B -c {} {}",
                shell_quote(
                    "import pathlib, sys; compile(pathlib.Path(sys.argv[1]).read_bytes(), sys.argv[1], 'exec')"
                ),
                shell_quote(target),
            ),
            ".".into(),
        )),
        _ => None,
    }
}

/// Where a dossier build ritual should run. A ritual carries the command but
/// not the directory it was observed in, and cargo is manifest-anchored:
/// running it at a workspace root whose crate lives in a subdir (this repo:
/// `cockpit/`) fails with "could not find Cargo.toml" in milliseconds — a
/// phantom red that gaslights the model mid-run and stamps false machine
/// labels into the substrate (five in a row on the first dogfood run). Anchor
/// cargo rituals at the edited file's nearest crate; other commands keep the
/// root, where repo-level tools (make, npm scripts) actually live.
fn ritual_dir(cmd: &str, workspace: &Path, target: &str) -> String {
    if cmd.trim_start().starts_with("cargo")
        && let Some(dir) = nearest_marker(workspace, target, "Cargo.toml")
    {
        return dir;
    }
    ".".to_string()
}

/// A failure that convicts the verify setup, not the code: the command itself
/// is missing, or cargo could not even locate a manifest where it ran. These
/// must never become red machine verdicts — the write was not judged at all.
fn environment_failure(exit: Option<i32>, err: &str) -> bool {
    exit == Some(127)
        || err.contains("could not find `Cargo.toml`")
        || err.contains("command not found")
}

/// Walk up from `target`'s directory to the workspace root looking for `marker`;
/// returns the workspace-relative directory that has it.
fn nearest_marker(workspace: &Path, target: &str, marker: &str) -> Option<String> {
    let mut dir = Path::new(target).parent();
    loop {
        let rel = dir.unwrap_or_else(|| Path::new(""));
        if workspace.join(rel).join(marker).is_file() {
            let rel = rel.to_string_lossy().to_string();
            return Some(if rel.is_empty() { ".".to_string() } else { rel });
        }
        match dir.and_then(Path::parent) {
            Some(parent) => dir = Some(parent),
            None => return None,
        }
    }
}

/// Single-quote a path for `sh -c` (paths from the model are workspace-relative
/// but not otherwise trusted to be shell-clean).
fn shell_quote(path: &str) -> String {
    format!("'{}'", path.replace('\'', r"'\''"))
}

fn verify_timeout() -> Duration {
    let secs = std::env::var("ANGEL_CUT_VERIFY_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_VERIFY_TIMEOUT_SECS)
        .clamp(5, 600);
    Duration::from_secs(secs)
}

/// Backoff ledger: a command that blew its deadline once is skipped for
/// [`VERIFY_BACKOFF_SECS`] rather than taxing every subsequent write.
fn backoff() -> &'static Mutex<HashMap<String, Instant>> {
    static CELL: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(HashMap::new()))
}

fn run_verify(
    workspace: &Path,
    plan: VerifyPlan,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Machine {
    if cancel.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed)) {
        return Machine::Skipped {
            reason: "cancelled",
        };
    }
    let key = format!("{}|{}|{}", workspace.display(), plan.dir, plan.cmd);
    if let Ok(map) = backoff().lock()
        && let Some(until) = map.get(&key)
        && Instant::now() < *until
    {
        return Machine::Skipped {
            reason: "backoff-after-timeout",
        };
    }
    let started = Instant::now();
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(&plan.cmd)
        .current_dir(workspace.join(&plan.dir));
    let Ok(capture) = crate::harness::exec::output_timed_captured_cancellable(
        cmd,
        Some(verify_timeout()),
        cancel,
    ) else {
        return Machine::Skipped {
            reason: "spawn-failed",
        };
    };
    // An interrupted check says nothing about the candidate. In particular,
    // do not train on a false failure or back off a healthy verifier.
    if capture.cancelled {
        return Machine::Skipped {
            reason: "cancelled",
        };
    }
    let output = capture.output;
    let timed_out = capture.timed_out;
    if timed_out && let Ok(mut map) = backoff().lock() {
        map.insert(
            key,
            Instant::now() + Duration::from_secs(VERIFY_BACKOFF_SECS),
        );
    }
    let exit = output.status.code();
    let failed = timed_out || exit != Some(0);
    let err = if failed {
        // stderr first — every one of these tools reports diagnostics there.
        let mut combined = String::from_utf8_lossy(&output.stderr).into_owned();
        if combined.trim().is_empty() {
            combined = String::from_utf8_lossy(&output.stdout).into_owned();
        }
        let (head, _) = cap_head(
            crate::barrel::redact_text(&combined).0,
            VERIFY_ERR_MAX_BYTES,
        );
        head
    } else {
        String::new()
    };
    if failed && !timed_out && environment_failure(exit, &err) {
        return Machine::Skipped {
            reason: "verify-misconfigured",
        };
    }
    Machine::Ran {
        cmd: plan.cmd,
        dir: plan.dir,
        source: plan.source,
        exit,
        timed_out,
        dur_ms: started.elapsed().as_millis(),
        err,
    }
}

/// Keep the head of a diagnostic (the first error is the actionable one).
fn cap_head(mut text: String, max: usize) -> (String, bool) {
    if text.len() <= max {
        return (text.trim_end().to_string(), false);
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text.push_str("\n…[verify output truncated]");
    (text, true)
}

// ---------------------------------------------------------------------------
// The turn reward — the machine verdict, turned into training signal
// ---------------------------------------------------------------------------
//
// `~/.angel0/trajectories` held 204 rollouts and ZERO reward labels (measured
// 2026-07-11). A LoRA trained on unlabeled rollouts is pure imitation: the
// student converges on a cheaper copy of the teacher and cannot, by
// construction, exceed it. `trajectory.rs` has carried `reward: Option<f32>`
// since the beginning — "a reward-labeled record is training data; an unlabeled
// one is just a log" — and nothing outside the coding-eval path ever had a
// reward to put in it.
//
// T2 produced one. Every source write angel makes now earns a real exit code
// from a real compiler, and a turn is a sequence of those verdicts. This is the
// function that turns that sequence into the number the forge trains on.

/// The machine verdicts one turn earned, in the order it earned them.
///
/// Accumulated by the turn loop's post-write seam
/// ([`crate::harness::post_write_verdict`]) and read at every exit of `run_turn`.
/// Per-turn state: it is a plain local, so parallel turns (delegate seats,
/// spawned subagents) each score their own rollout and never bleed into one
/// another.
#[derive(Default)]
pub(crate) struct TurnVerdicts {
    /// `(check identity, passed)`, oldest first. See [`Machine::verdict`].
    seen: Vec<(String, bool)>,
    /// A later mutation invalidates an earlier pass until this check completes.
    /// Cancellation, timeout and incomplete scaffolding cannot inherit green.
    pending: std::collections::HashSet<String>,
}

impl TurnVerdicts {
    pub(crate) fn pending_note(&self) -> Option<String> {
        (!self.pending.is_empty()).then(|| {
            format!(
                "[post-write verification unverified: {} changed check scope(s) have no completed current verdict]",
                self.pending.len()
            )
        })
    }

    pub(crate) fn invalidate(&mut self, plan: &VerifyPlan) {
        self.pending.insert(format!("{}|{}", plan.dir, plan.cmd));
    }

    /// Fold in one post-write verdict. Skips, timeouts and "verify is off" add
    /// nothing — they are not labels.
    pub(crate) fn observe(&mut self, machine: &Machine) {
        if let Some(v) = machine.verdict() {
            self.pending.remove(&v.0);
            self.seen.push(v);
        }
    }

    /// This turn's reward, or `None` if the turn earned no label.
    pub(crate) fn reward(&self) -> Option<f32> {
        if self.pending.is_empty() {
            turn_reward(&self.seen)
        } else {
            None
        }
    }
}

/// **The reward of a turn: the share of angel's source writes that compiled —
/// and zero if it walked away from a red check.**
///
/// Given every machine verdict the turn earned, oldest first:
///
/// | outcome | reward | what it means |
/// |---|---|---|
/// | no verdicts | `None` | the turn wrote no source (prose, research, a docs edit), or verification was off/skipped/timed out. **Not a label. Never invent one** — a fabricated label is worse than no label, and this is the majority of turns. |
/// | every check green | `1.0` | angel wrote code and never broke anything. |
/// | broke it, then fixed it | `passes / verdicts`, strictly in `(0.0, 1.0)` | angel broke the build and **repaired it inside the same turn**. |
/// | left a check red | `0.0` | at least one check's *last* word was a failure: angel broke something and stopped anyway. |
///
/// Why this shape, and not a hand-tuned constant per case:
///
/// * **The repair band is the point.** A fail → edit → pass turn is a
///   demonstrated recovery, and it is exactly the trajectory shape RL wants to
///   reinforce (the entire experience ledger contains *two* of them). It must
///   therefore score strictly **above** the walk-away — otherwise the policy
///   learns that the safe move after breaking the build is to abandon it — and
///   strictly **below** a clean turn, or breaking the build becomes free and a
///   policy could farm reward by arson. `passes / verdicts` gives both for free:
///   a recovered turn always contains at least one pass (so it is `> 0`) and at
///   least one failure (so it is `< 1`).
/// * **Thrash is priced, but never fatal.** One break repaired = `0.5`; three
///   breaks repaired = `0.25`. Monotonically worse the more it flailed, yet it
///   can never sink to the walk-away's `0.0`, because the last thing it did was
///   turn the build green.
/// * **No magic numbers.** Every value above falls out of counting. The only
///   editorial decision is the hard `0.0` rail for ending red, and that is the
///   whole point of The Cut: the experience ledger already reports `ok: true` on
///   96% of turns because `ok` means "did not crash". This is the first signal in
///   the repo that can say *no*.
///
/// "Ended red" is judged **per check** (`cargo check` in `cockpit`, `node --check
/// a.js`), not per file, because that is the scope the command actually
/// adjudicates: a crate-wide check failing on a write to `a.rs` and passing after
/// a write to `b.rs` — a repair that fixed the *caller* — is a genuine recovery,
/// and per-file bookkeeping would misread it as an abandoned file.
fn turn_reward(seen: &[(String, bool)]) -> Option<f32> {
    if seen.is_empty() {
        return None; // no verdict → no label. The line the whole design holds.
    }
    // The last word of each distinct check is the state angel left it in.
    let left_red = seen
        .iter()
        .any(|(key, _)| matches!(seen.iter().rev().find(|(k, _)| k == key), Some((_, false))));
    if left_red {
        return Some(0.0);
    }
    let passes = seen.iter().filter(|(_, ok)| *ok).count();
    Some(passes as f32 / seen.len() as f32)
}

// ---------------------------------------------------------------------------
// Small shared helpers
// ---------------------------------------------------------------------------

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Monotonic per-process record sequence — the same join key the experience
/// ledger stamps (`session` = pid, `seq` = this counter), so a manifest row and
/// a turn record can be lined up by the Node tick.
fn next_seq() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(1);
    SEQ.fetch_add(1, Ordering::Relaxed)
}

/// Stable SHA-256 content identity using the already-linked platform implementation.
pub(crate) fn sha256_hex(data: &[u8]) -> String {
    use std::fmt::Write as _;
    let digest = ring::digest::digest(&ring::digest::SHA256, data);
    let mut hex = String::with_capacity(64);
    for byte in digest.as_ref() {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

// ---------------------------------------------------------------------------
// T4 — `/cut` morning read
// ---------------------------------------------------------------------------

const CUT_USAGE: &str = "usage: /cut — morning read of the authored-diff manifest \
(machine pass rate, keep/edited/discarded, slices by driver/model/repo/path/hour)";

/// The `/cut` command body: counts and metadata from `authored-YYYYMMDD.jsonl`.
/// Observational only — never writes the graph, never loads `authored` bodies
/// into the summary, never ranks a driver as better.
pub(crate) fn status_text(arg: Option<&str>, workspace: &Path) -> String {
    if arg
        .map(str::trim)
        .is_some_and(|a| a.eq_ignore_ascii_case("help"))
    {
        return CUT_USAGE.to_string();
    }
    let cfg = CutCfg::from_env();
    if !cfg.enabled {
        return format!(
            "the cut is disabled (ANGEL_CUT=0)\n\
             manifest: {}\n\
             signal density is zero — capture is off",
            cfg.dir.display()
        );
    }
    morning_read(&cfg.dir, workspace)
}

fn morning_read(dir: &Path, workspace: &Path) -> String {
    let mut out = format!(
        "the cut — morning read\nworkspace: {}\n",
        workspace.display()
    );
    if !dir.exists() {
        out.push_str(&format!(
            "manifest: {} (missing — signal density is zero)\n",
            dir.display()
        ));
        push_zero_body(&mut out);
        return out;
    }
    let shards = authored_shards(dir);
    if shards.is_empty() {
        out.push_str(&format!(
            "manifest: {} (empty — signal density is zero)\n",
            dir.display()
        ));
        push_zero_body(&mut out);
        return out;
    }

    let mut authored = 0u64;
    let mut labeled = 0u64;
    let mut passed = 0u64;
    let mut skipped = 0u64;
    let mut timed_out = 0u64;
    let mut unverified = 0u64;
    let mut kept = 0u64;
    let mut edited = 0u64;
    let mut discarded = 0u64;
    let mut human_fields = false;
    let mut huge_shards = 0u64;
    let mut repair_hits: Vec<RepairHit> = Vec::new();
    let mut labeled_unpairable = 0u64;
    let mut drivers: HashMap<String, u64> = HashMap::new();
    let mut models: HashMap<String, u64> = HashMap::new();
    let mut repos: HashMap<String, u64> = HashMap::new();
    let mut paths: HashMap<String, u64> = HashMap::new();
    let mut hours: HashMap<String, u64> = HashMap::new();

    for shard in &shards {
        let len = match std::fs::metadata(shard) {
            Ok(meta) => meta.len(),
            Err(_) => continue,
        };
        if len > STATUS_SHARD_MAX_BYTES {
            huge_shards += 1;
            continue;
        }
        let Ok(file) = File::open(shard) else {
            continue;
        };
        for line in BufReader::new(file).lines() {
            let Ok(line) = line else { continue };
            if line.trim().is_empty() {
                continue;
            }
            if line.len() > STATUS_LINE_MAX_BYTES {
                continue;
            }
            let Ok(mut v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if !v.is_object() {
                continue;
            }
            if let Some(obj) = v.as_object_mut() {
                obj.remove("authored");
            }
            authored += 1;
            bump_slice(&mut drivers, string_field(&v, "driver"));
            bump_slice(&mut models, string_field(&v, "model"));
            bump_slice(
                &mut repos,
                v.get("repo")
                    .and_then(|r| r.get("key"))
                    .and_then(|k| k.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty()),
            );
            bump_slice(
                &mut paths,
                path_kind(v.get("path").and_then(|p| p.as_str())),
            );
            if let Some(ts) = v.get("ts").and_then(|t| t.as_u64()) {
                let hour = hour_utc(ts);
                bump_slice(&mut hours, Some(&hour));
            }
            match machine_stat(&v) {
                MachineStat::Skipped => skipped += 1,
                MachineStat::TimedOut => timed_out += 1,
                MachineStat::Pass => {
                    labeled += 1;
                    passed += 1;
                    note_repair(&v, true, &mut repair_hits, &mut labeled_unpairable);
                }
                MachineStat::Fail => {
                    labeled += 1;
                    note_repair(&v, false, &mut repair_hits, &mut labeled_unpairable);
                }
                MachineStat::Unverified => unverified += 1,
            }
            match human_of(&v) {
                Human::Absent => {}
                Human::Keep => {
                    human_fields = true;
                    kept += 1;
                }
                Human::Edited => {
                    human_fields = true;
                    edited += 1;
                }
                Human::Discarded => {
                    human_fields = true;
                    discarded += 1;
                }
                Human::Other => human_fields = true,
            }
        }
    }

    out.push_str(&format!("manifest: {} · {authored} row(s)", dir.display()));
    if huge_shards > 0 {
        out.push_str(&format!(" · skipped {huge_shards} oversized shard(s)"));
    }
    if authored == 0 {
        out.push_str(" — signal density is zero");
    }
    out.push('\n');
    out.push_str(&format!("  authored: {authored}\n"));
    out.push_str(&format!(
        "  machine: labeled {labeled} · skipped {skipped} · timed_out {timed_out} · unverified {unverified}\n"
    ));
    if labeled == 0 {
        out.push_str("  pass rate: n/a (no labeled rows)\n");
    } else {
        out.push_str(&format!("  pass rate: {passed}/{labeled}\n"));
    }
    if human_fields {
        let judged = kept + edited + discarded;
        out.push_str(&format!(
            "  human: KEPT {kept} / EDITED {edited} / DISCARDED {discarded} (judged {judged})\n"
        ));
    } else {
        out.push_str("  judged corpus is empty\n");
    }
    let pairable = labeled.saturating_sub(labeled_unpairable);
    if labeled > 0 && pairable == 0 {
        out.push_str("  repair trajectories: n/a (manifest has no fail→pass pairing)\n");
    } else {
        out.push_str(&format!(
            "  repair trajectories: {}\n",
            count_repairs(&mut repair_hits)
        ));
    }
    let judged = if human_fields {
        kept + edited + discarded
    } else {
        0
    };
    out.push_str(&format!(
        "  signal density: {authored} authored · {labeled} labeled · {judged} judged\n"
    ));
    out.push_str("  slices (counts only; observational — not a ranking)\n");
    out.push_str(&format!("    driver: {}\n", format_slice(&drivers)));
    out.push_str(&format!("    model: {}\n", format_slice(&models)));
    out.push_str(&format!("    repo: {}\n", format_slice(&repos)));
    out.push_str(&format!("    path: {}\n", format_slice(&paths)));
    out.push_str(&format!("    hour: {}", format_slice(&hours)));
    out
}

fn push_zero_body(out: &mut String) {
    out.push_str("  authored: 0\n");
    out.push_str("  machine: labeled 0 · skipped 0 · timed_out 0 · unverified 0\n");
    out.push_str("  pass rate: n/a (no labeled rows)\n");
    out.push_str("  judged corpus is empty\n");
    out.push_str("  repair trajectories: 0\n");
    out.push_str("  signal density: 0 authored · 0 labeled · 0 judged\n");
    out.push_str("  slices (counts only; observational — not a ranking)\n");
    out.push_str("    driver: (empty)\n");
    out.push_str("    model: (empty)\n");
    out.push_str("    repo: (empty)\n");
    out.push_str("    path: (empty)\n");
    out.push_str("    hour: (empty)");
}

fn authored_shards(dir: &Path) -> Vec<PathBuf> {
    let mut names = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return names;
    };
    for ent in rd.flatten() {
        let name = ent.file_name();
        let Some(s) = name.to_str() else { continue };
        if !s.starts_with("authored-") || !s.ends_with(".jsonl") {
            continue;
        }
        let date = &s["authored-".len()..s.len() - ".jsonl".len()];
        if date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()) {
            names.push(ent.path());
        }
    }
    names.sort();
    names
}

fn string_field<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn bump_slice(map: &mut HashMap<String, u64>, key: Option<&str>) {
    let Some(key) = key.filter(|s| !s.is_empty()) else {
        return;
    };
    *map.entry(key.to_string()).or_insert(0) += 1;
}

fn path_kind(path: Option<&str>) -> Option<&str> {
    let path = path.filter(|s| !s.is_empty())?;
    if let Some(kind) = source_kind(path) {
        return Some(kind);
    }
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| !e.is_empty())
}

fn hour_utc(ts: u64) -> String {
    format!("{:02}h", (ts / 3600) % 24)
}

fn format_slice(counts: &HashMap<String, u64>) -> String {
    if counts.is_empty() {
        return "(empty)".to_string();
    }
    let mut items: Vec<(&String, &u64)> = counts.iter().collect();
    items.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    let extra = items.len() > STATUS_SLICE_TOP;
    items.truncate(STATUS_SLICE_TOP);
    let body = items
        .into_iter()
        .map(|(k, n)| format!("{k} {n}"))
        .collect::<Vec<_>>()
        .join(", ");
    if extra { format!("{body} …") } else { body }
}

#[derive(Clone, Copy)]
enum MachineStat {
    Unverified,
    Skipped,
    TimedOut,
    Pass,
    Fail,
}

fn machine_stat(v: &Value) -> MachineStat {
    let Some(m) = v.get("machine") else {
        return MachineStat::Unverified;
    };
    if !m.is_object() {
        return MachineStat::Unverified;
    }
    if m.get("skipped").is_some() {
        return MachineStat::Skipped;
    }
    if m.get("timed_out").and_then(|t| t.as_bool()) == Some(true) {
        return MachineStat::TimedOut;
    }
    match m.get("exit").and_then(|e| e.as_i64()) {
        Some(0) => MachineStat::Pass,
        Some(_) => MachineStat::Fail,
        None => MachineStat::Unverified,
    }
}

#[derive(Clone, Copy)]
enum Human {
    Absent,
    Keep,
    Edited,
    Discarded,
    Other,
}

fn human_of(v: &Value) -> Human {
    if let Some(s) = v
        .pointer("/cut/verdict")
        .and_then(|x| x.as_str())
        .or_else(|| v.get("verdict").and_then(|x| x.as_str()))
    {
        return classify_human(s);
    }
    let keep = v.get("keep").or_else(|| v.get("kept"));
    let edited = v.get("edited");
    let discarded = v.get("discarded");
    if keep.is_none() && edited.is_none() && discarded.is_none() {
        return Human::Absent;
    }
    if truthy(keep) {
        return Human::Keep;
    }
    if truthy(edited) {
        return Human::Edited;
    }
    if truthy(discarded) {
        return Human::Discarded;
    }
    Human::Other
}

fn classify_human(s: &str) -> Human {
    match s.trim().to_ascii_uppercase().as_str() {
        "KEPT" | "KEEP" => Human::Keep,
        "EDITED" | "EDIT" => Human::Edited,
        "DISCARDED" | "DISCARD" => Human::Discarded,
        _ => Human::Other,
    }
}

fn truthy(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_i64().is_some_and(|i| i != 0),
        Some(Value::String(s)) => {
            let t = s.trim();
            !t.is_empty()
                && !t.eq_ignore_ascii_case("false")
                && t != "0"
                && !t.eq_ignore_ascii_case("no")
        }
        _ => false,
    }
}

struct RepairHit {
    session: u64,
    path: String,
    cmd: String,
    ts: u64,
    seq: u64,
    pass: bool,
}

fn note_repair(v: &Value, pass: bool, hits: &mut Vec<RepairHit>, unpairable: &mut u64) {
    let session = v.get("session").and_then(|s| s.as_u64());
    let path = v
        .get("path")
        .and_then(|p| p.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let cmd = v
        .get("machine")
        .and_then(|m| m.get("cmd"))
        .and_then(|c| c.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let (Some(session), Some(path), Some(cmd)) = (session, path, cmd) else {
        *unpairable += 1;
        return;
    };
    hits.push(RepairHit {
        session,
        path: path.to_string(),
        cmd: cmd.to_string(),
        ts: v.get("ts").and_then(|t| t.as_u64()).unwrap_or(0),
        seq: v.get("seq").and_then(|s| s.as_u64()).unwrap_or(0),
        pass,
    });
}

fn count_repairs(hits: &mut [RepairHit]) -> u64 {
    hits.sort_by(|a, b| {
        a.session
            .cmp(&b.session)
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.cmd.cmp(&b.cmd))
            .then_with(|| a.ts.cmp(&b.ts))
            .then_with(|| a.seq.cmp(&b.seq))
    });
    let mut n = 0u64;
    for w in hits.windows(2) {
        if w[0].session == w[1].session
            && w[0].path == w[1].path
            && w[0].cmd == w[1].cmd
            && !w[0].pass
            && w[1].pass
        {
            n += 1;
        }
    }
    n
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/cut__tests.rs"]
mod tests;
