use super::*;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "angel-caddy-staleness-{}-{}-{}",
            std::process::id(),
            now_ms(),
            SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn workspace(&self) -> PathBuf {
        let path = self.0.join("workspace");
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn store(&self, workspace: &Path, rows: &[Recipe]) -> PathBuf {
        let repo = self
            .0
            .join(crate::platform::workspace_store::repo_identity(workspace).key);
        std::fs::create_dir_all(&repo).unwrap();
        let path = repo.join("recipes.jsonl");
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

fn recipe(ts_ms: u64) -> Recipe {
    Recipe {
        ts_ms,
        command: "run_tests --owned-source".into(),
        env: vec![],
        duration_ms: None,
        tool: "run_tests".into(),
        note: "verified: ok".into(),
        verification: Some(RecipeVerification::ExecutedVerifierV1),
        verified_head: None,
        stale_since_changes: 0,
        workspace_state: None,
        observed_workspace_state: None,
    }
}

#[test]
fn source_changes_keep_historical_hint_but_require_revalidation_without_rewriting_store() {
    let _lock = crate::tests::env_lock();
    let _enabled = crate::tests::TestEnvGuard::unset("ANGEL_CADDY");
    let fixture = Fixture::new();
    let workspace = fixture.workspace();
    let source = workspace.join("owned.rs");
    std::fs::write(&source, "fn answer() -> u32 { 1 }\n").unwrap();
    let path = fixture.store(&workspace, &[recipe(1_700_000_000_000)]);
    let original = std::fs::read(&path).unwrap();
    let before = render_card_in(&fixture.0, &workspace, 4096);
    std::fs::write(&source, "fn answer() -> u32 { 2 }\n").unwrap();
    let after = render_card_in(&fixture.0, &workspace, 4096);
    for card in [&before, &after] {
        assert!(
            card.contains("run_tests --owned-source"),
            "historical hint retained: {card}"
        );
        assert!(
            card.contains("historical; source unbound; rerun before relying"),
            "current verification is unsupported: {card}"
        );
        assert!(
            card.contains("saved 2023-11-14"),
            "timestamp labels persistence rather than execution: {card}"
        );
    }
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn duplicate_stored_records_do_not_claim_independent_verifier_reruns() {
    let _lock = crate::tests::env_lock();
    let _enabled = crate::tests::TestEnvGuard::unset("ANGEL_CADDY");
    let fixture = Fixture::new();
    let workspace = fixture.workspace();
    fixture.store(
        &workspace,
        &[recipe(1_700_000_000_000), recipe(1_700_086_400_000)],
    );
    let card = render_card_in(&fixture.0, &workspace, 4096);
    assert!(
        card.contains("records ×2"),
        "count is stored records: {card}"
    );
    assert!(
        !card.contains("ok ×"),
        "replayed history is not another successful run: {card}"
    );
    assert_eq!(card.matches("run_tests --owned-source").count(), 1);
}

#[test]
fn cap_never_separates_retained_recipe_from_revalidation_policy() {
    let _lock = crate::tests::env_lock();
    let _enabled = crate::tests::TestEnvGuard::unset("ANGEL_CADDY");
    let fixture = Fixture::new();
    let workspace = fixture.workspace();
    fixture.store(&workspace, &[recipe(1_700_000_000_000)]);
    let mut retained = false;
    for cap in [0, 80, 128, 256, 512, 1536] {
        let card = render_card_in(&fixture.0, &workspace, cap);
        assert!(card.len() <= cap);
        assert!(
            card.lines()
                .all(|line| line.chars().count() <= MAX_LINE_CHARS)
        );
        if card.contains("run_tests --owned-source") {
            retained = true;
            assert!(
                card.contains("historical; source unbound; rerun before relying"),
                "{card}"
            );
        }
    }
    assert!(
        retained,
        "ordinary card budgets retain valid historical hints"
    );
}

#[test]
fn typed_historical_hint_remains_visible_while_untyped_legacy_stays_excluded() {
    let _lock = crate::tests::env_lock();
    let _enabled = crate::tests::TestEnvGuard::unset("ANGEL_CADDY");
    let fixture = Fixture::new();
    let workspace = fixture.workspace();
    let mut legacy = recipe(1_700_000_000_000);
    legacy.command = "untyped-legacy-command".into();
    legacy.verification = None;
    let path = fixture.store(&workspace, &[recipe(1_700_000_000_000), legacy]);
    let original = std::fs::read(&path).unwrap();
    let card = render_card_in(&fixture.0, &workspace, 1536);
    assert!(card.contains("run_tests --owned-source"));
    assert!(!card.contains("untyped-legacy-command"));
    assert_eq!(std::fs::read(path).unwrap(), original);
}
