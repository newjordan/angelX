//! Turn-owned maintenance work on the existing harness clock. These are work
//! durations, not an exclusive wall-time partition: off-thread compaction can
//! overlap model/tool waits. UI reflex/idle work is outside the harness turn.
use crate::agent::club::{ChatMsg, ChatRole};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub(crate) struct BackgroundTelemetry {
    pub(crate) compaction_ms: u128,
    pub(crate) aging_ms: u128,
    pub(crate) reflex_tick_ms: u128,
    pub(crate) idle_probe_ms: u128,
    pub(crate) other_ms: u128,
}

/// UTF-8 bytes of results produced in this turn, after tool-internal limits
/// and post-write diagnostics, before the harness output cap. Prior-turn
/// history is excluded. Retained includes replacement receipts; aged is net
/// shrinkage in-place, dropped is initial admission loss or removed messages.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub(crate) struct ToolsOutput {
    pub(crate) produced_bytes: u64,
    pub(crate) retained_bytes: u64,
    pub(crate) aged_bytes: u64,
    pub(crate) dropped_bytes: u64,
}

#[derive(Default)]
struct State {
    origin: Option<Instant>,
    next_id: u64,
    active: HashMap<u64, (&'static str, u128)>,
    nanos: HashMap<&'static str, u128>,
    outputs: HashMap<String, Vec<u64>>,
    output: ToolsOutput,
}
#[derive(Clone, Default)]
pub(crate) struct Meter(Arc<Mutex<State>>);
thread_local! {
    static CURRENT: std::cell::RefCell<Option<Meter>> = const { std::cell::RefCell::new(None) };
}
pub(crate) struct Scope(Option<Meter>);
impl Drop for Scope {
    fn drop(&mut self) {
        CURRENT.with(|cell| *cell.borrow_mut() = self.0.take());
    }
}
impl Meter {
    #[cfg(test)]
    pub(super) fn note_for_test(&self, kind: &'static str, nanos: u128) {
        *self.0.lock().unwrap().nanos.entry(kind).or_default() += nanos;
    }

    pub(crate) fn enter(&self, origin: Instant) -> Scope {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).origin = Some(origin);
        Scope(CURRENT.with(|cell| cell.replace(Some(self.clone()))))
    }
    pub(crate) fn snapshot(&self) -> BackgroundTelemetry {
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let now = state.origin.map_or(0, |o| o.elapsed().as_nanos());
        let mut nanos = state.nanos.clone();
        for (kind, start) in state.active.values() {
            *nanos.entry(kind).or_default() += now.saturating_sub(*start);
        }
        let ms = |key| nanos.get(key).copied().unwrap_or(0) / 1_000_000;
        BackgroundTelemetry {
            compaction_ms: ms("compaction_ms"),
            aging_ms: ms("aging_ms"),
            reflex_tick_ms: ms("reflex_tick_ms"),
            idle_probe_ms: ms("idle_probe_ms"),
            other_ms: ms("other_ms"),
        }
    }
}
pub(crate) struct Span(Option<(Meter, u64)>);
impl Drop for Span {
    fn drop(&mut self) {
        if let Some((meter, id)) = self.0.take() {
            let mut state = meter.0.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((kind, start)) = state.active.remove(&id) {
                let now = state.origin.map_or(0, |o| o.elapsed().as_nanos());
                *state.nanos.entry(kind).or_default() += now.saturating_sub(start);
            }
        }
    }
}
pub(crate) fn span(kind: &'static str) -> Span {
    Span(CURRENT.with(|cell| {
        cell.borrow().as_ref().map(|meter| {
            let mut state = meter.0.lock().unwrap_or_else(|e| e.into_inner());
            let now = state.origin.map_or(0, |o| o.elapsed().as_nanos());
            let id = state.next_id;
            state.next_id += 1;
            state.active.insert(id, (kind, now));
            (meter.clone(), id)
        })
    }))
}
pub(crate) fn produced(id: &str, produced: u64, retained: u64) {
    CURRENT.with(|cell| {
        if let Some(meter) = cell.borrow().as_ref() {
            let mut state = meter.0.lock().unwrap_or_else(|e| e.into_inner());
            // Pair repeated provider IDs in protocol order, retaining every
            // result occurrence rather than overwriting an earlier call.
            state.outputs.entry(id.into()).or_default().push(retained);
            state.output.produced_bytes += produced;
            state.output.dropped_bytes += produced.saturating_sub(retained);
            state.output.retained_bytes = state.outputs.values().flatten().sum();
            super::super::trajectory::retain_tools_output(state.output.clone());
        }
    });
}
pub(crate) fn observe(history: &[ChatMsg]) {
    CURRENT.with(|cell| {
        if let Some(meter) = cell.borrow().as_ref() {
            let mut state = meter.0.lock().unwrap_or_else(|e| e.into_inner());
            if state.outputs.is_empty() {
                return;
            }
            let mut current: HashMap<&str, Vec<u64>> = HashMap::new();
            for message in history.iter().filter(|m| m.role == ChatRole::Tool) {
                if let Some(id) = message.tool_call_id.as_deref() {
                    current
                        .entry(id)
                        .or_default()
                        .push(message.content.len() as u64);
                }
            }
            let mut aged = 0;
            let mut dropped = 0;
            state.outputs.retain(|id, bytes| {
                let now = current.get(id.as_str()).map(Vec::as_slice).unwrap_or(&[]);
                // Context pruning/compaction removes the oldest complete
                // protocol region. Align suffixes; any older incoming-turn
                // history sharing an ID is outside this turn's inventory.
                let keep = bytes.len().min(now.len());
                let remove = bytes.len() - keep;
                dropped += bytes.drain(..remove).sum::<u64>();
                for (previous, current) in bytes.iter_mut().zip(&now[now.len() - keep..]) {
                    aged += previous.saturating_sub(*current);
                    *previous = *current;
                }
                !bytes.is_empty()
            });
            state.output.aged_bytes += aged;
            state.output.dropped_bytes += dropped;
            state.output.retained_bytes = state.outputs.values().flatten().sum();
            super::super::trajectory::retain_tools_output(state.output.clone());
        }
    });
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/turn__background__tests.rs"]
mod tests;
