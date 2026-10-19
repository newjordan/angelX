//! What a knight reads over the town: the clock bar, the candles, the
//! Ledger when it is open, the bell's menu, a villager's words, and the
//! cards that mark the dawn and the end.
use super::super::super::ink::{Img, text_width};
use super::ground::{plate, plate_centred, seat_ink};
use super::labels::{Layout, Rect};
use super::{VIEW_H, VIEW_Y};
use crate::rules::moonfall::{
    Session, clock, control,
    folk::{CAST, Folk},
    ledger::{self, Fact},
    state::{Item, Knight, Mask, Note, Panel, Phase, Ring},
    story::LIGHT_HOLD,
};

/// A tick mark.
fn check(im: &mut Img, x: i32, y: i32, ink: char) {
    for (dx, dy) in [(0, 3), (1, 4), (2, 3), (3, 2), (4, 1), (5, 0)] {
        im.put(x + dx, y + dy, ink);
        im.put(x + dx, y + dy + 1, ink);
    }
}

/// `s` broken into lines of at most `n` characters, at spaces.
pub(super) fn wrap(s: &str, n: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in s.split_whitespace() {
        match out.last_mut() {
            Some(line) if line.len() + 1 + word.len() <= n => {
                line.push(' ');
                line.push_str(word);
            }
            _ => out.push(word.to_string()),
        }
    }
    out
}

/// `s` cut to at most `n` characters: whole words, then `..` where words
/// were left out.
pub(super) fn fit(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let head: String = s.chars().take(n - 2).collect();
    let whole = if s.chars().nth(n - 2) == Some(' ') { head.as_str() } else { head.rsplit_once(' ').map_or(head.as_str(), |(a, _)| a) };
    format!("{}..", whole.trim_end_matches([' ', ',', ':', '-', ';']))
}

pub(super) fn say_note(n: Note) -> String {
    crate::rules::moonfall::story::note_text(n)
}

/// The clock bar and the lines around it, at the foot of the screen.
pub(super) fn bar(im: &mut Img, s: &Session, me: &Knight, me_id: u32, shown: u32, tick: u32) {
    let y0 = VIEW_Y + VIEW_H;
    im.rect(0, y0, im.w, im.h - y0, 'k');
    im.rect(0, y0, im.w, 1, 'g');
    seat_chips(im, s, me_id, y0);
    let day = s.nums.day;
    let final_now = shown >= clock::final_start(day) && s.completed.is_none();
    let head = head_text(s, shown);
    im.text(8, y0 + HEAD_ROW, &head, if final_now { '7' } else { 'T' });
    let loops = loops_text(s);
    im.text(8 + text_width(&head) + 12, y0 + HEAD_ROW, &loops, 'h');
    // Candles, right: short, so the clock's marker above the track clears them.
    for c in 0..3u8 {
        let x = 340 + i32::from(c) * 11;
        im.rect(x, y0 + 3, 4, 6, if c < me.candles { 'T' } else { 'g' });
        im.rect(x + 1, y0 + 1, 2, 2, if c < me.candles { '6' } else { 'j' });
    }
    // Mask and what is in hand.
    let mask = mask_text(me);
    if !mask.is_empty() {
        im.text(300 - text_width(mask), y0 + HEAD_ROW, mask, if me.mask == Mask::Swift { 'z' } else { 'R' });
    }
    // The bar itself: three days, nights shaded, the Final Hours red.
    let (bx, bw, by) = (8, 368, y0 + 12);
    im.rect(bx, by, bw, 9, 'g');
    let end = clock::end(day).max(1);
    for px in 0..bw {
        let t = (i64::from(px) * i64::from(end) / i64::from(bw)) as u32;
        let night = clock::night(t, day);
        let fin = t >= clock::final_start(day);
        let ink = if fin {
            '7'
        } else if night {
            's'
        } else {
            'U'
        };
        im.rect(bx + px, by + 1, 1, 7, ink);
    }
    im.frame(bx - 1, by - 1, bw + 2, 11, 'j');
    for d in 1..3 {
        im.rect(bx + bw * d / 3, by - 2, 1, 13, 'H');
    }
    for q in 1..12 {
        if q % 4 != 0 {
            im.rect(bx + bw * q / 12, by + 7, 1, 2, 'h');
        }
    }
    for (d, n) in ["1", "2", "3"].iter().enumerate() {
        im.text(bx + bw * d as i32 / 3 + 3, by + 1, n, 'k');
    }
    im.text(bx + bw - 30, by + 1, "FINAL", 'T');
    let mx = bx + (i64::from(shown.min(end)) * i64::from(bw) / i64::from(end)) as i32;
    im.rect(mx, by - 3, 1, 15, 'w');
    // The marker: the sun by day, the moon by night.
    if clock::night(shown, day) {
        im.ellipse(mx as f32, by as f32 + 4.5, 4.0, 4.0, 'V');
        im.ellipse(mx as f32 + 2.0, by as f32 + 3.5, 3.0, 3.0, 'k');
    } else {
        im.ellipse(mx as f32, by as f32 + 4.5, 4.0, 4.0, '6');
        im.ellipse(mx as f32, by as f32 + 4.5, 2.0, 2.0, 'w');
    }
    // An hour just struck: the marker rings.
    let hour = (day / 24).max(1);
    if shown % hour < 6 && matches!(s.phase, Phase::Play) {
        im.frame(mx - 6, by - 2, 13, 13, '6');
    }
    // The foot line.
    im.text(8, y0 + FOOT_ROW, &foot_line(s, me, tick), if me.note.is_some() { '6' } else { 'h' });
}

