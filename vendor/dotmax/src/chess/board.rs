//! Chess board rendering logic.

use crate::chess::sprites::get_sprite;
use crate::error::DotmaxError;
use crate::grid::{BrailleGrid, Color};
use shakmaty::{Chess, File, Position, Rank, Square};

/// Options for rendering the chess board.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Target width in terminal cells. If None, uses a default size.
    pub target_width: Option<usize>,
    /// Target height in terminal cells. If None, uses a default size.
    pub target_height: Option<usize>,
    /// Color scheme for the board.
    pub color_scheme: BoardColorScheme,
    /// Whether to show coordinates (a-h, 1-8).
    pub show_coordinates: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            target_width: Some(32),
            target_height: Some(16),
            color_scheme: BoardColorScheme::default(),
            show_coordinates: true,
        }
    }
}

/// Color scheme for the chess board.
#[derive(Debug, Clone)]
pub struct BoardColorScheme {
    /// Color of light squares.
    pub light_square: Color,
    /// Color of dark squares.
    pub dark_square: Color,
    /// Color of white pieces.
    pub white_piece: Color,
    /// Color of black pieces.
    pub black_piece: Color,
    /// Color of coordinates and borders.
    pub border: Color,
}

impl Default for BoardColorScheme {
    fn default() -> Self {
        Self {
            light_square: Color::rgb(240, 217, 181),
            dark_square: Color::rgb(181, 136, 99),
            white_piece: Color::rgb(255, 255, 255),
            black_piece: Color::rgb(0, 0, 0),
            border: Color::rgb(200, 200, 200),
        }
    }
}

/// Render a chess position to a BrailleGrid with options.
pub fn render_position_with_options(
    pos: &Chess,
    options: &RenderOptions,
) -> Result<BrailleGrid, DotmaxError> {
    // 1. Determine grid dimensions.
    // Standard terminal cells are ~2:1 aspect ratio (taller than wide).
    // Braille cells are 2x4 dots.
    // To get a square board, we want the pixel aspect ratio to be balanced.
    // Let's assume 1 cell width = 2 cell height in visual space.
    // If we want a square board of N units:
    // width_cells = N
    // height_cells = N / 2 (approx)

    let width_cells = options.target_width.unwrap_or(32);
    let height_cells = options.target_height.unwrap_or(16);

    let mut grid = BrailleGrid::new(width_cells, height_cells)?;
    let board = pos.board();

    // 8x8 squares. Each square size in dots:
    let square_width_dots = (width_cells * 2) / 8;
    let square_height_dots = (height_cells * 4) / 8;

    for rank in Rank::ALL.iter().rev() {
        for file in File::ALL.iter() {
            let square = Square::from_coords(*file, *rank);
            let is_light = (u32::from(*file) + u32::from(*rank)) % 2 != 0;

            let x_dot_start = u32::from(*file) * square_width_dots as u32;
            let y_dot_start = (7 - u32::from(*rank)) * square_height_dots as u32;

            // Draw square background (dithering or solid dots)
            let square_color = if is_light {
                options.color_scheme.light_square
            } else {
                options.color_scheme.dark_square
            };

            // Set background color for the cells
            for dy in 0..square_height_dots {
                for dx in 0..square_width_dots {
                    let dot_x = (x_dot_start + dx as u32) as usize;
                    let dot_y = (y_dot_start + dy as u32) as usize;
                    if dot_x < grid.dot_width() && dot_y < grid.dot_height() {
                        grid.set_cell_color(dot_x / 2, dot_y / 4, square_color)?;
                    }
                }
            }

            if let Some(piece) = board.piece_at(square) {
                let sprite = get_sprite(piece.role, piece.color);
                let piece_color = if piece.color == shakmaty::Color::White {
                    options.color_scheme.white_piece
                } else {
                    options.color_scheme.black_piece
                };

                // Scale sprite to square size
                // Piece should be centered and slightly smaller than square
                let padding_x = square_width_dots / 8;
                let padding_y = square_height_dots / 8;
                let inner_w = square_width_dots.saturating_sub(padding_x * 2);
                let inner_h = square_height_dots.saturating_sub(padding_y * 2);

                for dy in 0..inner_h {
                    for dx in 0..inner_w {
                        // Map (dx, dy) to sprite coordinates (0..8)
                        let sx = (dx * 8) / inner_w;
                        let sy = (dy * 8) / inner_h;

                        if (sprite.mask[sy as usize] & (1 << (7 - sx))) != 0 {
                            let dot_x = (x_dot_start + (dx + padding_x) as u32) as usize;
                            let dot_y = (y_dot_start + (dy + padding_y) as u32) as usize;
                            if dot_x < grid.dot_width() && dot_y < grid.dot_height() {
                                grid.set_dot(dot_x, dot_y)?;
                                grid.set_cell_color(dot_x / 2, dot_y / 4, piece_color)?;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(grid)
}

/// Render a chess position to a BrailleGrid (legacy simple API).
pub fn render_position(pos: &Chess) -> Result<BrailleGrid, DotmaxError> {
    render_position_with_options(pos, &RenderOptions::default())
}
