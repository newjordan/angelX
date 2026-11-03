//! The host services stand on their own: no cockpit types, no shared env.
use super::*;
use std::path::PathBuf;

struct Dir(PathBuf);
impl Dir {
    fn new(tag: &str) -> Self {
        static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "angel-live-host-{tag}-{}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn sha256_hex_is_the_lowercase_digest() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn workspace_keys_are_stable_and_filesystem_safe() {
    let key = workspace_store::workspace_key(std::path::Path::new("/home/user/my project"));
    assert!(key.starts_with("home-user-my-project-"), "{key}");
    assert_eq!(
        key,
        workspace_store::workspace_key(std::path::Path::new("/home/user/my project"))
    );
    assert!(workspace_store::workspace_key(std::path::Path::new("/")).starts_with("ws-"));
}

#[cfg(unix)]
#[test]
fn atomic_writers_keep_their_modes_and_the_private_tier_redacts() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = Dir::new("atomic");
    let private = dir.0.join("deep/state.json");
    workspace_store::write_private_atomic_exact(
        &private,
        b"{\"token\":\"keep-me-verbatim-123456789\"}",
    )
    .unwrap();
    assert_eq!(
        std::fs::metadata(&private).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::read(&private).unwrap(),
        b"{\"token\":\"keep-me-verbatim-123456789\"}"
    );
    workspace_store::write_private_atomic(&private, b"{\"token\":\"keep-me-verbatim-123456789\"}")
        .unwrap();
    let redacted = std::fs::read_to_string(&private).unwrap();
    assert!(!redacted.contains("keep-me-verbatim"), "{redacted}");
    let bulk = dir.0.join("bulk.bin");
    workspace_store::write_atomic(&bulk, b"bulk").unwrap();
    assert_eq!(std::fs::read(&bulk).unwrap(), b"bulk");
    let leftovers: Vec<_> = std::fs::read_dir(&dir.0)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty());
}

#[test]
fn the_private_lock_is_exclusive_until_dropped() {
    let dir = Dir::new("lock");
    let path = dir.0.join("state.json");
    let guard = workspace_store::try_private_lock(&path).unwrap();
    assert!(workspace_store::try_private_lock(&path).is_err());
    drop(guard);
    assert!(dir.0.join(".state.json.lock").is_file());
    workspace_store::try_private_lock(&path).unwrap();
}

#[test]
fn one_shot_faults_fail_the_publish_without_losing_the_old_state() {
    let dir = Dir::new("fault");
    let path = dir.0.join("state.json");
    workspace_store::write_private_atomic(&path, b"old").unwrap();
    workspace_store::fault::AFTER_RENAME.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(|_| Err("injected".into())));
    });
    assert_eq!(
        workspace_store::write_private_atomic(&path, b"new"),
        Err("injected".to_string())
    );
    workspace_store::write_private_atomic(&path, b"next").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"next");
}

#[test]
fn shaped_secrets_are_redacted_and_plain_text_is_borrowed() {
    let text = "token ghp_abcdefghijklmnopqrstuvwxyz0123456789 end";
    let clean = secrets::redact_str(text);
    assert!(!clean.contains("ghp_"), "{clean}");
    assert!(matches!(
        secrets::redact_str("nothing to hide"),
        std::borrow::Cow::Borrowed(_)
    ));
    assert!(secrets::is_secret_name("ANGEL_API_KEY"));
    assert!(!secrets::is_secret_name("ANGEL_HOME"));
}

#[test]
fn resolve_root_prefers_the_configured_folder_then_the_install_then_the_source() {
    let dir = Dir::new("root");
    let bundle = dir.0.join("prefix/share/angelX/bundles/id");
    std::fs::create_dir_all(bundle.join("cockpit")).unwrap();
    std::fs::create_dir_all(bundle.join("scripts")).unwrap();
    let exe = dir.0.join("prefix/bin/angel");
    let absent = std::path::Path::new("/absent/build/cockpit");
    assert_eq!(
        runtime_paths::resolve_root(None, Some(&exe), Some(absent), "id"),
        bundle
    );
    assert_eq!(
        runtime_paths::resolve_root(Some(dir.0.clone()), Some(&exe), Some(absent), "id"),
        dir.0
    );
    assert_eq!(
        runtime_paths::resolve_root(None, None, Some(absent), "id"),
        PathBuf::from("/absent/build")
    );
    assert_eq!(
        runtime_paths::resolve_root(None, None, None, "id"),
        PathBuf::from(".")
    );
}

#[test]
fn a_heartbeat_goes_stale_after_the_window() {
    let beat = peer_watch::Heartbeat {
        schema: "angelX.heartbeat/v1".into(),
        instance_id: "a".into(),
        host: "h".into(),
        pid: 1,
        version: "0".into(),
        started_ms: 0,
        updated_ms: 1_000,
        workspace: "/w".into(),
        session_id: None,
        activity: "idle".into(),
        detail: None,
        cpu_pct: 0.0,
        mem_pct: 0.0,
        gpu_pct: 0.0,
    };
    assert!(beat.is_live(1_000 + peer_watch::STALE_AFTER_MS));
    assert!(!beat.is_live(1_001 + peer_watch::STALE_AFTER_MS));
    assert_eq!(beat.age_ms(500), 0);
}
