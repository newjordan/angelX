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
#[path = "../../tests/cockpit/app/dot_canvas__tests.rs"]
mod tests;
