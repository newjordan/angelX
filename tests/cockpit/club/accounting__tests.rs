use super::*;

fn sample(
    input: Option<u64>,
    read: Option<u64>,
    write: Option<u64>,
    cache: CacheConvention,
) -> UsageObservation {
    UsageObservation {
        raw: [input, Some(5), None, read, write],
        paths: [
            Some("input_tokens"),
            Some("output_tokens"),
            None,
            Some("cache_read_input_tokens"),
            Some("cache_creation_input_tokens"),
        ],
        contract: UsageContract {
            cache,
            reasoning: ReasoningConvention::Unknown,
        },
    }
}

fn report(values: &[Option<UsageObservation>]) -> AccountingReport {
    let cell = AccountingCell::default();
    let before = cell.view();
    for value in values {
        cell.record(*value);
    }
    cell.view().delta(&before)
}

#[test]
fn usage_accounting_reasoning_contract_coverage_survives_merge_and_window_delta() {
    let observation = |reasoning, output, thinking| UsageObservation {
        raw: [Some(10), Some(output), Some(thinking), None, None],
        contract: UsageContract {
            cache: CacheConvention::Included,
            reasoning,
        },
        ..Default::default()
    };
    let included = observation(ReasoningConvention::Included, 10, 3);
    let separate = observation(ReasoningConvention::Separate, 10, 3);
    // Zero is observed data, but does not establish the provider's convention.
    let unknown_zero = observation(ReasoningConvention::Unknown, 0, 0);
    let mut before = AccountingSnapshot::default();
    before.record(Some(included));
    let mut after = before;
    for row in [Some(included), Some(separate), Some(unknown_zero), None] {
        after.record(row);
    }
    let window = after.delta(before);
    let mut left = AccountingSnapshot::default();
    left.record(Some(included));
    left.record(None);
    let mut right = AccountingSnapshot::default();
    right.record(Some(separate));
    right.record(Some(unknown_zero));
    left.add(right);
    assert_eq!(left, window, "source merge and exact window must agree");
    let report = AccountingReport::from_snapshot(window, false);
    assert_eq!(report.attempts, 4);
    assert_eq!(report.output, Some(20));
    assert_eq!(report.reasoning, Some(6));
    assert_eq!(report.generation_output, Some(23));
    assert_eq!(report.reported_attempts.generation_output, 2);
    assert_eq!(report.reported_attempts.reasoning, 3);
    assert!(!report.core_complete);
    let encoded = serde_json::to_value(report).unwrap();
    assert_eq!(
        encoded["reasoning_convention_attempts"],
        serde_json::json!({"included": 1, "separate": 1, "unknown": 1}),
        "receiptless attempts stay distinct from observed unknown conventions"
    );
}

#[test]
fn usage_accounting_missing_partial_and_explicit_zero_remain_distinct() {
    let unknown = report(&[None]);
    assert_eq!(
        (unknown.attempts, unknown.input, unknown.output),
        (1, None, None)
    );
    let partial = report(&[Some(UsageObservation {
        raw: [Some(17), None, None, None, None],
        ..Default::default()
    })]);
    assert_eq!(
        (partial.input, partial.output, partial.reasoning),
        (Some(17), None, None)
    );
    assert!(!partial.core_complete);
    let zero = report(&[Some(UsageObservation {
        raw: [Some(0), Some(0), None, None, None],
        ..Default::default()
    })]);
    assert_eq!(
        (zero.attempts, zero.input, zero.output, zero.reasoning),
        (1, Some(0), Some(0), None)
    );
    assert!(zero.core_complete);
    let value = serde_json::to_value(zero).unwrap();
    assert_eq!(value["input"], 0);
    assert!(value["reasoning"].is_null());
}

#[test]
fn usage_accounting_cache_only_is_visible_without_fabricated_input() {
    let usage = report(&[Some(UsageObservation {
        raw: [None, None, None, Some(80), None],
        paths: [
            None,
            None,
            None,
            Some("input_tokens_details.cached_tokens"),
            None,
        ],
        ..Default::default()
    })]);
    assert_eq!(
        (usage.attempts, usage.cache_read, usage.input),
        (1, Some(80), None)
    );
    assert_eq!(usage.total_prompt, None);
    assert_eq!(usage.cache_hit_pct, None);
    assert_eq!(
        usage.raw_field_reports["input_tokens_details.cached_tokens"],
        1
    );
}

