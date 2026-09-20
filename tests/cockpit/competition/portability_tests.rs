use super::dossier::{DOSSIER_SCHEMA_V1, DossierV1};
use super::dossier_store::DossierStoreV1;
use super::journal::{ActionUpdateV1, canonical_action_key};
use super::leases::{LeaseError, LeasePhaseV1, WorkLeaseKeyV1};
use super::portability::{PortableEntryV1, export_portable_state, import_portable_state};
use super::recovery::recover_store;
use super::schema::{ActionIntentV1, ActionKindV1, ActionPhaseV1, CompetitionKeyV1, LaneIdV1};
use super::store::CompetitionStore;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) struct TestRoot(pub(super) PathBuf);

impl TestRoot {
    pub(super) fn new(tag: &str) -> Self {
        Self(std::env::temp_dir().join(format!(
            "angel-portable-{tag}-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        )))
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn lease_key() -> WorkLeaseKeyV1 {
    WorkLeaseKeyV1 {
        campaign_id: "campaign-portable".into(),
        lane_id: LaneIdV1::FrontierGuard,
        work_item_id: "portable-work".into(),
    }
}

fn intent(subject: &str) -> ActionIntentV1 {
    let mut intent = ActionIntentV1 {
        action_key: String::new(),
        campaign_id: "campaign-portable".into(),
        competition: CompetitionKeyV1 {
            platform_id: "fixture".into(),
            competition_id: "contest".into(),
            field_id: "kernels".into(),
            benchmark_id: "bench".into(),
            profile_id: "profile".into(),
            hardware_id: "gpu".into(),
        },
        kind: ActionKindV1::DispatchWorker,
        subject_id: subject.into(),
        payload_sha256: crate::knowledge::cut::sha256_hex(
            format!("portable-payload:{subject}").as_bytes(),
        ),
        intent_version: "v1".into(),
    };
    intent.action_key = canonical_action_key(&intent).unwrap();
    intent
}

pub(super) fn populate(root: &TestRoot) -> (Vec<u8>, WorkLeaseKeyV1, u64) {
    populate_with_subject(root, "portable-work")
}

pub(super) fn populate_with_subject(
    root: &TestRoot,
    subject: &str,
) -> (Vec<u8>, WorkLeaseKeyV1, u64) {
    let action_store = CompetitionStore::new(root.0.join("action"));
    let key = lease_key();
    let lease = action_store
        .update_leases(|leases| leases.grant(key.clone(), "portable-worker".into(), 10, 500_000, 7))
        .unwrap();
    action_store
        .append_action_fenced(
            &key,
            lease.fencing_generation,
            ActionUpdateV1 {
                intent: intent(subject),
                attempt: 0,
                phase: ActionPhaseV1::Planned,
                retryable: false,
                at_ms: 11,
                reconcile_key: None,
                receipt_sha256: None,
                next: None,
            },
        )
        .unwrap();
    let journal_head = action_store.recover().unwrap().state.head_sha256;
    let mut dossier = DossierV1::new(
        "dossier-portable".into(),
        "project-portable".into(),
        "repository-portable".into(),
        "revision-one".into(),
        1,
        journal_head.clone(),
    )
    .unwrap();
    dossier
        .rollover_fresh_turn(
            "turn-portable".into(),
            journal_head,
            "portable continuation".into(),
        )
        .unwrap();
    DossierStoreV1::new(root.0.join("dossier"))
        .sync(&dossier)
        .unwrap();
    (
        export_portable_state(&root.0, "campaign-portable").unwrap(),
        key,
        lease.fencing_generation,
    )
}

#[test]
fn cross_rig_export_import_is_path_independent_ac15() {
    let source = TestRoot::new("source");
    let left = TestRoot::new("left-rig");
    let right = TestRoot::new("right-rig");
    let (bundle, key, old_generation) = populate(&source);
    let encoded = String::from_utf8(bundle.clone()).unwrap();
    assert!(!encoded.contains(source.0.to_str().unwrap()));
    let left_receipt = import_portable_state(&bundle, &left.0).unwrap();
    let right_receipt = import_portable_state(&bundle, &right.0).unwrap();
    assert_eq!(left_receipt, right_receipt);

    let left_journal = recover_store(&CompetitionStore::new(left.0.join("action"))).unwrap();
    let right_journal = recover_store(&CompetitionStore::new(right.0.join("action"))).unwrap();
    assert_eq!(left_journal.journal, right_journal.journal);
    let left_dossier = DossierStoreV1::new(left.0.join("dossier")).load().unwrap();
    let right_dossier = DossierStoreV1::new(right.0.join("dossier")).load().unwrap();
    assert_eq!(left_dossier, right_dossier);
    let left_leases = CompetitionStore::new(left.0.join("action"))
        .recover_leases()
        .unwrap();
    let right_leases = CompetitionStore::new(right.0.join("action"))
        .recover_leases()
        .unwrap();
    assert_eq!(left_leases, right_leases);
    let imported = left_leases.current(&key).unwrap();
    assert_eq!(imported.fencing_generation, old_generation + 1);
    assert_eq!(imported.phase, LeasePhaseV1::Fenced);
    assert_eq!(imported.checkpoint_revision, 7);
    assert_eq!(
        left_leases.accept_landing(&key, old_generation),
        Err(LeaseError::StaleGeneration)
    );
}

#[test]
fn clean_rig_embedded_assets_without_checkout_ac15() {
    let source = TestRoot::new("detached-source");
    let target = TestRoot::new("clean-rig");
    let (bundle, _, _) = populate(&source);
    std::fs::remove_dir_all(&source.0).unwrap();
    let receipt = import_portable_state(&bundle, &target.0).unwrap();
    assert_eq!(receipt.campaign_id, "campaign-portable");
    let dossier = DossierStoreV1::new(target.0.join("dossier"))
        .load()
        .unwrap()
        .unwrap();
    assert_eq!(dossier.schema, DOSSIER_SCHEMA_V1);
    assert_eq!(dossier.dossier_revision, receipt.dossier_revision);
    assert_eq!(
        recover_store(&CompetitionStore::new(target.0.join("action")))
            .unwrap()
            .journal
            .head_sha256,
        receipt.journal_head_sha256
    );
}

#[test]
fn portable_bundle_rejects_schema_digest_and_absolute_paths() {
    let source = TestRoot::new("tamper-source");
    let (bundle, _, _) = populate(&source);
    for (tag, mutate) in [
        (
            "schema",
            (|value: &mut serde_json::Value| value["schema"] = "future-schema".into())
                as fn(&mut serde_json::Value),
        ),
        (
            "body",
            (|value: &mut serde_json::Value| value["entries"][0]["body"][0] = 0.into())
                as fn(&mut serde_json::Value),
        ),
        (
            "path",
            (|value: &mut serde_json::Value| {
                value["entries"][0]["path"] = "C:/checkout/actions.jsonl".into()
            }) as fn(&mut serde_json::Value),
        ),
    ] {
        let mut value: serde_json::Value = serde_json::from_slice(&bundle).unwrap();
        mutate(&mut value);
        reseal(&mut value);
        let target = TestRoot::new(tag);
        assert!(import_portable_state(&serde_json::to_vec(&value).unwrap(), &target.0).is_err());
        assert!(!target.0.exists());
    }
}

#[test]
fn portable_component_schemas_are_validated() {
    let source = TestRoot::new("component-schema-source");
    let (bundle, _, _) = populate(&source);
    for component in [
        "action/actions.jsonl",
        "action/leases.snapshot.json",
        "dossier/current.json",
    ] {
        let mut value: serde_json::Value = serde_json::from_slice(&bundle).unwrap();
        let entry = value["entries"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|entry| entry["path"] == component)
            .unwrap();
        let mut body: Vec<u8> = serde_json::from_value(entry["body"].clone()).unwrap();
        let had_newline = body.last() == Some(&b'\n');
        if had_newline {
            body.pop();
        }
        let mut component_value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        component_value["schema"] = "future-component-schema".into();
        body = serde_json::to_vec(&component_value).unwrap();
        if had_newline {
            body.push(b'\n');
        }
        entry["content_sha256"] = crate::knowledge::cut::sha256_hex(&body).into();
        entry["body"] = serde_json::to_value(body).unwrap();
        reseal(&mut value);
        let target = TestRoot::new("component-schema-target");
        assert!(import_portable_state(&serde_json::to_vec(&value).unwrap(), &target.0).is_err());
        assert!(!target.0.exists());
    }
}

#[cfg(unix)]
#[test]
fn portable_import_preserves_parent_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let source = TestRoot::new("permission-source");
    let parent = TestRoot::new("permission-parent");
    std::fs::create_dir(&parent.0).unwrap();
    std::fs::set_permissions(&parent.0, std::fs::Permissions::from_mode(0o755)).unwrap();
    let before = std::fs::metadata(&parent.0).unwrap().permissions().mode();
    let (bundle, _, _) = populate(&source);
    import_portable_state(&bundle, &parent.0.join("imported")).unwrap();
    let after = std::fs::metadata(&parent.0).unwrap().permissions().mode();
    assert_eq!(before, after);
}

pub(super) fn reseal(value: &mut serde_json::Value) {
    let entries: Vec<PortableEntryV1> = serde_json::from_value(value["entries"].clone()).unwrap();
    let body = serde_json::to_vec(&(
        value["schema"].as_str().unwrap(),
        value["contract"].as_str().unwrap(),
        value["campaign_id"].as_str().unwrap(),
        entries,
    ))
    .unwrap();
    value["bundle_sha256"] = crate::knowledge::cut::sha256_hex(&body).into();
}
