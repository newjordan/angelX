//! Line drawing using Bresenham's algorithm.
//!
//! Bresenham's line algorithm is an integer-only algorithm for drawing straight lines
//! between two points. It uses no floating point arithmetic and no division, making it
//! extremely fast and suitable for real-time graphics.
//!
//! ## Algorithm Properties
//!
//! - **Integer-only**: No floating point, only addition/subtraction/bit shifts
//! - **O(n) complexity**: Where n is the line length in dots
//! - **All octants**: Handles horizontal, vertical, diagonal, and arbitrary angles
//! - **Pixel-perfect**: Produces the mathematically correct rasterization
//!
//! ## Coordinate System
//!
//! Functions use **dot coordinates** (not cell coordinates):
//! - Grid is `width*2 × height*4` dots
//! - Example: 80×24 cell grid = 160×96 dot grid
//! - Signed `i32` coordinates allow negative values for clipping calculations
//!
//! ## References
//!
//! - Bresenham, J.E. (1965). "Algorithm for computer control of a digital plotter"
//! - Foley & Van Dam, "Computer Graphics: Principles and Practice"
//! - <https://en.wikipedia.org/wiki/Bresenham%27s_line_algorithm>

use crate::error::DotmaxError;
use crate::grid::{BrailleGrid, Color};

/// Draw a line between two points on the braille grid.
///
/// Uses Bresenham's line algorithm (integer-only, fast) to draw a straight line
/// from `(x0, y0)` to `(x1, y1)`. Handles all octants (horizontal, vertical, diagonal,
/// and arbitrary angles). Out-of-bounds coordinates are clipped gracefully.
///
/// # Arguments
///
/// * `grid` - Mutable reference to `BrailleGrid` to draw on
/// * `x0`, `y0` - Starting point in dot coordinates (signed for clipping)
/// * `x1`, `y1` - Ending point in dot coordinates (signed for clipping)
///
/// # Returns
///
/// * `Ok(())` on success
///
/// # Errors
///
/// Currently no error conditions. Returns `Ok(())` in all cases. Future versions may return
/// errors for invalid grid states.
///
/// # Examples
///
/// ```
/// use dotmax::{BrailleGrid, primitives::draw_line};
///
/// let mut grid = BrailleGrid::new(80, 24)?; // 160×96 dots
///
/// // Horizontal line
/// draw_line(&mut grid, 10, 10, 150, 10)?;
///
/// // Vertical line
/// draw_line(&mut grid, 80, 0, 80, 95)?;
///
/// // Diagonal line
/// draw_line(&mut grid, 0, 0, 159, 95)?;
///
/// // Arbitrary angle
/// draw_line(&mut grid, 20, 30, 140, 60)?;
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
///
/// # Performance
///
/// O(n) where n is the line length in dots. Typically <0.5ms for 1000-pixel line.
/// Target: <1ms for 1000-pixel lines on modern hardware.
///
/// # Clipping Behavior
///
/// Out-of-bounds coordinates do NOT return an error. Dots outside grid boundaries
/// are silently skipped (clipped). This allows drawing lines that extend beyond
/// the grid without panicking or error handling.
///
/// ```
/// use dotmax::{BrailleGrid, primitives::draw_line};
///
/// let mut grid = BrailleGrid::new(80, 24)?;
///
/// // Line partially off-grid: no error, visible portion renders correctly
/// draw_line(&mut grid, -50, -50, 100, 100)?;
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
pub fn draw_line(
    grid: &mut BrailleGrid,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
) -> Result<(), DotmaxError> {
    // Get grid bounds in dots
    // Safe cast: grid dimensions are bounded by terminal size (<10000)
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let max_x = grid.dot_width() as i32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let max_y = grid.dot_height() as i32;

    // Bresenham's line algorithm
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();

    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };

    let mut err = dx - dy;
    let mut x = x0;
    let mut y = y0;

    loop {
        // Set dot if within bounds (clip out-of-bounds dots)
        if x >= 0 && x < max_x && y >= 0 && y < max_y {
            // Safe to convert to usize - we checked x >= 0 and y >= 0
            #[allow(clippy::cast_sign_loss)]
            let _ = grid.set_dot(x as usize, y as usize);
            // Ignore errors from set_dot (defensive, shouldn't happen after bounds check)
        }

        // Check if we've reached the end point
        if x == x1 && y == y1 {
            break;
        }

        let e2 = 2 * err;

        if e2 > -dy {
            err -= dy;
            x += sx;
        }

        if e2 < dx {
            err += dx;
            y += sy;
        }
    }

    Ok(())
}

