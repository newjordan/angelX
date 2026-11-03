use super::profile::DeepCutProfileV1;
use super::rewards::{
    BindDispositionV1, OfficialProvenanceV1, OfficialResultSourceV1, OfficialResultV1,
    OrientedComparisonV1, ProvisionalShapingV1, RewardContextV1, RewardLedgerV1, RewardSelectionV1,
};
use super::schema::{
    AdapterCapabilityV1, AdapterIdentityV1, ComparatorKindV1, CompetitionKeyV1,
    ObjectiveComparatorV1, ScoreV1,
};
use std::collections::BTreeSet;

pub(super) fn context(kind: ComparatorKindV1) -> RewardContextV1 {
    RewardContextV1 {
        episode_id: "episode-rwd".into(),
        candidate_id: "candidate-rwd".into(),
        submission_id: "submission-rwd".into(),
        comparable_base_id: "base-rwd".into(),
        competition: CompetitionKeyV1 {
            platform_id: "fixture".into(),
            competition_id: "competition".into(),
            field_id: "field".into(),
            benchmark_id: "benchmark".into(),
            profile_id: "profile".into(),
            hardware_id: "hardware".into(),
        },
        objective: ObjectiveComparatorV1 {
            objective_id: "latency".into(),
            version: "objective-v1".into(),
            kind,
        },
        profile: DeepCutProfileV1::embedded().identity().unwrap(),
    }
}

pub(super) fn official(
    context: &RewardContextV1,
    id: &str,
    revision: u64,
    candidate: &str,
    base: &str,
    corrects_binding_id: Option<String>,
) -> OfficialResultV1 {
    OfficialResultV1 {
        result_id: id.into(),
        result_revision: revision,
        episode_id: context.episode_id.clone(),
        candidate_id: context.candidate_id.clone(),
        submission_id: context.submission_id.clone(),
        comparable_base_id: context.comparable_base_id.clone(),
        competition: context.competition.clone(),
        objective: context.objective.clone(),
        profile: context.profile.clone(),
        candidate_score: ScoreV1::new(candidate).unwrap(),
        base_score: ScoreV1::new(base).unwrap(),
        provenance: OfficialProvenanceV1 {
            source: OfficialResultSourceV1::OfficialSubmissionResult,
            adapter: AdapterIdentityV1 {
                adapter_id: "fixture-adapter".into(),
                adapter_version: "v1".into(),
                runtime_sha256: crate::knowledge::cut::sha256_hex(b"fixture-adapter"),
                capabilities: BTreeSet::from([AdapterCapabilityV1::Results]),
            },
            receipt_sha256: crate::knowledge::cut::sha256_hex(format!("receipt:{id}").as_bytes()),
        },
        corrects_binding_id,
    }
}

#[test]
fn dc_rwd_001_higher_lower_tie_regression_golden_matrix() {
    let cases = [
        (
            ComparatorKindV1::HigherIsBetter,
            "12.5",
            "10.25",
            OrientedComparisonV1::Improvement,
            "2.25",
            1,
        ),
        (
            ComparatorKindV1::LowerIsBetter,
            "8",
            "10",
            OrientedComparisonV1::Improvement,
            "2",
            1,
        ),
        (
            ComparatorKindV1::HigherIsBetter,
            "-1.5",
            "-1.5",
            OrientedComparisonV1::Tie,
            "0",
            0,
        ),
        (
            ComparatorKindV1::HigherIsBetter,
            "8",
            "10",
            OrientedComparisonV1::Regression,
            "-2",
            -1,
        ),
        (
            ComparatorKindV1::LowerIsBetter,
            "12",
            "10",
            OrientedComparisonV1::Regression,
            "-2",
            -1,
        ),
    ];
    for (index, (kind, candidate, base, comparison, delta, reward)) in cases.into_iter().enumerate()
    {
        let context = context(kind);
        let result = official(
            &context,
            &format!("result-{index}"),
            1,
            candidate,
            base,
            None,
        );
        let (_, binding) = RewardLedgerV1::default()
            .bind_official(&context, result)
            .unwrap();
        assert_eq!(binding.oriented_comparison, comparison);
        assert_eq!(binding.oriented_delta.as_str(), delta);
        assert_eq!(binding.primary_reward, reward);
    }
}

