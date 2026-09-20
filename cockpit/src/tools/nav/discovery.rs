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
#[path = "../../../../tests/cockpit/tools/nav__discovery__tests.rs"]
mod tests;
