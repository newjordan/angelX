use crate::agent::harness::RenderedOutputAcceptance;
#[test]
fn task_mode_parses_sealed_profile_and_rejects_unknown_or_duplicate() {
    let args = parse_task_args(
        "--task-json",
        ["--sandbox-profile", "sealed", "fixture"]
            .into_iter()
            .map(str::to_string),
    )
    .unwrap();
    assert_eq!(args.sandbox_profile.as_deref(), Some("sealed"));
    for flags in [
        vec!["--sandbox-profile"],
        vec!["--sandbox-profile", "unknown", "fixture"],
        vec![
            "--sandbox-profile",
            "sealed",
            "--sandbox-profile",
            "sealed",
            "fixture",
        ],
    ] {
        assert!(parse_task_args("--task-json", flags.into_iter().map(str::to_string)).is_err());
    }
}

use super::*;
use crate::agent::tools::work_landing::{Visibility, WorkContext};

#[test]
fn authority_profile_task_envelope_reports_full_and_guarded_startup() {
    let _lock = crate::tests::env_lock();
    let _smart = crate::tests::TestEnvGuard::set("ANGEL_YOLO_SMART", "0");
    for (flag, expected) in [("1", "full"), ("0", "guarded")] {
        let _full = crate::tests::TestEnvGuard::set("ANGEL_YOLO", flag);
        let value = serde_json::to_value(TaskJsonEnvelope::from_startup_failure(
            &TaskCliArgs::default(),
            PathBuf::from("/fixture"),
            0,
            TaskStartupStopReason::InvalidArguments,
            "fixture".into(),
        ))
        .unwrap();
        assert_eq!(value["authority_profile"]["profile"], expected);
        assert_eq!(
            value["authority_profile"]["text"],
            crate::platform::authority_profile::active(true).text
        );
    }
}

#[test]
fn task_usage_serializes_partial_zero_and_cache_only_without_fabrication() {
    let cell = crate::agent::club::AccountingCell::default();
    let before = cell.view();
    cell.record(Some(crate::agent::club::UsageObservation {
        raw: [Some(0), None, None, Some(0), None],
        ..Default::default()
    }));
    let usage = task_usage_delta(before, cell.view()).unwrap();
    let value = serde_json::to_value(usage).unwrap();
    assert_eq!(value["input"], 0);
    assert_eq!(value["cache_read"], 0);
    assert!(
        value["output"].is_null()
            && value["reasoning"].is_null()
            && value["uncached_input"].is_null()
    );
    assert_eq!(value["core_complete"], false);
    let before = cell.view();
    cell.record(Some(crate::agent::club::UsageObservation {
        raw: [None, None, None, Some(80), None],
        ..Default::default()
    }));
    let usage = task_usage_delta(before, cell.view()).unwrap();
    assert_eq!(
        (usage.input, usage.output, usage.cache_read),
        (None, None, Some(80))
    );
}

// -----------------------------------------------------------------------
// Warm start.
// -----------------------------------------------------------------------

/// A dossier artifact shaped exactly like the compiler's output: one confident
/// ritual, one below-threshold ritual, one confident trap, and a thread.
fn dossier_artifact() -> serde_json::Value {
    serde_json::json!({
        "v": 1,
        "repo": { "key": "k", "root": "/r", "slug": "u/p" },
        "generatedAt": "2026-07-11T00:00:00.000Z",
        "facts": [
            { "kind": "ritual", "class": "test", "text": "cargo test --quiet",
              "meanDurMs": 42_000, "belief": 0.88,
              "evidence": "9 runs, 9 pass, 4 session(s), last 2026-07-10" },
            { "kind": "ritual", "class": "build", "text": "cargo build --release",
              "meanDurMs": 90_000, "belief": 0.55,
              "evidence": "3 runs, 2 pass, 2 session(s), last 2026-07-02" },
            { "kind": "trap", "text": "npm test", "belief": 0.79,
              "evidence": "3 runs, 0 pass (3 fail), 3 session(s), last 2026-07-05" },
        ],
        "thread": { "ts": 1_783_300_000u64, "stop": "answer", "driver": "gemma", "ok": true },
    })
}

/// Clear every knob the warm start reads, so a test never inherits the
/// developer's real environment (or a previous test's leftovers).
fn clear_warm_start_env() {
    for key in [
        "ANGEL_DOSSIER",
        "ANGEL_DOSSIER_TASK",
        "ANGEL_DOSSIER_DIR",
        "ANGEL_DOSSIER_MIN_BELIEF",
        "ANGEL_DOSSIER_MAX_BYTES",
        "ANGEL_WORK_LANDING",
        "ANGEL_WORK_CONTEXT_DIR",
        "ANGEL_TASK_WORKSPACE_MAP",
        "ANGEL_TASK_CODING_DISCIPLINE",
        "ANGEL_TASK_PACE",
        "ANGEL_TASK_PACE_RESOLVED",
        "ANGEL_TASK_PACE_SOURCE",
        "ANGEL_CONTINUAL_HARNESS",
        "ANGEL_CONTINUAL_HARNESS_TASK",
    ] {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(key) };
    }
}

#[test]
fn task_workspace_map_lists_fixture_files_when_enabled() {
    let _guard = crate::tests::env_lock();
    let (workspace, _d, _w) = warm_start_fixture("map");
    std::fs::create_dir_all(workspace.join("src")).unwrap();
    std::fs::write(workspace.join("src/parse-duration.mjs"), "export\n").unwrap();
    std::fs::write(workspace.join("test.mjs"), "test\n").unwrap();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_TASK_WORKSPACE_MAP", "1") };

    let block = task_system_contract(&workspace);
    assert!(block.contains("## Task workspace map"), "{block}");
    assert!(block.contains("Coding root (absolute):"), "{block}");
    assert!(block.contains("src/parse-duration.mjs"), "{block}");
    assert!(block.contains("test.mjs"), "{block}");
    assert!(block.contains("Stay inside this directory"), "{block}");
    // The repository-derived carrier no longer duplicates the map.
    assert!(!task_warm_start(&workspace).contains("Task workspace map"));

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_TASK_WORKSPACE_MAP", "0") };
    assert!(!task_system_contract(&workspace).contains("Task workspace map"));
    clear_warm_start_env();
}

/// A private workspace plus empty dossier / work-context dirs, wired into the
/// env. Nothing is seeded — the tests opt into what they want to exist.
fn warm_start_fixture(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
    clear_warm_start_env();
    let base = std::env::temp_dir().join(format!("angel-task-warm-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let dossier_dir = base.join("dossier");
    let work_dir = base.join("work-context");
    let workspace = base.join("repo");
    for dir in [&dossier_dir, &work_dir, &workspace] {
        std::fs::create_dir_all(dir).unwrap();
    }
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_DOSSIER_DIR", &dossier_dir) };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_WORK_CONTEXT_DIR", &work_dir) };
    (workspace, dossier_dir, work_dir)
}

fn seed_dossier(dir: &Path, workspace: &Path) {
    // Dossiers are repository-scoped, including when a fixture directory
    // lives beneath a checkout rather than in a Git-free system temp dir.
    let key = crate::platform::workspace_store::repo_identity(workspace).key;
    std::fs::write(
        dir.join(format!("{key}.json")),
        serde_json::to_string(&dossier_artifact()).unwrap(),
    )
    .unwrap();
}