#[test]
fn dc_rwd_002_delayed_official_result_overrides_only_shaping() {
    let context = context(ComparatorKindV1::HigherIsBetter);
    let mut ledger = RewardLedgerV1::default();
    ledger
        .record_shaping(ProvisionalShapingV1 {
            shaping_id: "local-measurement".into(),
            episode_id: context.episode_id.clone(),
            value_millis: 900,
            provenance_sha256: crate::knowledge::cut::sha256_hex(b"local"),
        })
        .unwrap();
    assert!(matches!(
        ledger.effective_reward(&context.episode_id),
        Some(RewardSelectionV1::Provisional(_))
    ));
    ledger
        .bind_official(
            &context,
            official(&context, "official-delayed", 1, "8", "10", None),
        )
        .unwrap();
    assert!(matches!(
        ledger.effective_reward(&context.episode_id),
        Some(RewardSelectionV1::Official(binding)) if binding.primary_reward == -1
    ));
    assert_eq!(ledger.shaping().len(), 1);
}

#[test]
fn dc_rwd_003_correction_chain_is_append_only_and_exactly_once() {
    let context = context(ComparatorKindV1::HigherIsBetter);
    let mut ledger = RewardLedgerV1::default();
    let (_, first) = ledger
        .bind_official(
            &context,
            official(&context, "result-v1", 1, "12", "10", None),
        )
        .unwrap();
    let (_, correction) = ledger
        .bind_official(
            &context,
            official(
                &context,
                "result-v2",
                2,
                "8",
                "10",
                Some(first.binding_id.clone()),
            ),
        )
        .unwrap();
    assert_eq!(ledger.bindings().len(), 2);
    assert_eq!(ledger.bindings()[0], first);
    assert_eq!(correction.primary_reward, -1);
    assert_eq!(
        ledger.canonical_binding(&context.episode_id),
        Some(&correction)
    );
    let stale_correction = official(&context, "result-v3", 3, "11", "10", Some(first.binding_id));
    assert!(ledger.bind_official(&context, stale_correction).is_err());
    assert_eq!(ledger.bindings().len(), 2);
}

#[test]
fn dc_rwd_004_rejects_model_gate_self_report_and_unrelated_rank_credit() {
    let context = context(ComparatorKindV1::HigherIsBetter);
    let mut ledger = RewardLedgerV1::default();
    for source in [
        OfficialResultSourceV1::ModelSelfReport,
        OfficialResultSourceV1::ControlGate,
        OfficialResultSourceV1::UnrelatedRankMovement,
    ] {
        let mut result = official(&context, "not-official", 1, "12", "10", None);
        result.provenance.source = source;
        assert!(ledger.bind_official(&context, result).is_err());
    }
    assert!(ledger.bindings().is_empty());
}

#[test]
fn dc_rwd_005_rejects_incompatible_profile_hardware_objective() {
    let context = context(ComparatorKindV1::HigherIsBetter);
    for mutation in 0..3 {
        let mut result = official(&context, "incompatible", 1, "12", "10", None);
        match mutation {
            0 => result.competition.profile_id = "other-profile".into(),
            1 => result.competition.hardware_id = "other-hardware".into(),
            _ => result.objective.version = "objective-v2".into(),
        }
        assert!(
            RewardLedgerV1::default()
                .bind_official(&context, result)
                .is_err()
        );
    }
}

#[test]
fn dc_rwd_006_duplicate_official_event_is_idempotent() {
    let context = context(ComparatorKindV1::HigherIsBetter);
    let result = official(&context, "same-result", 1, "12", "10", None);
    let mut ledger = RewardLedgerV1::default();
    let (_, first) = ledger.bind_official(&context, result.clone()).unwrap();
    let (disposition, duplicate) = ledger.bind_official(&context, result).unwrap();
    assert_eq!(disposition, BindDispositionV1::Duplicate);
    assert_eq!(duplicate, first);
    assert_eq!(ledger.bindings().len(), 1);
    let conflict = official(&context, "same-result", 1, "13", "10", None);
    assert!(ledger.bind_official(&context, conflict).is_err());
    assert_eq!(ledger.bindings().len(), 1);
}

