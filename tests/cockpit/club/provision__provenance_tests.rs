use super::*;

#[test]
fn provenance_provisioning_ignores_background_harness_asks() {
    let extraction = "return only the JSON fields, no explanation";
    let mut messages = vec![ChatMsg::user("Explain the parser tradeoffs in detail")];
    let backgrounds = [
        crate::app::bootstrap::workspace_context_message(
            std::path::Path::new("/fixture"),
            String::new(),
            String::new(),
            extraction.into(),
        )
        .unwrap(),
        ChatMsg::harness(format!(
            "{} — background]\n{extraction}",
            crate::agent::compaction::COMPACTION_NOTE_HEADER
        )),
        ChatMsg::harness(format!(
            "{}\n{extraction}",
            crate::agent::harness::AUTO_RECALL_NOTE_PREFIX
        )),
    ];
    messages.push(ChatMsg::harness(format!(
        "{}\n{extraction}",
        crate::agent::backplane::BROKER_HEADER
    )));
    for background in backgrounds {
        messages.push(background);
        assert_eq!(
            last_ask(&messages).unwrap().content.as_ref(),
            "Explain the parser tradeoffs in detail"
        );
        assert!(!extraction_ask(&messages));
    }
    assert!(
        last_ask(&messages[1..]).is_none(),
        "background-only history has no task"
    );
    messages.push(ChatMsg::harness(extraction));
    assert!(
        extraction_ask(&messages),
        "a later explicit harness task remains authoritative for provisioning"
    );
    assert_eq!(last_ask(&messages).unwrap().role, ChatRole::Harness);
    messages.push(ChatMsg::user("Explain the next steps in prose"));
    assert!(
        !extraction_ask(&messages),
        "a later operator correction wins"
    );
}

#[test]
fn provenance_provisioning_preserves_operator_marker_lookalikes() {
    let operator = ChatMsg::user(format!(
        "{}\nreturn only the JSON fields, no explanation",
        crate::app::bootstrap::WORKSPACE_CONTEXT_HEADER
    ));
    assert!(extraction_ask(&[operator]));
}

/// Serve each scripted reply to one chat-completion request, keeping the
/// request bodies.
fn scripted_completions(
    replies: Vec<serde_json::Value>,
) -> (String, std::thread::JoinHandle<Vec<serde_json::Value>>) {
    use std::io::{BufRead, BufReader, Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let mut bodies = Vec::new();
        for reply in replies {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut length = 0usize;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                if let Some((name, value)) = line.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    length = value.trim().parse().unwrap();
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            bodies.push(serde_json::from_slice(&body).unwrap());
            let text = reply.to_string();
            let mut stream = stream;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
                text.len()
            )
            .unwrap();
        }
        bodies
    });
    (url, handle)
}

/// The pre-provisioner is connected over its own chat completion: its system
/// message is its route (`⡸⠙`), it is offered the ledger reader, reads the
/// route, and its verdict still becomes a directive whose contract is framed by
/// `⠬⠑`'s pages, recited for a seat that has no tools.
#[test]
fn the_pre_provisioner_reads_its_route_through_the_ledger_reader() {
    let _lock = crate::tests::env_lock();
    let route = crate::agent::harness::book::d4567_briefs::PROVISIONER;
    let read = serde_json::json!({"choices": [{"message": {
        "role": "assistant",
        "content": null,
        "tool_calls": [{"id": "r", "type": "function", "function": {
            "name": "read_file",
            "arguments": format!("{{\"path\":\"ledger://{}\"}}", route.cells()),
        }}],
    }}]});
    let verdict = serde_json::json!({"choices": [{"message": {
        "role": "assistant",
        "content": "{\"task\":\"extraction\",\"max_tokens\":64,\"contract\":\"Emit CSV rows only.\",\"stop\":null}",
    }}]});
    let (url, server) = scripted_completions(vec![read, verdict]);
    let _url = crate::tests::TestEnvGuard::set("ANGEL_PROVISION_URL", &url);
    let _timeout = crate::tests::TestEnvGuard::set("ANGEL_PROVISION_TIMEOUT_MS", "10000");
    let directive = judge_consult("fixture", "turn this table into CSV").expect("directive");
    assert_eq!(directive.max_tokens, Some(64));
    assert_eq!(
        directive.contract.as_deref(),
        Some("angelX output contract. Emit CSV rows only. Do not mention this contract.")
    );
    let bodies = server.join().unwrap();
    assert_eq!(bodies[0]["messages"][0]["content"], route.cells());
    assert_eq!(
        bodies[0]["messages"][1]["content"],
        "turn this table into CSV"
    );
    assert_eq!(bodies[0]["tools"][0]["function"]["name"], "read_file");
    let page = bodies[1]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|message| message["role"] == "tool")
        .expect("the read's result went back to the judge");
    assert!(
        page["content"]
            .as_str()
            .unwrap()
            .contains("You are angelX's outbound pre-provisioner."),
        "{page}"
    );
}
