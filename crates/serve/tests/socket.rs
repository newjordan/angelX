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
    let mut why = None;
    let until = Instant::now() + 2 * WAIT;
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
    for _ in 0..300 {
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
    let (why, code) = until_closed(&mut ws);
    assert_eq!((why, code), (Some(Reject::Abuse), Some(ws::code::POLICY)));
    let waited = started.elapsed();
    assert!(waited >= Duration::from_millis(4900) && waited < Duration::from_secs(7), "{waited:?}");
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
