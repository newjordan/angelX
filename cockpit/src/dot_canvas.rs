//! Generated Dotmax canvas: separated 1-pixel dots on equal pitch.
//!
//! Converts an already-composed [`ColoredBrailleImage`] into a physical raster.
//! Kitty transport, world sampling, filesystem, and terminal I/O live elsewhere.

use crate::terminal_art::{ColoredBrailleImage, braille_bits, braille_dot_bit};

const BG: [u8; 4] = [5, 8, 12, 255];
/// No background at all: the frame carries only ink, so the terminal composites
/// it over whatever the pane already painted behind the dots.
pub(crate) const TRANSPARENT: [u8; 4] = [0, 0, 0, 0];
const MAX_INK: u8 = 190;
const MAX_WIDTH: u32 = 2048;
const MAX_HEIGHT: u32 = 1536;
const MAX_PIXELS: u64 = 2_000_000;
const MIN_PITCH: u16 = 2;
const MAX_PITCH: u16 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DotGeometry {
    pub width: u32,
    pub height: u32,
    pub grid_width: usize,
    pub grid_height: usize,
    pub pitch: u32,
}

impl DotGeometry {
    pub(crate) fn new(
        columns: u16,
        rows: u16,
        cell_pixels: (u16, u16),
        pitch: u16,
    ) -> Option<Self> {
        if columns == 0 || rows == 0 || cell_pixels.0 == 0 || cell_pixels.1 == 0 {
            return None;
        }
        if !(MIN_PITCH..=MAX_PITCH).contains(&pitch) {
            return None;
        }
        let pitch = u32::from(pitch);
        let width = u32::from(columns).checked_mul(u32::from(cell_pixels.0))?;
        let height = u32::from(rows).checked_mul(u32::from(cell_pixels.1))?;
        if !canvas_bounds_ok(width, height) {
            return None;
        }
        let cell_w = pitch.checked_mul(2)?;
        let cell_h = pitch.checked_mul(4)?;
        let grid_width = usize::try_from(width / cell_w).ok()?;
        let grid_height = usize::try_from(height / cell_h).ok()?;
        if grid_width == 0 || grid_height == 0 {
            return None;
        }
        Some(Self {
            width,
            height,
            grid_width,
            grid_height,
            pitch,
        })
    }

    pub(crate) fn rasterize(&self, image: &ColoredBrailleImage) -> Option<image::RgbaImage> {
        self.rasterize_on(image, BG)
    }

    pub(crate) fn rasterize_on(
        &self,
        image: &ColoredBrailleImage,
        background: [u8; 4],
    ) -> Option<image::RgbaImage> {
        if !self.geometry_ok() {
            return None;
        }
        let expected = image.width.checked_mul(image.height)?;
        if image.cells.len() != expected {
            return None;
        }
        if image.width > self.grid_width || image.height > self.grid_height {
            return None;
        }
        let (pad_x, pad_y) = self.origin()?;
        let mut raster =
            image::RgbaImage::from_pixel(self.width, self.height, image::Rgba(background));
        if image.width == 0 || image.height == 0 {
            return Some(raster);
        }
        let cell_origin_x = (self.grid_width - image.width) / 2;
        let cell_origin_y = (self.grid_height - image.height) / 2;
        for cell_y in 0..image.height {
            for cell_x in 0..image.width {
                let index = cell_y.checked_mul(image.width)?.checked_add(cell_x)?;
                let cell = *image.cells.get(index)?;
                let bits = braille_bits(cell.glyph);
                if bits == 0 {
                    continue;
                }
                let ink = image::Rgba(clamp_ink(cell.fg));
                let grid_x = cell_origin_x + cell_x;
                let grid_y = cell_origin_y + cell_y;
                for local_y in 0..4 {
                    for local_x in 0..2 {
                        if bits & braille_dot_bit(local_x, local_y) == 0 {
                            continue;
                        }
                        let x = pad_x
                            + u32::try_from(grid_x).ok()? * 2 * self.pitch
                            + u32::try_from(local_x).ok()? * self.pitch;
                        let y = pad_y
                            + u32::try_from(grid_y).ok()? * 4 * self.pitch
                            + u32::try_from(local_y).ok()? * self.pitch;
                        if x >= self.width || y >= self.height {
                            return None;
                        }
                        raster.put_pixel(x, y, ink);
                    }
                }
            }
        }
        Some(raster)
    }

