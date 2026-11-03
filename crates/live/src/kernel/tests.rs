use super::*;
use serde_json::json;

struct Counter {
    step: i64,
    schema: u32,
    reject: bool,
    effect: bool,
}
impl Implementation for Counter {
    fn prepare(&self, previous: Option<&Checkpoint>) -> Result<PreparedState, String> {
        if self.reject {
            return Err("migration refused".into());
        }
        Ok(PreparedState {
            checkpoint: Checkpoint {
                schema: self.schema,
                state: previous.map(|p| p.state.clone()).unwrap_or(json!(0)),
            },
            resources: BTreeMap::new(),
            surfaces: BTreeMap::new(),
        })
    }
    fn reduce(&self, checkpoint: &Checkpoint, event: &Value) -> Result<Transition, String> {
        if event["fail"] == true {
            return Err("event failed".into());
        }
        Ok(Transition {
            checkpoint: Checkpoint {
                schema: self.schema,
                state: json!(checkpoint.state.as_i64().unwrap() + self.step),
            },
            output: checkpoint.state.clone(),
            resources: BTreeMap::new(),
            surfaces: BTreeMap::new(),
            effects: if self.effect && event["type"] != "completion" {
                vec![Effect {
                    service: "provider".into(),
                    method: "compute".into(),
                    arguments: json!(8),
                }]
            } else {
                vec![]
            },
        })
    }
}
fn candidate(id: &str, step: i64, schema: u32) -> Candidate {
    Candidate {
        manifest: Manifest {
            id: id.into(),
            state_schema: schema,
            exports: vec![Export {
                kind: Kind::Command,
                name: id.into(),
                contract: "counter/1".into(),
            }],
            imports: vec![],
        },
        identity: format!("{id}-{step}-{schema}"),
        implementation: Arc::new(Counter {
            step,
            schema,
            reject: false,
            effect: false,
        }),
    }
}
fn install(kernel: &mut Kernel, candidates: Vec<Candidate>) {
    let staged = kernel.stage(candidates).unwrap();
    kernel.commit(staged).unwrap();
}