use super::rewards::{
    BindingDecision, BindingOutcome, BindingRow, Incoming, OfficialRewardBindingV1, Selection,
    SelectionKind, ShapingDecision, ShapingOutcome, ShapingRow, binding_input, binding_rows,
    incoming, make_binding, selection_input, shaping_input, shaping_rows,
};

/// One provisional shaping event, shaped like the ledger's own rows.
fn shaping_event(index: usize) -> ProvisionalShapingV1 {
    ProvisionalShapingV1 {
        shaping_id: format!("shaping-{index}"),
        episode_id: "episode-rwd".into(),
        value_millis: index as i64 * 5,
        provenance_sha256: crate::knowledge::cut::sha256_hex(format!("prov-{index}").as_bytes()),
    }
}

/// A ledger built through the ordinary calls: `bindings` official results and
/// `shapings` provisional events, all for the context's episode.
fn rewards_ledger(bindings: usize, shapings: usize) -> (RewardLedgerV1, RewardContextV1) {
    let context = context(ComparatorKindV1::HigherIsBetter);
    let mut ledger = RewardLedgerV1::default();
    for index in 0..shapings {
        ledger.record_shaping(shaping_event(index)).unwrap();
    }
    // One episode admits a first official result and then only corrections that
    // advance its canonical binding, so the fixture chains them.
    let mut previous: Option<String> = None;
    for index in 0..bindings {
        let result = official(
            &context,
            &format!("result-{index}"),
            index as u64 + 1,
            "10.5",
            "10",
            previous.clone(),
        );
        let (_, binding) = ledger.bind_official(&context, result).unwrap();
        previous = Some(binding.binding_id);
    }
    (ledger, context)
}

/// The shipped leg and the native reverse scan must agree on every episode, and
/// the leg must be the one that answered.
#[test]
fn live_rewards_leg_matches_every_native_selection() {
    use crate::agent::harness::live_policy::{LegSource, decide_shipped};
    let mut checked = 0;
    for bindings in 0..6 {
        for shapings in 0..6 {
            let (ledger, context) = rewards_ledger(bindings, shapings);
            for episode in [context.episode_id.clone(), "episode-elsewhere".to_string()] {
                let binding_flags: Vec<bool> = ledger
                    .bindings()
                    .iter()
                    .map(|entry| entry.episode_id == episode)
                    .collect();
                let shaping_flags: Vec<bool> = ledger
                    .shaping()
                    .iter()
                    .map(|entry| entry.episode_id == episode)
                    .collect();
                let native = select_native(&binding_flags, &shaping_flags);
                let input = selection_input(&binding_flags, &shaping_flags);
                let (selection, source) =
                    decide_shipped("rewards", &input, |selection: &Selection| {
                        selection.check(&binding_flags, &shaping_flags)
                    });
                assert_eq!(
                    selection, native,
                    "{bindings} bindings, {shapings} shapings, {episode}"
                );
                assert!(
                    matches!(source, LegSource::Shipped { .. }),
                    "{episode} kept the {} leg: {source:?}",
                    source.as_str()
                );
                checked += 1;
            }
        }
    }
    assert!(checked >= 30, "only {checked} selections were compared");
}

