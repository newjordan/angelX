//! The Treebeard compactor (`⡌`): a local seat that reads the bulk the lane
//! parks under a handle and writes the root a digest as the result lands, so
//! the details stay on the local box and the paid root carries only what it
//! needs to act.
//!
//! It is the same helper that runs background compaction — `ANGEL_COMPACT_URL`,
//! else a reachable local utility club when the in-hand driver is a paid link
//! or a fan-out pipeline — so one local model is the lane's rolling compactor:
//! digests at entry, summaries as the window fills. The in-hand driver never
//! digests for itself.
//!
//! Digests roll: the hop never waits for one. A digest that has landed by the
//! time the hop's own tools finish rides its receipt (`⡌⠃`), attached before
//! that message is ever sent. One still in flight is carried forward and, when
//! it lands, appended at the tail of a later request under its handle
//! (`⡌⠉`). Both are append-only, so the cached prefix never moves; nothing cuts
//! a digest off, and one that fails keeps the bare receipt. Digests still out
//! when the turn ends wait in the registry for the next turn.
//!
//! The same seat reads a foreground call the harness handed to the background
//! after it ran past its limit (`⡌⠑`), and that read rolls the same way: on
//! the hand-off receipt (`⡌⠋`) or later under the job's id.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};

use super::book::st_connected::{
    COMPACTOR, COMPACTOR_DIGEST, COMPACTOR_LATE, OVERSEER, OVERSEER_READ,
};
use super::context::{cap_text, env_flag};
use crate::agent::club::{ChatMsg, Club};

/// Ceiling on one digest as it rides the receipt: an eighth of the eager
/// offload floor it stands in for.
const DIGEST_MAX_BYTES: usize = 2 * 1024;

/// First line of a hand-off receipt (see `exec::HandedOff::receipt`).
pub(crate) const HANDOFF_RECEIPT_MARK: &str = "[handoff: job ";

/// Whether the lane digests parked bulk. Treebeard only; `ANGEL_TREEBEARD_DIGEST=0`
/// keeps bare receipts.
pub(crate) fn enabled() -> bool {
    super::lane::is_treebeard() && env_flag("ANGEL_TREEBEARD_DIGEST", true)
}

/// Most digests one hop starts; the rest keep bare receipts. Shares the
/// summarizer's fan-out width (`ANGEL_COMPACT_FANOUT`): it is the same helper.
pub(crate) fn per_hop() -> usize {
    crate::agent::compaction::fanout_width()
}

/// The compactor for a turn on `in_hand`, or `None` when the lane keeps bare
/// receipts: the explicit compaction helper, else a reachable local utility
/// club when the in-hand driver is paid or a fan-out.
pub(crate) fn compactor(in_hand: &dyn Club, aux: &[Arc<dyn Club>]) -> Option<Arc<dyn Club>> {
    if !enabled() {
        return None;
    }
    helper(in_hand, aux)
}

/// The seat that reads a handed-off job: the same helper as the compactor,
/// in any lane. `None` (no helper, or only the in-hand driver) leaves the
/// hand-off receipt with its raw facts.
pub(crate) fn overseer(in_hand: &dyn Club, aux: &[Arc<dyn Club>]) -> Option<Arc<dyn Club>> {
    helper(in_hand, aux)
}

fn helper(in_hand: &dyn Club, aux: &[Arc<dyn Club>]) -> Option<Arc<dyn Club>> {
    if let Some(helper) = super::compact::compaction_summarizer() {
        return Some(Arc::new(helper));
    }
    if super::compact::wants_cheap_summarizer(in_hand) {
        return super::compact::pick_local_summarizer(aux);
    }
    None
}

/// What a digest reads: a parked output, or a handed-off job.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reading {
    Parked,
    Handoff,
}

/// One digest in flight, for the tool message at `index` (its receipt) and
/// named by `label` (the handle, or the job) should it land after that
/// receipt was sent.
pub(crate) struct Digesting {
    index: usize,
    label: String,
    reading: Reading,
    reply: mpsc::Receiver<Result<String, String>>,
    /// An answer already taken off `reply` (see [`settle`]).
    answer: Option<Result<String, String>>,
}

/// Start digesting `body` (the output parked for `identity` under the handle
/// `label`) on its own thread. The seat reads `⡌⠁`; the call and the output
/// ride beside it as data, bounded by the summarizer's per-call ceiling.
pub(crate) fn start(
    helper: &Arc<dyn Club>,
    workspace: PathBuf,
    index: usize,
    label: &str,
    identity: &str,
    body: &str,
) -> Digesting {
    let input_max = crate::agent::compaction::configured_compact_chunk_tokens() * 4;
    let prompt = format!(
        "{}\n{identity}\n\n{}",
        COMPACTOR.cells(),
        cap_text(body, input_max, 0)
    );
    spawn(helper, workspace, index, label, Reading::Parked, prompt)
}

/// Start reading a handed-off job (`label`, e.g. `job 7`) from the facts its
/// receipt carries. The seat reads `⡌⠑`; the facts ride beside it as data.
pub(crate) fn start_handoff(
    helper: &Arc<dyn Club>,
    workspace: PathBuf,
    index: usize,
    label: &str,
    facts: &str,
) -> Digesting {
    let input_max = crate::agent::compaction::configured_compact_chunk_tokens() * 4;
    let prompt = format!(
        "{}\n{label}\n\n{}",
        OVERSEER.cells(),
        cap_text(facts, input_max, 0)
    );
    spawn(helper, workspace, index, label, Reading::Handoff, prompt)
}

