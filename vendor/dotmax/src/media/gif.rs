//! Animated GIF playback support.
//!
//! This module provides [`GifPlayer`] for animated GIF playback, implementing
//! the [`MediaPlayer`] trait for integration with the universal media system.
//!
//! # Features
//!
//! - Frame-by-frame GIF decoding with correct timing
//! - Loop count handling (finite and infinite loops)
//! - Disposal method support for correct frame rendering
//! - Memory-efficient streaming decode (frames processed one at a time)
//!
//! # Examples
//!
//! ## Basic Playback
//!
//! ```no_run
//! use dotmax::media::{GifPlayer, MediaPlayer};
//! use std::time::Duration;
//!
//! let mut player = GifPlayer::new("animation.gif")?;
//! while let Some(result) = player.next_frame() {
//!     let (grid, delay) = result?;
//!     // Render grid and wait for delay
//! }
//! # Ok::<(), dotmax::DotmaxError>(())
//! ```
//!
//! ## Using as MediaPlayer Trait Object
//!
//! ```no_run
//! use dotmax::media::{GifPlayer, MediaPlayer};
//!
//! let player: Box<dyn MediaPlayer> = Box::new(GifPlayer::new("animation.gif")?);
//! println!("Frame count: {:?}", player.frame_count());
//! # Ok::<(), dotmax::DotmaxError>(())
//! ```

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::image::ImageRenderer;
use crate::{BrailleGrid, DotmaxError, Result};

use super::MediaPlayer;

// ============================================================================
// GIF Frame Types (AC: #2, #5)
// ============================================================================

/// Disposal method for GIF frames.
///
/// Disposal methods determine what happens to the frame area after the frame
/// is displayed and before the next frame is rendered. This is critical for
/// correct animated GIF rendering.
///
/// # GIF Specification Reference
///
/// | Value | Method | Description |
/// |-------|--------|-------------|
/// | 0 | None | No disposal specified (treat as DoNotDispose) |
/// | 1 | DoNotDispose | Leave frame in place |
/// | 2 | RestoreBackground | Clear frame area to background color |
/// | 3 | RestorePrevious | Restore canvas to state before frame |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisposalMethod {
    /// No disposal specified - leave frame in place (default).
    #[default]
    None,

    /// Do not dispose - leave frame in place.
    ///
    /// The frame remains visible and subsequent frames are drawn over it.
    DoNotDispose,

    /// Restore to background color.
    ///
    /// The frame area is cleared to the background color after display.
    RestoreBackground,

    /// Restore to previous frame state.
    ///
    /// The canvas is restored to the state before this frame was drawn.
    RestorePrevious,
}

impl From<gif::DisposalMethod> for DisposalMethod {
    fn from(method: gif::DisposalMethod) -> Self {
        match method {
            gif::DisposalMethod::Any => Self::None,
            gif::DisposalMethod::Keep => Self::DoNotDispose,
            gif::DisposalMethod::Background => Self::RestoreBackground,
            gif::DisposalMethod::Previous => Self::RestorePrevious,
        }
    }
}

/// A single frame from an animated GIF.
///
/// Contains the decoded RGBA pixel data along with timing and disposal metadata.
#[derive(Debug, Clone)]
pub struct GifFrame {
    /// RGBA pixel data for this frame.
    pub pixels: Vec<u8>,

    /// Frame width in pixels.
    pub width: u32,

    /// Frame height in pixels.
    pub height: u32,

    /// Frame delay in milliseconds.
    ///
    /// GIF spec uses centiseconds (1/100th of a second), which is converted
    /// to milliseconds here. Default delay (0) is interpreted as 100ms per spec.
    pub delay_ms: u32,

    /// Frame index (0-based).
    pub index: usize,

    /// Disposal method for this frame.
    pub disposal: DisposalMethod,

    /// Frame position (left offset from canvas origin).
    pub left: u16,

    /// Frame position (top offset from canvas origin).
    pub top: u16,
}

// ============================================================================
// GifPlayer (AC: #3, #4, #9)
// ============================================================================

