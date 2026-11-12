use angel_live::game::{Cartridge, Picture};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Clear, Paragraph};
use std::time::Instant;

const STATUS: Color = Color::Rgb(150, 150, 158);
const HELP: Color = Color::Rgb(104, 104, 112);

const BLACK: Color = Color::Rgb(0, 0, 0);

const SHELF_HINT: &str = "/game <name> loads one · /game <name> host · Esc gives the screen back";

pub(crate) fn fit(area: Rect, w: u32, h: u32) -> Rect {
    if area.width == 0 || area.height == 0 || w == 0 || h == 0 {
        return Rect {
            width: 0,
            height: 0,
            ..area
        };
    }
    let cols = u32::from(area.width);
    let rows = u32::from(area.height) * 2;
    let (cw, ch) = if cols * h <= rows * w {
        (cols, (cols * h / w).max(2))
    } else {
        ((rows * w / h).max(1), rows)
    };
    let width = cw.min(cols) as u16;
    let height = (ch / 2).clamp(1, u32::from(area.height)) as u16;
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

pub(crate) fn paint(frame: &mut Frame, area: Rect, picture: &Picture<'_>) {
    let (w, h) = (picture.w.max(1), picture.h.max(1));
    let rows = (u32::from(area.height) * 2).max(1);
    let cols = u32::from(area.width).max(1);
    let rgb = |x: u32, y: u32| {
        let [r, g, b] = picture.rgb((x * w / cols).min(w - 1), (y * h / rows).min(h - 1));
        Color::Rgb(r, g, b)
    };
    let buf = frame.buffer_mut();
    for cy in 0..area.height {
        for cx in 0..area.width {
            let (x, y) = (u32::from(cx), u32::from(cy) * 2);
            if let Some(cell) = buf.cell_mut((area.x + cx, area.y + cy)) {
                cell.set_char('▀').set_fg(rgb(x, y)).set_bg(rgb(x, y + 1));
            }
        }
    }
}

pub(crate) fn render(frame: &mut Frame, area: Rect, cart: &mut dyn Cartridge, harness: &str) {
    frame.render_widget(Clear, area);
    if area.width < 8 || area.height < 4 {
        return;
    }
    let status = cart.status();
    let help = format!("{} · {harness}", cart.help());
    let room = Rect {
        height: area.height - 2,
        ..area
    };
    if let Some(picture) = cart.picture(Instant::now()) {
        let at = fit(room, picture.w, picture.h);
        paint(frame, at, &picture);
    }
    let line = |y: u16| Rect {
        y,
        height: 1,
        ..area
    };
    frame.render_widget(
        Paragraph::new(status).style(Style::default().fg(STATUS)),
        line(area.bottom() - 2),
    );
    frame.render_widget(
        Paragraph::new(help).style(Style::default().fg(HELP)),
        line(area.bottom() - 1),
    );
}

fn shelf_lines(carts: &[Box<dyn Cartridge>]) -> Vec<ratatui::text::Line<'static>> {
    let mut lines: Vec<ratatui::text::Line<'static>> = carts
        .iter()
        .map(|cart| {
            let state = if cart.running() { "running" } else { "ready" };
            ratatui::text::Line::styled(
                format!("{} · {} · {state}", cart.id(), cart.title()),
                Style::default().fg(STATUS),
            )
        })
        .collect();
    lines.push(ratatui::text::Line::styled(
        SHELF_HINT,
        Style::default().fg(HELP),
    ));
    lines
}

fn draw_shelf(frame: &mut Frame, area: Rect, carts: &[Box<dyn Cartridge>]) {
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(shelf_lines(carts))
            .wrap(ratatui::widgets::Wrap { trim: true })
            .style(Style::default().bg(BLACK)),
        area,
    );
}

pub(crate) fn shelf(frame: &mut Frame, area: Rect, app: &crate::app::App) {
    draw_shelf(frame, area, &app.games.carts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shelf_lists_the_slot_and_ends_on_the_hint() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        let carts = crate::app::control::games::registry();
        let mut terminal = Terminal::new(TestBackend::new(100, 6)).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                draw_shelf(frame, area, &carts);
            })
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains("muster · The Muster · ready"));
        assert!(text.contains(SHELF_HINT));
    }

    #[test]
    fn a_wide_picture_fills_the_width_and_centres_down() {
        let at = fit(Rect::new(0, 0, 100, 40), 320, 180);
        assert_eq!(at.width, 100);
        assert_eq!(at.height, 28);
        assert_eq!(at.y, 6);
    }

    #[test]
    fn a_tall_area_keeps_the_aspect() {
        let at = fit(Rect::new(0, 0, 40, 60), 320, 180);
        assert_eq!(at.width, 40);
        assert_eq!(at.height, 11);
    }

    #[test]
    fn a_short_area_fills_the_height_and_centres_across() {
        let at = fit(Rect::new(0, 0, 200, 20), 320, 180);
        assert_eq!(at.height, 20);
        assert_eq!(at.width, 71);
        assert_eq!(at.x, 64);
    }
}
