use super::*;

fn route(model: &str) -> RouteIdentity {
    RouteIdentity {
        driver: "fixture".into(),
        model: Some(model.into()),
        reasoning_effort: None,
    }
}

#[test]
fn catalog_binding_survives_save_and_compaction_without_changing_wire() {
    let digest = "a".repeat(64);
    let mut history = vec![ChatMsg::system("system"), ChatMsg::user("research")];
    let wire = crate::agent::club::messages_to_json(&history, false);
    bind_generation(&mut history, &digest).unwrap();
    assert_eq!(crate::agent::club::messages_to_json(&history, false), wire);
    assert_eq!(generation_digest(&history), Some(digest.as_str()));
    let saved = serde_json::to_vec(&history).unwrap();
    let restored: Vec<ChatMsg> = serde_json::from_slice(&saved).unwrap();
    assert_eq!(generation_digest(&restored), Some(digest.as_str()));
    let mut summary = ChatMsg::harness("summary without any book signals");
    summary.book_memory = compaction_memory(&restored);
    let mut compacted = vec![summary, ChatMsg::user("continue")];
    assert_eq!(generation_digest(&compacted), Some(digest.as_str()));
    let mut state = Continuity::new(&compacted);
    state.prepare(&mut compacted, &route("a"));
    assert_eq!(generation_digest(&compacted), Some(digest.as_str()));
}

#[test]
fn catalog_binding_refuses_invalid_or_different_generations() {
    let mut history = vec![ChatMsg::user("task")];
    assert!(bind_generation(&mut history, "not-a-digest").is_err());
    let a = "a".repeat(64);
    let b = "b".repeat(64);
    bind_generation(&mut history, &a).unwrap();
    bind_generation(&mut history, &a).unwrap();
    assert!(bind_generation(&mut history, &b).is_err());
    assert_eq!(generation_digest(&history), Some(a.as_str()));
    assert!(bind_generation(&mut [ChatMsg::system("only system")], &a).is_err());
}

#[test]
fn saved_session_recovers_its_exact_published_generation() {
    let _guard = crate::tests::env_lock();
    let workspace = crate::tests::TestGitWorkspace::new("book-generation-resume");
    let home = workspace.path().join("home");
    let sessions = workspace.path().join("sessions");
    std::fs::create_dir(&home).unwrap();
    let store = home.join(".angelX/book-generations");
    let _book_store = super::super::generation_store::TestRootGuard::enter(store.clone());
    let _sessions =
        crate::tests::TestEnvGuard::set("ANGEL_SESSION_DIR", sessions.to_str().unwrap());
    let original = super::super::catalog::Catalog::baseline().bytes();
    let edited = br#"{"schema":"angel.book-catalog/v1","chapter":"edited"}"#;
    let digest = super::super::generation_store::publish(&original).unwrap();
    let mut history = vec![ChatMsg::system("system"), ChatMsg::user("task")];
    let session = crate::knowledge::session::Session::at_for(
        sessions,
        "generation-fixture".into(),
        workspace.path(),
    );
    session.save(&history).unwrap();
    // Exit checkpoint reuse must notice a change to local-only book metadata.
    bind_generation(&mut history, &digest).unwrap();
    session.checkpoint_for_exit(&history).unwrap();
    assert_ne!(
        super::super::generation_store::publish(edited).unwrap(),
        digest
    );
    let restored =
        crate::knowledge::session::load_for("generation-fixture", workspace.path()).unwrap();
    let saved_digest = generation_digest(&restored).unwrap();
    assert_eq!(
        super::super::generation_store::load(saved_digest).unwrap(),
        original
    );
    std::fs::write(store.join(format!("{digest}.json")), b"corrupted").unwrap();
    let mut restored =
        crate::knowledge::session::load_for("generation-fixture", workspace.path()).unwrap();
    let error = super::super::catalog::bind_turn(&mut restored).unwrap_err();
    assert!(error.contains("corrupt"), "{error}");
}

#[test]
fn legacy_system_only_session_loads_without_a_writable_generation_store() {
    let _guard = crate::tests::env_lock();
    let workspace = crate::tests::TestGitWorkspace::new("book-legacy-empty");
    let sessions = workspace.path().join("sessions");
    let home_file = workspace.path().join("home-file");
    std::fs::write(&home_file, b"not a directory").unwrap();
    let unavailable = home_file.join("book-generations");
    let _book_store = super::super::generation_store::TestRootGuard::enter(unavailable);
    let _sessions =
        crate::tests::TestEnvGuard::set("ANGEL_SESSION_DIR", sessions.to_str().unwrap());
    let session = crate::knowledge::session::Session::at_for(
        sessions,
        "legacy-system".into(),
        workspace.path(),
    );
    session.save(&[ChatMsg::system("system")]).unwrap();
    let history = crate::knowledge::session::load_for("legacy-system", workspace.path()).unwrap();
    assert!(generation_digest(&history).is_none());
}

