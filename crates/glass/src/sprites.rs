//! Box-drawing, block and braille characters, drawn by us at the cell's exact
//! pixel size instead of taken from a font. The realm, the Delve and the
//! minimaps are braille and half blocks: drawn here they tile without seams at
//! every size and need no font that covers them.

use crate::config::BrailleStyle;
use crate::font::CellMetrics;

/// An alpha mask the size of one cell, or `None` when `ch` is not ours.
pub fn draw(ch: char, cell: &CellMetrics, braille: BrailleStyle) -> Option<Vec<u8>> {
    let mut canvas = Canvas::new(cell.width, cell.height);
    let code = ch as u32;
    match code {
        0x2500..=0x257f => box_drawing(&mut canvas, code, cell.line, cell.heavy),
        0x2580..=0x259f => blocks(&mut canvas, code),
        0x2800..=0x28ff => braille_cell(&mut canvas, (code - 0x2800) as u8, braille),
        _ => return None,
    }
    Some(canvas.data)
}

struct Canvas {
    w: u32,
    h: u32,
    data: Vec<u8>,
}

impl Canvas {
    fn new(w: u32, h: u32) -> Self {
        Self {
            w,
            h,
            data: vec![0; (w * h) as usize],
        }
    }

    /// Fill `[x0, x1) × [y0, y1)` with `alpha`, clipped to the cell.
    fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, alpha: u8) {
        let (x0, x1) = (x0.max(0) as u32, (x1.max(0) as u32).min(self.w));
        let (y0, y1) = (y0.max(0) as u32, (y1.max(0) as u32).min(self.h));
        for y in y0..y1 {
            for x in x0..x1 {
                let p = &mut self.data[(y * self.w + x) as usize];
                *p = (*p).max(alpha);
            }
        }
    }

    /// Antialiased shape: `inside(x, y)` sampled 4×4 times per pixel.
    fn shape(&mut self, inside: impl Fn(f32, f32) -> bool) {
        for y in 0..self.h {
            for x in 0..self.w {
                let mut hits = 0u32;
                for sy in 0..4 {
                    for sx in 0..4 {
                        let px = x as f32 + (sx as f32 + 0.5) / 4.0;
                        let py = y as f32 + (sy as f32 + 0.5) / 4.0;
                        hits += u32::from(inside(px, py));
                    }
                }
                if hits > 0 {
                    let p = &mut self.data[(y * self.w + x) as usize];
                    *p = (*p).max((hits * 255 / 16) as u8);
                }
            }
        }
    }
}

const NONE: u8 = 0;
const HEAVY: u8 = 2;
const DOUBLE: u8 = 3;

/// Arms of U+2500..U+257F as `up right down left`, one digit each
/// (0 none, 1 light, 2 heavy, 3 double). Letters mark the special shapes:
/// `a`–`d` dashes (3, 4, 2 dashes; light/heavy, horizontal/vertical are
/// in the arm digits), `r` rounded corners, `x` diagonals.
const ARMS: [&str; 128] = [
    "0101", "0202", "1010", "2020", // ─ ━ │ ┃
    "d3h1", "d3h2", "d3v1", "d3v2", // ┄ ┅ ┆ ┇
    "d4h1", "d4h2", "d4v1", "d4v2", // ┈ ┉ ┊ ┋
    "0110", "0210", "0120", "0220", // ┌ ┍ ┎ ┏
    "0011", "0012", "0021", "0022", // ┐ ┑ ┒ ┓
    "1100", "1200", "2100", "2200", // └ ┕ ┖ ┗
    "1001", "1002", "2001", "2002", // ┘ ┙ ┚ ┛
    "1110", "1210", "2110", "1120", // ├ ┝ ┞ ┟
    "2120", "2210", "1220", "2220", // ┠ ┡ ┢ ┣
    "1011", "1012", "2011", "1021", // ┤ ┥ ┦ ┧
    "2021", "2012", "1022", "2022", // ┨ ┩ ┪ ┫
    "0111", "0112", "0211", "0212", // ┬ ┭ ┮ ┯
    "0121", "0122", "0221", "0222", // ┰ ┱ ┲ ┳
    "1101", "1102", "1201", "1202", // ┴ ┵ ┶ ┷
    "2101", "2102", "2201", "2202", // ┸ ┹ ┺ ┻
    "1111", "1112", "1211", "1212", // ┼ ┽ ┾ ┿
    "2111", "1121", "2121", "2112", // ╀ ╁ ╂ ╃
    "2211", "1122", "1221", "2212", // ╄ ╅ ╆ ╇
    "1222", "2122", "2221", "2222", // ╈ ╉ ╊ ╋
    "d2h1", "d2h2", "d2v1", "d2v2", // ╌ ╍ ╎ ╏
    "0303", "3030", "0310", "0130", // ═ ║ ╒ ╓
    "0330", "0013", "0031", "0033", // ╔ ╕ ╖ ╗
    "1300", "3100", "3300", "1003", // ╘ ╙ ╚ ╛
    "3001", "3003", "1310", "3130", // ╜ ╝ ╞ ╟
    "3330", "1013", "3031", "3033", // ╠ ╡ ╢ ╣
    "0313", "0131", "0333", "1303", // ╤ ╥ ╦ ╧
    "3101", "3303", "1313", "3131", // ╨ ╩ ╪ ╫
    "3333", "r0", "r1", "r2", // ╬ ╭ ╮ ╯
    "r3", "x0", "x1", "x2", // ╰ ╱ ╲ ╳
    "0001", "1000", "0100", "0010", // ╴ ╵ ╶ ╷
    "0002", "2000", "0200", "0020", // ╸ ╹ ╺ ╻
    "0201", "1020", "0102", "2010", // ╼ ╽ ╾ ╿
];

