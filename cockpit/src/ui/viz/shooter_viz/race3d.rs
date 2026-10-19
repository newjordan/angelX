/// The race hall uses the same 3D scene in cells, with live, legible words.
pub(super) fn paint(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    run: &crate::drive::together_shooter::Run,
    focus: Option<u32>,
) {
    use crate::drive::together_shooter::race3d::{Course, DRIFT_LAPS, TITLE};
    use ratatui::{
        layout::Rect,
        style::{Color, Style},
        widgets::{Block, Borders, Clear, Paragraph},
    };
    let Some(race) = run.race3d.as_ref() else {
        return;
    };
    let id = focus.unwrap_or(1);
    let Some(me) = race.riders.get(&id) else {
        return;
    };
    let stage = if me.drift.boost > 0 {
        me.drift.boost_stage as usize
    } else {
        me.drift.stage()
    };
    let colour = [
        Color::Gray,
        Color::Rgb(32, 121, 166),
        Color::Rgb(220, 118, 46),
        Color::Rgb(146, 84, 180),
    ][stage];
    let text_style = Style::new().bg(Color::Black).fg(Color::Rgb(176, 178, 168));
    let panel = |frame: &mut ratatui::Frame, title: &str, lines: String| {
        let width = area.width.min(64);
        let height = area.height.min(14);
        let rect = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        frame.render_widget(Clear, rect);
        frame.render_widget(
            Paragraph::new(lines)
                .style(text_style)
                .block(Block::default().title(title).borders(Borders::ALL)),
            rect,
        );
    };
    if race.done {
        let rows = race
            .order()
            .iter()
            .enumerate()
            .map(|(n, id)| {
                let r = &race.riders[id];
                let name = run.players.get(id).map_or("KNIGHT", |h| h.name.as_str());
                let time = r.finish.map_or("OUT".into(), |t| {
                    format!(
                        "{}S",
                        (t - TITLE).div_ceil(crate::drive::together_shooter::HZ)
                    )
                });
                let tally = if race.course == Course::Touge {
                    format!(
                        "{} DRIFT",
                        r.drift.score / crate::drive::together_shooter::HZ
                    )
                } else {
                    format!("{} HAUL", r.haul)
                };
                format!(
                    "{}  {:12}  {:4}  {}  +{}G",
                    n + 1,
                    name.chars().take(12).collect::<String>(),
                    time,
                    tally,
                    race.won.get(id).copied().unwrap_or(0)
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        panel(
            frame,
            race.course.board(),
            format!("{}\n\n{}", race.course.name(), rows),
        );
    } else if race.titled() {
        let [(k1, w1), (k2, w2)] = race.course.keys();
        let controls = if race.course == Course::Touge {
            "W GAS  S BRAKE  A/D STEER  SPACE DRIFT".into()
        } else {
            format!("{k1} {w1}  {k2} {w2}")
        };
        panel(
            frame,
            race.course.name(),
            format!(
                "\n{}\n\n{}\n\n{}",
                race.course.words(),
                (TITLE - race.t).div_ceil(crate::drive::together_shooter::HZ),
                controls
            ),
        );
    } else {
        let lap = if race.course == Course::Touge {
            format!(" LAP {}/{}", race.lap(id), DRIFT_LAPS)
        } else {
            String::new()
        };
        let line = format!(
            "PLACE {}  {}  {} SPEED  {}S",
            race.place(id),
            lap,
            me.vs.round() as u32,
            race.left()
        );
        frame.render_widget(Clear, Rect::new(area.x, area.y, area.width, 1));
        frame.render_widget(
            Paragraph::new(line).style(text_style),
            Rect::new(area.x, area.y, area.width, 1),
        );
        if race.course == Course::Touge && area.height >= 4 {
            let state = if me.crash > 0 {
                "SPUN OUT"
            } else if me.drift.boost > 0 {
                "BOOST"
            } else if me.drift.sliding {
                "DRIFT"
            } else {
                "GRIP"
            };
            let drift = format!(
                "{state} {stage}  {} DEG  CORNER {}  TOTAL {}",
                me.drift.angle().to_degrees().abs().round() as u32,
                if me.drift.corner > 0 {
                    me.drift.corner / crate::drive::together_shooter::HZ
                } else {
                    me.drift.last_corner / crate::drive::together_shooter::HZ
                },
                me.drift.score / crate::drive::together_shooter::HZ
            );
            frame.render_widget(Clear, Rect::new(area.x, area.bottom() - 3, area.width, 1));
            frame.render_widget(
                Paragraph::new(drift).style(text_style.fg(colour)),
                Rect::new(area.x, area.bottom() - 3, area.width, 1),
            );
            frame.render_widget(Clear, Rect::new(area.x, area.bottom() - 1, area.width, 1));
            frame.render_widget(
                Paragraph::new("W GAS  S BRAKE  A/D STEER  SPACE DRIFT")
                    .style(text_style),
                Rect::new(area.x, area.bottom() - 1, area.width, 1),
            );
        }
    }
}
