use super::*;
use crate::app::setup_menu::{Field, Item, Page};
use ratatui::{Terminal, backend::TestBackend};

const PLATE: Option<(u32, u32)> = Some((1489, 874));

fn inside(outer: Rect, inner: Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.right() <= outer.right()
        && inner.bottom() <= outer.bottom()
}

#[test]
fn a_big_terminal_gets_the_scroll_and_the_guide_beside_it() {
    let root = Rect::new(0, 0, 180, 50);
    let lay = layout(root, PLATE, true, (8, 16));
    let scroll = lay.scroll.expect("scroll");
    let guide = lay.guide.expect("guide");
    assert!(inside(scroll, lay.write));
    assert!(lay.write.height >= 20, "{lay:?}");
    assert!(lay.write.width >= 60, "{lay:?}");
    assert!(
        guide.right() <= scroll.x,
        "the guide stands left of the scroll"
    );
    assert_eq!(guide.bottom(), scroll.bottom(), "and on the same floor");
    // Square frames: twice as many columns as rows on 8x16 cells.
    assert_eq!(guide.width, guide.height * 2);
    assert_eq!(lay.bar, Rect::new(0, 49, 180, 1));
    assert!(scroll.bottom() <= lay.bar.y);
    // The parchment stops widening; the rest is centred black.
    assert!(scroll.width <= 96 + 40, "{scroll:?}");
}

#[test]
fn an_80_by_24_terminal_keeps_the_scroll_and_drops_the_guide() {
    let lay = layout(Rect::new(0, 0, 80, 24), PLATE, true, (8, 16));
    let scroll = lay.scroll.expect("scroll");
    assert!(lay.guide.is_none());
    assert!(inside(scroll, lay.write));
    assert!(lay.write.height >= 12, "{lay:?}");
    assert!(lay.write.width >= 50, "{lay:?}");
}

#[test]
fn a_tiny_terminal_gets_plain_text() {
    let lay = layout(Rect::new(0, 0, 40, 11), PLATE, true, (8, 16));
    assert!(lay.scroll.is_none());
    assert!(lay.guide.is_none());
    assert!(lay.write.width > 0 && lay.write.height > 0);
}

#[test]
fn without_the_plate_the_scroll_is_plain_parchment() {
    let lay = layout(Rect::new(0, 0, 100, 30), None, false, (8, 16));
    let scroll = lay.scroll.expect("scroll");
    assert!(inside(scroll, lay.write));
    assert_eq!(lay.write.x, scroll.x + 3 + 2);
}

#[test]
fn spans_walk_the_three_slices_in_order() {
    // 10 target pixels: 2 for the left slice, 6 stretched, 2 for the right.
    let source = [0.0, 20.0, 80.0, 100.0];
    assert_eq!(span(0, [2, 6, 2], source), (0.0, 10.0));
    assert_eq!(span(1, [2, 6, 2], source), (10.0, 20.0));
    assert_eq!(span(2, [2, 6, 2], source), (20.0, 30.0));
    assert_eq!(span(7, [2, 6, 2], source), (70.0, 80.0));
    assert_eq!(span(9, [2, 6, 2], source), (90.0, 100.0));
}

#[test]
fn the_plate_paints_clear_corners_and_solid_parchment() {
    let mut plate = image::RgbaImage::new(40, 20);
    for (x, y, p) in plate.enumerate_pixels_mut() {
        // A clear border two pixels deep, parchment inside.
        *p = if x < 2 || y < 2 || x >= 38 || y >= 18 {
            image::Rgba([0, 0, 0, 0])
        } else {
            image::Rgba([240, 220, 180, 255])
        };
    }
    let painted = painted(&plate, 30, 10);
    assert_eq!(painted.pixels.len(), 30 * 20);
    assert_eq!(painted.pixels[0], None);
    assert_eq!(painted.pixels[10 * 30 + 15], Some([240, 220, 180]));
}

