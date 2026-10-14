//! `wss://` end to end: a TLS door in front of the muster, as Funnel or a
//! relay stands in front of a host, with a certificate made here. The
//! client verifies it against roots the test hands it (`Trust::Only`); the
//! public roots, and a certificate for another name, are refused.

mod common;

use angel_bots::{Headless, Muster};
use angel_net::proto::{PROTOCOL, ToClient, ToServer};
use angel_serve::client::{LinkOptions, LinkStatus, NativeLink, Trust, WsClient};
use angel_serve::ws::Message;
use common::{Clock, KEY, frames_until, start};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(5);
/// How often the door looks each way.
const DOOR_POLL: Duration = Duration::from_millis(2);

/// A TLS door to `to`, with a certificate for `localhost`: its address,
/// the certificate (DER) to trust, and its connections, to cut.
struct Door {
    addr: SocketAddr,
    cert: Vec<u8>,
    conns: Arc<Mutex<Vec<TcpStream>>>,
}

impl Door {
    fn start(to: SocketAddr) -> Door {
        let made = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let cert = made.cert.der().to_vec();
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(made.signing_key.serialize_der()));
        let config = ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![CertificateDer::from(cert.clone())], key)
            .unwrap();
        let config = Arc::new(config);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let conns: Arc<Mutex<Vec<TcpStream>>> = Arc::default();
        let held = conns.clone();
        thread::spawn(move || {
            for client in listener.incoming() {
                let Ok(client) = client else { continue };
                held.lock().unwrap().push(client.try_clone().unwrap());
                let config = config.clone();
                thread::spawn(move || terminate(config, client, to));
            }
        });
        Door { addr, cert, conns }
    }

    fn cut(&self) {
        for c in self.conns.lock().unwrap().drain(..) {
            let _ = c.shutdown(Shutdown::Both);
        }
    }
}

/// One connection through the door: TLS on the outside, plain to the server.
fn terminate(config: Arc<ServerConfig>, client: TcpStream, to: SocketAddr) {
    let Ok(mut server) = TcpStream::connect(to) else { return };
    client.set_read_timeout(Some(DOOR_POLL)).unwrap();
    server.set_read_timeout(Some(DOOR_POLL)).unwrap();
    let mut tls = StreamOwned::new(ServerConnection::new(config).unwrap(), client);
    let mut buf = vec![0u8; 16 * 1024];
    let waiting = |e: &std::io::Error| matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut);
    loop {
        match tls.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if server.write_all(&buf[..n]).is_err() {
                    break;
                }
            }
            Err(e) if waiting(&e) => {}
            Err(_) => break,
        }
        match server.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if tls.write_all(&buf[..n]).and_then(|_| tls.flush()).is_err() {
                    break;
                }
            }
            Err(e) if waiting(&e) => {}
            Err(_) => break,
        }
    }
    let _ = server.shutdown(Shutdown::Both);
    let _ = tls.sock.shutdown(Shutdown::Both);
}

fn hello() -> Vec<u8> {
    ToServer::Hello { protocol: PROTOCOL, game: "muster", join: KEY, resume: "", name: "Gwen", caps: 0 }.to_vec()
}

#[test]
fn a_wss_socket_joins_through_a_tls_door_whose_certificate_it_trusts() {
    let server = start();
    let door = Door::start(server.addr());
    let url = format!("wss://localhost:{}/ws", door.addr.port());
    let mut ws = WsClient::connect_trusting(&url, WAIT, &Trust::Only(vec![door.cert.clone()])).unwrap();
    ws.send(&hello()).unwrap();
    let Some(Message::Binary(first)) = ws.recv(WAIT).unwrap() else { panic!("no answer") };
    assert!(matches!(ToClient::decode(&first), Ok(ToClient::Welcome(w)) if w.seat.is_some()));
    // Snapshots follow, a whole one first, and pings are answered under TLS.
    let mut snapshots = 0;
    while snapshots < 10 {
        if let Some(Message::Binary(b)) = ws.recv(WAIT).unwrap()
            && matches!(ToClient::decode(&b), Ok(ToClient::Snapshot { .. }))
        {
            snapshots += 1;
        }
    }
    assert!(ws.read_bytes > 0);
    server.stop();
}

#[test]
fn certificates_are_verified_against_the_roots_and_the_name() {
    let server = start();
    let door = Door::start(server.addr());
    // The public roots do not know a certificate made here.
    let url = format!("wss://localhost:{}/ws", door.addr.port());
    let refused = WsClient::connect_trusting(&url, WAIT, &Trust::Public).err().expect("refused");
    assert!(refused.to_string().contains("UnknownIssuer"), "{refused}");
    // A trusted certificate for another name: the link's host is checked.
    let by_ip = format!("wss://127.0.0.1:{}/ws", door.addr.port());
    let refused = WsClient::connect_trusting(&by_ip, WAIT, &Trust::Only(vec![door.cert.clone()])).err().expect("refused");
    assert!(refused.to_string().contains("not valid for name"), "{refused}");
    // And plain ws:// to a TLS door does not get through.
    assert!(WsClient::connect(&format!("ws://localhost:{}/ws", door.addr.port()), Duration::from_secs(2)).is_err());
    server.stop();
}

#[test]
fn a_native_link_plays_over_an_https_link_and_comes_back_through_it() {
    let server = start();
    let door = Door::start(server.addr());
    let clock = Clock::new();
    let link = format!("https://localhost:{}/#{KEY}", door.addr.port());
    let options = LinkOptions { trust: Trust::Only(vec![door.cert.clone()]), ..LinkOptions::default() };
    let mut native = NativeLink::<Headless>::connect(&link, "Gwen", options).unwrap();
    let playing = |l: &NativeLink<Headless>| *l.status() == LinkStatus::Playing && l.runtime().seat().is_some();
    assert!(frames_until(&mut native, &clock, WAIT, playing), "{:?}", native.status());
    let seat = native.runtime().seat();
    // Commands go up through TLS and are answered.
    native.hands().stick(Some((127, 0)));
    assert!(frames_until(&mut native, &clock, WAIT, |l| {
        let m = l.runtime().game().unwrap().muster();
        m.mirror().get(m.seat().unwrap()).is_some() && l.runtime().rtt_ms().is_some()
    }));
    door.cut();
    assert!(frames_until(&mut native, &clock, WAIT, |l| *l.status() == LinkStatus::Reconnecting));
    assert!(frames_until(&mut native, &clock, WAIT, playing));
    assert_eq!((native.runtime().seat(), native.reconnects()), (seat, 1));
    native.bye();
    server.stop();
}

/// A public `wss://` server, by hand: `cargo test -p angel-bots --test tls
/// -- --ignored`. It needs the network, so it is never part of a run. The
/// certificate verifies against the compiled-in roots and the upgrade is
/// accepted; with no roots at all it is refused. (Public echo servers
/// greet in text, which the game's binary-only protocol refuses, so the
/// echo itself is not checked.)
#[test]
#[ignore]
fn a_public_wss_server_verifies_against_the_compiled_in_roots() {
    let url = "wss://echo.websocket.org/";
    let mut ws = WsClient::connect(url, Duration::from_secs(10)).unwrap();
    ws.send(b"angel").unwrap();
    ws.close(angel_serve::ws::code::NORMAL);
    let refused = WsClient::connect_trusting(url, Duration::from_secs(10), &Trust::Only(Vec::new())).err().expect("refused");
    assert!(refused.to_string().contains("UnknownIssuer"), "{refused}");
}
