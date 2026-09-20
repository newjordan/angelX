//! Hashline — a compact, content-anchored patch format for the agent's edits.
//!
//! Ported and adapted from oh-my-pi's `hashline` (github.com/can1357/oh-my-pi).
//! The problem it solves: angel0's other edit tools (`str_replace`, `multi_edit`,
//! freeform `apply_patch`) locate a region by re-emitting the *old* text and
//! fuzzy-matching it. That costs output tokens twice (old **and** new), and a
//! non-unique `old` must be padded with surrounding context — more tokens still.
//!
//! Hashline instead addresses lines by **number**, and binds the whole patch to
//! a short **content hash** of the file the model is looking at. The model emits
//! only the *new* rows; if the live file has changed since it was read (the tag
//! diverges) the patcher first tries session-snapshot recovery (remap anchors
//! through unchanged lines), then rejects before it can corrupt anything.
//!
//! ## Format
//!
//! ```text
//! *** Begin Patch
//! [src/foo.rs#a1b2c3d4]
//! SWAP 10.=12:
//! +    let x = compute();
//! +    use_it(x);
//! SWAP.BLK 20:
//! +fn whole_block() {
//! +    // …
//! +}
//! INS.POST 30:
//! +// trailing note
//! DEL 40.=41
//! MV src/foo_v2.rs
//! *** End Patch
//! ```
//!
//! - `[PATH#TAG]` starts a file section. `TAG` is [`content_tag`] of the file's
//!   current text (what the model was shown). A mismatch rejects the section.
//! - Line ids are **1-based**; `A.=B` is the inclusive range `A..=B`, `A` alone
//!   is the single line `A`.
//! - `SWAP A[.=B]:` replaces those lines with the following `+` body rows.
//! - `SWAP.BLK A:` / `DEL.BLK A` / `INS.BLK.POST A:` resolve a multi-line
//!   construct that begins on line A (brace / markdown / indent heuristics).
//! - `DEL A[.=B]` deletes those lines (no body).
//! - `INS.PRE A:` / `INS.POST A:` insert the body before / after line `A`.
//! - `INS.HEAD:` / `INS.TAIL:` insert the body at the top / bottom of the file.
//! - `REM` deletes the whole file named by the section header.
//! - `MV DEST` renames/moves the file (after any line edits) to `DEST`.
//! - `+text` is one literal body row; a bare `+` is a blank line.
//!
//! All ids refer to the file's ORIGINAL numbering, so ops are order-independent
//! and never have to account for lines an earlier op added or removed.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

/// Per-process bound on distinct paths retained for stale-tag recovery.
const SNAP_MAX_PATHS: usize = 48;
/// Versions kept per path (oldest dropped first). Enough for a short edit chain.
const SNAP_MAX_VERSIONS: usize = 6;

/// One observed whole-file version, keyed by its content tag.
#[derive(Debug, Clone)]
struct Snapshot {
    text: String,
    tag: String,
}

/// Session-scoped store of full-file versions the model has actually seen
/// (via `read_file` / successful writes). Stale hashline tags resolve back to
/// these texts so recovery can remap anchors onto the live file.
#[derive(Default)]
struct SnapshotStore {
    /// Path → ring of versions (front = newest).
    by_path: HashMap<String, VecDeque<Snapshot>>,
    /// Recency list of paths (front = newest); enforces `SNAP_MAX_PATHS`.
    order: VecDeque<String>,
}

impl SnapshotStore {
    fn touch_path(&mut self, path: &str) {
        self.order.retain(|p| p != path);
        self.order.push_front(path.to_string());
        while self.order.len() > SNAP_MAX_PATHS {
            if let Some(evict) = self.order.pop_back() {
                self.by_path.remove(&evict);
            }
        }
    }

    fn record(&mut self, path: &str, text: &str) -> String {
        let normalised = normalise_newlines(text);
        let tag = content_tag(&normalised);
        self.touch_path(path);
        let ring = self.by_path.entry(path.to_string()).or_default();
        // Fuse identical content: refresh to front, reuse tag.
        if let Some(pos) = ring
            .iter()
            .position(|s| s.tag == tag && s.text == normalised)
        {
            if pos != 0
                && let Some(snap) = ring.remove(pos)
            {
                ring.push_front(snap);
            }
            return tag;
        }
        ring.push_front(Snapshot {
            text: normalised,
            tag: tag.clone(),
        });
        while ring.len() > SNAP_MAX_VERSIONS {
            ring.pop_back();
        }
        tag
    }