fn seed_work_context(dir: &Path, workspace: &Path, confirmed: bool) {
    let ctx = WorkContext {
        folder: workspace.display().to_string(),
        repo: Some("newjordan/angelX".to_string()),
        visibility: Visibility::Private,
        mode: Mode::InternalDev,
        confirmed,
        updated_at: 1_783_300_000,
    };
    crate::agent::tools::work_landing::save_context_in(dir, workspace, &ctx).unwrap();
}

/// The headless seat gets the dossier the TUI gets — confident facts asserted,
/// below-threshold facts withheld (counted, never stated).
#[test]
fn task_warm_start_injects_the_confidence_gated_dossier() {
    let _guard = crate::tests::env_lock();
    let _legacy = crate::tests::TestEnvGuard::set("ANGEL_BACKPLANE", "0");
    let (workspace, dossier_dir, _work_dir) = warm_start_fixture("dossier");
    seed_dossier(&dossier_dir, &workspace);

    let block = task_warm_start(&workspace);
    assert!(
        block.contains(crate::knowledge::dossier::DOSSIER_BLOCK_HEADER),
        "warm start must carry the dossier block: {block}"
    );
    // S05: memory recall now crosses the evidence fence; the dossier's own
    // sentinel is inside it, so the block ends with the fence close.
    assert!(
        block
            .trim_end()
            .ends_with(crate::knowledge::evidence::EVIDENCE_FENCE_SENTINEL)
    );
    // The confident ritual and trap are asserted...
    assert!(block.contains("test: `cargo test --quiet`"), "{block}");
    assert!(block.contains("trap: `npm test` fails here"), "{block}");
    assert!(block.contains("last session here"), "{block}");
    // ...and the 0.55-belief build ritual is withheld, not asserted.
    assert!(!block.contains("cargo build --release"), "{block}");
    assert!(
        block.contains("1 fact(s) below 0.70 belief withheld"),
        "{block}"
    );

    clear_warm_start_env();
}

/// The confidence gate is the whole safety story: raise it and even the 0.88
/// ritual is withheld rather than stated.
#[test]
fn task_warm_start_honors_the_belief_threshold() {
    let _guard = crate::tests::env_lock();
    let _legacy = crate::tests::TestEnvGuard::set("ANGEL_BACKPLANE", "0");
    let (workspace, dossier_dir, _work_dir) = warm_start_fixture("belief");
    seed_dossier(&dossier_dir, &workspace);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_DOSSIER_MIN_BELIEF", "0.95") };

    let block = task_warm_start(&workspace);
    assert!(!block.contains("cargo test --quiet"), "{block}");
    assert!(!block.contains("npm test"), "{block}");
    assert!(
        block.contains("3 fact(s) below 0.95 belief withheld"),
        "{block}"
    );

    clear_warm_start_env();
}

/// Two kill switches, both honored: the task-mode-only one and the family-wide
/// one. And a workspace with no artifact is a clean no-op — zero tokens.
#[test]
fn task_warm_start_is_suppressible_and_a_no_op_when_empty() {
    let _guard = crate::tests::env_lock();
    let _legacy = crate::tests::TestEnvGuard::set("ANGEL_BACKPLANE", "0");
    let (workspace, dossier_dir, _work_dir) = warm_start_fixture("suppress");

    // Nothing seeded → nothing said.
    assert_eq!(task_warm_start(&workspace), "");

    seed_dossier(&dossier_dir, &workspace);
    assert!(!task_warm_start(&workspace).is_empty());

    // The headless-only knob.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_DOSSIER_TASK", "0") };
    assert_eq!(task_warm_start(&workspace), "");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_DOSSIER_TASK") };

    // The family-wide knob still governs both seats.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_DOSSIER", "0") };
    assert_eq!(task_warm_start(&workspace), "");

    clear_warm_start_env();
}

/// The work context rides along only once a human has CONFIRMED it. The
/// unconfirmed branch would tell an unattended agent to call `work_landing` (a
/// tool task mode does not register) and to confirm with a user who is not
/// there — so it must never reach a headless prompt.
#[test]
fn task_warm_start_carries_only_a_confirmed_work_context() {
    let _guard = crate::tests::env_lock();
    let (workspace, _dossier_dir, work_dir) = warm_start_fixture("work");

    seed_work_context(&work_dir, &workspace, false);
    let block = task_warm_start(&workspace);
    assert_eq!(
        block, "",
        "unconfirmed context must not prompt a headless run"
    );

    seed_work_context(&work_dir, &workspace, true);
    let block = task_warm_start(&workspace);
    assert!(block.contains("# Active work context"), "{block}");
    assert!(block.contains("mode=internal-dev"), "{block}");
    assert!(block.contains("Optimize for velocity"), "{block}");
    assert!(!block.contains("work_landing"), "{block}");

    // The existing kill switch covers it; no second knob.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_WORK_LANDING", "0") };
    assert_eq!(task_warm_start(&workspace), "");

    clear_warm_start_env();
}

#[test]
fn task_flags_accept_json_as_entrypoint_or_modifier() {
    let direct = parse_task_args(
        "--task-json",
        [
            "--task",
            "--workspace",
            "/tmp/work",
            "--task-id",
            "case-7",
            "--run-id",
            "run-9",
            "--driver",
            "longcat",
            "--reasoning-effort",
            "high",
            "--task-pace",
            "deep",
            "--max-hops",
            "48",
            "--deadline-secs",
            "900",
            "--tool-profile",
            "essential",
            "--rollout",
            "local",
            "--require-rollout",
            "--require-rendered-output",
            "fix it",
        ]
        .into_iter()
        .map(str::to_string),
    )
    .unwrap();
    assert!(direct.json);
    assert_eq!(direct.workspace, Some(PathBuf::from("/tmp/work")));
    assert_eq!(direct.task_id.as_deref(), Some("case-7"));
    assert_eq!(direct.run_id.as_deref(), Some("run-9"));
    assert_eq!(direct.driver.as_deref(), Some("longcat"));
    assert_eq!(direct.reasoning_effort.as_deref(), Some("high"));
    assert_eq!(direct.task_pace.as_deref(), Some("deep"));
    assert_eq!(direct.max_hops, Some(48));
    assert_eq!(direct.deadline_secs, Some(900));
    assert_eq!(direct.tool_profile.as_deref(), Some("essential"));
    assert_eq!(direct.rollout_capture.as_deref(), Some("local"));
    assert!(direct.require_rollout);
    assert!(direct.require_rendered_output);
    assert_eq!(direct.prompt.as_deref(), Some("fix it"));

    let modifier = parse_task_args(
        "--task",
        ["--task-json", "--workspace", "/tmp/work", "fix it"]
            .into_iter()
            .map(str::to_string),
    )
    .unwrap();
    assert!(modifier.json);

    let ordinary = parse_task_args(
        "--task",
        ["--workspace", "/tmp/work", "fix it"]
            .into_iter()
            .map(str::to_string),
    )
    .unwrap();
    assert!(!ordinary.json);

    let flag_prompt = parse_task_args(
        "--task",
        ["--", "--literal-prompt"].into_iter().map(str::to_string),
    )
    .unwrap();
    assert_eq!(flag_prompt.prompt.as_deref(), Some("--literal-prompt"));
}

