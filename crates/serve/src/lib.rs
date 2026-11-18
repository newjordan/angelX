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

pub mod limits {
    use std::time::Duration;

    pub use crate::session::{MAX_PLAYERS, MAX_SPECTATORS, RESUME_WINDOW};
    pub use angel_net::proto::MAX_CLIENT_MESSAGE;

    pub const MAX_HANDSHAKE: usize = 4096;
    pub const HANDSHAKE_TIME: Duration = Duration::from_secs(20);
    pub const MAX_MESSAGES_PER_SEC: u32 = 1000;
    pub const PING_EVERY: Duration = Duration::from_secs(1);
    pub const SILENT_CLOSE: Duration = Duration::from_secs(10);
    pub const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
    pub const MAX_PENDING: usize = 512;
    pub const KEEP_ALIVE_IDLE: Duration = Duration::from_secs(2);
    pub const QUEUE: Duration = Duration::from_millis(250);
    pub const MIN_IN_FLIGHT: usize = 1200;
    pub const MAX_IN_FLIGHT: usize = 64 * 1024;
    pub const DELAY_WINDOW: Duration = Duration::from_secs(5);
    pub const EVENT_RING: usize = 64;
    pub const CLOSE_WAIT: Duration = Duration::from_secs(2);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rates {
    pub tick_hz: u32,
    pub snapshot_hz: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum To {
    All,
    Seat(u32),
    Near(i32, i32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub to: To,
    pub bytes: Vec<u8>,
    pub assets: Vec<std::sync::Arc<http::LiveAsset>>,
}

#[derive(Clone, Debug, Default)]
pub struct Events {
    list: Vec<Event>,
}

impl Events {
    pub fn all(&mut self, bytes: impl Into<Vec<u8>>) {
        self.list.push(Event { to: To::All, bytes: bytes.into(), assets:Vec::new() });
    }

    pub fn seat(&mut self, seat: u32, bytes: impl Into<Vec<u8>>) {
        self.list.push(Event { to: To::Seat(seat), bytes: bytes.into(), assets:Vec::new() });
    }

    pub fn near(&mut self, x: i32, y: i32, bytes: impl Into<Vec<u8>>) {
        self.list.push(Event { to: To::Near(x, y), bytes: bytes.into(), assets:Vec::new() });
    }

    pub fn lease_last(&mut self, asset: std::sync::Arc<http::LiveAsset>) {
        if let Some(event) = self.list.last_mut() { event.assets.push(asset); }
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

pub trait Replicate: Send + Sync + 'static {
    type Tick: Send + Sync + 'static;
    type Viewer: Send + 'static;

    fn viewer(&self, seat: Option<u32>) -> Self::Viewer;
    fn snapshot(&self, tick: &Self::Tick, seat: Option<u32>, viewer: &mut Self::Viewer, out: &mut Vec<u8>) -> bool;
    fn hears(&self, _tick: &Self::Tick, _seat: Option<u32>, _x: i32, _y: i32) -> bool {
        true
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Presence {
    pub seated: bool,
    pub connected: bool,
    pub playing: bool,
    pub applied: u32,
}

pub const AWAY_AFTER_TICKS: u32 = 30;

pub trait GameServer: Send + 'static {
    type Cmd: Command + Send + 'static;
    type Rep: Replicate;

    const GAME: &'static str;

    fn rates(&self) -> Rates;
    fn seats(&self) -> u32;
    fn params(&self) -> Vec<u8>;
    fn replicator(&self) -> Self::Rep;
    fn decode_cmd(bytes: &[u8]) -> Option<Self::Cmd>;

    fn join(&mut self, seat: u32, name: &str, events: &mut Events);
    fn leave(&mut self, seat: u32, events: &mut Events);
    fn action(&mut self, _seat: Option<u32>, _kind: u32, _body: &[u8], _events: &mut Events) {}
    fn presence(&mut self, _tick: u32, _seats: &[Presence]) {}
    fn input_age(&mut self, _seat: u32, _ticks: u64) {}
    fn step(&mut self, tick: u32, cmds: &[Option<Self::Cmd>], events: &mut Events);
    fn publish(&mut self, tick: u32) -> <Self::Rep as Replicate>::Tick;

    fn knobs(&self) -> Vec<Knob> {
        Vec::new()
    }
    fn set_knob(&mut self, _name: &str, _value: f64) -> bool {
        false
    }
}
