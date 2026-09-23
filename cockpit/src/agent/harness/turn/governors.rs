use super::classify::feed_payload_value;
use super::*;
/// Pick the redirect injected once at the anti-spin nudge point: the active
/// [`SPIN_PERTURBATION`] reframe by default, the plain [`SPIN_NUDGE`] when
/// perturbation is disabled.
pub(crate) fn spin_redirect(perturb: bool) -> &'static str {
    if perturb {
        SPIN_PERTURBATION
    } else {
        SPIN_NUDGE
    }
}

/// Whether a hop's tool batch should advance the anti-spin counter.
///
/// A8: pure competition board wait/poll (no mutation, no submit/score outcome,
/// does not burn first-write) must not count — the same `head`/`read_file` of
/// the living handoff can become a new outcome when the shared slot flips, and
/// first-write already treats those hops as legal wait. Repeated submit/score
/// (outcome) and free-form recon still count as spin thrash.
pub(crate) fn anti_spin_counts_batch(
    mutation: bool,
    outcome: bool,
    wait_or_progress: bool,
    burns: bool,
) -> bool {
    if mutation || outcome || burns {
        return true;
    }
    // Pure wait/poll: wait_or_progress && !burns && !mutation && !outcome
    !wait_or_progress
}

fn parse_sleep_duration_secs(token: &str) -> Option<u64> {
    let token = token.trim_matches(['\'', '"', ')', '}', ',']);
    if token.eq_ignore_ascii_case("infinity") || token.eq_ignore_ascii_case("inf") {
        return Some(u64::MAX);
    }
    let (number, multiplier) = match token.as_bytes().last().copied() {
        Some(b's') | Some(b'S') => (&token[..token.len().saturating_sub(1)], 1.0),
        Some(b'm') | Some(b'M') => (&token[..token.len().saturating_sub(1)], 60.0),
        Some(b'h') | Some(b'H') => (&token[..token.len().saturating_sub(1)], 3600.0),
        Some(b'd') | Some(b'D') => (&token[..token.len().saturating_sub(1)], 86_400.0),
        _ => (token, 1.0),
    };
    let seconds = number.parse::<f64>().ok()? * multiplier;
    (seconds.is_finite() && seconds >= 0.0).then(|| seconds.ceil() as u64)
}

/// Return the longest direct `sleep DURATION` command in a shell chain.
/// Splitting only at shell control separators avoids treating prose such as
/// `echo sleep is bad` as a wait. Unknown/variable durations fail closed as an
/// unbounded wait because the harness cannot prove they are short.
pub(crate) fn shell_passive_sleep_secs(call: &ToolCall) -> Option<u64> {
    if call.name != "shell" {
        return None;
    }
    let command = crate::agent::tools::shell::shell_command_arg(&call.args)?;
    command
        .split([';', '|', '&', '\n'])
        .filter_map(|segment| {
            let mut words = segment
                .trim_start_matches(|c: char| c.is_ascii_whitespace() || matches!(c, '(' | '{'))
                .split_whitespace();
            let program = words.next()?.trim_matches(['\'', '"', '(', '{']);
            let base = program.rsplit('/').next().unwrap_or(program);
            if !base.eq_ignore_ascii_case("sleep") {
                return None;
            }
            Some(
                words
                    .next()
                    .and_then(parse_sleep_duration_secs)
                    .unwrap_or(u64::MAX),
            )
        })
        .max()
}

pub(super) fn poll_batch_advances_work(calls: &[ToolCall]) -> bool {
    calls.iter().any(|call| {
        is_first_write_progress_call(call)
            || is_local_preflight_call(call)
            || classify_tool_lane(call) == ToolLane::RunnerDispatch
            || matches!(call.name.as_str(), "proc_run" | "proc_stop")
    })
}

