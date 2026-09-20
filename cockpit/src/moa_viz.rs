//! Pure, display-only motion for the selected MoA formation card.

use crate::formations::{self, Formation, FormationId};
use crate::lifecycle_viz::MotionMode;
use crate::terminal_art::{self, ColoredBrailleCell, ColoredBrailleImage, DMD_PALETTE};
use ratatui::{
    style::{Color, Style},
    text::{Line, Span, Text},
};

pub(crate) const FPS: f32 = 12.0;
pub(crate) const FRAME_COUNT: usize = 24;
const BLACK: [u8; 3] = DMD_PALETTE[0];
const CYAN: [u8; 3] = DMD_PALETTE[1];
const HUD_BLUE: [u8; 3] = DMD_PALETTE[2];
const STEEL: [u8; 3] = DMD_PALETTE[3];
const PALE: [u8; 3] = DMD_PALETTE[4];
const GOLD: [u8; 3] = DMD_PALETTE[5];
const SUCCESS: [u8; 3] = DMD_PALETTE[7];
const BAYER4: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cell {
    glyph: char,
    fg: [u8; 3],
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            glyph: '\u{2800}',
            fg: BLACK,
        }
    }
}

pub(crate) fn render(
    formation: Formation,
    previous: Option<(FormationId, f32)>,
    elapsed_secs: f32,
    width: u16,
    height: u16,
    motion: MotionMode,
) -> Text<'static> {
    let (width, height) = (width as usize, height as usize);
    if width == 0 || height == 0 {
        return Text::default();
    }
    let frame = frame_at(elapsed_secs);
    let current = render_cells(formation, width, height, frame, motion);
    let cells = if let Some((previous_id, progress)) = previous {
        let prior = render_cells(
            *formations::formation(previous_id),
            width,
            height,
            frame,
            motion,
        );
        dissolve(&prior, &current, width, height, progress)
    } else {
        current
    };
    cells_to_text(&cells, width, height)
}

fn frame_at(elapsed_secs: f32) -> usize {
    let elapsed = if elapsed_secs.is_finite() {
        elapsed_secs.max(0.0)
    } else {
        0.0
    };
    ((elapsed * FPS).floor() as usize) % FRAME_COUNT
}

fn render_cells(
    formation: Formation,
    width: usize,
    height: usize,
    frame: usize,
    motion: MotionMode,
) -> Vec<Cell> {
    let base = load_card(formation, width, height)
        .map(|image| exact_cells(&image, width, height))
        .unwrap_or_else(|| fallback_card(formation.id, width, height));
    if motion != MotionMode::Full {
        return base;
    }
    match formation.id {
        FormationId::SoloStrike => solo_pulse(&base, frame),
        FormationId::Recon => recon_sweep(&base, width, height, frame),
        // Tag Team shares the Duel card and its two-corners-converge motion —
        // the same shape reads correctly for a pair trading blows.
        FormationId::Duel | FormationId::TagTeam => duel_convergence(&base, width, height, frame),
        FormationId::Council | FormationId::MathGod => council_orbit(&base, width, height, frame),
        FormationId::AllIn => layered_convergence(&base, width, height, frame, false),
        FormationId::GpuComp => layered_convergence(&base, width, height, frame, true),
        FormationId::GrokWar => layered_convergence(&base, width, height, frame, true),
        // The standing mixture orbits like Council: a full-panel rotation.
        FormationId::AutoMoa => council_orbit(&base, width, height, frame),
    }
}

fn load_card(
    formation: Formation,
    width: usize,
    height: usize,
) -> Option<std::sync::Arc<ColoredBrailleImage>> {
    terminal_art::colored_image_braille(&formation.asset_path(), width, height)
}

fn exact_cells(image: &ColoredBrailleImage, width: usize, height: usize) -> Vec<Cell> {
    let mut cells = vec![Cell::default(); width * height];
    for y in 0..height {
        let source_y = (y * image.height / height).min(image.height.saturating_sub(1));
        for x in 0..width {
            let source_x = (x * image.width / width).min(image.width.saturating_sub(1));
            if let Some(ColoredBrailleCell { glyph, fg }) = image.cell(source_x, source_y) {
                cells[y * width + x] = Cell { glyph, fg };
            }
        }
    }
    cells
}

