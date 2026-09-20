//! Double-buffered frame management for flicker-free animation.
//!
//! This module provides [`FrameBuffer`], a double-buffering implementation that
//! enables smooth terminal animations without tearing or flickering.

use crate::error::DotmaxError;
use crate::grid::BrailleGrid;
use crate::render::TerminalRenderer;

/// Double-buffered frame management for flicker-free animation.
///
/// `FrameBuffer` maintains two [`BrailleGrid`] buffers:
/// - **Front buffer**: The currently displayed frame (rendered to terminal)
/// - **Back buffer**: The next frame being prepared (where you draw)
///
/// This separation ensures users never see partially drawn frames, eliminating
/// visual tearing and flickering common in terminal graphics.
///
/// # Performance
///
/// Buffer swapping is an O(1) pointer swap operation (<1ms), not a data copy.
/// This makes it suitable for high frame-rate animations (60+ fps).
///
/// # Examples
///
/// Basic double-buffering workflow:
///
/// ```
/// use dotmax::animation::FrameBuffer;
///
/// // Create a double-buffered frame system (80x24 terminal cells)
/// let mut buffer = FrameBuffer::new(80, 24);
///
/// // Draw to the back buffer
/// {
///     let back = buffer.get_back_buffer();
///     back.clear();
///     back.set_dot(10, 10).unwrap();
///     back.set_dot(11, 10).unwrap();
/// }
///
/// // Swap buffers - the back buffer becomes the new front
/// buffer.swap_buffers();
///
/// // Now the front buffer contains what we drew
/// assert!(buffer.get_front_buffer().get_dot(10 / 2, 10 / 4, 0).is_ok());
/// ```
///
/// Animation loop pattern:
///
/// ```no_run
/// use dotmax::animation::FrameBuffer;
/// use dotmax::TerminalRenderer;
///
/// let mut buffer = FrameBuffer::new(80, 24);
/// let mut renderer = TerminalRenderer::new().unwrap();
///
/// loop {
///     // 1. Clear back buffer
///     buffer.get_back_buffer().clear();
///
///     // 2. Draw next frame
///     buffer.get_back_buffer().set_dot(10, 10).unwrap();
///
///     // 3. Swap buffers (instant)
///     buffer.swap_buffers();
///
///     // 4. Render to terminal
///     buffer.render(&mut renderer).unwrap();
///
///     // 5. Wait for next frame timing
///     std::thread::sleep(std::time::Duration::from_millis(16)); // ~60fps
/// }
/// ```
pub struct FrameBuffer {
    /// The currently displayed buffer (front)
    front: BrailleGrid,
    /// The buffer being prepared (back)
    back: BrailleGrid,
}