/// The clock bar's top row of words sits above the clock's marker, which
/// stands from the row after it to the foot line.
const HEAD_ROW: i32 = 2;
const FOOT_ROW: i32 = 24;

fn head_text(s: &Session, shown: u32) -> String {
    let st = clock::stamp(shown, s.nums.day);
    format!("DAY {}  {}", st.day, st.text())
}

fn loops_text(s: &Session) -> String {
    format!("LOOP {}", s.memory.loops + 1)
}

fn mask_text(me: &Knight) -> &'static str {
    match me.mask {
        Mask::Bare => "",
        Mask::Swift => "SWIFT",
        Mask::Stout => "STOUT",
    }
}

/// The foot line: a note, what is in hand, or the keys; cut to fit.
fn foot_line(s: &Session, me: &Knight, tick: u32) -> String {
    let line = if let Some((n, _)) = me.note {
        say_note(n)
    } else if me.held != Item::Empty {
        format!("HOLDING {}  X PUT DOWN", me.held.name())
    } else if s.memory.written() > me.read && tick / 10 % 2 == 0 {
        "E TALK  Q LEDGER *NEW*  R BELL  Z MASK  SPACE DODGE".to_string()
    } else {
        "E TALK  Q LEDGER  R BELL  Z MASK  SPACE DODGE".to_string()
    };
    fit(&line, 62)
}

/// Each place in the clock bar, by name: the day, the loop, the mask, the
/// candles, the seat chips, the clock's track with its marker, the foot line.
/// None of them touches another.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn bar_rows(s: &Session, me: &Knight, shown: u32, tick: u32) -> Vec<(&'static str, Rect)> {
    let y0 = VIEW_Y + VIEW_H;
    let head = head_text(s, shown);
    let mut rows = vec![
        ("day", Rect::new(8, y0 + HEAD_ROW, text_width(&head), 7)),
        ("loop", Rect::new(8 + text_width(&head) + 12, y0 + HEAD_ROW, text_width(&loops_text(s)), 7)),
        ("candles", Rect::new(340, y0 + 1, 26, 8)),
        ("track", Rect::new(0, y0 + 9, 384, 15)),
        ("foot", Rect::new(8, y0 + FOOT_ROW, text_width(&foot_line(s, me, tick)), 7)),
    ];
    let mask = mask_text(me);
    if !mask.is_empty() {
        rows.push(("mask", Rect::new(300 - text_width(mask), y0 + HEAD_ROW, text_width(mask), 7)));
    }
    if let (Some(first), Some(last)) = (chip_rects(s.seats.len()).first().copied(), chip_rects(s.seats.len()).last().copied()) {
        rows.push(("seats", Rect::new(first.x, first.y, last.right() - first.x, first.h)));
    }
    rows
}

