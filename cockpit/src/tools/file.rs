//! File-editing tools confined to a workspace root: `read_file`, `write_file`,
//! `str_replace`, `multi_edit` (atomic batch), and `apply_patch` (unified-diff
//! or freeform). The patch machinery (freeform-hunk parser/applier, unified-diff
//! targeting) lives here too. Direct operations use descriptor-anchored
//! confinement; unified-diff subprocesses are forced into a write-confined
//! Landlock domain on Linux.

use crate::club::ToolDef;
use crate::harness::{
    Tool, confined_create_new, confined_edit, confined_read, confined_read_limited,
    confined_read_page, confined_remove_file, confined_write, env_flag, output_timed, safe_path,
    tool_timeout,
};
use crate::tools::nav::suggest_workspace_paths;
use serde_json::Value;
use std::collections::BTreeSet;
#[cfg(target_os = "linux")]
use std::ffi::OsString;
use std::path::{Path, PathBuf};
#[cfg(not(target_os = "linux"))]
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

/// Keep an in-process multi-file commit (including error rollback) alive while
/// the signal reaper performs normal process exit. `atexit` runs outside signal
/// context, and other Rust threads continue running while this callback waits.
/// SIGKILL/power loss are deliberately outside this guarantee.
#[cfg(target_os = "linux")]
mod exit_safe_patch {
    use std::sync::{Condvar, Mutex, OnceLock};

    static STATE: Mutex<(usize, bool)> = Mutex::new((0, false));
    static IDLE: Condvar = Condvar::new();
    static REGISTERED: OnceLock<Result<(), String>> = OnceLock::new();

    extern "C" fn drain() {
        let mut state = STATE.lock().unwrap_or_else(|error| error.into_inner());
        state.1 = true;
        while state.0 != 0 {
            state = IDLE.wait(state).unwrap_or_else(|error| error.into_inner());
        }
    }

    pub(super) struct Guard;

    impl Guard {
        pub(super) fn enter() -> Result<Self, String> {
            REGISTERED
                .get_or_init(|| {
                    // SAFETY: static C callback; no captured state or signal IO.
                    if unsafe { libc::atexit(drain) } == 0 {
                        Ok(())
                    } else {
                        Err("could not register patch exit drain; no mutation performed".into())
                    }
                })
                .clone()?;
            let mut state = STATE.lock().unwrap_or_else(|error| error.into_inner());
            if state.1 {
                return Err("process exiting before patch commit; no mutation performed".into());
            }
            state.0 += 1;
            Ok(Self)
        }
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            let mut state = STATE.lock().unwrap_or_else(|error| error.into_inner());
            state.0 -= 1;
            if state.0 == 0 {
                IDLE.notify_all();
            }
        }
    }
}

/// A page stays below the generic 64 KiB / 800-line history ceiling, so the
/// model receives one contiguous range instead of a second head/tail elision.
/// `from_utf8_lossy` can expand malformed bytes threefold, hence 20 KiB rather
/// than a superficially larger raw-byte ceiling.
const DEFAULT_READ_PAGE_LINES: usize = 200;
const MAX_READ_PAGE_LINES: usize = 400;
const MAX_READ_PAGE_BYTES: usize = 20 * 1024;
const MAX_READ_PAGE_SCAN_BYTES: usize = 2 * 1024 * 1024;
const MAX_MISSING_PATH_HINTS: usize = 5;
pub(crate) const MISSING_PATH_HINT_MARKER: &str = "Did you mean one of these workspace paths?";

