//! Staged for cockpit/src/caddy/storage_regression_tests.rs.
use super::*;
use std::cell::RefCell;

type TailHook = Box<dyn FnOnce(&Path)>;
thread_local! { static AFTER_METADATA: RefCell<Option<TailHook>> = RefCell::new(None); }

pub(super) fn after_tail_metadata(path: &Path) {
    if let Some(hook) = AFTER_METADATA.with(|slot| slot.borrow_mut().take()) {
        hook(path);
    }
}
struct HookGuard;
impl Drop for HookGuard {
    fn drop(&mut self) {
        AFTER_METADATA.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "angel-caddy-storage-{}-{}-{serial}",
            std::process::id(),
            now_ms()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn recipe(command: &str, note: &str, ts_ms: u64) -> Recipe {
    Recipe {
        ts_ms,
        command: command.into(),
        env: Vec::new(),
        duration_ms: None,
        tool: "run_tests".into(),
        note: note.into(),
        verification: Some(RecipeVerification::ExecutedVerifierV1),
        verified_head: None,
        stale_since_changes: 0,
        workspace_state: None,
        observed_workspace_state: None,
    }
}

#[test]
fn s05_recipe_redacts_verifier_args_before_json_command_encoding() {
    let _guard = crate::tests::env_lock();
    let call = ToolCall {
        id: "s05-encoded-args".into(),
        name: "cargo".into(),
        args: serde_json::json!({"args": "test -- HOSTILE_RECIPE\nsk-ABCDEFGHIJKLMNOPQRSTUV2345"}),
    };
    let (command, _) = command_of(&call).unwrap();
    assert!(command.contains("HOSTILE_RECIPE"));
    assert!(!command.contains("sk-ABCDEFGHIJKLMNOPQRSTUV2345"));
    let args: serde_json::Value =
        serde_json::from_str(command.strip_prefix("cargo ").unwrap()).unwrap();
    assert!(args["args"].as_str().unwrap().contains("«redacted»"));
}

#[test]
fn s05_secret_redaction_preserves_single_row_jsonl_during_lifecycle_refresh() {
    let _guard = crate::tests::env_lock();
    let fixture = Fixture::new();
    let path = fixture.0.join("recipes.jsonl");
    let hostile = recipe(
        "cargo test",
        "HOSTILE_RECIPE sk-ABCDEFGHIJKLMNOPQRSTUV2345",
        now_ms(),
    );
    // Reproduce the Python proof's legacy row and the state observation that
    // republishes it. Redaction must not turn this JSONL record into pretty JSON.
    std::fs::write(
        &path,
        format!("{}\n", serde_json::to_string(&hostile).unwrap()),
    )
    .unwrap();
    let state = serde_json::json!({"head":"unbound","tree_sha256":"unbound"});
    let report =
        storage::append::<Recipe>(&fixture.0, "recipes.jsonl", &[], Some(&state.to_string()));
    assert!(matches!(report.status, storage::WriteStatus::Published));
    let loaded = storage::load::<Recipe>(&path);
    assert_eq!(loaded.health.malformed_rows, 0);
    assert!(!loaded.health.unterminated_tail);
    assert_eq!(loaded.rows.len(), 1);
    assert!(loaded.rows[0].note.contains("HOSTILE_RECIPE"));
    let text = std::fs::read_to_string(path).unwrap();
    assert!(!text.contains("sk-ABCDEFGHIJKLMNOPQRSTUV2345"));
    assert_eq!(text.lines().count(), 1);
}

#[test]
fn same_batch_dedup_preserves_order_legacy_filter_and_day_buckets() {
    let _guard = crate::tests::env_lock();
    let fixture = Fixture::new();
    let now = now_ms();
    let mut legacy = recipe("legacy", "untrusted old record", now);
    legacy.verification = None;
    let initial = [
        recipe("known", "existing verifier", now),
        legacy,
        recipe("prior-day", "yesterday", now - 86_400_000),
    ];
    let path = fixture.0.join("recipes.jsonl");
    std::fs::write(
        &path,
        initial
            .iter()
            .map(|row| serde_json::to_string(row).unwrap() + "\n")
            .collect::<String>(),
    )
    .unwrap();
    let entries = [
        recipe("known", "repeat", now),
        recipe("fresh", "first fresh result", now),
        recipe("fresh", "later duplicate", now),
        recipe("legacy", "real verifier", now),
        recipe("legacy", "duplicate verifier", now),
        recipe("prior-day", "today", now),
    ];
    assert_eq!(
        append_new(&fixture.0, "recipes.jsonl", &entries, Path::new(".")),
        3
    );
    let rows: Vec<Recipe> = load_jsonl(&path);
    assert_eq!(rows.iter().filter(|row| row.command == "known").count(), 1);
    let fresh: Vec<_> = rows.iter().filter(|row| row.command == "fresh").collect();
    assert_eq!(fresh.len(), 1);
    assert_eq!(fresh[0].note, "first fresh result");
    assert_eq!(
        rows.iter()
            .filter(|row| row.command == "legacy" && row.verification.is_some())
            .count(),
        1
    );
    assert_eq!(
        rows.iter().filter(|row| row.command == "prior-day").count(),
        2
    );
    assert_eq!(
        append_new(&fixture.0, "recipes.jsonl", &entries, Path::new(".")),
        0
    );
    assert_eq!(rows.len(), load_jsonl::<Recipe>(&path).len());
}

#[test]
fn bounded_tail_ignores_growth_after_its_metadata_snapshot() {
    use std::io::Write as _;
    let fixture = Fixture::new();
    let path = fixture.0.join("growing.jsonl");
    std::fs::write(&path, b"old\n").unwrap();
    let expected = path.clone();
    let _hook = HookGuard;
    AFTER_METADATA.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move |observed| {
            assert_eq!(observed, expected);
            let mut writer = std::fs::OpenOptions::new()
                .append(true)
                .open(observed)
                .unwrap();
            writer.write_all(&vec![b'x'; 65_536]).unwrap();
        }))
    });
    assert_eq!(read_tail(&path, 4).as_deref(), Some("old\n"));
    assert_eq!(
        std::fs::metadata(&path).unwrap().len(),
        65_540,
        "the growth hook must actually execute"
    );
}

#[test]
fn bounded_tail_keeps_newest_complete_lines_and_honors_zero_budget() {
    let fixture = Fixture::new();
    let path = fixture.0.join("tail.jsonl");
    std::fs::write(&path, b"old row\nnew row\n").unwrap();
    assert_eq!(read_tail(&path, 10).as_deref(), Some("new row\n"));
    assert_eq!(read_tail(&path, 0).as_deref(), Some(""));
    assert_eq!(read_tail(&path, 100).as_deref(), Some("old row\nnew row\n"));
}

include!("lifecycle_regression_tests.rs");
