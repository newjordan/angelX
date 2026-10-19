//! `/setup` as a floating, physically unrolling paper surface in the window.
//! Native atlas text follows the curved paper in the final compositor pass.
//! Terminals keep the authored nine-slice parchment/guide; tiny layouts use
//! plain text. Setup actions and asynchronous provider jobs stay in setup_menu.

use crate::app::App;
use crate::app::setup_menu::{Hit, Page, Pose};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use unicode_width::UnicodeWidthStr;

/// The plate's slices, as fractions of its visible extent: the rollers end
/// and the parchment begins at `ROLL_*`; the knobs and torn edges end and the
/// writing surface begins at `EDGE_*`. Only the parts between them stretch.
const ROLL_LEFT: f32 = 0.125;
const ROLL_RIGHT: f32 = 0.884;
const EDGE_TOP: f32 = 0.25;
const EDGE_BOTTOM: f32 = 0.765;
/// The plate is drawn at this share of the height it would take unstretched,
/// so the knobs stay small and the writing surface gets the rows.
const PLATE_SCALE: f32 = 0.65;
/// The most parchment columns between the rollers; wider terminals centre it.
const MAX_MIDDLE: u16 = 96;
const MAX_ROWS: u16 = 30;
/// Below this the page is plain text without a scroll.
const MIN_COLS: u16 = 44;
const MIN_ROWS: u16 = 12;
/// Below this the existing compact layout keeps more action rows readable.
const MIN_NATIVE_SCROLL_ROWS: u16 = 20;

const PARCHMENT: [u8; 3] = [236, 216, 174];
const ROLLER: [u8; 3] = [146, 96, 46];

#[derive(Clone, Copy)]
struct Inks {
    text: Color,
    soft: Color,
    title: Color,
    ready: Color,
    pick_fg: Color,
    pick_bg: Color,
    field_bg: Color,
}

const ON_PARCHMENT: Inks = Inks {
    text: Color::Rgb(62, 40, 22),
    soft: Color::Rgb(116, 84, 50),
    title: Color::Rgb(128, 34, 24),
    ready: Color::Rgb(58, 96, 40),
    pick_fg: Color::Rgb(44, 26, 10),
    pick_bg: Color::Rgb(206, 162, 100),
    field_bg: Color::Rgb(224, 198, 148),
};

const ON_BLACK: Inks = Inks {
    text: Color::Rgb(196, 188, 172),
    soft: Color::Rgb(140, 134, 122),
    title: Color::Rgb(214, 160, 120),
    ready: Color::Rgb(140, 180, 120),
    pick_fg: Color::Rgb(24, 20, 16),
    pick_bg: Color::Rgb(180, 160, 128),
    field_bg: Color::Rgb(48, 44, 40),
};

const KEY_FG: Color = Color::Rgb(24, 24, 24);
const KEY_BG: Color = Color::Rgb(150, 150, 150);
const HINT: Color = Color::Rgb(136, 136, 136);

// --- the plate ---------------------------------------------------------------

static PLATE: OnceLock<Option<image::RgbaImage>> = OnceLock::new();

