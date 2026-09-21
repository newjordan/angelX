use super::*;

#[test]
fn m05_experience_compacts_without_archive_growth() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("m05-experience-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("ledger.jsonl");
    for i in 0..1000 {
        append_jsonl_bounded(
            &path,
            &serde_json::json!({"i":i,"body":"abcdefghijk"}),
            1024,
        )
        .unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() <= 1024);
    }
    let rows = std::fs::read_to_string(&path).unwrap();
    let rows: Vec<serde_json::Value> = rows
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(rows.last().unwrap()["i"], 999);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
    let before = std::fs::read(&path).unwrap();
    assert!(append_jsonl_bounded(&path, &serde_json::json!("x".repeat(2048)), 1024).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn env_flag_reads_truthiness() {
    // Unset falls through to the default without touching the environment.
    assert!(env_flag("ANGEL_EXPERIENCE_DEFINITELY_UNSET_XYZ", true));
    assert!(!env_flag("ANGEL_EXPERIENCE_DEFINITELY_UNSET_XYZ", false));
}

#[test]
fn secret_names_are_denied_case_insensitively() {
    assert!(is_secret_name("OPENAI_API_KEY"));
    assert!(is_secret_name("some_token_here"));
    assert!(is_secret_name("Z_AI_SECRET"));
    assert!(is_secret_name("GH_AUTH"));
    assert!(is_secret_name("db_password"));
    // The catalog knobs must all pass (never look like secrets).
    for &k in KNOB_CATALOG {
        assert!(!is_secret_name(k), "{k} tripped the secret denylist");
    }
}

#[test]
fn snapshot_drops_secrets_and_empties_and_trims() {
    let names = [
        "ANGEL_SOTA_MOA_AGG_CLUB",
        "ANGEL_FAKE_API_KEY", // secret-named → dropped
        "ANGEL_EMPTY",        // empty → dropped
        "ANGEL_MISSING",      // unset → dropped
    ];
    let snap = snapshot_from(&names, |k| match k {
        "ANGEL_SOTA_MOA_AGG_CLUB" => Some("  codex-run  ".to_string()),
        "ANGEL_FAKE_API_KEY" => Some("sk-should-never-appear".to_string()),
        "ANGEL_EMPTY" => Some("   ".to_string()),
        _ => None,
    });
    assert_eq!(snap.len(), 1);
    assert_eq!(snap.get("ANGEL_SOTA_MOA_AGG_CLUB").unwrap(), "codex-run");
    assert!(!snap.contains_key("ANGEL_FAKE_API_KEY"));
    // The secret value never made it into the map at all.
    assert!(!snap.values().any(|v| v.contains("sk-")));
}

#[test]
fn cfg_hash_is_stable_and_order_independent() {
    let mut a = BTreeMap::new();
    a.insert(
        "ANGEL_SOTA_MOA_AGG_CLUB".to_string(),
        "codex-run".to_string(),
    );
    a.insert("ANGEL_PXPIPE".to_string(), "1".to_string());
    // Same entries, different insertion order → identical hash (sorted map).
    let mut b = BTreeMap::new();
    b.insert("ANGEL_PXPIPE".to_string(), "1".to_string());
    b.insert(
        "ANGEL_SOTA_MOA_AGG_CLUB".to_string(),
        "codex-run".to_string(),
    );
    assert_eq!(cfg_hash(&a), cfg_hash(&b));
    // A different value → a different hash.
    let mut c = a.clone();
    c.insert("ANGEL_SOTA_MOA_AGG_CLUB".to_string(), "longcat".to_string());
    assert_ne!(cfg_hash(&a), cfg_hash(&c));
    // 16 hex chars.
    assert_eq!(cfg_hash(&a).len(), 16);
}

#[test]
fn rotated_path_preserves_stem() {
    let p = PathBuf::from("/home/u/.angelX/experience/ledger.jsonl");
    assert_eq!(
        rotated_path(&p, 1_700_000_000),
        PathBuf::from("/home/u/.angelX/experience/ledger-1700000000.jsonl")
    );
}

#[test]
fn hiq_outcome_prefers_treebeard_and_eager_offload() {
    // Serialized: this test mutates process-global environment that other
    // tests also read or write. Without the lock they interleave and each
    // observes the other's value between its own set and restore.
    let _guard = crate::tests::env_lock();
    let counters = TurnCounters {
        eager_offload_results: 4,
        eager_offload_bytes_saved: 10_000,
        aged_inspection_results: 2,
        duplicate_inspection_results: 1,
        repeated_inspections: 1,
        code_mode_calls: 2,
        ..TurnCounters::default()
    };
    let prev = std::env::var_os("ANGEL_LANE");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_LANE", "treebeard") };
    // Seed strip snapshot so outcome carries true root LID mass.
    crate::agent::harness::note_last_root_hiq(crate::agent::harness::LastRootHiq {
        offload_ratio: 1.0,
        hiq_priority: 2.1875,
        handle_receipts: 3,
        aged_receipts: 1,
        bulk_tool_results: 0,
        strategy_token_n: 12,
    });
    let j = hiq_outcome_json(&counters);
    assert_eq!(j["lane"], "treebeard");
    assert_eq!(j["eager_offload_results"], 4);
    assert!(j["offload_signal"].as_f64().unwrap() > 0.5);
    let tree_p = j["hiq_priority"].as_f64().unwrap();
    assert!(tree_p > 1.0);
    assert!((j["root_offload_ratio"].as_f64().unwrap() - 1.0).abs() < 1e-9);
    assert!(j["root_hiq_priority"].as_f64().unwrap() > 2.0);
    // Living peer is optional (file may be absent in CI) — only type-check.
    if j.get("living_peer_us").is_some() {
        assert!(j["living_peer_us"].as_f64().unwrap() > 0.0);
    }
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_LANE") };
    let j2 = hiq_outcome_json(&counters);
    assert_eq!(j2["lane"], "default");
    assert!(j2["hiq_priority"].as_f64().unwrap() < tree_p);
    match prev {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_LANE", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_LANE") },
    }
}

