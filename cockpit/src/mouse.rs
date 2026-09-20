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

/// Content surfaces that can be hit-tested. Clipboard selection policy is
/// stricter: only [`PaneId::Transcript`] is eligible. Chrome (the 1-row header
/// and footer, pane borders, titles) is intentionally excluded from every rect.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PaneId {
    Transcript,
    AgentBay,
    Artifacts,
    Input,
    Shell,
}

/// Interactive content rects observed on the last draw, in absolute buffer
/// coordinates and already shrunk *inside* any border. Rebuilt fresh every frame
/// in `ui()`; copy authorization is enforced separately by the app.
#[derive(Clone, Debug, Default)]
pub struct PaneRegistry {
    panes: Vec<(PaneId, Rect)>,
}

impl PaneRegistry {
    pub fn clear(&mut self) {
        self.panes.clear();
    }

    /// Register a pane's *content* rect (empty rects are ignored). Panes pushed
    /// later are treated as "on top" for hit-testing.
    pub fn push(&mut self, id: PaneId, rect: Rect) {
        if rect.width > 0 && rect.height > 0 {
            self.panes.push((id, rect));
        }
    }

    /// Hit-test a point; the last-registered (topmost) pane containing it wins.
    pub fn pane_at(&self, x: u16, y: u16) -> Option<(PaneId, Rect)> {
        self.panes
            .iter()
            .rev()
            .find(|(_, r)| point_in(*r, x, y))
            .copied()
    }

    /// The content rect registered for `id` this frame. Last registration wins
    /// (same stacking rule as [`Self::pane_at`]) so a later, tighter transcript
    /// prose rect is not shadowed by an earlier full-body registration.
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

/// Is the cell `(x, y)` inside `r`?
pub fn point_in(r: Rect, x: u16, y: u16) -> bool {
    x >= r.x && x < r.x.saturating_add(r.width) && y >= r.y && y < r.y.saturating_add(r.height)
}

/// Clamp a point into `rect` so a drag that wanders outside the pane stays
/// pinned to its edge — the selection can never escape the pane.
pub fn clamp_point(rect: Rect, x: u16, y: u16) -> (u16, u16) {
    let last_x = rect.right().saturating_sub(1).max(rect.x);
    let last_y = rect.bottom().saturating_sub(1).max(rect.y);
    (x.clamp(rect.x, last_x), y.clamp(rect.y, last_y))
}

/// An in-progress / completed freeform selection, confined to a single pane.
/// `anchor`/`cursor` are absolute cells, always pre-clamped into `rect`.
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

    /// Extend the selection to a new cursor point (clamped to the pane).
    pub fn extend(&mut self, x: u16, y: u16) {
        self.cursor = clamp_point(self.rect, x, y);
    }

    /// Keep the selection locked to the live pane geometry for this frame.
    /// Layout can change under a drag (reasoning height, resize); never let a
    /// stale larger rect paint reverse-video across neighboring panels.
    ///
    /// Returns `false` when the selection should be dropped: empty live rect,
    /// none of the currently selected cells fall inside the new prose (e.g. a
    /// single-row pick that lived only in the strip that became a side column),
    /// or a previously multi-cell pick that *collapses* onto one cell after
    /// clamping (phantom reverse-video / one-glyph clipboard). Callers must not
    /// edge-pin those — that invents a false highlight of unrelated text.
    ///
    /// Brand-new clicks start empty while `dragging`; those may rebind empty.
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
        // A1: layout collapse that pins both ends to the same edge cell leaves a
        // stuck white cell and can copy a single wrong glyph on the next draw.
        if self.is_empty() {
            return was_empty && self.dragging;
        }
        true
    }

    /// `(start, end)` ordered in reading order (row-major, then column).
    pub fn ordered(&self) -> ((u16, u16), (u16, u16)) {
        order_points(self.anchor, self.cursor)
    }

    /// A single-cell (zero-width) selection — nothing to copy.
    pub fn is_empty(&self) -> bool {
        self.anchor == self.cursor
    }

    /// Whether this selection is still owned by a live pointer drag. Completed
    /// selections stay highlighted, but must not keep stealing later mouse
    /// events from a mouse-aware program in the shell pane.
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    pub fn finish(&mut self) {
        self.dragging = false;
    }
}