#[test]
fn usage_accounting_inclusive_and_disjoint_prompt_contracts_are_explicit() {
    let included = report(&[Some(sample(
        Some(100),
        Some(20),
        Some(10),
        CacheConvention::Included,
    ))]);
    assert_eq!(
        (
            included.input,
            included.uncached_input,
            included.total_prompt,
            included.cache_hit_pct
        ),
        (Some(100), Some(70), Some(100), Some(20))
    );
    let separate = report(&[Some(sample(
        Some(100),
        Some(20),
        Some(4),
        CacheConvention::Separate,
    ))]);
    assert_eq!(
        (
            separate.input,
            separate.uncached_input,
            separate.total_prompt,
            separate.cache_hit_pct
        ),
        (Some(100), Some(100), Some(124), Some(16))
    );
    let unknown = report(&[Some(sample(
        Some(100),
        Some(20),
        Some(4),
        CacheConvention::Unknown,
    ))]);
    assert_eq!(
        (
            unknown.input,
            unknown.uncached_input,
            unknown.total_prompt,
            unknown.cache_hit_pct
        ),
        (Some(100), None, None, None)
    );
}

#[test]
fn usage_accounting_missing_cache_write_is_not_implicitly_zero() {
    let usage = report(&[Some(sample(
        Some(100),
        Some(20),
        None,
        CacheConvention::Included,
    ))]);
    assert_eq!((usage.cache_write, usage.uncached_input), (None, None));
    assert_eq!(
        (usage.total_prompt, usage.cache_hit_pct),
        (Some(100), Some(20))
    );
}

#[test]
fn usage_accounting_mixed_families_normalize_before_aggregation() {
    let usage = report(&[
        Some(sample(
            Some(100),
            Some(20),
            Some(10),
            CacheConvention::Included,
        )),
        Some(sample(
            Some(100),
            Some(20),
            Some(4),
            CacheConvention::Separate,
        )),
    ]);
    assert_eq!(
        (usage.input, usage.uncached_input, usage.total_prompt),
        (Some(200), Some(170), Some(224))
    );
    assert_eq!(usage.cache_hit_pct, Some(18));
    assert_eq!(usage.cache_convention_attempts["included"], 1);
    assert_eq!(usage.cache_convention_attempts["separate"], 1);
}

#[test]
fn usage_accounting_unknown_attempts_make_observed_totals_partial() {
    let usage = report(&[
        Some(sample(
            Some(100),
            Some(20),
            Some(0),
            CacheConvention::Included,
        )),
        None,
    ]);
    assert_eq!(
        (usage.attempts, usage.input, usage.reported_attempts.input),
        (2, Some(100), 1)
    );
    assert!(!usage.core_complete);
    assert_eq!(usage.cache_hit_pct, None);
}

#[test]
fn usage_accounting_dropped_attempt_commits_unknown_once() {
    let cell = AccountingCell::default();
    let before = cell.view();
    {
        let _attempt = cell.attempt();
    }
    {
        let mut attempt = cell.attempt();
        attempt.observe(Some(sample(Some(8), None, None, CacheConvention::Unknown)));
    }
    let usage = cell.view().delta(&before);
    assert_eq!(
        (usage.attempts, usage.input, usage.reported_attempts.input),
        (2, Some(8), 1)
    );
}

#[test]
fn usage_accounting_alias_wrappers_do_not_duplicate_sources_or_invent_zero() {
    let cell = AccountingCell::default();
    cell.record(Some(sample(Some(9), None, None, CacheConvention::Unknown)));
    let mut wrapper = AccountingView::default();
    wrapper.extend(cell.view());
    wrapper.extend(cell.view());
    let usage = wrapper.delta(&AccountingView::default());
    assert_eq!((usage.attempts, usage.input), (1, Some(9)));
    wrapper.extend(AccountingView::untracked());
    let partial = wrapper.delta(&AccountingView::default());
    assert!(partial.untracked_sources && !partial.core_complete);
    assert_eq!(partial.input, Some(9));
}

#[test]
fn usage_accounting_overflow_is_flagged_instead_of_wrapping() {
    let usage = report(&[
        Some(sample(Some(u64::MAX), None, None, CacheConvention::Unknown)),
        Some(sample(Some(1), None, None, CacheConvention::Unknown)),
    ]);
    assert!(usage.overflowed);
    assert_eq!(usage.input, None);
    assert!(!usage.core_complete);
    let derived = report(&[Some(sample(
        Some(u64::MAX),
        Some(1),
        Some(0),
        CacheConvention::Separate,
    ))]);
    assert!(derived.overflowed && derived.total_prompt.is_none());
}

