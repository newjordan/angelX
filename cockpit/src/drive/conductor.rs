//! `/conductor` — the human-facing surface for the Conductor loop (C2 of
//! `docs/WORKERS.md`).
//!
//! The graph remains Node-owned. Rust reads only the small status artifacts the
//! tick writes under `~/.angel0/conductor`: `status.json`, `queue.json`, and
//! `heartbeat.json`. Approval/rejection spool verdicts for the next worker tick.

use crate::agent::harness::run_git;
use std::io::Write;
use std::path::{Path, PathBuf};

pub(crate) const STARTUP_NOTICE_PREFIX: &str = "conductor:";

fn state_dir() -> PathBuf {
    match std::env::var("ANGEL_CONDUCTOR_DIR") {
        Ok(p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => crate::platform::workspace_store::angel_subdir("conductor"),
    }
}

fn read_json(path: &Path) -> Option<serde_json::Value> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
}

#[derive(Debug, Clone)]
struct AgendaItem {
    rung: String,
    goal: String,
    priority: Option<f64>,
}

#[derive(Debug, Clone)]
struct QueueItem {
    id: String,
    branch: Option<String>,
    goal: Option<String>,
    agenda_node: Option<String>,
    worktree: Option<PathBuf>,
    worktree_top: Option<PathBuf>,
}

fn array_from<'a>(v: &'a serde_json::Value, keys: &[&str]) -> Option<&'a Vec<serde_json::Value>> {
    if let Some(a) = v.as_array() {
        return Some(a);
    }
    for key in keys {
        if let Some(a) = v.get(*key).and_then(|x| x.as_array()) {
            return Some(a);
        }
    }
    None
}

fn agenda_items(v: Option<&serde_json::Value>) -> Vec<AgendaItem> {
    let Some(items) = v.and_then(|v| array_from(v, &["agenda", "ranking", "top"])) else {
        return Vec::new();
    };
    items
        .iter()
        .take(5)
        .map(|it| AgendaItem {
            rung: it
                .get("rung")
                .and_then(|v| v.as_str())
                .unwrap_or("?")
                .to_string(),
            goal: it
                .get("goal")
                .or_else(|| it.get("label"))
                .and_then(|v| v.as_str())
                .unwrap_or("(no goal)")
                .to_string(),
            priority: it.get("priority").and_then(|v| v.as_f64()),
        })
        .collect()
}

fn queue_items(v: Option<&serde_json::Value>) -> Vec<QueueItem> {
    let Some(items) = v.and_then(|v| array_from(v, &["items", "queue"])) else {
        return Vec::new();
    };
    items
        .iter()
        .map(|it| QueueItem {
            id: it
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("?")
                .to_string(),
            branch: it
                .get("branch")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            goal: it.get("goal").and_then(|v| v.as_str()).map(str::to_string),
            agenda_node: it
                .get("agendaNode")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            worktree: it
                .get("worktree")
                .and_then(|v| v.as_str())
                .map(PathBuf::from),
            worktree_top: it
                .get("worktreeTop")
                .and_then(|v| v.as_str())
                .map(PathBuf::from),
        })
        .collect()
}

fn heartbeat_line(v: Option<&serde_json::Value>) -> String {
    let Some(v) = v else {
        return "heartbeat: none".to_string();
    };
    let action = v.get("action").and_then(|v| v.as_str()).unwrap_or("?");
    let ts = v.get("ts").and_then(|v| v.as_str()).unwrap_or("?");
    let reason = v.get("reason").and_then(|v| v.as_str()).unwrap_or("");
    if reason.is_empty() {
        format!("heartbeat: {action} at {ts}")
    } else {
        format!("heartbeat: {action} at {ts} - {reason}")
    }
}

/// Tri-state, mirroring `conductorMode` in `scripts/runtime/conductor-tick.mjs`: a typo
/// falls through to the state that cannot act. Measure runs the whole night path
/// (lock, gates, folds, briefing) and withholds only the act — see
/// `docs/WORKERS.md`.
fn armed_line(conductor: Option<&str>, driver: Option<&str>) -> String {
    match conductor
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("1") => match driver.map(str::trim).filter(|s| !s.is_empty()) {
            Some(d) => format!("state: armed (code driver: {d})"),
            None => "state: armed; code rung waits for ANGEL_CONDUCTOR_DRIVER".to_string(),
        },
        Some("measure") => "state: measure (folds evidence, dispatches nothing)".to_string(),
        _ => "state: disarmed (ANGEL_CONDUCTOR is not 1 or measure)".to_string(),
    }
}