/// Animated GIF player implementing the [`MediaPlayer`] trait.
///
/// `GifPlayer` provides frame-by-frame access to animated GIFs with correct
/// timing, loop handling, and disposal method support.
///
/// # Frame Iteration
///
/// Use [`next_frame()`](Self::next_frame) to get frames one at a time:
///
/// ```no_run
/// use dotmax::media::{GifPlayer, MediaPlayer};
///
/// let mut player = GifPlayer::new("animation.gif")?;
/// while let Some(result) = player.next_frame() {
///     let (grid, delay) = result?;
///     // Display grid, then sleep for delay
///     std::thread::sleep(delay);
/// }
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
///
/// # Loop Handling
///
/// GIF loop count is respected:
/// - `loop_count() == Some(0)` → infinite loop
/// - `loop_count() == Some(n)` → play n times
/// - After all loops complete, `next_frame()` returns `None`
///
/// Use [`reset()`](Self::reset) to restart from frame 0.
///
/// # Memory Efficiency
///
/// Frames are decoded on-demand, not all at once. The player maintains
/// a canvas buffer for disposal method handling, but individual frames
/// are not cached.
///
/// # Thread Safety
///
/// `GifPlayer` implements `Send` and can be moved across threads.
pub struct GifPlayer {
    /// Path to the GIF file (for reset support).
    path: PathBuf,

    /// GIF decoder (streaming).
    decoder: gif::Decoder<BufReader<File>>,

    /// Canvas dimensions (from GIF global header).
    canvas_width: u16,
    canvas_height: u16,

    /// Current canvas state (RGBA pixels).
    /// Used for disposal method handling.
    canvas: Vec<u8>,

    /// Previous canvas state (for RestorePrevious disposal).
    previous_canvas: Vec<u8>,

    /// Total frame count (populated after first full iteration).
    frame_count: Option<usize>,

    /// Loop count from NETSCAPE extension.
    /// - `Some(0)` → infinite
    /// - `Some(n)` → play n times
    /// - `None` → play once (no extension)
    gif_loop_count: Option<u16>,

    /// Current frame index (0-based).
    current_frame: usize,

    /// Current loop iteration (1-based).
    current_loop: u16,

    /// Whether we've completed all loops.
    loops_completed: bool,

    /// Previous frame's disposal method.
    previous_disposal: DisposalMethod,

    /// Previous frame's rectangle (for disposal).
    previous_rect: (u16, u16, u16, u16), // (left, top, width, height)

    /// Terminal dimensions for rendering.
    terminal_width: usize,
    terminal_height: usize,
}

impl std::fmt::Debug for GifPlayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GifPlayer")
            .field("path", &self.path)
            .field("canvas_width", &self.canvas_width)
            .field("canvas_height", &self.canvas_height)
            .field("frame_count", &self.frame_count)
            .field("loop_count", &self.gif_loop_count)
            .field("current_frame", &self.current_frame)
            .field("current_loop", &self.current_loop)
            .finish_non_exhaustive()
    }
}