fn load_plate() -> Option<image::RgbaImage> {
    let path =
        crate::platform::runtime_paths::cockpit_dir().join(crate::app::setup_menu::SCROLL_PLATE);
    let image = image::open(path).ok()?.to_rgba8();
    // Only the clearly visible plate counts; its transparent margin does not.
    let (mut left, mut top) = image.dimensions();
    let (mut right, mut bottom) = (0, 0);
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel[3] >= 128 {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    (right > left && bottom > top).then(|| {
        image::imageops::crop_imm(&image, left, top, right - left, bottom - top).to_image()
    })
}

/// Decode the plate off the draw thread; the scroll is plain cells until then.
pub(crate) fn warm_plate() {
    if cfg!(test) || PLATE.get().is_some() {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("setup-plate".into())
        .spawn(|| {
            PLATE.get_or_init(load_plate);
        });
}

fn plate() -> Option<&'static image::RgbaImage> {
    PLATE.get().and_then(Option::as_ref)
}

/// The plate's slice sizes on a scroll `rows` tall: roller columns left and
/// right, edge pixel rows top and bottom (two pixel rows per cell).
fn slices(plate: Option<(u32, u32)>, rows: u16) -> (u16, u16, u32, u32) {
    let Some((w, h)) = plate else {
        return (3, 3, 2, 2);
    };
    let scale = f32::from(rows) * 2.0 / h as f32 * PLATE_SCALE;
    let cols = |share: f32| (share * w as f32 * scale).round().max(1.0) as u16;
    let px = |share: f32| (share * h as f32 * scale).round().max(1.0) as u32;
    (
        cols(ROLL_LEFT),
        cols(1.0 - ROLL_RIGHT),
        px(EDGE_TOP),
        px(1.0 - EDGE_BOTTOM),
    )
}

/// Map target coordinate `t` through three slices: `target` sizes onto the
/// source boundaries `source`. Returns the source span it covers.
fn span(t: u32, target: [u32; 3], source: [f32; 4]) -> (f32, f32) {
    let (mut offset, mut slice) = (t, 0);
    while slice < 2 && offset >= target[slice] {
        offset -= target[slice];
        slice += 1;
    }
    let len = target[slice].max(1) as f32;
    let step = (source[slice + 1] - source[slice]) / len;
    let start = source[slice] + offset as f32 * step;
    (start, start + step)
}

/// The mean colour of a source span, or `None` where it is mostly clear.
fn average(plate: &image::RgbaImage, x: (f32, f32), y: (f32, f32)) -> Option<[u8; 3]> {
    let (w, h) = plate.dimensions();
    let x0 = (x.0.floor().max(0.0) as u32).min(w - 1);
    let y0 = (y.0.floor().max(0.0) as u32).min(h - 1);
    let x1 = (x.1.ceil() as u32).clamp(x0 + 1, w);
    let y1 = (y.1.ceil() as u32).clamp(y0 + 1, h);
    let (mut rgb, mut alpha, mut count) = ([0u64; 3], 0u64, 0u64);
    for yy in y0..y1 {
        for xx in x0..x1 {
            let p = plate.get_pixel(xx, yy);
            let a = u64::from(p[3]);
            for (sum, channel) in rgb.iter_mut().zip(&p.0[..3]) {
                *sum += u64::from(*channel) * a;
            }
            alpha += a;
            count += 1;
        }
    }
    (alpha * 2 >= 255 * count).then(|| rgb.map(|sum| (sum / alpha.max(1)) as u8))
}

/// The scroll's pixels at one size: `cols` × `rows * 2`, row-major.
struct Painted {
    size: (u16, u16),
    pixels: Vec<Option<[u8; 3]>>,
}

static PAINTED: Mutex<Option<Arc<Painted>>> = Mutex::new(None);

fn painted(plate: &image::RgbaImage, cols: u16, rows: u16) -> Arc<Painted> {
    if let Ok(cache) = PAINTED.lock()
        && let Some(hit) = cache.as_ref().filter(|p| p.size == (cols, rows))
    {
        return Arc::clone(hit);
    }
    let (w, h) = plate.dimensions();
    let (left, right, top, bottom) = slices(Some((w, h)), rows);
    let px_w = u32::from(cols);
    let px_h = u32::from(rows) * 2;
    let target_x = [
        u32::from(left),
        px_w.saturating_sub(u32::from(left + right)),
        u32::from(right),
    ];
    let target_y = [top, px_h.saturating_sub(top + bottom), bottom];
    let source_x = [0.0, ROLL_LEFT * w as f32, ROLL_RIGHT * w as f32, w as f32];
    let source_y = [0.0, EDGE_TOP * h as f32, EDGE_BOTTOM * h as f32, h as f32];
    let mut pixels = Vec::with_capacity((px_w * px_h) as usize);
    for y in 0..px_h {
        let sy = span(y, target_y, source_y);
        for x in 0..px_w {
            pixels.push(average(plate, span(x, target_x, source_x), sy));
        }
    }
    let fresh = Arc::new(Painted {
        size: (cols, rows),
        pixels,
    });
    if let Ok(mut cache) = PAINTED.lock() {
        *cache = Some(Arc::clone(&fresh));
    }
    fresh
}

fn rgb(c: [u8; 3]) -> Color {
    Color::Rgb(c[0], c[1], c[2])
}

fn paint_plate(buf: &mut Buffer, area: Rect, painted: &Painted) {
    let cols = usize::from(area.width);
    for row in 0..area.height {
        for col in 0..area.width {
            let i = usize::from(row) * 2 * cols + usize::from(col);
            let (top, bottom) = (painted.pixels[i], painted.pixels[i + cols]);
            let Some(cell) = buf.cell_mut((area.x + col, area.y + row)) else {
                continue;
            };
            match (top, bottom) {
                (None, None) => {}
                (Some(t), Some(b)) => {
                    cell.set_char('▀').set_fg(rgb(t)).set_bg(rgb(b));
                }
                (Some(t), None) => {
                    cell.set_char('▀').set_fg(rgb(t)).set_bg(Color::Reset);
                }
                (None, Some(b)) => {
                    cell.set_char('▄').set_fg(rgb(b)).set_bg(Color::Reset);
                }
            }
        }
    }
}

/// The plain scroll: parchment between two rollers.
fn paint_plain(buf: &mut Buffer, area: Rect) {
    for row in area.top()..area.bottom() {
        for col in area.left()..area.right() {
            let roller = col < area.left() + 3 || col >= area.right().saturating_sub(3);
            let edge = row == area.top() || row + 1 == area.bottom();
            let color = if roller {
                ROLLER
            } else if edge {
                [PARCHMENT[0] - 24, PARCHMENT[1] - 26, PARCHMENT[2] - 30]
            } else {
                PARCHMENT
            };
            if let Some(cell) = buf.cell_mut((col, row)) {
                cell.set_char(' ').set_bg(rgb(color));
            }
        }
    }
}

// --- layout ------------------------------------------------------------------

/// Where things go: the scroll (if it fits), the page's text, the guide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Layout {
    pub(crate) scroll: Option<Rect>,
    pub(crate) write: Rect,
    pub(crate) guide: Option<Rect>,
    pub(crate) bar: Rect,
}

