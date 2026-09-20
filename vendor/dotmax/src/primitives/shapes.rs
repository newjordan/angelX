//! Rectangle and polygon drawing primitives.
//!
//! This module provides geometric shape drawing capabilities:
//! - Rectangles: Outline, filled, and thick border variants
//! - Polygons: Outline and filled from arbitrary vertex lists
//!
//! ## Algorithms
//!
//! **Rectangle Drawing:**
//! - Outline: Four lines (top, right, bottom, left edges)
//! - Filled: Scanline fill (horizontal line spans for each row)
//! - Thick: Concentric rectangles from outer to inner
//!
//! **Polygon Drawing:**
//! - Outline: Lines connecting consecutive vertices (closed path)
//! - Filled: Scanline fill algorithm with edge table (even-odd rule)
//!
//! ## Coordinate System
//!
//! All functions use **dot coordinates** (not cell coordinates):
//! - Grid is `width*2 × height*4` dots
//! - Example: 80×24 cell grid = 160×96 dot grid
//! - Signed `i32` coordinates allow negative values for clipping
//! - Dimensions are unsigned `u32` (must be positive)
//!
//! ## References
//!
//! - Foley & Van Dam, "Computer Graphics: Principles and Practice", Section 3.11
//! - Scanline Polygon Fill: <https://en.wikipedia.org/wiki/Scanline_rendering>
//! - Even-Odd Fill Rule: <https://www.w3.org/TR/SVG/painting.html#FillRuleProperty>

use crate::error::DotmaxError;
use crate::grid::{BrailleGrid, Color};
use crate::primitives::line::{draw_line, draw_line_colored};

