use super::scratch;
use crate::agent::harness::swarm_compile::policy::PolicyStore;
use crate::agent::harness::swarm_compile::schema::{
    Contribution, CreditRow, ReviewVerdict, SwarmRun,
};
use crate::agent::harness::swarm_compile::store::RunStore;
use crate::agent::harness::swarm_compile::tool::SwarmCompilerTool;

#[test]
fn store_round_trips_versioned_run_and_append_only_event() {
    let root = scratch("store");
    let store = RunStore::new(root.clone());
    let id = store.new_run_id();
    let run = SwarmRun::new(
        id.clone(),
        "/tmp/work".into(),
        "/tmp/work".into(),
        ".".into(),
        "abc123".into(),
        "fix bug".into(),
        "bugfix".into(),
        "node test.mjs".into(),
        "node --test".into(),
        Vec::new(),
        vec!["tests".into()],
        vec!["a".into(), "b".into(), "c".into(), "d".into()],
        7,
    );
    store.save(&run).unwrap();
    let lease = store.acquire_lease(&id).unwrap();
    assert!(
        store.acquire_lease(&id).is_err(),
        "one run cannot have two live executors"
    );
    drop(lease);
    drop(store.acquire_lease(&id).unwrap());
    store.event(&id, "info", "created", "token=secret").unwrap();
    let loaded = store.load(&id).unwrap();
    assert_eq!(loaded.id, id);
    assert_eq!(loaded.nodes.len(), 5);
    let events = std::fs::read_to_string(store.run_dir(&id).unwrap().join("events.jsonl")).unwrap();
    assert!(events.contains("created"));
    assert!(
        !events.contains("secret"),
        "credential-like values are scrubbed"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let dir_mode = std::fs::metadata(store.run_dir(&id).unwrap())
            .unwrap()
            .permissions()
            .mode();
        let file_mode = std::fs::metadata(store.run_dir(&id).unwrap().join("run.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(dir_mode & 0o077, 0);
        assert_eq!(file_mode & 0o077, 0);
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn routing_policy_learns_only_from_post_action_credit() {
    let root = scratch("policy");
    let policy = PolicyStore::new(root.join("policy.json"));
    let credit = |club: &str, reward: f64| CreditRow {
        role: "implementer".into(),
        club: club.into(),
        reward,
        basis: "verified".into(),
        tokens: 100,
        elapsed_ms: 50,
    };
    policy
        .record("run-alpha", "bugfix", &[credit("alpha", 1.0)])
        .unwrap();
    assert_eq!(
        policy
            .select("bugfix", "implementer", &["alpha".into(), "untried".into()])
            .unwrap(),
        "untried",
        "an observed winner must not permanently starve an untried route"
    );
    policy
        .record("run-beta", "bugfix", &[credit("beta", 0.0)])
        .unwrap();
    policy
        .record("run-alpha", "bugfix", &[credit("alpha", 1.0)])
        .unwrap();
    let loaded = policy.load().unwrap();
    assert_eq!(
        loaded
            .priors
            .iter()
            .find(|prior| prior.club == "alpha")
            .unwrap()
            .post_action_trials,
        1,
        "a resumed finalization must not double-count one run"
    );
    assert_eq!(
        policy
            .select("bugfix", "implementer", &["beta".into(), "alpha".into()])
            .unwrap(),
        "alpha"
    );
    assert!(policy.path().exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn concurrent_run_completions_do_not_lose_routing_evidence() {
    let root = scratch("policy_concurrent");
    let policy = std::sync::Arc::new(PolicyStore::new(root.join("policy.json")));
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let handles = (0..8)
        .map(|index| {
            let policy = std::sync::Arc::clone(&policy);
            let barrier = std::sync::Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                policy
                    .record(
                        &format!("run-{index}"),
                        "feature",
                        &[CreditRow {
                            role: "implementer".into(),
                            club: "alpha".into(),
                            reward: 1.0,
                            basis: "verified".into(),
                            tokens: 10,
                            elapsed_ms: 20,
                        }],
                    )
                    .unwrap();
            })
        })
        .collect::<Vec<_>>();
    for handle in handles {
        handle.join().unwrap();
    }
    let loaded = policy.load().unwrap();
    assert_eq!(loaded.applied_run_ids.len(), 8);
    assert_eq!(loaded.priors.len(), 1);
    assert_eq!(loaded.priors[0].post_action_trials, 8);
    assert_eq!(loaded.priors[0].verified_wins, 8);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn deterministic_early_rejection_records_zero_credit() {
    let _guard = crate::tests::env_lock();
    let root = scratch("early_rejection_credit");
    let tool =
        SwarmCompilerTool::with_store(root.join("workspace"), Vec::new(), root.join("store"));
    let id = tool.engine.store.new_run_id();
    let mut run = SwarmRun::new(
        id.clone(),
        root.join("workspace").to_string_lossy().into_owned(),
        root.to_string_lossy().into_owned(),
        "workspace".into(),
        "abc123".into(),
        "fix bug".into(),
        "bugfix".into(),
        "test command".into(),
        "accept command".into(),
        Vec::new(),
        vec!["tests".into()],
        vec![
            "alpha".into(),
            "alpha".into(),
            "alpha".into(),
            "alpha".into(),
        ],
        7,
    );
    run.contributions.push(Contribution {
        task_id: "task-test_author".into(),
        role: "test_author".into(),
        club: "alpha".into(),
        resolved_route: Some("alpha".into()),
        model_revision: Some("alpha-v1".into()),
        base_oid: "abc123".into(),
        branch: Some("swarm/test".into()),
        diff_hash: Some("hash".into()),
        changed_paths: vec!["tests/regression.rs".into()],
        summary: "wrote a test that failed the deterministic gate".into(),
        elapsed_ms: 20,
        tokens: 10,
        review_verdict: None::<ReviewVerdict>,
    });
    tool.engine
        .reject(&mut run, "missing marker-bearing red proof")
        .unwrap();
    let policy = tool.engine.policy.load().unwrap();
    assert_eq!(run.credits.len(), 1);
    assert_eq!(run.credits[0].reward, 0.0);
    assert_eq!(policy.applied_run_ids, vec![id]);
    assert_eq!(policy.priors[0].post_action_trials, 1);
    assert_eq!(policy.priors[0].verified_wins, 0);
    assert_eq!(policy.priors[0].reward_sum, 0.0);
    let _ = std::fs::remove_dir_all(root);
}
