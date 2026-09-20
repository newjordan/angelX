//! Path safety, output caps, and context-window management.

use super::*;

/// One resolved security boundary shared by a registry and its path/Git tools.
/// The lexical spelling remains the operator-facing workspace, while the
/// canonical root is the containment boundary and `repository` is the stable
/// worktree-aware identity used by Git and persisted state.
#[derive(Clone, Debug)]
pub(crate) struct WorkspaceBoundary {
    pub(crate) lexical_root: PathBuf,
    pub(crate) canonical_root: PathBuf,
    pub(crate) repository: crate::workspace_store::RepoIdentity,
}

impl WorkspaceBoundary {
    #[cfg(test)]
    pub(crate) fn new(root: &Path) -> Self {
        let lexical_root = root.to_path_buf();
        let canonical_root = root.canonicalize().unwrap_or_else(|_| lexical_root.clone());
        let repository = crate::workspace_store::repo_identity(root);
        Self {
            lexical_root,
            canonical_root,
            repository,
        }
    }

    pub(crate) fn cached(root: &Path) -> Self {
        type BoundaryCache = std::sync::Mutex<HashMap<PathBuf, WorkspaceBoundary>>;
        static CACHE: std::sync::OnceLock<BoundaryCache> = std::sync::OnceLock::new();
        let cache = CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
        if let Ok(boundaries) = cache.lock()
            && let Some(boundary) = boundaries.get(root)
        {
            return boundary.clone();
        }
        // A missing root may be created later through a symlinked ancestor.
        // Cache only a boundary resolved now, never the lexical error fallback.
        // Existing cache entries above remain pinned even if an alias retargets.
        let resolved = root.canonicalize();
        let cacheable = resolved.is_ok();
        let boundary = Self {
            lexical_root: root.to_path_buf(),
            canonical_root: resolved.unwrap_or_else(|_| root.to_path_buf()),
            repository: crate::workspace_store::repo_identity(root),
        };
        if cacheable && let Ok(mut boundaries) = cache.lock() {
            return boundaries
                .entry(root.to_path_buf())
                .or_insert(boundary)
                .clone();
        }
        boundary
    }

    pub(crate) fn safe_path(&self, supplied: &str) -> Result<PathBuf, String> {
        let relative = workspace_relative_with_boundary(self, Path::new(supplied))?;
        let resolved = resolve_with_missing_suffix(&self.lexical_root.join(relative))?;
        if !resolved.starts_with(&self.canonical_root) {
            return Err(format!("path resolves outside the workspace: {supplied:?}"));
        }
        Ok(resolved)
    }

    pub(crate) fn confined_git_path(&self, supplied: &str) -> Result<String, String> {
        self.safe_path(supplied)?
            .strip_prefix(&self.canonical_root)
            .map(|path| path.to_string_lossy().into_owned())
            .map_err(|_| format!("path resolves outside the workspace: {supplied:?}"))
    }

    /// Resolve one mutation target to the same canonical parent spelling used
    /// by descriptor-confined writes while retaining the final component.
    /// Unknown parents fail closed so their writes serialize rather than race
    /// a symlink that appears after scheduling.
    pub(crate) fn mutation_footprint_path(&self, supplied: &str) -> Result<PathBuf, String> {
        let relative = workspace_relative_with_boundary(self, Path::new(supplied))?;
        let name = relative
            .file_name()
            .ok_or_else(|| "mutation path names the workspace directory".to_string())?;
        let parent = relative.parent().unwrap_or_else(|| Path::new(""));
        let resolved_parent = self
            .lexical_root
            .join(parent)
            .canonicalize()
            .map_err(|e| format!("resolve mutation parent {}: {e}", parent.display()))?;
        if !resolved_parent.is_dir() || !resolved_parent.starts_with(&self.canonical_root) {
            return Err(format!(
                "mutation parent resolves outside the workspace: {supplied:?}"
            ));
        }
        let mut resolved = resolved_parent;
        resolved.push(name);
        resolved
            .strip_prefix(&self.canonical_root)
            .map(Path::to_path_buf)
            .map_err(|_| format!("path resolves outside the workspace: {supplied:?}"))
    }

