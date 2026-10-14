//! The server's knobs over real sockets: the table every client is told,
//! who may set one, the game and the engine taking it between ticks, the
//! tune file a restart reads, and the numbers a panel asks for.

use angel_net::cmd::Command;
use angel_net::codec::Bytes;
use angel_net::doc::value::Value;
use angel_net::doc::wire::Reader;
use angel_net::knob::{self, Knob, Side};
use angel_net::proto::{PROTOCOL, ToClient, ToServer};
use angel_serve::client::WsClient;
use angel_serve::rep::{DocRep, DocTick, Everything};
use angel_serve::ws::Message;
use angel_serve::{Events, GameServer, Rates, ServeConfig, Server};
use std::borrow::Cow;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const KEY: &str = "DIALKEYDIALKEYDIALKEYDIAL2";
const WAIT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Nothing;

impl Command for Nothing {
    fn neutral() -> Self {
        Nothing
    }
    fn held_only(&self) -> Self {
        Nothing
    }
    fn absorb(&mut self, _older: &Self) {}
}

/// One knob, `pace`, published in the game's state as a client would read it.
struct Dial {
    pace: f64,
    /// The pace the first tick was published with (set before it, from the file).
    first: Option<f64>,
}

impl Dial {
    fn new() -> Dial {
        Dial { pace: 4.0, first: None }
    }
}

impl GameServer for Dial {
    type Cmd = Nothing;
    type Rep = DocRep<Everything>;
    const GAME: &'static str = "dial";

    fn rates(&self) -> Rates {
        Rates { tick_hz: 20, snapshot_hz: 20 }
    }
    fn seats(&self) -> u32 {
        2
    }
    fn params(&self) -> Vec<u8> {
        Vec::new()
    }
    fn replicator(&self) -> Self::Rep {
        DocRep(Everything)
    }
    fn decode_cmd(_bytes: &[u8]) -> Option<Nothing> {
        Some(Nothing)
    }
    fn join(&mut self, _seat: u32, _name: &str, _events: &mut Events) {}
    fn leave(&mut self, _seat: u32, _events: &mut Events) {}
    fn step(&mut self, _tick: u32, _cmds: &[Option<Nothing>], _events: &mut Events) {}
    fn publish(&mut self, tick: u32) -> DocTick {
        self.first.get_or_insert(self.pace);
        DocTick {
            tree: Value::Struct(vec![
                (Cow::Borrowed("tick"), Value::U(u64::from(tick))),
                (Cow::Borrowed("pace"), Value::F64(self.pace)),
                (Cow::Borrowed("first"), Value::F64(self.first.unwrap_or(0.0))),
            ]),
        }
    }
    fn knobs(&self) -> Vec<Knob> {
        vec![
            Knob::new("pace", "dial", "Pace", "PACE (tests)", Side::Server).range(1.0, 10.0, 0.5).default(4.0).at(self.pace),
            // A client's own knob is never the server's to list or set.
            Knob::new("glow", "dial", "Glow", "GLOW", Side::Client).default(1.0),
        ]
    }
    fn set_knob(&mut self, name: &str, value: f64) -> bool {
        match name {
            "pace" => {
                self.pace = value;
                true
            }
            _ => false,
        }
    }
}

fn start(dev: bool, tune_file: Option<PathBuf>) -> Server {
    let mut config = ServeConfig::new("127.0.0.1:0".parse::<SocketAddr>().unwrap());
    config.key = Some(KEY.into());
    config.dev = dev;
    config.tune_file = tune_file;
    Server::start(Dial::new(), config).unwrap()
}