impl GifPlayer {
    /// Creates a new `GifPlayer` from a GIF file path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the animated GIF file
    ///
    /// # Errors
    ///
    /// Returns `DotmaxError::GifError` if the file cannot be opened or is
    /// not a valid GIF.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::media::GifPlayer;
    ///
    /// let player = GifPlayer::new("animation.gif")?;
    /// println!("Canvas: {}x{}", player.canvas_width(), player.canvas_height());
    /// # Ok::<(), dotmax::DotmaxError>(())
    /// ```
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path)?;
        let reader = BufReader::new(file);

        let mut options = gif::DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::RGBA);

        let decoder = options
            .read_info(reader)
            .map_err(|e| DotmaxError::GifError {
                path: path.clone(),
                message: format!("Failed to decode GIF: {e}"),
            })?;

        let canvas_width = decoder.width();
        let canvas_height = decoder.height();

        // Get loop count from NETSCAPE extension
        let gif_loop_count = match decoder.repeat() {
            gif::Repeat::Infinite => Some(0),
            gif::Repeat::Finite(n) => Some(n),
        };

        // Initialize canvas buffer (RGBA)
        let canvas_size = (canvas_width as usize) * (canvas_height as usize) * 4;
        let canvas = vec![0u8; canvas_size];
        let previous_canvas = vec![0u8; canvas_size];

        // Get terminal size for rendering
        let (terminal_width, terminal_height) = crossterm::terminal::size()
            .map(|(w, h)| (w as usize, h as usize))
            .unwrap_or((80, 24));

        Ok(Self {
            path,
            decoder,
            canvas_width,
            canvas_height,
            canvas,
            previous_canvas,
            frame_count: None,
            gif_loop_count,
            current_frame: 0,
            current_loop: 1,
            loops_completed: false,
            previous_disposal: DisposalMethod::None,
            previous_rect: (0, 0, 0, 0),
            terminal_width,
            terminal_height,
        })
    }

    /// Returns the canvas width in pixels.
    #[must_use]
    pub const fn canvas_width(&self) -> u16 {
        self.canvas_width
    }

    /// Returns the canvas height in pixels.
    #[must_use]
    pub const fn canvas_height(&self) -> u16 {
        self.canvas_height
    }

    /// Decodes the next frame from the GIF.
    ///
    /// Returns `None` if no more frames are available.
    fn decode_next_frame(&mut self) -> Option<Result<GifFrame>> {
        // Read next frame from decoder
        let frame = match self.decoder.read_next_frame() {
            Ok(Some(f)) => f,
            Ok(None) => return None,
            Err(e) => {
                tracing::warn!(
                    "GIF frame decode error at frame {}: {:?}",
                    self.current_frame,
                    e
                );
                // Try to continue with next frame
                return Some(Err(DotmaxError::GifError {
                    path: self.path.clone(),
                    message: format!("Frame {} decode error: {e}", self.current_frame),
                }));
            }
        };

        // Convert delay from centiseconds to milliseconds
        // Default delay of 0 is interpreted as 100ms per GIF spec
        let delay_ms = if frame.delay == 0 {
            100
        } else {
            u32::from(frame.delay) * 10
        };

        let gif_frame = GifFrame {
            pixels: frame.buffer.to_vec(),
            width: u32::from(frame.width),
            height: u32::from(frame.height),
            delay_ms,
            index: self.current_frame,
            disposal: frame.dispose.into(),
            left: frame.left,
            top: frame.top,
        };

        Some(Ok(gif_frame))
    }

    /// Applies the previous frame's disposal method before drawing new frame.
    fn apply_previous_disposal(&mut self) {
        let (left, top, width, height) = self.previous_rect;
        if width == 0 || height == 0 {
            return;
        }

        match self.previous_disposal {
            DisposalMethod::None | DisposalMethod::DoNotDispose => {
                // Leave frame in place - nothing to do
            }
            DisposalMethod::RestoreBackground => {
                // Clear the previous frame area to transparent black
                for y in 0..height {
                    let canvas_y = (top + y) as usize;
                    if canvas_y >= self.canvas_height as usize {
                        continue;
                    }
                    for x in 0..width {
                        let canvas_x = (left + x) as usize;
                        if canvas_x >= self.canvas_width as usize {
                            continue;
                        }
                        let idx = (canvas_y * self.canvas_width as usize + canvas_x) * 4;
                        if idx + 3 < self.canvas.len() {
                            self.canvas[idx] = 0; // R
                            self.canvas[idx + 1] = 0; // G
                            self.canvas[idx + 2] = 0; // B
                            self.canvas[idx + 3] = 0; // A
                        }
                    }
                }
            }
            DisposalMethod::RestorePrevious => {
                // Restore canvas to previous state
                self.canvas.copy_from_slice(&self.previous_canvas);
            }
        }
    }

    /// Saves current canvas state for RestorePrevious disposal.
    fn save_canvas_state(&mut self) {
        self.previous_canvas.copy_from_slice(&self.canvas);
    }

    /// Composites a frame onto the canvas.
    fn composite_frame(&mut self, frame: &GifFrame) {
        let frame_width = frame.width as usize;
        let frame_height = frame.height as usize;
        let canvas_width = self.canvas_width as usize;

        for y in 0..frame_height {
            let canvas_y = (frame.top as usize) + y;
            if canvas_y >= self.canvas_height as usize {
                continue;
            }
            for x in 0..frame_width {
                let canvas_x = (frame.left as usize) + x;
                if canvas_x >= canvas_width {
                    continue;
                }

                let frame_idx = (y * frame_width + x) * 4;
                let canvas_idx = (canvas_y * canvas_width + canvas_x) * 4;

                if frame_idx + 3 < frame.pixels.len() && canvas_idx + 3 < self.canvas.len() {
                    let alpha = frame.pixels[frame_idx + 3];

                    if alpha == 255 {
                        // Fully opaque - direct copy
                        self.canvas[canvas_idx] = frame.pixels[frame_idx];
                        self.canvas[canvas_idx + 1] = frame.pixels[frame_idx + 1];
                        self.canvas[canvas_idx + 2] = frame.pixels[frame_idx + 2];
                        self.canvas[canvas_idx + 3] = 255;
                    } else if alpha > 0 {
                        // Alpha blend
                        let src_a = f32::from(alpha) / 255.0;
                        let dst_a = f32::from(self.canvas[canvas_idx + 3]) / 255.0;
                        let out_a = src_a + dst_a * (1.0 - src_a);

                        if out_a > 0.0 {
                            for i in 0..3 {
                                let src = f32::from(frame.pixels[frame_idx + i]);
                                let dst = f32::from(self.canvas[canvas_idx + i]);
                                let blended =
                                    src.mul_add(src_a, dst * dst_a * (1.0 - src_a)) / out_a;
                                self.canvas[canvas_idx + i] = blended as u8;
                            }
                            self.canvas[canvas_idx + 3] = (out_a * 255.0) as u8;
                        }
                    }
                    // alpha == 0: transparent, leave canvas pixel unchanged
                }
            }
        }
    }

    /// Converts the current canvas to a BrailleGrid.
    fn canvas_to_grid(&self) -> Result<BrailleGrid> {
        // Create RGBA image from canvas
        let img = image::RgbaImage::from_raw(
            u32::from(self.canvas_width),
            u32::from(self.canvas_height),
            self.canvas.clone(),
        )
        .ok_or_else(|| DotmaxError::GifError {
            path: self.path.clone(),
            message: "Failed to create image from canvas".to_string(),
        })?;

        // Use ImageRenderer to convert to BrailleGrid
        let grid = ImageRenderer::new()
            .load_from_rgba(img)
            .resize(self.terminal_width, self.terminal_height, true)?
            .render()?;

        Ok(grid)
    }

    /// Reopens the GIF file and resets decoder state.
    fn reopen_decoder(&mut self) -> Result<()> {
        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);

        let mut options = gif::DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::RGBA);

        self.decoder = options
            .read_info(reader)
            .map_err(|e| DotmaxError::GifError {
                path: self.path.clone(),
                message: format!("Failed to reopen GIF: {e}"),
            })?;

        Ok(())
    }
}

