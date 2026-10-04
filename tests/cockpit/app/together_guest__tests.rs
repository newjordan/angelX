use super::*;
use crate::drive::together_shooter::RoomKind;
use std::collections::BTreeMap;

struct Reply {
    status: u16,
    head: String,
    body: Vec<u8>,
}

impl Reply {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}

fn raw(server: &GuestServer, raw: &str) -> Reply {
    let mut stream = TcpStream::connect(server.address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream.write_all(raw.as_bytes()).unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    let split = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&response[..split]).into_owned();
    let status = head.split(' ').nth(1).unwrap().parse().unwrap();
    assert!(head.contains("Cache-Control: no-store"));
    assert!(head.contains("X-Content-Type-Options: nosniff"));
    assert!(head.contains("Content-Security-Policy: default-src 'none'"));
    assert!(!head.contains("Access-Control-Allow-Origin"));
    Reply {
        status,
        head,
        body: response[split + 4..].to_vec(),
    }
}

fn request(
    server: &GuestServer,
    method: &str,
    path: &str,
    body: &str,
    authenticated: bool,
) -> Reply {
    let auth = if authenticated {
        format!("Authorization: Bearer {}\r\n", server.tokens[0])
    } else {
        String::new()
    };
    raw(
        server,
        &format!(
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    )
}

/// The painter draws off the host's tick; wait for frame `seq` to land.
fn wait_frame(server: &GuestServer, seq: u64) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while server.published.lock().unwrap().frame_seq < seq {
        assert!(Instant::now() < deadline, "frame {seq} was never painted");
        thread::sleep(Duration::from_millis(5));
    }
}

fn drawn(server: &GuestServer) -> Option<(u64, u64)> {
    *server.drawn.lock().unwrap()
}

/// A run standing in a fight room, `ticks` into it, with both knights whole.
fn fight(ticks: u32) -> Run {
    let mut run = Run::new(11, 7, Some("Friend"));
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(room);
    let aim = BTreeMap::from([(
        1,
        Input {
            aim_y: -1,
            ..Default::default()
        },
    )]);
    for _ in 0..ticks {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        run.step(&aim);
    }
    run
}

#[test]
fn guest_http_requires_the_token_and_serves_no_harness_routes() {
    let server = GuestServer::start("").unwrap();
    assert_eq!(request(&server, "GET", "/state", "", true).status, 503);
    server.publish(&Run::new(91, 7, Some("Guest <script>")), false, "");
    assert_eq!(server.tokens[0].len(), 32);
    assert!(server.tokens[0].bytes().all(|b| b.is_ascii_hexdigit()));
    assert!(
        server
            .invitation_url()
            .ends_with(&format!("#{}", server.tokens[0]))
    );
    let page = request(&server, "GET", "/", "", false);
    assert_eq!(page.status, 200);
    let hint = page.text();
    assert!(hint.contains("/dungeon join"), "the link says to paste it into angelX");
    assert!(!hint.contains(&server.tokens[0]));
    for (method, path) in [
        ("GET", "/state"),
        ("GET", "/frame.png"),
        ("GET", "/frame.png?seq=1"),
        ("POST", "/shooter/input"),
    ] {
        assert_eq!(
            request(&server, method, path, "", false).status,
            401,
            "{path}"
        );
    }
    for (method, path) in [
        ("GET", "/shell"),
        ("GET", "/chat"),
        ("GET", "/tools"),
        ("GET", "/../.env"),
        ("GET", "/forge/agent"),
        ("GET", "/avatar/2.png"),
        ("POST", "/action"),
    ] {
        assert_eq!(
            request(&server, method, path, "", true).status,
            404,
            "{path}"
        );
    }
}

