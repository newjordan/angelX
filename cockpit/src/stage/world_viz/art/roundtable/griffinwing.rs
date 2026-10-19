//! Griffinwing's terminal text over its cell picture: the shell's header on
//! the first row, then the knight's own shield, bombs and spur, the Wyrm's
//! lamps, the wing's latest call-out, and at the end the tally. The picture
//! underneath is the same one the browser seat draws (its cells sample it);
//! small words do not survive a cell, so the words are printed here.
use angel_delve::rules::{
    HZ, Run,
    griffinwing::{self as rules, Mode, St, Wyrm, rail},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Clear, Paragraph},
};

const DIM: Color = Color::Rgb(176, 178, 168);
const GOLD: Color = Color::Rgb(236, 182, 74);
const RED: Color = Color::Rgb(220, 92, 70);
const COLD: Color = Color::Rgb(143, 232, 220);

fn put(frame: &mut Frame, area: Rect, row: u16, text: String, fg: Color) {
    if row >= area.height || area.width == 0 {
        return;
    }
    let at = Rect::new(area.x, area.y + row, area.width, 1);
    frame.render_widget(Clear, at);
    frame.render_widget(
        Paragraph::new(text).style(Style::default().fg(fg).bg(Color::Black)),
        at,
    );
}

/// The keys, longest first. The shell's "Esc: leave" bar owns the last 24
/// columns of the bottom row, so the row is the longest of these that stops a
/// column short of it: whole words, never one cut by the bar.
const KEYS: [&str; 3] = [
    " WASD fly  ARROWS aim  F fire/lock  SPACE roll  E spur  Q brake  X bomb",
    " WASD fly ARROWS aim F fire SPACE roll E spur Q brake X bomb",
    " F fire  SPACE roll  E spur  Q brake  X bomb",
];

fn keys_row(width: u16) -> &'static str {
    let room = usize::from(width).saturating_sub(25);
    KEYS.iter()
        .find(|k| k.len() <= room)
        .unwrap_or(&KEYS[KEYS.len() - 1])
}

fn time(ticks: u32) -> String {
    let secs = ticks / HZ;
    format!("{}:{:02}", secs / 60, secs % 60)
}

