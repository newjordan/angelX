//! Merge-conflict detection and `conflict://` resolution (oh-my-pi style).
//!
//! Workflow:
//! 1. `read_file` scans the page (or whole file when small) for well-formed
//!    `<<<<<<<` / `=======` / `>>>>>>>` blocks (optional diff3 `|||||||` base).
//! 2. Each completed block is registered in a process-wide history and gets a
//!    stable numeric id.
//! 3. The agent resolves with `write_file path=conflict://N content=@theirs`
//!    (or `@ours` / `@base` / custom body). The recorded region is spliced out
//!    of the live file by content match, not brittle line numbers alone.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const OURS_PREFIX: &str = "<<<<<<<";
const BASE_PREFIX: &str = "|||||||";
const SEPARATOR: &str = "=======";
const THEIRS_PREFIX: &str = ">>>>>>>";

/// One fully-closed conflict region as observed on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConflictBlock {
    /// 1-indexed line of the `<<<<<<<` marker.
    pub start_line: usize,
    /// 1-indexed line of the `=======` separator.
    pub separator_line: usize,
    /// 1-indexed line of the `>>>>>>>` marker.
    pub end_line: usize,
    /// 1-indexed line of the optional `|||||||` base marker (diff3).
    pub base_line: Option<usize>,
    pub ours_label: Option<String>,
    pub base_label: Option<String>,
    pub theirs_label: Option<String>,
    pub ours_lines: Vec<String>,
    pub base_lines: Option<Vec<String>>,
    pub theirs_lines: Vec<String>,
}

/// Registered conflict with a session-stable id and workspace-relative path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConflictEntry {
    pub id: u32,
    pub path: String,
    pub block: ConflictBlock,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConflictScope {
    Ours,
    Theirs,
    Base,
}

/// Parsed `conflict://` target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConflictUri {
    /// List every currently registered conflict.
    List,
    /// One conflict (optionally one side only).
    One {
        id: u32,
        scope: Option<ConflictScope>,
    },
    /// Bulk resolve every registered conflict with the same content.
    All,
}

/// Whether `raw` looks like a `conflict://…` path (including recovered prefixes).
#[allow(dead_code)] // public for callers that branch before parse
pub(crate) fn is_conflict_uri(raw: &str) -> bool {
    let t = raw.trim();
    t.starts_with("conflict:") || t.contains(":conflict://")
}

/// Parse `conflict://*`, `conflict://N`, `conflict://N/ours|theirs|base`, or
/// the bare `conflict://` list form. Also accepts `path:conflict://N` recovery.
pub(crate) fn parse_conflict_uri(raw: &str) -> Result<ConflictUri, String> {
    let trimmed = raw.trim();
    let (recovered, rest) = match trimmed.rfind(":conflict://") {
        Some(idx) if idx > 0 => (true, &trimmed[idx + 1..]),
        _ => (false, trimmed),
    };
    let Some(tail) = rest
        .strip_prefix("conflict://")
        .or_else(|| rest.strip_prefix("conflict:/"))
    else {
        return Err(format!(
            "not a conflict:// URI: {raw:?}{}",
            if recovered {
                " (recovered prefix stripped)"
            } else {
                ""
            }
        ));
    };
    let tail = tail.trim_start_matches('/');
    if tail.is_empty() || tail == "*" || tail.eq_ignore_ascii_case("list") {
        if tail == "*" {
            return Ok(ConflictUri::All);
        }
        return Ok(ConflictUri::List);
    }
    let (id_part, scope_part) = match tail.split_once('/') {
        Some((id, scope)) => (id, Some(scope)),
        None => (tail, None),
    };
    if id_part == "*" {
        if scope_part.is_some() {
            return Err(
                "conflict://* does not accept a scope segment; use conflict://N/ours|theirs|base"
                    .into(),
            );
        }
        return Ok(ConflictUri::All);
    }
    let id: u32 = id_part.parse().map_err(|_| {
        format!(
            "invalid conflict URI {raw:?}: expected conflict://N, conflict://N/<scope>, \
             conflict://*, or conflict://"
        )
    })?;
    if id == 0 {
        return Err("conflict id must be ≥ 1".into());
    }
    let scope = match scope_part {
        None => None,
        Some("ours") => Some(ConflictScope::Ours),
        Some("theirs") => Some(ConflictScope::Theirs),
        Some("base") => Some(ConflictScope::Base),
        Some(other) => {
            return Err(format!(
                "invalid conflict scope {other:?}; use ours, theirs, or base"
            ));
        }
    };
    Ok(ConflictUri::One { id, scope })
}

