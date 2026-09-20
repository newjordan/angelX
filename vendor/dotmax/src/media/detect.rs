//! Format detection via magic bytes and extension fallback.
//!
//! This module implements fast format detection by reading file signatures
//! (magic bytes) from the first 16 bytes of a file.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::Result;

// ============================================================================
// Format Enums (AC: #2, #3)
// ============================================================================

/// Detected media format.
///
/// This enum categorizes media files into their rendering pipelines:
/// - Static images → existing `ImageRenderer`
/// - SVG → existing SVG rasterization (with `svg` feature)
/// - Animated formats → future animated playback (Stories 9.2, 9.3)
/// - Video → future video playback (Story 9.4)
///
/// # Examples
///
/// ```
/// use dotmax::media::{MediaFormat, ImageFormat};
///
/// let format = MediaFormat::StaticImage(ImageFormat::Png);
/// println!("Format: {}", format);  // "static image (PNG)"
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaFormat {
    /// Static raster image (PNG, JPEG, GIF, BMP, WebP, TIFF)
    StaticImage(ImageFormat),

    /// Animated GIF (contains multiple frames)
    ///
    /// Note: Detection of animated vs static GIF requires reading
    /// beyond magic bytes. Initially detected as `StaticImage(Gif)`,
    /// then promoted to `AnimatedGif` after frame count check.
    AnimatedGif,

    /// Animated PNG (APNG)
    AnimatedPng,

    /// SVG vector graphics
    Svg,

    /// Video file with detected codec
    Video(VideoCodec),

    /// Unknown or unsupported format
    Unknown,
}

impl std::fmt::Display for MediaFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StaticImage(img) => write!(f, "static image ({})", img),
            Self::AnimatedGif => write!(f, "animated GIF"),
            Self::AnimatedPng => write!(f, "animated PNG (APNG)"),
            Self::Svg => write!(f, "SVG vector graphics"),
            Self::Video(codec) => write!(f, "video ({})", codec),
            Self::Unknown => write!(f, "unknown format"),
        }
    }
}

/// Static image format variants.
///
/// These formats are handled by the existing `ImageRenderer` pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// PNG (Portable Network Graphics)
    Png,
    /// JPEG (Joint Photographic Experts Group)
    Jpeg,
    /// GIF (Graphics Interchange Format) - static, single frame
    Gif,
    /// BMP (Windows Bitmap)
    Bmp,
    /// WebP (Google's image format)
    WebP,
    /// TIFF (Tagged Image File Format)
    Tiff,
}

impl std::fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Png => write!(f, "PNG"),
            Self::Jpeg => write!(f, "JPEG"),
            Self::Gif => write!(f, "GIF"),
            Self::Bmp => write!(f, "BMP"),
            Self::WebP => write!(f, "WebP"),
            Self::Tiff => write!(f, "TIFF"),
        }
    }
}

/// Video codec detected from container format.
///
/// Note: This is a simplified detection based on container format.
/// Actual codec detection would require parsing container metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    /// H.264/AVC (common in MP4)
    H264,
    /// H.265/HEVC
    H265,
    /// VP9 (common in WebM)
    Vp9,
    /// AV1 (next-gen codec)
    Av1,
    /// Other/unknown codec
    Other,
}

impl std::fmt::Display for VideoCodec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::H264 => write!(f, "H.264"),
            Self::H265 => write!(f, "H.265"),
            Self::Vp9 => write!(f, "VP9"),
            Self::Av1 => write!(f, "AV1"),
            Self::Other => write!(f, "unknown codec"),
        }
    }
}

// ============================================================================
// Magic Byte Detection (AC: #2, #3, #7)
// ============================================================================

/// Maximum bytes needed for format detection.
///
/// 16 bytes is sufficient for all supported formats:
/// - PNG: 8 bytes
/// - JPEG: 3 bytes
/// - GIF: 6 bytes
/// - BMP: 2 bytes
/// - WebP: 12 bytes (RIFF + WEBP)
/// - TIFF: 4 bytes
/// - MP4: 8 bytes (offset 4 + ftyp)
/// - MKV/WebM: 4 bytes
/// - AVI: 12 bytes (RIFF + AVI)
/// - SVG: 5 bytes (<?xml or <svg)
const MAGIC_BYTES_SIZE: usize = 16;