#[test]
fn legacy_session_binds_the_embedded_baseline_not_live_edits() {
    let _guard = crate::tests::env_lock();
    let workspace = crate::tests::TestGitWorkspace::new("book-legacy-baseline");
    let home = workspace.path().join("home");
    let sessions = workspace.path().join("sessions");
    let resources = workspace.path().join("resources");
    let chapter_dir = resources.join("cockpit/book");
    std::fs::create_dir(&home).unwrap();
    std::fs::create_dir_all(&chapter_dir).unwrap();
    let shipped_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("book");
    for entry in std::fs::read_dir(&shipped_dir).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            std::fs::copy(entry.path(), chapter_dir.join(entry.file_name())).unwrap();
        }
    }
    let shipped = std::fs::read_to_string(shipped_dir.join("gh_navigation.tsv")).unwrap();
    let edited = format!("{} edited\n", shipped.trim_end());
    std::fs::write(
        chapter_dir.join("gh_navigation.tsv"),
        &edited,
    )
    .unwrap();
    let manifest_path = chapter_dir.join("catalog-manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    manifest["chapters"]["gh_navigation.tsv"]["sha256"] =
        crate::knowledge::cut::sha256_hex(edited.as_bytes()).into();
    let mut bytes = serde_json::to_vec(&manifest).unwrap();
    bytes.push(b'\n');
    std::fs::write(manifest_path, bytes).unwrap();
    let store = home.join(".angelX/book-generations");
    let _book_store = super::super::generation_store::TestRootGuard::enter(store);
    let _sessions =
        crate::tests::TestEnvGuard::set("ANGEL_SESSION_DIR", sessions.to_str().unwrap());
    let _resources =
        crate::tests::TestEnvGuard::set("ANGEL_RESOURCE_DIR", resources.to_str().unwrap());
    let baseline = super::super::catalog::Catalog::baseline().bytes();
    assert_ne!(
        super::super::catalog::Catalog::live().unwrap().bytes(),
        baseline
    );
    let session =
        crate::knowledge::session::Session::at_for(sessions, "legacy".into(), workspace.path());
    session.save(&[ChatMsg::user("old conversation")]).unwrap();
    let mut history = crate::knowledge::session::load_for("legacy", workspace.path()).unwrap();
    assert!(is_legacy(&history));
    super::super::catalog::bind_turn(&mut history).unwrap();
    let saved_digest = generation_digest(&history).unwrap();
    assert_eq!(
        super::super::generation_store::load(saved_digest).unwrap(),
        baseline
    );
}

#[test]
fn legacy_conversation_reports_unavailable_generation_storage() {
    let _guard = crate::tests::env_lock();
    let workspace = crate::tests::TestGitWorkspace::new("book-legacy-unavailable");
    let sessions = workspace.path().join("sessions");
    let home_file = workspace.path().join("home-file");
    std::fs::write(&home_file, b"not a directory").unwrap();
    let unavailable = home_file.join("book-generations");
    let _book_store = super::super::generation_store::TestRootGuard::enter(unavailable);
    let _sessions =
        crate::tests::TestEnvGuard::set("ANGEL_SESSION_DIR", sessions.to_str().unwrap());
    let session = crate::knowledge::session::Session::at_for(
        sessions,
        "legacy-used".into(),
        workspace.path(),
    );
    session.save(&[ChatMsg::user("old conversation")]).unwrap();
    let mut history = crate::knowledge::session::load_for("legacy-used", workspace.path()).unwrap();
    assert!(is_legacy(&history));
    let mut summary = ChatMsg::harness("compacted legacy conversation");
    summary.book_memory = compaction_memory(&history);
    assert!(is_legacy(&[summary]));
    let error = super::super::catalog::bind_turn(&mut history).unwrap_err();
    assert!(error.contains("book generation"), "{error}");
}

#[test]
fn swapping_models_reteaches_used_signals_and_keeps_the_cached_prefix() {
    let mut history = vec![
        ChatMsg::system("⠽⠙"),
        ChatMsg::user("research"),
        ChatMsg::harness("⚠⡫⠃"),
    ];
    let mut state = Continuity::new(&history);
    assert!(!state.prepare(&mut history, &route("a")));
    let prefix = history
        .iter()
        .map(|m| m.content.clone())
        .collect::<Vec<_>>();
    assert!(!state.prepare(&mut history, &route("a")));
    assert!(state.prepare(&mut history, &route("b")));
    for (before, after) in prefix.iter().zip(&history) {
        assert_eq!(before, &after.content);
    }
    let introductions = introduction::introductions(&history, None);
    assert!(
        introductions
            .last()
            .unwrap()
            .1
            .contains("the same check returned identical evidence")
    );
    assert_eq!(introductions.last().unwrap().0, history.len() - 1);
}

