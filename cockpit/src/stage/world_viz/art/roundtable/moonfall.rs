//! Moonfall's terminal text over its cell picture: the clock and the loop on
//! top, the Ledger's writing, and, when a knight reads the Ledger, talks or
//! rings the bell, those words in plain text (the picture's small print is
//! too fine for four-pixel cells).
use angel_delve::rules::{
    Run,
    moonfall::{
        self as rules, clock,
        state::{Panel, Phase},
        story::{note_text, page_lines},
    },
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Clear, Paragraph},
};

pub(super) fn paint(frame: &mut Frame, area: Rect, run: &Run, focus: Option<u32>) {
    let Some(s) = run.moonfall.as_ref() else {
        return;
    };
    if area.width == 0 || area.height == 0 {
        return;
    }
    let me_id = focus
        .filter(|id| s.seats.contains_key(id))
        .or_else(|| s.seats.keys().next().copied())
        .unwrap_or(1);
    let plain = Style::default().fg(Color::Rgb(205, 207, 198)).bg(Color::Black);
    let warm = Style::default().fg(Color::Rgb(247, 202, 88)).bg(Color::Black);
    let red = Style::default().fg(Color::Rgb(231, 96, 72)).bg(Color::Black);
    let line = |frame: &mut Frame, row: u16, text: String, style: Style| {
        if row < area.height {
            let rect = Rect::new(area.x, area.y + row, area.width, 1);
            frame.render_widget(Clear, rect);
            frame.render_widget(Paragraph::new(text).style(style), rect);
        }
    };
    let shown = s.shown_clock();
    let st = clock::stamp(shown, s.nums.day);
    let fin = shown >= clock::final_start(s.nums.day) && s.completed.is_none();
    let left = clock::remaining_minutes(shown, s.nums.day);
    line(
        frame,
        0,
        format!(
            " {}  DAY {}  {}   LOOP {}   {} seated{}",
            rules::NAME.to_uppercase(),
            st.day,
            st.text(),
            s.memory.loops + 1,
            s.seats.len(),
            if fin { format!("   FINAL HOURS {}:{:02}", left / 60, left % 60) } else { String::new() }
        ),
        if fin { red } else { plain },
    );
    let Some(me) = s.seats.get(&me_id).map(|seat| &seat.knight) else {
        return;
    };
    // What the Ledger has written, for the table.
    let mut row = 1;
    for &(note, _) in s.news.iter().rev().take(2) {
        line(frame, row, format!(" {}", note_text(note)), warm);
        row += 1;
    }
    if let Some((note, _)) = me.note {
        line(frame, row, format!(" {}", note_text(note)), warm);
    }
    // The picture's own clock bar is too fine for four-pixel cells: the foot
    // of the screen is text (the shell keeps the last row for "hold Esc").
    let h = area.height;
    if h >= 8 {
        let status = {
            let mask = match me.mask {
                rules::state::Mask::Bare => "",
                rules::state::Mask::Swift => "  mask SWIFT",
                rules::state::Mask::Stout => "  mask STOUT",
            };
            let held = if me.held == rules::state::Item::Empty {
                String::new()
            } else {
                format!("  holding {}", me.held.name())
            };
            format!(
                " DAY {}  {}   loop {}   candles {}{}{}",
                st.day,
                st.text(),
                s.memory.loops + 1,
                "o".repeat(usize::from(me.candles)),
                mask,
                held
            )
        };
        line(frame, h - 4, status, if fin { red } else { plain });
        let width = usize::from(area.width).saturating_sub(2).max(8);
        let end = clock::end(s.nums.day).max(1);
        let mut bar: Vec<char> = (0..width)
            .map(|i| {
                let t = (i as u64 * u64::from(end) / width as u64) as u32;
                if t >= clock::final_start(s.nums.day) {
                    '!'
                } else if clock::night(t, s.nums.day) {
                    '.'
                } else {
                    '-'
                }
            })
            .collect();
        for d in 1..3 {
            bar[width * d / 3] = '|';
        }
        let at = (u64::from(shown.min(end)) * width as u64 / u64::from(end)) as usize;
        bar[at.min(width - 1)] = if clock::night(shown, s.nums.day) { ')' } else { '*' };
        line(frame, h - 3, format!(" {}", bar.into_iter().collect::<String>()), if fin { red } else { warm });
    }
    match me.panel {
        Panel::Ledger(page) => {
            for (i, text) in page_lines(s, page).into_iter().take(usize::from(area.height).saturating_sub(9)).enumerate() {
                line(frame, 3 + i as u16, format!(" {text}"), plain);
            }
            line(frame, area.height.saturating_sub(2), " A D  pages   Q close".into(), plain);
        }
        Panel::Bell(at) => {
            let rows = ["REWIND TO FIRST DAWN", "WAIT FOR DUSK", "WAIT FOR DAWN"];
            line(frame, 3, " THE MOONWELL BELL".into(), warm);
            for (i, text) in rows.iter().enumerate() {
                line(
                    frame,
                    4 + i as u16,
                    format!(" {} {text}", if usize::from(at) == i { ">" } else { " " }),
                    plain,
                );
            }
            line(frame, area.height.saturating_sub(2), " W S choose   E ring   Q back".into(), plain);
        }
        Panel::Shut => {
            if let Some(say) = me.say {
                let lines = s.words(say.folk, say.topic);
                line(frame, area.height.saturating_sub(4), format!(" {}", say.folk.name()), warm);
                for (i, text) in lines.iter().skip(usize::from(say.page) * 2).take(2).enumerate() {
                    line(frame, area.height.saturating_sub(3) + i as u16, format!(" {text}"), plain);
                }
            }
            let hint = if s.completed.is_some() && s.phase == Phase::Play {
                " THE MOON TURNS BACK   E return to the kingdom".to_string()
            } else {
                " WASD walk  E talk  Q Ledger  R bell  Z mask  X put down  Space dodge".to_string()
            };
            line(frame, area.height.saturating_sub(2), hint, plain);
        }
    }
}

