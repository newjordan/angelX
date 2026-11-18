//! `NativeLink`: a game played from a terminal, over a socket to a host
//! (`ws://`, or `wss://` through Funnel or a relay) or over an in-process
//! `LocalConn` to this one, by one code path. The terminal's thread owns
//! the game's `Runtime` and calls `frame` once a display frame. A socket is
//! dialed, read and written on threads of its own, so nothing here blocks
//! the thread that draws; a `LocalConn` needs no threads, since sending to
//! it and taking from it never block.
//!
//! The link keeps the page's rules (`crates/web/shell/boot.js`):
//!
//! - a lost link is dialed again after 0.5, 1, 2 and 4 s, then every 5 s,
//!   resuming the seat with the token from its welcome;
//! - 3 s with nothing heard is a dead link, even if the socket has not
//!   noticed (a network change, a laptop that slept);
//! - a `Reject` stops it, and says why; a stale seat token (`Reject::Key`)
//!   is dropped and the link joins fresh at once;
//! - close code 4000 (another connection took the seat) stops it: retrying
//!   would take the seat back, and the two would pass it between them.
//!
//! Each message reaches the runtime with the time it arrived, not the time
//! the frame looked, so the server clock and the round trip are not off by
//! up to a frame.

use super::{Trust, WsClient, share_link};
use crate::limits::CLOSE_WAIT;
use crate::local::{Closed, Dial, LocalConn};
use crate::ws::{BINARY, Message, PONG, code};
use angel_net::client::{Frame, GameClient, Runtime, Status};
use angel_net::input::Sampler;
use angel_net::proto::{Reject, caps};
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const BACKOFF_MS: [f64; 5] = [500.0, 1000.0, 2000.0, 4000.0, 5000.0];
const SILENCE_MS: f64 = 3000.0;
const DIAL_TIMEOUT: Duration = Duration::from_secs(10);
const READ_POLL: Duration = Duration::from_millis(250);

#[derive(Clone, Debug, Default)]
pub struct LinkOptions {
    pub resume: String,
    pub caps: u32,
    pub trust: Trust,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LinkStatus {
    Connecting,
    Playing,
    Reconnecting,
    Rejected(Reject, String),
    Replaced,
    Closed,
}

enum Door {
    Remote { url: String, trust: Trust },
    Local(Arc<dyn Dial>),
}

enum Said {
    Open(Instant),
    Message(Vec<u8>, Instant),
    Heard(Instant),
    Closed(Option<u16>),
}

enum Line {
    Remote(RemoteLine),
    Local(LocalLine),
}

impl Line {
    fn poll(&mut self) -> Option<Said> {
        match self {
            Line::Remote(line) => line.poll(),
            Line::Local(line) => line.poll(),
        }
    }

    fn send(&self, msg: Vec<u8>) {
        match self {
            Line::Remote(line) => line.send(msg),
            Line::Local(line) => {
                let _ = line.conn.send(msg);
            }
        }
    }

    fn goodbye(self) {
        match self {
            Line::Remote(line) => line.goodbye(),
            Line::Local(line) => line.conn.close(),
        }
    }
}

pub struct NativeLink<G: GameClient> {
    rt: Runtime<G>,
    door: Door,
    join: String,
    name: String,
    caps: u32,
    token: String,
    line: Option<Line>,
    heard_ms: Option<f64>,
    status: LinkStatus,
    attempt: usize,
    retry_at_ms: Option<f64>,
    reconnects: u32,
    bytes_in: u64,
    bytes_out: u64,
    tap: Option<Tap>,
}

type Tap = Box<dyn FnMut(&[u8], f64) + Send>;

impl<G: GameClient> NativeLink<G> {
    pub fn connect(link: &str, name: &str, options: LinkOptions) -> Result<NativeLink<G>, String> {
        let (url, key) = share_link(link)?;
        if url.starts_with("wss://") && !cfg!(feature = "tls") {
            return Err("this build cannot open https:// links (angel-serve's `tls` feature is off)".into());
        }
        let trust = options.trust.clone();
        let mut link = NativeLink::new(Door::Remote { url: url.clone(), trust: trust.clone() }, key, name, options);
        let hello = link.hello();
        link.open(Line::Remote(RemoteLine::dial(url, trust)), hello);
        Ok(link)
    }