/// Detects the media format of a file by reading its magic bytes.
///
/// This function reads only the first 16 bytes of the file, making it
/// extremely fast even for large files.
///
/// For GIF files, this function performs additional detection to distinguish
/// between static (single-frame) and animated (multi-frame) GIFs. Animated
/// GIFs are returned as `MediaFormat::AnimatedGif`.
///
/// # Arguments
///
/// * `path` - Path to the file to detect
///
/// # Returns
///
/// The detected [`MediaFormat`], or `MediaFormat::Unknown` if the format
/// cannot be determined from magic bytes. Falls back to extension parsing
/// if magic bytes are inconclusive.
///
/// # Errors
///
/// Returns `DotmaxError::Terminal` if the file cannot be read.
///
/// # Performance
///
/// Detection completes in <5ms regardless of file size, as only the
/// first 16 bytes are read. For GIF files, animated detection adds
/// minimal overhead (stops after finding 2 frames).
///
/// # Examples
///
/// ```no_run
/// use dotmax::media::{detect_format, MediaFormat};
///
/// let format = detect_format("photo.png")?;
/// println!("Detected: {}", format);
///
/// // Animated GIF detection
/// let gif_format = detect_format("animation.gif")?;
/// match gif_format {
///     MediaFormat::AnimatedGif => println!("Animated GIF!"),
///     MediaFormat::StaticImage(_) => println!("Static image"),
///     _ => {}
/// }
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
pub fn detect_format(path: impl AsRef<Path>) -> Result<MediaFormat> {
    let path = path.as_ref();

    // Read magic bytes
    let mut file = File::open(path)?;
    let mut buffer = [0u8; MAGIC_BYTES_SIZE];
    let bytes_read = file.read(&mut buffer)?;

    // Detect from magic bytes
    let format = detect_format_from_bytes(&buffer[..bytes_read]);

    // If unknown, try extension fallback
    if format == MediaFormat::Unknown {
        return Ok(detect_from_extension(path));
    }

    // For GIF files, check if animated (Story 9.2 - AC #1)
    #[cfg(feature = "image")]
    if matches!(format, MediaFormat::StaticImage(ImageFormat::Gif)) && is_animated_gif(path)? {
        return Ok(MediaFormat::AnimatedGif);
    }

    // For PNG files, check if animated (Story 9.3 - AC #1, #2)
    #[cfg(feature = "image")]
    if matches!(format, MediaFormat::StaticImage(ImageFormat::Png)) && is_animated_png(path)? {
        return Ok(MediaFormat::AnimatedPng);
    }

    Ok(format)
}

