//! The server over real sockets, with a tiny two-seat game in document
//! mode: the protocol's rules, the seats, the events, and HTTP.

use angel_net::cmd::Command;
use angel_net::codec::Bytes;
use angel_net::doc::delta::apply;
use angel_net::doc::value::Value;
use angel_net::doc::wire::Reader;
use angel_net::proto::{PROTOCOL, Reject, ToClient, ToServer, caps};
use angel_serve::client::WsClient;
use angel_serve::rep::{DocRep, DocTick, Everything};
use angel_serve::ws::{self, BINARY, CLOSE, CONTINUATION, Message, PING, TEXT, code};
use angel_serve::{Events, GameServer, Rates, ServeConfig, Server};
use std::borrow::Cow;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

const KEY: &str = "TALLYKEYTALLYKEYTALLYKEY22";
const WAIT: Duration = Duration::from_secs(5);

/// Steps pressed this tick (-1, 0 or 1 on the wire). A press: a starved
/// tick does not repeat it, and a fold keeps every one.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Step(i8);

impl Command for Step {
    fn neutral() -> Self {
        Step(0)
    }
    fn held_only(&self) -> Self {
        Step(0)
    }
    fn absorb(&mut self, older: &Self) {
        self.0 = self.0.saturating_add(older.0);
    }
}

/// Two seats, each counting its steps.
#[derive(Default)]
struct Tally {
    names: [Option<String>; 2],
    steps: [i64; 2],
}

impl GameServer for Tally {
    type Cmd = Step;
    type Rep = DocRep<Everything>;
    const GAME: &'static str = "tally";

    fn rates(&self) -> Rates {
        Rates { tick_hz: 20, snapshot_hz: 10 }
    }
    fn seats(&self) -> u32 {
        2
    }
    fn params(&self) -> Vec<u8> {
        vec![7]
    }
    fn replicator(&self) -> Self::Rep {
        DocRep(Everything)
    }
    fn decode_cmd(bytes: &[u8]) -> Option<Step> {
        match bytes {
            [b] if (*b as i8).abs() <= 1 => Some(Step(*b as i8)),
            // A bug in game code, for the test that a panic stays contained.
            [0x55] => panic!("the tally cannot read 0x55"),
            _ => None,
        }
    }
    fn join(&mut self, seat: u32, name: &str, events: &mut Events) {
        self.names[seat as usize] = Some(name.to_string());
        events.all(format!("join {name}"));
    }
    fn leave(&mut self, seat: u32, _events: &mut Events) {
        self.names[seat as usize] = None;
    }
    fn action(&mut self, seat: Option<u32>, kind: u32, body: &[u8], events: &mut Events) {
        events.all(format!("all {kind} {}", String::from_utf8_lossy(body)));
        if let Some(seat) = seat {
            events.seat(seat, "you");
        }
    }
    fn step(&mut self, _tick: u32, cmds: &[Option<Step>], _events: &mut Events) {
        for (seat, cmd) in cmds.iter().enumerate() {
            if let Some(Step(s)) = cmd {
                self.steps[seat] += i64::from(*s);
            }
        }
    }
    fn publish(&mut self, tick: u32) -> DocTick {
        let names = self.names.iter().map(|n| n.as_deref().map_or(Value::None, |n| Value::Some(Box::new(Value::Str(n.into())))));
        DocTick {
            tree: Value::Struct(vec![
                (Cow::Borrowed("tick"), Value::U(u64::from(tick))),
                (Cow::Borrowed("names"), Value::Seq(names.collect())),
                (Cow::Borrowed("steps"), Value::Seq(self.steps.iter().map(|&s| Value::I(s)).collect())),
            ]),
        }
    }
}

fn start() -> Server {
    let mut config = ServeConfig::new("127.0.0.1:0".parse::<SocketAddr>().unwrap());
    config.key = Some(KEY.into());
    Server::start(Tally::default(), config).unwrap()
}

fn url(server: &Server) -> String {
    format!("ws://{}/ws", server.addr())
}

fn hello_bytes(game: &str, protocol: u32, join: &str, caps: u32) -> Vec<u8> {
    ToServer::Hello { protocol, game, join, resume: "", name: "Gwen", caps }.to_vec()
}

/// The next binary message, skipping nothing else but pongs.
fn next_binary(ws: &mut WsClient) -> Vec<u8> {
    let until = Instant::now() + WAIT;
    while Instant::now() < until {
        match ws.recv(WAIT).unwrap() {
            Some(Message::Binary(b)) => return b,
            Some(Message::Pong(_)) | None => {}
            Some(other) => panic!("expected a message, got {other:?}"),
        }
    }
    panic!("no message in time");
}

fn join(server: &Server, caps: u32) -> (WsClient, Option<u32>) {
    let mut ws = WsClient::connect(&url(server), WAIT).unwrap();
    ws.send(&hello_bytes("tally", PROTOCOL, KEY, caps)).unwrap();
    match ToClient::decode(&next_binary(&mut ws)) {
        Ok(ToClient::Welcome(w)) => {
            assert_eq!(w.params, &[7]);
            assert_eq!((w.tick_hz, w.snapshot_hz), (20, 10));
            let seat = w.seat;
            (ws, seat)
        }
        other => panic!("not a welcome: {other:?}"),
    }
}

/// Read until the socket closes; the reject (if any) and the close code.
fn until_closed(ws: &mut WsClient) -> (Option<Reject>, Option<u16>) {
    until_closed_within(ws, 2 * WAIT)
}

fn until_closed_within(ws: &mut WsClient, wait: Duration) -> (Option<Reject>, Option<u16>) {
    let mut why = None;
    let until = Instant::now() + wait;
    while Instant::now() < until {
        match ws.recv(Duration::from_millis(200)) {
            Ok(Some(Message::Binary(b))) => {
                if let Ok(ToClient::Reject { why: w, .. }) = ToClient::decode(&b) {
                    why = Some(w);
                }
            }
            Ok(Some(Message::Close(c))) => return (why, c),
            Ok(_) => {}
            Err(_) => return (why, None),
        }
    }
    panic!("the socket stayed open");
}