    pub fn local(conn: LocalConn, name: &str, options: LinkOptions) -> NativeLink<G> {
        let mut link = NativeLink::new(Door::Local(conn.door()), String::new(), name, options);
        let hello = link.hello();
        link.open(Line::Local(LocalLine::new(conn)), hello);
        link
    }

    fn new(door: Door, join: String, name: &str, options: LinkOptions) -> NativeLink<G> {
        NativeLink {
            rt: Runtime::new(),
            door,
            join,
            name: name.to_string(),
            caps: options.caps | caps::NATIVE,
            token: options.resume,
            line: None,
            heard_ms: None,
            status: LinkStatus::Connecting,
            attempt: 0,
            retry_at_ms: None,
            reconnects: 0,
            bytes_in: 0,
            bytes_out: 0,
            tap: None,
        }
    }

    pub fn frame(&mut self, now_ms: f64) -> &Frame {
        self.pump(now_ms);
        self.rt.frame(now_ms);
        self.flush();
        self.rt.last_frame()
    }

    pub fn poll(&mut self, now_ms: f64) {
        self.pump(now_ms);
    }

    pub fn tick(&mut self, now_ms: f64) {
        self.pump(now_ms);
        self.rt.tick(now_ms);
        self.flush();
    }

    pub fn idle(&mut self, now_ms: f64) {
        self.pump(now_ms);
        self.rt.idle(now_ms);
        self.flush();
    }

    pub fn draw(&mut self, now_ms: f64) -> &Frame {
        self.pump(now_ms);
        self.rt.draw(now_ms);
        self.flush();
        self.rt.last_frame()
    }

    pub fn hands(&mut self) -> &mut Sampler {
        self.rt.hands()
    }

    pub fn release_all(&mut self, now_ms: f64) {
        self.rt.hands().release_all();
        self.rt.command_now(now_ms);
        self.flush();
    }

    pub fn action(&mut self, kind: u32, body: &[u8]) {
        self.rt.action(kind, body);
        self.flush();
    }

    pub fn bye(&mut self) {
        if !self.trying() {
            return;
        }
        self.rt.bye();
        self.flush();
        if let Some(line) = self.line.take() {
            line.goodbye();
        }
        self.stop(LinkStatus::Closed);
    }

    pub fn close(&mut self) {
        self.stop(LinkStatus::Closed);
    }

    pub fn remote(&self) -> Option<(&str, &Trust)> {
        match &self.door {
            Door::Remote { url, trust } => Some((url, trust)),
            Door::Local(_) => None,
        }
    }

    pub fn status(&self) -> &LinkStatus {
        &self.status
    }

    pub fn set_knob(&mut self, name: &str, value: f64) -> Option<f64> {
        self.rt.set_knob(name, value)
    }

    pub fn runtime(&self) -> &Runtime<G> {
        &self.rt
    }

    pub fn take_lines(&mut self) -> Vec<String> {
        self.rt.take_lines()
    }

    pub fn seat_token(&self) -> &str {
        &self.token
    }

    pub fn reconnects(&self) -> u32 {
        self.reconnects
    }

    pub fn retry_at_ms(&self) -> Option<f64> {
        self.retry_at_ms
    }

    pub fn bytes(&self) -> (u64, u64) {
        (self.bytes_in, self.bytes_out)
    }

    pub fn on_message(&mut self, tap: impl FnMut(&[u8], f64) + Send + 'static) {
        self.tap = Some(Box::new(tap));
    }

    fn trying(&self) -> bool {
        matches!(self.status, LinkStatus::Connecting | LinkStatus::Playing | LinkStatus::Reconnecting)
    }

    fn stop(&mut self, status: LinkStatus) {
        self.status = status;
        self.line = None;
        self.heard_ms = None;
        self.retry_at_ms = None;
    }

    fn hello(&mut self) -> Vec<Vec<u8>> {
        self.rt.connected(&self.join, &self.token, &self.name, self.caps);
        self.rt.take_outgoing()
    }

    fn open(&mut self, line: Line, hello: Vec<Vec<u8>>) {
        for msg in hello {
            self.bytes_out += msg.len() as u64;
            line.send(msg);
        }
        self.line = Some(line);
    }

