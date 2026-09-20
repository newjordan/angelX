//! Error types for dotmax operations
//!
//! This module defines `DotmaxError`, the primary error type returned by all
//! public dotmax APIs. All errors include contextual information (coordinates,
//! dimensions, indices) to aid debugging.
//!
//! # Zero Panics Policy
//!
//! All public API methods return `Result<T, DotmaxError>` instead of panicking.
//! This ensures applications can gracefully handle all error conditions.
//!
//! # Examples
//!
//! ```
//! use dotmax::{BrailleGrid, DotmaxError};
//!
//! // Create grid with invalid dimensions
//! let result = BrailleGrid::new(0, 10);
//! match result {
//!     Err(DotmaxError::InvalidDimensions { width, height }) => {
//!         println!("Invalid dimensions: {}×{}", width, height);
//!     }
//!     _ => unreachable!(),
//! }
//!
//! // Access out-of-bounds coordinates
//! let mut grid = BrailleGrid::new(10, 10).unwrap();
//! let result = grid.set_dot(100, 50);
//! match result {
//!     Err(DotmaxError::OutOfBounds { x, y, width, height }) => {
//!         println!("({}, {}) is outside {}×{} grid", x, y, width, height);
//!     }
//!     _ => unreachable!(),
//! }
//! ```

use thiserror::Error;

/// Comprehensive error type for all dotmax operations
///
/// All variants include contextual information to aid debugging and provide
/// actionable error messages to end users.
#[derive(Error, Debug)]
pub enum DotmaxError {
    /// Grid dimensions are invalid (zero or exceeding maximum limits)
    ///
    /// Valid dimensions must satisfy:
    /// - `width > 0 && width <= 10,000`
    /// - `height > 0 && height <= 10,000`
    #[error("Invalid grid dimensions: width={width}, height={height}")]
    InvalidDimensions {
        /// The invalid width value
        width: usize,
        /// The invalid height value
        height: usize,
    },

    /// Coordinate access is outside grid boundaries
    ///
    /// Valid coordinates must satisfy:
    /// - `x < width`
    /// - `y < height`
    #[error("Out of bounds access: ({x}, {y}) in grid of size ({width}, {height})")]
    OutOfBounds {
        /// The X coordinate that was out of bounds
        x: usize,
        /// The Y coordinate that was out of bounds
        y: usize,
        /// The grid width
        width: usize,
        /// The grid height
        height: usize,
    },

    /// Dot index is invalid (must be 0-7 for 2×4 braille cells)
    ///
    /// Valid dot indices:
    /// ```text
    /// 0 3    (positions in braille cell)
    /// 1 4
    /// 2 5
    /// 6 7
    /// ```
    #[error("Invalid dot index: {index} (must be 0-7)")]
    InvalidDotIndex {
        /// The invalid dot index (must be 0-7)
        index: u8,
    },

