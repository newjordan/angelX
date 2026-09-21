//! Trajectory logging, spawn seat admission, and MoA synthesis deadlines.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory.

use super::*;

/// Tests that read or bound the process-wide spawn-seat counter run one at a time:
/// a parallel sibling reserving or releasing seats between `baseline` and the
/// assertion makes admission bounds look wrong (b19a verify, 2026-09-10).
static SPAWN_SEAT_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

// --- trajectory / MoA suite ---

#[test]
fn persona_catalog_loads_embedded_builtins_and_canonical_legacy_user_layouts() {
    let _guard = crate::tests::env_lock();
    let root = scratch("persona_catalog");
    let canonical_dir = root.join("custom-reviewer");
    let legacy_dir = root.join("legacy-lens");
    std::fs::create_dir_all(&canonical_dir).unwrap();
    std::fs::create_dir_all(&legacy_dir).unwrap();
    std::fs::write(
        canonical_dir.join("PERSONA.md"),
        "---\nname: REVIEWER\ndescription: user override\n---\ncanonical reviewer body",
    )
    .unwrap();
    // Canonical wins deterministically when both filenames exist.
    std::fs::write(
        canonical_dir.join("SKILL.md"),
        "---\nname: reviewer\n---\nwrong legacy body",
    )
    .unwrap();
    std::fs::write(
        legacy_dir.join("SKILL.md"),
        "---\nname: legacy-lens\n---\nlegacy body",
    )
    .unwrap();
    std::fs::write(root.join("flat.md"), "---\nname: flat-lens\n---\nflat body").unwrap();
    let _user = EnvGuard::set("ANGEL_PERSONAS_DIR", root.to_str().unwrap());
    let _bundled = EnvGuard::unset("ANGEL_BUNDLED_PERSONAS_DIR");

    let personas = load_personas();
    let names = personas
        .iter()
        .map(|persona| persona.name.to_ascii_lowercase())
        .collect::<Vec<_>>();
    for builtin in [
        "architect",
        "code-help",
        "librarian",
        "reviewer",
        "security",
        "skeptic",
        "speed-freak",
        "treebeard",
    ] {
        assert!(
            names.iter().any(|name| name == builtin),
            "missing {builtin}"
        );
    }
    assert!(names.iter().any(|name| name == "legacy-lens"));
    assert!(names.iter().any(|name| name == "flat-lens"));
    let reviewer = personas
        .iter()
        .find(|persona| persona.name.eq_ignore_ascii_case("reviewer"))
        .unwrap();
    assert_eq!(reviewer.body, "canonical reviewer body");
    assert!(!reviewer.body.contains("wrong legacy"));
}

#[test]
fn spawn_reviewer_persona_reaches_provider_and_plain_aliases_are_absence() {
    let _guard = crate::tests::env_lock();
    let _user = EnvGuard::set(
        "ANGEL_PERSONAS_DIR",
        scratch("persona_empty_user").to_str().unwrap(),
    );
    let _bundled = EnvGuard::unset("ANGEL_BUNDLED_PERSONAS_DIR");
    let _retries = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "0");

    struct PersonaProbe {
        systems: Mutex<Vec<String>>,
    }
    impl Club for PersonaProbe {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            Err("persona probe must use structured chat".to_string())
        }
        fn label(&self) -> &str {
            "persona-probe"
        }
        fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            self.systems
                .lock()
                .unwrap()
                .push(messages.first().unwrap().content.to_string());
            Ok(ClubReply::Text("probe landed".to_string()))
        }
    }
    let club = Arc::new(PersonaProbe {
        systems: Mutex::new(Vec::new()),
    });
    let tool = SpawnTool::new(
        scratch("persona_spawn"),
        Some(Arc::clone(&club) as Arc<dyn Club>),
        Vec::new(),
    );
    let schema = tool.def().params;
    let persona_schema = &schema["properties"]["persona"];
    let string_enum = persona_schema["oneOf"][0]["enum"].as_array().unwrap();
    let array_enum = persona_schema["oneOf"][1]["items"]["enum"]
        .as_array()
        .unwrap();
    assert!(string_enum.iter().any(|name| name == "reviewer"));
    assert_eq!(string_enum, array_enum);
    assert!(persona_schema.get("type").is_none());
    let digest = tool
        .call(&serde_json::json!({
            "task": "audit",
            "n": 1,
            "formation": "solo",
            "persona": "reviewer",
            "tools": "none",
            "club": "self"
        }))
        .expect("checked-in reviewer persona must resolve and reach provider");
    assert!(digest.contains("probe landed"), "{digest}");
    let systems = club.systems.lock().unwrap();
    assert_eq!(systems.len(), 1);
    assert!(systems[0].contains("lead code reviewer"), "{}", systems[0]);
    drop(systems);

    assert!(
        requested_personas(Some(&serde_json::json!([" ", "none", "plain", "DEFAULT"])))
            .unwrap()
            .is_empty()
    );
    let error = tool
        .call(&serde_json::json!({
            "task": "audit",
            "n": 1,
            "persona": "invented-lens",
            "tools": "none",
            "club": "self"
        }))
        .unwrap_err();
    assert!(error.contains("unknown persona 'invented-lens'"), "{error}");
    assert!(error.contains("omit `persona` for a plain seat"), "{error}");
}

#[test]
fn spawn_names_the_sota_gate_for_withheld_clubs() {
    let _guard = crate::tests::env_lock();
    let _gate = EnvGuard::set("ANGEL_ALLOW_SOTA_DELEGATE", "0");
    let tool = SpawnTool::new(std::env::temp_dir(), None, Vec::new());
    let err = tool
        .call(&serde_json::json!({"task":"x","club":"grok"}))
        .unwrap_err();
    assert!(
        err.contains("ANGEL_ALLOW_SOTA_DELEGATE=0") && err.contains("ANGEL_ALLOW_SOTA_DELEGATE=1"),
        "an opted-out SOTA club must name the opt-out and how to restore, not read as nonexistent: {err}"
    );
    let err = tool
        .call(&serde_json::json!({"task":"x","club":"no-such-fleet-club"}))
        .unwrap_err();
    assert!(
        err.contains("unknown club"),
        "non-SOTA labels keep the plain unknown-club error: {err}"
    );
}