/// `cell` is a terminal cell's pixel size; the guide's frames are square.
pub(crate) fn layout(
    root: Rect,
    plate: Option<(u32, u32)>,
    guide: bool,
    cell: (u16, u16),
) -> Layout {
    let bar = Rect::new(
        root.x,
        root.bottom().saturating_sub(1),
        root.width,
        1.min(root.height),
    );
    // A row of black above the knobs and between them and the key bar.
    let avail = Rect::new(
        root.x + 1.min(root.width),
        root.y + 1.min(root.height),
        root.width.saturating_sub(2),
        root.height.saturating_sub(3),
    );
    let plain = Layout {
        scroll: None,
        write: Rect::new(
            avail.x + 1.min(avail.width),
            avail.y + 1.min(avail.height),
            avail.width.saturating_sub(2),
            avail.height.saturating_sub(1),
        ),
        guide: None,
        bar,
    };
    if avail.width < MIN_COLS || avail.height < MIN_ROWS {
        return plain;
    }
    let rows = avail.height.min(MAX_ROWS);
    let (left, right, top, bottom) = slices(plate, rows);
    let (cell_w, cell_h) = (cell.0.max(1), cell.1.max(1));
    let guide_rows = (rows / 2).max(8);
    let guide_cols = (u32::from(guide_rows) * u32::from(cell_h) / u32::from(cell_w)) as u16;
    let rollers = left + right;
    let guide_cols = if guide && avail.width >= guide_cols + 1 + rollers + 50 {
        guide_cols + 1
    } else {
        0
    };
    if avail.width < guide_cols + rollers + 24 {
        return plain;
    }
    let middle = (avail.width - guide_cols - rollers).min(MAX_MIDDLE);
    let group = guide_cols + rollers + middle;
    let x = avail.x + (avail.width - group) / 2;
    let y = avail.y + (avail.height - rows) / 2;
    let scroll = Rect::new(x + guide_cols, y, rollers + middle, rows);
    let write_top = top.div_ceil(2) as u16;
    let write_bottom = (bottom / 2) as u16;
    let write = Rect::new(
        scroll.x + left + 2,
        scroll.y + write_top,
        middle.saturating_sub(4),
        rows.saturating_sub(write_top + write_bottom),
    );
    let guide = (guide_cols > 0).then(|| {
        Rect::new(
            x,
            scroll.bottom().saturating_sub(guide_rows),
            guide_cols - 1,
            guide_rows,
        )
    });
    Layout {
        scroll: Some(scroll),
        write,
        guide,
        bar,
    }
}

// --- the page ----------------------------------------------------------------

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

fn put(buf: &mut Buffer, x: u16, y: u16, text: &str, width: u16, style: Style) -> u16 {
    if width == 0 {
        return x;
    }
    buf.set_stringn(x, y, text, usize::from(width), style).0
}

