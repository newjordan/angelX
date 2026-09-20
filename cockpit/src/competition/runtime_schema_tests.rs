use super::journal::canonical_action_key;
use super::profile::{DEEP_CUT_PROFILE_VERSION_V1, DeepCutProfileV1};
use super::runtime_authorization::ServiceAuthorizationV1;
use super::runtime_evidence::ServiceResultV1;
use super::runtime_schema::*;
use super::schema::{ActionIntentV1, CompetitionKeyV1, LaneIdV1, ObjectiveComparatorV1, ScoreV1};
use std::collections::BTreeSet;

#[test]
fn result_tag_wire_names_remain_v1_compatible() {
    for (tag, wire) in [
        (ServiceResultTagV1::Board, "board_evidence"),
        (ServiceResultTagV1::Candidate, "candidate_evidence"),
        (ServiceResultTagV1::DeepCut, "deep_cut_evidence"),
        (ServiceResultTagV1::Context, "context_evidence"),
        (ServiceResultTagV1::Submission, "submission_evidence"),
        (ServiceResultTagV1::Official, "official_evidence"),
        (ServiceResultTagV1::Worker, "worker_evidence"),
    ] {
        let value = serde_json::Value::String(wire.into());
        assert_eq!(serde_json::to_value(tag).unwrap(), value);
        assert_eq!(
            serde_json::from_value::<ServiceResultTagV1>(value).unwrap(),
            tag
        );
    }
}

pub(super) fn digest(label: &str) -> String {
    crate::cut::sha256_hex(label.as_bytes())
}

fn competition() -> CompetitionKeyV1 {
    CompetitionKeyV1 {
        platform_id: "fixture".into(),
        competition_id: "runtime-v1".into(),
        field_id: "kernels".into(),
        benchmark_id: "latency".into(),
        profile_id: DEEP_CUT_PROFILE_VERSION_V1.into(),
        hardware_id: "gpu-a".into(),
    }
}

fn official_join(parents: OfficialParentsV1) -> OfficialJoinV1 {
    OfficialJoinV1 {
        episode_id: "episode-1".into(),
        candidate_id: "candidate-1".into(),
        submission_id: "submission-1".into(),
        comparable_base_id: "base-1".into(),
        comparable_base_score: ScoreV1::new("10").unwrap(),
        competition: competition(),
        objective: ObjectiveComparatorV1 {
            objective_id: "speed".into(),
            version: "v1".into(),
            kind: super::schema::ComparatorKindV1::HigherIsBetter,
        },
        profile: DeepCutProfileV1::embedded().identity().unwrap(),
        hardware_id: "gpu-a".into(),
        adapter_receipt_id: "receipt-1".into(),
        adapter_event_id: "event-1".into(),
        parents,
    }
}

pub(super) fn operation(tag: ServiceOperationTagV1) -> ServiceOperationV1 {
    use ServiceOperationTagV1 as O;
    let service_intent_id = "service-intent-1".to_string();
    match tag {
        O::BoardObserve | O::BoardRefresh | O::BoardRemediate => ServiceOperationV1::Board {
            operation_tag: tag,
            service_intent_id,
            adapter_id: "fixture-adapter".into(),
            request_sha256: digest("board-request"),
        },
        O::CandidateReplay | O::CandidatePort | O::CandidateExplore => {
            ServiceOperationV1::Candidate {
                operation_tag: tag,
                service_intent_id,
                work_id: "candidate-work-1".into(),
                board_decision_sha256: digest("board-decision"),
                lineage_input_sha256: digest("lineage-input"),
            }
        }
        O::DeepCutRegisterWorker
        | O::DeepCutRegisterEpisode
        | O::DeepCutTerminal
        | O::DeepCutRollover => ServiceOperationV1::DeepCut {
            operation_tag: tag,
            service_intent_id,
            work_id: "deep-cut-work-1".into(),
            profile_sha256: digest("profile"),
            episode_input_sha256: digest("episode-input"),
        },
        O::Context => ServiceOperationV1::Context {
            service_intent_id,
            dossier_revision: 1,
            context_head_sha256: digest("context-head"),
        },
        O::SubmissionPlan | O::SubmissionEffect | O::SubmissionReconcile => {
            ServiceOperationV1::Submission {
                operation_tag: tag,
                service_intent_id,
                journal_head_sha256: digest("journal-head"),
                repository_sha256: digest("repository"),
                spool_sha256: digest("spool"),
            }
        }
        O::OfficialResult => ServiceOperationV1::OfficialResult {
            service_intent_id,
            join: Box::new(official_join(OfficialParentsV1::Initial { revision: 1 })),
        },
        O::WorkerInspect | O::WorkerNudge | O::WorkerReplace | O::WorkerCheckpoint => {
            ServiceOperationV1::WorkerControl {
                operation_tag: tag,
                service_intent_id,
                lease_key_sha256: digest("lease-key"),
                lease_id: "lease-1".into(),
                generation: 1,
                checkpoint_revision: 0,
            }
        }
    }
}

fn reducer(tag: ServiceOperationTagV1) -> ReducerContractV1 {
    let (slot, _, _, _) = tag.contract_tuple();
    let (read, write) = match slot {
        ServiceSlotV1::Observer => (StateComponentV1::Board, LeafSelectorV1::BoardObservation),
        ServiceSlotV1::FrontierLane => (StateComponentV1::Board, LeafSelectorV1::CandidateWorkItem),
        ServiceSlotV1::DeepCutLane => (
            StateComponentV1::Candidates,
            LeafSelectorV1::EpisodeWorkItem,
        ),
        ServiceSlotV1::Context => (StateComponentV1::Dossier, LeafSelectorV1::DossierAnchors),
        ServiceSlotV1::SubmissionPump => {
            (StateComponentV1::Submissions, LeafSelectorV1::ActionJournal)
        }
        ServiceSlotV1::WorkerControl => (StateComponentV1::Leases, LeafSelectorV1::SchedulerSlot),
    };
    ReducerContractV1::new(
        tag,
        digest(&format!("reducer-{tag:?}")),
        BTreeSet::from([read]),
        BTreeSet::from([write]),
        RebaseRuleV1::ExactReadSetUnchanged,
    )
    .unwrap()
}