#[test]
fn spawn_grant_registry_reports_the_workspace_its_tools_use() {
    // Capture reads process-global Cargo/rustup env; hold the env lock so a
    // concurrent CARGO_HOME/PATH-poisoning test cannot redirect resolution.
    let _guard = crate::tests::env_lock();
    let workspace = std::env::temp_dir();
    for grant in [Grant::None, Grant::ReadOnly, Grant::Code, Grant::Research] {
        let cargo = PinnedCargo::capture(&workspace);
        let registry = grant.registry(&workspace, None, &cargo);
        assert_eq!(
            registry.current_workspace(),
            workspace,
            "{grant:?} registry policy/evidence root diverged from its tools"
        );
    }
}

#[test]
fn nested_spawn_grants_are_capability_subsets_not_an_ordinal_ladder() {
    let _guard = crate::tests::env_lock();
    let _retries_off = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "0");

    struct CapabilityProbeClub;
    impl Club for CapabilityProbeClub {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            Ok("capability probe landed".to_string())
        }
        fn label(&self) -> &str {
            "capability-probe"
        }
    }

    let club: Arc<dyn Club> = Arc::new(CapabilityProbeClub);
    let workspace = std::env::temp_dir();
    let root = SpawnTool::new(workspace.clone(), Some(club), Vec::new());
    let cases = [
        (Grant::None, Grant::None, true),
        (Grant::None, Grant::ReadOnly, false),
        (Grant::None, Grant::Research, false),
        (Grant::None, Grant::Code, false),
        (Grant::ReadOnly, Grant::None, true),
        (Grant::ReadOnly, Grant::ReadOnly, true),
        (Grant::ReadOnly, Grant::Research, false),
        (Grant::ReadOnly, Grant::Code, false),
        (Grant::Research, Grant::None, true),
        (Grant::Research, Grant::Research, true),
        (Grant::Research, Grant::ReadOnly, false),
        (Grant::Research, Grant::Code, false),
        (Grant::Code, Grant::None, true),
        (Grant::Code, Grant::ReadOnly, true),
        (Grant::Code, Grant::Code, true),
        (Grant::Code, Grant::Research, false),
    ];
    let cargo = PinnedCargo::capture(&workspace);

    for (parent, requested, allowed) in cases {
        let registry = parent.registry(&workspace, Some(&root), &cargo);
        let result = registry.dispatch(
            "spawn",
            &serde_json::json!({
                "task": "capability probe",
                "n": 1,
                "formation": "solo",
                "tools": requested.label(),
                "club": "self"
            }),
        );
        assert_eq!(
            result.is_ok(),
            allowed,
            "parent={} requested={} result={result:?}",
            parent.label(),
            requested.label()
        );
        if !allowed {
            let error = result.unwrap_err();
            assert!(error.contains("parent grant ceiling"), "{error}");
            assert!(error.contains(requested.label()), "{error}");
            assert!(error.contains(parent.label()), "{error}");
        }
    }
}

#[test]
fn read_only_seat_cannot_use_its_real_nested_spawn_to_request_code() {
    let _guard = crate::tests::env_lock();
    let _lane = EnvGuard::set("ANGEL_LANE", "treebeard");
    let _depth = EnvGuard::set("ANGEL_TREEBEARD_MAX_DEPTH", "3");
    let _spawn_max = EnvGuard::set("ANGEL_SPAWN_MAX", "1");
    let _retries_off = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "0");
    let _full_schemas = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "full");

    struct NestedEscalationClub {
        calls: AtomicUsize,
        receipt: Mutex<Option<String>>,
    }

    impl Club for NestedEscalationClub {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            Err("nested escalation test uses tool chat".to_string())
        }

        fn label(&self) -> &str {
            "nested-escalation"
        }

        fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            if let Some(receipt) = messages
                .iter()
                .rev()
                .find(|message| message.role == ChatRole::Tool)
            {
                *self.receipt.lock().unwrap() = Some(receipt.content.to_string());
                return Ok(ClubReply::Text("nested escalation contained".to_string()));
            }
            if call > 0 {
                return Err("nested code seat ran before a denial receipt".to_string());
            }
            assert!(tools.iter().any(|tool| tool.name == "spawn"));
            Ok(ClubReply::Calls(vec![ToolCall {
                id: "nested-code-attempt".to_string(),
                name: "spawn".to_string(),
                args: serde_json::json!({
                    "task": "try to obtain a write-capable child",
                    "n": 1,
                    "formation": "solo",
                    "tools": "code",
                    "club": "self"
                }),
            }]))
        }
    }

    let club = Arc::new(NestedEscalationClub {
        calls: AtomicUsize::new(0),
        receipt: Mutex::new(None),
    });
    let root = SpawnTool::new(
        std::env::temp_dir(),
        Some(Arc::clone(&club) as Arc<dyn Club>),
        Vec::new(),
    );
    // This is the exact immutable handle Grant::registry installs for a
    // read-only parent seat; invoking it through a real run_turn exercises the
    // provider → nested spawn → denial receipt → provider recovery sequence.
    let tool = root.nested_for_test(Grant::ReadOnly);
    let digest = tool
        .call(&serde_json::json!({
            "task": "adversarial parent seat",
            "n": 1,
            "formation": "solo",
            "tools": "read_only",
            "club": "self"
        }))
        .expect("the parent seat should recover after the nested denial");

    assert!(digest.contains("nested escalation contained"), "{digest}");
    assert_eq!(
        club.calls.load(Ordering::SeqCst),
        2,
        "a write-capable child provider call escaped the ceiling"
    );
    let receipt = club
        .receipt
        .lock()
        .unwrap()
        .clone()
        .expect("parent model received nested tool denial");
    assert!(receipt.contains("tools=code"), "{receipt}");
    assert!(receipt.contains("tools=read_only"), "{receipt}");
    assert!(receipt.contains("never escalate"), "{receipt}");
}

