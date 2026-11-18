//! Pictures in the window's cells.
//!
//! The cockpit hands the window a picture's pixels (`Host::upload_picture`)
//! and marks the cells it shows in: each such cell is the placeholder
//! character with two marks for its row and column in the picture's grid
//! (and a third for the id's high byte), its foreground colour naming the
//! picture's id. The renderer reads those cells every frame and draws each
//! one's share of the picture. No terminal escape codes are involved.
//!
//! This module is CPU only: reading picture cells and the cell-to-pixel
//! maths. The GPU side lives in `angel-gpu`.

use std::path::Path;

use ratatui_core::style::Color;

use angel_stage::marks;

pub const PLACEHOLDER: char = '\u{10EEEE}';
const PLACEHOLDER_UTF8: [u8; 4] = [0xF4, 0x8E, 0xBB, 0xAE];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placeholder {
    pub image: u32,
    pub row: u16,
    pub col: u16,
}

#[inline]
pub fn is_placeholder(symbol: &str) -> bool {
    symbol.as_bytes().starts_with(&PLACEHOLDER_UTF8)
}

#[inline]
pub fn read_placeholder(symbol: &str, fg: Color) -> Option<Placeholder> {
    if !is_placeholder(symbol) {
        return None;
    }
    let mut chars = symbol[4..].chars();
    let row = diacritic(chars.next()?)?;
    let col = diacritic(chars.next()?)?;
    let high = match chars.next() {
        Some(c) => diacritic(c)?,
        None => 0,
    };
    if chars.next().is_some() {
        return None;
    }
    Some(Placeholder {
        image: color_id(fg) | (u32::from(high & 0xff) << 24),
        row,
        col,
    })
}

#[inline]
fn diacritic(c: char) -> Option<u16> {
    marks::index(c)
}

fn color_id(color: Color) -> u32 {
    match color {
        Color::Rgb(r, g, b) => (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b),
        Color::Indexed(n) => u32::from(n),
        Color::Black => 0,
        Color::Red => 1,
        Color::Green => 2,
        Color::Yellow => 3,
        Color::Blue => 4,
        Color::Magenta => 5,
        Color::Cyan => 6,
        Color::Gray => 7,
        Color::DarkGray => 8,
        Color::LightRed => 9,
        Color::LightGreen => 10,
        Color::LightYellow => 11,
        Color::LightBlue => 12,
        Color::LightMagenta => 13,
        Color::LightCyan => 14,
        Color::White => 15,
        Color::Reset => 0,
    }
}

pub use angel_gpu::{Picture, premultiply_in_place, well_formed};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaKind {
    Png,
    Jpeg,
    Gif,
    Bmp,
    WebP,
    Tiff,
    Qoi,
    Hdr,
    Ico,
    Pnm,
    Avif,
    Heif,
    Svg,
    Mp4,
    Webm,
}

impl MediaKind {
    pub const ALL: [MediaKind; 15] = [
        MediaKind::Png,
        MediaKind::Jpeg,
        MediaKind::Gif,
        MediaKind::Bmp,
        MediaKind::WebP,
        MediaKind::Tiff,
        MediaKind::Qoi,
        MediaKind::Hdr,
        MediaKind::Ico,
        MediaKind::Pnm,
        MediaKind::Avif,
        MediaKind::Heif,
        MediaKind::Svg,
        MediaKind::Mp4,
        MediaKind::Webm,
    ];
}

pub fn sniff_media(bytes: &[u8]) -> Option<MediaKind> {
    if bytes.len() >= 8
        && bytes[..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
    {
        Some(MediaKind::Png)
    } else if bytes.len() >= 3 && bytes[..3] == [0xFF, 0xD8, 0xFF] {
        Some(MediaKind::Jpeg)
    } else if bytes.len() >= 6 && (&bytes[..6] == b"GIF87a" || &bytes[..6] == b"GIF89a") {
        Some(MediaKind::Gif)
    } else if bytes.len() >= 2 && bytes[..2] == *b"BM" {
        Some(MediaKind::Bmp)
    } else if bytes.len() >= 12
        && &bytes[..4] == b"RIFF"
        && &bytes[8..12] == b"WEBP"
    {
        Some(MediaKind::WebP)
    } else if bytes.len() >= 4
        && ((bytes[0] == b'I' && bytes[1] == b'I' && bytes[2] == 42 && bytes[3] == 0)
            || (bytes[0] == b'M' && bytes[1] == b'M' && bytes[2] == 0 && bytes[3] == 42))
    {
        Some(MediaKind::Tiff)
    } else if bytes.len() >= 4 && &bytes[..4] == b"qoif" {
        Some(MediaKind::Qoi)
    } else if bytes.len() >= 10
        && (&bytes[..10] == b"#?RADIANCE" || &bytes[..6] == b"#?RGBE")
    {
        Some(MediaKind::Hdr)
    } else if bytes.len() >= 4
        && bytes[0] == 0
        && bytes[1] == 0
        && (bytes[2] == 1 || bytes[2] == 2)
        && bytes[3] == 0
    {
        Some(MediaKind::Ico)
    } else if bytes.len() >= 3
        && bytes[0] == b'P'
        && (b'1'..=b'6').contains(&bytes[1])
        && matches!(bytes[2], b' ' | b'\t' | b'\n' | b'\r')
    {
        Some(MediaKind::Pnm)
    } else if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        let brand = &bytes[8..12];
        if brand == b"avif" || brand == b"avis" {
            Some(MediaKind::Avif)
        } else if brand == b"heic"
            || brand == b"heix"
            || brand == b"hevc"
            || brand == b"heis"
            || brand == b"heif"
            || brand == b"mif1"
            || brand == b"msf1"
        {
            Some(MediaKind::Heif)
        } else if brand == b"isom"
            || brand == b"iso2"
            || brand == b"iso3"
            || brand == b"iso4"
            || brand == b"iso5"
            || brand == b"mp41"
            || brand == b"mp42"
            || brand == b"m4v "
            || brand == b"M4V "
            || brand == b"m4a "
            || brand == b"m4b "
            || brand == b"m4p "
            || brand == b"avc1"
            || brand == b"qt  "
            || brand == b"dash"
        {
            Some(MediaKind::Mp4)
        } else {
            None
        }
    } else if bytes.len() >= 4 && bytes[..4] == [0x1A, 0x45, 0xDF, 0xA3] {
        Some(MediaKind::Webm)
    } else if sniff_is_svg(bytes) {
        Some(MediaKind::Svg)
    } else {
        None
    }
}

fn sniff_is_svg(bytes: &[u8]) -> bool {
    let mut i = 0;
    if bytes.len() >= 3 && bytes[..3] == [0xEF, 0xBB, 0xBF] {
        i = 3;
    }
    let end = bytes.len().min(512);
    loop {
        while i < end
            && (bytes[i] == b' ' || bytes[i] == b'\t' || bytes[i] == b'\n' || bytes[i] == b'\r')
        {
            i += 1;
        }
        if i >= end {
            return false;
        }
        let rest = &bytes[i..end];
        if rest.len() >= 4 && rest[..4] == *b"<!--" {
            match find_bytes(rest, b"-->") {
                Some(off) => {
                    i += off + 3;
                    continue;
                }
                None => return false,
            }
        }
        if rest.len() >= 9 && rest[..9].eq_ignore_ascii_case(b"<!doctype") {
            match rest.iter().position(|&b| b == b'>') {
                Some(off) => {
                    i += off + 1;
                    continue;
                }
                None => return false,
            }
        }
        if rest.len() >= 2 && rest[..2] == *b"<?" {
            match find_bytes(rest, b"?>") {
                Some(off) => {
                    i += off + 2;
                    continue;
                }
                None => return false,
            }
        }
        if rest.len() >= 4 && rest[..4].eq_ignore_ascii_case(b"<svg") {
            return true;
        }
        return false;
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|w| w == needle)
}

pub const MAX_TEXTURE_SIDE: u32 = 8192;

pub fn fit_inside(width: u32, height: u32, budget: u32) -> (u32, u32) {
    if width == 0 || height == 0 || budget == 0 {
        return (0, 0);
    }
    if width <= budget && height <= budget {
        return (width, height);
    }
    let b64 = budget as u64;
    if width >= height {
        let h = ((height as u64 * b64) / width.max(1) as u64).max(1).min(b64);
        (budget, h as u32)
    } else {
        let w = ((width as u64 * b64) / height.max(1) as u64).max(1).min(b64);
        (w as u32, budget)
    }
}

fn downscale_to_budget(
    rgba: image::RgbaImage,
    budget: u32,
) -> Option<(u32, u32, Vec<u8>)> {
    let (w, h) = (rgba.width(), rgba.height());
    let (tw, th) = fit_inside(w, h, budget);
    if (tw, th) == (w, h) {
        return None;
    }
    let out = image::imageops::resize(&rgba, tw, th, image::imageops::FilterType::Triangle);
    Some((tw, th, out.into_raw()))
}