    fn by_hash(&self, path: &str, tag: &str) -> Option<&str> {
        self.by_path
            .get(path)?
            .iter()
            .find(|s| s.tag == tag)
            .map(|s| s.text.as_str())
    }

    fn invalidate(&mut self, path: &str) {
        self.by_path.remove(path);
        self.order.retain(|p| p != path);
    }

    fn relocate(&mut self, from: &str, to: &str) {
        if from == to {
            return;
        }
        if let Some(ring) = self.by_path.remove(from) {
            self.order.retain(|p| p != from);
            self.by_path.insert(to.to_string(), ring);
            self.touch_path(to);
        }
    }
}

fn snapshot_store() -> std::sync::MutexGuard<'static, SnapshotStore> {
    use std::sync::OnceLock;
    static SNAPSHOTS: OnceLock<Mutex<SnapshotStore>> = OnceLock::new();
    let mtx = SNAPSHOTS.get_or_init(|| Mutex::new(SnapshotStore::default()));
    match mtx.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Record a whole-file version the model has observed. Returns the content tag
/// (same as [`content_tag`]). Call after successful reads and writes so a later
/// stale hashline patch can recover.
pub(crate) fn record_snapshot(path: &str, text: &str) -> String {
    snapshot_store().record(path, text)
}

/// Drop retained versions for `path` (after REM, or when deliberately forgetting).
pub(crate) fn invalidate_snapshot(path: &str) {
    snapshot_store().invalidate(path);
}

/// Move retained history from `from` to `to` after a successful MV.
pub(crate) fn relocate_snapshot(from: &str, to: &str) {
    snapshot_store().relocate(from, to);
}

fn lookup_snapshot(path: &str, tag: &str) -> Option<String> {
    snapshot_store().by_hash(path, tag).map(str::to_string)
}

/// Short content tag for a file: the low 32 bits of a hash over the file's
/// newline-normalised text, as 8 lowercase hex chars. Any real edit flips it;
/// its only job is to notice the file changed under the model, so a short tag is
/// enough (and keeps the header the model must echo tiny).
pub(crate) fn content_tag(text: &str) -> String {
    let normalised = normalise_newlines(text);
    let mut hasher = DefaultHasher::new();
    normalised.hash(&mut hasher);
    // Low 32 bits → 8 hex. DefaultHasher isn't stable across Rust versions, but
    // the tag is only ever compared within one process against a file we just
    // read, so cross-version stability is irrelevant.
    format!("{:08x}", (hasher.finish() & 0xffff_ffff) as u32)
}

fn normalise_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Whether `patch` looks like a hashline patch (vs a freeform/unified diff), so
/// the dispatcher can route it. We require the Begin/End envelope plus at least
/// one `[path#tag]` section header, which the other formats never carry.
pub(crate) fn looks_like_hashline(patch: &str) -> bool {
    let mut saw_begin = false;
    let mut saw_section = false;
    for line in patch.lines() {
        let t = line.trim();
        if t == "*** Begin Patch" {
            saw_begin = true;
        } else if is_section_header(t).is_some() {
            saw_section = true;
        }
    }
    saw_begin && saw_section
}

/// If `line` is a `[PATH#TAG]` header, return `(path, tag)`.
fn is_section_header(line: &str) -> Option<(&str, &str)> {
    let inner = line.strip_prefix('[')?.strip_suffix(']')?;
    // TAG is the substring after the LAST '#', so paths may themselves contain
    // '#'. TAG must be non-empty and all hex.
    let hash = inner.rfind('#')?;
    let (path, tag) = (&inner[..hash], &inner[hash + 1..]);
    if path.is_empty() || tag.is_empty() || !tag.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some((path, tag))
}

#[derive(Debug, Clone, PartialEq)]
enum Op {
    /// Replace inclusive 1-based range `start..=end` with `body`.
    Swap {
        start: usize,
        end: usize,
        body: Vec<String>,
    },
    /// Delete inclusive 1-based range `start..=end`.
    Del { start: usize, end: usize },
    /// Insert `body` before line `at` (1-based).
    InsPre { at: usize, body: Vec<String> },
    /// Insert `body` after line `at` (1-based).
    InsPost { at: usize, body: Vec<String> },
    /// Insert `body` at the very top of the file.
    InsHead { body: Vec<String> },
    /// Insert `body` at the very bottom of the file.
    InsTail { body: Vec<String> },
    /// Replace the multi-line construct that begins on `at`.
    SwapBlk { at: usize, body: Vec<String> },
    /// Delete the multi-line construct that begins on `at`.
    DelBlk { at: usize },
    /// Insert `body` after the end of the construct that begins on `at`.
    InsBlkPost { at: usize, body: Vec<String> },
}

