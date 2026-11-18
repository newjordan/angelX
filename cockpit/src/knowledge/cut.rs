//! The Cut (T1 + T2 + T4) — what angel authored, whether it built, and `/cut`.
//! Plan: `docs/plans/the-cut.md`.
//!
//! Two labels are attached to every mutation angel makes. This module writes
//! the first one and captures the raw material for the second:
//!
//! * **T1 — the authored-diff manifest.** Every file mutation funnels through
//!   four tools (`write_file`, `str_replace`, `multi_edit`, `apply_patch`),
//!   registered in one place ([`crate::agent::harness::register_file_tools`]). We wrap
//!   them there ([`capture_writes`]) so there is exactly one capture site and no
//!   write path can grow around it — code-mode host calls and delegate seats
//!   dispatch through the same registry, so they are captured too. Records land
//!   in `~/.angelX/cut/authored-YYYYMMDD.jsonl`.
//!
//! * **T2 — the machine verdict.** After a successful mutation the turn loop
//!   ([`crate::agent::harness::post_write_verdict`]) resolves a *cheap, scoped* verify
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
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::agent::club::ToolDef;
use crate::agent::harness::Tool;

mod verification;
pub(crate) use verification::{PostWriteVerification, record_final_verification};

const SCHEMA_V: u64 = 1;
const DEFAULT_VERIFY_TIMEOUT_SECS: u64 = 60;
const VERIFY_BACKOFF_SECS: u64 = 300;
const AUTHORED_MAX_BYTES: usize = 64 * 1024;
const VERIFY_ERR_MAX_BYTES: usize = 2_500;
const MAX_PENDING: usize = 64;

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
                .unwrap_or_else(|| crate::platform::workspace_store::angel_subdir("cut")),
            max_bytes: std::env::var("ANGEL_CUT_MAX_MB")
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .filter(|n| *n > 0)
                .unwrap_or_else(|| {
                    crate::platform::store_caps::value("cut", "max_bytes") / (1024 * 1024)
                })
                .saturating_mul(1024 * 1024),
        }
    }
}

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
        .map(|path| crate::agent::harness::workspace_relative(root, Path::new(path)))
        .collect::<Result<Vec<_>, _>>()?;
    let targets = mutation_targets(tool, args);
    if targets.is_empty() {
        return Err(format!(
            "sealed task could not determine the {tool} target before mutation; edit only one of {configured:?}"
        ));
    }
    for target in targets {
        let relative = crate::agent::harness::workspace_relative(root, Path::new(&target))?;
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
            vec![(path(), new.join("\n"))]
        }
        "apply_patch" => match args["diff"].as_str() {
            Some(diff) => patch_units(diff),
            None => Vec::new(),
        },
        _ => Vec::new(),
    }
}

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

