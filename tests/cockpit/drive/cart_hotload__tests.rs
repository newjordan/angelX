use crate::agent::hotload::{bind_resource, evaluate, validate_host_contract};

#[test]
fn character_module_publishes_prepared_native_resources_through_its_registered_command() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("live");
    let mut registry = angel_live::Registry::with_resources(
        root,
        evaluate,
        validate_host_contract,
        bind_resource,
    );
    registry.refresh(true);
    let response = registry
        .dispatch(
            angel_live::kernel::Kind::Command,
            "character",
            &serde_json::json!({"type":"command","name":"character","arguments":"gold 2"}),
        )
        .unwrap();
    assert_eq!(response["output"], "character gold selected for player 2");
    let snapshot = registry.snapshot();
    let appearance = snapshot.resources["abilities/character-seat-2"]
        .binding::<crate::delve::together_abilities::Loadout>()
        .unwrap()
        .appearance
        .as_ref()
        .unwrap();
    let mut run = angel_delve::rules::Run::new(3, 9, Some("Two"));
    assert!(appearance.install(&mut run));
    assert_eq!(
        run.players[&2].avatar.as_ref().unwrap().colour,
        [225, 165, 50]
    );
    assert!(
        super::character_resource(
            "another-seat",
            &serde_json::json!({"player":2,"disabled":true}),
            &Default::default()
        )
        .is_err()
    );
}