pub fn decode_image_bytes_in_budget(
    bytes: &[u8],
    id: u32,
    cols: u16,
    rows: u16,
    budget: u32,
) -> Option<Picture> {
    let kind = sniff_media(bytes)?;
    let format = kind.image_format()?;
    let budget = budget.clamp(1, MAX_TEXTURE_SIDE);
    let decoded = image::load_from_memory_with_format(bytes, format).ok()?;
    let mut rgba = decoded.to_rgba8();
    let (mut width, mut height) = (rgba.width(), rgba.height());
    if width == 0 || height == 0 {
        return None;
    }
    if width > budget || height > budget {
        let (tw, th, raw) =
            downscale_to_budget(std::mem::replace(&mut rgba, image::RgbaImage::new(1, 1)), budget)?;
        width = tw;
        height = th;
        let picture = Picture { id, width, height, rgba: raw, cols, rows };
        return well_formed(&picture).then_some(picture);
    }
    let picture = Picture { id, width, height, rgba: rgba.into_raw(), cols, rows };
    well_formed(&picture).then_some(picture)
}

pub fn decode_image_bytes(bytes: &[u8], id: u32, cols: u16, rows: u16) -> Option<Picture> {
    decode_image_bytes_in_budget(bytes, id, cols, rows, MAX_TEXTURE_SIDE)
}

pub fn decode_image_bytes_for_adapter(
    bytes: &[u8],
    id: u32,
    cols: u16,
    rows: u16,
    adapter_name: Option<&str>,
) -> Option<Picture> {
    decode_image_bytes_in_budget(bytes, id, cols, rows, picture_side_budget(adapter_name))
}

pub fn shrink_picture_to_budget(picture: &mut Picture, budget: u32) -> bool {
    let budget = budget.clamp(1, MAX_TEXTURE_SIDE);
    if picture.width <= budget && picture.height <= budget {
        return false;
    }
    if picture.width == 0 || picture.height == 0 {
        return false;
    }
    let expect = (picture.width as usize)
        .checked_mul(picture.height as usize)
        .and_then(|n| n.checked_mul(4));
    if expect != Some(picture.rgba.len()) {
        return false;
    }
    let (tw, th) = fit_inside(picture.width, picture.height, budget);
    let Ok(img) = image::RgbaImage::from_raw(picture.width, picture.height, std::mem::take(&mut picture.rgba)).ok_or(()) else {
        return false;
    };
    let out = image::imageops::resize(&img, tw, th, image::imageops::FilterType::Triangle);
    picture.width = tw;
    picture.height = th;
    picture.rgba = out.into_raw();
    true
}

impl MediaKind {
    pub fn mime(self) -> &'static str {
        match self {
            MediaKind::Png => "image/png",
            MediaKind::Jpeg => "image/jpeg",
            MediaKind::Gif => "image/gif",
            MediaKind::Bmp => "image/bmp",
            MediaKind::WebP => "image/webp",
            MediaKind::Avif => "image/avif",
            MediaKind::Heif => "image/heif",
            MediaKind::Svg => "image/svg+xml",
            MediaKind::Mp4 => "video/mp4",
            MediaKind::Webm => "video/webm",
            MediaKind::Tiff => "image/tiff",
            MediaKind::Qoi => "image/qoi",
            MediaKind::Hdr => "image/vnd.radiance",
            MediaKind::Ico => "image/x-icon",
            MediaKind::Pnm => "image/x-portable-anymap",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            MediaKind::Png => "png",
            MediaKind::Jpeg => "jpg",
            MediaKind::Gif => "gif",
            MediaKind::Bmp => "bmp",
            MediaKind::WebP => "webp",
            MediaKind::Avif => "avif",
            MediaKind::Heif => "heic",
            MediaKind::Svg => "svg",
            MediaKind::Mp4 => "mp4",
            MediaKind::Webm => "webm",
            MediaKind::Tiff => "tiff",
            MediaKind::Qoi => "qoi",
            MediaKind::Hdr => "hdr",
            MediaKind::Ico => "ico",
            MediaKind::Pnm => "pnm",
        }
    }

    pub fn is_directly_decodable(self) -> bool {
        self.image_format().is_some()
    }

    pub fn image_format(self) -> Option<image::ImageFormat> {
        match self {
            MediaKind::Png => Some(image::ImageFormat::Png),
            MediaKind::Jpeg => Some(image::ImageFormat::Jpeg),
            MediaKind::Gif => Some(image::ImageFormat::Gif),
            MediaKind::Bmp => Some(image::ImageFormat::Bmp),
            MediaKind::WebP => Some(image::ImageFormat::WebP),
            MediaKind::Tiff => Some(image::ImageFormat::Tiff),
            MediaKind::Qoi => Some(image::ImageFormat::Qoi),
            MediaKind::Hdr => Some(image::ImageFormat::Hdr),
            MediaKind::Ico => Some(image::ImageFormat::Ico),
            MediaKind::Pnm => Some(image::ImageFormat::Pnm),
            MediaKind::Avif
            | MediaKind::Heif
            | MediaKind::Svg
            | MediaKind::Mp4
            | MediaKind::Webm => None,
        }
    }

    pub fn is_video(self) -> bool {
        matches!(self, MediaKind::Mp4 | MediaKind::Webm)
    }

    pub fn needs_rasterizer(self) -> bool {
        matches!(self, MediaKind::Svg)
    }

    pub fn is_codec_still(self) -> bool {
        matches!(self, MediaKind::Avif | MediaKind::Heif)
    }

    pub fn is_routed(self) -> bool {
        !self.is_directly_decodable()
    }

    pub fn is_linear_light(self) -> bool {
        matches!(self, MediaKind::Hdr)
    }

    pub fn from_extension(ext: &str) -> Option<MediaKind> {
        if ext.eq_ignore_ascii_case("png") {
            Some(MediaKind::Png)
        } else if ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg") {
            Some(MediaKind::Jpeg)
        } else if ext.eq_ignore_ascii_case("gif") {
            Some(MediaKind::Gif)
        } else if ext.eq_ignore_ascii_case("bmp") {
            Some(MediaKind::Bmp)
        } else if ext.eq_ignore_ascii_case("webp") {
            Some(MediaKind::WebP)
        } else if ext.eq_ignore_ascii_case("tiff") || ext.eq_ignore_ascii_case("tif") {
            Some(MediaKind::Tiff)
        } else if ext.eq_ignore_ascii_case("qoi") {
            Some(MediaKind::Qoi)
        } else if ext.eq_ignore_ascii_case("hdr") || ext.eq_ignore_ascii_case("rgbe") {
            Some(MediaKind::Hdr)
        } else if ext.eq_ignore_ascii_case("ico") || ext.eq_ignore_ascii_case("cur") {
            Some(MediaKind::Ico)
        } else if ext.eq_ignore_ascii_case("pnm")
            || ext.eq_ignore_ascii_case("pbm")
            || ext.eq_ignore_ascii_case("pgm")
            || ext.eq_ignore_ascii_case("ppm")
        {
            Some(MediaKind::Pnm)
        } else if ext.eq_ignore_ascii_case("avif") {
            Some(MediaKind::Avif)
        } else if ext.eq_ignore_ascii_case("heic")
            || ext.eq_ignore_ascii_case("heif")
            || ext.eq_ignore_ascii_case("heix")
        {
            Some(MediaKind::Heif)
        } else if ext.eq_ignore_ascii_case("svg") {
            Some(MediaKind::Svg)
        } else if ext.eq_ignore_ascii_case("mp4")
            || ext.eq_ignore_ascii_case("m4v")
            || ext.eq_ignore_ascii_case("m4a")
            || ext.eq_ignore_ascii_case("m4b")
            || ext.eq_ignore_ascii_case("m4p")
        {
            Some(MediaKind::Mp4)
        } else if ext.eq_ignore_ascii_case("webm") || ext.eq_ignore_ascii_case("mkv") {
            Some(MediaKind::Webm)
        } else {
            None
        }
    }

    pub fn from_path(path: &Path) -> Option<MediaKind> {
        let ext = path.extension()?.to_str()?;
        Self::from_extension(ext)
    }

    pub fn probe_with_hint(bytes: &[u8], hint: Option<&str>) -> Option<MediaKind> {
        if let Some(kind) = sniff_media(bytes) {
            return Some(kind);
        }
        hint.and_then(Self::from_extension)
    }

    pub fn probe_path(bytes: &[u8], path: Option<&Path>) -> Option<MediaKind> {
        if let Some(kind) = sniff_media(bytes) {
            return Some(kind);
        }
        path.and_then(Self::from_path)
    }
}

pub fn texture_byte_budget(budget: u32) -> u64 {
    let side = u64::from(budget.clamp(1, MAX_TEXTURE_SIDE));
    side * side * 4
}

pub fn probe_dimensions(bytes: &[u8]) -> Option<(MediaKind, u32, u32)> {
    let kind = sniff_media(bytes)?;
    if !kind.is_directly_decodable() {
        let (w, h) = routed_header_dimensions(kind, bytes)?;
        if w == 0 || h == 0 {
            return None;
        }
        return Some((kind, w, h));
    }
    if let Some((w, h)) = fast_header_dimensions(kind, bytes) {
        if w == 0 || h == 0 {
            return None;
        }
        return Some((kind, w, h));
    }
    let format = kind.image_format()?;
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes));
    reader.set_format(format);
    let (w, h) = reader.into_dimensions().ok()?;
    if w == 0 || h == 0 {
        return None;
    }
    Some((kind, w, h))
}

