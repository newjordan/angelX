//! The shell pane and the module host's "shell" close together.
use super::*;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// `/open shell`, the module's own way in: it spawns the pane where this host
/// has a PTY, and the module runs either way.
fn open_shell(app: &mut App) {
    assert_eq!(app.open_module("shell"), "module shell active");
    assert!(app.module_host.is_running("shell"));
    assert_eq!(app.shell.is_some(), pty_available());
}

fn assert_closed(app: &App) {
    assert!(app.shell.is_none());
    assert!(!app.shell_focused);
    assert!(
        !app.module_host.is_running("shell"),
        "the module host still lists the shell as active:\n{}",
        app.modules_text()
    );
}

/// A pane whose shell has already exited: `$SHELL` is `true`.
fn open_dead_shell(app: &mut App) {
    open_shell(app);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.shell.as_ref().is_some_and(|shell| shell.is_alive()) {
        assert!(
            std::time::Instant::now() < deadline,
            "the shell never exited"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn a_workspace_change_closes_the_shell_pane_and_its_module() {
    let _guard = env_lock();
    let _shell = TestEnvGuard::set("SHELL", "/bin/sh");
    let mut app = seed_preview_app();
    open_shell(&mut app);

    let workspace = crate::tests::TestGitWorkspace::new("shell-workspace-change");
    app.change_workspace(workspace.path().to_str());

    assert_closed(&app);
}

#[test]
fn a_hard_exit_closes_the_shell_pane_and_its_module() {
    let _guard = env_lock();
    let fixture = std::env::temp_dir().join(format!(
        "angel-shell-exit-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let workspace = fixture.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let _env = [
        TestEnvGuard::set("HOME", fixture.to_str().unwrap()),
        TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0"),
        TestEnvGuard::set("SHELL", "/bin/sh"),
    ];
    let mut app = seed_preview_app();
    app.tools = Arc::new(harness::ToolRegistry::with_team(
        workspace.clone(),
        Vec::new(),
    ));
    app.session = session::Session::at_for(fixture.join("sessions"), "owned".into(), &workspace);
    open_shell(&mut app);

    app.input = "/exit!".into();
    app.submit();

    assert!(app.should_quit);
    assert_closed(&app);
    let _ = std::fs::remove_dir_all(&fixture);
}

#[test]
fn a_shell_that_exits_on_its_own_is_closed_with_a_notice_by_the_poll() {
    let _guard = env_lock();
    if !pty_available() {
        eprintln!("skipping: no PTY access in this environment");
        return;
    }
    let _shell = TestEnvGuard::set("SHELL", "/bin/true");
    let mut app = seed_preview_app();
    open_dead_shell(&mut app);

    app.poll_shell_status();

    assert_closed(&app);
    assert_eq!(
        app.messages.last().map(|m| m.text.as_ref()),
        Some("shell process exited")
    );
}

#[test]
fn a_key_sent_to_a_shell_that_exited_closes_it_with_a_notice() {
    let _guard = env_lock();
    if !pty_available() {
        eprintln!("skipping: no PTY access in this environment");
        return;
    }
    let _shell = TestEnvGuard::set("SHELL", "/bin/true");
    let mut app = seed_preview_app();
    open_dead_shell(&mut app);
    assert!(app.shell_focused);

    app.on_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));

    assert_closed(&app);
    assert_eq!(
        app.messages.last().map(|m| m.text.as_ref()),
        Some("shell process exited")
    );
}