/// Order two `(x, y)` points in reading order (compare row first, then column).
pub fn order_points(a: (u16, u16), b: (u16, u16)) -> ((u16, u16), (u16, u16)) {
    if (a.1, a.0) <= (b.1, b.0) {
        (a, b)
    } else {
        (b, a)
    }
}

/// The selected inclusive column span `(y, x0, x1)` for each row, as a *linear*
/// (text-flow) selection clamped to the pane rect — exactly how a terminal
/// selects text: from the anchor cell to the end of its row, full rows through
/// the middle, then up to the cursor cell. Never crosses the pane boundary.
#[cfg(test)]
pub fn selection_spans(sel: &Selection) -> Vec<(u16, u16, u16)> {
    selection_spans_raw(sel, sel.rect)
}

/// Like [`selection_spans`], but forced into `clip` so a stale selection rect
/// cannot reverse-video cells outside the live transcript prose surface.
///
/// A1: if none of the currently selected cells fall inside `clip`, return
/// empty — never clamp endpoints onto the clip edge (that invented a phantom
/// reverse-video / one-glyph copy of unrelated prose).
pub fn selection_spans_in(sel: &Selection, clip: Rect) -> Vec<(u16, u16, u16)> {
    // A1: zero-width (anchor == cursor) is not a highlight/copy source — match
    // extract/highlight policy so a click without drag never reverse-videos one cell.
    if sel.is_empty() {
        return Vec::new();
    }
    // Same gate as [`Selection::rebind`]: geometry intersection of rects is not
    // enough when the pick lived only in columns the live prose no longer owns.
    // Use raw spans (not this function) so the intersect check cannot recurse.
    if !selection_cells_intersect(sel, clip) {
        return Vec::new();
    }
    selection_spans_raw(sel, clip)
}

/// Core linear spans clamped into `clip` without the disjoint-pick gate.
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

/// True when any cell of `sel`'s linear spans lies inside `rect` (no clamping
/// of endpoints onto `rect`'s edge — used to decide whether a layout rebind
/// still has something real to show).
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

/// Axis-aligned intersection of two rects; empty if they do not overlap.
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

/// Read the selected text out of the *rendered* buffer: row by row, trailing
/// spaces trimmed, rows joined with `\n`, trailing blank lines dropped. Because
/// it reads the buffer the user is looking at and is clamped to the pane's
/// content rect, the result is exactly that pane's visible text, chrome-free.
#[cfg(test)]
pub fn extract_text(buf: &Buffer, sel: &Selection) -> String {
    extract_text_in(buf, sel, sel.rect)
}

/// True when column `x` on row `y` is a cleared trailing cell of a wide
/// grapheme whose lead sits at a smaller column (Ratatui layout, not prose).
///
/// A1: a selection that *starts* on such a cell used to copy a phantom space
/// (the cleared continuation looks like a real width-1 space by symbol alone).
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

/// Like [`extract_text`], but hard-clipped to `clip` with the same
/// [`selection_spans_in`] gate as highlight (A1: zero-width picks copy nothing;
/// disjoint clips never invent an edge glyph).
pub fn extract_text_in(buf: &Buffer, sel: &Selection, clip: Rect) -> String {
    // Policy matches `Selection::is_empty`: a zero-width (anchor == cursor) pick
    // is not a copy source even though a linear span of one cell could be built.
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
            // Selection can start mid-wide-glyph (click/rebind on a trailing
            // cleared cell). Skip those layout pads without emitting a space.
            if cell_is_wide_grapheme_continuation(buf, x, *y) {
                continue;
            }
            if let Some(cell) = buf.cell((x, *y)) {
                let symbol = cell.symbol();
                row.push_str(symbol);
                // Ratatui stores a wide grapheme in its leading cell and clears
                // the following display cells to spaces. Those continuation
                // cells are layout, not text; skip exactly the lead symbol's
                // remaining display width while retaining genuine width-1 spaces.
                continuation_cells = UnicodeWidthStr::width(symbol).saturating_sub(1);
            }
        }
        out.push_str(row.trim_end_matches(' '));
    }
    out.trim_end_matches('\n').to_string()
}

