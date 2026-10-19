//! Lightgallop's terminal text over its cell picture: the cells are four
//! pixels wide, so the small print of the heads-up display is said in text
//! rows instead (the clock, the sparks, the spur's lantern gauge, the lanterns
//! lit and the Dimming's gap), the words the picture prints in the sky (the
//! title card, the fall, the bell, the keys) are said over their own garbled
//! lettering, and the tally and its rank in a box that covers the picture's
//! panel. Row 0 is the shell's header, which the shared checks read.
use angel_delve::rules::{
    HZ, Run,
    lightgallop::{self as rules, level::level, rider::GAUGE_MAX},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};

const GOLD: Color = Color::Rgb(226, 203, 139);
const PALE: Color = Color::Rgb(176, 178, 168);
const AMBER: Color = Color::Rgb(236, 182, 74);
const RED: Color = Color::Rgb(168, 52, 31);

fn clock(ticks: u32) -> String {
    let s = ticks / HZ;
    format!("{}:{:02}", s / 60, s % 60)
}

/// A row of text on black, left-aligned at `area`'s left edge.
fn row(frame: &mut Frame, area: Rect, y: u16, text: String, fg: Color) {
    if y >= area.height || area.width == 0 {
        return;
    }
    let r = Rect::new(area.x, area.y + y, area.width, 1);
    frame.render_widget(Clear, r);
    frame.render_widget(Paragraph::new(text).style(Style::default().fg(fg).bg(Color::Black)), r);
}

pub(super) fn paint(frame: &mut Frame, area: Rect, run: &Run, focus: Option<u32>) {
    let Some(s) = run.lightgallop.as_ref() else {
        return;
    };
    super::header(frame, area, rules::NAME, s.elapsed, s.seats.len());
    let Some(id) = focus.filter(|id| s.seats.contains_key(id)).or_else(|| s.seats.keys().next().copied()) else {
        return;
    };
    let lvl = level();
    let r = &s.seats[&id].rider;
    // The spur's lantern gauge, ten cells.
    let lit = (r.gauge * 10 / GAUGE_MAX).clamp(0, 10) as usize;
    let gauge: String = "\u{2588}".repeat(lit) + &"\u{2591}".repeat(10 - lit);
    let posts = s.posts.count_ones();
    row(
        frame,
        area,
        1,
        format!(
            " SPARKS {:3}   SPUR {gauge}   CHAIN {}   LANTERNS {posts}/{}",
            r.sparks,
            r.chain,
            lvl.posts.len()
        ),
        if r.gauge < 150 { RED } else { AMBER },
    );
    let gap = ((r.x - s.dim) / 256).max(0);
    let act = ["DOWNHILL GATE", "WHEEL ARCH", "REED FEN", "BELL GATE"][rules::Session::act_at(r.x / 256)];
    let hint = if s.elapsed < 300 { "A D RUN   E LEAP   SPACE SPUR" } else { "HOLD ESC LEAVE" };
    row(
        frame,
        area,
        2,
        format!(" {act:<14} {}   GLOAM {gap:4}   {hint}", clock(s.elapsed)),
        if gap < 260 { RED } else { PALE },
    );
    words_over_the_picture(frame, area, s, r);
    if let Some(y) = &s.tally {
        tally_box(frame, area, s, y);
    }
}

/// Where the picture's pixels land on the cells: its 384 by 224 pixels cover
/// the whole area.
fn col_of(area: Rect, x: i32) -> u16 {
    area.x + (x.clamp(0, 383) * i32::from(area.width) / 384) as u16
}

fn row_of(area: Rect, y: i32) -> u16 {
    area.y + (y.clamp(0, 223) * i32::from(area.height) / 224) as u16
}