fn page() -> Page {
    Page {
        title: "Connect angelX to an AI model".into(),
        lines: vec!["angelX needs an AI model before it can do real work.".into()],
        items: [
            "Use Ollama on this computer",
            "Sign in with ChatGPT",
            "Not now",
        ]
        .iter()
        .enumerate()
        .map(|(i, label)| Item {
            label: label.to_string(),
            about: if i == 1 {
                "Your browser opens.".into()
            } else {
                String::new()
            },
            ready: i == 0,
        })
        .collect(),
        cursor: 1,
        field: None,
        note: None,
        pose: Pose::Point,
        busy: false,
        keys: vec![("↑↓", "choose"), ("Enter", "pick"), ("Esc", "back")],
        link: false,
    }
}

fn text(buf: &Buffer) -> String {
    let area = buf.area;
    (area.top()..area.bottom())
        .map(|y| {
            (area.left()..area.right())
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_page_inks_numbered_rows_and_each_row_is_clickable() {
    let area = Rect::new(0, 0, 60, 16);
    let mut buf = Buffer::empty(area);
    let mut hits = Vec::new();
    ink(&mut buf, area, &page(), ON_PARCHMENT, 0.0, &mut hits);
    let shown = text(&buf);
    assert!(shown.contains("Connect angelX to an AI model"));
    assert!(shown.contains("  1  Use Ollama on this computer  ✓ ready"));
    assert!(shown.contains("▶ 2  Sign in with ChatGPT"));
    assert!(
        shown.contains("Your browser opens."),
        "the picked row explains itself"
    );
    assert_eq!(hits.len(), 3);
    assert_eq!(hits[1].1, Hit::Item(1));
}

#[test]
fn a_short_page_scrolls_its_rows_to_keep_the_cursor_in_view() {
    let mut page = page();
    page.items = (0..12)
        .map(|i| Item {
            label: format!("model-{i}"),
            about: String::new(),
            ready: false,
        })
        .collect();
    page.cursor = 11;
    page.field = Some(Field {
        shown: String::new(),
        hint: "type to search",
    });
    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    let mut hits = Vec::new();
    ink(&mut buf, area, &page, ON_PARCHMENT, 0.0, &mut hits);
    let shown = text(&buf);
    assert!(shown.contains("▶ model-11"), "{shown}");
    assert!(!shown.contains("model-0 "), "{shown}");
    assert!(shown.contains("type to search"));
    assert!(shown.contains('▲'));
    assert_eq!(hits.last().unwrap().1, Hit::Item(11));
}

#[test]
fn the_key_bar_makes_esc_and_ctrl_o_clickable() {
    let mut page = page();
    page.link = true;
    let bar = Rect::new(0, 0, 100, 1);
    let mut buf = Buffer::empty(bar);
    let mut hits = Vec::new();
    key_bar(&mut buf, bar, &page, &mut hits);
    let shown = text(&buf);
    assert!(shown.contains(" Esc  back"));
    assert!(shown.contains(" Ctrl+O  open in browser"));
    assert_eq!(
        hits.iter().map(|h| h.1).collect::<Vec<_>>(),
        [Hit::Back, Hit::Open]
    );
}

#[test]
fn the_cockpit_draws_the_menu_over_everything() {
    let mut app = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
    app.open_setup();
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| crate::ui::draw::ui(frame, &mut app))
        .unwrap();
    let shown = text(terminal.backend().buffer());
    assert!(shown.contains("Connect angelX to an AI model"), "{shown}");
    assert!(shown.contains("Paste an API key"));
    assert!(!app.setup.hits.is_empty());
    // A click on a row goes through the hit it drew.
    let (rect, _) = app
        .setup
        .hits
        .iter()
        .find(|(_, hit)| *hit == Hit::Item(2))
        .copied()
        .unwrap();
    app.on_mouse(ratatui::crossterm::event::MouseEvent {
        kind: ratatui::crossterm::event::MouseEventKind::Down(
            ratatui::crossterm::event::MouseButton::Left,
        ),
        column: rect.x + 2,
        row: rect.y,
        modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
    });
    terminal
        .draw(|frame| crate::ui::draw::ui(frame, &mut app))
        .unwrap();
    let shown = text(terminal.backend().buffer());
    assert!(shown.contains("Which company is your key from?"), "{shown}");
}