impl MediaPlayer for GifPlayer {
    /// Returns the next frame and its display duration.
    ///
    /// Handles loop iteration automatically. Returns `None` when all loops
    /// are complete or no more frames are available.
    fn next_frame(&mut self) -> Option<Result<(BrailleGrid, Duration)>> {
        if self.loops_completed {
            return None;
        }

        // Apply previous frame's disposal method
        self.apply_previous_disposal();

        // If RestorePrevious is needed for next frame, save state now
        // (before compositing this frame)
        // We'll check the current frame's disposal after decoding

        // Decode next frame
        let frame = match self.decode_next_frame() {
            Some(Ok(f)) => f,
            Some(Err(e)) => {
                // Log error and try to continue
                tracing::warn!("Skipping corrupted frame {}: {:?}", self.current_frame, e);
                self.current_frame += 1;
                // Recursively try next frame
                return self.next_frame();
            }
            None => {
                // No more frames - handle loop
                if let Some(frame_count) = self.frame_count {
                    // We know the frame count
                    tracing::debug!(
                        "Loop {} complete ({} frames)",
                        self.current_loop,
                        frame_count
                    );
                } else {
                    // First complete iteration - save frame count
                    self.frame_count = Some(self.current_frame);
                    tracing::debug!("First loop complete, {} frames", self.current_frame);
                }

                // Check if we should loop
                let should_loop = match self.gif_loop_count {
                    Some(0) => true, // Infinite loop
                    Some(n) if self.current_loop < n => true,
                    _ => false,
                };

                if should_loop {
                    // Reset for next loop
                    self.current_loop += 1;
                    self.current_frame = 0;

                    // Clear canvas for fresh start
                    self.canvas.fill(0);
                    self.previous_canvas.fill(0);
                    self.previous_disposal = DisposalMethod::None;
                    self.previous_rect = (0, 0, 0, 0);

                    // Reopen decoder to restart from beginning
                    if let Err(e) = self.reopen_decoder() {
                        return Some(Err(e));
                    }

                    // Recursively get first frame of new loop
                    return self.next_frame();
                }

                // All loops complete
                self.loops_completed = true;
                return None;
            }
        };

        // Save canvas state if this frame has RestorePrevious disposal
        if frame.disposal == DisposalMethod::RestorePrevious {
            self.save_canvas_state();
        }

        // Composite frame onto canvas
        self.composite_frame(&frame);

        // Convert canvas to BrailleGrid
        let grid = match self.canvas_to_grid() {
            Ok(g) => g,
            Err(e) => return Some(Err(e)),
        };

        // Store disposal info for next frame
        self.previous_disposal = frame.disposal;
        self.previous_rect = (
            frame.left,
            frame.top,
            frame.width as u16,
            frame.height as u16,
        );
        self.current_frame += 1;

        let duration = Duration::from_millis(u64::from(frame.delay_ms));
        Some(Ok((grid, duration)))
    }

