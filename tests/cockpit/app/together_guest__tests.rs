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
fn together_shooter_guest_http_requires_the_token_and_serves_no_harness_routes() {
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
    assert!(page.head.contains("Content-Type: text/plain; charset=utf-8"));
    let hint = page.text();
    assert!(
        hint.contains("/dungeon join"),
        "the link says to paste it into angelX"
    );
    assert!(!hint.contains(&server.tokens[0]));
    let hud = request(&server, "GET", "/state", "", true).json();
    assert!(hud["floor"].as_u64().is_some());
    assert_eq!(hud["floors"].as_u64(), Some(6));
    assert!(hud["boss_support"].as_str().is_some());
    // The playable browser page is opt-in; with it on, the same link serves it.
    server.set_browser_view(true);
    let view = request(&server, "GET", "/", "", false);
    assert_eq!(view.status, 200);
    assert!(view.head.contains("Content-Type: text/html; charset=utf-8"));
    assert!(
        view.head.contains("img-src 'self' blob:"),
        "bounded object-URL PNGs are visible, not blocked by CSP"
    );
    let shown = view.text();
    assert!(!shown.contains(&server.tokens[0]));
    assert!(shown.contains("fetch('/hello'"), "the page takes its seat");
    assert!(shown.contains("fetch('/shooter/input'"), "the page sends held controls");
    assert!(shown.contains("location.hash"), "the seat code stays in the fragment");
    server.set_browser_view(false);
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
fn guest_state_counts_the_deep_and_calls_home_floor_zero() {
    let server = GuestServer::start("").unwrap();
    let run = Run::at_home(91, 7, Some("Guest"), Default::default(), Default::default());
    server.publish(&run, false, "");
    let state = request(&server, "GET", "/state", "", true).json();
    assert_eq!(
        (state["floor"].clone(), state["floors"].clone()),
        (0.into(), DEEPEST.into())
    );
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
            "boss_gates",
            "boss_names",
            "boss_support",
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
            "viewers",
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
        (1.into(), DEEPEST.into())
    );
    assert_eq!(state["pack"], run.dungeon.pack.name());
    assert_eq!(state["notice"], run.boss_gate_line().unwrap());
    assert_eq!(
        state["boss_gates"],
        serde_json::to_value(run.boss_gate_state()).unwrap()
    );
    assert_eq!(
        state["boss_support"],
        serde_json::to_value(run.boss_support_line()).unwrap()
    );
    assert_eq!(
        state["boss_names"],
        serde_json::to_value(
            run.bosses
                .iter()
                .take(64)
                .map(|boss| boss.name.clone())
                .collect::<Vec<_>>()
        )
        .unwrap()
    );
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
    // The original HUD budget excludes the added floor-politics projection.
    let mut base_hud = state.clone();
    for key in ["boss_gates", "boss_support", "boss_names"] {
        base_hud.as_object_mut().unwrap().remove(key);
    }
    assert!(
        base_hud.to_string().len() < 1280,
        "the base HUD stays small: {base_hud}"
    );

    let home = Run::at_home(
        91,
        7,
        Some("Guest <script>"),
        Default::default(),
        Default::default(),
    );
    server.publish(&home, false, "Clear a room\nto unbar its doors");
    assert_eq!(
        request(&server, "GET", "/state", "", true).json()["notice"],
        "Clear a roomto unbar its doors"
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

    // Every tick, and only when the run moved on.
    let tick = run.tick;
    server.publish(&run, false, "");
    assert_eq!(drawn(&server), Some((7, tick)));
    server.publish(&run, false, "");
    assert_eq!(
        drawn(&server),
        Some((7, tick)),
        "an unchanged tick is not a new frame"
    );
    run.step(&BTreeMap::new());
    server.publish(&run, false, "");
    assert_eq!(drawn(&server), Some((7, tick + 1)));
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
    // The painter draws only for a seat someone is looking at.
    request(&server, "GET", "/state", "", true);
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

#[test]
fn socket_handshake_answers_the_rfc_example_key() {
    // RFC 6455 section 1.3: the sample nonce and its accept value.
    assert_eq!(
        socket_accept("dGhlIHNhbXBsZSBub25jZQ=="),
        "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
    );
}

#[test]
fn fast_and_packed_room_pngs_carry_the_same_pixels() {
    let run = fight(12);
    let view = arena::frame_for(&run, FRAME_W as i32, FRAME_H as i32, Some(2));
    let rgb = view.rgb_bytes();
    let fast = encode_rgb(&rgb, true);
    let packed = encode_rgb(&rgb, false);
    for png in [&fast, &packed] {
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
        let pixels = image::load_from_memory_with_format(png, image::ImageFormat::Png)
            .unwrap()
            .to_rgb8();
        assert_eq!(pixels.dimensions(), (FRAME_W, FRAME_H));
        assert_eq!(pixels.as_raw(), &rgb);
    }
    assert!(
        fast.len() > packed.len(),
        "local png {} bytes should be larger than packed {}",
        fast.len(),
        packed.len()
    );
    // The painter skips a picture by its hash: the same run hashes the same,
    // and one changed pixel does not.
    let again = arena::frame_for(&run, FRAME_W as i32, FRAME_H as i32, Some(2));
    assert_eq!(view.content_hash(), again.content_hash());
    let mut moved = view.clone();
    let [r, g, b] = view.get(10, 10).unwrap_or([0, 0, 0]);
    moved.set(10, 10, [r ^ 1, g, b]);
    assert_ne!(view.content_hash(), moved.content_hash());
}

fn room_rgb() -> Vec<u8> {
    arena::frame_for(&fight(12), FRAME_W as i32, FRAME_H as i32, Some(2)).rgb_bytes()
}

fn blank() -> Vec<u8> {
    vec![0u8; FRAME_W as usize * FRAME_H as usize * 3]
}

fn dab(rgb: &mut [u8], x: usize, y: usize, colour: [u8; 3]) {
    let at = (y * FRAME_W as usize + x) * 3;
    rgb[at..at + 3].copy_from_slice(&colour);
}

fn picture(seq: u64, rgb: &[u8], fast: bool) -> Picture {
    Picture {
        seq,
        painted_ms: 0,
        stamp: [0; 8],
        rgb: Arc::new(rgb.to_vec()),
        png: None,
        fast,
    }
}

/// A patch body (after the socket header) laid over `prev`, as the page
/// lays it over its picture.
fn apply_patch(prev: &[u8], body: &[u8]) -> Vec<u8> {
    let count = u16::from_be_bytes([body[0], body[1]]) as usize;
    let colours = u16::from_be_bytes([body[2], body[3]]) as usize;
    let palette = &body[4..4 + colours * 3];
    let mut at = 4 + colours * 3;
    let mut out = prev.to_vec();
    for _ in 0..count {
        let (col, row) = (body[at] as usize, body[at + 1] as usize);
        at += 2;
        for p in 0..256 {
            let c = body[at + p] as usize * 3;
            let pixel = ((row * 16 + p / 16) * FRAME_W as usize + col * 16 + p % 16) * 3;
            out[pixel..pixel + 3].copy_from_slice(&palette[c..c + 3]);
        }
        at += 256;
    }
    assert_eq!(at, body.len(), "a patch is its tiles and nothing more");
    out
}

#[test]
fn a_patch_carries_only_the_changed_tiles_and_rebuilds_the_picture() {
    let prev = room_rgb();
    let mut next = prev.clone();
    dab(&mut next, 3, 3, [9, 9, 9]);
    dab(&mut next, 200, 100, [250, 1, 2]);
    let Tiles::Changed(body) = tile_patch(&prev, &next, LOCAL_PATCH_TILES) else {
        panic!("two pixels are two tiles");
    };
    assert_eq!(u16::from_be_bytes([body[0], body[1]]), 2);
    assert!(body.len() < 2 * 300, "{}", body.len());
    assert_eq!(apply_patch(&prev, &body), next);
    assert!(matches!(
        tile_patch(&prev, &prev, LOCAL_PATCH_TILES),
        Tiles::Same
    ));
    // A full repaint is a whole picture.
    assert!(matches!(
        tile_patch(&blank(), &vec![1u8; blank().len()], LOCAL_PATCH_TILES),
        Tiles::Wide
    ));
    // So is changed ink of more than 256 colours, even in a few tiles.
    let mut inky = blank();
    for i in 0..300 {
        dab(&mut inky, i, 0, [i as u8, (i >> 8) as u8, 7]);
    }
    assert!(matches!(
        tile_patch(&blank(), &inky, LOCAL_PATCH_TILES),
        Tiles::Wide
    ));
}

#[test]
fn no_patch_carries_more_tiles_than_the_page_takes() {
    assert_eq!(far_tile_cap(usize::MAX), MAX_PATCH_TILES);
    assert_eq!(far_tile_cap(0), 0);
    assert!(
        far_tile_cap(10_393) >= 8,
        "a packed room leaves room for tiles"
    );
    assert!(LOCAL_PATCH_TILES <= MAX_PATCH_TILES);
    let cols = FRAME_W as usize / 16;
    let tiles = |n: usize| {
        let mut rgb = blank();
        for t in 0..n {
            dab(&mut rgb, (t % cols) * 16, (t / cols) * 16, [5, 5, 5]);
        }
        rgb
    };
    let Tiles::Changed(body) = tile_patch(&blank(), &tiles(MAX_PATCH_TILES), usize::MAX) else {
        panic!("the page's limit fits");
    };
    assert_eq!(
        u16::from_be_bytes([body[0], body[1]]) as usize,
        MAX_PATCH_TILES
    );
    assert!(
        matches!(
            tile_patch(&blank(), &tiles(MAX_PATCH_TILES + 1), usize::MAX),
            Tiles::Wide
        ),
        "a budget past the page's limit is held to it"
    );
    assert!(
        BROWSER_VIEW.contains(&format!("MAX_TILES={MAX_PATCH_TILES}")),
        "the page and the host share the limit"
    );
}

#[test]
fn a_socket_patches_only_against_the_picture_its_page_holds() {
    let first = room_rgb();
    assert_eq!(
        socket_wire(&picture(1, &first, true), None, 0, 0),
        Wire::Whole,
        "a session starts with a whole picture"
    );
    let mut second = first.clone();
    dab(&mut second, 20, 20, [200, 10, 10]);
    let mut third = second.clone();
    dab(&mut third, 300, 150, [10, 200, 10]);
    // Picture 2 never went out (the window was shut). Picture 3 is built on
    // picture 1, the one the page holds, and rebuilds the newest from it.
    let held = (1, Arc::new(first.clone()));
    let Wire::Patch(wire) = socket_wire(&picture(3, &third, true), Some(&held), 30_000, 0) else {
        panic!("a small change is a patch");
    };
    assert_eq!(wire[0], SOCKET_PATCH);
    assert_eq!(u64::from_be_bytes(wire[1..9].try_into().unwrap()), 3);
    assert_eq!(u64::from_be_bytes(wire[25..33].try_into().unwrap()), 1);
    assert_eq!(apply_patch(&first, &wire[33..]), third);
    assert_eq!(
        socket_wire(&picture(4, &first, true), Some(&held), 30_000, 0),
        Wire::Same,
        "the page already shows these pixels"
    );
    assert_eq!(
        socket_wire(&picture(3, &third, true), Some(&held), 30_000, HEAL_AFTER),
        Wire::Whole,
        "a long run of patches heals with a whole picture"
    );
    // A far patch must beat the last whole PNG.
    let packed = encode_rgb(&first, false).len();
    assert!(matches!(
        socket_wire(&picture(3, &third, false), Some(&held), packed, 0),
        Wire::Patch(_)
    ));
    assert_eq!(
        socket_wire(&picture(3, &third, false), Some(&held), 300, 0),
        Wire::Whole
    );
}

/// The page's end of `/play`, as a browser speaks it.
struct TestSocket(TcpStream);

impl TestSocket {
    fn open(server: &GuestServer) -> Self {
        let mut stream = TcpStream::connect(server.address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        write!(
            stream,
            "GET /play HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: delve.v1, {}\r\n\r\n",
            server.tokens[0]
        )
        .unwrap();
        let mut head = Vec::new();
        let mut byte = [0u8];
        while !head.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            head.push(byte[0]);
        }
        assert!(
            head.starts_with(b"HTTP/1.1 101"),
            "{}",
            String::from_utf8_lossy(&head)
        );
        TestSocket(stream)
    }

    /// The next picture (kind 1 or 4), past state and notes.
    fn picture(&mut self) -> Vec<u8> {
        loop {
            let mut head = [0u8; 2];
            self.0.read_exact(&mut head).unwrap();
            let length = match head[1] & 0x7f {
                126 => {
                    let mut n = [0u8; 2];
                    self.0.read_exact(&mut n).unwrap();
                    u16::from_be_bytes(n) as usize
                }
                127 => {
                    let mut n = [0u8; 8];
                    self.0.read_exact(&mut n).unwrap();
                    u64::from_be_bytes(n) as usize
                }
                n => n as usize,
            };
            let mut payload = vec![0u8; length];
            self.0.read_exact(&mut payload).unwrap();
            if head[0] & 0x0f == 0x2 && matches!(payload.first(), Some(1 | 4)) {
                return payload;
            }
        }
    }

    fn send(&mut self, text: &str) {
        let mask = [1u8, 2, 3, 4];
        let mut frame = vec![0x81, 0x80 | text.len() as u8];
        frame.extend_from_slice(&mask);
        frame.extend(text.bytes().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        self.0.write_all(&frame).unwrap();
    }
}

fn seq_of(picture: &[u8]) -> u64 {
    u64::from_be_bytes(picture[1..9].try_into().unwrap())
}

#[test]
fn a_socket_starts_whole_and_answers_a_lost_picture_with_a_whole_one() {
    let server = GuestServer::start("").unwrap();
    let mut run = fight(40);
    let mut socket = TestSocket::open(&server);
    let deadline = Instant::now() + Duration::from_secs(30);
    while !server
        .published
        .lock()
        .unwrap()
        .frame_streams
        .contains_key(&2)
    {
        assert!(Instant::now() < deadline, "the session never opened");
        thread::sleep(Duration::from_millis(5));
    }
    server.publish(&run, false, "");
    let first = socket.picture();
    assert_eq!(
        first[0], SOCKET_FRAME,
        "a new session starts with a whole picture"
    );
    let seq = seq_of(&first);
    // The page lost it and asks: the same picture comes again, whole.
    socket.send(r#"{"keyframe":1}"#);
    let again = socket.picture();
    assert_eq!((again[0], seq_of(&again)), (SOCKET_FRAME, seq));
    socket.send(&format!(r#"{{"ack":{seq}}}"#));
    // A later picture is built on the one the page holds.
    run.step(&BTreeMap::new());
    run.players.get_mut(&2).unwrap().x += 2.0;
    server.publish(&run, false, "");
    let next = socket.picture();
    assert!(seq_of(&next) > seq);
    if next[0] == SOCKET_PATCH {
        assert_eq!(u64::from_be_bytes(next[25..33].try_into().unwrap()), seq);
    }
    // A second socket for the seat takes over, and starts whole too.
    let mut other = TestSocket::open(&server);
    assert_eq!(other.picture()[0], SOCKET_FRAME);
}

#[test]
fn nobody_watching_paints_nothing_and_a_new_viewer_gets_the_paused_picture() {
    let server = GuestServer::start("").unwrap();
    let run = fight(40);
    server.publish(&run, true, "");
    thread::sleep(Duration::from_millis(300));
    assert_eq!(server.published.lock().unwrap().frame_seq, 0);
    assert_eq!(
        drawn(&server),
        None,
        "nobody watches, so the painter has nothing"
    );
    // A poll makes the seat watched; the paused delve is drawn without a tick.
    assert_eq!(request(&server, "GET", "/frame.png", "", true).status, 503);
    server.publish(&run, true, "");
    wait_frame(&server, 1);
    let reply = request(&server, "GET", "/frame.png", "", true);
    assert_eq!(reply.status, 200);
    assert!(reply.head.contains("X-Frame-Seq: 1"));
    // The PNG is encoded once for the picture, however many polls ask.
    let kept = server.published.lock().unwrap().pictures[&2]
        .png
        .clone()
        .expect("the first poll keeps its PNG");
    assert_eq!(reply.body, *kept);
    assert_eq!(request(&server, "GET", "/frame.png", "", true).body, *kept);
    assert!(Arc::ptr_eq(
        &kept,
        server.published.lock().unwrap().pictures[&2]
            .png
            .as_ref()
            .unwrap()
    ));
}

#[test]
fn lead_state_ends_with_the_seat_session_and_with_stale_keys() {
    let right = Input {
        move_x: 1,
        ..Default::default()
    };
    let mut state = Published::default();
    let serial = state.open_frame_stream(2);
    state.held.insert(2, (right, Instant::now()));
    note_seat_rtt(&mut state, 2, 40);
    let published = std::sync::Mutex::new(state);
    assert_eq!(seat_leads(&published, &[2], true, 0.0).len(), 1);
    published.lock().unwrap().close_frame_stream(2, serial);
    assert!(
        seat_leads(&published, &[2], true, 0.0).is_empty(),
        "a closed session leads nothing"
    );
    assert!(!published.lock().unwrap().seat_rtt_ms.contains_key(&2));
    // An older session closing leaves the newer one's keys alone.
    {
        let mut state = published.lock().unwrap();
        let old = state.open_frame_stream(2);
        state.open_frame_stream(2);
        state.held.insert(2, (right, Instant::now()));
        state.close_frame_stream(2, old);
    }
    assert_eq!(seat_leads(&published, &[2], true, 0.0).len(), 1);
    // Keys older than the host's lease lead nothing either.
    published
        .lock()
        .unwrap()
        .held
        .insert(2, (right, Instant::now() - GUEST_LEASE));
    assert!(seat_leads(&published, &[2], true, 0.0).is_empty());
}

#[test]
fn a_browser_seat_is_painted_one_way_plus_a_frame_ahead() {
    let local = paint_lead(None, true);
    assert!((local - 1.0 / 120.0).abs() < 1e-6, "{local}");
    let far = paint_lead(Some(160), false);
    assert!((far - 0.080 - 1.0 / 60.0).abs() < 1e-4, "{far}");
    assert!((paint_lead(Some(9_000), true) - 0.25).abs() < 1e-6);
    let mut state = Published::default();
    note_seat_rtt(&mut state, 2, 160);
    state.held.insert(
        2,
        (
            Input {
                move_x: 1,
                ..Default::default()
            },
            Instant::now(),
        ),
    );
    note_seat_rtt(&mut state, 3, 9_000);
    assert_eq!(state.seat_rtt_ms[&2], 160);
    assert_eq!(state.seat_rtt_ms[&3], 500, "a wild trip is capped");
    let published = std::sync::Mutex::new(state);
    let leads = seat_leads(&published, &[2, 4], true, 0.0);
    assert_eq!(leads.len(), 1, "a seat with no held direction is not led");
    assert_eq!(leads[0].0, 2);
    assert!(
        (leads[0].2 - (0.080 + 1.0 / 120.0)).abs() < 1e-4,
        "{}",
        leads[0].2
    );
    let further = seat_leads(&published, &[2], true, 1.0 / 120.0);
    assert!(
        (further[0].2 - (0.080 + 2.0 / 120.0)).abs() < 1e-4,
        "a late tick adds a frame of lead, was {}",
        further[0].2
    );
    let capped = seat_leads(&published, &[2], true, 1.0);
    assert!(
        (capped[0].2 - 0.25).abs() < 1e-6,
        "lead stays within a quarter second"
    );
}

/// The page's wire and slide code, run as it is in V8.
fn run_page(script: &str) -> serde_json::Value {
    let start = BROWSER_VIEW
        .find("// Wire decoding and the slide")
        .expect("the page marks its tested block");
    let end = BROWSER_VIEW
        .find("// ---- end of the block the tests run")
        .expect("and where it ends");
    let source = format!("{}\n{script}", &BROWSER_VIEW[start..end]);
    let outcome = crate::agent::code_mode::run(
        &source,
        &[],
        &|_, _| Err("no tools".into()),
        &|calls| vec![0..calls.len()],
        Duration::from_secs(20),
        64,
    )
    .unwrap();
    serde_json::from_str(&outcome.result).unwrap()
}

#[test]
fn the_page_reads_the_hosts_patches_and_refuses_a_broken_one() {
    let first = room_rgb();
    let mut next = first.clone();
    dab(&mut next, 20, 20, [200, 10, 10]);
    dab(&mut next, 300, 150, [10, 200, 10]);
    let held = (1, Arc::new(first));
    let Wire::Patch(wire) = socket_wire(&picture(2, &next, true), Some(&held), 30_000, 0) else {
        panic!("a small change is a patch");
    };
    let bytes = |wire: &[u8]| wire.iter().map(u8::to_string).collect::<Vec<_>>().join(",");
    let body = 33 + 4 + u16::from_be_bytes([wire[35], wire[36]]) as usize * 3;
    let mut too_many = wire.clone();
    too_many[33..35].copy_from_slice(&(MAX_PATCH_TILES as u16 + 1).to_be_bytes());
    let mut off_edge = wire.clone();
    off_edge[body] = (FRAME_W / 16) as u8;
    let mut bad_ink = wire.clone();
    bad_ink[body + 2] = wire[36];
    let short = &wire[..wire.len() - 1];
    let read = run_page(&format!(
        "const read=bytes=>readPatch(new Uint8Array(bytes),{FRAME_W},{FRAME_H});
         const tiles=read([{}]);
         return JSON.stringify({{
           tiles:tiles&&tiles.map(t=>[t.col,t.row,Array.from(t.rgba)]),
           refused:[read([{}]),read([{}]),read([{}]),read([{}]),read([1,2,3])].map(t=>t===null),
         }});",
        bytes(&wire),
        bytes(&too_many),
        bytes(&off_edge),
        bytes(&bad_ink),
        bytes(short),
    ));
    assert_eq!(
        read["refused"],
        serde_json::json!([true, true, true, true, true])
    );
    let tiles = read["tiles"].as_array().expect("the host's patch reads");
    assert_eq!(tiles.len(), 2);
    for tile in tiles {
        let (col, row) = (
            tile[0].as_u64().unwrap() as usize,
            tile[1].as_u64().unwrap() as usize,
        );
        let rgba: Vec<u8> = tile[2]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();
        for p in 0..256 {
            let at = ((row * 16 + p / 16) * FRAME_W as usize + col * 16 + p % 16) * 3;
            assert_eq!(
                rgba[p * 4..p * 4 + 3],
                next[at..at + 3],
                "tile {col},{row} pixel {p}"
            );
            assert_eq!(rgba[p * 4 + 3], 255);
        }
    }
    // What the page cannot read, it does not draw: it asks for a whole one.
    assert!(BROWSER_VIEW.contains("if(!tiles){askWhole();return}"));
    assert!(BROWSER_VIEW.contains(r#"ws.send('{"keyframe":1}')"#));
}

#[test]
fn a_delayed_key_is_on_screen_within_one_frame() {
    // A 60 ms route cannot return a picture inside one frame. The page slides
    // the knight by at least one frame of walk in the key handler, so the
    // wait is not the round trip.
    let shift = run_page(
        "return JSON.stringify({start:photonShift(0,60,1,0),done:photonShift(60,60,1,0),
           local:photonShift(0,1,1,0),held:photonShift(30,60,1,0),diag:photonShift(0,60,1,1)});",
    );
    let pair = |key: &str| {
        let v = shift[key].as_array().unwrap();
        (v[0].as_f64().unwrap(), v[1].as_f64().unwrap())
    };
    let frame_px = 104.0 / 60.0;
    let (dx, dy) = pair("start");
    assert!(dx + 1e-4 >= frame_px, "{dx} < one frame ({frame_px})");
    assert!(dy.abs() < 1e-6);
    assert!(dx > 1.0, "the slide is visible before any picture returns");
    assert_eq!(
        pair("done").0,
        0.0,
        "the slide stops once the host picture can include the key"
    );
    assert_eq!(pair("local").0, 0.0, "a local trip is not slid");
    assert!((pair("held").0 - 104.0 * 0.030).abs() < 0.02);
    let (diag, down) = pair("diag");
    assert!((diag.hypot(down) - frame_px).abs() < 0.02, "{diag},{down}");
}