fn box_drawing(canvas: &mut Canvas, code: u32, line: u32, heavy: u32) {
    let spec = ARMS[(code - 0x2500) as usize].as_bytes();
    let (w, h) = (canvas.w as i32, canvas.h as i32);
    let light = line as i32;
    let heavy = heavy as i32;
    let thickness = |weight: u8| if weight == HEAVY { heavy } else { light };
    match spec[0] {
        b'd' => {
            let dashes = i32::from(spec[1] - b'0');
            let horizontal = spec[2] == b'h';
            let t = thickness(spec[3] - b'0');
            let length = if horizontal { w } else { h };
            for i in 0..dashes {
                let start = i * length / dashes;
                let end = (i + 1) * length / dashes;
                let gap = ((end - start) / 4).max(1);
                if horizontal {
                    let y0 = (h - t) / 2;
                    canvas.rect(start, y0, end - gap, y0 + t, 255);
                } else {
                    let x0 = (w - t) / 2;
                    canvas.rect(x0, start, x0 + t, end - gap, 255);
                }
            }
        }
        b'r' => rounded(canvas, spec[1] - b'0', light),
        b'x' => {
            let half = light as f32 / 2.0 + 0.25;
            let (fw, fh) = (w as f32, h as f32);
            let length = (fw * fw + fh * fh).sqrt();
            let which = spec[1] - b'0';
            canvas.shape(|x, y| {
                // Distance to the two cell diagonals.
                let rising = ((fh * x + fw * y - fw * fh) / length).abs();
                let falling = ((fh * x - fw * y) / length).abs();
                (which != 1 && rising <= half) || (which != 0 && falling <= half)
            });
        }
        _ => {
            let [up, right, down, left] = [spec[0], spec[1], spec[2], spec[3]].map(|b| b - b'0');
            arms(canvas, up, right, down, left, light, heavy);
        }
    }
}

fn arms(canvas: &mut Canvas, up: u8, right: u8, down: u8, left: u8, light: i32, heavy: i32) {
    let (w, h) = (canvas.w as i32, canvas.h as i32);
    let thickness = |weight: u8| match weight {
        HEAVY => heavy,
        DOUBLE => light * 3,
        _ => light,
    };
    // Horizontal arms reach across the widest vertical stroke, and back.
    let vertical = thickness(up.max(down)).max(if up == NONE && down == NONE { 0 } else { light });
    let horizontal = thickness(left.max(right)).max(if left == NONE && right == NONE {
        0
    } else {
        light
    });
    let vx0 = (w - vertical) / 2;
    let vx1 = vx0 + vertical;
    let hy0 = (h - horizontal) / 2;
    let hy1 = hy0 + horizontal;
    let stroke_h = |canvas: &mut Canvas, weight: u8, x0: i32, x1: i32| {
        if weight == DOUBLE {
            let top = (h - light * 3) / 2;
            canvas.rect(x0, top, x1, top + light, 255);
            canvas.rect(x0, top + light * 2, x1, top + light * 3, 255);
        } else if weight != NONE {
            let t = thickness(weight);
            let y0 = (h - t) / 2;
            canvas.rect(x0, y0, x1, y0 + t, 255);
        }
    };
    let stroke_v = |canvas: &mut Canvas, weight: u8, y0: i32, y1: i32| {
        if weight == DOUBLE {
            let left_edge = (w - light * 3) / 2;
            canvas.rect(left_edge, y0, left_edge + light, y1, 255);
            canvas.rect(left_edge + light * 2, y0, left_edge + light * 3, y1, 255);
        } else if weight != NONE {
            let t = thickness(weight);
            let x0 = (w - t) / 2;
            canvas.rect(x0, y0, x0 + t, y1, 255);
        }
    };
    let (left_end, right_start) = if up == NONE && down == NONE {
        (w / 2 + 1, w / 2)
    } else {
        (vx1, vx0)
    };
    let (up_end, down_start) = if left == NONE && right == NONE {
        (h / 2 + 1, h / 2)
    } else {
        (hy1, hy0)
    };
    stroke_h(canvas, left, 0, left_end);
    stroke_h(canvas, right, right_start, w);
    stroke_v(canvas, up, 0, up_end);
    stroke_v(canvas, down, down_start, h);
}

