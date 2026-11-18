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

const DIGEST_MAX_BYTES: usize = 2 * 1024;

pub(crate) const HANDOFF_RECEIPT_MARK: &str = "[handoff: job ";

pub(crate) fn enabled() -> bool {
    super::lane::is_treebeard() && env_flag("ANGEL_TREEBEARD_DIGEST", true)
}

pub(crate) fn per_hop() -> usize {
    crate::agent::compaction::fanout_width()
}

pub(crate) fn compactor(in_hand: &dyn Club, aux: &[Arc<dyn Club>]) -> Option<Arc<dyn Club>> {
    if !enabled() {
        return None;
    }
    helper(in_hand, aux)
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reading {
    Parked,
    Handoff,
}

pub(crate) struct Digesting {
    index: usize,
    label: String,
    reading: Reading,
    reply: mpsc::Receiver<Result<String, String>>,
    answer: Option<Result<String, String>>,
}

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
    let book_catalog = super::book::catalog::current();
    let _ = std::thread::Builder::new()
        .name("treebeard-digest".into())
        .spawn(move || {
            let _book_catalog = book_catalog.map(super::book::catalog::enter);
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

pub(crate) fn handoff_job(content: &str) -> Option<String> {
    let rest = content.trim_start().strip_prefix(HANDOFF_RECEIPT_MARK)?;
    let id: String = rest.chars().take_while(char::is_ascii_digit).collect();
    (!id.is_empty()).then(|| format!("job {id}"))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Landed {
    pub(crate) landed: usize,
    pub(crate) missed: usize,
    pub(crate) rolling: usize,
    pub(crate) bytes: usize,
}

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

fn render(reading: Reading, text: &str) -> String {
    let frame = match reading {
        Reading::Parked => COMPACTOR_DIGEST,
        Reading::Handoff => OVERSEER_READ,
    };
    format!("\n{}\n{}", frame.cells(), data(text))
}

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
