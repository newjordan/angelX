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