/// The lines the whole table reads, top to bottom: the bell's call first (the
/// one a knight must answer), then the Ledger's writing and the reminders;
/// four at most, each cut to the width of the picture.
pub(super) fn news_lines(s: &Session, me: &Knight) -> Vec<(String, char)> {
    let mut lines: Vec<(String, char)> = Vec::new();
    if let Some(c) = s.call {
        let who = c.by;
        let need = s.bell_need();
        let what = match c.ring {
            Ring::Rewind => "REWIND TO DAWN",
            Ring::Dusk => "WAIT FOR DUSK",
            Ring::Dawn => "WAIT FOR DAWN",
        };
        let mine = c.ayes & (1 << (me_id_bit(s, me))) != 0;
        lines.push((
            format!(
                "KNIGHT {who} RINGS FOR {what} {}/{need}{}",
                c.ayes.count_ones(),
                if mine { "" } else { "  R JOINS" }
            ),
            '6',
        ));
    }
    lines.extend(s.news.iter().map(|&(n, _)| (say_note(n), 'T')));
    lines.into_iter().take(4).map(|(line, ink)| (fit(&line, 62), ink)).collect()
}

/// The first row of the news: under the sky's edge, and under the Final Hours
/// box when it is up.
fn news_top(s: &Session) -> i32 {
    VIEW_Y + 4 + if s.final_hours() && s.completed.is_none() { 36 } else { 0 }
}

/// Where each news line's plate stands.
fn news_rects(s: &Session, me: &Knight) -> Vec<Rect> {
    let top = news_top(s);
    news_lines(s, me)
        .iter()
        .enumerate()
        .map(|(n, (line, _))| {
            let w = text_width(line);
            Rect::new(192 - w / 2 - 1, top + 10 * n as i32 - 1, w + 2, 9)
        })
        .collect()
}

/// Words the whole table reads: the Ledger writing, reminders, the bell.
pub(super) fn news(im: &mut Img, s: &Session, me: &Knight) {
    let mut y = news_top(s);
    for (line, ink) in news_lines(s, me) {
        plate(im, 192 - text_width(&line) / 2, y, &line, ink);
        y += 10;
    }
}

fn me_id_bit(s: &Session, me: &Knight) -> u32 {
    s.seats
        .iter()
        .find(|(_, st)| &st.knight == me)
        .map_or(0, |(&id, _)| id.saturating_sub(1))
}

/// The Final Hours box: its words, the minutes left, and where it stands.
fn final_box(s: &Session, shown: u32) -> Option<(String, Rect)> {
    let day = s.nums.day;
    if shown < clock::final_start(day) || s.completed.is_some() || !matches!(s.phase, Phase::Play) {
        return None;
    }
    let left = clock::remaining_minutes(shown, day);
    let count = format!("{}:{:02} UNTIL THE MOON FALLS", left / 60, left % 60);
    let pw = (text_width("FINAL HOURS") * 2).max(text_width(&count)) + 12;
    Some((count, Rect::new(192 - pw / 2, VIEW_Y + 3, pw, 30)))
}

/// FINAL HOURS and the minutes left.
pub(super) fn final_hours(im: &mut Img, s: &Session, shown: u32, tick: u32) {
    let Some((count, box_at)) = final_box(s, shown) else { return };
    let text = "FINAL HOURS";
    let pulse = tick / 15 % 2 == 0;
    let w = text_width(text) * 2;
    let x = 192 - w / 2;
    let y = VIEW_Y + 6;
    let mut words = Img::new(text_width(text), 7);
    words.text(0, 0, text, if pulse { '7' } else { 'R' });
    im.rect(box_at.x, box_at.y, box_at.w, box_at.h, 'k');
    im.frame(box_at.x, box_at.y, box_at.w, box_at.h, '7');
    im.stamp_times(&words, x, y, 2);
    im.text(192 - text_width(&count) / 2, y + 17, &count, 'T');
}