#[test]
fn trajectory_record_captures_turn() {
    // Serialized: this reads env-gated behavior (`ANGEL_ROOT_TRAJECTORY`, the
    // lane/gpu-comp markers) that sibling tests set and restore. Without the
    // lock it observes their value mid-flight and the unlabeled-row assertions
    // fail intermittently.
    let _guard = crate::tests::env_lock();
    let history = vec![
        ChatMsg::user("reverse hello"),
        ChatMsg::harness(FINAL_VERIFY_NUDGE),
        ChatMsg::assistant("olleh"),
    ];
    // Unlabeled: no reward field.
    let rec = trajectory_record("turbo", &history, "olleh", 2, false, None, 123);
    assert_eq!(rec["club"], "turbo");
    assert_eq!(rec["hops"], 2);
    assert_eq!(rec["interrupted"], false);
    assert_eq!(rec["answer"], "olleh");
    assert_eq!(rec["ts_ms"], 123);
    assert_eq!(rec["schema"], "angel-trajectory/v2");
    assert_eq!(rec["messages"].as_array().unwrap().len(), 3);
    assert_eq!(rec["messages"][0]["role"], "user");
    assert_eq!(rec["messages"][0]["origin"], "operator");
    assert_eq!(rec["messages"][1]["role"], "user");
    assert_eq!(rec["messages"][1]["origin"], "harness");
    assert!(rec["messages"][2].get("origin").is_none());
    assert!(
        rec.get("reward").is_none(),
        "unlabeled record must omit reward"
    );

    // Reward-labeled: training data — always carries Hi/Q root trajectory +
    // harness treatment so the forge / promotion layer can train on strategy
    // isomorphism and split Hi/Q vs baseline treatments.
    let labeled = trajectory_record("turbo", &history, "olleh", 2, false, Some(0.75), 123);
    assert_eq!(labeled["reward"], 0.75);
    assert!(
        labeled.get("root_trajectory").is_some(),
        "reward-labeled rows are RL training data and must carry root_trajectory"
    );
    assert!(labeled["root_trajectory"].get("fingerprint").is_some());
    assert!(labeled["root_trajectory"].get("offload_ratio").is_some());
    assert!(
        labeled["root_trajectory"].get("hiq_priority").is_some(),
        "forge curriculum needs hiq_priority on labeled rows"
    );
    assert_eq!(
        labeled["harness_treatment"]["handle_store"].as_bool(),
        Some(true)
    );
    assert!(labeled["harness_treatment"].get("handle_stats").is_some());
    // Unlabeled rows stay light unless analysis is requested.
    assert!(
        rec.get("root_trajectory").is_none(),
        "unlabeled log must omit root_trajectory by default"
    );

    let eval = eval_trajectory_record("turbo", &history, "olleh", 1.0, "receipt-sha256", 124);
    assert_eq!(eval["reward"], 1.0);
    assert_eq!(eval["evaluator_evidence_manifest_sha256"], "receipt-sha256");
    assert!(eval.get("root_trajectory").is_some());
    assert!(eval.get("harness_treatment").is_some());
}

#[test]
fn unlabeled_root_trajectory_is_opt_in_for_analysis() {
    let _guard = crate::tests::env_lock();
    let history = vec![ChatMsg::user("x"), ChatMsg::assistant("y")];
    let _off = EnvGuard::set("ANGEL_ROOT_TRAJECTORY", "0");
    let plain = trajectory_record("c", &history, "y", 1, false, None, 1);
    assert!(plain.get("root_trajectory").is_none());
    drop(_off);
    let _on = EnvGuard::set("ANGEL_ROOT_TRAJECTORY", "1");
    let analyzed = trajectory_record("c", &history, "y", 1, false, None, 1);
    assert!(analyzed.get("root_trajectory").is_some());
    assert!(analyzed.get("harness_treatment").is_some());
}

#[test]
fn harness_message_origin_survives_session_serde_roundtrip() {
    let history = vec![
        ChatMsg::user("operator task"),
        ChatMsg::harness(FINAL_MILE_NUDGE),
        ChatMsg::assistant("working"),
    ];
    let encoded = serde_json::to_vec(&history).unwrap();
    let decoded: Vec<ChatMsg> = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded[0].role, ChatRole::User);
    assert_eq!(decoded[1].role, ChatRole::Harness);
    assert_eq!(
        decoded
            .iter()
            .filter(|message| message.role == ChatRole::User)
            .count(),
        1,
        "session turn counts must exclude harness-authored nudges"
    );
}

