//! Pane-aware mouse selection + system-clipboard copy for the cockpit.
//!
//! Native terminal selection operates over the raw cell grid, so dragging there
//! grabs terminal output, HUD panels, and borders together. With
//! `EnableMouseCapture` on (see `main.rs`) the app accepts selection only from
//! the Agent chat transcript, then copies that surface's chrome-free content
//! through OSC-52. Other pane rectangles remain registered for focus, scrolling,
//! and control hit-testing but are not clipboard sources.
//!
//! Everything here is pure and headless-testable: point→pane hit-testing,
//! drag→clamped region, region→trimmed text (read straight out of the rendered
//! ratatui buffer), the OSC-52 payload encoding, and the PTY mouse-report
//! encoding used to forward the mouse to a program that asked for it. `main.rs`
//! owns the side-effecting wiring (capture lifecycle, event routing, the
//! per-draw highlight + extraction, the clipboard write).

use base64::Engine as _;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use unicode_width::UnicodeWidthStr;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PaneId {
    Transcript,
    AgentBay,
    Artifacts,
    Input,
    Shell,
}

#[derive(Clone, Debug, Default)]
pub struct PaneRegistry {
    panes: Vec<(PaneId, Rect)>,
}

impl PaneRegistry {
    pub fn clear(&mut self) {
        self.panes.clear();
    }

    pub fn push(&mut self, id: PaneId, rect: Rect) {
        if rect.width > 0 && rect.height > 0 {
            self.panes.push((id, rect));
        }
    }

    pub fn pane_at(&self, x: u16, y: u16) -> Option<(PaneId, Rect)> {
        self.panes
            .iter()
            .rev()
            .find(|(_, r)| point_in(*r, x, y))
            .copied()
    }

    pub fn rect_of(&self, id: PaneId) -> Option<Rect> {
        self.panes
            .iter()
            .rev()
            .find(|(p, _)| *p == id)
            .map(|(_, r)| *r)
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.panes.is_empty()
    }
}

pub fn point_in(r: Rect, x: u16, y: u16) -> bool {
    x >= r.x && x < r.x.saturating_add(r.width) && y >= r.y && y < r.y.saturating_add(r.height)
}

pub fn clamp_point(rect: Rect, x: u16, y: u16) -> (u16, u16) {
    let last_x = rect.right().saturating_sub(1).max(rect.x);
    let last_y = rect.bottom().saturating_sub(1).max(rect.y);
    (x.clamp(rect.x, last_x), y.clamp(rect.y, last_y))
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    pub pane: PaneId,
    pub rect: Rect,
    pub anchor: (u16, u16),
    pub cursor: (u16, u16),
    dragging: bool,
}

impl Selection {
    pub fn new(pane: PaneId, rect: Rect, x: u16, y: u16) -> Self {
        let p = clamp_point(rect, x, y);
        Self {
            pane,
            rect,
            anchor: p,
            cursor: p,
            dragging: true,
        }
    }

    pub fn extend(&mut self, x: u16, y: u16) {
        self.cursor = clamp_point(self.rect, x, y);
    }

    pub fn rebind(&mut self, rect: Rect) -> bool {
        if rect.width == 0 || rect.height == 0 {
            return false;
        }
        if !selection_cells_intersect(self, rect) {
            return false;
        }
        let was_empty = self.is_empty();
        self.rect = rect;
        self.anchor = clamp_point(rect, self.anchor.0, self.anchor.1);
        self.cursor = clamp_point(rect, self.cursor.0, self.cursor.1);
        if self.is_empty() {
            return was_empty && self.dragging;
        }
        true
    }