/// Every shaping admission — append, duplicate and conflicting identity — is
/// the same decision on both legs.
#[test]
fn live_rewards_leg_matches_native_shaping_decisions() {
    use crate::agent::harness::live_policy::{LegSource, decide_shipped};
    let pool: Vec<ProvisionalShapingV1> = (0..4).map(shaping_event).collect();
    // Each pool event, plus a same-id event with a different body, so the table
    // reaches the conflicting-identity branch.
    let candidates: Vec<ProvisionalShapingV1> = pool
        .iter()
        .flat_map(|entry| {
            let mut changed = entry.clone();
            changed.value_millis += 1;
            [entry.clone(), changed]
        })
        .collect();
    let mut checked = 0;
    let (mut appends, mut duplicates, mut conflicts) = (0, 0, 0);
    for candidate in &candidates {
        for mask in 0..16u32 {
            let existing: Vec<ProvisionalShapingV1> = pool
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1 << index) != 0)
                .map(|(_, entry)| entry.clone())
                .collect();
            let rows = shaping_rows(&existing, candidate);
            let native = shaping_native(&rows);
            let input = shaping_input(&rows);
            let (decision, source) =
                decide_shipped("rewards", &input, |decision: &ShapingDecision| {
                    decision.check(&rows)
                });
            assert_eq!(decision, native, "mask {mask} for {candidate:?}");
            assert!(
                matches!(source, LegSource::Shipped { .. }),
                "{source:?} kept the native decision"
            );
            match decision.outcome {
                ShapingOutcome::Append => appends += 1,
                ShapingOutcome::Duplicate => duplicates += 1,
                ShapingOutcome::Conflict => conflicts += 1,
            }
            checked += 1;
        }
    }
    assert!(
        checked >= 30,
        "only {checked} shaping decisions were compared"
    );
    assert!(
        appends > 0 && duplicates > 0 && conflicts > 0,
        "the table missed a branch: {appends}/{duplicates}/{conflicts}"
    );
    eprintln!(
        "rewards leg admitted {checked} shaping events: {appends} append, {duplicates} duplicate, {conflicts} conflict"
    );
}

/// Every binding admission the ledger can reach — appended, duplicate by result
/// id or receipt, both conflicts and both correction refusals — is the same
/// decision on both legs.
#[test]
fn live_rewards_leg_matches_native_binding_decisions() {
    use crate::agent::harness::live_policy::{LegSource, decide_shipped};
    let context = context(ComparatorKindV1::HigherIsBetter);
    let first = official(&context, "result-a", 1, "10", "10", None);
    let twin = {
        let mut result = first.clone();
        result.result_id = "result-a-twin".into();
        result
    };
    let changed = {
        let mut result = first.clone();
        result.candidate_score = ScoreV1::new("11").unwrap();
        result
    };
    let later = official(&context, "result-b", 2, "12", "10", None);
    let pool: Vec<OfficialRewardBindingV1> = [first.clone(), twin, changed, later]
        .into_iter()
        .map(|result| make_binding(result).unwrap())
        .collect();
    let first_binding = pool[0].binding_id.clone();
    let receipt_duplicate = {
        let mut result = first.clone();
        result.result_id = "result-y".into();
        make_binding(result).unwrap()
    };
    let receipt_conflict = {
        let mut result = first.clone();
        result.result_id = "result-z".into();
        result.candidate_score = ScoreV1::new("7").unwrap();
        make_binding(result).unwrap()
    };
    let correction = |target: &str, revision: u64, comparable: &str| {
        let mut result = official(
            &context,
            "result-c",
            revision,
            "13",
            "10",
            Some(target.to_string()),
        );
        result.comparable_base_id = comparable.to_string();
        make_binding(result).unwrap()
    };
    let id_conflict = {
        let mut result = official(&context, "result-a", 1, "10", "10", None);
        result.candidate_score = ScoreV1::new("11").unwrap();
        make_binding(result).unwrap()
    };
    let incoming_pool = vec![
        make_binding(official(&context, "result-c", 3, "13", "10", None)).unwrap(),
        id_conflict,
        receipt_duplicate,
        receipt_conflict,
        correction(&first_binding, 2, "base-rwd"),
        correction(&first_binding, 1, "base-rwd"),
        correction("binding-absent", 2, "base-rwd"),
        correction(&first_binding, 2, "base-elsewhere"),
    ];
    let mut checked = 0;
    let mut seen = BTreeSet::new();
    for mask in 0..16u32 {
        let existing: Vec<OfficialRewardBindingV1> = pool
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1 << index) != 0)
            .map(|(_, entry)| entry.clone())
            .collect();
        for candidate in &incoming_pool {
            let rows = binding_rows(&existing, candidate);
            let facts = incoming(candidate);
            let native = binding_native(&rows, &facts);
            let input = binding_input(&rows, &facts);
            let (decision, source) =
                decide_shipped("rewards", &input, |decision: &BindingDecision| {
                    decision.check(&rows, &facts)
                });
            assert_eq!(decision, native, "mask {mask}");
            assert!(
                matches!(source, LegSource::Shipped { .. }),
                "{source:?} kept the native decision"
            );
            seen.insert(format!("{:?}", decision.outcome));
            checked += 1;
        }
    }
    assert!(
        checked >= 30,
        "only {checked} binding decisions were compared"
    );
    assert!(seen.len() >= 6, "the table only reached {seen:?}");
    eprintln!("rewards leg admitted {checked} official results: {seen:?}");
}