/// Outcome of applying one hashline section — the dispatcher commits these
/// transactionally (all-or-nothing across the patch).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SectionPlan {
    /// Rewrite `path` to `content` (may also be a no-op rewrite after pure MV
    /// content transform — the move itself is separate).
    Update { path: String, content: String },
    /// Delete `path` entirely.
    Remove { path: String },
    /// Write final content at `to`, remove `from`. Line edits (if any) were
    /// applied against `from` first.
    Move {
        from: String,
        to: String,
        content: String,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct Section {
    pub(crate) path: String,
    tag: String,
    ops: Vec<Op>,
    /// Delete the whole file. Incompatible with line ops / `move_to`.
    remove: bool,
    /// After applying line ops (or with no line ops: pure rename), move here.
    move_to: Option<String>,
}

impl Section {
    pub(crate) fn path(&self) -> &str {
        &self.path
    }
}

/// Parse a hashline patch into its file sections. Returns a human-readable error
/// on any malformed header, op, or body row so the model can self-correct.
pub(crate) fn parse(patch: &str) -> Result<Vec<Section>, String> {
    let mut lines = patch.lines().peekable();
    // Skip to the Begin marker (tolerate leading prose/fences the model added).
    let mut started = false;
    for line in lines.by_ref() {
        if line.trim() == "*** Begin Patch" {
            started = true;
            break;
        }
    }
    if !started {
        return Err("hashline: missing `*** Begin Patch`".to_string());
    }

    let mut sections: Vec<Section> = Vec::new();
    let mut cur: Option<Section> = None;

    while let Some(raw) = lines.next() {
        let line = raw;
        let trimmed = line.trim_end();
        if trimmed.trim() == "*** End Patch" {
            if let Some(s) = cur.take() {
                sections.push(s);
            }
            return finalize(sections);
        }
        if let Some((path, tag)) = is_section_header(trimmed.trim()) {
            if let Some(s) = cur.take() {
                sections.push(s);
            }
            cur = Some(Section {
                path: path.to_string(),
                tag: tag.to_string(),
                ops: Vec::new(),
                remove: false,
                move_to: None,
            });
            continue;
        }
        // Blank line between ops is harmless.
        if trimmed.trim().is_empty() {
            continue;
        }
        let section = cur
            .as_mut()
            .ok_or("hashline: op before any `[path#tag]` header")?;
        let head = trimmed.trim();
        if head == "REM" {
            if section.remove {
                return Err(format!("hashline: {}: duplicate REM", section.path));
            }
            if section.move_to.is_some() {
                return Err(format!(
                    "hashline: {}: REM cannot combine with MV",
                    section.path
                ));
            }
            if !section.ops.is_empty() {
                return Err(format!(
                    "hashline: {}: REM cannot combine with line ops",
                    section.path
                ));
            }
            section.remove = true;
            continue;
        }
        if let Some(dest) = head.strip_prefix("MV ") {
            let dest = dest.trim();
            if dest.is_empty() {
                return Err(format!("hashline: {}: empty MV destination", section.path));
            }
            // Quoted destinations (spaces) — strip one pair of matching quotes.
            let dest = unquote_path(dest)?;
            if section.remove {
                return Err(format!(
                    "hashline: {}: MV cannot combine with REM",
                    section.path
                ));
            }
            if section.move_to.is_some() {
                return Err(format!("hashline: {}: duplicate MV", section.path));
            }
            section.move_to = Some(dest);
            continue;
        }
        if section.remove {
            return Err(format!(
                "hashline: {}: line ops cannot follow REM",
                section.path
            ));
        }
        // Op headers introduce an operation; a `+` body row is consumed by the
        // op parser via the shared `lines` iterator.
        let op = parse_op(head, &mut lines)?;
        section.ops.push(op);
    }
    Err("hashline: missing `*** End Patch`".to_string())
}

fn unquote_path(s: &str) -> Result<String, String> {
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
        || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2)
    {
        return Ok(s[1..s.len() - 1].to_string());
    }
    Ok(s.to_string())
}

fn finalize(sections: Vec<Section>) -> Result<Vec<Section>, String> {
    if sections.is_empty() {
        return Err("hashline: patch has no file sections".to_string());
    }
    for section in &sections {
        if !section.remove && section.ops.is_empty() && section.move_to.is_none() {
            return Err(format!(
                "hashline: {}: section has no ops (expected SWAP/DEL/INS…, REM, or MV)",
                section.path
            ));
        }
    }
    Ok(sections)
}