fn fast_header_dimensions(kind: MediaKind, bytes: &[u8]) -> Option<(u32, u32)> {
    match kind {
        MediaKind::Png => png_header_dimensions(bytes),
        MediaKind::Gif => gif_header_dimensions(bytes),
        MediaKind::Bmp => bmp_header_dimensions(bytes),
        MediaKind::Qoi => qoi_header_dimensions(bytes),
        MediaKind::Jpeg => jpeg_header_dimensions(bytes),
        MediaKind::WebP => webp_header_dimensions(bytes),
        MediaKind::Ico => ico_header_dimensions(bytes),
        MediaKind::Pnm => pnm_header_dimensions(bytes),
        MediaKind::Tiff => tiff_header_dimensions(bytes),
        MediaKind::Hdr => hdr_header_dimensions(bytes),
        _ => None,
    }
}

fn routed_header_dimensions(kind: MediaKind, bytes: &[u8]) -> Option<(u32, u32)> {
    match kind {
        MediaKind::Mp4 => mp4_header_dimensions(bytes),
        MediaKind::Webm => webm_header_dimensions(bytes),
        MediaKind::Avif | MediaKind::Heif => ispe_header_dimensions(bytes),
        MediaKind::Svg => svg_header_dimensions(bytes),
        _ => None,
    }
}

fn mp4_header_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let limit = bytes.len().min(2 * 1024 * 1024);
    if let Some(dims) = mp4_tkhd_dimensions(bytes, limit) {
        return Some(dims);
    }
    mp4_visual_entry_dimensions(bytes, limit)
}

fn mp4_tkhd_dimensions(bytes: &[u8], limit: usize) -> Option<(u32, u32)> {
    let mut i = 0usize;
    while i + 8 <= limit {
        if bytes[i + 4..i + 8] == *b"tkhd" {
            let size = u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]])
                as usize;
            let (payload, payload_len) = if size == 1 {
                if i + 16 > bytes.len() {
                    i += 1;
                    continue;
                }
                let large = u64::from_be_bytes([
                    bytes[i + 8],
                    bytes[i + 9],
                    bytes[i + 10],
                    bytes[i + 11],
                    bytes[i + 12],
                    bytes[i + 13],
                    bytes[i + 14],
                    bytes[i + 15],
                ]) as usize;
                if large < 16 || i + large > bytes.len() {
                    i += 1;
                    continue;
                }
                (i + 16, large - 16)
            } else if size == 0 {
                (i + 8, bytes.len().saturating_sub(i + 8))
            } else {
                if size < 8 || i + size > bytes.len() {
                    i += 1;
                    continue;
                }
                (i + 8, size - 8)
            };
            if payload_len >= 84 {
                let version = bytes[payload];
                if version == 0 {
                    let wfix = u32::from_be_bytes([
                        bytes[payload + 76],
                        bytes[payload + 77],
                        bytes[payload + 78],
                        bytes[payload + 79],
                    ]);
                    let hfix = u32::from_be_bytes([
                        bytes[payload + 80],
                        bytes[payload + 81],
                        bytes[payload + 82],
                        bytes[payload + 83],
                    ]);
                    let (w, h) = (wfix >> 16, hfix >> 16);
                    if w != 0 && h != 0 && w <= 16384 && h <= 16384 {
                        return Some((w, h));
                    }
                } else if version == 1 && payload_len >= 96 {
                    let wfix = u32::from_be_bytes([
                        bytes[payload + 88],
                        bytes[payload + 89],
                        bytes[payload + 90],
                        bytes[payload + 91],
                    ]);
                    let hfix = u32::from_be_bytes([
                        bytes[payload + 92],
                        bytes[payload + 93],
                        bytes[payload + 94],
                        bytes[payload + 95],
                    ]);
                    let (w, h) = (wfix >> 16, hfix >> 16);
                    if w != 0 && h != 0 && w <= 16384 && h <= 16384 {
                        return Some((w, h));
                    }
                }
            }
            if size != 0 && size != 1 && size >= 8 {
                i += size;
                continue;
            }
        }
        i += 1;
    }
    None
}

fn mp4_visual_entry_dimensions(bytes: &[u8], limit: usize) -> Option<(u32, u32)> {
    const VISUAL: [&[u8; 4]; 6] = [b"avc1", b"hev1", b"hvc1", b"vp09", b"av01", b"mp4v"];
    let mut i = 0usize;
    while i + 8 <= limit {
        let tag = &bytes[i + 4..i + 8];
        let mut is_visual = false;
        for v in VISUAL {
            if tag == v {
                is_visual = true;
                break;
            }
        }
        if is_visual {
            let size = u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]])
                as usize;
            if size != 0 && size != 1 {
                if size < 32 || i + size > bytes.len() {
                    i += 1;
                    continue;
                }
                let payload = i + 8;
                let w =
                    u16::from_be_bytes([bytes[payload + 24], bytes[payload + 25]]) as u32;
                let h =
                    u16::from_be_bytes([bytes[payload + 26], bytes[payload + 27]]) as u32;
                if w != 0 && h != 0 && w <= 16384 && h <= 16384 {
                    return Some((w, h));
                }
                i += size;
                continue;
            }
        }
        i += 1;
    }
    None
}

fn ispe_header_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let limit = bytes.len().min(1024 * 1024);
    let mut i = 0usize;
    while i + 20 <= limit {
        if bytes[i + 4..i + 8] == *b"ispe" {
            let size = u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]])
                as usize;
            if size == 1 {
                if i + 24 > bytes.len() {
                    i += 1;
                    continue;
                }
                let w = u32::from_be_bytes([
                    bytes[i + 16],
                    bytes[i + 17],
                    bytes[i + 18],
                    bytes[i + 19],
                ]);
                let h = u32::from_be_bytes([
                    bytes[i + 20],
                    bytes[i + 21],
                    bytes[i + 22],
                    bytes[i + 23],
                ]);
                if w != 0 && h != 0 && w <= 16384 && h <= 16384 {
                    return Some((w, h));
                }
            } else {
                if size < 20 || i + size > bytes.len() {
                    i += 1;
                    continue;
                }
                let w = u32::from_be_bytes([
                    bytes[i + 12],
                    bytes[i + 13],
                    bytes[i + 14],
                    bytes[i + 15],
                ]);
                let h = u32::from_be_bytes([
                    bytes[i + 16],
                    bytes[i + 17],
                    bytes[i + 18],
                    bytes[i + 19],
                ]);
                if w != 0 && h != 0 && w <= 16384 && h <= 16384 {
                    return Some((w, h));
                }
            }
        }
        i += 1;
    }
    None
}

fn webm_header_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let limit = bytes.len().min(1024 * 1024);
    let mut w: Option<u32> = None;
    let mut h: Option<u32> = None;
    let mut i = 0usize;
    while i + 3 <= limit {
        let id = bytes[i];
        if id == 0xB0 || id == 0xBA {
            if let Some((value, _)) = ebml_uint_value(bytes, i) {
                if value != 0 && value <= 16384 {
                    if id == 0xB0 && w.is_none() {
                        w = Some(value);
                    } else if id == 0xBA && h.is_none() {
                        h = Some(value);
                    }
                    if w.is_some() && h.is_some() {
                        return Some((w.unwrap(), h.unwrap()));
                    }
                }
            }
        }
        i += 1;
    }
    None
}

fn ebml_uint_value(bytes: &[u8], pos: usize) -> Option<(u32, usize)> {
    if pos + 2 > bytes.len() {
        return None;
    }
    let first = bytes[pos + 1];
    let (len, mask): (usize, u8) = if first & 0x80 != 0 {
        (1, 0x7F)
    } else if first & 0x40 != 0 {
        (2, 0x3F)
    } else if first & 0x20 != 0 {
        (3, 0x1F)
    } else if first & 0x10 != 0 {
        (4, 0x0F)
    } else if first & 0x08 != 0 {
        (5, 0x07)
    } else if first & 0x04 != 0 {
        (6, 0x03)
    } else if first & 0x02 != 0 {
        (7, 0x01)
    } else if first & 0x01 != 0 {
        (8, 0x00)
    } else {
        return None;
    };
    if len > 4 {
        return None;
    }
    if pos + 1 + len > bytes.len() {
        return None;
    }
    let mut data_len = u64::from(first & mask);
    for k in 1..len {
        data_len = (data_len << 8) | u64::from(bytes[pos + 1 + k]);
    }
    if data_len == 0 || data_len > 4 {
        return None;
    }
    let dlen = data_len as usize;
    if pos + 1 + len + dlen > bytes.len() {
        return None;
    }
    let mut value: u32 = 0;
    for k in 0..dlen {
        value = (value << 8) | u32::from(bytes[pos + 1 + len + k]);
    }
    Some((value, 1 + len + dlen))
}

fn svg_header_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let limit = bytes.len().min(4096);
    let window = &bytes[..limit];
    if let (Some(w), Some(h)) = (
        svg_attr_length(window, b"width"),
        svg_attr_length(window, b"height"),
    ) {
        if w != 0 && h != 0 && w <= 16384 && h <= 16384 {
            return Some((w, h));
        }
    }
    svg_viewbox_dimensions(window)
}

