//! Starting a game's server, the state its threads share, and the host's
//! handle on it.

use crate::conn::{self, Conn, Out};
use crate::http::{self, Assets, ClientBundle};
use crate::limits::{EVENT_RING, MAX_PENDING};
use crate::link::{lan_ip, link, random_key};
use crate::local::{self, Dial, LocalConn, Pipe};
use crate::session::{MAX_PLAYERS, Player, Sessions};
use crate::sim::{self, Inbound, Manual, Stepper};
use crate::stats::{Telemetry, Totals};
use crate::ws::code;
use crate::{Event, GameServer, Rates, Replicate};
use angel_net::knob::{self, Knob};
use angel_net::proto::{Reject, ToClient};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering::SeqCst};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, Weak};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// How a server is started.
#[derive(Clone, Debug)]
pub struct ServeConfig {
    pub bind: SocketAddr,
    /// The browser client; a placeholder page without one.
    pub bundle: Option<ClientBundle>,
    /// A fixed join key (scripts and tests); a random one without.
    pub key: Option<String>,
    /// Print a line of stats to stderr this often.
    pub log_every: Option<Duration>,
    /// Dev mode: any seat may set the server's knobs, not only the host
    /// (`angel_net::knob`), and every page offers its panel.
    pub dev: bool,
    /// Where the knobs moved from their defaults are kept: read at the
    /// start, written whenever one moves.
    pub tune_file: Option<PathBuf>,
}

impl ServeConfig {
    pub fn new(bind: SocketAddr) -> ServeConfig {
        ServeConfig { bind, bundle: None, key: None, log_every: None, dev: false, tune_file: None }
    }
}

/// One published tick, as the writers read it.
pub(crate) struct TickOut<T> {
    pub tick: u32,
    /// A snapshot goes out on this tick (`tick_hz / snapshot_hz` apart).
    pub snapshot: bool,
    /// Each seat's last command seq consumed, by seat.
    pub acks: Vec<u32>,
    /// The connection playing each seat on this tick, by seat. A writer
    /// uses its seat's ack, view and private events only on ticks its own
    /// connection plays it: a seat that just changed hands shows nothing
    /// of its last player.
    pub playing: Vec<Option<u64>>,
    pub body: T,
    pub events: Vec<Event>,
}

impl<T> TickOut<T> {
    /// The seat this connection plays on this tick, if the sim has it so.
    pub fn seat_of(&self, conn: &Conn) -> Option<u32> {
        conn.seat.filter(|&s| self.playing.get(s as usize).copied().flatten() == Some(conn.id))
    }
}

pub(crate) type Published<G> = Arc<TickOut<<<G as GameServer>::Rep as Replicate>::Tick>>;

/// What every thread of one server shares.
pub(crate) struct Hub<G: GameServer> {
    pub rates: Rates,
    pub seats: u32,
    pub params: Vec<u8>,
    pub rep: G::Rep,
    pub assets: Assets,
    pub sessions: Mutex<Sessions>,
    pub conns: Mutex<HashMap<u64, Arc<Conn>>>,
    /// The newest published ticks, oldest first, `EVENT_RING` at most.
    pub ticks: Mutex<VecDeque<Published<G>>>,
    pub newest: AtomicU32,
    pub inbound: Sender<Inbound<G::Cmd>>,
    pub tele: Telemetry,
    pub stop: AtomicBool,
    /// Connections not yet past their hello.
    pub pending: AtomicUsize,
    pub next_conn: AtomicU64,
    pub started: Instant,
    pub addr: SocketAddr,
    /// The in-process connections' pipes, for a sim stepped by hand to wait on.
    pub locals: Mutex<Vec<Weak<Pipe>>>,
    /// Anyone may set the server's knobs, not only the host.
    pub dev: bool,
    /// The server's knobs as they stand (the engine's, then the game's),
    /// kept by the sim whenever one moves.
    pub knobs: Mutex<Vec<Knob>>,
    /// Ticks between snapshots (the `snapshot_every` knob), for the writers.
    pub every: AtomicU32,
    /// Connections that asked for the server's numbers once a second.
    pub watching: Mutex<HashSet<u64>>,
    pub tune_file: Option<PathBuf>,
}