/// Scan `lines` for completed conflict blocks. `first_line` is the 1-based
/// number of `lines[0]` (windowed reads pass their page offset).
pub(crate) fn scan_conflict_lines(lines: &[&str], first_line: usize) -> Vec<ConflictBlock> {
    let mut blocks = Vec::new();
    let mut phase = Phase::Idle;
    let mut partial: Option<Partial> = None;

    for (i, raw) in lines.iter().enumerate() {
        let line = strip_trailing_cr(raw);
        let ln = first_line + i;

        if let Some(label) = match_marker(line, OURS_PREFIX) {
            partial = Some(Partial {
                start_line: ln,
                ours_label: nonempty_label(label),
                ours_lines: Vec::new(),
                base_line: None,
                base_label: None,
                base_lines: None,
                separator_line: None,
                theirs_lines: None,
            });
            phase = Phase::Ours;
            continue;
        }
        if phase == Phase::Idle || partial.is_none() {
            continue;
        }
        let p = partial.as_mut().unwrap();

        if let Some(label) = match_marker(line, BASE_PREFIX) {
            if phase != Phase::Ours {
                partial = None;
                phase = Phase::Idle;
                continue;
            }
            p.base_line = Some(ln);
            p.base_label = nonempty_label(label);
            p.base_lines = Some(Vec::new());
            phase = Phase::Base;
            continue;
        }

        if line == SEPARATOR {
            if phase == Phase::Ours || phase == Phase::Base {
                p.separator_line = Some(ln);
                p.theirs_lines = Some(Vec::new());
                phase = Phase::Theirs;
            } else {
                partial = None;
                phase = Phase::Idle;
            }
            continue;
        }

        if let Some(label) = match_marker(line, THEIRS_PREFIX) {
            if phase == Phase::Theirs
                && let (Some(sep), Some(theirs)) = (p.separator_line, p.theirs_lines.take())
            {
                blocks.push(ConflictBlock {
                    start_line: p.start_line,
                    separator_line: sep,
                    end_line: ln,
                    base_line: p.base_line,
                    ours_label: p.ours_label.clone(),
                    base_label: p.base_label.clone(),
                    theirs_label: nonempty_label(label),
                    ours_lines: std::mem::take(&mut p.ours_lines),
                    base_lines: p.base_lines.take(),
                    theirs_lines: theirs,
                });
            }
            partial = None;
            phase = Phase::Idle;
            continue;
        }

        match phase {
            Phase::Ours => p.ours_lines.push(line.to_string()),
            Phase::Base => {
                if let Some(base) = p.base_lines.as_mut() {
                    base.push(line.to_string());
                }
            }
            Phase::Theirs => {
                if let Some(theirs) = p.theirs_lines.as_mut() {
                    theirs.push(line.to_string());
                }
            }
            Phase::Idle => {}
        }
    }
    blocks
}

/// Scan a full file text for conflicts.
#[cfg(test)]
pub(crate) fn scan_text_for_conflicts(text: &str) -> Vec<ConflictBlock> {
    let lines: Vec<&str> = text.lines().collect();
    scan_conflict_lines(&lines, 1)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Ours,
    Base,
    Theirs,
}

struct Partial {
    start_line: usize,
    ours_label: Option<String>,
    ours_lines: Vec<String>,
    base_line: Option<usize>,
    base_label: Option<String>,
    base_lines: Option<Vec<String>>,
    separator_line: Option<usize>,
    theirs_lines: Option<Vec<String>>,
}

fn strip_trailing_cr(line: &str) -> &str {
    line.strip_suffix('\r').unwrap_or(line)
}

fn match_marker<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    if !line.starts_with(prefix) {
        return None;
    }
    if line.len() == prefix.len() {
        return Some("");
    }
    if line.as_bytes().get(prefix.len()) != Some(&b' ') {
        return None;
    }
    Some(&line[prefix.len() + 1..])
}