/// Detects media format from raw bytes.
///
/// This function is useful when you already have the file's header bytes
/// in memory (e.g., from a network stream or embedded data).
///
/// # Arguments
///
/// * `bytes` - The first bytes of the file (at least 16 recommended)
///
/// # Returns
///
/// The detected [`MediaFormat`], or `MediaFormat::Unknown` if no known
/// signature matches.
///
/// # Examples
///
/// ```
/// use dotmax::media::{detect_format_from_bytes, MediaFormat, ImageFormat};
///
/// // JPEG magic bytes
/// let jpeg_bytes = &[0xFF, 0xD8, 0xFF, 0xE0];
/// let format = detect_format_from_bytes(jpeg_bytes);
/// assert!(matches!(format, MediaFormat::StaticImage(ImageFormat::Jpeg)));
///
/// // Unknown bytes
/// let unknown = detect_format_from_bytes(&[0x00, 0x00, 0x00, 0x00]);
/// assert!(matches!(unknown, MediaFormat::Unknown));
/// ```
#[must_use]
pub fn detect_format_from_bytes(bytes: &[u8]) -> MediaFormat {
    // PNG: 89 50 4E 47 0D 0A 1A 0A
    if bytes.len() >= 8
        && bytes[0] == 0x89
        && bytes[1] == 0x50
        && bytes[2] == 0x4E
        && bytes[3] == 0x47
        && bytes[4] == 0x0D
        && bytes[5] == 0x0A
        && bytes[6] == 0x1A
        && bytes[7] == 0x0A
    {
        return MediaFormat::StaticImage(ImageFormat::Png);
    }

    // JPEG: FF D8 FF
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return MediaFormat::StaticImage(ImageFormat::Jpeg);
    }

    // GIF: 47 49 46 38 (GIF8)
    // Note: Both GIF87a and GIF89a start with GIF8
    if bytes.len() >= 4
        && bytes[0] == 0x47
        && bytes[1] == 0x49
        && bytes[2] == 0x46
        && bytes[3] == 0x38
    {
        // For now, return as static GIF. Story 9.2 will add animated detection.
        return MediaFormat::StaticImage(ImageFormat::Gif);
    }

    // BMP: 42 4D (BM)
    if bytes.len() >= 2 && bytes[0] == 0x42 && bytes[1] == 0x4D {
        return MediaFormat::StaticImage(ImageFormat::Bmp);
    }

    // WebP: 52 49 46 46 ?? ?? ?? ?? 57 45 42 50 (RIFF....WEBP)
    if bytes.len() >= 12
        && bytes[0] == 0x52
        && bytes[1] == 0x49
        && bytes[2] == 0x46
        && bytes[3] == 0x46
        && bytes[8] == 0x57
        && bytes[9] == 0x45
        && bytes[10] == 0x42
        && bytes[11] == 0x50
    {
        return MediaFormat::StaticImage(ImageFormat::WebP);
    }

    // TIFF (little-endian): 49 49 2A 00 (II*\0)
    if bytes.len() >= 4
        && bytes[0] == 0x49
        && bytes[1] == 0x49
        && bytes[2] == 0x2A
        && bytes[3] == 0x00
    {
        return MediaFormat::StaticImage(ImageFormat::Tiff);
    }

    // TIFF (big-endian): 4D 4D 00 2A (MM\0*)
    if bytes.len() >= 4
        && bytes[0] == 0x4D
        && bytes[1] == 0x4D
        && bytes[2] == 0x00
        && bytes[3] == 0x2A
    {
        return MediaFormat::StaticImage(ImageFormat::Tiff);
    }

    // SVG: Check for XML declaration or <svg tag
    // <?xml = 3C 3F 78 6D 6C
    // <svg  = 3C 73 76 67
    if bytes.len() >= 5
        && bytes[0] == 0x3C
        && bytes[1] == 0x3F
        && bytes[2] == 0x78
        && bytes[3] == 0x6D
        && bytes[4] == 0x6C
    {
        return MediaFormat::Svg;
    }
    if bytes.len() >= 4
        && bytes[0] == 0x3C
        && bytes[1] == 0x73
        && bytes[2] == 0x76
        && bytes[3] == 0x67
    {
        return MediaFormat::Svg;
    }

    // Also check for SVG with leading whitespace or BOM
    // Look for "<svg" anywhere in first 16 bytes (handles BOM, whitespace)
    if let Ok(text) = std::str::from_utf8(bytes) {
        let text_lower = text.to_lowercase();
        if text_lower.contains("<svg") || text_lower.contains("<?xml") {
            return MediaFormat::Svg;
        }
    }

    // MP4: ftyp at offset 4 (66 74 79 70)
    if bytes.len() >= 8
        && bytes[4] == 0x66
        && bytes[5] == 0x74
        && bytes[6] == 0x79
        && bytes[7] == 0x70
    {
        // MP4/MOV container - assume H.264 as it's most common
        return MediaFormat::Video(VideoCodec::H264);
    }

    // MKV/WebM: EBML header 1A 45 DF A3
    if bytes.len() >= 4
        && bytes[0] == 0x1A
        && bytes[1] == 0x45
        && bytes[2] == 0xDF
        && bytes[3] == 0xA3
    {
        // Could be MKV or WebM - assume VP9 for WebM, Other for MKV
        // More sophisticated detection would check DocType element
        return MediaFormat::Video(VideoCodec::Vp9);
    }

    // AVI: 52 49 46 46 ?? ?? ?? ?? 41 56 49 20 (RIFF....AVI )
    if bytes.len() >= 12
        && bytes[0] == 0x52
        && bytes[1] == 0x49
        && bytes[2] == 0x46
        && bytes[3] == 0x46
        && bytes[8] == 0x41
        && bytes[9] == 0x56
        && bytes[10] == 0x49
        && bytes[11] == 0x20
    {
        return MediaFormat::Video(VideoCodec::Other);
    }

    MediaFormat::Unknown
}