impl<G: GameServer> Hub<G> {
    /// The published ticks newer than `after`, oldest first.
    pub fn ticks_since(&self, after: u32, out: &mut Vec<Published<G>>) {
        out.clear();
        let ticks = self.ticks.lock().expect("tick ring lock");
        let from = ticks.partition_point(|t| t.tick <= after);
        out.extend(ticks.range(from..).cloned());
    }

    pub fn publish(&self, out: TickOut<<G::Rep as Replicate>::Tick>) {
        let tick = out.tick;
        {
            let mut ticks = self.ticks.lock().expect("tick ring lock");
            ticks.push_back(Arc::new(out));
            while ticks.len() > EVENT_RING {
                ticks.pop_front();
            }
        }
        self.newest.store(tick, SeqCst);
        for conn in self.conns.lock().expect("connections lock").values() {
            conn.wake();
        }
    }

    /// Wait (at most `wait` in all) until every in-process connection's
    /// reader has acted on everything its client sent: a sim stepped by
    /// hand then takes a command sent before the step in that step.
    pub fn settle_local(&self, wait: Duration) {
        let until = Instant::now() + wait;
        let pipes: Vec<Arc<Pipe>> = {
            let mut locals = self.locals.lock().expect("locals lock");
            locals.retain(|p| p.strong_count() > 0);
            locals.iter().filter_map(Weak::upgrade).collect()
        };
        for pipe in pipes {
            pipe.settle(until);
        }
    }

    pub fn server_ms(&self) -> f64 {
        self.started.elapsed().as_secs_f64() * 1000.0
    }

    /// Whether a connection may set the server's knobs: the host's own,
    /// or anyone's in dev mode.
    pub fn may_tune(&self, local: bool) -> bool {
        local || self.dev
    }

    /// The knob table as a connection is told it.
    pub fn knobs_message(&self, local: bool) -> Vec<u8> {
        let json = knob::table_json(self.dev, self.may_tune(local), &self.knobs.lock().expect("knobs lock"));
        ToClient::Knobs { json: &json }.to_vec()
    }

    /// A line to one connection (a knob it may not set, or has no such).
    pub fn tell_one(&self, conn: u64, text: &str) {
        if let Some(c) = self.conns.lock().expect("connections lock").get(&conn) {
            c.send(Out::Msg(ToClient::Notice { text }.to_vec()));
        }
    }

    /// The knob table to one connection.
    pub fn tell_knobs_to(&self, conn: u64) {
        let local = self.conns.lock().expect("connections lock").get(&conn).map(|c| c.local);
        if let Some(local) = local {
            let msg = self.knobs_message(local);
            if let Some(c) = self.conns.lock().expect("connections lock").get(&conn) {
                c.send(Out::Msg(msg));
            }
        }
    }

    /// Tell every connection the knob table as it stands.
    pub fn tell_knobs(&self) {
        let (host, guest) = (self.knobs_message(true), self.knobs_message(false));
        for c in self.conns.lock().expect("connections lock").values() {
            c.send(Out::Msg(if self.may_tune(c.local) { host.clone() } else { guest.clone() }));
        }
    }

    fn counts(&self) -> (u32, u32) {
        let s = self.sessions.lock().expect("sessions lock");
        (s.players().iter().filter(|p| p.connected).count() as u32, s.watchers())
    }
}

/// The host's handle, the same whatever game runs. It opens in-process
/// connections too (`Dial`), so a `LocalConn` can dial again on its own.
trait Control: Dial + Send + Sync {
    fn addr(&self) -> SocketAddr;
    fn key(&self) -> String;
    fn new_key(&self) -> String;
    fn lock(&self, locked: bool);
    fn kick(&self, seat: u32) -> bool;
    fn players(&self) -> Vec<Player>;
    fn notice(&self, text: &str);
    fn totals(&self) -> Totals;
    /// True when the accept thread was woken and will end.
    fn stop(&self) -> bool;
    fn live(&self) -> usize;
    fn knobs(&self) -> Vec<Knob>;
    fn tune(&self, name: &str, value: f64) -> Result<f64, String>;
}

impl<G: GameServer> Control for Hub<G> {
    fn addr(&self) -> SocketAddr {
        self.addr
    }

