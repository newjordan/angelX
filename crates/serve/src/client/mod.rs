//! The native client's side of the link.
//!
//! - `WsClient`: a blocking WebSocket client for `ws://` and, with the
//!   `tls` feature, `wss://`. It masks every frame and answers the server's
//!   pings itself. The bots and the tests drive it on one thread.
//! - `share_link`: a host's share link read into its socket and join key.
//! - `NativeLink`: a game played from a terminal, over a socket or an
//!   in-process `LocalConn`, without ever blocking the thread that draws.

mod native;
#[cfg(feature = "tls")]
mod tls;

pub use native::{LinkOptions, LinkStatus, NativeLink};

use crate::hash::base64;
use crate::limits::WRITE_TIMEOUT;
use crate::ws::{self, BINARY, CLOSE, Message, PONG, Parser, Role, close_payload};
use std::io::{self, ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

/// A server message is at most this (`angel_net::proto::MAX_SERVER_MESSAGE`).
const MAX_MESSAGE: usize = angel_net::proto::MAX_SERVER_MESSAGE;
/// The upgrade's answer is at most this long.
const MAX_HEAD: usize = 4096;
/// The shortest read timeout: the OS refuses a zero one.
const MIN_WAIT: Duration = Duration::from_micros(100);
/// Bytes read off the socket at a time.
const CHUNK: usize = 16 * 1024;

/// What a `wss://` link's certificate must chain to. It is always
/// verified, and its name checked against the link's host; this says only
/// against which roots.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Trust {
    /// Mozilla's root certificates (`webpki-roots`), compiled in: what
    /// browsers trust, so Funnel's and a relay's certificates verify, and
    /// the same on Linux, macOS and Windows with nothing to configure.
    #[default]
    Public,
    /// Only these roots (DER): a test's own certificate, or a private relay's.
    Only(Vec<Vec<u8>>),
}

fn bad(text: &str) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, text.to_string())
}

fn timed_out(e: &io::Error) -> bool {
    matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
}

/// A host's share link, read the way the page reads it: the socket is
/// `/ws` on the link's own host, `wss://` for an `https://` link, and the
/// join key is the fragment.
///
/// - `https://castle.ts.net/#KEY` is `wss://castle.ts.net/ws` and `KEY`;
/// - `http://192.168.1.5:8790/#KEY` is `ws://192.168.1.5:8790/ws`;
/// - a path or query on the link is the page's, not the socket's, and is
///   left out, as the page leaves it;
/// - a `ws://` or `wss://` URL with a key is taken as the socket itself.
///
/// The link may be upper case, as its QR code is.
pub fn share_link(link: &str) -> Result<(String, String), String> {
    let link = link.trim();
    let (base, key) = link.split_once('#').ok_or("the link has no join key (the part after #)")?;
    let key = key.trim();
    if key.is_empty() || key.contains(char::is_whitespace) {
        return Err("the link's join key (after #) is empty or broken".into());
    }
    let (scheme, rest) = base.split_once("://").ok_or("not a link: it should start https:// or http://")?;
    let scheme = scheme.to_ascii_lowercase();
    let (socket, own_path) = match scheme.as_str() {
        "https" => ("wss", false),
        "http" => ("ws", false),
        "wss" => ("wss", true),
        "ws" => ("ws", true),
        _ => return Err(format!("{scheme}:// links are not game links")),
    };
    let end = rest.find(['/', '?']).unwrap_or(rest.len());
    let authority = rest[..end].to_ascii_lowercase();
    if authority.is_empty() || authority.contains('@') || host_port(&authority).is_none() {
        return Err("the link's host does not read".into());
    }
    let path = rest[end..].split('?').next().unwrap_or("");
    let path = if own_path && path.len() > 1 { path } else { "/ws" };
    Ok((format!("{socket}://{authority}{path}"), key.to_string()))
}

/// `host`, `host:port`, `[v6]` or `[v6]:port`: the host (without brackets)
/// and the port, if given.
fn host_port(authority: &str) -> Option<(&str, Option<u16>)> {
    if let Some(rest) = authority.strip_prefix('[') {
        let (host, after) = rest.split_once(']')?;
        let port = match after {
            "" => None,
            _ => Some(after.strip_prefix(':')?.parse().ok()?),
        };
        Some((host, port))
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) if !host.is_empty() && !host.contains(':') => Some((host, Some(port.parse().ok()?))),
            Some(_) => None,
            None => Some((authority, None)),
        }
    }
}

