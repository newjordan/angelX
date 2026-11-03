//! The window's end: a glass window drawing a session that runs on another
//! machine (docs/REMOTE_GLASS.md).
//!
//! The window is the same one a local cockpit gets; here its "cockpit
//! thread" is a relay that reads the session's messages and hands them to
//! the [`Host`] as the cockpit would (cells to the backend, pictures,
//! layers, stages, scenes, the showcase), and sends the window's input back.
//! A broken link shows "reconnecting" and attaches again; the session on
//! the far side never noticed.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui_core::backend::Backend;
use ratatui_core::buffer::{Buffer, Cell};
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Modifier};

use super::wire::{self, CardChange, Down, Feedback, Grid, Hash, Kind, Up, WireCard};
use crate::scene::MeshData;
use crate::{GridSize, Host, LockExt, Options};

/// The most bytes of blobs a window keeps between links. Within one link
/// it keeps all it was sent (the session counts on it); a new link starts
/// from the newest that fit.
const KEEP_BYTES: usize = 1 << 30;

/// One end of a link to a session: what the window reads and writes, and
/// the process carrying it (an `ssh`), ended when the link is let go.
pub struct Link {
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
    pub child: Option<std::process::Child>,
}

impl Link {
    fn close(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Why connecting failed: worth another try, or not.
#[derive(Debug)]
pub enum Fail {
    /// Say it once and stop (the far angelX is too old, or the versions
    /// differ).
    Fatal(String),
    /// Say it and try again.
    Again(String),
}

impl std::fmt::Display for Fail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fail::Fatal(s) | Fail::Again(s) => f.write_str(s),
        }
    }
}

/// How an attach ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The operator detached (the key, or closing the window); the session
    /// runs on.
    Detached,
    /// The session ended, saying why.
    Ended(String),
}

/// Say hello over `link` and wait for the session's. On success the link
/// is ready for messages, and the session's name is returned.
pub fn handshake(link: &mut Link, kind: Kind, size: GridSize, have: Vec<Hash>, who: &str) -> Result<String, Fail> {
    let hello = Up::Hello { version: wire::VERSION, kind, size, have, who: who.to_string() };
    if let Err(e) = wire::write(&mut link.writer, &hello) {
        // The far end may already have said why; read it below.
        let _ = e;
    }
    if let Err(said) = wire::await_magic(&mut link.reader) {
        let last = said.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
        let old = last.contains("unknown or trailing") || last.contains("unrecognized") || last.contains("Usage");
        return Err(if old {
            Fail::Fatal(format!("the angelX there is too old for remote glass ({last}); update it there"))
        } else if last.is_empty() {
            Fail::Again("the link closed before a session answered".into())
        } else {
            Fail::Again(last)
        });
    }
    match wire::read::<Down>(&mut link.reader) {
        Ok(Some(Down::Hello { version, session })) if version == wire::VERSION => Ok(session),
        Ok(Some(Down::Hello { version, .. })) => Err(Fail::Fatal(format!(
            "the session speaks remote glass {version}, this angelX {}: update angelX on one side",
            wire::VERSION
        ))),
        Ok(Some(Down::Ended(why))) => Err(Fail::Fatal(why)),
        Ok(Some(other)) => Err(Fail::Again(format!("the session said {other:?} first"))),
        Ok(None) => Err(Fail::Again("the session closed the link".into())),
        Err(e) => Err(Fail::Again(e.to_string())),
    }
}

/// A picture as the session last sent it.
pub struct Shown {
    pub id: u32,
    pub hash: Hash,
    pub width: u32,
    pub height: u32,
    pub cols: u16,
    pub rows: u16,
    pub rgba: Arc<Vec<u8>>,
}

/// What any window keeps of a session: the grid, the blobs and the latest
/// picture of each id. The window and the terminal both draw from it.
#[derive(Default)]
pub struct Screen {
    pub size: GridSize,
    pub cols: u16,
    pub rows: u16,
    pub cells: Vec<Cell>,
    pub cursor: Option<(u16, u16)>,
    pub card: Option<WireCard>,
    /// The grid was replaced whole since `changed` was last taken.
    pub full: bool,
    /// Cells changed since last taken.
    pub changed: Vec<usize>,
    blobs: HashMap<Hash, (Arc<Vec<u8>>, u64)>,
    blob_bytes: usize,
    pictures: HashMap<u32, Shown>,
    meshes: HashMap<Hash, Arc<MeshData>>,
    /// The last pictures sent, any id, as the session keeps them: the
    /// bases of deltas (`wire::RECENT`).
    recent: std::collections::VecDeque<(Hash, Arc<Vec<u8>>)>,
    tick: u64,
}

