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
mod tests {
    use super::*;
    use crate::club::{CacheConvention, ReasoningConvention, UsageContract};
    fn usage(input: u64, output: u64) -> UsageObservation {
        UsageObservation {
            raw: [Some(input), Some(output), Some(2), Some(4), Some(0)],
            contract: UsageContract {
                cache: CacheConvention::Included,
                reasoning: ReasoningConvention::Included,
            },
            ..UsageObservation::default()
        }
    }
    /// Operator-ordered F01 contract: over-allocation and late workers are admitted.
    #[test]
    fn formation_budget_abort_closes_detached_reservations_once() {
        let budget = Budget::new(Some(100), None);
        let observed = budget.reserve("propose-a", 20, 20).unwrap();
        let unknown = budget.reserve("propose-b", 20, 20).unwrap();
        observed.observe(usage(10, 5));
        let coordinator = budget.reserve("coordinator", 20, 20).unwrap();
        assert_eq!(budget.snapshot()["over_allocation_tokens"], 20);
        coordinator.settle(Some(usage(10, 5)));
        budget.finish();
        let snapshot = budget.snapshot();
        assert_eq!(snapshot["reserved"], 0);
        assert_eq!(snapshot["charged"], 70);
        assert_eq!(snapshot["calls"][0]["spent"], 15);
        assert_eq!(snapshot["calls"][1]["usage"], Value::Null);
        for call in snapshot["calls"].as_array().unwrap() {
            assert_eq!(call["settled"], true);
            assert!(matches!(
                call["settled_reason"].as_str(),
                Some("aborted" | "reported" | "observed")
            ));
        }
        observed.settle(Some(usage(10, 5)));
        drop(unknown);
        assert_eq!(budget.snapshot(), snapshot);
        let late = budget.reserve("late-worker", 1, 1).unwrap();
        assert_eq!(
            budget.snapshot()["calls"][3]["admitted_after_owner_exit"],
            true
        );
        drop(late);
    }

    #[test]
    fn formation_budget_dropped_reservation_is_settled() {
        let budget = Budget::new(None, None);
        drop(budget.reserve("provider-error", 20, 20).unwrap());
        assert_eq!(budget.snapshot()["reserved"], 0);
        assert_eq!(budget.snapshot()["calls"][0]["settled_reason"], "aborted");
    }

    #[test]
    fn formation_budget_owner_exit_finalizes_but_worker_scope_does_not() {
        let _lock = crate::tests::env_lock();
        let budget = Budget::new(None, None);
        let mut owner = enter(Some(budget.clone()));
        owner.1 = true;
        let pending = budget.reserve("detached", 20, 20).unwrap();
        drop(enter(Some(budget.clone())));
        assert_eq!(budget.snapshot()["reserved"], 40);
        drop(owner);
        assert_eq!(snapshot().unwrap()["reserved"], 0);
        drop(pending);
    }

    #[test]
    fn formation_budget_paid_usage_separates_cache_without_double_reasoning() {
        let budget = Budget::new(Some(100), None);
        budget
            .reserve("route", 20, 30)
            .unwrap()
            .settle(Some(usage(10, 10)));
        assert_eq!(budget.snapshot()["spent"], 20);
        assert_eq!(budget.snapshot()["paid_spent"], 16);
        assert_eq!(budget.snapshot()["cached"], 4);
    }

    #[test]
    fn formation_budget_retried_attempts_sum_paid_usage() {
        let budget = Budget::new(Some(100), None);
        drop(budget.reserve("stalled", 20, 30).unwrap());
        budget
            .reserve("retry", 20, 30)
            .unwrap()
            .settle(Some(usage(10, 10)));
        let snap = budget.snapshot();
        // The stalled attempt sent no usage: the total stays unknown (law), the reported subset is 16.
        assert_eq!(snap["paid_spent"], serde_json::Value::Null);
        assert_eq!(snap["paid_spent_reported"], 16);
        assert_eq!(snap["unreported_attempts"], 1);
        assert_eq!(snap["calls"][0]["usage_absent"], "provider sent none");
        assert_eq!(snap["calls"][0]["paid_spent"], serde_json::Value::Null);
        assert_eq!(snap["calls"][1]["paid_spent"], 16);
    }

