//! M05 deliverable tests: growth ceiling + compaction, each corruption
//! class surfacing in the health summary, staleness marking / refresh /
//! expiry on the card, and the memory-health ledger event.
use super::*;

struct Fixture(PathBuf);

impl Fixture {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "angel-caddy-m05-{tag}-{}-{}",
            std::process::id(),
            now_ms()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn workspace(&self) -> PathBuf {
        let path = self.0.join("workspace");
        std::fs::create_dir_all(&path).unwrap();
        path
    }
    fn repo_dir(&self, workspace: &Path) -> PathBuf {
        let dir = self
            .0
            .join(crate::platform::workspace_store::repo_identity(workspace).key);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
    fn store(&self, workspace: &Path, rows: &[Recipe]) -> PathBuf {
        let dir = self.repo_dir(workspace);
        let path = dir.join("recipes.jsonl");
        let bytes = rows
            .iter()
            .map(|row| serde_json::to_string(row).unwrap() + "\n")
            .collect::<String>();
        std::fs::write(&path, bytes).unwrap();
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn recipe_at(ts_ms: u64, command: &str, head: Option<&str>, stale: u32) -> Recipe {
    Recipe {
        ts_ms,
        command: command.into(),
        env: vec![],
        duration_ms: None,
        tool: "run_tests".into(),
        note: "verified: ok".into(),
        verification: Some(RecipeVerification::ExecutedVerifierV1),
        verified_head: head.map(str::to_string),
        stale_since_changes: stale,
        workspace_state: head.map(|h| state(h, "a")),
        observed_workspace_state: head.map(|h| state(h, "a")),
    }
}

/// 1. Bounded growth: superseded rows (same command, older ts) are compacted
///    away on the next append, and the row ceiling is enforced.
#[test]
fn m05_compaction_drops_superseded_rows_on_append() {
    let _guard = crate::tests::env_lock();
    let fixture = Fixture::new("compact");
    let workspace = fixture.workspace();
    std::fs::write(workspace.join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
    let repo_dir = fixture.repo_dir(&workspace);
    fixture.store(
        &workspace,
        &[
            recipe_at(1, "cargo {\"args\":\"test\"}", Some("aaa1111"), 0),
            recipe_at(2, "cargo {\"args\":\"test\"}", Some("bbb2222"), 3),
            recipe_at(3, "cargo {\"args\":\"check\"}", Some("ccc3333"), 0),
        ],
    );
    // Append one new recipe: the two older "cargo test" rows are superseded
    // by the newest stored one; the published snapshot keeps one row per
    // command plus the new entry.
    let fresh = recipe_at(
        9_000_000_000_000,
        "cargo {\"args\":\"clippy\"}",
        Some("ddd4444"),
        0,
    );
    let report = storage::append(&repo_dir, "recipes.jsonl", &[fresh], Some("ddd4444"));
    assert!(
        matches!(report.status, storage::WriteStatus::Published),
        "{:?}",
        report.status
    );
    let rows = load_jsonl::<Recipe>(&repo_dir.join("recipes.jsonl"));
    let test_rows = rows.iter().filter(|r| r.command.contains("test")).count();
    assert_eq!(test_rows, 1, "superseded rows compacted: {rows:?}");
    assert_eq!(rows.len(), 3);
    // Readers keep working: the compacted file parses cleanly.
    assert!(
        !storage::load::<Recipe>(&repo_dir.join("recipes.jsonl"))
            .health
            .degraded()
    );
}

/// 1b. Entry ceiling: a store larger than MAX_STORE_ROWS is cut to the bound
/// by one append.
#[test]
fn m05_row_ceiling_is_enforced() {
    let _guard = crate::tests::env_lock();
    let fixture = Fixture::new("ceiling");
    let workspace = fixture.workspace();
    std::fs::write(workspace.join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
    let repo_dir = fixture.repo_dir(&workspace);
    // Minimal legacy rows fit below the byte ceiling, so this independently
    // exercises the entry limit. Reverse timestamps to test newest retention.
    let body = (0..storage::MAX_STORE_ROWS + 200).rev().map(|i| {
        serde_json::json!({"ts_ms":i,"command":format!("cmd-{i:05}"),"env":[],"tool":"","note":""}).to_string() + "\n"
    }).collect::<String>();
    assert!(body.len() < STORE_TAIL_BYTES as usize);
    std::fs::write(repo_dir.join("recipes.jsonl"), body).unwrap();
    let fresh = recipe_at(9_000_000_000_000, "cmd-fresh", None, 0);
    let report = storage::append(&repo_dir, "recipes.jsonl", &[fresh], None);
    assert!(
        matches!(report.status, storage::WriteStatus::Published),
        "{:?}",
        report.status
    );
    let after = load_jsonl::<Recipe>(&repo_dir.join("recipes.jsonl"));
    assert!(
        after.len() <= storage::MAX_STORE_ROWS,
        "row ceiling enforced: {} > {}",
        after.len(),
        storage::MAX_STORE_ROWS
    );
    assert!(after.iter().any(|r| r.command == "cmd-fresh"));
    assert_eq!(after.len(), storage::MAX_STORE_ROWS);
    assert!(!after.iter().any(|r| r.command == "cmd-00000"));
}

/// 2. Corruption classes surface in the health summary: a corrupt line, a
///    truncated (unterminated) tail, invalid UTF-8, and an IO failure.
#[test]
fn m05_health_summary_counts_each_corruption_class() {
    let _guard = crate::tests::env_lock();
    let _experience = crate::tests::TestEnvGuard::set("ANGEL_EXPERIENCE", "0");
    let fixture = Fixture::new("health");
    let workspace = fixture.workspace();

    // Clean store: healthy.
    fixture.store(&workspace, &[recipe_at(1, "cmd-ok", None, 0)]);
    let health = memory_health_in(&fixture.0, &workspace);
    assert!(health.healthy(), "{health:?}");

    // Corrupt line + truncated tail + invalid UTF-8.
    let dir = fixture.repo_dir(&workspace);
    std::fs::write(dir.join("recipes.jsonl"), b"{not json}\n").unwrap();
    std::fs::write(
        dir.join("hazards.jsonl"),
        b"{\"ts_ms\":1,\"command\":\"h\",\"diagnostic\":\"d\",\"tool\":\"shell\"}",
    )
    .unwrap();
    let health = memory_health_in(&fixture.0, &workspace);
    assert_eq!(health.malformed_rows, 1);
    assert!(health.incomplete_tail > 0, "hazards tail unterminated");
    assert!(!health.healthy());

    // Invalid UTF-8 row.
    std::fs::write(dir.join("recipes.jsonl"), b"\xff\xfe not utf8\n").unwrap();
    let health = memory_health_in(&fixture.0, &workspace);
    assert_eq!(health.invalid_utf8_rows, 1);

    // IO failure: replace the store with a directory.
    std::fs::remove_file(dir.join("recipes.jsonl")).unwrap();
    std::fs::create_dir(dir.join("recipes.jsonl")).unwrap();
    let health = memory_health_in(&fixture.0, &workspace);
    assert_eq!(health.lock_or_io_failures, 1);
}

fn state(head: &str, tree: &str) -> serde_json::Value {
    serde_json::json!({"head":head,"tree_sha256":tree.repeat(64),"dirty_paths_sha256":"0".repeat(64)})
}

#[test]
fn m05_state_changes_expire_and_same_day_execution_refreshes() {
    let _guard = crate::tests::env_lock();
    let fixture = Fixture::new("states");
    let workspace = fixture.workspace();
    let repo = fixture.repo_dir(&workspace);
    let initial = state("abc123", "a");
    let mut recipe = recipe_at(now_ms(), "cargo test", Some("abc123"), 0);
    recipe.workspace_state = Some(initial.clone());
    recipe.observed_workspace_state = Some(initial.clone());
    fixture.store(&workspace, &[recipe.clone()]);
    assert!(matches!(stale_mark(&recipe, &initial), StaleMark::Fresh));
    for i in 1..=RECIPE_STALE_CHANGE_LIMIT {
        // Content changes under an unchanged HEAD are separate observations.
        let changed = state("abc123", &i.to_string());
        for _ in 0..3 {
            let report =
                storage::append::<Recipe>(&repo, "recipes.jsonl", &[], Some(&changed.to_string()));
            assert!(matches!(
                report.status,
                storage::WriteStatus::Published | storage::WriteStatus::Unchanged
            ));
            let rows = load_jsonl::<Recipe>(&repo.join("recipes.jsonl"));
            assert_eq!(rows[0].stale_since_changes, i);
            if i < RECIPE_STALE_CHANGE_LIMIT {
                assert!(matches!(
                    stale_mark(&rows[0], &changed),
                    StaleMark::UnverifiedSince(_)
                ));
            } else {
                assert!(matches!(stale_mark(&rows[0], &changed), StaleMark::Expired));
            }
        }
    }
    let fresh_state = state("newhead", "b");
    recipe.ts_ms += 1;
    recipe.verified_head = Some("newhead".into());
    recipe.workspace_state = Some(fresh_state.clone());
    recipe.observed_workspace_state = Some(fresh_state.clone());
    let report = storage::append(
        &repo,
        "recipes.jsonl",
        &[recipe],
        Some(&fresh_state.to_string()),
    );
    assert_eq!(report.written, 1);
    let rows = load_jsonl::<Recipe>(&repo.join("recipes.jsonl"));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].stale_since_changes, 0);
    assert!(matches!(
        stale_mark(&rows[0], &fresh_state),
        StaleMark::Fresh
    ));
}

#[test]
fn m05_card_marks_and_expires_legacy_recipe_on_changes() {
    let _guard = crate::tests::env_lock();
    let _enabled = crate::tests::TestEnvGuard::set("ANGEL_CADDY", "1");
    let _experience = crate::tests::TestEnvGuard::set("ANGEL_EXPERIENCE", "0");
    let fixture = Fixture::new("card");
    let workspace = fixture.workspace();
    fixture.store(&workspace, &[recipe_at(1, "cmd-stale", Some("deadbee"), 0)]);
    let card = render_card_in(&fixture.0, &workspace, 4096);
    assert!(card.contains("unverified-since deadbee"), "{card}");
    fixture.store(
        &workspace,
        &[recipe_at(
            1,
            "cmd-stale",
            Some("deadbee"),
            RECIPE_STALE_CHANGE_LIMIT,
        )],
    );
    assert!(!render_card_in(&fixture.0, &workspace, 4096).contains("cmd-stale"));
}

#[test]
fn m05_write_failures_reach_classified_ledger() {
    let _guard = crate::tests::env_lock();
    let fixture = Fixture::new("failures");
    let workspace = fixture.workspace();
    let ledger = fixture.0.join("events.jsonl");
    let _log = crate::tests::TestEnvGuard::set("ANGEL_EXPERIENCE_LOG", ledger.to_str().unwrap());
    let _enabled = crate::tests::TestEnvGuard::set("ANGEL_EXPERIENCE", "1");
    let repo = fixture.repo_dir(&workspace);
    let lock = std::fs::File::create(repo.join("recipes.jsonl.lock")).unwrap();
    lock.lock().unwrap();
    let report = storage::append(&repo, "recipes.jsonl", &[recipe_at(1, "c", None, 0)], None);
    assert_eq!(report.status, storage::WriteStatus::Busy);
    expose_report(&workspace, "recipes.jsonl", &report);
    lock.unlock().unwrap();
    // A leftover unlocked lock file is reusable; it is not a stale lease.
    assert_eq!(
        storage::append(&repo, "recipes.jsonl", &[recipe_at(1, "c", None, 0)], None).written,
        1
    );
    let bad = fixture.0.join("not-directory");
    std::fs::write(&bad, "blocked").unwrap();
    let report = storage::append(&bad, "recipes.jsonl", &[recipe_at(2, "d", None, 0)], None);
    assert!(matches!(report.status, storage::WriteStatus::Failed(_)));
    expose_report(&workspace, "recipes.jsonl", &report);
    let rows: Vec<serde_json::Value> = std::fs::read_to_string(ledger)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["class"], "Transient");
    assert_eq!(rows[1]["class"], "Environment");
    assert_eq!(rows[1]["outcome"]["lock_or_io_failures"], 1);
}

#[test]
fn m05_real_tree_change_revalidation_and_both_card_expiries() {
    let _guard = crate::tests::env_lock();
    let fixture = Fixture::new("real-tree");
    let workspace = fixture.workspace();
    let _enabled = crate::tests::TestEnvGuard::set("ANGEL_CADDY", "1");
    let _store = crate::tests::TestEnvGuard::set("ANGEL_CADDY_DIR", fixture.0.to_str().unwrap());
    let _experience = crate::tests::TestEnvGuard::set("ANGEL_EXPERIENCE", "0");
    std::fs::write(
        workspace.join("Cargo.toml"),
        "[package]\nname=\"fixture\"\n",
    )
    .unwrap();
    let source = workspace.join("source.rs");
    std::fs::write(&source, "first").unwrap();
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(&workspace)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    git(&["init", "-q"]);
    git(&["add", "."]);
    let tree = git(&["write-tree"]);
    let commit = git(&[
        "-c",
        "user.name=m05",
        "-c",
        "user.email=m05@invalid",
        "commit-tree",
        &tree,
        "-m",
        "fixture",
    ]);
    git(&["update-ref", "HEAD", &commit]);
    let call = ToolCall {
        id: "verify".into(),
        name: "cargo".into(),
        args: serde_json::json!({"args":"test"}),
    };
    let receipt = || {
        ChatMsg::tool("verify", "1 passed; 0 failed")
            .with_tool_receipt(
                &call,
                crate::agent::harness::ToolOutcome {
                    execution: crate::agent::harness::ExecutionOutcome::Succeeded,
                    verification: crate::agent::harness::VerificationOutcome::Passed,
                },
            )
            .with_verified_workspace(&workspace, true)
    };
    let history = vec![ChatMsg::assistant_calls(vec![call.clone()]), receipt()];
    assert_eq!(write_back_from_history(&workspace, &history), (1, 0));
    assert_eq!(write_back_from_history(&workspace, &history), (0, 0));
    let original = workspace_state(&workspace);
    for i in 1..=RECIPE_STALE_CHANGE_LIMIT {
        std::fs::write(&source, format!("changed {i}")).unwrap();
        let changed = workspace_state(&workspace);
        assert_eq!(original["head"], changed["head"]);
        assert_ne!(original["tree_sha256"], changed["tree_sha256"]);
        for _ in 0..2 {
            let full = render_card_in(&fixture.0, &workspace, 4096);
            let relevant = render_relevant_card_in(&fixture.0, &workspace, 4096);
            for card in [full, relevant] {
                if i < RECIPE_STALE_CHANGE_LIMIT {
                    assert!(card.contains("unverified-since"), "{card}");
                } else {
                    assert!(!card.contains("cargo {"), "{card}");
                }
            }
        }
    }
    // Replaying old evidence after edits cannot rebind it to the new tree.
    assert_eq!(write_back_from_history(&workspace, &history), (0, 0));
    let refreshed = vec![ChatMsg::assistant_calls(vec![call.clone()]), receipt()];
    assert_eq!(write_back_from_history(&workspace, &refreshed), (1, 0));
    for card in [
        render_card_in(&fixture.0, &workspace, 4096),
        render_relevant_card_in(&fixture.0, &workspace, 4096),
    ] {
        assert!(card.contains("cargo {"), "{card}");
        assert!(!card.contains("unverified-since"), "{card}");
    }
}

#[test]
fn m05_admission_preserves_the_latest_end_of_long_histories() {
    let _guard = crate::tests::env_lock();
    let fixture = Fixture::new("admission");
    let workspace = fixture.workspace();
    let repo = fixture.repo_dir(&workspace);
    let rows: Vec<Hazard> = (0..1500)
        .map(|i| Hazard {
            ts_ms: i,
            command: format!("command-{i}"),
            diagnostic: "failed".into(),
            tool: "shell".into(),
        })
        .collect();
    let report = storage::append(&repo, "hazards.jsonl", &rows, None);
    assert_eq!(report.written, 1024);
    assert_eq!(report.skipped_new, 476);
    let kept = load_jsonl::<Hazard>(&repo.join("hazards.jsonl"));
    assert_eq!(kept.first().unwrap().command, "command-476");
    assert_eq!(kept.last().unwrap().command, "command-1499");
}
