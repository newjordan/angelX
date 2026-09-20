//! Read-only git tools: `git_diff` (uncommitted or verified-base changes),
//! `git_status` (branch + staged/modified/untracked), and `git_log` (recent
//! commits, path-scopable).
//! They shell out via the shared `run_git` helper and confine any path argument
//! to the workspace with `safe_path`.

use crate::club::ToolDef;
use crate::harness::{Tool, WorkspaceBoundary, cap_lines, run_git};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn confined_git_path(workspace: &Path, supplied: &str) -> Result<String, String> {
    WorkspaceBoundary::cached(workspace).confined_git_path(supplied)
}

pub(crate) fn validate_git_revision(revision: &str) -> Result<&str, String> {
    if revision.is_empty() {
        return Err("git_diff.base must not be empty".to_string());
    }
    if revision.len() > 128 {
        return Err("git_diff.base exceeds 128 bytes".to_string());
    }
    if revision.starts_with('-') {
        return Err("git_diff.base must be a revision, not an option".to_string());
    }
    if revision.contains("..") {
        return Err("git_diff.base accepts one revision, not a revision range".to_string());
    }
    if !revision.chars().all(|ch| {
        ch.is_ascii_alphanumeric()
            || matches!(ch, '-' | '_' | '.' | '/' | '~' | '^' | '@' | '{' | '}')
    }) {
        return Err(
            "git_diff.base contains unsupported characters; use one branch, tag, or commit"
                .to_string(),
        );
    }
    Ok(revision)
}

pub(crate) fn git_diff_argv(
    staged: bool,
    base: Option<&str>,
    stat: bool,
    path: Option<&str>,
) -> Result<Vec<String>, String> {
    if staged && base.is_some() {
        return Err("git_diff: staged=true cannot be combined with base".to_string());
    }
    let mut argv = vec![
        "diff".to_string(),
        "--no-ext-diff".to_string(),
        "--no-textconv".to_string(),
    ];
    if stat {
        argv.push("--stat".to_string());
    }
    if staged {
        argv.push("--cached".to_string());
    }
    if let Some(base) = base {
        argv.push(validate_git_revision(base)?.to_string());
    }
    if let Some(path) = path.filter(|path| !path.is_empty()) {
        argv.push("--".to_string());
        argv.push(path.to_string());
    }
    Ok(argv)
}

pub(crate) struct GitDiffTool {
    pub(crate) workspace: PathBuf,
}
impl Tool for GitDiffTool {
    fn name(&self) -> &str {
        "git_diff"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "git_diff".to_string(),
            description: "Show workspace changes (read-only). By default this is the unstaged \
                          worktree; staged=true shows the index. base compares the full current \
                          worktree against one verified branch, tag, or commit (and cannot combine \
                          with staged). stat=true returns a cheaper summary first. An optional path \
                          confines the comparison. Review your own edits before committing."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "staged": { "type": "boolean", "description": "diff the index (--cached) instead of the worktree" },
                    "base": { "type": "string", "description": "compare the current worktree against one verified branch, tag, or commit" },
                    "stat": { "type": "boolean", "description": "show a compact file/change summary instead of the full patch" },
                    "path": { "type": "string", "description": "limit the diff to this in-workspace path (relative or absolute)" },
                },
                "required": [],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let staged = args["staged"].as_bool().unwrap_or(false);
        let stat = args["stat"].as_bool().unwrap_or(false);
        let base = args["base"].as_str().filter(|base| !base.is_empty());
        let path = args["path"]
            .as_str()
            .filter(|path| !path.is_empty())
            .map(|path| confined_git_path(&self.workspace, path))
            .transpose()?;
        let argv = git_diff_argv(staged, base, stat, path.as_deref())?;
        if let Some(base) = base {
            let commit = format!("{base}^{{commit}}");
            run_git(
                &self.workspace,
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    "--end-of-options",
                    &commit,
                ],
            )
            .map_err(|_| format!("git_diff.base did not resolve to one commit: {base}"))?;
        }
        let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
        let out = run_git(&self.workspace, &argv_ref)?;
        if out.trim().is_empty() {
            Ok(if let Some(base) = base {
                format!("(no changes against {base})")
            } else if staged {
                "(no staged changes)".to_string()
            } else {
                "(no uncommitted changes)".to_string()
            })
        } else {
            Ok(cap_lines(&out, 600))
        }
    }
}

