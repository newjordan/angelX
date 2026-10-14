//! In-process connections: the host's own seat, and one for each couch
//! player. A pipe stands where a socket would, and everything past it is
//! the socket's own code (`conn`): the hello and admission, the seat and
//! its token, resume, the reader and the writer, replication, events and
//! the per-seat command FIFO. So what the host sees is what a friend sees.
//!
//! A pipe is a pair of queues of whole messages, one each way. It carries
//! exactly the bytes a socket client would send and receive in its
//! WebSocket messages, with no frames around them. It differs from a socket
//! only in what a pipe cannot do or need (`conn::Inlet::LOCAL`): the host
//! is let in without the key and past the lock, and is never limited in
//! messages a second, closed for silence, or held back for being behind.
//! What the client has not taken is held up to `LOCAL_UNREAD` bytes; past
//! that the writer blocks and skips to the newest tick, as on a full
//! socket, and is cut off after `WRITE_TIMEOUT`, as a socket is.

use crate::conn::{self, Cut, Heard, Inlet, Outlet, Put};
use crate::limits::{MAX_IN_FLIGHT, WRITE_TIMEOUT};
use crate::server::Hub;
use crate::ws::Message;
use crate::GameServer;
use angel_net::proto::ToServer;
use std::collections::VecDeque;
use std::io::{self, ErrorKind};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Bytes the server's writer lets an in-process client leave untaken
/// before it blocks: a window's ceiling, seconds of the busiest stream. A
/// terminal takes what arrived once a frame, so only a client that stopped
/// taking (a stalled frame loop) ever reaches it.
pub const LOCAL_UNREAD: usize = MAX_IN_FLIGHT;

/// The server closed an in-process connection: its close code, if it gave
/// one (`ws::code`; `REPLACED` when another connection took the seat).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Closed {
    pub code: Option<u16>,
}

#[derive(Default)]
struct State {
    /// Client messages the server's reader has not read yet.
    up: VecDeque<Vec<u8>>,
    /// Server messages the client has not taken yet, with when each was written.
    down: VecDeque<(Instant, Vec<u8>)>,
    down_bytes: usize,
    /// The writer's newest ping (once a second): the server is alive even
    /// when it has nothing to send.
    beat: Option<Instant>,
    /// The client let go of its end.
    client_gone: bool,
    /// The server closed its end: with its close code, or without one when cut.
    server_done: Option<Option<u16>>,
    /// The server's reader has a message in hand: it took it from `up` and
    /// has not come back for the next, so it may not have acted on it yet.
    in_hand: bool,
}

/// The two queues, shared by the client's `LocalConn` and the server's
/// reader and writer.
#[derive(Default)]
pub(crate) struct Pipe {
    state: Mutex<State>,
    /// Wakes the server's reader (a message, or the client gone) and its
    /// writer (room in the pipe).
    server: Condvar,
    /// Wakes a client waiting in `recv_timeout`.
    client: Condvar,
    /// Wakes a sim stepped by hand waiting for the reader to act on
    /// everything the client sent (`settle`).
    settled: Condvar,
}

impl Pipe {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Wait (at most until `until`) for the server's reader to have acted
    /// on everything the client sent: nothing left in `up`, nothing in
    /// hand. A pipe either side let go of is settled.
    pub(crate) fn settle(&self, until: Instant) {
        let mut s = self.lock();
        while !(s.up.is_empty() && !s.in_hand) && !s.client_gone && s.server_done.is_none() {
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return;
            }
            s = self.settled.wait_timeout(s, left).unwrap_or_else(|p| p.into_inner()).0;
        }
    }
}

impl Cut for Pipe {
    fn cut(&self) {
        let mut s = self.lock();
        if s.server_done.is_none() {
            s.server_done = Some(None);
        }
        drop(s);
        self.server.notify_all();
        self.client.notify_all();
        self.settled.notify_all();
    }
}

/// The server's reading end.
struct PipeIn(Arc<Pipe>);

impl Inlet for PipeIn {
    const LOCAL: bool = true;

    fn next(&mut self, wait: Duration) -> Heard {
        let until = Instant::now() + wait;
        let mut s = self.0.lock();
        // Back for the next: the last message taken is acted on.
        if s.in_hand {
            s.in_hand = false;
            if s.up.is_empty() {
                self.0.settled.notify_all();
            }
        }
        loop {
            // What the client sent before it let go is still read, as a
            // socket's bytes are before its end.
            if let Some(m) = s.up.pop_front() {
                s.in_hand = true;
                return Heard::Message(Message::Binary(m));
            }
            if s.client_gone || s.server_done.is_some() {
                return Heard::Gone;
            }
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Heard::Quiet;
            }
            s = self.0.server.wait_timeout(s, left).unwrap_or_else(|p| p.into_inner()).0;
        }
    }

    /// Nothing to wait for: what the server sent is in the pipe already,
    /// for the client to take whenever it looks.
    fn linger(&mut self) {
        self.0.cut();
    }
}