#[test]
fn task_flags_reject_ambiguous_or_invalid_runner_controls() {
    let unknown = parse_task_args(
        "--task-json",
        ["--max-turns", "4", "fix it"]
            .into_iter()
            .map(str::to_string),
    )
    .unwrap_err();
    assert!(unknown.to_string().contains("unknown task option"));
    assert!(unknown.parsed.json);

    let in_turn_verifier = parse_task_args(
        "--task-json",
        ["--accept-cmd", "cargo test", "fix it"]
            .into_iter()
            .map(str::to_string),
    )
    .unwrap_err();
    assert!(
        in_turn_verifier
            .to_string()
            .contains("unknown task option: --accept-cmd")
    );

    let invalid_hops = parse_task_args(
        "--task",
        ["--max-hops", "many", "fix it"]
            .into_iter()
            .map(str::to_string),
    )
    .unwrap_err();
    assert!(invalid_hops.to_string().contains("non-negative integer"));

    let invalid_capture = parse_task_args(
        "--task",
        ["--rollout", "remote", "fix it"]
            .into_iter()
            .map(str::to_string),
    )
    .unwrap_err();
    assert!(
        invalid_capture
            .to_string()
            .contains("off, shadow, or local")
    );

    let invalid_pace = parse_task_args(
        "--task",
        ["--task-pace", "medium", "fix it"]
            .into_iter()
            .map(str::to_string),
    )
    .unwrap_err();
    assert!(invalid_pace.to_string().contains("auto, rapid, or deep"));

    let extra_prompt =
        parse_task_args("--task", ["fix", "it"].into_iter().map(str::to_string)).unwrap_err();
    assert!(extra_prompt.to_string().contains("one quoted argument"));

    let missing_capture = parse_task_args(
        "--task",
        ["--require-rollout", "fix it"]
            .into_iter()
            .map(str::to_string),
    )
    .unwrap_err();
    assert!(
        missing_capture
            .to_string()
            .contains("requires --rollout shadow")
    );

    let unsafe_effort = parse_task_args(
        "--task",
        ["--reasoning-effort", "high;rm", "fix it"]
            .into_iter()
            .map(str::to_string),
    )
    .unwrap_err();
    assert!(unsafe_effort.to_string().contains("short alphanumeric"));

    let duplicate_limit = parse_task_args(
        "--task-json",
        ["--max-hops", "8", "--max-hops", "16", "fix it"]
            .into_iter()
            .map(str::to_string),
    )
    .unwrap_err();
    assert!(duplicate_limit.to_string().contains("only once"));

    let unsafe_identity = parse_task_args(
        "--task-json",
        ["--run-id", "seed 1\nsecret", "fix it"]
            .into_iter()
            .map(str::to_string),
    )
    .unwrap_err();
    assert!(
        unsafe_identity
            .to_string()
            .contains("portable identity characters")
    );
}

#[test]
fn task_pace_can_be_designated_or_inferred_without_submission_pressure() {
    let _guard = crate::tests::env_lock();
    let _requested = crate::tests::TestEnvGuard::unset("ANGEL_TASK_PACE");
    let _resolved = crate::tests::TestEnvGuard::unset("ANGEL_TASK_PACE_RESOLVED");

    let deep_prompt =
        resolve_task_pace("Treat this as a slow-burn major solve; do not submit yet.");
    assert_eq!(deep_prompt.pace, TaskPace::Deep);
    assert_eq!(deep_prompt.source, "prompt");

    let rapid_prompt = resolve_task_pace("Run a rapid-fire quick pass.");
    assert_eq!(rapid_prompt.pace, TaskPace::Rapid);
    assert_eq!(rapid_prompt.source, "prompt");

    // A long or unbounded budget is not a deep-pace request (operator law 2026-09-11).
    for (hops, seconds) in [("64", "3600"), ("0", "0")] {
        let _hops = crate::tests::TestEnvGuard::set("ANGEL_MAX_HOPS", hops);
        let _seconds = crate::tests::TestEnvGuard::set("ANGEL_TURN_DEADLINE_SECS", seconds);
        let pace = resolve_task_pace("Optimize this implementation.");
        assert_eq!(pace.pace, TaskPace::Rapid);
        assert_eq!(pace.source, "default");
    }

    let ordinary = resolve_task_pace("Fix this implementation.");
    assert_eq!(ordinary.pace, TaskPace::Rapid);
    assert_eq!(ordinary.source, "default");

    let _explicit = crate::tests::TestEnvGuard::set("ANGEL_TASK_PACE", "rapid");
    let explicit = resolve_task_pace("This is a slow burn.");
    assert_eq!(explicit.pace, TaskPace::Rapid);
    assert_eq!(explicit.source, "explicit");
}

#[test]
fn task_hop_guard_is_only_enabled_by_an_explicit_positive_value() {
    assert_eq!(parse_task_max_hops(None), None);
    assert_eq!(parse_task_max_hops(Some("0")), None);
    assert_eq!(parse_task_max_hops(Some(" 0 ")), None);
    assert_eq!(parse_task_max_hops(Some("not-a-number")), None);
    assert_eq!(parse_task_max_hops(Some("17")), Some(17));
}

