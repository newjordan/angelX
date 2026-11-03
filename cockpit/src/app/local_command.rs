use crate::agent::club::{ChatMsg, ChatRole};
use crate::knowledge::session::{self, SessionInfo};
use crate::ui::media::{Media, open_target};
use crate::ui::viewer::Viewer;

pub enum ResumeResult {
    Loaded {
        id: String,
        history: Vec<ChatMsg>,
        turns: usize,
    },
    NoSession,
    Failed(String),
}

pub fn open_media(media: &[Media], viewer: &mut Viewer, n: usize) -> String {
    match media.get(n.wrapping_sub(1)) {
        Some(m) => {
            if m.is_image() {
                match m.local_path() {
                    Some(path) => viewer
                        .show(&path)
                        .unwrap_or_else(|e| format!("preview #{n} failed: {e}")),
                    None => format!("preview #{n} failed: image target is not a local file"),
                }
            } else {
                let target = m.target();
                match open_target(&target) {
                    Ok(()) => format!("opening #{n} [{}] {} → {}", m.sigil(), m.label(), target),
                    Err(e) => format!("open #{n} failed: {e}"),
                }
            }
        }
        None => format!("no card #{n} — {} on the carousel (/open <n>)", media.len()),
    }
}

pub fn open_target_command(target: &str) -> String {
    match open_target(target) {
        Ok(()) => format!("opening {target}"),
        Err(e) => format!("open failed for {target}: {e}"),
    }
}

pub fn page_media(scroll: &mut usize, total: usize) {
    if total > 0 {
        *scroll = (*scroll + 2) % total;
    }
}

pub fn sessions_text(infos: &[SessionInfo]) -> String {
    if infos.is_empty() {
        "no saved sessions yet".to_string()
    } else {
        let visible = infos.len().min(15);
        let mut out = String::with_capacity(64 + visible * 84);
        out.push_str("saved sessions (newest first):\n");
        for (index, session) in infos.iter().take(visible).enumerate() {
            if index > 0 {
                out.push('\n');
            }
            out.push_str(&session.id);
            out.push_str("  (");
            out.push_str(&session.turns.to_string());
            out.push_str(" turns)  ");
            out.push_str(&session_preview(&session.preview));
        }
        out.push_str("\n— /resume [id] to return (no id = latest)");
        out
    }
}

fn session_preview(preview: &str) -> String {
    const LIMIT: usize = 48;
    let first = preview.lines().next().unwrap_or("");
    crate::ui::toolstrip::ellipsize(first, LIMIT)
}

#[path = "slash_help.rs"]
mod slash_help;

/// The slash-command help list (Codex commands ported into the cockpit).
pub fn help_text(topic: Option<&str>) -> String {
    match slash_help::load() {
        Ok(help) => {
            let first_screen = matches!(
                topic.map(str::trim).filter(|topic| !topic.is_empty()),
                None | Some("1") | Some("index")
            );
            help.render(
                topic,
                crate::ui::glyphs::chrome_bullet(),
                !first_screen && crate::knowledge::atlas::atlas_env_disabled(),
            )
        }
        Err(error) => format!("commands · help catalog error: {error}"),
    }
}

/// Run a Git command through the shared bounded, timeout-aware process capture;
/// stdout on success, the trimmed stderr on failure.
fn git(args: &[&str]) -> Result<String, String> {
    let mut command = std::process::Command::new("git");
    command
        .args(crate::agent::harness::GIT_NO_WORKSPACE_EXEC)
        .arg("--no-pager")
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0");
    let timeout = crate::agent::harness::git_timeout();
    let (out, timed_out) = crate::agent::harness::output_timed(command, timeout)
        .map_err(|e| format!("git not available: {e}"))?;
    if timed_out {
        return Err(format!(
            "git {} timed out after {}s",
            args.join(" "),
            timeout.map(|duration| duration.as_secs()).unwrap_or(0)
        ));
    }
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn git_with_prefix<'a>(prefix: &[&'a str], tail: &[&'a str]) -> Result<String, String> {
    let mut args = prefix.to_vec();
    args.extend_from_slice(tail);
    git(&args)
}

fn append_git_section(out: &mut String, label: &str, content: &str) {
    let content = content.trim_end();
    if content.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(label);
    out.push('\n');
    out.push_str(content);
}

