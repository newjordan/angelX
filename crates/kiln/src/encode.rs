//! Kiln's pictures written out: PNG and animated GIF, both straight from
//! ink numbers with the realm's inks as their palette, so nothing is
//! quantised and nothing leaves the palette. Ink 0, clear, is transparent.
//! And a PNG read back, for the look tools' references and floors.

use crate::palette::{Ink, Palette};
use crate::raster::Raster;
use std::borrow::Cow;

/// An indexed picture: an ink number a pixel, row by row; 0 is clear.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Indexed {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

impl Indexed {
    /// A clear `w` x `h` picture.
    pub fn new(w: u32, h: u32) -> Indexed {
        Indexed { w, h, px: vec![0; (w * h) as usize] }
    }

    /// A raster's inks.
    pub fn of(r: &Raster) -> Indexed {
        Indexed { w: r.w, h: r.h, px: r.ink.iter().map(|i| i.0).collect() }
    }

    /// Every pixel `ink`.
    pub fn fill(&mut self, ink: Ink) {
        self.px.fill(ink.0);
    }

    /// `ink` in the box `(x, y, w, h)`, held to the picture.
    pub fn rect(&mut self, x: i32, y: i32, w: u32, h: u32, ink: Ink) {
        for yy in y.max(0)..(y + h as i32).min(self.h as i32) {
            for xx in x.max(0)..(x + w as i32).min(self.w as i32) {
                self.px[(yy as u32 * self.w + xx as u32) as usize] = ink.0;
            }
        }
    }

    /// `src` laid at `(x, y)`, each of its pixels `scale` pixels square;
    /// its clear pixels leave what is under them.
    pub fn blit(&mut self, src: &Indexed, x: i32, y: i32, scale: u32) {
        let s = scale.max(1) as i32;
        for sy in 0..src.h as i32 {
            for sx in 0..src.w as i32 {
                let v = src.px[(sy as u32 * src.w + sx as u32) as usize];
                if v == 0 {
                    continue;
                }
                for dy in 0..s {
                    let ty = y + sy * s + dy;
                    if ty < 0 || ty >= self.h as i32 {
                        continue;
                    }
                    for dx in 0..s {
                        let tx = x + sx * s + dx;
                        if tx >= 0 && tx < self.w as i32 {
                            self.px[(ty as u32 * self.w + tx as u32) as usize] = v;
                        }
                    }
                }
            }
        }
    }

    /// The picture `n` times bigger, each pixel a square.
    pub fn scaled(&self, n: u32) -> Indexed {
        let mut out = Indexed::new(self.w * n, self.h * n);
        out.blit(self, 0, 0, n);
        out
    }
}

/// The palette as RGB bytes, clear first: `n` entries, at least the inks.
fn table(palette: &Palette, n: usize) -> Vec<u8> {
    let mut out = vec![0u8; n.max(palette.len() + 1) * 3];
    for ink in palette.inks() {
        let [r, g, b] = palette.rgb(ink).unwrap_or([0, 0, 0]);
        out[ink.0 as usize * 3..ink.0 as usize * 3 + 3].copy_from_slice(&[r, g, b]);
    }
    out
}

/// A PNG of `img`, indexed, the clear transparent.
pub fn png(img: &Indexed, palette: &Palette) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, img.w, img.h);
        e.set_color(png::ColorType::Indexed);
        e.set_depth(png::BitDepth::Eight);
        e.set_palette(table(palette, 0));
        e.set_trns(vec![0u8]);
        let mut w = e.write_header().map_err(|e| e.to_string())?;
        w.write_image_data(&img.px).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

/// A PNG of RGBA bytes, row by row: for sheets that set kiln's pictures
/// beside pictures from elsewhere.
pub fn png_rgba(w: u32, h: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        let mut w = e.write_header().map_err(|e| e.to_string())?;
        w.write_image_data(rgba).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

/// An animated GIF being written, the realm's inks its palette.
pub struct Gif {
    enc: gif::Encoder<Vec<u8>>,
    w: u16,
    h: u16,
}

impl Gif {
    /// A GIF of `w` x `h` frames that plays round and round.
    pub fn new(w: u32, h: u32, palette: &Palette) -> Result<Gif, String> {
        let (w, h) = (u16::try_from(w).map_err(|_| "too wide for a GIF")?, u16::try_from(h).map_err(|_| "too tall for a GIF")?);
        let mut enc = gif::Encoder::new(Vec::new(), w, h, &table(palette, 128)).map_err(|e| e.to_string())?;
        enc.set_repeat(gif::Repeat::Infinite).map_err(|e| e.to_string())?;
        Ok(Gif { enc, w, h })
    }

    /// A frame shown for `delay` hundredths of a second.
    pub fn frame(&mut self, img: &Indexed, delay: u16) -> Result<(), String> {
        if (img.w, img.h) != (u32::from(self.w), u32::from(self.h)) {
            return Err(format!("a {}x{} frame in a {}x{} GIF", img.w, img.h, self.w, self.h));
        }
        let f = gif::Frame {
            width: self.w,
            height: self.h,
            buffer: Cow::Borrowed(&img.px),
            delay,
            transparent: Some(0),
            dispose: gif::DisposalMethod::Background,
            ..Default::default()
        };
        self.enc.write_frame(&f).map_err(|e| e.to_string())
    }

    /// The GIF's bytes.
    pub fn finish(self) -> Result<Vec<u8>, String> {
        self.enc.into_inner().map_err(|e| e.to_string())
    }
}

/// A PNG read back as RGBA bytes, row by row, whatever its colour type:
/// a reference to compare against, a floor to stand a figure on.
pub fn read_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut d = png::Decoder::new(std::io::Cursor::new(bytes));
    d.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut r = d.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; r.output_buffer_size().ok_or("a PNG too big to read")?];
    let info = r.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let src = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => src.to_vec(),
        png::ColorType::Rgb => src.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => src.chunks(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
        png::ColorType::Grayscale => src.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("an indexed PNG that did not expand".into()),
    };
    Ok((info.width, info.height, rgba))
}

/// A frame's delay in hundredths of a second at `fps`.
pub fn delay(fps: u32) -> u16 {
    (100.0 / fps.max(1) as f32).round() as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pictures_write_as_png_and_gif() {
        let p = Palette::realm();
        let mut img = Indexed::new(4, 3);
        img.rect(1, 1, 2, 1, p.ink('W').unwrap());
        let big = img.scaled(3);
        assert_eq!((big.w, big.h), (12, 9));
        assert_eq!(big.px[(4 * 12 + 4) as usize], p.ink('W').unwrap().0);
        let bytes = png(&img, p).unwrap();
        assert_eq!(&bytes[1..4], b"PNG");
        let mut g = Gif::new(4, 3, p).unwrap();
        g.frame(&img, delay(12)).unwrap();
        g.frame(&Indexed::new(4, 3), delay(12)).unwrap();
        let bytes = g.finish().unwrap();
        assert_eq!(&bytes[..6], b"GIF89a");
        let (w, h, rgba) = read_png(&png(&img, p).unwrap()).unwrap();
        assert_eq!((w, h), (4, 3));
        assert_eq!(&rgba[(4 + 1) * 4..(4 + 1) * 4 + 4], &[0xd5, 0xde, 0xe0, 255]);
        assert_eq!(rgba[3], 0);
        assert_eq!(delay(12), 8);
    }
}
