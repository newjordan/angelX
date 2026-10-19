//! Bannerpeak's terminal text over its cell picture: the top rows say what the
//! pixel strip says (gold, wedges, banners, sparks, the table), the bottom row
//! the keys, and the board, the banner just raised and the finale are plain
//! words in the middle, where the cells are too coarse for a 5 by 7 font.
use angel_delve::rules::{
    HZ, Run,
    bannerpeak::{
        self as rules,
        knight::PIPS,
        layout::{cell_centre, info, sign_text},
        session::{BANNER_NAMES, BOARD, Phase, SELECT_TICKS, SUMMIT_BANNERS},
    },
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Clear, Paragraph},
};

const INK: Color = Color::Rgb(176, 178, 168);
const GOLD: Color = Color::Rgb(246, 202, 88);
const WARM: Color = Color::Rgb(226, 203, 139);
const DIM: Color = Color::Rgb(99, 101, 92);

pub(super) fn paint(frame: &mut Frame, area: Rect, run: &Run, focus: Option<u32>) {
    let Some(s) = run.bannerpeak.as_ref() else {
        return;
    };
    if area.width == 0 || area.height == 0 {
        return;
    }
    let seat = focus
        .filter(|id| s.seats.contains_key(id))
        .or_else(|| s.seats.keys().next().copied());
    let me = seat.and_then(|id| s.seats.get(&id));
    let line = |frame: &mut Frame, row: u16, text: String, fg: Color| {
        if row < area.height {
            let r = Rect::new(area.x, area.y + row, area.width, 1);
            frame.render_widget(Clear, r);
            frame.render_widget(
                Paragraph::new(text).style(Style::default().fg(fg).bg(Color::Black)),
                r,
            );
        }
    };
    let secs = s.elapsed / HZ;
    line(
        frame,
        0,
        format!(
            " {}  {}:{:02}   BANNERS {}/8   SPARKS {}/8   GOLD {}   WEDGES {}/{}   {} seated",
            rules::NAME.to_uppercase(),
            secs / 60,
            secs % 60,
            s.count(),
            s.sparks.count_ones(),
            me.map_or(0, |k| k.coins),
            me.map_or(0, |k| k.pips),
            PIPS,
            s.seats.len(),
        ),
        INK,
    );
    // The banners as eight cells, the table as a line.
    let mut banners = String::from(" ");
    for n in 0..8 {
        banners.push_str(if s.banners & (1 << n) != 0 { "[#] " } else { "[ ] " });
    }
    line(frame, 1, banners, GOLD);
    let mut table = String::new();
    for (&id, k) in &s.seats {
        let state = if k.bubble.is_some() {
            "BUBBLE".to_string()
        } else {
            format!("{}", k.pips)
        };
        table.push_str(&format!(" {id}:{state}"));
    }
    if area.width > 24 {
        let x = area.x + area.width.saturating_sub(table.len() as u16 + 1);
        if x > area.x + 40 {
            frame.render_widget(
                Paragraph::new(table).style(Style::default().fg(WARM).bg(Color::Black)),
                Rect::new(x, area.y + 1, area.right() - x, 1),
            );
        }
    }
    line(
        frame,
        area.height.saturating_sub(1),
        " SPACE jump   S duck, slam   F dive, carry   E shield   Hold Esc return".into(),
        DIM,
    );
    let centre = |frame: &mut Frame, row: u16, text: &str, fg: Color, bold: bool| {
        if row < area.height {
            let w = text.len() as u16;
            let x = area.x + area.width.saturating_sub(w) / 2;
            let r = Rect::new(x, area.y + row, w.min(area.right() - x), 1);
            let mut style = Style::default().fg(fg).bg(Color::Black);
            if bold {
                style = style.add_modifier(Modifier::BOLD);
            }
            frame.render_widget(Clear, r);
            frame.render_widget(Paragraph::new(text.to_string()).style(style), r);
        }
    };
    if s.elapsed < 150 && s.phase == Phase::Play {
        centre(frame, 4, " RAISE EIGHT BANNERS ", GOLD, true);
    }
    if let Some(a) = s.announce {
        centre(
            frame,
            4,
            &format!(" BANNER {} OF 8: {} ", s.count(), BANNER_NAMES[usize::from(a.banner)]),
            GOLD,
            true,
        );
    }
    if me.is_some_and(|k| k.bubble.is_some()) {
        centre(frame, 6, " IN A BUBBLE: STEER TO A FRIEND ", WARM, true);
    }
    // What the sign beside a knight says, and what the shut door waits for: the
    // picture's own pixel words are too fine for cells.
    if s.phase == Phase::Play && s.announce.is_none() {
        if let Some(k) = me {
            let (fx, fy) = k.body.foot();
            let i = info();
            let (ax, ay) = cell_centre(i.arch);
            if s.elapsed > 150 && s.doors & 1 == 0 && (fx - ax).abs() < 80 && (fy - ay).abs() < 40 {
                centre(frame, 7, " TWO BANNERS OPEN THIS DOOR ", GOLD, true);
            } else if let Some(&(sx, sy, g)) = i.signs.iter().find(|&&(cx, cy, _)| {
                let (x, y) = cell_centre((cx, cy));
                (fx - x).abs() < 56 && (fy - y).abs() < 40
            }) {
                let _ = (sx, sy);
                centre(frame, 7, &format!(" {} ", sign_text(g)), WARM, false);
            }
        }
    }
    match s.phase {
        Phase::Select => {
            centre(frame, 6, " MOONWELL MOUNTAIN ", GOLD, true);
            for (n, (name, banner)) in BOARD.iter().enumerate() {
                let on = usize::from(s.cursor) == n;
                let raised = s.banners & (1 << banner) != 0;
                let locked = n == 5 && s.doors & 2 == 0;
                let tag = if raised {
                    "[#]".to_string()
                } else if locked {
                    format!("{}/{}", s.door_count(), SUMMIT_BANNERS)
                } else {
                    "[ ]".to_string()
                };
                let text = format!(" {} {:<14} {} ", if on { ">" } else { " " }, name, tag);
                let fg = if locked { DIM } else if on { GOLD } else { INK };
                centre(frame, 8 + n as u16, &text, fg, on);
            }
            let left = SELECT_TICKS.saturating_sub(s.phase_t) / HZ;
            centre(frame, 15, &format!(" W S choose   E go   {left} "), DIM, false);
        }
        Phase::Finale => {
            let t = s.phase_t;
            centre(
                frame,
                4,
                if t > 120 { " THE COURT IS LIT " } else { " EIGHT BANNERS FLY " },
                GOLD,
                true,
            );
            if s.completed.is_some() {
                centre(frame, 6, " E return to the kingdom ", WARM, true);
            }
        }
        Phase::Play => {}
    }
}

