//! SessionStart tells an external tool which session this process holds.
use super::*;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const SECRET: &str = "sk-session-start-secret";

struct StartFixture {
    root: PathBuf,
    alpha: PathBuf,
    beta: PathBuf,
    sessions: PathBuf,
    log: PathBuf,
    ids: PathBuf,
    workspaces: PathBuf,
    config: PathBuf,
}

impl StartFixture {
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
        let sessions = root.join("sessions");
        for dir in [&alpha, &beta, &sessions] {
            std::fs::create_dir_all(dir).unwrap();
        }
        Self {
            alpha: std::fs::canonicalize(&alpha).unwrap(),
            beta: std::fs::canonicalize(&beta).unwrap(),
            sessions,
            log: root.join("events.jsonl"),
            ids: root.join("ids"),
            workspaces: root.join("workspaces"),
            config: root.join("hooks.json"),
            root,
        }
    }

    fn write_hook(&self, command: &str) {
        let script = self.root.join("hook.sh");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\npayload=$(cat)\nprintf '%s\\n' \"$payload\" >> '{}'\nprintf '%s\\n' \"$ANGEL_SESSION_ID\" >> '{}'\nprintf '%s\\n' \"$ANGEL_WORKSPACE\" >> '{}'\n",
                self.log.display(),
                self.ids.display(),
                self.workspaces.display()
            ),
        )
        .unwrap();
        let command = if command.is_empty() {
            format!("sh '{}'", script.display())
        } else {
            command.to_string()
        };
        std::fs::write(
            &self.config,
            serde_json::json!({
                "hooks": {
                    "SessionStart": [{ "matcher": "*", "command": command }]
                }
            })
            .to_string(),
        )
        .unwrap();
    }

    fn guards(&self) -> [TestEnvGuard; 12] {
        [
            TestEnvGuard::set("HOME", self.root.to_str().unwrap()),
            TestEnvGuard::set("ANGEL_SESSION_DIR", self.sessions.to_str().unwrap()),
            TestEnvGuard::set("ANGEL_HOOKS_CONFIG", self.config.to_str().unwrap()),
            TestEnvGuard::set(
                "ANGEL_MCP_CONFIG",
                self.root.join("absent-mcp.json").to_str().unwrap(),
            ),
            TestEnvGuard::set("ANGEL_LSP", "0"),
            TestEnvGuard::set("ANGEL_PROJECT_DOC", "0"),
            TestEnvGuard::set("ANGEL_YOLO", "0"),
            TestEnvGuard::set("ANGEL_YOLO_SMART", "0"),
            // Inherited startup value. SessionStart must override it with the
            // workspace the process holds now, including after /cd.
            TestEnvGuard::set("ANGEL_WORKSPACE", self.root.to_str().unwrap()),
            TestEnvGuard::unset("ANGEL_GOAL_FILE"),
            TestEnvGuard::unset("ANGEL_MEMORY_FILE"),
            TestEnvGuard::unset("ANGEL_LOOP_FILE"),
        ]
    }

    fn app(&self, workspace: &Path) -> App {
        App::new_selected_in_workspace(
            Bag::practice_for_test(),
            Viewer::new(),
            workspace.to_path_buf(),
        )
    }
}

impl Drop for StartFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

struct Event {
    session_id: String,
    cwd: String,
    source: String,
}