fn svg_attr_length(window: &[u8], name: &[u8]) -> Option<u32> {
    let mut i = 0usize;
    while i + name.len() <= window.len() {
        if window[i..i + name.len()].eq_ignore_ascii_case(name) {
            let mut j = i + name.len();
            while j < window.len() && matches!(window[j], b' ' | b'\t' | b'\n' | b'\r') {
                j += 1;
            }
            if j < window.len() && window[j] == b'=' {
                j += 1;
                while j < window.len()
                    && matches!(window[j], b' ' | b'\t' | b'\n' | b'\r')
                {
                    j += 1;
                }
                if j < window.len() && (window[j] == b'"' || window[j] == b'\'') {
                    let quote = window[j];
                    j += 1;
                    let start = j;
                    while j < window.len() && window[j] != quote {
                        j += 1;
                    }
                    if j < window.len() {
                        let raw = &window[start..j];
                        if raw.contains(&b'%') {
                            return None;
                        }
                        if let Some(v) = svg_length_value(raw) {
                            return Some(v);
                        }
                    }
                }
            }
        }
        i += 1;
    }
    None
}

fn svg_length_value(raw: &[u8]) -> Option<u32> {
    let mut end = 0usize;
    while end < raw.len()
        && (raw[end].is_ascii_digit()
            || matches!(raw[end], b'+' | b'-' | b'.' | b'e' | b'E'))
    {
        end += 1;
    }
    if end == 0 {
        return None;
    }
    let s = std::str::from_utf8(&raw[..end]).ok()?;
    let v: f32 = s.parse().ok()?;
    if !v.is_finite() || v <= 0.0 || v > 16384.0 {
        return None;
    }
    let u = v as u32;
    if u == 0 || u > 16384 {
        return None;
    }
    Some(u)
}

fn svg_viewbox_dimensions(window: &[u8]) -> Option<(u32, u32)> {
    let name = b"viewBox";
    let mut i = 0usize;
    while i + name.len() <= window.len() {
        if window[i..i + name.len()] == *name {
            let mut j = i + name.len();
            while j < window.len() && matches!(window[j], b' ' | b'\t' | b'\n' | b'\r') {
                j += 1;
            }
            if j < window.len() && window[j] == b'=' {
                j += 1;
                while j < window.len()
                    && matches!(window[j], b' ' | b'\t' | b'\n' | b'\r')
                {
                    j += 1;
                }
                if j < window.len() && (window[j] == b'"' || window[j] == b'\'') {
                    let quote = window[j];
                    j += 1;
                    let start = j;
                    while j < window.len() && window[j] != quote {
                        j += 1;
                    }
                    if j < window.len() {
                        let raw = &window[start..j];
                        if let Some((w, h)) = svg_viewbox_value(raw) {
                            return Some((w, h));
                        }
                    }
                }
            }
        }
        i += 1;
    }
    None
}

fn svg_viewbox_value(raw: &[u8]) -> Option<(u32, u32)> {
    let s = std::str::from_utf8(raw).ok()?;
    let parts: Vec<&str> = s
        .split([' ', '\t', '\n', '\r', ','])
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() != 4 {
        return None;
    }
    let w: f32 = parts[2].parse().ok()?;
    let h: f32 = parts[3].parse().ok()?;
    if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 || w > 16384.0 || h > 16384.0
    {
        return None;
    }
    let (wi, hi) = (w as u32, h as u32);
    if wi == 0 || hi == 0 {
        return None;
    }
    Some((wi, hi))
}

fn png_header_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    if w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

fn gif_header_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 10 {
        return None;
    }
    let w = u16::from_le_bytes([bytes[6], bytes[7]]) as u32;
    let h = u16::from_le_bytes([bytes[8], bytes[9]]) as u32;
    if w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

fn bmp_header_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 26 || &bytes[0..2] != b"BM" {
        return None;
    }
    let dib = u32::from_le_bytes([bytes[14], bytes[15], bytes[16], bytes[17]]);
    if dib != 40 || bytes.len() < 26 {
        return None;
    }
    let w = i32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]);
    let h = i32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]);
    if w <= 0 || h == 0 {
        return None;
    }
    Some((w as u32, h.unsigned_abs()))
}

fn qoi_header_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 14 {
        return None;
    }
    let w = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let h = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    if w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

fn jpeg_header_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }
    let mut i = 2;
    while i + 3 < bytes.len() {
        if bytes[i] != 0xFF {
            return None;
        }
        while i < bytes.len() && bytes[i] == 0xFF {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }
        let marker = bytes[i];
        i += 1;
        if marker == 0xD8 || marker == 0xD9 || (0xD0..=0xD7).contains(&marker) || marker == 0x01
        {
            continue;
        }
        if i + 1 >= bytes.len() {
            return None;
        }
        let seg_len = u16::from_be_bytes([bytes[i], bytes[i + 1]]) as usize;
        if seg_len < 2 || i + seg_len > bytes.len() + 2 {
            return None;
        }
        let is_sof = matches!(
            marker,
            0xC0 | 0xC1 | 0xC2 | 0xC3 | 0xC5 | 0xC6 | 0xC7 | 0xC9 | 0xCA | 0xCB | 0xCD
                | 0xCE | 0xCF
        );
        if is_sof {
            if seg_len >= 7 && i + 7 <= bytes.len() {
                let h = u16::from_be_bytes([bytes[i + 3], bytes[i + 4]]) as u32;
                let w = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
                if w == 0 || h == 0 {
                    return None;
                }
                return Some((w, h));
            }
            return None;
        }
        i += seg_len;
    }
    None
}

fn webp_header_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 21 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return None;
    }
    match &bytes[12..16] {
        b"VP8 " => {
            if bytes.len() < 30 {
                return None;
            }
            if bytes[23] != 0x9D || bytes[24] != 0x01 || bytes[25] != 0x2A {
                return None;
            }
            let w = (u16::from_le_bytes([bytes[26], bytes[27]]) & 0x3FFF) as u32;
            let h = (u16::from_le_bytes([bytes[28], bytes[29]]) & 0x3FFF) as u32;
            if w == 0 || h == 0 || w > 16384 || h > 16384 {
                return None;
            }
            Some((w, h))
        }
        b"VP8L" => {
            if bytes.len() < 25 || bytes[20] != 0x2F {
                return None;
            }
            let bits =
                u32::from_le_bytes([bytes[21], bytes[22], bytes[23], bytes[24]]);
            let w = (bits & 0x3FFF) + 1;
            let h = ((bits >> 14) & 0x3FFF) + 1;
            if w == 0 || h == 0 || w > 16384 || h > 16384 {
                return None;
            }
            Some((w, h))
        }
        b"VP8X" => {
            if bytes.len() < 30 {
                return None;
            }
            let chunk = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
            if chunk < 10 {
                return None;
            }
            let w = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], 0]) + 1;
            let h = u32::from_le_bytes([bytes[27], bytes[28], bytes[29], 0]) + 1;
            if w > 16384 || h > 16384 {
                return None;
            }
            Some((w, h))
        }
        _ => None,
    }
}

fn ico_header_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 8 || bytes[0] != 0 || bytes[1] != 0 {
        return None;
    }
    let kind = u16::from_le_bytes([bytes[2], bytes[3]]);
    if kind != 1 && kind != 2 {
        return None;
    }
    if u16::from_le_bytes([bytes[4], bytes[5]]) == 0 {
        return None;
    }
    let w = if bytes[6] == 0 { 256 } else { u32::from(bytes[6]) };
    let h = if bytes[7] == 0 { 256 } else { u32::from(bytes[7]) };
    Some((w, h))
}

fn pnm_header_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 3 || bytes[0] != b'P' || !(b'1'..=b'6').contains(&bytes[1]) {
        return None;
    }
    if !matches!(bytes[2], b' ' | b'\t' | b'\n' | b'\r') {
        return None;
    }
    let limit = bytes.len().min(1024);
    let mut i = 2usize;
    let mut vals = [0u32; 2];
    let mut n = 0usize;
    while i < limit && n < 2 {
        while i < limit && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r') {
            i += 1;
        }
        if i >= limit {
            break;
        }
        if bytes[i] == b'#' {
            while i < limit && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if !bytes[i].is_ascii_digit() {
            return None;
        }
        let mut v: u32 = 0;
        let mut digits = 0usize;
        while i < limit && bytes[i].is_ascii_digit() {
            v = v.checked_mul(10)?.checked_add(u32::from(bytes[i] - b'0'))?;
            digits += 1;
            if digits > 5 || v > 65535 {
                return None;
            }
            i += 1;
        }
        if v == 0 {
            return None;
        }
        vals[n] = v;
        n += 1;
    }
    if n != 2 {
        return None;
    }
    Some((vals[0], vals[1]))
}