#[test]
fn document_mode_sends_the_tree_whole_then_patches_and_acks_each_command() {
    let server = start();
    let (mut ws, seat) = join(&server, 0);
    let seat = seat.unwrap() as usize;
    let mut reader = Reader::default();
    let mut tree = Value::Unit;
    let mut seq = 0;
    let mut acked = 0;
    // The newest tick read, said back as a client does (`applied`).
    let mut applied = 0;
    let until = Instant::now() + WAIT;
    // Every command walked: the tree says 10 (a late one's step may come a
    // tick after the ack that counted its seq).
    let walked = |tree: &Value| matches!(tree.field("steps"), Some(Value::Seq(s)) if s.get(seat) == Some(&Value::I(10)));
    while (acked < 10 || !walked(&tree)) && Instant::now() < until {
        if seq < 10 {
            seq += 1;
            ws.send(&ToServer::Cmds { applied, cmds: vec![(seq, &[1u8][..])] }.to_vec()).unwrap();
        }
        let msg = next_binary(&mut ws);
        if let Ok(ToClient::Snapshot { ack, whole, body, tick }) = ToClient::decode(&msg) {
            if whole {
                reader = Reader::default();
                tree = reader.value(&mut Bytes::new(body)).unwrap();
            } else if !body.is_empty() {
                apply(&mut tree, reader.patch(&mut Bytes::new(body)).unwrap()).unwrap();
            }
            acked = ack;
            applied = tick;
        }
    }
    // A tick the tenth command was late for plays it as guessed and the
    // ack moves on (`SeatInputs`), so the ack may pass 10; each step is
    // still walked once.
    assert!(acked >= 10, "{acked}");
    let Some(Value::Seq(steps)) = tree.field("steps") else { panic!("{tree:?}") };
    assert_eq!(steps[seat], Value::I(10), "every command walked one step, once");
    let Some(Value::Seq(names)) = tree.field("names") else { panic!() };
    assert_eq!(names[seat], Value::Some(Box::new(Value::Str("Gwen".into()))));
    server.stop();
}

#[test]
fn past_the_seats_clients_watch_and_a_watcher_never_takes_a_seat() {
    let server = start();
    let (_a, sa) = join(&server, caps::WATCH);
    assert_eq!(sa, None);
    let (_b, sb) = join(&server, 0);
    let (_c, sc) = join(&server, 0);
    let (_d, sd) = join(&server, 0);
    assert_eq!((sb, sc, sd), (Some(0), Some(1), None));
    assert_eq!(server.players().len(), 2);
    server.stop();
}

#[test]
fn a_wrong_protocol_game_or_key_is_told_why() {
    let server = start();
    for (hello, expect) in [
        (hello_bytes("tally", PROTOCOL + 1, KEY, 0), Reject::Protocol),
        (hello_bytes("delve", PROTOCOL, KEY, 0), Reject::Game),
        (hello_bytes("tally", PROTOCOL, "WRONG", 0), Reject::Key),
        (ToServer::Bye.to_vec(), Reject::Abuse),
    ] {
        let mut ws = WsClient::connect(&url(&server), WAIT).unwrap();
        ws.send(&hello).unwrap();
        let (why, _) = until_closed(&mut ws);
        assert_eq!(why, Some(expect));
    }
    server.stop();
}

#[test]
fn frames_that_break_the_rules_close_the_socket_with_their_code() {
    let server = start();
    // Unmasked.
    let mut ws = WsClient::connect(&url(&server), WAIT).unwrap();
    let mut raw = Vec::new();
    ws::frame(&mut raw, BINARY, &hello_bytes("tally", PROTOCOL, KEY, 0), None);
    ws.stream().write_all(&raw).unwrap();
    assert_eq!(until_closed(&mut ws), (Some(Reject::Abuse), Some(code::PROTOCOL)));
    // Text.
    let mut ws = WsClient::connect(&url(&server), WAIT).unwrap();
    ws.send_frame(TEXT, b"hello").unwrap();
    assert_eq!(until_closed(&mut ws).1, Some(code::UNSUPPORTED));
    // Past the message limit, after joining.
    let (mut ws, _) = join(&server, 0);
    ws.send(&vec![0u8; 2000]).unwrap();
    assert_eq!(until_closed(&mut ws), (Some(Reject::Abuse), Some(code::TOO_BIG)));
    // A message that does not read.
    let (mut ws, _) = join(&server, 0);
    ws.send(&[0x7f, 1, 2]).unwrap();
    assert_eq!(until_closed(&mut ws), (Some(Reject::Abuse), Some(code::POLICY)));
    server.stop();
}

#[test]
fn too_many_messages_a_second_is_abuse() {
    let server = start();
    let (mut ws, _) = join(&server, 0);
    // The played server permits a page's catch-up burst (2026-10-23,
    // 639972239); exceed its current rate, not the retired 120-message cap.
    for _ in 0..3 * angel_serve::limits::MAX_MESSAGES_PER_SEC {
        if ws.send(&ToServer::Ping { t: 0.0 }.to_vec()).is_err() {
            break;
        }
    }
    assert_eq!(until_closed(&mut ws), (Some(Reject::Abuse), Some(code::POLICY)));
    server.stop();
}

#[test]
fn a_fragmented_hello_reads_and_pings_and_closes_are_answered() {
    let server = start();
    let mut ws = WsClient::connect(&url(&server), WAIT).unwrap();
    let hello = hello_bytes("tally", PROTOCOL, KEY, 0);
    let (a, rest) = hello.split_at(3);
    let (b, c) = rest.split_at(5);
    let mut raw = Vec::new();
    for (op, part, fin) in [(BINARY, a, false), (PING, &b"between"[..], true), (CONTINUATION, b, false), (CONTINUATION, c, true)] {
        let start = raw.len();
        ws::frame(&mut raw, op, part, Some([1, 2, 3, 4]));
        if !fin {
            raw[start] &= 0x7f;
        }
    }
    ws.stream().write_all(&raw).unwrap();
    // The ping before the hello was finished is answered.
    assert_eq!(ws.recv(WAIT).unwrap(), Some(Message::Pong(b"between".to_vec())));
    assert!(matches!(ToClient::decode(&next_binary(&mut ws)), Ok(ToClient::Welcome(_))));
    ws.send_frame(PING, b"are you there").unwrap();
    let until = Instant::now() + WAIT;
    loop {
        assert!(Instant::now() < until, "no pong");
        if let Some(Message::Pong(p)) = ws.recv(WAIT).unwrap() {
            assert_eq!(p, b"are you there");
            break;
        }
    }
    ws.send_frame(CLOSE, &ws::close_payload(code::NORMAL)).unwrap();
    assert_eq!(until_closed(&mut ws), (None, Some(code::NORMAL)));
    server.stop();
}