#[test]
fn hiq_outcome_weight_max_env_raises_cap() {
    // Serialized: this test mutates process-global environment that other
    // tests also read or write. Without the lock they interleave and each
    // observes the other's value between its own set and restore.
    let _guard = crate::tests::env_lock();
    // Soft treebeard full-offload proxy is 2.1875; default WEIGHT_MAX 3.0
    // leaves room. Explicit low cap still clamps (forge contract).
    let counters = TurnCounters {
        eager_offload_results: 8,
        aged_inspection_results: 2,
        duplicate_inspection_results: 1,
        ..TurnCounters::default()
    };
    let prev_lane = std::env::var_os("ANGEL_LANE");
    let prev_wmax = std::env::var_os("FORGE_HIQ_WEIGHT_MAX");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_LANE", "treebeard") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("FORGE_HIQ_WEIGHT_MAX") };
    let open = hiq_outcome_json(&counters)["hiq_priority"]
        .as_f64()
        .unwrap();
    assert!(open > 2.0);
    assert!(open <= 3.0);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("FORGE_HIQ_WEIGHT_MAX", "1.5") };
    let capped = hiq_outcome_json(&counters)["hiq_priority"]
        .as_f64()
        .unwrap();
    assert!((capped - 1.5).abs() < 1e-9);
    match prev_wmax {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("FORGE_HIQ_WEIGHT_MAX", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("FORGE_HIQ_WEIGHT_MAX") },
    }
    match prev_lane {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_LANE", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_LANE") },
    }
}