#[cfg(test)]
mod tests {
    //! Bannerpeak's words on the terminal route: the board, the banner just
    //! raised, the finale and the bubble, as `paint_picture_ui` draws them over
    //! the cell picture. `ANGEL_ARENA_SHOTS=<dir> cargo test --bin angel
    //! write_bannerpeak_terminal_review -- --ignored` writes each as a picture
    //! and a text dump.
    use angel_delve::rules::{
        Run,
        bannerpeak::{
            knight::Bubble,
            session::{Announce, Phase},
        },
    };
    use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
    use std::collections::BTreeMap;

    fn party(seats: u32) -> Run {
        let mut r = Run::roundtable_for_test("bannerpeak").unwrap();
        for id in 2..=seats {
            r.join_seat(id, &format!("Knight {id}"));
        }
        r.step(&BTreeMap::new());
        r
    }

    fn moment(name: &str, seats: u32) -> Run {
        let mut r = party(seats);
        let s = r.bannerpeak.as_mut().unwrap();
        match name {
            "start" => {}
            "late" => s.elapsed = 400,
            "select" => {
                s.banners = 0b11;
                s.doors = 1;
                s.phase = Phase::Select;
                s.phase_t = 60;
            }
            "announce" => {
                s.banners = 0b111;
                s.announce = Some(Announce { banner: 2, t: 60 });
                s.elapsed = 900;
            }
            "finale" => {
                s.banners = 0xFF;
                s.sparks = 0xFF;
                s.phase = Phase::Finale;
                s.phase_t = 90;
                s.elapsed = 5000;
            }
            "finale-done" => {
                s.banners = 0xFF;
                s.sparks = 0xFF;
                s.phase = Phase::Finale;
                s.phase_t = 200;
                s.elapsed = 5000;
                s.completed = Some(5000);
            }
            "bubble" => {
                s.elapsed = 900;
                if let Some(k) = s.seats.get_mut(&1) {
                    k.bubble = Some(Bubble { x: 80 * 256, y: 700 * 256, t: 40 });
                }
            }
            other => panic!("no moment {other}"),
        }
        r
    }