impl Screen {
    /// Take one message's state. Returns false for messages that carry
    /// nothing a screen keeps (the window acts on those itself).
    pub fn take(&mut self, down: &Down) -> bool {
        match down {
            Down::Size(size) => self.size = *size,
            Down::Grid(grid) => self.grid(grid),
            Down::Blob { hash, deflated, bytes } => {
                let raw = if *deflated {
                    match wire::inflate(bytes) {
                        Ok(raw) => raw,
                        Err(e) => {
                            eprintln!("REMOTE: a blob did not inflate: {e}");
                            return true;
                        }
                    }
                } else {
                    bytes.clone()
                };
                self.keep(*hash, raw);
            }
            Down::Picture { id, width, height, cols, rows, hash, keep } => {
                let rgba = if *keep {
                    self.blobs.get(hash).map(|(b, _)| Arc::clone(b))
                } else {
                    self.blobs.remove(hash).map(|(b, _)| {
                        self.blob_bytes -= b.len();
                        b
                    })
                };
                match rgba {
                    Some(rgba) => self.show(Shown { id: *id, hash: *hash, width: *width, height: *height, cols: *cols, rows: *rows, rgba }),
                    None => eprintln!("REMOTE: picture {id} named a blob this window does not hold"),
                }
            }
            Down::PictureDelta { id, width, height, cols, rows, base, hash, deflated, bytes } => {
                let next = self
                    .recent
                    .iter()
                    .find(|(h, _)| h == base)
                    .and_then(|(_, rgba)| wire::undelta(rgba, *deflated, bytes));
                match next {
                    Some(rgba) => self.show(Shown { id: *id, hash: *hash, width: *width, height: *height, cols: *cols, rows: *rows, rgba: Arc::new(rgba) }),
                    None => eprintln!("REMOTE: picture {id} changed from a picture this window does not hold"),
                }
            }
            _ => return false,
        }
        true
    }

    fn show(&mut self, shown: Shown) {
        if self.recent.len() == wire::RECENT {
            self.recent.pop_front();
        }
        self.recent.push_back((shown.hash, Arc::clone(&shown.rgba)));
        self.pictures.insert(shown.id, shown);
    }

    /// Hold a blob.
    pub fn keep(&mut self, hash: Hash, raw: Vec<u8>) {
        self.tick += 1;
        self.blob_bytes += raw.len();
        if let Some((old, _)) = self.blobs.insert(hash, (Arc::new(raw), self.tick)) {
            self.blob_bytes -= old.len();
        }
    }

    pub fn blob(&self, hash: &Hash) -> Option<Arc<Vec<u8>>> {
        self.blobs.get(hash).map(|(b, _)| Arc::clone(b))
    }

    /// A mesh from its blob, decoded once.
    pub fn mesh(&mut self, hash: &Hash) -> Option<Arc<MeshData>> {
        if let Some(mesh) = self.meshes.get(hash) {
            return Some(Arc::clone(mesh));
        }
        let bytes = self.blob(hash)?;
        let mesh: MeshData = wire::decode(&bytes).ok()?;
        let mesh = Arc::new(mesh);
        self.meshes.insert(*hash, Arc::clone(&mesh));
        Some(mesh)
    }

    pub fn picture(&self, id: u32) -> Option<&Shown> {
        self.pictures.get(&id)
    }

    /// The blobs to name in the next hello: the newest that fit the budget
    /// (the rest are let go).
    pub fn held(&mut self) -> Vec<Hash> {
        if self.blob_bytes > KEEP_BYTES {
            let mut by_age: Vec<(u64, Hash, usize)> = self.blobs.iter().map(|(h, (b, t))| (*t, *h, b.len())).collect();
            by_age.sort_unstable_by(|a, b| b.0.cmp(&a.0));
            let mut total = 0;
            for (_, hash, len) in by_age {
                total += len;
                if total > KEEP_BYTES {
                    self.blobs.remove(&hash);
                    self.meshes.remove(&hash);
                    self.blob_bytes -= len;
                }
            }
        }
        self.blobs.keys().copied().collect()
    }

