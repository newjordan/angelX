use super::*;

#[test]
fn http_error_bodies_yield_to_deadline_for_all_web_tools() {
    use std::time::{Duration, Instant};
    let _lock = crate::tests::env_lock();
    let tools: [&dyn Tool; 3] = [&WebFetchTool, &WebSearchTool, &HttpRequestTool];
    for tool in tools {
        let (url, server) = http_transport::tests::error_fixture();
        let _search_url = crate::tests::TestEnvGuard::set("ANGEL_SEARXNG_URL", &url);
        let start = Instant::now();
        let error = http_transport::with_deadline(Some(start + Duration::from_millis(400)), || {
            tool.call(&serde_json::json!({"url": url, "query": "fixture"}))
        })
        .unwrap_err();
        server.join().unwrap();
        assert!(error.contains("stalled {"), "{}: {error}", tool.name());
        assert!(error.contains("\"bound\":\"deadline\""), "{error}");
        assert!(error.contains("\"bytes_received\":4"), "{error}");
        assert!(start.elapsed() < Duration::from_secs(2));
        println!("{}: {error}", tool.name());
    }
}

#[test]
fn latin1_body_is_transcoded_not_mojibake() {
    // 0xE9 is 'é' in ISO-8859-1 but an invalid lone byte in UTF-8.
    let (text, note) = decode_body(&[b'c', b'a', b'f', 0xE9], Some("iso-8859-1"), false);
    assert_eq!(
        text, "café",
        "latin-1 byte is transcoded to the right codepoint"
    );
    assert!(note.as_deref().unwrap().contains("windows-1252") || note.is_some());
}

#[test]
fn utf8_body_passes_through_without_a_note() {
    let (text, note) = decode_body("café".as_bytes(), Some("utf-8"), false);
    assert_eq!(text, "café");
    assert!(note.is_none(), "clean UTF-8 carries no decode note");
}

#[test]
fn undeclared_but_invalid_utf8_degrades_lossily_with_a_note() {
    let (text, note) = decode_body(&[0xFF, 0xFE, b'x'], None, false);
    assert!(text.contains('x'));
    assert!(note.as_deref().unwrap().contains("invalid UTF-8"));
}

#[test]
fn html_meta_charset_is_sniffed_when_the_header_is_silent() {
    let html = b"<html><head><meta charset=\"shift_jis\"></head><body>";
    assert_eq!(sniff_meta_charset(html).as_deref(), Some("shift_jis"));
    // A meta charset drives decoding when no HTTP charset is present.
    let body = [b'<', b'p', b'>', 0xE9];
    let mut doc = b"<meta charset=iso-8859-1>".to_vec();
    doc.extend_from_slice(&body);
    let (text, note) = decode_body(&doc, None, true);
    assert!(
        text.contains('é'),
        "meta-declared latin-1 is transcoded: {text:?}"
    );
    assert!(note.is_some());
}

#[test]
fn no_charset_no_meta_defaults_to_utf8_passthrough() {
    assert_eq!(sniff_meta_charset(b"<html><body>plain</body>"), None);
    let (text, note) = decode_body(b"plain ascii", None, true);
    assert_eq!(text, "plain ascii");
    assert!(note.is_none());
}

#[test]
fn credential_headers_pin_http_requests_to_the_original_origin() {
    for header in [
        "Authorization",
        "Cookie",
        "X-Api-Key",
        "X-Custom-Token",
        "client-secret",
    ] {
        let headers = serde_json::json!({ header: "sensitive" });
        assert_eq!(
            http_request_redirect_limit("GET", headers.as_object()),
            0,
            "{header} must disable redirects"
        );
    }
    let public_headers = serde_json::json!({ "Accept": "application/json" });
    assert_eq!(
        http_request_redirect_limit("GET", public_headers.as_object()),
        5
    );
    assert_eq!(
        http_request_redirect_limit("POST", public_headers.as_object()),
        0
    );
}