#[test]
fn guest_state_is_a_compact_hud_without_the_run_inside() {
    let server = GuestServer::start("").unwrap();
    let run = Run::new(91, 7, Some("Guest <script>"));
    server.publish(&run, false, "Clear a room\nto unbar its doors");
    let state = request(&server, "GET", "/state", "", true).json();
    let mut keys: Vec<&str> = state
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "actor",
            "book_cards",
            "floor",
            "floors",
            "frame",
            "frame_seq",
            "host",
            "music",
            "next_sequence",
            "notice",
            "pack",
            "paused",
            "phase",
            "phrasebook",
            "playable",
            "players",
            "raid_id",
            "realm",
            "realm_seq",
            "sanctuary",
            "score",
            "side_on",
            "sounds",
            "version",
            "voice",
        ]
    );
    assert_eq!(state["actor"], 2);
    assert_eq!(state["raid_id"], 7);
    assert_eq!(state["next_sequence"], 1);
    assert_eq!(state["frame"]["width"], FRAME_W);
    assert_eq!(state["frame"]["height"], FRAME_H);
    assert_eq!(
        (state["host"].clone(), state["paused"].clone()),
        (true.into(), false.into())
    );
    assert_eq!(state["playable"], true);
    assert_eq!(state["phase"], "fighting");
    assert_eq!(
        (state["floor"].clone(), state["floors"].clone()),
        (1.into(), 3.into())
    );
    assert_eq!(state["pack"], run.dungeon.pack.name());
    assert_eq!(state["notice"], "Clear a roomto unbar its doors");
    assert_eq!(state["sanctuary"], false);
    assert_eq!(state["book_cards"], run.book.cards.len());
    let players = state["players"].as_array().unwrap();
    assert_eq!(players.len(), 2);
    assert_eq!(
        players[0]["name"], "Host",
        "the host is not \"You\" to the guest"
    );
    assert_eq!(
        players[1],
        serde_json::json!({
            "id": 2, "name": "Guest <script>", "knight": "", "hp": 100, "max_hp": 100,
            "weapon": "Bow", "mail": 0, "bombs": 2, "alive": true, "stone": false, "vigil": false,
            "hand": [], "deck": [], "carrying": "nothing", "can_reforge": false, "can_wish": false,
            "hand_ids": [], "deck_ids": [], "arm_id": "bow",
            "spells": [null, null, null], "spell_ids": [null, null, null],
            "spell_cooldowns": [0, 0, 0],
        })
    );
    assert!(
        state.to_string().len() < 1280,
        "the HUD stays small: {state}"
    );

    server.publish(&run, true, "");
    let state = request(&server, "GET", "/state", "", true).json();
    assert_eq!(
        (state["paused"].clone(), state["playable"].clone()),
        (true.into(), false.into())
    );
    server.published.lock().unwrap().host_seen -= Duration::from_secs(3);
    assert_eq!(
        request(&server, "GET", "/state", "", true).json()["host"],
        false
    );
}

#[test]
fn frame_png_is_the_hosts_own_rendering_at_the_chosen_size() {
    assert_eq!((FRAME_W, FRAME_H), (384, 224));
    let server = GuestServer::start("").unwrap();
    assert_eq!(request(&server, "GET", "/frame.png", "", true).status, 503);
    let mut run = fight(40);
    server.publish(&run, false, "");
    wait_frame(&server, 1);
    let reply = request(&server, "GET", "/frame.png?seq=1", "", true);
    assert_eq!(reply.status, 200);
    assert!(reply.head.contains("Content-Type: image/png"));
    assert!(reply.head.contains("X-Frame-Seq: 1"));
    assert!(reply.body.starts_with(b"\x89PNG\r\n\x1a\n"));
    let picture = image::load_from_memory_with_format(&reply.body, image::ImageFormat::Png)
        .unwrap()
        .to_rgb8();
    assert_eq!(picture.dimensions(), (FRAME_W, FRAME_H));
    assert_eq!(
        picture.as_raw(),
        &arena::frame(&run, FRAME_W as i32, FRAME_H as i32).rgb_bytes(),
        "the guest sees the host's pixels"
    );
    assert_eq!(
        request(&server, "GET", "/state", "", true).json()["frame_seq"],
        1
    );

    // At most every other tick, and only when the run moved on.
    let tick = run.tick;
    server.publish(&run, false, "");
    assert_eq!(drawn(&server), Some((7, tick)));
    run.step(&BTreeMap::new());
    server.publish(&run, false, "");
    assert_eq!(
        drawn(&server),
        Some((7, tick)),
        "one tick is not yet a frame"
    );
    run.step(&BTreeMap::new());
    server.publish(&run, false, "");
    assert_eq!(drawn(&server), Some((7, tick + 2)));
    run.step(&BTreeMap::new());
    server.publish(&run, true, "");
    assert_eq!(
        drawn(&server),
        Some((7, tick + 3)),
        "a pause shows the last tick"
    );
    server.publish(&run, true, "");
    assert_eq!(drawn(&server), Some((7, tick + 3)));
    run.raid_id = 8;
    server.publish(&run, true, "");
    assert_eq!(
        drawn(&server),
        Some((8, tick + 3)),
        "a new delve redraws at once"
    );
    wait_frame(&server, 2);
}

