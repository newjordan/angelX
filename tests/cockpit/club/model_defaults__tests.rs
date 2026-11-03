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
    let _guard = crate::tests::env_lock();
    assert_eq!(
        resolve(Some(0), Some(240), 45, "2026-09-08"),
        (0, "env".into())
    );
    assert_eq!(
        resolve(None, Some(240), 45, "2026-09-08"),
        (240, "table 2026-09-08".into())
    );
    assert_eq!(resolve(None, None::<u64>, 45, ""), (45, "provider".into()));
    let grok47 = parse(EMBEDDED)
        .unwrap()
        .into_iter()
        .find(|e| e.model == "grok-4.7")
        .expect("grok-4.7 carries a measured effort");
    assert_eq!(grok47.default_effort.as_deref(), Some("low"));
    // No calibration installs a clock on a live stream.
    let b = budgets("grok-4.7", "ANGEL_GROK");
    assert!(b.get("stream_stall_secs").is_none(), "{b}");
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
    let _global = crate::tests::TestEnvGuard::set("ANGEL_REASONING_EFFORT", "high");
    let _driver = crate::tests::TestEnvGuard::set("ANGEL_GROK_REASONING_EFFORT", "max");
    let b = budgets("grok-4.6", "ANGEL_GROK");
    assert_eq!(b["reasoning_effort"], "max");
    assert_eq!(b["reasoning_effort_source"], "env");
}