/// Summarize `git status --porcelain=v1 --branch` into a compact, readable
/// orientation: branch + ahead/behind, then staged / modified / untracked files.
/// Pure → testable against canned porcelain.
pub(crate) fn summarize_git_status(porcelain: &str) -> String {
    let mut branch = String::new();
    let (mut staged, mut modified, mut untracked) = (Vec::new(), Vec::new(), Vec::new());
    for line in porcelain.lines() {
        if let Some(b) = line.strip_prefix("## ") {
            branch = b.to_string();
            continue;
        }
        if line.len() < 3 || !line.is_char_boundary(2) || !line.is_char_boundary(3) {
            continue;
        }
        let (code, path) = (&line[..2], line[3..].trim());
        if code == "??" {
            untracked.push(path);
            continue;
        }
        let bytes = code.as_bytes();
        if bytes[0] != b' ' {
            staged.push(path); // index has a change for this path
        }
        if bytes[1] != b' ' {
            modified.push(path); // worktree change not yet staged
        }
    }
    let section = |title: &str, items: &[&str]| -> String {
        if items.is_empty() {
            return String::new();
        }
        let shown: Vec<&str> = items.iter().take(50).copied().collect();
        let more = items.len().saturating_sub(shown.len());
        let mut s = format!("{title} ({}):\n", items.len());
        for p in shown {
            s.push_str(&format!("  {p}\n"));
        }
        if more > 0 {
            s.push_str(&format!("  …(+{more} more)\n"));
        }
        s
    };
    let mut out = String::new();
    if !branch.is_empty() {
        out.push_str(&format!("branch: {branch}\n"));
    }
    if staged.is_empty() && modified.is_empty() && untracked.is_empty() {
        out.push_str("working tree clean");
        return out.trim_end().to_string();
    }
    out.push_str(&section("staged", &staged));
    out.push_str(&section("modified", &modified));
    out.push_str(&section("untracked", &untracked));
    out.trim_end().to_string()
}

pub(crate) struct GitStatusTool {
    pub(crate) workspace: PathBuf,
}
impl Tool for GitStatusTool {
    fn name(&self) -> &str {
        "git_status"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "git_status".to_string(),
            description: "Show the workspace's git status (read-only): current branch + \
                          ahead/behind, and the staged / modified / untracked files. A compact \
                          orientation before editing or committing; complements git_diff (which \
                          shows the actual changes)."
                .to_string(),
            params: serde_json::json!({ "type": "object", "properties": {} }),
        }
    }
    fn call(&self, _args: &Value) -> Result<String, String> {
        let out = run_git(&self.workspace, &["status", "--porcelain=v1", "--branch"])?;
        Ok(summarize_git_status(&out))
    }
}

/// Build the `git log` argv (clamped count, optional path scope). Pure → tested.
pub(crate) fn git_log_argv(count: usize, path: Option<&str>) -> Vec<String> {
    let n = count.clamp(1, 100);
    let mut argv = vec![
        "log".to_string(),
        "--oneline".to_string(),
        "--no-color".to_string(),
        format!("-n{n}"),
    ];
    if let Some(p) = path.filter(|p| !p.is_empty()) {
        argv.push("--".to_string());
        argv.push(p.to_string());
    }
    argv
}

pub(crate) struct GitLogTool {
    pub(crate) workspace: PathBuf,
}
impl Tool for GitLogTool {
    fn name(&self) -> &str {
        "git_log"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "git_log".to_string(),
            description: "Show recent commits (read-only): one line each (short hash + subject). \
                          count defaults to 15 (max 100); an optional path scopes history to that \
                          file/dir. Orient on what changed recently and why."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "count": { "type": "integer", "description": "how many commits (default 15, max 100)" },
                    "path": { "type": "string", "description": "limit history to this in-workspace path (relative or absolute)" },
                },
                "required": [],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let count = args["count"].as_u64().unwrap_or(15) as usize;
        let path = args["path"]
            .as_str()
            .filter(|path| !path.is_empty())
            .map(|path| confined_git_path(&self.workspace, path))
            .transpose()?;
        let argv = git_log_argv(count, path.as_deref());
        let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
        let out = run_git(&self.workspace, &argv_ref)?;
        if out.trim().is_empty() {
            Ok("(no commits)".to_string())
        } else {
            Ok(cap_lines(&out, 200))
        }
    }
}

/// Plan (default) or execute atomic git commits over the dirty worktree.
///
/// Ported in spirit from oh-my-pi's `omp commit` split: group dirty paths by
/// file class (source → test → docs → config → other → lock) and emit one
/// commit per group, in that order. `execute=false` (default) only prints the
/// plan; set `execute=true` to stage each unit and `git commit`.
pub(crate) struct GitCommitTool {
    pub(crate) workspace: PathBuf,
}