    /// Terminal I/O error from underlying terminal backend
    ///
    /// This wraps `std::io::Error` using `#[from]` to preserve the error source
    /// chain for proper debugging and error context propagation.
    #[error("Terminal I/O error: {0}")]
    Terminal(#[from] std::io::Error),

    /// Terminal backend operation failed
    ///
    /// Used for terminal-specific errors that don't map to standard I/O errors
    /// (e.g., capability detection failures, initialization errors).
    #[error("Terminal backend error: {0}")]
    TerminalBackend(String),

    /// Frame-pack export failed
    ///
    /// This error is returned when static animation frame data cannot be
    /// serialized for website or documentation previews.
    #[error("Frame export error: {0}")]
    FrameExport(String),

    /// Unicode braille character conversion failed
    ///
    /// This should rarely occur as braille Unicode range (U+2800–U+28FF) is
    /// well-defined, but may happen if cell data becomes corrupted.
    #[error("Unicode conversion failed for cell ({x}, {y})")]
    UnicodeConversion {
        /// The X coordinate of the cell
        x: usize,
        /// The Y coordinate of the cell
        y: usize,
    },

    /// Image loading failed (file not found, decode error, etc.)
    ///
    /// This error wraps the underlying `image::ImageError` using `#[source]`
    /// to preserve the error chain for debugging.
    ///
    /// Common causes:
    /// - File does not exist or is not readable
    /// - File format is corrupted or unsupported
    /// - Memory allocation failure during decode
    #[cfg(feature = "image")]
    #[error("Failed to load image from {path:?}: {source}")]
    ImageLoad {
        /// Path to the image file
        path: std::path::PathBuf,
        /// Underlying image loading error
        #[source]
        source: image::ImageError,
    },

    /// Unsupported image format
    ///
    /// The provided file or byte buffer is not in a supported image format.
    /// See [`crate::image::supported_formats`] for the list of valid formats.
    #[cfg(feature = "image")]
    #[error("Unsupported image format: {format}")]
    UnsupportedFormat {
        /// The unsupported format name
        format: String,
    },

    /// Image dimensions exceed maximum limits
    ///
    /// Images larger than 10,000×10,000 pixels are rejected to prevent
    /// memory exhaustion attacks.
    #[cfg(feature = "image")]
    #[error("Invalid image dimensions: {width}×{height} exceeds maximum (10,000×10,000)")]
    InvalidImageDimensions {
        /// The image width in pixels
        width: u32,
        /// The image height in pixels
        height: u32,
    },

    /// Invalid parameter value provided to image processing function
    ///
    /// This error is returned when a function parameter (brightness, contrast,
    /// gamma, etc.) is outside its valid range.
    ///
    /// The error message includes:
    /// - Parameter name (e.g., "brightness factor")
    /// - Provided value
    /// - Valid range (min-max)
    #[cfg(feature = "image")]
    #[error("Invalid {parameter_name}: {value} (valid range: {min}-{max})")]
    InvalidParameter {
        /// Name of the invalid parameter
        parameter_name: String,
        /// The invalid value provided
        value: String,
        /// Minimum valid value
        min: String,
        /// Maximum valid value
        max: String,
    },

    /// SVG rendering error (parsing or rasterization failure)
    ///
    /// This error is returned when SVG loading fails due to:
    /// - Malformed or invalid SVG syntax
    /// - Unsupported SVG features (complex filters, animations)
    /// - Rasterization failures (pixmap creation, rendering errors)
    /// - Font loading issues for text-heavy SVGs
    ///
    /// The error message includes descriptive context to aid debugging.
    #[cfg(feature = "svg")]
    #[error("SVG rendering error: {0}")]
    SvgError(String),

    /// Invalid line thickness (must be ≥ 1)
    ///
    /// This error is returned when attempting to draw a line with thickness=0.
    /// Valid thickness values must be at least 1. For braille resolution,
    /// recommended maximum is 10 dots.
    #[error("Invalid line thickness: {thickness} (must be ≥ 1)")]
    InvalidThickness {
        /// The invalid thickness value
        thickness: u32,
    },

    /// Invalid polygon definition
    ///
    /// This error is returned when attempting to draw a polygon with invalid
    /// parameters (e.g., fewer than 3 vertices, empty vertex list).
    /// Polygons require at least 3 vertices to form a closed shape.
    #[error("Invalid polygon: {reason}")]
    InvalidPolygon {
        /// The reason the polygon is invalid
        reason: String,
    },

    /// Density set cannot be empty
    ///
    /// This error is returned when attempting to create a `DensitySet` with an
    /// empty character list. A valid density set must contain at least one
    /// character for intensity mapping.
    #[error("Density set cannot be empty")]
    EmptyDensitySet,

    /// Density set has too many characters (max 256)
    ///
    /// This error is returned when attempting to create a `DensitySet` with more
    /// than 256 characters. The limit ensures reasonable memory usage and
    /// mapping performance.
    #[error("Density set has too many characters: {count} (max 256)")]
    TooManyCharacters {
        /// The number of characters in the set
        count: usize,
    },

    /// Intensity buffer size mismatch with grid dimensions
    ///
    /// This error is returned when the intensity buffer length does not match
    /// the expected grid size (width × height). All intensity buffers must
    /// have exactly one f32 value per grid cell.
    #[error(
        "Intensity buffer size mismatch: expected {expected} (grid width × height), got {actual}"
    )]
    BufferSizeMismatch {
        /// Expected buffer size (grid width × height)
        expected: usize,
        /// Actual buffer size provided
        actual: usize,
    },

    /// Color scheme cannot have an empty color list
    ///
    /// This error is returned when attempting to create a `ColorScheme` with an
    /// empty color vector. A valid color scheme must contain at least one color
    /// stop for intensity mapping.
    #[error("Color scheme cannot be empty: at least one color is required")]
    EmptyColorScheme,

    /// Invalid color scheme configuration
    ///
    /// This error is returned when attempting to build a `ColorScheme` with an
    /// invalid configuration. Common causes include:
    /// - Fewer than 2 color stops defined
    /// - Duplicate intensity values at the same position
    ///
    /// The error message provides specific details about the validation failure.
    #[error("Invalid color scheme: {0}")]
    InvalidColorScheme(String),

    /// Invalid intensity value for color scheme
    ///
    /// This error is returned when a color stop's intensity value is outside
    /// the valid range of 0.0 to 1.0 (inclusive).
    ///
    /// Valid intensity values must satisfy: `0.0 <= intensity <= 1.0`
    #[error("Invalid intensity value: {0} (must be 0.0-1.0)")]
    InvalidIntensity(f32),

    /// Unsupported or unknown media format
    ///
    /// This error is returned when attempting to display or load a file
    /// with an unsupported or unrecognized format. The format detection
    /// system could not identify the file type from magic bytes or extension.
    ///
    /// Supported formats include:
    /// - Static images: PNG, JPEG, GIF (single frame), BMP, WebP, TIFF
    /// - Vector graphics: SVG (requires `svg` feature)
    /// - Animated: GIF (multi-frame), APNG
    /// - Video: MP4, MKV, AVI, WebM (requires `video` feature)
    #[error("Unsupported media format: {format}. Supported: static (PNG, JPEG, GIF, BMP, WebP, TIFF), vector (SVG), animated (GIF, APNG), video (MP4, MKV, AVI, WebM)")]
    FormatError {
        /// Description of the detected or unknown format
        format: String,
    },

    /// GIF decoding or playback error
    ///
    /// This error is returned when a GIF file cannot be decoded or played back.
    /// Common causes include:
    /// - Corrupted GIF file
    /// - Invalid GIF structure
    /// - Memory allocation failure during decode
    /// - Frame decode errors
    #[cfg(feature = "image")]
    #[error("GIF error for {path:?}: {message}")]
    GifError {
        /// Path to the GIF file
        path: std::path::PathBuf,
        /// Error message
        message: String,
    },

    /// APNG decoding or playback error
    ///
    /// This error is returned when an APNG file cannot be decoded or played back.
    /// Common causes include:
    /// - Corrupted APNG file or invalid chunk structure
    /// - Missing or invalid animation control (acTL) chunk
    /// - Missing or invalid frame control (fcTL) chunks
    /// - Memory allocation failure during decode
    /// - Frame decode errors
    #[cfg(feature = "image")]
    #[error("APNG error for {path:?}: {message}")]
    ApngError {
        /// Path to the APNG file
        path: std::path::PathBuf,
        /// Error message
        message: String,
    },

    /// Video decoding or playback error
    ///
    /// This error is returned when a video file cannot be decoded or played back.
    /// Common causes include:
    /// - File not found or cannot be opened
    /// - No video stream found in container
    /// - Unsupported video codec
    /// - FFmpeg initialization failure
    /// - Frame decode errors
    ///
    /// Requires the `video` feature and FFmpeg system libraries.
    #[cfg(feature = "video")]
    #[error("Video error for {path:?}: {message}")]
    VideoError {
        /// Path to the video file
        path: std::path::PathBuf,
        /// Error message
        message: String,
    },

    /// Webcam capture error
    ///
    /// This error is returned when a webcam cannot be accessed or captured from.
    /// Common causes include:
    /// - No webcam detected on the system
    /// - Camera is in use by another application
    /// - Permission denied to access camera
    /// - Invalid device ID or path
    /// - FFmpeg device capture initialization failure
    ///
    /// Requires the `video` feature and FFmpeg system libraries.
    #[cfg(feature = "video")]
    #[error("Webcam error for {device}: {message}")]
    WebcamError {
        /// Device identifier (path, name, or index)
        device: String,
        /// Error message
        message: String,
    },

    /// Camera device not found
    ///
    /// This error is returned when the specified camera device cannot be found.
    /// The error includes a list of available cameras to help the user select
    /// a valid device.
    ///
    /// # Remediation
    ///
    /// - Check the device path/name is correct
    /// - Use `list_webcams()` to see available devices
    /// - Ensure the camera is connected and recognized by the OS
    #[cfg(feature = "video")]
    #[error("Camera not found: {device}. Available cameras: {}", if available.is_empty() { "none detected".to_string() } else { available.join(", ") })]
    CameraNotFound {
        /// The device that was requested but not found
        device: String,
        /// List of available camera names/paths
        available: Vec<String>,
    },

    /// Camera permission denied
    ///
    /// This error is returned when the application lacks permission to access
    /// the camera. This is common on systems with privacy controls.
    ///
    /// # Remediation
    ///
    /// - **Linux**: Add user to `video` group (`sudo usermod -aG video $USER`)
    /// - **macOS**: Grant camera access in System Preferences > Security & Privacy
    /// - **Windows**: Grant camera access in Settings > Privacy > Camera
    #[cfg(feature = "video")]
    #[error("Camera permission denied: {device}. {hint}")]
    CameraPermissionDenied {
        /// The device that permission was denied for
        device: String,
        /// Platform-specific remediation hint
        hint: String,
    },

    /// Camera is in use by another application
    ///
    /// This error is returned when the camera is exclusively locked by another
    /// process. Most cameras can only be used by one application at a time.
    ///
    /// # Remediation
    ///
    /// - Close other applications that might be using the camera
    /// - Check for video conferencing apps (Zoom, Teams, Meet, etc.)
    /// - Check for other terminal applications using the camera
    #[cfg(feature = "video")]
    #[error("Camera in use: {device}. Close other applications that may be using the camera (video conferencing, browsers, etc.)")]
    CameraInUse {
        /// The device that is in use
        device: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_dimensions_message_includes_context() {
        let err = DotmaxError::InvalidDimensions {
            width: 0,
            height: 10,
        };
        let msg = format!("{err}");
        assert!(msg.contains('0'));
        assert!(msg.contains("10"));
        assert!(msg.contains("width"));
        assert!(msg.contains("height"));
    }

    // ========================================================================
    // Story 5.4: InvalidColorScheme and InvalidIntensity Error Tests
    // ========================================================================

    #[test]
    fn test_invalid_color_scheme_message_includes_reason() {
        let err = DotmaxError::InvalidColorScheme("at least 2 colors required".into());
        let msg = format!("{err}");
        assert!(msg.contains("Invalid color scheme"));
        assert!(msg.contains("at least 2 colors required"));
    }

    #[test]
    fn test_invalid_color_scheme_duplicate_intensity() {
        let err = DotmaxError::InvalidColorScheme("duplicate intensity value".into());
        let msg = format!("{err}");
        assert!(msg.contains("Invalid color scheme"));
        assert!(msg.contains("duplicate"));
    }

    #[test]
    fn test_invalid_intensity_negative() {
        let err = DotmaxError::InvalidIntensity(-0.5);
        let msg = format!("{err}");
        assert!(msg.contains("Invalid intensity value"));
        assert!(msg.contains("-0.5"));
        assert!(msg.contains("0.0-1.0"));
    }

    #[test]
    fn test_invalid_intensity_above_one() {
        let err = DotmaxError::InvalidIntensity(1.5);
        let msg = format!("{err}");
        assert!(msg.contains("Invalid intensity value"));
        assert!(msg.contains("1.5"));
        assert!(msg.contains("0.0-1.0"));
    }

    #[test]
    fn test_out_of_bounds_message_includes_all_context() {
        let err = DotmaxError::OutOfBounds {
            x: 100,
            y: 50,
            width: 80,
            height: 24,
        };
        let msg = format!("{err}");
        assert!(msg.contains("100"));
        assert!(msg.contains("50"));
        assert!(msg.contains("80"));
        assert!(msg.contains("24"));
    }

    #[test]
    fn test_invalid_dot_index_message_includes_index() {
        let err = DotmaxError::InvalidDotIndex { index: 10 };
        let msg = format!("{err}");
        assert!(msg.contains("10"));
        assert!(msg.contains("0-7"));
    }

    #[test]
    fn test_unicode_conversion_message_includes_coordinates() {
        let err = DotmaxError::UnicodeConversion { x: 15, y: 20 };
        let msg = format!("{err}");
        assert!(msg.contains("15"));
        assert!(msg.contains("20"));
    }

    #[test]
    fn test_terminal_backend_message() {
        let err = DotmaxError::TerminalBackend("Test error".to_string());
        let msg = format!("{err}");
        assert!(msg.contains("Test error"));
        assert!(msg.contains("Terminal backend error"));
    }

    #[test]
    fn test_io_error_automatic_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test file");
        let dotmax_err: DotmaxError = io_err.into();
        assert!(matches!(dotmax_err, DotmaxError::Terminal(_)));
    }

    #[test]
    fn test_io_error_preserves_source() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let dotmax_err: DotmaxError = io_err.into();

        match dotmax_err {
            DotmaxError::Terminal(inner) => {
                assert_eq!(inner.kind(), std::io::ErrorKind::PermissionDenied);
                assert!(inner.to_string().contains("access denied"));
            }
            _ => panic!("Expected Terminal variant"),
        }
    }

    #[cfg(feature = "image")]
    #[test]
    fn test_image_load_error_includes_path_and_source() {
        use std::path::PathBuf;
        let err = DotmaxError::ImageLoad {
            path: PathBuf::from("/path/to/image.png"),
            source: image::ImageError::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "file not found",
            )),
        };
        let msg = format!("{err}");
        assert!(msg.contains("image.png"));
        assert!(msg.contains("Failed to load"));
    }

    #[cfg(feature = "image")]
    #[test]
    fn test_unsupported_format_error_includes_format() {
        let err = DotmaxError::UnsupportedFormat {
            format: "xyz".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("xyz"));
        assert!(msg.contains("Unsupported"));
    }

    #[cfg(feature = "image")]
    #[test]
    fn test_invalid_image_dimensions_includes_dimensions() {
        let err = DotmaxError::InvalidImageDimensions {
            width: 15_000,
            height: 20_000,
        };
        let msg = format!("{err}");
        assert!(msg.contains("15000") || msg.contains("15,000"));
        assert!(msg.contains("20000") || msg.contains("20,000"));
        assert!(msg.contains("10,000"));
    }

    #[cfg(feature = "image")]
    #[test]
    fn test_invalid_parameter_includes_all_context() {
        let err = DotmaxError::InvalidParameter {
            parameter_name: "brightness factor".to_string(),
            value: "3.5".to_string(),
            min: "0.0".to_string(),
            max: "2.0".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("brightness factor"));
        assert!(msg.contains("3.5"));
        assert!(msg.contains("0.0"));
        assert!(msg.contains("2.0"));
        assert!(msg.contains("Invalid"));
    }

    // ========================================================================
    // Story 9.1: FormatError Tests (AC: #6)
    // ========================================================================

    #[test]
    fn test_format_error_includes_format_name() {
        let err = DotmaxError::FormatError {
            format: "unknown format".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("unknown format"));
        assert!(msg.contains("Unsupported media format"));
    }

    #[test]
    fn test_format_error_includes_supported_formats() {
        let err = DotmaxError::FormatError {
            format: "xyz".to_string(),
        };
        let msg = format!("{err}");
        // Static formats
        assert!(msg.contains("PNG"));
        assert!(msg.contains("JPEG"));
        assert!(msg.contains("GIF"));
        // Vector formats
        assert!(msg.contains("SVG"));
        // Animated formats
        assert!(msg.contains("APNG"));
        // Video formats
        assert!(msg.contains("MP4"));
        assert!(msg.contains("MKV"));
    }

    // ========================================================================
    // Story 9.6: Webcam Error Tests (AC: #7)
    // ========================================================================

    #[cfg(feature = "video")]
    #[test]
    fn test_webcam_error_includes_device_and_message() {
        let err = DotmaxError::WebcamError {
            device: "/dev/video0".to_string(),
            message: "Failed to open device".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("/dev/video0"));
        assert!(msg.contains("Failed to open device"));
        assert!(msg.contains("Webcam error"));
    }

    #[cfg(feature = "video")]
    #[test]
    fn test_camera_not_found_includes_device_and_available_list() {
        let err = DotmaxError::CameraNotFound {
            device: "/dev/video5".to_string(),
            available: vec!["/dev/video0".to_string(), "/dev/video1".to_string()],
        };
        let msg = format!("{err}");
        assert!(msg.contains("/dev/video5"));
        assert!(msg.contains("/dev/video0"));
        assert!(msg.contains("/dev/video1"));
        assert!(msg.contains("Camera not found"));
        assert!(msg.contains("Available cameras"));
    }

    #[cfg(feature = "video")]
    #[test]
    fn test_camera_not_found_empty_available_list() {
        let err = DotmaxError::CameraNotFound {
            device: "camera0".to_string(),
            available: vec![],
        };
        let msg = format!("{err}");
        assert!(msg.contains("camera0"));
        assert!(msg.contains("none detected"));
    }

    #[cfg(feature = "video")]
    #[test]
    fn test_camera_permission_denied_includes_hint() {
        let err = DotmaxError::CameraPermissionDenied {
            device: "/dev/video0".to_string(),
            hint: "Add user to video group".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("/dev/video0"));
        assert!(msg.contains("Add user to video group"));
        assert!(msg.contains("permission denied"));
    }

    #[cfg(feature = "video")]
    #[test]
    fn test_camera_in_use_includes_remediation() {
        let err = DotmaxError::CameraInUse {
            device: "Integrated Camera".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("Integrated Camera"));
        assert!(msg.contains("in use"));
        assert!(msg.contains("Close other applications"));
    }
}