    /// Resolve a scoped read for scheduler comparisons. Failures become broad
    /// reads at the footprint layer, preserving a conservative conflict gate.
    pub(crate) fn read_footprint_path(&self, supplied: &str) -> Result<PathBuf, String> {
        self.safe_path(supplied)?
            .strip_prefix(&self.canonical_root)
            .map(Path::to_path_buf)
            .map_err(|_| format!("path resolves outside the workspace: {supplied:?}"))
    }
}

// ---------------------------------------------------------------------------
// File tools — read / write / str_replace / list_dir, confined to a root.
// First-class file editing (vs. fragile shell heredocs) is the foundation for
// clean software-dev RL trajectories. Adapted from the Read/Write/Edit toolset
// every serious coding agent ships (Claude Code, aider, OpenHands). Paths may
// be relative or absolute, are lexically normalized, then resolved through
// existing symlinks, and may never escape `root`.
// ---------------------------------------------------------------------------

/// Convert a model-supplied path to a normalized workspace-relative spelling.
/// Absolute paths are accepted only when they are component-wise beneath the
/// configured root (or its canonical spelling). Keeping the original suffix
/// rather than canonicalizing it here is intentional: the descriptor-confined
/// mutation layer must still see and reject a final symlink.
pub(crate) fn workspace_relative(root: &Path, path: &Path) -> Result<PathBuf, String> {
    workspace_relative_with_boundary(&WorkspaceBoundary::cached(root), path)
}

fn workspace_relative_with_boundary(
    boundary: &WorkspaceBoundary,
    path: &Path,
) -> Result<PathBuf, String> {
    let candidate = if path.is_absolute() {
        match path.strip_prefix(&boundary.lexical_root) {
            Ok(relative) => relative.to_path_buf(),
            Err(_) => path
                .strip_prefix(&boundary.canonical_root)
                .map(Path::to_path_buf)
                .map_err(|_| {
                    format!(
                        "absolute path is outside the workspace {}: {:?}",
                        boundary.canonical_root.display(),
                        path
                    )
                })?,
        }
    } else {
        path.to_path_buf()
    };
    let stack = lexical_normalize(&candidate)
        .ok_or_else(|| format!("path escapes the workspace: {:?}", path))?;
    Ok(stack.into_iter().collect())
}

/// Resolve `rel` against `root`, rejecting lexical escapes and existing
/// symlinks that resolve outside the workspace. Absolute paths are accepted
/// only when they name a path inside the workspace. For a write target
/// that does not exist yet, canonicalize its nearest existing ancestor and then
/// append the missing suffix before checking containment.
pub(crate) fn safe_path(root: &Path, rel: &str) -> Result<PathBuf, String> {
    WorkspaceBoundary::cached(root).safe_path(rel)
}

/// Canonicalize the longest existing prefix of `path`, preserving any missing
/// final components. A dangling symlink counts as an existing prefix but cannot
/// be canonicalized, so it is rejected rather than treated as a safe new path.
fn resolve_with_missing_suffix(path: &Path) -> Result<PathBuf, String> {
    let mut cursor = path.to_path_buf();
    let mut missing = Vec::<OsString>::new();
    loop {
        match std::fs::symlink_metadata(&cursor) {
            Ok(_) => {
                let mut resolved = cursor
                    .canonicalize()
                    .map_err(|e| format!("resolve path {}: {e}", cursor.display()))?;
                for component in missing.iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let component = cursor.file_name().ok_or_else(|| {
                    format!("cannot resolve path ancestor for {}", path.display())
                })?;
                missing.push(component.to_os_string());
                cursor = cursor
                    .parent()
                    .ok_or_else(|| format!("cannot resolve path ancestor for {}", path.display()))?
                    .to_path_buf();
            }
            Err(e) => return Err(format!("resolve path {}: {e}", cursor.display())),
        }
    }
}

pub(crate) fn cap_lines(s: &str, max: usize) -> String {
    let lines: Vec<&str> = s.lines().collect();
    if lines.len() > max {
        format!("{}\n…[truncated at {max} matches]", lines[..max].join("\n"))
    } else {
        s.trim_end().to_string()
    }
}

/// Read an env var as a `usize`, falling back to `default`.
pub(crate) fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .unwrap_or(default)
}

/// Truthy env flag with an explicit default (`0`/`false`/`no`/`off`/empty = off).
pub(crate) fn env_flag(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            !(v.is_empty() || v == "0" || v == "false" || v == "no" || v == "off")
        }
        Err(_) => default,
    }
}

