//! Opt-in, turn-owned reservations. Cloned handles follow formation workers;
//! neither admission nor settlement relies on cumulative provider counters.
use crate::club::{AccountingSnapshot, UsageObservation};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Minimum share used for sizing; an allocation never denies admission.
pub(crate) const SHARE_FLOOR: u64 = 1024;

#[derive(Clone)]
pub(crate) struct Budget(Arc<Mutex<State>>);
struct State {
    tokens: Option<u64>,
    wall: Option<Duration>,
    start: Instant,
    reserved: u64,
    spent: u64,
    /// Nodes/seats not yet admitted. Share math divides remaining by this count.
    unstarted: u64,
    exhaustion_reason: Option<&'static str>,
    calls: Vec<Value>,
    observations: Vec<Option<UsageObservation>>,
    closed: bool,
    reservation_denial: Option<Value>,
    peak_over_allocation: u64,
    accounting_overflowed: bool,
    untracked_provider: bool,
}
thread_local! {
    static ACTIVE: RefCell<Option<Budget>> = const { RefCell::new(None) };
    static LAST: RefCell<Option<Value>> = const { RefCell::new(None) };
    static REQUEST_WALL: RefCell<Option<Instant>> = const { RefCell::new(None) };
    static ROLE: RefCell<String> = RefCell::new("single".into());
}

/// A graph worker carries its absolute deadline across every provider attempt.
pub(crate) struct RequestWallScope(Option<Instant>);
impl RequestWallScope {
    pub(crate) fn enter(deadline: Instant) -> Self {
        Self(REQUEST_WALL.with(|slot| {
            let deadline = slot.borrow().map_or(deadline, |old| old.min(deadline));
            slot.replace(Some(deadline))
        }))
    }
}
impl Drop for RequestWallScope {
    fn drop(&mut self) {
        REQUEST_WALL.with(|slot| *slot.borrow_mut() = self.0);
    }
}
pub(crate) fn request_wall_remaining() -> Option<Duration> {
    // Formation wall allocations are observations, not provider deadlines.
    REQUEST_WALL.with(|slot| {
        slot.borrow()
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
    })
}

pub(crate) struct RoleScope(String);
impl Drop for RoleScope {
    fn drop(&mut self) {
        ROLE.with(|role| *role.borrow_mut() = std::mem::take(&mut self.0));
    }
}
pub(crate) fn role() -> String {
    ROLE.with(|role| role.borrow().clone())
}
pub(crate) fn enter_role(value: &str) -> RoleScope {
    RoleScope(ROLE.with(|role| role.replace(value.into())))
}

pub(crate) struct Scope(Option<Budget>, bool);
impl Drop for Scope {
    fn drop(&mut self) {
        ACTIVE.with(|slot| {
            let current = slot.replace(self.0.take());
            LAST.with(|last| {
                *last.borrow_mut() = current.map(|budget| {
                    if self.1 {
                        budget.finish();
                    }
                    budget.snapshot()
                });
            });
        });
    }
}

