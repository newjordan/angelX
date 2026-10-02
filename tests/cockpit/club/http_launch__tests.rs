use super::*;
use std::io::{Read, Write};

fn server(reject: bool) -> (String, std::thread::JoinHandle<serde_json::Value>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let thread = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let mut socket = match listener.accept() {
                Ok((socket, _)) => socket,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "fixture received no request");
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(error) => panic!("{error}"),
            };
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut bytes = Vec::new();
            let (end, length) = loop {
                let mut part = [0u8; 4096];
                let n = socket.read(&mut part).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&part[..n]);
                if let Some(end) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..end]);
                    let length = header
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    break (end + 4, length);
                }
            };
            if bytes.starts_with(b"GET ") {
                socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"data\":[{\"id\":\"fixture-exact\",\"context_length\":8192}]}").unwrap();
                continue;
            }
            while bytes.len() < end + length {
                let mut part = [0u8; 4096];
                let n = socket.read(&mut part).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&part[..n]);
            }
            let body = serde_json::from_slice(&bytes[end..end + length]).unwrap();
            let response: &[u8] = if reject {
                b"HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"error\":{\"message\":\"unsupported parameter reasoning_effort\"}}"
            } else {
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {\"choices\":[{\"delta\":{\"content\":\"fixture-ok\"},\"finish_reason\":null}]}\n\ndata: [DONE]\n\n"
            };
            socket.write_all(response).unwrap();
            return body;
        }
    });
    (url, thread)
}

#[test]
fn native_launch_http_effort_capture_reaches_body_and_source_receipt_without_mutating_preferences()
{
    let _env = crate::tests::env_lock();
    let (url, server) = server(false);
    let inner = Arc::new(
        HttpClub::new(
            "openai-api",
            url,
            "fixture-exact",
            Some("fixture-only".into()),
        )
        .with_cli_model(),
    );
    inner.set_reasoning_effort("high").unwrap();
    let bound = crate::agent::club::launch_effort(inner.clone(), "low", "cli").unwrap();
    assert!(
        matches!(bound.chat(&[ChatMsg::user("literal fixture")], &[]).unwrap(), ClubReply::Text(s) if s == "fixture-ok")
    );
    let body = server.join().unwrap();
    assert_eq!(body["model"], "fixture-exact");
    assert_eq!(body["reasoning_effort"], "low");
    let receipt = crate::agent::harness::run_identity::current_for_turn().unwrap();
    assert_eq!(receipt.budgets["model_source"], "cli");
    assert_eq!(receipt.budgets["reasoning_effort_source"], "cli");
    assert_eq!(receipt.budgets["requested_reasoning_effort"], "low");
    assert_eq!(receipt.budgets["resolved_reasoning_effort"], receipt.effort);
    assert_eq!(inner.reasoning_effort().as_deref(), Some("high"));
}
#[test]
fn native_launch_http_explicit_effort_refuses_provider_rejection_instead_of_retrying_without_it() {
    let _env = crate::tests::env_lock();
    let (url, server) = server(true);
    let inner = Arc::new(HttpClub::new("openai-api", url, "fixture-exact", None));
    let bound = crate::agent::club::launch_effort(inner, "low", "cli").unwrap();
    assert!(
        bound
            .chat(&[ChatMsg::user("literal fixture")], &[])
            .unwrap_err()
            .contains("reasoning_effort")
    );
    assert_eq!(server.join().unwrap()["reasoning_effort"], "low");
}
#[test]
fn native_launch_model_requires_immutable_binding_not_dynamic_display_identity() {
    let fixed = HttpClub::new("local", "http://127.0.0.1:9/v1", "fixture-exact", None);
    assert!(fixed.supports_exact_launch_model("fixture-exact"));
    assert!(!fixed.supports_exact_launch_model("fixture"));
    let dynamic = fixed.follow_backend();
    assert!(!dynamic.supports_exact_launch_model("fixture-exact"));
    let mut bag = Bag::for_render_test(&[("practice", &[("practice", true)])]);
    bag.replace_in_hand_club_for_test(Arc::new(dynamic));
    assert!(
        bag.select_launch_route("local", Some("fixture-exact"))
            .is_err()
    );
}