    fn dial(&mut self, now_ms: f64) {
        self.retry_at_ms = None;
        self.heard_ms = None;
        let hello = self.hello();
        match &self.door {
            Door::Remote { url, trust } => {
                let line = Line::Remote(RemoteLine::dial(url.clone(), trust.clone()));
                self.open(line, hello);
            }
            Door::Local(door) => match door.clone().dial() {
                Ok(conn) => self.open(Line::Local(LocalLine::new(conn)), hello),
                Err(_) => self.lost(None, now_ms),
            },
        }
    }

    fn lost(&mut self, close: Option<u16>, now_ms: f64) {
        self.line = None;
        self.heard_ms = None;
        if !self.trying() {
            return;
        }
        if close == Some(code::REPLACED) {
            self.stop(LinkStatus::Replaced);
            return;
        }
        let wait = BACKOFF_MS[self.attempt.min(BACKOFF_MS.len() - 1)];
        self.attempt += 1;
        self.reconnects += 1;
        self.retry_at_ms = Some(now_ms + wait);
        self.status = LinkStatus::Reconnecting;
    }

    fn heard(&mut self, now_ms: f64, at: Instant) -> f64 {
        let at_ms = now_ms - at.elapsed().as_secs_f64() * 1000.0;
        self.heard_ms = Some(self.heard_ms.map_or(at_ms, |h| h.max(at_ms)));
        at_ms
    }

    fn pump(&mut self, now_ms: f64) {
        while let Some(said) = self.line.as_mut().and_then(Line::poll) {
            match said {
                Said::Open(at) | Said::Heard(at) => {
                    self.heard(now_ms, at);
                }
                Said::Message(bytes, at) => {
                    let at_ms = self.heard(now_ms, at);
                    self.bytes_in += bytes.len() as u64;
                    if let Some(tap) = self.tap.as_mut() {
                        tap(&bytes, at_ms);
                    }
                    let _ = self.rt.recv(&bytes, at_ms);
                    self.watch(now_ms);
                }
                Said::Closed(close) => self.lost(close, now_ms),
            }
        }
        if self.line.is_some() && self.heard_ms.is_some_and(|heard| now_ms - heard > SILENCE_MS) {
            self.lost(None, now_ms);
        }
        if self.line.is_none() && self.retry_at_ms.is_some_and(|at| now_ms >= at) {
            self.dial(now_ms);
        }
    }

    fn watch(&mut self, now_ms: f64) {
        match self.rt.status().clone() {
            Status::Joining => {}
            Status::Playing => {
                if self.status != LinkStatus::Playing {
                    self.status = LinkStatus::Playing;
                    self.attempt = 0;
                    self.token = self.rt.seat_token().to_string();
                }
            }
            Status::Rejected(why, text) => {
                if why == Reject::Key && !self.token.is_empty() {
                    self.token.clear();
                    self.line = None;
                    self.dial(now_ms);
                } else {
                    self.stop(LinkStatus::Rejected(why, text));
                }
            }
        }
    }

    fn flush(&mut self) {
        let out = self.rt.take_outgoing();
        let Some(line) = &self.line else {
            return;
        };
        for msg in out {
            self.bytes_out += msg.len() as u64;
            line.send(msg);
        }
    }
}

struct LocalLine {
    conn: LocalConn,
    opened: Option<Instant>,
    beat: Option<Instant>,
}

impl LocalLine {
    fn new(conn: LocalConn) -> LocalLine {
        LocalLine { conn, opened: Some(Instant::now()), beat: None }
    }