pub(super) fn passive_poll_only_batch(calls: &[ToolCall], max_sleep_secs: u64) -> bool {
    !calls.is_empty()
        && calls.iter().all(|call| {
            is_passive_status_call(call)
                || shell_passive_sleep_secs(call).is_some_and(|secs| secs > max_sleep_secs)
        })
}

/// Per-turn governor for model-owned waiting. One status-only batch may be
/// useful after real work; a second one without intervening candidate progress
/// is a loop. Competition polls while a slot is in flight are denied
/// immediately because the independent watcher has already probed that slot at
/// this hop boundary. Long explicit shell sleeps are also denied immediately.
#[derive(Default)]
pub(crate) struct PassivePollGuard {
    polls_since_progress: usize,
    denied_streak: usize,
}

/// Bound repeated model-owned polling even when timestamps/counters change in
/// the output. This is independent of opt-in suppression and exact-result spin
/// detection. Distinct inspections remain allowed; other work clears the window.
pub(crate) struct RepeatedPollGuard {
    limit: usize,
    recent: std::collections::VecDeque<u64>,
}

impl RepeatedPollGuard {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            limit: limit.min(32),
            recent: std::collections::VecDeque::new(),
        }
    }

    /// Called only after the complete batch is paired with its results. A real
    /// workspace change or non-poll batch retires all prior observations.
    pub(crate) fn observe(&mut self, fingerprint: Option<u64>, mutated: bool) -> bool {
        let Some(fingerprint) = fingerprint.filter(|_| !mutated && self.limit > 0) else {
            self.recent.clear();
            return false;
        };
        if self.recent.len() == 32 {
            self.recent.pop_front();
        }
        self.recent.push_back(fingerprint);
        self.recent
            .iter()
            .filter(|&&value| value == fingerprint)
            .count()
            >= self.limit
    }
}

impl PassivePollGuard {
    pub(crate) fn should_suppress(
        &mut self,
        calls: &[ToolCall],
        active: bool,
        cadence_verdict: Option<CadenceVerdict>,
        poll_limit: usize,
        max_sleep_secs: u64,
    ) -> bool {
        if !active {
            return false;
        }
        let blocks_on_sleep = calls
            .iter()
            .any(|call| shell_passive_sleep_secs(call).is_some_and(|secs| secs > max_sleep_secs));
        if blocks_on_sleep {
            if poll_batch_advances_work(calls) {
                self.polls_since_progress = 0;
                self.denied_streak = 0;
            }
            self.denied_streak = self.denied_streak.saturating_add(1);
            return true;
        }
        if poll_batch_advances_work(calls) {
            self.polls_since_progress = 0;
            self.denied_streak = 0;
            return false;
        }
        if !passive_poll_only_batch(calls, max_sleep_secs) {
            return false;
        }
        let watcher_owns_status = cadence_verdict == Some(CadenceVerdict::FailPollOnly);
        let suppress = watcher_owns_status || self.polls_since_progress >= poll_limit;
        self.polls_since_progress = self.polls_since_progress.saturating_add(1);
        if suppress {
            self.denied_streak = self.denied_streak.saturating_add(1);
        }
        suppress
    }

    /// Consecutive suppressed batches without intervening candidate progress.
    /// Drives the escalating model-facing denial receipt and the operator
    /// gauge; resets the moment a batch advances real work.
    pub(crate) fn denied_streak(&self) -> usize {
        self.denied_streak
    }
}

