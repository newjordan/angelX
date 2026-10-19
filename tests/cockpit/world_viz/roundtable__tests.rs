//! The Round Table stubs on the terminal's own route: the cell picture and
//! the header text over it, as `paint_picture_ui` draws a seat's screen.
//! `ANGEL_ARENA_SHOTS=<dir> cargo test --bin angel write_roundtable_terminal_review
//! -- --ignored` writes each as a picture (cells as drawn) and a text dump.
use angel_delve::rules::{MAX_PLAYERS, Run, roundtable::ADVENTURES};
use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
use std::collections::BTreeMap;

fn party(slug: &str, seats: u32) -> Run {
    let mut r = Run::roundtable_for_test(slug).unwrap();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.step(&BTreeMap::new());
    r
}

fn draw(run: &Run, focus: u32) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(96, 28)).unwrap();
    terminal
        .draw(|f| {
            crate::ui::viz::shooter_viz::paint_picture_ui(
                f,
                Rect::new(0, 0, 96, 28),
                run,
                None,
                Some(focus),
            )
        })
        .unwrap();
    terminal
}

fn row(terminal: &Terminal<TestBackend>, y: u16) -> String {
    (0..96)
        .map(|x| terminal.backend().buffer()[(x, y)].symbol().to_string())
        .collect()
}

#[test]
fn each_stub_prints_its_header_over_a_picture_that_fills_the_cells() {
    for a in &ADVENTURES {
        for seats in [1, 4, MAX_PLAYERS] {
            let terminal = draw(&party(a.slug, seats), 1);
            let header = row(&terminal, 0);
            assert!(
                header.contains(&a.name.to_uppercase()),
                "{} header: {header:?}",
                a.slug
            );
            assert!(
                header.contains(&format!("{seats} seated")),
                "{} header: {header:?}",
                a.slug
            );
            let lit = (1..28u16)
                .flat_map(|y| (0..96u16).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    let cell = &terminal.backend().buffer()[(x, y)];
                    cell.bg != Color::Black && cell.bg != Color::Reset || cell.symbol() != " "
                })
                .count();
            assert!(
                lit > 96 * 27 / 2,
                "{} {seats} seats: the picture fills the cells ({lit})",
                a.slug
            );
        }
    }
}

#[test]
fn the_cells_differ_by_adventure_and_by_seat() {
    let dump =
        |t: &Terminal<TestBackend>| (0..28).map(|y| row(t, y)).collect::<Vec<_>>().join("\n");
    let mut seen = std::collections::BTreeSet::new();
    for a in &ADVENTURES {
        assert!(
            seen.insert(dump(&draw(&party(a.slug, 4), 1))),
            "{} draws its own cells",
            a.slug
        );
        assert_ne!(
            dump(&draw(&party(a.slug, 2), 1)),
            dump(&draw(&party(a.slug, 4), 1)),
            "{}",
            a.slug
        );
    }
}

#[test]
#[ignore = "writes the Round Table stubs' terminal review pictures"]
fn write_roundtable_terminal_review() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let rgb = |c: Color| match c {
        Color::Rgb(r, g, b) => [r, g, b],
        _ => [0, 0, 0],
    };
    for a in &ADVENTURES {
        for seats in [1, 4, MAX_PLAYERS] {
            let terminal = draw(&party(a.slug, seats), 1);
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
            img.save(dir.join(format!("{}-cells-seats{seats}.png", a.slug)))
                .unwrap();
            std::fs::write(dir.join(format!("{}-cells-seats{seats}.txt", a.slug)), text).unwrap();
        }
    }
}