/// Universal ceiling on a tool result before it enters the conversation. Tools
/// truncate their own output, but several (`git_diff`, `find_files`, `defs`,
/// `cargo`/`lint`/`check`, `delegate`/`integrate`, MCP/peer results) don't — so
/// this is the last line of defense the model never sees past, the same
/// dispatch-level cap Codex (`tool_output`) and Hermes (`max_bytes`/`max_lines`)
/// ship. Env-tunable via `ANGEL_TOOL_OUTPUT_MAX_BYTES` / `_MAX_LINES` (either
/// `0` disables that dimension); defaults are generous so per-tool caps (grep
/// 300 lines, `tail` 1500 B, outline 400) pass through untouched.
#[cfg(test)]
pub(crate) fn cap_tool_output(s: &str, window: Option<usize>) -> String {
    let (max_bytes, max_lines) = tool_output_limits(window);
    cap_text(s, max_bytes, max_lines)
}

/// Owned variant for the turn loop, which already holds the tool result. The
/// overwhelmingly common small result returns its original allocation instead
/// of cloning it before discovering that no cap is needed.
pub(crate) fn cap_tool_output_owned(s: String, window: Option<usize>) -> String {
    let (max_bytes, max_lines) = tool_output_limits(window);
    cap_text_owned(s, max_bytes, max_lines)
}

/// Env-backed tool-output ceilings. Production caches the first parse for the
/// process lifetime (A7: multi-tool hops used to re-read two env vars per
/// result). Tests re-read so `EnvGuard` / `TestEnvGuard` stay honest.
fn tool_output_env_ceilings() -> (usize, usize) {
    #[cfg(not(test))]
    {
        static CACHED: std::sync::OnceLock<(usize, usize)> = std::sync::OnceLock::new();
        *CACHED.get_or_init(|| {
            (
                env_usize("ANGEL_TOOL_OUTPUT_MAX_BYTES", 64 * 1024),
                env_usize("ANGEL_TOOL_OUTPUT_MAX_LINES", 800),
            )
        })
    }
    #[cfg(test)]
    {
        (
            env_usize("ANGEL_TOOL_OUTPUT_MAX_BYTES", 64 * 1024),
            env_usize("ANGEL_TOOL_OUTPUT_MAX_LINES", 800),
        )
    }
}

fn tool_output_limits(window: Option<usize>) -> (usize, usize) {
    let (env_bytes, max_lines) = tool_output_env_ceilings();
    let mut max_bytes = env_bytes;
    // A single tool output must never dominate the context. Cap it to ~1/3 of the
    // model's window (bytes ≈ tokens × 4) so one huge read/grep can't crowd out
    // the conversation — decisive on small local windows, where the flat 64 KB
    // cap alone is larger than the whole window.
    if let Some(w) = window
        && w > 0
    {
        max_bytes = max_bytes.min(((w / 3) * 4).max(2048));
    }
    (max_bytes, max_lines)
}

/// Head-then-tail truncation to `max_bytes` / `max_lines` (either `0` = no cap
/// on that dimension). Keeps the start (summary/headers) and the end (totals/
/// final errors) and elides the middle with a marker stating how much was
/// dropped, so the model can narrow its next call rather than guess. Split apart
/// from [`cap_tool_output`] so tests pass explicit limits and stay env-free.
pub(crate) fn cap_text(s: &str, max_bytes: usize, max_lines: usize) -> String {
    let mut out = s.to_string();
    // Line cap first: keep 70% head / 30% tail of the allowance.
    if max_lines > 0 {
        let lines: Vec<&str> = out.lines().collect();
        if lines.len() > max_lines {
            let head = (max_lines * 7 / 10).max(1);
            let tail = max_lines.saturating_sub(head).max(1);
            let dropped = lines.len() - head - tail;
            out = format!(
                "{}\n…[{dropped} middle line(s) elided — {} of {} lines shown]\n{}",
                lines[..head].join("\n"),
                head + tail,
                lines.len(),
                lines[lines.len() - tail..].join("\n"),
            );
        }
    }
    // Byte cap second: same head/tail split, on char boundaries.
    if max_bytes > 0 && out.len() > max_bytes {
        let total = out.len();
        let head_budget = (max_bytes * 7 / 10).max(1);
        let tail_budget = max_bytes.saturating_sub(head_budget).max(1);
        let mut head_end = head_budget.min(total);
        while head_end > 0 && !out.is_char_boundary(head_end) {
            head_end -= 1;
        }
        let mut tail_start = total - tail_budget.min(total);
        while tail_start < total && !out.is_char_boundary(tail_start) {
            tail_start += 1;
        }
        if tail_start > head_end {
            let dropped = tail_start - head_end;
            out = format!(
                "{}\n…[{dropped} middle byte(s) elided — ~{} of {} bytes shown]\n{}",
                &out[..head_end],
                head_end + (total - tail_start),
                total,
                &out[tail_start..],
            );
        }
    }
    out
}

