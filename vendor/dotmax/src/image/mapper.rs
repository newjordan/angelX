//! Pixel-to-braille mapping module (feature-gated)
//!
//! This module provides functionality to convert binary pixel data to braille dot patterns.
//! It implements the final stage of the image-to-braille rendering pipeline by mapping 2×4
//! pixel blocks to individual braille cells.
//!
//! # Feature Gate
//!
//! This module is behind the `image` feature flag and will only be compiled when the feature is enabled.
//!
//! # Algorithm
//!
//! The mapping algorithm divides a binary image into 2×4 pixel blocks, where each block
//! corresponds to one braille cell. Each pixel in the block maps to a specific dot position
//! in the braille cell following the Unicode braille standard (U+2800-U+28FF).
//!
//! ## Pixel Block to Braille Cell Mapping
//!
//! ```text
//! Pixel Block (2 wide × 4 tall):     Braille Cell (8 dots):
//! ┌───┬───┐                          ┌─┬─┐
//! │0,0│1,0│  →  Dots 1, 4  →         │•│ │  Unicode Braille:
//! ├───┼───┤                          ├─┼─┤  Bit 0 (0x01): dot 1 (top-left)
//! │0,1│1,1│  →  Dots 2, 5  →         │•│•│  Bit 1 (0x02): dot 2
//! ├───┼───┤                          ├─┼─┤  Bit 2 (0x04): dot 3
//! │0,2│1,2│  →  Dots 3, 6  →         │ │•│  Bit 3 (0x08): dot 4 (top-right)
//! ├───┼───┤                          └─┴─┘  Bit 4 (0x10): dot 5
//! │0,3│1,3│  →  Dots 7, 8  →                Bit 5 (0x20): dot 6
//! └───┴───┘                                 Bit 6 (0x40): dot 7 (bottom-left)
//!                                           Bit 7 (0x80): dot 8 (bottom-right)
//! ```
//!
//! ## Padding Strategy
//!
//! Images not perfectly divisible by 2×4 are padded with white pixels (dot OFF) on the
//! bottom and right edges to complete the final cells.
//!
//! ## Examples
//!
//! ```no_run
//! use dotmax::image::{auto_threshold, load_from_path};
//! use dotmax::image::mapper::pixels_to_braille;
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Load and process image
//! let img = load_from_path(Path::new("image.png"))?;
//! let binary = auto_threshold(&img);
//!
//! // Map to braille grid
//! let grid = pixels_to_braille(&binary, 80, 24)?;
//!
//! // Grid is now ready for rendering
//! println!("Created {}×{} braille grid", grid.width(), grid.height());
//! # Ok(())
//! # }
//! ```
//!
//! # Performance
//!
//! The braille mapping targets <10ms for standard terminal sizes (160×96 pixels = 80×24 cells).
//! The algorithm performs a direct pixel→dot conversion with no intermediate buffers.

use tracing::{debug, info};

use crate::error::DotmaxError;
use crate::grid::BrailleGrid;

use super::threshold::BinaryImage;