#[test]
fn ledger_status_text_tabulates_recent_turns() {
    let _guard = crate::tests::env_lock();
    let dir = scratch("ledger_status");
    let trajectory_log = EnvGuard::set("ANGEL_TRAJECTORY_LOG", "1");
    let trajectory_dir = EnvGuard::set("ANGEL_TRAJECTORY_DIR", dir.to_string_lossy().as_ref());
    write_trajectory(&serde_json::json!({
        "schema": "angel-trajectory/v2", "ts_ms": 1_700_000_000_000u64, "club": "glm-5.3-flash",
        "hops": 4, "reward": 1.0, "answer": "done",
        "usage": {"input": 1000, "output": 321, "reasoning": 7},
        "timing": {"schema": "angel-task-timing/v1", "model_ms": 22_400, "tool_ms": 190, "tool_calls": 4},
        "tools": [
            {"hop": 1, "tool": "read_file", "exec": "ok", "err": false, "bytes": 10},
            {"hop": 3, "tool": "shell", "exec": "failed", "err": true, "class": "exec", "bytes": 40},
            {"hop": 4, "tool": "run_tests", "exec": "ok", "err": false, "verify": "passed", "bytes": 60}
        ]
    }));
    // A pre-ledger record is skipped, not counted.
    write_trajectory(&serde_json::json!({
        "schema": "angel-trajectory/v2", "ts_ms": 1_700_000_001_000u64, "club": "old", "hops": 9, "answer": "x"
    }));
    let text = ledger_status_text("");
    assert!(text.contains("last 1 turn(s)"), "{text}");
    assert!(text.contains("glm-5.3-flash"), "{text}");
    assert!(text.contains("passed"), "{text}");
    assert!(text.contains("1.00"), "{text}");
    assert!(
        text.contains(
            "mean: hops 4.0 · tool errors 1.00/turn · model 22.4 s/turn · out 321 tok/turn"
        ),
        "{text}"
    );
    drop(trajectory_dir);
    drop(trajectory_log);
    let _ = std::fs::remove_dir_all(dir);
}

/// The eval row is the reward-labeled training sample, so it carries the same
/// turn ledger (timing + per-call tool outcomes) as the run_turn row; usage
/// needs the club and stays on the run_turn row.
#[test]
fn eval_rows_carry_the_turn_ledger() {
    let _guard = crate::tests::env_lock();
    let dir = scratch("eval_ledger");
    let trajectory_log = EnvGuard::set("ANGEL_TRAJECTORY_LOG", "1");
    let trajectory_dir = EnvGuard::set("ANGEL_TRAJECTORY_DIR", dir.to_string_lossy().as_ref());
    let club = ScriptedClub {
        hops: AtomicUsize::new(0),
    };
    let workspace = dir.join("workspace");
    crate::knowledge::experience::note_turn_workspace(&workspace);
    reset_turn_ledger(&club);
    note_tool_outcome(
        1,
        "read_file",
        &serde_json::json!({"path":"src/lib.rs"}),
        "contents",
        "ok",
        false,
        None,
        None,
        Some(3),
        120,
    );
    note_tool_outcome(
        2,
        "run_tests",
        &serde_json::json!({}),
        "tests: 1 passed, 0 failed",
        "ok",
        false,
        None,
        Some("passed"),
        Some(900),
        64,
    );
    note_timing(&serde_json::json!({"schema": "angel-task-timing/v1", "tool_calls": 2}));
    let answer = "eval-ledger-marker";
    let history = vec![ChatMsg::user("write code"), ChatMsg::assistant(answer)];
    log_eval_trajectory_ex("turbo", &history, answer, 1.0, "sha", None);

    let log =
        std::fs::read_to_string(dir.join(format!("session-{}.jsonl", std::process::id()))).unwrap();
    let record: Value = log
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|r| r["answer"] == answer)
        .expect("eval row written");
    assert_eq!(record["reward"], 1.0);
    let tools = record["tools"].as_array().expect("eval row carries tools");
    assert_eq!(tools.len(), 2, "{tools:?}");
    assert_eq!(tools[1]["tool"], "run_tests");
    assert_eq!(tools[1]["verify"], "passed");
    assert_eq!(tools[1]["ms"], 900);
    assert_eq!(record["timing"]["tool_calls"], 2);
    // The trace schema (lane F) always carries the `usage` key; without a
    // club it is null, never a fabricated usage object.
    assert!(
        record.get("usage").is_none_or(Value::is_null),
        "usage needs the club: {record}"
    );
    drop(trajectory_dir);
    drop(trajectory_log);
    let _ = std::fs::remove_dir_all(dir);
}

/// Do not double-count: the coding-eval path scores its own rollout with the
/// test suite, so `run_turn`'s row for that same rollout must stay unlabeled —
/// otherwise the forge trains on one attempt twice, under two different rewards.
#[test]
fn the_eval_path_owns_its_rollouts_label() {
    let _guard = crate::tests::env_lock();
    let dir = scratch("eval_label_scope");
    let trajectory_log = EnvGuard::set("ANGEL_TRAJECTORY_LOG", "1");
    let trajectory_dir = EnvGuard::set("ANGEL_TRAJECTORY_DIR", dir.to_string_lossy().as_ref());
    let club = ScriptedClub {
        hops: AtomicUsize::new(0),
    };
    let workspace = dir.join("workspace");
    crate::knowledge::experience::note_turn_workspace(&workspace);
    let history = vec![ChatMsg::user("write code")];
    // Trajectory logging is process-global. A detached turn from an unrelated
    // parallel test can finish while this test's destination is active, so the
    // per-process file is shared even though environment mutation is locked.
    // Mark and select this test's rows instead of claiming ownership of every
    // append (the max-hop trajectory test follows the same discipline).
    let answer = "eval-label-scope-marker";

    // An ordinary turn carries The Cut's machine verdict as its label.
    log_trajectory(&club, &history, answer, 3, false, Some(0.5));
    // The same call inside a coding eval writes a plain log: the eval owns the
    // label and will write the stronger, test-verified one itself.
    {
        let _eval_owns_the_label = EvalLabelScope::new();
        log_trajectory(&club, &history, answer, 3, false, Some(0.5));
    }
    // …and the claim is released with the eval.
    log_trajectory(&club, &history, answer, 3, false, Some(1.0));

    let path = dir.join(format!("session-{}.jsonl", std::process::id()));
    let rows: Vec<Value> = std::fs::read_to_string(&path)
        .expect("trajectory log")
        .lines()
        .map(|line| serde_json::from_str(line).expect("valid JSONL"))
        .filter(|row: &Value| row["answer"] == answer && row["hops"] == 3)
        .collect();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["reward"], 0.5);
    assert!(
        rows[1].get("reward").is_none(),
        "run_turn must not label a rollout the eval already scores: {}",
        rows[1]
    );
    assert_eq!(rows[2]["reward"], 1.0);
    let identity = crate::platform::workspace_store::repo_identity(&workspace);
    assert!(rows.iter().all(|row| row["repo"]["key"] == identity.key));

    drop(trajectory_dir);
    drop(trajectory_log);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A trajectory written under cfg(test) never lands in the real home dir: the
