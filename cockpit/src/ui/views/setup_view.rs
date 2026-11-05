//! `/setup` as a card. In the window it is the glass card (`angel_glass::card`):
//! the squire on its roof, the title set large, the words coming up into
//! place. In a terminal it is the same page in a hairline box. One
//! composer lays out both; setup actions and provider jobs stay in
//! setup_menu.

use crate::app::App;
use crate::app::setup_menu::{Hit, Page};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Clear, Widget},
};
use std::time::Instant;
use unicode_width::UnicodeWidthStr;

/// The card's widest; wider windows centre it.
const MAX_COLS: u16 = 84;
/// Below this the page is plain words over the whole window, no box.
const MIN_COLS: u16 = 44;
const MIN_ROWS: u16 = 12;
/// Words sit this far in from the card's sides.
pub(crate) const INSET: u16 = 3;
/// The most choices shown at once; the rest scroll.
const MOST_ITEMS: usize = 9;

/// Tonal inks: no white, one warm accent.
pub(crate) const TEXT: Color = Color::Rgb(176, 180, 188);
pub(crate) const SOFT: Color = Color::Rgb(118, 123, 132);
pub(crate) const BRIGHT: Color = Color::Rgb(224, 226, 230);
pub(crate) const ACCENT: Color = Color::Rgb(196, 134, 92);
const READY: Color = Color::Rgb(138, 172, 124);
const EDGE: Color = Color::Rgb(62, 66, 74);
/// The chosen row in a terminal, where no glass pill glides under it.
const PICK_BG: Color = Color::Rgb(30, 33, 40);

// --- the composer -------------------------------------------------------------

/// One page laid out on a card: its cells, where the chosen row and the
/// field are, and what answers a click, all from the card's top-left.
pub(crate) struct Composed {
    pub(crate) buffer: Buffer,
    pub(crate) cursor: Option<u16>,
    pub(crate) field: Option<u16>,
    pub(crate) hits: Vec<(Rect, Hit)>,
}