/// Convert a binary image to a braille grid using 2×4 pixel block mapping.
///
/// This function takes a binary image (pixels represented as boolean values where `true` = black,
/// `false` = white) and converts it to a [`BrailleGrid`] by mapping each 2×4 pixel block to a
/// single braille cell. Each pixel in the block corresponds to a specific dot position in the
/// braille cell following the Unicode braille standard.
///
/// # Algorithm
///
/// 1. Validate input dimensions (must be non-zero)
/// 2. Calculate grid dimensions using ceiling division (padding for incomplete cells)
/// 3. Create output [`BrailleGrid`] with calculated dimensions
/// 4. Iterate over each cell position in the grid:
///    - For each 2×4 pixel block within the cell:
///      - Map pixel value to dot value (black pixel → dot ON, white pixel → dot OFF)
///      - Call [`BrailleGrid::set_dot`] to set the dot in the grid
///      - Handle padding for pixels outside image bounds (default to white/OFF)
/// 5. Return populated grid ready for terminal rendering
///
/// # Pixel-to-Dot Coordinate Mapping
///
/// Pixels map 1:1 to dots in coordinate space. The [`BrailleGrid`] handles the internal
/// cell-to-dot conversion. Pixel at (x, y) maps directly to dot at (x, y).
///
/// # Padding
///
/// Images not divisible by 2×4 are padded with white pixels (false) on the bottom and right
/// edges to complete the final cells. For example:
/// - 5×5 image → padded to 6×8 → 3×2 braille grid (3 cells wide, 2 cells tall)
/// - 160×100 image → padded to 160×100 → 80×25 braille grid
///
/// # Arguments
///
/// * `binary` - The binary image to convert (pixels as `Vec<bool>`, true=black, false=white)
/// * `cell_width` - Target grid width in braille cells (not used for calculation, reserved for future optimization)
/// * `cell_height` - Target grid height in braille cells (not used for calculation, reserved for future optimization)
///
/// # Returns
///
/// Returns `Ok(BrailleGrid)` if conversion succeeds, or a [`DotmaxError`] if:
/// - Image dimensions are zero (0×0)
/// - Grid creation fails
///
/// # Errors
///
/// - [`DotmaxError::InvalidImageDimensions`] - Image width or height is zero
/// - [`DotmaxError::InvalidParameter`] - `cell_width` or `cell_height` is zero (reserved, not currently validated)
///
/// # Performance
///
/// Target: <10ms for 160×96 pixel image (80×24 braille grid).
/// Actual performance scales linearly with image size.
///
/// # Examples
///
/// ```no_run
/// use dotmax::image::{auto_threshold, load_from_path};
/// use dotmax::image::mapper::pixels_to_braille;
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Load and threshold image
/// let img = load_from_path(Path::new("photo.png"))?;
/// let binary = auto_threshold(&img);
///
/// // Map to braille (80×24 cells)
/// let grid = pixels_to_braille(&binary, 80, 24)?;
///
/// assert_eq!(grid.width(), ((binary.width + 1) / 2) as usize); // Ceiling division
/// assert_eq!(grid.height(), ((binary.height + 3) / 4) as usize); // Ceiling division
/// # Ok(())
/// # }
/// ```
#[allow(clippy::similar_names)] // Intentional use of similar coordinate variable names (x/y pairs)
#[allow(clippy::cast_possible_truncation)] // Safe: dimensions bounded by BrailleGrid validation
pub fn pixels_to_braille(
    binary: &BinaryImage,
    _cell_width: usize,
    _cell_height: usize,
) -> Result<BrailleGrid, DotmaxError> {
    // Validate: binary image must not be empty
    if binary.width == 0 || binary.height == 0 {
        return Err(DotmaxError::InvalidImageDimensions {
            width: binary.width,
            height: binary.height,
        });
    }

    // Calculate grid dimensions (ceiling division for padding)
    // Each braille cell is 2 dots wide and 4 dots tall
    let grid_width = ((binary.width + 1) / 2) as usize;
    let grid_height = ((binary.height + 3) / 4) as usize;

    info!(
        "Mapping {}×{} binary image to {}×{} braille grid",
        binary.width, binary.height, grid_width, grid_height
    );

    // Create output grid with calculated dimensions
    let mut grid = BrailleGrid::new(grid_width, grid_height)?;

    debug!(
        "Created BrailleGrid with dimensions {}×{}",
        grid_width, grid_height
    );

    // Iterate over each cell in the grid
    for cell_y in 0..grid_height {
        for cell_x in 0..grid_width {
            // Calculate pixel block top-left corner for this cell
            let pixel_x_start = (cell_x * 2) as u32;
            let pixel_y_start = (cell_y * 4) as u32;

            // Iterate 2×4 block within cell (2 columns, 4 rows of dots)
            for dot_y in 0..4 {
                for dot_x in 0..2 {
                    // Calculate absolute pixel position
                    let pixel_x = pixel_x_start + dot_x;
                    let pixel_y = pixel_y_start + dot_y;

                    // Get pixel value (or default to white if outside bounds for padding)
                    let pixel_value = if pixel_x < binary.width && pixel_y < binary.height {
                        // Pixel is within image bounds, get actual value
                        let pixel_index = (pixel_y * binary.width + pixel_x) as usize;
                        binary.pixels[pixel_index]
                    } else {
                        // Pixel is outside bounds (padding area), default to white (dot OFF)
                        false
                    };

                    // Calculate absolute dot position in grid
                    // Pixel (x, y) maps 1:1 to dot (x, y) - BrailleGrid handles cell conversion
                    let dot_x_abs = pixel_x as usize;
                    let dot_y_abs = pixel_y as usize;

                    // Set dot in grid (BrailleGrid::set_dot handles Unicode bit mapping)
                    // Only set the dot if pixel_value is true (black pixel = dot ON)
                    if pixel_value {
                        grid.set_dot(dot_x_abs, dot_y_abs)?;
                    }
                }
            }
        }
    }

    info!(
        "Braille mapping complete: {}×{} grid with {} total dots",
        grid.width(),
        grid.height(),
        grid.width() * grid.height() * 8
    );

    Ok(grid)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper function to create a test binary image
    fn create_test_image(width: u32, height: u32, pixels: Vec<bool>) -> BinaryImage {
        assert_eq!(
            pixels.len(),
            (width * height) as usize,
            "Pixel count must match width × height"
        );
        BinaryImage {
            width,
            height,
            pixels,
        }
    }

    #[test]
    fn test_empty_image_returns_error() {
        let binary = BinaryImage {
            width: 0,
            height: 0,
            pixels: vec![],
        };

        let result = pixels_to_braille(&binary, 0, 0);

        assert!(result.is_err());
        match result {
            Err(DotmaxError::InvalidImageDimensions { width, height }) => {
                assert_eq!(width, 0);
                assert_eq!(height, 0);
            }
            _ => panic!("Expected InvalidImageDimensions error"),
        }
    }

    #[test]
    fn test_all_black_2x4_block_all_dots_on() {
        // Create 2×4 image with all black pixels
        let pixels = vec![true; 8]; // 2×4 = 8 pixels
        let binary = create_test_image(2, 4, pixels);

        let grid = pixels_to_braille(&binary, 1, 1).unwrap();

        // Grid should be 1×1 (one cell for 2×4 block)
        assert_eq!(grid.width(), 1);
        assert_eq!(grid.height(), 1);

        // Verify all 8 dots are ON (braille character with all dots = U+28FF)
        let ch = grid.get_char(0, 0);
        assert_eq!(
            ch, '\u{28FF}',
            "All black pixels should produce U+28FF (all dots on)"
        );
    }

    #[test]
    fn test_all_white_2x4_block_all_dots_off() {
        // Create 2×4 image with all white pixels
        let pixels = vec![false; 8]; // 2×4 = 8 pixels
        let binary = create_test_image(2, 4, pixels);

        let grid = pixels_to_braille(&binary, 1, 1).unwrap();

        // Grid should be 1×1 (one cell for 2×4 block)
        assert_eq!(grid.width(), 1);
        assert_eq!(grid.height(), 1);

        // Verify all 8 dots are OFF (braille blank = U+2800)
        let ch = grid.get_char(0, 0);
        assert_eq!(
            ch, '\u{2800}',
            "All white pixels should produce U+2800 (blank braille)"
        );
    }

    #[test]
    fn test_single_pixel_1x1_image() {
        // Create 1×1 image with single black pixel
        let pixels = vec![true];
        let binary = create_test_image(1, 1, pixels);

        let grid = pixels_to_braille(&binary, 1, 1).unwrap();

        // Grid should be 1×1 (ceiling division: (1+1)/2 = 1, (1+3)/4 = 1)
        assert_eq!(grid.width(), 1);
        assert_eq!(grid.height(), 1);

        // Only top-left dot (0,0) should be ON (Dot 1 = U+2800 + 0x01 = U+2801)
        let ch = grid.get_char(0, 0);
        assert_eq!(
            ch, '\u{2801}',
            "Single pixel at (0,0) should produce U+2801 (dot 1 only)"
        );
    }

    #[test]
    fn test_padding_5x5_image() {
        // Create 5×5 image (not divisible by 2 or 4)
        // Should pad to 6×8 → 3×2 grid
        let pixels = vec![true; 25]; // 5×5 = 25 pixels (all black)
        let binary = create_test_image(5, 5, pixels);

        let grid = pixels_to_braille(&binary, 3, 2).unwrap();

        // Grid dimensions: width = (5+1)/2 = 3, height = (5+3)/4 = 2
        assert_eq!(grid.width(), 3);
        assert_eq!(grid.height(), 2);

        // Top-left cell (0,0) should have all original pixels (2×4) = all black = U+28FF
        let ch_00 = grid.get_char(0, 0);
        assert_eq!(ch_00, '\u{28FF}', "Cell (0,0) should have all dots on");

        // Top-right cell (2,0) should have partial pixels (only left column is real, right is padding)
        // Left column (x=4): 4 pixels all black = dots 1,2,3,7 = 0x01 + 0x02 + 0x04 + 0x40 = 0x47
        let ch_20 = grid.get_char(2, 0);
        assert_eq!(ch_20, '\u{2847}', "Cell (2,0) should have left column only");

        // Bottom-left cell (0,1) should have top row only (y=4, pixels 0-1) = dots 1,4 = 0x01 + 0x08 = 0x09
        let ch_01 = grid.get_char(0, 1);
        assert_eq!(ch_01, '\u{2809}', "Cell (0,1) should have top row only");
    }

    #[test]
    fn test_pixel_0_0_maps_to_dot_1() {
        // Create 2×4 image with only pixel (0,0) black
        let mut pixels = vec![false; 8];
        pixels[0] = true; // (0,0) = index 0

        let binary = create_test_image(2, 4, pixels);
        let grid = pixels_to_braille(&binary, 1, 1).unwrap();

        // Pixel (0,0) maps to dot 1 = U+2800 + 0x01 = U+2801
        let ch = grid.get_char(0, 0);
        assert_eq!(ch, '\u{2801}', "Pixel (0,0) should map to dot 1 (U+2801)");
    }

    #[test]
    fn test_pixel_1_0_maps_to_dot_4() {
        // Create 2×4 image with only pixel (1,0) black (top-right of block)
        let mut pixels = vec![false; 8];
        pixels[1] = true; // (1,0) = y*width + x = 0*2 + 1 = 1

        let binary = create_test_image(2, 4, pixels);
        let grid = pixels_to_braille(&binary, 1, 1).unwrap();

        // Pixel (1,0) maps to dot 4 = U+2800 + 0x08 = U+2808
        let ch = grid.get_char(0, 0);
        assert_eq!(ch, '\u{2808}', "Pixel (1,0) should map to dot 4 (U+2808)");
    }

    #[test]
    fn test_checkerboard_pattern() {
        // Create 2×4 checkerboard pattern
        // Row 0: [black, white] = dots 1, _ = 0x01
        // Row 1: [white, black] = dots _, 5 = 0x10
        // Row 2: [black, white] = dots 3, _ = 0x04
        // Row 3: [white, black] = dots _, 8 = 0x80
        // Total: 0x01 + 0x10 + 0x04 + 0x80 = 0x95
        let pixels = vec![
            true, false, // Row 0
            false, true, // Row 1
            true, false, // Row 2
            false, true, // Row 3
        ];
        let binary = create_test_image(2, 4, pixels);

        let grid = pixels_to_braille(&binary, 1, 1).unwrap();

        // Verify checkerboard pattern (U+2800 + 0x95 = U+2895)
        let ch = grid.get_char(0, 0);
        assert_eq!(ch, '\u{2895}', "Checkerboard should produce U+2895");
    }

    #[test]
    fn test_grid_dimensions_160x96_pixels() {
        // Standard terminal: 80×24 cells = 160×96 pixels
        let pixels = vec![false; 160 * 96];
        let binary = create_test_image(160, 96, pixels);

        let grid = pixels_to_braille(&binary, 80, 24).unwrap();

        // Grid should be exactly 80×24 (perfect division)
        assert_eq!(grid.width(), 80);
        assert_eq!(grid.height(), 24);
    }

    #[test]
    fn test_non_divisible_width() {
        // 5×4 image (width not divisible by 2)
        // Should pad to 6×4 → 3×1 grid
        let pixels = vec![false; 20]; // 5×4 = 20 pixels
        let binary = create_test_image(5, 4, pixels);

        let grid = pixels_to_braille(&binary, 3, 1).unwrap();

        // Grid width: (5+1)/2 = 3, height: (4+3)/4 = 1
        assert_eq!(grid.width(), 3);
        assert_eq!(grid.height(), 1);
    }

    #[test]
    fn test_non_divisible_height() {
        // 4×5 image (height not divisible by 4)
        // Should pad to 4×8 → 2×2 grid
        let pixels = vec![false; 20]; // 4×5 = 20 pixels
        let binary = create_test_image(4, 5, pixels);

        let grid = pixels_to_braille(&binary, 2, 2).unwrap();

        // Grid width: (4+1)/2 = 2, height: (5+3)/4 = 2
        assert_eq!(grid.width(), 2);
        assert_eq!(grid.height(), 2);
    }

    #[test]
    fn test_very_small_2x2_image() {
        // 2×2 image should create 1×1 grid
        // Pixels: (0,0)=true, (1,0)=false, (0,1)=false, (1,1)=true
        // Maps to: dot 1 + dot 5 = 0x01 + 0x10 = 0x11 = U+2811
        let pixels = vec![true, false, false, true]; // 2×2 = 4 pixels
        let binary = create_test_image(2, 2, pixels);

        let grid = pixels_to_braille(&binary, 1, 1).unwrap();

        assert_eq!(grid.width(), 1);
        assert_eq!(grid.height(), 1);

        // Verify the 4 pixels map correctly to dots 1 and 5
        let ch = grid.get_char(0, 0);
        assert_eq!(
            ch, '\u{2811}',
            "2×2 pattern should produce U+2811 (dots 1,5)"
        );
    }
}
