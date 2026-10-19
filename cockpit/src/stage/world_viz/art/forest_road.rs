//! Text remains terminal text above the Forest Road's actual cell picture.
use angel_delve::rules::{HZ, Run, classes};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Clear, Paragraph},
};
pub(crate) fn paint_forest_road_panels(
    frame: &mut Frame,
    area: Rect,
    run: &Run,
    focus: Option<u32>,
) {
    let Some(s) = run.forest_road.as_ref() else {
        return;
    };
    if area.width == 0 || area.height == 0 {
        return;
    }
    let style = Style::default()
        .fg(Color::Rgb(176, 178, 168))
        .bg(Color::Black);
    for row in (0..5).chain(area.height.saturating_sub(2)..area.height) {
        if row < area.height {
            frame.render_widget(Clear, Rect::new(area.x, area.y + row, area.width, 1));
        }
    }
    let line = |frame: &mut Frame, row: u16, text: String| {
        if row < area.height {
            frame.render_widget(Clear, Rect::new(area.x, area.y + row, area.width, 1));
            frame.render_widget(
                Paragraph::new(text).style(style),
                Rect::new(area.x, area.y + row, area.width, 1),
            );
        }
    };
    line(
        frame,
        0,
        format!(
            " FOREST ROAD   {}s   checkpoint {}/8",
            s.elapsed / HZ,
            s.checkpoint
        ),
    );
    let width = (area.width / 4).max(1);
    for (&id, k) in &s.knights {
        let x = area.x + (id as u16 - 1) * width;
        if x >= area.right() {
            continue;
        }
        let name = run
            .players
            .get(&id)
            .and_then(classes::of)
            .map_or("Knight", |c| c.name);
        frame.render_widget(
            Paragraph::new(format!("{id} {name} {}/{}", k.hp, k.max_hp)).style(style),
            Rect::new(x, area.y + 1, width.min(area.right() - x), 1),
        );
    }
    line(
        frame,
        2,
        format!(" Seat {}   run, jump, shoot", focus.unwrap_or(1)),
    );
    line(
        frame,
        area.height - 1,
        " UP jump  FIRE shoot  DOWN duck  DASH roll  Hold Esc return".into(),
    );
    if s.freeing.is_some() {
        line(frame, 5, " THE CHAINS BREAK".into());
    }
    if s.completed.is_some() {
        line(frame, 5, " SIEGE CAMPAIGN BEGUN".into());
        line(frame, 7, " Old Oak: Thank you, little knights.".into());
        line(frame, 8, " My roots will open the siege road.".into());
        line(frame, 10, " E return to kingdom".into());
    }
}
