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

/// The user's answer to an approval request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Allow this one action.
    Approve,
    /// Reject this one action.
    Deny,
    /// Allow this and every later action of the same exact scope this turn.
    ApproveAll,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ApprovalScope {
    SelfTest,
    RemoteHost(String),
    Peer(String),
    PhoneModel(String),
    ActionBatch(String),
    /// A turn gate is about to retract a finished answer. Unlike the other
    /// scopes this asks permission to *stop*, not to act, so denying it is the
    /// conservative outcome — see `harness::turn`.
    TurnGate(String),
}

impl ApprovalScope {
    /// Trusted one-line identity shown beside the human-authored prompt. The
    /// enum variant supplies the authority kind; untrusted endpoint fragments
    /// are bounded and stripped of terminal-confusing format controls.
    pub fn label(&self) -> String {
        match self {
            Self::SelfTest => "self-test".to_string(),
            Self::RemoteHost(host) => format!("remote host · {}", safe_scope_fragment(host)),
            Self::Peer(peer) => format!("peer · {}", safe_scope_fragment(peer)),
            Self::PhoneModel(model) => format!("phone model · {}", safe_scope_fragment(model)),
            Self::ActionBatch(batch) => {
                format!("action batch · {}", safe_scope_fragment(batch))
            }
            Self::TurnGate(gate) => format!("turn gate · {}", safe_scope_fragment(gate)),
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

/// A pending approval handed to the UI. The UI presents the human-authored
/// prompt beside a canonical label derived from `scope`, then fills `reply` with
/// the decision. Coalescing by exact scope happens in [`Broker::ask`] before the
/// request is sent.
pub struct Request {
    /// Human-readable description of what's about to happen.
    pub prompt: String,
    pub scope: ApprovalScope,
    /// One-shot channel the UI sends the [`Decision`] back on.
    pub reply: Sender<Decision>,
}

/// How long the worker waits for a decision before defaulting to deny (so a
/// closed/unresponsive UI can never hang a turn forever).
const REPLY_TIMEOUT: Duration = Duration::from_secs(600);

/// The approval broker: a UI inbox (None ⇒ headless) + the per-turn approve-all
/// cache. Kept as a struct so the decision logic is unit-testable without the
/// global / the real TUI.
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
    /// Point the broker at a UI; returns the receiver the UI polls for requests.
    fn set_ui(&self) -> Receiver<Request> {
        let (tx, rx) = channel();
        *self.ui.lock().unwrap() = Some(tx);
        rx
    }

    /// Clear per-turn state (the approve-all cache).
    fn reset(&self) {
        let mut state = self.state.lock().unwrap();
        state.turn_epoch = state.turn_epoch.saturating_add(1);
        state.grants.clear();
    }

    /// Ask for approval of an exact scope. Returns [`Decision::Approve`] or
    /// [`Decision::Deny`] (an approve-all is recorded and reported as Approve).
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
            return Decision::Deny; // headless: don't run external actions unattended
        };
        let (reply_tx, reply_rx) = channel();
        let req = Request {
            prompt: prompt.to_string(),
            scope: scope.clone(),
            reply: reply_tx,
        };
        if ui.send(req).is_err() {
            return Decision::Deny; // UI gone
        }
        let reply = reply_rx.recv_timeout(REPLY_TIMEOUT);
        let mut state = self.state.lock().unwrap();
        // A delayed UI reply cannot authorize an action from an expired turn,
        // nor seed the current turn's grant cache.
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
            Err(_) => Decision::Deny, // timed out / UI dropped the reply
        }
    }
}

fn broker() -> &'static Broker {
    static BROKER: OnceLock<Broker> = OnceLock::new();
    BROKER.get_or_init(Broker::default)
}

/// Install the global broker's UI inbox; returns the receiver the TUI polls.
pub fn install_ui() -> Receiver<Request> {
    broker().set_ui()
}

