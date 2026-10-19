//! One connection: its HTTP requests, then (on `/ws`) its hello, its
//! reader on this thread and its writer on another.
//!
//! The WebSocket's life (the hello, admission, the seat, the reader and the
//! writer) is written once, over a transport: a socket's frames, or an
//! in-process pipe's messages (`crate::local`). So the host's own
//! connections take the same path as everyone's, and a transport changes
//! only what it must (`Inlet::LOCAL`).

use crate::http::{self, IMMUTABLE, REVALIDATE, Request};
use crate::limits::{
    CLOSE_WAIT, HANDSHAKE_TIME, KEEP_ALIVE_IDLE, MAX_CLIENT_MESSAGE, MAX_HANDSHAKE, MAX_MESSAGES_PER_SEC, PING_EVERY, SILENT_CLOSE,
    WRITE_TIMEOUT,
};
use crate::link::random_key;
use crate::pace::Pace;
use crate::server::{Hub, Published};
use crate::session::Admit;
use crate::sim::Inbound;
use crate::stats::ConnCounters;
use crate::ws::{self, BINARY, CLOSE, Message, PING, PONG, Parser, Role, Violation, close_payload, code};
use crate::{GameServer, Replicate, To};
use angel_net::knob;
use angel_net::proto::{PROTOCOL, Reject, ToClient, ToServer, Welcome, caps};
use std::io::{self, ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::thread::{self, Thread};
use std::time::{Duration, Instant};

/// A reader only parses small messages: a small stack is plenty.
pub(crate) const READER_STACK: usize = 256 * 1024;
/// How often a blocked read wakes to check for silence and closing.
const READ_POLL: Duration = Duration::from_millis(250);
/// The shortest read timeout: the OS refuses a zero one.
const MIN_WAIT: Duration = Duration::from_millis(1);

/// What a writer sends besides snapshots and events.
pub(crate) enum Out {
    /// A whole protocol message (Welcome, Notice, Reject).
    Msg(Vec<u8>),
    /// A protocol `Pong`, answering the client's `Ping`. Only the newest
    /// waits: a writer stuck on a slow socket cannot pile them up.
    Reply(Vec<u8>),
    /// A WebSocket pong, answering the client's ping; likewise only the newest.
    Pong(Vec<u8>),
    /// The close frame; nothing goes after it.
    Close(u16),
}

/// What a reader heard.
pub(crate) enum Heard {
    /// A whole message.
    Message(Message),
    /// Bytes of a message not yet whole.
    Bytes,
    /// Nothing, within the wait.
    Quiet,
    /// The client is gone: its socket closed under us, or it let go of its pipe.
    Gone,
    /// The client broke the WebSocket protocol.
    Broke(Violation),
}

/// How a connection's reader hears its client.
pub(crate) trait Inlet {
    /// The host's own connection, in process. It is let in without the key
    /// and past the lock, and never counts among the connections not yet in:
    /// there is no door to flood. It is never limited in messages a second
    /// or closed for silence: it cannot flood a network, and it cannot
    /// vanish without its pipe saying so. Its writer is never held back for
    /// being behind: a pipe has no link to queue on, and a full pipe blocks
    /// the writer as a full socket does.
    const LOCAL: bool;
    /// The next thing heard, waiting at most `wait`.
    fn next(&mut self, wait: Duration) -> Heard;
    /// After this side's close: wait (at most `CLOSE_WAIT`) for the
    /// client's, so the reason is read before the connection goes; then let go.
    fn linger(&mut self);
}

/// One thing a writer puts out.
#[derive(Clone, Copy)]
pub(crate) enum Put<'a> {
    Binary(&'a [u8]),
    Ping,
    Pong(&'a [u8]),
    Close(u16),
}

/// How a connection's writer reaches its client.
pub(crate) trait Outlet: Send + 'static {
    /// Write one whole message: the bytes it took.
    fn put(&mut self, what: Put) -> io::Result<usize>;
    /// After the close: nothing more is written, and the client still reads
    /// what was.
    fn finish(&mut self);
}

/// Cuts a connection under both its halves, waking a reader or a writer
/// blocked on it.
pub(crate) trait Cut: Send + Sync {
    fn cut(&self);
}

impl Cut for TcpStream {
    fn cut(&self) {
        let _ = self.shutdown(Shutdown::Both);
    }
}

/// The state a connection's reader and writer share.
pub(crate) struct Conn {
    pub id: u64,
    pub seat: Option<u32>,
    pub welcome_tick: u32,
    /// The host's own, in process (`Inlet::LOCAL`).
    pub local: bool,
    /// The newest snapshot tick the client says it has applied, and when
    /// it said so.
    applied: Mutex<(u32, Option<Instant>)>,
    pub counters: Arc<ConnCounters>,
    cut: Arc<dyn Cut>,
    outbox: Mutex<Vec<Out>>,
    writer: OnceLock<Thread>,
    closing: AtomicBool,
}

impl Conn {
    pub fn send(&self, out: Out) {
        self.outbox.lock().expect("outbox lock").push(out);
        self.wake();
    }

    /// An answer (a pong): it replaces one of its kind still waiting.
    pub fn answer(&self, out: Out) {
        {
            let mut outbox = self.outbox.lock().expect("outbox lock");
            let same = outbox.iter().position(|o| std::mem::discriminant(o) == std::mem::discriminant(&out));
            match same {
                Some(i) => outbox[i] = out,
                None => outbox.push(out),
            }
        }
        self.wake();
    }

    pub fn wake(&self) {
        if let Some(t) = self.writer.get() {
            t.unpark();
        }
    }

    /// Close from this side: `reject` first if given (it says why), then
    /// the close frame. The reader waits `CLOSE_WAIT` for the client's.
    pub fn close(&self, code: u16, reject: Option<Vec<u8>>) {
        if self.closing.swap(true, SeqCst) {
            return;
        }
        {
            let mut outbox = self.outbox.lock().expect("outbox lock");
            if let Some(msg) = reject {
                outbox.push(Out::Msg(msg));
            }
            outbox.push(Out::Close(code));
        }
        self.wake();
    }

    pub fn closing(&self) -> bool {
        self.closing.load(SeqCst)
    }

    fn shutdown(&self) {
        self.cut.cut();
    }
}

/// Counts this connection among those not yet past their hello until
/// it is let in or gone.
pub(crate) struct Pending<'a> {
    count: &'a std::sync::atomic::AtomicUsize,
    held: bool,
}