pub(crate) fn status_text() -> String {
    let conductor = std::env::var("ANGEL_CONDUCTOR").ok();
    let driver = std::env::var("ANGEL_CONDUCTOR_DRIVER").ok();
    status_text_in(&state_dir(), conductor.as_deref(), driver.as_deref())
}

pub(crate) fn status_text_in(
    state: &Path,
    conductor: Option<&str>,
    driver: Option<&str>,
) -> String {
    let status_path = state.join("status.json");
    let queue_path = state.join("queue.json");
    let heartbeat_path = state.join("heartbeat.json");

    let status = read_json(&status_path);
    let queue = read_json(&queue_path);
    let heartbeat = read_json(&heartbeat_path);
    let agenda = agenda_items(status.as_ref());
    let queued = queue_items(queue.as_ref());

    let mut out = String::from("conductor status\n");
    out.push_str(&format!("{}\n", armed_line(conductor, driver)));
    if agenda.is_empty() {
        out.push_str(&format!(
            "agenda: no status export yet ({})\n",
            status_path.display()
        ));
    } else {
        out.push_str("agenda:\n");
        for (i, item) in agenda.iter().enumerate() {
            let priority = item
                .priority
                .map(|p| format!(" · priority {p:.3}"))
                .unwrap_or_default();
            out.push_str(&format!(
                "  {}. [{}] {}{}\n",
                i + 1,
                item.rung,
                item.goal,
                priority
            ));
        }
    }

    if queued.is_empty() {
        out.push_str("queue: empty\n");
    } else {
        out.push_str(&format!(
            "queue: {} gated branch(es) pending\n",
            queued.len()
        ));
        for item in queued.iter().take(5) {
            let branch = item.branch.as_deref().unwrap_or("(no branch)");
            let goal = item.goal.as_deref().unwrap_or("(no goal)");
            out.push_str(&format!("  {} · {} · {}\n", item.id, branch, goal));
        }
    }
    out.push_str(&heartbeat_line(heartbeat.as_ref()));
    out
}

pub(crate) fn brief_text() -> String {
    brief_text_in(&state_dir())
}

