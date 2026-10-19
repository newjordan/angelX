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

/// One plain GET to `addr`: the head and the body.
fn get(addr: &str, path: &str) -> (String, Vec<u8>) {
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut answer = Vec::new();
    let _ = stream.read_to_end(&mut answer);
    let at = answer
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("a response head");
    (
        String::from_utf8_lossy(&answer[..at]).into_owned(),
        answer[at + 4..].to_vec(),
    )
}

#[test]
fn the_hosted_page_names_the_gpu_renderer_it_serves() {
    // Built with the wasm-bindgen CLI at hand (`build.rs`), the full game's
    // page carries the renderer in both builds beside the Delve's wasm, each
    // file at an /a/<hash> name with its own content type. Without the CLI the
    // page names none and keeps its own presenter.
    let Some(bundle) = crate::drive::together_web::bundle() else {
        return;
    };
    let mut view = DungeonView::default();
    view.shooter.set(Some(Run::new(7, 1, None)));
    view.shooter
        .host(crate::drive::together_sim::Door {
            bind: "127.0.0.1:0".parse().unwrap(),
            seats: 2,
            key: None,
            bundle: Some(bundle),
        })
        .unwrap();
    let addr = view.shooter.invitation().expect("a hosted session's link").0;
    let (head, body) = get(&addr, "/");
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    let page = String::from_utf8_lossy(&body).into_owned();
    let json = page
        .split("id=\"renderers\">")
        .nth(1)
        .and_then(|rest| rest.split("</script>").next())
        .expect("the page carries its renderers");
    let renderers: serde_json::Value = serde_json::from_str(json).expect("the renderers are JSON");
    let renderers = renderers.as_object().expect("an object");
    if renderers.is_empty() {
        eprintln!("this build has no GPU renderer (no wasm-bindgen CLI at build time)");
        return;
    }
    assert_eq!(
        renderers.keys().map(String::as_str).collect::<Vec<_>>(),
        ["webgl", "webgpu"],
        "{json}"
    );
    for (build, files) in renderers {
        for (key, kind, magic) in [
            ("js", "text/javascript", &b""[..]),
            ("wasm", "application/wasm", &b"\0asm"[..]),
        ] {
            let url = files[key].as_str().expect("a url");
            assert!(url.starts_with("/a/") && url.ends_with(&format!(".{key}")), "{build} {key}: {url}");
            let (head, body) = get(&addr, url);
            assert!(head.starts_with("HTTP/1.1 200"), "{url}: {head}");
            assert!(
                head.to_ascii_lowercase().contains(&format!("content-type: {kind}")),
                "{url}: {head}"
            );
            assert!(body.starts_with(magic) && body.len() > 1000, "{url} is the {build} {key}");
        }
    }
}

#[test]
fn a_session_saved_with_four_seats_restores_and_hosts_six() {
    // The checkpoint of a delve hosted when the party was four wide: the
    // merchant's seat memories are four long. It restores, opens the same
    // door for the host and five friends, and the run steps on.
    let dir = std::env::temp_dir().join(format!("angel-session-four-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut view = DungeonView::default();
    view.save_path = Some(dir.join(".angel/dungeon/session.json"));
    view.shooter.set(Some(Run::new(7, 1, Some("Friend"))));
    view.shooter
        .host(crate::drive::together_sim::Door {
            bind: "127.0.0.1:0".parse().unwrap(),
            seats: 4,
            key: None,
            bundle: None,
        })
        .unwrap();
    view.hosting = Some(Hosting { friends: 3, watch: false });
    let invitation = view.shooter.invitation().expect("a hosted session's link");
    let path = view.save_path.clone().unwrap();
    view.checkpoint(Instant::now(), true).unwrap();
    drop(view);

    let mut saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let approaches = saved["run"]["merchant"]["approaches"].as_array_mut().expect("the merchant's memories");
    approaches.truncate(4);
    assert_eq!(approaches.len(), 4);
    std::fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();

    let mut back = DungeonView::default();
    back.restore(&dir).expect("a four-seat save loads");
    assert_eq!(back.shooter.invitation(), Some(invitation));
    assert_eq!(back.hosting.as_ref().map(|h| h.friends), Some(MAX_FRIENDS));
    assert_eq!(MAX_FRIENDS, 5);
    assert_eq!(
        back.shooter.as_ref().unwrap().merchant.approaches.len(),
        crate::drive::together_shooter::MAX_PLAYERS as usize
    );
    assert!(back.shooter.as_ref().unwrap().valid_snapshot());
    let _ = std::fs::remove_dir_all(&dir);
}
