//! The engine's cells as glass cells, with the selection shown reversed,
//! and the selected text and words read back off them.

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Position, Rect};
use ratatui_core::style::{Color, Modifier};

use crate::engine::{Cell, Color as EngineColor, Coord, Engine};
use alacritty_terminal::term::cell::Flags as F;

/// How the cursor shows, as glass draws it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorLook {
    /// A block: the cell's colours reverse under it.
    Block,
    /// An underline along the bottom of the cell.
    Underline,
    /// A bar on the left edge of the cell.
    Beam,
    /// Not drawn at all.
    Hidden,
}

/// A stretch of the screen as it reads, from one cell to another (both
/// included), in the cells shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    /// (row, column) where it began.
    pub anchor: (u16, u16),
    /// (row, column) where it reaches now.
    pub head: (u16, u16),
}

impl Selection {
    pub fn at(row: u16, col: u16) -> Self {
        Self { anchor: (row, col), head: (row, col) }
    }

    pub fn line(row: u16, cols: u16) -> Self {
        Self { anchor: (row, 0), head: (row, cols.saturating_sub(1)) }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    fn ordered(&self) -> ((u16, u16), (u16, u16)) {
        if self.anchor <= self.head { (self.anchor, self.head) } else { (self.head, self.anchor) }
    }

    fn contains(&self, row: u16, col: u16) -> bool {
        let (start, end) = self.ordered();
        (start..=end).contains(&(row, col))
    }
}

fn color(color: EngineColor) -> Color {
    match color {
        EngineColor::Default => Color::Reset,
        EngineColor::Indexed(i) => Color::Indexed(i),
        EngineColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

/// Draw the screen. `hover` is the link under the mouse, when Ctrl is
/// held: its cells show underlined.
pub fn paint(
    buf: &mut Buffer,
    area: Rect,
    engine: &mut Engine,
    selection: Option<&Selection>,
    hover: Option<&Selection>,
) {
    let (rows, cols) = engine.size();
    let cells = engine.cells();
    for row in 0..area.height.min(u16::try_from(rows).unwrap_or(u16::MAX)) {
        for col in 0..area.width.min(u16::try_from(cols).unwrap_or(u16::MAX)) {
            let Some(cell) = buf.cell_mut((area.x + col, area.y + row)) else {
                continue;
            };
            cell.reset();
            let index = usize::from(row) * cols + usize::from(col);
            let Some(source) = cells.get(index) else {
                continue;
            };
            // The right half of a wide character is the left half's.
            if source.flags.contains(alacritty_terminal::term::cell::Flags::WIDE_CHAR_SPACER)
                && !source.flags.contains(alacritty_terminal::term::cell::Flags::WIDE_CHAR)
            {
                continue;
            }
            // A placeholder cell is a picture, never a glyph. The background stays.
            let symbol = if source.c == '\0' || source.c == '\u{10EEEE}' { ' ' } else { source.c };
            cell.set_char(symbol);
            cell.fg = color(source.fg);
            cell.bg = color(source.bg);
            let mut modifier = Modifier::empty();
            use alacritty_terminal::term::cell::Flags as F;
            modifier.set(Modifier::BOLD, source.flags.contains(F::BOLD));
            modifier.set(Modifier::DIM, source.flags.contains(F::DIM));
            modifier.set(Modifier::ITALIC, source.flags.contains(F::ITALIC));
            // Every underline style, including curly (SGR 4:3), is an underline.
            // SGR 58's colour rides the cell so the bar is not stuck on the fg.
            modifier.set(Modifier::UNDERLINED, source.flags.intersects(F::ALL_UNDERLINES));
            modifier.set(Modifier::CROSSED_OUT, source.flags.contains(F::STRIKEOUT));
            let selected = selection.is_some_and(|s| s.contains(row, col));
            modifier.set(Modifier::REVERSED, source.flags.contains(F::INVERSE) != selected);
            if hover.is_some_and(|h| h.contains(row, col)) && !selected {
                modifier.insert(Modifier::UNDERLINED);
            }
            cell.modifier = modifier;
            if let Some(underline) = match source.underline {
                crate::engine::Color::Default => None,
                other => Some(color(other)),
            } {
                cell.underline_color = underline;
            }
        }
    }
}

/// Where the cursor shows, if it does: not while the program hides it, and
/// not while the view is in the scrollback.
pub fn cursor(engine: &Engine, area: Rect) -> Option<Position> {
    let (row, col): Coord = engine.cursor()?;
    let (rows, cols) = engine.size();
    if usize::from(row) >= rows || usize::from(col) >= cols {
        return None;
    }
    let (x, y) = (area.x + col.min(area.width.saturating_sub(1)), area.y + row.min(area.height.saturating_sub(1)));
    Some(Position { x, y })
}

/// The selected text, as a terminal copies it: trailing blanks trimmed,
/// rows that wrapped join up, others break into lines.
pub fn text(cells: &[Cell], cols: u16, selection: &Selection) -> String {
    let ((r0, c0), (r1, c1)) = selection.ordered();
    let mut out = String::new();
    let mut row = r0;
    while row <= r1 {
        let last_col = if row == r1 { c1.min(cols.saturating_sub(1)) } else { cols.saturating_sub(1) };
        let mut line = String::new();
        let mut col = if row == r0 { c0 } else { 0 };
        while col <= last_col {
            if let Some(cell) = cell_at(cells, cols, row, col)
                && cell.c != '\0'
                && cell.c != '\u{10EEEE}'
            {
                line.push(cell.c);
            }
            col += 1;
        }
        // A row that the program wrapped carries on to the next; one that
        // ended breaks the line. Only a row that ends a line has its
        // trailing blanks trimmed: a wrapped row's blanks are text.
        let wrapped = cell_at(cells, cols, row, cols.saturating_sub(1)).is_some_and(|c| c.flags.contains(F::WRAPLINE));
        out.push_str(if wrapped { &line } else { line.trim_end_matches(' ') });
        if !wrapped && row != r1 {
            out.push('\n');
        }
        if row == u16::MAX {
            break;
        }
        row += 1;
    }
    out
}

/// The word under a double click: a run of cells that are not blanks or
/// the punctuation that usually bounds a word, a path or a URL.
pub fn word(cells: &[Cell], cols: u16, row: u16, col: u16) -> Selection {
    let is_word = |c: char| !c.is_whitespace() && !",│`|\"' ()[]{}<>\t;".contains(c);
    let Some(start) = cell_at(cells, cols, row, col).map(|c| c.c) else {
        return Selection::at(row, col);
    };
    if !is_word(start) {
        return Selection::at(row, col);
    }
    let mut left = col;
    while left > 0 && cell_at(cells, cols, row, left - 1).is_some_and(|c| is_word(c.c)) {
        left -= 1;
    }
    let mut right = col;
    while cell_at(cells, cols, row, right + 1).is_some_and(|c| is_word(c.c)) && right < u16::MAX {
        right += 1;
    }
    Selection { anchor: (row, left), head: (row, right) }
}

/// The plain URL sitting over a cell, if one does (http, https, file).
pub fn url_at(cells: &[Cell], cols: u16, row: u16, col: u16) -> Option<String> {
    let mut line = String::new();
    let mut c = 0;
    while let Some(cell) = cell_at(cells, cols, row, c) {
        line.push(cell.c);
        c += 1;
    }
    let target = usize::from(col);
    for schema in ["https://", "http://", "file://"] {
        let mut from = 0;
        while let Some(found) = line[from..].find(schema) {
            let start = from + found;
            let end = line[start..]
                .find(|c: char| c.is_whitespace() || c == '<' || c == '>' || c == '"')
                .map_or(line.len(), |n| start + n);
            if (start..end).contains(&target) {
                return Some(line[start..end].to_string());
            }
            from = end.max(start + schema.len());
        }
    }
    None
}

/// The cell at (row, col) of a flat `rows x cols` grid, when the grid's
/// width is known: callers pass the engine's, so this only fails off the
/// edges (a wide character's right half reads as the left's).
fn cell_at(cells: &[Cell], cols: u16, row: u16, col: u16) -> Option<&Cell> {
    if col >= cols {
        return None;
    }
    cells.get(usize::from(row) * usize::from(cols) + usize::from(col))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::HostInfo;

    fn engine(input: &[u8]) -> Engine {
        let mut engine = Engine::new(4, 10, HostInfo { colors: [[0; 3]; 3], cell_px: (1, 1) });
        engine.process(input);
        engine
    }

    fn text_of(engine: &mut Engine, selection: &Selection) -> String {
        let cols = u16::try_from(engine.size().1).unwrap_or(u16::MAX);
        text(engine.cells(), cols, selection)
    }

    #[test]
    fn selection_reads_back_as_lines() {
        let mut parser = engine(b"one  \r\ntwo three");
        let all = Selection { anchor: (1, 9), head: (0, 0) };
        assert_eq!(text_of(&mut parser, &all), "one\ntwo three");
        let part = Selection { anchor: (1, 4), head: (1, 6) };
        assert_eq!(text_of(&mut parser, &part), "thr");
    }

    #[test]
    fn wrapped_rows_join_up() {
        let mut parser = engine(b"0123456789abc");
        let all = Selection { anchor: (0, 0), head: (1, 9) };
        assert_eq!(text_of(&mut parser, &all), "0123456789abc");
    }

    #[test]
    fn cjk_and_emoji_copy_once() {
        // A wide character takes two cells; the copy takes the character
        // once and skips the null that marks its right half.
        let mut parser = engine("你好🎉🎊".as_bytes());
        let all = Selection { anchor: (0, 0), head: (0, 7) };
        assert_eq!(text_of(&mut parser, &all), "你好🎉🎊");
    }

    #[test]
    fn wrapped_rows_keep_their_trailing_spaces() {
        // `a` followed by three spaces wraps onto `b`: the spaces are part
        // of the text, so the copy keeps them. Only a row that ends a line
        // has its trailing blanks trimmed.
        let mut parser = engine(b"a        bcd");
        // `a` and eight spaces fill the first row and it wraps: the
        // spaces are copied as written, not trimmed, because the row
        // carries on instead of ending a line.
        let all = Selection { anchor: (0, 0), head: (1, 1) };
        assert_eq!(text_of(&mut parser, &all), "a        bcd");
    }

    #[test]
    fn selection_stops_at_the_last_column() {
        // A selection whose head sits past the grid reads to the last
        // column of the middle rows and no further, instead of spinning up
        // to the column limit.
        let mut parser = engine(b"ab");
        let wide = Selection { anchor: (0, 0), head: (0, u16::MAX) };
        assert_eq!(text_of(&mut parser, &wide), "ab");
    }

    #[test]
    fn double_click_takes_the_word() {
        let mut parser = engine(b"cd ~/a.b; x");
        assert_eq!({ let w = word(parser.cells(), 10, 0, 5); text_of(&mut parser, &w) }, "~/a.b");
        { let w = word(parser.cells(), 10, 0, 2); assert!(w.is_empty()); }
    }

    #[test]
    fn cells_carry_colour_and_selection() {
        let mut parser = engine(b"\x1b[1;31mA\x1b[0m\x1b[7mB");
        let area = Rect::new(0, 0, 10, 4);
        let mut buf = Buffer::empty(area);
        paint(&mut buf, area, &mut parser, Some(&Selection::at(0, 1)), None);
        let a = &buf[(0, 0)];
        assert_eq!((a.symbol(), a.fg), ("A", Color::Indexed(1)));
        assert!(a.modifier.contains(Modifier::BOLD));
        // Reversed by the program and selected: shown plain.
        assert!(!buf[(1, 0)].modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn curly_and_coloured_underlines_reach_the_cells() {
        let mut parser = engine(b"\x1b[4:3m\x1b[58;2;255;80;40mX");
        let area = Rect::new(0, 0, 4, 1);
        let mut buf = Buffer::empty(area);
        paint(&mut buf, area, &mut parser, None, None);
        let cell = &buf[(0, 0)];
        assert!(
            cell.modifier.contains(Modifier::UNDERLINED),
            "SGR 4:3 is an underline"
        );
        assert_eq!(cell.underline_color, Color::Rgb(255, 80, 40));
    }
}
