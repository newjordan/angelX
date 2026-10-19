//! Pictures in the window's cells.
//!
//! The cockpit hands the window a picture's pixels (`Host::upload_picture`)
//! and marks the cells it shows in: each such cell is the placeholder
//! character with two marks for its row and column in the picture's grid
//! (and a third for the id's high byte), its foreground colour naming the
//! picture's id. The renderer reads those cells every frame and draws each
//! one's share of the picture. No terminal escape codes are involved.
//!
//! This module is CPU only: reading picture cells and the cell-to-pixel
//! maths. The GPU side lives in `angel-gpu`.

use ratatui_core::style::Color;

use angel_stage::marks;

/// The placeholder character a picture cell starts with.
pub const PLACEHOLDER: char = '\u{10EEEE}';
const PLACEHOLDER_UTF8: [u8; 4] = [0xF4, 0x8E, 0xBB, 0xAE];

/// One cell's share of a picture: the image, and the row and column of its
/// placement grid this cell shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placeholder {
    pub image: u32,
    pub row: u16,
    pub col: u16,
}

/// Fast prefix check: whether `symbol` starts with [`PLACEHOLDER`].
#[inline]
pub fn is_placeholder(symbol: &str) -> bool {
    symbol.as_bytes().starts_with(&PLACEHOLDER_UTF8)
}

/// Read a picture cell from its symbol and foreground: the placeholder, its
/// row and column marks and, optionally, the id's high byte. `None` for any
/// other cell. Every frame reads every picture cell, so nothing allocates.
#[inline]
pub fn read_placeholder(symbol: &str, fg: Color) -> Option<Placeholder> {
    if !is_placeholder(symbol) {
        return None;
    }
    let mut chars = symbol[4..].chars();
    let row = diacritic(chars.next()?)?;
    let col = diacritic(chars.next()?)?;
    let high = match chars.next() {
        Some(c) => diacritic(c)?,
        None => 0,
    };
    if chars.next().is_some() {
        return None;
    }
    Some(Placeholder {
        image: color_id(fg) | (u32::from(high & 0xff) << 24),
        row,
        col,
    })
}

#[inline]
fn diacritic(c: char) -> Option<u16> {
    marks::index(c)
}

/// The image id a colour names: 24-bit colours are the id, palette colours
/// their index.
fn color_id(color: Color) -> u32 {
    match color {
        Color::Rgb(r, g, b) => (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b),
        Color::Indexed(n) => u32::from(n),
        Color::Black => 0,
        Color::Red => 1,
        Color::Green => 2,
        Color::Yellow => 3,
        Color::Blue => 4,
        Color::Magenta => 5,
        Color::Cyan => 6,
        Color::Gray => 7,
        Color::DarkGray => 8,
        Color::LightRed => 9,
        Color::LightGreen => 10,
        Color::LightYellow => 11,
        Color::LightBlue => 12,
        Color::LightMagenta => 13,
        Color::LightCyan => 14,
        Color::White => 15,
        Color::Reset => 0,
    }
}

/// The picture upload type and the check the GPU holds every upload to are
/// the renderer core's (`angel-gpu`).
pub use angel_gpu::{Picture, premultiply_in_place, well_formed};

/// The placement box in cells: `cols`/`rows` when given, otherwise the
/// picture's size in cells, rounded up.
pub fn box_cells(image: (u32, u32), cols: u16, rows: u16, cell: (u32, u32)) -> (u16, u16) {
    let fit =
        |pixels: u32, size: u32| pixels.div_ceil(size.max(1)).clamp(1, u32::from(u16::MAX)) as u16;
    (
        if cols > 0 { cols } else { fit(image.0, cell.0) },
        if rows > 0 { rows } else { fit(image.1, cell.1) },
    )
}

/// Where picture cell (`row`, `col`) lands. The picture is fit inside its
/// box with its aspect kept and centred; a box that is
/// only the picture's own size rounded up to cells shows it at its own size,
/// so a picture made for the cell grid stays pixel for pixel. Returns the
/// destination inside the cell (`[x, y, w, h]` in pixels from the cell's
/// top-left) and the source rectangle in picture pixels, or `None` when the
/// picture does not reach that cell.
pub fn cell_slice(
    image: (u32, u32),
    box_cells: (u16, u16),
    cell: (u32, u32),
    row: u16,
    col: u16,
) -> Option<([f32; 4], [f32; 4])> {
    cell_span_slice(image, box_cells, cell, row, col, 1)
}