    fn key(&self) -> String {
        self.sessions.lock().expect("sessions lock").key().to_string()
    }

    fn new_key(&self) -> String {
        let key = random_key();
        self.sessions.lock().expect("sessions lock").set_key(&key);
        key
    }

    fn lock(&self, locked: bool) {
        self.sessions.lock().expect("sessions lock").lock(locked);
    }

    fn kick(&self, seat: u32) -> bool {
        let conn = {
            let mut sessions = self.sessions.lock().expect("sessions lock");
            let Some(conn) = sessions.kick(seat) else {
                return false;
            };
            let _ = self.inbound.send(Inbound::Left { seat });
            conn
        };
        if let Some(id) = conn
            && let Some(c) = self.conns.lock().expect("connections lock").get(&id)
        {
            let reject = ToClient::Reject { why: Reject::Kicked, text: "the host removed you from this session" };
            c.close(code::POLICY, Some(reject.to_vec()));
        }
        true
    }

    fn players(&self) -> Vec<Player> {
        self.sessions.lock().expect("sessions lock").players()
    }

    fn notice(&self, text: &str) {
        let msg = ToClient::Notice { text }.to_vec();
        for c in self.conns.lock().expect("connections lock").values() {
            c.send(Out::Msg(msg.clone()));
        }
    }

    fn totals(&self) -> Totals {
        let (players, spectators) = self.counts();
        self.tele.read(players, spectators)
    }

    fn stop(&self) -> bool {
        if self.stop.swap(true, SeqCst) {
            return true;
        }
        let reject = ToClient::Reject { why: Reject::Stopping, text: "the host closed the session" }.to_vec();
        for c in self.conns.lock().expect("connections lock").values() {
            c.close(code::GOING_AWAY, Some(reject.clone()));
        }
        // Wake the accept thread, blocked in accept, with a connection of our own.
        let ip = match self.addr.ip() {
            IpAddr::V4(ip) if ip.is_unspecified() => IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V6(ip) if ip.is_unspecified() => IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
            ip => ip,
        };
        TcpStream::connect_timeout(&SocketAddr::new(ip, self.addr.port()), Duration::from_secs(1)).is_ok()
    }

    fn live(&self) -> usize {
        self.conns.lock().expect("connections lock").len()
    }

    fn knobs(&self) -> Vec<Knob> {
        self.knobs.lock().expect("knobs lock").clone()
    }

    fn tune(&self, name: &str, value: f64) -> Result<f64, String> {
        let knobs = self.knobs.lock().expect("knobs lock");
        let Some(k) = knobs.iter().find(|k| k.name == name) else {
            let names: Vec<_> = knobs.iter().map(|k| k.name).collect();
            return Err(format!("no knob {name}: {}", names.join(", ")));
        };
        let Some(value) = k.fit(value) else {
            return Err(format!("{value} is not a number {name} takes"));
        };
        drop(knobs);
        let _ = self.inbound.send(Inbound::Tune { name: name.to_string(), value });
        Ok(value)
    }
}

impl<G: GameServer> Dial for Hub<G> {
    fn dial(self: Arc<Self>) -> io::Result<LocalConn> {
        let pipe = Arc::new(Pipe::default());
        self.locals.lock().expect("locals lock").push(Arc::downgrade(&pipe));
        let client = LocalConn::new(pipe.clone(), self.clone());
        thread::Builder::new()
            .name("angel-local".into())
            .stack_size(conn::READER_STACK)
            .spawn(move || local::serve(self, pipe))?;
        Ok(client)
    }
}

/// A running server.
pub struct Server {
    hub: Arc<dyn Control>,
    threads: Vec<JoinHandle<()>>,
    accept: Option<JoinHandle<()>>,
}

/// How long `stop` waits for connections to finish their close.
/// Each connection's reader waits `CLOSE_WAIT` for the client's close, then
/// as long again for a writer stuck on its socket.
const STOP_WAIT: Duration = Duration::from_secs(5);
/// How often a stopping server looks for its connections to be gone.
const STOP_POLL: Duration = Duration::from_millis(20);

