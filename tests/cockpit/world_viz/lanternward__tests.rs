//! Lanternward on the terminal's own route: the cell picture with the words
//! printed over it as `paint_picture_ui` draws a seat's screen.
//! `ANGEL_ARENA_SHOTS=<dir> cargo test --bin angel write_lanternward_terminal_review
//! -- --ignored` writes each moment as a picture of the cells and a text dump.
use angel_delve::rules::Run;
use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};

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

const MOMENTS: &[(&str, u32)] = &[
    ("stair", 1),
    ("lock", 1),
    ("slash3", 1),
    ("parry", 1),
    ("chest", 1),
    ("lamp-lit", 2),
    ("gallery-tell", 1),
    ("vault-flare", 1),
    ("warden-beam", 2),
    ("warden-open", 4),
    ("warden-dark", 6),
    ("beacon", 1),
    ("finale", 2),
    ("down", 2),
];

#[test]
fn every_moment_prints_its_words_over_cells_that_fill_the_screen() {
    for &(moment, seats) in MOMENTS {
        let run = Run::lanternward_review_for_test(moment, seats);
        let terminal = draw(&run, 1);
        let header = row(&terminal, 0);
        assert!(header.contains("LANTERNWARD"), "{moment}: {header:?}");
        assert!(header.contains(&format!("{seats} seated")), "{moment}: {header:?}");
        // Hearts (full or, for a downed knight, empty) on the second row, the
        // keys at the foot.
        let hearts = row(&terminal, 1);
        assert!(hearts.contains('\u{2665}') || hearts.contains('\u{2661}'), "{moment}: {hearts:?}");
        assert!(row(&terminal, 27).contains("Space shield"), "{moment}");
        let lit = (1..28u16)
            .flat_map(|y| (0..96u16).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let cell = &terminal.backend().buffer()[(x, y)];
                cell.bg != Color::Black && cell.bg != Color::Reset || cell.symbol() != " "
            })
            .count();
        assert!(lit > 96 * 27 / 3, "{moment}: the picture fills the cells ({lit})");
    }
}

#[test]
fn the_warden_bar_and_the_wisps_line_are_terminal_text() {
    let run = Run::lanternward_review_for_test("warden-open", 2);
    let terminal = draw(&run, 1);
    let all: String = (0..28).map(|y| row(&terminal, y)).collect::<Vec<_>>().join("\n");
    assert!(all.contains("WARDEN"), "{all}");
    assert!(all.contains("HEART OPEN"), "{all}");
    let run = Run::lanternward_review_for_test("stair", 1);
    let terminal = draw(&run, 1);
    let foot = row(&terminal, 26);
    assert!(foot.contains("[E ROLL]"), "{foot:?}");
}

#[test]
#[ignore = "writes Lanternward's terminal review pictures"]
fn write_lanternward_terminal_review() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("lanternward");
    std::fs::create_dir_all(&dir).unwrap();
    let rgb = |c: Color| match c {
        Color::Rgb(r, g, b) => [r, g, b],
        _ => [0, 0, 0],
    };
    for &(moment, seats) in MOMENTS {
        let run = Run::lanternward_review_for_test(moment, seats);
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
                    '\u{2800}'..='\u{28ff}' => {
                        let bits = c as u32 - 0x2800;
                        for (bit, (dx, dy)) in [(0, 0), (0, 1), (0, 2), (1, 0), (1, 1), (1, 2), (0, 3), (1, 3)]
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
        img.save(dir.join(format!("{moment}-cells.png"))).unwrap();
        std::fs::write(dir.join(format!("{moment}-cells.txt")), text).unwrap();
    }
}
