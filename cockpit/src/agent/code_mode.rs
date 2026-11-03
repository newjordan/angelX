//! Host policy/scheduler facade for the independently compiled V8 bridge.
//!
//! Tool authority, effect barriers, concurrency and live worker provenance stay
//! here; `angel-code-mode` knows only the host callbacks. See
//! docs/performance/v8-compile-boundary-20261028.md.

use serde_json::Value;
use std::ops::Range;
use std::time::Duration;

pub use angel_code_mode::Outcome;
type ToolInvoker<'a> = dyn Fn(&str, &Value) -> Result<String, String> + Sync + 'a;
type BatchScheduler<'a> = dyn Fn(&[(String, Value)]) -> Vec<Range<usize>> + Sync + 'a;

/// Run with the existing host authority and footprint scheduler. No V8 types
/// cross this boundary, and no tool gains capabilities through the extraction.
pub fn run(
    script: &str,
    tool_names: &[String],
    invoke: &ToolInvoker<'_>,
    batch_scheduler: &BatchScheduler<'_>,
    timeout: Duration,
    heap_mb: usize,
) -> Result<Outcome, String> {
    let batch_runner = |specs: &[(String, Value)]| {
        run_scheduled_batch(invoke, specs, batch_scheduler(specs), batch_cap())
    };
    angel_code_mode::run(
        script,
        tool_names,
        invoke,
        &batch_runner,
        timeout,
        heap_mb,
        &batch_item_cap,
    )
}

/// Concurrency cap for `batch()`: `ANGEL_CODE_MODE_BATCH_CONCURRENCY`, else the
/// machine's parallelism clamped to 16.
fn batch_cap() -> usize {
    if let Ok(v) = std::env::var("ANGEL_CODE_MODE_BATCH_CONCURRENCY")
        && let Ok(n) = v.trim().parse::<usize>()
        && n > 0
    {
        return n;
    }
    std::thread::available_parallelism()
        .map(|n| n.get().min(16))
        .unwrap_or(8)
}

/// A separate construction bound prevents a script from materialising a huge
/// batch specification before the per-call dispatch budget can reject it.
fn batch_item_cap() -> usize {
    if crate::platform::yolo::enabled() {
        return usize::MAX;
    }
    std::env::var("ANGEL_CODE_MODE_MAX_BATCH_ITEMS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(48)
}

/// Run `specs` through `invoke` concurrently, `cap` at a time, preserving order.
/// Pure (no V8) so it's unit-testable. A worker panic becomes that item's error.
pub fn run_batch(
    invoke: &(dyn Fn(&str, &Value) -> Result<String, String> + Sync),
    specs: &[(String, Value)],
    cap: usize,
) -> Vec<Result<String, String>> {
    let cap = cap.max(1);
    let mut out: Vec<Result<String, String>> = Vec::with_capacity(specs.len());
    let mut idx = 0;
    while idx < specs.len() {
        let end = (idx + cap).min(specs.len());
        let chunk = &specs[idx..end];
        let chunk_out: Vec<Result<String, String>> = std::thread::scope(|s| {
            let handles: Vec<_> = chunk
                .iter()
                .map(|(name, a)| {
                    let model = crate::agent::harness::run_identity::live_model();
                    let turn = crate::agent::harness::run_identity::live_turn();
                    s.spawn(move || {
                        let _turn = crate::agent::harness::run_identity::LiveTurnScope::enter(turn);
                        let _model =
                            crate::agent::harness::run_identity::LiveModelScope::enter(model);
                        invoke(name, a)
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| {
                    h.join()
                        .unwrap_or_else(|_| Err("code_mode: batch worker panicked".to_string()))
                })
                .collect()
        });
        out.extend(chunk_out);
        idx = end;
    }
    out
}

/// Run consecutive scheduler segments in declaration order, concurrently
/// within multi-item segments. A malformed scheduler response fails closed to
/// one serial segment per call rather than risking reordered effects.
pub fn run_scheduled_batch(
    invoke: &(dyn Fn(&str, &Value) -> Result<String, String> + Sync),
    specs: &[(String, Value)],
    segments: Vec<Range<usize>>,
    cap: usize,
) -> Vec<Result<String, String>> {
    let mut cursor = 0;
    let valid = segments.iter().all(|segment| {
        let contiguous =
            segment.start == cursor && segment.start < segment.end && segment.end <= specs.len();
        cursor = segment.end;
        contiguous
    }) && cursor == specs.len();
    let segments = if valid {
        segments
    } else {
        (0..specs.len()).map(|index| index..index + 1).collect()
    };

    let mut out = Vec::with_capacity(specs.len());
    for segment in segments {
        if segment.len() > 1 {
            out.extend(run_batch(invoke, &specs[segment], cap));
        } else {
            let (name, args) = &specs[segment.start];
            out.push(invoke(name, args));
        }
    }
    out
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/code_mode__tests.rs"]
mod tests;
