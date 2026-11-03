//! The session's end: a [`Host`] with no window, whose frames go to the
//! windows attached to it (docs/REMOTE_GLASS.md).
//!
//! The cockpit draws into it exactly as into a window: the same backend,
//! layers, pictures, stages, scenes and showcase. A publisher thread takes
//! what the window's render thread would take, keeps what a newly attached
//! window needs to catch up, and sends each of the session's frames to
//! every window as messages. Input comes back from the windows as the
//! terminal events the cockpit reads.

use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

use crossterm::event::Event;
use ratatui_core::buffer::Cell;

use super::wire::{self, Down, Feedback, CardChange, Grid, Hash, Kind, Presence, Status, Up, WireCard, WireCell};
use crate::scene::MeshData;
use crate::showcase::{Request, SHOWCASE_PICTURE};
use crate::{GridSize, Host, Layer, LockExt, Shared, Showing, Snapshot, UserEvent, Wake};

/// The same picture id handed over again within this long is a stream (a
/// video, an animation): its frames are not kept on the far side.
const STREAM_WITHIN: Duration = Duration::from_millis(1500);

/// How long a layers-only change waits for the frame that usually follows
/// it, so the two are shown together (the window's own rule: layers ride
/// the next frame).
const LAYERS_WAIT: Duration = Duration::from_millis(4);

/// A blob, encoded for the wire once, by the first writer that needs it.
pub(crate) struct Blob {
    hash: Hash,
    raw: Vec<u8>,
    framed: OnceLock<Arc<[u8]>>,
}

impl Blob {
    fn new(raw: Vec<u8>) -> Arc<Self> {
        Arc::new(Self { hash: wire::hash(&raw), raw, framed: OnceLock::new() })
    }

    fn framed(&self) -> io::Result<Arc<[u8]>> {
        if let Some(bytes) = self.framed.get() {
            return Ok(Arc::clone(bytes));
        }
        let (deflated, bytes) = match wire::deflate(&self.raw) {
            Some(packed) => (true, packed),
            None => (false, self.raw.clone()),
        };
        // The blob is already deflated or noise: the frame itself is not.
        let message = Down::Blob { hash: self.hash, deflated, bytes };
        let body = wire::encode(&message)?;
        let mut out = Vec::with_capacity(body.len() + 5);
        out.extend_from_slice(&(body.len() as u32 + 1).to_le_bytes());
        out.push(0);
        out.extend_from_slice(&body);
        let framed: Arc<[u8]> = out.into();
        Ok(Arc::clone(self.framed.get_or_init(|| framed)))
    }
}

/// One thing for a window's writer: a framed message, and the blobs it
/// names, which go first to a window that lacks them. `keep: false` blobs
/// are a stream's: sent every time, never counted as held.
#[derive(Clone)]
pub(crate) struct Item {
    blobs: Vec<(Arc<Blob>, bool)>,
    frame: Arc<[u8]>,
    kind: usize,
    /// A picture's id, size and cells: a writer may send it as a delta of
    /// what that window shows under the id.
    picture: Option<(u32, u32, u32, u16, u16)>,
}

/// What the link carried, by kind (`ANGEL_REMOTE_STATS=1` prints it on the
/// session's stderr every five seconds): the bytes written to windows.
mod stats {
    use std::sync::atomic::{AtomicU64, Ordering};
    pub(super) const KINDS: [&str; 9] = ["grid", "layers", "stage", "scene", "picture", "mesh", "blob", "other", "delta"];
    pub(super) static BYTES: [AtomicU64; 9] = [const { AtomicU64::new(0) }; 9];
    pub(super) static FRAMES: AtomicU64 = AtomicU64::new(0);

    pub(super) fn kind(message: &super::Down) -> usize {
        use super::Down;
        match message {
            Down::Grid(_) => 0,
            Down::Layers(_) => 1,
            Down::Stage { .. } => 2,
            Down::Scene { .. } => 3,
            Down::Picture { .. } => 4,
            Down::Mesh { .. } => 5,
            Down::Blob { .. } => 6,
            _ => 7,
        }
    }

    pub(super) fn add(kind: usize, bytes: usize) {
        BYTES[kind].fetch_add(bytes as u64, Ordering::Relaxed);
    }

    pub(super) fn start() {
        if !std::env::var("ANGEL_REMOTE_STATS").is_ok_and(|v| v == "1") {
            return;
        }
        std::thread::spawn(|| {
            let mut last = [0u64; 9];
            let mut last_frames = 0;
            loop {
                std::thread::sleep(std::time::Duration::from_secs(5));
                let now: Vec<u64> = BYTES.iter().map(|b| b.load(Ordering::Relaxed)).collect();
                let frames = FRAMES.load(Ordering::Relaxed);
                let parts: Vec<String> = KINDS
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| now[*i] > last[*i])
                    .map(|(i, k)| format!("{k} {:.1}", (now[i] - last[i]) as f64 / 5.0 / 1024.0))
                    .collect();
                let total: u64 = now.iter().zip(&last).map(|(n, l)| n - l).sum();
                eprintln!(
                    "REMOTE_STATS: {:.1} KiB/s, {:.1} frames/s ({})",
                    total as f64 / 5.0 / 1024.0,
                    (frames - last_frames) as f64 / 5.0,
                    parts.join(", ")
                );
                last.copy_from_slice(&now);
                last_frames = frames;
            }
        });
    }
}

impl Item {
    fn plain(message: &Down) -> Option<Self> {
        match wire::frame(message) {
            Ok(frame) => Some(Self {
                blobs: Vec::new(),
                frame,
                kind: stats::kind(message),
                picture: match message {
                    Down::Picture { id, width, height, cols, rows, .. } => Some((*id, *width, *height, *cols, *rows)),
                    _ => None,
                },
            }),
            Err(e) => {
                eprintln!("REMOTE: a message did not encode: {e}");
                None
            }
        }
    }

