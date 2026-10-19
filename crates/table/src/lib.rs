//! The Round Table board: the pure rules of a host-arbitrated work
//! board for multiplayer agent seats. No IO, no threads — the server
//! ([`angel_serve`]) runs it, each seat's cockpit renders it, and the
//! same types cross the wire as an `angel_net` document tree.
//!
//! Governance in one paragraph: anyone seated may propose, comment and
//! vote; a claim takes an accepted issue for a bounded time; the
//! claimant submits a patch bundle (bytes + summary + test evidence);
//! only the arbiter (seat [`ARBITER`], the host) accepts, rejects,
//! records a merge, or closes. Votes are signal, never authority.
//! Every accepted mutation appends to an event log that exports as
//! JSONL and replays to the same board — the table's memory.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

pub mod act;
pub mod client;

/// The game's name in `Hello`/`Welcome`.
pub const GAME: &str = "round-table";
/// The arbiter's seat: the host's own. Only it accepts, rejects,
/// records merges, and closes.
pub const ARBITER: u32 = 0;
/// How long a claim holds, in server ticks (the table ticks at 2 Hz:
/// one hour). Past it the issue returns to `Accepted` for anyone.
pub const CLAIM_TTL_TICKS: u64 = 7_200;
/// The largest patch bundle a submission may carry.
pub const MAX_BUNDLE: usize = 4 * 1024 * 1024;
const MAX_TITLE: usize = 200;
const MAX_TEXT: usize = 8_000;
const MAX_NAME: usize = 64;

/// A per-tick command: the table has no held input — everything real
/// is an `action` ([`act`]). One zero byte keeps the link's command
/// stream (and its presence clock) alive.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TableCmd;

impl angel_net::cmd::Command for TableCmd {
    fn neutral() -> Self {
        TableCmd
    }
    fn held_only(&self) -> Self {
        TableCmd
    }
    fn absorb(&mut self, _older: &Self) {}
}

/// Where an issue stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    Proposed,
    Accepted,
    InProgress,
    Submitted,
    Merged,
    Closed,
    Rejected,
}

impl State {
    pub fn label(self) -> &'static str {
        match self {
            State::Proposed => "proposed",
            State::Accepted => "accepted",
            State::InProgress => "in-progress",
            State::Submitted => "submitted",
            State::Merged => "merged",
            State::Closed => "closed",
            State::Rejected => "rejected",
        }
    }
}

/// Who holds an issue, until when (server ticks).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    pub seat: u32,
    pub until: u64,
}

/// One spoken line on an issue — the v1 conversation surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    pub seat: u32,
    pub tick: u64,
    pub text: String,
}

/// A submitted patch set's public face. The bundle bytes themselves
/// live in [`Board::bundle`] (arbiter-side only, never published).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Submission {
    pub seat: u32,
    pub tick: u64,
    pub summary: String,
    pub evidence: String,
    pub bundle_hash: String,
    pub bundle_len: u64,
}

/// One issue before the table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Issue {
    pub id: u64,
    pub title: String,
    pub body: String,
    pub author: u32,
    pub state: State,
    pub claim: Option<Claim>,
    /// Advisory support/oppose per seat. Never authority.
    pub votes: BTreeMap<u32, bool>,
    pub comments: Vec<Comment>,
    pub submission: Option<Submission>,
    pub merged_commit: Option<String>,
    pub created: u64,
    pub updated: u64,
}

impl Issue {
    /// Supporting seats minus opposing seats.
    pub fn score(&self) -> i64 {
        self.votes.values().map(|v| if *v { 1 } else { -1 }).sum()
    }
    /// Whether at least `k` seats support this issue (advisory quorum).
    pub fn quorum(&self, k: usize) -> bool {
        self.votes.values().filter(|v| **v).count() >= k
    }
}

/// One thing a seat asks the table to do.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Op {
    NewIssue { title: String, body: String },
    Comment { id: u64, text: String },
    Vote { id: u64, support: bool },
    Claim { id: u64 },
    Release { id: u64 },
    Submit { id: u64, summary: String, evidence: String, bundle: Vec<u8> },
    Accept { id: u64 },
    Reject { id: u64 },
    Merge { id: u64, commit: String },
    Close { id: u64 },
}

