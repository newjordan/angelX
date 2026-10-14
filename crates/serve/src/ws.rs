//! WebSocket framing (RFC 6455), both ends, with no IO: bytes go into a
//! `Parser` and messages come out; `frame` writes one frame. Only binary
//! messages are spoken; a text frame is a violation. The server's reader
//! and the bots' client both read through this.

use crate::hash::{base64, is_base64_of, sha1};

pub const CONTINUATION: u8 = 0x0;
pub const TEXT: u8 = 0x1;
pub const BINARY: u8 = 0x2;
pub const CLOSE: u8 = 0x8;
pub const PING: u8 = 0x9;
pub const PONG: u8 = 0xA;

/// Close codes (RFC 6455 §7.4.1).
pub mod code {
    pub const NORMAL: u16 = 1000;
    pub const GOING_AWAY: u16 = 1001;
    pub const PROTOCOL: u16 = 1002;
    pub const UNSUPPORTED: u16 = 1003;
    pub const POLICY: u16 = 1008;
    pub const TOO_BIG: u16 = 1009;
    /// The engine's own: another page took this connection's seat (a
    /// reload, a second tab). A shell should not reconnect on it, or two
    /// tabs would take the seat in turns.
    pub const REPLACED: u16 = 4000;
}

/// A control frame's payload is at most this (RFC 6455 §5.5).
pub const MAX_CONTROL: usize = 125;

/// The GUID every accept key is made with (RFC 6455 §1.3).
const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// The handshake's `Sec-WebSocket-Accept` for a client's key.
pub fn accept_key(key: &str) -> String {
    base64(&sha1(format!("{key}{GUID}").as_bytes()))
}

/// A client's `Sec-WebSocket-Key` is 16 random bytes in base64.
pub fn valid_key(key: &str) -> bool {
    is_base64_of(key, 16)
}

/// Which end is reading: a server takes only masked frames, a client only
/// unmasked ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Server,
    Client,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Binary(Vec<u8>),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    /// The peer is closing, with its code if it gave one.
    Close(Option<u16>),
}

/// How a peer broke the protocol; the connection closes with `code()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Violation {
    /// A client frame without a mask, or a server frame with one.
    Mask,
    /// Reserved bits set, an unknown opcode, or a length's top bit set.
    Reserved,
    /// A text frame: only binary is spoken.
    Text,
    /// A message past the limit.
    TooBig,
    /// A control frame fragmented or longer than 125 bytes.
    Control,
    /// A continuation with no message started, or a new message inside one.
    Continuation,
    /// A close frame with a one-byte payload or a code no peer may send.
    Close,
}

impl Violation {
    pub fn code(self) -> u16 {
        match self {
            Violation::Text => code::UNSUPPORTED,
            Violation::TooBig => code::TOO_BIG,
            _ => code::PROTOCOL,
        }
    }
}

/// Bytes in, messages out.
pub struct Parser {
    role: Role,
    max: usize,
    buf: Vec<u8>,
    at: usize,
    /// A binary message whose later fragments have not all come.
    partial: Option<Vec<u8>>,
}

impl Parser {
    /// `max` bounds a whole message, fragments together.
    pub fn new(role: Role, max: usize) -> Parser {
        Parser { role, max, buf: Vec::new(), at: 0, partial: None }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        if self.at > 0 && self.at == self.buf.len() {
            self.buf.clear();
            self.at = 0;
        } else if self.at > 4096 {
            self.buf.drain(..self.at);
            self.at = 0;
        }
        self.buf.extend_from_slice(bytes);
    }

    /// Bytes fed but not yet read as a frame.
    pub fn buffered(&self) -> usize {
        self.buf.len() - self.at
    }