impl FrameBuffer {
    /// Creates a new double-buffered frame system.
    ///
    /// Allocates two [`BrailleGrid`] buffers of the specified dimensions.
    /// Both buffers are initialized empty (all dots cleared).
    ///
    /// # Arguments
    ///
    /// * `width` - Width in terminal cells (characters)
    /// * `height` - Height in terminal cells (lines)
    ///
    /// # Panics
    ///
    /// Panics if `BrailleGrid::new()` fails (e.g., zero dimensions).
    /// For fallible construction, use the underlying `BrailleGrid::new()` directly.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::FrameBuffer;
    ///
    /// // Standard terminal size
    /// let buffer = FrameBuffer::new(80, 24);
    /// assert_eq!(buffer.width(), 80);
    /// assert_eq!(buffer.height(), 24);
    ///
    /// // Larger buffer for detailed graphics
    /// let large = FrameBuffer::new(200, 50);
    /// assert_eq!(large.width(), 200);
    /// ```
    #[must_use]
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            front: BrailleGrid::new(width, height).expect("FrameBuffer: invalid grid dimensions"),
            back: BrailleGrid::new(width, height).expect("FrameBuffer: invalid grid dimensions"),
        }
    }

    /// Returns a mutable reference to the back buffer for drawing.
    ///
    /// Use this to prepare the next frame. Draw operations on the back buffer
    /// do not affect the currently displayed front buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::FrameBuffer;
    ///
    /// let mut buffer = FrameBuffer::new(80, 24);
    ///
    /// // Get the back buffer and draw to it
    /// let back = buffer.get_back_buffer();
    /// back.clear();
    /// back.set_dot(0, 0).unwrap();  // Top-left dot
    /// back.set_dot(1, 0).unwrap();  // Adjacent dot
    ///
    /// // Front buffer is unchanged until swap_buffers() is called
    /// ```
    #[must_use]
    pub fn get_back_buffer(&mut self) -> &mut BrailleGrid {
        &mut self.back
    }

    /// Returns an immutable reference to the front buffer.
    ///
    /// The front buffer contains the currently displayed frame. This is
    /// read-only access; to modify a buffer, use [`get_back_buffer()`](Self::get_back_buffer).
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::FrameBuffer;
    ///
    /// let buffer = FrameBuffer::new(80, 24);
    ///
    /// // Inspect the front buffer (read-only)
    /// let front = buffer.get_front_buffer();
    /// let (width, height) = front.dimensions();
    /// assert_eq!(width, 80);
    /// assert_eq!(height, 24);
    /// ```
    #[must_use]
    pub const fn get_front_buffer(&self) -> &BrailleGrid {
        &self.front
    }

    /// Atomically swaps the front and back buffers.
    ///
    /// After this call:
    /// - The previous back buffer becomes the new front buffer
    /// - The previous front buffer becomes the new back buffer
    ///
    /// This is an O(1) pointer swap operation, not a data copy.
    /// Typical execution time is <1μs (well under the 1ms target).
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::FrameBuffer;
    ///
    /// let mut buffer = FrameBuffer::new(80, 24);
    ///
    /// // Draw a dot in the back buffer
    /// buffer.get_back_buffer().set_dot(0, 0).unwrap();
    ///
    /// // Swap - now the front buffer has the dot
    /// buffer.swap_buffers();
    ///
    /// // The old front (now back) is available for the next frame
    /// buffer.get_back_buffer().clear();  // Prepare for next frame
    /// ```
    pub fn swap_buffers(&mut self) {
        std::mem::swap(&mut self.front, &mut self.back);
    }

    /// Renders the front buffer to the terminal.
    ///
    /// Delegates to [`TerminalRenderer::render()`] to display the current
    /// front buffer contents. Supports both colored and non-colored grids.
    ///
    /// # Arguments
    ///
    /// * `renderer` - The terminal renderer to output to
    ///
    /// # Errors
    ///
    /// Returns [`DotmaxError::Terminal`] if terminal I/O fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::animation::FrameBuffer;
    /// use dotmax::TerminalRenderer;
    ///
    /// let mut buffer = FrameBuffer::new(80, 24);
    /// let mut renderer = TerminalRenderer::new().unwrap();
    ///
    /// // Draw something
    /// buffer.get_back_buffer().set_dot(10, 10).unwrap();
    /// buffer.swap_buffers();
    ///
    /// // Render to terminal
    /// buffer.render(&mut renderer).expect("Failed to render");
    ///
    /// // Clean up
    /// renderer.cleanup().unwrap();
    /// ```
    pub fn render(&self, renderer: &mut TerminalRenderer) -> Result<(), DotmaxError> {
        renderer.render(&self.front)
    }

    /// Returns the width of the buffers in terminal cells.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::FrameBuffer;
    ///
    /// let buffer = FrameBuffer::new(80, 24);
    /// assert_eq!(buffer.width(), 80);
    /// ```
    #[must_use]
    pub const fn width(&self) -> usize {
        self.front.width()
    }

    /// Returns the height of the buffers in terminal cells.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::FrameBuffer;
    ///
    /// let buffer = FrameBuffer::new(80, 24);
    /// assert_eq!(buffer.height(), 24);
    /// ```
    #[must_use]
    pub const fn height(&self) -> usize {
        self.front.height()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // AC #1: FrameBuffer::new() Creates Two BrailleGrid Buffers
    // ========================================================================

    #[test]
    fn test_new_creates_buffers_with_correct_dimensions() {
        let buffer = FrameBuffer::new(80, 24);

        // Verify both buffers have correct dimensions
        assert_eq!(buffer.front.width(), 80);
        assert_eq!(buffer.front.height(), 24);
        assert_eq!(buffer.back.width(), 80);
        assert_eq!(buffer.back.height(), 24);
    }

    #[test]
    fn test_new_dimensions_1x1() {
        // Edge case: minimum dimensions
        let buffer = FrameBuffer::new(1, 1);
        assert_eq!(buffer.width(), 1);
        assert_eq!(buffer.height(), 1);
    }

    #[test]
    fn test_new_dimensions_80x24() {
        // Standard terminal size
        let buffer = FrameBuffer::new(80, 24);
        assert_eq!(buffer.width(), 80);
        assert_eq!(buffer.height(), 24);
    }

    #[test]
    fn test_new_dimensions_200x50() {
        // Large buffer stress test
        let buffer = FrameBuffer::new(200, 50);
        assert_eq!(buffer.width(), 200);
        assert_eq!(buffer.height(), 50);
    }

    // ========================================================================
    // AC #2: get_back_buffer() Returns Mutable Reference
    // ========================================================================

    #[test]
    fn test_get_back_buffer_returns_mutable_reference() {
        let mut buffer = FrameBuffer::new(80, 24);

        // Should be able to draw to back buffer
        let back = buffer.get_back_buffer();
        let result = back.set_dot(0, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn test_back_buffer_modifications_dont_affect_front() {
        let mut buffer = FrameBuffer::new(10, 10);

        // Draw to back buffer
        buffer.get_back_buffer().set_dot(0, 0).unwrap();

        // Front buffer should still be empty
        // Cell (0,0) should have pattern 0 (empty)
        let front = buffer.get_front_buffer();
        let pattern = front.cell_to_braille_char(0, 0).unwrap();
        assert_eq!(pattern, '⠀', "Front buffer should be empty before swap");
    }

    // ========================================================================
    // AC #3: swap_buffers() Exchanges Front/Back
    // ========================================================================

    #[test]
    fn test_swap_buffers_exchanges_buffers() {
        let mut buffer = FrameBuffer::new(10, 10);

        // Draw pattern to back buffer
        buffer.get_back_buffer().set_dot(0, 0).unwrap();

        // Verify front is empty before swap
        let front_before = buffer
            .get_front_buffer()
            .cell_to_braille_char(0, 0)
            .unwrap();
        assert_eq!(front_before, '⠀');

        // Swap buffers
        buffer.swap_buffers();

        // Now front should have the pattern
        let front_after = buffer
            .get_front_buffer()
            .cell_to_braille_char(0, 0)
            .unwrap();
        assert_ne!(front_after, '⠀', "Front should have content after swap");
    }

    #[test]
    fn test_swap_buffers_double_swap_restores_original() {
        let mut buffer = FrameBuffer::new(10, 10);

        // Draw to back buffer
        buffer.get_back_buffer().set_dot(0, 0).unwrap();

        // Swap twice
        buffer.swap_buffers();
        buffer.swap_buffers();

        // Back buffer should have the content again
        let back_char = buffer.get_back_buffer().cell_to_braille_char(0, 0).unwrap();
        assert_ne!(
            back_char, '⠀',
            "Double swap should restore back buffer content"
        );
    }

    #[test]
    fn test_multiple_sequential_swaps() {
        let mut buffer = FrameBuffer::new(10, 10);

        // Draw different patterns to track which buffer is which
        buffer.get_back_buffer().set_dot(0, 0).unwrap(); // Back has dot at (0,0)

        // Swap 1: back->front
        buffer.swap_buffers();
        assert_ne!(
            buffer
                .get_front_buffer()
                .cell_to_braille_char(0, 0)
                .unwrap(),
            '⠀'
        );

        // Add another dot to the new back buffer
        buffer.get_back_buffer().set_dot(2, 0).unwrap();

        // Swap 2: back->front (now front has dot at (2,0))
        buffer.swap_buffers();
        assert_ne!(
            buffer
                .get_front_buffer()
                .cell_to_braille_char(1, 0)
                .unwrap(),
            '⠀'
        );

        // Swap 3: back->front (back to original with dot at (0,0))
        buffer.swap_buffers();
        assert_ne!(
            buffer
                .get_front_buffer()
                .cell_to_braille_char(0, 0)
                .unwrap(),
            '⠀'
        );
    }

    // ========================================================================
    // AC #6: Unit Tests Verify Buffer Swap Correctness
    // ========================================================================

    #[test]
    fn test_width_height_accessors() {
        let buffer = FrameBuffer::new(100, 50);
        assert_eq!(buffer.width(), 100);
        assert_eq!(buffer.height(), 50);
    }

    #[test]
    fn test_get_back_buffer_after_swap() {
        let mut buffer = FrameBuffer::new(10, 10);

        // Draw to back, swap, then get back buffer again
        buffer.get_back_buffer().set_dot(0, 0).unwrap();
        buffer.swap_buffers();

        // The new back buffer (old front) should be empty
        let new_back_char = buffer.get_back_buffer().cell_to_braille_char(0, 0).unwrap();
        assert_eq!(
            new_back_char, '⠀',
            "New back buffer should be empty after swap"
        );
    }

    #[test]
    fn test_content_preservation_through_swap() {
        let mut buffer = FrameBuffer::new(20, 20);

        // Draw a complex pattern to back buffer
        for i in 0..10 {
            buffer.get_back_buffer().set_dot(i * 2, i * 4).unwrap();
        }

        // Swap and verify front has all the content
        buffer.swap_buffers();

        for i in 0..10 {
            let cell_x = i;
            let cell_y = i;
            let char_at_cell = buffer
                .get_front_buffer()
                .cell_to_braille_char(cell_x, cell_y)
                .unwrap();
            assert_ne!(
                char_at_cell, '⠀',
                "Pattern should be preserved at ({}, {})",
                cell_x, cell_y
            );
        }
    }
}
