//! A test card for the glass renderer: styles, colours, box drawing, blocks,
//! braille, emoji and CJK. `q` quits.
//!
//! ANGEL_GLASS_SHOT=/tmp/card.png ANGEL_GLASS_SHOT_CLOSE=1 cargo run --example sampler

use std::time::Duration;

use crossterm::event::{Event, KeyCode};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Modifier, Style};
use ratatui_core::terminal::Terminal;

fn paint(buf: &mut Buffer, tick: u32) {
    let area = buf.area;
    let plain = Style::default();
    let mut y = 1;
    let mut line = |buf: &mut Buffer, text: &str, style: Style| {
        if y < area.height {
            buf.set_string(2, y, text, style);
        }
        y += 1;
    };
    line(
        buf,
        "angelX glass — the cockpit's text is one layer of a GPU compositor",
        plain.add_modifier(Modifier::BOLD),
    );
    line(
        buf,
        "Regular  Bold  Italic  Dim  Underline  Crossed  Reversed",
        plain,
    );
    let styles = [
        (2, 7, Modifier::empty()),
        (11, 4, Modifier::BOLD),
        (17, 6, Modifier::ITALIC),
        (25, 3, Modifier::DIM),
        (30, 9, Modifier::UNDERLINED),
        (41, 7, Modifier::CROSSED_OUT),
        (50, 8, Modifier::REVERSED),
    ];
    for (x, w, modifier) in styles {
        let text: String = "Regular  Bold  Italic  Dim  Underline  Crossed  Reversed"
            .chars()
            .skip(x as usize - 2)
            .take(w)
            .collect();
        buf.set_string(x, y - 1, text, plain.add_modifier(modifier));
    }
    y += 1;
    let colors = [
        Color::Black,
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::Gray,
        Color::DarkGray,
        Color::LightRed,
        Color::LightGreen,
        Color::LightYellow,
        Color::LightBlue,
        Color::LightMagenta,
        Color::LightCyan,
        Color::White,
    ];
    for (i, color) in colors.iter().enumerate() {
        buf.set_string(2 + i as u16 * 4, y, " ██ ", plain.fg(*color));
        buf.set_string(
            2 + i as u16 * 4,
            y + 1,
            format!(" {i:>2} "),
            plain.bg(*color).fg(Color::White),
        );
    }
    y += 3;
    for i in 0..72u16 {
        let v = (i * 255 / 71) as u8;
        buf.set_string(2 + i, y, " ", plain.bg(Color::Rgb(v, 40, 255 - v)));
        buf.set_string(
            2 + i,
            y + 1,
            "▀",
            plain
                .fg(Color::Indexed(16 + (i % 216) as u8))
                .bg(Color::Indexed(232 + (i % 24) as u8)),
        );
    }
    y += 3;
    let boxes = [
        "╭──────╮ ┌──┬──┐ ┏━━┳━━┓ ╔══╦══╗ ┄┄┄ ┈┈┈ ╌╌╌",
        "│ round│ │  │  │ ┃  ┃  ┃ ║  ║  ║ ┆ ┊ ╎ ╱╲╳",
        "╰──────╯ ├──┼──┤ ┣━━╋━━┫ ╠══╬══╣ ╶─╴ ╺━╸",
        "         └──┴──┘ ┗━━┻━━┛ ╚══╩══╝ ┍┑┕┙ ╒╕╘╛",
    ];
    for row in boxes {
        buf.set_string(2, y, row, plain.fg(Color::Cyan));
        y += 1;
    }
    y += 1;
    buf.set_string(
        2,
        y,
        "▁▂▃▄▅▆▇█ ▏▎▍▌▋▊▉█ ░▒▓█ ▖▗▘▙▚▛▜▝▞▟ ▀▄▐▌▔▕",
        plain.fg(Color::LightYellow),
    );
    y += 2;
    // A braille ring and a half-block gradient: the realm's two pictures.
    let (bw, bh) = (40u16, 8u16);
    for cy in 0..bh {
        let mut row = String::new();
        for cx in 0..bw {
            let mut bits = 0u32;
            for (bit, (dx, dy)) in [
                (0, 0),
                (0, 1),
                (0, 2),
                (1, 0),
                (1, 1),
                (1, 2),
                (0, 3),
                (1, 3),
            ]
            .iter()
            .enumerate()
            {
                let px = f32::from(cx * 2 + dx) - 40.0;
                let py = (f32::from(cy * 4 + dy) - 16.0) * 1.0;
                let r = (px * px + py * py).sqrt();
                let wave = ((r - tick as f32 * 0.5) * 0.6).sin();
                if wave > 0.2 {
                    bits |= 1 << bit;
                }
            }
            row.push(char::from_u32(0x2800 + bits).unwrap());
        }
        buf.set_string(2, y + cy, row, plain.fg(Color::Rgb(120, 220, 255)));
    }
    for cy in 0..bh {
        for cx in 0..30u16 {
            let top = Color::Rgb((cx * 8) as u8, (cy * 30) as u8, 160);
            let bottom = Color::Rgb((cx * 8) as u8, (cy * 30 + 15) as u8, 200);
            buf.set_string(46 + cx, y + cy, "▀", plain.fg(top).bg(bottom));
        }
    }
    y += bh + 1;
    buf.set_string(
        2,
        y,
        "emoji 🦀 🛡️ ⚔️ 🏰  CJK 界語漢字  marks e\u{301} n\u{303}  arrows ▸ ◆ ● ◇ ▶",
        plain,
    );
}

fn main() {
    let result = angel_glass::run(angel_glass::Options::from_environment(), |host| {
        let mut terminal = Terminal::new(host.backend()).expect("terminal");
        let mut input = host.input_reader().expect("input");
        let mut tick = 0;
        loop {
            terminal
                .draw(|frame| {
                    // Paint a fixed card, then show what fits the window.
                    let mut card = Buffer::empty(Rect::new(0, 0, 100, 48));
                    paint(&mut card, tick);
                    let area = frame.area().intersection(card.area);
                    for y in area.top()..area.bottom() {
                        for x in area.left()..area.right() {
                            frame.buffer_mut()[(x, y)] = card[(x, y)].clone();
                        }
                    }
                })
                .expect("draw");
            terminal.set_cursor_position((2, 0)).ok();
            match input(Duration::from_millis(33)) {
                Ok(Some(Event::Key(key))) if key.code == KeyCode::Char('q') => break,
                Err(_) => break,
                _ => {}
            }
            tick += 1;
        }
    });
    if let Err(error) = result {
        eprintln!("sampler: {error}");
        std::process::exit(1);
    }
}
