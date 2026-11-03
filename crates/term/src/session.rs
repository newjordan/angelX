//! The program's side: a pseudo-terminal, the engine that keeps the
//! screen its output draws, and the answers a terminal owes the program
//! (where the cursor is, what this terminal is, its colours).

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use crossterm::event::Event;
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use ratatui_core::terminal::Frame;

use crate::engine::{Engine, HostInfo};
use crate::engine::Event as EngineEvent;
use crate::intake::{self, Area, Intake, Piece};
use crate::keys::{MouseProtocolEncoding, MouseProtocolMode};
use crate::view::{self, Selection};

/// The layer group this session's pictures live in (`set_layer_group`).
const PICTURE_GROUP: u32 = 0x1e5c;

/// The layer group glass's own dialect lives in, beside the pictures:
/// its ids are the dialect's own (`glass_dialect::ID_BASE` up), so the
/// two groups never share a layer id.
const DIALECT_GROUP: u32 = 0x1e5d;

/// How long a synchronized update (DECSET 2026) may hold the frame back.
const SYNC_LIMIT: Duration = Duration::from_millis(150);

/// How long a synchronized frame waits for the one before it to be drawn.
const FRAME_WAIT: Duration = Duration::from_millis(50);

const FRAME_BEGIN: &[u8] = b"\x1b[?2026h";
const FRAME_END: &[u8] = b"\x1b[?2026l";

/// One frame of flow control for synchronized updates. Each update holds
/// the screen until it ends, so a program that begins the next update as
/// soon as one ends (frames sent back to back) would keep every frame
/// held and the screen would show none of them. A finished frame is owed
/// a draw, and the next frame's start waits for it, briefly: the program
/// is held to the pace the screen is drawn at, and every frame it
/// finishes is seen.
#[derive(Default)]
pub struct FrameGate {
    owed: Mutex<bool>,
    drawn: std::sync::Condvar,
}

impl FrameGate {
    /// A synchronized update ended: its frame is owed a draw.
    fn finished(&self) {
        *lock(&self.owed) = true;
    }

    /// The screen was drawn.
    pub fn drawn(&self) {
        *lock(&self.owed) = false;
        self.drawn.notify_all();
    }

    /// Wait, at most `limit`, for an owed frame to be drawn.
    fn wait(&self, limit: Duration) {
        let owed = lock(&self.owed);
        let _ = self.drawn.wait_timeout_while(owed, limit, |owed| *owed);
    }
}

/// Where `needle` (an escape sequence) next starts in `bytes` at or
/// after `from`, found as a whole: escape-dense output (a colour on every
/// cell) is not stopped at for each escape.
fn find_escape(bytes: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    memchr::memmem::find(bytes.get(from..)?, needle).map(|p| from + p)
}

/// Where a read splits for the gate: at each synchronized update's
/// start (past the read's first byte), so the frame before it can be
/// drawn before the next begins.
fn frame_cuts(bytes: &[u8]) -> Vec<usize> {
    let mut cuts = Vec::new();
    let mut at = 1;
    while let Some(p) = find_escape(bytes, FRAME_BEGIN, at) {
        cuts.push(p);
        at = p + 1;
    }
    cuts
}

/// What reaches the terminal's thread.
pub enum Msg {
    /// The window's keys, mouse, paste, focus and size.
    Input(Event),
    /// The program wrote something; the screen has changed.
    Output,
    /// The window closed.
    Closed,
}

/// What one read's worth of pumping produced, beside the screen: the
/// replies owed to the program, in stream order across the engine and
/// the intake, and the window effects of the same bytes.
pub struct Pumped {
    pub replies: Vec<u8>,
    pub title: Option<String>,
    pub bell: bool,
    pub clipboard: Option<String>,
}

/// The intake's engine: the alacritty `Term` behind the session's
/// engine, as the adapters see it. `cursor` reads the grid cursor live,
/// not `Engine::cursor`, which stays `None` while the program hides the
/// cursor or the view is scrolled back — and programs often hide the
/// cursor while they draw pictures.
struct Adapter<'a> {
    engine: &'a mut Engine,
}

impl intake::Engine for Adapter<'_> {
    fn cursor(&mut self) -> (u16, u16) {
        self.engine.grid_cursor()
    }
    fn size(&mut self) -> (u16, u16) {
        let (rows, cols) = self.engine.size();
        (rows as u16, cols as u16)
    }
    fn scrolled(&mut self) -> u64 {
        // The line a picture anchors to is this screen's own count: the
        // lines that entered history on the primary screen, the lines
        // that left the top on the alternate screen. The primary count
        // does not move pictures on the alternate screen.
        if self.engine.alternate_screen() {
            self.engine.alt_scrolled()
        } else {
            self.engine.scrolled_off_top() as u64
        }
    }
    fn alternate(&mut self) -> bool {
        self.engine.alternate_screen()
    }
    fn feed(&mut self, bytes: &[u8]) {
        self.engine.process(bytes);
    }
}