fn git_worktree_snapshot(workspace: &std::path::Path) -> Result<String, String> {
    let workspace = workspace.to_string_lossy();
    let prefix = ["-C", workspace.as_ref()];
    let mut out = String::new();
    let status = git_with_prefix(
        &prefix,
        &["status", "--short", "--untracked-files=all", "--no-renames"],
    )?;
    let staged = git_with_prefix(
        &prefix,
        &[
            "diff",
            "--cached",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
        ],
    )?;
    let unstaged = git_with_prefix(
        &prefix,
        &["diff", "--no-ext-diff", "--no-textconv", "--no-renames"],
    )?;
    append_git_section(&mut out, "worktree status:", &status);
    append_git_section(&mut out, "staged changes:", &staged);
    append_git_section(&mut out, "unstaged changes:", &unstaged);
    Ok(out)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiffScope {
    All,
    Staged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DiffSpec {
    scope: DiffScope,
    stat: bool,
}

fn parse_diff_spec(arg: Option<&str>) -> Result<DiffSpec, &'static str> {
    let mut spec = DiffSpec {
        scope: DiffScope::All,
        stat: false,
    };
    let Some(arg) = arg.map(str::trim).filter(|arg| !arg.is_empty()) else {
        return Ok(spec);
    };
    let mut scope_seen = false;
    for token in arg.split_whitespace() {
        match token {
            "all" if !scope_seen => {
                scope_seen = true;
            }
            "staged" if !scope_seen => {
                spec.scope = DiffScope::Staged;
                scope_seen = true;
            }
            "stat" | "--stat" if !spec.stat => spec.stat = true,
            _ => return Err("usage: /diff [all|staged] [--stat]"),
        }
    }
    Ok(spec)
}

fn git_diff_snapshot(workspace: &std::path::Path, spec: DiffSpec) -> Result<String, String> {
    if spec
        == (DiffSpec {
            scope: DiffScope::All,
            stat: false,
        })
    {
        return git_worktree_snapshot(workspace);
    }

    let workspace = workspace.to_string_lossy();
    let prefix = ["-C", workspace.as_ref()];
    let mut out = String::new();
    let stat = ["--stat", "--no-ext-diff", "--no-textconv", "--no-renames"];
    match (spec.scope, spec.stat) {
        (DiffScope::All, true) => {
            let status = git_with_prefix(
                &prefix,
                &["status", "--short", "--untracked-files=all", "--no-renames"],
            )?;
            let mut staged_args = vec!["diff", "--cached"];
            staged_args.extend_from_slice(&stat);
            let staged = git_with_prefix(&prefix, &staged_args)?;
            let mut unstaged_args = vec!["diff"];
            unstaged_args.extend_from_slice(&stat);
            let unstaged = git_with_prefix(&prefix, &unstaged_args)?;
            append_git_section(&mut out, "worktree status:", &status);
            append_git_section(&mut out, "staged summary:", &staged);
            append_git_section(&mut out, "unstaged summary:", &unstaged);
        }
        (DiffScope::Staged, stat_only) => {
            let mut args = vec!["diff", "--cached"];
            if stat_only {
                args.extend_from_slice(&stat);
            } else {
                args.extend_from_slice(&["--no-ext-diff", "--no-textconv", "--no-renames"]);
            }
            let staged = git_with_prefix(&prefix, &args)?;
            append_git_section(
                &mut out,
                if stat_only {
                    "staged summary:"
                } else {
                    "staged changes:"
                },
                &staged,
            );
        }
        (DiffScope::All, false) => unreachable!("default all-diff returned above"),
    }
    Ok(out)
}

/// Bounded staged, unstaged, and untracked worktree snapshot for `/review`.
pub fn git_diff_raw(workspace: &std::path::Path) -> Result<String, String> {
    git_worktree_snapshot(workspace)
}

/// Worktree snapshot for `/diff`, capped so a huge diff cannot flood the
/// transcript. Untracked files are listed; the agent can inspect their contents
/// during `/review`.
pub fn git_diff_text(workspace: &std::path::Path, arg: Option<&str>) -> String {
    let spec = match parse_diff_spec(arg) {
        Ok(spec) => spec,
        Err(usage) => return usage.to_string(),
    };
    let body = match git_diff_snapshot(workspace, spec) {
        Ok(body) if body.trim().is_empty() => {
            return match spec.scope {
                DiffScope::All => "working tree clean — no changes".to_string(),
                DiffScope::Staged => "no staged changes".to_string(),
            };
        }
        Ok(body) => body,
        Err(error) => return format!("/diff: {error}"),
    };
    const CAP: usize = 4000;
    if body.len() > CAP {
        let end = (0..=CAP)
            .rev()
            .find(|&i| body.is_char_boundary(i))
            .unwrap_or(0);
        let mut out = body[..end].to_string();
        out.push_str(&format!(
            "\n… (+{} bytes — /review to send a larger diff slice to the agent)",
            body.len() - end
        ));
        out
    } else {
        body.trim_end().to_string()
    }
}

/// The `/review` task the harness writes in the operator's place: `⠬⠚⠁`.
const REVIEW_TASK: &str = "⠬⠚⠁";

pub struct EvidenceTurn {
    task: String,
    evidence: String,
}

impl EvidenceTurn {
    pub fn new(task: impl Into<String>, evidence: impl Into<String>) -> Self {
        Self {
            task: task.into(),
            evidence: evidence.into(),
        }
    }

    pub fn into_messages(self) -> (ChatMsg, ChatMsg) {
        (ChatMsg::user(self.task), ChatMsg::harness(self.evidence))
    }
}

fn format_review_evidence(mut snapshot: String) -> String {
    const SNAPSHOT_CAP: usize = 40_000;
    let original_bytes = snapshot.len();
    crate::agent::harness::truncate_to_char_boundary(&mut snapshot, SNAPSHOT_CAP);
    let shown_bytes = snapshot.len();
    // `⠬⠙` frames the snapshot; the framing and the truncation guidance are
    // its ledger pages, the byte count is the data.
    let truncation = if shown_bytes < original_bytes {
        format!(
            "\nsnapshot_truncated_bytes={}",
            original_bytes - shown_bytes
        )
    } else {
        String::new()
    };
    format!(
        "{}\n\n<worktree_snapshot>\n{snapshot}\n</worktree_snapshot>{truncation}",
        crate::agent::harness::book::ing_drivers::REVIEW.cells()
    )
}

/// Build the bounded evidence packet for `/review`; the caller keeps its
/// operator task and this repository snapshot in distinct protocol roles.
pub fn git_review_request(workspace: &std::path::Path) -> Result<Option<EvidenceTurn>, String> {
    let snapshot = git_diff_raw(workspace)?;
    if snapshot.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(EvidenceTurn::new(
            REVIEW_TASK,
            format_review_evidence(snapshot),
        )))
    }
}

