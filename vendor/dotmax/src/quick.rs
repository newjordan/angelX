//! Quick one-liner functions for common dotmax tasks.
//!
//! This module provides the simplest possible API for displaying images
//! and working with braille grids. Use these functions when you want
//! results fast without configuring options.
//!
//! # Overview
//!
//! The `quick` module is designed for rapid prototyping and simple use cases
//! where sensible defaults are sufficient. For fine-grained control, use the
//! underlying [`BrailleGrid`] and [`TerminalRenderer`] types directly
//! (plus `ImageRenderer` with the `image` feature).
//!
//! # Examples
//!
//! ## Display an Image (One Line!)
//!
//! ```ignore
//! // Requires `image` feature
//! use dotmax::quick;
//!
//! // Display an image with auto-detected terminal size and Floyd-Steinberg dithering
//! quick::show_image("photo.png")?;
//! # Ok::<(), dotmax::DotmaxError>(())
//! ```
//!
//! ## Create and Display a Custom Grid
//!
//! ```no_run
//! use dotmax::quick;
//! use dotmax::primitives::draw_circle;
//!
//! // Get a terminal-sized grid for drawing
//! let mut grid = quick::grid()?;
//!
//! // Draw something on it
//! draw_circle(&mut grid, 40, 40, 20)?;
//!
//! // Display and wait for keypress
//! quick::show(&grid)?;
//! # Ok::<(), dotmax::DotmaxError>(())
//! ```
//!
//! ## Load an Image for Manipulation
//!
//! ```ignore
//! // Requires `image` feature
//! use dotmax::quick;
//!
//! // Load image into grid (doesn't display yet)
//! let grid = quick::load_image("photo.png")?;
//! println!("Grid dimensions: {}x{}", grid.width(), grid.height());
//!
//! // Now display it
//! quick::show(&grid)?;
//! # Ok::<(), dotmax::DotmaxError>(())
//! ```
//!
//! # Sensible Defaults
//!
//! All functions use these defaults for optimal results:
//!
//! - **Terminal size**: Auto-detected via `crossterm::terminal::size()`, with 80×24 fallback
//! - **Dithering**: Floyd-Steinberg (best quality for most images)
//! - **Aspect ratio**: Preserved (no distortion)
//! - **Wait behavior**: `show()` and `show_image()` wait for any keypress before returning
//!
//! # Performance
//!
//! Quick functions add minimal overhead (< 5ms) compared to manual API usage.
//! The main cost is the terminal setup/teardown in `show()`.
//!
//! # Error Handling
//!
//! All functions return `Result<_, DotmaxError>` and never panic.
//! Common errors include:
//!
//! - `DotmaxError::ImageLoad` - File not found or unsupported format
//! - `DotmaxError::Terminal` - Terminal I/O errors
//! - `DotmaxError::TerminalBackend` - Terminal too small (minimum 40×12)
//!
//! [`BrailleGrid`]: crate::BrailleGrid
//! [`TerminalRenderer`]: crate::TerminalRenderer

use crate::{BrailleGrid, Result, TerminalRenderer};

// ============================================================================
// Terminal Size Detection (AC: #6)
// ============================================================================

/// Default terminal dimensions when detection fails.
const DEFAULT_WIDTH: usize = 80;
const DEFAULT_HEIGHT: usize = 24;

/// Detects the current terminal size with fallback.
///
/// Uses `crossterm::terminal::size()` to detect terminal dimensions.
/// Returns `(80, 24)` if detection fails (e.g., running without a terminal).
///
/// # Returns
///
/// Tuple of `(width, height)` in terminal cells.
#[inline]
fn terminal_size() -> (usize, usize) {
    crossterm::terminal::size()
        .map(|(w, h)| (w as usize, h as usize))
        .unwrap_or((DEFAULT_WIDTH, DEFAULT_HEIGHT))
}

/// Waits for any keypress.
///
/// Blocks until the user presses any key. Used by `show()` and `show_image()`
/// to prevent immediate return.
fn wait_for_key() -> Result<()> {
    use crossterm::event::{self, Event};

    loop {
        if let Event::Key(_) = event::read()? {
            break;
        }
    }
    Ok(())
}