fn tiff_header_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 8 {
        return None;
    }
    let le = if bytes[0..4] == *b"II*\0" {
        true
    } else if bytes[0..4] == *b"MM\0*" {
        false
    } else {
        return None;
    };
    let get_u16 = |b: [u8; 2]| {
        if le {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        }
    };
    let get_u32 = |b: [u8; 4]| {
        if le {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        }
    };
    let ifd = get_u32([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    if ifd.checked_add(2)? > bytes.len() || ifd > 1024 * 1024 {
        return None;
    }
    let count = get_u16([bytes[ifd], bytes[ifd + 1]]) as usize;
    if count == 0 || count > 256 {
        return None;
    }
    if ifd.checked_add(2 + count.checked_mul(12)?)? > bytes.len() {
        return None;
    }
    let mut w: Option<u32> = None;
    let mut h: Option<u32> = None;
    for e in 0..count {
        let base = ifd + 2 + e * 12;
        let tag = get_u16([bytes[base], bytes[base + 1]]);
        if tag != 256 && tag != 257 {
            continue;
        }
        let typ = get_u16([bytes[base + 2], bytes[base + 3]]);
        if get_u32([bytes[base + 4], bytes[base + 5], bytes[base + 6], bytes[base + 7]]) != 1 {
            continue;
        }
        let val = match typ {
            3 => u32::from(get_u16([bytes[base + 8], bytes[base + 9]])),
            4 => get_u32([
                bytes[base + 8],
                bytes[base + 9],
                bytes[base + 10],
                bytes[base + 11],
            ]),
            _ => continue,
        };
        if val == 0 {
            return None;
        }
        if tag == 256 {
            w = Some(val);
        } else {
            h = Some(val);
        }
        if w.is_some() && h.is_some() {
            break;
        }
    }
    Some((w?, h?))
}

fn hdr_header_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if !(bytes.starts_with(b"#?RADIANCE") || bytes.starts_with(b"#?RGBE")) {
        return None;
    }
    let limit = bytes.len().min(4096);
    let mut start = 0usize;
    while start < limit {
        let mut end = start;
        while end < limit && bytes[end] != b'\n' {
            end += 1;
        }
        if let Some(dims) = parse_hdr_resolution(&bytes[start..end]) {
            return Some(dims);
        }
        start = end + 1;
    }
    None
}

fn parse_hdr_resolution(line: &[u8]) -> Option<(u32, u32)> {
    let mut bounds: [[usize; 2]; 4] = [[0, 0]; 4];
    let mut tokens = 0usize;
    let mut i = 0usize;
    while i < line.len() {
        while i < line.len() && (line[i] == b' ' || line[i] == b'\t' || line[i] == b'\r') {
            i += 1;
        }
        if i >= line.len() {
            break;
        }
        if tokens >= 4 {
            return None;
        }
        let s = i;
        while i < line.len() && line[i] != b' ' && line[i] != b'\t' && line[i] != b'\r' {
            i += 1;
        }
        bounds[tokens] = [s, i];
        tokens += 1;
    }
    if tokens != 4 {
        return None;
    }
    let tok = |k: usize| &line[bounds[k][0]..bounds[k][1]];
    let axis = |t: &[u8]| -> Option<u8> {
        match t {
            [s, b'X'] | [s, b'Y'] if *s == b'-' || *s == b'+' => Some(t[1]),
            _ => None,
        }
    };
    let num = |t: &[u8]| -> Option<u32> {
        if t.is_empty() || t.len() > 5 || !t.iter().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let mut v: u32 = 0;
        for b in t {
            v = v.checked_mul(10)?.checked_add(u32::from(b - b'0'))?;
        }
        if v == 0 || v > 65535 {
            return None;
        }
        Some(v)
    };
    let (a0, n0, a1, n1) = (axis(tok(0))?, num(tok(1))?, axis(tok(2))?, num(tok(3))?);
    match (a0, a1) {
        (b'Y', b'X') => Some((n1, n0)),
        (b'X', b'Y') => Some((n0, n1)),
        _ => None,
    }
}

pub fn probe_fitted_dimensions(bytes: &[u8], budget: u32) -> Option<(MediaKind, u32, u32)> {
    let (kind, w, h) = probe_dimensions(bytes)?;
    let (tw, th) = fit_inside(w, h, budget.clamp(1, MAX_TEXTURE_SIDE));
    if tw == 0 || th == 0 {
        return None;
    }
    Some((kind, tw, th))
}

pub fn routed_reason(kind: MediaKind) -> Option<&'static str> {
    if kind.is_directly_decodable() {
        return None;
    }
    if kind.is_video() {
        Some("video-pipeline")
    } else if kind.needs_rasterizer() {
        Some("needs-rasterizer")
    } else if kind.is_codec_still() {
        Some("codec-pipeline")
    } else {
        Some("routed")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServePlan {
    pub kind: MediaKind,
    pub mime: &'static str,
    pub extension: &'static str,
    pub native: Option<(u32, u32)>,
    pub fitted: Option<(u32, u32)>,
    pub budget: u32,
    pub byte_cap: u64,
    pub routed_reason: Option<&'static str>,
}

pub fn serve_plan(bytes: &[u8], path: Option<&Path>, budget: u32) -> Option<ServePlan> {
    let kind = MediaKind::probe_path(bytes, path)?;
    let budget = budget.clamp(1, MAX_TEXTURE_SIDE);
    let byte_cap = texture_byte_budget(budget);
    let base = ServePlan {
        kind,
        mime: kind.mime(),
        extension: kind.extension(),
        native: None,
        fitted: None,
        budget,
        byte_cap,
        routed_reason: routed_reason(kind),
    };
    match probe_dimensions(bytes) {
        Some((_, w, h)) => {
            let (tw, th) = fit_inside(w, h, budget);
            if tw == 0 || th == 0 {
                Some(base)
            } else {
                Some(ServePlan { native: Some((w, h)), fitted: Some((tw, th)), ..base })
            }
        }
        None => Some(base),
    }
}

pub fn serve_budget_for_adapter(adapter_name: Option<&str>) -> (u32, u64) {
    let side = picture_side_budget(adapter_name);
    (side, texture_byte_budget(side))
}

pub fn serve_plan_for_adapter(
    bytes: &[u8],
    path: Option<&Path>,
    adapter_name: Option<&str>,
) -> Option<ServePlan> {
    serve_plan(bytes, path, picture_side_budget(adapter_name))
}

impl ServePlan {
    pub fn content_type(&self) -> &'static str {
        self.mime
    }
    pub fn layout_box(&self) -> Option<(u32, u32)> {
        self.fitted
    }
    pub fn is_held(&self) -> bool {
        self.routed_reason.is_some()
    }
    pub fn is_linear_light(&self) -> bool {
        self.kind.is_linear_light()
    }
    pub fn http_headers(&self, body_len: usize) -> Vec<(String, String)> {
        let mut out = Vec::with_capacity(7);
        out.push(("Content-Type".to_string(), self.mime.to_string()));
        out.push(("Content-Length".to_string(), body_len.to_string()));
        if let Some((w, h)) = self.native {
            out.push(("X-Glass-Native".to_string(), format!("{w}x{h}")));
        }
        if let Some((w, h)) = self.fitted {
            out.push(("X-Glass-Fitted".to_string(), format!("{w}x{h}")));
        }
        out.push(("X-Glass-Budget".to_string(), self.budget.to_string()));
        out.push(("X-Glass-Byte-Cap".to_string(), self.byte_cap.to_string()));
        if let Some(reason) = self.routed_reason {
            out.push(("X-Glass-Hold".to_string(), reason.to_string()));
        }
        if self.is_linear_light() {
            out.push(("X-Glass-Linear".to_string(), "scene-linear-ibl".to_string()));
        }
        out
    }
    pub fn http_header_lines(&self, body_len: usize) -> Vec<String> {
        self.http_headers(body_len)
            .into_iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect()
    }
}

pub fn kitty_base64_frame_cost(raw_len: usize) -> usize {
    (raw_len.saturating_add(2) / 3).saturating_mul(4)
}

pub fn serve_vs_kitty_bytes(raw_len: usize, frames: u32) -> (usize, usize) {
    let per = kitty_base64_frame_cost(raw_len);
    let kitty = per.saturating_mul(frames.max(1) as usize);
    (kitty, raw_len)
}

pub fn nvidia_unreal_hint(adapter_name: Option<&str>) -> &'static str {
    match adapter_name {
        Some(name) if is_rtx_adapter(name) => {
            "nvidia-rtx: serve via wgpu/Vulkan driver (VK_NV_ray_tracing, Vulkan Video, Nsight); no nvcc/CUDA context; half-float IBL + video-texture pipeline ready"
        }
        Some(name) if is_nvidia_adapter(name) => {
            "nvidia: serve via wgpu/Vulkan driver (Vulkan Video, Nsight); no nvcc/CUDA context"
        }
        Some(_) => "generic wgpu adapter: portable path; RTX/Vulkan extensions unavailable",
        None => "headless/unknown adapter: 4096 budget path; attach NVIDIA for Unreal-grade 8K",
    }
}

pub fn decode_hdr_linear_in_budget(bytes: &[u8], budget: u32) -> Option<(u32, u32, Vec<f32>)> {
    let kind = sniff_media(bytes)?;
    if !kind.is_linear_light() {
        return None;
    }
    let format = kind.image_format()?;
    let decoded = image::load_from_memory_with_format(bytes, format).ok()?;
    let rgb = decoded.to_rgb32f();
    let (mut width, mut height) = (rgb.width(), rgb.height());
    if width == 0 || height == 0 {
        return None;
    }
    let budget = budget.clamp(1, MAX_TEXTURE_SIDE);
    if width <= budget && height <= budget {
        return Some((width, height, rgb.into_raw()));
    }
    let (tw, th) = fit_inside(width, height, budget);
    if tw == 0 || th == 0 {
        return None;
    }
    let resized = image::imageops::resize(&rgb, tw, th, image::imageops::FilterType::Triangle);
    width = resized.width();
    height = resized.height();
    Some((width, height, resized.into_raw()))
}

