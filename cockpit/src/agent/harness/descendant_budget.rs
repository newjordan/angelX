//! Root-turn cumulative allowance for model descendants.
//!
//! This is deliberately separate from `SPAWN_SEATS_INFLIGHT`: the latter is a
//! process-wide live-resource backstop whose permits are returned, while this
//! budget counts logical descendant calls for one root turn and never refunds
//! them. A handle is inherited explicitly by detached seat threads so nested
//! formations cannot multiply a fresh allowance at each level.

use super::*;

const DESCENDANT_CALL_BUDGET_DEFAULT: usize = 0;
const DESCENDANT_CALL_BUDGET_MAX: usize = 1024;
const DESCENDANT_CALL_BUDGET_ENV: &str = "ANGEL_DESCENDANT_CALL_BUDGET";

#[derive(Debug)]
pub(crate) struct DescendantCallBudget {
    total: usize,
    spent: AtomicUsize,
}

#[derive(Clone, Debug)]
pub(crate) enum DescendantBudgetHandle {
    Ready(Arc<DescendantCallBudget>),
    Invalid(Arc<str>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DescendantBudgetReceipt {
    pub(crate) requested: usize,
    pub(crate) total: usize,
    pub(crate) spent: usize,
    pub(crate) remaining: usize,
}

impl DescendantBudgetReceipt {
    pub(crate) fn status_fields(self) -> String {
        if self.total == 0 {
            return format!("descendant_calls={} unbounded", self.spent);
        }
        format!(
            "descendant_calls={}/{} remaining={}",
            self.spent, self.total, self.remaining
        )
    }

    pub(crate) fn fields(self) -> String {
        format!("{} admitted={}", self.status_fields(), self.requested)
    }
}

thread_local! {
    static DESCENDANT_BUDGET: std::cell::RefCell<Option<DescendantBudgetHandle>> =
        const { std::cell::RefCell::new(None) };
}

/// A root scope creates one allowance only when the thread has no inherited
/// handle. Nested `run_turn` entries therefore retain their parent's budget.
pub(crate) struct DescendantBudgetScope {
    previous: Option<DescendantBudgetHandle>,
}

impl DescendantBudgetScope {
    pub(crate) fn enter_root() -> Self {
        let previous = DESCENDANT_BUDGET.with(|slot| slot.borrow().clone());
        if previous.is_none() {
            let handle = configured_budget_handle();
            DESCENDANT_BUDGET.with(|slot| *slot.borrow_mut() = Some(handle));
        }
        Self { previous }
    }

    /// Install the root handle in a newly spawned thread. Callers obtain the
    /// handle before spawning; an absent context is a programming error rather
    /// than an excuse to mint a second allowance.
    pub(crate) fn inherit(handle: DescendantBudgetHandle) -> Self {
        let previous = DESCENDANT_BUDGET.with(|slot| slot.borrow_mut().replace(handle));
        Self { previous }
    }

    #[cfg(test)]
    pub(crate) fn for_test(total: usize) -> Self {
        assert!((1..=DESCENDANT_CALL_BUDGET_MAX).contains(&total));
        Self::inherit(DescendantBudgetHandle::Ready(Arc::new(
            DescendantCallBudget {
                total,
                spent: AtomicUsize::new(0),
            },
        )))
    }
}

impl Drop for DescendantBudgetScope {
    fn drop(&mut self) {
        DESCENDANT_BUDGET.with(|slot| *slot.borrow_mut() = self.previous.take());
    }
}

fn configured_budget_handle() -> DescendantBudgetHandle {
    let raw = match std::env::var(DESCENDANT_CALL_BUDGET_ENV) {
        Ok(raw) => raw,
        Err(std::env::VarError::NotPresent) => {
            return DescendantBudgetHandle::Ready(Arc::new(DescendantCallBudget {
                total: DESCENDANT_CALL_BUDGET_DEFAULT,
                spent: AtomicUsize::new(0),
            }));
        }
        Err(error) => {
            return DescendantBudgetHandle::Invalid(
                format!("read {DESCENDANT_CALL_BUDGET_ENV}: {error}").into(),
            );
        }
    };
    match raw.trim().parse::<usize>() {
        Ok(total) if total <= DESCENDANT_CALL_BUDGET_MAX => {
            DescendantBudgetHandle::Ready(Arc::new(DescendantCallBudget {
                total,
                spent: AtomicUsize::new(0),
            }))
        }
        _ => DescendantBudgetHandle::Invalid(
            format!(
                "invalid {DESCENDANT_CALL_BUDGET_ENV}: expected an integer in 0..={DESCENDANT_CALL_BUDGET_MAX}"
            )
            .into(),
        ),
    }
}

pub(crate) fn current_descendant_budget() -> Result<DescendantBudgetHandle, String> {
    DESCENDANT_BUDGET.with(|slot| {
        slot.borrow()
            .clone()
            .ok_or_else(|| "descendant budget context is not installed".to_string())
    })
}

pub(crate) fn reserve_descendant_calls(
    handle: &DescendantBudgetHandle,
    requested: usize,
    kind: &str,
) -> Result<DescendantBudgetReceipt, String> {
    let budget = match handle {
        DescendantBudgetHandle::Ready(budget) => budget,
        DescendantBudgetHandle::Invalid(error) => {
            return Err(format!(
                "descendant call budget unavailable for {kind}: {error}"
            ));
        }
    };
    let mut spent = budget.spent.load(Ordering::Acquire);
    loop {
        let Some(next) = spent.checked_add(requested) else {
            return Err(format!(
                "descendant call budget exhausted for {kind}: requested={requested} total={} spent={spent} remaining={}; zero calls admitted; operator cap ANGEL_DESCENDANT_CALL_BUDGET={}",
                budget.total,
                budget.total.saturating_sub(spent),
                budget.total,
            ));
        };
        if budget.total > 0 && next > budget.total {
            return Err(format!(
                "descendant call budget exhausted for {kind}: requested={requested} total={} spent={spent} remaining={}; zero calls admitted; operator cap ANGEL_DESCENDANT_CALL_BUDGET={}",
                budget.total,
                budget.total.saturating_sub(spent),
                budget.total,
            ));
        }
        match budget
            .spent
            .compare_exchange_weak(spent, next, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => {
                return Ok(DescendantBudgetReceipt {
                    requested,
                    total: budget.total,
                    spent: next,
                    remaining: budget.total.saturating_sub(next),
                });
            }
            Err(observed) => spent = observed,
        }
    }
}

pub(crate) fn descendant_budget_status(
    handle: &DescendantBudgetHandle,
) -> Result<DescendantBudgetReceipt, String> {
    match handle {
        DescendantBudgetHandle::Ready(budget) => {
            let spent = budget.spent.load(Ordering::Acquire);
            Ok(DescendantBudgetReceipt {
                requested: 0,
                total: budget.total,
                spent,
                remaining: budget.total.saturating_sub(spent),
            })
        }
        DescendantBudgetHandle::Invalid(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/descendant_budget__tests.rs"]
mod tests;
