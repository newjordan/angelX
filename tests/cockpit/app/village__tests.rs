use super::*;

#[test]
fn state_round_trips_atomically_and_survives_corruption() {
    let dir = std::env::temp_dir().join(format!("angel_village_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("village.json");

    // Absent file → fresh village.
    let fresh = load_state(Some(&path));
    assert_eq!(fresh.smithy_level, 0);
    assert!(fresh.last_adapter.is_none());

    let state = VillageState {
        smithy_level: 3,
        last_adapter: Some("v3".into()),
        apprentice_name: "Wat".into(),
        dataset_total: 812,
        chatter: vec!["the anvil remembers".into()],
        ..VillageState::default()
    };
    save_state(&path, &state);
    let back = load_state(Some(&path));
    assert_eq!(back.smithy_level, 3);
    assert_eq!(back.last_adapter.as_deref(), Some("v3"));
    assert_eq!(back.apprentice_name, "Wat");
    assert_eq!(back.dataset_total, 812);
    assert_eq!(back.chatter, vec!["the anvil remembers".to_string()]);
    // The tmp file never lingers after a clean save.
    assert!(!path.with_extension("json.tmp").exists());

    // Corrupt file → fresh village, never a crash.
    std::fs::write(&path, "{ not json").unwrap();
    assert_eq!(load_state(Some(&path)).smithy_level, 0);
    // No path at all (no HOME) → fresh village.
    assert_eq!(load_state(None).smithy_level, 0);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn forge_health_parses_the_real_shape_and_degrades() {
    let v: serde_json::Value = serde_json::from_str(
        r#"{
            "training": true,
            "dataset": {"total": 1234, "new_since_last_train": 56},
            "current_adapter": "v2",
            "last_cycle": {"gate_pass": false, "promoted": false,
                           "eval_loss_adapter": 0.91, "eval_loss_base": 0.87},
            "gpu_free_mib": 9000,
            "ollama_models": ["qwen3.5:4b", "angel-head2:v2", "angel-head2:latest"]
        }"#,
    )
    .unwrap();
    let f = parse_forge_health(&v);
    assert!(f.training);
    assert_eq!(f.dataset_total, 1234);
    assert_eq!(f.new_samples, 56);
    assert_eq!(f.adapter.as_deref(), Some("v2"));
    assert_eq!(f.gate_pass, Some(false));
    assert_eq!(f.gpu_free_mib, Some(9000));
    assert_eq!(f.ollama_models, 3);

    // A bare/foreign body degrades to defaults, never a panic.
    let empty = parse_forge_health(&serde_json::json!({}));
    assert!(!empty.training);
    assert_eq!(empty.dataset_total, 0);
    assert!(empty.adapter.is_none());
    assert!(empty.gate_pass.is_none());
}

#[test]
fn merge_local_last_cycle_fills_autopropagate_zero_handoff() {
    // Serialized: this test mutates process-global environment that other
    // tests also read or write. Without the lock they interleave and each
    // observes the other's value between its own set and restore.
    let _guard = crate::tests::env_lock();
    let id = std::process::id();
    let cycle = std::env::temp_dir().join(format!("village-last-cycle-{id}.json"));
    std::fs::write(
        &cycle,
        r#"{"version":"v7","gate_pass":true,"promoted":false,"eval_loss_adapter":1.1,"eval_loss_base":1.3,"adapter_local":"/tmp/a/v7"}"#,
    )
    .unwrap();
    // SAFETY: single-threaded test; path is process-unique.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("FORGE_LAST_CYCLE", &cycle) };
    let bare = parse_forge_health(&serde_json::json!({}));
    let merged = merge_local_last_cycle(bare);
    assert_eq!(merged.adapter.as_deref(), Some("v7"));
    assert_eq!(merged.gate_pass, Some(true));
    assert_eq!(merged.promoted, Some(false));
    assert_eq!(merged.eval_loss_adapter, Some(1.1));
    // Remote current_adapter wins over local version.
    let remote = parse_forge_health(&serde_json::json!({
        "current_adapter": "v2",
        "last_cycle": {"gate_pass": false}
    }));
    let keep = merge_local_last_cycle(remote);
    assert_eq!(keep.adapter.as_deref(), Some("v2"));
    assert_eq!(keep.gate_pass, Some(false)); // remote gate already set
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("FORGE_LAST_CYCLE") };
    let _ = std::fs::remove_file(&cycle);
}

#[test]
fn merge_local_free_train_status_lights_forge_while_lora_runs() {
    // Serialized: this test mutates process-global environment that other
    // tests also read or write. Without the lock they interleave and each
    // observes the other's value between its own set and restore.
    let _guard = crate::tests::env_lock();
    let id = std::process::id();
    let status = std::env::temp_dir().join(format!("village-when-free-{id}.json"));
    std::fs::write(
        &status,
        r#"{"state":"training","job_state":"running","train_step":144,"train_total":200,"gpu_free_mib":13168}"#,
    )
    .unwrap();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("FORGE_WHEN_FREE_STATUS", &status) };
    let idle = parse_forge_health(&serde_json::json!({"training": false}));
    assert!(!idle.training);
    let lit = merge_local_free_train_status(idle);
    assert!(lit.training, "local free-train pulse must light the forge");
    assert_eq!(lit.gpu_free_mib, Some(13168));
    // Done/failed status must not force training on.
    std::fs::write(
        &status,
        r#"{"state":"done","job_state":"done","version":"v1","gate_pass":true}"#,
    )
    .unwrap();
    let after = merge_local_free_train_status(ForgeSnapshot::default());
    assert!(!after.training);
    assert_eq!(after.adapter.as_deref(), Some("v1"));
    assert_eq!(after.gate_pass, Some(true));
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("FORGE_WHEN_FREE_STATUS") };
    let _ = std::fs::remove_file(&status);
}

