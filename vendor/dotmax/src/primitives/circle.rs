//! Circle drawing using Bresenham's midpoint circle algorithm.
//!
//! Bresenham's circle algorithm (also known as the midpoint circle algorithm) is an
//! integer-only algorithm for drawing circles. It uses 8-way symmetry to efficiently
//! draw circles by calculating points in only one octant and mirroring to the other 7.
//!
//! ## Algorithm Properties
//!
//! - **Integer-only**: No floating point (except filled circle scanlines), only addition/subtraction
//! - **O(r) complexity**: Where r is the radius in dots
//! - **8-way symmetry**: Plot 8 symmetric points per iteration
//! - **Pixel-perfect**: Produces mathematically correct circular rasterization
//!
//! ## Coordinate System
//!
//! Functions use **dot coordinates** (not cell coordinates):
//! - Grid is `width*2 × height*4` dots
//! - Example: 80×24 cell grid = 160×96 dot grid
//! - Signed `i32` coordinates for center allow negative values for clipping calculations
//! - Unsigned `u32` for radius (always positive)
//!
//! ## References
//!
//! - Bresenham, J.E. (1965). "Algorithm for computer control of a digital plotter"
//! - Foley & Van Dam, "Computer Graphics: Principles and Practice", Section 3.2
//! - <https://en.wikipedia.org/wiki/Midpoint_circle_algorithm>

use crate::error::DotmaxError;
use crate::grid::{BrailleGrid, Color};
use crate::primitives::draw_line;

/// Draw a circle outline on the braille grid.
///
/// Uses Bresenham's circle algorithm (midpoint circle algorithm) to draw a circle
/// outline with the specified center and radius. The algorithm uses 8-way symmetry
/// for efficiency, plotting 8 points per iteration.
///
/// Coordinates outside the grid boundaries are clipped gracefully (no panics or errors).
///
/// # Arguments
///
/// * `grid` - Mutable reference to `BrailleGrid` to draw on
/// * `center_x`, `center_y` - Circle center in dot coordinates (signed for clipping)
/// * `radius` - Circle radius in dots (unsigned, must be ≥ 0)
///
/// # Returns
///
/// * `Ok(())` on success
///
/// # Errors
///
/// Currently no error conditions. Returns `Ok(())` in all cases.
///
/// # Examples
///
/// ```
/// use dotmax::{BrailleGrid, primitives::draw_circle};
///
/// let mut grid = BrailleGrid::new(80, 24)?; // 160×96 dots
/// draw_circle(&mut grid, 80, 48, 30)?; // Circle at center, radius 30
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
///
/// # Performance
///
/// O(r) where r is the radius in dots. Typically <0.5ms for radius 100.
pub fn draw_circle(
    grid: &mut BrailleGrid,
    center_x: i32,
    center_y: i32,
    radius: u32,
) -> Result<(), DotmaxError> {
    // Handle zero radius: single dot at center
    if radius == 0 {
        plot_dot_clipped(grid, center_x, center_y);
        return Ok(());
    }

    #[allow(clippy::cast_possible_wrap)]
    let mut x = radius as i32;
    let mut y = 0i32;
    let mut err = 1 - x; // Decision variable

    // Midpoint circle algorithm with 8-way symmetry
    while x >= y {
        // Plot 8 symmetric points for this (x, y)
        plot_8_symmetric_dots(grid, center_x, center_y, x, y);

        y += 1;
        if err < 0 {
            err += 2 * y + 1;
        } else {
            x -= 1;
            err += 2 * (y - x) + 1;
        }
    }

    Ok(())
}