impl Server {
    /// Bind, and start the accept and sim threads.
    pub fn start<G: GameServer>(game: G, config: ServeConfig) -> io::Result<Server> {
        let (mut server, stepper) = Server::open(game, &config)?;
        server.threads.push(thread::Builder::new().name("angel-sim".into()).spawn(move || sim::run(stepper))?);
        Ok(server)
    }

    /// Bind and start the accept thread, with the sim stepped by hand on
    /// the caller's thread (`Manual::run_to`) instead of a thread of its
    /// own: a host that keeps its own clock, and tests that step a game
    /// exactly.
    pub fn start_manual<G: GameServer>(game: G, config: ServeConfig) -> io::Result<(Server, Manual<G>)> {
        let (server, stepper) = Server::open(game, &config)?;
        Ok((server, Manual::new(stepper)))
    }

    fn open<G: GameServer>(game: G, config: &ServeConfig) -> io::Result<(Server, Stepper<G>)> {
        let rates = game.rates();
        if rates.tick_hz == 0 || rates.snapshot_hz == 0 || !rates.tick_hz.is_multiple_of(rates.snapshot_hz) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "tick_hz must be a whole multiple of snapshot_hz"));
        }
        let bundle = config.bundle.clone().unwrap_or_else(ClientBundle::placeholder);
        let assets = http::Assets::prepare(&bundle)?;
        let listener = TcpListener::bind(config.bind)?;
        let addr = listener.local_addr()?;
        let seats = game.seats().min(MAX_PLAYERS);
        let key = config.key.clone().unwrap_or_else(random_key);
        let every = (rates.tick_hz / rates.snapshot_hz).max(1);
        let (tx, rx): (Sender<Inbound<G::Cmd>>, Receiver<Inbound<G::Cmd>>) = channel();
        let hub = Arc::new(Hub::<G> {
            rates,
            seats,
            params: game.params(),
            rep: game.replicator(),
            assets,
            sessions: Mutex::new(Sessions::new(&key, seats)),
            conns: Mutex::new(HashMap::new()),
            ticks: Mutex::new(VecDeque::with_capacity(EVENT_RING + 1)),
            newest: AtomicU32::new(0),
            inbound: tx,
            tele: Telemetry::default(),
            stop: AtomicBool::new(false),
            pending: AtomicUsize::new(0),
            next_conn: AtomicU64::new(1),
            started: Instant::now(),
            addr,
            locals: Mutex::new(Vec::new()),
            dev: config.dev,
            knobs: Mutex::new(Vec::new()),
            every: AtomicU32::new(every),
            watching: Mutex::new(HashSet::new()),
            tune_file: config.tune_file.clone(),
        });
        let mut stepper = Stepper::new(game, hub.clone(), rx);
        // The tune file's numbers, before anyone sees the first tick.
        stepper.restore_knobs();
        stepper.publish_start();
        let mut threads = Vec::new();
        let accept_hub = hub.clone();
        let accept = Some(thread::Builder::new().name("angel-accept".into()).spawn(move || accept(accept_hub, listener))?);
        if let Some(every) = config.log_every {
            let log_hub = hub.clone();
            threads.push(thread::Builder::new().name("angel-log".into()).spawn(move || log(log_hub, every))?);
        }
        Ok((Server { hub, threads, accept }, stepper))
    }

    pub fn addr(&self) -> SocketAddr {
        self.hub.addr()
    }

    /// The join key.
    pub fn key(&self) -> String {
        self.hub.key()
    }

    /// A new join key: the old link stops letting anyone new in.
    pub fn new_key(&self) -> String {
        self.hub.new_key()
    }

    /// Closed to newcomers (seated players still resume).
    pub fn lock(&self, locked: bool) {
        self.hub.lock(locked)
    }

    /// Remove a player; their token stops working. To keep them out, make a
    /// new key too (they still hold the link).
    pub fn kick(&self, seat: u32) -> bool {
        self.hub.kick(seat)
    }

    pub fn players(&self) -> Vec<Player> {
        self.hub.players()
    }

    /// A line from the host, shown on every client.
    pub fn notice(&self, text: &str) {
        self.hub.notice(text)
    }

    /// A connection in process, for the host's own seat or a couch
    /// player's: a client like any other (the same admission, seat, resume,
    /// writer and replication), over a pipe instead of a socket. It is let
    /// in without the key and while the session is locked. Its hello is due
    /// within `limits::HANDSHAKE_TIME`, as a socket's is.
    pub fn connect_local(&self) -> io::Result<LocalConn> {
        let door: Arc<dyn Dial> = self.hub.clone();
        door.dial()
    }

    /// Everything counted so far; two readings give a window (`Totals::since`).
    pub fn totals(&self) -> Totals {
        self.hub.totals()
    }

    /// The server's knobs as they stand: the engine's, then the game's.
    pub fn knobs(&self) -> Vec<Knob> {
        self.hub.knobs()
    }

    /// Set a server knob as the host: the value it will take (inside its
    /// bounds, on its step), set between the next ticks and told to every
    /// client; or why not.
    pub fn tune(&self, name: &str, value: f64) -> Result<f64, String> {
        self.hub.tune(name, value)
    }

    /// The doors this server listens behind: the LAN address and this host.
    pub fn doors(&self) -> Vec<String> {
        let addr = self.addr();
        let port = addr.port();
        let mut doors = Vec::new();
        if addr.ip().is_unspecified() {
            if let Some(ip) = lan_ip() {
                doors.push(door(ip, port));
            }
            doors.push(door(IpAddr::V4(Ipv4Addr::LOCALHOST), port));
        } else {
            doors.push(door(addr.ip(), port));
        }
        doors
    }

    /// The link for each door.
    pub fn links(&self) -> Vec<String> {
        let key = self.key();
        self.doors().iter().map(|d| link(d, &key)).collect()
    }

    /// Every client is told the session is over, then everything stops.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        let woke = self.hub.stop();
        let until = Instant::now() + STOP_WAIT;
        while self.hub.live() > 0 && Instant::now() < until {
            thread::sleep(STOP_POLL);
        }
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
        // An accept thread no connection could reach (the address it was
        // bound to has left the machine) is not waited for: it ends with
        // the next connection it accepts, or with the process.
        if let Some(t) = self.accept.take()
            && woke
        {
            let _ = t.join();
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if !self.threads.is_empty() || self.accept.is_some() {
            self.shutdown();
        }
    }
}