impl Tool for GitCommitTool {
    fn name(&self) -> &str {
        "git_commit"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "git_commit".to_string(),
            description: "Plan or create atomic git commits from the dirty worktree. \
                          Default is plan-only (execute=false): classifies dirty paths \
                          into source → test → docs → config → other → lock units and \
                          suggests subjects. Set execute=true to stage each unit and \
                          commit in that order. split=true (default) makes one commit \
                          per class; split=false packs everything into one commit. \
                          Optional message sets the subject (validated: ≤72 chars, \
                          blank line before body). Never force-pushes; never amends."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "execute": {
                        "type": "boolean",
                        "description": "false (default) = print plan only; true = stage+commit each unit"
                    },
                    "split": {
                        "type": "boolean",
                        "description": "true (default) = one commit per file class; false = single commit"
                    },
                    "message": {
                        "type": "string",
                        "description": "commit subject (and optional blank-line + body). Used for the first/only unit; further split units get class-scoped subjects"
                    },
                    "include_untracked": {
                        "type": "boolean",
                        "description": "include untracked files in the plan (default true)"
                    },
                },
                "required": [],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        use crate::git_commit_split::{
            format_plan, parse_porcelain_files, plan_atomic_commits, validate_commit_message,
        };
        use crate::harness::run_git;

        let execute = args["execute"].as_bool().unwrap_or(false);
        if execute
            && std::env::var("ANGEL_TASK_SHELL_PROTECT_GIT")
                .map(|value| {
                    let value = value.trim();
                    value == "1"
                        || value.eq_ignore_ascii_case("true")
                        || value.eq_ignore_ascii_case("on")
                })
                .unwrap_or(false)
        {
            return Err(
                "git_commit execute rejected: this sealed task protects repository control state"
                    .to_string(),
            );
        }
        let split = args["split"].as_bool().unwrap_or(true);
        let include_untracked = args["include_untracked"].as_bool().unwrap_or(true);
        let message = args["message"].as_str().filter(|m| !m.is_empty());

        // Ensure we're inside a git repo with an identity-friendly environment
        // (run_git already sets user.name/email for harness commits).
        run_git(&self.workspace, &["rev-parse", "--is-inside-work-tree"])
            .map_err(|e| format!("git_commit: workspace is not a git worktree: {e}"))?;

        let porcelain = run_git(
            &self.workspace,
            &["status", "--porcelain=v1", "--untracked-files=all"],
        )?;
        let mut files = parse_porcelain_files(&porcelain);
        if !include_untracked {
            files.retain(|f| !f.is_untracked());
        }
        if files.is_empty() {
            return Ok("working tree clean — nothing to commit".into());
        }

        let plan = plan_atomic_commits(&files, split, message)?;
        if !execute {
            return Ok(format_plan(&plan));
        }

        // Execute: stage each unit's paths and commit. Stop on first failure
        // without rewriting earlier commits (partial progress is reported).
        let mut receipts = Vec::new();
        for (i, unit) in plan.units.iter().enumerate() {
            validate_commit_message(&unit.subject)?;
            // Stage only this unit's paths (never `git add -A` mid-split).
            let mut add_args: Vec<&str> = vec!["add", "--"];
            let path_refs: Vec<&str> = unit.paths.iter().map(String::as_str).collect();
            add_args.extend(path_refs.iter().copied());
            run_git(&self.workspace, &add_args).map_err(|e| {
                format!(
                    "git_commit: failed staging unit {} ({}): {e}; earlier commits (if any) kept: {}",
                    i + 1,
                    unit.class,
                    receipts.join("; ")
                )
            })?;
            // Skip empty index (e.g. path vanished between plan and execute).
            let cached = run_git(&self.workspace, &["diff", "--cached", "--name-only"])?;
            if cached.trim().is_empty() {
                receipts.push(format!(
                    "[{}] class={} skipped (nothing staged)",
                    i + 1,
                    unit.class
                ));
                continue;
            }
            run_git(
                &self.workspace,
                &["commit", "-q", "-m", &unit.subject],
            )
            .map_err(|e| {
                format!(
                    "git_commit: failed committing unit {} ({}): {e}; earlier commits (if any) kept: {}",
                    i + 1,
                    unit.class,
                    receipts.join("; ")
                )
            })?;
            let head = run_git(&self.workspace, &["rev-parse", "--short", "HEAD"])
                .unwrap_or_else(|_| "?".into());
            receipts.push(format!(
                "[{}] {}  {}  {} path(s)  {:?}",
                i + 1,
                head.trim(),
                unit.class,
                unit.paths.len(),
                unit.subject
            ));
        }
        Ok(format!(
            "atomic commit execute: {} unit(s)\n{}",
            receipts.len(),
            receipts.join("\n")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_task_rejects_typed_commit_execution_before_touching_git() {
        let _lock = crate::tests::env_lock();
        let _sealed = crate::tests::TestEnvGuard::set("ANGEL_TASK_SHELL_PROTECT_GIT", "1");
        let tool = GitCommitTool {
            workspace: PathBuf::from("/definitely/not/a/repository"),
        };

        let error = tool
            .call(&serde_json::json!({ "execute": true }))
            .unwrap_err();
        assert!(
            error.contains("protects repository control state"),
            "{error}"
        );
    }
}
