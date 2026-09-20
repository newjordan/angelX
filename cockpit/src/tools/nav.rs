//! Navigation / search tools: `outline` (top-level symbol map), `list_dir`,
//! `grep` (in-process regex over descriptor-confined reads),
//! `find_files` (glob), `file_search` (fuzzy filename), and `defs` (symbol
//! definitions). Filesystem access goes through the descriptor-anchored
//! workspace helpers so validation and use cannot be separated by a symlink
//! swap.

use crate::club::ToolDef;
use crate::harness::{
    Tool, confined_read, confined_read_dir, confined_read_limited, env_flag, env_usize,
    lexical_normalize, run_git, workspace_relative,
};
use regex::RegexBuilder;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

mod definition_bundle;
mod discovery;
use discovery::SearchOptions;

/// A bounded source observation for Atlas. Reuse discovery's hard exclusions,
/// and reject aliases at every component so an allowed name cannot reach an
/// excluded tree. This remains confined even in an unrestricted shell profile.
pub(crate) fn atlas_source(root: &Path, supplied: &str) -> Result<(String, String), String> {
    let relative = confine_nav_target(root, supplied)?;
    let policy = discovery::Policy::new(root, SearchOptions::default());
    policy.check_scope(&relative)?;
    let path = relative.to_str().ok_or("source path must be UTF-8")?;
    if path.is_empty() || path.len() > 160 || path.contains(['\n', '\r']) {
        return Err("source path must be 1–160 bytes without newlines".into());
    }
    let bytes = crate::harness::confined_read_limited_no_symlinks(root, &relative, 512 * 1024)?
        .ok_or("Atlas source exceeds 512 KiB")?;
    let source = String::from_utf8(bytes).map_err(|_| "Atlas source must be UTF-8")?;
    if source.contains('\0') {
        return Err("Atlas source must be text".into());
    }
    Ok((path.to_string(), source))
}

/// Lexical workspace check shared by read-only navigation tools. Outside,
/// escaping, NUL, and overlong targets are refused with the same wording as
/// write tools — never a silent empty listing.
const NAV_PATH_MAX_CHARS: usize = 2048;

pub(crate) fn confine_nav_target(root: &Path, supplied: &str) -> Result<PathBuf, String> {
    if supplied.contains('\0') {
        return Err("path contains a nul byte; outside the workspace".into());
    }
    if supplied.chars().count() > NAV_PATH_MAX_CHARS {
        return Err("path is overlong; outside the workspace".into());
    }
    workspace_relative(root, Path::new(supplied))
}

/// True if a trimmed source line begins a notable top-level/item declaration
/// (Rust/Python/JS/Go/Java-ish), after stripping common leading modifiers. A
/// cheap repomap heuristic — not a parser, but enough to navigate a file.
pub(crate) fn is_outline_decl(line: &str) -> bool {
    let mut s = line.trim_start();
    // Visibility / export modifiers. Do not strip Rust `static`/`const` here —
    // those are declaration keywords and must still match KW.
    const MODS: &[&str] = &[
        "pub(crate) ",
        "pub(super) ",
        "pub ",
        "public ",
        "private ",
        "protected ",
        "async ",
        "unsafe ",
        "default ",
        "export ",
        "abstract ",
        "final ",
    ];
    loop {
        let mut stripped = false;
        for m in MODS {
            if let Some(rest) = s.strip_prefix(m) {
                s = rest.trim_start();
                stripped = true;
                break;
            }
        }
        if !stripped {
            break;
        }
    }
    const KW: &[&str] = &[
        "fn ",
        "struct ",
        "enum ",
        "trait ",
        "impl ",
        "mod ",
        "type ",
        "const ",
        "static ",
        "def ",
        "class ",
        "function ",
        "func ",      // Go
        "interface ", // Go / TS / Java
        "package ",   // Go / Java
        "macro_rules!",
    ];
    if KW.iter().any(|k| s.starts_with(k)) {
        return true;
    }
    // Java `static` methods after visibility was stripped: `static void run(`.
    while let Some(rest) = s.strip_prefix("static ") {
        s = rest.trim_start();
    }
    // Method-ish residual: "void run(" / "String name(" (not fields ending in ';').
    if s.contains('(') && !s.trim_end().ends_with(';') {
        let head = s.split('(').next().unwrap_or("");
        let tokens: Vec<&str> = head.split_whitespace().collect();
        if tokens.len() >= 2 {
            return true;
        }
    }
    false
}

pub(crate) struct OutlineTool {
    pub(crate) root: PathBuf,
}
impl Tool for OutlineTool {
    fn name(&self) -> &str {
        "outline"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "outline".to_string(),
            description: "List the top-level symbols of a workspace source file (fn/struct/\
                          enum/trait/impl/mod/type/const, plus py def/class, js function/class, \
                          Go func/interface/package, and Java-style visibility methods) with line \
                          numbers — a fast map without reading the whole file."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": { "path": { "type": "string", "description": "path inside the workspace (relative or absolute)" } },
                "required": ["path"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let path = args["path"].as_str().ok_or("missing 'path'")?;
        confine_nav_target(&self.root, path)?;
        // Same did-you-mean hints as read_file: a path-guess miss should cost
        // one corrected call, not a listing round trip (12 bare misses in one
        // night's trajectories, mostly outline/list_dir).
        let bytes = confined_read(&self.root, Path::new(path)).map_err(|error| {
            crate::tools::file::with_missing_path_hints(&self.root, Path::new(path), error)
        })?;
        let content = String::from_utf8(bytes).map_err(|e| format!("read {path}: {e}"))?;
        let mut out: Vec<String> = Vec::new();
        for (i, raw) in content.lines().enumerate() {
            if is_outline_decl(raw) {
                out.push(format!("{}: {}", i + 1, raw.trim_end()));
                if out.len() >= 400 {
                    out.push("…[truncated at 400 symbols]".to_string());
                    break;
                }
            }
        }
        if out.is_empty() {
            Ok(format!("no top-level symbols found in {path}"))
        } else {
            Ok(out.join("\n"))
        }
    }
}

pub(crate) struct ListDirTool {
    pub(crate) root: PathBuf,
}
impl Tool for ListDirTool {
    fn name(&self) -> &str {
        "list_dir"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "list_dir".to_string(),
            description: "List the entries of a workspace directory (default: workspace \
                          root). Directories are suffixed with '/'. Optional hint/pattern ranks entries; \
                          otherwise directories and source/config files come first. Results report paging when truncated."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "hint": { "type": "string", "description": "filename fragment to rank first (exact, substring, then fuzzy); does not filter" },
                    "pattern": { "type": "string", "description": "basename glob to rank first; does not filter" },
                    "offset": { "type": "integer", "minimum": 0, "description": "zero-based entry offset in the sorted listing" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 700, "description": "page size (default 700); 24000-byte page window also applies; omitted counts and continuation are reported" },
                    "no_ignore": { "type": "boolean", "description": "include ignored/generated paths (default false); credential and quarantine exclusions still apply" },
                    "hidden": { "type": "boolean", "description": "include hidden paths (default false); .git and credential files remain excluded" }, "path": { "type": "string", "description": "workspace-relative directory (default '.')" } },
                "required": [],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let path = args["path"].as_str().unwrap_or(".");
        let options = SearchOptions::from_args(args)?;
        let relative = workspace_relative(&self.root, Path::new(path))?;
        let policy = discovery::Policy::new(&self.root, options);
        policy.check_scope(&relative)?;
        let mut entries: Vec<String> = Vec::new();
        for e in confined_read_dir(&self.root, Path::new(path)).map_err(|error| {
            crate::tools::file::with_missing_path_hints(&self.root, Path::new(path), error)
        })? {
            if !policy.allowed(&relative.join(&e.name), e.is_dir)? {
                continue;
            }
            let name = e.name.to_string_lossy().into_owned();
            let is_dir = e.is_dir;
            entries.push(if is_dir { format!("{name}/") } else { name });
        }
        if !options.no_ignore && workspace_is_git_root(&self.root) && !entries.is_empty() {
            let paths = entries
                .iter()
                .map(|name| relative.join(name).to_string_lossy().into_owned())
                .collect::<Vec<_>>();
            let mut args = vec!["check-ignore", "-z", "--"];
            args.extend(paths.iter().map(String::as_str));
            if let Ok(raw) = run_git(&self.root, &args) {
                let ignored = raw.split_terminator('\0').collect::<BTreeSet<_>>();
                entries.retain(|name| {
                    !ignored.contains(relative.join(name).to_string_lossy().as_ref())
                });
            }
        }
        discovery::list_window(entries, path, args)
    }
}

