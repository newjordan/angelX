//! Color scheme application to intensity buffers.
//!
//! This module provides functions to apply color schemes to intensity data,
//! enabling vibrant colored visualizations from grayscale intensity values.
//!
//! # Overview
//!
//! The color application pipeline transforms intensity values (0.0-1.0) into
//! RGB colors using a [`ColorScheme`]. This enables:
//!
//! - Heatmap visualizations
//! - Data-driven color gradients
//! - Audio visualizations with color
//! - Scientific data rendering
//! - Artistic effects based on intensity
//!
//! # Examples
//!
//! ## Apply Color Scheme to 2D Intensity Buffer
//!
//! ```
//! use dotmax::color::apply::apply_color_scheme;
//! use dotmax::color::schemes::heat_map;
//!
//! // Create a 3x3 intensity buffer
//! let intensities = vec![
//!     vec![0.0, 0.5, 1.0],
//!     vec![0.25, 0.5, 0.75],
//!     vec![0.0, 0.5, 1.0],
//! ];
//!
//! let scheme = heat_map();
//! let colors = apply_color_scheme(&intensities, &scheme);
//!
//! assert_eq!(colors.len(), 3);
//! assert_eq!(colors[0].len(), 3);
//! ```
//!
//! ## Apply Colors to `BrailleGrid`
//!
//! ```
//! use dotmax::{BrailleGrid, Color};
//! use dotmax::color::apply::apply_colors_to_grid;
//! use dotmax::color::schemes::rainbow;
//!
//! let mut grid = BrailleGrid::new(3, 3).unwrap();
//! let colors = vec![
//!     vec![Color::rgb(255, 0, 0), Color::rgb(0, 255, 0), Color::rgb(0, 0, 255)],
//!     vec![Color::rgb(255, 255, 0), Color::rgb(0, 255, 255), Color::rgb(255, 0, 255)],
//!     vec![Color::rgb(128, 128, 128), Color::rgb(64, 64, 64), Color::rgb(192, 192, 192)],
//! ];
//!
//! apply_colors_to_grid(&mut grid, &colors).unwrap();
//! assert_eq!(grid.get_color(0, 0), Some(Color::rgb(255, 0, 0)));
//! ```
//!
//! # Integration with Epic 3 Image Pipeline
//!
//! The existing image pipeline produces grayscale intensity buffers:
//!
//! 1. Load image → Resize → Grayscale → produces intensity buffer
//! 2. Apply color scheme → intensity buffer becomes colored
//! 3. Render colored `BrailleGrid` to terminal
//!
//! # Performance
//!
//! - `apply_color_scheme()`: <10ms for 80×24 grid, <100ms for 200×50 grid
//! - Zero allocations in hot path except output buffer creation

use crate::color::schemes::ColorScheme;
use crate::error::DotmaxError;
use crate::grid::{BrailleGrid, Color};

/// Apply a color scheme to a 2D intensity buffer.
///
/// Maps each intensity value (0.0-1.0) to an RGB color using the scheme's
/// [`ColorScheme::sample`] method, producing a 2D color grid matching
/// the input dimensions.
///
/// # Arguments
///
/// * `intensities` - 2D buffer of intensity values (row-major order)
/// * `scheme` - Color scheme to use for mapping
///
/// # Returns
///
/// A 2D vector of colors with the same dimensions as the input.
///
/// # Intensity Handling
///
/// - Values in range 0.0-1.0 are mapped normally
/// - Values outside range are clamped (consistent with `ColorScheme::sample`)
/// - NaN values are treated as 0.0
/// - Infinity values are clamped to 0.0 or 1.0
///
/// # Examples
///
/// ```
/// use dotmax::color::apply::apply_color_scheme;
/// use dotmax::color::schemes::grayscale;
///
/// let intensities = vec![
///     vec![0.0, 0.5, 1.0],
/// ];
///
/// let scheme = grayscale();
/// let colors = apply_color_scheme(&intensities, &scheme);
///
/// // First color is black (intensity 0.0)
/// assert_eq!(colors[0][0].r, 0);
/// // Last color is white (intensity 1.0)
/// assert_eq!(colors[0][2].r, 255);
/// ```
///
/// # Performance
///
/// Target: <10ms for 80×24 grid (1,920 cells), <100ms for 200×50 grid (10,000 cells)
#[must_use]
pub fn apply_color_scheme(intensities: &[Vec<f32>], scheme: &ColorScheme) -> Vec<Vec<Color>> {
    // Handle empty input
    if intensities.is_empty() {
        return Vec::new();
    }

    // Pre-allocate with correct capacity
    let mut result = Vec::with_capacity(intensities.len());

    for row in intensities {
        let mut color_row = Vec::with_capacity(row.len());
        for &intensity in row {
            // Handle special float values
            let normalized = normalize_intensity(intensity);
            color_row.push(scheme.sample(normalized));
        }
        result.push(color_row);
    }

    result
}

