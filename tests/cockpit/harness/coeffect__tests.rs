use super::*;

fn spec(id: &str) -> Requirement {
    Requirement::any([Key::provider(id)])
}

#[test]
fn binding_a_required_key_activates_and_releasing_it_deactivates() {
    let mut store = CoeffectStore::default();
    let requirement = spec("mcp:web");
    let change = store.bind(Key::provider("mcp:web"), Value::Count(3));
    assert_eq!(
        change.react(&requirement).classification,
        Classification::Activating
    );
    assert!(store.satisfied(&requirement));

    let change = store.release(&Key::provider("mcp:web"));
    assert_eq!(
        change.react(&requirement).classification,
        Classification::Deactivating
    );
    assert!(!store.satisfied(&requirement));
}

#[test]
fn an_unrelated_key_is_neutral_by_independence() {
    let mut store = CoeffectStore::default();
    store.bind(Key::provider("mcp:web"), Value::Count(1));
    let change = store.bind(Key::provider("mcp:files"), Value::Count(1));
    assert!(change.react(&spec("mcp:web")).is_neutral());
}

#[test]
fn a_present_but_unready_binding_does_not_satisfy() {
    let mut store = CoeffectStore::default();
    let requirement = spec("mcp:web");
    let change = store.bind(Key::provider("mcp:web"), Value::Count(0));
    assert_eq!(
        change.react(&requirement).classification,
        Classification::Neutral,
        "declared-but-not-ready is not activation"
    );
    assert!(!store.satisfied(&requirement));

    let change = store.retune(Key::provider("mcp:web"), Value::Count(2));
    assert_eq!(
        change.react(&requirement).classification,
        Classification::Activating,
        "readiness arriving later is the activation"
    );
}

#[test]
fn a_replaced_provider_is_a_change_even_when_the_value_is_equal() {
    let mut store = CoeffectStore::default();
    let requirement = spec("mcp:web");
    store.bind(Key::provider("mcp:web"), Value::Count(1));
    let change = store.bind(Key::provider("mcp:web"), Value::Count(1));
    let reaction = change.react(&requirement);
    assert_eq!(reaction.classification, Classification::Neutral);
    assert!(reaction.identity_changed, "§5.1.3 provider identity");
    assert!(reaction.reloads());
}

#[test]
fn a_retune_under_a_live_provider_reloads_nothing() {
    let mut store = CoeffectStore::default();
    let requirement = spec("mcp:web");
    store.bind(Key::provider("mcp:web"), Value::Count(1));
    let before = store.version();
    let change = store.retune(Key::provider("mcp:web"), Value::Count(4));
    assert!(change.react(&requirement).is_neutral());
    assert_eq!(
        store.version(),
        before,
        "a retune is invisible to derivations"
    );
}