fn nonempty_label(label: &str) -> Option<String> {
    let t = label.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

// --- session history --------------------------------------------------------

#[derive(Default)]
struct ConflictHistory {
    next_id: u32,
    by_id: HashMap<u32, ConflictEntry>,
}

impl ConflictHistory {
    fn register(&mut self, path: &str, block: ConflictBlock) -> ConflictEntry {
        // Reuse id when the same path+start_line is re-read.
        for existing in self.by_id.values() {
            if existing.path == path && existing.block.start_line == block.start_line {
                let id = existing.id;
                let entry = ConflictEntry {
                    id,
                    path: path.to_string(),
                    block,
                };
                self.by_id.insert(id, entry.clone());
                return entry;
            }
        }
        if self.next_id == 0 {
            self.next_id = 1;
        }
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let entry = ConflictEntry {
            id,
            path: path.to_string(),
            block,
        };
        self.by_id.insert(id, entry.clone());
        entry
    }

    fn get(&self, id: u32) -> Option<&ConflictEntry> {
        self.by_id.get(&id)
    }

    fn entries(&self) -> Vec<ConflictEntry> {
        let mut v: Vec<_> = self.by_id.values().cloned().collect();
        v.sort_by_key(|e| e.id);
        v
    }

    fn invalidate(&mut self, id: u32) {
        self.by_id.remove(&id);
    }

    fn invalidate_path(&mut self, path: &str) {
        self.by_id.retain(|_, e| e.path != path);
    }

    #[cfg(test)]
    fn clear(&mut self) {
        self.by_id.clear();
        self.next_id = 1;
    }
}

fn history() -> std::sync::MutexGuard<'static, ConflictHistory> {
    static H: OnceLock<Mutex<ConflictHistory>> = OnceLock::new();
    let mtx = H.get_or_init(|| Mutex::new(ConflictHistory::default()));
    match mtx.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    }
}

/// Register conflicts found on `path` and return the assigned entries.
pub(crate) fn register_conflicts(path: &str, blocks: &[ConflictBlock]) -> Vec<ConflictEntry> {
    let mut h = history();
    blocks.iter().map(|b| h.register(path, b.clone())).collect()
}

pub(crate) fn get_conflict(id: u32) -> Option<ConflictEntry> {
    history().get(id).cloned()
}

pub(crate) fn list_conflicts() -> Vec<ConflictEntry> {
    history().entries()
}

pub(crate) fn invalidate_conflict(id: u32) {
    history().invalidate(id);
}

pub(crate) fn invalidate_conflicts_for_path(path: &str) {
    history().invalidate_path(path);
}

#[cfg(test)]
pub(crate) fn clear_conflicts_for_test() {
    history().clear();
}

/// Expand `@ours` / `@theirs` / `@base` / `@both` shorthand, or return custom
/// content as-is (without a trailing forced newline — caller joins lines).
pub(crate) fn resolve_replacement(entry: &ConflictEntry, content: &str) -> Result<String, String> {
    let trimmed = content.trim();
    let lines = match trimmed {
        "@ours" => entry.block.ours_lines.clone(),
        "@theirs" => entry.block.theirs_lines.clone(),
        "@base" => entry.block.base_lines.clone().ok_or_else(|| {
            format!(
                "conflict://{} has no base side (not a diff3 conflict)",
                entry.id
            )
        })?,
        "@both" => {
            let mut both = entry.block.ours_lines.clone();
            both.extend(entry.block.theirs_lines.iter().cloned());
            both
        }
        _ => {
            // Custom body: keep as provided, normalize to LF lines.
            return Ok(content.replace("\r\n", "\n").replace('\r', "\n"));
        }
    };
    let mut body = lines.join("\n");
    if !body.is_empty() {
        body.push('\n');
    }
    Ok(body)
}