pub(crate) fn current() -> Option<Budget> {
    ACTIVE.with(|slot| slot.borrow().clone())
}
pub(crate) fn enter(budget: Option<Budget>) -> Scope {
    Scope(ACTIVE.with(|slot| slot.replace(budget)), false)
}
pub(crate) fn start_turn() -> Result<Scope, String> {
    if let Some(budget) = current() {
        return Ok(enter(Some(budget)));
    }
    LAST.with(|last| *last.borrow_mut() = None);
    let tokens = parse("ANGEL_FORMATION_TOKEN_BUDGET")?;
    let wall = parse("ANGEL_FORMATION_WALL_SECS")?.map(Duration::from_secs);
    let mut scope = enter((tokens.is_some() || wall.is_some()).then(|| Budget::new(tokens, wall)));
    scope.1 = true;
    Ok(scope)
}
fn parse(key: &str) -> Result<Option<u64>, String> {
    match std::env::var(key) {
        Ok(value) => value
            .parse::<u64>()
            .map(Some)
            .map_err(|_| format!("{key} must be an unsigned integer")),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(_) => Err(format!("{key} must be an unsigned integer")),
    }
}
pub(crate) fn snapshot() -> Option<Value> {
    current()
        .map(|budget| budget.snapshot())
        .or_else(|| LAST.with(|last| last.borrow().clone()))
}
pub(crate) fn identity(budgets: &mut Value) {
    if let Some(budget) = current() {
        let state = budget.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(tokens) = state.tokens {
            budgets["formation_token_budget"] = json!(tokens);
        }
        if let Some(wall) = state.wall {
            budgets["formation_wall_secs"] = json!(wall.as_secs());
        }
    }
}
impl Budget {
    pub(crate) fn new(tokens: Option<u64>, wall: Option<Duration>) -> Self {
        Self(Arc::new(Mutex::new(State {
            tokens,
            wall,
            start: Instant::now(),
            reserved: 0,
            spent: 0,
            unstarted: 1,
            exhaustion_reason: None,
            calls: Vec::new(),
            observations: Vec::new(),
            closed: false,
            reservation_denial: None,
            peak_over_allocation: 0,
            accounting_overflowed: false,
            untracked_provider: false,
        })))
    }
    pub(crate) fn note_untracked_provider(&self) {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .untracked_provider = true;
    }
    pub(crate) fn available_tokens(&self) -> Option<u64> {
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state
            .tokens
            .map(|limit| limit.saturating_sub(state.spent.saturating_add(state.reserved)))
    }
    /// Remaining graph nodes or hop seats that have not reserved yet.
    pub(crate) fn set_unstarted(&self, n: u64) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).unstarted = n.max(1);
    }
    /// Output tokens this caller may take: min(provider cap, max(floor, remaining/unstarted)).
    pub(crate) fn share_output(&self, provider_cap: u64) -> Option<u64> {
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let available = state
            .tokens
            .map(|limit| limit.saturating_sub(state.spent.saturating_add(state.reserved)))?;
        let n = state.unstarted.max(1);
        Some(provider_cap.min(SHARE_FLOOR.max(available / n)))
    }
    #[cfg(test)]
    pub(crate) fn remaining_wall(&self) -> Option<Duration> {
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state
            .wall
            .map(|wall| wall.saturating_sub(state.start.elapsed()))
    }
    pub(crate) fn exhausted(&self) -> bool {
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.tokens.is_some_and(|tokens| state.spent > tokens)
    }

    pub(crate) fn reserve(
        &self,
        route: &str,
        prompt: u64,
        max_output: u64,
    ) -> Result<Reservation, String> {
        self.reserve_fitted(route, prompt, max_output, max_output)
    }

    /// Preserve the caller's output intent across HTTP wire-cap fitting. A
    /// dynamically reduced cap is not an operator's explicit small request.
    pub(crate) fn reserve_fitted(
        &self,
        route: &str,
        prompt: u64,
        max_output: u64,
        requested_output: u64,
    ) -> Result<Reservation, String> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let available = state
            .tokens
            .map(|limit| limit.saturating_sub(state.spent.saturating_add(state.reserved)));
        let want = prompt.saturating_add(max_output);
        let share = available.map(|available| SHARE_FLOOR.max(available / state.unstarted.max(1)));
        // Preserve share sizing for large requests, but never shrink below the
        // prompt plus a useful output floor. Explicit small wants are admitted.
        let amount = if state.unstarted > 1 || requested_output > SHARE_FLOOR {
            share.map_or(want, |share| {
                want.min(share.max(prompt.saturating_add(max_output.min(SHARE_FLOOR))))
            })
        } else {
            want
        };
        state.accounting_overflowed |= prompt.checked_add(max_output).is_none()
            || state.reserved.checked_add(amount).is_none();
        state.reserved = state.reserved.saturating_add(amount);
        state.accounting_overflowed |= state.spent.checked_add(state.reserved).is_none();
        let over = state.tokens.map_or(0, |limit| {
            state
                .spent
                .saturating_add(state.reserved)
                .saturating_sub(limit)
        });
        state.peak_over_allocation = state.peak_over_allocation.max(over);
        if over > 0 || state.accounting_overflowed {
            let record = json!({"route":route, "role":role(),
                "required_tokens":prompt.checked_add(max_output),
                "available_tokens":available, "token_budget":state.tokens,
                "over_allocation":true, "over_allocation_tokens":over,
                "admitted":true});
            state.reservation_denial.get_or_insert(record);
        }
        state.unstarted = state.unstarted.saturating_sub(1).max(1);
        let id = state.calls.len();
        let remaining = remaining_value(&state);
        let start_ms = state.start.elapsed().as_millis();
        let closed = state.closed;
        state.calls.push(
            json!({"id":id, "route":route, "role":role(), "prompt_estimate":prompt,
            "max_output":max_output, "requested_output":requested_output,
            "reserved":amount, "start_ms":start_ms,
            "required_tokens": prompt.checked_add(max_output),
            "remaining_before_reservation": available,
            "remaining_after_reservation":remaining, "over_allocation":over > 0,
            "over_allocation_tokens":over, "admitted_after_owner_exit":closed,
            "usage":null, "settled":false}),
        );
        state.observations.push(None);
        trace_budget("reserved", &state.calls[id]);
        Ok(Reservation {
            budget: self.clone(),
            id,
            amount,
        })
    }
    /// Close accounting at the owning turn boundary, including detached workers.
    fn finish(&self) {
        let pending = {
            let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
            state.closed = true;
            state
                .calls
                .iter()
                .enumerate()
                .filter(|(_, call)| call["settled"] != true)
                .map(|(id, call)| (id, call["reserved"].as_u64().unwrap()))
                .collect::<Vec<_>>()
        };
        for (id, amount) in pending {
            Reservation {
                budget: self.clone(),
                id,
                amount,
            }
            .settle_inner(None, "aborted");
        }
    }

    pub(crate) fn snapshot(&self) -> Value {
        let exhausted = self.exhausted();
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        json!({"schema":"angel.formation-budget/v1", "token_budget":state.tokens,
            "wall_secs":state.wall.map(|wall| wall.as_secs()), "budget_exhausted":exhausted,
            "exhaustion_reason":state.exhaustion_reason,
            "reservation_denial":state.reservation_denial,
            "charged":state.spent,
            "untracked_provider":state.untracked_provider,
            // Accounting law: an attempt whose usage never arrived makes the TOTAL unknown (never a
            // partial number presented as the whole). The reported subset and the count of
            // unreported attempts travel beside it so reducers can show "known ≥ X (n unreported)".
            "spent":(!state.untracked_provider).then(|| state.calls.iter().try_fold(0_u64, |sum, call| sum.checked_add(call["spent"].as_u64()?))).flatten(),
            "paid_spent":(!state.untracked_provider).then(|| state.calls.iter().try_fold(0_u64, |sum, call| sum.checked_add(call["paid_spent"].as_u64()?))).flatten(),
            "paid_spent_reported":(!state.untracked_provider).then(|| {
                let mut any = false;
                let mut sum = 0_u64;
                for call in &state.calls {
                    if let Some(v) = call["paid_spent"].as_u64() {
                        any = true;
                        sum = sum.checked_add(v)?;
                    }
                }
                any.then_some(sum)
            }).flatten(),
            "unreported_attempts":state.calls.iter().filter(|call| call["paid_spent"].as_u64().is_none()).count(),
            "cached":state.calls.iter().try_fold(0_u64, |sum, call| sum.checked_add(call["cached"].as_u64()?)),
            "reserved":state.reserved,
            "remaining":remaining_value(&state),
            "over_allocation":state.peak_over_allocation > 0 || state.accounting_overflowed,
            "over_allocation_tokens":state.peak_over_allocation,
            "current_over_allocation_tokens":state.tokens.map(|limit| state.spent.saturating_add(state.reserved).saturating_sub(limit)),
            "reservation_denied":exhausted,
            "accounting_overflowed":state.accounting_overflowed,
            "wall_allocation_exceeded":state.wall.is_some_and(|wall| state.start.elapsed() > wall),
            "calls":state.calls})
    }
}

