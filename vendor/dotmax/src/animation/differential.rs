//! Differential rendering for animations.
//!
//! This module provides optimized rendering that outputs only changed cells between frames,
//! reducing terminal I/O by 60-80% for typical animations with small moving objects on
//! static backgrounds.
//!
//! # Overview
//!
//! Traditional animation rendering redraws every cell every frame, which can be expensive
//! for terminal I/O (1920 escape codes for an 80x24 terminal). Differential rendering
//! compares each frame to the previous frame and only outputs cells that have changed,
//! dramatically reducing I/O overhead.
//!
//! # Performance
//!
//! For a typical animation with 5% of cells changing per frame:
//! - **Full render**: 80×24 = 1920 cells → 1920 escape codes
//! - **Differential**: ~96 cells → ~96 escape codes
//! - **I/O reduction**: ~95% (exceeds the 60-80% target)
//!
//! # Example
//!
//! ```no_run
//! use dotmax::animation::DifferentialRenderer;
//! use dotmax::{BrailleGrid, TerminalRenderer};
//!
//! # fn main() -> Result<(), dotmax::DotmaxError> {
//! let mut diff_renderer = DifferentialRenderer::new();
//! let mut terminal = TerminalRenderer::new()?;
//!
//! // First frame renders fully (no previous frame to compare)
//! let frame1 = BrailleGrid::new(80, 24)?;
//! diff_renderer.render_diff(&frame1, &mut terminal)?;
//!
//! // Subsequent frames render only changes
//! let mut frame2 = BrailleGrid::new(80, 24)?;
//! frame2.set_dot(10, 10)?;  // Only one cell changed
//! diff_renderer.render_diff(&frame2, &mut terminal)?;  // Only outputs that one cell!
//!
//! // Force full render after resize
//! diff_renderer.invalidate();
//! diff_renderer.render_diff(&frame2, &mut terminal)?;  // Full render again
//! # Ok(())
//! # }
//! ```
//!
//! # When to Use Differential Rendering
//!
//! Differential rendering is most effective when:
//! - Animations have static backgrounds with small moving elements
//! - High frame rates are needed (30-60+ fps)
//! - Terminal I/O bandwidth is a concern
//!
//! It's less beneficial when:
//! - Most of the screen changes every frame (e.g., full-screen scrolling)
//! - Frame rate is already low (< 10 fps)

use crate::error::DotmaxError;
use crate::grid::BrailleGrid;
use crate::render::TerminalRenderer;
use crossterm::{cursor::MoveTo, QueueableCommand};
use std::io::Write;
use tracing::debug;

/// Optimized renderer that only outputs changed cells.
///
/// `DifferentialRenderer` compares the current frame to the previous frame
/// and renders only the cells that have changed. For typical animations with
/// small moving objects on static backgrounds, this reduces terminal I/O
/// by 60-80% or more.
///
/// # Performance
///
/// - Full render at 80×24: 1920 cells, ~1920 escape codes
/// - Differential with 5% changes: ~96 cells, ~96 escape codes
/// - Typical I/O reduction: 60-95%
///
/// # Example
///
/// ```no_run
/// use dotmax::animation::DifferentialRenderer;
/// use dotmax::{BrailleGrid, TerminalRenderer};
///
/// # fn main() -> Result<(), dotmax::DotmaxError> {
/// let mut diff_renderer = DifferentialRenderer::new();
/// let mut terminal = TerminalRenderer::new()?;
///
/// // First frame renders fully (no previous frame)
/// let frame1 = BrailleGrid::new(80, 24)?;
/// diff_renderer.render_diff(&frame1, &mut terminal)?;
///
/// // Subsequent frames render only changes
/// let mut frame2 = BrailleGrid::new(80, 24)?;
/// frame2.set_dot(10, 10)?;  // One changed cell
/// diff_renderer.render_diff(&frame2, &mut terminal)?;  // Only outputs one cell!
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct DifferentialRenderer {
    /// The last frame rendered, used for comparison.
    /// `None` if no frame has been rendered yet or after `invalidate()`.
    last_frame: Option<BrailleGrid>,
}