    fn geometry_ok(&self) -> bool {
        if self.pitch < u32::from(MIN_PITCH) || self.pitch > u32::from(MAX_PITCH) {
            return false;
        }
        if self.grid_width == 0 || self.grid_height == 0 {
            return false;
        }
        canvas_bounds_ok(self.width, self.height)
    }

    fn origin(&self) -> Option<(u32, u32)> {
        let used_w = u32::try_from(self.grid_width)
            .ok()?
            .checked_mul(self.pitch)?
            .checked_mul(2)?;
        let used_h = u32::try_from(self.grid_height)
            .ok()?
            .checked_mul(self.pitch)?
            .checked_mul(4)?;
        if used_w > self.width || used_h > self.height {
            return None;
        }
        Some(((self.width - used_w) / 2, (self.height - used_h) / 2))
    }
}

fn canvas_bounds_ok(width: u32, height: u32) -> bool {
    width > 0
        && height > 0
        && width <= MAX_WIDTH
        && height <= MAX_HEIGHT
        && u64::from(width).saturating_mul(u64::from(height)) <= MAX_PIXELS
}

fn clamp_ink([r, g, b]: [u8; 3]) -> [u8; 4] {
    [r.min(MAX_INK), g.min(MAX_INK), b.min(MAX_INK), 255]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_art::ColoredBrailleCell;

    const INK: [u8; 3] = [200, 40, 255];
    const CLAMPED: [u8; 4] = [190, 40, 190, 255];

    fn cell(glyph: char, fg: [u8; 3]) -> ColoredBrailleCell {
        ColoredBrailleCell { glyph, fg }
    }

    fn grid(width: usize, height: usize, cells: Vec<ColoredBrailleCell>) -> ColoredBrailleImage {
        ColoredBrailleImage {
            width,
            height,
            cells,
        }
    }

    fn single(glyph: char, fg: [u8; 3]) -> ColoredBrailleImage {
        grid(1, 1, vec![cell(glyph, fg)])
    }

    fn ink_pixels(raster: &image::RgbaImage) -> Vec<(u32, u32)> {
        raster
            .enumerate_pixels()
            .filter(|(_, _, pixel)| pixel.0 != BG)
            .map(|(x, y, _)| (x, y))
            .collect()
    }

    /// Standard 8-dot Braille: columns 0,1 and rows 0..4, bits 0..=7.
    const DOT_MAP: [(u32, u32, char); 8] = [
        (0, 0, '\u{2801}'),
        (0, 1, '\u{2802}'),
        (0, 2, '\u{2804}'),
        (1, 0, '\u{2808}'),
        (1, 1, '\u{2810}'),
        (1, 2, '\u{2820}'),
        (0, 3, '\u{2840}'),
        (1, 3, '\u{2880}'),
    ];

    #[test]
    fn eight_dot_positions_and_clamped_rgb() {
        let pitch = 4u16;
        let geometry = DotGeometry::new(1, 1, (8, 16), pitch).expect("1x1 cell canvas");
        assert_eq!(geometry.width, 8);
        assert_eq!(geometry.height, 16);
        assert_eq!(geometry.grid_width, 1);
        assert_eq!(geometry.grid_height, 1);
        assert_eq!(geometry.pitch, 4);
        let (pad_x, pad_y) = geometry.origin().expect("origin");
        assert_eq!((pad_x, pad_y), (0, 0));

        for (local_x, local_y, glyph) in DOT_MAP {
            let raster = geometry
                .rasterize(&single(glyph, INK))
                .expect("rasterize one dot");
            assert_eq!(raster.dimensions(), (8, 16));
            let expected = (
                pad_x + local_x * u32::from(pitch),
                pad_y + local_y * u32::from(pitch),
            );
            let lit = ink_pixels(&raster);
            assert_eq!(lit, vec![expected], "glyph {glyph:?}");
            assert_eq!(raster.get_pixel(expected.0, expected.1).0, CLAMPED);
        }

        let all = geometry
            .rasterize(&single('\u{28FF}', [255, 207, 92]))
            .expect("all eight dots");
        let lit = ink_pixels(&all);
        assert_eq!(lit.len(), 8);
        for (local_x, local_y, _) in DOT_MAP {
            let x = pad_x + local_x * u32::from(pitch);
            let y = pad_y + local_y * u32::from(pitch);
            assert_eq!(all.get_pixel(x, y).0, [190, 190, 92, 255]);
        }
        assert_eq!(all.get_pixel(1, 0).0, BG);
        assert_eq!(all.get_pixel(0, 1).0, BG);
    }

    #[test]
    fn equal_pitch_across_cell_and_row_boundaries() {
        let pitch = 3u32;
        let geometry = DotGeometry::new(1, 1, (12, 24), 3).expect("2x2 virtual cells");
        assert_eq!(
            (geometry.grid_width, geometry.grid_height, geometry.pitch),
            (2, 2, pitch)
        );
        let (pad_x, pad_y) = geometry.origin().expect("origin");
        assert_eq!((pad_x, pad_y), (0, 0));

        let cells = vec![
            cell('\u{28FF}', [80, 90, 100]),
            cell('\u{28FF}', [80, 90, 100]),
            cell('\u{28FF}', [80, 90, 100]),
            cell('\u{28FF}', [80, 90, 100]),
        ];
        let raster = geometry.rasterize(&grid(2, 2, cells)).expect("2x2 raster");
        let lit = ink_pixels(&raster);
        assert_eq!(lit.len(), 32);

        let xs: Vec<u32> = {
            let mut values: Vec<u32> = lit.iter().map(|(x, _)| *x).collect();
            values.sort_unstable();
            values.dedup();
            values
        };
        let ys: Vec<u32> = {
            let mut values: Vec<u32> = lit.iter().map(|(_, y)| *y).collect();
            values.sort_unstable();
            values.dedup();
            values
        };
        assert_eq!(xs, vec![0, 3, 6, 9]);
        assert_eq!(ys, vec![0, 3, 6, 9, 12, 15, 18, 21]);
        for window in xs.windows(2) {
            assert_eq!(window[1] - window[0], pitch);
        }
        for window in ys.windows(2) {
            assert_eq!(window[1] - window[0], pitch);
        }

        let right_of_first_cell = pad_x + pitch;
        let first_of_next_cell = pad_x + 2 * pitch;
        assert_eq!(first_of_next_cell - right_of_first_cell, pitch);
        let bottom_of_first_row = pad_y + 3 * pitch;
        let first_of_next_row = pad_y + 4 * pitch;
        assert_eq!(first_of_next_row - bottom_of_first_row, pitch);
        assert_eq!(raster.get_pixel(right_of_first_cell + 1, 0).0, BG);
    }

    #[test]
    fn centering_and_aspect_at_different_cell_sizes() {
        let compact = DotGeometry::new(10, 5, (8, 16), 2).expect("8x16 cells");
        assert_eq!((compact.width, compact.height), (80, 80));
        assert_eq!((compact.grid_width, compact.grid_height), (20, 10));
        assert_eq!(compact.origin(), Some((0, 0)));

        let padded = DotGeometry::new(10, 5, (9, 18), 2).expect("9x18 cells");
        assert_eq!((padded.width, padded.height), (90, 90));
        assert_eq!((padded.grid_width, padded.grid_height), (22, 11));
        assert_eq!(padded.origin(), Some((1, 1)));
        assert_eq!(
            padded.width as usize,
            padded.grid_width * 2 * padded.pitch as usize + 2
        );
        assert_eq!(
            padded.height as usize,
            padded.grid_height * 4 * padded.pitch as usize + 2
        );

        let wide = DotGeometry::new(4, 2, (16, 8), 2).expect("wide cells");
        let tall = DotGeometry::new(4, 2, (8, 16), 2).expect("tall cells");
        assert_eq!((wide.width, wide.height), (64, 16));
        assert_eq!((tall.width, tall.height), (32, 32));
        assert_ne!(
            wide.width * tall.height,
            tall.width * wide.height,
            "canvas aspect follows terminal cell size, not a warped square"
        );

        let odd = DotGeometry::new(1, 1, (11, 19), 4).expect("odd leftover");
        assert_eq!((odd.grid_width, odd.grid_height), (1, 1));
        assert_eq!(odd.origin(), Some((1, 1)));
        let raster = odd
            .rasterize(&single('\u{2801}', [10, 20, 30]))
            .expect("padded raster");
        assert_eq!(raster.dimensions(), (11, 19));
        assert_eq!(ink_pixels(&raster), vec![(1, 1)]);
        assert_eq!(raster.get_pixel(0, 1).0, BG);
        assert_eq!(raster.get_pixel(10, 18).0, BG);

        let host = DotGeometry::new(1, 1, (16, 32), 2).expect("4x4 host grid");
        assert_eq!((host.grid_width, host.grid_height), (4, 4));
        let small = grid(
            2,
            1,
            vec![
                cell('\u{2801}', [12, 34, 56]),
                cell('\u{2801}', [12, 34, 56]),
            ],
        );
        let centered = host.rasterize(&small).expect("centered smaller grid");
        let (pad_x, pad_y) = host.origin().expect("host origin");
        let cell_origin_x = (4 - 2) / 2;
        let cell_origin_y = (4 - 1) / 2;
        let left = pad_x + cell_origin_x as u32 * 2 * host.pitch;
        let right = pad_x + (cell_origin_x as u32 + 1) * 2 * host.pitch;
        let y = pad_y + cell_origin_y as u32 * 4 * host.pitch;
        assert_eq!(ink_pixels(&centered), vec![(left, y), (right, y)]);
        assert_eq!(centered.dimensions(), (host.width, host.height));
    }

    #[test]
    fn malformed_zero_and_huge_dimensions_reject() {
        assert!(DotGeometry::new(0, 4, (8, 16), 2).is_none());
        assert!(DotGeometry::new(4, 0, (8, 16), 2).is_none());
        assert!(DotGeometry::new(4, 4, (0, 16), 2).is_none());
        assert!(DotGeometry::new(4, 4, (8, 0), 2).is_none());
        assert!(DotGeometry::new(4, 4, (8, 16), 0).is_none());
        assert!(DotGeometry::new(4, 4, (8, 16), 1).is_none());
        assert!(DotGeometry::new(4, 4, (8, 16), 9).is_none());
        assert!(DotGeometry::new(257, 1, (8, 16), 2).is_none());
        assert!(DotGeometry::new(1, 97, (8, 16), 2).is_none());
        assert!(DotGeometry::new(2048, 977, (1, 1), 2).is_none());
        assert!(DotGeometry::new(1, 1, (8, 16), 8).is_none());
        assert!(DotGeometry::new(u16::MAX, u16::MAX, (u16::MAX, u16::MAX), 2).is_none());

        let ok = DotGeometry::new(2, 2, (8, 16), 2).expect("valid");
        let mismatch = ColoredBrailleImage {
            width: 2,
            height: 2,
            cells: vec![cell('\u{28FF}', INK)],
        };
        assert!(ok.rasterize(&mismatch).is_none());
        let oversized = grid(
            ok.grid_width + 1,
            1,
            vec![cell('\u{2801}', INK); ok.grid_width + 1],
        );
        assert!(ok.rasterize(&oversized).is_none());

        let huge = DotGeometry {
            width: 10_000,
            height: 8_000,
            grid_width: 10,
            grid_height: 10,
            pitch: 4,
        };
        assert!(huge.rasterize(&single('\u{28FF}', INK)).is_none());
        let zero_pitch = DotGeometry {
            width: 16,
            height: 16,
            grid_width: 1,
            grid_height: 1,
            pitch: 0,
        };
        assert!(zero_pitch.rasterize(&single('\u{28FF}', INK)).is_none());
    }

    #[test]
    fn non_braille_glyphs_never_become_solid_blocks() {
        let geometry = DotGeometry::new(2, 2, (8, 16), 2).expect("canvas");
        let glyphs = ['A', '█', ' ', '#', '\u{FFFD}', '\u{2593}', '\u{25A0}'];
        let cells: Vec<_> = glyphs
            .into_iter()
            .cycle()
            .take(geometry.grid_width * geometry.grid_height)
            .map(|glyph| cell(glyph, [255, 255, 255]))
            .collect();
        let raster = geometry
            .rasterize(&grid(geometry.grid_width, geometry.grid_height, cells))
            .expect("non-braille raster");
        assert!(
            ink_pixels(&raster).is_empty(),
            "non-Braille glyphs must not light any pixels"
        );
        assert!(raster.pixels().all(|pixel| pixel.0 == BG));

        let mixed = grid(
            2,
            1,
            vec![cell('█', [255, 0, 0]), cell('\u{2801}', [20, 30, 40])],
        );
        let mixed_raster = geometry.rasterize(&mixed).expect("mixed");
        assert_eq!(ink_pixels(&mixed_raster).len(), 1);
    }
}