pub(crate) fn with_missing_path_hints(root: &Path, path: &Path, error: String) -> String {
    let missing = error.contains("No such file or directory")
        || error.contains("The system cannot find the file specified");
    if !missing || !env_flag("ANGEL_MISSING_PATH_HINTS", true) {
        return error;
    }
    let suggestions = suggest_workspace_paths(root, path, MAX_MISSING_PATH_HINTS);
    if suggestions.is_empty() {
        return error;
    }
    format!(
        "{error}\n{MISSING_PATH_HINT_MARKER}\n{}",
        suggestions
            .into_iter()
            .map(|candidate| format!("- {candidate}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

/// Handle virtual URL schemes that share the FS-shaped tool surface.
/// Returns `Ok(Some(text))` when `path` is a known scheme, `Ok(None)` for
/// ordinary workspace paths, and `Err` for a malformed known scheme.
fn try_read_virtual(root: &Path, path: &str) -> Result<Option<String>, String> {
    if path.starts_with("conflict:") || path.contains(":conflict://") {
        return Ok(Some(read_conflict_uri(path)?));
    }
    if let Some(rest) = path.strip_prefix("skill://") {
        return Ok(Some(read_skill_uri(root, rest)?));
    }
    if let Some(rest) = path.strip_prefix("agent://") {
        return Ok(Some(read_agent_uri(rest)?));
    }
    if crate::git::hub_url::is_github_uri(path) {
        return Ok(Some(crate::git::hub_url::resolve_github_uri(root, path)?));
    }
    if let Some(rest) = path.strip_prefix("outline://") {
        return Ok(Some(read_outline_uri(root, rest)?));
    }
    Ok(None)
}

fn try_write_virtual(root: &Path, path: &str, content: &str) -> Result<Option<String>, String> {
    if path.starts_with("conflict:") || path.contains(":conflict://") {
        return Ok(Some(write_conflict_uri(root, path, content)?));
    }
    if path.starts_with("skill://") {
        return Err(
            "skill:// is read-only; use the skill tool or edit the skill files on disk".into(),
        );
    }
    if path.starts_with("agent://") {
        return Err(
            "agent:// is read-only; handles are deposited by the harness, not written".into(),
        );
    }
    if crate::git::hub_url::is_github_uri(path) {
        return Err("pr:// and issue:// are read-only; use gh CLI or the browser to mutate".into());
    }
    if path.starts_with("outline://") {
        return Err("outline:// is read-only".into());
    }
    Ok(None)
}

/// Structural map of a source file (same heuristics as the `outline` tool),
/// addressable as `outline://path/to/file.rs` without a separate tool hop.
fn read_outline_uri(root: &Path, rest: &str) -> Result<String, String> {
    use crate::tools::nav::is_outline_decl;
    let rel = rest.trim().trim_start_matches('/');
    if rel.is_empty() {
        return Err("outline:// requires a workspace path: outline://src/main.rs".into());
    }
    if rel.starts_with("hnd_") {
        return Err(format!(
            "outline:// expects a workspace source path; `{rel}` is an opaque result handle. \
             Read it with `agent://{rel}` or call handle_read with handle `{rel}`"
        ));
    }
    let bytes = confined_read(root, Path::new(rel))?;
    let content = String::from_utf8(bytes).map_err(|e| format!("outline://{rel}: {e}"))?;
    let mut out: Vec<String> = vec![format!("[outline://{rel}]")];
    for (i, raw) in content.lines().enumerate() {
        if is_outline_decl(raw) {
            out.push(format!("{}: {}", i + 1, raw.trim_end()));
            if out.len() >= 401 {
                out.push("…[truncated at 400 symbols]".into());
                break;
            }
        }
    }
    if out.len() == 1 {
        out.push(format!("no top-level symbols found in {rel}"));
    }
    Ok(out.join("\n") + "\n")
}

/// `agent://` — FS-shaped access to session handle bodies (spawn digests,
/// aged tool bulk, code_mode offloads). Forms:
/// - `agent://` — list live handles
/// - `agent://hnd_xxx` — capped disclose of that handle
/// - `agent://hnd_xxx/path.to.field` — if body is JSON, extract via dotted path
fn read_agent_uri(rest: &str) -> Result<String, String> {
    use crate::harness::{HandleStoreLimits, session_disclose, session_list_receipts};

    let rest = rest.trim().trim_start_matches('/');
    if rest.is_empty() || rest == "*" {
        let receipts = session_list_receipts();
        if receipts.is_empty() {
            return Ok(
                "no live agent:// handles — bulk is parked under hnd_* when handle store \
                 offload runs (spawn digests, large tool results, code_mode)\n"
                    .into(),
            );
        }
        let mut out = format!("agent:// catalog ({} live handle(s)):\n", receipts.len());
        for r in receipts.iter().rev().take(64) {
            // Newest last in LRU → reverse for newest-first listing.
            out.push_str(&format!(
                "  agent://{}  kind={}  producer={}  bytes={}  lines={}\n",
                r.handle,
                r.kind.as_str(),
                r.producer,
                r.bytes,
                r.lines
            ));
            if let Some(id) = &r.identity {
                out.push_str(&format!("    identity={id}\n"));
            }
            if let Some(p) = &r.preview {
                out.push_str(&format!("    preview={p}\n"));
            }
        }
        if receipts.len() > 64 {
            out.push_str(&format!("  …(+{} more)\n", receipts.len() - 64));
        }
        out.push_str("Read agent://hnd_… for a capped body slice.\n");
        return Ok(out);
    }

    let (id, json_path) = match rest.split_once('/') {
        Some((id, path)) if !path.is_empty() => (id, Some(path)),
        _ => (rest, None),
    };
    if id.is_empty() {
        return Err("agent:// requires a handle id: agent://hnd_…".into());
    }
    let max = HandleStoreLimits::from_env().max_disclose_bytes;
    let slice = session_disclose(id, 0, max)?;
    let mut body = slice.content;
    if let Some(path) = json_path {
        body = extract_json_path(&body, path).map_err(|e| format!("agent://{id}/{path}: {e}"))?;
    }
    let mut out = format!(
        "[agent://{}] offset={} bytes={} total={}{}\n",
        id,
        slice.offset,
        slice.bytes,
        slice.total_bytes,
        if slice.truncated {
            " truncated=true"
        } else {
            ""
        }
    );
    out.push_str(&body);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    if slice.truncated && json_path.is_none() {
        out.push_str(&format!(
            "…[truncated at {max} bytes; re-call handle_read with a higher budget if enabled]\n"
        ));
    }
    Ok(out)
}

/// Minimal dotted/indexed JSON path extractor: `a.b.0.c` over a JSON value.
/// Returns pretty-printed JSON for complex values, or a bare string for scalars.
fn extract_json_path(body: &str, path: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(body.trim())
        .map_err(|e| format!("body is not JSON ({e}); use agent://id without a path"))?;
    let mut cur = &value;
    for seg in path.split('.').filter(|s| !s.is_empty()) {
        if let Ok(idx) = seg.parse::<usize>() {
            cur = cur
                .get(idx)
                .ok_or_else(|| format!("index {idx} out of range at segment {seg:?}"))?;
        } else {
            cur = cur
                .get(seg)
                .ok_or_else(|| format!("missing field {seg:?}"))?;
        }
    }
    match cur {
        serde_json::Value::String(s) => Ok(s.clone()),
        other => Ok(serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string())),
    }
}

fn read_conflict_uri(path: &str) -> Result<String, String> {
    use crate::conflict::{
        ConflictUri, format_conflict_detail, format_conflict_footer, get_conflict, list_conflicts,
        parse_conflict_uri,
    };
    match parse_conflict_uri(path)? {
        ConflictUri::List | ConflictUri::All => {
            let entries = list_conflicts();
            if entries.is_empty() {
                return Ok(
                    "no registered merge conflicts — read a conflicted file first so \
                     conflict:// ids are assigned\n"
                        .into(),
                );
            }
            Ok(format_conflict_footer(&entries) + "\n")
        }
        ConflictUri::One { id, scope } => {
            let entry = get_conflict(id).ok_or_else(|| {
                format!(
                    "unknown conflict://{id} — read a conflicted file first, or call \
                     read_file path=conflict:// to list active ids"
                )
            })?;
            Ok(format_conflict_detail(&entry, scope.as_ref()))
        }
    }
}

fn write_conflict_uri(root: &Path, path: &str, content: &str) -> Result<String, String> {
    use crate::conflict::{
        ConflictUri, get_conflict, invalidate_conflict, invalidate_conflicts_for_path,
        list_conflicts, parse_conflict_uri, resolve_replacement, splice_conflict,
    };
    match parse_conflict_uri(path)? {
        ConflictUri::List => Err(
            "conflict:// (list) is read-only; write conflict://N or conflict://* with a resolution"
                .into(),
        ),
        ConflictUri::One { id, scope: _ } => {
            let entry = get_conflict(id).ok_or_else(|| {
                format!("unknown conflict://{id}; read the conflicted file first")
            })?;
            let repl = resolve_replacement(&entry, content)?;
            let original = confined_read(root, Path::new(&entry.path))
                .map_err(|e| format!("conflict://{id}: cannot read {}: {e}", entry.path))?;
            let original_str = String::from_utf8(original)
                .map_err(|_| format!("conflict://{id}: {} is not valid UTF-8", entry.path))?;
            let updated = splice_conflict(&original_str, &entry, &repl)?;
            confined_write(root, Path::new(&entry.path), updated.as_bytes())?;
            crate::hashline::record_snapshot(&entry.path, &updated);
            invalidate_conflict(id);
            // Other conflicts in the same file may have shifted; drop them so
            // the agent re-reads for fresh ids.
            invalidate_conflicts_for_path(&entry.path);
            let tag = crate::hashline::content_tag(&updated);
            Ok(format!(
                "resolved conflict://{id} in {} · new tag #{tag}",
                entry.path
            ))
        }
        ConflictUri::All => {
            let entries = list_conflicts();
            if entries.is_empty() {
                return Err("no registered conflicts to bulk-resolve".into());
            }
            let mut receipts = Vec::new();
            // Resolve in reverse id order so earlier regions stay stable if
            // multiple land in one file (we invalidate the whole path after
            // each file write, so group by path).
            let mut by_path: std::collections::BTreeMap<String, Vec<_>> =
                std::collections::BTreeMap::new();
            for e in entries {
                by_path.entry(e.path.clone()).or_default().push(e);
            }
            for (file_path, mut file_entries) in by_path {
                file_entries.sort_by_key(|e| std::cmp::Reverse(e.block.start_line));
                let original = confined_read(root, Path::new(&file_path))
                    .map_err(|e| format!("conflict://*: cannot read {file_path}: {e}"))?;
                let mut text = String::from_utf8(original)
                    .map_err(|_| format!("conflict://*: {file_path} is not valid UTF-8"))?;
                for entry in &file_entries {
                    let repl = resolve_replacement(entry, content)?;
                    text = splice_conflict(&text, entry, &repl)?;
                    receipts.push(format!("conflict://{}", entry.id));
                }
                confined_write(root, Path::new(&file_path), text.as_bytes())?;
                crate::hashline::record_snapshot(&file_path, &text);
                invalidate_conflicts_for_path(&file_path);
            }
            Ok(format!(
                "bulk-resolved {} conflict(s): {}",
                receipts.len(),
                receipts.join(", ")
            ))
        }
    }
}

fn read_skill_uri(root: &Path, rest: &str) -> Result<String, String> {
    let name = rest
        .trim()
        .trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or("")
        .trim();
    if name.is_empty() {
        // skill:// → catalog listing
        let skills = crate::harness::load_skills_for(root);
        if skills.is_empty() {
            return Ok("no skills available for this workspace\n".into());
        }
        let mut out = String::from("skill:// catalog (read skill://NAME for full body):\n");
        for s in skills {
            out.push_str(&format!("  skill://{} — {}\n", s.name, s.description));
        }
        return Ok(out);
    }
    let skills = crate::harness::load_skills_for(root);
    let skill = skills
        .into_iter()
        .find(|s| s.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| {
            let available: Vec<_> = crate::harness::load_skills_for(root)
                .into_iter()
                .map(|s| s.name)
                .collect();
            format!(
                "unknown skill://{name}; available: {}",
                if available.is_empty() {
                    "none".into()
                } else {
                    available.join(", ")
                }
            )
        })?;
    Ok(format!(
        "[skill://{}]\n{}\n\n{}",
        skill.name,
        skill.description,
        skill.body.trim_end()
    ))
}

/// Scan a just-read page for merge conflict markers, register them, and return
/// a footer when any completed blocks were found.
fn scan_and_register_conflicts(path: &str, text: &str, first_line: usize) -> Option<String> {
    // When hashline anchors are on, `text` may be annotated with a header and
    // line-number prefixes — scan the raw page body only (lines after the
    // optional `[path#tag]` header, stripping `NN  ` prefixes when present).
    let body = strip_read_annotation_for_conflict_scan(text);
    let lines: Vec<&str> = body.lines().collect();
    if lines.is_empty() {
        return None;
    }
    let blocks = crate::conflict::scan_conflict_lines(&lines, first_line);
    if blocks.is_empty() {
        return None;
    }
    let entries = crate::conflict::register_conflicts(path, &blocks);
    Some(crate::conflict::format_conflict_footer(&entries))
}

fn strip_read_annotation_for_conflict_scan(text: &str) -> String {
    let mut lines = text.lines().peekable();
    // Drop `[path#tag]` header if present.
    if let Some(first) = lines.peek() {
        let t = first.trim();
        if t.starts_with('[') && t.contains('#') && t.ends_with(']') {
            lines.next();
        }
    }
    let mut out = String::new();
    for line in lines {
        // Hashline-annotated body: `NN  content` (right-aligned number, two spaces).
        if let Some(rest) = strip_line_number_prefix(line) {
            out.push_str(rest);
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

fn strip_line_number_prefix(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == start {
        return None;
    }
    // Require the two-space gap hashline uses: `"  "`.
    if bytes.get(i..i + 2) == Some(b"  ") {
        return Some(&line[i + 2..]);
    }
    None
}

/// When enabled (the default), `read_file` prefixes a `[path#tag]` header and
/// absolute line numbers so the model can author token-cheap hashline
/// `apply_patch` edits. Set `ANGEL_HASHLINE_ANCHORS=0` (or `false`/`off`) to
/// restore plain unnumbered pages.
fn hashline_anchors_enabled() -> bool {
    match std::env::var("ANGEL_HASHLINE_ANCHORS") {
        Ok(v) => {
            let t = v.trim();
            !(t.is_empty()
                || t == "0"
                || t.eq_ignore_ascii_case("false")
                || t.eq_ignore_ascii_case("off")
                || t.eq_ignore_ascii_case("no"))
        }
        Err(_) => true,
    }
}

/// Above this whole-file size, `read_file` serves the plain page rather than
/// re-reading the whole file to compute a hashline tag.
const HASHLINE_ANNOTATE_MAX_BYTES: usize = 512 * 1024;

/// Optional stale-edit guard shared by `str_replace`/`multi_edit`: if the caller
/// passed an `expect_tag`, reject the edit unless the live file still hashes to
/// it. Reuses the hashline content tag so read_file's `[path#tag]` header and
/// this guard speak the same language — a cheap way to fail an edit written
/// against a version the model no longer sees, rather than silently applying to
/// changed bytes.
/// The tag as the model is likely to quote it: `tag`, `#tag`, `path#tag` or the
/// literal `[path#tag]` header. Measured 2026-09-07 (glm-5.3-flash on a real
/// repo): it copied the whole `path#tag` and lost two hops to "stale edit" with
/// identical tags on both sides.
fn normalize_expect_tag(raw: &str) -> &str {
    let trimmed = raw.trim().trim_start_matches('[').trim_end_matches(']');
    trimmed.rsplit('#').next().unwrap_or(trimmed).trim()
}

pub(crate) fn guard_expected_tag(
    path: &str,
    content: &str,
    expect_tag: Option<&str>,
) -> Result<(), String> {
    if let Some(tag) = expect_tag
        .map(normalize_expect_tag)
        .filter(|t| !t.is_empty())
    {
        let live = crate::hashline::content_tag(content);
        if live != tag {
            return Err(format!(
                "stale edit for {path}: file tag is #{live} but you expected #{tag} — \
                 re-read the file and retry"
            ));
        }
    }
    Ok(())
}

pub(crate) struct ReadFileTool {
    pub(crate) root: PathBuf,
}
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }
    fn def(&self) -> ToolDef {
        let anchors = if hashline_anchors_enabled() {
            " Each page is headed with `[path#tag]` (whole-file content tag) and \
             absolute 1-based line numbers so you can author token-cheap hashline \
             `apply_patch` edits that emit only the NEW lines. A mismatched tag is \
             rejected, or recovered when a session snapshot of that tag still \
             proves the anchors map cleanly onto the live file."
        } else {
            ""
        };
        ToolDef {
            name: "read_file".to_string(),
            description: format!(
                "Read one bounded, contiguous UTF-8 source page in the workspace. \
                 `offset` is a one-based line number (default 1); `limit` is the \
                 number of complete lines (default 200, max 400). When more content \
                 remains, the result names the next offset to use. Also resolves \
                 virtual URLs: `conflict://…`, `skill://name`, `agent://hnd_…`, \
                 `outline://path` (structural symbol map), `pr://N`[/diff|files|comments], \
                 `issue://N`[/comments] (via `gh`). Merge conflict markers in ordinary \
                 files are registered and footered for write_file path=conflict://N. \
                 Use `agent://hnd_…` or handle_read for opaque handles; `outline://` \
                 always requires a workspace source path.{anchors}"
            ),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "workspace path, or conflict:// / skill:// / agent:// / outline:// / pr:// / issue:// virtual URL" },
                    "offset": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "one-based first source line; default 1. Reuse the next offset named by a truncated page"
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_READ_PAGE_LINES,
                        "description": "complete source lines to return; default 200, maximum 400"
                    }
                },
                "required": ["path"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let path = args["path"].as_str().ok_or("missing 'path'")?;
        // Virtual schemes share the read_file surface (oh-my-pi internal URLs).
        if let Some(text) = try_read_virtual(&self.root, path)? {
            return Ok(text);
        }
        let offset = match args.get("offset") {
            None => 1,
            Some(value) => value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| *value >= 1)
                .ok_or("'offset' must be a one-based positive integer")?,
        };
        let limit = match args.get("limit") {
            None => DEFAULT_READ_PAGE_LINES,
            Some(value) => value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| (1..=MAX_READ_PAGE_LINES).contains(value))
                .ok_or_else(|| {
                    format!(
                        "'limit' must be a positive integer no greater than {MAX_READ_PAGE_LINES}"
                    )
                })?,
        };
        let page = confined_read_page(
            &self.root,
            Path::new(path),
            offset,
            limit,
            MAX_READ_PAGE_BYTES,
            MAX_READ_PAGE_SCAN_BYTES,
        )
        .map_err(|error| with_missing_path_hints(&self.root, Path::new(path), error))?;
        // Binary guard (git's heuristic: a NUL in the first 8 KiB ⇒ binary). The
        // page helper probes before rendering, so a huge asset never becomes a
        // whole-file lossy decode or a context flood.
        if page.binary {
            return Ok(format!(
                "[binary file: {} bytes, not shown — read_file is for text]",
                page.total_bytes
            ));
        }
        if page.before_offset_eof || (page.bytes.is_empty() && offset > 1) {
            return Ok(format!("[end of file before line {offset}]"));
        }
        let mut text = String::from_utf8_lossy(&page.bytes).into_owned();
        // Optional hashline anchors: a whole-file `[path#tag]` header plus
        // absolute line numbers, so the model can address edits by line for a
        // token-cheap `apply_patch`. The tag is over the WHOLE file (edits are
        // whole-file addressed); a large file just serves the plain page.
        if hashline_anchors_enabled() {
            // Refuse oversized files before allocation. The limited reader
            // also caps a file that grows after its descriptor metadata check.
            if let Ok(Some(whole)) =
                confined_read_limited(&self.root, Path::new(path), HASHLINE_ANNOTATE_MAX_BYTES)
            {
                let whole_str = String::from_utf8_lossy(&whole);
                // Session snapshot so a later stale-tag hashline patch can
                // remap anchors instead of hard-rejecting.
                let tag = crate::hashline::record_snapshot(path, &whole_str);
                let width = (offset + text.lines().count()).max(1).to_string().len();
                let mut annotated = format!("[{path}#{tag}]\n");
                for (k, line) in text.lines().enumerate() {
                    annotated.push_str(&format!("{:>width$}  {line}\n", offset + k, width = width));
                }
                text = annotated;
            }
        }
        // Surface merge conflicts so the agent can resolve via conflict://N.
        if let Some(footer) = scan_and_register_conflicts(path, &text, offset) {
            if !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(&footer);
        }
        if let Some(next_offset) = page.next_offset {
            if !text.ends_with('\n') {
                text.push('\n');
            }
            match page.truncated_more_bytes {
                // Self-correcting byte-cap truncation: the model resumes at the
                // named offset instead of blindly retrying the oversized page.
                Some(more) => text.push_str(&format!(
                    "[truncated: {more} more bytes; next offset {next_offset}]"
                )),
                None => text.push_str(&format!(
                    "…[more content; re-call read_file with offset={next_offset}]"
                )),
            }
        }
        Ok(text)
    }
}

pub(crate) struct WriteFileTool {
    pub(crate) root: PathBuf,
}
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "write_file".to_string(),
            description: "Create or overwrite a text file in the workspace (creates parent \
                          directories as needed). Returns the new content tag for guarded \
                          follow-up edits without a reread. Also resolves \
                          `conflict://N` (and `conflict://*` bulk): content may be \
                          `@ours`, `@theirs`, `@base`, `@both`, or a custom body that \
                          replaces the marker region."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "workspace path, or conflict://N / conflict://* virtual URL" },
                    "content": { "type": "string", "description": "full file contents, or @ours/@theirs/@base/@both for conflict resolve" },
                },
                "required": ["path", "content"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        crate::harness::hardlink_result(|| {
            let path = args["path"].as_str().ok_or("missing 'path'")?;
            let content = args["content"].as_str().ok_or("missing 'content'")?;
            if let Some(msg) = try_write_virtual(&self.root, path, content)? {
                return Ok(msg);
            }
            confined_write(&self.root, Path::new(path), content.as_bytes())?;
            let tag = crate::hashline::record_snapshot(path, content);
            Ok(format!(
                "wrote {} bytes to {path} · new tag #{tag}",
                content.len()
            ))
        })
    }
}

