use super::runtime_authorization::{
    ParityProjectionV1, ServiceAuthorizationV1, reducer_registry_sha256,
};
use super::runtime_evidence::{
    BoardOutcomeV1, DeepCutEvidenceV1, ServiceResultV1, SubmissionEvidenceV1,
};
use super::runtime_schema::{
    DispatchBindingV1, LeafSelectorV1, ReducerContractV1, ServiceOperationTagV1,
    ServiceOperationV1, ServiceSlotV1,
};
use super::runtime_schema_tests::{authorization, digest, operation, registry};
use super::schema::LaneIdV1;

fn result(operation: &ServiceOperationV1) -> ServiceResultV1 {
    use ServiceOperationTagV1 as O;
    let service_intent_id = operation.service_intent_id().into();
    match operation.tag() {
        tag @ (O::BoardObserve | O::BoardRefresh | O::BoardRemediate) => ServiceResultV1::Board {
            service_intent_id,
            operation_tag: tag,
            raw_sha256: digest("board-raw"),
            outcome: BoardOutcomeV1::Observed {
                outcome_sha256: digest("board-outcome"),
            },
        },
        tag @ (O::CandidateReplay | O::CandidatePort | O::CandidateExplore) => {
            ServiceResultV1::Candidate {
                service_intent_id,
                operation_tag: tag,
                batch_sha256: digest("candidate-batch"),
            }
        }
        tag @ (O::DeepCutRegisterWorker
        | O::DeepCutRegisterEpisode
        | O::DeepCutTerminal
        | O::DeepCutRollover) => ServiceResultV1::DeepCut {
            service_intent_id,
            evidence: match tag {
                O::DeepCutRegisterWorker => DeepCutEvidenceV1::RegisterWorker {
                    worker_sha256: digest("worker"),
                },
                O::DeepCutRegisterEpisode => DeepCutEvidenceV1::RegisterEpisode {
                    episode_sha256: digest("episode"),
                },
                O::DeepCutTerminal => DeepCutEvidenceV1::Terminal {
                    terminal_sha256: digest("terminal"),
                },
                O::DeepCutRollover => DeepCutEvidenceV1::Rollover {
                    rollover_sha256: digest("rollover"),
                },
                _ => unreachable!(),
            },
        },
        O::Context => ServiceResultV1::Context {
            service_intent_id,
            events_sha256: digest("context-events"),
            anchors_sha256: digest("context-anchors"),
        },
        tag @ (O::SubmissionPlan | O::SubmissionEffect | O::SubmissionReconcile) => {
            ServiceResultV1::Submission {
                service_intent_id,
                evidence: match tag {
                    O::SubmissionPlan => SubmissionEvidenceV1::Plan {
                        plan_sha256: digest("plan"),
                    },
                    O::SubmissionEffect => SubmissionEvidenceV1::Effect {
                        effect_sha256: digest("effect"),
                    },
                    O::SubmissionReconcile => SubmissionEvidenceV1::Reconcile {
                        reconcile_sha256: digest("reconcile"),
                    },
                    _ => unreachable!(),
                },
            }
        }
        O::OfficialResult => ServiceResultV1::Official {
            service_intent_id,
            result_sha256: digest("official-result"),
            reward_input_sha256: digest("official-reward-input"),
        },
        tag @ (O::WorkerInspect | O::WorkerNudge | O::WorkerReplace | O::WorkerCheckpoint) => {
            ServiceResultV1::Worker {
                service_intent_id,
                operation_tag: tag,
                control_sha256: digest("worker-control"),
                lease_receipt_sha256: digest("lease-receipt"),
            }
        }
    }
}

#[test]
fn closed_evidence_covers_every_operation_and_rejects_malformed_operation() {
    for tag in ServiceOperationTagV1::ALL {
        let operation = operation(tag);
        result(&operation).validate_for(&operation).unwrap();
    }
    let mut malformed = operation(ServiceOperationTagV1::CandidateExplore);
    let ServiceOperationV1::Candidate { operation_tag, .. } = &mut malformed else {
        unreachable!()
    };
    *operation_tag = ServiceOperationTagV1::BoardObserve;
    let board = ServiceResultV1::Board {
        service_intent_id: malformed.service_intent_id().into(),
        operation_tag: ServiceOperationTagV1::BoardObserve,
        raw_sha256: digest("board-raw"),
        outcome: BoardOutcomeV1::Failed {
            failure_sha256: digest("board-failure"),
        },
    };
    assert!(board.validate_for(&malformed).is_err());
}

