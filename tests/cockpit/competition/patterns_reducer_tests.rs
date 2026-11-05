//! Parity, fallback and timing for the `patterns-reducer` leg. The native
//! `apply_evidence`/`apply_to_fields` pass is the oracle: every case below is
//! reduced twice, once through the shipped leg and once through Rust.

use super::patterns::{
    FIELD_PATTERN_SCHEMA_V1, PatternEvidenceClassV1, PatternEvidenceV1, PatternKeyV1,
    PatternOutcomeV1,
};
use super::patterns_reducer::reduce_fields_with;
use super::patterns_tests::key;
use crate::agent::harness::live_policy::{LegSource, shipped_leg};

const LEG: &str = "patterns-reducer";

fn evidence(
    id: &str,
    target: PatternKeyV1,
    class: PatternEvidenceClassV1,
    outcome: PatternOutcomeV1,
    uncertainty: u16,
    cost: u64,
) -> PatternEvidenceV1 {
    PatternEvidenceV1 {
        schema: FIELD_PATTERN_SCHEMA_V1.into(),
        evidence_id: id.into(),
        episode_id: "episode-1".into(),
        target_key: target,
        class,
        outcome,
        uncertainty_millis: uncertainty,
        cost_microunits: cost,
        promotes_evidence_id: None,
    }
}

fn at(benchmark: &str, mechanism: &str) -> PatternKeyV1 {
    let mut base = key();
    base.benchmark_id = benchmark.into();
    base.mechanism = mechanism.into();
    base
}

fn official(binding: u64) -> PatternEvidenceClassV1 {
    PatternEvidenceClassV1::ConfirmedOfficial {
        reward_binding_id: format!("{binding:064x}"),
        corrects_reward_binding_id: None,
    }
}

fn corrected(binding: u64, corrects: u64) -> PatternEvidenceClassV1 {
    PatternEvidenceClassV1::ConfirmedOfficial {
        reward_binding_id: format!("{binding:064x}"),
        corrects_reward_binding_id: Some(format!("{corrects:064x}")),
    }
}

fn transfer() -> PatternEvidenceClassV1 {
    PatternEvidenceClassV1::ProvisionalTransfer {
        source_key: at("benchmark", "other-mechanism"),
    }
}

fn outcome(index: usize) -> PatternOutcomeV1 {
    match index {
        0 => PatternOutcomeV1::Improvement,
        1 => PatternOutcomeV1::Tie,
        2 => PatternOutcomeV1::Regression,
        3 => PatternOutcomeV1::Failure,
        _ => PatternOutcomeV1::Timeout,
    }
}

