//! Unicode placeholder cells: a private-use character whose colour and
//! diacritics name a part of a virtual placement.
//!
//! The cells are ordinary text, so they scroll, reflow, clear and copy
//! with the text. This module only turns the visible ones into picture
//! parts. It draws nothing itself, and it does not touch the glyph.

use angel_stage::marks;

/// The private-use character a program prints where a picture part belongs.
pub const GLYPH: char = '\u{10EEEE}';

/// One visible cell, as the engine keeps it: the character, up to three
/// combining marks, and the colours that carry ids.
#[derive(Clone, Copy, Debug)]
pub struct CellRef {
    pub symbol: char,
    pub marks: [char; 3],
    /// Foreground as an image-id fragment: a palette index, or 24-bit RGB.
    /// `None` when the colour is the default and carries no id.
    pub fg: Option<u32>,
    /// Underline colour as a placement id. `None` or zero means any
    /// virtual placement of the image.
    pub underline: Option<u32>,
}

/// A virtual placement the cells may refer to.
#[derive(Clone, Copy, Debug)]
pub struct Virtual {
    pub image: u32,
    pub placement: Option<u32>,
    /// The box the picture is fitted into, in cells.
    pub columns: u32,
    pub rows: u32,
    pub width: u32,
    pub height: u32,
    pub gpu: u32,
}

/// One run of adjacent placeholder cells, as a picture part.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub gpu: u32,
    pub image: u32,
    pub placement: Option<u32>,
    /// Screen position of the fitted region, in cells.
    pub column: f32,
    pub row: f32,
    pub width: f32,
    pub height: f32,
    /// Source rectangle in image pixels.
    pub source: [f32; 4],
}

/// Where a virtual placement's placeholders sit: the minimum column and
/// row, in screen cells. A relative child anchors here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Origin {
    pub image: u32,
    pub placement: Option<u32>,
    pub column: f32,
    pub row: f32,
}

#[derive(Clone, Copy, Debug)]
struct Resolved {
    image: u32,
    placement: u32,
    row: u32,
    col: u32,
    fg: u32,
    underline: u32,
}

/// The picture parts for the visible cells, and the origin of each virtual
/// placement those cells refer to. Empty when nothing virtual exists, so
/// the caller can skip the scan.
pub fn parts(
    cells: &[CellRef],
    cols: u16,
    virtuals: &[Virtual],
    cell_w: f32,
    cell_h: f32,
) -> (Vec<Part>, Vec<Origin>) {
    if virtuals.is_empty() || cols == 0 || cells.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let cols = usize::from(cols);
    let mut resolved: Vec<Option<Resolved>> = Vec::with_capacity(cells.len());
    for (i, cell) in cells.iter().enumerate() {
        let left = if i % cols == 0 { None } else { resolved.last().and_then(|r| *r) };
        resolved.push(resolve(cell, left));
    }
    let mut parts = Vec::new();
    let mut origins: Vec<Origin> = Vec::new();
    let mut i = 0;
    while i < resolved.len() {
        let Some(cell) = resolved[i] else {
            i += 1;
            continue;
        };
        let Some(virt) = pick(virtuals, cell.image, cell.placement) else {
            i += 1;
            continue;
        };
        let screen_row = (i / cols) as u32;
        let screen_col = (i % cols) as u32;
        note_origin(&mut origins, virt, screen_col, screen_row);
        // A run: the same image, placement and image-row, in adjacent
        // screen columns, with consecutive image columns.
        let mut span = 1u32;
        while i + (span as usize) < resolved.len()
            && !(i + span as usize).is_multiple_of(cols)
            && resolved[i + span as usize].is_some_and(|next| {
                next.image == cell.image
                    && next.placement == cell.placement
                    && next.row == cell.row
                    && next.col == cell.col + span
            })
        {
            let next_col = screen_col + span;
            note_origin(&mut origins, virt, next_col, screen_row);
            span += 1;
        }
        if let Some(part) = fitted(virt, cell.row, cell.col, span, (screen_col, screen_row), (cell_w, cell_h))
        {
            parts.push(part);
        }
        i += span as usize;
    }
    (parts, origins)
}

fn note_origin(origins: &mut Vec<Origin>, virt: &Virtual, col: u32, row: u32) {
    if let Some(origin) = origins
        .iter_mut()
        .find(|o| o.image == virt.image && o.placement == virt.placement)
    {
        origin.column = origin.column.min(col as f32);
        origin.row = origin.row.min(row as f32);
    } else {
        origins.push(Origin {
            image: virt.image,
            placement: virt.placement,
            column: col as f32,
            row: row as f32,
        });
    }
}

