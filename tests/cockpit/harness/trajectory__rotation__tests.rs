use super::*;
struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "angel-p06c-rotation-{}-{}-{}",
            std::process::id(),
            super::super::now_ms(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn p06c_trajectory_rotation_enforces_size_and_emits_receipt() {
    let _lock = crate::tests::env_lock();
    let dir = Fixture::new();
    let path = dir.path().join("session-fixture.jsonl");
    let row = json!({"payload":"x".repeat(500)});
    for _ in 0..12 {
        append(&path, &row, 2048, Duration::from_secs(86400)).unwrap();
    }
    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.len() <= 2048);
    let rows: Vec<Value> = body
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert!(rows.iter().any(|r| {
        r["store_rotations"][0]["removed_files"]
            .as_u64()
            .unwrap_or(0)
            > 0
    }));
    assert_eq!(rows.last().unwrap()["payload"], row["payload"]);
    let previous = body;
    assert!(
        append(
            &path,
            &json!({"payload":"x".repeat(2048)}),
            2048,
            Duration::from_secs(86400)
        )
        .is_err()
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), previous);
}
#[test]
fn p06c_trajectory_rotation_age_evicts_old_shards_only() {
    let _lock = crate::tests::env_lock();
    let dir = Fixture::new();
    let old = dir.path().join("old.jsonl");
    append(&old, &json!({"old":true}), 4096, Duration::from_secs(86400)).unwrap();
    let file = std::fs::File::options().write(true).open(&old).unwrap();
    file.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1))
        .unwrap();
    let current = dir.path().join("new.jsonl");
    append(
        &current,
        &json!({"new":true}),
        4096,
        Duration::from_secs(86400),
    )
    .unwrap();
    assert!(!old.exists());
    let record: Value = serde_json::from_str(&std::fs::read_to_string(current).unwrap()).unwrap();
    assert_eq!(record["store_rotations"][0]["aged_files"], 1);
}