/// Three membership classes crossed with the five outcomes, then the bucket and
/// pattern orderings, a correction, a promotion, a carry and a saturated sum.
fn cases() -> Vec<(&'static str, Vec<PatternEvidenceV1>)> {
    const GRID: [[&str; 5]; 3] = [
        [
            "confirmed official improvement",
            "confirmed official tie",
            "confirmed official regression",
            "confirmed official failure",
            "confirmed official timeout",
        ],
        [
            "provisional transfer improvement",
            "provisional transfer tie",
            "provisional transfer regression",
            "provisional transfer failure",
            "provisional transfer timeout",
        ],
        [
            "operational improvement",
            "operational tie",
            "operational regression",
            "operational failure",
            "operational timeout",
        ],
    ];
    let class = |row: usize| match row {
        0 => official(1),
        1 => transfer(),
        _ => PatternEvidenceClassV1::OperationalOutcome,
    };
    let mut cases: Vec<(&'static str, Vec<PatternEvidenceV1>)> = Vec::new();
    cases.push(("no active evidence", Vec::new()));
    for (row, names) in GRID.iter().enumerate() {
        for (column, name) in names.iter().enumerate() {
            cases.push((
                name,
                vec![evidence(
                    "evidence-1",
                    key(),
                    class(row),
                    outcome(column),
                    0,
                    0,
                )],
            ));
        }
    }
    cases.push((
        "zero uncertainty and cost count nothing extra",
        vec![evidence(
            "evidence-1",
            key(),
            PatternEvidenceClassV1::OperationalOutcome,
            PatternOutcomeV1::Tie,
            0,
            0,
        )],
    ));
    cases.push((
        "uncertainty stops at its per-evidence bound",
        vec![evidence(
            "evidence-1",
            key(),
            PatternEvidenceClassV1::OperationalOutcome,
            PatternOutcomeV1::Improvement,
            1_000,
            1,
        )],
    ));
    cases.push((
        "an official correction keeps the newest binding",
        vec![
            evidence(
                "evidence-1",
                key(),
                official(1),
                PatternOutcomeV1::Improvement,
                10,
                100,
            ),
            evidence(
                "evidence-2",
                key(),
                corrected(2, 1),
                PatternOutcomeV1::Regression,
                20,
                200,
            ),
            evidence(
                "evidence-3",
                key(),
                corrected(3, 2),
                PatternOutcomeV1::Tie,
                30,
                300,
            ),
        ],
    ));
    cases.push((
        "an official binding promotes one transfer",
        vec![
            evidence(
                "evidence-1",
                key(),
                transfer(),
                PatternOutcomeV1::Improvement,
                5,
                50,
            ),
            {
                let mut promotion = evidence(
                    "evidence-2",
                    key(),
                    official(1),
                    PatternOutcomeV1::Improvement,
                    5,
                    50,
                );
                promotion.promotes_evidence_id = Some("evidence-1".into());
                promotion
            },
        ],
    ));
    cases.push((
        "two buckets in bucket-key order",
        vec![
            evidence(
                "evidence-1",
                at("benchmark-b", "vectorized-load"),
                PatternEvidenceClassV1::OperationalOutcome,
                PatternOutcomeV1::Improvement,
                1,
                1,
            ),
            evidence(
                "evidence-2",
                at("benchmark-a", "vectorized-load"),
                PatternEvidenceClassV1::OperationalOutcome,
                PatternOutcomeV1::Tie,
                2,
                2,
            ),
            evidence(
                "evidence-3",
                at("benchmark-b", "scalar-load"),
                PatternEvidenceClassV1::OperationalOutcome,
                PatternOutcomeV1::Failure,
                3,
                3,
            ),
        ],
    ));
    cases.push((
        "three patterns in one bucket",
        vec![
            evidence(
                "evidence-1",
                at("benchmark", "scalar-load"),
                PatternEvidenceClassV1::OperationalOutcome,
                PatternOutcomeV1::Timeout,
                4,
                4,
            ),
            evidence(
                "evidence-2",
                at("benchmark", "vectorized-load"),
                PatternEvidenceClassV1::OperationalOutcome,
                PatternOutcomeV1::Regression,
                5,
                5,
            ),
            evidence(
                "evidence-3",
                at("benchmark", "other-mechanism"),
                PatternEvidenceClassV1::OperationalOutcome,
                PatternOutcomeV1::Tie,
                6,
                6,
            ),
        ],
    ));
    cases.push((
        "twelve rows on one pattern",
        (0..12u64)
            .map(|row| {
                let class = match row {
                    0 => official(1),
                    1 | 2 => transfer(),
                    _ => PatternEvidenceClassV1::OperationalOutcome,
                };
                evidence(
                    &format!("evidence-{row}"),
                    key(),
                    class,
                    outcome((row % 5) as usize),
                    (row * 7) as u16,
                    row * 1_000_003,
                )
            })
            .collect(),
    ));
    cases.push((
        "a cost sum that fills the low half and carries",
        vec![
            evidence(
                "evidence-1",
                key(),
                PatternEvidenceClassV1::OperationalOutcome,
                PatternOutcomeV1::Improvement,
                0,
                u64::MAX - 10,
            ),
            evidence(
                "evidence-2",
                key(),
                PatternEvidenceClassV1::OperationalOutcome,
                PatternOutcomeV1::Improvement,
                0,
                5,
            ),
            evidence(
                "evidence-3",
                key(),
                PatternEvidenceClassV1::OperationalOutcome,
                PatternOutcomeV1::Improvement,
                0,
                5,
            ),
        ],
    ));
    cases.push((
        "a cost one below the top of u64",
        vec![evidence(
            "evidence-1",
            key(),
            PatternEvidenceClassV1::OperationalOutcome,
            PatternOutcomeV1::Timeout,
            1_000,
            u64::MAX - 1,
        )],
    ));
    cases.push((
        "bucket keys that share a prefix stay ordered",
        vec![
            evidence(
                "evidence-1",
                at("benchmark-2", "vectorized-load"),
                PatternEvidenceClassV1::OperationalOutcome,
                PatternOutcomeV1::Improvement,
                1,
                1,
            ),
            evidence(
                "evidence-2",
                at("benchmark", "vectorized-load"),
                PatternEvidenceClassV1::OperationalOutcome,
                PatternOutcomeV1::Tie,
                1,
                1,
            ),
        ],
    ));
    cases.push((
        "mechanisms that share a prefix stay ordered",
        vec![
            evidence(
                "evidence-1",
                at("benchmark", "scalar-load-2"),
                PatternEvidenceClassV1::OperationalOutcome,
                PatternOutcomeV1::Failure,
                2,
                2,
            ),
            evidence(
                "evidence-2",
                at("benchmark", "scalar-load"),
                PatternEvidenceClassV1::OperationalOutcome,
                PatternOutcomeV1::Failure,
                2,
                2,
            ),
        ],
    ));
    cases.push((
        "every class on its own pattern of one bucket",
        vec![
            evidence(
                "evidence-1",
                at("benchmark", "mechanism-a"),
                official(9),
                PatternOutcomeV1::Improvement,
                7,
                70,
            ),
            evidence(
                "evidence-2",
                at("benchmark", "mechanism-b"),
                transfer(),
                PatternOutcomeV1::Timeout,
                8,
                80,
            ),
            evidence(
                "evidence-3",
                at("benchmark", "mechanism-c"),
                PatternEvidenceClassV1::OperationalOutcome,
                PatternOutcomeV1::Failure,
                9,
                90,
            ),
        ],
    ));
    cases.push((
        "one episode across two buckets",
        vec![
            evidence(
                "evidence-1",
                at("benchmark-a", "vectorized-load"),
                official(4),
                PatternOutcomeV1::Improvement,
                3,
                30,
            ),
            evidence(
                "evidence-2",
                at("benchmark-b", "vectorized-load"),
                PatternEvidenceClassV1::OperationalOutcome,
                PatternOutcomeV1::Regression,
                4,
                40,
            ),
            evidence(
                "evidence-3",
                at("benchmark-a", "vectorized-load"),
                PatternEvidenceClassV1::OperationalOutcome,
                PatternOutcomeV1::Tie,
                5,
                50,
            ),
        ],
    ));
    cases.push((
        "a failure and a timeout on two patterns of one bucket",
        vec![
            evidence(
                "evidence-1",
                at("benchmark", "mechanism-a"),
                PatternEvidenceClassV1::OperationalOutcome,
                PatternOutcomeV1::Failure,
                0,
                0,
            ),
            evidence(
                "evidence-2",
                at("benchmark", "mechanism-b"),
                PatternEvidenceClassV1::OperationalOutcome,
                PatternOutcomeV1::Timeout,
                0,
                0,
            ),
        ],
    ));
    cases.push((
        "buckets arriving out of key order",
        vec![
            evidence(
                "evidence-1",
                at("benchmark-b", "vectorized-load"),
                PatternEvidenceClassV1::OperationalOutcome,
                PatternOutcomeV1::Improvement,
                6,
                60,
            ),
            evidence(
                "evidence-2",
                at("benchmark", "vectorized-load"),
                PatternEvidenceClassV1::OperationalOutcome,
                PatternOutcomeV1::Improvement,
                7,
                70,
            ),
            evidence(
                "evidence-3",
                at("benchmark-a", "vectorized-load"),
                PatternEvidenceClassV1::OperationalOutcome,
                PatternOutcomeV1::Improvement,
                8,
                80,
            ),
        ],
    ));
    cases
}

#[test]
fn the_leg_reduces_every_planned_case_like_the_native_pass() {
    let leg = shipped_leg(LEG);
    let cases = cases();
    assert!(cases.len() >= 30, "the table lost its edge cases");
    for (label, evidence) in cases {
        let (native, native_source) = reduce_fields_with(None, &evidence)
            .unwrap_or_else(|error| panic!("{label}: native refused the case: {error}"));
        assert_eq!(native_source, LegSource::Native, "{label}: native leg");
        let (live, source) = reduce_fields_with(Some(leg.clone()), &evidence)
            .unwrap_or_else(|error| panic!("{label}: live leg refused the case: {error}"));
        assert!(
            matches!(source, LegSource::Live { .. }),
            "{label}: {source:?} kept the leg"
        );
        assert_eq!(live, native, "{label}: leg and native disagree");
    }
}

#[test]
fn the_leg_still_folds_the_counters_the_board_reads() {
    // One hand-checked case, so parity is not the only evidence: three rows on
    // one pattern must land in exactly the counters the oracle documents.
    let evidence = vec![
        evidence(
            "evidence-1",
            key(),
            official(1),
            PatternOutcomeV1::Improvement,
            250,
            7,
        ),
        evidence(
            "evidence-2",
            key(),
            PatternEvidenceClassV1::OperationalOutcome,
            PatternOutcomeV1::Timeout,
            5,
            1,
        ),
        evidence(
            "evidence-3",
            key(),
            PatternEvidenceClassV1::OperationalOutcome,
            PatternOutcomeV1::Timeout,
            5,
            2,
        ),
    ];
    let (fields, _) = reduce_fields_with(Some(shipped_leg(LEG)), &evidence).unwrap();
    assert_eq!(fields.len(), 1);
    let aggregate = &fields[0].patterns[0].aggregate;
    assert_eq!(aggregate.confirmed_official, 1);
    assert_eq!(aggregate.provisional_transfer, 0);
    assert_eq!(aggregate.operational, 2);
    assert_eq!(aggregate.improvements, 1);
    assert_eq!(aggregate.timeouts, 2);
    assert_eq!(aggregate.ties, 0);
    assert_eq!(aggregate.uncertainty_millis, 260);
    assert_eq!(aggregate.cost_microunits, 10);
}

/// The fixture leg in place of a shipped generation, for a shape the host has
/// to refuse.
fn temporary_policy_leg(id: &str, render: &str) -> angel_live::Program {
    let root = std::env::temp_dir().join(format!("angel-patterns-leg-{}-{id}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let module = root.join(id);
    std::fs::create_dir_all(&module).unwrap();
    std::fs::write(
        module.join("module.json"),
        format!(
            r#"{{"api_version":1,"id":"{id}","kind":"policy","entry":"index.js","probe":{{}}}}"#
        ),
    )
    .unwrap();
    std::fs::write(module.join("index.js"), render).unwrap();
    let mut registry =
        angel_live::Registry::validated(root, crate::agent::hotload::evaluate, |_, _| Ok(()));
    registry.refresh(true);
    registry
        .snapshot()
        .policies
        .get(id)
        .cloned()
        .unwrap_or_else(|| panic!("fixture leg {id} did not publish"))
}

#[test]
fn an_out_of_contract_reduction_keeps_the_native_decision() {
    let evidence = vec![
        evidence(
            "evidence-1",
            key(),
            PatternEvidenceClassV1::OperationalOutcome,
            PatternOutcomeV1::Improvement,
            1,
            1,
        ),
        evidence(
            "evidence-2",
            at("benchmark-b", "vectorized-load"),
            PatternEvidenceClassV1::OperationalOutcome,
            PatternOutcomeV1::Tie,
            2,
            2,
        ),
    ];
    let (native, _) = reduce_fields_with(None, &evidence).unwrap();
    let truncated = temporary_policy_leg(
        "patterns-reducer-truncated",
        "module.exports={apiVersion:1,render:()=>({buckets:[]})}",
    );
    let (decision, source) = reduce_fields_with(Some(truncated), &evidence).unwrap();
    assert_eq!(decision, native);
    assert!(
        matches!(source, LegSource::LiveFallback { .. }),
        "{source:?} kept the leg"
    );
}

#[test]
fn a_saturated_counter_falls_back_to_the_native_refusal() {
    // Two rows whose cost sums past u64::MAX: the leg raises `over`, the host
    // refuses that answer and keeps the native `checked_add` error verbatim.
    let evidence = vec![
        evidence(
            "evidence-1",
            key(),
            PatternEvidenceClassV1::OperationalOutcome,
            PatternOutcomeV1::Improvement,
            0,
            u64::MAX,
        ),
        evidence(
            "evidence-2",
            key(),
            PatternEvidenceClassV1::OperationalOutcome,
            PatternOutcomeV1::Improvement,
            0,
            1,
        ),
    ];
    assert_eq!(
        reduce_fields_with(None, &evidence).unwrap_err(),
        "pattern cost exhausted"
    );
    assert_eq!(
        reduce_fields_with(Some(shipped_leg(LEG)), &evidence).unwrap_err(),
        "pattern cost exhausted"
    );
}

#[test]
fn the_leg_decides_in_microseconds() {
    let mut rows: Vec<PatternEvidenceV1> = Vec::new();
    for bucket in 0..4u64 {
        for pattern in 0..3u64 {
            for row in 0..5u64 {
                let index = bucket * 15 + pattern * 5 + row;
                let class = match row {
                    // One official binding per pattern: a second would need to
                    // correct the first, which the canonical scan rejects.
                    0 => official(bucket * 3 + pattern + 1),
                    1 => transfer(),
                    _ => PatternEvidenceClassV1::OperationalOutcome,
                };
                rows.push(evidence(
                    &format!("evidence-{index}"),
                    at(
                        &format!("benchmark-{bucket}"),
                        &format!("mechanism-{pattern}"),
                    ),
                    class,
                    outcome((index % 5) as usize),
                    (index % 40) as u16,
                    index,
                ));
            }
        }
    }
    let leg = shipped_leg(LEG);
    let (native, _) = reduce_fields_with(None, &rows).unwrap();
    let mut samples: Vec<std::time::Duration> = Vec::with_capacity(200);
    for _ in 0..200 {
        let started = std::time::Instant::now();
        let (fields, source) = reduce_fields_with(Some(leg.clone()), &rows).unwrap();
        samples.push(started.elapsed());
        assert!(matches!(source, LegSource::Live { .. }), "{source:?}");
        assert_eq!(fields, native);
    }
    samples.sort();
    eprintln!(
        "patterns-reducer leg: {:.1} µs median, {:.1} µs p95 over 200 decisions",
        samples[100].as_secs_f64() * 1e6,
        samples[190].as_secs_f64() * 1e6
    );
}

/// With no live generation offered, the shipped module still answers and the
/// receipt says where the answer came from.
#[test]
fn a_turn_with_no_live_generation_uses_the_shipped_patterns_reducer() {
    use super::patterns_reducer::reduce_fields_shipped;
    let evidence = vec![
        evidence(
            "evidence-1",
            key(),
            official(1),
            PatternOutcomeV1::Improvement,
            250,
            7,
        ),
        evidence(
            "evidence-2",
            key(),
            PatternEvidenceClassV1::OperationalOutcome,
            PatternOutcomeV1::Timeout,
            5,
            1,
        ),
    ];
    let (fields, source) = reduce_fields_shipped(&evidence).unwrap();
    assert!(
        matches!(source, LegSource::Shipped { .. }),
        "{source:?} did not name the shipped leg"
    );
    let (native, _) = reduce_fields_with(None, &evidence).unwrap();
    assert_eq!(fields, native, "the shipped leg left the native reduction");
}