#[test]
fn yolo_preserves_task_hops_and_bounded_coding_defaults() {
    let _guard = crate::tests::env_lock();
    const KEYS: [&str; 19] = [
        "ANGEL_YOLO",
        "ANGEL_MAX_HOPS",
        "ANGEL_TURN_DEADLINE_SECS",
        "ANGEL_TOOL_TIMEOUT",
        "ANGEL_TOOL_HARD_TIMEOUT",
        "ANGEL_FIRST_WRITE_CALLS",
        "ANGEL_FIRST_WRITE_REJECTIONS",
        "ANGEL_FINAL_MILE_HOPS",
        "ANGEL_FINAL_MILE_ANSWER_HOPS",
        "ANGEL_MUTATION_THRASH_NUDGE",
        "ANGEL_MUTATION_THRASH_STOP",
        "ANGEL_TASK_RECON",
        "ANGEL_TASK_CODING_DISCIPLINE",
        "ANGEL_RELENTLESS_EXECUTION",
        "ANGEL_TASK_TREEBEARD",
        "ANGEL_TASK_PACE",
        "ANGEL_TASK_PACE_RESOLVED",
        "ANGEL_TASK_PACE_SOURCE",
        "ANGEL_LANE",
    ];
    struct Restore([(&'static str, Option<std::ffi::OsString>); 19]);
    impl Drop for Restore {
        fn drop(&mut self) {
            for (key, value) in &self.0 {
                match value {
                    // TODO: Audit that the environment access only happens in single-threaded code.
                    Some(value) => unsafe { std::env::set_var(key, value) },
                    // TODO: Audit that the environment access only happens in single-threaded code.
                    None => unsafe { std::env::remove_var(key) },
                }
            }
        }
    }
    let _restore = Restore(KEYS.map(|key| (key, std::env::var_os(key))));
    for key in KEYS {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(key) };
    }
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_YOLO", "1") };

    assert_eq!(configured_max_hops(), None);
    apply_task_runtime_defaults("");
    assert_eq!(configured_max_hops(), None);
    assert_eq!(std::env::var("ANGEL_TURN_DEADLINE_SECS").unwrap(), "0");
    assert!(std::env::var_os("ANGEL_FIRST_WRITE_CALLS").is_none());
    assert!(std::env::var_os("ANGEL_FIRST_WRITE_REJECTIONS").is_none());
    assert_eq!(std::env::var("ANGEL_FINAL_MILE_HOPS").unwrap(), "4");
    assert_eq!(std::env::var("ANGEL_FINAL_MILE_ANSWER_HOPS").unwrap(), "1");
    assert_eq!(std::env::var("ANGEL_MUTATION_THRASH_NUDGE").unwrap(), "3");
    assert_eq!(std::env::var("ANGEL_MUTATION_THRASH_STOP").unwrap(), "0");
    assert_eq!(std::env::var("ANGEL_TASK_RECON").unwrap(), "repo");
    assert_eq!(std::env::var("ANGEL_TASK_CODING_DISCIPLINE").unwrap(), "1");
    assert_eq!(std::env::var("ANGEL_RELENTLESS_EXECUTION").unwrap(), "1");
    assert_eq!(std::env::var("ANGEL_TASK_TREEBEARD").unwrap(), "auto");
    // The bounded 64-hop headless default still selects the long-horizon lane.
    assert_eq!(std::env::var("ANGEL_LANE").unwrap(), "treebeard");

    // The explicit individual off control still wins under YOLO.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_MAX_HOPS", "0") };
    assert_eq!(configured_max_hops(), None);
}

#[test]
fn task_treebeard_auto_enables_for_long_headless_when_lane_unset() {
    let _guard = crate::tests::env_lock();
    struct Restore(Vec<(&'static str, Option<std::ffi::OsString>)>);
    impl Drop for Restore {
        fn drop(&mut self) {
            for (key, value) in self.0.drain(..) {
                match value {
                    // TODO: Audit that the environment access only happens in single-threaded code.
                    Some(value) => unsafe { std::env::set_var(key, value) },
                    // TODO: Audit that the environment access only happens in single-threaded code.
                    None => unsafe { std::env::remove_var(key) },
                }
            }
        }
    }
    let keys = [
        "ANGEL_LANE",
        "ANGEL_TASK_TREEBEARD",
        "ANGEL_TASK_TREEBEARD_HOPS",
        "ANGEL_MAX_HOPS",
        "ANGEL_YOLO",
    ];
    let _restore = Restore(
        keys.into_iter()
            .map(|key| (key, std::env::var_os(key)))
            .collect(),
    );
    for key in keys {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(key) };
    }
    // Explicit off: no treebeard (ReAct ablation control).
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_TASK_TREEBEARD", "0") };
    maybe_apply_task_treebeard_lane();
    assert_eq!(std::env::var("ANGEL_LANE").unwrap(), "default");

    // auto + unbounded default → treebeard.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_LANE") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_TASK_TREEBEARD", "auto") };
    maybe_apply_task_treebeard_lane();
    assert_eq!(std::env::var("ANGEL_LANE").unwrap(), "treebeard");

    // Explicit lane wins.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_LANE", "default") };
    maybe_apply_task_treebeard_lane();
    assert_eq!(std::env::var("ANGEL_LANE").unwrap(), "default");

    // Short hop budget under auto floor leaves the global Treebeard default
    // implicit rather than writing an environment override.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_LANE") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_MAX_HOPS", "8") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_TASK_TREEBEARD", "auto") };
    maybe_apply_task_treebeard_lane();
    assert!(std::env::var_os("ANGEL_LANE").is_none());
    assert!(crate::agent::harness::is_treebeard());

    // always forces regardless of hops.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_TASK_TREEBEARD", "1") };
    maybe_apply_task_treebeard_lane();
    assert_eq!(std::env::var("ANGEL_LANE").unwrap(), "treebeard");
}

#[test]
fn task_runtime_policy_defaults_are_headless_and_preserve_explicit_overrides() {
    let _guard = crate::tests::env_lock();
    const KEYS: [&str; 22] = [
        "ANGEL_MAX_HOPS",
        "ANGEL_TURN_DEADLINE_SECS",
        "ANGEL_TOOL_TIMEOUT",
        "ANGEL_TOOL_HARD_TIMEOUT",
        "ANGEL_TOOL_IDLE_FLOOR_SECS",
        "ANGEL_FIRST_WRITE_CALLS",
        "ANGEL_FIRST_WRITE_REJECTIONS",
        "ANGEL_FINAL_MILE_HOPS",
        "ANGEL_FINAL_MILE_ANSWER_HOPS",
        "ANGEL_MUTATION_THRASH_NUDGE",
        "ANGEL_MUTATION_THRASH_STOP",
        "ANGEL_PERIPHERAL_MUTATION_NUDGE",
        "ANGEL_POST_GREEN_TOOL_BATCHES",
        "ANGEL_NO_EDIT_ANSWER_GUARD",
        "ANGEL_TASK_RECON",
        "ANGEL_TASK_CODING_DISCIPLINE",
        "ANGEL_RELENTLESS_EXECUTION",
        "ANGEL_TASK_TREEBEARD",
        "ANGEL_TASK_PACE",
        "ANGEL_TASK_PACE_RESOLVED",
        "ANGEL_TASK_PACE_SOURCE",
        "ANGEL_LANE",
    ];
    struct Restore(Vec<(&'static str, Option<std::ffi::OsString>)>);
    impl Drop for Restore {
        fn drop(&mut self) {
            for (key, value) in self.0.drain(..) {
                match value {
                    // TODO: Audit that the environment access only happens in single-threaded code.
                    Some(value) => unsafe { std::env::set_var(key, value) },
                    // TODO: Audit that the environment access only happens in single-threaded code.
                    None => unsafe { std::env::remove_var(key) },
                }
            }
        }
    }
    let _restore = Restore(
        KEYS.into_iter()
            .map(|key| (key, std::env::var_os(key)))
            .collect(),
    );
    for key in KEYS {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(key) };
    }

    apply_task_runtime_defaults("");
    assert_eq!(std::env::var("ANGEL_MAX_HOPS").unwrap(), "0");
    assert_eq!(std::env::var("ANGEL_TURN_DEADLINE_SECS").unwrap(), "0");
    // Tool ceilings follow the turn deadline: a benchmark-length verifier
    // must not die to the interactive 120 s default.
    assert!(std::env::var_os("ANGEL_TOOL_TIMEOUT").is_none());
    assert!(std::env::var_os("ANGEL_TOOL_HARD_TIMEOUT").is_none());
    assert!(std::env::var_os("ANGEL_TOOL_IDLE_FLOOR_SECS").is_none());
    assert!(std::env::var_os("ANGEL_FIRST_WRITE_CALLS").is_none());
    assert!(std::env::var_os("ANGEL_FIRST_WRITE_REJECTIONS").is_none());
    assert_eq!(std::env::var("ANGEL_FINAL_MILE_HOPS").unwrap(), "4");
    assert_eq!(std::env::var("ANGEL_FINAL_MILE_ANSWER_HOPS").unwrap(), "1");
    assert_eq!(std::env::var("ANGEL_MUTATION_THRASH_NUDGE").unwrap(), "3");
    assert_eq!(std::env::var("ANGEL_MUTATION_THRASH_STOP").unwrap(), "0");
    assert_eq!(
        std::env::var("ANGEL_PERIPHERAL_MUTATION_NUDGE").unwrap(),
        "4"
    );
    assert_eq!(std::env::var("ANGEL_POST_GREEN_TOOL_BATCHES").unwrap(), "0");
    assert_eq!(std::env::var("ANGEL_NO_EDIT_ANSWER_GUARD").unwrap(), "0");
    assert_eq!(std::env::var("ANGEL_TASK_RECON").unwrap(), "repo");
    assert_eq!(std::env::var("ANGEL_TASK_CODING_DISCIPLINE").unwrap(), "1");
    assert_eq!(std::env::var("ANGEL_RELENTLESS_EXECUTION").unwrap(), "1");
    assert_eq!(std::env::var("ANGEL_TASK_TREEBEARD").unwrap(), "auto");
    // The bounded long-horizon headless default → Treebeard lane.
    assert_eq!(std::env::var("ANGEL_LANE").unwrap(), "treebeard");

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_FIRST_WRITE_CALLS", "9") };
    apply_task_runtime_defaults("");
    assert_eq!(std::env::var("ANGEL_FIRST_WRITE_CALLS").unwrap(), "9");

    // An explicit operator ceiling and a longer deadline are both honoured.
    for key in KEYS {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(key) };
    }
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_TURN_DEADLINE_SECS", "3600") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_TOOL_TIMEOUT", "300") };
    apply_task_runtime_defaults("");
    assert_eq!(std::env::var("ANGEL_TOOL_TIMEOUT").unwrap(), "300");
    assert_eq!(std::env::var("ANGEL_TOOL_HARD_TIMEOUT").unwrap(), "3600");
    assert!(std::env::var_os("ANGEL_TOOL_IDLE_FLOOR_SECS").is_none());
    // An explicit operator floor remains opt-in and is preserved.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_TOOL_IDLE_FLOOR_SECS", "45") };
    apply_task_runtime_defaults("");
    assert_eq!(std::env::var("ANGEL_TOOL_IDLE_FLOOR_SECS").unwrap(), "45");
}

#[test]
fn coding_discipline_block_carries_action_ladder() {
    let _guard = crate::tests::env_lock();
    let prior = std::env::var_os("ANGEL_TASK_CODING_DISCIPLINE");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_TASK_CODING_DISCIPLINE", "1") };
    let block = task_coding_discipline_block();
    assert!(block.contains("Action ladder"));
    assert!(block.contains("Map"));
    assert!(block.contains("Verify"));
    assert!(block.contains("implementing library"));
    assert!(block.contains("Treebeard"));
    assert!(block.contains("Batch independent"));
    assert!(block.contains("parallel"));
    assert!(block.contains("earliest actual prerequisite"));
    assert!(block.contains("usable input"));
    assert!(block.contains("zero performance"));
    assert!(block.contains("user-visible scope"));
    assert!(block.contains("never omit or auto-remove"));
    for word in ["budget", "deadline", "bounded horizon"] {
        assert!(!block.to_lowercase().contains(word));
        assert!(!task_pace_contract_block().to_lowercase().contains(word));
    }
    match prior {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_TASK_CODING_DISCIPLINE", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_TASK_CODING_DISCIPLINE") },
    }
}