/// The server's writing end.
struct PipeOut(Arc<Pipe>);

impl Outlet for PipeOut {
    fn put(&mut self, what: Put) -> io::Result<usize> {
        let mut s = self.0.lock();
        let gone = |s: &State| s.client_gone || s.server_done.is_some();
        match what {
            Put::Binary(bytes) => {
                let until = Instant::now() + WRITE_TIMEOUT;
                while s.down_bytes >= LOCAL_UNREAD && !gone(&s) {
                    let left = until.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Err(io::Error::new(ErrorKind::TimedOut, "the in-process client took nothing"));
                    }
                    s = self.0.server.wait_timeout(s, left).unwrap_or_else(|p| p.into_inner()).0;
                }
                if gone(&s) {
                    return Err(ErrorKind::BrokenPipe.into());
                }
                s.down.push_back((Instant::now(), bytes.to_vec()));
                s.down_bytes += bytes.len();
                drop(s);
                self.0.client.notify_all();
                Ok(bytes.len())
            }
            Put::Ping => {
                if gone(&s) {
                    return Err(ErrorKind::BrokenPipe.into());
                }
                s.beat = Some(Instant::now());
                Ok(0)
            }
            // A pipe carries no WebSocket pings to answer.
            Put::Pong(_) => Ok(0),
            Put::Close(code) => {
                if s.server_done.is_none() {
                    s.server_done = Some(Some(code));
                }
                drop(s);
                self.0.server.notify_all();
                self.0.client.notify_all();
                Ok(0)
            }
        }
    }

    fn finish(&mut self) {}
}

/// Serve one in-process connection, on the thread `connect_local` started.
pub(crate) fn serve<G: GameServer>(hub: Arc<Hub<G>>, pipe: Arc<Pipe>) {
    conn::websocket(&hub, PipeIn(pipe.clone()), PipeOut(pipe.clone()), pipe, None);
}

/// Opens in-process connections to one server: the server's handle, kept
/// so a `LocalConn` can dial again after its connection is lost.
pub(crate) trait Dial: Send + Sync {
    fn dial(self: Arc<Self>) -> io::Result<LocalConn>;
}

/// The client's end of an in-process connection: send what a socket client
/// would send in one WebSocket message, and take what the server sent, a
/// message at a time. Nothing here blocks except `recv_timeout`, so a
/// terminal can poll it once a frame. Closing it (or dropping it) is a
/// socket closing: the seat is held for a resume. `bye` frees it.
pub struct LocalConn {
    pipe: Arc<Pipe>,
    door: Arc<dyn Dial>,
}

impl LocalConn {
    pub(crate) fn new(pipe: Arc<Pipe>, door: Arc<dyn Dial>) -> LocalConn {
        LocalConn { pipe, door }
    }

    /// One client message (a `ToServer`'s bytes).
    pub fn send(&self, msg: Vec<u8>) -> Result<(), Closed> {
        let mut s = self.pipe.lock();
        if let Some(code) = s.server_done {
            return Err(Closed { code });
        }
        if s.client_gone {
            return Err(Closed { code: None });
        }
        s.up.push_back(msg);
        drop(s);
        self.pipe.server.notify_all();
        Ok(())
    }

    /// The next server message, if one is waiting. Once the server has
    /// closed and everything it sent is taken, the close.
    pub fn try_recv(&self) -> Result<Option<Vec<u8>>, Closed> {
        self.recv_timeout(Duration::ZERO)
    }

    /// The next server message, waiting up to `wait` for it.
    pub fn recv_timeout(&self, wait: Duration) -> Result<Option<Vec<u8>>, Closed> {
        Ok(self.recv_stamped(wait)?.map(|(m, _)| m))
    }

