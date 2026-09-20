use super::*;

/// Env guards that keep the fixture's overrides alive for the test body
/// (`TestEnvGuard` restores on drop, so it must outlive the assertions).
struct FixtureEnv {
    _caddy: crate::tests::TestEnvGuard,
    _dossier: crate::tests::TestEnvGuard,
    _on: crate::tests::TestEnvGuard,
}

fn fixture(tag: &str) -> (PathBuf, PathBuf, PathBuf, FixtureEnv) {
    let base = std::env::temp_dir().join(format!("angel-caddy-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let caddy_dir = base.join("caddy");
    let dossier_dir = base.join("dossier");
    let workspace = base.join("repo");
    for dir in [&caddy_dir, &dossier_dir, &workspace] {
        std::fs::create_dir_all(dir).unwrap();
    }
    std::fs::write(workspace.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
    let caddy_text = caddy_dir.to_string_lossy().into_owned();
    let dossier_text = dossier_dir.to_string_lossy().into_owned();
    let env = FixtureEnv {
        _caddy: crate::tests::TestEnvGuard::set("ANGEL_CADDY_DIR", &caddy_text),
        _dossier: crate::tests::TestEnvGuard::set("ANGEL_DOSSIER_DIR", &dossier_text),
        _on: crate::tests::TestEnvGuard::unset("ANGEL_CADDY"),
    };
    (caddy_dir, dossier_dir, workspace, env)
}

#[test]
fn startup_walk_caddy_skip_events_distinguish_repeated_invocations() {
    let _guard = crate::tests::env_lock();
    let (dir, _, workspace, _env) = fixture("startup-skip-events");
    let ledger = dir.join("experience.jsonl");
    let _enabled = crate::tests::TestEnvGuard::set("ANGEL_EXPERIENCE", "1");
    let _log = crate::tests::TestEnvGuard::set("ANGEL_EXPERIENCE_LOG", &ledger.to_string_lossy());
    record_skip(&workspace);
    record_skip(&workspace);
    let rows: Vec<serde_json::Value> = std::fs::read_to_string(&ledger)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["outcome"], rows[1]["outcome"]);
    assert_eq!(rows[0]["pid"], rows[1]["pid"]);
    assert_ne!(rows[0]["seq"], rows[1]["seq"]);
    assert_ne!(rows[0], rows[1]);
}

#[test]
fn startup_walk_caddy_complete_state_matches_verifier_identity() {
    let _guard = crate::tests::env_lock();
    let _budget = crate::tests::TestEnvGuard::set("ANGEL_STARTUP_WALK_MS", "10000");
    let (dir, _, workspace, _env) = fixture("startup-identity");
    for args in [
        vec!["init", "-q"],
        vec!["add", "."],
        vec![
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "fixture",
        ],
    ] {
        assert!(
            std::process::Command::new("git")
                .arg("-C")
                .arg(&workspace)
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    let expected = workspace_state(&workspace);
    assert_eq!(observe_workspace(&dir.join("store"), &workspace), expected);
}

fn seed_store(
    caddy_dir: &Path,
    workspace: &Path,
    recipes: &[Recipe],
    hazards: &[Hazard],
) -> PathBuf {
    let repo_dir = caddy_dir.join(crate::workspace_store::repo_identity(workspace).key);
    std::fs::create_dir_all(&repo_dir).unwrap();
    let mut body = String::new();
    for recipe in recipes {
        body.push_str(&serde_json::to_string(recipe).unwrap());
        body.push('\n');
    }
    std::fs::write(repo_dir.join("recipes.jsonl"), &body).unwrap();
    let mut body = String::new();
    for hazard in hazards {
        body.push_str(&serde_json::to_string(hazard).unwrap());
        body.push('\n');
    }
    std::fs::write(repo_dir.join("hazards.jsonl"), &body).unwrap();
    repo_dir
}

fn seed_dossier(dossier_dir: &Path, workspace: &Path) {
    let key = crate::workspace_store::repo_identity(workspace).key;
    std::fs::write(
        dossier_dir.join(format!("{key}.json")),
        serde_json::json!({
            "facts": [
                { "kind": "ritual", "class": "test", "text": "cargo test --release",
                  "belief": 0.9 },
                { "kind": "trap", "text": "npm test", "belief": 0.2 },
            ],
        })
        .to_string(),
    )
    .unwrap();
}

#[test]
fn caddy_render_cap_drops_doors_first_and_keeps_newest_recipes() {
    let _guard = crate::tests::env_lock();
    let (caddy_dir, dossier_dir, workspace, _env) = fixture("cap");
    seed_dossier(&dossier_dir, &workspace);
    let recipes: Vec<Recipe> = (0..30)
        .map(|i| Recipe {
            ts_ms: 1_700_000_000_000 + i * 86_400_000,
            command: format!("cmd-{i:03} --flag"),
            env: Vec::new(),
            duration_ms: Some(i * 1_000),
            tool: "shell".to_string(),
            note: "verified: ok".to_string(),
            verification: Some(RecipeVerification::ExecutedVerifierV1),
            verified_head: None,
            stale_since_changes: 0,
            workspace_state: None,
            observed_workspace_state: None,
        })
        .collect();
    let hazards: Vec<Hazard> = (0..30)
        .map(|i| Hazard {
            ts_ms: 1_700_000_000_000 + i * 86_400_000,
            command: format!("haz-{i:03} run"),
            diagnostic: "boom".to_string(),
            tool: "shell".to_string(),
        })
        .collect();
    seed_store(&caddy_dir, &workspace, &recipes, &hazards);

    let cap = 600;
    let card = render_card(&workspace, cap);
    assert!(!card.is_empty(), "known recipes must render");
    assert!(card.len() <= cap, "card is {} bytes > {cap}", card.len());
    assert!(
        card.lines()
            .all(|line| line.chars().count() <= MAX_LINE_CHARS),
        "no split lines: {card}"
    );
    assert!(card.contains("[caddy ·"), "header survives: {card}");
    // Languages come from the shared shallow scan and the header names the
    // host runtimes so the model never guesses `python` vs `python3`.
    assert!(card.contains("· rust"), "langs from workspace_lang: {card}");
    assert!(card.contains("· host: "), "host runtimes in header: {card}");
    assert!(card.contains("cmd-029"), "newest recipe survives: {card}");
    assert!(!card.contains("doors:"), "doors drop first: {card}");
    assert!(
        card.contains("recipes (historical; source unbound; rerun before relying):"),
        "{card}"
    );
    let _ = std::fs::remove_dir_all(caddy_dir.parent().unwrap());
}

#[test]
fn caddy_write_back_classifies_and_is_idempotent() {
    let _guard = crate::tests::env_lock();
    let (caddy_dir, _dossier_dir, workspace, _env) = fixture("writeback");
    let call = ToolCall {
        id: "c1".to_string(),
        name: "run_tests".to_string(),
        args: serde_json::json!({"args": "--release"}),
    };
    let receipt = ChatMsg::tool("c1", "1 passed; 0 failed").with_tool_receipt(
        &call,
        crate::harness::ToolOutcome {
            execution: crate::harness::ExecutionOutcome::Succeeded,
            verification: crate::harness::VerificationOutcome::Passed,
        },
    );
    let history = vec![
        ChatMsg::user("run it"),
        ChatMsg::assistant_calls(vec![call]),
        receipt,
        ChatMsg::assistant_calls(vec![ToolCall {
            id: "c2".to_string(),
            name: "shell".to_string(),
            args: serde_json::json!({ "command": "swift test" }),
        }]),
        ChatMsg::tool("c2", "tool error: sandbox_apply denied write to /Users/x"),
        ChatMsg::assistant_calls(vec![ToolCall {
            id: "c3".to_string(),
            name: "shell".to_string(),
            args: serde_json::json!({ "command": "ls" }),
        }]),
        ChatMsg::tool("c3", "file1\nfile2"),
    ];

    let (recipes, hazards) = write_back_from_history(&workspace, &history);
    assert_eq!((recipes, hazards), (1, 1));
    let repo_dir = caddy_dir.join(crate::workspace_store::repo_identity(&workspace).key);
    let stored: Vec<Recipe> = load_jsonl(&repo_dir.join("recipes.jsonl"));
    assert_eq!(stored.len(), 1);
    assert!(stored[0].command.starts_with("run_tests "));
    assert!(stored[0].env.is_empty());
    assert!(stored[0].verification.is_some());
    assert_eq!(stored[0].note, "verified: ok");
    assert_eq!(stored[0].duration_ms, None);
    let stored: Vec<Hazard> = load_jsonl(&repo_dir.join("hazards.jsonl"));
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].command, "swift test");
    assert!(
        stored[0].diagnostic.contains("sandbox_apply denied"),
        "{}",
        stored[0].diagnostic
    );
    assert!(stored[0].diagnostic.len() <= MAX_DIAGNOSTIC_CHARS);

    let again = write_back_from_history(&workspace, &history);
    assert_eq!(again, (0, 0), "same history on the same day adds nothing");
    let _ = std::fs::remove_dir_all(caddy_dir.parent().unwrap());
}

#[test]
fn cargo_run_tests_seed_stores_verified_recipe_command() {
    let _guard = crate::tests::env_lock();
    let (caddy_dir, _dossier_dir, workspace, _env) = fixture("cargo-seed-recipe");
    let call = ToolCall {
        id: "c1".to_string(),
        name: "run_tests".to_string(),
        args: serde_json::json!({
            "args": "--locked --test contract policy_jade",
            "runtime": "rust"
        }),
    };
    let receipt = ChatMsg::tool("c1", "tests: 1 passed, 0 failed, 0 ignored — reward 1.00")
        .with_tool_receipt(
            &call,
            crate::harness::ToolOutcome {
                execution: crate::harness::ExecutionOutcome::Succeeded,
                verification: crate::harness::VerificationOutcome::Passed,
            },
        );
    let history = vec![
        ChatMsg::user("verify the selected policy"),
        ChatMsg::assistant_calls(vec![call]),
        receipt,
    ];
    let (recipes, hazards) = write_back_from_history(&workspace, &history);
    assert_eq!((recipes, hazards), (1, 0));
    let repo_dir = caddy_dir.join(crate::workspace_store::repo_identity(&workspace).key);
    let stored: Vec<Recipe> = load_jsonl(&repo_dir.join("recipes.jsonl"));
    assert_eq!(stored.len(), 1);
    assert!(
        stored[0].command.contains("runtime")
            && stored[0].command.contains("rust")
            && stored[0].command.contains("--test contract policy_jade"),
        "{}",
        stored[0].command
    );
    assert!(
        stored[0].verification.is_some(),
        "{:?}",
        stored[0].verification
    );
    let _ = std::fs::remove_dir_all(caddy_dir.parent().unwrap());
}

#[test]
fn t04b_caddy_routed_shell_recipe() {
    let _guard = crate::tests::env_lock();
    let (caddy_dir, _dossier_dir, workspace, _env) = fixture("t04b");
    let call = ToolCall {
        id: "route".into(),
        name: "shell".into(),
        args: serde_json::json!({"command":"cd . && env TEST_MODE=fixture npm test"}),
    };
    let route = crate::harness::shell_verifier::plan("shell", &call.args, &workspace).unwrap();
    let receipt = ChatMsg::tool("route", "tests: 1 passed, 0 failed")
        .with_tool_receipt(
            &call,
            crate::harness::ToolOutcome {
                execution: crate::harness::ExecutionOutcome::Succeeded,
                verification: crate::harness::VerificationOutcome::Passed,
            },
        )
        .with_routing_receipt(Some(crate::harness::shell_verifier::RoutingReceipt {
            routed_call: Some(route.call),
            routed_cwd: Some(workspace.clone()),
            reason: "argv routed".into(),
        }));
    let history = vec![ChatMsg::assistant_calls(vec![call]), receipt];
    assert_eq!(write_back_from_history(&workspace, &history), (1, 0));
    let recipes: Vec<Recipe> = load_jsonl(
        &caddy_dir
            .join(crate::workspace_store::repo_identity(&workspace).key)
            .join("recipes.jsonl"),
    );
    assert!(recipes[0].command.starts_with("run_tests "));
    assert!(recipes[0].command.contains("\"dir\":\".\""));
    assert_eq!(recipes[0].env, vec!["TEST_MODE=fixture"]);
    assert!(recipes[0].command.contains("node"));
    let _ = std::fs::remove_dir_all(caddy_dir.parent().unwrap());
}

#[test]
fn caddy_kill_switch_blanks_render_and_write_back() {
    let _guard = crate::tests::env_lock();
    let (caddy_dir, _dossier_dir, workspace, _env) = fixture("kill");
    seed_store(
        &caddy_dir,
        &workspace,
        &[Recipe {
            ts_ms: 1_700_000_000_000,
            command: "./benchmark.sh".to_string(),
            env: Vec::new(),
            duration_ms: None,
            tool: "shell".to_string(),
            note: "verified: ok".to_string(),
            verification: Some(RecipeVerification::ExecutedVerifierV1),
            verified_head: None,
            stale_since_changes: 0,
            workspace_state: None,
            observed_workspace_state: None,
        }],
        &[],
    );
    let history = vec![ChatMsg::assistant_calls(vec![ToolCall {
        id: "c1".to_string(),
        name: "shell".to_string(),
        args: serde_json::json!({ "command": "./benchmark.sh" }),
    }])];
    let _off = crate::tests::TestEnvGuard::set("ANGEL_CADDY", "0");
    assert_eq!(render_card(&workspace, card_cap()), "");
    assert_eq!(write_back_from_history(&workspace, &history), (0, 0));
    let _ = std::fs::remove_dir_all(caddy_dir.parent().unwrap());
}

/// M06b: the default card is cost-aware — only entries relevant to the
/// workspace's verifier family, never doors, ≤ 400 B; a store with
/// nothing relevant renders nothing and the ledger records the skip.
#[test]
fn m06b_relevant_card_injects_only_matching_entries_and_ledgers_skip() {
    let _guard = crate::tests::env_lock();
    let (caddy_dir, dossier_dir, workspace, _env) = fixture("m06b");
    seed_dossier(&dossier_dir, &workspace); // doors must NOT appear
    seed_store(
        &caddy_dir,
        &workspace,
        &[
            Recipe {
                ts_ms: 1_700_000_000_000,
                command: "cargo {\"args\":\"test --quiet\"}".to_string(),
                env: Vec::new(),
                duration_ms: None,
                tool: "cargo".to_string(),
                note: "verified: ok".to_string(),
                verification: Some(RecipeVerification::ExecutedVerifierV1),
                verified_head: None,
                stale_since_changes: 0,
                workspace_state: None,
                observed_workspace_state: None,
            },
            Recipe {
                ts_ms: 1_700_000_100_000,
                command: "run_tests {\"args\":\"\",\"runtime\":\"node\"}".to_string(),
                env: Vec::new(),
                duration_ms: None,
                tool: "run_tests".to_string(),
                note: "verified: ok".to_string(),
                verification: Some(RecipeVerification::ExecutedVerifierV1),
                verified_head: None,
                stale_since_changes: 0,
                workspace_state: None,
                observed_workspace_state: None,
            },
            Recipe {
                ts_ms: 1_700_000_200_000,
                command: "cargo {\"args\":\"build --release\"}".to_string(),
                env: Vec::new(),
                duration_ms: None,
                tool: "cargo".to_string(),
                note: "verified: ok".to_string(),
                verification: Some(RecipeVerification::ExecutedVerifierV1),
                verified_head: None,
                stale_since_changes: 0,
                workspace_state: None,
                observed_workspace_state: None,
            },
        ],
        &[Hazard {
            ts_ms: 1_700_000_000_000,
            command: "cargo {\"args\":\"build\"}".to_string(),
            diagnostic: "offline".to_string(),
            tool: "cargo".to_string(),
        }],
    );

    // Rust workspace (fixture writes Cargo.toml): the node recipe and the
    // cargo build hazard are irrelevant; cargo test is relevant.
    let _card_env = crate::tests::TestEnvGuard::unset("ANGEL_CADDY_CARD");
    let _defaults = crate::tests::TestEnvGuard::unset("ANGEL_FEATURE_DEFAULTS");
    let card = render_card_for_task(&workspace, card_cap());
    assert!(
        card.contains("cargo {\"args\":\"test"),
        "relevant cargo test recipe: {card}"
    );
    assert!(!card.contains("node"), "irrelevant runtime hidden: {card}");
    assert!(
        !card.contains("build --release"),
        "build recipe is not a test verifier: {card}"
    );
    assert!(
        !card.contains("doors:"),
        "cost-aware card carries no doors: {card}"
    );
    assert!(
        card.len() <= RELEVANT_CARD_BYTES,
        "card {} B > {RELEVANT_CARD_BYTES}",
        card.len()
    );

    // A store with nothing relevant: nothing injected, skip ledgered.
    let mut ledger =
        std::env::temp_dir().join(format!("angel-caddy-m06b-skip-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ledger);
    ledger.push("ledger.jsonl");
    let _exp = crate::tests::TestEnvGuard::set("ANGEL_EXPERIENCE_LOG", ledger.to_str().unwrap());
    let recipes: Vec<Recipe> = ["python", "node"]
        .into_iter()
        .map(|runtime| Recipe {
            ts_ms: 1_700_000_000_000,
            command: format!("run_tests {{\"args\":\"\",\"runtime\":\"{runtime}\"}}"),
            env: Vec::new(),
            duration_ms: None,
            tool: "run_tests".to_string(),
            note: "verified: ok".to_string(),
            verification: Some(RecipeVerification::ExecutedVerifierV1),
            verified_head: None,
            stale_since_changes: 0,
            workspace_state: None,
            observed_workspace_state: None,
        })
        .collect();
    seed_store(&caddy_dir, &workspace, &recipes, &[]);
    assert_eq!(render_card_for_task(&workspace, card_cap()), "");
    let body = std::fs::read_to_string(&ledger).unwrap_or_default();
    assert!(
        body.contains("\"kind\":\"caddy_card\"")
            && body.contains("caddy: skipped (no relevant entry)"),
        "skip must be ledgered: {body}"
    );

    // ANGEL_CADDY_CARD=full restores the full pre-M06b card.
    let _full_card = crate::tests::TestEnvGuard::set("ANGEL_CADDY_CARD", "full");
    let card = render_card_for_task(&workspace, card_cap());
    assert!(card.contains("node"), "full card shows everything: {card}");
    assert!(card.contains("doors:"), "full card keeps doors: {card}");
    let _ = std::fs::remove_dir_all(caddy_dir.parent().unwrap());
    let _ = std::fs::remove_file(&ledger);
}

/// The Rust-side ruling matches the cohort's `recipe_relevance` labels.
#[test]
fn m06b_recipe_relevance_matches_the_cohort_ruling() {
    let families = ["cargo-test"];
    assert_eq!(
        recipe_relevance("cargo {\"args\":\"test --quiet\"}", &families),
        Some("typed-equivalent")
    );
    assert_eq!(
        recipe_relevance("cargo {\"args\":\"build\"}", &families),
        None
    );
    assert_eq!(
        recipe_relevance("run_tests {\"args\":\"\",\"runtime\":\"rust\"}", &families),
        Some("typed-equivalent")
    );
    assert_eq!(
        recipe_relevance("run_tests {\"args\":\"\",\"runtime\":\"node\"}", &families),
        None
    );
    assert_eq!(recipe_relevance("echo hi", &families), None);
    assert_eq!(recipe_relevance("./benchmark.sh", &families), None);
    // Compound shell args never establish relevance.
    assert_eq!(
        recipe_relevance("run_tests {\"args\":\"a; rm -rf / && b\"}", &families),
        None
    );
    let py = ["python-test"];
    assert_eq!(
        recipe_relevance("run_tests {\"args\":\"\"}", &py),
        Some("typed-equivalent")
    );
}

/// The retirement switch: a receipt file with default = "off" disables the
/// card when no explicit env flag is set, and an explicit flag outranks it.
#[test]
fn m06b_receipt_file_can_retire_the_card() {
    let _guard = crate::tests::env_lock();
    let (caddy_dir, _dossier_dir, workspace, _env) = fixture("receipt");
    let base = caddy_dir.parent().unwrap();
    std::fs::write(
            base.join("defaults.toml"),
            "[[features]]\nname = \"unrelated-before\"\ndefault = \"on\"\n\
             [[features]]\ndefault = \"off\" # key order and comments are valid TOML\nname = \"caddy\"\n\
             [[features]]\nname = \"unrelated-after\"\ndefault = \"full\"\n",
        )
        .unwrap();
    let _defaults = crate::tests::TestEnvGuard::set(
        "ANGEL_FEATURE_DEFAULTS",
        base.join("defaults.toml").to_str().unwrap(),
    );
    let _unset = crate::tests::TestEnvGuard::unset("ANGEL_CADDY");
    let _card = crate::tests::TestEnvGuard::unset("ANGEL_CADDY_CARD");
    seed_store(
        &caddy_dir,
        &workspace,
        &[Recipe {
            ts_ms: 1_700_000_000_000,
            command: "run_tests {\"args\":\"\",\"runtime\":\"rust\"}".to_string(),
            env: Vec::new(),
            duration_ms: None,
            tool: "run_tests".to_string(),
            note: "verified: ok".to_string(),
            verification: Some(RecipeVerification::ExecutedVerifierV1),
            verified_head: None,
            stale_since_changes: 0,
            workspace_state: None,
            observed_workspace_state: None,
        }],
        &[],
    );
    assert_eq!(card_mode(), CardMode::Off);
    assert_eq!(render_card_for_task(&workspace, card_cap()), "");
    // An explicit env flag outranks the receipt.
    let _on = crate::tests::TestEnvGuard::set("ANGEL_CADDY", "1");
    assert_eq!(card_mode(), CardMode::Relevant);
    let _ = std::fs::remove_dir_all(base);
}

/// M06b measurement receipt: injected card bytes, full (pre-M06b) vs
/// cost-aware (default), for each of the six cohort fixture languages.
/// Prints one JSON line per fixture; captured into
/// docs/audits/evidence/2026-09-08-straight-a/M06b/card-bytes.json.
#[test]
fn m06b_card_bytes_table() {
    let _guard = crate::tests::env_lock();
    let cases = [
        ("js-duration-parser", "node-test", "node"),
        ("js-retry-plan", "node-test", "node"),
        ("js-safe-workspace-path", "node-test", "node"),
        ("python-deep-config-merge", "python-test", "python"),
        ("python-ttl-cache", "python-test", "python"),
        ("rust-capped-backoff", "cargo-test", "cargo"),
    ];
    let recipes: Vec<Recipe> = [
            // A mixed store: the fixture's own family recipe plus four
            // entries from other families — what an always-on full card
            // charged for in cohort #6.
            ("run_tests {\"args\":\"\",\"runtime\":\"node\"}", "run_tests"),
            ("run_tests {\"args\":\"\",\"runtime\":\"python\"}", "run_tests"),
            ("run_tests {\"args\":\"\",\"runtime\":\"rust\"}", "run_tests"),
            ("cargo {\"args\":\"test --quiet\"}", "cargo"),
            // Long irrelevant entries (a real store carries full command
            // lines); these are what the always-on full card paid for.
            (
                concat!(
                    "run_tests {\"args\":\"",
                    "integration/e2e-suite --reporter verbose --timeout 600 --retry 2 --shuffle --seed 99",
                    "\",\"runtime\":\"go\"}"
                ),
                "run_tests",
            ),
            (
                concat!(
                    "cargo {\"args\":\"build --release --features long-feature-list,",
                    "with-many-flags --target x86_64-unknown-linux-gnu\"}"
                ),
                "cargo",
            ),
            (
                concat!(
                    "run_tests {\"args\":\"",
                    "acceptance/full-stack.mjs --browser headless --screenshots --profile ci",
                    "\",\"runtime\":\"swift\"}"
                ),
                "run_tests",
            ),
            (
                concat!(
                    "run_tests {\"args\":\"",
                    "regression/tier3 --fail-fast --glob '*.spec.ts' --workers 8 --update-snapshots",
                    "\",\"runtime\":\"node\"}"
                ),
                "run_tests",
            ),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (command, tool))| Recipe {
            ts_ms: 1_700_000_000_000 + i as u64 * 86_400_000,
            command: command.to_string(),
            env: Vec::new(),
            duration_ms: Some(41_000),
            tool: tool.to_string(),
            note: "verified: ok".to_string(),
            verification: Some(RecipeVerification::ExecutedVerifierV1),
                verified_head: None,
                stale_since_changes: 0,
        workspace_state: None,
        observed_workspace_state: None,
        })
        .collect();
    let hazards: Vec<Hazard> = [
        ("node --test test.mjs", "exit 1: failing assertion"),
        ("python3 -m unittest -v", "exit 1: ImportError"),
        ("cargo test --quiet", "exit 101: compile error"),
        ("swift test", "tool error: sandbox_apply denied write"),
        ("make build", "exit 2: no rule to make target"),
        ("./scripts/release.sh", "[timed out after 30s"),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (command, diagnostic))| Hazard {
        ts_ms: 1_700_000_000_000 + i as u64 * 86_400_000,
        command: command.to_string(),
        diagnostic: diagnostic.to_string(),
        tool: "shell".to_string(),
    })
    .collect();
    for (name, family, runtime) in cases {
        let (caddy_dir, _dossier_dir, workspace, _env) = fixture("bytes");
        // Materialize the fixture's verifier layout so the workspace scan
        // detects its language (package.json / test_*.py / Cargo.toml+src).
        std::fs::remove_file(workspace.join("Cargo.toml")).unwrap();
        match family {
            "node-test" => {
                std::fs::write(
                    workspace.join("package.json"),
                    "{\n  \"scripts\": { \"test\": \"node --test test.mjs\" }\n}\n",
                )
                .unwrap();
            }
            "python-test" => {
                std::fs::write(workspace.join("test_value.py"), "def test_x():\n    pass\n")
                    .unwrap();
            }
            _ => {
                // Fixture already writes Cargo.toml; keep src/ present.
                std::fs::write(workspace.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
                std::fs::create_dir_all(workspace.join("src")).unwrap();
                std::fs::write(workspace.join("src").join("lib.rs"), "#[test]\nfn t() {}\n")
                    .unwrap();
            }
        }
        let recipe = Recipe {
            ts_ms: 1_700_100_000_000,
            command: match family {
                "cargo-test" => "cargo {\"args\":\"test --quiet\"}".to_string(),
                _ => format!("run_tests {{\"args\":\"\",\"runtime\":\"{runtime}\"}}"),
            },
            env: Vec::new(),
            duration_ms: Some(41_000),
            tool: "run_tests".to_string(),
            note: "verified: ok".to_string(),
            verification: Some(RecipeVerification::ExecutedVerifierV1),
            verified_head: None,
            stale_since_changes: 0,
            workspace_state: None,
            observed_workspace_state: None,
        };
        let mut store = recipes.clone();
        store.push(recipe);
        seed_store(&caddy_dir, &workspace, &store, &hazards);
        let _defaults = crate::tests::TestEnvGuard::unset("ANGEL_FEATURE_DEFAULTS");
        let _exp = crate::tests::TestEnvGuard::set("ANGEL_EXPERIENCE_LOG", "");
        let full = {
            let _full = crate::tests::TestEnvGuard::set("ANGEL_CADDY_CARD", "full");
            render_card_for_task(&workspace, card_cap())
        };
        let relevant = {
            let _rel = crate::tests::TestEnvGuard::unset("ANGEL_CADDY_CARD");
            render_card_for_task(&workspace, card_cap())
        };
        // The fixture's own-family recipe line, exactly as the card shows
        // it: `run_tests {"args":"","runtime":"<runtime>"}`.
        let own = match family {
            "cargo-test" => "cargo {\"args\":\"test --quiet\"}".to_string(),
            _ => format!("\"runtime\":\"{runtime}\""),
        };
        let consumed = relevant.contains(&own);
        if !consumed {
            eprintln!("RELEVANT CARD [{name}]: {relevant:?} (looking for {own})");
        }
        assert!(
            consumed,
            "{name}: cost-aware card must carry the fixture's own-family recipe"
        );
        let drop_pct = 100.0 * (1.0 - (relevant.len() as f64) / (full.len() as f64));
        assert!(
            drop_pct >= 70.0,
            "{name}: dropped bytes must be >= 70% (full {} B -> relevant {} B, drop {drop_pct:.1}%)",
            full.len(),
            relevant.len()
        );
        let drop_pct = if full.is_empty() {
            0.0
        } else {
            100.0 * (full.len() - relevant.len()) as f64 / full.len() as f64
        };
        println!(
            "M06B_ROW {}",
            serde_json::json!({
                "fixture": name,
                "family": family,
                "full_bytes": full.len(),
                "relevant_bytes": relevant.len(),
                "drop_pct": drop_pct,
                "recipe_consumed": consumed,
            })
        );
        let _ = std::fs::remove_dir_all(caddy_dir.parent().unwrap());
    }
}

#[test]
fn caddy_card_follows_the_dossier_block_in_task_warm_start() {
    let _guard = crate::tests::env_lock();
    let _backplane = crate::tests::TestEnvGuard::set("ANGEL_BACKPLANE", "0");
    let (caddy_dir, dossier_dir, workspace, _env) = fixture("order");
    seed_dossier(&dossier_dir, &workspace);
    seed_store(
        &caddy_dir,
        &workspace,
        &[Recipe {
            ts_ms: 1_700_000_000_000,
            command: "./benchmark.sh --local-iterate".to_string(),
            env: Vec::new(),
            duration_ms: Some(41_000),
            tool: "shell".to_string(),
            note: "verified: ok".to_string(),
            verification: Some(RecipeVerification::ExecutedVerifierV1),
            verified_head: None,
            stale_since_changes: 0,
            workspace_state: None,
            observed_workspace_state: None,
        }],
        &[Hazard {
            ts_ms: 1_700_000_000_000,
            command: "swift test".to_string(),
            diagnostic: "sandbox_apply denied".to_string(),
            tool: "shell".to_string(),
        }],
    );

    let block = crate::harness::task_warm_start(&workspace);
    let _defaults = crate::tests::TestEnvGuard::set("ANGEL_CADDY_CARD", "full");
    let block_full = crate::harness::task_warm_start(&workspace);
    let dossier_at = block
        .find(crate::dossier::DOSSIER_BLOCK_HEADER)
        .expect("dossier block present");
    let caddy_at = block_full
        .find("[caddy ·")
        .expect("caddy card present (full mode)");
    assert!(
        dossier_at < caddy_at,
        "card must follow the dossier block:\n{block}"
    );
    let _ = std::fs::remove_dir_all(caddy_dir.parent().unwrap());
}
include!("caddy_integrity_tests.rs");
