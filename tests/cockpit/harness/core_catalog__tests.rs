use super::*;
#[test]
fn embedded_catalog_matches_compiled_original_baseline() {
    // Captured after comparing every name, description and schema with the
    // independently compiled original definitions.
    assert_eq!(
        crate::knowledge::cut::sha256_hex(EMBEDDED.as_bytes()),
        "327bbe6700ef68cd2c9606f750517c8716adf188ca863b2ec36845c753410947"
    );
    assert_eq!(baseline().tools.len(), 60);
}

#[test]
fn pinned_reload_preserves_native_schemas_and_lean_descriptions() {
    let dir = std::env::temp_dir().join(format!(
        "angel-core-catalog-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("catalog.json");
    let mut data: Value = serde_json::from_str(EMBEDDED).unwrap();
    data["tools"]["write_file"]["description"] = "operator description one".into();
    std::fs::write(&path, serde_json::to_vec(&data).unwrap()).unwrap();
    let first = capture(Some(&path));
    data["tools"]["write_file"]["description"] = "operator description two".into();
    std::fs::write(&path, serde_json::to_vec(&data).unwrap()).unwrap();
    let second = capture(Some(&path));
    let mut a = baseline_def("write_file");
    first.overlay(&mut a);
    let mut b = baseline_def("write_file");
    second.overlay(&mut b);
    assert_eq!(a.description, "operator description one");
    assert_eq!(b.description, "operator description two");
    let mut lean = baseline_def("write_file");
    lean.description = "native lean".into();
    first.overlay(&mut lean);
    assert_eq!(lean.description, "native lean");
    data["tools"]["write_file"]["params"]["required"] = serde_json::json!([]);
    std::fs::write(&path, serde_json::to_vec(&data).unwrap()).unwrap();
    let invalid = capture(Some(&path));
    let mut c = baseline_def("write_file");
    invalid.overlay(&mut c);
    assert_eq!(c.description, baseline_def("write_file").description);
    assert!(read_override(Path::new("relative.json"), &baseline()).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cached_native_definitions_keep_turn_generations_separate_and_include_deferred() {
    use crate::agent::harness::{ToolRegistry, WriteFileTool};
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(WriteFileTool {
        root: std::env::temp_dir(),
    }));
    registry.enable_tool_search();
    let cached = registry.defs_arc();
    let mut one: Generation = serde_json::from_str(EMBEDDED).unwrap();
    let mut two = one.clone();
    one.tools.get_mut("write_file").unwrap().description = "one".into();
    two.tools.get_mut("write_file").unwrap().description = "two".into();
    let mut a = cached.as_ref().clone();
    let mut b = cached.as_ref().clone();
    registry.apply_core_catalog(&mut a, &one);
    registry.apply_core_catalog(&mut b, &two);
    assert_eq!(
        a.iter()
            .find(|d| d.name == "write_file")
            .unwrap()
            .description,
        "one"
    );
    assert_eq!(
        b.iter()
            .find(|d| d.name == "write_file")
            .unwrap()
            .description,
        "two"
    );
    assert_eq!(
        cached
            .iter()
            .find(|d| d.name == "write_file")
            .unwrap()
            .description,
        baseline_def("write_file").description
    );

    let mut deferred = ToolRegistry::new();
    deferred.register_deferred(Box::new(WriteFileTool {
        root: std::env::temp_dir(),
    }));
    deferred.enable_tool_search();
    let mut defs = vec![baseline_def("write_file")];
    deferred.apply_core_catalog(&mut defs, &one);
    assert_eq!(defs[0].description, "one");
}

#[test]
fn rejects_description_exceeding_native_budget() {
    let mut data: Value = serde_json::from_str(EMBEDDED).unwrap();
    let old = data["tools"]["write_file"]["description"]
        .as_str()
        .unwrap()
        .len();
    data["tools"]["write_file"]["description"] = "a".repeat(old + 1).into();
    let path = std::env::temp_dir().join(format!(
        "angel-core-catalog-length-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, serde_json::to_vec(&data).unwrap()).unwrap();
    assert!(read_override(&path, &baseline()).is_err());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn searchable_definition_overrides_are_allowed_with_native_schema() {
    let mut data: Value = serde_json::from_str(EMBEDDED).unwrap();
    data["tools"]["git_diff"]["description"] = "different".into();
    let path = std::env::temp_dir().join(format!(
        "angel-search-catalog-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, serde_json::to_vec(&data).unwrap()).unwrap();
    let generation = read_override(&path, &baseline()).unwrap();
    let mut def = baseline_def("git_diff");
    generation.overlay(&mut def);
    assert_eq!(def.description, "different");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn old_complete_catalog_overrides_remain_valid_and_partial_sets_fail() {
    let dir = std::env::temp_dir().join(format!(
        "angel-core-legacy-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("catalog.json");
    let mut legacy: Value = serde_json::from_str(EMBEDDED).unwrap();
    for name in V1_ADDITIONS
        .iter()
        .chain(V1_NEW_ADDITIONS)
        .chain(V1_DYNAMIC_ADDITIONS)
        .chain(V1_CAMPAIGN_ADDITIONS)
    {
        legacy["tools"].as_object_mut().unwrap().remove(*name);
    }
    legacy["tools"]["write_file"]["description"] = "legacy operator text".into();
    std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    let generation = read_override(&path, &baseline()).unwrap();
    assert_eq!(generation.tools.len(), 60);
    let mut def = baseline_def("write_file");
    generation.overlay(&mut def);
    assert_eq!(def.description, "legacy operator text");
    assert_eq!(
        generation.tools["knowledge_graph"].params,
        baseline().tools["knowledge_graph"].params
    );
    let mut partial = legacy.clone();
    partial["tools"].as_object_mut().unwrap().remove("git_diff");
    std::fs::write(&path, serde_json::to_vec(&partial).unwrap()).unwrap();
    assert!(read_override(&path, &baseline()).is_err());
    let mut unknown = legacy.clone();
    unknown["tools"]["unexpected"] = unknown["tools"]["git_diff"].clone();
    std::fs::write(&path, serde_json::to_vec(&unknown).unwrap()).unwrap();
    assert!(read_override(&path, &baseline()).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn previous_41_entry_override_stays_pinned_and_partial_upgrade_fails() {
    let path = std::env::temp_dir().join(format!(
        "angel-catalog-v41-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut old: Value = serde_json::from_str(EMBEDDED).unwrap();
    for name in V1_NEW_ADDITIONS
        .iter()
        .chain(V1_DYNAMIC_ADDITIONS)
        .chain(V1_CAMPAIGN_ADDITIONS)
    {
        old["tools"].as_object_mut().unwrap().remove(*name);
    }
    old["tools"]["list_dir"]["description"] = "previous generation".into();
    std::fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
    let generation = read_override(&path, &baseline()).unwrap();
    assert_eq!(generation.tools.len(), 60);
    let mut def = baseline_def("list_dir");
    generation.overlay(&mut def);
    assert_eq!(def.description, "previous generation");
    let current: Value = serde_json::from_str(EMBEDDED).unwrap();
    old["tools"]["lsp_hover"] = current["tools"]["lsp_hover"].clone();
    std::fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
    assert!(read_override(&path, &baseline()).is_err());
    std::fs::remove_file(path).unwrap();
}

fn test_generation(description: &str) -> Arc<Generation> {
    let mut data = (*baseline()).clone();
    data.tools.get_mut("git_diff").unwrap().description = description.into();
    Arc::new(data)
}

fn book_with(generation: Arc<Generation>) -> Arc<super::super::book::catalog::Catalog> {
    let mut book = super::super::book::catalog::Catalog::baseline();
    book.tool_generation = Some(generation);
    Arc::new(book)
}

#[test]
fn concurrent_turns_and_late_nested_seats_keep_distinct_generations() {
    use super::super::book::catalog::{self, Catalog};
    use std::sync::{Barrier, mpsc};
    let first = test_generation("narwhal first generation");
    let second = test_generation("dolphin second generation");
    let a = book_with(Arc::clone(&first));
    let b = book_with(Arc::clone(&second));
    let original = Catalog::baseline();
    assert_eq!(a.bytes(), original.bytes());
    assert_eq!(a.identity(), original.identity());
    let barrier = Arc::new(Barrier::new(2));
    std::thread::scope(|scope| {
        for (book, expected) in [(a, first), (b, second)] {
            let barrier = Arc::clone(&barrier);
            scope.spawn(move || {
                let _scope = catalog::enter(book);
                barrier.wait();
                assert!(Arc::ptr_eq(&current().unwrap(), &expected));
                let nested = bind_book(Arc::new(Catalog::baseline()));
                assert!(Arc::ptr_eq(
                    nested.tool_generation.as_ref().unwrap(),
                    &expected
                ));
            });
        }
    });
    let late_generation = test_generation("late retained generation");
    let parent = book_with(Arc::clone(&late_generation));
    let (ready_tx, ready_rx) = mpsc::channel();
    let (go_tx, go_rx) = mpsc::channel();
    std::thread::scope(|scope| {
        let worker_book = Arc::clone(&parent);
        let worker = scope.spawn(move || {
            let _scope = catalog::enter(worker_book);
            ready_tx.send(()).unwrap();
            go_rx.recv().unwrap();
            let child = bind_book(Arc::new(Catalog::baseline()));
            child.tool_generation.as_ref().cloned().unwrap()
        });
        ready_rx.recv().unwrap();
        drop(parent); // Parent returned or was cancelled; the late seat still owns its Arc.
        go_tx.send(()).unwrap();
        assert!(Arc::ptr_eq(&worker.join().unwrap(), &late_generation));
    });
}

#[test]
fn nested_code_mode_probe_inherits_the_generation() {
    use crate::agent::harness::{Tool, ToolRegistry};
    struct Probe;
    impl Tool for Probe {
        fn name(&self) -> &str {
            "git_diff"
        }
        fn def(&self) -> ToolDef {
            baseline_def("git_diff")
        }
        fn call(&self, _: &Value) -> Result<String, String> {
            let mut def = baseline_def("git_diff");
            current()
                .ok_or("missing tool generation")?
                .overlay(&mut def);
            Ok(def.description)
        }
    }
    let generation = test_generation("narwhal nested probe");
    let _book = super::super::book::catalog::enter(book_with(generation));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(Probe));
    let result = super::super::code_mode::run_code_mode_tool(
        &registry,
        &super::super::hooks::Hooks::default(),
        &serde_json::json!({"script":"return tool('git_diff', {});"}),
        None,
        None,
    );
    assert!(result.contains("narwhal nested probe"), "{result}");
}

#[test]
fn actual_turn_search_and_activated_definition_share_the_generation() {
    use crate::agent::club::{ChatMsg, ChatRole, Club, ClubReply, ToolCall};
    use crate::agent::harness::{GitDiffTool, ToolRegistry};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    struct SearchClub(AtomicUsize);
    impl Club for SearchClub {
        fn label(&self) -> &str {
            "catalog-search-test"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok("unused".into())
        }
        fn chat(&self, messages: &[ChatMsg], defs: &[ToolDef]) -> Result<ClubReply, String> {
            if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
                Ok(ClubReply::Calls(vec![ToolCall {
                    id: "catalog-search".into(),
                    name: "tool_search".into(),
                    args: serde_json::json!({"query":"narwhal", "limit":1}),
                }]))
            } else {
                assert!(
                    messages.iter().any(|msg| msg.role == ChatRole::Tool
                        && msg.content.contains("narwhal evidence"))
                );
                assert!(
                    defs.iter()
                        .any(|def| def.name == "git_diff" && def.description == "narwhal evidence")
                );
                Ok(ClubReply::Text("The search result is available.".into()))
            }
        }
    }
    let _book = super::super::book::catalog::enter(book_with(test_generation("narwhal evidence")));
    let mut registry = ToolRegistry::new();
    registry.register_deferred(Box::new(GitDiffTool {
        workspace: std::env::temp_dir(),
    }));
    registry.enable_tool_search();
    let mut history = vec![ChatMsg::user("find narwhal evidence")];
    let (events, _) = std::sync::mpsc::channel();
    let answer = super::super::run_turn(
        &SearchClub(AtomicUsize::new(0)),
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(3),
        &events,
    )
    .unwrap();
    assert!(answer.contains("search result"));
}

#[test]
fn independent_graph_root_binds_generation_without_changing_book_identity() {
    use super::super::book::catalog::{self, Catalog};
    let book = Catalog::baseline();
    let bytes = book.bytes();
    let identity = book.identity().to_string();
    let root = bind_book(Arc::new(book));
    assert!(root.tool_generation.is_some());
    assert_eq!(root.bytes(), bytes);
    assert_eq!(root.identity(), identity);
    let _scope = catalog::enter(root.clone());
    let child = bind_book(Arc::new(Catalog::baseline()));
    assert!(Arc::ptr_eq(
        root.tool_generation.as_ref().unwrap(),
        child.tool_generation.as_ref().unwrap()
    ));
}

#[test]
fn outer_turn_owner_carries_inherited_tool_generation() {
    use super::super::book::catalog::{self, Catalog};
    use std::sync::atomic::AtomicBool;
    let generation = test_generation("outer turn generation");
    let parent = book_with(Arc::clone(&generation));
    let _scope = catalog::enter(parent);
    let mut empty: Vec<crate::agent::club::ChatMsg> = Vec::new();
    let book = bind_book(catalog::bind_turn(&mut empty).unwrap());
    let cancel = AtomicBool::new(false);
    let owner = &cancel as *const AtomicBool as usize;
    let _owner = catalog::publish_owner(owner, &book);
    let published = catalog::for_owner(owner).unwrap();
    assert!(Arc::ptr_eq(
        published.tool_generation.as_ref().unwrap(),
        &generation
    ));
    assert_eq!(published.bytes(), Catalog::baseline().bytes());
}

#[test]
fn identical_complete_and_legacy_overrides_reuse_embedded_generation() {
    let path = std::env::temp_dir().join(format!(
        "angel-catalog-equal-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, EMBEDDED).unwrap();
    assert!(Arc::ptr_eq(&capture(Some(&path)), &baseline()));
    let mut legacy: Value = serde_json::from_str(EMBEDDED).unwrap();
    for name in V1_ADDITIONS
        .iter()
        .chain(V1_NEW_ADDITIONS)
        .chain(V1_DYNAMIC_ADDITIONS)
        .chain(V1_CAMPAIGN_ADDITIONS)
    {
        legacy["tools"].as_object_mut().unwrap().remove(*name);
    }
    std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert!(Arc::ptr_eq(&capture(Some(&path)), &baseline()));
    legacy["tools"]["git_diff"]["description"] = "changed".into();
    std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert!(!Arc::ptr_eq(&capture(Some(&path)), &baseline()));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn native_limit_changes_do_not_disable_description_reload_or_change_other_schema() {
    let mut generation = (*baseline()).clone();
    generation.tools.get_mut("grep").unwrap().description = "new search guide".into();
    let mut def = baseline_def("grep");
    *def.params
        .pointer_mut("/properties/paths/maxItems")
        .unwrap() = serde_json::json!(7);
    generation.overlay(&mut def);
    assert_eq!(def.description, "new search guide");
    assert_eq!(def.params["properties"]["paths"]["maxItems"], 7);
    let mut altered = baseline_def("grep");
    altered.params["required"] = serde_json::json!([]);
    generation.overlay(&mut altered);
    assert_eq!(altered.description, baseline_def("grep").description);
}

#[test]
fn previous_50_entry_override_stays_valid() {
    let path = std::env::temp_dir().join(format!(
        "angel-catalog-v50-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut old: Value = serde_json::from_str(EMBEDDED).unwrap();
    for name in V1_DYNAMIC_ADDITIONS.iter().chain(V1_CAMPAIGN_ADDITIONS) {
        old["tools"].as_object_mut().unwrap().remove(*name);
    }
    old["tools"]["git_diff"]["description"] = "v50 custom".into();
    std::fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
    let generation = read_override(&path, &baseline()).unwrap();
    assert_eq!(generation.tools.len(), 60);
    let mut def = baseline_def("git_diff");
    generation.overlay(&mut def);
    assert_eq!(def.description, "v50 custom");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn every_native_limit_mask_preserves_bounds_and_rejects_unrelated_changes() {
    for (name, pointers) in [
        (
            "grep",
            &[
                "/properties/paths/maxItems",
                "/properties/skip_files/maxItems",
                "/properties/context/maximum",
            ][..],
        ),
        ("defs", &["/properties/names/maxItems"][..]),
        (
            "todo",
            &[
                "/properties/text/maxLength",
                "/properties/items/maxItems",
                "/properties/items/items/maxLength",
            ][..],
        ),
        (
            "handoff",
            &["/properties/note/minLength", "/properties/note/maxLength"][..],
        ),
        (
            "semantic_read",
            &[
                "/properties/paths/maxItems",
                "/properties/max_results/maximum",
            ][..],
        ),
        ("science_search", &["/properties/query/maxLength"][..]),
        ("repo_search", &["/properties/query/maxLength"][..]),
    ] {
        let mut generation = (*baseline()).clone();
        generation.tools.get_mut(name).unwrap().description = "updated".into();
        let mut native = baseline_def(name);
        for pointer in pointers {
            *native.params.pointer_mut(pointer).unwrap() = serde_json::json!(3);
        }
        generation.overlay(&mut native);
        assert_eq!(native.description, "updated", "{name}");
        for pointer in pointers {
            assert_eq!(
                native.params.pointer(pointer).unwrap(),
                &serde_json::json!(3)
            );
        }
        let mut invalid = baseline_def(name);
        *invalid.params.pointer_mut(pointers[0]).unwrap() = serde_json::json!("3");
        generation.overlay(&mut invalid);
        assert_eq!(
            invalid.description,
            baseline_def(name).description,
            "{name}"
        );
        let mut invalid = baseline_def(name);
        *invalid.params.pointer_mut(pointers[0]).unwrap() = Value::Null;
        generation.overlay(&mut invalid);
        assert_eq!(
            invalid.description,
            baseline_def(name).description,
            "{name}"
        );
        let mut invalid = baseline_def(name);
        invalid.params["unrelated"] = serde_json::json!(true);
        generation.overlay(&mut invalid);
        assert_eq!(
            invalid.description,
            baseline_def(name).description,
            "{name}"
        );
    }
}

#[test]
fn dynamic_tool_definitions_match_compiled_original_goldens() {
    use crate::agent::harness::Tool;
    let root = std::env::temp_dir();
    let defs = [
        (
            crate::agent::tools::nav::GrepTool { root: root.clone() }.def(),
            "29b0c5d44504f41a1b7aab4c35d4db863c0b66a3289a025107c00da544d8b103",
        ),
        (
            crate::agent::tools::nav::DefsTool { root: root.clone() }.def(),
            "17d1d099b49513423388c5790fc136b1877e85182295fc1085af95defde85e8a",
        ),
        (
            crate::agent::tools::plan::TodoTool::new().def(),
            "e236d58d3a1496eed8f95d434f6c3a2a37e146d85224152cf538e0dcd5a09a18",
        ),
        (
            crate::agent::tools::plan::HandoffTool::new(&root).def(),
            "6ab46989a4324c499d846c5e7d24f9e5dfe174e1cfa58bd7640db098e7829b2c",
        ),
        (
            crate::agent::tools::embed::catalog_current_semantic_read(),
            "c838b808e1b8da1a91a33c120c9cb558602776202e0210146a910c2b7810904a",
        ),
        (
            crate::agent::tools::science::ScienceTool.def(),
            "df9aa23469f68cd1a0da15653b9a769a880ce7d0a6ff68f7a69a48c2210fbbb6",
        ),
        (
            crate::agent::tools::repos::RepoSearchTool.def(),
            "2d14c106685f29e80bbe5dcdae74071c6531459262974b1997b15657074e7c4c",
        ),
    ];
    for (def, expected) in defs {
        let name = def.name.clone();
        let canonical =
            serde_json::json!({"name":def.name,"description":def.description,"params":def.params});
        assert_eq!(
            crate::knowledge::cut::sha256_hex(&serde_json::to_vec(&canonical).unwrap()),
            expected,
            "{name}"
        );
    }
}

#[test]
fn campaign_definitions_match_compiled_original_goldens_and_keep_native_schema() {
    let spec = crate::drive::labyrinth::entry::spec_schema();
    assert_eq!(
        crate::knowledge::cut::sha256_hex(&serde_json::to_vec(&spec).unwrap()),
        "a6f5f9d17d7f848f83ddc37d6dc9dcdcd98a5cddd7e474b3aa45aae100f4f750"
    );
    let research = crate::agent::tools::loop_research::catalog_current_loop_research();
    let canonical = serde_json::json!({"name":research.name,"description":research.description,"params":research.params});
    assert_eq!(
        crate::knowledge::cut::sha256_hex(&serde_json::to_vec(&canonical).unwrap()),
        "d6d73baff8800b5777727a611e0f2731e7afc950fdfca24e09d51e1587a39255"
    );
    let campaign = crate::agent::tools::labyrinth_campaign::catalog_current_labyrinth_campaign();
    assert_eq!(
        campaign.description,
        crate::agent::harness::book::labyrinth_campaign::CAMPAIGN.cells()
    );
    assert_eq!(
        crate::knowledge::cut::sha256_hex(&serde_json::to_vec(&campaign.params).unwrap()),
        "07865905bcd0cdcae95f937d800cdcc30abdbe8aac1cd0441f7c12c59fdb973a"
    );
    let mut generation = (*baseline()).clone();
    generation
        .tools
        .get_mut("loop_research")
        .unwrap()
        .description = "updated research guidance".into();
    let mut def = crate::agent::tools::loop_research::catalog_current_loop_research();
    def.params["properties"]["campaign_spec"]["properties"]["extra"] =
        serde_json::json!({"type":"string"});
    generation.overlay(&mut def);
    assert_eq!(def.description, "updated research guidance");
    assert!(
        def.params["properties"]["campaign_spec"]["properties"]
            .get("extra")
            .is_some()
    );
}

#[test]
fn previous_57_entry_override_and_native_campaign_description_rules() {
    let path = std::env::temp_dir().join(format!(
        "angel-catalog-v57-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut old: Value = serde_json::from_str(EMBEDDED).unwrap();
    for name in V1_CAMPAIGN_ADDITIONS {
        old["tools"].as_object_mut().unwrap().remove(*name);
    }
    std::fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
    let generation = read_override(&path, &baseline()).unwrap();
    assert_eq!(generation.tools.len(), 60);
    let mut full: Value = serde_json::from_str(EMBEDDED).unwrap();
    full["tools"]["labyrinth_campaign"]["description"] = "other".into();
    std::fs::write(&path, serde_json::to_vec(&full).unwrap()).unwrap();
    assert!(read_override(&path, &baseline()).is_err());
    std::fs::remove_file(path).unwrap();
}