/// Collect the following `+`-prefixed body rows until a non-body line. The
/// iterator is left positioned so the caller's loop sees that next line.
fn take_body<'a, I>(lines: &mut std::iter::Peekable<I>) -> Vec<String>
where
    I: Iterator<Item = &'a str>,
{
    let mut body = Vec::new();
    while let Some(next) = lines.peek() {
        let t = next.trim_end();
        if let Some(rest) = t.strip_prefix('+') {
            body.push(rest.to_string());
            lines.next();
        } else if t.trim().is_empty() {
            // A blank physical line inside a body is ambiguous; treat it as the
            // body's end (use a bare `+` for an intentional blank row).
            break;
        } else {
            break;
        }
    }
    body
}

/// Parse `A` or `A.=B` into an inclusive 1-based `(start, end)`.
fn parse_range(spec: &str) -> Result<(usize, usize), String> {
    let spec = spec.trim();
    if let Some((a, b)) = spec.split_once(".=") {
        let start = parse_lid(a)?;
        let end = parse_lid(b)?;
        if end < start {
            return Err(format!("hashline: range end {end} before start {start}"));
        }
        Ok((start, end))
    } else {
        let n = parse_lid(spec)?;
        Ok((n, n))
    }
}

fn parse_lid(s: &str) -> Result<usize, String> {
    let n: usize = s.trim().parse().map_err(|_| {
        format!(
            "hashline: bad line id {s:?} — a line id is the bare 1-based number shown \
                 in the read_file hashline gutter (e.g. `255`), and a range is `A.=B` \
                 (e.g. `255.=257`); `=`, `-`, `:` and `#hash` suffixes are not ranges"
        )
    })?;
    if n == 0 {
        return Err("hashline: line ids are 1-based (got 0)".to_string());
    }
    Ok(n)
}

fn parse_op<'a, I>(head: &str, lines: &mut std::iter::Peekable<I>) -> Result<Op, String>
where
    I: Iterator<Item = &'a str>,
{
    // Op headers optionally end in ':' to introduce a body.
    if let Some(rest) = head.strip_prefix("SWAP.BLK ") {
        let at = parse_lid(rest.trim().trim_end_matches(':'))?;
        let body = take_body(lines);
        return Ok(Op::SwapBlk { at, body });
    }
    if let Some(rest) = head.strip_prefix("DEL.BLK ") {
        let at = parse_lid(rest.trim())?;
        return Ok(Op::DelBlk { at });
    }
    if let Some(rest) = head.strip_prefix("INS.BLK.POST ") {
        let at = parse_lid(rest.trim().trim_end_matches(':'))?;
        let body = take_body(lines);
        return Ok(Op::InsBlkPost { at, body });
    }
    if let Some(rest) = head.strip_prefix("SWAP ") {
        let spec = rest.trim().trim_end_matches(':');
        let (start, end) = parse_range(spec)?;
        let body = take_body(lines);
        return Ok(Op::Swap { start, end, body });
    }
    if let Some(rest) = head.strip_prefix("DEL ") {
        let (start, end) = parse_range(rest.trim())?;
        return Ok(Op::Del { start, end });
    }
    if let Some(rest) = head.strip_prefix("INS.PRE ") {
        let at = parse_lid(rest.trim().trim_end_matches(':'))?;
        let body = take_body(lines);
        return Ok(Op::InsPre { at, body });
    }
    if let Some(rest) = head.strip_prefix("INS.POST ") {
        let at = parse_lid(rest.trim().trim_end_matches(':'))?;
        let body = take_body(lines);
        return Ok(Op::InsPost { at, body });
    }
    if head == "INS.HEAD:" || head == "INS.HEAD" {
        let body = take_body(lines);
        return Ok(Op::InsHead { body });
    }
    if head == "INS.TAIL:" || head == "INS.TAIL" {
        let body = take_body(lines);
        return Ok(Op::InsTail { body });
    }
    Err(format!("hashline: unrecognised op `{head}`"))
}

/// Apply a section against live file content and produce a [`SectionPlan`].
/// Outcome of planning one section, with an optional recovery note when the
/// live file drifted but anchors remapped cleanly onto it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlanOutcome {
    pub(crate) plan: SectionPlan,
    /// Set when the patch tag was stale and session snapshot recovery remapped
    /// anchors onto the live file. Surfaced on the apply receipt.
    pub(crate) recovery_note: Option<String>,
}

/// Verifies the section tag (stale-patch guard) before any transform. When the
/// tag is stale but a session snapshot still holds the version the model read,
/// attempts line-map recovery (fail closed on ambiguity).
#[cfg(test)]
pub(crate) fn plan_section(current: &str, section: &Section) -> Result<SectionPlan, String> {
    Ok(plan_section_detailed(current, section)?.plan)
}