pub(crate) struct StrReplaceTool {
    pub(crate) root: PathBuf,
}
impl Tool for StrReplaceTool {
    fn name(&self) -> &str {
        "str_replace"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "str_replace".to_string(),
            description: "Replace an exact, unique substring in a workspace file. Fails if \
                          'old' is absent or appears more than once — include enough \
                          surrounding context to make it unique. If the exact text isn't \
                          found, falls back to a whole-line match tolerant of trailing \
                          whitespace, CRLF, and smart quotes/dashes (indentation must still \
                          match — it never silently re-indents). Returns the new content tag \
                          for a guarded follow-up edit without a reread. An indentation-only \
                          miss can return exact `old` and `expect_tag` recovery fields; copy \
                          them and author `new` with the intended indentation."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "path inside the workspace (relative or absolute)" },
                    "old": { "type": "string", "description": "exact text to replace (must be unique)" },
                    "new": { "type": "string", "description": "replacement text" },
                    "expect_tag": { "type": "string", "description": "optional stale-edit guard: the file's content tag from read_file's [path#tag] header. The edit is rejected if the live file no longer matches it." },
                },
                "required": ["path", "old", "new"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        crate::harness::hardlink_result(|| {
            let path = args["path"].as_str().ok_or("missing 'path'")?;
            let old = args["old"].as_str().ok_or("missing 'old'")?;
            let new = args["new"].as_str().ok_or("missing 'new'")?;
            if old.is_empty() {
                return Err("'old' must not be empty".to_string());
            }
            let expect_tag = args["expect_tag"].as_str();
            let (fuzzy, updated) = confined_edit(&self.root, Path::new(path), |bytes| {
                let content = String::from_utf8(bytes).map_err(|e| format!("read {path}: {e}"))?;
                guard_expected_tag(path, &content, expect_tag)?;
                let (start, end, fuzzy) = locate_replacement(&content, old).map_err(|error| {
                    let error = replacement_recovery(&content, old, error, None);
                    format!("{error} in {path}")
                })?;
                let mut updated = String::with_capacity(content.len() - (end - start) + new.len());
                updated.push_str(&content[..start]);
                updated.push_str(new);
                updated.push_str(&content[end..]);
                Ok((updated.clone().into_bytes(), (fuzzy, updated)))
            })?;
            let tag = crate::hashline::record_snapshot(path, &updated);
            Ok(format!(
                "edited {path} (1 replacement{}) · new tag #{tag}",
                if fuzzy {
                    ", whitespace-tolerant match"
                } else {
                    ""
                }
            ))
        })
    }
}