#[test]
fn events_reach_their_audience() {
    let server = start();
    let (mut a, _) = join(&server, 0);
    let (mut b, _) = join(&server, 0);
    a.send(&ToServer::Action { kind: 3, body: b"wish" }.to_vec()).unwrap();
    let heard = |ws: &mut WsClient| -> Vec<String> {
        let mut lines = Vec::new();
        let until = Instant::now() + Duration::from_millis(800);
        while Instant::now() < until {
            if let Ok(Some(Message::Binary(m))) = ws.recv(Duration::from_millis(50))
                && let Ok(ToClient::Events { events, .. }) = ToClient::decode(&m)
            {
                lines.extend(events.iter().map(|e| String::from_utf8_lossy(e).into_owned()));
            }
        }
        lines
    };
    let (ha, hb) = (heard(&mut a), heard(&mut b));
    assert!(ha.contains(&"all 3 wish".to_string()) && ha.contains(&"you".to_string()), "{ha:?}");
    assert!(hb.contains(&"all 3 wish".to_string()) && !hb.contains(&"you".to_string()), "{hb:?}");
    server.stop();
}

#[test]
fn the_host_stopping_tells_everyone() {
    let server = start();
    let (mut ws, _) = join(&server, 0);
    server.stop();
    assert_eq!(until_closed(&mut ws).0, Some(Reject::Stopping));
}

fn http(server: &Server, request: &str) -> String {
    let mut s = TcpStream::connect(server.addr()).unwrap();
    s.set_read_timeout(Some(WAIT)).unwrap();
    s.write_all(request.as_bytes()).unwrap();
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out);
    String::from_utf8_lossy(&out).into_owned()
}

#[test]
fn http_keeps_alive_answers_head_and_refuses_what_it_does_not_serve() {
    let server = start();
    let two = http(&server, "GET / HTTP/1.1\r\nHost: x\r\n\r\nHEAD / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n");
    assert_eq!(two.matches("HTTP/1.1 200 OK").count(), 2, "{two}");
    // The HEAD's length is the page's, with no page after it.
    assert_eq!(two.matches("<p>").count(), 1);
    assert!(http(&server, "GET /a/nothing.js HTTP/1.1\r\nConnection: close\r\n\r\n").starts_with("HTTP/1.1 404"));
    assert!(http(&server, "DELETE / HTTP/1.1\r\nConnection: close\r\n\r\n").starts_with("HTTP/1.1 405"));
    assert!(http(&server, "GET /ws HTTP/1.1\r\nConnection: close\r\n\r\n").starts_with("HTTP/1.1 400"));
    let long = format!("GET / HTTP/1.1\r\nX-Long: {}\r\n\r\n", "x".repeat(5000));
    assert!(http(&server, &long).starts_with("HTTP/1.1 431"));
    let old = "GET /ws HTTP/1.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 8\r\n\r\n";
    assert!(http(&server, old).starts_with("HTTP/1.1 426"));
    server.stop();
}

#[test]
fn a_socket_that_never_says_hello_is_closed_after_the_handshake_time() {
    let server = start();
    let started = Instant::now();
    let mut ws = WsClient::connect(&url(&server), WAIT).unwrap();
    let limit = angel_serve::limits::HANDSHAKE_TIME;
    let (why, code) = until_closed_within(&mut ws, limit + 2 * WAIT);
    assert_eq!((why, code), (Some(Reject::Abuse), Some(ws::code::POLICY)));
    let waited = started.elapsed();
    assert!(waited >= limit - Duration::from_millis(100) && waited < limit + Duration::from_secs(2), "{waited:?}");
    server.stop();
}

#[test]
fn a_class_loading_the_page_at_once_leaves_room_to_join_and_idle_pages_are_let_go() {
    let server = start();
    // 300 page loads, each kept alive after its response.
    let mut pages = Vec::new();
    for _ in 0..300 {
        let mut s = TcpStream::connect(server.addr()).unwrap();
        s.write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        pages.push(s);
    }
    for s in &mut pages {
        s.set_read_timeout(Some(WAIT)).unwrap();
        let mut head = [0u8; 15];
        s.read_exact(&mut head).unwrap();
        assert_eq!(&head, b"HTTP/1.1 200 OK");
    }
    // Someone joins while they all sit open.
    let (_ws, seat) = join(&server, 0);
    assert!(seat.is_some());
    // An idle kept-alive page is closed a couple of seconds on.
    let started = Instant::now();
    let mut rest = Vec::new();
    let _ = pages[0].read_to_end(&mut rest);
    let waited = started.elapsed();
    assert!(waited < Duration::from_secs(4), "{waited:?}");
    server.stop();
}