/// Max `path:line:text` lines any grep returns.
pub(crate) const GREP_MAX_LINES: usize = 300;
pub(crate) const GREP_MAX_CONTEXT_LINES: usize = 10;
pub(crate) const SEARCH_FILE_MAX_BYTES: usize = 2 * 1024 * 1024;
const GREP_MAX_PATHS: usize = 8;
const GREP_MAX_SKIP_FILES: usize = 128;
const GREP_MAX_MATCHES_PER_FILE: usize = 20;
const GREP_MAX_RESULT_LINES_PER_FILE: usize = 60;
const GREP_MAX_OUTPUT_BYTES: usize = 128 * 1024;
const GREP_RECEIPT_RESERVE_BYTES: usize = 20 * 1024;
const GREP_MAX_RECEIPT_LINES: usize = 3;
const GREP_MAX_OUTPUT_LINE_BYTES: usize = 1800;
const GREP_RECEIPT_FILE_NAMES: usize = 8;

pub(crate) struct GrepTool {
    pub(crate) root: PathBuf,
}
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "grep".to_string(),
            description:
                "Search one path, or up to eight workspace files/directories, for a regular-expression \
                          pattern. Uses Git ignore rules when available and skips hidden, credential, \
                          key, and quarantined files. Results are deterministic and diverse across \
                          files; a bounded receipt provides `after_file` when another page exists."
                    .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "no_ignore": { "type": "boolean", "description": "include ignored/generated paths (default false); credential and quarantine exclusions still apply" },
                    "hidden": { "type": "boolean", "description": "include hidden paths (default false); .git and credential files remain excluded" },
                    "pattern": { "type": "string", "description": "regular expression" },
                    "path": { "type": "string", "description": "file or directory inside the workspace, relative or absolute (default '.'); may be combined with `paths` — both are merged and deduplicated" },
                    "paths": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": GREP_MAX_PATHS,
                        "items": { "type": "string" },
                        "description": "1-8 files/directories to search as one sorted, deduplicated union; both `path` and `paths` together are accepted (merged, deduplicated)"
                    },
                    "after_file": {
                        "type": "string",
                        "description": "deterministic continuation cursor from a prior grep receipt; only files lexically after this workspace-relative path are considered"
                    },
                    "skip_files": {
                        "type": "array",
                        "maxItems": GREP_MAX_SKIP_FILES,
                        "items": { "type": "string" },
                        "description": "workspace-relative files to omit (maximum 128); useful for bounded explicit resumption/exclusion"
                    },
                    "ignore_case": { "type": "boolean", "description": "case-insensitive (default false)" },
                    "context": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": GREP_MAX_CONTEXT_LINES,
                        "description": "lines before and after each match (default 0, maximum 10); overlapping windows are merged"
                    },
                },
                "required": ["pattern"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let pattern = args["pattern"].as_str().ok_or("missing 'pattern'")?;
        if pattern.is_empty() {
            return Err("'pattern' must not be empty".to_string());
        }
        let starts = grep_start_paths(args)?;
        let after_file = grep_continuation_path(
            &self.root,
            args.get("after_file"),
            "after_file",
            SearchOptions::from_args(args)?,
        )?;
        let skip_files = grep_skip_files(
            &self.root,
            args.get("skip_files"),
            SearchOptions::from_args(args)?,
        )?;
        let ignore_case = args["ignore_case"].as_bool().unwrap_or(false);
        let context = match args.get("context") {
            None => env_usize("ANGEL_GREP_CONTEXT_LINES", 0).min(GREP_MAX_CONTEXT_LINES),
            Some(value) => value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| *value <= GREP_MAX_CONTEXT_LINES)
                .ok_or_else(|| {
                    format!("'context' must be an integer from 0 through {GREP_MAX_CONTEXT_LINES}")
                })?,
        };
        grep_confined_many_with_options(
            &self.root,
            &starts,
            &skip_files,
            after_file.as_deref(),
            pattern,
            ignore_case,
            env_flag("ANGEL_GREP_FILE_SCOPE", true),
            context,
            SearchOptions::from_args(args)?,
        )
    }
}

fn grep_start_paths(args: &Value) -> Result<Vec<PathBuf>, String> {
    let single = args.get("path").filter(|value| match value {
        Value::Null => false,
        Value::String(path) => !path.trim().is_empty(),
        _ => true,
    });
    let batch = args.get("paths").filter(|value| match value {
        Value::Null => false,
        Value::Array(paths) => !paths.is_empty(),
        _ => true,
    });
    // `path` + `paths` together used to be a hard schema error the model kept
    // retrying (916 live occurrences). Merge instead: `path` first, then
    // `paths`, deduplicated in order. Only a missing/unusable scope fails.
    let mut requested: Vec<String> = Vec::new();
    if let Some(path) = single {
        requested.push(
            path.as_str()
                .ok_or("'path' must be a string")?
                .trim()
                .to_string(),
        );
    }
    if let Some(paths) = batch {
        let paths = paths.as_array().ok_or("'paths' must be an array")?;
        for path in paths {
            requested.push(
                path.as_str()
                    .ok_or("every 'paths' item must be a string")?
                    .trim()
                    .to_string(),
            );
        }
    }
    if requested.is_empty() {
        return Ok(vec![PathBuf::from(".")]);
    }
    if requested.len() > GREP_MAX_PATHS {
        return Err(format!(
            "'paths' must contain 1 through {GREP_MAX_PATHS} workspace paths"
        ));
    }
    let mut out = Vec::with_capacity(requested.len());
    for path in requested {
        if path.is_empty() {
            return Err("'paths' items must not be empty".to_string());
        }
        if path.contains('\0') {
            return Err("path contains a nul byte; outside the workspace".into());
        }
        if path.chars().count() > NAV_PATH_MAX_CHARS {
            return Err("path is overlong; outside the workspace".into());
        }
        let path = PathBuf::from(path);
        if !out.contains(&path) {
            out.push(path);
        }
    }
    Ok(out)
}

fn grep_continuation_path(
    root: &Path,
    value: Option<&Value>,
    field: &str,
    options: SearchOptions,
) -> Result<Option<PathBuf>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() || value.as_str().is_some_and(str::is_empty) {
        return Ok(None);
    }
    let raw = value
        .as_str()
        .ok_or_else(|| format!("'{field}' must be a string"))?;
    if raw.trim().is_empty() {
        return Err(format!("'{field}' must not be empty"));
    }
    let relative = workspace_relative(root, Path::new(raw))?;
    if relative.as_os_str().is_empty() || !options.path_allowed(&relative) {
        return Err(format!("'{field}' is excluded by workspace search policy"));
    }
    Ok(Some(relative))
}

fn grep_skip_files(
    root: &Path,
    value: Option<&Value>,
    options: SearchOptions,
) -> Result<BTreeSet<PathBuf>, String> {
    let Some(value) = value else {
        return Ok(BTreeSet::new());
    };
    let values = value.as_array().ok_or("'skip_files' must be an array")?;
    if values.len() > GREP_MAX_SKIP_FILES {
        return Err(format!(
            "'skip_files' accepts at most {GREP_MAX_SKIP_FILES} paths"
        ));
    }
    let mut skipped = BTreeSet::new();
    for value in values {
        let raw = value
            .as_str()
            .ok_or("every 'skip_files' item must be a string")?;
        if raw.trim().is_empty() {
            return Err("'skip_files' items must not be empty".to_string());
        }
        let relative = workspace_relative(root, Path::new(raw))?;
        if relative.as_os_str().is_empty() || !options.path_allowed(&relative) {
            return Err("a 'skip_files' item is excluded by workspace search policy".to_string());
        }
        skipped.insert(relative);
    }
    Ok(skipped)
}

/// Resolve every requested scope before reading search content. Each path is
/// descriptor-checked independently, then the union is sorted and deduplicated.
fn collect_grep_files(
    root: &Path,
    starts: &[PathBuf],
    allow_file_scope: bool,
    options: SearchOptions,
) -> Result<Vec<PathBuf>, String> {
    let mut files = BTreeSet::new();
    for start in starts {
        let start = workspace_relative(root, start)?;
        // Reject an excluded scope before even probing whether it is a file or
        // directory. In particular, quarantine names must never be listed as a
        // side effect of argument validation.
        if !start.as_os_str().is_empty() && !options.path_allowed(&start) {
            return Err("grep path is excluded by workspace search policy".to_string());
        }
        discovery::Policy::new(root, options).check_scope(&start)?;
        match confined_read_dir(root, &start) {
            Ok(_) => {
                files.extend(search_workspace_files_with_options(
                    root, &start, 50_000, options,
                )?);
            }
            Err(_directory_error) if allow_file_scope => {
                if !options.path_allowed(&start) {
                    return Err("grep path is excluded by workspace search policy".to_string());
                }
                match confined_read_limited(root, &start, SEARCH_FILE_MAX_BYTES) {
                    Ok(Some(_)) => {
                        files.insert(start);
                    }
                    Ok(None) => {
                        return Err(format!(
                            "grep file exceeds the {SEARCH_FILE_MAX_BYTES}-byte search limit"
                        ));
                    }
                    Err(file_error) => return Err(file_error),
                }
            }
            Err(error) => return Err(error),
        }
    }
    Ok(files.into_iter().take(50_000).collect())
}

