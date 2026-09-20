use super::*;

fn temp_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "angel-route-memory-{label}-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn preference(agent: &str, driver: &str, model: &str, effort: Option<&str>) -> RoutePreference {
    RoutePreference {
        v: SCHEMA_V,
        route_id: None,
        agent: agent.to_string(),
        driver: driver.to_string(),
        model: model.to_string(),
        reasoning_effort: effort.map(str::to_string),
    }
}

#[test]
fn atomic_round_trip_is_versioned_and_bounded() {
    let path = temp_path("roundtrip");
    let expected = preference("openai", "openai", "gpt-5.6-sol", Some("ultra"));
    save_to(&path, &expected).unwrap();
    assert_eq!(load_from(&path), Some(expected));

    let legacy = serde_json::json!({
        "v": 1,
        "agent": "openai",
        "driver": "openai",
        "model": "gpt-5.6-sol",
        "reasoning_effort": "high"
    });
    std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert!(
        load_from(&path).is_some(),
        "v1 legacy strings remain readable"
    );

    let stale = preference("openai", "openai", "old", None);
    let mut stale_json = serde_json::to_value(stale).unwrap();
    stale_json["v"] = serde_json::json!(99);
    std::fs::write(&path, serde_json::to_vec(&stale_json).unwrap()).unwrap();
    assert_eq!(load_from(&path), None);
    std::fs::write(&path, vec![b'x'; MAX_BYTES as usize + 1]).unwrap();
    assert_eq!(load_from(&path), None);
    let _ = std::fs::remove_file(path);
}

#[test]
fn clear_is_idempotent() {
    let path = temp_path("clear");
    std::fs::write(&path, b"{}").unwrap();
    assert!(clear_path(&path).unwrap());
    assert!(!clear_path(&path).unwrap());
}

#[test]
fn restore_selects_only_an_available_exact_route() {
    let mut bag = Bag::for_reasoning_render_test();
    let beta = preference("beta", "practice", "model-b", None);
    assert!(apply(&mut bag, &beta, false, false).route);
    assert_eq!(bag.in_hand_label(), "beta");

    let mut bag = Bag::for_render_test(&[
        ("alpha", &[("model-a", true)]),
        ("beta", &[("model-b", false)]),
    ]);
    let unavailable = preference("beta", "practice", "model-b", None);
    assert_eq!(
        apply(&mut bag, &unavailable, false, false),
        RestoreResult::default()
    );
    assert_eq!(bag.in_hand_label(), "alpha");
}

#[test]
fn restore_respects_route_and_effort_pins() {
    let preferred = preference("openai", "gpt-5.6-sol", "gpt-5.6-sol", Some("high"));

    let mut bag = Bag::for_reasoning_render_test();
    let restored = apply(&mut bag, &preferred, false, false);
    assert!(restored.route);
    assert!(restored.effort);
    assert_eq!(bag.reasoning_effort().as_deref(), Some("high"));

    let mut pinned = Bag::for_reasoning_render_test();
    let restored = apply(&mut pinned, &preferred, true, true);
    assert_eq!(restored, RestoreResult::default());
    assert_eq!(pinned.reasoning_effort().as_deref(), Some("medium"));

    let mut differently_pinned = Bag::for_reasoning_render_test();
    let other_route = preference("beta", "practice", "model-b", Some("high"));
    let restored = apply(&mut differently_pinned, &other_route, true, false);
    assert_eq!(restored, RestoreResult::default());
    assert_eq!(
        differently_pinned.reasoning_effort().as_deref(),
        Some("medium"),
        "a stale route's effort must not leak onto the pinned model"
    );
}

#[test]
fn legacy_sota_moa_pin_does_not_override_the_concrete_brain_route() {
    assert!(!route_pin_is_concrete(std::ffi::OsStr::new("sota-moa")));
    assert!(!route_pin_is_concrete(std::ffi::OsStr::new(" SOTA-MOA ")));
    assert!(route_pin_is_concrete(std::ffi::OsStr::new("openai")));
    assert!(route_pin_is_concrete(std::ffi::OsStr::new("longcat")));
}

#[test]
fn snapshot_contains_only_selected_route_metadata() {
    let mut bag = Bag::for_reasoning_render_test();
    assert!(bag.select_route(0, 0));
    assert!(bag.set_reasoning_effort("high").is_some());
    let mut expected = preference("openai", "gpt-5.6-sol", "gpt-5.6-sol", Some("high"));
    expected.route_id = Some(crate::backplane::RouteId::chat(
        "openai",
        "gpt-5.6-sol",
        Some("high"),
    ));
    assert_eq!(snapshot(&bag), Some(expected));
}

#[test]
fn active_restore_uses_stable_route_id_while_legacy_keeps_model_strings() {
    let original = Bag::for_render_test(&[("alpha", &[("model-a", true)])]);
    let preference = snapshot(&original).unwrap();
    let refreshed = Bag::for_render_test(&[("alpha", &[("model-b", true)])]);
    let choice = refreshed.route_choices().iter().next().cloned().unwrap();
    assert!(matches_with_backplane(&choice, &preference, true));
    assert!(!matches_with_backplane(&choice, &preference, false));
}
