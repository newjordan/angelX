use super::*;
/// Drive one agent turn to completion: keep letting the club call tools until it
/// produces a final text answer. `history` is mutated in place to include the
/// assistant turns and tool results, so the caller can persist the full thread.
///
/// The loop is **unbounded by default** — nex2-style extended reasoning runs for
/// as long as it needs (potentially days). Two controls:
/// - `cancel`: a cooperative **soft interrupt**, checked at each hop boundary
///   and propagated into blocking provider/tool calls. Process-owning and
///   streaming implementations can stop in flight; the turn records its clean
///   interrupt at the next owned boundary with `history` intact.
/// - `max_hops`: `None` = unbounded; `Some(n)` = a runaway guard, used for the
///   delegate sub-agents (bounded tasks), not the interactive driver.
#[allow(clippy::too_many_arguments)]
pub(super) fn dispatch_parallel_segment(
    registry: &ToolRegistry,
    hooks: &Hooks,
    calls: &[ToolCall],
    index_offset: usize,
    events: &mpsc::Sender<TurnEvent>,
    cancel: &AtomicBool,
    action_batch: Option<&ActionBatch>,
    sandbox_receipts: &std::sync::Mutex<Vec<Option<serde_json::Value>>>,
) -> Vec<(String, Option<Duration>, ToolOutcome)> {
    // Tool calls execute on scoped worker threads. Carry the root turn's
    // cumulative descendant allowance across that thread boundary so parallel
    // spawn/graph calls contend on one atomic budget.
    let _budget_scope = DescendantBudgetScope::enter_root();
    let descendant_budget = current_descendant_budget()
        .expect("parallel dispatch always runs inside a descendant budget scope");
    std::thread::scope(|scope| {
        let handles = calls
            .iter()
            .enumerate()
            .map(|(local_index, call)| {
                let ev = events.clone();
                let event_call = call.clone();
                let thread_budget = descendant_budget.clone();
                let http_context = crate::agent::tools::http_transport::context();
                let live_model = crate::agent::harness::run_identity::live_model();
                let live_turn = crate::agent::harness::run_identity::live_turn();
                let handle = scope.spawn(move || {
                    let _model =
                        crate::agent::harness::run_identity::LiveModelScope::enter(live_model);
                    let _turn =
                        crate::agent::harness::run_identity::LiveTurnScope::enter(live_turn);
                    let _descendant_budget = DescendantBudgetScope::inherit(thread_budget);
                    let _ = ev.send(TurnEvent::ToolCall {
                        id: ToolEventId(call.id.clone()),
                        name: call.name.clone(),
                        args_summary: summarize_args(&call.args),
                    });
                    crate::agent::tools::graph::emit_requested(
                        &ev,
                        &ToolEventId(call.id.clone()),
                        &call.name,
                        &call.args,
                    );
                    let preview =
                        action_batch.and_then(|batch| batch.contains(index_offset + local_index));
                    let started = Instant::now();
                    let outer_id = ToolEventId(call.id.clone());
                    let result =
                        crate::agent::tools::http_transport::with_context(http_context, || {
                            dispatch_with_hooks_events_cancel(
                                registry,
                                hooks,
                                &call.name,
                                &call.args,
                                Some((&outer_id, &ev)),
                                Some(cancel),
                            )
                        });
                    let elapsed = started.elapsed();
                    if let Some(preview) = preview {
                        let _ = ev.send(TurnEvent::Notice(
                            preview.receipt(&result, elapsed.as_millis()),
                        ));
                    }
                    let outcome = registry.executed_outcome(call, &result, false);
                    crate::agent::tools::graph::emit_returned(
                        &ev,
                        &ToolEventId(call.id.clone()),
                        &call.name,
                        &result,
                        outcome,
                    );
                    let _ = ev.send(TurnEvent::ToolResult {
                        id: ToolEventId(call.id.clone()),
                        name: call.name.clone(),
                        summary: summarize_result(&result),
                        outcome,
                    });
                    sandbox_receipts.lock().unwrap()[index_offset + local_index] =
                        crate::agent::harness::exec::sandbox_receipt();
                    (result, Some(elapsed), outcome)
                });
                (event_call, handle)
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|(call, handle)| {
                handle.join().unwrap_or_else(|_| {
                    let result = "tool error: worker panicked".to_string();
                    let outcome = ToolOutcome {
                        execution: ExecutionOutcome::Panicked,
                        verification: if is_verification_call(&call) {
                            VerificationOutcome::Failed
                        } else {
                            VerificationOutcome::NotApplicable
                        },
                    };
                    let _ = events.send(TurnEvent::ToolResult {
                        id: ToolEventId(call.id.clone()),
                        name: call.name.clone(),
                        summary: summarize_result(&result),
                        outcome,
                    });
                    (result, None, outcome)
                })
            })
            .collect()
    })
}

/// Allocation-free walk of every non-empty historical tool-call identity:
/// assistant `tool_calls[*].id`, then the tool-result `tool_call_id`.
fn historical_tool_ids(history: &[ChatMsg]) -> impl Iterator<Item = &str> {
    history.iter().flat_map(|message| {
        message
            .tool_calls
            .iter()
            .map(|call| call.id.as_str())
            .chain(message.tool_call_id.as_deref())
            .filter(|id| !id.trim().is_empty())
    })
}

/// True when `history` already used this tool-call identity.
/// Single-call hops (the common case) compare this one spelling — no set.
fn history_reuses_tool_id(history: &[ChatMsg], id: &str) -> bool {
    historical_tool_ids(history).any(|prior| prior == id)
}

/// True when any current-batch identity already appears in `history`.
/// Multi-call hops probe the batch set instead of walking `calls` per prior.
fn history_reuses_any_tool_id(history: &[ChatMsg], ids: &std::collections::HashSet<&str>) -> bool {
    historical_tool_ids(history).any(|prior| ids.contains(prior))
}

/// True when the hop loop must allocate a reserved-id set and rewrite
/// identities. Unique, non-empty, historically-fresh IDs skip that tax.
/// A single unique provider ID also skips the batch HashSet.
pub(crate) fn tool_call_ids_need_repair(calls: &[ToolCall], history: &[ChatMsg]) -> bool {
    if calls.is_empty() {
        return false;
    }
    if calls.len() == 1 {
        let id = calls[0].id.as_str();
        return id.trim().is_empty() || history_reuses_tool_id(history, id);
    }
    let mut seen = std::collections::HashSet::<&str>::with_capacity(calls.len());
    for call in calls {
        if call.id.trim().is_empty() || !seen.insert(call.id.as_str()) {
            return true;
        }
    }
    history_reuses_any_tool_id(history, &seen)
}

/// Repair provider tool-call identities before any policy accounting or
/// dispatch. IDs must be unique across the conversation for assistant-call /
/// tool-result pairing; valid, unused provider IDs remain byte-for-byte intact.
/// Generated IDs include the logical hop and batch index, then probe a bounded
/// local suffix until they avoid every historical and current provider ID.
/// Default hops with already-unique IDs skip the reserved-set clone.
pub(crate) fn normalize_tool_call_ids(
    calls: &mut [ToolCall],
    history: &[ChatMsg],
    hop: usize,
) -> usize {
    if !tool_call_ids_need_repair(calls, history) {
        return 0;
    }
    let mut reserved = std::collections::HashSet::<String>::new();
    reserved.extend(historical_tool_ids(history).map(str::to_owned));

    let historical = reserved.clone();
    let mut batch_counts = std::collections::HashMap::<String, usize>::new();
    for call in calls.iter() {
        if !call.id.trim().is_empty() {
            *batch_counts.entry(call.id.clone()).or_default() += 1;
            // Reserve every provider spelling before generating replacements,
            // including IDs that look exactly like our generated namespace.
            reserved.insert(call.id.clone());
        }
    }

    let mut repaired = 0usize;
    for (index, call) in calls.iter_mut().enumerate() {
        let invalid = call.id.trim().is_empty()
            || batch_counts.get(&call.id).copied().unwrap_or_default() > 1
            || historical.contains(&call.id);
        if !invalid {
            continue;
        }

        let base = format!("angel_h{hop}_call_{index}");
        let mut candidate = base.clone();
        let mut collision = 0usize;
        while reserved.contains(&candidate) {
            collision = collision.saturating_add(1);
            candidate = format!("{base}_{collision}");
        }
        reserved.insert(candidate.clone());
        call.id = candidate;
        repaired = repaired.saturating_add(1);
    }
    repaired
}