fn truncate_grep_line(mut line: String) -> (String, bool) {
    if line.len() <= GREP_MAX_OUTPUT_LINE_BYTES {
        return (line, false);
    }
    let suffix = "…[line truncated]";
    let mut keep = GREP_MAX_OUTPUT_LINE_BYTES.saturating_sub(suffix.len());
    while keep > 0 && !line.is_char_boundary(keep) {
        keep -= 1;
    }
    line.truncate(keep);
    line.push_str(suffix);
    (line, true)
}

fn grep_file_results(
    rel: &Path,
    text: &str,
    regex: &regex::Regex,
    context: usize,
    match_cap: usize,
    result_line_cap: usize,
) -> (Vec<String>, bool) {
    let lines = text.lines().collect::<Vec<_>>();
    let mut matches = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| regex.is_match(line).then_some(index))
        .take(match_cap + 1)
        .collect::<Vec<_>>();
    let mut truncated = matches.len() > match_cap;
    matches.truncate(match_cap);
    if matches.is_empty() {
        return (Vec::new(), false);
    }

    let mut results = Vec::new();
    let mut push = |line: String| {
        if results.len() >= result_line_cap {
            truncated = true;
            return false;
        }
        let (line, line_truncated) = truncate_grep_line(line);
        truncated |= line_truncated;
        results.push(line);
        true
    };
    if context == 0 {
        for index in matches {
            if !push(format!(
                "{}:{}:{}",
                rel.display(),
                index + 1,
                lines[index].trim_end()
            )) {
                break;
            }
        }
        return (results, truncated);
    }

    let mut windows = Vec::<(usize, usize)>::new();
    for index in &matches {
        let start = index.saturating_sub(context);
        let end = index
            .saturating_add(context)
            .saturating_add(1)
            .min(lines.len());
        if let Some((_, previous_end)) = windows.last_mut()
            && start <= *previous_end
        {
            *previous_end = (*previous_end).max(end);
            continue;
        }
        windows.push((start, end));
    }
    'windows: for (window_index, (start, end)) in windows.into_iter().enumerate() {
        if window_index > 0 && !push("--".to_string()) {
            break;
        }
        for (index, line) in lines.iter().enumerate().take(end).skip(start) {
            let separator = if matches.binary_search(&index).is_ok() {
                ':'
            } else {
                '-'
            };
            if !push(format!(
                "{}{separator}{}{separator}{}",
                rel.display(),
                index + 1,
                line.trim_end()
            )) {
                break 'windows;
            }
        }
    }
    (results, truncated)
}

fn receipt_path(path: &Path) -> String {
    let raw = path.to_string_lossy().replace('\\', "/");
    let mut end = raw.len().min(240);
    while end > 0 && !raw.is_char_boundary(end) {
        end -= 1;
    }
    if end == raw.len() {
        raw
    } else {
        format!("{}…", &raw[..end])
    }
}

fn receipt_names(paths: impl IntoIterator<Item = PathBuf>) -> String {
    let paths = paths.into_iter().collect::<Vec<_>>();
    let names = paths
        .iter()
        .take(GREP_RECEIPT_FILE_NAMES)
        .map(|path| receipt_path(path))
        .collect::<Vec<_>>()
        .join(", ");
    if paths.len() > GREP_RECEIPT_FILE_NAMES {
        format!("{names} (+{} more)", paths.len() - GREP_RECEIPT_FILE_NAMES)
    } else {
        names
    }
}

/// Race-resistant regex search. File discovery and each read are descriptor-
/// relative, so a directory swapped to an outbound symlink is skipped or
/// rejected instead of becoming a read outside the workspace.
#[allow(clippy::too_many_arguments)]
fn grep_confined_many(
    root: &Path,
    starts: &[PathBuf],
    skip_files: &BTreeSet<PathBuf>,
    after_file: Option<&Path>,
    pattern: &str,
    ignore_case: bool,
    allow_file_scope: bool,
    context: usize,
) -> Result<String, String> {
    grep_confined_many_with_options(
        root,
        starts,
        skip_files,
        after_file,
        pattern,
        ignore_case,
        allow_file_scope,
        context,
        SearchOptions::default(),
    )
}