/// Where the dawn card stands, while it shows.
fn card_rect(s: &Session) -> Option<Rect> {
    if s.card == 0 || !matches!(s.phase, Phase::Play) {
        return None;
    }
    let total = s.nums.rewind.saturating_sub(control(&s.nums)) + 24;
    let total = total.max(s.card);
    let age = total - s.card;
    // It fades in over the flash, holds, and goes.
    let shown_ok = age >= 2 && s.card >= 6;
    let w = text_width("DAWN OF THE FIRST DAY") * 2;
    shown_ok.then(|| Rect::new(192 - w / 2 - 10, 62, w + 20, 40))
}

/// Dawn card.
pub(super) fn card(im: &mut Img, s: &Session) {
    if card_rect(s).is_none() {
        return;
    }
    let text = "DAWN OF THE FIRST DAY";
    let mut words = Img::new(text_width(text), 7);
    words.text(0, 0, text, 'T');
    let w = words.w * 2;
    let (x, y) = (192 - w / 2, 70);
    im.rect(x - 10, y - 8, w + 20, 40, 'k');
    im.frame(x - 10, y - 8, w + 20, 40, 'O');
    im.frame(x - 8, y - 6, w + 16, 36, 'B');
    im.stamp_times(&words, x, y, 2);
    let sub = if s.memory.loops == 0 { "THE MOON FALLS IN THREE DAYS".to_string() } else { format!("LOOP {}  THE LEDGER REMEMBERS", s.memory.loops + 1) };
    im.text(192 - text_width(&sub) / 2, y + 20, &sub, 'v');
}

fn panel(im: &mut Img, x: i32, y: i32, w: i32, h: i32) {
    im.rect(x, y, w, h, 'T');
    im.rect(x + 2, y + 2, w - 4, h - 4, '9');
    im.frame(x, y, w, h, 'B');
    im.frame(x + 1, y + 1, w - 2, h - 2, 'O');
}

/// The Ledger, open at a page: the folk down the side, their page at right.
pub(super) fn ledger_panel(im: &mut Img, s: &Session, page: u8, shown: u32, tick: u32) {
    let (px, py, pw, ph) = (14, VIEW_Y + 2, 356, VIEW_H - 4);
    panel(im, px, py, pw, ph);
    im.rect(px + 100, py + 6, 1, ph - 12, 'O');
    // Tabs.
    let names: [(&str, Option<Folk>); 7] = [
        ("TOWN", None),
        ("MIRA", Some(Folk::Mira)),
        ("TOVIN", Some(Folk::Tovin)),
        ("NESSA", Some(Folk::Nessa)),
        ("ORIN", Some(Folk::Orin)),
        ("PELLA", Some(Folk::Pella)),
        ("ASTER", Some(Folk::Aster)),
    ];
    for (i, (name, folk)) in names.iter().enumerate() {
        let y = py + 8 + i as i32 * 16;
        let on = usize::from(page) == i;
        if on {
            im.rect(px + 5, y - 2, 92, 13, 'O');
            im.frame(px + 5, y - 2, 92, 13, 'B');
        }
        im.text(px + 12, y + 1, name, if on { 'b' } else { 'P' });
        if let Some(f) = folk {
            if s.memory.ticked(*f) {
                check(im, px + 82, y, 'l');
            } else {
                im.frame(px + 82, y, 7, 7, 'P');
            }
        }
    }
    im.text(px + 8, py + ph - 24, "A D  PAGES", 'P');
    im.text(px + 8, py + ph - 14, "Q  CLOSE", 'P');
    let (cx, mut cy) = (px + 110, py + 8);
    match page {
        0 => town_page(im, s, cx, &mut cy, shown),
        n => {
            let folk = CAST[usize::from(n) - 1];
            person_page(im, s, folk, cx, &mut cy, shown, tick);
        }
    }
}

fn line(im: &mut Img, x: i32, y: &mut i32, s: &str, ink: char) {
    im.text(x, *y, s, ink);
    *y += 10;
}