/// ten-minute rsync timer feeds `~/.angelX/trajectories` to the Spark trainer
/// inbox, and cockpit test suites must default to a per-process temp dir.
#[test]
fn unit_test_trajectory_writes_never_land_in_the_real_home_dir() {
    let _guard = crate::tests::env_lock();
    let trajectory_log = EnvGuard::set("ANGEL_TRAJECTORY_LOG", "1");
    let no_dir_override = EnvGuard::unset("ANGEL_TRAJECTORY_DIR");

    let dir = trajectory_dir();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        let live = home.join(".angelX").join("trajectories");
        assert_ne!(dir, live, "cfg(test) default must not be the live dir");
        assert!(
            !dir.starts_with(&live),
            "cfg(test) default must not sit inside the live dir: {dir:?}"
        );
    }
    assert!(
        dir.starts_with(std::env::temp_dir()),
        "cfg(test) default is a per-process temp dir: {dir:?}"
    );
    // One write through the ordinary path lands in the temp dir, not $HOME.
    write_trajectory(&serde_json::json!({
        "kind": "w4-probe",
        "marker": "never-in-home",
    }));
    let written =
        std::fs::read_to_string(dir.join(format!("session-{}.jsonl", std::process::id())))
            .expect("per-process temp trajectory file");
    assert!(written.contains("never-in-home"), "{written}");

    drop(no_dir_override);
    drop(trajectory_log);
}

#[test]
fn trajectory_append_redacts_escaped_nested_credentials_without_changing_types() {
    let _guard = crate::tests::env_lock();
    let secret = "fixture-雪-\"value\"\\with\n\t\u{1}012345";
    let _key = EnvGuard::set("ANGEL_T_TRAJECTORY_SECRET", secret);
    let dir = scratch("trajectory_redaction");
    let path = dir.join("rollouts.jsonl");
    let record = serde_json::json!({
        "answer": secret,
        "messages": [{"args": {"access_token": "opaque-short", "n": 5, "ok": true, "missing": null}}]
    });
    append_trajectory(&path, &record).unwrap();
    let body = std::fs::read_to_string(path).unwrap();
    let saved: Value = serde_json::from_str(body.trim()).unwrap();
    assert_eq!(saved["answer"], "«redacted:ANGEL_T_TRAJECTORY_SECRET»");
    assert_eq!(
        saved["messages"][0]["args"]["access_token"],
        crate::platform::secrets::REDACTED
    );
    assert_eq!(saved["messages"][0]["args"]["n"], 5);
    assert_eq!(saved["messages"][0]["args"]["ok"], true);
    assert!(saved["messages"][0]["args"]["missing"].is_null());
    assert_eq!(record["answer"], secret);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn trajectory_appends_remain_valid_under_parallel_writers() {
    let dir = scratch("trajectory_parallel");
    let path = dir.join("rollouts.jsonl");
    std::thread::scope(|scope| {
        for id in 0..64 {
            let path = path.clone();
            scope.spawn(move || {
                let record = serde_json::json!({
                    "id": id,
                    "payload": "x".repeat(8_192),
                });
                append_trajectory(&path, &record).unwrap();
            });
        }
    });
    let body = std::fs::read_to_string(&path).unwrap();
    let mut ids = body
        .lines()
        .map(|line| {
            serde_json::from_str::<Value>(line).unwrap()["id"]
                .as_u64()
                .unwrap()
        })
        .collect::<Vec<_>>();
    ids.sort_unstable();
    assert_eq!(ids, (0..64).collect::<Vec<_>>());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn spawn_seat_admission_bounds_detached_workers() {
    let _serial = SPAWN_SEAT_TESTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let baseline = spawn_seats_inflight();
    let mut permits = reserve_spawn_seats(2, baseline + 2).unwrap();
    let err = reserve_spawn_seats(1, baseline + 2)
        .err()
        .expect("capacity must reject another detached seat");
    assert!(err.contains("capacity exhausted"), "{err}");

    drop(permits.pop());
    let replacement = reserve_spawn_seats(1, baseline + 2).unwrap();
    drop(replacement);
    drop(permits);
    assert_eq!(spawn_seats_inflight(), baseline);
}

#[test]
fn spawn_panels_share_one_cumulative_budget_and_reject_atomically() {
    let _guard = crate::tests::env_lock();
    let _spawn_max = EnvGuard::set("ANGEL_SPAWN_MAX", "8");
    let _retries_off = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "0");
    let _budget_scope = DescendantBudgetScope::for_test(3);
    let budget = current_descendant_budget().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));

    struct CountingClub(Arc<AtomicUsize>);
    impl Club for CountingClub {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            Ok("unused".to_string())
        }

        fn label(&self) -> &str {
            "descendant-budget-counter"
        }

        fn chat_streaming(
            &self,
            _messages: &[ChatMsg],
            _tools: &[ToolDef],
            _cancel: &AtomicBool,
            _on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            self.0.fetch_add(1, Ordering::AcqRel);
            Ok(ClubReply::Text("landed".to_string()))
        }
    }

    let club: Arc<dyn Club> = Arc::new(CountingClub(Arc::clone(&calls)));
    let tool = SpawnTool::new(std::env::temp_dir(), Some(club), Vec::new());
    let first = tool
        .call(&serde_json::json!({
            "task": "first wave",
            "n": 2,
            "formation": "panel",
            "tools": "none",
            "club": "self"
        }))
        .unwrap();
    assert!(
        first.contains("descendant_calls=2/3 remaining=1 admitted=2"),
        "{first}"
    );

    let error = tool
        .call(&serde_json::json!({
            "task": "non-fitting wave",
            "n": 2,
            "formation": "panel",
            "tools": "none",
            "club": "self"
        }))
        .unwrap_err();
    assert!(error.contains("spent=2 remaining=1"), "{error}");
    assert!(error.contains("zero calls admitted"), "{error}");
    assert_eq!(calls.load(Ordering::Acquire), 2, "rejected panel launched");
    assert_eq!(descendant_budget_status(&budget).unwrap().spent, 2);
}

