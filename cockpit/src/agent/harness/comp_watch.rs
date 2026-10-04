//! Explicitly configured submission status observation and tool activity classification.
//! Activity classification supports loop accounting; it does not judge research intent.

use super::*;
use crate::agent::club::ToolCall;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Shipped built-in fixture: in-flight → still-running → terminal with score.
/// Redacted gold id; not a live competition identifier.
pub const BUILTIN_WATCH_FIXTURE_JSON: &str = r#"{
  "id": "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee",
  "snapshots": [
    {"status": "validating"},
    {"status": "validating"},
    {"status": "accepted", "officialScore": "1844075.40"}
  ]
}"#;

/// Injected when the watcher observes a terminal slot result.
pub const WATCHER_NOTIFY_MARK: &str = "[harness-telemetry] WATCHER NOTIFY";

/// Phase of the watched slot, independent of the model's tool choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlotPhase {
    Empty,
    InFlight,
    Terminal,
}

/// One typed status snapshot from a configured status source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlotSnapshot {
    pub id: String,
    pub status: String,
    pub score: Option<String>,
    pub rejection_reason: Option<String>,
    pub receipt: Option<SubmissionReceipt>,
}

/// Typed exact-ID fields returned by an authenticated competition status API.
/// Prose notes are supplementary; queued state is not a terminal receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SubmissionReceipt {
    pub benchmark_id: String,
    pub promotion_status: Option<String>,
    pub promoted_source_ref: Option<String>,
    pub submission_commit_id: Option<String>,
    pub observed_at_ms: Option<u128>,
    pub source_id: String,
    pub official: Option<crate::agent::harness::cartridges::records::SubmissionStatus>,
}

impl SlotSnapshot {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn is_in_flight(&self) -> bool {
        !self.is_terminal()
    }

    pub(crate) fn is_terminal(&self) -> bool {
        // A new provider status is not evidence of acceptance. Keep observing
        // until the provider reports an explicitly recognized terminal result.
        matches!(
            self.status.trim().to_ascii_lowercase().as_str(),
            "accepted"
                | "promoted"
                | "superseded"
                | "rejected"
                | "failed"
                | "error"
                | "cancelled"
                | "canceled"
                | "timeout"
                | "timed_out"
                | "timed-out"
        )
    }
}

/// Model-visible terminal injection produced by the watcher.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WatchNotify {
    pub id: String,
    pub status: String,
    pub score: Option<String>,
    pub rejection_reason: Option<String>,
    pub source_note: Option<String>,
    pub receipt: Option<SubmissionReceipt>,
}

impl WatchNotify {
    pub(crate) fn from_snapshot(snap: &SlotSnapshot) -> Self {
        Self {
            id: snap.id.clone(),
            status: snap.status.clone(),
            score: snap.score.clone(),
            rejection_reason: snap.rejection_reason.clone(),
            source_note: None,
            receipt: snap.receipt.clone(),
        }
    }

    /// Text injected into the model's next turn. Always carries id + status
    /// and either a score or a rejection reason.
    pub(crate) fn injection_text(&self) -> String {
        let mut out = format!(
            "{WATCHER_NOTIFY_MARK} — submission {} status={}",
            self.id, self.status
        );
        match (&self.score, &self.rejection_reason) {
            (Some(score), Some(reason)) => {
                out.push_str(&format!(" score={score} reason={reason}"));
            }
            (Some(score), None) => out.push_str(&format!(" score={score}")),
            (None, Some(reason)) => out.push_str(&format!(" reason={reason}")),
            (None, None) => out.push_str(" reason=unspecified-terminal"),
        }
        if let Some(note) = &self.source_note {
            out.push_str("; ");
            out.push_str(note);
        }
        if let Some(receipt) = &self.receipt {
            out.push_str(&format!(
                "; exact receipt benchmark={} promotion={} promoted_source={} submission_commit={} observed_at_ms={} source={}",
                receipt.benchmark_id,
                receipt.promotion_status.as_deref().unwrap_or("unavailable"),
                receipt.promoted_source_ref.as_deref().unwrap_or("unavailable"),
                receipt.submission_commit_id.as_deref().unwrap_or("unavailable"),
                receipt.observed_at_ms.map(|v| v.to_string()).unwrap_or_else(|| "unavailable".into()),
                receipt.source_id,
            ));
        }
        out
    }
}

/// Status I/O is separate from proof authority and tool-result prose.
pub(crate) trait StatusSource {
    fn probe(&mut self, id: &str) -> Result<SlotSnapshot, String>;
    fn receipt_note(&self) -> Option<String> {
        None
    }
    fn take_error_notice(&mut self) -> Option<String> {
        None
    }
}

/// Sequential fixture source: each [`probe`] advances one snapshot.
#[derive(Clone, Debug)]
pub(crate) struct FixtureStatusSource {
    pub id: String,
    snapshots: Vec<SlotSnapshot>,
    cursor: usize,
}

impl FixtureStatusSource {
    pub(crate) fn from_json(json: &str) -> Result<Self, String> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("watch fixture json: {e}"))?;
        Self::from_value(&value)
    }

    pub(crate) fn from_path(path: &Path) -> Result<Self, String> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| format!("watch fixture {}: {e}", path.display()))?;
        Self::from_json(&raw)
    }

    fn from_value(value: &serde_json::Value) -> Result<Self, String> {
        let id = value
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("watch fixture missing id")?
            .trim()
            .to_string();
        if id.is_empty() {
            return Err("watch fixture id is empty".into());
        }
        let snaps = value
            .get("snapshots")
            .and_then(|v| v.as_array())
            .ok_or("watch fixture missing snapshots array")?;
        if snaps.is_empty() {
            return Err("watch fixture snapshots are empty".into());
        }
        let snapshots = snaps
            .iter()
            .map(|row| parse_snapshot(&id, row))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            id,
            snapshots,
            cursor: 0,
        })
    }

    pub(crate) fn exhausted(&self) -> bool {
        self.cursor >= self.snapshots.len()
    }
}

impl StatusSource for FixtureStatusSource {
    fn probe(&mut self, id: &str) -> Result<SlotSnapshot, String> {
        if id != self.id {
            return Err(format!("fixture id mismatch: wanted {}, got {id}", self.id));
        }
        if self.cursor >= self.snapshots.len() {
            return self
                .snapshots
                .last()
                .cloned()
                .ok_or_else(|| "empty fixture".into());
        }
        let snap = self.snapshots[self.cursor].clone();
        self.cursor = self.cursor.saturating_add(1);
        Ok(snap)
    }
}

