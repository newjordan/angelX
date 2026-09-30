use super::*;

#[test]
fn hiq_priority_prefers_offload_and_treebeard() {
    let _guard = crate::tests::env_lock();
    let _default_cap = crate::agent::harness::tests::EnvGuard::unset("FORGE_HIQ_WEIGHT_MAX");
    let bulk = hiq_priority(0.0, Lane::Default);
    let offloaded = hiq_priority(1.0, Lane::Default);
    let treebeard = hiq_priority(1.0, Lane::Treebeard);
    assert!(bulk < offloaded);
    assert!(offloaded < treebeard);
    assert!((bulk - 0.5).abs() < 1e-9);
    assert!((offloaded - 1.75).abs() < 1e-9);
    // Treebeard full offload = 1.75 * 1.25 = 2.1875 (under WEIGHT_MAX 3.0).
    assert!((treebeard - 1.75 * 1.25).abs() < 1e-9);
    assert!(treebeard < 2.5); // still room for PRIMARY competition bonuses at forge
    assert!(treebeard <= 3.0);
}

#[test]
fn hiq_priority_weight_max_env_raises_cap() {
    // Serialized: this test mutates process-global environment that other
    // tests also read or write. Without the lock they interleave and each
    // observes the other's value between its own set and restore.
    let _guard = crate::tests::env_lock();
    let _default_cap = crate::agent::harness::tests::EnvGuard::unset("FORGE_HIQ_WEIGHT_MAX");
    // Without env: default max 3.0 still leaves headroom after treebeard.
    let t = hiq_priority(1.0, Lane::Treebeard);
    assert!((t - 2.1875).abs() < 1e-9);
    // Explicit low cap still clamps (legacy-compatible).
    let _cap = crate::agent::harness::tests::EnvGuard::set("FORGE_HIQ_WEIGHT_MAX", "2.0");
    let capped = hiq_priority(1.0, Lane::Treebeard);
    assert!((capped - 2.0).abs() < 1e-9);
}

#[test]
fn offload_ratio_is_zero_without_tool_results() {
    assert_eq!(offload_ratio(0, 0, 0), 0.0);
    assert!((offload_ratio(1, 1, 2) - 0.5).abs() < 1e-9);
    assert!((offload_ratio(3, 1, 0) - 1.0).abs() < 1e-9);
}

#[test]
fn root_trajectory_json_notes_last_root_hiq() {
    let _guard = crate::tests::env_lock();
    note_last_root_hiq(LastRootHiq::default());
    let history = vec![
        ChatMsg::user("read the kernel under a handle"),
        ChatMsg::assistant_calls(vec![ToolCall {
            id: "c1".into(),
            name: "read_file".into(),
            args: serde_json::json!({"path": "k.py"}),
        }]),
        ChatMsg::tool(
            "c1",
            format!(
                "{TOOL_AGED_MARK}: read_file|k.py (5000 bytes) handle=hnd_1 — re-run if needed]"
            ),
        ),
        ChatMsg::assistant("parked under handle"),
    ];
    let v = root_trajectory_json(&history);
    assert!(v.get("offload_ratio").is_some());
    let snap = last_root_hiq().expect("snapshot after root_trajectory_json");
    assert!((snap.offload_ratio - v["offload_ratio"].as_f64().unwrap()).abs() < 1e-9);
    assert!((snap.hiq_priority - v["hiq_priority"].as_f64().unwrap()).abs() < 1e-9);
    assert_eq!(snap.handle_receipts, 1);
    assert!((snap.offload_ratio - 1.0).abs() < 1e-9);
}

#[test]
fn unlabeled_trajectory_still_refreshes_last_root_hiq() {
    let _guard = crate::tests::env_lock();
    // Strip must update even when root meta is omitted from the JSONL row.
    note_last_root_hiq(LastRootHiq::default());
    let history = vec![
        ChatMsg::user("inspect"),
        ChatMsg::assistant_calls(vec![ToolCall {
            id: "1".into(),
            name: "read_file".into(),
            args: serde_json::json!({"path": "a.rs"}),
        }]),
        ChatMsg::tool(
            "1",
            format!("{TOOL_AGED_MARK}: read_file|a.rs (9000 bytes) handle=hnd_z — re-run]"),
        ),
        ChatMsg::assistant("ok"),
    ];
    let rec = trajectory_record("fixture-club", &history, "ok", 2, false, None, 1);
    assert!(rec.get("root_trajectory").is_none());
    let snap = last_root_hiq().expect("unlabeled turn still notes strip snapshot");
    assert_eq!(snap.handle_receipts, 1);
    assert!((snap.offload_ratio - 1.0).abs() < 1e-9);
}

#[test]
fn harness_treatment_stamps_only_an_explicit_rl_reward() {
    // Serialized: this test mutates process-global environment that other
    // tests also read or write. Without the lock they interleave and each
    // observes the other's value between its own set and restore.
    let _guard = crate::tests::env_lock();
    let _lane = crate::agent::harness::tests::EnvGuard::set("ANGEL_LANE", "treebeard");
    let _reward = crate::agent::harness::tests::EnvGuard::unset("ANGEL_RL_REWARD");
    let t = harness_treatment_json();
    assert_eq!(t["lane"], "treebeard");
    assert!(t.get("rl_reward").is_none(), "unset scorer is not stamped");
    assert!(t.get("gpu_comp").is_none());
    let _reward = crate::agent::harness::tests::EnvGuard::set("ANGEL_RL_REWARD", "tests");
    assert_eq!(harness_treatment_json()["rl_reward"], "tests");
    assert!(
        harness_treatment_json()
            .get("rl_reward_requested")
            .is_none()
    );
    // A scorer no evaluator can feed is recorded as requested, not as used.
    let _reward = crate::agent::harness::tests::EnvGuard::set("ANGEL_RL_REWARD", "lint");
    let t = harness_treatment_json();
    assert_eq!(t["rl_reward"], "tests");
    assert_eq!(t["rl_reward_requested"], "lint");
}