pub(crate) struct MultiEditTool {
    pub(crate) root: PathBuf,
}
impl Tool for MultiEditTool {
    fn name(&self) -> &str {
        "multi_edit"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "multi_edit".to_string(),
            description: "Apply an ordered list of exact str-replace edits to ONE file, \
                          atomically (all-or-nothing). Edits apply in sequence; each 'old' \
                          must be unique in the file's current state. Fewer hops than \
                          repeated str_replace for a multi-site refactor. Returns the new \
                          content tag for a guarded follow-up edit without a reread."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "path inside the workspace (relative or absolute)" },
                    "expect_tag": { "type": "string", "description": "optional stale-edit guard: the file's content tag from read_file's [path#tag] header. All edits are rejected if the live file no longer matches it." },
                    "edits": {
                        "type": "array",
                        "description": "ordered edits, each applied to the result of the previous",
                        "items": {
                            "type": "object",
                            "properties": {
                                "old": { "type": "string" },
                                "new": { "type": "string" },
                            },
                            "required": ["old", "new"],
                        },
                    },
                },
                "required": ["path", "edits"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        crate::harness::hardlink_result(|| {
            let path = args["path"].as_str().ok_or("missing 'path'")?;
            let edits = args["edits"].as_array().ok_or("missing 'edits' array")?;
            if edits.is_empty() {
                return Err("'edits' must not be empty".to_string());
            }
            let expect_tag = args["expect_tag"].as_str();
            // Mutate an in-memory copy through one stable descriptor; only write if
            // EVERY edit lands, so a failed edit leaves the file untouched.
            let (any_fuzzy, updated) = confined_edit(&self.root, Path::new(path), |bytes| {
                let mut content =
                    String::from_utf8(bytes).map_err(|e| format!("read {path}: {e}"))?;
                guard_expected_tag(path, &content, expect_tag)?;
                let live_tag = crate::hashline::content_tag(&content);
                let mut any_fuzzy = false;
                for (i, e) in edits.iter().enumerate() {
                    let n = i + 1;
                    let old = e["old"]
                        .as_str()
                        .ok_or_else(|| format!("edit {n}: missing 'old'"))?;
                    let new = e["new"]
                        .as_str()
                        .ok_or_else(|| format!("edit {n}: missing 'new'"))?;
                    if old.is_empty() {
                        return Err(format!("edit {n}: 'old' must not be empty"));
                    }
                    let (start, end, fuzzy) =
                        locate_replacement(&content, old).map_err(|error| {
                            let error =
                                replacement_recovery(&content, old, error, Some((&live_tag, n)));
                            format!("edit {n}: {error} in {path}")
                        })?;
                    any_fuzzy |= fuzzy;
                    let mut next = String::with_capacity(content.len() - (end - start) + new.len());
                    next.push_str(&content[..start]);
                    next.push_str(new);
                    next.push_str(&content[end..]);
                    content = next;
                }
                Ok((content.clone().into_bytes(), (any_fuzzy, content)))
            })?;
            let tag = crate::hashline::record_snapshot(path, &updated);
            Ok(format!(
                "applied {} edits to {path}{} · new tag #{tag}",
                edits.len(),
                if any_fuzzy {
                    " (some whitespace-tolerant)"
                } else {
                    ""
                }
            ))
        })
    }
}

/// Map each line of `s` to its byte span `[start, end)` *excluding* the line
/// terminator (and a trailing `\r`), mirroring `str::lines()` so a pattern split
/// with `.lines()` aligns index-for-index with these spans.
fn line_spans(s: &str) -> Vec<(usize, usize)> {
    let bytes = s.as_bytes();
    let mut spans = Vec::new();
    let mut start = 0usize;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' {
            let mut end = i;
            if end > start && bytes[end - 1] == b'\r' {
                end -= 1; // fold CRLF to mirror str::lines()
            }
            spans.push((start, end));
            start = i + 1;
        }
    }
    if start < bytes.len() {
        let mut end = bytes.len();
        if end > start && bytes[end - 1] == b'\r' {
            end -= 1;
        }
        spans.push((start, end));
    }
    spans
}

/// Fold exotic typographic code-points (smart quotes, en/em dashes, non-breaking
/// & wide spaces) to their ASCII equivalents — the same mapping Codex's
/// `seek_sequence` uses — so an edit survives a copy that smuggled in curly
/// quotes or an nbsp. Only these specific characters are folded; ordinary
/// leading whitespace is left untouched so indentation can't be normalized away.
fn normalise_typography(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{2010}'..='\u{2015}' | '\u{2212}' => '-',
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => '\'',
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => '"',
            '\u{00A0}' | '\u{2002}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}' => ' ',
            other => other,
        })
        .collect()
}

/// 1-based line number for a byte offset into `content` (last line if past end).
fn line_number_at(content: &str, byte_offset: usize) -> usize {
    let prefix = &content[..byte_offset.min(content.len())];
    prefix.bytes().filter(|&b| b == b'\n').count() + 1
}

/// Bounded ambiguity hint so the model can disambiguate without re-reading the
/// whole file. Shows up to `cap` match starts with 1-based line numbers and a
/// single-line preview. Roll 07 debug rows burned hops on repeated non-unique
/// multi_edit of identical DateTimeParser sites that only differed by surrounding
/// function context the error never named.
fn format_ambiguous_matches(
    content: &str,
    byte_starts: &[usize],
    n: usize,
    whitespace_tolerant: bool,
) -> String {
    const CAP: usize = 5;
    const PREVIEW: usize = 100;
    let kind = if whitespace_tolerant {
        "whitespace-tolerant matches"
    } else {
        "matches"
    };
    let mut msg = format!("'old' is not unique ({n} {kind}) — add more context");
    if byte_starts.is_empty() {
        return msg;
    }
    msg.push_str(" that distinguishes the target. Occurrences start at lines ");
    let shown: Vec<usize> = byte_starts.iter().take(CAP).copied().collect();
    let line_nums: Vec<String> = shown
        .iter()
        .map(|&off| line_number_at(content, off).to_string())
        .collect();
    msg.push_str(&line_nums.join(", "));
    if n > CAP {
        msg.push_str(&format!(", … (+{} more)", n - CAP));
    }
    msg.push(':');
    for &off in &shown {
        let line_no = line_number_at(content, off);
        let line_start = content[..off].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let line_end = content[off..]
            .find('\n')
            .map(|i| off + i)
            .unwrap_or(content.len());
        let mut preview = content[line_start..line_end].trim_end();
        let mut truncated = false;
        if preview.len() > PREVIEW {
            // stay on a char boundary
            let mut end = PREVIEW;
            while end > 0 && !preview.is_char_boundary(end) {
                end -= 1;
            }
            preview = &preview[..end];
            truncated = true;
        }
        msg.push_str(&format!("\n  L{line_no}: {preview}"));
        if truncated {
            msg.push('…');
        }
    }
    msg.push_str("\nInclude surrounding unique lines (e.g. the enclosing function) in `old`.");
    msg
}