#[test]
fn a_reload_takes_its_seat_back_and_the_old_page_is_told_not_to_return() {
    let server = start();
    let (mut old, seat) = join(&server, 0);
    // The welcome carried the token; ask again for it the way a page would.
    let mut fresh = WsClient::connect(&url(&server), WAIT).unwrap();
    fresh.send(&hello_bytes("tally", PROTOCOL, KEY, 0)).unwrap();
    let first = next_binary(&mut fresh);
    let Ok(ToClient::Welcome(w)) = ToClient::decode(&first) else { panic!() };
    let token = w.seat_token.to_string();
    let other_seat = w.seat;
    assert_ne!(other_seat, seat);
    // The second page reloads: same token, while its old socket still lives.
    let mut reloaded = WsClient::connect(&url(&server), WAIT).unwrap();
    reloaded
        .send(&ToServer::Hello { protocol: PROTOCOL, game: "tally", join: "", resume: &token, name: "", caps: 0 }.to_vec())
        .unwrap();
    let again = next_binary(&mut reloaded);
    let Ok(ToClient::Welcome(w)) = ToClient::decode(&again) else { panic!() };
    assert_eq!((w.seat, w.seat_token), (other_seat, token.as_str()));
    assert_eq!(until_closed(&mut fresh), (None, Some(code::REPLACED)));
    // The first page was never disturbed.
    assert!(ToClient::decode(&next_binary(&mut old)).is_ok());
    assert_eq!(server.players().len(), 2);
    server.stop();
}

#[test]
fn a_panic_in_game_code_ends_only_that_connection_and_holds_its_seat() {
    let server = start();
    let (mut ws, seat) = join(&server, 0);
    ws.send(&ToServer::Cmds { applied: 0, cmds: vec![(1, &[0x55u8][..])] }.to_vec()).unwrap();
    let (_, _) = until_closed(&mut ws);
    let deadline = Instant::now() + WAIT;
    while server.players().iter().any(|p| p.connected) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(server.players(), vec![angel_serve::Player { seat: seat.unwrap(), name: "Gwen".into(), connected: false }]);
    // Everyone else carries on.
    let (_other, other_seat) = join(&server, 0);
    assert!(other_seat.is_some() && other_seat != seat);
    server.stop();
}

// ---- in-process connections: the host's own, through the same doors ----

use angel_serve::{Closed, LocalConn};

fn local_hello(conn: &LocalConn, join: &str, resume: &str, name: &str) {
    conn.send(ToServer::Hello { protocol: PROTOCOL, game: "tally", join, resume, name, caps: 0 }.to_vec()).unwrap();
}

/// The next message on an in-process connection.
fn next_local(conn: &LocalConn) -> Vec<u8> {
    conn.recv_timeout(WAIT).unwrap().expect("a message in time")
}

/// Read an in-process connection until it closes: the reject (if any) and the close.
fn local_until_closed(conn: &LocalConn) -> (Option<Reject>, Closed) {
    let mut why = None;
    loop {
        match conn.recv_timeout(WAIT) {
            Ok(Some(b)) => {
                if let Ok(ToClient::Reject { why: w, .. }) = ToClient::decode(&b) {
                    why = Some(w);
                }
            }
            Ok(None) => panic!("the connection stayed open"),
            Err(closed) => return (why, closed),
        }
    }
}

fn local_join(server: &Server, resume: &str, name: &str) -> (LocalConn, Option<u32>, String) {
    let conn = server.connect_local().unwrap();
    local_hello(&conn, "", resume, name);
    match ToClient::decode(&next_local(&conn)) {
        Ok(ToClient::Welcome(w)) => {
            let (seat, token) = (w.seat, w.seat_token.to_string());
            (conn, seat, token)
        }
        other => panic!("not a welcome: {other:?}"),
    }
}

/// The newest whole tree an in-process client holds after `wait`.
fn local_tree(conn: &LocalConn, wait: Duration) -> Value {
    let mut reader = Reader::default();
    let mut tree = Value::Unit;
    let until = Instant::now() + wait;
    while Instant::now() < until {
        let Ok(Some(msg)) = conn.recv_timeout(Duration::from_millis(50)) else { continue };
        if let Ok(ToClient::Snapshot { whole, body, .. }) = ToClient::decode(&msg) {
            if whole {
                reader = Reader::default();
                tree = reader.value(&mut Bytes::new(body)).unwrap();
            } else if !body.is_empty() {
                apply(&mut tree, reader.patch(&mut Bytes::new(body)).unwrap()).unwrap();
            }
        }
    }
    tree
}

#[test]
fn the_host_in_process_and_a_socket_client_share_one_session() {
    let server = start();
    let (mut ws, ws_seat) = join(&server, 0);
    let (host, host_seat, _) = local_join(&server, "", "Host");
    let (ws_seat, host_seat) = (ws_seat.unwrap() as usize, host_seat.unwrap() as usize);
    assert_ne!(ws_seat, host_seat);
    // The host steps three times; the socket client once.
    host.send(ToServer::Cmds { applied: 0, cmds: vec![(1, &[1u8][..]), (2, &[1u8][..]), (3, &[1u8][..])] }.to_vec()).unwrap();
    ws.send(&ToServer::Cmds { applied: 0, cmds: vec![(1, &[1u8][..])] }.to_vec()).unwrap();
    let tree = local_tree(&host, Duration::from_millis(600));
    let Some(Value::Seq(steps)) = tree.field("steps") else { panic!("{tree:?}") };
    assert_eq!((steps[host_seat].clone(), steps[ws_seat].clone()), (Value::I(3), Value::I(1)));
    let Some(Value::Seq(names)) = tree.field("names") else { panic!() };
    assert_eq!(names[host_seat], Value::Some(Box::new(Value::Str("Host".into()))));
    assert_eq!(names[ws_seat], Value::Some(Box::new(Value::Str("Gwen".into()))));
    // The socket client heard the host join, as it hears anyone.
    let mut heard = Vec::new();
    let until = Instant::now() + Duration::from_millis(600);
    while Instant::now() < until {
        if let Ok(Some(Message::Binary(m))) = ws.recv(Duration::from_millis(50))
            && let Ok(ToClient::Events { events, .. }) = ToClient::decode(&m)
        {
            heard.extend(events.iter().map(|e| String::from_utf8_lossy(e).into_owned()));
        }
    }
    assert!(heard.contains(&"join Host".to_string()), "{heard:?}");
    let players = server.players();
    assert_eq!(players.len(), 2);
    assert!(players.iter().all(|p| p.connected));
    server.stop();
}

