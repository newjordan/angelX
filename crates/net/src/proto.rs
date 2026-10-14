//! The messages, both ways. One WebSocket binary message is one message
//! here; its first byte is the kind. Browsers and the native angelX client
//! speak exactly this, on `GET /ws`.
//!
//! Client to server:
//!
//! | kind | message | what |
//! |---|---|---|
//! | 0x01 | `Hello` | protocol, game, join key or seat token, name, caps |
//! | 0x02 | `Cmds` | the newest snapshot tick applied, then commands (seq, game bytes) |
//! | 0x03 | `Ping` | the client's clock, echoed back |
//! | 0x04 | `Action` | a one-off choice (a vote, a wish, a chat line): kind and game bytes |
//! | 0x05 | `Bye` | leaving on purpose (the seat is freed, not held for resume) |
//!
//! Server to client:
//!
//! | kind | message | what |
//! |---|---|---|
//! | 0x81 | `Welcome` | the seat (or spectator), its resume token, the rates, the tick, game params |
//! | 0x82 | `Reject` | why not, then the connection closes |
//! | 0x83 | `Snapshot` | tick, the last command seq this seat's step consumed, whole or patch, game body |
//! | 0x84 | `Events` | tick, then game-defined events (sounds, cues, notices) |
//! | 0x85 | `Pong` | the client's clock echoed, and the server's |
//! | 0x86 | `Notice` | a line from the host to show |
//! | 0x87 | `Knobs` | the server's knobs as JSON (`knob::table_json`): at the welcome and whenever one moves |
//! | 0x88 | `Stats` | the server's numbers for this seat as JSON, once a second, to a client that asked (`knob::action::STATS`) |
//!
//! Action kinds from `knob::action::ENGINE` up are the engine's own (a
//! knob set, the stats asked for): the server takes them and never hands
//! them to the game.

use crate::codec::{Bytes, Malformed, put_blob, put_f64, put_str, put_varint};

/// Raised whenever a message's layout changes; both ends refuse a mismatch.
pub const PROTOCOL: u32 = 3;

/// Nothing a client sends may be longer (a `Hello` with a long name included).
pub const MAX_CLIENT_MESSAGE: usize = 1024;
/// A whole document snapshot of the Delve is a few hundred KB; nothing is
/// allowed near this.
pub const MAX_SERVER_MESSAGE: usize = 8 << 20;
/// Join keys and seat tokens: 128 bits in base32 is 26 characters.
pub const MAX_KEY: usize = 64;
/// Bytes, not characters.
pub const MAX_NAME: usize = 48;
pub const MAX_GAME: usize = 32;
/// Commands in one `Cmds` (a tab that was busy sends what piled up).
pub const MAX_CMDS: usize = 8;
pub const MAX_CMD: usize = 96;
pub const MAX_EVENTS: usize = 256;
/// The knob table and the stats, as JSON text.
pub const MAX_JSON: usize = 64 * 1024;