    /// The next server message and when the server wrote it.
    pub(crate) fn recv_stamped(&self, wait: Duration) -> Result<Option<(Vec<u8>, Instant)>, Closed> {
        let until = Instant::now() + wait;
        let mut s = self.pipe.lock();
        loop {
            if let Some((at, m)) = s.down.pop_front() {
                s.down_bytes -= m.len();
                drop(s);
                // The writer may be waiting for room.
                self.pipe.server.notify_all();
                return Ok(Some((m, at)));
            }
            if let Some(code) = s.server_done {
                return Err(Closed { code });
            }
            if s.client_gone {
                return Err(Closed { code: None });
            }
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(None);
            }
            s = self.pipe.client.wait_timeout(s, left).unwrap_or_else(|p| p.into_inner()).0;
        }
    }

    /// When the server's writer last said it was alive: it does once a
    /// second, as a socket's writer pings, so a client that hears neither
    /// this nor a message for a while knows the server has stopped.
    pub fn beat(&self) -> Option<Instant> {
        self.pipe.lock().beat
    }

    /// Let go, as a socket closing: the seat is held for a resume
    /// (`limits::RESUME_WINDOW`) with this connection's seat token.
    pub fn close(&self) {
        let mut s = self.pipe.lock();
        if s.client_gone {
            return;
        }
        s.client_gone = true;
        drop(s);
        self.pipe.server.notify_all();
        self.pipe.client.notify_all();
        self.pipe.settled.notify_all();
    }

    /// Leave on purpose: a goodbye, then close. The seat is freed.
    pub fn bye(&self) {
        let _ = self.send(ToServer::Bye.to_vec());
        self.close();
    }

    /// The server this connection was opened on, to dial it again.
    pub(crate) fn door(&self) -> Arc<dyn Dial> {
        self.door.clone()
    }

    /// Another in-process connection to the same server, as a socket client
    /// would dial again (to resume its seat after this one was lost).
    pub fn redial(&self) -> io::Result<LocalConn> {
        self.door.clone().dial()
    }
}

impl Drop for LocalConn {
    fn drop(&mut self) {
        self.close();
    }
}

impl std::fmt::Debug for LocalConn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self.pipe.lock();
        f.debug_struct("LocalConn")
            .field("unread", &s.down.len())
            .field("client_gone", &s.client_gone)
            .field("server_done", &s.server_done)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Nowhere;

    impl Dial for Nowhere {
        fn dial(self: Arc<Self>) -> io::Result<LocalConn> {
            Err(ErrorKind::NotConnected.into())
        }
    }

    fn pair() -> (LocalConn, PipeIn, PipeOut) {
        let pipe = Arc::new(Pipe::default());
        (LocalConn::new(pipe.clone(), Arc::new(Nowhere)), PipeIn(pipe.clone()), PipeOut(pipe))
    }

    #[test]
    fn messages_go_each_way_in_order_and_what_was_sent_before_closing_is_read() {
        let (client, mut reader, mut writer) = pair();
        client.send(vec![1]).unwrap();
        client.send(vec![2]).unwrap();
        client.close();
        assert!(matches!(reader.next(Duration::ZERO), Heard::Message(Message::Binary(m)) if m == [1]));
        assert!(matches!(reader.next(Duration::ZERO), Heard::Message(Message::Binary(m)) if m == [2]));
        assert!(matches!(reader.next(Duration::from_secs(1)), Heard::Gone));
        // The server writing to a client that let go fails, as a socket does.
        assert!(writer.put(Put::Binary(&[9])).is_err());
        assert_eq!(client.send(vec![3]), Err(Closed { code: None }));
    }

    #[test]
    fn the_server_closing_is_seen_after_what_it_sent() {
        let (client, mut reader, mut writer) = pair();
        assert!(matches!(reader.next(Duration::from_millis(5)), Heard::Quiet));
        assert_eq!(writer.put(Put::Binary(&[7, 7])).unwrap(), 2);
        writer.put(Put::Ping).unwrap();
        assert!(client.beat().is_some());
        writer.put(Put::Close(4000)).unwrap();
        assert_eq!(client.try_recv(), Ok(Some(vec![7, 7])));
        assert_eq!(client.try_recv(), Err(Closed { code: Some(4000) }));
        assert!(matches!(reader.next(Duration::from_secs(1)), Heard::Gone));
        assert!(client.redial().is_err());
    }

    #[test]
    fn a_waiting_client_is_woken_by_a_message() {
        let (client, _reader, mut writer) = pair();
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            writer.put(Put::Binary(&[5])).unwrap();
            writer
        });
        let started = Instant::now();
        assert_eq!(client.recv_timeout(Duration::from_secs(5)), Ok(Some(vec![5])));
        assert!(started.elapsed() < Duration::from_secs(1));
        let _writer = t.join().unwrap();
    }

    #[test]
    fn a_full_pipe_holds_the_writer_until_the_client_takes() {
        let (client, _reader, mut writer) = pair();
        let big = vec![0u8; LOCAL_UNREAD];
        // The first is taken whatever its size, as a whole document must be.
        writer.put(Put::Binary(&big)).unwrap();
        let t = std::thread::spawn(move || {
            let started = Instant::now();
            writer.put(Put::Binary(&[1])).unwrap();
            started.elapsed()
        });
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(client.try_recv().unwrap().map(|m| m.len()), Some(LOCAL_UNREAD));
        let waited = t.join().unwrap();
        assert!(waited >= Duration::from_millis(40), "{waited:?}");
        assert_eq!(client.try_recv(), Ok(Some(vec![1])));
    }
}
