//! Universal media format detection and routing.
//!
//! This module provides automatic format detection and routing for media files,
//! enabling a single API to handle images, animations, and videos.
//!
//! # Overview
//!
//! The media module solves the problem of displaying arbitrary media files without
//! knowing their format in advance. Using magic byte detection (file signatures)
//! combined with extension fallback, it identifies the format and routes to the
//! appropriate renderer.
//!
//! # Format Detection
//!
//! Format detection works in two stages:
//!
//! 1. **Magic Bytes**: Read the first 16 bytes and match against known file signatures
//! 2. **Extension Fallback**: If magic bytes are inconclusive, use file extension as hint
//!
//! This approach is fast (<5ms even for large files) and reliable.
//!
//! # Supported Formats
//!
//! ## Static Images
//! - PNG, JPEG, GIF (static), BMP, WebP, TIFF
//!
//! ## Vector Graphics
//! - SVG (requires `svg` feature)
//!
//! ## Animated Formats (Future)
//! - Animated GIF (Story 9.2)
//! - Animated PNG/APNG (Story 9.3)
//!
//! ## Video Formats (Future)
//! - MP4, MKV, AVI, WebM (Story 9.4)
//!
//! # Examples
//!
//! ## Detect a File's Format
//!
//! ```no_run
//! use dotmax::media::{detect_format, MediaFormat};
//!
//! let format = detect_format("image.png")?;
//! match format {
//!     MediaFormat::StaticImage(_) => println!("It's a static image!"),
//!     MediaFormat::Svg => println!("It's an SVG!"),
//!     MediaFormat::Video(_) => println!("It's a video!"),
//!     _ => println!("Other format"),
//! }
//! # Ok::<(), dotmax::DotmaxError>(())
//! ```
//!
//! ## Detect Format from Bytes
//!
//! ```
//! use dotmax::media::{detect_format_from_bytes, MediaFormat, ImageFormat};
//!
//! // PNG magic bytes
//! let png_bytes = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
//! let format = detect_format_from_bytes(png_bytes);
//! assert!(matches!(format, MediaFormat::StaticImage(ImageFormat::Png)));
//! ```
//!
//! # Performance
//!
//! Format detection is designed to be extremely fast:
//! - Only reads first 16 bytes from file
//! - No full file loading required
//! - Completes in <5ms even for multi-gigabyte files
//!
//! # Error Handling
//!
//! Detection functions return clear errors:
//! - `DotmaxError::Terminal` for I/O errors (file not found, permission denied)
//! - `DotmaxError::FormatError` for unsupported/unknown formats

#[cfg(feature = "image")]
pub mod apng;
mod detect;
#[cfg(feature = "image")]
pub mod gif;
mod router;
#[cfg(feature = "video")]
pub mod video;
#[cfg(feature = "video")]
pub mod webcam;

// Public re-exports
#[cfg(feature = "image")]
pub use apng::{ApngFrame, ApngPlayer, BlendOp, DisposeOp};
pub use detect::{detect_format, detect_format_from_bytes, ImageFormat, MediaFormat, VideoCodec};
#[cfg(feature = "image")]
pub use detect::{is_animated_gif, is_animated_gif_from_bytes};
#[cfg(feature = "image")]
pub use detect::{is_animated_png, is_animated_png_from_bytes};
#[cfg(feature = "image")]
pub use gif::{DisposalMethod, GifFrame, GifPlayer};
pub use router::{MediaContent, MediaPlayer};
#[cfg(feature = "video")]
pub use video::{TimedFrame, TimedRgbaFrame, VideoPlayer};
#[cfg(feature = "video")]
pub use webcam::{list_webcams, WebcamDevice, WebcamDeviceId, WebcamPlayer, WebcamPlayerBuilder};
