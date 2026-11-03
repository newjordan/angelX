//! Offline, self-closing look at the card: the first-launch title, a list
//! with the pill gliding, a key page, a job running, done, then an
//! approval. Frames land in ANGEL_CARD_OUT as the timeline plays.
//! ANGEL_CARD_OUT=/absolute/path cargo run --release --example card_menu
use angel_glass::card::{Card, Entrance, Knight, TITLE_ROWS};
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    terminal::Terminal,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

const TEXT: Color = Color::Rgb(176, 180, 188);
const SOFT: Color = Color::Rgb(118, 123, 132);
const BRIGHT: Color = Color::Rgb(224, 226, 230);
const ACCENT: Color = Color::Rgb(196, 134, 92);
const READY: Color = Color::Rgb(138, 172, 124);

struct Page {
    title: &'static str,
    lines: Vec<&'static str>,
    items: Vec<(&'static str, bool)>,
    cursor: usize,
    field: Option<&'static str>,
    keys: Vec<(&'static str, &'static str)>,
    knight: Knight,
    busy: bool,
    serial: u64,
}

fn card(page: &Page, size: (u16, u16), entrance: Entrance) -> Option<Card> {
    let width = 76.min(size.0.saturating_sub(8));
    let mut rows: Vec<Vec<(String, Style)>> = Vec::new();
    for line in &page.lines {
        rows.push(vec![(line.to_string(), Style::default().fg(TEXT))]);
    }
    let mut cursor = None;
    let mut field = None;
    if let Some(shown) = page.field {
        rows.push(vec![]);
        field = Some(rows.len() as u16);
        rows.push(vec![
            ("› ".into(), Style::default().fg(ACCENT)),
            (shown.into(), Style::default().fg(BRIGHT)),
            ("▏".into(), Style::default().fg(SOFT)),
        ]);
    }
    if !page.items.is_empty() {
        rows.push(vec![]);
    }
    for (i, (label, ready)) in page.items.iter().enumerate() {
        let picked = i == page.cursor;
        if picked {
            cursor = Some(rows.len() as u16);
        }
        let mut row = vec![
            (format!("{}  ", i + 1), Style::default().fg(if picked { ACCENT } else { SOFT })),
            (
                label.to_string(),
                if picked { Style::default().fg(BRIGHT).add_modifier(Modifier::BOLD) } else { Style::default().fg(TEXT) },
            ),
        ];
        if *ready {
            row.push(("   ready".into(), Style::default().fg(READY)));
        }
        rows.push(row);
    }
    rows.push(vec![]);
    let keys = rows.len();
    let mut bar = Vec::new();
    for (key, word) in &page.keys {
        bar.push((key.to_string(), Style::default().fg(ACCENT)));
        bar.push((format!(" {word}     "), Style::default().fg(SOFT)));
    }
    rows.push(bar);
    let height = TITLE_ROWS + rows.len() as u16 + 1;
    let area = Rect::new((size.0 - width) / 2, (size.1.saturating_sub(height)) / 2, width, height);
    let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
    for (i, row) in rows.iter().enumerate() {
        let y = TITLE_ROWS + i as u16;
        let mut x = 3;
        let room = width - 3;
        for (text, style) in row {
            let (nx, _) = buf.set_stringn(x, y, text, usize::from(room.saturating_sub(x)), *style);
            x = nx;
        }
        let _ = keys;
    }
    let mut card = Card::new(area, buf)?;
    card.title = page.title.into();
    card.serial = page.serial;
    card.cursor = cursor.map(|r| r + TITLE_ROWS);
    card.field = field.map(|r| r + TITLE_ROWS);
    card.knight = page.knight;
    card.busy = page.busy;
    card.entrance = entrance;
    Some(card)
}

fn main() {
    let out = std::env::var_os("ANGEL_CARD_OUT").map(PathBuf::from).expect("ANGEL_CARD_OUT is required");
    std::fs::create_dir_all(&out).unwrap();
    let every = std::env::var("ANGEL_CARD_EVERY").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.1);
    let mut options = angel_glass::Options::from_environment();
    options.columns = 140;
    options.rows = 44;
    let result = angel_glass::run(options, move |host| {
        let mut terminal = Terminal::new(host.backend()).unwrap();
        let list = |cursor: usize, knight: Knight, serial: u64| Page {
            title: "Connect angelX to an AI model",
            lines: vec!["angelX needs an AI model before it can do real work.", "Pick one: use ↑ ↓ and Enter, or press its number."],
            items: vec![
                ("Sign in with ChatGPT", false),
                ("A free model on this computer", true),
                ("Paste an API key", false),
                ("Find a model on my network", false),
                ("Not now", false),
            ],
            cursor,
            field: None,
            keys: vec![("↑↓", "choose"), ("Enter", "pick"), ("Esc", "back")],
            knight,
            busy: false,
            serial,
        };
        // (seconds, page, entrance); None closes the card.
        let script: Vec<(f32, Option<Page>, Entrance)> = vec![
            (0.0, Some(list(0, Knight::Wave, 1)), Entrance::Title),
            (3.6, Some(list(0, Knight::Sit, 1)), Entrance::Title),
            (4.0, Some(list(1, Knight::Sit, 1)), Entrance::Title),
            (4.5, Some(list(2, Knight::Sit, 1)), Entrance::Title),
            (5.2, Some(Page {
                title: "Paste your DeepSeek key",
                lines: vec![
                    "1. Make a key at platform.deepseek.com  (Ctrl+O opens it)",
                    "2. Copy the key, then paste it here.",
                    "3. Press Enter. We'll check that it works.",
                ],
                items: vec![],
                cursor: 0,
                field: Some("sk-••••••••••••••••3f9a"),
                keys: vec![("Enter", "check key"), ("Ctrl+O", "open site"), ("Esc", "back")],
                knight: Knight::Present,
                busy: false,
                serial: 2,
            }), Entrance::Title),
            (7.2, Some(Page {
                title: "Checking your key",
                lines: vec!["Asking DeepSeek whether the key works…"],
                items: vec![],
                cursor: 0,
                field: None,
                keys: vec![("Esc", "stop")],
                knight: Knight::Busy,
                busy: true,
                serial: 3,
            }), Entrance::Title),
            (10.0, Some(Page {
                title: "You're all set",
                lines: vec![
                    "angelX now uses DeepSeek.",
                    "Close this, type what you want to make, and press Enter.",
                    "",
                    "Type /setup to change it any time.",
                ],
                items: vec![],
                cursor: 0,
                field: None,
                keys: vec![("Enter", "start")],
                knight: Knight::Cheer,
                busy: false,
                serial: 4,
            }), Entrance::Title),
            (12.6, None, Entrance::Title),
            (13.4, Some(Page {
                title: "Allow this command?",
                lines: vec!["cargo test -p angel-glass", "", "in ~/angelX"],
                items: vec![("Allow", false), ("Allow for this turn", false), ("Deny", false)],
                cursor: 0,
                field: None,
                keys: vec![("y", "allow"), ("a", "all this turn"), ("n", "deny")],
                knight: Knight::Guard,
                busy: false,
                serial: 9,
            }), Entrance::Pop),
            (14.4, Some(Page {
                title: "Allow this command?",
                lines: vec!["cargo test -p angel-glass", "", "in ~/angelX"],
                items: vec![("Allow", false), ("Allow for this turn", false), ("Deny", false)],
                cursor: 2,
                field: None,
                keys: vec![("y", "allow"), ("a", "all this turn"), ("n", "deny")],
                knight: Knight::Guard,
                busy: false,
                serial: 9,
            }), Entrance::Pop),
            (15.6, None, Entrance::Pop),
            (16.4, None, Entrance::Pop),
        ];
        // The window assembles itself first; the card comes after.
        let delay = std::env::var("ANGEL_CARD_DELAY").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.8);
        let script: Vec<_> = script.into_iter().map(|(at, page, e)| (at + delay, page, e)).collect();
        let start = Instant::now();
        let mut next = 0;
        let mut shot = delay;
        let mut index = 0;
        let mut current: Option<(Page, Entrance)> = None;
        let end = script.last().unwrap().0;
        loop {
            let t = start.elapsed().as_secs_f32();
            if t > end {
                break;
            }
            while next < script.len() && script[next].0 <= t {
                let (_, page, entrance) = &script[next];
                current = page.as_ref().map(|p| (Page { title: p.title, lines: p.lines.clone(), items: p.items.clone(), cursor: p.cursor, field: p.field, keys: p.keys.clone(), knight: p.knight, busy: p.busy, serial: p.serial }, *entrance));
                next += 1;
            }
            let size = host.grid_size();
            host.set_card(current.as_ref().and_then(|(p, e)| card(p, (size.cols, size.rows), *e)));
            terminal
                .draw(|f| {
                    let r = f.area();
                    let b = f.buffer_mut();
                    let ink = Style::default().fg(Color::Rgb(96, 104, 116));
                    for yy in 0..r.height {
                        let line = match yy {
                            1 => "  angelX   ~/angelX                                                   deepseek ▸ v4   think low",
                            3..=30 if yy % 3 == 0 => "  │ reading crates/glass/src/showcase.rs … the panel springs open from the corner he kicked",
                            3..=30 if yy % 3 == 1 => "  │ ran cargo check -p angel-glass: finished in 41.2 s",
                            _ => "",
                        };
                        b.set_stringn(0, yy, line, usize::from(r.width), ink);
                    }
                    b.set_stringn(2, r.height - 2, "› what should we make?", usize::from(r.width), Style::default().fg(Color::Rgb(150, 156, 166)));
                })
                .unwrap();
            if t >= shot {
                let path = out.join(format!("f{index:04}.png"));
                host.capture(&path);
                for _ in 0..2000 {
                    if std::fs::read(&path).is_ok_and(|b| b.ends_with(&[0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130])) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                index += 1;
                shot += every;
            }
            std::thread::sleep(Duration::from_millis(12));
        }
        host.set_card(None);
    });
    if let Err(error) = result {
        eprintln!("card preview: {error}");
        std::process::exit(1);
    }
}
