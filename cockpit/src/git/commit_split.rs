//! Atomic git commit planning — split a dirty worktree into dependency-ordered
//! commits by file class (oh-my-pi `omp commit` spirit, pure Rust, no LLM).
//!
//! Order (source before tests before docs before config; lockfiles last):
//! 1. **source** — implementation code
//! 2. **test** — test files / suites
//! 3. **docs** — markdown / docs trees
//! 4. **config** — project config (toml/yml/jsonc, gitignore, …)
//! 5. **lock** — lockfiles (excluded from "analysis" weight; commit last)
//! 6. **other** — everything else
//!
//! The planner is pure over `git status --porcelain` input. The `git_commit`
//! tool executes plans only when `execute=true`.

use std::fmt;

/// Coarse file class used for split ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum FileClass {
    Source = 0,
    Test = 1,
    Docs = 2,
    Config = 3,
    Other = 4,
    Lock = 5,
}

impl FileClass {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Test => "test",
            Self::Docs => "docs",
            Self::Config => "config",
            Self::Other => "other",
            Self::Lock => "lock",
        }
    }

    pub(crate) fn default_type_prefix(self) -> &'static str {
        match self {
            Self::Source => "feat",
            Self::Test => "test",
            Self::Docs => "docs",
            Self::Config => "chore",
            Self::Other => "chore",
            Self::Lock => "chore",
        }
    }
}

impl fmt::Display for FileClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One dirty path from porcelain status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DirtyFile {
    pub(crate) path: String,
    pub(crate) index_status: char,
    pub(crate) worktree_status: char,
}

impl DirtyFile {
    pub(crate) fn is_untracked(&self) -> bool {
        self.index_status == '?' && self.worktree_status == '?'
    }

    #[cfg(test)]
    pub(crate) fn has_index_change(&self) -> bool {
        self.index_status != ' ' && self.index_status != '?'
    }

    #[cfg(test)]
    pub(crate) fn has_worktree_change(&self) -> bool {
        self.worktree_status != ' ' && !self.is_untracked()
    }
}

/// One planned atomic commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommitUnit {
    pub(crate) class: FileClass,
    pub(crate) paths: Vec<String>,
    pub(crate) subject: String,
}

/// Full plan over the current dirty tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommitPlan {
    pub(crate) units: Vec<CommitUnit>,
    pub(crate) skipped_lock_note: bool,
}

/// Classify a repo-relative path into a file class.
pub(crate) fn classify_path(path: &str) -> FileClass {
    let p = path.replace('\\', "/");
    let lower = p.to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(lower.as_str());

    // Lockfiles first (OMP excludes them from "analysis" weight).
    if is_lockfile(name) {
        return FileClass::Lock;
    }

    // Tests: path or name signals.
    if is_test_path(&lower, name) {
        return FileClass::Test;
    }

    // Docs.
    if is_docs_path(&lower, name) {
        return FileClass::Docs;
    }

    // Config / project metadata.
    if is_config_path(&lower, name) {
        return FileClass::Config;
    }

    // Source-ish extensions → source; unknown → other.
    if is_source_path(name) {
        return FileClass::Source;
    }
    FileClass::Other
}

fn is_lockfile(name: &str) -> bool {
    matches!(
        name,
        "cargo.lock"
            | "package-lock.json"
            | "yarn.lock"
            | "pnpm-lock.yaml"
            | "bun.lock"
            | "bun.lockb"
            | "composer.lock"
            | "gemfile.lock"
            | "poetry.lock"
            | "pipfile.lock"
            | "flake.lock"
            | "go.sum"
    ) || name.ends_with(".lock")
}

fn is_test_path(lower: &str, name: &str) -> bool {
    lower.contains("/tests/")
        || lower.contains("/test/")
        || lower.contains("/__tests__/")
        || lower.contains("/spec/")
        || lower.starts_with("tests/")
        || lower.starts_with("test/")
        || name.starts_with("test_")
        || name.ends_with("_test.rs")
        || name.ends_with("_test.go")
        || name.ends_with("_test.py")
        || name.ends_with(".test.ts")
        || name.ends_with(".test.tsx")
        || name.ends_with(".test.js")
        || name.ends_with(".test.mjs")
        || name.ends_with(".spec.ts")
        || name.ends_with(".spec.tsx")
        || name.ends_with(".spec.js")
        || name.ends_with("_spec.rb")
        || name.contains(".test.")
        || name.contains(".spec.")
}

fn is_docs_path(lower: &str, name: &str) -> bool {
    lower.starts_with("docs/")
        || lower.contains("/docs/")
        || name == "readme"
        || name == "readme.md"
        || name == "changelog.md"
        || name == "contributing.md"
        || name == "license"
        || name == "license.md"
        || name.ends_with(".md")
        || name.ends_with(".mdx")
        || name.ends_with(".rst")
        || name.ends_with(".adoc")
}

