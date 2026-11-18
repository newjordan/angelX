//! What the native link's tests share: a muster to join, a terminal's frame
//! loop, and a relay between client and server that can cut or stall what
//! goes through it.

#![allow(dead_code)]

use angel_muster::MusterServer;
use angel_net::client::GameClient;
use angel_serve::client::NativeLink;
use angel_serve::{ServeConfig, Server};
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub const KEY: &str = "LINKTESTKEYLINKTESTKEYLINK";
pub const FRAME: Duration = Duration::from_millis(16);

pub fn start() -> Server {
    let mut config = ServeConfig::new("127.0.0.1:0".parse::<SocketAddr>().unwrap());
    config.key = Some(KEY.into());
    Server::start(MusterServer::new(10), config).expect("the server starts")
}

pub fn share(addr: SocketAddr) -> String {
    format!("http://{addr}/#{KEY}")
}

pub struct Clock(Instant);

impl Clock {
    pub fn new() -> Clock {
        Clock(Instant::now())
    }

    pub fn ms(&self) -> f64 {
        self.0.elapsed().as_secs_f64() * 1000.0
    }
}

pub fn frames_until<G: GameClient>(
    link: &mut NativeLink<G>,
    clock: &Clock,
    timeout: Duration,
    mut done: impl FnMut(&NativeLink<G>) -> bool,
) -> bool {
    let until = Instant::now() + timeout;
    while Instant::now() < until {
        link.frame(clock.ms());
        if done(link) {
            return true;
        }
        thread::sleep(FRAME);
    }
    false
}

pub fn frames_for<G: GameClient>(link: &mut NativeLink<G>, clock: &Clock, span: Duration) {
    frames_until(link, clock, span, |_| false);
}

pub struct Relay {
    pub addr: SocketAddr,
    conns: Arc<Mutex<Vec<Relayed>>>,
}

struct Relayed {
    client: TcpStream,
    server: TcpStream,
    stalled: Arc<AtomicBool>,
}

impl Relay {
    pub fn start(to: SocketAddr) -> Relay {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let conns: Arc<Mutex<Vec<Relayed>>> = Arc::default();
        let held = conns.clone();
        thread::spawn(move || {
            for client in listener.incoming() {
                let Ok(client) = client else { continue };
                let Ok(server) = TcpStream::connect(to) else { continue };
                let stalled = Arc::new(AtomicBool::new(false));
                let (c, s) = (client.try_clone().unwrap(), server.try_clone().unwrap());
                held.lock().unwrap().push(Relayed { client: c, server: s, stalled: stalled.clone() });
                carry(client.try_clone().unwrap(), server.try_clone().unwrap(), stalled.clone());
                carry(server, client, stalled);
            }
        });
        Relay { addr, conns }
    }

    pub fn cut(&self) {
        for r in self.conns.lock().unwrap().drain(..) {
            let _ = r.client.shutdown(Shutdown::Both);
            let _ = r.server.shutdown(Shutdown::Both);
        }
    }

    pub fn stall(&self) {
        for r in self.conns.lock().unwrap().iter() {
            r.stalled.store(true, SeqCst);
        }
    }
}

fn carry(mut from: TcpStream, mut to: TcpStream, stalled: Arc<AtomicBool>) {
    thread::spawn(move || {
        let mut buf = [0u8; 16 * 1024];
        loop {
            match from.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) if stalled.load(SeqCst) => {}
                Ok(n) => {
                    if to.write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
            }
        }
        let _ = to.shutdown(Shutdown::Write);
    });
}