pub fn f32_to_f16_bits(v: f32) -> u16 {
    let bits = v.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xFF) as i32;
    let mant = bits & 0x7F_FFFF;
    if exp == 0xFF {
        return sign | 0x7C00 | if mant == 0 { 0 } else { ((mant >> 13) as u16).max(1) };
    }
    let e = exp - 127 + 15;
    if e >= 31 {
        return sign | 0x7C00;
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        let with_hidden = mant | 0x80_0000;
        let shift = (14 - e) as u32;
        let dropped_mask = (1u32 << shift) - 1;
        let dropped = with_hidden & dropped_mask;
        let half_unit = 1u32 << (shift - 1);
        let mut half = (with_hidden >> shift) as u16;
        if dropped > half_unit || (dropped == half_unit && (half & 1) == 1) {
            half += 1;
        }
        return sign | (half & 0x3FF);
    }
    let mut half_exp = (e as u16) << 10;
    let mut half_mant = (mant >> 13) as u16;
    let dropped = mant & 0x1FFF;
    if dropped > 0x1000 || (dropped == 0x1000 && (half_mant & 1) == 1) {
        half_mant += 1;
        if half_mant == 0x400 {
            half_mant = 0;
            half_exp += 1 << 10;
            if half_exp >= 0x7C00 {
                return sign | 0x7C00;
            }
        }
    }
    sign | half_exp | (half_mant & 0x3FF)
}

pub fn rgb32f_to_rgba_f16le(pixels: &[f32]) -> Vec<u8> {
    const ONE: u16 = 0x3C00;
    let n = pixels.len() / 3;
    let mut out = Vec::with_capacity(n * 8);
    for i in 0..n {
        for c in 0..3 {
            out.extend_from_slice(&f32_to_f16_bits(pixels[i * 3 + c]).to_le_bytes());
        }
        out.extend_from_slice(&ONE.to_le_bytes());
    }
    out
}

pub fn hdr_f16_byte_len(width: u32, height: u32) -> u64 {
    u64::from(width).saturating_mul(u64::from(height)).saturating_mul(8)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterReport {
    pub adapter: String,
    pub is_nvidia: bool,
    pub is_rtx: bool,
    pub side_budget: u32,
    pub byte_cap: u64,
}

impl AdapterReport {
    pub fn describe(&self) -> String {
        format!(
            "{} (nvidia={} rtx={}) budget={} cap={}B",
            self.adapter, self.is_nvidia, self.is_rtx, self.side_budget, self.byte_cap
        )
    }
}

pub fn adapter_report(adapter_name: Option<&str>) -> AdapterReport {
    let name = adapter_name.unwrap_or("unknown").to_string();
    let is_nvidia = adapter_name.is_some_and(is_nvidia_adapter);
    let is_rtx = adapter_name.is_some_and(is_rtx_adapter);
    let side_budget = picture_side_budget(adapter_name);
    let byte_cap = texture_byte_budget(side_budget);
    AdapterReport { adapter: name, is_nvidia, is_rtx, side_budget, byte_cap }
}

fn contains_insensitive(haystack: &str, needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|w| w.eq_ignore_ascii_case(needle))
}

pub fn is_nvidia_adapter(adapter_name: &str) -> bool {
    contains_insensitive(adapter_name, b"nvidia")
        || contains_insensitive(adapter_name, b"geforce")
        || contains_insensitive(adapter_name, b"quadro")
        || contains_insensitive(adapter_name, b"tesla")
}

pub fn is_rtx_adapter(adapter_name: &str) -> bool {
    if !is_nvidia_adapter(adapter_name) {
        return false;
    }
    adapter_name
        .as_bytes()
        .windows(3)
        .any(|w| w.eq_ignore_ascii_case(b"rtx"))
}

pub fn picture_side_budget(adapter_name: Option<&str>) -> u32 {
    match adapter_name {
        Some(name) if is_rtx_adapter(name) => 8192,
        Some(name) if is_nvidia_adapter(name) => 8192,
        Some(name) => {
            if contains_insensitive(name, b"apple")
                || contains_insensitive(name, b"m1")
                || contains_insensitive(name, b"m2")
                || contains_insensitive(name, b"m3")
                || contains_insensitive(name, b"radeon")
                || contains_insensitive(name, b"rdna")
            {
                8192
            } else {
                4096
            }
        }
        None => 4096,
    }
}

pub fn box_cells(image: (u32, u32), cols: u16, rows: u16, cell: (u32, u32)) -> (u16, u16) {
    let fit =
        |pixels: u32, size: u32| pixels.div_ceil(size.max(1)).clamp(1, u32::from(u16::MAX)) as u16;
    (
        if cols > 0 { cols } else { fit(image.0, cell.0) },
        if rows > 0 { rows } else { fit(image.1, cell.1) },
    )
}

pub fn cell_slice(
    image: (u32, u32),
    box_cells: (u16, u16),
    cell: (u32, u32),
    row: u16,
    col: u16,
) -> Option<([f32; 4], [f32; 4])> {
    cell_span_slice(image, box_cells, cell, row, col, 1)
}