pub(super) fn registry() -> Vec<ReducerContractV1> {
    ServiceOperationTagV1::ALL
        .into_iter()
        .map(reducer)
        .collect()
}

fn action(operation: &ServiceOperationV1) -> ActionIntentV1 {
    let mut value = ActionIntentV1 {
        action_key: String::new(),
        campaign_id: "campaign-1".into(),
        competition: competition(),
        kind: operation.tag().contract_tuple().3,
        subject_id: operation.service_intent_id().into(),
        payload_sha256: operation.canonical_sha256().unwrap(),
        intent_version: RUNTIME_ACTION_INTENT_V1.into(),
    };
    value.action_key = canonical_action_key(&value).unwrap();
    value
}

fn dispatch(tag: ServiceOperationTagV1) -> DispatchBindingV1 {
    let (slot, action, _, _) = tag.contract_tuple();
    match slot {
        ServiceSlotV1::FrontierLane | ServiceSlotV1::DeepCutLane => DispatchBindingV1::Lane {
            decision_sha256: digest("decision"),
            tick: 1,
            slot,
            action,
            lane: if slot == ServiceSlotV1::FrontierLane {
                LaneIdV1::FrontierGuard
            } else {
                LaneIdV1::DeepCut
            },
            lane_sequence: 1,
            assignment_sha256: digest("assignment"),
            lease_key_sha256: digest("lease-key"),
            lease_id: "lease-1".into(),
            generation: 1,
            checkpoint_revision: 0,
        },
        _ => DispatchBindingV1::Independent {
            decision_sha256: digest("decision"),
            tick: 1,
            slot,
            action,
            service_sequence: 1,
        },
    }
}

pub(super) fn authorization(
    registry: &[ReducerContractV1],
    operation: &ServiceOperationV1,
    revision: u64,
    runtime: &str,
) -> ServiceAuthorizationV1 {
    ServiceAuthorizationV1::new(
        registry,
        operation,
        dispatch(operation.tag()),
        action(operation),
        revision,
        digest(runtime),
        digest("director"),
        digest("scheduler"),
        digest("typed-input"),
    )
    .unwrap()
}

#[test]
fn rt_auth_004_closed_operation_slot_action_registry() {
    let registry = registry();
    assert_eq!(registry.len(), 19);
    for tag in ServiceOperationTagV1::ALL {
        operation(tag).validate().unwrap();
        let contract = registry
            .iter()
            .find(|entry| entry.operation_tag == tag)
            .unwrap();
        assert_eq!(
            (
                contract.legal_slot,
                contract.legal_action,
                contract.result_tag
            ),
            {
                let tuple = tag.contract_tuple();
                (tuple.0, tuple.1, tuple.2)
            }
        );
    }
    assert_eq!(
        serde_json::to_string(&ServiceActionV1::ServiceFrontierLane).unwrap(),
        "\"service-lane:frontier-guard\""
    );
    assert!(serde_json::from_str::<ServiceOperationTagV1>("\"invented\"").is_err());
}

#[test]
fn rt_auth_006_family_suboperation_result_confusion_rejected() {
    let operation = operation(ServiceOperationTagV1::CandidateExplore);
    let valid = ServiceResultV1::Candidate {
        service_intent_id: operation.service_intent_id().into(),
        operation_tag: operation.tag(),
        batch_sha256: digest("batch"),
    };
    assert!(valid.validate_for(&operation).is_ok());
    let mut wrong_suboperation = valid.clone();
    let ServiceResultV1::Candidate { operation_tag, .. } = &mut wrong_suboperation else {
        unreachable!()
    };
    *operation_tag = ServiceOperationTagV1::CandidatePort;
    assert!(wrong_suboperation.validate_for(&operation).is_err());
    let wrong_family = ServiceResultV1::Context {
        service_intent_id: operation.service_intent_id().into(),
        events_sha256: digest("events"),
        anchors_sha256: digest("anchors"),
    };
    assert!(wrong_family.validate_for(&operation).is_err());
}

#[test]
fn official_join_requires_exact_initial_or_correction_parents() {
    assert!(
        official_join(OfficialParentsV1::Initial { revision: 1 })
            .validate()
            .is_ok()
    );
    assert!(
        official_join(OfficialParentsV1::Initial { revision: 2 })
            .validate()
            .is_err()
    );
    let correction = OfficialParentsV1::Correction {
        next_revision: 4,
        current_revision: 3,
        current_result_id: "result-3".into(),
        current_binding_id: digest("binding-3"),
    };
    assert!(official_join(correction.clone()).validate().is_ok());
    let mut skipped = official_join(correction);
    let OfficialParentsV1::Correction { next_revision, .. } = &mut skipped.parents else {
        unreachable!()
    };
    *next_revision = 5;
    assert!(skipped.validate().is_err());
    let mut value =
        serde_json::to_value(official_join(OfficialParentsV1::Initial { revision: 1 })).unwrap();
    value.as_object_mut().unwrap().remove("parents");
    assert!(serde_json::from_value::<OfficialJoinV1>(value).is_err());
}