/// Words wrapped to `width` columns. A leading indent stays on every line,
/// and a numbered step ("1. …") wraps under its words, not its number.
fn wrap(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let words = text.trim_start_matches(' ');
    let lead = text.len() - words.len();
    let step = words
        .split_once(". ")
        .filter(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        .map_or(0, |(n, _)| n.len() + 2);
    let indent = " ".repeat(lead);
    let hang = " ".repeat(lead + step);
    let width = width.saturating_sub(lead + step).max(1);
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in words.split(' ') {
        let wanted = line.width() + usize::from(!line.is_empty()) + word.width();
        if !line.is_empty() && wanted > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    lines.push(line);
    lines
        .into_iter()
        .enumerate()
        .map(|(i, l)| format!("{}{l}", if i == 0 { &indent } else { &hang }))
        .collect()
}

/// A row of styled runs, and what a click on it does.
type Row = (Vec<(String, Style)>, Option<RowHit>);

#[derive(Clone, Copy)]
enum RowHit {
    Item(usize),
    Keys,
}

/// Lay `page` out on a card `width` columns wide and at most `most_rows`
/// tall, its first `title_rows` left for the title. `pill` is true where a
/// glass pill marks the chosen row; a terminal shades it instead.
pub(crate) fn compose(
    page: &Page,
    width: u16,
    most_rows: u16,
    title_rows: u16,
    pill: bool,
) -> Composed {
    let room = usize::from(width.saturating_sub(2 * INSET)).max(1);
    let text = Style::default().fg(TEXT);
    let soft = Style::default().fg(SOFT);
    let mut rows: Vec<Row> = Vec::new();
    for line in page.lines.iter().flat_map(|l| wrap(l, room)) {
        rows.push((vec![(line, text)], None));
    }
    let mut field = None;
    if let Some(f) = &page.field {
        rows.push((Vec::new(), None));
        field = Some(rows.len());
        let shown = if f.shown.is_empty() {
            vec![
                ("› ".to_string(), Style::default().fg(ACCENT)),
                (f.hint.to_string(), soft),
            ]
        } else {
            // The end of a long entry stays in view, with the caret after it.
            let chars: Vec<char> = f.shown.chars().collect();
            let keep = room.saturating_sub(4);
            let tail: String = chars[chars.len().saturating_sub(keep)..].iter().collect();
            vec![
                ("› ".to_string(), Style::default().fg(ACCENT)),
                (tail, Style::default().fg(BRIGHT)),
                ("▏".to_string(), soft),
            ]
        };
        rows.push((shown, None));
    }
    if let Some(note) = &page.note {
        rows.push((Vec::new(), None));
        for line in wrap(note, room).into_iter().take(2) {
            rows.push((vec![(line, Style::default().fg(ACCENT))], None));
        }
    }
    let about: Vec<String> = page
        .items
        .get(page.cursor)
        .filter(|item| !item.about.is_empty())
        .map(|item| wrap(&item.about, room).into_iter().take(2).collect())
        .unwrap_or_default();
    // What must fit around the choices: the title, the about lines, the
    // key bar and the margins.
    let fixed =
        usize::from(title_rows) + rows.len() + 1 + about.len() + usize::from(!about.is_empty()) + 3;
    let item_rows = page
        .items
        .len()
        .min(MOST_ITEMS)
        .min(usize::from(most_rows).saturating_sub(fixed).max(3));
    let mut cursor = None;
    if !page.items.is_empty() {
        rows.push((Vec::new(), None));
        let start = if page.cursor < item_rows {
            0
        } else {
            page.cursor + 1 - item_rows
        };
        let numbered = page.field.is_none();
        for (index, item) in page.items.iter().enumerate().skip(start).take(item_rows) {
            let picked = index == page.cursor;
            if picked {
                cursor = Some(rows.len());
            }
            let number = if numbered && index < 9 {
                format!("{}  ", index + 1)
            } else {
                "   ".into()
            };
            let mut runs = vec![
                (
                    number,
                    Style::default().fg(if picked { ACCENT } else { SOFT }),
                ),
                (
                    item.label.clone(),
                    if picked {
                        Style::default().fg(BRIGHT).add_modifier(Modifier::BOLD)
                    } else {
                        text
                    },
                ),
            ];
            if item.ready {
                runs.push(("   ready".into(), Style::default().fg(READY)));
            }
            let above = index == start && start > 0;
            let below = index + 1 == start + item_rows && start + item_rows < page.items.len();
            if above || below {
                runs.push((if above { "  ↑" } else { "  ↓" }.into(), soft));
            }
            rows.push((runs, Some(RowHit::Item(index))));
        }
    }
    if !about.is_empty() {
        rows.push((Vec::new(), None));
        for line in about {
            rows.push((vec![(line, soft.add_modifier(Modifier::ITALIC))], None));
        }
    }
    rows.push((Vec::new(), None));
    let mut keys = page.keys.clone();
    if page.link && !keys.iter().any(|(key, _)| *key == "Ctrl+O") {
        keys.push(("Ctrl+O", "open in browser"));
    }
    rows.push((Vec::new(), Some(RowHit::Keys)));

    let height = (title_rows + rows.len() as u16 + 1).min(most_rows.max(title_rows + 2));
    let mut buffer = Buffer::empty(Rect::new(0, 0, width, height));
    let mut hits = Vec::new();
    let right = width.saturating_sub(INSET);
    // The key bar always shows: on a short card it takes the last row.
    let bar_y = (title_rows + rows.len() as u16 - 1).min(height.saturating_sub(2));
    for (i, (runs, hit)) in rows.iter().enumerate() {
        let y = title_rows + i as u16;
        if matches!(hit, Some(RowHit::Keys)) {
            let mut x = INSET;
            for (key, word) in &keys {
                let start = x;
                x = buffer
                    .set_stringn(
                        x,
                        bar_y,
                        key,
                        usize::from(right.saturating_sub(x)),
                        Style::default().fg(ACCENT),
                    )
                    .0;
                x = buffer
                    .set_stringn(
                        x,
                        bar_y,
                        format!(" {word}"),
                        usize::from(right.saturating_sub(x)),
                        soft,
                    )
                    .0;
                let what = match *key {
                    "Esc" => Some(Hit::Back),
                    "Ctrl+O" => Some(Hit::Open),
                    _ => None,
                };
                if let Some(what) = what {
                    hits.push((Rect::new(start, bar_y, x.saturating_sub(start), 1), what));
                }
                x = x.saturating_add(5).min(right);
            }
            continue;
        }
        if y >= bar_y {
            continue;
        }
        let shaded = !pill && Some(i) == cursor;
        if shaded {
            for x in INSET.saturating_sub(1)..right.saturating_add(1).min(width) {
                if let Some(cell) = buffer.cell_mut((x, y)) {
                    cell.set_bg(PICK_BG);
                }
            }
        }
        let mut x = INSET;
        for (words, style) in runs {
            let style = if shaded { style.bg(PICK_BG) } else { *style };
            x = buffer
                .set_stringn(x, y, words, usize::from(right.saturating_sub(x)), style)
                .0;
        }
        if let Some(RowHit::Item(index)) = hit {
            hits.push((
                Rect::new(
                    INSET.saturating_sub(1),
                    y,
                    width.saturating_sub(2 * INSET - 2),
                    1,
                ),
                Hit::Item(*index),
            ));
        }
    }
    let row = |r: Option<usize>| r.map(|r| title_rows + r as u16).filter(|r| *r < bar_y);
    Composed {
        buffer,
        cursor: row(cursor),
        field: row(field),
        hits,
    }
}

/// Where a card `width` by `height` sits in `root`: centred.
fn card_rect(root: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(root.width);
    let height = height.min(root.height);
    Rect::new(
        root.x + root.width.saturating_sub(width) / 2,
        root.y + root.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

fn card_width(root: Rect) -> u16 {
    root.width.saturating_sub(8).min(MAX_COLS)
}

/// Whether `root` is room for a card at all.
fn roomy(root: Rect) -> bool {
    root.width >= MIN_COLS + 8 && root.height >= MIN_ROWS
}

pub(crate) fn offset<T>(hits: Vec<(Rect, T)>, area: Rect) -> Vec<(Rect, T)> {
    hits.into_iter()
        .map(|(rect, hit)| {
            (
                Rect {
                    x: rect.x + area.x,
                    y: rect.y + area.y,
                    ..rect
                },
                hit,
            )
        })
        .collect()
}

/// Copy a composed card's cells onto `area` of the frame.
pub(crate) fn paint(buf: &mut Buffer, area: Rect, card: &Buffer) {
    for y in 0..area.height.min(card.area.height) {
        for x in 0..area.width.min(card.area.width) {
            let from = &card[(x, y)];
            if let Some(cell) = buf.cell_mut((area.x + x, area.y + y))
                && (from.symbol() != " " || from.bg != Color::Reset)
            {
                *cell = from.clone();
            }
        }
    }
}

/// A card in a terminal: black, a hairline round it, its title on the
/// first row inside.
pub(crate) fn paint_box(buf: &mut Buffer, area: Rect, title: &str, card: &Buffer) {
    Clear.render(area, buf);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_bg(Color::Black);
            }
        }
    }
    paint(buf, area, card);
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(EDGE))
        .render(area, buf);
    buf.set_stringn(
        area.x + INSET,
        area.y + 1,
        title,
        usize::from(area.width.saturating_sub(2 * INSET)),
        Style::default().fg(BRIGHT).add_modifier(Modifier::BOLD),
    );
}