impl DifferentialRenderer {
    /// Creates a new differential renderer.
    ///
    /// The first call to [`render_diff()`](Self::render_diff) will render the full frame
    /// since there's no previous frame to compare against.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::DifferentialRenderer;
    ///
    /// let renderer = DifferentialRenderer::new();
    /// // First render_diff() call will render the entire frame
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self { last_frame: None }
    }

    /// Renders only the cells that changed since the last frame.
    ///
    /// Compares `current` to the stored previous frame and outputs
    /// only the changed cells using ANSI cursor positioning.
    ///
    /// # Behavior
    ///
    /// - **First call**: Renders entire grid (no previous frame)
    /// - **Dimension mismatch**: Renders entire grid (auto-invalidates)
    /// - **Normal operation**: Renders only changed cells
    ///
    /// # Arguments
    ///
    /// * `current` - The current frame to render
    /// * `renderer` - The terminal renderer for full-frame fallback
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Render completed successfully
    /// * `Err(DotmaxError)` - Terminal I/O error
    ///
    /// # Errors
    ///
    /// Returns `DotmaxError::Terminal` if terminal I/O operations fail
    /// (cursor positioning or character output).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::animation::DifferentialRenderer;
    /// use dotmax::{BrailleGrid, TerminalRenderer};
    ///
    /// # fn main() -> Result<(), dotmax::DotmaxError> {
    /// let mut diff = DifferentialRenderer::new();
    /// let mut terminal = TerminalRenderer::new()?;
    ///
    /// let frame = BrailleGrid::new(80, 24)?;
    /// diff.render_diff(&frame, &mut terminal)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn render_diff(
        &mut self,
        current: &BrailleGrid,
        renderer: &mut TerminalRenderer,
    ) -> Result<(), DotmaxError> {
        // Check for dimension mismatch or no previous frame
        let should_full_render = self.last_frame.as_ref().map_or(true, |last| {
            if last.width() != current.width() || last.height() != current.height() {
                debug!(
                    old_width = last.width(),
                    old_height = last.height(),
                    new_width = current.width(),
                    new_height = current.height(),
                    "Dimension mismatch - performing full frame render"
                );
                true
            } else {
                false
            }
        });

        // Get reference to last frame for comparison, or render full if None
        let last = match (should_full_render, &self.last_frame) {
            (true, _) | (_, None) => {
                debug!("First render or dimension change - performing full frame render");
                // Full render using the terminal renderer
                renderer.render(current)?;
                self.last_frame = Some(current.clone());
                return Ok(());
            }
            (false, Some(last)) => last,
        };

        // Differential render
        let mut stdout = std::io::stdout();
        let mut changed_count = 0;

        for y in 0..current.height() {
            for x in 0..current.width() {
                if Self::cells_differ(current, last, x, y) {
                    // Move cursor to position
                    // Safe to truncate: terminal dimensions fit in u16
                    #[allow(clippy::cast_possible_truncation)]
                    stdout.queue(MoveTo(x as u16, y as u16))?;

                    // Get the character to render
                    let ch = current.get_char(x, y);

                    // Apply color if present
                    if let Some(color) = current.get_color(x, y) {
                        write!(
                            stdout,
                            "\x1b[38;2;{};{};{}m{}\x1b[0m",
                            color.r, color.g, color.b, ch
                        )?;
                    } else {
                        write!(stdout, "{ch}")?;
                    }
                    changed_count += 1;
                }
            }
        }

        stdout.flush()?;
        debug!(
            changed_cells = changed_count,
            "Differential render complete"
        );
        self.last_frame = Some(current.clone());
        Ok(())
    }

    /// Forces a full render on the next [`render_diff()`](Self::render_diff) call.
    ///
    /// Use this after terminal resize, mode changes, or when the entire screen
    /// needs to be refreshed.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::DifferentialRenderer;
    ///
    /// let mut renderer = DifferentialRenderer::new();
    /// // ... after some renders ...
    /// renderer.invalidate();  // Next render_diff() will render full frame
    /// ```
    pub fn invalidate(&mut self) {
        debug!("Invalidating differential renderer - next render will be full");
        self.last_frame = None;
    }

    /// Compares cells at (x, y) between current and last frames.
    ///
    /// Returns `true` if the cells differ (dots or colors).
    fn cells_differ(current: &BrailleGrid, last: &BrailleGrid, x: usize, y: usize) -> bool {
        // Compare dot patterns (using raw access for efficiency)
        let current_patterns = current.get_raw_patterns();
        let last_patterns = last.get_raw_patterns();
        let index = y * current.width() + x;

        if current_patterns[index] != last_patterns[index] {
            return true;
        }

        // Compare colors
        if current.get_color(x, y) != last.get_color(x, y) {
            return true;
        }

        false
    }

    /// Returns the number of cells that would change between two frames.
    ///
    /// This is useful for benchmarking and debugging to understand
    /// how much I/O reduction differential rendering provides.
    ///
    /// # Arguments
    ///
    /// * `current` - The current frame
    /// * `previous` - The previous frame to compare against
    ///
    /// # Returns
    ///
    /// The number of cells that differ between the two frames.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::DifferentialRenderer;
    /// use dotmax::BrailleGrid;
    ///
    /// let renderer = DifferentialRenderer::new();
    ///
    /// let mut frame1 = BrailleGrid::new(80, 24).unwrap();
    /// let mut frame2 = BrailleGrid::new(80, 24).unwrap();
    /// frame2.set_dot(10, 10).unwrap();
    ///
    /// let changed = renderer.count_changed_cells(&frame2, &frame1);
    /// assert_eq!(changed, 1);  // Only one cell changed
    /// ```
    #[must_use]
    pub fn count_changed_cells(&self, current: &BrailleGrid, previous: &BrailleGrid) -> usize {
        if current.width() != previous.width() || current.height() != previous.height() {
            // Different dimensions = all cells changed
            return current.width() * current.height();
        }

        let mut count = 0;
        for y in 0..current.height() {
            for x in 0..current.width() {
                if Self::cells_differ(current, previous, x, y) {
                    count += 1;
                }
            }
        }
        count
    }

    /// Returns whether the renderer has a cached previous frame.
    ///
    /// # Returns
    ///
    /// * `true` - A previous frame is cached (differential rendering enabled)
    /// * `false` - No previous frame (next render will be full)
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::DifferentialRenderer;
    ///
    /// let renderer = DifferentialRenderer::new();
    /// assert!(!renderer.has_previous_frame());
    /// ```
    #[must_use]
    pub const fn has_previous_frame(&self) -> bool {
        self.last_frame.is_some()
    }
}