/// Create a starter `AGENTS.md` in the active workspace without replacing an
/// existing file or following a pre-existing symlink.
pub fn init_agents_md(workspace: &std::path::Path) -> String {
    use std::io::Write;

    let path = workspace.join("AGENTS.md");
    let template = "# AGENTS.md\n\n\
        Instructions for agents working in this repository.\n\n\
        ## Build & test\n- build: \n- test: \n- lint: \n\n\
        ## Conventions\n- \n";
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut file) => match file.write_all(template.as_bytes()) {
            Ok(()) => "created AGENTS.md — fill in build/test/convention notes".to_string(),
            Err(error) => format!("/init: could not write AGENTS.md: {error}"),
        },
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            "AGENTS.md already exists — leaving it untouched".to_string()
        }
        Err(error) => format!("/init: could not write AGENTS.md: {error}"),
    }
}

pub fn resume(id: Option<String>, workspace: &std::path::Path) -> ResumeResult {
    let target = id.or_else(|| {
        session::list_for(workspace)
            .into_iter()
            .next()
            .map(|s| s.id)
    });
    let Some(id) = target else {
        return ResumeResult::NoSession;
    };
    match session::load_for(&id, workspace) {
        Ok(history) => {
            let turns = history.iter().filter(|m| m.role == ChatRole::User).count();
            ResumeResult::Loaded { id, history, turns }
        }
        Err(e) => ResumeResult::Failed(e),
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/local_command__tests.rs"]
mod tests;