#[test]
fn turn_record_has_expected_shape() {
    let cfg = serde_json::json!({ "hash": "deadbeef0000cafe", "knobs": {} });
    let exp = TurnExperience {
        path: "codex-run",
        driver: "sota-moa",
        model: Some("gpt-5.6-sol"),
        reasoning_effort: Some("ultra"),
        ok: true,
        stop: "answer",
        latency_ms: 48_211,
        hops: 6,
        time_to_first_mutation_ms: Some(12_345),
        time_to_green_ms: Some(40_000),
        system_prompt_tokens: 9_876,
        project_doc_bytes: 4_096,
        tool_schema_count: 17,
        tool_schema_tokens: 1_234,
        tool_schema_peak_count: 19,
        tool_schema_peak_tokens: 1_456,
        tool_schema_token_requests: 8_292,
        counters: TurnCounters {
            deferred_nudges: 0,
            spin: 0,
            err_streak: 0,
            churn: 0,
            first_write_rejections: 3,
            duplicate_inspection_results: 7,
            duplicate_inspection_bytes_saved: 12_345,
            aged_inspection_results: 8,
            aged_inspection_bytes_saved: 34_567,
            eager_offload_results: 3,
            eager_offload_bytes_saved: 50_000,
            post_edit_diagnostic_attempts: 4,
            post_edit_diagnostic_findings: 3,
            post_edit_diagnostic_failures: 1,
            post_edit_diagnostic_paths_skipped: 2,
            post_edit_diagnostic_output_bytes: 2_048,
            post_edit_diagnostic_elapsed_ms: 375,
            tool_argument_shrinks: 5,
            tool_argument_strings_shrunk: 6,
            tool_argument_bytes_saved: 23_456,
            repeated_inspections: 9,
            code_mode_calls: 2,
            code_mode_nested_calls: 17,
            code_mode_nested_output_bytes: 65_432,
            code_mode_policy_rejections: 1,
            code_mode_recipe_calls: 1,
            task_recon_context_bytes: 8_192,
            code_mode_schema_tokens: 611,
            request_overflows: 2,
            cache_control_requests: 4,
            cache_read_input_tokens: 8_192,
            cache_write_input_tokens: 2_048,
            cache_read_accounting_responses: 2,
            cache_write_accounting_responses: 1,
            unverified_completion_claims: 1,
            redundant_verifier_skips: 3,
            discovered_tool_schema_failures: 0,
            verification_denials: 2,
            skill_hints: 1,
            provider_truncation_retries: 3,
            provider_truncation_episodes: 2,
            provider_truncation_retained_partials: 1,
            provider_truncation_recoveries: 1,
            provider_truncation_failures: 0,
            action_operations: 3,
            action_previews: 2,
            action_denied: 1,
            action_preflight_us: 14,
            action_approval_wait_ms: 2_000,
            action_exec_ms: 89,
            tool_errors_by_class: ToolErrorClasses {
                schema: 2,
                exec: 1,
                timeout: 1,
                other: 0,
            },
        },
        tokens: vec![("codex-run".to_string(), 91_234, 3_120)],
        failovers: vec![Failover {
            from: "longcat".to_string(),
            to: "openai".to_string(),
            reason: "raw tool markup".to_string(),
        }],
    };
    let repo = serde_json::json!({
        "key": "home-u-proj-0011223344556677",
        "root": "/home/u/proj",
        "slug": "u/proj",
    });
    let rec = turn_record(&exp, &cfg, &repo, 1_700_000_000, 143_224, 17);
    assert_eq!(rec["kind"], "turn");
    assert_eq!(rec["v"], SCHEMA_V);
    assert_eq!(rec["seq"], 17);
    assert_eq!(rec["path"], "codex-run");
    assert_eq!(rec["driver"], "sota-moa");
    assert_eq!(rec["route"]["model"], "gpt-5.6-sol");
    assert_eq!(rec["route"]["reasoning_effort"], "ultra");
    assert_eq!(rec["cfg"]["hash"], "deadbeef0000cafe");
    assert_eq!(rec["repo"]["key"], "home-u-proj-0011223344556677");
    assert_eq!(rec["repo"]["slug"], "u/proj");
    assert_eq!(rec["outcome"]["ok"], true);
    assert!(
        rec["outcome"]["hiq"].is_object(),
        "Treebeard/HiQ block must land on every turn record"
    );
    assert!(rec["outcome"]["hiq"]["lane"].is_string());
    assert_eq!(rec["outcome"]["hiq"]["eager_offload_results"], 3);
    assert_eq!(rec["outcome"]["hiq"]["eager_offload_bytes_saved"], 50_000);
    assert!(rec["outcome"]["hiq"]["hiq_priority"].as_f64().unwrap() >= 0.5);
    assert_eq!(rec["outcome"]["counters"]["first_write_rejections"], 3);
    assert_eq!(
        rec["outcome"]["counters"]["duplicate_inspection_results"],
        7
    );
    assert_eq!(
        rec["outcome"]["counters"]["duplicate_inspection_bytes_saved"],
        12_345
    );
    assert_eq!(rec["outcome"]["counters"]["aged_inspection_results"], 8);
    assert_eq!(
        rec["outcome"]["counters"]["aged_inspection_bytes_saved"],
        34_567
    );
    assert_eq!(
        rec["outcome"]["counters"]["post_edit_diagnostic_attempts"],
        4
    );
    assert_eq!(
        rec["outcome"]["counters"]["post_edit_diagnostic_findings"],
        3
    );
    assert_eq!(
        rec["outcome"]["counters"]["post_edit_diagnostic_failures"],
        1
    );
    assert_eq!(
        rec["outcome"]["counters"]["post_edit_diagnostic_paths_skipped"],
        2
    );
    assert_eq!(
        rec["outcome"]["counters"]["post_edit_diagnostic_output_bytes"],
        2_048
    );
    assert_eq!(
        rec["outcome"]["counters"]["post_edit_diagnostic_elapsed_ms"],
        375
    );
    assert_eq!(rec["outcome"]["counters"]["tool_argument_shrinks"], 5);
    assert_eq!(
        rec["outcome"]["counters"]["tool_argument_strings_shrunk"],
        6
    );
    assert_eq!(
        rec["outcome"]["counters"]["tool_argument_bytes_saved"],
        23_456
    );
    assert_eq!(rec["outcome"]["counters"]["repeated_inspections"], 9);
    assert_eq!(rec["outcome"]["counters"]["code_mode_calls"], 2);
    assert_eq!(rec["outcome"]["counters"]["code_mode_nested_calls"], 17);
    assert_eq!(
        rec["outcome"]["counters"]["code_mode_nested_output_bytes"],
        65_432
    );
    assert_eq!(rec["outcome"]["counters"]["code_mode_policy_rejections"], 1);
    assert_eq!(rec["outcome"]["counters"]["code_mode_recipe_calls"], 1);
    assert_eq!(
        rec["outcome"]["counters"]["task_recon_context_bytes"],
        8_192
    );
    assert_eq!(rec["outcome"]["counters"]["code_mode_schema_tokens"], 611);
    assert_eq!(rec["outcome"]["counters"]["request_overflows"], 2);
    assert_eq!(rec["outcome"]["counters"]["cache_control_requests"], 4);
    assert_eq!(rec["outcome"]["counters"]["cache_read_input_tokens"], 8_192);
    assert_eq!(
        rec["outcome"]["counters"]["cache_write_input_tokens"],
        2_048
    );
    assert_eq!(
        rec["outcome"]["counters"]["cache_read_accounting_responses"],
        2
    );
    assert_eq!(
        rec["outcome"]["counters"]["cache_write_accounting_responses"],
        1
    );
    assert_eq!(
        rec["outcome"]["counters"]["unverified_completion_claims"],
        1
    );
    assert_eq!(
        rec["outcome"]["counters"]["discovered_tool_schema_failures"],
        0
    );
    assert_eq!(
        rec["outcome"]["counters"]["tool_errors_by_class"]["schema"],
        2
    );
    assert_eq!(
        rec["outcome"]["counters"]["tool_errors_by_class"]["exec"],
        1
    );
    assert_eq!(
        rec["outcome"]["counters"]["tool_errors_by_class"]["timeout"],
        1
    );
    assert_eq!(
        rec["outcome"]["counters"]["tool_errors_by_class"]["other"],
        0
    );
    assert_eq!(rec["outcome"]["stop"], "answer");
    assert_eq!(rec["outcome"]["latency_ms"], 48_211);
    assert_eq!(rec["outcome"]["hops"], 6);
    assert_eq!(rec["outcome"]["time_to_first_mutation_ms"], 12_345);
    assert_eq!(rec["outcome"]["time_to_green_ms"], 40_000);
    assert_eq!(rec["outcome"]["system_prompt_tokens"], 9_876);
    assert_eq!(rec["outcome"]["project_doc_bytes"], 4_096);
    assert_eq!(rec["outcome"]["tool_schema_count"], 17);
    assert_eq!(rec["outcome"]["tool_schema_tokens"], 1_234);
    assert_eq!(rec["outcome"]["tool_schema_peak_count"], 19);
    assert_eq!(rec["outcome"]["tool_schema_peak_tokens"], 1_456);
    assert_eq!(rec["outcome"]["tool_schema_token_requests"], 8_292);
    assert_eq!(rec["outcome"]["counters"]["verification_denials"], 2);
    assert_eq!(rec["outcome"]["counters"]["skill_hints"], 1);
    assert_eq!(rec["outcome"]["counters"]["provider_truncation_retries"], 3);
    assert_eq!(
        rec["outcome"]["counters"]["provider_truncation_episodes"],
        2
    );
    assert_eq!(
        rec["outcome"]["counters"]["provider_truncation_retained_partials"],
        1
    );
    assert_eq!(
        rec["outcome"]["counters"]["provider_truncation_recoveries"],
        1
    );
    assert_eq!(
        rec["outcome"]["counters"]["provider_truncation_failures"],
        0
    );
    assert_eq!(rec["outcome"]["counters"]["action_operations"], 3);
    assert_eq!(rec["outcome"]["counters"]["action_previews"], 2);
    assert_eq!(rec["outcome"]["counters"]["action_denied"], 1);
    assert_eq!(rec["outcome"]["counters"]["action_preflight_us"], 14);
    assert_eq!(rec["outcome"]["counters"]["action_approval_wait_ms"], 2_000);
    assert_eq!(rec["outcome"]["counters"]["action_exec_ms"], 89);
    assert_eq!(rec["outcome"]["tokens"]["codex-run"]["in"], 91_234);
    assert_eq!(rec["outcome"]["failovers"][0]["reason"], "raw tool markup");
    assert_eq!(rec["outcome"]["failovers"][0]["from"], "longcat");
    // Every record is one line of valid JSON.
    let line = rec.to_string();
    assert!(!line.contains('\n'));
    assert!(serde_json::from_str::<serde_json::Value>(&line).is_ok());
}