#[allow(clippy::too_many_arguments)]
fn grep_confined_many_with_options(
    root: &Path,
    starts: &[PathBuf],
    skip_files: &BTreeSet<PathBuf>,
    after_file: Option<&Path>,
    pattern: &str,
    ignore_case: bool,
    allow_file_scope: bool,
    context: usize,
    options: SearchOptions,
) -> Result<String, String> {
    let regex = RegexBuilder::new(pattern)
        .case_insensitive(ignore_case)
        .build()
        .map_err(|e| format!("grep error: {e}"))?;
    let files = collect_grep_files(root, starts, allow_file_scope, options)?;
    let input_skipped = files
        .iter()
        .filter(|file| skip_files.contains(*file))
        .cloned()
        .collect::<Vec<_>>();
    let files = files
        .into_iter()
        .filter(|file| {
            !skip_files.contains(file) && after_file.is_none_or(|cursor| file.as_path() > cursor)
        })
        .collect::<Vec<_>>();

    // Diversity caps matter only when files compete for the same response.
    // Preserve the legacy capacity of an explicitly scoped single-file grep;
    // callers use that shape for dense, contextual inspection and historically
    // received up to the global response budget.
    let (match_cap, result_line_cap) = if files.len() > 1 {
        (GREP_MAX_MATCHES_PER_FILE, GREP_MAX_RESULT_LINES_PER_FILE)
    } else {
        let legacy_line_cap = GREP_MAX_LINES.saturating_sub(GREP_MAX_RECEIPT_LINES);
        (legacy_line_cap, legacy_line_cap)
    };

    let mut hits = Vec::new();
    let mut hit_bytes = 0usize;
    let mut truncated_files = BTreeSet::new();
    let mut first_omitted_file = None;
    let mut last_emitted_file = None;
    let data_byte_cap = GREP_MAX_OUTPUT_BYTES.saturating_sub(GREP_RECEIPT_RESERVE_BYTES);
    for rel in files {
        let Ok(Some(bytes)) = confined_read_limited(root, &rel, SEARCH_FILE_MAX_BYTES) else {
            continue;
        };
        if bytes.iter().take(8000).any(|&byte| byte == 0) {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        let (file_hits, locally_truncated) =
            grep_file_results(&rel, &text, &regex, context, match_cap, result_line_cap);
        if file_hits.is_empty() {
            continue;
        }
        let block_bytes = file_hits
            .iter()
            .map(|line| line.len().saturating_add(1))
            .sum::<usize>();
        if hits.len().saturating_add(file_hits.len())
            > GREP_MAX_LINES.saturating_sub(GREP_MAX_RECEIPT_LINES)
            || hit_bytes.saturating_add(block_bytes) > data_byte_cap
        {
            first_omitted_file = Some(rel);
            break;
        }
        if locally_truncated {
            truncated_files.insert(rel.clone());
        }
        hit_bytes = hit_bytes.saturating_add(block_bytes);
        hits.extend(file_hits);
        last_emitted_file = Some(rel);
    }

    let mut receipts = Vec::new();
    if !input_skipped.is_empty() {
        receipts.push(format!(
            "[grep receipt: skipped {} requested file(s): {}]",
            input_skipped.len(),
            receipt_names(input_skipped)
        ));
    }
    if !truncated_files.is_empty() {
        receipts.push(format!(
            "[grep receipt: per-file caps omitted additional matches/context in {} file(s): {}]",
            truncated_files.len(),
            receipt_names(truncated_files.into_iter().collect::<Vec<_>>())
        ));
    }
    if let Some(omitted) = first_omitted_file {
        let first = receipt_path(&omitted);
        if let Some(cursor) = last_emitted_file {
            let cursor = cursor.to_string_lossy().replace('\\', "/");
            let encoded = serde_json::to_string(&cursor).unwrap_or_else(|_| "\"\"".into());
            receipts.push(format!(
                "[grep continuation: output cap reached before {first}; resume without duplicates with \"after_file\":{encoded}]"
            ));
        } else {
            receipts.push(format!(
                "[grep receipt: output cap reached before {first}; narrow the path or pattern]"
            ));
        }
    }

    if hits.is_empty() && receipts.is_empty() {
        return Ok("no matches".to_string());
    }
    if hits.is_empty() {
        return Ok(format!("no matches\n{}", receipts.join("\n")));
    }
    hits.extend(receipts);
    let output = hits.join("\n");
    debug_assert!(output.len() <= GREP_MAX_OUTPUT_BYTES);
    Ok(output)
}

fn grep_confined(
    root: &Path,
    start: &Path,
    pattern: &str,
    ignore_case: bool,
    allow_file_scope: bool,
    context: usize,
) -> Result<String, String> {
    grep_confined_many(
        root,
        &[start.to_path_buf()],
        &BTreeSet::new(),
        None,
        pattern,
        ignore_case,
        allow_file_scope,
        context,
    )
}

/// Enumerate the same broad set users expect from ripgrep: tracked files plus
/// untracked, non-ignored files in a Git worktree. Git only supplies names; all
/// metadata/content access still goes through the confined descriptor layer.
/// A non-Git workspace falls back to the secure walker.
fn search_workspace_files(
    root: &Path,
    start: &Path,
    max_files: usize,
) -> Result<Vec<PathBuf>, String> {
    search_workspace_files_with_options(root, start, max_files, SearchOptions::default())
}

fn search_workspace_files_with_options(
    root: &Path,
    start: &Path,
    max_files: usize,
    options: SearchOptions,
) -> Result<Vec<PathBuf>, String> {
    debug_assert!(!start.is_absolute());
    // A scoped grep should not first enumerate every tracked path in a large
    // repository only to discard almost all of them locally. Git's pathspec
    // preserves its ignore/untracked semantics and is merely a name source;
    // every actual metadata/content operation below remains descriptor-confined.
    let start_pathspec = (!start.as_os_str().is_empty())
        .then(|| start.to_str())
        .flatten();
    let git_args = git_listing_args(start_pathspec);
    // A nested workspace must not inherit an ancestor repository's ignore of
    // the workspace itself (e.g. the cohort under .tmp). Its own rules apply.
    if options.no_ignore || !workspace_is_git_root(root) {
        return discovery::walk(root, start, max_files, options);
    }
    let mut files = match run_git(root, &git_args) {
        Ok(raw) => match parse_git_file_listing(&raw) {
            Some(files) => files,
            None => discovery::walk(root, start, max_files, options)?,
        },
        Err(_) => discovery::walk(root, start, max_files, options)?,
    };
    let policy = discovery::Policy::new(root, options);
    let mut allowed_files = Vec::new();
    for path in files {
        if (start.as_os_str().is_empty() || path.starts_with(start))
            && policy.allowed(&path, false)?
        {
            allowed_files.push(path);
        }
    }
    files = allowed_files;
    files.sort();
    files.dedup();
    files.truncate(max_files);
    Ok(files)
}

fn workspace_is_git_root(root: &Path) -> bool {
    run_git(root, &["rev-parse", "--show-toplevel"])
        .ok()
        .and_then(|path| std::fs::canonicalize(path.trim()).ok())
        .zip(std::fs::canonicalize(root).ok())
        .is_some_and(|(git_root, workspace)| git_root == workspace)
}

fn git_listing_args(start: Option<&str>) -> Vec<&str> {
    let mut args = vec![
        "ls-files",
        "--cached",
        "--others",
        "--exclude-standard",
        "-z",
    ];
    if let Some(start) = start.filter(|path| !path.is_empty()) {
        args.extend(["--", start]);
    }
    args
}

const GIT_OUTPUT_CAP_BYTES: usize = 1 << 20;

fn parse_git_file_listing(raw: &str) -> Option<Vec<PathBuf>> {
    // `run_git` retains at most 1 MiB of stdout. Exactly hitting that boundary
    // is ambiguous, and a missing terminal NUL proves the final path is partial.
    if raw.len() >= GIT_OUTPUT_CAP_BYTES || (!raw.is_empty() && !raw.ends_with('\0')) {
        return None;
    }
    Some(
        raw.split_terminator('\0')
            .map(PathBuf::from)
            .filter(|path| lexical_normalize(path).is_some())
            .collect(),
    )
}

pub(crate) fn search_path_allowed(path: &Path) -> bool {
    SearchOptions::default().path_allowed(path)
}

#[cfg(test)]
fn search_dir_excluded(name: &str) -> bool {
    name.starts_with('.') || matches!(name, "target" | "node_modules" | "off-limits")
}

/// Return bounded, deterministic candidates for a missing model-supplied path.
/// Candidates must share the requested basename and pass the same workspace
/// search policy as grep/find. A longer matching suffix wins, so a request for
/// `scripts/tool.mjs` prefers `plugins/x/scripts/tool.mjs` over an unrelated
/// `examples/tool.mjs`. This helper never reads or substitutes a candidate.
pub(crate) fn suggest_workspace_paths(root: &Path, requested: &Path, limit: usize) -> Vec<String> {
    const MAX_CANDIDATE_FILES: usize = 50_000;
    const MAX_CANDIDATE_CHARS: usize = 240;

    if limit == 0 {
        return Vec::new();
    }
    let Ok(requested) = workspace_relative(root, requested) else {
        return Vec::new();
    };
    let requested = requested.to_string_lossy().replace('\\', "/");
    let requested_parts = requested
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let Some(requested_basename) = requested_parts.last() else {
        return Vec::new();
    };
    if confined_read_dir(root, Path::new("")).is_err() {
        return Vec::new();
    }
    let Ok(files) = search_workspace_files(root, Path::new(""), MAX_CANDIDATE_FILES) else {
        return Vec::new();
    };

    let mut scored = files
        .into_iter()
        .filter_map(|path| {
            let candidate = path.to_string_lossy().replace('\\', "/");
            if candidate.chars().count() > MAX_CANDIDATE_CHARS {
                return None;
            }
            let parts = candidate
                .split('/')
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>();
            let basename = parts.last()?;
            if !basename.eq_ignore_ascii_case(requested_basename) {
                return None;
            }
            let suffix = requested_parts
                .iter()
                .rev()
                .zip(parts.iter().rev())
                .take_while(|(left, right)| left.eq_ignore_ascii_case(right))
                .count();
            let extra_prefix = parts.len().saturating_sub(requested_parts.len());
            Some((suffix, extra_prefix, candidate))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.len().cmp(&right.2.len()))
            .then_with(|| left.2.cmp(&right.2))
    });
    scored
        .into_iter()
        .take(limit)
        .map(|(_, _, path)| path)
        .collect()
}

/// Test-only reference scanner: recursive literal-substring scan, skipping
/// `.git`/`target`/`node_modules`/hidden dirs and binary/large files.
#[cfg(test)]
pub(crate) fn grep_fallback(dir: &Path, needle: &str, ignore_case: bool) -> Result<String, String> {
    let needle_cmp = if ignore_case {
        needle.to_lowercase()
    } else {
        needle.to_string()
    };
    let mut hits: Vec<String> = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    'walk: while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                if search_dir_excluded(&name) {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if entry.metadata().map(|m| m.len()).unwrap_or(0) > 2 * 1024 * 1024 {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            if bytes.iter().take(8000).any(|&b| b == 0) {
                continue; // binary
            }
            let text = String::from_utf8_lossy(&bytes);
            let rel = path.strip_prefix(dir).unwrap_or(&path).display();
            for (i, line) in text.lines().enumerate() {
                let hay = if ignore_case {
                    line.to_lowercase()
                } else {
                    line.to_string()
                };
                if hay.contains(&needle_cmp) {
                    hits.push(format!("{rel}:{}:{}", i + 1, line.trim_end()));
                    if hits.len() >= GREP_MAX_LINES {
                        break 'walk;
                    }
                }
            }
        }
    }
    if hits.is_empty() {
        Ok("no matches".to_string())
    } else {
        Ok(hits.join("\n"))
    }
}

