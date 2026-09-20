//! High-level animation loop abstraction for terminal animations.
//!
//! This module provides [`AnimationLoop`], a builder-pattern API that simplifies
//! creating terminal animations. It handles all the complexity of double-buffering,
//! frame timing, and terminal management automatically.
//!
//! # Overview
//!
//! `AnimationLoop` encapsulates:
//! - **Double-buffering**: Uses [`FrameBuffer`](super::FrameBuffer) internally for flicker-free updates
//! - **Frame timing**: Uses [`FrameTimer`](super::FrameTimer) to maintain consistent FPS
//! - **Terminal management**: Sets up raw mode, alternate screen, and cleanup
//! - **Graceful exit**: Handles Ctrl+C signal for clean shutdown
//!
//! # Example
//!
//! ```no_run
//! use dotmax::animation::AnimationLoop;
//!
//! // Create a simple bouncing dot animation
//! AnimationLoop::new(80, 24)
//!     .fps(30)
//!     .on_frame(|frame, buffer| {
//!         // Calculate bouncing position
//!         let x = (frame as usize * 2) % 160;
//!         let y = (48 - ((frame as i32 * 3) % 48).abs()) as usize;
//!         buffer.set_dot(x, y)?;
//!         Ok(true)  // Continue animation
//!     })
//!     .run()?;
//! # Ok::<(), dotmax::DotmaxError>(())
//! ```
//!
//! # Callback Return Values
//!
//! The frame callback returns `Result<bool, DotmaxError>`:
//! - `Ok(true)`: Continue animation to the next frame
//! - `Ok(false)`: Stop animation gracefully
//! - `Err(...)`: Stop animation with an error
//!
//! # Performance
//!
//! - Target 60fps with <10% single-core CPU usage
//! - Buffer swap is O(1) pointer exchange (~2.4ns)
//! - Frame timing uses efficient sleep-based rate limiting

use crate::animation::{FrameBuffer, FrameTimer};
use crate::error::DotmaxError;
use crate::grid::BrailleGrid;
use crate::render::TerminalRenderer;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{stdout, Write};
use std::time::Duration;
use tracing::{debug, info};

/// Minimum FPS value (cannot be below 1).
const MIN_FPS: u32 = 1;

/// Maximum FPS value (capped at 240 for practical limits).
const MAX_FPS: u32 = 240;

/// Default FPS when not specified.
const DEFAULT_FPS: u32 = 60;

/// High-level animation loop abstraction.
///
/// `AnimationLoop` provides a simple builder-pattern API for creating
/// terminal animations. It handles all the complexity of double-buffering,
/// frame timing, and terminal management automatically.
///
/// # Type Parameter
///
/// - `F`: The frame callback type, which must implement
///   `FnMut(u64, &mut BrailleGrid) -> Result<bool, DotmaxError>`
///
/// # Examples
///
/// Basic animation with default 60 FPS:
///
/// ```no_run
/// use dotmax::animation::AnimationLoop;
///
/// let mut frame_count = 0;
/// AnimationLoop::new(80, 24)
///     .on_frame(|frame, buffer| {
///         buffer.set_dot((frame as usize * 2) % 160, 48)?;
///         Ok(frame < 100)  // Run for 100 frames
///     })
///     .run()?;
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
///
/// Animation with custom FPS:
///
/// ```no_run
/// use dotmax::animation::AnimationLoop;
///
/// AnimationLoop::new(80, 24)
///     .fps(30)  // 30 FPS for less CPU usage
///     .on_frame(|frame, buffer| {
///         // Draw something each frame
///         Ok(true)
///     })
///     .run()?;
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
pub struct AnimationLoop<F>
where
    F: FnMut(u64, &mut BrailleGrid) -> Result<bool, DotmaxError>,
{
    /// Width in terminal cells (characters).
    width: usize,
    /// Height in terminal cells (lines).
    height: usize,
    /// Target frames per second (1-240).
    target_fps: u32,
    /// Frame callback function.
    on_frame: F,
}