#[test]
fn replacement_changes_executable_behavior_and_migrates_live_state() {
    let mut kernel = Kernel::default();
    install(&mut kernel, vec![candidate("counter", 1, 1)]);
    kernel
        .dispatch(Kind::Command, "counter", &json!({}))
        .unwrap();
    install(&mut kernel, vec![candidate("counter", 7, 2)]);
    kernel
        .dispatch(Kind::Command, "counter", &json!({}))
        .unwrap();
    assert_eq!(
        kernel.checkpoint("counter"),
        Some(Checkpoint {
            schema: 2,
            state: json!(8)
        })
    );
    assert_eq!(kernel.generation("counter"), Some(2));
}
#[test]
fn stale_preparation_cannot_overwrite_an_event_or_other_commit() {
    let mut kernel = Kernel::default();
    install(&mut kernel, vec![candidate("counter", 1, 1)]);
    let staged = kernel.stage(vec![candidate("counter", 2, 1)]).unwrap();
    kernel
        .dispatch(Kind::Command, "counter", &json!({}))
        .unwrap();
    assert!(kernel.commit(staged).unwrap_err().contains("stale"));
    let staged = kernel.stage(vec![candidate("counter", 3, 1)]).unwrap();
    install(&mut kernel, vec![candidate("other", 1, 1)]);
    assert!(kernel.commit(staged).is_err());
    assert_eq!(kernel.checkpoint("counter").unwrap().state, json!(1));
}
#[test]
fn group_migration_and_dependency_contracts_are_atomic() {
    let mut kernel = Kernel::default();
    let mut dependent = candidate("dependent", 1, 1);
    dependent.manifest.imports.push(Import {
        module: "counter".into(),
        contract: "counter/1".into(),
    });
    install(
        &mut kernel,
        vec![candidate("counter", 1, 1), dependent.clone()],
    );
    let mut replacement = candidate("counter", 2, 2);
    replacement.manifest.exports[0].contract = "counter/2".into();
    assert!(kernel.stage(vec![replacement.clone()]).is_err());
    dependent.manifest.imports[0].contract = "counter/2".into();
    dependent.implementation = Arc::new(Counter {
        step: 1,
        schema: 1,
        reject: true,
        effect: false,
    });
    assert!(
        kernel
            .stage(vec![replacement.clone(), dependent.clone()])
            .is_err()
    );
    assert_eq!(kernel.generation("counter"), Some(1));
    dependent.implementation = Arc::new(Counter {
        step: 1,
        schema: 1,
        reject: false,
        effect: false,
    });
    install(&mut kernel, vec![replacement, dependent]);
    assert_eq!(kernel.generation("counter"), Some(2));
    assert_eq!(kernel.generation("dependent"), Some(2));
}
#[test]
fn issued_services_pin_both_generations_finish_once_and_reject_late_results() {
    let mut kernel = Kernel::default();
    let mut caller = candidate("caller", 1, 1);
    let mut provider = candidate("provider", 10, 1);
    caller.manifest.imports.push(Import {
        module: "provider".into(),
        contract: "compute/1".into(),
    });
    caller.implementation = Arc::new(Counter {
        step: 1,
        schema: 1,
        reject: false,
        effect: true,
    });
    provider.manifest.exports = vec![Export {
        kind: Kind::Service,
        name: "compute".into(),
        contract: "compute/1".into(),
    }];
    install(&mut kernel, vec![caller.clone(), provider.clone()]);
    let response = kernel
        .dispatch(Kind::Command, "caller", &json!({}))
        .unwrap();
    let issued = &response.effects[0];
    assert!(
        kernel
            .stage(vec![caller.clone()])
            .err()
            .unwrap()
            .contains("deferred")
    );
    assert!(
        kernel
            .stage(vec![provider.clone()])
            .err()
            .unwrap()
            .contains("deferred")
    );
    let mut forged = issued.clone();
    forged.job.generation += 1;
    assert!(kernel.invoke(&forged).is_err());
    kernel.invoke(issued).unwrap();
    assert!(kernel.invoke(issued).is_err());
    kernel.complete(&issued.job, json!({"ok":true})).unwrap();
    assert!(kernel.complete(&issued.job, json!({"ok":true})).is_err());
    install(&mut kernel, vec![caller, provider]);
    assert_eq!(kernel.checkpoint("provider").unwrap().state, json!(10));
    assert_eq!(kernel.checkpoint("caller").unwrap().state, json!(2));
}
#[test]
fn failed_event_preserves_state_and_unknown_service_cannot_commit() {
    let mut kernel = Kernel::default();
    install(&mut kernel, vec![candidate("counter", 1, 1)]);
    assert!(
        kernel
            .dispatch(Kind::Command, "counter", &json!({"fail":true}))
            .is_err()
    );
    assert_eq!(kernel.checkpoint("counter").unwrap().state, json!(0));
    let mut malicious = candidate("counter", 1, 1);
    malicious.implementation = Arc::new(Counter {
        step: 1,
        schema: 1,
        reject: false,
        effect: true,
    });
    install(&mut kernel, vec![malicious]);
    assert!(
        kernel
            .dispatch(Kind::Command, "counter", &json!({}))
            .is_err()
    );
    assert_eq!(kernel.checkpoint("counter").unwrap().state, json!(0));
}
#[test]
fn duplicate_routes_and_dependency_cycles_cannot_replace_working_code() {
    let mut kernel = Kernel::default();
    let a = candidate("a", 1, 1);
    install(&mut kernel, vec![a.clone()]);
    let mut b = candidate("b", 1, 1);
    b.manifest.exports[0].name = "a".into();
    assert!(kernel.stage(vec![b]).is_err());
    let mut a = a;
    let mut b = candidate("b", 1, 1);
    a.manifest.imports.push(Import {
        module: "b".into(),
        contract: "counter/1".into(),
    });
    b.manifest.imports.push(Import {
        module: "a".into(),
        contract: "counter/1".into(),
    });
    assert!(kernel.stage(vec![a, b]).err().unwrap().contains("cycle"));
    assert_eq!(kernel.generation("a"), Some(1));
}