#[test]
fn schema_class_tool_errors_increment_the_schema_bucket() {
    // The four live waste shapes the audit attributed to pure
    // argument-schema misuse all classify as schema.
    let schema_errors = [
        "tool error: grep: missing 'pattern'",
        "tool error: provide at most one of 'path' or 'paths'",
        "tool error: code_mode requires exactly one of `script` or `recipe`",
        "tool error: code_mode: missing 'command'",
        "tool error: 'limit' must be a positive integer no greater than 400",
        "tool error: every 'paths' item must be a string",
        "tool error: argument string exceeds the 65536-byte limit",
    ];
    let mut classes = ToolErrorClasses::default();
    for error in schema_errors {
        assert!(
            matches!(classify_tool_error(error), ToolErrorClass::Schema),
            "{error}"
        );
        classes.record(classify_tool_error(error));
    }
    assert_eq!(
        classes,
        ToolErrorClasses {
            schema: schema_errors.len() as u32,
            exec: 0,
            timeout: 0,
            other: 0
        }
    );
    assert_eq!(
        classes.to_json(),
        serde_json::json!({
            "schema": schema_errors.len() as u32,
            "exec": 0,
            "timeout": 0,
            "other": 0,
        })
    );

    // The neighboring classes stay distinct.
    assert!(matches!(
        classify_tool_error("tool error: shell command timed out after 30000ms"),
        ToolErrorClass::Timeout
    ));
    assert!(matches!(
        classify_tool_error("tool error: command exited with code 101"),
        ToolErrorClass::Exec
    ));
    assert!(matches!(
        classify_tool_error("tool error: unknown tool: frobnicate"),
        ToolErrorClass::Other
    ));
    let mut mixed = ToolErrorClasses::default();
    mixed.record(classify_tool_error(
        "tool error: code_mode: missing 'command'",
    ));
    mixed.record(classify_tool_error("tool error: worker lease expired"));
    assert_eq!(mixed.schema, 1);
    assert_eq!(mixed.other, 1);
    assert_eq!(mixed.exec, 0);
    assert_eq!(mixed.timeout, 0);
}

