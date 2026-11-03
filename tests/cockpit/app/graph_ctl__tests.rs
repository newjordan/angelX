use super::*;

#[test]
fn graph_status_reports_catalog_preflight_failure() {
    let _guard = crate::tests::env_lock();
    let workspace = crate::tests::TestGitWorkspace::new("graph-catalog-failure");
    let root = workspace.path();
    let graphs = root.join("graphs");
    let resources = root.join("resources");
    std::fs::create_dir_all(&graphs).unwrap();
    std::fs::create_dir_all(resources.join("cockpit/book")).unwrap();
    std::fs::write(
        graphs.join("fixture.toml"),
        "name = 'fixture'\n[[node]]\nid = 'one'\nprompt = 'do the task'\n",
    )
    .unwrap();
    std::fs::write(
        resources.join("cockpit/book/gh_navigation.tsv"),
        "invalid\n",
    )
    .unwrap();
    let _graphs = crate::tests::TestEnvGuard::set("ANGEL_GRAPHS_DIR", graphs.to_str().unwrap());
    let _bundled = crate::tests::TestEnvGuard::set(
        "ANGEL_BUNDLED_GRAPHS_DIR",
        root.join("no-bundled-graphs").to_str().unwrap(),
    );
    let _resources =
        crate::tests::TestEnvGuard::set("ANGEL_RESOURCE_DIR", resources.to_str().unwrap());
    let mut state = GraphState::default();
    state
        .start(root.to_path_buf(), None, vec![], "fixture", "work")
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while state.running() {
        assert!(
            std::time::Instant::now() < deadline,
            "graph worker never stopped"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let status = status_text(&state);
    assert!(status.contains("'fixture' · FAILED"), "{status}");
    assert!(status.contains("book catalog manifest is missing"), "{status}");
}

#[test]
fn graph_status_does_not_show_the_previous_episode_while_starting() {
    let _guard = crate::tests::env_lock();
    let previous =
        crate::agent::harness::current_graph_snapshot().map(|snapshot| snapshot.episode_id);
    let (release, wait) = std::sync::mpsc::channel::<()>();
    let worker = std::thread::spawn(move || {
        let _ = wait.recv();
    });
    let mut state = GraphState::default();
    state.worker = Some(worker);
    state.run_name = Some("new-run".into());
    state.previous_episode = previous;
    assert_eq!(status_text(&state), "graph · 'new-run' · starting");
    release.send(()).unwrap();
    state.worker.take().unwrap().join().unwrap();
}