fn town_page(im: &mut Img, s: &Session, cx: i32, cy: &mut i32, shown: u32) {
    let st = clock::stamp(shown, s.nums.day);
    let mut t = Img::new(text_width("THE LEDGER"), 7);
    t.text(0, 0, "THE LEDGER", 'b');
    im.stamp_times(&t, cx, *cy, 2);
    *cy += 18;
    line(im, cx, cy, &format!("DAY {}  {}   LOOP {}", st.day, st.text(), s.memory.loops + 1), 'I');
    *cy += 4;
    line(im, cx, cy, "THREE LIGHTS AT THE THIRD MIDNIGHT:", 'b');
    let status = |ok: bool, done: &str, todo: &str| if ok { done.to_string() } else { todo.to_string() };
    let towers = [
        ("NORTH TOWER", s.memory.has_keep(ledger::NORTH_KEY), "KEY KEPT", "NEEDS TOVIN'S KEY"),
        ("SOUTH TOWER", s.memory.has_keep(ledger::SOUTH_SEAL), "SEAL KEPT", "NEEDS MIRA'S SEAL"),
        ("THE MOONWELL", s.memory.has_keep(ledger::OIL), "OIL KEPT", "NEEDS ASTER'S OIL"),
    ];
    for (name, ok, yes, no) in towers {
        let text = format!("{name}: {}", status(ok, yes, no));
        im.text(cx + 8, *cy, &text, if ok { 'l' } else { 'p' });
        if ok {
            check(im, cx, *cy, 'l');
        }
        *cy += 10;
    }
    *cy += 4;
    let ticks = s.memory.ticks.count_ones();
    line(im, cx, cy, &format!("FAVOURS TICKED: {ticks} OF 6"), 'I');
    let masks = format!(
        "MASKS: {}{}",
        if s.memory.has_mask(ledger::SWIFT) { "SWIFT " } else { "" },
        if s.memory.has_mask(ledger::STOUT) { "STOUT" } else { "" }
    );
    line(im, cx, cy, if masks.len() > 7 { &masks } else { "MASKS: NONE YET" }, 'I');
    *cy += 4;
    if s.memory.knows(Fact::Verse) {
        for l in wrap(Fact::Verse.text(), 36) {
            line(im, cx, cy, &l, 'p');
        }
    } else {
        for l in wrap("THE VERSE OF THE THREE LIGHTS IS NOT YET HEARD.", 36) {
            line(im, cx, cy, &l, 'P');
        }
    }
}

fn person_page(im: &mut Img, s: &Session, folk: Folk, cx: i32, cy: &mut i32, shown: u32, tick: u32) {
    let page = ledger::page(&s.memory, folk, shown, s.nums.day);
    // Name, large; the portrait block at the right edge.
    let mut t = Img::new(text_width(folk.name()), 7);
    t.text(0, 0, folk.name(), 'b');
    im.stamp_times(&t, cx, *cy, 1);
    portrait(im, 322, *cy - 2, folk, tick, 30);
    *cy += 10;
    im.text(cx, *cy, folk.title(), 'P');
    *cy += 12;
    // The tick.
    im.text(cx, *cy, "FAVOUR", 'I');
    if page.ticked {
        check(im, cx + 44, *cy - 1, 'l');
        im.text(cx + 54, *cy, "DONE", 'l');
    } else {
        im.frame(cx + 44, *cy, 7, 7, 'P');
    }
    *cy += 12;
    if page.asks.is_empty() && page.routine.is_empty() && page.notes.is_empty() {
        for l in wrap("NOTHING WRITTEN YET. GO AND SEE WHAT THEY DO, AND ASK.", 40) {
            line(im, cx, cy, &l, 'P');
        }
        return;
    }
    for a in &page.asks {
        for (i, l) in wrap(a, 38).into_iter().enumerate() {
            im.text(cx + if i == 0 { 0 } else { 6 }, *cy, &if i == 0 { format!("> {l}") } else { l }, 'p');
            *cy += 10;
        }
    }
    for a in &page.notes {
        for (i, l) in wrap(a, 38).into_iter().enumerate() {
            im.text(cx + if i == 0 { 0 } else { 6 }, *cy, &if i == 0 { format!("* {l}") } else { l }, 'I');
            *cy += 10;
        }
    }
    *cy += 3;
    // The routine fills what room is left, from the line the clock stands in,
    // so the page reads what is happening and what comes next.
    let room = ((VIEW_Y + 2 + VIEW_H - 4 - 10 - *cy) / 10).max(1) as usize;
    let now = page.routine.iter().position(|l| l.now).unwrap_or(0);
    let from = now.min(page.routine.len().saturating_sub(room));
    for l in page.routine.iter().skip(from).take(room) {
        let ink = if l.now { 'p' } else { 'I' };
        if l.now {
            im.rect(cx - 2, *cy - 1, 232, 9, 'O');
        }
        im.text(cx, *cy, &l.time, ink);
        im.text(cx + 34, *cy, &l.text.chars().take(32).collect::<String>(), ink);
        *cy += 10;
    }
    if page.routine.len() > room {
        let more = format!("{} OF {} LINES", room.min(page.routine.len()), page.routine.len());
        im.text(px_right_edge() - text_width(&more) - 4, VIEW_Y + 2 + VIEW_H - 4 - 11, &more, 'P');
    }
}