// ============================================================================
// Extension Fallback (AC: #4)
// ============================================================================

/// Detects format from file extension when magic bytes are inconclusive.
fn detect_from_extension(path: &Path) -> MediaFormat {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase);

    match ext.as_deref() {
        // Static images
        Some("png") => MediaFormat::StaticImage(ImageFormat::Png),
        Some("jpg" | "jpeg") => MediaFormat::StaticImage(ImageFormat::Jpeg),
        Some("gif") => MediaFormat::StaticImage(ImageFormat::Gif),
        Some("bmp") => MediaFormat::StaticImage(ImageFormat::Bmp),
        Some("webp") => MediaFormat::StaticImage(ImageFormat::WebP),
        Some("tif" | "tiff") => MediaFormat::StaticImage(ImageFormat::Tiff),

        // SVG
        Some("svg") => MediaFormat::Svg,

        // Video
        Some("mp4" | "m4v" | "mov") => MediaFormat::Video(VideoCodec::H264),
        Some("mkv" | "avi") => MediaFormat::Video(VideoCodec::Other),
        Some("webm") => MediaFormat::Video(VideoCodec::Vp9),

        // Unknown
        _ => MediaFormat::Unknown,
    }
}

// ============================================================================
// Animated GIF Detection (AC: #1 - Story 9.2)
// ============================================================================

/// Checks if a GIF file contains multiple frames (animated).
///
/// This function opens the GIF file and iterates through frames, stopping
/// as soon as a second frame is found. This avoids loading the entire GIF
/// into memory for detection purposes.
///
/// # Arguments
///
/// * `path` - Path to the GIF file
///
/// # Returns
///
/// - `Ok(true)` if the GIF has more than one frame (animated)
/// - `Ok(false)` if the GIF has exactly one frame (static)
/// - `Err` if the file cannot be opened or decoded
///
/// # Errors
///
/// Returns `DotmaxError::Terminal` if the file cannot be opened (not found,
/// permission denied, etc.).
///
/// # Performance
///
/// This function is optimized for detection, not full loading:
/// - Stops iteration after finding 2 frames
/// - Does not decode pixel data (just metadata)
/// - Typical execution: <5ms even for large animated GIFs
///
/// # Examples
///
/// ```no_run
/// use dotmax::media::is_animated_gif;
///
/// // Check if a GIF is animated
/// if is_animated_gif("animation.gif")? {
///     println!("This GIF is animated!");
/// }
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
#[cfg(feature = "image")]
pub fn is_animated_gif(path: impl AsRef<Path>) -> Result<bool> {
    use gif::DecodeOptions;

    let path = path.as_ref();
    let file = File::open(path)?;

    // Create decoder with minimal options (we only need frame count)
    let mut options = DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::Indexed);

    let mut decoder = match options.read_info(file) {
        Ok(d) => d,
        Err(e) => {
            // If we can't decode as GIF, treat as static (not animated)
            tracing::debug!(
                "GIF decode error for {:?}: {:?}, treating as static",
                path,
                e
            );
            return Ok(false);
        }
    };

    // Count frames - stop at 2 (we only need to know if > 1)
    let mut frame_count = 0;
    while decoder.read_next_frame().ok().flatten().is_some() {
        frame_count += 1;
        if frame_count > 1 {
            return Ok(true); // Animated (more than one frame)
        }
    }

    Ok(false) // Static (0 or 1 frame)
}