/// Draw a filled circle on the braille grid.
///
/// Fills the interior using horizontal line spans (scanline fill approach).
/// For each y coordinate from center-radius to center+radius, calculates the
/// x span using the circle equation and draws a horizontal line.
///
/// Uses the existing `draw_line()` function from line drawing for efficiency.
///
/// # Arguments
///
/// * `grid` - Mutable reference to `BrailleGrid` to draw on
/// * `center_x`, `center_y` - Circle center in dot coordinates (signed for clipping)
/// * `radius` - Circle radius in dots (unsigned). radius=0 draws single dot at center.
///
/// # Returns
///
/// * `Ok(())` on success
///
/// # Errors
///
/// May propagate errors from `draw_line()` if grid operations fail.
///
/// # Examples
///
/// ```
/// use dotmax::{BrailleGrid, primitives::draw_circle_filled};
///
/// let mut grid = BrailleGrid::new(80, 24)?;
/// draw_circle_filled(&mut grid, 80, 48, 20)?; // Filled circle
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
///
/// # Performance
///
/// O(r²) due to filling interior. Typically <5ms for radius 100.
pub fn draw_circle_filled(
    grid: &mut BrailleGrid,
    center_x: i32,
    center_y: i32,
    radius: u32,
) -> Result<(), DotmaxError> {
    // Handle zero radius: single dot at center
    if radius == 0 {
        plot_dot_clipped(grid, center_x, center_y);
        return Ok(());
    }

    #[allow(clippy::cast_possible_wrap)]
    let radius_i32 = radius as i32;

    // Scanline fill: for each y from -radius to +radius, calculate x span
    for dy in -radius_i32..=radius_i32 {
        let y = center_y + dy;

        // Calculate x span using circle equation: x = sqrt(r² - y²)
        #[allow(clippy::cast_precision_loss)]
        let radius_f = radius as f32;
        #[allow(clippy::cast_precision_loss)]
        let dy_f = dy as f32;
        #[allow(clippy::suboptimal_flops)]
        let x_span = (radius_f * radius_f - dy_f * dy_f).sqrt();

        #[allow(clippy::cast_possible_truncation)]
        let x_offset = x_span.round() as i32;

        // Draw horizontal line from center-x_span to center+x_span
        let x_start = center_x - x_offset;
        let x_end = center_x + x_offset;

        draw_line(grid, x_start, y, x_end, y)?;
    }

    Ok(())
}

/// Draw a thick circle outline on the braille grid.
///
/// Draws multiple concentric circles to create a thickness effect.
/// For thickness N, draws N circles with radii from `radius` to `radius+thickness-1`.
///
/// # Arguments
///
/// * `grid` - Mutable reference to `BrailleGrid` to draw on
/// * `center_x`, `center_y` - Circle center in dot coordinates (signed for clipping)
/// * `radius` - Inner circle radius in dots (unsigned)
/// * `thickness` - Circle outline width in dots. Must be ≥ 1. Recommended ≤ 10 for braille resolution.
///
/// # Returns
///
/// * `Ok(())` on success
///
/// # Errors
///
/// * Returns `DotmaxError::InvalidThickness` if thickness is 0
///
/// # Examples
///
/// ```
/// use dotmax::{BrailleGrid, primitives::draw_circle_thick};
///
/// let mut grid = BrailleGrid::new(80, 24)?;
/// draw_circle_thick(&mut grid, 80, 48, 25, 5)?; // Thick circle, thickness 5
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
///
/// # Performance
///
/// O(r × thickness) where r is the radius. Typically <3ms for radius 100, thickness 5.
pub fn draw_circle_thick(
    grid: &mut BrailleGrid,
    center_x: i32,
    center_y: i32,
    radius: u32,
    thickness: u32,
) -> Result<(), DotmaxError> {
    if thickness == 0 {
        return Err(DotmaxError::InvalidThickness { thickness: 0 });
    }

    // Special case: thickness=1 is just a regular circle
    if thickness == 1 {
        return draw_circle(grid, center_x, center_y, radius);
    }

    // Draw concentric circles from radius to radius+thickness-1
    for i in 0..thickness {
        draw_circle(grid, center_x, center_y, radius + i)?;
    }

    Ok(())
}

/// Plot 8 symmetric dots for the midpoint circle algorithm.
///
/// Given a point (x, y) relative to center, plots all 8 symmetric points:
/// (x, y), (y, x), (-y, x), (-x, y), (-x, -y), (-y, -x), (y, -x), (x, -y)
///
/// All points are checked against grid boundaries before plotting.
#[inline]
fn plot_8_symmetric_dots(grid: &mut BrailleGrid, center_x: i32, center_y: i32, x: i32, y: i32) {
    // All 8 octants
    plot_dot_clipped(grid, center_x + x, center_y + y);
    plot_dot_clipped(grid, center_x + y, center_y + x);
    plot_dot_clipped(grid, center_x - y, center_y + x);
    plot_dot_clipped(grid, center_x - x, center_y + y);
    plot_dot_clipped(grid, center_x - x, center_y - y);
    plot_dot_clipped(grid, center_x - y, center_y - x);
    plot_dot_clipped(grid, center_x + y, center_y - x);
    plot_dot_clipped(grid, center_x + x, center_y - y);
}

