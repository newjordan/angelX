use super::*;
use crate::agent::club::{CacheConvention, ReasoningConvention, UsageContract};
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
impl crate::agent::club::Club for ScriptedSeat {
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
    use crate::agent::club::Club;
    let _lock = crate::tests::env_lock();
    let budget = Budget::new(Some(160), None);
    let _scope = enter(Some(budget.clone()));
    let seat = Arc::new(ScriptedSeat(std::sync::atomic::AtomicUsize::new(0)));
    let swarm = crate::agent::swarm::SwarmClub::with_knobs(
        "budget-test",
        seat.clone(),
        crate::agent::swarm::Knobs {
            width: 2,
            max_width: 2,
            refine_width: 2,
            layers: 3,
            always: true,
            ..crate::agent::swarm::Knobs::default()
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
    use crate::agent::club::Club;
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
impl crate::agent::club::Club for StalledSeat {
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
    use crate::agent::club::Club;
    let _lock = crate::tests::env_lock();
    let budget = Budget::new(None, Some(Duration::from_millis(30)));
    let _scope = enter(Some(budget.clone()));
    let swarm = crate::agent::swarm::SwarmClub::with_knobs(
        "budget-test",
        Arc::new(StalledSeat),
        crate::agent::swarm::Knobs {
            width: 2,
            max_width: 2,
            layers: 2,
            always: true,
            ..crate::agent::swarm::Knobs::default()
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
