//! `GlassBackend`: ratatui draws into it exactly as it draws into a terminal.
//!
//! The cockpit already produces a grid of cells every frame, so there is no
//! terminal to emulate: changed cells land in the backend's own grid, and
//! `flush` hands a snapshot of it to the render thread.

use std::io;

use ratatui_core::backend::{Backend, ClearType, WindowSize};
use ratatui_core::buffer::Cell;
use ratatui_core::layout::{Position, Size};
use unicode_width::UnicodeWidthStr;

use crate::{Host, LockExt, UserEvent};

/// A cell symbol's width in columns. Printable ASCII and the built-in
/// box-drawing (`U+2500..=U+257F`), block (`U+2580..=U+259F`) and braille
/// (`U+2800..=U+28FF`) ranges — nearly every cell, nearly every frame —
/// are one column by definition and never need the Unicode table.
#[inline]
pub(crate) fn symbol_width(symbol: &str) -> usize {
    let b = symbol.as_bytes();
    if (b.len() == 1 && b[0] >= 0x20)
        || (b.len() == 3
            && b[0] == 0xe2
            && (matches!(b[1], 0x94 | 0x95 | 0xa0..=0xa3) || (b[1] == 0x96 && b[2] < 0xa0)))
    {
        1
    } else {
        UnicodeWidthStr::width(symbol)
    }
}

pub struct GlassBackend {
    host: Host,
    cells: Vec<Cell>,
    cols: u16,
    rows: u16,
    cursor: Position,
    cursor_visible: bool,
    flushed_cursor: Option<(u16, u16)>,
    changed: bool,
    /// Half-open `[start, end)` cell index span modified since the last flush.
    dirty_curr: Option<(usize, usize)>,
    /// Half-open `[start, end)` cell index span modified between `revision - 1`
    /// and `revision`, used when the render thread swaps its `t - 1` snapshot back.
    dirty_prev: Option<(usize, usize)>,
    revision: u64,
    /// A cell carried terminal escape codes (a picture made for a terminal
    /// emulator): said once, then such cells stay blank.
    escapes_seen: bool,
}

#[inline]
fn union_span(a: Option<(usize, usize)>, b: Option<(usize, usize)>) -> Option<(usize, usize)> {
    match (a, b) {
        (Some((l0, r0)), Some((l1, r1))) => Some((l0.min(l1), r0.max(r1))),
        (Some(s), None) | (None, Some(s)) => Some(s),
        (None, None) => None,
    }
}

impl GlassBackend {
    pub(crate) fn new(host: Host) -> Self {
        let mut backend = Self {
            host,
            cells: Vec::new(),
            cols: 0,
            rows: 0,
            cursor: Position::ORIGIN,
            cursor_visible: false,
            flushed_cursor: None,
            changed: true,
            dirty_curr: None,
            dirty_prev: None,
            revision: 0,
            escapes_seen: false,
        };
        backend.fit();
        backend
    }

    #[inline]
    fn mark_dirty(&mut self, start: usize, end: usize) {
        if start < end {
            self.dirty_curr = union_span(self.dirty_curr, Some((start, end)));
        }
        self.changed = true;
    }

    /// Match the grid to the window's current size.
    fn fit(&mut self) {
        let grid = *self.host.shared.grid.lock_recover();
        if grid.cols != self.cols || grid.rows != self.rows {
            self.cols = grid.cols;
            self.rows = grid.rows;
            let len = usize::from(self.cols) * usize::from(self.rows);
            self.cells = vec![Cell::default(); len];
            self.dirty_curr = Some((0, len));
            self.dirty_prev = Some((0, len));
            self.changed = true;
        }
    }

    fn index(&self, x: u16, y: u16) -> usize {
        usize::from(y) * usize::from(self.cols) + usize::from(x)
    }

    /// One cell, and the cells a wide symbol covers, as on a terminal:
    /// ratatui does not send a covered cell again.
    fn put(&mut self, x: u16, y: u16, cell: Cell) -> usize {
        let i = self.index(x, y);
        let width = symbol_width(cell.symbol()).max(1);
        let max_dx = width.min(usize::from(self.cols).saturating_sub(usize::from(x)));
        self.cells[i] = cell;
        for dx in 1..max_dx {
            self.cells[i + dx] = Cell::default();
        }
        self.mark_dirty(i, i + max_dx.max(1));
        width
    }