/// Like [`plan_section`], but also returns a recovery note when stale-tag
/// recovery succeeded so the dispatcher can surface it on the receipt.
pub(crate) fn plan_section_detailed(
    current: &str,
    section: &Section,
) -> Result<PlanOutcome, String> {
    let live = content_tag(current);
    let mut recovery_note = None;
    let base_text: String;
    let work_section: Section;

    if live == section.tag {
        base_text = current.to_string();
        work_section = section.clone();
    } else if section.remove {
        // REM on a drifted file is too dangerous to auto-recover — the model
        // may have intended to delete a different revision.
        return Err(format!(
            "hashline: stale patch for {} — file tag is #{live} but the patch was written \
             against #{}. Re-read the file before REM.",
            section.path, section.tag
        ));
    } else if let Some(snapshot) = lookup_snapshot(&section.path, &section.tag) {
        match try_recover_ops(&snapshot, current, section) {
            Ok((remapped, note)) => {
                base_text = current.to_string();
                work_section = remapped;
                recovery_note = Some(note);
            }
            Err(reason) => {
                return Err(format!(
                    "hashline: stale patch for {} — file tag is #{live} but the patch was written \
                     against #{}. Recovery failed ({reason}). Re-read the file and rebuild the edit.",
                    section.path, section.tag
                ));
            }
        }
    } else {
        return Err(format!(
            "hashline: stale patch for {} — file tag is #{live} but the patch was written \
             against #{}. No session snapshot for that tag; re-read the file and rebuild the edit.",
            section.path, section.tag
        ));
    }

    if work_section.remove {
        return Ok(PlanOutcome {
            plan: SectionPlan::Remove {
                path: work_section.path.clone(),
            },
            recovery_note,
        });
    }

    let content = if work_section.ops.is_empty() {
        // Pure MV — content is unchanged.
        base_text
    } else {
        apply_line_ops(&base_text, &work_section)?
    };

    if let Some(to) = &work_section.move_to {
        if to == &work_section.path {
            return Err(format!(
                "hashline: {}: MV destination equals source",
                work_section.path
            ));
        }
        return Ok(PlanOutcome {
            plan: SectionPlan::Move {
                from: work_section.path.clone(),
                to: to.clone(),
                content,
            },
            recovery_note,
        });
    }

    Ok(PlanOutcome {
        plan: SectionPlan::Update {
            path: work_section.path.clone(),
            content,
        },
        recovery_note,
    })
}