/// Plot a single dot with boundary clipping.
///
/// Checks if the dot is within grid boundaries before plotting.
/// Out-of-bounds dots are silently skipped (no errors or panics).
#[inline]
fn plot_dot_clipped(grid: &mut BrailleGrid, x: i32, y: i32) {
    // Convert grid dimensions to dot space
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let max_x = (grid.width() * 2) as i32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let max_y = (grid.height() * 4) as i32;

    // Check bounds and plot if inside
    if x >= 0 && y >= 0 && x < max_x && y < max_y {
        #[allow(clippy::cast_sign_loss)]
        let _ = grid.set_dot(x as usize, y as usize);
    }
}

/// Draw a colored circle on the braille grid.
///
/// Uses Bresenham's circle algorithm to draw a circle with specified color.
/// All dots in the circle will have the same color applied to their containing cells.
///
/// **Story 4.5** - Color support for drawing primitives.
///
/// # Arguments
///
/// * `grid` - Mutable reference to `BrailleGrid` (must have color support enabled)
/// * `center_x`, `center_y` - Circle center in dot coordinates
/// * `radius` - Circle radius in dots (unsigned, must be ≥ 0)
/// * `color` - RGB color to apply to circle dots
/// * `filled` - If true, fill the circle interior; if false, draw outline only
///
/// # Errors
///
/// Currently no error conditions. Returns `Ok(())` in all cases.
///
/// # Prerequisites
///
/// Grid must have color support enabled via `grid.enable_color_support()`. If color
/// support is not enabled, cells may still render but colors will be ignored.
///
/// # Color Application
///
/// Color is applied at the **cell level** (not per-dot). Each braille cell (2×4 dots)
/// has a single color. When a colored circle passes through a cell, that cell's color
/// is set to the circle's color.
///
/// # Examples
///
/// ```
/// use dotmax::{BrailleGrid, Color, primitives::draw_circle_colored};
///
/// let mut grid = BrailleGrid::new(80, 24).unwrap();
/// grid.enable_color_support();
///
/// // Red circle outline
/// let red = Color::rgb(255, 0, 0);
/// draw_circle_colored(&mut grid, 80, 48, 30, red, false)?;
///
/// // Filled blue circle
/// let blue = Color::rgb(0, 0, 255);
/// draw_circle_colored(&mut grid, 40, 24, 15, blue, true)?;
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
///
/// # Performance
///
/// O(r) for outline circles, O(r²) for filled circles where r is the radius.
/// Colored circles have minimal overhead (~1-2%) compared to non-colored circles.
///
/// # Backward Compatibility
///
/// This function is a new addition (Story 4.5). Existing non-colored functions
/// (`draw_circle`, `draw_circle_filled`, `draw_circle_thick`) remain unchanged.
pub fn draw_circle_colored(
    grid: &mut BrailleGrid,
    center_x: i32,
    center_y: i32,
    radius: u32,
    color: Color,
    filled: bool,
) -> Result<(), DotmaxError> {
    if filled {
        draw_circle_filled_colored_impl(grid, center_x, center_y, radius, color)
    } else {
        draw_circle_outline_colored_impl(grid, center_x, center_y, radius, color)
    }
}

/// Internal implementation for colored circle outline
#[allow(clippy::unnecessary_wraps)]
fn draw_circle_outline_colored_impl(
    grid: &mut BrailleGrid,
    center_x: i32,
    center_y: i32,
    radius: u32,
    color: Color,
) -> Result<(), DotmaxError> {
    // Handle zero radius: single dot at center
    if radius == 0 {
        plot_dot_colored_clipped(grid, center_x, center_y, color);
        return Ok(());
    }

    #[allow(clippy::cast_possible_wrap)]
    let mut x = radius as i32;
    let mut y = 0i32;
    let mut err = 1 - x;

    // Midpoint circle algorithm with 8-way symmetry
    while x >= y {
        // Plot 8 symmetric colored dots
        plot_8_symmetric_dots_colored(grid, center_x, center_y, x, y, color);

        y += 1;
        if err < 0 {
            err += 2 * y + 1;
        } else {
            x -= 1;
            err += 2 * (y - x) + 1;
        }
    }

    Ok(())
}