/// Where a `ws://` or `wss://` URL points.
struct Target<'a> {
    secure: bool,
    /// As written, for the `Host` header.
    authority: &'a str,
    /// For the name lookup and TLS's server name: an IPv6 host without its brackets.
    host: &'a str,
    port: u16,
    path: &'a str,
}

fn target(url: &str) -> io::Result<Target<'_>> {
    let (secure, rest) = if let Some(rest) = url.strip_prefix("ws://") {
        (false, rest)
    } else if let Some(rest) = url.strip_prefix("wss://") {
        (true, rest)
    } else {
        return Err(bad("only ws:// and wss:// links"));
    };
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = host_port(authority).ok_or_else(|| bad("the link's host does not read"))?;
    let port = port.unwrap_or(if secure { 443 } else { 80 });
    Ok(Target { secure, authority, host, port, path })
}

/// TCP to the first of the host's addresses that answers before `deadline`
/// (a name with an IPv6 address on a network that drops IPv6 still joins
/// on its IPv4 one).
fn dial(t: &Target, deadline: Instant) -> io::Result<TcpStream> {
    let addrs: Vec<SocketAddr> = (t.host, t.port).to_socket_addrs()?.collect();
    let mut last = io::Error::new(ErrorKind::NotFound, "the host has no address");
    for addr in addrs {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::Error::new(ErrorKind::TimedOut, "no answer in time"));
        }
        match TcpStream::connect_timeout(&addr, left) {
            Ok(stream) => return Ok(stream),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// The bytes under the WebSocket, as read: plain, or TLS's plaintext.
enum ReadWire {
    Plain(TcpStream, Vec<u8>),
    #[cfg(feature = "tls")]
    Tls(tls::ReadHalf),
}

impl ReadWire {
    /// Wait up to `wait` for the socket, read at most `max` bytes off it,
    /// and hand what they carry to `sink`. The bytes handed, 0 when nothing
    /// came; an error once the socket is closed.
    fn fill(&mut self, max: usize, wait: Duration, sink: &mut dyn FnMut(&[u8])) -> io::Result<usize> {
        match self {
            ReadWire::Plain(stream, chunk) => {
                stream.set_read_timeout(Some(wait.max(MIN_WAIT)))?;
                let n = max.min(chunk.len());
                match stream.read(&mut chunk[..n]) {
                    Ok(0) => Err(io::Error::new(ErrorKind::UnexpectedEof, "the server closed the socket")),
                    Ok(n) => {
                        sink(&chunk[..n]);
                        Ok(n)
                    }
                    Err(e) if timed_out(&e) => Ok(0),
                    Err(e) => Err(e),
                }
            }
            #[cfg(feature = "tls")]
            ReadWire::Tls(half) => half.fill(max, wait.max(MIN_WAIT), sink),
        }
    }

    fn socket(&self) -> &TcpStream {
        match self {
            ReadWire::Plain(stream, _) => stream,
            #[cfg(feature = "tls")]
            ReadWire::Tls(half) => half.socket(),
        }
    }
}

/// The bytes under the WebSocket, as written.
enum WriteWire {
    Plain(TcpStream),
    #[cfg(feature = "tls")]
    Tls(tls::WriteHalf),
}

impl WriteWire {
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        match self {
            WriteWire::Plain(stream) => stream.write_all(bytes),
            #[cfg(feature = "tls")]
            WriteWire::Tls(half) => half.write_all(bytes),
        }
    }

    /// Nothing more will be written.
    fn shutdown(&mut self) {
        match self {
            WriteWire::Plain(stream) => {
                let _ = stream.shutdown(Shutdown::Write);
            }
            #[cfg(feature = "tls")]
            WriteWire::Tls(half) => half.shutdown(),
        }
    }
}

/// A WebSocket's reading half: messages out of the socket's bytes.
pub(crate) struct WsReader {
    wire: ReadWire,
    parser: Parser,
}

impl WsReader {
    /// The next message already read, pings included, without touching the socket.
    pub(crate) fn message(&mut self) -> io::Result<Option<Message>> {
        self.parser.message().map_err(|v| bad(&format!("the server broke the protocol: {v:?}")))
    }

    /// Read at most `max` bytes, waiting up to `wait`: the bytes read.
    fn fill(&mut self, max: usize, wait: Duration) -> io::Result<usize> {
        let parser = &mut self.parser;
        self.wire.fill(max, wait, &mut |bytes| parser.feed(bytes))
    }

