//! The window's background image (`background_image`): under everything,
//! laid out one of a few ways, tinted toward the background colour so text
//! stays readable. Every layout is one quad; the sampler's
//! address mode does the tiling, mirroring and edge clamping.

use std::path::{Path, PathBuf};

/// The layouts and the decoded image are the renderer core's (`angel-gpu`):
/// this module finds the file and decodes it.
pub use angel_gpu::{Layout, Wallpaper};

/// Where the background image comes from and how it shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub path: PathBuf,
    pub layout: Layout,
    /// 0 shows the image as it is; 1 hides it under the background colour.
    pub tint: f32,
    /// Smooth scaling instead of nearest pixels.
    pub linear: bool,
}

/// Load the PNG the settings name. `None` (with one line on stderr) when
/// it can't be read.
pub fn load(settings: &Settings) -> Option<Wallpaper> {
    match decode(&settings.path) {
        Some((width, height, rgba)) => Some(Wallpaper {
            width,
            height,
            rgba,
            layout: settings.layout,
            tint: settings.tint,
            linear: settings.linear,
        }),
        None => {
            eprintln!(
                "GLASS: background image {} is not a readable PNG",
                settings.path.display()
            );
            None
        }
    }
}

/// A PNG as straight 8-bit RGBA.
fn decode(path: &Path) -> Option<(u32, u32, Vec<u8>)> {
    let file = std::io::BufReader::new(std::fs::File::open(path).ok()?);
    let mut decoder = png::Decoder::new(file);
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    // Size is checked before the frame is allocated, and capped at the
    // texture limit the GPU upload must meet anyway.
    if !(1..=8192).contains(&reader.info().width) || !(1..=8192).contains(&reader.info().height) {
        return None;
    }
    let mut buffer = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buffer).ok()?;
    let pixels = &buffer[..info.buffer_size()];
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        png::ColorType::Rgb => pixels
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => pixels
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => pixels.iter().flat_map(|&v| [v, v, v, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    (rgba.len() == (info.width as usize) * (info.height as usize) * 4)
        .then_some((info.width, info.height, rgba))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_a_png() {
        let path = std::env::temp_dir().join(format!("glass-wallpaper-{}.png", std::process::id()));
        {
            let file = std::fs::File::create(&path).unwrap();
            let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), 2, 1);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[1, 2, 3, 4, 5, 6]).unwrap();
        }
        let settings = Settings {
            path: path.clone(),
            layout: Layout::Centered,
            tint: 0.5,
            linear: false,
        };
        let wallpaper = load(&settings).unwrap();
        assert_eq!((wallpaper.width, wallpaper.height), (2, 1));
        assert_eq!(wallpaper.rgba, [1, 2, 3, 255, 4, 5, 6, 255]);
        let _ = std::fs::remove_file(path);
    }
}
