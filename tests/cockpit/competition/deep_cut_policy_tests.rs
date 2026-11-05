use super::deep_cut_policy::{
    DeepCutDirectionV1, DirectionDecisionBasisV1, DirectionDecisionV1, DirectionEvidenceUseKindV1,
    DirectionEvidenceUseV1, DirectionLegOutV1, allocate_direction_via_leg, decided, prepare,
};
use super::patterns::{FieldPatternMemoryV1, PatternKeyV1, PatternOutcomeV1};
use super::patterns_adversarial_tests::corrected_memory;
use super::patterns_tests::key;
use crate::agent::harness::live_policy::{self, LegSource, decide_with, shipped_leg};
use std::time::Instant;

// The parity oracle: the allocator the shipped `deep-cut` leg replaced. It
// lives here so production keeps only the features, the input and the check.
fn allocate_direction(
    memory: &FieldPatternMemoryV1,
    directions: &[DeepCutDirectionV1],
    fallback_direction_id: &str,
    now_ms: u64,
) -> Result<DirectionDecisionV1, String> {
    use super::schema::ScheduledActionV1;
    use super::schema_validation::validate_id;
    memory.validate()?;
    validate_id(fallback_direction_id, "invalid fallback direction").map_err(str::to_string)?;
    let mut seen = std::collections::BTreeSet::new();
    let mut eligible = Vec::new();
    for direction in directions {
        validate_id(&direction.direction_id, "invalid direction id").map_err(str::to_string)?;
        direction.target_key.validate()?;
        if !seen.insert(direction.direction_id.clone()) {
            return Err("duplicate deep-cut direction id".into());
        }
        if let Some((score, basis)) = score_direction(memory, direction)? {
            eligible.push((score, direction.direction_id.as_str(), basis));
        }
    }
    eligible.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(right.1)));
    let (selected, basis) = eligible
        .first()
        .map(|(_, id, basis)| ((*id).to_string(), *basis))
        .unwrap_or_else(|| {
            (
                fallback_direction_id.to_string(),
                DirectionDecisionBasisV1::UntriedFallback,
            )
        });
    Ok(DirectionDecisionV1 {
        next: ScheduledActionV1 {
            action: format!("explore-direction:{selected}"),
            next_attempt_at_ms: now_ms,
        },
        selected_direction_id: selected,
        basis,
    })
}

fn score_direction(
    memory: &FieldPatternMemoryV1,
    direction: &DeepCutDirectionV1,
) -> Result<Option<(i64, DirectionDecisionBasisV1)>, String> {
    use super::patterns::PatternEvidenceClassV1;
    use super::schema_validation::validate_id;
    let mut ids = std::collections::BTreeSet::new();
    let mut score = 0i64;
    let mut saw_target = false;
    let mut saw_transfer = false;
    for usage in &direction.evidence {
        validate_id(&usage.evidence_id, "invalid direction evidence id").map_err(str::to_string)?;
        if !ids.insert(&usage.evidence_id) {
            return Err("duplicate evidence use in deep-cut direction".into());
        }
        let Some(evidence) = memory.canonical_active_evidence(&usage.evidence_id)? else {
            return Ok(None);
        };
        if evidence.target_key != direction.target_key {
            return Ok(None);
        }
        let provisional = matches!(
            evidence.class,
            PatternEvidenceClassV1::ProvisionalTransfer { .. }
        );
        match (usage.kind, provisional) {
            (DirectionEvidenceUseKindV1::TargetEvidence, true)
            | (DirectionEvidenceUseKindV1::MarkedProvisionalTransfer, false) => return Ok(None),
            (DirectionEvidenceUseKindV1::TargetEvidence, false) => saw_target = true,
            (DirectionEvidenceUseKindV1::MarkedProvisionalTransfer, true) => saw_transfer = true,
        }
        let weight = match evidence.outcome {
            PatternOutcomeV1::Improvement => 4,
            PatternOutcomeV1::Tie => 1,
            PatternOutcomeV1::Regression => -4,
            PatternOutcomeV1::Failure => -5,
            PatternOutcomeV1::Timeout => -3,
        };
        score = score.saturating_add(weight);
        let cost_penalty = i64::try_from(evidence.cost_microunits / 1_000_000).unwrap_or(i64::MAX);
        score = score.saturating_sub(cost_penalty);
    }
    let basis = match (saw_target, saw_transfer, direction.evidence.is_empty()) {
        (true, _, _) => DirectionDecisionBasisV1::TargetEvidence,
        (false, true, _) => DirectionDecisionBasisV1::ExplicitProvisionalTransfer,
        (_, _, true) => DirectionDecisionBasisV1::UntriedFallback,
        _ => return Ok(None),
    };
    Ok(Some((score, basis)))
}

