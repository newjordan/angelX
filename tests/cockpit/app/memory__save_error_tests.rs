//! Explicit memory save results preserve old bytes and UTF-8 schema compatibility.
use super::*;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "angel-memory-save-errors-{}-{}",
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
fn rejected_memory_content_and_foreign_or_malformed_store_are_explicit() {
    let _lock = crate::tests::env_lock();
    let fixture = Fixture::new();
    let path = fixture.0.join("memory.json");
    let _env = [
        crate::tests::TestEnvGuard::set("ANGEL_MEMORY_FILE", path.to_str().unwrap()),
        crate::tests::TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0"),
    ];
    let alpha = fixture.0.join("alpha");
    let beta = fixture.0.join("beta");
    std::fs::create_dir(&alpha).unwrap();
    std::fs::create_dir(&beta).unwrap();
    save_for(&["last good memory"], &alpha).unwrap();
    let old = std::fs::read(&path).unwrap();
    assert_eq!(
        save_for(&[""], &alpha),
        Err(MemorySaveError::InvalidMemories)
    );
    assert!(matches!(
        save_for(&["foreign"], &beta),
        Err(MemorySaveError::ExistingStore(_))
    ));
    assert_eq!(std::fs::read(&path).unwrap(), old);
    std::fs::write(&path, b"unrecognized prior bytes").unwrap();
    assert!(matches!(
        save_for(&["replacement"], &alpha),
        Err(MemorySaveError::ExistingStore(_))
    ));
    assert_eq!(std::fs::read(&path).unwrap(), b"unrecognized prior bytes");
}

#[cfg(unix)]
#[test]
fn invalid_workspace_save_is_explicit_and_preserves_prior_store() {
    use std::os::unix::ffi::OsStringExt as _;
    let _lock = crate::tests::env_lock();
    let fixture = Fixture::new();
    let path = fixture.0.join("memory.json");
    let _env = [
        crate::tests::TestEnvGuard::set("ANGEL_MEMORY_FILE", path.to_str().unwrap()),
        crate::tests::TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0"),
    ];
    let valid = fixture.0.join("valid");
    let raw = fixture
        .0
        .join(std::ffi::OsString::from_vec(b"raw-\xff".to_vec()));
    std::fs::create_dir(&valid).unwrap();
    std::fs::create_dir(&raw).unwrap();
    save_for(&["prior bytes belong to the valid project"], &valid).unwrap();
    let old = std::fs::read(&path).unwrap();
    for memories in [vec!["new"], vec![]] {
        let error = save_for(&memories, &raw).unwrap_err();
        assert_eq!(
            error,
            MemorySaveError::UnsupportedWorkspacePath(raw.clone())
        );
        assert!(error.to_string().contains("invalid UTF-8"));
        assert_eq!(std::fs::read(&path).unwrap(), old);
    }
    assert!(!path.with_extension("json.tmp").exists());
}

#[cfg(unix)]
#[test]
fn non_utf8_destination_preserves_raw_override_and_valid_json_schema() {
    use std::os::unix::{ffi::OsStringExt as _, fs::PermissionsExt as _};
    let _lock = crate::tests::env_lock();
    let fixture = Fixture::new();
    let workspace = fixture.0.join("valid naïve 🦀");
    std::fs::create_dir(&workspace).unwrap();
    let path = fixture
        .0
        .join(std::ffi::OsString::from_vec(b"store-\xfe.json".to_vec()));
    // The ordinary guard owns restoration; the scoped raw override remains
    // protected by the same process-wide environment lock.
    let _env = [
        crate::tests::TestEnvGuard::unset("ANGEL_MEMORY_FILE"),
        crate::tests::TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0"),
    ];
    unsafe {
        std::env::set_var("ANGEL_MEMORY_FILE", &path);
    }
    assert_eq!(store_path_for(&workspace), path);
    save_for(&["unicode memory 🦀"], &workspace).unwrap();
    assert_eq!(load_for(&workspace), vec!["unicode memory 🦀"]);
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["schema"], MEMORY_SCHEMA);
    assert_eq!(json["workspace"], workspace.to_str().unwrap());
    assert_eq!(
        json["project_key"],
        crate::workspace_store::workspace_key(&workspace)
    );
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 2);
}