// ============================================================================
// Core Functions (AC: #2, #3)
// ============================================================================

/// Creates a [`BrailleGrid`] sized to the current terminal.
///
/// This is the quickest way to get a grid ready for drawing. The grid
/// dimensions match the terminal size, so rendering fills the screen.
///
/// # Returns
///
/// A new `BrailleGrid` with dimensions matching the terminal.
///
/// # Errors
///
/// Returns `DotmaxError::InvalidDimensions` if the detected terminal size
/// is invalid (which should never happen with the 80×24 fallback).
///
/// # Examples
///
/// ```no_run
/// use dotmax::quick;
/// use dotmax::primitives::draw_line;
///
/// let mut grid = quick::grid()?;
/// draw_line(&mut grid, 0, 0, 100, 50)?;
/// quick::show(&grid)?;
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
pub fn grid() -> Result<BrailleGrid> {
    let (w, h) = terminal_size();
    BrailleGrid::new(w, h)
}

/// Creates a [`BrailleGrid`] with explicit dimensions.
///
/// Use this when you need a specific grid size rather than terminal-sized.
///
/// # Arguments
///
/// * `width` - Grid width in terminal cells
/// * `height` - Grid height in terminal cells
///
/// # Returns
///
/// A new `BrailleGrid` with the specified dimensions.
///
/// # Errors
///
/// Returns `DotmaxError::InvalidDimensions` if width or height is 0
/// or exceeds maximum limits (10,000).
///
/// # Examples
///
/// ```
/// use dotmax::quick;
///
/// // Create a 100x50 grid
/// let grid = quick::grid_sized(100, 50)?;
/// assert_eq!(grid.width(), 100);
/// assert_eq!(grid.height(), 50);
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
pub fn grid_sized(width: usize, height: usize) -> Result<BrailleGrid> {
    BrailleGrid::new(width, height)
}

/// Displays a grid and waits for a keypress.
///
/// This is a blocking function that:
/// 1. Initializes the terminal (enters raw mode, alternate screen)
/// 2. Renders the grid
/// 3. Waits for any keypress
/// 4. Cleans up terminal state
///
/// # Arguments
///
/// * `grid` - The braille grid to display
///
/// # Errors
///
/// Returns `DotmaxError::Terminal` for I/O errors, or
/// `DotmaxError::TerminalBackend` if the terminal is too small.
///
/// # Examples
///
/// ```no_run
/// use dotmax::quick;
/// use dotmax::primitives::draw_circle;
///
/// let mut grid = quick::grid()?;
/// draw_circle(&mut grid, 80, 48, 30)?;
/// quick::show(&grid)?; // Displays until keypress
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
///
/// # Note
///
/// For non-blocking rendering or more control over terminal state,
/// use [`TerminalRenderer`] directly.
pub fn show(grid: &BrailleGrid) -> Result<()> {
    let mut renderer = TerminalRenderer::new()?;
    renderer.render(grid)?;
    wait_for_key()?;
    // TerminalRenderer::drop() handles cleanup automatically
    Ok(())
}

// ============================================================================
// Image Functions (AC: #4, #5, #7) - Feature-gated
// ============================================================================

/// Loads and displays an image in one call.
///
/// This is the ultimate one-liner for image display:
/// 1. Loads image from file
/// 2. Resizes to terminal dimensions (preserving aspect ratio)
/// 3. Converts to braille using Floyd-Steinberg dithering
/// 4. Displays in terminal
/// 5. Waits for keypress
/// 6. Cleans up
///
/// # Arguments
///
/// * `path` - Path to image file (PNG, JPEG, GIF, BMP, WebP, TIFF supported)
///
/// # Errors
///
/// Returns `DotmaxError::ImageLoad` if the file doesn't exist or format
/// is unsupported, or terminal errors during display.
///
/// # Examples
///
/// ```no_run
/// use dotmax::quick;
///
/// // That's it - one line to display any image!
/// quick::show_image("photo.png")?;
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
#[cfg(feature = "image")]
pub fn show_image(path: impl AsRef<std::path::Path>) -> Result<()> {
    let grid = load_image(path)?;
    show(&grid)
}