/// A socket client: its welcome, then the knob table it is told.
fn join(server: &Server) -> (WsClient, String) {
    let mut ws = WsClient::connect(&format!("ws://{}/ws", server.addr()), WAIT).unwrap();
    ws.send(&ToServer::Hello { protocol: PROTOCOL, game: "dial", join: KEY, resume: "", name: "Ann", caps: 0 }.to_vec())
        .unwrap();
    assert!(matches!(ToClient::decode(&next(&mut ws)), Ok(ToClient::Welcome(_))));
    let table = match ToClient::decode(&next(&mut ws)) {
        Ok(ToClient::Knobs { json }) => json.to_string(),
        other => panic!("the knob table comes after the welcome: {other:?}"),
    };
    (ws, table)
}

fn next(ws: &mut WsClient) -> Vec<u8> {
    let until = Instant::now() + WAIT;
    while Instant::now() < until {
        if let Some(Message::Binary(b)) = ws.recv(WAIT).unwrap() {
            return b;
        }
    }
    panic!("no message in time");
}

/// Messages until one `pick` takes, skipping the rest.
fn until<T>(ws: &mut WsClient, mut pick: impl FnMut(ToClient) -> Option<T>) -> T {
    let until = Instant::now() + WAIT;
    while Instant::now() < until {
        let bytes = next(ws);
        if let Some(t) = ToClient::decode(&bytes).ok().and_then(&mut pick) {
            return t;
        }
    }
    panic!("not heard in time");
}

fn pace_in(table: &str) -> f64 {
    let at = table.find("\"name\":\"pace\"").expect("the pace knob");
    let value = &table[at..];
    let from = value.find("\"value\":").unwrap() + 8;
    value[from..].split([',', '}']).next().unwrap().parse().unwrap()
}

#[test]
fn every_client_is_told_the_table_and_only_the_host_sets_it_outside_dev_mode() {
    let server = start(false, None);
    let (mut ws, table) = join(&server);
    assert!(table.starts_with("{\"dev\":false,\"may\":false,"), "{table}");
    assert!(table.contains("\"name\":\"snapshot_every\""), "the engine's own knob: {table}");
    assert!(!table.contains("glow"), "a client knob is not the server's: {table}");
    assert_eq!(pace_in(&table), 4.0);
    let names: Vec<_> = server.knobs().iter().map(|k| k.name).collect();
    assert_eq!(names, ["snapshot_every", "pace"]);

    // A guest's tune is refused, said so, and told the table again.
    ws.send(&ToServer::Action { kind: knob::action::TUNE, body: &knob::tune_body("pace", 9.0) }.to_vec()).unwrap();
    let said = until(&mut ws, |m| match m {
        ToClient::Notice { text } => Some(text.to_string()),
        _ => None,
    });
    assert!(said.contains("only the host"), "{said}");
    let again = until(&mut ws, |m| match m {
        ToClient::Knobs { json } => Some(json.to_string()),
        _ => None,
    });
    assert_eq!(pace_in(&again), 4.0);

    // The host sets it, on its step; everyone is told.
    assert_eq!(server.tune("pace", 7.3), Ok(7.5));
    assert!(server.tune("nope", 1.0).unwrap_err().contains("no knob nope"));
    assert!(server.tune("glow", 2.0).is_err(), "a client knob is no server knob");
    let told = until(&mut ws, |m| match m {
        ToClient::Knobs { json } => Some(json.to_string()),
        _ => None,
    });
    assert_eq!(pace_in(&told), 7.5);
    // ...and the game took it: a newcomer's first tick carries it.
    let (mut late, _) = join(&server);
    let tree = until(&mut late, |m| match m {
        ToClient::Snapshot { whole: true, body, .. } => Reader::default().value(&mut Bytes::new(body)).ok(),
        _ => None,
    });
    assert_eq!(tree.field("pace"), Some(&Value::F64(7.5)));
    server.stop();
}

