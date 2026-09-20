use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

fn test_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "angel-semantic-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn read_request(stream: &mut std::net::TcpStream) -> (String, Value) {
    let mut bytes = Vec::new();
    let mut buf = [0u8; 4096];
    let header_end = loop {
        let read = stream.read(&mut buf).unwrap();
        assert!(read > 0);
        bytes.extend_from_slice(&buf[..read]);
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let headers = String::from_utf8_lossy(&bytes[..header_end]).into_owned();
    let content_length = headers
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(str::trim)
                .and_then(|value| value.parse::<usize>().ok())
        })
        .unwrap();
    while bytes.len() < header_end + content_length {
        let read = stream.read(&mut buf).unwrap();
        assert!(read > 0);
        bytes.extend_from_slice(&buf[..read]);
    }
    let body = serde_json::from_slice(&bytes[header_end..header_end + content_length]).unwrap();
    (headers, body)
}

fn write_json(stream: &mut std::net::TcpStream, value: Value) {
    let body = serde_json::to_vec(&value).unwrap();
    write!(
            stream,
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        )
        .unwrap();
    stream.write_all(&body).unwrap();
}

#[test]
fn endpoint_policy_requires_https_off_host() {
    assert_eq!(
        validate_base("http://127.0.0.1:8098").unwrap(),
        "http://127.0.0.1:8098"
    );
    assert_eq!(
        validate_base("https://casper.example:8098/v2/").unwrap(),
        "https://casper.example:8098"
    );
    assert!(validate_base("http://casper:8098").is_err());
    assert!(validate_base("http://127.0.0.1.evil.example:8098").is_err());
    assert!(validate_base("file:///tmp/socket").is_err());
}

#[test]
fn chunking_is_bounded_and_utf8_safe() {
    let content = format!("{}\nlast\n", "β".repeat(MAX_CHUNK_BYTES));
    let chunks = chunks_for("src/lib.rs", &content);
    assert!(chunks.len() >= 3);
    assert!(
        chunks
            .iter()
            .all(|chunk| chunk.text.len() <= MAX_CHUNK_BYTES)
    );
    assert_eq!(chunks.concat_text(), content);
}

trait ConcatChunkText {
    fn concat_text(&self) -> String;
}
impl ConcatChunkText for [TextChunk] {
    fn concat_text(&self) -> String {
        self.iter().map(|chunk| chunk.text.as_str()).collect()
    }
}