/// Paint the selected cells reverse-video, in place, on the rendered buffer.
#[cfg(test)]
pub fn highlight(buf: &mut Buffer, sel: &Selection) {
    highlight_in(buf, sel, sel.rect);
}

/// Paint selection reverse-video, hard-clipped to `clip` (live transcript rect).
///
/// A1: zero-width picks paint nothing (same policy as [`extract_text_in`]).
pub fn highlight_in(buf: &mut Buffer, sel: &Selection, clip: Rect) {
    if sel.is_empty() {
        return;
    }
    let reversed = Style::new().add_modifier(Modifier::REVERSED);
    let buf_right = buf.area().right();
    let buf_bottom = buf.area().bottom();
    for (y, x0, x1) in selection_spans_in(sel, clip) {
        // Absolute belt: never touch cells outside the clip or the buffer.
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

/// Shrink `area` inside a `Borders::ALL` block to its content rect (mirrors
/// `Block::inner` for an all-borders block). Used to register chrome-free pane
/// rects without reconstructing each pane's block.
pub fn inner_border(area: Rect) -> Rect {
    Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    }
}

// ---------------------------------------------------------------------------
// Clipboard (OSC-52)
// ---------------------------------------------------------------------------

/// Many terminals cap the base64 body of an OSC-52 request. This limit applies
/// to the encoded payload, not the smaller source text. Larger selections still
/// reach the file fallback (see `app_control`).
pub const OSC52_MAX_PAYLOAD_BYTES: usize = 74_994;

/// Base64 length without allocating the encoded payload.
pub fn base64_clipboard_len(text: &str) -> Option<usize> {
    text.len()
        .checked_add(2)
        .map(|bytes| bytes / 3)
        .and_then(|groups| groups.checked_mul(4))
}

pub fn osc52_payload_fits(text: &str) -> bool {
    base64_clipboard_len(text).is_some_and(|len| len <= OSC52_MAX_PAYLOAD_BYTES)
}

/// Base64 (standard, padded) of `text` — the OSC-52 clipboard payload body.
pub fn base64_clipboard(text: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(text.as_bytes())
}

/// The full OSC-52 *set-clipboard* escape sequence for the system clipboard
/// (`c`): `ESC ] 52 ; c ; <base64> BEL`. Supporting terminals forward it to
/// the system clipboard.
pub fn osc52_sequence(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64_clipboard(text))
}

/// Shift-drag is the conventional escape hatch for selecting shell text while
/// vim/htop/etc. own ordinary mouse input. Once started, the drag remains
/// app-owned through release even if the terminal omits Shift on later reports.
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

// ---------------------------------------------------------------------------
// PTY mouse forwarding (so vim/htop/etc. in the shell pane keep working)
// ---------------------------------------------------------------------------

/// The xterm mouse-tracking mode a PTY program has requested (mirrors
/// `vt100::MouseProtocolMode`, kept local so this module stays vt100-free and
/// trivially testable).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TrackMode {
    None,
    Press,
    PressRelease,
    ButtonMotion,
    AnyMotion,
}

/// The encoding a PTY program asked mouse reports to use.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReportEncoding {
    /// Legacy X10 single-byte encoding (`ESC [ M ...`).
    X10,
    /// SGR 1006 encoding (`ESC [ < ... M/m`).
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
            // Legacy: ESC [ M (32+cb) (32+col) (32+row); release reports button 3
            // (keeping the modifier bits) since X10 can't name the button.
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

/// Should an event of this `kind` be reported to a program tracking in `mode`?
/// (Matches xterm: press needs any tracking; release needs ≥ VT200; drag needs
/// button-motion; bare motion needs any-motion; the wheel reports in all modes.)
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

/// Translate a crossterm mouse event into the bytes a PTY program tracking the
/// mouse expects on stdin. `rect` is the shell pane's content rect; coordinates
/// are made pane-relative + 1-based (the program thinks the pane is its screen).
/// Returns `None` when this event isn't reportable in `mode` (so the caller can
/// fall through to app-selection over a shell that isn't tracking the mouse).
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
#[path = "../../tests/cockpit/app/mouse__tests.rs"]
mod tests;