/// Loads an image into a [`BrailleGrid`] for further manipulation.
///
/// Like `show_image()` but returns the grid instead of displaying it.
/// Use this when you want to modify the grid before display or use
/// it for other purposes.
///
/// # Arguments
///
/// * `path` - Path to image file
///
/// # Returns
///
/// A `BrailleGrid` containing the rendered image.
///
/// # Errors
///
/// Returns `DotmaxError::ImageLoad` if loading fails.
///
/// # Examples
///
/// ```no_run
/// use dotmax::quick;
/// use dotmax::primitives::draw_rectangle;
///
/// // Load image
/// let mut grid = quick::load_image("photo.png")?;
///
/// // Add a border (uses u32 for width/height)
/// let (w, h) = (grid.dot_width() as u32, grid.dot_height() as u32);
/// draw_rectangle(&mut grid, 0, 0, w, h)?;
///
/// // Now display
/// quick::show(&grid)?;
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
#[cfg(feature = "image")]
pub fn load_image(path: impl AsRef<std::path::Path>) -> Result<BrailleGrid> {
    use crate::image::ImageRenderer;

    let (w, h) = terminal_size();
    ImageRenderer::new()
        .load_from_path(path.as_ref())?
        .resize(w, h, true)?
        .render()
}

/// Loads an image with explicit dimensions.
///
/// Like `load_image()` but with manual size control instead of auto-detection.
///
/// # Arguments
///
/// * `path` - Path to image file
/// * `width` - Target width in terminal cells
/// * `height` - Target height in terminal cells
///
/// # Returns
///
/// A `BrailleGrid` containing the rendered image at the specified size.
///
/// # Errors
///
/// Returns `DotmaxError::ImageLoad` if loading fails, or
/// `DotmaxError::InvalidDimensions` if dimensions are invalid.
///
/// # Examples
///
/// ```no_run
/// use dotmax::quick;
///
/// // Load at specific size
/// let grid = quick::load_image_sized("photo.png", 100, 50)?;
/// assert_eq!(grid.width(), 100);
/// assert_eq!(grid.height(), 50);
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
#[cfg(feature = "image")]
pub fn load_image_sized(
    path: impl AsRef<std::path::Path>,
    width: usize,
    height: usize,
) -> Result<BrailleGrid> {
    use crate::image::ImageRenderer;

    ImageRenderer::new()
        .load_from_path(path.as_ref())?
        .resize(width, height, true)?
        .render()
}

// ============================================================================
// Universal Media Functions (AC: #1, #5, #8) - Story 9.1
// ============================================================================

/// Loads and displays any supported media file.
///
/// This is the universal entry point for media display. It automatically
/// detects the file format using magic bytes and routes to the appropriate
/// renderer.
///
/// # Supported Formats
///
/// - **Static Images**: PNG, JPEG, GIF (static), BMP, WebP, TIFF
/// - **Vector Graphics**: SVG (requires `svg` feature)
/// - **Animated** (future): Animated GIF, APNG
/// - **Video** (future): MP4, MKV, AVI, WebM
///
/// # Arguments
///
/// * `path` - Path to any supported media file
///
/// # Errors
///
/// - `DotmaxError::Terminal` - File not found or read error
/// - `DotmaxError::FormatError` - Unsupported or unknown format
/// - `DotmaxError::ImageLoad` - Image decode error
///
/// # Examples
///
/// ```no_run
/// use dotmax::quick;
///
/// // Display any image - format auto-detected
/// quick::show_file("photo.png")?;
/// quick::show_file("picture.jpg")?;
/// quick::show_file("diagram.svg")?;  // Requires 'svg' feature
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
///
/// # Format Detection
///
/// Format is detected by reading the first 16 bytes (magic bytes) of the file.
/// If magic bytes are inconclusive, the file extension is used as a fallback.
/// This detection is fast (<5ms) regardless of file size.
#[cfg(feature = "image")]
pub fn show_file(path: impl AsRef<std::path::Path>) -> Result<()> {
    use crate::media::{detect_format, MediaFormat};
    use crate::DotmaxError;

    let path = path.as_ref();
    let format = detect_format(path)?;

    match format {
        MediaFormat::StaticImage(_) => {
            // Route to existing show_image() implementation
            show_image(path)
        }
        MediaFormat::Svg => {
            // Route to SVG rendering (if feature enabled)
            #[cfg(feature = "svg")]
            {
                show_svg(path)
            }
            #[cfg(not(feature = "svg"))]
            {
                Err(DotmaxError::FormatError {
                    format: "SVG (requires 'svg' feature)".to_string(),
                })
            }
        }
        MediaFormat::AnimatedGif => {
            // Route to animated GIF playback (Story 9.2)
            play_animated_gif(path)
        }
        MediaFormat::AnimatedPng => {
            // Route to APNG playback (Story 9.3)
            play_animated_png(path)
        }
        MediaFormat::Video(_codec) => {
            // Route to video playback (Story 9.4)
            #[cfg(feature = "video")]
            {
                play_video(path)
            }
            #[cfg(not(feature = "video"))]
            {
                Err(DotmaxError::FormatError {
                    format: "video (requires 'video' feature and FFmpeg libraries)".to_string(),
                })
            }
        }
        MediaFormat::Unknown => Err(DotmaxError::FormatError {
            format: "unknown format".to_string(),
        }),
    }
}