/// Why an [`Op`] was refused. Refusals change nothing and log nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Denied {
    UnknownIssue,
    NotArbiter,
    NotClaimant,
    BadState(&'static str),
    Empty(&'static str),
    TooLong(&'static str),
    BundleTooLarge,
}

impl fmt::Display for Denied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Denied::UnknownIssue => f.write_str("no such issue"),
            Denied::NotArbiter => f.write_str("only the arbiter (the host seat) may do that"),
            Denied::NotClaimant => f.write_str("only the claiming seat may do that"),
            Denied::BadState(what) => write!(f, "the issue is {what}"),
            Denied::Empty(what) => write!(f, "{what} must not be empty"),
            Denied::TooLong(what) => write!(f, "{what} is too long"),
            Denied::BundleTooLarge => write!(f, "the patch bundle exceeds {MAX_BUNDLE} bytes"),
        }
    }
}

impl std::error::Error for Denied {}

/// One logged mutation. The log is the board's memory: replaying it
/// in order rebuilds the board (bundles included, base64 on export).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub seq: u64,
    pub tick: u64,
    pub seat: u32,
    pub op: Op,
}

/// The board: who is seated, what is proposed, and everything that
/// was ever decided. Serializes to the published document tree
/// (bundles and the log are arbiter-side only and skipped).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Board {
    next_id: u64,
    /// Seat number to display name, for everyone currently joined.
    pub seats: BTreeMap<u32, String>,
    pub issues: BTreeMap<u64, Issue>,
    #[serde(skip)]
    bundles: BTreeMap<u64, Vec<u8>>,
    #[serde(skip)]
    log: Vec<Event>,
}

impl Default for Board {
    fn default() -> Board {
        Board {
            // Issue ids start at 1 — a derived default of 0 made the
            // first issue #0 and every command naming #1 miss it.
            next_id: 1,
            seats: BTreeMap::new(),
            issues: BTreeMap::new(),
            bundles: BTreeMap::new(),
            log: Vec::new(),
        }
    }
}

impl Board {
    pub fn new() -> Board {
        Board::default()
    }

    /// The event log so far (arbiter-side; guests never receive it).
    pub fn log(&self) -> &[Event] {
        &self.log
    }

    /// A seat joins (or renames itself). Names are display only.
    pub fn join(&mut self, seat: u32, name: &str) {
        let mut name: String = name.trim().chars().take(MAX_NAME).collect();
        if name.is_empty() {
            name = format!("seat {seat}");
        }
        self.seats.insert(seat, name);
    }

    /// A seat leaves for good. Its claims stay until they expire.
    pub fn leave(&mut self, seat: u32) {
        self.seats.remove(&seat);
    }

    /// The patch bundle submitted for `id`, arbiter-side.
    pub fn bundle(&self, id: u64) -> Option<&[u8]> {
        self.bundles.get(&id).map(Vec::as_slice)
    }

    /// Let expired claims lapse back to `Accepted`. Call once a tick.
    pub fn expire(&mut self, tick: u64) {
        for issue in self.issues.values_mut() {
            if issue.state == State::InProgress
                && issue.claim.is_some_and(|c| tick > c.until)
            {
                issue.state = State::Accepted;
                issue.claim = None;
                issue.updated = tick;
            }
        }
    }

    fn issue_mut(&mut self, id: u64) -> Result<&mut Issue, Denied> {
        self.issues.get_mut(&id).ok_or(Denied::UnknownIssue)
    }

    fn arbiter(seat: u32) -> Result<(), Denied> {
        if seat == ARBITER { Ok(()) } else { Err(Denied::NotArbiter) }
    }