/// Builder for constructing [`AnimationLoop`] instances.
///
/// Created by [`AnimationLoop::new()`] and configured with fluent methods
/// like [`fps()`](Self::fps). The builder is finalized by calling
/// [`on_frame()`](Self::on_frame) which returns an `AnimationLoop`.
///
/// # Examples
///
/// ```no_run
/// use dotmax::animation::AnimationLoop;
///
/// // Full builder chain
/// AnimationLoop::new(80, 24)
///     .fps(60)
///     .on_frame(|frame, buffer| {
///         // Draw frame content
///         Ok(true)
///     })
///     .run()?;
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
///
/// ```
/// use dotmax::animation::AnimationLoop;
///
/// // Check default FPS
/// let builder = AnimationLoop::new(80, 24);
/// // Default FPS is 60
/// ```
pub struct AnimationLoopBuilder {
    /// Width in terminal cells.
    width: usize,
    /// Height in terminal cells.
    height: usize,
    /// Target FPS (default 60).
    target_fps: u32,
}

// Convenience alias for AnimationLoop::new
impl AnimationLoop<fn(u64, &mut BrailleGrid) -> Result<bool, DotmaxError>> {
    /// Creates a new animation loop builder with the specified dimensions.
    ///
    /// The dimensions specify the size of the braille grid in terminal cells
    /// (characters wide × lines tall). Each cell contains an 8-dot braille
    /// character (2×4 dots), so a 80×24 grid provides 160×96 dot resolution.
    ///
    /// # Arguments
    ///
    /// * `width` - Width in terminal cells (characters)
    /// * `height` - Height in terminal cells (lines)
    ///
    /// # Returns
    ///
    /// An [`AnimationLoopBuilder`] with default settings (60 FPS).
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::AnimationLoop;
    ///
    /// // Standard terminal size (80×24)
    /// let builder = AnimationLoop::new(80, 24);
    ///
    /// // Larger buffer for detailed graphics
    /// let large = AnimationLoop::new(120, 40);
    /// ```
    #[must_use]
    #[allow(clippy::new_ret_no_self)]
    pub const fn new(width: usize, height: usize) -> AnimationLoopBuilder {
        AnimationLoopBuilder {
            width,
            height,
            target_fps: DEFAULT_FPS,
        }
    }
}

impl AnimationLoopBuilder {
    /// Sets the target frames per second.
    ///
    /// The FPS value is clamped to the valid range of 1-240. Values outside
    /// this range are silently corrected to the nearest valid value.
    ///
    /// Higher FPS provides smoother animation but uses more CPU. For most
    /// terminal animations, 30-60 FPS is sufficient.
    ///
    /// # Arguments
    ///
    /// * `fps` - Target frames per second (1-240, clamped if out of range)
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::AnimationLoop;
    ///
    /// // 30 FPS for lower CPU usage
    /// let builder = AnimationLoop::new(80, 24).fps(30);
    ///
    /// // 120 FPS for smooth motion
    /// let builder = AnimationLoop::new(80, 24).fps(120);
    ///
    /// // Values are clamped to valid range
    /// let builder = AnimationLoop::new(80, 24).fps(0);   // Becomes 1
    /// let builder = AnimationLoop::new(80, 24).fps(500); // Becomes 240
    /// ```
    #[must_use]
    pub fn fps(mut self, fps: u32) -> Self {
        self.target_fps = fps.clamp(MIN_FPS, MAX_FPS);
        self
    }

