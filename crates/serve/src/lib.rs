//! The engine's server. One listener speaks HTTP/1.1 and WebSocket on
//! blocking std sockets: the shell page and the client's files, and `GET
//! /ws` for every client, browser and angelX alike. A game implements
//! `GameServer`; `Server::start` runs it.
//!
//! Threads, and why:
//!
//! - **One accept thread.** Each accepted connection gets its own thread,
//!   which reads its HTTP requests and, once it upgrades, its WebSocket
//!   frames. It starts a **writer thread** with the other half of the socket
//!   (`try_clone`). Blocking sockets and a thread pair per client are
//!   portable to Linux, macOS and Windows hosts, simple to reason about, and
//!   fine to the cap of 256 players and 32 watchers.
//! - **The sim thread** steps the game at its fixed tick and never touches
//!   a socket. Commands reach it through a channel, already decoded by the
//!   readers; after each tick it publishes the tick once, behind an `Arc`,
//!   and wakes the writers. A host that keeps its own clock, and tests that
//!   step a game exactly, step the same sim by hand instead
//!   (`Server::start_manual`).
//! - **Each writer** builds its own client's snapshot from the newest
//!   published tick, against what it last wrote to that client, so encoding
//!   spreads across cores. A slow client's writer blocks on its own socket,
//!   or holds back while its client is behind on what it was sent,
//!   and skips to the newest tick: it never slows the sim or anyone else.
//! - **The host's own connections** (`Server::connect_local`) get the same
//!   reader and writer threads, on an in-process pipe instead of a socket
//!   (`local`), so the host's terminal is a client like any other.
//!
//! The native client's side is here too (`client`): `WsClient`, and
//! `NativeLink`, which plays a game from a terminal over a socket or the
//! host's own pipe.
//!
//! There is no Origin check on `/ws`: the join key (or a seat token) is
//! the authority, and the static client may later be served from a CDN or
//! a relay on another origin.

pub mod client;
mod conn;
pub mod hash;
pub mod http;
pub mod link;
pub mod local;
mod pace;
pub mod rep;
mod server;
pub mod session;
mod sim;
pub mod stats;
mod tune;
pub mod ws;

pub use http::ClientBundle;
pub use local::{Closed, LocalConn};
pub use rep::{DocRep, DocTick, DocView, EntityRep, EntityTick, Everything};
pub use server::{ServeConfig, Server};
pub use session::Player;
pub use sim::Manual;
pub use stats::{Spread, Stats, Totals};

use angel_net::cmd::Command;
use angel_net::knob::Knob;

/// The limits every connection lives within. Fixed numbers, changed only
/// from measurement (see `docs/ENGINE.md`).
pub mod limits {
    use std::time::Duration;

    pub use crate::session::{MAX_PLAYERS, MAX_SPECTATORS, RESUME_WINDOW};
    pub use angel_net::proto::MAX_CLIENT_MESSAGE;

    /// A request head, or the WebSocket handshake, is at most this long.
    pub const MAX_HANDSHAKE: usize = 4096;
    /// ...and must arrive within this long; so must the `Hello` after it.
    pub const HANDSHAKE_TIME: Duration = Duration::from_secs(5);
    /// Messages (and control frames) a client may send in a second.
    pub const MAX_MESSAGES_PER_SEC: u32 = 120;
    /// The server pings each client this often.
    pub const PING_EVERY: Duration = Duration::from_secs(1);
    /// A client heard nothing from for this long is closed.
    pub const SILENT_CLOSE: Duration = Duration::from_secs(10);
    /// A write that makes no progress for this long closes the connection.
    pub const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
    /// Connections not yet past their hello (page loads, and sockets
    /// waiting for their hello), at most; more are answered 503, so a flood
    /// of half-open sockets cannot pile up threads without end. A class of
    /// 200 phones scanning the code at once holds two or three each.
    pub const MAX_PENDING: usize = 512;
    /// A kept-alive HTTP connection waits this long for its next request:
    /// a page's files follow it at once, and an idle one should not hold a
    /// place a newcomer needs.
    pub const KEEP_ALIVE_IDLE: Duration = Duration::from_secs(2);
    /// A writer holds snapshots back while more bytes are unapplied by its
    /// client (by the `applied` tick in its commands) than its window: a
    /// window that grows while snapshots are applied within `QUEUE` of the
    /// client's least delay, and halves when one is later than that (see
    /// `pace`). A slow link then carries about one snapshot of state, not a
    /// queue of stale state, and a far but fast client is not held.
    pub const QUEUE: Duration = Duration::from_millis(250);
    /// The window's floor and start: one full entity snapshot
    /// (`angel_net::entity::BUDGET`).
    pub const MIN_IN_FLIGHT: usize = 1200;
    /// The window's ceiling: seconds of the busiest stream, so a link that
    /// slows down is followed within a few halvings.
    pub const MAX_IN_FLIGHT: usize = 64 * 1024;
    /// A client's least delay is read over this long, so a move to a
    /// slower network is followed within it.
    pub const DELAY_WINDOW: Duration = Duration::from_secs(5);
    /// Ticks of events kept for writers that fell behind.
    pub const EVENT_RING: usize = 64;
    /// After the server sends a close, it waits this long for the client's.
    pub const CLOSE_WAIT: Duration = Duration::from_secs(2);
}

/// A game's fixed rates. `tick_hz` must be a whole multiple of `snapshot_hz`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rates {
    pub tick_hz: u32,
    pub snapshot_hz: u32,
}

/// Who hears an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum To {
    All,
    Seat(u32),
    /// Clients whose interest covers this point (`Replicate::hears`).
    Near(i32, i32),
}

