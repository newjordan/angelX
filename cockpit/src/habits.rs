//! `/habits` — Habitsmith proposal surfacing + the approval gate (H4 of
//! `docs/plans/habitsmith.md`).
//!
//! The Habitsmith compiler (`scripts/habitsmith.mjs --propose`) renders
//! confident observed workflows into draft skill folders under
//! `~/.angel0/skills-proposed/<name>/SKILL.md`. This module is the human side
//! of that Tier B loop: list the drafts with their evidence, and turn one
//! keypress into the ONLY path a draft can take into the live skills dir
//! (`/habits approve <name>` moves the folder; `/habits reject <name>`
//! deletes it). Nothing here ever writes a skill the user didn't approve.
//!
//! Verdicts don't touch the causal graph directly — the graph is Node-owned
//! and unlocked, and racing the idle ticks from Rust would corrupt it.
//! Instead approve/reject append one JSONL line to
//! `~/.angel0/habitsmith/verdicts.jsonl`; the habitsmith tick (H5) folds the
//! spool into SUPPORTS/CONTRADICTS edges (conf 0.9) via updateBeliefs on its
//! next run. A rejected workflow's belief sinks under the propose gate, so it
//! never re-proposes.

use std::path::{Path, PathBuf};

/// Draft dir: `ANGEL_HABIT_PROPOSED_DIR`, else `~/.angel0/skills-proposed`.
pub(crate) fn proposed_dir() -> PathBuf {
    match std::env::var("ANGEL_HABIT_PROPOSED_DIR") {
        Ok(p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => crate::workspace_store::angel_subdir("skills-proposed"),
    }
}

/// Live skills dir the loader reads: `ANGEL_SKILLS_DIR`, else `~/.angel0/skills`.
fn live_dir() -> PathBuf {
    match std::env::var("ANGEL_SKILLS_DIR") {
        Ok(p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => crate::workspace_store::angel_subdir("skills"),
    }
}

fn state_dir() -> PathBuf {
    std::env::var_os("ANGEL_HABITS_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::workspace_store::angel_subdir("habitsmith"))
}

/// One parsed draft: harness frontmatter (name/description) plus the
/// habitsmith-specific keys the harness parser ignores (`fact:`, `risky:`)
/// and the evidence footer line.
pub(crate) struct Proposal {
    pub name: String,
    pub description: String,
    pub fact: Option<String>,
    pub risky: bool,
    pub evidence: Option<String>,
    pub project_key: Option<String>,
    pub project_root: Option<PathBuf>,
    pub dir: PathBuf,
}

impl Proposal {
    /// Habits are learned from one repository's command evidence. Missing or
    /// mismatched bindings are therefore inert; legacy drafts do not become
    /// global merely because they predate the binding fields.
    fn applies_to(&self, workspace: &Path) -> bool {
        let identity = crate::workspace_store::repo_identity(workspace);
        self.project_key.as_deref() == Some(identity.key.as_str())
            && self.project_root.as_deref() == Some(identity.root.as_path())
    }
}

fn front_key(text: &str, key: &str) -> Option<String> {
    let rest = text.strip_prefix("---")?;
    let end = rest.find("\n---")?;
    rest[..end]
        .lines()
        .find_map(|l| l.strip_prefix(key))
        .map(|v| v.trim().trim_matches('"').trim().to_string())
        .filter(|v| !v.is_empty())
}

fn parse_proposal(dir: &Path, text: &str, fallback: &str) -> Proposal {
    let sk = crate::harness::parse_skill(text, fallback);
    Proposal {
        name: sk.name,
        description: sk.description,
        fact: front_key(text, "fact:"),
        risky: front_key(text, "risky:").is_some_and(|v| v == "true"),
        evidence: sk
            .body
            .lines()
            .find(|l| l.starts_with("Evidence:"))
            .map(|l| l.to_string()),
        project_key: front_key(text, "repo_key:"),
        project_root: front_key(text, "repo_root:").map(PathBuf::from),
        dir: dir.to_path_buf(),
    }
}

fn list_proposals_for_in(dir: &Path, workspace: &Path) -> Vec<Proposal> {
    list_proposals_in(dir)
        .into_iter()
        .filter(|proposal| proposal.applies_to(workspace))
        .collect()
}

/// All drafts under `dir`, sorted by name. Best-effort: unreadable entries skip.
pub(crate) fn list_proposals_in(dir: &Path) -> Vec<Proposal> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let d = e.path();
        if !d.is_dir() {
            continue;
        }
        let fallback = e.file_name().to_string_lossy().to_string();
        if let Ok(text) = std::fs::read_to_string(d.join("SKILL.md")) {
            out.push(parse_proposal(&d, &text, &fallback));
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Append one verdict line to the spool the habitsmith tick folds into the
/// graph. Append-only JSONL, the ledger pattern; failure is reported, not fatal.
fn spool_verdict(state: &Path, action: &str, p: &Proposal) -> std::io::Result<()> {
    use std::io::Write;
    std::fs::create_dir_all(state)?;
    let line = serde_json::json!({
        "ts": now_secs(),
        "action": action,
        "name": p.name,
        "fact": p.fact,
        "repoKey": p.project_key,
        "repoRoot": p.project_root,
    });
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(state.join("verdicts.jsonl"))?;
    writeln!(f, "{}", crate::secrets::redact_error(&line.to_string()))
}

/// The `/habits` list body: pending drafts, then the installed-skill health
/// section from the tick's status artifact (H5 — drift flags live there).
pub(crate) fn status_text(workspace: &Path) -> String {
    status_text_in(&proposed_dir(), &state_dir().join("status.json"), workspace)
}

pub(crate) fn status_text_in(dir: &Path, status_path: &Path, workspace: &Path) -> String {
    let proposals = list_proposals_for_in(dir, workspace);
    let worker = crate::runtime_paths::script("habitsmith-tick.mjs");
    let quoted_worker = worker.to_string_lossy().replace('\'', "'\"'\"'");
    let mut out = if proposals.is_empty() {
        format!(
            "no habitsmith proposals ({}).\n\
             Drafts appear once a command sequence recurs across enough sessions:\n\
             node '{}' --force\n",
            dir.display(),
            quoted_worker
        )
    } else {
        let mut s = format!("habitsmith proposals — {} draft(s)\n", proposals.len());
        for p in &proposals {
            s.push_str(&format!(
                "  {}{}\n    {}\n",
                p.name,
                if p.risky {
                    "  ⚠ risky — read before approving"
                } else {
                    ""
                },
                p.description
            ));
            if let Some(ev) = &p.evidence {
                s.push_str(&format!("    {ev}\n"));
            }
        }
        s.push_str(
            "/habits approve <name> installs into the live skills dir · /habits reject <name> deletes the draft\n",
        );
        s
    };
    if let Some(section) = std::fs::read_to_string(status_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|v| installed_section(&v, workspace))
    {
        out.push_str(&section);
    }
    out.trim_end().to_string()
}

/// The installed-skill health lines from the tick's status.json: every fact
/// with recorded skill usage, drift first. `None` when nothing is installed.
fn installed_section(status: &serde_json::Value, workspace: &Path) -> Option<String> {
    let identity = crate::workspace_store::repo_identity(workspace);
    let facts = status["facts"].as_array()?;
    let mut lines = Vec::new();
    for f in facts {
        if f["repoKey"].as_str() != Some(identity.key.as_str())
            || f["repoRoot"].as_str() != Some(identity.root.to_string_lossy().as_ref())
        {
            continue;
        }
        let Some(skill) = f["skill"].as_object() else {
            continue;
        };
        let name = f["name"].as_str().unwrap_or("?");
        let belief = f["belief"].as_f64().unwrap_or(0.0);
        let uses = skill.get("uses").and_then(|v| v.as_u64()).unwrap_or(0);
        if f["drifting"].as_bool() == Some(true) {
            lines.push(format!(
                "  {name} [belief {belief:.2}] ⚠ drifting — repo may have changed; steps may need re-verification"
            ));
        } else {
            lines.push(format!("  {name} [belief {belief:.2}] · {uses} use(s)"));
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!("installed habit skills:\n{}\n", lines.join("\n")))
}

/// Approve: move the draft folder into the live skills dir and spool the
/// SUPPORTS verdict. The move is the install — the skills loader picks the
/// folder up on its next catalog scan, no restart needed.
pub(crate) fn approve(name: &str, workspace: &Path) -> String {
    approve_in(&proposed_dir(), &live_dir(), &state_dir(), name, workspace)
}

pub(crate) fn approve_in(
    proposed: &Path,
    live: &Path,
    state: &Path,
    name: &str,
    workspace: &Path,
) -> String {
    let Some(p) = list_proposals_for_in(proposed, workspace)
        .into_iter()
        .find(|p| p.name == name)
    else {
        return unknown(proposed, name, workspace);
    };
    let dest = live.join(p.dir.file_name().unwrap_or_default());
    if dest.exists() {
        return format!(
            "/habits approve: '{}' already exists in {}",
            name,
            live.display()
        );
    }
    if let Err(e) = std::fs::create_dir_all(live) {
        return format!("/habits approve: cannot create {}: {e}", live.display());
    }
    if let Err(e) = std::fs::rename(&p.dir, &dest) {
        return format!("/habits approve: move failed: {e}");
    }
    let spooled = spool_verdict(state, "approve", &p).is_ok();
    format!(
        "approved '{}' → {}\n  live in the skill catalog on the next scan{}",
        name,
        dest.display(),
        if spooled {
            " · verdict spooled for the next habitsmith tick (SUPPORTS 0.9)"
        } else {
            " · WARNING: verdict spool write failed — belief won't update"
        }
    )
}

/// Reject: delete the draft and spool the CONTRADICTS verdict so the same
/// workflow never re-proposes (its belief sinks under the propose gate).
pub(crate) fn reject(name: &str, workspace: &Path) -> String {
    reject_in(&proposed_dir(), &state_dir(), name, workspace)
}

pub(crate) fn reject_in(proposed: &Path, state: &Path, name: &str, workspace: &Path) -> String {
    let Some(p) = list_proposals_for_in(proposed, workspace)
        .into_iter()
        .find(|p| p.name == name)
    else {
        return unknown(proposed, name, workspace);
    };
    if let Err(e) = std::fs::remove_dir_all(&p.dir) {
        return format!("/habits reject: delete failed: {e}");
    }
    let spooled = spool_verdict(state, "reject", &p).is_ok();
    format!(
        "rejected '{}' — draft deleted{}",
        name,
        if spooled {
            " · verdict spooled (CONTRADICTS 0.9): it will not re-propose"
        } else {
            " · WARNING: verdict spool write failed — it may re-propose"
        }
    )
}

fn unknown(proposed: &Path, name: &str, workspace: &Path) -> String {
    let names: Vec<String> = list_proposals_for_in(proposed, workspace)
        .into_iter()
        .map(|p| p.name)
        .collect();
    format!(
        "no proposal named '{name}'. Available: {}",
        if names.is_empty() {
            "(none)".to_string()
        } else {
            names.join(", ")
        }
    )
}

/// `/habits [approve <name> | reject <name>]` dispatch.
pub(crate) fn run(arg: Option<&str>, workspace: &Path) -> String {
    let arg = arg.unwrap_or("").trim();
    match arg.split_once(char::is_whitespace) {
        Some(("approve", name)) => approve(name.trim(), workspace),
        Some(("reject", name)) => reject(name.trim(), workspace),
        _ if arg.is_empty() => status_text(workspace),
        _ => {
            format!("usage: /habits · /habits approve <name> · /habits reject <name> (got '{arg}')")
        }
    }
}

/// The one startup Notice line (the deferred Reflex M6 pattern): shown only
/// when the proposed dir has drafts newer than the previous launch. The stamp
/// file advances every call, so the line appears once per new batch.
pub(crate) fn startup_notice(workspace: &Path) -> Option<String> {
    let identity = crate::workspace_store::repo_identity(workspace);
    startup_notice_in(
        &proposed_dir(),
        &state_dir()
            .join("launch-notices")
            .join(format!("{}.stamp", identity.key)),
        now_secs(),
        workspace,
    )
}

pub(crate) fn startup_notice_in(
    proposed: &Path,
    stamp: &Path,
    now: u64,
    workspace: &Path,
) -> Option<String> {
    let prev: u64 = std::fs::read_to_string(stamp)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    if let Some(dir) = stamp.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(stamp, now.to_string());

    let proposals = list_proposals_for_in(proposed, workspace);
    if proposals.is_empty() {
        return None;
    }
    let newest = proposals
        .iter()
        .filter_map(|p| std::fs::metadata(p.dir.join("SKILL.md")).ok())
        .filter_map(|m| m.modified().ok())
        .filter_map(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .max()?;
    if newest <= prev {
        return None;
    }
    Some(format!(
        "habitsmith drafted {} skill proposal(s) from your observed workflows — /habits to review",
        proposals.len()
    ))
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/habits__tests.rs"]
mod tests;
