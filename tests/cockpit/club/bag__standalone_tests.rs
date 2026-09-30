#[cfg(test)]
#[test]
fn openai_codex_driver_preserves_resolved_slot_for_explicit_and_default_route() {
    struct OAuthSlot;
    impl Club for OAuthSlot {
        fn label(&self) -> &str {
            "openai"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Err("fixture only".into())
        }
    }
    let mut agents = vec![Agent {
        name: "openai".into(),
        slots: ["gpt-6-astra", "gpt-5.6-luna"]
            .into_iter()
            .map(|model| Slot {
                label: model.into(),
                club: Arc::new(OAuthSlot),
                available: Arc::new(AtomicBool::new(true)),
            })
            .collect(),
        active: 1,
    }];
    // Bag::standard supplies this same preference for an unset ANGEL_DRIVER.
    for _ in 0..2 {
        assert_eq!(resolve_driver(&mut agents, "openai"), Some(0));
        assert_eq!(agents[0].active, 1);
    }
    // An explicit model-name route remains selectable.
    assert_eq!(resolve_driver(&mut agents, "gpt-6-astra"), Some(0));
    assert_eq!(agents[0].active, 0);
}

#[cfg(test)]
#[test]
fn k3_driver_aliases_resolve_to_the_kimi_seat() {
    // The Kimi Code plan calls the model `k3`; the sota-link alias is `kimi`.
    // Every spelling an operator might pin must resolve to that seat — a miss
    // here is the silent smartest-available fallback seen on rig B.
    struct KimiSeat;
    impl Club for KimiSeat {
        fn label(&self) -> &str {
            "kimi"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Err("fixture only".into())
        }
        fn live_model_name(&self) -> Option<String> {
            Some("kimi-k3".into())
        }
    }
    let agents_fn = || {
        vec![Agent {
            name: "sota".into(),
            slots: vec![Slot {
                label: "kimi".into(),
                club: Arc::new(KimiSeat),
                available: Arc::new(AtomicBool::new(true)),
            }],
            active: 0,
        }]
    };
    for pref in ["k3", "kimi-k3", "moonshot", "kimi-code", "kimi"] {
        let mut agents = agents_fn();
        assert_eq!(
            resolve_driver(&mut agents, pref),
            Some(0),
            "ANGEL_DRIVER={pref} must resolve to the kimi seat"
        );
    }
}

#[test]
fn explicit_driver_startup_notice_names_missing_preference_and_selected_route() {
    let _lock = crate::tests::env_lock();
    let bag = Bag::practice_only();
    let _driver = crate::tests::TestEnvGuard::set("ANGEL_DRIVER", "missing-model");
    let notice = bag.driver_startup_notice().unwrap();
    assert!(
        notice.contains("ANGEL_DRIVER=\"missing-model\""),
        "{notice}"
    );
    assert!(
        notice.contains("does not match a configured route"),
        "{notice}"
    );
    assert!(
        notice.contains("selected startup route: practice / practice"),
        "{notice}"
    );
}

#[test]
fn explicit_driver_startup_notice_checks_requested_slot_without_mutating_selection() {
    struct FixtureSeat;
    impl Club for FixtureSeat {
        fn label(&self) -> &str {
            "fixture"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            panic!("startup diagnostics must not call a provider")
        }
        fn live_model_name(&self) -> Option<String> {
            Some("fallback-checkpoint".into())
        }
    }
    let _lock = crate::tests::env_lock();
    let mut bag = Bag::practice_only();
    bag.agents = vec![Agent {
        name: "fixture-box".into(),
        slots: ["wanted", "fallback"]
            .into_iter()
            .enumerate()
            .map(|(index, label)| Slot {
                label: label.into(),
                club: Arc::new(FixtureSeat),
                available: Arc::new(AtomicBool::new(index == 1)),
            })
            .collect(),
        active: 1,
    }];
    let _driver = crate::tests::TestEnvGuard::set("ANGEL_DRIVER", "wanted");
    assert!(bag.agents[0].available(), "the box is up via another slot");
    let notice = bag.driver_startup_notice().unwrap();
    assert!(
        notice.contains("ANGEL_DRIVER=\"wanted\" is unavailable at startup"),
        "{notice}"
    );
    assert!(
        notice.contains("fixture-box / fallback (fallback-checkpoint)"),
        "{notice}"
    );
    assert_eq!(bag.selected_route_indices(), (0, 1));

    bag.agents[0].slots[0]
        .available
        .store(true, Ordering::Relaxed);
    let notice = bag.driver_startup_notice().unwrap();
    assert!(notice.contains("was not selected at startup"), "{notice}");
    assert_eq!(bag.selected_route_indices(), (0, 1));
    bag.agents[0].active = 0;
    assert!(bag.driver_startup_notice().is_none());
}

#[test]
fn driver_startup_notice_is_silent_for_implicit_defaults_and_satisfied_pins() {
    let _lock = crate::tests::env_lock();
    let bag = Bag::practice_only();
    let _unset = crate::tests::TestEnvGuard::unset("ANGEL_DRIVER");
    assert!(bag.driver_startup_notice().is_none());
    let _empty = crate::tests::TestEnvGuard::set("ANGEL_DRIVER", "  ");
    assert!(bag.driver_startup_notice().is_none());
    let _satisfied = crate::tests::TestEnvGuard::set("ANGEL_DRIVER", "practice");
    assert!(bag.driver_startup_notice().is_none());
}
