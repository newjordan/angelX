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

/// Load the image the settings name at an explicit side budget (see
/// [`crate::graphics::picture_side_budget`]): integrated GPUs pass 4096
/// so a hostile 8K desktop cannot OOM the session the way kitty's
/// unbounded upload can; RTX/discrete passes 8192 for full artist
/// exports on this host's RTX 3090. The renderer passes its adapter's
/// budget here.
///
/// Header-first: the [`plan_in_budget`] reserve names the kind, MIME and
/// fitted layout box from the file prefix without decoding pixels
/// (serve/showcase answer `Content-Type`-style headers from the same call),
/// and routed kinds are held here with their pipeline reason before the
/// decoder is ever paid for — where kitty must decode (or base64-decode per
/// frame) to learn the size.
/// Oversized images downscale to the budget instead of failing the
/// background; `None` (with one line on stderr) when it can't be read.
pub fn load_in_budget(settings: &Settings, budget: u32) -> Option<Wallpaper> {
    // Header-first gate at the adapter's side budget:
    // RTX/discrete reserves full 8K boxes, integrated reserves 4096 boxes,
    // and routed/unknown bytes never reach the decoder.
    if let Some(planned) = plan_in_budget(&settings.path, budget) {
        if planned.is_held() {
            eprintln!(
                "GLASS: background image {} needs {}",
                settings.path.display(),
                planned.routed_reason.unwrap_or("a routed pipeline")
            );
            return None;
        }
    }
    match decode_in_budget(&settings.path, budget) {
        Some(decoded) => Some(wallpaper_of(settings, decoded)),
        None => {
            eprintln!(
                "GLASS: background image {} is not a readable image",
                settings.path.display()
            );
            None
        }
    }
}

/// Build the window's background quad from a decoded `(width, height, rgba)`.
fn wallpaper_of(settings: &Settings, decoded: (u32, u32, Vec<u8>)) -> Wallpaper {
    Wallpaper {
        width: decoded.0,
        height: decoded.1,
        rgba: decoded.2,
        layout: settings.layout,
        tint: settings.tint,
        linear: settings.linear,
    }
}

/// Header-only serve plan for the background file `path` at an explicit side
/// budget (see [`crate::graphics::picture_side_budget`]): RTX/discrete passes 8192 for
/// full 8K artist exports on this host's RTX 3090 (256 MiB cap),
/// integrated passes 4096 (64 MiB cap) so a hostile desktop cannot OOM the
/// session the way kitty's unbounded upload can.
pub fn plan_in_budget(path: &Path, budget: u32) -> Option<crate::graphics::ServePlan> {
    let bytes = std::fs::read(path).ok()?;
    crate::graphics::serve_plan(&bytes, Some(path), budget)
}

/// Decoded `(width, height, RGBA)` capped at `budget` a side.
fn decode_in_budget(path: &Path, budget: u32) -> Option<(u32, u32, Vec<u8>)> {
    let bytes = std::fs::read(path).ok()?;
    // Probe first: the magic sniff wins, the path's extension rescues serving
    // when the prefix is truncated or a text preamble pushes `<svg` past the
    // sniff window. Unknown/routed bytes are rejected without paying the
    // decoder. Direct kinds reuse the sniffed format so `image` skips
    // its own guess loop: one magic scan per image instead of two.
    let kind = crate::graphics::MediaKind::probe_path(&bytes, Some(path))?;
    let format = kind.image_format()?;
    let decoded = image::load_from_memory_with_format(&bytes, format).ok()?;
    let rgba = decoded.to_rgba8();
    let (width, height) = (rgba.width(), rgba.height());
    if width == 0 || height == 0 {
        return None;
    }
    let budget = budget.clamp(1, crate::graphics::MAX_TEXTURE_SIDE);
    if width <= budget && height <= budget {
        return Some((width, height, rgba.into_raw()));
    }
    let (tw, th) = crate::graphics::fit_inside(width, height, budget);
    let out = image::imageops::resize(&rgba, tw, th, image::imageops::FilterType::Triangle);
    Some((tw, th, out.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_png(path: &std::path::Path) {
        let file = std::fs::File::create(path).unwrap();
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), 2, 1);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[1, 2, 3, 4, 5, 6]).unwrap();
    }

    fn image_bytes(width: u32, height: u32, format: image::ImageFormat) -> Vec<u8> {
        let mut pixels = image::RgbImage::new(width, height);
        for (x, y, pixel) in pixels.enumerate_pixels_mut() {
            *pixel = image::Rgb([(x * 37 + y * 91) as u8, (x * 11 + 7) as u8, 200]);
        }
        let dynamic = image::DynamicImage::ImageRgb8(pixels);
        let mut out = std::io::Cursor::new(Vec::new());
        dynamic.write_to(&mut out, format).unwrap();
        out.into_inner()
    }

    fn load_bytes_via_file(bytes: &[u8], suffix: &str) -> Option<Wallpaper> {
        let path = std::env::temp_dir().join(format!(
            "glass-wallpaper-{}-{suffix}",
            std::process::id()
        ));
        std::fs::write(&path, bytes).unwrap();
        let settings = Settings {
            path: path.clone(),
            layout: Layout::Centered,
            tint: 0.0,
            linear: false,
        };
        let loaded = load_in_budget(&settings, crate::graphics::MAX_TEXTURE_SIDE);
        let _ = std::fs::remove_file(&path);
        loaded
    }

    #[test]
    fn loads_a_png() {
        let path = std::env::temp_dir().join(format!("glass-wallpaper-{}.png", std::process::id()));
        write_png(&path);
        let settings = Settings {
            path: path.clone(),
            layout: Layout::Centered,
            tint: 0.5,
            linear: false,
        };
        let wallpaper = load_in_budget(&settings, crate::graphics::MAX_TEXTURE_SIDE).unwrap();
        assert_eq!((wallpaper.width, wallpaper.height), (2, 1));
        assert_eq!(wallpaper.rgba, [1, 2, 3, 255, 4, 5, 6, 255]);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn loads_jpeg_and_bmp_wallpapers() {
        for (format, suffix) in [
            (image::ImageFormat::Jpeg, "jpg"),
            (image::ImageFormat::Bmp, "bmp"),
        ] {
            let bytes = image_bytes(3, 2, format);
            let wallpaper =
                load_bytes_via_file(&bytes, suffix).unwrap_or_else(|| panic!("loads {suffix}"));
            assert_eq!((wallpaper.width, wallpaper.height), (3, 2));
            assert_eq!(wallpaper.rgba.len(), 3 * 2 * 4);
            // Opaque alpha throughout: JPEG has no alpha, BMP decodes opaque.
            for chunk in wallpaper.rgba.chunks_exact(4) {
                assert_eq!(chunk[3], 255, "{suffix} alpha");
            }
        }
    }

    #[test]
    fn loads_gif_wallpaper_first_frame() {
        let bytes = image_bytes(2, 2, image::ImageFormat::Gif);
        let wallpaper = load_bytes_via_file(&bytes, "gif").expect("loads gif");
        assert_eq!((wallpaper.width, wallpaper.height), (2, 2));
        assert_eq!(wallpaper.rgba.len(), 2 * 2 * 4);
    }

    #[test]
    fn rejects_unknown_bytes() {
        let wallpaper = load_bytes_via_file(b"not an image at all", "bin");
        assert!(wallpaper.is_none());
    }
}