/// Build a map of unchanged 1-based lines from `prev` → `curr` via LCS of lines,
/// then remap every op anchor. All anchors must share one consistent offset and
/// the anchored line text must still match — fail closed otherwise.
fn try_recover_ops(
    snapshot: &str,
    live: &str,
    section: &Section,
) -> Result<(Section, String), String> {
    if section.ops.is_empty() && section.move_to.is_some() {
        // Pure rename: content is whatever is live; no anchors to remap.
        let mut remapped = section.clone();
        // Force the section tag to match live so apply_line_ops path is unused.
        remapped.tag = content_tag(live);
        return Ok((
            remapped,
            "recovered pure MV against drifted file (session snapshot)".into(),
        ));
    }
    if section.ops.is_empty() {
        return Err("no ops to recover".into());
    }

    let prev_norm = normalise_newlines(snapshot);
    let curr_norm = normalise_newlines(live);
    let prev_lines: Vec<&str> = prev_norm.lines().collect();
    let curr_lines: Vec<&str> = curr_norm.lines().collect();
    let line_map = build_line_map(&prev_lines, &curr_lines);

    let mut offsets: Vec<i64> = Vec::new();
    let mut remapped_ops = Vec::with_capacity(section.ops.len());

    let map_line = |line: usize, offsets: &mut Vec<i64>| -> Result<usize, String> {
        let mapped = line_map
            .get(&line)
            .copied()
            .ok_or_else(|| format!("anchor line {line} was deleted or changed in the live file"))?;
        // Anchored text must still match (line map only tracks equal lines, so
        // this is a belt-and-braces check).
        if prev_lines.get(line - 1) != curr_lines.get(mapped - 1) {
            return Err(format!("anchor line {line} text diverged"));
        }
        offsets.push(mapped as i64 - line as i64);
        Ok(mapped)
    };

    let map_range =
        |start: usize, end: usize, offsets: &mut Vec<i64>| -> Result<(usize, usize), String> {
            for line in start..=end {
                let _ = map_line(line, offsets)?;
            }
            let new_start = *line_map.get(&start).unwrap();
            let new_end = *line_map.get(&end).unwrap();
            if new_end < new_start || (new_end - new_start) != (end - start) {
                return Err(format!(
                    "range {start}.={end} did not stay contiguous after remap"
                ));
            }
            Ok((new_start, new_end))
        };

    for op in &section.ops {
        let remapped = match op {
            Op::Swap { start, end, body } => {
                let (s, e) = map_range(*start, *end, &mut offsets)?;
                Op::Swap {
                    start: s,
                    end: e,
                    body: body.clone(),
                }
            }
            Op::Del { start, end } => {
                let (s, e) = map_range(*start, *end, &mut offsets)?;
                Op::Del { start: s, end: e }
            }
            Op::InsPre { at, body } => Op::InsPre {
                at: map_line(*at, &mut offsets)?,
                body: body.clone(),
            },
            Op::InsPost { at, body } => Op::InsPost {
                at: map_line(*at, &mut offsets)?,
                body: body.clone(),
            },
            Op::InsHead { body } => Op::InsHead { body: body.clone() },
            Op::InsTail { body } => Op::InsTail { body: body.clone() },
            Op::SwapBlk { at, body } => Op::SwapBlk {
                at: map_line(*at, &mut offsets)?,
                body: body.clone(),
            },
            Op::DelBlk { at } => Op::DelBlk {
                at: map_line(*at, &mut offsets)?,
            },
            Op::InsBlkPost { at, body } => Op::InsBlkPost {
                at: map_line(*at, &mut offsets)?,
                body: body.clone(),
            },
        };
        remapped_ops.push(remapped);
    }

    if offsets.is_empty() {
        // Only head/tail inserts — safe to apply on live as-is.
    } else {
        let first = offsets[0];
        if !offsets.iter().all(|o| *o == first) {
            return Err("anchors moved by inconsistent offsets".into());
        }
    }

    let mut remapped = section.clone();
    remapped.ops = remapped_ops;
    remapped.tag = content_tag(live);
    let note = if offsets.first().copied().unwrap_or(0) == 0 {
        "recovered via session snapshot (external drift outside anchors)".into()
    } else {
        format!(
            "recovered via session snapshot (anchors remapped by {:+})",
            offsets[0]
        )
    };
    Ok((remapped, note))
}