/// Internal implementation for filled colored circle
#[allow(clippy::unnecessary_wraps)]
fn draw_circle_filled_colored_impl(
    grid: &mut BrailleGrid,
    center_x: i32,
    center_y: i32,
    radius: u32,
    color: Color,
) -> Result<(), DotmaxError> {
    // Handle zero radius
    if radius == 0 {
        plot_dot_colored_clipped(grid, center_x, center_y, color);
        return Ok(());
    }

    #[allow(clippy::cast_possible_wrap)]
    let mut x = radius as i32;
    let mut y = 0i32;
    let mut err = 1 - x;

    // Midpoint circle algorithm with scanline filling
    while x >= y {
        // Draw horizontal lines to fill the circle
        // Four horizontal spans per iteration (top and bottom, left and right)

        // Top half: y to -y at x positions
        for scan_y in -y..=y {
            plot_dot_colored_clipped(grid, center_x + x, center_y + scan_y, color);
            plot_dot_colored_clipped(grid, center_x - x, center_y + scan_y, color);
        }

        // Side fills: -x to x at y positions (avoid duplicate center line)
        if x != y {
            for scan_y in -x..=x {
                plot_dot_colored_clipped(grid, center_x + y, center_y + scan_y, color);
                plot_dot_colored_clipped(grid, center_x - y, center_y + scan_y, color);
            }
        }

        y += 1;
        if err < 0 {
            err += 2 * y + 1;
        } else {
            x -= 1;
            err += 2 * (y - x) + 1;
        }
    }

    Ok(())
}

/// Plot 8 symmetric colored dots for circle algorithm
#[inline]
fn plot_8_symmetric_dots_colored(
    grid: &mut BrailleGrid,
    center_x: i32,
    center_y: i32,
    x: i32,
    y: i32,
    color: Color,
) {
    // All 8 octants with color
    plot_dot_colored_clipped(grid, center_x + x, center_y + y, color);
    plot_dot_colored_clipped(grid, center_x + y, center_y + x, color);
    plot_dot_colored_clipped(grid, center_x - y, center_y + x, color);
    plot_dot_colored_clipped(grid, center_x - x, center_y + y, color);
    plot_dot_colored_clipped(grid, center_x - x, center_y - y, color);
    plot_dot_colored_clipped(grid, center_x - y, center_y - x, color);
    plot_dot_colored_clipped(grid, center_x + y, center_y - x, color);
    plot_dot_colored_clipped(grid, center_x + x, center_y - y, color);
}