fn solo_pulse(base: &[Cell], frame: usize) -> Vec<Cell> {
    let lit = (frame / 6).is_multiple_of(2);
    base.iter()
        .map(|cell| Cell {
            glyph: cell.glyph,
            fg: if terminal_art::braille_bits(cell.glyph) != 0
                && matches!(cell.fg, CYAN | HUD_BLUE | STEEL | PALE)
            {
                if lit { HUD_BLUE } else { STEEL }
            } else {
                cell.fg
            },
        })
        .collect()
}

fn recon_sweep(base: &[Cell], width: usize, height: usize, frame: usize) -> Vec<Cell> {
    let mut out = vec![Cell::default(); base.len()];
    let band = frame * (width + 6) / FRAME_COUNT;
    for y in 0..height {
        for x in 0..width {
            let source = base[y * width + x];
            let distance = x.abs_diff(band.min(width.saturating_sub(1)));
            let shift = if distance <= 1 && y.is_multiple_of(2) {
                1
            } else {
                0
            };
            let dest_x = x.saturating_add(shift).min(width - 1);
            let mut cell = source;
            if distance <= 1 && terminal_art::braille_bits(cell.glyph) != 0 {
                cell.fg = if y.is_multiple_of(3) { GOLD } else { PALE };
            }
            merge_cell(&mut out[y * width + dest_x], cell);
        }
    }
    out
}

fn duel_convergence(base: &[Cell], width: usize, height: usize, frame: usize) -> Vec<Cell> {
    let mut out = vec![Cell::default(); base.len()];
    let triangle = if frame < FRAME_COUNT / 2 {
        frame
    } else {
        FRAME_COUNT - 1 - frame
    };
    let shift = 2usize.saturating_sub(triangle / 4).min(2);
    let middle = width / 2;
    for y in 0..height {
        for x in 0..width {
            let dest_x = if x < middle {
                x.saturating_add(shift).min(width - 1)
            } else {
                x.saturating_sub(shift)
            };
            let mut cell = base[y * width + x];
            if x.abs_diff(middle) <= 1 && terminal_art::braille_bits(cell.glyph) != 0 {
                cell.fg = GOLD;
            }
            merge_cell(&mut out[y * width + dest_x], cell);
        }
    }
    out
}

fn council_orbit(base: &[Cell], width: usize, height: usize, frame: usize) -> Vec<Cell> {
    let mut out = base.to_vec();
    if width < 3 || height < 3 {
        return out;
    }
    const ORBIT: [(i32, i32); 8] = [
        (0, -2),
        (2, -1),
        (3, 0),
        (2, 1),
        (0, 2),
        (-2, 1),
        (-3, 0),
        (-2, -1),
    ];
    let center = (width as i32 / 2, height as i32 / 2);
    for node in 0..3 {
        let (dx, dy) = ORBIT[(frame / 2 + node * 2) % ORBIT.len()];
        let x = (center.0 + dx).clamp(0, width as i32 - 1) as usize;
        let y = (center.1 + dy).clamp(0, height as i32 - 1) as usize;
        out[y * width + x] = Cell {
            glyph: '\u{28ff}',
            fg: [GOLD, CYAN, SUCCESS][node],
        };
    }
    out
}