    pub fn ordered(&self) -> ((u16, u16), (u16, u16)) {
        order_points(self.anchor, self.cursor)
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.cursor
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    pub fn finish(&mut self) {
        self.dragging = false;
    }
}

pub fn order_points(a: (u16, u16), b: (u16, u16)) -> ((u16, u16), (u16, u16)) {
    if (a.1, a.0) <= (b.1, b.0) {
        (a, b)
    } else {
        (b, a)
    }
}

#[cfg(test)]
pub fn selection_spans(sel: &Selection) -> Vec<(u16, u16, u16)> {
    selection_spans_raw(sel, sel.rect)
}

pub fn selection_spans_in(sel: &Selection, clip: Rect) -> Vec<(u16, u16, u16)> {
    if sel.is_empty() {
        return Vec::new();
    }
    if !selection_cells_intersect(sel, clip) {
        return Vec::new();
    }
    selection_spans_raw(sel, clip)
}

fn selection_spans_raw(sel: &Selection, clip: Rect) -> Vec<(u16, u16, u16)> {
    let r = intersect_rects(sel.rect, clip);
    if r.width == 0 || r.height == 0 {
        return Vec::new();
    }
    let left = r.x;
    let right = r.right().saturating_sub(1).max(r.x);
    let top = r.y;
    let bottom = r.bottom().saturating_sub(1).max(r.y);
    let ((sx, sy), (ex, ey)) = sel.ordered();
    let sy = sy.clamp(top, bottom);
    let ey = ey.clamp(top, bottom);
    let sx = sx.clamp(left, right);
    let ex = ex.clamp(left, right);
    let ((sx, sy), (ex, ey)) = order_points((sx, sy), (ex, ey));
    let mut spans = Vec::new();
    for y in sy..=ey {
        let x0 = if y == sy { sx } else { left };
        let x1 = if y == ey { ex } else { right };
        let x0 = x0.clamp(left, right);
        let x1 = x1.clamp(left, right);
        if x1 >= x0 {
            spans.push((y, x0, x1));
        }
    }
    spans
}

fn selection_cells_intersect(sel: &Selection, rect: Rect) -> bool {
    if rect.width == 0 || rect.height == 0 {
        return false;
    }
    let right = rect.right().saturating_sub(1).max(rect.x);
    let bottom = rect.bottom();
    for (y, x0, x1) in selection_spans_raw(sel, sel.rect) {
        if y < rect.y || y >= bottom {
            continue;
        }
        let cx0 = x0.max(rect.x);
        let cx1 = x1.min(right);
        if cx1 >= cx0 {
            return true;
        }
    }
    false
}

pub fn intersect_rects(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = a.right().min(b.right());
    let y1 = a.bottom().min(b.bottom());
    if x1 <= x0 || y1 <= y0 {
        Rect::new(x0, y0, 0, 0)
    } else {
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }
}

#[cfg(test)]
pub fn extract_text(buf: &Buffer, sel: &Selection) -> String {
    extract_text_in(buf, sel, sel.rect)
}

fn cell_is_wide_grapheme_continuation(buf: &Buffer, x: u16, y: u16) -> bool {
    let mut col = 0u16;
    while col < x {
        let Some(cell) = buf.cell((col, y)) else {
            col = col.saturating_add(1);
            continue;
        };
        let w = UnicodeWidthStr::width(cell.symbol()).max(1) as u16;
        let end = col.saturating_add(w);
        if x > col && x < end {
            return true;
        }
        col = end.max(col.saturating_add(1));
    }
    false
}

pub fn extract_text_in(buf: &Buffer, sel: &Selection, clip: Rect) -> String {
    if sel.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for (i, (y, x0, x1)) in selection_spans_in(sel, clip).iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let mut row = String::new();
        let mut continuation_cells = 0usize;
        for x in *x0..=*x1 {
            if continuation_cells > 0 {
                continuation_cells -= 1;
                continue;
            }
            if cell_is_wide_grapheme_continuation(buf, x, *y) {
                continue;
            }
            if let Some(cell) = buf.cell((x, *y)) {
                let symbol = cell.symbol();
                row.push_str(symbol);
                continuation_cells = UnicodeWidthStr::width(symbol).saturating_sub(1);
            }
        }
        out.push_str(row.trim_end_matches(' '));
    }
    out.trim_end_matches('\n').to_string()
}

#[cfg(test)]
pub fn highlight(buf: &mut Buffer, sel: &Selection) {
    highlight_in(buf, sel, sel.rect);
}

pub fn highlight_in(buf: &mut Buffer, sel: &Selection, clip: Rect) {
    if sel.is_empty() {
        return;
    }
    let reversed = Style::new().add_modifier(Modifier::REVERSED);
    let buf_right = buf.area().right();
    let buf_bottom = buf.area().bottom();
    for (y, x0, x1) in selection_spans_in(sel, clip) {
        if y < clip.y || y >= clip.bottom() || y >= buf_bottom {
            continue;
        }
        for x in x0..=x1 {
            if x < clip.x || x >= clip.right() || x >= buf_right {
                continue;
            }
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_style(reversed);
            }
        }
    }
}

pub fn inner_border(area: Rect) -> Rect {
    Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    }
}

pub const OSC52_MAX_PAYLOAD_BYTES: usize = 74_994;

pub fn base64_clipboard_len(text: &str) -> Option<usize> {
    text.len()
        .checked_add(2)
        .map(|bytes| bytes / 3)
        .and_then(|groups| groups.checked_mul(4))
}

pub fn osc52_payload_fits(text: &str) -> bool {
    base64_clipboard_len(text).is_some_and(|len| len <= OSC52_MAX_PAYLOAD_BYTES)
}

pub fn base64_clipboard(text: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(text.as_bytes())
}

