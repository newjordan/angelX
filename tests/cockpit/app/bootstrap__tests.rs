use super::*;

struct TestClub(&'static str);

impl Club for TestClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok(String::new())
    }

    fn label(&self) -> &str {
        self.0
    }
}

#[test]
fn specialist_labels_skip_driver_and_practice_agent() {
    let roster: Vec<Arc<dyn Club>> = vec![
        Arc::new(TestClub("swarm")),
        Arc::new(TestClub("atlas")),
        Arc::new(TestClub("practice")),
    ];
    assert_eq!(specialist_labels(&roster, "swarm"), vec!["atlas"]);
}

#[test]
fn capabilities_block_names_gates_and_absences() {
    assert!(path_has("sh"), "sh must be on PATH");
    assert!(!path_has("angel-definitely-not-a-real-binary"));

    let block = capabilities_block(&["grok".to_string(), "openai".to_string()]);
    assert!(block.contains("On PATH:"));
    assert!(
        block.contains("grok, openai")
            && block.contains("ANGEL_ALLOW_SOTA_DELEGATE=0")
            && block.contains("ANGEL_ALLOW_SOTA_DELEGATE=1"),
        "withheld SOTA clubs must name the opt-out and how to restore: {block}"
    );
    assert!(
        block.contains("no image-generation tool"),
        "the no-invented-capabilities line must be present: {block}"
    );

    let block = capabilities_block(&[]);
    assert!(
        !block.contains("ANGEL_ALLOW_SOTA_DELEGATE"),
        "no gated-club line when nothing is withheld: {block}"
    );
}

#[test]
fn delegation_roster_includes_sota_when_allowed() {
    let roster: Vec<Arc<dyn Club>> = vec![
        Arc::new(TestClub("gemma")),
        Arc::new(TestClub("openai")),
        Arc::new(TestClub("practice")),
    ];
    // Opt-out path: local-only fleet.
    let labels: Vec<String> = filter_delegation_roster(roster, false)
        .iter()
        .map(|club| club.label().to_string())
        .collect();
    assert!(labels.contains(&"gemma".to_string()));
    assert!(labels.contains(&"practice".to_string()));
    assert!(!labels.contains(&"openai".to_string()));

    let roster: Vec<Arc<dyn Club>> = vec![
        Arc::new(TestClub("gemma")),
        Arc::new(TestClub("openai")),
        Arc::new(TestClub("practice")),
    ];
    // Default product path: SOTA seats are full agents.
    let labels: Vec<String> = filter_delegation_roster(roster, true)
        .iter()
        .map(|club| club.label().to_string())
        .collect();
    assert!(labels.contains(&"openai".to_string()));
}

#[test]
fn shared_workspace_bootstrap_keeps_prompt_and_registry_skill_catalogs_identical() {
    let _env = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-single-scan-bootstrap-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let skills_dir = root.join("skills");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&skills_dir).unwrap();
    std::fs::write(
        skills_dir.join("single-scan-proof.md"),
        "Use the shared bootstrap catalog.",
    )
    .unwrap();
    let skills_dir_text = skills_dir.to_string_lossy();
    let _skills = crate::tests::TestEnvGuard::set("ANGEL_SKILLS_DIR", &skills_dir_text);
    let bag = Bag::practice_for_test();

    let (shared_history, shared_registry) =
        build_history_and_registry(&bag, "shared-session", root.clone());
    let standalone_history = build_history(&bag, &root);
    let standalone_registry = build_registry(&bag, "standalone-session", root.clone());

    assert_eq!(
        serde_json::to_value(&shared_history).unwrap(),
        serde_json::to_value(&standalone_history).unwrap(),
    );
    let tool_names = |registry: &ToolRegistry| {
        registry
            .defs()
            .into_iter()
            .map(|definition| definition.name)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        tool_names(&shared_registry),
        tool_names(&standalone_registry)
    );
    assert!(
        shared_history
            .iter()
            .any(|message| is_workspace_context(message)
                && message.content.contains("single-scan-proof")),
        "{shared_history:?}"
    );
    let loaded = shared_registry
        .dispatch("skill", &serde_json::json!({"name": "single-scan-proof"}))
        .unwrap();
    assert!(loaded.contains("shared bootstrap catalog"), "{loaded}");

    let _ = std::fs::remove_dir_all(root);
}
