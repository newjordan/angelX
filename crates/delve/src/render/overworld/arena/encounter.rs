//! A random room's game: the card that names it and says in one line how to
//! play it, the clock of a game that is cut, and the payout card
//! (`rules::encounter`). The follies keep their own titles and podiums.
use super::*;
use crate::rules::HZ;
use crate::rules::encounter::{End, Live};

/// An arcade game plays on its own stage in the hall: it is drawn as that
/// room is, from the surface (the crossroads' Powder Keg does the same).
pub(super) fn pastime(run: &Run) -> bool {
    run.encounter
        .as_ref()
        .is_some_and(|e| e.room == run.at && matches!(e.kind, crate::rules::encounter::Kind::Arcade(_)))
}

/// The one card, in view pixels, drawn over whatever the game draws. A game
/// that is not an encounter's, or a folly (which has its own), draws nothing.
pub(super) fn panel(over: &mut Img, run: &Run, _focus: Option<u32>) {
    let Some(live) = run.encounter.as_ref().filter(|e| e.room == run.at && !e.kind.folly()) else {
        return;
    };
    match &live.end {
        Some(end) => payout(over, run, live, end),
        None if live.titled() => title(over, live),
        None if live.kind.cut_by_cap() => clock(over, live),
        None => {}
    }
}

/// The name at twice the size, and one line of how to play under it.
fn title(over: &mut Img, live: &Live) {
    let name = live.kind.name();
    let words = live.kind.words();
    let big = super::super::ink::text_width(name) * 2;
    let line = super::super::ink::text_width(words);
    let w = (big.max(line) + 24).min(NATIVE_W - 8);
    let x = (NATIVE_W - w) / 2;
    chrome::panel(over, (x, 10, w, 38), chrome::Tone::BRASS);
    let mut label = Img::new(super::super::ink::text_width(name), 7);
    label.text(0, 0, name, 'O');
    over.stamp_times(&label, x + (w - big) / 2, 17, 2);
    over.text(x + (w - line) / 2, 36, words, 'O');
}

/// A game that is cut shows how long it has: a small plate in the corner.
fn clock(over: &mut Img, live: &Live) {
    let left = live.left();
    let text = format!("{}:{:02}", left / 60, left % 60);
    let w = super::super::ink::text_width(&text) + 14;
    chrome::panel(over, (NATIVE_W - w - 6, 6, w, 15), chrome::Tone::BRASS);
    over.text(NATIVE_W - w - 6 + 7, 10, &text, 'O');
}

/// Who was paid, in order, and what each took home.
fn payout(over: &mut Img, run: &Run, live: &Live, end: &End) {
    let head = if end.won {
        "WON"
    } else if end.capped {
        "TIME UP"
    } else {
        "LOST"
    };
    let rows: Vec<String> = end
        .order
        .iter()
        .enumerate()
        .map(|(place, id)| {
            let name: String = run.players.get(id).map_or("KNIGHT", |h| h.name.as_str()).chars().take(14).collect();
            let wish = if place == 0 || live.kind.co_op() { " + WISH" } else { "" };
            format!("{}  {}  +{} GOLD{}", place + 1, name.to_uppercase(), end.paid.get(id).copied().unwrap_or(0), wish)
        })
        .collect();
    let mut lines = vec![format!("{}  {}", live.kind.name(), head)];
    if rows.is_empty() {
        lines.push("THE STAKE IS LOST".into());
    }
    lines.extend(rows);
    let w = lines.iter().map(|l| super::super::ink::text_width(l)).max().unwrap_or(0) + 26;
    let h = 14 + 11 * lines.len() as i32;
    let (x, y) = ((NATIVE_W - w) / 2, 14);
    chrome::panel(over, (x, y, w, h), chrome::Tone::FRAMED);
    for (i, line) in lines.iter().enumerate() {
        let wide = super::super::ink::text_width(line);
        over.text(x + (w - wide) / 2, y + 8 + 11 * i as i32, line, if i == 0 { 'h' } else { 'O' });
    }
    let secs = end.left.div_ceil(HZ);
    let tail = format!("{}S", secs);
    over.text(x + w - super::super::ink::text_width(&tail) - 6, y + h - 9, &tail, 'J');
}