impl Pending<'_> {
    fn release(&mut self) {
        if self.held {
            self.held = false;
            self.count.fetch_sub(1, SeqCst);
        }
    }
}

impl Drop for Pending<'_> {
    fn drop(&mut self) {
        self.release();
    }
}

fn timed_out(e: &io::Error) -> bool {
    matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
}

/// The connection's whole life, on the thread the accept loop started.
pub(crate) fn handle<G: GameServer>(hub: Arc<Hub<G>>, mut stream: TcpStream) {
    let mut pending = Pending { count: &hub.pending, held: true };
    let _ = stream.set_nodelay(true);
    let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
    let Some(leftover) = serve_http(&hub, &mut stream) else {
        return;
    };
    hub.tele.counters.upgrades.fetch_add(1, SeqCst);
    // The reader keeps this handle on the socket, the writer a second, and
    // whoever must cut the connection a third.
    let (Ok(writer), Ok(cut)) = (stream.try_clone(), stream.try_clone()) else {
        return;
    };
    let mut parser = Parser::new(Role::Server, MAX_CLIENT_MESSAGE);
    parser.feed(&leftover);
    let inlet = SocketIn { stream, parser, chunk: vec![0; 4096], wait: None };
    let outlet = SocketOut { stream: writer, frame: Vec::with_capacity(4096) };
    websocket(&hub, inlet, outlet, Arc::new(cut), Some(&mut pending));
}

/// Read one request head into `buf`, within the deadline and the size
/// limit. Returns the head's length, its blank line included.
fn read_head(stream: &mut TcpStream, buf: &mut Vec<u8>, deadline: Instant) -> Result<usize, &'static str> {
    let mut chunk = [0u8; 1024];
    loop {
        // Bytes past the head (a pipelined request, a hello sent with the
        // upgrade) stay in `buf` for whoever reads next.
        if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            return if at <= MAX_HANDSHAKE { Ok(at + 4) } else { Err("431 Request Header Fields Too Large") };
        }
        if buf.len() >= MAX_HANDSHAKE {
            return Err("431 Request Header Fields Too Large");
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err("408 Request Timeout");
        }
        let _ = stream.set_read_timeout(Some(left));
        match stream.read(&mut chunk) {
            Ok(0) => return Err(""),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) if timed_out(&e) => return Err("408 Request Timeout"),
            Err(_) => return Err(""),
        }
    }
}

/// How long a closing HTTP connection keeps reading what the client still
/// sends, and how much of it, so its last response is not lost to a reset.
const HTTP_LINGER: std::time::Duration = std::time::Duration::from_millis(500);
const HTTP_LINGER_BYTES: usize = 64 * 1024;

