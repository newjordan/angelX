//! SessionStart reaches a configured hook at startup and after each session change.
use super::*;

const SENTINEL: &str = "SESSION_START_MUST_NOT_LEAK_THIS_TRANSCRIPT";

struct Fixture {
    root: PathBuf,
    alpha: PathBuf,
    beta: PathBuf,
    log: PathBuf,
}

impl Fixture {
    fn new(case: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "angel_session_start_{case}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let alpha = root.join("alpha");
        let beta = root.join("beta");
        std::fs::create_dir_all(&alpha).unwrap();
        std::fs::create_dir_all(&beta).unwrap();
        std::fs::create_dir_all(root.join("sessions")).unwrap();
        Self {
            log: root.join("hooks.log"),
            root,
            alpha,
            beta,
        }
    }

    fn canon(path: &Path) -> PathBuf {
        std::fs::canonicalize(path).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn recording_command(log: &Path) -> String {
    let quoted = format!("'{}'", log.display());
    format!(
        "printf '%s\\n' \"$ANGEL_SESSION_ID\" >> {quoted}; printf '%s\\n' \"$ANGEL_WORKSPACE\" >> {quoted}; cat >> {quoted}"
    )
}

fn write_hooks(root: &Path, command: &str) -> PathBuf {
    let path = root.join("hooks.json");
    let body = serde_json::json!({
        "hooks": { "SessionStart": [{ "matcher": "*", "command": command }] }
    });
    std::fs::write(&path, serde_json::to_vec(&body).unwrap()).unwrap();
    path
}

fn events(log: &Path) -> Vec<serde_json::Value> {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let mut lines = text.lines();
    let mut out = Vec::new();
    while let Some(session_id) = lines.next() {
        let workspace = lines
            .next()
            .unwrap_or_else(|| panic!("workspace line after {session_id}"));
        let json = lines
            .next()
            .unwrap_or_else(|| panic!("payload after {session_id}"));
        let value: serde_json::Value = serde_json::from_str(json).unwrap_or_else(|error| {
            panic!("payload is not JSON ({error}): {json}");
        });
        let mut keys: Vec<_> = value.as_object().expect("object").keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            ["cwd", "hook_event_name", "session_id", "source"],
            "payload must stay identity-only: {value}"
        );
        assert_eq!(value["hook_event_name"], "SessionStart");
        assert_eq!(value["session_id"], session_id, "ANGEL_SESSION_ID");
        assert_eq!(value["cwd"], workspace, "ANGEL_WORKSPACE");
        out.push(value);
    }
    out
}

fn last_note(app: &App) -> String {
    app.messages
        .last()
        .map(|message| message.text.to_string())
        .unwrap_or_default()
}

#[test]
fn session_start_hook_fires_on_startup_and_session_changes() {
    let _lock = crate::tests::env_lock();
    let fixture = Fixture::new("lifecycle");
    let hooks = write_hooks(&fixture.root, &recording_command(&fixture.log));
    let _env = [
        crate::tests::TestEnvGuard::set("ANGEL_HOOKS_CONFIG", hooks.to_str().unwrap()),
        crate::tests::TestEnvGuard::set(
            "ANGEL_SESSION_DIR",
            fixture.root.join("sessions").to_str().unwrap(),
        ),
        crate::tests::TestEnvGuard::set("ANGEL_HOOK_TIMEOUT", "10"),
        crate::tests::TestEnvGuard::set("ANGEL_PROJECT_DOC", "0"),
        crate::tests::TestEnvGuard::unset("ANGEL_GOAL_FILE"),
        crate::tests::TestEnvGuard::unset("ANGEL_MEMORY_FILE"),
        crate::tests::TestEnvGuard::unset("ANGEL_LOOP_FILE"),
    ];
    let alpha = Fixture::canon(&fixture.alpha);
    let beta = Fixture::canon(&fixture.beta);
    let mut app = crate::seed_preview_app();

    app.input = format!("/cd {}", alpha.display());
    app.submit();
    assert_eq!(
        app.tools.current_workspace(),
        alpha.as_path(),
        "/cd failed: {}",
        last_note(&app)
    );
    let after_cd = app.session.id.clone();

    app.announce_startup_session(None);
    assert_eq!(
        app.session.id, after_cd,
        "fresh startup keeps the open session"
    );

    app.history
        .push(crate::agent::club::ChatMsg::user(SENTINEL));
    app.session
        .save(&app.history)
        .expect("checkpoint before relight");
    app.announce_startup_session(Some(Some(after_cd.clone())));
    assert_eq!(app.session.id, after_cd, "relight keeps the resumed id");

    app.input = "/new".into();
    app.submit();
    let after_new = app.session.id.clone();
    assert_ne!(
        after_new,
        after_cd,
        "/new should open a new session: {}",
        last_note(&app)
    );
    assert!(last_note(&app).contains("new chat"), "{}", last_note(&app));

    app.input = "/clear".into();
    app.submit();
    let after_clear = app.session.id.clone();
    assert_ne!(after_clear, after_new, "/clear should open a new session");

    app.input = format!("/resume {after_cd}");
    app.submit();
    assert_eq!(
        app.session.id,
        after_cd,
        "/resume failed: {}",
        last_note(&app)
    );

    app.input = "/fork".into();
    app.submit();
    let after_fork = app.session.id.clone();
    assert_ne!(after_fork, after_cd, "/fork failed: {}", last_note(&app));
    assert_eq!(app.tools.current_workspace(), alpha.as_path());

    app.input = format!("/cd {}", beta.display());
    app.submit();
    let after_workspace = app.session.id.clone();
    assert_ne!(
        after_workspace, after_fork,
        "/cd should open a fresh session"
    );
    assert_eq!(
        app.tools.current_workspace(),
        beta.as_path(),
        "{}",
        last_note(&app)
    );

    let got = events(&fixture.log);
    let expect = [
        ("workspace", after_cd.as_str(), alpha.as_path()),
        ("startup", after_cd.as_str(), alpha.as_path()),
        ("resume", after_cd.as_str(), alpha.as_path()),
        ("startup", after_new.as_str(), alpha.as_path()),
        ("clear", after_clear.as_str(), alpha.as_path()),
        ("resume", after_cd.as_str(), alpha.as_path()),
        ("fork", after_fork.as_str(), alpha.as_path()),
        ("workspace", after_workspace.as_str(), beta.as_path()),
    ];
    assert_eq!(got.len(), expect.len(), "{got:?}");
    for (event, (source, session_id, cwd)) in got.iter().zip(expect) {
        assert_eq!(event["source"], source, "{event}");
        assert_eq!(event["session_id"], session_id, "{event}");
        assert_eq!(event["cwd"], cwd.to_string_lossy().as_ref(), "{event}");
    }
    let raw = std::fs::read_to_string(&fixture.log).unwrap();
    assert!(!raw.contains(SENTINEL), "conversation leaked into the hook");
    assert!(
        !raw.contains(&fixture.root.join("sessions").display().to_string()),
        "session file path leaked into the hook: {raw}"
    );
}

#[test]
fn session_start_hook_failure_and_timeout_still_let_the_session_start() {
    let _lock = crate::tests::env_lock();
    let fixture = Fixture::new("best-effort");
    let hooks = write_hooks(&fixture.root, "sleep 30");
    let _env = [
        crate::tests::TestEnvGuard::set("ANGEL_HOOKS_CONFIG", hooks.to_str().unwrap()),
        crate::tests::TestEnvGuard::set(
            "ANGEL_SESSION_DIR",
            fixture.root.join("sessions").to_str().unwrap(),
        ),
        crate::tests::TestEnvGuard::set("ANGEL_HOOK_TIMEOUT", "0"),
        crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1"),
        crate::tests::TestEnvGuard::set("ANGEL_PROJECT_DOC", "0"),
        crate::tests::TestEnvGuard::unset("ANGEL_GOAL_FILE"),
        crate::tests::TestEnvGuard::unset("ANGEL_MEMORY_FILE"),
        crate::tests::TestEnvGuard::unset("ANGEL_LOOP_FILE"),
    ];
    let alpha = Fixture::canon(&fixture.alpha);
    let mut app = crate::seed_preview_app();
    let started = std::time::Instant::now();
    app.input = format!("/cd {}", alpha.display());
    app.submit();
    assert_eq!(
        app.tools.current_workspace(),
        alpha.as_path(),
        "a timed-out SessionStart hook blocked /cd: {}",
        last_note(&app)
    );
    let opened = app.session.id.clone();
    assert!(!app.session.is_disabled(), "session did not start");
    app.input = "/new".into();
    app.submit();
    assert_ne!(
        app.session.id,
        opened,
        "a timed-out SessionStart hook blocked /new: {}",
        last_note(&app)
    );
    let kept = app.session.id.clone();
    app.announce_startup_session(None);
    assert_eq!(kept, app.session.id);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(20),
        "slow SessionStart hook blocked session changes for {:?}",
        started.elapsed()
    );

    // Same hooks file the loader cached. A different command length changes the
    // cache key, so /clear runs the failing command rather than the sleep.
    let _rewritten = write_hooks(&fixture.root, "echo blocked >&2; exit 7");
    let _timeout = crate::tests::TestEnvGuard::set("ANGEL_HOOK_TIMEOUT", "10");
    let before = app.session.id.clone();
    app.input = "/clear".into();
    app.submit();
    assert_ne!(
        app.session.id,
        before,
        "a failing SessionStart hook blocked /clear: {}",
        last_note(&app)
    );
    assert!(last_note(&app).contains("new chat"), "{}", last_note(&app));
}
