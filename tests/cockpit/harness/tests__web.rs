//! Web search/fetch, HTML scrubbing, and HTTP policy coverage.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory.

use super::*;

// --- web / HTTP suite -------------------------------------------------------

#[test]
fn searxng_formats_and_limits() {
    let v = serde_json::json!({
        "results": [
            { "title": "Rust", "url": "https://rust-lang.org", "content": "systems lang" },
            { "title": "Tokio", "url": "https://tokio.rs", "content": "async runtime" },
            { "title": "Serde", "url": "https://serde.rs", "content": "serialization" },
        ]
    });
    let out = format_searxng(&v, 2);
    assert!(out.contains("1. Rust") && out.contains("https://rust-lang.org"));
    assert!(out.contains("2. Tokio"));
    assert!(!out.contains("Serde"), "limit=2 should drop the third");
}

#[test]
fn searxng_handles_empty_and_missing() {
    assert_eq!(format_searxng(&serde_json::json!({}), 5), "no results");
    assert_eq!(
        format_searxng(&serde_json::json!({ "results": [] }), 5),
        "no results"
    );
    // Entries with no title and no content are skipped.
    let blanks = serde_json::json!({ "results": [ { "url": "x" } ] });
    assert_eq!(format_searxng(&blanks, 5), "no results");
}

#[test]
fn grok_tool_prompt_frames_a_scout_brief() {
    let p = crate::agent::tools::web::grok_tool_prompt("who won the match last night");
    // Carries the query and steers Grok toward live, cited, recency-first search.
    assert!(p.contains("who won the match last night"));
    assert!(p.contains("web and X"));
    assert!(p.to_ascii_lowercase().contains("cite source"));
    assert!(p.to_ascii_lowercase().contains("date"));
}

#[test]
#[ignore = "needs a live SearXNG on :8888 (run with --ignored when the web stack is up)"]
fn web_search_live() {
    let out = WebSearchTool
        .call(&serde_json::json!({ "query": "rust programming language", "limit": 3 }))
        .expect("live SearXNG query");
    assert!(out.contains('.') && out.contains("http"), "got: {out}");
}

#[test]
fn html_to_text_strips_markup_and_scripts() {
    let html = r#"<html><head><title>Doc</title>
            <script>var x = "<b>ignored</b>";</script>
            <STYLE>body { color: red }</STYLE></head>
            <body><h1>Hello</h1><p>a &amp; b &lt;ok&gt;</p></body></html>"#;
    let text = crate::agent::tools::web::html_to_text(html);
    assert!(text.contains("Doc") && text.contains("Hello"), "{text}");
    assert!(text.contains("a & b <ok>"), "{text}");
    assert!(
        !text.contains("var x"),
        "script body must be dropped: {text}"
    );
    assert!(
        !text.contains("color: red"),
        "style body must be dropped: {text}"
    );
    assert!(
        !text.contains('<') || text.contains("<ok>"),
        "no tags survive: {text}"
    );
}

#[test]
fn html_to_text_collapses_whitespace_and_survives_unterminated_blocks() {
    assert_eq!(
        crate::agent::tools::web::html_to_text("a\n\n\n\n\nb   c"),
        "a\n\nb c"
    );
    // An unterminated <script> drops the rest instead of panicking.
    let cut = crate::agent::tools::web::html_to_text("keep <script> everything after is gone");
    assert_eq!(cut, "keep");
}

#[test]
fn web_fetch_rejects_non_http_urls() {
    let t = crate::agent::tools::web::WebFetchTool;
    assert!(t.call(&serde_json::json!({})).is_err(), "missing url");
    assert!(
        t.call(&serde_json::json!({ "url": "file:///etc/passwd" }))
            .is_err(),
        "non-http scheme must be rejected"
    );
}