#[test]
fn calibration_hotload_requires_complete_atomic_publication() {
    let dir = std::env::temp_dir().join(format!(
        "angel-calibration-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("calibration.toml");
    let pending = dir.join("pending.toml");
    let write = |effort: &str| {
        format!(
            "date='2026-09-09'\n[[models]]\nmodel='grok-4.6'\ndefault_effort='{effort}'\nreceipt='test'\nreason='test'\n"
        )
    };
    let value = |entries: &Vec<Entry>| {
        entries
            .iter()
            .find(|e| e.model == "grok-4.6")
            .unwrap()
            .default_effort
            .clone()
            .unwrap()
    };
    std::fs::write(&path, write("low")).unwrap();
    let mut cache = CalibrationCache::new();
    let first = cache.snapshot(Some(&path));
    assert_eq!(value(&first), "low");
    std::fs::write(&path, "date='2026-09-09'\n").unwrap();
    assert_eq!(value(&cache.snapshot(Some(&path))), "low");
    std::fs::write(&pending, "[[models]]\nmodel='unfinished'\n").unwrap();
    std::fs::rename(&pending, &path).unwrap();
    assert_eq!(value(&cache.snapshot(Some(&path))), "low");
    std::fs::write(&path, write("high")).unwrap();
    assert_eq!(value(&cache.snapshot(Some(&path))), "low");
    std::fs::write(&pending, write("high")).unwrap();
    std::fs::rename(&pending, &path).unwrap();
    let second = cache.snapshot(Some(&path));
    assert_eq!(value(&second), "high");
    assert_eq!(value(&first), "low");
    std::fs::remove_file(&path).unwrap();
    assert_eq!(value(&cache.snapshot(Some(&path))), "high");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn http_request_pins_body_and_recorded_defaults_to_one_generation() {
    let _env = crate::tests::env_lock();
    let _global = crate::tests::TestEnvGuard::unset("ANGEL_REASONING_EFFORT");
    let _driver = crate::tests::TestEnvGuard::unset("ANGEL_GROK_REASONING_EFFORT");
    super::super::http::resync_reasoning_effort_env_from_env();
    let entries = Arc::new(parse("date='2099-01-01'\n[[models]]\nmodel='grok-4.6'\ndefault_effort='high'\nreceipt='pinned'\nreason='pinned'\n").unwrap());
    REQUEST_CALIBRATION.with(|slot| *slot.borrow_mut() = Some(entries));
    let _guard = RequestCalibrationGuard(true, std::marker::PhantomData);
    let club = super::super::HttpClub::new("grok", "http://127.0.0.1:1/v1", "grok-4.6", None);
    let body = club.build_body(&[], &[], false).unwrap();
    use super::super::Club;
    let defaults = club.resolved_model_defaults();
    assert_eq!(body["reasoning_effort"], "high");
    assert_eq!(defaults["reasoning_effort"], "high");
    assert_eq!(defaults["calibration_receipt"], "pinned");
}

#[test]
fn calibration_pins_are_isolated_between_concurrent_requests() {
    let _guard = crate::tests::env_lock();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = ["low", "high"].into_iter().map(|effort| {
        let barrier = Arc::clone(&barrier);
        std::thread::spawn(move || {
            let text = format!("date='2026-09-09'\n[[models]]\nmodel='grok-4.6'\ndefault_effort='{effort}'\nreceipt='test'\nreason='test'\n");
            REQUEST_CALIBRATION.with(|slot| *slot.borrow_mut() = Some(Arc::new(parse(&text).unwrap())));
            let _guard = RequestCalibrationGuard(true, std::marker::PhantomData);
            let _nested = pin_request();
            barrier.wait();
            assert_eq!(entry("grok-4.6").unwrap().default_effort.as_deref(), Some(effort));
        })
    }).collect();
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn preflight_and_wire_share_calibration_after_publication() {
    let _env = crate::tests::env_lock();
    let _global = crate::tests::TestEnvGuard::unset("ANGEL_REASONING_EFFORT");
    let _driver = crate::tests::TestEnvGuard::unset("ANGEL_GROK_REASONING_EFFORT");
    super::super::http::resync_reasoning_effort_env_from_env();
    let dir = std::env::temp_dir().join(format!(
        "angel-policy-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("calibration.toml");
    let pending = dir.join("pending.toml");
    let table = |effort: &str| {
        format!(
            "date='2099-01-01'\n[[models]]\nmodel='grok-4.6'\ndefault_effort='{effort}'\nreceipt='{effort}'\nreason='test'\n"
        )
    };
    std::fs::write(&path, table("low")).unwrap();
    let mut cache = CalibrationCache::new();
    let first = cache.snapshot(Some(&path));
    REQUEST_CALIBRATION.with(|slot| *slot.borrow_mut() = Some(first));
    let guard = RequestCalibrationGuard(true, std::marker::PhantomData);
    let club = super::super::HttpClub::new("grok", "http://127.0.0.1:1/v1", "grok-4.6", None);
    use super::super::Club;
    assert_eq!(
        club.route_identity().reasoning_effort.as_deref(),
        Some("low")
    );
    std::fs::write(&pending, table("high")).unwrap();
    std::fs::rename(&pending, &path).unwrap();
    let second = cache.snapshot(Some(&path));
    assert_eq!(
        second
            .iter()
            .find(|e| e.model == "grok-4.6")
            .unwrap()
            .default_effort
            .as_deref(),
        Some("high")
    );
    let _provider_guard = pin_request();
    let body = club.build_body(&[], &[], false).unwrap();
    let mut defaults = club.resolved_model_defaults();
    super::super::http::attribute_wire_effort(&mut defaults, &body);
    assert_eq!(body["reasoning_effort"], "low");
    assert_eq!(defaults["reasoning_effort"], "low");
    assert_eq!(defaults["calibration_receipt"], "low");
    let explicit = club
        .build_body_with_effort(&[], &[], false, Some("high"))
        .unwrap();
    super::super::http::attribute_wire_effort(&mut defaults, &explicit);
    assert_eq!(defaults["reasoning_effort"], "high");
    assert_eq!(defaults["reasoning_effort_source"], "wire");
    assert_eq!(defaults["calibration_receipt"], "low");
    drop(guard);
    std::fs::remove_dir_all(dir).unwrap();
}