#[test]
fn the_host_is_let_in_while_locked_and_with_no_key_and_past_the_seats_it_watches() {
    let server = start();
    server.lock(true);
    server.new_key();
    let mut ws = WsClient::connect(&url(&server), WAIT).unwrap();
    ws.send(&hello_bytes("tally", PROTOCOL, KEY, 0)).unwrap();
    assert_eq!(until_closed(&mut ws).0, Some(Reject::Key));
    let (_a, sa, _) = local_join(&server, "", "Host");
    let (_b, sb, _) = local_join(&server, "", "Couch");
    let (_c, sc, _) = local_join(&server, "", "Third");
    assert_eq!((sa, sb, sc), (Some(0), Some(1), None));
    server.stop();
}

#[test]
fn a_local_connection_resumes_its_seat_after_closing_and_a_goodbye_frees_it() {
    let server = start();
    let (host, seat, token) = local_join(&server, "", "Host");
    host.send(ToServer::Cmds { applied: 0, cmds: vec![(1, &[1u8][..]), (2, &[1u8][..])] }.to_vec()).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    // Closed (or dropped) without a goodbye: the seat waits for its token.
    drop(host);
    let deadline = Instant::now() + WAIT;
    while server.players().iter().any(|p| p.connected) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(server.players(), vec![angel_serve::Player { seat: seat.unwrap(), name: "Host".into(), connected: false }]);
    let (again, seat_again, token_again) = local_join(&server, &token, "");
    assert_eq!((seat_again, token_again.as_str()), (seat, token.as_str()));
    // The same knight: its steps were kept.
    let tree = local_tree(&again, Duration::from_millis(400));
    let Some(Value::Seq(steps)) = tree.field("steps") else { panic!("{tree:?}") };
    assert_eq!(steps[seat.unwrap() as usize], Value::I(2));
    assert!(server.players()[0].connected);
    again.bye();
    let deadline = Instant::now() + WAIT;
    while !server.players().is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(server.players().is_empty());
    server.stop();
}

#[test]
fn a_local_connection_is_never_rate_limited_and_its_pongs_come_back() {
    let server = start();
    let (host, _, _) = local_join(&server, "", "Host");
    for i in 0..600 {
        host.send(ToServer::Ping { t: f64::from(i) }.to_vec()).unwrap();
    }
    let mut pongs = 0;
    let until = Instant::now() + Duration::from_millis(800);
    while Instant::now() < until {
        match host.recv_timeout(Duration::from_millis(50)) {
            Ok(Some(m)) => {
                assert!(!matches!(ToClient::decode(&m), Ok(ToClient::Reject { .. })), "turned away");
                if matches!(ToClient::decode(&m), Ok(ToClient::Pong { .. })) {
                    pongs += 1;
                }
            }
            Ok(None) => {}
            Err(closed) => panic!("closed: {closed:?}"),
        }
    }
    // Only the newest pong waits for the writer, so some are folded.
    assert!(pongs >= 1);
    assert!(server.players()[0].connected);
    server.stop();
}

#[test]
fn a_local_seat_taken_by_another_connection_is_told_not_to_return() {
    let server = start();
    let (first, seat, token) = local_join(&server, "", "Host");
    let (_second, seat_again, _) = local_join(&server, &token, "");
    assert_eq!(seat_again, seat);
    assert_eq!(local_until_closed(&first), (None, Closed { code: Some(code::REPLACED) }));
    server.stop();
}

#[test]
fn a_local_connection_hears_the_kick_and_the_stop_with_their_reasons() {
    let server = start();
    let (kicked, seat, token) = local_join(&server, "", "Host");
    assert!(server.kick(seat.unwrap()));
    assert_eq!(local_until_closed(&kicked), (Some(Reject::Kicked), Closed { code: Some(code::POLICY) }));
    // Its token is dead, for the host's connections as for anyone's.
    let conn = server.connect_local().unwrap();
    local_hello(&conn, "", &token, "Host");
    assert_eq!(local_until_closed(&conn).0, Some(Reject::Kicked));
    let (host, _, _) = local_join(&server, "", "Host");
    server.stop();
    assert_eq!(local_until_closed(&host), (Some(Reject::Stopping), Closed { code: Some(code::GOING_AWAY) }));
}

/// The first snapshot an in-process client is sent: its tick, and whether it is whole.
fn first_snapshot(conn: &LocalConn) -> (u32, bool, Value) {
    let until = Instant::now() + WAIT;
    while Instant::now() < until {
        let Ok(Some(msg)) = conn.recv_timeout(Duration::from_millis(50)) else { continue };
        if let Ok(ToClient::Snapshot { tick, whole, body, .. }) = ToClient::decode(&msg) {
            let tree = if whole { Reader::default().value(&mut Bytes::new(body)).unwrap() } else { Value::Unit };
            return (tick, whole, tree);
        }
    }
    panic!("no snapshot in time");
}

#[test]
fn a_newcomer_is_sent_the_newest_tick_whole_at_once_and_the_start_is_tick_one() {
    let mut config = ServeConfig::new("127.0.0.1:0".parse::<SocketAddr>().unwrap());
    config.key = Some(KEY.into());
    let (server, mut sim) = Server::start_manual(Tally::default(), config).unwrap();
    // Nothing stepped yet: the game as it starts is tick 1, sent whole.
    let (first, _, _) = local_join(&server, "", "Host");
    let (tick, whole, tree) = first_snapshot(&first);
    assert_eq!((tick, whole), (1, true));
    assert_eq!(tree.field("tick"), Some(&Value::U(1)));
    // Six ticks on, at a tick the snapshot beat sends as a patch, a
    // newcomer is sent it whole, without waiting for the beat's next.
    let step = Duration::from_secs(1) / 20;
    let start = Instant::now();
    sim.run_to(start);
    for n in 1..=6 {
        assert_eq!(sim.run_to(start + step * n), 1);
    }
    let (second, _, _) = local_join(&server, "", "Guest");
    let (tick, whole, tree) = first_snapshot(&second);
    assert_eq!((tick, whole), (7, true));
    assert_eq!(tree.field("tick"), Some(&Value::U(7)));
    drop((first, second));
    server.stop();
}