/// The oracle's answer to the same prepared call.
fn oracle_answer(
    memory: &FieldPatternMemoryV1,
    directions: &[DeepCutDirectionV1],
    fallback: &str,
    now_ms: u64,
) -> Result<DirectionLegOutV1, String> {
    allocate_direction(memory, directions, fallback, now_ms).map(|native| DirectionLegOutV1 {
        selected_direction_id: native.selected_direction_id,
        basis: native.basis,
    })
}

#[test]
fn dc_pat_005_direction_allocation_never_mixes_unmarked_transfer() {
    let source = key();
    let mut transfer_target = key();
    transfer_target.hardware_id = "transfer-target".into();
    let mut local_target = key();
    local_target.mechanism = "local-alternative".into();
    let mut memory = FieldPatternMemoryV1::default();
    memory
        .record_transfer(
            "transfer-evidence".into(),
            "episode-transfer".into(),
            source,
            transfer_target.clone(),
            PatternOutcomeV1::Improvement,
            100,
            1,
        )
        .unwrap();
    memory
        .record_operational(
            "local-evidence".into(),
            "episode-local".into(),
            local_target.clone(),
            PatternOutcomeV1::Timeout,
            100,
            1,
        )
        .unwrap();

    let silent_transfer = DeepCutDirectionV1 {
        direction_id: "silent-transfer".into(),
        target_key: transfer_target.clone(),
        evidence: vec![DirectionEvidenceUseV1 {
            evidence_id: "transfer-evidence".into(),
            kind: DirectionEvidenceUseKindV1::TargetEvidence,
        }],
    };
    let local_alternative = DeepCutDirectionV1 {
        direction_id: "local-alternative".into(),
        target_key: local_target,
        evidence: vec![DirectionEvidenceUseV1 {
            evidence_id: "local-evidence".into(),
            kind: DirectionEvidenceUseKindV1::TargetEvidence,
        }],
    };
    let decision = allocate_direction(
        &memory,
        &[silent_transfer.clone(), local_alternative],
        "fresh-direction",
        90,
    )
    .unwrap();
    assert_eq!(decision.selected_direction_id, "local-alternative");
    assert_eq!(decision.basis, DirectionDecisionBasisV1::TargetEvidence);
    assert_eq!(decision.next.action, "explore-direction:local-alternative");

    let fallback = allocate_direction(&memory, &[silent_transfer], "fresh-direction", 91).unwrap();
    assert_eq!(fallback.selected_direction_id, "fresh-direction");
    assert_eq!(fallback.basis, DirectionDecisionBasisV1::UntriedFallback);
    assert_eq!(fallback.next.next_attempt_at_ms, 91);

    let marked = DeepCutDirectionV1 {
        direction_id: "marked-transfer".into(),
        target_key: transfer_target,
        evidence: vec![DirectionEvidenceUseV1 {
            evidence_id: "transfer-evidence".into(),
            kind: DirectionEvidenceUseKindV1::MarkedProvisionalTransfer,
        }],
    };
    let explicit = allocate_direction(&memory, &[marked], "fresh-direction", 92).unwrap();
    assert_eq!(explicit.selected_direction_id, "marked-transfer");
    assert_eq!(
        explicit.basis,
        DirectionDecisionBasisV1::ExplicitProvisionalTransfer
    );

    let mut tampered = serde_json::to_value(&memory).unwrap();
    tampered["fields"][0]["patterns"][0]["aggregate"]["improvements"] = 99.into();
    let tampered = serde_json::from_value(tampered).unwrap();
    assert!(allocate_direction(&tampered, &[], "fresh-direction", 93).is_err());
}

