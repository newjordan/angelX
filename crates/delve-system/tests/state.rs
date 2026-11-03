use angel_delve::rules::{Input, Run, Triumph, runtime_state as state};
use std::collections::BTreeMap;

/// An older checkpoint made from today's Run: schema 8 added the squire
/// company's reservation state, which no earlier save carries.
fn before_schema_eight(run: &mut serde_json::Value) {
    let fields = run.as_object_mut().unwrap();
    for name in ["squire_company_reserved", "squire_company_claimed", "squire_pending_delves"] {
        fields.remove(name);
    }
}

#[test]
fn live_checkpoint_keeps_queues_latches_and_private_state_without_changing_saves() {
    let mut run = Run::new(77, 123, None);
    run.triumph = Some(Triumph::Dragon);
    run.feats.push("first_blood");
    run.cues.push("unconsumed cue".into());
    run.sounds.push("hit");
    run.seat_sounds.push((1, "knights/a_ooh_1"));
    run.feel.echoes.push((89, "gore_kick"));
    run.feel.lines.push("pending ledger".into());
    run.kingdom.door_latched.insert(1);
    run.latch_input_age.insert(2, 7);
    run.playtest.dirty = true;
    let saved_before = serde_json::to_value(&run).unwrap();
    assert!(saved_before.get("cues").is_none());
    let snapshot = state::capture(&run).unwrap();
    // Exercise private fields through the public versioned handoff contract.
    let mut json = snapshot.run;
    json["descend_for"] = "stair".into();
    json["lever_held"] = 3.into();
    json["offer_keys"] = serde_json::json!({"1": 2});
    json["idle"] = serde_json::json!({"1": 19});
    json["shoulders"]["keys"] = serde_json::json!([1, 2]);
    json["lamp"]["held"] = serde_json::json!([1]);
    let restored = state::restore(state::Checkpoint {
        schema: state::SCHEMA,
        run: json.clone(),
    })
    .unwrap();
    assert_eq!(state::capture(&restored).unwrap().run, json);
    assert_eq!(serde_json::to_value(&run).unwrap(), saved_before);
    assert_eq!(restored.feats, run.feats);
    assert_eq!(restored.seat_sounds, run.seat_sounds);
    assert_eq!(restored.feel.echoes, run.feel.echoes);
    assert_eq!(restored.kingdom.door_latched, run.kingdom.door_latched);
    let mut incomplete = json.clone();
    incomplete.as_object_mut().unwrap().remove("cues");
    assert!(
        state::restore(state::Checkpoint {
            schema: state::SCHEMA,
            run: incomplete
        })
        .is_err()
    );
    // Ordinary save/wire decoding continues to ignore private live-only fields.
    let durable: Run = serde_json::from_value(json.clone()).unwrap();
    assert!(durable.cues.is_empty());
    assert!(durable.kingdom.door_latched.is_empty());
    assert!(
        state::restore(state::Checkpoint {
            schema: 999,
            run: json
        })
        .is_err()
    );
}

#[test]
fn repeated_handoffs_match_uninterrupted_full_simulation() {
    let mut uninterrupted = Run::new(310, 99, None);
    let mut handed = uninterrupted.clone();
    for tick in 0..180 {
        let inputs = BTreeMap::from([(
            1,
            Input {
                move_x: if tick % 60 < 30 { 1 } else { -1 },
                fire: tick % 3 == 0,
                ..Input::default()
            },
        )]);
        uninterrupted.step(&inputs);
        let bytes = serde_json::to_vec(&state::capture(&handed).unwrap()).unwrap();
        handed = state::restore(serde_json::from_slice(&bytes).unwrap()).unwrap();
        handed.step(&inputs);
        assert_eq!(
            state::capture(&handed).unwrap().run,
            state::capture(&uninterrupted).unwrap().run,
            "tick {tick}"
        );
    }
}

#[test]
fn borrowed_rpc_decode_has_the_same_complete_state_contract() {
    let run = Run::new(9, 13, None);
    let checkpoint = state::capture(&run).unwrap();
    let mut value = serde_json::to_value(&checkpoint).unwrap();
    assert_eq!(
        state::capture(&state::restore_value(&value).unwrap())
            .unwrap()
            .run,
        checkpoint.run
    );
    value["extra"] = true.into();
    assert!(state::restore_value(&value).is_err());
    value.as_object_mut().unwrap().remove("extra");
    value["schema"] = serde_json::json!(u64::MAX);
    assert!(state::restore_value(&value).is_err());
    value["schema"] = serde_json::json!(state::SCHEMA);
    value["run"].as_object_mut().unwrap().remove("cues");
    assert!(state::restore_value(&value).is_err());
}