    fn grid(&mut self, grid: &Grid) {
        let len = usize::from(grid.cols) * usize::from(grid.rows);
        if grid.full || grid.cols != self.cols || grid.rows != self.rows || self.cells.len() != len {
            self.cols = grid.cols;
            self.rows = grid.rows;
            self.cells = vec![Cell::default(); len];
            self.full = true;
            self.changed.clear();
        }
        for (start, cells) in &grid.runs {
            let start = *start as usize;
            for (k, cell) in cells.iter().enumerate() {
                if let Some(slot) = self.cells.get_mut(start + k) {
                    *slot = cell.cell();
                    if !self.full {
                        self.changed.push(start + k);
                    }
                }
            }
        }
        self.cursor = grid.cursor;
        match &grid.card {
            CardChange::Same => {}
            CardChange::Gone => self.card = None,
            CardChange::Card(card) => self.card = Some(card.clone()),
        }
    }
}

/// A card back from the wire.
fn card(wire: &WireCard) -> Option<crate::card::Card> {
    let [x, y, w, h] = wire.area;
    let mut buffer = Buffer::empty(Rect::new(0, 0, w, h));
    if buffer.content.len() != wire.cells.len() {
        return None;
    }
    for (slot, cell) in buffer.content.iter_mut().zip(&wire.cells) {
        *slot = cell.cell();
    }
    let mut card = crate::card::Card::new(Rect::new(x, y, w, h), buffer)?;
    card.title.clone_from(&wire.title);
    card.serial = wire.serial;
    card.cursor = wire.cursor;
    card.field = wire.field;
    card.knight = wire.knight;
    card.busy = wire.busy;
    card.entrance = wire.entrance;
    Some(card)
}

/// Whether a key is the detach key: Ctrl+\ (as `dtach` has it). A terminal
/// reports it as Ctrl+4 too.
pub fn is_detach(event: &Event) -> bool {
    matches!(
        event,
        Event::Key(KeyEvent { code: KeyCode::Char('\\' | '4'), modifiers, kind: KeyEventKind::Press, .. })
            if modifiers.contains(KeyModifiers::CONTROL)
    )
}

enum Ev {
    Down(u64, Down),
    Lost(u64, String),
    Resized,
    Detach,
}

/// Open a glass window on the session behind `link` (already through its
/// [`handshake`]). `connect` makes a fresh link when this one breaks,
/// given the blobs the window holds; `label` names the session in the
/// window (its host and name).
pub fn attach_window<C>(options: Options, link: Link, label: String, connect: C) -> Result<Outcome, String>
where
    C: FnMut(GridSize, Vec<Hash>) -> Result<Link, Fail> + Send + 'static,
{
    crate::run(options, move |host| relay(host, link, label, connect))
}