#[test]
fn explicit_route_verdict_contains_metadata_but_no_conversation_text() {
    let route = crate::agent::club::RouteIdentity {
        driver: "openai".to_string(),
        model: Some("gpt-5.6-sol".to_string()),
        reasoning_effort: Some("ultra".to_string()),
    };
    let repo = serde_json::json!({"key":"k","root":"/r","slug":null});
    let record = route_verdict_record(
        &route,
        RouteVerdict::Useful,
        1_700_000_000_123,
        &repo,
        1_700_000_001,
        42,
        9,
    );
    assert_eq!(record["kind"], "route_verdict");
    assert_eq!(record["source"], "user");
    assert_eq!(record["verdict"], "useful");
    assert_eq!(record["route"]["model"], "gpt-5.6-sol");
    assert_eq!(record["route"]["reasoning_effort"], "ultra");
    assert!(record.get("prompt").is_none());
    assert!(record.get("response").is_none());
    assert!(record.get("note").is_none());
}

#[test]
fn moa_record_carries_dissent_judge_verify() {
    let cfg = serde_json::json!({ "hash": "0", "knobs": {} });
    let exp = MoaExperience {
        driver: "sota-moa",
        route: "deliberate",
        ok: true,
        latency_ms: 9_000,
        dissent: Some(0.18),
        gate: Some("escalate"),
        proposed: 5,
        kept: 2,
        effective: Some((2, true, 1, 1)),
        formation: None,
        judge: Some(JudgeFacts { panel: 3, kept: 2 }),
        verify: Some(VerifyFacts {
            rounds: 2,
            passed: Some(true),
        }),
        tokens: vec![("deepseek".to_string(), 100, 200)],
        failovers: vec![],
    };
    let repo = serde_json::json!({ "key": "k", "root": "/r", "slug": null });
    let rec = moa_record(&exp, &cfg, &repo, 1_700_000_000, 1, 0);
    assert_eq!(rec["kind"], "moa_turn");
    assert_eq!(rec["path"], "sota-moa");
    assert_eq!(rec["repo"]["key"], "k");
    assert!(rec["repo"]["slug"].is_null());
    assert_eq!(rec["outcome"]["route"], "deliberate");
    assert_eq!(rec["outcome"]["dissent"], 0.18);
    assert_eq!(rec["outcome"]["gate"], "escalate");
    assert_eq!(rec["outcome"]["drafts"]["kept"], 2);
    assert_eq!(rec["outcome"]["knobs"]["layers"], 2);
    assert_eq!(rec["outcome"]["judge"]["panel"], 3);
    assert_eq!(rec["outcome"]["verify"]["rounds"], 2);
    assert_eq!(rec["outcome"]["verify"]["passed"], true);
    assert!(rec["outcome"]["formation"].is_null());
}

/// A turn an engaged formation owned must carry that formation's name, so
/// per-formation token economics are measurable from the ledger rows.
#[test]
fn moa_record_carries_the_engaged_formation_name() {
    let cfg = serde_json::json!({ "hash": "0", "knobs": {} });
    let exp = MoaExperience {
        driver: "sota-moa",
        route: "deliberate",
        ok: true,
        latency_ms: 9_000,
        dissent: None,
        gate: None,
        proposed: 3,
        kept: 3,
        effective: Some((2, true, 1, 2)),
        formation: Some("Full Muster".to_string()),
        judge: None,
        verify: None,
        tokens: vec![],
        failovers: vec![],
    };
    let repo = serde_json::json!({ "key": "k", "root": "/r", "slug": null });
    let rec = moa_record(&exp, &cfg, &repo, 1_700_000_000, 1, 0);
    assert_eq!(rec["kind"], "moa_turn");
    assert_eq!(rec["outcome"]["formation"], "Full Muster");
}

