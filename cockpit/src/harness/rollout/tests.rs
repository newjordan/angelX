use super::audit_export::{
    CORPUS_EXPORT_SCHEMA, audit_store_rollout, audit_store_rollout_receipt,
    export_store_rollout_corpus_v2, export_store_rollout_v2,
};
use super::recorder::{RolloutRecorder, recover_rollout, request_for_test};
use super::schema::*;
use super::store::{RolloutStore, validate_rollout_id};
use crate::club::{ChatMsg, ClubReply, RouteIdentity, ToolCall, ToolDef};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let seq = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("angel-rollout-{name}-{}-{seq}", std::process::id()))
}

fn project() -> ProjectIdentity {
    ProjectIdentity {
        workspace_key: "fixture-workspace".to_string(),
        repo_key: "fixture-project".to_string(),
        canonical_root_sha256: crate::cut::sha256_hex(b"/fixture/project"),
    }
}

fn route() -> RouteIdentity {
    RouteIdentity {
        driver: "scripted".to_string(),
        model: Some("fixture-v1".to_string()),
        reasoning_effort: None,
    }
}

fn defs() -> Vec<ToolDef> {
    vec![ToolDef {
        name: "read_file".to_string(),
        description: "read one fixture".to_string(),
        params: serde_json::json!({
            "type": "object",
            "properties": { "path": { "type": "string" } },
            "required": ["path"]
        }),
    }]
}

#[test]
fn capture_mode_defaults_off_and_off_never_allocates_a_store() {
    assert_eq!(CaptureMode::from_setting(None), CaptureMode::Off);
    assert_eq!(CaptureMode::from_setting(Some("unknown")), CaptureMode::Off);
    assert_eq!(
        CaptureMode::from_setting(Some("shadow")),
        CaptureMode::Shadow
    );
    assert_eq!(CaptureMode::from_setting(Some("LOCAL")), CaptureMode::Local);

    let root = scratch("off");
    let store = RolloutStore::new(root.clone());
    let mut recorder = RolloutRecorder::start_for_test(store, CaptureMode::Off, project()).unwrap();
    assert!(
        recorder
            .begin_policy_attempt(&[ChatMsg::user("hello")], &defs(), || {
                panic!("off mode must not resolve route metadata")
            })
            .unwrap()
            .is_none()
    );
    recorder
        .finish_turn("answer", false, false, false, Some("done"))
        .unwrap();
    assert!(!root.exists(), "off mode must not create rollout storage");
}

