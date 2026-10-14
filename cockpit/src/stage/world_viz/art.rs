//! The second art kit's terminal painters: half blocks and braille into
//! ratatui cells, and the display-size panels. The kit itself (materials,
//! light, the native picture) is the Delve's (`angel_delve::render::art`).

pub(crate) use angel_delve::render::art::*;

use super::overworld::ink::{Img, Rgb};
use crate::ui::term::art::braille_bits;

/// One actual half-block plus braille painter for local, joined and mini views.
/// A terminal cell has one glyph: figures/cores keep half blocks; atmosphere
/// uses a braille glyph on the dark material's average background.
pub(crate) fn paint_cells(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    base: &Img,
    fx: &Img,
) {
    use ratatui::style::Color;
    if area.width == 0 || area.height == 0 {
        return;
    }
    let (w, h) = (area.width as usize, area.height as usize);
    let dots = braille(fx, w, h);
    let color = |c: Rgb| Color::Rgb(c[0], c[1], c[2]);
    for cy in 0..h {
        for cx in 0..w {
            let mut halves = [[0u8; 3]; 2];
            for half in 0..2 {
                let x0 = cx * base.w as usize / w;
                let x1 = ((cx + 1) * base.w as usize / w).max(x0 + 1);
                let y0 = (cy * 2 + half) * base.h as usize / (h * 2);
                let y1 = ((cy * 2 + half + 1) * base.h as usize / (h * 2)).max(y0 + 1);
                let mut sum = [0u32; 3];
                let mut n = 0;
                let mut core = BLACK;
                let mut rank = 0;
                for y in y0..y1 {
                    for x in x0..x1 {
                        let c = base.get(x as i32, y as i32).unwrap_or(BLACK);
                        for k in 0..3 {
                            sum[k] += c[k] as u32;
                        }
                        n += 1;
                        let r = 299 * c[0] as u32 + 587 * c[1] as u32 + 114 * c[2] as u32;
                        if r > rank {
                            rank = r;
                            core = c;
                        }
                    }
                }
                // Protect a bright body/core even when its coverage is subcell.
                halves[half] = if rank > 105_000 {
                    mix(std::array::from_fn(|k| (sum[k] / n) as u8), core, 0.48)
                } else {
                    std::array::from_fn(|k| (sum[k] / n) as u8)
                };
            }
            let cell = &dots.cells[cy * w + cx];
            let crisp = halves
                .iter()
                .any(|c| c.iter().copied().max().unwrap_or(0) > 100);
            if let Some(dest) = frame
                .buffer_mut()
                .cell_mut((area.x + cx as u16, area.y + cy as u16))
            {
                if !crisp && braille_bits(cell.glyph) != 0 {
                    dest.set_char(cell.glyph)
                        .set_fg(color(mix(cell.fg, BLACK, 0.18)))
                        .set_bg(color(mix(halves[0], halves[1], 0.5)));
                } else {
                    dest.set_char('▀')
                        .set_fg(color(halves[0]))
                        .set_bg(color(halves[1]));
                }
            }
        }
    }
}

