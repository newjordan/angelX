//! Turn-owned maintenance work on the existing harness clock. These are work
//! durations, not an exclusive wall-time partition: off-thread compaction can
//! overlap model/tool waits. UI reflex/idle work is outside the harness turn.
use crate::club::{ChatMsg, ChatRole};
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
mod tests {
    use super::*;
    #[test]
    fn p06c_background_uses_owner_clock_and_accumulates_submillisecond_work() {
        let meter = Meter::default();
        let _scope = meter.enter(Instant::now());
        {
            let mut state = meter.0.lock().unwrap();
            state.nanos.insert("aging_ms", 1_500_000);
            state.nanos.insert("compaction_ms", 4_750_000);
        }
        let snapshot = meter.snapshot();
        assert_eq!(snapshot.aging_ms, 1);
        assert_eq!(snapshot.compaction_ms, 4);
        assert_eq!(snapshot.reflex_tick_ms, 0);
        assert_eq!(snapshot.idle_probe_ms, 0);
        let active = span("other_ms");
        assert_eq!(meter.0.lock().unwrap().active.len(), 1);
        drop(active);
        assert!(meter.0.lock().unwrap().active.is_empty());
    }
    #[test]
    fn p06c_retention_pairs_reused_ids_in_protocol_order() {
        let meter = Meter::default();
        let _scope = meter.enter(Instant::now());
        produced("same", 600, 600);
        produced("same", 800, 800);
        let mut history = vec![
            ChatMsg::tool("same", "x".repeat(600)),
            ChatMsg::tool("same", "x".repeat(800)),
        ];
        observe(&history);
        assert_eq!(
            super::super::super::trajectory::tools_output_snapshot().retained_bytes,
            1400
        );
        history[0].content = "x".repeat(100).into();
        observe(&history);
        history.remove(0);
        observe(&history);
        let output = super::super::super::trajectory::tools_output_snapshot();
        assert_eq!(
            output,
            ToolsOutput {
                produced_bytes: 1400,
                retained_bytes: 800,
                aged_bytes: 500,
                dropped_bytes: 100
            }
        );
    }
    #[test]
    fn p06c_retention_counts_admission_aging_removal_once_and_excludes_old_turn() {
        let meter = Meter::default();
        let _scope = meter.enter(Instant::now());
        let mut history = vec![ChatMsg::tool("old", "old output")];
        produced("new", 1000, 900);
        history.push(ChatMsg::tool("new", "x".repeat(900)));
        observe(&history);
        history[1].content = "é".repeat(100).into();
        observe(&history);
        observe(&history);
        let output = super::super::super::trajectory::tools_output_snapshot();
        assert_eq!(
            output,
            ToolsOutput {
                produced_bytes: 1000,
                retained_bytes: 200,
                aged_bytes: 700,
                dropped_bytes: 100
            }
        );
        history.pop();
        observe(&history);
        let output = super::super::super::trajectory::tools_output_snapshot();
        assert_eq!(output.dropped_bytes, 300);
        assert_eq!(output.retained_bytes, 0);
        assert_eq!(
            output.produced_bytes,
            output.retained_bytes + output.aged_bytes + output.dropped_bytes
        );
    }
}