#[test]
fn rt_auth_001_unrelated_higher_revision_rejected() {
    let registry = registry();
    let operation = operation(ServiceOperationTagV1::CandidateExplore);
    let first = authorization(&registry, &operation, 7, "runtime-7");
    let advanced = authorization(&registry, &operation, 8, "runtime-8");
    assert_ne!(first.authorization_id, advanced.authorization_id);
    let mut stale = first;
    stale.preimage.pre_runtime_revision = 8;
    stale.preimage.pre_runtime_sha256 = digest("runtime-8");
    assert!(stale.validate(&registry, &operation).is_err());
}

fn reject_dispatch_mutation(
    original: &ServiceAuthorizationV1,
    registry: &[ReducerContractV1],
    operation: &ServiceOperationV1,
    mutate: impl FnOnce(&mut DispatchBindingV1),
) {
    let mut changed = original.clone();
    mutate(&mut changed.preimage.dispatch);
    assert!(changed.validate(registry, operation).is_err());
}

#[test]
fn rt_auth_003_cross_slot_lane_generation_checkpoint_rejected() {
    let registry = registry();
    let operation = operation(ServiceOperationTagV1::CandidateExplore);
    let auth = authorization(&registry, &operation, 1, "runtime");
    reject_dispatch_mutation(&auth, &registry, &operation, |binding| {
        let DispatchBindingV1::Lane { slot, .. } = binding else {
            unreachable!()
        };
        *slot = ServiceSlotV1::DeepCutLane;
    });
    reject_dispatch_mutation(&auth, &registry, &operation, |binding| {
        let DispatchBindingV1::Lane { lane, .. } = binding else {
            unreachable!()
        };
        *lane = LaneIdV1::DeepCut;
    });
    reject_dispatch_mutation(&auth, &registry, &operation, |binding| {
        let DispatchBindingV1::Lane { generation, .. } = binding else {
            unreachable!()
        };
        *generation += 1;
    });
    reject_dispatch_mutation(&auth, &registry, &operation, |binding| {
        let DispatchBindingV1::Lane {
            checkpoint_revision,
            ..
        } = binding
        else {
            unreachable!()
        };
        *checkpoint_revision += 1;
    });
}

#[test]
fn rt_auth_005_persisted_scope_cannot_authorize_itself() {
    let registry = registry();
    let operation = operation(ServiceOperationTagV1::CandidateExplore);
    let auth = authorization(&registry, &operation, 1, "runtime");
    let mut rewritten = registry.clone();
    let index = rewritten
        .iter()
        .position(|entry| entry.operation_tag == operation.tag())
        .unwrap();
    let mut writes = rewritten[index].write_selectors.clone();
    writes.insert(LeafSelectorV1::RewardBinding);
    rewritten[index] = ReducerContractV1::new(
        operation.tag(),
        rewritten[index].reducer_version_sha256.clone(),
        rewritten[index].read_components.clone(),
        writes,
        rewritten[index].rebase_rule,
    )
    .unwrap();
    assert!(auth.validate(&rewritten, &operation).is_err());
    let mut tampered = registry;
    tampered[index]
        .write_selectors
        .insert(LeafSelectorV1::RewardBinding);
    assert!(reducer_registry_sha256(&tampered).is_err());
}

#[test]
fn rt_auth_007_operation_swap_changes_authorization_id() {
    let registry = registry();
    let explore = operation(ServiceOperationTagV1::CandidateExplore);
    let port = operation(ServiceOperationTagV1::CandidatePort);
    let first = authorization(&registry, &explore, 1, "runtime");
    let second = authorization(&registry, &port, 1, "runtime");
    assert_ne!(first.authorization_id, second.authorization_id);
    assert!(first.validate(&registry, &port).is_err());
}

#[test]
fn registry_identity_is_unique_and_order_independent() {
    let registry = registry();
    let mut reversed = registry.clone();
    reversed.reverse();
    assert_eq!(
        reducer_registry_sha256(&registry).unwrap(),
        reducer_registry_sha256(&reversed).unwrap()
    );
    let mut duplicate = registry;
    duplicate.push(duplicate[0].clone());
    assert!(reducer_registry_sha256(&duplicate).is_err());
}

#[test]
fn parity_projection_is_identity_only_and_closed() {
    let json = r#""identity_v1""#;
    assert_eq!(
        serde_json::from_str::<ParityProjectionV1>(json).unwrap(),
        ParityProjectionV1::IdentityV1
    );
    assert!(serde_json::from_str::<ParityProjectionV1>(r#""semantic""#).is_err());
    assert!(
        serde_json::from_str::<ParityProjectionV1>(
            r#"{"projection":"identity_v1","drop":"hardware"}"#
        )
        .is_err()
    );
}