/// Checks if a GIF file is animated by reading from raw bytes.
///
/// This is useful when you have the GIF data in memory (e.g., from a network stream).
/// The function reads the GIF metadata to determine if it has multiple frames.
///
/// # Arguments
///
/// * `bytes` - The complete GIF file data
///
/// # Returns
///
/// `true` if the GIF has more than one frame, `false` otherwise.
/// Returns `false` for corrupted or invalid GIF data.
///
/// # Examples
///
/// ```
/// use dotmax::media::is_animated_gif_from_bytes;
///
/// // Single-frame GIF (typical static image)
/// let static_gif = include_bytes!("../../tests/fixtures/media/static.gif");
/// // This would return false for a static GIF fixture
/// ```
#[cfg(feature = "image")]
#[must_use]
pub fn is_animated_gif_from_bytes(bytes: &[u8]) -> bool {
    use gif::DecodeOptions;
    use std::io::Cursor;

    let cursor = Cursor::new(bytes);
    let mut options = DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::Indexed);

    let Ok(mut decoder) = options.read_info(cursor) else {
        return false;
    };

    let mut frame_count = 0;
    while decoder.read_next_frame().ok().flatten().is_some() {
        frame_count += 1;
        if frame_count > 1 {
            return true;
        }
    }

    false
}

// ============================================================================
// Animated PNG Detection (AC: #1, #2 - Story 9.3)
// ============================================================================

/// Checks if a PNG file is an animated PNG (APNG).
///
/// APNG files contain an `acTL` (Animation Control) chunk that indicates
/// animation data is present. This function reads the file and checks for
/// the presence of this chunk without decoding all frame data.
///
/// # Arguments
///
/// * `path` - Path to the PNG file
///
/// # Returns
///
/// - `Ok(true)` if the PNG has animation control data (APNG)
/// - `Ok(false)` if the PNG is static (no `acTL` chunk)
/// - `Err` if the file cannot be opened or decoded
///
/// # Errors
///
/// Returns `DotmaxError::Terminal` if the file cannot be opened (not found,
/// permission denied, etc.).
///
/// # Performance
///
/// This function is optimized for detection, not full loading:
/// - Only reads PNG header and initial chunks
/// - Stops after finding `acTL` chunk
/// - Typical execution: <5ms even for large APNGs
///
/// # Examples
///
/// ```no_run
/// use dotmax::media::is_animated_png;
///
/// // Check if a PNG is animated
/// if is_animated_png("animation.png")? {
///     println!("This PNG is animated (APNG)!");
/// }
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
#[cfg(feature = "image")]
pub fn is_animated_png(path: impl AsRef<Path>) -> Result<bool> {
    use std::io::BufReader;

    let path = path.as_ref();
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let decoder = png::Decoder::new(reader);

    let png_reader = match decoder.read_info() {
        Ok(r) => r,
        Err(e) => {
            // If we can't decode as PNG, treat as not animated
            tracing::debug!(
                "PNG decode error for {:?}: {:?}, treating as static",
                path,
                e
            );
            return Ok(false);
        }
    };

    // Check for animation control chunk - if present, it's an APNG
    Ok(png_reader.info().animation_control().is_some())
}

/// Checks if PNG data is an animated PNG (APNG) by reading from raw bytes.
///
/// This is useful when you have the PNG data in memory (e.g., from a network stream).
/// The function reads the PNG metadata to determine if it has animation data.
///
/// # Arguments
///
/// * `bytes` - The complete PNG file data
///
/// # Returns
///
/// `true` if the PNG has an `acTL` (animation control) chunk, `false` otherwise.
/// Returns `false` for corrupted or invalid PNG data.
///
/// # Examples
///
/// ```ignore
/// use dotmax::media::is_animated_png_from_bytes;
///
/// // Check bytes in memory
/// let png_data: &[u8] = &[...];
/// if is_animated_png_from_bytes(png_data) {
///     println!("This is an animated PNG!");
/// }
/// ```
#[cfg(feature = "image")]
#[must_use]
pub fn is_animated_png_from_bytes(bytes: &[u8]) -> bool {
    use std::io::Cursor;

    let cursor = Cursor::new(bytes);
    let decoder = png::Decoder::new(cursor);

    let Ok(png_reader) = decoder.read_info() else {
        return false;
    };

    png_reader.info().animation_control().is_some()
}

