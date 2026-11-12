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
        optional: false,
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
        optional: false,
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
        optional: false,
    });
    b.manifest.imports.push(Import {
        module: "a".into(),
        contract: "counter/1".into(),
        optional: false,
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
                        optional: false,
                    },
                    Import {
                        module: "right".into(),
                        contract: "pack/1".into(),
                        optional: false,
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
        optional: false,
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
        optional: false,
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
        .probe_instance(&Value::Null)
        .unwrap();
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
    let prepared = source
        .prepare_commit_recoverable(retained)
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

#[derive(Clone, Default)]
struct OptionalConsumer {
    dependencies: Resources,
}
impl Implementation for OptionalConsumer {
    fn with_resources(
        &self,
        dependencies: &Resources,
    ) -> Result<Option<Arc<dyn Implementation>>, String> {
        Ok(Some(Arc::new(Self {
            dependencies: dependencies.clone(),
        })))
    }
    fn prepare(&self, previous: Option<&Checkpoint>) -> Result<PreparedState, String> {
        self.rebind(&previous.cloned().unwrap_or(Checkpoint {
            schema: 1,
            state: json!({"selected":false}),
        }))
    }
    fn rebind(&self, checkpoint: &Checkpoint) -> Result<PreparedState, String> {
        let provider = self.dependencies.get("left", "pack", "pack/1").ok();
        if checkpoint.state["selected"] == true
            && provider.is_none_or(|resource| resource.value["version"] == 99)
        {
            return Err("selected optional clip unavailable".into());
        }
        Ok(PreparedState {
            checkpoint: checkpoint.clone(),
            surfaces: BTreeMap::new(),
            resources: BTreeMap::from([(
                "pack".into(),
                json!({"provider":provider.map(|resource| resource.value.as_ref()),"selected":checkpoint.state["selected"]}),
            )]),
        })
    }
    fn reduce(&self, checkpoint: &Checkpoint, event: &Value) -> Result<Transition, String> {
        let mut next = checkpoint.clone();
        next.state["selected"] = event["selected"].clone();
        let prepared = self.rebind(&next)?;
        Ok(Transition {
            checkpoint: prepared.checkpoint,
            output: Value::Null,
            surfaces: prepared.surfaces,
            resources: prepared.resources,
            effects: vec![],
        })
    }
    fn bind_resource(&self, _: &str, _: &Value) -> Result<Binding, String> {
        Ok(Arc::new(self.dependencies.clone()))
    }
}
fn optional_consumer() -> Candidate {
    Candidate {
        manifest: Manifest {
            id: "aaa-optional".into(),
            state_schema: 1,
            exports: vec![
                Export {
                    kind: Kind::Resource,
                    name: "pack".into(),
                    contract: "optional/1".into(),
                },
                Export {
                    kind: Kind::Command,
                    name: "select-optional".into(),
                    contract: "command/1".into(),
                },
            ],
            imports: vec![Import {
                module: "left".into(),
                contract: "pack/1".into(),
                optional: true,
            }],
        },
        identity: "optional-v1".into(),
        implementation: Arc::new(OptionalConsumer::default()),
    }
}
#[test]
fn optional_absence_and_late_provider_rebind_preserve_explicit_selection_and_leases() {
    let mut kernel = Kernel::default();
    install(&mut kernel, vec![optional_consumer()]);
    assert_eq!(
        kernel.resources()["aaa-optional/pack"].value["provider"],
        Value::Null
    );
    let checkpoint = kernel.checkpoint("aaa-optional").unwrap();
    assert!(
        kernel
            .dispatch(Kind::Command, "select-optional", &json!({"selected":true}))
            .is_err()
    );
    assert_eq!(kernel.checkpoint("aaa-optional").unwrap(), checkpoint);
    install(&mut kernel, vec![pack("left", 1, false)]);
    assert_eq!(
        kernel.resources()["aaa-optional/pack"].value["provider"]["version"],
        1
    );
    kernel
        .dispatch(Kind::Command, "select-optional", &json!({"selected":true}))
        .unwrap();
    let checkpoint = kernel.checkpoint("aaa-optional").unwrap();
    let old = kernel.resources()["aaa-optional/pack"].clone();
    assert!(kernel.stage(vec![pack("left", 99, false)]).is_err());
    assert_eq!(kernel.checkpoint("aaa-optional").unwrap(), checkpoint);
    assert_eq!(kernel.resources()["left/pack"].value["version"], 1);
    install(&mut kernel, vec![pack("left", 2, false)]);
    assert_eq!(kernel.checkpoint("aaa-optional").unwrap(), checkpoint);
    assert_eq!(
        kernel.resources()["aaa-optional/pack"].value["provider"]["version"],
        2
    );
    assert_eq!(
        old.binding::<Resources>()
            .unwrap()
            .get("left", "pack", "pack/1")
            .unwrap()
            .value["version"],
        1
    );
}
#[test]
fn present_optional_provider_is_ordered_contract_checked_and_cycles_are_rejected() {
    let mut kernel = Kernel::default();
    install(
        &mut kernel,
        vec![optional_consumer(), pack("left", 3, false)],
    );
    assert_eq!(
        kernel.resources()["aaa-optional/pack"].value["provider"]["version"],
        3
    );
    let old = kernel.resources();
    let mut wrong = pack("left", 4, false);
    wrong.manifest.exports[0].contract = "pack/2".into();
    assert!(kernel.stage(vec![wrong]).is_err());
    assert_eq!(kernel.resources(), old);
    let mut cycle = pack("left", 4, false);
    cycle.manifest.imports.push(Import {
        module: "aaa-optional".into(),
        contract: "optional/1".into(),
        optional: true,
    });
    assert!(kernel.stage(vec![cycle]).err().unwrap().contains("cycle"));
    assert_eq!(kernel.resources(), old);
}
#[test]
fn required_imports_stay_required_and_staged_absence_is_fenced_by_late_provider_commit() {
    let mut kernel = Kernel::default();
    let mut required = optional_consumer();
    required.manifest.imports[0].optional = false;
    assert!(kernel.stage(vec![required]).is_err());
    let absent = kernel.stage(vec![optional_consumer()]).unwrap();
    install(&mut kernel, vec![pack("left", 1, false)]);
    assert!(kernel.commit(absent).is_err());
    assert_eq!(kernel.generation("aaa-optional"), None);
    let legacy: Import =
        serde_json::from_value(json!({"module":"left","contract":"pack/1"})).unwrap();
    assert!(!legacy.optional);
    assert_eq!(
        serde_json::to_value(legacy).unwrap(),
        json!({"module":"left","contract":"pack/1"})
    );
}

#[test]
fn optional_provider_presence_without_resources_rebinds_and_fences_state_only_changes() {
    let mut kernel = Kernel::default();
    install(&mut kernel, vec![optional_consumer()]);
    assert_eq!(kernel.generation("aaa-optional"), Some(1));
    let mut provider = pack("left", 1, false);
    provider.implementation = Arc::new(Counter {
        step: 1,
        schema: 1,
        reject: false,
        effect: false,
    });
    install(&mut kernel, vec![provider]);
    assert_eq!(
        kernel.generation("aaa-optional"),
        Some(2),
        "presence change without a published resource must rebind its optional consumers"
    );
    let mut replacement = optional_consumer();
    replacement.identity = "optional-v2".into();
    let prepared = kernel.stage(vec![replacement]).unwrap();
    kernel.dispatch(Kind::Command, "left", &json!({})).unwrap();
    assert!(
        kernel.commit(prepared).is_err(),
        "present optional provider state changes must invalidate a staged consumer even without resource values"
    );
}

#[derive(Clone, Default)]
struct DrainConsumer {
    dependencies: Resources,
    select_on_completion: bool,
}
impl Implementation for DrainConsumer {
    fn with_resources(
        &self,
        dependencies: &Resources,
    ) -> Result<Option<Arc<dyn Implementation>>, String> {
        Ok(Some(Arc::new(Self {
            dependencies: dependencies.clone(),
            select_on_completion: self.select_on_completion,
        })))
    }
    fn prepare(&self, previous: Option<&Checkpoint>) -> Result<PreparedState, String> {
        self.rebind(&previous.cloned().unwrap_or(Checkpoint {
            schema: 1,
            state: json!({"count":0,"selected":false}),
        }))
    }
    fn rebind(&self, previous: &Checkpoint) -> Result<PreparedState, String> {
        let provider = self.dependencies.get("left", "pack", "pack/1").ok();
        if previous.state["selected"] == true && provider.is_none() {
            return Err("selected optional resource still required".into());
        }
        Ok(PreparedState {
            checkpoint: previous.clone(),
            surfaces: BTreeMap::from([(
                "drain-view".into(),
                Surface {
                    title: "count".into(),
                    lines: vec![previous.state["count"].to_string()],
                },
            )]),
            resources: BTreeMap::from([(
                "pack".into(),
                json!({"count":previous.state["count"],"provider":provider.map(|resource|resource.value.as_ref())}),
            )]),
        })
    }
    fn reduce(&self, checkpoint: &Checkpoint, event: &Value) -> Result<Transition, String> {
        let mut next = checkpoint.clone();
        let completion = event["type"] == "completion";
        if completion {
            next.state["count"] = json!(next.state["count"].as_u64().unwrap() + 1);
            if self.select_on_completion {
                next.state["selected"] = json!(true);
            }
        }
        let prepared = self.rebind(&next)?;
        let available = self
            .dependencies
            .optional_providers()
            .any(|(id, _)| id == "provider");
        let again = (event["wait"] == true
            || completion && next.state["count"].as_u64().unwrap() < 2)
            && available;
        Ok(Transition {
            checkpoint: prepared.checkpoint,
            surfaces: prepared.surfaces,
            resources: prepared.resources,
            output: Value::Null,
            effects: if again {
                vec![Effect {
                    service: "provider".into(),
                    method: "compute".into(),
                    arguments: Value::Null,
                }]
            } else {
                vec![]
            },
        })
    }
}
#[derive(Clone, Default)]
struct DrainMirror(Resources);
impl Implementation for DrainMirror {
    fn with_resources(
        &self,
        resources: &Resources,
    ) -> Result<Option<Arc<dyn Implementation>>, String> {
        Ok(Some(Arc::new(Self(resources.clone()))))
    }
    fn prepare(&self, previous: Option<&Checkpoint>) -> Result<PreparedState, String> {
        self.rebind(&previous.cloned().unwrap_or(Checkpoint {
            schema: 1,
            state: json!({"local":"kept"}),
        }))
    }
    fn rebind(&self, previous: &Checkpoint) -> Result<PreparedState, String> {
        Ok(PreparedState {
            checkpoint: previous.clone(),
            surfaces: BTreeMap::new(),
            resources: BTreeMap::from([(
                "pack".into(),
                self.0
                    .get("worker", "pack", "worker/1")?
                    .value
                    .as_ref()
                    .clone(),
            )]),
        })
    }
    fn reduce(&self, _: &Checkpoint, _: &Value) -> Result<Transition, String> {
        Err("not used".into())
    }
}
fn drain_fixture(selected: bool) -> Kernel {
    let worker = Candidate {
        manifest: Manifest {
            id: "worker".into(),
            state_schema: 1,
            exports: vec![
                Export {
                    kind: Kind::Command,
                    name: "worker".into(),
                    contract: "command/1".into(),
                },
                Export {
                    kind: Kind::Resource,
                    name: "pack".into(),
                    contract: "worker/1".into(),
                },
                Export {
                    kind: Kind::Surface,
                    name: "drain-view".into(),
                    contract: "surface/1".into(),
                },
            ],
            imports: vec![
                Import {
                    module: "left".into(),
                    contract: "pack/1".into(),
                    optional: true,
                },
                Import {
                    module: "provider".into(),
                    contract: "compute/1".into(),
                    optional: true,
                },
            ],
        },
        identity: "worker-v1".into(),
        implementation: Arc::new(DrainConsumer {
            select_on_completion: selected,
            ..Default::default()
        }),
    };
    let mirror = Candidate {
        manifest: Manifest {
            id: "mirror".into(),
            state_schema: 1,
            exports: vec![Export {
                kind: Kind::Resource,
                name: "pack".into(),
                contract: "mirror/1".into(),
            }],
            imports: vec![Import {
                module: "worker".into(),
                contract: "worker/1".into(),
                optional: false,
            }],
        },
        identity: "mirror-v1".into(),
        implementation: Arc::new(DrainMirror::default()),
    };
    let mut provider = candidate("provider", 10, 1);
    provider.manifest.exports = vec![Export {
        kind: Kind::Service,
        name: "compute".into(),
        contract: "compute/1".into(),
    }];
    let mut kernel = Kernel::default();
    install(
        &mut kernel,
        vec![
            worker,
            mirror,
            provider,
            pack("left", 1, false),
            candidate("spare", 7, 1),
        ],
    );
    kernel
}
fn start_drain(kernel: &mut Kernel) -> (IssuedEffect, WithdrawalStatus) {
    let first = kernel
        .dispatch(Kind::Command, "worker", &json!({"wait":true}))
        .unwrap()
        .effects
        .pop()
        .unwrap();
    let status = kernel
        .begin_withdraw(vec![("left".into(), 1), ("provider".into(), 1)])
        .unwrap();
    (first, status)
}
#[test]
fn pending_withdrawal_drains_descendants_then_rebinds_the_final_checkpoint_atomically() {
    let mut kernel = drain_fixture(false);
    let (first, status) = start_drain(&mut kernel);
    assert_eq!(status.phase, WithdrawalPhase::Draining);
    assert_eq!(status.pending_requests, 1);
    assert!(
        kernel
            .dispatch(Kind::Command, "worker", &Value::Null)
            .is_err()
    );
    assert!(
        kernel
            .routes()
            .iter()
            .any(|route| ["worker", "compute"].contains(&route.export.name.as_str()))
    );
    assert_eq!(kernel.resources()["worker/pack"].value["count"], 0);
    assert_eq!(kernel.resources()["mirror/pack"].value["count"], 0);
    assert!(kernel.resources().contains_key("left/pack"));
    assert_eq!(kernel.surfaces()["drain-view"].lines, vec!["0"]);
    kernel
        .dispatch(Kind::Command, "spare", &Value::Null)
        .unwrap();
    assert_eq!(kernel.checkpoint("spare").unwrap().state, 7);
    assert!(kernel.stage(vec![candidate("provider", 1000, 1)]).is_err());
    kernel.invoke(&first).unwrap();
    let second = kernel
        .complete(&first.job, Value::Null)
        .unwrap()
        .effects
        .pop()
        .unwrap();
    assert_eq!(second.target.generation, first.target.generation);
    assert_ne!(second.job.request, first.job.request);
    assert_eq!(
        kernel.withdrawal_status(status.id).unwrap().phase,
        WithdrawalPhase::Draining
    );
    assert_eq!(kernel.checkpoint("worker").unwrap().state["count"], 1);
    assert_eq!(kernel.generation("worker"), Some(1));
    assert!(kernel.invoke(&first).is_err());
    assert!(kernel.complete(&first.job, Value::Null).is_err());
    kernel.invoke(&second).unwrap();
    kernel.complete(&second.job, Value::Null).unwrap();
    assert_eq!(
        kernel.withdrawal_status(status.id).unwrap().phase,
        WithdrawalPhase::Published
    );
    assert!(kernel.pending.is_none());
    assert!(kernel.work.is_empty());
    assert_eq!(kernel.generation("left"), None);
    assert_eq!(kernel.generation("provider"), None);
    assert_eq!(kernel.checkpoint("worker").unwrap().state["count"], 2);
    assert_eq!(kernel.resources()["worker/pack"].value["count"], 2);
    assert_eq!(kernel.resources()["mirror/pack"].value["count"], 2);
    assert_eq!(
        kernel.resources()["worker/pack"].value["provider"],
        Value::Null
    );
    assert_eq!(kernel.checkpoint("mirror").unwrap().state["local"], "kept");
    assert_eq!(kernel.surfaces()["drain-view"].lines, vec!["2"]);
    assert_eq!(kernel.generation("worker"), Some(2));
    kernel
        .dispatch(Kind::Command, "worker", &Value::Null)
        .unwrap();
}
#[test]
fn failed_final_optional_rebind_keeps_completion_updates_and_abort_restores_them() {
    let mut kernel = drain_fixture(true);
    let (first, status) = start_drain(&mut kernel);
    kernel.invoke(&first).unwrap();
    let second = kernel
        .complete(&first.job, Value::Null)
        .unwrap()
        .effects
        .pop()
        .unwrap();
    kernel.invoke(&second).unwrap();
    kernel.complete(&second.job, Value::Null).unwrap();
    let failed = kernel.withdrawal_status(status.id).unwrap();
    assert_eq!(failed.phase, WithdrawalPhase::Draining);
    assert!(failed.error.unwrap().contains("selected optional"));
    assert_eq!(failed.pending_requests, 0);
    assert_eq!(kernel.checkpoint("worker").unwrap().state["count"], 2);
    assert_eq!(kernel.generation("left"), Some(1));
    assert!(
        kernel
            .dispatch(Kind::Command, "worker", &Value::Null)
            .is_err()
    );
    assert!(kernel.retry_withdraw(status.id).unwrap().error.is_some());
    let restored = kernel.abort_withdraw(status.id).unwrap();
    assert_eq!(restored.phase, WithdrawalPhase::Aborted);
    assert_eq!(kernel.resources()["worker/pack"].value["count"], 2);
    assert_eq!(kernel.resources()["mirror/pack"].value["count"], 2);
    assert!(kernel.resources()["left/pack"].value.is_object());
    assert_eq!(kernel.checkpoint("worker").unwrap().state["selected"], true);
    assert!(kernel.pending.is_none());
    assert!(kernel.work.is_empty());
}
#[test]
fn abort_waits_for_admitted_work_and_cancel_can_finish_a_withdrawal_without_replay() {
    let mut kernel = drain_fixture(false);
    let (first, status) = start_drain(&mut kernel);
    let abort = kernel.abort_withdraw(status.id).unwrap();
    assert_eq!(abort.phase, WithdrawalPhase::Draining);
    assert!(abort.abort_requested);
    kernel.invoke(&first).unwrap();
    let second = kernel
        .complete(&first.job, Value::Null)
        .unwrap()
        .effects
        .pop()
        .unwrap();
    kernel.invoke(&second).unwrap();
    kernel.complete(&second.job, Value::Null).unwrap();
    assert_eq!(
        kernel.withdrawal_status(status.id).unwrap().phase,
        WithdrawalPhase::Aborted
    );
    assert_eq!(kernel.checkpoint("worker").unwrap().state["count"], 2);
    assert_eq!(kernel.generation("provider"), Some(1));
    assert!(
        kernel
            .routes()
            .iter()
            .any(|route| route.export.name == "compute")
    );
    let (first, status) = start_drain(&mut kernel);
    let mut forged = first.job.clone();
    forged.generation += 1;
    assert!(kernel.cancel(&forged).is_err());
    kernel.cancel(&first.job).unwrap();
    assert_eq!(
        kernel.withdrawal_status(status.id).unwrap().phase,
        WithdrawalPhase::Published
    );
    assert!(kernel.invoke(&first).is_err());
    assert!(kernel.complete(&first.job, Value::Null).is_err());
    assert!(kernel.work.is_empty());
    assert!(kernel.pending.is_none());
}
#[test]
fn pending_withdrawal_fences_stale_transactions_requests_and_same_name_readds() {
    let mut kernel = drain_fixture(false);
    let stale = kernel.stage(vec![candidate("unpublished", 1, 1)]).unwrap();
    assert!(kernel.begin_withdraw(vec![("left".into(), 99)]).is_err());
    let (first, status) = start_drain(&mut kernel);
    assert!(kernel.commit(stale).is_err());
    assert!(kernel.begin_withdraw(vec![("left".into(), 1)]).is_err());
    assert!(kernel.abort_withdraw(status.id + 1).is_err());
    install(&mut kernel, vec![candidate("unrelated", 3, 1)]);
    kernel
        .dispatch(Kind::Command, "unrelated", &Value::Null)
        .unwrap();
    kernel.cancel(&first.job).unwrap();
    assert_eq!(
        kernel.withdrawal_status(status.id).unwrap().phase,
        WithdrawalPhase::Published
    );
    assert!(kernel.abort_withdraw(status.id).is_err());
    assert!(kernel.retry_withdraw(status.id).is_err());
    install(&mut kernel, vec![pack("left", 2, false)]);
    assert_eq!(kernel.generation("left"), Some(2));
    assert!(kernel.begin_withdraw(vec![("left".into(), 1)]).is_err());
    let next = kernel.begin_withdraw(vec![("left".into(), 2)]).unwrap();
    assert_eq!(next.phase, WithdrawalPhase::Published);
    assert!(kernel.withdrawal_status(status.id).is_err());
}

#[test]
fn pending_withdrawal_rejects_required_dependents_without_hiding_any_publication() {
    let mut kernel = drain_fixture(false);
    let routes = kernel.routes();
    let resources = kernel.resources();
    assert!(
        kernel
            .begin_withdraw(vec![("worker".into(), 1)])
            .unwrap_err()
            .contains("requires")
    );
    assert_eq!(kernel.routes(), routes);
    assert_eq!(kernel.resources(), resources);
    assert!(kernel.pending.is_none());
    assert!(kernel.withdrawal_status.is_none());
}
#[test]
fn completed_withdrawal_releases_internal_leases_but_existing_instances_still_own_theirs() {
    let mut kernel = drain_fixture(false);
    let resource = kernel.resources()["worker/pack"].clone();
    let weak = Arc::downgrade(&kernel.resources()["left/pack"].binding);
    let instance = resource.instantiate(&Value::Null).unwrap();
    let status = kernel
        .begin_withdraw(vec![("left".into(), 1), ("provider".into(), 1)])
        .unwrap();
    assert_eq!(status.phase, WithdrawalPhase::Published);
    assert!(kernel.pending.is_none());
    assert!(kernel.work.is_empty());
    assert!(weak.upgrade().is_some());
    drop(resource);
    assert!(weak.upgrade().is_some());
    drop(instance);
    assert!(weak.upgrade().is_none());
}
#[test]
fn pending_withdrawal_can_cancel_an_already_started_service_and_never_replay_it() {
    let mut kernel = drain_fixture(false);
    let first = kernel
        .dispatch(Kind::Command, "worker", &json!({"wait":true}))
        .unwrap()
        .effects
        .pop()
        .unwrap();
    kernel.invoke(&first).unwrap();
    assert_eq!(kernel.checkpoint("provider").unwrap().state, 10);
    let status = kernel
        .begin_withdraw(vec![("left".into(), 1), ("provider".into(), 1)])
        .unwrap();
    assert_eq!(status.phase, WithdrawalPhase::Draining);
    kernel.cancel(&first.job).unwrap();
    assert_eq!(
        kernel.withdrawal_status(status.id).unwrap().phase,
        WithdrawalPhase::Published
    );
    assert!(kernel.invoke(&first).is_err());
    assert!(kernel.complete(&first.job, Value::Null).is_err());
    assert_eq!(kernel.checkpoint("worker").unwrap().state["count"], 0);
}
#[test]
fn withdrawal_counter_exhaustion_leaves_admissions_state_and_jobs_unchanged() {
    for epoch in [true, false] {
        let mut kernel = drain_fixture(false);
        let routes = kernel.routes();
        let resources = kernel.resources();
        if epoch {
            kernel.epoch = u64::MAX;
        } else {
            kernel.next_withdrawal = u64::MAX;
        }
        assert!(
            kernel
                .begin_withdraw(vec![("left".into(), 1)])
                .unwrap_err()
                .contains("exhausted")
        );
        assert_eq!(kernel.routes(), routes);
        assert_eq!(kernel.resources(), resources);
        assert!(kernel.pending.is_none());
    }
}

#[test]
fn withdrawal_reserves_two_epochs_before_acceptance_without_tagging_work_on_failure() {
    for epoch in [u64::MAX - 1, u64::MAX] {
        let mut kernel = drain_fixture(false);
        let first = kernel
            .dispatch(Kind::Command, "worker", &json!({"wait":true}))
            .unwrap()
            .effects
            .pop()
            .unwrap();
        kernel.epoch = epoch;
        let routes = kernel.routes();
        let resources = kernel.resources();
        let checkpoint = kernel.checkpoint("worker");
        assert!(
            kernel
                .begin_withdraw(vec![("left".into(), 1), ("provider".into(), 1)])
                .unwrap_err()
                .contains("epoch capacity")
        );
        assert_eq!(kernel.epoch, epoch);
        assert_eq!(kernel.routes(), routes);
        assert_eq!(kernel.resources(), resources);
        assert_eq!(kernel.checkpoint("worker"), checkpoint);
        assert_eq!(kernel.next_withdrawal, 0);
        assert!(kernel.withdrawal_status.is_none());
        assert!(kernel.pending.is_none());
        assert_eq!(kernel.work[&first.job.request].drain, None);
        kernel.cancel(&first.job).unwrap();
    }
}
#[test]
fn pending_epoch_reservation_survives_unrelated_commits_until_final_publication() {
    for starting_epoch in [u64::MAX - 2, u64::MAX - 3] {
        let mut kernel = drain_fixture(false);
        kernel.epoch = starting_epoch;
        let (first, status) = start_drain(&mut kernel);
        if starting_epoch == u64::MAX - 3 {
            install(&mut kernel, vec![candidate("allowed-unrelated", 1, 1)]);
            assert!(kernel.identity("allowed-unrelated").is_some());
        }
        assert_eq!(kernel.epoch, u64::MAX - 1);
        let last = kernel
            .stage(vec![candidate("reserved-slot", 1, 1)])
            .unwrap();
        assert!(kernel.commit(last).unwrap_err().contains("reserved"));
        assert!(kernel.identity("reserved-slot").is_none());
        assert_eq!(kernel.epoch, u64::MAX - 1);
        // Independent state-only work remains admitted without using an epoch.
        kernel
            .dispatch(Kind::Command, "spare", &Value::Null)
            .unwrap();
        assert_eq!(kernel.checkpoint("spare").unwrap().state, 7);
        assert_eq!(kernel.epoch, u64::MAX - 1);
        kernel.invoke(&first).unwrap();
        let second = kernel
            .complete(&first.job, Value::Null)
            .unwrap()
            .effects
            .pop()
            .unwrap();
        kernel.invoke(&second).unwrap();
        kernel.complete(&second.job, Value::Null).unwrap();
        assert_eq!(
            kernel.withdrawal_status(status.id).unwrap().phase,
            WithdrawalPhase::Published
        );
        assert_eq!(kernel.epoch, u64::MAX);
        assert_eq!(kernel.resources()["mirror/pack"].value["count"], 2);
        assert!(kernel.work.is_empty());
        assert!(kernel.pending.is_none());
    }
}
#[test]
fn reserved_epoch_remains_available_for_state_rebinding_abort_after_final_refusal() {
    let mut kernel = drain_fixture(true);
    kernel.epoch = u64::MAX - 2;
    let (first, status) = start_drain(&mut kernel);
    kernel.invoke(&first).unwrap();
    let second = kernel
        .complete(&first.job, Value::Null)
        .unwrap()
        .effects
        .pop()
        .unwrap();
    kernel.invoke(&second).unwrap();
    kernel.complete(&second.job, Value::Null).unwrap();
    assert_eq!(kernel.epoch, u64::MAX - 1);
    assert_eq!(
        kernel.withdrawal_status(status.id).unwrap().phase,
        WithdrawalPhase::Draining
    );
    assert!(kernel.withdrawal_status(status.id).unwrap().error.is_some());
    let unrelated = kernel
        .stage(vec![candidate("would-trap-abort", 1, 1)])
        .unwrap();
    assert!(kernel.commit(unrelated).unwrap_err().contains("reserved"));
    let aborted = kernel.abort_withdraw(status.id).unwrap();
    assert_eq!(aborted.phase, WithdrawalPhase::Aborted);
    // Abort really needed the reserved epoch to rebind mirror from count0 to2.
    assert_eq!(kernel.epoch, u64::MAX);
    assert_eq!(kernel.generation("mirror"), Some(2));
    assert_eq!(kernel.resources()["mirror/pack"].value["count"], 2);
    assert_eq!(kernel.resources()["worker/pack"].value["count"], 2);
    assert_eq!(kernel.generation("left"), Some(1));
    assert!(kernel.pending.is_none());
    assert!(kernel.work.is_empty());
}

#[test]
fn drain_admissions_are_separate_from_retained_resource_surface_and_route_publication() {
    let mut kernel = drain_fixture(false);
    install(&mut kernel, vec![pack("independent", 5, false)]);
    let first = kernel
        .dispatch(Kind::Command, "worker", &json!({"wait":true}))
        .unwrap()
        .effects
        .pop()
        .unwrap();
    let published = kernel.resources();
    let surfaces = kernel.surfaces();
    let routes = kernel.routes();
    let status = kernel
        .begin_withdraw(vec![("left".into(), 1), ("provider".into(), 1)])
        .unwrap();
    assert_eq!(kernel.resources(), published);
    assert_eq!(kernel.surfaces(), surfaces);
    assert_eq!(kernel.routes(), routes);
    assert!(
        kernel
            .dispatch(Kind::Command, "worker", &Value::Null)
            .is_err()
    );
    kernel
        .dispatch(Kind::Command, "independent", &json!({"delta":2}))
        .unwrap();
    assert_eq!(
        kernel.resources()["independent/pack"].value["version"],
        7,
        "unaffected publication must stay live"
    );
    kernel.invoke(&first).unwrap();
    let second = kernel
        .complete(&first.job, Value::Null)
        .unwrap()
        .effects
        .pop()
        .unwrap();
    assert_eq!(kernel.checkpoint("worker").unwrap().state["count"], 1);
    for key in ["left/pack", "worker/pack", "mirror/pack"] {
        let resource = kernel.resources()[key].clone();
        assert_eq!(resource, published[key]);
        assert!(Arc::ptr_eq(&resource.value, &published[key].value));
        assert!(Arc::ptr_eq(&resource.binding, &published[key].binding));
    }
    assert_eq!(kernel.surfaces(), surfaces);
    assert_eq!(kernel.routes(), routes);
    kernel.invoke(&second).unwrap();
    kernel.complete(&second.job, Value::Null).unwrap();
    assert_eq!(
        kernel.withdrawal_status(status.id).unwrap().phase,
        WithdrawalPhase::Published
    );
    assert!(!kernel.resources().contains_key("left/pack"));
    assert!(
        !kernel
            .routes()
            .iter()
            .any(|route| route.export.name == "compute")
    );
    assert_eq!(kernel.resources()["worker/pack"].value["count"], 2);
    assert_eq!(kernel.resources()["mirror/pack"].value["count"], 2);
    assert_eq!(kernel.surfaces()["drain-view"].lines, vec!["2"]);
    assert_eq!(published["worker/pack"].value["count"], 0);
}
#[test]
fn failed_publication_retains_old_leases_and_abort_publishes_final_drained_state() {
    let mut kernel = drain_fixture(true);
    let first = kernel
        .dispatch(Kind::Command, "worker", &json!({"wait":true}))
        .unwrap()
        .effects
        .pop()
        .unwrap();
    let published = kernel.resources();
    let surfaces = kernel.surfaces();
    let routes = kernel.routes();
    let status = kernel
        .begin_withdraw(vec![("left".into(), 1), ("provider".into(), 1)])
        .unwrap();
    kernel.invoke(&first).unwrap();
    let second = kernel
        .complete(&first.job, Value::Null)
        .unwrap()
        .effects
        .pop()
        .unwrap();
    kernel.invoke(&second).unwrap();
    kernel.complete(&second.job, Value::Null).unwrap();
    assert!(kernel.withdrawal_status(status.id).unwrap().error.is_some());
    assert_eq!(kernel.checkpoint("worker").unwrap().state["count"], 2);
    assert_eq!(kernel.resources(), published);
    assert_eq!(kernel.surfaces(), surfaces);
    assert_eq!(kernel.routes(), routes);
    assert!(Arc::ptr_eq(
        &kernel.resources()["worker/pack"].binding,
        &published["worker/pack"].binding
    ));
    assert!(kernel.retry_withdraw(status.id).unwrap().error.is_some());
    assert_eq!(kernel.resources(), published);
    let aborted = kernel.abort_withdraw(status.id).unwrap();
    assert_eq!(aborted.phase, WithdrawalPhase::Aborted);
    assert!(kernel.resources().contains_key("left/pack"));
    assert!(
        kernel
            .routes()
            .iter()
            .any(|route| route.export.name == "compute")
    );
    assert_eq!(kernel.resources()["worker/pack"].value["count"], 2);
    assert_eq!(kernel.resources()["mirror/pack"].value["count"], 2);
    assert_eq!(kernel.surfaces()["drain-view"].lines, vec!["2"]);
    assert!(
        kernel
            .dispatch(Kind::Command, "worker", &Value::Null)
            .is_ok()
    );
    assert!(kernel.pending.is_none());
    assert!(kernel.work.is_empty());
}
