use super::*;
use serde_json::{Value, json};

fn rows() -> Vec<Value> {
    let manifest: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/usage/supplemental-chat-usage-wire-v1.json"
    )))
    .unwrap();
    assert_eq!(manifest["schema"], "angel.supplemental-chat-usage-wire/v1");
    manifest["cases"].as_array().unwrap().iter().map(|case| {
        let club = HttpClub::new("owned-supplemental-wire", case["logical_endpoint"].as_str().unwrap(), "owned-no-model-call", None);
        let before = club.usage_accounting();
        {
            let mut attempt = StreamUsageCommit::new(&club);
            attempt.observe(&json!({"usage":case["usage"]}));
        }
        let usage = serde_json::to_value(crate::harness::task_usage_delta(before, club.usage_accounting()).unwrap()).unwrap();
        assert_eq!(json!([usage["input"],usage["output"],usage["reasoning"]]), case["expected_raw"]);
        assert_eq!(json!([usage["total_prompt"],usage["generation_output"],usage["reasoning"]]), case["expected_normalized"]);
        assert_eq!(usage["uncached_input"], case["expected_uncached"]);
        assert_eq!(usage["reasoning_convention_attempts"], json!({"included":1}));
        assert_eq!(usage["cache_convention_attempts"], json!({"included":1}));
        assert_eq!(usage["attempts"], 1);
        assert_eq!(usage["core_complete"], true);
        assert_eq!(usage["unknown_path_fields"], 0);
        assert_eq!(usage["raw_field_reports"]["completion_tokens_details.reasoning_tokens"], 1);
        if case["name"] == "deepseek-chat-included" {
            assert_eq!(usage["raw_field_reports"]["prompt_cache_hit_tokens"], 1);
            assert_eq!(usage["cache_write"], Value::Null, "miss tokens are not cache-write tokens");
        } else {
            assert_eq!(usage["cache_write"], 0);
        }
        json!({"schema":"angel.supplemental-chat-usage-producer/v1","name":case["name"],
            "origin":manifest["origin"],"logical_endpoint":case["logical_endpoint"],
            "contract":case["contract"],"primary_source":case["primary_source"],
            "original_wire_usage":case["usage"],"expected_normalized":case["expected_normalized"],
            "producer_path":"StreamUsageCommit -> parse_http_usage -> AccountingAttempt -> task_usage_delta -> AccountingReport",
            "usage":usage})
    }).collect()
}

#[test]
#[ignore = "exports two actual native parser rows from shared synthetic OpenAI/DeepSeek wire; no requests"]
fn supplemental_chat_usage_fixture_export() {
    let rows = rows();
    assert_eq!(rows.len(), 2);
    for row in rows {
        println!(
            "ANGEL_SUPPLEMENTAL_CHAT_USAGE_FIXTURE {}",
            serde_json::to_string(&row).unwrap()
        );
    }
}
