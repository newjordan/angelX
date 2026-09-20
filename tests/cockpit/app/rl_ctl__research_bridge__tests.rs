use super::*;

#[test]
fn sloptomizer_bundled_originals_match_source_receipt() {
    let manifest: Value =
        serde_json::from_slice(FILES.iter().find(|(p, _)| *p == "UPSTREAM.json").unwrap().1)
            .unwrap();
    assert_eq!(
        manifest["source_head"],
        "0e3c17b539bf14ffa6c546f25b648bb58b4514ac"
    );
    let rows = manifest["files"].as_array().unwrap();
    assert_eq!(rows.len(), 10);
    for row in rows {
        let (_, bytes) = FILES
            .iter()
            .find(|(p, _)| Some(*p) == row["path"].as_str())
            .unwrap();
        assert_eq!(
            crate::knowledge::cut::sha256_hex(bytes),
            row["sha256"].as_str().unwrap()
        );
    }
}

#[test]
fn sloptomizer_concurrent_feedback_is_durable_deduplicated_and_failure_preserves_state() {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-slop-store-{}", new_run_id()));
    let _env = crate::tests::TestEnvGuard::set("ANGEL_RL_DIR", root.to_str().unwrap());
    let scope = root.join("learning");
    // Synthetic bridge inputs test storage, not objective verification. Only
    // the production controller admits observations through physical evidence.
    let event = |i| {
        json!({"action":"observe","task":"fixture objective","observation":{
            "id":format!("synthetic-{i}"),"idea":format!("fixture idea {i}"),"approach":"fixture",
            "task":"fixture objective","receipt_sha256":"synthetic-receipt","source_sha256":"synthetic-source","passed":true
        }})
    };
    let workers = (0..4)
        .map(|i| {
            let scope = scope.clone();
            let request = event(i);
            std::thread::spawn(move || transform(&scope, request, &AtomicBool::new(false)).unwrap())
        })
        .collect::<Vec<_>>();
    for worker in workers {
        worker.join().unwrap();
    }
    let repeated = transform(&scope, event(0), &AtomicBool::new(false)).unwrap();
    assert_eq!(repeated["advice"]["observations"], 4);
    assert_eq!(repeated["updated"], false);
    let before = read(&scope.join("state.json")).unwrap();
    let mut conflict = event(0);
    conflict["observation"]["passed"] = json!(false);
    assert!(
        transform(&scope, conflict, &AtomicBool::new(false))
            .unwrap_err()
            .contains("conflicting observation")
    );
    assert_eq!(read(&scope.join("state.json")).unwrap(), before);
    assert!(
        transform(&scope, event(4), &AtomicBool::new(true))
            .unwrap_err()
            .contains("cancelled")
    );
    assert_eq!(read(&scope.join("state.json")).unwrap(), before);
    write(&scope.join("state.json"), b"null").unwrap();
    assert!(
        transform(&scope, json!({"action":"suggest"}), &AtomicBool::new(false))
            .unwrap_err()
            .contains("unsupported")
    );
    assert_eq!(read(&scope.join("state.json")).unwrap(), b"null");
    std::fs::remove_dir_all(&root).unwrap();
}