    /// Sets the frame callback and builds the [`AnimationLoop`].
    ///
    /// The callback is called once per frame with:
    /// - `frame`: Frame number starting at 0, incrementing each frame
    /// - `buffer`: Mutable reference to the back buffer ([`BrailleGrid`])
    ///
    /// The back buffer is cleared before each callback, so you only need
    /// to draw the current frame content.
    ///
    /// # Arguments
    ///
    /// * `callback` - Function called each frame to draw content
    ///
    /// # Returns
    ///
    /// An [`AnimationLoop`] ready to be run.
    ///
    /// # Callback Return Values
    ///
    /// - `Ok(true)`: Continue to the next frame
    /// - `Ok(false)`: Stop the animation gracefully
    /// - `Err(...)`: Stop with an error
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::animation::AnimationLoop;
    ///
    /// AnimationLoop::new(80, 24)
    ///     .fps(60)
    ///     .on_frame(|frame, buffer| {
    ///         // frame starts at 0 and increments each frame
    ///         let x = (frame as usize * 2) % 160;
    ///         buffer.set_dot(x, 48)?;
    ///
    ///         // Return false after 1000 frames to stop
    ///         Ok(frame < 1000)
    ///     })
    ///     .run()?;
    /// # Ok::<(), dotmax::DotmaxError>(())
    /// ```
    #[must_use]
    pub const fn on_frame<F>(self, callback: F) -> AnimationLoop<F>
    where
        F: FnMut(u64, &mut BrailleGrid) -> Result<bool, DotmaxError>,
    {
        AnimationLoop {
            width: self.width,
            height: self.height,
            target_fps: self.target_fps,
            on_frame: callback,
        }
    }
}