/// Draw a thick line between two points.
///
/// Draws a line with specified thickness by drawing multiple parallel lines
/// perpendicular to the main line. Thickness is measured in dots.
///
/// # Arguments
///
/// * `grid` - Mutable reference to `BrailleGrid` to draw on
/// * `x0`, `y0` - Starting point in dot coordinates
/// * `x1`, `y1` - Ending point in dot coordinates
/// * `thickness` - Line width in dots. Must be ≥ 1. Recommended ≤ 10 for braille resolution.
///
/// # Returns
///
/// * `Ok(())` on success
///
/// # Errors
///
/// * Returns `Err(DotmaxError::InvalidThickness)` if thickness is 0
///
/// # Examples
///
/// ```
/// use dotmax::{BrailleGrid, primitives::draw_line_thick};
///
/// let mut grid = BrailleGrid::new(80, 24)?;
///
/// // Thin line (thickness=1, same as draw_line)
/// draw_line_thick(&mut grid, 10, 10, 150, 10, 1)?;
///
/// // Medium line (thickness=3)
/// draw_line_thick(&mut grid, 10, 20, 150, 20, 3)?;
///
/// // Thick line (thickness=5)
/// draw_line_thick(&mut grid, 10, 30, 150, 30, 5)?;
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
///
/// # Performance
///
/// O(n * thickness) where n is the line length. Thickness=5 on 1000-pixel line
/// typically <2.5ms. Target: <5ms for thickness=5 on 1000-pixel lines.
///
/// # Recommended Thickness
///
/// For braille resolution (2×4 dots per cell), recommended maximum thickness is 10 dots.
/// Larger thickness values work but may look chunky at braille resolution.
pub fn draw_line_thick(
    grid: &mut BrailleGrid,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    thickness: u32,
) -> Result<(), DotmaxError> {
    // Validate thickness
    if thickness == 0 {
        return Err(DotmaxError::InvalidThickness { thickness: 0 });
    }

    // Thickness=1 is just a regular line
    if thickness == 1 {
        return draw_line(grid, x0, y0, x1, y1);
    }

    // Calculate perpendicular direction
    // For line (dx, dy), perpendicular is (-dy, dx) or (dy, -dx)
    let dx = x1 - x0;
    let dy = y1 - y0;

    // Normalize perpendicular vector (approximate for integer math)
    let length = f64::from(dx * dx + dy * dy).sqrt();

    if length == 0.0 {
        // Zero-length line: just draw a thick dot (filled rectangle)
        // Safe cast: thickness is bounded by API contract (recommended max 10)
        #[allow(clippy::cast_possible_wrap)]
        let half_thick = (thickness / 2) as i32;
        for i in -(half_thick)..=(half_thick) {
            for j in -(half_thick)..=(half_thick) {
                draw_line(grid, x0 + i, y0 + j, x0 + i, y0 + j)?;
            }
        }
        return Ok(());
    }

    // Safe casts: length > 0 ensures no division by zero, result is small integers
    #[allow(clippy::cast_possible_truncation)]
    let perp_x = (f64::from(-dy) / length) as i32;
    #[allow(clippy::cast_possible_truncation)]
    let perp_y = (f64::from(dx) / length) as i32;

    // Draw parallel lines offset by thickness
    // Safe cast: thickness is bounded by API contract
    #[allow(clippy::cast_possible_wrap)]
    let half_thickness = (thickness / 2) as i32;

    for offset in -half_thickness..=half_thickness {
        let offset_x = perp_x * offset;
        let offset_y = perp_y * offset;

        draw_line(
            grid,
            x0 + offset_x,
            y0 + offset_y,
            x1 + offset_x,
            y1 + offset_y,
        )?;
    }

    Ok(())
}