/// Clear the per-turn approve-all cache. Call once at the start of each turn.
pub fn reset_turn() {
    broker().reset();
}

/// Whether a TUI approval surface is attached. Choosers that want "popup when
/// an operator is present, silent default when unattended" branch on this —
/// calling [`ask`] headless would auto-deny, which is the wrong default for a
/// choice between two approved options.
pub fn ui_installed() -> bool {
    broker().ui.lock().unwrap().is_some()
}

/// Ask the user to approve an action, blocking until they answer. Coalesced by
/// exact scope; headless auto-denies. See the module docs.
pub fn ask(scope: ApprovalScope, prompt: &str) -> Decision {
    if crate::yolo::enabled() {
        return Decision::Approve;
    }
    // Smart YOLO: auto-approve workspace action batches and self-tests only.
    // Remote phone/peer/host scopes still prompt so spendy escalations stay human-gated.
    if crate::yolo::smart_auto_approves(&scope) {
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
mod tests {
    use super::*;
    use std::thread;

    /// Spawn a fake UI that answers each incoming request from a scripted list of
    /// decisions (in order), recording how many requests it actually received.
    fn fake_ui(
        b: &Broker,
        script: Vec<Decision>,
    ) -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let rx = b.set_ui();
        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        thread::spawn(move || {
            let mut it = script.into_iter();
            while let Ok(req) = rx.recv() {
                c.fetch_add(1, Ordering::SeqCst);
                let d = it.next().unwrap_or(Decision::Deny);
                let _ = req.reply.send(d);
            }
        });
        count
    }

    #[test]
    fn adversarial_approval_reply_cannot_cross_turn_reset() {
        let _lock = crate::tests::env_lock();
        for decision in [Decision::Approve, Decision::ApproveAll] {
            let b = Broker::default();
            let ui = b.set_ui();
            let scope = ApprovalScope::ActionBatch("exact-batch".into());
            thread::scope(|threads| {
                let worker = threads.spawn(|| b.ask(scope.clone(), "old turn"));
                let request = ui.recv().unwrap();
                b.reset();
                request.reply.send(decision).unwrap();
                assert_eq!(worker.join().unwrap(), Decision::Deny);
            });
            assert!(b.state.lock().unwrap().grants.is_empty());
        }
    }

    #[test]
    fn headless_denies() {
        let b = Broker::default();
        assert_eq!(
            b.ask(ApprovalScope::PhoneModel("sota".into()), "call SOTA?"),
            Decision::Deny
        );
    }

    #[test]
    fn scope_labels_are_typed_single_line_and_bounded() {
        assert_eq!(ApprovalScope::SelfTest.label(), "self-test");
        assert_eq!(
            ApprovalScope::RemoteHost(" spark\n\u{202e}evil\t host ".into()).label(),
            "remote host · spark evil host"
        );
        assert_eq!(
            ApprovalScope::Peer("\u{00ad}\u{200b}\u{fe0f}\n".into()).label(),
            "peer · unspecified"
        );
        assert_eq!(
            ApprovalScope::Peer("a\u{0007}b".into()).label(),
            "peer · ab"
        );
        let label = ApprovalScope::PhoneModel("x".repeat(200)).label();
        assert!(label.starts_with("phone model · "));
        assert_eq!(
            label.trim_start_matches("phone model · ").chars().count(),
            SCOPE_FRAGMENT_MAX_CHARS
        );
        assert!(!label.contains('\n'));
        let boundary = ApprovalScope::ActionBatch(format!("{} next", "x".repeat(95))).label();
        assert!(!boundary.ends_with(' '));
        assert_eq!(
            boundary
                .trim_start_matches("action batch · ")
                .chars()
                .count(),
            95
        );
    }

    #[test]
    fn yolo_global_ask_approves_without_a_ui() {
        let _guard = crate::tests::env_lock();
        let previous = std::env::var_os("ANGEL_YOLO");
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_YOLO", "1") };
        assert_eq!(
            ask(ApprovalScope::RemoteHost("spark".into()), "run it"),
            Decision::Approve
        );
        match previous {
            // TODO: Audit that the environment access only happens in single-threaded code.
            Some(value) => unsafe { std::env::set_var("ANGEL_YOLO", value) },
            // TODO: Audit that the environment access only happens in single-threaded code.
            None => unsafe { std::env::remove_var("ANGEL_YOLO") },
        }
    }

    #[test]
    fn approve_and_deny_pass_through() {
        let b = Broker::default();
        let _c = fake_ui(&b, vec![Decision::Approve, Decision::Deny]);
        let scope = ApprovalScope::PhoneModel("reasoner".into());
        assert_eq!(b.ask(scope.clone(), "1"), Decision::Approve);
        assert_eq!(b.ask(scope, "2"), Decision::Deny);
    }

    #[test]
    fn approve_all_coalesces_same_scope_without_reprompting() {
        let b = Broker::default();
        // The fake UI would only answer ONE request (then Deny). If coalescing
        // works, the 2nd/3rd asks never reach the UI.
        let count = fake_ui(&b, vec![Decision::ApproveAll]);
        let scope = ApprovalScope::PhoneModel("math".into());
        assert_eq!(b.ask(scope.clone(), "agent 1"), Decision::Approve);
        assert_eq!(b.ask(scope.clone(), "agent 2"), Decision::Approve);
        assert_eq!(b.ask(scope, "agent 3"), Decision::Approve);
        // give the UI thread a beat to have logged anything it received
        thread::sleep(Duration::from_millis(20));
        assert_eq!(
            count.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "only the first ask prompts"
        );
    }

    #[test]
    fn approve_all_is_exact_to_host_model_peer_or_batch() {
        let b = Broker::default();
        let count = fake_ui(
            &b,
            vec![Decision::ApproveAll, Decision::Deny, Decision::Deny],
        );
        let first = ApprovalScope::PhoneModel("model-a".into());
        assert_eq!(b.ask(first.clone(), "p"), Decision::Approve);
        assert_eq!(b.ask(first, "p2"), Decision::Approve);
        assert_eq!(
            b.ask(ApprovalScope::PhoneModel("model-b".into()), "p3"),
            Decision::Deny
        );
        assert_eq!(
            b.ask(ApprovalScope::RemoteHost("model-a".into()), "r"),
            Decision::Deny
        );
        thread::sleep(Duration::from_millis(20));
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[test]
    fn reset_clears_approve_all() {
        let b = Broker::default();
        let count = fake_ui(&b, vec![Decision::ApproveAll, Decision::ApproveAll]);
        let scope = ApprovalScope::ActionBatch("sha256:a".into());
        assert_eq!(b.ask(scope.clone(), "a"), Decision::Approve);
        assert_eq!(b.ask(scope.clone(), "b"), Decision::Approve); // cached
        b.reset();
        assert_eq!(b.ask(scope, "c"), Decision::Approve); // prompts again after reset
        thread::sleep(Duration::from_millis(20));
        assert_eq!(
            count.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "reset re-armed the prompt"
        );
    }

    #[test]
    fn stale_grant_expires_inside_the_same_turn() {
        let b = Broker::default();
        let scope = ApprovalScope::Peer("reviewer".into());
        let count = fake_ui(&b, vec![Decision::ApproveAll, Decision::Deny]);
        assert_eq!(b.ask(scope.clone(), "first"), Decision::Approve);
        b.state
            .lock()
            .unwrap()
            .grants
            .get_mut(&scope)
            .unwrap()
            .granted_at = std::time::Instant::now() - REPLY_TIMEOUT;
        assert_eq!(b.ask(scope, "expired"), Decision::Deny);
        thread::sleep(Duration::from_millis(20));
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 2);
    }
}
