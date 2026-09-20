//! dotmax braille line chart, rendered into a ratatui buffer.
//!
//! Plots a data series as a connected braille line using dotmax's `BrailleGrid`
//! and `draw_line`, then converts the grid to ratatui `Text` — the same
//! buffer-friendly approach as the loading bar (no dotmax stdout renderer). Per
//! the cockpit decision, dotmax is used ONLY for bars / braille charts.

use dotmax::BrailleGrid;
use dotmax::primitives::draw_line;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span, Text},
};

/// Render `series` as a braille line chart sized `width_cells` × `height_cells`,
/// auto-scaled to the series' min/max. Fewer than 2 points renders empty.
pub fn line_chart(series: &[f32], width_cells: u16, height_cells: u16) -> Text<'static> {
    let w = (width_cells as usize).max(1);
    let h = (height_cells as usize).max(1);
    let mut grid = match BrailleGrid::new(w, h) {
        Ok(g) => g,
        Err(_) => return Text::raw(""),
    };
    plot(&mut grid, series, w, h);
    grid_to_text(&grid, w, h)
}

fn plot(grid: &mut BrailleGrid, series: &[f32], w_cells: usize, h_cells: usize) {
    let dw = (w_cells * 2) as i32; // dots wide  (2 per cell)
    let dh = (h_cells * 4) as i32; // dots tall  (4 per cell)
    if series.len() < 2 || dw < 2 || dh < 2 {
        return;
    }
    let lo = series.iter().copied().fold(f32::INFINITY, f32::min);
    let hi = series.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    if !lo.is_finite() || !hi.is_finite() {
        return;
    }
    let span = (hi - lo).max(1e-6);
    let n = series.len();
    let point = |i: usize| -> (i32, i32) {
        let x = ((i as f32) / ((n - 1) as f32) * (dw - 1) as f32).round() as i32;
        let frac = (series[i] - lo) / span; // 0..=1
        let y = ((1.0 - frac) * (dh - 1) as f32).round() as i32; // invert: high value = top
        (x, y)
    };
    for i in 0..n - 1 {
        let (x0, y0) = point(i);
        let (x1, y1) = point(i + 1);
        let _ = draw_line(grid, x0, y0, x1, y1);
    }
}

fn grid_to_text(grid: &BrailleGrid, w_cells: usize, h_cells: usize) -> Text<'static> {
    let accent = Style::new().fg(Color::Green);
    let mut lines = Vec::with_capacity(h_cells);
    for y in 0..h_cells {
        let mut row = String::with_capacity(w_cells);
        for x in 0..w_cells {
            row.push(grid.get_char(x, y));
        }
        lines.push(Line::from(Span::styled(row, accent)));
    }
    Text::from(lines)
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/chart__tests.rs"]
mod tests;