/// Draw a colored line between two points on the braille grid.
///
/// Uses Bresenham's line algorithm to draw a line with specified color.
/// All dots in the line will have the same color applied to their containing cells.
///
/// **Story 4.5** - Color support for drawing primitives.
///
/// # Arguments
///
/// * `grid` - Mutable reference to `BrailleGrid` (must have color support enabled)
/// * `x0`, `y0` - Starting point in dot coordinates
/// * `x1`, `y1` - Ending point in dot coordinates
/// * `color` - RGB color to apply to line dots
/// * `thickness` - Optional line thickness (None = thin/1 dot, Some(n) = n dots thick)
///
/// # Errors
///
/// * Returns `Err(DotmaxError::InvalidThickness)` if thickness is Some(0)
///
/// # Prerequisites
///
/// Grid must have color support enabled via `grid.enable_color_support()`. If color
/// support is not enabled, cells may still render but colors will be ignored.
///
/// # Color Application
///
/// Color is applied at the **cell level** (not per-dot). Each braille cell (2×4 dots)
/// has a single color. When a colored line passes through a cell, that cell's color
/// is set to the line's color.
///
/// # Examples
///
/// ```
/// use dotmax::{BrailleGrid, Color, primitives::draw_line_colored};
///
/// let mut grid = BrailleGrid::new(80, 24).unwrap();
/// grid.enable_color_support();
///
/// // Red diagonal line
/// let red = Color::rgb(255, 0, 0);
/// draw_line_colored(&mut grid, 0, 0, 159, 95, red, None)?;
///
/// // Thick green horizontal line
/// let green = Color::rgb(0, 255, 0);
/// draw_line_colored(&mut grid, 10, 50, 150, 50, green, Some(3))?;
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
///
/// # Performance
///
/// O(n) for thin lines, O(n * thickness) for thick lines. Colored lines have
/// minimal overhead (~1-2%) compared to non-colored lines due to additional
/// cell color setting operations.
///
/// # Backward Compatibility
///
/// This function is a new addition (Story 4.5). Existing non-colored functions
/// (`draw_line`, `draw_line_thick`) remain unchanged and continue to work.
pub fn draw_line_colored(
    grid: &mut BrailleGrid,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: Color,
    thickness: Option<u32>,
) -> Result<(), DotmaxError> {
    // Handle thickness
    match thickness {
        None | Some(1) => draw_line_colored_impl(grid, x0, y0, x1, y1, color),
        Some(0) => Err(DotmaxError::InvalidThickness { thickness: 0 }),
        Some(t) => draw_line_thick_colored_impl(grid, x0, y0, x1, y1, color, t),
    }
}

/// Internal implementation for thin colored lines
#[allow(clippy::unnecessary_wraps)]
fn draw_line_colored_impl(
    grid: &mut BrailleGrid,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: Color,
) -> Result<(), DotmaxError> {
    // Get grid bounds in dots
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let max_x = grid.dot_width() as i32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let max_y = grid.dot_height() as i32;

    // Bresenham's line algorithm (same as draw_line but with color setting)
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();

    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };

    let mut err = dx - dy;
    let mut x = x0;
    let mut y = y0;

    loop {
        // Set dot and color if within bounds
        if x >= 0 && x < max_x && y >= 0 && y < max_y {
            #[allow(clippy::cast_sign_loss)]
            let dot_x = x as usize;
            #[allow(clippy::cast_sign_loss)]
            let dot_y = y as usize;

            // Set dot
            let _ = grid.set_dot(dot_x, dot_y);

            // Convert dot coordinates to cell coordinates
            let cell_x = dot_x / 2; // 2 dots per cell horizontally
            let cell_y = dot_y / 4; // 4 dots per cell vertically

            // Set cell color
            let _ = grid.set_cell_color(cell_x, cell_y, color);
        }

        // Check if we've reached the end point
        if x == x1 && y == y1 {
            break;
        }

        let e2 = 2 * err;

        if e2 > -dy {
            err -= dy;
            x += sx;
        }

        if e2 < dx {
            err += dx;
            y += sy;
        }
    }

    Ok(())
}