/// A sound, a cue, a notice: game-defined bytes, sent in tick order with
/// the tick's snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub to: To,
    pub bytes: Vec<u8>,
}

/// One tick's events, as the game makes them.
#[derive(Clone, Debug, Default)]
pub struct Events {
    list: Vec<Event>,
}

impl Events {
    pub fn all(&mut self, bytes: impl Into<Vec<u8>>) {
        self.list.push(Event { to: To::All, bytes: bytes.into() });
    }

    pub fn seat(&mut self, seat: u32, bytes: impl Into<Vec<u8>>) {
        self.list.push(Event { to: To::Seat(seat), bytes: bytes.into() });
    }

    pub fn near(&mut self, x: i32, y: i32, bytes: impl Into<Vec<u8>>) {
        self.list.push(Event { to: To::Near(x, y), bytes: bytes.into() });
    }

    pub fn list(&self) -> &[Event] {
        &self.list
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    fn take(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.list)
    }
}

/// How a game's published ticks become each client's snapshots. Shared by
/// every writer thread (behind an `Arc`); everything here runs on the
/// writers, never on the sim.
pub trait Replicate: Send + Sync + 'static {
    /// One published tick, read by every writer.
    type Tick: Send + Sync + 'static;
    /// What one client holds, kept by its writer for the connection's life.
    type Viewer: Send + 'static;

    fn viewer(&self, seat: Option<u32>) -> Self::Viewer;
    /// Write this client's snapshot body of `tick` against what it holds;
    /// returns whether the body is whole (the client starts over).
    fn snapshot(&self, tick: &Self::Tick, seat: Option<u32>, viewer: &mut Self::Viewer, out: &mut Vec<u8>) -> bool;
    /// Whether an event at a point reaches this client.
    fn hears(&self, _tick: &Self::Tick, _seat: Option<u32>, _x: i32, _y: i32) -> bool {
        true
    }
}

/// What the server knows of one seat's player, each tick (`GameServer::presence`).
///
/// A seat is **here** (`playing`) while a connection plays it whose client
/// has the game on screen: it has applied the tick it was welcomed at, and
/// its commands keep coming. A browser still loading, a page in a hidden
/// tab (it sends no commands), a link gone quiet and a seat held for a
/// resume are all not here. The host's own in-process seats are here while
/// connected: they load nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Presence {
    /// A player holds the seat: connected, or held for a resume.
    pub seated: bool,
    /// A connection plays the seat now.
    pub connected: bool,
    /// That connection's client shows the game and plays it.
    pub playing: bool,
    /// The newest tick the seat's client says it has applied (0: none yet).
    pub applied: u32,
}

/// A connected seat whose newest command is this many ticks old is no
/// longer here: a second at the Delve's 30 Hz. Commands come every tick;
/// a link's bunching holds them back a few ticks at most, while a hidden
/// tab, a frozen page or a link gone dead sends none.
pub const AWAY_AFTER_TICKS: u32 = 30;

/// A game, as the server runs it. Everything here runs on the sim thread
/// except `decode_cmd`, which each connection's reader calls.
pub trait GameServer: Send + 'static {
    /// One seat's command for one tick (the muster: 4 bytes of stick and
    /// buttons; the Delve: its `Input`).
    type Cmd: Command + Send + 'static;
    /// Entity mode (`EntityRep`) or document mode (`DocRep`), or the game's own.
    type Rep: Replicate;

    /// The name clients ask for in their `Hello`.
    const GAME: &'static str;

    fn rates(&self) -> Rates;
    /// Seats (at most `limits::MAX_PLAYERS`).
    fn seats(&self) -> u32;
    /// What every client's `Welcome` carries for the game's client.
    fn params(&self) -> Vec<u8>;
    /// The replication the writers share.
    fn replicator(&self) -> Self::Rep;
    /// A command's bytes; `None` is malformed and closes the connection.
    fn decode_cmd(bytes: &[u8]) -> Option<Self::Cmd>;

    /// A player took `seat` (a newcomer: a resume does not call this).
    fn join(&mut self, seat: u32, name: &str, events: &mut Events);
    /// The seat is given up for good: a goodbye, a kick, or its resume
    /// window ran out.
    fn leave(&mut self, seat: u32, events: &mut Events);
    /// A one-off action, delivered in arrival order before the tick's step.
    fn action(&mut self, _seat: Option<u32>, _kind: u32, _body: &[u8], _events: &mut Events) {}
    /// Who is here, told just before each `step`: `seats[seat]` is what the
    /// server knows of that seat's player this tick. A game that keeps an
    /// absent player's character out of harm's way reads it here.
    fn presence(&mut self, _tick: u32, _seats: &[Presence]) {}
    /// One tick. `cmds[seat]` is the seat's command, `None` for an empty seat.
    fn step(&mut self, tick: u32, cmds: &[Option<Self::Cmd>], events: &mut Events);
    /// What the writers read for this tick, published once.
    fn publish(&mut self, tick: u32) -> <Self::Rep as Replicate>::Tick;

    /// The game's server knobs (`angel_net::knob`), where they stand. A
    /// knob's default is the game's number today, so nothing changes until
    /// one is moved. A number a client's prediction uses must also reach the
    /// clients in the game's own state (the Delve's `Run::tune`), so they
    /// step as the server does.
    fn knobs(&self) -> Vec<Knob> {
        Vec::new()
    }
    /// Set one, between ticks: `value` is already inside its bounds and on
    /// its step. Whether the name is the game's.
    fn set_knob(&mut self, _name: &str, _value: f64) -> bool {
        false
    }
}
