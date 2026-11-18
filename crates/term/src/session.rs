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

const PICTURE_GROUP: u32 = 0x1e5c;

const DIALECT_GROUP: u32 = 0x1e5d;

const SYNC_LIMIT: Duration = Duration::from_millis(150);

const FRAME_WAIT: Duration = Duration::from_millis(50);

const FRAME_BEGIN: &[u8] = b"\x1b[?2026h";
const FRAME_END: &[u8] = b"\x1b[?2026l";

#[derive(Default)]
pub struct FrameGate {
    owed: Mutex<bool>,
    drawn: std::sync::Condvar,
}

impl FrameGate {
    fn finished(&self) {
        *lock(&self.owed) = true;
    }

    pub fn drawn(&self) {
        *lock(&self.owed) = false;
        self.drawn.notify_all();
    }

    fn wait(&self, limit: Duration) {
        let owed = lock(&self.owed);
        let _ = self.drawn.wait_timeout_while(owed, limit, |owed| *owed);
    }
}

fn find_escape(bytes: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    memchr::memmem::find(bytes.get(from..)?, needle).map(|p| from + p)
}

fn frame_ended(held: &mut usize, piece: &[u8]) -> bool {
    let n = FRAME_END.len();
    let take = piece.len().min(n - 1);
    let mut joined = [0u8; 14];
    joined[..*held].copy_from_slice(&FRAME_END[..*held]);
    joined[*held..*held + take].copy_from_slice(&piece[..take]);
    let joined = &joined[..*held + take];
    let ended = find_escape(joined, FRAME_END, 0).is_some() || find_escape(piece, FRAME_END, 0).is_some();
    let tail = if piece.len() >= n - 1 { &piece[piece.len() + 1 - n..] } else { joined };
    *held = (1..n).rev().find(|&k| tail.ends_with(&FRAME_END[..k])).unwrap_or(0);
    ended
}

fn frame_cuts(bytes: &[u8]) -> Vec<usize> {
    let mut cuts = Vec::new();
    let mut at = 1;
    while let Some(p) = find_escape(bytes, FRAME_BEGIN, at) {
        cuts.push(p);
        at = p + 1;
    }
    cuts
}

pub enum Msg {
    Input(Event),
    Output,
    Closed,
}