/// Splice `replacement` over the conflict region in `original`, locating the
/// marker block by content (preferring `entry.block.start_line` as the match).
pub(crate) fn splice_conflict(
    original: &str,
    entry: &ConflictEntry,
    replacement: &str,
) -> Result<String, String> {
    let uses_crlf = original.contains("\r\n");
    let had_trailing_newline = original.ends_with('\n');
    let norm = original.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = norm.lines().collect();
    let n = lines.len();
    let b = &entry.block;

    // Rebuild the expected marker span text from the recorded block.
    let mut expected: Vec<String> = Vec::new();
    expected.push(format_marker(OURS_PREFIX, b.ours_label.as_deref()));
    expected.extend(b.ours_lines.iter().cloned());
    if let Some(base_lines) = &b.base_lines {
        expected.push(format_marker(BASE_PREFIX, b.base_label.as_deref()));
        expected.extend(base_lines.iter().cloned());
    }
    expected.push(SEPARATOR.to_string());
    expected.extend(b.theirs_lines.iter().cloned());
    expected.push(format_marker(THEIRS_PREFIX, b.theirs_label.as_deref()));

    let span_len = expected.len();
    if span_len == 0 || n < span_len {
        return Err(format!(
            "conflict://{}: recorded region no longer fits in the file; re-read",
            entry.id
        ));
    }

    // Prefer the recorded start line; fall back to a unique content scan.
    let prefer = b.start_line.saturating_sub(1); // 0-based
    let mut match_at: Option<usize> = None;
    if prefer + span_len <= n
        && lines[prefer..prefer + span_len]
            .iter()
            .zip(expected.iter())
            .all(|(a, e)| *a == e.as_str())
    {
        match_at = Some(prefer);
    }
    if match_at.is_none() {
        let mut hits = Vec::new();
        for i in 0..=(n - span_len) {
            if lines[i..i + span_len]
                .iter()
                .zip(expected.iter())
                .all(|(a, e)| *a == e.as_str())
            {
                hits.push(i);
            }
        }
        match hits.as_slice() {
            [one] => match_at = Some(*one),
            [] => {
                return Err(format!(
                    "conflict://{}: marker block for {} not found (already resolved or edited); re-read",
                    entry.id, entry.path
                ));
            }
            _ => {
                return Err(format!(
                    "conflict://{}: marker block matched {} times in {}; re-read and resolve carefully",
                    entry.id,
                    hits.len(),
                    entry.path
                ));
            }
        }
    }
    let at = match_at.unwrap();

    // Build replacement lines (without forcing trailing blank unless present).
    let repl_norm = replacement.replace("\r\n", "\n").replace('\r', "\n");
    let mut repl_lines: Vec<&str> = if repl_norm.is_empty() {
        Vec::new()
    } else if repl_norm.ends_with('\n') {
        // Keep empty last line only if body is non-empty with trailing newline
        // — join logic below treats trailing newline separately for the file.
        let mut v: Vec<&str> = repl_norm.lines().collect();
        // `lines()` drops a final empty after trailing \n — good for line list.
        // If content was "\n" only, lines() is empty; treat as one blank line.
        if v.is_empty() && replacement.contains('\n') {
            // pure newline(s): leave empty so we insert nothing extra? @ours empty sides insert nothing.
        }
        let _ = &mut v;
        repl_norm.lines().collect()
    } else {
        repl_norm.lines().collect()
    };

    // If replacement was non-empty without trailing newline, lines() is fine.
    // Empty sides (resolve with empty ours) → insert zero lines (delete conflict).
    if replacement.is_empty() {
        repl_lines.clear();
    }

    let mut out: Vec<&str> = Vec::with_capacity(n - span_len + repl_lines.len());
    out.extend_from_slice(&lines[..at]);
    out.extend_from_slice(&repl_lines);
    out.extend_from_slice(&lines[at + span_len..]);

    let mut joined = out.join("\n");
    if had_trailing_newline && !joined.ends_with('\n') && !out.is_empty() {
        joined.push('\n');
    }
    if out.is_empty() && had_trailing_newline {
        joined.push('\n');
    }
    if uses_crlf {
        joined = joined.replace('\n', "\r\n");
    }
    Ok(joined)
}

fn format_marker(prefix: &str, label: Option<&str>) -> String {
    match label {
        Some(l) if !l.is_empty() => format!("{prefix} {l}"),
        _ => prefix.to_string(),
    }
}