#[derive(Clone)]
struct Pack {
    version: i64,
    dependencies: Resources,
    consumer: bool,
    waiting: bool,
}
impl Implementation for Pack {
    fn with_resources(
        &self,
        dependencies: &Resources,
    ) -> Result<Option<Arc<dyn Implementation>>, String> {
        Ok(Some(Arc::new(Self {
            dependencies: dependencies.clone(),
            ..self.clone()
        })))
    }
    fn prepare(&self, previous: Option<&Checkpoint>) -> Result<PreparedState, String> {
        self.rebind(&previous.cloned().unwrap_or(Checkpoint {
            schema: 1,
            state: json!(0),
        }))
    }
    fn rebind(&self, checkpoint: &Checkpoint) -> Result<PreparedState, String> {
        let value = if self.consumer {
            let left = self.dependencies.get("left", "pack", "pack/1")?;
            let right = self.dependencies.get("right", "pack", "pack/1")?;
            if left.value["version"] == 99 {
                return Err("selected pack cannot be resolved".into());
            }
            json!({"left":left.value.as_ref(),"right":right.value.as_ref()})
        } else {
            json!({"version":checkpoint.state.as_i64().unwrap()+self.version})
        };
        Ok(PreparedState {
            checkpoint: checkpoint.clone(),
            surfaces: BTreeMap::new(),
            resources: BTreeMap::from([("pack".into(), value)]),
        })
    }
    fn bind_resource(&self, _: &str, _: &Value) -> Result<Binding, String> {
        Ok(Arc::new(self.dependencies.clone()))
    }
    fn reduce(&self, checkpoint: &Checkpoint, event: &Value) -> Result<Transition, String> {
        let mut checkpoint = checkpoint.clone();
        if let Some(delta) = event["delta"].as_i64() {
            checkpoint.state = json!(checkpoint.state.as_i64().unwrap() + delta);
        }
        let prepared = self.rebind(&checkpoint)?;
        Ok(Transition {
            checkpoint: prepared.checkpoint,
            output: Value::Null,
            surfaces: prepared.surfaces,
            resources: prepared.resources,
            effects: if self.waiting && event["wait"] == true {
                vec![Effect {
                    service: "left".into(),
                    method: "compute".into(),
                    arguments: Value::Null,
                }]
            } else {
                vec![]
            },
        })
    }
    fn instantiate(&self, _: &Value) -> Result<Checkpoint, String> {
        Ok(Checkpoint {
            schema: 1,
            state: Value::Null,
        })
    }
    fn reduce_instance(&self, checkpoint: &Checkpoint, _: &Value) -> Result<Transition, String> {
        let left = self.dependencies.get("left", "pack", "pack/1")?;
        Ok(Transition {
            checkpoint: checkpoint.clone(),
            output: left.value.clone().as_ref().clone(),
            surfaces: BTreeMap::new(),
            resources: BTreeMap::new(),
            effects: vec![],
        })
    }
}
fn pack(id: &str, version: i64, consumer: bool) -> Candidate {
    Candidate {
        manifest: Manifest {
            id: id.into(),
            state_schema: 1,
            exports: vec![
                Export {
                    kind: Kind::Resource,
                    name: "pack".into(),
                    contract: "pack/1".into(),
                },
                Export {
                    kind: Kind::Command,
                    name: id.into(),
                    contract: "command/1".into(),
                },
            ],
            imports: if consumer {
                vec![
                    Import {
                        module: "left".into(),
                        contract: "pack/1".into(),
                    },
                    Import {
                        module: "right".into(),
                        contract: "pack/1".into(),
                    },
                ]
            } else {
                vec![]
            },
        },
        identity: format!("{id}-{version}"),
        implementation: Arc::new(Pack {
            version,
            dependencies: Default::default(),
            consumer,
            waiting: false,
        }),
    }
}
fn packs() -> Kernel {
    let mut kernel = Kernel::default();
    // Consumer sorts before providers and is deliberately supplied first.
    install(
        &mut kernel,
        vec![
            pack("loadout", 0, true),
            pack("right", 2, false),
            pack("left", 1, false),
        ],
    );
    kernel
}
#[test]
fn resource_names_are_scoped_and_bindings_see_only_the_declared_prepared_providers() {
    let kernel = packs();
    let resources = kernel.resources();
    assert_eq!(resources.len(), 3);
    assert_eq!(resources["loadout/pack"].value["left"]["version"], 1);
    let imports = resources["loadout/pack"].binding::<Resources>().unwrap();
    assert_eq!(
        imports.get("right", "pack", "pack/1").unwrap().value["version"],
        2
    );
    assert!(imports.get("right", "pack", "pack/2").is_err());
    assert!(imports.get("loadout", "pack", "pack/1").is_err());
    assert!(resources["left/pack"].imports().is_empty());
}
#[test]
fn provider_code_and_state_changes_rebind_consumers_atomically_without_resetting_state() {
    let mut kernel = packs();
    kernel
        .dispatch(Kind::Command, "loadout", &json!({"delta":7}))
        .unwrap();
    install(&mut kernel, vec![pack("left", 3, false)]);
    assert_eq!(kernel.generation("loadout"), Some(2));
    assert_eq!(kernel.checkpoint("loadout").unwrap().state, 7);
    assert_eq!(
        kernel.resources()["loadout/pack"].value["left"]["version"],
        3
    );
    kernel
        .dispatch(Kind::Command, "left", &json!({"delta":4}))
        .unwrap();
    assert_eq!(kernel.generation("left"), Some(2));
    assert_eq!(kernel.generation("loadout"), Some(3));
    assert_eq!(
        kernel.resources()["loadout/pack"].value["left"]["version"],
        7
    );
    assert_eq!(kernel.checkpoint("loadout").unwrap().state, 7);
    // A read-only command must not manufacture another closure generation.
    kernel.dispatch(Kind::Command, "left", &json!({})).unwrap();
    assert_eq!(kernel.generation("loadout"), Some(3));
}
#[test]
fn failed_dependency_rebind_rolls_back_provider_state_and_code_with_the_entire_closure() {
    let mut kernel = packs();
    let before = kernel.resources();
    assert!(kernel.stage(vec![pack("left", 99, false)]).is_err());
    assert_eq!(kernel.resources(), before);
    assert!(
        kernel
            .dispatch(Kind::Command, "left", &json!({"delta":98}))
            .is_err()
    );
    assert_eq!(kernel.checkpoint("left").unwrap().state, 0);
    assert_eq!(kernel.resources(), before);
}
#[test]
fn a_prepared_consumer_fences_provider_events_and_instances_keep_their_original_imports() {
    let mut kernel = packs();
    let old = kernel.resources()["loadout/pack"]
        .instantiate(&Value::Null)
        .unwrap();
    let staged = kernel.stage(vec![pack("loadout", 0, true)]).unwrap();
    kernel.dispatch(Kind::Command, "left", &json!({})).unwrap();
    assert_eq!(kernel.generation("loadout"), Some(1));
    assert!(kernel.commit(staged).unwrap_err().contains("stale"));
    kernel
        .dispatch(Kind::Command, "left", &json!({"delta":5}))
        .unwrap();
    assert_eq!(old.stage(&Value::Null).unwrap().output["version"], 1);
    let next = kernel.resources()["loadout/pack"]
        .instantiate(&Value::Null)
        .unwrap();
    assert_eq!(next.stage(&Value::Null).unwrap().output["version"], 6);
}