    /// Apply one seat's op at `tick`. On success the op is logged and
    /// its sequence number returned; on refusal nothing changes.
    pub fn apply(&mut self, seat: u32, tick: u64, op: Op) -> Result<u64, Denied> {
        self.expire(tick);
        match &op {
            Op::NewIssue { title, body } => {
                check_text(title, MAX_TITLE, "the title")?;
                check_text(body, MAX_TEXT, "the body")?;
                let id = self.next_id;
                self.next_id += 1;
                self.issues.insert(
                    id,
                    Issue {
                        id,
                        title: title.trim().to_string(),
                        body: body.clone(),
                        author: seat,
                        state: State::Proposed,
                        claim: None,
                        votes: BTreeMap::new(),
                        comments: Vec::new(),
                        submission: None,
                        merged_commit: None,
                        created: tick,
                        updated: tick,
                    },
                );
            }
            Op::Comment { id, text } => {
                check_text(text, MAX_TEXT, "the comment")?;
                let issue = self.issue_mut(*id)?;
                issue.comments.push(Comment { seat, tick, text: text.clone() });
                issue.updated = tick;
            }
            Op::Vote { id, support } => {
                let issue = self.issue_mut(*id)?;
                issue.votes.insert(seat, *support);
                issue.updated = tick;
            }
            Op::Claim { id } => {
                let issue = self.issue_mut(*id)?;
                if issue.state != State::Accepted {
                    return Err(Denied::BadState(issue.state.label()));
                }
                issue.state = State::InProgress;
                issue.claim = Some(Claim { seat, until: tick + CLAIM_TTL_TICKS });
                issue.updated = tick;
            }
            Op::Release { id } => {
                let issue = self.issue_mut(*id)?;
                if seat != ARBITER && !issue.claim.is_some_and(|c| c.seat == seat) {
                    return Err(Denied::NotClaimant);
                }
                if issue.state != State::InProgress {
                    return Err(Denied::BadState(issue.state.label()));
                }
                issue.state = State::Accepted;
                issue.claim = None;
                issue.updated = tick;
            }
            Op::Submit { id, summary, evidence, bundle } => {
                check_text(summary, MAX_TEXT, "the summary")?;
                check_text(evidence, MAX_TEXT, "the evidence")?;
                if bundle.is_empty() {
                    return Err(Denied::Empty("the patch bundle"));
                }
                if bundle.len() > MAX_BUNDLE {
                    return Err(Denied::BundleTooLarge);
                }
                let issue = self.issue_mut(*id)?;
                if !issue.claim.is_some_and(|c| c.seat == seat) {
                    return Err(Denied::NotClaimant);
                }
                if issue.state != State::InProgress {
                    return Err(Denied::BadState(issue.state.label()));
                }
                issue.submission = Some(Submission {
                    seat,
                    tick,
                    summary: summary.clone(),
                    evidence: evidence.clone(),
                    bundle_hash: bundle_hash(bundle),
                    bundle_len: bundle.len() as u64,
                });
                issue.state = State::Submitted;
                issue.updated = tick;
                self.bundles.insert(*id, bundle.clone());
            }
            Op::Accept { id } => {
                Self::arbiter(seat)?;
                let issue = self.issue_mut(*id)?;
                if issue.state != State::Proposed {
                    return Err(Denied::BadState(issue.state.label()));
                }
                issue.state = State::Accepted;
                issue.updated = tick;
            }
            Op::Reject { id } => {
                Self::arbiter(seat)?;
                let issue = self.issue_mut(*id)?;
                if matches!(issue.state, State::Merged | State::Closed | State::Rejected) {
                    return Err(Denied::BadState(issue.state.label()));
                }
                issue.state = State::Rejected;
                issue.claim = None;
                issue.updated = tick;
            }
            Op::Merge { id, commit } => {
                Self::arbiter(seat)?;
                check_text(commit, MAX_TITLE, "the commit")?;
                let issue = self.issue_mut(*id)?;
                if issue.state != State::Submitted {
                    return Err(Denied::BadState(issue.state.label()));
                }
                issue.state = State::Merged;
                issue.merged_commit = Some(commit.trim().to_string());
                issue.claim = None;
                issue.updated = tick;
            }
            Op::Close { id } => {
                Self::arbiter(seat)?;
                let issue = self.issue_mut(*id)?;
                if matches!(issue.state, State::Closed | State::Rejected) {
                    return Err(Denied::BadState(issue.state.label()));
                }
                issue.state = State::Closed;
                issue.claim = None;
                issue.updated = tick;
            }
        }
        let seq = self.log.len() as u64 + 1;
        self.log.push(Event { seq, tick, seat, op });
        Ok(seq)
    }