/// Locate the unique region of `content` to replace with the caller's `new`.
/// Tiers, tried in order — each requires a UNIQUE hit, because str_replace has no
/// positional anchor, so ambiguity must fail loudly rather than guess:
///   1. exact substring — the byte-identical fast path (matches mid-line too)
///   2. whole-line block ignoring per-line *trailing* whitespace and `\r` (CRLF)
///   3. whole-line block after also folding exotic typography to ASCII
///
/// Tiers 2–3 are the Codex `seek_sequence` idea, deliberately restricted to
/// *leading-whitespace-preserving* comparisons: indentation must always match,
/// so a fuzzy hit can never silently re-indent code (the dominant footgun of
/// fuzzy patching). Returns `(start, end, fuzzy)` — the byte range to splice and
/// whether a non-exact tier was used.
pub(crate) fn locate_replacement(content: &str, old: &str) -> Result<(usize, usize, bool), String> {
    // Tier 1: exact substring.
    let exact: Vec<usize> = content.match_indices(old).map(|(i, _)| i).collect();
    match exact.len() {
        1 => return Ok((exact[0], exact[0] + old.len(), false)),
        n if n > 1 => {
            return Err(format_ambiguous_matches(content, &exact, n, false));
        }
        _ => {}
    }
    // Tiers 2–4: whole-line, leading-whitespace-preserving fuzzy match. Each tier
    // only *locates* the byte range; leading whitespace is always part of the
    // comparison, so a fuzzy hit can never silently re-indent code. Modes,
    // easiest→loosest:
    //   0 trailing-ws/CRLF tolerant · 1 + exotic-typography folded ·
    //   2 + internal whitespace collapsed (model reflowed spacing inside a line)
    let cl = line_spans(content);
    let pat: Vec<&str> = old.lines().collect();
    if !pat.is_empty() && cl.len() >= pat.len() {
        for mode in 0u8..3 {
            let pn: Vec<String> = pat.iter().map(|p| norm_match_line(p, mode)).collect();
            let hits: Vec<usize> = (0..=cl.len() - pat.len())
                .filter(|&i| {
                    (0..pat.len())
                        .all(|k| norm_match_line(&content[cl[i + k].0..cl[i + k].1], mode) == pn[k])
                })
                .collect();
            match hits.len() {
                1 => {
                    let i = hits[0];
                    return Ok((cl[i].0, cl[i + pat.len() - 1].1, true));
                }
                n if n > 1 => {
                    let starts: Vec<usize> = hits.iter().map(|&i| cl[i].0).collect();
                    return Err(format_ambiguous_matches(content, &starts, n, true));
                }
                _ => {}
            }
        }
    }
    Err(format_near_miss(content, &cl, &pat))
}

/// Recovery is evidence for a new guarded call, never another matching tier.
/// In multi_edit the candidate belongs to its intermediate buffer, but the
/// guard belongs to the original live file and the entire atomic list retries.
fn replacement_recovery(
    content: &str,
    old: &str,
    mut error: String,
    atomic_retry: Option<(&str, usize)>,
) -> String {
    const MAX_RECOVERY_BYTES: usize = 4096;
    let pat: Vec<&str> = old.lines().map(str::trim).collect();
    let lines = line_spans(content);
    if pat.is_empty() || pat.len() > lines.len() || pat.iter().all(|line| line.is_empty()) {
        return error;
    }
    let mut matches = (0..=lines.len() - pat.len()).filter(|&start| {
        pat.iter().enumerate().all(|(at, expected)| {
            content[lines[start + at].0..lines[start + at].1].trim() == *expected
        })
    });
    let Some(start) = matches.next() else {
        return error;
    };
    if matches.next().is_some() {
        error.push_str("\nMultiple regions match after trimming; no recovery region selected. Add unique surrounding context.");
        return error;
    }
    let mut end = lines[start + pat.len() - 1].1;
    if old.ends_with('\n') {
        if content.as_bytes().get(end) == Some(&b'\r') {
            end += 1;
        }
        if content.as_bytes().get(end) == Some(&b'\n') {
            end += 1;
        }
    }
    let exact = &content[lines[start].0..end];
    // A unique line window can still be an ambiguous substring (for example
    // "    f();" also appears inside an eight-space line). Match retry semantics.
    if content.match_indices(exact).take(2).count() != 1 {
        error.push_str("\nThe exact region is not a unique substring; add surrounding context before retrying.");
        return error;
    }
    if exact.len() > MAX_RECOVERY_BYTES {
        error.push_str(&format!(
            "\nExact recovery exceeds {MAX_RECOVERY_BYTES} bytes; no clipped edit supplied. Read this path with offset={} limit={} and choose a smaller unique edit.",
            start + 1,
            pat.len().min(MAX_READ_PAGE_LINES),
        ));
        return error;
    }
    let mut recovery = serde_json::json!({
        "old": exact,
        "expect_tag": atomic_retry.map(|(tag, _)| tag.to_owned())
            .unwrap_or_else(|| crate::hashline::content_tag(content)),
    });
    if let Some((_, edit)) = atomic_retry {
        recovery["edit_index"] = serde_json::json!(edit);
    }
    let recovery = recovery.to_string();
    if recovery.len() > MAX_RECOVERY_BYTES {
        error.push_str("\nEscaped recovery exceeds 4096 bytes; no clipped edit supplied. Read the indicated region and choose a smaller unique edit.");
        return error;
    }
    error.push_str("\nNo edit was applied. Exact source is JSON-escaped below; copy `old` and author `new` with the intended indentation. Trimming is only a discovery hint, including for Python/YAML; it does not authorize re-indentation.");
    if atomic_retry.is_some() {
        error.push_str(" Retry the ENTIRE multi_edit list with this outer expect_tag; replace only the identified edit's old. This old describes the buffer AFTER preceding edits, not the unchanged live file. edit_index is one-based recovery metadata, not a tool argument.");
    } else {
        error.push_str(" Retry str_replace on the same path with these old/expect_tag fields.");
    }
    error.push_str("\n[edit recovery] ");
    error.push_str(&recovery);
    error.push('\n');
    error
}

/// All match tiers missed: point the model at the closest region so the next
/// call can succeed with a corrected `old` instead of a full re-read cycle
/// (28 misses in one night's trajectories — each cost a re-read + retry hop).
fn format_near_miss(content: &str, cl: &[(usize, usize)], pat: &[&str]) -> String {
    const BASE: &str = "'old' not found (even with whitespace/typography-tolerant matching)";
    if pat.is_empty() || cl.is_empty() {
        return BASE.to_string();
    }
    if pat.len() > cl.len() {
        return format!("{BASE} — `old` has more lines than the file itself");
    }
    // Score every window by loosest-mode line equality; break ties by
    // trimmed-line equality (detects indentation-only misses).
    let norm_content: Vec<String> = cl
        .iter()
        .map(|&(s, e)| norm_match_line(&content[s..e], 2))
        .collect();
    let norm_pat: Vec<String> = pat.iter().map(|p| norm_match_line(p, 2)).collect();
    let trim_pat: Vec<&str> = pat.iter().map(|p| p.trim()).collect();
    let mut best = (0usize, 0usize, 0usize); // (loose hits, trim hits, window start)
    for i in 0..=cl.len() - pat.len() {
        let mut loose = 0usize;
        let mut trimmed = 0usize;
        for k in 0..pat.len() {
            if norm_content[i + k] == norm_pat[k] {
                loose += 1;
            }
            if content[cl[i + k].0..cl[i + k].1].trim() == trim_pat[k] {
                trimmed += 1;
            }
        }
        let score = loose.max(trimmed);
        if (score, trimmed) > (best.0, best.1) {
            best = (score, trimmed, i);
        }
    }
    if best.0 == 0 {
        return format!(
            "{BASE} — no line of `old` matches anywhere; the file likely changed since it was read. Re-read the file before editing."
        );
    }
    let start = best.2;
    let indent_only = best.1 == pat.len();
    let mut msg = format!(
        "{BASE}\nClosest region ({}/{} lines match) — file content is authoritative:",
        best.0,
        pat.len()
    );
    let show_end = (start + pat.len()).min(cl.len());
    for (line_index, &(s, e)) in cl[start..show_end].iter().take(12).enumerate() {
        let mut preview = &content[s..e];
        if preview.len() > 160 {
            let mut end = 160;
            while end > 0 && !preview.is_char_boundary(end) {
                end -= 1;
            }
            preview = &preview[..end];
        }
        msg.push_str(&format!("\n  L{}: {preview}", start + line_index + 1));
    }
    if show_end - start > 12 {
        msg.push_str(&format!(
            "\n  … {} more lines omitted from preview",
            show_end - start - 12
        ));
    }
    if indent_only {
        msg.push_str(
            "\nEvery line matches after trimming: the LEADING WHITESPACE in `old` is wrong. Copy the exact indentation shown above.",
        );
    } else {
        msg.push_str(
            "\nCorrect `old` to the exact lines above (they may have changed since your last read).",
        );
    }
    msg
}