/// Loads any supported media file into a [`crate::media::MediaContent`] for handling.
///
/// This function detects the file format and loads it into the appropriate
/// variant of [`crate::media::MediaContent`]:
/// - Static images → `MediaContent::Static(BrailleGrid)`
/// - Animated content → `MediaContent::Animated(Box<dyn MediaPlayer>)` (future)
///
/// Use this when you need programmatic access to the loaded content rather
/// than immediate display.
///
/// # Arguments
///
/// * `path` - Path to any supported media file
///
/// # Returns
///
/// [`crate::media::MediaContent`] containing the loaded media, ready for rendering or
/// further processing.
///
/// # Errors
///
/// - `DotmaxError::Terminal` - File not found or read error
/// - `DotmaxError::FormatError` - Unsupported or unknown format
/// - `DotmaxError::ImageLoad` - Image decode error
///
/// # Examples
///
/// ```no_run
/// use dotmax::quick;
/// use dotmax::media::MediaContent;
///
/// let content = quick::load_file("image.png")?;
///
/// match content {
///     MediaContent::Static(grid) => {
///         println!("Loaded static image: {}x{}", grid.width(), grid.height());
///         quick::show(&grid)?;
///     }
///     MediaContent::Animated(mut player) => {
///         println!("Loaded animation with {:?} frames", player.frame_count());
///         // Play frames...
///     }
/// }
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
#[cfg(feature = "image")]
pub fn load_file(path: impl AsRef<std::path::Path>) -> Result<crate::media::MediaContent> {
    use crate::media::{detect_format, MediaContent, MediaFormat};
    use crate::DotmaxError;

    let path = path.as_ref();
    let format = detect_format(path)?;

    match format {
        MediaFormat::StaticImage(_) => {
            // Load via existing load_image() and wrap in MediaContent::Static
            let grid = load_image(path)?;
            Ok(MediaContent::Static(grid))
        }
        MediaFormat::Svg => {
            // Load SVG (if feature enabled)
            #[cfg(feature = "svg")]
            {
                let grid = load_svg(path)?;
                Ok(MediaContent::Static(grid))
            }
            #[cfg(not(feature = "svg"))]
            {
                Err(DotmaxError::FormatError {
                    format: "SVG (requires 'svg' feature)".to_string(),
                })
            }
        }
        MediaFormat::AnimatedGif => {
            // Load animated GIF player (Story 9.2)
            use crate::media::GifPlayer;
            let player = GifPlayer::new(path)?;
            Ok(MediaContent::Animated(Box::new(player)))
        }
        MediaFormat::AnimatedPng => {
            // Load animated PNG player (Story 9.3)
            use crate::media::ApngPlayer;
            let player = ApngPlayer::new(path)?;
            Ok(MediaContent::Animated(Box::new(player)))
        }
        MediaFormat::Video(_codec) => {
            // Load video player (Story 9.4)
            #[cfg(feature = "video")]
            {
                use crate::media::VideoPlayer;
                let player = VideoPlayer::new(path)?;
                Ok(MediaContent::Animated(Box::new(player)))
            }
            #[cfg(not(feature = "video"))]
            {
                Err(DotmaxError::FormatError {
                    format: "video (requires 'video' feature and FFmpeg libraries)".to_string(),
                })
            }
        }
        MediaFormat::Unknown => Err(DotmaxError::FormatError {
            format: "unknown format".to_string(),
        }),
    }
}

