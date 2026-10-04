//! Nonblocking, model-free live input to the bundled Sloptomizer.
//! The turn supplies public hypothesis fragments and typed verifier receipts.
//! The learner's independent physical-experiment reward path is unchanged.

use super::{research_bridge as bridge, *};
use crate::agent::club::{ChatMsg, ChatRole, RouteIdentity, ToolCall};
use crate::agent::harness::{ExecutionOutcome, ToolOutcome, VerificationOutcome, book};
use serde_json::{Value, json};
use std::sync::mpsc::{self, SyncSender, TrySendError};

pub(super) fn scope(workspace: &Path, task: &str, verify: Option<&str>) -> PathBuf {
    // Deliberately model-independent context. The route stays on every receipt;
    // the existing route-specific fitness learner retains its own statistics.
    workspace_run_root(workspace)
        .join("research/relationships")
        .join(crate::knowledge::cut::sha256_hex(
            json!([task, verify]).to_string().as_bytes(),
        ))
}

pub(super) fn context(
    workspace: &Path,
    task: &str,
    verify: Option<&str>,
    cancel: &AtomicBool,
) -> Result<Value, String> {
    let root = scope(workspace, task, verify);
    if !root.join("state.json").exists() {
        return Ok(Value::Null);
    }
    bridge::transform(&root, json!({"action":"context"}), cancel).map(|v| v["advice"].clone())
}

#[derive(Default)]
struct Output {
    generation: u64,
    advice: Value,
    pending_notice: Option<Value>,
    error: Option<String>,
}

