//! Path safety, output caps, and context-window management.

use super::*;

#[derive(Clone, Debug)]
pub(crate) struct WorkspaceBoundary {
    pub(crate) lexical_root: PathBuf,
    pub(crate) canonical_root: PathBuf,
    pub(crate) repository: crate::platform::workspace_store::RepoIdentity,
}

impl WorkspaceBoundary {
    #[cfg(test)]
    pub(crate) fn new(root: &Path) -> Self {
        let lexical_root = root.to_path_buf();
        let canonical_root = root.canonicalize().unwrap_or_else(|_| lexical_root.clone());
        let repository = crate::platform::workspace_store::repo_identity(root);
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
        let resolved = root.canonicalize();
        let cacheable = resolved.is_ok();
        let boundary = Self {
            lexical_root: root.to_path_buf(),
            canonical_root: resolved.unwrap_or_else(|_| root.to_path_buf()),
            repository: crate::platform::workspace_store::repo_identity(root),
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

    pub(crate) fn read_footprint_path(&self, supplied: &str) -> Result<PathBuf, String> {
        self.safe_path(supplied)?
            .strip_prefix(&self.canonical_root)
            .map(Path::to_path_buf)
            .map_err(|_| format!("path resolves outside the workspace: {supplied:?}"))
    }
}

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
        launch_relative_in_workspace(&boundary.lexical_root, path)
            .unwrap_or_else(|| path.to_path_buf())
    };
    let stack = lexical_normalize(&candidate)
        .ok_or_else(|| format!("path escapes the workspace: {:?}", path))?;
    Ok(stack.into_iter().collect())
}

pub(crate) fn launch_relative_in_workspace(lexical_root: &Path, path: &Path) -> Option<PathBuf> {
    if lexical_root.is_absolute() || path.is_absolute() {
        return None;
    }
    let root = lexical_normalize(lexical_root)?;
    let supplied = lexical_normalize(path)?;
    if root.is_empty() || supplied.len() < root.len() || supplied[..root.len()] != root[..] {
        return None;
    }
    if std::fs::symlink_metadata(lexical_root.join(&supplied[0])).is_ok() {
        return None;
    }
    Some(supplied[root.len()..].iter().collect())
}

pub(crate) fn safe_path(root: &Path, rel: &str) -> Result<PathBuf, String> {
    WorkspaceBoundary::cached(root).safe_path(rel)
}

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
        format!(
            "{}\n…[{} max={max}]",
            lines[..max].join("\n"),
            super::book::d467_receipts::MATCH_CAP.cells()
        )
    } else {
        s.trim_end().to_string()
    }
}

pub(crate) fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .unwrap_or(default)
}

pub(crate) fn env_flag(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            !(v.is_empty() || v == "0" || v == "false" || v == "no" || v == "off")
        }
        Err(_) => default,
    }
}

#[cfg(test)]
pub(crate) fn cap_tool_output(s: &str, window: Option<usize>) -> String {
    let (max_bytes, max_lines) = tool_output_limits(window);
    cap_text(s, max_bytes, max_lines)
}

pub(crate) fn cap_tool_output_owned(s: String, window: Option<usize>) -> String {
    let (max_bytes, max_lines) = tool_output_limits(window);
    cap_text_owned(s, max_bytes, max_lines)
}

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
    if let Some(w) = window
        && w > 0
    {
        max_bytes = max_bytes.min((w / 3).saturating_mul(4).max(2048));
    }
    (max_bytes, max_lines)
}

pub(crate) fn cap_text(s: &str, max_bytes: usize, max_lines: usize) -> String {
    let mut out = s.to_string();
    if max_lines > 0 {
        let lines: Vec<&str> = out.lines().collect();
        if lines.len() > max_lines {
            let head = (max_lines * 7 / 10).max(1);
            let tail = max_lines.saturating_sub(head).max(1);
            let dropped = lines.len() - head - tail;
            out = format!(
                "{}\n…[{} dropped={dropped} shown={} total={}]\n{}",
                lines[..head].join("\n"),
                super::book::d467_receipts::MIDDLE_LINES.cells(),
                head + tail,
                lines.len(),
                lines[lines.len() - tail..].join("\n"),
            );
        }
    }
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
                "{}\n…[{} dropped={dropped} shown=~{} total={}]\n{}",
                &out[..head_end],
                super::book::d467_receipts::MIDDLE_BYTES.cells(),
                head_end + (total - tail_start),
                total,
                &out[tail_start..],
            );
        }
    }
    out
}