#[test]
fn resource_rebinding_defers_during_an_issued_job_without_losing_state_or_replaying_the_service() {
    let mut kernel = Kernel::default();
    let mut left = pack("left", 1, false);
    let mut caller = pack("loadout", 0, true);
    left.manifest.exports.push(Export {
        kind: Kind::Service,
        name: "compute".into(),
        contract: "compute/1".into(),
    });
    caller.manifest.imports.push(Import {
        module: "left".into(),
        contract: "compute/1".into(),
    });
    caller.implementation = Arc::new(Pack {
        version: 0,
        consumer: true,
        waiting: true,
        dependencies: Default::default(),
    });
    install(&mut kernel, vec![caller, left, pack("right", 2, false)]);
    let issued = kernel
        .dispatch(Kind::Command, "loadout", &json!({"wait":true}))
        .unwrap()
        .effects
        .pop()
        .unwrap();
    let resources = kernel.resources();
    assert!(
        kernel
            .dispatch(Kind::Command, "left", &json!({"delta":1}))
            .unwrap_err()
            .contains("deferred")
    );
    assert_eq!(kernel.resources(), resources);
    assert_eq!(kernel.checkpoint("left").unwrap().state, 0);
    kernel.invoke(&issued).unwrap();
    assert!(kernel.invoke(&issued).is_err());
    kernel.complete(&issued.job, Value::Null).unwrap();
    kernel
        .dispatch(Kind::Command, "left", &json!({"delta":1}))
        .unwrap();
    assert_eq!(
        kernel.resources()["loadout/pack"].value["left"]["version"],
        2
    );
    assert!(kernel.complete(&issued.job, Value::Null).is_err());
}