/// Apply a 2D color grid to a `BrailleGrid`.
///
/// Sets the color for each cell in the grid using the corresponding color
/// from the color grid. The color grid dimensions must match the grid dimensions.
///
/// # Arguments
///
/// * `grid` - The `BrailleGrid` to apply colors to
/// * `color_grid` - 2D vector of colors (row-major, `color_grid[y][x]`)
///
/// # Returns
///
/// * `Ok(())` if colors were applied successfully
/// * `Err(DotmaxError::BufferSizeMismatch)` if dimensions don't match
///
/// # Examples
///
/// ```
/// use dotmax::{BrailleGrid, Color};
/// use dotmax::color::apply::apply_colors_to_grid;
///
/// let mut grid = BrailleGrid::new(2, 2).unwrap();
/// let colors = vec![
///     vec![Color::rgb(255, 0, 0), Color::rgb(0, 255, 0)],
///     vec![Color::rgb(0, 0, 255), Color::rgb(255, 255, 0)],
/// ];
///
/// apply_colors_to_grid(&mut grid, &colors).unwrap();
/// assert_eq!(grid.get_color(0, 0), Some(Color::rgb(255, 0, 0)));
/// assert_eq!(grid.get_color(1, 1), Some(Color::rgb(255, 255, 0)));
/// ```
///
/// # Errors
///
/// Returns [`DotmaxError::BufferSizeMismatch`] if:
/// - `color_grid.len() != grid.height()`
/// - Any `color_grid[y].len() != grid.width()`
pub fn apply_colors_to_grid(
    grid: &mut BrailleGrid,
    color_grid: &[Vec<Color>],
) -> Result<(), DotmaxError> {
    let (width, height) = grid.dimensions();

    // Validate height
    if color_grid.len() != height {
        return Err(DotmaxError::BufferSizeMismatch {
            expected: height,
            actual: color_grid.len(),
        });
    }

    // Validate width and apply colors
    for (y, row) in color_grid.iter().enumerate() {
        if row.len() != width {
            return Err(DotmaxError::BufferSizeMismatch {
                expected: width,
                actual: row.len(),
            });
        }

        for (x, &color) in row.iter().enumerate() {
            // set_cell_color is guaranteed to succeed since we validated bounds
            grid.set_cell_color(x, y, color)?;
        }
    }

    Ok(())
}