    fn blank(&mut self, range: std::ops::Range<usize>) {
        let end = range.end.min(self.cells.len());
        let start = range.start.min(end);
        for cell in &mut self.cells[start..end] {
            *cell = Cell::default();
        }
        self.mark_dirty(start, end);
    }
}

impl Backend for GlassBackend {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        for (x, y, cell) in content {
            if x >= self.cols || y >= self.rows {
                continue;
            }
            if cell.symbol().contains('\x1b') {
                // The window draws cells, not escape codes: a cell carrying
                // them (a picture made for a terminal) shows nothing.
                if !self.escapes_seen {
                    self.escapes_seen = true;
                    eprintln!("GLASS: a cell carried terminal escape codes; it is drawn blank");
                }
                let mut blank = cell.clone();
                blank.set_symbol(" ");
                self.put(x, y, blank);
            } else {
                self.put(x, y, cell.clone());
            }
        }
        Ok(())
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.cursor_visible = false;
        Ok(())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.cursor_visible = true;
        Ok(())
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        Ok(self.cursor)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.cursor = position.into();
        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        self.fit();
        self.blank(0..self.cells.len());
        Ok(())
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        let x = self.cursor.x.min(self.cols.saturating_sub(1));
        let y = self.cursor.y.min(self.rows.saturating_sub(1));
        let here = self.index(x, y);
        let line_start = self.index(0, y);
        let line_end = line_start + usize::from(self.cols);
        match clear_type {
            ClearType::All => return self.clear(),
            ClearType::AfterCursor => self.blank(here..self.cells.len()),
            ClearType::BeforeCursor => self.blank(0..here + 1),
            ClearType::CurrentLine => self.blank(line_start..line_end),
            ClearType::UntilNewLine => self.blank(here..line_end),
        }
        Ok(())
    }

    fn size(&self) -> io::Result<Size> {
        let grid = *self.host.shared.grid.lock_recover();
        Ok(Size::new(grid.cols, grid.rows))
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        let grid = *self.host.shared.grid.lock_recover();
        Ok(WindowSize {
            columns_rows: Size::new(grid.cols, grid.rows),
            pixels: Size::new(
                u16::try_from(u32::from(grid.cols) * grid.cell_width).unwrap_or(u16::MAX),
                u16::try_from(u32::from(grid.rows) * grid.cell_height).unwrap_or(u16::MAX),
            ),
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        let pictures = self
            .host
            .shared
            .pictures_waiting
            .swap(false, std::sync::atomic::Ordering::AcqRel);
        if self.host.shared.scroll_changed.swap(false, std::sync::atomic::Ordering::AcqRel) {
            self.changed = true;
        }
        let cursor = self
            .cursor_visible
            .then_some((self.cursor.x, self.cursor.y));
        if cursor != self.flushed_cursor {
            self.changed = true;
        }
        if !self.changed {
            // Same cells, new picture: still a frame to present.
            if pictures {
                self.host.send(UserEvent::Pictures);
            }
            return Ok(());
        }
        self.changed = false;
        self.flushed_cursor = cursor;
        let dirty_curr = self.dirty_curr.take();
        {
            let scroll = self.host.shared.scroll.lock_recover().clone();
            let mut slot = self.host.shared.slot.lock_recover();
            sync_into_slot(
                &self.cells,
                self.cols,
                self.rows,
                cursor,
                scroll,
                &mut self.revision,
                &mut self.dirty_prev,
                dirty_curr,
                &mut slot,
            );
        }
        self.host.send(UserEvent::Frame);
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn sync_into_slot(
    cells: &[Cell],
    cols: u16,
    rows: u16,
    cursor: Option<(u16, u16)>,
    scroll: Option<std::sync::Arc<crate::scroll::Page>>,
    revision: &mut u64,
    dirty_prev: &mut Option<(usize, usize)>,
    dirty_curr: Option<(usize, usize)>,
    slot: &mut crate::Slot,
) {
    slot.snapshot.scroll = scroll;
    let same_dims = slot.snapshot.cols == cols
        && slot.snapshot.rows == rows
        && slot.snapshot.cells.len() == cells.len();
    if same_dims && *revision > 0 && slot.snapshot.revision == *revision {
        // The render thread has not swapped `slot.snapshot` since our
        // previous flush: copy only `dirty_curr` and accumulate into `dirty_prev`.
        if let Some((start, end)) = dirty_curr {
            let end = end.min(cells.len());
            let start = start.min(end);
            slot.snapshot.cells[start..end].clone_from_slice(&cells[start..end]);
        }
        *dirty_prev = union_span(*dirty_prev, dirty_curr);
    } else if same_dims && *revision > 1 && slot.snapshot.revision == *revision - 1 {
        // The render thread swapped its `t - 1` buffer back into `slot.snapshot`:
        // copy only the union of `dirty_prev` and `dirty_curr`.
        if let Some((start, end)) = union_span(*dirty_prev, dirty_curr) {
            let end = end.min(cells.len());
            let start = start.min(end);
            slot.snapshot.cells[start..end].clone_from_slice(&cells[start..end]);
        }
        *revision += 1;
        slot.snapshot.revision = *revision;
        *dirty_prev = dirty_curr;
    } else {
        slot.snapshot.cells.clone_from_slice_or_vec(cells);
        *revision += 1;
        slot.snapshot.revision = *revision;
        *dirty_prev = dirty_curr;
    }
    slot.snapshot.cols = cols;
    slot.snapshot.rows = rows;
    slot.snapshot.cursor = cursor;
    slot.fresh = true;
}

trait VecCloneExt<T: Clone> {
    fn clone_from_slice_or_vec(&mut self, src: &[T]);
}

impl<T: Clone> VecCloneExt<T> for Vec<T> {
    #[inline]
    fn clone_from_slice_or_vec(&mut self, src: &[T]) {
        if self.len() == src.len() {
            self.clone_from_slice(src);
        } else {
            self.clear();
            self.extend_from_slice(src);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fast_path_agrees_with_the_table() {
        for symbol in ["a", " ", "!", "~", "\t", "\u{7f}", "é", "界", "🦀", "e\u{301}", "⣿", "─", ""] {
            assert_eq!(
                symbol_width(symbol),
                UnicodeWidthStr::width(symbol),
                "{symbol:?}"
            );
        }
        // Every printable ASCII byte, for completeness.
        for byte in 0x20u8..=0x7e {
            let s = (byte as char).to_string();
            assert_eq!(symbol_width(&s), 1, "{s:?}");
        }
        // Every box-drawing, block and braille character, for completeness.
        for code in (0x2500u32..=0x259f).chain(0x2800u32..=0x28ff) {
            let s = char::from_u32(code).unwrap().to_string();
            assert_eq!(
                symbol_width(&s),
                UnicodeWidthStr::width(s.as_str()),
                "U+{code:04X}"
            );
            assert_eq!(symbol_width(&s), 1, "U+{code:04X}");
        }
    }

    #[test]
    fn incremental_dirty_flush_matches_full_clone_across_double_buffer_swaps() {
        let (cols, rows) = (80u16, 24u16);
        let len = usize::from(cols) * usize::from(rows);
        let mut cells = vec![Cell::default(); len];
        let mut revision = 0u64;
        let mut dirty_prev = Some((0, len));
        let mut dirty_curr = Some((0, len));
        let mut slot = crate::Slot::default();
        let mut current = crate::Snapshot::default();

        for step in 0..60u16 {
            let x = (step * 7) % cols;
            let y = (step * 3) % rows;
            let idx = usize::from(y) * usize::from(cols) + usize::from(x);
            cells[idx].set_symbol(if step % 2 == 0 { "A" } else { "─" });
            dirty_curr = union_span(dirty_curr, Some((idx, idx + 1)));

            sync_into_slot(
                &cells,
                cols,
                rows,
                Some((x, y)),
                None,
                &mut revision,
                &mut dirty_prev,
                dirty_curr.take(),
                &mut slot,
            );

            assert_eq!(slot.snapshot.cells, cells, "mismatch before swap at step {step}");
            if step % 3 != 0 {
                std::mem::swap(&mut slot.snapshot, &mut current);
                slot.fresh = false;
                assert_eq!(current.cells, cells, "mismatch in swapped current at step {step}");
            }
        }
    }
}