#[test]
fn task_stops_are_nonzero_by_default_with_explicit_legacy_override() {
    assert!(parse_task_strict_exit(None));
    assert!(parse_task_strict_exit(Some("unexpected")));
    assert!(!parse_task_strict_exit(Some("0")));
    assert!(!parse_task_strict_exit(Some("off")));
    assert!(!parse_task_strict_exit(Some("FALSE")));
}

#[test]
fn task_parse_error_preserves_json_mode_and_prior_metadata() {
    let failure = parse_task_args(
        "--task",
        [
            "--task-json",
            "--workspace",
            "/tmp/work",
            "--task-id",
            "case-7",
            "--run-id",
        ]
        .into_iter()
        .map(str::to_string),
    )
    .unwrap_err();

    assert_eq!(failure.message, "--run-id requires a value");
    assert!(failure.parsed.json);
    assert_eq!(failure.parsed.workspace, Some(PathBuf::from("/tmp/work")));
    assert_eq!(failure.parsed.task_id.as_deref(), Some("case-7"));
}

/// Operator-ordered F01 contract: successful envelopes carry observational overruns.
#[test]
fn formation_budget_envelope_reports_over_allocation_without_stopping_answer() {
    let _lock = crate::tests::env_lock();
    let budget = super::super::formation_budget::Budget::new(Some(10), None);
    let _scope = super::super::formation_budget::enter(Some(budget.clone()));
    budget.reserve("coordinator", 20, 20).unwrap().settle(Some(
        crate::agent::club::UsageObservation {
            raw: [Some(20), Some(20), Some(0), Some(0), Some(0)],
            contract: crate::agent::club::UsageContract {
                cache: crate::agent::club::CacheConvention::Included,
                reasoning: crate::agent::club::ReasoningConvention::Included,
            },
            ..Default::default()
        },
    ));
    let envelope = TaskJsonEnvelope::new(
        TaskJsonContext {
            task_id: None,
            run_id: None,
            workspace: PathBuf::from("/w"),
            club: None,
            model: None,
            reasoning_effort: None,
            output_budget: None,
            elapsed_ms: 1,
            tools: Vec::new(),
            timing: None,
            usage: None,
            runtime: None,
            session_id: None,
            artifacts: Vec::new(),
            memory_health: crate::knowledge::caddy::StoreHealthSummary::default(),
        },
        "completed",
        "answer",
        Some("complete answer".into()),
        None,
        1,
        false,
        false,
        false,
        None,
        None,
    );
    let value = serde_json::to_value(envelope).unwrap();
    assert_eq!(value["answer"], "complete answer");
    assert_eq!(value["stop_reason"], "answer");
    assert_eq!(value["over_allocation"], true);
    assert_eq!(value["over_allocation_tokens"], 30);
    assert_eq!(value["budget_exhausted"], true);
    assert_eq!(value["reservation_denied"], true);
    assert_eq!(value["formation_budget"]["remaining"], -30);
}

#[test]
fn task_json_startup_failure_is_a_v1_error_envelope() {
    let args = TaskCliArgs {
        json: true,
        workspace: Some(PathBuf::from("/tmp/work")),
        task_id: Some("case-7".to_string()),
        run_id: Some("run-9".to_string()),
        prompt: None,
        ..TaskCliArgs::default()
    };
    let value = serde_json::to_value(TaskJsonEnvelope::from_startup_failure(
        &args,
        PathBuf::from("/tmp/work"),
        12,
        TaskStartupStopReason::InvalidArguments,
        "--workspace requires a directory".to_string(),
    ))
    .unwrap();

    assert_eq!(value["version"], 1);
    assert_eq!(value["kind"], "angel.task_result");
    assert_eq!(value["status"], "error");
    assert_eq!(value["stop_reason"], "invalid_arguments");
    assert_eq!(value["error"], "--workspace requires a directory");
    assert_eq!(value["workspace"], "/tmp/work");
    assert_eq!(value["task_id"], "case-7");
    assert_eq!(value["run_id"], "run-9");
    assert_eq!(value["hops"], 0);
    assert_eq!(value["interrupted"], false);
    assert_eq!(value["deadline_reached"], false);
    assert_eq!(value["max_hops_reached"], false);
    assert!(value.get("answer").is_none());
    assert!(value.get("club").is_none());
    assert!(value.get("model").is_none());
    assert!(value.get("output_budget").is_none());
}

#[test]
fn task_output_budget_preserves_runtime_policy_and_provenance() {
    let explicit = RouteMetadata {
        output_budget: OutputBudgetPolicy::Explicit {
            tokens: 512,
            source: OutputBudgetSource::PerClubEnv,
        },
        output_budget_provenance: Some("ANGEL_LONGCAT_MAX_TOKENS".to_string()),
        ..RouteMetadata::default()
    };
    let value = serde_json::to_value(TaskOutputBudget::from_route_metadata(&explicit)).unwrap();
    assert_eq!(value["policy"], "explicit");
    assert_eq!(value["tokens"], 512);
    assert_eq!(value["source"], "per-club-env");
    assert_eq!(value["provenance"], "ANGEL_LONGCAT_MAX_TOKENS");

    let native = serde_json::to_value(TaskOutputBudget::from_route_metadata(
        &RouteMetadata::default(),
    ))
    .unwrap();
    assert_eq!(native["policy"], "provider-native");
    assert!(native.get("tokens").is_none());

    let managed = RouteMetadata {
        output_budget: OutputBudgetPolicy::EndpointManaged,
        ..RouteMetadata::default()
    };
    let managed = serde_json::to_value(TaskOutputBudget::from_route_metadata(&managed)).unwrap();
    assert_eq!(managed["policy"], "endpoint-managed");
    assert_eq!(managed["source"], "provider-plan");
}

