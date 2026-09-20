//! Static frame-pack export for web and documentation previews.
//!
//! This module converts [`BrailleGrid`] snapshots into a compact JSON-friendly
//! format. The generated files are intended for static websites that need to
//! replay terminal output without launching a native terminal process.

use crate::{BrailleGrid, DotmaxError};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::Path;

/// RGB color encoded as `[red, green, blue]`.
pub type DotmaxFrameColor = [u8; 3];

/// One captured terminal frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DotmaxFrame {
    /// Rows of braille or density characters, one string per terminal row.
    pub rows: Vec<String>,
    /// Optional per-cell RGB colors matching `rows[y][x]`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colors: Option<Vec<Vec<Option<DotmaxFrameColor>>>>,
}

/// A replayable set of terminal frames.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DotmaxFramePack {
    /// Human-readable example name.
    pub name: String,
    /// Command that produced or corresponds to this frame stream.
    pub command: String,
    /// Width in terminal cells.
    pub width: usize,
    /// Height in terminal cells.
    pub height: usize,
    /// Intended playback rate.
    pub fps: u32,
    /// Captured frames.
    pub frames: Vec<DotmaxFrame>,
}

impl DotmaxFramePack {
    /// Build a frame pack from metadata and captured frames.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        command: impl Into<String>,
        width: usize,
        height: usize,
        fps: u32,
        frames: Vec<DotmaxFrame>,
    ) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            width,
            height,
            fps,
            frames,
        }
    }
}

/// Capture the visible characters and optional colors from a [`BrailleGrid`].
#[must_use]
pub fn capture_frame(grid: &BrailleGrid) -> DotmaxFrame {
    let (width, height) = grid.dimensions();
    let mut rows = Vec::with_capacity(height);
    let mut colors = Vec::with_capacity(height);
    let mut has_colors = false;

    for y in 0..height {
        let mut row = String::with_capacity(width);
        let mut color_row = Vec::with_capacity(width);

        for x in 0..width {
            row.push(grid.get_char(x, y));
            let color = grid.get_color(x, y).map(|color| {
                has_colors = true;
                [color.r, color.g, color.b]
            });
            color_row.push(color);
        }

        rows.push(row);
        colors.push(color_row);
    }

    DotmaxFrame {
        rows,
        colors: has_colors.then_some(colors),
    }
}

/// Write a frame pack as pretty JSON.
///
/// # Errors
/// Returns a [`DotmaxError`] if the file cannot be created or the pack cannot
/// be serialized.
pub fn write_frame_pack(path: impl AsRef<Path>, pack: &DotmaxFramePack) -> Result<(), DotmaxError> {
    let file = File::create(path)?;
    serde_json::to_writer_pretty(file, pack)
        .map_err(|error| DotmaxError::FrameExport(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BrailleGrid, Color};

    #[test]
    fn capture_frame_preserves_dimensions_rows_characters_and_colors() {
        let mut grid = BrailleGrid::new(3, 2).unwrap();
        grid.set_dot(0, 0).unwrap();
        grid.set_char(1, 0, '@').unwrap();
        grid.set_cell_color(1, 0, Color::rgb(12, 34, 56)).unwrap();

        let frame = capture_frame(&grid);

        assert_eq!(frame.rows.len(), 2);
        assert!(frame.rows.iter().all(|row| row.chars().count() == 3));
        assert_eq!(
            frame.rows[0].chars().collect::<Vec<_>>(),
            vec!['⠁', '@', '⠀']
        );
        assert_eq!(
            frame.colors,
            Some(vec![
                vec![None, Some([12, 34, 56]), None],
                vec![None, None, None],
            ])
        );
    }

    #[test]
    fn frame_pack_json_snapshot_stays_stable() {
        let mut grid = BrailleGrid::new(2, 1).unwrap();
        grid.set_dot(0, 0).unwrap();

        let pack = DotmaxFramePack::new(
            "tiny",
            "cargo run --example tiny",
            2,
            1,
            12,
            vec![capture_frame(&grid)],
        );

        let json = serde_json::to_string_pretty(&pack).unwrap();

        assert_eq!(
            json,
            r#"{
  "name": "tiny",
  "command": "cargo run --example tiny",
  "width": 2,
  "height": 1,
  "fps": 12,
  "frames": [
    {
      "rows": [
        "⠁⠀"
      ]
    }
  ]
}"#
        );
    }
}