fn relay<C>(host: Host, mut link: Link, label: String, mut connect: C) -> Outcome
where
    C: FnMut(GridSize, Vec<Hash>) -> Result<Link, Fail>,
{
    let mut backend = host.backend();
    let (tx, rx) = mpsc::channel::<Ev>();
    let writer: Arc<Mutex<Option<Box<dyn Write + Send>>>> = Arc::new(Mutex::new(None));
    let closed = Arc::new(AtomicBool::new(false));
    // The window's input goes up the link; resizes and the detach key
    // come back here too.
    match host.input_reader() {
        Ok(mut input) => {
            let (writer, tx, host, closed) = (Arc::clone(&writer), tx.clone(), host.clone(), Arc::clone(&closed));
            std::thread::Builder::new()
                .name("remote-input".into())
                .spawn(move || loop {
                    let event = match input(Duration::from_millis(250)) {
                        Ok(Some(event)) => event,
                        Ok(None) => continue,
                        Err(_) => {
                            closed.store(true, Ordering::Release);
                            let _ = tx.send(Ev::Detach);
                            return;
                        }
                    };
                    if is_detach(&event) {
                        let _ = tx.send(Ev::Detach);
                        return;
                    }
                    let up = match event {
                        Event::Resize(..) => {
                            let _ = tx.send(Ev::Resized);
                            Up::Resize(host.grid_size())
                        }
                        other => Up::Input(other),
                    };
                    if let Some(w) = writer.lock_recover().as_mut() {
                        let _ = wire::write(w, &up);
                    }
                })
                .expect("the input thread");
        }
        Err(e) => return Outcome::Ended(e.to_string()),
    }
    let mut screen = Screen::default();
    let mut generation = 0u64;
    let mut showing: Option<(u64, u64)> = None;
    let mut seen = (0u32, 0u32, 0i32);
    let mut title = String::new();
    loop {
        // A live link: read it on its own thread, write from this one and
        // the input thread.
        generation += 1;
        {
            let (tx, mut reader, g) = (tx.clone(), std::mem::replace(&mut link.reader, Box::new(io::empty())), generation);
            std::thread::Builder::new()
                .name("remote-read".into())
                .spawn(move || loop {
                    match wire::read::<Down>(&mut reader) {
                        Ok(Some(down)) => {
                            if tx.send(Ev::Down(g, down)).is_err() {
                                return;
                            }
                        }
                        Ok(None) => {
                            let _ = tx.send(Ev::Lost(g, "the link closed".into()));
                            return;
                        }
                        Err(e) => {
                            let _ = tx.send(Ev::Lost(g, e.to_string()));
                            return;
                        }
                    }
                })
                .expect("the reader thread");
        }
        {
            let mut w = writer.lock_recover();
            let mut fresh: Box<dyn Write + Send> = std::mem::replace(&mut link.writer, Box::new(io::sink()));
            let _ = wire::write(&mut fresh, &Up::Resize(host.grid_size()));
            *w = Some(fresh);
        }
        let lost = loop {
            let ev = match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(ev) => Some(ev),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => break "the window closed".to_string(),
            };
            match ev {
                Some(Ev::Down(g, down)) if g == generation => {
                    if let Some(outcome) = apply(&host, &mut backend, &mut screen, &mut showing, &mut title, &label, down) {
                        link.close();
                        return outcome;
                    }
                }
                Some(Ev::Lost(g, why)) if g == generation => break why,
                Some(Ev::Down(..) | Ev::Lost(..)) => {}
                Some(Ev::Resized) => redraw(&mut backend, &screen),
                Some(Ev::Detach) => {
                    link.close();
                    *writer.lock_recover() = None;
                    return Outcome::Detached;
                }
                None => {}
            }
            report_showcase(&host, &writer, &mut showing, &mut seen);
        };
        if closed.load(Ordering::Acquire) {
            return Outcome::Detached;
        }
        // The link broke: say so over the last picture of the session and
        // attach again until it works, the operator detaches, or it is
        // clear it never will.
        *writer.lock_recover() = None;
        link.close();
        eprintln!("REMOTE: the link to {label} broke ({lost}); reconnecting");
        let mut attempt = 0u32;
        link = loop {
            attempt += 1;
            banner(&mut backend, &screen, &format!(" reconnecting to {label}… ({lost}) "));
            let wait = Duration::from_millis((250 * u64::from(attempt.min(8))).min(2000));
            let until = Instant::now() + wait;
            while Instant::now() < until {
                match rx.recv_timeout(until.saturating_duration_since(Instant::now())) {
                    Ok(Ev::Detach) | Err(RecvTimeoutError::Disconnected) => return Outcome::Detached,
                    Ok(Ev::Resized) => banner(&mut backend, &screen, &format!(" reconnecting to {label}… ")),
                    _ => {}
                }
            }
            match connect(host.grid_size(), screen.held()) {
                Ok(link) => break link,
                Err(Fail::Fatal(why)) => {
                    banner(&mut backend, &screen, &format!(" {why} "));
                    std::thread::sleep(Duration::from_secs(3));
                    return Outcome::Ended(why);
                }
                Err(Fail::Again(why)) => {
                    eprintln!("REMOTE: reconnect {attempt}: {why}");
                }
            }
        };
        eprintln!("REMOTE: reattached to {label}");
    }
}