#[test]
fn usage_accounting_inconsistent_cache_components_are_not_plausible_ratios() {
    let usage = report(&[Some(sample(
        Some(100),
        Some(80),
        Some(30),
        CacheConvention::Included,
    ))]);
    assert_eq!(usage.inconsistent_attempts, 1);
    assert_eq!(
        (usage.input, usage.cache_read, usage.cache_write),
        (Some(100), Some(80), Some(30))
    );
    assert_eq!(
        (
            usage.total_prompt,
            usage.uncached_input,
            usage.cache_hit_pct
        ),
        (None, None, None)
    );
}

#[test]
fn usage_accounting_reasoning_inclusion_never_double_adds_output() {
    let mut observation = sample(Some(100), None, None, CacheConvention::Unknown);
    observation.raw[1] = Some(50);
    observation.raw[2] = Some(30);
    observation.contract.reasoning = ReasoningConvention::Included;
    let included = report(&[Some(observation)]);
    assert_eq!(
        (
            included.output,
            included.reasoning,
            included.generation_output
        ),
        (Some(50), Some(30), Some(50))
    );
    observation.contract.reasoning = ReasoningConvention::Separate;
    assert_eq!(report(&[Some(observation)]).generation_output, Some(80));
    observation.contract.reasoning = ReasoningConvention::Unknown;
    assert_eq!(report(&[Some(observation)]).generation_output, None);
}

#[test]
fn usage_accounting_window_provenance_counts_do_not_include_old_fields() {
    let cell = AccountingCell::default();
    let mut observation = sample(Some(12), None, None, CacheConvention::Unknown);
    cell.record(Some(observation));
    let before = cell.view();
    observation.paths[0] = Some("prompt_tokens");
    cell.record(Some(observation));
    let usage = cell.view().delta(&before);
    assert_eq!(usage.raw_field_reports.get("prompt_tokens"), Some(&1));
    assert!(!usage.raw_field_reports.contains_key("input_tokens"));
}

#[test]
fn usage_accounting_snapshots_are_coherent_under_concurrent_writers() {
    let cell = std::sync::Arc::new(AccountingCell::default());
    let readers = std::sync::Arc::clone(&cell);
    let reader = std::thread::spawn(move || {
        for _ in 0..512 {
            let report = readers.view().delta(&AccountingView::default());
            if let (Some(input), Some(output)) = (report.input, report.output) {
                assert_eq!(input * 2, output);
                assert_eq!(report.attempts, input);
            } else {
                assert_eq!(
                    (report.attempts, report.input, report.output),
                    (0, None, None)
                );
            }
        }
    });
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let cell = &cell;
            scope.spawn(move || {
                for _ in 0..64 {
                    cell.record(Some(UsageObservation {
                        raw: [Some(1), Some(2), None, None, None],
                        ..Default::default()
                    }));
                }
            });
        }
    });
    reader.join().unwrap();
    let result = cell.view().delta(&AccountingView::default());
    assert_eq!(
        (result.attempts, result.input, result.output),
        (256, Some(256), Some(512))
    );
}

#[test]
fn usage_accounting_poisoned_or_disappearing_sources_are_untracked() {
    let cell = AccountingCell::default();
    let _ = std::panic::catch_unwind(|| {
        let _lock = cell.state.lock().unwrap();
        panic!("fixture");
    });
    let report = cell.view().delta(&AccountingView::default());
    assert!(report.untracked_sources && report.input.is_none());
    let good = AccountingCell::default();
    let before = good.view();
    assert!(AccountingView::default().delta(&before).untracked_sources);
}

/// Operator-ordered F01 contract: settled overrun cannot prevent the next attempt.
#[test]
fn formation_budget_accounting_attempts_keep_overrun_and_unknown_usage() {
    let _lock = crate::tests::env_lock();
    let budget = crate::agent::harness::formation_budget::Budget::new(Some(10), None);
    let cell = AccountingCell::default();
    for known in [true, false, true] {
        let mut attempt = cell.attempt();
        attempt.reserve_formation(budget.reserve("retry-seat", 20, 20).unwrap());
        if known {
            attempt.observe(Some(UsageObservation {
                raw: [Some(20), Some(20), Some(0), Some(0), Some(0)],
                contract: UsageContract {
                    cache: CacheConvention::Included,
                    reasoning: ReasoningConvention::Included,
                },
                ..Default::default()
            }));
        }
    }
    let receipt = budget.snapshot();
    assert_eq!(receipt["charged"], 120);
    assert_eq!(receipt["spent"], serde_json::Value::Null);
    assert_eq!(receipt["reserved"], 0);
    assert_eq!(receipt["over_allocation_tokens"], 110);
    assert_eq!(receipt["calls"].as_array().unwrap().len(), 3);
    assert_eq!(cell.view().sources.values().next().unwrap().attempts, 3);
}