pub(super) fn paint(frame: &mut Frame, area: Rect, run: &Run, focus: Option<u32>) {
    let Some(s) = run.griffinwing.as_ref() else {
        return;
    };
    super::header(frame, area, rules::NAME, s.elapsed, s.seats.len());
    if area.height < 6 {
        return;
    }
    let seat = focus
        .filter(|id| s.seats.contains_key(id))
        .or_else(|| s.seats.keys().next().copied());
    let Some(craft) = seat.and_then(|id| s.seats.get(&id)).map(|x| &x.craft) else {
        return;
    };
    let low = craft.shield <= 30;
    let stars: String = (0..3)
        .map(|k| if k < craft.bombs { '*' } else { '.' })
        .collect();
    put(
        frame,
        area,
        1,
        format!(
            " {} {}/3   SHIELD {:>3}   STARFALL {stars}   SPUR {:>3}%   SCORE {}",
            rail::NAMES[usize::from(s.stage.min(2))],
            s.stage + 1,
            craft.shield,
            craft.meter * 100 / rules::METER,
            craft.pts
        ),
        if low { RED } else { DIM },
    );
    if let Some(b) = &s.boss {
        if b.st != Wyrm::Rise {
            let lamps: Vec<String> = b
                .lamps
                .iter()
                .enumerate()
                .map(|(k, id)| {
                    let st = s.foes.iter().find(|f| f.id == *id).map(|f| f.st);
                    match st {
                        Some(St::Broken) => "out".to_string(),
                        Some(St::Open) => "OPEN".to_string(),
                        _ if usize::from(b.phase) == k => "shut".to_string(),
                        _ => "-".to_string(),
                    }
                })
                .collect();
            put(
                frame,
                area,
                2,
                format!(
                    " WYRM  lamps {}   reflect to open, charge to break",
                    lamps.join(" / ")
                ),
                COLD,
            );
        }
    }
    if let Some(call) = s.calls.last() {
        let name = rules::rider_name(call.who)
            .map(str::to_string)
            .or_else(|| {
                run.players
                    .get(&u32::from(call.who))
                    .map(|h| h.name.to_uppercase())
            })
            .unwrap_or_else(|| "RIDER".into());
        let me = seat == Some(u32::from(call.who));
        put(
            frame,
            area,
            area.height.saturating_sub(2),
            format!(" {name}: {}", call.line.says(me, call.who <= 6)),
            GOLD,
        );
    }
    put(
        frame,
        area,
        area.height.saturating_sub(1),
        keys_row(area.width).into(),
        DIM,
    );
    let mid = area.height / 2;
    match s.mode {
        Mode::Retry => put(
            frame,
            area,
            mid,
            "   TRY AGAIN  -  the beat starts over".into(),
            RED,
        ),
        Mode::Victory => put(frame, area, mid, "   THE WYRM FALLS".into(), GOLD),
        Mode::Tally => {
            let medal = s.medal.map_or("", |m| m.name());
            let mut rows = vec![
                (format!("   BEACON LIT  -  {medal} MEDAL"), GOLD),
                (
                    format!(
                        "   HITS {}   SAVED {}   LOST {}   TIME {}   SCORE {}",
                        s.bank.kills,
                        s.bank.saved,
                        s.bank.lost,
                        time(s.done.unwrap_or(s.elapsed)),
                        s.bank.pts
                    ),
                    DIM,
                ),
                (format!("   PURSE +{}G for each knight", s.purse()), GOLD),
                (String::new(), DIM),
            ];
            // What each knight did, a row each (the picture's own small
            // rows do not survive a cell).
            for (&id, knight) in &s.seats {
                let name = run
                    .players
                    .get(&id)
                    .map_or_else(|| format!("KNIGHT {id}"), |h| h.name.to_uppercase());
                let name: String = name.chars().take(10).collect();
                rows.push((
                    format!(
                        "   {name:<10} {:>6} PTS  {:>4} HITS  {:>3} SAVED",
                        knight.craft.pts, knight.craft.hits, knight.craft.saved
                    ),
                    if Some(id) == seat { GOLD } else { DIM },
                ));
            }
            rows.push((String::new(), DIM));
            rows.push(("   E to return".to_string(), DIM));
            // The block stands over the picture's own tally panel, whose
            // rows the cells smear: it is cleared to the panel's foot first,
            // so no half-row of it shows between the lines.
            let top = area.height * 6 / 28;
            let foot = area.height.saturating_sub(2);
            for row in top..foot {
                put(frame, area, row, String::new(), DIM);
            }
            for (k, (text, fg)) in rows.into_iter().enumerate() {
                if top + (k as u16) < foot {
                    put(frame, area, top + k as u16, text, fg);
                }
            }
        }
        Mode::Play => {
            if craft.down > 0 {
                put(
                    frame,
                    area,
                    mid,
                    format!("   DOWN  -  rising in {}", craft.down.div_ceil(HZ)),
                    RED,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angel_delve::rules::griffinwing::{Kind, Session};
    use ratatui::{Terminal, backend::TestBackend};
    use std::collections::BTreeMap;

    fn draw(run: &Run, focus: u32) -> Terminal<TestBackend> {
        draw_wide(run, focus, 96)
    }

    fn draw_wide(run: &Run, focus: u32, width: u16) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(width, 28)).unwrap();
        terminal
            .draw(|f| {
                crate::ui::viz::shooter_viz::paint_picture_ui(
                    f,
                    Rect::new(0, 0, width, 28),
                    run,
                    None,
                    Some(focus),
                )
            })
            .unwrap();
        terminal
    }

    fn row(terminal: &Terminal<TestBackend>, y: u16) -> String {
        let width = terminal.backend().buffer().area.width;
        (0..width)
            .map(|x| terminal.backend().buffer()[(x, y)].symbol().to_string())
            .collect()
    }

    /// The bottom row's keys and the shell's "Esc: leave" bar share it: at
    /// 96 columns and at a narrower 80 the keys are whole words, and the bar
    /// is whole too (the keys do not run under it).
    #[test]
    fn the_keys_row_stops_short_of_the_leave_bar_at_any_width() {
        let mut r = Run::griffinwing_for_test();
        r.step(&BTreeMap::new());
        // Holding Esc brings the bar up.
        for _ in 0..4 {
            r.step(&BTreeMap::new());
        }
        for width in [96u16, 80, 72] {
            let t = draw_wide(&r, 1, width);
            let keys = row(&t, 27);
            assert!(keys.contains("X bomb"), "{width} columns: {keys:?}");
            assert!(keys.contains("Esc: leave"), "{width} columns: {keys:?}");
            let at = keys.find("Esc: leave").unwrap();
            let before = &keys[..at];
            assert!(
                before.ends_with(' '),
                "{width} columns: the keys run into the bar: {keys:?}"
            );
        }
    }

    #[test]
    fn the_text_names_the_knights_own_numbers_the_wing_and_the_end() {
        let mut r = Run::griffinwing_for_test();
        r.join_seat(2, "Knight 2");
        r.step(&BTreeMap::new());
        for _ in 0..200 {
            let i = r.roundtable_inputs();
            r.step(&i);
        }
        let t = draw(&r, 1);
        let all: String = (0..28).map(|y| row(&t, y)).collect::<Vec<_>>().join("\n");
        assert!(row(&t, 0).contains("GRIFFINWING") && row(&t, 0).contains("2 seated"));
        assert!(
            row(&t, 1).contains("SHIELD") && row(&t, 1).contains("STARFALL"),
            "{}",
            row(&t, 1)
        );
        assert!(
            row(&t, 27).contains("SPACE roll") && row(&t, 27).contains("X bomb"),
            "the keys are printed along the bottom"
        );
        assert!(all.contains("THUNDERHEAD RUN 1/3"));
        // To the end: the tally prints its medal and the purse.
        for _ in 0..8000 {
            let i = r.roundtable_inputs();
            r.step(&i);
            if r.griffinwing.as_ref().is_some_and(Session::finished) {
                break;
            }
        }
        let t = draw(&r, 1);
        let all: String = (0..28).map(|y| row(&t, y)).collect::<Vec<_>>().join("\n");
        assert!(
            all.contains("BEACON LIT") && all.contains("PURSE") && all.contains("E to return"),
            "{all}"
        );
    }

    /// The tally on the cells: the picture's own panel is smeared to nothing
    /// at four dots a cell, so the rows print each knight and stand over a
    /// block cleared to the panel's foot, with no half-row of picture between.
    #[test]
    fn the_tally_prints_every_knights_row_over_a_cleared_block() {
        use angel_delve::rules::griffinwing::{Medal, Tally};
        let mut r = Run::griffinwing_for_test();
        for id in 2..=6 {
            r.join_seat(id, &format!("Knight {id}"));
        }
        r.step(&BTreeMap::new());
        {
            let s = r.griffinwing.as_mut().unwrap();
            s.mode = Mode::Tally;
            s.mode_t = 100;
            s.done = Some(6300);
            s.home = s.anchor();
            s.bank = Tally {
                pts: 14_800,
                base: 14_000,
                offered: 15_000,
                kills: 120,
                saved: 3,
                lost: 1,
                gates: 90,
            };
            s.medal = Some(Medal::Silver);
            for (k, seat) in s.seats.values_mut().enumerate() {
                seat.craft.pts = 2000 + k as u32 * 130;
                seat.craft.hits = 20 + k as u32;
            }
        }
        r.step(&BTreeMap::new());
        let t = draw(&r, 1);
        let rows: Vec<String> = (0..28).map(|y| row(&t, y)).collect();
        let all = rows.join("\n");
        assert!(
            all.contains("SILVER MEDAL") && all.contains("PURSE") && all.contains("E to return")
        );
        for id in 2..=6 {
            assert!(
                all.contains(&format!("KNIGHT {id}")),
                "knight {id} has a row:\n{all}"
            );
        }
        assert!(all.contains("2650 PTS"), "the points are printed:\n{all}");
        // From the panel's top to its foot: words or empty, never picture.
        for (y, text) in rows.iter().enumerate().take(26).skip(6) {
            assert!(
                !text
                    .chars()
                    .any(|c| c == '\u{2580}' || ('\u{2800}'..='\u{28ff}').contains(&c)),
                "row {y} has picture in it: {text:?}"
            );
        }
    }

    /// `ANGEL_ARENA_SHOTS=<dir> cargo test --bin angel write_griffinwing_terminal_review
    /// -- --ignored` writes the cells as drawn (a picture and the text).
    #[test]
    #[ignore = "writes Griffinwing's terminal review pictures"]
    fn write_griffinwing_terminal_review() {
        let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let rgb = |c: Color| match c {
            Color::Rgb(r, g, b) => [r, g, b],
            _ => [0, 0, 0],
        };
        let save = |terminal: &Terminal<TestBackend>, name: &str| {
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
                    let put = |img: &mut image::RgbaImage,
                               px: u32,
                               py: u32,
                               w: u32,
                               h: u32,
                               c: [u8; 3]| {
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
                        '\u{2800}'..='\u{28ff}' => {
                            let bits = c as u32 - 0x2800;
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
                            .into_iter()
                            .enumerate()
                            {
                                if bits & (1 << bit) != 0 {
                                    put(&mut img, px + dx * 4 + 1, py + dy * 4 + 1, 2, 2, fg);
                                }
                            }
                        }
                        ' ' => {}
                        _ => put(&mut img, px + 1, py + 3, cw - 2, ch - 6, fg),
                    }
                }
                text.push('\n');
            }
            img.save(dir.join(format!("griffinwing-cells-{name}.png")))
                .unwrap();
            std::fs::write(dir.join(format!("griffinwing-cells-{name}.txt")), text).unwrap();
        };
        let wants: [(&str, fn(&Session) -> bool); 8] = [
            ("title", |s| s.stage == 0 && s.t == 40),
            ("wisps", |s| s.stage == 0 && s.t == 250),
            ("locks", |s| {
                s.seats.values().any(|k| k.craft.locks.len() >= 3)
            }),
            ("turret-tell", |s| {
                s.foes
                    .iter()
                    .any(|f| f.kind == Kind::Turret && f.ticks < 25 && f.ticks > 10)
            }),
            ("lamp-open", |s| {
                s.foes.iter().any(|f| f.kind == Kind::Lamp && f.hp < f.full)
            }),
            ("coil-tell", |s| {
                s.boss
                    .as_ref()
                    .is_some_and(|b| b.st == Wyrm::Tell && b.atk == 1 && b.ticks < 25)
            }),
            ("victory", |s| {
                s.mode == angel_delve::rules::griffinwing::Mode::Victory && s.mode_t == 80
            }),
            ("tally", |s| {
                s.mode == angel_delve::rules::griffinwing::Mode::Tally && s.mode_t == 40
            }),
        ];
        for seats in [1u32, 2, 4, 6] {
            let mut r = Run::griffinwing_for_test();
            for id in 2..=seats {
                r.join_seat(id, &format!("Knight {id}"));
            }
            r.step(&BTreeMap::new());
            let mut todo = wants.to_vec();
            for _ in 0..14_000 {
                let i = r.roundtable_inputs();
                r.step(&i);
                let Some(s) = r.griffinwing.as_ref() else {
                    break;
                };
                let hits: Vec<usize> = todo
                    .iter()
                    .enumerate()
                    .filter(|(_, w)| (w.1)(s))
                    .map(|(k, _)| k)
                    .collect();
                for k in hits.into_iter().rev() {
                    let name = todo.remove(k).0;
                    save(&draw(&r, 1), &format!("seats{seats}-{name}"));
                }
                if todo.is_empty() {
                    break;
                }
            }
        }
    }
}