    /// The board as the published document tree.
    pub fn to_value(&self) -> angel_net::doc::value::Value {
        angel_net::doc::value::to_value(self).unwrap_or(angel_net::doc::value::Value::Unit)
    }

    /// A board read back from a published tree (bundles/log empty:
    /// they never cross the wire).
    pub fn from_value(tree: &angel_net::doc::value::Value) -> Option<Board> {
        angel_net::doc::value::from_value(tree).ok()
    }

    /// The whole log as JSONL — one event a line, bundles base64.
    /// This is the board's portable memory: checkpoint and export.
    pub fn export_jsonl(&self) -> String {
        let mut out = String::new();
        for event in &self.log {
            let line = LogLine {
                seq: event.seq,
                tick: event.tick,
                seat: event.seat,
                op: LogOp::of(&event.op),
            };
            out.push_str(&serde_json::to_string(&line).expect("a log line serializes"));
            out.push('\n');
        }
        out
    }

    /// Rebuild a board by replaying an exported log. A line that no
    /// longer applies (a hand-edited log) stops the import there and
    /// says which line failed; nothing is silently dropped.
    pub fn import_jsonl(text: &str) -> Result<Board, String> {
        let mut board = Board::new();
        for (n, raw) in text.lines().enumerate() {
            if raw.trim().is_empty() {
                continue;
            }
            let line: LogLine =
                serde_json::from_str(raw).map_err(|e| format!("line {}: {e}", n + 1))?;
            let op = line.op.into_op().map_err(|e| format!("line {}: {e}", n + 1))?;
            board
                .apply(line.seat, line.tick, op)
                .map_err(|e| format!("line {}: {e}", n + 1))?;
        }
        Ok(board)
    }
}

fn check_text(text: &str, max: usize, what: &'static str) -> Result<(), Denied> {
    if text.trim().is_empty() {
        return Err(Denied::Empty(what));
    }
    if text.chars().count() > max {
        return Err(Denied::TooLong(what));
    }
    Ok(())
}