pub(crate) fn render(frame: &mut Frame, root: Rect, app: &mut App) {
    #[cfg(feature = "glass")]
    if crate::ui::glass::active() && roomy(root) {
        render_card(root, app);
        return;
    }
    let Some(menu) = app.setup.menu.as_ref() else {
        return;
    };
    let now = Instant::now();
    let mut page = menu.page(now);
    if page.busy {
        let dots = (now.saturating_duration_since(app.started).as_secs_f32() * 2.5) as usize % 4;
        page.title.push_str(&".".repeat(dots));
    }
    let buf = frame.buffer_mut();
    if !roomy(root) {
        // Too small for a box: the page as plain words over the window.
        let composed = compose(&page, root.width, root.height, 2, false);
        Clear.render(root, buf);
        paint(buf, root, &composed.buffer);
        buf.set_stringn(
            root.x + INSET,
            root.y,
            &page.title,
            usize::from(root.width.saturating_sub(INSET)),
            Style::default().fg(BRIGHT).add_modifier(Modifier::BOLD),
        );
        app.setup.hits = offset(composed.hits, root);
        return;
    }
    let composed = compose(
        &page,
        card_width(root),
        root.height.saturating_sub(2),
        3,
        false,
    );
    let area = card_rect(root, card_width(root), composed.buffer.area.height);
    paint_box(buf, area, &page.title, &composed.buffer);
    app.setup.hits = offset(composed.hits, area);
}