/// Normalize one line for fuzzy `locate_replacement` matching at the given mode.
/// Leading whitespace is ALWAYS preserved (so a match can't drive a re-indent);
/// only trailing/internal whitespace and exotic typography are folded.
fn norm_match_line(s: &str, mode: u8) -> String {
    let base = if mode >= 1 {
        normalise_typography(s)
    } else {
        s.to_string()
    };
    if mode >= 2 {
        // Preserve leading indent; collapse internal whitespace runs; drop trailing.
        let body = base.trim_start();
        let lead = &base[..base.len() - body.len()];
        let collapsed = body.split_whitespace().collect::<Vec<_>>().join(" ");
        format!("{lead}{collapsed}")
    } else {
        base.trim_end().to_string()
    }
}

// ---------------------------------------------------------------------------
// Freeform patch envelope (Codex apply_patch grammar). An alternative to unified
// diff that's friendlier for models to emit and is applied with the fuzzy
// `locate_replacement` matcher above, so hunks survive whitespace/context drift.
// ---------------------------------------------------------------------------

/// One file operation parsed from a freeform patch.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum FileOp {
    Add {
        path: String,
        content: String,
    },
    Delete {
        path: String,
    },
    Update {
        path: String,
        move_to: Option<String>,
        hunks: Vec<FreeformHunk>,
    },
}

/// One `@@`-delimited change within an Update: the `old` block (context + removed
/// lines) to locate and the `new` block (context + added lines) to write.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct FreeformHunk {
    pub(crate) old: String,
    pub(crate) new: String,
}

/// Parse the `*** Begin Patch … *** End Patch` envelope into file ops. Lenient on
/// surrounding blank lines and a closing ` ***` on envelope markers; strict on
/// malformed hunk lines (so the model gets a
/// clear correction rather than a silent mis-apply).
pub(crate) fn parse_freeform_patch(text: &str) -> Result<Vec<FileOp>, String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    // Skip to the begin marker.
    while i < lines.len() && !matches!(lines[i].trim(), "*** Begin Patch" | "*** Begin Patch ***") {
        i += 1;
    }
    if i == lines.len() {
        return Err("missing '*** Begin Patch'".to_string());
    }
    i += 1;
    let mut ops = Vec::new();
    while i < lines.len() {
        let line = lines[i];
        let t = line.trim_end();
        if matches!(t.trim(), "*** End Patch" | "*** End Patch ***") {
            return Ok(ops);
        } else if let Some(p) = t.strip_prefix("*** Add File: ") {
            i += 1;
            let mut content = String::new();
            while i < lines.len() && !lines[i].trim_start().starts_with("*** ") {
                let l = lines[i];
                let body = l.strip_prefix('+').unwrap_or(l);
                if !content.is_empty() {
                    content.push('\n');
                }
                content.push_str(body);
                i += 1;
            }
            ops.push(FileOp::Add {
                path: p.trim().to_string(),
                content,
            });
        } else if let Some(p) = t.strip_prefix("*** Delete File: ") {
            ops.push(FileOp::Delete {
                path: p.trim().to_string(),
            });
            i += 1;
        } else if let Some(p) = t.strip_prefix("*** Update File: ") {
            let path = p.trim().to_string();
            i += 1;
            let mut move_to = None;
            if i < lines.len()
                && let Some(m) = lines[i].trim_end().strip_prefix("*** Move to: ")
            {
                move_to = Some(m.trim().to_string());
                i += 1;
            }
            // Collect change lines until the next top-level marker, splitting into
            // hunks on each `@@`.
            let mut hunks = Vec::new();
            let mut cur: Vec<&str> = Vec::new();
            let flush =
                |cur: &mut Vec<&str>, hunks: &mut Vec<FreeformHunk>| -> Result<(), String> {
                    if cur.is_empty() {
                        return Ok(());
                    }
                    let h = build_hunk(cur)?;
                    hunks.push(h);
                    cur.clear();
                    Ok(())
                };
            while i < lines.len() && !lines[i].trim_start().starts_with("*** ") {
                let l = lines[i];
                if l.trim_start().starts_with("@@") {
                    flush(&mut cur, &mut hunks)?;
                } else {
                    cur.push(l);
                }
                i += 1;
            }
            flush(&mut cur, &mut hunks)?;
            if hunks.is_empty() {
                return Err(format!("update for {path} has no hunks"));
            }
            ops.push(FileOp::Update {
                path,
                move_to,
                hunks,
            });
        } else if t.trim().is_empty() {
            i += 1; // tolerate blank lines between file sections
        } else {
            return Err(format!("unexpected line in patch: {t:?}"));
        }
    }
    Err("missing '*** End Patch'".to_string())
}

/// Turn a hunk's change lines into `(old, new)` blocks. ` ` = context (both),
/// `-` = removed (old), `+` = added (new); a bare empty line is blank context.
fn build_hunk(change_lines: &[&str]) -> Result<FreeformHunk, String> {
    let mut old = Vec::new();
    let mut new = Vec::new();
    for &l in change_lines {
        if l == "*** End of File" {
            continue;
        }
        if l.is_empty() {
            old.push("");
            new.push("");
        } else if let Some(rest) = l.strip_prefix(' ') {
            old.push(rest);
            new.push(rest);
        } else if let Some(rest) = l.strip_prefix('-') {
            old.push(rest);
        } else if let Some(rest) = l.strip_prefix('+') {
            new.push(rest);
        } else {
            return Err(format!("malformed hunk line (need ' '/'+'/'-'): {l:?}"));
        }
    }
    if old.is_empty() {
        return Err("hunk has no context or removed lines to locate".to_string());
    }
    Ok(FreeformHunk {
        old: old.join("\n"),
        new: new.join("\n"),
    })
}

#[derive(Debug)]
enum PlannedFileOp {
    Add {
        path: String,
        content: Vec<u8>,
    },
    Delete {
        path: String,
        original: Vec<u8>,
    },
    Update {
        path: String,
        move_to: Option<String>,
        original: Vec<u8>,
        updated: Vec<u8>,
    },
}

#[derive(Debug)]
enum UndoFileOp {
    Remove {
        path: String,
    },
    Restore {
        path: String,
        content: Vec<u8>,
    },
    UndoMove {
        source: String,
        destination: String,
        content: Vec<u8>,
    },
}