#[test]
fn guest_http_bounds_body_headers_and_shutdown() {
    let server = GuestServer::start("").unwrap();
    let oversized =
        "POST /shooter/input HTTP/1.1\r\nHost: localhost\r\nContent-Length: 513\r\n\r\n";
    assert_eq!(raw(&server, oversized).status, 413);
    assert_eq!(
        raw(
            &server,
            "POST /shooter/input HTTP/1.1\r\nContent-Length: 0\r\nContent-Length: 0\r\n\r\n"
        )
        .status,
        400
    );
    assert_eq!(
        raw(
            &server,
            "POST /shooter/input HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n"
        )
        .status,
        400
    );
    let address = server.address;
    let before = Instant::now();
    drop(server);
    assert!(before.elapsed() < Duration::from_secs(1));
    assert!(TcpStream::connect(address).is_err());
}

#[test]
fn guest_public_url_and_private_listener_contracts() {
    for value in [
        "127.0.0.1",
        "192.168.1.2",
        "10.1.2.3",
        "172.16.5.2",
        "100.64.0.1",
        "100.127.255.254",
        "::1",
        "fd7a:115c:a1e0::1",
    ] {
        assert!(private_address(value.parse().unwrap()), "{value}");
    }
    for value in ["0.0.0.0", "::", "8.8.8.8", "100.63.255.255", "100.128.0.1"] {
        assert!(!private_address(value.parse().unwrap()), "{value}");
    }
    assert!(GuestServer::start("0.0.0.0:0").is_err());
    assert!(lan_address().is_none_or(|ip| private_address(ip) && !ip.is_loopback()));
    assert_eq!(
        validate_public_base("https://game.example:10000").unwrap(),
        "https://game.example:10000/"
    );
    for value in [
        "ftp://example/",
        "https://name:secret@example/",
        "https://example/path",
        "https://example/?token=x",
        "https://example/#token",
    ] {
        assert!(validate_public_base(value).is_err(), "{value}");
    }
}

#[test]
fn guest_request_rates_are_bounded_and_frames_have_their_own_budget() {
    let mut state = Published::default();
    for _ in 0..48 {
        assert!(state.allow_request());
    }
    assert!(!state.allow_request());
    for _ in 0..FRAMES_PER_SECOND {
        assert!(state.allow_frame());
    }
    assert!(!state.allow_frame());
    state.rate_start -= Duration::from_secs(2);
    assert!(state.allow_request() && state.allow_frame());
}

fn shooter_submission(sequence: u64, raid_id: u64, input: Input) -> String {
    serde_json::json!({"sequence": sequence, "raid_id": raid_id, "input": input}).to_string()
}

#[test]
fn shooter_transport_accepts_held_controls_across_ticks_and_rejects_stale_frames() {
    let server = GuestServer::start("").unwrap();
    let mut run = Run::new(91, 7, Some("Guest <script>"));
    server.publish(&run, false, "");
    let held = Input {
        move_x: 1,
        aim_y: -1,
        fire: true,
        ..Input::default()
    };
    let body = shooter_submission(1, 7, held);
    assert_eq!(
        request(&server, "POST", "/shooter/input", &body, false).status,
        401
    );
    assert_eq!(
        request(&server, "POST", "/shooter/input", &body, true).status,
        202
    );
    assert_eq!(
        request(&server, "POST", "/shooter/input", &body, true).status,
        202
    );
    let inputs = server.drain_shooter_intents();
    assert_eq!(
        inputs.len(),
        1,
        "identical retry must not renew the control lease"
    );
    assert_eq!((inputs[0].raid_id, inputs[0].input), (7, held));
    assert!(inputs[0].received_at.elapsed() < Duration::from_secs(1));
    assert_eq!(
        request(&server, "GET", "/state", "", true).json()["next_sequence"],
        2
    );
    for _ in 0..12 {
        run.step(&Default::default());
    }
    server.publish(&run, false, "");
    let release = shooter_submission(3, 7, Input::default());
    assert_eq!(
        request(&server, "POST", "/shooter/input", &release, true).status,
        202,
        "held frames do not depend on the current simulation tick or gapless sequence"
    );
    for stale in [
        shooter_submission(2, 7, held),
        shooter_submission(3, 7, held),
    ] {
        assert_eq!(
            request(&server, "POST", "/shooter/input", &stale, true).status,
            409
        );
    }
    assert_eq!(server.drain_shooter_intents()[0].input, Input::default());
    run.raid_id = 8;
    server.publish(&run, false, "");
    assert_eq!(
        request(
            &server,
            "POST",
            "/shooter/input",
            &shooter_submission(4, 7, held),
            true
        )
        .status,
        409
    );
    assert_eq!(
        request(
            &server,
            "POST",
            "/shooter/input",
            &shooter_submission(4, 8, held),
            true
        )
        .status,
        202
    );
    assert_eq!(server.drain_shooter_intents()[0].raid_id, 8);
}

