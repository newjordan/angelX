//! A page of text as pixels: lines rasterized with the window's own
//! font stack (`font.rs`), for a producer that hands the result to
//! the compositor as a picture instead of painting cells. This is
//! the one raster service both hosts share — the cockpit asks here
//! rather than carrying a second font stack of its own.
//!
//! Layout is the cell grid the fonts measure: every glyph advances
//! by whole cells, lines stack by cell height. The picture is
//! straight (not premultiplied) RGBA, like every producer handover;
//! the GPU side premultiplies at upload. Glyph pictures are memoized
//! per symbol, so re-rendering a scrolled page pays the composite,
//! not the rasterizer.

use std::collections::HashMap;
use std::sync::Arc;

use unicode_width::UnicodeWidthChar;

use crate::font::{Fonts, Picture, BOLD, ITALIC};

/// The most glyph pictures one pager memoizes before starting over:
/// far past a document's alphabet, bounded so a producer streaming
/// novel symbols forever cannot grow it without limit.
const MEMO_MAX: usize = 8192;
/// The texture side every host guarantees; a page past it is refused
/// rather than truncated.
const MAX_SIDE: u32 = 8192;

/// A loaded font stack that renders text pages. Loading runs
/// fontconfig, so keep one and reuse it.
pub struct Pager {
    fonts: Fonts,
    memo: HashMap<String, Option<Arc<Picture>>>,
}

impl Pager {
    /// Load `family` (a fontconfig family name, like the window's)
    /// sized so its cell height lands on `cell_height_px`: load at a
    /// guess, measure, and correct once. `None` when no font answers.
    pub fn load(family: &str, cell_height_px: u32) -> Option<Self> {
        let target = cell_height_px.max(4) as f32;
        let mut fonts = Fonts::load(family, target, (target / 12.0).max(1.0)).ok()?;
        let measured = fonts.cell.height.max(1) as f32;
        if (measured - target).abs() > 1.0 {
            fonts.set_size(target * target / measured, (target / 12.0).max(1.0));
        }
        Some(Self {
            fonts,
            memo: HashMap::new(),
        })
    }

    /// The cell a page is laid out on, in pixels `(width, height)`.
    pub fn cell(&self) -> (u32, u32) {
        (self.fonts.cell.width, self.fonts.cell.height)
    }

    /// The pixel size `lines` renders to, or `None` past the texture
    /// limit. An empty page is `None`: nothing to present.
    pub fn size(&self, lines: &[impl AsRef<str>]) -> Option<(u32, u32)> {
        let cols = lines
            .iter()
            .map(|line| line.as_ref().chars().filter_map(UnicodeWidthChar::width).sum::<usize>())
            .max()
            .unwrap_or(0) as u32;
        let (cw, ch) = self.cell();
        let (w, h) = (cols.checked_mul(cw)?, (lines.len() as u32).checked_mul(ch)?);
        (w > 0 && h > 0 && w <= MAX_SIDE && h <= MAX_SIDE).then_some((w, h))
    }

    /// Render `lines` in `fg` (straight RGB; style flags are `BOLD` /
    /// `ITALIC` bits, same for every line) over `bg` (straight RGBA).
    /// Returns `(width, height, rgba)`, or `None` past the limit.
    pub fn render(
        &mut self,
        lines: &[impl AsRef<str>],
        style: u8,
        fg: [u8; 3],
        bg: [u8; 4],
    ) -> Option<(u32, u32, Vec<u8>)> {
        let (w, h) = self.size(lines)?;
        let (cw, ch) = self.cell();
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..w * h {
            rgba.extend_from_slice(&bg);
        }
        for (row, line) in lines.iter().enumerate() {
            let mut col = 0u32;
            let mut chars = line.as_ref().chars().peekable();
            while let Some(base) = chars.next() {
                let span = UnicodeWidthChar::width(base).unwrap_or(0) as u32;
                if span == 0 {
                    continue; // a mark with no base before it
                }
                let mut symbol = base.to_string();
                while let Some(&mark) = chars.peek() {
                    if UnicodeWidthChar::width(mark) != Some(0) {
                        break;
                    }
                    symbol.push(mark);
                    chars.next();
                }
                let picture = (base != ' ')
                    .then(|| self.glyph(&symbol, style, span))
                    .flatten();
                if let Some(picture) = picture {
                    let (px, py) = (
                        (col * cw) as i32 + picture.left,
                        (row as u32 * ch) as i32 + picture.top,
                    );
                    if bg[3] == 255 {
                        composite_opaque(&mut rgba, w, h, &picture, px, py, fg);
                    } else {
                        composite(&mut rgba, w, h, &picture, px, py, fg);
                    }
                }
                col += span;
            }
        }
        Some((w, h, rgba))
    }