fn rollback_freeform_patch(root: &Path, undo: Vec<UndoFileOp>) -> Result<(), String> {
    let mut failures = Vec::new();
    for op in undo.into_iter().rev() {
        let result = match op {
            UndoFileOp::Remove { path } => confined_remove_file(root, Path::new(&path)),
            UndoFileOp::Restore { path, content } => {
                confined_write(root, Path::new(&path), &content)
            }
            UndoFileOp::UndoMove {
                source,
                destination,
                content,
            } => confined_create_new(root, Path::new(&source), &content)
                .and_then(|()| confined_remove_file(root, Path::new(&destination))),
        };
        if let Err(error) = result {
            failures.push(error);
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

fn plan_freeform_patch(root: &Path, ops: Vec<FileOp>) -> Result<Vec<PlannedFileOp>, String> {
    let mut claimed = BTreeSet::new();
    for op in &ops {
        let mut claim = |path: &str| -> Result<(), String> {
            let resolved = safe_path(root, path)?;
            if !claimed.insert(resolved) {
                return Err(format!(
                    "patch touches {path:?} more than once; combine its hunks into one operation"
                ));
            }
            Ok(())
        };
        match op {
            FileOp::Add { path, .. } | FileOp::Delete { path } => claim(path)?,
            FileOp::Update { path, move_to, .. } => {
                claim(path)?;
                if let Some(destination) = move_to {
                    claim(destination)?;
                    if safe_path(root, destination)?.exists() {
                        return Err(format!(
                            "move destination already exists: {destination}; refusing to overwrite it"
                        ));
                    }
                }
            }
        }
    }

    let mut planned = Vec::with_capacity(ops.len());
    for op in ops {
        match op {
            FileOp::Add { path, content } => {
                let content = if content.is_empty() {
                    Vec::new()
                } else {
                    format!("{content}\n").into_bytes()
                };
                planned.push(PlannedFileOp::Add { path, content });
            }
            FileOp::Delete { path } => {
                let original = confined_read(root, Path::new(&path))
                    .map_err(|error| format!("delete {path}: {error}"))?;
                planned.push(PlannedFileOp::Delete { path, original });
            }
            FileOp::Update {
                path,
                move_to,
                hunks,
            } => {
                let original = confined_read(root, Path::new(&path))
                    .map_err(|error| format!("update {path}: {error}"))?;
                let mut content = String::from_utf8(original.clone())
                    .map_err(|error| format!("read {path}: {error}"))?;
                apply_hunks(&path, &hunks, &mut content)?;
                planned.push(PlannedFileOp::Update {
                    path,
                    move_to,
                    original,
                    updated: content.into_bytes(),
                });
            }
        }
    }
    Ok(planned)
}

/// Parse + apply a freeform patch under `root`. Every path, source byte string,
/// and hunk is validated before the first mutation. Commits then compare the
/// source against that preflight snapshot and roll prior operations back if a
/// later filesystem action fails.
fn apply_freeform_patch(root: &Path, text: &str) -> Result<String, String> {
    let ops = parse_freeform_patch(text)?;
    if ops.is_empty() {
        return Err("patch contains no file operations".to_string());
    }
    let planned = plan_freeform_patch(root, ops)?;
    #[cfg(target_os = "linux")]
    let _exit_guard = exit_safe_patch::Guard::enter()?;
    let (mut added, mut updated, mut deleted) = (0u32, 0u32, 0u32);
    let mut undo = Vec::with_capacity(planned.len());
    for op in planned {
        let result = (|| -> Result<(), String> {
            match op {
                PlannedFileOp::Add { path, content } => {
                    confined_create_new(root, Path::new(&path), &content)
                        .map_err(|error| format!("add file {path}: {error}"))?;
                    undo.push(UndoFileOp::Remove { path });
                    added += 1;
                    Ok(())
                }
                PlannedFileOp::Delete { path, original } => {
                    let current = confined_read(root, Path::new(&path))
                        .map_err(|error| format!("delete {path}: {error}"))?;
                    if current != original {
                        return Err(format!(
                            "delete {path}: file changed after patch preflight; no mutation performed"
                        ));
                    }
                    confined_remove_file(root, Path::new(&path))
                        .map_err(|error| format!("delete {path}: {error}"))?;
                    undo.push(UndoFileOp::Restore {
                        path,
                        content: original,
                    });
                    deleted += 1;
                    Ok(())
                }
                PlannedFileOp::Update {
                    path,
                    move_to,
                    original,
                    updated: content,
                } => {
                    match move_to {
                        Some(dest) => {
                            let current = confined_read(root, Path::new(&path))
                                .map_err(|error| format!("move {path}: {error}"))?;
                            if current != original {
                                return Err(format!(
                                    "move {path}: file changed after patch preflight; no mutation performed"
                                ));
                            }
                            confined_create_new(root, Path::new(&dest), &content)
                                .map_err(|error| format!("move {path} to {dest}: {error}"))?;
                            if let Err(error) = confined_remove_file(root, Path::new(&path)) {
                                let rollback = confined_remove_file(root, Path::new(&dest));
                                return Err(match rollback {
                                    Ok(()) => format!(
                                        "move {path} to {dest}: {error}; destination creation rolled back"
                                    ),
                                    Err(rollback_error) => format!(
                                        "move {path} to {dest}: {error}; rollback failed: {rollback_error}"
                                    ),
                                });
                            }
                            undo.push(UndoFileOp::UndoMove {
                                source: path,
                                destination: dest,
                                content: original,
                            });
                        }
                        None => {
                            confined_edit(root, Path::new(&path), |bytes| {
                                if bytes != original {
                                    return Err(format!(
                                        "update {path}: file changed after patch preflight; no mutation performed"
                                    ));
                                }
                                Ok((content, ()))
                            })?;
                            undo.push(UndoFileOp::Restore {
                                path,
                                content: original,
                            });
                        }
                    }
                    updated += 1;
                    Ok(())
                }
            }
        })();
        if let Err(error) = result {
            return Err(match rollback_freeform_patch(root, undo) {
                Ok(()) => format!("{error}; earlier patch operations rolled back"),
                Err(rollback_error) => {
                    format!("{error}; rollback incomplete: {rollback_error}")
                }
            });
        }
    }
    Ok(format!(
        "applied freeform patch: {added} added, {updated} updated, {deleted} deleted"
    ))
}

/// Apply a hashline patch: line-addressed, content-tag-anchored edits to
/// existing files. Each section verifies the file's live content tag before
/// applying (stale-patch guard), and the whole patch commits transactionally
/// with the same "changed since preflight" re-check the freeform path uses.
/// Supports REM (delete), MV (rename), and block ops via [`crate::hashline`].
fn apply_hashline_patch(root: &Path, text: &str, stage: bool) -> Result<String, String> {
    use crate::hashline::SectionPlan;

    let sections = crate::hashline::parse(text)?;
    // Preflight: resolve every path inside the workspace and compute the plan,
    // so a stale tag, out-of-bounds line, or destination collision fails before
    // any write. Stale tags may recover via the session snapshot store.
    let mut plans: Vec<(SectionPlan, Vec<u8>, Option<String>)> = Vec::new(); // plan, orig, recovery
    let mut claimed_dests = BTreeSet::new();
    for section in &sections {
        safe_path(root, section.path())?;
        let original = confined_read(root, Path::new(section.path()))
            .map_err(|error| format!("hashline {}: {error}", section.path()))?;
        let original_str = String::from_utf8(original.clone())
            .map_err(|_| format!("hashline {}: file is not valid UTF-8", section.path()))?;
        let outcome = crate::hashline::plan_section_detailed(&original_str, section)?;
        match &outcome.plan {
            SectionPlan::Move { to, .. } => {
                safe_path(root, to)?;
                if !claimed_dests.insert(to.clone()) {
                    return Err(format!(
                        "hashline: move destination {to:?} claimed more than once in this patch"
                    ));
                }
                if safe_path(root, to)?.exists() {
                    return Err(format!(
                        "hashline: move destination already exists: {to}; refusing to overwrite it"
                    ));
                }
            }
            SectionPlan::Update { path, .. } | SectionPlan::Remove { path } => {
                // Source paths must not collide with a peer section's dest.
                if claimed_dests.contains(path) {
                    return Err(format!(
                        "hashline: {path} is both a move destination and another section's target"
                    ));
                }
            }
        }
        plans.push((outcome.plan, original, outcome.recovery_note));
    }

    // Stage mode: hold plans off disk until resolve_edit accept.
    if stage {
        let (actions, notes) = crate::staged_edit::actions_from_hashline_plans(plans);
        return crate::staged_edit::stage_batch(root, actions, notes);
    }

    // Live commit via shared staged_edit commit path.
    let (actions, notes) = crate::staged_edit::actions_from_hashline_plans(plans);
    #[cfg(target_os = "linux")]
    let _exit_guard = exit_safe_patch::Guard::enter()?;
    let receipts = crate::staged_edit::commit_actions(root, actions)?;
    let note_suffix = if notes.is_empty() {
        String::new()
    } else {
        format!(" · {}", notes.join("; "))
    };
    Ok(format!(
        "applied hashline patch: {} file(s) — {}{note_suffix}",
        receipts.len(),
        receipts.join("; ")
    ))
}

fn apply_hunks(path: &str, hunks: &[FreeformHunk], content: &mut String) -> Result<(), String> {
    for (n, h) in hunks.iter().enumerate() {
        let (start, end, _) = locate_replacement(content, &h.old)
            .map_err(|e| format!("update {path} hunk {}: {e}", n + 1))?;
        let mut next = String::with_capacity(content.len());
        next.push_str(&content[..start]);
        next.push_str(&h.new);
        next.push_str(&content[end..]);
        *content = next;
    }
    Ok(())
}

/// Accept or reject a staged hashline edit batch (`apply_patch` with stage=true).
pub(crate) struct ResolveEditTool {
    pub(crate) root: PathBuf,
}
impl Tool for ResolveEditTool {
    fn name(&self) -> &str {
        "resolve_edit"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "resolve_edit".to_string(),
            description: "Accept or reject a staged edit batch created by apply_patch \
                          with stage=true (oh-my-pi preview/accept). action=list shows \
                          pending proposals; accept writes transactionally; reject drops \
                          the proposal. Disk is untouched until accept."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "description": "accept | reject | list (default list)"
                    },
                    "id": {
                        "type": "integer",
                        "description": "staged edit id from the proposal card (required for accept/reject)"
                    },
                    "reason": {
                        "type": "string",
                        "description": "optional note recorded on accept/reject"
                    },
                },
                "required": [],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        crate::harness::hardlink_result(|| {
            let action = args["action"]
                .as_str()
                .unwrap_or("list")
                .trim()
                .to_ascii_lowercase();
            let reason = args["reason"].as_str().filter(|s| !s.is_empty());
            match action.as_str() {
                "list" | "" => Ok(crate::staged_edit::list_staged()),
                "accept" => {
                    let id = args["id"]
                        .as_u64()
                        .ok_or("resolve_edit accept requires integer id")?
                        as u32;
                    // Ensure batch was staged for this workspace when possible.
                    #[cfg(target_os = "linux")]
                    let _exit_guard = exit_safe_patch::Guard::enter()?;
                    crate::staged_edit::accept(id, reason).inspect(|_msg| {
                        // Touch root so capture_writes / dependents see activity.
                        let _ = &self.root;
                    })
                }
                "reject" => {
                    let id = args["id"]
                        .as_u64()
                        .ok_or("resolve_edit reject requires integer id")?
                        as u32;
                    crate::staged_edit::reject(id, reason)
                }
                other => Err(format!(
                    "unknown action {other:?}; use accept, reject, or list"
                )),
            }
        })
    }
}

pub(crate) struct ApplyPatchTool {
    pub(crate) root: PathBuf,
}
impl Tool for ApplyPatchTool {
    fn name(&self) -> &str {
        "apply_patch"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "apply_patch".to_string(),
            description: "Apply a preflight-validated patch to the workspace (multi-file). Accepts a unified \
                          diff (applied via `git apply`/`patch`), the freeform envelope \
                          (`*** Begin Patch` / `*** Add File:` / `*** Update File:` with `@@` \
                          hunks of ` `/`+`/`-` lines / `*** End Patch`, whitespace-tolerant), OR \
                          the token-cheap hashline format: `*** Begin Patch`, then per file a \
                          `[path#tag]` header (tag = the content hash shown by read_file), then \
                          line-addressed ops — `SWAP A[.=B]:` / `SWAP.BLK A:` / `INS.PRE A:` / \
                          `INS.POST A:` / `INS.BLK.POST A:` / `INS.HEAD:` / `INS.TAIL:` each \
                          followed by `+`-prefixed new rows; `DEL A[.=B]` / `DEL.BLK A`; whole-file \
                          `REM`; rename `MV DEST` (optionally after line edits) — then `*** End Patch`. \
                          Block ops resolve the multi-line construct beginning on line A (brace, \
                          markdown heading, or indent suite). Hashline emits only the NEW lines \
                          (no retyping the old). If `tag` no longer matches the live file, the \
                          patcher first tries session-snapshot recovery (remap anchors through \
                          unchanged lines); only an unrecoverable drift rejects. Receipts return \
                          each file's new tag so the next edit can chain without a reread. Every \
                          target path must stay inside the workspace; every edit is checked before \
                          the first write. Hashline also supports stage=true: preflight and queue \
                          the plan without writing; call resolve_edit to accept or reject."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "diff": { "type": "string", "description": "unified diff (a/ b/ headers)" },
                    "strip": { "type": "integer", "description": "path strip level -pN (default 1)" },
                    "stage": {
                        "type": "boolean",
                        "description": "hashline only: when true, preflight and stage the plan (no disk write); resolve with resolve_edit"
                    },
                },
                "required": ["diff"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        crate::harness::hardlink_result(|| {
            let diff = args["diff"].as_str().ok_or("missing 'diff'")?;
            if diff.trim().is_empty() {
                return Err("'diff' is empty".to_string());
            }
            let stage = args["stage"].as_bool().unwrap_or(false);
            // `*** Begin Patch` fronts two formats. The hashline variant (line-
            // addressed, content-tag-anchored) carries `[path#tag]` section headers
            // the freeform envelope never has; route on that. Both avoid shelling
            // out to git/patch.
            if diff.trim_start().starts_with("*** Begin Patch") {
                if crate::hashline::looks_like_hashline(diff) {
                    return apply_hashline_patch(&self.root, diff, stage);
                }
                if stage {
                    return Err(
                    "stage=true is only supported for hashline patches (sections with [path#tag])"
                        .into(),
                );
                }
                return apply_freeform_patch(&self.root, diff);
            }
            if stage {
                return Err("stage=true is only supported for hashline patches".into());
            }
            let strip = args["strip"].as_u64().unwrap_or(1);

            // Every file the diff touches must resolve inside the workspace.
            let mut targets = extract_diff_targets(diff);
            if targets.is_empty() {
                return Err("no '+++'/'---' file headers found in diff".to_string());
            }
            targets.sort();
            targets.dedup();
            for t in &targets {
                safe_path(&self.root, t)?;
            }

            std::fs::create_dir_all(&self.root).ok();
            static PATCH_SEQUENCE: AtomicU64 = AtomicU64::new(0);
            let patch_path = self.root.join(format!(
                ".angel_patch_{}_{}.diff",
                std::process::id(),
                PATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            {
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&patch_path)
                    .map_err(|e| format!("create patch tmp: {e}"))?;
                file.write_all(diff.as_bytes())
                    .map_err(|e| format!("write patch tmp: {e}"))?;
            }
            let pn = format!("-p{strip}");

            let outcome = match check_and_apply_with(
                "git",
                &["apply", "--check", &pn],
                &["apply", &pn],
                &patch_path,
                &self.root,
            ) {
                Ok(()) => Ok("git apply"),
                Err(ge) => match check_and_apply_with(
                    "patch",
                    &["--batch", "--forward", "--dry-run", &pn, "-i"],
                    &["--batch", "--forward", &pn, "-i"],
                    &patch_path,
                    &self.root,
                ) {
                    Ok(()) => Ok("patch"),
                    Err(pe) => Err(format!("could not apply diff — git: {ge}; patch: {pe}")),
                },
            };
            let _ = std::fs::remove_file(&patch_path);
            let via = outcome?;
            Ok(format!(
                "applied diff via {via} to {} file(s): {}",
                targets.len(),
                targets.join(", ")
            ))
        })
    }
}

/// Pull the touched file paths out of a unified diff's `+++`/`---` headers,
/// stripping the conventional `a/`/`b/` prefix and ignoring `/dev/null`.
fn extract_diff_targets(diff: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in diff.lines() {
        let rest = match line
            .strip_prefix("+++ ")
            .or_else(|| line.strip_prefix("--- "))
        {
            Some(r) => r,
            None => continue,
        };
        let p = rest.split('\t').next().unwrap_or("").trim();
        if p.is_empty() || p == "/dev/null" {
            continue;
        }
        let p = p.strip_prefix("a/").unwrap_or(p);
        let p = p.strip_prefix("b/").unwrap_or(p);
        out.push(p.to_string());
    }
    out
}

/// Run a patch applier (`git apply`/`patch`) with the diff file appended to
/// `pre_args`, in `root`. Ok(()) iff it exits 0.
fn apply_with(
    program: &str,
    pre_args: &[&str],
    patch_path: &Path,
    root: &Path,
) -> Result<(), String> {
    // `git apply`/`patch` necessarily operate by pathname. On Linux, force a
    // write-confined Landlock domain even when the general shell sandbox is
    // disabled, so a target swapped to an outbound symlink cannot be modified.
    #[cfg(target_os = "linux")]
    let mut cmd = {
        let policy = crate::sandbox::SandboxPolicy {
            writable_roots: vec![root.to_path_buf()],
            allow_network: false,
            enforce: true,
            mandatory: true,
            sealed_reads: Vec::new(),
            deny_reads: Vec::new(),
        };
        let command_args = pre_args
            .iter()
            .map(OsString::from)
            .chain(std::iter::once(patch_path.as_os_str().to_owned()))
            .collect::<Vec<_>>();
        crate::sandbox::command(program, &command_args, &policy)?
    };
    #[cfg(not(target_os = "linux"))]
    let mut cmd = {
        let mut command = Command::new(program);
        command.args(pre_args).arg(patch_path);
        command
    };
    cmd.current_dir(root);
    cmd.env("TMPDIR", root);
    // `output_timed` supplies a null stdin, capped head+tail capture, and
    // process-group cleanup. In particular the fallback `patch` command can
    // otherwise prompt on stdin or leave a child holding a pipe forever.
    let (out, timed_out) =
        output_timed(cmd, tool_timeout()).map_err(|e| format!("{program} unavailable: {e}"))?;
    if timed_out {
        return Err(format!(
            "{program} timed out after {}s; process group killed",
            tool_timeout().map(|timeout| timeout.as_secs()).unwrap_or(0)
        ));
    }
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

fn check_and_apply_with(
    program: &str,
    check_args: &[&str],
    apply_args: &[&str],
    patch_path: &Path,
    root: &Path,
) -> Result<(), String> {
    apply_with(program, check_args, patch_path, root)
        .map_err(|error| format!("preflight failed: {error}"))?;
    let diff = std::fs::read_to_string(patch_path).map_err(|e| e.to_string())?;
    for target in extract_diff_targets(&diff) {
        crate::harness::confined_break_hardlink(root, Path::new(&target))?;
    }
    apply_with(program, apply_args, patch_path, root)
        .map_err(|error| format!("commit failed after successful preflight: {error}"))
}

#[cfg(test)]
#[path = "../../../tests/cockpit/tools/file__replace_tests.rs"]
mod replace_tests;

#[cfg(all(test, target_os = "linux"))]
#[path = "../../../tests/cockpit/tools/file__interruption_tests.rs"]
mod interruption_tests;

#[cfg(all(test, target_os = "linux"))]
#[path = "../../../tests/cockpit/tools/file__hardlink_tests.rs"]
mod hardlink_tests;