/// A bundle's content name: FNV-1a 64 over its bytes, hex. A name,
/// not a security boundary (the arbiter reads every diff anyway).
pub fn bundle_hash(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// An [`Op`] as it is written to the log file: like the op, but a
/// submission's bundle travels base64 instead of raw bytes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum LogOp {
    NewIssue { title: String, body: String },
    Comment { id: u64, text: String },
    Vote { id: u64, support: bool },
    Claim { id: u64 },
    Release { id: u64 },
    Submit { id: u64, summary: String, evidence: String, bundle_b64: String },
    Accept { id: u64 },
    Reject { id: u64 },
    Merge { id: u64, commit: String },
    Close { id: u64 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct LogLine {
    seq: u64,
    tick: u64,
    seat: u32,
    op: LogOp,
}

impl LogOp {
    fn of(op: &Op) -> LogOp {
        match op {
            Op::NewIssue { title, body } => LogOp::NewIssue { title: title.clone(), body: body.clone() },
            Op::Comment { id, text } => LogOp::Comment { id: *id, text: text.clone() },
            Op::Vote { id, support } => LogOp::Vote { id: *id, support: *support },
            Op::Claim { id } => LogOp::Claim { id: *id },
            Op::Release { id } => LogOp::Release { id: *id },
            Op::Submit { id, summary, evidence, bundle } => LogOp::Submit {
                id: *id,
                summary: summary.clone(),
                evidence: evidence.clone(),
                bundle_b64: b64_encode(bundle),
            },
            Op::Accept { id } => LogOp::Accept { id: *id },
            Op::Reject { id } => LogOp::Reject { id: *id },
            Op::Merge { id, commit } => LogOp::Merge { id: *id, commit: commit.clone() },
            Op::Close { id } => LogOp::Close { id: *id },
        }
    }

    fn into_op(self) -> Result<Op, String> {
        Ok(match self {
            LogOp::NewIssue { title, body } => Op::NewIssue { title, body },
            LogOp::Comment { id, text } => Op::Comment { id, text },
            LogOp::Vote { id, support } => Op::Vote { id, support },
            LogOp::Claim { id } => Op::Claim { id },
            LogOp::Release { id } => Op::Release { id },
            LogOp::Submit { id, summary, evidence, bundle_b64 } => Op::Submit {
                id,
                summary,
                evidence,
                bundle: b64_decode(&bundle_b64)?,
            },
            LogOp::Accept { id } => Op::Accept { id },
            LogOp::Reject { id } => Op::Reject { id },
            LogOp::Merge { id, commit } => Op::Merge { id, commit },
            LogOp::Close { id } => Op::Close { id },
        })
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            out.push(if i <= chunk.len() { B64[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

fn b64_decode(text: &str) -> Result<Vec<u8>, String> {
    let mut vals = Vec::with_capacity(text.len());
    for c in text.bytes() {
        match c {
            b'=' => break,
            _ => vals.push(B64.iter().position(|&b| b == c).ok_or("bad base64")? as u32),
        }
    }
    let mut out = Vec::with_capacity(vals.len() * 3 / 4 + 3);
    for chunk in vals.chunks(4) {
        let mut n: u32 = 0;
        for (i, v) in chunk.iter().enumerate() {
            n |= v << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seated_board() -> Board {
        let mut board = Board::new();
        board.join(ARBITER, "Host");
        board.join(1, "Percival");
        board.join(2, "Isolde");
        board
    }

    fn propose(board: &mut Board) -> u64 {
        board
            .apply(1, 10, Op::NewIssue { title: "Oil the gate".into(), body: "It squeaks.".into() })
            .unwrap();
        board.issues.keys().next().copied().unwrap()
    }

    #[test]
    fn the_whole_lifecycle_runs_under_the_arbiter() {
        let mut board = seated_board();
        let id = propose(&mut board);
        assert_eq!(board.issues[&id].state, State::Proposed);
        board.apply(2, 11, Op::Vote { id, support: true }).unwrap();
        board.apply(1, 11, Op::Vote { id, support: true }).unwrap();
        assert!(board.issues[&id].quorum(2));
        // A claim before acceptance is refused, by anyone.
        assert_eq!(board.apply(1, 12, Op::Claim { id }), Err(Denied::BadState("proposed")));
        board.apply(ARBITER, 13, Op::Accept { id }).unwrap();
        board.apply(1, 14, Op::Claim { id }).unwrap();
        assert_eq!(board.issues[&id].claim, Some(Claim { seat: 1, until: 14 + CLAIM_TTL_TICKS }));
        // Only the claimant submits.
        let submit = Op::Submit {
            id,
            summary: "oiled".into(),
            evidence: "cargo test green".into(),
            bundle: b"patch-bytes".to_vec(),
        };
        assert_eq!(board.apply(2, 15, submit.clone()), Err(Denied::NotClaimant));
        board.apply(1, 15, submit).unwrap();
        assert_eq!(board.issues[&id].state, State::Submitted);
        assert_eq!(board.bundle(id), Some(b"patch-bytes".as_slice()));
        assert_eq!(board.issues[&id].submission.as_ref().unwrap().bundle_hash, bundle_hash(b"patch-bytes"));
        // Only the arbiter records the merge.
        assert_eq!(
            board.apply(1, 16, Op::Merge { id, commit: "abc".into() }),
            Err(Denied::NotArbiter)
        );
        board.apply(ARBITER, 16, Op::Merge { id, commit: "abc123".into() }).unwrap();
        assert_eq!(board.issues[&id].state, State::Merged);
        assert_eq!(board.issues[&id].merged_commit.as_deref(), Some("abc123"));
        assert_eq!(board.log().len(), 7, "every accepted op logged, refusals not");
    }

    #[test]
    fn guests_cannot_accept_reject_or_close() {
        let mut board = seated_board();
        let id = propose(&mut board);
        for op in [Op::Accept { id }, Op::Reject { id }, Op::Close { id }] {
            assert_eq!(board.apply(1, 20, op), Err(Denied::NotArbiter));
        }
        assert_eq!(board.issues[&id].state, State::Proposed);
        assert_eq!(board.log().len(), 1);
    }

    #[test]
    fn claims_expire_and_release() {
        let mut board = seated_board();
        let id = propose(&mut board);
        board.apply(ARBITER, 1, Op::Accept { id }).unwrap();
        board.apply(1, 2, Op::Claim { id }).unwrap();
        // Someone else cannot release my claim.
        assert_eq!(board.apply(2, 3, Op::Release { id }), Err(Denied::NotClaimant));
        board.expire(2 + CLAIM_TTL_TICKS + 1);
        assert_eq!(board.issues[&id].state, State::Accepted);
        assert_eq!(board.issues[&id].claim, None);
        board.apply(2, 9_999, Op::Claim { id }).unwrap();
        board.apply(2, 10_000, Op::Release { id }).unwrap();
        assert_eq!(board.issues[&id].state, State::Accepted);
    }

    #[test]
    fn the_log_replays_to_the_same_board() {
        let mut board = seated_board();
        let id = propose(&mut board);
        board.apply(2, 11, Op::Comment { id, text: "seconded".into() }).unwrap();
        board.apply(ARBITER, 12, Op::Accept { id }).unwrap();
        board.apply(1, 13, Op::Claim { id }).unwrap();
        board
            .apply(1, 14, Op::Submit {
                id,
                summary: "done".into(),
                evidence: "ok".into(),
                bundle: vec![0, 1, 2, 250, 251, b'\n'],
            })
            .unwrap();
        let replayed = Board::import_jsonl(&board.export_jsonl()).unwrap();
        assert_eq!(replayed.issues, board.issues);
        assert_eq!(replayed.bundle(id), board.bundle(id), "bundles survive the log");
        assert_eq!(replayed.log().len(), board.log().len());
    }

    #[test]
    fn the_published_tree_hides_bundles_and_the_log() {
        let mut board = seated_board();
        let id = propose(&mut board);
        board.apply(ARBITER, 1, Op::Accept { id }).unwrap();
        board.apply(1, 2, Op::Claim { id }).unwrap();
        board
            .apply(1, 3, Op::Submit { id, summary: "s".into(), evidence: "e".into(), bundle: vec![9; 4] })
            .unwrap();
        let read = Board::from_value(&board.to_value()).expect("the tree reads back");
        assert_eq!(read.issues[&id].state, State::Submitted);
        assert_eq!(read.bundle(id), None, "bundle bytes never cross the wire");
        assert!(read.log().is_empty());
        assert_eq!(read.seats.get(&1).map(String::as_str), Some("Percival"));
    }

    #[test]
    fn a_default_board_starts_issue_ids_at_one() {
        // Regression: the host loads a fresh board via a fallback
        // default; with a derived `Default` its first issue was #0
        // and every command naming #1 reported "no such issue".
        let mut board = Board::default();
        board.apply(1, 1, Op::NewIssue { title: "First".into(), body: "b".into() }).unwrap();
        assert!(board.issues.contains_key(&1));
    }

    #[test]
    fn oversized_and_empty_inputs_are_refused() {
        let mut board = seated_board();
        assert_eq!(
            board.apply(1, 1, Op::NewIssue { title: " ".into(), body: "x".into() }),
            Err(Denied::Empty("the title"))
        );
        let id = propose(&mut board);
        board.apply(ARBITER, 1, Op::Accept { id }).unwrap();
        board.apply(1, 2, Op::Claim { id }).unwrap();
        assert_eq!(
            board.apply(1, 3, Op::Submit {
                id,
                summary: "s".into(),
                evidence: "e".into(),
                bundle: vec![0; MAX_BUNDLE + 1],
            }),
            Err(Denied::BundleTooLarge)
        );
        assert_eq!(
            board.apply(1, 4, Op::Comment { id: 999, text: "hi".into() }),
            Err(Denied::UnknownIssue)
        );
    }
}