#[test]
fn imported_bindings_survive_replacement_until_instances_drop_even_when_the_backend_does_not_capture_imports()
 {
    struct Opaque {
        token: Option<Arc<()>>,
    }
    impl Implementation for Opaque {
        fn prepare(&self, _: Option<&Checkpoint>) -> Result<PreparedState, String> {
            self.rebind(&Checkpoint {
                schema: 1,
                state: Value::Null,
            })
        }
        fn rebind(&self, state: &Checkpoint) -> Result<PreparedState, String> {
            Ok(PreparedState {
                checkpoint: state.clone(),
                surfaces: BTreeMap::new(),
                resources: BTreeMap::from([("pack".into(), Value::Null)]),
            })
        }
        fn reduce(&self, _: &Checkpoint, _: &Value) -> Result<Transition, String> {
            Err("not used".into())
        }
        fn bind_resource(&self, _: &str, _: &Value) -> Result<Binding, String> {
            Ok(self.token.clone().unwrap_or_else(|| Arc::new(())))
        }
    }
    let mut kernel = Kernel::default();
    let token = Arc::new(());
    let weak = Arc::downgrade(&token);
    let mut provider = pack("left", 1, false);
    provider.implementation = Arc::new(Opaque { token: Some(token) });
    let mut consumer = pack("loadout", 0, false);
    consumer.manifest.imports = vec![Import {
        module: "left".into(),
        contract: "pack/1".into(),
    }];
    consumer.implementation = Arc::new(Opaque { token: None });
    install(&mut kernel, vec![consumer, provider]);
    let instance = kernel.resources()["loadout/pack"]
        .instantiate(&Value::Null)
        .unwrap();
    let mut provider = pack("left", 2, false);
    provider.implementation = Arc::new(Opaque {
        token: Some(Arc::new(())),
    });
    install(&mut kernel, vec![provider]);
    assert!(
        weak.upgrade().is_some(),
        "backend-independent instance lost its imported binding lease"
    );
    drop(instance);
    assert!(
        weak.upgrade().is_none(),
        "retired closure retained an imported binding"
    );
}


#[test]
fn dropped_instance_commit_guard_leaves_checkpoint_and_revision_unchanged() {
    let mut instance = candidate("guard-abort", 2, 1)
        .probe_instance(&Value::Null)
        .unwrap();
    let before = instance.checkpoint().clone();
    let revision = instance.revision;
    let transition = instance.stage(&Value::Null).unwrap();
    let still_valid = instance.stage(&Value::Null).unwrap();
    drop(instance.prepare_commit(transition).unwrap());
    assert_eq!(instance.checkpoint(), &before);
    assert_eq!(instance.revision, revision);
    assert_eq!(instance.commit(still_valid).unwrap(), json!(0));
    assert_eq!(instance.checkpoint().state, json!(2));
    assert_eq!(instance.revision, revision + 1);
}

#[test]
fn aborted_guard_retains_the_exact_computation_for_external_ack_retry() {
    let mut instance = candidate("guard-retain", 7, 1)
        .probe_instance(&Value::Null).unwrap();
    let before = instance.checkpoint().clone();
    let transition = instance.stage(&Value::Null).unwrap();
    let output = transition.output.clone();
    let retained = instance.prepare_commit(transition).unwrap().abort();
    assert_eq!(instance.checkpoint(), &before);
    assert_eq!(instance.revision, 0);
    assert_eq!(retained.output, output);
    assert_eq!(instance.prepare_commit(retained).unwrap().commit(), output);
    assert_eq!(instance.checkpoint().state, json!(7));
    assert_eq!(instance.revision, 1);
}

#[test]
fn recoverable_preparation_preserves_foreign_computation_and_both_owners() {
    let owner = candidate("guard-recover", 8, 1);
    let mut source = owner.probe_instance(&Value::Null).unwrap();
    let mut target = owner.probe_instance(&Value::Null).unwrap();
    let before_source = source.checkpoint().clone();
    let before_target = target.checkpoint().clone();
    let transition = source.stage(&Value::Null).unwrap();
    let (error, retained) = target.prepare_commit_recoverable(transition).err().unwrap();
    assert_eq!(error, "stale or foreign instance transition");
    assert_eq!(source.checkpoint(), &before_source);
    assert_eq!(target.checkpoint(), &before_target);
    let prepared = source.prepare_commit_recoverable(retained)
        .unwrap_or_else(|(error, _)| panic!("original owner lost its preparation: {error}"));
    assert_eq!(prepared.commit(), json!(0));
    assert_eq!(source.checkpoint().state, json!(8));
    assert_eq!(target.checkpoint(), &before_target);
}

