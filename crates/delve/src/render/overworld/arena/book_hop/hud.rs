//! The strip along the foot of a seat's view, in the bottom bookcase, so no
//! lane is ever under a board: the book and the seconds, each knight's chip
//! (their ink, name, count, a flame for a run of clean crossings, a crown
//! on the leader), and the pot. The view's own HUD never shakes.

use super::super::super::chrome;
use super::super::super::ink::{Img, text_width};
use super::super::{NATIVE_W, juice, knights};
use super::bits;
use super::words::Ledger;
use crate::rules::hop::Hop;
use crate::rules::{HZ, Run};

/// The strip's top row and where its words sit.
pub(super) const TOP: i32 = 207;
pub(super) const TEXT_Y: i32 = 211;
/// Ticks a crossing's number takes to fly from the far bank to the strip.
pub(super) const FLY: u32 = 30;

const LEFT: i32 = 40;
const RIGHT: i32 = 44;

/// One knight's place in the strip.
pub(super) struct Chip {
    pub id: u32,
    pub x: i32,
    /// Letters of the name that fit.
    pub name: usize,
}

/// The chips, left to right in seat order, fitted to the strip.
pub(super) fn chips(h: &Hop) -> Vec<Chip> {
    let n = h.crossings.len().max(1) as i32;
    let each = (NATIVE_W - 8 - LEFT - RIGHT) / n;
    let name = ((each - 7 - 3 - 9 - 12) / 5).clamp(1, 5) as usize;
    h.crossings.keys().enumerate().map(|(i, &id)| Chip { id, x: 4 + LEFT + each * i as i32, name }).collect()
}

/// How the seat looking calls a knight, at most `take` letters: `YOU` for
/// themselves and only themselves; any other knight by their own name (`KNIGHT
/// n` for one who still carries the default `You`, or none).
pub(super) fn called(run: &Run, id: u32, me: Option<u32>, take: usize) -> String {
    if Some(id) == me {
        return "YOU".into();
    }
    let full = run.players.get(&id).map_or_else(String::new, |h| h.name.trim().to_uppercase());
    if full.is_empty() || full == "YOU" {
        return format!("KNIGHT {id}");
    }
    full.chars().take(take).collect()
}

/// A chip's short name: `YOU` for the seat looking, `K` and the seat for a
/// knight on the default name, else their own first letters.
pub(super) fn tag(run: &Run, id: u32, me: Option<u32>, take: usize) -> String {
    if Some(id) == me {
        return "YOU".into();
    }
    let full = run.players.get(&id).map_or_else(String::new, |h| h.name.trim().to_uppercase());
    if full.is_empty() || full == "YOU" || full.starts_with("KNIGHT ") {
        return format!("K{id}");
    }
    full.chars().take(take).collect()
}

/// Where a knight's count sits in the strip, for what flies to it.
pub(super) fn anchor(h: &Hop, run: &Run, id: u32, me: Option<u32>) -> (i32, i32) {
    chips(h)
        .iter()
        .find(|c| c.id == id)
        .map_or((NATIVE_W / 2, TEXT_Y + 3), |c| (c.x + 7 + text_width(&tag(run, id, me, c.name)) + 8, TEXT_Y + 3))
}

/// The strip: the clock, a chip for each knight, the pot. Every word is a
/// claim in the screen's ledger.
pub(super) fn strip(led: &mut Ledger, over: &mut Img, run: &Run, h: &Hop, focus: Option<u32>) {
    let now = run.tick as u32;
    chrome::panel(over, (4, TOP, NATIVE_W - 8, 14), chrome::Tone::SLATE);
    over.stamp(bits::strip_book(), 8, TOP + 3);
    let left = if h.done { 0 } else { h.left() };
    let low = left <= 10 && !h.done && !h.titled();
    let secs = if h.done { "DONE".to_string() } else { format!("{left}S") };
    let blink = low && now / 6 % 2 == 0;
    led.small(over, "clock", 20, TEXT_Y, &secs, if blink { '7' } else if low { 'R' } else { 'O' });
    let best = h.crossings.values().max().copied().unwrap_or(0);
    let rivals = h.crossings.len() > 1;
    for chip in chips(h) {
        let n = h.crossings.get(&chip.id).copied().unwrap_or(0);
        let ink = knights::seat_mark(chip.id);
        let you = focus == Some(chip.id);
        over.rect(chip.x, TOP + 4, 5, 5, 'k');
        over.rect(chip.x + 1, TOP + 5, 3, 3, ink);
        let lead = rivals && n == best && n > 0;
        if lead {
            over.stamp(bits::crown(), chip.x - 1, TOP - 2);
        }
        let name = tag(run, chip.id, focus, chip.name);
        let nx = led.small(over, "chip name", chip.x + 7, TEXT_Y, &name, if you { 'T' } else { 'i' });
        // A crossing flying in lands, and the knight's count swells.
        let landed = h
            .show
            .crosses
            .iter()
            .filter(|c| u32::from(c.who) == chip.id)
            .filter_map(|c| (now.saturating_sub(c.born)).checked_sub(FLY))
            .min();
        let pulse = landed.map_or(0.0, |age| juice::bump(age, 8));
        let num = n.to_string();
        let ink = if pulse > 0.0 { '9' } else if lead { 'T' } else { 'h' };
        let nx = led.small(over, "chip count", nx + 3, TEXT_Y - i32::from(pulse > 0.5), &num, ink);
        let run_of = h.show.run_of(chip.id);
        if run_of >= 2 {
            over.stamp(bits::flame(now / 4 + chip.id), nx + 2, TEXT_Y + 1);
            led.small(over, "chip streak", nx + 6, TEXT_Y, &run_of.min(99).to_string(), '6');
        }
    }
    // The pot, with a coin.
    let pot: u32 = h.ante.values().sum();
    let tag = format!("{pot}G");
    let px = NATIVE_W - 9 - text_width(&tag) - 8;
    over.stamp(bits::coin(), px, TEXT_Y + 1);
    led.small(over, "pot", px + 7, TEXT_Y, &tag, 'O');
    let _ = HZ;
}