fn capture(tool: &str, root: &Path, args: &Value) {
    if cfg!(test) {
        return;
    }
    let cfg = CutCfg::from_env();
    if !cfg.enabled {
        return;
    }
    let repo = crate::knowledge::experience::repo_value_for(root);
    let driver = crate::knowledge::experience::driver_path();
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

pub(crate) fn authored_record(
    tool: &str,
    path: &str,
    authored: &str,
    repo: &Value,
    driver: &str,
    ts: u64,
    seq: u64,
) -> Value {
    let (redacted, redacted_lines) = crate::knowledge::barrel::redact_text(authored);
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
    while buf.len() >= MAX_PENDING {
        let row = buf.remove(0);
        append_row(&row.rec);
    }
    buf.push(row);
}

pub(crate) fn settle(
    tool: &str,
    paths: &[String],
    hop: usize,
    route: &crate::agent::club::RouteIdentity,
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

pub(crate) fn sweep() {
    let rows: Vec<Pending> = match pending().lock() {
        Ok(mut buf) => std::mem::take(&mut buf),
        Err(_) => return,
    };
    for row in rows {
        append_row(&row.rec);
    }
}

fn append_row(rec: &Value) {
    let cfg = CutCfg::from_env();
    if !cfg.enabled {
        return;
    }
    let Ok(body) = crate::platform::secrets::to_redacted_vec(rec) else {
        return;
    };
    let body = String::from_utf8(body).expect("JSON is UTF-8");
    let ts = rec["ts"].as_u64().unwrap_or_else(now_secs);
    let verification = rec["kind"] == "verification";
    let prefix = if verification {
        "verification-"
    } else {
        "authored-"
    };
    let shard = cfg.dir.join(format!(
        "{prefix}{}.jsonl",
        crate::knowledge::barrel::utc_yyyymmdd(ts)
    ));
    let mut evicted = 0u64;
    crate::knowledge::barrel::append_capped_shard(
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

#[derive(Clone, Debug)]
pub(crate) enum Machine {
    Ran {
        cmd: String,
        dir: String,
        source: &'static str,
        exit: Option<i32>,
        timed_out: bool,
        dur_ms: u128,
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

    pub(crate) fn inline_note(&self) -> Option<String> {
        if self.passed() {
            return None;
        }
        if matches!(self, Machine::Skipped { reason } if reason.starts_with("incomplete-cargo-scaffold"))
        {
            return Some("post-write check deferred: the Cargo package has no target yet".into());
        }
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
            return Some(format!("`{cmd}` timed out; no verdict"));
        }
        let status = exit
            .map(|c| format!("exit {c}"))
            .unwrap_or_else(|| "killed by signal".to_string());
        Some(format!("`{cmd}` failed in {dir} ({status})\n{err}"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct VerifyPlan {
    pub(crate) cmd: String,
    pub(crate) dir: String,
    pub(crate) source: &'static str,
}

pub(crate) enum Verify {
    Off,
    Skip(&'static str),
    Run(VerifyPlan),
}

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
    let Some(target) = targets.iter().find(|p| source_kind(p).is_some()) else {
        return Verify::Skip("not-source");
    };
    if let Some(cmd) = crate::knowledge::dossier::build_ritual(workspace) {
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

fn default_plan(workspace: &Path, target: &str) -> Option<VerifyPlan> {
    let kind = source_kind(target)?;
    let plan = |cmd: String, dir: String| VerifyPlan {
        cmd,
        dir,
        source: "default",
    };
    match kind {
        "rust" => nearest_marker(workspace, target, "Cargo.toml")
            .map(|dir| plan("cargo check".to_string(), dir)),
        "ts" => nearest_marker(workspace, target, "tsconfig.json")
            .map(|dir| plan("npx --no-install tsc --noEmit".to_string(), dir)),
        "js" => Some(plan(
            format!("node --check {}", shell_quote(target)),
            ".".into(),
        )),
        "py" => Some(plan(
            format!(
                "python3 -I -S -B -c {} {}",
                shell_quote(
                    "import sys; compile(open(sys.argv[1], 'rb').read(), sys.argv[1], 'exec')"
                ),
                shell_quote(target),
            ),
            ".".into(),
        )),
        _ => None,
    }
}

fn ritual_dir(cmd: &str, workspace: &Path, target: &str) -> String {
    if cmd.trim_start().starts_with("cargo")
        && let Some(dir) = nearest_marker(workspace, target, "Cargo.toml")
    {
        return dir;
    }
    ".".to_string()
}

fn environment_failure(exit: Option<i32>, err: &str) -> bool {
    exit == Some(127)
        || err.contains("could not find `Cargo.toml`")
        || err.contains("command not found")
}

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
    let Ok(cmd) = crate::agent::harness::exec::sandboxed_workspace_sh(
        &plan.cmd,
        &workspace.join(&plan.dir),
        workspace,
    ) else {
        return Machine::Skipped {
            reason: "sandbox-unavailable",
        };
    };
    let Ok(capture) = crate::agent::harness::exec::output_timed_captured_cancellable(
        cmd,
        Some(verify_timeout()),
        cancel,
    ) else {
        return Machine::Skipped {
            reason: "spawn-failed",
        };
    };
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
        let mut combined = String::from_utf8_lossy(&output.stderr).into_owned();
        if combined.trim().is_empty() {
            combined = String::from_utf8_lossy(&output.stdout).into_owned();
        }
        let (head, _) = cap_head(
            crate::knowledge::barrel::redact_text(&combined).0,
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

#[derive(Default)]
pub(crate) struct TurnVerdicts {
    seen: Vec<(String, bool)>,
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

    pub(crate) fn observe(&mut self, machine: &Machine) {
        if let Some(v) = machine.verdict() {
            self.pending.remove(&v.0);
            self.seen.push(v);
        }
    }

    pub(crate) fn reward(&self) -> Option<f32> {
        if self.pending.is_empty() {
            turn_reward(&self.seen)
        } else {
            None
        }
    }
}

fn turn_reward(seen: &[(String, bool)]) -> Option<f32> {
    if seen.is_empty() {
        return None;
    }
    let left_red = seen
        .iter()
        .any(|(key, _)| matches!(seen.iter().rev().find(|(k, _)| k == key), Some((_, false))));
    if left_red {
        return Some(0.0);
    }
    let passes = seen.iter().filter(|(_, ok)| *ok).count();
    Some(passes as f32 / seen.len() as f32)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn next_seq() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(1);
    SEQ.fetch_add(1, Ordering::Relaxed)
}

pub(crate) use angel_live::host::sha256_hex;

const CUT_USAGE: &str = "usage: /cut — morning read of the authored-diff manifest \
(machine pass rate, keep/edited/discarded, slices by driver/model/repo/path/hour)";

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
    match report_module(dir, workspace) {
        Ok(report) => report,
        Err(error) => format!("the cut — report unavailable: {error}"),
    }
}

#[cfg(unix)]
fn report_module(dir: &Path, workspace: &Path) -> Result<String, String> {
    report_module_with_timeout(dir, workspace, Duration::from_secs(10))
}

#[cfg(unix)]
fn report_module_with_timeout(
    dir: &Path,
    workspace: &Path,
    timeout: Duration,
) -> Result<String, String> {
    use std::process::Command;

    let node = crate::agent::tools::runtime_missing::resolve("node")
        .map_err(|_| "Node.js unavailable (install Node or set ANGEL_NODE_BIN)".to_string())?;
    let script = crate::platform::runtime_paths::script("runtime/cut-report.mjs");
    let mut command = Command::new(node);
    command
        .arg(script)
        .arg("--dir")
        .arg(dir)
        .arg("--workspace")
        .arg(workspace);
    let bytes = crate::platform::worker::run_cancellable(
        command,
        &[],
        timeout,
        0,
        256 * 1024,
        || false,
    )
    .map_err(|error| format!("report module: {error}"))?;
    String::from_utf8(bytes).map_err(|_| "report output was not UTF-8".to_string())
}

#[cfg(not(unix))]
fn report_module(_dir: &Path, _workspace: &Path) -> Result<String, String> {
    Err("report module is unavailable on this platform".into())
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/cut__tests.rs"]
mod tests;
