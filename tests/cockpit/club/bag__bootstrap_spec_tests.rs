use super::*;
use std::collections::HashMap;

#[test]
fn default_bootstrap_is_one_generic_local_box() {
    let specs = Bag::bootstrap_specs(|_| None);
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].name, "local");
    assert!(specs[0].host.is_empty());
    assert_eq!(specs[0].fallback_ip, "127.0.0.1");
    assert_eq!(
        specs[0]
            .slots
            .iter()
            .map(|slot| (slot.label, slot.env_key, slot.port, slot.is_swarm))
            .collect::<Vec<_>>(),
        vec![
            ("local", "LOCAL", 8080, false),
            ("swarm", "LOCAL", 8080, true),
        ]
    );
    assert!(specs[0].slots.iter().any(|slot| slot.label == "swarm"));
}

#[test]
fn legacy_slots_require_explicit_url_pins() {
    let mut configured = HashMap::new();
    configured.insert(
        "ANGEL_SPARK_URL".to_string(),
        "http://legacy.example/v1".to_string(),
    );
    configured.insert(
        "ANGEL_TURBO_URL".to_string(),
        "http://turbo.example/v1".to_string(),
    );
    configured.insert(
        "ANGEL_GEMMA_URL".to_string(),
        "http://gemma.example/v1".to_string(),
    );
    let specs = Bag::bootstrap_specs(|key| configured.get(key).cloned());
    let local_slots = specs[0]
        .slots
        .iter()
        .map(|slot| slot.label)
        .collect::<Vec<_>>();
    assert_eq!(local_slots, vec!["local", "local-swarm"]);
    assert_eq!(specs[1].name, "spark");
    assert_eq!(
        specs[1]
            .slots
            .iter()
            .map(|slot| (slot.label, slot.port, slot.is_swarm))
            .collect::<Vec<_>>(),
        vec![
            ("spark", 0, false),
            ("swarm", 0, true),
            ("gemma", 0, false),
            ("coder", 0, false),
        ]
    );
    assert_eq!(
        specs
            .iter()
            .flat_map(|spec| spec.slots.iter())
            .filter(|slot| slot.label == "swarm")
            .count(),
        1
    );
    assert_eq!(specs[0].slots[1].label, "local-swarm");
    assert_eq!(specs[2].name, "turbo");
    assert_eq!(specs[2].slots[0].label, "turbo");
    assert_eq!(specs[2].slots[0].port, 0);
    assert_eq!(specs.len(), 3);
}
