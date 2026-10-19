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

#[test]
fn floating_setup_scroll_preserves_the_workspace_outside_its_paper() {
    let mut app = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
    app.open_setup();
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal.draw(|frame| {
        frame.buffer_mut()[(0, 0)].set_symbol("W");
        render(frame, frame.area(), &mut app);
    }).unwrap();
    assert_eq!(terminal.backend().buffer()[(0, 0)].symbol(), "W",
        "a floating menu must not clear the workspace behind it");
}

#[test]
#[cfg(feature = "glass")]
fn native_setup_scroll_keeps_title_choices_and_global_click_targets() {
    use angel_glass::scroll::{Page as ScrollPage, Pose as ScrollPose};
    let root = Rect::new(0,0,120,42);
    let (area,buf,hits) = native_content(root,&page());
    let shown = text(&buf);
    assert!(shown.contains("Connect angelX to an AI model"),"{shown}");
    assert!(shown.contains("Use Ollama on this computer"),"{shown}");
    assert!(shown.contains("Sign in with ChatGPT"),"{shown}");
    assert!(hits.iter().any(|(_,hit)| matches!(hit,Hit::Back)));
    let scroll = ScrollPage::new(area,buf,ScrollPose {open:1.0,sway:0.0}).unwrap();
    for (rect,hit) in &hits {
        if matches!(hit,Hit::Item(_)) {
            assert!(rect.x>=area.x && rect.y>=area.y && rect.right()<=area.right());
            let mapped = scroll.hit(rect.x+2,rect.y+1);
            assert!(mapped.is_some(),"visible menu actions need a projected target");
        }
    }
    let (_,short,hits) = native_content(Rect::new(0,0,44,20),&page());
    assert!(text(&short).contains("Sign in with ChatGPT"));
    assert!(!hits.is_empty());
}

#[test]
#[cfg(feature = "glass")]
fn native_setup_key_bar_lies_on_the_flat_paper() {
    // The paper curls under the lower roller past `readable_rows`: a key
    // bar there is hidden behind it, and its click targets with it.
    use angel_glass::scroll::{Page as ScrollPage, Pose as ScrollPose, readable_rows};
    for rows in [20u16, 30, 35, 37, 42, 60] {
        let root = Rect::new(0, 0, 120, rows);
        let (area, buf, hits) = native_content(root, &page());
        let bar = hits.iter().find(|(_, hit)| matches!(hit, Hit::Back)).map(|(rect, _)| *rect).unwrap();
        assert!(bar.y - area.y < readable_rows(area.height), "rows {rows}: bar at {} of {}", bar.y - area.y, area.height);
        let scroll = ScrollPage::new(area, buf, ScrollPose { open: 1.0, sway: 0.0 }).unwrap();
        // Clicking where the bar is drawn lands on the bar's row.
        let drawn = (bar.y..area.bottom()).find(|&row| scroll.hit(bar.x, row).is_some_and(|(_, r)| r == bar.y));
        assert!(drawn.is_some(), "rows {rows}: the bar has a visible click target");
    }
}

#[test]
#[cfg(feature = "glass")]
fn native_setup_scroll_reduced_motion_closes_without_retaining_input() {
    use crate::ui::viz::lifecycle_viz::MotionMode;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    for mode in [MotionMode::Reduced, MotionMode::Off] {
        let mut app = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
        app.visual_motion = mode;
        app.open_setup();
        render_scroll(Rect::new(0, 0, 100, 30), &mut app);
        assert_eq!(app.setup.scroll_motion.pose().open, 1.0);
        assert!(!app.setup.scroll_motion.moving());
        assert!(app.setup.scroll_page.as_ref().unwrap().interactive);
        assert!(!app.setup.hits.is_empty());
        app.setup_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.setup.open());
        assert!(app.setup.hits.is_empty(), "closing releases input before the next frame");
        render_scroll(Rect::new(0, 0, 100, 30), &mut app);
        assert!(app.setup.scroll_motion.hidden());
        assert!(!app.setup.scroll_motion.moving());
        assert!(app.setup.scroll_page.is_none());
        app.on_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        assert_eq!(app.input, "x", "closed menu cannot keep composer input");
    }
}

#[test]
#[cfg(feature = "glass")]
fn native_setup_scroll_suppression_disables_hits_then_restores_them() {
    use crate::ui::viz::lifecycle_viz::MotionMode;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut app = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
    app.visual_motion = MotionMode::Full;
    app.open_setup();
    app.setup.scroll_motion.snap(true, Instant::now());
    let root = Rect::new(0, 0, 100, 30);
    render_scroll(root, &mut app);
    let hits = app.setup.hits.clone();
    assert!(!hits.is_empty());
    let (reply, _decision) = std::sync::mpsc::channel();
    app.pending_approval = Some(crate::app::PendingApproval {
        prompt: "test-only approval".into(), scope_label: None, reply,
    });
    render_scroll(root, &mut app);
    assert!(app.setup.open(), "suppression is not cancellation");
    assert!(app.setup.hits.is_empty());
    let page = app.setup.scroll_page.as_ref().unwrap();
    assert!(!page.interactive);
    assert!(page.hit(hits[0].0.x, hits[0].0.y).is_none());
    app.pending_approval = None;
    render_scroll(root, &mut app);
    assert_eq!(app.setup.hits, hits);
    assert!(app.setup.scroll_page.as_ref().unwrap().interactive);
    // Exercise the retained page used during a full-motion exit, independently
    // of a real window host. Input ownership must end before paper disappears.
    app.setup_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    app.setup.scroll_motion.target(false, Instant::now());
    render_scroll(root, &mut app);
    assert!(!app.setup.open());
    assert!(app.setup.hits.is_empty());
    assert!(!app.setup.scroll_page.as_ref().unwrap().interactive);
    app.setup.scroll_motion.snap(false, Instant::now());
    render_scroll(root, &mut app);
    assert!(app.setup.scroll_page.is_none());
}