/// Glob match (`*`/`?` within a path segment, `**` across segments) of a
/// forward-slash path against a pattern. Dependency-free.
pub(crate) fn glob_match(pattern: &str, path: &str) -> bool {
    let pat: Vec<&str> = pattern.split('/').collect();
    let txt: Vec<&str> = path.split('/').collect();
    match_segments(&pat, &txt)
}
fn match_segments(pat: &[&str], txt: &[&str]) -> bool {
    // Iterative greedy match at the path-segment level (`**` == "zero or more
    // whole segments"), mirroring `wildcard` below. O(|pat|*|txt|). The old
    // recursive `for i in 0..=txt.len()` form re-explored ~C(d+k,k) failing
    // combinations for a pattern with k `**` segments against a depth-d path, so
    // a single model-supplied glob like `**/**/.../nope.rs` (with no timeout on
    // this in-process CPU) could wedge the agent turn for minutes-to-hours.
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut star_ti): (Option<usize>, usize) = (None, 0);
    while ti < txt.len() {
        if pi < pat.len() && pat[pi] != "**" && seg_match(pat[pi], txt[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < pat.len() && pat[pi] == "**" {
            star = Some(pi);
            star_ti = ti;
            pi += 1;
        } else if let Some(sp) = star {
            // Backtrack: let the last `**` swallow one more segment.
            pi = sp + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }
    // Trailing `**` segments match the empty remainder.
    while pi < pat.len() && pat[pi] == "**" {
        pi += 1;
    }
    pi == pat.len()
}
fn seg_match(pat: &str, txt: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    let t: Vec<char> = txt.chars().collect();
    wildcard(&p, &t)
}
fn wildcard(p: &[char], t: &[char]) -> bool {
    // Iterative greedy two-pointer glob: O(|p|*|t|), no recursion. The old
    // branch-recursion form (`wildcard(&p[1..], t) || wildcard(p, &t[1..])`)
    // backtracked exponentially on `*`-heavy segments like `*a*a*...*b`.
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut star_ti): (Option<usize>, usize) = (None, 0);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            star_ti = ti;
            pi += 1;
        } else if let Some(sp) = star {
            pi = sp + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod glob_stress {
    use super::*;

    #[test]
    fn pathological_double_star_pattern_terminates_and_is_correct() {
        // 24 `**` segments + a non-matching literal against a deep path: the old
        // recursive matcher explored ~C(depth+24,24) failing combinations and
        // could wedge the turn for minutes. The iterative matcher is O(n*m) and
        // returns immediately. (If this test ever hangs, the blow-up is back.)
        let pattern = format!("{}nope.rs", "**/".repeat(24));
        assert!(!glob_match(&pattern, "a/b/c/d/e/f/g/h/i/j/k/l/m/n/o/p.rs"));

        // Ordinary glob semantics preserved.
        assert!(glob_match("**/*.rs", "src/a/b/main.rs"));
        assert!(glob_match("src/*.toml", "src/Cargo.toml"));
        assert!(!glob_match("src/*.toml", "src/a/Cargo.toml"));
        assert!(glob_match("a/**/z", "a/z"));
        assert!(glob_match("a/**/z", "a/b/c/z"));
        assert!(glob_match("a?c", "abc"));
        assert!(!glob_match("a?c", "ac"));
    }

    #[test]
    fn pathological_star_segment_terminates() {
        // `a*a*...*b` against a long run of `a` with no trailing `b`: the old
        // `*` branch-recursion was exponential; the two-pointer form is linear.
        let pattern = format!("{}b", "a*".repeat(30));
        let text = "a".repeat(60);
        assert!(!glob_match(&pattern, &text));
    }
}

pub(crate) struct FindFilesTool {
    pub(crate) root: PathBuf,
}
impl Tool for FindFilesTool {
    fn name(&self) -> &str {
        "find_files"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "find_files".to_string(),
            description: "List workspace files matching a glob (`*`/`?` within a segment, `**` \
                          across dirs), e.g. '**/*.rs' or 'src/*.toml'. Skips \
                          hidden and ignored paths by default. Complements grep (by name vs by content)."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "no_ignore": { "type": "boolean", "description": "include ignored/generated paths (default false); credential and quarantine exclusions still apply" },
                    "hidden": { "type": "boolean", "description": "include hidden paths (default false); .git and credential files remain excluded" }, "pattern": { "type": "string", "description": "glob pattern, matched against workspace-relative paths" } },
                "required": ["pattern"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let pattern = args["pattern"].as_str().ok_or("missing 'pattern'")?;
        if pattern.is_empty() {
            return Err("'pattern' must not be empty".to_string());
        }
        let mut hits: Vec<String> = Vec::new();
        for path in search_workspace_files_with_options(
            &self.root,
            Path::new(""),
            50_000,
            SearchOptions::from_args(args)?,
        )? {
            let rel_str = path.to_string_lossy().replace('\\', "/");
            if glob_match(pattern, &rel_str) {
                hits.push(rel_str);
                if hits.len() >= 500 {
                    break;
                }
            }
        }
        hits.sort();
        if hits.is_empty() {
            Ok(format!("no files match {pattern:?}"))
        } else {
            Ok(hits.join("\n"))
        }
    }
}

// ---------------------------------------------------------------------------
// file_search — fuzzy filename finder (the Codex `file-search` capability). Codex
// uses the `nucleo` crate; we keep the cockpit dep-free with a compact fzf-style
// subsequence scorer. Complements find_files (glob) and grep (content): you type
// a fragment of a path and get the best-matching files, ranked.
// ---------------------------------------------------------------------------

/// fzf-style fuzzy score of `query` against `path`, or `None` if `query` isn't a
/// (case-insensitive) subsequence of `path`. Higher is better. Rewards matches
/// at segment/word boundaries and camel humps, consecutive runs, and basename
/// hits; penalizes gaps and long paths.
pub(crate) fn fuzzy_score(query: &str, path: &str) -> Option<i32> {
    let qstr: String = query
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let q: Vec<char> = qstr.chars().collect();
    if q.is_empty() {
        return Some(0);
    }
    let chars: Vec<char> = path.chars().collect();
    let lchars: Vec<char> = path.to_lowercase().chars().collect();

    let mut score = 0i32;
    let mut qi = 0usize;
    let mut prev: Option<usize> = None;
    for i in 0..lchars.len() {
        if qi >= q.len() {
            break;
        }
        if lchars[i] == q[qi] {
            let prev_sep = i == 0 || matches!(lchars[i - 1], '/' | '_' | '-' | '.' | ' ');
            let camel = i > 0 && chars[i].is_uppercase() && !chars[i - 1].is_uppercase();
            if prev_sep || camel {
                score += 12;
            }
            if let Some(p) = prev {
                if p + 1 == i {
                    score += 8; // consecutive
                } else {
                    score -= ((i - p - 1) as i32).min(6); // gap penalty
                }
            }
            score += 1;
            prev = Some(i);
            qi += 1;
        }
    }
    if qi < q.len() {
        return None; // not a subsequence
    }

    // Basename emphasis: typing part of a filename should rank that file high.
    let pl = path.to_lowercase();
    let base = pl.rsplit('/').next().unwrap_or(&pl);
    if base.starts_with(&qstr) {
        score += 100;
    } else if base.contains(&qstr) {
        score += 50;
    }
    // Brevity: prefer the more concise of equally-good matches.
    score -= (lchars.len() as i32) / 16;
    Some(score)
}

/// Rank `paths` by `fuzzy_score(query, …)`, returning the top `limit`. Ties break
/// by shorter path, then lexicographically — fully deterministic.
pub(crate) fn rank_paths(query: &str, paths: &[String], limit: usize) -> Vec<String> {
    let mut scored: Vec<(i32, &str)> = paths
        .iter()
        .filter_map(|p| fuzzy_score(query, p).map(|s| (s, p.as_str())))
        .collect();
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.len().cmp(&b.1.len()))
            .then_with(|| a.1.cmp(b.1))
    });
    scored
        .into_iter()
        .take(limit)
        .map(|(_, p)| p.to_string())
        .collect()
}

pub(crate) struct FileSearchTool {
    pub(crate) root: PathBuf,
}
impl Tool for FileSearchTool {
    fn name(&self) -> &str {
        "file_search"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "file_search".to_string(),
            description: "Fuzzy-find files by name: type a fragment of a path (fzf-style \
                          subsequence ranking) and get the best matches, best first. Skips \
                          hidden and ignored paths by default. Use when you half-remember a filename; \
                          complements find_files (exact glob) and grep (by content)."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "no_ignore": { "type": "boolean", "description": "include ignored/generated paths (default false); credential and quarantine exclusions still apply" },
                    "hidden": { "type": "boolean", "description": "include hidden paths (default false); .git and credential files remain excluded" },
                    "query": { "type": "string", "description": "path fragment to fuzzy-match" },
                    "limit": { "type": "integer", "description": "max results (default 20)" },
                },
                "required": ["query"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let query = args["query"].as_str().ok_or("missing 'query'")?;
        if query.trim().is_empty() {
            return Err("'query' must not be empty".to_string());
        }
        confine_nav_target(&self.root, query)?;
        let limit = args["limit"].as_u64().unwrap_or(20).clamp(1, 200) as usize;

        // Walk the workspace, mirroring find_files' skip rules; bound exploration.
        let paths: Vec<String> = search_workspace_files_with_options(
            &self.root,
            Path::new(""),
            50_000,
            SearchOptions::from_args(args)?,
        )?
        .into_iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect();

        let ranked = rank_paths(query, &paths, limit);
        if ranked.is_empty() {
            Ok(format!("no files fuzzy-match {query:?}"))
        } else {
            Ok(ranked.join("\n"))
        }
    }
}