#[test]
fn shooter_transport_releases_while_paused_and_rejects_extra_fields_and_invalid_axes() {
    let server = GuestServer::start("").unwrap();
    let held = Input {
        fire: true,
        ..Input::default()
    };
    assert_eq!(
        request(
            &server,
            "POST",
            "/shooter/input",
            &shooter_submission(1, 7, held),
            true
        )
        .status,
        409,
        "no delve is open yet"
    );
    let run = Run::new(91, 7, Some("Guest"));
    server.publish(&run, false, "");
    let base = serde_json::json!({"sequence":1,"raid_id":7,"input":Input::default()});
    let mut actor = base.clone();
    actor["actor"] = 1.into();
    let mut tool = base.clone();
    tool["input"]["tool"] = "shell".into();
    let mut axis = base.clone();
    axis["input"]["move_x"] = 2.into();
    let mut unknown = base.clone();
    unknown["input"]["move_y"] = (-2).into();
    for invalid in [actor, tool, axis, unknown] {
        assert_eq!(
            request(
                &server,
                "POST",
                "/shooter/input",
                &invalid.to_string(),
                true
            )
            .status,
            400,
            "{invalid}"
        );
    }
    assert!(server.drain_shooter_intents().is_empty());
    server.publish(&run, true, "");
    assert_eq!(
        request(
            &server,
            "POST",
            "/shooter/input",
            &shooter_submission(1, 7, held),
            true
        )
        .status,
        409
    );
    assert_eq!(
        request(&server, "POST", "/shooter/input", &base.to_string(), true).status,
        202,
        "a release always lands"
    );
    assert_eq!(server.drain_shooter_intents()[0].input, Input::default());
}

/// Review render: `ANGEL_DELVE_GUEST_FRAME=<file.png> cargo test ... -- --ignored`
/// writes the PNG a friend's angelX receives.
#[test]
#[ignore]
fn write_guest_frame() {
    let Some(path) = std::env::var_os("ANGEL_DELVE_GUEST_FRAME") else {
        return;
    };
    let server = GuestServer::start("").unwrap();
    server.publish(&fight(100), false, "");
    wait_frame(&server, 1);
    let reply = request(&server, "GET", "/frame.png", "", true);
    assert_eq!(reply.status, 200);
    std::fs::write(path, reply.body).unwrap();
}

/// Live review server: `ANGEL_DELVE_GUEST_SERVE=<file> cargo test ... -- --ignored`
/// writes the invitation to `<file>` and hosts a fight room for 45 seconds, so
/// a friend's angelX can play player 2 against it; player 2's moves are logged to
/// `<file>.log`. `ANGEL_DELVE_GUEST_PAUSED=1` holds it paused.
#[test]
#[ignore]
fn serve_guest_page() {
    let Some(path) = std::env::var_os("ANGEL_DELVE_GUEST_SERVE") else {
        return;
    };
    let paused = std::env::var_os("ANGEL_DELVE_GUEST_PAUSED").is_some();
    let server = GuestServer::start("127.0.0.1:0").unwrap();
    let mut run = fight(60);
    let mut log = std::fs::File::create(format!("{}.log", path.to_string_lossy())).unwrap();
    std::fs::write(&path, server.invitation_url()).unwrap();
    let mut remote: Option<(Input, Instant)> = None;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(45) {
        for intent in server.drain_shooter_intents() {
            writeln!(log, "{:?} input {:?}", start.elapsed(), intent.input).unwrap();
            remote = Some((intent.input, intent.received_at));
        }
        if !paused {
            for hero in run.players.values_mut() {
                hero.hp = hero.max_hp;
            }
            let held = remote
                .filter(|(_, at)| at.elapsed() < Duration::from_millis(250))
                .map_or_else(Input::default, |(input, _)| input);
            let before = (run.players[&2].x, run.players[&2].y);
            let inputs = BTreeMap::from([
                (
                    1,
                    Input {
                        aim_y: -1,
                        ..Default::default()
                    },
                ),
                (2, held),
            ]);
            run.step(&inputs);
            let after = (run.players[&2].x, run.players[&2].y);
            if after != before {
                writeln!(log, "{:?} p2 {after:?}", start.elapsed()).unwrap();
            }
        }
        server.publish(
            &run,
            paused,
            "Clear a room to unbar its doors · loot what falls · find the stairs down",
        );
        thread::sleep(Duration::from_millis(33));
    }
}

