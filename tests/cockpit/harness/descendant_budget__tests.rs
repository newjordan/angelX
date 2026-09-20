use super::*;
use std::sync::Barrier;

#[test]
fn l01_descendants_continue_past_old_default_and_honor_operator_cap() {
    let _guard = crate::tests::env_lock();
    let _unset = crate::tests::TestEnvGuard::unset("ANGEL_DESCENDANT_CALL_BUDGET");
    let handle = configured_budget_handle();
    for _ in 0..200 {
        reserve_descendant_calls(&handle, 1, "fixture").unwrap();
    }
    assert!(
        descendant_budget_status(&handle)
            .unwrap()
            .status_fields()
            .contains("unbounded")
    );
    let _cap = crate::tests::TestEnvGuard::set("ANGEL_DESCENDANT_CALL_BUDGET", "2");
    let handle = configured_budget_handle();
    reserve_descendant_calls(&handle, 2, "fixture").unwrap();
    assert!(
        reserve_descendant_calls(&handle, 1, "fixture")
            .unwrap_err()
            .contains("ANGEL_DESCENDANT_CALL_BUDGET=2")
    );
}

#[test]
fn concurrent_admission_is_atomic_and_never_partially_reserves() {
    let _scope = DescendantBudgetScope::for_test(7);
    let handle = current_descendant_budget().unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let handle = handle.clone();
        let barrier = Arc::clone(&barrier);
        workers.push(std::thread::spawn(move || {
            let _scope = DescendantBudgetScope::inherit(handle.clone());
            barrier.wait();
            reserve_descendant_calls(&handle, 4, "panel")
        }));
    }
    barrier.wait();
    let outcomes = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
    let failure = outcomes
        .iter()
        .find_map(|result| result.as_ref().err())
        .unwrap();
    assert!(failure.contains("zero calls admitted"), "{failure}");
    assert_eq!(
        descendant_budget_status(&handle).unwrap(),
        DescendantBudgetReceipt {
            requested: 0,
            total: 7,
            spent: 4,
            remaining: 3,
        }
    );
}

#[test]
fn inherited_scope_never_refunds_failed_or_completed_calls() {
    let _scope = DescendantBudgetScope::for_test(3);
    let handle = current_descendant_budget().unwrap();
    reserve_descendant_calls(&handle, 2, "outer panel").unwrap();
    let inherited = handle.clone();
    std::thread::spawn(move || {
        let _scope = DescendantBudgetScope::inherit(inherited.clone());
        reserve_descendant_calls(&inherited, 1, "nested panel").unwrap();
    })
    .join()
    .unwrap();
    let error = reserve_descendant_calls(&handle, 1, "replacement").unwrap_err();
    assert!(error.contains("spent=3 remaining=0"), "{error}");
}
