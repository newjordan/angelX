//! Project docs, workspace resolution, and trusted git operations.

use super::*;

pub(crate) const PROJECT_DOC_FILENAMES: &[&str] = &["AGENTS.md", "AGENTS.override.md"];
pub(crate) const DEFAULT_PROJECT_DOC_MAX_BYTES: usize = 32 * 1024;
const PROJECT_DOC_TRUNCATION_MARK: &str = "\n…⠸⠉⠁…\n";
pub(crate) const PROJECT_DOC_BYTES_MARKER: &str = "⠸⠉⠃";

pub(crate) fn is_git_root(dir: &Path) -> bool {
    let g = dir.join(".git");
    g.is_file() || g.join("HEAD").exists()
}

pub(crate) fn discover_project_docs(start: &Path, max_bytes: usize) -> String {
    if max_bytes == 0 {
        return String::new();
    }
    let start = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());

    let mut git_root: Option<PathBuf> = None;
    let mut probe: Option<&Path> = Some(start.as_path());
    while let Some(dir) = probe {
        if is_git_root(dir) {
            git_root = Some(dir.to_path_buf());
            break;
        }
        probe = dir.parent();
    }

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
        "\n\n{PROJECT_DOC_BYTES_MARKER} {}\n{}\n\n{docs}\n",
        docs.len(),
        super::book::n_environment::PROJECT.cells()
    )
}

pub fn default_workspace() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".angelX/workspace")
}

pub fn current_dir_workspace() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn resolve_workspace(explicit: Option<PathBuf>, fallback: fn() -> PathBuf) -> PathBuf {
    explicit
        .or_else(|| std::env::var_os("ANGEL_WORKSPACE").map(PathBuf::from))
        .unwrap_or_else(fallback)
}

pub(crate) fn git_timeout() -> Option<Duration> {
    match std::env::var("ANGEL_GIT_TIMEOUT")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
    {
        Some(0) | None => None,
        Some(secs) => Some(Duration::from_secs(secs)),
    }
}

pub(crate) const GIT_NO_WORKSPACE_EXEC: [&str; 4] = [
    "-c",
    "core.fsmonitor=false",
    "-c",
    "core.hooksPath=/dev/null",
];

pub(crate) fn run_git(workspace: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.args(GIT_NO_WORKSPACE_EXEC)
        .arg("-c")
        .arg("user.name=angel")
        .arg("-c")
        .arg("user.email=angel@local")
        .arg("-C")
        .arg(workspace)
        .args(args)
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
                    "git repository {} has no commits and contains uncommitted files\n{}",
                    root.display(),
                    crate::agent::harness::book::d56_replies::INITIAL_COMMIT.cells()
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
    crate::platform::workspace_store::write_atomic(&marker, b"angel shared workspace\n")
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