fn read_events(fixture: &StartFixture) -> Vec<Event> {
    let raw = std::fs::read_to_string(&fixture.log).unwrap_or_default();
    let id_text = std::fs::read_to_string(&fixture.ids).unwrap_or_default();
    let ws_text = std::fs::read_to_string(&fixture.workspaces).unwrap_or_default();
    let lines: Vec<_> = raw.lines().filter(|line| !line.is_empty()).collect();
    let ids: Vec<_> = id_text.lines().collect();
    let workspaces: Vec<_> = ws_text.lines().collect();
    assert_eq!(
        lines.len(),
        ids.len(),
        "stdin events and ANGEL_SESSION_ID lines diverged"
    );
    assert_eq!(
        lines.len(),
        workspaces.len(),
        "stdin events and ANGEL_WORKSPACE lines diverged"
    );
    let sessions = fixture.sessions.display().to_string();
    lines
        .into_iter()
        .zip(ids)
        .zip(workspaces)
        .map(|((line, env_id), env_ws)| {
            assert!(
                !line.contains(SECRET) && !line.contains(&sessions) && !line.contains(".json"),
                "payload leaked conversation, a credential, or a transcript path: {line}"
            );
            let value: serde_json::Value = serde_json::from_str(line).unwrap_or_else(|error| {
                panic!("stdin was not the SessionStart object: {error}: {line}")
            });
            let object = value.as_object().expect("payload object");
            assert_eq!(object.len(), 4, "unexpected payload fields: {line}");
            assert_eq!(value["hook_event_name"], "SessionStart");
            let session_id = value["session_id"].as_str().unwrap().to_string();
            let cwd = value["cwd"].as_str().unwrap().to_string();
            assert_eq!(env_id, session_id, "ANGEL_SESSION_ID");
            assert_eq!(env_ws, cwd, "ANGEL_WORKSPACE");
            Event {
                session_id,
                cwd,
                source: value["source"].as_str().unwrap().to_string(),
            }
        })
        .collect()
}

fn expect_last(events: &[Event], id: &str, cwd: &Path, source: &str) {
    let event = events.last().expect("SessionStart event");
    assert_eq!(event.session_id, id);
    assert_eq!(event.cwd, cwd.to_str().unwrap());
    assert_eq!(event.source, source);
}

fn run_cmd(app: &mut App, command: &str) {
    app.input = command.to_string();
    app.submit();
}

#[test]
fn session_start_receives_the_payload_once_at_startup_and_on_each_change() {
    let _lock = env_lock();
    let fixture = StartFixture::new("payload");
    fixture.write_hook("");
    let _env = fixture.guards();
    let mut app = fixture.app(&fixture.alpha);
    let tools = app.tools.defs().len();
    assert!(tools > 0, "startup must still register tools");
    let startup_id = app.session.id.clone();
    app.establish_launch_session(None);
    let events = read_events(&fixture);
    assert_eq!(events.len(), 1, "startup announces the session once");
    expect_last(&events, &startup_id, &fixture.alpha, "startup");
    assert_ne!(events[0].cwd, fixture.root.to_str().unwrap());

    app.history
        .push(ChatMsg::user(format!("{SECRET} in the conversation")));
    run_cmd(&mut app, "/new");
    let new_id = app.session.id.clone();
    assert_ne!(new_id, startup_id);
    assert_eq!(app.tools.defs().len(), tools);
    assert!(
        app.messages
            .iter()
            .any(|message| message.text.contains("new chat"))
    );
    let events = read_events(&fixture);
    assert_eq!(events.len(), 2);
    expect_last(&events, &new_id, &fixture.alpha, "clear");

    run_cmd(&mut app, &format!("/resume {startup_id}"));
    assert_eq!(app.session.id, startup_id);
    assert!(app.messages.iter().any(|message| {
        message
            .text
            .contains(&format!("resumed session {startup_id}"))
    }));
    let events = read_events(&fixture);
    assert_eq!(events.len(), 3);
    expect_last(&events, &startup_id, &fixture.alpha, "resume");

    run_cmd(&mut app, "/fork");
    let fork_id = app.session.id.clone();
    assert_ne!(fork_id, startup_id);
    assert!(
        app.messages
            .iter()
            .any(|message| message.text.contains("forked"))
    );
    let events = read_events(&fixture);
    assert_eq!(events.len(), 4);
    expect_last(&events, &fork_id, &fixture.alpha, "fork");

    run_cmd(&mut app, &format!("/cd {}", fixture.beta.display()));
    let cd_id = app.session.id.clone();
    assert_ne!(cd_id, fork_id);
    assert_eq!(app.tools.current_workspace(), fixture.beta);
    assert!(app.tools.defs().len() > 0);
    let events = read_events(&fixture);
    assert_eq!(events.len(), 5);
    expect_last(&events, &cd_id, &fixture.beta, "workspace");

    run_cmd(&mut app, "/clear");
    let clear_id = app.session.id.clone();
    assert_ne!(clear_id, cd_id);
    assert_eq!(app.tools.current_workspace(), fixture.beta);
    let events = read_events(&fixture);
    assert_eq!(events.len(), 6);
    expect_last(&events, &clear_id, &fixture.beta, "clear");

    run_cmd(&mut app, &format!("/workspace {}", fixture.alpha.display()));
    let moved_id = app.session.id.clone();
    assert_ne!(moved_id, clear_id);
    assert_eq!(app.tools.current_workspace(), fixture.alpha);
    let events = read_events(&fixture);
    assert_eq!(events.len(), 7);
    expect_last(&events, &moved_id, &fixture.alpha, "workspace");

    let before_failures = events.len();
    run_cmd(&mut app, "/cd /no/such/angel-session-start");
    run_cmd(&mut app, "/resume definitely-missing-session");
    assert_eq!(
        read_events(&fixture).len(),
        before_failures,
        "a failed /cd or /resume must not announce a session change"
    );
    assert_eq!(app.session.id, moved_id);
    assert_eq!(app.tools.current_workspace(), fixture.alpha);

    // A relight re-execs with `--resume` and must announce that id once, as resume.
    let mut reborn = fixture.app(&fixture.alpha);
    reborn.establish_launch_session(Some(Some(startup_id.clone())));
    assert_eq!(reborn.session.id, startup_id);
    let events = read_events(&fixture);
    assert_eq!(events.len(), before_failures + 1);
    expect_last(&events, &startup_id, &fixture.alpha, "resume");
}

