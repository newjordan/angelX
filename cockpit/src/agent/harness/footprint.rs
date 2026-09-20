//! Read/write footprints and the parallel-safe batch gate.

use super::*;

/// Read-only tools that are safe to run concurrently when the model batches
/// several in one turn. Anything that writes, execs, or has side effects (shell,
/// cargo, file writes/patches, todo/notes, present, delegate/integrate) stays
/// serial — the conservative parallel-safe gate Codex and Hermes both use.
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

/// Capabilities available inside default read-only code mode. This is a
/// security/capability boundary, deliberately narrower than `is_parallel_safe`:
/// a tool can be concurrency-safe yet still reach the network or another
/// external system. Keep this list limited to deterministic, workspace-confined
/// inspection. LSP is excluded because server startup can execute project-aware
/// behavior; network tools are excluded even though they are read-only.
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

// ---------------------------------------------------------------------------
// Batch parallelism by **footprint** — the Hermes path-overlap refinement.
// Today a batch parallelizes only if every call is read-only-safe. This extends
// it: path-scoped writes (write_file/str_replace/multi_edit) to *disjoint* paths
// can also run concurrently, and a write never runs alongside a read that could
// observe it mid-write. Same conservative gate, just less pessimistic — the
// common "edit 3 unrelated files at once" batch stops being serialized.
// ---------------------------------------------------------------------------

/// What a single tool call touches on the filesystem.
#[derive(Debug, Clone)]
pub(crate) enum Footprint {
    /// A read-only-safe op. `path = None` is a broad read (grep/list_dir/… scan
    /// the tree); `Some` is a single-file read (read_file).
    Read { path: Option<PathBuf> },
    /// A path-scoped write to one file.
    Write { path: PathBuf },
    /// Effectful or unbounded (shell/cargo/apply_patch/delegate/code_mode/…).
    /// Conflicts with everything → forces the batch serial.
    Effect,
}

/// Lexically collapse a relative path's components: drop `.`, resolve `..`
/// against preceding `Normal` components. Returns the normalized component
/// stack, or `None` if the path is absolute or a `..` climbs above the start (an
/// escape). Purely lexical — no filesystem access, no symlink resolution. Shared
/// by `safe_path` (confinement) and `norm_path` (footprint conflict comparison)
/// so both normalize identically.
pub(crate) fn lexical_normalize(p: &Path) -> Option<Vec<std::ffi::OsString>> {
    let mut stack: Vec<std::ffi::OsString> = Vec::new();
    for comp in p.components() {
        match comp {
            std::path::Component::Normal(c) => stack.push(c.to_os_string()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                stack.pop()?; // climbs above the start → escape
            }
            _ => return None, // RootDir / Prefix → absolute / non-relative
        }
    }
    Some(stack)
}

/// Normalize a workspace-relative tool-arg path for stable conflict comparison:
/// trim whitespace, then collapse `.`/`..` exactly as `safe_path` does (so `a/x`
/// and `a/./x`, or `a/b/../x` and `a/x`, resolve equal and a batch won't run two
/// writes to the same file concurrently). An escaping/absolute path can't be
/// normalized; fall back to its trimmed form (it'll be rejected at write time).
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

/// Shared classification for lexical and workspace-bound schedulers.
/// Path resolution is a later, explicit step: BroadRead/Effect never look at
/// `args.path`, and write-off collapses to Effect without a resolver call.
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

/// The filesystem footprint of a call. `allow_writes=false` collapses writes to
/// `Effect`, restoring the old "any mutation → serial" behavior.
pub(crate) fn footprint(name: &str, args: &Value, allow_writes: bool) -> Footprint {
    // This pure helper does not own the workspace root, so it cannot prove that
    // an absolute and relative spelling name different files. Treat absolute
    // reads as broad and absolute writes as effectful/serial. Live scheduling
    // uses `footprint_in` with the registry boundary below.
    let lexical = |supplied: &str| {
        let path = norm_path(supplied);
        (!path.is_absolute()).then_some(path)
    };
    footprint_with(name, args, allow_writes, lexical, lexical)
}

/// Registry-bound footprint used by the actual parallel scheduler. Scoped
/// paths resolve through the same workspace boundary as file tools: reads use
/// their full target and mutations canonicalize only the known parent. A path
/// that cannot be resolved becomes broad/effectful and therefore serial.
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

/// Two paths conflict if they're the same file or one contains the other.
pub(crate) fn paths_overlap(a: &Path, b: &Path) -> bool {
    a == b || a.starts_with(b) || b.starts_with(a)
}

