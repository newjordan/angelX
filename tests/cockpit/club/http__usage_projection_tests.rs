use super::*;
#[test]
fn usage_projection_http_commits_partial_stream_paths_and_cache_writes() {
    let club = HttpClub::new("fixture", "https://api.openai.com/v1", "fixture", None);
    let before = club.usage_accounting();
    {
        let mut commit = StreamUsageCommit::new(&club);
        commit.observe(&serde_json::json!({"usage":{"prompt_tokens":100,"completion_tokens":1,"prompt_tokens_details":{"cached_tokens":20,"cache_write_tokens":10}}}));
        commit.observe(&serde_json::json!({"usage":{"completion_tokens":5}}));
        commit.observe(&serde_json::json!({"usage":null}));
    }
    let report = crate::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
    assert_eq!(
        (report.input, report.output, report.reasoning),
        (Some(100), Some(5), None)
    );
    assert_eq!(
        (
            report.uncached_input,
            report.total_prompt,
            report.generation_output
        ),
        (Some(70), Some(100), Some(5))
    );
    assert_eq!(report.raw_field_reports["prompt_tokens"], 1);
    assert_eq!(
        report.raw_field_reports["prompt_tokens_details.cache_write_tokens"],
        1
    );
}

#[test]
fn usage_projection_http_zero_retry_unknown_and_cache_only_survive() {
    let club = HttpClub::new("fixture", "https://openrouter.ai/api/v1", "fixture", None);
    let before = club.usage_accounting();
    {
        let _missing = StreamUsageCommit::new(&club);
    }
    {
        let mut commit = StreamUsageCommit::new(&club);
        commit.observe(&serde_json::json!({"usage":{"prompt_tokens":0,"completion_tokens":0}}));
    }
    let report = crate::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
    assert_eq!(
        (
            report.attempts,
            report.input,
            report.output,
            report.reasoning
        ),
        (2, Some(0), Some(0), None)
    );
    assert_eq!(report.reported_attempts.input, 1);
    assert!(!report.core_complete);
    let before = club.usage_accounting();
    {
        let mut commit = StreamUsageCommit::new(&club);
        commit
            .observe(&serde_json::json!({"usage":{"prompt_tokens_details":{"cached_tokens":80}}}));
    }
    let report = crate::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
    assert_eq!(
        (report.input, report.output, report.cache_read),
        (None, None, Some(80))
    );
    assert_eq!(report.cache_hit_pct, None);
}

#[test]
fn usage_projection_http_custom_endpoint_never_guesses_contract() {
    let club = HttpClub::new(
        "openai-looking-label",
        "http://127.0.0.1:9/v1",
        "fixture",
        None,
    );
    let before = club.usage_accounting();
    {
        let mut commit = StreamUsageCommit::new(&club);
        commit.observe(&serde_json::json!({"usage":{"input_tokens":100,"output_tokens":5,"cache_read_input_tokens":20,"cache_creation_input_tokens":4}}));
    }
    let report = crate::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
    assert_eq!(
        (report.input, report.cache_read, report.cache_write),
        (Some(100), Some(20), Some(4))
    );
    assert_eq!(
        (
            report.uncached_input,
            report.total_prompt,
            report.cache_hit_pct
        ),
        (None, None, None)
    );
    assert_eq!(report.cache_convention_attempts["unknown"], 1);
}

#[test]
fn usage_projection_http_fallback_alias_keeps_exact_numeric_provenance() {
    let club = HttpClub::new("fixture", "https://api.deepseek.com/v1", "fixture", None);
    let before = club.usage_accounting();
    {
        let mut commit = StreamUsageCommit::new(&club);
        commit.observe(&serde_json::json!({"usage":{"prompt_tokens":null,"input_tokens":100,"completion_tokens":5,"prompt_tokens_details":{"cached_tokens":null},"prompt_cache_hit_tokens":75}}));
    }
    let report = crate::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
    assert_eq!(report.raw_field_reports.get("input_tokens"), Some(&1));
    assert_eq!(
        report.raw_field_reports.get("prompt_cache_hit_tokens"),
        Some(&1)
    );
    assert!(!report.raw_field_reports.contains_key("prompt_tokens"));
    assert!(
        !report
            .raw_field_reports
            .contains_key("prompt_tokens_details.cached_tokens")
    );
}
#[test]
fn usage_projection_http_actual_retry_and_decode_failure_close_attempts() {
    let _lock = crate::tests::env_lock();
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        for (status, body) in [
            (
                "500 Internal Server Error",
                r#"{"usage":{"prompt_tokens":7},"error":{"message":"fixture retry"}}"#,
            ),
            (
                "200 OK",
                r#"{"usage":{"completion_tokens":3},"choices":[]}"#,
            ),
            ("200 OK", "invalid json fixture"),
        ] {
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(error) => panic!("bounded fixture accept: {error}"),
                }
            };
            socket
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let mut request = Vec::new();
            loop {
                let mut chunk = [0u8; 1024];
                let size = socket.read(&mut chunk).unwrap();
                assert!(size > 0 && request.len() + size <= 65536);
                request.extend_from_slice(&chunk[..size]);
                if let Some(end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
                    let length = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length:"))
                        .unwrap()
                        .trim()
                        .parse::<usize>()
                        .unwrap();
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).unwrap();
        }
    });
    let mut club = HttpClub::new("fixture", format!("http://{address}"), "fixture", None);
    club.agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(2))
        .build();
    club.policy.retries = 1;
    club.policy.backoff_base = Duration::ZERO;
    club.policy.backoff_cap = Duration::ZERO;
    let before = club.usage_accounting();
    assert!(
        club.post_chat(serde_json::json!({"model":"fixture","messages":[]}))
            .is_ok()
    );
    assert!(
        club.post_chat(serde_json::json!({"model":"fixture","messages":[]}))
            .unwrap_err()
            .contains("decode response")
    );
    server.join().unwrap();
    let report = crate::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
    assert_eq!(report.attempts, 3);
    assert_eq!((report.input, report.output), (Some(7), Some(3)));
    assert_eq!(
        (
            report.reported_attempts.input,
            report.reported_attempts.output
        ),
        (1, 1)
    );
    assert_eq!(report.raw_field_reports["prompt_tokens"], 1);
    assert!(!report.core_complete);
}