/// Close after a last response: half-close, then read and drop what the
/// client was still sending (a long head, a pipelined request). Closing
/// with unread bytes would reset the connection, and a client can lose the
/// response it has not read yet to that reset.
fn http_close(stream: &mut TcpStream) {
    let _ = stream.shutdown(Shutdown::Write);
    let until = Instant::now() + HTTP_LINGER;
    let mut chunk = [0u8; 4096];
    let mut drained = 0;
    while drained < HTTP_LINGER_BYTES {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        let _ = stream.set_read_timeout(Some(left));
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => drained += n,
        }
    }
}

/// Requests until the connection closes or upgrades. On an upgrade, the
/// bytes read past the handshake.
fn serve_http<G: GameServer>(hub: &Hub<G>, stream: &mut TcpStream) -> Option<Vec<u8>> {
    let mut buf = Vec::with_capacity(1024);
    let mut first = true;
    loop {
        // The first request (or one already begun) has the handshake time;
        // a kept-alive connection waits less for its next one.
        let wait = if first || !buf.is_empty() { HANDSHAKE_TIME } else { KEEP_ALIVE_IDLE };
        first = false;
        let len = match read_head(stream, &mut buf, Instant::now() + wait) {
            Ok(len) => len,
            Err(status) => {
                if !status.is_empty() && !buf.is_empty() {
                    let _ = stream.write_all(&http::plain(status, status, false, &[]));
                    http_close(stream);
                }
                return None;
            }
        };
        hub.tele.counters.http.fetch_add(1, SeqCst);
        let head = std::str::from_utf8(&buf[..len - 4]).ok();
        let Some(req) = head.and_then(Request::parse) else {
            let _ = stream.write_all(&http::plain("400 Bad Request", "bad request", false, &[]));
            http_close(stream);
            return None;
        };
        // No request here has a body.
        if req.header("content-length").is_some_and(|n| n.trim() != "0") || req.header("transfer-encoding").is_some() {
            let _ = stream.write_all(&http::plain("400 Bad Request", "no bodies here", false, &[]));
            http_close(stream);
            return None;
        }
        let mut keep = req.keep_alive();
        let response = match (req.method, req.path) {
            ("GET", "/ws") => match upgrade(&req) {
                Ok(accept) => {
                    let response = format!(
                        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
                         Sec-WebSocket-Accept: {accept}\r\n\r\n"
                    );
                    if stream.write_all(response.as_bytes()).is_err() {
                        return None;
                    }
                    return Some(buf[len..].to_vec());
                }
                Err(response) => {
                    keep = false;
                    response
                }
            },
            ("GET" | "HEAD", "/") => http::body(&req, &hub.assets.index, REVALIDATE, keep),
            ("GET" | "HEAD", path) => match path.strip_prefix("/a/").and_then(|name| hub.assets.files.get(name)) {
                Some(body) => http::body(&req, body, IMMUTABLE, keep),
                None => http::plain("404 Not Found", "not here", keep, &[]),
            },
            _ => http::plain("405 Method Not Allowed", "GET only", keep, &[("Allow", "GET, HEAD")]),
        };
        if stream.write_all(&response).is_err() {
            return None;
        }
        if !keep {
            http_close(stream);
            return None;
        }
        buf.drain(..len);
    }
}

/// The handshake's checks (RFC 6455 §4.2.1), and the accept key.
fn upgrade(req: &Request) -> Result<String, Vec<u8>> {
    if !req.lists("upgrade", "websocket") || !req.lists("connection", "upgrade") {
        return Err(http::plain("400 Bad Request", "this is the game socket: it speaks WebSocket", false, &[]));
    }
    if req.header("sec-websocket-version") != Some("13") {
        return Err(http::plain("426 Upgrade Required", "WebSocket version 13", false, &[("Sec-WebSocket-Version", "13")]));
    }
    match req.header("sec-websocket-key") {
        Some(key) if ws::valid_key(key) => Ok(ws::accept_key(key)),
        _ => Err(http::plain("400 Bad Request", "bad Sec-WebSocket-Key", false, &[])),
    }
}


/// A socket's reading half: WebSocket frames, read into messages.
struct SocketIn {
    stream: TcpStream,
    parser: Parser,
    chunk: Vec<u8>,
    /// The read timeout last set, so it is set again only when it changes.
    wait: Option<Duration>,
}

impl Inlet for SocketIn {
    const LOCAL: bool = false;

