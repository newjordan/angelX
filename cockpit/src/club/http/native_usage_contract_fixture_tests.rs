//! Test-only bridge from original wire fields through the native provider parser
//! and attempt owner to unchanged AccountingReport values for real consumers.
use super::*;
use serde_json::{Value, json};

fn native_usage_contract_fixture_rows() -> Vec<Value> {
    let manifest: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../benchmarks/action-agent/fixtures/native-usage-wire-v1.json"
    )))
    .unwrap();
    assert_eq!(manifest["schema"], "angel.native-usage-wire-fixtures/v1");
    manifest["cases"].as_array().unwrap().iter().map(|case| {
        // This logical official endpoint selects its real accounting contract;
        // no HTTP request is made. Raw fixture frames enter the same native
        // attempt-owned parser used by real streaming responses.
        let club = HttpClub::new("glm-labelled-owned-fixture", case["logical_endpoint"].as_str().unwrap(), "glm-5.3-flash", None);
        let before = club.usage_accounting();
        for frames in case["attempt_frames"].as_array().unwrap() {
            let mut attempt = StreamUsageCommit::new(&club);
            for frame in frames.as_array().unwrap() { attempt.observe(frame); }
        }
        let usage = serde_json::to_value(crate::harness::task_usage_delta(before, club.usage_accounting()).unwrap()).unwrap();
        for (key, expected) in case["expected"].as_object().unwrap() {
            assert_eq!(&usage[key], expected, "{}: {key}", case["name"]);
        }
        assert_eq!(usage["unknown_path_fields"], 0, "wire aliases must survive parser export");
        let mut row = case.clone();
        row["usage"] = usage;
        row["producer_path"] = json!("StreamUsageCommit -> parse_http_usage -> accounting_observation -> task_usage_delta -> AccountingReport serializer");
        row
    }).collect()
}

#[test]
fn native_usage_contract_fixture_parser_path_matches_documented_and_unknown_cases() {
    let rows = native_usage_contract_fixture_rows();
    assert_eq!(rows.len(), 10);
    let included = &rows
        .iter()
        .find(|row| row["name"] == "openrouter-documented-write-shape")
        .unwrap()["usage"];
    assert_eq!(
        included["raw_field_reports"]["prompt_tokens_details.cache_write_tokens"],
        1
    );
    let observed = &rows
        .iter()
        .find(|row| row["name"] == "zai-observed-reasoning-field-shape")
        .unwrap()["usage"];
    assert_eq!(
        observed["raw_field_reports"]["completion_tokens_details.reasoning_tokens"],
        1
    );
    assert_eq!(observed["reported_attempts"]["generation_output"], 1);
}

#[test]
#[ignore = "exports original synthetic wire inputs and actual parser-produced rows for shared consumer qualification"]
fn native_usage_contract_fixture_export() {
    for row in native_usage_contract_fixture_rows() {
        println!(
            "ANGEL_NATIVE_USAGE_FIXTURE {}",
            serde_json::to_string(&row).unwrap()
        );
    }
    // An explicit existing campaign receipt can be added as a separate original
    // aggregate. Never reconstruct its 20 unretained provider responses.
    if let Some(path) = std::env::var_os("ANGEL_NATIVE_USAGE_RECORDED_ENVELOPE") {
        let path = std::path::PathBuf::from(path);
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(&path)
            .unwrap()
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .unwrap();
        assert!(bytes.len() <= 2 * 1024 * 1024, "bounded recorded envelope");
        let envelope: Value = serde_json::from_slice(&bytes).unwrap();
        let usage = envelope.get("usage").expect("recorded usage").clone();
        assert_eq!(usage["schema"], "angel.usage.v2");
        assert_eq!(usage["attempts"], 20, "exact reviewed campaign aggregate");
        assert_eq!(usage["input"], 264209);
        assert_eq!(usage["output"], 4959);
        assert_eq!(usage["reasoning"], 1733);
        assert_eq!(usage["generation_output"], Value::Null);
        let row = json!({"name":"recorded-glm-paid-aggregate", "origin":"recorded-native-task-aggregate",
            "source_path":path, "source_sha256":crate::cut::sha256_hex(&bytes),
            "producer_path":"original native task envelope; copied usage unchanged; not reparsed as one wire attempt",
            "expected_normalized":[264209, null, 1733], "usage":usage});
        println!(
            "ANGEL_NATIVE_USAGE_FIXTURE {}",
            serde_json::to_string(&row).unwrap()
        );
    }
}
