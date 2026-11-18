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
use std::net::{Shutdown, TcpStream};
use std::time::{Duration, Instant};

const MAX_MESSAGE: usize = angel_net::proto::MAX_SERVER_MESSAGE;
const MAX_HEAD: usize = 4096;
const MIN_WAIT: Duration = Duration::from_micros(100);
const CHUNK: usize = 16 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Trust {
    #[default]
    Public,
    Only(Vec<Vec<u8>>),
}

fn bad(text: &str) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, text.to_string())
}

fn timed_out(e: &io::Error) -> bool {
    matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
}

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

struct Target<'a> {
    secure: bool,
    authority: &'a str,
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

mod resolver;
fn dial(t: &Target, deadline: Instant) -> io::Result<TcpStream> {
    let addrs = resolver::resolve(t.host,t.port,deadline)?;
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

enum ReadWire {
    Plain(TcpStream, Vec<u8>),
    #[cfg(feature = "tls")]
    Tls(tls::ReadHalf),
}

impl ReadWire {
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

pub(crate) struct WsReader {
    wire: ReadWire,
    parser: Parser,
}

impl WsReader {
    pub(crate) fn message(&mut self) -> io::Result<Option<Message>> {
        self.parser.message().map_err(|v| bad(&format!("the server broke the protocol: {v:?}")))
    }

    fn fill(&mut self, max: usize, wait: Duration) -> io::Result<usize> {
        let parser = &mut self.parser;
        self.wire.fill(max, wait, &mut |bytes| parser.feed(bytes))
    }

    pub(crate) fn next(&mut self, wait: Duration) -> io::Result<Option<Message>> {
        if let Some(m) = self.message()? {
            return Ok(Some(m));
        }
        self.fill(usize::MAX, wait)?;
        self.message()
    }

    pub(crate) fn socket(&self) -> &TcpStream {
        self.wire.socket()
    }
}

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

    pub(crate) fn close(&mut self, code: u16) {
        let _ = self.send_frame(CLOSE, &close_payload(code));
        self.wire.shutdown();
    }
}

pub struct WsClient {
    reader: WsReader,
    writer: WsWriter,
    pub read_bytes: u64,
}

impl WsClient {
    pub fn connect(url: &str, timeout: Duration) -> io::Result<WsClient> {
        WsClient::connect_trusting(url, timeout, &Trust::Public)
    }

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

    pub fn send(&mut self, payload: &[u8]) -> io::Result<()> {
        self.send_frame(BINARY, payload)
    }

    pub fn message(&mut self) -> io::Result<Option<Message>> {
        loop {
            match self.reader.message()? {
                Some(Message::Ping(p)) => self.send_frame(PONG, &p)?,
                other => return Ok(other),
            }
        }
    }

    pub fn fill(&mut self, max: usize, wait: Duration) -> io::Result<bool> {
        let n = self.reader.fill(max, wait)?;
        self.read_bytes += n as u64;
        Ok(n > 0)
    }

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

    pub fn close(&mut self, code: u16) {
        self.writer.close(code);
    }

    pub fn stream(&self) -> &TcpStream {
        self.reader.socket()
    }

    pub(crate) fn into_halves(self) -> (WsReader, WsWriter) {
        (self.reader, self.writer)
    }
}

pub fn http_get(url: &str, path: &str, trust: &Trust, timeout: Duration, max: usize) -> io::Result<Vec<u8>> {
    let t = target(url)?;
    if !path.starts_with('/') || path.bytes().any(|b| b <= b' ' || b == 0x7f) {
        return Err(bad("a path is one word from /"));
    }
    let deadline = Instant::now() + timeout;
    let stream = dial(&t, deadline)?;
    stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
    let (mut read, mut write) = if t.secure {
        secure(stream, t.host, trust, deadline)?
    } else {
        (ReadWire::Plain(stream.try_clone()?, vec![0; CHUNK]), WriteWire::Plain(stream))
    };
    let request = format!("GET {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", t.authority);
    write.write_all(request.as_bytes())?;
    let mut got = Vec::new();
    let mut shut = false;
    loop {
        if let Some(at) = got.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&got[..at]).into_owned();
            let status = head.lines().next().unwrap_or("");
            if !status.starts_with("HTTP/1.1 200") && !status.starts_with("HTTP/1.0 200") {
                return Err(io::Error::new(ErrorKind::NotFound, format!("the host answered {status}")));
            }
            let length = head.lines().find_map(|l| {
                let (name, value) = l.split_once(':')?;
                name.trim().eq_ignore_ascii_case("content-length").then(|| value.trim().parse::<usize>().ok())?
            });
            if length.is_some_and(|n| n > max) {
                return Err(bad("the body is too big"));
            }
            let body = &got[at + 4..];
            match length {
                Some(n) if body.len() >= n => return Ok(body[..n].to_vec()),
                None if shut => return Ok(body.to_vec()),
                _ => {}
            }
        }
        if shut {
            return Err(bad("the host closed before its answer was whole"));
        }
        if got.len() > max + MAX_HEAD {
            return Err(bad("the body is too big"));
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::Error::new(ErrorKind::TimedOut, "no answer in time"));
        }
        match read.fill(usize::MAX, left, &mut |bytes| got.extend_from_slice(bytes)) {
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => shut = true,
            Err(e) => return Err(e),
        }
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