/// Internal implementation for thick colored lines
fn draw_line_thick_colored_impl(
    grid: &mut BrailleGrid,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: Color,
    thickness: u32,
) -> Result<(), DotmaxError> {
    // Calculate perpendicular direction for thickness
    let dx = x1 - x0;
    let dy = y1 - y0;

    // Normalize perpendicular vector
    let length = f64::from(dx * dx + dy * dy).sqrt();

    if length == 0.0 {
        // Zero-length line: draw thick dot (filled rectangle)
        #[allow(clippy::cast_possible_wrap)]
        let half_thick = (thickness / 2) as i32;
        for i in -(half_thick)..=(half_thick) {
            for j in -(half_thick)..=(half_thick) {
                draw_line_colored_impl(grid, x0 + i, y0 + j, x0 + i, y0 + j, color)?;
            }
        }
        return Ok(());
    }

    // Calculate perpendicular offsets
    #[allow(clippy::cast_possible_truncation)]
    let perp_x = (f64::from(-dy) / length) as i32;
    #[allow(clippy::cast_possible_truncation)]
    let perp_y = (f64::from(dx) / length) as i32;

    // Draw parallel colored lines
    #[allow(clippy::cast_possible_wrap)]
    let half_thickness = (thickness / 2) as i32;

    for offset in -half_thickness..=half_thickness {
        let offset_x = perp_x * offset;
        let offset_y = perp_y * offset;

        draw_line_colored_impl(
            grid,
            x0 + offset_x,
            y0 + offset_y,
            x1 + offset_x,
            y1 + offset_y,
            color,
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to check if a specific dot is set at dot coordinates
    /// Returns true if dot is set, false otherwise
    fn is_dot_set(grid: &BrailleGrid, dot_x: usize, dot_y: usize) -> bool {
        // Convert dot coordinates to cell and dot index
        let cell_x = dot_x / 2;
        let cell_y = dot_y / 4;
        let local_x = dot_x % 2;
        let local_y = dot_y % 4;

        // Map to dot index (0-7)
        let dot_index = match (local_x, local_y) {
            (0, 0) => 0, // Dot1
            (0, 1) => 1, // Dot2
            (0, 2) => 2, // Dot3
            (0, 3) => 6, // Dot7
            (1, 0) => 3, // Dot4
            (1, 1) => 4, // Dot5
            (1, 2) => 5, // Dot6
            (1, 3) => 7, // Dot8
            _ => return false,
        };

        grid.get_dot(cell_x, cell_y, dot_index).unwrap_or(false)
    }

    #[test]
    fn test_horizontal_line() {
        let mut grid = BrailleGrid::new(20, 10).unwrap(); // 40×40 dots

        // Draw horizontal line from (0,5) to (10,5)
        draw_line(&mut grid, 0, 5, 10, 5).unwrap();

        // Verify dots are set along y=5
        for x in 0..=10 {
            assert!(is_dot_set(&grid, x, 5), "Dot at ({}, 5) should be set", x);
        }

        // Verify dots not set elsewhere (spot check)
        assert!(!is_dot_set(&grid, 0, 4), "Dot at (0, 4) should not be set");
        assert!(!is_dot_set(&grid, 0, 6), "Dot at (0, 6) should not be set");
    }

    #[test]
    fn test_vertical_line() {
        let mut grid = BrailleGrid::new(20, 10).unwrap(); // 40×40 dots

        // Draw vertical line from (5,0) to (5,10)
        draw_line(&mut grid, 5, 0, 5, 10).unwrap();

        // Verify dots are set along x=5
        for y in 0..=10 {
            assert!(is_dot_set(&grid, 5, y), "Dot at (5, {}) should be set", y);
        }

        // Verify dots not set elsewhere (spot check)
        assert!(!is_dot_set(&grid, 4, 0), "Dot at (4, 0) should not be set");
        assert!(!is_dot_set(&grid, 6, 0), "Dot at (6, 0) should not be set");
    }

    #[test]
    fn test_diagonal_line_45deg() {
        let mut grid = BrailleGrid::new(20, 10).unwrap(); // 40×40 dots

        // Draw 45° diagonal from (0,0) to (10,10)
        draw_line(&mut grid, 0, 0, 10, 10).unwrap();

        // Verify dots along diagonal
        for i in 0..=10 {
            assert!(
                is_dot_set(&grid, i, i),
                "Dot at ({}, {}) should be set",
                i,
                i
            );
        }
    }

    #[test]
    fn test_arbitrary_angle() {
        let mut grid = BrailleGrid::new(20, 10).unwrap(); // 40×40 dots

        // Draw line from (0,0) to (10,5)
        draw_line(&mut grid, 0, 0, 10, 5).unwrap();

        // Verify endpoints are set
        assert!(is_dot_set(&grid, 0, 0), "Start point should be set");
        assert!(is_dot_set(&grid, 10, 5), "End point should be set");

        // Verify line is continuous (at least some dots along path)
        let mut dots_set = 0;
        for x in 0..=10 {
            for y in 0..=5 {
                if is_dot_set(&grid, x, y) {
                    dots_set += 1;
                }
            }
        }
        // Line from (0,0) to (10,5) should set ~10-11 dots
        assert!(
            dots_set >= 8,
            "Line should set at least 8 dots, got {}",
            dots_set
        );
    }

    #[test]
    fn test_boundary_clipping() {
        let mut grid = BrailleGrid::new(10, 10).unwrap(); // 20×40 dots

        // Draw line from outside grid to inside: should not panic
        draw_line(&mut grid, -10, -10, 10, 10).unwrap();

        // Verify visible portion rendered
        assert!(is_dot_set(&grid, 0, 0), "Origin should have dot");
        assert!(is_dot_set(&grid, 10, 10), "Point (10,10) should have dot");

        // Draw line with extreme coordinates: should not panic
        draw_line(&mut grid, -10000, -10000, 50000, 50000).unwrap();
    }

    #[test]
    fn test_zero_length_line() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();

        // Zero-length line (same start and end)
        draw_line(&mut grid, 5, 5, 5, 5).unwrap();

        // Should set the single dot
        assert!(is_dot_set(&grid, 5, 5), "Single dot should be set");
    }

    #[test]
    fn test_invalid_thickness() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();

        // Thickness=0 should return error
        let result = draw_line_thick(&mut grid, 0, 0, 10, 10, 0);
        assert!(result.is_err(), "Thickness=0 should return error");

        match result {
            Err(DotmaxError::InvalidThickness { thickness }) => {
                assert_eq!(thickness, 0);
            }
            _ => panic!("Expected InvalidThickness error"),
        }
    }

    #[test]
    fn test_thick_line() {
        let mut grid = BrailleGrid::new(20, 10).unwrap(); // 40×40 dots

        // Draw thin line (thickness=1)
        draw_line_thick(&mut grid, 5, 10, 5, 20, 1).unwrap();

        // Count dots set for thin line
        let mut thin_dots = 0;
        for x in 0..40 {
            for y in 0..40 {
                if is_dot_set(&grid, x, y) {
                    thin_dots += 1;
                }
            }
        }

        // Clear grid
        grid.clear();

        // Draw thick line (thickness=3)
        draw_line_thick(&mut grid, 5, 10, 5, 20, 3).unwrap();

        // Count dots set for thick line
        let mut thick_dots = 0;
        for x in 0..40 {
            for y in 0..40 {
                if is_dot_set(&grid, x, y) {
                    thick_dots += 1;
                }
            }
        }

        // Thick line should have more dots than thin line
        assert!(
            thick_dots > thin_dots,
            "Thick line ({} dots) should have more dots than thin line ({} dots)",
            thick_dots,
            thin_dots
        );
    }
}