fn spawn(
    helper: &Arc<dyn Club>,
    workspace: PathBuf,
    index: usize,
    label: &str,
    reading: Reading,
    prompt: String,
) -> Digesting {
    let (send, reply) = mpsc::channel();
    let helper = Arc::clone(helper);
    // A thread that cannot spawn drops `send`; that reads as a miss. Nobody
    // joins these threads: one still running at exit never holds the process.
    let _ = std::thread::Builder::new()
        .name("treebeard-digest".into())
        .spawn(move || {
            let answer = super::book::connect::chat(
                &*helper,
                &workspace,
                &[ChatMsg::user(prompt)],
                None,
                &AtomicBool::new(false),
            );
            let _ = send.send(answer);
        });
    Digesting {
        index,
        label: label.to_string(),
        reading,
        reply,
        answer: None,
    }
}

/// The job a tool result handed to the background (`job 7`), when it is a
/// hand-off receipt.
pub(crate) fn handoff_job(content: &str) -> Option<String> {
    let rest = content.trim_start().strip_prefix(HANDOFF_RECEIPT_MARK)?;
    let id: String = rest.chars().take_while(char::is_ascii_digit).collect();
    (!id.is_empty()).then(|| format!("job {id}"))
}

/// What a set of digests came to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Landed {
    pub(crate) landed: usize,
    pub(crate) missed: usize,
    /// Still in flight, carried forward.
    pub(crate) rolling: usize,
    /// Bytes the landed digests added to root history.
    pub(crate) bytes: usize,
}

/// A digest's answer if it has one now: `Some(Ok)` landed, `Some(Err)` a miss
/// (failed, empty, or its thread gone), `None` still in flight. Never waits.
fn poll(digesting: &mut Digesting) -> Option<Result<String, ()>> {
    let answer = match digesting.answer.take() {
        Some(answer) => answer,
        None => match digesting.reply.try_recv() {
            Ok(answer) => answer,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => return Some(Err(())),
        },
    };
    match answer {
        Ok(text) if !text.trim().is_empty() => Some(Ok(text)),
        _ => Some(Err(())),
    }
}

/// Block until every digest in `pending` has its answer (tests only: the
/// harness itself never waits on one).
#[cfg(test)]
pub(crate) fn settle(pending: &mut [Digesting]) {
    for digesting in pending {
        if digesting.answer.is_none()
            && let Ok(answer) = digesting.reply.recv()
        {
            digesting.answer = Some(answer);
        }
    }
}

/// Attach every digest of this hop that has already landed to its receipt
/// (the message is not yet sent, so this is cache-safe) and return the rest,
/// still in flight, to roll forward. Never waits.
pub(crate) fn attach_ready(
    history: &mut [ChatMsg],
    pending: Vec<Digesting>,
) -> (Landed, Vec<Digesting>) {
    let mut out = Landed::default();
    let mut rolling = Vec::new();
    for mut digesting in pending {
        match poll(&mut digesting) {
            None => rolling.push(digesting),
            Some(Err(())) => out.missed += 1,
            Some(Ok(text)) => {
                let note = render(digesting.reading, &text);
                let Some(message) = history.get_mut(digesting.index) else {
                    out.missed += 1;
                    continue;
                };
                message.content = format!("{}{note}", message.content).into();
                out.landed += 1;
                out.bytes += note.len();
            }
        }
    }
    out.rolling = rolling.len();
    (out, rolling)
}

/// Append every rolling digest that has landed since at the tail of
/// `history`, one Harness note each, named by its handle or job; keep the
/// rest rolling. Never waits; nothing already sent is rewritten.
pub(crate) fn drain_late(history: &mut Vec<ChatMsg>, rolling: &mut Vec<Digesting>) -> Landed {
    let mut out = Landed::default();
    let mut still = Vec::new();
    for mut digesting in std::mem::take(rolling) {
        match poll(&mut digesting) {
            None => still.push(digesting),
            Some(Err(())) => out.missed += 1,
            Some(Ok(text)) => {
                let note = render_late(digesting.reading, &digesting.label, &text);
                out.landed += 1;
                out.bytes += note.len();
                history.push(ChatMsg::harness(note));
            }
        }
    }
    out.rolling = still.len();
    *rolling = still;
    out
}

/// The digest as data: its braille broken so nothing it quotes reads as a
/// harness route, capped at [`DIGEST_MAX_BYTES`].
fn data(text: &str) -> String {
    let text = text.trim();
    let mut data = String::with_capacity(text.len());
    for ch in text.chars() {
        if super::book::ledger::is_cell(ch) {
            data.push('·');
        }
        data.push(ch);
    }
    cap_text(&data, DIGEST_MAX_BYTES, 0).to_string()
}

/// The digest as it rides its receipt: framed by `⡌⠃` (a parked output) or
/// `⡌⠋` (a handed-off job).
fn render(reading: Reading, text: &str) -> String {
    let frame = match reading {
        Reading::Parked => COMPACTOR_DIGEST,
        Reading::Handoff => OVERSEER_READ,
    };
    format!("\n{}\n{}", frame.cells(), data(text))
}

/// A digest that landed after its receipt was sent: `⡌⠉` (or `⡌⠋`) with the
/// handle (or job) beside the route, then the digest.
fn render_late(reading: Reading, label: &str, text: &str) -> String {
    let frame = match reading {
        Reading::Parked => COMPACTOR_LATE,
        Reading::Handoff => OVERSEER_READ,
    };
    format!("{} {label}\n{}", frame.cells(), data(text))
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/compactor__tests.rs"]
mod tests;