/// Act on one message. `Some` ends the attach.
#[allow(clippy::too_many_arguments)]
fn apply(
    host: &Host,
    backend: &mut crate::GlassBackend,
    screen: &mut Screen,
    showing: &mut Option<(u64, u64)>,
    title: &mut String,
    label: &str,
    down: Down,
) -> Option<Outcome> {
    match &down {
        Down::Grid(_) => {
            screen.take(&down);
            if screen.full {
                screen.full = false;
                screen.changed.clear();
                redraw(backend, screen);
            } else {
                let cols = usize::from(screen.cols.max(1));
                let changed = std::mem::take(&mut screen.changed);
                let _ = backend.draw(changed.iter().filter_map(|&i| {
                    let cell = screen.cells.get(i)?;
                    Some(((i % cols) as u16, (i / cols) as u16, cell))
                }));
            }
            return None;
        }
        Down::Picture { id, .. } | Down::PictureDelta { id, .. } => {
            screen.take(&down);
            if let Some(shown) = screen.picture(*id) {
                host.upload_picture(crate::graphics::Picture {
                    id: shown.id,
                    width: shown.width,
                    height: shown.height,
                    rgba: (*shown.rgba).clone(),
                    cols: shown.cols,
                    rows: shown.rows,
                });
            }
            return None;
        }
        _ => {}
    }
    if screen.take(&down) {
        return None;
    }
    match down {
        Down::Mesh { id, hash } => match screen.mesh(&hash) {
            Some(mesh) => host.upload_mesh(id, (*mesh).clone()),
            None => eprintln!("REMOTE: mesh {id} named a blob this window does not hold"),
        },
        Down::Layers(layers) => host.set_layers(layers),
        Down::Stage { stage: Some(mut stage), meshes, .. } => {
            if let Some(world) = stage.props.as_mut().and_then(|p| p.world.as_mut()) {
                world.meshes = meshes.iter().filter_map(|(mid, hash)| Some((*mid, screen.mesh(hash)?))).collect();
            }
            host.set_stage(stage);
        }
        Down::Stage { id, stage: None, .. } => host.remove_stage(id),
        Down::Scene { id, scene: Some((scene, size)) } => host.set_scene(id, scene, size),
        Down::Scene { id, scene: None } => host.remove_scene(id),
        Down::ShowcaseOpen { serial, showing: show, marks, model } => {
            host.showcase_marks(marks);
            if let Some(nodes) = model {
                host.showcase_model(nodes);
            }
            let local = host.showcase(show);
            *showing = Some((serial, local));
        }
        Down::ShowcaseClose => {
            host.close_showcase();
            *showing = None;
        }
        Down::ShowcaseProgress { at, paused } => host.showcase_progress(at, paused),
        Down::Title(t) => {
            *title = t;
            host.set_title(&format!("{title} — {label}"));
        }
        Down::Attention => host.request_attention(),
        Down::Clipboard(text) => {
            host.copy_to_clipboard(&text);
        }
        Down::GameKeyboard(on) => host.set_game_keyboard(on),
        Down::Present => present(host, backend, screen),
        Down::Ended(why) => return Some(Outcome::Ended(why)),
        Down::Hello { .. } | Down::Status(_) | Down::Presence(_) => {}
        Down::Size(_) | Down::Grid(_) | Down::Blob { .. } | Down::Picture { .. } | Down::PictureDelta { .. } => {}
    }
    None
}

fn present(host: &Host, backend: &mut crate::GlassBackend, screen: &Screen) {
    host.set_card(screen.card.as_ref().and_then(card));
    match screen.cursor {
        Some((x, y)) => {
            let _ = backend.set_cursor_position((x, y));
            let _ = backend.show_cursor();
        }
        None => {
            let _ = backend.hide_cursor();
        }
    }
    let _ = backend.flush();
}

/// Draw the whole grid again (a new grid, or the window's own size changed).
fn redraw(backend: &mut crate::GlassBackend, screen: &Screen) {
    let _ = backend.clear();
    let cols = usize::from(screen.cols.max(1));
    let _ = backend.draw(screen.cells.iter().enumerate().map(|(i, cell)| ((i % cols) as u16, (i / cols) as u16, cell)));
}