pub struct Pumped {
    pub replies: Vec<u8>,
    pub title: Option<String>,
    pub bell: bool,
    pub clipboard: Option<String>,
}

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
                let effects = engine.take_effects();
                replies.extend_from_slice(&effects.replies);
                title = effects.title.or(title);
                bell |= effects.bell;
                clipboard = effects.clipboard.or(clipboard);
                tell_the_store(engine, intake);
            }
            Piece::Media(media) => {
                i += 1;
                engine.set_motion(Some(u64::MAX));
                engine.settle();
                replies.extend_from_slice(&engine.take_effects().replies);
                tell_the_store(engine, intake);
                let mut adapter = Adapter { engine };
                intake.handle(media.clone(), &mut adapter);
                replies.extend_from_slice(&intake.take_replies());
                let effects = engine.take_effects();
                replies.extend_from_slice(&effects.replies);
                title = effects.title.or(title);
                bell |= effects.bell;
                clipboard = effects.clipboard.or(clipboard);
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

fn is_reset(text: &[u8]) -> bool {
    text.starts_with(b"\x1bc")
}

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

const FOREIGN_PREFIXES: [&str; 5] = ["KITTY_", "ALACRITTY_", "GHOSTTY_", "WEZTERM_", "KONSOLE_"];
const FOREIGN: [&str; 9] = [
    "WINDOWID",
    "TERMINFO",
    "VTE_VERSION",
    "TMUX",
    "TMUX_PANE",
    "STY",
    "ANGEL_GLASS",
    "ANGEL_GLASS_TRANSITION",
    "TERM_PROGRAM_VERSION",
];

pub struct Session {
    engine: Arc<Mutex<Engine>>,
    intake: Arc<Mutex<Intake>>,
    #[allow(dead_code)]
    splitter: Arc<Mutex<intake::Splitter>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    output: Arc<AtomicBool>,
    gate: Arc<FrameGate>,
    showcase_shown: Mutex<Option<OpenStamp>>,
    scene_ids: Mutex<Vec<u32>>,
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
        drop(pair.slave);

        let info = HostInfo {
            colors: [palette.foreground, palette.background, palette.cursor],
            cell_px: (grid.cell_width, grid.cell_height),
        };
        let engine = Arc::new(Mutex::new(Engine::new(usize::from(rows), usize::from(cols), info)));
        let writer: Arc<Mutex<Box<dyn Write + Send>>> =
            Arc::new(Mutex::new(pair.master.take_writer().map_err(|e| format!("pty writer: {e}"))?));
        let reader = pair.master.try_clone_reader().map_err(|e| format!("pty reader: {e}"))?;
        let mut reader = crate::read_ahead::ReadAhead::spawn(reader, pty_fd(pair.master.as_ref()))
            .map_err(|e| format!("pty reader: {e}"))?;
        let output = Arc::new(AtomicBool::new(false));
        let gate = Arc::new(FrameGate::default());
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
                let mut held = 0;
                'reads: loop {
                    let n = match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => n,
                    };
                    let mut starts = frame_cuts(&buf[..n]);
                    starts.push(n);
                    let mut from = 0;
                    for to in starts {
                        let piece = &buf[from..to];
                        from = to;
                        if piece.starts_with(FRAME_BEGIN) {
                            gate.wait(FRAME_WAIT);
                        }
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
                        let ended = frame_ended(&mut held, piece);
                        if ended {
                            gate.finished();
                        }
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

    pub fn write(&mut self, bytes: &[u8]) {
        let mut writer = lock(&self.writer);
        let _ = writer.write_all(bytes);
        let _ = writer.flush();
    }

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

    pub fn program_exited(&mut self) {
        lock(&self.intake).expansion.reset();
    }

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
        let engine = lock(&self.engine);
        let top = engine.top_line();
        let alternate = engine.alternate_screen();
        drop(engine);
        let view = intake::View { top, rows: area.height, alternate };
        let mut description = lock(&self.intake).describe(view);
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
        if !description.freed.is_empty() {
            host.free_pictures(description.freed);
        }
        {
            let mut intake = lock(&self.intake);
            let layers = intake.expansion.frame(top, area.height, alternate);
            host.set_layer_group(DIALECT_GROUP, layers);
            apply_scenes(&mut intake.expansion, host, &self.scene_ids);
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

    pub fn cursor_shape(&self) -> crate::view::CursorLook {
        lock(&self.engine).cursor_look()
    }

    pub fn cursor_blinking(&self) -> bool {
        lock(&self.engine).cursor_blinking()
    }

    pub fn cursor_shown(&self) -> bool {
        lock(&self.engine).cursor().is_some()
    }

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

    pub fn synchronized(&self) -> bool {
        lock(&self.engine).synchronized()
    }

    pub fn animation_deadline(&self) -> Option<u64> {
        lock(&self.intake).animation_deadline()
    }

    pub fn tick_animations(&self, now: u64) -> bool {
        lock(&self.intake).tick_animations(now)
    }

    pub fn touch_animation_clock(&self, now: u64) {
        lock(&self.intake).set_animation_clock(now);
    }

    pub fn set_display_frame(&self, ms: u64) {
        lock(&self.intake).set_display_frame(ms);
    }

    pub fn holding_frame(&self) -> bool {
        lock(&self.engine).holding_frame(SYNC_LIMIT)
    }

    pub fn end_sync_update(&mut self) -> bool {
        lock(&self.engine).end_sync_update()
    }

    pub fn link_at(&self, row: u16, col: u16) -> Option<String> {
        lock(&self.engine).link_at(row, col)
    }

    pub fn url_at(&self, row: u16, col: u16) -> Option<String> {
        let mut engine = lock(&self.engine);
        let cols = u16::try_from(engine.size().1).unwrap_or(u16::MAX);
        view::url_at(engine.cells(), cols, row, col)
    }

    pub fn link_or_url_at(&self, row: u16, col: u16) -> Option<String> {
        self.link_at(row, col).or_else(|| self.url_at(row, col))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
        }
    }
}

fn pty_fd(master: &dyn MasterPty) -> Option<std::os::fd::OwnedFd> {
    let fd = master.as_raw_fd()?;
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
        assert_eq!(frame_cuts(b"x\x1b[?20"), Vec::<usize>::new());
        assert_eq!(find_escape(b"ab\x1b[?2026l", FRAME_END, 0), Some(2));
        assert_eq!(find_escape(b"\x1b\x1b[?2026l", FRAME_END, 0), Some(1));
    }

    #[test]
    fn a_frame_end_split_across_reads_is_seen() {
        let stream = b"a\x1b[?2026\x1b[?2026lq\x1b[?2026h\x1b[?2026l\x1b[?2026";
        let check = |cuts: &[usize]| {
            let mut held = 0;
            for w in cuts.windows(2) {
                let ends = (w[0]..w[1]).any(|at| stream[..=at].ends_with(FRAME_END));
                assert_eq!(frame_ended(&mut held, &stream[w[0]..w[1]]), ends, "cuts {cuts:?}");
            }
        };
        for cut in 0..=stream.len() {
            for next in cut..=stream.len() {
                check(&[0, cut, next, stream.len()]);
            }
        }
        check(&(0..=stream.len()).collect::<Vec<_>>());
    }

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
        let started = Instant::now();
        gate.wait(Duration::from_secs(5));
        assert!(started.elapsed() < Duration::from_millis(100));
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

    const APC_PUT: &[u8] = b"\x1b_Gi=31,s=1,v=1,a=T,f=24;AAAA\x1b\\";

    #[test]
    fn text_keeps_its_clipboard_title_and_bell() {
        let (mut engine, mut intake, mut splitter) = rig();
        let p = pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b]52;c;aGk=\x07\x1b]2;one\x07\x07text");
        assert_eq!(p.clipboard.as_deref(), Some("hi"));
        assert_eq!(p.title.as_deref(), Some("one"));
        assert!(p.bell);
        let p = pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b]52;cp;eW8=\x07");
        assert_eq!(p.clipboard.as_deref(), Some("yo"));
        let mut stream = b"\x1b]52;c;aGk=\x07\x1b]2;one\x07".to_vec();
        stream.extend_from_slice(APC_PUT);
        stream.extend_from_slice(b"\x1b]52;c;eW8=\x07\x1b]2;two\x07");
        let p = pump_bytes(&mut engine, &mut intake, &mut splitter, &stream);
        assert_eq!(p.clipboard.as_deref(), Some("yo"));
        assert_eq!(p.title.as_deref(), Some("two"));
        assert!(!p.bell);
    }

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
                        format!("\x1b[{};{}Hwwwwwwwwwwww", 1 + rng.below(6), 8 + rng.below(2))
                            .into_bytes()
                    }
                    _ => {
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
            let rows = engine.size().0 as u16;
            let view = intake::View { top: engine.top_line(), rows, alternate: false };
            let placements = intake.describe(view).placements;
            let cells = engine.cells().to_vec();
            let cols = engine.size().1;
            for p in &placements {
                let _ = p;
            }
            let _ = (cells, cols, markers, rng);
        }
    }

    #[test]
    fn alternate_full_screen_scrolls_move_its_pictures() {
        let (mut engine, mut intake, mut splitter) = rig();
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[?1049h");
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[3;1H");
        pump_bytes(&mut engine, &mut intake, &mut splitter, APC_PUT);
        let at_put = shown(&mut intake, &mut engine, true);
        assert_eq!(at_put, vec![(2.0, 0.0)], "anchored at its text's row");
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
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"a\r\nb\r\n");
        pump_bytes(&mut engine, &mut intake, &mut splitter, APC_PUT);
        let at_put = shown(&mut intake, &mut engine, false);
        assert_eq!(at_put, vec![(2.0, 0.0)], "anchored at count + cursor row");
        for _ in 0..25 {
            pump_bytes(&mut engine, &mut intake, &mut splitter, b"aaaa\r\nbbbb\r\ncccc\r\ndddd\r\n");
        }
        assert_eq!(engine.scrolled_off_top(), 97, "the count is exact");
        assert!(shown(&mut intake, &mut engine, false).is_empty());
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
        assert_eq!(
            pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[?2026$p").replies,
            b"\x1b[?2026;2$y"
        );
        assert!(engine.holding_frame(Duration::from_millis(150)));
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"more");
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[?2026l");
        assert!(!engine.holding_frame(Duration::from_millis(150)));
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
        let d = pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[?1;1S\x1b[?25lSome text\x1b[c");
        assert_eq!(d.replies, b"\x1b[?1;0;1024S\x1b[?62;4;22c");
    }

    #[test]
    fn the_count_stays_exact_past_a_full_history() {
        let (mut engine, mut intake, mut splitter) = rig();
        pump_bytes(&mut engine, &mut intake, &mut splitter, APC_PUT);
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
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"marker\r\n");
        pump_bytes(&mut engine, &mut intake, &mut splitter, APC_PUT);
        for _ in 0..2 {
            pump_bytes(&mut engine, &mut intake, &mut splitter, b"fill\r\nfill\r\nfill\r\n");
        }
        let rows = engine.size().0 as u16;
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[H\x1b[2J");
        let on_screen =
            intake.describe(intake::View { top: engine.top_line(), rows, alternate: false });
        assert!(on_screen.placements.is_empty(), "the screen's pictures went");
        for _ in 0..4 {
            engine.scroll_display(6);
        }
        let back =
            intake.describe(intake::View { top: engine.top_line(), rows, alternate: false });
        assert!(
            !back.placements.is_empty(),
            "a picture stayed in history, aligned with its text"
        );
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

    #[test]
    fn in_cell_pictures_die_when_covered_and_apc_survives() {
        let (mut engine, mut intake, mut splitter) = rig();
        intake.set_cell((10, 20));
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1bPq#0~\x1b\\");
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\r\n$ ");
        let rows = engine.size().0 as u16;
        let sixel = intake.describe(intake::View { top: 0, rows, alternate: false });
        assert!(!sixel.placements.is_empty(), "the sixel was placed");
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[1;1HX");
        let covered = intake.describe(intake::View { top: 0, rows, alternate: false });
        assert!(covered.placements.is_empty(), "text over every cell removed the sixel");

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
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[1;1H\x1b[K");
        let after_el = intake.describe(intake::View { top: 0, rows, alternate: false });
        assert!(!after_el.placements.is_empty(), "an APC placement survives EL");
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

        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[?1049h");

        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b_Gi=32,s=1,v=1,a=T,f=24;AAAA\x1b\\");
        let alt = intake.describe(intake::View { top: 0, rows, alternate: true });
        assert!(!alt.placements.is_empty(), "the alternate screen holds its picture");
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[?1049l");
        let main =
            intake.describe(intake::View { top: engine.top_line(), rows, alternate: false });
        assert!(!main.placements.is_empty(), "the main screen's picture is still there");

        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b[?1049h");
        assert_eq!(engine.alt_scrolled(), 0, "the alternate count starts at 0");
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"one\r\ntwo\r\nthree\r\n");
        assert_eq!(engine.alt_scrolled(), 0, "scrolls that fit on the screen do not count");
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"four\r\nfive\r\nsix\r\nseven\r\n");
        assert_eq!(engine.alt_scrolled(), 2, "the alternate screen's count is exact");
    }

    fn tiny_picture(id: u32) -> Vec<u8> {
        format!("\x1b_Gi={id},s=1,v=1,a=T,f=24;AAAA\x1b\\").into_bytes()
    }

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

    #[test]
    fn a_picture_that_leaves_a_region_is_cropped() {
        let (mut engine, mut intake, mut splitter) = rig();
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
                intake
                    .pixels_of(crate::intake::Space::Osc, id)
                    .or_else(|| intake.pixels_of(crate::intake::Space::Apc, id))
                    .map(|(_, _, rgba, _)| (id, rgba.as_ref().clone()))
            })
            .collect()
    }

    #[test]
    fn a_file_transmission_ignores_chunking() {
        let (mut engine, mut intake, mut splitter) = rig();
        let bogus = b64_str(b"/no/such/glass-gap-file");
        let first = format!("\x1b_Ga=T,t=f,f=32,m=1;{bogus}\x1b\\");
        pump_bytes(&mut engine, &mut intake, &mut splitter, first.as_bytes());
        pump_bytes(&mut engine, &mut intake, &mut splitter, b"\x1b_Ga=T,f=24,s=1,v=1,q=2;AAAA\x1b\\");
        assert_eq!(shown(&mut intake, &mut engine, false).len(), 1, "m=1 on t=f must not swallow the next picture");
    }

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

    #[test]
    fn tiff_webp_and_bmp_decode_through_both_dialects() {
        let rgba = [255u8, 0, 0, 255, 0, 255, 0, 255];
        for kind in ["tiff", "webp", "bmp"] {
            let encoded = encode_still(kind, &rgba, 2, 1);
            let payload = b64_str(&encoded);

            let (mut engine, mut intake, mut splitter) = rig();
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
