//! Read/write footprints and the parallel-safe batch gate.

use super::*;

pub(crate) fn is_parallel_safe(name: &str) -> bool {
    matches!(
        name,
        "read_file"
            | "grep"
            | "find_files"
            | "file_search"
            | "semantic_read"
            | "web_search"
            | "outline"
            | "defs"
            | "list_dir"
            | "git_diff"
            | "git_status"
            | "git_log"
            | "lsp_diagnostics"
            | "lsp_definition"
            | "lsp_references"
            | "lsp_hover"
            | "lsp_symbols"
            | "lsp_workspace_symbol"
            | "word_count"
            | "benchmark_compare"
            | "reverse"
            | "skill"
            | "get_context_remaining"
            | "tool_search"
    )
}

pub(crate) fn is_code_mode_repo_read(name: &str) -> bool {
    matches!(
        name,
        "read_file"
            | "grep"
            | "find_files"
            | "file_search"
            | "outline"
            | "defs"
            | "list_dir"
            | "git_diff"
            | "git_status"
            | "git_log"
            | "word_count"
            | "benchmark_compare"
            | "reverse"
            | "get_context_remaining"
    )
}

#[derive(Debug, Clone)]
pub(crate) enum Footprint {
    Read { path: Option<PathBuf> },
    Write { path: PathBuf },
    Effect,
}

pub(crate) fn lexical_normalize(p: &Path) -> Option<Vec<std::ffi::OsString>> {
    let mut stack: Vec<std::ffi::OsString> = Vec::new();
    for comp in p.components() {
        match comp {
            std::path::Component::Normal(c) => stack.push(c.to_os_string()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                stack.pop()?;
            }
            _ => return None,
        }
    }
    Some(stack)
}

pub(crate) fn norm_path(s: &str) -> PathBuf {
    let trimmed = s.trim();
    match lexical_normalize(Path::new(trimmed)) {
        Some(stack) => {
            let mut p = PathBuf::new();
            for c in stack {
                p.push(c);
            }
            p
        }
        None => PathBuf::from(trimmed),
    }
}

#[derive(Clone, Copy)]
enum FootprintClass {
    ScopedRead,
    ScopedWrite,
    BroadRead,
    Effect,
}

fn classify_footprint(name: &str, allow_writes: bool) -> FootprintClass {
    match name {
        "read_file" => FootprintClass::ScopedRead,
        "write_file" | "str_replace" | "multi_edit" if allow_writes => FootprintClass::ScopedWrite,
        _ if is_parallel_safe(name) => FootprintClass::BroadRead,
        _ => FootprintClass::Effect,
    }
}

fn footprint_with(
    name: &str,
    args: &Value,
    allow_writes: bool,
    resolve_read: impl FnOnce(&str) -> Option<PathBuf>,
    resolve_write: impl FnOnce(&str) -> Option<PathBuf>,
) -> Footprint {
    match classify_footprint(name, allow_writes) {
        FootprintClass::ScopedRead => Footprint::Read {
            path: args
                .get("path")
                .and_then(Value::as_str)
                .and_then(resolve_read),
        },
        FootprintClass::ScopedWrite => match args
            .get("path")
            .and_then(Value::as_str)
            .and_then(resolve_write)
        {
            Some(path) => Footprint::Write { path },
            None => Footprint::Effect,
        },
        FootprintClass::BroadRead => Footprint::Read { path: None },
        FootprintClass::Effect => Footprint::Effect,
    }
}

pub(crate) fn footprint(name: &str, args: &Value, allow_writes: bool) -> Footprint {
    let lexical = |supplied: &str| {
        let path = norm_path(supplied);
        (!path.is_absolute()).then_some(path)
    };
    footprint_with(name, args, allow_writes, lexical, lexical)
}

fn footprint_in(
    boundary: &WorkspaceBoundary,
    name: &str,
    args: &Value,
    allow_writes: bool,
) -> Footprint {
    footprint_with(
        name,
        args,
        allow_writes,
        |supplied| boundary.read_footprint_path(supplied.trim()).ok(),
        |supplied| boundary.mutation_footprint_path(supplied.trim()).ok(),
    )
}