/// Where a horizontal run of `cols_span` contiguous picture cells starting at
/// (`row`, `col`) lands. Returns the destination (`[x, y, w, h]` in pixels from
/// the first cell's top-left) and the source rectangle in picture pixels, or
/// `None` when the picture does not intersect the span.
pub fn cell_span_slice(
    image: (u32, u32),
    box_cells: (u16, u16),
    cell: (u32, u32),
    row: u16,
    col: u16,
    cols_span: u16,
) -> Option<([f32; 4], [f32; 4])> {
    let (iw, ih) = (image.0 as f32, image.1 as f32);
    let (cw, ch) = (cell.0 as f32, cell.1 as f32);
    let (bw, bh) = (f32::from(box_cells.0) * cw, f32::from(box_cells.1) * ch);
    if iw <= 0.0 || ih <= 0.0 || bw <= 0.0 || bh <= 0.0 || cols_span == 0 {
        return None;
    }
    let own_size = box_cells == self::box_cells(image, 0, 0, cell);
    let fit = (bw / iw).min(bh / ih);
    let scale = if own_size { fit.min(1.0) } else { fit };
    let (dw, dh) = ((iw * scale).round().max(1.0), (ih * scale).round().max(1.0));
    let (ox, oy) = (((bw - dw) / 2.0).round(), ((bh - dh) / 2.0).round());
    let (cx, cy) = (f32::from(col) * cw, f32::from(row) * ch);
    let span_w = f32::from(cols_span) * cw;
    let (x0, x1) = (cx.max(ox), (cx + span_w).min(ox + dw));
    let (y0, y1) = (cy.max(oy), (cy + ch).min(oy + dh));
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let (sx, sy) = (iw / dw, ih / dh);
    Some((
        [x0 - cx, y0 - cy, x1 - x0, y1 - y0],
        [
            (x0 - ox) * sx,
            (y0 - oy) * sy,
            (x1 - x0) * sx,
            (y1 - y0) * sy,
        ],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(row: u16, col: u16, high: Option<u16>) -> String {
        let mark = |n: u16| marks::mark(usize::from(n));
        let mut symbol = String::from(PLACEHOLDER);
        symbol.push(mark(row));
        symbol.push(mark(col));
        if let Some(high) = high {
            symbol.push(mark(high));
        }
        symbol
    }

    #[test]
    fn a_picture_cell_names_its_picture_row_and_column() {
        let fg = Color::Rgb(0x12, 0x34, 0x56);
        assert_eq!(
            read_placeholder(&cell(3, 7, None), fg),
            Some(Placeholder { image: 0x0012_3456, row: 3, col: 7 })
        );
        assert_eq!(
            read_placeholder(&cell(0, 1, Some(2)), fg),
            Some(Placeholder { image: 0x0212_3456, row: 0, col: 1 })
        );
    }

    #[test]
    fn anything_else_is_not_a_picture_cell() {
        let fg = Color::Rgb(1, 2, 3);
        assert_eq!(read_placeholder("a", fg), None);
        assert_eq!(read_placeholder(&String::from(PLACEHOLDER), fg), None, "no marks");
        assert_eq!(read_placeholder(&format!("{}x", cell(1, 1, None)), fg), None, "trailing text");
        assert_eq!(read_placeholder(&format!("{PLACEHOLDER}ab"), fg), None, "not marks");
    }

    #[test]
    fn box_comes_from_the_picture_when_not_given() {
        assert_eq!(box_cells((100, 50), 0, 0, (12, 25)), (9, 2));
        assert_eq!(box_cells((100, 50), 40, 12, (12, 25)), (40, 12));
    }

    #[test]
    fn own_size_box_shows_pixel_for_pixel() {
        // 24 × 50 in 12 × 25 cells: exactly 2 × 2 cells.
        let slice = cell_slice((24, 50), (2, 2), (12, 25), 1, 1).unwrap();
        assert_eq!(slice, ([0.0, 0.0, 12.0, 25.0], [12.0, 25.0, 12.0, 25.0]));
    }

    #[test]
    fn given_box_scales_and_letterboxes() {
        // A 2:1 picture in a 4 × 4 box of 10 × 10 cells: 40 × 20, centred.
        let (dst, src) = cell_slice((200, 100), (4, 4), (10, 10), 1, 0).unwrap();
        assert_eq!(dst, [0.0, 0.0, 10.0, 10.0]);
        assert_eq!(src, [0.0, 0.0, 50.0, 50.0]);
        // Row 0 is above the picture.
        assert_eq!(cell_slice((200, 100), (4, 4), (10, 10), 0, 0), None);
    }

    #[test]
    fn span_slice_matches_the_union_of_individual_cell_slices() {
        let image = (384, 224);
        let boxed = (40, 14);
        let cell_px = (10, 23);
        let (dst_span, src_span) = cell_span_slice(image, boxed, cell_px, 5, 0, 40).unwrap();
        let mut x_min = f32::INFINITY;
        let mut x_max = f32::NEG_INFINITY;
        let mut u_min = f32::INFINITY;
        let mut u_max = f32::NEG_INFINITY;
        let mut y_dst = None;
        let mut y_src = None;
        for c in 0..40u16 {
            if let Some((dst, src)) = cell_slice(image, boxed, cell_px, 5, c) {
                let offset = f32::from(c) * cell_px.0 as f32;
                x_min = x_min.min(offset + dst[0]);
                x_max = x_max.max(offset + dst[0] + dst[2]);
                u_min = u_min.min(src[0]);
                u_max = u_max.max(src[0] + src[2]);
                y_dst = Some((dst[1], dst[3]));
                y_src = Some((src[1], src[3]));
            }
        }
        assert!((dst_span[0] - x_min).abs() < 1e-4);
        assert!((dst_span[2] - (x_max - x_min)).abs() < 1e-4);
        assert_eq!((dst_span[1], dst_span[3]), y_dst.unwrap());
        assert!((src_span[0] - u_min).abs() < 1e-4);
        assert!((src_span[2] - (u_max - u_min)).abs() < 1e-4);
        assert_eq!((src_span[1], src_span[3]), y_src.unwrap());
    }
}