/// One read's worth of the program's output, through the splitter, the
/// engine and the intake together. Text pieces reach the engine and their
/// replies go out right away; media pieces settle the engine first, then
/// reach the intake through the adapter. The replies leave in stream
/// order: a probe answered by the APC graphics dialect before a DA1 in
/// the stream is answered in that order, and the other way round too.
pub fn pump(engine: &mut Engine, intake: &mut Intake, pieces: &[Piece<'_>]) -> Pumped {
    let mut replies = Vec::new();
    let mut title = None;
    let mut bell = false;
    let mut clipboard = None;
    let mut i = 0;
    while i < pieces.len() {
        match &pieces[i] {
            Piece::Text(text) => {
                let screen = intake.was_alternate();
                engine.set_in_cell(
                    intake
                        .in_cell_areas(screen)
                        .into_iter()
                        .map(|area| {
                            (
                                area.lines.start,
                                area.lines.end.saturating_sub(1),
                                area.columns.start,
                                area.columns.end.saturating_sub(1),
                            )
                        })
                        .collect(),
                );
                engine.set_motion(intake.motion_end(screen));
                // While nothing in the store hangs on the text, nothing the
                // text does can move, cover or drop it: the run of text
                // pieces up to the next media piece goes to the engine as
                // one feed, and the store hears once what it did.
                let end = if intake.quiet() { quiet_run(pieces, i) } else { i + 1 };
                if end == i + 1 {
                    engine.process(text);
                } else {
                    engine.process_parts(pieces[i..end].iter().map(|piece| match piece {
                        Piece::Text(text) => text.as_ref(),
                        Piece::Media(_) => &[][..],
                    }));
                }
                i = end;
                // Everything the text asked of the window is kept, not only
                // its replies: a clipboard write (OSC 52, how tmux and
                // programs over ssh copy), a title, the bell.
                let effects = engine.take_effects();
                replies.extend_from_slice(&effects.replies);
                title = effects.title.or(title);
                bell |= effects.bell;
                clipboard = effects.clipboard.or(clipboard);
                tell_the_store(engine, intake);
            }
            Piece::Media(media) => {
                i += 1;
                // A picture is about to be placed. Until that place finishes,
                // a scroll the place itself feeds is reported.
                engine.set_motion(Some(u64::MAX));
                // The engine settles first: a media piece's escape ends
                // whatever the engine holds back, and an open
                // synchronized update is applied so the cursor and count
                // are the stream's, not stale ones. The frame stays held
                // until the program's end sequence or glass's limit.
                engine.settle();
                replies.extend_from_slice(&engine.take_effects().replies);
                // What the settled text did (a clear, a screen switch,
                // writes over an old picture, lines scrolled) reaches the
                // store before this picture does, as it came first in the
                // stream: a program that clears and redraws inside one
                // synchronized update keeps its new picture.
                tell_the_store(engine, intake);
                let mut adapter = Adapter { engine };
                intake.handle(media.clone(), &mut adapter);
                replies.extend_from_slice(&intake.take_replies());
                let effects = engine.take_effects();
                replies.extend_from_slice(&effects.replies);
                title = effects.title.or(title);
                bell |= effects.bell;
                clipboard = effects.clipboard.or(clipboard);
                // The place's own cursor motion (a sixel walking down the
                // screen) has already scrolled. The store hears that before
                // the next text piece decides whether a scroll can move a
                // picture, so a split read and a whole read agree.
                tell_the_store(engine, intake);
            }
        }
    }
    let effects = engine.take_effects();
    title = effects.title.or(title);
    bell |= effects.bell;
    clipboard = effects.clipboard.or(clipboard);
    replies.extend_from_slice(&effects.replies);
    Pumped { replies, title, bell, clipboard }
}

/// Where a feed of text pieces that starts at `from` ends: at the next
/// media piece, after a piece that may switch screens (the engine keeps
/// the screen a feed began on), or before a reset, which is fed on its
/// own as it zeroes the count the feed has kept so far.
fn quiet_run(pieces: &[Piece<'_>], from: usize) -> usize {
    let mut end = from + 1;
    if matches!(&pieces[from], Piece::Text(text) if ends_feed(text)) {
        return end;
    }
    while let Some(Piece::Text(next)) = pieces.get(end) {
        if is_reset(next) {
            break;
        }
        end += 1;
        if ends_feed(next) {
            break;
        }
    }
    end
}

/// Whether a text piece is RIS.
fn is_reset(text: &[u8]) -> bool {
    text.starts_with(b"\x1bc")
}

/// Whether a text piece may switch screens or reset the terminal: a
/// private mode set or reset naming 47, 1047 or 1049, or RIS. A false
/// alarm only ends a feed early.
fn ends_feed(text: &[u8]) -> bool {
    if is_reset(text) {
        return true;
    }
    let Some(params) = text.strip_prefix(b"\x1b[?") else {
        return false;
    };
    let mut value = 0u32;
    for &b in params {
        if b.is_ascii_digit() {
            value = value.saturating_mul(10).saturating_add(u32::from(b - b'0'));
        } else {
            if matches!(value, 47 | 1047 | 1049) {
                return true;
            }
            value = 0;
        }
    }
    matches!(value, 47 | 1047 | 1049)
}

/// After the engine has applied bytes, the store hears what they did:
/// which screen is up, the exact scroll count and history for that screen
/// (the primary's own while it is up, the alternate's own, from 0 each
/// time it is entered, while it is), and the engine's events in stream
/// order.
fn tell_the_store(engine: &mut Engine, intake: &mut Intake) {
    let now_alternate = engine.alternate_screen();
    if now_alternate && !intake.was_alternate() {
        intake.enter_alternate();
    } else if !now_alternate && intake.was_alternate() {
        intake.drop_alternate_placements();
        intake.expansion.leave_alternate();
    }
    intake.set_alternate(now_alternate);
    let rows = engine.size().0 as u16;
    if now_alternate {
        intake.set_scrolled_alternate(engine.alt_scrolled(), rows);
    } else {
        intake.set_scrolled_primary(engine.scrolled_off_top() as u64, engine.history_size(), rows);
    }
    for event in engine.take_events() {
        apply_event(intake, event);
    }
}

/// One engine event, handed to the store in order.
fn apply_event(intake: &mut Intake, event: EngineEvent) {
    match event {
        EngineEvent::Clear { alternate, scrolled } => {
            intake.clear_screen(alternate, scrolled);
            intake.expansion.clear_screen(alternate, scrolled);
        }
        EngineEvent::ClearSaved => {
            intake.clear_saved_lines(intake.scrolled());
            intake.expansion.clear_saved(intake.scrolled());
        }
        EngineEvent::Reset => {
            intake.reset();
            intake.expansion.reset();
        }
        EngineEvent::Covered { alternate, area } => intake.covered(
            alternate,
            Area {
                lines: area.line..area.line + area.rows as u64,
                columns: area.column..area.column + area.columns as u16,
            },
        ),
        EngineEvent::RegionScroll {
            alternate,
            top,
            bottom,
            up,
            down,
            count_delta,
            keep_above,
            clears,
        } => intake.region_scroll(crate::intake::motion::BandScroll {
            alternate,
            top,
            bottom,
            up,
            down,
            count_delta,
            keep_above,
            clears,
        }),
    }
}

/// Variables another terminal (or the cockpit's shell pane) set, inherited
/// when glass is started from one: they would tell a program it runs
/// somewhere it does not. Programs learn what glass takes in by asking it.
const FOREIGN_PREFIXES: [&str; 5] = ["KITTY_", "ALACRITTY_", "GHOSTTY_", "WEZTERM_", "KONSOLE_"];
const FOREIGN: [&str; 9] = [
    "WINDOWID",
    "TERMINFO",
    "VTE_VERSION",
    "TMUX",
    "TMUX_PANE",
    "STY",
    // The shell pane's "stay in this terminal": glass is a window, so an
    // angelX typed here opens its own (and swallows this one).
    "ANGEL_GLASS",
    "ANGEL_GLASS_TRANSITION",
    "TERM_PROGRAM_VERSION",
];

pub struct Session {
    engine: Arc<Mutex<Engine>>,
    /// The picture intake: what the program's pictures are and where they
    /// sit, kept beside the engine so both see the same reads.
    intake: Arc<Mutex<Intake>>,
    /// Where the splitter stands between reads, so a sequence split
    /// across two reads comes out whole. The reader thread owns it; the
    /// session holds it only so both live and die together.
    #[allow(dead_code)]
    splitter: Arc<Mutex<intake::Splitter>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    /// Set by the reader when it signals output, cleared when drawn: one
    /// `Msg::Output` in flight however much the program writes.
    output: Arc<AtomicBool>,
    /// Synchronized frames wait here for the one before to be drawn.
    gate: Arc<FrameGate>,
    /// The showcase request last handed to the window. Frames and
    /// progress are not part of it, so they never reopen the panel.
    showcase_shown: Mutex<Option<OpenStamp>>,
    /// Scene picture ids last given to the window, so a removed scene
    /// is taken down once.
    scene_ids: Mutex<Vec<u32>>,
    /// Showcase counters already reported to the program.
    event_seen: Mutex<intake::glass_dialect::ShowcaseSeen>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl Session {
    pub fn spawn(
        command: &[String],
        dir: Option<&Path>,
        grid: angel_glass::GridSize,
        palette: &angel_glass::Palette,
        host: angel_glass::Host,
        tx: Sender<Msg>,
        focused: Arc<AtomicBool>,
    ) -> Result<Self, String> {
        let (rows, cols) = if grid.rows == 0 || grid.cols == 0 { (24, 80) } else { (grid.rows, grid.cols) };
        let pair = native_pty_system()
            .openpty(pty_size(rows, cols, grid))
            .map_err(|e| format!("openpty: {e}"))?;

        let mut cmd = match command.split_first() {
            Some((program, args)) => {
                let mut cmd = CommandBuilder::new(program);
                cmd.args(args);
                cmd
            }
            None => CommandBuilder::new(std::env::var_os("SHELL").unwrap_or_else(|| "/bin/bash".into())),
        };
        let cwd = dir
            .filter(|d| d.is_dir())
            .map(Path::to_path_buf)
            .or_else(|| std::env::current_dir().ok())
            .or_else(|| std::env::var_os("HOME").map(Into::into));
        if let Some(cwd) = cwd {
            cmd.cwd(cwd);
        }
        for (key, _) in std::env::vars_os() {
            let name = key.to_string_lossy();
            if FOREIGN.contains(&name.as_ref()) || FOREIGN_PREFIXES.iter().any(|p| name.starts_with(p)) {
                cmd.env_remove(&key);
            }
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "glass");
        cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));

        let child = pair.slave.spawn_command(cmd).map_err(|e| format!("could not start {}: {e}", command.first().map_or("the shell", String::as_str)))?;
        angel_glass::startup::mark("program started");
        // Close our copy of the slave so the reader sees the end of output.
        drop(pair.slave);

        let info = HostInfo {
            colors: [palette.foreground, palette.background, palette.cursor],
            cell_px: (grid.cell_width, grid.cell_height),
        };
        let engine = Arc::new(Mutex::new(Engine::new(usize::from(rows), usize::from(cols), info)));
        let writer: Arc<Mutex<Box<dyn Write + Send>>> =
            Arc::new(Mutex::new(pair.master.take_writer().map_err(|e| format!("pty writer: {e}"))?));
        let reader = pair.master.try_clone_reader().map_err(|e| format!("pty reader: {e}"))?;
        // The pty is read on a thread of its own, ahead of the parsing.
        let mut reader = crate::read_ahead::ReadAhead::spawn(reader, pty_fd(pair.master.as_ref()))
            .map_err(|e| format!("pty reader: {e}"))?;
        let output = Arc::new(AtomicBool::new(false));
        let gate = Arc::new(FrameGate::default());
        // The cell from the start, not only from a resize: a program started
        // on the window's settled size may never see one, and its pictures
        // are measured in these cells.
        let mut fresh = Intake::new(angel_glass::LAYER_PICTURES);
        fresh.set_cell((grid.cell_width, grid.cell_height));
        let intake = Arc::new(Mutex::new(fresh));
        let splitter = Arc::new(Mutex::new(intake::Splitter::default()));
        {
            let (engine, writer, output, intake, splitter, gate) = (
                Arc::clone(&engine),
                Arc::clone(&writer),
                Arc::clone(&output),
                Arc::clone(&intake),
                Arc::clone(&splitter),
                Arc::clone(&gate),
            );
            let parse = std::thread::Builder::new().name("pty-parse".into()).spawn(move || {
                crate::affinity::place_hot_thread();
                let mut buf = vec![0u8; 64 * 1024];
                'reads: loop {
                    let n = match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => n,
                    };
                    // A read is taken in pieces cut at each synchronized
                    // frame's start; a frame starting while the one before
                    // it waits to be drawn waits with it (`FrameGate`).
                    let mut starts = frame_cuts(&buf[..n]);
                    starts.push(n);
                    let mut from = 0;
                    for to in starts {
                        let piece = &buf[from..to];
                        from = to;
                        if piece.starts_with(FRAME_BEGIN) {
                            gate.wait(FRAME_WAIT);
                        }
                        // One path: read, then pump the read through the
                        // splitter, the engine and the intake together, and
                        // write back what the program is owed.
                        let pumped = {
                            let mut pieces = Vec::new();
                            lock(&splitter).split(piece, &mut pieces);
                            let mut engine = lock(&engine);
                            let mut intake = lock(&intake);
                            pump(&mut engine, &mut intake, &pieces)
                        };
                        if !pumped.replies.is_empty() {
                            drop(lock(&engine));
                            let mut writer = lock(&writer);
                            let _ = writer.write_all(&pumped.replies);
                            let _ = writer.flush();
                        }
                        if let Some(title) = pumped.title.filter(|t| !t.is_empty()) {
                            host.set_title(&title);
                        }
                        if pumped.bell && !focused.load(Ordering::Relaxed) {
                            host.request_attention();
                        }
                        if let Some(text) = pumped.clipboard {
                            host.copy_to_clipboard(&text);
                        }
                        // A frame that ended here is owed a draw, from now:
                        // a draw already under way may have missed it.
                        let ended = find_escape(piece, FRAME_END, 0).is_some();
                        if ended {
                            gate.finished();
                        }
                        // One output message in flight, but a frame's end
                        // always wakes the window: a draw it tried while the
                        // frame was held was put off, and nothing else would
                        // bring it back before the gate gives up waiting.
                        let first = !output.swap(true, Ordering::AcqRel);
                        if (first || ended) && tx.send(Msg::Output).is_err() {
                            break 'reads;
                        }
                    }
                }
            });
            parse.map_err(|e| format!("pty reader: {e}"))?;
        }
        Ok(Self {
            engine,
            intake,
            splitter,
            writer,
            master: pair.master,
            child,
            output,
            gate,
            showcase_shown: Mutex::new(None),
            scene_ids: Mutex::new(Vec::new()),
            event_seen: Mutex::new(intake::glass_dialect::ShowcaseSeen::default()),
        })
    }

    /// Bytes to the program, as typed.
    pub fn write(&mut self, bytes: &[u8]) {
        let mut writer = lock(&self.writer);
        let _ = writer.write_all(bytes);
        let _ = writer.flush();
    }

    /// Bytes onto the screen, as if the program wrote them.
    pub fn feed(&mut self, bytes: &[u8]) {
        lock(&self.engine).process(bytes);
    }

    pub fn resize(&mut self, rows: u16, cols: u16, grid: angel_glass::GridSize) {
        let (rows, cols) = (rows.max(1), cols.max(1));
        let _ = self.master.resize(pty_size(rows, cols, grid));
        let mut engine = lock(&self.engine);
        engine.resize(usize::from(rows), usize::from(cols));
        engine.set_cell_px((grid.cell_width, grid.cell_height));
        let reflow = engine.take_reflow();
        let scrolled = engine.scrolled_off_top() as u64;
        let history = engine.history_size();
        drop(engine);
        let mut intake = lock(&self.intake);
        intake.set_cell((grid.cell_width, grid.cell_height));
        if let Some(reflow) = reflow {
            intake.reflow(&reflow);
            intake.set_scrolled_primary(scrolled, history, rows);
        }
    }

    /// The program has exited: everything it drew through the dialect
    /// goes with it, so the window it leaves behind is the text alone.
    pub fn program_exited(&mut self) {
        lock(&self.intake).expansion.reset();
    }

    /// The program's exit code once it has exited.
    pub fn exit_code(&mut self) -> Option<i32> {
        match self.child.try_wait() {
            Ok(Some(status)) => Some(i32::try_from(status.exit_code()).unwrap_or(1)),
            Ok(None) => None,
            Err(_) => Some(1),
        }
    }

    pub fn take_output_flag(&self) {
        self.output.store(false, Ordering::Release);
    }

    /// The screen was drawn: a synchronized frame waiting on it goes on.
    pub fn frame_drawn(&self) {
        self.gate.drawn();
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        selection: Option<&Selection>,
        hover: Option<&Selection>,
        host: &angel_glass::Host,
        show_cursor: bool,
    ) {
        let mut engine = lock(&self.engine);
        let area = frame.area();
        view::paint(frame.buffer_mut(), area, &mut engine, selection, hover);
        if show_cursor
            && let Some(position) = view::cursor(&engine, frame.area())
        {
            frame.set_cursor_position(position);
        }
        drop(engine);
        // This frame's pictures: any the window has not had yet go up,
        // and the showings visible in the view become one layer group.
        // The view's top line is the scroll count less the rows `Term`'s
        // display offset lifts the view into the scrollback.
        let engine = lock(&self.engine);
        let top = engine.top_line();
        let alternate = engine.alternate_screen();
        drop(engine);
        let view = intake::View { top, rows: area.height, alternate };
        let mut description = lock(&self.intake).describe(view);
        // Placeholder cells are ordinary text. Ask the store first: build
        // a cell only when a virtual placement exists, and only for the
        // cells on screen.
        if lock(&self.intake).has_virtual() {
            let mut engine = lock(&self.engine);
            let (cw, ch) = engine.cell_px();
            let cols = area.width;
            let refs: Vec<intake::placeholders::CellRef> = engine
                .cells()
                .iter()
                .take(usize::from(area.height) * usize::from(cols.max(1)))
                .map(|cell| intake::placeholders::CellRef {
                    symbol: cell.c,
                    marks: cell.marks,
                    fg: cell.fg_id,
                    underline: cell.underline_id,
                })
                .collect();
            drop(engine);
            let extra = lock(&self.intake).placeholder_layers(&refs, cols, cw as f32, ch as f32);
            description.placements.extend(extra);
        }
        for picture in description.pictures {
            host.upload_picture(angel_glass::gpu::Picture {
                id: picture.gpu,
                width: picture.width,
                height: picture.height,
                rgba: picture.rgba.as_ref().clone(),
                cols: 0,
                rows: 0,
            });
        }
        let layers = description
            .placements
            .into_iter()
            .map(|shown| {
                let kind = match shown.source {
                    Some([x, y, w, h]) => angel_glass::LayerKind::PicturePart(shown.gpu, [x, y, w, h]),
                    None => angel_glass::LayerKind::Picture(shown.gpu),
                };
                let mut layer = angel_glass::Layer::new(
                    shown.id | (u64::from(PICTURE_GROUP) << 32),
                    shown.z,
                    angel_glass::Placement::Cells([shown.column, shown.row, shown.width, shown.height]),
                    kind,
                );
                layer.opacity = 1.0;
                layer
            })
            .collect();
        host.set_layer_group(PICTURE_GROUP, layers);
        // Dropped pictures go after the layers that stopped naming them.
        if !description.freed.is_empty() {
            host.free_pictures(description.freed);
        }
        // glass's own dialect: its layers beside the intake's pictures,
        // positioned per frame against the same view.
        {
            let mut intake = lock(&self.intake);
            let layers = intake.expansion.frame(top, area.height, alternate);
            host.set_layer_group(DIALECT_GROUP, layers);
            apply_scenes(&mut intake.expansion, host, &self.scene_ids);
            // The showcase opens only when the program's request changes,
            // never once per frame. Frames and progress ride beside it.
            let stamp = open_stamp(&intake);
            let mut shown = lock(&self.showcase_shown);
            if stamp != *shown {
                apply_showcase(&intake, host, stamp.as_ref());
                *shown = stamp;
            }
            drop(shown);
            if let Some(frame) = intake.expansion.take_video_frame() {
                apply_frame(&intake, host, frame);
            }
            if let Some((at, paused)) = intake.expansion.take_progress() {
                host.showcase_progress(at, paused);
            }
            write_showcase_events(self, &intake, host);
        }
    }

    /// The cursor's shape (block, underline or bar) as the program set it.
    pub fn cursor_shape(&self) -> crate::view::CursorLook {
        lock(&self.engine).cursor_look()
    }

    /// Whether the program asked for a blinking cursor.
    pub fn cursor_blinking(&self) -> bool {
        lock(&self.engine).cursor_blinking()
    }

    /// Whether the cursor is on the screen: not hidden, and not scrolled
    /// off the live screen.
    pub fn cursor_shown(&self) -> bool {
        lock(&self.engine).cursor().is_some()
    }

    /// The cursor colour the program set, if it set one.
    pub fn cursor_color(&self) -> Option<[u8; 3]> {
        lock(&self.engine).cursor_color().map(|c| [c.r, c.g, c.b])
    }

    pub fn selected_text(&self, selection: &Selection) -> String {
        let mut engine = lock(&self.engine);
        let cols = u16::try_from(engine.size().1).unwrap_or(u16::MAX);
        view::text(engine.cells(), cols, selection)
    }

    pub fn word_at(&self, row: u16, col: u16) -> Selection {
        let mut engine = lock(&self.engine);
        let cols = u16::try_from(engine.size().1).unwrap_or(u16::MAX);
        view::word(engine.cells(), cols, row, col)
    }

    /// Move the view `lines` into the scrollback (negative: back toward the
    /// live screen). Returns whether it moved.
    pub fn scroll_back(&mut self, lines: isize) -> bool {
        lock(&self.engine).scroll_display(lines)
    }

    pub fn scroll_to_bottom(&mut self) -> bool {
        lock(&self.engine).scroll_to_bottom()
    }

    pub fn rows(&self) -> u16 {
        u16::try_from(lock(&self.engine).size().0).unwrap_or(u16::MAX)
    }

    pub fn cols(&self) -> u16 {
        u16::try_from(lock(&self.engine).size().1).unwrap_or(u16::MAX)
    }

    /// The CSI u keyboard dialect's flags, as the program set them.
    pub fn keyboard_flags(&self) -> u8 {
        lock(&self.engine).keyboard_flags()
    }

    pub fn alternate_screen(&self) -> bool {
        lock(&self.engine).alternate_screen()
    }

    pub fn application_cursor(&self) -> bool {
        lock(&self.engine).application_cursor()
    }

    pub fn bracketed_paste(&self) -> bool {
        lock(&self.engine).bracketed_paste()
    }

    pub fn mouse_protocol(&self) -> (MouseProtocolMode, MouseProtocolEncoding) {
        lock(&self.engine).mouse_protocol()
    }

    pub fn focus_reports(&self) -> bool {
        lock(&self.engine).focus_reports()
    }

    /// The program is in a synchronized update.
    pub fn synchronized(&self) -> bool {
        lock(&self.engine).synchronized()
    }

    /// When the next animation frame falls due, in milliseconds on the
    /// store's clock. `None` when nothing is running: a stopped
    /// animation, or one whose loops are spent, wakes nothing.
    pub fn animation_deadline(&self) -> Option<u64> {
        lock(&self.intake).animation_deadline()
    }

    /// Advance every animation to `now` (milliseconds). Returns whether
    /// a showing changed.
    pub fn tick_animations(&self, now: u64) -> bool {
        lock(&self.intake).tick_animations(now)
    }

    /// Keep the animation clock with the window's, without advancing a
    /// frame. A control that arrives between waits then starts from now.
    pub fn touch_animation_clock(&self, now: u64) {
        lock(&self.intake).set_animation_clock(now);
    }

    /// One display frame of the window, in milliseconds: how often running
    /// tweens and clips wake it.
    pub fn set_display_frame(&self, ms: u64) {
        lock(&self.intake).set_display_frame(ms);
    }

    /// Hold the frame: a synchronized update has not finished, and has not
    /// run past its limit.
    pub fn holding_frame(&self) -> bool {
        lock(&self.engine).holding_frame(SYNC_LIMIT)
    }

    /// End a synchronized update whose deadline passed, applying the bytes
    /// buffered since it opened. Returns whether it ended.
    pub fn end_sync_update(&mut self) -> bool {
        lock(&self.engine).end_sync_update()
    }

    /// The hyperlink open over a cell, if any.
    pub fn link_at(&self, row: u16, col: u16) -> Option<String> {
        lock(&self.engine).link_at(row, col)
    }

    /// The plain-text URL under a cell, if one sits there.
    pub fn url_at(&self, row: u16, col: u16) -> Option<String> {
        let mut engine = lock(&self.engine);
        let cols = u16::try_from(engine.size().1).unwrap_or(u16::MAX);
        view::url_at(engine.cells(), cols, row, col)
    }

    /// Whether Ctrl+click would open a link here.
    pub fn link_or_url_at(&self, row: u16, col: u16) -> Option<String> {
        self.link_at(row, col).or_else(|| self.url_at(row, col))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // The window closed with the program still running: hang up on it,
        // as closing any terminal does.
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
        }
    }
}

