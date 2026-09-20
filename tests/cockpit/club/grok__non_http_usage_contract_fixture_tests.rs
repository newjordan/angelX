use super::*;
use serde_json::{Value, json};

fn rows() -> Vec<Value> {
    let _lock = env_lock();
    let _crash = EnvGuard::unset("ANGEL_FAKE_GROK_CRASH_ONCE");
    let _hang = EnvGuard::unset("ANGEL_FAKE_GROK_HANG");
    let mut rows = Vec::new();
    for name in [
        "normal",
        "zero",
        "missing",
        "null",
        "empty-text",
        "setup-failure",
        "transport-retry",
        "fresh-session-totals",
        "draft-top-level",
    ] {
        let (auth, script, spawns) = fake_grok_acp_fixture(&format!("non-http-usage-{name}"));
        let mut source = std::fs::read_to_string(&script).unwrap();
        let original =
            r#""_meta":{"usage":{"inputTokens":10,"outputTokens":2,"reasoningTokens":1}}"#;
        assert!(
            source.contains(original),
            "owned fixture has exact terminal usage seam"
        );
        let (replacement, terminal_usage, expected_normalized) = match name {
            "zero" => (
                r#""_meta":{"usage":{"inputTokens":0,"outputTokens":0,"reasoningTokens":0}}"#,
                json!({"_meta":{"usage":{"inputTokens":0,"outputTokens":0,"reasoningTokens":0}}}),
                json!([null, null, 0]),
            ),
            "missing" => (
                r#""_meta":{}"#,
                json!({"_meta":{}}),
                json!([null, null, null]),
            ),
            "null" => (
                r#""_meta":{"usage":null}"#,
                json!({"_meta":{"usage":null}}),
                json!([null, null, null]),
            ),
            "draft-top-level" => (
                r#""usage":{"inputTokens":10,"outputTokens":2,"thoughtTokens":1}"#,
                json!({"usage":{"inputTokens":10,"outputTokens":2,"thoughtTokens":1}}),
                json!([null, null, null]),
            ),
            "setup-failure" | "transport-retry" => (
                original,
                json!({"_meta":{"usage":{"inputTokens":10,"outputTokens":2,"reasoningTokens":1}}}),
                json!([null, null, null]),
            ),
            "fresh-session-totals" => (
                original,
                json!({"_meta":{"usage":{"inputTokens":10,"outputTokens":2,"reasoningTokens":1}}}),
                json!([null, null, 2]),
            ),
            _ => (
                original,
                json!({"_meta":{"usage":{"inputTokens":10,"outputTokens":2,"reasoningTokens":1}}}),
                json!([null, null, 1]),
            ),
        };
        source = source.replace(original, replacement);
        source = source.replace(
            "    *'\"method\":\"session/prompt\"'*)",
            "    *'\"method\":\"session/prompt\"'*)\n      printf 'prompt\\n' >> \"${ANGEL_FAKE_GROK_SPAWNS}\"",
        );
        if name == "setup-failure" {
            source = source.replace("      session_no=$((session_no + 1))", "      printf '{\"jsonrpc\":\"2.0\",\"id\":%s,\"result\":{}}\\n' \"$id\"\n      continue");
        } else if name == "empty-text" {
            source = source
                .lines()
                .filter(|line| !line.contains("\"sessionUpdate\""))
                .collect::<Vec<_>>()
                .join("\n");
            source.push('\n');
        } else if name == "transport-retry" {
            // The existing spawn count log now includes prompt entries. Crash
            // exactly the first owned process before it returns a usage receipt.
            source = source.replace("[ \"${ANGEL_FAKE_GROK_CRASH_ONCE:-0}\" = 1 ]", "[ 1 = 1 ]");
        }
        std::fs::write(&script, source).unwrap();
        let _auth = EnvGuard::set("ANGEL_GROK_OAUTH_FILE", auth.to_str().unwrap());
        let _cmd = EnvGuard::set("ANGEL_GROK_CMD", script.to_str().unwrap());
        let _spawn_log = EnvGuard::set("ANGEL_FAKE_GROK_SPAWNS", spawns.to_str().unwrap());
        let _enabled = EnvGuard::set("ANGEL_GROK_RESEARCH", "1");
        let club = GrokResearchClub::from_env().expect("owned ACP fixture");
        let before = club.usage_accounting();
        let result = club.respond("owned non-HTTP usage fixture");
        if name == "setup-failure" {
            assert!(result.unwrap_err().contains("session/new"));
        } else if name == "empty-text" {
            assert!(result.unwrap_err().contains("no assistant text"));
        } else {
            assert!(result.unwrap().starts_with("fake-"));
        }
        if name == "fresh-session-totals" {
            assert_eq!(
                club.respond("second owned fresh session").unwrap(),
                "fake-2"
            );
        }
        let report = crate::agent::harness::task_usage_delta(before, club.usage_accounting());
        let dispatches = std::fs::read_to_string(&spawns)
            .unwrap()
            .lines()
            .filter(|line| *line == "prompt")
            .count();
        let expected_attempts = match name {
            "setup-failure" => 0,
            "transport-retry" | "fresh-session-totals" => 2,
            _ => 1,
        };
        assert_eq!(dispatches, expected_attempts, "{name}");
        if name == "setup-failure" {
            assert!(
                report.is_none(),
                "zero prompt dispatch has no native task usage envelope"
            );
            drop(club);
            std::fs::remove_dir_all(auth.parent().unwrap()).unwrap();
            continue; // This setup control produces no accounting receipt to export.
        }
        let report = report.unwrap();
        assert_eq!(report.attempts, expected_attempts as u64, "{name}");
        let usage = serde_json::to_value(report).unwrap();
        let observations = match name {
            "setup-failure" | "missing" | "null" | "draft-top-level" => 0,
            "fresh-session-totals" => 2,
            _ => 1,
        };
        let expected_coverage = if observations == 0 {
            json!({})
        } else {
            json!({"unknown":observations})
        };
        assert_eq!(
            usage["reasoning_convention_attempts"], expected_coverage,
            "{name}"
        );
        assert_eq!(
            usage["cache_convention_attempts"], expected_coverage,
            "{name}"
        );
        assert_eq!(usage["total_prompt"], Value::Null, "{name}");
        assert_eq!(usage["generation_output"], Value::Null, "{name}");
        assert_eq!(usage["cache_read"], Value::Null, "{name}");
        assert_eq!(usage["unknown_path_fields"], 0, "{name}");
        let raw = if observations == 0 {
            json!([null, null, null])
        } else if name == "zero" {
            json!([0, 0, 0])
        } else {
            json!([10 * observations, 2 * observations, observations])
        };
        assert_eq!(
            json!([usage["input"], usage["output"], usage["reasoning"]]),
            raw,
            "{name}"
        );
        if observations > 0 {
            assert_eq!(
                usage["raw_field_reports"]["_meta.usage.reasoningTokens"],
                observations
            );
        }
        rows.push(json!({"schema":"angel.non-http-usage-producer-fixture/v1","name":format!("grok-acp-{name}"),
            "origin":"synthetic-owned-acp-process","contract_boundary":"Grok ACP v1 _meta.usage extension; inclusion and internal model-request scope unknown",
            "primary_contract":"https://agentclientprotocol.com/rfds/end-turn-token-usage",
            "producer_path":"owned JSON-RPC subprocess -> GrokAcpConnection::prompt -> parse_grok_acp_usage -> AccountingAttempt -> task_usage_delta -> AccountingReport",
            "synthetic_terminal_usage":terminal_usage,"prompt_dispatches":dispatches,"expected_normalized":expected_normalized,"usage":usage}));
        drop(club); // Settle only the retained owned process before removing its fixture.
        std::fs::remove_dir_all(auth.parent().unwrap()).unwrap();
    }
    rows
}

#[test]
fn non_http_usage_contract_grok_actual_process_controls() {
    assert_eq!(rows().len(), 8);
}

#[test]
#[ignore = "exports actual owned ACP process parser rows; no provider or model calls"]
fn non_http_usage_contract_grok_export() {
    for row in rows() {
        println!(
            "ANGEL_NON_HTTP_USAGE_FIXTURE {}",
            serde_json::to_string(&row).unwrap()
        );
    }
}