fn is_config_path(lower: &str, name: &str) -> bool {
    if matches!(
        name,
        "cargo.toml"
            | "package.json"
            | "pyproject.toml"
            | "go.mod"
            | "makefile"
            | "dockerfile"
            | "docker-compose.yml"
            | "docker-compose.yaml"
            | ".gitignore"
            | ".gitattributes"
            | ".editorconfig"
            | ".npmrc"
            | ".rustfmt.toml"
            | "rust-toolchain.toml"
            | "rust-toolchain"
            | "tsconfig.json"
            | "jsconfig.json"
            | "eslint.config.js"
            | "biome.json"
            | ".eslintrc"
            | ".eslintrc.json"
            | ".prettierrc"
            | "prettier.config.js"
    ) {
        return true;
    }
    if name.starts_with('.')
        && (name.ends_with("rc")
            || name.ends_with(".json")
            || name.ends_with(".yml")
            || name.ends_with(".yaml")
            || name.ends_with(".toml"))
    {
        return true;
    }
    // Top-level CI configs.
    lower.starts_with(".github/")
        || lower.starts_with(".gitlab/")
        || lower.starts_with(".circleci/")
        || name.ends_with(".yml")
        || name.ends_with(".yaml")
}

fn is_source_path(name: &str) -> bool {
    const EXTS: &[&str] = &[
        ".rs", ".go", ".py", ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".c", ".cc", ".cpp",
        ".h", ".hpp", ".java", ".kt", ".swift", ".rb", ".php", ".cs", ".scala", ".zig", ".sh",
        ".bash", ".zsh", ".fish", ".sql", ".html", ".css", ".scss", ".vue", ".svelte", ".wgsl",
        ".glsl",
    ];
    EXTS.iter().any(|e| name.ends_with(e))
}

/// Parse `git status --porcelain=v1` (no branch header required) into dirty files.
/// Handles rename `R  old -> new` by keeping the new path only.
pub(crate) fn parse_porcelain_files(porcelain: &str) -> Vec<DirtyFile> {
    let mut out = Vec::new();
    for line in porcelain.lines() {
        if line.starts_with("## ") || line.len() < 3 {
            continue;
        }
        let index_status = line.as_bytes()[0] as char;
        let worktree_status = line.as_bytes()[1] as char;
        let rest = line[2..].trim_start();
        // Rename / copy: `old -> new`
        let path = if let Some((_, new)) = rest.split_once(" -> ") {
            new.trim()
        } else {
            rest.trim()
        };
        if path.is_empty() {
            continue;
        }
        // Skip quoted paths' outer quotes when present.
        let path = path
            .strip_prefix('"')
            .and_then(|p| p.strip_suffix('"'))
            .unwrap_or(path);
        out.push(DirtyFile {
            path: path.to_string(),
            index_status,
            worktree_status,
        });
    }
    out
}

/// Validate a commit message subject (and optional body after a blank line).
/// Subject must be non-empty, ≤72 chars, no trailing period preferred but
/// allowed; reject empty, control chars, and leading/trailing whitespace-only.
pub(crate) fn validate_commit_message(message: &str) -> Result<(), String> {
    let message = message.replace("\r\n", "\n").replace('\r', "\n");
    if message.trim().is_empty() {
        return Err("commit message must not be empty".into());
    }
    let mut lines = message.lines();
    let subject = lines.next().unwrap_or("").trim_end();
    if subject.is_empty() {
        return Err("commit subject (first line) must not be empty".into());
    }
    if subject.chars().count() > 72 {
        return Err(format!(
            "commit subject is {} chars (max 72): {subject:?}",
            subject.chars().count()
        ));
    }
    if subject.chars().any(|c| c.is_control()) {
        return Err("commit subject must not contain control characters".into());
    }
    // Optional blank line then body — body lines may be longer.
    if let Some(second) = lines.next()
        && !second.is_empty()
    {
        return Err(
            "commit message: subject must be followed by a blank line before the body".into(),
        );
    }
    Ok(())
}

/// Build an atomic commit plan. When `split` is false, a single unit holds every
/// path (using `message` or a generic subject). When `split` is true, paths are
/// grouped by [`FileClass`] in dependency order.
pub(crate) fn plan_atomic_commits(
    files: &[DirtyFile],
    split: bool,
    message: Option<&str>,
) -> Result<CommitPlan, String> {
    if files.is_empty() {
        return Err("nothing to commit — working tree clean".into());
    }
    if let Some(msg) = message {
        validate_commit_message(msg)?;
    }

    if !split {
        let paths: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
        let subject = message
            .map(|m| m.lines().next().unwrap_or(m).trim_end().to_string())
            .unwrap_or_else(|| {
                format!(
                    "chore: update {} path{}",
                    paths.len(),
                    if paths.len() == 1 { "" } else { "s" }
                )
            });
        return Ok(CommitPlan {
            units: vec![CommitUnit {
                class: FileClass::Other,
                paths,
                subject,
            }],
            skipped_lock_note: false,
        });
    }

    let mut buckets: Vec<(FileClass, Vec<String>)> = Vec::new();
    for f in files {
        let class = classify_path(&f.path);
        if let Some((_, paths)) = buckets.iter_mut().find(|(c, _)| *c == class) {
            paths.push(f.path.clone());
        } else {
            buckets.push((class, vec![f.path.clone()]));
        }
    }
    buckets.sort_by_key(|(c, _)| *c);

    let mut units = Vec::new();
    let mut skipped_lock_note = false;
    for (class, paths) in buckets {
        if class == FileClass::Lock {
            skipped_lock_note = true;
        }
        let subject = if let Some(msg) = message {
            // Only the first unit gets the full operator message; others get
            // class-scoped subjects so multi-commit messages stay honest.
            if units.is_empty() {
                msg.lines().next().unwrap_or(msg).trim_end().to_string()
            } else {
                suggest_subject(class, &paths)
            }
        } else {
            suggest_subject(class, &paths)
        };
        units.push(CommitUnit {
            class,
            paths,
            subject,
        });
    }
    Ok(CommitPlan {
        units,
        skipped_lock_note,
    })
}