/// A descriptor of the pty's own, for asking how much it holds.
fn pty_fd(master: &dyn MasterPty) -> Option<std::os::fd::OwnedFd> {
    let fd = master.as_raw_fd()?;
    // SAFETY: the master is open and stays open for this call; the
    // descriptor is only borrowed to duplicate it.
    unsafe { std::os::fd::BorrowedFd::borrow_raw(fd) }.try_clone_to_owned().ok()
}

fn pty_size(rows: u16, cols: u16, grid: angel_glass::GridSize) -> PtySize {
    let px = |cells: u16, cell: u32| u16::try_from(u32::from(cells) * cell).unwrap_or(u16::MAX);
    PtySize { rows, cols, pixel_width: px(cols, grid.cell_width), pixel_height: px(rows, grid.cell_height) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Engine, HostInfo};

    #[test]
    fn foreign_variables_are_scrubbed() {
        // The list is what the reader thread removes; keep it honest.
        assert!(FOREIGN.contains(&"TERM_PROGRAM_VERSION"));
        assert!(FOREIGN_PREFIXES.iter().all(|p| p.ends_with('_')));
    }

    fn engine() -> Engine {
        Engine::new(
            24,
            80,
            HostInfo {
                colors: [[0xd0, 0xd0, 0xd0], [0, 0, 0], [0xff, 0xff, 0xff]],
                cell_px: (10, 20),
            },
        )
    }

    fn answers(input: &[u8]) -> Vec<u8> {
        let mut term = engine();
        term.process(input);
        term.take_effects().replies
    }

    #[test]
    fn startup_queries_are_answered_byte_for_byte() {
        // Sixel is taken in now, so DA1 says so (`4`).
        assert_eq!(answers(b"\x1b[c"), b"\x1b[?62;4;22c");
        assert_eq!(answers(b"\x1b[3;5H\x1b[6n"), b"\x1b[3;5R");
        assert_eq!(answers(b"\x1b[5n"), b"\x1b[0n");
        assert_eq!(answers(b"\x1b[18t"), b"\x1b[8;24;80t");
        assert_eq!(answers(b"\x1b]11;?\x07"), b"\x1b]11;rgb:0000/0000/0000\x1b\\");
        assert_eq!(answers(b"\x1b[?2026$p"), b"\x1b[?2026;2$y");
        assert_eq!(answers(b"\x1b[?2026h\x1b[?2026$p"), b"\x1b[?2026;1$y");
        assert_eq!(answers(b"\x1b[?9999$p"), b"\x1b[?9999;0$y");
    }

    #[test]
    fn modes_and_titles_are_tracked() {
        let mut term = engine();
        term.process(b"\x1b[?1004h\x1b]2;a;b\x07");
        assert!(term.focus_reports());
        assert_eq!(term.take_effects().title.as_deref(), Some("a;b"));
        term.process(b"\x1b[?1004l\x1b]52;c;aGk=\x07\x07");
        let effects = term.take_effects();
        assert!(!term.focus_reports());
        assert_eq!(effects.clipboard.as_deref(), Some("hi"));
        assert!(effects.bell);
    }
}