/// [`cap_text`] with an owned fast path. `str::lines().nth(max_lines)` detects
/// a line overage without allocating the complete line-index vector used by the
/// truncation path, so normal concise tool receipts stay zero-copy.
pub(crate) fn cap_text_owned(s: String, max_bytes: usize, max_lines: usize) -> String {
    let too_many_lines = max_lines > 0 && s.lines().nth(max_lines).is_some();
    if (max_bytes == 0 || s.len() <= max_bytes) && !too_many_lines {
        return s;
    }
    cap_text(&s, max_bytes, max_lines)
}

/// Marker placed on a recent tool result only when the aggregate request would
/// otherwise exceed the active context budget. It is distinct from normal tool
/// aging: this emergency path may trim the protected tail, but keeps the oldest
/// evidence first and tells the model exactly how to recover it.
pub(crate) const TOOL_CONTEXT_FIT_MARK: &str = "…[tool output elided for context fit";
const TOOL_CONTEXT_FIT_MIN: &str = "[tool output elided; re-run]";

fn context_fit_already_elided(content: &str) -> bool {
    content.contains(TOOL_CONTEXT_FIT_MARK) || content == TOOL_CONTEXT_FIT_MIN
}

/// Shrink the oldest remaining tool outputs just enough for the *aggregate*
/// conversation plus schemas to fit `budget`. Per-tool caps alone cannot make
/// this guarantee: four recent results, each legal in isolation, can exceed a
/// small model's whole window. Returns the number of changed tool messages.
///
/// This deliberately never changes tool-call pairing, user text, system prompt,
/// or assistant calls. If those non-tool messages alone exceed the budget, the
/// function leaves the whole history intact rather than destroying useful
/// evidence for an unattainable target. This target is not a provider hard-cap
/// admission check; callers must still enforce their actual request limits.
pub(crate) fn fit_tool_results_to_budget(
    history: &mut [ChatMsg],
    budget: usize,
    tools: &[ToolDef],
) -> usize {
    let _span = super::turn::background::span("aging_ms");
    let result = fit_tool_results_to_budget_measured(history, budget, tools);
    super::turn::background::observe(history);
    result
}

fn fit_tool_results_to_budget_measured(
    history: &mut [ChatMsg],
    budget: usize,
    tools: &[ToolDef],
) -> usize {
    if budget == 0 {
        return 0;
    }
    let tool_tokens = estimate_tool_tokens(tools);
    let mut history_chars = estimate_history_chars(history);
    let mut used = history_chars / 4 + tool_tokens;
    if used <= budget {
        return 0;
    }
    // Do not erase fresh evidence when even replacing every tool result with
    // its minimum marker cannot fit. Protected instructions and schemas must
    // remain intact; deleting proof would merely invite an equally futile rerun.
    let removable_chars: usize = history
        .iter()
        .filter(|message| {
            message.role == ChatRole::Tool && !context_fit_already_elided(&message.content)
        })
        .map(|message| {
            message
                .content
                .len()
                .saturating_sub(TOOL_CONTEXT_FIT_MIN.len())
        })
        .sum();
    let minimum_tokens = history_chars.saturating_sub(removable_chars) / 4 + tool_tokens;
    if minimum_tokens > budget {
        return 0;
    }
    let mut changed = 0;
    for message in history.iter_mut() {
        if used <= budget {
            break;
        }
        let (is_tool, content_len) = (message.role == ChatRole::Tool, message.content.len());
        if !is_tool
            || content_len <= TOOL_CONTEXT_FIT_MIN.len()
            || context_fit_already_elided(&message.content)
        {
            continue;
        }
        // Token estimation floors at /4 and the marker has framing overhead;
        // over-remove a small amount then update the exact character count
        // before touching another result. This keeps the policy correct
        // without rescanning the complete history for every message.
        let remove_bytes = used
            .saturating_sub(budget)
            .saturating_mul(4)
            .saturating_add(32);
        let target = content_len
            .saturating_sub(remove_bytes)
            .max(TOOL_CONTEXT_FIT_MIN.len());
        let replacement = elide_tool_result_for_context(&message.content, target);
        if replacement.len() < content_len {
            history_chars = history_chars.saturating_sub(content_len - replacement.len());
            message.content = replacement.into();
            used = history_chars / 4 + tool_tokens;
            changed += 1;
        }
    }
    changed
}