/// Workspace file probe: `{dir}/{id}.json`. Missing file means still in-flight.
pub(crate) struct FileStatusSource {
    dir: PathBuf,
}

impl FileStatusSource {
    pub(crate) fn new(dir: PathBuf) -> Self {
        Self { dir }
    }
}

impl StatusSource for FileStatusSource {
    fn probe(&mut self, id: &str) -> Result<SlotSnapshot, String> {
        let path = self.dir.join(format!("{id}.json"));
        if !path.is_file() {
            return Ok(SlotSnapshot {
                id: id.to_string(),
                status: "validating".into(),
                score: None,
                rejection_reason: None,
                receipt: None,
            });
        }
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| format!("watch file {}: {e}", path.display()))?;
        let value: serde_json::Value =
            serde_json::from_str(&raw).map_err(|e| format!("watch file json: {e}"))?;
        parse_snapshot(id, &value)
    }
}

fn parse_snapshot(id: &str, value: &serde_json::Value) -> Result<SlotSnapshot, String> {
    // A source may omit an ID when it is already bound by a fixture/file name,
    // but an explicit identity must never be silently relabeled as our slot.
    if let Some(observed) = value.get("id")
        && observed.as_str() != Some(id)
    {
        return Err("snapshot submission id mismatch".into());
    }
    let status = value
        .get("status")
        .and_then(|v| v.as_str())
        .ok_or("snapshot missing status")?
        .trim()
        .to_string();
    if status.is_empty() {
        return Err("snapshot status is empty".into());
    }
    let score = value
        .get("officialScore")
        .or_else(|| value.get("score"))
        .and_then(json_to_opt_string)
        .filter(|s| !s.is_empty() && s != "None" && s != "null");
    let rejection_reason = value
        .get("rejectionReason")
        .or_else(|| value.get("reason"))
        .and_then(json_to_opt_string)
        .filter(|s| !s.is_empty() && s != "None" && s != "null");
    Ok(SlotSnapshot {
        id: id.to_string(),
        status,
        score,
        rejection_reason,
        receipt: None,
    })
}

fn json_to_opt_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

/// Independent slot tracker. The model does not poll this; the harness does.
#[derive(Clone, Debug, Default)]
pub(crate) struct SubmissionWatcher {
    slot_id: Option<String>,
    last: Option<SlotSnapshot>,
    last_source_note: Option<String>,
    notified_terminal: bool,
    just_notified: bool,
}

impl SubmissionWatcher {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn slot_id(&self) -> Option<&str> {
        self.slot_id.as_deref()
    }

    pub(crate) fn phase(&self) -> SlotPhase {
        if self.slot_id.is_none() {
            return SlotPhase::Empty;
        }
        if self.notified_terminal || self.last.as_ref().is_some_and(SlotSnapshot::is_terminal) {
            return SlotPhase::Terminal;
        }
        SlotPhase::InFlight
    }