/// A line across the top of the last picture of the session, presented at
/// once: the link's state while there is no session to draw.
fn banner(backend: &mut crate::GlassBackend, screen: &Screen, text: &str) {
    redraw(backend, screen);
    let cols = screen.cols.max(text.chars().count() as u16);
    let mut cells = Vec::with_capacity(usize::from(cols));
    let chars: Vec<char> = text.chars().collect();
    for x in 0..cols {
        let mut cell = Cell::default();
        cell.set_symbol(&chars.get(usize::from(x)).map_or(" ".to_string(), |c| c.to_string()));
        cell.fg = Color::Rgb(230, 220, 200);
        cell.bg = Color::Rgb(70, 40, 20);
        cell.modifier = Modifier::BOLD;
        cells.push(cell);
    }
    let _ = backend.draw(cells.iter().enumerate().map(|(x, c)| (x as u16, 0u16, c)));
    let _ = backend.hide_cursor();
    let _ = backend.flush();
}

/// What the operator did to the showcase panel, up to the session.
fn report_showcase(
    host: &Host,
    writer: &Arc<Mutex<Option<Box<dyn Write + Send>>>>,
    showing: &mut Option<(u64, u64)>,
    seen: &mut (u32, u32, i32),
) {
    let Some((serial, local)) = *showing else {
        return;
    };
    let (clicks, copies, steps) = (host.showcase_clicks(), host.showcase_copies(), host.showcase_steps());
    let feedback = Feedback {
        serial,
        closed: !host.showcase_live(local),
        clicks: clicks.wrapping_sub(seen.0),
        copies: copies.wrapping_sub(seen.1),
        steps: steps.wrapping_sub(seen.2),
        seek: host.showcase_seek(),
    };
    *seen = (clicks, copies, steps);
    if feedback.closed {
        *showing = None;
    }
    if feedback != (Feedback { serial, ..Feedback::default() }) {
        if let Some(w) = writer.lock_recover().as_mut() {
            let _ = wire::write(w, &Up::Showcase(feedback));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_detach_key_is_ctrl_backslash() {
        let key = |code, modifiers| Event::Key(KeyEvent::new(code, modifiers));
        assert!(is_detach(&key(KeyCode::Char('\\'), KeyModifiers::CONTROL)));
        assert!(is_detach(&key(KeyCode::Char('4'), KeyModifiers::CONTROL)));
        assert!(!is_detach(&key(KeyCode::Char('\\'), KeyModifiers::NONE)));
        assert!(!is_detach(&key(KeyCode::Char('d'), KeyModifiers::CONTROL)));
    }

    #[test]
    fn a_stream_picture_lets_its_blob_go() {
        let mut screen = Screen::default();
        let raw = vec![9u8; 16];
        let hash = wire::hash(&raw);
        screen.take(&Down::Blob { hash, deflated: false, bytes: raw.clone() });
        screen.take(&Down::Picture { id: 1, width: 2, height: 2, cols: 0, rows: 0, hash, keep: false });
        assert_eq!(screen.picture(1).map(|p| p.rgba.len()), Some(16));
        assert!(screen.held().is_empty(), "a video frame is not held");
    }

    #[test]
    fn a_card_comes_back_from_the_wire() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 6, 6));
        buffer.content[1] = Cell::new("x");
        let mut original = crate::card::Card::new(Rect::new(3, 4, 6, 6), buffer).unwrap();
        original.title = "Setup".into();
        original.serial = 4;
        original.cursor = Some(5);
        original.knight = crate::card::Knight::Guard;
        let wire = WireCard {
            area: [3, 4, 6, 6],
            cells: original.buffer.content.iter().map(super::super::wire::WireCell::of).collect(),
            title: original.title.clone(),
            serial: 4,
            cursor: Some(5),
            field: None,
            knight: crate::card::Knight::Guard,
            busy: false,
            entrance: crate::card::Entrance::Kick,
        };
        let back = card(&wire).unwrap();
        assert_eq!(back.buffer, original.buffer);
        assert_eq!((back.area, back.title.as_str(), back.serial, back.cursor, back.knight), (original.area, "Setup", 4, Some(5), crate::card::Knight::Guard));
    }
}