pub(crate) fn brief_text_in(state: &Path) -> String {
    let briefing_path = state.join("briefing.md");
    let queue_path = state.join("queue.json");
    let mut out = std::fs::read_to_string(&briefing_path)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("no conductor briefing yet ({})\n", briefing_path.display()));
    let queued = queue_items(read_json(&queue_path).as_ref());
    if !queued.is_empty() {
        out.push_str("\nparked branches awaiting review:\n");
        let values = queue_values(&queue_path);
        for item in queued {
            let diffstat = values
                .iter()
                .find(|v| v.get("id").and_then(|v| v.as_str()) == Some(item.id.as_str()))
                .and_then(|v| v.get("diffstat"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            out.push_str(&format!(
                "- {} · {} · {}\n",
                item.id,
                item.branch.as_deref().unwrap_or("(no branch)"),
                item.goal.as_deref().unwrap_or("(no goal)")
            ));
            if !diffstat.is_empty() {
                for line in diffstat.lines() {
                    out.push_str(&format!("  {line}\n"));
                }
            }
        }
    }
    out.trim_end().to_string()
}

pub(crate) fn startup_notice_text(n: usize) -> String {
    format!("{STARTUP_NOTICE_PREFIX} {n} gated branches await /conductor review")
}

pub(crate) fn startup_notice(workspace: &Path) -> Option<String> {
    if !is_live_project(workspace) {
        return None;
    }
    startup_notice_in(
        &state_dir().join("queue.json"),
        &state_dir().join("last-launch"),
    )
}

pub(crate) fn startup_notice_in(queue_path: &Path, stamp: &Path) -> Option<String> {
    let queued = queue_items(read_json(queue_path).as_ref()).len();
    let prev: usize = std::fs::read_to_string(stamp)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    if let Some(dir) = stamp.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(stamp, queued.to_string());
    if queued > prev {
        Some(startup_notice_text(queued))
    } else {
        None
    }
}

fn live_root() -> Result<PathBuf, String> {
    crate::agent::tools::self_model::source_root()
        .ok_or_else(|| "cannot locate live source root (set ANGEL_SELF_SRC)".to_string())
}

fn is_live_project(workspace: &Path) -> bool {
    let Ok(root) = live_root() else {
        return false;
    };
    let identity = crate::platform::workspace_store::repo_identity(&root);
    crate::platform::workspace_store::matches_project(workspace, &identity.root, &identity.key)
}

fn queue_values(queue_path: &Path) -> Vec<serde_json::Value> {
    let v = read_json(queue_path).unwrap_or_else(|| serde_json::Value::Array(Vec::new()));
    if let Some(a) = v.as_array() {
        return a.clone();
    }
    v.get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

fn write_queue_values(queue_path: &Path, values: &[serde_json::Value]) -> std::io::Result<()> {
    if let Some(dir) = queue_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(
        queue_path,
        crate::platform::secrets::to_redacted_vec_pretty(values).map_err(std::io::Error::other)?,
    )
}

fn item_from_value(v: &serde_json::Value) -> QueueItem {
    queue_items(Some(&serde_json::Value::Array(vec![v.clone()])))
        .into_iter()
        .next()
        .unwrap_or(QueueItem {
            id: "?".to_string(),
            branch: None,
            goal: None,
            agenda_node: None,
            worktree: None,
            worktree_top: None,
        })
}

fn find_queue_item(queue_path: &Path, id: &str) -> Option<(QueueItem, Vec<serde_json::Value>)> {
    let values = queue_values(queue_path);
    let item = values
        .iter()
        .find(|v| v.get("id").and_then(|v| v.as_str()) == Some(id))
        .map(item_from_value)?;
    Some((item, values))
}

fn remove_queue_item(queue_path: &Path, values: Vec<serde_json::Value>, id: &str) -> String {
    let kept: Vec<serde_json::Value> = values
        .into_iter()
        .filter(|v| v.get("id").and_then(|v| v.as_str()) != Some(id))
        .collect();
    match write_queue_values(queue_path, &kept) {
        Ok(_) => String::new(),
        Err(e) => format!(" · WARNING: queue update failed: {e}"),
    }
}

fn spool_verdict(state: &Path, action: &str, item: &QueueItem) -> std::io::Result<()> {
    std::fs::create_dir_all(state)?;
    let line = serde_json::json!({
        "ts": now_secs(),
        "action": action,
        "name": item.id,
        "fact": item.agenda_node,
    });
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(state.join("verdicts.jsonl"))?;
    writeln!(
        f,
        "{}",
        crate::platform::secrets::redact_error(&line.to_string())
    )
}

fn worktree_top(item: &QueueItem) -> Option<PathBuf> {
    item.worktree_top.clone().or_else(|| item.worktree.clone())
}

fn git_root(live_root: &Path) -> Result<PathBuf, String> {
    run_git(live_root, &["rev-parse", "--show-toplevel"]).map(|s| PathBuf::from(s.trim()))
}

fn dirty_tree(git_root: &Path) -> Result<bool, String> {
    run_git(git_root, &["status", "--porcelain"]).map(|s| !s.trim().is_empty())
}

pub(crate) fn approve(id: &str) -> String {
    match live_root() {
        Ok(root) => approve_in(&state_dir(), &root, id),
        Err(e) => format!("/conductor approve: {e}"),
    }
}

pub(crate) fn approve_in(state: &Path, live_root: &Path, id: &str) -> String {
    let queue_path = state.join("queue.json");
    let Some((item, values)) = find_queue_item(&queue_path, id) else {
        return unknown(&queue_path, id);
    };
    let Some(branch) = item.branch.as_deref() else {
        return format!("/conductor approve: queue item '{id}' has no branch");
    };
    let root = match git_root(live_root) {
        Ok(r) => r,
        Err(e) => return format!("/conductor approve: live tree not a git repo: {e}"),
    };
    match dirty_tree(&root) {
        Ok(true) => {
            return "/conductor approve: live tree is dirty; commit/stash your work first"
                .to_string();
        }
        Ok(false) => {}
        Err(e) => return format!("/conductor approve: cannot inspect live tree: {e}"),
    }
    let merge_msg = format!("conductor approve {id}");
    if let Err(e) = run_git(&root, &["merge", "--no-ff", "-m", &merge_msg, branch]) {
        let conflicts =
            run_git(&root, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
        let _ = run_git(&root, &["merge", "--abort"]);
        return format!(
            "/conductor approve: merge failed and was aborted; conflicts: {}; {e}",
            conflicts.trim()
        );
    }
    let mut note = String::new();
    if let Some(wt) = worktree_top(&item)
        && let Err(e) = run_git(
            &root,
            &["worktree", "remove", "--force", &wt.to_string_lossy()],
        )
    {
        note.push_str(&format!(" · worktree remove failed: {e}"));
    }
    let spooled = spool_verdict(state, "approve", &item).is_ok();
    note.push_str(&remove_queue_item(&queue_path, values, id));
    format!(
        "conductor approved '{id}' — merged {branch} into the live tree{}{}",
        if spooled {
            " · verdict spooled (SUPPORTS 0.9)"
        } else {
            " · WARNING: verdict spool write failed"
        },
        note
    )
}

pub(crate) fn reject(id: &str) -> String {
    match live_root() {
        Ok(root) => reject_in(&state_dir(), &root, id),
        Err(e) => format!("/conductor reject: {e}"),
    }
}

pub(crate) fn reject_in(state: &Path, live_root: &Path, id: &str) -> String {
    let queue_path = state.join("queue.json");
    let Some((item, values)) = find_queue_item(&queue_path, id) else {
        return unknown(&queue_path, id);
    };
    let root = match git_root(live_root) {
        Ok(r) => r,
        Err(e) => return format!("/conductor reject: live tree not a git repo: {e}"),
    };
    let mut note = String::new();
    if let Some(wt) = worktree_top(&item)
        && let Err(e) = run_git(
            &root,
            &["worktree", "remove", "--force", &wt.to_string_lossy()],
        )
    {
        note.push_str(&format!(" · worktree remove failed: {e}"));
    }
    if let Some(branch) = item.branch.as_deref()
        && let Err(e) = run_git(&root, &["branch", "-D", branch])
    {
        note.push_str(&format!(" · branch delete failed: {e}"));
    }
    let spooled = spool_verdict(state, "reject", &item).is_ok();
    note.push_str(&remove_queue_item(&queue_path, values, id));
    format!(
        "conductor rejected '{id}' — parked branch discarded{}{}",
        if spooled {
            " · verdict spooled (CONTRADICTS 0.9)"
        } else {
            " · WARNING: verdict spool write failed"
        },
        note
    )
}

fn unknown(queue_path: &Path, id: &str) -> String {
    let names: Vec<String> = queue_items(read_json(queue_path).as_ref())
        .into_iter()
        .map(|i| i.id)
        .collect();
    format!(
        "no conductor queue item named '{id}'. Available: {}",
        if names.is_empty() {
            "(none)".to_string()
        } else {
            names.join(", ")
        }
    )
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `/conductor [status | approve <id> | reject <id>]` dispatch.
pub(crate) fn run(arg: Option<&str>, workspace: &Path) -> String {
    if !is_live_project(workspace) {
        return "/conductor is bound to the angel0 source project; /cd to that repository before reviewing or mutating its queue".to_string();
    }
    let arg = arg.unwrap_or("").trim();
    if arg.is_empty() || arg == "status" {
        return status_text();
    }
    if arg == "brief" {
        return brief_text();
    }
    if let Some(id) = arg
        .strip_prefix("approve ")
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return approve(id);
    }
    if let Some(id) = arg
        .strip_prefix("reject ")
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return reject(id);
    }
    format!("usage: /conductor [status|brief|approve <id>|reject <id>] (got '{arg}')")
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/conductor__tests.rs"]
mod tests;