#[test]
fn in_dev_mode_any_seat_sets_a_knob_and_the_host_connection_always_may() {
    let server = start(true, None);
    let (mut ws, table) = join(&server);
    assert!(table.starts_with("{\"dev\":true,\"may\":true,"), "{table}");
    ws.send(&ToServer::Action { kind: knob::action::TUNE, body: &knob::tune_body("pace", 2.0) }.to_vec()).unwrap();
    let told = until(&mut ws, |m| match m {
        ToClient::Knobs { json } => Some(json.to_string()),
        _ => None,
    });
    assert_eq!(pace_in(&told), 2.0);
    server.stop();

    // Outside dev mode the host's own in-process connection may.
    let server = start(false, None);
    let local = server.connect_local().unwrap();
    local.send(ToServer::Hello { protocol: PROTOCOL, game: "dial", join: "", resume: "", name: "Host", caps: 0 }.to_vec()).unwrap();
    let mut host_table = String::new();
    let deadline = Instant::now() + WAIT;
    while host_table.is_empty() && Instant::now() < deadline {
        if let Ok(Some(b)) = local.recv_timeout(WAIT)
            && let Ok(ToClient::Knobs { json }) = ToClient::decode(&b)
        {
            host_table = json.to_string();
        }
    }
    assert!(host_table.starts_with("{\"dev\":false,\"may\":true,"), "{host_table}");
    local.send(ToServer::Action { kind: knob::action::TUNE, body: &knob::tune_body("pace", 6.0) }.to_vec()).unwrap();
    let deadline = Instant::now() + WAIT;
    while server.knobs().iter().find(|k| k.name == "pace").unwrap().value != 6.0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(server.knobs().iter().find(|k| k.name == "pace").unwrap().value, 6.0);
    server.stop();
}

#[test]
fn a_tune_file_keeps_what_moved_and_a_restart_starts_with_it() {
    let dir = std::env::temp_dir().join(format!("angel-serve-tune-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let file = dir.join("tune.json");
    let server = start(false, Some(file.clone()));
    server.tune("pace", 8.0).unwrap();
    server.tune("snapshot_every", 2.0).unwrap();
    let deadline = Instant::now() + WAIT;
    let mut text = String::new();
    while !text.contains("\"snapshot_every\": 2") && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
        text = std::fs::read_to_string(&file).unwrap_or_default();
    }
    assert_eq!(text, "{\n  \"snapshot_every\": 2,\n  \"pace\": 8\n}\n");
    server.stop();

    let server = start(false, Some(file.clone()));
    let pace = server.knobs().into_iter().find(|k| k.name == "pace").unwrap();
    assert_eq!((pace.value, pace.default), (8.0, 4.0));
    // The game had it before its first tick went out.
    let (mut ws, _) = join(&server);
    let first = until(&mut ws, |m| match m {
        ToClient::Snapshot { whole: true, body, .. } => Reader::default().value(&mut Bytes::new(body)).ok(),
        _ => None,
    });
    assert_eq!(first.field("first"), Some(&Value::F64(8.0)));
    // Snapshots two ticks apart, as kept.
    let mut ticks = Vec::new();
    while ticks.len() < 4 {
        if let Ok(ToClient::Snapshot { tick, .. }) = ToClient::decode(&next(&mut ws)) {
            ticks.push(tick);
        }
    }
    assert!(ticks.windows(2).all(|w| w[1] - w[0] == 2), "{ticks:?}");
    server.stop();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_panel_that_asks_hears_the_servers_numbers_each_second() {
    let server = start(false, None);
    let (mut ws, _) = join(&server);
    ws.send(&ToServer::Action { kind: knob::action::STATS, body: &[1] }.to_vec()).unwrap();
    let stats = until(&mut ws, |m| match m {
        ToClient::Stats { json } => Some(json.to_string()),
        _ => None,
    });
    for field in ["\"tick_ms\":", "\"tick_ms_max\":", "\"snapshot_every\":1", "\"seat\":0", "\"queued\":", "\"repeated\":", "\"skipped\":"] {
        assert!(stats.contains(field), "{field} in {stats}");
    }
    server.stop();
}