#[test]
fn a_sim_stepped_by_hand_takes_each_command_in_the_tick_after_it_was_sent() {
    let mut config = ServeConfig::new("127.0.0.1:0".parse::<SocketAddr>().unwrap());
    config.key = Some(KEY.into());
    let (server, mut sim) = Server::start_manual(Tally::default(), config).unwrap();
    let (host, seat, _) = local_join(&server, "", "Host");
    let seat = seat.unwrap() as usize;
    let step = Duration::from_secs(1) / 20;
    let start = Instant::now();
    // The beat begins at the first call; its first tick is a step later.
    assert_eq!(sim.run_to(start), 0);
    assert_eq!(sim.next_due(), Some(start + step));
    // An in-process seat has no cushion (`SeatInputs::set_cushion`).
    for n in 1..=3u32 {
        host.send(ToServer::Cmds { applied: 0, cmds: vec![(n, &[1u8][..])] }.to_vec()).unwrap();
        // Sent just before the step: the step has it, every time.
        assert_eq!(sim.run_to(start + step * n), 1);
        assert_eq!(sim.game().steps[seat], i64::from(n));
    }
    // Far behind (a laptop that slept), the beat starts over: one tick, not forty.
    assert_eq!(sim.run_to(start + step * 40), 1);
    assert_eq!(sim.next_due(), Some(start + step * 41));
    assert_eq!(sim.run_to(start + step * 43), 3, "the ticks due at 41, 42 and 43");
    drop(host);
    server.stop();
    assert_eq!(sim.run_to(start + step * 50), 0, "a stopped server steps no more");
}