#[test]
fn allocator_after_correction_uses_only_canonical_active_evidence() {
    let (memory, target_key) = corrected_memory();
    let old_id = memory.evidence_records()[0].evidence_id.clone();
    let active_id = memory.evidence_records()[1].evidence_id.clone();
    assert!(memory.canonical_active_evidence(&old_id).unwrap().is_none());
    assert!(
        memory
            .canonical_active_evidence(&active_id)
            .unwrap()
            .is_some()
    );

    let usage = |evidence_id| DirectionEvidenceUseV1 {
        evidence_id,
        kind: DirectionEvidenceUseKindV1::TargetEvidence,
    };
    let superseded = DeepCutDirectionV1 {
        direction_id: "superseded-direction".into(),
        target_key: target_key.clone(),
        evidence: vec![usage(old_id.clone())],
    };
    let mixed = DeepCutDirectionV1 {
        direction_id: "mixed-direction".into(),
        target_key: target_key.clone(),
        evidence: vec![usage(old_id), usage(active_id.clone())],
    };
    let canonical = DeepCutDirectionV1 {
        direction_id: "canonical-direction".into(),
        target_key,
        evidence: vec![usage(active_id)],
    };
    let decision = allocate_direction(
        &memory,
        &[superseded.clone(), mixed.clone(), canonical],
        "fresh-direction",
        100,
    )
    .unwrap();
    assert_eq!(decision.selected_direction_id, "canonical-direction");
    assert_eq!(decision.basis, DirectionDecisionBasisV1::TargetEvidence);

    let fallback =
        allocate_direction(&memory, &[superseded, mixed], "fresh-direction", 101).unwrap();
    assert_eq!(fallback.selected_direction_id, "fresh-direction");
    assert_eq!(fallback.basis, DirectionDecisionBasisV1::UntriedFallback);
}

// --- the hot-loaded `deep-cut` leg -------------------------------------------

const OUTCOMES: [PatternOutcomeV1; 5] = [
    PatternOutcomeV1::Improvement,
    PatternOutcomeV1::Tie,
    PatternOutcomeV1::Regression,
    PatternOutcomeV1::Failure,
    PatternOutcomeV1::Timeout,
];

struct Fixture {
    memory: FieldPatternMemoryV1,
    transfers: Vec<(String, PatternKeyV1)>,
    operational: Vec<(String, PatternKeyV1)>,
}

/// One provisional-transfer record per outcome, plus the operational failures
/// and timeouts the memory contract allows. Each record costs whole units, so a
/// direction's score mixes the outcome weight with its cost penalty.
fn fixture() -> Fixture {
    let source = key();
    let mut memory = FieldPatternMemoryV1::default();
    let mut transfers = Vec::new();
    for (index, outcome) in OUTCOMES.into_iter().enumerate() {
        let mut target = key();
        target.mechanism = format!("transfer-{index}");
        let id = format!("transfer-{index}");
        memory
            .record_transfer(
                id.clone(),
                format!("episode-transfer-{index}"),
                source.clone(),
                target.clone(),
                outcome,
                100,
                1_000_000,
            )
            .unwrap();
        transfers.push((id, target));
    }
    let mut operational = Vec::new();
    for (index, outcome) in [PatternOutcomeV1::Failure, PatternOutcomeV1::Timeout]
        .into_iter()
        .enumerate()
    {
        let mut target = key();
        target.mechanism = format!("operational-{index}");
        let id = format!("operational-{index}");
        memory
            .record_operational(
                id.clone(),
                format!("episode-operational-{index}"),
                target.clone(),
                outcome,
                100,
                2_500_000,
            )
            .unwrap();
        operational.push((id, target));
    }
    memory.validate().unwrap();
    Fixture {
        memory,
        transfers,
        operational,
    }
}