#[test]
fn adapter_levels_read_the_last_number() {
    assert_eq!(adapter_level("v2"), 2);
    assert_eq!(adapter_level("angel-head2:v13"), 13);
    assert_eq!(adapter_level("latest"), 0);
    assert_eq!(adapter_level(""), 0);
}

#[test]
fn granary_fill_is_log_scaled_so_early_samples_matter() {
    assert_eq!(granary_fill(0), 0.0);
    let ten = granary_fill(10);
    let thousand = granary_fill(1_000);
    let cap = granary_fill(50_000);
    assert!(ten > 0.15, "10 samples must already show: {ten}");
    assert!(thousand > ten && cap > thousand, "fill is monotonic");
    assert!(cap > 0.95 && granary_fill(u64::MAX) <= 1.0);
}

#[test]
fn flavor_prefers_content_then_falls_back_to_reasoning() {
    let content: serde_json::Value = serde_json::json!({
        "choices": [{"message": {"content": "Hark, the forge sings anew!",
                                 "reasoning": "thinking..."}}]
    });
    assert_eq!(
        flavor_from_response(&content).as_deref(),
        Some("Hark, the forge sings anew!")
    );

    // Empty content + text stuck in `reasoning` (the qwen3.5 failure mode):
    // the last non-empty line is the utterance.
    let reasoning: serde_json::Value = serde_json::json!({
        "choices": [{"message": {"content": "",
            "reasoning": "Let me think of a line.\n\nThe bellows breathe again!"}}]
    });
    assert_eq!(
        flavor_from_response(&reasoning).as_deref(),
        Some("The bellows breathe again!")
    );

    // Nothing anywhere → None (caller uses the canned lines).
    let empty: serde_json::Value = serde_json::json!({"choices": [{"message": {"content": ""}}]});
    assert!(flavor_from_response(&empty).is_none());
}

#[test]
fn clip_line_caps_and_sheds_quotes() {
    assert_eq!(clip_line("\"a fine day\"", 40), "a fine day");
    assert_eq!(clip_line("first\nsecond", 40), "first");
    let long = "x".repeat(100);
    let clipped = clip_line(&long, 10);
    assert_eq!(clipped.chars().count(), 10);
    assert!(clipped.ends_with('~'));
}

#[test]
fn heads_json_yields_cottages_but_never_the_forge() {
    let v: serde_json::Value = serde_json::json!({"heads": [
        {"id": "dice", "baseURL": "http://127.0.0.1:8008", "health": "/health"},
        {"id": "forge", "baseURL": "http://127.0.0.1:8017", "health": "/health"},
        {"id": "ocr", "baseURL": "http://localhost:8018/"}
    ]});
    let heads = heads_from_json(&v);
    assert_eq!(heads.len(), 2, "the forge is a building, not a cottage");
    assert_eq!(heads[0].0, "dice");
    assert_eq!(heads[0].1, "http://127.0.0.1:8008/health");
    assert_eq!(heads[1].1, "http://localhost:8018/health");
    assert!(heads_from_json(&serde_json::json!({})).is_empty());
}

#[test]
fn head_registry_requires_an_explicit_file_and_otherwise_stays_local() {
    let _guard = crate::tests::env_lock();
    let _heads = crate::tests::TestEnvGuard::unset("ANGEL_HEADS_FILE");
    let fallback = discover_heads();
    assert!(
        fallback
            .iter()
            .all(|(_, url)| url.starts_with("http://127.0.0.1:"))
    );

    let path =
        std::env::temp_dir().join(format!("angel-village-heads-{}.json", std::process::id()));
    std::fs::write(
        &path,
        r#"{"heads":[{"id":"runner","baseURL":"http://runner.example:9000"}]}"#,
    )
    .unwrap();
    let _configured = crate::tests::TestEnvGuard::set(
        "ANGEL_HEADS_FILE",
        path.to_str().expect("temporary path is UTF-8"),
    );
    assert_eq!(
        discover_heads(),
        vec![(
            "runner".to_string(),
            "http://runner.example:9000/health".to_string()
        )]
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn network_polling_is_opt_in_and_fallback_urls_are_loopback() {
    let _guard = crate::tests::env_lock();
    let _village = crate::tests::TestEnvGuard::unset("ANGEL_VILLAGE");
    let _forge = crate::tests::TestEnvGuard::unset("ANGEL_FORGE_URL");
    let _voice = crate::tests::TestEnvGuard::unset("ANGEL_VILLAGE_VOICE_URL");
    assert!(!enabled());
    assert_eq!(forge_url(), "http://127.0.0.1:8017");
    assert_eq!(voice_url(), "http://127.0.0.1:11434");

    let _enabled = crate::tests::TestEnvGuard::set("ANGEL_VILLAGE", "1");
    let _forge_route =
        crate::tests::TestEnvGuard::set("ANGEL_FORGE_URL", "http://forge.example:8017");
    let _voice_route =
        crate::tests::TestEnvGuard::set("ANGEL_VILLAGE_VOICE_URL", "http://voice.example:11434");
    assert!(enabled());
    assert_eq!(forge_url(), "http://forge.example:8017");
    assert_eq!(voice_url(), "http://voice.example:11434");
}

#[test]
fn fallbacks_always_have_a_voice() {
    assert!(!fallback_line().is_empty());
    assert!(!fallback_name().is_empty());
    assert!(fallback_line().chars().count() <= CHATTER_MAX_CHARS);
}
