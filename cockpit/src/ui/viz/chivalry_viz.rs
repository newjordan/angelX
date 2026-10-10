//! Real Dotmax side stage in the ordinary Delve overlay (braille transport on
//! all terminals, deliberately no Kitty worker or opt-in art fixture).
use crate::{
    drive::chivalry::{Mount, Place, Visit},
    stage::world_viz::World,
    ui::viz::lifecycle_viz::MotionMode,
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Clear, Paragraph, Wrap},
};
pub(crate) fn render(
    frame: &mut Frame,
    world: &World,
    visit: Visit,
    area: Rect,
    notice: &str,
    motion: MotionMode,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    frame.render_widget(Clear, area);
    let block = Block::bordered().title(format!(
        "{} · HOST-LOCAL PRACTICE · solo Delve paused",
        visit.place.label()
    ));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let status = match visit.place {
        Place::Stables => {
            let state = &world.chivalry;
            let roster = Mount::ALL
                .map(|mount| {
                    format!(
                        "{} {}{}",
                        mount.name(),
                        mount.specialty().word(),
                        if state.tended[mount.index()] {
                            " / tended"
                        } else {
                            ""
                        }
                    )
                })
                .join(" · ");
            format!("{} selected · {roster}", state.selected.name())
        }
        Place::Tournament => world.chivalry.tournament.status(),
    };
    let stable = visit.place == Place::Stables;
    let header_h = if stable {
        if inner.width >= 90 { 1 } else { 2 }
    } else {
        3
    };
    let header = Rect::new(inner.x, inner.y, inner.width, inner.height.min(header_h));
    frame.render_widget(
        Paragraph::new(status)
            .style(Style::new().fg(Color::Rgb(218, 181, 113)))
            .wrap(Wrap { trim: false }),
        header,
    );
    // The status belongs at the top once. Entry/status notices echo that same
    // string; only an actual action notice gets another line below the keys.
    let notice = if notice == world.chivalry.stable_status()
        || notice == world.chivalry.tournament.status()
    {
        ""
    } else {
        notice
    };
    let footer_rows = if inner.width >= 90 {
        1 + u16::from(!notice.is_empty())
    } else {
        3
    };
    let footer_h = inner.height.saturating_sub(header.height).min(footer_rows);
    let view = Rect::new(
        inner.x,
        header.bottom(),
        inner.width,
        inner.height.saturating_sub(header.height + footer_h),
    );
    if view.width > 0 && view.height > 0 {
        let image = world.chivalry_frame(
            visit,
            usize::from(view.width),
            usize::from(view.height),
            0.0,
            0.0,
            0.0,
            1.05,
            motion,
        );
        for y in 0..image.height.min(usize::from(view.height)) {
            for x in 0..image.width.min(usize::from(view.width)) {
                let c = image.cells[y * image.width + x];
                if let Some(cell) = frame
                    .buffer_mut()
                    .cell_mut((view.x + x as u16, view.y + y as u16))
                {
                    cell.set_char(c.glyph)
                        .set_fg(Color::Rgb(c.fg[0], c.fg[1], c.fg[2]))
                        .set_bg(Color::Black);
                }
            }
        }
    }
    let keys = match visit.place {
        Place::Stables => "1 Bramble · 2 Cinder · 3 Mist · E tend (+1 first pass)",
        Place::Tournament => "Enter start · G guard · A aim · C charge",
    };
    let footer = Rect::new(inner.x, inner.bottom() - footer_h, inner.width, footer_h);
    frame.render_widget(
        Paragraph::new(format!(
            "{keys} · arrows walk aisle · Esc leave\n{}",
            notice.lines().next().unwrap_or("")
        ))
        .style(Style::new().fg(Color::Rgb(169, 189, 201)))
        .wrap(Wrap { trim: false }),
        footer,
    );
}