pub(crate) fn paths_overlap(a: &Path, b: &Path) -> bool {
    a == b || a.starts_with(b) || b.starts_with(a)
}

pub(crate) fn footprints_conflict(a: &Footprint, b: &Footprint) -> bool {
    use Footprint::*;
    match (a, b) {
        (Effect, _) | (_, Effect) => true,
        (Read { .. }, Read { .. }) => false,
        (Read { path: r }, Write { path: w }) | (Write { path: w }, Read { path: r }) => {
            match r {
                None => true,
                Some(p) => paths_overlap(p, w),
            }
        }
        (Write { path: p }, Write { path: q }) => paths_overlap(p, q),
    }
}

#[cfg(test)]
#[derive(Debug, PartialEq)]
pub(crate) enum HopProgress {
    Advanced,
    Reread,
    Neutral,
}

fn shell_inspection_identity(call: &ToolCall) -> Option<String> {
    if call.name != "shell" {
        return None;
    }
    let command = crate::agent::tools::shell::shell_command_arg(&call.args)?.trim();
    if command.is_empty()
        || [";", "&&", "||", ">", "<", "`", "$("]
            .iter()
            .any(|operator| command.contains(operator))
    {
        return None;
    }
    let mut segments = command.split('|');
    let safe = segments.all(|segment| {
        let words = segment.split_whitespace().collect::<Vec<_>>();
        let Some(program) = words
            .first()
            .map(|word| word.rsplit('/').next().unwrap_or(word))
        else {
            return false;
        };
        match program {
            "rg" | "grep" | "ls" | "head" | "tail" | "wc" | "pwd" | "stat" | "file" | "tree"
            | "which" => true,
            "sed" => !words
                .iter()
                .any(|word| *word == "-i" || word.starts_with("-i") || *word == "--in-place"),
            "find" => !words
                .iter()
                .any(|word| matches!(*word, "-delete" | "-exec" | "-execdir")),
            "git" => words.get(1).is_some_and(|subcommand| {
                matches!(*subcommand, "status" | "diff" | "log" | "show")
            }),
            _ => false,
        }
    });
    safe.then(|| {
        let normalized = command.split_whitespace().collect::<Vec<_>>().join(" ");
        format!("shell|{normalized}")
    })
}

pub(crate) fn inspection_identity(call: &ToolCall) -> Option<String> {
    if let Some(identity) = shell_inspection_identity(call) {
        return Some(identity);
    }
    if matches!(call.name.as_str(), "proc_status" | "proc_wait") {
        return Some(format!(
            "proc_status|{}",
            call.args
                .get("id")
                .and_then(|id| id.as_u64())
                .map(|id| id.to_string())
                .unwrap_or_else(|| "all".to_string())
        ));
    }
    match footprint(&call.name, &call.args, true) {
        Footprint::Read { path: Some(path) } => {
            Some(format!("{}|{}", call.name, path.to_string_lossy()))
        }
        Footprint::Read { path: None } => {
            Some(format!("{}|{}", call.name, payload_fingerprint(&call.args)))
        }
        Footprint::Write { .. } | Footprint::Effect => None,
    }
}

pub(crate) fn inspection_identities(calls: &[ToolCall]) -> Vec<String> {
    calls.iter().filter_map(inspection_identity).collect()
}

#[cfg(test)]
pub(crate) fn inspection_identities_for_loop(apply: bool, calls: &[ToolCall]) -> Vec<String> {
    if apply {
        inspection_identities(calls)
    } else {
        Vec::new()
    }
}

#[cfg(test)]
pub(crate) fn classify_hop(
    calls: &[ToolCall],
    seen: &std::collections::HashSet<String>,
) -> HopProgress {
    classify_hop_with_identities(calls, seen, &inspection_identities(calls))
}

