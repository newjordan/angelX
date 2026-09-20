use super::*;
use crate::agent::club::{ChatMsg, ChatRole};
use crate::agent::harness::ToolRegistry;

struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
    store: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "atlas-grounding-{}-{}-{}",
            std::process::id(),
            now_ms(),
            SEQ.fetch_add(1, AtomicOrdering::Relaxed)
        ));
        let workspace = root.join("workspace");
        std::fs::create_dir_all(workspace.join("src")).unwrap();
        std::fs::write(
            workspace.join("src/quartz.rs"),
            "pub fn persist_snapshot() -> bool { true }\n",
        )
        .unwrap();
        Self {
            store: root.join("store"),
            root,
            workspace,
        }
    }
    fn atlas(&self) -> Arc<AtlasService> {
        AtlasService::open_in(&self.workspace, self.store.clone())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn relation(id: &str) -> AtlasLink {
    AtlasLink {
        kind: AtlasLinkKind::DerivedFrom,
        target_id: id.into(),
    }
}

#[test]
fn native_observation_review_restart_linked_retrieval_and_actual_read() {
    let _guard = crate::tests::env_lock();
    let f = Fixture::new();
    let _dir = crate::tests::TestEnvGuard::set("ANGEL_ATLAS_DIR", &f.store.to_string_lossy());
    let _atlas = crate::tests::TestEnvGuard::set("ANGEL_ATLAS", "1");
    let _links = crate::tests::TestEnvGuard::set("ANGEL_ATLAS_LINKS", "1");
    let registry = ToolRegistry::with_team(f.workspace.clone(), Vec::new());
    let observed: AtlasItem = serde_json::from_str(
        &registry
            .dispatch(
                "atlas",
                &json!({
                    "action": "observe", "path": "src/quartz.rs", "symbol": "persist_snapshot"
                }),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(observed.epistemic, EpistemicState::Observed);
    assert_eq!(observed.authority, AtlasAuthority::ImportedEvidence);
    let source = observed.sources[0].clone();
    assert_eq!(
        source.digest,
        crate::knowledge::cut::sha256_hex(&std::fs::read(f.workspace.join(&source.id)).unwrap())
    );
    let atlas = registry.atlas();
    registry.dispatch("atlas", &json!({"action":"propose", "kind":"entity",
        "content":"Crystalline continuity", "sources":[source], "links":[relation(&observed.id)]
    })).unwrap();
    let concept = atlas.list(AtlasLane::Review, None).pop().unwrap();
    assert!(
        atlas
            .build_lens("crystalline", std::iter::empty())
            .is_none()
    );
    atlas.accept(&concept.id).unwrap();
    drop(atlas);
    drop(registry);
    let registry = ToolRegistry::with_team(f.workspace.clone(), Vec::new());
    let atlas = registry.atlas();
    let lens = atlas.build_lens("crystalline", std::iter::empty()).unwrap();
    assert!(
        lens.contains(&observed.id),
        "concept must retrieve code with no lexical overlap: {lens}"
    );
    assert!(lens.contains("src/quartz.rs:1"));
    assert!(lens.len() <= MAX_LENS_BYTES);
    let candidates = crate::agent::backplane::atlas_lens_candidates(&lens, atlas.project_key());
    let code = candidates
        .iter()
        .find(|c| c.source_id == format!("atlas:{}", observed.id))
        .unwrap();
    assert_eq!(code.epistemic_state, "observed");
    assert_eq!(
        code.authority,
        crate::agent::backplane::KnowledgeAuthority::Episodic
    );
    assert!(code.content.contains(&source.digest));
    // A real next action, chosen using the returned graph, crosses the native
    // file-tool boundary and reads the digest-bound implementation.
    let trace: Value = serde_json::from_str(
        &registry
            .dispatch("atlas", &json!({"action":"trace", "id":concept.id}))
            .unwrap(),
    )
    .unwrap();
    let node = trace["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == observed.id)
        .unwrap();
    let path = node["sources"][0]["id"].as_str().unwrap();
    let actual = registry
        .dispatch("read_file", &json!({"path":path}))
        .unwrap();
    assert!(actual.contains("persist_snapshot"));
    let _off = crate::tests::TestEnvGuard::set("ANGEL_ATLAS_LINKS", "0");
    let ablated = atlas.build_lens("crystalline", std::iter::empty()).unwrap();
    assert!(
        !ablated.contains(&observed.id),
        "lexical-only control cannot discover this code"
    );
    let foreign = f.root.join("foreign");
    std::fs::create_dir_all(&foreign).unwrap();
    assert!(
        AtlasService::open_in(&foreign, f.store.clone())
            .build_lens("crystalline", std::iter::empty())
            .is_none()
    );
}

#[test]
fn changed_deleted_and_replaced_sources_invalidate_derived_claims_durably() {
    let _guard = crate::tests::env_lock();
    for change in ["edit", "delete", "alias"] {
        let f = Fixture::new();
        let atlas = f.atlas();
        let code = atlas.observe_code("src/quartz.rs", None).unwrap();
        let concept = atlas
            .add_operator(AtlasKind::Entity, "Crystalline continuity")
            .unwrap();
        atlas
            .link_operator(&concept.id, relation(&code.id))
            .unwrap();
        let plan = atlas
            .add_operator(
                AtlasKind::Procedure,
                "Recovery runbook\n$ old-recovery-command",
            )
            .unwrap();
        atlas
            .link_operator(&plan.id, relation(&concept.id))
            .unwrap();
        assert!(atlas.build_lens("recovery", std::iter::empty()).is_some());
        assert!(
            atlas
                .begin_tool_call("shell", &json!({"command":"old-recovery-command"}))
                .is_some()
        );
        let path = f.workspace.join("src/quartz.rs");
        match change {
            "edit" => std::fs::write(&path, "pub fn recovered_snapshot() {}\n").unwrap(),
            "delete" => std::fs::remove_file(&path).unwrap(),
            _ => {
                std::fs::remove_file(&path).unwrap();
                std::fs::write(f.root.join("outside.rs"), "pub fn outside() {}\n").unwrap();
                std::os::unix::fs::symlink(f.root.join("outside.rs"), &path).unwrap();
            }
        }
        assert!(atlas.build_lens("recovery", std::iter::empty()).is_none());
        assert!(
            atlas
                .begin_tool_call("shell", &json!({"command":"old-recovery-command"}))
                .is_none()
        );
        let fresh = f.atlas();
        for id in [&code.id, &concept.id, &plan.id] {
            assert!(fresh.item(id).unwrap().stale, "{change}: {id}");
        }
        if change == "edit" {
            let corrected = fresh.observe_code("src/quartz.rs", None).unwrap();
            assert_ne!(code.id, corrected.id);
            assert!(
                fresh
                    .build_lens("recovered", std::iter::empty())
                    .unwrap()
                    .contains("recovered_snapshot")
            );
            assert!(
                fresh.build_lens("recovery", std::iter::empty()).is_none(),
                "re-observation must not bless old conclusions"
            );
        }
        if change == "delete" {
            std::fs::write(&path, "pub fn persist_snapshot() -> bool { true }\n").unwrap();
            let restored = fresh.observe_code("src/quartz.rs", None).unwrap();
            assert_ne!(restored.id, code.id);
            assert!(fresh.item(&concept.id).unwrap().stale);
        }
    }
}

#[test]
fn broker_revalidation_removes_revoked_evidence_even_with_prefix_caching() {
    let _guard = crate::tests::env_lock();
    let f = Fixture::new();
    let _dir = crate::tests::TestEnvGuard::set("ANGEL_ATLAS_DIR", &f.store.to_string_lossy());
    let _broker = crate::tests::TestEnvGuard::set("ANGEL_BACKPLANE", "1");
    let registry = ToolRegistry::with_team(f.workspace.clone(), Vec::new());
    let atlas = registry.atlas();
    let code = atlas.observe_code("src/quartz.rs", None).unwrap();
    let concept = atlas
        .add_operator(AtlasKind::Entity, "Crystalline continuity")
        .unwrap();
    atlas
        .link_operator(&concept.id, relation(&code.id))
        .unwrap();
    let mut history = vec![
        ChatMsg::system("stable system"),
        ChatMsg::user("crystalline"),
    ];
    crate::agent::harness::refresh_knowledge_broker(&registry, &mut history, 120_000, &[], true);
    assert!(
        history
            .iter()
            .any(|m| m.content.contains("persist_snapshot"))
    );
    assert_eq!(history.last().unwrap().role, ChatRole::Harness);
    let previous = history.clone();
    crate::agent::harness::refresh_knowledge_broker(&registry, &mut history, 120_000, &[], true);
    assert_eq!(
        serde_json::to_string(&history).unwrap(),
        serde_json::to_string(&previous).unwrap()
    );
    std::fs::write(
        f.workspace.join("src/quartz.rs"),
        "pub fn replacement() {}\n",
    )
    .unwrap();
    crate::agent::harness::refresh_knowledge_broker(&registry, &mut history, 120_000, &[], true);
    assert!(
        !history
            .iter()
            .any(|m| m.content.contains("persist_snapshot")
                || m.content.contains("Crystalline continuity"))
    );
    assert_eq!(history[0].content.as_ref(), "stable system");
    assert_eq!(history[1].content.as_ref(), "crystalline");
    // A different live session's revocation also supersedes the first handle's cache.
    let item = atlas
        .add_operator(AtlasKind::Fact, "crystalline revocable assertion")
        .unwrap();
    crate::agent::harness::refresh_knowledge_broker(&registry, &mut history, 120_000, &[], true);
    assert!(history.iter().any(|m| m.content.contains(&item.content)));
    f.atlas()
        .reject(&item.id, "independent contradiction")
        .unwrap();
    crate::agent::harness::refresh_knowledge_broker(&registry, &mut history, 120_000, &[], true);
    assert!(!history.iter().any(|m| m.content.contains(&item.content)));
}

#[test]
fn grounding_rejects_forged_digests_unsafe_paths_and_unreviewed_links() {
    let _guard = crate::tests::env_lock();
    let f = Fixture::new();
    let atlas = f.atlas();
    for path in [
        "../outside.rs",
        "off-limits/hidden.rs",
        ".env",
        "src/../../outside.rs",
    ] {
        assert!(atlas.observe_code(path, None).is_err(), "{path}");
    }
    let observed = atlas.observe_code("src/quartz.rs", None).unwrap();
    assert_eq!(
        observed.id,
        atlas.observe_code("src/quartz.rs", None).unwrap().id
    );
    let mut source = observed.sources[0].clone();
    source.digest = "forged".into();
    assert!(
        atlas
            .propose(AtlasKind::Fact, "bad claim", None, vec![source])
            .is_err()
    );
    assert!(
        atlas
            .promote(&observed.id, Some(&observed.content_digest))
            .is_err()
    );
    let tool = AtlasTool::new(atlas.clone());
    assert!(
        tool.call(&json!({"action":"link", "id":observed.id}))
            .is_err()
    );
    assert!(
        atlas
            .propose_linked(
                AtlasKind::Fact,
                "bad link",
                None,
                observed.sources.clone(),
                vec![relation("missing")]
            )
            .is_err()
    );
    let pending = atlas
        .propose_linked(
            AtlasKind::Fact,
            "pending claim",
            None,
            observed.sources,
            vec![relation(&observed.id)],
        )
        .unwrap();
    std::fs::write(f.workspace.join("src/quartz.rs"), "pub fn changed() {}\n").unwrap();
    assert!(atlas.accept(&pending.id).is_err());
}

#[test]
fn stale_cycles_do_not_loop_or_reactivate_claims() {
    let _guard = crate::tests::env_lock();
    let f = Fixture::new();
    let atlas = f.atlas();
    let a = atlas
        .add_operator(AtlasKind::Fact, "crystalline A")
        .unwrap();
    let b = atlas
        .add_operator(AtlasKind::Fact, "crystalline B")
        .unwrap();
    atlas.link_operator(&a.id, relation(&b.id)).unwrap();
    atlas.link_operator(&b.id, relation(&a.id)).unwrap();
    assert!(atlas.trace(&a.id).unwrap().contains(&b.id));
    atlas.challenge(&b.id, None).unwrap();
    assert!(
        atlas
            .build_lens("crystalline", std::iter::empty())
            .is_none()
    );
    assert!(f.atlas().item(&a.id).unwrap().stale);
}

#[test]
fn legacy_and_shadow_lenses_recheck_sources_between_tool_hops() {
    let _guard = crate::tests::env_lock();
    for mode in ["0", "shadow"] {
        let f = Fixture::new();
        let _dir = crate::tests::TestEnvGuard::set("ANGEL_ATLAS_DIR", &f.store.to_string_lossy());
        let _mode = crate::tests::TestEnvGuard::set("ANGEL_BACKPLANE", mode);
        let registry = ToolRegistry::with_team(f.workspace.clone(), Vec::new());
        registry
            .atlas()
            .observe_code("src/quartz.rs", None)
            .unwrap();
        let mut history = vec![ChatMsg::user("quartz")];
        crate::agent::harness::refresh_knowledge_broker(
            &registry,
            &mut history,
            120_000,
            &[],
            false,
        );
        assert!(
            history
                .iter()
                .any(|m| m.content.contains("persist_snapshot")),
            "{mode}"
        );
        std::fs::remove_file(f.workspace.join("src/quartz.rs")).unwrap();
        crate::agent::harness::refresh_knowledge_broker(
            &registry,
            &mut history,
            120_000,
            &[],
            false,
        );
        assert!(
            !history
                .iter()
                .any(|m| m.content.contains("persist_snapshot")),
            "{mode}"
        );
    }
}

#[test]
fn source_anchors_are_repository_relative_across_nested_sessions() {
    let _guard = crate::tests::env_lock();
    let f = Fixture::new();
    assert!(
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&f.workspace)
            .status()
            .unwrap()
            .success()
    );
    let nested = AtlasService::open_in(&f.workspace.join("src"), f.store.clone());
    let item = nested.observe_code("quartz.rs", None).unwrap();
    assert_eq!(item.sources[0].id, "src/quartz.rs");
    let root = f.atlas();
    assert_eq!(nested.project_key(), root.project_key());
    assert!(
        root.build_lens("quartz", std::iter::empty())
            .unwrap()
            .contains("persist_snapshot")
    );
}

#[test]
fn trace_and_source_read_budgets_are_explicit() {
    let _guard = crate::tests::env_lock();
    let f = Fixture::new();
    let atlas = f.atlas();
    let root = atlas
        .add_operator(AtlasKind::Entity, "root concept")
        .unwrap();
    for index in 0..30 {
        let child = atlas
            .add_operator(AtlasKind::Entity, &format!("child concept {index}"))
            .unwrap();
        atlas
            .link_operator(
                &child.id,
                AtlasLink {
                    kind: AtlasLinkKind::Related,
                    target_id: root.id.clone(),
                },
            )
            .unwrap();
    }
    let trace: Value = serde_json::from_str(&atlas.trace(&root.id).unwrap()).unwrap();
    assert_eq!(trace["nodes"].as_array().unwrap().len(), 24);
    assert_eq!(trace["truncated"], true);
    std::fs::write(f.workspace.join("huge.rs"), vec![b'x'; 512 * 1024 + 1]).unwrap();
    assert!(
        atlas
            .observe_code("huge.rs", None)
            .unwrap_err()
            .contains("512 KiB")
    );
    // A permitted lexical alias still may not redirect an observation.
    std::os::unix::fs::symlink(
        f.workspace.join("src/quartz.rs"),
        f.workspace.join("alias.rs"),
    )
    .unwrap();
    assert!(atlas.observe_code("alias.rs", None).is_err());
}

#[test]
fn harvested_receipts_keep_identity_influence_and_merge_links_after_restart() {
    let _guard = crate::tests::env_lock();
    let f = Fixture::new();
    let atlas = f.atlas();
    let target = atlas
        .add_operator(AtlasKind::Procedure, "crystalline procedure")
        .unwrap();
    atlas.record_tool_receipt(
        "read_file",
        &json!({"path":"src/quartz.rs"}),
        &Ok("inspected source".into()),
        None,
    );
    atlas.record_tool_receipt(
        "shell",
        &json!({"command":"suggested"}),
        &Ok("reported success".into()),
        Some((target.id.clone(), "suggested".into())),
    );
    atlas
        .enqueue_harvest(
            "crystalline",
            "done",
            &[format!("atlas:{}", target.id)],
            &["one actual inspection".into()],
        )
        .unwrap();
    let atlas = f.atlas();
    let pack = atlas.next_harvest().unwrap();
    assert!(pack.source_ids.contains(&format!("atlas:{}", target.id)));
    assert_eq!(pack.verifier_receipts[0], "one actual inspection");
    assert_eq!(pack.receipt_sources.len(), 2);
    assert_ne!(pack.receipt_sources[0].id, pack.receipt_sources[1].id);
    let make = |sources| ClerkOutput {
        schema: CLERK_SCHEMA.into(),
        action: ClerkAction::Merge,
        candidates: vec![ClerkCandidate {
            kind: AtlasKind::Procedure,
            content: "revised crystalline procedure".into(),
            confidence: None,
            sources,
            merge_into: Some(target.id.clone()),
        }],
    };
    let mut forged = pack.receipt_sources[1].clone();
    forged.independent = true;
    forged.influenced_by = None;
    assert!(
        atlas
            .apply_clerk_output(&pack.id, make(vec![forged]))
            .unwrap_err()
            .contains("cannot independently")
    );
    let mut forged = pack.receipt_sources[0].clone();
    forged.digest = "invented".into();
    assert!(
        atlas
            .apply_clerk_output(&pack.id, make(vec![forged]))
            .unwrap_err()
            .contains("not bound")
    );
    assert_eq!(
        atlas
            .apply_clerk_output(&pack.id, make(pack.receipt_sources))
            .unwrap(),
        1
    );
    let proposal = atlas.list(AtlasLane::Review, None).pop().unwrap();
    assert_eq!(proposal.links, vec![relation(&target.id)]);
    assert_eq!(proposal.lifecycle, AtlasLifecycle::Proposed);
    assert!(!proposal.sources[1].independent);
    assert_eq!(
        proposal.sources[1].influenced_by.as_deref(),
        Some(target.id.as_str())
    );
    assert_eq!(
        atlas.item(&target.id).unwrap().content,
        "crystalline procedure"
    );
}

#[test]
fn history_dedup_keeps_linked_evidence_but_stale_origins_cannot_seed_it() {
    let _guard = crate::tests::env_lock();
    let f = Fixture::new();
    std::fs::write(
        f.workspace.join("src/onyx.rs"),
        "pub fn independent_component() {}\n",
    )
    .unwrap();
    let atlas = f.atlas();
    let source = atlas.observe_code("src/quartz.rs", None).unwrap();
    let neighbor = atlas.observe_code("src/onyx.rs", None).unwrap();
    let concept = atlas
        .propose_linked(
            AtlasKind::Entity,
            "Crystalline continuity",
            None,
            source.sources,
            vec![AtlasLink {
                kind: AtlasLinkKind::Related,
                target_id: neighbor.id.clone(),
            }],
        )
        .unwrap();
    atlas.accept(&concept.id).unwrap();
    let lens = atlas
        .build_lens(&concept.content, std::iter::once(concept.content.as_str()))
        .unwrap();
    assert!(
        lens.contains(&neighbor.id),
        "a concept already in history must still find unseen code"
    );
    assert!(
        !lens.contains("Crystalline continuity"),
        "deduplicate the concept body itself"
    );
    std::fs::write(f.workspace.join("src/quartz.rs"), "pub fn changed() {}\n").unwrap();
    assert!(
        atlas
            .build_lens(&concept.content, std::iter::once(concept.content.as_str()))
            .is_none(),
        "an invalidated seed must not keep nominating an unrelated current source"
    );
    assert!(
        !atlas.item(&neighbor.id).unwrap().stale,
        "association is not evidence dependence"
    );
}
