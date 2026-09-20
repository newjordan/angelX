//! Single-shot braille renderer: PNG path → braille text on stdout.
//!
//! Designed for shell-out from external tools (ComfyUI, scripts, editors).
//! All rendering parameters come from env vars so the CLI surface stays
//! tiny — three positional args.
//!
//! Usage:
//!   dotmax-braille <input.png> <width_cells> <height_cells>
//!
//! Env vars (all optional, sensible defaults):
//!   DOTMAX_DITHER       floyd | bayer | atkinson | none      (default: atkinson)
//!   DOTMAX_THRESHOLD    0..255 | auto                        (default: 128)
//!   DOTMAX_BRIGHTNESS   0.0..2.0                             (default: 1.0)
//!   DOTMAX_CONTRAST     0.0..2.0                             (default: 1.3)
//!   DOTMAX_GAMMA        0.1..3.0                             (default: 1.0)
//!   DOTMAX_INVERT       true|false  (invert source pixels)   (default: true)
//!   DOTMAX_INVERT_OUTPUT true|false (flip braille bits)      (default: false)
//!   DOTMAX_AMBIENT      0.0..1.0    (temporal jitter)        (default: 0.55)
//!   DOTMAX_SEED         u64         (0 → clock-driven)       (default: 0)
//!   DOTMAX_FRAME        u64         (advances jitter)        (default: 0)

use dotmax::image::{DitheringMethod, ImageRenderer};
use std::io::Write;

fn env_str(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

fn env_f32(key: &str, default: f32) -> f32 {
    env_str(key).and_then(|s| s.parse().ok()).unwrap_or(default)
}

fn env_u8(key: &str, default: u8) -> Option<u8> {
    match env_str(key).as_deref() {
        Some("auto" | "AUTO" | "") => None,
        Some(s) => s.parse().ok(),
        None => Some(default),
    }
}

fn env_u64(key: &str, default: u64) -> u64 {
    env_str(key).and_then(|s| s.parse().ok()).unwrap_or(default)
}

fn env_bool(key: &str, default: bool) -> bool {
    match env_str(key).as_deref() {
        Some("1" | "true" | "TRUE" | "yes" | "YES") => true,
        Some("0" | "false" | "FALSE" | "no" | "NO") => false,
        _ => default,
    }
}

fn env_dither() -> DitheringMethod {
    match env_str("DOTMAX_DITHER")
        .as_deref()
        .map(str::to_lowercase)
        .as_deref()
    {
        Some("floyd" | "floydsteinberg" | "floyd_steinberg") => DitheringMethod::FloydSteinberg,
        Some("bayer") => DitheringMethod::Bayer,
        Some("none") => DitheringMethod::None,
        _ => DitheringMethod::Atkinson,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: dotmax-braille <input.png> <width_cells> <height_cells>")?;
    let width: usize = args
        .next()
        .ok_or("missing width argument")?
        .parse()
        .map_err(|e| format!("invalid width: {e}"))?;
    let height: usize = args
        .next()
        .ok_or("missing height argument")?
        .parse()
        .map_err(|e| format!("invalid height: {e}"))?;

    let dither = env_dither();
    let threshold = env_u8("DOTMAX_THRESHOLD", 128);
    let brightness = env_f32("DOTMAX_BRIGHTNESS", 1.0);
    let contrast = env_f32("DOTMAX_CONTRAST", 1.3);
    let gamma = env_f32("DOTMAX_GAMMA", 1.0);
    let invert = env_bool("DOTMAX_INVERT", true);
    let invert_output = env_bool("DOTMAX_INVERT_OUTPUT", false);
    let ambient = env_f32("DOTMAX_AMBIENT", 0.55).clamp(0.0, 1.0);
    let seed = env_u64("DOTMAX_SEED", 0);
    let frame = env_u64("DOTMAX_FRAME", 0);

    let src = image::open(&path).map_err(|e| format!("failed to open {path}: {e}"))?;
    let mut rgba = src.to_rgba8();
    if invert {
        for px in rgba.pixels_mut() {
            px.0[0] = 255 - px.0[0];
            px.0[1] = 255 - px.0[1];
            px.0[2] = 255 - px.0[2];
        }
    }

    let mut builder = ImageRenderer::new()
        .load_from_rgba(rgba)
        .resize(width, height, true)?
        .dithering(dither)
        .ambient(ambient)
        .ambient_frame(frame);

    if seed != 0 {
        builder = builder.ambient_seed(seed);
    }

    if (brightness - 1.0).abs() > f32::EPSILON {
        builder = builder.brightness(brightness)?;
    }
    if (contrast - 1.0).abs() > f32::EPSILON {
        builder = builder.contrast(contrast)?;
    }
    if (gamma - 1.0).abs() > f32::EPSILON {
        builder = builder.gamma(gamma)?;
    }
    if let Some(t) = threshold {
        builder = builder.threshold(t);
    }

    let grid = builder.render()?;
    let mut unicode = grid.to_unicode_grid();
    if invert_output {
        for row in unicode.iter_mut() {
            for ch in row.iter_mut() {
                let bits = (*ch as u32).saturating_sub(0x2800) as u8;
                *ch = char::from_u32(0x2800 + (!bits as u32)).unwrap_or(*ch);
            }
        }
    }

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for row in &unicode {
        for ch in row {
            let mut buf = [0u8; 4];
            out.write_all(ch.encode_utf8(&mut buf).as_bytes())?;
        }
        out.write_all(b"\n")?;
    }
    out.flush()?;
    Ok(())
}