#[test]
fn prepared_instance_commit_applies_once_and_rejects_preexisting_stale_transition() {
    let mut instance = candidate("guard-once", 3, 1)
        .probe_instance(&Value::Null)
        .unwrap();
    let transition = instance.stage(&Value::Null).unwrap();
    let stale = instance.stage(&Value::Null).unwrap();
    let prepared = instance.prepare_commit(transition).unwrap();
    assert_eq!(prepared.commit(), json!(0));
    assert_eq!(instance.checkpoint().state, json!(3));
    assert_eq!(instance.revision, 1);
    let before = instance.checkpoint().clone();
    assert_eq!(
        instance.prepare_commit(stale).err().unwrap(),
        "stale or foreign instance transition"
    );
    assert_eq!(instance.checkpoint(), &before);
    assert_eq!(instance.revision, 1);
}

#[test]
fn instance_prepare_commit_rejects_foreign_transition_without_changing_either_instance() {
    let owner = candidate("guard-owner", 4, 1);
    let source = owner.probe_instance(&Value::Null).unwrap();
    let mut target = owner.probe_instance(&Value::Null).unwrap();
    let source_before = source.checkpoint().clone();
    let target_before = target.checkpoint().clone();
    let foreign = source.stage(&Value::Null).unwrap();
    assert_eq!(
        target.prepare_commit(foreign).err().unwrap(),
        "stale or foreign instance transition"
    );
    assert_eq!(source.checkpoint(), &source_before);
    assert_eq!(target.checkpoint(), &target_before);
    assert_eq!(source.revision, 0);
    assert_eq!(target.revision, 0);
}

#[test]
fn instance_commit_revision_exhaustion_is_rejected_before_preparation_or_legacy_commit() {
    let mut instance = candidate("guard-overflow", 5, 1)
        .probe_instance(&Value::Null)
        .unwrap();
    instance.revision = u64::MAX;
    let before = instance.checkpoint().clone();
    let prepared_transition = instance.stage(&Value::Null).unwrap();
    let legacy_transition = instance.stage(&Value::Null).unwrap();
    assert_eq!(
        instance.prepare_commit(prepared_transition).err().unwrap(),
        "instance revision exhausted"
    );
    assert_eq!(instance.checkpoint(), &before);
    assert_eq!(instance.revision, u64::MAX);
    assert_eq!(
        instance.commit(legacy_transition).unwrap_err(),
        "instance revision exhausted"
    );
    assert_eq!(instance.checkpoint(), &before);
    assert_eq!(instance.revision, u64::MAX);
}

#[test]
fn legacy_instance_commit_matches_prepared_commit_outputs_checkpoints_and_revisions() {
    let owner = candidate("guard-parity", 6, 1);
    let mut legacy = owner.probe_instance(&Value::Null).unwrap();
    let mut prepared = owner.probe_instance(&Value::Null).unwrap();
    for _ in 0..3 {
        let legacy_transition = legacy.stage(&Value::Null).unwrap();
        let stale_legacy = legacy.stage(&Value::Null).unwrap();
        let prepared_transition = prepared.stage(&Value::Null).unwrap();
        assert_eq!(
            legacy.commit(legacy_transition).unwrap(),
            prepared
                .prepare_commit(prepared_transition)
                .unwrap()
                .commit()
        );
        assert_eq!(legacy.checkpoint(), prepared.checkpoint());
        assert_eq!(legacy.revision, prepared.revision);
        assert_eq!(
            legacy.commit(stale_legacy).unwrap_err(),
            "stale or foreign instance transition"
        );
        assert_eq!(legacy.checkpoint(), prepared.checkpoint());
        assert_eq!(legacy.revision, prepared.revision);
    }
    let foreign = legacy.stage(&Value::Null).unwrap();
    assert_eq!(
        prepared.commit(foreign).unwrap_err(),
        "stale or foreign instance transition"
    );
    assert_eq!(legacy.checkpoint(), prepared.checkpoint());
    assert_eq!(legacy.revision, prepared.revision);
}