/// The first and second marks are the row and the column; the third is
/// the image id's most significant byte. A missing mark is taken from the
/// placeholder to the left when the specification's colour rules hold.
/// The first cell of a row, with only a row mark, takes column 0.
fn resolve(cell: &CellRef, left: Option<Resolved>) -> Option<Resolved> {
    if cell.symbol != GLYPH {
        return None;
    }
    let mut marks = [None; 3];
    let mut n = 0;
    for ch in cell.marks {
        if ch == '\0' {
            continue;
        }
        if let Some(index) = marks::index(ch)
            && n < 3
        {
            marks[n] = Some(u32::from(index));
            n += 1;
        }
    }
    let fg = cell.fg.unwrap_or(0);
    let underline = cell.underline.unwrap_or(0);
    let (row, col, msb) = match (marks[0], marks[1], marks[2], left) {
        (None, None, None, Some(left)) if left.fg == fg && left.underline == underline => {
            (left.row, left.col.saturating_add(1), left.image >> 24)
        }
        (Some(row), None, None, Some(left))
            if left.row == row && left.fg == fg && left.underline == underline =>
        {
            (row, left.col.saturating_add(1), left.image >> 24)
        }
        (Some(row), Some(col), None, Some(left))
            if left.row == row
                && left.col.saturating_add(1) == col
                && left.fg == fg
                && left.underline == underline =>
        {
            (row, col, left.image >> 24)
        }
        (Some(row), col, msb, _) => (row, col.unwrap_or(0), msb.unwrap_or(0)),
        _ => return None,
    };
    let low = fg & 0x00FF_FFFF;
    Some(Resolved {
        image: (msb << 24) | low,
        placement: underline,
        row,
        col,
        fg,
        underline,
    })
}

/// Underline zero may choose any virtual placement of the image. A named
/// placement id must match. The lowest placement id wins when several may.
fn pick(virtuals: &[Virtual], image: u32, placement: u32) -> Option<&Virtual> {
    if placement != 0 {
        return virtuals
            .iter()
            .find(|v| v.image == image && v.placement == Some(placement));
    }
    virtuals
        .iter()
        .filter(|v| v.image == image)
        .min_by_key(|v| v.placement.unwrap_or(0))
}

/// Fit the picture into the virtual placement's box, keeping its aspect,
/// and take the source rectangle the cells cover. The destination is the
/// fitted region, not the whole cell, so a letterbox is not stretched.
fn fitted(
    virt: &Virtual,
    img_row: u32,
    img_col: u32,
    span: u32,
    screen: (u32, u32),
    cell: (f32, f32),
) -> Option<Part> {
    let (screen_col, screen_row) = screen;
    let cell_w = cell.0.max(1.0);
    let cell_h = cell.1.max(1.0);
    let columns = virt.columns.max(1) as f32;
    let rows = virt.rows.max(1) as f32;
    let img_w = virt.width.max(1) as f32;
    let img_h = virt.height.max(1) as f32;
    let box_w = columns * cell_w;
    let box_h = rows * cell_h;
    let scale = (box_w / img_w).min(box_h / img_h);
    if scale <= 0.0 {
        return None;
    }
    let drawn_w = img_w * scale;
    let drawn_h = img_h * scale;
    let origin_x = (box_w - drawn_w) * 0.5;
    let origin_y = (box_h - drawn_h) * 0.5;
    let x0 = (img_col as f32 * cell_w).max(origin_x);
    let y0 = (img_row as f32 * cell_h).max(origin_y);
    let x1 = ((img_col + span) as f32 * cell_w).min(origin_x + drawn_w);
    let y1 = ((img_row + 1) as f32 * cell_h).min(origin_y + drawn_h);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let source = [
        (x0 - origin_x) / scale,
        (y0 - origin_y) / scale,
        (x1 - x0) / scale,
        (y1 - y0) / scale,
    ];
    let box_col = screen_col as f32 - img_col as f32;
    let box_row = screen_row as f32 - img_row as f32;
    Some(Part {
        gpu: virt.gpu,
        image: virt.image,
        placement: virt.placement,
        column: box_col + x0 / cell_w,
        row: box_row + y0 / cell_h,
        width: (x1 - x0) / cell_w,
        height: (y1 - y0) / cell_h,
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(symbol: char, marks: [char; 3], fg: u32) -> CellRef {
        CellRef { symbol, marks, fg: Some(fg), underline: None }
    }

    fn virt(columns: u32, rows: u32, width: u32, height: u32) -> Virtual {
        Virtual { image: 42, placement: Some(1), columns, rows, width, height, gpu: 7 }
    }

    #[test]
    fn a_missing_column_mark_is_the_cell_to_the_left_plus_one() {
        let row0 = marks::mark(0);
        let row1 = marks::mark(1);
        let cells = [
            cell(GLYPH, [row0, '\0', '\0'], 42),
            cell(GLYPH, ['\0', '\0', '\0'], 42),
            cell(GLYPH, ['\0', '\0', '\0'], 42),
            cell(GLYPH, [row1, '\0', '\0'], 42),
            cell(GLYPH, ['\0', '\0', '\0'], 42),
            cell(GLYPH, ['\0', '\0', '\0'], 42),
        ];
        let (parts, _) = parts(&cells, 3, &[virt(3, 2, 30, 20)], 10.0, 10.0);
        assert_eq!(parts.len(), 2, "one run per image row");
        assert_eq!(parts[0].source[0], 0.0);
        assert!(parts[0].source[2] > 20.0, "the run covers the row's columns");
        assert_eq!(parts[1].row, 1.0);
    }

    #[test]
    fn an_unknown_image_draws_no_part() {
        let cells = [cell(GLYPH, [marks::mark(0), marks::mark(0), '\0'], 99)];
        let (parts, _) = parts(&cells, 1, &[virt(1, 1, 8, 8)], 8.0, 16.0);
        assert!(parts.is_empty());
    }
}
