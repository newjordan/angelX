//! Expected-red control before exit repair: accepted queue must survive exit.
use super::*;

#[test]
fn exit_queued_input_baseline_must_not_report_a_clean_exit_without_the_operator() {
    let _lock = env_lock();
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "angel-exit-queue-baseline-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    )));
    let workspace = fixture.0.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let _env = [
        TestEnvGuard::set("HOME", fixture.0.to_str().unwrap()),
        TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0"),
    ];
    let (mut app, worker) = seed_live_streaming_app(Vec::new());
    app.tools = Arc::new(harness::ToolRegistry::with_team(
        workspace.clone(),
        Vec::new(),
    ));
    app.session = session::Session::at_for(fixture.0.join("sessions"), "owned".into(), &workspace);
    app.history = vec![
        ChatMsg::system("static policy"),
        ChatMsg::user("OLD_DURABLE_OPERATOR"),
    ];
    app.session.checkpoint(&app.history).unwrap();
    let old = std::fs::read(app.session.path()).unwrap();
    let sentinel = "EXACT_UNPUBLISHED_OPERATOR\nnaïve 日本語 🧭 e\u{301}";
    app.input = sentinel.into();
    app.submit();
    assert_eq!(app.steer_queue.len(), 1);
    assert!(!app.history.iter().any(|m| m.content.as_ref() == sentinel));
    app.input = "/exit".into();
    app.submit();
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(app.session.path()).unwrap()).unwrap();
    let saved_exact_operator = saved["history"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["content"] == sentinel);
    eprintln!(
        "EXIT_QUEUED_BASELINE {}",
        serde_json::json!({
            "probe": "actual App submit + held Thinking result channel; no provider calls",
            "should_quit": app.should_quit,
            "queued_operator_count": app.steer_queue.len(),
            "saved_exact_operator": saved_exact_operator,
            "old_checkpoint_unchanged": std::fs::read(app.session.path()).unwrap() == old,
            "operator_utf8_bytes": sentinel.len(),
            "receipt": app.messages.last().map(|m| m.text.as_ref()),
        })
    );
    drop(worker);
    assert!(
        !app.should_quit,
        "accepted queued User bytes have no durable checkpoint; exit must retain the App lifecycle"
    );
    assert_eq!(app.steer_queue.len(), 1);
    assert!(
        !saved_exact_operator,
        "the worker has not published this queue yet"
    );
}