fn px_right_edge() -> i32 {
    14 + 356
}

/// A villager's little portrait: a labelled block.
pub(super) fn portrait(im: &mut Img, x: i32, y: i32, folk: Folk, tick: u32, size: i32) {
    im.rect(x, y, size, size, 'x');
    im.frame(x, y, size, size, 'k');
    let ink = folk.ink();
    im.rect(x + size / 2 - 6, y + 12, 12, size - 14, ink);
    im.rect(x + size / 2 - 5, y + 4, 10, 10, 'c');
    im.frame(x + size / 2 - 5, y + 4, 10, 10, 'k');
    let blink = tick / 40 % 5 == 0;
    if !blink {
        im.put(x + size / 2 - 3, y + 8, 'k');
        im.put(x + size / 2 + 2, y + 8, 'k');
    }
    im.rect(x + size / 2 - 2, y + 11, 4, 1, 'k');
}

/// The bell's menu.
pub(super) fn bell_menu(im: &mut Img, s: &Session, line_at: u8, tick: u32) {
    let (x, y, w, h) = (96, VIEW_Y + 28, 192, 92);
    panel(im, x, y, w, h);
    let title = "THE MOONWELL BELL";
    im.text(192 - text_width(title) / 2, y + 7, title, 'b');
    let rows = [
        (Ring::Rewind, "REWIND TO FIRST DAWN"),
        (Ring::Dusk, "WAIT FOR DUSK"),
        (Ring::Dawn, "WAIT FOR DAWN"),
    ];
    for (i, (ring, text)) in rows.iter().enumerate() {
        let ry = y + 22 + i as i32 * 14;
        let ok = s.ring_ok(*ring);
        let on = usize::from(line_at) == i;
        if on {
            im.rect(x + 8, ry - 3, w - 16, 12, 'O');
            im.put(x + 12, ry, 'b');
            im.put(x + 13, ry + 1, 'b');
            im.put(x + 12, ry + 2, 'b');
        }
        im.text(x + 20, ry, text, if !ok { 'P' } else if on { 'b' } else { 'I' });
    }
    let help = if s.seats.len() > 1 { "THE TABLE MUST AGREE" } else { "W S CHOOSE   E RING" };
    im.text(192 - text_width(help) / 2, y + h - 15, help, 'P');
    let _ = tick;
}

/// The box a villager speaks to the viewer in.
const TALK_BOX: Rect = Rect::new(10, VIEW_Y + VIEW_H - 56, 364, 52);

/// The Ledger's open pages, and the bell's menu.
const LEDGER_PANEL: Rect = Rect::new(14, VIEW_Y + 2, 356, VIEW_H - 4);
const BELL_PANEL: Rect = Rect::new(96, VIEW_Y + 28, 192, 92);