    fn glyph(&mut self, symbol: &str, style: u8, span: u32) -> Option<Arc<Picture>> {
        let key = format!("{style}:{span}:{symbol}");
        if let Some(held) = self.memo.get(&key) {
            return held.clone();
        }
        if self.memo.len() >= MEMO_MAX {
            self.memo.clear();
        }
        let picture = self.fonts.draw(symbol, style, span).map(Arc::new);
        self.memo.insert(key, picture.clone());
        picture
    }
}

/// Style bits for [`Pager::render`], the font stack's own.
pub const PLAIN: u8 = 0;
pub const BOLD_STYLE: u8 = BOLD;
pub const ITALIC_STYLE: u8 = ITALIC;

/// Composite over opaque paper, in integer math: a mask pixel is
/// `fg` at its alpha, a coloured pixel is already premultiplied, and
/// the paper's alpha never moves. The document case (dark text on
/// dark paper, light on light) always takes this path.
fn composite_opaque(page: &mut [u8], w: u32, h: u32, picture: &Picture, x: i32, y: i32, fg: [u8; 3]) {
    for gy in 0..picture.height as i32 {
        let py = y + gy;
        if py < 0 || py >= h as i32 {
            continue;
        }
        for gx in 0..picture.width as i32 {
            let px = x + gx;
            if px < 0 || px >= w as i32 {
                continue;
            }
            let cell = (gy as u32 * picture.width + gx as u32) as usize;
            let at = ((py as u32 * w + px as u32) * 4) as usize;
            if picture.colored {
                let i = cell * 4;
                let a = u16::from(picture.data.get(i + 3).copied().unwrap_or(0));
                if a == 0 {
                    continue;
                }
                // Stored premultiplied: out = src + dst * (1 - a).
                for k in 0..3 {
                    let s = u16::from(picture.data.get(i + k).copied().unwrap_or(0));
                    page[at + k] = (s + (u16::from(page[at + k]) * (255 - a)) / 255) as u8;
                }
            } else {
                let a = u16::from(picture.data.get(cell).copied().unwrap_or(0));
                if a == 0 {
                    continue;
                }
                for k in 0..3 {
                    let s = u16::from(fg[k]) * a / 255;
                    page[at + k] = (s + (u16::from(page[at + k]) * (255 - a)) / 255) as u8;
                }
            }
        }
    }
}

