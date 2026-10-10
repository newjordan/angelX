use super::*;

#[test]
fn spawn_prompt_batches_only_tool_bearing_seats() {
    for grant in [Grant::None, Grant::ReadOnly, Grant::Code, Grant::Research] {
        let prompt = seat_system("reviewer", "", Formation::Panel, 2, grant);
        // A tool-bearing seat carries the batching page `⠺⠉⠁`; a bare seat keeps prose.
        assert_eq!(prompt.contains("⠺⠉⠁"), grant != Grant::None);
        assert!(!prompt.contains("code_mode"));
    }
}

#[test]
fn labyrinth_navigation_respects_scoped_workspace_read_grants() {
    let root = std::env::temp_dir().join(format!("angel-labyrinth-grants-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    crate::drive::labyrinth::initialize(&root).unwrap();
    crate::drive::labyrinth::observe_iteration(
        &root,
        "deli",
        "fixture",
        &["retained research lead".into()],
        &[],
    )
    .unwrap();
    let cargo = PinnedCargo::capture(&root);
    for grant in [Grant::None, Grant::ReadOnly, Grant::Code, Grant::Research] {
        let registry = grant.registry(&root, None, &cargo);
        assert_eq!(registry.has_tool("labyrinth"), grant != Grant::None);
        if grant != Grant::None {
            let reply = registry
                .dispatch("labyrinth", &serde_json::json!({"action":"plan"}))
                .unwrap();
            assert!(reply.contains("retained research lead"));
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}