    /// The next whole message, `None` until enough bytes have come. A
    /// frame's length is checked against the limit before its payload is
    /// waited for, so no peer can make the buffer grow past one message.
    pub fn message(&mut self) -> Result<Option<Message>, Violation> {
        loop {
            let rest = &self.buf[self.at..];
            if rest.len() < 2 {
                return Ok(None);
            }
            let (b0, b1) = (rest[0], rest[1]);
            let fin = b0 & 0x80 != 0;
            let opcode = b0 & 0x0f;
            if b0 & 0x70 != 0 {
                return Err(Violation::Reserved);
            }
            let masked = b1 & 0x80 != 0;
            if masked != (self.role == Role::Server) {
                return Err(Violation::Mask);
            }
            let control = opcode & 0x8 != 0;
            let (len, mut header) = match b1 & 0x7f {
                126 => {
                    if rest.len() < 4 {
                        return Ok(None);
                    }
                    (u64::from(u16::from_be_bytes([rest[2], rest[3]])), 4)
                }
                127 => {
                    if rest.len() < 10 {
                        return Ok(None);
                    }
                    let mut n = [0u8; 8];
                    n.copy_from_slice(&rest[2..10]);
                    let n = u64::from_be_bytes(n);
                    if n >> 63 != 0 {
                        return Err(Violation::Reserved);
                    }
                    (n, 10)
                }
                n => (u64::from(n), 2),
            };
            if control && (!fin || len > MAX_CONTROL as u64) {
                return Err(Violation::Control);
            }
            match opcode {
                CONTINUATION | BINARY | CLOSE | PING | PONG => {}
                TEXT => return Err(Violation::Text),
                _ => return Err(Violation::Reserved),
            }
            let so_far = self.partial.as_ref().map_or(0, Vec::len) as u64;
            if !control && so_far + len > self.max as u64 {
                return Err(Violation::TooBig);
            }
            let mask = if masked {
                if rest.len() < header + 4 {
                    return Ok(None);
                }
                let m = [rest[header], rest[header + 1], rest[header + 2], rest[header + 3]];
                header += 4;
                Some(m)
            } else {
                None
            };
            let len = len as usize;
            if rest.len() < header + len {
                return Ok(None);
            }
            let mut payload = rest[header..header + len].to_vec();
            if let Some(m) = mask {
                for (i, b) in payload.iter_mut().enumerate() {
                    *b ^= m[i & 3];
                }
            }
            self.at += header + len;
            match opcode {
                BINARY => {
                    if self.partial.is_some() {
                        return Err(Violation::Continuation);
                    }
                    if fin {
                        return Ok(Some(Message::Binary(payload)));
                    }
                    self.partial = Some(payload);
                }
                CONTINUATION => {
                    let Some(mut message) = self.partial.take() else {
                        return Err(Violation::Continuation);
                    };
                    message.extend_from_slice(&payload);
                    if fin {
                        return Ok(Some(Message::Binary(message)));
                    }
                    self.partial = Some(message);
                }
                PING => return Ok(Some(Message::Ping(payload))),
                PONG => return Ok(Some(Message::Pong(payload))),
                _ => {
                    return match payload.len() {
                        0 => Ok(Some(Message::Close(None))),
                        1 => Err(Violation::Close),
                        _ => {
                            let code = u16::from_be_bytes([payload[0], payload[1]]);
                            if sendable(code) { Ok(Some(Message::Close(Some(code)))) } else { Err(Violation::Close) }
                        }
                    };
                }
            }
        }
    }
}

/// Close codes a peer may put on the wire (RFC 6455 §7.4).
fn sendable(code: u16) -> bool {
    matches!(code, 1000..=1003 | 1007..=1014 | 3000..=4999)
}

/// One whole frame. A client masks with `mask`; a server passes `None`.
pub fn frame(out: &mut Vec<u8>, opcode: u8, payload: &[u8], mask: Option<[u8; 4]>) {
    out.push(0x80 | opcode);
    let bit = if mask.is_some() { 0x80 } else { 0 };
    match payload.len() {
        n if n < 126 => out.push(bit | n as u8),
        n if n <= usize::from(u16::MAX) => {
            out.push(bit | 126);
            out.extend_from_slice(&(n as u16).to_be_bytes());
        }
        n => {
            out.push(bit | 127);
            out.extend_from_slice(&(n as u64).to_be_bytes());
        }
    }
    match mask {
        Some(m) => {
            out.extend_from_slice(&m);
            out.extend(payload.iter().enumerate().map(|(i, b)| b ^ m[i & 3]));
        }
        None => out.extend_from_slice(payload),
    }
}

