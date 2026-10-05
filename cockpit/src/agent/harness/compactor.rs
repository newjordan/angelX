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
//! A digest is attached before its message is ever sent, so the cached prefix
//! never moves. The hop's digests run in parallel and the hop waits for every
//! one; nothing cuts a digest off. One that fails keeps the bare receipt.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};

use super::book::st_connected::{COMPACTOR, COMPACTOR_DIGEST};
use super::context::{cap_text, env_flag};
use crate::agent::club::{ChatMsg, Club};

/// Ceiling on one digest as it rides the receipt: an eighth of the eager
/// offload floor it stands in for.
const DIGEST_MAX_BYTES: usize = 2 * 1024;

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
    if let Some(helper) = super::compact::compaction_summarizer() {
        return Some(Arc::new(helper));
    }
    if super::compact::wants_cheap_summarizer(in_hand) {
        return super::compact::pick_local_summarizer(aux);
    }
    None
}

/// One digest in flight, for the tool message at `index`.
pub(crate) struct Digesting {
    index: usize,
    reply: mpsc::Receiver<Result<String, String>>,
}

/// Start digesting `body` (the output parked for `identity`) on its own
/// thread. The seat reads `⡌⠁`; the call and the output ride beside it as
/// data, bounded by the summarizer's per-call ceiling.
pub(crate) fn start(
    helper: &Arc<dyn Club>,
    workspace: PathBuf,
    index: usize,
    identity: &str,
    body: &str,
) -> Digesting {
    let input_max = crate::agent::compaction::configured_compact_chunk_tokens() * 4;
    let prompt = format!(
        "{}\n{identity}\n\n{}",
        COMPACTOR.cells(),
        cap_text(body, input_max, 0)
    );
    let (send, reply) = mpsc::channel();
    let helper = Arc::clone(helper);
    // A thread that cannot spawn drops `send`; the hop reads that as a miss.
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
    Digesting { index, reply }
}

/// What a hop's digests came to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Landed {
    pub(crate) landed: usize,
    pub(crate) missed: usize,
    /// Bytes the landed digests added to root history.
    pub(crate) bytes: usize,
}

/// Wait for every digest in `pending`, attaching each that lands to its
/// receipt. One that fails or comes back empty is a miss; its receipt stays
/// bare.
pub(crate) fn finish(history: &mut [ChatMsg], pending: Vec<Digesting>) -> Landed {
    let mut out = Landed::default();
    for digesting in pending {
        let note = match digesting.reply.recv() {
            Ok(Ok(text)) => render(&text),
            _ => None,
        };
        let Some(note) = note else {
            out.missed += 1;
            continue;
        };
        let Some(message) = history.get_mut(digesting.index) else {
            out.missed += 1;
            continue;
        };
        message.content = format!("{}{note}", message.content).into();
        out.landed += 1;
        out.bytes += note.len();
    }
    out
}

/// The digest as it rides the receipt: framed by `⡌⠃`, its braille broken so
/// nothing it quotes reads as a harness route, capped at [`DIGEST_MAX_BYTES`].
pub(crate) fn render(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let mut data = String::with_capacity(text.len());
    for ch in text.chars() {
        if super::book::ledger::is_cell(ch) {
            data.push('·');
        }
        data.push(ch);
    }
    Some(format!(
        "\n{}\n{}",
        COMPACTOR_DIGEST.cells(),
        cap_text(&data, DIGEST_MAX_BYTES, 0)
    ))
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/compactor__tests.rs"]
mod tests;
