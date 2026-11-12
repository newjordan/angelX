use angel_delve::rules::Run;
use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
use std::collections::BTreeMap;

fn draw(run: &Run, focus: u32) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(96, 28)).unwrap();
    terminal
        .draw(|f| {
            crate::delve::shooter_viz::paint_picture_ui(f, Rect::new(0, 0, 96, 28), run, None, Some(focus))
        })
        .unwrap();
    terminal
}

fn row(t: &Terminal<TestBackend>, y: u16) -> String {
    (0..96).map(|x| t.backend().buffer()[(x, y)].symbol().to_string()).collect()
}

/// A table of `seats` bots ridden `ticks` ticks.
fn ridden(seats: u32, ticks: u32) -> Run {
    crate::agent::harness::live_policy::install_scripted_rules();
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
    crate::agent::harness::live_policy::install_scripted_rules();
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
    crate::agent::harness::live_policy::install_scripted_rules();
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
