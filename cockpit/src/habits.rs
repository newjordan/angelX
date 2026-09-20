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
    crate::workspace_store::angel_subdir("habitsmith")
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
    let mut out = if proposals.is_empty() {
        format!(
            "no habitsmith proposals ({}).\n\
             Drafts appear once a command sequence recurs across enough sessions:\n\
             node scripts/habitsmith.mjs --mine && node scripts/habitsmith.mjs --propose\n",
            dir.display()
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
mod tests {
    use super::*;

    fn draft(workspace: &Path) -> String {
        let identity = crate::workspace_store::repo_identity(workspace);
        format!(
            "---\nname: proj-build-test\ndescription: Run the observed build → test workflow.\nfact: hyp_habit_k_build-test\nrisky: true\nscope: project\nrepo_key: \"{}\"\nrepo_root: \"{}\"\n---\n\n# proj-build-test\n\nSteps:\n1. `cargo build` (build, passes 100%)\n\n---\nEvidence: 6 run(s) across 6 session(s), 6 clean end-to-end · belief 0.78\n",
            identity.key,
            identity.root.display()
        )
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("angel-habits-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write_draft(base: &Path, folder: &str, text: &str) {
        let dir = base.join(folder);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), text).unwrap();
    }

    #[test]
    fn lists_proposals_with_frontmatter_evidence_and_risk() {
        let base = scratch("list");
        write_draft(&base, "proj-build-test", &draft(&base));
        let ps = list_proposals_in(&base);
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0].name, "proj-build-test");
        assert_eq!(ps[0].fact.as_deref(), Some("hyp_habit_k_build-test"));
        assert!(ps[0].risky);
        assert!(ps[0].evidence.as_deref().unwrap().contains("6 run(s)"));

        let text = status_text_in(&base, &base.join("no-status.json"), &base);
        assert!(text.contains("proj-build-test"));
        assert!(text.contains("⚠ risky"));
        assert!(text.contains("approve <name>"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn empty_dir_says_how_drafts_appear() {
        let base = scratch("empty");
        let text = status_text_in(&base, &base.join("no-status.json"), &base);
        assert!(text.contains("no habitsmith proposals"));
        assert!(text.contains("--propose"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn installed_section_renders_usage_and_drift_from_the_tick_artifact() {
        let base = scratch("drift");
        let identity = crate::workspace_store::repo_identity(&base);
        let status = serde_json::json!({
            "v": 1,
            "facts": [
                { "name": "proj-build-test-run", "repoKey": identity.key, "repoRoot": identity.root, "belief": 0.42, "drifting": true,
                  "skill": { "uses": 7, "ok": 5, "lastUsed": 1_783_300_000u64 } },
                { "name": "proj-lint-test", "repoKey": identity.key, "repoRoot": identity.root, "belief": 0.81, "drifting": false,
                  "skill": { "uses": 3, "ok": 3, "lastUsed": 1_783_300_000u64 } },
                { "name": "never-installed", "repoKey": identity.key, "repoRoot": identity.root, "belief": 0.77, "drifting": false, "skill": null },
            ],
        });
        std::fs::write(
            base.join("status.json"),
            serde_json::to_string(&status).unwrap(),
        )
        .unwrap();

        let text = status_text_in(
            &base.join("nothing-proposed"),
            &base.join("status.json"),
            &base,
        );
        assert!(
            text.contains("proj-build-test-run [belief 0.42] ⚠ drifting"),
            "{text}"
        );
        assert!(text.contains("proj-lint-test [belief 0.81] · 3 use(s)"));
        assert!(!text.contains("never-installed"), "no usage → not listed");
        // Garbage status.json never breaks the command.
        std::fs::write(base.join("status.json"), "{nope").unwrap();
        let ok = status_text_in(
            &base.join("nothing-proposed"),
            &base.join("status.json"),
            &base,
        );
        assert!(ok.contains("no habitsmith proposals"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn approve_moves_the_folder_and_spools_supports() {
        let base = scratch("approve");
        let (proposed, live, state) = (base.join("p"), base.join("l"), base.join("s"));
        write_draft(&proposed, "proj-build-test", &draft(&base));

        let msg = approve_in(&proposed, &live, &state, "proj-build-test", &base);
        assert!(msg.contains("approved"), "{msg}");
        assert!(
            live.join("proj-build-test/SKILL.md").exists(),
            "installed into live dir"
        );
        assert!(
            !proposed.join("proj-build-test").exists(),
            "draft gone from proposed"
        );
        let spool = std::fs::read_to_string(state.join("verdicts.jsonl")).unwrap();
        assert!(spool.contains("\"action\":\"approve\""));
        assert!(spool.contains("hyp_habit_k_build-test"));

        // Approving again: the draft no longer exists.
        let again = approve_in(&proposed, &live, &state, "proj-build-test", &base);
        assert!(again.contains("no proposal named"), "{again}");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn approve_refuses_to_clobber_an_existing_live_skill() {
        let base = scratch("clobber");
        let (proposed, live, state) = (base.join("p"), base.join("l"), base.join("s"));
        write_draft(&proposed, "proj-build-test", &draft(&base));
        write_draft(&live, "proj-build-test", "already here");

        let msg = approve_in(&proposed, &live, &state, "proj-build-test", &base);
        assert!(msg.contains("already exists"), "{msg}");
        assert!(proposed.join("proj-build-test").exists(), "draft untouched");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn reject_deletes_and_spools_contradicts() {
        let base = scratch("reject");
        let (proposed, state) = (base.join("p"), base.join("s"));
        write_draft(&proposed, "proj-build-test", &draft(&base));

        let msg = reject_in(&proposed, &state, "proj-build-test", &base);
        assert!(msg.contains("rejected"), "{msg}");
        assert!(!proposed.join("proj-build-test").exists());
        let spool = std::fs::read_to_string(state.join("verdicts.jsonl")).unwrap();
        assert!(spool.contains("\"action\":\"reject\""));

        let unknown = reject_in(&proposed, &state, "nope", &base);
        assert!(unknown.contains("no proposal named 'nope'"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn startup_notice_fires_once_per_new_batch() {
        let base = scratch("notice");
        let (proposed, stamp) = (base.join("p"), base.join("s/last-launch"));
        write_draft(&proposed, "proj-build-test", &draft(&base));

        // First launch after the draft appeared: notice (stamp was 0/missing).
        let first = startup_notice_in(&proposed, &stamp, now_secs(), &base);
        assert!(first.is_some_and(|n| n.contains("1 skill proposal(s)")));
        // Next launch, nothing new: silent (stamp advanced past the mtime).
        let second = startup_notice_in(&proposed, &stamp, now_secs() + 10, &base);
        assert!(second.is_none());
        // Empty dir: silent.
        let none = startup_notice_in(&base.join("nowhere"), &stamp, now_secs() + 20, &base);
        assert!(none.is_none());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn proposal_lifecycle_is_project_bound_and_legacy_drafts_are_inert() {
        let base = scratch("project-bound");
        let alpha = base.join("alpha");
        let beta = base.join("beta");
        let proposed = base.join("proposed");
        let live = base.join("live");
        let state = base.join("state");
        std::fs::create_dir_all(&alpha).unwrap();
        std::fs::create_dir_all(&beta).unwrap();
        write_draft(&proposed, "alpha-draft", &draft(&alpha));
        write_draft(&proposed, "beta-draft", &draft(&beta));
        write_draft(
            &proposed,
            "legacy-draft",
            "---\nname: legacy-habit\nfact: hyp_habit_legacy\n---\nlegacy",
        );

        assert_eq!(list_proposals_for_in(&proposed, &alpha).len(), 1);
        assert_eq!(list_proposals_for_in(&proposed, &beta).len(), 1);
        assert!(
            approve_in(&proposed, &live, &state, "legacy-habit", &alpha)
                .contains("no proposal named")
        );

        let alpha_msg = approve_in(&proposed, &live, &state, "proj-build-test", &alpha);
        assert!(alpha_msg.contains("approved"), "{alpha_msg}");
        assert!(proposed.join("beta-draft").exists());
        assert!(!proposed.join("alpha-draft").exists());

        let beta_msg = approve_in(&proposed, &live, &state, "proj-build-test", &beta);
        assert!(beta_msg.contains("approved"), "{beta_msg}");
        assert!(live.join("alpha-draft/SKILL.md").exists());
        assert!(live.join("beta-draft/SKILL.md").exists());
        let _ = std::fs::remove_dir_all(&base);
    }
}