/// Terminal image protocols repaint whole image tiles. Downgrade only panes
/// touching this menu (not the whole workspace) so a retained image cannot
/// repaint across the card between UI frames. The window draws its card
/// over its pictures, so it needs none of this.
pub(crate) fn register_image_occlusion(root: Rect, app: &mut App) {
    if !app.setup.open() || (crate::ui::glass::active() && roomy(root)) {
        return;
    }
    let area = match app.setup.menu.as_ref() {
        Some(menu) if roomy(root) => {
            let page = menu.page(Instant::now());
            let height = compose(
                &page,
                card_width(root),
                root.height.saturating_sub(2),
                3,
                false,
            )
            .buffer
            .area
            .height;
            card_rect(root, card_width(root), height)
        }
        _ => root,
    };
    app.image_occluders.push(area);
}

/// The glass card: the page composed with the title left to the window,
/// published with the frame. An approval outranks it.
#[cfg(feature = "glass")]
fn render_card(root: Rect, app: &mut App) {
    if app.pending_approval.is_some() {
        app.setup.hits.clear();
        return;
    }
    let Some(menu) = app.setup.menu.as_ref() else {
        app.setup.card = None;
        return;
    };
    let page = menu.page(Instant::now());
    let Some((mut card, hits)) = native_card(root, &page) else {
        return;
    };
    card.entrance = app.setup.entrance;
    app.setup.hits = hits;
    app.setup.card = Some(card.clone());
    crate::ui::glass::set_card(Some(card));
}

/// What the squire does for the guide's mood.
#[cfg(feature = "glass")]
fn knight(pose: crate::app::setup_menu::Pose) -> angel_glass::card::Knight {
    use crate::app::setup_menu::Pose;
    use angel_glass::card::Knight;
    match pose {
        Pose::Idle | Pose::Point => Knight::Sit,
        Pose::Wave => Knight::Wave,
        Pose::LiftKey => Knight::Present,
        Pose::Pull | Pose::Strain => Knight::Busy,
        Pose::Stumble => Knight::Oops,
        Pose::ThumbsUp => Knight::Cheer,
    }
}

/// A page's identity: a new one plays its words in.
#[cfg(feature = "glass")]
fn serial(page: &Page) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    page.title.hash(&mut hasher);
    page.field.is_some().hash(&mut hasher);
    page.busy.hash(&mut hasher);
    hasher.finish()
}

/// Pure card construction, so the native contract is testable without a
/// window or provider discovery.
#[cfg(feature = "glass")]
pub(crate) fn native_card(
    root: Rect,
    page: &Page,
) -> Option<(angel_glass::card::Card, Vec<(Rect, Hit)>)> {
    use angel_glass::card::TITLE_ROWS;
    let width = card_width(root);
    // Rows above are kept for the squire on the roof.
    let composed = compose(page, width, root.height.saturating_sub(6), TITLE_ROWS, true);
    let area = card_rect(root, width, composed.buffer.area.height);
    let area = Rect {
        y: area.y.max(root.y + 3.min(root.height)),
        ..area
    };
    let mut card = angel_glass::card::Card::new(area, composed.buffer)?;
    card.title = page.title.clone();
    card.serial = serial(page);
    card.cursor = composed.cursor;
    card.field = composed.field;
    card.knight = knight(page.pose);
    card.busy = page.busy;
    Some((card, offset(composed.hits, area)))
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/setup_view__tests.rs"]
mod tests;
