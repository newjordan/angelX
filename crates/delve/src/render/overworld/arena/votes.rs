//! A big realm-ink ballot, drawn only on the voting seat's camera.
use super::*;
use crate::rules::votes::{ITEMS, Rating};

fn star(im: &mut Img, x: i32, y: i32, filled: bool) {
    for (dy, row) in [
        "...x...", "...x...", "xxxxxxx", ".xxxxx.", "..xxx..", ".xx.xx.", "xx...xx",
    ]
    .iter()
    .enumerate()
    {
        for (dx, c) in row.chars().enumerate() {
            if c == 'x' {
                im.put(x + dx as i32, y + dy as i32, if filled { 'O' } else { 'u' });
            }
        }
    }
}
fn title(im: &mut Img, x: i32, y: i32, text: &str) {
    let mut label = Img::new(super::super::ink::text_width(text), 7);
    label.text(0, 0, text, 'O');
    for dy in 0..label.h {
        for dx in 0..label.w {
            if let Some(color) = label.get(dx, dy) {
                for oy in 0..2 {
                    for ox in 0..2 {
                        im.set(x + dx * 2 + ox, y + dy * 2 + oy, color);
                    }
                }
            }
        }
    }
}
pub(super) fn panel(over: &mut Img, run: &Run, focus: Option<u32>) {
    let id = focus.unwrap_or_else(|| run.playtest.local_seat());
    let Some(seat) = run.playtest.seats.get(&id) else {
        return;
    };
    if !seat.open {
        if seat.hint > 0 {
            let text = if seat.couch {
                "B: VOTE ON WHAT'S NEW"
            } else {
                "V: VOTE ON WHAT'S NEW"
            };
            let w = super::super::ink::text_width(text) + 20;
            let x = (NATIVE_W - w) / 2;
            chrome::panel(over, (x, 12, w, 21), chrome::Tone::BRASS);
            over.text(x + 10, 19, text, 'O');
        }
        return;
    }
    let (w, h) = (NATIVE_W - 16, NATIVE_H - 16);
    let mut im = Img::new(w + 1, h + 1);
    chrome::panel(&mut im, (0, 0, w, h), chrome::Tone::FRAMED);
    chrome::ring(&mut im, (0, 0, w, h), 4);
    title(&mut im, 14, 12, "WHAT'S NEW");
    im.text(w - 77, 16, &format!("P{id}  0.2.2"), 'O');
    im.text(14, 33, "FEATURE", 'O');
    im.text(218, 33, "YOU", 'O');
    im.text(276, 33, "PARTY", 'O');
    let mut rows = Vec::new();
    let mut group = "";
    let mut selected_row = 0;
    let selected = seat.selected.min(ITEMS.len() - 1);
    for (index, item) in ITEMS.iter().enumerate() {
        if group != item.group {
            group = item.group;
            rows.push((None, group));
        }
        if index == selected {
            selected_row = rows.len();
        }
        rows.push((Some(index), item.label));
    }
    const SHOWN: usize = 11;
    let start = selected_row
        .saturating_sub(4)
        .min(rows.len().saturating_sub(SHOWN));
    for (row, &(index, label)) in rows.iter().skip(start).take(SHOWN).enumerate() {
        let y = 47 + row as i32 * 12;
        let Some(index) = index else {
            im.text(14, y, &label.to_uppercase(), 'O');
            continue;
        };
        if index == selected {
            chrome::highlight(&mut im, (10, y - 2, w - 20, 11));
            im.text(13, y, ">", 'O');
        }
        im.text(
            24,
            y,
            &label.to_uppercase(),
            if index == selected { 'T' } else { 'i' },
        );
        let item = &ITEMS[index];
        let rating = run
            .players
            .get(&id)
            .and_then(|h| run.playtest.records.get(&h.name))
            .and_then(|r| r.get(item.id));
        match rating {
            Some(Rating::Untried(_)) => {
                im.text(214, y, "UNTRIED", 'O');
            }
            _ => {
                let stars = match rating {
                    Some(Rating::Stars(n)) => *n,
                    _ => 0,
                };
                for n in 0..5 {
                    star(&mut im, 215 + n * 9, y, n < i32::from(stars));
                }
            }
        }
        let (avg, count) = run.playtest.average(item.id);
        if count >= 2 {
            im.text(276, y, &format!("{avg:.1} ({count})"), 'O');
        }
    }
    chrome::rule(&mut im, 10, w - 11, h - 31);
    let couch = seat.couch;
    let (controls, keys) = if couch {
        (
            "I/K CHOOSE  J/L RATE  B/ESC CLOSE",
            "5-9 STARS  0 CLEAR  U HAVEN'T TRIED",
        )
    } else {
        (
            "UP/DOWN CHOOSE  LEFT/RIGHT RATE",
            "1-5 STARS  0 CLEAR  N UNTRIED  V/ESC CLOSE",
        )
    };
    im.text(14, h - 24, controls, 'i');
    im.text(14, h - 13, keys, 'O');
    over.stamp(&im, 8, 8);
}