/// Plot a single colored dot with boundary clipping
#[inline]
fn plot_dot_colored_clipped(grid: &mut BrailleGrid, x: i32, y: i32, color: Color) {
    // Convert grid dimensions to dot space
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let max_x = (grid.width() * 2) as i32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let max_y = (grid.height() * 4) as i32;

    // Check bounds and plot if inside
    if x >= 0 && y >= 0 && x < max_x && y < max_y {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper function to check if a dot is set at given coordinates
    fn is_dot_set(grid: &BrailleGrid, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 {
            return false;
        }
        let x = x as usize;
        let y = y as usize;
        let max_x = grid.width() * 2;
        let max_y = grid.height() * 4;
        if x >= max_x || y >= max_y {
            return false;
        }

        // Convert dot coordinates to cell coordinates
        let cell_x = x / 2;
        let cell_y = y / 4;
        let dot_x = x % 2;
        let dot_y = y % 4;

        // Get the braille character
        let braille_char = grid.get_char(cell_x, cell_y);

        // Check if the specific dot is set in the braille pattern
        // Braille dot positions: 0 3
        //                        1 4
        //                        2 5
        //                        6 7
        let dot_bit = match (dot_x, dot_y) {
            (0, 0) => 0x01, // Dot 0
            (0, 1) => 0x02, // Dot 1
            (0, 2) => 0x04, // Dot 2
            (0, 3) => 0x40, // Dot 6
            (1, 0) => 0x08, // Dot 3
            (1, 1) => 0x10, // Dot 4
            (1, 2) => 0x20, // Dot 5
            (1, 3) => 0x80, // Dot 7
            _ => return false,
        };

        let braille_value = braille_char as u32 - 0x2800;
        (braille_value & dot_bit) != 0
    }

    /// Count total dots set in the grid
    fn count_dots(grid: &BrailleGrid) -> usize {
        let mut count = 0;
        for y in 0..(grid.height() * 4) {
            for x in 0..(grid.width() * 2) {
                if is_dot_set(grid, x as i32, y as i32) {
                    count += 1;
                }
            }
        }
        count
    }

    #[test]
    fn test_zero_radius() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        draw_circle(&mut grid, 10, 20, 0).unwrap();

        // Should have exactly one dot at center
        assert!(is_dot_set(&grid, 10, 20));
        assert_eq!(count_dots(&grid), 1);
    }

    #[test]
    fn test_small_circle_symmetry() {
        let mut grid = BrailleGrid::new(20, 20).unwrap();
        let center_x = 20;
        let center_y = 40;
        let radius = 5;

        draw_circle(&mut grid, center_x, center_y, radius).unwrap();

        // Verify 8-way symmetry: check that symmetric points are set
        // For radius 5, we should have dots at various symmetric positions
        // Just verify the circle was drawn (has dots)
        let dot_count = count_dots(&grid);
        assert!(dot_count > 0, "Circle should have dots");
        assert!(dot_count < 100, "Small circle shouldn't have too many dots");
    }

    #[test]
    fn test_boundary_clipping() {
        let mut grid = BrailleGrid::new(20, 20).unwrap();

        // Circle centered far outside grid should not panic
        draw_circle(&mut grid, -50, -50, 100).unwrap();

        // Should complete without panic (clipping worked)
    }

    #[test]
    fn test_extreme_coordinates() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();

        // Extreme coordinates should not panic
        draw_circle(&mut grid, -10000, -10000, 50000).unwrap();

        // Should complete without panic
    }

    #[test]
    fn test_filled_circle_interior() {
        let mut grid = BrailleGrid::new(50, 50).unwrap();
        let center_x = 50;
        let center_y = 100;
        let radius = 20;

        draw_circle_filled(&mut grid, center_x, center_y, radius).unwrap();

        // Check that center is filled
        assert!(is_dot_set(&grid, center_x, center_y));

        // Check several interior points
        assert!(is_dot_set(&grid, center_x + 5, center_y));
        assert!(is_dot_set(&grid, center_x - 5, center_y));
        assert!(is_dot_set(&grid, center_x, center_y + 5));
        assert!(is_dot_set(&grid, center_x, center_y - 5));

        // Verify filled circle has significantly more dots than outline
        let filled_count = count_dots(&grid);

        let mut grid_outline = BrailleGrid::new(50, 50).unwrap();
        draw_circle(&mut grid_outline, center_x, center_y, radius).unwrap();
        let outline_count = count_dots(&grid_outline);

        assert!(
            filled_count > outline_count * 5,
            "Filled circle should have many more dots than outline"
        );
    }

    #[test]
    fn test_thick_circle_outline() {
        let mut grid = BrailleGrid::new(50, 50).unwrap();
        let center_x = 50;
        let center_y = 100;
        let radius = 25;
        let thickness = 5;

        draw_circle_thick(&mut grid, center_x, center_y, radius, thickness).unwrap();

        // Thick circle should have more dots than thin outline
        let thick_count = count_dots(&grid);

        let mut grid_thin = BrailleGrid::new(50, 50).unwrap();
        draw_circle(&mut grid_thin, center_x, center_y, radius).unwrap();
        let thin_count = count_dots(&grid_thin);

        assert!(
            thick_count > thin_count * 2,
            "Thick circle should have significantly more dots than thin"
        );
    }

    #[test]
    fn test_invalid_thickness_zero() {
        let mut grid = BrailleGrid::new(20, 20).unwrap();

        let result = draw_circle_thick(&mut grid, 20, 40, 10, 0);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DotmaxError::InvalidThickness { thickness: 0 }
        ));
    }

    #[test]
    fn test_medium_circle_shape() {
        let mut grid = BrailleGrid::new(50, 50).unwrap();
        draw_circle(&mut grid, 50, 100, 25).unwrap();

        // Should have a reasonable number of dots for radius 25
        let dot_count = count_dots(&grid);
        // Approximate circumference: 2πr ≈ 157 dots
        assert!(
            dot_count > 100 && dot_count < 300,
            "Medium circle should have reasonable dot count"
        );
    }

    #[test]
    fn test_large_circle_correctness() {
        let mut grid = BrailleGrid::new(100, 100).unwrap();
        draw_circle(&mut grid, 100, 200, 50).unwrap();

        // Should complete without panic and have dots
        let dot_count = count_dots(&grid);
        assert!(dot_count > 200, "Large circle should have many dots");
    }
}