pub fn cell_span_slice(
    image: (u32, u32),
    box_cells: (u16, u16),
    cell: (u32, u32),
    row: u16,
    col: u16,
    cols_span: u16,
) -> Option<([f32; 4], [f32; 4])> {
    let (iw, ih) = (image.0 as f32, image.1 as f32);
    let (cw, ch) = (cell.0 as f32, cell.1 as f32);
    let (bw, bh) = (f32::from(box_cells.0) * cw, f32::from(box_cells.1) * ch);
    if iw <= 0.0 || ih <= 0.0 || bw <= 0.0 || bh <= 0.0 || cols_span == 0 {
        return None;
    }
    let own_size = box_cells == self::box_cells(image, 0, 0, cell);
    let fit = (bw / iw).min(bh / ih);
    let scale = if own_size { fit.min(1.0) } else { fit };
    let (dw, dh) = ((iw * scale).round().max(1.0), (ih * scale).round().max(1.0));
    let (ox, oy) = (((bw - dw) / 2.0).round(), ((bh - dh) / 2.0).round());
    let (cx, cy) = (f32::from(col) * cw, f32::from(row) * ch);
    let span_w = f32::from(cols_span) * cw;
    let (x0, x1) = (cx.max(ox), (cx + span_w).min(ox + dw));
    let (y0, y1) = (cy.max(oy), (cy + ch).min(oy + dh));
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let (sx, sy) = (iw / dw, ih / dh);
    Some((
        [x0 - cx, y0 - cy, x1 - x0, y1 - y0],
        [
            (x0 - ox) * sx,
            (y0 - oy) * sy,
            (x1 - x0) * sx,
            (y1 - y0) * sy,
        ],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(row: u16, col: u16, high: Option<u16>) -> String {
        let mark = |n: u16| marks::mark(usize::from(n));
        let mut symbol = String::from(PLACEHOLDER);
        symbol.push(mark(row));
        symbol.push(mark(col));
        if let Some(high) = high {
            symbol.push(mark(high));
        }
        symbol
    }

    #[test]
    fn a_picture_cell_names_its_picture_row_and_column() {
        let fg = Color::Rgb(0x12, 0x34, 0x56);
        assert_eq!(
            read_placeholder(&cell(3, 7, None), fg),
            Some(Placeholder { image: 0x0012_3456, row: 3, col: 7 })
        );
        assert_eq!(
            read_placeholder(&cell(0, 1, Some(2)), fg),
            Some(Placeholder { image: 0x0212_3456, row: 0, col: 1 })
        );
    }

    #[test]
    fn anything_else_is_not_a_picture_cell() {
        let fg = Color::Rgb(1, 2, 3);
        assert_eq!(read_placeholder("a", fg), None);
        assert_eq!(read_placeholder(&String::from(PLACEHOLDER), fg), None, "no marks");
        assert_eq!(read_placeholder(&format!("{}x", cell(1, 1, None)), fg), None, "trailing text");
        assert_eq!(read_placeholder(&format!("{PLACEHOLDER}ab"), fg), None, "not marks");
    }

    #[test]
    fn box_comes_from_the_picture_when_not_given() {
        assert_eq!(box_cells((100, 50), 0, 0, (12, 25)), (9, 2));
        assert_eq!(box_cells((100, 50), 40, 12, (12, 25)), (40, 12));
    }

    #[test]
    fn own_size_box_shows_pixel_for_pixel() {
        let slice = cell_slice((24, 50), (2, 2), (12, 25), 1, 1).unwrap();
        assert_eq!(slice, ([0.0, 0.0, 12.0, 25.0], [12.0, 25.0, 12.0, 25.0]));
    }

    #[test]
    fn given_box_scales_and_letterboxes() {
        let (dst, src) = cell_slice((200, 100), (4, 4), (10, 10), 1, 0).unwrap();
        assert_eq!(dst, [0.0, 0.0, 10.0, 10.0]);
        assert_eq!(src, [0.0, 0.0, 50.0, 50.0]);
        assert_eq!(cell_slice((200, 100), (4, 4), (10, 10), 0, 0), None);
    }

    #[test]
    fn routed_kinds_sniff_but_do_not_direct_decode() {
        let mut avif = vec![0u8; 12];
        avif[4..8].copy_from_slice(b"ftyp");
        avif[8..12].copy_from_slice(b"avif");
        assert_eq!(sniff_media(&avif), Some(MediaKind::Avif));
        assert_eq!(MediaKind::Avif.mime(), "image/avif");
        assert_eq!(MediaKind::Avif.extension(), "avif");
        assert_eq!(MediaKind::from_extension("avif"), Some(MediaKind::Avif));
        assert!(!MediaKind::Avif.is_directly_decodable());
        assert!(decode_image_bytes_in_budget(&avif, 1, 0, 0, 8192).is_none());
        let mut heic = vec![0u8; 12];
        heic[4..8].copy_from_slice(b"ftyp");
        heic[8..12].copy_from_slice(b"heic");
        assert_eq!(sniff_media(&heic), Some(MediaKind::Heif));
        assert_eq!(MediaKind::Heif.mime(), "image/heif");
        let mut mp4 = vec![0u8; 12];
        mp4[4..8].copy_from_slice(b"ftyp");
        mp4[8..12].copy_from_slice(b"isom");
        assert_eq!(sniff_media(&mp4), Some(MediaKind::Mp4));
        assert!(MediaKind::Mp4.is_video());
        assert!(decode_image_bytes_in_budget(&mp4, 1, 0, 0, 8192).is_none());
        assert_eq!(sniff_media(&[0x1A, 0x45, 0xDF, 0xA3, 0x00]), Some(MediaKind::Webm));
        assert!(decode_image_bytes_in_budget(&[0x1A, 0x45, 0xDF, 0xA3, 0x00], 1, 0, 0, 8192).is_none());
        assert_eq!(sniff_media(b"  <svg xmlns=\"x\">"), Some(MediaKind::Svg));
        assert!(MediaKind::Svg.needs_rasterizer());
        assert!(decode_image_bytes_in_budget(b"  <svg xmlns=\"x\">", 1, 0, 0, 8192).is_none());
        assert_eq!(MediaKind::ALL.len(), 15);
        for kind in MediaKind::ALL {
            assert!(!kind.mime().is_empty());
            assert!(!kind.extension().is_empty());
            assert_eq!(MediaKind::from_extension(kind.extension()), Some(kind));
        }
    }

    #[test]
    fn fit_inside_keeps_aspect_with_integer_maths() {
        assert_eq!(fit_inside(0, 10, 100), (0, 0));
        assert_eq!(fit_inside(100, 50, 8192), (100, 50), "never upscales");
        assert_eq!(fit_inside(16384, 8192, 8192), (8192, 4096));
        assert_eq!(fit_inside(8192, 16384, 8192), (4096, 8192));
        assert_eq!(fit_inside(16384, 16384, 4096), (4096, 4096));
        assert_eq!(fit_inside(8000, 6000, 4096), (4096, 3072));
        assert_eq!(fit_inside(6000, 8000, 4096), (3072, 4096));
    }

    #[test]
    fn sniffed_format_drives_direct_decode_without_a_second_guess() {
        assert_eq!(MediaKind::Png.image_format(), Some(image::ImageFormat::Png));
        assert_eq!(MediaKind::Jpeg.image_format(), Some(image::ImageFormat::Jpeg));
        assert_eq!(MediaKind::Gif.image_format(), Some(image::ImageFormat::Gif));
        assert_eq!(MediaKind::Bmp.image_format(), Some(image::ImageFormat::Bmp));
        assert_eq!(MediaKind::WebP.image_format(), Some(image::ImageFormat::WebP));
        assert_eq!(MediaKind::Tiff.image_format(), Some(image::ImageFormat::Tiff));
        assert_eq!(MediaKind::Qoi.image_format(), Some(image::ImageFormat::Qoi));
        assert_eq!(MediaKind::Hdr.image_format(), Some(image::ImageFormat::Hdr));
        assert_eq!(MediaKind::Ico.image_format(), Some(image::ImageFormat::Ico));
        assert_eq!(MediaKind::Pnm.image_format(), Some(image::ImageFormat::Pnm));
        for routed in [
            MediaKind::Avif,
            MediaKind::Heif,
            MediaKind::Svg,
            MediaKind::Mp4,
            MediaKind::Webm,
        ] {
            assert_eq!(routed.image_format(), None);
            assert!(!routed.is_directly_decodable());
        }
        for brand in ["qt  ", "avc1", "m4a ", "m4b ", "m4p "] {
            let mut bytes = vec![0u8; 12];
            bytes[4..8].copy_from_slice(b"ftyp");
            bytes[8..12].copy_from_slice(brand.as_bytes());
            assert_eq!(sniff_media(&bytes), Some(MediaKind::Mp4), "{brand}");
        }
        let mut heif = vec![0u8; 12];
        heif[4..8].copy_from_slice(b"ftyp");
        heif[8..12].copy_from_slice(b"heif");
        assert_eq!(sniff_media(&heif), Some(MediaKind::Heif));
        assert_eq!(MediaKind::from_extension("m4a"), Some(MediaKind::Mp4));
        assert_eq!(MediaKind::from_extension("m4b"), Some(MediaKind::Mp4));
        let mut pixels = image::RgbImage::new(3, 2);
        for (x, y, pixel) in pixels.enumerate_pixels_mut() {
            *pixel = image::Rgb([(x * 37 + y * 91) as u8, (x * 11 + 7) as u8, 200]);
        }
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(pixels)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        let png = out.into_inner();
        assert_eq!(sniff_media(&png), Some(MediaKind::Png));
        let picture = decode_image_bytes_in_budget(&png, 7, 0, 0, 8192).unwrap();
        assert_eq!((picture.width, picture.height), (3, 2));
        assert_eq!(picture.rgba.len(), 3 * 2 * 4);
    }

    #[test]
    fn span_slice_matches_the_union_of_individual_cell_slices() {
        let image = (384, 224);
        let boxed = (40, 14);
        let cell_px = (10, 23);
        let (dst_span, src_span) = cell_span_slice(image, boxed, cell_px, 5, 0, 40).unwrap();
        let mut x_min = f32::INFINITY;
        let mut x_max = f32::NEG_INFINITY;
        let mut u_min = f32::INFINITY;
        let mut u_max = f32::NEG_INFINITY;
        let mut y_dst = None;
        let mut y_src = None;
        for c in 0..40u16 {
            if let Some((dst, src)) = cell_slice(image, boxed, cell_px, 5, c) {
                let offset = f32::from(c) * cell_px.0 as f32;
                x_min = x_min.min(offset + dst[0]);
                x_max = x_max.max(offset + dst[0] + dst[2]);
                u_min = u_min.min(src[0]);
                u_max = u_max.max(src[0] + src[2]);
                y_dst = Some((dst[1], dst[3]));
                y_src = Some((src[1], src[3]));
            }
        }
        assert!((dst_span[0] - x_min).abs() < 1e-4);
        assert!((dst_span[2] - (x_max - x_min)).abs() < 1e-4);
        assert_eq!((dst_span[1], dst_span[3]), y_dst.unwrap());
        assert!((src_span[0] - u_min).abs() < 1e-4);
        assert!((src_span[2] - (u_max - u_min)).abs() < 1e-4);
        assert_eq!((src_span[1], src_span[3]), y_src.unwrap());
    }

    #[test]
    fn fast_header_path_names_dimensions_without_the_image_crate() {
        let mut png = vec![0u8; 24];
        png[..8].copy_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        png[12..16].copy_from_slice(b"IHDR");
        png[16..20].copy_from_slice(&3u32.to_be_bytes());
        png[20..24].copy_from_slice(&2u32.to_be_bytes());
        assert_eq!(fast_header_dimensions(MediaKind::Png, &png), Some((3, 2)));
        assert_eq!(sniff_media(&png), Some(MediaKind::Png));

        let gif = [b'G', b'I', b'F', b'8', b'9', b'a', 3, 0, 2, 0];
        assert_eq!(fast_header_dimensions(MediaKind::Gif, &gif), Some((3, 2)));

        let mut bmp = vec![0u8; 26];
        bmp[0..2].copy_from_slice(b"BM");
        bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
        bmp[18..22].copy_from_slice(&3i32.to_le_bytes());
        bmp[22..26].copy_from_slice(&2i32.to_le_bytes());
        assert_eq!(fast_header_dimensions(MediaKind::Bmp, &bmp), Some((3, 2)));

        let mut qoi = vec![0u8; 14];
        qoi[..4].copy_from_slice(b"qoif");
        qoi[4..8].copy_from_slice(&3u32.to_be_bytes());
        qoi[8..12].copy_from_slice(&2u32.to_be_bytes());
        assert_eq!(fast_header_dimensions(MediaKind::Qoi, &qoi), Some((3, 2)));

        let jpeg: Vec<u8> = vec![
            0xFF, 0xD8,
            0xFF, 0xE0, 0x00, 0x04, 0xAA, 0xBB,
            0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x02, 0x00, 0x03, 0x01, 0x02,
            0x03,
        ];
        assert_eq!(fast_header_dimensions(MediaKind::Jpeg, &jpeg), Some((3, 2)));

        let mut zero_png = png.clone();
        zero_png[16..20].copy_from_slice(&0u32.to_be_bytes());
        assert_eq!(fast_header_dimensions(MediaKind::Png, &zero_png), None);
        assert_eq!(fast_header_dimensions(MediaKind::Png, &[0u8; 10]), None);
        let mut webp = vec![0u8; 25];
        webp[0..4].copy_from_slice(b"RIFF");
        webp[8..12].copy_from_slice(b"WEBP");
        webp[12..16].copy_from_slice(b"VP8L");
        webp[20] = 0x2F;
        let bits: u32 = 2 | (1 << 14);
        webp[21..25].copy_from_slice(&bits.to_le_bytes());
        assert_eq!(fast_header_dimensions(MediaKind::WebP, &webp), Some((3, 2)));
        let ico = [0u8, 0, 1, 0, 1, 0, 3, 2];
        assert_eq!(fast_header_dimensions(MediaKind::Ico, &ico), Some((3, 2)));
        assert_eq!(fast_header_dimensions(MediaKind::Pnm, b"P6\n3 2\n"), Some((3, 2)));
        assert_eq!(
            fast_header_dimensions(MediaKind::Pnm, b"P6\n# comment\n3 2\n"),
            Some((3, 2))
        );
        let mut tiff = vec![0u8; 8 + 2 + 2 * 12];
        tiff[0..4].copy_from_slice(b"II*\0");
        tiff[4..8].copy_from_slice(&8u32.to_le_bytes());
        tiff[8..10].copy_from_slice(&2u16.to_le_bytes());
        tiff[10..12].copy_from_slice(&256u16.to_le_bytes());
        tiff[12..14].copy_from_slice(&3u16.to_le_bytes());
        tiff[14..18].copy_from_slice(&1u32.to_le_bytes());
        tiff[18..22].copy_from_slice(&3u32.to_le_bytes());
        tiff[22..24].copy_from_slice(&257u16.to_le_bytes());
        tiff[24..26].copy_from_slice(&3u16.to_le_bytes());
        tiff[26..30].copy_from_slice(&1u32.to_le_bytes());
        tiff[30..34].copy_from_slice(&2u32.to_le_bytes());
        assert_eq!(fast_header_dimensions(MediaKind::Tiff, &tiff), Some((3, 2)));
        let mut be = vec![0u8; 8 + 2 + 2 * 12];
        be[0..4].copy_from_slice(b"MM\0*");
        be[4..8].copy_from_slice(&8u32.to_be_bytes());
        be[8..10].copy_from_slice(&2u16.to_be_bytes());
        be[10..12].copy_from_slice(&256u16.to_be_bytes());
        be[12..14].copy_from_slice(&3u16.to_be_bytes());
        be[14..18].copy_from_slice(&1u32.to_be_bytes());
        be[18..22].copy_from_slice(&[0x00, 0x03, 0x00, 0x00]);
        be[22..24].copy_from_slice(&257u16.to_be_bytes());
        be[24..26].copy_from_slice(&3u16.to_be_bytes());
        be[26..30].copy_from_slice(&1u32.to_be_bytes());
        be[30..34].copy_from_slice(&[0x00, 0x02, 0x00, 0x00]);
        assert_eq!(fast_header_dimensions(MediaKind::Tiff, &be), Some((3, 2)));
        let hdr = b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y 2 +X 3\n";
        assert_eq!(fast_header_dimensions(MediaKind::Hdr, hdr), Some((3, 2)));
    }

    #[test]
    fn f32_to_f16_rounds_like_the_unreal_uploader_needs() {
        assert_eq!(f32_to_f16_bits(0.0), 0x0000);
        assert_eq!(f32_to_f16_bits(1.0), 0x3C00);
        assert_eq!(f32_to_f16_bits(-1.0), 0xBC00);
        assert_eq!(f32_to_f16_bits(f32::INFINITY), 0x7C00);
        assert!(f32_to_f16_bits(f32::NAN) & 0x7C00 == 0x7C00);
        assert_eq!(f32_to_f16_bits(65504.0), 0x7BFF);
        assert_eq!(f32_to_f16_bits(100_000.0), 0x7C00);
        let packed = rgb32f_to_rgba_f16le(&[1.0, 0.0, 0.5]);
        assert_eq!(packed.len(), 8);
        assert_eq!(&packed[0..2], &0x3C00u16.to_le_bytes());
        assert_eq!(&packed[2..4], &0x0000u16.to_le_bytes());
        assert_eq!(&packed[6..8], &0x3C00u16.to_le_bytes());
        assert_eq!(hdr_f16_byte_len(3, 2), 48);
    }

    #[test]
    fn routed_header_path_names_video_codec_vector_without_pixel_decode() {
        let mut mp4 = Vec::new();
        mp4.extend_from_slice(&20u32.to_be_bytes());
        mp4.extend_from_slice(b"ftyp");
        mp4.extend_from_slice(b"isom");
        mp4.extend_from_slice(&[0u8; 8]);
        let tkhd_payload_len = 84usize;
        let tkhd_size = (8 + tkhd_payload_len) as u32;
        mp4.extend_from_slice(&tkhd_size.to_be_bytes());
        mp4.extend_from_slice(b"tkhd");
        let mut tkhd = vec![0u8; tkhd_payload_len];
        tkhd[0] = 0;
        tkhd[76..80].copy_from_slice(&(640u32 << 16).to_be_bytes());
        tkhd[80..84].copy_from_slice(&(360u32 << 16).to_be_bytes());
        mp4.extend_from_slice(&tkhd);
        assert_eq!(sniff_media(&mp4), Some(MediaKind::Mp4));
        assert_eq!(routed_header_dimensions(MediaKind::Mp4, &mp4), Some((640, 360)));
        assert_eq!(probe_dimensions(&mp4), Some((MediaKind::Mp4, 640, 360)));

        let mut mp4v = Vec::new();
        mp4v.extend_from_slice(&20u32.to_be_bytes());
        mp4v.extend_from_slice(b"ftyp");
        mp4v.extend_from_slice(b"isom");
        mp4v.extend_from_slice(&[0u8; 8]);
        let zero_tkhd_size = (8 + 84) as u32;
        mp4v.extend_from_slice(&zero_tkhd_size.to_be_bytes());
        mp4v.extend_from_slice(b"tkhd");
        mp4v.extend_from_slice(&[0u8; 84]);
        let avc_size = (8 + 32) as u32;
        mp4v.extend_from_slice(&avc_size.to_be_bytes());
        mp4v.extend_from_slice(b"avc1");
        let mut avc = vec![0u8; 32];
        avc[24..26].copy_from_slice(&640u16.to_be_bytes());
        avc[26..28].copy_from_slice(&360u16.to_be_bytes());
        mp4v.extend_from_slice(&avc);
        assert_eq!(
            routed_header_dimensions(MediaKind::Mp4, &mp4v),
            Some((640, 360))
        );

        let mut webm = vec![0x1A, 0x45, 0xDF, 0xA3, 0x93, 0x42, 0x82, 0x88, b'm', b'a',
            b't', b'r', b'o', b's', b'k', b'a'];
        webm.extend_from_slice(&[0xB0, 0x82, 0x02, 0x80]);
        webm.extend_from_slice(&[0xBA, 0x82, 0x01, 0x68]);
        assert_eq!(sniff_media(&webm), Some(MediaKind::Webm));
        assert_eq!(
            routed_header_dimensions(MediaKind::Webm, &webm),
            Some((640, 360))
        );
        assert_eq!(
            probe_dimensions(&webm),
            Some((MediaKind::Webm, 640, 360))
        );

        let mut avif = Vec::new();
        avif.extend_from_slice(&20u32.to_be_bytes());
        avif.extend_from_slice(b"ftyp");
        avif.extend_from_slice(b"avif");
        avif.extend_from_slice(&[0u8; 8]);
        avif.extend_from_slice(&20u32.to_be_bytes());
        avif.extend_from_slice(b"ispe");
        avif.extend_from_slice(&[0, 0, 0, 0]);
        avif.extend_from_slice(&3u32.to_be_bytes());
        avif.extend_from_slice(&2u32.to_be_bytes());
        assert_eq!(sniff_media(&avif), Some(MediaKind::Avif));
        assert_eq!(
            routed_header_dimensions(MediaKind::Avif, &avif),
            Some((3, 2))
        );
        assert_eq!(probe_dimensions(&avif), Some((MediaKind::Avif, 3, 2)));

        let svg = b"<svg width=\"640\" height=\"480\"></svg>";
        assert_eq!(sniff_media(svg), Some(MediaKind::Svg));
        assert_eq!(routed_header_dimensions(MediaKind::Svg, svg), Some((640, 480)));
        assert_eq!(probe_dimensions(svg), Some((MediaKind::Svg, 640, 480)));
        let svg_box = b"<?xml version=\"1.0\"?><!-- c --><svg viewBox=\"0 0 3 2\"></svg>";
        assert_eq!(sniff_media(svg_box), Some(MediaKind::Svg));
        assert_eq!(
            routed_header_dimensions(MediaKind::Svg, svg_box),
            Some((3, 2))
        );
        let svg_pct = b"<svg width=\"100%\" height=\"100%\" viewBox=\"0 0 3 2\"></svg>";
        assert_eq!(
            routed_header_dimensions(MediaKind::Svg, svg_pct),
            Some((3, 2))
        );

        let plan = serve_plan(&mp4, None, 8192).unwrap();
        assert_eq!(plan.native, Some((640, 360)));
        assert_eq!(plan.fitted, Some((640, 360)));
        assert_eq!(plan.routed_reason, Some("video-pipeline"));
        assert_eq!(routed_header_dimensions(MediaKind::Mp4, b"ftyp"), None);
        assert_eq!(routed_header_dimensions(MediaKind::Svg, b"<svg></svg>"), None);
    }
}
