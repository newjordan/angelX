use super::*;
use crate::club::types::{ChatMsg, Media};
use std::io::{Read, Write};
use std::net::TcpListener;

/// One-shot HTTP server: captures requests, each answered with the same
/// HTTP 400 + JSON error, exactly like the GLM endpoint rejecting an image part.
fn serve_rejecting(
    status: &'static str,
    json: &'static str,
) -> (
    String,
    std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    std::thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let bodies = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let captured = std::sync::Arc::clone(&bodies);
    let handle = std::thread::spawn(move || {
        for _ in 0..2 {
            let Ok((mut sock, _)) = listener.accept() else {
                return;
            };
            let mut req = Vec::new();
            let mut buf = [0u8; 16384];
            // Read headers, parse content-length, then read exactly that body.
            loop {
                let n = sock.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break;
                }
                req.extend_from_slice(&buf[..n]);
                if let Some(pos) = req.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&req[..pos]).to_ascii_lowercase();
                    let len: usize = headers
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse().ok())
                        .unwrap_or(0);
                    if req.len() >= pos + 4 + len {
                        break;
                    }
                }
            }
            captured
                .lock()
                .unwrap()
                .push(String::from_utf8_lossy(&req).into_owned());
            let resp = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{json}",
                json.len()
            );
            let _ = sock.write_all(resp.as_bytes());
            let _ = sock.flush();
        }
    });
    (format!("http://{addr}"), bodies, handle)
}

fn tiny_png() -> Vec<u8> {
    // Minimal valid 1x1 grayscale PNG (admission decodes it for real).
    vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8,
        0xCF, 0xC0, 0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D, 0xB0, 0x00, 0x00, 0x00,
        0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ]
}

/// The live failure: a clipboard image attachment is serialized as an
/// `image_url` data-URL part; the GLM endpoint answers HTTP 400. Before the
/// fix the 400 is terminal — the turn dies and later turns replay the same
/// poison part against the same text-only route. After the fix the club
/// learns the route is text-only, and the immediate retry strips image parts
/// so the conversation continues.
#[test]
fn glm_image_attachment_400_learns_text_only_and_retries_without_image_parts() {
    let media = Media::image_from_bytes(&tiny_png(), "clipboard image").unwrap();
    let msg = ChatMsg::user_with_media("what is this?", vec![media]);
    let (url, bodies, _h) = serve_rejecting(
        "400 Bad Request",
        r#"{"error":{"code":"1214","message":"messages.1.content.1.image_url is not supported in this endpoint"}}"#,
    );
    let club = HttpClub::new("glm", &url, "glm-5.3", None);
    let _ = club.chat(&[msg], &[]);
    let requests = bodies.lock().unwrap();
    assert!(
        requests.len() >= 2,
        "expected one immediate text-only retry after the image 400, got {} requests",
        requests.len()
    );
    assert!(
        requests[0].contains("image_url"),
        "first request carries the poison image part"
    );
    assert!(
        !requests[1].contains("image_url"),
        "retry must drop the image part for the text-only route"
    );
    assert!(requests[1].contains("what is this?"));
}
