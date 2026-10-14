use super::*;

#[test]
fn a_restored_session_opens_the_same_link_and_serves_the_page_at_its_door() {
    let dir = std::env::temp_dir().join(format!("angel-session-link-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut view = DungeonView::default();
    view.save_path = Some(dir.join(".angel/dungeon/session.json"));
    view.shooter.set(Some(Run::new(7, 1, None)));
    view.shooter
        .host(crate::drive::together_sim::Door {
            bind: "127.0.0.1:0".parse().unwrap(),
            seats: 3,
            key: None,
            bundle: None,
        })
        .unwrap();
    view.hosting = Some(Hosting { friends: 2, watch: false });
    let invitation = view.shooter.invitation().expect("a hosted session's link");
    view.checkpoint(Instant::now(), true).unwrap();
    drop(view);

    let mut back = DungeonView::default();
    back.restore(&dir).unwrap();
    assert_eq!(
        back.shooter.invitation(),
        Some(invitation.clone()),
        "same door, same key"
    );
    assert!(back.hosting.is_some());
    // The door answers a browser with the page, as it did before the restart.
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect(&invitation.0).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    write!(
        stream,
        "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut answer = Vec::new();
    let _ = stream.read_to_end(&mut answer);
    assert!(
        answer.starts_with(b"HTTP/1.1 200"),
        "{}",
        String::from_utf8_lossy(&answer)
    );
    // Built with the Delve's web crate, the page is the Delve's, and its
    // wasm is served beside it.
    if crate::drive::together_web::bundle().is_some() {
        let page = String::from_utf8_lossy(&answer).into_owned();
        assert!(page.contains("<title>Delve</title>"), "{page}");
        let wasm = page
            .split("data-wasm=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("the page names its wasm");
        let mut stream = std::net::TcpStream::connect(&invitation.0).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        write!(
            stream,
            "GET {wasm} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut answer = Vec::new();
        let _ = stream.read_to_end(&mut answer);
        let body = answer
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .map(|at| &answer[at + 4..])
            .unwrap_or_default();
        assert!(answer.starts_with(b"HTTP/1.1 200"));
        assert!(body.starts_with(b"\0asm"), "the Delve's wasm");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