/// The words the picture prints in pixels, which four-pixel cells turn to mush:
/// said again as text, each on cells cleared of its own lettering. The lines
/// and columns are the picture's (`render/overworld/arena/lightgallop.rs`: the
/// title card in the sky, the fall and the bell's prompt under it, the keys at
/// the bottom, the bell's name over the bell). Every plate is cleared before any
/// text is laid down, so one never wipes another.
fn words_over_the_picture(frame: &mut Frame, area: Rect, s: &rules::Session, r: &rules::rider::Rider) {
    if area.width < 48 || area.height < 14 {
        return;
    }
    // (centre column in picture pixels, top row in picture pixels, text, colour)
    let mut words: Vec<(i32, i32, String, Color)> = Vec::new();
    if s.elapsed < 75 {
        words.push((192, 36, "MOONWELL ROAD".into(), GOLD));
        words.push((192, 47, "THE DIMMING COMES".into(), PALE));
    }
    if r.down > 0 {
        words.push((192, 59, "THE LANTERN CARRIES YOU".into(), GOLD));
    }
    if s.tally.is_none() && s.bell_t.is_some() {
        words.push((192, 70, "RING THE BELL".into(), GOLD));
    }
    if s.elapsed < 360 {
        words.push((192, 213, "A D RUN   E LEAP   SPACE SPUR".into(), GOLD));
    }
    if s.bell_t.is_none() {
        // The bell's name stands over the bell as tall as the bell is drawn.
        let (bx, by) = level().bell;
        let (sx, sy) = (((bx - r.cam_x) * r.zoom).div_euclid(256), ((by - r.cam_y) * r.zoom).div_euclid(256));
        if (0..384).contains(&sx) && (0..224).contains(&sy) {
            words.push((sx, sy - 72 * r.zoom / 256 - 12, "BELL GATE".into(), GOLD));
        }
    }
    let bounds = |cx: i32, y: i32, text: &str| {
        // The plate the picture lays behind its lettering: six pixels a letter, a pixel round.
        let half = (6 * text.chars().count() as i32 + 1) / 2;
        (col_of(area, cx - half), col_of(area, cx + half), row_of(area, y - 1), row_of(area, y + 8))
    };
    for (cx, y, text, _) in &words {
        let (c0, c1, r0, r1) = bounds(*cx, *y, text);
        frame.render_widget(Clear, Rect::new(c0, r0, c1 - c0 + 1, r1 - r0 + 1).intersection(area));
    }
    for (cx, y, text, fg) in &words {
        let n = text.chars().count() as u16;
        let x = col_of(area, *cx).saturating_sub(n / 2).max(area.x);
        let w = n.min(area.right().saturating_sub(x));
        if w == 0 {
            continue;
        }
        let line = Rect::new(x, row_of(area, *y + 3), w, 1).intersection(area);
        frame.render_widget(Paragraph::new(text.as_str()).style(Style::default().fg(*fg).bg(Color::Black)), line);
    }
}

/// The tally in text, over the picture's own panel (which it covers, so none of
/// its garbled lettering shows round the edges): the party's figures, the
/// lantern bonus on its own line, each knight's score, the rank.
fn tally_box(frame: &mut Frame, area: Rect, s: &rules::Session, y: &rules::Tally) {
    let lvl = level();
    let t = s.elapsed.saturating_sub(y.at);
    let rank = ["C", "B", "A", "S"][usize::from(y.rank.min(3))];
    // A hundred for all six lanterns (`make_tally`): in the points, not in each knight's own line.
    let bonus = 100 * y.posts / lvl.posts.len().max(1) as u32;
    let scores: Vec<String> = y
        .scores
        .iter()
        .map(|(id, v)| format!("{id}{} {v}", if *id == y.best { "*" } else { "" }))
        .collect();
    let table = |name: &str, value: String| format!("{name:<15}{value}");
    let mut lines = vec![
        "MOONWELL ROAD CLEAR".to_string(),
        String::new(),
        table("TIME", format!("{}   PAR {}", clock(y.time), clock(lvl.par))),
        table("SPARKS", format!("{} / {}", y.sparks, lvl.sparks.len())),
        table("BEST CHAIN", y.chain.to_string()),
        table("LANTERNS", format!("{} / {}", y.posts, lvl.posts.len())),
        table("LANTERN BONUS", format!("+{bonus}")),
        table("POINTS", y.points.to_string()),
        if t >= 110 { table("RANK", rank.to_string()) } else { String::new() },
        String::new(),
        "EACH KNIGHT".to_string(),
    ];
    // The scores in one row, or two for a full table on a narrow box.
    let one = scores.join("   ");
    if one.chars().count() + 2 <= usize::from((col_of(area, 327) - col_of(area, 56)).max(36)) {
        lines.push(one);
    } else {
        for half in scores.chunks(3) {
            lines.push(half.join("   "));
        }
    }
    lines.push(String::new());
    lines.push(if t >= 45 { "E RIDE AGAIN   HOLD ESC LEAVE".to_string() } else { String::new() });
    let (x0, x1) = (col_of(area, 56), col_of(area, 327) + 1);
    let (y0, y1) = (row_of(area, 36), row_of(area, 203) + 1);
    let w = (x1 - x0).max(36).min(area.width);
    let h = (y1 - y0).max(lines.len() as u16 + 2).min(area.height);
    let (x, y) = (x0.min(area.right() - w), y0.min(area.bottom() - h));
    let inner = usize::from(w.saturating_sub(2));
    // The table is one block, centred; the headings and the scores are each centred alone.
    let block = 30;
    let spaces = |n: usize| " ".repeat(n);
    let table_names = ["TIME", "SPARKS", "BEST CHAIN", "LANTERNS", "LANTERN BONUS", "POINTS", "RANK"];
    let centred: Vec<String> = lines
        .iter()
        .map(|l| {
            let width = if table_names.iter().any(|n| l.starts_with(n) && l.len() > n.len()) { block } else { l.chars().count() };
            spaces(inner.saturating_sub(width) / 2) + l
        })
        .collect();
    let pad = usize::from(h).saturating_sub(2 + centred.len()) / 2;
    let text = (0..pad).map(|_| String::new()).chain(centred).collect::<Vec<_>>().join("\n");
    let r = Rect::new(x, y, w, h);
    frame.render_widget(Clear, r);
    frame.render_widget(
        Paragraph::new(text)
            .style(Style::default().fg(GOLD).bg(Color::Black))
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(GOLD))),
        r,
    );
}