    fn next(&mut self, wait: Duration) -> Heard {
        match self.parser.message() {
            Ok(Some(m)) => return Heard::Message(m),
            Ok(None) => {}
            Err(v) => return Heard::Broke(v),
        }
        let wait = wait.max(MIN_WAIT);
        if self.wait != Some(wait) {
            let _ = self.stream.set_read_timeout(Some(wait));
            self.wait = Some(wait);
        }
        match self.stream.read(&mut self.chunk) {
            Ok(0) => Heard::Gone,
            Ok(n) => {
                self.parser.feed(&self.chunk[..n]);
                Heard::Bytes
            }
            Err(e) if timed_out(&e) => Heard::Quiet,
            Err(_) => Heard::Gone,
        }
    }

    fn linger(&mut self) {
        let until = Instant::now() + CLOSE_WAIT;
        loop {
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            match self.next(left) {
                Heard::Message(Message::Close(_)) | Heard::Broke(_) | Heard::Gone => break,
                Heard::Message(_) | Heard::Bytes | Heard::Quiet => {}
            }
        }
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

/// A socket's writing half: each message one frame, written whole.
struct SocketOut {
    stream: TcpStream,
    frame: Vec<u8>,
}

impl Outlet for SocketOut {
    fn put(&mut self, what: Put) -> io::Result<usize> {
        let close;
        let (opcode, payload) = match what {
            Put::Binary(bytes) => (BINARY, bytes),
            Put::Ping => (PING, &[][..]),
            Put::Pong(p) => (PONG, p),
            Put::Close(c) => {
                close = close_payload(c);
                (CLOSE, &close[..])
            }
        };
        self.frame.clear();
        ws::frame(&mut self.frame, opcode, payload, None);
        self.stream.write_all(&self.frame)?;
        Ok(self.frame.len())
    }

    /// Half-close: the client still reads what was sent (the reason), and
    /// the reader waits for its close.
    fn finish(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Write);
    }
}

/// Turned away after the upgrade: say why, close, and wait briefly for the
/// client's close so the reason is read before the connection goes.
fn refuse(inlet: &mut impl Inlet, outlet: &mut impl Outlet, why: Reject, text: &str, close_code: u16) {
    let _ = outlet.put(Put::Binary(&ToClient::Reject { why, text }.to_vec()));
    let _ = outlet.put(Put::Close(close_code));
    inlet.linger();
}

/// The first message, which must be a `Hello`, within the handshake time.
/// Pings before it are answered here; there is no writer yet.
fn first_message(inlet: &mut impl Inlet, outlet: &mut impl Outlet) -> Result<Vec<u8>, (u16, &'static str)> {
    let until = Instant::now() + HANDSHAKE_TIME;
    loop {
        match inlet.next(until.saturating_duration_since(Instant::now())) {
            Heard::Message(Message::Binary(bytes)) => return Ok(bytes),
            Heard::Message(Message::Ping(p)) => {
                if outlet.put(Put::Pong(&p)).is_err() {
                    return Err((code::GOING_AWAY, ""));
                }
            }
            Heard::Message(Message::Close(c)) => {
                let _ = outlet.put(Put::Close(c.unwrap_or(code::NORMAL)));
                return Err((code::NORMAL, ""));
            }
            Heard::Message(Message::Pong(_)) | Heard::Bytes | Heard::Quiet => {}
            Heard::Gone => return Err((code::GOING_AWAY, "")),
            Heard::Broke(v) => return Err((v.code(), "the socket broke the protocol")),
        }
        if Instant::now() >= until {
            return Err((code::POLICY, "no hello in time"));
        }
    }
}

/// The WebSocket's life over either transport: the hello, then reading
/// while the writer writes. A socket's `pending` counts it among those not
/// yet in until its hello is let in.
pub(crate) fn websocket<G: GameServer, I: Inlet, O: Outlet>(
    hub: &Arc<Hub<G>>,
    mut inlet: I,
    mut outlet: O,
    cut: Arc<dyn Cut>,
    pending: Option<&mut Pending>,
) {
    let counters = &hub.tele.counters;
    let hello = match first_message(&mut inlet, &mut outlet) {
        Ok(bytes) => bytes,
        Err((close_code, why)) => {
            if !why.is_empty() {
                counters.violations.fetch_add(1, SeqCst);
                refuse(&mut inlet, &mut outlet, Reject::Abuse, why, close_code);
            }
            return;
        }
    };
    let Ok(ToServer::Hello { protocol, game, join, resume, name, caps: client_caps }) = ToServer::decode(&hello) else {
        counters.violations.fetch_add(1, SeqCst);
        refuse(&mut inlet, &mut outlet, Reject::Abuse, "the first message must be a hello", code::POLICY);
        return;
    };
    if protocol != PROTOCOL {
        counters.rejected.fetch_add(1, SeqCst);
        refuse(&mut inlet, &mut outlet, Reject::Protocol, "this page is out of date: reload it", code::NORMAL);
        return;
    }
    if game != G::GAME {
        counters.rejected.fetch_add(1, SeqCst);
        refuse(&mut inlet, &mut outlet, Reject::Game, "this session is playing another game", code::NORMAL);
        return;
    }
    if hub.stop.load(SeqCst) {
        refuse(&mut inlet, &mut outlet, Reject::Stopping, "the host closed the session", code::GOING_AWAY);
        return;
    }

    let id = hub.next_conn.fetch_add(1, SeqCst);
    // Admitted, registered and told to the sim in one critical section: a
    // kick, a stop or a second reload finds this connection the moment its
    // seat is taken, and the sim sees seats change in the order they did.
    let admitted = {
        let mut sessions = hub.sessions.lock().expect("sessions lock");
        let watch = client_caps & caps::WATCH != 0;
        match sessions.hello(id, join, resume, name, watch, I::LOCAL, Instant::now(), random_key) {
            Err(why) => Err(why),
            Ok(admit) => {
                let (seat, token, resumed, displaced) = match &admit {
                    Admit::Seat { seat, token, resumed, displaced, .. } => (Some(*seat), token.clone(), *resumed, *displaced),
                    Admit::Watch => (None, String::new(), false, None),
                };
                let welcome_tick = hub.newest.load(SeqCst);
                let welcome = ToClient::Welcome(Welcome {
                    protocol: PROTOCOL,
                    game: G::GAME,
                    seat,
                    seat_token: &token,
                    tick_hz: hub.rates.tick_hz,
                    snapshot_hz: hub.rates.snapshot_hz,
                    tick: welcome_tick,
                    params: &hub.params,
                })
                .to_vec();
                let conn = Arc::new(Conn {
                    id,
                    seat,
                    welcome_tick,
                    local: I::LOCAL,
                    applied: Mutex::new((0, None)),
                    counters: Arc::new(ConnCounters::default()),
                    cut,
                    // The welcome, then the knob table as this connection
                    // is told it (whether it may set them).
                    outbox: Mutex::new(vec![Out::Msg(welcome), Out::Msg(hub.knobs_message(I::LOCAL))]),
                    writer: OnceLock::new(),
                    closing: AtomicBool::new(false),
                });
                {
                    let mut conns = hub.conns.lock().expect("connections lock");
                    conns.insert(id, conn.clone());
                    if let Some(old) = displaced.and_then(|d| conns.get(&d)) {
                        old.close(code::REPLACED, None);
                    }
                }
                if let Admit::Seat { seat, name, resumed, .. } = admit {
                    let _ = hub.inbound.send(Inbound::Joined {
                        seat,
                        conn: id,
                        name,
                        resumed,
                        welcomed: welcome_tick,
                        local: I::LOCAL,
                    });
                }
                Ok((conn, resumed))
            }
        }
    };
    let conn = match admitted {
        Ok((conn, resumed)) => {
            let n = if resumed { &counters.resumed } else { &counters.joined };
            n.fetch_add(1, SeqCst);
            conn
        }
        Err(why) => {
            counters.rejected.fetch_add(1, SeqCst);
            let text = match why {
                Reject::Key => "this link is not this session's (the host may have made a new one)",
                Reject::Locked => "the host has closed the session to newcomers",
                Reject::Full => "every seat and every watcher's place is taken",
                Reject::Kicked => "the host removed you from this session",
                _ => "turned away",
            };
            refuse(&mut inlet, &mut outlet, why, text, code::NORMAL);
            return;
        }
    };
    if let Some(pending) = pending {
        pending.release();
    }
    hub.tele.conns.lock().expect("telemetry lock").push((id, conn.counters.clone(), Instant::now()));
    // From here, whatever happens (a goodbye, a drop, a panic in game code),
    // the guard closes the connection and gives the seat back or holds it.
    let mut admitted = Admitted { hub, conn: conn.clone(), bye: false };
    // A stop that looked at the connections before this one was in them.
    if hub.stop.load(SeqCst) {
        let reject = ToClient::Reject { why: Reject::Stopping, text: "the host closed the session" };
        conn.close(code::GOING_AWAY, Some(reject.to_vec()));
    }

    let writer_hub = hub.clone();
    let writer_conn = conn.clone();
    // Dropped when the writer ends, however it ends.
    let (done, writer_done) = mpsc::channel::<()>();
    let writer = thread::Builder::new().name("angel-writer".into()).spawn(move || {
        let _done = done;
        write_loop(writer_hub, writer_conn, outlet);
    });
    if let Ok(handle) = writer {
        let _ = conn.writer.set(handle.thread().clone());
        conn.wake();
        admitted.bye = read_loop(hub, &conn, &mut inlet);
        // The writer finishes what it was sending (a close), then stops;
        // one stuck on a connection that takes nothing is cut off.
        if !conn.closing() {
            conn.close(code::GOING_AWAY, None);
        }
        let _ = writer_done.recv_timeout(CLOSE_WAIT);
        conn.shutdown();
        let _ = handle.join();
    }
}

/// An admitted connection's end, run however `websocket` returns.
struct Admitted<'a, G: GameServer> {
    hub: &'a Hub<G>,
    conn: Arc<Conn>,
    bye: bool,
}