/// One workspace scan for several definition names. `repo_recon` uses this to
/// avoid rereading the same repository once per task term; direct callers can
/// likewise resolve a small concept set in one tool round-trip.
pub(crate) fn defs_regex_many(names: &[&str]) -> String {
    let alternatives = names
        .iter()
        .map(|name| regex::escape(name))
        .collect::<Vec<_>>()
        .join("|");
    let name = format!("(?:{alternatives})");
    format!(
        r"\b(fn|struct|enum|trait|type|const|static|class|def|function|func|interface|mod)\s+{name}\b|\bimpl\b[^\n]*\b{name}\b|\bmacro_rules!\s+{name}\b"
    )
}

#[cfg(test)]
pub(crate) fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `word` appears in `hay` bounded by non-identifier chars (whole-word).
#[cfg(test)]
pub(crate) fn word_present(hay: &str, word: &str) -> bool {
    let bytes = hay.as_bytes();
    for (i, _) in hay.match_indices(word) {
        let before_ok = i == 0 || !is_ident_char(bytes[i - 1] as char);
        let after = i + word.len();
        let after_ok = after >= bytes.len() || !is_ident_char(bytes[after] as char);
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

/// Dependency-free heuristic (ripgrep fallback): a line mentioning `name` as a
/// word AND carrying a definition keyword. Looser than the regex.
#[cfg(test)]
pub(crate) fn line_defines(line: &str, name: &str) -> bool {
    if !word_present(line, name) {
        return false;
    }
    const KW: &[&str] = &[
        "fn ",
        "struct ",
        "enum ",
        "trait ",
        "type ",
        "const ",
        "static ",
        "class ",
        "def ",
        "function ",
        "impl ",
        "macro_rules!",
    ];
    KW.iter().any(|kw| line.contains(kw))
}

pub(crate) struct DefsTool {
    pub(crate) root: PathBuf,
}

const DEFS_MAX_NAMES: usize = 8;

fn valid_definition_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | ':' | '$'))
}

fn definition_names(args: &Value) -> Result<Vec<String>, String> {
    let single = args
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty());
    let batch = args.get("names").filter(|value| match value {
        Value::Null => false,
        Value::Array(names) => !names.is_empty(),
        _ => true,
    });
    if single.is_some() == batch.is_some() {
        return Err("provide exactly one of 'name' or 'names'".to_string());
    }
    let raw = if let Some(name) = single {
        vec![name]
    } else {
        let names = batch
            .and_then(Value::as_array)
            .ok_or("'names' must be an array of symbol identifiers")?;
        if names.is_empty() || names.len() > DEFS_MAX_NAMES {
            return Err(format!(
                "'names' must contain 1 through {DEFS_MAX_NAMES} symbol identifiers"
            ));
        }
        names
            .iter()
            .map(|name| {
                name.as_str()
                    .ok_or_else(|| "every 'names' item must be a string".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    let mut names = Vec::with_capacity(raw.len());
    for name in raw {
        let name = name.trim();
        if !valid_definition_name(name) {
            return Err(
                "symbol identifiers may contain only letters, digits, _, :, or $".to_string(),
            );
        }
        if !names.iter().any(|known| known == name) {
            names.push(name.to_string());
        }
    }
    Ok(names)
}

impl Tool for DefsTool {
    fn name(&self) -> &str {
        "defs"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "defs".to_string(),
            description: "Find where one symbol, or up to eight symbols, are DEFINED across the \
                          workspace in one scan (fn/struct/enum/trait/type/const/static/impl, plus \
                          Python, JS, and Go declarations). Default discovery uses Git ignore \
                          rules when available. Both modes skip hidden, credential, key, and \
                          quarantined paths; explicit source mode also rejects symlinks. Returns \
                          file:line by default. Set include_source with 1-8 discovered file paths to \
                          collect bounded definition and reference windows with file hashes in one \
                          read per file. Matches are lexical heuristics, not a semantic call graph. \
                          Provide exactly one of name/names."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "optional workspace file or directory to scope; outside targets are refused"
                    },
                    "name": { "type": "string", "description": "one symbol name to locate" },
                    "names": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": DEFS_MAX_NAMES,
                        "items": { "type": "string" },
                        "description": "1-8 symbol names to resolve in one workspace scan"
                    },
                    "ignore_case": {
                        "type": "boolean",
                        "description": "case-insensitive symbol matching (default false)"
                    },
                    "include_source": {
                        "type": "boolean",
                        "description": "return JSON source windows from explicit paths (default false)"
                    },
                    "paths": {
                        "type": "array", "minItems": 1, "maxItems": 8,
                        "items": { "type": "string", "maxLength": 1024 },
                        "description": "discovered source files for include_source; no directories"
                    },
                    "max_source_bytes": {
                        "type": "integer", "minimum": 1024, "maximum": 65536,
                        "description": "total returned source text bytes, excluding JSON metadata (default 16384)"
                    }
                },
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let names = definition_names(args)?;
        let include_source = match args.get("include_source") {
            None => false,
            Some(value) => value.as_bool().ok_or("include_source must be boolean")?,
        };
        if include_source {
            return definition_bundle::collect(&self.root, args, &names);
        }
        if args.get("paths").is_some() || args.get("max_source_bytes").is_some() {
            return Err("paths/max_source_bytes require include_source: true".to_string());
        }
        let name_refs = names.iter().map(String::as_str).collect::<Vec<_>>();
        let start = match args.get("path").and_then(Value::as_str) {
            Some(path) if !path.trim().is_empty() => confine_nav_target(&self.root, path)?,
            _ => PathBuf::from(""),
        };
        grep_confined(
            &self.root,
            &start,
            &defs_regex_many(&name_refs),
            args["ignore_case"].as_bool().unwrap_or(false),
            true,
            0,
        )
    }
}