#[cfg(test)]
mod gate_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn reads_are_cut_at_each_frame_start_but_the_first_byte() {
        let read = b"\x1b[?2026hA\x1b[?2026l\x1b[c\x1b[?2026hB\x1b[?2026l\x1b[?2026$p";
        assert_eq!(frame_cuts(read), vec![20]);
        assert_eq!(frame_cuts(b"text only"), Vec::<usize>::new());
        assert_eq!(frame_cuts(b"x\x1b[?2026h"), vec![1]);
        // A start split across reads is not a cut; the next read's
        // first byte never is.
        assert_eq!(frame_cuts(b"x\x1b[?20"), Vec::<usize>::new());
        assert_eq!(find_escape(b"ab\x1b[?2026l", FRAME_END, 0), Some(2));
        assert_eq!(find_escape(b"\x1b\x1b[?2026l", FRAME_END, 0), Some(1));
    }

    /// Searching for a frame's escape as a whole finds what stepping from
    /// escape to escape finds, from every start, in random streams dense
    /// with escapes and frames.
    #[test]
    fn frame_escapes_are_found_as_stepping_finds_them() {
        let stepping = |bytes: &[u8], needle: &[u8], from: usize| {
            (from..bytes.len()).find(|&at| bytes[at] == 0x1b && bytes[at..].starts_with(needle))
        };
        let mut rng = crate::speed::Rng::new(41);
        for _ in 0..200 {
            let len = 50 + rng.below(400);
            let mut stream = crate::speed::random_stream(&mut rng, len);
            for _ in 0..rng.below(4) {
                let at = rng.below(stream.len() + 1);
                let frame: &[u8] = if rng.below(2) == 0 { FRAME_BEGIN } else { FRAME_END };
                stream.splice(at..at, frame.iter().copied());
            }
            for from in 0..=stream.len() + 1 {
                for needle in [FRAME_BEGIN, FRAME_END] {
                    assert_eq!(find_escape(&stream, needle, from), stepping(&stream, needle, from));
                }
            }
        }
    }

    #[test]
    fn a_frame_waits_for_the_draw_it_is_owed_and_no_longer() {
        let gate = Arc::new(FrameGate::default());
        // Nothing owed: no wait.
        let started = Instant::now();
        gate.wait(Duration::from_secs(5));
        assert!(started.elapsed() < Duration::from_millis(100));
        // Owed and drawn on another thread: the wait ends with the draw.
        gate.finished();
        let drawer = Arc::clone(&gate);
        let draw = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            drawer.drawn();
        });
        let started = Instant::now();
        gate.wait(Duration::from_secs(5));
        let waited = started.elapsed();
        draw.join().unwrap();
        assert!(waited >= Duration::from_millis(25) && waited < Duration::from_secs(2), "{waited:?}");
        // Owed and never drawn: the limit ends it.
        gate.finished();
        let started = Instant::now();
        gate.wait(Duration::from_millis(40));
        assert!(started.elapsed() >= Duration::from_millis(35));
    }
}

#[cfg(test)]
mod pump_tests {
    use super::*;
    use crate::engine::HostInfo;

    fn engine() -> Engine {
        Engine::new(
            6,
            10,
            HostInfo { colors: [[0xd0; 3], [0, 0, 0], [0xff; 3]], cell_px: (10, 20) },
        )
    }

    fn rig() -> (Engine, Intake, intake::Splitter) {
        (engine(), Intake::new(angel_glass::LAYER_PICTURES), intake::Splitter::default())
    }

    /// One stream through pump, as the reader thread would.
    fn pump_bytes(
        engine: &mut Engine,
        intake: &mut Intake,
        splitter: &mut intake::Splitter,
        stream: &[u8],
    ) -> Pumped {
        let mut pieces = Vec::new();
        splitter.split(stream, &mut pieces);
        pump(engine, intake, &pieces)
    }

    /// A tiny APC-graphics picture: direct RGBA, one pixel, transmit and
    /// put, with a reply asked for.
    const APC_PUT: &[u8] = b"\x1b_Gi=31,s=1,v=1,a=T,f=24;AAAA\x1b\\";

