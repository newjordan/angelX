//! Who is here (`GameServer::presence`): a socket's seat is here once its
//! client has applied the tick it was welcomed at and its commands come,
//! and away a second after they stop; the host's in-process seat is here
//! while connected.

use angel_net::cmd::Command;
use angel_net::codec::Bytes;
use angel_net::doc::delta::apply;
use angel_net::doc::value::Value;
use angel_net::doc::wire::Reader;
use angel_net::proto::{PROTOCOL, ToClient, ToServer};
use angel_serve::client::WsClient;
use angel_serve::rep::{DocRep, DocTick, Everything};
use angel_serve::ws::Message;
use angel_serve::{AWAY_AFTER_TICKS, Events, GameServer, Presence, Rates, ServeConfig, Server};
use std::borrow::Cow;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

const KEY: &str = "PRESENCEPRESENCEPRESENCE22";
const HZ: u32 = 30;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Nothing;

impl Command for Nothing {
    fn neutral() -> Self {
        Nothing
    }
    fn held_only(&self) -> Self {
        Nothing
    }
    fn absorb(&mut self, _: &Self) {}
}

/// Two seats; the tree is who is here.
#[derive(Default)]
struct Here {
    here: Vec<Presence>,
}

impl GameServer for Here {
    type Cmd = Nothing;
    type Rep = DocRep<Everything>;
    const GAME: &'static str = "here";

    fn rates(&self) -> Rates {
        Rates { tick_hz: HZ, snapshot_hz: HZ }
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
    fn decode_cmd(_: &[u8]) -> Option<Nothing> {
        Some(Nothing)
    }
    fn join(&mut self, _: u32, _: &str, _: &mut Events) {}
    fn leave(&mut self, _: u32, _: &mut Events) {}
    fn presence(&mut self, _tick: u32, seats: &[Presence]) {
        self.here = seats.to_vec();
    }
    fn step(&mut self, _: u32, _: &[Option<Nothing>], _: &mut Events) {}
    fn publish(&mut self, _tick: u32) -> DocTick {
        let flag = |p: &Presence| Value::Seq(vec![Value::Bool(p.seated), Value::Bool(p.connected), Value::Bool(p.playing)]);
        DocTick {
            tree: Value::Struct(vec![(Cow::Borrowed("here"), Value::Seq(self.here.iter().map(flag).collect()))]),
        }
    }
}

/// A page: seated over a socket, the tree read back, commands when told.
struct Page {
    ws: WsClient,
    reader: Reader,
    tree: Option<Value>,
    newest: u32,
    seq: u32,
}

impl Page {
    fn open(server: &Server) -> Page {
        let mut ws = WsClient::connect(&format!("ws://{}/ws", server.addr()), Duration::from_secs(5)).unwrap();
        ws.send(&ToServer::Hello { protocol: PROTOCOL, game: "here", join: KEY, resume: "", name: "Page", caps: 0 }.to_vec())
            .unwrap();
        Page { ws, reader: Reader::default(), tree: None, newest: 0, seq: 0 }
    }

    /// For `wait`: read what comes, sending a command a tick if `sending`.
    fn run(&mut self, wait: Duration, sending: bool) {
        let until = Instant::now() + wait;
        let mut next = Instant::now();
        while Instant::now() < until {
            if sending && Instant::now() >= next {
                self.seq += 1;
                let cmd = ToServer::Cmds { applied: self.newest, cmds: vec![(self.seq, &[][..])] };
                self.ws.send(&cmd.to_vec()).unwrap();
                next += Duration::from_secs(1) / HZ;
            }
            let Ok(Some(Message::Binary(bytes))) = self.ws.recv(Duration::from_millis(5)) else {
                continue;
            };
            if let Ok(ToClient::Snapshot { tick, whole, body, .. }) = ToClient::decode(&bytes) {
                if whole {
                    self.reader = Reader::default();
                    self.tree = Some(self.reader.value(&mut Bytes::new(body)).unwrap());
                } else if !body.is_empty() {
                    apply(self.tree.as_mut().unwrap(), self.reader.patch(&mut Bytes::new(body)).unwrap()).unwrap();
                }
                self.newest = tick;
            }
        }
    }

    /// Seat `seat`'s (seated, connected, playing), as the tree says.
    fn here(&self, seat: usize) -> (bool, bool, bool) {
        let Some(Value::Struct(fields)) = &self.tree else {
            return (false, false, false);
        };
        let Value::Seq(seats) = &fields[0].1 else { panic!("the seats") };
        let Value::Seq(flags) = &seats[seat] else { panic!("a seat") };
        let flag = |i: usize| matches!(flags[i], Value::Bool(true));
        (flag(0), flag(1), flag(2))
    }
}

#[test]
fn a_page_is_here_once_it_plays_and_away_a_second_after_it_stops() {
    let mut config = ServeConfig::new("127.0.0.1:0".parse::<SocketAddr>().unwrap());
    config.key = Some(KEY.into());
    let server = Server::start(Here::default(), config).unwrap();
    let mut page = Page::open(&server);
    // Seated and connected, still loading: it has said nothing more.
    page.run(Duration::from_millis(500), false);
    assert_eq!(page.here(0), (true, true, false), "a page still loading is not here");
    // It has the game and plays.
    page.run(Duration::from_millis(300), true);
    assert_eq!(page.here(0), (true, true, true), "playing");
    // Its commands stop (a hidden tab): away a second later, still seated.
    let quiet = Duration::from_secs(1) * AWAY_AFTER_TICKS / HZ + Duration::from_millis(300);
    page.run(quiet, false);
    assert_eq!(page.here(0), (true, true, false), "away while quiet");
    page.run(Duration::from_millis(300), true);
    assert_eq!(page.here(0), (true, true, true), "back the moment it plays");
    // The host's own in-process seat is here as soon as it is seated.
    let host = server.connect_local().unwrap();
    host.send(ToServer::Hello { protocol: PROTOCOL, game: "here", join: KEY, resume: "", name: "Host", caps: 0 }.to_vec())
        .unwrap();
    page.run(Duration::from_millis(300), true);
    assert_eq!(page.here(1), (true, true, true), "the host's own seat loads nothing");
    server.stop();
}