fn usage(evidence_id: &str, kind: DirectionEvidenceUseKindV1) -> DirectionEvidenceUseV1 {
    DirectionEvidenceUseV1 {
        evidence_id: evidence_id.into(),
        kind,
    }
}

fn direction(
    id: &str,
    target_key: PatternKeyV1,
    evidence: Vec<DirectionEvidenceUseV1>,
) -> DeepCutDirectionV1 {
    DeepCutDirectionV1 {
        direction_id: id.into(),
        target_key,
        evidence,
    }
}

fn table(fixture: &Fixture) -> Vec<(&'static str, Vec<DeepCutDirectionV1>, &'static str)> {
    let marked = DirectionEvidenceUseKindV1::MarkedProvisionalTransfer;
    let target = DirectionEvidenceUseKindV1::TargetEvidence;
    let mut cases: Vec<(&'static str, Vec<DeepCutDirectionV1>, &'static str)> = vec![
        ("no directions", Vec::new(), "fresh-direction"),
        (
            "no evidence at all",
            vec![direction("untried", key(), Vec::new())],
            "fresh-direction",
        ),
        (
            "the fallback is also offered",
            vec![
                direction("fresh-direction", key(), Vec::new()),
                direction("untried", key(), Vec::new()),
            ],
            "fresh-direction",
        ),
    ];
    for (id, evidence_target) in &fixture.transfers {
        cases.push((
            "marked provisional transfer",
            vec![direction(
                "marked",
                evidence_target.clone(),
                vec![usage(id, marked)],
            )],
            "fresh-direction",
        ));
        cases.push((
            "an unmarked provisional transfer",
            vec![direction(
                "unmarked",
                evidence_target.clone(),
                vec![usage(id, target)],
            )],
            "fresh-direction",
        ));
        cases.push((
            "evidence that is not active",
            vec![direction(
                "inactive",
                evidence_target.clone(),
                vec![usage("missing-evidence", target)],
            )],
            "fresh-direction",
        ));
        let mut elsewhere = evidence_target.clone();
        elsewhere.mechanism = "elsewhere".into();
        cases.push((
            "evidence for another target",
            vec![direction("mismatch", elsewhere, vec![usage(id, marked)])],
            "fresh-direction",
        ));
    }
    for (id, evidence_target) in &fixture.operational {
        cases.push((
            "operational evidence",
            vec![direction(
                "operational",
                evidence_target.clone(),
                vec![usage(id, target)],
            )],
            "fresh-direction",
        ));
        cases.push((
            "operational evidence marked as a transfer",
            vec![direction(
                "unmarked-operational",
                evidence_target.clone(),
                vec![usage(id, marked)],
            )],
            "fresh-direction",
        ));
    }
    let (improvement_id, improvement_target) = &fixture.transfers[0];
    let (tie_id, tie_target) = &fixture.transfers[1];
    let (regression_id, regression_target) = &fixture.transfers[2];
    let (timeout_id, _) = &fixture.transfers[4];
    let (failure_id, _) = &fixture.operational[0];
    cases.push((
        "three uses in one direction",
        vec![direction(
            "stacked",
            improvement_target.clone(),
            vec![
                usage(improvement_id, marked),
                usage(timeout_id, marked),
                usage(failure_id, target),
            ],
        )],
        "fresh-direction",
    ));
    cases.push((
        "an improvement outranks a tie",
        vec![
            direction("low", tie_target.clone(), vec![usage(tie_id, marked)]),
            direction(
                "high",
                improvement_target.clone(),
                vec![usage(improvement_id, marked)],
            ),
        ],
        "fresh-direction",
    ));
    cases.push((
        "a tie outranks a regression",
        vec![
            direction(
                "regression",
                regression_target.clone(),
                vec![usage(regression_id, marked)],
            ),
            direction("tie", tie_target.clone(), vec![usage(tie_id, marked)]),
        ],
        "fresh-direction",
    ));
    cases.push((
        "equal scores break towards the smaller id",
        vec![
            direction("beta", tie_target.clone(), vec![usage(tie_id, marked)]),
            direction(
                "alpha",
                improvement_target.clone(),
                vec![usage(improvement_id, marked)],
            ),
        ],
        "fresh-direction",
    ));
    cases.push((
        "only the fallback survives",
        vec![
            direction("silent", tie_target.clone(), vec![usage(tie_id, target)]),
            direction(
                "mismatched",
                regression_target.clone(),
                vec![usage(improvement_id, marked)],
            ),
        ],
        "fresh-direction",
    ));
    cases
}