/// Normalize an intensity value, handling special float cases.
///
/// - Clamps values to 0.0-1.0 range
/// - Treats NaN as 0.0
/// - Clamps +Infinity to 1.0
/// - Clamps -Infinity to 0.0
#[inline]
fn normalize_intensity(intensity: f32) -> f32 {
    if intensity.is_nan() {
        0.0
    } else if intensity.is_infinite() {
        if intensity.is_sign_positive() {
            1.0
        } else {
            0.0
        }
    } else {
        intensity.clamp(0.0, 1.0)
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::schemes::{
        blue_purple, cyan_magenta, grayscale, green_yellow, heat_map, monochrome, rainbow,
    };

    // ========================================================================
    // AC1: apply_color_scheme Tests
    // ========================================================================

    #[test]
    fn test_apply_color_scheme_empty_input() {
        let intensities: Vec<Vec<f32>> = vec![];
        let scheme = grayscale();
        let colors = apply_color_scheme(&intensities, &scheme);
        assert!(colors.is_empty());
    }

    #[test]
    fn test_apply_color_scheme_1x1() {
        let intensities = vec![vec![0.5]];
        let scheme = grayscale();
        let colors = apply_color_scheme(&intensities, &scheme);

        assert_eq!(colors.len(), 1);
        assert_eq!(colors[0].len(), 1);
        // Gray at 0.5 should be around 128
        assert!(colors[0][0].r >= 127 && colors[0][0].r <= 128);
    }

    #[test]
    fn test_apply_color_scheme_10x10() {
        let intensities: Vec<Vec<f32>> = (0..10)
            .map(|_| (0..10).map(|x| x as f32 / 10.0).collect())
            .collect();

        let scheme = heat_map();
        let colors = apply_color_scheme(&intensities, &scheme);

        assert_eq!(colors.len(), 10);
        assert_eq!(colors[0].len(), 10);
    }

    #[test]
    fn test_apply_color_scheme_80x24() {
        let intensities: Vec<Vec<f32>> = (0..24)
            .map(|y| {
                (0..80)
                    .map(|x| (x as f32 / 80.0 + y as f32 / 24.0) / 2.0)
                    .collect()
            })
            .collect();

        let scheme = rainbow();
        let colors = apply_color_scheme(&intensities, &scheme);

        assert_eq!(colors.len(), 24);
        assert_eq!(colors[0].len(), 80);
    }

    #[test]
    fn test_apply_color_scheme_boundary_values() {
        let intensities = vec![vec![0.0, 0.5, 1.0]];
        let scheme = grayscale();
        let colors = apply_color_scheme(&intensities, &scheme);

        // 0.0 -> black
        assert_eq!(colors[0][0], Color::black());
        // 1.0 -> white
        assert_eq!(colors[0][2], Color::white());
    }

    // ========================================================================
    // AC2: apply_colors_to_grid Tests
    // ========================================================================

    #[test]
    fn test_apply_colors_to_grid_success() {
        let mut grid = BrailleGrid::new(3, 2).unwrap();
        let colors = vec![
            vec![
                Color::rgb(255, 0, 0),
                Color::rgb(0, 255, 0),
                Color::rgb(0, 0, 255),
            ],
            vec![
                Color::rgb(255, 255, 0),
                Color::rgb(0, 255, 255),
                Color::rgb(255, 0, 255),
            ],
        ];

        apply_colors_to_grid(&mut grid, &colors).unwrap();

        assert_eq!(grid.get_color(0, 0), Some(Color::rgb(255, 0, 0)));
        assert_eq!(grid.get_color(1, 0), Some(Color::rgb(0, 255, 0)));
        assert_eq!(grid.get_color(2, 0), Some(Color::rgb(0, 0, 255)));
        assert_eq!(grid.get_color(0, 1), Some(Color::rgb(255, 255, 0)));
        assert_eq!(grid.get_color(1, 1), Some(Color::rgb(0, 255, 255)));
        assert_eq!(grid.get_color(2, 1), Some(Color::rgb(255, 0, 255)));
    }

    #[test]
    fn test_apply_colors_to_grid_height_mismatch() {
        let mut grid = BrailleGrid::new(3, 3).unwrap();
        let colors = vec![
            vec![Color::black(), Color::black(), Color::black()],
            // Missing row
        ];

        let result = apply_colors_to_grid(&mut grid, &colors);
        assert!(matches!(
            result,
            Err(DotmaxError::BufferSizeMismatch { .. })
        ));
    }

    #[test]
    fn test_apply_colors_to_grid_width_mismatch() {
        let mut grid = BrailleGrid::new(3, 2).unwrap();
        let colors = vec![
            vec![Color::black(), Color::black()], // Missing column
            vec![Color::black(), Color::black(), Color::black()],
        ];

        let result = apply_colors_to_grid(&mut grid, &colors);
        assert!(matches!(
            result,
            Err(DotmaxError::BufferSizeMismatch { .. })
        ));
    }

    #[test]
    fn test_apply_colors_to_grid_empty() {
        let mut grid = BrailleGrid::new(3, 3).unwrap();
        let colors: Vec<Vec<Color>> = vec![];

        let result = apply_colors_to_grid(&mut grid, &colors);
        assert!(matches!(
            result,
            Err(DotmaxError::BufferSizeMismatch { .. })
        ));
    }

    // ========================================================================
    // AC4: Intensity Clamping Tests
    // ========================================================================

    #[test]
    fn test_intensity_clamping_negative() {
        let intensities = vec![vec![-0.5, -1.0, -100.0]];
        let scheme = grayscale();
        let colors = apply_color_scheme(&intensities, &scheme);

        // All negative values should clamp to 0.0 -> black
        for color in &colors[0] {
            assert_eq!(*color, Color::black());
        }
    }

    #[test]
    fn test_intensity_clamping_above_one() {
        let intensities = vec![vec![1.5, 2.0, 100.0]];
        let scheme = grayscale();
        let colors = apply_color_scheme(&intensities, &scheme);

        // All values > 1.0 should clamp to 1.0 -> white
        for color in &colors[0] {
            assert_eq!(*color, Color::white());
        }
    }

    #[test]
    fn test_intensity_nan_handling() {
        let intensities = vec![vec![f32::NAN]];
        let scheme = grayscale();
        let colors = apply_color_scheme(&intensities, &scheme);

        // NaN should be treated as 0.0 -> black
        assert_eq!(colors[0][0], Color::black());
    }

    #[test]
    fn test_intensity_infinity_handling() {
        let intensities = vec![vec![f32::INFINITY, f32::NEG_INFINITY]];
        let scheme = grayscale();
        let colors = apply_color_scheme(&intensities, &scheme);

        // +Infinity -> 1.0 -> white
        assert_eq!(colors[0][0], Color::white());
        // -Infinity -> 0.0 -> black
        assert_eq!(colors[0][1], Color::black());
    }

    // ========================================================================
    // AC7: Integration with All Predefined Schemes
    // ========================================================================

    #[test]
    fn test_all_schemes_produce_valid_colors() {
        let intensities = vec![vec![0.0, 0.25, 0.5, 0.75, 1.0]];
        let schemes = vec![
            rainbow(),
            heat_map(),
            blue_purple(),
            green_yellow(),
            cyan_magenta(),
            grayscale(),
            monochrome(),
        ];

        for scheme in schemes {
            let colors = apply_color_scheme(&intensities, &scheme);
            assert_eq!(colors.len(), 1);
            assert_eq!(colors[0].len(), 5);
            // Just verify no panics and colors are valid (u8 values)
        }
    }

    #[test]
    fn test_rainbow_scheme_produces_spectrum() {
        let intensities = vec![vec![0.0, 0.5, 1.0]];
        let scheme = rainbow();
        let colors = apply_color_scheme(&intensities, &scheme);

        // 0.0 should be red
        let red = &colors[0][0];
        assert_eq!(red.r, 255);
        assert_eq!(red.g, 0);
        assert_eq!(red.b, 0);

        // 1.0 should be purple-ish
        let purple = &colors[0][2];
        assert!(purple.r > 200);
        assert!(purple.b > 200);
    }

    #[test]
    fn test_heat_map_scheme_thermal_gradient() {
        let intensities = vec![vec![0.0, 1.0]];
        let scheme = heat_map();
        let colors = apply_color_scheme(&intensities, &scheme);

        // 0.0 -> black (cold)
        assert_eq!(colors[0][0], Color::black());
        // 1.0 -> white (hot)
        assert_eq!(colors[0][1], Color::white());
    }

    // ========================================================================
    // Helper Function Tests
    // ========================================================================

    #[test]
    fn test_normalize_intensity_normal_range() {
        assert_eq!(normalize_intensity(0.0), 0.0);
        assert_eq!(normalize_intensity(0.5), 0.5);
        assert_eq!(normalize_intensity(1.0), 1.0);
    }

    #[test]
    fn test_normalize_intensity_clamping() {
        assert_eq!(normalize_intensity(-0.5), 0.0);
        assert_eq!(normalize_intensity(1.5), 1.0);
    }

    #[test]
    fn test_normalize_intensity_special_values() {
        assert_eq!(normalize_intensity(f32::NAN), 0.0);
        assert_eq!(normalize_intensity(f32::INFINITY), 1.0);
        assert_eq!(normalize_intensity(f32::NEG_INFINITY), 0.0);
    }

    // ========================================================================
    // Integration Test: End-to-End Pipeline
    // ========================================================================

    #[test]
    fn test_full_pipeline_intensity_to_grid() {
        // Create 5x5 intensity buffer with gradient
        let intensities: Vec<Vec<f32>> = (0..5)
            .map(|y| (0..5).map(|x| (x + y) as f32 / 8.0).collect())
            .collect();

        // Apply rainbow color scheme
        let scheme = rainbow();
        let colors = apply_color_scheme(&intensities, &scheme);

        // Apply to grid
        let mut grid = BrailleGrid::new(5, 5).unwrap();
        apply_colors_to_grid(&mut grid, &colors).unwrap();

        // Verify all cells have colors
        for y in 0..5 {
            for x in 0..5 {
                assert!(grid.get_color(x, y).is_some());
            }
        }
    }
}