fn door(ip: IpAddr, port: u16) -> String {
    match ip {
        IpAddr::V4(ip) => format!("http://{ip}:{port}"),
        IpAddr::V6(ip) => format!("http://[{ip}]:{port}"),
    }
}

/// A full door is answered, not ignored, so a browser shows why.
const BUSY: &[u8] = b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nRetry-After: 1\r\nConnection: close\r\n\r\n";
/// After a failed accept (out of file descriptors), a short rest.
const ACCEPT_REST: Duration = Duration::from_millis(10);

fn accept<G: GameServer>(hub: Arc<Hub<G>>, listener: TcpListener) {
    for stream in listener.incoming() {
        if hub.stop.load(SeqCst) {
            break;
        }
        let mut stream = match stream {
            Ok(s) => s,
            Err(_) => {
                thread::sleep(ACCEPT_REST);
                continue;
            }
        };
        if hub.pending.load(SeqCst) >= MAX_PENDING {
            hub.tele.counters.busy.fetch_add(1, SeqCst);
            let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
            let _ = stream.write_all(BUSY);
            continue;
        }
        hub.pending.fetch_add(1, SeqCst);
        let conn_hub = hub.clone();
        let spawned = thread::Builder::new()
            .name("angel-conn".into())
            .stack_size(conn::READER_STACK)
            .spawn(move || conn::handle(conn_hub, stream));
        if spawned.is_err() {
            hub.pending.fetch_sub(1, SeqCst);
        }
    }
}

/// How often the log thread looks for the stop flag between lines.
const LOG_POLL: Duration = Duration::from_millis(100);

fn log<G: GameServer>(hub: Arc<Hub<G>>, every: Duration) {
    let mut last = hub.totals();
    let mut due = Instant::now() + every;
    while !hub.stop.load(SeqCst) {
        thread::sleep(LOG_POLL);
        if Instant::now() < due {
            continue;
        }
        due += every;
        let now = hub.totals();
        eprintln!("angel-serve {}", now.since(&last));
        last = now;
    }
}