#[test]
fn deep_cut_leg_matches_the_native_allocator_over_a_table_of_directions() {
    let leg = shipped_leg("deep-cut");
    let fixture = fixture();
    let cases = table(&fixture);
    assert!(
        cases.len() >= 30,
        "the parity table needs at least 30 inputs, saw {}",
        cases.len()
    );
    for (offset, (name, directions, fallback)) in cases.iter().enumerate() {
        let now_ms = 90 + offset as u64;
        let call = prepare(&fixture.memory, directions, fallback)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let native = allocate_direction(&fixture.memory, directions, fallback, now_ms)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let (answer, source) = decide_with(
            Some(leg.clone()),
            "deep-cut",
            &call.input,
            |answer: &DirectionLegOutV1| call.check(answer),
            || DirectionLegOutV1 {
                selected_direction_id: native.selected_direction_id.clone(),
                basis: native.basis,
            },
        );
        assert!(
            matches!(source, LegSource::Live { .. }),
            "{name}: {source:?}"
        );
        assert_eq!(
            answer,
            DirectionLegOutV1 {
                selected_direction_id: native.selected_direction_id.clone(),
                basis: native.basis,
            },
            "{name}: the leg and the native allocator disagree"
        );
        assert_eq!(decided(answer, now_ms), native, "{name}");
    }
}

#[test]
fn deep_cut_leg_keeps_the_native_error_for_malformed_input() {
    let fixture = fixture();
    let (id, target) = &fixture.transfers[0];
    let cases = [
        (
            "a duplicate direction id",
            vec![
                direction("same", target.clone(), Vec::new()),
                direction("same", target.clone(), Vec::new()),
            ],
        ),
        (
            "a duplicate evidence use",
            vec![direction(
                "twice",
                target.clone(),
                vec![
                    usage(id, DirectionEvidenceUseKindV1::MarkedProvisionalTransfer),
                    usage(id, DirectionEvidenceUseKindV1::MarkedProvisionalTransfer),
                ],
            )],
        ),
        (
            "an invalid direction id",
            vec![direction("", target.clone(), Vec::new())],
        ),
    ];
    for (name, directions) in cases {
        let refused = prepare(&fixture.memory, &directions, "fresh-direction")
            .expect_err("malformed input reaches the leg");
        let native = allocate_direction(&fixture.memory, &directions, "fresh-direction", 1)
            .expect_err("malformed input reaches the allocator");
        assert_eq!(refused, native, "{name}");
    }
    let mut tampered = serde_json::to_value(&fixture.memory).unwrap();
    tampered["fields"][0]["patterns"][0]["aggregate"]["improvements"] = 99.into();
    let tampered = serde_json::from_value(tampered).unwrap();
    assert!(prepare(&tampered, &[], "fresh-direction").is_err());
}

