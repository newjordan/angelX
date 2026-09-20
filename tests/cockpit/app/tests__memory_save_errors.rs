//! Failed memory commands retain live values and never acknowledge a false save.
use super::*;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "angel-memory-command-errors-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn memory_mutations_report_failed_save_keep_values_and_retry_normally() {
    let _lock = env_lock();
    let fixture = Fixture::new();
    let path = fixture.0.join("memory.json");
    let _env = [
        TestEnvGuard::set("HOME", fixture.0.to_str().unwrap()),
        TestEnvGuard::set("ANGEL_MEMORY_FILE", path.to_str().unwrap()),
        TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0"),
    ];
    let workspace = fixture.0.join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let mut app = seed_preview_app();
    app.tools = Arc::new(harness::ToolRegistry::with_team(
        workspace.clone(),
        Vec::new(),
    ));
    let blocked_parent = fixture.0.join("blocked");
    std::fs::write(&blocked_parent, b"owned non-directory").unwrap();
    for (command, expected) in [
        ("add next", vec!["held", "kept", "next"]),
        ("next", vec!["held", "kept", "next"]),
        ("forget 1", vec!["kept"]),
        ("clear", vec![]),
    ] {
        app.memories = vec![Arc::from("held"), Arc::from("kept")];
        memory::save_for(&app.memories, &workspace).unwrap();
        let old = std::fs::read(&path).unwrap();
        let prior = app.memories.clone();
        {
            let _blocked = TestEnvGuard::set(
                "ANGEL_MEMORY_FILE",
                blocked_parent.join("memory.json").to_str().unwrap(),
            );
            let response = app.run_memories(Some(command));
            assert!(
                response.starts_with("/memories: save failed; change not applied in this chat:"),
                "{response}"
            );
            assert!(response.contains("retry"), "{response}");
            assert_eq!(app.memories, prior, "{command}");
            assert_eq!(std::fs::read(&path).unwrap(), old);
            assert_eq!(
                std::fs::read(&blocked_parent).unwrap(),
                b"owned non-directory"
            );
        }
        let response = app.run_memories(Some(command));
        assert!(!response.contains("save failed"), "{response}");
        assert_eq!(
            app.memories
                .iter()
                .map(AsRef::as_ref)
                .collect::<Vec<&str>>(),
            expected
        );
        assert_eq!(memory::load_for(&workspace), expected);
    }
    app.memories = vec![Arc::from("retained")];
    let response = app.run_memories(Some(&"x".repeat(memory::MAX_MEMORY_ITEM_BYTES + 1)));
    assert!(response.contains("maximum"), "{response}");
    assert_eq!(app.memories, vec![Arc::<str>::from("retained")]);
}

#[cfg(unix)]
#[test]
fn invalid_workspace_memory_commands_report_unsupported_without_erasing_values() {
    use std::os::unix::ffi::OsStringExt as _;
    let _lock = env_lock();
    let fixture = Fixture::new();
    let path = fixture.0.join("memory.json");
    let _env = [
        TestEnvGuard::set("HOME", fixture.0.to_str().unwrap()),
        TestEnvGuard::set("ANGEL_MEMORY_FILE", path.to_str().unwrap()),
        TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0"),
    ];
    let valid = fixture.0.join("valid");
    let raw = fixture
        .0
        .join(std::ffi::OsString::from_vec(b"raw-\xff".to_vec()));
    std::fs::create_dir(&valid).unwrap();
    std::fs::create_dir(&raw).unwrap();
    memory::save_for(&["prior project"], &valid).unwrap();
    let old = std::fs::read(&path).unwrap();
    let mut app = seed_preview_app();
    app.tools = Arc::new(harness::ToolRegistry::with_team(raw, Vec::new()));
    app.memories = vec![Arc::from("held"), Arc::from("kept")];
    let prior = app.memories.clone();
    for command in ["add next", "next", "forget 1", "clear"] {
        let response = app.run_memories(Some(command));
        assert!(
            response.contains("save failed") && response.contains("invalid UTF-8"),
            "{response}"
        );
        assert_eq!(app.memories, prior, "{command}");
        assert_eq!(std::fs::read(&path).unwrap(), old);
    }
}