impl Output {
    fn publish(&mut self, mut advice: Value, root: &Path) {
        advice["state_path"] = json!(root.join("state.json"));
        if advice["signals"]
            .as_array()
            .is_some_and(|rows| !rows.is_empty())
        {
            self.pending_notice = Some(advice.clone());
        }
        self.advice = advice;
        self.error = None;
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct EvidenceStats {
    pub(crate) active: bool,
    pub(crate) observations: u32,
    pub(crate) checks: u32,
    pub(crate) contrasts: u32,
    pub(crate) inconclusive: u32,
}

impl EvidenceStats {
    fn from_advice(advice: &Value, active: bool) -> Self {
        let count = |key: &str| advice[key].as_u64().unwrap_or(0).min(u32::MAX as u64) as u32;
        Self {
            active,
            observations: count("relation_count"),
            checks: count("check_count"),
            contrasts: count("contrast_count"),
            inconclusive: count("inconclusive_count"),
        }
    }
}

/// Owned by one turn. A single worker drains bursts into one local transform;
/// Python, disk locks, and fsync never block the model/tool execution thread.
pub(crate) struct LiveResearch {
    root: PathBuf,
    enabled: bool,
    send: Option<SyncSender<Vec<Value>>>,
    output: Arc<Mutex<Output>>,
    session: String,
    nonce: String,
    sequence: u64,
    pending: Vec<Value>,
    seen: u64,
    capsule: Option<String>,
    error_seen: Option<String>,
    events: mpsc::Sender<crate::agent::harness::TurnEvent>,
}

impl LiveResearch {
    pub(crate) fn new(
        workspace: &Path,
        task: &str,
        verify: Option<&str>,
        session: &str,
        events: mpsc::Sender<crate::agent::harness::TurnEvent>,
    ) -> Self {
        let enabled = !task.trim().is_empty()
            // Synthetic unit turns opt in explicitly; they must not write
            // invented verifier evidence into an operator's real learner.
            && crate::agent::harness::env_flag("ANGEL_SLOPTOMIZER_LIVE", !cfg!(test));
        let mut live = Self {
            root: scope(workspace, task, verify),
            enabled,
            send: None,
            output: Arc::new(Mutex::new(Output::default())),
            session: session.to_string(),
            nonce: new_run_id(),
            sequence: 0,
            pending: Vec::new(),
            seen: 0,
            capsule: None,
            error_seen: None,
            events,
        };
        if enabled && live.root.join("state.json").exists() {
            live.start();
        }
        live
    }

    fn start(&mut self) {
        if !self.enabled || self.send.is_some() {
            return;
        }
        let (send, receive) = mpsc::sync_channel::<Vec<Value>>(32);
        let root = self.root.clone();
        let output = Arc::clone(&self.output);
        let launch = std::thread::Builder::new()
            .name("angel-sloptomizer-live".into())
            .spawn(move || {
                let cancel = AtomicBool::new(false);
                let publish = |result: Result<Value, String>| {
                    let mut out = output.lock().unwrap_or_else(|e| e.into_inner());
                    out.generation += 1;
                    match result {
                        Ok(value) => {
                            out.publish(value["advice"].clone(), &root);
                        }
                        Err(error) => out.error = Some(error),
                    }
                };
                if root.join("state.json").exists() {
                    publish(bridge::transform(
                        &root,
                        json!({"action":"context"}),
                        &cancel,
                    ));
                }
                while let Ok(mut events) = receive.recv() {
                    // Coalesce only what is already waiting; never wait for a batch.
                    for extra in receive.try_iter().take(31) {
                        events.extend(extra);
                    }
                    publish(bridge::transform(
                        &root,
                        json!({"action":"relate","events":events}),
                        &cancel,
                    ));
                }
            });
        match launch {
            Ok(_) => self.send = Some(send),
            Err(error) => {
                self.output.lock().unwrap_or_else(|e| e.into_inner()).error =
                    Some(error.to_string())
            }
        }
    }

    pub(crate) fn observe(
        &mut self,
        call: &ToolCall,
        outcome: ToolOutcome,
        result: &str,
        preamble: &str,
        route: &RouteIdentity,
    ) {
        if !self.enabled
            || !matches!(
                outcome.execution,
                ExecutionOutcome::Succeeded | ExecutionOutcome::Failed
            )
            || outcome.verification == VerificationOutcome::NotApplicable
        {
            return;
        }
        let hash = crate::knowledge::cut::sha256_hex;
        self.sequence += 1;
        let mut event = json!({
            // Some providers reuse call IDs after their old history compacts.
            // Each actual execution is distinct; retrying its queued event is
            // still idempotent at the durable store.
            "id": hash(format!("{}:{}:{}", self.nonce, self.sequence, call.id).as_bytes()),
            "check": hash(json!([call.name, call.args]).to_string().as_bytes()),
            "receipt": hash(result.as_bytes()), "tool": call.name,
            "verdict": outcome.verification.as_str(), "route": serde_json::to_string(route).unwrap_or_default(),
            "call_id": call.id, "session": self.session, "turn": self.nonce, "sequence": self.sequence,
            "check_note": excerpt(call.args["command"].as_str().unwrap_or(call.name.as_str()), false),
            "receipt_excerpt": excerpt(result, true),
        });
        if let Some(frame) = caveman_frame(preamble) {
            event["hypothesis"] = json!(frame[1]);
            event["check_note"] = json!(frame[2]);
            event["expected"] = json!(frame[3]);
        }
        self.pending.push(event);
    }

    pub(crate) fn flush(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        self.start();
        let Some(send) = &self.send else {
            return;
        };
        match send.try_send(std::mem::take(&mut self.pending)) {
            Ok(()) => {}
            Err(TrySendError::Full(mut rows)) => {
                if rows.len() > 256 {
                    rows.drain(..rows.len() - 256);
                    self.output.lock().unwrap_or_else(|e| e.into_inner()).error =
                        Some("live observation queue full; some receipts were not retained".into());
                }
                self.pending = rows;
            }
            Err(TrySendError::Disconnected(_)) => {
                self.output.lock().unwrap_or_else(|e| e.into_inner()).error = Some(
                    "live observation worker disconnected; tool receipts remain in the run".into(),
                );
                self.send = None;
            }
        }
    }

    /// Returns changed scalar telemetry, and at most one compact advice turn.
    /// A context splice/model change recalls the current frontier. Ordinary
    /// progress stays silent; repeated evidence and contrasts speak on events.
    pub(crate) fn poll(&mut self, workspace: &Path, history: &mut Vec<ChatMsg>, handoff: bool) {
        let mut out = self.output.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(error) = &out.error
            && self.error_seen.as_ref() != Some(error)
        {
            self.error_seen = Some(error.clone());
            let route = book::d12467_sloptomizer::UNAVAILABLE;
            history.push(ChatMsg::harness(format!(
                "⚠{}\n{}",
                route.cells(),
                json!({"error":error})
            )));
        }
        if out.advice["relation_count"].as_u64().unwrap_or(0) == 0 {
            return;
        }
        let fresh = out.generation != self.seen;
        if fresh {
            let _ = self
                .events
                .send(crate::agent::harness::TurnEvent::ResearchEvidence(
                    EvidenceStats::from_advice(&out.advice, true),
                ));
        }
        let missing = self.capsule.as_ref().is_none_or(|text| {
            !history.iter().any(|message| {
                message.role == ChatRole::Harness && message.content.as_ref() == text
            })
        });
        // A later quiet update cannot swallow a contrast/repetition signal
        // before the model has had its next request boundary.
        let notice = out.pending_notice.take();
        let warns = notice.is_some();
        if handoff || missing || warns {
            let turn = book::d12467_sloptomizer::context_turn(
                workspace,
                notice.as_ref().unwrap_or(&out.advice),
            );
            let existing = (!handoff && !warns)
                .then(|| {
                    history.iter().rev().find(|message| {
                        message.role == ChatRole::Harness
                            && message.content.starts_with("⚠⡫⠁")
                            && message.content.split_once('\n').map(|(_, data)| data)
                                == turn.split_once('\n').map(|(_, data)| data)
                    })
                })
                .flatten();
            if let Some(existing) = existing {
                // A new turn can reuse an unchanged card still in history.
                self.capsule = Some(existing.content.to_string());
            } else {
                history.push(ChatMsg::harness(turn.clone()));
                self.capsule = Some(turn);
            }
        }
        self.seen = out.generation;
    }
}

impl Drop for LiveResearch {
    fn drop(&mut self) {
        self.flush();
        // The worker never owns the turn's event sender. A slow optional
        // learner therefore cannot hold headless event draining/turn exit open.
        if let Ok(out) = self.output.try_lock()
            && out.advice["relation_count"].as_u64().unwrap_or(0) > 0
        {
            let _ = self
                .events
                .send(crate::agent::harness::TurnEvent::ResearchEvidence(
                    EvidenceStats::from_advice(&out.advice, false),
                ));
        }
    }
}

fn excerpt(text: &str, tail: bool) -> String {
    let redacted = crate::knowledge::barrel::redact_text(text).0;
    if tail {
        book::v_verification::tail_chars(&redacted, 240)
    } else {
        redacted.chars().take(240).collect()
    }
}

/// Optional parsing of the existing public caveman line. No private reasoning,
/// extra model request, format rejection, or invented hypothesis is involved.
fn caveman_frame(text: &str) -> Option<[String; 4]> {
    text.lines().find_map(|line| {
        let parts = line.split('→').map(str::trim).collect::<Vec<_>>();
        (parts.len() == 4 && parts.iter().all(|s| !s.is_empty()))
            .then(|| std::array::from_fn(|i| excerpt(parts[i], false)))
    })
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/rl_ctl__research_live__tests.rs"]
mod tests;