/// Do two footprints conflict (can't safely run concurrently)?
pub(crate) fn footprints_conflict(a: &Footprint, b: &Footprint) -> bool {
    use Footprint::*;
    match (a, b) {
        (Effect, _) | (_, Effect) => true,
        (Read { .. }, Read { .. }) => false,
        (Read { path: r }, Write { path: w }) | (Write { path: w }, Read { path: r }) => {
            match r {
                None => true, // a broad read could observe the write mid-flight
                Some(p) => paths_overlap(p, w),
            }
        }
        (Write { path: p }, Write { path: q }) => paths_overlap(p, q),
    }
}

/// What a single hop did, for the no-progress guard.
#[cfg(test)]
#[derive(Debug, PartialEq)]
pub(crate) enum HopProgress {
    /// Wrote, ran something effectful, or read a file/searched something NEW.
    Advanced,
    /// Only re-read file(s) already read this turn — no write, no new read.
    Reread,
    /// Only broad reads (grep/list_dir/…) or nothing scoped — neither penalized
    /// nor counted as progress (a distinct search IS legitimate exploration).
    Neutral,
}

/// Recognize a deliberately narrow set of shell commands that only inspect the
/// workspace. Shell stays effectful for scheduling and safety; this identity is
/// used solely by the no-progress guard so alternating `sed`/`rg` reads cannot
/// masquerade as productive execution forever.
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

/// Stable identity for one read/search call. Tool-call ids are deliberately
/// excluded: providers routinely mint a fresh id while repeating the same
/// operation. Scoped file reads normalize their path; broad searches include
/// the tool name and arguments so alternating two searches cannot evade the
/// no-progress guard. Writes and effectful calls have no identity.
pub(crate) fn inspection_identity(call: &ToolCall) -> Option<String> {
    if let Some(identity) = shell_inspection_identity(call) {
        return Some(identity);
    }
    // A background-process status check observes existing state; it does
    // not advance the task. Treat the same handle like a repeated file
    // inspection so changing uptime text or log tails cannot masquerade
    // as productive work across hundreds of model hops.
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
        // Broad searches hash args in place. Display-serializing the JSON
        // (grep patterns, globs) was leftover hop-loop tax on every default
        // hop that still records repeated_inspections.
        Footprint::Read { path: None } => {
            Some(format!("{}|{}", call.name, payload_fingerprint(&call.args)))
        }
        Footprint::Write { .. } | Footprint::Effect => None,
    }
}

/// Stable identities for the read/search calls in a batch.
pub(crate) fn inspection_identities(calls: &[ToolCall]) -> Vec<String> {
    calls.iter().filter_map(inspection_identity).collect()
}

/// Identities the hop loop needs for no-progress classify + repeat counts.
/// Default hops (classify off) must not allocate or hash payloads.
#[cfg(test)]
pub(crate) fn inspection_identities_for_loop(apply: bool, calls: &[ToolCall]) -> Vec<String> {
    if apply {
        inspection_identities(calls)
    } else {
        Vec::new()
    }
}

/// Classify a hop against every inspection already performed this turn.
/// Effect/Write or any new read/search advances; a read/search-only batch whose
/// identities have all been seen is a re-read. Pure, so the guard is testable.
#[cfg(test)]
pub(crate) fn classify_hop(
    calls: &[ToolCall],
    seen: &std::collections::HashSet<String>,
) -> HopProgress {
    classify_hop_with_identities(calls, seen, &inspection_identities(calls))
}

/// Same verdict as [`classify_hop`], using identities the hop loop already
/// built. A write/effect call has no inspection identity, so
/// `identities.len() < calls.len()` is the effect signal — no second
/// footprint walk.
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

/// Whether the hop loop should classify no-progress this turn.
/// The 2026-08-17 +++ loop does not apply synthetic reread stops, so
/// default hops must not pay classify just to throw the verdict away.
/// Re-enable by returning `churn_stop > 0` once `hop_progress` drives a
/// nudge/stop again.
#[cfg(test)]
pub(crate) fn churn_classify_applied(churn_stop: usize) -> bool {
    let _ = churn_stop;
    false
}

/// Classify only when the loop will consume the verdict.
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

/// A batch runs in parallel iff it has >1 call and no pair of calls conflicts.
/// `ANGEL_PARALLEL_WRITES=0` disables the disjoint-write extension (reads still
/// parallelize as before).
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

/// Partition one model-emitted batch into maximal consecutive runs whose
/// footprints are pairwise non-conflicting. Segments always execute in order;
/// only a segment with more than one call is eligible for concurrent dispatch.
/// This recovers parallel read/disjoint-write runs on either side of an
/// effectful barrier without allowing later calls to overtake that barrier.
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
