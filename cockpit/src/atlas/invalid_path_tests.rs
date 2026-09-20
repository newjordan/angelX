//! Unsupported project path bytes remain inert and preserve existing snapshots.
#![cfg(unix)]
use super::*;
use std::os::unix::ffi::OsStringExt as _;

#[test]
fn invalid_workspace_has_inert_atlas_health_and_cannot_publish_a_snapshot() {
    let _lock = crate::tests::env_lock();
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "angel-atlas-invalid-path-{}-{}",
        std::process::id(),
        now_ms()
    )));
    let raw = fixture
        .0
        .join(std::ffi::OsString::from_vec(b"raw-\xff".to_vec()));
    std::fs::create_dir_all(&raw).unwrap();
    let _env = [
        crate::tests::TestEnvGuard::set("ANGEL_ATLAS", "1"),
        crate::tests::TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0"),
    ];
    let store = fixture.0.join("atlas");
    let service = AtlasService::open_in(&raw, store.clone());
    let status = service.status();
    let result = service.add_operator(AtlasKind::Fact, "must not become a false durable fact");
    let error = result.unwrap_err();
    println!(
        "initial health: {:?}; rejected mutation: {error}; subsequent health: {:?}",
        status.health,
        service.status().health
    );
    assert!(error.contains("invalid UTF-8"));
    assert_eq!(status.health, AtlasHealth::Inert);
    assert!(status.detail.unwrap().contains("invalid UTF-8"));
    assert_eq!(service.status().health, AtlasHealth::Inert);
    assert!(service.item("missing").is_none());
    assert!(!service.project_path.exists());
    assert!(!service.shared_path.exists());
    std::fs::create_dir_all(service.project_path.parent().unwrap()).unwrap();
    std::fs::write(&service.project_path, b"preserved historical bytes").unwrap();
    let reopened = AtlasService::open_in(&raw, store);
    assert_eq!(reopened.status().health, AtlasHealth::Inert);
    assert!(
        reopened
            .add_operator(AtlasKind::Fact, "still rejected")
            .is_err()
    );
    assert_eq!(
        std::fs::read(&service.project_path).unwrap(),
        b"preserved historical bytes"
    );
}