pub(crate) fn cap_text_owned(s: String, max_bytes: usize, max_lines: usize) -> String {
    let too_many_lines = max_lines > 0 && s.lines().nth(max_lines).is_some();
    if (max_bytes == 0 || s.len() <= max_bytes) && !too_many_lines {
        return s;
    }
    cap_text(&s, max_bytes, max_lines)
}

pub(crate) const TOOL_CONTEXT_FIT_MARK: &str = "…[⡨⠃⠓";
pub(crate) const TOOL_CONTEXT_FIT_MIN: &str = "[⡨⠃⠁ ⠵⠛]";

fn context_fit_already_elided(content: &str) -> bool {
    content.contains(TOOL_CONTEXT_FIT_MARK) || content == TOOL_CONTEXT_FIT_MIN
}

pub(crate) fn fit_tool_results_to_budget(
    history: &mut [ChatMsg],
    budget: usize,
    tools: &[ToolDef],
) -> usize {
    let _span = super::turn::background::span("aging_ms");
    let (result, rewritten) = fit_tool_results_to_budget_measured(history, budget, tools);
    if rewritten {
        super::turn::background::observe(history);
    }
    result
}

fn fit_tool_results_to_budget_measured(
    history: &mut [ChatMsg],
    budget: usize,
    tools: &[ToolDef],
) -> (usize, bool) {
    if budget == 0 {
        return (0, false);
    }
    let tool_tokens = estimate_tool_tokens(tools);
    let mut history_chars = estimate_history_chars(history);
    let mut used = history_chars / 4 + tool_tokens;
    if used <= budget {
        return (0, false);
    }
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
        return (0, false);
    }
    let mut changed = 0;
    let mut rewritten = false;
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
            rewritten = true;
        }
    }
    (changed, rewritten)
}

pub(crate) fn context_tokens(history: &[ChatMsg], tools: &[ToolDef]) -> usize {
    estimate_tokens(history) + estimate_tool_tokens(tools)
}

