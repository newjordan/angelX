//! The screen a program's output draws, kept by `alacritty_terminal`'s
//! `Term`, and the answers a terminal owes the program that `Term` leaves
//! to its host: identification and window queries, cell-pixel sizes, the
//! title, the bell and clipboard requests.
//!
//! Everything the rest of the crate needs from the terminal state goes
//! through this module: [`Engine::process`] bytes, [`Engine::resize`],
//! [`Engine::scroll_display`], the cells, cursor and modes, and the
//! replies, title, bell and clipboard the program's escape sequences ask
//! for, taken with [`Engine::take_effects`].
//!
//! A few queries are answered here rather than by `Term`, because glass
//! speaks for itself on them: what it is (DA1, DA2, XTVERSION), how big
//! its cells are (XTWINOPS 16), whether a synchronized update is open
//! (DECRQM 2026). Those byte sequences are recognized before the rest of
//! the stream reaches `Term`, so a sequence split across two reads is
//! held back until it is complete.

use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use alacritty_terminal::event::{Event as TermEvent, EventListener, WindowSize};
use alacritty_terminal::grid::{Dimensions, GridCell};
use alacritty_terminal::index::{Column, Point};
use alacritty_terminal::term::cell::Flags as CellFlags;
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{
    Attr, CharsetIndex, ClearMode, CursorStyle, Hyperlink, KeyboardModes, KeyboardModesApplyBehavior,
    LineClearMode, Mode, ModifyOtherKeys, PrivateMode, ScpCharPath, ScpUpdateMode, StandardCharset,
    NamedPrivateMode,
    TabulationClearMode,
};
use alacritty_terminal::vte::ansi::{Color as AnsiColor, CursorShape, NamedColor, Rgb};
use alacritty_terminal::vte::ansi::Processor;
use alacritty_terminal::index::{Line, Point as GridPoint, Side};
use alacritty_terminal::selection::{Selection as Sentinel, SelectionType};

use crate::keys::{MouseProtocolEncoding, MouseProtocolMode};
use crate::view::CursorLook;

const SCROLLBACK: usize = 10_000;
const SCROLL_COUNT_CHUNK: usize = 1024;
const SCROLL_COUNT_REPLANT: usize = 4_096;

fn fold_sentinel_line(count: &mut u64, read: &mut usize, alive: &mut bool, line: i32) -> bool {
    if line < 0 {
        let scrolled = line.unsigned_abs() as usize;
        if scrolled >= *read {
            *count += (scrolled - *read) as u64;
            *read = scrolled;
            *alive = true;
            scrolled >= SCROLL_COUNT_REPLANT
        } else {
            true
        }
    } else if line > 0 || *read > 0 {
        true
    } else {
        *alive = true;
        false
    }
}
const SYNC_DRAIN_CHUNK: usize = 1024;

#[derive(Default)]
pub struct Effects {
    pub replies: Vec<u8>,
    pub title: Option<String>,
    pub bell: bool,
    pub clipboard: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub c: char,
    pub fg: Color,
    pub bg: Color,
    pub flags: CellFlags,
    pub link: Option<String>,
    pub marks: [char; 3],
    pub fg_id: Option<u32>,
    pub underline_id: Option<u32>,
    pub underline: Color,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

fn simulated_after(
    before: &crate::intake::motion::GridSnap,
    rows: usize,
    cols: usize,
) -> crate::intake::motion::GridSnap {
    let mut logical: Vec<u32> = Vec::new();
    let mut acc = 0u32;
    for row in &before.lines {
        acc = acc.saturating_add(u32::from(row.len));
        if !row.wrap {
            logical.push(acc);
            acc = 0;
        }
    }
    if acc > 0 {
        logical.push(acc);
    }
    let mut lines = Vec::new();
    for chars in logical {
        if chars == 0 || cols == 0 {
            lines.push(crate::intake::motion::RowSnap { wrap: false, len: 0 });
            continue;
        }
        let mut left = chars;
        while left > cols as u32 {
            lines.push(crate::intake::motion::RowSnap { wrap: true, len: cols as u16 });
            left -= cols as u32;
        }
        lines.push(crate::intake::motion::RowSnap { wrap: false, len: left as u16 });
    }
    let screen = rows.max(1);
    let cap = screen.saturating_add(SCROLLBACK);
    if lines.len() > cap {
        let extra = lines.len() - cap;
        lines.drain(0..extra);
    }
    let history = lines.len().saturating_sub(screen);
    while lines.len() < screen {
        lines.push(crate::intake::motion::RowSnap { wrap: false, len: 0 });
    }
    crate::intake::motion::GridSnap {
        cols,
        screen,
        history,
        scrolled: before.scrolled,
        lines,
    }
}

fn color_of(color: AnsiColor) -> Color {
    match color {
        AnsiColor::Named(NamedColor::Foreground | NamedColor::Background) => Color::Default,
        AnsiColor::Named(named) => Color::Indexed(u8::try_from(named as usize).unwrap_or(255)),
        AnsiColor::Spec(Rgb { r, g, b }) => Color::Rgb(r, g, b),
        AnsiColor::Indexed(i) => Color::Indexed(i),
    }
}

pub fn color_id(color: Color) -> Option<u32> {
    match color {
        Color::Indexed(i) => Some(u32::from(i)),
        Color::Rgb(r, g, b) => Some((u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)),
        Color::Default => None,
    }
}

pub type Coord = (u16, u16);

#[derive(Clone, Copy)]
pub struct HostInfo {
    pub colors: [[u8; 3]; 3],
    pub cell_px: (u32, u32),
}

enum Query {
    Reply(Vec<u8>),
    Title(String),
    Bell,
    Clipboard(String),
    ColorRequest(usize),
    TextAreaSize(std::sync::Arc<dyn Fn(WindowSize) -> String + Send + Sync>),
}

struct Listener {
    tx: Sender<Query>,
}

impl EventListener for Listener {
    fn send_event(&self, event: TermEvent) {
        let query = match event {
            TermEvent::PtyWrite(text) => Query::Reply(text.into_bytes()),
            TermEvent::Title(title) => Query::Title(title),
            TermEvent::ResetTitle => Query::Title(String::new()),
            TermEvent::Bell => Query::Bell,
            TermEvent::ClipboardStore(_, text) => Query::Clipboard(text),
            TermEvent::ColorRequest(index, _) => Query::ColorRequest(index),
            TermEvent::TextAreaSizeRequest(size) => Query::TextAreaSize(size),
            _ => return,
        };
        let _ = self.tx.send(query);
    }
}

struct Size {
    columns: usize,
    lines: usize,
}

impl Dimensions for Size {
    fn columns(&self) -> usize {
        self.columns
    }

    fn screen_lines(&self) -> usize {
        self.lines
    }

    fn total_lines(&self) -> usize {
        self.lines
    }
}

pub struct Engine {
    term: Term<Listener>,
    processor: Processor,
    rx: Receiver<Query>,
    info: Arc<Mutex<HostInfo>>,
    tail: Vec<u8>,
    sync_since: Option<Instant>,
    scrolled_off_top: usize,
    sentinel_read: usize,
    sentinel_dropped: bool,
    hold_since: Option<Instant>,
    events: Vec<Event>,
    in_cell: Vec<(u64, u64, u16, u16)>,
    motion_end: Option<u64>,
    alt_scrolled: u64,
    region_top: i32,
    region_bottom: i32,
    primary_snap: Option<crate::intake::motion::GridSnap>,
    pending_reflow: Option<crate::intake::motion::Reflow>,
    sentinel_alive: bool,
    cells: Vec<Cell>,
    effects: Effects,
    joined: Vec<u8>,
    mirror: Mirror,
    fast: bool,
}

impl Engine {
    pub fn new(rows: usize, cols: usize, info: HostInfo) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let info = Arc::new(Mutex::new(info));
        let _ = &tx;
        let config = Config {
            scrolling_history: SCROLLBACK,
            kitty_keyboard: true,
            default_cursor_style: CursorStyle { shape: CursorShape::Block, blinking: false },
            ..Config::default()
        };
        let term = Term::new(
            config,
            &Size { columns: cols.max(2), lines: rows.max(1) },
            Listener { tx },
        );
        Self {
            term,
            processor: Processor::new(),
            rx,
            info,
            tail: Vec::new(),
            sync_since: None,
            scrolled_off_top: 0,
            sentinel_read: 0,
            sentinel_dropped: false,
            events: Vec::new(),
            in_cell: Vec::new(),
            motion_end: Some(u64::MAX),
            alt_scrolled: 0,
            region_top: 0,
            region_bottom: rows.max(1) as i32,
            primary_snap: None,
            pending_reflow: None,
            sentinel_alive: false,
            hold_since: None,
            cells: Vec::new(),
            effects: Effects::default(),
            joined: Vec::new(),
            mirror: Mirror::default(),
            fast: true,
        }
    }