    fn draw(run: &Run) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(96, 28)).unwrap();
        terminal
            .draw(|f| {
                crate::ui::viz::shooter_viz::paint_picture_ui(
                    f,
                    Rect::new(0, 0, 96, 28),
                    run,
                    None,
                    Some(1),
                )
            })
            .unwrap();
        terminal
    }

    fn rows(t: &Terminal<TestBackend>) -> Vec<String> {
        (0..28u16)
            .map(|y| {
                (0..96u16)
                    .map(|x| t.backend().buffer()[(x, y)].symbol().to_string())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn the_board_the_banner_the_finale_and_the_bubble_are_plain_words_over_the_cells() {
        let board = rows(&draw(&moment("select", 4)));
        let all = board.join("\n");
        for line in ["MOONWELL MOUNTAIN", "MARSH FLAG", "MILL RACE", "BELL SHAFT", "SNOW RUN", "EIGHT SPARKS", "KING CINDER"] {
            assert!(all.contains(line), "the board names {line}");
        }
        assert!(all.contains("W S choose"), "and how to choose");
        assert!(board[0].contains("BANNERS 2/8") && board[0].contains("4 seated"));

        let up = rows(&draw(&moment("announce", 2))).join("\n");
        assert!(up.contains("BANNER 3 OF 8"), "the banner just raised is named");

        let lit = rows(&draw(&moment("finale", 3))).join("\n");
        assert!(lit.contains("EIGHT BANNERS FLY") || lit.contains("THE COURT IS LIT"));
        let done = rows(&draw(&moment("finale-done", 3))).join("\n");
        assert!(done.contains("E return to the kingdom"), "the way home is said");

        let bubble = rows(&draw(&moment("bubble", 2))).join("\n");
        assert!(bubble.contains("IN A BUBBLE"), "the fallen knight is told what to do");

        let start = rows(&draw(&moment("start", 1))).join("\n");
        assert!(start.contains("RAISE EIGHT BANNERS"), "the title card words");
        assert!(start.contains("Hold Esc"), "the keys line");
        assert!(start.contains("RUN. JUMP."), "the first sign, said in plain text");
    }

    #[test]
    fn every_moment_fills_the_cells_for_one_and_six_seats() {
        for name in ["start", "late", "select", "announce", "finale", "finale-done", "bubble"] {
            for seats in [1, 6] {
                let t = draw(&moment(name, seats));
                let lit = (1..28u16)
                    .flat_map(|y| (0..96u16).map(move |x| (x, y)))
                    .filter(|&(x, y)| {
                        let c = &t.backend().buffer()[(x, y)];
                        c.bg != Color::Black && c.bg != Color::Reset || c.symbol() != " "
                    })
                    .count();
                assert!(lit > 96 * 27 / 2, "{name} {seats} seats: filled ({lit})");
            }
        }
    }

    #[test]
    #[ignore = "writes Bannerpeak's terminal review pictures"]
    fn write_bannerpeak_terminal_review() {
        let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let rgb = |c: Color| match c {
            Color::Rgb(r, g, b) => [r, g, b],
            _ => [0, 0, 0],
        };
        for name in ["start", "late", "select", "announce", "finale", "finale-done", "bubble"] {
            for seats in [1, 6] {
                let t = draw(&moment(name, seats));
                let buf = t.backend().buffer();
                let (cw, ch) = (8u32, 16u32);
                let mut img = image::RgbaImage::new(96 * cw, 28 * ch);
                let mut text = String::new();
                for y in 0..28u16 {
                    for x in 0..96u16 {
                        let cell = &buf[(x, y)];
                        let (fg, bg) = (rgb(cell.fg), rgb(cell.bg));
                        let symbol = cell.symbol();
                        text.push_str(symbol);
                        let c = symbol.chars().next().unwrap_or(' ');
                        let (px, py) = (u32::from(x) * cw, u32::from(y) * ch);
                        let mut put = |dx: u32, dy: u32, w: u32, h: u32, c: [u8; 3]| {
                            for yy in py + dy..py + dy + h {
                                for xx in px + dx..px + dx + w {
                                    img.put_pixel(xx, yy, image::Rgba([c[0], c[1], c[2], 255]));
                                }
                            }
                        };
                        put(0, 0, cw, ch, bg);
                        match c {
                            '\u{2580}' => put(0, 0, cw, ch / 2, fg),
                            '\u{2584}' => put(0, ch / 2, cw, ch / 2, fg),
                            '\u{2588}' => put(0, 0, cw, ch, fg),
                            ' ' => {}
                            _ => put(1, 3, cw - 2, ch - 6, fg),
                        }
                    }
                    text.push('\n');
                }
                img.save(dir.join(format!("bannerpeak-term-{name}-seats{seats}.png")))
                    .unwrap();
                std::fs::write(dir.join(format!("bannerpeak-term-{name}-seats{seats}.txt")), text)
                    .unwrap();
            }
        }
    }
}
