//! Carrying a loop's conversation from one iteration into the next.
//!
//! The loop controller hands every iteration a fresh conversation built from curated
//! state. That keeps a wandering model on course. It also makes a capable one pay to
//! find its place again each time: read the brief, re-open the files, re-run the
//! profile. Measured on a long optimisation task, that took 16% to 30% of the spend,
//! where a harness that kept one conversation spent 3%, and work that was not yet in
//! the notes was lost whenever an iteration ended.
//!
//! When carrying is on, the finished iteration's conversation is kept and the next
//! iteration is appended to it: the loop's updated state arrives as the next message
//! instead of opening a new conversation. That holds for a turn that ended in an answer
//! and for one stopped with its conversation whole (its time budget, a stalled stream,
//! an interrupt), so a loop that is paused and resumed goes on where it stopped. A
//! conversation past the size limit, a turn that failed or was declined, or a different
//! loop starts fresh as before.
//!
//! `ANGEL_LOOP_CARRY=1` carries for every loop, `=0` for none; unset, only a podrace
//! loop carries. `ANGEL_LOOP_CARRY_TOKENS` sets the size limit (estimated tokens).

use crate::agent::club::{ChatMsg, ChatRole};
use crate::agent::harness::TurnStopReason;
use std::sync::{Arc, Mutex};

struct Carry {
    loop_id: String,
    convo: Vec<ChatMsg>,
}

static CARRY: Mutex<Option<Carry>> = Mutex::new(None);

const DEFAULT_LIMIT_TOKENS: usize = 200_000;

/// Opens the appended message, so the model reads the loop's state as an update and
/// not as a new task.
pub(crate) const CONTINUES: &str = "The loop continues in this same conversation: everything \
     above still stands, and your working files are as you left them. The loop's updated \
     state for the next iteration follows.";

fn enabled(podrace: bool) -> bool {
    match std::env::var("ANGEL_LOOP_CARRY") {
        Ok(value) => {
            let value = value.trim().to_ascii_lowercase();
            if matches!(value.as_str(), "0" | "false" | "no" | "off") {
                false
            } else if matches!(value.as_str(), "1" | "true" | "yes" | "on") {
                true
            } else {
                podrace
            }
        }
        Err(_) => podrace,
    }
}

fn limit_tokens() -> usize {
    std::env::var("ANGEL_LOOP_CARRY_TOKENS")
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .filter(|limit| *limit > 0)
        .unwrap_or(DEFAULT_LIMIT_TOKENS)
}

fn estimated_tokens(convo: &[ChatMsg]) -> usize {
    convo
        .iter()
        .map(|msg| {
            let calls: usize = msg
                .tool_calls
                .iter()
                .map(|call| call.name.len() + call.args.to_string().len())
                .sum();
            (msg.content.len() + calls).div_ceil(4)
        })
        .sum()
}

/// Whether a turn that ended this way left a conversation the next iteration can go on in.
/// An answer does. So do the stops after which the cockpit keeps an ordinary turn's
/// conversation and invites the next message. Every other stop starts fresh.
pub(crate) fn continues_after(reason: TurnStopReason) -> bool {
    matches!(
        reason,
        TurnStopReason::Answer
            | TurnStopReason::Deadline
            | TurnStopReason::IdleTimeout
            | TurnStopReason::Interrupt
    )
}

/// Keeps a finished iteration's conversation for the next one, or drops what was kept
/// when carrying is off or the conversation has grown past the limit.
pub(crate) fn keep(loop_id: &str, podrace: bool, convo: Vec<ChatMsg>) {
    let kept = (enabled(podrace)
        && !loop_id.is_empty()
        && estimated_tokens(&convo) <= limit_tokens())
    .then(|| Carry {
            loop_id: loop_id.to_string(),
            convo,
        });
    *CARRY.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = kept;
}

/// Forgets any kept conversation: the next iteration starts fresh.
pub(crate) fn clear() {
    *CARRY.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
}

/// The conversation the next turn should run on. A loop iteration whose loop has a kept
/// conversation continues it: the kept messages, then this iteration's own messages
/// without their system prompt (the kept one already leads the conversation). Anything
/// else gets `fresh` untouched. The kept conversation is used once; the turn that
/// finishes hands back the longer one.
pub(crate) fn next_conversation(loop_owner: Option<&str>, fresh: Arc<[ChatMsg]>) -> Arc<[ChatMsg]> {
    let Some(loop_id) = loop_owner else {
        return fresh;
    };
    let kept = CARRY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();
    let Some(kept) = kept.filter(|kept| kept.loop_id == loop_id) else {
        return fresh;
    };
    let Some(first_user) = fresh.iter().position(|msg| msg.role == ChatRole::User) else {
        return fresh;
    };
    let mut convo = kept.convo;
    for (offset, msg) in fresh[first_user..].iter().enumerate() {
        if offset == 0 {
            let mut update = ChatMsg::user(format!("{CONTINUES}\n\n{}", msg.content));
            update.recovery_context = msg.recovery_context.clone();
            convo.push(update);
        } else {
            convo.push(msg.clone());
        }
    }
    Arc::from(convo)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/loop_carry__tests.rs"]
mod tests;