#[test]
fn compaction_restores_the_inventory_even_when_all_old_signals_disappeared() {
    let mut history = vec![ChatMsg::harness("⚠⡫⠉"), ChatMsg::harness("⠧⠉")];
    let mut state = Continuity::new(&history);
    state.prepare(&mut history, &route("a"));
    history = vec![ChatMsg::harness("summary with no warning addresses")];
    assert!(state.prepare(&mut history, &route("a")));
    let carried = history.last().unwrap().content.clone();
    assert!(carried.contains("⡫⠉") && carried.contains("⠧⠉"));
    // Session restore starts from the compact history, not process-global state.
    let mut restored = Continuity::new(&history);
    assert!(!restored.prepare(&mut history, &route("a")));
    assert!(restored.prepare(&mut history, &route("b")));
}

#[test]
fn effort_change_and_quiet_turn_add_no_handoff() {
    let mut history = vec![ChatMsg::system("⠽⠙"), ChatMsg::user("hello")];
    let mut state = Continuity::new(&history);
    assert!(!state.prepare(&mut history, &route("a")));
    assert_eq!(history.len(), 2);
    assert!(history.iter().all(|message| message.book_memory.is_none()));
    history.push(ChatMsg::harness("⚠⡫⠙"));
    state.prepare(&mut history, &route("a"));
    let mut changed = route("a");
    changed.reasoning_effort = Some("high".into());
    assert!(!state.prepare(&mut history, &changed));
}

#[test]
fn warning_is_decodable_and_removed_from_distilled_prose() {
    assert!(ledger::is_warpath_message("⚠⡫⠃\n{}"));
    let history = [ChatMsg::harness("⚠⡫⠃")];
    assert!(
        introduction::introductions(&history, None)[0]
            .1
            .contains("intentional replication")
    );
}

#[test]
fn recalled_hypothesis_braille_is_data_and_does_not_raise_a_new_route() {
    let root = std::path::Path::new("/fixture/book-data");
    let advice = serde_json::json!({"relation_count":1,"checks":[{"latest":{
        "hypothesis":"⠇⠁", "expected":"⠟⠁"}, "count":1}], "signals":[]});
    let note = live::context_turn(root, &advice);
    let intros = introduction::introductions(&[ChatMsg::harness(note)], None);
    assert!(!intros[0].1.contains("⠇⠁ "));
    assert!(!intros[0].1.contains("⠟⠁ "));
    assert!(intros[0].1.contains("⡫⠁ "));
}

#[test]
fn session_restore_keeps_inventory_without_changing_the_provider_wire() {
    let mut history = vec![ChatMsg::user("research"), ChatMsg::harness("⚠⡫⠃")];
    let wire = crate::agent::club::messages_to_json(&history, false);
    let mut state = Continuity::new(&history);
    assert!(!state.prepare(&mut history, &route("a")));
    assert_eq!(crate::agent::club::messages_to_json(&history, false), wire);
    let saved = serde_json::to_vec(&history).unwrap();
    let mut restored: Vec<ChatMsg> = serde_json::from_slice(&saved).unwrap();
    let mut state = Continuity::new(&restored);
    assert!(!state.prepare(&mut restored, &route("a")));
    restored.push(ChatMsg::user("continue"));
    assert!(!state.prepare(&mut restored, &route("a")));
    assert_eq!(
        restored.iter().filter(|m| m.book_memory.is_some()).count(),
        1
    );
    assert!(state.prepare(&mut restored, &route("b")));
    assert!(restored.last().unwrap().content.contains("⡫⠃"));
}

#[test]
fn compaction_between_turns_carries_even_uncheckpointed_signals() {
    let mut history = vec![ChatMsg::user("research"), ChatMsg::harness("⚠⡫⠃")];
    Continuity::new(&history).prepare(&mut history, &route("a"));
    history.push(ChatMsg::harness("⚠⡫⠉"));
    let mut summary = ChatMsg::harness("summary with no signals");
    summary.book_memory = compaction_memory(&history);
    let mut compacted = vec![summary];
    let mut state = Continuity::new(&compacted);
    assert!(state.prepare(&mut compacted, &route("a")));
    let note = compacted.last().unwrap();
    assert!(note.content.contains("⡫⠃") && note.content.contains("⡫⠉"));
    assert!(!Continuity::new(&compacted).prepare(&mut compacted, &route("a")));
    assert!(Continuity::new(&compacted).prepare(&mut compacted, &route("b")));
}
