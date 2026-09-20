use super::*;

include!("atlas_adversarial_tests.rs");

#[test]
fn test_bounded_unicode() {
    let text = "mechanism real, recursive lane confirmed live → grind-dispatch worker next.";
    let res = bounded(text, 60);
    assert!(res.ends_with('…'));
    assert!(res.is_char_boundary(res.len()));
}

fn service(name: &str) -> (Arc<AtlasService>, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "angel-atlas-{name}-{}-{}",
        std::process::id(),
        now_ms()
    ));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    (AtlasService::open_in(&workspace, root.clone()), root)
}

fn source(id: &str) -> AtlasSource {
    AtlasSource {
        id: id.to_string(),
        kind: "test".to_string(),
        digest: digest(id),
        excerpt: Some("bounded evidence".to_string()),
        independent: true,
        influenced_by: None,
    }
}

fn no_exclusion() -> std::iter::Empty<&'static str> {
    std::iter::empty()
}

fn history_contents(history: &[crate::agent::club::ChatMsg]) -> impl Iterator<Item = &str> {
    history.iter().map(|message| message.content.as_ref())
}

#[test]
fn proposal_never_enters_lens_before_review() {
    let _guard = crate::tests::env_lock();
    let (service, root) = service("proposal");
    service
        .propose(
            AtlasKind::Fact,
            "cargo test validates the cockpit",
            Some(0.9),
            vec![source("src:1")],
        )
        .unwrap();
    assert_eq!(service.status().review_count, 1);
    assert!(
        service
            .build_lens("please run cargo test", no_exclusion())
            .is_none()
    );
    let id = service.list(AtlasLane::Review, None)[0].id.clone();
    service.accept(&id).unwrap();
    let lens = service
        .build_lens("please run cargo test", no_exclusion())
        .expect("accepted relevant item");
    assert!(lens.contains("cargo test validates"));
    assert!(lens.len() <= MAX_LENS_BYTES);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn status_cache_matches_fresh_recount_after_every_mutation_path() {
    let (service, root) = service("status-cache");
    let assert_fresh = |service: &AtlasService| {
        // Twice: first fill/refresh the cache, then serve from it.
        for status in [service.status(), service.status()] {
            assert_eq!(status.health, AtlasHealth::Healthy);
            assert_eq!(
                status.project_items,
                service.list(AtlasLane::Project, None).len()
            );
            assert_eq!(
                status.review_count,
                service.list(AtlasLane::Review, None).len()
            );
            assert_eq!(
                status.shared_items,
                service.list(AtlasLane::Shared, None).len()
            );
        }
    };
    assert_fresh(&service);
    let proposal = service
        .propose(
            AtlasKind::Fact,
            "a cached fact",
            None,
            vec![source("src:c")],
        )
        .unwrap();
    assert_fresh(&service);
    // Lifecycle flip only: item totals stay put while review_count moves.
    service.accept(&proposal.id).unwrap();
    assert_fresh(&service);
    let operator = service
        .add_operator(AtlasKind::Note, "operator note")
        .unwrap();
    assert_fresh(&service);
    service
        .revise(&operator.id, "revised operator note")
        .unwrap();
    assert_fresh(&service);
    service.reject(&proposal.id, "wrong").unwrap();
    assert_fresh(&service);
    service.undo().unwrap();
    assert_fresh(&service);
    service.challenge(&proposal.id, None).unwrap();
    assert_fresh(&service);
    let keeper = service
        .add_operator(AtlasKind::Procedure, "promote me")
        .unwrap();
    assert_fresh(&service);
    service
        .promote(&keeper.id, Some(&keeper.content_digest))
        .unwrap();
    assert_fresh(&service);
    service.tend().unwrap();
    assert_fresh(&service);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rejection_tombstone_prevents_resurrection_after_restart() {
    let (service, root) = service("tombstone");
    let item = service
        .propose(
            AtlasKind::Fact,
            "a rejected claim",
            None,
            vec![source("src:2")],
        )
        .unwrap();
    service.reject(&item.id, "incorrect").unwrap();
    let reopened = AtlasService::open_in(&service.workspace, root.clone());
    let error = reopened
        .propose(
            AtlasKind::Fact,
            "a rejected claim",
            None,
            vec![source("src:3")],
        )
        .unwrap_err();
    assert!(error.contains("tombstone"), "{error}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn revisions_supersede_instead_of_overwriting() {
    let (service, root) = service("revise");
    let old = service.add_operator(AtlasKind::Decision, "use A").unwrap();
    let new = service.revise(&old.id, "use B").unwrap();
    assert_ne!(old.id, new.id);
    assert_eq!(
        service.item(&old.id).unwrap().lifecycle,
        AtlasLifecycle::Superseded
    );
    assert_eq!(new.lifecycle, AtlasLifecycle::Active);
    assert!(
        new.links
            .iter()
            .any(|link| link.kind == AtlasLinkKind::Supersedes && link.target_id == old.id)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn malformed_and_foreign_snapshots_are_inert_and_not_overwritten() {
    let (service, root) = service("inert");
    if let Some(parent) = service.project_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&service.project_path, b"{ definitely not json").unwrap();
    let reopened = AtlasService::open_in(&service.workspace, root.clone());
    assert_eq!(reopened.status().health, AtlasHealth::Inert);
    assert!(
        reopened
            .add_operator(AtlasKind::Note, "must not overwrite")
            .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(&service.project_path).unwrap(),
        "{ definitely not json"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn foreign_unknown_and_oversized_records_stay_inert() {
    let (service, root) = service("foreign");
    if let Some(parent) = service.project_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let mut foreign = empty_project(Path::new("/foreign/repository"), "foreign-key");
    foreign.schema = PROJECT_SCHEMA.to_string();
    std::fs::write(
        &service.project_path,
        serde_json::to_vec_pretty(&foreign).unwrap(),
    )
    .unwrap();
    assert_eq!(
        AtlasService::open_in(&service.workspace, root.clone())
            .status()
            .health,
        AtlasHealth::Inert
    );

    let mut unknown = empty_project(&service.project_root, &service.project_key);
    unknown.schema = "angel-atlas-project/v999".to_string();
    std::fs::write(
        &service.project_path,
        serde_json::to_vec_pretty(&unknown).unwrap(),
    )
    .unwrap();
    assert_eq!(
        AtlasService::open_in(&service.workspace, root.clone())
            .status()
            .health,
        AtlasHealth::Inert
    );

    std::fs::write(
        &service.project_path,
        vec![b'x'; MAX_SNAPSHOT_BYTES as usize + 1],
    )
    .unwrap();
    assert_eq!(
        AtlasService::open_in(&service.workspace, root.clone())
            .status()
            .health,
        AtlasHealth::Inert
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn concurrent_writers_merge_under_the_snapshot_lock() {
    let (service, root) = service("concurrent");
    let mut joins = Vec::new();
    for index in 0..12 {
        let service = Arc::clone(&service);
        joins.push(std::thread::spawn(move || {
            service
                .add_operator(AtlasKind::Note, &format!("concurrent item {index}"))
                .unwrap();
        }));
    }
    for join in joins {
        join.join().unwrap();
    }
    let reopened = AtlasService::open_in(&service.workspace, root.clone());
    assert_eq!(reopened.list(AtlasLane::Project, None).len(), 12);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn same_basename_workspaces_get_distinct_nonrepo_records() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("atlas-basename-{}", now_ms()));
    let left = root.join("left/repo");
    let right = root.join("right/repo");
    std::fs::create_dir_all(&left).unwrap();
    std::fs::create_dir_all(&right).unwrap();
    let left_service = AtlasService::open_in(&left, root.join("store"));
    let right_service = AtlasService::open_in(&right, root.join("store"));
    assert_ne!(left_service.project_key, right_service.project_key);
    assert_ne!(left_service.project_path, right_service.project_path);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn shared_promotion_requires_exact_digest_confirmation() {
    let (service, root) = service("promote");
    let item = service
        .add_operator(AtlasKind::Procedure, "run the narrow test first")
        .unwrap();
    let request = service.promote(&item.id, None).unwrap();
    assert!(request.contains(&item.content_digest));
    service
        .promote(&item.id, Some(&item.content_digest))
        .unwrap();
    let shared = service.list(AtlasLane::Shared, None);
    assert_eq!(shared.len(), 1);
    assert_eq!(shared[0].scope, AtlasScope::Shared);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn lens_order_caps_and_irrelevance_are_deterministic() {
    let _guard = crate::tests::env_lock();
    let (service, root) = service("lens");
    for index in 0..9 {
        service
            .add_operator(
                AtlasKind::Fact,
                &format!("rust atlas retrieval fact number {index}"),
            )
            .unwrap();
    }
    let first = service
        .build_lens("rust atlas retrieval", no_exclusion())
        .unwrap();
    let second = service
        .build_lens("rust atlas retrieval", no_exclusion())
        .unwrap();
    assert_eq!(first, second);
    assert!(first.matches("\n- [").count() <= MAX_LENS_ITEMS);
    assert!(first.len() <= MAX_LENS_BYTES);
    assert!(
        service
            .build_lens("unrelated-zebra-quantum", no_exclusion())
            .is_none()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn lens_exclusion_drops_items_already_present_in_a_history_message() {
    let _guard = crate::tests::env_lock();
    let _atlas = crate::tests::TestEnvGuard::unset("ANGEL_ATLAS");
    let _lens = crate::tests::TestEnvGuard::unset("ANGEL_ATLAS_LENS");
    let (service, root) = service("lens-present");
    let content = "cargo test validates the cockpit atlas retrieval";
    service.add_operator(AtlasKind::Fact, content).unwrap();
    let query = "please run cargo test";
    let selected = service
        .build_lens(query, no_exclusion())
        .expect("relevant item is selected");
    assert!(selected.contains(content));

    let matching = [crate::agent::club::ChatMsg::assistant(content)];
    assert!(
        service
            .build_lens(query, history_contents(&matching))
            .is_none(),
        "an item whose content already appears in one history message stays out of the lens"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn lens_exclusion_does_not_false_match_a_needle_split_across_messages() {
    let _guard = crate::tests::env_lock();
    let _atlas = crate::tests::TestEnvGuard::unset("ANGEL_ATLAS");
    let _lens = crate::tests::TestEnvGuard::unset("ANGEL_ATLAS_LENS");
    let (service, root) = service("lens-split");
    // Needle straddles the old `"\\n".join(history)` boundary: the joined
    // blob would `contains` this item, but neither message does. That
    // cross-boundary hit was a false match, not intended exclusion.
    let content = "the cockpit\natlas retrieval";
    service.add_operator(AtlasKind::Fact, content).unwrap();
    let query = "atlas retrieval cockpit";
    let unexcluded = service
        .build_lens(query, no_exclusion())
        .expect("relevant item is selected");
    assert!(unexcluded.contains(content));

    let split = [
        crate::agent::club::ChatMsg::user("please run cargo test in the cockpit"),
        crate::agent::club::ChatMsg::assistant("atlas retrieval is ready"),
    ];
    let joined = split
        .iter()
        .map(|message| message.content.as_ref())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        joined.contains(content),
        "precondition: concatenating history would false-match across messages"
    );
    assert!(
        !split
            .iter()
            .any(|message| message.content.contains(content)),
        "precondition: no single message contains the item"
    );
    let split_lens = service
        .build_lens(query, history_contents(&split))
        .expect("split-across-messages needle must not exclude");
    assert_eq!(
        split_lens, unexcluded,
        "per-message membership keeps the same lens bytes when no source contains the item"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn tending_marks_old_active_items_stale_and_excludes_them() {
    let _guard = crate::tests::env_lock();
    let (service, root) = service("tend");
    let item = service
        .add_operator(AtlasKind::OpenThread, "old atlas release thread")
        .unwrap();
    service
        .mutate_project(|snapshot| {
            snapshot
                .items
                .iter_mut()
                .find(|candidate| candidate.id == item.id)
                .unwrap()
                .updated_ms = 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(service.tend().unwrap(), 1);
    let stale = service.item(&item.id).unwrap();
    assert!(stale.stale);
    assert_eq!(stale.injection, InjectionPolicy::Never);
    assert!(
        service
            .build_lens("atlas release thread", no_exclusion())
            .is_none()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn strict_clerk_contract_rejects_unknowns_and_candidate_overflow() {
    let (service, root) = service("clerk");
    let unknown =
        r#"{"schema":"angel-atlas-clerk/v1","action":"abstain","candidates":[],"extra":1}"#;
    assert!(service.parse_clerk_output(unknown).is_err());
    let overflow = json!({
        "schema": CLERK_SCHEMA,
        "action": "propose",
        "candidates": (0..4).map(|index| json!({
            "kind": "fact",
            "content": format!("fact {index}"),
            "sources": [source("src")]
        })).collect::<Vec<_>>()
    });
    assert!(
        service
            .parse_clerk_output(&overflow.to_string())
            .unwrap_err()
            .contains("at most three")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn persisted_harvest_processing_is_idempotent_and_review_only() {
    let (service, root) = service("harvest-idempotent");
    service.record_tool_receipt(
        "read_file",
        &json!({"path":"src/routes.rs"}),
        &Ok("route ids inspected".into()),
        None,
    );
    service
        .enqueue_harvest(
            "fix routing",
            "implemented stable ids",
            &["tests:passed".to_string()],
            &[],
        )
        .unwrap();
    let harvest = service.next_harvest().expect("queued harvest");
    let output = ClerkOutput {
        schema: CLERK_SCHEMA.to_string(),
        action: ClerkAction::Propose,
        candidates: vec![ClerkCandidate {
            kind: AtlasKind::Decision,
            content: "Routing preferences use stable route ids".to_string(),
            confidence: Some(0.9),
            sources: harvest.receipt_sources.clone(),
            merge_into: None,
        }],
    };
    assert_eq!(
        service
            .apply_clerk_output(&harvest.id, output.clone())
            .unwrap(),
        1
    );
    assert_eq!(service.apply_clerk_output(&harvest.id, output).unwrap(), 0);
    assert_eq!(service.harvest_queue_status().0, 0);
    let proposed = service.list(AtlasLane::Review, None);
    assert_eq!(proposed.len(), 1);
    assert_eq!(proposed[0].lifecycle, AtlasLifecycle::Proposed);
    assert_eq!(proposed[0].injection, InjectionPolicy::Never);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn influenced_receipt_cannot_confirm_originating_claim() {
    let (service, root) = service("influence");
    let source = AtlasSource {
        id: "tool:1".to_string(),
        kind: "tool-receipt".to_string(),
        digest: digest("receipt"),
        excerpt: None,
        independent: false,
        influenced_by: Some("run tests".to_string()),
    };
    let error = service
        .propose(
            AtlasKind::Fact,
            "run tests confirms the claim",
            None,
            vec![source],
        )
        .unwrap_err();
    assert!(error.contains("cannot independently"), "{error}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn end_to_end_review_lens_influence_contradiction_and_rejection() {
    let _guard = crate::tests::env_lock();
    let (service, root) = service("e2e");
    let proposal = service
        .propose(
            AtlasKind::Procedure,
            "Run the verifier before release.\n$ cargo test --quiet",
            Some(0.82),
            vec![source("evidence:initial")],
        )
        .unwrap();
    service.accept(&proposal.id).unwrap();
    let reviewed = service
        .revise(
            &proposal.id,
            "Run the independent verifier before release.\n$ cargo test --quiet",
        )
        .unwrap();
    let lens = service
        .build_lens("run the cargo verifier before release", no_exclusion())
        .unwrap();
    assert!(lens.contains(&reviewed.id));
    let influence = service
        .begin_tool_call("shell", &json!({"command": "cargo test --quiet"}))
        .expect("exact Atlas-suggested command is marked influenced");
    assert_eq!(influence.0, reviewed.id);

    let self_receipt = AtlasSource {
        id: "receipt:self".to_string(),
        kind: "tool-receipt".to_string(),
        digest: digest("self receipt"),
        excerpt: Some("tests passed".to_string()),
        independent: false,
        influenced_by: Some(reviewed.id.clone()),
    };
    assert!(
        service
            .propose(
                AtlasKind::Fact,
                "the verifier proves the procedure",
                Some(0.9),
                vec![self_receipt],
            )
            .is_err()
    );

    let contradiction = service
        .propose(
            AtlasKind::Fact,
            "independent evidence contradicts the release procedure",
            Some(0.95),
            vec![source("evidence:independent-contradiction")],
        )
        .unwrap();
    service
        .challenge(&reviewed.id, Some(&contradiction.id))
        .unwrap();
    assert!(
        service
            .build_lens("run the cargo verifier before release", no_exclusion())
            .is_none()
    );
    service.reject(&reviewed.id, "contradicted").unwrap();
    let reopened = AtlasService::open_in(&service.workspace, root.clone());
    assert_eq!(
        reopened.item(&reviewed.id).unwrap().lifecycle,
        AtlasLifecycle::Rejected
    );
    assert!(
        reopened
            .propose(
                AtlasKind::Procedure,
                "Run the independent verifier before release.\n$ cargo test --quiet",
                None,
                vec![source("evidence:resurrection")],
            )
            .is_err()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rollout_gate_and_dynamic_roles_fail_closed() {
    let metrics = AtlasPromotionMetrics {
        reviewed_lessons: 256,
        held_out: 64,
        kind_scope_macro_f1: 0.95,
        action_accuracy: 0.90,
        false_positive_rate: 0.02,
        source_precision: 0.98,
        canary_reviews: 100,
        scope_leaks: 0,
        rejection_regression_points: 2.0,
        schema_source_scope_safety: true,
    };
    assert!(clerk_promotion_ready(metrics));
    let routes = vec![
        AtlasModelCapability {
            route_id: "teacher-live".to_string(),
            parameter_billions: Some(30),
            atlas_adapter: false,
            available: true,
            training: false,
        },
        AtlasModelCapability {
            route_id: "clerk-live".to_string(),
            parameter_billions: Some(4),
            atlas_adapter: true,
            available: true,
            training: false,
        },
    ];
    let roles = resolve_model_roles(&routes, true);
    assert_eq!(roles.teacher.as_deref(), Some("teacher-live"));
    assert_eq!(roles.clerk.as_deref(), Some("clerk-live"));
    let mut training = routes;
    training[1].training = true;
    assert_eq!(resolve_model_roles(&training, true).clerk, None);
    training[0].training = true;
    assert_eq!(resolve_model_roles(&training, true).teacher, None);
    training.push(AtlasModelCapability {
        route_id: "teacher-replacement".to_string(),
        parameter_billions: Some(32),
        atlas_adapter: false,
        available: true,
        training: false,
    });
    assert_eq!(
        resolve_model_roles(&training, true).teacher.as_deref(),
        Some("teacher-replacement")
    );
}

#[test]
fn lens_replacement_is_one_post_task_harness_block() {
    let mut history = vec![
        crate::agent::club::ChatMsg::system("system"),
        crate::agent::club::ChatMsg::user("first task"),
        crate::agent::club::ChatMsg::harness(format!("{LENS_HEADER}\nold\n{LENS_SENTINEL}")),
        crate::agent::club::ChatMsg::assistant("answer"),
        crate::agent::club::ChatMsg::user("current task"),
    ];
    replace_lens_message(
        &mut history,
        Some(format!("{LENS_HEADER}\nnew\n{LENS_SENTINEL}")),
    );
    let lenses = history
        .iter()
        .filter(|message| is_lens_message(&message.content))
        .collect::<Vec<_>>();
    assert_eq!(lenses.len(), 1);
    assert_eq!(lenses[0].role, crate::agent::club::ChatRole::Harness);
    assert_eq!(
        history.last().unwrap().role,
        crate::agent::club::ChatRole::Harness
    );
    assert_eq!(
        history[history.len() - 2].role,
        crate::agent::club::ChatRole::User
    );
}

#[test]
fn model_tool_is_deferred_and_cannot_review_or_share() {
    let _guard = crate::tests::env_lock();
    let previous = std::env::var_os("ANGEL_ATLAS");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_ATLAS") };
    let workspace = std::env::temp_dir().join(format!("atlas-tool-{}", now_ms()));
    std::fs::create_dir_all(&workspace).unwrap();
    let mut registry =
        crate::agent::harness::ToolRegistry::with_team(workspace.clone(), Vec::new());
    assert!(registry.has_tool("atlas"));
    assert!(
        registry
            .defs()
            .iter()
            .all(|definition| definition.name != "atlas"),
        "Atlas must stay deferred until tool_search surfaces it"
    );
    registry.enable_tool_search();
    let search = registry
        .dispatch("tool_search", &json!({"query": "atlas"}))
        .unwrap();
    assert!(
        search.contains("- atlas [active next request]:"),
        "{search}"
    );
    let error = registry
        .dispatch("atlas", &json!({"action": "accept", "id": "atl_x"}))
        .unwrap_err();
    assert!(error.contains("atlas.action must be"), "{error}");
    match previous {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(value) => unsafe { std::env::set_var("ANGEL_ATLAS", value) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_ATLAS") },
    }
    let _ = std::fs::remove_dir_all(workspace);
}