#[test]
fn moa_admission_charges_the_synthesizer_with_the_draft_wave() {
    let _guard = crate::tests::env_lock();
    let _retries_off = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "0");
    let _budget_scope = DescendantBudgetScope::for_test(2);
    let budget = current_descendant_budget().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));

    struct MoaBudgetClub(Arc<AtomicUsize>);
    impl Club for MoaBudgetClub {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            Ok("unused".to_string())
        }

        fn label(&self) -> &str {
            "moa-budget"
        }

        fn chat_streaming(
            &self,
            _messages: &[ChatMsg],
            _tools: &[ToolDef],
            _cancel: &AtomicBool,
            _on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            let call = self.0.fetch_add(1, Ordering::AcqRel);
            Ok(ClubReply::Text(if call == 0 {
                "draft".to_string()
            } else {
                "folded".to_string()
            }))
        }
    }

    let club: Arc<dyn Club> = Arc::new(MoaBudgetClub(Arc::clone(&calls)));
    let tool = SpawnTool::new(std::env::temp_dir(), Some(club), Vec::new());
    let digest = tool
        .run_moa_for_test(&["task".to_string()], Duration::from_secs(2))
        .unwrap();

    assert_eq!(calls.load(Ordering::Acquire), 2);
    assert_eq!(descendant_budget_status(&budget).unwrap().spent, 2);
    assert!(
        digest.contains("descendant_calls=2/2 remaining=0 admitted=2"),
        "{digest}"
    );
}

#[test]
fn quorum_waits_for_k_successes_after_a_fast_failure() {
    let _guard = crate::tests::env_lock();
    let _yolo_off = EnvGuard::set("ANGEL_YOLO", "0");
    let _retries_off = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "0");
    let _spawn_max = EnvGuard::set("ANGEL_SPAWN_MAX", "3");

    struct StaggeredQuorumClub;
    impl Club for StaggeredQuorumClub {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok("unused".to_string())
        }
        fn label(&self) -> &str {
            "staggered-quorum"
        }
        fn chat_streaming(
            &self,
            messages: &[ChatMsg],
            _tools: &[ToolDef],
            cancel: &AtomicBool,
            _on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            let task = messages
                .iter()
                .rev()
                .find(|message| message.role == ChatRole::User)
                .map(|message| message.content.as_ref())
                .unwrap_or_default();
            let (delay, reply) = match task {
                "fail-fast" => (Duration::from_millis(5), Err("fast failure".to_string())),
                "success-one" => (
                    Duration::from_millis(50),
                    Ok(ClubReply::Text("first healthy answer".to_string())),
                ),
                "success-two" => (
                    Duration::from_millis(100),
                    Ok(ClubReply::Text("second healthy answer".to_string())),
                ),
                other => panic!("unexpected scripted quorum task: {other}"),
            };
            let deadline = Instant::now() + delay;
            while Instant::now() < deadline {
                if cancel.load(Ordering::Acquire) {
                    return Err(format!("{task} was cancelled before landing"));
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            reply
        }
    }

    let club: Arc<dyn Club> = Arc::new(StaggeredQuorumClub);
    let tool = SpawnTool::new(std::env::temp_dir(), Some(club), Vec::new());
    let digest = tool
        .call(&serde_json::json!({
            "tasks": ["fail-fast", "success-one", "success-two"],
            "formation": "quorum",
            "quorum": 2,
            "tools": "none",
            "club": "self"
        }))
        .expect("two slower successful seats must satisfy K=2");

    let first = digest.find("first healthy answer").expect("first success");
    let second = digest
        .find("second healthy answer")
        .expect("second success");
    assert!(
        first < second,
        "seat output must retain index order: {digest}"
    );
    assert!(digest.contains("fast failure"), "{digest}");
}

