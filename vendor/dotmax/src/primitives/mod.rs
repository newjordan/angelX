//! Drawing primitives for braille graphics.
//!
//! This module provides geometric drawing capabilities using industry-standard algorithms:
//! - Lines: Bresenham's line algorithm (integer-only, all octants)
//! - Circles: Bresenham's circle algorithm (midpoint circle, 8-way symmetry)
//! - Rectangles: Outline, filled, and thick border variants
//! - Polygons: Outline and filled from arbitrary vertex lists
//!
//! All primitives operate on `BrailleGrid` using dot coordinates (not cell coordinates).
//! Grid is `width*2 × height*4` dots where each cell is 2×4 dots.
//!
//! # Examples
//!
//! ```
//! use dotmax::{BrailleGrid, primitives::{draw_line, draw_circle, shapes::draw_rectangle}};
//!
//! let mut grid = BrailleGrid::new(80, 24)?; // 160×96 dots
//! draw_line(&mut grid, 0, 0, 159, 95)?; // Diagonal line
//! draw_circle(&mut grid, 80, 48, 30)?; // Circle at center
//! draw_rectangle(&mut grid, 10, 10, 50, 30)?; // Rectangle
//! # Ok::<(), dotmax::DotmaxError>(())
//! ```

pub mod circle;
pub mod line;
pub mod shapes;

pub use circle::{draw_circle, draw_circle_colored, draw_circle_filled, draw_circle_thick};
pub use line::{draw_line, draw_line_colored, draw_line_thick};
pub use shapes::{
    draw_polygon, draw_polygon_colored, draw_polygon_filled, draw_rectangle,
    draw_rectangle_colored, draw_rectangle_filled, draw_rectangle_thick,
};