    fn poll(&mut self) -> Option<Said> {
        if let Some(at) = self.opened.take() {
            return Some(Said::Open(at));
        }
        match self.conn.recv_stamped(Duration::ZERO) {
            Ok(Some((msg, at))) => Some(Said::Message(msg, at)),
            Err(Closed { code }) => Some(Said::Closed(code)),
            Ok(None) => {
                let beat = self.conn.beat();
                if beat == self.beat {
                    return None;
                }
                self.beat = beat;
                beat.map(Said::Heard)
            }
        }
    }
}

enum Up {
    Message(Vec<u8>),
    Pong(Vec<u8>),
    Close(u16),
    End,
}

#[derive(Default)]
struct Shared {
    let_go: AtomicBool,
    leaving: AtomicBool,
    socket: Mutex<Option<TcpStream>>,
}

impl Shared {
    fn cut(&self) {
        if let Some(socket) = self.socket.lock().unwrap_or_else(|p| p.into_inner()).as_ref() {
            let _ = socket.shutdown(Shutdown::Both);
        }
    }
}

struct RemoteLine {
    said: Receiver<Said>,
    up: Sender<Up>,
    shared: Arc<Shared>,
}

impl RemoteLine {
    fn dial(url: String, trust: Trust) -> RemoteLine {
        let (said_tx, said) = channel();
        let (up, up_rx) = channel();
        let shared = Arc::new(Shared::default());
        let (thread_shared, thread_up) = (shared.clone(), up.clone());
        let _ = thread::Builder::new()
            .name("angel-link".into())
            .spawn(move || read_socket(&url, &trust, &thread_shared, &said_tx, &thread_up, up_rx));
        RemoteLine { said, up, shared }
    }

    fn poll(&mut self) -> Option<Said> {
        match self.said.try_recv() {
            Ok(said) => Some(said),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Said::Closed(None)),
        }
    }

    fn send(&self, msg: Vec<u8>) {
        let _ = self.up.send(Up::Message(msg));
    }

    fn goodbye(self) {
        self.shared.leaving.store(true, SeqCst);
        let _ = self.up.send(Up::Close(code::NORMAL));
    }
}

impl Drop for RemoteLine {
    fn drop(&mut self) {
        self.shared.let_go.store(true, SeqCst);
        if !self.shared.leaving.load(SeqCst) {
            self.shared.cut();
        }
    }
}

fn read_socket(url: &str, trust: &Trust, shared: &Shared, said: &Sender<Said>, up: &Sender<Up>, up_rx: Receiver<Up>) {
    let ws = match WsClient::connect_trusting(url, DIAL_TIMEOUT, trust) {
        Ok(ws) => ws,
        Err(_) => {
            let _ = said.send(Said::Closed(None));
            return;
        }
    };
    let (mut reader, mut writer) = ws.into_halves();
    {
        let mut socket = shared.socket.lock().unwrap_or_else(|p| p.into_inner());
        if shared.let_go.load(SeqCst) {
            return;
        }
        *socket = reader.socket().try_clone().ok();
    }
    let _ = said.send(Said::Open(Instant::now()));
    let out = thread::Builder::new().name("angel-link-out".into()).spawn(move || {
        while let Ok(up) = up_rx.recv() {
            let sent = match up {
                Up::Message(msg) => writer.send_frame(BINARY, &msg),
                Up::Pong(p) => writer.send_frame(PONG, &p),
                Up::Close(close) => {
                    writer.close(close);
                    break;
                }
                Up::End => break,
            };
            if sent.is_err() {
                break;
            }
        }
    });
    let Ok(out) = out else {
        shared.cut();
        let _ = said.send(Said::Closed(None));
        return;
    };
    let mut leaving_since: Option<Instant> = None;
    let (close, finish) = loop {
        match reader.next(READ_POLL) {
            Ok(Some(Message::Binary(msg))) => {
                let _ = said.send(Said::Message(msg, Instant::now()));
            }
            Ok(Some(Message::Ping(p))) => {
                let _ = said.send(Said::Heard(Instant::now()));
                let _ = up.send(Up::Pong(p));
            }
            Ok(Some(Message::Pong(_))) => {
                let _ = said.send(Said::Heard(Instant::now()));
            }
            Ok(Some(Message::Close(close))) => {
                let _ = up.send(Up::Close(close.unwrap_or(code::NORMAL)));
                break (close, true);
            }
            Ok(None) => {
                if shared.let_go.load(SeqCst) {
                    let leaving = shared.leaving.load(SeqCst);
                    if !leaving || leaving_since.get_or_insert_with(Instant::now).elapsed() >= CLOSE_WAIT {
                        break (None, leaving);
                    }
                }
            }
            Err(_) => break (None, false),
        }
    };
    let _ = said.send(Said::Closed(close));
    if !finish {
        shared.cut();
    }
    let _ = up.send(Up::End);
    let _ = out.join();
    shared.cut();
}
