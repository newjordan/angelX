//! The whole engine end to end: a muster server on a real socket, twenty
//! real WebSocket clients each running the runtime a browser runs, and
//! afterwards a look inside every client's mirror.

use angel_bots::{BotConfig, Headless, Report, run};
use angel_muster::MusterServer;
use angel_muster::rules::{FIELD, KIND_KNIGHT, Knight, MARGIN_PX, SUB, VIEW_H, VIEW_W};
use angel_net::client::Runtime;
use angel_serve::{ServeConfig, Server};
use std::net::SocketAddr;
use std::thread;
use std::time::{Duration, Instant};

const KEY: &str = "MUSTERTESTKEYMUSTERTESTKEY";
const CLIENTS: u32 = 20;

fn start() -> Server {
    let mut config = ServeConfig::new("127.0.0.1:0".parse::<SocketAddr>().unwrap());
    config.key = Some(KEY.into());
    Server::start(MusterServer::new(60), config).expect("the server starts")
}

fn bot(server: &Server, i: u32) -> BotConfig {
    BotConfig {
        url: format!("ws://{}/ws", server.addr()),
        join: KEY.into(),
        name: format!("Knight {i}"),
        seed: u64::from(i) + 1,
        wander: false,
        slow: false,
        fps: 30,
        warmup: Duration::from_millis(500),
        stray: angel_bots::STRAY,
    }
}

#[test]
fn twenty_clients_on_real_sockets_each_see_everyone_near_them() {
    let server = start();
    let until = Instant::now() + Duration::from_millis(3000);
    let bots: Vec<_> = (0..CLIENTS)
        .map(|i| {
            let cfg = bot(&server, i);
            thread::spawn(move || run::<Headless>(&cfg, until))
        })
        .collect();
    let results: Vec<(Report, Runtime<Headless>)> = bots.into_iter().map(|b| b.join().unwrap()).collect();

    for (report, _) in &results {
        assert!(report.error.is_none() && report.rejected.is_none(), "{report:?}");
        assert!(report.seat.is_some());
        assert!(report.snapshots > 20, "{} snapshots", report.snapshots);
        assert!(!report.rtts.is_empty());
    }
    let mut seats: Vec<u32> = results.iter().map(|(r, _)| r.seat.unwrap()).collect();
    seats.sort_unstable();
    seats.dedup();
    assert_eq!(seats.len(), CLIENTS as usize);

    let truth: Vec<(u32, Knight)> = results
        .iter()
        .map(|(r, rt)| {
            let seat = r.seat.unwrap();
            let mirror = rt.game().unwrap().0.mirror();
            (seat, Knight::from_state(mirror.get(seat).expect("each client holds its own knight").latest()).unwrap())
        })
        .collect();
    let half_w = (VIEW_W as i32 / 2 + MARGIN_PX) * SUB;
    let half_h = (VIEW_H as i32 / 2 + MARGIN_PX) * SUB;
    let mut pairs = 0;
    for (report, rt) in &results {
        let me = report.seat.unwrap();
        let mirror = rt.game().unwrap().0.mirror();
        let mine = truth.iter().find(|(s, _)| *s == me).unwrap().1;
        for (seat, k) in &truth {
            let near = (k.x - mine.x).abs() <= half_w && (k.y - mine.y).abs() <= half_h;
            if !near {
                assert!(mirror.get(*seat).is_none(), "seat {me} holds seat {seat}, out of view");
                continue;
            }
            pairs += 1;
            let held = mirror.get(*seat).unwrap_or_else(|| panic!("seat {me} does not see seat {seat}"));
            assert_eq!(held.kind, KIND_KNIGHT);
            assert_eq!(Knight::from_state(held.latest()).unwrap(), *k, "seat {me} sees seat {seat} elsewhere");
            assert_eq!(&held.spawn[1..], format!("Knight {}", truth.iter().position(|(s, _)| s == seat).unwrap()).as_bytes());
        }
    }
    assert!(pairs > (CLIENTS * CLIENTS / 2) as usize, "{pairs} pairs in view");
    assert!(truth.iter().all(|(_, k)| (k.x - FIELD / 2).abs() < half_w));

    let deadline = Instant::now() + Duration::from_secs(5);
    while !server.players().is_empty() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(server.players().is_empty(), "everyone said goodbye: {:?}", server.players());
    server.stop();
}

#[test]
fn a_dropped_client_resumes_its_seat_and_a_bad_key_is_turned_away() {
    use angel_net::proto::{PROTOCOL, Reject, ToClient, ToServer};
    use angel_serve::client::WsClient;
    use angel_serve::ws::Message;

    let server = start();
    let url = format!("ws://{}/ws", server.addr());
    let hello = |join: &str, resume: &str| {
        let mut ws = WsClient::connect(&url, Duration::from_secs(5)).unwrap();
        ws.send(&ToServer::Hello { protocol: PROTOCOL, game: "muster", join, resume, name: "Gwen", caps: 0 }.to_vec())
            .unwrap();
        let Some(Message::Binary(first)) = ws.recv(Duration::from_secs(5)).unwrap() else { panic!("no answer") };
        (ws, first)
    };
    let (ws, first) = hello(KEY, "");
    let Ok(ToClient::Welcome(w)) = ToClient::decode(&first) else { panic!("not a welcome") };
    let (seat, token) = (w.seat.unwrap(), w.seat_token.to_string());
    drop(ws);
    thread::sleep(Duration::from_millis(300));
    assert_eq!(server.players().len(), 1);
    assert!(!server.players()[0].connected);
    let (mut again, first) = hello("", &token);
    let Ok(ToClient::Welcome(w)) = ToClient::decode(&first) else { panic!("not a welcome") };
    assert_eq!(w.seat, Some(seat));
    assert_eq!(w.seat_token, token);
    let mut got_snapshot = false;
    for _ in 0..20 {
        if let Some(Message::Binary(b)) = again.recv(Duration::from_secs(1)).unwrap()
            && matches!(ToClient::decode(&b), Ok(ToClient::Snapshot { .. }))
        {
            got_snapshot = true;
            break;
        }
    }
    assert!(got_snapshot);

    let (_bad, first) = hello("NOTTHEKEY", "");
    assert!(matches!(ToClient::decode(&first), Ok(ToClient::Reject { why: Reject::Key, .. })));

    assert!(server.kick(seat));
    let mut told = false;
    for _ in 0..40 {
        match again.recv(Duration::from_millis(100)) {
            Ok(Some(Message::Binary(b))) if matches!(ToClient::decode(&b), Ok(ToClient::Reject { why: Reject::Kicked, .. })) => {
                told = true;
                break;
            }
            Ok(Some(Message::Close(_))) | Err(_) => break,
            _ => {}
        }
    }
    assert!(told);
    let (_ws, first) = hello(KEY, &token);
    assert!(matches!(ToClient::decode(&first), Ok(ToClient::Reject { why: Reject::Kicked, .. })));

    server.lock(true);
    let (_ws, first) = hello(KEY, "");
    assert!(matches!(ToClient::decode(&first), Ok(ToClient::Reject { why: Reject::Locked, .. })));
    server.stop();
}