    pub fn process(&mut self, bytes: &[u8]) {
        let mut joined;
        let bytes: &[u8] = if self.tail.is_empty() {
            bytes
        } else {
            joined = std::mem::take(&mut self.tail);
            joined.extend_from_slice(bytes);
            &joined
        };
        let mut pass = 0;
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i..].starts_with(b"\x1b[?2026h") && self.sync_since.is_none() {
                self.sync_since = Some(Instant::now());
                self.hold_since = self.sync_since;
            }
            if bytes[i..].starts_with(b"\x1b[?2026l") {
                self.hold_since = None;
            }
            if let Some((consumed, ask)) = intercepted(&bytes[i..]) {
                if pass < i {
                    self.apply(&bytes[pass..i]);
                    self.note_sync();
                }
                self.collect();
                let answer = self.answer(ask);
                self.effects.replies.extend_from_slice(&answer);
                i += consumed;
                pass = i;
                continue;
            }
            if let Some(kept) = held_back(&bytes[i..]) {
                self.tail = kept;
                break;
            }
            i = memchr::memchr(0x1b, &bytes[i + 1..]).map_or(bytes.len(), |p| i + 1 + p);
        }
        self.apply(&bytes[pass..i]);
        self.note_sync();
    }

    fn apply(&mut self, pass: &[u8]) {
        if !self.term.mode().contains(TermMode::ALT_SCREEN) && self.term.selection.is_none() {
            self.plant_sentinel();
        }
        for chunk in pass.chunks(SCROLL_COUNT_CHUNK) {
            let Engine {
                term,
                processor,
                in_cell,
                scrolled_off_top,
                events,
                alt_scrolled,
                region_top,
                region_bottom,
                mirror,
                fast,
                ..
            } = self;
            let alternate = term.mode().contains(TermMode::ALT_SCREEN);
            let scrolled = *scrolled_off_top as u64;
            let base = if alternate { *alt_scrolled } else { scrolled };
            let count = if alternate { 0 } else { scrolled };
            let mut watch = Watch {
                term,
                events: Vec::new(),
                in_cell: in_cell.clone(),
                base,
                count,
                alternate,
                alt_scrolled: *alt_scrolled,
                region_top: *region_top,
                region_bottom: *region_bottom,
                sentinel_read: self.sentinel_read,
                sentinel_alive: self.sentinel_alive,
                drained: chunk.len(),
                motion_end: self.motion_end,
                primary_snap: None,
                primary: scrolled,
                fast: *fast,
            };
            feed(processor, &mut watch, mirror, chunk, *fast);
            *alt_scrolled = watch.alt_scrolled;
            *region_top = watch.region_top;
            *region_bottom = watch.region_bottom;
            self.sentinel_read = watch.sentinel_read;
            self.sentinel_alive = watch.sentinel_alive;
            let primary = if watch.alternate { watch.primary } else { watch.count };
            self.scrolled_off_top = self.scrolled_off_top.max(primary as usize);
            let raised = std::mem::take(&mut watch.events);
            if !raised.is_empty() {
                events.extend(raised);
            }
            if let Some(snap) = watch.primary_snap.take() {
                self.primary_snap = Some(snap);
            }
            drop(watch);
            self.fold_sentinel();
        }
    }

    pub fn process_parts<'a>(&mut self, parts: impl IntoIterator<Item = &'a [u8]>) {
        let mut joined = std::mem::take(&mut self.joined);
        joined.clear();
        for part in parts {
            joined.extend_from_slice(part);
        }
        self.process(&joined);
        self.joined = joined;
    }

    fn note_sync(&mut self) {
        let open = self.processor.sync_timeout().sync_timeout().is_some();
        if open && self.sync_since.is_none() {
            self.sync_since = Some(Instant::now());
        } else if !open && self.sync_since.is_some() {
            self.sync_since = None;
        }
    }

    fn plant_sentinel(&mut self) {
        let column = self.term.last_column();
        let top = GridPoint::new(Line(0), Column(0));
        let mut sentinel = Sentinel::new(SelectionType::Simple, top, Side::Left);
        sentinel.update(GridPoint::new(Line(0), column), Side::Right);
        self.term.selection = Some(sentinel);
        self.sentinel_read = 0;
        self.sentinel_alive = true;
    }

    fn fold_sentinel(&mut self) {
        if self.term.mode().contains(TermMode::ALT_SCREEN) {
            self.term.selection = None;
            self.sentinel_dropped = true;
            self.sentinel_alive = false;
            return;
        }
        if let Some(range) = self
            .term
            .selection
            .as_ref()
            .and_then(|s| s.to_range(&self.term))
        {
            let mut count = self.scrolled_off_top as u64;
            let plant = fold_sentinel_line(
                &mut count,
                &mut self.sentinel_read,
                &mut self.sentinel_alive,
                range.start.line.0,
            );
            self.scrolled_off_top = count as usize;
            if plant {
                self.plant_sentinel();
            }
        } else {
            self.plant_sentinel();
        }
    }

    pub fn settle(&mut self) {
        if !self.tail.is_empty() {
            let tail = std::mem::take(&mut self.tail);
            self.process(&tail);
        }
        if self.sync_since.is_some() {
            self.stop_sync_and_count();
        }
    }

    fn stop_sync_and_count(&mut self) {
        if self.sync_since.is_some() {
            let Engine {
                term,
                processor,
                in_cell,
                scrolled_off_top,
                events,
                alt_scrolled,
                region_top,
                region_bottom,
                sentinel_read,
                sentinel_alive,
                ..
            } = self;
            let alternate = term.mode().contains(TermMode::ALT_SCREEN);
            let scrolled = *scrolled_off_top as u64;
            let base = if alternate { *alt_scrolled } else { scrolled };
            let count = if alternate { 0 } else { scrolled };
            let mut watch = Watch {
                term,
                events: Vec::new(),
                in_cell: in_cell.clone(),
                base,
                count,
                alternate,
                alt_scrolled: *alt_scrolled,
                region_top: *region_top,
                region_bottom: *region_bottom,
                sentinel_read: *sentinel_read,
                sentinel_alive: *sentinel_alive,
                drained: 0,
                motion_end: self.motion_end,
                primary_snap: None,
                primary: scrolled,
                fast: self.fast,
            };
            processor.stop_sync(&mut watch);
            self.sentinel_read = watch.sentinel_read;
            self.sentinel_alive = watch.sentinel_alive;
            let primary = if watch.alternate { watch.primary } else { watch.count };
            self.scrolled_off_top = self.scrolled_off_top.max(primary as usize);
            *alt_scrolled = watch.alt_scrolled;
            *region_top = watch.region_top;
            *region_bottom = watch.region_bottom;
            let raised = std::mem::take(&mut watch.events);
            if let Some(snap) = watch.primary_snap.take() {
                self.primary_snap = Some(snap);
            }
            drop(watch);
            if !raised.is_empty() {
                events.extend(raised);
            }
            self.fold_sentinel();
            self.sync_since = None;
        }
    }

    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    pub fn set_in_cell(&mut self, areas: Vec<(u64, u64, u16, u16)>) {
        self.in_cell = areas;
    }

    pub fn set_motion(&mut self, end: Option<u64>) {
        self.motion_end = end;
    }

    pub fn grid_cursor(&self) -> (u16, u16) {
        let point = self.term.grid().cursor.point;
        (point.line.0.max(0) as u16, point.column.0 as u16)
    }

    pub fn alt_scrolled(&self) -> u64 {
        self.alt_scrolled
    }

    pub fn history_size(&self) -> usize {
        self.term.history_size()
    }

    pub fn top_line(&self) -> u64 {
        let offset = self.term.grid().display_offset() as u64;
        let scrolled = if self.alternate_screen() {
            self.alt_scrolled
        } else {
            self.scrolled_off_top as u64
        };
        scrolled.saturating_sub(offset)
    }

    pub fn take_effects(&mut self) -> Effects {
        self.collect();
        std::mem::take(&mut self.effects)
    }

    fn collect(&mut self) {
        let effects = &mut self.effects;
        loop {
            match self.rx.try_recv() {
                Ok(Query::Reply(bytes)) => effects.replies.extend_from_slice(&bytes),
                Ok(Query::Title(title)) => effects.title = Some(title),
                Ok(Query::Bell) => effects.bell = true,
                Ok(Query::Clipboard(text)) => effects.clipboard = Some(text),
                Ok(Query::ColorRequest(index)) => {
                    let which = match index {
                        256 => 10,
                        257 => 11,
                        258 => 12,
                        _ => continue,
                    };
                    let [r, g, b] = self.info.lock().unwrap_or_else(|e| e.into_inner()).colors
                        [(which - 10) as usize];
                    effects.replies.extend_from_slice(
                        format!(
                            "\x1b]{which};rgb:{r:02x}{r:02x}/{g:02x}{g:02x}/{b:02x}{b:02x}\x1b\\"
                        )
                        .as_bytes(),
                    );
                }
                Ok(Query::TextAreaSize(area)) => {
                    let info = self.info.lock().unwrap_or_else(|e| e.into_inner());
                    let size = WindowSize {
                        num_lines: u16::try_from(self.term.screen_lines()).unwrap_or(u16::MAX),
                        num_cols: u16::try_from(self.term.columns()).unwrap_or(u16::MAX),
                        cell_width: u16::try_from(info.cell_px.0).unwrap_or(u16::MAX),
                        cell_height: u16::try_from(info.cell_px.1).unwrap_or(u16::MAX),
                    };
                    drop(info);
                    effects.replies.extend_from_slice(area(size).as_bytes());
                }
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
    }

    fn answer(&self, ask: Ask) -> Vec<u8> {
        match ask {
            Ask::CellSize => {
                let (cw, ch) = self.cell_px();
                format!("\x1b[6;{ch};{cw}t").into_bytes()
            }
            Ask::Primary => b"\x1b[?62;4;22c".to_vec(),
            Ask::Graphics(pi) => match pi {
                1 => b"\x1b[?1;0;1024S".to_vec(),
                2 => {
                    let (cw, ch) = self.cell_px();
                    let width = (self.term.columns() * cw.max(1) as usize).min(8192);
                    let height = (self.term.screen_lines() * ch.max(1) as usize).min(8192);
                    format!("\x1b[?2;0;{width};{height}S").into_bytes()
                }
                other => format!("\x1b[?{other};1;0S").into_bytes(),
            },
            Ask::Secondary => b"\x1b[>1;10;0c".to_vec(),
            Ask::Version => format!("\x1bP>|glass {}\x1b\\", env!("CARGO_PKG_VERSION")).into_bytes(),
            Ask::Synchronized => {
                let state = if self.sync_since.is_some() { 1 } else { 2 };
                format!("\x1b[?2026;{state}$y").into_bytes()
            }
        }
    }

    pub fn resize(&mut self, rows: usize, cols: usize) {
        let rows = rows.max(1);
        let cols = cols.max(2);
        let old_cols = self.term.columns();
        let on_alt = self.term.mode().contains(TermMode::ALT_SCREEN);
        let before = if old_cols != cols {
            if on_alt { self.primary_snap.clone() } else { Some(self.snap_grid()) }
        } else {
            None
        };
        self.term.resize(Size { columns: cols, lines: rows });
        self.region_top = 0;
        self.region_bottom = rows as i32;
        if let Some(before) = before {
            let after = if on_alt {
                simulated_after(&before, rows, cols)
            } else {
                self.snap_grid()
            };
            let mut reflow = crate::intake::motion::reflow_of(&before, &after);
            reflow.new_scrolled = crate::intake::motion::scrolled_after(&reflow);
            self.scrolled_off_top = reflow.new_scrolled as usize;
            self.pending_reflow = Some(reflow);
        }
        self.fold_sentinel();
        if self.term.mode().contains(TermMode::ALT_SCREEN) {
            if let Some(reflow) = self.pending_reflow.as_ref() {
                self.primary_snap = Some(crate::intake::motion::GridSnap {
                    cols: reflow.new_cols,
                    screen: rows,
                    history: reflow.new_history,
                    scrolled: self.scrolled_off_top as u64,
                    lines: reflow.new_rows.clone(),
                });
            }
        } else {
            self.primary_snap = Some(self.snap_grid());
        }
    }

    pub fn take_reflow(&mut self) -> Option<crate::intake::motion::Reflow> {
        self.pending_reflow.take()
    }

    fn snap_grid(&self) -> crate::intake::motion::GridSnap {
        snap_term(&self.term, self.scrolled_off_top as u64)
    }

    pub fn set_cell_px(&mut self, cell_px: (u32, u32)) {
        self.info.lock().unwrap_or_else(|e| e.into_inner()).cell_px = cell_px;
    }

    pub fn cell_px(&self) -> (u32, u32) {
        self.info.lock().unwrap_or_else(|e| e.into_inner()).cell_px
    }

    pub fn scroll_display(&mut self, lines: isize) -> bool {
        let before = self.display_offset();
        self.term.scroll_display(Scroll::Delta(i32::try_from(lines).unwrap_or(0)));
        self.display_offset() != before
    }

    pub fn scroll_to_bottom(&mut self) -> bool {
        let before = self.display_offset();
        self.term.scroll_display(Scroll::Bottom);
        self.display_offset() != before
    }

    fn display_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    pub fn size(&self) -> (usize, usize) {
        (self.term.screen_lines(), self.term.columns())
    }

    pub fn cursor(&self) -> Option<Coord> {
        if self.term.cursor_style().shape == CursorShape::Hidden {
            return None;
        }
        if !self.term.mode().contains(TermMode::SHOW_CURSOR) || self.display_offset() != 0 {
            return None;
        }
        let point = self.term.grid().cursor.point;
        let row = point.line.0;
        (0..self.term.screen_lines() as i32).contains(&row).then(|| {
            (
                u16::try_from(row).unwrap_or(0),
                u16::try_from(point.column.0).unwrap_or(0),
            )
        })
    }

    pub fn cursor_blinking(&self) -> bool {
        self.term.cursor_style().blinking
    }

    pub fn cursor_look(&self) -> CursorLook {
        match self.term.cursor_style().shape {
            CursorShape::Block | CursorShape::HollowBlock => CursorLook::Block,
            CursorShape::Underline => CursorLook::Underline,
            CursorShape::Beam => CursorLook::Beam,
            CursorShape::Hidden => CursorLook::Hidden,
        }
    }

    pub fn cursor_color(&self) -> Option<Rgb> {
        self.term.colors()[NamedColor::Cursor as usize]
    }

    pub fn keyboard_flags(&self) -> u8 {
        let mode = self.term.mode();
        let mut flags = 0;
        flags |= u8::from(mode.contains(TermMode::DISAMBIGUATE_ESC_CODES));
        flags |= u8::from(mode.contains(TermMode::REPORT_EVENT_TYPES)) << 1;
        flags |= u8::from(mode.contains(TermMode::REPORT_ALTERNATE_KEYS)) << 2;
        flags |= u8::from(mode.contains(TermMode::REPORT_ALL_KEYS_AS_ESC)) << 3;
        flags |= u8::from(mode.contains(TermMode::REPORT_ASSOCIATED_TEXT)) << 4;
        flags
    }

    pub fn alternate_screen(&self) -> bool {
        self.term.mode().contains(TermMode::ALT_SCREEN)
    }

    pub fn application_cursor(&self) -> bool {
        self.term.mode().contains(TermMode::APP_CURSOR)
    }

    pub fn bracketed_paste(&self) -> bool {
        self.term.mode().contains(TermMode::BRACKETED_PASTE)
    }

    pub fn focus_reports(&self) -> bool {
        self.term.mode().contains(TermMode::FOCUS_IN_OUT)
    }

    pub fn mouse_protocol(&self) -> (MouseProtocolMode, MouseProtocolEncoding) {
        let mode = self.term.mode();
        let protocol = if mode.contains(TermMode::MOUSE_MOTION) {
            MouseProtocolMode::AnyMotion
        } else if mode.contains(TermMode::MOUSE_DRAG) {
            MouseProtocolMode::ButtonMotion
        } else if mode.contains(TermMode::MOUSE_REPORT_CLICK) {
            MouseProtocolMode::PressRelease
        } else {
            MouseProtocolMode::None
        };
        let encoding = if mode.contains(TermMode::SGR_MOUSE) {
            MouseProtocolEncoding::Sgr
        } else if mode.contains(TermMode::UTF8_MOUSE) {
            MouseProtocolEncoding::Utf8
        } else {
            MouseProtocolEncoding::Default
        };
        (protocol, encoding)
    }

    pub fn synchronized(&self) -> bool {
        self.sync_since.is_some()
    }

    pub fn end_sync_update(&mut self) -> bool {
        let deadline = self.processor.sync_timeout().sync_timeout();
        let Some(deadline) = deadline else {
            return false;
        };
        if Instant::now() < deadline {
            return false;
        }
        self.stop_sync_and_count();
        true
    }

    pub fn holding_frame(&self, limit: std::time::Duration) -> bool {
        self.hold_since.is_some_and(|since| since.elapsed() < limit)
    }

    #[allow(dead_code)]
    pub fn scrolled_off_top(&self) -> usize {
        self.scrolled_off_top
    }

    #[cfg(test)]
    pub fn grid_line(&self, line: i32) -> String {
        use alacritty_terminal::index::{Column, Line};
        let cols = self.term.columns();
        let row = &self.term.grid()[Line(line)];
        let mut text = String::with_capacity(cols);
        for c in 0..cols {
            let ch = row[Column(c)].c;
            text.push(if ch == '\0' { ' ' } else { ch });
        }
        text
    }

    #[cfg(test)]
    pub fn set_fast(&mut self, on: bool) {
        self.fast = on;
    }

    #[cfg(test)]
    pub fn grid_state(&self) -> (Vec<String>, String, usize, usize) {
        use alacritty_terminal::index::Line;
        use alacritty_terminal::term::cell::Cell as TermCell;
        let key = |cell: &TermCell| {
            let link = cell.hyperlink().map(|link| {
                let id = link.id().to_owned();
                (if id.ends_with("_alacritty") { String::new() } else { id }, link.uri().to_owned())
            });
            format!(
                "{:?} {:?} {:?} {:?} {:?} {:?} {:?}",
                cell.c,
                cell.fg,
                cell.bg,
                cell.flags,
                cell.zerowidth(),
                cell.underline_color(),
                link
            )
        };
        let grid = self.term.grid();
        let history = self.term.history_size();
        let mut cells = Vec::new();
        for line in -(history as i32)..self.term.screen_lines() as i32 {
            for column in 0..self.term.columns() {
                cells.push(key(&grid[Line(line)][Column(column)]));
            }
        }
        let cursor = format!(
            "{:?} {} {} {:?} {:?}",
            grid.cursor.point,
            grid.cursor.input_needs_wrap,
            key(&grid.cursor.template),
            grid.cursor.charsets,
            self.term.mode()
        );
        (cells, cursor, history, self.scrolled_off_top)
    }

    pub fn cells(&mut self) -> &[Cell] {
        let grid = self.term.grid();
        let offset = grid.display_offset();
        let (rows, cols) = (grid.screen_lines(), grid.columns());
        self.cells.clear();
        self.cells.reserve(rows * cols);
        for row in 0..rows {
            let line = alacritty_terminal::term::viewport_to_point(offset, Point::new(row, Column(0))).line;
            let cells = &grid[line];
            for col in 0..cols {
                let cell = &cells[Column(col)];
                let c = if cell.flags.intersects(
                    CellFlags::WIDE_CHAR_SPACER | CellFlags::LEADING_WIDE_CHAR_SPACER,
                ) {
                    '\0'
                } else {
                    cell.c
                };
                let fg = color_of(cell.fg);
                let mut marks = ['\0'; 3];
                if let Some(extra) = cell.zerowidth() {
                    for (i, ch) in extra.iter().take(3).enumerate() {
                        marks[i] = *ch;
                    }
                }
                let underline = cell.underline_color().map(color_of).unwrap_or(Color::Default);
                self.cells.push(Cell {
                    c,
                    fg,
                    bg: color_of(cell.bg),
                    flags: cell.flags,
                    link: cell.hyperlink().map(|link| link.uri().to_owned()),
                    marks,
                    fg_id: color_id(fg),
                    underline_id: color_id(underline),
                    underline,
                });
            }
        }
        &self.cells
    }

    pub fn link_at(&self, row: u16, col: u16) -> Option<String> {
        if usize::from(row) >= self.term.screen_lines()
            || usize::from(col) >= self.term.columns()
        {
            return None;
        }
        let grid = self.term.grid();
        let point = alacritty_terminal::term::viewport_to_point(
            grid.display_offset(),
            Point::new(usize::from(row), Column(usize::from(col))),
        );
        grid[point.line][point.column].hyperlink().map(|link| link.uri().to_owned())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoveredArea {
    pub line: u64,
    pub rows: usize,
    pub column: u16,
    pub columns: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Clear { alternate: bool, scrolled: u64 },
    ClearSaved,
    Reset,
    Covered { alternate: bool, area: CoveredArea },
    RegionScroll {
        alternate: bool,
        top: u64,
        bottom: u64,
        up: u64,
        down: u64,
        count_delta: u64,
        keep_above: bool,
        clears: bool,
    },
}

fn snap_term(term: &Term<Listener>, scrolled: u64) -> crate::intake::motion::GridSnap {
    use alacritty_terminal::index::{Column, Line};
    use alacritty_terminal::term::cell::Flags;
    let cols = term.columns();
    let screen = term.screen_lines();
    let history = term.history_size();
    let grid = term.grid();
    let mut lines = Vec::with_capacity(history + screen);
    for i in -(history as i32)..(screen as i32) {
        let row = &grid[Line(i)];
        let wrap = cols > 0 && row[Column(cols - 1)].flags.contains(Flags::WRAPLINE);
        let mut len = 0u16;
        if wrap {
            len = cols as u16;
        } else {
            for c in 0..cols {
                if !row[Column(c)].is_empty() {
                    len = (c as u16) + 1;
                }
            }
        }
        lines.push(crate::intake::motion::RowSnap { wrap, len });
    }
    crate::intake::motion::GridSnap {
        cols,
        screen,
        history,
        scrolled,
        lines,
    }
}

struct Watch<'a> {
    term: &'a mut Term<Listener>,
    events: Vec<Event>,
    in_cell: Vec<(u64, u64, u16, u16)>,
    base: u64,
    count: u64,
    alternate: bool,
    alt_scrolled: u64,
    region_top: i32,
    region_bottom: i32,
    sentinel_read: usize,
    sentinel_alive: bool,
    drained: usize,
    motion_end: Option<u64>,
    primary_snap: Option<crate::intake::motion::GridSnap>,
    primary: u64,
    fast: bool,
}

impl Watch<'_> {
    #[allow(dead_code)]
    fn over<'a>(engine: &'a mut Engine, _chunk: &[u8]) -> Watch<'a> {
        let alternate = engine.term.mode().contains(TermMode::ALT_SCREEN);
        let scrolled = engine.scrolled_off_top as u64;
        let base = if alternate { engine.alt_scrolled } else { scrolled };
        let count = if alternate { 0 } else { scrolled };
        Watch {
            term: &mut engine.term,
            events: Vec::new(),
            in_cell: engine.in_cell.clone(),
            base,
            count,
            alternate,
            alt_scrolled: engine.alt_scrolled,
            region_top: engine.region_top,
            region_bottom: engine.region_bottom,
            sentinel_read: engine.sentinel_read,
            sentinel_alive: engine.sentinel_alive,
            drained: 0,
            motion_end: engine.motion_end,
            primary_snap: None,
            primary: scrolled,
            fast: engine.fast,
        }
    }

    fn plant_sentinel(&mut self) {
        let column = self.term.last_column();
        let top = GridPoint::new(Line(0), Column(0));
        let mut sentinel = Sentinel::new(SelectionType::Simple, top, Side::Left);
        sentinel.update(GridPoint::new(Line(0), column), Side::Right);
        self.term.selection = Some(sentinel);
        self.sentinel_read = 0;
        self.sentinel_alive = true;
    }

    fn replant(&mut self) {
        self.plant_sentinel();
    }

    fn fold_sentinel(&mut self) {
        if self.term.mode().contains(TermMode::ALT_SCREEN) {
            self.term.selection = None;
            self.sentinel_alive = false;
            return;
        }
        if let Some(range) = self
            .term
            .selection
            .as_ref()
            .and_then(|s| s.to_range(self.term))
        {
            let plant = fold_sentinel_line(
                &mut self.count,
                &mut self.sentinel_read,
                &mut self.sentinel_alive,
                range.start.line.0,
            );
            if plant {
                self.plant_sentinel();
            }
        } else {
            self.plant_sentinel();
        }
        self.base = self.count;
    }

    fn line_of(&self, row: i32) -> u64 {
        self.base + row.max(0) as u64
    }

    fn covers(&mut self, start_row: i32, rows: usize, column: usize, columns: usize) {
        if self.in_cell.is_empty() || rows == 0 || columns == 0 {
            return;
        }
        let cols_total = self.term.columns();
        let columns = columns.min(cols_total.saturating_sub(column));
        let line = self.line_of(start_row);
        let hits = self.in_cell.iter().any(|(l0, l1, c0, c1)| {
            line <= *l1
                && line + rows as u64 > *l0
                && column as u16 <= *c1
                && column as u16 + columns as u16 > *c0
        });
        if hits {
            self.events.push(Event::Covered {
                alternate: self.alternate,
                area: CoveredArea {
                    line,
                    rows,
                    column: column as u16,
                    columns,
                },
            });
        }
    }

    #[inline(always)]
    fn print_plain(&mut self, c: char) -> bool {
        if !self.fast || !self.in_cell.is_empty() || self.term.mode().contains(TermMode::INSERT) {
            return false;
        }
        if !(' '..='~').contains(&c) {
            return self.print_wide(c);
        }
        let columns = self.term.columns();
        let grid = self.term.grid_mut();
        let cursor = &grid.cursor;
        if cursor.input_needs_wrap || cursor.template.extra.is_some() || cursor.charsets != Default::default() {
            return false;
        }
        let point = cursor.point;
        let (fg, bg, flags) = (cursor.template.fg, cursor.template.bg, cursor.template.flags);
        let cell = &mut grid[point.line][point.column];
        if cell.flags.intersects(CellFlags::WIDE_CHAR | CellFlags::WIDE_CHAR_SPACER) {
            return false;
        }
        cell.c = c;
        cell.fg = fg;
        cell.bg = bg;
        cell.flags = flags;
        cell.extra = None;
        if point.column.0 + 1 < columns {
            grid.cursor.point.column.0 += 1;
        } else {
            grid.cursor.input_needs_wrap = true;
        }
        self.drained += 1;
        if self.drained >= SYNC_DRAIN_CHUNK {
            self.fold_sentinel();
            self.drained = 0;
        }
        true
    }

    #[inline(never)]
    fn print_wide(&mut self, c: char) -> bool {
        if c < '\u{a0}' {
            return false;
        }
        let width = match unicode_width::UnicodeWidthChar::width(c) {
            Some(width @ 1..=2) => width,
            _ => return false,
        };
        let columns = self.term.columns();
        let grid = self.term.grid_mut();
        let cursor = &grid.cursor;
        let wide = CellFlags::WIDE_CHAR | CellFlags::WIDE_CHAR_SPACER | CellFlags::LEADING_WIDE_CHAR_SPACER;
        if cursor.input_needs_wrap || cursor.template.extra.is_some() || cursor.template.flags.intersects(wide) {
            return false;
        }
        let point = cursor.point;
        let column = point.column.0;
        if column + width > columns {
            return false;
        }
        let (fg, bg, flags) = (cursor.template.fg, cursor.template.bg, cursor.template.flags);
        let row = &mut grid[point.line];
        let underneath = CellFlags::WIDE_CHAR | CellFlags::WIDE_CHAR_SPACER;
        if row[Column(column)].flags.intersects(underneath)
            || (width == 2 && row[Column(column + 1)].flags.intersects(underneath))
        {
            return false;
        }
        let cell = &mut row[Column(column)];
        cell.c = c;
        cell.fg = fg;
        cell.bg = bg;
        cell.flags = if width == 2 { flags | CellFlags::WIDE_CHAR } else { flags };
        cell.extra = None;
        if width == 2 {
            let spacer = &mut row[Column(column + 1)];
            spacer.c = ' ';
            spacer.fg = fg;
            spacer.bg = bg;
            spacer.flags = flags | CellFlags::WIDE_CHAR_SPACER;
            spacer.extra = None;
        }
        let last = column + width - 1;
        if last + 1 < columns {
            grid.cursor.point.column = Column(last + 1);
        } else {
            grid.cursor.point.column = Column(last);
            grid.cursor.input_needs_wrap = true;
        }
        self.drained += 1;
        if self.drained >= SYNC_DRAIN_CHUNK {
            self.fold_sentinel();
            self.drained = 0;
        }
        true
    }

    fn print_run(&mut self, run: &[u8]) -> usize {
        if run.is_empty() || !self.in_cell.is_empty() || self.term.mode().contains(TermMode::INSERT) {
            return 0;
        }
        {
            let cursor = &self.term.grid().cursor;
            if cursor.template.extra.is_some() || cursor.charsets != Default::default() {
                return 0;
            }
        }
        let columns = self.term.columns();
        let mut taken = 0;
        while taken < run.len() {
            let grid = self.term.grid_mut();
            if grid.cursor.input_needs_wrap {
                alacritty_terminal::vte::ansi::Handler::input(self, char::from(run[taken]));
                taken += 1;
                continue;
            }
            let point = grid.cursor.point;
            let template = &grid.cursor.template;
            let (fg, bg, flags) = (template.fg, template.bg, template.flags);
            let column = point.column.0.min(columns - 1);
            let n = (columns - column).min(run.len() - taken);
            let cells = &mut grid[point.line][Column(column)..Column(column + n)];
            let mut written = 0;
            for (cell, &b) in cells.iter_mut().zip(&run[taken..taken + n]) {
                if cell.flags.intersects(CellFlags::WIDE_CHAR | CellFlags::WIDE_CHAR_SPACER) {
                    break;
                }
                cell.c = char::from(b);
                cell.fg = fg;
                cell.bg = bg;
                cell.flags = flags;
                cell.extra = None;
                written += 1;
            }
            if written > 0 {
                let end = column + written;
                if end < columns {
                    grid.cursor.point.column = Column(end);
                } else {
                    grid.cursor.point.column = Column(columns - 1);
                    grid.cursor.input_needs_wrap = true;
                }
                taken += written;
                self.drained += written;
            }
            if written < n {
                alacritty_terminal::vte::ansi::Handler::input(self, char::from(run[taken]));
                taken += 1;
            }
        }
        if self.drained >= SYNC_DRAIN_CHUNK {
            self.fold_sentinel();
            self.drained = 0;
        }
        taken
    }

    fn print_utf8(&mut self, text: &[u8]) -> usize {
        let plain = self.fast && self.in_cell.is_empty() && !self.term.mode().contains(TermMode::INSERT) && {
            let template = &self.term.grid().cursor.template;
            template.extra.is_none()
                && !template.flags.intersects(CellFlags::WIDE_CHAR | CellFlags::WIDE_CHAR_SPACER | CellFlags::LEADING_WIDE_CHAR_SPACER)
        };
        let ascii = plain && self.term.grid().cursor.charsets == Default::default();
        let mut at = 0;
        let mut held: Option<(char, usize)> = None;
        while let Some((c, len)) = utf8_at(text, at) {
            if ('\u{7f}'..='\u{9f}').contains(&c) {
                break;
            }
            if let Some((last, _)) = held
                && !self.put_plain(last, plain, ascii)
            {
                alacritty_terminal::vte::ansi::Handler::input(self, last);
            }
            held = Some((c, at));
            at += len;
        }
        if self.drained >= SYNC_DRAIN_CHUNK {
            self.fold_sentinel();
            self.drained = 0;
        }
        held.map_or(0, |(_, start)| start)
    }

    #[inline(always)]
    fn put_plain(&mut self, c: char, plain: bool, ascii: bool) -> bool {
        let width = if (' '..='~').contains(&c) {
            if !ascii {
                return false;
            }
            1
        } else {
            if !plain {
                return false;
            }
            match unicode_width::UnicodeWidthChar::width(c) {
                Some(width @ 1..=2) => width,
                _ => return false,
            }
        };
        let columns = self.term.columns();
        let grid = self.term.grid_mut();
        let cursor = &grid.cursor;
        let column = cursor.point.column.0;
        if cursor.input_needs_wrap || column + width > columns {
            return false;
        }
        let line = cursor.point.line;
        let (fg, bg, flags) = (cursor.template.fg, cursor.template.bg, cursor.template.flags);
        let row = &mut grid[line];
        let underneath = CellFlags::WIDE_CHAR | CellFlags::WIDE_CHAR_SPACER;
        if row[Column(column)].flags.intersects(underneath)
            || (width == 2 && row[Column(column + 1)].flags.intersects(underneath))
        {
            return false;
        }
        let cell = &mut row[Column(column)];
        cell.c = c;
        cell.fg = fg;
        cell.bg = bg;
        cell.flags = if width == 2 { flags | CellFlags::WIDE_CHAR } else { flags };
        cell.extra = None;
        if width == 2 {
            let spacer = &mut row[Column(column + 1)];
            spacer.c = ' ';
            spacer.fg = fg;
            spacer.bg = bg;
            spacer.flags = flags | CellFlags::WIDE_CHAR_SPACER;
            spacer.extra = None;
        }
        let last = column + width - 1;
        if last + 1 < columns {
            grid.cursor.point.column = Column(last + 1);
        } else {
            grid.cursor.point.column = Column(last);
            grid.cursor.input_needs_wrap = true;
        }
        self.drained += 1;
        true
    }

    #[inline]
    fn warm_next_row(&mut self) {
        #[cfg(target_arch = "x86_64")]
        {
            let history = self.term.history_size();
            if history == 0 {
                return;
            }
            let grid = self.term.grid();
            let row = &grid[Line(-(history as i32))];
            let cells = &row[..Column(grid.columns())];
            let bytes = std::mem::size_of_val(cells);
            let start = cells.as_ptr().cast::<i8>();
            for offset in (0..bytes).step_by(64) {
                unsafe {
                    std::arch::x86_64::_mm_prefetch::<{ std::arch::x86_64::_MM_HINT_ET0 }>(start.wrapping_add(offset));
                }
            }
        }
    }

    fn back_to_primary(&mut self) {
        self.alternate = false;
        self.count = self.primary;
        self.base = self.count;
        self.plant_sentinel();
    }

    fn fold_around_down(&mut self) {
        if !self.alternate {
            self.fold_sentinel();
        }
    }

    fn note_region(&mut self, top: usize, bottom: Option<usize>) {
        let screen = self.term.screen_lines();
        let bottom = bottom.unwrap_or(screen);
        if top >= bottom {
            return;
        }
        self.region_top = (top as i32 - 1).min(screen as i32);
        self.region_bottom = (bottom as i32).min(screen as i32);
    }

    fn reset_region(&mut self) {
        self.region_top = 0;
        self.region_bottom = self.term.screen_lines() as i32;
    }

    fn scrolled_now(&mut self) -> u64 {
        if self.alternate {
            self.alt_scrolled
        } else {
            self.fold_sentinel();
            self.base = self.count;
            self.count
        }
    }

    fn after_scroll(&mut self) {
        if !self.alternate {
            self.fold_sentinel();
            self.base = self.count;
        }
    }

    fn scroll_moves_pictures(&self, origin: i32, clears: bool, up: bool) -> bool {
        let Some(content_end) = self.motion_end else {
            return false;
        };
        let band_top = self.base.saturating_add(origin.max(0) as u64);
        if content_end <= band_top {
            return false;
        }
        if up && !clears && !self.alternate && origin == 0 && self.region_top == 0
            && self.region_bottom >= self.term.screen_lines() as i32
        {
            let screen_bottom = self.base.saturating_add(self.term.screen_lines() as u64);
            if content_end <= screen_bottom {
                return false;
            }
        }
        true
    }

    fn note_scroll_up(&mut self, origin: i32, mut lines: usize) -> bool {
        let (start, end) = (self.region_top, self.region_bottom);
        lines = lines.min((end - start).max(0) as usize);
        if lines == 0 || end <= origin {
            return false;
        }
        let clears = (end - origin).max(0) as usize <= lines && origin != 0;
        let before = self.scrolled_now();
        let count_delta = if origin == 0 && !clears { lines as u64 } else { 0 };
        if self.alternate && count_delta > 0 {
            self.alt_scrolled = self.alt_scrolled.saturating_add(count_delta);
        }
        let moves = self.scroll_moves_pictures(origin, clears, true);
        if count_delta > 0 && (self.alternate || !moves) {
            self.base = self.base.saturating_add(count_delta);
        }
        if !moves {
            return false;
        }
        self.events.push(Event::RegionScroll {
            alternate: self.alternate,
            top: before.saturating_add(origin.max(0) as u64),
            bottom: before.saturating_add(end.max(0) as u64),
            up: if clears { 0 } else { lines as u64 },
            down: 0,
            count_delta,
            keep_above: origin == 0 && !self.alternate && !clears,
            clears,
        });
        true
    }

    fn note_scroll_down(&mut self, origin: i32, mut lines: usize) -> bool {
        let (start, end) = (self.region_top, self.region_bottom);
        lines = lines.min((end - start).max(0) as usize);
        lines = lines.min((end - origin).max(0) as usize);
        if lines == 0 || end <= origin {
            return false;
        }
        let clears = (end - origin).max(0) as usize <= lines;
        if !self.scroll_moves_pictures(origin, clears, false) {
            return false;
        }
        let before = self.scrolled_now();
        self.events.push(Event::RegionScroll {
            alternate: self.alternate,
            top: before.saturating_add(origin.max(0) as u64),
            bottom: before.saturating_add(end.max(0) as u64),
            up: 0,
            down: if clears { 0 } else { lines as u64 },
            count_delta: 0,
            keep_above: false,
            clears,
        });
        true
    }
}
impl alacritty_terminal::vte::ansi::Handler for Watch<'_> {
    fn set_title(&mut self, title: Option<String>) {
        self.term.set_title(title);
    }
    fn set_cursor_style(&mut self, style: Option<CursorStyle>) {
        self.term.set_cursor_style(style);
    }
    fn set_cursor_shape(&mut self, shape: CursorShape) {
        self.term.set_cursor_shape(shape);
    }
    fn input(&mut self, c: char) {
        if self.print_plain(c) {
            return;
        }
        let mut emitted = false;
        if self.term.mode().contains(TermMode::LINE_WRAP) {
            let cursor = &self.term.grid().cursor;
            if cursor.input_needs_wrap || cursor.point.column.0 + 1 >= self.term.columns() {
                let line = cursor.point.line.0;
                if line + 1 == self.region_bottom {
                    let width = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
                    let pending = cursor.input_needs_wrap && width > 0;
                    let wide = !cursor.input_needs_wrap && width > 1;
                    if pending || wide {
                        emitted = self.note_scroll_up(self.region_top, 1);
                    }
                }
            }
        }
        self.term.input(c);
        if emitted {
            self.after_scroll();
        }
        if !self.in_cell.is_empty() {
            let cursor = &self.term.grid().cursor;
            let row = cursor.point.line.0;
            let col = cursor.point.column.0;
            let wrapped = cursor.input_needs_wrap;
            let line = self.base.saturating_add(row.max(0) as u64);
            if self.in_cell.iter().any(|&(top, bottom, _, _)| line <= bottom && line >= top) {
                let width = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
                if width > 0 {
                    let end = col + usize::from(wrapped);
                    self.covers(row, 1, end.saturating_sub(width), width);
                }
            }
        }
        self.drained += 1;
        if self.drained >= SYNC_DRAIN_CHUNK {
            self.fold_sentinel();
            self.drained = 0;
        }
    }
    fn goto(&mut self, line: i32, col: usize) {
        self.term.goto(line, col);
    }
    fn goto_line(&mut self, line: i32) {
        self.term.goto_line(line);
    }
    fn goto_col(&mut self, col: usize) {
        self.term.goto_col(col);
    }
    fn insert_blank(&mut self, count: usize) {
        let point = self.term.grid().cursor.point;
        let rest = self.term.columns().saturating_sub(point.column.0);
        self.covers(point.line.0, 1, point.column.0, rest.max(count));
        self.term.insert_blank(count);
    }
    fn move_up(&mut self, rows: usize) {
        self.term.move_up(rows);
    }
    fn move_down(&mut self, rows: usize) {
        self.term.move_down(rows);
    }
    fn identify_terminal(&mut self, intermediate: Option<char>) {
        self.term.identify_terminal(intermediate);
    }
    fn device_status(&mut self, arg: usize) {
        self.term.device_status(arg);
    }
    fn move_forward(&mut self, cols: usize) {
        self.term.move_forward(cols);
    }
    fn move_backward(&mut self, cols: usize) {
        self.term.move_backward(cols);
    }
    fn move_down_and_cr(&mut self, row: usize) {
        self.term.move_down_and_cr(row);
    }
    fn move_up_and_cr(&mut self, row: usize) {
        self.term.move_up_and_cr(row);
    }
    fn put_tab(&mut self, count: u16) {
        self.term.put_tab(count);
    }
    fn backspace(&mut self) {
        self.term.backspace();
    }
    fn carriage_return(&mut self) {
        self.term.carriage_return();
    }
    fn linefeed(&mut self) {
        let line = self.term.grid().cursor.point.line.0;
        let emitted = if line + 1 == self.region_bottom {
            self.note_scroll_up(self.region_top, 1)
        } else {
            false
        };
        self.term.linefeed();
        if emitted {
            self.after_scroll();
        }
        if line + 1 == self.region_bottom && self.region_top == 0 {
            self.warm_next_row();
        }
        self.drained += 1;
        if self.drained >= SYNC_DRAIN_CHUNK {
            self.fold_sentinel();
            self.drained = 0;
        }
    }
    fn bell(&mut self) {
        self.term.bell();
    }
    fn substitute(&mut self) {
        self.term.substitute();
    }
    fn newline(&mut self) {
        let line = self.term.grid().cursor.point.line.0;
        let emitted = if line + 1 == self.region_bottom {
            self.note_scroll_up(self.region_top, 1)
        } else {
            false
        };
        self.term.newline();
        if emitted {
            self.after_scroll();
        }
    }
    fn set_horizontal_tabstop(&mut self) {
        self.term.set_horizontal_tabstop();
    }
    fn scroll_up(&mut self, rows: usize) {
        let emitted = self.note_scroll_up(self.region_top, rows);
        self.term.scroll_up(rows);
        if emitted {
            self.after_scroll();
        }
    }
    fn scroll_down(&mut self, rows: usize) {
        self.fold_around_down();
        let emitted = self.note_scroll_down(self.region_top, rows);
        self.term.scroll_down(rows);
        if emitted {
            self.after_scroll();
        }
        self.fold_around_down();
    }
    fn insert_blank_lines(&mut self, count: usize) {
        self.fold_around_down();
        let point = self.term.grid().cursor.point;
        let line = point.line.0;
        let emitted = if line >= self.region_top && line < self.region_bottom {
            self.note_scroll_down(line, count)
        } else {
            false
        };
        self.covers(line, count, 0, self.term.columns());
        self.term.insert_blank_lines(count);
        if emitted {
            self.after_scroll();
        }
        self.fold_around_down();
    }
    fn delete_lines(&mut self, count: usize) {
        let point = self.term.grid().cursor.point;
        let line = point.line.0;
        let lines = (self.term.screen_lines() - line.max(0) as usize).min(count);
        let emitted = if lines > 0 && line >= self.region_top && line < self.region_bottom {
            self.note_scroll_up(line, lines)
        } else {
            false
        };
        self.covers(line, count, 0, self.term.columns());
        self.term.delete_lines(count);
        if emitted {
            self.after_scroll();
        }
    }
    fn erase_chars(&mut self, count: usize) {
        let point = self.term.grid().cursor.point;
        self.covers(point.line.0, 1, point.column.0, count);
        self.term.erase_chars(count);
    }
    fn delete_chars(&mut self, count: usize) {
        let point = self.term.grid().cursor.point;
        let rest = self.term.columns().saturating_sub(point.column.0);
        self.covers(point.line.0, 1, point.column.0, rest.max(count));
        self.term.delete_chars(count);
    }
    fn move_backward_tabs(&mut self, count: u16) {
        self.term.move_backward_tabs(count);
    }
    fn move_forward_tabs(&mut self, count: u16) {
        self.term.move_forward_tabs(count);
    }
    fn save_cursor_position(&mut self) {
        self.term.save_cursor_position();
    }
    fn restore_cursor_position(&mut self) {
        self.term.restore_cursor_position();
    }
    fn clear_line(&mut self, mode: LineClearMode) {
        let point = self.term.grid().cursor.point;
        let columns = self.term.columns();
        match mode {
            LineClearMode::Right => {
                self.covers(point.line.0, 1, point.column.0, columns);
            }
            LineClearMode::Left => {
                self.covers(point.line.0, 1, 0, point.column.0 + 1);
            }
            LineClearMode::All => {
                self.covers(point.line.0, 1, 0, columns);
            }
        }
        self.term.clear_line(mode);
    }
    fn clear_screen(&mut self, mode: ClearMode) {
        if matches!(mode, ClearMode::All | ClearMode::Saved) {
            self.fold_sentinel();
        }
        if matches!(mode, ClearMode::All) {
            let alternate = self.term.mode().contains(TermMode::ALT_SCREEN);
            let pushed = if alternate {
                0
            } else {
                let mut last = 0;
                for row in 0..self.term.screen_lines() {
                    let line = &self.term.grid()[Line(row as i32)];
                    if line.into_iter().any(|cell| !cell.is_empty())
                        && row + 1 > last
                    {
                        last = row + 1;
                    }
                }
                last as u64
            };
            let scrolled = self.count;
            self.term.clear_screen(ClearMode::All);
            if !alternate {
                self.count += pushed;
                self.base = self.count;
            }
            self.replant();
            self.events.push(Event::Clear { alternate, scrolled });
        } else if matches!(mode, ClearMode::Saved) {
            self.term.clear_screen(ClearMode::Saved);
            self.replant();
            self.events.push(Event::ClearSaved);
        } else if matches!(mode, ClearMode::Below) {
            let point = self.term.grid().cursor.point;
            let cols = self.term.columns();
            let rows = self.term.screen_lines() as i32;
            let line = point.line.0;
            self.covers(line, 1, point.column.0, cols.saturating_sub(point.column.0));
            if line + 1 < rows {
                self.covers(line + 1, (rows - line - 1) as usize, 0, cols);
            }
            self.term.clear_screen(mode);
        } else if matches!(mode, ClearMode::Above) {
            let point = self.term.grid().cursor.point;
            let cols = self.term.columns();
            let line = point.line.0;
            if line > 1 {
                self.covers(0, line as usize, 0, cols);
            }
            self.covers(line, 1, 0, point.column.0.saturating_add(1));
            self.term.clear_screen(mode);
        } else {
            self.term.clear_screen(mode);
        }
    }
    fn clear_tabs(&mut self, mode: TabulationClearMode) {
        self.term.clear_tabs(mode);
    }
    fn set_tabs(&mut self, interval: u16) {
        self.term.set_tabs(interval);
    }
    fn reset_state(&mut self) {
        self.fold_sentinel();
        self.term.reset_state();
        if self.alternate {
            self.back_to_primary();
        }
        self.base = self.count;
        self.reset_region();
        self.replant();
        self.events.push(Event::Reset);
    }
    fn reverse_index(&mut self) {
        self.fold_around_down();
        let line = self.term.grid().cursor.point.line.0;
        let emitted = if line == self.region_top {
            self.note_scroll_down(self.region_top, 1)
        } else {
            false
        };
        self.term.reverse_index();
        if emitted {
            self.after_scroll();
        }
        self.fold_around_down();
    }
    fn terminal_attribute(&mut self, attr: Attr) {
        self.term.terminal_attribute(attr);
    }
    fn set_mode(&mut self, mode: Mode) {
        self.term.set_mode(mode);
    }
    fn unset_mode(&mut self, mode: Mode) {
        self.term.unset_mode(mode);
    }
    fn report_mode(&mut self, mode: Mode) {
        self.term.report_mode(mode);
    }
    fn set_private_mode(&mut self, mode: PrivateMode) {
        let entering_alt = matches!(mode, PrivateMode::Named(NamedPrivateMode::SwapScreenAndSetRestoreCursor))
            && !self.term.mode().contains(TermMode::ALT_SCREEN);
        if entering_alt {
            self.fold_sentinel();
            self.primary_snap = Some(snap_term(self.term, self.count));
        }
        self.term.set_private_mode(mode);
        if matches!(mode, PrivateMode::Named(NamedPrivateMode::ColumnMode)) {
            self.reset_region();
        }
        if entering_alt {
            self.alt_scrolled = 0;
            self.primary = self.count;
            self.alternate = true;
            self.count = 0;
            self.base = 0;
        }
    }
    fn unset_private_mode(&mut self, mode: PrivateMode) {
        let leaving_alt = matches!(mode, PrivateMode::Named(NamedPrivateMode::SwapScreenAndSetRestoreCursor))
            && self.term.mode().contains(TermMode::ALT_SCREEN);
        if leaving_alt {
            self.fold_sentinel();
        }
        self.term.unset_private_mode(mode);
        if matches!(mode, PrivateMode::Named(NamedPrivateMode::ColumnMode)) {
            self.reset_region();
        }
        if leaving_alt {
            self.back_to_primary();
        }
    }
    fn report_private_mode(&mut self, mode: PrivateMode) {
        self.term.report_private_mode(mode);
    }
    fn set_scrolling_region(&mut self, top: usize, bottom: Option<usize>) {
        self.note_region(top, bottom);
        self.term.set_scrolling_region(top, bottom);
    }
    fn set_keypad_application_mode(&mut self) {
        self.term.set_keypad_application_mode();
    }
    fn unset_keypad_application_mode(&mut self) {
        self.term.unset_keypad_application_mode();
    }
    fn set_active_charset(&mut self, index: CharsetIndex) {
        self.term.set_active_charset(index);
    }
    fn configure_charset(&mut self, index: CharsetIndex, charset: StandardCharset) {
        self.term.configure_charset(index, charset);
    }
    fn set_color(&mut self, index: usize, rgb: Rgb) {
        self.term.set_color(index, rgb);
    }
    fn dynamic_color_sequence(&mut self, query: String, index: usize, terminator: &str) {
        self.term.dynamic_color_sequence(query, index, terminator);
    }
    fn reset_color(&mut self, index: usize) {
        self.term.reset_color(index);
    }
    fn clipboard_store(&mut self, clipboard: u8, text: &[u8]) {
        self.term.clipboard_store(clipboard, text);
    }
    fn clipboard_load(&mut self, clipboard: u8, message: &str) {
        self.term.clipboard_load(clipboard, message);
    }
    fn decaln(&mut self) {
        self.covers(-(self.term.grid().display_offset() as i32), self.term.screen_lines(), 0, self.term.columns());
        self.term.decaln();
    }
    fn push_title(&mut self) {
        self.term.push_title();
    }
    fn pop_title(&mut self) {
        self.term.pop_title();
    }
    fn text_area_size_pixels(&mut self) {
        self.term.text_area_size_pixels();
    }
    fn text_area_size_chars(&mut self) {
        self.term.text_area_size_chars();
    }
    fn set_hyperlink(&mut self, hyperlink: Option<Hyperlink>) {
        self.term.set_hyperlink(hyperlink);
    }
    fn set_mouse_cursor_icon(&mut self, icon: cursor_icon::CursorIcon) {
        self.term.set_mouse_cursor_icon(icon);
    }
    fn report_keyboard_mode(&mut self) {
        self.term.report_keyboard_mode();
    }
    fn push_keyboard_mode(&mut self, mode: KeyboardModes) {
        self.term.push_keyboard_mode(mode);
    }
    fn pop_keyboard_modes(&mut self, to_pop: u16) {
        self.term.pop_keyboard_modes(to_pop);
    }
    fn set_keyboard_mode(&mut self, mode: KeyboardModes, behavior: KeyboardModesApplyBehavior) {
        self.term.set_keyboard_mode(mode, behavior);
    }
    fn set_modify_other_keys(&mut self, mode: ModifyOtherKeys) {
        self.term.set_modify_other_keys(mode);
    }
    fn report_modify_other_keys(&mut self) {
        self.term.report_modify_other_keys();
    }
    fn set_scp(&mut self, char_path: ScpCharPath, update_mode: ScpUpdateMode) {
        self.term.set_scp(char_path, update_mode);
    }
}