impl<G: GameServer> Drop for Admitted<'_, G> {
    fn drop(&mut self) {
        let (hub, conn) = (self.hub, &self.conn);
        if !conn.closing() {
            conn.close(code::GOING_AWAY, None);
        }
        conn.shutdown();
        hub.conns.lock().unwrap_or_else(|p| p.into_inner()).remove(&conn.id);
        hub.tele.conns.lock().unwrap_or_else(|p| p.into_inner()).retain(|(c, _, _)| *c != conn.id);
        leave(hub, conn.id, conn.seat, self.bye);
    }
}

/// A connection is gone: a goodbye frees its seat, anything else holds it
/// for a resume.
fn leave<G: GameServer>(hub: &Hub<G>, id: u64, seat: Option<u32>, bye: bool) {
    let mut sessions = hub.sessions.lock().unwrap_or_else(|p| p.into_inner());
    match seat {
        Some(seat) if bye => {
            if sessions.bye(id, seat) {
                let _ = hub.inbound.send(Inbound::Left { seat });
            }
        }
        Some(seat) => {
            sessions.dropped(id, seat, Instant::now());
            let _ = hub.inbound.send(Inbound::Dropped { seat, conn: id });
        }
        None => sessions.watcher_left(),
    }
}

/// Messages a client may send, as a bucket that refills each second.
struct Bucket {
    tokens: f64,
    at: Instant,
}