    /// The next message, waiting up to `wait` for bytes.
    pub(crate) fn next(&mut self, wait: Duration) -> io::Result<Option<Message>> {
        if let Some(m) = self.message()? {
            return Ok(Some(m));
        }
        self.fill(usize::MAX, wait)?;
        self.message()
    }

    /// The TCP socket under it (and under TLS, if any).
    pub(crate) fn socket(&self) -> &TcpStream {
        self.wire.socket()
    }
}

/// A WebSocket's writing half: each message one masked frame.
pub(crate) struct WsWriter {
    wire: WriteWire,
    rng: u64,
    frame: Vec<u8>,
}

impl WsWriter {
    fn mask(&mut self) -> [u8; 4] {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng as u32).to_le_bytes()
    }

    pub(crate) fn send_frame(&mut self, opcode: u8, payload: &[u8]) -> io::Result<()> {
        let mask = self.mask();
        self.frame.clear();
        ws::frame(&mut self.frame, opcode, payload, Some(mask));
        self.wire.write_all(&self.frame)
    }

    /// The close frame, then nothing more.
    pub(crate) fn close(&mut self, code: u16) {
        let _ = self.send_frame(CLOSE, &close_payload(code));
        self.wire.shutdown();
    }
}

pub struct WsClient {
    reader: WsReader,
    writer: WsWriter,
    /// Bytes read from the socket, frames and all (TLS's plaintext, under TLS).
    pub read_bytes: u64,
}

impl WsClient {
    /// Connect to `ws://host[:port]/path`, or `wss://` (with the `tls`
    /// feature, trusting the public roots), and complete the handshake.
    pub fn connect(url: &str, timeout: Duration) -> io::Result<WsClient> {
        WsClient::connect_trusting(url, timeout, &Trust::Public)
    }

    /// The same, with a `wss://` link's certificate verified against `trust`.
    pub fn connect_trusting(url: &str, timeout: Duration, trust: &Trust) -> io::Result<WsClient> {
        let t = target(url)?;
        let deadline = Instant::now() + timeout;
        let stream = dial(&t, deadline)?;
        stream.set_nodelay(true)?;
        stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
        let (read, write) = if t.secure {
            secure(stream, t.host, trust, deadline)?
        } else {
            (ReadWire::Plain(stream.try_clone()?, vec![0; CHUNK]), WriteWire::Plain(stream))
        };
        let mut seed = [0u8; 16];
        getrandom::fill(&mut seed).map_err(|_| bad("no random bytes"))?;
        let mut reader = WsReader { wire: read, parser: Parser::new(Role::Client, MAX_MESSAGE) };
        let mut writer =
            WsWriter { wire: write, rng: u64::from_le_bytes(seed[..8].try_into().expect("8 bytes")) | 1, frame: Vec::new() };
        let key = base64(&seed);
        let (authority, path) = (t.authority, t.path);
        let request = format!(
            "GET {path} HTTP/1.1\r\nHost: {authority}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
             Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
        );
        writer.wire.write_all(request.as_bytes())?;
        // The answer's head, and whatever came after it (the server's first
        // frames) for the parser.
        let mut head = Vec::new();
        let end = loop {
            if let Some(at) = head.windows(4).position(|w| w == b"\r\n\r\n") {
                break at + 4;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() || head.len() > MAX_HEAD {
                return Err(bad("no handshake answer"));
            }
            reader.wire.fill(usize::MAX, left, &mut |bytes| head.extend_from_slice(bytes))?;
        };
        let answer = String::from_utf8_lossy(&head[..end]);
        let accept = ws::accept_key(&key);
        let ok = answer.starts_with("HTTP/1.1 101")
            && answer.lines().any(|l| {
                l.split_once(':')
                    .is_some_and(|(n, v)| n.eq_ignore_ascii_case("sec-websocket-accept") && v.trim() == accept)
            });
        if !ok {
            let status = answer.lines().next().unwrap_or("").to_string();
            return Err(bad(&format!("the server did not accept the upgrade ({status})")));
        }
        reader.parser.feed(&head[end..]);
        Ok(WsClient { reader, writer, read_bytes: 0 })
    }

    pub fn send_frame(&mut self, opcode: u8, payload: &[u8]) -> io::Result<()> {
        self.writer.send_frame(opcode, payload)
    }

    /// One binary message.
    pub fn send(&mut self, payload: &[u8]) -> io::Result<()> {
        self.send_frame(BINARY, payload)
    }

    /// The next message already read, without touching the socket. Pings
    /// are answered and not returned.
    pub fn message(&mut self) -> io::Result<Option<Message>> {
        loop {
            match self.reader.message()? {
                Some(Message::Ping(p)) => self.send_frame(PONG, &p)?,
                other => return Ok(other),
            }
        }
    }

    /// Read at most `max` bytes from the socket, waiting up to `wait`.
    /// False when nothing came; an error once the socket is closed.
    pub fn fill(&mut self, max: usize, wait: Duration) -> io::Result<bool> {
        let n = self.reader.fill(max, wait)?;
        self.read_bytes += n as u64;
        Ok(n > 0)
    }

    /// The next message, waiting up to `wait` for it.
    pub fn recv(&mut self, wait: Duration) -> io::Result<Option<Message>> {
        let until = Instant::now() + wait;
        loop {
            if let Some(m) = self.message()? {
                return Ok(Some(m));
            }
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(None);
            }
            self.fill(usize::MAX, left)?;
        }
    }