/// Stable anti-spin fingerprint for a tool batch (canonical JSON args so key
/// order cannot dodge the guard — same identity as toolcall-storm signatures).
#[cfg(test)]
pub(crate) fn anti_spin_batch_signature(calls: &[ToolCall]) -> String {
    calls
        .iter()
        .map(toolcall_storm_signature)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Hop-loop anti-spin identity. Hashes names + args in place so default hops
/// do not build or join storm strings. Key order is sorted; mutation bodies
/// hash raw (same uniqueness as [`payload_fingerprint`]).
pub(crate) fn anti_spin_batch_fingerprint(calls: &[ToolCall]) -> u64 {
    let mut hasher = DefaultHasher::new();
    calls.len().hash(&mut hasher);
    for call in calls {
        call.name.hash(&mut hasher);
        feed_payload_value(&mut hasher, &call.args);
    }
    hasher.finish()
}

/// Bounded detector for short alternating tool cycles (A→B→A→B, A→B→C→…).
/// The existing anti-spin guard catches only identical adjacent batches; this
/// retains at most `max_period * repeats` outcome-aware observations and looks
/// for periods 2..=max_period. Period 1 remains the cheaper existing guard's
/// responsibility.
pub(crate) struct ToolBatchCycle {
    observations: Vec<u64>,
    max_period: usize,
    repeats: usize,
}

impl ToolBatchCycle {
    pub(crate) fn new(max_period: usize, repeats: usize) -> Self {
        Self {
            observations: Vec::with_capacity(max_period.saturating_mul(repeats)),
            max_period,
            repeats,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.observations.clear();
    }

    #[cfg(test)]
    pub(crate) fn retained(&self) -> usize {
        self.observations.len()
    }

    /// Record one fully paired tool batch and return the detected cycle period.
    pub(crate) fn observe(&mut self, observation: u64) -> Option<usize> {
        let capacity = self.max_period.saturating_mul(self.repeats);
        if capacity == 0 {
            return None;
        }
        if self.observations.len() == capacity {
            self.observations.remove(0);
        }
        self.observations.push(observation);
        let len = self.observations.len();
        (2..=self.max_period).find(|period| {
            let needed = period.saturating_mul(self.repeats);
            if len < needed {
                return false;
            }
            let suffix = &self.observations[len - needed..];
            suffix
                .iter()
                .enumerate()
                .all(|(index, value)| *value == suffix[index % period])
        })
    }
}

/// Cycle identity from an already-hashed batch. Default hops hash args once
/// (anti-spin) and fold results here instead of walking write payloads twice.
pub(crate) fn tool_batch_cycle_observation_from_calls_fp(
    calls_fp: u64,
    results: &[(String, Option<Duration>, ToolOutcome)],
) -> u64 {
    let mut hasher = DefaultHasher::new();
    calls_fp.hash(&mut hasher);
    for (result, _, outcome) in results {
        result.hash(&mut hasher);
        outcome.execution.as_str().hash(&mut hasher);
        outcome.verification.as_str().hash(&mut hasher);
    }
    hasher.finish()
}

/// Hash calls together with their actual pre-cap results and typed outcomes.
/// A board/read whose output changes is progress, not a cycle, even when the
/// model repeats the same arguments. Elapsed time is deliberately excluded.
/// Call identity reuses [`anti_spin_batch_fingerprint`] so default hops do
/// not rebuild storm strings after anti-spin already hashed the batch.
#[cfg(test)]
pub(crate) fn tool_batch_cycle_observation(
    calls: &[ToolCall],
    results: &[(String, Option<Duration>, ToolOutcome)],
) -> u64 {
    tool_batch_cycle_observation_from_calls_fp(anti_spin_batch_fingerprint(calls), results)
}

/// Occurrence at which the opt-in storm guard answers a duplicate call itself
/// instead of dispatching it again.
pub(crate) const TOOLCALL_STORM_THRESHOLD: usize = 3;

/// Sliding-window duplicate-call tracker (opt-in, `ANGEL_TOOLCALL_STORM`).
/// Distinct from anti-spin, which compares the whole tool *batch* against the
/// immediately preceding one and ends the turn: this matches individual calls
/// across a window of hops and answers only the duplicate, so the rest of a
/// mixed batch keeps making progress.
///
/// A duplicate is the same call against the same workspace. A call's result
/// depends on the files it reads, so an edit starts a fresh window: an earlier
/// sighting described code that no longer exists. Without that, a plain
/// edit → test → edit → test loop had its third test run suppressed as
/// "unchanged" (polyglot-v1: 35 of 37 suppressions came right after an edit).
pub(crate) struct ToolCallStorm {
    hops: std::collections::VecDeque<Vec<(String, std::time::Instant)>>,
    window: usize,
}

impl ToolCallStorm {
    pub(crate) fn new(window: usize) -> Self {
        Self {
            hops: std::collections::VecDeque::new(),
            window: window.max(1),
        }
    }

    /// Fold one hop's batch into the window and return each call's occurrence
    /// count (1 = first sighting). Earlier duplicates in the same batch count,
    /// so a model that repeats a call inside one batch is caught too. A call
    /// that follows an edit in the same batch is compared only with what came
    /// after that edit.
    pub(crate) fn observe(&mut self, calls: &[ToolCall]) -> Vec<usize> {
        let observed_at = std::time::Instant::now();
        let mut counts = Vec::with_capacity(calls.len());
        let mut batch: Vec<(String, std::time::Instant)> = Vec::with_capacity(calls.len());
        let mut after_edit: Option<usize> = None;
        for call in calls {
            let signature = toolcall_storm_signature(call);
            let repeats = |(prior, seen_at): &&(String, std::time::Instant)| {
                prior == &signature
                    // A process snapshot is time-dependent. Real solver
                    // runs were denied status even minutes after a prior
                    // observation because too few model hops had elapsed.
                    // Keep rapid-poll suppression, but age these sightings.
                    && (!matches!(call.name.as_str(), "proc_status" | "proc_wait")
                        || observed_at.saturating_duration_since(*seen_at).as_secs() < 30)
            };
            let seen = match after_edit {
                Some(start) => batch[start..].iter().filter(repeats).count(),
                None => self
                    .hops
                    .iter()
                    .flatten()
                    .chain(batch.iter())
                    .filter(repeats)
                    .count(),
            };
            counts.push(seen + 1);
            batch.push((signature, observed_at));
            // Suppression is decided before dispatch, so an edit earlier in
            // the batch counts as a change even though it has not run yet.
            if is_mutation_call(call) {
                after_edit = Some(batch.len());
            }
        }
        self.hops.push_back(batch);
        while self.hops.len() > self.window {
            self.hops.pop_front();
        }
        counts
    }

    /// The workspace changed during the last hop, through a direct edit or an
    /// opaque one such as a shell `sed -i`. Every earlier sighting described
    /// files that are gone, so none of them can make a later call a duplicate.
    /// An identical rewrite that changed no bytes does not get here, so a
    /// storm of no-op writes is still caught.
    pub(crate) fn workspace_changed(&mut self) {
        self.hops.clear();
    }
}

/// Identity of a call for storm comparison: the tool name plus its arguments in
/// canonical form, so key order and whitespace cannot dodge the guard.
/// Mutation payloads are hashed so ordinary write hops do not allocate
/// megabyte anti-spin / storm strings.
pub(crate) fn toolcall_storm_signature(call: &ToolCall) -> String {
    let mut args = String::new();
    write_canonical_json(&call.name, &call.args, &mut args);
    format!("{}|{args}", call.name)
}

fn write_canonical_json(tool: &str, value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).unwrap_or_default());
                out.push(':');
                if is_competition_payload_key(tool, key) {
                    out.push('"');
                    out.push('#');
                    out.push_str(&payload_fingerprint(&map[key]));
                    out.push('"');
                } else {
                    write_canonical_json(tool, &map[key], out);
                }
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_canonical_json(tool, item, out);
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

/// The result a suppressed duplicate gets instead of a second execution.
pub(super) fn duplicate_storm_result(call: &ToolCall, count: usize) -> String {
    format!(
        "tool error: [duplicate call suppressed: {count}×] You have issued this exact `{}` call {count} times with identical arguments in the observation window; it was not executed again. No fresh result was read because workspace files have not changed. Do not repeat this call. You must edit the code using write_file or str_replace to fix the issue, or run a different command.",
        call.name
    )
}

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