/// Shared estimate so all context-fit checks count the same thing.
pub(crate) fn context_tokens(history: &[ChatMsg], tools: &[ToolDef]) -> usize {
    estimate_tokens(history) + estimate_tool_tokens(tools)
}

fn elide_tool_result_for_context(content: &str, max_bytes: usize) -> String {
    if max_bytes <= TOOL_CONTEXT_FIT_MIN.len() {
        return TOOL_CONTEXT_FIT_MIN.to_string();
    }
    let marker = format!(
        "{TOOL_CONTEXT_FIT_MARK} ({} bytes) — re-run the tool if needed]",
        content.len()
    );
    if marker.len() >= max_bytes {
        return TOOL_CONTEXT_FIT_MIN.to_string();
    }
    let mut head_end = max_bytes
        .saturating_sub(marker.len() + 1)
        .min(content.len());
    while head_end > 0 && !content.is_char_boundary(head_end) {
        head_end -= 1;
    }
    if head_end == 0 {
        marker
    } else {
        format!("{}\n{marker}", &content[..head_end])
    }
}

/// Evict the oldest complete turns when the conversation outgrows `max_msgs`, so
/// an unbounded multi-day loop can't grow history (and the request body) without
/// bound — the hygiene cap Hermes ships as `hygiene_hard_message_limit`. Opt-in
/// via `ANGEL_HISTORY_MAX_MSGS` (`0` = unbounded, the default). Preserves every
/// leading system message (the prompt / pinned context), the current
/// replaceable Harness turn context, the most recent messages, and tool-call
/// *pairing*: the kept suffix never begins on a `Tool` result orphaned from the
/// assistant `tool_calls` that requested it. A no-op unless it can drop a clean
/// prefix without gutting the system preamble.
pub(crate) fn prune_history(history: &mut Vec<ChatMsg>, max_msgs: usize) {
    let _span = super::turn::background::span("other_ms");
    prune_history_measured(history, max_msgs);
    super::turn::background::observe(history);
}

fn prune_history_measured(history: &mut Vec<ChatMsg>, max_msgs: usize) {
    if max_msgs == 0 || history.len() <= max_msgs {
        return;
    }
    // Leading system messages are pinned — never evict the prompt.
    let sys_end = history
        .iter()
        .position(|m| m.role != ChatRole::System)
        .unwrap_or(history.len());
    let keep_tail = max_msgs.saturating_sub(sys_end);
    if keep_tail == 0 {
        return; // cap is smaller than the system preamble — refuse to gut it.
    }
    let mut cut = history.len() - keep_tail;
    if cut <= sys_end {
        return; // nothing to drop past the system preamble.
    }
    // Don't let the kept suffix begin on an orphaned tool result: advance the
    // cut past any leading tool messages whose assistant turn is being dropped.
    while cut < history.len() && history[cut].role == ChatRole::Tool {
        cut += 1;
    }
    if cut >= history.len() || cut <= sys_end {
        return; // no safe boundary found — leave history intact.
    }

    // Reserve explicit operator contracts and the turn context using the same
    // anchor machinery as compaction; ordinary transcript prose remains prunable.
    // Moving the cut may move another anchor into the removed prefix, so settle
    // the reservation before mutating history. If protection leaves no live
    // tail, decline this optional pruning pass rather than discard a contract.
    loop {
        let anchors = compaction_task_anchors(history, sys_end, cut, usize::MAX)
            .into_iter()
            .filter(|anchor| operator_directive(anchor))
            .collect::<Vec<_>>();
        let turn_context = compaction_turn_context_anchor(history, sys_end, cut);
        let reserved = sys_end + anchors.len() + usize::from(turn_context.is_some());
        let keep_tail = max_msgs.saturating_sub(reserved);
        if keep_tail == 0 {
            return;
        }
        let mut next_cut = cut.max(history.len().saturating_sub(keep_tail));
        while next_cut < history.len() && history[next_cut].role == ChatRole::Tool {
            next_cut += 1;
        }
        if next_cut >= history.len() {
            return;
        }
        if next_cut != cut {
            cut = next_cut;
            continue;
        }
        history.splice(
            sys_end..cut,
            anchors.into_iter().map(ChatMsg::user).chain(turn_context),
        );
        break;
    }
}