/// Composite one glyph picture over the page at `(x, y)`: a mask
/// tints `fg`, a coloured picture (premultiplied RGBA) lands as
/// itself. Straight-alpha `over`, clipped to the page.
fn composite(page: &mut [u8], w: u32, h: u32, picture: &Picture, x: i32, y: i32, fg: [u8; 3]) {
    for gy in 0..picture.height as i32 {
        let py = y + gy;
        if py < 0 || py >= h as i32 {
            continue;
        }
        for gx in 0..picture.width as i32 {
            let px = x + gx;
            if px < 0 || px >= w as i32 {
                continue;
            }
            let cell = (gy as u32 * picture.width + gx as u32) as usize;
            let (sr, sg, sb, sa) = if picture.colored {
                let i = cell * 4;
                let a = picture.data.get(i + 3).copied().unwrap_or(0);
                if a == 0 {
                    continue;
                }
                // Held premultiplied; un-premultiply for the straight page.
                let un = |c: u8| ((u16::from(c) * 255 + u16::from(a) / 2) / u16::from(a)).min(255) as u8;
                (
                    un(picture.data.get(i).copied().unwrap_or(0)),
                    un(picture.data.get(i + 1).copied().unwrap_or(0)),
                    un(picture.data.get(i + 2).copied().unwrap_or(0)),
                    a,
                )
            } else {
                let a = picture.data.get(cell).copied().unwrap_or(0);
                if a == 0 {
                    continue;
                }
                (fg[0], fg[1], fg[2], a)
            };
            let at = ((py as u32 * w + px as u32) * 4) as usize;
            let (da, sa_f) = (f32::from(page[at + 3]) / 255.0, f32::from(sa) / 255.0);
            let out_a = sa_f + da * (1.0 - sa_f);
            if out_a <= 0.0 {
                continue;
            }
            for (k, s) in [sr, sg, sb].iter().enumerate() {
                let d = f32::from(page[at + k]) / 255.0;
                let out = (f32::from(*s) / 255.0 * sa_f + d * da * (1.0 - sa_f)) / out_a;
                page[at + k] = (out * 255.0).round().clamp(0.0, 255.0) as u8;
            }
            page[at + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_renders_ink_over_paper_at_the_measured_cell() {
        let mut pager = Pager::load("monospace", 20).expect("fontconfig answers");
        let lines = ["Hello, page", "second line"];
        let (cw, ch) = pager.cell();
        assert_eq!(pager.size(&lines), Some((11 * cw, 2 * ch)));
        let (w, h, rgba) = pager
            .render(&lines, PLAIN, [240, 230, 210], [16, 14, 12, 255])
            .expect("page renders");
        assert_eq!((w, h), (11 * cw, 2 * ch));
        assert_eq!(rgba.len(), (w * h * 4) as usize);
        // Paper where nothing was written (last pixel), ink somewhere.
        assert_eq!(&rgba[rgba.len() - 4..], [16, 14, 12, 255]);
        let ink = rgba
            .chunks_exact(4)
            .filter(|px| px[0] > 100 && px[3] == 255)
            .count();
        assert!(ink > 100, "glyphs left ink (got {ink} bright pixels)");
        // Rendering again (memo warm) is the same page, byte for byte.
        let again = pager.render(&lines, PLAIN, [240, 230, 210], [16, 14, 12, 255]).unwrap();
        assert_eq!(again.2, rgba);
        // Empty and oversized pages are refused, not truncated.
        assert_eq!(pager.size(&[] as &[&str]), None);
        assert_eq!(pager.size(&["x".repeat(9000).as_str()]), None);
    }

    #[test]
    fn opaque_composite_matches_the_general_path_within_rounding() {
        // A mask glyph (half, full) and a premultiplied colour glyph,
        // over mid-grey paper, through both compositors.
        let bg = [100u8, 90, 80, 255];
        let mut general: Vec<u8> = bg.repeat(4);
        let mut fast = general.clone();
        let mask = Picture { width: 2, height: 1, left: 0, top: 0, colored: false, data: vec![128, 255] };
        let colour = Picture {
            width: 1,
            height: 1,
            left: 0,
            top: 0,
            colored: true,
            data: vec![64, 32, 16, 128],
        };
        composite(&mut general, 4, 1, &mask, 0, 0, [200, 150, 50]);
        composite_opaque(&mut fast, 4, 1, &mask, 0, 0, [200, 150, 50]);
        composite(&mut general, 4, 1, &colour, 2, 0, [0, 0, 0]);
        composite_opaque(&mut fast, 4, 1, &colour, 2, 0, [0, 0, 0]);
        for (a, b) in general.iter().zip(&fast) {
            assert!(a.abs_diff(*b) <= 1, "{general:?} vs {fast:?}");
        }
        assert_eq!(fast[3], 255, "paper stays opaque");
    }
}
