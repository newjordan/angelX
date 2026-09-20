//! `pr://` and `issue://` virtual URL resolution via the GitHub CLI (`gh`).
//!
//! oh-my-pi treats PRs/issues as FS-shaped paths so one `read` surface covers
//! local files and GitHub objects. angel0 adapts that: `read_file` resolves
//! these schemes by shelling out to `gh` in the workspace (default repo from
//! git remote). Fail closed when `gh` is missing or the id is invalid.
//!
//! ## Forms
//!
//! | URL | Resolves to |
//! |---|---|
//! | `pr://` / `pr://*` | Open PR list for the current repo |
//! | `pr://42` | PR #42 metadata + body |
//! | `pr://42/diff` | Unified diff |
//! | `pr://42/files` | Changed file paths |
//! | `pr://42/comments` | Review comments |
//! | `pr://owner/repo/42` | Explicit repo |
//! | `issue://` / `issue://*` | Open issue list |
//! | `issue://7` | Issue #7 |
//! | `issue://7/comments` | Issue comments |
//! | `issue://owner/repo/7` | Explicit repo |

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::harness::{output_timed, tool_timeout};

const MAX_GH_OUT: usize = 200_000;

/// Whether `raw` is a pr:// or issue:// URI.
pub(crate) fn is_github_uri(raw: &str) -> bool {
    let t = raw.trim();
    t.starts_with("pr://") || t.starts_with("issue://")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Kind {
    Pr,
    Issue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum View {
    Default,
    Diff,
    Files,
    Comments,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Parsed {
    pub(crate) kind: Kind,
    /// Optional `owner/repo` from the URI.
    pub(crate) repo: Option<String>,
    /// None means "list open items".
    pub(crate) number: Option<u64>,
    pub(crate) view: View,
}

/// Parse `pr://…` / `issue://…`. Returns a human error for malformed ids.
pub(crate) fn parse_github_uri(raw: &str) -> Result<Parsed, String> {
    let t = raw.trim();
    let (kind, rest) = if let Some(r) = t.strip_prefix("pr://") {
        (Kind::Pr, r)
    } else if let Some(r) = t.strip_prefix("issue://") {
        (Kind::Issue, r)
    } else {
        return Err(format!("not a pr:// or issue:// URI: {raw:?}"));
    };
    let rest = rest.trim().trim_start_matches('/');
    if rest.is_empty() || rest == "*" {
        return Ok(Parsed {
            kind,
            repo: None,
            number: None,
            view: View::Default,
        });
    }

    // Split trailing view segment: …/diff|files|comments
    let (main, view) = match rest.rsplit_once('/') {
        Some((head, "diff")) if kind == Kind::Pr => (head, View::Diff),
        Some((head, "files")) if kind == Kind::Pr => (head, View::Files),
        Some((head, "comments")) => (head, View::Comments),
        _ => (rest, View::Default),
    };

    // Forms: `42` | `owner/repo/42` | `owner/repo` (list for that repo)
    let parts: Vec<&str> = main.split('/').filter(|p| !p.is_empty()).collect();
    match parts.as_slice() {
        [] => Ok(Parsed {
            kind,
            repo: None,
            number: None,
            view,
        }),
        [n] if n.parse::<u64>().is_ok() => Ok(Parsed {
            kind,
            repo: None,
            number: Some(n.parse().unwrap()),
            view,
        }),
        [owner, repo] => Ok(Parsed {
            kind,
            repo: Some(format!("{owner}/{repo}")),
            number: None,
            view,
        }),
        [owner, repo, n] => {
            let number: u64 = n.parse().map_err(|_| {
                format!("invalid {kind:?} number {n:?} in {raw:?}; expected a positive integer")
            })?;
            if number == 0 {
                return Err("PR/issue number must be ≥ 1".into());
            }
            Ok(Parsed {
                kind,
                repo: Some(format!("{owner}/{repo}")),
                number: Some(number),
                view,
            })
        }
        _ => Err(format!(
            "invalid GitHub URI {raw:?}; expected pr://N, pr://owner/repo/N[/diff|files|comments], \
             issue://N, or issue://owner/repo/N[/comments]"
        )),
    }
}

/// Resolve a GitHub virtual URL against `workspace` (for default repo context).
pub(crate) fn resolve_github_uri(workspace: &Path, raw: &str) -> Result<String, String> {
    let parsed = parse_github_uri(raw)?;
    if which_gh().is_none() {
        return Err(
            "gh CLI not found on PATH — install GitHub CLI (https://cli.github.com) to use pr:// and issue://"
                .into(),
        );
    }
    match (&parsed.kind, parsed.number, &parsed.view) {
        (Kind::Pr, None, _) => run_gh(
            workspace,
            parsed.repo.as_deref(),
            &["pr", "list", "--limit", "30"],
        ),
        (Kind::Issue, None, _) => run_gh(
            workspace,
            parsed.repo.as_deref(),
            &["issue", "list", "--limit", "30"],
        ),
        (Kind::Pr, Some(n), View::Default) => {
            let num = n.to_string();
            run_gh(
                workspace,
                parsed.repo.as_deref(),
                &[
                    "pr",
                    "view",
                    &num,
                    "--json",
                    "number,title,state,author,baseRefName,headRefName,url,body,additions,deletions,changedFiles,labels,reviewDecision",
                    "--template",
                    "{{printf \"# PR %v · %s\\n\" .number .state}}{{printf \"title: %s\\n\" .title}}{{printf \"author: %s\\n\" .author.login}}{{printf \"base: %s ← %s\\n\" .baseRefName .headRefName}}{{printf \"url: %s\\n\" .url}}{{printf \"+-%v/-%v files:%v review:%s\\n\\n\" .additions .deletions .changedFiles .reviewDecision}}{{.body}}",
                ],
            )
        }
        (Kind::Pr, Some(n), View::Diff) => {
            let num = n.to_string();
            // `gh pr diff` has no --json; raw unified diff.
            run_gh(workspace, parsed.repo.as_deref(), &["pr", "diff", &num])
        }
        (Kind::Pr, Some(n), View::Files) => {
            let num = n.to_string();
            run_gh(
                workspace,
                parsed.repo.as_deref(),
                &[
                    "pr",
                    "view",
                    &num,
                    "--json",
                    "files",
                    "--template",
                    "{{range .files}}{{.path}}{{printf \" (+%v/-%v)\\n\" .additions .deletions}}{{end}}",
                ],
            )
        }
        (Kind::Pr, Some(n), View::Comments) => {
            let num = n.to_string();
            run_gh(
                workspace,
                parsed.repo.as_deref(),
                &["pr", "view", &num, "--comments"],
            )
        }
        (Kind::Issue, Some(n), View::Default) => {
            let num = n.to_string();
            run_gh(
                workspace,
                parsed.repo.as_deref(),
                &[
                    "issue",
                    "view",
                    &num,
                    "--json",
                    "number,title,state,author,url,body,labels,comments",
                    "--template",
                    "{{printf \"# Issue %v · %s\\n\" .number .state}}{{printf \"title: %s\\n\" .title}}{{printf \"author: %s\\n\" .author.login}}{{printf \"url: %s\\n\\n\" .url}}{{.body}}",
                ],
            )
        }
        (Kind::Issue, Some(n), View::Comments) => {
            let num = n.to_string();
            run_gh(
                workspace,
                parsed.repo.as_deref(),
                &["issue", "view", &num, "--comments"],
            )
        }
        (Kind::Issue, Some(_), View::Diff | View::Files) => Err(
            "issue:// does not support /diff or /files; use pr://N/diff for pull requests".into(),
        ),
    }
}

fn which_gh() -> Option<PathBuf> {
    // Cheap existence check without shelling through `which`.
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join("gh");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

use std::path::PathBuf;

fn run_gh(workspace: &Path, repo: Option<&str>, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("gh");
    cmd.current_dir(workspace);
    // Unattended: never prompt for auth mid-tool.
    cmd.env("GH_PROMPT_DISABLED", "1");
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    if let Some(r) = repo {
        cmd.args(["-R", r]);
    }
    cmd.args(args);
    let timeout = tool_timeout().or(Some(Duration::from_secs(45)));
    let (out, timed_out) =
        output_timed(cmd, timeout).map_err(|e| format!("gh unavailable: {e}"))?;
    if timed_out {
        return Err(format!(
            "gh {} timed out after {}s",
            args.join(" "),
            timeout.map(|d| d.as_secs()).unwrap_or(0)
        ));
    }
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.trim();
        return Err(if err.is_empty() {
            format!(
                "gh {} failed (exit {:?})",
                args.join(" "),
                out.status.code()
            )
        } else {
            format!("gh {} failed: {err}", args.join(" "))
        });
    }
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    if text.trim().is_empty() {
        text = String::from_utf8_lossy(&out.stderr).into_owned();
    }
    if text.len() > MAX_GH_OUT {
        text.truncate(MAX_GH_OUT);
        // Snap to char boundary.
        while !text.is_char_boundary(text.len()) {
            text.pop();
        }
        text.push_str("\n…[truncated pr:// / issue:// body]\n");
    }
    if text.is_empty() {
        Ok("(empty gh response)\n".into())
    } else {
        if !text.ends_with('\n') {
            text.push('\n');
        }
        Ok(text)
    }
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/github_url__tests.rs"]
mod tests;