impl Bucket {
    fn take(&mut self) -> bool {
        let now = Instant::now();
        let rate = f64::from(MAX_MESSAGES_PER_SEC);
        self.tokens = (self.tokens + now.duration_since(self.at).as_secs_f64() * rate).min(rate);
        self.at = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Read until the client leaves, breaks a rule, or goes silent. True when
/// it said goodbye.
fn read_loop<G: GameServer>(hub: &Arc<Hub<G>>, conn: &Conn, inlet: &mut impl Inlet) -> bool {
    let counters = &hub.tele.counters;
    let mut bucket = Bucket { tokens: f64::from(MAX_MESSAGES_PER_SEC), at: Instant::now() };
    let mut heard = Instant::now();
    let mut closing_since: Option<Instant> = None;
    let abuse = |text: &str| {
        counters.violations.fetch_add(1, SeqCst);
        conn.close(code::POLICY, Some(ToClient::Reject { why: Reject::Abuse, text }.to_vec()));
    };
    loop {
        if conn.closing() {
            let since = *closing_since.get_or_insert_with(Instant::now);
            if since.elapsed() >= CLOSE_WAIT {
                return false;
            }
        }
        let message = match inlet.next(READ_POLL) {
            Heard::Message(m) => {
                heard = Instant::now();
                m
            }
            Heard::Bytes => {
                heard = Instant::now();
                continue;
            }
            Heard::Quiet => {
                if !conn.local && heard.elapsed() >= SILENT_CLOSE && !conn.closing() {
                    counters.silent.fetch_add(1, SeqCst);
                    conn.close(code::GOING_AWAY, None);
                }
                continue;
            }
            Heard::Gone => return false,
            Heard::Broke(v) => {
                counters.violations.fetch_add(1, SeqCst);
                let text = "the socket broke the protocol";
                conn.close(v.code(), Some(ToClient::Reject { why: Reject::Abuse, text }.to_vec()));
                return false;
            }
        };
        if let Message::Close(c) = message {
            conn.close(c.unwrap_or(code::NORMAL), None);
            return false;
        }
        if conn.closing() {
            continue;
        }
        if !conn.local && !bucket.take() {
            abuse("too many messages");
            continue;
        }
        match message {
            Message::Ping(p) => conn.answer(Out::Pong(p)),
            Message::Pong(_) | Message::Close(_) => {}
            Message::Binary(bytes) => match ToServer::decode(&bytes) {
                Ok(ToServer::Cmds { applied, cmds: raw }) => {
                    let mut held = conn.applied.lock().expect("applied lock");
                    if applied > held.0 {
                        *held = (applied, Some(Instant::now()));
                    }
                    drop(held);
                    let Some(seat) = conn.seat else { continue };
                    let decoded: Option<Vec<(u32, G::Cmd)>> =
                        raw.iter().map(|&(seq, body)| G::decode_cmd(body).map(|cmd| (seq, cmd))).collect();
                    match decoded {
                        Some(cmds) => {
                            let _ = hub.inbound.send(Inbound::Cmds { seat, conn: conn.id, applied, cmds });
                        }
                        None => abuse("a command did not read"),
                    }
                }
                Ok(ToServer::Ping { t }) => {
                    conn.answer(Out::Reply(ToClient::Pong { t, server_ms: hub.server_ms() }.to_vec()));
                }
                Ok(ToServer::Action { kind, body }) if kind == knob::action::STATS => {
                    let mut watching = hub.watching.lock().expect("watching lock");
                    if body.first() == Some(&1) {
                        watching.insert(conn.id);
                    } else {
                        watching.remove(&conn.id);
                    }
                }
                Ok(ToServer::Action { kind, body }) => {
                    let _ = hub.inbound.send(Inbound::Action {
                        seat: conn.seat,
                        conn: conn.id,
                        local: conn.local,
                        kind,
                        body: body.to_vec(),
                    });
                }
                Ok(ToServer::Bye) => {
                    conn.close(code::NORMAL, None);
                    return true;
                }
                Ok(ToServer::Hello { .. }) | Err(_) => abuse("a message did not read"),
            },
        }
    }
}

/// The writer: the welcome and anything queued, then each newest tick's
/// events and this client's snapshot, and a ping every second.
fn write_loop<G: GameServer, O: Outlet>(hub: Arc<Hub<G>>, conn: Arc<Conn>, mut outlet: O) {
    let _unwind = WriterUnwind(&conn);
    let counters = &conn.counters;
    let tele = &hub.tele;
    let mut viewer = hub.rep.viewer(conn.seat);
    let mut msg = Vec::with_capacity(2048);
    let mut body = Vec::with_capacity(2048);
    let mut outs = Vec::new();
    let mut recent: Vec<Published<G>> = Vec::new();
    // Events and snapshots are sent through this tick.
    let mut through = conn.welcome_tick;
    let mut opened = false;
    let mut pace = Pace::default();
    let mut applied_seen = 0u32;
    let mut held_at = 0u32;
    let mut last_ping = Instant::now();
    let sent = |outlet: &mut O, what: Put| -> bool {
        match outlet.put(what) {
            Ok(n) => {
                counters.bytes.fetch_add(n as u64, SeqCst);
                true
            }
            Err(_) => {
                tele.counters.write_errors.fetch_add(1, SeqCst);
                false
            }
        }
    };
    'run: loop {
        std::mem::swap(&mut outs, &mut *conn.outbox.lock().expect("outbox lock"));
        for out in outs.drain(..) {
            let ok = match &out {
                Out::Msg(m) | Out::Reply(m) => sent(&mut outlet, Put::Binary(m)),
                Out::Pong(p) => sent(&mut outlet, Put::Pong(p)),
                Out::Close(c) => {
                    sent(&mut outlet, Put::Close(*c));
                    conn.closing.store(true, SeqCst);
                    outlet.finish();
                    return;
                }
            };
            if !ok {
                break 'run;
            }
        }

        // A new client sees the world at once: the tick newest at its
        // welcome, whole, rather than nothing until the next one.
        if !opened {
            opened = true;
            let newest = hub.ticks.lock().expect("tick ring lock").back().filter(|t| t.tick == through).cloned();
            if let Some(newest) = newest {
                let mut put = Snapshot { viewer: &mut viewer, body: &mut body, msg: &mut msg, pace: &mut pace };
                if !put.write(&hub, &conn, &newest, &mut outlet) {
                    break 'run;
                }
            }
        }

        if let (applied, Some(at)) = *conn.applied.lock().expect("applied lock")
            && applied != applied_seen
        {
            applied_seen = applied;
            pace.applied(applied, at);
            if let (Some(seat), Some(round_trip)) = (conn.seat, pace.round_trip()) {
                let _ = hub.inbound.send(Inbound::Latency { seat, conn: conn.id, round_trip });
            }
        }
        // A writer holding back only looks at the newest tick's number, so
        // it never touches the tick ring the sim publishes into.
        let newest_tick = hub.newest.load(SeqCst);
        if newest_tick != through {
            if !conn.local && pace.behind() {
                if newest_tick.is_multiple_of(hub.every.load(SeqCst).max(1)) && newest_tick > held_at {
                    held_at = newest_tick;
                    counters.held.fetch_add(1, SeqCst);
                }
            } else {
                hub.ticks_since(through, &mut recent);
                if let Some(newest) = recent.last() {
                    // Ticks that aged out of the ring while this writer was held
                    // back or blocked: their events are gone.
                    let lost = recent[0].tick.wrapping_sub(through).saturating_sub(1);
                    tele.counters.events_lost.fetch_add(u64::from(lost), SeqCst);
                    for t in &recent {
                        let seat = t.seat_of(&conn);
                        let heard: Vec<&[u8]> = t
                            .events
                            .iter()
                            .filter(|e| match e.to {
                                To::All => true,
                                To::Seat(s) => seat == Some(s),
                                To::Near(x, y) => hub.rep.hears(&t.body, seat, x, y),
                            })
                            .map(|e| e.bytes.as_slice())
                            .collect();
                        for events in heard.chunks(angel_net::proto::MAX_EVENTS) {
                            msg.clear();
                            ToClient::Events { tick: t.tick, events: events.to_vec() }.encode(&mut msg);
                            if !sent(&mut outlet, Put::Binary(&msg)) {
                                break 'run;
                            }
                        }
                    }
                    let skipped = recent[..recent.len() - 1].iter().filter(|t| t.snapshot).count();
                    counters.skipped.fetch_add(skipped as u64, SeqCst);
                    through = newest.tick;
                    if newest.snapshot {
                        let mut put = Snapshot { viewer: &mut viewer, body: &mut body, msg: &mut msg, pace: &mut pace };
                        if !put.write(&hub, &conn, newest, &mut outlet) {
                            break 'run;
                        }
                    }
                }
            }
        }

        if last_ping.elapsed() >= PING_EVERY {
            last_ping = Instant::now();
            if !sent(&mut outlet, Put::Ping) {
                break 'run;
            }
        }
        thread::park_timeout(PING_EVERY.saturating_sub(last_ping.elapsed()));
    }
    // The connection failed: the reader is woken by it closing under it.
    conn.closing.store(true, SeqCst);
    conn.shutdown();
}

/// What a writer keeps to put one snapshot to its client.
struct Snapshot<'a, V> {
    viewer: &'a mut V,
    body: &'a mut Vec<u8>,
    msg: &'a mut Vec<u8>,
    pace: &'a mut Pace,
}