/// ╭ ╮ ╯ ╰: a quarter circle joining the two arms, then straight to the edges.
fn rounded(canvas: &mut Canvas, which: u8, light: i32) {
    let (w, h) = (canvas.w as f32, canvas.h as f32);
    let t = light as f32;
    let cx = ((canvas.w as i32 - light) / 2) as f32 + t / 2.0;
    let cy = ((canvas.h as i32 - light) / 2) as f32 + t / 2.0;
    let r = cx.min(cy).min(w - cx).min(h - cy).max(1.0);
    // Toward the right (+1) or left (-1), down (+1) or up (-1).
    let (sx, sy) = match which {
        0 => (1.0, 1.0),
        1 => (-1.0, 1.0),
        2 => (-1.0, -1.0),
        _ => (1.0, -1.0),
    };
    let (ox, oy) = (cx + sx * r, cy + sy * r);
    canvas.shape(|x, y| {
        let (dx, dy) = (x - ox, y - oy);
        let in_quadrant = dx * sx <= 0.0 && dy * sy <= 0.0;
        let ring = ((dx * dx + dy * dy).sqrt() - r).abs() <= t / 2.0;
        let straight_h = (y - cy).abs() <= t / 2.0 && (x - ox) * sx >= 0.0;
        let straight_v = (x - cx).abs() <= t / 2.0 && (y - oy) * sy >= 0.0;
        (in_quadrant && ring) || straight_h || straight_v
    });
}

fn blocks(canvas: &mut Canvas, code: u32) {
    let (w, h) = (canvas.w as i32, canvas.h as i32);
    let eighth_h = |n: i32| (h * n + 4) / 8;
    let eighth_w = |n: i32| (w * n + 4) / 8;
    let (hw, hh) = (w / 2, h / 2);
    match code {
        0x2580 => canvas.rect(0, 0, w, hh, 255),
        0x2581..=0x2588 => {
            let n = (code - 0x2580) as i32;
            canvas.rect(0, h - eighth_h(n), w, h, 255);
        }
        0x2589..=0x258f => {
            let n = (0x2590 - code) as i32;
            canvas.rect(0, 0, eighth_w(n), h, 255);
        }
        0x2590 => canvas.rect(hw, 0, w, h, 255),
        0x2591 => canvas.rect(0, 0, w, h, 64),
        0x2592 => canvas.rect(0, 0, w, h, 128),
        0x2593 => canvas.rect(0, 0, w, h, 192),
        0x2594 => canvas.rect(0, 0, w, eighth_h(1), 255),
        0x2595 => canvas.rect(w - eighth_w(1), 0, w, h, 255),
        _ => {
            // Quadrants: upper-left, upper-right, lower-left, lower-right.
            let quads: u8 = match code {
                0x2596 => 0b0010,
                0x2597 => 0b0001,
                0x2598 => 0b1000,
                0x2599 => 0b1011,
                0x259a => 0b1001,
                0x259b => 0b1110,
                0x259c => 0b1101,
                0x259d => 0b0100,
                0x259e => 0b0110,
                _ => 0b0111,
            };
            if quads & 0b1000 != 0 {
                canvas.rect(0, 0, hw, hh, 255);
            }
            if quads & 0b0100 != 0 {
                canvas.rect(hw, 0, w, hh, 255);
            }
            if quads & 0b0010 != 0 {
                canvas.rect(0, hh, hw, h, 255);
            }
            if quads & 0b0001 != 0 {
                canvas.rect(hw, hh, w, h, 255);
            }
        }
    }
}