/// Display-size panels retain the pixel kit's navy/brass hierarchy and live data.
pub(crate) fn paint_panels(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    run: &crate::drive::together_shooter::Run,
    id: u32,
) {
    use crate::drive::together_shooter::votes::{ITEMS, Rating};
    use ratatui::{
        layout::Rect,
        style::{Color, Style},
        widgets::{Block, Borders, Clear, Paragraph},
    };
    let rgb = |c: Rgb| Color::Rgb(c[0], c[1], c[2]);
    if let Some(seat) = run.playtest.seats.get(&id).filter(|s| s.open) {
        let panel = Rect::new(
            area.x + 2,
            area.y + 1,
            area.width.saturating_sub(4),
            area.height.saturating_sub(2),
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" WHAT'S NEW ")
            .style(Style::new().bg(rgb(WELL)).fg(rgb(TEXT)))
            .border_style(Style::new().fg(rgb(TRIM)));
        let inner = block.inner(panel);
        frame.render_widget(Clear, panel);
        frame.render_widget(block, panel);
        let start = seat.selected.saturating_sub(3);
        let mut rows = vec![
            "FEATURE                       YOU       PARTY".to_string(),
            String::new(),
        ];
        for (i, item) in ITEMS
            .iter()
            .enumerate()
            .skip(start)
            .take(inner.height.saturating_sub(6) as usize)
        {
            let rating = run
                .players
                .get(&id)
                .and_then(|h| run.playtest.records.get(&h.name))
                .and_then(|r| r.get(item.id));
            let stars = match rating {
                Some(Rating::Stars(n)) => {
                    format!("{}{}", "★".repeat(*n as usize), "☆".repeat(5 - *n as usize))
                }
                Some(Rating::Untried(_)) => "UNTRIED".into(),
                _ => "☆☆☆☆☆".into(),
            };
            let (avg, count) = run.playtest.average(item.id);
            rows.push(format!(
                "{} {:28}  {:7}   {}",
                if i == seat.selected { ">" } else { " " },
                item.label,
                stars,
                if count >= 2 {
                    format!("{avg:.1} ({count})")
                } else {
                    String::new()
                }
            ));
        }
        rows.extend([
            String::new(),
            "UP/DOWN choose  LEFT/RIGHT rate".into(),
            "1–5 stars   0 clear   N untried   V/ESC close".into(),
        ]);
        frame.render_widget(
            Paragraph::new(rows.join("\n")).style(Style::new().fg(rgb(TEXT)).bg(rgb(WELL))),
            inner,
        );
    }
    if let Some(race) = &run.chariot {
        let p = race.player();
        let order = race.order();
        let place = 1 + order.iter().position(|&i| i == 0).unwrap_or(0);
        let panel = Rect::new(
            area.x + 1,
            area.y,
            area.width.saturating_sub(2).min(65),
            area.height.min(5),
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .style(Style::new().fg(rgb(TEXT)).bg(rgb(WELL)))
            .border_style(Style::new().fg(rgb(TRIM)));
        let inner = block.inner(panel);
        frame.render_widget(Clear, panel);
        frame.render_widget(block, panel);
        frame.render_widget(
            Paragraph::new(format!(
                "{}\nLAP {}/{}  PLACE {place}/4  CALTROPS {}\nA D steer   SPACE whip   X caltrop",
                race.call(),
                p.lap(race.course()) + 1,
                crate::drive::together_shooter::chariot::LAPS,
                p.caltrops
            ))
            .style(Style::new().fg(rgb(TEXT)).bg(rgb(WELL))),
            inner,
        );
    }

    if let Some((h, r)) = run
        .players
        .get(&id)
        .and_then(|h| h.pit.result.as_ref().map(|r| (h, r)))
    {
        if r.left >= 10 && area.height > 8 {
            let time = format!(
                "{:.2}S",
                (r.total * 100 / crate::drive::together_shooter::HZ) as f32 / 100.0
            );
            let top = Rect::new(area.x + 5, area.y + 1, area.width.saturating_sub(10), 3);
            frame.render_widget(Clear, top);
            frame.render_widget(
                Paragraph::new(format!("\nPRIVY DASH   {}   {time}", h.name))
                    .style(Style::new().fg(rgb(TEXT)).bg(rgb(WELL))),
                top,
            );
            let bottom = Rect::new(
                area.x + 5,
                area.bottom() - 8,
                area.width.saturating_sub(10),
                6,
            );
            frame.render_widget(Clear, bottom);
            frame.render_widget(
                Paragraph::new(format!(
                    "\n{}\n{}  {time}",
                    r.quality.word(),
                    if r.party_record {
                        "PARTY RECORD!"
                    } else {
                        "FINISHED"
                    }
                ))
                .style(Style::new().fg(rgb(TRIM)).bg(rgb(WELL))),
                bottom,
            );
            let board = Rect::new(
                area.right() - area.width.min(24) - 4,
                area.y + area.height * 82 / 224,
                area.width.min(24),
                6,
            );
            let block = Block::default()
                .borders(Borders::ALL)
                .title(" PARTY BEST ")
                .style(Style::new().fg(rgb(TEXT)).bg(rgb(WELL)))
                .border_style(Style::new().fg(rgb(TRIM)));
            let inner = block.inner(board);
            frame.render_widget(Clear, board);
            frame.render_widget(block, board);
            let ranks = run
                .pits
                .ranked()
                .iter()
                .take(3)
                .map(|b| {
                    format!(
                        "{:<10} {:>6.2}",
                        b.name.chars().take(10).collect::<String>(),
                        (b.best * 100 / crate::drive::together_shooter::HZ) as f32 / 100.0
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            frame.render_widget(
                Paragraph::new(ranks).style(Style::new().fg(rgb(TEXT)).bg(rgb(WELL))),
                inner,
            );
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/overworld__art_v2_tests.rs"]
mod tests;