fn suggest_subject(class: FileClass, paths: &[String]) -> String {
    let prefix = class.default_type_prefix();
    let scope = common_scope(paths);
    let verb = class_verb(class, paths);
    let focus = subject_focus(paths);
    let subject = match scope.as_deref() {
        Some(s) => format!("{prefix}({s}): {verb} {focus}"),
        None => format!("{prefix}: {verb} {focus}"),
    };
    if subject.chars().count() > 72 {
        // Prefer dropping the focus before the scope.
        let short = match scope.as_deref() {
            Some(s) => format!("{prefix}({s}): {verb} {} files", paths.len()),
            None => format!("{prefix}: {verb} {} files", paths.len()),
        };
        if short.chars().count() <= 72 {
            short
        } else {
            format!("{prefix}: update {} files", paths.len())
        }
    } else {
        subject
    }
}

/// Conventional-commit scope from a shared top path segment (crate/dir), when
/// unambiguous across the unit's paths.
fn common_scope(paths: &[String]) -> Option<String> {
    if paths.is_empty() {
        return None;
    }
    let first = scope_token(&paths[0])?;
    if paths
        .iter()
        .all(|p| scope_token(p).as_deref() == Some(first.as_str()))
    {
        // Keep scopes short and conventional (no dots/slashes).
        let s = first
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .take(24)
            .collect::<String>();
        if s.is_empty() { None } else { Some(s) }
    } else {
        None
    }
}

fn scope_token(path: &str) -> Option<String> {
    let p = path.replace('\\', "/");
    let mut parts = p.split('/').filter(|s| !s.is_empty());
    let first = parts.next()?;
    // Skip boring roots; prefer the package/crate under monorepo roots.
    if matches!(
        first,
        "src" | "lib" | "bin" | "tests" | "test" | "docs" | "doc" | "." | ".."
    ) {
        return None;
    }
    // cockpit/src/foo.rs → cockpit; packages/hashline/x → hashline when under packages/
    if matches!(first, "packages" | "crates" | "apps" | "services") {
        return parts.next().map(|s| s.to_string());
    }
    Some(first.to_string())
}

fn class_verb(class: FileClass, paths: &[String]) -> &'static str {
    // Light intent from class + path names (no LLM).
    let joined = paths.join(" ");
    let lower = joined.to_ascii_lowercase();
    if lower.contains("fix") || lower.contains("delete") || lower.contains("remove") {
        return "remove";
    }
    if lower.contains("rename") || lower.contains("move") {
        return "move";
    }
    match class {
        FileClass::Test => "cover",
        FileClass::Docs => "document",
        FileClass::Config => "configure",
        FileClass::Lock => "refresh",
        FileClass::Source | FileClass::Other => "update",
    }
}

fn subject_focus(paths: &[String]) -> String {
    if paths.len() == 1 {
        let name = paths[0].rsplit('/').next().unwrap_or(&paths[0]);
        // Drop extension for readability: hashline.rs → hashline
        let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name);
        stem.to_string()
    } else if let Some(scope) = common_scope(paths) {
        format!("{scope} {} files", paths.len())
    } else {
        format!("{} files", paths.len())
    }
}

/// Render a human/agent-readable plan.
pub(crate) fn format_plan(plan: &CommitPlan) -> String {
    let mut out = format!(
        "atomic commit plan: {} commit{}\n",
        plan.units.len(),
        if plan.units.len() == 1 { "" } else { "s" }
    );
    for (i, unit) in plan.units.iter().enumerate() {
        out.push_str(&format!(
            "\n[{}] class={}  subject={:?}\n  paths ({}):\n",
            i + 1,
            unit.class,
            unit.subject,
            unit.paths.len()
        ));
        for p in unit.paths.iter().take(40) {
            out.push_str(&format!("    {p}\n"));
        }
        if unit.paths.len() > 40 {
            out.push_str(&format!("    …(+{} more)\n", unit.paths.len() - 40));
        }
    }
    if plan.skipped_lock_note {
        out.push_str(
            "\nnote: lockfiles are committed last (separate unit) so they do not drown the headline.\n",
        );
    }
    out.push_str(
        "\nRe-run with execute=true to stage each unit and create the commits \
         (in order). Default is plan-only.\n",
    );
    out
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/git_commit_split__tests.rs"]
mod tests;
