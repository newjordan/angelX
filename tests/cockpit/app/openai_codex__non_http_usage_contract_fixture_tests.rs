use super::*;
use crate::agent::club::Club;
use serde_json::{Value, json};

fn rows() -> Vec<Value> {
    let documented = json!({"input_tokens":75,"output_tokens":1186,
        "input_tokens_details":{"cached_tokens":0},
        "output_tokens_details":{"reasoning_tokens":1024}});
    let cases = vec![
        (
            "responses-documented-public-shape",
            vec![vec![
                json!({"type":"response.completed","response":{"usage":documented}}),
            ]],
            json!([75, 1186, 1024]),
            1,
            json!({"included":1}),
        ),
        (
            "responses-explicit-zero",
            vec![vec![
                json!({"type":"response.completed","response":{"usage":{"input_tokens":0,"output_tokens":0,"input_tokens_details":{"cached_tokens":0,"cache_write_tokens":0},"output_tokens_details":{"reasoning_tokens":0}}}}),
            ]],
            json!([0, 0, 0]),
            1,
            json!({"included":1}),
        ),
        (
            "responses-incomplete-reasoning-only",
            vec![vec![
                json!({"type":"response.incomplete","response":{"incomplete_details":{"reason":"max_output_tokens"},"usage":{"output_tokens_details":{"reasoning_tokens":9}}}}),
            ]],
            json!([null, null, 9]),
            1,
            json!({"included":1}),
        ),
        (
            "responses-failed-cache-only",
            vec![vec![
                json!({"type":"response.failed","response":{"error":{"message":"owned failure"},"usage":{"input_tokens_details":{"cached_tokens":80}}}}),
            ]],
            json!([null, null, null]),
            1,
            json!({"included":1}),
        ),
        (
            "responses-missing-receipt",
            vec![vec![
                json!({"type":"response.completed","response":{"usage":null}}),
            ]],
            json!([null, null, null]),
            1,
            json!({}),
        ),
        (
            "responses-receiptless-then-success",
            vec![
                vec![],
                vec![json!({"type":"response.completed","response":{"usage":documented}})],
            ],
            json!([null, null, null]),
            2,
            json!({"included":1}),
        ),
        (
            "responses-cumulative-final-zero",
            vec![vec![
                json!({"type":"response.in_progress","response":{"usage":{"input_tokens":100,"output_tokens":10,"input_tokens_details":{"cached_tokens":40,"cache_write_tokens":0},"output_tokens_details":{"reasoning_tokens":4}}}}),
                json!({"type":"response.completed","response":{"usage":{"output_tokens":0,"output_tokens_details":{"reasoning_tokens":0}}}}),
                json!({"type":"response.completed","response":{"usage":null}}),
            ]],
            json!([100, 0, 0]),
            1,
            json!({"included":1}),
        ),
    ];
    cases.into_iter().map(|(name, frames, normalized, attempts, conventions)| {
        // Avoid the catalog-loading convenience constructor and all operator files.
        let club = CodexClub::new_with_reasoning(
            "owned-responses-fixture", "owned-responses-model",
            crate::agent::openai_codex::ChatGptAuth {
                access_token: "owned-unused-token".into(), refresh_token: "".into(),
                account_id: "owned-unused-account".into(),
                path: std::path::PathBuf::new(), disk_snapshot: None,
            }, None, vec![],
        );
        let before = club.usage_accounting();
        for turn in &frames {
            let mut attempt = Attempt::new(&club, br#"{"model":"owned-responses-fixture"}"#);
            for frame in turn {
                attempt.receive(&format!("data: {}", serde_json::to_string(frame).unwrap()), false);
            }
        }
        let report = crate::agent::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
        assert_eq!(report.attempts, attempts, "{name}");
        let usage = serde_json::to_value(report).unwrap();
        assert_eq!(usage["reasoning_convention_attempts"], conventions, "{name}");
        assert_eq!(usage["unknown_path_fields"], 0, "{name}");
        let expected_raw = match name {
            "responses-documented-public-shape" | "responses-receiptless-then-success" => json!([75,1186,1024]),
            "responses-explicit-zero" => json!([0,0,0]),
            "responses-incomplete-reasoning-only" => json!([null,null,9]),
            "responses-cumulative-final-zero" => json!([100,0,0]),
            _ => json!([null,null,null]),
        };
        assert_eq!(json!([usage["input"],usage["output"],usage["reasoning"]]), expected_raw, "{name}");
        assert_eq!(usage["core_complete"], matches!(name, "responses-documented-public-shape" | "responses-explicit-zero" | "responses-cumulative-final-zero"), "{name}");
        if name == "responses-failed-cache-only" { assert_eq!(usage["cache_read"], 80); }
        if name == "responses-documented-public-shape" {
            assert_eq!(usage["generation_output"], 1186, "reasoning must not be added twice");
            assert_eq!(usage["uncached_input"], Value::Null, "absent cache-write is not reported zero");
        }
        if name == "responses-cumulative-final-zero" {
            assert_eq!(usage["input"], 100);
            assert_eq!(usage["output"], 0);
            assert_eq!(usage["uncached_input"], 60);
            assert_eq!(usage["raw_field_reports"]["response.usage.output_tokens"], 1);
        }
        if name == "responses-receiptless-then-success" {
            assert_eq!(usage["input"], 75);
            assert_eq!(usage["output"], 1186);
            assert_eq!(usage["reported_attempts"]["input"], 1);
        }
        json!({"schema":"angel.non-http-usage-producer-fixture/v1","name":name,
            "origin":"synthetic-responses-sse","contract_boundary":"public Responses field semantics applied by current Codex OAuth backend compatibility adapter; not live backend evidence",
            "primary_contract":"https://developers.openai.com/api/docs/guides/reasoning",
            "producer_path":"Attempt::receive -> parse_responses_event -> parse_usage -> Attempt::drop -> task_usage_delta -> AccountingReport",
            "attempt_frames":frames,"expected_normalized":normalized,"usage":usage})
    }).collect()
}

#[test]
fn non_http_usage_contract_responses_parser_and_attempt_controls() {
    assert_eq!(rows().len(), 7);
}

#[test]
#[ignore = "exports synthetic Responses wire through actual native attempt owner; no provider calls"]
fn non_http_usage_contract_responses_export() {
    for row in rows() {
        println!(
            "ANGEL_NON_HTTP_USAGE_FIXTURE {}",
            serde_json::to_string(&row).unwrap()
        );
    }
}
