//! Compressed transport for generated dots, with independently owned cells.
//!
//! Kitty PNG transport and Unicode placeholder rules:
//! https://sw.kovidgoyal.net/kitty/graphics-protocol/
//! The only encoder input is a composed Braille grid, never a source image.

use crate::dot_canvas::DotGeometry;
use crate::terminal_art::ColoredBrailleImage;
use base64::Engine;
use image::ImageEncoder;
use ratatui::{
    buffer::{Buffer, CellDiffOption},
    layout::{Rect, Size},
    style::Color,
};
use std::fmt::Write;
use std::num::NonZeroU16;

include!("dot_diacritics.rs");

pub(crate) struct DotProtocol {
    size: Size,
    id: u32,
    upload: Option<String>,
}

impl DotProtocol {
    pub(crate) fn encode(
        geometry: DotGeometry,
        image: &ColoredBrailleImage,
        size: Size,
        id: u32,
    ) -> Result<Self, String> {
        Self::encode_on(geometry, image, size, id, None)
    }

    pub(crate) fn encode_on(
        geometry: DotGeometry,
        image: &ColoredBrailleImage,
        size: Size,
        id: u32,
        background: Option<[u8; 4]>,
    ) -> Result<Self, String> {
        if size.width == 0
            || size.height == 0
            || size.width > 256
            || size.height > 256
            || id == 0
            || id > 0x00ff_ffff
        {
            return Err("dot placement outside bounded cell/id range".into());
        }
        let rgba = match background {
            Some(background) => geometry.rasterize_on(image, background),
            None => geometry.rasterize(image),
        }
        .ok_or("invalid generated dot canvas")?;
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new_with_quality(
            &mut png,
            image::codecs::png::CompressionType::Default,
            image::codecs::png::FilterType::Up,
        )
        .write_image(
            rgba.as_raw(),
            geometry.width,
            geometry.height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|error| error.to_string())?;
        if png.len() > 384 * 1024 {
            return Err("dot frame exceeds bounded terminal payload".into());
        }
        let payload = base64::engine::general_purpose::STANDARD.encode(png);
        let mut upload = String::with_capacity(payload.len() + payload.len() / 100);
        let chunks = payload.as_bytes().chunks(4096);
        let count = chunks.len();
        for (index, chunk) in chunks.enumerate() {
            let more = u8::from(index + 1 < count);
            if index == 0 {
                write!(
                    upload,
                    "\x1b_Gq=2,a=T,U=1,f=100,C=1,i={id},c={},r={},m={more};",
                    size.width, size.height
                )
                .unwrap();
            } else {
                write!(upload, "\x1b_Gq=2,m={more};").unwrap();
            }
            upload.push_str(std::str::from_utf8(chunk).expect("base64 is ASCII"));
            upload.push_str("\x1b\\");
        }
        Ok(Self {
            size,
            id,
            upload: Some(upload),
        })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn size(&self) -> Size {
        self.size
    }

    pub(crate) fn image_id(&self) -> u32 {
        self.id
    }

    pub(crate) fn take_upload(&mut self) -> Option<String> {
        self.upload.take()
    }

    pub(crate) fn render(&self, area: Rect, buffer: &mut Buffer) {
        let color = Color::Rgb((self.id >> 16) as u8, (self.id >> 8) as u8, self.id as u8);
        let width = area.width.min(self.size.width);
        let height = area.height.min(self.size.height);
        for y in 0..height {
            for x in 0..width {
                if let Some(cell) = buffer.cell_mut((area.x + x, area.y + y)) {
                    // Each cell carries explicit row/column coordinates. Overlays
                    // may replace any cell without a hidden row write erasing them.
                    cell.set_symbol(&format!(
                        "\u{10eeee}{}{}",
                        DOT_DIACRITICS[usize::from(y)],
                        DOT_DIACRITICS[usize::from(x)]
                    ))
                    .set_fg(color)
                    .set_diff_option(CellDiffOption::ForcedWidth(NonZeroU16::new(1).unwrap()));
                }
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn decode_upload(upload: &str) -> image::RgbaImage {
    let mut payload = String::new();
    for command in upload.split("\x1b\\").filter(|command| !command.is_empty()) {
        let (_, chunk) = command.split_once(';').expect("kitty command payload");
        assert!(chunk.len() <= 4096, "kitty payload chunk must stay bounded");
        payload.push_str(chunk);
    }
    image::load_from_memory(
        &base64::engine::general_purpose::STANDARD
            .decode(payload)
            .expect("base64 payload"),
    )
    .expect("PNG payload")
    .to_rgba8()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_art::ColoredBrailleCell;

    #[test]
    fn compressed_payload_decodes_to_exact_generated_dots_and_uploads_once() {
        let geometry = DotGeometry::new(40, 20, (8, 16), 2).unwrap();
        let image = ColoredBrailleImage {
            width: geometry.grid_width,
            height: geometry.grid_height,
            cells: vec![
                ColoredBrailleCell {
                    glyph: '\u{28a5}',
                    fg: [100, 140, 180]
                };
                geometry.grid_width * geometry.grid_height
            ],
        };
        let mut protocol =
            DotProtocol::encode(geometry, &image, Size::new(40, 20), 0x123456).unwrap();
        let upload = protocol.take_upload().unwrap();
        assert!(protocol.take_upload().is_none());
        assert!(upload.contains("f=100") && upload.contains("c=40,r=20"));
        let decoded = decode_upload(&upload);
        assert_eq!(decoded, geometry.rasterize(&image).unwrap());
        assert!(
            upload.len() < decoded.as_raw().len() / 8,
            "sparse dots should compress substantially"
        );
        eprintln!(
            "dot_payload raw_rgba_bytes={} wire_bytes={}",
            decoded.as_raw().len(),
            upload.len()
        );
    }

    #[test]
    fn dot_cells_do_not_own_neighbors_or_erase_overlays() {
        let geometry = DotGeometry::new(4, 3, (8, 16), 2).unwrap();
        let image = ColoredBrailleImage {
            width: 1,
            height: 1,
            cells: vec![ColoredBrailleCell {
                glyph: '\u{28ff}',
                fg: [100, 100, 100],
            }],
        };
        let protocol = DotProtocol::encode(geometry, &image, Size::new(4, 3), 17).unwrap();
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 8));
        buffer.cell_mut((0, 0)).unwrap().set_symbol("H");
        protocol.render(Rect::new(2, 2, 4, 3), &mut buffer);
        buffer
            .cell_mut((3, 3))
            .unwrap()
            .set_symbol("X")
            .set_fg(Color::White);
        assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "H");
        assert_eq!(buffer.cell((3, 3)).unwrap().symbol(), "X");
        assert_eq!(buffer.cell((6, 3)).unwrap().symbol(), " ");
        assert!(
            buffer
                .content()
                .iter()
                .all(|cell| cell.diff_option != CellDiffOption::Skip)
        );
        assert_ne!(
            buffer.cell((2, 2)).unwrap().symbol(),
            buffer.cell((3, 2)).unwrap().symbol()
        );
    }
}