#[test]
fn task_json_serializes_truthful_stop_metadata_and_omits_unknowns() {
    let envelope = TaskJsonEnvelope::from_outcome(
        TaskJsonContext {
            task_id: Some("case-7".to_string()),
            run_id: None,
            workspace: PathBuf::from("/tmp/work"),
            club: Some("practice".to_string()),
            model: None,
            reasoning_effort: Some("high".to_string()),
            output_budget: Some(TaskOutputBudget {
                policy: "explicit",
                tokens: Some(4096),
                source: Some("per-club-env"),
                provenance: Some("ANGEL_PRACTICE_MAX_TOKENS".to_string()),
            }),
            elapsed_ms: 42,
            tools: Vec::new(),
            timing: None,
            usage: None,
            runtime: Some(TaskRuntimeConfig::new(
                "b".repeat(64),
                Some("practice".to_string()),
                Some("high".to_string()),
                TaskPaceResolution {
                    requested: "rapid".to_string(),
                    pace: TaskPace::Rapid,
                    source: "explicit".to_string(),
                },
                Some(64),
                900,
                "essential".to_string(),
                "local".to_string(),
                true,
                false,
            )),
            session_id: None,
            artifacts: Vec::new(),
            memory_health: crate::knowledge::caddy::StoreHealthSummary::default(),
        },
        TurnOutcome {
            stop_notice: None,
            reward_binding: Some(
                super::super::rollout::RewardReceipt::new(
                    super::super::rollout::RewardOwner::CodingEval,
                    1.0,
                    "c".repeat(64),
                )
                .unwrap(),
            ),
            answer: "done".to_string(),
            stop_reason: TurnStopReason::Deadline,
            hops: 3,
            interrupted: true,
            deadline_reached: true,
            max_hops_reached: false,
            acceptance: Some(TaskAcceptanceTelemetry {
                schema: "angel-task-acceptance/v1",
                command_sha256: "a".repeat(64),
                armed: true,
                baseline_result: "failed",
                baseline_passed: false,
                baseline_ms: 9,
                post_checks: 2,
                post_ms: 17,
                last_post_result: Some("passed"),
                terminal_passed: true,
                completion_source: "accept_cmd",
                rendered_output: None,
            }),
            rollout_id: Some("rollout-fixture".to_string()),
            tools: Vec::new(),
            timing: None,
        },
        &[],
    );
    let value = serde_json::to_value(envelope).unwrap();
    assert_eq!(value["version"], 1);
    assert_eq!(value["kind"], "angel.task_result");
    assert_eq!(value["status"], "stopped");
    assert_eq!(value["stop_reason"], "deadline");
    assert_eq!(value["hops"], 3);
    assert_eq!(value["deadline_reached"], true);
    assert_eq!(value["acceptance"]["baseline_ms"], 9);
    assert_eq!(value["acceptance"]["post_checks"], 2);
    assert_eq!(value["acceptance"]["post_ms"], 17);
    assert_eq!(value["acceptance"]["schema"], "angel-task-acceptance/v1");
    assert_eq!(value["acceptance"]["baseline_passed"], false);
    assert_eq!(value["acceptance"]["armed"], true);
    assert_eq!(value["acceptance"]["baseline_result"], "failed");
    assert_eq!(value["acceptance"]["last_post_result"], "passed");
    assert_eq!(value["acceptance"]["terminal_passed"], true);
    assert_eq!(value["acceptance"]["completion_source"], "accept_cmd");
    assert_eq!(value["accepted"], true);
    assert_eq!(value["reward_binding"]["owner"], "coding_eval");
    assert_eq!(
        value["reward_binding"]["evaluator_evidence_sha256"],
        "c".repeat(64)
    );
    assert_eq!(value["rollout_id"], "rollout-fixture");
    assert_eq!(value["reasoning_effort"], "high");
    assert_eq!(value["runtime"]["schema"], "angel-task-runtime/v1");
    assert_eq!(value["runtime"]["max_hops"], 64);
    assert_eq!(value["runtime"]["deadline_secs"], 900);
    assert_eq!(value["runtime"]["verification_policy"], "external-only");
    assert_eq!(
        value["runtime"]["config_sha256"].as_str().unwrap().len(),
        64
    );
    assert_eq!(value["output_budget"]["policy"], "explicit");
    assert_eq!(value["output_budget"]["tokens"], 4096);
    assert_eq!(value["output_budget"]["source"], "per-club-env");
    assert_eq!(
        value["output_budget"]["provenance"],
        "ANGEL_PRACTICE_MAX_TOKENS"
    );
    assert!(value.get("run_id").is_none());
    assert!(value.get("model").is_none());
    assert!(value.get("usage").is_none());
    assert!(value.get("timing").is_none());
    assert!(value.get("session_id").is_none());
    assert!(value.get("artifacts").is_none());

    let failure = TaskJsonEnvelope::from_failure(
        TaskJsonContext {
            task_id: None,
            run_id: None,
            workspace: PathBuf::from("/tmp/work"),
            club: Some("practice".to_string()),
            model: None,
            reasoning_effort: None,
            output_budget: None,
            elapsed_ms: 1,
            tools: Vec::new(),
            timing: None,
            usage: None,
            runtime: None,
            session_id: None,
            artifacts: Vec::new(),
            memory_health: crate::knowledge::caddy::StoreHealthSummary::default(),
        },
        TurnFailure {
            message: "guard reached".to_string(),
            stop_reason: TurnStopReason::MaxHops,
            hops: 4,
            interrupted: true,
            deadline_reached: false,
            max_hops_reached: true,
            acceptance: None,
            rollout_id: None,
        },
    );
    let value = serde_json::to_value(failure).unwrap();
    assert_eq!(value["status"], "stopped");
    assert_eq!(value["stop_reason"], "max_hops");
    assert_eq!(value["max_hops_reached"], true);
    assert!(value.get("answer").is_none());
    assert_eq!(value["error"], "guard reached");
}

fn passing_unit_build(rendered: Option<RenderedOutputAcceptance>) -> TaskAcceptanceTelemetry {
    TaskAcceptanceTelemetry {
        schema: "angel-task-acceptance/v1",
        command_sha256: "b".repeat(64),
        armed: true,
        baseline_result: "passed",
        baseline_passed: true,
        baseline_ms: 11,
        post_checks: 1,
        post_ms: 17,
        last_post_result: Some("passed"),
        terminal_passed: true,
        completion_source: "accept_cmd",
        rendered_output: rendered,
    }
}

