//! Project docs, workspace resolution, and trusted git operations.

use super::*;

// ---------------------------------------------------------------------------
// Project context — discover AGENTS.md conventions for a separate repository
// guidance message; filesystem text never receives System authority. Walk from the
// workspace dir up to the git root, collecting AGENTS.md (+ AGENTS.override.md) at
// each level, concatenated git-root→workspace (broadest context first). Capped;
// `ANGEL_PROJECT_DOC=0` disables, `ANGEL_PROJECT_DOC_MAX_BYTES` resizes.
// ---------------------------------------------------------------------------

/// Filenames scanned for project docs, in per-directory precedence order.
pub(crate) const PROJECT_DOC_FILENAMES: &[&str] = &["AGENTS.md", "AGENTS.override.md"];
/// Default ceiling on the concatenated project-doc block (matches Codex's 32 KiB).
pub(crate) const DEFAULT_PROJECT_DOC_MAX_BYTES: usize = 32 * 1024;
const PROJECT_DOC_TRUNCATION_MARK: &str = "\n…[scoped project doc truncated: middle elided]…\n";
pub(crate) const PROJECT_DOC_BYTES_MARKER: &str = "<!-- angel-project-doc-bytes:";

/// Is `dir` a git root? Requires a *real* marker — a `.git` directory containing
/// `HEAD` (a repo) or a `.git` file (a worktree) — not merely a `.git` entry, so
/// a stray empty `.git` (e.g. left in `/tmp`) doesn't masquerade as a root.
pub(crate) fn is_git_root(dir: &Path) -> bool {
    let g = dir.join(".git");
    g.is_file() || g.join("HEAD").exists()
}

/// Concatenate the AGENTS.md docs found from the git root down to `start`
/// (inclusive), in that order. If no git root ancestor exists, only `start`
/// itself is considered (never walk the whole filesystem). Pure over the fs;
/// "" if none.
pub(crate) fn discover_project_docs(start: &Path, max_bytes: usize) -> String {
    if max_bytes == 0 {
        return String::new();
    }
    let start = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());

    // Find the git root by walking up from `start`.
    let mut git_root: Option<PathBuf> = None;
    let mut probe: Option<&Path> = Some(start.as_path());
    while let Some(dir) = probe {
        if is_git_root(dir) {
            git_root = Some(dir.to_path_buf());
            break;
        }
        probe = dir.parent();
    }

    // Directory chain to scan, ordered git-root → start (broadest first). With no
    // git root, just the start dir (Codex: "only the cwd is considered").
    let chain: Vec<PathBuf> = match &git_root {
        Some(root) => {
            let mut v = Vec::new();
            let mut cur = start.as_path();
            loop {
                v.push(cur.to_path_buf());
                if cur == root {
                    break;
                }
                match cur.parent() {
                    Some(p) => cur = p,
                    None => break,
                }
            }
            v.reverse();
            v
        }
        None => vec![start.clone()],
    };

    let label_root = git_root.as_deref().unwrap_or(start.as_path());
    let mut docs = Vec::new();
    for dir in chain {
        for fname in PROJECT_DOC_FILENAMES {
            let path = dir.join(fname);
            if let Ok(body) = std::fs::read_to_string(&path) {
                let body = body.trim();
                if !body.is_empty() {
                    let label = path.strip_prefix(label_root).unwrap_or(&path).display();
                    docs.push(format!("## {label}\n{body}"));
                }
            }
        }
    }
    fit_project_docs(&docs, max_bytes)
}

/// Fit root→workspace instruction sections into one exact byte budget. Every
/// scope first receives a fair share; unused share from short files flows from
/// deepest scope upward. Oversized sections retain both their provenance/head
/// and their tail, where repositories commonly place exceptions and commands.
fn fit_project_docs(docs: &[String], max_bytes: usize) -> String {
    if docs.is_empty() || max_bytes == 0 {
        return String::new();
    }
    let separators = docs.len().saturating_sub(1).saturating_mul(2);
    if separators >= max_bytes {
        return truncate_project_doc(docs.last().expect("non-empty"), max_bytes);
    }
    let budget = max_bytes - separators;
    let base = budget / docs.len();
    let remainder = budget % docs.len();
    let mut allocations = docs
        .iter()
        .enumerate()
        .map(|(index, doc)| {
            let deepest_remainder = usize::from(index >= docs.len() - remainder);
            doc.len().min(base + deepest_remainder)
        })
        .collect::<Vec<_>>();
    let mut left = budget.saturating_sub(allocations.iter().sum());
    for (doc, allocation) in docs.iter().zip(&mut allocations).rev() {
        let extra = left.min(doc.len().saturating_sub(*allocation));
        *allocation += extra;
        left -= extra;
        if left == 0 {
            break;
        }
    }
    docs.iter()
        .zip(allocations)
        .filter(|(_, allocation)| *allocation > 0)
        .map(|(doc, allocation)| truncate_project_doc(doc, allocation))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn truncate_project_doc(doc: &str, max_bytes: usize) -> String {
    if doc.len() <= max_bytes {
        return doc.to_string();
    }
    if max_bytes <= PROJECT_DOC_TRUNCATION_MARK.len() + 2 {
        let mut end = max_bytes.min(doc.len());
        while end > 0 && !doc.is_char_boundary(end) {
            end -= 1;
        }
        return doc[..end].to_string();
    }
    let keep = max_bytes - PROJECT_DOC_TRUNCATION_MARK.len();
    let mut head_end = keep * 2 / 5;
    while head_end > 0 && !doc.is_char_boundary(head_end) {
        head_end -= 1;
    }
    let tail_budget = keep - head_end;
    let mut tail_start = doc.len() - tail_budget.min(doc.len());
    while tail_start < doc.len() && !doc.is_char_boundary(tail_start) {
        tail_start += 1;
    }
    format!(
        "{}{}{}",
        &doc[..head_end],
        PROJECT_DOC_TRUNCATION_MARK,
        &doc[tail_start..]
    )
}

/// The scoped guidance block for `workspace`'s project docs (AGENTS.md walked from
/// the workspace up to its git root), or "" when disabled or none found. Injected
/// after the skills catalog. Rooted at the *active* workspace so the project
/// context always matches where the tools actually operate.
pub fn project_context(workspace: &Path) -> String {
    if !env_flag("ANGEL_PROJECT_DOC", true) {
        return String::new();
    }
    let max = env_usize("ANGEL_PROJECT_DOC_MAX_BYTES", DEFAULT_PROJECT_DOC_MAX_BYTES);
    let docs = discover_project_docs(workspace, max);
    if docs.is_empty() {
        return String::new();
    }
    format!(
        "\n\n{PROJECT_DOC_BYTES_MARKER}{} -->\n# Project context (AGENTS.md)\n\
         The project's scoped instructions and conventions — apply them consistently with the \
         operator's request and harness policy. Each section names \
         its source; later, more deeply nested sections take precedence for their directory scope.\n\n{docs}\n",
        docs.len()
    )
}

/// The isolated last-resort workspace (`~/.angel0/workspace`). Still the fallback
/// for headless `--task` (benchmark harnesses rely on it) and the base for
/// delegation worktrees / media resolution — but no longer the TUI default.
pub fn default_workspace() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".angel0/workspace")
}