/// Cheap token estimate for a message slice — char count / 4, the standard rough
/// proxy when no tokenizer is on hand. Counts content, tool-call args, and a
/// small per-message overhead for role/structure framing.
///
/// JSON values are streamed into a byte counter instead of materialized with
/// `Value::to_string()`. Tool arguments can contain multi-megabyte payloads
/// (including encoded media), and request budgeting runs more than once around
/// a model hop. Counting the exact compact JSON bytes avoids cloning that whole
/// payload onto the first-output critical path.
pub(crate) fn json_serialized_len(value: &serde_json::Value) -> usize {
    // Fast paths for the common tool-arg / schema shapes. Must match compact
    // serde_json output exactly (tests pin this).
    match value {
        serde_json::Value::Null => return 4,        // null
        serde_json::Value::Bool(true) => return 4,  // true
        serde_json::Value::Bool(false) => return 5, // false
        serde_json::Value::String(s)
            // Unescaped ASCII strings (the usual case for paths/names) are
            // `2 + len`. Anything that needs JSON escapes falls through.
            if s.bytes().all(|b| match b {
                0x20..=0x21 | 0x23..=0x5b | 0x5d..=0x7e => true, // printable ASCII except " and \
                _ => false,
            }) => {
                return 2 + s.len();
            }
        serde_json::Value::Array(items) if items.is_empty() => return 2, // []
        serde_json::Value::Object(map) if map.is_empty() => return 2,    // {}
        _ => {}
    }

    #[derive(Default)]
    struct ByteCounter(usize);

    impl std::io::Write for ByteCounter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.saturating_add(bytes.len());
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut counter = ByteCounter::default();
    // ByteCounter is infallible, and serde_json::Value always serializes.
    serde_json::to_writer(&mut counter, value)
        .expect("serializing a JSON value into an infallible byte counter");
    counter.0
}

fn estimate_history_chars(history: &[ChatMsg]) -> usize {
    #[cfg(test)]
    HISTORY_CHAR_SCAN_PROBE.with(|probe| {
        if let Some(scans) = probe.get() {
            probe.set(Some(scans.saturating_add(1)));
        }
    });
    let mut chars = 0usize;
    for m in history {
        chars += m.content.len();
        for c in m.tool_calls.iter() {
            chars += c.name.len() + json_serialized_len(&c.args);
        }
        chars += 8; // role + framing overhead
    }
    chars
}

#[cfg(test)]
std::thread_local! {
    static HISTORY_CHAR_SCAN_PROBE: std::cell::Cell<Option<usize>> = const {
        std::cell::Cell::new(None)
    };
}

#[cfg(test)]
pub(crate) fn count_history_char_scans<T>(run: impl FnOnce() -> T) -> (T, usize) {
    HISTORY_CHAR_SCAN_PROBE.with(|probe| {
        let prior = probe.replace(Some(0));
        let result = run();
        let scans = probe.replace(prior).unwrap_or(0);
        (result, scans)
    })
}

pub(crate) fn estimate_tokens(history: &[ChatMsg]) -> usize {
    estimate_history_chars(history) / 4
}

/// Rolling history token estimate for the multi-hop turn loop. Reuses the
/// prior count when the history only grew by appends (tool results, steers);
/// full recompute when earlier messages may have been rewritten (aging,
/// compact, shrink, prune, context-fit).
#[derive(Clone, Debug, Default)]
pub(crate) struct HistoryTokenRoll {
    tokens: usize,
    /// Per-message content+tool-shape hashes for the prefix already counted.
    msg_hashes: Vec<u64>,
}

