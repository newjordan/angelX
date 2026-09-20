//! Chess visualization module for dotmax.
//!
//! Provides tools to render chess boards from PGN or FEN using braille.

pub mod board;
pub mod sprites;

use crate::error::DotmaxError;
pub use board::{BoardColorScheme, RenderOptions};
use pgn_reader::{BufferedReader, SanPlus, Visitor};
use shakmaty::{Chess, Position};

/// Render a PGN string to a braille-encoded string.
///
/// This is an agnostic output suitable for terminals or web frontends.
pub fn render_pgn(pgn: &str, move_index: Option<usize>) -> Result<String, DotmaxError> {
    render_pgn_with_options(pgn, move_index, &RenderOptions::default())
}

/// Render a PGN string to a braille-encoded string with options.
pub fn render_pgn_with_options(
    pgn: &str,
    move_index: Option<usize>,
    options: &RenderOptions,
) -> Result<String, DotmaxError> {
    let mut visitor = ChessVisitor::new(move_index);
    let mut reader = BufferedReader::new_cursor(pgn.as_bytes());

    if let Ok(Some(pos)) = reader.read_game(&mut visitor) {
        let grid = board::render_position_with_options(&pos, options)?;
        let mut out = String::new();
        for row in grid.to_unicode_grid() {
            for ch in row {
                out.push(ch);
            }
            out.push('\n');
        }
        Ok(out)
    } else {
        Err(DotmaxError::TerminalBackend(
            "No game found or PGN error".to_string(),
        ))
    }
}

struct ChessVisitor {
    pos: Chess,
    target_move: Option<usize>,
    current_move: usize,
    finished: bool,
}

impl ChessVisitor {
    fn new(target_move: Option<usize>) -> Self {
        Self {
            pos: Chess::default(),
            target_move,
            current_move: 0,
            finished: false,
        }
    }
}

impl Visitor for ChessVisitor {
    type Result = Chess;

    fn san(&mut self, san: SanPlus) {
        if self.finished {
            return;
        }

        if let Some(target) = self.target_move {
            if self.current_move >= target {
                self.finished = true;
                return;
            }
        }

        if let Ok(m) = san.san.to_move(&self.pos) {
            self.pos.play_unchecked(m);
            self.current_move += 1;
        } else {
            self.finished = true;
        }
    }

    fn end_game(&mut self) -> Self::Result {
        self.pos.clone()
    }
}