/// Dots 1-2-3-7 run down the left column, 4-5-6-8 down the right.
const BRAILLE_DOTS: [(u8, u8); 8] = [
    (0, 0),
    (0, 1),
    (0, 2),
    (1, 0),
    (1, 1),
    (1, 2),
    (0, 3),
    (1, 3),
];

fn braille_cell(canvas: &mut Canvas, bits: u8, style: BrailleStyle) {
    let (w, h) = (canvas.w as i32, canvas.h as i32);
    let col = |c: i32| (c * w / 2, (c + 1) * w / 2);
    let row = |r: i32| (r * h / 4, (r + 1) * h / 4);
    let lit: Vec<(i32, i32)> = BRAILLE_DOTS
        .iter()
        .enumerate()
        .filter(|(bit, _)| bits & (1 << bit) != 0)
        .map(|(_, &(c, r))| (i32::from(c), i32::from(r)))
        .collect();
    match style {
        BrailleStyle::Pixels => {
            for (c, r) in lit {
                let (x0, x1) = col(c);
                let (y0, y1) = row(r);
                canvas.rect(x0, y0, x1, y1, 255);
            }
        }
        BrailleStyle::Dots => {
            let radius = (w as f32 / 2.0).min(h as f32 / 4.0) * 0.36;
            let centers: Vec<(f32, f32)> = lit
                .iter()
                .map(|&(c, r)| {
                    let (x0, x1) = col(c);
                    let (y0, y1) = row(r);
                    ((x0 + x1) as f32 / 2.0, (y0 + y1) as f32 / 2.0)
                })
                .collect();
            let r2 = radius * radius;
            canvas.shape(|x, y| {
                centers
                    .iter()
                    .any(|&(cx, cy)| (x - cx) * (x - cx) + (y - cy) * (y - cy) <= r2)
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell() -> CellMetrics {
        CellMetrics {
            width: 10,
            height: 20,
            baseline: 15,
            underline_y: 17,
            underline_height: 1,
            strike_y: 10,
            line: 1,
            heavy: 2,
        }
    }

    fn coverage(mask: &[u8]) -> usize {
        mask.iter().filter(|&&a| a > 0).count()
    }

    #[test]
    fn full_block_fills_the_cell() {
        let mask = draw('█', &cell(), BrailleStyle::Dots).unwrap();
        assert!(mask.iter().all(|&a| a == 255));
    }

    #[test]
    fn half_blocks_split_the_cell() {
        let upper = draw('▀', &cell(), BrailleStyle::Dots).unwrap();
        let lower = draw('▄', &cell(), BrailleStyle::Dots).unwrap();
        assert_eq!(coverage(&upper), 100);
        assert_eq!(coverage(&lower), 100);
        assert!(
            upper
                .iter()
                .zip(&lower)
                .all(|(a, b)| (*a == 0) != (*b == 0))
        );
    }

    #[test]
    fn horizontal_line_spans_the_width() {
        let mask = draw('─', &cell(), BrailleStyle::Dots).unwrap();
        assert_eq!(coverage(&mask), 10);
    }

    #[test]
    fn braille_pixels_are_eighths() {
        let all = draw('⣿', &cell(), BrailleStyle::Pixels).unwrap();
        assert_eq!(coverage(&all), 200);
        let one = draw('⠁', &cell(), BrailleStyle::Pixels).unwrap();
        assert_eq!(coverage(&one), 25);
        assert_eq!(one[0], 255);
    }

    #[test]
    fn braille_dots_keep_gaps() {
        let all = draw('⣿', &cell(), BrailleStyle::Dots).unwrap();
        let lit = coverage(&all);
        assert!(lit > 8 && lit < 200, "{lit}");
        assert_eq!(
            draw('⠀', &cell(), BrailleStyle::Dots).map(|m| coverage(&m)),
            Some(0)
        );
    }

    #[test]
    fn every_box_char_draws_something() {
        for code in 0x2500..=0x257f {
            let ch = char::from_u32(code).unwrap();
            let mask = draw(ch, &cell(), BrailleStyle::Dots).unwrap();
            assert!(coverage(&mask) > 0, "{ch:?} drew nothing");
        }
    }
}