const RUN_MIN: usize = 24;

fn feed(processor: &mut Processor, watch: &mut Watch<'_>, mirror: &mut Mirror, chunk: &[u8], fast: bool) {
    if !fast {
        mirror.follow(chunk);
        processor.advance(watch, chunk);
        return;
    }
    let mut fed = 0;
    let mut known = 0;
    let mut stretch = 0;
    for esc in memchr::memchr_iter(0x1b, chunk).chain(std::iter::once(chunk.len())) {
        let mut from = stretch;
        stretch = esc + 1;
        if esc - from < RUN_MIN {
            continue;
        }
        while let Some((start, end)) = next_run(&chunk[..esc], from) {
            from = end;
            mirror.catch_up(chunk, known, start);
            let ground = mirror.stretch(chunk, start, end);
            known = end;
            if mirror.state != Parse::Ground {
                continue;
            }
            mirror.high = chunk[end - 1] >= 0x80;
            let before_high = if ground == 0 { mirror.high_at_start } else { chunk[ground - 1] >= 0x80 };
            if end - ground < RUN_MIN || before_high {
                continue;
            }
            if fed < ground {
                processor.advance(watch, &chunk[fed..ground]);
                fed = ground;
            }
            if processor.sync_timeout().sync_timeout().is_some() {
                continue;
            }
            let run = &chunk[ground..end];
            fed += if run.is_ascii() { watch.print_run(&run[..run.len() - 1]) } else { watch.print_utf8(run) };
        }
    }
    mirror.catch_up(chunk, known, chunk.len());
    mirror.high_at_start = mirror.high;
    if fed < chunk.len() {
        processor.advance(watch, &chunk[fed..]);
    }
}