    fn with(message: &Down, blobs: Vec<(Arc<Blob>, bool)>) -> Option<Self> {
        Self::plain(message).map(|mut item| {
            item.blobs = blobs;
            item
        })
    }
}

/// What a new window needs to catch up: the latest of everything.
#[derive(Default)]
struct Mirror {
    cells: Vec<Cell>,
    cols: u16,
    rows: u16,
    cursor: Option<(u16, u16)>,
    card: Option<Arc<crate::card::Card>>,
    /// The render side's half of the snapshot double buffer.
    current: Snapshot,
    layers: Arc<Vec<Layer>>,
    pictures: HashMap<u32, Item>,
    uploaded_at: HashMap<u32, Instant>,
    streams: HashSet<u32>,
    meshes: HashMap<u32, Item>,
    stages: HashMap<u32, Item>,
    scenes: HashMap<u32, Item>,
    /// World meshes already made blobs, by the `Arc` they came in.
    world_meshes: HashMap<usize, (Weak<MeshData>, Arc<Blob>)>,
    showcase: Option<Item>,
    showcase_serial: u64,
    progress: (Option<f32>, bool),
    title: String,
    game_keyboard: bool,
}

struct Client {
    id: u64,
    kind: Kind,
    size: GridSize,
    queue: Sender<Item>,
}

struct Inner {
    mirror: Mirror,
    clients: Vec<Client>,
    /// The client whose window sizes the session: the last one used.
    sizer: Option<u64>,
    next_id: u64,
    presence: Presence,
}

/// The session: its headless host's other half. Cloned freely; every
/// clone is the same session.
#[derive(Clone)]
pub struct Hub {
    host: Host,
    inner: Arc<Mutex<Inner>>,
    keys: Sender<Event>,
    name: Arc<str>,
    started: u64,
    ended: Arc<AtomicBool>,
    serial: Arc<AtomicU64>,
}

/// Header-first serve headers for raw upload `bytes` (file bytes, not decoded
/// RGBA): `Content-Type` + `Content-Length` plus glass layout/budget/cap/hold
/// annotations, without decoding pixels. `path` is the artist's filename hint
/// (rescues truncated prefixes / leading-comment SVG / audio-boxed `m4a`);
/// `adapter_name` is `wgpu::AdapterInfo::name` (`None` headless → 4096 box).
/// `None` for unknown bytes (rejected before the decoder, where kitty would
/// base64-decode per frame to learn the size). Remote-serve answers these
/// headers once per blob; kitty pays base64 inflation every frame
/// ([`crate::graphics::serve_vs_kitty_bytes`]).
pub fn serve_headers_for_bytes(
    bytes: &[u8],
    path: Option<&std::path::Path>,
    adapter_name: Option<&str>,
) -> Option<Vec<(String, String)>> {
    let plan = crate::graphics::serve_plan_for_adapter(bytes, path, adapter_name)?;
    Some(plan.http_headers(bytes.len()))
}

/// A host with no window, sized `size` until a window says otherwise, and
/// the hub that sends its frames to the windows that attach. Its publisher
/// runs until the process ends.
pub fn headless(name: &str, size: GridSize) -> (Host, Hub) {
    let (wake, events) = mpsc::channel();
    let (keys, input) = mpsc::channel();
    let shared = Shared::default();
    *shared.grid.lock_recover() = size;
    let host = Host {
        shared: Arc::new(shared),
        proxy: Arc::new(Mutex::new(Wake::Channel(wake))),
        input: Arc::new(Mutex::new(Some(input))),
    };
    let hub = Hub {
        host: host.clone(),
        inner: Arc::new(Mutex::new(Inner {
            mirror: Mirror { title: "angelX".into(), ..Mirror::default() },
            clients: Vec::new(),
            sizer: None,
            next_id: 1,
            presence: Presence { session: name.to_string(), ..Presence::default() },
        })),
        keys,
        name: name.into(),
        started: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        ended: Arc::new(AtomicBool::new(false)),
        serial: Arc::new(AtomicU64::new(0)),
    };
    stats::start();
    let publisher = hub.clone();
    std::thread::Builder::new()
        .name("remote-publish".into())
        .spawn(move || publisher.publish(events))
        .expect("the publisher thread");
    (host, hub)
}

impl Hub {
    pub fn status(&self) -> Status {
        let inner = self.inner.lock_recover();
        Status {
            name: self.name.to_string(),
            pid: std::process::id(),
            clients: inner.clients.iter().filter(|c| matches!(c.kind, Kind::Window | Kind::Terminal)).count() as u32,
            size: self.host.grid_size(),
            started: self.started,
            title: inner.mirror.title.clone(),
            presence: inner.presence.clone(),
        }
    }

    /// Tell every window the session is over and give their writers a
    /// moment to say so.
    pub fn end(&self, why: &str) {
        if self.ended.swap(true, Ordering::AcqRel) {
            return;
        }
        let item = Item::plain(&Down::Ended(why.to_string()));
        let mut inner = self.inner.lock_recover();
        if let Some(item) = item {
            for client in &inner.clients {
                let _ = client.queue.send(item.clone());
            }
        }
        inner.clients.clear();
        drop(inner);
        std::thread::sleep(Duration::from_millis(150));
    }