// ============================================================================
// Animated GIF Helper Functions (Story 9.2)
// ============================================================================

/// Plays an animated GIF file in the terminal.
///
/// This function:
/// 1. Creates a GifPlayer for the file
/// 2. Initializes the terminal (raw mode, alternate screen)
/// 3. Plays frames with correct timing until keypress or loop completion
/// 4. Cleans up terminal state
#[cfg(feature = "image")]
fn play_animated_gif(path: impl AsRef<std::path::Path>) -> Result<()> {
    use crate::media::{GifPlayer, MediaPlayer};
    use crossterm::event::{self, Event, KeyCode};
    use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
    use crossterm::{cursor, execute};
    use std::io::stdout;
    use std::time::{Duration, Instant};

    let mut player = GifPlayer::new(path)?;

    // Enter raw mode and alternate screen
    terminal::enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;

    let mut renderer = TerminalRenderer::new()?;

    // Play frames
    let result = (|| -> Result<()> {
        while let Some(frame_result) = player.next_frame() {
            let (grid, delay) = frame_result?;

            // Render frame
            renderer.render(&grid)?;

            // Wait for frame duration, checking for keypress
            let deadline = Instant::now() + delay;
            while Instant::now() < deadline {
                // Check for keypress with short timeout
                if event::poll(Duration::from_millis(10))? {
                    if let Event::Key(key_event) = event::read()? {
                        // Stop on any key (except modifiers alone)
                        if !matches!(key_event.code, KeyCode::Modifier(_)) {
                            return Ok(());
                        }
                    }
                }
            }
        }

        // Animation complete - wait for final keypress
        wait_for_key()?;
        Ok(())
    })();

    // Cleanup - always restore terminal state
    execute!(stdout, cursor::Show, LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;

    result
}

// ============================================================================
// Animated PNG Helper Functions (Story 9.3)
// ============================================================================

/// Plays an animated PNG (APNG) file in the terminal.
///
/// This function:
/// 1. Creates an ApngPlayer for the file
/// 2. Initializes the terminal (raw mode, alternate screen)
/// 3. Plays frames with correct timing until keypress or loop completion
/// 4. Cleans up terminal state
#[cfg(feature = "image")]
fn play_animated_png(path: impl AsRef<std::path::Path>) -> Result<()> {
    use crate::media::{ApngPlayer, MediaPlayer};
    use crossterm::event::{self, Event, KeyCode};
    use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
    use crossterm::{cursor, execute};
    use std::io::stdout;
    use std::time::{Duration, Instant};

    let mut player = ApngPlayer::new(path)?;

    // Enter raw mode and alternate screen
    terminal::enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;

    let mut renderer = TerminalRenderer::new()?;

    // Play frames
    let result = (|| -> Result<()> {
        while let Some(frame_result) = player.next_frame() {
            let (grid, delay) = frame_result?;

            // Render frame
            renderer.render(&grid)?;

            // Wait for frame duration, checking for keypress
            let deadline = Instant::now() + delay;
            while Instant::now() < deadline {
                // Check for keypress with short timeout
                if event::poll(Duration::from_millis(10))? {
                    if let Event::Key(key_event) = event::read()? {
                        // Stop on any key (except modifiers alone)
                        if !matches!(key_event.code, KeyCode::Modifier(_)) {
                            return Ok(());
                        }
                    }
                }
            }
        }

        // Animation complete - wait for final keypress
        wait_for_key()?;
        Ok(())
    })();

    // Cleanup - always restore terminal state
    execute!(stdout, cursor::Show, LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;

    result
}

// ============================================================================
// Video Playback Helper Functions (Story 9.4)
// ============================================================================

/// Plays a video file in the terminal.
///
/// This function:
/// 1. Creates a VideoPlayer for the file using FFmpeg
/// 2. Initializes the terminal (raw mode, alternate screen)
/// 3. Plays frames with correct timing until keypress or video completion
/// 4. Cleans up terminal state
#[cfg(feature = "video")]
fn play_video(path: impl AsRef<std::path::Path>) -> Result<()> {
    use crate::media::{MediaPlayer, VideoPlayer};
    use crossterm::event::{self, Event, KeyCode};
    use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
    use crossterm::{cursor, execute};
    use std::io::stdout;
    use std::time::{Duration, Instant};

    let mut player = VideoPlayer::new(path)?;

    // Enter raw mode and alternate screen
    terminal::enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;

    let mut renderer = TerminalRenderer::new()?;

    // Play frames
    let result = (|| -> Result<()> {
        while let Some(frame_result) = player.next_frame() {
            let (grid, delay) = frame_result?;

            // Render frame
            renderer.render(&grid)?;

            // Wait for frame duration, checking for keypress
            let deadline = Instant::now() + delay;
            while Instant::now() < deadline {
                // Check for keypress with short timeout
                if event::poll(Duration::from_millis(10))? {
                    if let Event::Key(key_event) = event::read()? {
                        // Stop on any key (except modifiers alone)
                        if !matches!(key_event.code, KeyCode::Modifier(_)) {
                            return Ok(());
                        }
                    }
                }
            }
        }

        // Video complete - wait for final keypress
        wait_for_key()?;
        Ok(())
    })();

    // Cleanup - always restore terminal state
    execute!(stdout, cursor::Show, LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;

    result
}

// ============================================================================
// Webcam Helper Functions (Story 9.6)
// ============================================================================

/// Displays the default webcam in the terminal.
///
/// This function:
/// 1. Opens the default system webcam
/// 2. Initializes the terminal (raw mode, alternate screen)
/// 3. Displays live feed at 30fps target until keypress
/// 4. Cleans up terminal state
///
/// # Errors
///
/// - `DotmaxError::CameraNotFound` - No webcam detected
/// - `DotmaxError::CameraInUse` - Camera is in use by another application
/// - `DotmaxError::CameraPermissionDenied` - No permission to access camera
/// - `DotmaxError::WebcamError` - Other webcam errors
/// - `DotmaxError::Terminal` - Terminal I/O errors
///
/// # Examples
///
/// ```no_run
/// use dotmax::quick;
///
/// // One-liner webcam display!
/// quick::show_webcam()?;
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
#[cfg(feature = "video")]
pub fn show_webcam() -> Result<()> {
    use crate::media::WebcamPlayer;
    play_webcam_internal(WebcamPlayer::new()?)
}

/// Displays a specific webcam in the terminal.
///
/// Accepts device selection by:
/// - Index: `show_webcam_device(0)` for first camera
/// - Path: `show_webcam_device("/dev/video1")` (Linux)
/// - Name: `show_webcam_device("FaceTime HD Camera")` (macOS/Windows)
///
/// # Arguments
///
/// * `device` - Device identifier (index, path, or name)
///
/// # Errors
///
/// - `DotmaxError::CameraNotFound` - Specified device not found
/// - `DotmaxError::CameraInUse` - Camera is in use
/// - `DotmaxError::CameraPermissionDenied` - No permission
/// - `DotmaxError::WebcamError` - Other webcam errors
///
/// # Examples
///
/// ```no_run
/// use dotmax::quick;
///
/// // By index
/// quick::show_webcam_device(0)?;
///
/// // By path (Linux)
/// quick::show_webcam_device("/dev/video1")?;
///
/// // By name (macOS/Windows)
/// quick::show_webcam_device("HD Pro Webcam C920")?;
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
#[cfg(feature = "video")]
pub fn show_webcam_device(device: impl Into<crate::media::WebcamDeviceId>) -> Result<()> {
    use crate::media::WebcamPlayer;
    play_webcam_internal(WebcamPlayer::from_device(device)?)
}

/// Internal function for webcam playback loop.
#[cfg(feature = "video")]
fn play_webcam_internal(mut player: crate::media::WebcamPlayer) -> Result<()> {
    use crate::media::MediaPlayer;
    use crossterm::event::{self, Event, KeyCode};
    use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
    use crossterm::{cursor, execute};
    use std::io::stdout;
    use std::time::{Duration, Instant};

    // Enter raw mode and alternate screen
    terminal::enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;

    let mut renderer = TerminalRenderer::new()?;

    // Play frames
    let result = (|| -> Result<()> {
        while let Some(frame_result) = player.next_frame() {
            let (grid, delay) = frame_result?;

            // Render frame
            renderer.render(&grid)?;

            // Wait for frame duration, checking for keypress and resize
            let deadline = Instant::now() + delay;
            while Instant::now() < deadline {
                // Check for events with short timeout
                if event::poll(Duration::from_millis(5))? {
                    match event::read()? {
                        Event::Key(key_event) => {
                            // Stop on any key (except modifiers alone)
                            if !matches!(key_event.code, KeyCode::Modifier(_)) {
                                return Ok(());
                            }
                        }
                        Event::Resize(w, h) => {
                            // Handle terminal resize
                            player.handle_resize(w as usize, h as usize);
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    })();

    // Cleanup - always restore terminal state
    execute!(stdout, cursor::Show, LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;

    result
}

// ============================================================================
// SVG Helper Functions
// ============================================================================

/// Shows an SVG file in the terminal.
#[cfg(all(feature = "image", feature = "svg"))]
fn show_svg(path: impl AsRef<std::path::Path>) -> Result<()> {
    let grid = load_svg(path)?;
    show(&grid)
}

/// Loads an SVG file into a BrailleGrid.
#[cfg(all(feature = "image", feature = "svg"))]
fn load_svg(path: impl AsRef<std::path::Path>) -> Result<BrailleGrid> {
    use crate::image::ImageRenderer;

    let (w, h) = terminal_size();
    // SVG rasterization needs target dimensions for proper resolution
    // Use dot dimensions for crisp rendering (2x4 dots per cell)
    let target_width = (w * 2) as u32;
    let target_height = (h * 4) as u32;

    ImageRenderer::new()
        .load_svg_from_path(path.as_ref(), target_width, target_height)?
        .resize(w, h, true)?
        .render()
}

// ============================================================================
// Tests (AC: #2, #3, #4, #5, #6, #8)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DotmaxError;

    // ========================================================================
    // Terminal Size Detection Tests
    // ========================================================================

    #[test]
    fn test_terminal_size_returns_reasonable_values() {
        let (w, h) = terminal_size();
        // Should return either actual terminal size or fallback
        assert!(w > 0, "Width should be positive");
        assert!(h > 0, "Height should be positive");
        // Fallback is 80x24, actual terminal might be larger
        assert!(
            w >= 80 || w > 0,
            "Width should be at least fallback or positive"
        );
        assert!(
            h >= 24 || h > 0,
            "Height should be at least fallback or positive"
        );
    }

    #[test]
    fn test_terminal_size_fallback_values() {
        // In CI/test environment without terminal, should get fallback
        // This test just verifies the function doesn't panic
        let (_w, _h) = terminal_size();
    }

    // ========================================================================
    // Grid Creation Tests
    // ========================================================================

    #[test]
    fn test_grid_creates_valid_grid() {
        let result = grid();
        assert!(result.is_ok(), "grid() should succeed");
        let g = result.unwrap();
        assert!(g.width() > 0, "Grid width should be positive");
        assert!(g.height() > 0, "Grid height should be positive");
    }

    #[test]
    fn test_grid_sized_creates_exact_dimensions() {
        let g = grid_sized(100, 50).unwrap();
        assert_eq!(g.width(), 100);
        assert_eq!(g.height(), 50);
    }

    #[test]
    fn test_grid_sized_zero_width_fails() {
        let result = grid_sized(0, 50);
        assert!(result.is_err());
        match result {
            Err(DotmaxError::InvalidDimensions { width, height }) => {
                assert_eq!(width, 0);
                assert_eq!(height, 50);
            }
            _ => panic!("Expected InvalidDimensions error"),
        }
    }

    #[test]
    fn test_grid_sized_zero_height_fails() {
        let result = grid_sized(100, 0);
        assert!(result.is_err());
        match result {
            Err(DotmaxError::InvalidDimensions { width, height }) => {
                assert_eq!(width, 100);
                assert_eq!(height, 0);
            }
            _ => panic!("Expected InvalidDimensions error"),
        }
    }

    // ========================================================================
    // Image Function Tests (feature-gated)
    // ========================================================================

    #[cfg(feature = "image")]
    mod image_tests {
        use super::*;
        use std::path::Path;

        #[test]
        fn test_load_image_sized_creates_correct_dimensions() {
            // Use a test image from fixtures
            let test_image = Path::new("tests/fixtures/images/sample.png");
            if test_image.exists() {
                let result = load_image_sized(test_image, 40, 20);
                assert!(
                    result.is_ok(),
                    "load_image_sized should succeed: {:?}",
                    result.err()
                );
                let g = result.unwrap();
                assert_eq!(g.width(), 40);
                assert_eq!(g.height(), 20);
            }
        }

        #[test]
        fn test_load_image_nonexistent_file_fails() {
            let result = load_image("nonexistent_image_12345.png");
            assert!(
                result.is_err(),
                "load_image should fail for nonexistent file"
            );
        }

        #[test]
        fn test_load_image_sized_nonexistent_file_fails() {
            let result = load_image_sized("nonexistent_image_12345.png", 100, 50);
            assert!(
                result.is_err(),
                "load_image_sized should fail for nonexistent file"
            );
        }
    }

    // ========================================================================
    // Error Handling Tests
    // ========================================================================

    #[test]
    fn test_grid_sized_exceeds_max_fails() {
        // Max is 10,000
        let result = grid_sized(10_001, 100);
        assert!(result.is_err(), "Should fail for width > 10000");
    }

    // Note: show() and show_image() require a terminal and are tested
    // via integration tests and examples rather than unit tests.
    // See tests/quick_test.rs and examples/quick_demo.rs

    // ========================================================================
    // Universal Media Function Tests (Story 9.1)
    // ========================================================================

    #[cfg(feature = "image")]
    mod media_tests {
        use super::*;
        use crate::DotmaxError;
        use std::path::Path;

        #[test]
        fn test_load_file_png_returns_static_content() {
            // Use test fixture if available
            let test_image = Path::new("tests/fixtures/images/sample.png");
            if test_image.exists() {
                let result = load_file(test_image);
                assert!(
                    result.is_ok(),
                    "load_file should succeed for PNG: {:?}",
                    result.err()
                );

                let content = result.unwrap();
                match content {
                    crate::media::MediaContent::Static(grid) => {
                        assert!(grid.width() > 0);
                        assert!(grid.height() > 0);
                    }
                    _ => panic!("Expected Static variant for PNG"),
                }
            }
        }

        #[test]
        fn test_load_file_nonexistent_fails() {
            let result = load_file("nonexistent_file_12345.png");
            assert!(
                result.is_err(),
                "load_file should fail for nonexistent file"
            );
        }

        #[test]
        fn test_load_file_unknown_format_returns_format_error() {
            // Create a temp file with unknown content
            use std::io::Write;
            let temp_dir = std::env::temp_dir();
            let temp_file = temp_dir.join("test_unknown.xyz");

            // Write some garbage bytes
            let mut file = std::fs::File::create(&temp_file).unwrap();
            file.write_all(&[0x00, 0x00, 0x00, 0x00]).unwrap();
            drop(file);

            let result = load_file(&temp_file);
            assert!(result.is_err(), "load_file should fail for unknown format");

            if let Err(DotmaxError::FormatError { format }) = result {
                assert!(
                    format.contains("unknown"),
                    "Error should mention unknown format"
                );
            } else {
                panic!("Expected FormatError for unknown format");
            }

            // Cleanup
            let _ = std::fs::remove_file(&temp_file);
        }

        #[test]
        fn test_show_file_nonexistent_fails() {
            let result = show_file("nonexistent_file_12345.png");
            assert!(
                result.is_err(),
                "show_file should fail for nonexistent file"
            );
        }
    }
}