fn message_token_hash(m: &ChatMsg) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    m.content.hash(&mut hasher);
    m.tool_calls.len().hash(&mut hasher);
    for call in m.tool_calls.iter() {
        call.name.hash(&mut hasher);
        // Args can be huge; length + a short prefix is enough to catch rewrites
        // without re-serializing multi-megabyte payloads on every hop.
        json_serialized_len(&call.args).hash(&mut hasher);
        if let Some(s) = call.args.as_str() {
            s.len().hash(&mut hasher);
            s.as_bytes()
                .get(..32.min(s.len()))
                .unwrap_or_default()
                .hash(&mut hasher);
        }
    }
    hasher.finish()
}

impl HistoryTokenRoll {
    /// Full recompute — call after any rewrite of earlier messages.
    pub(crate) fn recompute(&mut self, history: &[ChatMsg]) -> usize {
        self.tokens = estimate_tokens(history);
        self.msg_hashes = history.iter().map(message_token_hash).collect();
        self.tokens
    }

    /// Observe history that may have only grown. If the previous prefix hashes
    /// still match, only the new tail is estimated and added — the common hop
    /// path after tool results land.
    pub(crate) fn observe(&mut self, history: &[ChatMsg]) -> usize {
        let n = history.len();
        if n == 0 {
            *self = Self::default();
            return 0;
        }
        let prefix = self.msg_hashes.len();
        // Pure append: every previously counted message is still byte-identical
        // by content hash.
        if n > prefix
            && prefix > 0
            && history[..prefix]
                .iter()
                .zip(self.msg_hashes.iter())
                .all(|(m, h)| message_token_hash(m) == *h)
        {
            self.tokens = self
                .tokens
                .saturating_add(estimate_tokens(&history[prefix..]));
            self.msg_hashes
                .extend(history[prefix..].iter().map(message_token_hash));
            return self.tokens;
        }
        // Unchanged snapshot.
        if n == prefix
            && history
                .iter()
                .zip(self.msg_hashes.iter())
                .all(|(m, h)| message_token_hash(m) == *h)
        {
            return self.tokens;
        }
        self.recompute(history)
    }
}

/// Rough token cost of the tool schemas advertised on every request (name +
/// description + JSON-schema params). These ride along with each call and can be
/// a large, otherwise-invisible slice of the context window, so compaction must
/// budget for them too. Same char/4 proxy as [`estimate_tokens`].
pub(crate) fn estimate_tool_tokens(tools: &[ToolDef]) -> usize {
    if tools.is_empty() {
        return 0;
    }
    let bytes: usize = tools
        .iter()
        .map(|t| t.name.len() + t.description.len() + json_serialized_len(&t.params) + 8)
        .sum();
    bytes / 4
}

/// `get_context_remaining` — report how much of the token budget is left, so the
/// model can pace itself (wrap up, or compact) before it runs out. Reads the
/// shared gauge `run_turn` updates each hop. With no budget configured it reports
/// usage only.
pub(crate) struct ContextTool {
    pub(crate) gauge: Arc<ContextGauge>,
}

impl ContextTool {
    pub(crate) fn new(gauge: Arc<ContextGauge>) -> Self {
        Self { gauge }
    }
}

impl Tool for ContextTool {
    fn name(&self) -> &str {
        "get_context_remaining"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "get_context_remaining".to_string(),
            description: "Report the conversation's estimated token usage and how much of the \
                          configured budget remains, so you can decide whether to wrap up or \
                          keep going. Takes no arguments."
                .to_string(),
            params: serde_json::json!({ "type": "object", "properties": {} }),
        }
    }
    fn call(&self, _args: &Value) -> Result<String, String> {
        let used = self.gauge.used_tokens.load(Ordering::Relaxed);
        let budget = self.gauge.budget_tokens.load(Ordering::Relaxed);
        if budget == 0 {
            Ok(format!(
                "~{used} tokens used so far. No context budget is set \
                 (ANGEL_CONTEXT_BUDGET_TOKENS=0), so there's no hard limit."
            ))
        } else {
            let remaining = budget.saturating_sub(used);
            let pct = (used as f64 / budget as f64 * 100.0).round() as u64;
            Ok(format!(
                "~{used} of ~{budget} budget tokens used ({pct}%), ~{remaining} remaining. \
                 Older turns auto-compact above the budget."
            ))
        }
    }
}

#[cfg(all(test, unix))]
mod alias_tests;