#[cfg(test)]
pub(crate) fn classify_hop_with_identities(
    calls: &[ToolCall],
    seen: &std::collections::HashSet<String>,
    identities: &[String],
) -> HopProgress {
    let has_effect = identities.len() < calls.len();
    if has_effect || identities.iter().any(|identity| !seen.contains(identity)) {
        HopProgress::Advanced
    } else if !identities.is_empty() {
        HopProgress::Reread
    } else {
        HopProgress::Neutral
    }
}

#[cfg(test)]
pub(crate) fn churn_classify_applied(churn_stop: usize) -> bool {
    let _ = churn_stop;
    false
}

#[cfg(test)]
pub(crate) fn hop_progress_for_loop(
    apply: bool,
    calls: &[ToolCall],
    seen: &std::collections::HashSet<String>,
    identities: &[String],
) -> HopProgress {
    if apply {
        classify_hop_with_identities(calls, seen, identities)
    } else {
        HopProgress::Neutral
    }
}

pub(crate) fn batch_parallelizable_in(boundary: &WorkspaceBoundary, calls: &[ToolCall]) -> bool {
    batch_parallelizable_in_with(boundary, calls, env_flag("ANGEL_PARALLEL_WRITES", true))
}

pub(crate) fn batch_parallelizable_in_with(
    boundary: &WorkspaceBoundary,
    calls: &[ToolCall],
    allow_writes: bool,
) -> bool {
    if calls.len() <= 1 {
        return false;
    }
    let fps = calls
        .iter()
        .map(|call| footprint_in(boundary, &call.name, &call.args, allow_writes))
        .collect::<Vec<_>>();
    for i in 0..fps.len() {
        for j in (i + 1)..fps.len() {
            if footprints_conflict(&fps[i], &fps[j]) {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
pub(crate) fn batch_parallelizable_with(calls: &[ToolCall], allow_writes: bool) -> bool {
    if calls.len() <= 1 {
        return false;
    }
    let fps: Vec<Footprint> = calls
        .iter()
        .map(|c| footprint(&c.name, &c.args, allow_writes))
        .collect();
    for i in 0..fps.len() {
        for j in (i + 1)..fps.len() {
            if footprints_conflict(&fps[i], &fps[j]) {
                return false;
            }
        }
    }
    true
}

pub(crate) fn batch_segments_in(
    boundary: &WorkspaceBoundary,
    calls: &[ToolCall],
) -> Vec<std::ops::Range<usize>> {
    batch_segments_in_with(boundary, calls, env_flag("ANGEL_PARALLEL_WRITES", true))
}

pub(crate) fn batch_segments_in_with(
    boundary: &WorkspaceBoundary,
    calls: &[ToolCall],
    allow_writes: bool,
) -> Vec<std::ops::Range<usize>> {
    if calls.is_empty() {
        return Vec::new();
    }
    let footprints = calls
        .iter()
        .map(|call| footprint_in(boundary, &call.name, &call.args, allow_writes))
        .collect::<Vec<_>>();
    segment_footprints(&footprints)
}

pub(crate) fn batch_segments_with(
    calls: &[ToolCall],
    allow_writes: bool,
) -> Vec<std::ops::Range<usize>> {
    if calls.is_empty() {
        return Vec::new();
    }
    let footprints = calls
        .iter()
        .map(|call| footprint(&call.name, &call.args, allow_writes))
        .collect::<Vec<_>>();
    segment_footprints(&footprints)
}

fn segment_footprints(footprints: &[Footprint]) -> Vec<std::ops::Range<usize>> {
    if footprints.is_empty() {
        return Vec::new();
    }
    let mut segments = Vec::new();
    let mut start = 0;
    for end in 1..footprints.len() {
        if footprints[start..end]
            .iter()
            .any(|prior| footprints_conflict(prior, &footprints[end]))
        {
            segments.push(start..end);
            start = end;
        }
    }
    segments.push(start..footprints.len());
    segments
}