/// The process's current directory (the TUI's default workspace — "operate on
/// the directory you launched me in", like Claude Code), or `.` if it can't be
/// read. A `fn() -> PathBuf` so it can be passed as a `resolve_workspace` fallback.
pub fn current_dir_workspace() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Resolve the active workspace root once, from a single precedence order shared
/// by the TUI and headless `--task`: an `explicit` path (positional arg /
/// `--workspace`) wins, else `$ANGEL_WORKSPACE`, else the caller's `fallback`
/// (the TUI passes [`current_dir_workspace`]; `--task` passes [`default_workspace`]).
pub fn resolve_workspace(explicit: Option<PathBuf>, fallback: fn() -> PathBuf) -> PathBuf {
    explicit
        .or_else(|| std::env::var_os("ANGEL_WORKSPACE").map(PathBuf::from))
        .unwrap_or_else(fallback)
}

/// Run a (trusted, un-sandboxed) git command in `workspace`, with a fixed
/// identity so commits succeed without global git config.
pub(crate) fn git_timeout() -> Option<Duration> {
    match std::env::var("ANGEL_GIT_TIMEOUT")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
    {
        Some(0) => None,
        Some(secs) => Some(Duration::from_secs(secs)),
        None => Some(Duration::from_secs(120)),
    }
}

pub(crate) fn run_git(workspace: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-c")
        .arg("user.name=angel")
        .arg("-c")
        .arg("user.email=angel@local")
        .arg("-C")
        .arg(workspace)
        .args(args)
        // Harness git operations are unattended. Never let a credential helper
        // or editor turn delegation into an invisible, unbounded prompt.
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_MERGE_AUTOEDIT", "no");
    let timeout = git_timeout();
    let (out, timed_out) = output_timed(cmd, timeout)?;
    if timed_out {
        return Err(format!(
            "git {} timed out after {}s",
            args.join(" "),
            timeout.map(|d| d.as_secs()).unwrap_or(0)
        ));
    }
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Ensure `workspace` belongs to a git repo with at least one commit (so
/// `worktree add … HEAD` works), returning that repository's top-level path.
/// A workspace may be a subdirectory of an existing repository; it must never
/// be silently converted into a nested repository.
pub(crate) fn ensure_git_workspace(workspace: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(workspace).map_err(|e| format!("mkdir workspace: {e}"))?;
    let canonical_workspace = workspace
        .canonicalize()
        .map_err(|e| format!("resolve workspace {}: {e}", workspace.display()))?;

    if let Ok(root) = run_git(&canonical_workspace, &["rev-parse", "--show-toplevel"]) {
        let root = PathBuf::from(root.trim());
        if run_git(&root, &["rev-parse", "--verify", "HEAD"]).is_err() {
            let dirty = run_git(&root, &["status", "--porcelain"])?;
            if !dirty.trim().is_empty() {
                return Err(format!(
                    "git repository {} has no commits and contains uncommitted files; create an initial commit before delegating",
                    root.display()
                ));
            }
            run_git(
                &root,
                &[
                    "commit",
                    "-q",
                    "--allow-empty",
                    "-m",
                    "angel workspace init",
                ],
            )?;
        }
        return Ok(root);
    }

    run_git(&canonical_workspace, &["init", "-q"])?;
    let marker = canonical_workspace.join(".angel-workspace");
    std::fs::write(&marker, "angel shared workspace\n")
        .map_err(|e| format!("write marker: {e}"))?;
    run_git(&canonical_workspace, &["add", "-A"])?;
    run_git(
        &canonical_workspace,
        &["commit", "-q", "-m", "angel workspace init"],
    )?;
    Ok(canonical_workspace)
}

pub(crate) fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{t}…")
    }
}