    /// Serve one window over `reader`/`writer` until it leaves. Returns at
    /// once; the link runs on its own threads. The first message must be
    /// the window's hello.
    pub fn serve<R, W>(&self, mut reader: R, mut writer: W) -> io::Result<()>
    where
        R: Read + Send + 'static,
        W: Write + Send + 'static,
    {
        let hello: Up = wire::read(&mut reader)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "no hello"))?;
        let Up::Hello { version, kind, size, have, who } = hello else {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "the first message was not a hello"));
        };
        if version != wire::VERSION {
            let why = format!(
                "this session speaks remote glass {}, the window {version}: update angelX on one side",
                wire::VERSION
            );
            wire::write(&mut writer, &Down::Ended(why.clone()))?;
            return Err(io::Error::new(io::ErrorKind::InvalidData, why));
        }
        wire::write(&mut writer, &Down::Hello { version: wire::VERSION, session: self.name.to_string() })?;
        if kind == Kind::Probe {
            wire::write(&mut writer, &Down::Status(self.status()))?;
            // A probe may ask once more: to end the session.
            if let Ok(Some(Up::Kill)) = wire::read::<Up>(&mut reader) {
                self.kill();
            }
            return Ok(());
        }
        let (queue, items) = mpsc::channel::<Item>();
        if kind == Kind::Watch {
            // The presence feed: what it is now, then every change, until
            // the watcher goes. Nothing it sends is read but its leaving.
            let mut inner = self.inner.lock_recover();
            let id = inner.next_id;
            inner.next_id += 1;
            if let Some(item) = Item::plain(&Down::Presence(inner.presence.clone())) {
                let _ = queue.send(item);
            }
            inner.clients.push(Client { id, kind, size, queue });
            drop(inner);
            std::thread::Builder::new()
                .name("remote-watch".into())
                .spawn(move || write_loop(writer, items, HashSet::new()))?;
            let hub = self.clone();
            std::thread::Builder::new().name("remote-watch-read".into()).spawn(move || {
                while let Ok(Some(_)) = wire::read::<Up>(&mut reader) {}
                hub.leave(id);
            })?;
            return Ok(());
        }
        let id = {
            let mut inner = self.inner.lock_recover();
            let id = inner.next_id;
            inner.next_id += 1;
            // Catching up and joining happen under one lock with the
            // publisher, so nothing is missed or sent twice.
            for item in self.catch_up(&mut inner) {
                let _ = queue.send(item);
            }
            inner.clients.push(Client { id, kind, size, queue });
            id
        };
        eprintln!("REMOTE: {who} attached ({kind:?}, {}x{})", size.cols, size.rows);
        // The window that attached last sizes the session.
        self.used(id);
        let held: HashSet<Hash> = have.into_iter().collect();
        std::thread::Builder::new()
            .name("remote-write".into())
            .spawn(move || write_loop(writer, items, held))?;
        let hub = self.clone();
        std::thread::Builder::new().name("remote-read".into()).spawn(move || {
            let why = loop {
                match wire::read::<Up>(&mut reader) {
                    Ok(Some(up)) => hub.take(id, up),
                    Ok(None) => break "left".to_string(),
                    Err(e) => break e.to_string(),
                }
            };
            hub.leave(id);
            eprintln!("REMOTE: {who} detached ({why})");
        })?;
        Ok(())
    }

    /// Give the session cell pixels if no window has (a terminal that
    /// never said its own).
    pub fn default_cell(&self, (width, height): (u32, u32)) {
        let mut grid = self.host.shared.grid.lock_recover();
        if grid.cell_width == 0 || grid.cell_height == 0 {
            grid.cell_width = width;
            grid.cell_height = height;
            grid.width_px = u32::from(grid.cols) * width;
            grid.height_px = u32::from(grid.rows) * height;
        }
    }

    /// Who is at work now: watchers hear it when it changes.
    pub fn set_presence(&self, mut presence: Presence) {
        presence.session = self.name.to_string();
        let mut inner = self.inner.lock_recover();
        if inner.presence == presence {
            return;
        }
        inner.presence = presence.clone();
        if let Some(item) = Item::plain(&Down::Presence(presence)) {
            inner.clients.retain(|c| c.kind != Kind::Watch || c.queue.send(item.clone()).is_ok());
        }
    }

    fn kill(&self) {
        eprintln!("REMOTE: the session was killed");
        self.end("the session was killed");
        // The cockpit's own signal path ends it, as a terminal's hangup would.
        unsafe {
            libc::kill(libc::getpid(), libc::SIGTERM);
        }
    }

    fn leave(&self, id: u64) {
        let mut inner = self.inner.lock_recover();
        inner.clients.retain(|c| c.id != id);
        if inner.sizer == Some(id) {
            // The next to size it is whoever is still here, the newest first.
            inner.sizer = inner.clients.iter().filter(|c| matches!(c.kind, Kind::Window | Kind::Terminal)).map(|c| c.id).max();
            if let Some(next) = inner.sizer {
                drop(inner);
                self.used(next);
            }
        }
    }

    /// Client `id` was just used: it sizes the session now.
    fn used(&self, id: u64) {
        let mut inner = self.inner.lock_recover();
        inner.sizer = Some(id);
        let Some(mut size) = inner.clients.iter().find(|c| c.id == id).map(|c| c.size) else {
            return;
        };
        if size.cols == 0 || size.rows == 0 {
            return;
        }
        let old = *self.host.shared.grid.lock_recover();
        // A window that does not know its cell's pixels yet (its hello
        // comes before it opens) keeps the ones the session has.
        if size.cell_width == 0 || size.cell_height == 0 {
            size.cell_width = old.cell_width;
            size.cell_height = old.cell_height;
            size.width_px = u32::from(size.cols) * size.cell_width;
            size.height_px = u32::from(size.rows) * size.cell_height;
        }
        *self.host.shared.grid.lock_recover() = size;
        if old != size {
            if let Some(item) = Item::plain(&Down::Size(size)) {
                for client in inner.clients.iter().filter(|c| c.kind != Kind::Watch) {
                    let _ = client.queue.send(item.clone());
                }
            }
            drop(inner);
            // New cells or new cell pixels: either way the cockpit refits
            // its layout and its pictures on a resize.
            let _ = self.keys.send(Event::Resize(size.cols, size.rows));
        }
    }

    fn take(&self, id: u64, up: Up) {
        match up {
            Up::Input(event) => {
                let sizer = self.inner.lock_recover().sizer;
                if sizer != Some(id) {
                    self.used(id);
                }
                let _ = self.keys.send(event);
            }
            Up::Resize(size) => {
                let mut inner = self.inner.lock_recover();
                if let Some(client) = inner.clients.iter_mut().find(|c| c.id == id) {
                    client.size = size;
                }
                let sizes = inner.sizer == Some(id);
                drop(inner);
                if sizes {
                    self.used(id);
                }
            }
            Up::Showcase(feedback) => self.feedback(feedback),
            Up::Kill => self.kill(),
            Up::Hello { .. } => {}
        }
    }

    fn feedback(&self, feedback: Feedback) {
        let shared = &self.host.shared;
        if feedback.serial == 0 || shared.showcase_live.load(Ordering::Acquire) != feedback.serial {
            return;
        }
        shared.showcase_clicks.fetch_add(feedback.clicks, Ordering::Relaxed);
        shared.showcase_copies.fetch_add(feedback.copies, Ordering::Relaxed);
        shared.showcase_steps.fetch_add(feedback.steps, Ordering::Relaxed);
        if let Some(at) = feedback.seek.filter(|a| a.is_finite()) {
            shared.showcase_seek.store(at.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
        }
        if feedback.closed {
            shared.showcase_live.store(0, Ordering::Release);
        }
    }

    /// Everything a window needs to draw the session as it is now.
    fn catch_up(&self, inner: &mut Inner) -> Vec<Item> {
        let m = &inner.mirror;
        let mut items = Vec::new();
        let mut push = |message: Down| {
            if let Some(item) = Item::plain(&message) {
                items.push(item);
            }
        };
        push(Down::Size(self.host.grid_size()));
        push(Down::Title(m.title.clone()));
        push(Down::GameKeyboard(m.game_keyboard));
        items.extend(m.pictures.values().cloned());
        items.extend(m.meshes.values().cloned());
        items.extend(m.stages.values().cloned());
        items.extend(m.scenes.values().cloned());
        let mut push = |message: Down| {
            if let Some(item) = Item::plain(&message) {
                items.push(item);
            }
        };
        push(Down::Layers(Arc::clone(&m.layers)));
        push(Down::Grid(Grid {
            cols: m.cols,
            rows: m.rows,
            full: true,
            runs: wire::whole(&m.cells),
            cursor: m.cursor,
            card: match &m.card {
                Some(card) => CardChange::Card(wire_card(card)),
                None => CardChange::Gone,
            },
        }));
        if let Some(open) = &m.showcase {
            items.push(open.clone());
            if let Some(item) = Item::plain(&Down::ShowcaseProgress { at: m.progress.0, paused: m.progress.1 }) {
                items.push(item);
            }
        }
        if let Some(item) = Item::plain(&Down::Present) {
            items.push(item);
        }
        items
    }

    fn broadcast(&self, inner: &mut Inner, items: Vec<Item>) {
        if items.is_empty() {
            return;
        }
        inner.clients.retain(|client| {
            client.kind == Kind::Watch || items.iter().all(|item| client.queue.send(item.clone()).is_ok())
        });
    }

    /// The publisher: wakes on the host's events, takes what changed and
    /// sends it to every window as one frame.
    fn publish(self, events: Receiver<UserEvent>) {
        let mut waiting: Option<Instant> = None;
        loop {
            let event = match waiting {
                Some(since) => {
                    let left = LAYERS_WAIT.saturating_sub(since.elapsed());
                    match events.recv_timeout(left) {
                        Ok(event) => Some(event),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                None => match events.recv() {
                    Ok(event) => Some(event),
                    Err(_) => return,
                },
            };
            let mut items = Vec::new();
            let mut frame = false;
            match event {
                // A layers-only change waits a moment for its frame.
                Some(UserEvent::Layers) => {
                    if waiting.is_none() {
                        waiting = Some(Instant::now());
                    }
                    continue;
                }
                None => frame = true,
                Some(UserEvent::Frame | UserEvent::Pictures | UserEvent::Showcase) => frame = true,
                Some(UserEvent::Title(title)) => {
                    self.inner.lock_recover().mirror.title = title.clone();
                    items.extend(Item::plain(&Down::Title(title)));
                }
                Some(UserEvent::Attention) => items.extend(Item::plain(&Down::Attention)),
                Some(UserEvent::Clipboard(text)) => items.extend(Item::plain(&Down::Clipboard(text))),
                Some(UserEvent::ExitProcess(code)) => {
                    self.end("the cockpit quit");
                    std::process::exit(code);
                }
                Some(UserEvent::Exit) => {
                    self.end("the cockpit quit");
                    return;
                }
                Some(UserEvent::Capture(_) | UserEvent::Film(_) | UserEvent::Record(_) | UserEvent::GpuReady | UserEvent::Close) => {}
            }
            waiting = None;
            let mut inner = self.inner.lock_recover();
            if frame {
                items.extend(self.take_frame(&mut inner.mirror));
            }
            self.broadcast(&mut inner, items);
        }
    }

    /// What the window's render thread takes each frame, as messages; the
    /// mirror keeps the latest of each for windows that attach later.
    fn take_frame(&self, m: &mut Mirror) -> Vec<Item> {
        let shared = &self.host.shared;
        let mut items = Vec::new();
        // Pictures, meshes, stages and scenes go before the frame that
        // shows them, as the window applies them before it draws.
        let pictures = std::mem::take(&mut *shared.pictures.lock_recover());
        for picture in pictures {
            let now = Instant::now();
            let stream = picture.id == SHOWCASE_PICTURE
                || m.uploaded_at.get(&picture.id).is_some_and(|at| now.duration_since(*at) < STREAM_WITHIN);
            if stream && !m.streams.contains(&picture.id) {
                m.streams.insert(picture.id);
                if std::env::var("ANGEL_REMOTE_STATS").is_ok_and(|v| v == "1") {
                    eprintln!("REMOTE_STATS: picture {:#x} ({}x{}) streams", picture.id, picture.width, picture.height);
                }
            }
            m.uploaded_at.insert(picture.id, now);
            let (id, width, height, cols, rows) = (picture.id, picture.width, picture.height, picture.cols, picture.rows);
            let blob = Blob::new(picture.rgba);
            let message = Down::Picture { id, width, height, cols, rows, hash: blob.hash, keep: !stream };
            if let Some(item) = Item::with(&message, vec![(blob, !stream)]) {
                // Kept for a late window as a kept picture: it has no stream.
                let late = Down::Picture { id, width, height, cols, rows, hash: item.blobs[0].0.hash, keep: true };
                if let Some(kept) = Item::with(&late, vec![(Arc::clone(&item.blobs[0].0), true)]) {
                    m.pictures.insert(id, kept);
                }
                items.push(item);
            }
        }
        let meshes = std::mem::take(&mut *shared.meshes.lock_recover());
        for (id, mesh) in meshes {
            let Ok(bytes) = wire::encode(&mesh) else { continue };
            let blob = Blob::new(bytes);
            if let Some(item) = Item::with(&Down::Mesh { id, hash: blob.hash }, vec![(blob, true)]) {
                m.meshes.insert(id, item.clone());
                items.push(item);
            }
        }
        let stages = std::mem::take(&mut *shared.stages.lock_recover());
        for (id, stage) in stages {
            let item = match stage {
                Some(stage) => self.stage_item(m, id, stage),
                None => Item::plain(&Down::Stage { id, stage: None, meshes: Vec::new() }),
            };
            if let Some(item) = item {
                m.stages.insert(id, item.clone());
                items.push(item);
            }
        }
        m.world_meshes.retain(|_, (weak, _)| weak.strong_count() > 0);
        let scenes = std::mem::take(&mut *shared.scenes.lock_recover());
        for (id, scene) in scenes {
            let gone = scene.is_none();
            if let Some(item) = Item::plain(&Down::Scene { id, scene }) {
                if gone {
                    m.scenes.remove(&id);
                } else {
                    m.scenes.insert(id, item.clone());
                }
                items.push(item);
            }
        }
        // The showcase: its request, its marks and model, its progress.
        if let Some(request) = shared.showcase.lock_recover().take() {
            match request {
                Request::Open { title, size, fit, livery, herald, retinue, names, nav } => {
                    let serial = shared.showcase_live.load(Ordering::Acquire);
                    let marks = std::mem::take(&mut *shared.showcase_marks.lock_recover());
                    let model = shared.showcase_model.lock_recover().take();
                    let showing = Showing { title, size, fit, herald, livery, retinue, names, back: nav.0, forward: nav.1 };
                    m.showcase_serial = serial;
                    self.serial.store(serial, Ordering::Relaxed);
                    let item = Item::plain(&Down::ShowcaseOpen { serial, showing, marks, model });
                    m.showcase = item.clone();
                    items.extend(item);
                }
                Request::Close => {
                    m.showcase = None;
                    items.extend(Item::plain(&Down::ShowcaseClose));
                }
            }
        }
        let progress = {
            let at = f32::from_bits(shared.showcase_progress.load(Ordering::Relaxed));
            ((!at.is_nan()).then_some(at), shared.showcase_paused.load(Ordering::Relaxed))
        };
        if progress != m.progress {
            m.progress = progress;
            items.extend(Item::plain(&Down::ShowcaseProgress { at: progress.0, paused: progress.1 }));
        }
        let game_keyboard = shared.game_keyboard.load(Ordering::Relaxed);
        if game_keyboard != m.game_keyboard {
            m.game_keyboard = game_keyboard;
            items.extend(Item::plain(&Down::GameKeyboard(game_keyboard)));
        }
        let layers = Arc::clone(&shared.layers.lock_recover());
        if !Arc::ptr_eq(&layers, &m.layers) && *layers != *m.layers {
            items.extend(Item::plain(&Down::Layers(Arc::clone(&layers))));
        }
        m.layers = layers;
        // The grid, as the render thread takes it: swap the snapshot.
        let fresh = {
            let mut slot = shared.slot.lock_recover();
            let fresh = slot.fresh;
            if fresh {
                std::mem::swap(&mut slot.snapshot, &mut m.current);
                slot.fresh = false;
            }
            fresh
        };
        if fresh {
            let snapshot = &m.current;
            let full = snapshot.cols != m.cols || snapshot.rows != m.rows || m.cells.len() != snapshot.cells.len();
            let runs = if full { wire::whole(&snapshot.cells) } else { wire::diff(&m.cells, &snapshot.cells) };
            let card = match (&snapshot.card, &m.card) {
                (None, None) => CardChange::Same,
                (Some(a), Some(b)) if Arc::ptr_eq(a, b) => CardChange::Same,
                (Some(card), _) => CardChange::Card(wire_card(card)),
                (None, Some(_)) => CardChange::Gone,
            };
            let changed = full || !runs.is_empty() || snapshot.cursor != m.cursor || card != CardChange::Same;
            if changed {
                items.extend(Item::plain(&Down::Grid(Grid {
                    cols: snapshot.cols,
                    rows: snapshot.rows,
                    full,
                    runs,
                    cursor: snapshot.cursor,
                    card,
                })));
            }
            m.cells.clone_from(&snapshot.cells);
            m.cols = snapshot.cols;
            m.rows = snapshot.rows;
            m.cursor = snapshot.cursor;
            m.card.clone_from(&snapshot.card);
        }
        if !items.is_empty() {
            items.extend(Item::plain(&Down::Present));
        }
        items
    }

    /// A stage as a message: its world's meshes leave it as blobs (made once
    /// per mesh), and the set's text and plate, which only a far side that
    /// remakes the meshes reads, stay home.
    fn stage_item(&self, m: &mut Mirror, id: u32, mut stage: crate::Stage) -> Option<Item> {
        let mut meshes = Vec::new();
        let mut blobs = Vec::new();
        if let Some(world) = stage.props.as_mut().and_then(|p| p.world.as_mut()) {
            for (mesh_id, mesh) in std::mem::take(&mut world.meshes) {
                let key = Arc::as_ptr(&mesh) as usize;
                let blob = match m.world_meshes.get(&key) {
                    Some((weak, blob)) if weak.upgrade().is_some_and(|w| Arc::ptr_eq(&w, &mesh)) => Arc::clone(blob),
                    _ => {
                        let blob = Blob::new(wire::encode(&*mesh).ok()?);
                        m.world_meshes.insert(key, (Arc::downgrade(&mesh), Arc::clone(&blob)));
                        blob
                    }
                };
                meshes.push((mesh_id, blob.hash));
                blobs.push((blob, true));
            }
            for set in world.set.iter_mut().chain(world.more.iter_mut()) {
                set.text = None;
                set.png = None;
            }
        }
        Item::with(&Down::Stage { id, stage: Some(stage), meshes }, blobs)
    }
}

fn wire_card(card: &crate::card::Card) -> WireCard {
    WireCard {
        area: [card.area.x, card.area.y, card.area.width, card.area.height],
        cells: card.buffer.content.iter().map(WireCell::of).collect(),
        title: card.title.clone(),
        serial: card.serial,
        cursor: card.cursor,
        field: card.field,
        knight: card.knight,
        busy: card.busy,
        entrance: card.entrance,
    }
}

/// A window's writer: each item's blobs first (those the window lacks),
/// then the item. Ends when the window's link breaks or the hub drops it.
fn write_loop<W: Write>(mut writer: W, items: Receiver<Item>, mut held: HashSet<Hash>) {
    // The last pictures this window was sent, any id, newest last: the
    // window keeps the same ring (`client::Screen`), so either end can name
    // one as the base of a delta.
    let mut recent: std::collections::VecDeque<(u32, Arc<Blob>)> = std::collections::VecDeque::new();
    let mut send = |writer: &mut W, item: Item| -> io::Result<()> {
        if let (Some((id, width, height, cols, rows)), [(blob, keep)]) = (item.picture, item.blobs.as_slice()) {
            let held_already = *keep && held.contains(&blob.hash);
            // The base: this id's last picture, else the newest of the same size.
            let base = recent
                .iter()
                .rev()
                .filter(|(_, b)| b.raw.len() == blob.raw.len() && b.hash != blob.hash)
                .max_by_key(|(other, _)| *other == id)
                .map(|(_, b)| Arc::clone(b));
            if recent.len() == super::wire::RECENT {
                recent.pop_front();
            }
            recent.push_back((id, Arc::clone(blob)));
            if let Some(base) = base.filter(|_| !held_already) {
                let (deflated, bytes) = wire::delta(&base.raw, &blob.raw);
                if bytes.len() < blob.raw.len() / 2 {
                    let message = Down::PictureDelta { id, width, height, cols, rows, base: base.hash, hash: blob.hash, deflated, bytes };
                    let frame = wire::frame(&message)?;
                    stats::add(8, frame.len());
                    return writer.write_all(&frame);
                }
            }
        }
        for (blob, keep) in &item.blobs {
            if *keep && held.contains(&blob.hash) {
                continue;
            }
            let framed = blob.framed()?;
            writer.write_all(&framed)?;
            stats::add(6, framed.len());
            if *keep {
                held.insert(blob.hash);
            }
        }
        stats::add(item.kind, item.frame.len());
        if item.kind == 7 && item.frame.len() <= 8 {
            stats::FRAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        writer.write_all(&item.frame)
    };
    while let Ok(item) = items.recv() {
        if send(&mut writer, item).is_err() {
            return;
        }
        // Everything already waiting goes in the same flush.
        while let Ok(item) = items.try_recv() {
            if send(&mut writer, item).is_err() {
                return;
            }
        }
        if writer.flush().is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::client::Screen;
    use ratatui_core::backend::Backend;

    fn size(cols: u16, rows: u16) -> GridSize {
        GridSize { cols, rows, width_px: u32::from(cols) * 9, height_px: u32::from(rows) * 18, cell_width: 9, cell_height: 18 }
    }

    /// A window's end of a link: a pipe pair the hub serves.
    fn attach(hub: &Hub, kind: Kind, size: GridSize, have: Vec<Hash>) -> (std::os::unix::net::UnixStream, std::os::unix::net::UnixStream) {
        let (ours, theirs) = std::os::unix::net::UnixStream::pair().unwrap();
        let mut w = ours.try_clone().unwrap();
        wire::write(&mut w, &Up::Hello { version: wire::VERSION, kind, size, have, who: "test".into() }).unwrap();
        let (r, wr) = (theirs.try_clone().unwrap(), theirs);
        let hub = hub.clone();
        std::thread::spawn(move || hub.serve(r, wr).unwrap());
        (ours, w)
    }

    fn read_until_present(link: &mut std::os::unix::net::UnixStream, screen: &mut Screen) -> Vec<Down> {
        link.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut seen = Vec::new();
        loop {
            let down: Down = wire::read(link).unwrap().expect("a message");
            let present = matches!(down, Down::Present);
            screen.take(&down);
            seen.push(down);
            if present {
                return seen;
            }
        }
    }

    #[test]
    fn a_window_sees_the_cockpits_cells_and_input_comes_back() {
        let (host, hub) = headless("test", size(20, 5));
        let mut backend = host.backend();
        let mut text = Cell::new("h");
        text.fg = ratatui_core::style::Color::Rgb(200, 10, 10);
        backend.draw([(0u16, 0u16, &text), (1, 0, &Cell::new("i"))].into_iter()).unwrap();
        backend.flush().unwrap();
        std::thread::sleep(Duration::from_millis(50));

        let (mut link, mut up) = attach(&hub, Kind::Window, size(20, 5), Vec::new());
        assert!(matches!(wire::read::<Down>(&mut link).unwrap(), Some(Down::Hello { .. })));
        let mut screen = Screen::default();
        read_until_present(&mut link, &mut screen);
        assert_eq!(screen.cells[0], text);
        assert_eq!(screen.cells[1].symbol(), "i");

        // A change goes as a diff and lands.
        backend.draw([(5u16, 2u16, &Cell::new("Z"))].into_iter()).unwrap();
        backend.flush().unwrap();
        let seen = read_until_present(&mut link, &mut screen);
        let Some(Down::Grid(grid)) = seen.iter().find(|d| matches!(d, Down::Grid(_))) else { panic!("{seen:?}") };
        assert!(!grid.full);
        assert_eq!(grid.runs.iter().map(|r| r.1.len()).sum::<usize>(), 1);
        assert_eq!(screen.cells[2 * 20 + 5].symbol(), "Z");

        // A key comes back as the cockpit's input.
        let key = Event::Key(crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Char('q')));
        wire::write(&mut up, &Up::Input(key.clone())).unwrap();
        let mut input = host.input_reader().unwrap();
        let got = loop {
            match input(Duration::from_secs(2)).unwrap() {
                Some(Event::Resize(..)) => continue,
                other => break other,
            }
        };
        assert_eq!(got, Some(key));
    }

    #[test]
    fn a_picture_goes_once_and_a_late_window_gets_it_too() {
        let (host, hub) = headless("pics", size(10, 4));
        let (mut link, _up) = attach(&hub, Kind::Window, size(10, 4), Vec::new());
        let mut screen = Screen::default();
        assert!(matches!(wire::read::<Down>(&mut link).unwrap(), Some(Down::Hello { .. })));
        read_until_present(&mut link, &mut screen);

        let rgba: Vec<u8> = (0..64 * 64 * 4).map(|i| (i % 251) as u8).collect();
        host.upload_picture(crate::graphics::Picture { id: 7, width: 64, height: 64, rgba: rgba.clone(), cols: 0, rows: 0 });
        let mut backend = host.backend();
        backend.draw([(0u16, 0u16, &Cell::new("p"))].into_iter()).unwrap();
        backend.flush().unwrap();
        let seen = read_until_present(&mut link, &mut screen);
        assert!(seen.iter().any(|d| matches!(d, Down::Blob { .. })));
        assert_eq!(screen.picture(7).map(|p| p.rgba.to_vec()), Some(rgba.clone()));

        // The same picture a few pixels on goes as a delta, not again whole.
        let mut moved = rgba.clone();
        moved[400..420].fill(255);
        host.upload_picture(crate::graphics::Picture { id: 7, width: 64, height: 64, rgba: moved.clone(), cols: 0, rows: 0 });
        backend.draw([(1u16, 0u16, &Cell::new("q"))].into_iter()).unwrap();
        backend.flush().unwrap();
        let seen = read_until_present(&mut link, &mut screen);
        let delta = seen.iter().find_map(|d| match d {
            Down::PictureDelta { bytes, .. } => Some(bytes.len()),
            _ => None,
        });
        assert!(delta.is_some_and(|n| n < 200), "{seen:?}");
        assert!(!seen.iter().any(|d| matches!(d, Down::Blob { .. })));
        assert_eq!(screen.picture(7).map(|p| p.rgba.to_vec()), Some(moved.clone()));
        let rgba = moved;

        // A second window that already holds the blob is not sent it again.
        let hash = wire::hash(&rgba);
        let (mut late, _up2) = attach(&hub, Kind::Window, size(10, 4), vec![hash]);
        let mut late_screen = Screen::default();
        late_screen.keep(hash, rgba.clone());
        assert!(matches!(wire::read::<Down>(&mut late).unwrap(), Some(Down::Hello { .. })));
        let seen = read_until_present(&mut late, &mut late_screen);
        assert!(!seen.iter().any(|d| matches!(d, Down::Blob { .. })), "held blobs stay home");
        assert_eq!(late_screen.picture(7).map(|p| p.rgba.to_vec()), Some(rgba));
    }

    #[test]
    fn an_animation_taking_turns_under_two_ids_goes_as_deltas() {
        let (host, hub) = headless("turns", size(10, 4));
        let (mut link, _up) = attach(&hub, Kind::Window, size(10, 4), Vec::new());
        let mut screen = Screen::default();
        assert!(matches!(wire::read::<Down>(&mut link).unwrap(), Some(Down::Hello { .. })));
        read_until_present(&mut link, &mut screen);
        let mut backend = host.backend();
        let base: Vec<u8> = (0..96 * 64 * 4).map(|i| (i % 13) as u8).collect();
        let mut deltas = 0;
        for frame in 0..6usize {
            let mut rgba = base.clone();
            rgba[frame * 40..frame * 40 + 16].fill(200);
            let id = 100 + (frame % 2) as u32;
            host.upload_picture(crate::graphics::Picture { id, width: 96, height: 64, rgba: rgba.clone(), cols: 0, rows: 0 });
            backend.draw([(0u16, 0u16, &Cell::new(if frame % 2 == 0 { "a" } else { "b" }))].into_iter()).unwrap();
            backend.flush().unwrap();
            let seen = read_until_present(&mut link, &mut screen);
            deltas += seen.iter().filter(|d| matches!(d, Down::PictureDelta { .. })).count();
            assert_eq!(screen.picture(id).map(|p| p.rgba.to_vec()), Some(rgba), "frame {frame}");
        }
        assert_eq!(deltas, 5, "every frame after the first is a delta");
    }

    #[test]
    fn the_last_window_used_sizes_the_session() {
        let (host, hub) = headless("size", size(80, 24));
        let (mut a, mut a_up) = attach(&hub, Kind::Window, size(100, 30), Vec::new());
        assert!(matches!(wire::read::<Down>(&mut a).unwrap(), Some(Down::Hello { .. })));
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!((host.grid_size().cols, host.grid_size().rows), (100, 30));
        let (mut b, _b_up) = attach(&hub, Kind::Terminal, size(60, 20), Vec::new());
        assert!(matches!(wire::read::<Down>(&mut b).unwrap(), Some(Down::Hello { .. })));
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(host.grid_size().cols, 60, "the newest window sizes it");
        // Typing in the first takes the size back.
        let key = Event::Key(crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Char('a')));
        wire::write(&mut a_up, &Up::Input(key)).unwrap();
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(host.grid_size().cols, 100);
        // A probe answers with the session and two windows.
        let (mut p, _p_up) = attach(&hub, Kind::Probe, GridSize::default(), Vec::new());
        assert!(matches!(wire::read::<Down>(&mut p).unwrap(), Some(Down::Hello { .. })));
        let Some(Down::Status(status)) = wire::read::<Down>(&mut p).unwrap() else { panic!() };
        assert_eq!((status.name.as_str(), status.clients), ("size", 2));
        drop(a);
    }

    #[test]
    fn a_stage_crosses_with_its_world_meshes_sent_once() {
        use crate::scene::{Props, World};
        let (host, hub) = headless("stage", size(10, 4));
        let (mut link, _up) = attach(&hub, Kind::Window, size(10, 4), Vec::new());
        let mut screen = Screen::default();
        assert!(matches!(wire::read::<Down>(&mut link).unwrap(), Some(Down::Hello { .. })));
        read_until_present(&mut link, &mut screen);

        let ground = Arc::new(MeshData::cube());
        let stage = |n: u64| crate::Stage {
            id: 0x00FA_57A4,
            size: (384, 224),
            cols: 8,
            rows: 3,
            layers: vec![Layer::new(n, 0, crate::Placement::Pixels([1.0, 2.0, 30.0, 40.0]), crate::LayerKind::Picture(5))],
            scene: 9,
            enter: Some([1.0, 0.0]),
            tilt: 0.3,
            props: Some(Props {
                at: 1,
                grain: 2,
                nodes: Vec::new(),
                world: Some(World { meshes: vec![(77, Arc::clone(&ground))], ..World::default() }),
            }),
            impact: 0.25,
        };
        let mut backend = host.backend();
        for frame in 0..3u64 {
            host.set_stage(stage(frame));
            backend.draw([(0u16, 0u16, &Cell::new(if frame % 2 == 0 { "a" } else { "b" }))].into_iter()).unwrap();
            backend.flush().unwrap();
            let seen = read_until_present(&mut link, &mut screen);
            let blobs = seen.iter().filter(|d| matches!(d, Down::Blob { .. })).count();
            assert_eq!(blobs, usize::from(frame == 0), "the world's mesh goes once (frame {frame})");
            let Some(Down::Stage { stage: Some(got), meshes, .. }) = seen.iter().find(|d| matches!(d, Down::Stage { .. })) else {
                panic!("{seen:?}")
            };
            let mut want = stage(frame);
            want.props.as_mut().unwrap().world.as_mut().unwrap().meshes.clear();
            assert_eq!(got, &want, "the stage itself is as set");
            let (id, hash) = meshes[0];
            assert_eq!(id, 77);
            assert_eq!(*screen.mesh(&hash).unwrap(), MeshData::cube(), "the window remakes the mesh from its blob");
        }
    }

    #[test]
    fn a_watcher_hears_who_is_at_work_and_nothing_else() {
        use crate::remote::wire::{Knight, KnightStatus};
        let (host, hub) = headless("feed", size(10, 4));
        let (mut watch, _up) = attach(&hub, Kind::Watch, GridSize::default(), Vec::new());
        watch.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        assert!(matches!(wire::read::<Down>(&mut watch).unwrap(), Some(Down::Hello { .. })));
        let Some(Down::Presence(first)) = wire::read::<Down>(&mut watch).unwrap() else { panic!() };
        assert_eq!((first.session.as_str(), first.knights.len()), ("feed", 0));
        // A frame of the cockpit's does not reach a watcher; presence does.
        let mut backend = host.backend();
        backend.draw([(0u16, 0u16, &Cell::new("x"))].into_iter()).unwrap();
        backend.flush().unwrap();
        let knight = Knight { family: "qwen".into(), persona: "Turbo".into(), doing: "reading".into(), status: KnightStatus::Working, task: "fix the link".into() };
        hub.set_presence(Presence { workspace: "angelX".into(), knights: vec![knight.clone()], ..Presence::default() });
        let Some(Down::Presence(now)) = wire::read::<Down>(&mut watch).unwrap() else { panic!("frames reached a watcher") };
        assert_eq!((now.session.as_str(), now.workspace.as_str(), now.knights), ("feed", "angelX", vec![knight]));
        assert_eq!(hub.status().clients, 0, "a watcher is not an attached window");
    }

    #[test]
    fn a_window_of_another_version_is_told_in_one_line() {
        let (_host, hub) = headless("v", size(10, 4));
        let (ours, theirs) = std::os::unix::net::UnixStream::pair().unwrap();
        let mut w = ours.try_clone().unwrap();
        wire::write(&mut w, &Up::Hello { version: 999, kind: Kind::Window, size: size(10, 4), have: Vec::new(), who: "x".into() }).unwrap();
        assert!(hub.serve(theirs.try_clone().unwrap(), theirs).is_err());
        let mut r = ours;
        let Some(Down::Ended(why)) = wire::read::<Down>(&mut r).unwrap() else { panic!() };
        assert!(why.contains("update angelX"), "{why}");
    }
}