/// Human footer listing registered conflicts for a read receipt.
pub(crate) fn format_conflict_footer(entries: &[ConflictEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let mut lines = vec![format!(
        "⚠ {} merge conflict(s) — resolve with write_file path=conflict://N content=@ours|@theirs|@base|custom:",
        entries.len()
    )];
    for e in entries {
        let span = e.block.end_line.saturating_sub(e.block.start_line) + 1;
        lines.push(format!(
            "  conflict://{}  {}  lines {}-{} ({} lines)  ours={} theirs={}",
            e.id,
            e.path,
            e.block.start_line,
            e.block.end_line,
            span,
            e.block.ours_lines.len(),
            e.block.theirs_lines.len()
        ));
    }
    lines.join("\n")
}

/// Render a single conflict entry for `read_file conflict://N`.
pub(crate) fn format_conflict_detail(
    entry: &ConflictEntry,
    scope: Option<&ConflictScope>,
) -> String {
    let b = &entry.block;
    match scope {
        Some(ConflictScope::Ours) => {
            b.ours_lines.join("\n") + if b.ours_lines.is_empty() { "" } else { "\n" }
        }
        Some(ConflictScope::Theirs) => {
            b.theirs_lines.join("\n") + if b.theirs_lines.is_empty() { "" } else { "\n" }
        }
        Some(ConflictScope::Base) => match &b.base_lines {
            Some(base) => base.join("\n") + if base.is_empty() { "" } else { "\n" },
            None => format!("conflict://{} has no base side\n", entry.id),
        },
        None => {
            let mut out = format!(
                "[conflict://{}] path={} lines={}-{} separator={}\n",
                entry.id, entry.path, b.start_line, b.end_line, b.separator_line
            );
            if let Some(l) = &b.ours_label {
                out.push_str(&format!("ours_label={l}\n"));
            }
            if let Some(l) = &b.theirs_label {
                out.push_str(&format!("theirs_label={l}\n"));
            }
            out.push_str("--- ours ---\n");
            for line in &b.ours_lines {
                out.push_str(line);
                out.push('\n');
            }
            if let Some(base) = &b.base_lines {
                out.push_str("--- base ---\n");
                for line in base {
                    out.push_str(line);
                    out.push('\n');
                }
            }
            out.push_str("--- theirs ---\n");
            for line in &b.theirs_lines {
                out.push_str(line);
                out.push('\n');
            }
            out.push_str("Resolve: write_file path=conflict://");
            out.push_str(&format!(
                "{} content=@ours|@theirs|@base|@both|custom\n",
                entry.id
            ));
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_standard_two_way_conflict() {
        let text = "head\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> branch\ntail\n";
        let blocks = scan_text_for_conflicts(text);
        assert_eq!(blocks.len(), 1);
        let b = &blocks[0];
        assert_eq!(b.start_line, 2);
        assert_eq!(b.separator_line, 4);
        assert_eq!(b.end_line, 6);
        assert_eq!(b.ours_lines, vec!["ours".to_string()]);
        assert_eq!(b.theirs_lines, vec!["theirs".to_string()]);
        assert_eq!(b.ours_label.as_deref(), Some("HEAD"));
        assert_eq!(b.theirs_label.as_deref(), Some("branch"));
    }

    #[test]
    fn scans_diff3_base() {
        let text = "<<<<<<< ours\na\n||||||| base\nb\n=======\nc\n>>>>>>> theirs\n";
        let blocks = scan_text_for_conflicts(text);
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            blocks[0].base_lines.as_ref().unwrap(),
            &vec!["b".to_string()]
        );
        assert_eq!(blocks[0].ours_lines, vec!["a".to_string()]);
        assert_eq!(blocks[0].theirs_lines, vec!["c".to_string()]);
    }

    #[test]
    fn ignores_markers_not_at_column_zero_style() {
        // Prefix must be exact; `<<<<<<<x` without space is not a marker.
        let text = "code // <<<<<<< not a marker\nreal\n";
        assert!(scan_text_for_conflicts(text).is_empty());
    }

    #[test]
    fn parse_uri_forms() {
        assert_eq!(
            parse_conflict_uri("conflict://").unwrap(),
            ConflictUri::List
        );
        assert_eq!(
            parse_conflict_uri("conflict://*").unwrap(),
            ConflictUri::All
        );
        assert_eq!(
            parse_conflict_uri("conflict://3").unwrap(),
            ConflictUri::One { id: 3, scope: None }
        );
        assert_eq!(
            parse_conflict_uri("conflict://3/theirs").unwrap(),
            ConflictUri::One {
                id: 3,
                scope: Some(ConflictScope::Theirs)
            }
        );
        assert_eq!(
            parse_conflict_uri("src/a.rs:conflict://2").unwrap(),
            ConflictUri::One { id: 2, scope: None }
        );
    }

    #[test]
    fn register_and_splice_theirs() {
        let _env = crate::tests::env_lock();
        clear_conflicts_for_test();
        let text = "head\n<<<<<<< HEAD\nours line\n=======\ntheirs line\n>>>>>>> branch\ntail\n";
        let blocks = scan_text_for_conflicts(text);
        let entries = register_conflicts("f.rs", &blocks);
        assert_eq!(entries.len(), 1);
        let id = entries[0].id;
        let entry = get_conflict(id).unwrap();
        let repl = resolve_replacement(&entry, "@theirs").unwrap();
        let out = splice_conflict(text, &entry, &repl).unwrap();
        assert_eq!(out, "head\ntheirs line\ntail\n");
        clear_conflicts_for_test();
    }

    #[test]
    fn splice_custom_body() {
        let _env = crate::tests::env_lock();
        clear_conflicts_for_test();
        let text = "<<<<<<<\nA\n=======\nB\n>>>>>>>\n";
        let blocks = scan_text_for_conflicts(text);
        let e = register_conflicts("x", &blocks)[0].clone();
        let out = splice_conflict(text, &e, "C\n").unwrap();
        assert_eq!(out, "C\n");
        clear_conflicts_for_test();
    }
}