fn elide_tool_result_for_context(content: &str, max_bytes: usize) -> String {
    if max_bytes <= TOOL_CONTEXT_FIT_MIN.len() {
        return TOOL_CONTEXT_FIT_MIN.to_string();
    }
    let marker = format!(
        "{TOOL_CONTEXT_FIT_MARK} ({} bytes){}",
        content.len(),
        super::compact::AGED_TAIL
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

pub(crate) fn prune_history(history: &mut Vec<ChatMsg>, max_msgs: usize) {
    let _span = super::turn::background::span("other_ms");
    if prune_history_measured(history, max_msgs) {
        super::turn::background::observe(history);
    }
}

fn prune_history_measured(history: &mut Vec<ChatMsg>, max_msgs: usize) -> bool {
    if max_msgs == 0 || history.len() <= max_msgs {
        return false;
    }
    let sys_end = history
        .iter()
        .position(|m| m.role != ChatRole::System)
        .unwrap_or(history.len());
    let keep_tail = max_msgs.saturating_sub(sys_end);
    if keep_tail == 0 {
        return false;
    }
    let mut cut = history.len() - keep_tail;
    if cut <= sys_end {
        return false;
    }
    while cut < history.len() && history[cut].role == ChatRole::Tool {
        cut += 1;
    }
    if cut >= history.len() || cut <= sys_end {
        return false;
    }

    loop {
        let anchors = compaction_task_anchors(history, sys_end, cut, usize::MAX)
            .into_iter()
            .filter(|anchor| operator_directive(anchor))
            .collect::<Vec<_>>();
        let turn_context = compaction_turn_context_anchor(history, sys_end, cut);
        let reserved = sys_end + anchors.len() + usize::from(turn_context.is_some());
        let keep_tail = max_msgs.saturating_sub(reserved);
        if keep_tail == 0 {
            return false;
        }
        let mut next_cut = cut.max(history.len().saturating_sub(keep_tail));
        while next_cut < history.len() && history[next_cut].role == ChatRole::Tool {
            next_cut += 1;
        }
        if next_cut >= history.len() {
            return false;
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
    true
}

pub(crate) fn json_serialized_len(value: &serde_json::Value) -> usize {
    #[cfg(test)]
    JSON_LENGTH_SCAN_PROBE.with(|probe| {
        if let Some(scans) = probe.get() {
            probe.set(Some(scans.saturating_add(1)));
        }
    });
    match value {
        serde_json::Value::Null => return 4,
        serde_json::Value::Bool(true) => return 4,
        serde_json::Value::Bool(false) => return 5,
        serde_json::Value::String(s)
            if s.bytes().all(|b| match b {
                0x20..=0x21 | 0x23..=0x5b | 0x5d..=0x7e => true,
                _ => false,
            }) => {
                return 2 + s.len();
            }
        serde_json::Value::Array(items) if items.is_empty() => return 2,
        serde_json::Value::Object(map) if map.is_empty() => return 2,
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
        chars += private_reasoning_bytes(m);
        for c in m.tool_calls.iter() {
            chars += c.name.len() + json_serialized_len(&c.args);
        }
        chars += 8;
    }
    chars
}

fn private_reasoning_bytes(message: &ChatMsg) -> usize {
    if message.role == ChatRole::Assistant && !message.tool_calls.is_empty() {
        message
            .private_reasoning
            .as_ref()
            .map_or(0, |text| text.len())
            + message
                .responses_replay
                .as_ref()
                .map_or(0, |replay| replay.extra_bytes())
    } else {
        0
    }
}

#[cfg(test)]
std::thread_local! {
    static HISTORY_CHAR_SCAN_PROBE: std::cell::Cell<Option<usize>> = const {
        std::cell::Cell::new(None)
    };
    static JSON_LENGTH_SCAN_PROBE: std::cell::Cell<Option<usize>> = const {
        std::cell::Cell::new(None)
    };
}

#[cfg(test)]
pub(crate) fn count_json_length_scans<T>(run: impl FnOnce() -> T) -> (T, usize) {
    JSON_LENGTH_SCAN_PROBE.with(|probe| {
        let prior = probe.replace(Some(0));
        let result = run();
        let scans = probe.replace(prior).unwrap_or(0);
        (result, scans)
    })
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

#[derive(Clone, Debug, Default)]
pub(crate) struct HistoryTokenRoll {
    bytes: usize,
    messages: Vec<MessageTokenStamp>,
}

#[derive(Clone, Debug)]
struct MessageTokenStamp {
    content_bytes: usize,
    private_reasoning_bytes: usize,
    tool_calls: Arc<[ToolCall]>,
}

impl MessageTokenStamp {
    fn new(message: &ChatMsg) -> Self {
        Self {
            content_bytes: message.content.len(),
            private_reasoning_bytes: private_reasoning_bytes(message),
            tool_calls: Arc::clone(&message.tool_calls),
        }
    }

    fn matches(&self, message: &ChatMsg) -> bool {
        self.content_bytes == message.content.len()
            && self.private_reasoning_bytes == private_reasoning_bytes(message)
            && Arc::ptr_eq(&self.tool_calls, &message.tool_calls)
    }
}

impl HistoryTokenRoll {
    pub(crate) fn recompute(&mut self, history: &[ChatMsg]) -> usize {
        self.bytes = estimate_history_chars(history);
        self.messages = history.iter().map(MessageTokenStamp::new).collect();
        self.bytes / 4
    }

    pub(crate) fn observe(&mut self, history: &[ChatMsg]) -> usize {
        let n = history.len();
        if n == 0 {
            *self = Self::default();
            return 0;
        }
        let prefix = self.messages.len();
        if n >= prefix
            && prefix > 0
            && history[..prefix]
                .iter()
                .zip(self.messages.iter())
                .all(|(message, stamp)| stamp.matches(message))
        {
            if n > prefix {
                self.bytes = self
                    .bytes
                    .saturating_add(estimate_history_chars(&history[prefix..]));
                self.messages
                    .extend(history[prefix..].iter().map(MessageTokenStamp::new));
            }
            return self.bytes / 4;
        }
        self.recompute(history)
    }
}

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
        crate::agent::harness::core_catalog::baseline_def("get_context_remaining")
    }
    fn call(&self, _args: &Value) -> Result<String, String> {
        let used = self.gauge.used_tokens.load(Ordering::Relaxed);
        let budget = self.gauge.budget_tokens.load(Ordering::Relaxed);
        if budget == 0 {
            Ok(format!(
                "~{used} tokens used so far.\n{}",
                crate::agent::harness::book::d56_replies::NO_BUDGET.cells()
            ))
        } else {
            let pct = (used as f64 / budget as f64 * 100.0).round() as u64;
            Ok(format!(
                "~{used} tokens in context; older turns compact at ~{budget} ({pct}%).\n{}",
                crate::agent::harness::book::d56_replies::AUTO_COMPACT.cells()
            ))
        }
    }
}

#[cfg(all(test, unix))]
#[path = "../../../../tests/cockpit/harness/context__alias_tests.rs"]
mod alias_tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/context__private_budget_tests.rs"]
mod private_budget_tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/context__launch_relative_tests.rs"]
mod launch_relative_tests;