    /// Resets playback to the first frame.
    fn reset(&mut self) {
        self.current_frame = 0;
        self.current_loop = 1;
        self.loops_completed = false;
        self.canvas.fill(0);
        self.previous_canvas.fill(0);
        self.previous_disposal = DisposalMethod::None;
        self.previous_rect = (0, 0, 0, 0);

        // Reopen decoder
        if let Err(e) = self.reopen_decoder() {
            tracing::warn!("Failed to reset GIF decoder: {:?}", e);
        }
    }

    /// Returns the total number of frames, if known.
    ///
    /// Returns `None` until the first complete iteration.
    fn frame_count(&self) -> Option<usize> {
        self.frame_count
    }

    /// Returns the GIF's loop count.
    ///
    /// - `Some(0)` → infinite looping
    /// - `Some(n)` → loop n times
    /// - `None` → play once
    fn loop_count(&self) -> Option<u16> {
        self.gif_loop_count
    }

    /// Updates terminal dimensions for subsequent frame rendering.
    ///
    /// Call this when the terminal is resized to ensure frames are
    /// rendered at the correct size.
    fn handle_resize(&mut self, width: usize, height: usize) {
        self.terminal_width = width;
        self.terminal_height = height;
        tracing::debug!("GifPlayer resized to {}x{}", width, height);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Ensure GifPlayer is Send (required by MediaPlayer trait)
    fn _assert_gif_player_is_send<T: Send>() {}
    fn _assert_gif_player_send() {
        // This function is never called - it just needs to compile
        // to verify GifPlayer implements Send
        fn assert_send<T: Send>() {}
        assert_send::<GifPlayer>();
    }

    #[test]
    fn test_disposal_method_from_gif() {
        assert_eq!(
            DisposalMethod::from(gif::DisposalMethod::Any),
            DisposalMethod::None
        );
        assert_eq!(
            DisposalMethod::from(gif::DisposalMethod::Keep),
            DisposalMethod::DoNotDispose
        );
        assert_eq!(
            DisposalMethod::from(gif::DisposalMethod::Background),
            DisposalMethod::RestoreBackground
        );
        assert_eq!(
            DisposalMethod::from(gif::DisposalMethod::Previous),
            DisposalMethod::RestorePrevious
        );
    }

    #[test]
    fn test_disposal_method_default() {
        assert_eq!(DisposalMethod::default(), DisposalMethod::None);
    }

    #[test]
    fn test_gif_frame_debug() {
        let frame = GifFrame {
            pixels: vec![0; 16],
            width: 2,
            height: 2,
            delay_ms: 100,
            index: 0,
            disposal: DisposalMethod::None,
            left: 0,
            top: 0,
        };
        let debug_str = format!("{:?}", frame);
        assert!(debug_str.contains("GifFrame"));
        assert!(debug_str.contains("delay_ms: 100"));
    }

    #[test]
    fn test_gif_player_new_animated() {
        use std::path::Path;
        let path = Path::new("tests/fixtures/media/animated.gif");
        if path.exists() {
            let player = GifPlayer::new(path);
            assert!(
                player.is_ok(),
                "Should load animated GIF: {:?}",
                player.err()
            );
            let player = player.unwrap();
            assert_eq!(player.canvas_width(), 10);
            assert_eq!(player.canvas_height(), 10);
            // Infinite loop
            assert_eq!(player.loop_count(), Some(0));
        }
    }

    #[test]
    fn test_gif_player_new_static() {
        use std::path::Path;
        let path = Path::new("tests/fixtures/media/static.gif");
        if path.exists() {
            let player = GifPlayer::new(path);
            assert!(player.is_ok(), "Should load static GIF: {:?}", player.err());
        }
    }

    #[test]
    fn test_gif_player_new_nonexistent() {
        let player = GifPlayer::new("nonexistent.gif");
        assert!(player.is_err(), "Should fail for nonexistent file");
    }

    #[test]
    fn test_gif_player_next_frame() {
        use std::path::Path;
        let path = Path::new("tests/fixtures/media/animated.gif");
        if path.exists() {
            let mut player = GifPlayer::new(path).unwrap();

            // Get first frame
            let frame1 = player.next_frame();
            assert!(frame1.is_some(), "Should have first frame");
            let (grid, delay) = frame1.unwrap().unwrap();
            assert!(grid.width() > 0);
            assert!(grid.height() > 0);
            assert_eq!(delay.as_millis(), 100); // 10 centiseconds = 100ms
        }
    }

    #[test]
    fn test_gif_player_loop_twice() {
        use std::path::Path;
        let path = Path::new("tests/fixtures/media/loop_twice.gif");
        if path.exists() {
            let player = GifPlayer::new(path).unwrap();
            assert_eq!(player.loop_count(), Some(2), "Should loop twice");
        }
    }

    #[test]
    fn test_gif_player_reset() {
        use std::path::Path;
        let path = Path::new("tests/fixtures/media/animated.gif");
        if path.exists() {
            let mut player = GifPlayer::new(path).unwrap();

            // Consume some frames
            let _ = player.next_frame();
            let _ = player.next_frame();

            // Reset
            player.reset();

            // Should get first frame again
            let frame = player.next_frame();
            assert!(frame.is_some(), "Should have frame after reset");
        }
    }

    #[test]
    fn test_gif_player_frame_delay() {
        use std::path::Path;
        let path = Path::new("tests/fixtures/media/animated.gif");
        if path.exists() {
            let mut player = GifPlayer::new(path).unwrap();

            // animated.gif has delay=10 (100ms)
            let (_, delay) = player.next_frame().unwrap().unwrap();
            assert_eq!(delay.as_millis(), 100, "Delay should be 100ms");
        }
    }

    #[test]
    fn test_gif_player_debug() {
        use std::path::Path;
        let path = Path::new("tests/fixtures/media/animated.gif");
        if path.exists() {
            let player = GifPlayer::new(path).unwrap();
            let debug_str = format!("{:?}", player);
            assert!(debug_str.contains("GifPlayer"));
            assert!(debug_str.contains("canvas_width"));
        }
    }

    #[test]
    fn test_gif_player_as_media_player_trait_object() {
        use std::path::Path;
        let path = Path::new("tests/fixtures/media/animated.gif");
        if path.exists() {
            let player = GifPlayer::new(path).unwrap();
            // This compiles = GifPlayer implements MediaPlayer
            let _trait_obj: Box<dyn MediaPlayer> = Box::new(player);
        }
    }
}