/// A villager speaking to the viewer.
pub(super) fn talk_box(im: &mut Img, s: &Session, me: &Knight, tick: u32) {
    let Some(say) = me.say else { return };
    let (x, y, w, h) = (10, VIEW_Y + VIEW_H - 56, 364, 52);
    panel(im, x, y, w, h);
    portrait(im, x + 6, y + 6, say.folk, tick, 40);
    im.text(x + 52, y + 6, say.folk.name(), 'p');
    let lines = s.words(say.folk, say.topic);
    let from = usize::from(say.page) * 2;
    for (i, l) in lines.iter().skip(from).take(2).enumerate() {
        im.text(x + 52, y + 20 + i as i32 * 11, l, 'b');
    }
    let more = from + 2 < lines.len();
    if tick / 12 % 2 == 0 {
        im.text(x + w - 22, y + h - 11, if more { "E >" } else { "E" }, 'p');
    }
}

/// The hold bar and its word, for the room words keep clear.
fn hold_rect() -> Rect {
    Rect::new(167, VIEW_Y + VIEW_H / 2 + 11, 50, 18)
}

/// A knight who holds E at a flame, and the bar filling.
pub(super) fn hold_bar(im: &mut Img, me: &Knight) {
    if me.hold == 0 {
        return;
    }
    let (x, y) = (192 - 24, VIEW_Y + VIEW_H / 2 + 22);
    im.rect(x - 1, y - 1, 50, 8, 'k');
    im.rect(x, y, (me.hold * 48 / LIGHT_HOLD) as i32, 6, '5');
    plate_centred(im, 192, y - 10, "LIGHTING", '6');
}

/// The card of a knight snuffed out.
const OUT_CARD: Rect = Rect::new(60, VIEW_Y + 50, 264, 36);

/// A knight who lies snuffed out.
pub(super) fn out_card(im: &mut Img, me: &Knight) {
    if me.down == 0 {
        return;
    }
    im.rect(60, VIEW_Y + 50, 264, 36, 'k');
    im.frame(60, VIEW_Y + 50, 264, 36, 'h');
    plate_centred(im, 192, VIEW_Y + 58, "SNUFFED OUT", 'T');
    plate_centred(im, 192, VIEW_Y + 70, "THE MOONWELL CARRIES YOU BACK", 'h');
}

/// Where the end card stands, while it shows.
fn end_rect(s: &Session) -> Option<Rect> {
    let free = s.completed.is_some() && matches!(s.phase, Phase::Play);
    let winning = matches!(s.phase, Phase::Won { .. });
    if !free && !winning {
        return None;
    }
    let w = text_width(if winning { "THE LIGHTS HOLD" } else { "THE MOON TURNS BACK" }) * 2;
    Some(Rect::new(192 - w / 2 - 10, VIEW_Y + 18, w + 20, if free { 56 } else { 30 }))
}

/// The end card.
pub(super) fn end_card(im: &mut Img, s: &Session) {
    let free = s.completed.is_some() && matches!(s.phase, Phase::Play);
    let winning = matches!(s.phase, Phase::Won { .. });
    if !free && !winning {
        return;
    }
    let text = if winning { "THE LIGHTS HOLD" } else { "THE MOON TURNS BACK" };
    let mut words = Img::new(text_width(text), 7);
    words.text(0, 0, text, '6');
    let w = words.w * 2;
    let (x, y) = (192 - w / 2, VIEW_Y + 26);
    im.rect(x - 10, y - 8, w + 20, if free { 56 } else { 30 }, 'k');
    im.frame(x - 10, y - 8, w + 20, if free { 56 } else { 30 }, 'O');
    im.stamp_times(&words, x, y, 2);
    if free {
        im.text(192 - text_width("MOONFALL IS STOPPED") / 2, y + 20, "MOONFALL IS STOPPED", 'T');
        let sub = format!("{} LOOPS TOLD IN THE LEDGER", s.memory.loops + 1);
        im.text(192 - text_width(&sub) / 2, y + 31, &sub, 'v');
        if (s.elapsed / 15) % 2 == 0 {
            im.text(192 - text_width("E  RETURN TO THE KINGDOM") / 2, y + 42, "E  RETURN TO THE KINGDOM", 'Y');
        }
    }
}