fn serialize_acceptance(acceptance: TaskAcceptanceTelemetry) -> serde_json::Value {
    serde_json::to_value(TaskJsonEnvelope::from_outcome(
        TaskJsonContext {
            task_id: Some("t-render".to_string()),
            run_id: None,
            workspace: PathBuf::from("/tmp/work"),
            club: Some("practice".to_string()),
            model: None,
            reasoning_effort: None,
            output_budget: None,
            elapsed_ms: 12,
            tools: Vec::new(),
            timing: None,
            usage: None,
            runtime: None,
            session_id: None,
            artifacts: Vec::new(),
            memory_health: crate::knowledge::caddy::StoreHealthSummary::default(),
        },
        TurnOutcome {
            stop_notice: None,
            reward_binding: None,
            answer: "ok".to_string(),
            stop_reason: TurnStopReason::Answer,
            hops: 3,
            interrupted: false,
            deadline_reached: false,
            max_hops_reached: false,
            acceptance: Some(acceptance),
            rollout_id: None,
            tools: Vec::new(),
            timing: None,
        },
        &[],
    ))
    .unwrap()
}
#[test]
fn task_json_keeps_unit_build_distinct_from_rendered_output_acceptance() {
    let no_media = serialize_acceptance(passing_unit_build(None));
    assert!(no_media["acceptance"].get("rendered_output").is_none());
    assert_eq!(no_media["accepted"], true);

    let value = serialize_acceptance(passing_unit_build(Some(
        RenderedOutputAcceptance::unverified_from_external_only(),
    )));
    assert_eq!(value["acceptance"]["terminal_passed"], true);
    assert_eq!(
        value["acceptance"]["rendered_output"]["state"],
        "unverified"
    );
    assert_eq!(
        value["acceptance"]["rendered_output"]["limitation"],
        RenderedOutputAcceptance::EXTERNAL_LIMITATION
    );
    assert_eq!(value["accepted"], false);
}