#[test]
fn semantic_read_ranks_relevant_chunks_and_sends_auth() {
    let root = test_root("rank");
    std::fs::write(
        root.join("relevant.rs"),
        "fn block_private_redirects() { /* reject metadata and RFC1918 targets */ }\n",
    )
    .unwrap();
    std::fs::write(root.join("other.rs"), "fn paint_terminal_portrait() {}\n").unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        for request_index in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let (headers, body) = read_request(&mut stream);
            assert!(
                headers
                    .to_ascii_lowercase()
                    .contains("authorization: bearer test-key")
            );
            assert_eq!(body["truncate"], "NONE");
            let texts = body["texts"].as_array().unwrap();
            let vectors = if request_index == 0 {
                assert_eq!(body["input_type"], "query");
                vec![serde_json::json!([1.0, 0.0])]
            } else {
                assert_eq!(body["input_type"], "document");
                texts
                    .iter()
                    .map(|text| {
                        if text.as_str().unwrap().contains("RFC1918") {
                            serde_json::json!([1.0, 0.0])
                        } else {
                            serde_json::json!([0.0, 1.0])
                        }
                    })
                    .collect()
            };
            write_json(
                &mut stream,
                serde_json::json!({"embeddings": {"float": vectors}}),
            );
        }
    });

    let tool = SemanticReadTool {
        root: root.clone(),
        config: EmbeddingConfig {
            base_url: format!("http://{address}"),
            model: DEFAULT_MODEL.to_string(),
            api_key: Some("test-key".to_string()),
        },
    };
    let result = tool
        .call(&serde_json::json!({
            "query": "prevent private network redirects",
            "paths": ["other.rs", "relevant.rs"],
            "max_results": 2
        }))
        .unwrap();
    assert!(
        result.contains("1. score=1.0000 relevant.rs:1-1"),
        "{result}"
    );
    assert!(result.contains("untrusted workspace evidence"));
    server.join().unwrap();

    let outside = root.parent().unwrap().join("outside.rs");
    std::fs::write(&outside, "secret").unwrap();
    let error = tool
        .call(&serde_json::json!({"query":"secret", "paths":["../outside.rs"]}))
        .unwrap_err();
    assert!(
        error.contains("escapes workspace") || error.contains("outside"),
        "{error}"
    );
    std::fs::remove_file(outside).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn semantic_read_refuses_quarantine_and_secret_paths_before_network() {
    let root = test_root("policy");
    let tool = SemanticReadTool {
        root: root.clone(),
        config: EmbeddingConfig {
            base_url: "http://127.0.0.1:9".to_string(),
            model: DEFAULT_MODEL.to_string(),
            api_key: None,
        },
    };
    for path in ["off-limits/anything.rs", ".env", "id_ed25519", "tls.key"] {
        let error = tool
            .call(&serde_json::json!({"query":"x", "paths":[path]}))
            .unwrap_err();
        assert!(error.contains("refuses"), "{path}: {error}");
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires ANGEL_EMBED_CONFIG and the live Casper tunnel"]
fn live_shared_service_keeps_two_repository_roots_separate() {
    let config = EmbeddingConfig::load()
        .expect("ANGEL_EMBED_CONFIG must parse for the explicit live contract")
        .expect("ANGEL_EMBED_CONFIG is absent; the live Casper contract was not exercised");
    let alpha = test_root("live-alpha");
    let beta = test_root("live-beta");
    std::fs::write(
        alpha.join("network.rs"),
        "fn reject_redirect_target() { /* block RFC1918 and metadata IPs */ }\n",
    )
    .unwrap();
    std::fs::write(
        beta.join("gpu.rs"),
        "fn serialize_gpu_jobs() { /* one kernel benchmark at a time */ }\n",
    )
    .unwrap();
    let mut registry = ToolRegistry::new();
    maybe_register_embedding_tools(&mut registry, alpha.clone());
    assert!(registry.has_tool("semantic_read"));
    assert!(
        !registry
            .defs()
            .iter()
            .any(|tool| tool.name == "semantic_read")
    );
    registry.enable_tool_search();
    assert!(
        registry
            .bindable_tool_names()
            .iter()
            .any(|name| name == "semantic_read")
    );
    let alpha_tool = SemanticReadTool {
        root: alpha.clone(),
        config,
    };
    let beta_tool = SemanticReadTool {
        root: beta.clone(),
        config: EmbeddingConfig::load()
            .expect("ANGEL_EMBED_CONFIG must remain parseable")
            .expect("ANGEL_EMBED_CONFIG disappeared during the live Casper contract"),
    };
    let alpha_result = alpha_tool
        .call(&serde_json::json!({
            "query": "prevent SSRF through redirects",
            "paths": ["network.rs"]
        }))
        .unwrap();
    let beta_result = beta_tool
        .call(&serde_json::json!({
            "query": "coordinate GPU benchmark jobs",
            "paths": ["gpu.rs"]
        }))
        .unwrap();
    assert!(alpha_result.contains("network.rs:1-1"), "{alpha_result}");
    assert!(!alpha_result.contains("gpu.rs"), "{alpha_result}");
    assert!(beta_result.contains("gpu.rs:1-1"), "{beta_result}");
    assert!(!beta_result.contains("network.rs"), "{beta_result}");

    let escape = alpha_tool
        .call(&serde_json::json!({
            "query": "GPU jobs",
            "paths": [beta.join("gpu.rs").to_string_lossy()]
        }))
        .unwrap_err();
    assert!(
        escape.contains("outside") || escape.contains("workspace"),
        "{escape}"
    );
    std::fs::remove_dir_all(alpha).unwrap();
    std::fs::remove_dir_all(beta).unwrap();
}
