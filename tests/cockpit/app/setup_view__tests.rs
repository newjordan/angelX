use super::*;
use crate::app::setup_menu::{Field, Item, Page, Pose};
use ratatui::{Terminal, backend::TestBackend};

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
fn the_page_lays_out_numbered_rows_and_each_row_is_clickable() {
    let composed = compose(&page(), 60, 30, 3, false);
    let shown = text(&composed.buffer);
    assert!(
        shown.contains("1  Use Ollama on this computer   ready"),
        "{shown}"
    );
    assert!(shown.contains("2  Sign in with ChatGPT"));
    assert!(
        shown.contains("Your browser opens."),
        "the picked row explains itself"
    );
    let items: Vec<_> = composed
        .hits
        .iter()
        .filter(|(_, h)| matches!(h, Hit::Item(_)))
        .collect();
    assert_eq!(items.len(), 3);
    assert_eq!(items[1].1, Hit::Item(1));
    assert_eq!(
        composed.cursor,
        Some(items[1].0.y),
        "the pill row is the picked row"
    );
    // The title is the card's own to set; the composer leaves its rows clear.
    assert!(!shown.lines().take(3).any(|l| l.contains("Connect")));
}

#[test]
fn a_short_card_scrolls_its_rows_to_keep_the_cursor_in_view() {
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
    let composed = compose(&page, 40, 16, 2, true);
    let shown = text(&composed.buffer);
    assert!(shown.contains("model-11"), "{shown}");
    assert!(!shown.contains("model-0 "), "{shown}");
    assert!(shown.contains("type to search"));
    assert!(shown.contains('↑'));
    assert!(composed.field.is_some());
    let last = composed
        .hits
        .iter()
        .filter(|(_, h)| matches!(h, Hit::Item(_)))
        .last()
        .unwrap();
    assert_eq!(last.1, Hit::Item(11));
    assert!(
        composed.hits.iter().any(|(_, h)| *h == Hit::Back),
        "the key bar always shows"
    );
}

#[test]
fn the_key_bar_makes_esc_and_ctrl_o_clickable() {
    let mut page = page();
    page.link = true;
    let composed = compose(&page, 100, 30, 3, true);
    let shown = text(&composed.buffer);
    assert!(shown.contains("Esc back"), "{shown}");
    assert!(shown.contains("Ctrl+O open in browser"));
    let keys: Vec<_> = composed
        .hits
        .iter()
        .map(|h| h.1)
        .filter(|h| !matches!(h, Hit::Item(_)))
        .collect();
    assert_eq!(keys, [Hit::Back, Hit::Open]);
}

#[test]
fn a_tiny_terminal_gets_plain_words() {
    let mut app = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
    app.open_setup();
    let mut terminal = Terminal::new(TestBackend::new(40, 11)).unwrap();
    terminal
        .draw(|frame| render(frame, frame.area(), &mut app))
        .unwrap();
    let shown = text(terminal.backend().buffer());
    assert!(shown.contains("Connect angelX"), "{shown}");
    assert!(!shown.contains('╭'), "no box when there is no room for one");
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
fn a_floating_card_preserves_the_workspace_outside_it() {
    let mut app = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
    app.open_setup();
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|frame| {
            frame.buffer_mut()[(0, 0)].set_symbol("W");
            render(frame, frame.area(), &mut app);
        })
        .unwrap();
    assert_eq!(
        terminal.backend().buffer()[(0, 0)].symbol(),
        "W",
        "a floating card must not clear the workspace behind it"
    );
}

#[test]
#[cfg(feature = "glass")]
fn the_native_card_carries_its_title_choices_and_click_targets() {
    let root = Rect::new(0, 0, 120, 42);
    let (card, hits) = native_card(root, &page()).unwrap();
    assert_eq!(card.title, "Connect angelX to an AI model");
    let shown = text(&card.buffer);
    assert!(shown.contains("Use Ollama on this computer"), "{shown}");
    assert!(shown.contains("Sign in with ChatGPT"), "{shown}");
    assert!(card.area.y >= 3, "room above the card for the squire");
    assert!(hits.iter().any(|(_, hit)| matches!(hit, Hit::Back)));
    for (rect, _) in &hits {
        assert!(
            rect.x >= card.area.x && rect.y >= card.area.y && rect.right() <= card.area.right()
        );
        assert_eq!(
            card.hit(rect.x + 1, rect.y),
            Some((rect.x + 1, rect.y)),
            "the card lies flat on its cells"
        );
    }
    let cursor = card.cursor.unwrap();
    assert!(
        hits.iter()
            .any(|(r, h)| *h == Hit::Item(1) && r.y == card.area.y + cursor)
    );
    assert_eq!(card.knight, angel_glass::card::Knight::Sit);
    let mut busy = page();
    busy.busy = true;
    busy.pose = Pose::Pull;
    let (working, _) = native_card(root, &busy).unwrap();
    assert!(working.busy);
    assert_eq!(working.knight, angel_glass::card::Knight::Busy);
    assert_ne!(working.serial, card.serial, "a new page plays its words in");
    let (small, hits) = native_card(Rect::new(0, 0, 52, 20), &page()).unwrap();
    assert!(text(&small.buffer).contains("Sign in with ChatGPT"));
    assert!(!hits.is_empty());
}

#[test]
#[cfg(feature = "glass")]
fn an_approval_suppresses_the_card_and_closing_releases_input() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut app = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
    app.open_setup();
    let root = Rect::new(0, 0, 100, 30);
    render_card(root, &mut app);
    let hits = app.setup.hits.clone();
    assert!(!hits.is_empty());
    assert!(app.setup.card.is_some());
    let (reply, _decision) = std::sync::mpsc::channel();
    app.pending_approval = Some(crate::app::PendingApproval {
        prompt: "test-only approval".into(),
        scope_label: None,
        reply,
    });
    render_card(root, &mut app);
    assert!(app.setup.open(), "suppression is not cancellation");
    assert!(app.setup.hits.is_empty());
    app.pending_approval = None;
    render_card(root, &mut app);
    assert_eq!(app.setup.hits, hits);
    app.setup_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.setup.open());
    assert!(
        app.setup.hits.is_empty(),
        "closing releases input before the next frame"
    );
    assert!(app.setup.card.is_none());
    app.on_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    assert_eq!(app.input, "x", "a closed menu keeps no composer input");
}
