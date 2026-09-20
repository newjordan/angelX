use super::*;

#[test]
fn d06b_practice_requires_exact_opt_in() {
    for value in [None, Some("0"), Some("true"), Some(" 1")] {
        assert!(!practice_route_allowed("practice", value));
    }
    assert!(practice_route_allowed("practice", Some("1")));
    assert!(practice_route_allowed("openrouter", None));
    assert_eq!(
        interactive_practice_notice("practice"),
        Some(PRACTICE_NOTICE)
    );
    assert_eq!(interactive_practice_notice("openrouter"), None);
    assert!(PRACTICE_NOTICE.contains("interactive practice"));
    assert!(PRACTICE_NOTICE.contains("offline echo"));
}

#[test]
fn d06b_no_route_envelope() {
    let value = serde_json::to_value(harness::TaskJsonEnvelope::from_startup_failure(
        &harness::TaskCliArgs::default(),
        std::path::PathBuf::from("."),
        0,
        harness::TaskStartupStopReason::NoRoute,
        NO_ROUTE.into(),
    ))
    .unwrap();
    assert_eq!(value["status"], "error");
    assert_eq!(value["error"]["kind"], "no_route");
    assert_eq!(value["hops"], 0);
    for knob in [
        "ANGEL_DRIVER",
        "ANGEL_API_CLUBS",
        "ANGEL_<DRIVER>_KEY",
        "ANGEL_<DRIVER>_URL",
        "ANGEL_<DRIVER>_MODEL",
        "ANGEL_PRACTICE=1",
    ] {
        assert!(value["error"]["message"].as_str().unwrap().contains(knob));
    }
}