#[cfg(test)]
mod search_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    pub(super) struct TestWorkspace(pub(super) PathBuf);

    impl TestWorkspace {
        pub(super) fn new(label: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "angel-nav-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            Self(root)
        }

        pub(super) fn write(&self, relative: &str, content: &str) {
            let path = self.0.join(relative);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(path, content).unwrap();
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn planted_cohort_tokens_are_found_in_an_ignored_nested_workspace() {
        let _guard = crate::tests::env_lock();
        let workspace = TestWorkspace::new("cohort-parent");
        workspace.write(".gitignore", "work/\n");
        assert!(
            std::process::Command::new("git")
                .args(["init", "--quiet"])
                .arg(&workspace.0)
                .status()
                .unwrap()
                .success()
        );
        let fixtures = workspace.0.join("work");
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/repo-search-cohort.py");
        assert!(
            std::process::Command::new("python3")
                .arg(script)
                .args(["--fixtures-only", "--fixture-dir"])
                .arg(&fixtures)
                .status()
                .unwrap()
                .success()
        );
        let manifest: Value =
            serde_json::from_slice(&std::fs::read(fixtures.join("manifest.json")).unwrap())
                .unwrap();
        assert!(manifest["large"]["file_count"].as_u64().unwrap() >= 5000);
        // Reproduce the old empty Git name source independently of the tool.
        assert_eq!(
            run_git(&fixtures.join("large"), &git_listing_args(None)).unwrap(),
            ""
        );
        let mut visible = 0;
        let mut ignored = 0;
        for pack in manifest.as_object().unwrap().values() {
            let root = PathBuf::from(pack["root"].as_str().unwrap());
            let grep = GrepTool { root: root.clone() };
            for target in pack["targets"].as_array().unwrap() {
                let path = target["path"].as_str().unwrap();
                let pattern = regex::escape(target["identifier"].as_str().unwrap());
                if target["escaping"].as_bool().unwrap() {
                    for no_ignore in [false, true] {
                        assert!(
                            grep.call(&serde_json::json!({"pattern": pattern, "path": path,
                            "no_ignore": no_ignore, "hidden": true}))
                                .is_err()
                        );
                    }
                    continue;
                }
                let output = grep.call(&serde_json::json!({"pattern": pattern})).unwrap();
                if target["ignored"].as_bool().unwrap() {
                    assert_eq!(output, "no matches", "{path}");
                    let output = grep
                        .call(&serde_json::json!({"pattern": pattern, "no_ignore": true}))
                        .unwrap();
                    assert!(grep_hits(&output).contains(path), "{path}: {output}");
                    let ff = FindFilesTool { root: root.clone() };
                    let fs = FileSearchTool { root: root.clone() };
                    let ld = ListDirTool { root: root.clone() };
                    assert!(
                        ff.call(&serde_json::json!({"pattern": path}))
                            .unwrap()
                            .starts_with("no files")
                    );
                    assert_eq!(
                        ff.call(&serde_json::json!({"pattern": path, "no_ignore": true}))
                            .unwrap(),
                        path
                    );
                    let name = Path::new(path).file_name().unwrap().to_str().unwrap();
                    assert!(
                        fs.call(&serde_json::json!({"query": name}))
                            .unwrap()
                            .starts_with("no files")
                    );
                    assert!(
                        fs.call(&serde_json::json!({"query": name, "no_ignore": true}))
                            .unwrap()
                            .contains(path)
                    );
                    let parent = Path::new(path).parent().unwrap().to_str().unwrap();
                    assert!(ld.call(&serde_json::json!({"path": parent})).is_err());
                    assert!(
                        ld.call(&serde_json::json!({"path": parent, "no_ignore": true}))
                            .unwrap()
                            .contains(name)
                    );
                    ignored += 1;
                } else {
                    assert!(
                        output
                            .lines()
                            .take(5)
                            .any(|line| line.starts_with(&format!("{path}:"))),
                        "{path}: {output}"
                    );
                    visible += 1;
                }
            }
        }
        // Greater than the cohort's two directory levels, with 5,012 peers.
        let root = fixtures.join("large");
        std::fs::create_dir_all(root.join("deep/three/levels")).unwrap();
        std::fs::write(
            root.join("deep/three/levels/token.txt"),
            "literal[a].b+ token\n",
        )
        .unwrap();
        let tool = GrepTool { root };
        let output = tool
            .call(&serde_json::json!({"pattern": r"literal\[a\]\.b\+"}))
            .unwrap();
        assert!(output.contains("deep/three/levels/token.txt:1:"));
        assert_eq!(
            tool.call(&serde_json::json!({"pattern": "literal[a].b+"}))
                .unwrap(),
            "no matches"
        );
        assert!(tool.call(&serde_json::json!({"pattern": "["})).is_err());
        assert_eq!((visible, ignored), (63, 20));
        println!(
            "direct cohort: visible grep recall@5 {visible}/{visible}; ignored default 0/{ignored}; ignored no_ignore {ignored}/{ignored}; escaping scope refused in both modes"
        );
    }

    #[test]
    fn discovery_flags_are_independent_and_keep_sensitive_paths_excluded() {
        let _guard = crate::tests::env_lock();
        let workspace = TestWorkspace::new("options");
        for path in [
            "visible.txt",
            ".hidden/token.txt",
            "build/token.txt",
            "node_modules/token.txt",
            ".cache/token.txt",
            ".env",
            ".env.local",
            "credentials.json",
            "private.key",
        ] {
            workspace.write(path, "needle\n");
        }
        workspace.write(".gitignore", "build/\n.cache/\n*.log\n");
        workspace.write("visible.log", "needle\n");
        for hidden in [false, true] {
            for no_ignore in [false, true] {
                let args = serde_json::json!({"pattern": "needle", "hidden": hidden, "no_ignore": no_ignore});
                let output = GrepTool {
                    root: workspace.0.clone(),
                }
                .call(&args)
                .unwrap();
                let hits = grep_hits(&output);
                assert!(hits.contains("visible.txt"));
                assert_eq!(hits.contains(".hidden/token.txt"), hidden);
                assert_eq!(hits.contains(".cache/token.txt"), hidden && no_ignore);
                assert_eq!(hits.contains("build/token.txt"), no_ignore);
                assert_eq!(hits.contains("node_modules/token.txt"), no_ignore);
                assert_eq!(hits.contains("visible.log"), no_ignore);
                for path in [".env", ".env.local", "credentials.json", "private.key"] {
                    assert!(!hits.contains(path));
                }
                let ff = FindFilesTool {
                    root: workspace.0.clone(),
                };
                let fs = FileSearchTool {
                    root: workspace.0.clone(),
                };
                let ld = ListDirTool {
                    root: workspace.0.clone(),
                };
                for (path, expected) in [
                    (".hidden/token.txt", hidden),
                    (".cache/token.txt", hidden && no_ignore),
                    ("build/token.txt", no_ignore),
                    ("node_modules/token.txt", no_ignore),
                ] {
                    let query = serde_json::json!({"pattern": path, "query": path, "hidden": hidden, "no_ignore": no_ignore});
                    assert_eq!(
                        ff.call(&query).unwrap().contains(path)
                            && !ff.call(&query).unwrap().starts_with("no files"),
                        expected
                    );
                    assert_eq!(
                        fs.call(&query).unwrap().lines().any(|line| line == path),
                        expected
                    );
                }
                let listing = ld
                    .call(&serde_json::json!({"hidden": hidden, "no_ignore": no_ignore}))
                    .unwrap();
                assert_eq!(listing.lines().any(|line| line == "build/"), no_ignore);
                assert_eq!(listing.lines().any(|line| line == ".hidden/"), hidden);
                for tool in [&ff as &dyn Tool, &fs, &ld] {
                    assert_eq!(
                        tool.def().params["properties"]["no_ignore"]["type"],
                        "boolean"
                    );
                    assert_eq!(tool.def().params["properties"]["hidden"]["type"], "boolean");
                }
            }
        }
        let grep = GrepTool {
            root: workspace.0.clone(),
        };
        assert_eq!(grep.def().params["properties"]["hidden"]["type"], "boolean");
        assert!(
            grep.call(&serde_json::json!({"pattern":"needle", "hidden":"true"}))
                .is_err()
        );
        assert!(
            grep.call(&serde_json::json!({"pattern":"needle", "no_ignore":1}))
                .is_err()
        );
    }

    fn grep_hits(output: &str) -> BTreeSet<String> {
        output
            .lines()
            .filter(|line| !line.starts_with('[') && *line != "--" && *line != "no matches")
            .filter_map(|line| line.split(':').next().map(str::to_string))
            .collect()
    }

    #[test]
    fn git_listing_parser_rejects_capped_or_partial_output() {
        assert_eq!(
            parse_git_file_listing("src/a.rs\0src/b.rs\0").unwrap(),
            vec![PathBuf::from("src/a.rs"), PathBuf::from("src/b.rs")]
        );
        assert!(parse_git_file_listing("src/a.rs").is_none());
        assert!(parse_git_file_listing(&"x".repeat(GIT_OUTPUT_CAP_BYTES)).is_none());
    }

    #[test]
    fn git_listing_pathspec_stays_narrow_when_search_is_scoped() {
        assert_eq!(
            git_listing_args(Some("src/harness")),
            vec![
                "ls-files",
                "--cached",
                "--others",
                "--exclude-standard",
                "-z",
                "--",
                "src/harness",
            ]
        );
        assert_eq!(
            git_listing_args(None),
            vec![
                "ls-files",
                "--cached",
                "--others",
                "--exclude-standard",
                "-z"
            ]
        );
    }

    #[test]
    fn repository_search_policy_excludes_quarantined_directories() {
        assert!(!search_path_allowed(Path::new(
            "off-limits/legacy-product/source.rs"
        )));
        assert!(!search_path_allowed(Path::new("src/target/generated.rs")));
        assert!(!search_path_allowed(Path::new("src/.private/secret.rs")));
        assert!(search_path_allowed(Path::new("src/harness/turn.rs")));
    }

    #[test]
    fn grep_diversity_keeps_a_hot_first_file_from_hiding_later_files() {
        let workspace = TestWorkspace::new("diverse");
        workspace.write("a-hot.txt", &"needle\n".repeat(1000));
        workspace.write("z-unique.txt", "one unique needle\n");

        let output = grep_confined_many(
            &workspace.0,
            &[PathBuf::from(".")],
            &BTreeSet::new(),
            None,
            "needle",
            false,
            true,
            0,
        )
        .unwrap();

        assert!(output.contains("z-unique.txt:1:one unique needle"));
        assert_eq!(
            output
                .lines()
                .filter(|line| line.starts_with("a-hot.txt:"))
                .count(),
            GREP_MAX_MATCHES_PER_FILE
        );
        assert!(output.contains("per-file caps omitted"));
        assert!(output.contains("a-hot.txt"));
    }

    #[test]
    fn grep_continuation_is_deterministic_and_has_no_duplicate_files() {
        let workspace = TestWorkspace::new("paging");
        for index in 0..30 {
            workspace.write(
                &format!("file-{index:02}.txt"),
                &format!("needle {index}\n").repeat(10),
            );
        }
        let first = grep_confined_many(
            &workspace.0,
            &[PathBuf::from(".")],
            &BTreeSet::new(),
            None,
            "needle",
            false,
            true,
            0,
        )
        .unwrap();
        assert!(first.contains("grep continuation"));
        let first_files = grep_hits(&first);
        let cursor = first_files.iter().next_back().unwrap().clone();

        let second = grep_confined_many(
            &workspace.0,
            &[PathBuf::from(".")],
            &BTreeSet::new(),
            Some(Path::new(&cursor)),
            "needle",
            false,
            true,
            0,
        )
        .unwrap();
        let second_repeat = grep_confined_many(
            &workspace.0,
            &[PathBuf::from(".")],
            &BTreeSet::new(),
            Some(Path::new(&cursor)),
            "needle",
            false,
            true,
            0,
        )
        .unwrap();
        assert_eq!(second, second_repeat);
        let second_files = grep_hits(&second);
        assert!(first_files.is_disjoint(&second_files));
        assert_eq!(first_files.len() + second_files.len(), 30);
    }

    #[test]
    fn grep_multi_path_and_skip_inputs_stay_confined() {
        let workspace = TestWorkspace::new("multipath");
        workspace.write("one/a.txt", "needle one\n");
        workspace.write("two/b.txt", "needle two\n");
        workspace.write("three/c.txt", "needle three\n");
        let skipped = BTreeSet::from([PathBuf::from("two/b.txt")]);
        let output = grep_confined_many(
            &workspace.0,
            &[
                PathBuf::from("one"),
                PathBuf::from("two"),
                PathBuf::from("three"),
            ],
            &skipped,
            None,
            "needle",
            false,
            true,
            0,
        )
        .unwrap();
        assert!(output.contains("one/a.txt:1:needle one"));
        assert!(!output.contains("two/b.txt:1:"));
        assert!(output.contains("three/c.txt:1:needle three"));
        assert!(output.contains("skipped 1 requested file"));
        assert!(output.contains("two/b.txt"));

        // Both `path` and `paths` supplied: one merged, deduplicated search.
        let merged = GrepTool {
            root: workspace.0.clone(),
        }
        .call(&serde_json::json!({
            "pattern": "needle",
            "path": "one",
            "paths": ["one", "three"],
            "context": 0,
        }))
        .unwrap();
        assert!(merged.contains("one/a.txt:1:needle one"), "{merged}");
        assert!(merged.contains("three/c.txt:1:needle three"), "{merged}");
        assert!(!merged.contains("two/b.txt"), "{merged}");

        assert!(
            grep_confined_many(
                &workspace.0,
                &[PathBuf::from("../escape")],
                &BTreeSet::new(),
                None,
                "needle",
                false,
                true,
                0,
            )
            .is_err()
        );
        assert!(
            grep_skip_files(
                &workspace.0,
                Some(&serde_json::json!(["../escape"])),
                SearchOptions::default()
            )
            .is_err()
        );
        assert_eq!(
            grep_start_paths(&serde_json::json!({"path":"one", "paths":["two"]})).unwrap(),
            vec![PathBuf::from("one"), PathBuf::from("two")]
        );
        assert_eq!(
            grep_start_paths(&serde_json::json!({"path":"one", "paths":["one", "two", "one"]}))
                .unwrap(),
            vec![PathBuf::from("one"), PathBuf::from("two")]
        );
        assert!(
            grep_start_paths(&serde_json::json!({
                "path": "one",
                "paths": ["a", "b", "c", "d", "e", "f", "g", "h"]
            }))
            .is_err()
        );
        assert_eq!(
            grep_start_paths(&serde_json::json!({"path":"", "paths":["two"]})).unwrap(),
            vec![PathBuf::from("two")]
        );
        assert_eq!(
            grep_start_paths(&serde_json::json!({"path":"one", "paths":[]})).unwrap(),
            vec![PathBuf::from("one")]
        );
        assert_eq!(
            grep_start_paths(&serde_json::json!({"path":null, "paths":["two"]})).unwrap(),
            vec![PathBuf::from("two")]
        );
        assert_eq!(
            grep_start_paths(&serde_json::json!({"path":"", "paths":[]})).unwrap(),
            vec![PathBuf::from(".")]
        );
    }

    #[test]
    fn grep_exact_empty_continuation_placeholder_is_absent() {
        let workspace = TestWorkspace::new("empty-continuation");
        workspace.write("src/example.rs", "needle\n");
        let tool = GrepTool {
            root: workspace.0.clone(),
        };

        for after_file in [Value::Null, Value::String(String::new())] {
            let output = tool
                .call(&serde_json::json!({
                    "pattern": "needle",
                    "after_file": after_file,
                    "context": 0,
                }))
                .unwrap();
            assert!(output.contains("src/example.rs:1:needle"));
        }

        assert!(
            tool.call(&serde_json::json!({"pattern":"needle", "after_file":" "}))
                .is_err()
        );
        assert!(
            tool.call(&serde_json::json!({"pattern":"needle", "after_file":[]}))
                .is_err()
        );
    }

    #[test]
    fn grep_output_is_line_byte_and_utf8_bounded() {
        let workspace = TestWorkspace::new("bounded");
        let long_match = format!("needle {}\n", "λ".repeat(1800));
        for index in 0..12 {
            workspace.write(
                &format!("long-{index:02}.txt"),
                &long_match.repeat(GREP_MAX_MATCHES_PER_FILE),
            );
        }
        let output = grep_confined_many(
            &workspace.0,
            &[PathBuf::from(".")],
            &BTreeSet::new(),
            None,
            "needle",
            false,
            true,
            0,
        )
        .unwrap();
        assert!(output.len() <= GREP_MAX_OUTPUT_BYTES);
        assert!(output.lines().count() <= GREP_MAX_LINES);
        assert!(output.contains("line truncated"));
        assert!(output.contains("grep continuation"));
        assert!(std::str::from_utf8(output.as_bytes()).is_ok());
    }

    #[test]
    fn definition_batch_builds_one_regex_and_validates_arguments() {
        let regex = regex::RegexBuilder::new(&defs_regex_many(&["NeedleWidget", "run"]))
            .case_insensitive(true)
            .build()
            .unwrap();
        assert!(regex.is_match("pub struct NeedleWidget {"));
        assert!(regex.is_match("func run() {"));
        assert!(!regex.is_match("let run = widget.run();"));

        assert_eq!(
            definition_names(&serde_json::json!({"names":["NeedleWidget", "run", "run"]})).unwrap(),
            vec!["NeedleWidget", "run"]
        );
        assert!(definition_names(&serde_json::json!({"name":"run", "names":["run"]})).is_err());
        assert_eq!(
            definition_names(&serde_json::json!({"name":"", "names":["run"]})).unwrap(),
            vec!["run"]
        );
        assert_eq!(
            definition_names(&serde_json::json!({"name":"run", "names":[]})).unwrap(),
            vec!["run"]
        );
        assert!(definition_names(&serde_json::json!({"names":[]})).is_err());
        assert!(definition_names(&serde_json::json!({"name":"run-now"})).is_err());
    }

    #[test]
    fn nav_tools_refuse_outside_escape_nul_and_overlong_targets() {
        let ws = TestWorkspace::new("confine");
        ws.write("src/lib.rs", "pub fn main() {}\n");
        let root = ws.0.clone();
        let over = "x".repeat(4000) + ".txt";
        let nul = "bad\0name.txt";
        let file_search = FileSearchTool { root: root.clone() };
        let defs = DefsTool { root: root.clone() };
        let outline = OutlineTool { root: root.clone() };
        let grep = GrepTool { root: root.clone() };
        for (tool, args) in [
            (
                "file_search",
                file_search.call(&serde_json::json!({"query": "/etc/passwd"})),
            ),
            (
                "file_search_dotdot",
                file_search.call(&serde_json::json!({"query": "../outside/secret.txt"})),
            ),
            (
                "file_search_nul",
                file_search.call(&serde_json::json!({"query": nul})),
            ),
            (
                "file_search_overlong",
                file_search.call(&serde_json::json!({"query": over})),
            ),
            (
                "defs",
                defs.call(&serde_json::json!({"name": "main", "path": "../outside/secret.txt"})),
            ),
            (
                "outline",
                outline.call(&serde_json::json!({"path": "/etc/passwd"})),
            ),
            (
                "grep",
                grep.call(&serde_json::json!({"pattern": "main", "path": "../outside/secret.txt"})),
            ),
        ] {
            let err = args.expect_err(tool);
            assert!(
                err.contains("outside the workspace") || err.contains("escapes the workspace"),
                "{tool}: {err}"
            );
        }
        let scoped = defs
            .call(&serde_json::json!({"name": "main", "path": "src/lib.rs"}))
            .unwrap();
        assert!(scoped.contains("src/lib.rs"), "{scoped}");
        let listed = file_search
            .call(&serde_json::json!({"query": "lib.rs"}))
            .unwrap();
        assert!(listed.contains("src/lib.rs"), "{listed}");
    }
}