#[cfg(test)]
mod tests {
    //! Moonfall's terminal route, state by state: the Ledger, a conversation,
    //! the bell and the Final Hours drawn as `paint_picture_ui` draws a seat's
    //! screen. `ANGEL_ARENA_SHOTS=<dir> cargo test --bin angel
    //! write_moonfall_terminal_review -- --ignored` writes each as a picture
    //! of the cells and a text dump.
    use angel_delve::rules::{
        Run,
        moonfall::{
            clock,
            folk::Folk,
            ledger::Fact,
            says::Topic,
            state::{Panel, Say},
        },
    };
    use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
    use std::collections::BTreeMap;

    fn party(seats: u32) -> Run {
        let mut r = Run::moonfall_for_test();
        for id in 2..=seats {
            r.join_seat(id, &format!("Knight {id}"));
        }
        r.step(&BTreeMap::new());
        r
    }

    fn at(r: &mut Run, day: u32, hh: u32) {
        let s = r.moonfall.as_mut().unwrap();
        let since = if hh < 6 { hh + 18 } else { hh - 6 };
        s.clock = clock::ticks(s.nums.day, ((day - 1) * 24 + since) * 2);
    }

    fn draw(run: &Run) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(96, 28)).unwrap();
        terminal
            .draw(|f| {
                crate::ui::viz::shooter_viz::paint_picture_ui(f, Rect::new(0, 0, 96, 28), run, None, Some(1))
            })
            .unwrap();
        terminal
    }

    fn row(t: &Terminal<TestBackend>, y: u16) -> String {
        (0..96).map(|x| t.backend().buffer()[(x, y)].symbol().to_string()).collect()
    }

    fn dump(t: &Terminal<TestBackend>) -> String {
        (0..28).map(|y| row(t, y)).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn the_terminal_prints_the_clock_the_bar_and_the_keys_over_the_picture() {
        let mut r = party(2);
        at(&mut r, 2, 14);
        let t = draw(&r);
        assert!(row(&t, 0).contains("DAY 2") && row(&t, 0).contains("14:00"), "{:?}", row(&t, 0));
        let text = dump(&t);
        assert!(text.contains("candles ooo"), "the candles are text");
        assert!(text.contains("WASD walk"), "the keys are text");
        assert!(row(&t, 25).contains('*') && row(&t, 25).contains('|'), "the day bar and its sun: {:?}", row(&t, 25));
        at(&mut r, 3, 2);
        let t = draw(&r);
        assert!(row(&t, 0).contains("FINAL HOURS"), "{:?}", row(&t, 0));
        assert!(row(&t, 25).contains('!'), "the red hours are marked on the bar");
    }

    #[test]
    fn the_ledger_a_talk_and_the_bell_are_plain_text_in_the_cells() {
        let mut r = party(1);
        {
            let s = r.moonfall.as_mut().unwrap();
            s.memory.learn(Fact::OrinLost);
            s.memory.witness(Folk::Orin, 1);
            s.seats.get_mut(&1).unwrap().knight.panel = Panel::Ledger(4);
        }
        let text = dump(&draw(&r));
        assert!(text.contains("ORIN WREN") && text.contains("INK POT"), "{text}");
        {
            let s = r.moonfall.as_mut().unwrap();
            let k = &mut s.seats.get_mut(&1).unwrap().knight;
            k.panel = Panel::Shut;
            k.say = Some(Say { folk: Folk::Mira, topic: Topic::Request, page: 0, age: 0 });
        }
        let text = dump(&draw(&r));
        assert!(text.contains("MIRA QUILL") && text.contains("SOUTH DOOR"), "{text}");
        r.moonfall.as_mut().unwrap().seats.get_mut(&1).unwrap().knight.say = None;
        r.moonfall.as_mut().unwrap().seats.get_mut(&1).unwrap().knight.panel = Panel::Bell(1);
        let text = dump(&draw(&r));
        assert!(text.contains("REWIND TO FIRST DAWN") && text.contains("WAIT FOR DUSK"), "{text}");
    }

    #[test]
    #[ignore = "writes Moonfall's terminal review pictures"]
    fn write_moonfall_terminal_review() {
        let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let rgb = |c: Color| match c {
            Color::Rgb(r, g, b) => [r, g, b],
            _ => [0, 0, 0],
        };
        let mut runs: Vec<(&str, Run)> = Vec::new();
        let mut r = party(2);
        at(&mut r, 1, 9);
        runs.push(("morning", r));
        let mut r = party(2);
        at(&mut r, 1, 22);
        runs.push(("night", r));
        let mut r = party(4);
        at(&mut r, 3, 2);
        runs.push(("final-hours", r));
        let mut r = party(1);
        {
            let s = r.moonfall.as_mut().unwrap();
            s.memory.learn(Fact::OrinLost);
            s.memory.learn(Fact::TovinPass);
            s.memory.witness(Folk::Orin, 1);
            s.memory.witness(Folk::Orin, 2);
            s.seats.get_mut(&1).unwrap().knight.panel = Panel::Ledger(4);
        }
        runs.push(("ledger", r));
        let mut r = party(2);
        r.moonfall.as_mut().unwrap().seats.get_mut(&1).unwrap().knight.panel = Panel::Bell(0);
        runs.push(("bell", r));
        let mut r = party(1);
        r.moonfall.as_mut().unwrap().seats.get_mut(&1).unwrap().knight.say =
            Some(Say { folk: Folk::Pella, topic: Topic::Request, page: 0, age: 0 });
        runs.push(("talk", r));
        for (name, run) in runs {
            let terminal = draw(&run);
            let buf = terminal.backend().buffer();
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
                    let put = |img: &mut image::RgbaImage, px: u32, py: u32, w: u32, h: u32, c: [u8; 3]| {
                        for yy in py..py + h {
                            for xx in px..px + w {
                                img.put_pixel(xx, yy, image::Rgba([c[0], c[1], c[2], 255]));
                            }
                        }
                    };
                    let (px, py) = (u32::from(x) * cw, u32::from(y) * ch);
                    put(&mut img, px, py, cw, ch, bg);
                    match c {
                        '\u{2580}' => put(&mut img, px, py, cw, ch / 2, fg),
                        '\u{2584}' => put(&mut img, px, py + ch / 2, cw, ch / 2, fg),
                        '\u{2588}' => put(&mut img, px, py, cw, ch, fg),
                        ' ' => {}
                        // Letters: a block, so the eye can tell where text sits.
                        _ => put(&mut img, px + 1, py + 3, cw - 2, ch - 6, fg),
                    }
                }
                text.push('\n');
            }
            img.save(dir.join(format!("moonfall-terminal-{name}.png"))).unwrap();
            std::fs::write(dir.join(format!("moonfall-terminal-{name}.txt")), text).unwrap();
        }
    }
}