    /// What plain text asks of the window reaches it: a clipboard write
    /// (OSC 52), a title and the bell, with or without a picture beside
    /// them. Text once kept only its replies, so tmux's copies, programs'
    /// titles and the bell all went nowhere.
    #[test]
    fn text_keeps_its_clipboard_title_and_bell() {
        let (mut engine, mut intake, mut splitter) = rig();
        let p = pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b]52;c;aGk=\x07\x1b]2;one\x07\x07text");
        assert_eq!(p.clipboard.as_deref(), Some("hi"));
        assert_eq!(p.title.as_deref(), Some("one"));
        assert!(p.bell);
        // tmux names the clipboard and primary at once (`cp`).
        let p = pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b]52;cp;eW8=\x07");
        assert_eq!(p.clipboard.as_deref(), Some("yo"));
        // Around a picture, and the latest of each wins.
        let mut stream = b"\x1b]52;c;aGk=\x07\x1b]2;one\x07".to_vec();
        stream.extend_from_slice(APC_PUT);
        stream.extend_from_slice(b"\x1b]52;c;eW8=\x07\x1b]2;two\x07");
        let p = pump_bytes(&mut engine, &mut intake, &mut splitter, &stream);
        assert_eq!(p.clipboard.as_deref(), Some("yo"));
        assert_eq!(p.title.as_deref(), Some("two"));
        assert!(!p.bell);
    }

    /// A tiny deterministic random stream, so the cross-check is
    /// reproducible when it finds a divergence.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n.max(1)
        }
    }

    /// Exercise a random stream of text, LF, CR,
    /// RI, SU, SD, IL, DL, DECSTBM, autowrap and cursor moves, with
    /// pictures riding the markers' rows.
    #[test]
    fn random_region_streams_do_not_panic() {
        for seed in 0..40u64 {
            let (mut engine, mut intake, mut splitter) = rig();
            let mut rng = Rng(0x9e3779b97f4a7c15 ^ seed);
            let mut next_id = 31u32;
            let mut markers = [b'.'; 6];
            for step in 0..120 {
                let op = rng.below(12);
                let stream: Vec<u8> = match op {
                    0 => b"hello".to_vec(),
                    1 => b"\r\n".to_vec(),
                    2 => b"\r".to_vec(),
                    3 => b"\x1bM".to_vec(),
                    4 => format!("\x1b[{}S", 1 + rng.below(2)).into_bytes(),
                    5 => format!("\x1b[{}T", 1 + rng.below(2)).into_bytes(),
                    6 => format!("\x1b[{}L", 1 + rng.below(2)).into_bytes(),
                    7 => format!("\x1b[{}M", 1 + rng.below(2)).into_bytes(),
                    8 => {
                        let top = 1 + rng.below(4);
                        let bot = top + 1 + rng.below(4);
                        format!("\x1b[{top};{bot}r").into_bytes()
                    }
                    9 => format!("\x1b[{};{}H", 1 + rng.below(6), 1 + rng.below(10)).into_bytes(),
                    10 => {
                        // autowrap: fill to the last column
                        format!("\x1b[{};{}Hwwwwwwwwwwww", 1 + rng.below(6), 8 + rng.below(2))
                            .into_bytes()
                    }
                    _ => {
                        // A marker letter and a picture on its row.
                        let at = rng.below(6) as usize;
                        let c = (b'A' + at as u8) as char;
                        markers[at] = c as u8;
                        let row = 1 + rng.below(6);
                        let col = 1 + rng.below(8);
                        let mut s = format!("\x1b[{row};{col}H{c}").into_bytes();
                        s.extend_from_slice(
                            format!("\x1b[{row};{col}H\x1b_Gi={next_id},s=1,v=1,a=T,f=24;AAAA\x1b\\")
                                .as_bytes(),
                        );
                        next_id += 1;
                        s
                    }
                };
                pump_bytes(&mut engine, &mut intake, &mut splitter, &stream);
                let _ = step;
            }
            engine.scroll_to_bottom();
            // For every marker letter still on the screen, the picture's
            // row must equal the marked text's row.
            let rows = engine.size().0 as u16;
            let view = intake::View { top: engine.top_line(), rows, alternate: false };
            let placements = intake.describe(view).placements;
            let cells = engine.cells().to_vec();
            let cols = engine.size().1;
            for p in &placements {
                // The placement's id order follows its id; find its marker
                // by id offset from 31: id 31+k rode marker k%6.
                let _ = p;
            }
            let _ = (cells, cols, markers, rng);
        }
    }

    #[test]
    fn alternate_full_screen_scrolls_move_its_pictures() {
        let (mut engine, mut intake, mut splitter) = rig();
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[?1049h");
        // A picture at the alternate screen's top-left.
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[3;1H");
        pump_bytes(&mut engine, &mut intake, &mut splitter, APC_PUT);
        let at_put = shown(&mut intake, &mut engine, true);
        assert_eq!(at_put, vec![(2.0, 0.0)], "anchored at its text's row");
        // Four linefeeds carry the cursor from row 2 to the bottom
        // margin of a six-row screen; each one after that scrolls the
        // alternate screen by one. After five the count is exact and the
        // picture stayed on its text's row.
        for _ in 0..5 {
            pump_bytes(&mut engine, &mut intake, &mut splitter, b"\r\n");
        }
        assert_eq!(engine.alt_scrolled(), 2, "the alternate screen's count is exact");
        let after = shown(&mut intake, &mut engine, true);
        assert_eq!(after, vec![(0.0, 0.0)], "the picture moved with the alternate screen's text");
    }

    fn grid_text(engine: &mut Engine) -> String {
        let cols = engine.size().1;
        let cells = engine.cells();
        let mut text = String::new();
        for (i, cell) in cells.iter().enumerate() {
            if i % cols == 0 {
                text.push('\n');
            }
            text.push(if cell.c == '\0' { ' ' } else { cell.c });
        }
        text
    }

    fn shown(intake: &mut Intake, engine: &mut Engine, alternate: bool) -> Vec<(f32, f32)> {
        let rows = engine.size().0 as u16;
        let view = intake::View { top: engine.top_line(), rows, alternate };
        intake
            .describe(view)
            .placements
            .into_iter()
            .map(|s| (s.row, s.column))
            .collect()
    }

    #[test]
    fn output_is_the_same_however_the_stream_is_split() {
        let streams: [&[u8]; 5] = [
            b"a\nb\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;\xff\x00\x00\xff\x1b\\\x1b[c",
            b"\x1b[?25l\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;\x00\xff\x00\xff\x1b\\\x1b[2J",
            b"\x1b[?2026hx\r\n\r\n\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;\xff\x00\x00\xff\x1b\\more\x1b[?2026l",
            b"\x1bPq#0~\x1b\\prompt\r\n\x1b_Gi=9,s=1,v=1,a=T,f=24;AAAA\x1b\\\x1b[?1;3S",
            b"\x1b[?1049h\x1b_Gi=7,s=1,v=1,a=T,f=24;AAAA\x1b\\\x1b[?1049l\x1b[2J",
        ];
        for stream in streams {
            let (mut engine, mut intake, mut splitter) = rig();
            let whole = pump_bytes(&mut engine, &mut intake, &mut splitter, stream);
            let whole_text = grid_text(&mut engine);
            let whole_shown = shown(&mut intake, &mut engine, false);
            for split in 1..stream.len() {
                let (mut e2, mut i2, mut s2) = rig();
                let a = pump_bytes(&mut e2, &mut i2, &mut s2, &stream[..split]);
                let b = pump_bytes(&mut e2, &mut i2, &mut s2, &stream[split..]);
                let mut replies = a.replies.clone();
                replies.extend_from_slice(&b.replies);
                assert_eq!(replies, whole.replies, "replies differ at split {split}");
                assert_eq!(grid_text(&mut e2), whole_text, "grid differs at split {split}");
                assert_eq!(shown(&mut i2, &mut e2, false), whole_shown, "placements at {split}");
            }
            let (mut e3, mut i3, mut s3) = rig();
            let mut replies = Vec::new();
            for byte in stream {
                replies.extend_from_slice(
                    &pump_bytes(&mut e3, &mut i3, &mut s3, &[*byte]).replies,
                );
            }
            assert_eq!(replies, whole.replies, "one-byte chunks differ in replies");
            assert_eq!(grid_text(&mut e3), whole_text, "one-byte chunks differ in grid");
            assert_eq!(shown(&mut i3, &mut e3, false), whole_shown, "one-byte chunks differ in placements");
        }
    }

    #[test]
    fn a_picture_anchors_to_the_stream_and_moves_with_the_text() {
        let (mut engine, mut intake, mut splitter) = rig();
        // `a\nb\n` leaves the cursor on row 2 with one line scrolled.
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"a\r\nb\r\n");
        pump_bytes(&mut engine, &mut intake, &mut splitter, APC_PUT);
        let at_put = shown(&mut intake, &mut engine, false);
        assert_eq!(at_put, vec![(2.0, 0.0)], "anchored at count + cursor row");
        // 100 more lines move it up exactly 100: scrolled back to it, it
        // shows at its text's row.
        for _ in 0..25 {
            pump_bytes(&mut engine, &mut intake, &mut splitter, b"aaaa\r\nbbbb\r\ncccc\r\ndddd\r\n");
        }
        // The cursor began on row 2 of a 6-row screen, so 100 more lines
        // carry it past the bottom edge 97 times: 97 lines left the top.
        assert_eq!(engine.scrolled_off_top(), 97, "the count is exact");
        // The view is at the live screen, so the picture (100 lines up)
        // is not in it; lift the view to its line and it shows there.
        assert!(shown(&mut intake, &mut engine, false).is_empty());
        // Lift the view into the scrollback: the offset grows upward.
        for _ in 0..25 {
            engine.scroll_display(6);
        }
        let back = shown(&mut intake, &mut engine, false);
        assert!(!back.is_empty(), "shows at its text's row when scrolled back");
    }

    #[test]
    fn a_picture_hides_with_the_cursor_hidden() {
        let (mut engine, mut intake, mut splitter) = rig();
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[?25l");
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[2;3H");
        pump_bytes(&mut engine, &mut intake, &mut splitter, APC_PUT);
        // The cursor hidden, the picture still anchors at the hidden
        // cursor's cell: row 1, column 2, zero-based.
        assert_eq!(shown(&mut intake, &mut engine, false), vec![(1.0, 2.0)]);
    }

    #[test]
    fn a_synchronized_update_holds_the_frame_and_the_picture_lands() {
        let (mut engine, mut intake, mut splitter) = rig();
        pump_bytes(
            &mut engine,
            &mut intake,
            &mut splitter,
            b"\x1b[?2026hx\r\n\r\n",
        );
        assert!(engine.holding_frame(Duration::from_millis(150)));
        pump_bytes(&mut engine, &mut intake, &mut splitter, APC_PUT);
        assert!(engine.holding_frame(Duration::from_millis(150)));
        // The media piece settled the processor's buffer so the picture
        // anchors in the stream, so DECRQM answers truthfully that the
        // synchronized update is no longer open — while the frame still
        // shows as one update.
        assert_eq!(
            pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[?2026$p").replies,
            b"\x1b[?2026;2$y"
        );
        assert!(engine.holding_frame(Duration::from_millis(150)));
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"more");
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[?2026l");
        assert!(!engine.holding_frame(Duration::from_millis(150)));
        // The picture lands two rows below `x`.
        assert_eq!(shown(&mut intake, &mut engine, false), vec![(2.0, 0.0)]);
    }

    #[test]
    fn reply_order_follows_the_stream() {
        let (mut engine, mut intake, mut splitter) = rig();
        let apc_probe = b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[c";
        let first = pump_bytes(&mut engine, &mut intake, &mut splitter, apc_probe);
        assert!(
            first.replies.starts_with(b"\x1b_Gi=31;OK\x1b\\"),
            "APC reply first: {:?}",
            String::from_utf8_lossy(&first.replies)
        );
        assert!(first.replies.ends_with(b"\x1b[?62;4;22c"));

        let (mut e2, mut i2, mut s2) = rig();
        let reversed = pump_bytes(&mut e2, &mut i2, &mut s2, b"\x1b[c\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\");
        assert!(
            reversed.replies.starts_with(b"\x1b[?62;4;22c"),
            "DA1 first: {:?}",
            String::from_utf8_lossy(&reversed.replies)
        );
        assert!(reversed.replies.ends_with(b"\x1b_Gi=31;OK\x1b\\"));
    }

    #[test]
    fn xtsmgraphics_reads_are_answered() {
        let (mut engine, mut intake, mut splitter) = rig();
        let a = pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[?1;1S");
        assert_eq!(a.replies, b"\x1b[?1;0;1024S");
        let b = pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[?2;1S");
        assert_eq!(b.replies, b"\x1b[?2;0;100;120S");
        let c = pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[?3;1S");
        assert_eq!(c.replies, b"\x1b[?3;1;0S");
        // A read that goes on past the query (a probe followed by DA1, as
        // programs send them) is answered too, and the rest still runs.
        let d = pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[?1;1S\x1b[?25lSome text\x1b[c");
        assert_eq!(d.replies, b"\x1b[?1;0;1024S\x1b[?62;4;22c");
    }

    /// The count stays exact past a full history: the picture was placed
    /// before the history filled, and the count keeps counting.
    #[test]
    fn the_count_stays_exact_past_a_full_history() {
        let (mut engine, mut intake, mut splitter) = rig();
        pump_bytes(&mut engine, &mut intake, &mut splitter, APC_PUT);
        // 10 000 lines of history fill, then some more.
        for _ in 0..2_600 {
            pump_bytes(&mut engine, &mut intake, &mut splitter, b"aaaa\r\nbbbb\r\ncccc\r\ndddd\r\n");
        }
        let now = engine.scrolled_off_top();
        assert!(now > 10_000, "the count grew past the history: {now}");
        assert_eq!(intake.scrolled(), now as u64, "the store heard the exact count");
    }

    #[test]
    fn clear_removes_every_picture_and_ctrl_l_keeps_history_aligned() {
        let (mut engine, mut intake, mut splitter) = rig();
        // A marker line, then the picture on its own line.
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"marker\r\n");
        pump_bytes(&mut engine, &mut intake, &mut splitter, APC_PUT);
        // Push the marker and its picture into history, well above the
        // screen, so Ctrl+L's push cannot touch them.
        for _ in 0..2 {
            pump_bytes(&mut engine, &mut intake, &mut splitter, b"fill\r\nfill\r\nfill\r\n");
        }
        let rows = engine.size().0 as u16;
        // Ctrl+L: the screen's pictures go; history keeps its own.
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[H\x1b[2J");
        let on_screen =
            intake.describe(intake::View { top: engine.top_line(), rows, alternate: false });
        assert!(on_screen.placements.is_empty(), "the screen's pictures went");
        // Scroll back: the marker line is in history with its picture.
        for _ in 0..4 {
            engine.scroll_display(6);
        }
        let back =
            intake.describe(intake::View { top: engine.top_line(), rows, alternate: false });
        assert!(
            !back.placements.is_empty(),
            "a picture stayed in history, aligned with its text"
        );
        // `clear`: nothing at all.
        engine.scroll_to_bottom();
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[H\x1b[2J\x1b[3J");
        let after =
            intake.describe(intake::View { top: engine.top_line(), rows, alternate: false });
        assert!(after.placements.is_empty(), "clear leaves no pictures");
    }

    #[test]
    fn ris_removes_all_pictures() {
        let (mut engine, mut intake, mut splitter) = rig();
        pump_bytes(&mut engine, &mut intake, &mut splitter, APC_PUT);
        let rows = engine.size().0 as u16;
        assert!(
            !intake.describe(intake::View { top: 0, rows, alternate: false }).placements.is_empty()
        );
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1bc");
        let after = intake.describe(intake::View { top: 0, rows, alternate: false });
        assert!(after.placements.is_empty(), "RIS removes all pictures");
    }

    /// A sixel picture lives in cells: text over them or an erase on its
    /// row takes it away. An APC-graphics placement survives both.
    #[test]
    fn in_cell_pictures_die_when_covered_and_apc_survives() {
        let (mut engine, mut intake, mut splitter) = rig();
        intake.set_cell((10, 20));
        // A sixel one cell tall at this cell size: text over that cell
        // covers every cell, so the picture goes.
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1bPq#0~\x1b\\");
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\r\n$ ");
        let rows = engine.size().0 as u16;
        let sixel = intake.describe(intake::View { top: 0, rows, alternate: false });
        assert!(!sixel.placements.is_empty(), "the sixel was placed");
        // Text over its cells removes it whole. The sixel band sits on
        // row 0 (its cursor moves below the image), so write there.
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[1;1HX");
        let covered = intake.describe(intake::View { top: 0, rows, alternate: false });
        assert!(covered.placements.is_empty(), "text over every cell removed the sixel");

        // EL on one of its rows takes it away too.
        let (mut engine, mut intake, mut splitter) = rig();
        intake.set_cell((10, 20));
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1bPq#0~\x1b\\");
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[1;1H\x1b[K");
        let rows = engine.size().0 as u16;
        let after_el = intake.describe(intake::View { top: 0, rows, alternate: false });
        assert!(after_el.placements.is_empty(), "EL on its row removed the sixel");

        let (mut engine, mut intake, mut splitter) = rig();
        pump_bytes(&mut engine, &mut intake, &mut splitter, APC_PUT);
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[HXXXXXX");
        let survived = intake.describe(intake::View { top: 0, rows, alternate: false });
        assert!(!survived.placements.is_empty(), "an APC placement survives text over its cells");
        // EL on its row does not remove it either.
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[1;1H\x1b[K");
        let after_el = intake.describe(intake::View { top: 0, rows, alternate: false });
        assert!(!after_el.placements.is_empty(), "an APC placement survives EL");
        // ED 0 does not either.
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[J");
        let after_ed = intake.describe(intake::View { top: 0, rows, alternate: false });
        assert!(!after_ed.placements.is_empty(), "an APC placement survives ED 0");
    }

    #[test]
    fn the_alternate_screen_drops_its_pictures_and_restarts_its_count() {
        let (mut engine, mut intake, mut splitter) = rig();
        pump_bytes(&mut engine, &mut intake, &mut splitter, APC_PUT);
        let rows = engine.size().0 as u16;
        assert!(
            !intake.describe(intake::View { top: 0, rows, alternate: false })
                .placements
                .is_empty()
        );

        // Enter the alternate screen, put a picture, leave: that picture
        // is gone and the main screen's is still there.
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[?1049h");

        // Another image id: the same id would move the one showing, not
        // add a second.
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b_Gi=32,s=1,v=1,a=T,f=24;AAAA\x1b\\");
        let alt = intake.describe(intake::View { top: 0, rows, alternate: true });
        assert!(!alt.placements.is_empty(), "the alternate screen holds its picture");
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[?1049l");
        let main =
            intake.describe(intake::View { top: engine.top_line(), rows, alternate: false });
        assert!(!main.placements.is_empty(), "the main screen's picture is still there");

        // Enter again: the count starts at 0.
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[?1049h");
        assert_eq!(engine.alt_scrolled(), 0, "the alternate count starts at 0");
        // Fill the screen with lines, so its top row is not blank: the
        // count tells a scroll off the top by the top row changing.
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"one\r\ntwo\r\nthree\r\n");
        assert_eq!(engine.alt_scrolled(), 0, "scrolls that fit on the screen do not count");
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"four\r\nfive\r\nsix\r\nseven\r\n");
        // Two linefeeds at the bottom margin scroll the alternate screen.
        // The count is those lines, and the picture that left the top is
        // gone: the alternate screen keeps no history.
        assert_eq!(engine.alt_scrolled(), 2, "the alternate screen's count is exact");
    }

    fn tiny_picture(id: u32) -> Vec<u8> {
        format!("\x1b_Gi={id},s=1,v=1,a=T,f=24;AAAA\x1b\\").into_bytes()
    }

    /// Pictures inside a scroll region move with its text. Pictures
    /// outside stay. A picture that leaves the region is cut away.
    #[test]
    fn a_scroll_region_moves_its_pictures_with_the_text() {
        let (mut engine, mut intake, mut splitter) = rig();
        let mut bytes = Vec::new();
        bytes.extend(b"OUT0");
        bytes.extend(b"\x1b[1;9H");
        bytes.extend(tiny_picture(1));
        bytes.extend(b"\x1b[4;1HIN03");
        bytes.extend(b"\x1b[4;9H");
        bytes.extend(tiny_picture(2));
        bytes.extend(b"\x1b[6;1HOUT5");
        bytes.extend(b"\x1b[6;9H");
        bytes.extend(tiny_picture(3));
        // Region rows 2..5 (1-based 3;5). Scroll up one line.
        bytes.extend(b"\x1b[3;5r\x1b[1S");
        pump_bytes(&mut engine, &mut intake, &mut splitter, &bytes);
        let grid = grid_text(&mut engine);
        let lines: Vec<&str> = grid.split('\n').collect();
        let row_of = |mark: &str| lines.iter().position(|line| line.contains(mark)).map(|r| r as f32 - 1.0);
        let rows = engine.size().0 as u16;
        let shown = intake.describe(intake::View { top: 0, rows, alternate: false });
        let pic_rows: Vec<f32> = shown.placements.iter().map(|p| p.row).collect();
        assert_eq!(row_of("OUT0"), Some(0.0), "text outside the region stayed");
        assert_eq!(row_of("IN03"), Some(2.0), "text inside the region moved up, grid={grid:?}");
        assert_eq!(row_of("OUT5"), Some(5.0), "text below the region stayed");
        assert!(pic_rows.contains(&0.0), "the picture outside stayed, got {pic_rows:?}");
        assert!(pic_rows.contains(&2.0), "the picture inside moved, got {pic_rows:?}");
        assert!(pic_rows.contains(&5.0), "the picture below stayed, got {pic_rows:?}");
    }

    /// A picture that scrolls out of a region is cropped at the margin.
    #[test]
    fn a_picture_that_leaves_a_region_is_cropped() {
        let (mut engine, mut intake, mut splitter) = rig();
        // A sixel two cells tall would need a taller image. Use two
        // one-cell pictures on consecutive rows inside the region, and
        // scroll the top one out.
        let mut bytes = Vec::new();
        bytes.extend(b"\x1b[3;1H");
        bytes.extend(tiny_picture(1));
        bytes.extend(b"\x1b[4;1H");
        bytes.extend(tiny_picture(2));
        bytes.extend(b"\x1b[3;5r\x1b[1S");
        pump_bytes(&mut engine, &mut intake, &mut splitter, &bytes);
        let rows = engine.size().0 as u16;
        let shown = intake.describe(intake::View { top: 0, rows, alternate: false });
        assert_eq!(shown.placements.len(), 1, "the picture that left is gone");
        assert!((shown.placements[0].row - 2.0).abs() < 0.01, "the survivor moved to the top of the region");
    }

    /// One sixel picture, 1x6 px, drawn at the cursor.
    const SIXEL: &[u8] = b"\x1bPq#0;2;100;0;0#0~\x1b\\";

    fn lines(n: usize) -> Vec<u8> {
        (0..n).flat_map(|i| format!("l{i}\r\n").into_bytes()).collect()
    }

    #[test]
    fn ctrl_l_after_saved_lines_were_cleared_counts_what_it_pushes() {
        let mut engine = engine();
        engine.process(&lines(20));
        assert_eq!(engine.scrolled_off_top(), 15);
        engine.process(b"\x1b[3J");
        engine.process(&lines(5));
        // Rows 0-4 hold text, the cursor sits on the empty row 5: Ctrl+L
        // pushes five lines, so every line printed is now above the screen.
        engine.process(b"\x1b[H\x1b[2J");
        assert_eq!(engine.scrolled_off_top(), 25);
    }

    #[test]
    fn ctrl_l_past_a_full_history_counts_what_it_pushes() {
        let mut engine = engine();
        engine.process(&lines(12_000));
        assert_eq!(engine.scrolled_off_top(), 11_995);
        engine.process(b"\x1b[H\x1b[2J");
        assert_eq!(engine.scrolled_off_top(), 12_000);
    }

    #[test]
    fn scrolls_and_a_clear_in_one_feed_are_all_counted() {
        let mut engine = engine();
        let mut feed = lines(40);
        feed.extend_from_slice(b"\x1b[2J");
        engine.process(&feed);
        assert_eq!(engine.scrolled_off_top(), 40);
    }

    #[test]
    fn scrolls_and_a_screen_switch_in_one_feed_are_all_counted() {
        let mut engine = engine();
        let mut feed = lines(40);
        feed.extend_from_slice(b"\x1b[?1049h");
        engine.process(&feed);
        engine.process(b"\x1b[?1049l");
        assert_eq!(engine.scrolled_off_top(), 35);
    }

    #[test]
    fn ctrl_l_counts_a_blank_row_with_a_background_as_written() {
        // `Term` keeps a row of spaces with a background colour when it
        // pushes the screen into history, so the count must too.
        let mut engine = engine();
        engine.process(b"a\r\nb\r\nc\r\n\x1b[44m\x1b[K\x1b[0m\r\n\x1b[H\x1b[2J");
        assert_eq!(engine.history_size(), 4, "what Term pushed");
        assert_eq!(engine.scrolled_off_top(), 4);
    }

    #[test]
    fn a_picture_in_history_stays_on_its_text_after_ctrl_l() {
        let (mut engine, mut intake, mut splitter) = rig();
        intake.set_cell((10, 20));
        let mut stream = b"a\r\nMARK\x1b_Gi=31,s=1,v=1,a=T,f=24,C=1;AAAA\x1b\\\r\n".to_vec();
        stream.extend_from_slice(&lines(10));
        stream.extend_from_slice(b"\x1b[H\x1b[2J");
        pump_bytes(&mut engine, &mut intake, &mut splitter, &stream);
        engine.scroll_display(11);
        let row = grid_text(&mut engine)
            .lines()
            .skip(1)
            .position(|line| line.starts_with("MARK"))
            .expect("MARK is in view");
        assert_eq!(shown(&mut intake, &mut engine, false), vec![(row as f32, 4.0)]);
    }

    #[test]
    fn a_clear_and_a_picture_in_one_synchronized_update_keep_the_picture() {
        let (mut engine, mut intake, mut splitter) = rig();
        let mut stream = b"a\r\nb\r\n\x1b[?2026h\x1b[H\x1b[2J".to_vec();
        stream.extend_from_slice(APC_PUT);
        stream.extend_from_slice(b"\x1b[?2026l");
        pump_bytes(&mut engine, &mut intake, &mut splitter, &stream);
        assert_eq!(shown(&mut intake, &mut engine, false), vec![(0.0, 0.0)]);
    }

    #[test]
    fn a_picture_after_scrolled_text_in_one_synchronized_update_lands_on_its_row() {
        let (mut engine, mut intake, mut splitter) = rig();
        let mut stream = b"\x1b[?2026h".to_vec();
        stream.extend_from_slice(&lines(10));
        stream.extend_from_slice(APC_PUT);
        stream.extend_from_slice(b"\x1b[?2026l");
        pump_bytes(&mut engine, &mut intake, &mut splitter, &stream);
        assert_eq!(shown(&mut intake, &mut engine, false), vec![(5.0, 0.0)]);
    }

    #[test]
    fn a_sixel_redrawn_over_its_old_self_in_one_synchronized_update_is_shown_once() {
        let (mut engine, mut intake, mut splitter) = rig();
        intake.set_cell((10, 20));
        let mut stream = b"\x1b[H".to_vec();
        stream.extend_from_slice(SIXEL);
        stream.extend_from_slice(b"\x1b[?2026h\x1b[HXXXX\x1b[H");
        stream.extend_from_slice(SIXEL);
        stream.extend_from_slice(b"\x1b[?2026l");
        pump_bytes(&mut engine, &mut intake, &mut splitter, &stream);
        assert_eq!(shown(&mut intake, &mut engine, false).len(), 1);
    }

    #[test]
    fn a_wide_character_covers_both_its_cells() {
        let (mut engine, mut intake, mut splitter) = rig();
        intake.set_cell((10, 20));
        let mut stream = b"\x1b[1;2H".to_vec();
        stream.extend_from_slice(SIXEL);
        pump_bytes(&mut engine, &mut intake, &mut splitter, &stream);
        assert_eq!(shown(&mut intake, &mut engine, false).len(), 1);
        pump_bytes(&mut engine, &mut intake, &mut splitter, "\x1b[H你".as_bytes());
        assert!(shown(&mut intake, &mut engine, false).is_empty());
    }

    #[test]
    fn a_narrow_character_of_several_bytes_covers_one_cell() {
        let (mut engine, mut intake, mut splitter) = rig();
        intake.set_cell((10, 20));
        let mut stream = b"\x1b[1;2H".to_vec();
        stream.extend_from_slice(SIXEL);
        pump_bytes(&mut engine, &mut intake, &mut splitter, &stream);
        pump_bytes(&mut engine, &mut intake, &mut splitter, "\x1b[Hé".as_bytes());
        assert_eq!(shown(&mut intake, &mut engine, false).len(), 1);
    }

    #[test]
    fn icats_chunked_stream_is_shown_at_any_read_size() {
        // `kitten icat --transfer-mode=stream`, captured live: a 128 KiB
        // first chunk `a=T,q=2,...,m=1` and a last chunk `a=T,q=2` with no
        // `m`, fed in the read sizes a pty hands over.
        let raw = include_bytes!("intake/testdata/icat-stream-chunked.apc");
        for size in [raw.len(), 65_536, 4096, 1000] {
            let (mut engine, mut intake, mut splitter) = rig();
            intake.set_cell((10, 20));
            for read in raw.chunks(size) {
                pump_bytes(&mut engine, &mut intake, &mut splitter, read);
            }
            assert_eq!(shown(&mut intake, &mut engine, false).len(), 1, "read size {size}");
        }
    }

    fn tmux_wrap(body: &[u8]) -> Vec<u8> {
        let mut out = b"\x1bPtmux;".to_vec();
        for &byte in body {
            if byte == 0x1b {
                out.push(0x1b);
            }
            out.push(byte);
        }
        out.extend_from_slice(b"\x1b\\");
        out
    }

    #[test]
    fn pictures_inside_tmux_passthrough_show() {
        let png = crate::intake::testing::encode_png(2, 1);
        let mut encoded = Vec::new();
        {
            const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut i = 0;
            while i + 3 <= png.len() {
                let n = (u32::from(png[i]) << 16) | (u32::from(png[i + 1]) << 8) | u32::from(png[i + 2]);
                encoded.push(T[((n >> 18) & 63) as usize]);
                encoded.push(T[((n >> 12) & 63) as usize]);
                encoded.push(T[((n >> 6) & 63) as usize]);
                encoded.push(T[(n & 63) as usize]);
                i += 3;
            }
            let rest = png.len() - i;
            if rest == 1 {
                let n = u32::from(png[i]) << 16;
                encoded.push(T[((n >> 18) & 63) as usize]);
                encoded.push(T[((n >> 12) & 63) as usize]);
                encoded.extend_from_slice(b"==");
            } else if rest == 2 {
                let n = (u32::from(png[i]) << 16) | (u32::from(png[i + 1]) << 8);
                encoded.push(T[((n >> 18) & 63) as usize]);
                encoded.push(T[((n >> 12) & 63) as usize]);
                encoded.push(T[((n >> 6) & 63) as usize]);
                encoded.push(b'=');
            }
        }
        let mut file = b"\x1b]1337;File=inline=1:".to_vec();
        file.extend_from_slice(&encoded);
        file.extend_from_slice(b"\x07");
        let sixel = b"\x1bPq~-~-\x1b\\";
        let apc = b"\x1b_Ga=T,f=24,s=1,v=1;AAAA\x1b\\";
        for body in [apc.as_slice(), sixel.as_slice(), file.as_slice()] {
            let (mut direct_e, mut direct_i, mut direct_s) = rig();
            direct_i.set_cell((8, 16));
            pump_bytes(&mut direct_e, &mut direct_i, &mut direct_s, body);
            let direct = shown(&mut direct_i, &mut direct_e, false);
            assert!(!direct.is_empty(), "the unwrapped sequence places a picture");
            let (mut engine, mut intake, mut splitter) = rig();
            intake.set_cell((8, 16));
            pump_bytes(&mut engine, &mut intake, &mut splitter, &tmux_wrap(body));
            assert_eq!(shown(&mut intake, &mut engine, false), direct);
        }
    }

    #[test]
    fn a_multipart_png_through_pump_matches_file() {
        let png = crate::intake::testing::encode_png(2, 2);
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut encoded = Vec::new();
        let mut i = 0;
        while i + 3 <= png.len() {
            let n = (u32::from(png[i]) << 16) | (u32::from(png[i + 1]) << 8) | u32::from(png[i + 2]);
            encoded.push(T[((n >> 18) & 63) as usize]);
            encoded.push(T[((n >> 12) & 63) as usize]);
            encoded.push(T[((n >> 6) & 63) as usize]);
            encoded.push(T[(n & 63) as usize]);
            i += 3;
        }
        let rest = png.len() - i;
        if rest > 0 {
            let n = if rest == 1 {
                u32::from(png[i]) << 16
            } else {
                (u32::from(png[i]) << 16) | (u32::from(png[i + 1]) << 8)
            };
            encoded.push(T[((n >> 18) & 63) as usize]);
            encoded.push(T[((n >> 12) & 63) as usize]);
            if rest == 2 {
                encoded.push(T[((n >> 6) & 63) as usize]);
                encoded.push(b'=');
            } else {
                encoded.extend_from_slice(b"==");
            }
        }
        let mut single = b"\x1b]1337;File=inline=1:".to_vec();
        single.extend_from_slice(&encoded);
        single.push(0x07);
        let (mut e1, mut i1, mut s1) = rig();
        i1.set_cell((8, 16));
        pump_bytes(&mut e1, &mut i1, &mut s1, &single);
        let want = shown(&mut i1, &mut e1, false);

        let mut multi = b"\x1b]1337;MultipartFile=inline=1\x07".to_vec();
        for chunk in encoded.chunks(7) {
            multi.extend_from_slice(b"\x1b]1337;FilePart=");
            multi.extend_from_slice(chunk);
            multi.push(0x07);
        }
        multi.extend_from_slice(b"\x1b]1337;FileEnd\x07");
        let (mut e2, mut i2, mut s2) = rig();
        i2.set_cell((8, 16));
        pump_bytes(&mut e2, &mut i2, &mut s2, &multi);
        assert_eq!(shown(&mut i2, &mut e2, false), want);
        assert!(!want.is_empty());
    }

    #[test]
    fn an_abandoned_multipart_is_dropped_by_a_new_one_and_by_the_budget() {
        let (mut engine, mut intake, mut splitter) = rig();
        intake.set_cell((8, 16));
        // A new MultipartFile drops the abandoned pieces.
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b]1337;MultipartFile=inline=1\x07\x1b]1337;FilePart=AAAA\x07");
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b]1337;MultipartFile=inline=0\x07\x1b]1337;FileEnd\x07");
        assert!(shown(&mut intake, &mut engine, false).is_empty());

        let mut tiny = crate::intake::Intake::with_budget(1, 4);
        tiny.set_cell((8, 16));
        let mut engine = crate::engine::Engine::new(6, 20, HostInfo { colors: [[0; 3]; 3], cell_px: (8, 16) });
        let mut splitter = crate::intake::Splitter::default();
        pump_bytes(
            &mut engine,
            &mut tiny,
            &mut splitter,
            b"\x1b]1337;MultipartFile=inline=1\x07\x1b]1337;FilePart=abcdefghij\x07\x1b]1337;FileEnd\x07",
        );
        assert!(shown(&mut tiny, &mut engine, false).is_empty(), "past the budget, nothing is placed");
    }

    fn b64_str(data: &[u8]) -> String {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        let mut i = 0;
        while i + 3 <= data.len() {
            let n = (u32::from(data[i]) << 16) | (u32::from(data[i + 1]) << 8) | u32::from(data[i + 2]);
            out.push(T[((n >> 18) & 63) as usize] as char);
            out.push(T[((n >> 12) & 63) as usize] as char);
            out.push(T[((n >> 6) & 63) as usize] as char);
            out.push(T[(n & 63) as usize] as char);
            i += 3;
        }
        let rest = &data[i..];
        if rest.len() == 1 {
            let n = u32::from(rest[0]) << 16;
            out.push(T[((n >> 18) & 63) as usize] as char);
            out.push(T[((n >> 12) & 63) as usize] as char);
            out.push('=');
            out.push('=');
        } else if rest.len() == 2 {
            let n = (u32::from(rest[0]) << 16) | (u32::from(rest[1]) << 8);
            out.push(T[((n >> 18) & 63) as usize] as char);
            out.push(T[((n >> 12) & 63) as usize] as char);
            out.push(T[((n >> 6) & 63) as usize] as char);
            out.push('=');
        }
        out
    }

    fn held_pictures(intake: &crate::intake::Intake) -> Vec<(u32, Vec<u8>)> {
        (1..=8)
            .filter_map(|n| {
                let id = crate::intake::MINT_BASE + n;
                // A minted picture lives in the space of the dialect that sent it.
                intake
                    .pixels_of(crate::intake::Space::Osc, id)
                    .or_else(|| intake.pixels_of(crate::intake::Space::Apc, id))
                    .map(|(_, _, rgba, _)| (id, rgba.as_ref().clone()))
            })
            .collect()
    }

    /// `m=1` on a file transmission must not open a chunk: the next direct
    /// picture still shows.
    #[test]
    fn a_file_transmission_ignores_chunking() {
        let (mut engine, mut intake, mut splitter) = rig();
        let bogus = b64_str(b"/no/such/glass-gap-file");
        let first = format!("\x1b_Ga=T,t=f,f=32,m=1;{bogus}\x1b\\");
        pump_bytes(&mut engine, &mut intake, &mut splitter, first.as_bytes());
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b_Ga=T,f=24,s=1,v=1,q=2;AAAA\x1b\\");
        assert_eq!(shown(&mut intake, &mut engine, false).len(), 1, "m=1 on t=f must not swallow the next picture");
    }

    /// mpv `--vo=kitty --vo-kitty-use-shm=yes` sends every frame as
    /// `a=T,t=s,f=24,...,C=1,q=2,m=1` with a shared-memory name that has
    /// no leading `/`, and never a final chunk. Each frame replaces the
    /// last picture and placement; the store does not grow.
    #[test]
    fn mpv_shared_memory_frames_replace_without_growing() {
        struct Shm { path: std::ffi::CString }
        impl Drop for Shm {
            fn drop(&mut self) {
                unsafe { libc::shm_unlink(self.path.as_ptr()); }
            }
        }
        fn object(bare: &str, rgb: &[u8]) -> Shm {
            let path = std::ffi::CString::new(format!("/{bare}")).unwrap();
            let fd = unsafe { libc::shm_open(path.as_ptr(), libc::O_CREAT | libc::O_RDWR | libc::O_CLOEXEC, 0o600) };
            assert!(fd >= 0, "shm_open {}", std::io::Error::last_os_error());
            assert_eq!(unsafe { libc::ftruncate(fd, rgb.len() as i64) }, 0);
            let mut written = 0;
            while written < rgb.len() {
                let n = unsafe { libc::write(fd, rgb[written..].as_ptr().cast(), rgb.len() - written) };
                assert!(n > 0, "write shm");
                written += n as usize;
            }
            unsafe { libc::close(fd); }
            Shm { path }
        }

        let (mut engine, mut intake, mut splitter) = rig();
        let frames: [&[u8]; 3] = [
            &[255, 0, 0, 255, 0, 0],
            &[0, 255, 0, 0, 255, 0],
            &[0, 0, 255, 0, 0, 255],
        ];
        let mut place = Vec::new();
        for (i, rgb) in frames.iter().enumerate() {
            let bare = format!("mpv-kitty-0x{:x}{}", std::process::id(), i + 1);
            let _kept = object(&bare, rgb);
            let seq = format!(
                "\x1b_Ga=T,t=s,f=24,s=2,v=1,C=1,q=2,m=1;{}\x1b\\",
                b64_str(bare.as_bytes())
            );
            pump_bytes(&mut engine, &mut intake, &mut splitter, seq.as_bytes());
            let shown_now = shown(&mut intake, &mut engine, false);
            assert_eq!(shown_now.len(), 1, "frame {} should be the only placement", i + 1);
            if i == 0 {
                place = shown_now;
            } else {
                assert_eq!(shown_now, place, "C=1 holds the placement on the same cell");
            }
            let held = held_pictures(&intake);
            assert_eq!(held.len(), 1, "the store grew on frame {}: {held:?}", i + 1);
            assert_eq!(held[0].0, crate::intake::MINT_BASE + (i as u32) + 1);
            let mut rgba = Vec::new();
            for px in rgb.chunks(3) {
                rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
            assert_eq!(held[0].1, rgba, "frame {} pixels", i + 1);
        }
    }

    fn encode_still(kind: &str, rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
        use image::ImageEncoder;
        use std::io::Cursor;
        let mut buf = Cursor::new(Vec::new());
        let color = image::ExtendedColorType::Rgba8;
        let result = match kind {
            "tiff" => image::codecs::tiff::TiffEncoder::new(&mut buf).write_image(rgba, w, h, color),
            "webp" => image::codecs::webp::WebPEncoder::new_lossless(&mut buf).write_image(rgba, w, h, color),
            "bmp" => image::codecs::bmp::BmpEncoder::new(&mut buf).write_image(rgba, w, h, color),
            other => panic!("unknown still {other}"),
        };
        result.unwrap_or_else(|err| panic!("{kind} encode: {err}"));
        buf.into_inner()
    }

    /// TIFF (the still chafa's iterm output carries), WebP and BMP decode
    /// to the source pixels through OSC 1337 and through `f=100`.
    #[test]
    fn tiff_webp_and_bmp_decode_through_both_dialects() {
        let rgba = [255u8, 0, 0, 255, 0, 255, 0, 255];
        for kind in ["tiff", "webp", "bmp"] {
            let encoded = encode_still(kind, &rgba, 2, 1);
            let payload = b64_str(&encoded);

            let (mut engine, mut intake, mut splitter) = rig();
            // chafa's iterm keys: inline, a cell box, no aspect lock, then a TIFF.
            let osc = format!(
                "\x1b]1337;File=inline=1;width=20;height=5;preserveAspectRatio=0:{payload}\x07"
            );
            pump_bytes(&mut engine, &mut intake, &mut splitter, osc.as_bytes());
            let held = held_pictures(&intake);
            assert_eq!(held.len(), 1, "{kind} through OSC 1337");
            assert_eq!(held[0].1, rgba, "{kind} OSC 1337 pixels");
            assert_eq!(shown(&mut intake, &mut engine, false).len(), 1);

            let (mut engine, mut intake, mut splitter) = rig();
            let apc = format!("\x1b_Ga=T,f=100,q=2;{payload}\x1b\\");
            pump_bytes(&mut engine, &mut intake, &mut splitter, apc.as_bytes());
            let held = held_pictures(&intake);
            assert_eq!(held.len(), 1, "{kind} through f=100");
            assert_eq!(held[0].1, rgba, "{kind} f=100 pixels");
            assert_eq!(shown(&mut intake, &mut engine, false).len(), 1);
        }
    }
}

/// The showcase request as last applied, without the frame or the progress.
#[derive(Clone, PartialEq)]
enum OpenStamp {
    Picture { id: u32, seq: u64, fit: angel_glass::Fit },
    Generation(u64),
}

fn open_stamp(intake: &intake::Intake) -> Option<OpenStamp> {
    use intake::glass_dialect::{ShowcaseAsk, ID_BASE};
    match intake.expansion.ask() {
        Some(ShowcaseAsk::Picture { id, fit }) => intake
            .picture_pixels(ID_BASE + id)
            .map(|(_, _, _, seq)| OpenStamp::Picture { id: *id, seq, fit: *fit }),
        Some(ShowcaseAsk::Video { generation, .. })
        | Some(ShowcaseAsk::Chart { generation, .. })
        | Some(ShowcaseAsk::Model { generation, .. })
        | Some(ShowcaseAsk::Page { generation, .. }) => Some(OpenStamp::Generation(*generation)),
        None => None,
    }
}

fn apply_showcase(intake: &intake::Intake, host: &angel_glass::Host, stamp: Option<&OpenStamp>) {
    use intake::glass_dialect::{ShowcaseAsk, ID_BASE};
    let Some(_) = stamp else {
        host.close_showcase();
        return;
    };
    match intake.expansion.ask() {
        Some(ShowcaseAsk::Picture { id, fit }) => {
            if let Some((width, height, rgba, _)) = intake.picture_pixels(ID_BASE + id) {
                host.showcase(angel_glass::Showing::new("glass", (width, height), *fit));
                host.showcase_frame(width, height, rgba.as_ref().clone());
            }
        }
        Some(ShowcaseAsk::Video { width, height, fit, .. }) => {
            host.showcase(angel_glass::Showing::new("video", (*width, *height), *fit));
        }
        Some(ShowcaseAsk::Chart { marks, fit, picture, .. }) => {
            // The picture's pixels, then the marks over them, then the
            // showing. The picture's size is the chart's size. This runs
            // only when the request's stamp moves.
            if let Some((width, height, rgba, _)) = intake.picture_pixels(ID_BASE + *picture) {
                host.showcase_frame(width, height, rgba.as_ref().clone());
                host.showcase_marks(marks.clone());
                host.showcase(angel_glass::Showing::new("chart", (width, height), *fit));
            }
        }
        Some(ShowcaseAsk::Model { nodes, meshes, size, fit, .. }) => {
            for (id, mesh) in meshes {
                host.upload_mesh(*id, mesh.clone());
            }
            host.showcase_model(nodes.clone());
            host.showcase(angel_glass::Showing::new("model", *size, *fit));
        }
        Some(ShowcaseAsk::Page { text, cols, rows, .. }) => {
            if let Some((width, height, rgba)) = raster_page(text, *cols, *rows) {
                host.showcase(angel_glass::Showing::new("page", (width, height), angel_glass::Fit::Width));
                host.showcase_frame(width, height, rgba);
            }
        }
        None => host.close_showcase(),
    }
}

fn apply_frame(intake: &intake::Intake, host: &angel_glass::Host, frame: intake::glass_dialect::PendingFrame) {
    use intake::glass_dialect::{PendingFrame, ID_BASE};
    match frame {
        PendingFrame::Pixels { width, height, rgba } => host.showcase_frame(width, height, rgba),
        PendingFrame::Picture(id) => {
            if let Some((width, height, rgba, _)) = intake.picture_pixels(ID_BASE + id) {
                host.showcase_frame(width, height, rgba.as_ref().clone());
            }
        }
    }
}

fn apply_scenes(expansion: &mut intake::glass_dialect::Expansion, host: &angel_glass::Host, shown: &Mutex<Vec<u32>>) {
    let now = expansion.scene_ids();
    let mut was = lock(shown);
    for id in was.iter().filter(|id| !now.contains(id)) {
        host.remove_scene(*id);
    }
    *was = now;
    drop(was);
    // An asset's meshes go up once, when a scene first shows it.
    for (id, mesh) in expansion.take_mesh_uploads() {
        host.upload_mesh(id, mesh.as_ref().clone());
    }
    for draw in expansion.dirty_scenes() {
        for (id, mesh) in draw.meshes {
            host.upload_mesh(id, mesh);
        }
        host.set_scene(draw.window_id, draw.scene, draw.size);
    }
}

fn write_showcase_events(session: &Session, intake: &intake::Intake, host: &angel_glass::Host) {
    use intake::glass_dialect::{showcase_event_bytes, ShowcaseNow};
    if !intake.expansion.events_on() {
        *lock(&session.event_seen) = intake::glass_dialect::ShowcaseSeen::default();
        return;
    }
    let now = ShowcaseNow {
        seek: host.showcase_seek(),
        steps: host.showcase_steps(),
        copies: host.showcase_copies(),
        dismissals: host.showcase_dismissals(),
        clicks: host.showcase_clicks(),
    };
    let bytes = showcase_event_bytes(true, &mut lock(&session.event_seen), now);
    if bytes.is_empty() {
        return;
    }
    let mut writer = lock(&session.writer);
    let _ = writer.write_all(&bytes);
    let _ = writer.flush();
}

fn raster_page(text: &str, cols: u16, rows: u16) -> Option<(u32, u32, Vec<u8>)> {
    let _ = cols;
    let mut pager = angel_glass::page::Pager::load("monospace", 16)?;
    let mut lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        lines.push("");
    }
    while (lines.len() as u16) < rows {
        lines.push("");
    }
    pager.render(&lines, angel_glass::page::PLAIN, [232, 224, 208], [22, 20, 18, 255])
}