    #[test]
    fn formation_budget_cache_writes_remain_paid_input() {
        let budget = Budget::new(Some(100), None);
        let mut observation = usage(10, 10);
        observation.raw[3] = Some(2);
        observation.raw[4] = Some(4);
        budget
            .reserve("route", 20, 30)
            .unwrap()
            .settle(Some(observation));
        assert_eq!(budget.snapshot()["paid_spent"], 18);
        assert_eq!(budget.snapshot()["cached"], 2);
    }

    #[test]
    fn formation_graph_request_wall_is_nested_and_restored() {
        let _lock = crate::tests::env_lock();
        let _budget = enter(None);
        assert!(request_wall_remaining().is_none());
        let end = Instant::now() + Duration::from_secs(2);
        {
            let _wall = RequestWallScope::enter(end);
            let first = request_wall_remaining().unwrap();
            assert!(first <= Duration::from_secs(2));
            {
                let _inner = RequestWallScope::enter(end + Duration::from_secs(10));
                assert!(request_wall_remaining().unwrap() <= first);
            }
            assert!(request_wall_remaining().unwrap() <= first);
        }
        assert!(request_wall_remaining().is_none());
    }

    /// Operator-ordered F01 contract: outstanding reservations report overruns without denial.
    #[test]
    fn formation_budget_reservations_share_admission_and_settle_inclusive_usage() {
        let budget = Budget::new(Some(100), None);
        let first = budget.reserve("seat-a", 20, 30).unwrap();
        let second = budget.reserve("seat-b", 20, 30).unwrap();
        first.settle(Some(usage(10, 10)));
        second.settle(Some(usage(10, 10)));
        assert_eq!(budget.snapshot()["spent"], 40);
        assert_eq!(budget.snapshot()["remaining"], 60);
        let third = budget.reserve("seat-a", 20, 30).unwrap();
        let fourth = budget.reserve("seat-b", 20, 30).unwrap();
        assert_eq!(budget.snapshot()["remaining"], -40);
        assert_eq!(budget.snapshot()["over_allocation_tokens"], 40);
        fourth.settle(Some(usage(20, 30)));
        third.settle(Some(usage(20, 30)));
        assert!(budget.exhausted());
        assert!(budget.snapshot()["spent"].as_u64().unwrap() == 140);
    }
    #[test]
    fn formation_budget_grid_round_fits_exact_declared_ceiling() {
        // The recorded width-two proposal requests; refine width is also two.
        // Settlement releases unused output allowance before the next round.
        let required = (10_030 + 1024) + (10_053 + 1024);
        let budget = Budget::new(Some(required), None);
        let a = budget.reserve("glm-5.3-flash", 10_030, 1024).unwrap();
        let b = budget.reserve("glm-5.3", 10_053, 1024).unwrap();
        assert_eq!(budget.snapshot()["reserved"], required);
        a.settle(Some(usage(500, 20)));
        b.settle(Some(usage(500, 20)));
        let a = budget.reserve("refine-a", 9000, 1024).unwrap();
        let b = budget.reserve("refine-b", 9000, 1024).unwrap();
        a.settle(Some(usage(500, 20)));
        b.settle(Some(usage(500, 20)));
        assert_eq!(budget.snapshot()["spent"], 2080);
        assert_eq!(budget.snapshot()["reserved"], 0);
        assert!(!budget.exhausted());
    }
    #[test]
    fn formation_budget_shared_plan_ceiling_admits_proportional_flash_and_combination() {
        // G02f default plan ceiling, equal for all arms. Captured b15 wire
        // reservations and proportional usage; no tokenizer estimate is weakened.
        let flash = Budget::new(Some(131_072), None);
        flash
            .reserve("glm-5.3-flash", 18_378, 1024)
            .unwrap()
            .settle(Some(usage(4531, 51)));
        assert_eq!(flash.snapshot()["spent"], 4582);
        assert!(!flash.exhausted());

        let combination = Budget::new(Some(131_072), None);
        let rounds: &[&[(&str, u64, u64)]] = &[
            &[("glm-5.3-flash", 10_068, 2495), ("glm-5.3", 10_091, 2501)],
            &[("glm-5.3-flash", 10_391, 2576)],
            &[("glm-5.3-flash", 18_653, 4651)],
            &[("glm-5.3-flash", 9880, 2448), ("glm-5.3", 9862, 2444)],
            &[("glm-5.3-flash", 10_201, 2538)],
        ];
        for round in rounds {
            let pending: Vec<_> = round
                .iter()
                .map(|(route, prompt, actual)| {
                    (combination.reserve(route, *prompt, 1024).unwrap(), *actual)
                })
                .collect();
            for (reservation, actual) in pending {
                reservation.settle(Some(usage(actual - 51, 51)));
            }
        }
        let receipt = combination.snapshot();
        assert_eq!(receipt["spent"], 19_653);
        assert_eq!(receipt["reserved"], 0);
        assert_eq!(receipt["calls"].as_array().unwrap().len(), 7);
        assert!(!combination.exhausted());
    }

