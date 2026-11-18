//! Human-in-the-loop **approval gate** for the agent pipeline.
//!
//! With the swarm fanning out many agents, expensive/external actions (phoning a
//! SOTA model, an ssh remote test, a peer consult) shouldn't fire unattended —
//! but you also can't prompt once per agent. This broker makes approval:
//!
//! - **Coalesced** — the first request of an exact typed scope prompts; choosing
//!   **approve-all** caches only that host, peer, model, or action batch for the
//!   current turn. One decision covers an identical wave without spilling into
//!   a neighboring destination.
//! - **Token-free** — an approval is a UI round-trip, not a model call, and the
//!   decision is never folded into any agent's context. Only the action's actual
//!   result (e.g. the SOTA's answer) flows into the pipeline, exactly as before.
//! - **Non-blocking to the architecture** — the worker thread blocks on a
//!   one-shot reply channel; the TUI surfaces a modal and sends the decision
//!   back. Headless (no UI installed) denies, so nothing runs unattended and
//!   nothing hangs.
//!
//! Default-off: callers only invoke [`ask`] when their own approval flag is set
//! (e.g. `ANGEL_SWARM_APPROVE`), so the broker is inert otherwise.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Approve,
    Deny,
    ApproveAll,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ApprovalScope {
    SelfTest,
    RemoteHost(String),
    Peer(String),
    PhoneModel(String),
    ActionBatch(String),
}

impl ApprovalScope {
    pub fn label(&self) -> String {
        match self {
            Self::SelfTest => "self-test".to_string(),
            Self::RemoteHost(host) => format!("remote host · {}", safe_scope_fragment(host)),
            Self::Peer(peer) => format!("peer · {}", safe_scope_fragment(peer)),
            Self::PhoneModel(model) => format!("phone model · {}", safe_scope_fragment(model)),
            Self::ActionBatch(batch) => {
                format!("action batch · {}", safe_scope_fragment(batch))
            }
        }
    }
}

const SCOPE_FRAGMENT_MAX_CHARS: usize = 96;

fn safe_scope_fragment(value: &str) -> String {
    let mut out = String::new();
    let mut chars = 0usize;
    let mut pending_space = false;
    for character in value.chars() {
        if character.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if character.is_control() || unsafe_scope_format(character) {
            continue;
        }
        if pending_space {
            if chars + 1 >= SCOPE_FRAGMENT_MAX_CHARS {
                break;
            }
            out.push(' ');
            chars += 1;
            pending_space = false;
        }
        if chars >= SCOPE_FRAGMENT_MAX_CHARS {
            break;
        }
        out.push(character);
        chars += 1;
    }
    if out.is_empty() {
        "unspecified".to_string()
    } else {
        out
    }
}

fn unsafe_scope_format(character: char) -> bool {
    matches!(
        character,
        '\u{00ad}'
            | '\u{034f}'
            | '\u{061c}'
            | '\u{17b4}'..='\u{17b5}'
            | '\u{180b}'..='\u{180f}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{3164}'
            | '\u{fe00}'..='\u{fe0f}'
            | '\u{feff}'
            | '\u{fff9}'..='\u{fffb}'
            | '\u{e0100}'..='\u{e01ef}'
    )
}

pub struct Request {
    pub prompt: String,
    pub scope: ApprovalScope,
    pub reply: Sender<Decision>,
    pub deadline: std::time::Instant,
}

const REPLY_TIMEOUT: Duration = Duration::from_secs(600);

pub struct Broker {
    ui: Mutex<Option<Sender<Request>>>,
    state: Mutex<ApprovalState>,
}

#[derive(Default)]
struct ApprovalState {
    turn_epoch: u64,
    grants: HashMap<ApprovalScope, ApprovalGrant>,
}

struct ApprovalGrant {
    turn_epoch: u64,
    granted_at: std::time::Instant,
}

impl Default for Broker {
    fn default() -> Self {
        Self {
            ui: Mutex::new(None),
            state: Mutex::new(ApprovalState::default()),
        }
    }
}

impl Broker {
    fn set_ui(&self) -> Receiver<Request> {
        let (tx, rx) = channel();
        *self.ui.lock().unwrap() = Some(tx);
        rx
    }

    fn reset(&self) {
        let mut state = self.state.lock().unwrap();
        state.turn_epoch = state.turn_epoch.saturating_add(1);
        state.grants.clear();
    }

    fn ask(&self, scope: ApprovalScope, prompt: &str) -> Decision {
        let request_epoch = {
            let mut state = self.state.lock().unwrap();
            let epoch = state.turn_epoch;
            if state.grants.get(&scope).is_some_and(|grant| {
                grant.turn_epoch == epoch && grant.granted_at.elapsed() < REPLY_TIMEOUT
            }) {
                return Decision::Approve;
            }
            state.grants.remove(&scope);
            epoch
        };
        let ui = self.ui.lock().unwrap().clone();
        let Some(ui) = ui else {
            return Decision::Deny;
        };
        let (reply_tx, reply_rx) = channel();
        let req = Request {
            prompt: prompt.to_string(),
            scope: scope.clone(),
            reply: reply_tx,
            deadline: std::time::Instant::now() + REPLY_TIMEOUT,
        };
        if ui.send(req).is_err() {
            return Decision::Deny;
        }
        let reply = reply_rx.recv_timeout(REPLY_TIMEOUT);
        let mut state = self.state.lock().unwrap();
        if state.turn_epoch != request_epoch {
            return Decision::Deny;
        }
        match reply {
            Ok(Decision::ApproveAll) => {
                let turn_epoch = state.turn_epoch;
                state.grants.insert(
                    scope,
                    ApprovalGrant {
                        turn_epoch,
                        granted_at: std::time::Instant::now(),
                    },
                );
                Decision::Approve
            }
            Ok(d) => d,
            Err(_) => Decision::Deny,
        }
    }
}

fn broker() -> &'static Broker {
    static BROKER: OnceLock<Broker> = OnceLock::new();
    BROKER.get_or_init(Broker::default)
}

pub fn install_ui() -> Receiver<Request> {
    broker().set_ui()
}

pub fn reset_turn() {
    broker().reset();
}

pub fn ui_installed() -> bool {
    broker().ui.lock().unwrap().is_some()
}

pub fn ask(scope: ApprovalScope, prompt: &str) -> Decision {
    if crate::platform::yolo::enabled() {
        return Decision::Approve;
    }
    if crate::platform::yolo::smart_auto_approves(&scope) {
        return Decision::Approve;
    }
    broker().ask(scope, prompt)
}

#[cfg(test)]
pub(crate) fn test_reuse(first: ApprovalScope, next: ApprovalScope) -> Decision {
    let b = Broker::default();
    let ui = b.set_ui();
    std::thread::scope(|threads| {
        threads.spawn(move || ui.recv().unwrap().reply.send(Decision::ApproveAll).unwrap());
        assert_eq!(b.ask(first, "approve exact batch"), Decision::Approve);
    });
    b.ask(next, "later call")
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/approval__tests.rs"]
mod tests;