/// What a client can do, sent in its `Hello`.
pub mod caps {
    /// A touch screen: show the touch controls.
    pub const TOUCH: u32 = 1;
    /// WebGL is there for presenting.
    pub const WEBGL: u32 = 2;
    /// An angelX terminal, not a browser.
    pub const NATIVE: u32 = 4;
    /// Only watching: never take a seat.
    pub const WATCH: u32 = 8;
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToServer<'a> {
    Hello {
        protocol: u32,
        game: &'a str,
        /// The session's join key, from the link.
        join: &'a str,
        /// A seat token from an earlier `Welcome` to take that seat back, or empty.
        resume: &'a str,
        name: &'a str,
        caps: u32,
    },
    Cmds {
        /// The newest snapshot tick the client has applied.
        applied: u32,
        cmds: Vec<(u32, &'a [u8])>,
    },
    Ping {
        t: f64,
    },
    Action {
        kind: u32,
        body: &'a [u8],
    },
    Bye,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Reject {
    /// The client speaks another protocol version: reload the page.
    Protocol = 1,
    /// The join key or seat token is not this session's.
    Key = 2,
    /// Every seat and every watcher's place is taken.
    Full = 3,
    /// The host has closed the session to newcomers.
    Locked = 4,
    /// The host removed this player.
    Kicked = 5,
    /// The host is shutting the session down.
    Stopping = 6,
    /// The client asked for a game this session is not playing.
    Game = 7,
    /// The client broke a limit (too fast, too long, malformed).
    Abuse = 8,
}

impl Reject {
    pub fn from_code(code: u8) -> Option<Reject> {
        Some(match code {
            1 => Reject::Protocol,
            2 => Reject::Key,
            3 => Reject::Full,
            4 => Reject::Locked,
            5 => Reject::Kicked,
            6 => Reject::Stopping,
            7 => Reject::Game,
            8 => Reject::Abuse,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Welcome<'a> {
    pub protocol: u32,
    pub game: &'a str,
    /// `None`: watching, no seat.
    pub seat: Option<u32>,
    /// Keep this to take the seat back after a dropped connection.
    pub seat_token: &'a str,
    pub tick_hz: u32,
    pub snapshot_hz: u32,
    /// The server's tick when this was written.
    pub tick: u32,
    /// Whatever the game's client needs before its first snapshot.
    pub params: &'a [u8],
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToClient<'a> {
    Welcome(Welcome<'a>),
    Reject {
        why: Reject,
        text: &'a str,
    },
    Snapshot {
        tick: u32,
        /// The last command seq this seat's step consumed (0: none yet).
        ack: u32,
        /// Whole state, or a patch against the last snapshot this client got.
        whole: bool,
        body: &'a [u8],
    },
    Events {
        tick: u32,
        events: Vec<&'a [u8]>,
    },
    Pong {
        t: f64,
        server_ms: f64,
    },
    Notice {
        text: &'a str,
    },
    /// The server's knobs (`knob::table_json`).
    Knobs {
        json: &'a str,
    },
    /// The server's numbers for this seat, once a second, when asked.
    Stats {
        json: &'a str,
    },
}

const HELLO: u8 = 0x01;
const CMDS: u8 = 0x02;
const PING: u8 = 0x03;
const ACTION: u8 = 0x04;
const BYE: u8 = 0x05;

const WELCOME: u8 = 0x81;
const REJECT: u8 = 0x82;
const SNAPSHOT: u8 = 0x83;
const EVENTS: u8 = 0x84;
const PONG: u8 = 0x85;
const NOTICE: u8 = 0x86;
const KNOBS: u8 = 0x87;
const STATS: u8 = 0x88;

impl<'a> ToServer<'a> {
    pub fn encode(&self, out: &mut Vec<u8>) {
        match self {
            ToServer::Hello { protocol, game, join, resume, name, caps } => {
                out.push(HELLO);
                put_varint(out, u64::from(*protocol));
                put_str(out, game);
                put_str(out, join);
                put_str(out, resume);
                put_str(out, name);
                put_varint(out, u64::from(*caps));
            }
            ToServer::Cmds { applied, cmds } => {
                out.push(CMDS);
                put_varint(out, u64::from(*applied));
                put_varint(out, cmds.len() as u64);
                for (seq, body) in cmds {
                    put_varint(out, u64::from(*seq));
                    put_blob(out, body);
                }
            }
            ToServer::Ping { t } => {
                out.push(PING);
                put_f64(out, *t);
            }
            ToServer::Action { kind, body } => {
                out.push(ACTION);
                put_varint(out, u64::from(*kind));
                put_blob(out, body);
            }
            ToServer::Bye => out.push(BYE),
        }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.encode(&mut out);
        out
    }

    /// Read one client message. Anything longer than `MAX_CLIENT_MESSAGE`,
    /// with bytes left over, or over any limit is malformed.
    pub fn decode(bytes: &'a [u8]) -> Result<ToServer<'a>, Malformed> {
        if bytes.len() > MAX_CLIENT_MESSAGE {
            return Err(Malformed);
        }
        let mut b = Bytes::new(bytes);
        let msg = match b.byte()? {
            HELLO => ToServer::Hello {
                protocol: b.u32()?,
                game: b.str(MAX_GAME)?,
                join: b.str(MAX_KEY)?,
                resume: b.str(MAX_KEY)?,
                name: b.str(MAX_NAME)?,
                caps: b.u32()?,
            },
            CMDS => {
                let applied = b.u32()?;
                let n = b.varint()?;
                if n > MAX_CMDS as u64 {
                    return Err(Malformed);
                }
                let mut cmds = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    cmds.push((b.u32()?, b.blob(MAX_CMD)?));
                }
                ToServer::Cmds { applied, cmds }
            }
            PING => ToServer::Ping { t: b.f64()? },
            ACTION => ToServer::Action { kind: b.u32()?, body: b.blob(MAX_CLIENT_MESSAGE)? },
            BYE => ToServer::Bye,
            _ => return Err(Malformed),
        };
        b.end()?;
        Ok(msg)
    }
}

impl<'a> ToClient<'a> {
    pub fn encode(&self, out: &mut Vec<u8>) {
        match self {
            ToClient::Welcome(w) => {
                out.push(WELCOME);
                put_varint(out, u64::from(w.protocol));
                put_str(out, w.game);
                // Seat + 1, so 0 is "watching".
                put_varint(out, w.seat.map_or(0, |s| u64::from(s) + 1));
                put_str(out, w.seat_token);
                put_varint(out, u64::from(w.tick_hz));
                put_varint(out, u64::from(w.snapshot_hz));
                put_varint(out, u64::from(w.tick));
                put_blob(out, w.params);
            }
            ToClient::Reject { why, text } => {
                out.push(REJECT);
                out.push(*why as u8);
                put_str(out, text);
            }
            ToClient::Snapshot { tick, ack, whole, body } => {
                out.push(SNAPSHOT);
                put_varint(out, u64::from(*tick));
                put_varint(out, u64::from(*ack));
                out.push(u8::from(*whole));
                out.extend_from_slice(body);
            }
            ToClient::Events { tick, events } => {
                out.push(EVENTS);
                put_varint(out, u64::from(*tick));
                put_varint(out, events.len() as u64);
                for event in events {
                    put_blob(out, event);
                }
            }
            ToClient::Pong { t, server_ms } => {
                out.push(PONG);
                put_f64(out, *t);
                put_f64(out, *server_ms);
            }
            ToClient::Notice { text } => {
                out.push(NOTICE);
                put_str(out, text);
            }
            ToClient::Knobs { json } => {
                out.push(KNOBS);
                put_str(out, json);
            }
            ToClient::Stats { json } => {
                out.push(STATS);
                put_str(out, json);
            }
        }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.encode(&mut out);
        out
    }

    pub fn decode(bytes: &'a [u8]) -> Result<ToClient<'a>, Malformed> {
        if bytes.len() > MAX_SERVER_MESSAGE {
            return Err(Malformed);
        }
        let mut b = Bytes::new(bytes);
        let msg = match b.byte()? {
            WELCOME => ToClient::Welcome(Welcome {
                protocol: b.u32()?,
                game: b.str(MAX_GAME)?,
                seat: match b.u32()? {
                    0 => None,
                    s => Some(s - 1),
                },
                seat_token: b.str(MAX_KEY)?,
                tick_hz: b.u32()?,
                snapshot_hz: b.u32()?,
                tick: b.u32()?,
                params: b.blob(MAX_SERVER_MESSAGE)?,
            }),
            REJECT => ToClient::Reject {
                why: Reject::from_code(b.byte()?).ok_or(Malformed)?,
                text: b.str(1024)?,
            },
            SNAPSHOT => {
                let tick = b.u32()?;
                let ack = b.u32()?;
                let whole = match b.byte()? {
                    0 => false,
                    1 => true,
                    _ => return Err(Malformed),
                };
                // The body runs to the end of the message.
                let body = b.take(b.rest.len())?;
                return Ok(ToClient::Snapshot { tick, ack, whole, body });
            }
            EVENTS => {
                let tick = b.u32()?;
                let n = b.varint()?;
                if n > MAX_EVENTS as u64 {
                    return Err(Malformed);
                }
                let mut events = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    events.push(b.blob(MAX_SERVER_MESSAGE)?);
                }
                ToClient::Events { tick, events }
            }
            PONG => ToClient::Pong { t: b.f64()?, server_ms: b.f64()? },
            NOTICE => ToClient::Notice { text: b.str(4096)? },
            KNOBS => ToClient::Knobs { json: b.str(MAX_JSON)? },
            STATS => ToClient::Stats { json: b.str(MAX_JSON)? },
            _ => return Err(Malformed),
        };
        b.end()?;
        Ok(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server_round_trip(msg: ToServer) {
        let bytes = msg.to_vec();
        assert_eq!(ToServer::decode(&bytes), Ok(msg));
    }

    fn client_round_trip(msg: ToClient) {
        let bytes = msg.to_vec();
        assert_eq!(ToClient::decode(&bytes), Ok(msg));
    }

    #[test]
    fn every_client_message_reads_back_as_written() {
        server_round_trip(ToServer::Hello {
            protocol: PROTOCOL,
            game: "delve",
            join: "abcdefghijklmnopqrstuvwxyz",
            resume: "",
            name: "Gwen of the Weald",
            caps: caps::TOUCH | caps::WEBGL,
        });
        server_round_trip(ToServer::Cmds { applied: 912, cmds: vec![(41, &[1, 2, 3]), (42, &[])] });
        server_round_trip(ToServer::Ping { t: 1234.5 });
        server_round_trip(ToServer::Action { kind: 3, body: b"yes" });
        server_round_trip(ToServer::Bye);
    }

    #[test]
    fn every_server_message_reads_back_as_written() {
        client_round_trip(ToClient::Welcome(Welcome {
            protocol: PROTOCOL,
            game: "muster",
            seat: Some(0),
            seat_token: "token",
            tick_hz: 30,
            snapshot_hz: 20,
            tick: 7,
            params: &[9, 9],
        }));
        client_round_trip(ToClient::Welcome(Welcome {
            protocol: PROTOCOL,
            game: "muster",
            seat: None,
            seat_token: "",
            tick_hz: 30,
            snapshot_hz: 30,
            tick: 0,
            params: &[],
        }));
        client_round_trip(ToClient::Reject { why: Reject::Full, text: "the field is full" });
        client_round_trip(ToClient::Snapshot { tick: 100, ack: 99, whole: true, body: &[1, 2, 3, 4] });
        client_round_trip(ToClient::Snapshot { tick: 101, ack: 99, whole: false, body: &[] });
        client_round_trip(ToClient::Events { tick: 5, events: vec![b"gold", b"roll"] });
        client_round_trip(ToClient::Pong { t: 1.0, server_ms: 2.0 });
        client_round_trip(ToClient::Notice { text: "the host locked the door" });
        client_round_trip(ToClient::Knobs { json: "{\"dev\":false,\"may\":true,\"knobs\":[]}" });
        client_round_trip(ToClient::Stats { json: "{\"queued\":1}" });
    }

    #[test]
    fn a_client_cannot_send_past_its_limits() {
        let mut long = vec![0u8; MAX_CLIENT_MESSAGE + 1];
        long[0] = CMDS;
        assert_eq!(ToServer::decode(&long), Err(Malformed));

        let name = "n".repeat(MAX_NAME + 1);
        let hello = ToServer::Hello { protocol: 2, game: "g", join: "k", resume: "", name: &name, caps: 0 };
        assert_eq!(ToServer::decode(&hello.to_vec()), Err(Malformed));

        let cmds: Vec<(u32, &[u8])> = (0..MAX_CMDS as u32 + 1).map(|s| (s, &[][..])).collect();
        let too_many = ToServer::Cmds { applied: 0, cmds };
        assert_eq!(ToServer::decode(&too_many.to_vec()), Err(Malformed));

        let mut trailing = ToServer::Bye.to_vec();
        trailing.push(0);
        assert_eq!(ToServer::decode(&trailing), Err(Malformed));
        assert_eq!(ToServer::decode(&[0x7f]), Err(Malformed));
        assert_eq!(ToServer::decode(&[]), Err(Malformed));
    }
}
