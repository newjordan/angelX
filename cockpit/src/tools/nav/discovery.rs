//! Workspace-local discovery policy. Ignore files are descriptor-read, never
//! inherited from an ancestor outside the configured workspace.
use super::*;
use std::cell::RefCell;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Default)]
pub(super) struct SearchOptions {
    pub(super) no_ignore: bool,
    hidden: bool,
}

impl SearchOptions {
    pub(super) fn from_args(args: &Value) -> Result<Self, String> {
        let flag = |name: &str| match args.get(name) {
            None => Ok(false),
            Some(value) => value
                .as_bool()
                .ok_or_else(|| format!("'{name}' must be a boolean")),
        };
        Ok(Self {
            no_ignore: flag("no_ignore")?,
            hidden: flag("hidden")?,
        })
    }

    pub(super) fn path_allowed(self, path: &Path) -> bool {
        for part in path.components() {
            let std::path::Component::Normal(name) = part else {
                continue;
            };
            let name = name.to_string_lossy();
            let lower = name.to_ascii_lowercase();
            // These exclusions cannot be overridden by discovery options.
            if matches!(
                lower.as_str(),
                ".git"
                    | "off-limits"
                    | ".ssh"
                    | ".aws"
                    | ".gnupg"
                    | ".netrc"
                    | ".npmrc"
                    | ".pypirc"
                    | "credentials"
                    | "credentials.json"
                    | "secrets"
                    | "secrets.json"
                    | "id_rsa"
                    | "id_ed25519"
                    | "service-account.json"
                    | "service_account.json"
                    | "terraform.tfstate"
                    | "terraform.tfstate.backup"
            ) || lower == ".env"
                || lower.starts_with(".env.")
                || (lower.contains("client_secret") && lower.ends_with(".json"))
                || matches!(
                    Path::new(lower.as_str())
                        .extension()
                        .and_then(|s| s.to_str()),
                    Some("pem" | "key" | "p12" | "pfx" | "kdbx")
                )
                || (!self.hidden && name.starts_with('.'))
                || (!self.no_ignore && matches!(name.as_ref(), "target" | "node_modules"))
            {
                return false;
            }
        }
        !path.as_os_str().is_empty()
    }
}

struct Rule {
    regex: regex::Regex,
    directory: bool,
    negate: bool,
}

// Git-style wildmatch: slash-aware *, ?, **, ranges, escaped punctuation;
// leading / anchors to the ignore file, slashless patterns match basenames.
fn rule(line: &str) -> Option<Rule> {
    let mut line = line.trim_end_matches('\r').to_string();
    while line.ends_with(' ') && !line.ends_with("\\ ") {
        line.pop();
    }
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let negate = line.starts_with('!');
    let line = if negate { &line[1..] } else { &line };
    let directory = line.ends_with('/');
    let line = line.trim_end_matches('/');
    let anchored = line.contains('/');
    let line = line.strip_prefix('/').unwrap_or(line);
    let mut out = String::from(if anchored { "^" } else { "(?:^|/)" });
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => out.push_str(&regex::escape(&chars.next()?.to_string())),
            '*' => {
                if chars.peek() == Some(&'*') {
                    chars.next();
                    if chars.peek() == Some(&'/') {
                        chars.next();
                        out.push_str("(?:.*/)?");
                    } else {
                        out.push_str(".*");
                    }
                } else {
                    out.push_str("[^/]*");
                }
            }
            '?' => out.push_str("[^/]"),
            '[' => {
                out.push('[');
                if chars.peek() == Some(&'!') {
                    chars.next();
                    out.push('^');
                }
                for ch in chars.by_ref() {
                    out.push(ch);
                    if ch == ']' {
                        break;
                    }
                }
            }
            _ => out.push_str(&regex::escape(&ch.to_string())),
        }
    }
    out.push('$');
    Some(Rule {
        regex: regex::Regex::new(&out).ok()?,
        directory,
        negate,
    })
}