/// A policy module from elsewhere in the tree that answers with a direction id
/// no offer contains: the leg loads and runs, but its answer is out of contract.
fn doctored_leg() -> angel_live::Program {
    let root = std::env::temp_dir().join(format!("angel-deep-cut-doctor-{}", std::process::id()));
    let module = root.join("deep-cut");
    std::fs::create_dir_all(&module).unwrap();
    std::fs::write(
        module.join("module.json"),
        r#"{"api_version":1,"id":"deep-cut","kind":"policy","entry":"index.js","probe":{}}"#,
    )
    .unwrap();
    std::fs::write(
        module.join("index.js"),
        "module.exports={apiVersion:1,render:()=>({selected_direction_id:'ghost-direction',basis:'target_evidence'})}",
    )
    .unwrap();
    let mut registry =
        angel_live::Registry::validated(root, crate::agent::hotload::evaluate, |_, _| Ok(()));
    registry.refresh(true);
    registry
        .snapshot()
        .policies
        .get("deep-cut")
        .cloned()
        .expect("the doctored leg loads")
}

#[test]
fn an_out_of_contract_leg_answer_falls_back_to_the_native_direction() {
    let fixture = fixture();
    let (id, target) = &fixture.transfers[0];
    let directions = vec![
        direction(
            "marked",
            target.clone(),
            vec![usage(
                id,
                DirectionEvidenceUseKindV1::MarkedProvisionalTransfer,
            )],
        ),
        direction("untried", key(), Vec::new()),
    ];
    let call = prepare(&fixture.memory, &directions, "fresh-direction").unwrap();
    let native = allocate_direction(&fixture.memory, &directions, "fresh-direction", 12).unwrap();
    let (answer, source) = decide_with(
        Some(doctored_leg()),
        "deep-cut",
        &call.input,
        |answer: &DirectionLegOutV1| call.check(answer),
        || oracle_answer(&fixture.memory, &directions, "fresh-direction", 12).unwrap(),
    );
    match &source {
        LegSource::LiveFallback { error, .. } => {
            assert!(error.contains("unoffered direction"), "{error}")
        }
        other => panic!("expected a checked fallback, saw {other:?}"),
    }
    assert_eq!(decided(answer, 12), native);
}

#[test]
fn the_published_leg_decides_through_the_shipped_route() {
    let fixture = fixture();
    let directions = table(&fixture).swap_remove(20).1;
    let native = allocate_direction(&fixture.memory, &directions, "fresh-direction", 33).unwrap();
    // Nothing published: the generation compiled into this binary answers.
    let (shipped_decision, shipped_source) =
        allocate_direction_via_leg(&fixture.memory, &directions, "fresh-direction", 33).unwrap();
    assert!(
        matches!(shipped_source, LegSource::Shipped { .. }),
        "{shipped_source:?}"
    );
    assert_eq!(shipped_decision, native);
    // The route reads the legs a loader publishes; nothing else in this binary
    // publishes them, and the map is cleared again before the test ends.
    live_policy::offer(&std::collections::BTreeMap::from([(
        "deep-cut".to_string(),
        shipped_leg("deep-cut"),
    )]));
    let decided = allocate_direction_via_leg(&fixture.memory, &directions, "fresh-direction", 33);
    live_policy::offer(&std::collections::BTreeMap::new());
    let (decision, source) = decided.unwrap();
    assert!(matches!(source, LegSource::Live { .. }), "{source:?}");
    assert_eq!(decision, native);
}

#[test]
fn deep_cut_leg_reports_median_microseconds_per_decision() {
    let leg = shipped_leg("deep-cut");
    let fixture = fixture();
    let directions = table(&fixture).swap_remove(20).1;
    let call = prepare(&fixture.memory, &directions, "fresh-direction").unwrap();
    let oracle = oracle_answer(&fixture.memory, &directions, "fresh-direction", 7).unwrap();
    let mut samples = Vec::with_capacity(200);
    for _ in 0..200 {
        let started = Instant::now();
        let (answer, source) = decide_with(
            Some(leg.clone()),
            "deep-cut",
            &call.input,
            |answer: &DirectionLegOutV1| call.check(answer),
            || oracle.clone(),
        );
        std::hint::black_box((answer, source));
        samples.push(started.elapsed());
    }
    samples.sort_unstable();
    eprintln!(
        "deep-cut leg: median {:.1} µs per decision over 200 calls",
        samples[100].as_secs_f64() * 1e6
    );
}