#[test]
fn web_http_policy_blocks_ssrf_ranges_and_allows_explicit_loopback() {
    use std::net::{IpAddr, Ipv4Addr};

    let allowed_public = IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34));
    let allowed_loopback = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let private = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7));
    let tailscale = IpAddr::V4(Ipv4Addr::new(100, 100, 0, 7));
    let metadata = IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254));
    let mapped_metadata: IpAddr = "::ffff:169.254.169.254".parse().unwrap();

    assert!(crate::agent::tools::web::http_ip_allowed(
        allowed_public,
        false
    ));
    assert!(crate::agent::tools::web::http_ip_allowed(
        allowed_loopback,
        false
    ));
    assert!(!crate::agent::tools::web::http_ip_allowed(private, false));
    assert!(crate::agent::tools::web::http_ip_allowed(private, true));
    assert!(!crate::agent::tools::web::http_ip_allowed(tailscale, false));
    assert!(crate::agent::tools::web::http_ip_allowed(tailscale, true));
    assert!(!crate::agent::tools::web::http_ip_allowed(metadata, true));
    assert!(!crate::agent::tools::web::http_ip_allowed(
        mapped_metadata,
        true
    ));
}

#[test]
fn http_request_mutations_are_disabled_by_default() {
    let _lock = crate::tests::env_lock();
    let _disabled = EnvGuard::set("ANGEL_HTTP_ALLOW_MUTATIONS", "0");
    let err = crate::agent::tools::web::HttpRequestTool
        .call(&serde_json::json!({
            "method": "POST",
            "url": "http://127.0.0.1:9/should-not-connect",
            "body": "{}"
        }))
        .expect_err("POST must fail before any connection without explicit opt-in");
    assert!(err.contains("ANGEL_HTTP_ALLOW_MUTATIONS"), "{err}");
}

#[test]
fn yolo_http_request_can_mutate_a_local_network_service_without_opt_ins() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let _lock = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let _mutations = EnvGuard::set("ANGEL_HTTP_ALLOW_MUTATIONS", "0");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = Vec::new();
        let mut chunk = [0u8; 2048];
        loop {
            let read = stream.read(&mut chunk).unwrap();
            assert!(read > 0, "client closed before sending a complete request");
            request.extend_from_slice(&chunk[..read]);
            let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n") else {
                continue;
            };
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_len = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if request.len() >= header_end + 4 + content_len {
                break;
            }
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .unwrap();
        String::from_utf8_lossy(&request).into_owned()
    });

    let out = crate::agent::tools::web::HttpRequestTool
        .call(&serde_json::json!({
            "method": "POST",
            "url": format!("http://{address}/deploy"),
            "body": "{\"go\":true}"
        }))
        .expect("YOLO POST should reach the operator-owned service");
    let request = server.join().unwrap();

    assert!(out.contains("[200 text/plain] POST"), "{out}");
    assert!(out.ends_with("ok"), "{out}");
    assert!(request.starts_with("POST /deploy HTTP/1.1"), "{request}");
    assert!(request.contains("{\"go\":true}"), "{request}");
}

#[test]
#[ignore = "fetches a live URL over the network; run with --ignored"]
fn web_fetch_live() {
    let out = crate::agent::tools::web::WebFetchTool
        .call(&serde_json::json!({ "url": "https://example.com", "max_bytes": 5000 }))
        .expect("live fetch");
    assert!(out.contains("Example Domain"), "got: {out}");
}

#[test]
fn default_kit_includes_web_fetch() {
    let _guard = crate::tests::env_lock();
    let names: Vec<String> = ToolRegistry::with_defaults()
        .defs()
        .into_iter()
        .map(|d| d.name)
        .collect();
    assert!(names.iter().any(|n| n == "web_fetch"), "{names:?}");
}

#[test]
fn tool_name_matches_def_name_for_all_tools() {
    let _guard = crate::tests::env_lock();
    // The cheap `name()` hot path MUST agree with `def().name` for every
    // registered tool, or dispatch (which now uses `name()`) would diverge
    // from the advertised schema. Guards against future drift.
    let r = ToolRegistry::with_defaults();
    for t in &r.tools {
        assert_eq!(
            t.name(),
            t.def().name,
            "Tool::name() disagrees with def().name"
        );
    }
}