    /// Say goodbye at the socket level and close.
    pub fn close(&mut self, code: u16) {
        self.writer.close(code);
    }

    /// The TCP socket (under TLS, if any).
    pub fn stream(&self) -> &TcpStream {
        self.reader.socket()
    }

    /// Two halves, for a thread that reads and another that writes.
    pub(crate) fn into_halves(self) -> (WsReader, WsWriter) {
        (self.reader, self.writer)
    }
}

#[cfg(feature = "tls")]
fn secure(stream: TcpStream, host: &str, trust: &Trust, deadline: Instant) -> io::Result<(ReadWire, WriteWire)> {
    let (read, write) = tls::handshake(stream, host, trust, deadline)?;
    Ok((ReadWire::Tls(read), WriteWire::Tls(write)))
}

#[cfg(not(feature = "tls"))]
fn secure(_: TcpStream, _: &str, _: &Trust, _: Instant) -> io::Result<(ReadWire, WriteWire)> {
    Err(io::Error::new(ErrorKind::Unsupported, "wss:// links need angel-serve built with its `tls` feature"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_share_link_is_its_socket_and_its_key() {
        let cases = [
            ("https://castle.ts.net/#ABC234", "wss://castle.ts.net/ws", "ABC234"),
            ("https://castle.ts.net#ABC234", "wss://castle.ts.net/ws", "ABC234"),
            ("http://192.168.1.5:8790/#KEY", "ws://192.168.1.5:8790/ws", "KEY"),
            ("http://192.168.1.5:8790/play/delve/#KEY", "ws://192.168.1.5:8790/ws", "KEY"),
            ("https://relay.example/s/abc?stats#KEY", "wss://relay.example/ws", "KEY"),
            ("  HTTP://192.168.1.5:8790/#key\n", "ws://192.168.1.5:8790/ws", "key"),
            ("HTTPS://CASTLE.TS.NET/#KEY", "wss://castle.ts.net/ws", "KEY"),
            ("http://[::1]:8790/#KEY", "ws://[::1]:8790/ws", "KEY"),
            ("wss://relay.example/room/7#KEY", "wss://relay.example/room/7", "KEY"),
            ("ws://127.0.0.1:8810#KEY", "ws://127.0.0.1:8810/ws", "KEY"),
        ];
        for (link, socket, key) in cases {
            assert_eq!(share_link(link), Ok((socket.to_string(), key.to_string())), "{link}");
        }
        for link in [
            "https://castle.ts.net/",
            "https://castle.ts.net/#",
            "castle.ts.net/#KEY",
            "ftp://x/#KEY",
            "https://u@x/#KEY",
            "http://x:port/#KEY",
            "https:///#KEY",
        ] {
            assert!(share_link(link).is_err(), "{link}");
        }
    }

    #[test]
    fn a_socket_url_is_its_host_port_and_path() {
        let t = target("wss://castle.ts.net/ws").unwrap();
        assert_eq!((t.secure, t.authority, t.host, t.port, t.path), (true, "castle.ts.net", "castle.ts.net", 443, "/ws"));
        let t = target("ws://[::1]:8810/ws").unwrap();
        assert_eq!((t.secure, t.authority, t.host, t.port, t.path), (false, "[::1]:8810", "::1", 8810, "/ws"));
        let t = target("ws://10.0.0.5").unwrap();
        assert_eq!((t.port, t.path), (80, "/"));
        assert!(target("http://x/ws").is_err());
    }
}