/// Who is at the table: one chip for each seat in the clock bar's top row,
/// between the day's words and the mask's, the viewer's framed bright.
fn seat_chips(im: &mut Img, s: &Session, me_id: u32, y0: i32) {
    for (n, (&id, seat)) in s.seats.iter().enumerate() {
        let x = chip_rects(s.seats.len())[n].x;
        let ink = seat_ink(id);
        im.rect(x, y0, 16, 9, 'k');
        im.frame(x, y0, 16, 9, if id == me_id { 'H' } else { 'j' });
        im.text(x + 2, y0 + 1, &format!("{id}"), ink);
        for c in 0..seat.knight.candles {
            im.put(x + 8 + i32::from(c) * 2, y0 + 4, '6');
        }
    }
}

/// The right edge of the seat chips: the mask's words start at 271.
const CHIPS_END: i32 = 266;

/// Where the chips of `n` seats stand, left to right: in the bar's top row,
/// 20 pixels apart.
pub(super) fn chip_rects(n: usize) -> Vec<Rect> {
    (0..n).map(|i| Rect::new(CHIPS_END - 16 - (n - 1 - i) as i32 * 20, VIEW_Y + VIEW_H, 16, 9)).collect()
}

/// Where the start-of-run hint stands, while it shows.
fn hint_rect(s: &Session) -> Option<Rect> {
    if s.memory.loops > 0 || s.clock > 150 || !matches!(s.phase, Phase::Play) || s.card > 0 {
        return None;
    }
    let w = text_width("TALK. WRITE. REWIND.");
    Some(Rect::new(192 - w / 2 - 1, VIEW_Y + VIEW_H - 15, w + 2, 9))
}

/// A short start-of-run hint: what the keys are.
pub(super) fn first_hint(im: &mut Img, s: &Session) {
    if hint_rect(s).is_some() {
        plate_centred(im, 192, VIEW_Y + VIEW_H - 14, "TALK. WRITE. REWIND.", 'T');
    }
}

/// A word over the town while the bell is working: it winds the town back, or
/// time runs on to the hour asked for. A banner across a free row of the town
/// view, clear of the bell's picture and of every plate.
pub(super) fn bell_banner(labels: &mut Layout, s: &Session) {
    let text = match s.phase {
        Phase::Rewind { fell: true, .. } => "THE MOON FALLS",
        Phase::Rewind { .. } => "THE BELL TOLLS: THE TOWN WINDS BACK",
        Phase::Skip { from, to, .. } => {
            if clock::night(to, s.nums.day) && !clock::night(from, s.nums.day) {
                "THE BELL TOLLS: TIME RUNS ON TO DUSK"
            } else {
                "THE BELL TOLLS: TIME RUNS ON TO DAWN"
            }
        }
        _ => return,
    };
    labels.banner(text, '6', 'O');
}

/// Everything the HUD draws over the town that words keep out of: the news
/// lines, the Final Hours box, the hold bar and the start hint; and what
/// hides them, which words are left out under: the dawn card, the end card,
/// the snuffed card, a conversation, the Ledger or the bell's menu, and the
/// ritual's glass.
pub(super) fn keep_outs(labels: &mut Layout, s: &Session, me: &Knight, shown: u32) {
    for r in news_rects(s, me) {
        labels.keep(r, false);
    }
    if let Some((_, r)) = final_box(s, shown) {
        labels.keep(r, false);
    }
    if me.hold > 0 {
        labels.keep(hold_rect(), false);
    }
    if let Some(r) = hint_rect(s) {
        labels.keep(r, false);
    }
    let modals = [card_rect(s), end_rect(s), (me.down > 0).then_some(OUT_CARD), me.say.map(|_| TALK_BOX), super::sky::glass(s)];
    for r in modals.into_iter().flatten() {
        labels.keep(r, true);
    }
    match me.panel {
        Panel::Ledger(_) => labels.keep(LEDGER_PANEL, true),
        Panel::Bell(_) => labels.keep(BELL_PANEL, true),
        Panel::Shut => {}
    }
}