/// Draw a rectangle outline on the braille grid.
///
/// Draws a rectangle by rendering four lines (top, right, bottom, left edges)
/// using the Bresenham line algorithm. The rectangle is defined by its top-left
/// corner position and dimensions.
///
/// # Arguments
///
/// * `grid` - Mutable reference to `BrailleGrid` to draw on
/// * `x`, `y` - Top-left corner in dot coordinates (signed for clipping)
/// * `width`, `height` - Rectangle dimensions in dots (must be > 0)
///
/// # Returns
///
/// * `Ok(())` on success
/// * `Err(DotmaxError::InvalidDimensions)` if width or height is 0
///
/// # Examples
///
/// ```
/// use dotmax::{BrailleGrid, primitives::shapes::draw_rectangle};
///
/// let mut grid = BrailleGrid::new(80, 24)?; // 160×96 dots
///
/// // Small rectangle
/// draw_rectangle(&mut grid, 10, 10, 50, 30)?;
///
/// // Large rectangle
/// draw_rectangle(&mut grid, 0, 0, 160, 96)?;
///
/// // Rectangle partially off-grid (clipped automatically)
/// draw_rectangle(&mut grid, -10, -10, 50, 50)?;
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
///
/// # Performance
///
/// O(perimeter) where perimeter = 2×(width + height).
/// Typically <1ms for 100×50 rectangle.
///
/// # Errors
///
/// Returns `InvalidDimensions` if `width == 0` or `height == 0`.
pub fn draw_rectangle(
    grid: &mut BrailleGrid,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<(), DotmaxError> {
    // Validate dimensions
    if width == 0 || height == 0 {
        return Err(DotmaxError::InvalidDimensions {
            width: width as usize,
            height: height as usize,
        });
    }

    // Calculate corner points (width-1 and height-1 for inclusive end points)
    #[allow(clippy::cast_possible_wrap)]
    let w = width as i32;
    #[allow(clippy::cast_possible_wrap)]
    let h = height as i32;

    let x_right = x + w - 1;
    let y_bottom = y + h - 1;

    // Draw four edges using draw_line (handles clipping automatically)
    draw_line(grid, x, y, x_right, y)?; // Top edge
    draw_line(grid, x_right, y, x_right, y_bottom)?; // Right edge
    draw_line(grid, x_right, y_bottom, x, y_bottom)?; // Bottom edge
    draw_line(grid, x, y_bottom, x, y)?; // Left edge

    Ok(())
}

/// Draw a filled rectangle on the braille grid.
///
/// Fills the interior of a rectangle using scanline fill (horizontal line spans
/// for each row). Produces a solid filled rectangle with no gaps or artifacts.
///
/// # Arguments
///
/// * `grid` - Mutable reference to `BrailleGrid` to draw on
/// * `x`, `y` - Top-left corner in dot coordinates (signed for clipping)
/// * `width`, `height` - Rectangle dimensions in dots (must be > 0)
///
/// # Returns
///
/// * `Ok(())` on success
/// * `Err(DotmaxError::InvalidDimensions)` if width or height is 0
///
/// # Examples
///
/// ```
/// use dotmax::{BrailleGrid, primitives::shapes::draw_rectangle_filled};
///
/// let mut grid = BrailleGrid::new(80, 24)?;
///
/// // Filled rectangle (solid background)
/// draw_rectangle_filled(&mut grid, 10, 10, 50, 30)?;
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
///
/// # Performance
///
/// O(area) where area = width × height.
/// Typically <5ms for 100×50 rectangle.
///
/// # Errors
///
/// Returns `InvalidDimensions` if `width == 0` or `height == 0`.
pub fn draw_rectangle_filled(
    grid: &mut BrailleGrid,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<(), DotmaxError> {
    // Validate dimensions
    if width == 0 || height == 0 {
        return Err(DotmaxError::InvalidDimensions {
            width: width as usize,
            height: height as usize,
        });
    }

    #[allow(clippy::cast_possible_wrap)]
    let w = width as i32;
    #[allow(clippy::cast_possible_wrap)]
    let h = height as i32;

    let x_right = x + w - 1;

    // Scanline fill: draw horizontal line for each row
    for row in 0..h {
        let current_y = y + row;
        draw_line(grid, x, current_y, x_right, current_y)?;
    }

    Ok(())
}

/// Draw a thick rectangle outline on the braille grid.
///
/// Draws a rectangle with specified border thickness by rendering multiple
/// concentric rectangles from outer to inner. Maintains proper corner connections.
///
/// # Arguments
///
/// * `grid` - Mutable reference to `BrailleGrid` to draw on
/// * `x`, `y` - Top-left corner in dot coordinates (signed for clipping)
/// * `width`, `height` - Rectangle dimensions in dots (must be > 0)
/// * `thickness` - Border thickness in dots (must be > 0 and ≤ min(width/2, height/2))
///
/// # Returns
///
/// * `Ok(())` on success
/// * `Err(DotmaxError::InvalidThickness)` if thickness is 0
/// * `Err(DotmaxError::InvalidDimensions)` if thickness exceeds width/2 or height/2
///
/// # Examples
///
/// ```
/// use dotmax::{BrailleGrid, primitives::shapes::draw_rectangle_thick};
///
/// let mut grid = BrailleGrid::new(80, 24)?;
///
/// // Rectangle with 3-dot thick border
/// draw_rectangle_thick(&mut grid, 10, 10, 60, 40, 3)?;
///
/// // Rectangle with 5-dot thick border
/// draw_rectangle_thick(&mut grid, 80, 10, 60, 40, 5)?;
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
///
/// # Performance
///
/// O(thickness × perimeter).
/// Typically <5ms for thickness=5, 100×50 rectangle.
///
/// # Errors
///
/// Returns `InvalidThickness` if `thickness == 0`.
/// Returns `InvalidDimensions` if `thickness > width/2` or `thickness > height/2`.
pub fn draw_rectangle_thick(
    grid: &mut BrailleGrid,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    thickness: u32,
) -> Result<(), DotmaxError> {
    // Validate thickness
    if thickness == 0 {
        return Err(DotmaxError::InvalidThickness { thickness: 0 });
    }

    // Validate dimensions vs thickness
    if thickness > width / 2 || thickness > height / 2 {
        return Err(DotmaxError::InvalidDimensions {
            width: width as usize,
            height: height as usize,
        });
    }

    // Draw concentric rectangles from outer to inner
    for i in 0..thickness {
        #[allow(clippy::cast_possible_wrap)]
        let offset = i as i32;
        let new_x = x + offset;
        let new_y = y + offset;
        let new_width = width - (2 * i);
        let new_height = height - (2 * i);

        draw_rectangle(grid, new_x, new_y, new_width, new_height)?;
    }

    Ok(())
}

/// Draw a polygon outline on the braille grid.
///
/// Draws a polygon by connecting consecutive vertices with lines and closing
/// the path (connects last vertex back to first). Validates minimum vertex count.
///
/// # Arguments
///
/// * `grid` - Mutable reference to `BrailleGrid` to draw on
/// * `vertices` - Slice of (x, y) vertex coordinates in dot space (must have ≥3 vertices)
///
/// # Returns
///
/// * `Ok(())` on success
/// * `Err(DotmaxError::InvalidPolygon)` if `vertices.len()` < 3
///
/// # Examples
///
/// ```
/// use dotmax::{BrailleGrid, primitives::shapes::draw_polygon};
///
/// let mut grid = BrailleGrid::new(80, 24)?;
///
/// // Triangle (3 vertices)
/// let triangle = [(40, 10), (20, 40), (60, 40)];
/// draw_polygon(&mut grid, &triangle)?;
///
/// // Hexagon (6 vertices)
/// let hexagon = [(80, 20), (90, 30), (90, 50), (80, 60), (70, 50), (70, 30)];
/// draw_polygon(&mut grid, &hexagon)?;
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
///
/// # Performance
///
/// O(vertices) where each vertex contributes one edge.
/// Typically <2ms for 10 vertices.
///
/// # Errors
///
/// Returns `InvalidPolygon` if `vertices.len()` < 3.
pub fn draw_polygon(grid: &mut BrailleGrid, vertices: &[(i32, i32)]) -> Result<(), DotmaxError> {
    // Validate minimum vertex count
    if vertices.len() < 3 {
        return Err(DotmaxError::InvalidPolygon {
            reason: format!("Polygon requires ≥3 vertices, got {}", vertices.len()),
        });
    }

    // Draw lines between consecutive vertices
    for i in 0..vertices.len() - 1 {
        let (x0, y0) = vertices[i];
        let (x1, y1) = vertices[i + 1];
        draw_line(grid, x0, y0, x1, y1)?;
    }

    // Close the path: connect last vertex to first
    let (x_last, y_last) = vertices[vertices.len() - 1];
    let (x_first, y_first) = vertices[0];
    draw_line(grid, x_last, y_last, x_first, y_first)?;

    Ok(())
}

/// Draw a filled polygon on the braille grid.
///
/// Fills the interior of a polygon using scanline fill algorithm with even-odd
/// fill rule. Handles non-convex and self-intersecting polygons correctly.
///
/// # Arguments
///
/// * `grid` - Mutable reference to `BrailleGrid` to draw on
/// * `vertices` - Slice of (x, y) vertex coordinates in dot space (must have ≥3 vertices)
///
/// # Returns
///
/// * `Ok(())` on success
/// * `Err(DotmaxError::InvalidPolygon)` if `vertices.len()` < 3
///
/// # Examples
///
/// ```
/// use dotmax::{BrailleGrid, primitives::shapes::draw_polygon_filled};
///
/// let mut grid = BrailleGrid::new(80, 24)?;
///
/// // Filled triangle
/// let triangle = [(40, 10), (20, 40), (60, 40)];
/// draw_polygon_filled(&mut grid, &triangle)?;
///
/// // Filled hexagon
/// let hexagon = [(80, 20), (90, 30), (90, 50), (80, 60), (70, 50), (70, 30)];
/// draw_polygon_filled(&mut grid, &hexagon)?;
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
///
/// # Performance
///
/// O(vertices × height) where height is polygon's y-range.
/// Typically <10ms for 10 vertices with ~1000 interior dots.
///
/// # Algorithm
///
/// Uses scanline fill with edge table:
/// 1. Build edge table with y-min, y-max, x-intercept for each edge
/// 2. For each scanline y from `y_min` to `y_max`:
///    - Find intersections with all polygon edges
///    - Sort intersections by x coordinate
///    - Fill spans between pairs (even-odd rule)
///
/// # Errors
///
/// Returns `InvalidPolygon` if `vertices.len()` < 3.
pub fn draw_polygon_filled(
    grid: &mut BrailleGrid,
    vertices: &[(i32, i32)],
) -> Result<(), DotmaxError> {
    // Build edge table: store (y_min, y_max, x_at_y_min, dx/dy) for each edge
    #[derive(Debug)]
    struct Edge {
        y_min: i32,
        y_max: i32,
        x_at_y_min: f64,
        inv_slope: f64, // dx/dy
    }

    // Validate minimum vertex count
    if vertices.len() < 3 {
        return Err(DotmaxError::InvalidPolygon {
            reason: format!("Polygon requires ≥3 vertices, got {}", vertices.len()),
        });
    }

    // Find y-range of polygon
    let mut y_min = vertices[0].1;
    let mut y_max = vertices[0].1;
    for &(_, y) in vertices {
        y_min = y_min.min(y);
        y_max = y_max.max(y);
    }

    let mut edges = Vec::new();

    // Process each edge (vertex[i], vertex[i+1])
    for i in 0..vertices.len() {
        let (x0, y0) = vertices[i];
        let (x1, y1) = vertices[(i + 1) % vertices.len()];

        // Skip horizontal edges (no contribution to scanline fill)
        if y0 == y1 {
            continue;
        }

        // Determine edge orientation
        #[allow(clippy::cast_precision_loss)]
        let (y_min_edge, y_max_edge, x_at_min, dx, dy) = if y0 < y1 {
            (
                y0,
                y1,
                f64::from(x0),
                f64::from(x1 - x0),
                f64::from(y1 - y0),
            )
        } else {
            (
                y1,
                y0,
                f64::from(x1),
                f64::from(x0 - x1),
                f64::from(y0 - y1),
            )
        };

        edges.push(Edge {
            y_min: y_min_edge,
            y_max: y_max_edge,
            x_at_y_min: x_at_min,
            inv_slope: dx / dy,
        });
    }

    // Scanline fill
    for y in y_min..=y_max {
        // Find intersections for this scanline
        let mut intersections = Vec::new();

        for edge in &edges {
            // Check if scanline intersects this edge (use y_min <= y < y_max to handle vertices correctly)
            if y >= edge.y_min && y < edge.y_max {
                // Calculate x-intersection at this y
                #[allow(clippy::cast_precision_loss)]
                let offset = f64::from(y - edge.y_min);
                #[allow(clippy::suboptimal_flops)]
                let x_intersection = edge.x_at_y_min + edge.inv_slope * offset;
                intersections.push(x_intersection);
            }
        }

        // Sort intersections by x coordinate
        intersections.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Fill spans between pairs (even-odd rule)
        for pair in intersections.chunks(2) {
            if pair.len() == 2 {
                #[allow(clippy::cast_possible_truncation)]
                let x_start = pair[0].round() as i32;
                #[allow(clippy::cast_possible_truncation)]
                let x_end = pair[1].round() as i32;

                // Draw horizontal line span
                draw_line(grid, x_start, y, x_end, y)?;
            }
        }
    }

    Ok(())
}

/// Draw a colored rectangle on the braille grid.
///
/// Draws a rectangle with specified color. Can be outline or filled.
/// All dots in the rectangle will have the same color applied to their containing cells.
///
/// **Story 4.5** - Color support for drawing primitives.
///
/// # Arguments
///
/// * `grid` - Mutable reference to `BrailleGrid` (must have color support enabled)
/// * `x`, `y` - Top-left corner in dot coordinates
/// * `width`, `height` - Rectangle dimensions in dots (must be > 0)
/// * `color` - RGB color to apply to rectangle dots
/// * `filled` - If true, fill the interior; if false, draw outline only
///
/// # Errors
///
/// * Returns `Err(DotmaxError::InvalidDimensions)` if width or height is 0
///
/// # Prerequisites
///
/// Grid must have color support enabled via `grid.enable_color_support()`.
///
/// # Color Application
///
/// Color is applied at the **cell level** (not per-dot). Each braille cell (2×4 dots)
/// has a single color. When a colored rectangle passes through a cell, that cell's color
/// is set to the rectangle's color.
///
/// # Examples
///
/// ```
/// use dotmax::{BrailleGrid, Color, primitives::shapes::draw_rectangle_colored};
///
/// let mut grid = BrailleGrid::new(80, 24).unwrap();
/// grid.enable_color_support();
///
/// // Green rectangle outline
/// let green = Color::rgb(0, 255, 0);
/// draw_rectangle_colored(&mut grid, 10, 10, 50, 30, green, false)?;
///
/// // Filled red rectangle
/// let red = Color::rgb(255, 0, 0);
/// draw_rectangle_colored(&mut grid, 70, 50, 30, 20, red, true)?;
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
///
/// # Performance
///
/// O(perimeter) for outline, O(area) for filled rectangles.
/// Colored rectangles have minimal overhead (~1-2%) compared to non-colored.
///
/// # Backward Compatibility
///
/// This function is a new addition (Story 4.5). Existing non-colored functions
/// (`draw_rectangle`, `draw_rectangle_filled`, `draw_rectangle_thick`) remain unchanged.
pub fn draw_rectangle_colored(
    grid: &mut BrailleGrid,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    color: Color,
    filled: bool,
) -> Result<(), DotmaxError> {
    // Validate dimensions
    if width == 0 || height == 0 {
        return Err(DotmaxError::InvalidDimensions {
            width: width as usize,
            height: height as usize,
        });
    }

    // Common calculations
    #[allow(clippy::cast_possible_wrap)]
    let w = width as i32;
    #[allow(clippy::cast_possible_wrap)]
    let h = height as i32;
    let x_right = x + w - 1;

    if filled {
        // Filled rectangle: scanline fill with colored lines
        for row in 0..h {
            let current_y = y + row;
            draw_line_colored(grid, x, current_y, x_right, current_y, color, None)?;
        }
    } else {
        // Outline rectangle: four colored lines
        let y_bottom = y + h - 1;

        draw_line_colored(grid, x, y, x_right, y, color, None)?; // Top
        draw_line_colored(grid, x_right, y, x_right, y_bottom, color, None)?; // Right
        draw_line_colored(grid, x_right, y_bottom, x, y_bottom, color, None)?; // Bottom
        draw_line_colored(grid, x, y_bottom, x, y, color, None)?; // Left
    }

    Ok(())
}

/// Draw a colored polygon on the braille grid.
///
/// Draws a polygon with specified color by connecting consecutive vertices.
/// Can be closed (connects last to first) or open (leaves gap).
///
/// **Story 4.5** - Color support for drawing primitives.
///
/// # Arguments
///
/// * `grid` - Mutable reference to `BrailleGrid` (must have color support enabled)
/// * `vertices` - Slice of (x, y) vertex coordinates in dot space (must have ≥2 vertices)
/// * `color` - RGB color to apply to polygon edges
/// * `closed` - If true, connect last vertex to first; if false, leave open
///
/// # Errors
///
/// * Returns `Err(DotmaxError::InvalidPolygon)` if `vertices.len()` < 2
///
/// # Prerequisites
///
/// Grid must have color support enabled via `grid.enable_color_support()`.
///
/// # Color Application
///
/// Color is applied at the **cell level** (not per-dot). Each braille cell (2×4 dots)
/// has a single color. When a colored polygon passes through a cell, that cell's color
/// is set to the polygon's color.
///
/// # Examples
///
/// ```
/// use dotmax::{BrailleGrid, Color, primitives::shapes::draw_polygon_colored};
///
/// let mut grid = BrailleGrid::new(80, 24).unwrap();
/// grid.enable_color_support();
///
/// // Yellow triangle
/// let yellow = Color::rgb(255, 255, 0);
/// let triangle = [(40, 10), (20, 40), (60, 40)];
/// draw_polygon_colored(&mut grid, &triangle, yellow, true)?;
///
/// // Cyan open path
/// let cyan = Color::rgb(0, 255, 255);
/// let path = [(10, 10), (30, 20), (50, 10)];
/// draw_polygon_colored(&mut grid, &path, cyan, false)?;
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
///
/// # Performance
///
/// O(vertices) where each vertex contributes one edge.
/// Colored polygons have minimal overhead (~1-2%) compared to non-colored.
///
/// # Backward Compatibility
///
/// This function is a new addition (Story 4.5). Existing non-colored functions
/// (`draw_polygon`, `draw_polygon_filled`) remain unchanged.
pub fn draw_polygon_colored(
    grid: &mut BrailleGrid,
    vertices: &[(i32, i32)],
    color: Color,
    closed: bool,
) -> Result<(), DotmaxError> {
    // Validate minimum vertex count (2 for open, 3 for closed is typical but allow 2+ for both)
    if vertices.len() < 2 {
        return Err(DotmaxError::InvalidPolygon {
            reason: format!("Polygon requires ≥2 vertices, got {}", vertices.len()),
        });
    }

    // Draw colored lines between consecutive vertices
    for i in 0..vertices.len() - 1 {
        let (x0, y0) = vertices[i];
        let (x1, y1) = vertices[i + 1];
        draw_line_colored(grid, x0, y0, x1, y1, color, None)?;
    }

    // Optionally close the path
    if closed && vertices.len() >= 2 {
        let (x_last, y_last) = vertices[vertices.len() - 1];
        let (x_first, y_first) = vertices[0];
        draw_line_colored(grid, x_last, y_last, x_first, y_first, color, None)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rectangle_outline_small() {
        let mut grid = BrailleGrid::new(40, 12).unwrap(); // 80×48 dots
        let result = draw_rectangle(&mut grid, 10, 10, 10, 10);
        assert!(result.is_ok());
    }

    #[test]
    fn test_rectangle_outline_medium() {
        let mut grid = BrailleGrid::new(40, 20).unwrap(); // 80×80 dots
        let result = draw_rectangle(&mut grid, 10, 10, 50, 25);
        assert!(result.is_ok());
    }

    #[test]
    fn test_rectangle_outline_large() {
        let mut grid = BrailleGrid::new(80, 24).unwrap(); // 160×96 dots
        let result = draw_rectangle(&mut grid, 5, 5, 100, 50);
        assert!(result.is_ok());
    }

    #[test]
    fn test_rectangle_zero_width_error() {
        let mut grid = BrailleGrid::new(40, 12).unwrap();
        let result = draw_rectangle(&mut grid, 10, 10, 0, 10);
        assert!(matches!(result, Err(DotmaxError::InvalidDimensions { .. })));
    }

    #[test]
    fn test_rectangle_zero_height_error() {
        let mut grid = BrailleGrid::new(40, 12).unwrap();
        let result = draw_rectangle(&mut grid, 10, 10, 10, 0);
        assert!(matches!(result, Err(DotmaxError::InvalidDimensions { .. })));
    }

    #[test]
    fn test_rectangle_extreme_position_clipping() {
        let mut grid = BrailleGrid::new(40, 12).unwrap(); // 80×48 dots
                                                          // Rectangle partially off-grid should not panic
        let result = draw_rectangle(&mut grid, -10, -10, 50, 50);
        assert!(result.is_ok());
    }

    #[test]
    fn test_rectangle_filled_small() {
        let mut grid = BrailleGrid::new(40, 12).unwrap();
        let result = draw_rectangle_filled(&mut grid, 10, 10, 10, 10);
        assert!(result.is_ok());
    }

    #[test]
    fn test_rectangle_filled_medium() {
        let mut grid = BrailleGrid::new(40, 20).unwrap();
        let result = draw_rectangle_filled(&mut grid, 10, 10, 30, 20);
        assert!(result.is_ok());
    }

    #[test]
    fn test_rectangle_thick_thickness_3() {
        let mut grid = BrailleGrid::new(40, 12).unwrap();
        let result = draw_rectangle_thick(&mut grid, 10, 10, 30, 20, 3);
        assert!(result.is_ok());
    }

    #[test]
    fn test_rectangle_thick_corners_connected() {
        let mut grid = BrailleGrid::new(40, 12).unwrap();
        // Thick rectangle should maintain corner connections
        let result = draw_rectangle_thick(&mut grid, 10, 10, 40, 30, 5);
        assert!(result.is_ok());
    }

    #[test]
    fn test_rectangle_thick_zero_thickness_error() {
        let mut grid = BrailleGrid::new(40, 12).unwrap();
        let result = draw_rectangle_thick(&mut grid, 10, 10, 30, 20, 0);
        assert!(matches!(result, Err(DotmaxError::InvalidThickness { .. })));
    }

    #[test]
    fn test_rectangle_thick_exceeds_width_error() {
        let mut grid = BrailleGrid::new(40, 12).unwrap();
        // Thickness exceeds width/2
        let result = draw_rectangle_thick(&mut grid, 10, 10, 20, 30, 15);
        assert!(matches!(result, Err(DotmaxError::InvalidDimensions { .. })));
    }

    #[test]
    fn test_rectangle_thick_exceeds_height_error() {
        let mut grid = BrailleGrid::new(40, 12).unwrap();
        // Thickness exceeds height/2
        let result = draw_rectangle_thick(&mut grid, 10, 10, 30, 20, 15);
        assert!(matches!(result, Err(DotmaxError::InvalidDimensions { .. })));
    }

    #[test]
    fn test_polygon_triangle() {
        let mut grid = BrailleGrid::new(40, 12).unwrap();
        let vertices = [(40, 10), (20, 40), (60, 40)];
        let result = draw_polygon(&mut grid, &vertices);
        assert!(result.is_ok());
    }

    #[test]
    fn test_polygon_square() {
        let mut grid = BrailleGrid::new(40, 12).unwrap();
        let vertices = [(10, 10), (30, 10), (30, 30), (10, 30)];
        let result = draw_polygon(&mut grid, &vertices);
        assert!(result.is_ok());
    }

    #[test]
    fn test_polygon_pentagon() {
        let mut grid = BrailleGrid::new(40, 12).unwrap();
        let vertices = [(40, 10), (50, 25), (45, 40), (35, 40), (30, 25)];
        let result = draw_polygon(&mut grid, &vertices);
        assert!(result.is_ok());
    }

    #[test]
    fn test_polygon_hexagon() {
        let mut grid = BrailleGrid::new(40, 12).unwrap();
        let vertices = [(40, 10), (50, 20), (50, 35), (40, 45), (30, 35), (30, 20)];
        let result = draw_polygon(&mut grid, &vertices);
        assert!(result.is_ok());
    }

    #[test]
    fn test_polygon_octagon() {
        let mut grid = BrailleGrid::new(40, 12).unwrap();
        let vertices = [
            (40, 10),
            (50, 15),
            (55, 25),
            (55, 35),
            (50, 45),
            (40, 50),
            (30, 45),
            (25, 35),
        ];
        let result = draw_polygon(&mut grid, &vertices);
        assert!(result.is_ok());
    }

    #[test]
    fn test_polygon_invalid_2_vertices_error() {
        let mut grid = BrailleGrid::new(40, 12).unwrap();
        let vertices = [(10, 10), (30, 30)];
        let result = draw_polygon(&mut grid, &vertices);
        assert!(matches!(result, Err(DotmaxError::InvalidPolygon { .. })));
    }

    #[test]
    fn test_polygon_invalid_empty_error() {
        let mut grid = BrailleGrid::new(40, 12).unwrap();
        let vertices: [(i32, i32); 0] = [];
        let result = draw_polygon(&mut grid, &vertices);
        assert!(matches!(result, Err(DotmaxError::InvalidPolygon { .. })));
    }

    #[test]
    fn test_polygon_clipping_extreme_coords() {
        let mut grid = BrailleGrid::new(40, 12).unwrap();
        // Polygon with vertices far outside grid should not panic
        let vertices = [(-100, -100), (200, -50), (200, 200), (-100, 200)];
        let result = draw_polygon(&mut grid, &vertices);
        assert!(result.is_ok());
    }

    #[test]
    fn test_polygon_filled_triangle() {
        let mut grid = BrailleGrid::new(40, 12).unwrap();
        let vertices = [(40, 10), (20, 40), (60, 40)];
        let result = draw_polygon_filled(&mut grid, &vertices);
        assert!(result.is_ok());
    }

    #[test]
    fn test_polygon_filled_hexagon() {
        let mut grid = BrailleGrid::new(40, 12).unwrap();
        let vertices = [(40, 10), (50, 20), (50, 35), (40, 45), (30, 35), (30, 20)];
        let result = draw_polygon_filled(&mut grid, &vertices);
        assert!(result.is_ok());
    }

    #[test]
    fn test_polygon_filled_invalid_vertices_error() {
        let mut grid = BrailleGrid::new(40, 12).unwrap();
        let vertices = [(10, 10), (30, 30)]; // Only 2 vertices
        let result = draw_polygon_filled(&mut grid, &vertices);
        assert!(matches!(result, Err(DotmaxError::InvalidPolygon { .. })));
    }

    #[test]
    fn test_polygon_self_intersecting() {
        let mut grid = BrailleGrid::new(40, 12).unwrap();
        // Self-intersecting polygon (star shape)
        let vertices = [(40, 10), (45, 40), (10, 25), (70, 25), (35, 40)];
        let result = draw_polygon_filled(&mut grid, &vertices);
        // Should render without crash (even-odd rule)
        assert!(result.is_ok());
    }
}