pub(super) struct Policy<'a> {
    root: &'a Path,
    options: SearchOptions,
    rules: RefCell<BTreeMap<PathBuf, Vec<Rule>>>,
}

impl<'a> Policy<'a> {
    pub(super) fn new(root: &'a Path, options: SearchOptions) -> Self {
        Self {
            root,
            options,
            rules: RefCell::new(BTreeMap::new()),
        }
    }

    fn ignored(&self, path: &Path, is_dir: bool) -> Result<bool, String> {
        if self.options.no_ignore {
            return Ok(false);
        }
        let mut ignored = false;
        let mut ancestors = path.ancestors().skip(1).collect::<Vec<_>>();
        ancestors.reverse();
        for dir in ancestors {
            if !self.rules.borrow().contains_key(dir) {
                let rules = match crate::harness::confined_read_limited_no_symlinks(
                    self.root,
                    &dir.join(".gitignore"),
                    SEARCH_FILE_MAX_BYTES,
                ) {
                    Ok(Some(bytes)) => String::from_utf8_lossy(&bytes)
                        .lines()
                        .filter_map(rule)
                        .collect(),
                    Ok(None) => {
                        return Err(format!(
                            "ignore file in {} exceeds search limit",
                            dir.display()
                        ));
                    }
                    Err(_) => Vec::new(),
                };
                self.rules.borrow_mut().insert(dir.to_path_buf(), rules);
            }
            let relative = path
                .strip_prefix(dir)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            for rule in &self.rules.borrow()[dir] {
                if (!rule.directory || is_dir) && rule.regex.is_match(&relative) {
                    ignored = !rule.negate;
                }
            }
        }
        Ok(ignored)
    }

