//! A leaving bot drops the socket at its close deadline even if the server
//! never sends a Close frame.

use angel_bots::{run, BotConfig, Headless, CLOSE_TIMEOUT, STRAY};
use angel_serve::ws::{self, BINARY};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn a_bot_leaves_when_the_server_streams_non_close_frames_past_the_deadline() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else { return };
        let mut buf = Vec::new();
        let mut tmp = [0u8; 1024];
        loop {
            let Ok(n) = stream.read(&mut tmp) else { return };
            if n == 0 {
                return;
            }
            buf.extend_from_slice(&tmp[..n]);
            if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        let req = String::from_utf8_lossy(&buf);
        let Some(key) = req.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("sec-websocket-key")
                .then_some(value.trim().to_string())
        }) else {
            return;
        };
        let accept = ws::accept_key(&key);
        let response = format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
             Sec-WebSocket-Accept: {accept}\r\n\r\n"
        );
        if stream.write_all(response.as_bytes()).is_err() {
            return;
        }
        // Binary frames forever, never Close: the bot's close wait must
        // still end at its deadline.
        let mut frame = Vec::new();
        ws::frame(&mut frame, BINARY, b"x", None);
        loop {
            if stream.write_all(&frame).is_err() {
                break;
            }
        }
    });

    let cfg = BotConfig {
        url: format!("ws://{addr}/ws"),
        join: "x".into(),
        name: "Gwen".into(),
        seed: 1,
        wander: false,
        slow: false,
        fps: 30,
        warmup: Duration::ZERO,
        stray: STRAY,
    };
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(run::<Headless>(&cfg, Instant::now()));
    });
    let (report, _) = rx
        .recv_timeout(CLOSE_TIMEOUT + Duration::from_secs(1))
        .expect("the bot leaves within the close deadline");
    assert!(report.error.is_none(), "{}", report.error.unwrap());
}
