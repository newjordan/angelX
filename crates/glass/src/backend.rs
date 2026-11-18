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

#[inline]
pub(crate) fn symbol_width(symbol: &str) -> usize {
    let b = symbol.as_bytes();
    if (b.len() == 1 && b[0] >= 0x20)
        || (b.len() == 2 && matches!(b[0], 0xC3..=0xCB | 0xCE | 0xCF | 0xD0 | 0xD1 | 0xD3..=0xD5 | 0xDA))
        || (b.len() == 3
            && b[0] == 0xe2
            && (matches!(b[1], 0x94 | 0x95 | 0xa0..=0xa3) || (b[1] == 0x96 && b[2] < 0xa0)))
    {
        1
    } else if b.len() == 3 {
        wide_or_narrow_3(b, symbol)
    } else if b.len() == 4
        && b[0] == 0xF0
        && matches!(b[1], 0x97 | 0xA0..=0xAE | 0xB0..=0xBE)
    {
        2
    } else {
        UnicodeWidthStr::width(symbol)
    }
}

#[inline]
fn wide_or_narrow_3(b: &[u8], symbol: &str) -> usize {
    let b0 = b[0];
    if matches!(b0, 0xE4..=0xE9) {
        return 2;
    }
    let cp =
        (u32::from(b0 & 0x0F) << 12) | (u32::from(b[1] & 0x3F) << 6) | u32::from(b[2] & 0x3F);
    match b0 {
        0xE1 if (0x1100..=0x115F).contains(&cp) => 2,
        0xE2
            if (0x2000..=0x200A).contains(&cp) || (0x2010..=0x2029).contains(&cp) =>
        {
            1
        }
        0xE3
            if (0x3000..=0x3029).contains(&cp)
                || (0x3030..=0x303E).contains(&cp)
                || (0x3041..=0x3096).contains(&cp)
                || (0x309B..=0x309E).contains(&cp)
                || (0x30A0..=0x30FF).contains(&cp)
                || (0x3105..=0x312E).contains(&cp)
                || (0x3131..=0x3163).contains(&cp)
                  || (0x3165..=0x318E).contains(&cp)
                || (0x3190..=0x31E4).contains(&cp)
                || (0x3200..=0x321E).contains(&cp)
                  || (0x3220..=0x3247).contains(&cp)
                  || (0x3250..=0x32FF).contains(&cp)
                || (0x3400..=0x3FFF).contains(&cp) =>
        {
            2
        }
        0xEA if (0xA000..=0xA48C).contains(&cp)
            || (0xA490..=0xA4C6).contains(&cp)
            || (0xAC00..=0xAFFF).contains(&cp) =>
        {
            2
        }
        0xEB | 0xEC => 2,
        0xED if (0xD000..=0xD7A3).contains(&cp) => 2,
        0xEF if (0xFF01..=0xFF60).contains(&cp) || (0xFFE0..=0xFFE6).contains(&cp) => {
            2
        }
        0xEF if (0xFF61..=0xFF9D).contains(&cp) || (0xFFA1..=0xFFDF).contains(&cp) => 1,
        _ => UnicodeWidthStr::width(symbol),
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
    flushed_game: bool,
    cursor_style: crate::CursorStyle,
    flushed_cursor_style: Option<crate::CursorStyle>,
    changed: bool,
    dirty_curr: Option<(usize, usize)>,
    dirty_prev: Option<(usize, usize)>,
    revision: u64,
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
    pub fn set_cursor_style(&mut self, style: crate::CursorStyle) {
        if style != self.cursor_style {
            self.changed = true;
        }
        self.cursor_style = style;
    }

    pub(crate) fn new(host: Host) -> Self {
        let mut backend = Self {
            host,
            cells: Vec::new(),
            cols: 0,
            rows: 0,
            cursor: Position::ORIGIN,
            cursor_visible: false,
            flushed_cursor: None,
            flushed_game: false,
            cursor_style: crate::CursorStyle::default(),
            flushed_cursor_style: None,
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
        let game = self.host.shared.game_surface.load(std::sync::atomic::Ordering::Relaxed);
        if game != self.flushed_game { self.changed = true; }
        let pictures = self
            .host
            .shared
            .pictures_waiting
            .swap(false, std::sync::atomic::Ordering::AcqRel);
        if self.host.shared.card_changed.swap(false, std::sync::atomic::Ordering::AcqRel) {
            self.changed = true;
        }
        let cursor = self
            .cursor_visible
            .then_some((self.cursor.x, self.cursor.y));
        if cursor != self.flushed_cursor
            || self.flushed_cursor_style != Some(self.cursor_style)
        {
            self.changed = true;
        }
        if !self.changed {
            if pictures {
                self.host.send(UserEvent::Pictures);
            }
            return Ok(());
        }
        self.changed = false;
        self.flushed_cursor = cursor;
        self.flushed_game = game;
        self.flushed_cursor_style = Some(self.cursor_style);
        let dirty_curr = self.dirty_curr.take();
        {
            let card = self.host.shared.card.lock_recover().clone();
            let mut slot = self.host.shared.slot.lock_recover();
            sync_into_slot(
                &self.cells,
                self.cols,
                self.rows,
                cursor,
                self.cursor_style,
                card,
                &mut self.revision,
                &mut self.dirty_prev,
                dirty_curr,
                &mut slot,
            );
            slot.snapshot.game = game;
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
    cursor_style: crate::CursorStyle,
    card: Option<std::sync::Arc<crate::card::Card>>,
    revision: &mut u64,
    dirty_prev: &mut Option<(usize, usize)>,
    dirty_curr: Option<(usize, usize)>,
    slot: &mut crate::Slot,
) {
    slot.snapshot.card = card;
    slot.snapshot.cursor_style = cursor_style;
    let same_dims = slot.snapshot.cols == cols
        && slot.snapshot.rows == rows
        && slot.snapshot.cells.len() == cells.len();
    if same_dims && *revision > 0 && slot.snapshot.revision == *revision {
        if let Some((start, end)) = dirty_curr {
            let end = end.min(cells.len());
            let start = start.min(end);
            slot.snapshot.cells[start..end].clone_from_slice(&cells[start..end]);
        }
        *dirty_prev = union_span(*dirty_prev, dirty_curr);
    } else if same_dims && *revision > 1 && slot.snapshot.revision == *revision - 1 {
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
        for byte in 0x20u8..=0x7e {
            let s = (byte as char).to_string();
            assert_eq!(symbol_width(&s), 1, "{s:?}");
        }
        for code in (0x2500u32..=0x259f).chain(0x2800u32..=0x28ff) {
            let s = char::from_u32(code).unwrap().to_string();
            assert_eq!(
                symbol_width(&s),
                UnicodeWidthStr::width(s.as_str()),
                "U+{code:04X}"
            );
            assert_eq!(symbol_width(&s), 1, "U+{code:04X}");
        }
        for code in (0xc0u32..=0x2ff).chain(0x370..=0x4ff).chain(0x680..=0x6bf) {
            let Some(ch) = char::from_u32(code) else {
                continue;
            };
            let s = ch.to_string();
            if s.len() != 2 {
                continue;
            }
            let lead = s.as_bytes()[0];
            if !matches!(lead, 0xC3..=0xCB | 0xCE | 0xCF | 0xD0 | 0xD1 | 0xD3..=0xD5 | 0xDA) {
                continue;
            }
            assert_eq!(
                symbol_width(&s),
                UnicodeWidthStr::width(s.as_str()),
                "U+{code:04X}"
            );
            assert_eq!(symbol_width(&s), 1, "U+{code:04X}");
        }
        for symbol in ["e\u{301}", "ע", "م", " ", "­"] {
            assert_eq!(
                symbol_width(symbol),
                UnicodeWidthStr::width(symbol),
                "{symbol:?}"
            );
        }
        for code in (0x1100u32..=0x115f)
            .chain(0x2000..=0x2029)
            .chain(0x3000..=0x32ff)
            .chain(0x3400..=0x9fff)
            .chain(0xa000..=0xa4c6)
            .chain(0xac00..=0xd7a3)
            .chain(0xff01..=0xff60)
            .chain(0xff61..=0xffdf)
            .chain(0xffe0..=0xffe6)
        {
            let Some(ch) = char::from_u32(code) else {
                continue;
            };
            let s = ch.to_string();
            if s.len() != 3 {
                continue;
            }
            assert_eq!(
                symbol_width(&s),
                UnicodeWidthStr::width(s.as_str()),
                "U+{code:04X}"
            );
        }
    }

    #[test]
    fn the_fast_path_agrees_with_the_table_supplementary() {
        let mut checked = 0u32;
        for b1 in [0x97u8, 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7,
                   0xA8, 0xA9, 0xAA, 0xAB, 0xAC, 0xAD, 0xAE,
                   0xB0, 0xB1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7,
                   0xB8, 0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xBE] {
            for b2 in 0x80u8..=0xBFu8 {
                for b3 in 0x80u8..=0xBFu8 {
                    let bs = [0xF0u8, b1, b2, b3];
                    let Ok(s) = std::str::from_utf8(&bs) else {
                        continue;
                    };
                    assert_eq!(symbol_width(s), UnicodeWidthStr::width(s), "{bs:02X?}");
                    assert_eq!(symbol_width(s), 2, "{bs:02X?}");
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 126_976);
    }

    #[test]
    fn the_fast_path_agrees_with_the_table_everywhere() {
        for code in 0u32..=0xffff {
            let Some(ch) = char::from_u32(code) else {
                continue;
            };
            let s = ch.to_string();
            assert_eq!(
                symbol_width(&s),
                UnicodeWidthStr::width(s.as_str()),
                "U+{code:04X}"
            );
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
                crate::CursorStyle::default(),
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