#[test]
fn guest_forge_checks_runes_and_queues_only_player_two_data() {
    let _guard = crate::tests::env_lock();
    let server = GuestServer::start("127.0.0.1:0").unwrap();
    let run = Run::new(17, 1, Some("Guest"));
    server.publish(&run, false, "");
    assert_eq!(
        request(&server, "GET", "/forge/rules", "", false).status,
        401
    );
    assert_eq!(
        request(&server, "GET", "/forge/rules", "", true).status,
        200
    );
    assert_eq!(
        request(
            &server,
            "POST",
            "/forge",
            "melee damage=34 reach=2 arc=120 every=18",
            true
        )
        .status,
        202
    );
    let gear = server.drain_gear();
    assert_eq!(gear.len(), 1);
    assert_eq!(gear[0].0, run.raid_id);
    assert!(matches!(&gear[0].2, Gear::Weapon(w) if w.melee.is_some()));
    assert_eq!(
        request(&server, "POST", "/forge", "execute rm -rf", true).status,
        400
    );
    assert!(server.drain_gear().is_empty());
}

#[test]
fn guest_card_is_checked_and_never_replaces_a_host_card() {
    let _guard = crate::tests::env_lock();
    let server = GuestServer::start("127.0.0.1:0").unwrap();
    let run = Run::new(17, 1, Some("Guest"));
    server.publish(&run, false, "");
    assert_eq!(
        request(&server, "GET", "/card/rules", "", false).status,
        401
    );
    assert_eq!(request(&server, "GET", "/card/rules", "", true).status, 200);
    let potion = "name Potion\nkind play\nheal 100\n";
    assert_eq!(request(&server, "POST", "/card", potion, true).status, 202);
    let gear = server.drain_gear();
    assert!(
        matches!(&gear[..], [(raid, 2, Gear::Card(card))] if *raid == run.raid_id && card.id == "p2-potion")
    );
    assert_eq!(
        request(
            &server,
            "POST",
            "/card",
            "name X\nkind hold\nexplode 9\n",
            true
        )
        .status,
        400
    );
    assert!(server.drain_gear().is_empty());
}

#[test]
fn guest_wishes_and_grants_reach_the_host_as_plain_words() {
    let _guard = crate::tests::env_lock();
    let server = GuestServer::start("127.0.0.1:0").unwrap();
    let run = Run::new(17, 1, Some("Guest"));
    server.publish(&run, false, "");
    assert_eq!(
        request(&server, "POST", "/wish", "a bridge over the river", false).status,
        401
    );
    assert_eq!(
        request(&server, "POST", "/wish", "a bridge over the river", true).status,
        202
    );
    assert_eq!(
        request(&server, "POST", "/grant", "bridge", true).status,
        202
    );
    assert_eq!(request(&server, "POST", "/wish", "   ", true).status, 400);
    let gear = server.drain_gear();
    assert!(
        matches!(&gear[..], [(_, _, Gear::Wish(w)), (_, _, Gear::Grant(g))] if w == "a bridge over the river" && g == "bridge")
    );
    let mut realm = crate::drive::together_realm::Realm::default();
    realm.ask("Guest", "a bridge over the river").unwrap();
    server.set_realm(RealmHud::of(&realm, None));
    let state = request(&server, "GET", "/state", "", true).json();
    assert_eq!(state["realm"]["wishes"][0]["status"], "asked");
    assert_eq!(state["realm"]["wishes"][0]["by"], "Guest");
    assert_eq!(request(&server, "GET", "/realm.png", "", true).status, 503);
    server.set_realm_picture(&[0; 2 * 2 * 3], 2, 2);
    assert_eq!(request(&server, "GET", "/realm.png", "", true).status, 200);
    assert_eq!(request(&server, "GET", "/realm.png", "", false).status, 401);
    assert_eq!(
        request(&server, "GET", "/state", "", true).json()["realm_seq"],
        1
    );
}

#[test]
fn guest_reforge_requests_reach_the_host_with_their_part() {
    let _guard = crate::tests::env_lock();
    let server = GuestServer::start("127.0.0.1:0").unwrap();
    let run = Run::new(17, 1, Some("Matt"));
    server.publish(&run, false, "");
    let body = r#"{"part":"offense","words":"triple shot that fans out"}"#;
    assert_eq!(request(&server, "POST", "/reforge", body, true).status, 202);
    assert_eq!(
        request(
            &server,
            "POST",
            "/reforge",
            r#"{"part":"hat","words":"a hat"}"#,
            true
        )
        .status,
        400
    );
    let gear = server.drain_gear();
    assert!(
        matches!(&gear[..], [(_, 2, Gear::Reforge(crate::drive::together_shooter::knights::Part::Offense, w))] if w == "triple shot that fans out")
    );
}