    /// Operator-ordered F01 contract: zero allocation admits each seat and reports the overrun.
    #[test]
    fn formation_budget_zero_allocation_reports_required_and_available_with_admitted_calls() {
        for (prompt, required) in [(10_030, 11_054), (17_732, 18_756)] {
            let budget = Budget::new(Some(0), None);
            budget.set_unstarted(2);
            let reservation = budget.reserve("grid-seat", prompt, 1024).unwrap();
            let record = budget.snapshot();
            assert_eq!(record["calls"].as_array().unwrap().len(), 1);
            assert_eq!(record["reservation_denial"]["required_tokens"], required);
            assert_eq!(record["reservation_denial"]["available_tokens"], 0);
            assert_eq!(record["reservation_denial"]["token_budget"], 0);
            assert_eq!(record["reservation_denial"]["admitted"], true);
            assert_eq!(record["over_allocation_tokens"], required);
            reservation.settle(Some(usage(10, 10)));
            assert!(budget.exhausted());
        }
    }
    /// Operator-ordered F01 contract: small explicit wants exceed remaining allocation and proceed.
    #[test]
    fn formation_budget_overrun_accounts_for_outstanding_and_spent_tokens() {
        let budget = Budget::new(Some(100), None);
        budget
            .reserve("settled", 10, 10)
            .unwrap()
            .settle(Some(usage(10, 10)));
        let pending = budget.reserve("pending", 20, 30).unwrap();
        let admitted = budget.reserve("admitted", 20, 11).unwrap();
        let record = budget.snapshot();
        assert_eq!(record["reservation_denial"]["available_tokens"], 30);
        assert_eq!(record["reservation_denial"]["required_tokens"], 31);
        assert_eq!(record["remaining"], -1);
        assert_eq!(record["over_allocation_tokens"], 1);
        pending.settle(Some(usage(10, 10)));
        admitted.settle(Some(usage(10, 10)));
        assert_eq!(budget.snapshot()["reserved"], 0);
        assert_eq!(budget.snapshot()["over_allocation"], true);
        assert!(!budget.exhausted());
    }
    /// Operator-ordered F01 contract: numeric overflow is explicit and never a denial.
    #[test]
    fn formation_budget_reservation_overflow_is_explicit() {
        let budget = Budget::new(Some(u64::MAX), None);
        let reservation = budget.reserve("overflow", u64::MAX, 1).unwrap();
        let record = budget.snapshot();
        assert_eq!(record["accounting_overflowed"], true);
        assert_eq!(record["reservation_denial"]["required_tokens"], Value::Null);
        assert_eq!(record["calls"].as_array().unwrap().len(), 1);
        drop(reservation);
    }
    /// Operator-ordered F01 contract: expired wall allocation and unknown usage remain observational.
    #[test]
    fn formation_budget_wall_and_unknown_usage_keep_admission_open() {
        let budget = Budget::new(None, Some(Duration::ZERO));
        drop(budget.reserve("stalled", 1, 1).unwrap());
        assert_eq!(budget.snapshot()["wall_allocation_exceeded"], true);
        let budget = Budget::new(Some(100), None);
        budget.reserve("unknown", 20, 30).unwrap().settle(None);
        assert_eq!(budget.snapshot()["charged"], 50);
        assert_eq!(budget.snapshot()["spent"], Value::Null);
        assert!(!budget.exhausted());
        drop(budget.reserve("next", 20, 30).unwrap());
    }
    #[test]
    fn formation_budget_unset_identity_is_byte_identical() {
        let _lock = crate::tests::env_lock();
        let _scope = enter(None);
        let mut budgets = json!({"max_hops":3,"compaction_budget_tokens":4096,"compact_keep_recent":2,"compact_keep_recent_tokens":1024});
        let before = serde_json::to_vec(&budgets).unwrap();
        identity(&mut budgets);
        assert_eq!(before, serde_json::to_vec(&budgets).unwrap());
    }
    struct ScriptedSeat(std::sync::atomic::AtomicUsize);
    impl crate::club::Club for ScriptedSeat {
        fn label(&self) -> &str {
            "scripted-budget-seat"
        }
        fn supports_formation_budget(&self) -> bool {
            true
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            let reservation = current().unwrap().reserve(self.label(), 10, 30)?;
            let n = self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            reservation.settle(Some(usage(10, 30)));
            Ok(["Use dynamic programming over prefixes to compute the exact minimum edit distance.",
                "Build a graph of states then use breadth first search to discover the shortest route.",
                "The complete refined answer is to memoize all subproblems and return the optimal cost.",
                "The complete alternative answer enumerates feasible candidates and selects the minimum."][n % 4].into())
        }
    }
    /// Operator-ordered F01 contract: refinement and synthesis continue past the allocation.
    #[test]
    fn formation_budget_width_two_completes_refinement_and_synthesis_after_overrun() {
        use crate::club::Club;
        let _lock = crate::tests::env_lock();
        let budget = Budget::new(Some(160), None);
        let _scope = enter(Some(budget.clone()));
        let seat = Arc::new(ScriptedSeat(std::sync::atomic::AtomicUsize::new(0)));
        let swarm = crate::swarm::SwarmClub::with_knobs(
            "budget-test",
            seat.clone(),
            crate::swarm::Knobs {
                width: 2,
                max_width: 2,
                refine_width: 2,
                layers: 3,
                always: true,
                ..crate::swarm::Knobs::default()
            },
        );
        let answer = swarm
            .respond("Compare two algorithms and derive a complete optimal solution.")
            .unwrap();
        assert!(answer.contains("complete"), "{answer}");
        let calls = seat.0.load(std::sync::atomic::Ordering::SeqCst);
        assert!(calls > 4);
        assert_eq!(budget.snapshot()["spent"], calls * 40);
        assert_eq!(budget.snapshot()["over_allocation"], true);
        assert_eq!(budget.snapshot()["reserved"], 0);
        assert!(budget.exhausted());
    }
    /// Operator-ordered F01 contract: the next seat is admitted after spend exceeds allocation.
    #[test]
    fn formation_budget_single_seat_uses_same_allowance() {
        use crate::club::Club;
        let _lock = crate::tests::env_lock();
        let budget = Budget::new(Some(160), None);
        let _scope = enter(Some(budget.clone()));
        let seat = ScriptedSeat(std::sync::atomic::AtomicUsize::new(0));
        for _ in 0..4 {
            assert!(!seat.respond("same task").unwrap().is_empty());
        }
        assert!(!seat.respond("same task").unwrap().is_empty());
        assert_eq!(budget.snapshot()["spent"], 200);
        assert_eq!(budget.snapshot()["over_allocation_tokens"], 40);
        assert!(budget.exhausted());
    }
    struct StalledSeat;
    impl crate::club::Club for StalledSeat {
        fn label(&self) -> &str {
            "stalled-budget-seat"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            let budget = current().unwrap();
            let reservation = budget.reserve(self.label(), 10, 30)?;
            while let Some(remaining) = budget.remaining_wall() {
                if remaining.is_zero() {
                    break;
                }
                std::thread::sleep(remaining.min(Duration::from_millis(2)));
            }
            reservation.settle(None);
            Ok("The complete answer remains available after the wall allocation.".into())
        }
    }
    /// Operator-ordered F01 contract: a wall overrun does not abort the stalled seat or formation.
    #[test]
    fn formation_budget_wall_fires_during_stalled_seat() {
        use crate::club::Club;
        let _lock = crate::tests::env_lock();
        let budget = Budget::new(None, Some(Duration::from_millis(30)));
        let _scope = enter(Some(budget.clone()));
        let swarm = crate::swarm::SwarmClub::with_knobs(
            "budget-test",
            Arc::new(StalledSeat),
            crate::swarm::Knobs {
                width: 2,
                max_width: 2,
                layers: 2,
                always: true,
                ..crate::swarm::Knobs::default()
            },
        );
        let start = Instant::now();
        let answer = swarm.respond("Compare two complete solutions.").unwrap();
        assert!(start.elapsed() < Duration::from_secs(1));
        assert!(answer.contains("complete answer"));
        assert_eq!(budget.snapshot()["wall_allocation_exceeded"], true);
        // Wait only for the owned scripted workers to settle before releasing
        // env_lock; the wall allocation itself did not end the barrier.
        while budget.snapshot()["reserved"] != 0 {
            assert!(start.elapsed() < Duration::from_secs(1));
            std::thread::yield_now();
        }
    }
    #[test]
    fn formation_budget_knobs_read_once_and_identity_records_effective_values() {
        let _lock = crate::tests::env_lock();
        let _tokens = crate::tests::TestEnvGuard::set("ANGEL_FORMATION_TOKEN_BUDGET", "160");
        let _wall = crate::tests::TestEnvGuard::set("ANGEL_FORMATION_WALL_SECS", "1");
        let _scope = start_turn().unwrap();
        let _changed = crate::tests::TestEnvGuard::set("ANGEL_FORMATION_TOKEN_BUDGET", "999");
        let mut budgets = json!({"max_hops":0});
        identity(&mut budgets);
        assert_eq!(budgets["formation_token_budget"], 160);
        assert_eq!(budgets["formation_wall_secs"], 1);
    }
    /// Operator-ordered F01 contract: all concurrent seats run and share one overrun ledger.
    #[test]
    fn formation_budget_concurrent_reservations_share_one_overrun_ledger() {
        let _lock = crate::tests::env_lock();
        let budget = Budget::new(Some(100), None);
        let barrier = Arc::new(std::sync::Barrier::new(16));
        let workers: Vec<_> = (0..16)
            .map(|_| {
                let budget = budget.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    budget
                        .reserve("racing-seat", 10, 30)
                        .unwrap()
                        .settle(Some(usage(10, 30)));
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        let record = budget.snapshot();
        assert_eq!(record["reserved"], 0);
        assert_eq!(record["calls"].as_array().unwrap().len(), 16);
        assert_eq!(record["spent"], 640);
        assert_eq!(record["remaining"], -540);
        assert_eq!(record["over_allocation_tokens"], 540);
    }

    /// Operator-ordered F01 contract: G01e small-want fail-closed behavior is repealed.
    #[test]
    fn formation_budget_fitted_cap_preserves_large_request_intent() {
        for requested in [1, 16_384] {
            let budget = Budget::new(Some(6000), None);
            let reservation = budget.reserve_fitted("fanin", 6470, 1, requested).unwrap();
            assert_eq!(budget.snapshot()["calls"][0]["reserved"], 6471);
            assert_eq!(budget.snapshot()["over_allocation_tokens"], 471);
            reservation.settle(Some(usage(1500, 8)));
            assert_eq!(budget.snapshot()["spent"], 1508);
            assert_eq!(budget.snapshot()["remaining"], 4492);
            assert_eq!(budget.snapshot()["over_allocation"], true);
        }
    }
    #[test]
    fn formation_budget_live_glm_release_already_admits_fanin_at_both_allocations() {
        // Observed nofault G01c/root-live-b20 calls, in settlement order.
        // Fan-in's usage was missing: test admission only, never invent spend.
        for total in [48_000, 288_000] {
            let budget = Budget::new(Some(total), None);
            for (prompt, input, output) in [
                (5295, 1268, 528),
                (7837, 1848, 42),
                (7840, 1848, 81),
                (7861, 1861, 80),
                (8377, 1944, 278),
                (8485, 1977, 314),
                (8382, 1950, 547),
                (10260, 2507, 428),
            ] {
                budget
                    .reserve("observed", prompt, 1024)
                    .unwrap()
                    .settle(Some(usage(input, output)));
            }
            let fanin = budget.reserve("fanin", 6470, 1024).unwrap();
            assert_eq!(budget.snapshot()["calls"][8]["reserved"], 7494);
            assert_eq!(budget.snapshot()["spent"], Value::Null);
            assert_eq!(budget.snapshot()["charged"], 17501);
            // Pending usage is unknown until the terminal provider frame.
            drop(fanin);
            assert_eq!(budget.snapshot()["exhaustion_reason"], "unknown_usage");
        }
    }

    #[test]
    fn formation_budget_release_from_three_workers_admits_fanin() {
        let budget = Budget::new(Some(48_000), None);
        budget.set_unstarted(5);
        budget
            .reserve("planner", 4872, 1024)
            .unwrap()
            .settle(Some(usage(40, 8)));
        let workers: Vec<_> = (0..3)
            .map(|_| budget.reserve("worker", 5100, 50_000).unwrap())
            .collect();
        let before = budget.available_tokens().unwrap();
        for worker in workers {
            worker.settle(Some(usage(40, 8)));
        }
        assert!(budget.available_tokens().unwrap() > before);
        assert_eq!(budget.available_tokens(), Some(47_808));
        let fanin = budget.reserve("fanin", 4383, 50_000).unwrap();
        assert_eq!(budget.snapshot()["reserved"], 47_808);
        fanin.settle(Some(usage(40, 8)));
        let snapshot = budget.snapshot();
        assert_eq!(snapshot["reserved"], 0);
        assert_eq!(snapshot["spent"], 240);
        assert_eq!(snapshot["calls"][4]["released_on_settlement"], 47_760);
        assert!(!budget.exhausted());
    }

    #[test]
    fn formation_budget_share_single_node_gets_everything() {
        let budget = Budget::new(Some(10_000), None);
        let reservation = budget.reserve("solo", 0, 50_000).unwrap();
        assert_eq!(budget.snapshot()["reserved"], 10_000);
        reservation.settle(Some(usage(10, 10)));
        assert_eq!(budget.snapshot()["spent"], 20);
        assert_eq!(budget.snapshot()["remaining"], 9_980);
    }
    #[test]
    fn formation_budget_share_five_equal_nodes_each_get_a_fifth() {
        let budget = Budget::new(Some(10_000), None);
        budget.set_unstarted(5);
        let pending: Vec<_> = (0..5)
            .map(|i| budget.reserve(&format!("n{i}"), 0, 50_000).unwrap())
            .collect();
        assert_eq!(budget.snapshot()["reserved"], 10_000);
        for call in budget.snapshot()["calls"].as_array().unwrap() {
            assert_eq!(call["reserved"], 2_000);
        }
        for reservation in pending {
            reservation.settle(Some(usage(10, 10)));
        }
        assert_eq!(budget.snapshot()["spent"], 100);
    }
    #[test]
    fn formation_budget_share_released_reservation_flows_to_next_node() {
        let budget = Budget::new(Some(10_000), None);
        budget.set_unstarted(2);
        let first = budget.reserve("first", 0, 50_000).unwrap();
        assert_eq!(budget.snapshot()["reserved"], 5_000);
        first.settle(Some(usage(10, 10)));
        let second = budget.reserve("second", 0, 50_000).unwrap();
        assert_eq!(budget.snapshot()["reserved"], 9_980);
        second.settle(Some(usage(10, 10)));
        assert_eq!(budget.snapshot()["spent"], 40);
    }
    #[test]
    fn formation_budget_four_seats_32768_admits_hop_three() {
        let budget = Budget::new(Some(32_768), None);
        for hop_seats in [4, 1, 1] {
            budget.set_unstarted(hop_seats);
            let pending: Vec<_> = (0..hop_seats)
                .map(|i| budget.reserve(&format!("seat{i}"), 100, 16_384).unwrap())
                .collect();
            for reservation in pending {
                reservation.settle(Some(usage(50, 20)));
            }
        }
        assert!(!budget.exhausted());
        assert_eq!(budget.snapshot()["calls"].as_array().unwrap().len(), 6);
    }
}
