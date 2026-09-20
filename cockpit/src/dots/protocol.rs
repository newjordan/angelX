//! Compressed transport for generated dots, with independently owned cells.
//!
//! Kitty PNG transport and Unicode placeholder rules:
//! https://sw.kovidgoyal.net/kitty/graphics-protocol/
//! The only encoder input is a composed Braille grid, never a source image.

use crate::dots::canvas::DotGeometry;
use crate::term::art::ColoredBrailleImage;
use base64::Engine;
use image::ImageEncoder;
use ratatui::{
    buffer::{Buffer, CellDiffOption},
    layout::{Rect, Size},
    style::Color,
};
use std::fmt::Write;
use std::num::NonZeroU16;

include!("diacritics.rs");

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
#[path = "../../../tests/cockpit/app/dot_protocol__tests.rs"]
mod tests;