impl<V> Snapshot<'_, V> {
    /// This client's snapshot of tick `t`; false when the outlet failed.
    fn write<G: GameServer<Rep = R>, R: Replicate<Viewer = V>, O: Outlet>(
        &mut self,
        hub: &Hub<G>,
        conn: &Conn,
        t: &Published<G>,
        outlet: &mut O,
    ) -> bool {
        let started = Instant::now();
        self.body.clear();
        let seat = t.seat_of(conn);
        let whole = hub.rep.snapshot(&t.body, seat, self.viewer, self.body);
        let ack = seat.and_then(|s| t.acks.get(s as usize)).copied().unwrap_or(0);
        self.msg.clear();
        ToClient::Snapshot { tick: t.tick, ack, whole, body: self.body }.encode(self.msg);
        hub.tele.encode.record(started.elapsed());
        let Ok(n) = outlet.put(Put::Binary(self.msg)) else {
            hub.tele.counters.write_errors.fetch_add(1, SeqCst);
            return false;
        };
        conn.counters.bytes.fetch_add(n as u64, SeqCst);
        conn.counters.snapshots.fetch_add(1, SeqCst);
        self.pace.written(t.tick, Instant::now(), n);
        true
    }
}

/// A writer that panics (in game code) closes its connection, so its
/// reader ends too and the seat is given back or held, not left half alive.
struct WriterUnwind<'a>(&'a Conn);

impl Drop for WriterUnwind<'_> {
    fn drop(&mut self) {
        if thread::panicking() {
            self.0.closing.store(true, SeqCst);
            self.0.shutdown();
        }
    }
}