#[test]
fn paintings_are_served_by_their_hash_and_nothing_else_is() {
    let dir = std::env::temp_dir().join(format!("angel-serve-paint-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let sha = "0123456789abcdef".repeat(4);
    let png = b"\x89PNG\r\n\x1a\nnot really";
    std::fs::write(dir.join(format!("{sha}.png")), png).unwrap();
    std::fs::write(dir.join("notes.txt"), b"private").unwrap();
    let mut config = ServeConfig::new("127.0.0.1:0".parse::<SocketAddr>().unwrap());
    config.key = Some(KEY.into());
    config.paint_dir = Some(dir.clone());
    let server = Server::start(Tally::default(), config).unwrap();
    let get = |path: &str| http(&server, &format!("GET {path} HTTP/1.1\r\nConnection: close\r\n\r\n"));
    let good = get(&format!("/paint/{sha}.png"));
    assert!(good.starts_with("HTTP/1.1 200 OK"), "{good}");
    assert!(good.contains("Content-Type: image/png") && good.contains("immutable"), "{good}");
    assert!(good.ends_with("not really"));
    let head = http(&server, &format!("HEAD /paint/{sha}.png HTTP/1.1\r\nConnection: close\r\n\r\n"));
    assert!(head.starts_with("HTTP/1.1 200 OK") && !head.contains("not really"));
    let again = http(&server, &format!("GET /paint/{sha}.png HTTP/1.1\r\nIf-None-Match: \"{}\"\r\nConnection: close\r\n\r\n", &sha[..16]));
    assert!(again.starts_with("HTTP/1.1 304"), "{again}");
    // A seat that joined over the socket fetches it from the same host.
    let fetch = |path: &str| angel_serve::client::http_get(&url(&server), path, &Default::default(), WAIT, 1 << 20);
    assert_eq!(fetch(&format!("/paint/{sha}.png")).unwrap(), png);
    assert!(fetch("/paint/notes.txt").is_err());
    assert!(angel_serve::client::http_get(&url(&server), &format!("/paint/{sha}.png"), &Default::default(), WAIT, 4).is_err(), "too big");
    for bad in [
        "/paint/notes.txt".to_string(),
        format!("/paint/{}.png", sha.to_uppercase()),
        format!("/paint/{}.png", &sha[1..]),
        format!("/paint/../{sha}.png"),
        format!("/paint/{sha}.png.png"),
        format!("/paint/{}.png", "f".repeat(64)),
        "/paint/".to_string(),
    ] {
        assert!(get(&bad).starts_with("HTTP/1.1 404"), "{bad}");
    }
    server.stop();
    // A server with no paint store has no paintings.
    let bare = start();
    assert!(http(&bare, &format!("GET /paint/{sha}.png HTTP/1.1\r\nConnection: close\r\n\r\n")).starts_with("HTTP/1.1 404"));
    bare.stop();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_hd_pack_serves_its_manifest_and_drawings_and_nothing_else() {
    let dir = std::env::temp_dir().join(format!("angel-serve-hd-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("knight/realm/idle")).unwrap();
    std::fs::write(dir.join("pack.json"), br#"{"art":[]}"#).unwrap();
    std::fs::write(dir.join("knight/realm/idle/seat1@64.png"), b"\x89PNG drawn").unwrap();
    std::fs::write(dir.join("notes.txt"), b"private").unwrap();
    let mut config = ServeConfig::new("127.0.0.1:0".parse::<SocketAddr>().unwrap());
    config.key = Some(KEY.into());
    config.hd_dir = Some(dir.clone());
    let server = Server::start(Tally::default(), config).unwrap();
    let get = |path: &str| http(&server, &format!("GET {path} HTTP/1.1\r\nConnection: close\r\n\r\n"));
    let manifest = get("/hd/pack.json");
    assert!(manifest.starts_with("HTTP/1.1 200 OK") && manifest.contains("application/json") && manifest.ends_with(r#"{"art":[]}"#), "{manifest}");
    let tag = manifest.lines().find_map(|l| l.strip_prefix("ETag: ")).expect("an etag").to_string();
    let same = http(&server, &format!("GET /hd/pack.json HTTP/1.1\r\nIf-None-Match: {tag}\r\nConnection: close\r\n\r\n"));
    assert!(same.starts_with("HTTP/1.1 304"), "{same}");
    let drawn = get("/hd/knight/realm/idle/seat1@64.png");
    assert!(drawn.starts_with("HTTP/1.1 200 OK") && drawn.contains("image/png") && drawn.ends_with("drawn"), "{drawn}");
    let etag = drawn.lines().find_map(|l| l.strip_prefix("ETag: ")).expect("an etag").to_string();
    let again = http(&server, &format!("GET /hd/knight/realm/idle/seat1@64.png HTTP/1.1\r\nIf-None-Match: {etag}\r\nConnection: close\r\n\r\n"));
    assert!(again.starts_with("HTTP/1.1 304"), "{again}");
    for bad in ["/hd/notes.txt", "/hd/../pack.json", "/hd/knight/../pack.json", "/hd/Knight/realm/idle/seat1@64.png", "/hd//x.png", "/hd/", "/hd/missing.png"] {
        assert!(get(bad).starts_with("HTTP/1.1 404"), "{bad}");
    }
    server.stop();
    // A server with no pack serves none.
    let bare = start();
    assert!(http(&bare, "GET /hd/pack.json HTTP/1.1\r\nConnection: close\r\n\r\n").starts_with("HTTP/1.1 404"));
    bare.stop();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn runtime_assets_can_be_added_and_replaced_while_the_same_socket_server_runs() {
    use angel_serve::http::{LiveAsset,LiveAssets};
    let files=LiveAssets::default();
    let mut config=ServeConfig::new("127.0.0.1:0".parse().unwrap());
    config.live_assets=files.clone();
    let server=Server::start(Tally::default(),config).unwrap();
    let address=server.addr();
    let old=LiveAsset::prepare(&"a".repeat(64),"wav","audio/wav",b"original sound".to_vec()).unwrap();
    files.admit(&[old.clone()]).unwrap();
    let path=format!("/a/{}",old.name);
    let fetch=|path:&str|angel_serve::client::http_get(&url(&server),path,&Default::default(),WAIT,4096).unwrap();
    assert_eq!(fetch(&path),b"original sound");
    let next=LiveAsset::prepare(&"b".repeat(64),"wav","audio/wav",b"replacement sound".to_vec()).unwrap();
    files.admit(&[next.clone()]).unwrap();
    assert_eq!(fetch(&format!("/a/{}",next.name)),b"replacement sound");
    assert_eq!(fetch(&path),b"original sound");
    assert_eq!(server.addr(),address);
    drop(old);files.retire(Instant::now());
    assert!(http(&server,&format!("GET {path} HTTP/1.1\r\nConnection: close\r\n\r\n")).starts_with("HTTP/1.1 404"));
    assert_eq!(fetch(&format!("/a/{}",next.name)),b"replacement sound");
    server.stop();
}

#[test]
fn cold_browser_generation_keeps_http_consistent_and_ws_live() {
    use angel_serve::http::{ClientBundle, PreparedAssets, LiveAsset, StaticFile};
    use std::sync::{Arc, atomic::{AtomicUsize, Ordering}, mpsc};
    let (release, held) = mpsc::channel();
    let (captured, ready) = mpsc::channel();
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let started = Instant::now();
    let static_files=Box::leak(vec![StaticFile::generated("song.txt", "text/plain", b"old sound", angel_serve::hash::sha1(b"old sound"), None)].into_boxed_slice());
    let generation = PreparedAssets::start_static(move |_| {
        count.fetch_add(1, Ordering::SeqCst);
        captured.send(()).unwrap();
        held.recv().unwrap();
        Ok(ClientBundle { index_html: "old={{asset:song.txt}}".into(), files: vec![("song.txt".into(), b"old sound".to_vec(), "text/plain".into()), ("flight.bin".into(), vec![b'z'; 4 * 1024 * 1024], "application/octet-stream".into())] })
    }, static_files).unwrap();
    assert!(started.elapsed() < Duration::from_millis(100));
    ready.recv_timeout(WAIT).unwrap();
    let mut config = ServeConfig::new("127.0.0.1:0".parse().unwrap());
    config.key = Some(KEY.into());
    config.prepared_assets = Some(generation.clone());
    let live = LiveAsset::prepare(&"a".repeat(64), "wav", "audio/wav", b"issued live sound".to_vec()).unwrap();
    config.live_assets.admit(&[live.clone()]).unwrap();
    let started = Instant::now();
    let server = Arc::new(Server::start(Tally::default(), config).unwrap());
    assert!(started.elapsed() < Duration::from_millis(100));
    let (done, completed) = mpsc::channel();
    let requests: Vec<_> = (0..6).map(|_| {
        let server = server.clone(); let done = done.clone();
        std::thread::spawn(move || { done.send(http(&server, "GET / HTTP/1.1\r\nConnection: close\r\n\r\n")).unwrap(); })
    }).collect();
    assert!(completed.recv_timeout(Duration::from_millis(50)).is_err());
    // Saturate the separate cold-page budget without filling the 512-slot
    // pre-hello pool. Extra page requests fail promptly, leaving WS admission.
    let overflow: Vec<_> = (0..34).map(|_| {
        let server = server.clone(); let done = done.clone();
        std::thread::spawn(move || { done.send(http(&server, "GET / HTTP/1.1\r\nConnection: close\r\n\r\n")).unwrap(); })
    }).collect();
    for _ in 0..24 {
        assert!(completed.recv_timeout(WAIT).unwrap().starts_with("HTTP/1.1 503"));
    }
    let before_join = Instant::now();
    let (mut ws, seat) = join(&server, 0);
    assert_eq!(seat, Some(0));
    assert!(before_join.elapsed() < Duration::from_millis(250));
    assert!(http(&server, &format!("GET /a/{} HTTP/1.1\r\nConnection: close\r\n\r\n", live.name)).ends_with("issued live sound"));
    release.send(()).unwrap();
    let page = completed.recv_timeout(WAIT).unwrap();
    assert!(page.starts_with("HTTP/1.1 200"));
    for _ in 1..16 { assert_eq!(completed.recv_timeout(WAIT).unwrap(), page); }
    for task in overflow { task.join().unwrap(); }
    for task in requests { task.join().unwrap(); }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let path = page.split("old=").nth(1).unwrap().trim();
    assert!(http(&server, &format!("GET {path} HTTP/1.1\r\nConnection: close\r\n\r\n")).ends_with("old sound"));
    let warm = Instant::now();
    for _ in 0..100 {
        assert!(http(&server, &format!("HEAD {path} HTTP/1.1\r\nConnection: close\r\n\r\n")).starts_with("HTTP/1.1 200"));
    }
    eprintln!("100 immutable warm HEAD requests {:?}", warm.elapsed());
    // A separate generation cannot change a live page's content identity.
    let mut replacement = ServeConfig::new("127.0.0.1:0".parse().unwrap());
    replacement.prepared_assets = Some(PreparedAssets::start(|_| Ok(ClientBundle { index_html: "new={{asset:song.txt}}".into(), files: vec![("song.txt".into(), b"new sound".to_vec(), "text/plain".into())] })).unwrap());
    let newer = Server::start(Tally::default(), replacement).unwrap();
    let newer_page = http(&newer, "GET / HTTP/1.1\r\nConnection: close\r\n\r\n");
    assert_ne!(newer_page.split("new=").nth(1).unwrap().trim(), path);
    assert!(http(&server, &format!("GET {path} HTTP/1.1\r\nConnection: close\r\n\r\n")).ends_with("old sound"));
    drop(newer);
    ws.close(code::NORMAL);
    drop(ws);
    let hash = angel_serve::hash::hex(&angel_serve::hash::sha1(&vec![b'z'; 4 * 1024 * 1024]));
    let mut flight = TcpStream::connect(server.addr()).unwrap();
    flight.set_read_timeout(Some(WAIT)).unwrap();
    flight.write_all(format!("GET /a/{}.bin HTTP/1.1\r\nConnection: close\r\n\r\n", &hash[..16]).as_bytes()).unwrap();
    let mut first = [0; 1024]; let n = flight.read(&mut first).unwrap();
    assert!(n > 0);
    drop(generation);
    let server = Arc::try_unwrap(server).unwrap_or_else(|_| panic!("request Arc leaked"));
    server.stop();
    let mut retained = first[..n].to_vec(); flight.read_to_end(&mut retained).unwrap();
    let body = retained.windows(4).position(|p| p == b"\r\n\r\n").unwrap() + 4;
    assert_eq!(retained.len() - body, 4 * 1024 * 1024);
    assert!(retained[body..].iter().all(|b| *b == b'z'));
    drop(flight);

    // A failed capture is retained: every page fails explicitly, never recaptured.
    let mut failed = ServeConfig::new("127.0.0.1:0".parse().unwrap());
    failed.key = Some(KEY.into());
    failed.prepared_assets = Some(PreparedAssets::start(|_| Err(std::io::Error::other("private source failure"))).unwrap());
    let server = Server::start(Tally::default(), failed).unwrap();
    for _ in 0..2 {
        let response = http(&server, "GET / HTTP/1.1\r\nConnection: close\r\n\r\n");
        assert!(response.starts_with("HTTP/1.1 503"));
        assert!(!response.contains("private source"));
    }
    let mut other_failed = ServeConfig::new("127.0.0.1:0".parse().unwrap());
    other_failed.prepared_assets = Some(PreparedAssets::start(|_| Err(std::io::Error::other("second failed source"))).unwrap());
    let other_failed = Server::start(Tally::default(), other_failed).unwrap();
    assert!(http(&other_failed, "GET / HTTP/1.1\r\nConnection: close\r\n\r\n").starts_with("HTTP/1.1 503"));
    // Both failure facts remain hosted while their resource slots permit retry.
    assert!(server.browser_failure().is_some());
    assert!(server.take_browser_failure().is_some());
    assert!(server.take_browser_failure().is_none());
    let (mut ws, seat) = join(&server, 0);
    let key = server.key();
    let addr = server.addr();
    server.retry_browser(PreparedAssets::start(|_| Ok(ClientBundle::placeholder())).unwrap()).unwrap();
    assert!(http(&server, "GET / HTTP/1.1\r\nConnection: close\r\n\r\n").starts_with("HTTP/1.1 200"));
    assert_eq!(server.addr(), addr); assert_eq!(server.key(), key);
    assert_eq!(server.players().iter().find(|p| p.seat == seat.unwrap()).unwrap().name, "Gwen");
    // The old large in-flight HTTP reader may still be leaving its existing
    // close/linger path; its successful generation remains charged until then.
    let until_retired = Instant::now() + Duration::from_secs(2);
    let retry = loop {
        match PreparedAssets::start(|_| Ok(ClientBundle::placeholder())) {
            Ok(generation) => break generation,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < until_retired); std::thread::yield_now();
            }
            Err(error) => panic!("retry admission: {error}"),
        }
    };
    other_failed.retry_browser(retry).unwrap();
    assert!(http(&other_failed, "GET / HTTP/1.1\r\nConnection: close\r\n\r\n").starts_with("HTTP/1.1 200"));
    drop(other_failed);
    // A served generation cannot be replaced: its URLs remain valid.
    assert!(server.retry_browser(PreparedAssets::start(|_| Ok(ClientBundle::placeholder())).unwrap()).is_err());
    ws.close(code::NORMAL); drop(ws); drop(server);

    // A blocked source does not make server stop join the worker. Waiting HTTP
    // is cancelled, capacity stays charged until that worker actually exits.
    let (release, held) = mpsc::channel();
    let mut pending = ServeConfig::new("127.0.0.1:0".parse().unwrap());
    pending.prepared_assets = Some(PreparedAssets::start(move |_| { held.recv().unwrap(); Ok(ClientBundle::placeholder()) }).unwrap());
    let server = Server::start(Tally::default(), pending).unwrap();
    let mut conn = TcpStream::connect(server.addr()).unwrap();
    conn.set_read_timeout(Some(WAIT)).unwrap();
    conn.write_all(b"GET / HTTP/1.1\r\nConnection: close\r\n\r\n").unwrap();
    std::thread::sleep(Duration::from_millis(30));
    let started = Instant::now();
    server.stop();
    assert!(started.elapsed() < Duration::from_millis(250));
    let mut response = String::new(); conn.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 503"));
    release.send(()).unwrap();
    eprintln!("browser generation: scheduling and server opening <100ms; held source WS join <250ms; stop {:?}", started.elapsed());
}