#[test]
fn body_free_audit_receipt_covers_sealed_but_training_ineligible_rollout() {
    let root = scratch("audit-receipt");
    let store = RolloutStore::new(root.clone());
    let binding = TaskRolloutBindingV1::new(
        Some("task-fixture".to_string()),
        Some("run-fixture".to_string()),
        crate::cut::sha256_hex(b"operator prompt"),
        crate::cut::sha256_hex(b"runtime config"),
        env!("CARGO_PKG_VERSION").to_string(),
        "unbound".to_string(),
    );
    let mut recorder = RolloutRecorder::start_for_test_with_binding(
        store.clone(),
        CaptureMode::Shadow,
        project(),
        Some(binding.clone()),
    )
    .unwrap();
    let rollout_id = recorder.rollout_id().unwrap().to_string();
    let attempt = recorder
        .begin_policy_attempt(&[ChatMsg::user("fixture task")], &defs(), route)
        .unwrap();
    recorder
        .complete_policy_attempt(attempt, &ClubReply::Text("done".to_string()), route)
        .unwrap();
    recorder
        .finish_turn("answer", false, false, false, Some("done"))
        .unwrap();

    let receipt = audit_store_rollout_receipt(&store, &rollout_id, "fixture-project").unwrap();
    assert_eq!(receipt["schema"], "angel-harness-rollout-audit/v1");
    assert_eq!(receipt["rollout_id"], rollout_id);
    assert_eq!(receipt["status"], "finalized");
    assert_eq!(receipt["attempt_count"], 1);
    assert_eq!(receipt["project"]["schema"], "angel-project-digest/v1");
    assert!(receipt["project"].get("workspace_key").is_none());
    assert!(receipt["project"].get("repo_key").is_none());
    assert!(!receipt.to_string().contains("fixture-workspace"));
    assert!(!receipt.to_string().contains("fixture-project"));
    assert_eq!(receipt["task_binding"]["task_id"], "task-fixture");
    assert_eq!(receipt["task_binding"]["run_id"], "run-fixture");
    assert_eq!(
        receipt["task_binding"]["binding_sha256"],
        binding.binding_sha256
    );
    assert!(receipt.get("attempts").is_none());
    assert!(receipt["manifest_sha256"].as_str().is_some());
    let receipt_sha256 = receipt["receipt_sha256"].as_str().unwrap().to_string();
    let mut unsigned = receipt.clone();
    unsigned.as_object_mut().unwrap().remove("receipt_sha256");
    assert_eq!(
        receipt_sha256,
        crate::cut::sha256_hex(&serde_json::to_vec(&unsigned).unwrap())
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn body_free_audit_resolved_actions_preserve_fallback_mixed_unknown_and_no_action() {
    for (name, models, expected) in [
        (
            "fallback",
            vec![Some("fixture-b")],
            serde_json::json!({
                "schema": "angel-resolved-actions/v1", "completed_actions": 1,
                "unresolved_actions": 0, "routes": [{"route": {"driver": "scripted",
                    "model_revision": "fixture-b", "reasoning_effort": null}, "completed_actions": 1}]
            }),
        ),
        (
            "mixed",
            vec![Some("fixture-c"), Some("fixture-b"), Some("fixture-b")],
            serde_json::json!({
                "schema": "angel-resolved-actions/v1", "completed_actions": 3,
                "unresolved_actions": 0, "routes": [
                    {"route": {"driver": "scripted", "model_revision": "fixture-b", "reasoning_effort": null}, "completed_actions": 2},
                    {"route": {"driver": "scripted", "model_revision": "fixture-c", "reasoning_effort": null}, "completed_actions": 1}]
            }),
        ),
        (
            "unknown",
            vec![None],
            serde_json::json!({
                "schema": "angel-resolved-actions/v1", "completed_actions": 1,
                "unresolved_actions": 1, "routes": []
            }),
        ),
        (
            "no-action",
            vec![],
            serde_json::json!({
                "schema": "angel-resolved-actions/v1", "completed_actions": 0,
                "unresolved_actions": 0, "routes": []
            }),
        ),
    ] {
        let root = scratch(name);
        let store = RolloutStore::new(root.clone());
        let mut recorder =
            RolloutRecorder::start_for_test(store.clone(), CaptureMode::Shadow, project()).unwrap();
        let id = recorder.rollout_id().unwrap().to_string();
        let failed = recorder
            .begin_policy_attempt(&[ChatMsg::user("owned task")], &defs(), route)
            .unwrap();
        recorder
            .fail_policy_attempt(failed, "scripted failed request", false, true)
            .unwrap();
        for model in &models {
            let attempt = recorder
                .begin_policy_attempt(&[ChatMsg::user("owned task")], &defs(), route)
                .unwrap();
            recorder
                .complete_policy_attempt(attempt, &ClubReply::Text("done".into()), || {
                    RouteIdentity {
                        model: model.map(str::to_owned),
                        ..route()
                    }
                })
                .unwrap();
        }
        if models.is_empty() {
            recorder
                .finish_turn("provider_error", false, false, false, None)
                .unwrap();
        } else {
            recorder
                .finish_turn("answer", false, false, false, Some("done"))
                .unwrap();
        }
        let receipt = audit_store_rollout_receipt(&store, &id, "fixture-project").unwrap();
        assert_eq!(receipt["attempt_count"], models.len() + 1);
        assert_eq!(receipt["resolved_actions"], expected, "case {name}");
        assert!(!receipt.to_string().contains("owned task"));
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn task_rollout_binding_rejects_tampering() {
    let mut binding = TaskRolloutBindingV1::new(
        Some("task-fixture".to_string()),
        Some("run-fixture".to_string()),
        crate::cut::sha256_hex(b"operator prompt"),
        crate::cut::sha256_hex(b"runtime config"),
        env!("CARGO_PKG_VERSION").to_string(),
        "unbound".to_string(),
    );
    binding.prompt_sha256 = crate::cut::sha256_hex(b"different prompt");
    assert!(binding.validate().is_err());
}

#[test]
fn hostile_rollout_ids_and_symlinked_run_dirs_fail_closed() {
    for invalid in [
        "",
        "rol-",
        "rol-deadbeef",
        "rol-dead-beef",
        "rol-dead-beef-zero",
        "rol-dead-beef-0/escape",
        "rol-dead-beef-0..",
        "../rol-dead-beef-0",
        "ROL-dead-beef-0",
    ] {
        assert!(validate_rollout_id(invalid).is_err(), "{invalid:?}");
    }
    assert!(validate_rollout_id("rol-dead-beef-0").is_ok());

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let root = scratch("hostile-id");
        let outside = scratch("hostile-id-outside");
        std::fs::create_dir_all(root.join("runs")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        symlink(&outside, root.join("runs/rol-dead-beef-0")).unwrap();
        let store = RolloutStore::new(root.clone());
        let error = store.discover_rollout_ids().unwrap_err();
        assert!(error.contains("symlinked rollout run"), "{error}");
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(outside);
    }
}

#[test]
fn retries_preserve_step_order_and_tool_pairs_advance_the_policy_step() {
    let root = scratch("ordering");
    let store = RolloutStore::new(root.clone());
    let mut recorder =
        RolloutRecorder::start_for_test(store.clone(), CaptureMode::Shadow, project()).unwrap();
    let rollout_id = recorder.rollout_id().unwrap().to_string();
    let mut history = vec![ChatMsg::system("fixture policy"), ChatMsg::user("inspect")];

    let first = recorder
        .begin_policy_attempt(&history, &defs(), route)
        .unwrap();
    recorder
        .fail_policy_attempt(first, "transient transport reset", false, true)
        .unwrap();

    let calls = vec![ToolCall {
        id: "call-1".to_string(),
        name: "read_file".to_string(),
        args: serde_json::json!({"path": "src/lib.rs"}),
    }];
    let retry = recorder
        .begin_policy_attempt(&history, &defs(), route)
        .unwrap();
    recorder
        .complete_policy_attempt(retry, &ClubReply::Calls(calls.clone()), route)
        .unwrap();

    history.push(ChatMsg::assistant_calls(calls));
    history.push(ChatMsg::tool("call-1", "fixture result"));
    let second_step = recorder
        .begin_policy_attempt(&history, &defs(), route)
        .unwrap();
    recorder
        .complete_policy_attempt(second_step, &ClubReply::Text("done".to_string()), route)
        .unwrap();
    let compatibility_evaluated = std::cell::Cell::new(false);
    recorder
        .attach_compatibility_snapshot(|| {
            compatibility_evaluated.set(true);
            (
                "must-not-persist".to_string(),
                serde_json::json!({"semantic": "must-not-persist"}),
                serde_json::json!({"treatment": "must-not-persist"}),
            )
        })
        .unwrap();
    assert!(
        !compatibility_evaluated.get(),
        "shadow capture must not evaluate semantic compatibility data"
    );
    recorder
        .finish_turn("answer", false, false, false, Some("done"))
        .unwrap();

    let manifest = store
        .load_manifest(&rollout_id, &project().repo_key)
        .unwrap();
    let order = manifest
        .attempts
        .iter()
        .map(|attempt| (attempt.step_index, attempt.attempt_index))
        .collect::<Vec<_>>();
    assert_eq!(order, vec![(0, 0), (0, 1), (1, 0)]);
    assert!(matches!(
        &manifest.attempts[0].outcome,
        PolicyStepOutcome::ProviderFailedNoAction { .. }
    ));
    let mut partial_retry = manifest.attempts.clone();
    for retryable in [false, true] {
        partial_retry[0].outcome = PolicyStepOutcome::ProviderFailedAfterPartial {
            failure: InfrastructureFailure {
                class: InfrastructureClass::ProviderPartialOutput,
                retryable,
                detail_sha256: "a".repeat(64),
            },
        };
        assert_eq!(
            validate_attempt_order(&partial_retry).is_ok(),
            retryable,
            "partial output only admits an explicitly retryable transport failure"
        );
    }
    assert!(matches!(
        &manifest.attempts[1].outcome,
        PolicyStepOutcome::ToolCallsAction
    ));
    assert!(
        manifest
            .attempts
            .iter()
            .all(|attempt| attempt.request.tool_pairing_valid)
    );
    assert_eq!(manifest.status, RolloutStatus::Finalized);
    assert!(manifest.compatibility.is_none());
    assert_eq!(
        manifest.eligibility,
        TrainingEligibility::Excluded {
            reason: ExclusionReason::MissingReward,
            detail_sha256: None
        }
    );
    assert!(
        !store.root().join("blobs").exists(),
        "shadow mode stores semantic hashes, not bodies"
    );
    assert!(
        export_store_rollout_v2(&store, &rollout_id, &project().repo_key)
            .unwrap_err()
            .contains("local semantic bodies")
    );
    let mut unknown = serde_json::to_value(&manifest).unwrap();
    unknown
        .as_object_mut()
        .unwrap()
        .insert("future_authority".to_string(), serde_json::json!(true));
    assert!(serde_json::from_value::<HarnessRolloutV1>(unknown).is_err());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let run_dir = store.run_dir(&rollout_id).unwrap();
        assert_eq!(
            std::fs::metadata(store.root())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(run_dir.join("journal.jsonl"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(run_dir.join("manifest.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        std::fs::set_permissions(
            run_dir.join("manifest.json"),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert!(
            store
                .load_manifest(&rollout_id, &project().repo_key)
                .is_err(),
            "unsafe manifest permissions must fail closed"
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn orphan_tool_results_are_typed_as_ineligible_pairing() {
    let root = scratch("pairing");
    let store = RolloutStore::new(root.clone());
    let valid = vec![
        ChatMsg::assistant_calls(vec![ToolCall {
            id: "call-1".to_string(),
            name: "read_file".to_string(),
            args: serde_json::json!({"path": "a"}),
        }]),
        ChatMsg::tool("call-1", "ok"),
    ];
    let (request, _) = request_for_test(&store, CaptureMode::Shadow, &valid, &defs()).unwrap();
    assert!(request.tool_pairing_valid);

    let orphan = vec![ChatMsg::tool("missing-call", "not paired")];
    let (request, _) = request_for_test(&store, CaptureMode::Shadow, &orphan, &defs()).unwrap();
    assert!(!request.tool_pairing_valid);

    let missing_result = vec![ChatMsg::assistant_calls(vec![ToolCall {
        id: "call-missing".to_string(),
        name: "read_file".to_string(),
        args: serde_json::json!({"path": "a"}),
    }])];
    let (request, _) =
        request_for_test(&store, CaptureMode::Shadow, &missing_result, &defs()).unwrap();
    assert!(!request.tool_pairing_valid);

    let duplicate_result = vec![
        ChatMsg::assistant_calls(vec![ToolCall {
            id: "call-duplicate".to_string(),
            name: "read_file".to_string(),
            args: serde_json::json!({"path": "a"}),
        }]),
        ChatMsg::tool("call-duplicate", "first"),
        ChatMsg::tool("call-duplicate", "second"),
    ];
    let (request, _) =
        request_for_test(&store, CaptureMode::Shadow, &duplicate_result, &defs()).unwrap();
    assert!(!request.tool_pairing_valid);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn recovery_ignores_a_torn_tail_and_corruption_fails_closed() {
    let root = scratch("recovery");
    let store = RolloutStore::new(root.clone());
    let mut recorder =
        RolloutRecorder::start_for_test(store.clone(), CaptureMode::Shadow, project()).unwrap();
    let rollout_id = recorder.rollout_id().unwrap().to_string();
    let token = recorder
        .begin_policy_attempt(&[ChatMsg::user("work")], &defs(), route)
        .unwrap();
    recorder
        .fail_policy_attempt(token, "provider unavailable", false, true)
        .unwrap();
    drop(recorder);

    let journal = store.run_dir(&rollout_id).unwrap().join("journal.jsonl");
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(&journal)
        .unwrap()
        .write_all(br#"{"torn":"#)
        .unwrap();
    let mut recovered = recover_rollout(&store, &rollout_id, &project().repo_key).unwrap();
    assert_eq!(recovered.status, RolloutStatus::Incomplete);
    assert_eq!(
        recovered.termination.as_ref().map(|value| value.kind),
        Some(TerminationKind::ProcessLost)
    );
    assert!(
        store
            .load_manifest(&rollout_id, &project().repo_key)
            .is_ok()
    );
    assert!(
        store.save_manifest(&mut recovered).is_err(),
        "a sealed rollout must not be rewritten in place"
    );

    let corrupt_root = scratch("corrupt");
    let corrupt_store = RolloutStore::new(corrupt_root.clone());
    let corrupt_recorder =
        RolloutRecorder::start_for_test(corrupt_store.clone(), CaptureMode::Shadow, project())
            .unwrap();
    let corrupt_id = corrupt_recorder.rollout_id().unwrap().to_string();
    drop(corrupt_recorder);
    let corrupt_journal = corrupt_store
        .run_dir(&corrupt_id)
        .unwrap()
        .join("journal.jsonl");
    let mut bytes = std::fs::read(&corrupt_journal).unwrap();
    let needle = br#""seq":0"#;
    let start = bytes
        .windows(needle.len())
        .position(|window| window == needle)
        .unwrap();
    bytes[start + needle.len() - 1] = b'9';
    std::fs::write(&corrupt_journal, bytes).unwrap();
    assert!(corrupt_store.audit_journal(&corrupt_id).is_err());
    assert!(recover_rollout(&corrupt_store, &corrupt_id, &project().repo_key).is_err());

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(corrupt_root);
}

#[test]
fn local_capture_rejects_known_escaped_and_provider_secrets_before_blob_write() {
    let _lock = crate::tests::env_lock();
    let secret = "fixture-quoted-\"credential\"\\01234567";
    let _key = crate::tests::TestEnvGuard::set("ANGEL_T_ROLLOUT_SECRET", secret);
    let root = scratch("secret-coverage");
    let store = RolloutStore::new(root.clone());
    let samples = [
        secret.to_string(),
        "hf_abcdefghijklmnopqrstuvwxyz01234567".to_string(),
        "AIzaSyabcdefghijklmnopqrstuvwxyz0123456".to_string(),
        serde_json::json!({"value": secret}).to_string(),
        serde_json::json!({"access_token": "opaque-short"}).to_string(),
    ];
    for sample in &samples {
        assert!(
            store.put_blob(sample.as_bytes()).is_err(),
            "direct blob writes must enforce the same boundary"
        );
        let history = vec![ChatMsg::user(sample.as_str())];
        let (request, signals) =
            request_for_test(&store, CaptureMode::Local, &history, &defs()).unwrap();
        assert!(signals.secret_detected);
        let content = &request.messages[0].content;
        assert_eq!(content.storage, BlobStorage::Absent);
        assert_eq!(content.sensitivity, Sensitivity::SecretRejected);
        assert_eq!(
            content.sha256,
            crate::cut::sha256_hex(sample.as_bytes()),
            "rejected content keeps its exact original identity"
        );
        assert!(!store.root().join("blobs").join(&content.sha256).exists());
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn ordinary_tool_schema_metadata_is_captured_without_cross_field_secret_matches() {
    let _lock = crate::tests::env_lock();
    let root = scratch("schema-metadata");
    let store = RolloutStore::new(root.clone());
    let mut definitions = defs();
    definitions[0].description = "task-conditioned capability search".into();
    definitions[0].params = serde_json::json!({
        "type": "object", "properties": {
            "query": {"description": "capability keywords", "type": "string"},
            "cache_key": {"description": "stable cache identity", "type": "string"}
        }
    });
    let (request, signals) = request_for_test(
        &store,
        CaptureMode::Local,
        &[ChatMsg::user("search capabilities")],
        &definitions,
    )
    .unwrap();
    assert!(!signals.secret_detected);
    assert_eq!(request.tool_schema_set.sensitivity, Sensitivity::Project);
    assert_eq!(request.tool_schema_set.storage, BlobStorage::Local);
    let bytes = store.read_blob(&request.tool_schema_set).unwrap();
    let captured: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(captured[0]["params"], definitions[0].params);
    assert_eq!(captured[0]["description"], definitions[0].description);

    // Structured/escaped transport credentials still trigger rejection, and
    // exact identity survives rejection. These are owned dummy credentials.
    for text in [
        "https://fixture-user:fixture-pass@localhost/path",
        "curl --token fixture-value",
        "Authorization: Bearer fixture-value",
    ] {
        definitions[0].params["description"] = serde_json::json!(text);
        let (request, signals) = request_for_test(
            &store,
            CaptureMode::Local,
            &[ChatMsg::user("search capabilities")],
            &definitions,
        )
        .unwrap();
        assert!(signals.secret_detected);
        assert_eq!(request.tool_schema_set.storage, BlobStorage::Absent);
        assert_eq!(
            request.tool_schema_set.sensitivity,
            Sensitivity::SecretRejected
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn secret_tool_schema_remains_excluded_and_auditable_without_storing_body() {
    let _lock = crate::tests::env_lock();
    for mode in [CaptureMode::Local, CaptureMode::Shadow] {
        let root = scratch("secret-schema-audit");
        let store = RolloutStore::new(root.clone());
        let mut definitions = defs();
        definitions[0].params["default"] =
            serde_json::json!({"access_token": "fixture-private-value"});
        let mut recorder = RolloutRecorder::start_for_test(store.clone(), mode, project()).unwrap();
        let id = recorder.rollout_id().unwrap().to_string();
        let attempt = recorder
            .begin_policy_attempt(&[ChatMsg::user("fixture task")], &definitions, route)
            .unwrap();
        recorder
            .complete_policy_attempt(attempt, &ClubReply::Text("done".to_string()), route)
            .unwrap();
        recorder
            .finish_turn("answer", false, false, false, Some("done"))
            .unwrap();
        let manifest = store.load_manifest(&id, &project().repo_key).unwrap();
        let request = &manifest.attempts[0].request;
        assert_eq!(
            request.messages[0].content.sensitivity,
            Sensitivity::Project
        );
        assert_eq!(
            request.tool_schema_set.sensitivity,
            Sensitivity::SecretRejected
        );
        assert_eq!(request.tool_schema_set.storage, BlobStorage::Absent);
        assert!(
            !store
                .root()
                .join("blobs")
                .join(&request.tool_schema_set.sha256)
                .exists()
        );
        assert_eq!(
            manifest.eligibility,
            TrainingEligibility::Excluded {
                reason: ExclusionReason::SecretDetected,
                detail_sha256: None,
            }
        );
        // Recorder and independent audit must agree even when only the schema
        // contains a secret. Excluded evidence still needs an auditable receipt.
        audit_store_rollout_receipt(&store, &id, &project().repo_key).unwrap();
        assert!(export_store_rollout_v2(&store, &id, &project().repo_key).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn secret_metadata_cannot_enter_a_hashed_rollout_journal() {
    use super::store::{JournalCursor, JournalEventKind};
    let _lock = crate::tests::env_lock();
    let root = scratch("secret-metadata");
    let store = RolloutStore::new(root.clone());
    let mut cursor = JournalCursor::default();
    let mut identity = project();
    identity.repo_key = "hf_abcdefghijklmnopqrstuvwxyz01234567".to_string();
    let kind = JournalEventKind::RolloutStarted {
        project: identity,
        task_binding: None,
        capture: CaptureDescriptor::new(CaptureMode::Local),
        requested_route: route().into(),
        started_ms: 1,
    };
    let prior = cursor.clone();
    assert!(
        store
            .append_event("fixture-secret", &mut cursor, kind)
            .is_err()
    );
    assert_eq!(cursor, prior);
    assert!(
        !root.exists(),
        "reject before opening the journal or hashing the event"
    );
}

#[test]
fn local_capture_rejects_secret_bodies_and_has_no_reasoning_or_header_fields() {
    let root = scratch("secrets");
    let store = RolloutStore::new(root.clone());
    let history = vec![ChatMsg::user(
        "Authorization: Bearer never-write-this-credential",
    )];
    let (request, signals) =
        request_for_test(&store, CaptureMode::Local, &history, &defs()).unwrap();
    assert!(signals.secret_detected);
    assert_eq!(request.messages[0].content.storage, BlobStorage::Absent);
    assert_eq!(
        request.messages[0].content.sensitivity,
        Sensitivity::SecretRejected
    );
    let capture = CaptureDescriptor::new(CaptureMode::Local);
    assert!(!capture.private_reasoning_captured);
    assert!(!capture.provider_headers_captured);

    let blob_dir = store.root().join("blobs");
    for entry in std::fs::read_dir(blob_dir).unwrap() {
        let body = std::fs::read(entry.unwrap().path()).unwrap();
        assert!(!String::from_utf8_lossy(&body).contains("never-write-this-credential"));
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn eligibility_requires_clean_structure_and_independent_reward_evidence() {
    let answer = Termination::from_turn("answer", false, false, false, Some("done"));
    let signals = EligibilitySignals {
        tool_pairing_valid: true,
        ..EligibilitySignals::default()
    };
    assert_eq!(
        TrainingEligibility::decide(&answer, None, signals),
        TrainingEligibility::Excluded {
            reason: ExclusionReason::MissingReward,
            detail_sha256: None
        }
    );
    let receipt = RewardReceipt::new(
        RewardOwner::CodingEval,
        1.0,
        crate::cut::sha256_hex(b"independent evidence"),
    )
    .unwrap();
    assert_eq!(
        TrainingEligibility::decide(&answer, Some(&receipt), signals),
        TrainingEligibility::Eligible
    );

    let provider = Termination::from_turn("provider_error", false, false, false, None);
    assert!(matches!(
        TrainingEligibility::decide(&provider, Some(&receipt), signals),
        TrainingEligibility::Excluded {
            reason: ExclusionReason::InfrastructureFailure,
            ..
        }
    ));
    assert_eq!(
        TrainingEligibility::decide(
            &answer,
            Some(&receipt),
            EligibilitySignals {
                secret_detected: true,
                ..signals
            }
        ),
        TrainingEligibility::Excluded {
            reason: ExclusionReason::SecretDetected,
            detail_sha256: None
        }
    );
    assert!(matches!(
        TrainingEligibility::decide(
            &provider,
            Some(&receipt),
            EligibilitySignals {
                reward_owner_conflict: true,
                ..signals
            }
        ),
        TrainingEligibility::Excluded {
            reason: ExclusionReason::InfrastructureFailure,
            ..
        }
    ));
}

#[test]
fn cut_reward_attaches_once_from_digest_only_evidence_without_raw_text() {
    let root = scratch("cut-reward");
    let store = RolloutStore::new(root.clone());
    let mut recorder =
        RolloutRecorder::start_for_test(store.clone(), CaptureMode::Local, project()).unwrap();
    let rollout_id = recorder.rollout_id().unwrap().to_string();
    let history = vec![ChatMsg::user("make the source compile")];
    let token = recorder
        .begin_policy_attempt(&history, &defs(), route)
        .unwrap();
    recorder
        .complete_policy_attempt(token, &ClubReply::Text("done".to_string()), route)
        .unwrap();

    recorder.observe_cut_machine(&serde_json::json!({
        "cmd": "cargo check --token never-store-command-secret",
        "dir": "/operator/private/worktree",
        "source": "heuristic",
        "exit": 0,
        "timed_out": false,
        "dur_ms": 42,
        "err": "provider/operator diagnostic never-store-output-secret",
    }));
    recorder.attach_cut_reward(Some(1.0), true).unwrap();
    recorder
        .attach_cut_reward(Some(1.0), true)
        .expect("the identical final Cut receipt is idempotent");
    recorder
        .attach_compatibility_snapshot(|| {
            (
                "scripted".to_string(),
                serde_json::json!({"fingerprint": "root"}),
                serde_json::json!({"lane": "fixture"}),
            )
        })
        .unwrap();
    recorder
        .finish_turn("answer", false, false, false, Some("done"))
        .unwrap();

    let audit = audit_store_rollout(&store, &rollout_id, &project().repo_key).unwrap();
    let receipt = audit.manifest.reward.as_ref().unwrap();
    assert_eq!(receipt.owner, RewardOwner::Cut);
    assert_eq!(receipt.contract, RewardContract::CutTurnVerdictV1);
    assert_eq!(receipt.evidence_storage, RewardEvidenceStorage::RolloutBlob);
    assert_eq!(audit.manifest.eligibility, TrainingEligibility::Eligible);
    let evidence = store
        .read_blob_by_digest(&receipt.evaluator_evidence_sha256)
        .unwrap();
    let evidence_text = String::from_utf8(evidence).unwrap();
    assert!(evidence_text.contains("angel-cut-evidence/v1"));
    for forbidden in [
        "never-store-command-secret",
        "operator/private",
        "never-store-output-secret",
        "provider/operator",
    ] {
        assert!(
            !evidence_text.contains(forbidden),
            "Cut evidence leaked raw text: {forbidden}"
        );
    }
    let reward_events = store
        .audit_journal(&rollout_id)
        .unwrap()
        .into_iter()
        .filter(|event| {
            matches!(
                event.kind,
                super::store::JournalEventKind::RewardAttached { .. }
            )
        })
        .count();
    assert_eq!(reward_events, 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn audited_cut_reward_must_equal_the_reward_recomputed_from_evidence() {
    let root = scratch("cut-reward-mismatch");
    let store = RolloutStore::new(root.clone());
    let mut recorder =
        RolloutRecorder::start_for_test(store.clone(), CaptureMode::Local, project()).unwrap();
    let rollout_id = recorder.rollout_id().unwrap().to_string();
    let token = recorder
        .begin_policy_attempt(&[ChatMsg::user("make the source compile")], &defs(), route)
        .unwrap();
    recorder
        .complete_policy_attempt(token, &ClubReply::Text("done".to_string()), route)
        .unwrap();
    recorder.observe_cut_machine(&serde_json::json!({
        "cmd": "cargo check",
        "dir": ".",
        "source": "default",
        "exit": 0,
        "timed_out": false,
        "dur_ms": 1,
    }));
    recorder
        .attach_cut_reward(Some(0.5), true)
        .expect("the recorder accepts a typed receipt before independent audit");
    recorder
        .attach_compatibility_snapshot(|| {
            (
                "scripted".to_string(),
                serde_json::json!({}),
                serde_json::json!({}),
            )
        })
        .unwrap();
    recorder
        .finish_turn("answer", false, false, false, Some("done"))
        .unwrap();

    assert!(
        audit_store_rollout(&store, &rollout_id, &project().repo_key)
            .unwrap_err()
            .contains("does not match"),
        "auditing must recompute Cut's reward instead of trusting the receipt"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_second_reward_owner_fails_closed_without_replacing_the_first() {
    let root = scratch("reward-owner");
    let store = RolloutStore::new(root.clone());
    let mut recorder =
        RolloutRecorder::start_for_test(store.clone(), CaptureMode::Local, project()).unwrap();
    let rollout_id = recorder.rollout_id().unwrap().to_string();
    let token = recorder
        .begin_policy_attempt(&[ChatMsg::user("answer")], &defs(), route)
        .unwrap();
    recorder
        .complete_policy_attempt(token, &ClubReply::Text("done".to_string()), route)
        .unwrap();
    let coding = RewardReceipt::new(
        RewardOwner::CodingEval,
        1.0,
        crate::cut::sha256_hex(b"coding evidence"),
    )
    .unwrap();
    recorder.attach_reward(coding.clone()).unwrap();
    let verifier = RewardReceipt::new(
        RewardOwner::SwarmVerifier,
        1.0,
        crate::cut::sha256_hex(b"swarm evidence"),
    )
    .unwrap();
    assert!(recorder.attach_reward(verifier).is_err());
    recorder
        .attach_compatibility_snapshot(|| {
            (
                "scripted".to_string(),
                serde_json::json!({}),
                serde_json::json!({}),
            )
        })
        .unwrap();
    recorder
        .finish_turn("answer", false, false, false, Some("done"))
        .unwrap();

    let manifest = store
        .load_manifest(&rollout_id, &project().repo_key)
        .unwrap();
    assert_eq!(manifest.reward, Some(coding));
    assert_eq!(
        manifest.eligibility,
        TrainingEligibility::Excluded {
            reason: ExclusionReason::RewardOwnerConflict,
            detail_sha256: None,
        }
    );
    assert_eq!(
        store
            .audit_journal(&rollout_id)
            .unwrap()
            .iter()
            .filter(|event| matches!(
                &event.kind,
                super::store::JournalEventKind::RewardAttachmentRejected { .. }
            ))
            .count(),
        1,
        "the journal must independently prove the owner conflict"
    );
    audit_store_rollout(&store, &rollout_id, &project().repo_key)
        .expect("the conflict exclusion must survive audited reconstruction");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn audited_v1_export_is_deterministic_and_retains_structured_pairing_and_mask() {
    let root = scratch("export-v2");
    let store = RolloutStore::new(root.clone());
    let mut recorder =
        RolloutRecorder::start_for_test(store.clone(), CaptureMode::Local, project()).unwrap();
    let rollout_id = recorder.rollout_id().unwrap().to_string();
    let mut history = vec![
        ChatMsg::system("fixture policy"),
        ChatMsg::user("inspect"),
        ChatMsg::harness("derived harness direction"),
    ];
    let calls = vec![ToolCall {
        id: "call-1".to_string(),
        name: "read_file".to_string(),
        args: serde_json::json!({"path": "src/lib.rs"}),
    }];
    let first = recorder
        .begin_policy_attempt(&history, &defs(), route)
        .unwrap();
    recorder
        .complete_policy_attempt(first, &ClubReply::Calls(calls.clone()), route)
        .unwrap();
    history.push(ChatMsg::assistant_calls(calls));
    history.push(ChatMsg::tool("call-1", "fixture result"));
    let second = recorder
        .begin_policy_attempt(&history, &defs(), route)
        .unwrap();
    recorder
        .complete_policy_attempt(second, &ClubReply::Text("finished".to_string()), route)
        .unwrap();
    recorder
        .attach_reward(
            RewardReceipt::new(
                RewardOwner::CodingEval,
                1.0,
                crate::cut::sha256_hex(b"independent coding-eval manifest"),
            )
            .unwrap(),
        )
        .unwrap();
    recorder
        .attach_compatibility_snapshot(|| {
            (
                "scripted".to_string(),
                serde_json::json!({"fingerprint": "root-fingerprint"}),
                serde_json::json!({"lane": "treebeard"}),
            )
        })
        .unwrap();
    recorder
        .finish_turn("answer", false, false, false, Some("finished"))
        .unwrap();

    let first = export_store_rollout_v2(&store, &rollout_id, &project().repo_key).unwrap();
    let second = export_store_rollout_v2(&store, &rollout_id, &project().repo_key).unwrap();
    assert_eq!(first, second);
    assert_eq!(first["schema"], "angel-trajectory/v2");
    assert_eq!(first["source_rollout_id"], rollout_id);
    assert_eq!(first["reward_owner"], "coding_eval");
    assert_eq!(first["messages"].as_array().unwrap().len(), 6);
    assert_eq!(first["messages"][2]["origin"], "harness");
    assert_eq!(first["messages"][3]["tool_calls"][0]["id"], "call-1");
    assert_eq!(first["messages"][4]["tool_call_id"], "call-1");
    assert_eq!(first["structured_tool_pairing"]["preserved"], true);
    assert_eq!(
        first["loss_mask"]["schema"],
        "angel-assistant-action-loss-mask/v1"
    );
    let targets = first["loss_mask"]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["target"].as_bool().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(targets, vec![false, false, false, true, false, true]);
    assert_eq!(first["policy_steps"].as_array().unwrap().len(), 2);
    assert_eq!(first["root_trajectory"]["fingerprint"], "root-fingerprint");
    assert_eq!(first["harness_treatment"]["lane"], "treebeard");
    assert_eq!(
        crate::cut::sha256_hex(&serde_json::to_vec(&first).unwrap()),
        crate::cut::sha256_hex(&serde_json::to_vec(&second).unwrap())
    );
    let corpus_first = export_store_rollout_corpus_v2(&store, &project()).unwrap();
    let corpus_second = export_store_rollout_corpus_v2(&store, &project()).unwrap();
    assert_eq!(corpus_first, corpus_second);
    assert_eq!(corpus_first["schema"], CORPUS_EXPORT_SCHEMA);
    assert_eq!(corpus_first["audit"]["scanned"], 1);
    assert_eq!(corpus_first["audit"]["audited"], 1);
    assert_eq!(corpus_first["audit"]["rejected"], 0);
    assert_eq!(corpus_first["audit"]["exported"], 1);
    assert_eq!(corpus_first["audit"]["by_reward_owner"]["coding_eval"], 1);
    assert_eq!(corpus_first["records"][0], first);
    let audited = audit_store_rollout(&store, &rollout_id, &project().repo_key).unwrap();
    let content_digest = audited.manifest.attempts[0].request.messages[0]
        .content
        .sha256
        .clone();
    std::fs::write(store.root().join("blobs").join(content_digest), b"tampered").unwrap();
    assert!(
        audit_store_rollout(&store, &rollout_id, &project().repo_key).is_err(),
        "audited load must reject a content blob changed after sealing"
    );
    let rejected_corpus = export_store_rollout_corpus_v2(&store, &project()).unwrap();
    assert_eq!(rejected_corpus["audit"]["scanned"], 1);
    assert_eq!(rejected_corpus["audit"]["audited"], 0);
    assert_eq!(rejected_corpus["audit"]["rejected"], 1);
    assert_eq!(
        rejected_corpus["audit"]["rejected_by_reason"]["corrupt_or_invalid"],
        1
    );
    assert!(rejected_corpus["records"].as_array().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn corpus_discovery_rejects_unsealed_and_foreign_records_without_hiding_valid_rows() {
    let root = scratch("corpus-rejections");
    let store = RolloutStore::new(root.clone());

    let mut valid =
        RolloutRecorder::start_for_test(store.clone(), CaptureMode::Local, project()).unwrap();
    let token = valid
        .begin_policy_attempt(&[ChatMsg::user("answer")], &defs(), route)
        .unwrap();
    valid
        .complete_policy_attempt(token, &ClubReply::Text("done".to_string()), route)
        .unwrap();
    valid
        .attach_reward(
            RewardReceipt::new(
                RewardOwner::CodingEval,
                1.0,
                crate::cut::sha256_hex(b"valid corpus evidence"),
            )
            .unwrap(),
        )
        .unwrap();
    valid
        .attach_compatibility_snapshot(|| {
            (
                "scripted".to_string(),
                serde_json::json!({}),
                serde_json::json!({}),
            )
        })
        .unwrap();
    valid
        .finish_turn("answer", false, false, false, Some("done"))
        .unwrap();

    let mut foreign_project = project();
    foreign_project.repo_key = "foreign-project".to_string();
    let mut foreign =
        RolloutRecorder::start_for_test(store.clone(), CaptureMode::Local, foreign_project)
            .unwrap();
    let token = foreign
        .begin_policy_attempt(&[ChatMsg::user("answer")], &defs(), route)
        .unwrap();
    foreign
        .complete_policy_attempt(token, &ClubReply::Text("done".to_string()), route)
        .unwrap();
    foreign
        .finish_turn("answer", false, false, false, Some("done"))
        .unwrap();

    let unsealed =
        RolloutRecorder::start_for_test(store.clone(), CaptureMode::Shadow, project()).unwrap();
    drop(unsealed);

    let first = export_store_rollout_corpus_v2(&store, &project()).unwrap();
    let second = export_store_rollout_corpus_v2(&store, &project()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first["audit"]["scanned"], 3);
    assert_eq!(first["audit"]["audited"], 1);
    assert_eq!(first["audit"]["rejected"], 2);
    assert_eq!(first["audit"]["exported"], 1);
    assert_eq!(first["audit"]["rejected_by_reason"]["foreign_project"], 1);
    assert_eq!(
        first["audit"]["rejected_by_reason"]["unsealed_or_missing"],
        1
    );
    assert_eq!(first["records"].as_array().unwrap().len(), 1);

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn local_blob_store_rejects_symlinked_digest_targets() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let root = scratch("blob-symlink");
    let store = RolloutStore::new(root.clone());
    let blobs = root.join("blobs");
    std::fs::create_dir_all(&blobs).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::set_permissions(&blobs, std::fs::Permissions::from_mode(0o700)).unwrap();
    let body = b"project body";
    let digest = crate::cut::sha256_hex(body);
    let target = root.join("attacker-target");
    std::fs::write(&target, body).unwrap();
    symlink(&target, blobs.join(digest)).unwrap();
    assert!(store.put_blob(body).unwrap_err().contains("symlinked"));
    let _ = std::fs::remove_dir_all(root);
}

fn task_reward_ownership_probe(task: bool, provider_error: bool) {
    use crate::harness::{eval_owns_label, run_task_turn_observed, run_turn_observed};
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex, mpsc};
    let _guard = crate::tests::env_lock();
    let root = scratch("task-label-workspace");
    let outer = scratch("task-label-outer");
    let probe = scratch("task-label-probe");
    std::fs::create_dir_all(&root).unwrap();
    let _mode = crate::tests::TestEnvGuard::set("ANGEL_HARNESS_ROLLOUTS", "local");
    let _dir =
        crate::tests::TestEnvGuard::set("ANGEL_HARNESS_ROLLOUT_DIR", outer.to_str().unwrap());
    let _required = crate::tests::TestEnvGuard::set("ANGEL_HARNESS_ROLLOUT_REQUIRED", "1");
    let _retries = crate::tests::TestEnvGuard::set("ANGEL_PROVIDER_RETRIES", "0");
    let _hint = crate::tests::TestEnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = crate::tests::TestEnvGuard::set("ANGEL_ADVISOR", "0");
    let _mcp = crate::tests::TestEnvGuard::set("ANGEL_MCP_CONFIG", "/nonexistent/owned-mcp.json");
    assert!(
        !eval_owns_label(),
        "fixture starts outside an evaluator scope"
    );
    type RewardObservation = (bool, TrainingEligibility, Option<RewardOwner>);
    struct Probe {
        root: PathBuf,
        seen: Arc<Mutex<Vec<RewardObservation>>>,
        provider_error: bool,
    }
    impl crate::club::Club for Probe {
        fn label(&self) -> &str {
            "task-external-reward-probe"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            let external = crate::harness::eval_owns_label();
            // cfg(test) disables post-write auto-verification. Exercise its exact
            // recorder attachment seam in the real task thread's ownership scope.
            let store = RolloutStore::new(self.root.clone());
            let mut recorder =
                RolloutRecorder::start_for_test(store.clone(), CaptureMode::Local, project())
                    .unwrap();
            let id = recorder.rollout_id().unwrap().to_string();
            let token = recorder
                .begin_policy_attempt(&[ChatMsg::user("owned verifier evidence")], &defs(), route)
                .unwrap();
            recorder
                .complete_policy_attempt(token, &ClubReply::Text("observed".into()), route)
                .unwrap();
            recorder.observe_cut_machine(&serde_json::json!({"cmd":"owned verifier", "dir":"/owned", "source":"fixture", "exit":0, "timed_out":false, "dur_ms":1}));
            recorder.attach_cut_reward(Some(1.0), !external).unwrap();
            recorder
                .attach_compatibility_snapshot(|| {
                    (
                        "task-external-reward-probe".into(),
                        serde_json::json!({"fingerprint":"owned-recorder-probe"}),
                        serde_json::json!({"lane":"fixture"}),
                    )
                })
                .unwrap();
            recorder
                .finish_turn("answer", false, false, false, Some("observed"))
                .unwrap();
            let audit = audit_store_rollout(&store, &id, &project().repo_key).unwrap();
            self.seen.lock().unwrap().push((
                external,
                audit.manifest.eligibility,
                audit.manifest.reward.map(|r| r.owner),
            ));
            if self.provider_error {
                Err("owned provider unavailable".into())
            } else {
                Ok(ClubReply::Text("Observed requested evidence.".into()))
            }
        }
    }
    let seen = Arc::new(Mutex::new(Vec::new()));
    let club = Probe {
        root: probe.clone(),
        seen: Arc::clone(&seen),
        provider_error,
    };
    let registry = crate::harness::ToolRegistry::with_team(root.clone(), Vec::new());
    let mut history = vec![ChatMsg::user(
        "Complete this bounded external evaluator task.",
    )];
    let binding = TaskRolloutBindingV1::new(
        Some("owned-task".into()),
        Some("owned-run".into()),
        crate::cut::sha256_hex(b"prompt"),
        crate::cut::sha256_hex(b"runtime"),
        "fixture".into(),
        crate::cut::sha256_hex(b"source"),
    );
    let (tx, _rx) = mpsc::channel::<crate::harness::TurnEvent>();
    let result = if task {
        run_task_turn_observed(
            &club,
            &registry,
            &mut history,
            &AtomicBool::new(false),
            Some(3),
            &tx,
            &binding,
            Some("owned-task-driver"),
        )
    } else {
        run_turn_observed(
            &club,
            &registry,
            &mut history,
            &AtomicBool::new(false),
            Some(3),
            &tx,
        )
    };
    assert!(
        !eval_owns_label(),
        "task success/error must restore caller ownership"
    );
    assert_eq!(result.is_err(), provider_error);
    let seen = seen.lock().unwrap();
    assert!(
        !seen.is_empty(),
        "actual task must reach the scripted provider"
    );
    for (external, eligibility, owner) in seen.iter() {
        assert_eq!(
            *external, task,
            "headless external-only contract must govern the actual task thread"
        );
        if task {
            assert_eq!(*owner, None);
            assert_eq!(
                *eligibility,
                TrainingEligibility::Excluded {
                    reason: ExclusionReason::MissingReward,
                    detail_sha256: None
                }
            );
        } else {
            assert_eq!(*owner, Some(RewardOwner::Cut));
            assert_eq!(*eligibility, TrainingEligibility::Eligible);
        }
    }
    if task && !provider_error {
        let id = result.unwrap().rollout_id.unwrap();
        let outer_audit = crate::harness::rollout::audit_workspace_rollout(&root, &id).unwrap();
        assert_eq!(
            outer_audit.manifest.requested_route.driver, "owned-task-driver",
            "task capture must retain the caller's selector, not the club display label"
        );
        assert!(outer_audit.manifest.reward.is_none());
        assert_eq!(
            outer_audit.manifest.eligibility,
            TrainingEligibility::Excluded {
                reason: ExclusionReason::MissingReward,
                detail_sha256: None
            }
        );
    }
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(outer);
    let _ = std::fs::remove_dir_all(probe);
}
#[test]
fn headless_task_preserves_external_reward_ownership_at_recorder_seam() {
    task_reward_ownership_probe(true, false);
}
#[test]
fn ordinary_turn_preserves_native_cut_reward_ownership() {
    task_reward_ownership_probe(false, false);
}
#[test]
fn headless_task_external_reward_scope_restores_after_provider_error() {
    task_reward_ownership_probe(true, true);
}
#[test]
fn nested_eval_label_scope_restores_the_callers_ownership() {
    use crate::harness::{EvalLabelScope, eval_owns_label};
    assert!(!eval_owns_label());
    let outer = EvalLabelScope::new();
    {
        let _inner = EvalLabelScope::new();
        assert!(eval_owns_label());
    }
    assert!(
        eval_owns_label(),
        "dropping inner scope must preserve external caller ownership"
    );
    drop(outer);
    assert!(!eval_owns_label());
}

#[test]
fn auxiliary_body_free_receipt_preserves_sealed_coverage_and_rejects_manifest_rewrite() {
    use crate::harness::auxiliary::AuxiliaryCoverage;
    for covered in [false, true] {
        let root = scratch("auxiliary-audit");
        let store = RolloutStore::new(root.clone());
        let mut recorder =
            RolloutRecorder::start_for_test(store.clone(), CaptureMode::Shadow, project()).unwrap();
        let id = recorder.rollout_id().unwrap().to_string();
        let token = recorder
            .begin_policy_attempt(&[ChatMsg::user("owned direct task")], &defs(), route)
            .unwrap();
        recorder
            .complete_policy_attempt(token, &ClubReply::Text("owned answer".into()), route)
            .unwrap();
        if covered {
            recorder
                .record_auxiliary_coverage(AuxiliaryCoverage::default())
                .unwrap();
        }
        recorder
            .finish_turn("answer", false, false, false, Some("owned answer"))
            .unwrap();
        let receipt = audit_store_rollout_receipt(&store, &id, &project().repo_key).unwrap();
        if covered {
            assert_eq!(receipt["auxiliary_coverage"]["complete"], true);
            assert_eq!(receipt["auxiliary_coverage"]["observed_tool_calls"], 0);
            let mut manifest = store.load_manifest(&id, &project().repo_key).unwrap();
            manifest
                .auxiliary_coverage
                .as_mut()
                .unwrap()
                .observed_tool_calls = 1;
            assert!(
                store
                    .save_manifest(&mut manifest)
                    .unwrap_err()
                    .contains("immutable")
            );
            assert_eq!(
                store
                    .load_manifest(&id, &project().repo_key)
                    .unwrap()
                    .auxiliary_coverage
                    .unwrap()
                    .observed_tool_calls,
                0,
                "the supported writer must preserve the sealed manifest"
            );
            // Owned tamper fixture: bypass the immutable writer and recompute
            // the manifest's own digest. Journal binding must still reject it.
            manifest.manifest_sha256 = None;
            manifest.manifest_sha256 = Some(crate::cut::sha256_hex(
                &serde_json::to_vec(&manifest).unwrap(),
            ));
            std::fs::write(
                store.run_dir(&id).unwrap().join("manifest.json"),
                serde_json::to_vec_pretty(&manifest).unwrap(),
            )
            .unwrap();
            assert_eq!(
                store
                    .load_manifest(&id, &project().repo_key)
                    .unwrap()
                    .auxiliary_coverage
                    .unwrap()
                    .observed_tool_calls,
                1,
                "tampered fixture has a valid self-digest and unchanged journal head"
            );
            assert!(
                audit_store_rollout_receipt(&store, &id, &project().repo_key)
                    .unwrap_err()
                    .contains("journal")
            );
        } else {
            assert!(
                receipt.get("auxiliary_coverage").is_none(),
                "historical absence cannot become complete"
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn auxiliary_failed_capability_receipt_stays_unlinked_and_duplicate_seal_rejects() {
    use crate::harness::auxiliary::AuxiliaryTracker;
    let tracker = AuxiliaryTracker::default();
    let scope = tracker.enter();
    tracker.tool_entered("spawn", &serde_json::json!({"task":"owned partial child"}));
    let root = scratch("auxiliary-unlinked");
    let store = RolloutStore::new(root.clone());
    let mut recorder =
        RolloutRecorder::start_for_test(store.clone(), CaptureMode::Shadow, project()).unwrap();
    let id = recorder.rollout_id().unwrap().to_string();
    let token = recorder
        .begin_policy_attempt(&[ChatMsg::user("owned task")], &defs(), route)
        .unwrap();
    recorder
        .complete_policy_attempt(
            token,
            &ClubReply::Text("retained partial work".into()),
            route,
        )
        .unwrap();
    recorder
        .record_auxiliary_coverage(scope.snapshot())
        .unwrap();
    assert!(
        recorder
            .record_auxiliary_coverage(scope.snapshot())
            .is_err()
    );
    recorder
        .finish_turn("answer", false, false, false, Some("retained partial work"))
        .unwrap();
    let receipt = audit_store_rollout_receipt(&store, &id, &project().repo_key).unwrap();
    assert_eq!(receipt["auxiliary_coverage"]["complete"], false);
    assert_eq!(receipt["auxiliary_coverage"]["sources"]["model_tool"], 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn coding_eval_reward_requires_current_passed_typed_receipt_and_exports() {
    use crate::harness::{ExecutionOutcome, ToolOutcome, VerificationOutcome};
    for (name, verification, mutation, bound) in [
        ("passed", VerificationOutcome::Passed, false, true),
        ("failed", VerificationOutcome::Failed, false, true),
        (
            "inconclusive",
            VerificationOutcome::Inconclusive,
            false,
            true,
        ),
        ("mutated", VerificationOutcome::Passed, true, true),
        ("unbound", VerificationOutcome::Passed, false, false),
    ] {
        let root = scratch(name);
        let store = RolloutStore::new(root.clone());
        let binding = TaskRolloutBindingV1::new(
            Some("task".into()),
            Some("run".into()),
            crate::cut::sha256_hex(b"prompt"),
            crate::cut::sha256_hex(b"runtime"),
            "fixture".into(),
            crate::cut::sha256_hex(b"source"),
        );
        let mut recorder = RolloutRecorder::start_for_test_with_binding(
            store.clone(),
            CaptureMode::Local,
            project(),
            bound.then_some(binding.clone()),
        )
        .unwrap();
        let id = recorder.rollout_id().unwrap().to_string();
        let token = recorder
            .begin_policy_attempt(&[ChatMsg::user("verify")], &defs(), route)
            .unwrap();
        recorder
            .complete_policy_attempt(token, &ClubReply::Text("done".into()), route)
            .unwrap();
        let call = ToolCall {
            id: "verify-1".into(),
            name: "run_tests".into(),
            args: serde_json::json!({}),
        };
        // A prior pass must not survive a later failed/inconclusive verifier.
        recorder.observe_task_verifier(
            &call,
            ToolOutcome {
                execution: ExecutionOutcome::Succeeded,
                verification: VerificationOutcome::Passed,
            },
            "prior receipt",
            false,
        );
        recorder.observe_task_verifier(
            &call,
            ToolOutcome {
                execution: ExecutionOutcome::Succeeded,
                verification,
            },
            "harness receipt bytes",
            false,
        );
        if mutation {
            recorder.observe_task_verifier(
                &ToolCall {
                    id: "edit".into(),
                    name: "apply_patch".into(),
                    args: serde_json::json!({}),
                },
                ToolOutcome {
                    execution: ExecutionOutcome::Succeeded,
                    verification: VerificationOutcome::NotApplicable,
                },
                "edit receipt",
                true,
            );
        }
        recorder.attach_task_reward().unwrap();
        recorder
            .attach_compatibility_snapshot(|| {
                (
                    "scripted".into(),
                    serde_json::json!({"fingerprint":"root"}),
                    serde_json::json!({"lane":"fixture"}),
                )
            })
            .unwrap();
        recorder
            .finish_turn("answer", false, false, false, Some("done"))
            .unwrap();
        let audit = audit_store_rollout(&store, &id, &project().repo_key).unwrap();
        if name == "passed" {
            let reward = audit.manifest.reward.as_ref().unwrap();
            assert_eq!(reward.owner, RewardOwner::CodingEval);
            assert_eq!(reward.evidence_storage, RewardEvidenceStorage::RolloutBlob);
            let bytes = store
                .read_blob_by_digest(&reward.evaluator_evidence_sha256)
                .unwrap();
            assert_eq!(
                crate::cut::sha256_hex(&bytes),
                reward.evaluator_evidence_sha256
            );
            let evidence: CodingEvalEvidence = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                evidence.receipt_sha256,
                crate::cut::sha256_hex(b"harness receipt bytes")
            );
            assert_eq!(evidence.task_binding_sha256, binding.binding_sha256);
            assert_eq!(audit.manifest.eligibility, TrainingEligibility::Eligible);
            // Tampering with the task subject must not pass evidence validation.
            let other = TaskRolloutBindingV1::new(
                Some("other".into()),
                None,
                crate::cut::sha256_hex(b"prompt"),
                crate::cut::sha256_hex(b"runtime"),
                "fixture".into(),
                "unbound".into(),
            );
            assert!(evidence.validate(&other).is_err());
            let export = export_store_rollout_v2(&store, &id, &project().repo_key).unwrap();
            assert_eq!(export["reward_owner"], "coding_eval");
        } else {
            assert!(audit.manifest.reward.is_none(), "{name}");
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