#[test]
fn schema_one_migration_and_pending_module_requests_are_explicit() {
    let mut run = Run::new(3, 9, None);
    let checkpoint = state::capture(&run).unwrap();
    let mut old = checkpoint.run.clone();
    old.as_object_mut().unwrap().remove("module_casts");
    old.as_object_mut().unwrap().remove("module_visuals");
    before_schema_eight(&mut old);
    let migrated = state::restore(state::Checkpoint {
        schema: 1,
        run: old.clone(),
    })
    .unwrap();
    assert!(migrated.module_casts.is_empty());
    assert!(
        state::restore(state::Checkpoint {
            schema: state::SCHEMA,
            run: old
        })
        .is_err()
    );
    run.module_casts
        .push(angel_delve::rules::abilities::Request {
            raid: run.raid_id,
            epoch: run.seat_epoch(1),
            definition: angel_delve::rules::abilities::Slot { module: "abilities".into(), generation: 1, ability: "spark".into(), name: "Spark".into(), mana: 1.0, cooldown: 30, icon: String::new() },
            player: 1,
            slot: 0,
            module: "abilities".into(),
            generation: 1,
            ability: "spark".into(),
            tick: 9,
        });
    let restored = state::restore_value(&state::capture(&run).unwrap().into_value()).unwrap();
    assert_eq!(restored.module_casts.len(), 1);
    assert_eq!(restored.module_casts[0].ability, "spark");
    assert!(
        serde_json::to_value(&run)
            .unwrap()
            .get("module_casts")
            .is_none()
    );
}

#[test]
fn visual_state_is_complete_in_handoffs_host_owned_during_rules_rpc_and_absent_from_saves() {
    let mut run = Run::new(3, 9, None);
    let id = "a".repeat(64);
    run.module_visuals=serde_json::from_value(serde_json::json!({
        "graphs":{id.clone():{"ticks":30,"nodes":[{"keys":[{"tick":0,"offset":[0,0],"scale":1,"colour":[255,80,20],"opacity":255}],"shape":{"type":"ring","radius":1,"width":1}}]}},
        "effects":[{"id":1,"player":1,"origin":{"module":"abilities","generation":1,"cast":"one"},"graph":id,"born":0,"position":[8,8],"follow":false}]
    })).unwrap();
    let full = state::capture(&run).unwrap();
    let restored = state::restore(full.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&restored.module_visuals).unwrap(),
        serde_json::to_value(&run.module_visuals).unwrap()
    );
    assert!(
        serde_json::to_value(&run)
            .unwrap()
            .get("module_visuals")
            .is_none()
    );
    let mut worker = state::restore(state::capture_for_rules(&run).unwrap()).unwrap();
    assert!(worker.module_visuals.is_empty());
    state::retain_appearance(&mut worker, &run);
    assert_eq!(worker.module_visuals.effects[0].origin.cast, "one");
    let mut missing = full.run.clone();
    missing.as_object_mut().unwrap().remove("module_visuals");
    before_schema_eight(&mut missing);
    assert!(
        state::restore(state::Checkpoint {
            schema: state::SCHEMA,
            run: missing.clone()
        })
        .is_err()
    );
    let migrated = state::restore(state::Checkpoint {
        schema: 2,
        run: missing,
    })
    .unwrap();
    assert!(migrated.module_visuals.is_empty());
    let mut full_run = full.run;
    before_schema_eight(&mut full_run);
    assert!(
        state::restore(state::Checkpoint {
            schema: 2,
            run: full_run
        })
        .is_err(),
        "new visual data cannot masquerade as schema two"
    );
}

#[test]
fn schema_six_migrates_only_an_empty_ability_outbox_without_modifying_input() {
    let run = Run::new(9, 13, None);
    let mut old = state::capture(&run).unwrap().into_value();
    old["schema"] = 6.into();
    before_schema_eight(&mut old["run"]);
    let before = serde_json::to_vec(&old).unwrap();
    let migrated = state::restore_value(&old).unwrap();
    assert!(migrated.module_casts.is_empty());
    assert_eq!(serde_json::to_vec(&old).unwrap(), before);
    old["run"]["module_casts"] = serde_json::json!([{
        "player":1,"slot":0,"module":"old","generation":1,"ability":"old","tick":0
    }]);
    let before = serde_json::to_vec(&old).unwrap();
    assert!(state::restore_value(&old).unwrap_err().contains("drain before reload"));
    assert_eq!(serde_json::to_vec(&old).unwrap(), before);
}