impl Default for DifferentialRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for DifferentialRenderer {
    fn clone(&self) -> Self {
        Self {
            last_frame: self.last_frame.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_renderer_with_no_last_frame() {
        let renderer = DifferentialRenderer::new();
        assert!(!renderer.has_previous_frame());
    }

    #[test]
    fn test_default_same_as_new() {
        let renderer1 = DifferentialRenderer::new();
        let renderer2 = DifferentialRenderer::default();
        assert!(!renderer1.has_previous_frame());
        assert!(!renderer2.has_previous_frame());
    }

    #[test]
    fn test_invalidate_clears_last_frame() {
        let mut renderer = DifferentialRenderer::new();
        // Simulate having a last frame by creating and cloning a grid
        renderer.last_frame = Some(BrailleGrid::new(10, 10).unwrap());
        assert!(renderer.has_previous_frame());

        renderer.invalidate();
        assert!(!renderer.has_previous_frame());
    }

    #[test]
    fn test_count_changed_cells_identical_frames() {
        let renderer = DifferentialRenderer::new();
        let frame1 = BrailleGrid::new(10, 10).unwrap();
        let frame2 = BrailleGrid::new(10, 10).unwrap();

        let changed = renderer.count_changed_cells(&frame1, &frame2);
        assert_eq!(changed, 0);
    }

    #[test]
    fn test_count_changed_cells_single_change() {
        let renderer = DifferentialRenderer::new();
        let frame1 = BrailleGrid::new(10, 10).unwrap();
        let mut frame2 = BrailleGrid::new(10, 10).unwrap();
        frame2.set_dot(0, 0).unwrap();

        let changed = renderer.count_changed_cells(&frame2, &frame1);
        assert_eq!(changed, 1);
    }

    #[test]
    fn test_count_changed_cells_dimension_mismatch() {
        let renderer = DifferentialRenderer::new();
        let frame1 = BrailleGrid::new(10, 10).unwrap();
        let frame2 = BrailleGrid::new(20, 20).unwrap();

        let changed = renderer.count_changed_cells(&frame2, &frame1);
        // Different dimensions = all cells count as changed
        assert_eq!(changed, 400); // 20 * 20
    }

    #[test]
    fn test_cells_differ_detects_dot_change() {
        let frame1 = BrailleGrid::new(10, 10).unwrap();
        let mut frame2 = BrailleGrid::new(10, 10).unwrap();
        frame2.set_dot(0, 0).unwrap(); // Set dot in cell (0, 0)

        assert!(DifferentialRenderer::cells_differ(&frame2, &frame1, 0, 0));
        assert!(!DifferentialRenderer::cells_differ(&frame2, &frame1, 1, 1));
    }

    #[test]
    fn test_cells_differ_detects_color_change() {
        use crate::grid::Color;

        let mut frame1 = BrailleGrid::new(10, 10).unwrap();
        let mut frame2 = BrailleGrid::new(10, 10).unwrap();

        // Set same dots but different colors
        frame1.set_dot(0, 0).unwrap();
        frame2.set_dot(0, 0).unwrap();
        frame2.set_cell_color(0, 0, Color::rgb(255, 0, 0)).unwrap();

        assert!(DifferentialRenderer::cells_differ(&frame2, &frame1, 0, 0));
    }

    #[test]
    fn test_clone() {
        let mut renderer = DifferentialRenderer::new();
        renderer.last_frame = Some(BrailleGrid::new(10, 10).unwrap());

        let cloned = renderer.clone();
        assert!(cloned.has_previous_frame());
    }
}