impl<F> AnimationLoop<F>
where
    F: FnMut(u64, &mut BrailleGrid) -> Result<bool, DotmaxError>,
{
    /// Runs the animation loop until stopped.
    ///
    /// This method blocks until:
    /// - The callback returns `Ok(false)`
    /// - The callback returns `Err(...)`
    /// - Ctrl+C is pressed
    ///
    /// The method handles all terminal setup (raw mode, alternate screen,
    /// cursor hiding) and cleanup automatically, even on error.
    ///
    /// # Returns
    ///
    /// - `Ok(())` on normal exit (callback returned false or Ctrl+C)
    /// - `Err(...)` if the callback returns an error or terminal I/O fails
    ///
    /// # Errors
    ///
    /// Returns [`DotmaxError::Terminal`] if:
    /// - Terminal initialization fails (raw mode, alternate screen)
    /// - Rendering to terminal fails during animation
    /// - Terminal cleanup fails on exit
    ///
    /// Returns the error from the callback if it returns `Err(...)`.
    ///
    /// # Terminal State
    ///
    /// On entry:
    /// - Enables raw mode for unbuffered input
    /// - Enters alternate screen to preserve original content
    /// - Hides cursor for clean animation
    ///
    /// On exit (any path):
    /// - Shows cursor
    /// - Leaves alternate screen
    /// - Disables raw mode
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::animation::AnimationLoop;
    ///
    /// AnimationLoop::new(80, 24)
    ///     .fps(60)
    ///     .on_frame(|frame, buffer| {
    ///         buffer.set_dot(frame as usize % 160, 48)?;
    ///         Ok(frame < 100)  // Stop after 100 frames
    ///     })
    ///     .run()?;
    /// # Ok::<(), dotmax::DotmaxError>(())
    /// ```
    pub fn run(&mut self) -> Result<(), DotmaxError> {
        info!(
            width = self.width,
            height = self.height,
            target_fps = self.target_fps,
            "Starting animation loop"
        );

        // Setup terminal
        let mut stdout = stdout();
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen, Hide)?;

        // Use a guard pattern to ensure cleanup on any exit path
        let result = self.run_inner();

        // Cleanup terminal (always runs)
        let cleanup_result = Self::cleanup_terminal(&mut stdout);

        // Return first error if any
        result.and(cleanup_result)
    }

    /// Inner animation loop, separated for cleanup guard pattern.
    fn run_inner(&mut self) -> Result<(), DotmaxError> {
        // Create animation infrastructure
        let mut frame_buffer = FrameBuffer::new(self.width, self.height);
        let mut frame_timer = FrameTimer::new(self.target_fps);
        let mut renderer = TerminalRenderer::new()?;
        let mut frame_num: u64 = 0;

        debug!(
            width = self.width,
            height = self.height,
            target_fps = self.target_fps,
            "Animation infrastructure initialized"
        );

        loop {
            // Check for Ctrl+C with non-blocking poll
            if event::poll(Duration::ZERO)? {
                if let Event::Key(key) = event::read()? {
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        info!("Ctrl+C detected, stopping animation gracefully");
                        break;
                    }
                    // Also allow 'q' to quit for convenience
                    if key.code == KeyCode::Char('q') {
                        debug!("'q' pressed, stopping animation");
                        break;
                    }
                }
            }

            // Clear back buffer before each frame
            frame_buffer.get_back_buffer().clear();

            // Call user's frame callback
            let should_continue = (self.on_frame)(frame_num, frame_buffer.get_back_buffer())?;

            if !should_continue {
                debug!(frame = frame_num, "Callback returned false, stopping");
                break;
            }

            // Swap buffers (O(1) pointer swap)
            frame_buffer.swap_buffers();

            // Render front buffer to terminal
            frame_buffer.render(&mut renderer)?;

            // Wait for next frame timing
            frame_timer.wait_for_next_frame();

            frame_num += 1;
        }

        info!(
            total_frames = frame_num,
            actual_fps = frame_timer.actual_fps(),
            "Animation completed"
        );

        Ok(())
    }

    /// Cleanup terminal state.
    fn cleanup_terminal(stdout: &mut std::io::Stdout) -> Result<(), DotmaxError> {
        // Show cursor, leave alternate screen, disable raw mode
        execute!(stdout, Show, LeaveAlternateScreen)?;
        disable_raw_mode()?;
        stdout.flush()?;
        debug!("Terminal state restored");
        Ok(())
    }

    /// Returns the animation width in terminal cells.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::AnimationLoop;
    ///
    /// let anim = AnimationLoop::new(80, 24)
    ///     .on_frame(|_, _| Ok(false));
    /// assert_eq!(anim.width(), 80);
    /// ```
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Returns the animation height in terminal cells.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::AnimationLoop;
    ///
    /// let anim = AnimationLoop::new(80, 24)
    ///     .on_frame(|_, _| Ok(false));
    /// assert_eq!(anim.height(), 24);
    /// ```
    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    /// Returns the target FPS for this animation.
    ///
    /// This is the FPS set via the builder, not the actual achieved FPS
    /// (which depends on system performance).
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::AnimationLoop;
    ///
    /// // Default FPS is 60
    /// let anim = AnimationLoop::new(80, 24)
    ///     .on_frame(|_, _| Ok(false));
    /// assert_eq!(anim.target_fps(), 60);
    ///
    /// // Custom FPS
    /// let anim = AnimationLoop::new(80, 24)
    ///     .fps(30)
    ///     .on_frame(|_, _| Ok(false));
    /// assert_eq!(anim.target_fps(), 30);
    /// ```
    #[must_use]
    pub const fn target_fps(&self) -> u32 {
        self.target_fps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // AC #1: Builder Pattern API
    // ========================================================================

    #[test]
    fn test_builder_creates_with_correct_dimensions() {
        let anim = AnimationLoop::new(80, 24).on_frame(|_, _| Ok(false));

        assert_eq!(anim.width(), 80);
        assert_eq!(anim.height(), 24);
    }

    #[test]
    fn test_builder_default_fps_is_60() {
        let anim = AnimationLoop::new(80, 24).on_frame(|_, _| Ok(false));

        assert_eq!(anim.target_fps(), 60);
    }

    #[test]
    fn test_builder_custom_fps() {
        let anim = AnimationLoop::new(80, 24)
            .fps(30)
            .on_frame(|_, _| Ok(false));

        assert_eq!(anim.target_fps(), 30);
    }

    #[test]
    fn test_builder_fps_clamping_below_min() {
        let anim = AnimationLoop::new(80, 24).fps(0).on_frame(|_, _| Ok(false));

        assert_eq!(anim.target_fps(), 1, "FPS 0 should be clamped to 1");
    }

    #[test]
    fn test_builder_fps_clamping_above_max() {
        let anim = AnimationLoop::new(80, 24)
            .fps(1000)
            .on_frame(|_, _| Ok(false));

        assert_eq!(anim.target_fps(), 240, "FPS 1000 should be clamped to 240");
    }

    #[test]
    fn test_builder_fps_at_min_boundary() {
        let anim = AnimationLoop::new(80, 24).fps(1).on_frame(|_, _| Ok(false));

        assert_eq!(anim.target_fps(), 1, "FPS 1 should remain 1");
    }

    #[test]
    fn test_builder_fps_at_max_boundary() {
        let anim = AnimationLoop::new(80, 24)
            .fps(240)
            .on_frame(|_, _| Ok(false));

        assert_eq!(anim.target_fps(), 240, "FPS 240 should remain 240");
    }

    // ========================================================================
    // AC #2: Callback Receives Frame Number and Mutable Back Buffer
    // ========================================================================

    #[test]
    fn test_callback_receives_frame_numbers_in_sequence() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let frames_received = Rc::new(RefCell::new(Vec::new()));
        let frames_clone = frames_received.clone();

        let mut anim = AnimationLoop::new(10, 10).fps(1).on_frame(move |frame, _| {
            frames_clone.borrow_mut().push(frame);
            Ok(frame < 4) // Run for 5 frames (0, 1, 2, 3, 4)
        });

        // Note: We can't actually run() in tests without a terminal
        // But we can verify the callback setup works

        // Simulate callback calls
        let mut buffer = BrailleGrid::new(10, 10).unwrap();
        for i in 0..5 {
            let _ = (anim.on_frame)(i, &mut buffer);
        }

        let frames = frames_received.borrow();
        assert_eq!(*frames, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_callback_returning_false_indicates_stop() {
        let mut anim = AnimationLoop::new(10, 10).on_frame(|frame, _| Ok(frame < 5));

        let mut buffer = BrailleGrid::new(10, 10).unwrap();

        // Frames 0-4 should return true
        for i in 0..5 {
            let result = (anim.on_frame)(i, &mut buffer);
            assert!(result.unwrap(), "Frame {} should return true", i);
        }

        // Frame 5 should return false (stop)
        let result = (anim.on_frame)(5, &mut buffer);
        assert!(!result.unwrap(), "Frame 5 should return false");
    }

    // ========================================================================
    // AC #4: Loop Handles Timing Automatically
    // ========================================================================

    #[test]
    fn test_accessor_methods_return_correct_values() {
        let anim = AnimationLoop::new(100, 50)
            .fps(120)
            .on_frame(|_, _| Ok(false));

        assert_eq!(anim.width(), 100);
        assert_eq!(anim.height(), 50);
        assert_eq!(anim.target_fps(), 120);
    }

    // ========================================================================
    // Additional Unit Tests
    // ========================================================================

    #[test]
    fn test_small_dimensions() {
        let anim = AnimationLoop::new(1, 1).on_frame(|_, _| Ok(false));

        assert_eq!(anim.width(), 1);
        assert_eq!(anim.height(), 1);
    }

    #[test]
    fn test_large_dimensions() {
        let anim = AnimationLoop::new(200, 100).on_frame(|_, _| Ok(false));

        assert_eq!(anim.width(), 200);
        assert_eq!(anim.height(), 100);
    }

    #[test]
    fn test_builder_method_chaining() {
        // Verify builder pattern works correctly
        let anim = AnimationLoop::new(80, 24)
            .fps(30)
            .on_frame(|_, _| Ok(false));

        assert_eq!(anim.width(), 80);
        assert_eq!(anim.height(), 24);
        assert_eq!(anim.target_fps(), 30);
    }
}
