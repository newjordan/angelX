//! Session transitions preserve unsaved operator history until checkpoint succeeds.
use super::*;

struct BoundaryFixture(PathBuf);

impl BoundaryFixture {
    fn new(case: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "angel_session_boundary_guard_{case}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        for child in ["alpha", "beta", "sessions"] {
            std::fs::create_dir_all(root.join(child)).unwrap();
        }
        Self(root)
    }

    fn app(&self) -> App {
        let mut app = seed_preview_app();
        let alpha = self.0.join("alpha");
        app.input = format!("/cd {}", alpha.display());
        app.submit();
        assert_eq!(app.tools.current_workspace(), alpha);
        app.history.push(ChatMsg::user("last durable baseline"));
        app.session.checkpoint(&app.history).unwrap();
        app
    }
}

impl Drop for BoundaryFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn session_boundaries_preserve_unsaved_history_and_retry_after_failure() {
    let _lock = env_lock();
    for case in ["cd", "new", "resume"] {
        let fixture = BoundaryFixture::new(case);
        let _env = [
            TestEnvGuard::set("HOME", fixture.0.to_str().unwrap()),
            TestEnvGuard::set(
                "ANGEL_SESSION_DIR",
                fixture.0.join("sessions").to_str().unwrap(),
            ),
            TestEnvGuard::set("ANGEL_PROJECT_DOC", "0"),
            TestEnvGuard::unset("ANGEL_GOAL_FILE"),
            TestEnvGuard::unset("ANGEL_MEMORY_FILE"),
            TestEnvGuard::unset("ANGEL_LOOP_FILE"),
        ];
        let alpha = fixture.0.join("alpha");
        let beta = fixture.0.join("beta");
        let mut app = fixture.app();
        let target = session::Session::at_for(fixture.0.join("sessions"), "other".into(), &alpha);
        target
            .save(&[ChatMsg::user("saved resume target")])
            .unwrap();
        let old_id = app.session.id.clone();
        let old_path = app.session.path().to_path_buf();
        let original = std::fs::read(&old_path).unwrap();
        app.history.extend([
            ChatMsg::user(format!("unsaved operator {case}")),
            ChatMsg::assistant_calls(vec![crate::club::ToolCall {
                id: "kept-tool-pair".into(),
                name: "read_file".into(),
                args: serde_json::json!({"path": "owned-example.rs"}),
            }]),
            ChatMsg::tool("kept-tool-pair", "kept tool result"),
            ChatMsg::assistant("kept post-tool answer"),
        ]);
        let exact_history = serde_json::to_value(&app.history).unwrap();
        // A real owned-path open failure exercises the shared boundary contract
        // without process-global resource limits or any model/tool execution.
        let blocked_temp = old_path.with_extension(format!("json.{}.tmp", std::process::id()));
        std::fs::create_dir(&blocked_temp).unwrap();
        let error = app.session.checkpoint(&app.history).unwrap_err();
        assert!(matches!(&error, session::SessionSaveError::Write(_)));
        let command = match case {
            "cd" => format!("/cd {}", beta.display()),
            "new" => "/new".into(),
            "resume" => "/resume other".into(),
            _ => unreachable!(),
        };
        app.input = command.clone();
        app.submit();
        assert_eq!(app.session.id, old_id, "{case}: keep the failed session");
        assert_eq!(
            app.tools.current_workspace(),
            alpha,
            "{case}: keep workspace"
        );
        assert_eq!(
            serde_json::to_value(&app.history).unwrap(),
            exact_history,
            "{case}: keep every role and tool pair"
        );
        assert_eq!(
            app.session.save_status(),
            session::SessionSaveStatus::Failed(error),
            "{case}: keep typed failure"
        );
        assert_eq!(
            std::fs::read(&old_path).unwrap(),
            original,
            "{case}: preserve last good bytes"
        );
        let message = &app.messages.last().unwrap().text;
        assert!(
            message.contains("boundary blocked") && message.contains("retry"),
            "{case}: actionable failure: {message}"
        );

        // Repairing the fixture permits the same normal command. The old
        // conversation must be durable before its in-memory narrative is reset.
        std::fs::remove_dir(&blocked_temp).unwrap();
        app.input = command;
        app.submit();
        let durable = session::load_for(&old_id, &alpha).unwrap();
        assert_eq!(
            serde_json::to_value(durable).unwrap(),
            exact_history,
            "{case}: old thread checkpointed exactly before replacement"
        );
        assert!(
            !app.history
                .iter()
                .any(|message| message.content.as_ref() == format!("unsaved operator {case}"))
        );
        match case {
            "cd" => {
                assert_eq!(app.tools.current_workspace(), beta);
                assert_ne!(app.session.id, old_id);
            }
            "new" => {
                assert_eq!(app.tools.current_workspace(), alpha);
                assert_ne!(app.session.id, old_id);
                assert!(
                    !app.history
                        .iter()
                        .any(|message| message.role == ChatRole::User)
                );
            }
            "resume" => {
                assert_eq!(app.session.id, "other");
                assert!(
                    app.history
                        .iter()
                        .any(|message| message.content.as_ref() == "saved resume target")
                );
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn resuming_current_session_reloads_the_checkpoint_just_saved() {
    let _lock = env_lock();
    let fixture = BoundaryFixture::new("same_id");
    let _env = [
        TestEnvGuard::set("HOME", fixture.0.to_str().unwrap()),
        TestEnvGuard::set(
            "ANGEL_SESSION_DIR",
            fixture.0.join("sessions").to_str().unwrap(),
        ),
        TestEnvGuard::set("ANGEL_PROJECT_DOC", "0"),
        TestEnvGuard::unset("ANGEL_GOAL_FILE"),
        TestEnvGuard::unset("ANGEL_MEMORY_FILE"),
        TestEnvGuard::unset("ANGEL_LOOP_FILE"),
    ];
    let mut app = fixture.app();
    app.history.push(ChatMsg::user(
        "newest operator absent from the old disk snapshot",
    ));
    let id = app.session.id.clone();
    app.input = format!("/resume {id}");
    app.submit();
    assert_eq!(app.session.id, id);
    assert!(app.messages.last().unwrap().text.contains("(2 turns)"));
    for history in [
        app.history.clone(),
        session::load_for(&id, app.tools.current_workspace()).unwrap(),
    ] {
        assert!(history.iter().any(|message| message.content.as_ref()
            == "newest operator absent from the old disk snapshot"));
    }
    assert_eq!(
        app.session.save_status(),
        session::SessionSaveStatus::Healthy
    );
}