    pub(super) fn allowed(&self, path: &Path, is_dir: bool) -> Result<bool, String> {
        if !self.options.path_allowed(path) {
            return Ok(false);
        }
        // A negated child cannot resurrect an excluded parent directory.
        let mut ancestors = path
            .ancestors()
            .filter(|p| !p.as_os_str().is_empty())
            .collect::<Vec<_>>();
        ancestors.reverse();
        for ancestor in ancestors {
            if self.ignored(ancestor, ancestor != path || is_dir)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn check_scope(&self, path: &Path) -> Result<(), String> {
        if !path.as_os_str().is_empty() && !self.allowed(path, true)? {
            return Err("path is excluded by workspace search policy; use no_ignore/hidden for optional exclusions".into());
        }
        Ok(())
    }
}

pub(super) fn walk(
    root: &Path,
    start: &Path,
    max_files: usize,
    options: SearchOptions,
) -> Result<Vec<PathBuf>, String> {
    let policy = Policy::new(root, options);
    policy.check_scope(start)?;
    let mut files = Vec::new();
    let mut stack = vec![start.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut entries = match confined_read_dir(root, &dir) {
            Ok(entries) => entries,
            Err(_) if dir != start => continue,
            Err(error) => return Err(error),
        };
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        for entry in entries {
            let path = dir.join(entry.name);
            if !policy.allowed(&path, entry.is_dir)? {
                continue;
            }
            if entry.is_dir {
                stack.push(path);
            } else {
                files.push(path);
                if files.len() >= max_files {
                    files.sort();
                    return Ok(files);
                }
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Rank the complete shallow listing before slicing; no traversal-prefix cap can
/// hide a late relevant name. Shared discovery policy is applied by the caller.
pub(super) fn list_window(
    mut entries: Vec<String>,
    path: &str,
    args: &Value,
) -> Result<String, String> {
    let string_arg = |name: &str| -> Result<Option<&str>, String> {
        args.get(name)
            .map(|v| {
                v.as_str()
                    .ok_or_else(|| format!("'{name}' must be a string"))
            })
            .transpose()
    };
    let hint = string_arg("hint")?.map(str::to_lowercase);
    let pattern = string_arg("pattern")?;
    let number = |name: &str, default: usize| -> Result<usize, String> {
        match args.get(name) {
            None => Ok(default),
            Some(v) => v
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| format!("'{name}' must be a nonnegative integer")),
        }
    };
    let offset = number("offset", 0)?;
    let limit = number("limit", 700)?;
    if !(1..=700).contains(&limit) {
        return Err("'limit' must be between 1 and 700 (page window)".into());
    }
    let rank = |entry: &str| {
        let name = entry.trim_end_matches('/');
        let lower = name.to_lowercase();
        let (tier, score) = match hint.as_deref().filter(|h| !h.trim().is_empty()) {
            Some(h) if lower == h => (0, 0),
            Some(h) if lower.contains(h) => (1, 0),
            Some(h) => fuzzy_score(h, name).map_or((3, 0), |s| (2, -s)),
            None => (3, 0),
        };
        let kind = if entry.ends_with('/') {
            0
        } else if matches!(
            Path::new(name).extension().and_then(|x| x.to_str()),
            Some(
                "rs" | "py"
                    | "js"
                    | "ts"
                    | "tsx"
                    | "jsx"
                    | "go"
                    | "c"
                    | "h"
                    | "cpp"
                    | "java"
                    | "rb"
                    | "sh"
                    | "toml"
                    | "json"
                    | "yaml"
                    | "yml"
                    | "md"
            )
        ) {
            1
        } else {
            2
        };
        (
            pattern.is_some_and(|p| !glob_match(p, name)),
            tier,
            score,
            kind,
        )
    };
    entries.sort_by_cached_key(|name| (rank(name), name.clone()));
    if entries.is_empty() {
        return Ok(format!("{path} is empty"));
    }
    let total = entries.len();
    let start = offset.min(total);
    let mut end = start;
    let mut bytes = 0;
    while end < total && end - start < limit {
        let next = entries[end].len() + usize::from(end > start);
        if bytes + next > 24_000 {
            break;
        }
        bytes += next;
        end += 1;
    }
    let mut output = entries[start..end].join("\n");
    if start > 0 || end < total || offset > total {
        if !output.is_empty() {
            output.push('\n');
        }
        let continuation = if end < total {
            format!(
                "next: list_dir with offset={end}, limit={limit}; keep path, hint, pattern, no_ignore and hidden unchanged"
            )
        } else {
            "end of listing; use offset=0 to restart".to_string()
        };
        output.push_str(&format!(
            "[list_dir window: total={total}, shown={}, omitted_before={start}, omitted_after={}; entry_limit={limit}, byte_limit=24000; {continuation}]",
            end - start, total - end
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::super::search_tests::TestWorkspace;
    use super::*;

    #[test]
    fn list_lean_schema_preserves_ranking_and_paging_controls() {
        let full = ListDirTool {
            root: PathBuf::from("."),
        }
        .def()
        .params;
        let lean = crate::harness::lean_list_dir_params();
        for key in [
            "path",
            "hint",
            "pattern",
            "offset",
            "limit",
            "no_ignore",
            "hidden",
        ] {
            assert_eq!(
                full["properties"][key]["type"],
                lean["properties"][key]["type"]
            );
        }
        assert_eq!(
            full["properties"]["limit"]["maximum"],
            lean["properties"]["limit"]["maximum"]
        );
    }

    #[test]
    fn list_late_hint_is_ranked_before_entry_window() {
        let mut entries = (0..900).map(|i| format!("a{i:04}.rs")).collect::<Vec<_>>();
        entries.push("z_target.rs".into());
        let plain = list_window(entries.clone(), ".", &serde_json::json!({})).unwrap();
        assert!(plain.contains("shown=700, omitted_before=0, omitted_after=201"));
        assert!(plain.contains("offset=700, limit=700"));
        let hinted = list_window(entries, ".", &serde_json::json!({"hint":"z_target.rs"})).unwrap();
        assert_eq!(hinted.lines().next(), Some("z_target.rs"));
        assert!(hinted.contains("shown=700, omitted_before=0, omitted_after=201"));
    }

    #[test]
    fn list_ordering_and_paging_preserve_complete_membership() {
        let entries = [
            "aaa.txt",
            "zebra.py",
            "lib/",
            "zebra_copy.py",
            "zebra/",
            "README.md",
        ]
        .map(str::to_string)
        .to_vec();
        let plain = list_window(entries.clone(), ".", &serde_json::json!({})).unwrap();
        assert_eq!(
            plain,
            "lib/\nzebra/\nREADME.md\nzebra.py\nzebra_copy.py\naaa.txt"
        );
        let hinted = list_window(
            entries.clone(),
            ".",
            &serde_json::json!({"hint":"ZEBRA.PY"}),
        )
        .unwrap();
        assert_eq!(hinted.lines().next(), Some("zebra.py"));
        let globbed = list_window(
            entries.clone(),
            ".",
            &serde_json::json!({"pattern":"*.txt"}),
        )
        .unwrap();
        assert_eq!(globbed.lines().next(), Some("aaa.txt"));
        let page = list_window(
            entries.clone(),
            ".",
            &serde_json::json!({"hint":"zebra.py", "limit":2}),
        )
        .unwrap();
        assert!(page.contains("total=6, shown=2, omitted_before=0, omitted_after=4"));
        assert!(page.contains("offset=2, limit=2"));
        let mut collected = Vec::new();
        for offset in [0, 2, 4] {
            let page = list_window(
                entries.clone(),
                ".",
                &serde_json::json!({"hint":"zebra.py", "limit":2, "offset":offset}),
            )
            .unwrap();
            collected.extend(
                page.lines()
                    .filter(|s| !s.starts_with('['))
                    .map(str::to_string),
            );
        }
        assert_eq!(collected.join("\n"), hinted);
        let past = list_window(entries, ".", &serde_json::json!({"offset":99})).unwrap();
        assert!(past.contains("shown=0, omitted_before=6, omitted_after=0"));
        assert!(past.contains("end of listing"));
    }

    #[test]
    fn list_window_reports_byte_cut_and_rejects_invalid_arguments() {
        let entries = (0..300)
            .map(|i| format!("{i:04}_{}.rs", "é".repeat(100)))
            .collect::<Vec<_>>();
        let first = list_window(entries.clone(), ".", &serde_json::json!({})).unwrap();
        let count = first.lines().filter(|s| !s.starts_with('[')).count();
        assert!(count > 0 && count < entries.len());
        assert!(first.contains(&format!("omitted_after={}", entries.len() - count)));
        assert!(first.contains(&format!("offset={count}, limit=700")));
        assert!(first.contains("byte_limit=24000"));
        let second =
            list_window(entries.clone(), ".", &serde_json::json!({"offset":count})).unwrap();
        assert_eq!(second.lines().next(), Some(entries[count].as_str()));
        for args in [
            serde_json::json!({"limit":0}),
            serde_json::json!({"limit":701}),
            serde_json::json!({"offset":-1}),
            serde_json::json!({"hint":true}),
            serde_json::json!({"pattern":3}),
        ] {
            assert!(list_window(entries.clone(), ".", &args).is_err());
        }
    }

    #[test]
    fn list_hint_does_not_override_ignore_or_shallow_scope() {
        let _guard = crate::tests::env_lock();
        let workspace = TestWorkspace::new("discovery-hint-policy");
        workspace.write(".gitignore", "secret-output.py\n");
        workspace.write("secret-output.py", "ignored");
        workspace.write("nested/deep.py", "deep");
        workspace.write("visible.py", "visible");
        let tool = ListDirTool {
            root: workspace.0.clone(),
        };
        let listed = tool
            .call(&serde_json::json!({"hint":"secret-output.py"}))
            .unwrap();
        assert!(!listed.contains("secret-output.py"));
        assert!(!listed.contains("deep.py"));
        let included = tool
            .call(&serde_json::json!({"hint":"secret-output.py", "no_ignore":true}))
            .unwrap();
        assert_eq!(included.lines().next(), Some("secret-output.py"));
    }

    #[test]
    fn list_dir_is_shallow_and_keeps_late_entries_in_large_directories() {
        let _guard = crate::tests::env_lock();
        let workspace = TestWorkspace::new("discovery-list-large");
        for i in 0..600 {
            workspace.write(&format!("wide/f{i:04}.txt"), "filler");
        }
        workspace.write("wide/zz_target.txt", "needle");
        workspace.write("wide/nested/deeper/target.txt", "deep needle");
        let listed = ListDirTool {
            root: workspace.0.clone(),
        }
        .call(&serde_json::json!({"path": "wide"}))
        .unwrap();
        let entries = listed.lines().collect::<Vec<_>>();
        assert_eq!(entries.len(), 602);
        assert_eq!(entries.last(), Some(&"zz_target.txt"));
        assert!(entries.contains(&"nested/"));
        assert!(!listed.contains("deeper"));
        let nested = ListDirTool {
            root: workspace.0.clone(),
        }
        .call(&serde_json::json!({"path": "wide/nested"}))
        .unwrap();
        assert_eq!(nested, "deeper/");
        let files = walk(
            &workspace.0,
            Path::new("wide"),
            700,
            SearchOptions::default(),
        )
        .unwrap();
        assert!(files.contains(&PathBuf::from("wide/nested/deeper/target.txt")));
        let capped = walk(
            &workspace.0,
            Path::new("wide"),
            10,
            SearchOptions::default(),
        )
        .unwrap();
        assert_eq!(capped.len(), 10);
        assert!(!capped.contains(&PathBuf::from("wide/zz_target.txt")));
    }

    #[cfg(unix)]
    #[test]
    fn list_dir_can_scope_to_an_in_tree_symlink_directory() {
        let _guard = crate::tests::env_lock();
        let workspace = TestWorkspace::new("discovery-directory-link");
        workspace.write("src/nested/token.txt", "needle");
        std::os::unix::fs::symlink("src", workspace.0.join("alias")).unwrap();
        let tool = ListDirTool {
            root: workspace.0.clone(),
        };
        assert_eq!(
            tool.call(&serde_json::json!({"path": "alias"})).unwrap(),
            "nested/"
        );
        assert_eq!(
            tool.call(&serde_json::json!({"path": "alias/nested"}))
                .unwrap(),
            "token.txt"
        );
    }

    #[test]
    fn ignored_directories_require_no_ignore_even_with_explicit_scope() {
        let _guard = crate::tests::env_lock();
        let workspace = TestWorkspace::new("discovery-ignored");
        workspace.write(".gitignore", "build/\n");
        for dir in ["build/out", "node_modules/pkg", "target/debug"] {
            let path = format!("{dir}/token.txt");
            workspace.write(&path, "needle\n");
            for no_ignore in [false, true] {
                let options = SearchOptions {
                    no_ignore,
                    hidden: false,
                };
                let files = walk(&workspace.0, Path::new(""), 100, options).unwrap();
                assert_eq!(files.contains(&PathBuf::from(&path)), no_ignore, "{path}");
                let scoped = walk(&workspace.0, Path::new(dir), 100, options);
                if no_ignore {
                    assert_eq!(scoped.unwrap(), vec![PathBuf::from(&path)]);
                } else {
                    assert!(scoped.is_err());
                }
                let found = FindFilesTool {
                    root: workspace.0.clone(),
                }
                .call(&serde_json::json!({"pattern": path, "no_ignore": no_ignore}))
                .unwrap();
                assert_eq!(found.lines().any(|line| line == path), no_ignore);
                let listed = ListDirTool {
                    root: workspace.0.clone(),
                }
                .call(&serde_json::json!({"path": dir, "no_ignore": no_ignore}));
                if no_ignore {
                    assert_eq!(listed.unwrap(), "token.txt");
                } else {
                    assert!(listed.is_err());
                }
            }
        }
    }

    #[test]
    fn hidden_files_and_directories_require_hidden_independently_of_no_ignore() {
        let _guard = crate::tests::env_lock();
        let workspace = TestWorkspace::new("discovery-hidden");
        workspace.write(".note.txt", "needle\n");
        workspace.write(".notes/token.txt", "needle\n");
        workspace.write(".cache/token.txt", "needle\n");
        workspace.write(".gitignore", ".cache/\n");
        for hidden in [false, true] {
            for no_ignore in [false, true] {
                let files = walk(
                    &workspace.0,
                    Path::new(""),
                    100,
                    SearchOptions { no_ignore, hidden },
                )
                .unwrap();
                assert_eq!(files.contains(&PathBuf::from(".note.txt")), hidden);
                assert_eq!(files.contains(&PathBuf::from(".notes/token.txt")), hidden);
                assert_eq!(
                    files.contains(&PathBuf::from(".cache/token.txt")),
                    hidden && no_ignore
                );
                let listed = ListDirTool {
                    root: workspace.0.clone(),
                }
                .call(&serde_json::json!({"hidden": hidden, "no_ignore": no_ignore}))
                .unwrap();
                assert_eq!(listed.lines().any(|line| line == ".note.txt"), hidden);
                assert_eq!(listed.lines().any(|line| line == ".notes/"), hidden);
                assert_eq!(
                    listed.lines().any(|line| line == ".cache/"),
                    hidden && no_ignore
                );
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn in_tree_file_symlink_is_discovered_and_followed() {
        let _guard = crate::tests::env_lock();
        let workspace = TestWorkspace::new("discovery-in-tree");
        workspace.write("src/token.txt", "in_tree_marker\n");
        std::fs::create_dir(workspace.0.join("links")).unwrap();
        std::os::unix::fs::symlink("../src/token.txt", workspace.0.join("links/alias.txt"))
            .unwrap();
        for no_ignore in [false, true] {
            let files = walk(
                &workspace.0,
                Path::new(""),
                100,
                SearchOptions {
                    no_ignore,
                    hidden: false,
                },
            )
            .unwrap();
            assert!(files.contains(&PathBuf::from("links/alias.txt")));
            assert_eq!(
                FindFilesTool {
                    root: workspace.0.clone()
                }
                .call(&serde_json::json!({"pattern": "links/alias.txt", "no_ignore": no_ignore}))
                .unwrap(),
                "links/alias.txt"
            );
            assert_eq!(
                ListDirTool {
                    root: workspace.0.clone()
                }
                .call(&serde_json::json!({"path": "links", "no_ignore": no_ignore}))
                .unwrap(),
                "alias.txt"
            );
            let hits = GrepTool {
                root: workspace.0.clone(),
            }
            .call(&serde_json::json!({"pattern": "in_tree_marker", "no_ignore": no_ignore}))
            .unwrap();
            assert!(
                hits.lines()
                    .any(|line| line.starts_with("links/alias.txt:1:")),
                "{hits}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn escaping_symlink_cannot_be_read_or_used_as_search_scope() {
        let _guard = crate::tests::env_lock();
        let workspace = TestWorkspace::new("discovery-escape");
        let outside = TestWorkspace::new("discovery-outside");
        outside.write("token.txt", "outside_marker\n");
        workspace.write("visible.txt", "inside_marker\n");
        std::os::unix::fs::symlink(outside.0.join("token.txt"), workspace.0.join("escape.txt"))
            .unwrap();
        std::os::unix::fs::symlink(&outside.0, workspace.0.join("escape-dir")).unwrap();
        for hidden in [false, true] {
            for no_ignore in [false, true] {
                let options = SearchOptions { no_ignore, hidden };
                assert!(confined_read(&workspace.0, Path::new("escape.txt")).is_err());
                assert!(walk(&workspace.0, Path::new("escape-dir"), 100, options).is_err());
                assert!(ListDirTool { root: workspace.0.clone() }
                    .call(&serde_json::json!({"path": "escape-dir", "no_ignore": no_ignore, "hidden": hidden})).is_err());
                let grep = GrepTool {
                    root: workspace.0.clone(),
                };
                assert_eq!(grep.call(&serde_json::json!({"pattern": "outside_marker", "no_ignore": no_ignore, "hidden": hidden})).unwrap(), "no matches");
                for path in ["escape.txt", "escape-dir"] {
                    assert!(grep.call(&serde_json::json!({"pattern": "outside_marker", "path": path, "no_ignore": no_ignore, "hidden": hidden})).is_err());
                }
            }
        }
    }

    #[test]
    fn grep_keeps_workspace_local_scope_inside_ignored_ancestor_repository() {
        let _guard = crate::tests::env_lock();
        let parent = TestWorkspace::new("discovery-grep-parent");
        parent.write(".gitignore", "work/\n");
        parent.write("work/src/token.txt", "scope_marker\n");
        parent.write("work/build/token.txt", "scope_marker\n");
        parent.write("work/.gitignore", "build/\n");
        assert!(
            std::process::Command::new("git")
                .args(["init", "--quiet"])
                .arg(&parent.0)
                .status()
                .unwrap()
                .success()
        );
        let root = parent.0.join("work");
        assert_eq!(run_git(&root, &git_listing_args(None)).unwrap(), "");
        assert!(!workspace_is_git_root(&root));
        let grep = GrepTool { root };
        for path in [".", "src", "src/token.txt"] {
            let hits = grep
                .call(&serde_json::json!({"pattern": "scope_marker", "path": path}))
                .unwrap();
            assert!(
                hits.lines()
                    .any(|line| line.starts_with("src/token.txt:1:")),
                "{hits}"
            );
            assert!(!hits.contains("build/token.txt"));
        }
        let hits = grep
            .call(&serde_json::json!({"pattern": "scope_marker", "no_ignore": true}))
            .unwrap();
        assert!(
            hits.lines()
                .any(|line| line.starts_with("build/token.txt:1:")),
            "{hits}"
        );
    }

    #[test]
    fn ignore_patterns_cover_anchoring_negation_and_escaping() {
        for (pattern, path, expected) in [
            ("*.log", "a/b/output.log", true),
            ("/build", "nested/build", false),
            ("/build", "build", true),
            ("a/**/b", "a/b", true),
            ("a/**/b", "a/x/y/b", true),
            ("a/*/b", "a/x/y/b", false),
            (r"\#file", "#file", true),
            (r"\!file", "!file", true),
            ("file[0-9].txt", "file3.txt", true),
            ("file[!0-9].txt", "file3.txt", false),
            (r"with\ space\ ", "with space ", true),
        ] {
            assert_eq!(
                rule(pattern).unwrap().regex.is_match(path),
                expected,
                "{pattern}: {path}"
            );
        }
        assert!(rule("#comment").is_none());
        assert!(rule("!keep.log").unwrap().negate);
        assert!(rule("build/").unwrap().directory);
    }

    #[test]
    fn nested_ignore_negation_and_excluded_parents_agree() {
        let workspace = super::super::search_tests::TestWorkspace::new("nested-ignore");
        workspace.write(
            ".gitignore",
            "*.log\n!keep.log\nblocked/\n!blocked/keep.txt\n",
        );
        workspace.write("src/.gitignore", "!nested.log\n");
        let policy = Policy::new(&workspace.0, SearchOptions::default());
        for (path, allowed) in [
            ("a.log", false),
            ("keep.log", true),
            ("src/nested.log", true),
            ("blocked/keep.txt", false),
        ] {
            assert_eq!(
                policy.allowed(Path::new(path), false).unwrap(),
                allowed,
                "{path}"
            );
        }
    }
}