/// Longest common subsequence of equal lines → map of 1-based prev line → curr.
/// O(n·m) on line counts; fine for the hashline annotate ceiling (~512 KiB).
fn build_line_map(prev: &[&str], curr: &[&str]) -> HashMap<usize, usize> {
    let n = prev.len();
    let m = curr.len();
    // Classic LCS DP with pair backtrack. Cap pathological blow-ups: if both
    // sides are huge, fall back to a greedy sequential match.
    const DP_CAP: usize = 4_000;
    if n == 0 || m == 0 {
        return HashMap::new();
    }
    if n > DP_CAP || m > DP_CAP {
        return greedy_line_map(prev, curr);
    }
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in 1..=n {
        for j in 1..=m {
            if prev[i - 1] == curr[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }
    let mut map = HashMap::new();
    let mut i = n;
    let mut j = m;
    while i > 0 && j > 0 {
        if prev[i - 1] == curr[j - 1] {
            map.insert(i, j);
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] >= dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    map
}

fn greedy_line_map(prev: &[&str], curr: &[&str]) -> HashMap<usize, usize> {
    let mut map = HashMap::new();
    let mut j = 0usize;
    for (i, line) in prev.iter().enumerate() {
        while j < curr.len() {
            if curr[j] == *line {
                map.insert(i + 1, j + 1);
                j += 1;
                break;
            }
            j += 1;
        }
    }
    map
}

/// Apply a section's line ops to `current` file content. Preserves the file's
/// dominant line ending and trailing-newline convention.
fn apply_line_ops(current: &str, section: &Section) -> Result<String, String> {
    let uses_crlf = current.contains("\r\n");
    let had_trailing_newline = current.ends_with('\n');
    let normalised = normalise_newlines(current);
    // `lines()` drops a trailing empty element for a trailing '\n'; that's the
    // behaviour we want (line count = content lines).
    let orig: Vec<&str> = normalised.lines().collect();
    let n = orig.len();

    let out = apply_ops(&orig, n, &section.ops, &section.path)?;

    let joined = out.join("\n");
    let mut result = if had_trailing_newline && !out.is_empty() {
        format!("{joined}\n")
    } else {
        joined
    };
    if uses_crlf {
        result = result.replace('\n', "\r\n");
    }
    Ok(result)
}

/// Resolve a multi-line construct beginning on 1-based line `at`.
///
/// Heuristics (first match wins):
/// 1. Markdown heading (`#`…`######`) → section until next same-or-higher heading
/// 2. Unmatched `{`/`[`/`(` on the opener line → brace-matched closer
/// 3. Indent block → take consecutive lines deeper-indented than the opener
///
/// Single-line constructs are rejected so the model uses plain SWAP/DEL/INS.
pub(crate) fn resolve_block(lines: &[&str], at: usize) -> Result<(usize, usize), String> {
    let n = lines.len();
    if at < 1 || at > n {
        return Err(format!(
            "hashline: block anchor line {at} out of bounds (file has {n} lines)"
        ));
    }
    let opener = lines[at - 1];

    if let Some(level) = markdown_heading_level(opener) {
        let mut end = at;
        for i in (at + 1)..=n {
            if let Some(next) = markdown_heading_level(lines[i - 1])
                && next <= level
            {
                break;
            }
            end = i;
        }
        if end == at {
            return Err(format!(
                "hashline: block at line {at} is a single-line heading with no body — \
                 use plain SWAP/DEL/INS"
            ));
        }
        return Ok((at, end));
    }

    if let Some(end) = match_brace_block(lines, at) {
        if end == at {
            return Err(format!(
                "hashline: block at line {at} closes on the same line — use plain SWAP/DEL/INS"
            ));
        }
        return Ok((at, end));
    }

    if let Some(end) = match_indent_block(lines, at) {
        if end == at {
            return Err(format!(
                "hashline: block at line {at} has no deeper-indented body — use plain SWAP/DEL/INS"
            ));
        }
        return Ok((at, end));
    }

    Err(format!(
        "hashline: cannot resolve multi-line block beginning at line {at} — \
         re-read and use plain line ranges, or point at a real block opener \
         ({{, [, (, heading, or indented suite)"
    ))
}

fn markdown_heading_level(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return None;
    }
    let level = trimmed.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    // Must be `# ` or bare `#` with rest of line, not `#!` shebang-ish.
    let rest = &trimmed[level..];
    if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
        Some(level)
    } else {
        None
    }
}

/// Brace-match from the first `{` on or after line `at`. Only curly braces
/// define a multi-line block — parentheses in `fn foo()` / `def bar():` must
/// not count as a same-line open/close (that would reject every function
/// header before indent resolution can run).
fn match_brace_block(lines: &[&str], at: usize) -> Option<usize> {
    let n = lines.len();
    let mut depth: i32 = 0;
    let mut started = false;
    for i in at..=n {
        let line = lines[i - 1];
        for ch in scan_code_chars(line) {
            match ch {
                '{' => {
                    depth += 1;
                    started = true;
                }
                '}' if started => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            }
        }
    }
    None
}

/// Yield structural code characters, skipping string/char literals and
/// line comments (`//`, `#` for Python-ish). Not a full lexer — good enough to
/// not trip on braces inside strings for the common case.
fn scan_code_chars(line: &str) -> impl Iterator<Item = char> + '_ {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut in_str: Option<u8> = None;
    let mut escape = false;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_str {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == q {
                in_str = None;
            }
            i += 1;
            continue;
        }
        // Line comments.
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            break;
        }
        if b == b'#' && !line.trim_start().starts_with("#!") {
            // Keep shebang-ish first-line hashes out; otherwise `#` starts a comment.
            // Only treat as comment when it looks like Python/shell (preceded by
            // whitespace or start), not `#[derive]` / rust attributes.
            let at_start = i == 0 || bytes[i - 1].is_ascii_whitespace();
            if at_start {
                break;
            }
        }
        if b == b'"' || b == b'\'' || b == b'`' {
            in_str = Some(b);
            i += 1;
            continue;
        }
        out.push(b as char);
        i += 1;
    }
    out.into_iter()
}

/// Indent suite: opener line establishes a baseline indent; consume following
/// blank lines and deeper-indented lines until a non-blank line at ≤ baseline.
fn match_indent_block(lines: &[&str], at: usize) -> Option<usize> {
    let n = lines.len();
    let baseline = leading_indent(lines[at - 1]);
    // Need at least one deeper line to count as a block.
    let mut end = at;
    let mut saw_body = false;
    for i in (at + 1)..=n {
        let line = lines[i - 1];
        if line.trim().is_empty() {
            // Blank lines are part of the suite only once we've seen body, and
            // only if something deeper or same-suite continues after.
            if saw_body {
                end = i;
            }
            continue;
        }
        let ind = leading_indent(line);
        if ind > baseline {
            saw_body = true;
            end = i;
        } else {
            break;
        }
    }
    if saw_body {
        // Trim trailing blanks from the suite so DEL.BLK doesn't eat the
        // separator blank before the next sibling.
        while end > at && lines[end - 1].trim().is_empty() {
            end -= 1;
        }
        Some(end)
    } else {
        None
    }
}