#[cfg(test)]
mod tests {
    use angel_delve::rules::Run;
    use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
    use std::collections::BTreeMap;

    fn draw(run: &Run, focus: u32) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(96, 28)).unwrap();
        terminal
            .draw(|f| {
                crate::ui::viz::shooter_viz::paint_picture_ui(f, Rect::new(0, 0, 96, 28), run, None, Some(focus))
            })
            .unwrap();
        terminal
    }

    fn row(t: &Terminal<TestBackend>, y: u16) -> String {
        (0..96).map(|x| t.backend().buffer()[(x, y)].symbol().to_string()).collect()
    }

    /// A table of `seats` bots ridden `ticks` ticks.
    fn ridden(seats: u32, ticks: u32) -> Run {
        let mut r = Run::lightgallop_for_test();
        for id in 2..=seats {
            r.join_seat(id, &format!("Knight {id}"));
        }
        for _ in 0..ticks {
            let inputs = r.roundtable_inputs();
            r.step(&inputs);
        }
        r
    }

    /// A table of `seats` bots that has rung the bell, 140 ticks into its tally.
    fn tallied(seats: u32) -> Run {
        let mut r = Run::lightgallop_for_test();
        for id in 2..=seats {
            r.join_seat(id, &format!("Knight {id}"));
        }
        for _ in 0..6000 {
            let inputs = r.roundtable_inputs();
            r.step(&inputs);
            if r.lightgallop.as_ref().is_some_and(|s| s.tally.is_some()) {
                break;
            }
        }
        for _ in 0..140 {
            r.step(&BTreeMap::new());
        }
        r
    }

    #[test]
    fn the_rows_say_the_sparks_the_gauge_the_lanterns_and_the_dimming() {
        let r = ridden(2, 400);
        let t = draw(&r, 1);
        let (r1, r2) = (row(&t, 1), row(&t, 2));
        assert!(r1.contains("SPARKS") && r1.contains("SPUR") && r1.contains("LANTERNS"), "{r1:?}");
        assert!(r2.contains("GLOAM") && r2.contains("DOWNHILL GATE"), "{r2:?}");
        assert!(row(&t, 0).contains("LIGHTGALLOP") && row(&t, 0).contains("2 seated"));
    }

    #[test]
    fn the_tally_is_said_in_a_box_with_its_rank_and_the_way_to_ride_again() {
        let mut r = Run::lightgallop_for_test();
        let mut done = false;
        for _ in 0..6000 {
            let inputs = r.roundtable_inputs();
            r.step(&inputs);
            if r.lightgallop.as_ref().is_some_and(|s| s.tally.is_some()) {
                done = true;
                break;
            }
        }
        assert!(done, "the bot rang the bell");
        for _ in 0..140 {
            r.step(&BTreeMap::new());
        }
        let t = draw(&r, 1);
        let all: String = (0..28).map(|y| row(&t, y)).collect::<Vec<_>>().join("\n");
        for word in ["MOONWELL ROAD CLEAR", "POINTS", "RANK", "RIDE AGAIN"] {
            assert!(all.contains(word), "{word}\n{all}");
        }
    }

    fn rows_of(t: &Terminal<TestBackend>) -> Vec<String> {
        (0..28).map(|y| row(t, y)).collect()
    }

    #[test]
    fn the_title_card_and_the_keys_are_said_in_text_over_their_garbled_lettering() {
        for seats in [1, 4, 6] {
            let r = ridden(seats, 12);
            let rows = rows_of(&draw(&r, 1));
            let all = rows.join("\n");
            let at = |word: &str| rows.iter().position(|l| l.contains(word)).unwrap_or_else(|| panic!("{word}\n{all}"));
            let (a, b) = (at("MOONWELL ROAD"), at("THE DIMMING COMES"));
            // In the sky, under the three rows of the heads-up display, one line over the other.
            assert!(a >= 3 && a < b && b <= 8, "{seats} seats: the title is on rows {a} and {b}\n{all}");
            // The keys, on the bottom rows (the top rows carry them too).
            assert!(rows[24..].iter().any(|l| l.contains("A D RUN   E LEAP   SPACE SPUR")), "{seats} seats\n{all}");
        }
        // Gone with the title card and, a few seconds on, the keys.
        let rows = rows_of(&draw(&ridden(1, 400), 1)).join("\n");
        assert!(!rows.contains("MOONWELL ROAD") && !rows.contains("DIMMING COMES"), "{rows}");
    }

    #[test]
    fn the_fall_and_the_bells_prompt_are_said_in_text() {
        let mut r = ridden(2, 200);
        r.lightgallop.as_mut().unwrap().seats.get_mut(&1).unwrap().rider.down = 12;
        let rows = rows_of(&draw(&r, 1));
        let fall = rows.iter().position(|l| l.contains("THE LANTERN CARRIES YOU")).expect("the fall is said");
        assert!((3..=12).contains(&fall), "row {fall}");
        r.lightgallop.as_mut().unwrap().bell_t = Some(5);
        let rows = rows_of(&draw(&r, 1));
        let ring = rows.iter().position(|l| l.contains("RING THE BELL")).expect("the bell's prompt is said");
        assert!(ring > fall && ring <= 12, "rows {fall} and {ring}");
    }

    #[test]
    fn the_tally_box_covers_the_pictures_panel_and_says_the_bonus_and_every_score() {
        for seats in [1, 2, 4, 6] {
            let r = tallied(seats);
            let t = draw(&r, 1);
            let rows = rows_of(&t);
            let all = rows.join("\n");
            for word in ["LANTERN BONUS", "+100", "POINTS", "EACH KNIGHT", "RANK", "RIDE AGAIN"] {
                assert!(all.contains(word), "{seats} seats: {word}\n{all}");
            }
            let y = r.lightgallop.as_ref().unwrap().tally.clone().unwrap();
            for (id, v) in &y.scores {
                assert!(all.contains(&format!("{id}")) && all.contains(&v.to_string()), "{seats} seats: seat {id}'s {v}\n{all}");
            }
            // The picture's own panel (columns 14 to 81, rows 4 to 25) is covered: no picture cell shows inside.
            let buf = t.backend().buffer();
            for yy in 5..25u16 {
                for xx in 15..81u16 {
                    assert_ne!(buf[(xx, yy)].symbol(), "\u{2580}", "{seats} seats: the picture shows through at {xx},{yy}\n{all}");
                }
            }
        }
    }

    #[test]
    #[ignore = "writes Lightgallop's terminal review pictures"]
    fn write_lightgallop_terminal_review() {
        let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let rgb = |c: Color| match c {
            Color::Rgb(r, g, b) => [r, g, b],
            _ => [0, 0, 0],
        };
        let mut shots: Vec<(String, Run)> = vec![
            ("ride-solo".to_string(), ridden(1, 1500)),
            ("ride-four".to_string(), ridden(4, 1500)),
            ("ride-six".to_string(), ridden(6, 1500)),
        ];
        for seats in [1, 2, 4, 6] {
            shots.push((format!("title-{seats}"), ridden(seats, 12)));
            shots.push((format!("tally-{seats}"), tallied(seats)));
        }
        for (name, run) in shots {
            let terminal = draw(&run, 1);
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
                        _ => put(2, 4, cw - 4, ch - 8, fg),
                    }
                }
                text.push('\n');
            }
            img.save(dir.join(format!("lightgallop-terminal-{name}.png"))).unwrap();
            std::fs::write(dir.join(format!("lightgallop-terminal-{name}.txt")), text).unwrap();
        }
    }
}