    pub(crate) fn adopt(&mut self, id: &str) {
        let id = id.trim();
        if id.is_empty() {
            return;
        }
        if self.slot_id.as_deref() == Some(id) {
            return;
        }
        self.slot_id = Some(id.to_string());
        self.last = Some(SlotSnapshot {
            id: id.to_string(),
            status: "validating".into(),
            score: None,
            rejection_reason: None,
            receipt: None,
        });
        self.notified_terminal = false;
        self.just_notified = false;
        self.last_source_note = None;
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn observe_snapshot(&mut self, snap: SlotSnapshot) {
        if self.slot_id.is_none() {
            self.slot_id = Some(snap.id.clone());
        }
        if self.slot_id.as_deref() != Some(snap.id.as_str()) {
            return;
        }
        let terminal = snap.is_terminal();
        self.last = Some(snap);
        self.last_source_note = None;
        if terminal && !self.notified_terminal {
            self.notified_terminal = true;
            self.just_notified = true;
        }
    }

    /// True when status observation and slot telemetry apply to this hop.
    pub(crate) fn hop_path_active(&self, competition: bool) -> bool {
        competition || self.slot_id.is_some()
    }

    /// Probe the status source once. Returns a notify the first time the slot
    /// becomes terminal.
    pub(crate) fn poll<S: StatusSource + ?Sized>(
        &mut self,
        source: Option<&mut S>,
    ) -> Option<WatchNotify> {
        let id = self.slot_id.clone()?;
        if self.notified_terminal {
            return None;
        }
        let src = source?;
        match src.probe(&id) {
            Ok(snap) => {
                // StatusSource is an I/O boundary, not authority to retarget the
                // current slot. A stale/cache-mixed response must stay silent.
                if snap.id != id {
                    return None;
                }
                let terminal = snap.is_terminal();
                self.last = Some(snap.clone());
                self.last_source_note = src.receipt_note();
                if terminal {
                    self.notified_terminal = true;
                    self.just_notified = true;
                    let mut notify = WatchNotify::from_snapshot(&snap);
                    notify.source_note = self.last_source_note.clone();
                    Some(notify)
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }

    pub(crate) fn take_just_notified(&mut self) -> bool {
        let flagged = self.just_notified;
        self.just_notified = false;
        flagged
    }

    pub(crate) fn pending_notify(&self) -> Option<WatchNotify> {
        if !self.just_notified {
            return None;
        }
        self.last.as_ref().map(|snapshot| {
            let mut notify = WatchNotify::from_snapshot(snapshot);
            notify.source_note = self.last_source_note.clone();
            notify
        })
    }

    /// Compact typed state for the operator HUD. Only recognized terminal
    /// statuses settle the slot; unknown statuses never imply acceptance.
    pub(crate) fn telemetry(&self, competition: bool) -> SubmissionSlotTelemetry {
        if !competition && self.slot_id.is_none() {
            return SubmissionSlotTelemetry::default();
        }
        let phase = match self.phase() {
            SlotPhase::Empty => SubmissionSlotPhase::Empty,
            SlotPhase::InFlight => SubmissionSlotPhase::InFlight,
            SlotPhase::Terminal => {
                let status = self
                    .last
                    .as_ref()
                    .map(|snapshot| snapshot.status.trim().to_ascii_lowercase())
                    .unwrap_or_default();
                match status.as_str() {
                    "timeout" | "timed_out" | "timed-out" => SubmissionSlotPhase::TimedOut,
                    "rejected" | "failed" | "error" | "cancelled" | "canceled" => {
                        SubmissionSlotPhase::Rejected
                    }
                    _ => SubmissionSlotPhase::Accepted,
                }
            }
        };
        SubmissionSlotTelemetry {
            phase,
            id: self.slot_id.clone(),
            score: self
                .last
                .as_ref()
                .and_then(|snapshot| snapshot.score.clone()),
        }
    }
}

/// Poll an already-adopted watcher against a fixture until the first terminal
/// notify. Shared by the launchable entry and submit-next retarget.
pub(crate) fn poll_fixture_until_terminal(
    watcher: &mut SubmissionWatcher,
    source: &mut FixtureStatusSource,
) -> Result<WatchNotify, String> {
    let id = source.id.clone();
    let budget = source.snapshots.len().saturating_add(2);
    for _ in 0..budget {
        if let Some(notify) = watcher.poll(Some(source)) {
            return Ok(notify);
        }
        if source.exhausted() {
            break;
        }
    }
    Err(format!(
        "watcher fixture for {id} ended without a terminal status+score/reason"
    ))
}

/// Drive a fixture source to its terminal snapshot and return the injection.
/// This is the crate's shipped watcher entry (also used by `angel --watch-fixture`).
pub(crate) fn run_fixture_watch(source: &mut FixtureStatusSource) -> Result<WatchNotify, String> {
    let id = source.id.clone();
    let mut watcher = SubmissionWatcher::new();
    watcher.adopt(&id);
    poll_fixture_until_terminal(&mut watcher, source)
}

/// Launchable CLI entry: print the model-visible injection and exit via caller.
pub(crate) fn run_watch_fixture_cli(path: Option<&Path>) -> io::Result<WatchNotify> {
    let mut source = match path {
        Some(p) => FixtureStatusSource::from_path(p)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
        None => FixtureStatusSource::from_json(BUILTIN_WATCH_FIXTURE_JSON)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
    };
    let notify = run_fixture_watch(&mut source)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let injection = notify.injection_text();
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "WATCHER TERMINAL")?;
    writeln!(stdout, "id: {}", notify.id)?;
    writeln!(stdout, "status: {}", notify.status)?;
    match (&notify.score, &notify.rejection_reason) {
        (Some(score), _) => writeln!(stdout, "score: {score}")?,
        (None, Some(reason)) => writeln!(stdout, "reason: {reason}")?,
        (None, None) => writeln!(stdout, "reason: unspecified-terminal")?,
    }
    writeln!(stdout, "injection: {injection}")?;
    stdout.flush()?;
    Ok(notify)
}

/// Concrete status source for a live turn. An enum (not a trait object) so the
/// hop loop can probe without holding a dyn borrow across iterations.
pub(crate) enum ConfiguredWatchSource {
    Fixture(FixtureStatusSource),
    File(FileStatusSource),
    /// The active cartridge's own status source.
    Cartridge(Box<dyn StatusSource + Send>),
}

impl StatusSource for ConfiguredWatchSource {
    fn probe(&mut self, id: &str) -> Result<SlotSnapshot, String> {
        match self {
            Self::Fixture(src) => src.probe(id),
            Self::File(src) => src.probe(id),
            Self::Cartridge(src) => src.probe(id),
        }
    }

    fn receipt_note(&self) -> Option<String> {
        match self {
            Self::Cartridge(src) => src.receipt_note(),
            _ => None,
        }
    }

    fn take_error_notice(&mut self) -> Option<String> {
        match self {
            Self::Cartridge(src) => src.take_error_notice(),
            _ => None,
        }
    }
}

/// Open the configured status source for a live turn. Fixture env wins;
/// otherwise a workspace file probe. Never a live competition API.
pub(crate) fn open_configured_status_source(workspace: &Path) -> Option<ConfiguredWatchSource> {
    if let Ok(path) = std::env::var("ANGEL_WATCH_FIXTURE") {
        let path = path.trim();
        if !path.is_empty() {
            return FixtureStatusSource::from_path(Path::new(path))
                .ok()
                .map(ConfiguredWatchSource::Fixture);
        }
    }
    let dir = workspace.join(".angelX").join("watch");
    Some(ConfiguredWatchSource::File(FileStatusSource::new(dir)))
}

/// Pull a submission UUID out of a submit receipt or keeper line.
/// Prefers lines that mention queued / in-flight / submission; skips bare
/// hex that is not UUID-shaped.
/// Cheap receipt sniff: only the first 8 KiB, so megabyte `cargo test`
/// tails never enter UUID extraction on the ordinary hop path.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const SLOT_HINT_SCAN: usize = 8192;

/// First `SLOT_HINT_SCAN` bytes, backed up to a char boundary. No copy.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn slot_hint_head(text: &str) -> &str {
    if text.len() <= SLOT_HINT_SCAN {
        return text;
    }
    let mut end = SLOT_HINT_SCAN;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// ASCII needle search with no lowercase heap copy. Needles are ASCII, so a
/// hit is always a char boundary (UTF-8 continuation bytes are never ASCII).
pub(crate) fn ascii_find_ignore_case(hay: &str, needle: &str) -> Option<usize> {
    let hay = hay.as_bytes();
    let needle = needle.as_bytes();
    if needle.is_empty() {
        return Some(0);
    }
    if hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle))
}

pub(crate) fn ascii_contains_ignore_case(hay: &str, needle: &str) -> bool {
    ascii_find_ignore_case(hay, needle).is_some()
}

/// True when the first 8 KiB looks like a submit receipt. Ordinary compile
/// and test logs stay false — no 8 KiB lowercase allocation on that path.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn text_may_carry_slot(text: &str) -> bool {
    let head = slot_hint_head(text);
    ascii_contains_ignore_case(head, "submission queued")
        || ascii_contains_ignore_case(head, "in flight as")
        || ascii_contains_ignore_case(head, "in-flight as")
}

pub(crate) fn extract_submission_id(text: &str) -> Option<String> {
    // Receipt stanzas print the benchmark uuid first, then "Submission queued"
    // and the submission uuid (slot-keeper excludes the bench id the same way).
    // Scan in place so a live slot does not lowercase megabyte tool tails.
    for marker in [
        "submission queued",
        "in flight as",
        "in-flight as",
        "submitted as",
        "submission id:",
        "submission id =",
        "submission_id:",
        "submission_id =",
    ] {
        if let Some(idx) = ascii_find_ignore_case(text, marker)
            && let Some(id) = first_uuid_in(&text[idx..])
        {
            return Some(id);
        }
    }
    for line in text.lines() {
        // "id"/"uuid" must be standalone tokens: harness-history lines such as
        // `git log`'s "Validate submission <uuid>" carry "id" inside
        // "Val-id-ate", and adopting one hands the watcher a phantom slot it
        // then "notifies" terminal from board text (2026-09-01: the
        // morning run believed a repo-log uuid was its own in-flight
        // submission and reported it rejected without ever submitting).
        let hot = ascii_contains_ignore_case(line, "in flight")
            || ascii_contains_ignore_case(line, "in-flight")
            || (ascii_contains_ignore_case(line, "submission")
                && (ascii_contains_word_ignore_case(line, "id")
                    || ascii_contains_word_ignore_case(line, "uuid")
                    || ascii_contains_ignore_case(line, "queued")
                    || ascii_contains_ignore_case(line, "created")
                    || ascii_contains_ignore_case(line, "submitted")));
        if hot && let Some(candidate) = first_uuid_in(line) {
            return Some(candidate);
        }
    }
    None
}