#[test]
fn public_task_json_preserves_unverified_rendered_without_accept_cmd() {
    use crate::agent::club::Club;
    use crate::agent::harness::{
        ChatMsg, TaskRolloutBindingV1, ToolRegistry, TurnStopReason, run_task_turn_observed,
    };
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;

    struct FakePractice;
    impl Club for FakePractice {
        fn label(&self) -> &str {
            "practice"
        }
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            Ok("done".into())
        }
    }

    let _lock = crate::tests::env_lock();
    let _accept = crate::tests::TestEnvGuard::unset("ANGEL_TASK_ACCEPT_CMD");
    let _recon = crate::tests::TestEnvGuard::set("ANGEL_TASK_RECON", "off");
    let _discipline = crate::tests::TestEnvGuard::set("ANGEL_TASK_CODING_DISCIPLINE", "0");
    let _map = crate::tests::TestEnvGuard::set("ANGEL_TASK_WORKSPACE_MAP", "0");
    let _verify = crate::tests::TestEnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _skill = crate::tests::TestEnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = crate::tests::TestEnvGuard::set("ANGEL_ADVISOR", "0");
    let _req = crate::tests::TestEnvGuard::set("ANGEL_TASK_RENDERED_REQUIREMENT", "1");

    let root = std::env::temp_dir().join(format!("angel-public-rendered-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let registry = ToolRegistry::with_team(root.clone(), Vec::new());
    let mut history = vec![ChatMsg::user("Say done when finished.")];
    let binding = TaskRolloutBindingV1::new(
        Some("public-rendered".into()),
        Some("run-1".into()),
        crate::knowledge::cut::sha256_hex(b"Say done when finished."),
        crate::knowledge::cut::sha256_hex(b"runtime"),
        "fixture".into(),
        crate::knowledge::cut::sha256_hex(b"source"),
    );
    let (tx, _rx) = mpsc::channel();
    let outcome = run_task_turn_observed(
        &FakePractice,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(2),
        &tx,
        &binding,
        Some("practice"),
    )
    .expect("public task completes");
    assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
    assert!(outcome.acceptance.is_some());
    let envelope = TaskJsonEnvelope::from_outcome(
        TaskJsonContext {
            task_id: Some("public-rendered".into()),
            run_id: Some("run-1".into()),
            workspace: root.clone(),
            club: Some("practice".into()),
            model: None,
            reasoning_effort: None,
            output_budget: None,
            elapsed_ms: 1,
            tools: Vec::new(),
            timing: None,
            usage: None,
            runtime: None,
            session_id: None,
            artifacts: Vec::new(),
            memory_health: crate::knowledge::caddy::StoreHealthSummary::default(),
        },
        outcome,
        &history,
    );
    let value = serde_json::to_value(&envelope).unwrap();
    assert_eq!(value["status"], "completed");
    assert!(value.get("acceptance").is_some());
    assert_eq!(
        value["acceptance"]["rendered_output"]["state"],
        "unverified"
    );
    assert_eq!(
        value["acceptance"]["rendered_output"]["limitation"],
        RenderedOutputAcceptance::EXTERNAL_LIMITATION
    );
    assert_ne!(value["acceptance"]["rendered_output"]["state"], "accepted");
    assert_eq!(value["accepted"], false);
    let _ = std::fs::remove_dir_all(&root);

    drop(_req);
    let _off = crate::tests::TestEnvGuard::unset("ANGEL_TASK_RENDERED_REQUIREMENT");
    let root_off =
        std::env::temp_dir().join(format!("angel-public-rendered-off-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root_off);
    std::fs::create_dir_all(&root_off).unwrap();
    let registry = ToolRegistry::with_team(root_off.clone(), Vec::new());
    let mut history = vec![ChatMsg::user("Say done when finished.")];
    let outcome = run_task_turn_observed(
        &FakePractice,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(2),
        &tx,
        &binding,
        Some("practice"),
    )
    .expect("ordinary public task completes");
    assert!(outcome.acceptance.is_none());
    let envelope = TaskJsonEnvelope::from_outcome(
        TaskJsonContext {
            task_id: Some("public-rendered-off".into()),
            run_id: Some("run-1".into()),
            workspace: root_off.clone(),
            club: Some("practice".into()),
            model: None,
            reasoning_effort: None,
            output_budget: None,
            elapsed_ms: 1,
            tools: Vec::new(),
            timing: None,
            usage: None,
            runtime: None,
            session_id: None,
            artifacts: Vec::new(),
            memory_health: crate::knowledge::caddy::StoreHealthSummary::default(),
        },
        outcome,
        &history,
    );
    let value = serde_json::to_value(&envelope).unwrap();
    assert_eq!(value["status"], "completed");
    assert!(value.get("acceptance").is_none());
    assert!(value.get("accepted").is_none());
    let _ = std::fs::remove_dir_all(&root_off);
}

#[test]
fn lifecycle_task_envelope_kill_reasons_preserve_recorded_answer() {
    let _lock = crate::tests::env_lock();
    for (stop, expected) in [
        ("interrupt", "cancelled"),
        ("deadline", "deadline"),
        ("idle_timeout", "tool_idle"),
        ("answer", "answer"),
    ] {
        let mut tool = serde_json::json!({"tool":"proc_run", "proc_id":17});
        crate::agent::sandbox::process_owner::KillReceipt::new(Some(15), expected, "turn_owner")
            .apply(&mut tool);
        let ctx = TaskJsonContext {
            task_id: None,
            run_id: None,
            workspace: PathBuf::from("/w"),
            club: None,
            model: None,
            reasoning_effort: None,
            output_budget: None,
            elapsed_ms: 1,
            tools: vec![tool],
            timing: None,
            usage: None,
            runtime: None,
            session_id: None,
            artifacts: Vec::new(),
            memory_health: crate::knowledge::caddy::StoreHealthSummary::default(),
        };
        let envelope = TaskJsonEnvelope::new(
            ctx,
            if stop == "answer" {
                "completed"
            } else {
                "stopped"
            },
            stop,
            None,
            None,
            1,
            stop == "interrupt",
            stop == "deadline",
            false,
            None,
            None,
        );
        let value = serde_json::to_value(envelope).unwrap();
        assert_eq!(value["stop_reason"], expected);
        assert_eq!(value["tools"][0]["status"], "killed");
        assert_eq!(value["tools"][0]["kill"]["signal"], 15);
        assert_eq!(value["tools"][0]["kill"]["owner"], "turn_owner");
    }
}

#[test]
fn task_json_carries_the_tool_ledger_only_when_calls_ran() {
    let mut value = serde_json::to_value(TaskJsonEnvelope {
        stop_notice: None,
        authority_profile: crate::platform::authority_profile::active(true),
        sandbox_profile: "ordinary",
        formation_budget: None,
        budget_exhausted: false,
        reservation_denied: false,
        over_allocation: false,
        over_allocation_tokens: None,
        reward_binding: None,
        identity: None,
        escalations: Vec::new(),
        hop_stream_cuts: Vec::new(),
        last_credited_progress: None,
        version: 1,
        kind: "task",
        task_id: None,
        run_id: None,
        workspace: "/w".to_string(),
        status: "completed",
        stop_reason: "answer",
        answer: Some("ok".to_string()),
        error: None,
        hops: 2,
        interrupted: false,
        deadline_reached: false,
        max_hops_reached: false,
        club: Some("c".to_string()),
        model: None,
        reasoning_effort: None,
        output_budget: None,
        elapsed_ms: 5,
        tools: Vec::new(),
        tools_output: Default::default(),
        store_rotations: Vec::new(),
        timing: None,
        acceptance: None,
        accepted: None,
        rollout_id: None,
        rollout_capture_errors: Vec::new(),
        runtime: None,
        usage: None,
        session_id: None,
        artifacts: Vec::new(),
        memory_health: crate::knowledge::caddy::StoreHealthSummary::default(),
        graph_episode_id: None,
        graph_episodes: None,
        graph_episode_list: Vec::new(),
    })
    .unwrap();
    assert!(
        value.get("tools").is_none(),
        "empty ledger is omitted: {value}"
    );
    value["tools"] = serde_json::json!([{"hop": 1, "tool": "read_file", "exec": "ok", "err": false, "bytes": 3}]);
    let text = value.to_string();
    assert!(text.contains("\"tool\":\"read_file\""), "{text}");
}

#[test]
fn task_timing_startup_failure_has_empty_request_samples() {
    let _guard = crate::tests::env_lock();
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            super::super::trajectory::clear_task_lifecycle();
        }
    }
    let _reset = Reset;
    super::super::trajectory::begin_task_lifecycle(std::time::Instant::now());
    let envelope = TaskJsonEnvelope::from_startup_failure(
        &TaskCliArgs::default(),
        PathBuf::from("."),
        0,
        TaskStartupStopReason::EmptyPrompt,
        "empty prompt".into(),
    );
    let timing = envelope.timing.unwrap();
    assert_eq!(timing.model_calls, 0);
    assert_eq!(timing.calls["model_calls"], serde_json::json!([]));
    assert_eq!(timing.wall_ms, timing.startup_ms);
}

#[test]
fn task_json_serializes_timing_block_keys() {
    let envelope = TaskJsonEnvelope::from_outcome(
        TaskJsonContext {
            task_id: None,
            run_id: None,
            workspace: PathBuf::from("/tmp/work"),
            club: Some("practice".to_string()),
            model: None,
            reasoning_effort: None,
            output_budget: None,
            elapsed_ms: 1_000,
            tools: Vec::new(),
            timing: None,
            usage: None,
            runtime: None,
            session_id: None,
            artifacts: Vec::new(),
            memory_health: crate::knowledge::caddy::StoreHealthSummary::default(),
        },
        TurnOutcome {
            stop_notice: None,
            reward_binding: None,
            answer: "done".to_string(),
            stop_reason: TurnStopReason::Answer,
            hops: 2,
            interrupted: false,
            deadline_reached: false,
            max_hops_reached: false,
            acceptance: None,
            rollout_id: None,
            tools: Vec::new(),
            timing: Some(TaskTimingTelemetry {
                schema: "angel-task-timing/v2",
                background: Default::default(),
                model_ms: 600,
                model_calls: 2,
                model_retry_ms: 250,
                model_retries: 1,
                tool_ms: 300,
                tool_calls: 3,
                tool_errors: 1,
                tool_max_ms: 250,
                tool_max_name: Some("shell".to_string()),
                other_ms: 100,
                wall_ms: 1000,
                envelope_wall_ms: None,
                startup_shutdown_ms: None,
                startup_ms: 0,
                shutdown_ms: 0,
                startup: serde_json::json!({}),
                tool_overhead_ms: 0,
                serial_overhead_ms: 0,
                residual_ms: 100,
                overlap_ms: 0,
                spans: serde_json::json!({"turn_start":0,"turn_end":1000}),
                calls: serde_json::json!({"model_calls":[]}),
            }),
        },
        &[],
    );
    let value = serde_json::to_value(envelope).unwrap();
    assert_eq!(value["timing"]["schema"], "angel-task-timing/v2");
    assert!(value["timing"]["background"].is_object());
    assert!(value["tools_output"].is_object());
    assert!(value["store_rotations"].is_array());
    assert_eq!(value["timing"]["model_ms"], 600);
    assert_eq!(value["timing"]["model_calls"], 2);
    assert_eq!(value["timing"]["model_retry_ms"], 250);
    assert_eq!(value["timing"]["model_retries"], 1);
    assert_eq!(value["timing"]["tool_ms"], 300);
    assert_eq!(value["timing"]["tool_calls"], 3);
    assert_eq!(value["timing"]["tool_errors"], 1);
    assert_eq!(value["timing"]["tool_max_ms"], 250);
    assert_eq!(value["timing"]["tool_max_name"], "shell");
    assert_eq!(value["timing"]["other_ms"], 100);
}
#[test]
fn runtime_missing_task_envelope_is_structured_and_recovery_clears_it() {
    let _env = crate::tests::env_lock();
    let error = crate::agent::tools::runtime_missing::RuntimeMissing::new("cargo", "PATH").encode();
    let mut tools = vec![serde_json::json!({
        "tool": "run_tests", "error": format!("tool error: {error}"),
    })];
    for recovered in [false, true] {
        if recovered {
            tools.push(serde_json::json!({"tool": "run_tests", "verify": "passed"}));
        }
        let envelope = TaskJsonEnvelope::new(
            TaskJsonContext {
                task_id: None,
                run_id: None,
                workspace: PathBuf::from("/nonexistent"),
                club: None,
                model: None,
                reasoning_effort: None,
                output_budget: None,
                elapsed_ms: 0,
                tools: tools.clone(),
                timing: None,
                usage: None,
                runtime: None,
                session_id: None,
                artifacts: Vec::new(),
                memory_health: crate::knowledge::caddy::StoreHealthSummary::default(),
            },
            "completed",
            "answer",
            Some("done".into()),
            None,
            2,
            false,
            false,
            false,
            None,
            None,
        );
        assert_eq!(envelope.stop_reason, "answer");
        if recovered {
            assert!(envelope.error.is_none());
            assert_eq!(envelope.status, "completed");
        } else {
            let error = envelope.error.unwrap();
            assert_eq!(error["kind"], "runtime_missing");
            assert_eq!(error["runtime"], "cargo");
            assert!(error["hint"].as_str().unwrap().contains("ANGEL_CARGO_BIN"));
            assert_eq!(envelope.status, "error");
        }
    }
}