pub fn osc52_sequence(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64_clipboard(text))
}

pub fn selection_overrides_pty_mouse(ev: &MouseEvent, selection: Option<&Selection>) -> bool {
    let left_gesture = matches!(
        ev.kind,
        MouseEventKind::Down(MouseButton::Left)
            | MouseEventKind::Drag(MouseButton::Left)
            | MouseEventKind::Up(MouseButton::Left)
    );
    left_gesture
        && (ev
            .modifiers
            .contains(ratatui::crossterm::event::KeyModifiers::SHIFT)
            || selection.is_some_and(|selection| {
                selection.pane == PaneId::Shell && selection.is_dragging()
            }))
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TrackMode {
    None,
    Press,
    PressRelease,
    ButtonMotion,
    AnyMotion,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReportEncoding {
    X10,
    Sgr,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ReportButton {
    Left,
    Middle,
    Right,
    None,
    WheelUp,
    WheelDown,
}

struct Report {
    button: ReportButton,
    motion: bool,
    release: bool,
    shift: bool,
    alt: bool,
    ctrl: bool,
    col1: u16,
    row1: u16,
}

fn report_cb(r: &Report) -> u16 {
    let mut cb: u16 = match r.button {
        ReportButton::Left => 0,
        ReportButton::Middle => 1,
        ReportButton::Right => 2,
        ReportButton::None => 3,
        ReportButton::WheelUp => 64,
        ReportButton::WheelDown => 65,
    };
    if r.motion {
        cb += 32;
    }
    if r.shift {
        cb += 4;
    }
    if r.alt {
        cb += 8;
    }
    if r.ctrl {
        cb += 16;
    }
    cb
}

fn encode_report(r: &Report, enc: ReportEncoding) -> Vec<u8> {
    let cb = report_cb(r);
    let col = r.col1.max(1);
    let row = r.row1.max(1);
    match enc {
        ReportEncoding::Sgr => {
            let final_byte = if r.release { 'm' } else { 'M' };
            format!("\x1b[<{cb};{col};{row}{final_byte}").into_bytes()
        }
        ReportEncoding::X10 => {
            let cb = if r.release {
                3 + (cb & 0b0001_1100)
            } else {
                cb
            };
            let byte = |v: u16| (32u16 + v).min(255) as u8;
            vec![0x1b, b'[', b'M', byte(cb), byte(col), byte(row)]
        }
    }
}

fn reportable(mode: TrackMode, kind: &MouseEventKind) -> bool {
    use MouseEventKind::*;
    match kind {
        Down(_) => mode != TrackMode::None,
        Up(_) => matches!(
            mode,
            TrackMode::PressRelease | TrackMode::ButtonMotion | TrackMode::AnyMotion
        ),
        Drag(_) => matches!(mode, TrackMode::ButtonMotion | TrackMode::AnyMotion),
        Moved => mode == TrackMode::AnyMotion,
        ScrollUp | ScrollDown => mode != TrackMode::None,
        ScrollLeft | ScrollRight => false,
    }
}

pub fn pty_mouse_bytes(
    ev: &MouseEvent,
    mode: TrackMode,
    enc: ReportEncoding,
    rect: Rect,
) -> Option<Vec<u8>> {
    if mode == TrackMode::None || !reportable(mode, &ev.kind) {
        return None;
    }
    use ratatui::crossterm::event::KeyModifiers;
    let (cx, cy) = clamp_point(rect, ev.column, ev.row);
    let col1 = cx - rect.x + 1;
    let row1 = cy - rect.y + 1;
    let map_btn = |b: MouseButton| match b {
        MouseButton::Left => ReportButton::Left,
        MouseButton::Middle => ReportButton::Middle,
        MouseButton::Right => ReportButton::Right,
    };
    let (button, motion, release) = match ev.kind {
        MouseEventKind::Down(b) => (map_btn(b), false, false),
        MouseEventKind::Up(b) => (map_btn(b), false, true),
        MouseEventKind::Drag(b) => (map_btn(b), true, false),
        MouseEventKind::Moved => (ReportButton::None, true, false),
        MouseEventKind::ScrollUp => (ReportButton::WheelUp, false, false),
        MouseEventKind::ScrollDown => (ReportButton::WheelDown, false, false),
        MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight => return None,
    };
    let report = Report {
        button,
        motion,
        release,
        shift: ev.modifiers.contains(KeyModifiers::SHIFT),
        alt: ev.modifiers.contains(KeyModifiers::ALT),
        ctrl: ev.modifiers.contains(KeyModifiers::CONTROL),
        col1,
        row1,
    };
    Some(encode_report(&report, enc))
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/mouse__tests.rs"]
mod tests;