#[test]
fn impossible_quorum_fails_and_cancels_the_remaining_seat() {
    let _guard = crate::tests::env_lock();
    let _yolo_off = EnvGuard::set("ANGEL_YOLO", "0");
    let _retries_off = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "0");
    let _spawn_max = EnvGuard::set("ANGEL_SPAWN_MAX", "3");

    struct ImpossibleQuorumClub {
        slow_cancelled: Arc<AtomicBool>,
    }
    impl Club for ImpossibleQuorumClub {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok("unused".to_string())
        }
        fn label(&self) -> &str {
            "impossible-quorum"
        }
        fn chat_streaming(
            &self,
            messages: &[ChatMsg],
            _tools: &[ToolDef],
            cancel: &AtomicBool,
            _on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            let task = messages
                .iter()
                .rev()
                .find(|message| message.role == ChatRole::User)
                .map(|message| message.content.as_ref())
                .unwrap_or_default();
            match task {
                "fail-one" => {
                    std::thread::sleep(Duration::from_millis(5));
                    Err("failure one".to_string())
                }
                "fail-two" => {
                    std::thread::sleep(Duration::from_millis(15));
                    Err("failure two".to_string())
                }
                "slow-success" => {
                    let deadline = Instant::now() + Duration::from_secs(2);
                    while Instant::now() < deadline {
                        if cancel.load(Ordering::Acquire) {
                            self.slow_cancelled.store(true, Ordering::Release);
                            return Err("cancelled after quorum became impossible".to_string());
                        }
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Ok(ClubReply::Text("too late".to_string()))
                }
                other => panic!("unexpected scripted quorum task: {other}"),
            }
        }
    }

    let slow_cancelled = Arc::new(AtomicBool::new(false));
    let club: Arc<dyn Club> = Arc::new(ImpossibleQuorumClub {
        slow_cancelled: Arc::clone(&slow_cancelled),
    });
    let tool = SpawnTool::new(std::env::temp_dir(), Some(club), Vec::new());
    let error = tool
        .call(&serde_json::json!({
            "tasks": ["fail-one", "fail-two", "slow-success"],
            "formation": "quorum",
            "quorum": 2,
            "tools": "none",
            "club": "self"
        }))
        .expect_err("one remaining seat cannot recover a K=2 quorum");

    assert!(error.contains("0/2 successful"), "{error}");
    let deadline = Instant::now() + Duration::from_secs(2);
    while !slow_cancelled.load(Ordering::Acquire) {
        assert!(
            Instant::now() < deadline,
            "slow seat never saw cancellation"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn moa_synthesis_respects_deadline_and_retains_admission_until_exit() {
    let _serial = SPAWN_SEAT_TESTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    struct BlockingSynthesisClub {
        started: Arc<AtomicBool>,
        release: Arc<AtomicBool>,
    }
    impl Club for BlockingSynthesisClub {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok("unused".to_string())
        }
        fn label(&self) -> &str {
            "blocking-synthesis"
        }
        fn chat_streaming(
            &self,
            _messages: &[ChatMsg],
            _tools: &[ToolDef],
            _cancel: &AtomicBool,
            _on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            self.started.store(true, Ordering::Release);
            // Deliberately ignore cooperative cancellation to model a provider
            // implementation blocked below the streaming layer.
            while !self.release.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(2));
            }
            Ok(ClubReply::Text("late synthesis".to_string()))
        }
    }

    let started = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let baseline = spawn_seats_inflight();
    let club: Arc<dyn Club> = Arc::new(BlockingSynthesisClub {
        started: Arc::clone(&started),
        release: Arc::clone(&release),
    });
    let wall = Instant::now();
    let answer = bounded_moa_synthesis(
        club,
        vec![ChatMsg::user("fold")],
        Duration::from_millis(40),
        // Other full-suite tests may legitimately hold global spawn seats.
        // Admission saturation is asserted against the live count below; do
        // not let unrelated seats consume this test's whole 40 ms budget
        // before its worker can start.
        usize::MAX,
    );
    assert!(
        answer.is_none(),
        "late synthesis must degrade to raw drafts"
    );
    assert!(wall.elapsed() < Duration::from_millis(500));
    // The full suite runs tests in parallel and can briefly starve the newly
    // spawned provider thread. The caller's deadline must still hold, but the
    // admission assertion below is meaningful only after the provider has
    // actually entered its blocking call. Wait independently of the 40 ms
    // caller budget and release the mock before panicking so a failed test
    // cannot leak a detached worker into the rest of the suite.
    let started_deadline = Instant::now() + Duration::from_secs(2);
    while !started.load(Ordering::Acquire) {
        if Instant::now() >= started_deadline {
            release.store(true, Ordering::Release);
            panic!("synthesis worker never entered the provider call");
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let held = spawn_seats_inflight();
    assert!(held >= 1, "blocking synthesis must still own a seat");
    assert!(
        reserve_spawn_seats(1, held).is_err(),
        "timed-out synthesis must retain its permit until the provider exits"
    );

    release.store(true, Ordering::Release);
    let deadline = Instant::now() + Duration::from_secs(2);
    while spawn_seats_inflight() > baseline {
        assert!(Instant::now() < deadline, "synthesis worker never exited");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn zero_budget_moa_synthesis_never_calls_provider() {
    struct CountingClub(Arc<AtomicUsize>);
    impl Club for CountingClub {
        fn respond(&self, _p: &str) -> Result<String, String> {
            self.0.fetch_add(1, Ordering::AcqRel);
            Ok("should not run".to_string())
        }
        fn label(&self) -> &str {
            "counting-synthesis"
        }
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let club: Arc<dyn Club> = Arc::new(CountingClub(Arc::clone(&calls)));
    let result = bounded_moa_synthesis(club, vec![ChatMsg::user("fold")], Duration::ZERO, 1);
    assert!(result.is_none());
    assert_eq!(calls.load(Ordering::Acquire), 0);
}

#[test]
fn moa_formation_uses_only_its_remaining_deadline_for_synthesis() {
    // Keep the global posture stable while the remaining-budget assertion runs.
    let _guard = crate::tests::env_lock();
    let _yolo_off = EnvGuard::set("ANGEL_YOLO", "0");

    struct QuickDraftBlockingFold {
        fold_started: Arc<AtomicBool>,
        fold_done: Arc<AtomicBool>,
        release: Arc<AtomicBool>,
        draft_delay: Duration,
    }
    impl Club for QuickDraftBlockingFold {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok("draft".to_string())
        }
        fn label(&self) -> &str {
            "quick-draft-blocking-fold"
        }
        fn chat_streaming(
            &self,
            messages: &[ChatMsg],
            _tools: &[ToolDef],
            _cancel: &AtomicBool,
            _on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            let folding = messages.iter().any(|message| {
                message.role == ChatRole::System
                    && message.content.contains("aggregator of a spawn formation")
            });
            if folding {
                self.fold_started.store(true, Ordering::Release);
                while !self.release.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(2));
                }
                self.fold_done.store(true, Ordering::Release);
                Ok(ClubReply::Text("late fold".to_string()))
            } else {
                std::thread::sleep(self.draft_delay);
                Ok(ClubReply::Text("fast draft".to_string()))
            }
        }
    }

    let fold_started = Arc::new(AtomicBool::new(false));
    let fold_done = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let club: Arc<dyn Club> = Arc::new(QuickDraftBlockingFold {
        fold_started: Arc::clone(&fold_started),
        fold_done: Arc::clone(&fold_done),
        release: Arc::clone(&release),
        draft_delay: Duration::from_millis(400),
    });
    let tool = SpawnTool::new(std::env::temp_dir(), Some(club), Vec::new());
    let started = Instant::now();
    // Leave enough wall time for a worker thread to land even on a loaded CI
    // host, while spending a known 400 ms before synthesis. A correct remaining-
    // budget implementation returns near the 1 s total deadline; incorrectly
    // granting a fresh 1 s to synthesis would take roughly 1.4 s.
    let result = tool.run_moa_for_test(
        &["answer independently".to_string()],
        Duration::from_secs(1),
    );
    let elapsed = started.elapsed();
    release.store(true, Ordering::Release);
    let digest = result.unwrap();
    assert!(
        elapsed < Duration::from_millis(1_200),
        "elapsed={elapsed:?}"
    );
    assert!(fold_started.load(Ordering::Acquire));
    assert!(digest.contains("synthesis unavailable"), "{digest}");
    assert!(digest.contains("fast draft"), "{digest}");

    let deadline = Instant::now() + Duration::from_secs(2);
    while !fold_done.load(Ordering::Acquire) {
        assert!(Instant::now() < deadline, "fold worker never exited");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn yolo_never_removes_the_spawn_formation_deadline() {
    let _serial = SPAWN_SEAT_TESTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _guard = crate::tests::env_lock();
    let workspace = crate::tests::TestGitWorkspace::new("yolo-formation-deadline");
    crate::agent::harness::run_identity::static_identity()
        .expect("warm executable identity before timing the blocked provider");
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let _retries_off = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "0");

    struct BlockingDraft {
        entered: Arc<AtomicBool>,
        release: Arc<AtomicBool>,
    }
    impl Club for BlockingDraft {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "yolo-blocking-draft"
        }
        fn chat_streaming(
            &self,
            _messages: &[ChatMsg],
            _tools: &[ToolDef],
            _cancel: &AtomicBool,
            _on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            self.entered.store(true, Ordering::Release);
            while !self.release.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(2));
            }
            Ok(ClubReply::Text("late draft".to_string()))
        }
    }

    let entered = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let club: Arc<dyn Club> = Arc::new(BlockingDraft {
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
    });
    // Grant::None exercises provider waiting, not repository IO. A missing
    // subdirectory makes optional Git probes fail immediately instead of
    // spending the provider's 60ms window inspecting unrelated filesystem state.
    let tool = SpawnTool::new(
        workspace.path().join("no-filesystem-grant"),
        Some(club),
        Vec::new(),
    );
    let baseline = spawn_seats_inflight();
    // Under host load the seat thread can miss a 60ms window entirely before it
    // reaches the provider; that says nothing about the deadline, so retry the
    // scenario a few times and only judge an attempt whose seat was entered.
    let mut attempt = 0;
    let (result, elapsed) = loop {
        attempt += 1;
        entered.store(false, Ordering::Release);
        release.store(false, Ordering::Release);
        let started = Instant::now();
        let result = tool.run_moa_for_test(&["bounded".to_string()], Duration::from_millis(60));
        let elapsed = started.elapsed();
        release.store(true, Ordering::Release);
        if entered.load(Ordering::Acquire) {
            break (result, elapsed);
        }
        assert!(
            attempt < 4,
            "the blocking seat was never scheduled inside the window in {attempt} attempts"
        );
        let reap_deadline = Instant::now() + Duration::from_secs(2);
        while spawn_seats_inflight() > baseline {
            assert!(
                Instant::now() < reap_deadline,
                "unentered YOLO seat never exited"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    };

    assert!(
        result.is_err(),
        "a draft that missed the deadline landed: {result:?}"
    );
    assert!(
        elapsed < Duration::from_millis(500),
        "YOLO widened a 60ms formation deadline to {elapsed:?}"
    );
    let reap_deadline = Instant::now() + Duration::from_secs(2);
    while spawn_seats_inflight() > baseline {
        assert!(
            Instant::now() < reap_deadline,
            "blocking YOLO seat never exited"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn yolo_never_allows_parallel_code_writers_in_one_workspace() {
    let _guard = crate::tests::env_lock();

    struct CountingClub(Arc<AtomicUsize>);
    impl Club for CountingClub {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            self.0.fetch_add(1, Ordering::AcqRel);
            Ok("provider must not run".to_string())
        }
        fn label(&self) -> &str {
            "parallel-code-denial-probe"
        }
    }

    for yolo in ["0", "1"] {
        let _yolo = EnvGuard::set("ANGEL_YOLO", yolo);
        let calls = Arc::new(AtomicUsize::new(0));
        let club: Arc<dyn Club> = Arc::new(CountingClub(Arc::clone(&calls)));
        let tool = SpawnTool::new(scratch("parallel_code_denial"), Some(club), Vec::new());
        let error = tool
            .call(&serde_json::json!({
                "task": "edit concurrently",
                "formation": "panel",
                "n": 2,
                "tools": "code",
                "club": "self"
            }))
            .unwrap_err();
        assert!(error.contains("tools=code needs n=1"), "{error}");
        assert!(error.contains("git-worktree isolated"), "{error}");
        assert_eq!(
            calls.load(Ordering::Acquire),
            0,
            "YOLO={yolo} launched a provider before rejecting shared writers"
        );
    }
}