fn leading_indent(line: &str) -> usize {
    line.chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

fn apply_ops(orig: &[&str], n: usize, ops: &[Op], path: &str) -> Result<Vec<String>, String> {
    // Expand block ops into concrete line ranges first, against ORIGINAL lines.
    let mut expanded: Vec<Op> = Vec::with_capacity(ops.len());
    for op in ops {
        match op {
            Op::SwapBlk { at, body } => {
                let (start, end) =
                    resolve_block(orig, *at).map_err(|e| format!("hashline: {path}: {e}"))?;
                expanded.push(Op::Swap {
                    start,
                    end,
                    body: body.clone(),
                });
            }
            Op::DelBlk { at } => {
                let (start, end) =
                    resolve_block(orig, *at).map_err(|e| format!("hashline: {path}: {e}"))?;
                expanded.push(Op::Del { start, end });
            }
            Op::InsBlkPost { at, body } => {
                let (_start, end) =
                    resolve_block(orig, *at).map_err(|e| format!("hashline: {path}: {e}"))?;
                expanded.push(Op::InsPost {
                    at: end,
                    body: body.clone(),
                });
            }
            other => expanded.push(other.clone()),
        }
    }

    // Validate ranges and build per-line directives against ORIGINAL numbering.
    // `claimed[i]` marks original line i (1-based) as consumed by a SWAP/DEL so
    // overlaps are rejected rather than silently mis-applied.
    let mut claimed = vec![false; n + 2];
    let mut swap_start: Vec<Option<(usize, Vec<String>)>> = vec![None; n + 2];
    let mut del_start: Vec<Option<usize>> = vec![None; n + 2];
    let mut ins_pre: Vec<Vec<String>> = vec![Vec::new(); n + 2];
    let mut ins_post: Vec<Vec<String>> = vec![Vec::new(); n + 2];
    let mut head: Vec<String> = Vec::new();
    let mut tail: Vec<String> = Vec::new();

    let in_range = |a: usize, b: usize| -> Result<(), String> {
        if a < 1 || b > n {
            return Err(format!(
                "hashline: {path}: line range {a}.={b} out of bounds (file has {n} lines)"
            ));
        }
        Ok(())
    };

    for op in &expanded {
        match op {
            Op::Swap { start, end, body } => {
                in_range(*start, *end)?;
                for (offset, slot) in claimed[*start..=*end].iter_mut().enumerate() {
                    if *slot {
                        return Err(format!(
                            "hashline: {path}: overlapping edit at line {}",
                            *start + offset
                        ));
                    }
                    *slot = true;
                }
                swap_start[*start] = Some((*end, body.clone()));
            }
            Op::Del { start, end } => {
                in_range(*start, *end)?;
                for (offset, slot) in claimed[*start..=*end].iter_mut().enumerate() {
                    if *slot {
                        return Err(format!(
                            "hashline: {path}: overlapping edit at line {}",
                            *start + offset
                        ));
                    }
                    *slot = true;
                }
                del_start[*start] = Some(*end);
            }
            Op::InsPre { at, body } => {
                in_range(*at, *at)?;
                ins_pre[*at].extend(body.clone());
            }
            Op::InsPost { at, body } => {
                in_range(*at, *at)?;
                ins_post[*at].extend(body.clone());
            }
            Op::InsHead { body } => head.extend(body.clone()),
            Op::InsTail { body } => tail.extend(body.clone()),
            // Block ops already expanded.
            Op::SwapBlk { .. } | Op::DelBlk { .. } | Op::InsBlkPost { .. } => unreachable!(),
        }
    }

    let mut out: Vec<String> = Vec::new();
    out.extend(head);
    let mut i = 1usize;
    while i <= n {
        out.extend(ins_pre[i].iter().cloned());
        if let Some((end, body)) = &swap_start[i] {
            out.extend(body.iter().cloned());
            out.extend(ins_post[*end].iter().cloned());
            i = end + 1;
        } else if let Some(end) = del_start[i] {
            out.extend(ins_post[end].iter().cloned());
            i = end + 1;
        } else {
            out.push(orig[i - 1].to_string());
            out.extend(ins_post[i].iter().cloned());
            i += 1;
        }
    }
    out.extend(tail);
    Ok(out)
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/hashline__tests.rs"]
mod tests;
