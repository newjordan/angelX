use super::portability::import_portable_state;
use super::portability_tests::{TestRoot, populate, populate_with_subject, reseal};

#[test]
fn portable_import_rejects_valid_prefix_with_torn_journal_tail() {
    let source = TestRoot::new("torn-journal-source");
    let target = TestRoot::new("torn-journal-target");
    let (bundle, _, _) = populate(&source);
    let mut value: serde_json::Value = serde_json::from_slice(&bundle).unwrap();
    let entry = value["entries"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|entry| entry["path"] == "action/actions.jsonl")
        .unwrap();
    let mut body: Vec<u8> = serde_json::from_value(entry["body"].clone()).unwrap();
    body.extend_from_slice(br#"{"schema":"torn""#);
    entry["content_sha256"] = crate::cut::sha256_hex(&body).into();
    entry["body"] = serde_json::to_value(body).unwrap();
    reseal(&mut value);
    assert!(import_portable_state(&serde_json::to_vec(&value).unwrap(), &target.0).is_err());
    assert!(!target.0.exists());
}

#[test]
fn portable_import_rejects_valid_but_unrelated_journal_dossier_composition() {
    let primary = TestRoot::new("primary-composition");
    let donor = TestRoot::new("donor-composition");
    let target = TestRoot::new("mixed-composition");
    let (primary_bundle, _, _) = populate(&primary);
    let (donor_bundle, _, _) = populate_with_subject(&donor, "unrelated-work");
    let mut primary_value: serde_json::Value = serde_json::from_slice(&primary_bundle).unwrap();
    let donor_value: serde_json::Value = serde_json::from_slice(&donor_bundle).unwrap();
    let donor_action = donor_value["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["path"] == "action/actions.jsonl")
        .unwrap()
        .clone();
    let target_action = primary_value["entries"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|entry| entry["path"] == "action/actions.jsonl")
        .unwrap();
    *target_action = donor_action;
    reseal(&mut primary_value);
    assert!(
        import_portable_state(&serde_json::to_vec(&primary_value).unwrap(), &target.0,).is_err()
    );
    assert!(!target.0.exists());
}