#[test]
fn append_jsonl_writes_a_parseable_line_to_a_fresh_dir() {
    // Exercise the real write path (create_dir_all + rotation check + append)
    // — `append_jsonl` has no cfg!(test) guard, only record_* wrappers do.
    let base = std::env::temp_dir().join(format!("angel-exp-{}-{}", std::process::id(), 1));
    let path = base.join("nested").join("ledger.jsonl");
    let _ = std::fs::remove_dir_all(&base);
    let rec = serde_json::json!({ "kind": "turn", "v": 1, "seq": 0 });
    append_jsonl(&path, &rec);
    append_jsonl(&path, &rec);
    let raw = std::fs::read_to_string(&path).expect("ledger written");
    let lines: Vec<&str> = raw.lines().collect();
    assert_eq!(lines.len(), 2, "one JSONL line per append");
    let parsed: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(parsed["kind"], "turn");
    // A small file is well under the rotation threshold, so nothing rotates.
    maybe_rotate(&path);
    assert!(path.exists(), "small ledger is not rotated");
    clear_jsonl(&path).expect("locked clear");
    assert!(!path.exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn persisted_records_scrub_all_text_fields_and_preserve_measurements() {
    let _lock = crate::tests::env_lock();
    let secret = "fixture-ledger-\"credential\"\\01234567";
    let _key = crate::tests::TestEnvGuard::set("ANGEL_T_LEDGER_SECRET", secret);
    let base = std::env::temp_dir().join(format!("angel-exp-redact-{}", std::process::id()));
    let path = base.join("ledger.jsonl");
    let rec = serde_json::json!({
        "kind": "turn", "count": 7, "ok": true,
        "diagnostic": secret,
        "results": ["hf_abcdefghijklmnopqrstuvwxyz01234567", {"token": "opaque-short"}]
    });
    append_jsonl(&path, &rec);
    let appended = std::fs::read_to_string(&path).unwrap();
    let clean: serde_json::Value = serde_json::from_str(appended.trim()).unwrap();
    assert_eq!(clean["diagnostic"], "«redacted:ANGEL_T_LEDGER_SECRET»");
    assert_eq!(clean["results"][0], crate::platform::secrets::REDACTED);
    assert_eq!(
        clean["results"][1]["token"],
        crate::platform::secrets::REDACTED
    );
    assert_eq!(clean["count"], 7);
    assert_eq!(clean["ok"], true);
    replace_jsonl(&path, &[rec]).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), appended);
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn concurrent_jsonl_writers_never_interleave_records() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!(
        "angel-exp-concurrent-{}-{nonce}",
        std::process::id()
    ));
    let path = std::sync::Arc::new(base.join("ledger.jsonl"));
    let writers = 8usize;
    let per_writer = 100usize;
    let threads = (0..writers)
        .map(|writer| {
            let path = std::sync::Arc::clone(&path);
            std::thread::spawn(move || {
                for seq in 0..per_writer {
                    append_jsonl(
                        &path,
                        &serde_json::json!({
                            "writer": writer,
                            "seq": seq,
                            "payload": "x".repeat(256),
                        }),
                    );
                }
            })
        })
        .collect::<Vec<_>>();
    for thread in threads {
        thread.join().unwrap();
    }

    let raw = std::fs::read_to_string(path.as_ref()).expect("concurrent ledger written");
    let lines = raw.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), writers * per_writer);
    let mut seen = std::collections::HashSet::new();
    for line in lines {
        let value: serde_json::Value =
            serde_json::from_str(line).expect("every physical line is one complete JSON value");
        seen.insert((
            value["writer"].as_u64().unwrap(),
            value["seq"].as_u64().unwrap(),
        ));
    }
    assert_eq!(seen.len(), writers * per_writer);
    assert!(jsonl_lock_path(&path).exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn scrub_masks_assignments_flags_headers_and_urls() {
    // Rule 1: NAME=value / Header:value keep the name, mask the value.
    assert_eq!(
        scrub_secrets("export OPENAI_API_KEY=sk-live-abc123 && cargo test"),
        "export OPENAI_API_KEY=… && cargo test"
    );
    // Rule 2: a bare marker masks the next non-marker token — chained
    // markers (`Authorization: Bearer <cred>`) keep the arm until a value.
    assert_eq!(
        scrub_secrets(r#"curl -H "Authorization: Bearer sk-abc" http://x"#),
        r#"curl -H "Authorization: Bearer … http://x"#
    );
    assert_eq!(scrub_secrets("cli --token abc123 run"), "cli --token … run");
    // Rule 3: URL userinfo.
    assert_eq!(
        scrub_secrets("git clone https://user:hunter2@github.com/o/r.git"),
        "git clone https://…@github.com/o/r.git"
    );
    // Non-secret text is untouched (modulo whitespace normalization).
    assert_eq!(
        scrub_secrets("cargo  test -p cockpit"),
        "cargo test -p cockpit"
    );
    // The masked value never survives anywhere in the output.
    for cmd in [
        "A_KEY=sk-99 x",
        "--password hunter2",
        "http://u:hunter2@h/p",
    ] {
        let s = scrub_secrets(cmd);
        assert!(!s.contains("sk-99") && !s.contains("hunter2"), "{s}");
    }
}

#[test]
fn cap_text_is_char_boundary_safe() {
    let long = "é".repeat(CMD_TEXT_MAX); // 2 bytes per char
    let capped = cap_text(&long);
    assert!(capped.len() <= CMD_TEXT_MAX + '…'.len_utf8());
    assert!(capped.ends_with('…'));
    assert_eq!(cap_text("short"), "short");
}

#[test]
fn cmd_event_record_has_expected_shape_and_scrubs() {
    let repo = serde_json::json!({
        "key": "home-u-proj-0011223344556677",
        "root": "/home/u/proj",
        "slug": null,
    });
    let exp = CmdExperience {
        tool: "shell",
        text: "MY_TOKEN=sk-secret cargo test -p cockpit",
        exit: Some(101),
        timed_out: false,
        dur_ms: 48_211,
        bytes_out: 1_874,
        shell: CmdShell::Shell {
            name: "bash",
            pipefail: true,
        },
    };
    let rec = cmd_event_record(&exp, &[], &repo, 1_700_000_000, 42, 7);
    assert_eq!(rec["kind"], "event");
    assert_eq!(rec["event"], "cmd");
    assert_eq!(rec["v"], SCHEMA_V);
    assert_eq!(rec["repo"]["key"], "home-u-proj-0011223344556677");
    assert_eq!(rec["cmd"]["exit"], 101);
    assert_eq!(rec["cmd"]["timed_out"], false);
    assert_eq!(rec["cmd"]["dur_ms"], 48_211);
    assert_eq!(rec["cmd"]["tool"], "shell");
    assert_eq!(rec["cmd"]["bytes_out"], 1_874);
    // The provenance that makes `exit` readable, and the verdict it earns.
    assert_eq!(rec["cmd"]["shell"], "bash");
    assert_eq!(rec["cmd"]["pipefail"], true);
    assert_eq!(rec["cmd"]["verdict"], "fail");
    assert!(rec["cmd"]["verdict_reason"].is_null());
    // Nothing was asserted into the prompt, so this run is the agent's own.
    assert_eq!(rec["cmd"]["source"], "agent");
    assert_eq!(rec["cmd"]["independent"], true);
    // Scrubbed at the builder — no path records raw text.
    assert_eq!(rec["cmd"]["text"], "MY_TOKEN=… cargo test -p cockpit");
    // Signal-killed commands serialize exit as null, and earn no verdict:
    // the status was the killer's.
    let killed = CmdExperience { exit: None, ..exp };
    let rec = cmd_event_record(&killed, &[], &repo, 0, 0, 0);
    assert!(rec["cmd"]["exit"].is_null());
    assert_eq!(rec["cmd"]["verdict"], "no_verdict");
    assert_eq!(rec["cmd"]["verdict_reason"], "signal");
    // A directly-exec'd command has no shell, so there is no pipefail question
    // to answer — `null`, not `false`, which would imply a stealable status.
    let direct = CmdExperience {
        shell: CmdShell::Direct,
        ..exp
    };
    let rec = cmd_event_record(&direct, &[], &repo, 0, 0, 0);
    assert!(rec["cmd"]["shell"].is_null());
    assert!(rec["cmd"]["pipefail"].is_null());
    assert_eq!(rec["cmd"]["verdict"], "fail");
    // One line of valid JSON.
    let line = rec.to_string();
    assert!(!line.contains('\n'));
    assert!(serde_json::from_str::<serde_json::Value>(&line).is_ok());
}

/// The self-confirmation guard. A command the dossier asserted in this
/// session's prompt is the dossier's own echo — it must never be recorded as
/// independent evidence for the fact that suggested it.
#[test]
fn a_command_the_dossier_suggested_is_not_independent_evidence() {
    let repo = serde_json::json!({ "key": "k", "root": "/r", "slug": null });
    let asserted = vec!["cargo check".to_string()];
    let row = |text: &str, asserted: &[String]| {
        let exp = CmdExperience {
            tool: "shell",
            text,
            exit: Some(0),
            timed_out: false,
            dur_ms: 1,
            bytes_out: 0,
            shell: CmdShell::Shell {
                name: "bash",
                pipefail: true,
            },
        };
        cmd_event_record(&exp, asserted, &repo, 0, 0, 0)
    };

    // The loop we are breaking: dossier says "build: `cargo check`" → agent
    // runs `cargo check` → miner must NOT count it as support for that fact.
    let rec = row("cargo check", &asserted);
    assert_eq!(rec["cmd"]["independent"], false);
    assert_eq!(rec["cmd"]["source"], "dossier");
    // And through the plumbing agents actually write it with.
    for echo in [
        "cd cockpit && cargo check 2>&1 | tail -20",
        "cargo check --all-features",
        "CARGO_TERM_COLOR=never   cargo   check",
    ] {
        assert_eq!(
            row(echo, &asserted)["cmd"]["independent"],
            false,
            "`{echo}` echoes an asserted `cargo check`"
        );
    }
    // A genuinely different command is still independent evidence.
    for own in ["cargo test", "cargo build", "npm run lint"] {
        let rec = row(own, &asserted);
        assert_eq!(rec["cmd"]["independent"], true, "{own}");
        assert_eq!(rec["cmd"]["source"], "agent");
    }
    // With nothing asserted (dossier off, or an empty block), everything the
    // agent runs is its own.
    assert_eq!(row("cargo check", &[])["cmd"]["independent"], true);
}

#[test]
fn asserted_commands_are_project_scoped_and_deduped() {
    let base = std::env::temp_dir().join(format!("angel_asserted_scope_{}", std::process::id()));
    let alpha = base.join("alpha");
    let beta = base.join("beta");
    std::fs::create_dir_all(&alpha).unwrap();
    std::fs::create_dir_all(&beta).unwrap();
    let marker = "zzz-marker-only-this-test --flag";
    note_asserted_commands(&alpha, &[marker.to_string(), marker.to_string()]);
    note_asserted_commands(&alpha, &["   ".to_string()]); // blank is not an assertion
    let all = asserted_commands_for(&alpha);
    assert_eq!(
        all.iter().filter(|c| c.as_str() == marker).count(),
        1,
        "a second injection of the same ritual is not a second assertion"
    );
    assert!(!all.iter().any(|c| c.trim().is_empty()), "blanks dropped");
    assert!(!command_is_independent(
        &all,
        &format!("cd x && {marker} 2>&1 | tail -5")
    ));
    assert!(
        asserted_commands_for(&beta).is_empty(),
        "dossier assertions from alpha crossed into beta's learning evidence"
    );
    let _ = std::fs::remove_dir_all(base);
}

/// A pipeline is detected over-eagerly and `||` is not one. Over-detection
/// only ever costs evidence; under-detection would let a stolen exit status be
/// recorded as a verdict.
#[test]
fn looks_piped_finds_pipelines_and_ignores_or() {
    for piped in [
        "cargo check 2>&1 | tail -20",
        "cd cockpit && cargo build | head",
        "a|b",
        "grep x f | wc -l | cat",
        "awk '{print $1 | \"sort\"}' f", // a quoted pipe still counts (safe side)
    ] {
        assert!(looks_piped(piped), "{piped}");
    }
    for unpiped in [
        "cargo check",
        "cargo check || echo failed",
        "test -f x || true",
        "cd cockpit && cargo test",
        "",
    ] {
        assert!(!looks_piped(unpiped), "{unpiped}");
    }
}

/// The verdict tristate — the rule that decides whether a recorded exit code
/// is allowed to mean anything at all.
#[test]
fn cmd_verdict_never_calls_an_inherited_exit_a_pass() {
    let base = CmdExperience {
        tool: "shell",
        text: "cargo check 2>&1 | tail -20",
        exit: Some(0),
        timed_out: false,
        dur_ms: 10,
        bytes_out: 0,
        shell: CmdShell::Shell {
            name: "bash",
            pipefail: true,
        },
    };
    let no_pipefail = CmdShell::Shell {
        name: "sh",
        pipefail: false,
    };

    // THE BUG: a piped command under a shell that cannot propagate pipeline
    // failure. Its exit is `tail`'s. It is not a pass — it is not anything.
    let (v, r) = cmd_verdict(&CmdExperience {
        shell: no_pipefail,
        ..base
    });
    assert_eq!((v, r), (VERDICT_NONE, Some("no_pipefail")));
    // …and that holds whatever the number happens to be.
    let (v, _) = cmd_verdict(&CmdExperience {
        shell: no_pipefail,
        exit: Some(1),
        ..base
    });
    assert_eq!(v, VERDICT_NONE);

    // With pipefail the same pipeline is real evidence, both ways.
    assert_eq!(cmd_verdict(&base), (VERDICT_PASS, None));
    assert_eq!(
        cmd_verdict(&CmdExperience {
            exit: Some(101),
            ..base
        }),
        (VERDICT_FAIL, None)
    );

    // A producer killed by its own consumer (`… | head -2`) is plumbing, not a
    // failing build: an explicit non-verdict, never a fail.
    assert_eq!(
        cmd_verdict(&CmdExperience {
            text: "seq 1 100000 | head -2",
            exit: Some(141),
            ..base
        }),
        (VERDICT_NONE, Some("sigpipe"))
    );
    // But an UNPIPED command that genuinely exits 141 is just a failure — the
    // sigpipe rule must not swallow real verdicts.
    assert_eq!(
        cmd_verdict(&CmdExperience {
            text: "cargo check",
            exit: Some(141),
            ..base
        }),
        (VERDICT_FAIL, None)
    );

    // A command we killed reached no verdict of its own; nor did one that died
    // to a signal. Neither is "your code is broken".
    assert_eq!(
        cmd_verdict(&CmdExperience {
            timed_out: true,
            exit: Some(0),
            ..base
        }),
        (VERDICT_NONE, Some("timed_out"))
    );
    assert_eq!(
        cmd_verdict(&CmdExperience { exit: None, ..base }),
        (VERDICT_NONE, Some("signal"))
    );

    // Direct exec (no shell): a pipe cannot exist, so the status is always the
    // command's own — even for text that happens to contain a `|`.
    assert_eq!(
        cmd_verdict(&CmdExperience {
            tool: "cargo",
            text: "cargo check",
            exit: Some(101),
            shell: CmdShell::Direct,
            ..base
        }),
        (VERDICT_FAIL, None)
    );
}

#[test]
fn skill_event_record_mirrors_the_cmd_shape() {
    let repo = serde_json::json!({
        "key": "home-u-proj-0011223344556677",
        "root": "/home/u/proj",
        "slug": null,
    });
    let exp = SkillExperience {
        name: "angelX-build-test-run",
        ok: true,
        dur_ms: 3,
    };
    let rec = skill_event_record(&exp, &repo, 1_700_000_000, 42, 7);
    assert_eq!(rec["kind"], "event");
    assert_eq!(rec["event"], "skill");
    assert_eq!(rec["v"], SCHEMA_V);
    assert_eq!(rec["repo"]["key"], "home-u-proj-0011223344556677");
    assert_eq!(rec["skill"]["name"], "angelX-build-test-run");
    assert_eq!(rec["skill"]["ok"], true);
    assert_eq!(rec["skill"]["dur_ms"], 3);
    // One line of valid JSON, like every ledger record.
    let line = rec.to_string();
    assert!(!line.contains('\n'));
    assert!(serde_json::from_str::<serde_json::Value>(&line).is_ok());
}

#[test]
fn drain_failovers_is_empty_under_test() {
    // note_failover is a no-op under cfg!(test) (keeps the global sink clean
    // across parallel unit tests); the drain therefore stays empty.
    note_failover("a", "b", "c");
    assert!(drain_failovers().is_empty());
}