fn next_run(bytes: &[u8], mut from: usize) -> Option<(usize, usize)> {
    let printable = |b: u8| (0x20..=0x7e).contains(&b) || b >= 0x80;
    while from + RUN_MIN <= bytes.len() {
        let probe = from + RUN_MIN - 1;
        if !printable(bytes[probe]) {
            from = probe + 1;
            continue;
        }
        let mut start = probe;
        while start > from && printable(bytes[start - 1]) {
            start -= 1;
        }
        let end = probe + 1 + text_len(&bytes[probe + 1..]);
        if end - start >= RUN_MIN {
            return Some((start, end));
        }
        from = end + 1;
    }
    None
}

fn text_len(bytes: &[u8]) -> usize {
    const ONES: u64 = 0x0101_0101_0101_0101;
    const HIGH: u64 = 0x8080_8080_8080_8080;
    let mut at = 0;
    while let Some(&eight) = bytes.get(at..).and_then(|rest| rest.first_chunk::<8>()) {
        let x = u64::from_le_bytes(eight);
        let control = x.wrapping_sub(ONES * 0x20) & !x & HIGH;
        let flipped = x ^ (ONES * 0x7f);
        let del = flipped.wrapping_sub(ONES) & !flipped & HIGH;
        let hit = control | del;
        if hit != 0 {
            return at + (hit.trailing_zeros() / 8) as usize;
        }
        at += 8;
    }
    at + bytes[at..].iter().position(|&b| b < 0x20 || b == 0x7f).unwrap_or(bytes.len() - at)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Parse {
    #[default]
    Ground,
    Escape,
    EscapeIntermediate,
    Csi,
    DcsEntry,
    DcsParam,
    DcsIntermediate,
    DcsIgnore,
    DcsPassthrough,
    Osc,
    String,
}

#[derive(Default)]
struct Mirror {
    state: Parse,
    high: bool,
    high_at_start: bool,
}

impl Mirror {
    fn follow(&mut self, bytes: &[u8]) {
        self.catch_up(bytes, 0, bytes.len());
        self.high_at_start = self.high;
    }

    fn catch_up(&mut self, bytes: &[u8], from: usize, to: usize) {
        let from = match memchr::memrchr(0x1b, &bytes[from..to]) {
            Some(k) => {
                self.state = Parse::Escape;
                self.high = false;
                from + k + 1
            }
            None => from,
        };
        if from < to {
            let ground = self.stretch(bytes, from, to);
            if self.state == Parse::Ground && ground < to {
                self.high = bytes[to - 1] >= 0x80;
            }
        }
    }

    fn stretch(&mut self, bytes: &[u8], mut at: usize, end: usize) -> usize {
        let find = |at: usize, hit: fn(u8) -> bool| bytes[at..end].iter().position(|&b| hit(b)).map(|k| at + k);
        while at < end {
            match self.state {
                Parse::Ground => return at,
                Parse::Csi => match find(at, |b| (0x40..=0x7e).contains(&b) || b == 0x18 || b == 0x1a) {
                    Some(k) => {
                        self.state = Parse::Ground;
                        at = k + 1;
                    }
                    None => return end,
                },
                Parse::Osc => match find(at, |b| b == 0x07 || b == 0x18 || b == 0x1a) {
                    Some(k) => {
                        self.state = Parse::Ground;
                        at = k + 1;
                    }
                    None => return end,
                },
                Parse::DcsPassthrough => match find(at, |b| b == 0x9c || b == 0x18 || b == 0x1a) {
                    Some(k) => {
                        self.state = Parse::Ground;
                        at = k + 1;
                    }
                    None => return end,
                },
                Parse::String | Parse::DcsIgnore => match find(at, |b| b == 0x18 || b == 0x1a) {
                    Some(k) => {
                        self.state = Parse::Ground;
                        at = k + 1;
                    }
                    None => return end,
                },
                state => {
                    let b = bytes[at];
                    at += 1;
                    self.state = match (state, b) {
                        (_, 0x18 | 0x1a) => Parse::Ground,
                        (Parse::Escape, b) => match b {
                            0x20..=0x2f => Parse::EscapeIntermediate,
                            b'P' => Parse::DcsEntry,
                            b'X' | b'^' | b'_' => Parse::String,
                            b'[' => Parse::Csi,
                            b']' => Parse::Osc,
                            0x30..=0x7e => Parse::Ground,
                            _ => Parse::Escape,
                        },
                        (Parse::EscapeIntermediate, 0x30..=0x7e) => Parse::Ground,
                        (Parse::DcsEntry, 0x20..=0x2f) => Parse::DcsIntermediate,
                        (Parse::DcsEntry, 0x30..=0x3f) => Parse::DcsParam,
                        (Parse::DcsEntry | Parse::DcsParam | Parse::DcsIntermediate, 0x40..=0x7e) => {
                            Parse::DcsPassthrough
                        }
                        (Parse::DcsParam, 0x20..=0x2f) => Parse::DcsIntermediate,
                        (Parse::DcsParam, 0x3c..=0x3f) => Parse::DcsIgnore,
                        (Parse::DcsIntermediate, 0x30..=0x3f) => Parse::DcsIgnore,
                        (state, _) => state,
                    };
                }
            }
            if self.state == Parse::Ground {
                self.high = false;
            }
        }
        at
    }
}

#[inline(always)]
fn utf8_at(bytes: &[u8], at: usize) -> Option<(char, usize)> {
    let lead = *bytes.get(at)?;
    if lead < 0x80 {
        return Some((char::from(lead), 1));
    }
    let next = |k: usize, low: u8, high: u8| bytes.get(at + k).copied().filter(|b| (low..=high).contains(b));
    let tail = |b: u8| u32::from(b & 0x3f);
    let (value, len) = match lead {
        0xc2..=0xdf => (u32::from(lead & 0x1f) << 6 | tail(next(1, 0x80, 0xbf)?), 2),
        0xe0..=0xef => {
            let (low, high) = match lead {
                0xe0 => (0xa0, 0xbf),
                0xed => (0x80, 0x9f),
                _ => (0x80, 0xbf),
            };
            let b1 = next(1, low, high)?;
            (u32::from(lead & 0x0f) << 12 | tail(b1) << 6 | tail(next(2, 0x80, 0xbf)?), 3)
        }
        0xf0..=0xf4 => {
            let (low, high) = match lead {
                0xf0 => (0x90, 0xbf),
                0xf4 => (0x80, 0x8f),
                _ => (0x80, 0xbf),
            };
            let b1 = next(1, low, high)?;
            let b2 = next(2, 0x80, 0xbf)?;
            (u32::from(lead & 0x07) << 18 | tail(b1) << 12 | tail(b2) << 6 | tail(next(3, 0x80, 0xbf)?), 4)
        }
        _ => return None,
    };
    Some((char::from_u32(value)?, len))
}

fn held_back(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.first() != Some(&0x1b) {
        return None;
    }
    const CANDIDATES: [&[u8]; 7] = [
        b"\x1b[c",
        b"\x1b[0c",
        b"\x1b[>c",
        b"\x1b[>0c",
        b"\x1b[>q",
        b"\x1b[>0q",
        b"\x1b[?2026$p",
    ];
    let could = |candidate: &[u8]| bytes.len() < candidate.len() && candidate.starts_with(bytes);
    CANDIDATES.iter().any(|c| could(c)).then(|| bytes.to_vec())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ask {
    CellSize,
    Primary,
    Graphics(u32),
    Secondary,
    Version,
    Synchronized,
}

fn intercepted(bytes: &[u8]) -> Option<(usize, Ask)> {
    let rest = bytes.strip_prefix(b"\x1b[")?;
    if rest.starts_with(b"16t") {
        return Some((5, Ask::CellSize));
    }
    if rest.starts_with(b"c") {
        return Some((3, Ask::Primary));
    }
    if rest.starts_with(b"0c") {
        return Some((4, Ask::Primary));
    }
    if let Some(params) = rest.strip_prefix(b"?") {
        let end = params.iter().position(|b| (0x40..=0x7e).contains(b));
        if let Some(end) = end
            && params[end] == b'S'
            && let Ok(text) = std::str::from_utf8(&params[..end])
            && let Some((pi, "1")) = text.split_once(';')
            && let Ok(pi) = pi.parse::<u32>()
        {
            return Some((3 + end + 1, Ask::Graphics(pi)));
        }
    }
    if rest.starts_with(b">c") {
        return Some((4, Ask::Secondary));
    }
    if rest.starts_with(b">0c") {
        return Some((5, Ask::Secondary));
    }
    if rest.starts_with(b">q") {
        return Some((4, Ask::Version));
    }
    if rest.starts_with(b">0q") {
        return Some((5, Ask::Version));
    }
    if rest.starts_with(b"?2026$p") {
        return Some((9, Ask::Synchronized));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> Engine {
        Engine::new(
            24,
            80,
            HostInfo { colors: [[0xd0, 0xd0, 0xd0], [0, 0, 0], [0xff, 0xff, 0xff]], cell_px: (10, 20) },
        )
    }

    fn answers(input: &[u8]) -> Vec<u8> {
        let mut term = engine();
        term.process(input);
        term.take_effects().replies
    }

    #[test]
    fn startup_queries_are_answered() {
        assert_eq!(answers(b"\x1b[c"), b"\x1b[?62;4;22c");
        assert_eq!(answers(b"\x1b[3;5H\x1b[6n"), b"\x1b[3;5R");
        assert_eq!(answers(b"\x1b[5n"), b"\x1b[0n");
        assert_eq!(answers(b"\x1b[18t"), b"\x1b[8;24;80t");
        assert_eq!(answers(b"\x1b[14t"), b"\x1b[4;480;800t");
        assert_eq!(answers(b"\x1b[16t"), b"\x1b[6;20;10t");
        assert_eq!(answers(b"\x1b[>c"), b"\x1b[>1;10;0c");
        let version = env!("CARGO_PKG_VERSION");
        assert_eq!(answers(b"\x1b[>q\x07"), format!("\x1bP>|glass {version}\x1b\\").as_bytes());
        assert_eq!(answers(b"\x1b]11;?\x07"), b"\x1b]11;rgb:0000/0000/0000\x1b\\");
        assert_eq!(answers(b"\x1b[?2026$p"), b"\x1b[?2026;2$y");
        assert_eq!(answers(b"\x1b[?2026h\x1b[?2026$p"), b"\x1b[?2026;1$y");
        assert_eq!(answers(b"\x1b[?9999$p"), b"\x1b[?9999;0$y");
    }

    #[test]
    fn text_runs_end_where_a_byte_at_a_time_scan_ends() {
        let slow = |bytes: &[u8]| bytes.iter().position(|&b| b < 0x20 || b == 0x7f).unwrap_or(bytes.len());
        for stop in 0..=255u8 {
            for at in 0..20 {
                for fill in [b'a', 0x20, 0x7e, 0x80, 0xff, 0x21, 0x9f] {
                    let mut bytes = [fill; 19];
                    if at < bytes.len() {
                        bytes[at] = stop;
                    }
                    for from in 0..bytes.len() {
                        assert_eq!(text_len(&bytes[from..]), slow(&bytes[from..]), "stop {stop:#x} at {at} over {fill:#x} from {from}");
                    }
                }
            }
        }
        assert_eq!(text_len(b"abc\x1f\x20xyz"), 3);
        assert_eq!(text_len(b"abcdefg\x00\x7f"), 7);
    }

    #[test]
    fn utf8_decoding_matches_the_standard_library() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let text = "a漢字🙂é\u{301}\u{85}\u{7f}\u{10ffff}\u{d7ff}\u{e000}\u{800}\u{80}".as_bytes();
        for round in 0..20_000 {
            let bytes: Vec<u8> = if round % 3 == 0 {
                text.iter().map(|&b| if next() % 13 == 0 { next() as u8 } else { b }).collect()
            } else {
                (0..8).map(|_| [0x80, 0xbf, 0xc2, 0xe0, 0xed, 0xf0, 0xf4, 0xf5, 0x9f, 0xa0, 0x41][next() as usize % 11] ^ ((next() % 4 == 0) as u8 * (next() as u8 & 0x3f))).collect()
            };
            for at in 0..bytes.len() {
                let rest = &bytes[at..];
                let valid = match std::str::from_utf8(rest) {
                    Ok(text) => text,
                    Err(error) => std::str::from_utf8(&rest[..error.valid_up_to()]).unwrap(),
                };
                let want = valid.chars().next().map(|c| (c, c.len_utf8()));
                assert_eq!(utf8_at(&bytes, at), want, "{:x?} at {at}", bytes);
            }
        }
    }

    #[test]
    fn replies_keep_stream_order_in_one_feed() {
        assert_eq!(answers(b"\x1b[2;3H\x1b[6n\x1b[c"), b"\x1b[2;3R\x1b[?62;4;22c");
        assert_eq!(answers(b"\x1b[c\x1b[6n"), b"\x1b[?62;4;22c\x1b[1;1R");
        assert_eq!(answers(b"\x1b[5n\x1b[16t\x1b[5n"), b"\x1b[0n\x1b[6;20;10t\x1b[0n");
        assert_eq!(
            answers(b"\x1b[?2026h\x1b[?2026$p\x1b[?2026l\x1b[?2026$p"),
            b"\x1b[?2026;1$y\x1b[?2026;2$y"
        );
        assert_eq!(answers(b"\x1b[?2;1Sx\x1b[?1;1S"), b"\x1b[?2;0;800;480S\x1b[?1;0;1024S");
    }

    #[test]
    fn split_sequences_are_still_answered() {
        let mut term = engine();
        term.process(b"\x1b");
        assert!(term.take_effects().replies.is_empty());
        term.process(b"[c");
        assert_eq!(term.take_effects().replies, b"\x1b[?62;4;22c");
        let mut term = engine();
        term.process(b"hi\x1b[");
        term.process(b"16t");
        assert_eq!(term.take_effects().replies, b"\x1b[6;20;10t");
        assert_eq!(&term.cells()[..2].iter().map(|c| c.c).collect::<String>(), "hi");
    }

    #[test]
    fn keyboard_dialect_flags_are_tracked() {
        let mut term = engine();
        assert_eq!(term.keyboard_flags(), 0);
        term.process(b"\x1b[>1u");
        assert_eq!(term.keyboard_flags(), 1);
        term.process(b"\x1b[?u");
        assert_eq!(term.take_effects().replies, b"\x1b[?1u");
        term.process(b"\x1b[<u");
        assert_eq!(term.keyboard_flags(), 0);
        term.process(b"\x1b[?u");
        assert_eq!(term.take_effects().replies, b"\x1b[?0u");
    }

    #[test]
    fn a_synchronized_update_ends_at_its_deadline() {
        let mut term = engine();
        term.process(b"\x1b[?2026hhi");
        assert!(term.synchronized());
        assert!(term.take_effects().replies.is_empty() || !term.take_effects().replies.is_empty());
        term.process(b"\x1b[?2026$p");
        assert!(term.take_effects().replies.ends_with(b"\x1b[?2026;1$y"));
        assert!(!term.end_sync_update());
        assert!(term.synchronized());
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(term.end_sync_update());
        assert!(!term.synchronized());
        assert_eq!(&term.cells()[..2].iter().map(|c| c.c).collect::<String>(), "hi");
        assert!(!term.end_sync_update());
        term.process(b"\x1b[?2026$p");
        assert!(term.take_effects().replies.ends_with(b"\x1b[?2026;2$y"));
    }

    #[test]
    fn lines_scrolled_off_top_count_exactly() {
        let mut term = Engine::new(4, 10, HostInfo { colors: [[0; 3]; 3], cell_px: (1, 1) });
        assert_eq!(term.scrolled_off_top(), 0);
        term.process(b"one\r\ntwo\r\nthree\r\nfour\r\nfive");
        assert_eq!(term.scrolled_off_top(), 1);
        term.process(b"six\r\nseven");
        assert_eq!(term.scrolled_off_top(), 2);
        term.process(b"0123456789abc");
        assert_eq!(term.scrolled_off_top(), 3);
    }

    #[test]
    fn lines_scrolled_off_top_survive_a_cleared_history() {
        let mut term = Engine::new(4, 10, HostInfo { colors: [[0; 3]; 3], cell_px: (1, 1) });
        term.process(b"one\r\ntwo\r\nthree\r\nfour\r\nfive");
        assert_eq!(term.scrolled_off_top(), 1);
        term.process(b"\x1b[3J");
        assert_eq!(term.scrolled_off_top(), 1);
        term.process(b"\r\nsix");
        assert_eq!(term.scrolled_off_top(), 2);
    }

    #[test]
    fn lines_scrolled_inside_a_region_do_not_count() {
        let mut term = Engine::new(6, 10, HostInfo { colors: [[0; 3]; 3], cell_px: (1, 1) });
        term.process(b"\x1b[2;6r\x1b[5S");
        assert_eq!(term.scrolled_off_top(), 0);
        term.process(b"\x1b[1;6r\x1b[2S");
        assert_eq!(term.scrolled_off_top(), 2);
    }

    #[test]
    fn lines_scrolled_off_top_count_through_a_full_history() {
        let mut term = Engine::new(4, 10, HostInfo { colors: [[0; 3]; 3], cell_px: (1, 1) });
        let lines = "0123456789\r\n".repeat(4_000);
        term.process(lines.as_bytes());
        assert_eq!(term.scrolled_off_top(), 3_997);
        let more = "0123456789\r\n".repeat(8_000);
        term.process(more.as_bytes());
        assert_eq!(term.scrolled_off_top(), 11_997);
    }

    #[test]
    fn the_alternate_screen_scrolls_are_not_counted() {
        let mut term = Engine::new(4, 10, HostInfo { colors: [[0; 3]; 3], cell_px: (1, 1) });
        term.process(b"one\r\ntwo\r\nthree\r\nfour\r\nfive");
        assert_eq!(term.scrolled_off_top(), 1);
        term.process(b"\x1b[?1049h");
        assert!(term.alternate_screen());
        term.process(b"a\r\nb\r\nc\r\nd\r\ne\r\nf");
        assert_eq!(term.scrolled_off_top(), 1);
        term.process(b"\x1b[?1049l");
        assert_eq!(term.scrolled_off_top(), 1);
        term.process(b"\r\nsix");
        assert_eq!(term.scrolled_off_top(), 2);
    }

    #[test]
    fn wide_characters_copy_once() {
        let mut term = Engine::new(3, 10, HostInfo { colors: [[0; 3]; 3], cell_px: (1, 1) });
        term.process("你好🎉🎊".as_bytes());
        let cells = term.cells();
        let chars: String = cells[..8].iter().map(|c| c.c).collect();
        assert_eq!(chars.chars().filter(|&c| c != '\0').collect::<String>(), "你好🎉🎊");
        assert_eq!(chars.chars().filter(|&c| c == '\0').count(), 4);
    }

    #[test]
    fn modes_titles_and_clipboard_are_tracked() {
        let mut term = engine();
        term.process(b"\x1b[?1004h\x1b]2;a;b\x07");
        assert!(term.focus_reports());
        assert_eq!(term.take_effects().title.as_deref(), Some("a;b"));
        term.process(b"\x1b[?1004l\x1b]52;c;aGk=\x07\x07");
        assert!(!term.focus_reports());
        let effects = term.take_effects();
        assert_eq!(effects.clipboard.as_deref(), Some("hi"));
        assert!(effects.bell);
    }

    #[test]
    fn cells_carry_style_and_links() {
        let mut term = engine();
        term.process(b"\x1b[1;31mA\x1b[0m");
        assert_eq!(term.cells()[0].c, 'A');
        assert_eq!(term.cells()[0].fg, Color::Indexed(1));
        assert!(term.cells()[0].flags.contains(CellFlags::BOLD));
        term.process(b"\x1b]8;;https://example.org\x1b\\link\x1b]8;;\x1b\\");
        assert_eq!(term.cells()[4].link.as_deref(), Some("https://example.org"));
        assert_eq!(term.link_at(0, 4).as_deref(), Some("https://example.org"));
        assert_eq!(term.link_at(0, 4).as_deref(), Some("https://example.org"));
        assert!(term.link_at(0, 9).is_none());
    }

    #[test]
    fn every_handler_method_is_forwarded() {
        let stream = b"\x1b[31;4mSGR\x1b[0m\
            \x1b[3;5H\x1b[A\x1b[B\x1b[C\x1b[D\x1b[E\x1b[F\x1b[H\x1b[d\x1b[2;3r\x1b[1;6H\
            \x1b[2J\x1b[3J\x1b[K\x1b[1K\x1b[2K\x1b[J\x1b[1J\x1b[2X\x1b[2P\x1b[2@\
            \x1b[2L\x1b[2M\x1b[2S\x1b[2T\x1b[r\x1bZ\x1b[g\x1b[3g\
            \x1b]0;the title\x07\x1b]2;two\x07\
            \x1b[4h\x1b[4l\x1b[?1h\x1b[?1l\x1b[?6h\x1b[?6l\x1b[?7h\x1b[?7l\
            \x1b[?12h\x1b[?12l\x1b[?25h\x1b[?25l\x1b[?47h\x1b[?47l\
            \x1b[?1000h\x1b[?1000l\x1b[?1002h\x1b[?1003h\x1b[?1005h\x1b[?1006h\x1b[?1015h\
            \x1b[?1048h\x1b[?1048l\x1b[?1049h\x1b[?1049l\x1b[?2004h\x1b[?2004l\
            \x1b[?2026h\x1b[?2026l\x1b[?7714h\x1b[?7714l\
            \x1b[>4;2m\x1b[>4;0m\x1b[=5;2u\
            \x1b[2 q\x1b[4 q\x1b7\x1b8\x1bD\x1bM\x1bE\x1b#8\x1b(B\x1b(0\x1b)\
            \x1bP+q544e\x1b\\\x1b_Gi=1,a=q\x1b\\\x1b^Xok\x1b\\\
            \x1bc";
        let mut wrapped = engine();
        wrapped.process(stream);
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut bare = Term::new(
            Config { scrolling_history: SCROLLBACK, kitty_keyboard: true, ..Config::default() },
            &Size { columns: 10, lines: 4 },
            Listener { tx },
        );
        let mut proc: Processor<alacritty_terminal::vte::ansi::StdSyncHandler> = Processor::new();
        let only_text = b"\x1b[31;4mSGR\x1b[0m\x1b[3;5H\x1b[A\x1b[B\x1b[C\x1b[D\
            \x1b[2X\x1b[2P\x1b[2@\x1b[2L\x1b[2M\x1b[2S\x1b[2T\x1bZ\x1b[g\x1b[3g\
            \x1b]0;the title\x07\x1b[4h\x1b[4l\x1b[?12h\x1b[?12l\x1b[=5;2u\x1b[2 q\
            \x1b7\x1b8\x1bD\x1bM\x1bE\x1b#8\x1b(B\x1b(0";
        proc.advance(&mut bare, only_text);
        let mut wrapped2 = engine();
        wrapped2.process(only_text);
        assert_eq!(
            wrapped2.term.grid().cursor.point, bare.grid().cursor.point,
            "the wrapper forwards every method it sees"
        );
        assert_eq!(wrapped2.keyboard_flags(), {
            let mode = bare.mode();
            let mut flags = 0;
            flags |= u8::from(mode.contains(TermMode::DISAMBIGUATE_ESC_CODES));
            flags |= u8::from(mode.contains(TermMode::REPORT_EVENT_TYPES)) << 1;
            flags |= u8::from(mode.contains(TermMode::REPORT_ALTERNATE_KEYS)) << 2;
            flags |= u8::from(mode.contains(TermMode::REPORT_ALL_KEYS_AS_ESC)) << 3;
            flags
        });
    }

    #[test]
    fn cursor_and_scrollback_move_together() {
        let mut term = Engine::new(4, 10, HostInfo { colors: [[0; 3]; 3], cell_px: (1, 1) });
        term.process(b"one\r\ntwo\r\nthree\r\nfour\r\nfive");
        assert_eq!(term.scrolled_off_top(), 1);
        assert!(term.scroll_display(1));
        assert!(term.cursor().is_none());
        assert!(term.scroll_display(-1));
        assert_eq!(term.cursor(), Some((3, 4)));
    }

    #[test]
    fn cursor_style_follows_the_program() {
        let mut term = engine();
        assert!(!term.cursor_blinking());
        assert_eq!(term.cursor_look(), CursorLook::Block);
        term.process(b"\x1b[0 q");
        assert_eq!(term.cursor_look(), CursorLook::Block);
        assert!(!term.cursor_blinking(), "DECSCUSR 0 restores glass's steady block");
        term.process(b"\x1b[1 q");
        assert_eq!(term.cursor_look(), CursorLook::Block);
        assert!(term.cursor_blinking());
        term.process(b"\x1b[2 q");
        assert_eq!(term.cursor_look(), CursorLook::Block);
        assert!(!term.cursor_blinking(), "DECSCUSR 2 is a steady block");
        term.process(b"\x1b[3 q");
        assert_eq!(term.cursor_look(), CursorLook::Underline);
        assert!(term.cursor_blinking());
        term.process(b"\x1b[4 q");
        assert_eq!(term.cursor_look(), CursorLook::Underline);
        assert!(!term.cursor_blinking());
        term.process(b"\x1b[5 q");
        assert_eq!(term.cursor_look(), CursorLook::Beam);
        assert!(term.cursor_blinking());
        term.process(b"\x1b[6 q");
        assert_eq!(term.cursor_look(), CursorLook::Beam);
        assert!(!term.cursor_blinking());
        term.process(b"\x1b[?12h");
        assert!(term.cursor_blinking());
        term.process(b"\x1b[?12l");
        assert!(!term.cursor_blinking());
        term.process(b"\x1b[?25l");
        assert_eq!(term.cursor_look(), CursorLook::Beam);
        assert!(term.cursor().is_none());
        term.process(b"\x1b[?25h");
        assert!(term.cursor().is_some());
        term.process(b"\x1b]12;#ff0000\x07");
        assert_eq!(term.cursor_color(), Some(Rgb { r: 0xff, g: 0, b: 0 }));
    }

    #[test]
    fn mouse_modes_and_encodings_are_reported() {
        let mut term = engine();
        assert_eq!(term.mouse_protocol(), (MouseProtocolMode::None, MouseProtocolEncoding::Default));
        term.process(b"\x1b[?1000;1006h");
        assert_eq!(term.mouse_protocol(), (MouseProtocolMode::PressRelease, MouseProtocolEncoding::Sgr));
        term.process(b"\x1b[?1002;1005h");
        assert_eq!(term.mouse_protocol(), (MouseProtocolMode::ButtonMotion, MouseProtocolEncoding::Utf8));
        term.process(b"\x1b[?1003h");
        assert_eq!(term.mouse_protocol(), (MouseProtocolMode::AnyMotion, MouseProtocolEncoding::Utf8));
    }
}