fn layered_convergence(
    base: &[Cell],
    width: usize,
    height: usize,
    frame: usize,
    gpu: bool,
) -> Vec<Cell> {
    let mut out = vec![Cell::default(); base.len()];
    let cycle = if gpu { 12 } else { FRAME_COUNT };
    let phase = frame % cycle;
    let half = (cycle / 2).max(1);
    let spread = if phase < half {
        2usize.saturating_sub(phase * 3 / half)
    } else {
        ((phase - half) * 3 / half).min(2)
    };
    let offsets: &[(isize, isize, [u8; 3])] = if gpu {
        &[
            (-2, 0, HUD_BLUE),
            (2, 0, GOLD),
            (0, -1, SUCCESS),
            (0, 0, PALE),
        ]
    } else {
        &[(-1, 0, STEEL), (1, 0, CYAN), (0, 0, GOLD)]
    };
    for &(ox, oy, tint) in offsets {
        let ox = ox * spread as isize;
        let oy = oy * spread as isize;
        for y in 0..height {
            for x in 0..width {
                let source = base[y * width + x];
                if terminal_art::braille_bits(source.glyph) == 0 {
                    continue;
                }
                let dx = x as isize + ox;
                let dy = y as isize + oy;
                if dx < 0 || dy < 0 || dx >= width as isize || dy >= height as isize {
                    continue;
                }
                merge_cell(
                    &mut out[dy as usize * width + dx as usize],
                    Cell {
                        glyph: source.glyph,
                        fg: if ox == 0 && oy == 0 { source.fg } else { tint },
                    },
                );
            }
        }
    }
    out
}

fn merge_cell(dest: &mut Cell, source: Cell) {
    let bits = terminal_art::braille_bits(dest.glyph) | terminal_art::braille_bits(source.glyph);
    if bits != 0 {
        dest.glyph = terminal_art::braille_char(bits);
        dest.fg = source.fg;
    }
}

fn dissolve(
    previous: &[Cell],
    current: &[Cell],
    width: usize,
    height: usize,
    progress: f32,
) -> Vec<Cell> {
    let progress = progress.clamp(0.0, 1.0);
    let cutoff = (progress * 16.0).round() as u8;
    let mut out = vec![Cell::default(); width * height];
    for y in 0..height {
        for x in 0..width {
            let old = previous[y * width + x];
            let new = current[y * width + x];
            let old_bits = terminal_art::braille_bits(old.glyph);
            let new_bits = terminal_art::braille_bits(new.glyph);
            let mut bits = 0u8;
            let mut new_votes = 0u8;
            let mut old_votes = 0u8;
            for local_y in 0..4 {
                for local_x in 0..2 {
                    let bit = terminal_art::braille_dot_bit(local_x, local_y);
                    let threshold = BAYER4[(y * 4 + local_y) % 4][(x * 2 + local_x) % 4];
                    if threshold < cutoff {
                        if new_bits & bit != 0 {
                            bits |= bit;
                            new_votes += 1;
                        }
                    } else if old_bits & bit != 0 {
                        bits |= bit;
                        old_votes += 1;
                    }
                }
            }
            out[y * width + x] = Cell {
                glyph: terminal_art::braille_char(bits),
                fg: if new_votes >= old_votes {
                    new.fg
                } else {
                    old.fg
                },
            };
        }
    }
    out
}

fn fallback_card(id: FormationId, width: usize, height: usize) -> Vec<Cell> {
    let mut cells = vec![Cell::default(); width * height];
    if width == 0 || height == 0 {
        return cells;
    }
    let seed = id.cache_key() as usize;
    for y in 0..height {
        for x in 0..width {
            if (x * 3 + y * 5 + seed).is_multiple_of(11) {
                cells[y * width + x] = Cell {
                    glyph: terminal_art::braille_char(0x42 | ((seed as u8) & 0x18)),
                    fg: [CYAN, GOLD, HUD_BLUE, SUCCESS][seed % 4],
                };
            }
        }
    }
    cells
}

fn cells_to_text(cells: &[Cell], width: usize, height: usize) -> Text<'static> {
    let mut lines = Vec::with_capacity(height);
    for y in 0..height {
        let spans = (0..width)
            .map(|x| {
                let cell = cells[y * width + x];
                Span::styled(
                    cell.glyph.to_string(),
                    Style::new()
                        .fg(Color::Rgb(cell.fg[0], cell.fg[1], cell.fg[2]))
                        .bg(Color::Black),
                )
            })
            .collect::<Vec<_>>();
        lines.push(Line::from(spans));
    }
    Text::from(lines)
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/moa_viz__tests.rs"]
mod tests;