/// A close frame's payload: the code, no reason.
pub fn close_payload(code: u16) -> [u8; 2] {
    code.to_be_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MASK: Option<[u8; 4]> = Some([0x37, 0xfa, 0x21, 0x3d]);

    fn client_frame(opcode: u8, fin: bool, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        frame(&mut out, opcode, payload, MASK);
        if !fin {
            out[0] &= 0x7f;
        }
        out
    }

    fn server() -> Parser {
        Parser::new(Role::Server, 1024)
    }

    #[test]
    fn the_rfc_handshake_example_accepts() {
        assert_eq!(accept_key("dGhlIHNhbXBsZSBub25jZQ=="), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
        assert!(valid_key("dGhlIHNhbXBsZSBub25jZQ=="));
        assert!(!valid_key("short"));
    }

    #[test]
    fn a_masked_binary_message_reads_back_byte_by_byte() {
        let mut p = server();
        let bytes = client_frame(BINARY, true, b"hello knights");
        for (i, b) in bytes.iter().enumerate() {
            assert_eq!(p.message(), Ok(None), "after {i} bytes");
            p.feed(&[*b]);
        }
        assert_eq!(p.message(), Ok(Some(Message::Binary(b"hello knights".to_vec()))));
        assert_eq!(p.message(), Ok(None));
    }

    #[test]
    fn the_three_length_forms_read() {
        for len in [0usize, 125, 126, 1000, 65_535, 65_536, 70_000] {
            let mut p = Parser::new(Role::Server, 100_000);
            let payload: Vec<u8> = (0..len).map(|i| i as u8).collect();
            p.feed(&client_frame(BINARY, true, &payload));
            assert_eq!(p.message(), Ok(Some(Message::Binary(payload))), "{len}");
        }
    }

    #[test]
    fn fragments_join_with_control_frames_between() {
        let mut p = server();
        p.feed(&client_frame(BINARY, false, b"one "));
        p.feed(&client_frame(PING, true, b"p"));
        p.feed(&client_frame(CONTINUATION, false, b"two "));
        p.feed(&client_frame(CONTINUATION, true, b"three"));
        assert_eq!(p.message(), Ok(Some(Message::Ping(b"p".to_vec()))));
        assert_eq!(p.message(), Ok(Some(Message::Binary(b"one two three".to_vec()))));
    }

    #[test]
    fn ping_pong_and_close_read() {
        let mut p = server();
        p.feed(&client_frame(PING, true, b"are you there"));
        p.feed(&client_frame(PONG, true, b""));
        p.feed(&client_frame(CLOSE, true, &close_payload(code::GOING_AWAY)));
        p.feed(&client_frame(CLOSE, true, b""));
        assert_eq!(p.message(), Ok(Some(Message::Ping(b"are you there".to_vec()))));
        assert_eq!(p.message(), Ok(Some(Message::Pong(vec![]))));
        assert_eq!(p.message(), Ok(Some(Message::Close(Some(code::GOING_AWAY)))));
        assert_eq!(p.message(), Ok(Some(Message::Close(None))));
    }

    #[test]
    fn an_unmasked_client_frame_is_refused() {
        let mut p = server();
        let mut out = Vec::new();
        frame(&mut out, BINARY, b"x", None);
        p.feed(&out);
        assert_eq!(p.message(), Err(Violation::Mask));
        // And a client refuses a masked frame from a server.
        let mut c = Parser::new(Role::Client, 1024);
        c.feed(&client_frame(BINARY, true, b"x"));
        assert_eq!(c.message(), Err(Violation::Mask));
    }

    #[test]
    fn an_oversize_message_is_refused_before_its_payload_arrives() {
        let mut p = server();
        let big = client_frame(BINARY, true, &[0u8; 1025]);
        p.feed(&big[..8]);
        assert_eq!(p.message(), Err(Violation::TooBig));

        // Fragments past the limit together.
        let mut p = server();
        p.feed(&client_frame(BINARY, false, &[0u8; 600]));
        p.feed(&client_frame(CONTINUATION, true, &[0u8; 600]));
        assert_eq!(p.message(), Err(Violation::TooBig));

        // A 64-bit length is read whole and refused, not allocated.
        let mut p = server();
        p.feed(&[0x82, 0x80 | 127, 0, 0, 0, 1, 0, 0, 0, 0, 1, 2, 3, 4]);
        assert_eq!(p.message(), Err(Violation::TooBig));
        let mut p = server();
        p.feed(&[0x82, 0x80 | 127, 0x80, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4]);
        assert_eq!(p.message(), Err(Violation::Reserved));
    }

    #[test]
    fn text_reserved_bits_and_bad_control_frames_are_refused() {
        let cases: Vec<(Vec<u8>, Violation)> = vec![
            (client_frame(TEXT, true, b"hi"), Violation::Text),
            (
                {
                    let mut f = client_frame(BINARY, true, b"x");
                    f[0] |= 0x40;
                    f
                },
                Violation::Reserved,
            ),
            (client_frame(0x3, true, b""), Violation::Reserved),
            (client_frame(PING, false, b""), Violation::Control),
            (client_frame(PING, true, &[0u8; 126]), Violation::Control),
            (client_frame(CONTINUATION, true, b"x"), Violation::Continuation),
            (client_frame(CLOSE, true, &[3]), Violation::Close),
            (client_frame(CLOSE, true, &close_payload(1005)), Violation::Close),
        ];
        for (bytes, violation) in cases {
            let mut p = server();
            p.feed(&bytes);
            assert_eq!(p.message(), Err(violation), "{bytes:?}");
        }
        // A new message inside a fragmented one.
        let mut p = server();
        p.feed(&client_frame(BINARY, false, b"a"));
        p.feed(&client_frame(BINARY, true, b"b"));
        assert_eq!(p.message(), Err(Violation::Continuation));
    }

    #[test]
    fn server_frames_read_on_the_client() {
        let mut c = Parser::new(Role::Client, 1 << 20);
        let mut out = Vec::new();
        frame(&mut out, BINARY, &[7u8; 300], None);
        frame(&mut out, PING, b"", None);
        c.feed(&out);
        assert_eq!(c.message(), Ok(Some(Message::Binary(vec![7u8; 300]))));
        assert_eq!(c.message(), Ok(Some(Message::Ping(vec![]))));
    }
}