// ============================================================================
// Tests (AC: #2, #3, #4)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Magic Byte Detection Tests (AC: #2)
    // ========================================================================

    #[test]
    fn test_detect_png_magic() {
        let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00];
        assert_eq!(
            detect_format_from_bytes(&png),
            MediaFormat::StaticImage(ImageFormat::Png)
        );
    }

    #[test]
    fn test_detect_jpeg_magic() {
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        assert_eq!(
            detect_format_from_bytes(&jpeg),
            MediaFormat::StaticImage(ImageFormat::Jpeg)
        );
    }

    #[test]
    fn test_detect_jpeg_exif_magic() {
        // JPEG with EXIF marker
        let jpeg_exif = [0xFF, 0xD8, 0xFF, 0xE1, 0x00, 0x10];
        assert_eq!(
            detect_format_from_bytes(&jpeg_exif),
            MediaFormat::StaticImage(ImageFormat::Jpeg)
        );
    }

    #[test]
    fn test_detect_gif87a_magic() {
        let gif87 = [0x47, 0x49, 0x46, 0x38, 0x37, 0x61];
        assert_eq!(
            detect_format_from_bytes(&gif87),
            MediaFormat::StaticImage(ImageFormat::Gif)
        );
    }

    #[test]
    fn test_detect_gif89a_magic() {
        let gif89 = [0x47, 0x49, 0x46, 0x38, 0x39, 0x61];
        assert_eq!(
            detect_format_from_bytes(&gif89),
            MediaFormat::StaticImage(ImageFormat::Gif)
        );
    }

    #[test]
    fn test_detect_bmp_magic() {
        let bmp = [0x42, 0x4D, 0x00, 0x00];
        assert_eq!(
            detect_format_from_bytes(&bmp),
            MediaFormat::StaticImage(ImageFormat::Bmp)
        );
    }

    #[test]
    fn test_detect_webp_magic() {
        // RIFF....WEBP
        let webp = [
            0x52, 0x49, 0x46, 0x46, // RIFF
            0x00, 0x00, 0x00, 0x00, // size (ignored)
            0x57, 0x45, 0x42, 0x50, // WEBP
        ];
        assert_eq!(
            detect_format_from_bytes(&webp),
            MediaFormat::StaticImage(ImageFormat::WebP)
        );
    }

    #[test]
    fn test_detect_tiff_little_endian_magic() {
        let tiff_le = [0x49, 0x49, 0x2A, 0x00];
        assert_eq!(
            detect_format_from_bytes(&tiff_le),
            MediaFormat::StaticImage(ImageFormat::Tiff)
        );
    }

    #[test]
    fn test_detect_tiff_big_endian_magic() {
        let tiff_be = [0x4D, 0x4D, 0x00, 0x2A];
        assert_eq!(
            detect_format_from_bytes(&tiff_be),
            MediaFormat::StaticImage(ImageFormat::Tiff)
        );
    }

    // ========================================================================
    // SVG Detection Tests
    // ========================================================================

    #[test]
    fn test_detect_svg_xml_declaration() {
        let svg = b"<?xml version=\"1.0\"?>";
        assert_eq!(detect_format_from_bytes(svg), MediaFormat::Svg);
    }

    #[test]
    fn test_detect_svg_direct() {
        let svg = b"<svg xmlns=\"http";
        assert_eq!(detect_format_from_bytes(svg), MediaFormat::Svg);
    }

    // ========================================================================
    // Video Format Detection Tests (AC: #3)
    // ========================================================================

    #[test]
    fn test_detect_mp4_magic() {
        // ....ftyp
        let mp4 = [
            0x00, 0x00, 0x00, 0x18, // size
            0x66, 0x74, 0x79, 0x70, // ftyp
            0x69, 0x73, 0x6F, 0x6D, // isom
        ];
        assert_eq!(
            detect_format_from_bytes(&mp4),
            MediaFormat::Video(VideoCodec::H264)
        );
    }

    #[test]
    fn test_detect_mkv_webm_magic() {
        // EBML header
        let mkv = [0x1A, 0x45, 0xDF, 0xA3];
        assert_eq!(
            detect_format_from_bytes(&mkv),
            MediaFormat::Video(VideoCodec::Vp9)
        );
    }

    #[test]
    fn test_detect_avi_magic() {
        // RIFF....AVI
        let avi = [
            0x52, 0x49, 0x46, 0x46, // RIFF
            0x00, 0x00, 0x00, 0x00, // size
            0x41, 0x56, 0x49, 0x20, // AVI
        ];
        assert_eq!(
            detect_format_from_bytes(&avi),
            MediaFormat::Video(VideoCodec::Other)
        );
    }

    // ========================================================================
    // Extension Fallback Tests (AC: #4)
    // ========================================================================

    #[test]
    fn test_extension_fallback_png() {
        let format = detect_from_extension(Path::new("image.png"));
        assert_eq!(format, MediaFormat::StaticImage(ImageFormat::Png));
    }

    #[test]
    fn test_extension_fallback_jpeg_variants() {
        assert_eq!(
            detect_from_extension(Path::new("image.jpg")),
            MediaFormat::StaticImage(ImageFormat::Jpeg)
        );
        assert_eq!(
            detect_from_extension(Path::new("image.jpeg")),
            MediaFormat::StaticImage(ImageFormat::Jpeg)
        );
    }

    #[test]
    fn test_extension_fallback_case_insensitive() {
        assert_eq!(
            detect_from_extension(Path::new("IMAGE.PNG")),
            MediaFormat::StaticImage(ImageFormat::Png)
        );
        assert_eq!(
            detect_from_extension(Path::new("video.MP4")),
            MediaFormat::Video(VideoCodec::H264)
        );
    }

    #[test]
    fn test_extension_fallback_unknown() {
        assert_eq!(
            detect_from_extension(Path::new("file.xyz")),
            MediaFormat::Unknown
        );
    }

    #[test]
    fn test_extension_fallback_no_extension() {
        assert_eq!(
            detect_from_extension(Path::new("noextension")),
            MediaFormat::Unknown
        );
    }

    // ========================================================================
    // Unknown Format Tests
    // ========================================================================

    #[test]
    fn test_detect_unknown_bytes() {
        let unknown = [0x00, 0x00, 0x00, 0x00];
        assert_eq!(detect_format_from_bytes(&unknown), MediaFormat::Unknown);
    }

    #[test]
    fn test_detect_empty_bytes() {
        let empty: &[u8] = &[];
        assert_eq!(detect_format_from_bytes(empty), MediaFormat::Unknown);
    }

    // ========================================================================
    // Display Trait Tests
    // ========================================================================

    #[test]
    fn test_media_format_display() {
        assert_eq!(
            format!("{}", MediaFormat::StaticImage(ImageFormat::Png)),
            "static image (PNG)"
        );
        assert_eq!(format!("{}", MediaFormat::AnimatedGif), "animated GIF");
        assert_eq!(
            format!("{}", MediaFormat::Video(VideoCodec::H264)),
            "video (H.264)"
        );
        assert_eq!(format!("{}", MediaFormat::Unknown), "unknown format");
    }

    #[test]
    fn test_image_format_display() {
        assert_eq!(format!("{}", ImageFormat::Png), "PNG");
        assert_eq!(format!("{}", ImageFormat::Jpeg), "JPEG");
        assert_eq!(format!("{}", ImageFormat::WebP), "WebP");
    }

    #[test]
    fn test_video_codec_display() {
        assert_eq!(format!("{}", VideoCodec::H264), "H.264");
        assert_eq!(format!("{}", VideoCodec::Vp9), "VP9");
        assert_eq!(format!("{}", VideoCodec::Other), "unknown codec");
    }

    // ========================================================================
    // Animated GIF Detection Tests (Story 9.2 - AC #1)
    // ========================================================================

    #[cfg(feature = "image")]
    mod animated_gif_tests {
        use super::*;
        use std::path::Path;

        #[test]
        fn test_is_animated_gif_static() {
            let path = Path::new("tests/fixtures/media/static.gif");
            if path.exists() {
                let result = is_animated_gif(path).unwrap();
                assert!(!result, "Static GIF should return false");
            }
        }

        #[test]
        fn test_is_animated_gif_animated() {
            let path = Path::new("tests/fixtures/media/animated.gif");
            if path.exists() {
                let result = is_animated_gif(path).unwrap();
                assert!(result, "Animated GIF should return true");
            }
        }

        #[test]
        fn test_is_animated_gif_nonexistent() {
            let result = is_animated_gif("nonexistent.gif");
            assert!(result.is_err(), "Nonexistent file should return error");
        }

        #[test]
        fn test_is_animated_gif_from_bytes_static() {
            // Minimal static GIF (single frame)
            let static_gif = include_bytes!("../../tests/fixtures/media/static.gif");
            assert!(
                !is_animated_gif_from_bytes(static_gif),
                "Static GIF bytes should return false"
            );
        }

        #[test]
        fn test_is_animated_gif_from_bytes_animated() {
            let animated_gif = include_bytes!("../../tests/fixtures/media/animated.gif");
            assert!(
                is_animated_gif_from_bytes(animated_gif),
                "Animated GIF bytes should return true"
            );
        }

        #[test]
        fn test_is_animated_gif_from_bytes_invalid() {
            let invalid = &[0x00, 0x01, 0x02, 0x03];
            assert!(
                !is_animated_gif_from_bytes(invalid),
                "Invalid data should return false"
            );
        }

        #[test]
        fn test_detect_format_static_gif() {
            let path = Path::new("tests/fixtures/media/static.gif");
            if path.exists() {
                let format = detect_format(path).unwrap();
                assert_eq!(
                    format,
                    MediaFormat::StaticImage(ImageFormat::Gif),
                    "Static GIF should be detected as StaticImage(Gif)"
                );
            }
        }

        #[test]
        fn test_detect_format_animated_gif() {
            let path = Path::new("tests/fixtures/media/animated.gif");
            if path.exists() {
                let format = detect_format(path).unwrap();
                assert_eq!(
                    format,
                    MediaFormat::AnimatedGif,
                    "Animated GIF should be detected as AnimatedGif"
                );
            }
        }
    }

    // ========================================================================
    // Animated PNG Detection Tests (Story 9.3 - AC #1, #2)
    // ========================================================================

    #[cfg(feature = "image")]
    mod animated_png_tests {
        use super::*;
        use std::path::Path;

        #[test]
        fn test_is_animated_png_static() {
            let path = Path::new("tests/fixtures/media/static_png.png");
            if path.exists() {
                let result = is_animated_png(path).unwrap();
                assert!(!result, "Static PNG should return false");
            }
        }

        #[test]
        fn test_is_animated_png_animated() {
            let path = Path::new("tests/fixtures/media/animated.png");
            if path.exists() {
                let result = is_animated_png(path).unwrap();
                assert!(result, "Animated PNG should return true");
            }
        }

        #[test]
        fn test_is_animated_png_nonexistent() {
            let result = is_animated_png("nonexistent.png");
            assert!(result.is_err(), "Nonexistent file should return error");
        }

        #[test]
        fn test_is_animated_png_from_bytes_static() {
            let static_png = include_bytes!("../../tests/fixtures/media/static_png.png");
            assert!(
                !is_animated_png_from_bytes(static_png),
                "Static PNG bytes should return false"
            );
        }

        #[test]
        fn test_is_animated_png_from_bytes_animated() {
            let animated_png = include_bytes!("../../tests/fixtures/media/animated.png");
            assert!(
                is_animated_png_from_bytes(animated_png),
                "Animated PNG bytes should return true"
            );
        }

        #[test]
        fn test_is_animated_png_from_bytes_invalid() {
            let invalid = &[0x00, 0x01, 0x02, 0x03];
            assert!(
                !is_animated_png_from_bytes(invalid),
                "Invalid data should return false"
            );
        }

        #[test]
        fn test_detect_format_static_png() {
            let path = Path::new("tests/fixtures/media/static_png.png");
            if path.exists() {
                let format = detect_format(path).unwrap();
                assert_eq!(
                    format,
                    MediaFormat::StaticImage(ImageFormat::Png),
                    "Static PNG should be detected as StaticImage(Png)"
                );
            }
        }

        #[test]
        fn test_detect_format_animated_png() {
            let path = Path::new("tests/fixtures/media/animated.png");
            if path.exists() {
                let format = detect_format(path).unwrap();
                assert_eq!(
                    format,
                    MediaFormat::AnimatedPng,
                    "Animated PNG should be detected as AnimatedPng"
                );
            }
        }
    }
}