#[test]
fn failing_or_timed_out_session_start_still_lets_the_session_start() {
    let _lock = env_lock();
    let fixture = StartFixture::new("best-effort");
    fixture.write_hook("echo no >&2; exit 9");
    let _env = fixture.guards();
    let _timeout = TestEnvGuard::set("ANGEL_HOOK_TIMEOUT", "1");
    let mut app = fixture.app(&fixture.alpha);
    let startup_id = app.session.id.clone();
    app.establish_launch_session(None);
    assert_eq!(app.session.id, startup_id);
    assert!(
        !app.messages.iter().any(
            |message| message.text.contains("blocked") || message.text.contains("SessionStart")
        ),
        "a failing SessionStart must not change the session: {:?}",
        app.messages
            .iter()
            .map(|message| message.text.as_ref())
            .collect::<Vec<_>>()
    );
    run_cmd(&mut app, "/new");
    assert_ne!(app.session.id, startup_id);
    assert!(
        app.messages
            .iter()
            .any(|message| message.text.contains("new chat"))
    );

    fixture.write_hook("sleep 30");
    let mut slow = fixture.app(&fixture.beta);
    let slow_id = slow.session.id.clone();
    let started = Instant::now();
    slow.establish_launch_session(None);
    let elapsed = started.elapsed();
    assert_eq!(slow.session.id, slow_id);
    assert!(
        elapsed >= Duration::from_millis(700) && elapsed < Duration::from_secs(8),
        "a timed-out SessionStart must return and leave the session in place: {elapsed:?}"
    );
    let before = slow.session.id.clone();
    let again = Instant::now();
    run_cmd(&mut slow, "/new");
    let again_elapsed = again.elapsed();
    assert_ne!(slow.session.id, before);
    assert!(
        slow.messages
            .iter()
            .any(|message| message.text.contains("new chat"))
    );
    assert!(
        again_elapsed >= Duration::from_millis(700) && again_elapsed < Duration::from_secs(8),
        "a timed-out SessionStart must not block /new: {again_elapsed:?}"
    );
}
