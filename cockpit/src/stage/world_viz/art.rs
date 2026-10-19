//! The second art kit's terminal painters: half blocks and braille into
//! ratatui cells, and the display-size panels. The kit itself (materials,
//! light, the native picture) is the Delve's (`angel_delve::render::art`).

pub(crate) use angel_delve::render::art::*;

#[cfg(test)]
use super::overworld::ink::Img;

mod cells;
pub(crate) use cells::{paint_cells, paint_cells_with_contour, paint_rafterwake_cells, paint_projectile_cells};

#[cfg(test)]
mod signature_tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};

    #[test]
    fn sparse_lash_retains_native_ink_in_the_actual_cell() {
        let mut base = Img::black(8, 16);
        let fx = Img::black(8, 16);
        // A pale-blue pigment of the enlarged effect frames, and the native
        // kit's ink (four fifths of its value) that it stands for.
        let lash = [178, 203, 210];
        let native = angel_delve::render::overworld::arena::cell_effect_ink(lash).expect("a pit boss effect pigment");
        assert_eq!(native, [143, 163, 168]);
        base.set(3, 3, lash);
        let mut terminal = Terminal::new(TestBackend::new(1, 1)).unwrap();
        terminal.draw(|f| paint_cells(f, Rect::new(0, 0, 1, 1), &base, &fx)).unwrap();
        assert_eq!(terminal.backend().buffer()[(0, 0)].fg, Color::Rgb(native[0], native[1], native[2]));
        assert_eq!(terminal.backend().buffer()[(0, 0)].bg, Color::Rgb(0, 0, 0));
    }

    #[test]
    fn furnace_core_survives_sparse_half_cell_coverage() {
        let mut base = Img::black(8, 16);
        let fx = Img::black(8, 16);
        let core = [241, 189, 90];
        base.set(3, 3, core);
        let mut terminal = Terminal::new(TestBackend::new(1, 1)).unwrap();
        terminal.draw(|f| paint_cells(f, Rect::new(0, 0, 1, 1), &base, &fx)).unwrap();
        assert_eq!(terminal.backend().buffer()[(0, 0)].fg, Color::Rgb(core[0], core[1], core[2]));
        let plate = [180, 170, 185];
        base.set(3, 3, plate);
        terminal.draw(|f| paint_cells(f, Rect::new(0, 0, 1, 1), &base, &fx)).unwrap();
        assert_ne!(terminal.backend().buffer()[(0, 0)].fg, Color::Rgb(plate[0], plate[1], plate[2]));
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
    let rgb = |c: [u8; 3]| Color::Rgb(c[0], c[1], c[2]);
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

/// Cell-sized words over the kiln's half-block figures and braille effects.
pub(crate) fn paint_cinder_panels(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    run: &crate::drive::together_shooter::Run,
    id: u32,
) {
    use ratatui::{
        layout::Rect,
        style::{Color, Style},
        widgets::{Clear, Paragraph},
    };
    let Some(s) = &run.cinder else {
        return;
    };
    let id = if s.boards.contains_key(&id) {
        id
    } else {
        *s.boards.keys().next().unwrap()
    };
    let text = |frame: &mut ratatui::Frame, x: u16, y: u16, width: u16, value: String| {
        let rect = Rect::new(area.x + x, area.y + y, width, 1).intersection(area);
        frame.render_widget(Clear, rect);
        frame.render_widget(
            Paragraph::new(value).style(Style::new().fg(Color::Rgb(176, 178, 168)).bg(
                if s.left == 0 && y > 3 {
                    Color::Rgb(25, 41, 52)
                } else {
                    Color::Black
                },
            )),
            rect,
        );
    };
    // Match the room's native 384x224 layout at any terminal size.
    let pos = |x: u16, y: u16| (x * area.width / 384, y * area.height / 224);
    let label = |frame: &mut ratatui::Frame, x, y, width: u16, value: String| {
        let Some((x, y, centered)) = angel_delve::render::overworld::arena::slot_art::cell_layout("cinder").place(i32::from(x), i32::from(y), s.left == 0, angel_delve::rules::cinder::LENGTH - s.left) else { return; };
        let (mut x, y) = pos(x as u16, y as u16);
        if centered { x = x.saturating_sub(value.chars().count() as u16 / 2); }
        text(frame, x, y, (width * area.width / 384).max(1), value);
    };
    if s.left == 0 {
        label(
            frame,
            56,
            35,
            272,
            if s.won {
                "THE KILN IS CRACKED"
            } else {
                "THE KILN HOLDS"
            }
            .into(),
        );
        for (n, &who) in s.order.iter().enumerate() {
            let y = 66 + n as u16 * 30;
            let name = run
                .players
                .get(&who)
                .map_or("Knight", |h| h.name.as_str())
                .chars()
                .take(16)
                .collect::<String>();
            let b = &s.boards[&who];
            label(frame, 56, y, 16, s.place(who).to_string());
            label(frame, 104, y, 108, name);
            label(frame, 217, y, 28, b.cracks.to_string());
            label(
                frame,
                253,
                y,
                76,
                b.finish_tick().map_or_else(
                    || "-".into(),
                    |t| format!("{}.{:02}s", t / 30, t % 30 * 100 / 30),
                ),
            );
        }
        if s.won {
            let b = &s.boards[&s.order[0]];
            let tied = s
                .boards
                .values()
                .filter(|v| (v.cracks, v.crack_tick) == (b.cracks, b.crack_tick))
                .count() as u32;
            let total = angel_delve::rules::cinder::GOLD_PER_SEAT * s.boards.len() as u32;
            let mut payout = format!(
                "{}G{}",
                total / tied,
                if tied > 1 { " EACH" } else { "" }
            );
            if total % tied > 0 {
                payout.push_str(&format!(", SEAT {} +{}G", s.order[0], total % tied));
            }
            label(frame, 56, 184, 272, payout);
        }
        label(frame, 56, 200, 272, "E AGAIN".into());
    } else {
        let b = &s.boards[&id];
        label(frame, 270, 5, 112, "CINDER STACK".into());
        label(frame, 256, 25, 128, "MATCH THREE POTS".into());
        label(
            frame,
            8,
            8,
            76,
            run.players
                .get(&id)
                .map_or("Knight", |h| h.name.as_str())
                .to_owned(),
        );
        label(frame, 16, 48, 64, "NEXT".into());
        label(frame, 290, 43, 68, "CROCK".into());
        label(
            frame,
            8,
            99,
            76,
            format!("BEST {}", run.home.countryside.games.cinder.best),
        );
        label(
            frame,
            24,
            87,
            56,
            format!("{}s", s.left.div_ceil(crate::drive::together_shooter::HZ)),
        );
        label(frame, 268, 197, 52, "F TURN".into());
        label(frame, 325, 197, 52, "E DROP".into());
        label(
            frame,
            262,
            177,
            120,
            format!("CRACKS {}/{}", b.cracks, angel_delve::rules::cinder::CRACKS),
        );
        if b.chain > 1 && b.flash > 0 {
            let row = b
                .pop
                .iter()
                .rposition(|r| r.iter().any(|&p| p))
                .unwrap_or(angel_delve::rules::cinder::ROWS - 1);
            let value = format!("CHAIN x{}", b.chain);
            let (x, y) = pos(96, 9 + row as u16 * 20 + 6);
            let width = 144 * area.width / 384;
            text(
                frame,
                x + (width.saturating_sub(value.len() as u16)) / 2,
                y,
                value.len() as u16,
                value,
            );
        }
        let mut rival = 0;
        for (n, (&who, b)) in s.boards.iter().enumerate() {
            if who != id {
                label(frame, 10, 128 + rival * 32, 16, who.to_string());
                rival += 1;
            }
            text(
                frame,
                n as u16 * area.width / 4,
                area.height.saturating_sub(1),
                area.width / 4,
                format!("{who}: {}", b.cracks),
            );
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/overworld__art_v2_tests.rs"]
mod tests;

/// Native terminal words stay crisp over Latch Duel's solid half-block figures.
pub(crate) fn paint_latch_panels(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    run: &crate::drive::together_shooter::Run,
    id: u32,
) {
    use angel_delve::rules::latch::{Beat, GOLD_PER_SEAT};
    use ratatui::{
        layout::Rect,
        style::{Color, Style},
    };
    let Some(s) = &run.latch else {
        return;
    };
    let id = if s.duels.contains_key(&id) {
        id
    } else {
        *s.duels.keys().next().unwrap()
    };
    let label = |frame: &mut ratatui::Frame, x: u16, y: u16, w: u16, value: String| {
        let Some((x, y, centered)) = angel_delve::render::overworld::arena::slot_art::cell_layout("latch").place(i32::from(x), i32::from(y), s.left == 0, angel_delve::rules::latch::LENGTH - s.left) else { return; };
        let x = (x.max(0) as u32 * u32::from(area.width) / 384) as u16;
        let x = if centered { x.saturating_sub(value.chars().count() as u16 / 2) } else { x };
        let rect = Rect::new(
            area.x + x,
            area.y + (y.max(0) as u32 * u32::from(area.height) / 224) as u16,
            (w * area.width / 384).max(1),
            1,
        )
        .intersection(area);
        // Preserve the sampled panel background under each glyph. Clearing a
        // Paragraph rectangle made every field a separate black slab.
        for (dx, ch) in value.chars().take(rect.width as usize).enumerate() {
            if let Some(cell) = frame.buffer_mut().cell_mut((rect.x + dx as u16, rect.y)) {
                cell.set_char(ch).set_style(Style::new().fg(Color::Rgb(176, 178, 168)));
            }
        }
    };
    if s.left == 0 {
        label(
            frame,
            104,
            29,
            220,
            if s.won {
                "CLASP YIELDS"
            } else {
                "THE BELL CLOSES"
            }
            .into(),
        );
        label(frame, 148, 43, 64, "HITS".into());
        label(frame, 212, 43, 56, "MISS".into());
        label(frame, 272, 43, 64, "TIMING".into());
        for (n, &who) in s.order.iter().enumerate() {
            let y = 56 + n as u16 * 24;
            let d = &s.duels[&who];
            label(frame, 32, y, 16, s.place(who).to_string());
            label(
                frame,
                48,
                y,
                92,
                format!("S{who} {}", run.players
                    .get(&who)
                    .map_or("Knight", |h| h.name.as_str())
                    .chars()
                    .take(11)
                    .collect::<String>()),
            );
            label(frame, 148, y, 48, format!("{}/3", d.counters));
            label(frame, 212, y, 40, d.misses.to_string());
            if d.counters > 0 {
                label(frame, 272, y, 60, angel_delve::rules::latch::timing(d.error));
            }
        }
        let first = s.order[0];
        let tied = s
            .duels
            .keys()
            .filter(|&&who| s.score(who) == s.score(first))
            .count() as u32;
        let pot = GOLD_PER_SEAT * s.duels.len() as u32;
        let payout = if tied == 1 {
            format!("SEAT {first} WINS {pot}G + CROWN")
        } else if pot % tied == 0 {
            format!("{}G EACH + SHARED CROWN", pot / tied)
        } else {
            format!("{}G EACH, SEAT {first} +1G", pot / tied)
        };
        label(frame, 40, 174, 308, payout);
        label(
            frame,
            40,
            188,
            308,
            "FEWER MISSES, THEN CLOSER TIMING".into(),
        );
        label(frame, 152, 202, 100, "E AGAIN".into());
        return;
    }
    let d = &s.duels[&id];
    label(frame, 152, 7, 100, "LATCH DUEL".into());
    label(frame, 128, 21, 144, "READ THEN COUNTER".into());
    label(frame, 12, 7, 56, format!("{}s", s.left.div_ceil(30)));
    label(
        frame,
        304,
        7,
        80,
        format!("{}G POT", GOLD_PER_SEAT * s.duels.len() as u32),
    );
    label(
        frame,
        116,
        46,
        80,
        run.players
            .get(&id)
            .map_or("Knight", |h| h.name.as_str())
            .chars()
            .take(14)
            .collect(),
    );
    label(frame, 247, 46, 64, "CLASP".into());
    if d.beat == Beat::Tell {
        label(
            frame,
            180,
            76,
            60,
            if d.height < 0 { "HIGH" } else { "LOW" }.into(),
        );
        if d.ticks <= run.tune.latch_window {
            label(frame, 164, 88, 80, "COUNTER".into());
        }
    }
    label(
        frame,
        72,
        179,
        80,
        if d.guard < 0 {
            "GUARD HIGH"
        } else {
            "GUARD LOW"
        }
        .into(),
    );
    if d.beat == Beat::Clean {
        label(
            frame,
            160,
            64,
            84,
            if d.chain == 1 {
                "PERFECT".into()
            } else {
                format!("CHAIN x{}", d.chain)
            },
        );
    }
    if d.beat == Beat::Miss {
        label(frame, 168, 179, 72, "MISSED".into());
    }
    for (n, (&who, d)) in s.duels.iter().enumerate() {
        label(frame, 12 + n as u16 * 92, 192, 16, who.to_string());
        let x = 12 + n as u16 * 92;
        let colour = run.players.get(&who).map(|h| h.colours())
            .filter(|c| !c.is_empty())
            .unwrap_or(match who {
                2 => &[('7', '1')], 3 => &[('7', 'A')], 4 => &[('7', '4')], _ => &[],
            }).iter().find(|&&(from, _)| from == '7').map_or('7', |&(_, to)| to);
        let [r,g,b] = angel_delve::render::overworld::ink::ink(colour).unwrap();
        let sx = area.x + x * area.width / 384;
        let sy = area.y + 218 * area.height / 224;
        if let Some(cell) = frame.buffer_mut().cell_mut((sx, sy)) {
            cell.set_symbol("■").set_fg(Color::Rgb(r,g,b));
        }
        label(frame, x + 12, 218, 64, format!("{} / 3", d.counters));
    }
    label(frame, 92, 202, 220, "UP DOWN GUARD  F COUNTER".into());
}

pub(crate) fn paint_mossbell_panels(frame:&mut ratatui::Frame,area:ratatui::layout::Rect,run:&angel_delve::rules::Run,focus:Option<u32>) {
    let podium = run.mossbell.as_ref().is_some_and(|s| s.left == 0);
    for (x,y,text) in angel_delve::render::overworld::arena::mossbell_labels(run,focus) {
        let elapsed=run.mossbell.as_ref().map_or(0,|s| angel_delve::rules::mossbell::LENGTH-s.left);
        let Some((x,y,centered))=angel_delve::render::overworld::arena::slot_art::cell_layout("mossbell").place(x,y,podium,elapsed) else {continue;};
        // Terminal glyphs have a different pitch from the native bitmap font.
        // Anchor the score header and values by their common right edge.
        let score = podium && (49..238).contains(&x) && (y == 47 || (66..=138).contains(&y));
        let x=area.x+if score {
            ((238*u32::from(area.width)/384) as u16).saturating_sub(text.chars().count() as u16)
        } else { (x.max(0) as u32*u32::from(area.width)/384) as u16 };
        let x=if centered {x.saturating_sub(text.chars().count() as u16/2)} else {x};
        let y=area.y+(y.max(0) as u32*u32::from(area.height)/224) as u16;
        for (dx,ch) in text.chars().enumerate(){if x+dx as u16>=area.right(){break;}if let Some(cell)=frame.buffer_mut().cell_mut((x+dx as u16,y)){cell.set_char(ch).set_fg(ratatui::style::Color::Rgb(176,178,168));}}
    }
}

pub(crate) fn paint_brasswake_panels(frame:&mut ratatui::Frame,area:ratatui::layout::Rect,run:&angel_delve::rules::Run,focus:Option<u32>) {
    let podium=run.brasswake.as_ref().is_some_and(|s|s.left==0);
    for (x,y,text) in angel_delve::render::overworld::arena::brasswake_labels(run,focus) {
        let elapsed=run.brasswake.as_ref().map_or(0,|s| angel_delve::rules::brasswake::LENGTH-s.left);
        let Some((x,y,centered))=angel_delve::render::overworld::arena::slot_art::cell_layout("brasswake").place(x,y,podium,elapsed) else {continue;};
        let x=area.x+(x.max(0) as u32*u32::from(area.width)/384) as u16;
        let x=if centered {x.saturating_sub(text.chars().count() as u16/2)} else {x};
        let y=area.y+(y.max(0) as u32*u32::from(area.height)/224) as u16;
        for (dx,ch) in text.chars().enumerate(){if x+dx as u16>=area.right(){break;}if let Some(cell)=frame.buffer_mut().cell_mut((x+dx as u16,y)){cell.set_char(ch).set_fg(ratatui::style::Color::Rgb(176,178,168));}}
    }
}

pub(crate) fn paint_rafterwake_panels(frame:&mut ratatui::Frame,area:ratatui::layout::Rect,run:&angel_delve::rules::Run,focus:Option<u32>) {
    let podium=run.rafterwake.as_ref().is_some_and(|s|s.left==0);
    for (x,y,text) in angel_delve::render::overworld::arena::rafterwake_labels(run,focus) {
        let elapsed=run.rafterwake.as_ref().map_or(0,|s| angel_delve::rules::rafterwake::LENGTH-s.left);
        let Some((x,y,centered))=angel_delve::render::overworld::arena::slot_art::cell_layout("rafterwake").place(x,y,podium,elapsed) else {continue;};
        let count=text.chars().count() as u16;
        let row=podium && (64..=136).contains(&y);
        let header=if row && (148..196).contains(&x) {Some((148,8))}
            else if row && (212..242).contains(&x) {Some((212,5))}
            else if row && x>=250 {Some((274,6))} else {None};
        let x=area.x+if matches!(text.as_str(),"E AGAIN"|"LOFT CLEARED"|"SHIFT FINISHED"|"MOST HEIGHT, FEWER FALLS, EARLIER FINISH"|"TIE SPLIT") || text.ends_with("G POT + CROWN") {area.width.saturating_sub(count)/2}
            else if let Some((start,len))=header {((start*u32::from(area.width)/384) as u16+len).saturating_sub(count)}
            else if !podium && x>=340 && (56..=152).contains(&y) {((377*u32::from(area.width)/384) as u16).saturating_sub(count)}
            else {(x.max(0) as u32*u32::from(area.width)/384) as u16};
        let x=if centered {x.saturating_sub(text.chars().count() as u16/2)} else {x};
        let y=area.y+(y.max(0) as u32*u32::from(area.height)/224) as u16;
        for (dx,ch) in text.chars().enumerate(){if x+dx as u16>=area.right(){break;}if let Some(cell)=frame.buffer_mut().cell_mut((x+dx as u16,y)){cell.set_char(ch).set_fg(ratatui::style::Color::Rgb(176,178,168));}}
    }
}

pub(crate) fn paint_pennon_panels(frame:&mut ratatui::Frame,area:ratatui::layout::Rect,run:&angel_delve::rules::Run,focus:Option<u32>) {
    let podium=run.pennon.as_ref().is_some_and(|s|s.left==0);
    for (x,y,text) in angel_delve::render::overworld::arena::pennon_labels(run,focus) {
        let elapsed=run.pennon.as_ref().map_or(0,|s| angel_delve::rules::pennon::LENGTH-s.left);
        let Some((x,y,centered))=angel_delve::render::overworld::arena::slot_art::cell_layout("pennon").place(x,y,podium,elapsed) else {continue;};
        let count=text.chars().count() as u16;
        let row=podium && (64..=136).contains(&y);
        let header=if row && (148..196).contains(&x) {Some((148,8))}
            else if row && (212..242).contains(&x) {Some((212,5))}
            else if row && x>=250 {Some((274,6))} else {None};
        let x=area.x+if matches!(text.as_str(),"E AGAIN"|"SIEGE BROKEN"|"SHIFT FINISHED"|"MOST WORK, FEWER FALLS, EARLIER FINISH"|"TIE SPLIT") || text.ends_with("G POT + CROWN") {area.width.saturating_sub(count)/2}
            else if let Some((start,len))=header {((start*u32::from(area.width)/384) as u16+len).saturating_sub(count)}
            else if !podium && x>=340 && (56..=152).contains(&y) {((377*u32::from(area.width)/384) as u16).saturating_sub(count)}
            else {(x.max(0) as u32*u32::from(area.width)/384) as u16};
        let x=if centered {x.saturating_sub(text.chars().count() as u16/2)} else {x};
        let y=area.y+(y.max(0) as u32*u32::from(area.height)/224) as u16;
        for (dx,ch) in text.chars().enumerate(){if x+dx as u16>=area.right(){break;}if let Some(cell)=frame.buffer_mut().cell_mut((x+dx as u16,y)){cell.set_char(ch).set_fg(ratatui::style::Color::Rgb(176,178,168));}}
    }
}
#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/arcade__tests.rs"]
mod arcade_tests;

mod forest_road;
pub(crate) use forest_road::paint_forest_road_panels;
// The Round Table adventures' terminal text: one dispatch, a file each.
mod roundtable;
pub(crate) use roundtable::paint_roundtable_panels;