// JSON integers cover ordinary allocations; retain exact decimal text at the
// signed JSON boundary instead of wrapping a large negative remainder.
fn remaining_value(state: &State) -> Value {
    state.tokens.map_or(Value::Null, |limit| {
        let remaining = i128::from(limit) - i128::from(state.spent) - i128::from(state.reserved);
        serde_json::to_value(remaining).unwrap_or_else(|_| json!(remaining.to_string()))
    })
}

// Opt-in numeric accounting only: never log prompts, credentials, or response bodies.
fn trace_budget(event: &str, receipt: &Value) {
    if std::env::var("ANGEL_GRAPH_BUDGET_TRACE").as_deref() == Ok("1") {
        eprintln!(
            "graph_budget_trace {}",
            json!({"event": event, "receipt": receipt})
        );
    }
}

pub(crate) struct Reservation {
    budget: Budget,
    id: usize,
    amount: u64,
}
impl Reservation {
    pub(crate) fn observe(&self, observation: UsageObservation) {
        let mut state = self.budget.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.calls[self.id]["settled"] != true {
            state.observations[self.id] = Some(observation);
        }
    }
    pub(crate) fn settle(self, observation: Option<UsageObservation>) {
        self.settle_inner(
            observation,
            if observation.is_some() {
                "observed"
            } else {
                "aborted"
            },
        );
    }
    fn settle_inner(&self, observation: Option<UsageObservation>, reason: &str) {
        let mut state = self.budget.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.calls[self.id]["settled"] == true {
            return;
        }
        let observation = observation.or(state.observations[self.id]);
        let mut accounting = AccountingSnapshot::default();
        accounting.record(observation);
        let prompt = accounting.fields[6];
        let output = accounting.fields[7];
        let actual = (prompt.reports == 1
            && output.reports == 1
            && !accounting.overflowed
            && accounting.inconsistent == 0)
            .then(|| prompt.sum.checked_add(output.sum))
            .flatten();
        let remaining_before = remaining_value(&state);
        state.reserved = state.reserved.saturating_sub(self.amount);
        // Unobserved attempts may have been billed: retain the whole reservation
        // as an estimate instead of claiming their usage was zero.
        let charge = actual.unwrap_or(self.amount);
        state.accounting_overflowed |= state.spent.checked_add(charge).is_none();
        state.spent = state.spent.saturating_add(charge);
        let over = state.tokens.map_or(0, |limit| {
            state
                .spent
                .saturating_add(state.reserved)
                .saturating_sub(limit)
        });
        state.peak_over_allocation = state.peak_over_allocation.max(over);
        if state.tokens.is_some() && actual.is_none()
            || actual.is_some_and(|spent| spent > self.amount)
            || state.tokens.is_some_and(|limit| state.spent >= limit)
        {
            state.exhaustion_reason.get_or_insert(if actual.is_none() {
                "unknown_usage"
            } else if actual.is_some_and(|spent| spent > self.amount) {
                "reservation_overrun"
            } else {
                "tokens"
            });
        }
        let remaining = remaining_value(&state);
        let end_ms = state.start.elapsed().as_millis();
        let call = &mut state.calls[self.id];
        call["settled"] = json!(true);
        call["settled_reason"] = json!(reason);
        call["end_ms"] = json!(end_ms);
        call["spent"] = json!(actual);
        // C06 paid input excludes cache reads. Cache writes remain paid input;
        // generated output already includes reasoning under the usage contract.
        call["paid_spent"] = json!(actual.and_then(|_| {
            prompt
                .sum
                .checked_sub(observation?.raw[3]?)?
                .checked_add(output.sum)
        }));
        call["cached"] = json!(observation.and_then(|o| o.raw[3]));
        call["charged"] = json!(charge);
        call["released_on_settlement"] = json!(self.amount.saturating_sub(charge));
        call["remaining_before_settlement"] = json!(remaining_before);
        call["remaining_after_settlement"] = json!(remaining);
        call["usage"] = json!(observation.map(|o| o.raw));
        if observation.is_none() {
            call["usage_absent"] = json!("provider sent none");
        }
        call["total_prompt"] = json!((prompt.reports == 1).then_some(prompt.sum));
        call["generation_output"] = json!((output.reports == 1).then_some(output.sum));
        call["normalized_usage"] = json!({
            "source": if actual.is_some() { "reported" } else { "unknown" },
            "input": (prompt.reports == 1).then_some(prompt.sum),
            "output": (output.reports == 1).then_some(output.sum),
            "cached": observation.and_then(|o| o.raw[3]),
            "reasoning": observation.and_then(|o| o.raw[2]),
            "input_semantics": "includes_cached", "output_semantics": "includes_reasoning",
        });
        let call_over = over.max(call["over_allocation_tokens"].as_u64().unwrap_or(0));
        call["over_allocation"] = json!(call_over > 0);
        call["over_allocation_tokens"] = json!(call_over);
        call["reservation_overrun"] = json!(actual.is_some_and(|actual| actual > self.amount));
        trace_budget("settled", call);
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        self.settle_inner(None, "aborted");
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/harness/formation_budget__tests.rs"]
mod tests;
