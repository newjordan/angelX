use super::*;
#[test]
fn l01_calibration_cannot_install_an_idle_stop() {
    let _guard = crate::tests::env_lock();
    let _unset = crate::tests::TestEnvGuard::unset("ANGEL_TURN_IDLE_TIMEOUT_SECS");
    for model in ["grok-4.6", "unknown"] {
        assert_eq!(budgets(model, "GROK")["turn_idle_timeout_secs"], 0);
    }
    let _cap = crate::tests::TestEnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "17");
    assert_eq!(budgets("grok-4.6", "GROK")["turn_idle_timeout_secs"], 17);
}

#[test]
fn model_defaults_precedence_and_unknown() {
    assert_eq!(
        resolve(Some(0), Some(240), 45, "2026-09-08"),
        (0, "env".into())
    );
    assert_eq!(
        resolve(None, Some(240), 45, "2026-09-08"),
        (240, "table 2026-09-08".into())
    );
    assert_eq!(resolve(None, None::<u64>, 45, ""), (45, "provider".into()));
    assert!(
        !parse(EMBEDDED)
            .unwrap()
            .iter()
            .any(|e| e.model == "grok-4.7")
    );
}
#[test]
fn model_defaults_toml_override() {
    let _guard = crate::tests::env_lock();
    let path = std::env::current_dir()
        .unwrap()
        .join(format!(".model-defaults-{}.toml", std::process::id()));
    std::fs::write(&path, "date='2026-09-09'\n[[models]]\nmodel='grok-4.6'\ndefault_effort='high'\nreceipt='test'\nreason='test'\n").unwrap();
    let entries = load(Some(&path)).unwrap();
    std::fs::remove_file(&path).unwrap();
    let grok = entries.iter().find(|e| e.model == "grok-4.6").unwrap();
    assert_eq!(grok.default_effort.as_deref(), Some("high"));
    assert_eq!(grok.date, "2026-09-09");
    assert!(entries.iter().any(|e| e.model == "deepseek-v4-flash"));
    assert!(parse("[[models]]\nmodel='oops'\n").is_err());
}
#[test]
fn model_defaults_http_wire_and_explicit_precedence() {
    let _guard = crate::tests::env_lock();
    struct ResetEffortCache;
    impl Drop for ResetEffortCache {
        fn drop(&mut self) {
            super::super::http::resync_reasoning_effort_env_from_env();
        }
    }
    let _reset = ResetEffortCache;
    let _global = crate::tests::TestEnvGuard::unset("ANGEL_REASONING_EFFORT");
    let _driver = crate::tests::TestEnvGuard::unset("ANGEL_GROK_REASONING_EFFORT");
    super::super::http::resync_reasoning_effort_env_from_env();
    let club = super::super::HttpClub::new("grok", "http://127.0.0.1:1/v1", "grok-4.6", None);
    let body = club.build_body(&[], &[], false).unwrap();
    assert_eq!(body["reasoning_effort"], "low");
    use super::super::Club;
    assert_eq!(
        club.resolved_model_defaults()["reasoning_effort_source"],
        "table 2026-09-09"
    );
    let body = club
        .build_body_with_effort(&[], &[], false, Some("high"))
        .unwrap();
    assert_eq!(body["reasoning_effort"], "high");
    let _explicit = crate::tests::TestEnvGuard::set("ANGEL_GROK_REASONING_EFFORT", "high");
    super::super::http::resync_reasoning_effort_env_from_env();
    assert_eq!(
        club.build_body(&[], &[], false).unwrap()["reasoning_effort"],
        "high"
    );
}

#[test]
fn model_defaults_budget_recording() {
    let _guard = crate::tests::env_lock();
    let _stall = crate::tests::TestEnvGuard::set("ANGEL_STREAM_STALL_SECS", "0");
    let _global = crate::tests::TestEnvGuard::set("ANGEL_REASONING_EFFORT", "high");
    let _driver = crate::tests::TestEnvGuard::set("ANGEL_GROK_REASONING_EFFORT", "max");
    let b = budgets("grok-4.6", "ANGEL_GROK");
    assert_eq!(b["stream_stall_secs"], 0);
    assert_eq!(b["stream_stall_source"], "env");
    assert_eq!(b["reasoning_effort"], "max");
    assert_eq!(b["reasoning_effort_source"], "env");
}