fn fill(buf: &mut Buffer, area: Rect, bg: Color) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(' ').set_bg(bg);
            }
        }
    }
}

/// Ink the page onto `area`. Items register their rows in `hits`.
fn ink(
    buf: &mut Buffer,
    area: Rect,
    page: &Page,
    inks: Inks,
    now: f32,
    hits: &mut Vec<(Rect, Hit)>,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let width = usize::from(area.width);
    let height = usize::from(area.height);
    let body: Vec<String> = page.lines.iter().flat_map(|l| wrap(l, width)).collect();
    let note: Vec<String> = page
        .note
        .as_deref()
        .map(|n| wrap(n, width).into_iter().take(2).collect())
        .unwrap_or_default();
    let about: Vec<String> = page
        .items
        .get(page.cursor)
        .filter(|item| !item.about.is_empty())
        .map(|item| wrap(&item.about, width).into_iter().take(2).collect())
        .unwrap_or_default();
    let field_rows = if page.field.is_some() { 2 } else { 0 };
    let about_rows = if about.is_empty() { 0 } else { about.len() + 1 };
    let fixed = 2 + field_rows + note.len() + about_rows;
    let body_wanted = if body.is_empty() { 0 } else { body.len() + 1 };
    let body_rows = body_wanted.min(height.saturating_sub(fixed + page.items.len().min(3)));
    let item_rows = page
        .items
        .len()
        .min(height.saturating_sub(fixed + body_rows));

    let mut y = area.y;
    let bottom = area.bottom();
    let line = |y: &mut u16| {
        let at = *y;
        *y += 1;
        (at < bottom).then_some(at)
    };

    // Title, with dots that walk while a job runs.
    let mut title = page.title.clone();
    if page.busy {
        let dots = (now * 2.5) as usize % 4;
        title.push_str(&".".repeat(dots));
    }
    if let Some(row) = line(&mut y) {
        put(
            buf,
            area.x,
            row,
            &title,
            area.width,
            Style::default().fg(inks.title).add_modifier(Modifier::BOLD),
        );
    }
    line(&mut y);
    for text in body.iter().take(body_rows.saturating_sub(1)) {
        if let Some(row) = line(&mut y) {
            put(
                buf,
                area.x,
                row,
                text,
                area.width,
                Style::default().fg(inks.text),
            );
        }
    }
    if body_rows > 0 {
        line(&mut y);
    }
    if let Some(field) = &page.field
        && let Some(row) = line(&mut y)
    {
        let rect = Rect::new(area.x, row, area.width, 1);
        fill(buf, rect, inks.field_bg);
        let x = put(
            buf,
            area.x,
            row,
            " › ",
            area.width,
            Style::default().fg(inks.soft).bg(inks.field_bg),
        );
        let room = area.right().saturating_sub(x);
        if field.shown.is_empty() {
            put(
                buf,
                x,
                row,
                field.hint,
                room,
                Style::default().fg(inks.soft).bg(inks.field_bg),
            );
        } else {
            // The end of a long entry stays in view, with the cursor after it.
            let shown: String = {
                let chars: Vec<char> = field.shown.chars().collect();
                let keep = usize::from(room.saturating_sub(2));
                chars[chars.len().saturating_sub(keep)..].iter().collect()
            };
            let end = put(
                buf,
                x,
                row,
                &shown,
                room,
                Style::default().fg(inks.text).bg(inks.field_bg),
            );
            put(
                buf,
                end,
                row,
                "▏",
                1,
                Style::default().fg(inks.text).bg(inks.field_bg),
            );
        }
        line(&mut y);
    }
    for text in &note {
        if let Some(row) = line(&mut y) {
            put(
                buf,
                area.x,
                row,
                text,
                area.width,
                Style::default().fg(inks.title).add_modifier(Modifier::BOLD),
            );
        }
    }
    let start = if page.cursor < item_rows {
        0
    } else {
        page.cursor + 1 - item_rows
    };
    let numbered = page.field.is_none();
    for (index, item) in page.items.iter().enumerate().skip(start).take(item_rows) {
        let Some(row) = line(&mut y) else { break };
        let rect = Rect::new(area.x, row, area.width, 1);
        let picked = index == page.cursor;
        let style = if picked {
            fill(buf, rect, inks.pick_bg);
            Style::default()
                .fg(inks.pick_fg)
                .bg(inks.pick_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(inks.text)
        };
        let marker = if picked { "▶ " } else { "  " };
        let number = if numbered && index < 9 {
            format!("{}  ", index + 1)
        } else {
            String::new()
        };
        let mut x = put(
            buf,
            area.x,
            row,
            &format!("{marker}{number}{}", item.label),
            area.width,
            style,
        );
        if item.ready {
            let ready_style = if picked {
                style.fg(inks.ready)
            } else {
                Style::default().fg(inks.ready)
            };
            x = put(
                buf,
                x,
                row,
                "  ✓ ready",
                area.right().saturating_sub(x),
                ready_style,
            );
        }
        let _ = x;
        let more_above = index == start && start > 0;
        let more_below = index + 1 == start + item_rows && start + item_rows < page.items.len();
        if more_above || more_below {
            let arrow = if more_above { "▲" } else { "▼" };
            put(
                buf,
                area.right().saturating_sub(1),
                row,
                arrow,
                1,
                style.fg(inks.soft),
            );
        }
        hits.push((rect, Hit::Item(index)));
    }
    if !about.is_empty() {
        line(&mut y);
        for text in &about {
            if let Some(row) = line(&mut y) {
                put(
                    buf,
                    area.x,
                    row,
                    text,
                    area.width,
                    Style::default()
                        .fg(inks.soft)
                        .add_modifier(Modifier::ITALIC),
                );
            }
        }
    }
}

/// The key bar along the bottom. Esc and Ctrl+O are clickable.
fn key_bar(buf: &mut Buffer, bar: Rect, page: &Page, hits: &mut Vec<(Rect, Hit)>) {
    let mut keys = page.keys.clone();
    if page.link && !keys.iter().any(|(key, _)| *key == "Ctrl+O") {
        keys.push(("Ctrl+O", "open in browser"));
    }
    let mut x = bar.x + 1;
    for (key, word) in keys {
        let chip = format!(" {key} ");
        let start = x;
        x = put(
            buf,
            x,
            bar.y,
            &chip,
            bar.right().saturating_sub(x),
            Style::default().fg(KEY_FG).bg(KEY_BG),
        );
        x = put(
            buf,
            x,
            bar.y,
            &format!(" {word}   "),
            bar.right().saturating_sub(x),
            Style::default().fg(HINT),
        );
        let hit = match key {
            "Esc" => Some(Hit::Back),
            "Ctrl+O" => Some(Hit::Open),
            _ => None,
        };
        if let Some(hit) = hit {
            hits.push((Rect::new(start, bar.y, x.saturating_sub(start), 1), hit));
        }
    }
}

/// The pose the guide takes next, so it can be decoded before it is due.
fn next_pose(pose: Pose) -> Pose {
    match pose {
        Pose::Wave => Pose::Point,
        Pose::Pull => Pose::Strain,
        Pose::Strain => Pose::Pull,
        other => other,
    }
}

pub(crate) fn render(frame: &mut Frame, root: Rect, app: &mut App) {
    #[cfg(feature = "glass")]
    if crate::ui::glass::active() && root.width >= MIN_COLS && root.height >= MIN_NATIVE_SCROLL_ROWS {
        render_scroll(root, app);
        return;
    }
    #[cfg(feature = "glass")]
    if crate::ui::glass::active() {
        // Tiny windows use the existing plain-text layout, never a zero-row 3D menu.
        app.setup.scroll_motion.snap(app.setup.open(), Instant::now());
        app.setup.scroll_page = None;
        crate::ui::glass::set_scroll(None);
    }
    let Some(menu) = app.setup.menu.as_ref() else {
        return;
    };
    let now = Instant::now();
    let page = menu.page(now);
    let seconds = now.saturating_duration_since(app.started).as_secs_f32();
    let guide = menu
        .guide
        .clone()
        .filter(|_| app.viewer.portraits_enabled());

    let plate = plate();
    let lay = layout(
        root,
        plate.map(image::RgbaImage::dimensions),
        guide.is_some(),
        app.viewer.portrait_cell_pixels(),
    );
    let mut hits = Vec::new();
    let buf = frame.buffer_mut();
    let inks = match lay.scroll {
        Some(scroll) => {
            fill(buf, scroll, Color::Rgb(PARCHMENT[0], PARCHMENT[1], PARCHMENT[2]));
            match plate {
                Some(plate) => {
                    paint_plate(buf, scroll, &painted(plate, scroll.width, scroll.height))
                }
                None => paint_plain(buf, scroll),
            }
            ON_PARCHMENT
        }
        None => {
            fill(buf, lay.write, Color::Black);
            ON_BLACK
        },
    };
    ink(buf, lay.write, &page, inks, seconds, &mut hits);
    fill(buf, lay.bar, Color::Black);
    key_bar(buf, lay.bar, &page, &mut hits);
    app.setup.hits = hits;
    if let (Some(path), Some(area)) = (guide, lay.guide) {
        app.viewer.set_portrait_mono(false);
        app.viewer.render_helm(frame, area, &path, page.pose as u8);
        let next = next_pose(page.pose);
        if next != page.pose {
            app.viewer.prefetch_helm(area, &path, next as u8);
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/setup_view__tests.rs"]
mod tests;

/// The window keeps the workspace in its normal grid and presents this separate
/// native-text page in the final compositor pass. Terminal rendering stays above.
#[cfg(feature = "glass")]
fn render_scroll(root: Rect, app: &mut App) {
    use angel_glass::scroll::Page as ScrollPage;
    let now = Instant::now();
    app.setup.scroll_motion.advance(now);
    if app.visual_motion != crate::ui::viz::lifecycle_viz::MotionMode::Full {
        app.setup.scroll_motion.snap(app.setup.open(), now);
    }
    let pose = app.setup.scroll_motion.pose();
    if app.pending_approval.is_some() {
        if let Some(page) = app.setup.scroll_page.as_mut() {
            page.interactive = false;
        }
        app.setup.hits.clear();
        crate::ui::glass::set_scroll(None);
        return;
    }
    if let Some(menu) = app.setup.menu.as_ref() {
        let (area, buf, hits) = native_content(root, &menu.page(now));
        app.setup.hits = hits;
        app.setup.scroll_page = ScrollPage::new(area, buf, pose);
    } else if app.setup.scroll_motion.hidden() {
        app.setup.scroll_page = None;
    }
    let page = app.setup.scroll_page.as_mut().map(|page| {
        page.pose = pose;
        page.interactive = app.setup.menu.is_some();
        page.clone()
    });
    crate::ui::glass::set_scroll(page);
}

/// Terminal image protocols repaint whole image tiles. Downgrade only panes
/// touching this menu (not the whole workspace) so a retained image cannot
/// repaint across the parchment between UI frames. Native glass has its own pass.
pub(crate) fn register_image_occlusion(root: Rect, app: &mut App) {
    if !app.setup.open() || (crate::ui::glass::active() && root.width >= MIN_COLS && root.height >= MIN_NATIVE_SCROLL_ROWS) {
        return;
    }
    let layout = layout(root, plate().map(image::RgbaImage::dimensions),
        app.viewer.portraits_enabled(), app.viewer.portrait_cell_pixels());
    app.image_occluders.push(layout.scroll.unwrap_or(layout.write));
    app.image_occluders.push(layout.bar);
    if let Some(guide) = layout.guide { app.image_occluders.push(guide); }
}

/// Pure page construction lets native and terminal menu contracts be tested
/// without opening a real window or invoking provider discovery.
#[cfg(feature = "glass")]
fn native_content(root: Rect, page: &Page) -> (Rect, Buffer, Vec<(Rect, Hit)>) {
    let width = root.width.saturating_sub(8).min(104).max(1);
    let height = root.height.saturating_sub(4).min(36).max(1);
    let area = Rect::new(root.x + root.width.saturating_sub(width) / 2,
        root.y + root.height.saturating_sub(height) / 2, width, height);
    let mut buf = Buffer::empty(Rect::new(0,0,width,height));
    // The key bar sits on the flat paper, two rows clear of where it curls
    // under the lower roller (at `height - 4` it lay under the roller).
    let bar_row = angel_glass::scroll::readable_rows(height).saturating_sub(2).min(height.saturating_sub(4));
    let write = Rect::new(4.min(width),3.min(height),width.saturating_sub(8),bar_row.saturating_sub(3));
    let bar = Rect::new(4.min(width),bar_row,width.saturating_sub(8),1);
    let mut hits = Vec::new();
    ink(&mut buf,write,page,ON_PARCHMENT,0.0,&mut hits);
    key_bar(&mut buf,bar,page,&mut hits);
    for (rect,_) in &mut hits {rect.x += area.x; rect.y += area.y;}
    (area,buf,hits)
}