/// True when `word` appears bounded by non-alphanumerics (a token, not a
/// fragment of a longer word). Byte-level so multi-byte text cannot panic a
/// boundary slice.
fn ascii_contains_word_ignore_case(hay: &str, word: &str) -> bool {
    let hay = hay.as_bytes();
    let word = word.as_bytes();
    if word.is_empty() || hay.len() < word.len() {
        return false;
    }
    for start in 0..=(hay.len() - word.len()) {
        if !hay[start..start + word.len()].eq_ignore_ascii_case(word) {
            continue;
        }
        let left_ok = start == 0 || !hay[start - 1].is_ascii_alphanumeric();
        let end = start + word.len();
        let right_ok = end == hay.len() || !hay[end].is_ascii_alphanumeric();
        if left_ok && right_ok {
            return true;
        }
    }
    false
}

fn first_uuid_in(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 36 <= bytes.len() {
        if is_uuid_at(bytes, i) {
            return Some(text[i..i + 36].to_ascii_lowercase());
        }
        i += 1;
    }
    None
}

fn is_uuid_at(bytes: &[u8], i: usize) -> bool {
    const DASHES: [usize; 4] = [8, 13, 18, 23];
    if i + 36 > bytes.len() {
        return false;
    }
    for (offset, b) in bytes[i..i + 36].iter().enumerate() {
        if DASHES.contains(&offset) {
            if *b != b'-' {
                return false;
            }
        } else if !b.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

// Tool activity classification is shared with the live loop accounting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolLane {
    SimpleLocal,
    RunnerDispatch,
}

fn call_hay(call: &ToolCall) -> (&str, std::borrow::Cow<'_, str>, bool) {
    competition_call_text(call)
}

/// `git diff` / `git diff --stat`, not `git difftool` or `git status`.
pub(crate) fn shell_hay_is_git_diff(hay: &str) -> bool {
    hay.contains("git diff ")
        || hay.contains("git diff\t")
        || hay.ends_with("git diff")
        || hay.contains("git-diff ")
}

/// `git status` / `git status --porcelain`, not `git status` as a path token
/// inside an unrelated command. `git stash` / `git submodule` stay recon.
pub(crate) fn shell_hay_is_git_status(hay: &str) -> bool {
    hay.contains("git status ")
        || hay.contains("git status\t")
        || hay.ends_with("git status")
        || hay.contains("git-status ")
}

fn skip_shell_assignments(hay: &str) -> &str {
    let mut rest = hay.trim();
    while let Some((head, tail)) = rest.split_once(char::is_whitespace) {
        if head.contains('=') && !head.starts_with('-') {
            rest = tail.trim_start();
            continue;
        }
        break;
    }
    rest
}

fn looks_like_git_ref(token: &str) -> bool {
    let token = token.trim_matches(|c| matches!(c, '"' | '\'' | '`'));
    if token.is_empty() {
        return false;
    }
    if matches!(
        token,
        "HEAD" | "ORIG_HEAD" | "FETCH_HEAD" | "MERGE_HEAD" | "main" | "master"
    ) {
        return true;
    }
    if token.starts_with("refs/")
        || token.starts_with("origin/")
        || token.starts_with("upstream/")
        || token.contains("..")
        || token.contains("@{")
    {
        return true;
    }

    token.len() >= 7 && token.len() <= 40 && token.chars().all(|c| c.is_ascii_hexdigit())
}

/// `git log -- src/kernel.cu` / `git blame src/kernel.cu` /
/// `git annotate src/kernel.cu` / `git ls-files -- src/kernel.cu` /
/// `git ls-tree HEAD src/kernel.cu` / `git rev-list HEAD -- src/kernel.cu` /
/// `git shortlog -- src/kernel.cu` / `git reflog -- src/kernel.cu` /
/// `git whatchanged -- src/kernel.cu` / `git hash-object src/kernel.cu` /
/// `git describe -- src/kernel.cu` / `git archive HEAD src/kernel.cu` /
/// `git diff-tree HEAD -- src/kernel.cu` / `git diff-index HEAD -- src/kernel.cu` /
/// `git diff-files -- src/kernel.cu` / `git check-attr -- src/kernel.cu` /
/// `git name-rev HEAD -- src/kernel.cu` / `git check-ignore src/kernel.cu`
/// is candidate inspect. Pathless `git log` / `git ls-files` / `git ls-tree` /
/// `git rev-list` / `git shortlog` / `git hash-object` / `git annotate` /
/// `git diff-index`, refs-only walks, compounds, and board history stay recon.
pub(crate) fn shell_hay_is_product_git_history(hay: &str) -> bool {
    let hay = hay.trim();
    if hay.is_empty() || hay_names_board_meta(hay) {
        return false;
    }
    if hay.contains('|')
        || hay.contains("&&")
        || hay.contains("||")
        || hay.contains(';')
        || hay.contains('`')
        || hay.contains('\n')
    {
        return false;
    }
    let rest = skip_shell_assignments(hay);
    let mut tokens = rest.split_whitespace();
    let Some(first) = tokens.next() else {
        return false;
    };
    if first.rsplit('/').next().unwrap_or(first) != "git" {
        return false;
    }
    let mut sub = None;
    let mut skip_next = false;
    let mut saw_product = false;
    for token in tokens {
        let token = token.trim_matches(|c| matches!(c, '"' | '\'' | '`'));
        if skip_next {
            skip_next = false;
            continue;
        }
        if sub.is_none() {
            if token == "-C" || token == "--git-dir" || token == "--work-tree" {
                skip_next = true;
                continue;
            }
            if token.starts_with('-') {
                continue;
            }
            sub = Some(token);
            continue;
        }
        if token.is_empty() || token == "--" || token.starts_with('-') || looks_like_git_ref(token)
        {
            continue;
        }
        if hay_names_board_meta(token) || is_meta_note_mutation_path(token) {
            return false;
        }
        if shell_inspect_path_is_product(token) {
            saw_product = true;
        }
    }
    matches!(
        sub,
        Some("log")
            | Some("blame")
            | Some("annotate")
            | Some("ls-files")
            | Some("ls-tree")
            | Some("shortlog")
            | Some("rev-list")
            | Some("reflog")
            | Some("whatchanged")
            | Some("hash-object")
            | Some("describe")
            | Some("archive")
            | Some("diff-tree")
            | Some("diff-index")
            | Some("diff-files")
            | Some("check-attr")
            | Some("name-rev")
            | Some("check-ignore")
    ) && saw_product
}

fn git_grep_flag_takes_value(token: &str) -> bool {
    matches!(
        token,
        "-e" | "-f"
            | "-O"
            | "-A"
            | "-B"
            | "-C"
            | "-j"
            | "--max-count"
            | "--max-depth"
            | "--threads"
            | "--open-files-in-pager"
    ) || token.starts_with("--max-count=")
        || token.starts_with("--max-depth=")
        || token.starts_with("--threads=")
        || (token.starts_with('-')
            && !token.starts_with("--")
            && token
                .bytes()
                .any(|c| matches!(c, b'e' | b'f' | b'O' | b'A' | b'B' | b'C' | b'j')))
}

fn git_grep_flag_is_pattern(token: &str) -> bool {
    token == "-e"
        || token == "-f"
        || (token.starts_with('-') && !token.starts_with("--") && token.contains('e'))
}

/// `git grep stream -- src/kernel.cu` is candidate inspect (submit then
/// search the file). Pathless `git grep TODO`, a path used as the pattern,
/// compounds, and board names stay recon.
pub(crate) fn shell_hay_is_product_git_grep(hay: &str) -> bool {
    let hay = hay.trim();
    if hay.is_empty() || hay_names_board_meta(hay) {
        return false;
    }
    if hay.contains('|')
        || hay.contains("&&")
        || hay.contains("||")
        || hay.contains(';')
        || hay.contains('`')
        || hay.contains('\n')
    {
        return false;
    }
    let rest = skip_shell_assignments(hay);
    let mut tokens = rest.split_whitespace();
    let Some(first) = tokens.next() else {
        return false;
    };
    if first.rsplit('/').next().unwrap_or(first) != "git" {
        return false;
    }
    let mut sub = None;
    let mut skip_next = false;
    let mut after_double_dash = false;
    let mut pattern_consumed = false;
    let mut saw_product = false;
    for token in tokens {
        let token = token.trim_matches(|c| matches!(c, '"' | '\'' | '`'));
        if skip_next {
            skip_next = false;
            continue;
        }
        if sub.is_none() {
            if token == "-C" || token == "--git-dir" || token == "--work-tree" {
                skip_next = true;
                continue;
            }
            if token.starts_with('-') {
                continue;
            }
            sub = Some(token);
            continue;
        }
        if token.is_empty() {
            continue;
        }
        if token == "--" {
            after_double_dash = true;
            continue;
        }
        if !after_double_dash && token.starts_with('-') {
            if git_grep_flag_takes_value(token) {
                skip_next = !token.contains('=');
            }
            if git_grep_flag_is_pattern(token) {
                pattern_consumed = true;
            }
            continue;
        }
        if !after_double_dash && !pattern_consumed {
            pattern_consumed = true;
            continue;
        }
        if hay_names_board_meta(token) || is_meta_note_mutation_path(token) {
            return false;
        }
        if shell_inspect_path_is_product(token) {
            saw_product = true;
        }
    }
    sub == Some("grep") && saw_product
}

fn git_show_is_inventory_listing(hay: &str) -> bool {
    const LIST_FLAGS: &[&str] = &[
        "--stat",
        "--name-only",
        "--name-status",
        "--numstat",
        "--shortstat",
        "--dirstat",
        "--summary",
        "--raw",
        "--batch",
        "--batch-check",
    ];
    LIST_FLAGS.iter().any(|flag| hay.contains(flag)) || hay.contains(" -- ")
}

fn git_show_blob_path(token: &str) -> Option<&str> {
    let token = token.trim_matches(|c| matches!(c, '"' | '\'' | '`'));
    if token.is_empty() || token.starts_with('-') || token.contains("://") {
        return None;
    }
    let (rev, path) = token.split_once(':')?;
    if rev.is_empty() || path.is_empty() || rev.contains('/') {
        return None;
    }
    Some(path)
}

/// `git show HEAD:src/kernel.cu` / `git cat-file -p HEAD:src/kernel.cu` /
/// `git rev-parse HEAD:src/kernel.cu` is candidate inspect (submit then
/// review the blob). Inventory (`--stat`, `--batch`, `REV -- path`),
/// pathless dumps, and board blobs stay recon / wait and cannot arm submit.
pub(crate) fn shell_hay_is_product_git_show(hay: &str) -> bool {
    let hay = hay.trim();
    if hay.is_empty() || hay_names_board_meta(hay) {
        return false;
    }
    if hay.contains('|')
        || hay.contains("&&")
        || hay.contains("||")
        || hay.contains(';')
        || hay.contains('`')
        || hay.contains('\n')
        || git_show_is_inventory_listing(hay)
    {
        return false;
    }
    let rest = skip_shell_assignments(hay);
    let mut tokens = rest.split_whitespace();
    let Some(first) = tokens.next() else {
        return false;
    };
    if first.rsplit('/').next().unwrap_or(first) != "git" {
        return false;
    }
    let mut sub = None;
    let mut skip_next = false;
    let mut saw_product = false;
    for token in tokens {
        let token = token.trim_matches(|c| matches!(c, '"' | '\'' | '`'));
        if skip_next {
            skip_next = false;
            continue;
        }
        if sub.is_none() {
            if token == "-C" || token == "--git-dir" || token == "--work-tree" {
                skip_next = true;
                continue;
            }
            if token.starts_with('-') {
                continue;
            }
            sub = Some(token);
            continue;
        }
        if token.is_empty() || token.starts_with('-') {
            continue;
        }
        let Some(path) = git_show_blob_path(token) else {
            continue;
        };
        if hay_names_board_meta(path) || is_meta_note_mutation_path(path) {
            return false;
        }
        if shell_inspect_path_is_product(path) {
            saw_product = true;
        }
    }
    matches!(sub, Some("show") | Some("cat-file") | Some("rev-parse")) && saw_product
}

const RUNNER_MARKERS: &[&str] = &[
    "popcorn submit",
    "popcorn-cli submit",
    "popcorn-cli run",
    "--mode benchmark",
    "--mode leaderboard",
    "modal run",
];

const PREFLIGHT_MARKERS: &[&str] = &[
    "nvcc",
    "ptxas",
    "cargo check",
    "cargo test",
    "cargo build",
    "rustc ",
    "python -c",
    "grep -ci stream",
    "grep -c stream",
    "cubin",
    "-xptxas",
];

/// World-card local-benchmark shells. Distinct from remote `--mode benchmark`
/// (a runner). Winning traces time the next candidate while a slot is in flight.
const LOCAL_BENCHMARK_MARKERS: &[&str] = &[
    "ncu ",
    "ncu\t",
    "nsys profile",
    "nsys launch",
    "python bench.py",
    "python3 bench.py",
    "uv run bench.py",
];

pub(crate) fn classify_tool_lane(call: &ToolCall) -> ToolLane {
    let (_name, hay, _is_shell) = call_hay(call);
    if RUNNER_MARKERS.iter().any(|m| hay.contains(m))
        || crate::agent::harness::cartridges::boards()
            .any(|board| hay.contains(&format!("{board} submit")))
    {
        ToolLane::RunnerDispatch
    } else {
        ToolLane::SimpleLocal
    }
}

/// Model-owned status snapshots. Runner dispatch is deliberately excluded:
/// `<board> submit` advances the board, while `<board> submissions`, board reads,
/// and `proc_status` only observe work already in flight.
pub(crate) fn is_passive_status_call(call: &ToolCall) -> bool {
    matches!(call.name.as_str(), "proc_status" | "proc_wait")
        || is_progress_artifact_snapshot_call(call)
        || (classify_tool_lane(call) != ToolLane::RunnerDispatch
            && is_competition_wait_or_progress_call(call))
}

/// Read-only snapshots of background progress artifacts. These are useful
/// once, but repeated `tail /tmp/*progress*`, log, and status-file checks are
/// polling just as surely as `proc_status`. Keep the match path-oriented so a
/// source grep for the word "progress" is still normal candidate inspection.
pub(crate) fn is_progress_artifact_snapshot_call(call: &ToolCall) -> bool {
    let path_marks_progress = |path: &str| {
        let path = path.to_ascii_lowercase();
        path.contains("/tmp/")
            || path.contains(".log")
            || path.contains("progress.")
            || path.contains("progress-")
            || path.contains("-progress")
            || path.contains("status.json")
            || path.contains("results.tsv")
    };
    if call.name == "read_file" {
        return inspect_call_path(call).is_some_and(path_marks_progress);
    }
    if call.name != "shell" || classify_tool_lane(call) == ToolLane::RunnerDispatch {
        return false;
    }
    let Some(command) = crate::agent::tools::shell::shell_command_arg(&call.args) else {
        return false;
    };
    let snapshot_program = ["tail", "head", "cat", "grep", "wc", "du", "ls", "stat"]
        .iter()
        .any(|program| shell_argv_has_token(command, program));
    snapshot_program && path_marks_progress(command)
}

/// Cheap local timing of the candidate (`ncu`, `nsys`, `python bench.py`).
/// Remote popcorn `--mode benchmark` is a runner, not this.
pub(crate) fn is_local_benchmark_call(call: &ToolCall) -> bool {
    if classify_tool_lane(call) == ToolLane::RunnerDispatch {
        return false;
    }
    if is_competition_wait_or_progress_call(call) {
        return false;
    }
    let (_name, hay, is_shell) = call_hay(call);
    is_shell && LOCAL_BENCHMARK_MARKERS.iter().any(|m| hay.contains(m))
}

pub(crate) fn is_local_preflight_call(call: &ToolCall) -> bool {
    if classify_tool_lane(call) == ToolLane::RunnerDispatch {
        return false;
    }
    // Board wait/poll is not candidate preflight. First-write already
    // exempts these calls from the inspection budget; the in-flight
    // watcher must still fail them as PollStatus (and they must not
    // launder runner escalation).
    if is_passive_status_call(call) {
        return false;
    }
    // Native check/run_tests/lint are the world-card local-benchmark —
    // they must not FailReconThrash while a slot is in flight, and they
    // may arm runner escalation. Board wait already returned above.
    if is_verification_call(call) {
        return true;
    }
    if is_local_benchmark_call(call) {
        return true;
    }
    if call.name == "find_files" {
        let pattern = call
            .args
            .get("pattern")
            .and_then(Value::as_str)
            .unwrap_or("");
        return find_files_pattern_is_path_scoped(pattern);
    }
    if call.name == "file_search" {
        let query = call.args.get("query").and_then(Value::as_str).unwrap_or("");
        return file_search_query_is_path_scoped(query);
    }
    // Working-tree `git_diff` / `git_status` / `git diff` / `git status` is
    // the world-card inspect of the candidate (submit then review the tree).
    // A board-path inspect is recon and must not arm submit.
    if matches!(call.name.as_str(), "git_diff" | "git_status") {
        return !inspect_path_is_board_meta(call);
    }
    // Path-scoped `git_log` is candidate history (submit then blame/log the
    // file). Pathless history dump and board-path log stay recon.
    if call.name == "git_log" {
        return inspect_call_path(call).is_some_and(shell_inspect_path_is_product);
    }
    let (name, hay, is_shell) = call_hay(call);
    if is_shell && (shell_hay_is_git_diff(&hay) || shell_hay_is_git_status(&hay)) {
        return !hay_names_board_meta(&hay);
    }
    // `git log -- src/kernel.cu` / `git blame src/kernel.cu` /
    // `git annotate src/kernel.cu` / `git ls-files -- src/kernel.cu` /
    // `git ls-tree HEAD src/kernel.cu` / `git rev-list HEAD -- src/kernel.cu` /
    // `git shortlog -- src/kernel.cu` / `git hash-object src/kernel.cu` /
    // `git describe -- src/kernel.cu` / `git diff-index HEAD -- src/kernel.cu` /
    // `git diff-files -- src/kernel.cu` is the same class as path-scoped
    // git_log. Pathless dumps and board history stay recon.
    if is_shell && shell_hay_is_product_git_history(&hay) {
        return true;
    }
    // `git show HEAD:src/kernel.cu` / `git cat-file -p HEAD:src/kernel.cu` /
    // `git rev-parse HEAD:src/kernel.cu` is candidate blob inspect.
    // Inventory listings and board blobs stay recon / wait.
    if is_shell && shell_hay_is_product_git_show(&hay) {
        return true;
    }
    // `git grep stream -- src/kernel.cu` is the same class as path-scoped
    // grep. Pathless dumps, path-as-pattern, and board names stay recon.
    if is_shell && shell_hay_is_product_git_grep(&hay) {
        return true;
    }
    // `cat`/`head` of a product path is the same class as path-scoped read_file.
    if is_shell && shell_hay_is_product_inspect(&hay) {
        return true;
    }
    // `ls src` is the same class as path-scoped list_dir. Bare `ls` and
    // recursive listings stay recon and cannot arm submit.
    if is_shell && shell_hay_is_product_ls(&hay) {
        return true;
    }
    // `grep stream src/kernel.cu` is the same class as path-scoped grep.
    // Pathless `grep TODO` and recursive `-r` stay recon.
    if is_shell && shell_hay_is_path_scoped_grep(&hay) {
        return true;
    }
    if is_shell && PREFLIGHT_MARKERS.iter().any(|m| hay.contains(m)) {
        return true;
    }
    // Cheap local inspect of a candidate counts as preflight, not recon thrash.
    // Board / living-handoff paths are not the candidate — outline/defs/grep
    // /list_dir of those files must not launder runner escalation.
    if matches!(name, "read_file" | "outline" | "defs" | "grep" | "list_dir") {
        if inspect_path_is_board_meta(call) {
            return false;
        }
        // Path-less outline/defs/grep/list_dir is workspace search, not a
        // candidate inspect — it must not Accept as LocalPreflight or
        // arm runner escalation (A9: defs(symbol=board_tip) / grep TODO).
        if name != "read_file" && inspect_call_path(call).is_none() {
            return false;
        }
        return true;
    }
    false
}

/// Path-scoped `file_search` query (`src/kernel`). Workspace name fragments
/// (`kernel`) and board-name queries stay recon.
pub(crate) fn file_search_query_is_path_scoped(query: &str) -> bool {
    find_files_pattern_is_path_scoped(query)
}

/// First argv basename is a read-only file dump (`cat`/`head`/`tail`/`wc`/`nl`),
/// a quiet `sed` pager (`sed -n`), a metadata probe (`stat`/`file`), a hex
/// dump (`od`/`hexdump`/`xxd`), a checksum (`md5sum`/`sha256sum`/`sha1sum`/`cksum`),
/// a compare (`diff`/`cmp`), a printable-string dump (`strings`), a binary
/// inspect (`nm`/`objdump`/`readelf`/`size` and the LLVM/eu/rust twins,
/// plus `addr2line`/`dwarfdump`/`llvm-symbolizer`), or a text encode (`base64`).
fn shell_inspect_program(first: &str) -> bool {
    let base = first.rsplit('/').next().unwrap_or(first);
    matches!(
        base,
        "cat"
            | "head"
            | "tail"
            | "wc"
            | "nl"
            | "sed"
            | "stat"
            | "file"
            | "od"
            | "hexdump"
            | "xxd"
            | "md5sum"
            | "sha256sum"
            | "sha1sum"
            | "cksum"
            | "diff"
            | "cmp"
            | "strings"
            | "nm"
            | "objdump"
            | "readelf"
            | "size"
            | "llvm-nm"
            | "llvm-objdump"
            | "llvm-readelf"
            | "llvm-size"
            | "llvm-dwarfdump"
            | "llvm-addr2line"
            | "llvm-symbolizer"
            | "eu-nm"
            | "eu-objdump"
            | "eu-readelf"
            | "eu-addr2line"
            | "eu-size"
            | "addr2line"
            | "dwarfdump"
            | "rust-objdump"
            | "rust-nm"
            | "rust-readelf"
            | "rust-size"
            | "rust-addr2line"
            | "base64"
    )
}

fn shell_inspect_basename(first: &str) -> &str {
    first.rsplit('/').next().unwrap_or(first)
}

/// `sed -i` / `--in-place` writes the tree. Must not launder as preflight.
pub(crate) fn shell_sed_is_inplace(hay: &str) -> bool {
    hay.split_whitespace().any(|token| {
        let token = token.trim_matches(|c| matches!(c, '"' | '\'' | '`'));
        token == "--in-place"
            || token.starts_with("--in-place=")
            || (token.starts_with("-i") && !token.starts_with("-n"))
    })
}

/// Quiet print-only `sed` (`-n` / `--quiet` / `--silent`, including `-ne`).
pub(crate) fn shell_sed_is_quiet_print(hay: &str) -> bool {
    hay.split_whitespace().any(|token| {
        let token = token.trim_matches(|c| matches!(c, '"' | '\'' | '`'));
        token == "--quiet"
            || token == "--silent"
            || token == "-n"
            || (token.starts_with('-')
                && !token.starts_with("--")
                && token.contains('n')
                && !token.contains('i'))
    })
}

/// Token looks like a repo-relative product path (`src/kernel.cu`).
/// Absolute paths, redirects, and board names stay recon.
fn shell_inspect_path_is_product(token: &str) -> bool {
    let token = token.trim_matches(|c| matches!(c, '"' | '\'' | '`'));
    if token.is_empty()
        || token.starts_with('-')
        || token.contains('>')
        || token.contains('<')
        || token.contains('|')
        || token.contains('=')
    {
        return false;
    }
    if token.starts_with('/') || token.starts_with('\\') {
        return false;
    }
    let rest = token
        .strip_prefix("./")
        .or_else(|| token.strip_prefix(".\\"))
        .unwrap_or(token);
    find_files_pattern_is_path_scoped(rest)
}

/// Path glued to an inspect flag (`--obj=src/kernel.o`, `--exe=src/kernel.o`).
/// Flag-only tokens (`--obj`, `-e`) stay flags.
pub(crate) fn shell_inspect_flag_path(token: &str) -> Option<&str> {
    const PREFIXES: &[&str] = &["--obj=", "--exe=", "-e="];
    for prefix in PREFIXES {
        if let Some(path) = token.strip_prefix(prefix) {
            let path = path.trim_matches(|c| matches!(c, '"' | '\'' | '`'));
            if !path.is_empty() {
                return Some(path);
            }
        }
    }
    None
}

/// `cat src/kernel.cu` / `head -n 80 src/kernel.cu` / `sed -n 1,80p src/kernel.cu`
/// / `stat src/kernel.cu` / `file src/kernel.cu` / `xxd src/kernel.cu` /
/// `md5sum src/kernel.cu` / `diff -u src/kernel.cu src/ref.cu` /
/// `strings src/kernel.cu` / `nm src/kernel.o` / `objdump -d src/kernel.o` /
/// `llvm-objdump -d src/kernel.o` / `llvm-nm src/kernel.o` /
/// `addr2line -e src/kernel.o` / `llvm-addr2line -e src/kernel.o` /
/// `llvm-symbolizer --obj=src/kernel.o` / `eu-addr2line -e src/kernel.o` /
/// `dwarfdump src/kernel.o` / `rust-objdump -d src/kernel.o` /
/// `eu-readelf -h src/kernel.o` /
/// `readelf -h src/kernel.o` / `base64 src/kernel.cu` is candidate inspect.
/// Workspace basenames (`cat kernel.cu`), board files, compounds, in-place
/// `sed -i`, and absolute dumps stay recon and cannot arm submit.
pub(crate) fn shell_hay_is_product_inspect(hay: &str) -> bool {
    let hay = hay.trim();
    if hay.is_empty() || hay_names_board_meta(hay) {
        return false;
    }
    if hay.contains('|')
        || hay.contains("&&")
        || hay.contains("||")
        || hay.contains(';')
        || hay.contains('`')
        || hay.contains('\n')
    {
        return false;
    }
    let mut tokens = hay.split_whitespace();
    let Some(first) = tokens.next() else {
        return false;
    };
    if !shell_inspect_program(first) {
        return false;
    }
    if shell_inspect_basename(first) == "sed"
        && (shell_sed_is_inplace(hay) || !shell_sed_is_quiet_print(hay))
    {
        return false;
    }
    let mut saw_product = false;
    for token in tokens {
        let token = token.trim_matches(|c| matches!(c, '"' | '\'' | '`'));
        if token.is_empty() {
            continue;
        }
        let path_token = if token.starts_with('-') {
            match shell_inspect_flag_path(token) {
                Some(path) => path,
                None => continue,
            }
        } else {
            token
        };
        if hay_names_board_meta(path_token) || is_meta_note_mutation_path(path_token) {
            return false;
        }
        if shell_inspect_path_is_product(path_token) {
            saw_product = true;
        }
    }
    saw_product
}

/// First argv basename is a non-recursive directory listing.
fn shell_ls_program(first: &str) -> bool {
    first.rsplit('/').next().unwrap_or(first) == "ls"
}

/// Token looks like a list_dir target (`src`, `src/kernel`). Absolute paths,
/// `.` / `..`, globs, and board names stay recon.
fn shell_ls_path_is_product(token: &str) -> bool {
    let token = token.trim_matches(|c| matches!(c, '"' | '\'' | '`'));
    if token.is_empty()
        || token.starts_with('-')
        || token.contains('>')
        || token.contains('<')
        || token.contains('|')
        || token.contains('=')
        || token.contains('*')
    {
        return false;
    }
    if token.starts_with('/') || token.starts_with('\\') {
        return false;
    }
    let rest = token
        .strip_prefix("./")
        .or_else(|| token.strip_prefix(".\\"))
        .unwrap_or(token);
    if rest.is_empty() || rest == "." || rest == ".." {
        return false;
    }
    if hay_names_board_meta(rest) || is_meta_note_mutation_path(rest) {
        return false;
    }
    true
}

/// `ls src` / `ls src/kernel` is candidate inspect (same class as list_dir).
/// Bare `ls`, recursive `-R`, compounds, and board listings stay recon.
pub(crate) fn shell_hay_is_product_ls(hay: &str) -> bool {
    let hay = hay.trim();
    if hay.is_empty() || hay_names_board_meta(hay) {
        return false;
    }
    if hay.contains('|')
        || hay.contains("&&")
        || hay.contains("||")
        || hay.contains(';')
        || hay.contains('`')
        || hay.contains('\n')
    {
        return false;
    }
    if ls_cmd_is_recursive_listing(hay) {
        return false;
    }
    let mut tokens = hay.split_whitespace();
    let Some(first) = tokens.next() else {
        return false;
    };
    if !shell_ls_program(first) {
        return false;
    }
    let mut saw_product = false;
    for token in tokens {
        let token = token.trim_matches(|c| matches!(c, '"' | '\'' | '`'));
        if token.is_empty() || token.starts_with('-') {
            continue;
        }
        if hay_names_board_meta(token) || is_meta_note_mutation_path(token) {
            return false;
        }
        if shell_ls_path_is_product(token) {
            saw_product = true;
        }
    }
    saw_product
}

/// First argv basename is a content search (`grep`/`rg`), not `git grep`.
fn shell_grep_program(first: &str) -> bool {
    matches!(
        first.rsplit('/').next().unwrap_or(first),
        "grep" | "egrep" | "fgrep" | "rg" | "ripgrep"
    )
}

/// `grep stream src/kernel.cu` / `rg stream src/` is candidate inspect.
/// Pathless `grep TODO`, recursive `-r`, compounds, and board paths stay recon.
pub(crate) fn shell_hay_is_path_scoped_grep(hay: &str) -> bool {
    let hay = hay.trim();
    if hay.is_empty() || hay_names_board_meta(hay) {
        return false;
    }
    if hay.contains('|')
        || hay.contains("&&")
        || hay.contains("||")
        || hay.contains(';')
        || hay.contains('`')
        || hay.contains('\n')
        || hay.contains("git grep")
    {
        return false;
    }
    let mut tokens = hay.split_whitespace();
    let Some(first) = tokens.next() else {
        return false;
    };
    if !shell_grep_program(first) {
        return false;
    }
    let mut saw_product = false;
    for token in tokens {
        let token = token.trim_matches(|c| matches!(c, '"' | '\'' | '`'));
        if token.is_empty() {
            continue;
        }
        if token.starts_with('-') {
            let flags = token.trim_start_matches('-');
            if flags == "recursive"
                || flags == "recurse"
                || flags.starts_with("recursive=")
                || flags.starts_with("recurse=")
                || (!token.starts_with("--") && flags.chars().any(|c| c == 'r'))
            {
                return false;
            }
            continue;
        }
        if hay_names_board_meta(token) || is_meta_note_mutation_path(token) {
            return false;
        }
        if find_files_pattern_is_path_scoped(token) {
            saw_product = true;
        }
    }
    saw_product
}

/// Path-scoped `find_files` glob (`src/**/*.cu`) is candidate inspect.
/// Workspace inventory (`**/*.cu`, `*.rs`) and board-name globs stay recon.
pub(crate) fn find_files_pattern_is_path_scoped(pattern: &str) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return false;
    }
    if hay_names_board_meta(pattern) || is_meta_note_mutation_path(pattern) {
        return false;
    }
    // Treat `\` as `/` in place so classify hops do not copy every path.
    if pattern.starts_with("**") || pattern.starts_with("*.") || pattern == "*" {
        return false;
    }
    pattern.contains('/') || pattern.contains('\\')
}

fn inspect_call_path(call: &ToolCall) -> Option<&str> {
    call.args
        .get("path")
        .or_else(|| call.args.get("file_path"))
        .or_else(|| call.args.get("file"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn inspect_path_is_board_meta(call: &ToolCall) -> bool {
    inspect_call_path(call).is_some_and(is_meta_note_mutation_path)
}