/// A throwaway policy module, so a deliberately out-of-contract leg can be
/// pinned without touching the shipped one.
fn temporary_policy_leg(id: &str, render: &str) -> angel_live::Program {
    let root = std::env::temp_dir().join(format!("angel-rewards-leg-{}-{id}", std::process::id()));
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

/// An out-of-contract selection and an out-of-contract admission are both
/// rejected, and the native scan answers instead, so a bad hot edit never moves
/// the ledger's decision.
#[test]
fn an_out_of_contract_rewards_leg_keeps_the_native_decision() {
    use crate::agent::harness::live_policy::{LegSource, decide_with};
    let bindings = [true, false];
    let shaping = [true];
    let native = select_native(&bindings, &shaping);
    let request = selection_input(&bindings, &shaping);
    let beyond = temporary_policy_leg(
        "rewards-beyond",
        "module.exports={apiVersion:1,render:()=>({kind:'official',index:99})}",
    );
    let (selection, source) = decide_with(
        Some(beyond),
        "rewards",
        &request,
        |selection: &Selection| selection.check(&bindings, &shaping),
        || native,
    );
    assert_eq!(selection, native);
    assert!(
        matches!(source, LegSource::LiveFallback { .. }),
        "{source:?} kept the leg"
    );

    let pool = vec![shaping_event(0), shaping_event(1)];
    let rows = shaping_rows(&pool, &shaping_event(1));
    let native = shaping_native(&rows);
    let request = shaping_input(&rows);
    let unmoved = temporary_policy_leg(
        "rewards-conflict",
        "module.exports={apiVersion:1,render:()=>({outcome:'conflict',index:0})}",
    );
    let (decision, source) = decide_with(
        Some(unmoved),
        "rewards",
        &request,
        |decision: &ShapingDecision| decision.check(&rows),
        || native,
    );
    assert_eq!(decision, native);
    assert!(
        matches!(source, LegSource::LiveFallback { .. }),
        "{source:?} kept the leg"
    );
    assert_eq!(native.outcome, ShapingOutcome::Duplicate);

    let no_bindings: Vec<OfficialRewardBindingV1> = Vec::new();
    let candidate = make_binding(official(
        &context(ComparatorKindV1::HigherIsBetter),
        "result-timing",
        1,
        "10",
        "10",
        None,
    ))
    .unwrap();
    let rows = binding_rows(&no_bindings, &candidate);
    let facts = incoming(&candidate);
    let native = binding_native(&rows, &facts);
    let request = binding_input(&rows, &facts);
    let invented = temporary_policy_leg(
        "rewards-invented",
        "module.exports={apiVersion:1,render:()=>({outcome:'duplicate',index:0})}",
    );
    let (decision, source) = decide_with(
        Some(invented),
        "rewards",
        &request,
        |decision: &BindingDecision| decision.check(&rows, &facts),
        || native,
    );
    assert_eq!(decision, native);
    assert_eq!(native.outcome, BindingOutcome::Append);
    assert!(
        matches!(source, LegSource::LiveFallback { .. }),
        "{source:?} kept the leg"
    );
}

/// The leg is a fresh evaluation per decision, so its own cost has to be small
/// against the ledger's.
#[test]
fn live_rewards_leg_decision_timing() {
    use crate::agent::harness::live_policy::decide_shipped;
    let (ledger, context) = rewards_ledger(6, 6);
    let binding_flags: Vec<bool> = ledger
        .bindings()
        .iter()
        .map(|entry| entry.episode_id == context.episode_id)
        .collect();
    let shaping_flags: Vec<bool> = ledger
        .shaping()
        .iter()
        .map(|entry| entry.episode_id == context.episode_id)
        .collect();
    let native = select_native(&binding_flags, &shaping_flags);
    let request = selection_input(&binding_flags, &shaping_flags);
    let mut samples: Vec<std::time::Duration> = Vec::with_capacity(200);
    for _ in 0..200 {
        let started = std::time::Instant::now();
        let (selection, source) = decide_shipped("rewards", &request, |selection: &Selection| {
            selection.check(&binding_flags, &shaping_flags)
        });
        samples.push(started.elapsed());
        assert_eq!(source.as_str(), "shipped");
        assert_eq!(selection, native);
    }
    samples.sort();
    eprintln!(
        "rewards leg: {:.1} µs median, {:.1} µs p95 over 200 decisions",
        samples[100].as_secs_f64() * 1e6,
        samples[190].as_secs_f64() * 1e6
    );
}

// --- the parity oracle ------------------------------------------------------
//
// The three native scans, moved here out of production: the shipped `rewards`
// leg answers every ledger decision now, and these copies exist so the parity
// checks can prove the two still agree.

/// The `canonical_binding` scan kept intact: the newest binding of the episode.
pub(super) fn select_native(bindings: &[bool], shaping: &[bool]) -> Selection {
    let newest = |flags: &[bool]| flags.iter().rposition(|flag| *flag);
    let built = |kind: SelectionKind, index: usize| Selection {
        kind,
        index: Some(index),
    };
    match newest(bindings) {
        Some(index) => built(SelectionKind::Official, index),
        None => match newest(shaping) {
            Some(index) => built(SelectionKind::Provisional, index),
            None => Selection {
                kind: SelectionKind::Absent,
                index: None,
            },
        },
    }
}

/// The `record_shaping` scan kept intact: the first row holding this shaping id.
pub(super) fn shaping_native(rows: &[ShapingRow]) -> ShapingDecision {
    let found = rows.iter().position(|row| row.id_equal);
    match found {
        Some(index) => ShapingDecision {
            outcome: if rows[index].entry_equal {
                ShapingOutcome::Duplicate
            } else {
                ShapingOutcome::Conflict
            },
            index: Some(index),
        },
        None => ShapingDecision {
            outcome: ShapingOutcome::Append,
            index: None,
        },
    }
}

/// The `bind_official` scans kept intact: result id, then receipt, then the
/// canonical binding with its correction rule.
pub(super) fn binding_native(rows: &[BindingRow], incoming: &Incoming) -> BindingDecision {
    if let Some(index) = rows.iter().position(|row| row.result_id_equal) {
        return BindingDecision {
            outcome: if rows[index].entry_equal {
                BindingOutcome::Duplicate
            } else {
                BindingOutcome::ConflictId
            },
            index: Some(index),
        };
    }
    if let Some(index) = rows.iter().position(|row| row.provenance_equal) {
        return BindingDecision {
            outcome: if rows[index].same_event {
                BindingOutcome::Duplicate
            } else {
                BindingOutcome::ConflictReceipt
            },
            index: Some(index),
        };
    }
    let Some(index) = rows.iter().rposition(|row| row.episode_equal) else {
        return BindingDecision {
            outcome: if incoming.has_correction {
                BindingOutcome::RejectFirstCorrection
            } else {
                BindingOutcome::Append
            },
            index: None,
        };
    };
    let current = &rows[index];
    let advances =
        (incoming.revision_hi, incoming.revision_lo) > (current.revision_hi, current.revision_lo);
    if current.correction_targets && current.identity_equal && advances {
        BindingDecision {
            outcome: BindingOutcome::Append,
            index: None,
        }
    } else {
        BindingDecision {
            outcome: BindingOutcome::RejectCorrection,
            index: Some(index),
        }
    }
}
